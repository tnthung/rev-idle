use std::path::PathBuf;

use crate::app::ScriptCommand;

const USAGE: &str = "commands: load <script-path> | reload | resume | capture | clear | exit";

// `pause`/`stop` are intentionally not parseable here: the hotkey (F8) and
// Ctrl+C are the only ways to pause/stop a running script. `ScriptCommand`
// keeps the `Pause`/`Stop` variants because the script lifecycle (and its
// tests) still drive them directly.
pub(super) fn parse_command(line: &str) -> Result<ScriptCommand, String> {
    let input = line.trim();
    let command_end = input.find(char::is_whitespace).unwrap_or(input.len());
    let (name, remainder) = input.split_at(command_end);
    let argument = remainder.trim();

    match name {
        "reload" if argument.is_empty() => Ok(ScriptCommand::Reload),
        "resume" if argument.is_empty() => Ok(ScriptCommand::Resume),
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lifecycle_commands() {
        assert_eq!(parse_command("reload"), Ok(ScriptCommand::Reload));
        assert_eq!(parse_command(" resume "), Ok(ScriptCommand::Resume));
        assert_eq!(parse_command("exit"), Ok(ScriptCommand::Exit));
    }

    #[test]
    fn rejects_the_removed_pause_and_stop_commands() {
        assert!(parse_command("pause").is_err());
        assert!(parse_command("stop").is_err());
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
}
