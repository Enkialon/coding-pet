use super::{LOOK_ANGLE_HYSTERESIS, LOOK_ENTER_RADIUS, LOOK_EXIT_RADIUS, WIN_HEIGHT, WIN_WIDTH};

/// Convert a pointer position into a stabilized Codex v2 clockwise direction.
/// Index 0 is up, 4 right, 8 down, and 12 left.
pub(super) fn stabilized_look_direction_index(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    current: Option<u8>,
) -> Option<u8> {
    let dx = f64::from(x) - f64::from(width) / 2.0;
    let dy = f64::from(y) - f64::from(height) / 2.0;
    let radius_squared = dx * dx + dy * dy;
    let threshold = if current.is_some() {
        LOOK_EXIT_RADIUS
    } else {
        LOOK_ENTER_RADIUS
    };
    if radius_squared <= threshold * threshold {
        return None;
    }

    let mut angle = dx.atan2(-dy);
    if angle < 0.0 {
        angle += std::f64::consts::TAU;
    }
    Some(quantize_look_angle(angle, current))
}

pub(super) fn normalize_cursor_position(
    x: f64,
    y: f64,
    surface_width: u32,
    surface_height: u32,
) -> (i32, i32) {
    (
        (x * f64::from(WIN_WIDTH) / f64::from(surface_width.max(1))) as i32,
        (y * f64::from(WIN_HEIGHT) / f64::from(surface_height.max(1))) as i32,
    )
}

fn quantize_look_angle(angle: f64, current: Option<u8>) -> u8 {
    let step = std::f64::consts::TAU / 16.0;
    if let Some(current) = current {
        let center = f64::from(current) * step;
        let distance = (angle - center + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        if distance.abs() <= step / 2.0 + LOOK_ANGLE_HYSTERESIS {
            return current;
        }
    }
    ((angle / step).round() as u8) % 16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn look_direction_cardinals_follow_codex_clockwise_order() {
        assert_eq!(
            stabilized_look_direction_index(96, 0, 192, 208, None),
            Some(0)
        );
        assert_eq!(
            stabilized_look_direction_index(191, 104, 192, 208, None),
            Some(4)
        );
        assert_eq!(
            stabilized_look_direction_index(96, 207, 192, 208, None),
            Some(8)
        );
        assert_eq!(
            stabilized_look_direction_index(0, 104, 192, 208, None),
            Some(12)
        );
    }

    #[test]
    fn look_direction_diagonal_and_deadzone() {
        assert_eq!(
            stabilized_look_direction_index(191, 0, 192, 208, None),
            Some(2)
        );
        assert_eq!(
            stabilized_look_direction_index(96, 104, 192, 208, None),
            None
        );
        assert_eq!(
            stabilized_look_direction_index(105, 110, 192, 208, None),
            None
        );
    }

    #[test]
    fn look_deadzone_uses_enter_exit_hysteresis() {
        assert_eq!(
            stabilized_look_direction_index(116, 104, 192, 208, None),
            None
        );
        assert_eq!(
            stabilized_look_direction_index(116, 104, 192, 208, Some(4)),
            Some(4)
        );
        assert_eq!(
            stabilized_look_direction_index(106, 104, 192, 208, Some(4)),
            None
        );
    }

    #[test]
    fn look_angle_hysteresis_prevents_sector_chatter() {
        assert_eq!(quantize_look_angle(13.0_f64.to_radians(), Some(0)), 0);
        assert_eq!(quantize_look_angle(18.0_f64.to_radians(), Some(0)), 1);
    }

    #[test]
    fn cursor_position_is_normalized_on_retina_surfaces() {
        assert_eq!(normalize_cursor_position(192.0, 208.0, 384, 416), (96, 104));
        assert_eq!(normalize_cursor_position(96.0, 104.0, 192, 208), (96, 104));
    }
}
