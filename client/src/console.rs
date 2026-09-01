use std::{io, path::PathBuf};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc,
};

const USAGE: &str = "commands: load <script-path> | reload | pause | resume | stop";

#[derive(Debug, PartialEq, Eq)]
pub enum ScriptCommand {
    Load(PathBuf),
    Reload,
    Pause,
    Resume,
    Stop,
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

pub async fn run(command_tx: mpsc::Sender<ScriptCommand>) -> io::Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();

    while let Some(line) = lines.next_line().await? {
        match parse_command(&line) {
            Ok(command) => {
                if command_tx.send(command).await.is_err() {
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

    #[test]
    fn parses_lifecycle_commands() {
        assert_eq!(parse_command("reload"), Ok(ScriptCommand::Reload));
        assert_eq!(parse_command(" pause "), Ok(ScriptCommand::Pause));
        assert_eq!(parse_command("resume"), Ok(ScriptCommand::Resume));
        assert_eq!(parse_command("stop"), Ok(ScriptCommand::Stop));
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
    }
}
