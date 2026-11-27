//! Glyphs the platform draws (C++'s `drawnByPlatform`, its web build's canvas 2D): where a platform's text engine
//! has what no font of the app's has — the web's browser, whose fonts KompozR can't read (emoji, CJK, Arabic and the
//! like beside the bundled Noto Sans) —, the characters no font has are measured and drawn by it. Such text is in a
//! *platform font* of its own (a [`FontId`] no registered font has, one for each weight and style, and one drawing
//! in colour for emoji), its glyphs its clusters (each grapheme cluster; a word of a script whose letters join, so
//! that the browser joins them), numbered as they come ([`glyph_id`]). The fallback picks it where no font has a
//! cluster ([`FontFallback`](super::fallback)); the shaper measures its glyphs through the platform; the GPU
//! renderer's glyph cache asks the platform for their pixels.

use super::registry::FontId;
use kompoz_graphics::Rect;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// How a platform font's text is drawn: its weight (100 to 900), italic or not, in colour (emoji: the platform's
/// colours) or as a mask (tinted by the text's colour, as an outline glyph is).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PlatformStyle {
    pub weight: u16,
    pub italic: bool,
    pub color: bool,
}

/// What the platform measures of a glyph at a size (px): its advance, how far its ink reaches from its origin (y
/// down), and its font's ascent and descent (both positive).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct PlatformGlyphMetrics {
    pub advance: f32,
    pub ink: Rect,
    pub ascent: f32,
    pub descent: f32,
}

/// A glyph's pixels as the platform drew them: straight-alpha RGBA, the top row first, `left` and `top` from the
/// glyph's origin to the image's top left (as a coverage image's: `top` up from the baseline).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PlatformGlyphImage {
    pub left: i32,
    pub top: i32,
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// A platform's text engine (the web platform's: its browser's canvas 2D).
pub trait PlatformGlyphs: Send + Sync {
    /// `text` (a glyph: a cluster, or a joined word) measured in `style` at `size` px.
    fn measure(&self, text: &str, style: PlatformStyle, size: f32) -> PlatformGlyphMetrics;
    /// `text` drawn in `style` at `px` px (white for a mask: its alpha the coverage); none if it draws nothing.
    fn draw(&self, text: &str, style: PlatformStyle, px: u32) -> Option<PlatformGlyphImage>;
}

/// The platform's engine, if it has one.
static GLYPHS: RwLock<Option<Arc<dyn PlatformGlyphs>>> = RwLock::new(None);

/// The platform's text engine from now on (none: what no font has is the fonts' missing glyph, as natively).
pub fn set_platform_glyphs(glyphs: Option<Arc<dyn PlatformGlyphs>>) {
    *GLYPHS.write().unwrap_or_else(|e| e.into_inner()) = glyphs;
}

/// The platform's text engine.
pub fn platform_glyphs() -> Option<Arc<dyn PlatformGlyphs>> {
    GLYPHS.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Platform fonts' ids: this, and the style's bits (no registered font has an id this high).
const BASE: u64 = 0xFFFF_0000_0000_0000;

/// The platform font of `style`.
pub fn platform_font(style: PlatformStyle) -> FontId {
    let weight = (style.weight.clamp(1, 1000) as u64) << 2;
    FontId(BASE | weight | (style.italic as u64) << 1 | style.color as u64)
}

/// The style of a platform font; none for a font of the app's.
pub fn platform_style(font: FontId) -> Option<PlatformStyle> {
    if font.0 & BASE != BASE {
        return None;
    }
    let bits = font.0 & !BASE;
    Some(PlatformStyle { weight: (bits >> 2) as u16, italic: bits & 2 != 0, color: bits & 1 != 0 })
}

struct Glyphs {
    texts: Vec<Arc<str>>,
    ids: HashMap<Arc<str>, u32>,
    measured: HashMap<(u32, PlatformStyle, u32), PlatformGlyphMetrics>,
}

fn glyphs() -> &'static Mutex<Glyphs> {
    static GLYPHS: OnceLock<Mutex<Glyphs>> = OnceLock::new();
    GLYPHS.get_or_init(|| Mutex::new(Glyphs { texts: Vec::new(), ids: HashMap::new(), measured: HashMap::new() }))
}

/// A platform font's glyph for `text` (numbered once, for as long as the process runs).
pub fn glyph_id(text: &str) -> u32 {
    let mut g = glyphs().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&id) = g.ids.get(text) {
        return id;
    }
    let id = g.texts.len() as u32;
    let text: Arc<str> = Arc::from(text);
    g.texts.push(text.clone());
    g.ids.insert(text, id);
    id
}

/// What a platform font's glyph draws.
pub fn glyph_text(id: u32) -> Option<Arc<str>> {
    glyphs().lock().unwrap_or_else(|e| e.into_inner()).texts.get(id as usize).cloned()
}

/// A glyph measured by the platform (each once a style and size); zeros without a platform engine.
pub fn measure(id: u32, style: PlatformStyle, size: f32) -> PlatformGlyphMetrics {
    let key = (id, style, (size + 0.0).to_bits());
    if let Some(m) = glyphs().lock().unwrap_or_else(|e| e.into_inner()).measured.get(&key) {
        return *m;
    }
    let (Some(engine), Some(text)) = (platform_glyphs(), glyph_text(id)) else {
        return PlatformGlyphMetrics::default();
    };
    let m = engine.measure(&text, style, size);
    glyphs().lock().unwrap_or_else(|e| e.into_inner()).measured.insert(key, m);
    m
}

/// A platform font's ascent and descent at a size (its glyphs' font's, as the platform measures a space).
pub fn font_metrics(style: PlatformStyle, size: f32) -> (f32, f32) {
    let m = measure(glyph_id(" "), style, size);
    (m.ascent, m.descent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_fonts_by_style_and_glyphs_by_text() {
        let style = PlatformStyle { weight: 700, italic: true, color: false };
        let font = platform_font(style);
        assert_eq!(platform_style(font), Some(style));
        let emoji = PlatformStyle { weight: 400, italic: false, color: true };
        assert_eq!(platform_style(platform_font(emoji)), Some(emoji));
        assert_ne!(platform_font(emoji), platform_font(style));
        assert_eq!(platform_style(FontId(3)), None);
        let a = glyph_id("漢");
        assert_eq!(glyph_id("漢"), a);
        assert_ne!(glyph_id("😀"), a);
        assert_eq!(glyph_text(a).as_deref(), Some("漢"));
    }
}
