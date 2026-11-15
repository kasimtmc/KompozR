//! The bottom of the pipeline and what it ends in: the fake backend's numbers (the C++ Kompoz's
//! FakeTextMeasurer's, `testing.hpp`), and the draw contract (a run of text, its glyph runs, its equality).

use kompoz_text::shape::{FontRunRequest, ShapeRequest};
use kompoz_text::*;
use std::rc::Rc;

use kompoz_graphics::{Density, Offset, Rect, Size};

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn shape(text: &str, size: f32, rtl: bool, letter_spacing: f32, scale_x: f32) -> shape::ShapedRun {
    let font = FakeBackend::instance(size);
    FakeBackend.shape(&ShapeRequest {
        text,
        range: 0..text.len(),
        font: &font,
        rtl,
        script: None,
        language: None,
        features: &[],
        letter_spacing,
        scale_x,
    })
}

#[test]
fn the_fake_measures_as_cpps_fake_text_measurer() {
    // Every code point half an em, stretched, plus the letter spacing.
    assert!(near(FakeBackend::measure_width("abç", 16.0, 1.0, 0.0), 24.0));
    assert!(near(FakeBackend::measure_width("abç", 16.0, 2.0, 0.0), 48.0));
    assert!(near(FakeBackend::measure_width("abç", 16.0, 1.0, 1.0), 27.0));
    let m = FakeBackend.metrics(&FakeBackend::instance(16.0));
    assert!(near(m.ascent, 12.8) && near(m.descent, 3.2) && near(m.leading, 4.0) && near(m.line_height(), 20.0));
    assert!(near(m.underline_position, 2.0) && near(m.underline_thickness, 16.0 / 14.0));
    assert!(
        near(m.strikeout_position, -3.84)
            && near(FakeBackend.metrics(&FakeBackend::instance(10.0)).strikeout_thickness, 1.0)
    );
    assert_eq!(FakeBackend.glyph_bounds(&FakeBackend::instance(10.0), 'a' as u32), Rect::new(0.0, -8.0, 5.0, 2.0));
    assert_eq!(FakeBackend.color_kind(FakeBackend::font_id()), GlyphColorKind::Outline);
}

#[test]
fn the_fake_shapes_a_glyph_a_code_point() {
    let run = shape("aç", 16.0, false, 2.0, 1.0);
    let ids: Vec<u32> = run.glyphs.iter().map(|g| g.id).collect();
    assert_eq!(ids, vec!['a' as u32, 'ç' as u32]);
    assert_eq!(run.glyphs.iter().map(|g| g.cluster).collect::<Vec<_>>(), vec![0, 1]);
    // The letter spacing half before, half after.
    assert!(run.glyphs.iter().all(|g| near(g.x_advance, 10.0) && near(g.x_offset, 1.0) && !g.unsafe_to_break));
    assert!(near(run.advance, 20.0));
    // Right to left: in visual order, the last cluster first.
    let rtl = shape("aç", 16.0, true, 0.0, 1.0);
    assert_eq!(rtl.glyphs.iter().map(|g| g.cluster).collect::<Vec<_>>(), vec![1, 0]);
    // Stretched.
    assert!(near(shape("ab", 10.0, false, 0.0, 1.5).advance, 15.0));
    // A run within a text: clusters are the text's offsets.
    let font = FakeBackend::instance(10.0);
    let part = FakeBackend.shape(&ShapeRequest {
        text: "xyz",
        range: 1..3,
        font: &font,
        rtl: false,
        script: Some(*b"Latn"),
        language: None,
        features: &[],
        letter_spacing: 0.0,
        scale_x: 1.0,
    });
    assert_eq!(part.glyphs.iter().map(|g| g.cluster).collect::<Vec<_>>(), vec![1, 2]);
}

#[test]
fn the_fake_shapes_everything_in_its_one_face() {
    let typeface = FakeFontFamilyResolver::typeface(
        Some(&FontFamily::new([Font::file("r.ttf")])),
        FontWeight::Bold,
        FontStyle::Normal,
        FontSynthesis::All,
    );
    let locales = LocaleList::from_tags("tr");
    let runs = FakeBackend.font_runs(&FontRunRequest {
        text: "merhaba 👋 שלום",
        range: 2..10,
        typeface: &typeface,
        size: 12.0,
        density: Density::default(),
        locales: &locales,
    });
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].range, 2..10);
    assert!(
        runs[0].font.font == FakeBackend::font_id() && near(runs[0].font.size, 12.0) && runs[0].font.synthesis.embolden
    );
}

#[test]
fn a_run_of_text_is_drawn_from_its_glyphs() {
    let style = Rc::new(TextRunStyle { letter_spacing: 2.0, ..TextRunStyle::new(10.0) });
    let t = FakeBackend.text_draw("abc", Offset::new(5.0, 8.0), style.clone(), false);
    // Advance and line height as C++'s, the glyphs half the letter spacing in, their ink the boxes.
    assert_eq!(t.size, Size::new(21.0, 12.5));
    let xs: Vec<f32> = t.glyphs[0].glyphs.iter().map(|g| g.x).collect();
    assert_eq!(xs, vec![1.0, 8.0, 15.0]);
    assert_eq!(t.ink_bounds, Rect::new(1.0, -8.0, 20.0, 2.0));
    assert_eq!(t.ink_rect(), Rect::new(6.0, 0.0, 25.0, 10.0));
    assert!(t.glyphs[0].font == FakeBackend::font_id() && near(t.glyphs[0].size, 10.0));
    assert_eq!(t.glyphs[0].text_motion, TextMotion::Static);
    assert_eq!(t.font_size(), 10.0);
    // A line height asked for is the run's height.
    let tall = FakeBackend.text_draw(
        "a",
        Offset::Zero,
        Rc::new(TextRunStyle { line_height: Some(30.0), ..TextRunStyle::new(10.0) }),
        false,
    );
    assert_eq!(tall.size.height, 30.0);
    // Equal while it shows the same: the same glyph runs, or runs of the same content.
    let again = FakeBackend.text_draw("abc", Offset::new(5.0, 8.0), style.clone(), false);
    assert!(!Rc::ptr_eq(&t.glyphs, &again.glyphs) && t.content_key == again.content_key && t == again);
    assert_ne!(t, t.at(Offset::new(6.0, 8.0)));
    let rtl = FakeBackend.text_draw("abc", Offset::new(5.0, 8.0), style.clone(), true);
    assert!(rtl != t && rtl.content_key != t.content_key);
    let mut runs = t.glyphs.to_vec();
    runs[0].glyphs[0].id = 'x' as u32;
    let other = TextDraw::new(t.text.clone(), t.baseline, t.size, t.ink_bounds, false, style.clone(), runs.into());
    assert_ne!(t, other);
    let restyled =
        TextDraw { style: Rc::new(TextRunStyle { font_style: FontStyle::Italic, ..(*style).clone() }), ..t.clone() };
    assert_ne!(t, restyled);
    assert_eq!(TextDraw::content_key_of(&t.glyphs), t.content_key);
    assert_ne!(TextDraw::content_key_of(&[]), t.content_key);
}
