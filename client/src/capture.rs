use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    mpsc as std_mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::cell::RefCell;
use crate::bridge::{ScriptUiPointer, ScriptUiPointerPhase, ScriptUiSnapshot};
use tokio::sync::watch;
use uuid::Uuid;

use windows::Win32::{
    Foundation::{HINSTANCE, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Gdi::ClientToScreen,
    System::{
        LibraryLoader::GetModuleHandleW,
        Threading::GetCurrentThreadId,
    },
    UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE,
        MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE,
        MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT,
    },
    UI::WindowsAndMessaging::{
        CallNextHookEx, GetForegroundWindow, GetMessageW, GetSystemMetrics, PeekMessageW,
        PostThreadMessageW, SetForegroundWindow, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK,
        MSLLHOOKSTRUCT, MSG, PM_NOREMOVE, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
        SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, WH_MOUSE_LL, WM_APP, WM_LBUTTONDOWN,
        WM_LBUTTONUP, WM_QUIT,
    },
};

use crate::{bridge::WsConnection, window};

// The low bit is capture enabled; the remaining bits identify the last arm.
static CAPTURE_STATE: AtomicU64 = AtomicU64::new(0);
static CAPTURE_LEFT_BUTTON_DOWN: AtomicBool = AtomicBool::new(false);
static CAPTURE_THREAD_ID: AtomicU32 = AtomicU32::new(0);
static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);

const CAPTURE_MESSAGE: u32 = WM_APP + 1;
const FOCUS_MESSAGE: u32 = WM_APP + 2;
const SCRIPT_UI_MESSAGE: u32 = WM_APP + 3;
const FOCUS_CLICK_EXTRA_INFO: usize = 0x52455646;

/// Width/height (in client pixels) of the overlay's button row, bottom-right
/// anchored, that `ControlOverlay.cs` renders in the game window. Clicks
/// inside this rect are exempt from the lock-mode block below so the
/// script controls stay usable while locked. Must be
/// kept in sync with the overlay's actual layout.
const OVERLAY_CONTROLS_WIDTH: i32 = 168;
const OVERLAY_CONTROLS_HEIGHT: i32 = 46;

struct QueuedPointer {
    pointer: ScriptUiPointer,
    generation: u64,
    capture_epoch: u64,
}

#[derive(Default)]
struct PointerRelay {
    sequence: u64,
    pressed: Option<(u64, Option<(Uuid, u64)>)>,
}

impl PointerRelay {
    fn decide(
        &mut self,
        down: bool,
        up: bool,
        location: Option<(i32, i32, i32, i32)>,
        locked: bool,
        capturing: bool,
        route: Option<(Uuid, u64)>,
    ) -> (bool, Option<QueuedPointer>) {
        if capturing {
            if let Some((_, target)) = &mut self.pressed { *target = None; }
        }
        let (press_id, target, phase) = if up {
            match self.pressed.take() {
                Some((id, target)) => (id, target.filter(|target| Some(*target) == route), ScriptUiPointerPhase::Up),
                None => return (false, None),
            }
        } else if down && locked && !capturing && location.is_some_and(|(x, y, width, height)| !within_overlay_controls(x, y, width, height)) {
            self.sequence += 1;
            self.pressed = Some((self.sequence, route));
            (self.sequence, route, ScriptUiPointerPhase::Down)
        } else {
            return (false, None);
        };
        let Some((session_id, generation)) = target else { return (true, None); };
        let (x, y, width, height) = location.unwrap_or((0, 0, 0, 0));
        (true, Some(QueuedPointer { generation, capture_epoch: 0, pointer: ScriptUiPointer { session_id, press_id, phase, x, y, width, height } }))
    }
}

struct HookRelay {
    snapshots: watch::Receiver<ScriptUiSnapshot>,
    generations: watch::Receiver<u64>,
    pointer: PointerRelay,
    sender: tokio::sync::mpsc::Sender<QueuedPointer>,
    release: Option<tokio::sync::mpsc::OwnedPermit<QueuedPointer>>,
    capture_epoch: u64,
}

impl HookRelay {
    fn enqueue(&mut self, packet: QueuedPointer) -> bool {
        if packet.pointer.phase == ScriptUiPointerPhase::Down {
            self.release = None;
            // Reserve both phases before accepting the press. A full queue
            // can reject a whole click, but cannot discard its release later.
            let Ok(down) = self.sender.clone().try_reserve_owned() else { return false; };
            let Ok(up) = self.sender.clone().try_reserve_owned() else { return false; };
            self.release = Some(up);
            down.send(packet);
            true
        } else if let Some(up) = self.release.take() {
            up.send(packet);
            true
        } else {
            false
        }
    }
}

async fn relay_pointers(
    connection: WsConnection,
    snapshots: watch::Receiver<ScriptUiSnapshot>,
    mut pointers: tokio::sync::mpsc::Receiver<QueuedPointer>,
) {
    while let Some(queued) = pointers.recv().await {
        let session_id = queued.pointer.session_id;
        let result = connection.send_for_generation(queued.pointer, queued.generation, || {
            CaptureState.inactive_epoch() == Some(queued.capture_epoch)
                && !CAPTURE_LEFT_BUTTON_DOWN.load(Ordering::Acquire)
                && snapshots.borrow().session_id == Some(session_id)
        }).await;
        if let Err(error) = result {
            eprintln!("[Script UI] pointer relay canceled: {error}");
        }
    }
}

thread_local! {
    // The hook and message pump run on the same capture thread. No network
    // work or async waits take place in the hook.
    static SCRIPT_UI_RELAY: RefCell<Option<HookRelay>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CaptureState;

impl CaptureState {
    pub(crate) fn epoch(self) -> u64 {
        CAPTURE_STATE.load(Ordering::Acquire) >> 1
    }

    pub(crate) fn inactive_epoch(self) -> Option<u64> {
        let state = CAPTURE_STATE.load(Ordering::Acquire);
        (state & 1 == 0).then_some(state >> 1)
    }

    pub(crate) fn is_enabled(self) -> bool {
        CAPTURE_STATE.load(Ordering::Acquire) & 1 != 0
    }

    pub(crate) fn set_enabled(self, enabled: bool) {
        if enabled {
            let _ = CAPTURE_STATE.fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| Some((state.wrapping_add(2) & !1) | 1));
        } else {
            CAPTURE_STATE.fetch_and(!1, Ordering::AcqRel);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LockState {
    pub(crate) enabled: &'static AtomicBool,
}

impl Default for LockState {
    fn default() -> Self {
        Self { enabled: &LOCK_ENABLED }
    }
}

impl LockState {
    pub(crate) fn is_enabled(self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    pub(crate) fn set_enabled(self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }
}

fn is_left_button_down(message: WPARAM) -> bool {
    message.0 as u32 == WM_LBUTTONDOWN
}

fn is_left_button_up(message: WPARAM) -> bool {
    message.0 as u32 == WM_LBUTTONUP
}

#[cfg(test)]
fn capture_event(enabled: bool, message: WPARAM, point: POINT) -> Option<POINT> {
    (enabled && is_left_button_down(message)).then_some(point)
}

fn claim_capture(state: &AtomicU64, message: WPARAM, in_bounds: bool) -> bool {
    is_left_button_down(message) && in_bounds && state.fetch_and(!1, Ordering::AcqRel) & 1 != 0
}

/// Whether (x, y) within a client area of (width, height) falls inside the
/// overlay's own button row, which lock mode always lets through.
fn within_overlay_controls(x: i32, y: i32, width: i32, height: i32) -> bool {
    x >= width - OVERLAY_CONTROLS_WIDTH && y >= height - OVERLAY_CONTROLS_HEIGHT
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum MouseAction {
    PassThrough,
    Consume,
    ConsumeAndNotify(POINT),
}

/// `location`, when present, is the click's (x, y, width, height) in the
/// game's client area, resolved by the caller only when a lookup is
/// actually warranted (see `mouse_hook`) so this stays cheap to call for
/// every mouse message. `lock_enabled` is likewise resolved by the caller
/// (rather than read from `LOCK_ENABLED` here) so unit tests never have to
/// touch the real global, which is also written by the lifecycle command
/// loop and would otherwise be a cross-test race.
fn decide_mouse_action(
    message: WPARAM,
    point: POINT,
    location: Option<(i32, i32, i32, i32)>,
    lock_enabled: bool,
) -> MouseAction {
    if is_left_button_up(message) && CAPTURE_LEFT_BUTTON_DOWN.swap(false, Ordering::AcqRel) {
        return MouseAction::Consume;
    }

    if claim_capture(&CAPTURE_STATE, message, location.is_some()) {
        CAPTURE_LEFT_BUTTON_DOWN.store(true, Ordering::Release);
        return MouseAction::ConsumeAndNotify(point);
    }

    if (is_left_button_down(message) || is_left_button_up(message)) && lock_enabled {
        match location {
            Some((x, y, width, height)) if !within_overlay_controls(x, y, width, height) => {
                return MouseAction::Consume;
            }
            _ => {}
        }
    }

    MouseAction::PassThrough
}

unsafe extern "system" fn mouse_hook(
    code: i32,
    message: WPARAM,
    data: LPARAM,
) -> LRESULT {
    if code >= 0 && data.0 != 0 {
        let hook_data = unsafe { &*(data.0 as *const MSLLHOOKSTRUCT) };
        if hook_data.dwExtraInfo == FOCUS_CLICK_EXTRA_INFO {
            return unsafe { CallNextHookEx(None, code, message, data) };
        }
        let point = hook_data.pt;
        let lock_enabled = LOCK_ENABLED.load(Ordering::Acquire);
        let needs_location = is_left_button_down(message) || is_left_button_up(message);
        let location = needs_location
            .then(|| window::screen_to_client_position(point.x, point.y).ok().flatten())
            .flatten();
        let consumed = SCRIPT_UI_RELAY.with(|relay| {
            let mut relay = relay.borrow_mut();
            let Some(relay) = relay.as_mut() else { return false; };
            let capture_epoch = CaptureState.epoch();
            if relay.capture_epoch != capture_epoch {
                relay.capture_epoch = capture_epoch;
                relay.release = None;
                if let Some((_, target)) = &mut relay.pointer.pressed { *target = None; }
            }
            let generation = *relay.generations.borrow();
            let route = relay.snapshots.borrow().session_id.filter(|_| generation != 0).map(|id| (id, generation));
            let (consumed, packet) = relay.pointer.decide(
                is_left_button_down(message), is_left_button_up(message), location, lock_enabled,
                CaptureState.is_enabled() || CAPTURE_LEFT_BUTTON_DOWN.load(Ordering::Acquire), route,
            );
            if let Some(mut packet) = packet {
                packet.capture_epoch = capture_epoch;
                let down = packet.pointer.phase == ScriptUiPointerPhase::Down;
                if relay.enqueue(packet) {
                    if down {
                        let _ = unsafe { PostThreadMessageW(CAPTURE_THREAD_ID.load(Ordering::Acquire), SCRIPT_UI_MESSAGE, WPARAM(0), LPARAM(0)) };
                    }
                } else {
                    let _ = unsafe { PostThreadMessageW(CAPTURE_THREAD_ID.load(Ordering::Acquire), SCRIPT_UI_MESSAGE, WPARAM(1), LPARAM(0)) };
                    if let Some((_, target)) = &mut relay.pointer.pressed { *target = None; }
                }
            }
            if is_left_button_up(message) { relay.release = None; }
            consumed
        });
        if consumed { return LRESULT(1); }
        let action = decide_mouse_action(message, point, location, lock_enabled);
        if is_left_button_down(message)
            && location.is_some()
            && action != MouseAction::PassThrough
        {
            let thread_id = CAPTURE_THREAD_ID.load(Ordering::Acquire);
            if thread_id != 0 {
                let _ = unsafe {
                    PostThreadMessageW(
                        thread_id,
                        FOCUS_MESSAGE,
                        WPARAM(point.x as u32 as usize),
                        LPARAM(point.y as isize),
                    )
                };
            }
        }
        match action {
            MouseAction::PassThrough => {}
            MouseAction::Consume => return LRESULT(1),
            MouseAction::ConsumeAndNotify(point) => {
                let thread_id = CAPTURE_THREAD_ID.load(Ordering::Acquire);
                if thread_id != 0 {
                    let _ = unsafe {
                        PostThreadMessageW(
                            thread_id,
                            CAPTURE_MESSAGE,
                            WPARAM(point.x as u32 as usize),
                            LPARAM(point.y as isize),
                        )
                    };
                }
                return LRESULT(1);
            }
        }
    }

    unsafe { CallNextHookEx(None, code, message, data) }
}

fn post_quit(thread_id: u32) -> Result<(), String> {
    unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }
        .map_err(|error| format!("PostThreadMessageW failed: {error}"))
}

async fn describe_capture(connection: &WsConnection, x: i32, y: i32, width: i32, height: i32, write_clipboard: impl FnOnce(&str) -> Result<(), String>) -> Vec<String> {
    use crate::bridge::UiPathTarget;

    match crate::bridge::request_ui_path(connection, x, y, width, height).await {
        Ok(UiPathTarget { target_type: Some(target_type), path: Some(path) })
            if target_type == "button" || target_type == "checkbox" || target_type == "input" || target_type == "slot" => {
            if let Err(error) = write_clipboard(&path) {
                return vec![format!("click: {x}, {y}; {target_type}: {path:?}; clipboard write failed: {error}")];
            }
            vec![format!("click: {x}, {y}; {target_type}: {path:?}"), "Copied to clipboard".to_owned()]
        }
        Ok(_) => vec![format!("click: {x}, {y}")],
        Err(error) => vec![format!("click: {x}, {y}; lookup failed: {error}")],
    }
}

fn run_capture_loop<S: crate::app::ScriptCommandSink>(hook: HHOOK, connection: WsConnection, command_tx: S, runtime: tokio::runtime::Handle) -> Result<(), String> {
    let mut result = Ok(());
    loop {
        let mut message = MSG::default();
        let value = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if value.0 == -1 {
            result = Err("GetMessageW failed".to_string());
            break;
        }
        if !value.as_bool() {
            break;
        }
        if message.message == SCRIPT_UI_MESSAGE {
            if message.wParam.0 == 1 {
                eprintln!("[Script UI] pointer queue full; canceled press");
            } else if !CaptureState.is_enabled() && let Ok(window) = window::find_game_window() {
                let _ = unsafe { SetForegroundWindow(window) };
            }
        } else if message.message == FOCUS_MESSAGE
            && let Ok(window) = window::find_game_window()
        {
            let foreground = unsafe { GetForegroundWindow() };
            if foreground != window {
                let mut focus = POINT { x: 1, y: 1 };
                let virtual_left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
                let virtual_top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
                let virtual_width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
                let virtual_height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
                if virtual_width > 1
                    && virtual_height > 1
                    && unsafe { ClientToScreen(window, &mut focus).as_bool() }
                {
                    let original = POINT {
                        x: message.wParam.0 as u32 as i32,
                        y: message.lParam.0 as i32,
                    };
                    let normalize_x = |x: i32| {
                        (((i64::from(x) - i64::from(virtual_left)) * 65535
                            / i64::from(virtual_width - 1))
                        .clamp(0, 65535)) as i32
                    };
                    let normalize_y = |y: i32| {
                        (((i64::from(y) - i64::from(virtual_top)) * 65535
                            / i64::from(virtual_height - 1))
                        .clamp(0, 65535)) as i32
                    };
                    let position_flags =
                        MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_MOVE | MOUSEEVENTF_VIRTUALDESK;
                    let mouse_input = |point: POINT, flags| INPUT {
                        r#type: INPUT_MOUSE,
                        Anonymous: INPUT_0 {
                            mi: MOUSEINPUT {
                                dx: normalize_x(point.x),
                                dy: normalize_y(point.y),
                                dwFlags: position_flags | flags,
                                dwExtraInfo: FOCUS_CLICK_EXTRA_INFO,
                                ..Default::default()
                            },
                        },
                    };
                    let inputs = [
                        mouse_input(focus, Default::default()),
                        mouse_input(focus, MOUSEEVENTF_LEFTDOWN),
                        mouse_input(focus, MOUSEEVENTF_LEFTUP),
                        mouse_input(original, Default::default()),
                    ];
                    let _ = unsafe {
                        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32)
                    };
                }
            }
        } else if message.message == CAPTURE_MESSAGE {
            let _ = command_tx.blocking_send(crate::app::ScriptCommand::CaptureConsumed);
            let point = POINT {
                x: message.wParam.0 as u32 as i32,
                y: message.lParam.0 as i32,
            };
            if let Some((x, y, width, height)) = window::screen_to_client_position(point.x, point.y)
                .ok()
                .flatten()
            {
                let connection = connection.clone();
                runtime.spawn(async move {
                    for line in describe_capture(&connection, x, y, width, height, |path| {
                        window::WindowControl::write_clipboard(&window::Win32WindowControl, path)
                    }).await { println!("{line}"); }
                });
            }
        }
    }

    if let Err(error) = unsafe { UnhookWindowsHookEx(hook) } {
        result = match result {
            Ok(()) => Err(format!("UnhookWindowsHookEx failed: {error}")),
            Err(original) => Err(format!("{original}; UnhookWindowsHookEx failed: {error}")),
        };
    }
    result
}

fn defer_worker_cleanup(handle: JoinHandle<Result<(), String>>, thread_id: u32) {
    thread::spawn(move || {
        while !handle.is_finished() {
            let _ = post_quit(thread_id);
            thread::sleep(Duration::from_millis(10));
        }
        let _ = handle.join();
    });
}

pub(crate) struct CaptureWorker {
    thread_id: u32,
    handle: Option<JoinHandle<Result<(), String>>>,
    relay: tokio::task::JoinHandle<()>,
}

impl CaptureWorker {
    pub(crate) fn start<S: crate::app::ScriptCommandSink>(connection: WsConnection, command_tx: S, snapshots: watch::Receiver<ScriptUiSnapshot>) -> Result<Self, String> {
        let (startup_tx, startup_rx) = std_mpsc::channel();
        let runtime = tokio::runtime::Handle::current();
        let (pointer_tx, pointer_rx) = tokio::sync::mpsc::channel(256);
        let relay = runtime.spawn(relay_pointers(connection.clone(), snapshots.clone(), pointer_rx));
        let handle = thread::spawn(move || {
            SCRIPT_UI_RELAY.with(|relay| *relay.borrow_mut() = Some(HookRelay {
                snapshots, generations: connection.connection_generation(), pointer: PointerRelay::default(), sender: pointer_tx, release: None,
                capture_epoch: CaptureState.epoch(),
            }));
            let thread_id = unsafe { GetCurrentThreadId() };
            let mut message = MSG::default();
            let _ = unsafe { PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE) };
            CAPTURE_THREAD_ID.store(thread_id, Ordering::Release);
            let module = unsafe { GetModuleHandleW(None) }
                .map(|module| HINSTANCE(module.0))
                .map_err(|error| format!("GetModuleHandleW failed: {error}"));
            let hook = module.and_then(|module| unsafe {
                SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), Some(module), 0)
            }.map_err(|error| {
                format!("SetWindowsHookExW(WH_MOUSE_LL) failed: {error}")
            }));
            let registration = hook;
            let startup = registration.as_ref().map(|_| ()).map_err(Clone::clone);
            startup_tx.send((thread_id, startup)).ok();
            let hook = match registration {
                Ok(hook) => hook,
                Err(error) => {
                    CAPTURE_THREAD_ID.store(0, Ordering::Release);
                    return Err(error);
                }
            };
            let result = run_capture_loop(hook, connection, command_tx, runtime);
            SCRIPT_UI_RELAY.with(|relay| *relay.borrow_mut() = None);
            CAPTURE_THREAD_ID.store(0, Ordering::Release);
            result
        });

        let (thread_id, registration) = startup_rx
            .recv()
            .map_err(|error| format!("mouse capture startup handshake failed: {error}"))?;
        if let Err(error) = registration {
            relay.abort();
            let _ = handle.join();
            return Err(error);
        }
        Ok(Self {
            thread_id,
            handle: Some(handle),
            relay,
        })
    }

    pub(crate) fn shutdown(mut self) -> Result<(), String> {
        self.relay.abort();
        CaptureState.set_enabled(false);
        CAPTURE_THREAD_ID.store(0, Ordering::Release);
        let post_result = post_quit(self.thread_id);
        let handle = self.handle.take().unwrap();
        match post_result {
            Ok(()) => handle
                .join()
                .map_err(|_| "mouse capture worker thread panicked".to_string())?
                .into(),
            Err(error) if handle.is_finished() => {
                let _ = handle.join();
                Err(error)
            }
            Err(error) => {
                defer_worker_cleanup(handle, self.thread_id);
                Err(error)
            }
        }
    }
}

impl Drop for CaptureWorker {
    fn drop(&mut self) {
        self.relay.abort();
        CaptureState.set_enabled(false);
        CAPTURE_THREAD_ID.store(0, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = post_quit(self.thread_id);
            if handle.is_finished() {
                let _ = handle.join();
            } else {
                defer_worker_cleanup(handle, self.thread_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IN_GAME: Option<(i32, i32, i32, i32)> = Some((10, 20, 1920, 1080));

    #[test]
    fn pointer_queue_reserves_a_release_before_accepting_a_press() {
        for capacity in [1, 2] {
            let (sender, mut received) = tokio::sync::mpsc::channel(capacity);
            let (_, generations) = watch::channel(1);
            let mut relay = HookRelay {
                snapshots: crate::bridge::ScriptUiPublisher::default().subscribe(), generations,
                pointer: PointerRelay::default(), sender, release: None, capture_epoch: 0,
            };
            let route = Some((Uuid::new_v4(), 1));
            let down = relay.pointer.decide(true, false, IN_GAME, true, false, route).1.unwrap();
            let press_id = down.pointer.press_id;
            assert_eq!(relay.enqueue(down), capacity == 2);
            if capacity == 1 {
                assert!(received.try_recv().is_err());
                assert_eq!(relay.sender.capacity(), 1);
            } else {
                assert_eq!(relay.sender.capacity(), 0);
                let up = relay.pointer.decide(false, true, IN_GAME, false, false, route).1.unwrap();
                assert!(relay.enqueue(up));
                for phase in [ScriptUiPointerPhase::Down, ScriptUiPointerPhase::Up] {
                    let packet = received.try_recv().unwrap().pointer;
                    assert_eq!(packet.phase, phase);
                    assert_eq!(packet.press_id, press_id);
                }
            }
        }
    }

    #[test]
    fn locked_custom_pointer_is_consumed_and_relayed() {
        let mut relay = PointerRelay::default();
        let route = Some((uuid::Uuid::new_v4(), 1));
        let (consumed, down) = relay.decide(true, false, IN_GAME, true, false, route);
        assert!(consumed);
        let down = down.unwrap();
        assert_eq!(down.pointer.phase, crate::bridge::ScriptUiPointerPhase::Down);
        let (consumed, up) = relay.decide(false, true, IN_GAME, true, false, route);
        assert!(consumed);
        assert_eq!(up.unwrap().pointer.press_id, down.pointer.press_id);
        assert!(!relay.decide(true, false, IN_GAME, false, false, route).0);
        assert!(!relay.decide(true, false, Some((1850, 1060, 1920, 1080)), true, false, route).0);
    }

    #[test]
    fn locked_pointer_release_keeps_press_ownership() {
        let mut relay = PointerRelay::default();
        let route = Some((uuid::Uuid::new_v4(), 1));
        relay.decide(true, false, IN_GAME, true, false, route);
        let (consumed, up) = relay.decide(false, true, None, false, false, route);
        assert!(consumed);
        assert_eq!(up.unwrap().pointer.width, 0);
        relay.decide(true, false, IN_GAME, true, false, None);
        assert!(relay.decide(false, true, IN_GAME, false, false, None).0);
        relay.decide(true, false, IN_GAME, true, false, route);
        let (consumed, up) = relay.decide(false, true, IN_GAME, false, false, Some((route.unwrap().0, 2)));
        assert!(consumed && up.is_none());
    }

    #[test]
    fn capture_takes_priority_over_script_ui() {
        let mut relay = PointerRelay::default();
        let route = Some((uuid::Uuid::new_v4(), 1));
        assert!(!relay.decide(true, false, IN_GAME, true, true, route).0);
        relay.decide(true, false, IN_GAME, true, false, route);
        let (consumed, up) = relay.decide(false, true, IN_GAME, true, true, route);
        assert!(consumed && up.is_none());
    }

    #[test]
    fn captured_click_is_consumed_including_release_after_capture_stops() {
        let point = POINT { x: 10, y: 20 };
        CaptureState.set_enabled(true);
        let press = decide_mouse_action(WPARAM(WM_LBUTTONDOWN as usize), point, IN_GAME, false);
        CaptureState.set_enabled(false);
        let release = decide_mouse_action(WPARAM(WM_LBUTTONUP as usize), point, IN_GAME, false);
        assert_eq!(press, MouseAction::ConsumeAndNotify(point));
        assert_eq!(release, MouseAction::Consume);
    }

    #[test]
    fn click_outside_the_game_window_passes_through_and_leaves_capture_armed() {
        let point = POINT { x: 10, y: 20 };
        CaptureState.set_enabled(true);
        let action = decide_mouse_action(WPARAM(WM_LBUTTONDOWN as usize), point, None, false);
        assert_eq!(action, MouseAction::PassThrough);
        assert!(CaptureState.is_enabled());
        CaptureState.set_enabled(false);
    }

    // These lock-mode tests pass `lock_enabled` directly rather than going
    // through `LockState`/`LOCK_ENABLED` (the real global also written by
    // the lifecycle command loop) so they can't race against tests that
    // exercise the real lock command end-to-end.

    #[test]
    fn locked_click_outside_overlay_controls_is_consumed_and_stays_locked() {
        let point = POINT { x: 10, y: 20 };
        let down = decide_mouse_action(WPARAM(WM_LBUTTONDOWN as usize), point, Some((100, 100, 1920, 1080)), true);
        let up = decide_mouse_action(WPARAM(WM_LBUTTONUP as usize), point, Some((100, 100, 1920, 1080)), true);
        assert_eq!(down, MouseAction::Consume);
        assert_eq!(up, MouseAction::Consume);
    }

    #[test]
    fn locked_click_inside_overlay_controls_passes_through() {
        let point = POINT { x: 1850, y: 1060 };
        // 1920x1080 client area; (1850, 1060) falls inside the bottom-right
        // 168x46 overlay button rect.
        let down = decide_mouse_action(WPARAM(WM_LBUTTONDOWN as usize), point, Some((1850, 1060, 1920, 1080)), true);
        assert_eq!(down, MouseAction::PassThrough);
    }

    #[test]
    fn locked_click_outside_the_game_window_entirely_passes_through() {
        let point = POINT { x: 10, y: 20 };
        let action = decide_mouse_action(WPARAM(WM_LBUTTONDOWN as usize), point, None, true);
        assert_eq!(action, MouseAction::PassThrough);
    }

    #[test]
    fn overlay_controls_rect_matches_the_bottom_right_corner() {
        assert!(within_overlay_controls(1850, 1060, 1920, 1080));
        assert!(within_overlay_controls(1752, 1034, 1920, 1080));
        assert!(!within_overlay_controls(1751, 1060, 1920, 1080));
        assert!(!within_overlay_controls(1850, 1033, 1920, 1080));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn describe_capture() {
        use crate::bridge::{test_support::raw_server, WsConnection};
        use futures_util::{SinkExt, StreamExt};
        use serde_json::{json, Value};
        use std::{cell::RefCell, rc::Rc};
        use tokio_tungstenite::tungstenite::Message;

        for (response_type, payload, expected, copied_path) in [
            ("UiPathRes", json!({"type":"button","path":"scene:1/Canvas[0]/Buy DTP"}), vec!["click: 123, 456; button: \"scene:1/Canvas[0]/Buy DTP\"".to_owned(), "Copied to clipboard".to_owned()], Some("scene:1/Canvas[0]/Buy DTP")),
            ("UiPathRes", json!({"type":"slot","path":"scene:1/Canvas[0]/Slot"}), vec!["click: 123, 456; slot: \"scene:1/Canvas[0]/Slot\"".to_owned(), "Copied to clipboard".to_owned()], Some("scene:1/Canvas[0]/Slot")),
            ("UiPathRes", json!({"type":"checkbox","path":"scene:1/Canvas[0]/Checkbox"}), vec!["click: 123, 456; checkbox: \"scene:1/Canvas[0]/Checkbox\"".to_owned(), "Copied to clipboard".to_owned()], Some("scene:1/Canvas[0]/Checkbox")),
            ("UiPathRes", json!({"type":"input","path":"scene:1/Canvas[0]/Input"}), vec!["click: 123, 456; input: \"scene:1/Canvas[0]/Input\"".to_owned(), "Copied to clipboard".to_owned()], Some("scene:1/Canvas[0]/Input")),
            ("UiPathRes", json!({"type":null,"path":null}), vec!["click: 123, 456".to_owned()], None),
            ("RemoteError", json!({"message":"no EventSystem"}), vec!["click: 123, 456; lookup failed: Remote(\"no EventSystem\")".to_owned()], None),
        ] {
            let (address, peer_rx) = raw_server().await;
            let connection = WsConnection::connect_for_test(
                address,
                Duration::from_millis(20),
                Duration::from_secs(1),
            );
            let mut peer = tokio::time::timeout(Duration::from_secs(1), peer_rx)
                .await
                .unwrap()
                .unwrap();
            let copied = Rc::new(RefCell::new(None));
            let copied_for_callback = copied.clone();
            let description = super::describe_capture(&connection, 123, 456, 1920, 1080, |path| {
                *copied_for_callback.borrow_mut() = Some(path.to_owned());
                Ok(())
            });
            tokio::pin!(description);
            let message = tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    tokio::select! {
                        result = &mut description => panic!("capture completed before bridge response: {result:?}"),
                        message = peer.next() => break message.unwrap().unwrap(),
                    }
                }
            })
            .await
            .unwrap();
            let request: Value = serde_json::from_str(message.into_text().unwrap().as_ref()).unwrap();
            assert_eq!(request.get("type"), Some(&json!("UiPathReq")));
            assert_eq!(request.get("payload"), Some(&json!({ "x": 123, "y": 456, "width": 1920, "height": 1080 })));
            peer.send(Message::Text(
                json!({
                    "uuid": request["uuid"],
                    "type": response_type,
                    "payload": payload,
                })
                .to_string()
                .into(),
            ))
            .await
            .unwrap();
            let description_result = description.as_mut().await;
            drop(description);
            assert_eq!(description_result, expected);
            assert_eq!(copied.borrow().as_deref(), copied_path);
            connection.shutdown().await;
        }
    }

    #[test]
    fn only_left_button_down_is_a_capture_event() {
        assert!(is_left_button_down(WPARAM(WM_LBUTTONDOWN as usize)));
        assert!(!is_left_button_down(WPARAM(514)));
    }

    #[test]
    fn capture_event_requires_enabled_left_button_down() {
        let point = POINT { x: -12, y: 34 };
        assert_eq!(capture_event(true, WPARAM(WM_LBUTTONDOWN as usize), point), Some(point));
        assert_eq!(capture_event(false, WPARAM(WM_LBUTTONDOWN as usize), point), None);
        assert_eq!(capture_event(true, WPARAM(514), point), None);
    }

    #[test]
    fn capture_claim_disarms_before_coordinate_lookup() {
        let enabled = AtomicU64::new(3);
        assert!(claim_capture(&enabled, WPARAM(WM_LBUTTONDOWN as usize), true));
        assert_eq!(enabled.load(Ordering::Acquire), 2);
        assert!(!claim_capture(&enabled, WPARAM(WM_LBUTTONDOWN as usize), true));
    }

    #[test]
    fn claim_capture_ignores_clicks_outside_the_game_window_and_stays_armed() {
        let enabled = AtomicU64::new(3);
        assert!(!claim_capture(&enabled, WPARAM(WM_LBUTTONDOWN as usize), false));
        assert_eq!(enabled.load(Ordering::Acquire), 3);
    }
}
