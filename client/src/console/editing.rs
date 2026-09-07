use std::{io::{self, Write}, sync::atomic::{AtomicBool, Ordering}};

use super::{history::push_history, input::{ConsoleEvent, ConsolePlatform}};

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
pub(super) fn run_console_input<P: ConsolePlatform>(
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
                    completions = super::completion::complete(&buffer);
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


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
}
