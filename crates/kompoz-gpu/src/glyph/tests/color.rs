//! Colour glyphs: COLR v1 paint graphs (testdata/KompozPaints.ttf), COLR v0 layers, sbix and CBDT bitmaps, the
//! composite modes.

use super::face;
use crate::glyph::bitmap::{BitmapImage, bitmap_glyph, multiply_alpha};
use crate::glyph::color::{ColorImage, composite, paint_colr};
use crate::glyph::outline::Hinting;
use kompoz_graphics::{BlendMode, Color, blend};
use skrifa::ScalerFont;
use skrifa::font::glyph::color::CompositeMode;

/// The paint font's glyph for `c` painted at `px` px with `foreground`.
fn paint(c: char, px: u32, foreground: [u8; 4]) -> (ColorImage, bool) {
    let mut face = face("src/glyph/testdata/KompozPaints.ttf");
    let font = face.font().clone();
    let glyph = font.charmap().map_unicode(c).unwrap().to_u32();
    let units = ScalerFont::unhinted(font.clone(), None);
    let mut layers = |layer: u32| face.outline(px, Hinting::Normal, layer);
    let painted = paint_colr(&font, glyph, px, foreground, &mut layers, &units).expect("a colour glyph");
    (painted.image.expect("pixels"), painted.uses_foreground)
}

fn near(got: [u8; 4], want: [f32; 4], tolerance: u8) -> bool {
    got.iter().zip(want).all(|(&g, w)| g.abs_diff((w * 255.0).round() as u8) <= tolerance)
}

#[test]
#[cfg_attr(miri, ignore)]
fn linear_gradients_run_along_their_line() {
    let (image, uses_foreground) = paint('\u{E001}', 40, [0, 0, 0, 255]);
    assert!(!uses_foreground);
    assert_eq!((image.left, image.top, image.width, image.height), (0, 40, 40, 40));
    for col in [0, 9, 20, 39] {
        let t = (col as f32 + 0.5) / 40.0;
        for row in [0, 20, 39] {
            let p = image.at(col, row);
            assert!(near(p, [1.0 - t, 0.0, t, 1.0], 1), "({col}, {row}): {p:?}");
        }
    }
    // Repeated: half the square long.
    let (image, _) = paint('\u{E008}', 40, [0; 4]);
    let t = (30.5f32 / 20.0).fract();
    assert!(near(image.at(30, 10), [1.0 - t, 0.0, t, 1.0], 1), "{:?}", image.at(30, 10));
    // Turned a quarter round the centre: red at the bottom, blue at the top.
    let (image, _) = paint('\u{E00A}', 40, [0; 4]);
    let top = image.at(20, 0);
    let bottom = image.at(20, 39);
    assert!(near(top, [0.0125, 0.0, 0.9875, 1.0], 2), "{top:?}");
    assert!(near(bottom, [0.9875, 0.0, 0.0125, 1.0], 2), "{bottom:?}");
}

#[test]
#[cfg_attr(miri, ignore)]
fn radial_gradients_spread_from_their_centre() {
    let (image, _) = paint('\u{E002}', 40, [0; 4]);
    for (col, row) in [(20, 20), (25, 18), (30, 30), (5, 12)] {
        let (x, y) = (col as f32 + 0.5 - 20.0, 20.0 - (row as f32 + 0.5));
        let t = ((x * x + y * y).sqrt() / 20.0).min(1.0);
        let p = image.at(col, row);
        assert!(near(p, [1.0 - t, 0.0, t, 1.0], 1), "({col}, {row}): {p:?}");
    }
    // Past the outer circle: padded.
    assert!(near(image.at(0, 0), [0.0, 0.0, 1.0, 1.0], 0));
}

#[test]
#[cfg_attr(miri, ignore)]
fn sweep_gradients_turn_counter_clockwise() {
    // Red at 0 degrees to blue at 360, counter-clockwise from the right (y up).
    let (image, _) = paint('\u{E003}', 40, [0; 4]);
    for (col, row) in [(20, 2), (2, 20), (20, 37), (30, 12), (10, 30)] {
        let (x, y) = (col as f32 + 0.5 - 20.0, 20.0 - (row as f32 + 0.5));
        let t = y.atan2(x).to_degrees().rem_euclid(360.0) / 360.0;
        let p = image.at(col, row);
        assert!(near(p, [1.0 - t, 0.0, t, 1.0], 2), "({col}, {row}): {p:?} for {t}");
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn transforms_clips_and_composites_paint_as_colr_says() {
    // The left half moved right.
    let (image, _) = paint('\u{E004}', 40, [0; 4]);
    assert_eq!(image.at(30, 20), [0, 255, 0, 255]);
    assert_eq!(image.at(10, 20)[3], 0);
    // Blue kept only where the red square was (SrcIn).
    let (image, _) = paint('\u{E005}', 40, [0; 4]);
    assert_eq!(image.at(20, 20), [0, 0, 255, 255]);
    assert_eq!(image.at(2, 2)[3], 0);
    // Cyan times yellow: green.
    let (image, _) = paint('\u{E006}', 40, [0; 4]);
    assert_eq!(image.at(20, 20), [0, 255, 0, 255]);
    // A clip box narrower than the glyph: the glyph's pixels are the clip box's.
    let (image, _) = paint('\u{E009}', 40, [0; 4]);
    assert_eq!((image.left, image.top, image.width, image.height), (10, 30, 20, 20));
    assert!(image.rgba.chunks(4).all(|p| p == [0, 255, 0, 255]));
}

#[test]
#[cfg_attr(miri, ignore)]
fn the_foreground_is_the_texts_colour() {
    let (image, uses_foreground) = paint('\u{E007}', 20, [10, 200, 30, 128]);
    assert!(uses_foreground);
    assert!(near(image.at(10, 10), [10.0 / 255.0, 200.0 / 255.0, 30.0 / 255.0, 128.0 / 255.0], 1));
}

#[test]
#[cfg_attr(miri, ignore)]
fn colr_v0_layers_blend_as_freetype_blends_them() {
    // U+E000: a red square, a blue one inside it (KompozCOLRv0.ttf). FreeType's arithmetic gives exact colours.
    let mut face = face("../kompoz-text/tests/fonts/KompozCOLRv0.ttf");
    let font = face.font().clone();
    let glyph = font.charmap().map_unicode('\u{E000}').unwrap().to_u32();
    let units = ScalerFont::unhinted(font.clone(), None);
    let mut layers = |layer: u32| face.outline(20, Hinting::Normal, layer);
    let painted = paint_colr(&font, glyph, 20, [0, 0, 0, 255], &mut layers, &units).unwrap();
    let image = painted.image.unwrap();
    // The outer square 100..900 × 0..800 units: 2..18 × 0..16 px.
    assert_eq!((image.left, image.top, image.width, image.height), (2, 16, 16, 16));
    assert_eq!(image.at(1, 1), [255, 0, 0, 255]);
    assert_eq!(image.at(8, 8), [0, 0, 255, 255]);
    // Not a colour glyph: None (drawn as an outline).
    assert!(paint_colr(&font, 0, 20, [0; 4], &mut layers, &units).is_none());
}

#[test]
#[cfg_attr(miri, ignore)]
fn sbix_bitmaps_are_scaled_from_their_strike() {
    let face = face("../kompoz-text/tests/fonts/KompozSbix.ttf");
    let font = face.font();
    let glyph = font.charmap().map_unicode('\u{E000}').unwrap().to_u32();
    // A 16 px strike of 16 × 16 red, its image at the outline's left bearing (100 units) and bottom (0).
    for (px, left, size) in [(16, 2, 16), (32, 3, 32), (8, 1, 8), (12, 1, 12)] {
        let Some(BitmapImage::Color(image)) = bitmap_glyph(font, glyph, px) else {
            panic!("a colour bitmap at {px}");
        };
        assert_eq!((image.left, image.top, image.width, image.height), (left, size as i32, size, size), "{px}");
        assert!(image.rgba.chunks(4).all(|p| p == [255, 0, 0, 255]), "{px}");
    }
}

#[test]
fn premultiplication_is_freetypes() {
    assert_eq!(multiply_alpha(255, 200), 200);
    assert_eq!(multiply_alpha(128, 255), 128);
    assert_eq!(multiply_alpha(128, 100), 50);
    assert_eq!(multiply_alpha(0, 255), 0);
}

#[test]
fn composite_modes_are_the_w3c_blend_modes() {
    // Against kompoz-graphics' blend (straight colours, 8 bits): every mode, a few colour pairs.
    let modes = [
        (CompositeMode::Clear, BlendMode::Clear),
        (CompositeMode::Src, BlendMode::Src),
        (CompositeMode::Dest, BlendMode::Dst),
        (CompositeMode::SrcOver, BlendMode::SrcOver),
        (CompositeMode::DestOver, BlendMode::DstOver),
        (CompositeMode::SrcIn, BlendMode::SrcIn),
        (CompositeMode::DestIn, BlendMode::DstIn),
        (CompositeMode::SrcOut, BlendMode::SrcOut),
        (CompositeMode::DestOut, BlendMode::DstOut),
        (CompositeMode::SrcAtop, BlendMode::SrcAtop),
        (CompositeMode::DestAtop, BlendMode::DstAtop),
        (CompositeMode::Xor, BlendMode::Xor),
        (CompositeMode::Plus, BlendMode::Plus),
        (CompositeMode::Screen, BlendMode::Screen),
        (CompositeMode::Overlay, BlendMode::Overlay),
        (CompositeMode::Darken, BlendMode::Darken),
        (CompositeMode::Lighten, BlendMode::Lighten),
        (CompositeMode::ColorDodge, BlendMode::ColorDodge),
        (CompositeMode::ColorBurn, BlendMode::ColorBurn),
        (CompositeMode::HardLight, BlendMode::Hardlight),
        (CompositeMode::SoftLight, BlendMode::Softlight),
        (CompositeMode::Difference, BlendMode::Difference),
        (CompositeMode::Exclusion, BlendMode::Exclusion),
        (CompositeMode::Multiply, BlendMode::Multiply),
        (CompositeMode::HslHue, BlendMode::Hue),
        (CompositeMode::HslSaturation, BlendMode::Saturation),
        (CompositeMode::HslColor, BlendMode::Color),
        (CompositeMode::HslLuminosity, BlendMode::Luminosity),
    ];
    // (In 8 bits, as Color holds them: some modes are steep.)
    let colors = [[0.9, 0.3, 0.1, 0.8], [0.2, 0.6, 0.9, 0.5], [1.0, 1.0, 0.0, 1.0], [0.1, 0.1, 0.1, 0.25]]
        .map(|c: [f32; 4]| c.map(|v| (v * 255.0).round() / 255.0));
    let premultiplied = |c: [f32; 4]| [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]];
    for (colr, mode) in modes {
        for s in colors {
            for d in colors {
                let got = composite(premultiplied(s), premultiplied(d), colr);
                let want = blend(Color::new(s[0], s[1], s[2], s[3]), Color::new(d[0], d[1], d[2], d[3]), mode);
                let a = got[3];
                assert!((a - want.alpha()).abs() < 0.01, "{colr:?} {s:?} {d:?}: alpha {a} vs {}", want.alpha());
                if a > 0.05 {
                    let straight = [got[0] / a, got[1] / a, got[2] / a];
                    let w = [want.red(), want.green(), want.blue()];
                    for i in 0..3 {
                        assert!((straight[i] - w[i]).abs() < 0.02, "{colr:?} {s:?} {d:?}: {straight:?} vs {w:?}");
                    }
                }
            }
        }
    }
}
