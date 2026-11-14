//! Material Color Utilities' Contrast: WCAG contrast ratios between tones, and the tones that make a ratio.

use crate::material_color::utils::color_utils::{lstar_from_y, y_from_lstar};
use crate::material_color::utils::math_utils::clamp_double;

/// Material Color Utilities' Contrast. A ratio is from 1 (none) to 21 (black on white); 4.5 is what WCAG asks of
/// text, 3 of large text and of what is not text.
pub struct Contrast;

impl Contrast {
    /// The most a ratio's result may fall short of the asked one: the luminance-to-tone round trip is not exact.
    const CONTRAST_RATIO_EPSILON: f64 = 0.04;
    /// What a found tone is moved by, to stay on the safe side of the ratio once rounded into a color.
    const LUMINANCE_GAMUT_MAP_TOLERANCE: f64 = 0.4;

    /// The contrast ratio of two tones (each kept within 0..100).
    pub fn ratio_of_tones(tone_a: f64, tone_b: f64) -> f64 {
        let tone_a = clamp_double(0.0, 100.0, tone_a);
        let tone_b = clamp_double(0.0, 100.0, tone_b);
        Contrast::ratio_of_ys(y_from_lstar(tone_a), y_from_lstar(tone_b))
    }

    /// The contrast ratio of two luminances (0..100).
    pub fn ratio_of_ys(y1: f64, y2: f64) -> f64 {
        let lighter = y1.max(y2);
        let darker = if lighter == y2 { y1 } else { y2 };
        (lighter + 5.0) / (darker + 5.0)
    }

    /// A tone at least `ratio` lighter than `tone`, or -1 where there is none.
    pub fn lighter(tone: f64, ratio: f64) -> f64 {
        if !(0.0..=100.0).contains(&tone) {
            return -1.0;
        }
        // Invert the contrast ratio equation to determine lighter Y given a ratio and darker Y.
        let dark_y = y_from_lstar(tone);
        let light_y = ratio * (dark_y + 5.0) - 5.0;
        if !(0.0..=100.0).contains(&light_y) {
            return -1.0;
        }
        let real_contrast = Contrast::ratio_of_ys(light_y, dark_y);
        let delta = (real_contrast - ratio).abs();
        if real_contrast < ratio && delta > Contrast::CONTRAST_RATIO_EPSILON {
            return -1.0;
        }
        let return_value = lstar_from_y(light_y) + Contrast::LUMINANCE_GAMUT_MAP_TOLERANCE;
        // (A validation the functions it calls could make necessary.)
        if !(0.0..=100.0).contains(&return_value) {
            return -1.0;
        }
        return_value
    }

    /// [`Contrast::lighter`], or 100 where there is none (the most contrast lighter can give).
    pub fn lighter_unsafe(tone: f64, ratio: f64) -> f64 {
        let lighter_safe = Contrast::lighter(tone, ratio);
        if lighter_safe < 0.0 { 100.0 } else { lighter_safe }
    }

    /// A tone at least `ratio` darker than `tone`, or -1 where there is none.
    pub fn darker(tone: f64, ratio: f64) -> f64 {
        if !(0.0..=100.0).contains(&tone) {
            return -1.0;
        }
        // Invert the contrast ratio equation to determine darker Y given a ratio and lighter Y.
        let light_y = y_from_lstar(tone);
        let dark_y = ((light_y + 5.0) / ratio) - 5.0;
        if !(0.0..=100.0).contains(&dark_y) {
            return -1.0;
        }
        let real_contrast = Contrast::ratio_of_ys(light_y, dark_y);
        let delta = (real_contrast - ratio).abs();
        if real_contrast < ratio && delta > Contrast::CONTRAST_RATIO_EPSILON {
            return -1.0;
        }
        // For information on 0.4 constant, see comment in lighter(tone, ratio).
        let return_value = lstar_from_y(dark_y) - Contrast::LUMINANCE_GAMUT_MAP_TOLERANCE;
        if !(0.0..=100.0).contains(&return_value) {
            return -1.0;
        }
        return_value
    }

    /// [`Contrast::darker`], or 0 where there is none (the most contrast darker can give).
    pub fn darker_unsafe(tone: f64, ratio: f64) -> f64 {
        let darker_safe = Contrast::darker(tone, ratio);
        darker_safe.max(0.0)
    }
}
