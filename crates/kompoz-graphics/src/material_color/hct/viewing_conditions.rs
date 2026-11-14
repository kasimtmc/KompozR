//! Material Color Utilities' ViewingConditions: what CAM16 assumes about how a color is seen.

use super::cam16::XYZ_TO_CAM16RGB;
use crate::material_color::utils::color_utils::{white_point_d65, y_from_lstar};
use crate::material_color::utils::math_utils::{clamp_double, lerp};
use std::f64::consts::PI;
use std::sync::LazyLock;

/// In what conditions a color is seen, and the values CAM16 works out of them once (Material Color Utilities'
/// ViewingConditions). [`ViewingConditions::default_conditions`] are sRGB's (MCU's DEFAULT): D65, a mid-grey (L*
/// 50) background, average surround, adapting to 200/π × the luminance of L* 50.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewingConditions {
    n: f64,
    aw: f64,
    nbb: f64,
    ncb: f64,
    c: f64,
    nc: f64,
    rgb_d: [f64; 3],
    fl: f64,
    fl_root: f64,
    z: f64,
}

static DEFAULT: LazyLock<ViewingConditions> = LazyLock::new(|| ViewingConditions::default_with_background_lstar(50.0));

impl ViewingConditions {
    /// sRGB's (Material Color Utilities' ViewingConditions.DEFAULT).
    pub fn default_conditions() -> &'static ViewingConditions {
        &DEFAULT
    }

    /// Of a white point (CIE XYZ), the light the eye has adapted to (in lux), the background's L*, the surround
    /// (0 dark, 1 dim, 2 average) and whether the eye discounts the illuminant.
    pub fn make(
        white_point: [f64; 3],
        adapting_luminance: f64,
        background_lstar: f64,
        surround: f64,
        discounting_illuminant: bool,
    ) -> ViewingConditions {
        // A background of pure black is non-physical and leads to infinities that represent the idea that any
        // color viewed in pure black can't be seen.
        let background_lstar = background_lstar.max(0.1);
        let m = XYZ_TO_CAM16RGB;
        let xyz = white_point;
        let r_w = (xyz[0] * m[0][0]) + (xyz[1] * m[0][1]) + (xyz[2] * m[0][2]);
        let g_w = (xyz[0] * m[1][0]) + (xyz[1] * m[1][1]) + (xyz[2] * m[1][2]);
        let b_w = (xyz[0] * m[2][0]) + (xyz[1] * m[2][1]) + (xyz[2] * m[2][2]);
        let f = 0.8 + (surround / 10.0);
        let c = if f >= 0.9 { lerp(0.59, 0.69, (f - 0.9) * 10.0) } else { lerp(0.525, 0.59, (f - 0.8) * 10.0) };
        let d = if discounting_illuminant {
            1.0
        } else {
            f * (1.0 - ((1.0 / 3.6) * ((-adapting_luminance - 42.0) / 92.0).exp()))
        };
        let d = clamp_double(0.0, 1.0, d);
        let nc = f;
        let rgb_d = [d * (100.0 / r_w) + 1.0 - d, d * (100.0 / g_w) + 1.0 - d, d * (100.0 / b_w) + 1.0 - d];
        let k = 1.0 / (5.0 * adapting_luminance + 1.0);
        let k4 = k * k * k * k;
        let k4_f = 1.0 - k4;
        let fl = (k4 * adapting_luminance) + (0.1 * k4_f * k4_f * (5.0 * adapting_luminance).cbrt());
        let n = y_from_lstar(background_lstar) / white_point[1];
        let z = 1.48 + n.sqrt();
        let nbb = 0.725 / n.powf(0.2);
        let ncb = nbb;
        let factors = [
            (fl * rgb_d[0] * r_w / 100.0).powf(0.42),
            (fl * rgb_d[1] * g_w / 100.0).powf(0.42),
            (fl * rgb_d[2] * b_w / 100.0).powf(0.42),
        ];
        let rgb_a = factors.map(|f| (400.0 * f) / (f + 27.13));
        let aw = ((2.0 * rgb_a[0]) + rgb_a[1] + (0.05 * rgb_a[2])) * nbb;
        ViewingConditions { n, aw, nbb, ncb, c, nc, rgb_d, fl, fl_root: fl.powf(0.25), z }
    }

    /// sRGB's, with another background (its L*).
    pub fn default_with_background_lstar(lstar: f64) -> ViewingConditions {
        ViewingConditions::make(white_point_d65(), 200.0 / PI * y_from_lstar(50.0) / 100.0, lstar, 2.0, false)
    }

    pub fn n(&self) -> f64 {
        self.n
    }
    pub fn aw(&self) -> f64 {
        self.aw
    }
    pub fn nbb(&self) -> f64 {
        self.nbb
    }
    pub fn ncb(&self) -> f64 {
        self.ncb
    }
    pub fn c(&self) -> f64 {
        self.c
    }
    pub fn nc(&self) -> f64 {
        self.nc
    }
    pub fn rgb_d(&self) -> [f64; 3] {
        self.rgb_d
    }
    pub fn fl(&self) -> f64 {
        self.fl
    }
    pub fn fl_root(&self) -> f64 {
        self.fl_root
    }
    pub fn z(&self) -> f64 {
        self.z
    }
}
