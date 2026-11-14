//! Material Color Utilities' DislikeAnalyzer: the dark yellow-greens most people dislike, and their fix.

use crate::material_color::hct::Hct;
use crate::material_color::utils::math_utils::round;

/// Material Color Utilities' DislikeAnalyzer. Colors that look like bile or mud (hue 90..111, chroma over 16,
/// tone under 65) are disliked, consistently across ages and cultures; lighter, they become pleasant.
pub struct DislikeAnalyzer;

impl DislikeAnalyzer {
    pub fn is_disliked(hct: &Hct) -> bool {
        let hue_passes = round(hct.hue()) >= 90.0 && round(hct.hue()) <= 111.0;
        let chroma_passes = round(hct.chroma()) > 16.0;
        let tone_passes = round(hct.tone()) < 65.0;
        hue_passes && chroma_passes && tone_passes
    }

    /// The color at tone 70 if it is disliked; otherwise as it is.
    pub fn fix_if_disliked(hct: Hct) -> Hct {
        if DislikeAnalyzer::is_disliked(&hct) { Hct::from(hct.hue(), hct.chroma(), 70.0) } else { hct }
    }
}
