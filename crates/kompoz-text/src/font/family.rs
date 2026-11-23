//! A family of fonts (Compose's FontFamily): the system's generic ones, a list of fonts of one's own, or a
//! typeface loaded already.

use super::font::Font;
use super::resolver::Typeface;
use std::fmt;
use std::sync::Arc;

/// A family the system provides by kind (Compose's GenericFontFamily and DefaultFontFamily): what text falls
/// back on, too.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum GenericFontFamily {
    /// The system's default (sans serif).
    #[default]
    Default,
    SansSerif,
    Serif,
    Monospace,
    Cursive,
}

impl GenericFontFamily {
    /// Its CSS name (Compose's GenericFontFamily.name): `sans-serif`, `serif`, `monospace`, `cursive`; none for
    /// the default.
    pub fn name(self) -> Option<&'static str> {
        match self {
            GenericFontFamily::Default => None,
            GenericFontFamily::SansSerif => Some("sans-serif"),
            GenericFontFamily::Serif => Some("serif"),
            GenericFontFamily::Monospace => Some("monospace"),
            GenericFontFamily::Cursive => Some("cursive"),
        }
    }
    /// The one of a CSS name.
    pub fn from_name(name: &str) -> Option<GenericFontFamily> {
        match name {
            "sans-serif" => Some(GenericFontFamily::SansSerif),
            "serif" => Some(GenericFontFamily::Serif),
            "monospace" => Some(GenericFontFamily::Monospace),
            "cursive" => Some(GenericFontFamily::Cursive),
            _ => None,
        }
    }
}

/// A family of one's own fonts (Compose's FontListFontFamily): never empty; the font for a weight and style is
/// matched as CSS does ([`FontMatcher`](super::matcher::FontMatcher)).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct FontListFontFamily {
    fonts: Arc<[Font]>,
}

impl FontListFontFamily {
    /// Of these fonts (none panics, as Compose's).
    pub fn new(fonts: impl IntoIterator<Item = Font>) -> FontListFontFamily {
        let fonts: Arc<[Font]> = fonts.into_iter().collect();
        assert!(!fonts.is_empty(), "kompoz: At least one font should be passed to FontFamily");
        FontListFontFamily { fonts }
    }
    pub fn fonts(&self) -> &[Font] {
        &self.fonts
    }
}

/// `FontListFontFamily(fonts=[…])`, as Compose writes it.
impl fmt::Debug for FontListFontFamily {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "FontListFontFamily(fonts={:?})", self.fonts)
    }
}

/// A family of fonts (Compose's FontFamily): the system's by kind, one's own fonts, or a typeface resolved
/// already (Compose's LoadedFontFamily). Equal and hashed by what it is (the same fonts make the same family).
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub enum FontFamily {
    /// The system's default.
    #[default]
    Default,
    SansSerif,
    Serif,
    Monospace,
    Cursive,
    List(FontListFontFamily),
    Loaded(Typeface),
}

impl FontFamily {
    /// A family of these fonts (Compose's FontFamily(vararg fonts)).
    ///
    /// Kotlin: `FontFamily(vararg fonts: Font)`
    pub fn new(fonts: impl IntoIterator<Item = Font>) -> FontFamily {
        FontFamily::List(FontListFontFamily::new(fonts))
    }
    /// A typeface's (Compose's FontFamily(typeface)).
    pub fn from_typeface(typeface: Typeface) -> FontFamily {
        FontFamily::Loaded(typeface)
    }
    /// The family of a generic kind.
    pub fn generic(kind: GenericFontFamily) -> FontFamily {
        match kind {
            GenericFontFamily::Default => FontFamily::Default,
            GenericFontFamily::SansSerif => FontFamily::SansSerif,
            GenericFontFamily::Serif => FontFamily::Serif,
            GenericFontFamily::Monospace => FontFamily::Monospace,
            GenericFontFamily::Cursive => FontFamily::Cursive,
        }
    }
    /// Its generic kind, for one of the system's.
    pub fn as_generic(&self) -> Option<GenericFontFamily> {
        match self {
            FontFamily::Default => Some(GenericFontFamily::Default),
            FontFamily::SansSerif => Some(GenericFontFamily::SansSerif),
            FontFamily::Serif => Some(GenericFontFamily::Serif),
            FontFamily::Monospace => Some(GenericFontFamily::Monospace),
            FontFamily::Cursive => Some(GenericFontFamily::Cursive),
            _ => None,
        }
    }
    /// Its fonts, for a family of one's own.
    pub fn fonts(&self) -> Option<&[Font]> {
        match self {
            FontFamily::List(l) => Some(l.fonts()),
            _ => None,
        }
    }
}

/// One font's family.
impl From<Font> for FontFamily {
    fn from(font: Font) -> FontFamily {
        FontFamily::new([font])
    }
}

/// As Compose writes it: `FontFamily.Serif`, `FontListFontFamily(fonts=[…])`.
impl fmt::Debug for FontFamily {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FontFamily::Default => f.write_str("FontFamily.Default"),
            FontFamily::SansSerif => f.write_str("FontFamily.SansSerif"),
            FontFamily::Serif => f.write_str("FontFamily.Serif"),
            FontFamily::Monospace => f.write_str("FontFamily.Monospace"),
            FontFamily::Cursive => f.write_str("FontFamily.Cursive"),
            FontFamily::List(l) => write!(f, "{l:?}"),
            FontFamily::Loaded(t) => write!(f, "LoadedFontFamily(typeface={t:?})"),
        }
    }
}
