//! How heavy glyphs are (Compose's FontWeight).

use std::fmt;

/// How heavy glyphs are, 1 to 1000 (Compose's FontWeight): 400 regular, 700 bold.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, kompoz_runtime::KotlinProperties)]
pub struct FontWeight {
    pub weight: i32,
}

impl FontWeight {
    pub const W100: FontWeight = FontWeight { weight: 100 };
    pub const W200: FontWeight = FontWeight { weight: 200 };
    pub const W300: FontWeight = FontWeight { weight: 300 };
    pub const W400: FontWeight = FontWeight { weight: 400 };
    pub const W500: FontWeight = FontWeight { weight: 500 };
    pub const W600: FontWeight = FontWeight { weight: 600 };
    pub const W700: FontWeight = FontWeight { weight: 700 };
    pub const W800: FontWeight = FontWeight { weight: 800 };
    pub const W900: FontWeight = FontWeight { weight: 900 };
    pub const Thin: FontWeight = FontWeight::W100;
    pub const ExtraLight: FontWeight = FontWeight::W200;
    pub const Light: FontWeight = FontWeight::W300;
    pub const Normal: FontWeight = FontWeight::W400;
    pub const Medium: FontWeight = FontWeight::W500;
    pub const SemiBold: FontWeight = FontWeight::W600;
    pub const Bold: FontWeight = FontWeight::W700;
    pub const ExtraBold: FontWeight = FontWeight::W800;
    pub const Black: FontWeight = FontWeight::W900;

    /// The nine named ones, lightest first.
    pub const VALUES: [FontWeight; 9] = [
        FontWeight::W100,
        FontWeight::W200,
        FontWeight::W300,
        FontWeight::W400,
        FontWeight::W500,
        FontWeight::W600,
        FontWeight::W700,
        FontWeight::W800,
        FontWeight::W900,
    ];

    /// A weight of its own, 1 to 1000 (others panic, as Compose's).
    pub const fn new(weight: i32) -> FontWeight {
        assert!(weight >= 1 && weight <= 1000, "kompoz: a font weight is 1 to 1000");
        FontWeight { weight }
    }
}

/// Regular (400).
impl Default for FontWeight {
    fn default() -> FontWeight {
        FontWeight::Normal
    }
}

/// `FontWeight(weight=700)`, as Compose writes it.
impl fmt::Debug for FontWeight {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "FontWeight(weight={})", self.weight)
    }
}

/// Between two weights, rounded, within 1 to 1000 (Compose's lerp(FontWeight, FontWeight, fraction)).
pub fn lerp_font_weight(start: FontWeight, stop: FontWeight, fraction: f32) -> FontWeight {
    let w = (start.weight as f32 + (stop.weight - start.weight) as f32 * fraction).round() as i32;
    FontWeight::new(w.clamp(1, 1000))
}
