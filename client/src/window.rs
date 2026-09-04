use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use windows::{
    core::{BOOL, Error as WinError, PWSTR},
    Win32::{
        Foundation::{CloseHandle, GlobalFree, HANDLE, HGLOBAL, HWND, LPARAM, POINT, RECT, WPARAM},
        Graphics::Gdi::ClientToScreen,
        System::{
            Console::GetConsoleWindow,
            DataExchange::{
                CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
            },
            Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
            Ole::CF_UNICODETEXT,
            Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetClientRect, GetWindowRect, GetWindowThreadProcessId,
            GetClassNameW, IsWindowVisible, PostMessageW, SetWindowPos, ShowWindow,
            SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOZORDER, SW_RESTORE,
        },
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

const GAME_EXECUTABLE: &str = "Revolution Idle.exe";
const CONSOLE_WINDOW_CLASS: &str = "ConsoleWindowClass";
const INPUT_BRIDGE_MESSAGE: u32 = 0x8417;
const SCROLL_BRIDGE_MESSAGE: u32 = 0x8418;

fn free_global(memory: HGLOBAL) {
    unsafe {
        let _ = GlobalFree(Some(memory));
    }
}

fn unicode_clipboard_contents(text: &str) -> Result<Vec<u16>, String> {
    if text.contains('\0') {
        return Err("clipboard text cannot contain a null character".to_string());
    }

    let text = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n");
    Ok(text.encode_utf16().chain(std::iter::once(0)).collect())
}

const CLIPBOARD_OPEN_ATTEMPTS: u32 = 10;
const CLIPBOARD_OPEN_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(20);

fn open_clipboard_with_retry(owner: HWND) -> Result<(), String> {
    let mut last_error = None;
    for attempt in 0..CLIPBOARD_OPEN_ATTEMPTS {
        match unsafe { OpenClipboard(Some(owner)) } {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                if attempt + 1 < CLIPBOARD_OPEN_ATTEMPTS {
                    std::thread::sleep(CLIPBOARD_OPEN_RETRY_DELAY);
                }
            }
        }
    }

    Err(format!(
        "OpenClipboard failed: {}",
        last_error.expect("loop always sets last_error before exhausting attempts")
    ))
}

fn write_unicode_clipboard(text: &str) -> Result<(), String> {
    let owner = unsafe { GetConsoleWindow() };
    if owner.is_invalid() {
        return Err("GetConsoleWindow returned no console window".to_string());
    }

    let contents = unicode_clipboard_contents(text)?;
    let byte_count = contents
        .len()
        .checked_mul(std::mem::size_of::<u16>())
        .ok_or_else(|| "clipboard text is too large".to_string())?;
    let memory = unsafe { GlobalAlloc(GMEM_MOVEABLE, byte_count) }
        .map_err(|error| format!("GlobalAlloc failed: {error}"))?;
    let destination = unsafe { GlobalLock(memory) };
    if destination.is_null() {
        let error = WinError::from_win32();
        free_global(memory);
        return Err(format!("GlobalLock failed: {error}"));
    }
    unsafe {
        std::ptr::copy_nonoverlapping(contents.as_ptr(), destination.cast::<u16>(), contents.len());
        let _ = GlobalUnlock(memory);
    }

    if let Err(error) = open_clipboard_with_retry(owner) {
        free_global(memory);
        return Err(error);
    }

    let result = unsafe { EmptyClipboard() }
        .map_err(|error| format!("EmptyClipboard failed: {error}"))
        .and_then(|()| {
            unsafe {
                SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(memory.0)))
            }
            .map(|_| ())
            .map_err(|error| format!("SetClipboardData failed: {error}"))
        });
    let close_result = unsafe { CloseClipboard() }
        .map_err(|error| format!("CloseClipboard failed: {error}"));

    if result.is_err() {
        free_global(memory);
    }
    result?;
    close_result
}

fn read_unicode_clipboard() -> Result<String, String> {
    let owner = unsafe { GetConsoleWindow() };
    if owner.is_invalid() {
        return Err("GetConsoleWindow returned no console window".to_string());
    }

    open_clipboard_with_retry(owner)?;

    let result = (|| {
        let handle = unsafe { GetClipboardData(CF_UNICODETEXT.0 as u32) }
            .map_err(|error| format!("GetClipboardData failed: {error}"))?;
        let memory = HGLOBAL(handle.0);
        let source = unsafe { GlobalLock(memory) };
        if source.is_null() {
            let error = WinError::from_win32();
            return Err(format!("GlobalLock failed: {error}"));
        }

        let text = unsafe {
            let mut length = 0usize;
            while *source.cast::<u16>().add(length) != 0 {
                length += 1;
            }
            let slice = std::slice::from_raw_parts(source.cast::<u16>(), length);
            String::from_utf16_lossy(slice)
        };

        let _ = unsafe { GlobalUnlock(memory) };
        Ok(text)
    })();

    let close_result = unsafe { CloseClipboard() }
        .map_err(|error| format!("CloseClipboard failed: {error}"));

    let text = result?;
    close_result?;
    Ok(text)
}

fn pack_bridge_coordinates(x: i32, y: i32) -> isize {
    (((y as u32 as u64) << 32) | x as u32 as u64) as isize
}

fn request_id_from(timestamp_nanos: u128, process_id: u32) -> usize {
    let mixed = timestamp_nanos as u64 ^ ((process_id as u64) << 32);
    (mixed as usize) | 1
}

fn post_bridge_click_with<F>(
    x: i32,
    y: i32,
    request_id: usize,
    post: F,
) -> Result<(), String>
where
    F: FnOnce(u32, WPARAM, LPARAM) -> windows::core::Result<()>,
{
    post(
        INPUT_BRIDGE_MESSAGE,
        WPARAM(request_id),
        LPARAM(pack_bridge_coordinates(x, y)),
    )
    .map_err(|error| format!("PostMessageW(INPUT_BRIDGE_MESSAGE) failed: {error}"))
}

fn pack_bridge_scroll(length: i32, axis: Axis, request_id: usize) -> usize {
    let axis_code = match axis {
        Axis::Vertical => 0u64,
        Axis::Horizontal => 1,
    };
    let request_id = (request_id as u64) & 0x7fff_ffff;
    ((request_id << 33) | (axis_code << 32) | length as u32 as u64) as usize
}

fn post_bridge_scroll_with<F>(
    x: i32,
    y: i32,
    length: i32,
    axis: Axis,
    request_id: usize,
    post: F,
) -> Result<(), String>
where
    F: FnOnce(u32, WPARAM, LPARAM) -> windows::core::Result<()>,
{
    post(
        SCROLL_BRIDGE_MESSAGE,
        WPARAM(pack_bridge_scroll(length, axis, request_id)),
        LPARAM(pack_bridge_coordinates(x, y)),
    )
    .map_err(|error| format!("PostMessageW(SCROLL_BRIDGE_MESSAGE) failed: {error}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ClientGeometry {
    origin_x: i32,
    origin_y: i32,
    width: i32,
    height: i32,
}

impl ClientGeometry {
    fn screen_to_client(self, screen_x: i32, screen_y: i32) -> Option<(i32, i32)> {
        let x = screen_x.checked_sub(self.origin_x)?;
        let y = screen_y.checked_sub(self.origin_y)?;
        (x >= 0 && y >= 0 && x < self.width && y < self.height).then_some((x, y))
    }
}

fn is_game_executable(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(GAME_EXECUTABLE))
}

fn is_console_window_class(class_name: &str) -> bool {
    class_name.eq_ignore_ascii_case(CONSOLE_WINDOW_CLASS)
}

fn require_exactly_one<T>(matches: Vec<T>) -> Result<T, String> {
    match matches.len() {
        0 => Err("Revolution Idle window not found".to_string()),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => Err("multiple Revolution Idle windows found".to_string()),
    }
}

fn outer_size_for_client(
    outer_width: i32,
    outer_height: i32,
    client_width: i32,
    client_height: i32,
    target_width: i32,
    target_height: i32,
) -> Result<(i32, i32), String> {
    let frame_width = outer_width
        .checked_sub(client_width)
        .ok_or_else(|| "arithmetic overflow while calculating window frame width".to_string())?;
    let frame_height = outer_height
        .checked_sub(client_height)
        .ok_or_else(|| "arithmetic overflow while calculating window frame height".to_string())?;
    let new_outer_width = target_width
        .checked_add(frame_width)
        .ok_or_else(|| "arithmetic overflow while calculating outer width".to_string())?;
    let new_outer_height = target_height
        .checked_add(frame_height)
        .ok_or_else(|| "arithmetic overflow while calculating outer height".to_string())?;
    Ok((new_outer_width, new_outer_height))
}

pub trait WindowControl {
    fn resize_client(&self, width: i32, height: i32) -> Result<(), String>;

    fn write_clipboard(&self, _text: &str) -> Result<(), String> {
        Err("clipboard writing is not supported".to_string())
    }

    fn read_clipboard(&self) -> Result<String, String> {
        Err("clipboard reading is not supported".to_string())
    }

    fn scroll(
        &self,
        _x: i32,
        _y: i32,
        _length: i32,
        _axis: Axis,
    ) -> Result<(), String> {
        Err("window scrolling is not supported".to_string())
    }
}

#[derive(Default)]
pub struct Win32WindowControl;

unsafe extern "system" fn collect_visible_windows(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if unsafe { IsWindowVisible(hwnd).as_bool() } {
        unsafe {
            (*(lparam.0 as *mut Vec<HWND>)).push(hwnd);
        }
    }
    true.into()
}

fn process_image_path(pid: u32) -> Result<PathBuf, String> {
    let process = unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
    }
    .map_err(|error| format!("OpenProcess failed for process {pid}: {error}"))?;

    let mut buffer = vec![0u16; 32_768];
    let mut length = buffer.len() as u32;
    let query_result = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };
    let close_result = unsafe { CloseHandle(process) };

    query_result.map_err(|error| {
        format!("QueryFullProcessImageNameW failed for process {pid}: {error}")
    })?;
    close_result
        .map_err(|error| format!("CloseHandle failed for process {pid}: {error}"))?;

    buffer.truncate(length as usize);
    Ok(PathBuf::from(String::from_utf16_lossy(&buffer)))
}

fn window_class_name(hwnd: HWND) -> Result<String, String> {
    let mut buffer = [0u16; 256];
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if length == 0 {
        return Err(format!("GetClassNameW failed for window {hwnd:?}"));
    }

    Ok(String::from_utf16_lossy(&buffer[..length as usize]))
}

fn find_game_window() -> Result<HWND, String> {
    let mut visible_windows = Vec::new();
    let lparam = LPARAM((&mut visible_windows as *mut Vec<HWND>) as isize);
    unsafe { EnumWindows(Some(collect_visible_windows), lparam) }
        .map_err(|error| format!("EnumWindows failed: {error}"))?;

    let mut matches = Vec::new();
    for hwnd in visible_windows {
        if is_console_window_class(&window_class_name(hwnd)?) {
            continue;
        }

        let mut pid = 0u32;
        let thread_id = unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32))
        };
        if thread_id == 0 {
            return Err(format!(
                "GetWindowThreadProcessId failed for window {hwnd:?}"
            ));
        }

        let path = process_image_path(pid)?;
        if is_game_executable(&path) {
            matches.push(hwnd);
        }
    }

    require_exactly_one(matches)
}

pub(crate) fn post_click_to_game(x: i32, y: i32) -> Result<(), String> {
    let hwnd = find_game_window()?;
    let request_id = request_id_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock is before UNIX epoch: {error}"))?
            .as_nanos(),
        std::process::id(),
    );

    post_bridge_click_with(x, y, request_id, |message, wparam, lparam| unsafe {
        PostMessageW(Some(hwnd), message, wparam, lparam)
    })
}

pub(crate) fn post_scroll_to_game(
    x: i32,
    y: i32,
    length: i32,
    axis: Axis,
) -> Result<(), String> {
    let hwnd = find_game_window()?;
    let request_id = request_id_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock is before UNIX epoch: {error}"))?
            .as_nanos(),
        std::process::id(),
    );

    post_bridge_scroll_with(
        x,
        y,
        length,
        axis,
        request_id,
        |message, wparam, lparam| unsafe {
            PostMessageW(Some(hwnd), message, wparam, lparam)
        },
    )
}

pub(crate) fn screen_to_client_position(
    screen_x: i32,
    screen_y: i32,
) -> Result<Option<(i32, i32)>, String> {
    let hwnd = find_game_window()?;
    let mut client_rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut client_rect) }
        .map_err(|error| format!("GetClientRect failed: {error}"))?;
    let width = client_rect
        .right
        .checked_sub(client_rect.left)
        .ok_or_else(|| "arithmetic overflow while reading client width".to_string())?;
    let height = client_rect
        .bottom
        .checked_sub(client_rect.top)
        .ok_or_else(|| "arithmetic overflow while reading client height".to_string())?;

    let mut origin = POINT { x: 0, y: 0 };
    if !unsafe { ClientToScreen(hwnd, &mut origin).as_bool() } {
        return Err("ClientToScreen failed".to_string());
    }

    Ok((ClientGeometry {
        origin_x: origin.x,
        origin_y: origin.y,
        width,
        height,
    })
    .screen_to_client(screen_x, screen_y))
}

impl WindowControl for Win32WindowControl {
    fn write_clipboard(&self, text: &str) -> Result<(), String> {
        write_unicode_clipboard(text)
    }

    fn read_clipboard(&self) -> Result<String, String> {
        read_unicode_clipboard()
    }

    fn resize_client(&self, width: i32, height: i32) -> Result<(), String> {
        if width <= 0 || height <= 0 {
            return Err("window dimensions must be positive".to_string());
        }

        let hwnd = find_game_window()?;
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }

        let mut outer_rect = RECT::default();
        unsafe { GetWindowRect(hwnd, &mut outer_rect) }
            .map_err(|error| format!("GetWindowRect failed: {error}"))?;

        let mut client_rect = RECT::default();
        unsafe { GetClientRect(hwnd, &mut client_rect) }
            .map_err(|error| format!("GetClientRect failed: {error}"))?;

        let outer_width = outer_rect
            .right
            .checked_sub(outer_rect.left)
            .ok_or_else(|| "arithmetic overflow while reading outer width".to_string())?;
        let outer_height = outer_rect
            .bottom
            .checked_sub(outer_rect.top)
            .ok_or_else(|| "arithmetic overflow while reading outer height".to_string())?;
        let client_width = client_rect
            .right
            .checked_sub(client_rect.left)
            .ok_or_else(|| "arithmetic overflow while reading client width".to_string())?;
        let client_height = client_rect
            .bottom
            .checked_sub(client_rect.top)
            .ok_or_else(|| "arithmetic overflow while reading client height".to_string())?;
        let (new_outer_width, new_outer_height) = outer_size_for_client(
            outer_width,
            outer_height,
            client_width,
            client_height,
            width,
            height,
        )?;

        unsafe {
            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                new_outer_width,
                new_outer_height,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .map_err(|error| format!("SetWindowPos failed: {error}"))?;

        let mut updated_client_rect = RECT::default();
        unsafe { GetClientRect(hwnd, &mut updated_client_rect) }
            .map_err(|error| format!("GetClientRect after resize failed: {error}"))?;
        let actual_width = updated_client_rect
            .right
            .checked_sub(updated_client_rect.left)
            .ok_or_else(|| {
                "arithmetic overflow while reading resized client width".to_string()
            })?;
        let actual_height = updated_client_rect
            .bottom
            .checked_sub(updated_client_rect.top)
            .ok_or_else(|| {
                "arithmetic overflow while reading resized client height".to_string()
            })?;
        if actual_width != width || actual_height != height {
            return Err(format!(
                "client area is {actual_width}x{actual_height}, requested {width}x{height}"
            ));
        }

        Ok(())
    }

    fn scroll(&self, x: i32, y: i32, length: i32, axis: Axis) -> Result<(), String> {
        post_scroll_to_game(x, y, length, axis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn assert_window_control<T: WindowControl>() {}

    #[test]
    fn unicode_clipboard_contents_normalizes_line_endings() {
        assert_eq!(
            unicode_clipboard_contents("a\nb\rc\r\nd").unwrap(),
            vec![97, 13, 10, 98, 13, 10, 99, 13, 10, 100, 0]
        );
    }

    #[test]
    fn matches_only_the_game_executable_name_case_insensitively() {
        assert!(is_game_executable(Path::new(
            r#"C:\Games\Revolution Idle.exe"#,
        )));
        assert!(is_game_executable(Path::new(
            r#"C:\Games\REVOLUTION IDLE.EXE"#,
        )));
        assert!(!is_game_executable(Path::new(
            r#"C:\Games\Revolution Idle Launcher.exe"#,
        )));
    }

    #[test]
    fn identifies_the_windows_console_class_case_insensitively() {
        assert!(is_console_window_class("ConsoleWindowClass"));
        assert!(is_console_window_class("consolewindowclass"));
        assert!(!is_console_window_class("UnityWndClass"));
    }

    #[test]
    fn requires_exactly_one_matching_window() {
        assert_eq!(
            require_exactly_one(Vec::<u32>::new()).unwrap_err(),
            "Revolution Idle window not found"
        );
        assert_eq!(require_exactly_one(vec![17]).unwrap(), 17);
        assert_eq!(
            require_exactly_one(vec![17, 23]).unwrap_err(),
            "multiple Revolution Idle windows found"
        );
    }

    #[test]
    fn converts_screen_points_to_client_coordinates_only_inside_the_client_area() {
        let geometry = ClientGeometry {
            origin_x: -1200,
            origin_y: 40,
            width: 1280,
            height: 720,
        };
        assert_eq!(geometry.screen_to_client(-1200, 40), Some((0, 0)));
        assert_eq!(geometry.screen_to_client(79, 759), Some((1279, 719)));
        assert_eq!(geometry.screen_to_client(-1201, 40), None);
        assert_eq!(geometry.screen_to_client(80, 759), None);
    }

    #[test]
    fn calculates_outer_size_from_the_current_frame_and_checks_overflow() {
        assert_eq!(
            outer_size_for_client(1296, 759, 1280, 720, 800, 600).unwrap(),
            (816, 639)
        );
        assert!(outer_size_for_client(
            i32::MAX,
            i32::MAX,
            1,
            1,
            i32::MAX,
            i32::MAX,
        )
        .is_err());
    }

    #[test]
    fn win32_controller_implements_window_control() {
        assert_window_control::<Win32WindowControl>();
    }

    #[test]
    fn background_click_posts_one_bridge_event() {
        let mut received = Vec::new();
        post_bridge_click_with(1200, 80, 17, |message, wparam, lparam| {
            received.push((message, wparam.0, lparam.0 as u64));
            Ok(())
        })
        .unwrap();

        assert_eq!(received, vec![(0x8417, 17, 0x00000050000004b0)]);
    }

    #[test]
    fn background_scroll_packs_coordinates_length_axis_and_request_id() {
        let mut received = Vec::new();
        post_bridge_scroll_with(1200, 80, -1, Axis::Horizontal, 17, |message, wparam, lparam| {
            received.push((message, wparam.0, lparam.0 as u64));
            Ok(())
        })
        .unwrap();

        assert_eq!(
            received,
            vec![(0x8418, 0x00000023ffffffff, 0x00000050000004b0)]
        );
    }
}
