//! Glyphs for the GPU renderer, rasterized on the CPU as the C++ Kompoz's FreeType rasterized them (its text goldens
//! are matched pixel for pixel): our own coverage rasterizer (FreeType's ftgrays, exact area per pixel), skrifa's
//! outlines light hinted at whole pixel sizes, the text's style (stretch and slant, fake bold and italic, strokes),
//! text shadows blurred on the CPU, COLR v0 and v1 colour glyphs painted on the CPU, bitmap strikes scaled to size;
//! packed into an atlas of coverage and colour pages that a renderer uploads as they change, by a cache that turns a
//! [`TextDraw`](kompoz_text::TextDraw) into quads on it.
//!
//! A renderer asks [`GlyphCache::layout`] for a text's quads, uploads each atlas page's dirty rectangle
//! ([`AtlasPage::take_dirty`], [`AtlasPage::rect_bytes`]; coverage pages a byte a texel, to be sampled as white with
//! that coverage for alpha, colour pages straight-alpha RGBA8), draws the quads, and calls
//! [`GlyphCache::end_frame`] once the frame has been drawn.

#![deny(unsafe_code)]

pub mod adjust;
pub mod atlas;
pub mod bitmap;
pub mod blur;
pub mod cache;
pub mod color;
pub mod outline;
pub mod raster;

#[cfg(test)]
mod tests;

pub use atlas::{AtlasPage, AtlasSlot, DirtyRect, GlyphAtlas, PageFormat};
pub use bitmap::{BitmapImage, bitmap_glyph};
pub use blur::{blur_coverage, blur_sigma, blurred};
pub use cache::{Face, GlyphCache, GlyphImage, GlyphKey, GlyphQuad, GlyphRequest, GlyphSpec};
pub use color::{ColorImage, PaintedGlyph, paint_colr};
pub use outline::{GlyphStroke, GlyphStyle, Hinting};
pub use raster::{Coverage, FillRule, FixedMatrix, FixedOutline, FixedPoint, PointTag};
