//! Material Color Utilities' Cam16: the CAM16 color appearance model.

use super::viewing_conditions::ViewingConditions;
use crate::material_color::utils::color_utils::{argb_from_xyz, linearized};
use crate::material_color::utils::math_utils::{signum, to_degrees, to_radians};

/// XYZ to the 'cone' (RGB) responses CAM16 works with.
pub(crate) const XYZ_TO_CAM16RGB: [[f64; 3]; 3] =
    [[0.401288, 0.650173, -0.051461], [-0.250268, 1.204414, 0.045854], [-0.002079, 0.048952, 0.953127]];

/// The cone responses back to XYZ.
pub(crate) const CAM16RGB_TO_XYZ: [[f64; 3]; 3] = [
    [1.86206786, -1.01125463, 0.14918677],
    [0.38752654, 0.62144744, -0.00897398],
    [-0.01584150, -0.03412294, 1.04996444],
];

/// A color in CAM16 (Material Color Utilities' Cam16): hue, chroma, lightness J, brightness Q, colorfulness M,
/// saturation s, and its CAM16-UCS coordinates J*, a*, b*, where distances measure how far apart colors look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cam16 {
    hue: f64,
    chroma: f64,
    j: f64,
    q: f64,
    m: f64,
    s: f64,
    jstar: f64,
    astar: f64,
    bstar: f64,
}

impl Cam16 {
    /// How far apart two colors look (CAM16-UCS ΔE').
    pub fn distance(&self, other: &Cam16) -> f64 {
        let d_j = self.jstar - other.jstar;
        let d_a = self.astar - other.astar;
        let d_b = self.bstar - other.bstar;
        let d_e_prime = (d_j * d_j + d_a * d_a + d_b * d_b).sqrt();
        1.41 * d_e_prime.powf(0.63)
    }

    pub fn hue(&self) -> f64 {
        self.hue
    }
    pub fn chroma(&self) -> f64 {
        self.chroma
    }
    pub fn j(&self) -> f64 {
        self.j
    }
    pub fn q(&self) -> f64 {
        self.q
    }
    pub fn m(&self) -> f64 {
        self.m
    }
    pub fn s(&self) -> f64 {
        self.s
    }
    pub fn jstar(&self) -> f64 {
        self.jstar
    }
    pub fn astar(&self) -> f64 {
        self.astar
    }
    pub fn bstar(&self) -> f64 {
        self.bstar
    }

    /// Of an ARGB color, seen in sRGB's conditions.
    pub fn from_int(argb: u32) -> Cam16 {
        Cam16::from_int_in_viewing_conditions(argb, ViewingConditions::default_conditions())
    }

    pub fn from_int_in_viewing_conditions(argb: u32, viewing_conditions: &ViewingConditions) -> Cam16 {
        let red = ((argb & 0x00ff0000) >> 16) as i32;
        let green = ((argb & 0x0000ff00) >> 8) as i32;
        let blue = (argb & 0x000000ff) as i32;
        let red_l = linearized(red);
        let green_l = linearized(green);
        let blue_l = linearized(blue);
        let x = 0.41233895 * red_l + 0.35762064 * green_l + 0.18051042 * blue_l;
        let y = 0.2126 * red_l + 0.7152 * green_l + 0.0722 * blue_l;
        let z = 0.01932141 * red_l + 0.11916382 * green_l + 0.95034478 * blue_l;
        Cam16::from_xyz_in_viewing_conditions(x, y, z, viewing_conditions)
    }

    pub fn from_xyz_in_viewing_conditions(x: f64, y: f64, z: f64, vc: &ViewingConditions) -> Cam16 {
        // Transform XYZ to 'cone'/'rgb' responses.
        let m = XYZ_TO_CAM16RGB;
        let r_t = (x * m[0][0]) + (y * m[0][1]) + (z * m[0][2]);
        let g_t = (x * m[1][0]) + (y * m[1][1]) + (z * m[1][2]);
        let b_t = (x * m[2][0]) + (y * m[2][1]) + (z * m[2][2]);

        // Discount illuminant.
        let rgb_d = vc.rgb_d();
        let r_d = rgb_d[0] * r_t;
        let g_d = rgb_d[1] * g_t;
        let b_d = rgb_d[2] * b_t;

        // Chromatic adaptation.
        let r_af = (vc.fl() * r_d.abs() / 100.0).powf(0.42);
        let g_af = (vc.fl() * g_d.abs() / 100.0).powf(0.42);
        let b_af = (vc.fl() * b_d.abs() / 100.0).powf(0.42);
        let r_a = signum(r_d) * 400.0 * r_af / (r_af + 27.13);
        let g_a = signum(g_d) * 400.0 * g_af / (g_af + 27.13);
        let b_a = signum(b_d) * 400.0 * b_af / (b_af + 27.13);

        // Redness-greenness and yellowness-blueness.
        let a = (11.0 * r_a + -12.0 * g_a + b_a) / 11.0;
        let b = (r_a + g_a - 2.0 * b_a) / 9.0;

        // Auxiliary components.
        let u = (20.0 * r_a + 20.0 * g_a + 21.0 * b_a) / 20.0;
        let p2 = (40.0 * r_a + 20.0 * g_a + b_a) / 20.0;

        // Hue.
        let atan_degrees = to_degrees(b.atan2(a));
        let hue = if atan_degrees < 0.0 {
            atan_degrees + 360.0
        } else if atan_degrees >= 360.0 {
            atan_degrees - 360.0
        } else {
            atan_degrees
        };
        let hue_radians = to_radians(hue);

        // Achromatic response to color.
        let ac = p2 * vc.nbb();

        // CAM16 lightness and brightness.
        let j = 100.0 * (ac / vc.aw()).powf(vc.c() * vc.z());
        let q = 4.0 / vc.c() * (j / 100.0).sqrt() * (vc.aw() + 4.0) * vc.fl_root();

        // CAM16 chroma, colorfulness and saturation.
        let hue_prime = if hue < 20.14 { hue + 360.0 } else { hue };
        let e_hue = 0.25 * ((to_radians(hue_prime) + 2.0).cos() + 3.8);
        let p1 = 50000.0 / 13.0 * e_hue * vc.nc() * vc.ncb();
        let t = p1 * a.hypot(b) / (u + 0.305);
        let alpha = (1.64 - 0.29f64.powf(vc.n())).powf(0.73) * t.powf(0.9);
        // CAM16 chroma, colorfulness, saturation.
        let c = alpha * (j / 100.0).sqrt();
        let m = c * vc.fl_root();
        let s = 50.0 * ((alpha * vc.c()) / (vc.aw() + 4.0)).sqrt();

        // CAM16-UCS components.
        let jstar = (1.0 + 100.0 * 0.007) * j / (1.0 + 0.007 * j);
        let mstar = 1.0 / 0.0228 * (0.0228 * m).ln_1p();
        let astar = mstar * hue_radians.cos();
        let bstar = mstar * hue_radians.sin();
        Cam16 { hue, chroma: c, j, q, m, s, jstar, astar, bstar }
    }

    /// Of a lightness `j`, chroma `c` and hue `h` (degrees), in sRGB's conditions.
    pub fn from_jch(j: f64, c: f64, h: f64) -> Cam16 {
        Cam16::from_jch_in_viewing_conditions(j, c, h, ViewingConditions::default_conditions())
    }

    pub fn from_jch_in_viewing_conditions(j: f64, c: f64, h: f64, vc: &ViewingConditions) -> Cam16 {
        let q = 4.0 / vc.c() * (j / 100.0).sqrt() * (vc.aw() + 4.0) * vc.fl_root();
        let m = c * vc.fl_root();
        let alpha = c / (j / 100.0).sqrt();
        let s = 50.0 * ((alpha * vc.c()) / (vc.aw() + 4.0)).sqrt();
        let hue_radians = to_radians(h);
        let jstar = (1.0 + 100.0 * 0.007) * j / (1.0 + 0.007 * j);
        let mstar = 1.0 / 0.0228 * (0.0228 * m).ln_1p();
        let astar = mstar * hue_radians.cos();
        let bstar = mstar * hue_radians.sin();
        Cam16 { hue: h, chroma: c, j, q, m, s, jstar, astar, bstar }
    }

    /// Of CAM16-UCS coordinates, in sRGB's conditions.
    pub fn from_ucs(jstar: f64, astar: f64, bstar: f64) -> Cam16 {
        Cam16::from_ucs_in_viewing_conditions(jstar, astar, bstar, ViewingConditions::default_conditions())
    }

    pub fn from_ucs_in_viewing_conditions(jstar: f64, astar: f64, bstar: f64, vc: &ViewingConditions) -> Cam16 {
        let (a, b) = (astar, bstar);
        let m = a.hypot(b);
        let mm = ((m * 0.0228).exp() - 1.0) / 0.0228;
        let c = mm / vc.fl_root();
        let mut h = b.atan2(a) * (180.0 / std::f64::consts::PI);
        if h < 0.0 {
            h += 360.0;
        }
        let j = jstar / (1.0 - (jstar - 100.0) * 0.007);
        Cam16::from_jch_in_viewing_conditions(j, c, h, vc)
    }

    /// The ARGB color it is in sRGB's conditions.
    pub fn to_int(&self) -> u32 {
        self.viewed(ViewingConditions::default_conditions())
    }

    /// The ARGB color it is in `viewing_conditions`.
    pub fn viewed(&self, viewing_conditions: &ViewingConditions) -> u32 {
        let xyz = self.xyz_in_viewing_conditions(viewing_conditions);
        argb_from_xyz(xyz[0], xyz[1], xyz[2])
    }

    /// Its CIE XYZ in `viewing_conditions`, before sRGB clips it.
    pub fn xyz_in_viewing_conditions(&self, vc: &ViewingConditions) -> [f64; 3] {
        let alpha = if self.chroma == 0.0 || self.j == 0.0 { 0.0 } else { self.chroma / (self.j / 100.0).sqrt() };
        let t = (alpha / (1.64 - 0.29f64.powf(vc.n())).powf(0.73)).powf(1.0 / 0.9);
        let h_rad = to_radians(self.hue);
        let e_hue = 0.25 * ((h_rad + 2.0).cos() + 3.8);
        let ac = vc.aw() * (self.j / 100.0).powf(1.0 / vc.c() / vc.z());
        let p1 = e_hue * (50000.0 / 13.0) * vc.nc() * vc.ncb();
        let p2 = ac / vc.nbb();
        let h_sin = h_rad.sin();
        let h_cos = h_rad.cos();
        let gamma = 23.0 * (p2 + 0.305) * t / (23.0 * p1 + 11.0 * t * h_cos + 108.0 * t * h_sin);
        let a = gamma * h_cos;
        let b = gamma * h_sin;
        let r_a = (460.0 * p2 + 451.0 * a + 288.0 * b) / 1403.0;
        let g_a = (460.0 * p2 - 891.0 * a - 261.0 * b) / 1403.0;
        let b_a = (460.0 * p2 - 220.0 * a - 6300.0 * b) / 1403.0;
        let base = |adapted: f64| 0f64.max((27.13 * adapted.abs()) / (400.0 - adapted.abs()));
        let r_c = signum(r_a) * (100.0 / vc.fl()) * base(r_a).powf(1.0 / 0.42);
        let g_c = signum(g_a) * (100.0 / vc.fl()) * base(g_a).powf(1.0 / 0.42);
        let b_c = signum(b_a) * (100.0 / vc.fl()) * base(b_a).powf(1.0 / 0.42);
        let rgb_d = vc.rgb_d();
        let r_f = r_c / rgb_d[0];
        let g_f = g_c / rgb_d[1];
        let b_f = b_c / rgb_d[2];
        let m = CAM16RGB_TO_XYZ;
        let x = (r_f * m[0][0]) + (g_f * m[0][1]) + (b_f * m[0][2]);
        let y = (r_f * m[1][0]) + (g_f * m[1][1]) + (b_f * m[1][2]);
        let z = (r_f * m[2][0]) + (g_f * m[2][1]) + (b_f * m[2][2]);
        [x, y, z]
    }
}
