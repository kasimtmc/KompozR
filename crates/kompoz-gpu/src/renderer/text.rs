//! Text (C++'s drawText, drawTextBrush, drawTextShadow and the atlas's upload in flush): a run's glyphs rasterized
//! into the glyph atlas (the glyph module's cache), each a GLYPH primitive sampling its page, white plus alpha tinted
//! by the paint, color glyphs as they are; under a transform that enlarges it, laid out that many times bigger; a
//! shadow from pre-blurred glyphs.
//!
//! Drawn by stage 7's text package: until then text draws nothing and the atlas has nothing to upload.

use super::Renderer;
use crate::device::Texture;
use crate::glyph::{GlyphCache, GlyphQuad};
use kompoz_graphics::{Brush, Color, Rect};
use kompoz_ui::graphics_context::TextDraw;

/// The glyphs (C++'s TextSource: the fonts read, the glyphs rasterized onto the atlas), the atlas's pages on the
/// GPU by page index, and the quads of the text being drawn.
#[derive(Default)]
pub(super) struct TextState {
    pub(super) glyphs: GlyphCache,
    pub(super) pages: Vec<Option<Texture>>,
    pub(super) quads: Vec<GlyphQuad>,
}

impl Renderer {
    /// What changed on the glyph atlas since the last flush, uploaded to its pages. Uploaded by stage 7's text
    /// package: until then there is nothing (no text is drawn, so nothing is put on the atlas).
    pub(super) fn upload_atlas(&mut self) {
        let _ = (&self.text.glyphs, &self.text.pages, &self.text.quads);
    }
}

/// A run of text in `color`. Drawn by stage 7's text package: until then nothing.
pub(super) fn draw_text(r: &mut Renderer, text: &TextDraw, color: Color) {
    let _ = (r, text, color);
}

/// Text painted with `brush` over `brush_area`. Drawn by stage 7's text package: until then nothing.
pub(super) fn draw_text_brush(r: &mut Renderer, text: &TextDraw, brush: &Brush, brush_area: Rect) {
    let _ = (r, text, brush, brush_area);
}

/// Text's shadow, its glyphs blurred by `blur_radius` px. Drawn by stage 7's text package: until then nothing.
pub(super) fn draw_text_shadow(r: &mut Renderer, text: &TextDraw, color: Color, blur_radius: f32) {
    let _ = (r, text, color, blur_radius);
}
