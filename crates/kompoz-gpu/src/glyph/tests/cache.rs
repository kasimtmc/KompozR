//! The glyph cache: a TextDraw's glyphs as quads on the atlas, cached by face, size and style.

use super::font_path;
use crate::glyph::atlas::{AtlasPage, PageFormat};
use crate::glyph::cache::{GlyphCache, GlyphKey, GlyphQuad, GlyphRequest, GlyphSpec};
use crate::glyph::outline::{GlyphStroke, GlyphStyle, Hinting};
use kompoz_graphics::{DrawStyle, Offset, PathEffect, Rect, Size, Stroke, StrokeCap, StrokeJoin};
use kompoz_text::draw::{Glyph, GlyphColorKind, GlyphRun, GlyphSynthesis, TextDraw, TextRunStyle};
use kompoz_text::font::{FontId, FontRegistry};
use kompoz_text::style::TextMotion;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

/// A test font registered once (the registry keeps fonts by their memory).
fn registered(path: &str) -> FontId {
    static FONTS: OnceLock<Mutex<HashMap<String, FontId>>> = OnceLock::new();
    let mut fonts = FONTS.get_or_init(Default::default).lock().unwrap();
    *fonts.entry(path.to_string()).or_insert_with(|| {
        let bytes: Arc<[u8]> = std::fs::read(font_path(path)).unwrap().into();
        FontRegistry::register(bytes, 0)
    })
}

const REGULAR: &str = "tests/fonts/Regular.ttf";

/// A run of `chars` of `font` at `size` px, at the x positions given (y 0).
fn run(font: &str, kind: GlyphColorKind, size: f32, chars: &[(char, f32)]) -> GlyphRun {
    let bytes = std::fs::read(font_path(font)).unwrap();
    let skrifa_font = skrifa::Font::new(bytes, 0).unwrap();
    let glyphs = chars
        .iter()
        .enumerate()
        .map(|(i, &(c, x))| Glyph {
            id: skrifa_font.charmap().map_unicode(c).unwrap().to_u32(),
            x,
            y: 0.0,
            cluster: i as u32,
        })
        .collect();
    GlyphRun {
        font: registered(font),
        coords: Rc::from([]),
        size,
        synthesis: GlyphSynthesis::None,
        scale_x: 1.0,
        skew_x: 0.0,
        color: kind,
        text_motion: TextMotion::Static,
        glyphs,
    }
}

fn text(runs: Vec<GlyphRun>, style: TextRunStyle) -> TextDraw {
    TextDraw::new("text", Offset::Zero, Size::Zero, Rect::Zero, false, Rc::new(style), Rc::from(runs))
}

fn layout(cache: &mut GlyphCache, draw: &TextDraw, request: &GlyphRequest) -> Vec<GlyphQuad> {
    let mut out = Vec::new();
    cache.layout(draw, request, &mut out);
    out
}

#[test]
#[cfg_attr(miri, ignore)]
fn quads_are_placed_as_cpp_places_them() {
    let mut cache = GlyphCache::new();
    let draw = text(
        vec![run(REGULAR, GlyphColorKind::Outline, 24.0, &[('K', 0.0), ('o', 14.6), (' ', 27.0)])],
        TextRunStyle::new(24.0),
    );
    let quads = layout(&mut cache, &draw, &GlyphRequest::new(Offset::new(10.3, 30.6)));
    // The space draws nothing.
    assert_eq!(quads.len(), 2);
    let mut face = super::face(REGULAR);
    for (q, (c, x)) in quads.iter().zip([('K', 0.0f32), ('o', 14.6)]) {
        let glyph = face.font().charmap().map_unicode(c).unwrap().to_u32();
        let coverage = face.outline(24, Hinting::Light, glyph).unwrap().render();
        // C++'s round(x + glyph.x) + left, round(baseline - glyph.y) - top.
        let x0 = (10.3 + x).round() + coverage.left as f32;
        let y0 = 30.6f32.round() - coverage.top as f32;
        assert_eq!((q.x0, q.y0), (x0, y0), "{c}");
        assert_eq!((q.x1 - q.x0, q.y1 - q.y0), (coverage.width as f32, coverage.height as f32));
        assert_eq!((q.width as usize, q.height as usize), (coverage.width, coverage.height));
        assert!(!q.color);
        // Its texels on its page are the glyph's.
        let page = cache.atlas().page(q.page);
        assert_eq!(page.format(), PageFormat::Coverage);
        for row in 0..coverage.height {
            let at = (q.v as usize + row) * page.stride() + q.u as usize;
            assert_eq!(
                &page.pixels()[at..at + coverage.width],
                &coverage.alpha[row * coverage.width..][..coverage.width]
            );
        }
        let s = AtlasPage::SIZE as f32;
        assert_eq!(q.uv(), [q.u as f32 / s, q.v as f32 / s, (q.u + q.width) as f32 / s, (q.v + q.height) as f32 / s]);
    }
    // Drawn again: from the cache.
    let rasterized = cache.rasterized();
    assert_eq!(layout(&mut cache, &draw, &GlyphRequest::new(Offset::new(10.3, 30.6))), quads);
    assert_eq!(cache.rasterized(), rasterized);
    // Elsewhere: the same glyphs moved.
    let moved = layout(&mut cache, &draw, &GlyphRequest::new(Offset::new(20.3, 40.6)));
    assert_eq!(cache.rasterized(), rasterized);
    assert!(moved.iter().zip(&quads).all(|(m, q)| m.x0 == q.x0 + 10.0 && m.y0 == q.y0 + 10.0 && m.u == q.u));
}

#[test]
#[cfg_attr(miri, ignore)]
fn text_laid_out_bigger_is_drawn_bigger() {
    let mut cache = GlyphCache::new();
    let draw =
        text(vec![run(REGULAR, GlyphColorKind::Outline, 12.0, &[('H', 0.0), ('H', 8.4)])], TextRunStyle::new(12.0));
    let one = layout(&mut cache, &draw, &GlyphRequest::new(Offset::new(5.0, 20.0)));
    let two = layout(&mut cache, &draw, &GlyphRequest { scale: 2.0, ..GlyphRequest::new(Offset::new(5.0, 20.0)) });
    // At 24 px, from (10, 40), the second H at 16.8 more (17 rounded).
    let mut face = super::face(REGULAR);
    let glyph = face.font().charmap().map_unicode('H').unwrap().to_u32();
    let big = face.outline(24, Hinting::Light, glyph).unwrap().render();
    assert_eq!((two[0].x0, two[0].y0), (10.0 + big.left as f32, 40.0 - big.top as f32));
    assert_eq!(two[1].x0 - two[0].x0, 17.0);
    assert!(two[0].height > one[0].height);
}

#[test]
#[cfg_attr(miri, ignore)]
fn shadows_have_blurred_glyphs_and_no_colour_ones() {
    let mut cache = GlyphCache::new();
    let draw = text(
        vec![
            run(REGULAR, GlyphColorKind::Outline, 28.0, &[('S', 0.0)]),
            run("../kompoz-text/tests/fonts/KompozCOLRv0.ttf", GlyphColorKind::ColrV0, 28.0, &[('\u{E000}', 20.0)]),
        ],
        TextRunStyle::new(28.0),
    );
    let plain = layout(&mut cache, &draw, &GlyphRequest::new(Offset::new(10.0, 40.0)));
    assert_eq!(plain.len(), 2);
    assert!(!plain[0].color && plain[1].color);
    assert_eq!(cache.atlas().page(plain[1].page).format(), PageFormat::Rgba);
    let shadow =
        layout(&mut cache, &draw, &GlyphRequest { shadow: Some(4.0), ..GlyphRequest::new(Offset::new(10.0, 40.0)) });
    assert_eq!(shadow.len(), 1);
    // Padded by three sigma each way (sigma 2.81 for a radius of 4: 9 px).
    assert_eq!(shadow[0].x0, plain[0].x0 - 9.0);
    assert_eq!(shadow[0].width, plain[0].width + 18);
    // A shadow without a blur: the glyphs as they are, still without colour ones.
    let sharp =
        layout(&mut cache, &draw, &GlyphRequest { shadow: Some(0.0), ..GlyphRequest::new(Offset::new(10.0, 40.0)) });
    assert_eq!(sharp, plain[..1]);
}

#[test]
#[cfg_attr(miri, ignore)]
fn strokes_are_cached_apart() {
    let mut cache = GlyphCache::new();
    let runs = vec![run(REGULAR, GlyphColorKind::Outline, 24.0, &[('o', 0.0)])];
    let filled =
        layout(&mut cache, &text(runs.clone(), TextRunStyle::new(24.0)), &GlyphRequest::new(Offset::new(0.0, 30.0)));
    let style = TextRunStyle {
        draw_style: DrawStyle::Stroke(Stroke::new(2.0, 4.0, StrokeCap::Butt, StrokeJoin::Miter, None)),
        ..TextRunStyle::new(24.0)
    };
    let stroked = layout(&mut cache, &text(runs, style), &GlyphRequest::new(Offset::new(0.0, 30.0)));
    assert_eq!(cache.rasterized(), 2);
    assert_eq!((stroked[0].x0, stroked[0].y0), (filled[0].x0 - 1.0, filled[0].y0 - 1.0));
    assert_eq!(stroked[0].width, filled[0].width + 2);
}

#[test]
#[cfg_attr(miri, ignore)]
fn moving_text_is_placed_to_a_quarter_pixel_unhinted() {
    let mut cache = GlyphCache::new();
    let mut moving = run(REGULAR, GlyphColorKind::Outline, 20.0, &[('l', 0.0), ('l', 5.25), ('l', 10.5), ('l', 16.0)]);
    moving.text_motion = TextMotion::Animated;
    let quads =
        layout(&mut cache, &text(vec![moving], TextRunStyle::new(20.0)), &GlyphRequest::new(Offset::new(0.0, 20.0)));
    // Four offsets within a pixel: 0, a quarter, a half, 0 again (the last one from the cache).
    assert_eq!(cache.rasterized(), 3);
    let mut face = super::face(REGULAR);
    let glyph = face.font().charmap().map_unicode('l').unwrap().to_u32();
    let unhinted = face.outline(20, Hinting::None, glyph).unwrap();
    assert_eq!(quads[0].x0, unhinted.render().left as f32);
    assert_eq!(quads[3].x0 - quads[0].x0, 16.0);
    // A quarter of a pixel in: rasterized that far into its pixel, drawn from the whole pixel left of it.
    let mut shifted = unhinted.clone();
    shifted.translate(16, 0);
    assert_eq!(quads[1].x0, 5.0 + shifted.render().left as f32);
}

#[test]
#[cfg_attr(miri, ignore)]
fn colour_glyphs_painted_with_the_foreground_are_kept_by_it() {
    let mut cache = GlyphCache::new();
    let font = "src/glyph/testdata/KompozPaints.ttf";
    let draw = text(
        vec![run(font, GlyphColorKind::ColrV1, 20.0, &[('\u{E007}', 0.0), ('\u{E006}', 20.0)])],
        TextRunStyle::new(20.0),
    );
    let red = GlyphRequest { foreground: [255, 0, 0, 255], ..GlyphRequest::new(Offset::new(0.0, 20.0)) };
    let blue = GlyphRequest { foreground: [0, 0, 255, 255], ..red };
    let a = layout(&mut cache, &draw, &red);
    assert_eq!(cache.rasterized(), 2);
    let b = layout(&mut cache, &draw, &blue);
    // Only the glyph that paints with the foreground again.
    assert_eq!(cache.rasterized(), 3);
    assert_ne!((a[0].u, a[0].v), (b[0].u, b[0].v));
    assert_eq!((a[1].u, a[1].v), (b[1].u, b[1].v));
    let page = cache.atlas().page(b[0].page);
    let at = (b[0].v as usize + 5) * page.stride() + (b[0].u as usize + 5) * 4;
    assert_eq!(&page.pixels()[at..at + 4], &[0, 0, 255, 255]);
    layout(&mut cache, &draw, &red);
    assert_eq!(cache.rasterized(), 3);
}

#[test]
#[cfg_attr(miri, ignore)]
fn glyphs_whose_page_was_cleared_are_rasterized_again() {
    // Big glyphs fill six pages in a frame; only the last is drawn after; the oldest pages are cleared, and their
    // glyphs come back when drawn.
    let mut cache = GlyphCache::new();
    let letters: Vec<(char, f32)> = ('A'..='Z').chain('a'..='z').map(|c| (c, 0.0)).collect();
    let all = text(vec![run(REGULAR, GlyphColorKind::Outline, 450.0, &letters)], TextRunStyle::new(450.0));
    let last = text(vec![run(REGULAR, GlyphColorKind::Outline, 450.0, &[('z', 0.0)])], TextRunStyle::new(450.0));
    let first_quads = layout(&mut cache, &all, &GlyphRequest::new(Offset::Zero));
    let pages = cache.atlas().page_count();
    assert!(pages > 4, "{pages}");
    let rasterized = cache.rasterized();
    for _ in 0..3 {
        cache.end_frame();
        layout(&mut cache, &last, &GlyphRequest::new(Offset::Zero));
    }
    cache.end_frame();
    assert_eq!(cache.rasterized(), rasterized);
    let again = layout(&mut cache, &all, &GlyphRequest::new(Offset::Zero));
    let on_cleared = first_quads.iter().filter(|q| q.page < pages - 4).count();
    assert!(on_cleared > 0);
    assert_eq!(cache.rasterized(), rasterized + on_cleared);
    assert_eq!(again.len(), first_quads.len());
}

#[test]
#[cfg_attr(miri, ignore)]
fn fonts_that_are_not_registered_draw_nothing() {
    let mut cache = GlyphCache::new();
    let mut r = run(REGULAR, GlyphColorKind::Outline, 20.0, &[('a', 0.0)]);
    r.font = FontId(u64::MAX);
    assert!(layout(&mut cache, &text(vec![r], TextRunStyle::new(20.0)), &GlyphRequest::new(Offset::Zero)).is_empty());
}

#[test]
fn keys_tell_styles_apart() {
    let hash = |k: &GlyphKey| {
        let mut h = DefaultHasher::new();
        k.hash(&mut h);
        h.finish()
    };
    let plain = GlyphSpec::new(16);
    let a = GlyphKey::new(0, 5, &plain);
    let b = GlyphKey::new(0, 5, &GlyphSpec::new(16));
    assert_eq!(a, b);
    assert_eq!(hash(&a), hash(&b));
    let stroke = |path_effect: Option<PathEffect>| GlyphSpec {
        style: GlyphStyle {
            stroke: Some(GlyphStroke {
                width: 2,
                cap: StrokeCap::Butt,
                join: StrokeJoin::Round,
                miter: 4.0,
                path_effect,
            }),
            ..GlyphStyle::PLAIN
        },
        ..GlyphSpec::new(16)
    };
    let dashed = || Some(PathEffect::dash_path_effect(&[3.0, 2.0], 0.0));
    assert_ne!(GlyphKey::new(0, 5, &stroke(None)), a);
    assert_ne!(GlyphKey::new(0, 5, &stroke(None)), GlyphKey::new(0, 5, &stroke(dashed())));
    assert_eq!(GlyphKey::new(0, 5, &stroke(dashed())), GlyphKey::new(0, 5, &stroke(dashed())));
    assert_ne!(GlyphKey::new(0, 5, &GlyphSpec { blur: Some(3), ..GlyphSpec::new(16) }), a);
    assert_ne!(GlyphKey::new(0, 5, &GlyphSpec { subpixel: 2, ..GlyphSpec::new(16) }), a);
    assert_ne!(GlyphKey::new(0, 5, &GlyphSpec { hinted: false, ..GlyphSpec::new(16) }), a);
    assert_ne!(GlyphKey::new(1, 5, &plain), a);
    assert_ne!(GlyphKey::new(0, 6, &plain), a);
    assert_ne!(GlyphKey::new(0, 5, &GlyphSpec::new(17)), a);
    // -0 and 0 are the same slant.
    let slant = |skew: f32| GlyphSpec {
        style: GlyphStyle { synthesis: GlyphSynthesis { embolden: false, skew }, ..GlyphStyle::PLAIN },
        ..GlyphSpec::new(16)
    };
    assert_eq!(GlyphKey::new(0, 5, &slant(-0.0)), GlyphKey::new(0, 5, &slant(0.0)));
    // The foreground isn't in a key made from a spec (the cache adds it for a glyph that uses it).
    assert_eq!(GlyphKey::new(0, 5, &GlyphSpec { foreground: [1, 2, 3, 4], ..GlyphSpec::new(16) }), a);
}
