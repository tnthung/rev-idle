use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc as std_mpsc,
    Arc,
};
use std::thread::{self, JoinHandle};
use tokio::sync::mpsc;
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::{Input::KeyboardAndMouse::{MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey, VK_F8}, WindowsAndMessaging::{
        GetMessageW, PostThreadMessageW, MSG, WM_HOTKEY, WM_QUIT,
    }},
};

use crate::console::ScriptCommand;

const HOTKEY_ID: i32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HotkeyEvent {
    Pressed,
    Continue,
    Quit,
}

trait HotkeyPlatform {
    fn register(&mut self) -> Result<(), String>;
    fn next_event(&mut self) -> Result<HotkeyEvent, String>;
    fn unregister(&mut self) -> Result<(), String>;
}

#[cfg(test)]
fn run_hotkey_loop<P: HotkeyPlatform>(
    mut platform: P,
    gate: Arc<AtomicBool>,
    command_tx: mpsc::Sender<ScriptCommand>,
) -> Result<(), String> {
    platform.register()?;
    run_registered_hotkey_loop(platform, gate, command_tx)
}

fn run_registered_hotkey_loop<P: HotkeyPlatform>(
    mut platform: P,
    gate: Arc<AtomicBool>,
    command_tx: mpsc::Sender<ScriptCommand>,
) -> Result<(), String> {
    let mut result = Ok(());
    loop {
        match platform.next_event() {
            Ok(HotkeyEvent::Pressed) => {
                let paused = !gate.fetch_xor(true, Ordering::AcqRel);
                if let Err(error) = command_tx.blocking_send(ScriptCommand::SetPaused(paused)) {
                    result = Err(format!("failed to deliver hotkey command: {error}"));
                    break;
                }
            }
            Ok(HotkeyEvent::Continue) => {}
            Ok(HotkeyEvent::Quit) => break,
            Err(error) => {
                result = Err(error);
                break;
            }
        }
    }
    if let Err(error) = platform.unregister() {
        result = match result {
            Ok(()) => Err(format!("failed to unregister hotkey: {error}")),
            Err(original) => Err(format!("{original}; failed to unregister hotkey: {error}")),
        };
    }
    result
}

struct Win32HotkeyPlatform;

impl HotkeyPlatform for Win32HotkeyPlatform {
    fn register(&mut self) -> Result<(), String> {
        unsafe { RegisterHotKey(None, HOTKEY_ID, MOD_NOREPEAT, VK_F8.0 as u32) }
            .map_err(|error| format!("RegisterHotKey failed: {error}"))
    }

    fn next_event(&mut self) -> Result<HotkeyEvent, String> {
        let mut message = MSG::default();
        let value = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if value.0 == -1 {
            Err("GetMessageW failed".to_string())
        } else if value.as_bool() {
                if message.message == WM_HOTKEY && message.wParam == WPARAM(HOTKEY_ID as usize) {
                    Ok(HotkeyEvent::Pressed)
                } else {
                    Ok(HotkeyEvent::Continue)
                }
        } else {
            Ok(HotkeyEvent::Quit)
        }
    }

    fn unregister(&mut self) -> Result<(), String> {
        unsafe { UnregisterHotKey(None, HOTKEY_ID) }
            .map_err(|error| format!("UnregisterHotKey failed: {error}"))
    }
}

pub struct HotkeyWorker {
    thread_id: u32,
    handle: Option<JoinHandle<Result<(), String>>>,
}

impl HotkeyWorker {
    pub fn start(
        gate: Arc<AtomicBool>,
        command_tx: mpsc::Sender<ScriptCommand>,
    ) -> Result<Self, String> {
        let (startup_tx, startup_rx) = std_mpsc::channel();
        let handle = thread::spawn(move || {
            let thread_id = unsafe { GetCurrentThreadId() };
            let mut platform = Win32HotkeyPlatform;
            let registration = platform.register();
            startup_tx.send((thread_id, registration.clone())).ok();
            registration?;
            run_registered_hotkey_loop(platform, gate, command_tx)
        });
        let (thread_id, registration) = startup_rx
            .recv()
            .map_err(|error| format!("hotkey startup handshake failed: {error}"))?;
        if let Err(error) = registration {
            let _ = handle.join();
            return Err(error);
        }
        Ok(Self { thread_id, handle: Some(handle) })
    }

    pub fn shutdown(mut self) -> Result<(), String> {
        let post_result = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }
            .map_err(|error| format!("PostThreadMessageW failed: {error}"));
        let join_result = self.handle.take().unwrap().join()
            .map_err(|_| "hotkey worker thread panicked".to_string())?;
        post_result?;
        join_result
    }
}

impl Drop for HotkeyWorker {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
    use tokio::sync::mpsc;

    struct FakeHotkeyPlatform {
        events: Vec<HotkeyEvent>,
        registered: bool,
        unregister_count: Arc<std::sync::atomic::AtomicUsize>,
        fail_register: bool,
    }

    impl FakeHotkeyPlatform {
        fn events(events: impl IntoIterator<Item = HotkeyEvent>) -> Self {
            Self { events: events.into_iter().collect(), registered: false, unregister_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)), fail_register: false }
        }
    }

    impl HotkeyPlatform for FakeHotkeyPlatform {
        fn register(&mut self) -> Result<(), String> {
            if self.fail_register { return Err("registration failed".to_string()); }
            self.registered = true;
            Ok(())
        }
        fn next_event(&mut self) -> Result<HotkeyEvent, String> {
            self.events.pop().ok_or_else(|| "event source exhausted".to_string())
        }
        fn unregister(&mut self) -> Result<(), String> {
            self.unregister_count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    #[test]
    fn f8_closes_the_gate_before_delivering_paused_state() {
        let gate = Arc::new(AtomicBool::new(false));
        let (command_tx, mut command_rx) = mpsc::channel(4);
        let platform = FakeHotkeyPlatform::events([HotkeyEvent::Quit, HotkeyEvent::Pressed]);
        run_hotkey_loop(platform, gate.clone(), command_tx).unwrap();
        assert!(gate.load(Ordering::Acquire));
        assert_eq!(command_rx.blocking_recv(), Some(ScriptCommand::SetPaused(true)));
    }

    #[test]
    fn second_f8_opens_the_gate_and_delivers_resume() {
        let gate = Arc::new(AtomicBool::new(false));
        let (command_tx, mut command_rx) = mpsc::channel(4);
        let platform = FakeHotkeyPlatform::events([HotkeyEvent::Quit, HotkeyEvent::Pressed, HotkeyEvent::Pressed]);
        run_hotkey_loop(platform, gate.clone(), command_tx).unwrap();
        assert!(!gate.load(Ordering::Acquire));
        assert_eq!(command_rx.blocking_recv(), Some(ScriptCommand::SetPaused(true)));
        assert_eq!(command_rx.blocking_recv(), Some(ScriptCommand::SetPaused(false)));
    }

    #[test]
    fn registration_failure_returns_error() {
        let mut platform = FakeHotkeyPlatform::events([]);
        platform.fail_register = true;
        let (command_tx, _) = mpsc::channel(1);
        assert_eq!(run_hotkey_loop(platform, Arc::new(AtomicBool::new(false)), command_tx), Err("registration failed".to_string()));
    }

    #[test]
    fn successful_loop_unregisters_once() {
        let platform = FakeHotkeyPlatform::events([HotkeyEvent::Quit, HotkeyEvent::Continue]);
        let unregister_count = platform.unregister_count.clone();
        let (command_tx, _) = mpsc::channel(1);
        let result = run_hotkey_loop(platform, Arc::new(AtomicBool::new(false)), command_tx);
        assert_eq!(result, Ok(()));
        assert_eq!(unregister_count.load(Ordering::Relaxed), 1);
    }
}
