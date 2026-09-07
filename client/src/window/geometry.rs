#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ClientGeometry {
    pub(super) origin_x: i32,
    pub(super) origin_y: i32,
    pub(super) width: i32,
    pub(super) height: i32,
}

impl ClientGeometry {
    pub(super) fn screen_to_client(self, screen_x: i32, screen_y: i32) -> Option<(i32, i32)> {
        let x = screen_x.checked_sub(self.origin_x)?;
        let y = screen_y.checked_sub(self.origin_y)?;
        (x >= 0 && y >= 0 && x < self.width && y < self.height).then_some((x, y))
    }
}

pub(super) fn outer_size_for_client(
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
