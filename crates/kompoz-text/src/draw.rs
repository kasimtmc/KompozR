//! What laid out text is drawn as: one [`TextDraw`] per line, style run and direction run, carrying its text,
//! where it goes, the style it was asked for and the glyphs it was shaped into. A drawing surface draws it from
//! this alone (stage 7's renderer reads each glyph run's font back from the [`FontRegistry`]).
//!
//! [`FontRegistry`]: crate::font::FontRegistry

use crate::font::{FontFamily, FontId, FontStyle, FontSynthesis, FontWeight};
use crate::span_style::SpanStyle;
use crate::style::{TextGeometricTransform, TextMotion};
use crate::text_style::TextStyle;
use kompoz_graphics::{Density, DrawStyle, Offset, Rect, Size, TextUnit};
use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

/// What a renderer fakes for a face that lacks the weight or style asked for: outlines emboldened, slanted by
/// `skew` (a point y above the baseline moves y · -skew to the right: -0.25 is Skia's synthetic italic; 0 none).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct GlyphSynthesis {
    pub embolden: bool,
    pub skew: f32,
}

impl GlyphSynthesis {
    /// Nothing faked.
    pub const None: GlyphSynthesis = GlyphSynthesis { embolden: false, skew: 0.0 };
    /// Skia's synthetic italic slant.
    pub const ITALIC_SKEW: f32 = -0.25;
}

impl Eq for GlyphSynthesis {}

impl Hash for GlyphSynthesis {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.embolden.hash(state);
        (self.skew + 0.0).to_bits().hash(state);
    }
}

/// How a run's glyphs are drawn: outlines (filled or stroked in the text's paint), or the font's own colors —
/// COLR v0 layers, COLR v1 paint graphs, bitmaps (CBDT, sbix) — which a paint doesn't tint.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum GlyphColorKind {
    #[default]
    Outline,
    ColrV0,
    ColrV1,
    Bitmap,
}

/// A glyph placed: its id in the run's font, where its origin is (px, from the [`TextDraw`]'s baseline origin, y
/// down), and the byte offset in the TextDraw's text of the cluster it is for.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Glyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub cluster: u32,
}

/// Glyphs of one font, in visual order (left to right on the screen): the font by its [`FontId`], its variation
/// coordinates (normalised, F2Dot14 bits, one per axis of the font's fvar), its size, what is faked, how it is
/// stretched and slanted (the text's geometric transform), what colors its glyphs, how it moves.
#[derive(Clone, PartialEq, Debug)]
pub struct GlyphRun {
    pub font: FontId,
    pub coords: Rc<[i16]>,
    /// Px.
    pub size: f32,
    pub synthesis: GlyphSynthesis,
    pub scale_x: f32,
    pub skew_x: f32,
    pub color: GlyphColorKind,
    pub text_motion: TextMotion,
    pub glyphs: Vec<Glyph>,
}

impl GlyphRun {
    fn hash_into(&self, h: &mut DefaultHasher) {
        self.font.hash(h);
        self.coords.hash(h);
        self.size.to_bits().hash(h);
        self.synthesis.hash(h);
        self.scale_x.to_bits().hash(h);
        self.skew_x.to_bits().hash(h);
        self.color.hash(h);
        self.text_motion.hash(h);
        for g in &self.glyphs {
            (g.id, g.x.to_bits(), g.y.to_bits(), g.cluster).hash(h);
        }
    }
}

/// The style a run of text was drawn in, in px (what C++'s tests read from each drawn text's Font): its family,
/// weight, style and synthesis as asked for, its font size, letter spacing and line height (None: the fonts' own),
/// its OpenType feature settings, its draw style (a stroke outlines the glyphs), its geometric transform.
#[derive(Clone, PartialEq, Debug)]
pub struct TextRunStyle {
    pub font_family: FontFamily,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub font_synthesis: FontSynthesis,
    pub font_size: f32,
    pub letter_spacing: f32,
    pub line_height: Option<f32>,
    pub font_feature_settings: String,
    pub draw_style: DrawStyle,
    pub scale_x: f32,
    pub skew_x: f32,
}

impl TextRunStyle {
    /// The default family, regular, upright, `font_size` px, nothing else given.
    pub fn new(font_size: f32) -> TextRunStyle {
        TextRunStyle { font_size, ..TextRunStyle::default() }
    }

    /// The style a run is drawn in: its text's style (its defaults resolved: [`TextStyle::resolve_defaults`]) with
    /// the span styles over the run applied in their order (a later one's attributes win; an em font size is of
    /// the size before it, as Android's relative size spans compound), in px through `density`. An em letter
    /// spacing is of the run's own font size, an em line height of the text's; a text style's own em font size is
    /// of the default 14 sp.
    pub fn resolve(text_style: &TextStyle, spans: &[&SpanStyle], density: Density) -> TextRunStyle {
        let text_px = font_size_px(text_style.font_size, crate::span_style::DefaultFontSize, density);
        let mut style = TextRunStyle {
            font_family: text_style.font_family.clone().unwrap_or_default(),
            font_weight: text_style.font_weight.unwrap_or_default(),
            font_style: text_style.font_style.unwrap_or_default(),
            font_synthesis: text_style.font_synthesis.unwrap_or_default(),
            font_size: text_px,
            letter_spacing: 0.0,
            line_height: text_style
                .line_height
                .is_specified()
                .then(|| text_unit_px(text_style.line_height, text_px, density)),
            font_feature_settings: text_style.font_feature_settings.clone().unwrap_or_default(),
            draw_style: text_style.draw_style.clone().unwrap_or_default(),
            scale_x: 1.0,
            skew_x: 0.0,
        };
        let mut letter_spacing = text_style.letter_spacing;
        let mut transform = text_style.text_geometric_transform.unwrap_or(TextGeometricTransform::None);
        for s in spans {
            if s.font_size.is_specified() {
                style.font_size = text_unit_px(s.font_size, style.font_size, density);
            }
            if let Some(w) = s.font_weight {
                style.font_weight = w;
            }
            if let Some(f) = s.font_style {
                style.font_style = f;
            }
            if let Some(f) = s.font_synthesis {
                style.font_synthesis = f;
            }
            if let Some(f) = &s.font_family {
                style.font_family = f.clone();
            }
            if let Some(f) = &s.font_feature_settings {
                style.font_feature_settings = f.clone();
            }
            if s.letter_spacing.is_specified() {
                letter_spacing = s.letter_spacing;
            }
            if let Some(d) = &s.draw_style {
                style.draw_style = d.clone();
            }
            if let Some(t) = s.text_geometric_transform {
                transform = t;
            }
        }
        if letter_spacing.is_specified() {
            style.letter_spacing = text_unit_px(letter_spacing, style.font_size, density);
        }
        style.scale_x = transform.scale_x;
        style.skew_x = transform.skew_x;
        style
    }
}

/// A text length in px: sp through the density, em of `em_of` px.
fn text_unit_px(unit: TextUnit, em_of: f32, density: Density) -> f32 {
    if unit.is_em() { unit.value() * em_of } else { density.text_to_px(unit) }
}

/// A font size in px: sp through the density, em of `default` (sp), unspecified `default`.
fn font_size_px(size: TextUnit, default: TextUnit, density: Density) -> f32 {
    let default_px = density.text_to_px(default);
    if size.is_unspecified() { default_px } else { text_unit_px(size, default_px, density) }
}

/// The default family, regular, upright, 14 px, no letter spacing, the fonts' own line height, filled.
impl Default for TextRunStyle {
    fn default() -> TextRunStyle {
        TextRunStyle {
            font_family: FontFamily::Default,
            font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal,
            font_synthesis: FontSynthesis::All,
            font_size: 14.0,
            letter_spacing: 0.0,
            line_height: None,
            font_feature_settings: String::new(),
            draw_style: DrawStyle::Fill,
            scale_x: 1.0,
            skew_x: 0.0,
        }
    }
}

/// A run of laid out text to draw: one line's stretch of one style and one direction (C++'s pieces). Its text in
/// logical order (an ellipsis included); where its baseline starts (its glyphs are placed from there); its advance
/// width and line height; how far its glyphs' ink reaches from the baseline origin (for what a display list
/// redraws: stroke and synthesis included, blur not); whether it goes right to left (its glyphs are in visual
/// order either way); the style it was asked for; its glyph runs, and a key of their content. Equal when it shows
/// the same: the same text, place, size, ink and direction, an equal style, and the same glyph runs (the same `Rc`,
/// or the same content key).
#[derive(Clone)]
pub struct TextDraw {
    pub text: Rc<str>,
    pub baseline: Offset,
    pub size: Size,
    pub ink_bounds: Rect,
    pub rtl: bool,
    pub style: Rc<TextRunStyle>,
    pub glyphs: Rc<[GlyphRun]>,
    pub content_key: u64,
}

impl TextDraw {
    /// A run of text to draw, its content key worked out from its glyph runs.
    pub fn new(
        text: impl Into<Rc<str>>,
        baseline: Offset,
        size: Size,
        ink_bounds: Rect,
        rtl: bool,
        style: Rc<TextRunStyle>,
        glyphs: Rc<[GlyphRun]>,
    ) -> TextDraw {
        let content_key = TextDraw::content_key_of(&glyphs);
        TextDraw { text: text.into(), baseline, size, ink_bounds, rtl, style, glyphs, content_key }
    }

    /// A 64-bit key of glyph runs' content: their fonts, coordinates, sizes, synthesis, transforms, color kinds,
    /// motion and glyphs (what layout works out once, so equal runs made apart compare equal cheaply).
    pub fn content_key_of(glyphs: &[GlyphRun]) -> u64 {
        let mut h = DefaultHasher::new();
        glyphs.len().hash(&mut h);
        for run in glyphs {
            run.hash_into(&mut h);
        }
        h.finish()
    }

    /// Its font size (px).
    pub fn font_size(&self) -> f32 {
        self.style.font_size
    }

    /// Where its ink reaches, where it is drawn.
    pub fn ink_rect(&self) -> Rect {
        self.ink_bounds.translate(self.baseline)
    }

    /// The same drawn with its baseline at `baseline`.
    pub fn at(&self, baseline: Offset) -> TextDraw {
        TextDraw { baseline, ..self.clone() }
    }
}

impl PartialEq for TextDraw {
    fn eq(&self, o: &TextDraw) -> bool {
        self.text == o.text
            && self.baseline == o.baseline
            && self.size == o.size
            && self.ink_bounds == o.ink_bounds
            && self.rtl == o.rtl
            && (Rc::ptr_eq(&self.style, &o.style) || *self.style == *o.style)
            && (Rc::ptr_eq(&self.glyphs, &o.glyphs) || self.content_key == o.content_key)
    }
}

/// Its text, baseline and size.
impl fmt::Debug for TextDraw {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TextDraw")
            .field("text", &self.text)
            .field("baseline", &self.baseline)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}
