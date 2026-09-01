use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc as std_mpsc,
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tokio::sync::watch;
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::{Input::KeyboardAndMouse::{MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey, VK_F8}, WindowsAndMessaging::{
        GetMessageW, PeekMessageW, PostThreadMessageW, MSG, PM_NOREMOVE, WM_HOTKEY, WM_QUIT,
    }},
};

const HOTKEY_ID: i32 = 1;
const PAUSED_BIT: u64 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PauseUpdate {
    state: u64,
}

impl PauseUpdate {
    pub(crate) fn initial() -> Self {
        Self::default()
    }

    pub(crate) fn paused(self) -> bool {
        self.state & PAUSED_BIT != 0
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ActionGate {
    state: Arc<AtomicU64>,
}

impl ActionGate {
    pub(crate) fn is_paused(&self) -> bool {
        self.state.load(Ordering::Acquire) & PAUSED_BIT != 0
    }

    pub(crate) fn set_paused(&self, paused: bool) -> PauseUpdate {
        self.update(|_| paused)
    }

    fn toggle(&self) -> PauseUpdate {
        self.update(|paused| !paused)
    }

    pub(crate) fn is_current(&self, update: PauseUpdate) -> bool {
        self.state.load(Ordering::Acquire) == update.state
    }

    pub(crate) fn current_update(&self) -> PauseUpdate {
        PauseUpdate {
            state: self.state.load(Ordering::Acquire),
        }
    }

    pub(crate) fn reset_if_current(&self, update: PauseUpdate) -> bool {
        if !update.paused() {
            return self.is_current(update);
        }
        let next = update.state.wrapping_add(1) & !PAUSED_BIT;
        self.state
            .compare_exchange(
                update.state,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    fn update(&self, requested: impl Fn(bool) -> bool) -> PauseUpdate {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let paused = requested(current & PAUSED_BIT != 0);
            let next = (current.wrapping_add(2) & !PAUSED_BIT) | u64::from(paused);
            match self.state.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return PauseUpdate { state: next },
                Err(observed) => current = observed,
            }
        }
    }
}

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
    gate: ActionGate,
    pause_tx: watch::Sender<PauseUpdate>,
) -> Result<(), String> {
    platform.register()?;
    run_registered_hotkey_loop(platform, gate, pause_tx)
}

fn run_registered_hotkey_loop<P: HotkeyPlatform>(
    mut platform: P,
    gate: ActionGate,
    pause_tx: watch::Sender<PauseUpdate>,
) -> Result<(), String> {
    let mut result = Ok(());
    loop {
        match platform.next_event() {
            Ok(HotkeyEvent::Pressed) => {
                pause_tx.send_replace(gate.toggle());
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

fn post_quit(thread_id: u32) -> Result<(), String> {
    unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }
        .map_err(|error| format!("PostThreadMessageW failed: {error}"))
}

fn join_worker(handle: JoinHandle<Result<(), String>>) -> Result<(), String> {
    handle
        .join()
        .map_err(|_| "hotkey worker thread panicked".to_string())?
}

fn defer_worker_cleanup<F>(
    handle: JoinHandle<Result<(), String>>,
    mut retry_quit: F,
    retry_delay: Duration,
) -> JoinHandle<()>
where
    F: FnMut() -> Result<(), String> + Send + 'static,
{
    thread::spawn(move || {
        while !handle.is_finished() {
            let _ = retry_quit();
            thread::sleep(retry_delay);
        }
        let _ = handle.join();
    })
}

fn complete_worker_shutdown<F>(
    handle: JoinHandle<Result<(), String>>,
    initial_post: Result<(), String>,
    retry_quit: F,
    retry_delay: Duration,
) -> (Result<(), String>, Option<JoinHandle<()>>)
where
    F: FnMut() -> Result<(), String> + Send + 'static,
{
    match initial_post {
        Ok(()) => (join_worker(handle), None),
        Err(error) if handle.is_finished() => {
            let _ = join_worker(handle);
            (Err(error), None)
        }
        Err(error) => (
            Err(error),
            Some(defer_worker_cleanup(handle, retry_quit, retry_delay)),
        ),
    }
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
        gate: ActionGate,
        pause_tx: watch::Sender<PauseUpdate>,
    ) -> Result<Self, String> {
        let (startup_tx, startup_rx) = std_mpsc::channel();
        let handle = thread::spawn(move || {
            let thread_id = unsafe { GetCurrentThreadId() };
            let mut message = MSG::default();
            let _ = unsafe { PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE) };
            let mut platform = Win32HotkeyPlatform;
            let registration = platform.register();
            startup_tx.send((thread_id, registration.clone())).ok();
            registration?;
            run_registered_hotkey_loop(platform, gate, pause_tx)
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
        let post_result = post_quit(self.thread_id);
        let handle = self.handle.take().unwrap();
        let thread_id = self.thread_id;
        let (result, cleanup) = complete_worker_shutdown(
            handle,
            post_result,
            move || post_quit(thread_id),
            Duration::from_millis(10),
        );
        drop(cleanup);
        result
    }
}

impl Drop for HotkeyWorker {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let thread_id = self.thread_id;
            let _ = post_quit(thread_id);
            if handle.is_finished() {
                let _ = handle.join();
            } else {
                drop(defer_worker_cleanup(
                    handle,
                    move || post_quit(thread_id),
                    Duration::from_millis(10),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::Ordering};
    use tokio::sync::watch;

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
        let gate = ActionGate::default();
        let (pause_tx, mut pause_rx) = watch::channel(PauseUpdate::initial());
        let platform = FakeHotkeyPlatform::events([HotkeyEvent::Quit, HotkeyEvent::Pressed]);
        run_hotkey_loop(platform, gate.clone(), pause_tx).unwrap();
        assert!(gate.is_paused());
        assert!(pause_rx.borrow_and_update().paused());
    }

    #[test]
    fn unread_f8_events_coalesce_without_blocking_the_message_loop() {
        let gate = ActionGate::default();
        let (pause_tx, mut pause_rx) = watch::channel(PauseUpdate::initial());
        let mut events = vec![HotkeyEvent::Quit];
        events.extend(std::iter::repeat_n(HotkeyEvent::Pressed, 10_001));
        let platform = FakeHotkeyPlatform::events(events);

        run_hotkey_loop(platform, gate.clone(), pause_tx).unwrap();

        assert!(gate.is_paused());
        assert!(pause_rx.borrow_and_update().paused());
    }

    #[test]
    fn closed_pause_receiver_does_not_stop_hotkey_cleanup() {
        let gate = ActionGate::default();
        let (pause_tx, pause_rx) = watch::channel(PauseUpdate::initial());
        drop(pause_rx);
        let platform = FakeHotkeyPlatform::events([HotkeyEvent::Quit, HotkeyEvent::Pressed]);
        let unregister_count = platform.unregister_count.clone();

        assert_eq!(run_hotkey_loop(platform, gate.clone(), pause_tx), Ok(()));
        assert!(gate.is_paused());
        assert_eq!(unregister_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn registration_failure_returns_error() {
        let mut platform = FakeHotkeyPlatform::events([]);
        platform.fail_register = true;
        let (pause_tx, _) = watch::channel(PauseUpdate::initial());
        assert_eq!(run_hotkey_loop(platform, ActionGate::default(), pause_tx), Err("registration failed".to_string()));
    }

    #[test]
    fn successful_loop_unregisters_once() {
        let platform = FakeHotkeyPlatform::events([HotkeyEvent::Quit, HotkeyEvent::Continue]);
        let unregister_count = platform.unregister_count.clone();
        let (pause_tx, _) = watch::channel(PauseUpdate::initial());
        let result = run_hotkey_loop(platform, ActionGate::default(), pause_tx);
        assert_eq!(result, Ok(()));
        assert_eq!(unregister_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn failed_quit_post_transfers_running_worker_to_retrying_cleanup() {
        let (quit_tx, quit_rx) = std_mpsc::channel();
        let (joined_tx, joined_rx) = std_mpsc::channel();
        let worker = thread::spawn(move || {
            quit_rx.recv().unwrap();
            joined_tx.send(()).unwrap();
            Ok(())
        });
        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let retry_attempts = attempts.clone();

        let (result, cleanup) = complete_worker_shutdown(
            worker,
            Err("initial post failed".to_string()),
            move || {
                let attempt = retry_attempts.fetch_add(1, Ordering::Relaxed);
                if attempt == 0 {
                    Err("retry post failed".to_string())
                } else {
                    if attempt == 1 {
                        let _ = quit_tx.send(());
                    }
                    Ok(())
                }
            },
            std::time::Duration::ZERO,
        );

        assert_eq!(result, Err("initial post failed".to_string()));
        cleanup.unwrap().join().unwrap();
        joined_rx.recv().unwrap();
        assert!(attempts.load(Ordering::Relaxed) >= 2);
    }
}
