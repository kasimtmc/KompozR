//! The bottom of the text pipeline: a font backend picks the faces a text's characters are shaped in, shapes runs
//! of text into positioned glyphs, and measures faces. Everything above it (paragraphs, bidi, line breaking,
//! ellipsis, hit testing, draw lists) is the same whichever backend is below: the real one over harfrust and
//! skrifa, or [`FakeBackend`], which measures as the C++ Kompoz's tests did.

mod cache;
pub(crate) mod face;
mod fake;
mod harfrust;

pub use cache::ShapeCacheStats;
pub use face::fake_bold_strength;
pub use fake::{FakeBackend, FakeFontFamilyResolver};
pub use harfrust::HarfrustBackend;

use crate::draw::{GlyphColorKind, GlyphSynthesis};
use crate::font::{FontFeature, FontId, Typeface};
use crate::intl::{Locale, LocaleList};
use kompoz_graphics::{Density, Rect};
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::rc::Rc;

/// A face at a size: the font, its variation coordinates (normalised, F2Dot14 bits, one per fvar axis), its size
/// in px and what is faked.
#[derive(Clone, PartialEq, Debug)]
pub struct FontInstance {
    pub font: FontId,
    pub coords: Rc<[i16]>,
    pub size: f32,
    pub synthesis: GlyphSynthesis,
}

impl Eq for FontInstance {}

impl Hash for FontInstance {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.font.hash(state);
        self.coords.hash(state);
        (self.size + 0.0).to_bits().hash(state);
        self.synthesis.hash(state);
    }
}

/// What to pick faces for: `text[range]` (the whole `text` its context) in a resolved typeface at `size` px, its
/// locales (they pick among fallback fonts: Japanese or Chinese Han), `density` for settings in sp (an optical
/// size).
#[derive(Clone, Debug)]
pub struct FontRunRequest<'a> {
    pub text: &'a str,
    pub range: Range<usize>,
    pub typeface: &'a Typeface,
    pub size: f32,
    pub density: Density,
    pub locales: &'a LocaleList,
}

/// A stretch of text shaped in one face (a fallback face where the typeface's lacks its characters).
#[derive(Clone, PartialEq, Debug)]
pub struct FontRun {
    pub range: Range<usize>,
    pub font: FontInstance,
}

/// What to shape: `text[range]` (the whole `text` its context, which joining and kerning across the cut see) in
/// one face, one direction and one script (None: the backend works it out), in a language (it picks localized
/// forms), with OpenType features, `letter_spacing` px added around each cluster (half before, half after; none
/// in cursive scripts) and advances stretched by `scale_x`.
#[derive(Clone, Debug)]
pub struct ShapeRequest<'a> {
    pub text: &'a str,
    pub range: Range<usize>,
    pub font: &'a FontInstance,
    pub rtl: bool,
    pub script: Option<[u8; 4]>,
    pub language: Option<&'a Locale>,
    pub features: &'a [FontFeature],
    pub letter_spacing: f32,
    pub scale_x: f32,
}

/// A glyph shaped: its id, the byte offset in the request's text of the cluster it is for, its advance and offset
/// (px, y up as fonts have it), and whether breaking the text at its cluster (or cutting the run there) would
/// change how either side shapes (HarfBuzz's UNSAFE_TO_BREAK: reshape such a cut).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ShapedGlyph {
    pub id: u32,
    pub cluster: usize,
    pub x_advance: f32,
    pub y_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
    pub unsafe_to_break: bool,
}

/// A run shaped: its glyphs in visual order (left to right; for right-to-left text the last cluster first) and
/// their advances' sum.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ShapedRun {
    pub glyphs: Vec<ShapedGlyph>,
    pub advance: f32,
}

/// A face's metrics at a size, in px: how far it reaches above and below the baseline (both positive), the gap
/// it puts below a line (lines are ascent + descent + leading apart), where its underline's and strikeout's
/// middles are (from the baseline, positive below) and how thick they are, its x-height and cap height.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct FontMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub leading: f32,
    pub underline_position: f32,
    pub underline_thickness: f32,
    pub strikeout_position: f32,
    pub strikeout_thickness: f32,
    pub x_height: f32,
    pub cap_height: f32,
}

impl FontMetrics {
    /// A line's height: ascent, descent and leading.
    pub fn line_height(&self) -> f32 {
        self.ascent + self.descent + self.leading
    }
}

/// The bottom of the text pipeline (single-threaded, as the UI is): picks the faces a text's characters are shaped
/// in, shapes, measures. The resolver that hands it typefaces ([`FontFamilyResolver`](crate::font::FontFamilyResolver))
/// gives it out (`backend()`).
pub trait FontBackend {
    /// The faces `request.text[request.range]` is shaped in, in order and covering the range: its typeface's face
    /// where it has the characters, fallback faces where not (a grapheme cluster always in one face).
    fn font_runs(&self, request: &FontRunRequest<'_>) -> Vec<FontRun>;

    /// Shapes a run in one face, one direction and one script.
    fn shape(&self, request: &ShapeRequest<'_>) -> ShapedRun;

    /// A face's metrics at its size.
    fn metrics(&self, font: &FontInstance) -> FontMetrics;

    /// How far a glyph's ink reaches from its origin, px, y down (its synthesis included; a run's stretch and
    /// slant not).
    fn glyph_bounds(&self, font: &FontInstance, glyph: u32) -> Rect;

    /// How a face's glyphs are colored.
    fn color_kind(&self, font: FontId) -> GlyphColorKind;
}
