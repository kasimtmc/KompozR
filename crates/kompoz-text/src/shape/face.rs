//! A registered font read with skrifa (read-fonts' model): its characters, its instances at variation
//! coordinates, its metrics and glyph extents at a size, how its glyphs are colored. Built once per font and kept.

use super::FontMetrics;
use crate::draw::{GlyphColorKind, GlyphSynthesis};
use crate::font::{FontId, FontRegistry};
use kompoz_graphics::Rect;
use skrifa::raw::types::F2Dot14;
use skrifa::{Font, GlyphId, Tag};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// What a fake bold adds to a glyph's strokes, as a share of the size (Skia's: 1/24 at 9 px down to 1/32 at 36 px
/// and up, linearly between): a renderer strokes the outline this wide, so the ink grows by half of it each way.
pub fn fake_bold_strength(size: f32) -> f32 {
    let t = ((size - 9.0) / (36.0 - 9.0)).clamp(0.0, 1.0);
    size * (1.0 / 24.0 + (1.0 / 32.0 - 1.0 / 24.0) * t)
}

/// A font, read.
pub(crate) struct Face {
    pub(crate) font: Font,
    pub(crate) upem: f32,
    pub(crate) color: GlyphColorKind,
    /// The weight and italics of its design (at its default location).
    pub(crate) weight: f32,
    pub(crate) italic: bool,
    instances: RefCell<HashMap<Rc<[i16]>, Font>>,
    coords: RefCell<HashMap<Vec<([u8; 4], u32)>, Rc<[i16]>>>,
}

impl Face {
    fn load(id: FontId) -> Option<Face> {
        let data = FontRegistry::get(id)?;
        let font = Font::new(data.blob, data.index)?;
        let upem = font.units_per_em() as f32;
        let color = match font.color_glyphs().version() {
            Some(skrifa::font::glyph::color::ColorVersion::V1) => GlyphColorKind::ColrV1,
            Some(skrifa::font::glyph::color::ColorVersion::V0) => GlyphColorKind::ColrV0,
            None => match font.bitmap_strikes().source() {
                Some(skrifa::font::glyph::bitmap::BitmapSource::Sbix)
                | Some(skrifa::font::glyph::bitmap::BitmapSource::Cbdt) => GlyphColorKind::Bitmap,
                _ => GlyphColorKind::Outline,
            },
        };
        let attributes = font.default_attributes();
        let italic = !matches!(attributes.style, skrifa::font::attribute::Style::Normal);
        let weight = attributes.weight.value();
        Some(Face {
            font,
            upem,
            color,
            weight,
            italic,
            instances: RefCell::new(HashMap::new()),
            coords: RefCell::new(HashMap::new()),
        })
    }

    /// Whether its glyphs have colors of their own.
    pub(crate) fn is_color(&self) -> bool {
        self.color != GlyphColorKind::Outline
    }

    /// Whether it has a glyph for `c`.
    pub(crate) fn has_char(&self, c: char) -> bool {
        self.font.charmap().map_unicode(c).is_some()
    }

    /// Whether it has a glyph for `c` followed by variation selector `selector` (its own, or the character's).
    pub(crate) fn has_variant(&self, c: char, selector: char) -> bool {
        self.font.charmap().map_unicode_variant(c, selector).is_some() || self.has_char(c)
    }

    /// The normalised coordinates (F2Dot14 bits, one per axis) of its axes set to `values` (user units; axes it
    /// lacks are left out, values are clamped to their axes): none for a font with no axes or none set.
    pub(crate) fn coords(&self, values: &[([u8; 4], f32)]) -> Rc<[i16]> {
        let key: Vec<([u8; 4], u32)> = values.iter().map(|(t, v)| (*t, v.to_bits())).collect();
        if let Some(c) = self.coords.borrow().get(&key) {
            return c.clone();
        }
        let coords: Rc<[i16]> = if self.font.axes().is_empty() || values.is_empty() {
            Rc::from([])
        } else {
            let variations = values.iter().map(|(t, v)| (Tag::new(t), *v));
            let instance = self.font.instance_builder().variations(variations).build();
            let c: Vec<i16> = instance.normalized_coords().iter().map(|c| c.to_bits()).collect();
            if c.iter().all(|&v| v == 0) { Rc::from([]) } else { Rc::from(c) }
        };
        self.coords.borrow_mut().insert(key, coords.clone());
        coords
    }

    /// The font at `coords`.
    pub(crate) fn instance(&self, coords: &Rc<[i16]>) -> Font {
        if coords.is_empty() {
            return self.font.clone();
        }
        if let Some(f) = self.instances.borrow().get(coords) {
            return f.clone();
        }
        let f = self.font.instance_builder().normalized_coords(coords.iter().map(|&c| F2Dot14::from_bits(c))).build();
        self.instances.borrow_mut().insert(coords.clone(), f.clone());
        f
    }

    /// Its metrics at `coords` and `size` px: the line its OS/2 table asks for (USE_TYPO_METRICS), else hhea's,
    /// else the typographic one, else the clipping one (no gap); how far its glyphs reach (its head table's box); the underline from `post` and the strikeout from
    /// OS/2 (Skia's defaults where it has none: an underline a ninth of the size below, a strikeout six
    /// twenty-firsts above, an eighteenth thick), its x-height and cap height (from OS/2, else its `x` and `H`).
    pub(crate) fn metrics(&self, coords: &Rc<[i16]>, size: f32) -> FontMetrics {
        let font = self.instance(coords);
        let metrics = font.metrics().scaled_f32(size);
        let (ascent, descent, leading) = match metrics.h_line() {
            Some(line) => (line.ascender, -line.descender, line.line_gap.max(0.0)),
            None => (0.0, 0.0, 0.0),
        };
        let style = font.style_metrics().scaled_f32(size);
        let fallback_thickness = size / 18.0;
        let (underline_position, underline_thickness) = match style.underline() {
            Some(u) => {
                let thickness = if u.thickness > 0.0 { u.thickness } else { fallback_thickness };
                // Its position is the stroke's top, y up: its middle half a thickness lower.
                (thickness / 2.0 - u.position, thickness)
            }
            None => (size / 9.0, fallback_thickness),
        };
        let (strikeout_position, strikeout_thickness) = match style.strike_through() {
            Some(s) if s.position != 0.0 => {
                let thickness = if s.thickness > 0.0 { s.thickness } else { fallback_thickness };
                (thickness / 2.0 - s.position, thickness)
            }
            _ => (-size * 6.0 / 21.0, fallback_thickness),
        };
        let glyph_top = |c: char| {
            let glyph = font.charmap().map_unicode(c)?;
            let extents = font.glyph_metrics().scaled_f32(size).extents(glyph)?;
            Some(extents.y_bearing)
        };
        // The head table's box: what its glyphs reach (Skia's top and bottom).
        let bounds = metrics.bounds();
        let (top, bottom) = (bounds.y_max.max(ascent), (-bounds.y_min).max(descent));
        let x_height = metrics.x_height().filter(|h| *h > 0.0).or_else(|| glyph_top('x')).unwrap_or(0.0);
        let cap_height = metrics.cap_height().filter(|h| *h > 0.0).or_else(|| glyph_top('H')).unwrap_or(0.0);
        FontMetrics {
            ascent,
            descent,
            leading,
            top,
            bottom,
            underline_position,
            underline_thickness,
            strikeout_position,
            strikeout_thickness,
            x_height,
            cap_height,
        }
    }

    /// Where a glyph's ink reaches from its origin at `coords` and `size` px, y down: its outline's bounds, its
    /// color glyph's clip box, its bitmap's extents in the strike for the size; grown by a fake bold's stroke,
    /// slanted by a fake italic. Empty for an empty glyph or one it hasn't.
    pub(crate) fn glyph_bounds(&self, coords: &Rc<[i16]>, size: f32, glyph: u32, synthesis: GlyphSynthesis) -> Rect {
        let font = self.instance(coords);
        let Some(e) = font.glyph_metrics().scaled_f32(size).extents(GlyphId::new(glyph)) else {
            return Rect::Zero;
        };
        if e.width <= 0.0 && e.height <= 0.0 {
            return Rect::Zero;
        }
        let mut r = Rect::new(e.x_bearing, -e.y_bearing, e.x_bearing + e.width, -e.y_bearing + e.height);
        if synthesis.embolden && self.color == GlyphColorKind::Outline {
            let grow = fake_bold_strength(size) / 2.0;
            r = Rect::new(r.left - grow, r.top - grow, r.right + grow, r.bottom + grow);
        }
        if synthesis.skew != 0.0 {
            // A point y below the baseline moves y · skew to the right (above it, -y · -skew).
            let (a, b) = (r.top * synthesis.skew, r.bottom * synthesis.skew);
            r = Rect::new(r.left + a.min(b), r.top, r.right + a.max(b), r.bottom);
        }
        r
    }
}

/// The fonts read so far, by id (a font that can't be read is remembered as such).
#[derive(Default)]
pub(crate) struct Faces {
    faces: RefCell<HashMap<FontId, Option<Rc<Face>>>>,
}

impl Faces {
    pub(crate) fn get(&self, id: FontId) -> Option<Rc<Face>> {
        if let Some(f) = self.faces.borrow().get(&id) {
            return f.clone();
        }
        let face = Face::load(id).map(Rc::new);
        self.faces.borrow_mut().insert(id, face.clone());
        face
    }
}
