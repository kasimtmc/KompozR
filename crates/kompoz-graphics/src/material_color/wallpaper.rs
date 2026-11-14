//! A wallpaper's colors as Android picks them (android.app.WallpaperColors): the picture scaled down, its colors
//! quantized, its main colors and its hints (whether dark text and a dark theme suit it); and the theme seeds
//! System UI takes of them (monet's ColorScheme.getSeedColors). Pixels are ARGB (`0xAARRGGBB`).

use crate::color::Color;
use crate::material_color::hct::{Cam16, Hct};
use crate::material_color::quantize::QuantizerCelebi;
use crate::material_color::utils::math_utils::round;

/// Google blue (System UI's GOOGLE_BLUE): the seed when a picture has none.
pub const GOOGLE_BLUE: u32 = 0xff1b6ef3;

/// The most area a picture keeps for its colors to be picked: 112 × 112.
const MAX_WALLPAPER_EXTRACTION_AREA: usize = 112 * 112;

/// The picture scaled to at most 112 × 112 of area (its aspect kept), without filtering: each pixel the one under
/// its center, as Android's createScaledBitmap without a filter. Smaller pictures as they are.
pub fn scale_to_extraction_area(pixels: &[u32], width: usize, height: usize) -> (Vec<u32>, usize, usize) {
    let (w, h) = (width, height);
    if (w as f64) * (h as f64) <= MAX_WALLPAPER_EXTRACTION_AREA as f64 {
        return (pixels.to_vec(), w, h);
    }
    let scale = (MAX_WALLPAPER_EXTRACTION_AREA as f64 / (w as f64 * h as f64)).sqrt();
    let sw = ((w as f64 * scale) as usize).max(1);
    let sh = ((h as f64 * scale) as usize).max(1);
    let mut small = Vec::with_capacity(sw * sh);
    for y in 0..sh {
        let sy = (h - 1).min(((y as f64 + 0.5) * h as f64 / sh as f64) as usize);
        for x in 0..sw {
            let sx = (w - 1).min(((x as f64 + 0.5) * w as f64 / sw as f64) as usize);
            small.push(pixels[sy * w + sx]);
        }
    }
    (small, sw, sh)
}

/// Which of a picture's colors (each with how many pixels have it) make good theme seeds, best first, as Android's
/// System UI picks a wallpaper's (monet's ColorScheme.getSeedColors; Material Color Utilities' Score follows it):
/// each scored by its chroma (48 is best) and by how much of the picture is within 15° of its hue (its greys left
/// out), the greyish (chroma below 5) and the rare (1% or less) not taken, then picked as far apart in hue as
/// `desired` of them can be. When none is left: Google blue alone.
pub fn seed_colors(colors: &[(u32, u32)], desired: usize) -> Vec<u32> {
    const TARGET_CHROMA: f64 = 48.0; // ACCENT1_CHROMA
    const MIN_CHROMA: f64 = 5.0;
    let wrap = |degrees: i64| degrees.rem_euclid(360) as usize;
    let total: f64 = colors.iter().map(|&(_, n)| n as f64).sum();
    let hcts: Vec<Hct> = colors.iter().map(|&(argb, _)| Hct::from_int(argb)).collect();
    // The share of the picture of each hue, its greys left out.
    let mut hue_population = [0.0f64; 360];
    for (h, &(_, n)) in hcts.iter().zip(colors) {
        if total > 0.0 && h.chroma() > MIN_CHROMA {
            hue_population[wrap(h.hue().round() as i64)] += n as f64 / total;
        }
    }
    let mut scored: Vec<(Hct, f64)> = Vec::new();
    for h in &hcts {
        // The share of the picture within 15° of its hue; too little of it, or too grey: not a seed (raising a
        // grey's chroma would make a much louder color than the picture has).
        let hue = h.hue().round() as i64;
        let proportion: f64 = (hue - 15..=hue + 15).map(|i| hue_population[wrap(i)]).sum();
        if h.chroma() < MIN_CHROMA || proportion <= 0.01 {
            continue;
        }
        let chroma_score = if h.chroma() < TARGET_CHROMA {
            0.1 * (h.chroma() - TARGET_CHROMA)
        } else {
            0.3 * (h.chroma() - TARGET_CHROMA)
        };
        scored.push((*h, chroma_score + 0.7 * 100.0 * proportion));
    }
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    // As far apart in hue as `desired` of them can be: 90° apart first, down to 15°.
    let mut chosen: Vec<Hct> = Vec::new();
    for apart in (15..=90).rev() {
        chosen.clear();
        for (h, _) in &scored {
            let near = chosen.iter().any(|c| {
                let d = (h.hue() - c.hue()).abs();
                d.min(360.0 - d) < apart as f64
            });
            if !near {
                chosen.push(*h);
            }
            if chosen.len() >= desired {
                break;
            }
        }
        if chosen.len() >= desired {
            break;
        }
    }
    if chosen.is_empty() {
        return vec![GOOGLE_BLUE];
    }
    chosen.iter().map(Hct::to_int).collect()
}

/// A wallpaper's colors (Android's WallpaperColors): its colors and how much of it each is, its main colors (the
/// most noticeable, as far apart in hue as 15° at least) and its hints.
#[derive(Clone, Debug, PartialEq)]
pub struct WallpaperColors {
    main_colors: Vec<Color>,
    all_colors: Vec<(u32, u32)>,
    color_hints: i32,
}

impl WallpaperColors {
    /// Dark text is best over the wallpaper (a launcher may set its text color to black).
    pub const HINT_SUPPORTS_DARK_TEXT: i32 = 1 << 0;
    /// A dark theme is best with the wallpaper (a launcher may set its drawer color to black).
    pub const HINT_SUPPORTS_DARK_THEME: i32 = 1 << 1;
    /// Its colors were extracted from a picture.
    pub const HINT_FROM_BITMAP: i32 = 1 << 2;

    /// A dark theme suits a picture this dark (its mean luminance) or darker.
    const DARK_THEME_MEAN_LUMINANCE: f32 = 0.3;
    /// Dark text suits a picture this bright or brighter...
    const BRIGHT_IMAGE_MEAN_LUMINANCE: f32 = 70.0 / 100.0;
    /// ...with this much of it at most too dark for dark text (contrast with black under 5.5).
    const DARK_PIXEL_CONTRAST: f32 = 5.5;
    const MAX_DARK_AREA: f32 = 5.0 / 100.0;

    /// From a picture (ARGB pixels, `width` × `height`; Android's fromBitmap): scaled to at most 112 × 112 of area
    /// without filtering, its colors quantized (Celebi, 5 to 128 of them, a 16th of its area), its hints from the
    /// scaled picture.
    pub fn from_bitmap(pixels: &[u32], width: usize, height: usize) -> WallpaperColors {
        WallpaperColors::from_bitmap_dimmed(pixels, width, height, 0.0)
    }

    /// The same, the wallpaper dimmed by `dim_amount` (0..1) when the dark hints are worked out.
    pub fn from_bitmap_dimmed(pixels: &[u32], width: usize, height: usize, dim_amount: f32) -> WallpaperColors {
        let bitmap_area = width * height;
        let (picture, _, _) = scale_to_extraction_area(pixels, width, height);
        // In any case, between 5 and 128 clusters; a very small picture: its area / 16.
        let number_of_colors = 5.max(128.min(bitmap_area / 16));
        let quantized = QuantizerCelebi::quantize(&picture, number_of_colors as i32);
        // (Sorted by population already.)
        let hints = WallpaperColors::calculate_dark_hints(&picture, dim_amount);
        WallpaperColors::new(quantized.color_to_count, WallpaperColors::HINT_FROM_BITMAP | hints)
    }

    /// From colors and how much of the wallpaper each is, and hints: the main colors the best scored (chroma and
    /// how much of the wallpaper is within 15° of its hue), at least 15° apart.
    pub fn new(color_to_population: Vec<(u32, u32)>, color_hints: i32) -> WallpaperColors {
        let cams: Vec<Cam16> = color_to_population.iter().map(|&(c, _)| Cam16::from_int(c)).collect();
        let total: f64 = color_to_population.iter().map(|&(_, n)| n as f64).sum();
        let wrap = |degrees: i64| degrees.rem_euclid(360) as usize;
        let mut hue_proportions = [0.0f64; 360];
        for (cam, &(_, population)) in cams.iter().zip(&color_to_population) {
            hue_proportions[wrap(round(cam.hue()) as i64)] += population as f64 / total;
        }
        let mut scored: Vec<(usize, f64)> = cams
            .iter()
            .enumerate()
            .map(|(i, cam)| {
                let hue = wrap(round(cam.hue()) as i64) as i64;
                let proportion: f64 = (hue - 15..hue + 15).map(|h| hue_proportions[wrap(h)]).sum();
                (i, cam.chroma() + proportion * 100.0)
            })
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        let hue_diff = |a: &Cam16, b: &Cam16| 180.0 - ((a.hue() - b.hue()).abs() - 180.0).abs();
        let mut main: Vec<usize> = Vec::new();
        for &(i, _) in &scored {
            if main.iter().all(|&m| hue_diff(&cams[i], &cams[m]) >= 15.0) {
                main.push(i);
            }
        }
        let main_colors = main.iter().map(|&i| Color::from_argb(color_to_population[i].0)).collect();
        WallpaperColors { main_colors, all_colors: color_to_population, color_hints }
    }

    /// From three colors (the first given; the third only with the second), a dark theme hinted where the first
    /// is dark (its HSL lightness under 0.3).
    pub fn from_colors(primary: Color, secondary: Option<Color>, tertiary: Option<Color>) -> WallpaperColors {
        let mut colors = WallpaperColors::with_hints(primary, secondary, tertiary, 0);
        if hsl_lightness(primary.to_argb()) < WallpaperColors::DARK_THEME_MEAN_LUMINANCE {
            colors.color_hints |= WallpaperColors::HINT_SUPPORTS_DARK_THEME;
        }
        colors
    }

    /// From three colors and hints. Panics, as Android throws, for a tertiary color without a secondary one.
    pub fn with_hints(
        primary: Color,
        secondary: Option<Color>,
        tertiary: Option<Color>,
        color_hints: i32,
    ) -> WallpaperColors {
        if tertiary.is_some() && secondary.is_none() {
            panic!("tertiaryColor can't be specified when secondaryColor is null");
        }
        let main_colors: Vec<Color> = [Some(primary), secondary, tertiary].into_iter().flatten().collect();
        let all_colors = main_colors.iter().map(|c| (c.to_argb(), 0)).collect();
        WallpaperColors { main_colors, all_colors, color_hints }
    }

    /// The most noticeable color of the wallpaper.
    pub fn primary_color(&self) -> Color {
        self.main_colors[0]
    }
    pub fn secondary_color(&self) -> Option<Color> {
        self.main_colors.get(1).copied()
    }
    pub fn tertiary_color(&self) -> Option<Color> {
        self.main_colors.get(2).copied()
    }
    /// The most noticeable colors, the most first.
    pub fn main_colors(&self) -> &[Color] {
        &self.main_colors
    }
    /// Every color (ARGB) and how much of the wallpaper it is, the most first.
    pub fn all_colors(&self) -> &[(u32, u32)] {
        &self.all_colors
    }
    pub fn color_hints(&self) -> i32 {
        self.color_hints
    }

    /// Whether dark text suits the picture (bright enough, and too dark for it in 5% of it at most) and whether a
    /// dark theme does (its mean luminance under 0.3), the picture dimmed by `dim_amount` (a black layer over it).
    fn calculate_dark_hints(pixels: &[u32], dim_amount: f32) -> i32 {
        if pixels.is_empty() {
            return 0;
        }
        let dim_amount = dim_amount.clamp(0.0, 1.0);
        let mut total_luminance = 0.0f64;
        let max_dark_pixels = (pixels.len() as f32 * WallpaperColors::MAX_DARK_AREA) as usize;
        let mut dark_pixels = 0;
        // A black layer with the dim amount as its alpha, over each pixel, for its luminance.
        let dimming_layer_alpha = (255.0 * dim_amount) as u32;
        let black_transparent = dimming_layer_alpha << 24;
        for &pixel in pixels {
            let alpha = pixel >> 24;
            let adjusted_luminance = calculate_luminance(composite_colors(black_transparent, pixel));
            // Make sure there is no dark pixel mass that would make text illegible.
            let satisfies_text_contrast =
                calculate_contrast(pixel, 0xff000000) > WallpaperColors::DARK_PIXEL_CONTRAST as f64;
            if !satisfies_text_contrast && alpha != 0 {
                dark_pixels += 1;
            }
            total_luminance += adjusted_luminance;
        }
        let mut hints = 0;
        let mean_luminance = total_luminance / pixels.len() as f64;
        if mean_luminance > WallpaperColors::BRIGHT_IMAGE_MEAN_LUMINANCE as f64 && dark_pixels <= max_dark_pixels {
            hints |= WallpaperColors::HINT_SUPPORTS_DARK_TEXT;
        }
        if mean_luminance < WallpaperColors::DARK_THEME_MEAN_LUMINANCE as f64 {
            hints |= WallpaperColors::HINT_SUPPORTS_DARK_THEME;
        }
        hints
    }
}

// Android's ColorUtils, on 8-bit channels.

fn channel(c: u32, shift: u32) -> u32 {
    (c >> shift) & 0xff
}

/// `foreground` over `background`, each channel in whole numbers.
fn composite_colors(foreground: u32, background: u32) -> u32 {
    let (fg_a, bg_a) = (channel(foreground, 24), channel(background, 24));
    let a = 0xff - (((0xff - bg_a) * (0xff - fg_a)) / 0xff);
    let component = |shift: u32| {
        if a == 0 {
            0
        } else {
            ((0xff * channel(foreground, shift) * fg_a) + (channel(background, shift) * bg_a * (0xff - fg_a)))
                / (a * 0xff)
        }
    };
    a << 24 | component(16) << 16 | component(8) << 8 | component(0)
}

/// The Y of a color in CIE XYZ, 0..1.
fn calculate_luminance(color: u32) -> f64 {
    let linear = |shift: u32| {
        let s = channel(color, shift) as f64 / 255.0;
        if s < 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    let (sr, sg, sb) = (linear(16), linear(8), linear(0));
    100.0 * (sr * 0.2126 + sg * 0.7152 + sb * 0.0722) / 100.0
}

/// The WCAG contrast of a color (composited over the background where it is translucent) and an opaque
/// background.
fn calculate_contrast(foreground: u32, background: u32) -> f64 {
    let foreground = if channel(foreground, 24) < 255 { composite_colors(foreground, background) } else { foreground };
    let luminance1 = calculate_luminance(foreground) + 0.05;
    let luminance2 = calculate_luminance(background) + 0.05;
    luminance1.max(luminance2) / luminance1.min(luminance2)
}

/// A color's lightness in HSL, 0..1.
fn hsl_lightness(color: u32) -> f32 {
    let (rf, gf, bf) =
        (channel(color, 16) as f32 / 255.0, channel(color, 8) as f32 / 255.0, channel(color, 0) as f32 / 255.0);
    let max = rf.max(gf.max(bf));
    let min = rf.min(gf.min(bf));
    ((max + min) / 2.0).clamp(0.0, 1.0)
}
