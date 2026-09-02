mod window;
mod console;
mod script;
mod udp;
mod hotkey;

use std::{
    io,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{
    sync::{mpsc, watch},
    task::{JoinError, JoinSet, LocalSet},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CtrlCAction {
    StopScript,
    ShutdownClient,
}

fn ctrl_c_action(script_running: bool) -> CtrlCAction {
    if script_running {
        CtrlCAction::StopScript
    } else {
        CtrlCAction::ShutdownClient
    }
}

fn finish_shutdown(selected: io::Result<()>, shutdown: io::Result<()>) -> io::Result<()> {
    match (selected, shutdown) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(shutdown_error)) => Err(io::Error::other(
            format!("{error}; hotkey shutdown failed: {shutdown_error}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn cleanup_error_is_returned_when_selected_task_succeeds() {
        let result = finish_shutdown(Ok(()), Err(io::Error::other("cleanup failed")));
        assert_eq!(result.unwrap_err().to_string(), "cleanup failed");
    }

    #[test]
    fn selected_task_error_is_preserved_when_cleanup_succeeds() {
        let result = finish_shutdown(Err(io::Error::other("task failed")), Ok(()));
        assert_eq!(result.unwrap_err().to_string(), "task failed");
    }

    #[test]
    fn omitted_cli_script_path_starts_without_an_initial_script() {
        assert_eq!(initial_script_path(vec![OsString::from("rev-idle")]), None);
    }

    #[test]
    fn ctrl_c_with_a_running_script_requests_script_stop() {
        assert_eq!(ctrl_c_action(true), CtrlCAction::StopScript);
    }

    #[test]
    fn ctrl_c_without_a_running_script_requests_client_shutdown() {
        assert_eq!(ctrl_c_action(false), CtrlCAction::ShutdownClient);
    }
}

fn initial_script_path<I>(args: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    args.into_iter().nth(1).map(PathBuf::from)
}

fn task_result(result: Result<io::Result<()>, JoinError>) -> io::Result<()> {
    result.map_err(|error| io::Error::other(error.to_string()))?
}

async fn shutdown_tasks(
    mut tasks: JoinSet<io::Result<()>>,
    initial_result: io::Result<()>,
) -> io::Result<()> {
    let mut result = initial_result;
    while let Some(task) = tasks.join_next().await {
        if result.is_ok() {
            result = task_result(task);
        }
    }
    result
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let initial_path = initial_script_path(std::env::args_os());
    let (state_tx, state_rx) = watch::channel(udp::State::default());
    let actions_paused = hotkey::ActionGate::default();
    let (pause_tx, pause_rx) = watch::channel(hotkey::PauseUpdate::initial());
    let (command_tx, command_rx) = mpsc::channel(32);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let script_running = Arc::new(AtomicBool::new(false));
    let console_commands = command_tx.clone();
    let hotkey = hotkey::HotkeyWorker::start(actions_paused.clone(), pause_tx)
        .map_err(io::Error::other)?;
    let local = LocalSet::new();

    local
        .run_until(async move {
            let mut tasks = JoinSet::new();
            let udp_shutdown = shutdown_rx.clone();
            tasks.spawn_local(async move {
                udp::run(state_tx, udp_shutdown).await
            });
            let console_shutdown = shutdown_rx.clone();
            tasks.spawn_local(async move {
                console::run(console_commands, console_shutdown).await
            });
            let script_shutdown = shutdown_rx.clone();
            let script_running_for_task = script_running.clone();
            tasks.spawn_local(async move {
                script::run(
                    command_rx,
                    state_rx,
                    pause_rx,
                    initial_path,
                    actions_paused,
                    script_shutdown,
                    script_running_for_task,
                )
                .await
                .map_err(io::Error::other)
            });

            let result = loop {
                tokio::select! {
                    Some(result) = tasks.join_next() => {
                        let result = task_result(result);
                        shutdown_tx.send_replace(true);
                        break shutdown_tasks(tasks, result).await;
                    }
                    result = tokio::signal::ctrl_c() => {
                        result?;
                        match ctrl_c_action(script_running.load(Ordering::Acquire)) {
                            CtrlCAction::StopScript => {
                                if command_tx.send(console::ScriptCommand::Stop).await.is_err() {
                                    shutdown_tx.send_replace(true);
                                    break shutdown_tasks(
                                        tasks,
                                        Err(io::Error::other("script command channel closed")),
                                    ).await;
                                }
                            }
                            CtrlCAction::ShutdownClient => {
                                shutdown_tx.send_replace(true);
                                break shutdown_tasks(tasks, Ok(())).await;
                            }
                        }
                    }
                }
            };

            let shutdown = hotkey.shutdown().map_err(io::Error::other);
            finish_shutdown(result, shutdown)
        })
        .await
}
