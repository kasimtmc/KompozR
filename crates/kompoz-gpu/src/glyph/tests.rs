//! The glyph rasterizer, outlines, styles, blur, colour glyphs, atlas and cache, on the CPU.

mod atlas;
mod cache;
mod color;
mod freetype;
mod outline;
mod raster;

use crate::glyph::cache::Face;
use std::sync::Arc;

/// A test font's path from the crate's directory, made absolute.
fn font_path(path: &str) -> String {
    format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))
}

/// A test font's bytes.
fn font_bytes(path: &str) -> Arc<Vec<u8>> {
    Arc::new(std::fs::read(font_path(path)).expect("a test font"))
}

/// A test font as a face.
fn face(path: &str) -> Face {
    Face::new(font_bytes(path), 0, &[]).expect("a font")
}
