use windows::{
    Win32::{
        Foundation::{POINT, RECT},
        Graphics::Gdi::ClientToScreen,
        UI::WindowsAndMessaging::{
            GetAncestor, GetClientRect, GetWindowRect, SetWindowPos, ShowWindow, WindowFromPoint,
            GA_ROOT, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SW_RESTORE,
        },
    },
};

mod discovery;
mod clipboard;
mod geometry;

use clipboard::{read_unicode_clipboard, write_unicode_clipboard};
pub(crate) use discovery::find_game_window;
use geometry::{outer_size_for_client, ClientGeometry};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

pub trait WindowControl {
    fn resize_client(&self, width: i32, height: i32) -> Result<(), String>;

    fn write_clipboard(&self, _text: &str) -> Result<(), String> {
        Err("clipboard writing is not supported".to_string())
    }

    fn read_clipboard(&self) -> Result<String, String> {
        Err("clipboard reading is not supported".to_string())
    }

}

#[derive(Default)]
pub struct Win32WindowControl;

pub(crate) fn client_size() -> Result<(i32, i32), String> {
    let hwnd = find_game_window()?;
    let mut client_rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut client_rect) }
        .map_err(|error| format!("GetClientRect failed: {error}"))?;
    Ok((
        client_rect.right.checked_sub(client_rect.left)
            .ok_or_else(|| "arithmetic overflow while reading client width".to_string())?,
        client_rect.bottom.checked_sub(client_rect.top)
            .ok_or_else(|| "arithmetic overflow while reading client height".to_string())?,
    ))
}

pub(crate) fn screen_to_client_position(
    screen_x: i32,
    screen_y: i32,
) -> Result<Option<(i32, i32, i32, i32)>, String> {
    let hwnd = find_game_window()?;
    // A click's screen coordinates can still fall within a minimized (or
    // simply not-focused, or partially covered by another window) game
    // window's last-known geometry, since the OS doesn't clear
    // GetClientRect/ClientToScreen just because a window isn't what's
    // actually visible there. WindowFromPoint answers the question real
    // mouse hit-testing would: which window is actually on top at this
    // exact screen pixel. That covers minimized (never topmost anywhere),
    // fully/partially covered by another window (that window is returned
    // instead), and "visible but not yet focused" (correctly resolves to
    // the game, since clicking it would activate and hit it) all in one
    // check — a foreground-window check alone gets the last two wrong.
    let point = POINT { x: screen_x, y: screen_y };
    let hit = unsafe { GetAncestor(WindowFromPoint(point), GA_ROOT) };
    if hit != hwnd {
        return Ok(None);
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

    Ok((ClientGeometry {
        origin_x: origin.x,
        origin_y: origin.y,
        width,
        height,
    })
    .screen_to_client(screen_x, screen_y)
    .map(|(x, y)| (x, y, width, height)))
}

impl WindowControl for Win32WindowControl {
    fn write_clipboard(&self, text: &str) -> Result<(), String> {
        write_unicode_clipboard(find_game_window()?, text)
    }

    fn read_clipboard(&self) -> Result<String, String> {
        read_unicode_clipboard(find_game_window()?)
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

}

#[cfg(test)]
mod tests {
    use super::*;
    fn assert_window_control<T: WindowControl>() {}

    #[test]
    fn win32_controller_implements_window_control() {
        assert_window_control::<Win32WindowControl>();
    }

}
