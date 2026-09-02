mod window;
mod console;
mod script;
mod udp;
mod hotkey;

use std::{
    io,
    path::PathBuf,
};
use tokio::{
    sync::{mpsc, watch},
    task::LocalSet,
};

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
}

fn initial_script_path<I>(args: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    args.into_iter().nth(1).map(PathBuf::from)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let initial_path = initial_script_path(std::env::args_os());
    let (state_tx, state_rx) = watch::channel(udp::State::default());
    let actions_paused = hotkey::ActionGate::default();
    let (pause_tx, pause_rx) = watch::channel(hotkey::PauseUpdate::initial());
    let (command_tx, command_rx) = mpsc::channel(32);
    let hotkey = hotkey::HotkeyWorker::start(actions_paused.clone(), pause_tx)
        .map_err(io::Error::other)?;
    let local = LocalSet::new();

    local
        .run_until(async move {
            let mut udp_task = tokio::task::spawn_local(udp::run(state_tx));
            let mut console_task =
                tokio::task::spawn_local(console::run(command_tx));
            let mut script_task = tokio::task::spawn_local(script::run(
                command_rx,
                state_rx,
                pause_rx,
                initial_path,
                actions_paused,
            ));

            let result = tokio::select! {
                result = &mut udp_task => {
                    result
                        .map_err(|error| io::Error::other(error.to_string()))
                        .and_then(|result| result)
                }
                result = &mut console_task => {
                    result
                        .map_err(|error| io::Error::other(error.to_string()))
                        .and_then(|result| result)
                }
                result = &mut script_task => {
                    result
                        .map_err(|error| io::Error::other(error.to_string()))
                        .and_then(|result| result.map_err(io::Error::other))
                }
                result = tokio::signal::ctrl_c() => result,
            };

            udp_task.abort();
            console_task.abort();
            script_task.abort();
            let shutdown = hotkey.shutdown().map_err(io::Error::other);
            finish_shutdown(result, shutdown)
        })
        .await
}
