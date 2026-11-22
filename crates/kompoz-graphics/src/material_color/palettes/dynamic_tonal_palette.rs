//! The system's palette as Android 12+ gives it (Material You), what Compose's dynamicTonalPalette reads, and how
//! Kompoz makes one where the system has none: from a seed color, or from a picture as Android does from the
//! wallpaper.

// (Kompoz's literals as it writes them.)
#![allow(clippy::excessive_precision)]

use crate::color::Color;
use crate::material_color::hct::{Hct, cam_to_color};
use crate::material_color::quantize::QuantizerCelebi;
use crate::material_color::utils::math_utils::sanitize_degrees_double;
use crate::material_color::wallpaper::{GOOGLE_BLUE, scale_to_extraction_area, seed_colors};
use kompoz_runtime::KotlinProperties;

/// The system's color palettes where it derives them from the wallpaper (Android 12+, "Material You"; its
/// system_accent1_0 … system_neutral2_1000): five key colors in 13 tones each, from tone 100 (white) through 99,
/// 95, 90, 80, …, 10 down to 0 (black). What Compose's dynamicLightColorScheme and dynamicDarkColorScheme read.
#[derive(Clone, Copy, Debug, PartialEq, KotlinProperties)]
pub struct DynamicTonalPalette {
    pub accent1: [Color; 13],
    pub accent2: [Color; 13],
    pub accent3: [Color; 13],
    pub neutral1: [Color; 13],
    pub neutral2: [Color; 13],
}

impl DynamicTonalPalette {
    /// The tones of each key color's 13, in order.
    pub const TONES: [i32; 13] = [100, 99, 95, 90, 80, 70, 60, 50, 40, 30, 20, 10, 0];

    /// The palettes Material You makes from one color (Android 13+'s "tonal spot"): its hue at chroma 36, 16
    /// (accent2), 24 turned 60° (accent3), 6 and 8 (the neutrals); each tone as Android's CAMToColor finds it
    /// ([`cam_to_color`]). On Linux the desktop's accent color (the XDG portal's) is turned into them.
    pub fn from_seed(seed: Color) -> DynamicTonalPalette {
        // (Kompoz keeps HCT's hue, chroma and tone as floats.)
        let hue = Hct::from_color(seed).hue() as f32 as f64;
        let palette = |h: f64, chroma: f64| {
            DynamicTonalPalette::TONES.map(|tone| Color::from_argb(cam_to_color(h as f32 as f64, chroma, tone as f64)))
        };
        DynamicTonalPalette {
            accent1: palette(hue, 36.0),
            accent2: palette(hue, 16.0),
            accent3: palette(sanitize_degrees_double(hue + 60.0), 24.0),
            neutral1: palette(hue, 6.0),
            neutral2: palette(hue, 8.0),
        }
    }

    /// Material You from a picture (ARGB pixels, `width` × `height`), as Android makes it from the wallpaper
    /// (WallpaperColors and System UI): [`from_seed`](DynamicTonalPalette::from_seed) of the picture's best seed
    /// ([`seed_colors`]' first) of its colors ([`QuantizerCelebi`], 128 of them), the picture first scaled to at
    /// most 112 × 112 of area without filtering, as Android does. An empty picture gives Google blue's.
    pub fn from_image(pixels: &[u32], width: usize, height: usize) -> DynamicTonalPalette {
        if pixels.is_empty() || width == 0 || height == 0 {
            return DynamicTonalPalette::from_seed(Color::from_argb(GOOGLE_BLUE));
        }
        let (picture, _, _) = scale_to_extraction_area(pixels, width, height);
        let colors = QuantizerCelebi::quantize(&picture, 128).color_to_count;
        DynamicTonalPalette::from_seed(Color::from_argb(seed_colors(&colors, 4)[0]))
    }
}

/// A tone (0 black .. 100 white, CIELAB lightness) of one of the palette's key colors: one of its 13 as it is, any
/// other the nearest one's color at that lightness, as Compose works out the tones the system does not give (98,
/// 94, 87, 6, …: its setLuminance).
pub fn palette_tone(key_color: &[Color; 13], tone: f32) -> Color {
    let mut nearest = 0;
    for i in 1..key_color.len() {
        if (DynamicTonalPalette::TONES[i] as f32 - tone).abs()
            < (DynamicTonalPalette::TONES[nearest] as f32 - tone).abs()
        {
            nearest = i;
        }
    }
    let c = key_color[nearest];
    if DynamicTonalPalette::TONES[nearest] as f32 == tone {
        return c;
    }
    let mut lab = to_lab(c);
    lab[0] = tone;
    from_lab(lab, c.alpha())
}

// sRGB <-> CIELAB (D65), in floats: a palette tone is the L* of a color.

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn from_linear(c: f32) -> f32 {
    if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

const XN: f32 = 0.95047;
const YN: f32 = 1.0;
const ZN: f32 = 1.08883;
const DELTA: f32 = 6.0 / 29.0;

fn lab_f(t: f32) -> f32 {
    if t > DELTA * DELTA * DELTA { t.cbrt() } else { t / (3.0 * DELTA * DELTA) + 4.0 / 29.0 }
}

fn lab_f_inverse(t: f32) -> f32 {
    if t > DELTA { t * t * t } else { 3.0 * DELTA * DELTA * (t - 4.0 / 29.0) }
}

fn to_lab(c: Color) -> [f32; 3] {
    let c = c.in_srgb();
    let (r, g, b) = (to_linear(c.red()), to_linear(c.green()), to_linear(c.blue()));
    let x = 0.4124564 * r + 0.3575761 * g + 0.1804375 * b;
    let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
    let z = 0.0193339 * r + 0.1191920 * g + 0.9503041 * b;
    let (fx, fy, fz) = (lab_f(x / XN), lab_f(y / YN), lab_f(z / ZN));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

fn from_lab(lab: [f32; 3], alpha: f32) -> Color {
    let fy = (lab[0] + 16.0) / 116.0;
    let fx = fy + lab[1] / 500.0;
    let fz = fy - lab[2] / 200.0;
    let (x, y, z) = (XN * lab_f_inverse(fx), YN * lab_f_inverse(fy), ZN * lab_f_inverse(fz));
    let r = 3.2404542 * x - 1.5371385 * y - 0.4985314 * z;
    let g = -0.9692660 * x + 1.8760108 * y + 0.0415560 * z;
    let b = 0.0556434 * x - 0.2040259 * y + 1.0572252 * z;
    let channel = |v: f32| from_linear(v.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    Color::new(channel(r), channel(g), channel(b), alpha)
}
