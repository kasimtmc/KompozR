//! Text on the web (C++'s font_system for Emscripten): the page has no fonts KompozR can read, so the app's module
//! bundles Noto Sans (Regular, Medium, Bold: C++'s bundled fonts; `fonts/`), registered as the default family of
//! every generic one; and what they lack — emoji, CJK, Arabic, every other script — is measured and drawn by the
//! browser (kompoz-text's [platform glyphs](kompoz_text::font::platform), on its canvas 2D: the browser's own fonts
//! and fallback, emoji in colour), a cluster at a time (a word in a script whose letters join), its pixels cached
//! by the renderer as any glyph's.

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
use super::sys;
use kompoz_graphics::Rect;
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
use kompoz_text::font::platform::{PlatformGlyphImage, PlatformGlyphs, set_platform_glyphs};
use kompoz_text::font::platform::{PlatformGlyphMetrics, PlatformStyle};
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
use kompoz_text::font::{FontFamilyResolverImpl, GenericFontFamily};
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
use std::sync::Arc;

/// The bundled fonts.
pub const NOTO_SANS_REGULAR: &[u8] = include_bytes!("fonts/NotoSans-Regular.ttf");
pub const NOTO_SANS_MEDIUM: &[u8] = include_bytes!("fonts/NotoSans-Medium.ttf");
pub const NOTO_SANS_BOLD: &[u8] = include_bytes!("fonts/NotoSans-Bold.ttf");

/// The bundled family's name.
pub const FAMILY: &str = "Noto Sans";

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
/// The bundled fonts registered with the thread's font collection as every generic family — unless the app
/// registered fonts of its own before it ran (then its families are its own to name), or `KOMPOZ_WEB_FONTS=0` (no
/// fonts: every glyph the browser's) — and the browser's text engine as the platform's glyphs.
pub(super) fn start() {
    let collection = FontFamilyResolverImpl::system().collection().clone();
    let own = !collection.family_names().is_empty();
    if !own && kompoz_runtime::environment::var("KOMPOZ_WEB_FONTS").as_deref() != Some("0") {
        for bytes in [NOTO_SANS_REGULAR, NOTO_SANS_MEDIUM, NOTO_SANS_BOLD] {
            collection.register(Arc::new(bytes));
        }
        for generic in [
            GenericFontFamily::SansSerif,
            GenericFontFamily::Serif,
            GenericFontFamily::Monospace,
            GenericFontFamily::Cursive,
        ] {
            collection.set_generic_families(generic, &[FAMILY]);
        }
    }
    set_platform_glyphs(Some(Arc::new(BrowserGlyphs)));
}

/// The CSS font a style is drawn in: the browser's sans-serif (its emoji fonts first for colour).
pub fn css_font(style: PlatformStyle, size: f32) -> String {
    let families = if style.color {
        r#""Noto Color Emoji", "Apple Color Emoji", "Segoe UI Emoji", sans-serif"#
    } else {
        "sans-serif"
    };
    format!("{}{} {}px {families}", if style.italic { "italic " } else { "" }, style.weight, size)
}

/// The browser's measures (web.js's seven numbers: advance; ink left, right, ascent, descent; the font's ascent and
/// descent) as a glyph's.
pub fn metrics_of(answer: &str) -> PlatformGlyphMetrics {
    let n: Vec<f32> = answer.split_whitespace().map(|v| v.parse().unwrap_or(0.0)).collect();
    if n.len() < 7 {
        return PlatformGlyphMetrics::default();
    }
    PlatformGlyphMetrics { advance: n[0], ink: Rect::new(-n[1], -n[3], n[2], n[4]), ascent: n[5], descent: n[6] }
}

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
/// The browser's canvas 2D as the platform's text engine.
struct BrowserGlyphs;

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
impl PlatformGlyphs for BrowserGlyphs {
    fn measure(&self, text: &str, style: PlatformStyle, size: f32) -> PlatformGlyphMetrics {
        metrics_of(&sys::text_measure(text, &css_font(style, size)))
    }

    /// Drawn on a canvas as big as its ink and a pixel around it, its origin where its ink's left and top put it.
    fn draw(&self, text: &str, style: PlatformStyle, px: u32) -> Option<PlatformGlyphImage> {
        let font = css_font(style, px as f32);
        let m = metrics_of(&sys::text_measure(text, &font));
        let (left, top) = ((-m.ink.left).ceil() as i32 + 1, (-m.ink.top).ceil() as i32 + 1);
        let width = (left + m.ink.right.ceil() as i32 + 1).max(1) as u32;
        let height = (top + m.ink.bottom.ceil() as i32 + 1).max(1) as u32;
        if m.ink.width() <= 0.0 || m.ink.height() <= 0.0 {
            return None;
        }
        let rgba = sys::text_draw(text, &font, width, height, left as f64, top as f64)?;
        Some(PlatformGlyphImage { left: -left, top, width: width as usize, height: height as usize, rgba })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonts_and_measures() {
        let style = PlatformStyle { weight: 700, italic: true, color: false };
        assert_eq!(css_font(style, 14.0), "italic 700 14px sans-serif");
        let emoji = PlatformStyle { weight: 400, italic: false, color: true };
        assert!(css_font(emoji, 20.0).starts_with("400 20px \"Noto Color Emoji\""));
        let m = metrics_of("10 1 9 12 3 14 4");
        assert_eq!(m.advance, 10.0);
        assert_eq!(m.ink, Rect::new(-1.0, -12.0, 9.0, 3.0));
        assert_eq!((m.ascent, m.descent), (14.0, 4.0));
        assert_eq!(metrics_of(""), PlatformGlyphMetrics::default());
    }
}
