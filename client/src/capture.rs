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
        MSG, PM_NOREMOVE, WH_MOUSE_LL, WM_APP, WM_LBUTTONDOWN, WM_QUIT,
    },
};

use crate::window;

static CAPTURE_ENABLED: AtomicBool = AtomicBool::new(false);
static CAPTURE_THREAD_ID: AtomicU32 = AtomicU32::new(0);

const CAPTURE_MESSAGE: u32 = WM_APP + 1;

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

fn is_left_button_down(message: WPARAM) -> bool {
    message.0 as u32 == WM_LBUTTONDOWN
}

fn capture_event(enabled: bool, message: WPARAM, point: POINT) -> Option<POINT> {
    (enabled && is_left_button_down(message)).then_some(point)
}

unsafe extern "system" fn mouse_hook(
    code: i32,
    message: WPARAM,
    data: LPARAM,
) -> LRESULT {
    if code >= 0 && data.0 != 0 {
        let hook_data = unsafe { &*(data.0 as *const MSLLHOOKSTRUCT) };
        if let Some(point) = capture_event(CaptureState.is_enabled(), message, hook_data.pt) {
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
        }
    }

    unsafe { CallNextHookEx(None, code, message, data) }
}

fn post_quit(thread_id: u32) -> Result<(), String> {
    unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }
        .map_err(|error| format!("PostThreadMessageW failed: {error}"))
}

fn run_capture_loop(hook: HHOOK) -> Result<(), String> {
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
        if message.message == CAPTURE_MESSAGE
            && CaptureState.is_enabled()
        {
            let point = POINT {
                x: message.wParam.0 as u32 as i32,
                y: message.lParam.0 as i32,
            };
            if let Some((x, y)) = window::screen_to_client_position(point.x, point.y)
                .ok()
                .flatten()
            {
                println!("click: {x}, {y}");
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
    pub(crate) fn start() -> Result<Self, String> {
        let (startup_tx, startup_rx) = std_mpsc::channel();
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
            let result = run_capture_loop(hook);
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
}
