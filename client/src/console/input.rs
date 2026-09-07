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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ConsoleEvent {
    Char(char),
    Backspace,
    Enter,
    Up,
    Down,
    Tab,
    Ignored,
}

pub(super) trait ConsolePlatform {
    fn next_event(&mut self) -> Result<ConsoleEvent, String>;
}

pub(super) struct Win32ConsolePlatform {
    input: HANDLE,
    original_mode: CONSOLE_MODE,
}

impl Win32ConsolePlatform {
    /// Switches the input handle out of cooked mode (line buffering +
    /// automatic echo) so keystrokes reach us one at a time and are only
    /// echoed when we choose to. `Drop` restores whatever mode was active
    /// before.
    pub(super) fn new() -> Result<Self, String> {
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
