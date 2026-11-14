//! Color spaces (Compose's `androidx.compose.ui.graphics.colorspace`): RGB ones, CIE XYZ, CIE L*a*b* and Oklab,
//! the connectors that turn a color of one into another, and the built-in ones whose ids a [`Color`] keeps.
//! Matrices are 3x3, column-major, as Compose's. The arithmetic is Compose's, operation for operation (Rust does
//! not fuse multiply-adds, as the JVM does not), so the results are its own to the last bit.
//!
//! [`Color`]: crate::Color

// (Compose's literals as it writes them.)
#![allow(clippy::excessive_precision)]

use std::any::Any;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

type Matrix = [f32; 9];
type Float3 = [f32; 3];

/// How a color space's components describe a color (Compose's ColorModel).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ColorModel {
    Rgb,
    Xyz,
    Lab,
    Cmyk,
}

impl ColorModel {
    /// 4 for CMYK, 3 for the others.
    pub const fn component_count(self) -> usize {
        match self {
            ColorModel::Cmyk => 4,
            _ => 3,
        }
    }
}

impl fmt::Display for ColorModel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// How a connector treats colors the destination cannot show (Compose's RenderIntent).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RenderIntent {
    Perceptual,
    Relative,
    Saturation,
    Absolute,
}

impl fmt::Display for RenderIntent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Equal as Kotlin compares a data class's floats (Float.compare: NaN equal to NaN, 0 not to -0).
const fn same_float(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

const fn same_double(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

/// A [`TransferParameters`]' gamma for a PQ curve (internal in Compose).
pub const TypePQish: f64 = -2.0;
/// A [`TransferParameters`]' gamma for an HLG curve (internal in Compose).
pub const TypeHLGish: f64 = -3.0;

/// A white point in CIE xyY, its Y 1 (Compose's WhitePoint).
#[derive(Clone, Copy, Debug)]
pub struct WhitePoint {
    pub x: f32,
    pub y: f32,
}

impl WhitePoint {
    pub const fn new(x: f32, y: f32) -> WhitePoint {
        WhitePoint { x, y }
    }

    /// From CIE XYZ.
    pub const fn from_xyz(x: f32, y: f32, z: f32) -> WhitePoint {
        let sum = x + y + z;
        WhitePoint { x: x / sum, y: y / sum }
    }

    /// In CIE XYZ, Y 1.
    pub const fn to_xyz(self) -> [f32; 3] {
        [self.x / self.y, 1.0, (1.0 - self.x - self.y) / self.y]
    }
}

impl PartialEq for WhitePoint {
    fn eq(&self, o: &WhitePoint) -> bool {
        same_float(self.x, o.x) && same_float(self.y, o.y)
    }
}

/// The CIE standard illuminants' white points (Compose's Illuminant).
pub mod Illuminant {
    use super::WhitePoint;

    pub const A: WhitePoint = WhitePoint::new(0.44757, 0.40745);
    pub const B: WhitePoint = WhitePoint::new(0.34842, 0.35161);
    pub const C: WhitePoint = WhitePoint::new(0.31006, 0.31616);
    pub const D50: WhitePoint = WhitePoint::new(0.34567, 0.35850);
    pub const D55: WhitePoint = WhitePoint::new(0.33242, 0.34743);
    pub const D60: WhitePoint = WhitePoint::new(0.32168, 0.33767);
    pub const D65: WhitePoint = WhitePoint::new(0.31271, 0.32902);
    pub const D75: WhitePoint = WhitePoint::new(0.29902, 0.31485);
    pub const E: WhitePoint = WhitePoint::new(0.33333, 0.33333);
    /// D50 in CIE XYZ (internal in Compose).
    pub const D50Xyz: [f32; 3] = [0.964212, 1.0, 0.825188];
}

/// An ICC parametric curve, type 4 (Compose's TransferParameters): the EOTF is (a·x + b)^gamma + e from d up,
/// c·x + f below it. A gamma of [`TypePQish`] or [`TypeHLGish`] makes it a PQ or HLG curve, its parameters packed
/// as Skia packs them.
#[derive(Clone, Copy, Debug)]
pub struct TransferParameters {
    pub gamma: f64,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl TransferParameters {
    /// Panics where Compose throws: a NaN, or (not PQ or HLG) d outside 0..1, a constant or decreasing curve.
    #[allow(clippy::too_many_arguments)]
    pub fn new(gamma: f64, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> TransferParameters {
        if [a, b, c, d, e, f, gamma].iter().any(|v| v.is_nan()) {
            panic!("Parameters cannot be NaN");
        }
        if gamma != TypePQish && gamma != TypeHLGish {
            if !(0.0..=1.0).contains(&d) {
                panic!("Parameter d must be in the range [0..1], was {d}");
            }
            if d == 0.0 && (a == 0.0 || gamma == 0.0) {
                panic!("Parameter a or g is zero, the transfer function is constant");
            }
            if d >= 1.0 && c == 0.0 {
                panic!("Parameter c is zero, the transfer function is constant");
            }
            if (a == 0.0 || gamma == 0.0) && c == 0.0 {
                panic!("Parameter a or g is zero, and c is zero, the transfer function is constant");
            }
            if c < 0.0 {
                panic!("The transfer function must be increasing");
            }
            if a < 0.0 || gamma < 0.0 {
                panic!("The transfer function must be positive or increasing");
            }
        }
        TransferParameters { gamma, a, b, c, d, e, f }
    }

    /// The built-in spaces' (valid as they are).
    const fn known(gamma: f64, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> TransferParameters {
        TransferParameters { gamma, a, b, c, d, e, f }
    }
}

impl PartialEq for TransferParameters {
    fn eq(&self, r: &TransferParameters) -> bool {
        same_double(self.gamma, r.gamma)
            && same_double(self.a, r.a)
            && same_double(self.b, r.b)
            && same_double(self.c, r.c)
            && same_double(self.d, r.d)
            && same_double(self.e, r.e)
            && same_double(self.f, r.f)
    }
}

/// A chromatic adaptation transform, CIE XYZ to LMS, for the von Kries method (Compose's Adaptation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Adaptation {
    transform: [f32; 9],
    name: &'static str,
}

impl Adaptation {
    pub const Bradford: Adaptation = Adaptation {
        transform: [0.8951, -0.7502, 0.0389, 0.2664, 1.7135, -0.0685, -0.1614, 0.0367, 1.0296],
        name: "Bradford",
    };
    pub const VonKries: Adaptation = Adaptation {
        transform: [0.40024, -0.22630, 0.00000, 0.70760, 1.16532, 0.00000, -0.08081, 0.04570, 0.91822],
        name: "VonKries",
    };
    pub const Ciecat02: Adaptation = Adaptation {
        transform: [0.7328, -0.7036, 0.0030, 0.4296, 1.6975, 0.0136, -0.1624, 0.0061, 0.9834],
        name: "Ciecat02",
    };

    /// CIE XYZ to LMS (internal in Compose).
    pub const fn transform(&self) -> [f32; 9] {
        self.transform
    }
}

impl fmt::Display for Adaptation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name)
    }
}

// ---- Matrices and curves ----------------------------------------------------------------------------------------

const SRGB_PRIMARIES: [f32; 6] = [0.640, 0.330, 0.300, 0.600, 0.150, 0.060];
const NTSC_1953_PRIMARIES: [f32; 6] = [0.67, 0.33, 0.21, 0.71, 0.14, 0.08];
const BT2020_PRIMARIES: [f32; 6] = [0.708, 0.292, 0.170, 0.797, 0.131, 0.046];
const DCI_P3_PRIMARIES: [f32; 6] = [0.680, 0.320, 0.265, 0.690, 0.150, 0.060];
const SRGB_TRANSFER_PARAMETERS: TransferParameters =
    TransferParameters::known(2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045, 0.0, 0.0);
const NONE_TRANSFER_PARAMETERS: TransferParameters =
    TransferParameters::known(2.2, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045, 0.0, 0.0);
const BT709_TRANSFER_PARAMETERS: TransferParameters =
    TransferParameters::known(1.0 / 0.45, 1.0 / 1.099, 0.099 / 1.099, 1.0 / 4.5, 0.081, 0.0, 0.0);
// HLG and PQ, with an SDR white point of 203 nits.
const BT2020_HLG_TRANSFER_PARAMETERS: TransferParameters =
    TransferParameters::known(TypeHLGish, 2.0, 2.0, 1.0 / 0.17883277, 0.28466892, 0.55991073, -0.685490157);
const BT2020_PQ_TRANSFER_PARAMETERS: TransferParameters = TransferParameters::known(
    TypePQish,
    -1.555223,
    1.860454,
    32.0 / 2523.0,
    2413.0 / 128.0,
    -2392.0 / 128.0,
    8192.0 / 1305.0,
);

/// Kotlin's Float.fastCoerceIn (a NaN as it is).
const fn fast_coerce_in(x: f32, min: f32, max: f32) -> f32 {
    let x = if x < min { min } else { x };
    if x > max { max } else { x }
}

/// Kotlin's Double.coerceIn (a NaN as it is).
fn coerce_in(x: f64, min: f64, max: f64) -> f64 {
    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}

/// Compose's fastCbrt: a first guess from the float's bits, then two Newton-Raphson steps.
fn fast_cbrt(x: f32) -> f32 {
    let v = (x.to_bits() as i32 as i64) & 0x1_ffff_ffff;
    let mut estimate = f32::from_bits(0x2a51_0554u32.wrapping_add((v / 3) as u32));
    estimate -= (estimate - x / (estimate * estimate)) * (1.0 / 3.0);
    estimate -= (estimate - x / (estimate * estimate)) * (1.0 / 3.0);
    estimate
}

const fn inverse3x3(m: &Matrix) -> Matrix {
    let (a, b, c, d, e, f, g, h, i) = (m[0], m[3], m[6], m[1], m[4], m[7], m[2], m[5], m[8]);
    let x_a = e * i - f * h;
    let x_b = f * g - d * i;
    let x_c = d * h - e * g;
    let det = a * x_a + b * x_b + c * x_c;
    [
        x_a / det,
        x_b / det,
        x_c / det,
        (c * h - b * i) / det,
        (a * i - c * g) / det,
        (b * g - a * h) / det,
        (b * f - c * e) / det,
        (c * d - a * f) / det,
        (a * e - b * d) / det,
    ]
}

const fn mul3x3(lhs: &Matrix, rhs: &Matrix) -> Matrix {
    [
        lhs[0] * rhs[0] + lhs[3] * rhs[1] + lhs[6] * rhs[2],
        lhs[1] * rhs[0] + lhs[4] * rhs[1] + lhs[7] * rhs[2],
        lhs[2] * rhs[0] + lhs[5] * rhs[1] + lhs[8] * rhs[2],
        lhs[0] * rhs[3] + lhs[3] * rhs[4] + lhs[6] * rhs[5],
        lhs[1] * rhs[3] + lhs[4] * rhs[4] + lhs[7] * rhs[5],
        lhs[2] * rhs[3] + lhs[5] * rhs[4] + lhs[8] * rhs[5],
        lhs[0] * rhs[6] + lhs[3] * rhs[7] + lhs[6] * rhs[8],
        lhs[1] * rhs[6] + lhs[4] * rhs[7] + lhs[7] * rhs[8],
        lhs[2] * rhs[6] + lhs[5] * rhs[7] + lhs[8] * rhs[8],
    ]
}

/// `rhs` times the matrix, in place.
const fn mul3x3_float3(lhs: &Matrix, rhs: &mut Float3) {
    let (r0, r1, r2) = (rhs[0], rhs[1], rhs[2]);
    rhs[0] = lhs[0] * r0 + lhs[3] * r1 + lhs[6] * r2;
    rhs[1] = lhs[1] * r0 + lhs[4] * r1 + lhs[7] * r2;
    rhs[2] = lhs[2] * r0 + lhs[5] * r1 + lhs[8] * r2;
}

/// A diagonal matrix (its diagonal) times a matrix.
const fn mul3x3_diag(lhs: &Float3, rhs: &Matrix) -> Matrix {
    [
        lhs[0] * rhs[0],
        lhs[1] * rhs[1],
        lhs[2] * rhs[2],
        lhs[0] * rhs[3],
        lhs[1] * rhs[4],
        lhs[2] * rhs[5],
        lhs[0] * rhs[6],
        lhs[1] * rhs[7],
        lhs[2] * rhs[8],
    ]
}

/// The von Kries transform from one white point (CIE XYZ) to another. As Compose's, it leaves the two white points
/// in LMS (Compose's RgbConnector goes on with them).
const fn chromatic_adaptation(matrix: &Matrix, src_white_point: &mut Float3, dst_white_point: &mut Float3) -> Matrix {
    mul3x3_float3(matrix, src_white_point);
    mul3x3_float3(matrix, dst_white_point);
    let lms = [
        dst_white_point[0] / src_white_point[0],
        dst_white_point[1] / src_white_point[1],
        dst_white_point[2] / src_white_point[2],
    ];
    mul3x3(&inverse3x3(matrix), &mul3x3_diag(&lms, matrix))
}

fn compare_parameters(a: &TransferParameters, b: Option<&TransferParameters>) -> bool {
    let Some(b) = b else { return false };
    (a.a - b.a).abs() < 1e-3
        && (a.b - b.b).abs() < 1e-3
        && (a.c - b.c).abs() < 1e-3
        && (a.d - b.d).abs() < 2e-3 // (variations in sRGB's OETF and EOTF)
        && (a.e - b.e).abs() < 1e-3
        && (a.f - b.f).abs() < 1e-3
        && (a.gamma - b.gamma).abs() < 1e-3
}

fn compare_white_points(a: WhitePoint, b: WhitePoint) -> bool {
    (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3
}

fn compare_arrays(a: &[f32], b: &[f32]) -> bool {
    a.iter().zip(b).all(|(&x, &y)| same_float(x, y) || (x - y).abs() <= 1e-3 || (x - y).is_nan())
}

// Piecewise gamma responses and their reciprocals; the abs ones sign(x)·f(|x|), for spaces that allow negative
// values.
fn rcp_response(x: f64, a: f64, b: f64, c: f64, d: f64, g: f64) -> f64 {
    if x >= d * c { (x.powf(1.0 / g) - b) / a } else { x / c }
}
fn response(x: f64, a: f64, b: f64, c: f64, d: f64, g: f64) -> f64 {
    if x >= d { (a * x + b).powf(g) } else { c * x }
}
#[allow(clippy::too_many_arguments)]
fn rcp_response_ef(x: f64, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64, g: f64) -> f64 {
    if x >= d * c { ((x - e).powf(1.0 / g) - b) / a } else { (x - f) / c }
}
#[allow(clippy::too_many_arguments)]
fn response_ef(x: f64, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64, g: f64) -> f64 {
    if x >= d { (a * x + b).powf(g) + e } else { c * x + f }
}
fn abs_rcp_response(x: f64, a: f64, b: f64, c: f64, d: f64, g: f64) -> f64 {
    rcp_response(if x < 0.0 { -x } else { x }, a, b, c, d, g).copysign(x)
}
fn abs_response(x: f64, a: f64, b: f64, c: f64, d: f64, g: f64) -> f64 {
    response(if x < 0.0 { -x } else { x }, a, b, c, d, g).copysign(x)
}

// HLG and PQ (SMPTE ST 2084), their parameters packed as Skia packs them.
fn transfer_hlg_oetf(p: &TransferParameters, x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let mut abs_x = x * sign;
    let (r, g, a, b, c, k) = (1.0 / p.a, 1.0 / p.b, 1.0 / p.c, p.d, p.e, p.f + 1.0);
    abs_x /= k;
    sign * if abs_x <= 1.0 { r * abs_x.powf(g) } else { a * (abs_x - b).ln() + c }
}
fn transfer_hlg_eotf(p: &TransferParameters, x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let abs_x = x * sign;
    let (r, g, a, b, c, k) = (p.a, p.b, p.c, p.d, p.e, p.f + 1.0);
    k * sign * if abs_x * r <= 1.0 { (abs_x * r).powf(g) } else { ((abs_x - c) * a).exp() + b }
}
fn transfer_st2048_oetf(p: &TransferParameters, x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let abs_x = x * sign;
    let (a, b, c, d, e, f) = (-p.a, p.d, 1.0 / p.f, p.b, -p.e, 1.0 / p.c);
    let tmp = (a + b * abs_x.powf(c)).max(0.0);
    sign * (tmp / (d + e * abs_x.powf(c))).powf(f)
}
fn transfer_st2048_eotf(p: &TransferParameters, x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let abs_x = x * sign;
    let mut tmp = p.a + p.b * abs_x.powf(p.c);
    tmp = if tmp < 0.0 { 0.0 } else { tmp };
    sign * (tmp / (p.d + p.e * abs_x.powf(p.c))).powf(p.f)
}

/// An RGB space's transfer function, before its range keeps it within min..max.
#[derive(Clone)]
enum Curve {
    Identity,
    /// x^(1/gamma) or x^gamma, negatives as 0 (a gamma's OETF and EOTF).
    GammaOetf(f64),
    GammaEotf(f64),
    /// TransferParameters' OETF and EOTF (Compose's generateOetf and generateEotf).
    ParametricOetf(TransferParameters),
    ParametricEotf(TransferParameters),
    /// The same, sign(x)·f(|x|) (extended sRGB's).
    AbsParametricOetf(TransferParameters),
    AbsParametricEotf(TransferParameters),
    Custom(Arc<dyn Fn(f64) -> f64 + Send + Sync>),
}

impl Curve {
    fn custom(f: impl Fn(f64) -> f64 + Send + Sync + 'static) -> Curve {
        Curve::Custom(Arc::new(f))
    }

    fn apply(&self, x: f64) -> f64 {
        match self {
            Curve::Identity => x,
            Curve::GammaOetf(gamma) => (if x < 0.0 { 0.0 } else { x }).powf(1.0 / gamma),
            Curve::GammaEotf(gamma) => (if x < 0.0 { 0.0 } else { x }).powf(*gamma),
            Curve::ParametricOetf(p) => {
                if p.gamma == TypeHLGish {
                    transfer_hlg_oetf(p, x)
                } else if p.gamma == TypePQish {
                    transfer_st2048_oetf(p, x)
                } else if p.e == 0.0 && p.f == 0.0 {
                    rcp_response(x, p.a, p.b, p.c, p.d, p.gamma)
                } else {
                    rcp_response_ef(x, p.a, p.b, p.c, p.d, p.e, p.f, p.gamma)
                }
            }
            Curve::ParametricEotf(p) => {
                if p.gamma == TypeHLGish {
                    transfer_hlg_eotf(p, x)
                } else if p.gamma == TypePQish {
                    transfer_st2048_eotf(p, x)
                } else if p.e == 0.0 && p.f == 0.0 {
                    response(x, p.a, p.b, p.c, p.d, p.gamma)
                } else {
                    response_ef(x, p.a, p.b, p.c, p.d, p.e, p.f, p.gamma)
                }
            }
            Curve::AbsParametricOetf(p) => abs_rcp_response(x, p.a, p.b, p.c, p.d, p.gamma),
            Curve::AbsParametricEotf(p) => abs_response(x, p.a, p.b, p.c, p.d, p.gamma),
            Curve::Custom(f) => f(x),
        }
    }

    /// The same function (Compose compares the function objects; an adapted space shares its source's).
    fn same(&self, o: &Curve) -> bool {
        match (self, o) {
            (Curve::Identity, Curve::Identity) => true,
            (Curve::GammaOetf(a), Curve::GammaOetf(b)) | (Curve::GammaEotf(a), Curve::GammaEotf(b)) => {
                same_double(*a, *b)
            }
            (Curve::ParametricOetf(a), Curve::ParametricOetf(b))
            | (Curve::ParametricEotf(a), Curve::ParametricEotf(b))
            | (Curve::AbsParametricOetf(a), Curve::AbsParametricOetf(b))
            | (Curve::AbsParametricEotf(a), Curve::AbsParametricEotf(b)) => a == b,
            (Curve::Custom(a), Curve::Custom(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

const fn compute_primaries(to_xyz: &Matrix) -> [f32; 6] {
    let mut r = [1.0, 0.0, 0.0];
    let mut g = [0.0, 1.0, 0.0];
    let mut b = [0.0, 0.0, 1.0];
    mul3x3_float3(to_xyz, &mut r);
    mul3x3_float3(to_xyz, &mut g);
    mul3x3_float3(to_xyz, &mut b);
    let r_sum = r[0] + r[1] + r[2];
    let g_sum = g[0] + g[1] + g[2];
    let b_sum = b[0] + b[1] + b[2];
    [r[0] / r_sum, r[1] / r_sum, g[0] / g_sum, g[1] / g_sum, b[0] / b_sum, b[1] / b_sum]
}

const fn compute_white_point(to_xyz: &Matrix) -> WhitePoint {
    let mut w = [1.0, 1.0, 1.0];
    mul3x3_float3(to_xyz, &mut w);
    let sum = w[0] + w[1] + w[2];
    WhitePoint::new(w[0] / sum, w[1] / sum)
}

/// 6 floats in CIE xyY as they are; 9 in CIE XYZ to xyY.
const fn xy_primaries(primaries: &[f32]) -> [f32; 6] {
    let mut xy = [0.0; 6];
    if primaries.len() == 9 {
        let mut i = 0;
        while i < 3 {
            let sum = primaries[i * 3] + primaries[i * 3 + 1] + primaries[i * 3 + 2];
            xy[i * 2] = primaries[i * 3] / sum;
            xy[i * 2 + 1] = primaries[i * 3 + 1] / sum;
            i += 1;
        }
    } else {
        let mut i = 0;
        while i < 6 {
            xy[i] = primaries[i];
            i += 1;
        }
    }
    xy
}

const fn compute_xyz_matrix(primaries: &[f32; 6], white_point: WhitePoint) -> Matrix {
    let (rx, ry, gx, gy, bx, by) = (primaries[0], primaries[1], primaries[2], primaries[3], primaries[4], primaries[5]);
    let (wx, wy) = (white_point.x, white_point.y);
    let one_rx_ry = (1.0 - rx) / ry;
    let one_gx_gy = (1.0 - gx) / gy;
    let one_bx_by = (1.0 - bx) / by;
    let one_wx_wy = (1.0 - wx) / wy;
    let rx_ry = rx / ry;
    let gx_gy = gx / gy;
    let bx_by = bx / by;
    let wx_wy = wx / wy;
    let by_numerator = (one_wx_wy - one_rx_ry) * (gx_gy - rx_ry) - (wx_wy - rx_ry) * (one_gx_gy - one_rx_ry);
    let by_denominator = (one_bx_by - one_rx_ry) * (gx_gy - rx_ry) - (bx_by - rx_ry) * (one_gx_gy - one_rx_ry);
    let b_y = by_numerator / by_denominator;
    let g_y = (wx_wy - rx_ry - b_y * (bx_by - rx_ry)) / (gx_gy - rx_ry);
    let r_y = 1.0 - g_y - b_y;
    let r_y_ry = r_y / ry;
    let g_y_gy = g_y / gy;
    let b_y_by = b_y / by;
    [
        r_y_ry * rx,
        r_y,
        r_y_ry * (1.0 - rx - ry),
        g_y_gy * gx,
        g_y,
        g_y_gy * (1.0 - gx - gy),
        b_y_by * bx,
        b_y,
        b_y_by * (1.0 - bx - by),
    ]
}

const fn area(p: &[f32; 6]) -> f32 {
    let (rx, ry, gx, gy, bx, by) = (p[0], p[1], p[2], p[3], p[4], p[5]);
    let det = rx * gy + ry * bx + gx * by - gy * bx - ry * gx - rx * by;
    let r = 0.5 * det;
    if r < 0.0 { -r } else { r }
}

const fn cross(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    ax * by - ay * bx
}

/// Whether the triangle `p1` holds the triangle `p2`.
const fn contains(p1: &[f32; 6], p2: &[f32; 6]) -> bool {
    // p1's vertices with p2's as the origin
    let p0 = [p1[0] - p2[0], p1[1] - p2[1], p1[2] - p2[2], p1[3] - p2[3], p1[4] - p2[4], p1[5] - p2[5]];
    if cross(p0[0], p0[1], p2[0] - p2[4], p2[1] - p2[5]) < 0.0
        || cross(p2[0] - p2[2], p2[1] - p2[3], p0[0], p0[1]) < 0.0
    {
        return false;
    }
    if cross(p0[2], p0[3], p2[2] - p2[0], p2[3] - p2[1]) < 0.0
        || cross(p2[2] - p2[4], p2[3] - p2[5], p0[2], p0[3]) < 0.0
    {
        return false;
    }
    !(cross(p0[4], p0[5], p2[4] - p2[2], p2[5] - p2[3]) < 0.0
        || cross(p2[4] - p2[0], p2[5] - p2[1], p0[4], p0[5]) < 0.0)
}

const fn is_wide_gamut(primaries: &[f32; 6], min: f32, max: f32) -> bool {
    (area(primaries) / area(&NTSC_1953_PRIMARIES) > 0.9 && contains(primaries, &SRGB_PRIMARIES))
        || (min < 0.0 && max > 1.0)
}

// ---- ColorSpace -------------------------------------------------------------------------------------------------

/// The lowest and the highest id a color space has: a built-in's (ColorSpaces) or, for any other, `MIN_ID`.
pub const MIN_ID: i32 = -1;
pub const MAX_ID: i32 = 63;

/// A color space (Compose's ColorSpace): its components' model and range, and how they turn into CIE XYZ (D50)
/// and back. Its id is the built-in space's ([`ColorSpaces`]), [`MIN_ID`] for any other. Two are equal when they
/// are of the same type with the same id, name and model (an [`Rgb`]'s: also its white point, primaries, range
/// and transfer parameters; with none on either side, whatever its functions, as Compose's; with them on one side
/// only, the same functions).
pub trait ColorSpace: Any + Send + Sync + 'static {
    fn name(&self) -> &str;
    fn model(&self) -> ColorModel;
    /// Internal in Compose.
    fn id(&self) -> i32;
    fn component_count(&self) -> usize {
        self.model().component_count()
    }
    fn is_wide_gamut(&self) -> bool;
    fn is_srgb(&self) -> bool {
        false
    }
    fn get_min_value(&self, component: usize) -> f32;
    fn get_max_value(&self, component: usize) -> f32;

    /// `v` into CIE XYZ, in place.
    fn to_xyz_in_place(&self, v: &mut [f32; 3]);
    /// `v` from CIE XYZ, in place.
    #[allow(clippy::wrong_self_convention)]
    fn from_xyz_in_place(&self, v: &mut [f32; 3]);

    fn to_xyz(&self, r: f32, g: f32, b: f32) -> [f32; 3] {
        let mut v = [r, g, b];
        self.to_xyz_in_place(&mut v);
        v
    }
    #[allow(clippy::wrong_self_convention)]
    fn from_xyz(&self, x: f32, y: f32, z: f32) -> [f32; 3] {
        let mut v = [x, y, z];
        self.from_xyz_in_place(&mut v);
        v
    }

    /// This space as an [`Rgb`], if it is one.
    fn as_rgb(&self) -> Option<&Rgb> {
        None
    }
    /// This space as a `dyn ColorSpace`.
    fn as_dyn(&self) -> &dyn ColorSpace;

    /// The connector from this space to `destination` (Compose's ColorSpace.connect), perceptual: from sRGB to
    /// sRGB, or a space to itself, the identity (its intent Relative). Two built-in spaces' is made once and kept
    /// for good (Compose's cache, by ids and intent); others' are made anew.
    fn connect<'a>(&'a self, destination: &'a dyn ColorSpace) -> Connector<'a> {
        connect(self.as_dyn(), destination, RenderIntent::Perceptual)
    }
    /// The same with another intent.
    fn connect_with<'a>(&'a self, destination: &'a dyn ColorSpace, intent: RenderIntent) -> Connector<'a> {
        connect(self.as_dyn(), destination, intent)
    }

    /// This space with another white point, Bradford's (Compose's ColorSpace.adapt): an RGB one's colors adapted to
    /// it, a new [`Rgb`] (id [`MIN_ID`]). One of another model, or already of that white point, is this space
    /// itself.
    fn adapt(&self, white_point: WhitePoint) -> Adapted<'_> {
        self.adapt_with(white_point, Adaptation::Bradford)
    }
    fn adapt_with(&self, white_point: WhitePoint, adaptation: Adaptation) -> Adapted<'_> {
        match self.as_rgb() {
            Some(rgb) if !compare_white_points(rgb.white_point, white_point) => {
                let mut xyz = white_point.to_xyz();
                let mut source = rgb.white_point.to_xyz();
                let adaptation_transform = chromatic_adaptation(&adaptation.transform, &mut source, &mut xyz);
                Adapted::Rgb(Box::new(Rgb::adapted(rgb, &mul3x3(&adaptation_transform, &rgb.transform), white_point)))
            }
            _ => Adapted::Same(self.as_dyn()),
        }
    }

    /// Equal as Compose's equals (see the trait).
    fn equals(&self, other: &dyn ColorSpace) -> bool;
}

/// The same type, id, name and model (Compose's ColorSpace.equals).
fn base_equals(a: &dyn ColorSpace, b: &dyn ColorSpace) -> bool {
    if same_space(a, b) {
        return true;
    }
    (a as &dyn Any).type_id() == (b as &dyn Any).type_id()
        && a.id() == b.id()
        && a.name() == b.name()
        && a.model() == b.model()
}

/// The very same space (an object, not an equal one).
pub fn same_space(a: &dyn ColorSpace, b: &dyn ColorSpace) -> bool {
    std::ptr::addr_eq(a as *const dyn ColorSpace, b as *const dyn ColorSpace)
}

impl PartialEq for dyn ColorSpace {
    fn eq(&self, other: &dyn ColorSpace) -> bool {
        self.equals(other)
    }
}

impl fmt::Display for dyn ColorSpace {
    /// "name (id=…, model=…)".
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} (id={}, model={})", self.name(), self.id(), self.model())
    }
}

impl fmt::Debug for dyn ColorSpace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

macro_rules! display_as_color_space {
    ($($t:ty)*) => {$(
        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                fmt::Display::fmt(self.as_dyn(), f)
            }
        }
        impl fmt::Debug for $t {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                fmt::Display::fmt(self.as_dyn(), f)
            }
        }
        impl PartialEq for $t {
            fn eq(&self, other: &$t) -> bool {
                self.equals(other)
            }
        }
    )*};
}
display_as_color_space!(Rgb Xyz Lab Oklab);

/// What [`ColorSpace::adapt`] gives: the space itself, or a new RGB one.
pub enum Adapted<'a> {
    Same(&'a dyn ColorSpace),
    Rgb(Box<Rgb>),
}

impl Deref for Adapted<'_> {
    type Target = dyn ColorSpace;
    fn deref(&self) -> &Self::Target {
        match self {
            Adapted::Same(space) => *space,
            Adapted::Rgb(rgb) => &**rgb,
        }
    }
}

/// CIE XYZ, D50, within -2..2 (Compose's Xyz).
pub struct Xyz {
    name: &'static str,
    id: i32,
}

impl Xyz {
    fn clamp(v: &mut Float3) {
        for c in v {
            *c = fast_coerce_in(*c, -2.0, 2.0);
        }
    }
}

impl ColorSpace for Xyz {
    fn name(&self) -> &str {
        self.name
    }
    fn model(&self) -> ColorModel {
        ColorModel::Xyz
    }
    fn id(&self) -> i32 {
        self.id
    }
    fn is_wide_gamut(&self) -> bool {
        true
    }
    fn get_min_value(&self, _component: usize) -> f32 {
        -2.0
    }
    fn get_max_value(&self, _component: usize) -> f32 {
        2.0
    }
    fn to_xyz_in_place(&self, v: &mut [f32; 3]) {
        Xyz::clamp(v);
    }
    fn from_xyz_in_place(&self, v: &mut [f32; 3]) {
        Xyz::clamp(v);
    }
    fn as_dyn(&self) -> &dyn ColorSpace {
        self
    }
    fn equals(&self, other: &dyn ColorSpace) -> bool {
        base_equals(self, other)
    }
}

/// CIE L*a*b*, through CIE XYZ D50 (Compose's Lab): L 0..100, a and b -128..128.
pub struct Lab {
    name: &'static str,
    id: i32,
}

impl Lab {
    const A: f32 = 216.0 / 24389.0;
    const B: f32 = 841.0 / 108.0;
    const C: f32 = 4.0 / 29.0;
    const D: f32 = 6.0 / 29.0;

    /// Kotlin's cbrt of a Float.
    fn cbrt(x: f32) -> f32 {
        (x as f64).cbrt() as f32
    }
}

impl ColorSpace for Lab {
    fn name(&self) -> &str {
        self.name
    }
    fn model(&self) -> ColorModel {
        ColorModel::Lab
    }
    fn id(&self) -> i32 {
        self.id
    }
    fn is_wide_gamut(&self) -> bool {
        true
    }
    fn get_min_value(&self, component: usize) -> f32 {
        if component == 0 { 0.0 } else { -128.0 }
    }
    fn get_max_value(&self, component: usize) -> f32 {
        if component == 0 { 100.0 } else { 128.0 }
    }
    fn to_xyz_in_place(&self, v: &mut [f32; 3]) {
        v[0] = fast_coerce_in(v[0], 0.0, 100.0);
        v[1] = fast_coerce_in(v[1], -128.0, 128.0);
        v[2] = fast_coerce_in(v[2], -128.0, 128.0);
        let fy = (v[0] + 16.0) / 116.0;
        let fx = fy + (v[1] * 0.002);
        let fz = fy - (v[2] * 0.005);
        let x = if fx > Lab::D { fx * fx * fx } else { (1.0 / Lab::B) * (fx - Lab::C) };
        let y = if fy > Lab::D { fy * fy * fy } else { (1.0 / Lab::B) * (fy - Lab::C) };
        let z = if fz > Lab::D { fz * fz * fz } else { (1.0 / Lab::B) * (fz - Lab::C) };
        v[0] = x * Illuminant::D50Xyz[0];
        v[1] = y * Illuminant::D50Xyz[1];
        v[2] = z * Illuminant::D50Xyz[2];
    }
    fn from_xyz_in_place(&self, v: &mut [f32; 3]) {
        let x = v[0] / Illuminant::D50Xyz[0];
        let y = v[1] / Illuminant::D50Xyz[1];
        let z = v[2] / Illuminant::D50Xyz[2];
        let fx = if x > Lab::A { Lab::cbrt(x) } else { Lab::B * x + Lab::C };
        let fy = if y > Lab::A { Lab::cbrt(y) } else { Lab::B * y + Lab::C };
        let fz = if z > Lab::A { Lab::cbrt(z) } else { Lab::B * z + Lab::C };
        let l = 116.0 * fy - 16.0;
        let a = 500.0 * (fx - fy);
        let b = 200.0 * (fy - fz);
        v[0] = fast_coerce_in(l, 0.0, 100.0);
        v[1] = fast_coerce_in(a, -128.0, 128.0);
        v[2] = fast_coerce_in(b, -128.0, 128.0);
    }
    fn as_dyn(&self) -> &dyn ColorSpace {
        self
    }
    fn equals(&self, other: &dyn ColorSpace) -> bool {
        base_equals(self, other)
    }
}

/// Oklab, D65, through CIE XYZ D50 (Compose's Oklab): L 0..1, a and b -0.5..0.5.
pub struct Oklab {
    name: &'static str,
    id: i32,
}

impl Oklab {
    /// Before the nonlinear step: D50 to D65 (Bradford), then XYZ to LMS.
    const M1: Matrix = {
        let mut d50 = Illuminant::D50.to_xyz();
        let mut d65 = Illuminant::D65.to_xyz();
        mul3x3(
            &[
                0.8189330101,
                0.0329845436,
                0.0482003018,
                0.3618667424,
                0.9293118715,
                0.2643662691,
                -0.1288597137,
                0.0361456387,
                0.6338517070,
            ],
            &chromatic_adaptation(&Adaptation::Bradford.transform, &mut d50, &mut d65),
        )
    };
    /// After it.
    const M2: Matrix = [
        0.2104542553,
        1.9779984951,
        0.0259040371,
        0.7936177850,
        -2.4285922050,
        0.7827717662,
        -0.0040720468,
        0.4505937099,
        -0.8086757660,
    ];
    const INVERSE_M1: Matrix = inverse3x3(&Oklab::M1);
    const INVERSE_M2: Matrix = inverse3x3(&Oklab::M2);
}

impl ColorSpace for Oklab {
    fn name(&self) -> &str {
        self.name
    }
    fn model(&self) -> ColorModel {
        ColorModel::Lab
    }
    fn id(&self) -> i32 {
        self.id
    }
    fn is_wide_gamut(&self) -> bool {
        true
    }
    fn get_min_value(&self, component: usize) -> f32 {
        if component == 0 { 0.0 } else { -0.5 }
    }
    fn get_max_value(&self, component: usize) -> f32 {
        if component == 0 { 1.0 } else { 0.5 }
    }
    fn to_xyz_in_place(&self, v: &mut [f32; 3]) {
        v[0] = fast_coerce_in(v[0], 0.0, 1.0);
        v[1] = fast_coerce_in(v[1], -0.5, 0.5);
        v[2] = fast_coerce_in(v[2], -0.5, 0.5);
        mul3x3_float3(&Oklab::INVERSE_M2, v);
        for c in v.iter_mut() {
            *c = *c * *c * *c;
        }
        mul3x3_float3(&Oklab::INVERSE_M1, v);
    }
    fn from_xyz_in_place(&self, v: &mut [f32; 3]) {
        mul3x3_float3(&Oklab::M1, v);
        for c in v.iter_mut() {
            *c = fast_cbrt(*c);
        }
        mul3x3_float3(&Oklab::M2, v);
    }
    fn as_dyn(&self) -> &dyn ColorSpace {
        self
    }
    fn equals(&self, other: &dyn ColorSpace) -> bool {
        base_equals(self, other)
    }
}

// ---- Rgb --------------------------------------------------------------------------------------------------------

/// An RGB color space (Compose's Rgb): three primaries and a white point (in CIE xyY, or the primaries in CIE
/// XYZ: 6 or 9 floats), or the RGB to CIE XYZ matrix they make, and its transfer functions: the OETF (linear to
/// encoded) and the EOTF (encoded to linear), given, from [`TransferParameters`] or a gamma. Its components are
/// within min..max (0..1 unless given). The constructors panic where Compose throws: an empty name, primaries of
/// another size, min not under max, bad transfer parameters.
#[derive(Clone)]
pub struct Rgb {
    name: Cow<'static, str>,
    id: i32,
    white_point: WhitePoint,
    min: f32,
    max: f32,
    transfer_parameters: Option<TransferParameters>,
    primaries: [f32; 6],
    transform: Matrix,
    inverse_transform: Matrix,
    oetf_orig: Curve,
    eotf_orig: Curve,
    is_wide_gamut: bool,
    is_srgb: OnceLock<bool>,
}

fn check_name_and_id(name: &str, id: i32) {
    if name.is_empty() {
        panic!("The name of a color space cannot be null and must contain at least 1 character");
    }
    if !(MIN_ID..=MAX_ID).contains(&id) {
        panic!("The id must be between {MIN_ID} and {MAX_ID}");
    }
}

fn check_primaries_and_range(primaries: &[f32], min: f32, max: f32) {
    if primaries.len() != 6 && primaries.len() != 9 {
        panic!("The color space's primaries must be defined as an array of 6 floats in xyY or 9 floats in XYZ");
    }
    if min >= max {
        panic!("Invalid range: min={min}, max={max}; min must be strictly < max");
    }
}

impl Rgb {
    /// Every constructor's end: the matrices and what the primaries and range make (checked already).
    #[allow(clippy::too_many_arguments)]
    const fn make(
        name: Cow<'static, str>,
        primaries: &[f32],
        white_point: WhitePoint,
        transform: Option<&Matrix>,
        oetf: Curve,
        eotf: Curve,
        min: f32,
        max: f32,
        transfer_parameters: Option<TransferParameters>,
        id: i32,
    ) -> Rgb {
        let primaries = xy_primaries(primaries);
        let transform = match transform {
            Some(t) => *t,
            None => compute_xyz_matrix(&primaries, white_point),
        };
        Rgb {
            name,
            id,
            white_point,
            min,
            max,
            transfer_parameters,
            inverse_transform: inverse3x3(&transform),
            transform,
            is_wide_gamut: is_wide_gamut(&primaries, min, max),
            primaries,
            oetf_orig: oetf,
            eotf_orig: eotf,
            is_srgb: OnceLock::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn checked(
        name: impl Into<Cow<'static, str>>,
        primaries: &[f32],
        white_point: WhitePoint,
        transform: Option<&Matrix>,
        oetf: Curve,
        eotf: Curve,
        min: f32,
        max: f32,
        transfer_parameters: Option<TransferParameters>,
        id: i32,
    ) -> Rgb {
        let name = name.into();
        check_name_and_id(&name, id);
        check_primaries_and_range(primaries, min, max);
        Rgb::make(name, primaries, white_point, transform, oetf, eotf, min, max, transfer_parameters, id)
    }

    /// A built-in space's.
    #[allow(clippy::too_many_arguments)]
    const fn known(
        name: &'static str,
        primaries: &[f32],
        white_point: WhitePoint,
        oetf: Curve,
        eotf: Curve,
        range: (f32, f32),
        transfer_parameters: Option<TransferParameters>,
        id: i32,
    ) -> Rgb {
        Rgb::make(
            Cow::Borrowed(name),
            primaries,
            white_point,
            None,
            oetf,
            eotf,
            range.0,
            range.1,
            transfer_parameters,
            id,
        )
    }

    const fn known_parametric(
        name: &'static str,
        primaries: &[f32],
        white_point: WhitePoint,
        p: TransferParameters,
        id: i32,
    ) -> Rgb {
        Rgb::known(
            name,
            primaries,
            white_point,
            Curve::ParametricOetf(p),
            Curve::ParametricEotf(p),
            (0.0, 1.0),
            Some(p),
            id,
        )
    }

    const fn known_gamma(
        name: &'static str,
        primaries: &[f32],
        white_point: WhitePoint,
        gamma: f64,
        range: (f32, f32),
        id: i32,
    ) -> Rgb {
        let (oetf, eotf) = if gamma == 1.0 {
            (Curve::Identity, Curve::Identity)
        } else {
            (Curve::GammaOetf(gamma), Curve::GammaEotf(gamma))
        };
        Rgb::known(
            name,
            primaries,
            white_point,
            oetf,
            eotf,
            range,
            Some(TransferParameters::known(gamma, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0)),
            id,
        )
    }

    /// From the RGB to CIE XYZ matrix and the transfer functions, 0..1.
    pub fn new(
        name: impl Into<Cow<'static, str>>,
        to_xyz: &[f32; 9],
        oetf: impl Fn(f64) -> f64 + Send + Sync + 'static,
        eotf: impl Fn(f64) -> f64 + Send + Sync + 'static,
    ) -> Rgb {
        let primaries = compute_primaries(to_xyz);
        let white_point = compute_white_point(to_xyz);
        Rgb::checked(
            name,
            &primaries,
            white_point,
            None,
            Curve::custom(oetf),
            Curve::custom(eotf),
            0.0,
            1.0,
            None,
            MIN_ID,
        )
    }

    /// From primaries (6 floats in CIE xyY or 9 in CIE XYZ), a white point, the transfer functions and a range.
    #[allow(clippy::too_many_arguments)]
    pub fn from_primaries(
        name: impl Into<Cow<'static, str>>,
        primaries: &[f32],
        white_point: WhitePoint,
        oetf: impl Fn(f64) -> f64 + Send + Sync + 'static,
        eotf: impl Fn(f64) -> f64 + Send + Sync + 'static,
        min: f32,
        max: f32,
    ) -> Rgb {
        Rgb::checked(
            name,
            primaries,
            white_point,
            None,
            Curve::custom(oetf),
            Curve::custom(eotf),
            min,
            max,
            None,
            MIN_ID,
        )
    }

    /// From the RGB to CIE XYZ matrix and the transfer parameters.
    pub fn with_parameters(name: impl Into<Cow<'static, str>>, to_xyz: &[f32; 9], function: TransferParameters) -> Rgb {
        Rgb::from_primaries_with_parameters(name, &compute_primaries(to_xyz), compute_white_point(to_xyz), function)
    }

    /// From primaries, a white point and the transfer parameters.
    pub fn from_primaries_with_parameters(
        name: impl Into<Cow<'static, str>>,
        primaries: &[f32],
        white_point: WhitePoint,
        function: TransferParameters,
    ) -> Rgb {
        let (oetf, eotf) = (Curve::ParametricOetf(function), Curve::ParametricEotf(function));
        Rgb::checked(name, primaries, white_point, None, oetf, eotf, 0.0, 1.0, Some(function), MIN_ID)
    }

    /// From the RGB to CIE XYZ matrix and a gamma.
    pub fn with_gamma(name: impl Into<Cow<'static, str>>, to_xyz: &[f32; 9], gamma: f64) -> Rgb {
        Rgb::from_primaries_with_gamma(name, &compute_primaries(to_xyz), compute_white_point(to_xyz), gamma)
    }

    /// From primaries, a white point and a gamma.
    pub fn from_primaries_with_gamma(
        name: impl Into<Cow<'static, str>>,
        primaries: &[f32],
        white_point: WhitePoint,
        gamma: f64,
    ) -> Rgb {
        let parameters = TransferParameters::new(gamma, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let (oetf, eotf) = if gamma == 1.0 {
            (Curve::Identity, Curve::Identity)
        } else {
            (Curve::GammaOetf(gamma), Curve::GammaEotf(gamma))
        };
        Rgb::checked(name, primaries, white_point, None, oetf, eotf, 0.0, 1.0, Some(parameters), MIN_ID)
    }

    /// `color_space` with another white point and the matrix adapted to it (Compose's internal constructor, adapt's).
    fn adapted(color_space: &Rgb, transform: &Matrix, white_point: WhitePoint) -> Rgb {
        Rgb::make(
            color_space.name.clone(),
            &color_space.primaries,
            white_point,
            Some(transform),
            color_space.oetf_orig.clone(),
            color_space.eotf_orig.clone(),
            color_space.min,
            color_space.max,
            color_space.transfer_parameters,
            MIN_ID,
        )
    }

    pub fn white_point(&self) -> WhitePoint {
        self.white_point
    }
    /// In CIE xyY.
    pub fn primaries(&self) -> [f32; 6] {
        self.primaries
    }
    /// RGB (linear) to CIE XYZ.
    pub fn transform(&self) -> [f32; 9] {
        self.transform
    }
    /// CIE XYZ to RGB (linear).
    pub fn inverse_transform(&self) -> [f32; 9] {
        self.inverse_transform
    }
    pub fn transfer_parameters(&self) -> Option<TransferParameters> {
        self.transfer_parameters
    }

    /// The OETF, its result kept within min..max.
    pub fn oetf(&self, x: f64) -> f64 {
        coerce_in(self.oetf_orig.apply(x), self.min as f64, self.max as f64)
    }
    /// The EOTF, its argument kept within min..max.
    pub fn eotf(&self, x: f64) -> f64 {
        self.eotf_orig.apply(coerce_in(x, self.min as f64, self.max as f64))
    }

    /// Through the EOTF.
    pub fn to_linear(&self, r: f32, g: f32, b: f32) -> [f32; 3] {
        let mut v = [r, g, b];
        self.to_linear_in_place(&mut v);
        v
    }
    pub fn to_linear_in_place(&self, v: &mut [f32; 3]) {
        for c in v {
            *c = self.eotf(*c as f64) as f32;
        }
    }
    /// Through the OETF.
    pub fn from_linear(&self, r: f32, g: f32, b: f32) -> [f32; 3] {
        let mut v = [r, g, b];
        self.from_linear_in_place(&mut v);
        v
    }
    pub fn from_linear_in_place(&self, v: &mut [f32; 3]) {
        for c in v {
            *c = self.oetf(*c as f64) as f32;
        }
    }

    /// sRGB: its id, or sRGB's primaries, white point and range, its transfer functions sRGB's within 1e-3.
    fn compute_is_srgb(&self) -> bool {
        if self.id == 0 {
            return true;
        }
        if !compare_arrays(&self.primaries, &SRGB_PRIMARIES) || !compare_white_points(self.white_point, Illuminant::D65)
        {
            return false;
        }
        if self.min != 0.0 || self.max != 1.0 {
            return false;
        }
        let s = &ColorSpaces::Srgb;
        let mut x = 0.0;
        while x <= 1.0 {
            let oetf = (self.oetf_orig.apply(x) - s.oetf_orig.apply(x)).abs();
            let eotf = (self.eotf_orig.apply(x) - s.eotf_orig.apply(x)).abs();
            if oetf.is_nan() || oetf > 1e-3 || eotf.is_nan() || eotf > 1e-3 {
                return false;
            }
            x += 1.0 / 255.0;
        }
        true
    }
}

impl ColorSpace for Rgb {
    fn name(&self) -> &str {
        &self.name
    }
    fn model(&self) -> ColorModel {
        ColorModel::Rgb
    }
    fn id(&self) -> i32 {
        self.id
    }
    /// A wide gamut: an area over 90% of NTSC 1953's holding all of sRGB's, or a range past 0..1 both ways.
    fn is_wide_gamut(&self) -> bool {
        self.is_wide_gamut
    }
    fn is_srgb(&self) -> bool {
        *self.is_srgb.get_or_init(|| self.compute_is_srgb())
    }
    fn get_min_value(&self, _component: usize) -> f32 {
        self.min
    }
    fn get_max_value(&self, _component: usize) -> f32 {
        self.max
    }
    fn to_xyz_in_place(&self, v: &mut [f32; 3]) {
        self.to_linear_in_place(v);
        mul3x3_float3(&self.transform, v);
    }
    fn from_xyz_in_place(&self, v: &mut [f32; 3]) {
        mul3x3_float3(&self.inverse_transform, v);
        self.from_linear_in_place(v);
    }
    fn as_rgb(&self) -> Option<&Rgb> {
        Some(self)
    }
    fn as_dyn(&self) -> &dyn ColorSpace {
        self
    }
    fn equals(&self, other: &dyn ColorSpace) -> bool {
        if same_space(self, other) {
            return true;
        }
        if !base_equals(self, other) {
            return false;
        }
        let rgb = other.as_rgb().expect("an Rgb");
        if !same_float(rgb.min, self.min) || !same_float(rgb.max, self.max) {
            return false;
        }
        if self.white_point != rgb.white_point {
            return false;
        }
        if !self.primaries.iter().zip(&rgb.primaries).all(|(&a, &b)| same_float(a, b)) {
            return false;
        }
        if let Some(p) = &self.transfer_parameters {
            return rgb.transfer_parameters.as_ref() == Some(p);
        }
        if rgb.transfer_parameters.is_none() {
            return true;
        }
        self.oetf_orig.same(&rgb.oetf_orig) && self.eotf_orig.same(&rgb.eotf_orig)
    }
}

// ---- Connectors -------------------------------------------------------------------------------------------------

enum ConnectorKind<'a> {
    /// A space to itself (Compose's Connector.identity): colors as they are.
    Identity,
    /// Two RGB spaces: linear, one matrix, encoded (Compose's Connector.RgbConnector).
    Rgb { source: &'a Rgb, destination: &'a Rgb, transform: Matrix },
    /// Any two spaces, through CIE XYZ D50 (Compose's Connector): an RGB one adapted to D50 first.
    Xyz { transform_source: Adapted<'a>, transform_destination: Adapted<'a>, transform: Option<Float3> },
}

struct ConnectorImpl<'a> {
    source: &'a dyn ColorSpace,
    destination: &'a dyn ColorSpace,
    render_intent: RenderIntent,
    kind: ConnectorKind<'a>,
}

impl<'a> ConnectorImpl<'a> {
    fn identity(space: &'a dyn ColorSpace) -> ConnectorImpl<'a> {
        ConnectorImpl {
            source: space,
            destination: space,
            render_intent: RenderIntent::Relative,
            kind: ConnectorKind::Identity,
        }
    }

    fn rgb(source: &'a Rgb, destination: &'a Rgb, intent: RenderIntent) -> ConnectorImpl<'a> {
        let transform = if compare_white_points(source.white_point, destination.white_point) {
            mul3x3(&destination.inverse_transform, &source.transform)
        } else {
            // Through CIE XYZ D50.
            let mut transform = source.transform;
            let mut inverse_transform = destination.inverse_transform;
            let mut src_xyz = source.white_point.to_xyz();
            let mut dst_xyz = destination.white_point.to_xyz();
            if !compare_white_points(source.white_point, Illuminant::D50) {
                let mut d50 = Illuminant::D50Xyz;
                transform = mul3x3(
                    &chromatic_adaptation(&Adaptation::Bradford.transform, &mut src_xyz, &mut d50),
                    &source.transform,
                );
            }
            if !compare_white_points(destination.white_point, Illuminant::D50) {
                let mut d50 = Illuminant::D50Xyz;
                inverse_transform = inverse3x3(&mul3x3(
                    &chromatic_adaptation(&Adaptation::Bradford.transform, &mut dst_xyz, &mut d50),
                    &destination.transform,
                ));
            }
            // (src_xyz and dst_xyz in LMS when adapted above, as Compose's.)
            if intent == RenderIntent::Absolute {
                transform = mul3x3_diag(
                    &[src_xyz[0] / dst_xyz[0], src_xyz[1] / dst_xyz[1], src_xyz[2] / dst_xyz[2]],
                    &transform,
                );
            }
            mul3x3(&inverse_transform, &transform)
        };
        ConnectorImpl {
            source,
            destination,
            render_intent: intent,
            kind: ConnectorKind::Rgb { source, destination, transform },
        }
    }

    fn xyz(source: &'a dyn ColorSpace, destination: &'a dyn ColorSpace, intent: RenderIntent) -> ConnectorImpl<'a> {
        let adapt = |space: &'a dyn ColorSpace| {
            if space.model() == ColorModel::Rgb { space.adapt(Illuminant::D50) } else { Adapted::Same(space) }
        };
        // Absolute, between an RGB space and another: from one white point to the other.
        let transform = if intent != RenderIntent::Absolute {
            None
        } else {
            let (src_rgb, dst_rgb) = (source.model() == ColorModel::Rgb, destination.model() == ColorModel::Rgb);
            if src_rgb == dst_rgb {
                None
            } else {
                let rgb = if src_rgb { source } else { destination }.as_rgb().expect("an Rgb");
                let src_xyz = if src_rgb { rgb.white_point.to_xyz() } else { Illuminant::D50Xyz };
                let dst_xyz = if dst_rgb { rgb.white_point.to_xyz() } else { Illuminant::D50Xyz };
                Some([src_xyz[0] / dst_xyz[0], src_xyz[1] / dst_xyz[1], src_xyz[2] / dst_xyz[2]])
            }
        };
        ConnectorImpl {
            source,
            destination,
            render_intent: intent,
            kind: ConnectorKind::Xyz {
                transform_source: adapt(source),
                transform_destination: adapt(destination),
                transform,
            },
        }
    }

    fn create(source: &'a dyn ColorSpace, destination: &'a dyn ColorSpace, intent: RenderIntent) -> ConnectorImpl<'a> {
        if same_space(source, destination) {
            return ConnectorImpl::identity(source);
        }
        match (source.as_rgb(), destination.as_rgb()) {
            (Some(s), Some(d)) => ConnectorImpl::rgb(s, d, intent),
            _ => ConnectorImpl::xyz(source, destination, intent),
        }
    }

    fn transform(&self, v: &mut Float3) {
        match &self.kind {
            ConnectorKind::Identity => {}
            ConnectorKind::Rgb { source, destination, transform } => {
                source.to_linear_in_place(v);
                mul3x3_float3(transform, v);
                destination.from_linear_in_place(v);
            }
            ConnectorKind::Xyz { transform_source, transform_destination, transform } => {
                transform_source.to_xyz_in_place(v);
                if let Some(t) = transform {
                    v[0] *= t[0];
                    v[1] *= t[1];
                    v[2] *= t[2];
                }
                transform_destination.from_xyz_in_place(v);
            }
        }
    }
}

/// Turns colors from one color space into another (Compose's Connector), from [`ColorSpace::connect`]. A handle:
/// clones are the same connector. Two built-in spaces' is kept for good.
#[derive(Clone)]
pub struct Connector<'a> {
    inner: Arc<ConnectorImpl<'a>>,
}

impl<'a> Connector<'a> {
    pub fn source(&self) -> &'a dyn ColorSpace {
        self.inner.source
    }
    pub fn destination(&self) -> &'a dyn ColorSpace {
        self.inner.destination
    }
    pub fn render_intent(&self) -> RenderIntent {
        self.inner.render_intent
    }
    /// A color of the source in the destination.
    pub fn transform(&self, r: f32, g: f32, b: f32) -> [f32; 3] {
        let mut v = [r, g, b];
        self.inner.transform(&mut v);
        v
    }
    pub fn transform_in_place(&self, v: &mut [f32; 3]) {
        self.inner.transform(v);
    }
}

/// The same connector (not merely an equal one).
impl PartialEq for Connector<'_> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

fn connector_key(src: i32, dst: i32, intent: RenderIntent) -> i32 {
    src | dst << 6 | (intent as i32) << 12
}

/// The built-in spaces' connectors, by connector_key, kept for good (Compose's Connectors).
static CONNECTORS: LazyLock<Mutex<HashMap<i32, Arc<ConnectorImpl<'static>>>>> = LazyLock::new(|| {
    let (s, o): (&'static dyn ColorSpace, &'static dyn ColorSpace) = (&ColorSpaces::Srgb, &ColorSpaces::Oklab);
    let mut map = HashMap::new();
    map.insert(connector_key(0, 0, RenderIntent::Perceptual), Arc::new(ConnectorImpl::identity(s)));
    map.insert(
        connector_key(0, 19, RenderIntent::Perceptual),
        Arc::new(ConnectorImpl::xyz(s, o, RenderIntent::Perceptual)),
    );
    map.insert(
        connector_key(19, 0, RenderIntent::Perceptual),
        Arc::new(ConnectorImpl::xyz(o, s, RenderIntent::Perceptual)),
    );
    Mutex::new(map)
});

fn connect<'a>(source: &'a dyn ColorSpace, destination: &'a dyn ColorSpace, intent: RenderIntent) -> Connector<'a> {
    let (src, dst) = (source.id(), destination.id());
    if (src | dst) < 0 {
        // A user's space: not cached.
        return Connector { inner: Arc::new(ConnectorImpl::create(source, destination, intent)) };
    }
    let mut cache = CONNECTORS.lock().unwrap_or_else(|e| e.into_inner());
    let inner = cache
        .entry(connector_key(src, dst, intent))
        .or_insert_with(|| {
            Arc::new(ConnectorImpl::create(
                ColorSpaces::get_color_space(src),
                ColorSpaces::get_color_space(dst),
                intent,
            ))
        })
        .clone();
    Connector { inner }
}

// ---- ColorSpaces ------------------------------------------------------------------------------------------------

/// The built-in color spaces, by their ids (Compose's ColorSpaces: the ids are kept in colors, they cannot change).
pub mod ColorSpaces {
    use super::*;

    /// sRGB IEC61966-2.1.
    pub static Srgb: Rgb =
        Rgb::known_parametric("sRGB IEC61966-2.1", &SRGB_PRIMARIES, Illuminant::D65, SRGB_TRANSFER_PARAMETERS, 0);
    /// sRGB, linear.
    pub static LinearSrgb: Rgb =
        Rgb::known_gamma("sRGB IEC61966-2.1 (Linear)", &SRGB_PRIMARIES, Illuminant::D65, 1.0, (0.0, 1.0), 1);
    /// scRGB-nl IEC 61966-2-2:2003: sRGB's curve both ways from 0.
    pub static ExtendedSrgb: Rgb = Rgb::known(
        "scRGB-nl IEC 61966-2-2:2003",
        &SRGB_PRIMARIES,
        Illuminant::D65,
        Curve::AbsParametricOetf(SRGB_TRANSFER_PARAMETERS),
        Curve::AbsParametricEotf(SRGB_TRANSFER_PARAMETERS),
        (-0.799, 2.399),
        Some(SRGB_TRANSFER_PARAMETERS),
        2,
    );
    /// scRGB IEC 61966-2-2:2003.
    pub static LinearExtendedSrgb: Rgb =
        Rgb::known_gamma("scRGB IEC 61966-2-2:2003", &SRGB_PRIMARIES, Illuminant::D65, 1.0, (-0.5, 7.499), 3);
    pub static Bt709: Rgb =
        Rgb::known_parametric("Rec. ITU-R BT.709-5", &SRGB_PRIMARIES, Illuminant::D65, BT709_TRANSFER_PARAMETERS, 4);
    pub static Bt2020: Rgb = Rgb::known_parametric(
        "Rec. ITU-R BT.2020-1",
        &BT2020_PRIMARIES,
        Illuminant::D65,
        TransferParameters::known(1.0 / 0.45, 1.0 / 1.0993, 0.0993 / 1.0993, 1.0 / 4.5, 0.08145, 0.0, 0.0),
        5,
    );
    pub static DciP3: Rgb = Rgb::known_gamma(
        "SMPTE RP 431-2-2007 DCI (P3)",
        &DCI_P3_PRIMARIES,
        WhitePoint::new(0.314, 0.351),
        2.6,
        (0.0, 1.0),
        6,
    );
    pub static DisplayP3: Rgb =
        Rgb::known_parametric("Display P3", &DCI_P3_PRIMARIES, Illuminant::D65, SRGB_TRANSFER_PARAMETERS, 7);
    pub static Ntsc1953: Rgb =
        Rgb::known_parametric("NTSC (1953)", &NTSC_1953_PRIMARIES, Illuminant::C, BT709_TRANSFER_PARAMETERS, 8);
    pub static SmpteC: Rgb = Rgb::known_parametric(
        "SMPTE-C RGB",
        &[0.630, 0.340, 0.310, 0.595, 0.155, 0.070],
        Illuminant::D65,
        BT709_TRANSFER_PARAMETERS,
        9,
    );
    pub static AdobeRgb: Rgb = Rgb::known_gamma(
        "Adobe RGB (1998)",
        &[0.64, 0.33, 0.21, 0.71, 0.15, 0.06],
        Illuminant::D65,
        2.2,
        (0.0, 1.0),
        10,
    );
    pub static ProPhotoRgb: Rgb = Rgb::known_parametric(
        "ROMM RGB ISO 22028-2:2013",
        &[0.7347, 0.2653, 0.1596, 0.8404, 0.0366, 0.0001],
        Illuminant::D50,
        TransferParameters::known(1.8, 1.0, 0.0, 1.0 / 16.0, 0.031248, 0.0, 0.0),
        11,
    );
    pub static Aces: Rgb = Rgb::known_gamma(
        "SMPTE ST 2065-1:2012 ACES",
        &[0.73470, 0.26530, 0.0, 1.0, 0.00010, -0.0770],
        Illuminant::D60,
        1.0,
        (-65504.0, 65504.0),
        12,
    );
    pub static Acescg: Rgb = Rgb::known_gamma(
        "Academy S-2014-004 ACEScg",
        &[0.713, 0.293, 0.165, 0.830, 0.128, 0.044],
        Illuminant::D60,
        1.0,
        (-65504.0, 65504.0),
        13,
    );
    /// D50, -2..2.
    pub static CieXyz: Xyz = Xyz { name: "Generic XYZ", id: 14 };
    /// D50, L 0..100, a and b -128..128.
    pub static CieLab: Lab = Lab { name: "Generic L*a*b*", id: 15 };
    /// No color's (internal in Compose).
    pub static Unspecified: Rgb =
        Rgb::known_parametric("None", &SRGB_PRIMARIES, Illuminant::D65, NONE_TRANSFER_PARAMETERS, 16);
    /// BT.2100, Hybrid Log Gamma.
    pub static Bt2020Hlg: Rgb = Rgb::known(
        "Hybrid Log Gamma encoding",
        &BT2020_PRIMARIES,
        Illuminant::D65,
        Curve::ParametricOetf(BT2020_HLG_TRANSFER_PARAMETERS),
        Curve::ParametricEotf(BT2020_HLG_TRANSFER_PARAMETERS),
        (0.0, 1.0),
        Some(BT2020_HLG_TRANSFER_PARAMETERS),
        17,
    );
    /// BT.2100, Perceptual Quantizer.
    pub static Bt2020Pq: Rgb = Rgb::known(
        "Perceptual Quantizer encoding",
        &BT2020_PRIMARIES,
        Illuminant::D65,
        Curve::ParametricOetf(BT2020_PQ_TRANSFER_PARAMETERS),
        Curve::ParametricEotf(BT2020_PQ_TRANSFER_PARAMETERS),
        (0.0, 1.0),
        Some(BT2020_PQ_TRANSFER_PARAMETERS),
        18,
    );
    /// D65, L 0..1, a and b -0.5..0.5.
    pub static Oklab: super::Oklab = super::Oklab { name: "Oklab", id: 19 };

    /// The built-in color space of an id, 0..19 (Compose's getColorSpace); panics for another.
    pub fn get_color_space(id: i32) -> &'static dyn ColorSpace {
        match id {
            0 => &Srgb,
            1 => &LinearSrgb,
            2 => &ExtendedSrgb,
            3 => &LinearExtendedSrgb,
            4 => &Bt709,
            5 => &Bt2020,
            6 => &DciP3,
            7 => &DisplayP3,
            8 => &Ntsc1953,
            9 => &SmpteC,
            10 => &AdobeRgb,
            11 => &ProPhotoRgb,
            12 => &Aces,
            13 => &Acescg,
            14 => &CieXyz,
            15 => &CieLab,
            16 => &Unspecified,
            17 => &Bt2020Hlg,
            18 => &Bt2020Pq,
            19 => &Oklab,
            _ => panic!("No color space of id {id}"),
        }
    }

    /// The built-in RGB space whose matrix, adapted to D50, and transfer parameters are these within 1e-3
    /// (Compose's match), or none.
    pub fn r#match(to_xyz_d50: &[f32; 9], function: &TransferParameters) -> Option<&'static dyn ColorSpace> {
        (0..20).map(get_color_space).find(|space| {
            if space.model() != ColorModel::Rgb {
                return false;
            }
            let adapted = space.adapt(Illuminant::D50);
            let rgb = adapted.as_rgb().expect("an Rgb");
            compare_arrays(to_xyz_d50, &rgb.transform) && compare_parameters(function, rgb.transfer_parameters.as_ref())
        })
    }
}
