mod command;
mod pause;
mod status;

pub(crate) use command::ScriptCommand;
pub(crate) use pause::{ActionGate, PauseUpdate};
pub(crate) use status::{ScriptPhase, StateUpdate};

use std::{
    io,
    net::SocketAddr,
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
    StopCapture,
    Ignore,
}

fn ctrl_c_action(script_running: bool, capture_enabled: bool) -> CtrlCAction {
    if script_running {
        CtrlCAction::StopScript
    } else if capture_enabled {
        CtrlCAction::StopCapture
    } else {
        CtrlCAction::Ignore
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
        assert_eq!(ctrl_c_action(true, false), CtrlCAction::StopScript);
        assert_eq!(ctrl_c_action(true, true), CtrlCAction::StopScript);
    }

    #[test]
    fn ctrl_c_stops_capture_or_is_ignored_when_idle() {
        assert_eq!(ctrl_c_action(false, true), CtrlCAction::StopCapture);
        assert_eq!(ctrl_c_action(false, false), CtrlCAction::Ignore);
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

pub(crate) async fn run() -> io::Result<()> {
    let initial_path = initial_script_path(std::env::args_os());
    let actions_paused = ActionGate::default();
    let (pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
    let (command_tx, command_rx) = mpsc::channel(32);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let script_running = Arc::new(AtomicBool::new(false));
    let console_locked = Arc::new(AtomicBool::new(false));
    let capture_state = crate::capture::CaptureState::default();
    let lock_state = crate::capture::LockState::default();
    let console_commands = command_tx.clone();
    let hotkey = crate::hotkey::HotkeyWorker::start(actions_paused.clone(), pause_tx, command_tx.clone())
        .map_err(io::Error::other)?;
    let connection = crate::bridge::WsConnection::connect(SocketAddr::from((
        [127, 0, 0, 1],
        19841,
    )));
    if let Err(error) = crate::bridge::register_control_handlers(&connection, command_tx.clone()) {
        connection.shutdown().await;
        return Err(io::Error::other(error));
    }
    let (state_tx, state_rx) = watch::channel(crate::app::StateUpdate::new(
        initial_path.is_some(),
        false,
        false,
        capture_state.is_enabled(),
        lock_state.is_enabled(),
    ));
    let capture = match crate::capture::CaptureWorker::start(connection.clone(), command_tx.clone()) {
        Ok(capture) => capture,
        Err(error) => {
            connection.shutdown().await;
            return Err(io::Error::other(error));
        }
    };
    let script_connection = connection.clone();
    let publisher_connection = connection.clone();
    let local = LocalSet::new();

    let result = local
        .run_until(async move {
            let mut tasks = JoinSet::new();
            let console_shutdown = shutdown_rx.clone();
            let console_locked_for_task = console_locked.clone();
            tasks.spawn_local(async move {
                crate::console::run(console_commands, console_shutdown, console_locked_for_task).await
            });
            let publisher_shutdown = shutdown_rx.clone();
            let publisher_connection = publisher_connection.clone();
            let publisher_generation = publisher_connection.connection_generation();
            tasks.spawn_local(async move {
                crate::bridge::publish_state(
                    publisher_connection,
                    state_rx,
                    publisher_generation,
                    publisher_shutdown,
                )
                .await;
                Ok(())
            });
            let script_shutdown = shutdown_rx.clone();
            let script_running_for_task = script_running.clone();
            tasks.spawn_local(async move {
                crate::script::run(
                    command_rx,
                    script_connection,
                    pause_rx,
                    initial_path,
                    actions_paused,
                    script_shutdown,
                    script_running_for_task,
                    console_locked,
                    capture_state,
                    lock_state,
                    state_tx,
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
                        match ctrl_c_action(
                            script_running.load(Ordering::Acquire),
                            capture_state.is_enabled(),
                        ) {
                            CtrlCAction::StopScript => {
                                capture_state.set_enabled(false);
                                let _ = command_tx.send(ScriptCommand::Stop).await;
                            }
                            CtrlCAction::StopCapture => {
                                let _ = command_tx.send(ScriptCommand::StopCapture).await;
                            }
                            CtrlCAction::Ignore => {}
                        }
                    }
                }
            };

            let result = finish_shutdown(result, hotkey.shutdown().map_err(io::Error::other));
            match (result, capture.shutdown().map_err(io::Error::other)) {
                (Ok(()), Ok(())) => Ok(()),
                (Err(error), Ok(())) => Err(error),
                (Ok(()), Err(error)) => Err(error),
                (Err(error), Err(capture_error)) => Err(io::Error::other(
                    format!("{error}; mouse capture shutdown failed: {capture_error}"),
                )),
            }
        })
        .await;
    connection.shutdown().await;
    result
}
