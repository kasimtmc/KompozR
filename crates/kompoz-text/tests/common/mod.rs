//! The test fonts (`tests/fonts/`, see its README) and resolvers over them.

#![allow(dead_code)]

use kompoz_text::font::FontFamilyResolverImpl;
use kompoz_text::shape::{FontRun, FontRunRequest, ShapeRequest, ShapedRun};
use kompoz_text::*;
use std::path::PathBuf;
use std::rc::Rc;

pub const KOMPOZ_TEST: &str = "KompozTest.ttf";
pub const NOTO_SANS: &str = "NotoSans-Subset.ttf";
pub const NOTO_ARABIC: &str = "NotoSansArabic-Subset.ttf";
pub const NOTO_DEVANAGARI: &str = "NotoSansDevanagari-Subset.ttf";
pub const NOTO_HEBREW: &str = "NotoSansHebrew-Subset.ttf";
pub const NOTO_EMOJI: &str = "NotoColorEmoji-Subset.ttf";
pub const COLR_V0: &str = "KompozCOLRv0.ttf";
pub const COLR_V1: &str = "KompozCOLRv1.ttf";
pub const SBIX: &str = "KompozSbix.ttf";
pub const CANTARELL: &str = "Cantarell-VF-Subset.otf";

/// A test font's path.
pub fn font_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fonts").join(name)
}

/// A test font's bytes.
pub fn font_bytes(name: &str) -> Vec<u8> {
    std::fs::read(font_path(name)).expect("a test font")
}

/// A resolver over the test fonts only, sans-serif (the default) being `sans`: Noto Sans for Latin text, Noto Sans
/// Arabic, Devanagari and Hebrew for their scripts, Noto Color Emoji for emoji, Kompoz Test registered last (the
/// last fallback: it has U+2764 as text).
pub fn resolver_with_default(sans: &str) -> Rc<FontFamilyResolverImpl> {
    let resolver = Rc::new(FontFamilyResolverImpl::registered_only());
    let c = resolver.collection();
    for f in [NOTO_SANS, NOTO_ARABIC, NOTO_DEVANAGARI, NOTO_HEBREW, NOTO_EMOJI, KOMPOZ_TEST, COLR_V0, COLR_V1, SBIX] {
        c.register_file(font_path(f)).expect("a test font");
    }
    c.set_generic_families(GenericFontFamily::SansSerif, &[sans]);
    c.set_emoji_families(&["Noto Color Emoji"]);
    c.set_fallback_families(*b"Arab", None, &["Noto Sans Arabic"]);
    c.set_fallback_families(*b"Deva", None, &["Noto Sans Devanagari"]);
    c.set_fallback_families(*b"Hebr", None, &["Noto Sans Hebrew"]);
    resolver
}

/// The test fonts' resolver with Noto Sans as the default family.
pub fn resolver() -> Rc<FontFamilyResolverImpl> {
    resolver_with_default("Noto Sans")
}

/// The default family's typeface, regular.
pub fn default_typeface(resolver: &FontFamilyResolverImpl) -> Typeface {
    resolver.typeface(None, FontWeight::Normal, FontStyle::Normal, FontSynthesis::All)
}

/// The font runs of all of `text` in `typeface` at `size` px.
pub fn runs(backend: &dyn FontBackend, text: &str, typeface: &Typeface, size: f32) -> Vec<FontRun> {
    let locales = LocaleList::empty();
    backend.font_runs(&FontRunRequest {
        text,
        range: 0..text.len(),
        typeface,
        size,
        density: kompoz_graphics::Density::default(),
        locales: &locales,
    })
}

/// `text[range]` shaped in `run`'s face.
pub fn shape_run(backend: &dyn FontBackend, text: &str, run: &FontRun, rtl: bool) -> ShapedRun {
    shape_with(backend, text, run, rtl, 0.0, 1.0, &[])
}

pub fn shape_with(
    backend: &dyn FontBackend,
    text: &str,
    run: &FontRun,
    rtl: bool,
    letter_spacing: f32,
    scale_x: f32,
    features: &[FontFeature],
) -> ShapedRun {
    backend.shape(&ShapeRequest {
        text,
        range: run.range.clone(),
        font: &run.font,
        rtl,
        script: None,
        language: None,
        features,
        letter_spacing,
        scale_x,
    })
}

/// The family a run's font is of.
pub fn family_of(resolver: &FontFamilyResolverImpl, font: FontId) -> String {
    let c = resolver.collection();
    for name in c.family_names() {
        for (w, s) in [(FontWeight::Normal, FontStyle::Normal), (FontWeight::Bold, FontStyle::Italic)] {
            if c.face_by_name(&name, w, s).is_some_and(|f| f.font == font) {
                return name;
            }
        }
    }
    format!("{font:?}")
}

pub fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}
