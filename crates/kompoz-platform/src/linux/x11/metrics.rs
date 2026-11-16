//! The X11 window's measures worked out from what the server says (C++'s `desktopScale` and
//! `displayRefreshRate`): the desktop's scale from its resources' `Xft.dpi`, the refresh rate of the display under the
//! window's middle from XRandR's CRTCs and their modes.

use super::ffi::{Crtc, RR_DOUBLE_SCAN, RR_INTERLACE};
use std::ffi::c_ulong;

/// The desktop's scale: `Xft.dpi` in the root window's resources (what KDE and GNOME set), / 96; 1 without it.
pub fn xft_dpi_scale(resources: &str) -> f64 {
    let mut from = 0;
    while let Some(at) = resources[from..].find("Xft.dpi:").map(|i| from + i) {
        if at == 0 || resources.as_bytes()[at - 1] == b'\n' {
            let dpi = leading_number(&resources[at + "Xft.dpi:".len()..]);
            return if dpi > 0.0 { dpi / 96.0 } else { 1.0 };
        }
        from = at + 1;
    }
    1.0
}

/// The number a text begins with, after blanks (as C's `atof`: 0 if none).
fn leading_number(text: &str) -> f64 {
    let text = text.trim_start_matches([' ', '\t']);
    let end = text
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || c == '.' || (i == 0 && (c == '-' || c == '+'))))
        .map_or(text.len(), |(i, _)| i);
    text[..end].parse().unwrap_or(0.0)
}

/// A mode's refresh rate: its pixel clock over its pixels a frame (lines doubled when scanned twice, halved when
/// interlaced); none if it has no timings.
pub fn mode_refresh_rate(dot_clock: c_ulong, h_total: u32, v_total: u32, flags: c_ulong) -> Option<f32> {
    if h_total == 0 || v_total == 0 {
        return None;
    }
    let mut lines = v_total as f64;
    if flags & RR_DOUBLE_SCAN != 0 {
        lines *= 2.0;
    }
    if flags & RR_INTERLACE != 0 {
        lines /= 2.0;
    }
    Some((dot_clock as f64 / (h_total as f64 * lines)) as f32)
}

/// The refresh rate of the display under (`x`, `y`) on the screen (the last CRTC showing it there); 60 where none
/// tells.
pub fn refresh_rate_at(crtcs: &[Crtc], x: i32, y: i32) -> f32 {
    let mut best = 0.0;
    for c in crtcs {
        let Some((clock, h, v, flags)) = c.mode else { continue };
        let under = x >= c.x && y >= c.y && x < c.x + c.width as i32 && y < c.y + c.height as i32;
        if under && let Some(rate) = mode_refresh_rate(clock, h, v, flags) {
            best = rate;
        }
    }
    if best > 0.0 { best } else { 60.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_is_xft_dpi_over_96() {
        assert_eq!(xft_dpi_scale(""), 1.0);
        assert_eq!(xft_dpi_scale("Xft.dpi:\t144\n"), 1.5);
        let kde =
            "Xcursor.size:\t24\nXcursor.theme:\tbreeze_cursors\nXft.antialias:\t1\nXft.dpi:\t120\nXft.hinting:\t1\n";
        assert_eq!(xft_dpi_scale(kde), 1.25);
        // Only at a line's start: another resource ending in it doesn't count.
        assert_eq!(xft_dpi_scale("Foo.Xft.dpi:\t192\nXft.dpi: 96.0\n"), 1.0);
        assert_eq!(xft_dpi_scale("Foo.Xft.dpi:\t192\n"), 1.0);
        assert_eq!(xft_dpi_scale("Xft.dpi:\tnone\n"), 1.0);
        assert_eq!(xft_dpi_scale("Xft.dpi: 0\n"), 1.0);
    }

    #[test]
    fn refresh_rates_come_from_the_mode_under_the_window() {
        // 1920x1080 at 60 Hz (CVT reduced blanking): 138.5 MHz over 2080 x 1111.
        let hd = mode_refresh_rate(138_500_000, 2080, 1111, 0).unwrap();
        assert!((hd - 59.934).abs() < 0.01, "{hd}");
        assert_eq!(mode_refresh_rate(1_000, 10, 10, RR_INTERLACE), Some(20.0));
        assert_eq!(mode_refresh_rate(1_000, 10, 10, RR_DOUBLE_SCAN), Some(5.0));
        assert_eq!(mode_refresh_rate(1_000, 0, 10, 0), None);
        let left = Crtc { x: 0, y: 0, width: 1920, height: 1080, mode: Some((148_500_000, 2200, 1125, 0)) };
        let right = Crtc { x: 1920, y: 0, width: 2560, height: 1440, mode: Some((241_500_000, 2720, 481, 0)) };
        let off = Crtc { x: 0, y: 0, width: 0, height: 0, mode: None };
        let crtcs = [left, right, off];
        assert_eq!(refresh_rate_at(&crtcs, 100, 100), 60.0);
        assert!((refresh_rate_at(&crtcs, 2000, 100) - 184.58).abs() < 0.01);
        assert_eq!(refresh_rate_at(&crtcs, -5, 100), 60.0); // (off every display)
        assert_eq!(refresh_rate_at(&[], 0, 0), 60.0);
    }
}
