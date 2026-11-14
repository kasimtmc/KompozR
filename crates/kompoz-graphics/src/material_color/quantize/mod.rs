//! The colors a picture is made of (Material Color Utilities' `quantize`, as Kompoz runs it): every opaque
//! pixel counted ([`QuantizerMap`]), Wu's quantizer boxing them ([`QuantizerWu`]), its colors refined by weighted
//! k-means in L*a*b* ([`QuantizerWsmeans`]); together [`QuantizerCelebi`].

mod quantizer_wsmeans;
mod quantizer_wu;

pub use quantizer_wsmeans::QuantizerWsmeans;
pub use quantizer_wu::QuantizerWu;

use crate::material_color::utils::color_utils::{argb_from_lab, is_opaque, lab_f};
use std::collections::HashMap;

/// What a quantizer finds: each color (ARGB, opaque) and how many pixels went to it, the most first (and, as
/// many, by color).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuantizerResult {
    pub color_to_count: Vec<(u32, u32)>,
}

/// Every opaque pixel's color and how many there are of it (Material Color Utilities' QuantizerMap); pixels not
/// fully opaque are left out.
pub struct QuantizerMap;

impl QuantizerMap {
    pub fn quantize(pixels: &[u32]) -> HashMap<u32, u32> {
        let mut counts = HashMap::new();
        for &pixel in pixels {
            if is_opaque(pixel) {
                *counts.entry(pixel).or_insert(0) += 1;
            }
        }
        counts
    }
}

/// A picture's colors (Material Color Utilities' QuantizerCelebi): Wu's quantizer, its colors then refined by
/// k-means in L*a*b*. Up to `max_colors` (at most 256), each with how many pixels went to it, the most first.
/// Pixels not fully opaque are left out.
pub struct QuantizerCelebi;

impl QuantizerCelebi {
    pub fn quantize(pixels: &[u32], max_colors: i32) -> QuantizerResult {
        let max_colors = max_colors.min(256);
        if pixels.is_empty() || max_colors <= 0 {
            return QuantizerResult::default();
        }
        let counts = QuantizerMap::quantize(pixels);
        if counts.is_empty() {
            return QuantizerResult::default();
        }
        let wu = QuantizerWu::quantize(&counts, max_colors as usize);
        let clusters = QuantizerWsmeans::quantize(&counts, &wu, max_colors as usize);
        let mut color_to_count: Vec<(u32, u32)> = clusters.into_iter().collect();
        color_to_count.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        QuantizerResult { color_to_count }
    }
}

/// A color as the k-means measures it: L*a*b* (D65), its distance squared (Material Color Utilities'
/// PointProviderLab).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointProviderLab {
    pub l: f64,
    pub a: f64,
    pub b: f64,
}

impl PointProviderLab {
    /// Of an ARGB color, its channels taken as Kompoz's colors keep them (floats, n/255).
    pub fn from_int(argb: u32) -> PointProviderLab {
        let channel = |shift: u32| {
            let c = (((argb >> shift) & 0xff) as f32 / 255.0) as f64;
            100.0 * if c <= 0.040449936 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        let (r, g, b) = (channel(16), channel(8), channel(0));
        let x = 0.41233895 * r + 0.35762064 * g + 0.18051042 * b;
        let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        let z = 0.01932141 * r + 0.11916382 * g + 0.95034478 * b;
        let cbrt_f = |t: f64| if t > 216.0 / 24389.0 { t.cbrt() } else { lab_f(t) };
        let (fx, fy, fz) = (cbrt_f(x / 95.047), cbrt_f(y / 100.0), cbrt_f(z / 108.883));
        PointProviderLab { l: 116.0 * fy - 16.0, a: 500.0 * (fx - fy), b: 200.0 * (fy - fz) }
    }

    /// The ARGB color of the point, clipped to sRGB.
    pub fn to_int(&self) -> u32 {
        argb_from_lab(self.l, self.a, self.b)
    }

    /// The distance squared.
    pub fn distance(&self, o: &PointProviderLab) -> f64 {
        (self.l - o.l) * (self.l - o.l) + (self.a - o.a) * (self.a - o.a) + (self.b - o.b) * (self.b - o.b)
    }
}
