//! Text (C++'s drawText, drawTextBrush, drawTextShadow and the atlas's upload in flush): a run's glyphs rasterized
//! into the glyph atlas (the glyph module's cache), each a GLYPH primitive sampling its page, white plus alpha tinted
//! by the paint, color glyphs as they are; under a transform that enlarges it, laid out that many times bigger; a
//! shadow from pre-blurred glyphs.
//!
//! The atlas's pages are RGBA8 textures, as C++'s: a coverage page's texels go up as white with the coverage for
//! alpha (what the shader reads of a glyph that takes the paint), a color page's as they are (straight alpha).

use super::{Inputs, Renderer};
use crate::device::{Format, IRect, Texture, TextureId, TextureUsage};
use crate::glyph::{AtlasPage, GlyphCache, GlyphQuad, GlyphRequest, PageFormat};
use crate::shaders::{GLYPH, Prim};
use kompoz_graphics::{Brush, Color, Offset, Rect};
use kompoz_ui::graphics_context::TextDraw;

/// The glyphs (C++'s TextSource: the fonts read, the glyphs rasterized onto the atlas), the atlas's pages on the
/// GPU by page index, and the quads of the text being drawn.
#[derive(Default)]
pub(super) struct TextState {
    pub(super) glyphs: GlyphCache,
    pub(super) pages: Vec<Option<Texture>>,
    pub(super) quads: Vec<GlyphQuad>,
}

/// Px each way a glyph may be at most when laid out bigger under a transform (C++'s 384 px a font size).
const MOST_ENLARGED: f32 = 384.0;

/// How many times bigger than its pixels text of `font_px` px is laid out under a transform that enlarges it by
/// `enlarged`: as much, unless its glyphs would no longer fit on the atlas (then they are enlarged after, a little
/// soft). C++'s textScale.
fn text_scale(enlarged: f32, font_px: f32) -> f32 {
    enlarged.min(1.0f32.max(MOST_ENLARGED / font_px.max(1.0)))
}

/// A color as 8-bit straight sRGB (a COLR glyph's foreground).
fn straight8(color: Color) -> [u8; 4] {
    let argb = color.to_argb();
    [(argb >> 16) as u8, (argb >> 8) as u8, argb as u8, (argb >> 24) as u8]
}

impl Renderer {
    /// Page `index`'s texture, made (and the page marked to go up whole) if it wasn't.
    fn page_texture(&mut self, index: usize) -> Option<TextureId> {
        if self.text.pages.len() <= index {
            self.text.pages.resize_with(index + 1, || None);
        }
        if self.text.pages[index].is_none() {
            let size = AtlasPage::SIZE as i32;
            self.text.pages[index] = self.device.create_texture(size, size, Format::Rgba8, TextureUsage::SAMPLED);
            self.text.glyphs.atlas_mut().page_mut(index).mark_all_dirty();
        }
        self.text.pages[index].as_ref().map(Texture::id)
    }

    /// What changed on the glyph atlas since the last flush, uploaded to its pages (coverage as white with that
    /// alpha).
    pub(super) fn upload_atlas(&mut self) {
        let mut expanded = Vec::new();
        for i in 0..self.text.glyphs.atlas().page_count() {
            let Some(texture) = self.page_texture(i) else { continue };
            let page = self.text.glyphs.atlas_mut().page_mut(i);
            let d = page.take_dirty();
            if d.is_empty() {
                continue;
            }
            let region = IRect::new(d.x0 as i32, d.y0 as i32, d.width() as i32, d.height() as i32);
            let bytes = page.rect_bytes(d);
            match page.format() {
                PageFormat::Rgba => self.device.upload(texture, region, bytes, page.stride()),
                PageFormat::Coverage => {
                    let (w, h, stride) = (d.width() as usize, d.height() as usize, page.stride());
                    expanded.clear();
                    expanded.reserve(w * h * 4);
                    for row in 0..h {
                        for &a in &bytes[row * stride..row * stride + w] {
                            expanded.extend_from_slice(&[255, 255, 255, a]);
                        }
                    }
                    self.device.upload(texture, region, &expanded, w * 4);
                }
            }
        }
    }

    /// Between frames: the atlas's pages unused longest cleared for reuse (uploaded again whole when next drawn
    /// from), the glyphs that were on them forgotten.
    pub(super) fn end_frame_text(&mut self) {
        self.text.glyphs.end_frame();
    }

    /// `text`'s glyphs laid out from (x, y) (the target's px) `k` times bigger, a shadow's blurred, into the quads
    /// (C++'s TextSource::layout); whether there are any.
    fn lay_out(
        &mut self,
        text: &TextDraw,
        (x, y): (f32, f32),
        k: f32,
        shadow: Option<f32>,
        foreground: [u8; 4],
    ) -> bool {
        // The cache places glyphs at `origin · scale + glyph · scale`: the origin in the text's units (the target's
        // px at 1 a unit, as a renderer outside layers draws).
        let s = self.scale;
        let request = GlyphRequest { origin: Offset::new(x / s, y / s), scale: s * k, shadow, foreground };
        let mut quads = std::mem::take(&mut self.text.quads);
        quads.clear();
        self.text.glyphs.layout(text, &request, &mut quads);
        self.text.quads = quads;
        !self.text.quads.is_empty()
    }

    /// The quads laid out last, each a GLYPH primitive painted as `paint` is, laid out `enlarged` times bigger
    /// than drawn.
    fn glyphs(&mut self, paint: &Prim, enlarged: f32) {
        let page = AtlasPage::SIZE as f32;
        let quads = std::mem::take(&mut self.text.quads);
        for g in &quads {
            let Some(texture) = self.page_texture(g.page) else { continue };
            let mut p = *paint;
            p.bounds = [g.x0, g.y0, g.x1, g.y1];
            p.shape = p.bounds;
            p.uv = g.uv();
            p.params = [if g.color { 1.0 } else { 0.0 }, 0.0, 0.5 / page, 0.5 / page];
            if enlarged != 1.0 {
                p.xform = [1.0 / enlarged, 0.0, 0.0, 1.0 / enlarged]; // (laid out that many times bigger)
            }
            self.add(GLYPH, &p, Inputs { texture: Some(texture), ..Inputs::default() });
        }
        self.text.quads = quads;
    }

    /// Where `text`'s baseline origin is in the target's px, and how many times bigger than that it is laid out
    /// (the transform's enlargement there, as text_scale allows).
    fn text_origin(&self, text: &TextDraw) -> (f32, f32, f32) {
        let s = self.scale;
        let (x, y) = (text.baseline.x * s - self.ox, text.baseline.y * s - self.oy);
        (x, y, text_scale(self.raster_scale(x, y), text.font_size() * s))
    }
}

/// A run of text in `color`.
pub(super) fn draw_text(r: &mut Renderer, text: &TextDraw, color: Color) {
    if color.alpha() <= 0.0 {
        return;
    }
    let (x, y, k) = r.text_origin(text);
    if r.lay_out(text, (x, y), k, None, straight8(color)) {
        let paint = r.painted(&Brush::from(color), Rect::Zero, 1.0, 1.0);
        r.glyphs(&paint, k);
    }
}

/// Text painted with `brush` over `brush_area`.
pub(super) fn draw_text_brush(r: &mut Renderer, text: &TextDraw, brush: &Brush, brush_area: Rect) {
    let (x, y, k) = r.text_origin(text);
    // (A COLR glyph's foreground: FreeType's opaque black, which C++ drew text with a brush in.)
    if r.lay_out(text, (x, y), k, None, [0, 0, 0, 255]) {
        let paint = r.painted(brush, brush_area, 1.0, k);
        r.glyphs(&paint, k);
    }
}

/// Text's shadow, its glyphs (but color ones) blurred by `blur_radius` px.
pub(super) fn draw_text_shadow(r: &mut Renderer, text: &TextDraw, color: Color, blur_radius: f32) {
    if color.alpha() <= 0.0 {
        return;
    }
    let (x, y, k) = r.text_origin(text);
    if r.lay_out(text, (x, y), k, Some(blur_radius.max(0.0)), straight8(color)) {
        let paint = r.painted(&Brush::from(color), Rect::Zero, 1.0, 1.0);
        r.glyphs(&paint, k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_enlarged_as_far_as_the_atlas_allows() {
        assert_eq!(text_scale(1.0, 14.0), 1.0);
        assert_eq!(text_scale(2.0, 14.0), 2.0);
        // 100 px text enlarged 8 times: no more than 384 px.
        assert_eq!(text_scale(8.0, 100.0), 3.84);
        // Never smaller than drawn.
        assert_eq!(text_scale(8.0, 1000.0), 1.0);
        assert_eq!(text_scale(0.5, 14.0), 0.5);
    }

    #[test]
    fn foregrounds_are_straight_srgb_bytes() {
        assert_eq!(straight8(Color::new(1.0, 0.0, 0.0, 0.5)), [255, 0, 0, 128]);
    }
}
