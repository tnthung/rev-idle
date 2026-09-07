mod input;
mod editing;
mod history;
mod completion;
mod parsing;

use std::{
    io::{self, Write},
    sync::{atomic::{AtomicBool, Ordering}, Arc},
    thread,
};
use tokio::sync::{mpsc, watch};

use crate::app::ScriptCommand;
use self::{
    editing::run_console_input,
    history::{history_path, load_history, save_history},
    input::Win32ConsolePlatform,
    parsing::parse_command,
};

fn dispatch_line(line: &str, command_tx: &mpsc::Sender<ScriptCommand>) -> bool {
    if line.trim() == "clear" {
        print!("\x1B[2J\x1B[H");
        let _ = io::stdout().flush();
        return false;
    }

    match parse_command(line) {
        Ok(command) => {
            let should_exit = command == ScriptCommand::Exit;
            if command_tx.blocking_send(command).is_err() {
                return true;
            }
            should_exit
        }
        Err(message) => {
            eprintln!("{message}");
            false
        }
    }
}

pub async fn run(
    command_tx: mpsc::Sender<ScriptCommand>,
    mut shutdown: watch::Receiver<bool>,
    locked: Arc<AtomicBool>,
) -> io::Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    let reader_stop = stop.clone();
    let handle = thread::spawn(move || {
        let platform = match Win32ConsolePlatform::new() {
            Ok(platform) => platform,
            Err(error) => {
                eprintln!("failed to initialize console input: {error}");
                return;
            }
        };
        let path = history_path();
        let history = load_history(&path);
        let _ = run_console_input(
            platform,
            &locked,
            &reader_stop,
            history,
            |line| dispatch_line(&line, &command_tx),
            |history| save_history(&path, history),
        );
    });

    let _ = shutdown.changed().await;
    stop.store(true, Ordering::Release);
    let _ = tokio::task::spawn_blocking(move || handle.join()).await;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_line_forwards_parsed_commands_and_flags_exit() {
        let (tx, mut rx) = mpsc::channel(4);
        assert!(!dispatch_line("reload", &tx));
        assert!(dispatch_line("exit", &tx));
        drop(tx);

        assert_eq!(rx.blocking_recv(), Some(ScriptCommand::Reload));
        assert_eq!(rx.blocking_recv(), Some(ScriptCommand::Exit));
    }

    #[test]
    fn dispatch_line_clears_screen_locally_without_forwarding_a_command() {
        let (tx, mut rx) = mpsc::channel(4);
        assert!(!dispatch_line("clear", &tx));
        drop(tx);

        assert_eq!(rx.blocking_recv(), None);
    }
}
