//! Material Color Utilities' CorePalette.

use super::TonalPalette;
use crate::material_color::hct::Hct;

/// The five palettes of a key color, all but one of its hue, differing in chroma, and an error palette (Material
/// Color Utilities' CorePalette, from before dynamic schemes): between a theme's key color and its scheme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CorePalette {
    pub a1: TonalPalette,
    pub a2: TonalPalette,
    pub a3: TonalPalette,
    pub n1: TonalPalette,
    pub n2: TonalPalette,
    pub error: TonalPalette,
}

impl CorePalette {
    /// The palettes of a color's hue: a1 at its chroma (at least 48), a2 at 16, a3 at 24 turned 60°, n1 at 4 and
    /// n2 at 8.
    pub fn of(argb: u32) -> CorePalette {
        CorePalette::new(argb, false)
    }

    /// The palettes of a content color (one from a picture): its own chroma, a third, a half (turned 60°), a
    /// twelfth (at most 4) and a sixth (at most 8).
    pub fn content_of(argb: u32) -> CorePalette {
        CorePalette::new(argb, true)
    }

    fn new(argb: u32, is_content: bool) -> CorePalette {
        let hct = Hct::from_int(argb);
        let (hue, chroma) = (hct.hue(), hct.chroma());
        let p = TonalPalette::from_hue_and_chroma;
        let error = p(25.0, 84.0);
        if is_content {
            CorePalette {
                a1: p(hue, chroma),
                a2: p(hue, chroma / 3.0),
                a3: p(hue + 60.0, chroma / 2.0),
                n1: p(hue, (chroma / 12.0).min(4.0)),
                n2: p(hue, (chroma / 6.0).min(8.0)),
                error,
            }
        } else {
            CorePalette {
                a1: p(hue, 48f64.max(chroma)),
                a2: p(hue, 16.0),
                a3: p(hue + 60.0, 24.0),
                n1: p(hue, 4.0),
                n2: p(hue, 8.0),
                error,
            }
        }
    }
}
