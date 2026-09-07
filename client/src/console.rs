use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};
use tokio::sync::{mpsc, watch};
use windows::Win32::{
    Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::{
        Console::{
            GetConsoleMode, GetStdHandle, ReadConsoleInputW, SetConsoleMode, CONSOLE_MODE,
            ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, INPUT_RECORD, KEY_EVENT, STD_INPUT_HANDLE,
        },
        Threading::WaitForSingleObject,
    },
    UI::Input::KeyboardAndMouse::{VK_BACK, VK_DOWN, VK_RETURN, VK_TAB, VK_UP},
};

const USAGE: &str = "commands: load <script-path> | reload | resume | capture | clear | exit";

#[derive(Debug, PartialEq, Eq)]
pub enum ScriptCommand {
    Load(PathBuf),
    Reload,
    #[allow(dead_code)]
    Pause,
    Resume,
    Stop,
    Capture,
    Exit,
    #[allow(dead_code)]
    SetPaused(bool),
}

// `pause`/`stop` are intentionally not parseable here: the hotkey (F8) and
// Ctrl+C are the only ways to pause/stop a running script. `ScriptCommand`
// keeps the `Pause`/`Stop` variants because script.rs's lifecycle (and its
// tests) still drive them directly.
pub fn parse_command(line: &str) -> Result<ScriptCommand, String> {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConsoleEvent {
    Char(char),
    Backspace,
    Enter,
    Up,
    Down,
    Tab,
    Ignored,
}

trait ConsolePlatform {
    fn next_event(&mut self) -> Result<ConsoleEvent, String>;
}

struct Win32ConsolePlatform {
    input: HANDLE,
    original_mode: CONSOLE_MODE,
}

impl Win32ConsolePlatform {
    /// Switches the input handle out of cooked mode (line buffering +
    /// automatic echo) so keystrokes reach us one at a time and are only
    /// echoed when we choose to. `Drop` restores whatever mode was active
    /// before.
    fn new() -> Result<Self, String> {
        let input = unsafe { GetStdHandle(STD_INPUT_HANDLE) }
            .map_err(|error| format!("GetStdHandle failed: {error}"))?;
        let mut original_mode = CONSOLE_MODE(0);
        unsafe { GetConsoleMode(input, &mut original_mode) }
            .map_err(|error| format!("GetConsoleMode failed: {error}"))?;
        let raw_mode = CONSOLE_MODE(original_mode.0 & !(ENABLE_LINE_INPUT.0 | ENABLE_ECHO_INPUT.0));
        unsafe { SetConsoleMode(input, raw_mode) }
            .map_err(|error| format!("SetConsoleMode failed: {error}"))?;
        Ok(Self { input, original_mode })
    }
}

impl Drop for Win32ConsolePlatform {
    fn drop(&mut self) {
        let _ = unsafe { SetConsoleMode(self.input, self.original_mode) };
    }
}

impl ConsolePlatform for Win32ConsolePlatform {
    fn next_event(&mut self) -> Result<ConsoleEvent, String> {
        loop {
            // A bounded wait (rather than blocking forever) lets the caller's
            // stop flag be noticed promptly even when nothing is typed.
            match unsafe { WaitForSingleObject(self.input, 200) } {
                WAIT_TIMEOUT => return Ok(ConsoleEvent::Ignored),
                WAIT_OBJECT_0 => {}
                _ => return Err("WaitForSingleObject on console input failed".to_string()),
            }

            let mut record = INPUT_RECORD::default();
            let mut read = 0u32;
            unsafe { ReadConsoleInputW(self.input, std::slice::from_mut(&mut record), &mut read) }
                .map_err(|error| format!("ReadConsoleInputW failed: {error}"))?;

            if read == 0 || record.EventType as u32 != KEY_EVENT {
                continue;
            }

            let key = unsafe { record.Event.KeyEvent };
            if !key.bKeyDown.as_bool() {
                continue;
            }

            if key.wVirtualKeyCode == VK_RETURN.0 {
                return Ok(ConsoleEvent::Enter);
            }
            if key.wVirtualKeyCode == VK_BACK.0 {
                return Ok(ConsoleEvent::Backspace);
            }
            if key.wVirtualKeyCode == VK_UP.0 {
                return Ok(ConsoleEvent::Up);
            }
            if key.wVirtualKeyCode == VK_DOWN.0 {
                return Ok(ConsoleEvent::Down);
            }
            if key.wVirtualKeyCode == VK_TAB.0 {
                return Ok(ConsoleEvent::Tab);
            }

            let unicode = unsafe { key.uChar.UnicodeChar };
            match char::from_u32(u32::from(unicode)) {
                Some(ch) if !ch.is_control() => return Ok(ConsoleEvent::Char(ch)),
                _ => continue,
            }
        }
    }
}

const HISTORY_LIMIT: usize = 500;

fn history_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("rev-idle").join("console_history.txt")
}

fn load_history(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .map(|contents| {
            contents
                .lines()
                .map(str::to_owned)
                .filter(|line| !line.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn save_history(path: &Path, history: &[String]) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, history.join("\n"));
}

fn push_history(history: &mut Vec<String>, line: &str) -> bool {
    if line.trim().is_empty() || history.last().is_some_and(|last| last == line) {
        return false;
    }
    history.push(line.to_owned());
    if history.len() > HISTORY_LIMIT {
        history.remove(0);
    }
    true
}

fn replace_line(buffer: &mut String, content: String) {
    print!("\r\x1B[K{content}");
    let _ = io::stdout().flush();
    *buffer = content;
}

/// Drives the console's line editing: builds up `buffer` one keystroke at a
/// time, echoing as it goes, and hands completed lines to `on_line`. Up/Down
/// walk `history` (oldest first, most recent last) the way a normal cooked
/// console does; `on_history_changed` is called with the updated history
/// whenever a line is actually recorded, so the caller can persist it.
///
/// While `locked` is set (a script is running, unpaused) keystrokes â€” including
/// Enter â€” are dropped with no echo and no effect; any partial line typed just
/// before the lock engaged is discarded rather than left to be submitted once
/// unlocked. `stop` ends the loop (checked before and after each blocking
/// read) without requiring another keystroke.
#[allow(clippy::too_many_arguments)]
fn run_console_input<P: ConsolePlatform>(
    mut platform: P,
    locked: &AtomicBool,
    stop: &AtomicBool,
    mut history: Vec<String>,
    mut on_line: impl FnMut(String) -> bool,
    mut on_history_changed: impl FnMut(&[String]),
) -> Result<(), String> {
    let mut buffer = String::new();
    // `Some(i)` while browsing `history[i]`; `None` means the line is the
    // user's own fresh (possibly empty) draft, saved here so Down can restore
    // it after browsing back up through history.
    let mut history_index: Option<usize> = None;
    let mut draft = String::new();
    let mut completions: Vec<String> = Vec::new();
    let mut completion_index: Option<usize> = None;

    while !stop.load(Ordering::Acquire) {
        let event = platform.next_event()?;

        if stop.load(Ordering::Acquire) {
            break;
        }

        if locked.load(Ordering::Acquire) {
            if !buffer.is_empty() || completion_index.is_some() {
                print!("\r\x1B[K");
                let _ = io::stdout().flush();
                buffer.clear();
            }
            history_index = None;
            draft.clear();
            completions.clear();
            completion_index = None;
            continue;
        }

        if !matches!(event, ConsoleEvent::Tab | ConsoleEvent::Ignored) {
            if let Some(index) = completion_index.take() {
                if matches!(event, ConsoleEvent::Char(' ' | '/' | '\\') | ConsoleEvent::Enter) {
                    buffer.push_str(&completions[index]);
                }
                print!("\r\x1B[K\x1B[97m{buffer}\x1B[39m");
                let _ = io::stdout().flush();
            }
            completions.clear();
        }

        match event {
            ConsoleEvent::Char(ch) => {
                buffer.push(ch);
                print!("\x1B[97m{ch}\x1B[39m");
                let _ = io::stdout().flush();
            }
            ConsoleEvent::Backspace => {
                if buffer.pop().is_some() {
                    print!("\u{8} \u{8}");
                    let _ = io::stdout().flush();
                }
            }
            ConsoleEvent::Up => {
                if history.is_empty() {
                    continue;
                }
                let next_index = match history_index {
                    None => {
                        draft = buffer.clone();
                        history.len() - 1
                    }
                    Some(0) => 0,
                    Some(index) => index - 1,
                };
                history_index = Some(next_index);
                replace_line(&mut buffer, history[next_index].clone());
            }
            ConsoleEvent::Down => match history_index {
                None => {}
                Some(index) if index + 1 < history.len() => {
                    history_index = Some(index + 1);
                    replace_line(&mut buffer, history[index + 1].clone());
                }
                Some(_) => {
                    history_index = None;
                    replace_line(&mut buffer, std::mem::take(&mut draft));
                }
            },
            ConsoleEvent::Enter => {
                println!();
                let line = std::mem::take(&mut buffer);
                history_index = None;
                draft.clear();
                if push_history(&mut history, &line) {
                    on_history_changed(&history);
                }
                if on_line(line) {
                    return Ok(());
                }
            }
            ConsoleEvent::Tab => {
                if let Some(index) = completion_index {
                    completion_index = Some((index + 1) % completions.len());
                } else {
                    let input = buffer.trim_start();
                    if let Some(command_end) = input.find(char::is_whitespace) {
                        if &input[..command_end] == "load" {
                            let argument = input[command_end..].trim_start();
                            let path = argument.strip_prefix('"').unwrap_or(argument);
                            let component_start = path.rfind(['/', '\\']).map_or(0, |index| index + 1);
                            let (directory, prefix) = path.split_at(component_start);
                            if let Ok(entries) = fs::read_dir(if directory.is_empty() { "." } else { directory }) {
                                for entry in entries.flatten() {
                                    let Ok(file_type) = entry.file_type() else { continue };
                                    if !file_type.is_dir() && !entry.path().extension().is_some_and(|extension| extension.eq_ignore_ascii_case("js")) {
                                        continue;
                                    }
                                    let name = entry.file_name();
                                    let Some(name) = name.to_str() else { continue };
                                    if name.get(..prefix.len()).is_some_and(|start| start.eq_ignore_ascii_case(prefix)) {
                                        completions.push(format!("{}{}", &name[prefix.len()..], if argument.starts_with('"') && !file_type.is_dir() { "\"" } else { "" }));
                                    }
                                }
                            }
                            completions.sort();
                        }
                    } else {
                        completions.extend(["load", "reload", "resume", "capture", "clear", "exit"]
                            .into_iter()
                            .filter_map(|command| command.strip_prefix(input).map(str::to_owned)));
                    }
                    if !completions.is_empty() {
                        completion_index = Some(0);
                    }
                }
                if let Some(index) = completion_index {
                    print!("\r\x1B[K\x1B[97m{buffer}\x1B[90m{}\x1B[39m", completions[index]);
                    let _ = io::stdout().flush();
                }
            }
            ConsoleEvent::Ignored => {}
        }
    }

    Ok(())
}

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

    struct FakeConsolePlatform {
        events: Vec<ConsoleEvent>,
    }

    impl FakeConsolePlatform {
        fn events(events: impl IntoIterator<Item = ConsoleEvent>) -> Self {
            let mut events: Vec<_> = events.into_iter().collect();
            events.reverse();
            Self { events }
        }
    }

    impl ConsolePlatform for FakeConsolePlatform {
        fn next_event(&mut self) -> Result<ConsoleEvent, String> {
            self.events.pop().ok_or_else(|| "fake console exhausted".to_string())
        }
    }

    fn chars(text: &str) -> impl Iterator<Item = ConsoleEvent> + '_ {
        text.chars().map(ConsoleEvent::Char)
    }

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

    #[test]
    fn submits_a_line_on_enter_and_stops_when_on_line_requests_it() {
        let events = chars("exit").chain([ConsoleEvent::Enter]);
        let platform = FakeConsolePlatform::events(events);
        let locked = AtomicBool::new(false);
        let stop = AtomicBool::new(false);
        let mut lines = Vec::new();

        let result = run_console_input(platform, &locked, &stop, Vec::new(), |line| {
            lines.push(line);
            true
        }, |_| {});

        assert_eq!(result, Ok(()));
        assert_eq!(lines, vec!["exit".to_string()]);
    }

    #[test]
    fn backspace_removes_the_previous_character() {
        let events = [
            ConsoleEvent::Char('h'),
            ConsoleEvent::Char('x'),
            ConsoleEvent::Backspace,
            ConsoleEvent::Char('i'),
            ConsoleEvent::Enter,
        ];
        let platform = FakeConsolePlatform::events(events);
        let locked = AtomicBool::new(false);
        let stop = AtomicBool::new(false);
        let mut lines = Vec::new();

        run_console_input(platform, &locked, &stop, Vec::new(), |line| {
            lines.push(line);
            true
        }, |_| {})
        .unwrap();

        assert_eq!(lines, vec!["hi".to_string()]);
    }

    #[test]
    fn tab_cycles_commands_and_space_commits_the_preview() {
        for (events, expected) in [
            (chars("l").chain([ConsoleEvent::Tab, ConsoleEvent::Char(' '), ConsoleEvent::Enter]).collect::<Vec<_>>(), "load "),
            (chars("re").chain([ConsoleEvent::Tab, ConsoleEvent::Ignored, ConsoleEvent::Tab, ConsoleEvent::Enter]).collect(), "resume"),
            (chars("l").chain([ConsoleEvent::Tab, ConsoleEvent::Char('x'), ConsoleEvent::Enter]).collect(), "lx"),
        ] {
            let mut lines = Vec::new();
            run_console_input(FakeConsolePlatform::events(events), &AtomicBool::new(false), &AtomicBool::new(false), Vec::new(), |line| {
                lines.push(line);
                true
            }, |_| {}).unwrap();
            assert_eq!(lines, vec![expected]);
        }
    }

    #[test]
    fn tab_completes_script_paths_and_slash_commits_directories() {
        let dir = std::env::temp_dir().join(format!("rev-idle-completion-{}", std::process::id()));
        fs::create_dir_all(dir.join("scripts")).unwrap();
        fs::write(dir.join("scripts/farm script.js"), "").unwrap();
        fs::write(dir.join("scripts/farm.txt"), "").unwrap();
        let prefix = format!("load {}/scr", dir.display());
        let events = chars(&prefix)
            .chain([ConsoleEvent::Tab, ConsoleEvent::Char('/')])
            .chain(chars("fa"))
            .chain([ConsoleEvent::Tab, ConsoleEvent::Enter]);
        let mut lines = Vec::new();
        run_console_input(FakeConsolePlatform::events(events), &AtomicBool::new(false), &AtomicBool::new(false), Vec::new(), |line| {
            lines.push(line);
            true
        }, |_| {}).unwrap();
        assert_eq!(lines, vec![format!("load {}/scripts/farm script.js", dir.display())]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn keystrokes_including_enter_are_dropped_while_locked() {
        let events = chars("ex").chain([ConsoleEvent::Enter]);
        let platform = FakeConsolePlatform::events(events);
        let locked = AtomicBool::new(true);
        let stop = AtomicBool::new(false);
        let mut lines: Vec<String> = Vec::new();

        // Nothing ever unlocks, so the fake platform runs out of events and
        // reports that as an error instead of a submitted line.
        let result = run_console_input(platform, &locked, &stop, Vec::new(), |line| {
            lines.push(line);
            false
        }, |_| {});

        assert!(result.is_err());
        assert!(lines.is_empty());
    }

    #[test]
    fn stop_flag_ends_the_loop_without_requiring_more_events() {
        let platform = FakeConsolePlatform::events([]);
        let locked = AtomicBool::new(false);
        let stop = AtomicBool::new(true);

        let result = run_console_input(platform, &locked, &stop, Vec::new(), |_| false, |_| {});

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn up_and_down_walk_history_and_restore_the_in_progress_draft() {
        let events = [ConsoleEvent::Char('x')]
            .into_iter()
            .chain([ConsoleEvent::Up, ConsoleEvent::Up, ConsoleEvent::Down, ConsoleEvent::Enter]);
        let platform = FakeConsolePlatform::events(events);
        let locked = AtomicBool::new(false);
        let stop = AtomicBool::new(false);
        let history = vec!["load a.js".to_string(), "reload".to_string()];
        let mut lines = Vec::new();

        // Typed "x", then Up (-> "reload"), Up again (-> "load a.js", the
        // oldest entry, so a third Up would have no effect), Down (back to
        // "reload"), Enter submits whatever is on the line at that point.
        run_console_input(platform, &locked, &stop, history, |line| {
            lines.push(line);
            true
        }, |_| {})
        .unwrap();

        assert_eq!(lines, vec!["reload".to_string()]);
    }

    #[test]
    fn down_past_the_newest_entry_restores_the_draft_typed_before_browsing() {
        let events = chars("hi").chain([ConsoleEvent::Up, ConsoleEvent::Down, ConsoleEvent::Enter]);
        let platform = FakeConsolePlatform::events(events);
        let locked = AtomicBool::new(false);
        let stop = AtomicBool::new(false);
        let history = vec!["reload".to_string()];
        let mut lines = Vec::new();

        run_console_input(platform, &locked, &stop, history, |line| {
            lines.push(line);
            true
        }, |_| {})
        .unwrap();

        assert_eq!(lines, vec!["hi".to_string()]);
    }

    #[test]
    fn submitted_lines_are_appended_to_history_without_immediate_duplicates() {
        let events = chars("reload").chain([ConsoleEvent::Enter]).chain(chars("reload")).chain([ConsoleEvent::Enter]);
        let platform = FakeConsolePlatform::events(events);
        let locked = AtomicBool::new(false);
        let stop = AtomicBool::new(false);
        let mut saved = Vec::new();

        let mut remaining = 2;
        run_console_input(platform, &locked, &stop, Vec::new(), |_| {
            remaining -= 1;
            remaining == 0
        }, |history| saved = history.to_vec())
        .unwrap();

        // The second "reload" repeats the last entry, so it isn't recorded
        // again; `on_history_changed` is only called for the first one.
        assert_eq!(saved, vec!["reload".to_string()]);
    }

    #[test]
    fn history_round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!(
            "rev-idle-console-history-test-{}-{}",
            std::process::id(),
            line!(),
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("console_history.txt");

        assert!(load_history(&path).is_empty());

        let history = vec!["load a.js".to_string(), "reload".to_string()];
        save_history(&path, &history);

        assert_eq!(load_history(&path), history);

        fs::remove_dir_all(&dir).unwrap();
    }
}
