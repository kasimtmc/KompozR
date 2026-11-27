//! What KompozInputDevices.kt tells of the input devices and the system's gesture thresholds (C++'s
//! `KompozInput.mediaInfo` and `viewConfiguration`): the most precise pointer there is (a mouse, a touchpad or a
//! stylus is fine, a touchscreen coarse, a D-pad or a game controller blunt), a physical keyboard, a microphone and a
//! camera, how far the screen is (a TV's far, a car's medium) — the host's media info, told again as devices come
//! and go —; and Android's `ViewConfiguration` (touch slop, the long press and double tap timeouts, the fling
//! velocities, the handwriting slop) as the host's. Pure: tested on the host.

use kompoz_graphics::Size;
use kompoz_ui::media_query::{MediaInfo, PointerPrecision, ViewingDistance};
use kompoz_ui::platform::ViewConfiguration;
use std::time::Duration;

/// KompozInputDevices' `mediaInfo()` (`[pointer, keyboard, microphone, camera, distance]`: the pointer 0 none, 1
/// blunt, 2 coarse, 3 fine; the distance 0 near, 1 medium, 2 far) as the host's media info.
pub fn media_info(values: &[i32]) -> MediaInfo {
    let at = |i: usize| values.get(i).copied().unwrap_or(0);
    MediaInfo {
        pointer_precision: match at(0) {
            1 => PointerPrecision::Blunt,
            2 => PointerPrecision::Coarse,
            3 => PointerPrecision::Fine,
            _ => PointerPrecision::None,
        },
        physical_keyboard: at(1) != 0,
        has_microphone: at(2) != 0,
        has_camera: at(3) != 0,
        viewing_distance: match at(4) {
            1 => ViewingDistance::Medium,
            2 => ViewingDistance::Far,
            _ => ViewingDistance::Near,
        },
        ..MediaInfo::default()
    }
}

/// KompozInputDevices' `viewConfiguration()` (`[touchSlop px, longPressTimeout ms, doubleTapTimeout ms,
/// doubleTapMinTime ms, minFlingVelocity px/s, maxFlingVelocity px/s, handwritingSlop px, minimumTouchTarget px]`;
/// what is missing or not positive: the defaults at `density`) as the host's.
pub fn view_configuration(values: &[f32], density: f32) -> ViewConfiguration {
    let base = ViewConfiguration::for_density(kompoz_graphics::Density::new(density.max(0.1), 1.0));
    let at = |i: usize| values.get(i).copied().filter(|v| v.is_finite() && *v > 0.0);
    let millis = |i: usize, d: Duration| at(i).map_or(d, |v| Duration::from_millis(v as u64));
    ViewConfiguration {
        touch_slop: at(0).unwrap_or(base.touch_slop),
        long_press_timeout: millis(1, base.long_press_timeout),
        double_tap_timeout: millis(2, base.double_tap_timeout),
        double_tap_min_time: millis(3, base.double_tap_min_time),
        minimum_fling_velocity: at(4).unwrap_or(base.minimum_fling_velocity),
        maximum_fling_velocity: at(5).unwrap_or(base.maximum_fling_velocity),
        handwriting_slop: at(6).unwrap_or(base.handwriting_slop),
        minimum_touch_target_size: at(7).map_or(base.minimum_touch_target_size, |v| Size::new(v, v)),
        // (the desktop's settings: Android's are Compose's)
        ..base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_are_media_info() {
        let tv = media_info(&[1, 0, 1, 0, 2]);
        assert_eq!(tv.pointer_precision, PointerPrecision::Blunt);
        assert!(!tv.physical_keyboard && tv.has_microphone && !tv.has_camera);
        assert_eq!(tv.viewing_distance, ViewingDistance::Far);
        let tablet = media_info(&[3, 1, 1, 1, 0]);
        assert_eq!(tablet.pointer_precision, PointerPrecision::Fine);
        assert!(tablet.physical_keyboard && tablet.has_camera);
        assert_eq!(media_info(&[]), MediaInfo::default());
    }

    #[test]
    fn the_systems_thresholds_are_the_hosts() {
        let c = view_configuration(&[22.0, 500.0, 300.0, 40.0, 137.5, 22000.0, 5.5, 132.0], 2.75);
        assert_eq!(c.touch_slop, 22.0);
        assert_eq!(c.long_press_timeout, Duration::from_millis(500));
        assert_eq!(c.double_tap_timeout_millis(), 300);
        assert_eq!(c.double_tap_min_time_millis(), 40);
        assert_eq!((c.minimum_fling_velocity, c.maximum_fling_velocity), (137.5, 22000.0));
        assert_eq!(c.handwriting_slop, 5.5);
        assert_eq!(c.minimum_touch_target_size, Size::new(132.0, 132.0));
        // Nothing told: the defaults at the density.
        let d = view_configuration(&[], 2.0);
        assert_eq!(d.touch_slop, 16.0);
        assert_eq!(d.long_press_timeout_millis(), 400);
        assert_eq!(d.minimum_touch_target_size, Size::new(96.0, 96.0));
    }
}
