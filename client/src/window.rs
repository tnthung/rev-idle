use std::path::{Path, PathBuf};

use windows::{
    core::{BOOL, PWSTR},
    Win32::{
        Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT},
        Graphics::Gdi::ClientToScreen,
        System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetClientRect, GetForegroundWindow, GetWindowRect,
            GetWindowThreadProcessId, GetClassNameW, IsWindowVisible, SetCursorPos,
            SetForegroundWindow, SetWindowPos, ShowWindow, SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOZORDER, SW_RESTORE,
        },
    },
};

const GAME_EXECUTABLE: &str = "Revolution Idle.exe";
const CONSOLE_WINDOW_CLASS: &str = "ConsoleWindowClass";

fn move_cursor_with<F>(x: i32, y: i32, set_cursor_pos: F) -> Result<(), String>
where
    F: FnOnce(i32, i32) -> windows::core::Result<()>,
{
    set_cursor_pos(x, y)
        .map_err(|error| format!("SetCursorPos({x}, {y}) failed: {error}"))
}

pub(crate) fn move_cursor_to_screen(x: i32, y: i32) -> Result<(), String> {
    move_cursor_with(x, y, |x, y| unsafe { SetCursorPos(x, y) })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ClientGeometry {
    origin_x: i32,
    origin_y: i32,
    width: i32,
    height: i32,
}

impl ClientGeometry {
    fn translate(self, x: i32, y: i32) -> Result<(i32, i32), String> {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return Err("coordinates outside client area".to_string());
        }

        let screen_x = self
            .origin_x
            .checked_add(x)
            .ok_or_else(|| "arithmetic overflow while translating x coordinate".to_string())?;
        let screen_y = self
            .origin_y
            .checked_add(y)
            .ok_or_else(|| "arithmetic overflow while translating y coordinate".to_string())?;
        Ok((screen_x, screen_y))
    }

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
    fn focus_and_translate(&self, x: i32, y: i32) -> Result<(i32, i32), String>;
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

    fn focus_and_translate(&self, x: i32, y: i32) -> Result<(i32, i32), String> {
        let hwnd = find_game_window()?;
        if !unsafe { SetForegroundWindow(hwnd).as_bool() } {
            return Err("SetForegroundWindow failed".to_string());
        }
        if unsafe { GetForegroundWindow() } != hwnd {
            return Err("window did not become foreground".to_string());
        }

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

        ClientGeometry {
            origin_x: origin.x,
            origin_y: origin.y,
            width,
            height,
        }
        .translate(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn assert_window_control<T: WindowControl>() {}

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
    fn translates_only_points_inside_the_client_area() {
        let geometry = ClientGeometry {
            origin_x: -1200,
            origin_y: 40,
            width: 1280,
            height: 720,
        };
        assert_eq!(geometry.translate(0, 0).unwrap(), (-1200, 40));
        assert_eq!(geometry.translate(1279, 719).unwrap(), (79, 759));
        for point in [(-1, 0), (0, -1), (1280, 0), (0, 720)] {
            assert!(geometry.translate(point.0, point.1).is_err());
        }
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
    fn cursor_movement_seam_preserves_signed_virtual_screen_coordinates() {
        let mut received = None;
        move_cursor_with(-1920, 1080, |x, y| {
            received = Some((x, y));
            Ok(())
        })
        .unwrap();

        assert_eq!(received, Some((-1920, 1080)));
    }
}
