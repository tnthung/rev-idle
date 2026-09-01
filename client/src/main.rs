mod console;
mod script;
mod udp;

use std::{io, path::PathBuf};
use tokio::{
    sync::{mpsc, watch},
    task::LocalSet,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let initial_path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("script.js"));
    let (state_tx, state_rx) = watch::channel(udp::State::default());
    let (command_tx, command_rx) = mpsc::channel(32);
    let local = LocalSet::new();

    local
        .run_until(async move {
            let mut udp_task = tokio::task::spawn_local(udp::run(state_tx));
            let mut console_task =
                tokio::task::spawn_local(console::run(command_tx));
            let mut script_task = tokio::task::spawn_local(script::run(
                command_rx,
                state_rx,
                initial_path,
            ));

            let result = tokio::select! {
                result = &mut udp_task => {
                    result
                        .map_err(|error| io::Error::other(error.to_string()))?
                }
                result = &mut console_task => {
                    result
                        .map_err(|error| io::Error::other(error.to_string()))?
                }
                result = &mut script_task => {
                    result
                        .map_err(|error| io::Error::other(error.to_string()))?
                        .map_err(io::Error::other)
                }
                result = tokio::signal::ctrl_c() => result,
            };

            udp_task.abort();
            console_task.abort();
            script_task.abort();
            result
        })
        .await
}
