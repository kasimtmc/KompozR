//! Upright or italic (Compose's FontStyle), and what may be faked where a family has no face for a weight or a
//! style (Compose's FontSynthesis, with Android's and minikin's rules for when).

use super::weight::FontWeight;

/// Upright or italic (Compose's FontStyle).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
}

impl FontStyle {
    /// Both (Compose's values()).
    pub const VALUES: [FontStyle; 2] = [FontStyle::Normal, FontStyle::Italic];

    /// Compose's value: 0 upright, 1 italic.
    pub fn value(self) -> i32 {
        match self {
            FontStyle::Normal => 0,
            FontStyle::Italic => 1,
        }
    }
}

/// What may be faked where a family has no face of the weight or style asked for (Compose's FontSynthesis):
/// glyphs emboldened, slanted, both or neither.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FontSynthesis {
    pub value: i32,
}

impl FontSynthesis {
    const WEIGHT: i32 = 0x1;
    const STYLE: i32 = 0x2;

    pub const None: FontSynthesis = FontSynthesis { value: 0 };
    pub const Weight: FontSynthesis = FontSynthesis { value: FontSynthesis::WEIGHT };
    pub const Style: FontSynthesis = FontSynthesis { value: FontSynthesis::STYLE };
    pub const All: FontSynthesis = FontSynthesis { value: 0xffff };

    /// Whether a weight may be faked.
    pub fn is_weight_on(self) -> bool {
        self.value & FontSynthesis::WEIGHT != 0
    }
    /// Whether italics may be faked.
    pub fn is_style_on(self) -> bool {
        self.value & FontSynthesis::STYLE != 0
    }

    /// Whether a face of weight `actual` is emboldened for `wanted`: allowed, and Compose's rule (a weight of 600
    /// and up asked of a face lighter than 600) and minikin's (200 or more heavier than the face) both say so.
    pub fn synthesizes_weight(self, wanted: FontWeight, actual: FontWeight) -> bool {
        self.is_weight_on() && wanted.weight >= 600 && actual.weight < 600 && wanted.weight - actual.weight >= 200
    }

    /// Whether a face of style `actual` is slanted for `wanted`: allowed, italics asked of an upright face.
    pub fn synthesizes_style(self, wanted: FontStyle, actual: FontStyle) -> bool {
        self.is_style_on() && wanted == FontStyle::Italic && actual != FontStyle::Italic
    }
}

/// Both may be faked (Compose's default).
impl Default for FontSynthesis {
    fn default() -> FontSynthesis {
        FontSynthesis::All
    }
}
