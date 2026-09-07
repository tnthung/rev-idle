use windows::Win32::Foundation::{LPARAM, WPARAM};

use super::Axis;

const INPUT_BRIDGE_MESSAGE: u32 = 0x8417;
const SCROLL_BRIDGE_MESSAGE: u32 = 0x8418;
pub(super) const DRAG_START_BRIDGE_MESSAGE: u32 = 0x8419;
pub(super) const DRAG_END_BRIDGE_MESSAGE: u32 = 0x841A;

fn pack_bridge_coordinates(x: i32, y: i32) -> isize {
    (((y as u32 as u64) << 32) | x as u32 as u64) as isize
}

pub(super) fn request_id_from(timestamp_nanos: u128, process_id: u32) -> usize {
    let mixed = timestamp_nanos as u64 ^ ((process_id as u64) << 32);
    (mixed as usize) | 1
}

pub(super) fn post_bridge_click_with<F>(
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

pub(super) fn post_bridge_drag_endpoint_with<F>(
    message: u32,
    x: i32,
    y: i32,
    request_id: usize,
    post: F,
) -> Result<(), String>
where
    F: FnOnce(u32, WPARAM, LPARAM) -> windows::core::Result<()>,
{
    post(
        message,
        WPARAM(request_id),
        LPARAM(pack_bridge_coordinates(x, y)),
    )
    .map_err(|error| format!("PostMessageW(0x{message:x}) failed: {error}"))
}

fn pack_bridge_scroll(length: i32, axis: Axis, request_id: usize) -> usize {
    let axis_code = match axis {
        Axis::Vertical => 0u64,
        Axis::Horizontal => 1,
    };
    let request_id = (request_id as u64) & 0x7fff_ffff;
    ((request_id << 33) | (axis_code << 32) | length as u32 as u64) as usize
}

pub(super) fn post_bridge_scroll_with<F>(
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn background_drag_posts_start_and_end_bridge_events_with_shared_request_id() {
        let mut received = Vec::new();
        post_bridge_drag_endpoint_with(DRAG_START_BRIDGE_MESSAGE, 1200, 80, 17, |message, wparam, lparam| {
            received.push((message, wparam.0, lparam.0 as u64));
            Ok(())
        })
        .unwrap();
        post_bridge_drag_endpoint_with(DRAG_END_BRIDGE_MESSAGE, 600, 400, 17, |message, wparam, lparam| {
            received.push((message, wparam.0, lparam.0 as u64));
            Ok(())
        })
        .unwrap();

        assert_eq!(
            received,
            vec![
                (0x8419, 17, 0x00000050000004b0),
                (0x841a, 17, 0x0000019000000258),
            ]
        );
    }
}
