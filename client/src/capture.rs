use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    mpsc as std_mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::Win32::{
    Foundation::{HINSTANCE, LPARAM, LRESULT, POINT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, PeekMessageW, PostThreadMessageW,
        SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, MSLLHOOKSTRUCT,
        MSG, PM_NOREMOVE, WH_MOUSE_LL, WM_APP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_QUIT,
    },
};

use crate::{bridge::WsConnection, window};

static CAPTURE_ENABLED: AtomicBool = AtomicBool::new(false);
static CAPTURE_LEFT_BUTTON_DOWN: AtomicBool = AtomicBool::new(false);
static CAPTURE_THREAD_ID: AtomicU32 = AtomicU32::new(0);
static LOCK_ENABLED: AtomicBool = AtomicBool::new(false);

const CAPTURE_MESSAGE: u32 = WM_APP + 1;

/// Width/height (in client pixels) of the overlay's button row, bottom-right
/// anchored, that `ControlOverlay.cs` renders in the game window. Clicks
/// inside this rect are exempt from the lock-mode block below so the
/// Reload/Stop and Resume/Pause buttons stay usable while locked. Must be
/// kept in sync with the overlay's actual layout.
const OVERLAY_CONTROLS_WIDTH: i32 = 130;
const OVERLAY_CONTROLS_HEIGHT: i32 = 50;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CaptureState;

impl CaptureState {
    pub(crate) fn is_enabled(self) -> bool {
        CAPTURE_ENABLED.load(Ordering::Acquire)
    }

    pub(crate) fn set_enabled(self, enabled: bool) {
        CAPTURE_ENABLED.store(enabled, Ordering::Release);
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

fn capture_event(enabled: bool, message: WPARAM, point: POINT) -> Option<POINT> {
    (enabled && is_left_button_down(message)).then_some(point)
}

fn claim_capture(enabled: &AtomicBool, message: WPARAM, in_bounds: bool) -> bool {
    is_left_button_down(message) && in_bounds && enabled.swap(false, Ordering::AcqRel)
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

    if (is_left_button_down(message) || is_left_button_up(message)) && lock_enabled {
        match location {
            Some((x, y, width, height)) if !within_overlay_controls(x, y, width, height) => {
                return MouseAction::Consume;
            }
            _ => {}
        }
    }

    if claim_capture(&CAPTURE_ENABLED, message, location.is_some()) {
        CAPTURE_LEFT_BUTTON_DOWN.store(true, Ordering::Release);
        return MouseAction::ConsumeAndNotify(point);
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
        let point = hook_data.pt;
        let lock_enabled = LOCK_ENABLED.load(Ordering::Acquire);
        let needs_location = (is_left_button_down(message) || is_left_button_up(message))
            && (CAPTURE_ENABLED.load(Ordering::Acquire) || lock_enabled);
        let location = needs_location
            .then(|| window::screen_to_client_position(point.x, point.y).ok().flatten())
            .flatten();
        match decide_mouse_action(message, point, location, lock_enabled) {
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
            if target_type == "button" || target_type == "slot" => {
            if let Err(error) = write_clipboard(&path) {
                return vec![format!("click: {x}, {y}; {target_type}: {path:?}; clipboard write failed: {error}")];
            }
            vec![format!("click: {x}, {y}; {target_type}: {path:?}"), "Copied to clipboard".to_owned()]
        }
        Ok(_) => vec![format!("click: {x}, {y}")],
        Err(error) => vec![format!("click: {x}, {y}; lookup failed: {error}")],
    }
}

fn run_capture_loop(hook: HHOOK, connection: WsConnection, command_tx: tokio::sync::mpsc::Sender<crate::app::ScriptCommand>, runtime: tokio::runtime::Handle) -> Result<(), String> {
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
        if message.message == CAPTURE_MESSAGE {
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
}

impl CaptureWorker {
    pub(crate) fn start(connection: WsConnection, command_tx: tokio::sync::mpsc::Sender<crate::app::ScriptCommand>) -> Result<Self, String> {
        let (startup_tx, startup_rx) = std_mpsc::channel();
        let runtime = tokio::runtime::Handle::current();
        let handle = thread::spawn(move || {
            let thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
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
            CAPTURE_THREAD_ID.store(0, Ordering::Release);
            result
        });

        let (thread_id, registration) = startup_rx
            .recv()
            .map_err(|error| format!("mouse capture startup handshake failed: {error}"))?;
        if let Err(error) = registration {
            let _ = handle.join();
            return Err(error);
        }
        Ok(Self {
            thread_id,
            handle: Some(handle),
        })
    }

    pub(crate) fn shutdown(mut self) -> Result<(), String> {
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
        // 130x50 overlay button rect.
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
        assert!(within_overlay_controls(1790, 1030, 1920, 1080));
        assert!(!within_overlay_controls(1789, 1060, 1920, 1080));
        assert!(!within_overlay_controls(1850, 1029, 1920, 1080));
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
        let enabled = AtomicBool::new(true);
        assert!(claim_capture(&enabled, WPARAM(WM_LBUTTONDOWN as usize), true));
        assert!(!enabled.load(Ordering::Acquire));
        assert!(!claim_capture(&enabled, WPARAM(WM_LBUTTONDOWN as usize), true));
    }

    #[test]
    fn claim_capture_ignores_clicks_outside_the_game_window_and_stays_armed() {
        let enabled = AtomicBool::new(true);
        assert!(!claim_capture(&enabled, WPARAM(WM_LBUTTONDOWN as usize), false));
        assert!(enabled.load(Ordering::Acquire));
    }
}
