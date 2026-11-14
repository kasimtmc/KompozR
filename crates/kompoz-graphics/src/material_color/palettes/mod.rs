//! Palettes (Material Color Utilities' `palettes`): a hue and chroma in every tone ([`TonalPalette`]), the five
//! of a key color ([`CorePalette`]), and the system's palette as Android gives it ([`DynamicTonalPalette`]).

mod core_palette;
mod dynamic_tonal_palette;

pub use core_palette::CorePalette;
pub use dynamic_tonal_palette::{DynamicTonalPalette, palette_tone};

use crate::material_color::hct::Hct;
use std::collections::HashMap;

/// A hue and chroma in every tone (Material Color Utilities' TonalPalette): tone 0 is black, 100 white, and each
/// tone between has as much of the chroma as sRGB has there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TonalPalette {
    hue: f64,
    chroma: f64,
    key_color: Hct,
}

impl TonalPalette {
    /// The palette of an ARGB color's hue and chroma, the color its key color.
    pub fn from_int(argb: u32) -> TonalPalette {
        TonalPalette::from_hct(Hct::from_int(argb))
    }

    /// The palette of a color's hue and chroma, the color its key color.
    pub fn from_hct(hct: Hct) -> TonalPalette {
        TonalPalette { hue: hct.hue(), chroma: hct.chroma(), key_color: hct }
    }

    /// The palette of a hue and chroma, its key color the tone nearest 50 that has the chroma.
    pub fn from_hue_and_chroma(hue: f64, chroma: f64) -> TonalPalette {
        TonalPalette { hue, chroma, key_color: KeyColor::new(hue, chroma).create() }
    }

    /// The ARGB color of a tone, 0..100. Yellows at 99 are the average of 98 and 100 (their 99 is too colorful
    /// next to white).
    pub fn tone(&self, tone: i32) -> u32 {
        if tone == 99 && Hct::is_yellow(self.hue) {
            average_argb(self.tone(98), self.tone(100))
        } else {
            Hct::from(self.hue, self.chroma, tone as f64).to_int()
        }
    }

    /// The color of any tone (not only a whole one).
    pub fn get_hct(&self, tone: f64) -> Hct {
        Hct::from(self.hue, self.chroma, tone)
    }

    pub fn hue(&self) -> f64 {
        self.hue
    }
    pub fn chroma(&self) -> f64 {
        self.chroma
    }
    /// The color the palette stands for: of its hue and chroma, the tone nearest 50 that has the chroma.
    pub fn key_color(&self) -> Hct {
        self.key_color
    }
}

/// The channels' averages, each rounded half up.
fn average_argb(argb1: u32, argb2: u32) -> u32 {
    let channel = |shift: u32| {
        let (c1, c2) = ((argb1 >> shift) & 0xff, (argb2 >> shift) & 0xff);
        // (Java's Math.round of a float.)
        (((c1 + c2) as f32 / 2.0 + 0.5).floor() as u32) & 255
    };
    255 << 24 | channel(16) << 16 | channel(8) << 8 | channel(0)
}

/// The key color of a hue and chroma: the tone nearest 50 that has the chroma (within 0.01), or else the tone
/// with the most chroma; found by binary search, the chroma at each tone cached.
struct KeyColor {
    hue: f64,
    requested_chroma: f64,
    chroma_cache: HashMap<i32, f64>,
}

impl KeyColor {
    const MAX_CHROMA_VALUE: f64 = 200.0;

    fn new(hue: f64, requested_chroma: f64) -> KeyColor {
        KeyColor { hue, requested_chroma, chroma_cache: HashMap::new() }
    }

    fn create(&mut self) -> Hct {
        // Pivot around T50 because T50 has the most chroma available, on average. Thus it is most likely to have
        // the direct answer.
        let pivot_tone = 50;
        let tone_step_size = 1;
        // Epsilon to accept values slightly higher than the requested chroma.
        let epsilon = 0.01;
        // Binary search to find the tone that can provide a chroma that is closest to the requested chroma.
        let (mut lower_tone, mut upper_tone) = (0, 100);
        while lower_tone < upper_tone {
            let mid_tone = (lower_tone + upper_tone) / 2;
            let is_ascending = self.max_chroma(mid_tone) < self.max_chroma(mid_tone + tone_step_size);
            let sufficient_chroma = self.max_chroma(mid_tone) >= self.requested_chroma - epsilon;
            if sufficient_chroma {
                // Either range [lowerTone, midTone] or [midTone, upperTone] has the answer, so search in the range
                // that is closer the pivot tone.
                if (lower_tone - pivot_tone).abs() < (upper_tone - pivot_tone).abs() {
                    upper_tone = mid_tone;
                } else {
                    if lower_tone == mid_tone {
                        return Hct::from(self.hue, self.requested_chroma, lower_tone as f64);
                    }
                    lower_tone = mid_tone;
                }
            } else if is_ascending {
                // As there is no sufficient chroma in the midTone, follow the direction to the chroma peak.
                lower_tone = mid_tone + tone_step_size;
            } else {
                // Keep midTone for potential chroma peak.
                upper_tone = mid_tone;
            }
        }
        Hct::from(self.hue, self.requested_chroma, lower_tone as f64)
    }

    /// The most chroma the hue has at a tone.
    fn max_chroma(&mut self, tone: i32) -> f64 {
        let hue = self.hue;
        *self
            .chroma_cache
            .entry(tone)
            .or_insert_with(|| Hct::from(hue, KeyColor::MAX_CHROMA_VALUE, tone as f64).chroma())
    }
}
