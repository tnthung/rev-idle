use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use windows::{
    core::{BOOL, PWSTR},
    Win32::{
        Foundation::{CloseHandle, HWND, LPARAM, WPARAM},
        System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId,
            IsWindowVisible, PostMessageW, SetForegroundWindow,
        },
    },
};

const GAME_EXECUTABLE: &str = "Revolution Idle.exe";
const CONSOLE_WINDOW_CLASS: &str = "ConsoleWindowClass";
const CLICK_X: i32 = 1200;
const CLICK_Y: i32 = 80;
const WM_APP: u32 = 0x8000;
const INPUT_BRIDGE_MESSAGE: u32 = WM_APP + 0x417;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = send_click(parse_focus_flag(&args)) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn pack_bridge_coordinates(x: u32, y: u32) -> isize {
    (((y as u64) << 32) | x as u64) as isize
}

fn request_id_from(timestamp_nanos: u128, process_id: u32) -> usize {
    let mixed = timestamp_nanos as u64 ^ ((process_id as u64) << 32);
    (mixed as usize) | 1
}

fn parse_focus_flag(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--focus")
}

unsafe extern "system" fn collect_visible_windows(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if unsafe { IsWindowVisible(hwnd).as_bool() } {
        unsafe {
            (*(lparam.0 as *mut Vec<HWND>)).push(hwnd);
        }
    }
    true.into()
}

fn process_image_path(pid: u32) -> Result<String, String> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
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
    Ok(String::from_utf16_lossy(&buffer))
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
        if window_class_name(hwnd)?.eq_ignore_ascii_case(CONSOLE_WINDOW_CLASS) {
            continue;
        }

        let mut pid = 0u32;
        let thread_id = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if thread_id == 0 {
            return Err(format!(
                "GetWindowThreadProcessId failed for window {hwnd:?}"
            ));
        }

        let path = process_image_path(pid)?;
        if Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(GAME_EXECUTABLE))
        {
            matches.push(hwnd);
        }
    }

    match matches.len() {
        0 => Err("Revolution Idle window not found".to_string()),
        1 => Ok(matches[0]),
        _ => Err("multiple Revolution Idle windows found".to_string()),
    }
}

fn send_click(focus: bool) -> Result<(), String> {
    let hwnd = find_game_window()?;
    let request_id = request_id_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock is before UNIX epoch: {error}"))?
            .as_nanos(),
        std::process::id(),
    );

    if focus {
        if !unsafe { SetForegroundWindow(hwnd).as_bool() } {
            return Err("SetForegroundWindow failed".to_string());
        }

        // wait for 0.1 seconds to allow the window to become foreground
        std::thread::sleep(std::time::Duration::from_millis(100));

        if unsafe { GetForegroundWindow() } != hwnd {
            return Err("window did not become foreground".to_string());
        }
    }

    let foreground_before = unsafe { GetForegroundWindow() };
    unsafe {
        PostMessageW(
            Some(hwnd),
            INPUT_BRIDGE_MESSAGE,
            WPARAM(request_id),
            LPARAM(pack_bridge_coordinates(CLICK_X as u32, CLICK_Y as u32)),
        )
    }
    .map_err(|error| format!("PostMessageW(INPUT_BRIDGE_MESSAGE) failed: {error}"))?;

    if !focus {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let foreground_after = unsafe { GetForegroundWindow() };
    if !focus && foreground_before != hwnd && foreground_after == hwnd {
        return Err("game became foreground unexpectedly".to_string());
    }

    println!(
        "request_id={request_id} coordinates=({CLICK_X}, {CLICK_Y}) target={hwnd:?} foreground_before={foreground_before:?} foreground_after={foreground_after:?}"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn packs_full_width_coordinates_for_bridge_message() {
        assert_eq!(
            super::pack_bridge_coordinates(0xabcdef01, 0x12345678) as u64,
            0x12345678abcdef01,
        );
    }

    #[test]
    fn bridge_message_uses_reserved_wm_app_offset() {
        assert_eq!(super::INPUT_BRIDGE_MESSAGE, 0x8417);
    }

    #[test]
    fn focus_flag_is_enabled_only_when_present() {
        assert!(!super::parse_focus_flag(&[]));
        assert!(super::parse_focus_flag(&["--focus".to_string()]));
    }

    #[test]
    fn generated_request_id_is_never_zero() {
        assert_ne!(super::request_id_from(0, 0), 0);
        assert_ne!(super::request_id_from(123, 456), 0);
    }
}
