use std::path::{Path, PathBuf};

use windows::{
    core::BOOL,
    Win32::{
        Foundation::{CloseHandle, HWND, LPARAM},
        System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsWindowVisible,
        },
    },
};

const GAME_EXECUTABLE: &str = "Revolution Idle.exe";
const CONSOLE_WINDOW_CLASS: &str = "ConsoleWindowClass";

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
            windows::core::PWSTR(buffer.as_mut_ptr()),
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

pub(super) fn find_game_window() -> Result<HWND, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
