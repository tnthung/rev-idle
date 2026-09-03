use std::{io, path::PathBuf};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    sync::{mpsc, watch},
};

const USAGE: &str = "commands: load <script-path> | reload | pause | resume | stop | capture | clear | exit";

#[derive(Debug, PartialEq, Eq)]
pub enum ScriptCommand {
    Load(PathBuf),
    Reload,
    Pause,
    Resume,
    Stop,
    Capture,
    Exit,
    #[allow(dead_code)]
    SetPaused(bool),
}

pub fn parse_command(line: &str) -> Result<ScriptCommand, String> {
    let input = line.trim();
    let command_end = input.find(char::is_whitespace).unwrap_or(input.len());
    let (name, remainder) = input.split_at(command_end);
    let argument = remainder.trim();

    match name {
        "reload" if argument.is_empty() => Ok(ScriptCommand::Reload),
        "pause" if argument.is_empty() => Ok(ScriptCommand::Pause),
        "resume" if argument.is_empty() => Ok(ScriptCommand::Resume),
        "stop" if argument.is_empty() => Ok(ScriptCommand::Stop),
        "capture" if argument.is_empty() => Ok(ScriptCommand::Capture),
        "exit" if argument.is_empty() => Ok(ScriptCommand::Exit),
        "load" => {
            let path = if argument.len() >= 2
                && argument.starts_with('"')
                && argument.ends_with('"')
            {
                &argument[1..argument.len() - 1]
            } else {
                argument
            };

            if path.is_empty() {
                Err(USAGE.to_owned())
            } else {
                Ok(ScriptCommand::Load(PathBuf::from(path)))
            }
        }
        _ => Err(USAGE.to_owned()),
    }
}

pub async fn run(
    command_tx: mpsc::Sender<ScriptCommand>,
    shutdown: watch::Receiver<bool>,
) -> io::Result<()> {
    run_with_input(tokio::io::stdin(), command_tx, shutdown).await
}

async fn run_with_input<R: AsyncRead + Unpin>(
    input: R,
    command_tx: mpsc::Sender<ScriptCommand>,
    mut shutdown: watch::Receiver<bool>,
) -> io::Result<()> {
    let mut lines = BufReader::new(input).lines();

    loop {
        let line = tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return Ok(());
                }
                continue;
            }
            line = lines.next_line() => line?,
        };
        let Some(line) = line else { break };

        if line.trim() == "clear" {
            print!("\x1B[2J\x1B[H");
            let _ = io::Write::flush(&mut io::stdout());
            continue;
        }

        match parse_command(&line) {
            Ok(command) => {
                let should_exit = command == ScriptCommand::Exit;
                if command_tx.send(command).await.is_err() {
                    return Ok(());
                }
                if should_exit {
                    return Ok(());
                }
            }
            Err(message) => eprintln!("{message}"),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tokio::io::AsyncWriteExt;

    #[test]
    fn parses_lifecycle_commands() {
        assert_eq!(parse_command("reload"), Ok(ScriptCommand::Reload));
        assert_eq!(parse_command(" pause "), Ok(ScriptCommand::Pause));
        assert_eq!(parse_command("resume"), Ok(ScriptCommand::Resume));
        assert_eq!(parse_command("stop"), Ok(ScriptCommand::Stop));
        assert_eq!(parse_command("exit"), Ok(ScriptCommand::Exit));
    }

    #[test]
    fn parses_mouse_capture_command() {
        assert_eq!(parse_command("capture"), Ok(ScriptCommand::Capture));
        assert!(parse_command("capture now").is_err());
    }

    #[test]
    fn load_uses_the_entire_remaining_path() {
        assert_eq!(
            parse_command(r#"load C:\My Scripts\farm.js"#),
            Ok(ScriptCommand::Load(PathBuf::from(
                r#"C:\My Scripts\farm.js"#
            )))
        );
        assert_eq!(
            parse_command(r#"load "C:\My Scripts\farm.js""#),
            Ok(ScriptCommand::Load(PathBuf::from(
                r#"C:\My Scripts\farm.js"#
            )))
        );
    }

    #[test]
    fn rejects_unknown_commands_and_empty_load_paths() {
        assert!(parse_command("").is_err());
        assert!(parse_command("start").is_err());
        assert!(parse_command("load").is_err());
        assert!(parse_command(r#"load """#).is_err());
        assert!(parse_command("pause now").is_err());
        assert!(parse_command("exit now").is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn clear_is_handled_locally_and_never_forwarded_as_a_command() {
        let (mut input, output) = tokio::io::duplex(64);
        let (command_tx, mut command_rx) = mpsc::channel(1);
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let runner = tokio::spawn(run_with_input(output, command_tx, shutdown_rx));

        input.write_all(b"clear\nexit\n").await.unwrap();
        assert_eq!(command_rx.recv().await, Some(ScriptCommand::Exit));

        tokio::time::timeout(std::time::Duration::from_millis(200), runner)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn exit_returns_without_waiting_for_more_input() {
        let (mut input, output) = tokio::io::duplex(64);
        let (command_tx, mut command_rx) = mpsc::channel(1);
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let mut runner = tokio::spawn(run_with_input(output, command_tx, shutdown_rx));

        input.write_all(b"exit\n").await.unwrap();
        assert_eq!(command_rx.recv().await, Some(ScriptCommand::Exit));

        match tokio::time::timeout(std::time::Duration::from_millis(30), &mut runner).await {
            Ok(result) => result.unwrap().unwrap(),
            Err(_) => {
                runner.abort();
                let _ = runner.await;
                panic!("console waited for more input after exit");
            }
        }
    }
}
