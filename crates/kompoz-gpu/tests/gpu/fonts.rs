//! Text for the GPU tests as the C++ Kompoz's tests drew it (its FontSystem's layout): the bundled Noto Sans
//! (tests/fonts: Regular, Medium, Bold), shaped by kompoz-text's HarfBuzz backend (harfrust) at a whole number of
//! pixels (`round(size · scale)`, FreeType's size), each glyph's advance, kerning and offsets in 64ths of a pixel as
//! HarfBuzz's FreeType font rounds them (advances from FreeType, GPOS in HarfBuzz's scale: a pen landing on half a
//! pixel goes the way it went for C++), letter spacing after each cluster (C++'s, not Android's half before and half
//! after).

use kompoz_graphics::{Density, Offset, Rect, Size};
use kompoz_text::font::{FontFamilyResolverImpl, FontId};
use kompoz_text::shape::{FontRunRequest, ShapeRequest};
use kompoz_text::style::TextMotion;
use kompoz_text::*;
use std::rc::Rc;

/// A font as C++'s tests gave one (Font{size}, its weight and letter spacing set): dp.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Font {
    pub size: f32,
    pub weight: FontWeight,
    pub letter_spacing: f32,
}

impl Font {
    /// Regular, `size` dp.
    pub fn new(size: f32) -> Font {
        Font { size, weight: FontWeight::Normal, letter_spacing: 0.0 }
    }

    /// Bold.
    pub fn bold(size: f32) -> Font {
        Font { weight: FontWeight::Bold, ..Font::new(size) }
    }

    /// Letters `spacing` dp apart besides their advances.
    pub fn spaced(self, spacing: f32) -> Font {
        Font { letter_spacing: spacing, ..self }
    }
}

thread_local! {
    static RESOLVER: Rc<FontFamilyResolverImpl> = {
        let resolver = Rc::new(FontFamilyResolverImpl::registered_only());
        let dir = crate::files::crate_dir().join("tests").join("fonts");
        let c = resolver.collection();
        for f in ["Regular.ttf", "Medium.ttf", "Bold.ttf"] {
            c.register(std::sync::Arc::new(crate::files::read(&dir.join(f)).expect("a bundled font")));
        }
        c.set_generic_families(GenericFontFamily::SansSerif, &["Noto Sans"]);
        resolver
    };
}

/// How HarfBuzz's FreeType font (hb-ft, FT_LOAD_NO_HINTING) at `px` px puts a face's lengths in 26.6: a glyph's
/// advance from FreeType's (its hmtx advance scaled by the size's x_scale, rounded to 16.16 and then to 26.6), what
/// GPOS adds to it and its offsets in HarfBuzz's own scale (the font's units times x_scale · upem, rounded).
struct HbFt {
    upem: i64,
    x_scale: i64,
    em_mult: i64,
    px: f32,
    advances: Vec<i64>,
}

impl HbFt {
    fn new(font: FontId, px: f32) -> HbFt {
        use skrifa::MetadataProvider;
        let data = kompoz_text::font::FontRegistry::get(font).expect("a registered font");
        let face = skrifa::FontRef::from_index(data.bytes(), data.index).expect("a font");
        let upem = face
            .metrics(skrifa::instance::Size::unscaled(), skrifa::instance::LocationRef::default())
            .units_per_em as i64;
        let metrics = face.glyph_metrics(skrifa::instance::Size::unscaled(), skrifa::instance::LocationRef::default());
        let advances = (0..metrics.glyph_count())
            .map(|g| metrics.advance_width(skrifa::GlyphId::new(g)).unwrap_or(0.0).round() as i64)
            .collect();
        // FT_Set_Pixel_Sizes: FT_DivFix(px << 6, upem).
        let x_scale = (((px as i64) << 6 << 16) + upem / 2) / upem;
        // hb_ft_font_create: its scale x_scale · upem (16.16 rounded), em_mult = scale << 16 / upem.
        let hb_scale = (x_scale * upem + (1 << 15)) >> 16;
        HbFt { upem, x_scale, em_mult: (hb_scale << 16) / upem, px, advances }
    }

    /// A length in px (units · px / upem) back in the font's units.
    fn units(&self, v: f32) -> i64 {
        (v * self.upem as f32 / self.px).round() as i64
    }

    /// Font units as HarfBuzz scales them (em_scale), 26.6.
    fn em_scale(&self, units: i64) -> i64 {
        (units * self.em_mult + (1 << 15)) >> 16
    }

    /// Glyph `id`'s advance as FreeType gives HarfBuzz it, 26.6.
    fn advance(&self, id: u32) -> i64 {
        let units = self.advances.get(id as usize).copied().unwrap_or(0);
        let fixed = (units * self.x_scale + 32).div_euclid(64); // (FT_MulDiv(units, x_scale, 64): 16.16)
        (fixed + (1 << 9)) >> 10
    }

    /// A shaped glyph's advance (px: its hmtx advance and what GPOS added, as harfrust gives it) in 26.6, as hb-ft
    /// gives it.
    fn shaped_advance(&self, id: u32, x_advance: f32) -> i64 {
        let total = self.units(x_advance);
        if total == 0 {
            return 0; // (a mark: no advance)
        }
        let own = self.advances.get(id as usize).copied().unwrap_or(0);
        self.advance(id) + self.em_scale(total - own)
    }
}

/// `text` (one line, left to right) in `font` with its baseline starting at `baseline` (dp), at `scale` px per dp:
/// what C++'s drawText(text, baseline, font, …) drew, as a TextDraw in px.
pub fn text_at(text: &str, baseline: Offset, font: Font, scale: f32) -> TextDraw {
    RESOLVER.with(|resolver| {
        let px = (font.size * scale).round().max(1.0);
        let spacing = font.letter_spacing * scale; // (px after each cluster)
        let typeface = resolver.typeface(None, font.weight, FontStyle::Normal, FontSynthesis::All);
        let backend = resolver.harfrust_backend();
        let locales = LocaleList::empty();
        let runs = backend.font_runs(&FontRunRequest {
            text,
            range: 0..text.len(),
            typeface: &typeface,
            size: px,
            density: Density::default(),
            locales: &locales,
        });
        let mut pen = 0.0f32;
        let mut ink: Option<Rect> = None;
        let mut glyph_runs = Vec::new();
        let mut height = 0.0f32;
        for run in &runs {
            let shaped = backend.shape(&ShapeRequest {
                text,
                range: run.range.clone(),
                font: &run.font,
                rtl: false,
                script: None,
                language: None,
                features: &[],
                letter_spacing: 0.0,
                scale_x: 1.0,
                linear: false,
            });
            height = height.max(backend.metrics(&run.font).line_height());
            let hb = HbFt::new(run.font.font, px);
            let in_px = |v: i64| v as f32 / 64.0;
            let mut glyphs = Vec::with_capacity(shaped.glyphs.len());
            for (i, g) in shaped.glyphs.iter().enumerate() {
                let x = pen + in_px(hb.em_scale(hb.units(g.x_offset)));
                let y = -in_px(hb.em_scale(hb.units(g.y_offset)));
                let b = backend.glyph_bounds(&run.font, g.id);
                if b.width() > 0.0 || b.height() > 0.0 {
                    let placed = Rect::new(x + b.left, y + b.top, x + b.right, y + b.bottom);
                    ink = Some(ink.map_or(placed, |r| r.union(placed)));
                }
                glyphs.push(Glyph { id: g.id, x, y, cluster: g.cluster as u32 });
                let mut advance = in_px(hb.shaped_advance(g.id, g.x_advance));
                if shaped.glyphs.get(i + 1).is_none_or(|n| n.cluster != g.cluster) {
                    advance += spacing; // (a cluster's last glyph)
                }
                pen += advance;
            }
            glyph_runs.push(GlyphRun {
                font: run.font.font,
                coords: run.font.coords.clone(),
                size: run.font.size,
                synthesis: run.font.synthesis,
                scale_x: 1.0,
                skew_x: 0.0,
                color: backend.color_kind(run.font.font),
                text_motion: TextMotion::Static,
                glyphs,
            });
        }
        let style = TextRunStyle { font_weight: font.weight, letter_spacing: spacing, ..TextRunStyle::new(px) };
        TextDraw::new(
            text,
            Offset::new(baseline.x * scale, baseline.y * scale),
            Size::new(pen, height),
            ink.unwrap_or(Rect::Zero),
            false,
            Rc::new(style),
            Rc::from(glyph_runs),
        )
    })
}

/// The same at 1 px per dp.
pub fn text(s: &str, baseline: Offset, font: Font) -> TextDraw {
    text_at(s, baseline, font, 1.0)
}
