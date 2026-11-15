//! Outlines (hinting, the points FreeType holds), styles (synthesis, strokes), FreeType 2.14's diacritic adjustment
//! database, the shadow blur.

use super::{face, font_bytes};
use crate::glyph::autohint::adjust::{self, adjustment};
use crate::glyph::blur::{blur_padding, blur_sigma, blurred, box_radii};
use crate::glyph::outline::{GlyphStroke, GlyphStyle, Hinting, glyf_tags, styled_coverage};
use crate::glyph::raster::{Coverage, PointTag};
use kompoz_graphics::{PathEffect, StrokeCap, StrokeJoin};
use kompoz_text::draw::GlyphSynthesis;
use skrifa::{FontRef, HintEngine};

const REGULAR: &str = "tests/fonts/Regular.ttf";

/// The coverage of `c` in Noto Sans Regular at `px` px, light hinted, in `style`.
fn coverage(c: char, px: u32, style: &GlyphStyle) -> Coverage {
    let mut face = face(REGULAR);
    let glyph = face.font().charmap().map_unicode(c).unwrap().to_u32();
    styled_coverage(face.outline(px, Hinting::Light, glyph).unwrap(), px, style, 1.0, 0)
}

/// `c`'s coverage at (x, y) from its origin (px, y up), 0 outside.
fn at(c: &Coverage, x: i32, y: i32) -> u8 {
    c.at(x - c.left, c.top - 1 - y)
}

fn total(c: &Coverage) -> f32 {
    c.alpha.iter().map(|&a| a as f32 / 255.0).sum()
}

#[test]
#[cfg_attr(miri, ignore)]
fn engines_are_freetypes_for_light_and_default_hinting() {
    // Light: TrueType outlines through the automatic hinter, PostScript ones through their own hints; as skrifa's
    // scalers choose (but for ours being the automatic hinter).
    let mut regular = face(REGULAR);
    let mut cff = face("../kompoz-text/tests/fonts/Cantarell-VF-Subset.otf");
    let mut unhinted = face("../kompoz-text/tests/fonts/NotoSans-Subset.ttf");
    assert_eq!(regular.hint_engine(Hinting::Light), Some(HintEngine::Auto));
    assert_eq!(cff.hint_engine(Hinting::Light), Some(HintEngine::PostScript));
    // Default: a font's own instructions, the automatic hinter for a font with none.
    assert_eq!(regular.hint_engine(Hinting::Normal), Some(HintEngine::TrueType));
    assert_eq!(unhinted.hint_engine(Hinting::Normal), Some(HintEngine::Auto));
    assert_eq!(regular.hint_engine(Hinting::None), None);
    for face in [&mut regular, &mut cff, &mut unhinted] {
        for hinting in [Hinting::Light, Hinting::Normal, Hinting::None] {
            assert_eq!(face.hint_engine(hinting), face.scaler(16, hinting).hint_engine(), "{hinting:?}");
        }
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn light_hinting_moves_points_only_vertically() {
    let mut face = face(REGULAR);
    for c in ['o', 'H', 'g', 'e'] {
        let glyph = face.font().charmap().map_unicode(c).unwrap().to_u32();
        let hinted = face.outline(16, Hinting::Light, glyph).unwrap();
        let plain = face.outline(16, Hinting::None, glyph).unwrap();
        assert_eq!(hinted.points.len(), plain.points.len(), "{c}");
        assert_eq!(hinted.tags, plain.tags, "{c}");
        assert!(hinted.points.iter().zip(&plain.points).all(|(a, b)| (a.x - b.x).abs() <= 1), "{c}");
        assert!(hinted.points.iter().zip(&plain.points).any(|(a, b)| a.y != b.y), "{c}");
    }
    // H's stems end on whole pixels: the baseline and the cap height (714 units: 11.4 px unhinted).
    let glyph = face.font().charmap().map_unicode('H').unwrap().to_u32();
    let b = face.outline(16, Hinting::Light, glyph).unwrap().cbox().unwrap();
    assert_eq!((b.y_min % 64, b.y_max % 64), (0, 0));
    let b = face.outline(16, Hinting::None, glyph).unwrap().cbox().unwrap();
    assert_ne!(b.y_max % 64, 0);
}

#[test]
#[cfg_attr(miri, ignore)]
fn outlines_hold_the_fonts_points() {
    // The points FreeType holds: the font's, tagged as the font tags them (no on-curve points added between
    // quadratic controls), contour by contour, a composite's components in turn.
    let bytes = font_bytes(REGULAR);
    let font_ref = FontRef::new(&bytes).unwrap();
    let mut face = face(REGULAR);
    for c in ['o', 'S', 'ğ', 'İ', '@'] {
        let glyph = face.font().charmap().map_unicode(c).unwrap().to_u32();
        let tags = glyf_tags(&font_ref, glyph).unwrap();
        let outline = face.outline(24, Hinting::Light, glyph).unwrap();
        let flat: Vec<PointTag> = tags.iter().flatten().copied().collect();
        assert_eq!(outline.tags, flat, "{c}");
        let ends: Vec<usize> = tags
            .iter()
            .scan(0usize, |n, t| {
                *n += t.len();
                Some(*n - 1)
            })
            .collect();
        assert_eq!(outline.contours, ends, "{c}");
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn fake_bold_adds_to_the_ink() {
    let bold = GlyphStyle { synthesis: GlyphSynthesis { embolden: true, skew: 0.0 }, ..GlyphStyle::PLAIN };
    for c in ['l', 'o', 'e', 'B'] {
        let plain = coverage(c, 24, &GlyphStyle::PLAIN);
        let heavy = coverage(c, 24, &bold);
        // Every pixel at least as covered (the stroke and the fill add, whichever way their contours run)...
        for y in plain.top - plain.height as i32..plain.top {
            for x in plain.left..plain.left + plain.width as i32 {
                assert!(at(&heavy, x, y) as i32 >= at(&plain, x, y) as i32 - 2, "{c} at ({x}, {y})");
            }
        }
        // ...and more ink: Skia's strength at 24 px is 0.84 px, half of it each side of every edge.
        assert!(total(&heavy) > total(&plain) * 1.15, "{c}: {} vs {}", total(&heavy), total(&plain));
    }
    // A counter stays open.
    let o = coverage('o', 40, &bold);
    assert_eq!(at(&o, 11, 10), 0);
}

#[test]
#[cfg_attr(miri, ignore)]
fn fake_italic_leans_right() {
    // Skia's slant: a point y up moves 0.25 y right. The stem of l: its ink near the top lies right of its ink at
    // the bottom by about a quarter of its height.
    let italic = GlyphStyle {
        synthesis: GlyphSynthesis { embolden: false, skew: GlyphSynthesis::ITALIC_SKEW },
        ..GlyphStyle::PLAIN
    };
    let l = coverage('l', 40, &italic);
    let centre = |y: i32| {
        let (sum, weight) = (l.left..l.left + l.width as i32)
            .map(|x| (x as f32 * at(&l, x, y) as f32, at(&l, x, y) as f32))
            .fold((0.0, 0.0), |a, b| (a.0 + b.0, a.1 + b.1));
        sum / weight
    };
    let (low, high) = (2, 26);
    let lean = (centre(high) - centre(low)) / (high - low) as f32;
    assert!((lean - 0.25).abs() < 0.03, "{lean}");
}

#[test]
#[cfg_attr(miri, ignore)]
fn text_skew_leans_as_androids() {
    // TextGeometricTransform(skewX = -0.25) as Android draws it (TextPaint.textSkewX, Skia's y down): leaning
    // right, as much as the fake italic; 0.25 leans left as much.
    let lean = |skew_x: f32| {
        let l = coverage('l', 40, &GlyphStyle { skew_x, ..GlyphStyle::PLAIN });
        let centre = |y: i32| {
            let (sum, weight) = (l.left..l.left + l.width as i32)
                .map(|x| (x as f32 * at(&l, x, y) as f32, at(&l, x, y) as f32))
                .fold((0.0, 0.0), |a, b| (a.0 + b.0, a.1 + b.1));
            sum / weight
        };
        let (low, high) = (2, 26);
        ((centre(high) - centre(low)) / (high - low) as f32, l)
    };
    let (right, slanted) = lean(-0.25);
    assert!((right - 0.25).abs() < 0.03, "{right}");
    // Its top right of the plain glyph's: the box reaches further right.
    let plain = coverage('l', 40, &GlyphStyle::PLAIN);
    assert!(slanted.left as usize + slanted.width > plain.left as usize + plain.width);
    let (left, _) = lean(0.25);
    assert!((left + 0.25).abs() < 0.03, "{left}");
}

#[test]
#[cfg_attr(miri, ignore)]
fn strokes_outline_the_glyph() {
    let stroke = |width: u32, join: StrokeJoin, path_effect: Option<PathEffect>| GlyphStyle {
        stroke: Some(GlyphStroke { width, cap: StrokeCap::Butt, join, miter: 4.0, path_effect }),
        ..GlyphStyle::PLAIN
    };
    let plain = coverage('o', 40, &GlyphStyle::PLAIN);
    let thin = coverage('o', 40, &stroke(2, StrokeJoin::Miter, None));
    let thick = coverage('o', 40, &stroke(4, StrokeJoin::Round, None));
    // Half the width each side: the box grows by a pixel (two) each way; the counter and the ring's middle empty.
    assert_eq!((thin.left, thin.top), (plain.left - 1, plain.top + 1));
    assert_eq!((thick.left, thick.top), (plain.left - 2, plain.top + 2));
    assert_eq!(at(&thin, 11, 10), 0);
    assert!(total(&thick) > total(&thin) * 1.6);
    // A dash: less ink than the whole stroke.
    let dashed = coverage('o', 40, &stroke(2, StrokeJoin::Miter, Some(PathEffect::dash_path_effect(&[4.0, 4.0], 0.0))));
    let ratio = total(&dashed) / total(&thin);
    assert!(ratio > 0.35 && ratio < 0.65, "{ratio}");
}

#[test]
fn blurs_are_cpps() {
    // Skia's sigma for a radius, three sigma of padding, and blurCoverage's three box widths.
    assert!((blur_sigma(4.0) - 2.80940).abs() < 1e-5);
    assert_eq!(blur_padding(blur_sigma(4.0)), 9);
    assert_eq!(box_radii(blur_sigma(4.0)), [2, 2, 3]);
    assert_eq!(box_radii(blur_sigma(1.0)), [0, 1, 1]);
    assert_eq!(box_radii(blur_sigma(10.0)), [5, 6, 6]);
    // The three boxes' variance is the Gaussian's, near enough.
    for r in [1.0, 2.0, 4.0, 7.0, 12.0] {
        let sigma: f32 = blur_sigma(r);
        let variance: f32 = box_radii(sigma).iter().map(|&k| ((2 * k + 1).pow(2) - 1) as f32 / 12.0).sum();
        assert!((variance.sqrt() - sigma).abs() < 0.5, "{r}: {variance} vs {}", sigma * sigma);
    }
    // A pixel blurred: spread evenly each way, its ink kept.
    let dot = Coverage { left: 5, top: 5, width: 1, height: 1, alpha: vec![255] };
    let b = blurred(&dot, 2);
    let pad = blur_padding(blur_sigma(2.0));
    assert_eq!((b.left, b.top, b.width, b.height), (5 - pad as i32, 5 + pad as i32, 2 * pad + 1, 2 * pad + 1));
    let n = b.width;
    for y in 0..n {
        for x in 0..n {
            assert_eq!(b.alpha[y * n + x], b.alpha[y * n + (n - 1 - x)]);
            assert_eq!(b.alpha[y * n + x], b.alpha[x * n + y]);
        }
    }
    let ink: f32 = b.alpha.iter().map(|&a| a as f32).sum();
    assert!((ink - 255.0).abs() < 10.0, "{ink}");
    // No radius, as it is.
    assert_eq!(blurred(&dot, 0), dot);
}

#[test]
fn the_adjustment_database_is_freetypes() {
    assert_eq!(adjustment('i' as u32), adjust::UP);
    assert_eq!(adjustment('ğ' as u32), adjust::UP);
    assert_eq!(adjustment('ç' as u32), adjust::IGNORE_SMALL_BOTTOM);
    assert_eq!(adjustment('ñ' as u32), adjust::UP | adjust::TILDE_TOP);
    assert_eq!(adjustment('a' as u32), 0);
    assert_eq!(adjustment(0x10), 0);
    // `Q` sits before `?` in FreeType's table, out of order: its binary search misses `Q`, as here.
    assert_eq!(adjustment('Q' as u32), 0);
    assert_eq!(adjustment('?' as u32), adjust::UP | adjust::NO_HEIGHT_CHECK);
    assert_eq!(adjustment('!' as u32), adjust::UP | adjust::NO_HEIGHT_CHECK);
}

#[test]
#[cfg_attr(miri, ignore)]
fn accents_are_kept_clear_as_freetype_2_14_keeps_them() {
    // ü at 11 px: the dieresis moved up from the hinted x-height (skrifa's hinter, FreeType 2.13's, leaves it a
    // pixel lower); its points are the glyph's own, as FreeType holds them.
    let mut face = face(REGULAR);
    let glyph = face.font().charmap().map_unicode('ü').unwrap().to_u32();
    let ours = face.outline(11, Hinting::Light, glyph).unwrap();
    let scaler = face.scaler(11, Hinting::Light);
    let bytes = font_bytes(REGULAR);
    let tags = glyf_tags(&FontRef::new(&bytes).unwrap(), glyph);
    let skrifas = crate::glyph::outline::load(scaler, glyph, tags.clone()).unwrap();
    assert_eq!(ours.points.len(), skrifas.points.len());
    assert_eq!(Some(ours.tags.clone()), tags.map(|t| t.concat()));
    assert!(ours.cbox().unwrap().y_max > skrifas.cbox().unwrap().y_max);
}
