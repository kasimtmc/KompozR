//! The font data that the autohinter reads (skrifa's `FontContext`): a [`Font`] at its location in the variation
//! space, its character map, its outlines as points in font units, its layout tables and metrics.

use super::metrics::BlueOutlineBuf;
use super::outline::Outline;
use skrifa::Font;
use skrifa::attribute::Style;
use skrifa::raw::TableProvider;
use skrifa::raw::model::charmap::Charmap;
use skrifa::raw::model::glyph::outline::UnscaledOutlineSink;
use skrifa::raw::model::glyph::outline::{DrawMemory, OutlineGlyph, OutlineGlyphs, OutlineSource};
use skrifa::raw::tables::{gdef::Gdef, gsub::Gsub};
use skrifa::raw::types::{F2Dot14, GlyphId};

/// Provides the font data the autohinter reads at one location in the variation space.
///
/// The autohinter holds this as a trait object so that a single copy of the autohinter serves every kind of font.
pub(crate) trait AutohintContext {
    /// Returns the normalized variation coordinates of the location.
    fn coords(&self) -> &[F2Dot14];

    /// Returns the number of glyphs that can be assigned a style.
    fn glyph_count(&self) -> u32;

    fn units_per_em(&self) -> i32;

    fn style(&self) -> Style;

    fn is_fixed_pitch(&self) -> bool;

    /// Returns true if the character map is a symbol map, which yields no metrics.
    fn is_symbol(&self) -> bool;

    fn map(&self, ch: u32) -> Option<GlyphId>;

    /// Calls `f` with each character and its glyph, in character order.
    fn for_each_mapping(&self, f: &mut dyn FnMut(u32, GlyphId));

    /// Returns the substitution table, which is read only when shaping uses it.
    fn gsub(&self) -> Option<Gsub<'_>>;

    /// Returns the glyph definition table, which provides the variation store.
    fn gdef(&self) -> Option<Gdef<'_>>;

    /// Returns the unscaled advance width of `glyph`.
    fn advance_width(&self, glyph: GlyphId) -> Option<f32>;

    /// Fills `outline` with `glyph` and analyzes it.
    ///
    /// Returns whether the glyph is flagged as having overlapping contours or components, or `None` if there is no
    /// outline for `glyph` (or it can't be drawn).
    fn fill_outline(&self, glyph: GlyphId, outline: &mut Outline) -> Option<bool>;

    /// Draws the unscaled points of `glyph` into `buf` and returns its advance, or returns `None` if there is no
    /// outline for `glyph` (or it can't be drawn).
    fn draw_unscaled(&self, glyph: GlyphId, buf: &mut BlueOutlineBuf) -> Option<i32>;
}

/// An autohinting context for a [`Font`] at the font's location.
pub(crate) struct FontContext<'a> {
    font: &'a Font,
    charmap: Charmap<'a>,
    outlines: OutlineGlyphs<'a>,
}

impl<'a> FontContext<'a> {
    /// Creates a context that reads glyphs from `font`'s outlines.
    pub fn new(font: &'a Font) -> Self {
        Self { font, charmap: font.charmap(), outlines: font.outline_glyphs() }
    }

    /// Writes `outline` into `sink` in font units and returns its advance.
    ///
    /// A `glyf` outline provides its advance through its phantom points, with variations applied. The other formats
    /// read it from the metrics tables.
    fn draw_unscaled_into(&self, outline: &OutlineGlyph<'_>, sink: &mut dyn UnscaledOutlineSink) -> Option<i32> {
        let glyph = outline.glyph();
        let metrics = outline.draw_unscaled(DrawMemory::Scratch, sink).ok()?;
        Some(match outline {
            OutlineGlyph::Glyf(_) => metrics.h_advance.unwrap_or_default() as i32,
            // A location can add a fraction, which FreeType rounds to a whole unit.
            _ => self.font.glyph_metrics().h_advance_exact(glyph).round().to_i32(),
        })
    }
}

impl AutohintContext for FontContext<'_> {
    fn coords(&self) -> &[F2Dot14] {
        self.font.normalized_coords()
    }

    fn glyph_count(&self) -> u32 {
        // FreeType's face's glyph count, which sizes its style table: a PostScript font's charstrings, else the
        // maximum profile's count (whether or not the glyphs have outlines: a bitmap font's are styled too).
        match self.outlines.underlying_source() {
            Some(OutlineSource::Cff2 | OutlineSource::Cff) => {
                self.outlines.cff_font().map_or(0, |cff| cff.num_glyphs())
            }
            _ => self.font.tables().selected_maxp().map_or(0, |maxp| maxp.num_glyphs()),
        }
    }

    fn units_per_em(&self) -> i32 {
        self.font.units_per_em() as i32
    }

    fn style(&self) -> Style {
        // From the model, which reads a PostScript font's own dictionary where an SFNT font has OS/2 and post.
        match self.font.default_attributes().style {
            skrifa::raw::model::attribute::Style::Normal => Style::Normal,
            skrifa::raw::model::attribute::Style::Italic => Style::Italic,
            skrifa::raw::model::attribute::Style::Oblique(angle) => Style::Oblique(angle),
        }
    }

    fn is_fixed_pitch(&self) -> bool {
        self.font.style_metrics().is_fixed_pitch.unwrap_or_default()
    }

    fn is_symbol(&self) -> bool {
        self.charmap.unicode_is_symbol()
    }

    fn map(&self, ch: u32) -> Option<GlyphId> {
        self.charmap.map_unicode(ch)
    }

    fn for_each_mapping(&self, f: &mut dyn FnMut(u32, GlyphId)) {
        for (ch, glyph) in self.charmap.unicodes() {
            f(ch, glyph);
        }
    }

    fn gsub(&self) -> Option<Gsub<'_>> {
        self.font.tables().gsub().ok()
    }

    fn gdef(&self) -> Option<Gdef<'_>> {
        self.font.tables().gdef().ok()
    }

    fn advance_width(&self, glyph: GlyphId) -> Option<f32> {
        Some(self.font.glyph_metrics().h_advance(glyph))
    }

    fn fill_outline(&self, glyph: GlyphId, outline: &mut Outline) -> Option<bool> {
        let source = self.outlines.get(glyph)?;
        let has_overlaps = matches!(&source, OutlineGlyph::Glyf(glyph) if glyph.plan().has_overlaps);
        outline.fill_with(self.units_per_em(), |outline| self.draw_unscaled_into(&source, outline))?;
        Some(has_overlaps)
    }

    fn draw_unscaled(&self, glyph: GlyphId, buf: &mut BlueOutlineBuf) -> Option<i32> {
        let source = self.outlines.get(glyph)?;
        self.draw_unscaled_into(&source, buf)
    }
}
