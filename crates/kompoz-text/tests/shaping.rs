//! The real backend over the test fonts: plain text shaped as the fake measures it (Kompoz Test is the fake as a
//! font), Arabic joining and right-to-left clusters, Devanagari reordering and conjuncts, emoji sequences as one
//! glyph in the emoji font, VS15 and VS16, the fallback chain, letter spacing, stretch, features, metrics from the
//! fonts, glyph bounds and color kinds, and the caches.

mod common;

use common::*;
use kompoz_graphics::Rect;
use kompoz_text::font::{FontCollection, FontFamilyResolverImpl};
use kompoz_text::shape::{FontInstance, FontRun, ShapeRequest, ShapedRun};
use kompoz_text::*;
use std::rc::Rc;

/// The fake's run of the same text, for comparing.
fn fake(text: &str, size: f32, rtl: bool, letter_spacing: f32, scale_x: f32) -> ShapedRun {
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
        linear: false,
    })
}

/// Everything but the glyph ids (the fake's are code points).
fn same_but_ids(a: &ShapedRun, b: &ShapedRun) -> bool {
    a.glyphs.len() == b.glyphs.len()
        && near(a.advance, b.advance)
        && a.glyphs.iter().zip(&b.glyphs).all(|(x, y)| {
            x.cluster == y.cluster
                && near(x.x_advance, y.x_advance)
                && near(x.x_offset, y.x_offset)
                && near(x.y_offset, y.y_offset)
                && x.unsafe_to_break == y.unsafe_to_break
        })
}

#[test]
fn kompoz_test_shapes_plain_text_as_the_fake_measures_it() {
    let r = resolver_with_default("Kompoz Test");
    let b = r.backend();
    let t = default_typeface(&r);
    for (text, rtl) in [("Merhaba dünya, şöyle!", false), ("שלום עולם", true), ("ب ت", true), ("Ğ", false)]
    {
        let runs = runs(&*b, text, &t, 16.0);
        assert_eq!(runs.len(), 1, "{text}: one run in Kompoz Test");
        assert_eq!(family_of(&r, runs[0].font.font), "Kompoz Test");
        for (spacing, stretch) in [(0.0, 1.0), (0.0, 1.5)] {
            let real = shape_with(&*b, text, &runs[0], rtl, spacing, stretch, &[]);
            assert!(same_but_ids(&real, &fake(text, 16.0, rtl, spacing, stretch)), "{text} at {spacing}, {stretch}");
        }
    }
    // Letter spacing too, but for Arabic (not spaced, as Android leaves it).
    let text = "Kompoz";
    let run = &runs(&*b, text, &t, 10.0)[0];
    assert!(same_but_ids(&shape_with(&*b, text, run, false, 2.0, 1.0, &[]), &fake(text, 10.0, false, 2.0, 1.0)));
    let arabic = "بت";
    let run = &runs(&*b, arabic, &t, 10.0)[0];
    assert!(near(shape_with(&*b, arabic, run, true, 2.0, 1.0, &[]).advance, 10.0));
    // Its metrics and glyph boxes are the fake's.
    let font = &runs(&*b, "a", &t, 16.0)[0].font;
    let (m, f) = (b.metrics(font), FakeBackend.metrics(&FakeBackend::instance(16.0)));
    assert!(near(m.ascent, f.ascent) && near(m.descent, f.descent) && near(m.leading, f.leading));
    assert!(near(m.line_height(), 20.0) && near(m.underline_position, f.underline_position));
    let glyph = shape_run(&*b, "a", &runs(&*b, "a", &t, 16.0)[0], false).glyphs[0].id;
    let (real, box_) =
        (b.glyph_bounds(font, glyph), FakeBackend.glyph_bounds(&FakeBackend::instance(16.0), 'a' as u32));
    assert!(near(real.left, box_.left) && near(real.top, box_.top) && near(real.right, box_.right));
    assert!(near(real.bottom, box_.bottom));
    assert_eq!(b.color_kind(font.font), GlyphColorKind::Outline);
}

#[test]
fn arabic_joins_its_letters_and_goes_right_to_left() {
    let r = resolver();
    let b = r.backend();
    let t = default_typeface(&r);
    // Beh joined to beh, a space, beh alone.
    let text = "بب ب";
    let runs = runs(&*b, text, &t, 16.0);
    assert_eq!(runs.len(), 1);
    assert_eq!(family_of(&r, runs[0].font.font), "Noto Sans Arabic");
    let shaped = shape_run(&*b, text, &runs[0], true);
    // In visual order: the last cluster first.
    let clusters: Vec<usize> = shaped.glyphs.iter().map(|g| g.cluster).collect();
    assert!(clusters.windows(2).all(|w| w[0] >= w[1]) && clusters[0] == 5 && *clusters.last().unwrap() == 0);
    let glyphs_of =
        |cluster: usize| -> Vec<u32> { shaped.glyphs.iter().filter(|g| g.cluster == cluster).map(|g| g.id).collect() };
    // Initial, final and isolated forms differ.
    let (initial, last, alone) = (glyphs_of(0), glyphs_of(2), glyphs_of(5));
    assert!(initial != last && last != alone && initial != alone, "{initial:?} {last:?} {alone:?}");
    // Joined letters can't be cut apart without shaping again (HarfBuzz's unsafe-to-break).
    assert!(shaped.glyphs.iter().any(|g| g.cluster == 2 && g.unsafe_to_break));
    // A beh alone is isolated; the first beh shaped alone but with the text after it as context still joins.
    let lone = "ب";
    assert_eq!(glyph_ids(&shape_run(&*b, lone, &runs_of_range(&*b, lone, 0..2, &t), true)), alone);
    let first = runs_of_range(&*b, text, 0..2, &t);
    assert_eq!(glyph_ids(&shape_run(&*b, text, &first, true)), initial);
}

/// A run of `range` in its one face.
fn runs_of_range(b: &dyn FontBackend, text: &str, range: std::ops::Range<usize>, t: &Typeface) -> FontRun {
    let locales = LocaleList::empty();
    let mut runs = b.font_runs(&kompoz_text::shape::FontRunRequest {
        text,
        range,
        typeface: t,
        size: 16.0,
        density: kompoz_graphics::Density::default(),
        locales: &locales,
    });
    assert_eq!(runs.len(), 1);
    runs.remove(0)
}

fn glyph_ids(run: &ShapedRun) -> Vec<u32> {
    run.glyphs.iter().map(|g| g.id).collect()
}

#[test]
fn devanagari_reorders_its_vowel_signs_and_joins_conjuncts() {
    let r = resolver();
    let b = r.backend();
    let t = default_typeface(&r);
    // Ka with the i sign: the sign is drawn first, both one cluster.
    let ki = "कि";
    let run = &runs(&*b, ki, &t, 16.0)[0];
    assert_eq!(family_of(&r, run.font.font), "Noto Sans Devanagari");
    let shaped = shape_run(&*b, ki, run, false);
    let ka = shape_run(&*b, "क", &runs(&*b, "क", &t, 16.0)[0], false).glyphs[0].id;
    assert_eq!(shaped.glyphs.len(), 2);
    assert!(shaped.glyphs[1].id == ka && shaped.glyphs[0].id != ka);
    assert!(shaped.glyphs.iter().all(|g| g.cluster == 0));
    // Ka, virama, ssa: one conjunct glyph; with the i sign, the sign before it.
    let kssa = "क्ष";
    let conjunct = shape_run(&*b, kssa, &runs(&*b, kssa, &t, 16.0)[0], false);
    assert_eq!(conjunct.glyphs.len(), 1);
    let kssi = "क्षि";
    let shaped = shape_run(&*b, kssi, &runs(&*b, kssi, &t, 16.0)[0], false);
    assert_eq!(glyph_ids(&shaped)[1..], glyph_ids(&conjunct)[..]);
    assert_eq!(shaped.glyphs.len(), 2);
    // One cluster, one grapheme.
    assert_eq!(segment::graphemes(kssi).count(), 1);
}

#[test]
fn emoji_sequences_are_one_glyph_of_the_emoji_font() {
    let r = resolver();
    let b = r.backend();
    let t = default_typeface(&r);
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    let england = "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}";
    for (sequence, glyphs) in [
        (family, 1),
        (england, 1),
        ("\u{1F44D}\u{1F3FD}", 1),
        ("1\u{FE0F}\u{20E3}", 1),
        ("\u{1F1F9}\u{1F1F7}\u{1F1E9}\u{1F1EA}", 2),
        ("\u{2764}\u{FE0F}", 1),
    ] {
        let text = format!("a{sequence}b");
        let runs = runs(&*b, &text, &t, 16.0);
        let families: Vec<String> = runs.iter().map(|r2| family_of(&r, r2.font.font)).collect();
        assert_eq!(families, vec!["Noto Sans", "Noto Color Emoji", "Noto Sans"], "{sequence:?}");
        assert_eq!(runs[1].range, 1..1 + sequence.len());
        let shaped = shape_run(&*b, &text, &runs[1], false);
        assert_eq!(shaped.glyphs.len(), glyphs, "{sequence:?}");
        assert!(shaped.glyphs.iter().all(|g| g.id != 0), "{sequence:?}: no missing glyph");
        assert_eq!(b.color_kind(runs[1].font.font), GlyphColorKind::Bitmap);
    }
    assert_eq!(segment::graphemes(family).count(), 1);
    // A flag is its pair of regional indicators: two flags are two glyphs, two clusters.
    let flags = "\u{1F1F9}\u{1F1F7}\u{1F1E9}\u{1F1EA}";
    let shaped = shape_run(&*b, flags, &runs(&*b, flags, &t, 16.0)[0], false);
    assert_eq!(shaped.glyphs.iter().map(|g| g.cluster).collect::<Vec<_>>(), vec![0, 8]);
    // A digit alone is text, in the text's font.
    let digit = "1";
    assert_eq!(family_of(&r, runs(&*b, digit, &t, 16.0)[0].font.font), "Noto Sans");
}

#[test]
fn variation_selectors_pick_text_or_emoji_fonts() {
    let r = resolver();
    let b = r.backend();
    let t = default_typeface(&r);
    let families = |text: &str| -> Vec<(String, String)> {
        runs(&*b, text, &t, 16.0)
            .iter()
            .map(|run| (text[run.range.clone()].to_string(), family_of(&r, run.font.font)))
            .collect()
    };
    // ❤ is text by default: an outline font has it (Kompoz Test), not the emoji font.
    assert_eq!(families("\u{2764}"), vec![("\u{2764}".into(), "Kompoz Test".into())]);
    // VS16 asks for emoji, VS15 for text.
    assert_eq!(families("\u{2764}\u{FE0F}"), vec![("\u{2764}\u{FE0F}".into(), "Noto Color Emoji".into())]);
    assert_eq!(families("\u{2764}\u{FE0E}"), vec![("\u{2764}\u{FE0E}".into(), "Kompoz Test".into())]);
    // 👍 is emoji by default; VS15 asks for text, which no outline font has: the emoji font still.
    assert_eq!(families("\u{1F44D}"), vec![("\u{1F44D}".into(), "Noto Color Emoji".into())]);
    assert_eq!(families("\u{1F44D}\u{FE0E}"), vec![("\u{1F44D}\u{FE0E}".into(), "Noto Color Emoji".into())]);
    // Where the text's own font has ❤ as text, VS16 still takes the emoji font.
    let kompoz = resolver_with_default("Kompoz Test");
    let kt = default_typeface(&kompoz);
    let kb = kompoz.backend();
    let runs = runs(&*kb, "\u{2764}\u{2764}\u{FE0F}", &kt, 16.0);
    assert_eq!(runs.len(), 2);
    assert_eq!(family_of(&kompoz, runs[0].font.font), "Kompoz Test");
    assert_eq!(family_of(&kompoz, runs[1].font.font), "Noto Color Emoji");
}

#[test]
fn latin_arabic_and_emoji_in_one_string_take_three_fonts() {
    let r = resolver();
    let b = r.backend();
    let t = default_typeface(&r);
    let text = "Merhaba مرحبا 👍!";
    let runs = runs(&*b, text, &t, 16.0);
    let parts: Vec<(&str, String)> =
        runs.iter().map(|run| (&text[run.range.clone()], family_of(&r, run.font.font))).collect();
    // The space after the Arabic word keeps to the Arabic font; the "!" after the emoji goes back to the text's.
    assert_eq!(
        parts,
        vec![
            ("Merhaba ", "Noto Sans".to_string()),
            ("مرحبا ", "Noto Sans Arabic".to_string()),
            ("👍", "Noto Color Emoji".to_string()),
            ("!", "Noto Sans".to_string()),
        ]
    );
    // Hebrew and Devanagari through their scripts' fallbacks; runs cover the text, cut at clusters.
    let mixed = "a שלום न";
    let runs2 = common::runs(&*b, mixed, &t, 16.0);
    let names: Vec<String> = runs2.iter().map(|run| family_of(&r, run.font.font)).collect();
    assert_eq!(names, vec!["Noto Sans", "Noto Sans Hebrew", "Noto Sans Devanagari"]);
    assert_eq!(runs2.first().unwrap().range.start, 0);
    assert_eq!(runs2.last().unwrap().range.end, mixed.len());
    assert!(runs2.windows(2).all(|w| w[0].range.end == w[1].range.start));
    // A part of a text: runs only over it.
    let locales = LocaleList::empty();
    let part = b.font_runs(&kompoz_text::shape::FontRunRequest {
        text,
        range: 3..12,
        typeface: &t,
        size: 16.0,
        density: kompoz_graphics::Density::default(),
        locales: &locales,
    });
    assert_eq!(part.iter().map(|p| p.range.clone()).collect::<Vec<_>>(), vec![3..8, 8..12]);
    // Characters no font has stay in the text's font (its missing glyph).
    let none = common::runs(&*b, "a\u{0E01}", &t, 16.0);
    assert_eq!(none.len(), 1);
    assert_eq!(shape_run(&*b, "a\u{0E01}", &none[0], false).glyphs[1].id, 0);
}

#[test]
fn fallback_follows_the_texts_locales_and_keeps_marks_and_punctuation_with_their_letters() {
    let r = resolver();
    let b = r.backend();
    let t = default_typeface(&r);
    // Persian's Arabic script in a font of its own (Kompoz Test standing in): what the text's locale asks for wins.
    r.collection().set_fallback_families(*b"Arab", Some(&Locale::new("fa")), &["Kompoz Test"]);
    let text = "a بب";
    let in_locales = |tags: &str| {
        let locales = LocaleList::from_tags(tags);
        let runs = b.font_runs(&kompoz_text::shape::FontRunRequest {
            text,
            range: 0..text.len(),
            typeface: &t,
            size: 16.0,
            density: kompoz_graphics::Density::default(),
            locales: &locales,
        });
        family_of(&r, runs.last().unwrap().font.font)
    };
    assert_eq!(in_locales("fa"), "Kompoz Test");
    assert_eq!(in_locales("en,fa"), "Kompoz Test");
    assert_eq!(in_locales("ar"), "Noto Sans Arabic");
    assert_eq!(common::runs(&*b, text, &t, 16.0).len(), 2);
    // A mark stays with its letter (one cluster), the Arabic comma and the space after it with the Arabic font.
    let marked = "\u{0645}\u{064E}\u{0631}\u{060C} ";
    let runs = common::runs(&*b, marked, &t, 16.0);
    assert_eq!(runs.len(), 1);
    assert_eq!(family_of(&r, runs[0].font.font), "Noto Sans Arabic");
    let shaped = shape_run(&*b, marked, &runs[0], true);
    assert!(shaped.glyphs.iter().filter(|g| g.cluster == 0).count() >= 2, "the fatha is in meem's cluster");
    assert!(!shaped.glyphs.iter().any(|g| g.cluster == 2));
    // Digits after Arabic go back to the text's font when the Arabic one has none.
    let digits = "\u{0645}\u{0631} 12";
    let runs = common::runs(&*b, digits, &t, 16.0);
    let parts: Vec<(&str, String)> =
        runs.iter().map(|run| (&digits[run.range.clone()], family_of(&r, run.font.font))).collect();
    assert_eq!(parts, vec![("\u{0645}\u{0631} ", "Noto Sans Arabic".to_string()), ("12", "Noto Sans".to_string())]);
}

#[test]
fn bold_and_italic_fallbacks_are_faked_as_minikin_fakes() {
    let r = resolver();
    let b = r.backend();
    let bold = r.typeface(None, FontWeight::Bold, FontStyle::Italic, FontSynthesis::All);
    // The default family has no bold or italic face: faked.
    assert!(bold.synthesis.embolden && bold.synthesis.skew == GlyphSynthesis::ITALIC_SKEW);
    let text = "a مر 👍";
    let runs = runs(&*b, text, &bold, 16.0);
    assert_eq!(runs.len(), 3);
    // The Arabic fallback too (minikin fakes fallback fonts whatever the synthesis allowed); the color emoji
    // isn't emboldened, slanted only.
    assert!(runs[1].font.synthesis.embolden && runs[1].font.synthesis.skew == GlyphSynthesis::ITALIC_SKEW);
    assert!(!runs[2].font.synthesis.embolden && runs[2].font.synthesis.skew == GlyphSynthesis::ITALIC_SKEW);
    // Nothing faked when synthesis is off, but fallbacks still are.
    let none = r.typeface(None, FontWeight::Bold, FontStyle::Normal, FontSynthesis::None);
    assert_eq!(none.synthesis, GlyphSynthesis::None);
    let runs = common::runs(&*b, "aم", &none, 16.0);
    assert!(runs[0].font.synthesis == GlyphSynthesis::None && runs[1].font.synthesis.embolden);
    // 600 asked of a 400 face is faked; 500 isn't.
    assert!(r.typeface(None, FontWeight::SemiBold, FontStyle::Normal, FontSynthesis::All).synthesis.embolden);
    assert!(!r.typeface(None, FontWeight::Medium, FontStyle::Normal, FontSynthesis::All).synthesis.embolden);
    // A fake bold grows the ink, a fake italic slants it to the right above the baseline.
    let plain = default_typeface(&r);
    let plain_run = &common::runs(&*b, "H", &plain, 20.0)[0];
    let id = shape_run(&*b, "H", plain_run, false).glyphs[0].id;
    let ink = b.glyph_bounds(&plain_run.font, id);
    let emboldened = FontInstance { synthesis: GlyphSynthesis { embolden: true, skew: 0.0 }, ..plain_run.font.clone() };
    let grown = b.glyph_bounds(&emboldened, id);
    let half = shape::fake_bold_strength(20.0) / 2.0;
    assert!(
        near(grown.left, ink.left - half) && near(grown.top, ink.top - half) && near(grown.right, ink.right + half)
    );
    let slanted = FontInstance { synthesis: GlyphSynthesis { embolden: false, skew: -0.25 }, ..plain_run.font.clone() };
    let s = b.glyph_bounds(&slanted, id);
    assert!(near(s.right, ink.right - ink.top * 0.25) && near(s.left, ink.left) && s.top == ink.top);
    assert!(near(shape::fake_bold_strength(9.0), 9.0 / 24.0) && near(shape::fake_bold_strength(72.0), 72.0 / 32.0));
}

#[test]
fn metrics_come_from_the_fonts() {
    let r = resolver();
    let b = r.backend();
    let c = r.collection();
    let instance = |family: &str, size: f32| {
        let face = c.face_by_name(family, FontWeight::Normal, FontStyle::Normal).expect(family);
        FontInstance { font: face.font, coords: Rc::from([]), size, synthesis: GlyphSynthesis::None }
    };
    // Kompoz Test: 800 / -200 / 250 of 1000 (USE_TYPO_METRICS), its underline 100 below to the stroke's top and
    // 50 thick, its strikeout 300 above and 50 thick, x-height 500, cap height 700.
    let m = b.metrics(&instance("Kompoz Test", 10.0));
    assert!(near(m.ascent, 8.0) && near(m.descent, 2.0) && near(m.leading, 2.5) && near(m.line_height(), 12.5));
    assert!(near(m.underline_position, 1.25) && near(m.underline_thickness, 0.5));
    assert!(near(m.strikeout_position, -2.75) && near(m.strikeout_thickness, 0.5));
    assert!(near(m.x_height, 5.0) && near(m.cap_height, 7.0));
    // Noto Sans: 1069 / -293, no gap.
    let m = b.metrics(&instance("Noto Sans", 10.0));
    assert!(near(m.ascent, 10.69) && near(m.descent, 2.93) && near(m.leading, 0.0));
    // Noto Sans Arabic reaches further: what makes a line with it taller.
    let arabic = b.metrics(&instance("Noto Sans Arabic", 10.0));
    assert!(arabic.ascent > m.ascent && arabic.descent > m.descent);
    // Cantarell doesn't ask for its typographic line: hhea's (983 / -217 / 0), not OS/2's (739 / -217 / 244).
    let cantarell = FontFamilyResolverImpl::registered_only();
    cantarell.collection().register_file(font_path(CANTARELL)).unwrap();
    let face = cantarell.collection().face_by_name("Cantarell", FontWeight::Normal, FontStyle::Normal).unwrap();
    let m = cantarell.harfrust_backend().metrics(&FontInstance {
        font: face.font,
        coords: Rc::from([]),
        size: 10.0,
        synthesis: GlyphSynthesis::None,
    });
    assert!(near(m.ascent, 9.83) && near(m.descent, 2.17) && near(m.leading, 0.0));
}

#[test]
fn color_kinds_and_glyph_bounds_come_from_the_fonts() {
    let r = resolver();
    let b = r.backend();
    let c = r.collection();
    let id = |family: &str| c.face_by_name(family, FontWeight::Normal, FontStyle::Normal).expect(family).font;
    assert_eq!(b.color_kind(id("Kompoz Test")), GlyphColorKind::Outline);
    assert_eq!(b.color_kind(id("Kompoz COLRv0")), GlyphColorKind::ColrV0);
    assert_eq!(b.color_kind(id("Kompoz COLRv1")), GlyphColorKind::ColrV1);
    assert_eq!(b.color_kind(id("Kompoz Sbix")), GlyphColorKind::Bitmap);
    assert_eq!(b.color_kind(id("Noto Color Emoji")), GlyphColorKind::Bitmap);
    // A COLR v1 glyph's ink is its clip box (100..900 by 0..800 of 1000).
    let colr =
        FontInstance { font: id("Kompoz COLRv1"), coords: Rc::from([]), size: 10.0, synthesis: GlyphSynthesis::None };
    let run = FontRun { range: 0..3, font: colr.clone() };
    let glyph = shape_run(&*b, "\u{E000}", &run, false).glyphs[0].id;
    assert_eq!(b.glyph_bounds(&colr, glyph), Rect::new(1.0, -8.0, 9.0, 0.0));
    // A COLR v0 glyph's: its layers' (the outlines).
    let v0 = FontInstance { font: id("Kompoz COLRv0"), ..colr.clone() };
    assert_eq!(b.glyph_bounds(&v0, glyph), Rect::new(1.0, -8.0, 9.0, 0.0));
    // A bitmap's: its 16 px image at the size asked.
    let sbix = FontInstance { font: id("Kompoz Sbix"), size: 32.0, ..colr.clone() };
    let bounds = b.glyph_bounds(&sbix, glyph);
    assert!(near(bounds.right - bounds.left, 32.0) && near(bounds.bottom - bounds.top, 32.0), "{bounds:?}");
    // An emoji bitmap: about an em.
    let emoji = FontInstance { font: id("Noto Color Emoji"), size: 20.0, ..colr };
    let thumbs = shape_run(&*b, "\u{1F44D}", &FontRun { range: 0..4, font: emoji.clone() }, false).glyphs[0].id;
    let e = b.glyph_bounds(&emoji, thumbs);
    assert!(e.right - e.left > 15.0 && e.right - e.left < 30.0 && e.top < 0.0, "{e:?}");
    // A space has no ink.
    let kt =
        FontInstance { font: id("Kompoz Test"), coords: Rc::from([]), size: 10.0, synthesis: GlyphSynthesis::None };
    let space = shape_run(&*b, " ", &FontRun { range: 0..1, font: kt.clone() }, false).glyphs[0].id;
    // (Kompoz Test's space is a box too, as the fake's.)
    assert_eq!(b.glyph_bounds(&kt, space), Rect::new(0.0, -8.0, 5.0, 2.0));
    let noto = FontInstance { font: id("Noto Sans"), ..kt };
    let space = shape_run(&*b, " ", &FontRun { range: 0..1, font: noto.clone() }, false).glyphs[0].id;
    assert_eq!(b.glyph_bounds(&noto, space), Rect::Zero);
}

#[test]
fn features_scale_and_letter_spacing_shape_as_asked() {
    let r = resolver();
    let b = r.backend();
    let t = default_typeface(&r);
    let text = "fi Ta";
    let run = &runs(&*b, text, &t, 20.0)[0];
    let plain = shape_run(&*b, text, run, false);
    // Kerning on by default: "Ta" is tighter without it off.
    let unkerned = shape_with(&*b, text, run, false, 0.0, 1.0, &font::parse_font_feature_settings("kern 0"));
    assert!(unkerned.advance > plain.advance);
    // Stretched advances.
    let wide = shape_with(&*b, text, run, false, 0.0, 2.0, &[]);
    assert!(near(wide.advance, plain.advance * 2.0));
    // Letter spacing: half before each cluster, half after.
    let spaced = shape_with(&*b, text, run, false, 4.0, 1.0, &[]);
    let clusters = {
        let mut c: Vec<usize> = spaced.glyphs.iter().map(|g| g.cluster).collect();
        c.dedup();
        c.len()
    };
    assert!(near(spaced.advance, plain.advance + 4.0 * clusters as f32));
    assert!(near(spaced.glyphs[0].x_offset, plain.glyphs[0].x_offset + 2.0));
    // A language picks localized forms; the request's script is used as given.
    let tr = Locale::new("tr");
    let localized = b.shape(&ShapeRequest {
        text,
        range: run.range.clone(),
        font: &run.font,
        rtl: false,
        script: Some(*b"Latn"),
        language: Some(&tr),
        features: &[],
        letter_spacing: 0.0,
        scale_x: 1.0,
        linear: false,
    });
    assert_eq!(localized.glyphs.len(), plain.glyphs.len());
    // A size scales everything.
    let big = FontRun { range: run.range.clone(), font: FontInstance { size: 40.0, ..run.font.clone() } };
    assert!(near(shape_run(&*b, text, &big, false).advance, plain.advance * 2.0));
}

/// All of `text` (one run) shaped `spacing` px apart, stretched by `scale_x`, as linear text or still.
fn spaced(
    b: &dyn FontBackend,
    text: &str,
    run: &FontRun,
    rtl: bool,
    spacing: f32,
    scale_x: f32,
    linear: bool,
) -> ShapedRun {
    b.shape(&ShapeRequest {
        text,
        range: run.range.clone(),
        font: &run.font,
        rtl,
        script: None,
        language: None,
        features: &[],
        letter_spacing: spacing,
        scale_x,
        linear,
    })
}

/// Whether `spaced` is `plain` spaced as minikin spaces clusters, `before` px before each and `after` after it:
/// the same glyphs, each moved in by `before` (a cluster's marks with its base), each cluster's last glyph
/// carrying both, its others' advances as they were.
fn spaced_as_minikin(plain: &ShapedRun, spaced: &ShapedRun, before: f32, after: f32) -> bool {
    let n = plain.glyphs.len();
    let clusters = plain.glyphs.windows(2).filter(|w| w[0].cluster != w[1].cluster).count() + usize::from(n > 0);
    spaced.glyphs.len() == n
        && near(spaced.advance, plain.advance + (before + after) * clusters as f32)
        && plain.glyphs.iter().zip(&spaced.glyphs).enumerate().all(|(i, (p, s))| {
            let last_of_cluster = plain.glyphs.get(i + 1).is_none_or(|next| next.cluster != p.cluster);
            let carried = if last_of_cluster { before + after } else { 0.0 };
            p.id == s.id
                && p.cluster == s.cluster
                && near(s.x_offset, p.x_offset + before)
                && near(s.y_offset, p.y_offset)
                && near(s.x_advance, p.x_advance + carried)
        })
}

#[test]
fn letter_spacing_goes_around_clusters_in_whole_pixels_unless_linear() {
    let r = resolver();
    let b = r.backend();
    let t = default_typeface(&r);
    let one_run = |text: &str| {
        let runs = runs(&*b, text, &t, 14.0);
        assert_eq!(runs.len(), 1, "{text}");
        runs.into_iter().next().unwrap()
    };
    // Material 3's titleSmall at density 1: 14 px, 0.1 px apart. Still text is spaced in whole pixels (minikin's,
    // unless the text is linear), so 0.1 px is none: "Sayaç" shaped as it is without it, nothing gathering into a
    // pixel's jump before the "ç" where its glyphs are drawn on whole pixels.
    let title = "Sayaç";
    let run = one_run(title);
    let plain = shape_run(&*b, title, &run, false);
    assert_eq!(plain.glyphs.len(), 5, "the ç one glyph, as all the others");
    assert_eq!(spaced(&*b, title, &run, false, 0.1, 1.0, false), plain);
    // Linear text keeps it, half before each cluster, half after.
    assert!(spaced_as_minikin(&plain, &spaced(&*b, title, &run, false, 0.1, 1.0, true), 0.05, 0.05));
    // 0.1 em of 14 px: 1.4 px, one whole pixel, after each cluster (the half before rounded down); 3 px: one
    // before, two after; half a pixel is a whole one (rounded half away from zero, as C's round).
    let cases = [(1.4, false, 0.0, 1.0), (1.4, true, 0.7, 0.7), (3.0, false, 1.0, 2.0), (0.5, false, 0.0, 1.0)];
    for (spacing, linear, before, after) in cases {
        let s = spaced(&*b, title, &run, false, spacing, 1.0, linear);
        assert!(spaced_as_minikin(&plain, &s, before, after), "{spacing} {linear}: {s:?}");
    }
    // The spacing is stretched with the text (minikin's letterSpacing · size · scaleX), then rounded.
    let wide = spaced(&*b, title, &run, false, 0.0, 1.5, false);
    assert!(spaced_as_minikin(&wide, &spaced(&*b, title, &run, false, 2.0, 1.5, true), 1.5, 1.5));
    assert!(spaced_as_minikin(&wide, &spaced(&*b, title, &run, false, 2.0, 1.5, false), 1.0, 2.0));
    assert!(spaced_as_minikin(&wide, &spaced(&*b, title, &run, false, 0.3, 1.5, false), 0.0, 0.0));

    // Decomposed, the cedilla is composed into the same glyph, one cluster with its c: spaced once.
    let decomposed = "Sayac\u{327}";
    let run = one_run(decomposed);
    let composed = shape_run(&*b, decomposed, &run, false);
    assert_eq!(glyph_ids(&composed), glyph_ids(&plain));
    for (spacing, linear, before, after) in cases {
        let s = spaced(&*b, decomposed, &run, false, spacing, 1.0, linear);
        assert!(spaced_as_minikin(&composed, &s, before, after), "{decomposed:?} {spacing} {linear}: {s:?}");
    }
    // Turkish letters with marks, precomposed, and a letter with a mark the font has no composition for (s and
    // an acute: two glyphs, one cluster): a cluster's mark moves with its base and adds no spacing of its own.
    let accented = "s\u{301}";
    let s_acute = shape_run(&*b, accented, &one_run(accented), false);
    assert!(s_acute.glyphs.len() == 2 && s_acute.glyphs.iter().all(|g| g.cluster == 0), "{s_acute:?}");
    for text in ["şğİ", "Iİıi", "ŞĞÜÖÇ", accented, "kas\u{301}a"] {
        let run = one_run(text);
        let plain = shape_run(&*b, text, &run, false);
        for (spacing, linear, before, after) in cases {
            let s = spaced(&*b, text, &run, false, spacing, 1.0, linear);
            assert!(spaced_as_minikin(&plain, &s, before, after), "{text:?} {spacing} {linear}: {s:?}");
        }
    }

    // Scripts whose letters join or hang from a line aren't spaced at all (minikin's isScriptOkForLetterspacing):
    // Arabic (a harakah with its letter too), Devanagari (a vowel sign reordered before its consonant).
    for (text, rtl) in [("بب ب", true), ("بَت", true), ("कि नमस्ते", false)] {
        let run = one_run(text);
        let plain = shape_run(&*b, text, &run, rtl);
        for (spacing, linear) in [(1.4, false), (3.0, false), (1.4, true)] {
            assert_eq!(spaced(&*b, text, &run, rtl, spacing, 1.0, linear), plain, "{text} {spacing} {linear}");
        }
    }
}

#[test]
fn shaped_runs_and_plans_are_cached() {
    let r = resolver();
    let b = r.harfrust_backend();
    let t = default_typeface(&r);
    let text = "Merhaba dünya";
    let run = &runs(&**b, text, &t, 16.0)[0];
    let before = b.cache_stats();
    let first = shape_run(&**b, text, run, false);
    let after_first = b.cache_stats();
    assert_eq!(after_first.run_misses, before.run_misses + 1);
    assert_eq!(after_first.plan_misses, before.plan_misses + 1);
    // The same run again: from the cache.
    assert_eq!(shape_run(&**b, text, run, false), first);
    let again = b.cache_stats();
    assert_eq!((again.run_hits, again.run_misses), (after_first.run_hits + 1, after_first.run_misses));
    // At another size or letter spacing: shaped once, scaled.
    let bigger = FontRun { range: run.range.clone(), font: FontInstance { size: 32.0, ..run.font.clone() } };
    assert!(near(shape_run(&**b, text, &bigger, false).advance, first.advance * 2.0));
    shape_with(&**b, text, run, false, 3.0, 1.0, &[]);
    assert_eq!(b.cache_stats().run_misses, after_first.run_misses);
    // Another text in the same font: shaped, its plan reused.
    shape_run(&**b, "Başka", &runs(&**b, "Başka", &t, 16.0)[0], false);
    let other = b.cache_stats();
    assert!(other.run_misses == after_first.run_misses + 1 && other.plan_misses == after_first.plan_misses);
    assert!(other.plan_hits > after_first.plan_hits && other.runs >= 2 && other.plans >= 1);
    // The same word in other text around it is shaped again (joining and kerning see across the cut).
    let long = "Merhaba dünyalar";
    let part = FontRun { range: 0..text.len(), font: run.font.clone() };
    shape_run(&**b, long, &part, false);
    assert_eq!(b.cache_stats().run_misses, other.run_misses + 1);
}

#[test]
fn a_collection_without_fonts_shapes_missing_glyphs() {
    let r = FontFamilyResolverImpl::registered_only();
    let b = r.backend();
    let t = default_typeface(&r);
    let text = "ab";
    let runs = runs(&*b, text, &t, 16.0);
    assert_eq!(runs.len(), 1);
    let shaped = shape_run(&*b, text, &runs[0], false);
    assert!(shaped.glyphs.iter().all(|g| g.id == 0) && shaped.advance == 0.0);
    assert_eq!(b.metrics(&runs[0].font), FontMetrics::default());
    let empty = FontCollection::registered_only();
    assert!(empty.family_names().is_empty() && !empty.has_system_fonts());
}
