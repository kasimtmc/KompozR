//! Material's HCT color space (Material Color Utilities' `hct`): hue and chroma as CAM16 sees them, tone as L*
//! (0 black, 100 white). Tone is what contrast is measured by, so a palette of one hue and chroma in many tones
//! gives colors that are told apart by how light they are.

mod cam16;
mod hct_solver;
mod viewing_conditions;

pub use cam16::Cam16;
pub use hct_solver::HctSolver;
pub use viewing_conditions::ViewingConditions;

use crate::color::Color;
use crate::material_color::utils::color_utils::{argb_from_lstar, lstar_from_argb, lstar_from_y};
use crate::material_color::utils::math_utils::sanitize_degrees_double;

/// A color in HCT (Material Color Utilities' Hct): its hue (0..360), chroma and tone (0..100), and the sRGB color
/// they make. Made from hue, chroma and tone, it is the sRGB color of that hue and tone with as much of that
/// chroma as sRGB has there ([`HctSolver`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hct {
    hue: f64,
    chroma: f64,
    tone: f64,
    argb: u32,
}

impl Hct {
    /// Of a hue (degrees), chroma and tone.
    pub fn from(hue: f64, chroma: f64, tone: f64) -> Hct {
        Hct::from_int(HctSolver::solve_to_int(hue, chroma, tone))
    }

    /// Of an ARGB color.
    pub fn from_int(argb: u32) -> Hct {
        let cam = Cam16::from_int(argb);
        Hct { hue: cam.hue(), chroma: cam.chroma(), tone: lstar_from_argb(argb), argb }
    }

    /// Of a color, as drawn in sRGB (8 bits a channel).
    pub fn from_color(color: Color) -> Hct {
        Hct::from_int(color.to_argb())
    }

    pub fn hue(&self) -> f64 {
        self.hue
    }
    pub fn chroma(&self) -> f64 {
        self.chroma
    }
    pub fn tone(&self) -> f64 {
        self.tone
    }
    pub fn to_int(&self) -> u32 {
        self.argb
    }
    pub fn to_color(&self) -> Color {
        Color::from_argb(self.argb)
    }

    /// Another hue, the chroma and tone kept (the chroma as far as sRGB lets it).
    pub fn set_hue(&mut self, new_hue: f64) {
        *self = Hct::from(new_hue, self.chroma, self.tone);
    }
    /// Another chroma (as far as sRGB lets it), the hue and tone kept.
    pub fn set_chroma(&mut self, new_chroma: f64) {
        *self = Hct::from(self.hue, new_chroma, self.tone);
    }
    /// Another tone, the hue and chroma kept (the chroma as far as sRGB lets it).
    pub fn set_tone(&mut self, new_tone: f64) {
        *self = Hct::from(self.hue, self.chroma, new_tone);
    }

    /// 250..270.
    pub fn is_blue(hue: f64) -> bool {
        (250.0..270.0).contains(&hue)
    }
    /// 105..125.
    pub fn is_yellow(hue: f64) -> bool {
        (105.0..125.0).contains(&hue)
    }
    /// 170..207.
    pub fn is_cyan(hue: f64) -> bool {
        (170.0..207.0).contains(&hue)
    }

    /// The color that looks in sRGB's conditions as this one does in `vc` (Material Color Utilities'
    /// inViewingConditions): what a color seen against a dark background looks like, for example.
    pub fn in_viewing_conditions(&self, vc: &ViewingConditions) -> Hct {
        // 1. CAM16 finds the XYZ coordinates of the color in the given conditions.
        let cam16 = Cam16::from_int(self.argb);
        let viewed_in_vc = cam16.xyz_in_viewing_conditions(vc);
        // 2. A CAM16 of those coordinates in the default conditions.
        let recast_in_vc = Cam16::from_xyz_in_viewing_conditions(
            viewed_in_vc[0],
            viewed_in_vc[1],
            viewed_in_vc[2],
            ViewingConditions::default_conditions(),
        );
        // 3. Its hue and chroma, and the L* of the Y in the given conditions.
        Hct::from(recast_in_vc.hue(), recast_in_vc.chroma(), lstar_from_y(viewed_in_vc[1]))
    }
}

/// The sRGB color of a CAM16 hue (degrees), chroma and an L*, as Android's CAMToColor found it before HctSolver
/// (Cam.getInt, as Kompoz's palettes from a seed are made): by bisecting CAM16 lightness for that tone (within
/// 0.2) at that chroma, then, if sRGB has no such color, bisecting chroma (down to within 0.4) for the most it
/// has. Under 1 of chroma, or a tone that rounds to 0 or 100, it is the grey of that tone.
pub fn cam_to_color(hue: f64, chroma: f64, tone: f64) -> u32 {
    let h = sanitize_degrees_double(hue);
    if chroma < 1.0 || tone.round() <= 0.0 || tone.round() >= 100.0 {
        return argb_from_lstar(tone);
    }
    // The most chroma up to the one asked for that this hue has at this tone.
    let (mut high, mut mid, mut low) = (chroma, chroma, 0.0);
    let mut first = true;
    let mut answer = None;
    while (low - high).abs() >= 0.4 {
        let possible = find_cam_by_j(h, mid, tone);
        if first {
            if let Some(color) = possible {
                return color;
            }
            first = false;
        } else if possible.is_some() {
            answer = possible;
            low = mid;
        } else {
            high = mid;
        }
        mid = low + (high - low) / 2.0;
    }
    answer.unwrap_or_else(|| argb_from_lstar(tone))
}

/// The color at `chroma` and `hue` whose tone is nearest `tone` (within 0.2), and as near that hue and chroma as
/// sRGB lets it (within 1 of CAM16-UCS); by bisecting CAM16 lightness.
fn find_cam_by_j(hue: f64, chroma: f64, tone: f64) -> Option<u32> {
    let vc = ViewingConditions::default_conditions();
    let (mut low, mut high) = (0.0f64, 100.0f64);
    let (mut best_l, mut best_e) = (1000.0, 1000.0);
    let mut best = None;
    while (low - high).abs() > 0.01 {
        let mid = low + (high - low) / 2.0;
        // The intended color, clipped to sRGB.
        let clipped = Cam16::from_jch(mid, chroma, hue).to_int();
        let lstar = lstar_from_argb(clipped);
        let d_l = (tone - lstar).abs();
        if d_l < 0.2 {
            // How far the clipped color is from the intended one at its own lightness and chroma: whether the hue
            // slipped too far.
            let cam = Cam16::from_int(clipped);
            let xyz = Cam16::from_jch(cam.j(), cam.chroma(), hue).xyz_in_viewing_conditions(vc);
            let d_e = cam.distance(&Cam16::from_xyz_in_viewing_conditions(xyz[0], xyz[1], xyz[2], vc));
            if d_e <= 1.0 && d_e <= best_e {
                best_l = d_l;
                best_e = d_e;
                best = Some(clipped);
            }
        }
        if best_l == 0.0 && best_e == 0.0 {
            break;
        }
        if lstar < tone {
            low = mid;
        } else {
            high = mid;
        }
    }
    best
}
