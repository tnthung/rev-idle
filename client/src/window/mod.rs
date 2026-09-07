use std::time::{SystemTime, UNIX_EPOCH};

use windows::{
    Win32::{
        Foundation::{POINT, RECT},
        Graphics::Gdi::ClientToScreen,
        UI::WindowsAndMessaging::{
            GetClientRect, GetWindowRect, PostMessageW, SetWindowPos, ShowWindow,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SW_RESTORE,
        },
    },
};

mod discovery;
mod clipboard;
mod bridge_input;
mod geometry;

use bridge_input::{
    post_bridge_click_with, post_bridge_drag_endpoint_with,
    post_bridge_scroll_with, request_id_from, DRAG_END_BRIDGE_MESSAGE, DRAG_START_BRIDGE_MESSAGE,
};
use clipboard::{read_unicode_clipboard, write_unicode_clipboard};
use discovery::find_game_window;
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

    fn scroll(
        &self,
        _x: i32,
        _y: i32,
        _length: i32,
        _axis: Axis,
    ) -> Result<(), String> {
        Err("window scrolling is not supported".to_string())
    }

    fn drag(
        &self,
        _x1: i32,
        _y1: i32,
        _x2: i32,
        _y2: i32,
    ) -> Result<(), String> {
        Err("window dragging is not supported".to_string())
    }
}

#[derive(Default)]
pub struct Win32WindowControl;

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

pub(crate) fn post_drag_to_game(x1: i32, y1: i32, x2: i32, y2: i32) -> Result<(), String> {
    let hwnd = find_game_window()?;
    let request_id = request_id_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock is before UNIX epoch: {error}"))?
            .as_nanos(),
        std::process::id(),
    );

    post_bridge_drag_endpoint_with(
        DRAG_START_BRIDGE_MESSAGE,
        x1,
        y1,
        request_id,
        |message, wparam, lparam| unsafe { PostMessageW(Some(hwnd), message, wparam, lparam) },
    )?;
    post_bridge_drag_endpoint_with(
        DRAG_END_BRIDGE_MESSAGE,
        x2,
        y2,
        request_id,
        |message, wparam, lparam| unsafe { PostMessageW(Some(hwnd), message, wparam, lparam) },
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

    fn drag(&self, x1: i32, y1: i32, x2: i32, y2: i32) -> Result<(), String> {
        post_drag_to_game(x1, y1, x2, y2)
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
