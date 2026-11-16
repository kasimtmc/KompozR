//! The paragraph engine over real fonts (the test fonts through the harfrust backend): Kompoz Test laying out as
//! the fake does, Arabic joined and right to left (its carets and selection where its letters are drawn, a word
//! cut across lines shaped again), Devanagari reordered and kept whole, emoji in their color font, lines growing
//! for the fallback fonts on them, an Arabic line ellipsized at its left, a title letter spaced in whole pixels.

mod common;

use common::*;
use kompoz_graphics::*;
use kompoz_text::*;
use std::rc::Rc;

fn lay(
    resolver: Rc<dyn FontFamilyResolver>,
    text: &str,
    style: TextStyle,
    width: i32,
    overflow: TextOverflow,
    max_lines: usize,
) -> TextLayoutResult {
    let m = TextMeasurer::new(resolver, Density::new(1.0, 1.0), LayoutDirection::Ltr, 0);
    m.measure_with(
        text,
        style,
        overflow,
        true,
        max_lines,
        Vec::new(),
        Constraints::new(0, width, 0, Constraints::Infinity),
        LayoutDirection::Ltr,
        m.default_density(),
        m.default_font_family_resolver().clone(),
        false,
    )
}

fn style(px: f32) -> TextStyle {
    TextStyle { font_size: px.sp(), text_direction: TextDirection::Content, ..TextStyle::Default }
}

/// Each drawn run: its text, its left, its glyph runs' fonts and glyph counts.
fn runs(r: &TextLayoutResult) -> Vec<(String, f32, Vec<(FontId, usize)>, bool)> {
    r.multi_paragraph
        .paint_ops(&TextPaintStyle::default())
        .into_iter()
        .filter_map(|op| match op {
            TextPaintOp::Text { text, .. } => Some((
                text.text.to_string(),
                text.baseline.x,
                text.glyphs.iter().map(|g| (g.font, g.glyphs.len())).collect(),
                text.rtl,
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn kompoz_test_lays_out_as_the_fake_does() {
    let real = lay(
        resolver_with_default("Kompoz Test"),
        "Merhaba dünya",
        style(20.0),
        80,
        TextOverflow::Clip,
        DefaultMaxLines,
    );
    let fake = lay(
        Rc::new(FakeFontFamilyResolver::new()),
        "Merhaba dünya",
        style(20.0),
        80,
        TextOverflow::Clip,
        DefaultMaxLines,
    );
    assert_eq!((real.size, real.line_count()), (fake.size, fake.line_count()));
    for i in 0..real.line_count() {
        assert_eq!(real.get_line_end(i, false), fake.get_line_end(i, false));
        assert!(near(real.get_line_baseline(i), fake.get_line_baseline(i)));
        assert!(near(real.get_line_right(i), fake.get_line_right(i)));
    }
    assert!(near(real.get_cursor_rect(9).left, fake.get_cursor_rect(9).left));
    let texts: Vec<String> = runs(&real).into_iter().map(|r| r.0).collect();
    assert_eq!(texts, ["Merhaba", "dünya"]);
}

#[test]
fn arabic_joins_and_goes_right_to_left() {
    let resolver: Rc<dyn FontFamilyResolver> = resolver();
    // "مرحبا بالعالم": right to left by its content, in Noto Sans Arabic (a fallback of Noto Sans).
    let text = "مرحبا بالعالم";
    let r = lay(resolver.clone(), text, style(20.0), Constraints::Infinity, TextOverflow::Clip, DefaultMaxLines);
    assert_eq!(r.line_count(), 1);
    assert_eq!(r.get_paragraph_direction(0), ResolvedTextDirection::Rtl);
    let drawn = runs(&r);
    assert_eq!(drawn.len(), 1);
    assert!(drawn[0].3 && drawn[0].0 == text);
    // Its first letter's caret at the right edge, its end at the left.
    let width = r.size.width as f32;
    assert!((r.get_cursor_rect(0).left - r.get_line_right(0)).abs() < 0.5);
    assert!((r.get_cursor_rect(text.len()).left - r.get_line_left(0)).abs() < 0.5);
    assert!(r.get_line_right(0) <= width + 0.5);
    // The first word's box is on the right, the second's on the left.
    let first = r.get_path_for_range(0, "مرحبا".len()).get_bounds();
    let second = r.get_path_for_range("مرحبا ".len(), text.len()).get_bounds();
    assert!(first.left > second.right - 0.5);
    // Joined: a letter inside a word is drawn in another form than alone.
    let alone = lay(resolver.clone(), "ح", style(20.0), Constraints::Infinity, TextOverflow::Clip, DefaultMaxLines);
    let alone_id = glyph_ids(&alone)[0];
    let ids = glyph_ids(&r);
    assert!(!ids.contains(&alone_id));
    // Two words a line where the width allows only one; a word wider than the line is cut and its pieces shaped
    // again (their widths add up to the word's).
    let narrow = lay(resolver.clone(), text, style(20.0), (width * 0.6) as i32, TextOverflow::Clip, DefaultMaxLines);
    assert_eq!(narrow.line_count(), 2);
    assert_eq!(narrow.get_line_end(0, true), "مرحبا".len());
    let word = "بالعالم";
    let whole = lay(resolver.clone(), word, style(20.0), Constraints::Infinity, TextOverflow::Clip, DefaultMaxLines);
    let cut = lay(resolver, word, style(20.0), (whole.size.width / 2).max(1), TextOverflow::Clip, DefaultMaxLines);
    assert!(cut.line_count() >= 2);
    let sum: f32 = (0..cut.line_count()).map(|i| cut.get_line_right(i) - cut.get_line_left(i)).sum();
    assert!((sum - whole.get_line_right(0) + whole.get_line_left(0)).abs() < 2.0, "{sum}");
}

fn glyph_ids(r: &TextLayoutResult) -> Vec<u32> {
    r.multi_paragraph
        .paint_ops(&TextPaintStyle::default())
        .into_iter()
        .filter_map(|op| match op {
            TextPaintOp::Text { text, .. } => {
                Some(text.glyphs.iter().flat_map(|g| g.glyphs.iter().map(|g| g.id)).collect::<Vec<_>>())
            }
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn devanagari_is_reordered_kept_whole_and_its_lines_grow() {
    // Kompoz Test lacks Devanagari: "हिन्दी" falls back to Noto Sans Devanagari, which reaches higher and lower than
    // Kompoz Test (0.8 and 0.45 em): its line grows (Android's fallback line spacing).
    let resolver = resolver_with_default("Kompoz Test");
    let plain = lay(resolver.clone(), "ab", style(20.0), Constraints::Infinity, TextOverflow::Clip, DefaultMaxLines);
    let hindi =
        lay(resolver.clone(), "ab हिन्दी", style(20.0), Constraints::Infinity, TextOverflow::Clip, DefaultMaxLines);
    assert!(hindi.get_line_bottom(0) > plain.get_line_bottom(0));
    assert!(hindi.first_baseline() > plain.first_baseline());
    let drawn = runs(&hindi);
    assert_eq!(drawn.len(), 1);
    let fonts: Vec<FontId> = drawn[0].2.iter().map(|g| g.0).collect();
    assert_eq!(fonts.len(), 2);
    assert_ne!(fonts[0], fonts[1]);
    // Its conjunct and vowel signs: fewer glyphs than characters.
    assert!(drawn[0].2[1].1 < "हिन्दी".chars().count());
    // A caret never lands inside a grapheme: "हि" is one.
    let at = hindi.get_offset_for_position(Offset::new(hindi.get_horizontal_position(3, true) + 3.0, 5.0));
    assert!(at == 3 || at == "ab हि".len(), "{at}");
}

#[test]
fn emoji_are_drawn_in_their_color_font() {
    let resolver = resolver();
    let text = "ok 👍🏽 ok";
    let r = lay(resolver, text, style(20.0), Constraints::Infinity, TextOverflow::Clip, DefaultMaxLines);
    let ops = r.multi_paragraph.paint_ops(&TextPaintStyle::default());
    let TextPaintOp::Text { text: draw, .. } = &ops[0] else { panic!("a text") };
    let kinds: Vec<GlyphColorKind> = draw.glyphs.iter().map(|g| g.color).collect();
    assert!(kinds.contains(&GlyphColorKind::Bitmap) && kinds.contains(&GlyphColorKind::Outline));
    // The skin toned thumb is one glyph and one caret stop.
    let emoji = draw.glyphs.iter().find(|g| g.color == GlyphColorKind::Bitmap).expect("the emoji's run");
    assert_eq!(emoji.glyphs.len(), 1);
    let start = 3;
    let end = start + "👍🏽".len();
    let box_ = r.get_bounding_box(start);
    assert!(box_.width() > 10.0);
    assert!(near(r.get_horizontal_position(end, true), box_.right));
}

#[test]
fn an_arabic_line_is_ellipsized_at_its_left() {
    let resolver = resolver();
    let text = "مرحبا بالعالم مرحبا";
    let full = lay(resolver.clone(), text, style(20.0), Constraints::Infinity, TextOverflow::Clip, DefaultMaxLines);
    let r = lay(resolver, text, style(20.0), full.size.width / 2, TextOverflow::Ellipsis, 1);
    assert!(r.is_line_ellipsized(0) && r.line_count() == 1);
    let drawn = runs(&r);
    assert!(drawn.iter().any(|d| d.0.ends_with('…') && d.3));
    // At the paragraph's end (its left): the ellipsis is the leftmost glyph.
    assert!(r.get_line_left(0) >= -0.5 && r.get_line_right(0) <= (full.size.width / 2) as f32 + 0.5);
}

/// Each drawn glyph's x (its run's baseline's and its own), in order.
fn glyph_xs(r: &TextLayoutResult) -> Vec<f32> {
    r.multi_paragraph
        .paint_ops(&TextPaintStyle::default())
        .into_iter()
        .filter_map(|op| match op {
            TextPaintOp::Text { text, .. } => Some(
                text.glyphs.iter().flat_map(|g| g.glyphs.iter().map(|g| text.baseline.x + g.x)).collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn a_title_letter_spaced_a_tenth_of_a_pixel_is_drawn_where_it_is_without_it() {
    // Material 3's titleSmall at density 1: 14 sp, 0.1 sp apart. Still text is spaced in whole pixels (minikin's,
    // unless the text is linear), so 0.1 px is none. Spaced by the tenths, its glyphs drawn on whole pixels jumped
    // a pixel apart where the tenths added up past a half (in Noto Sans Medium before the "ç": at 31.56 after the
    // "a" at 23.45, drawn at 32 and 23, where without the spacing they are at 31.11 and 23.1, drawn at 31 and 23).
    let title = |spacing: f32, motion: TextMotion| TextStyle {
        letter_spacing: spacing.sp(),
        font_weight: Some(FontWeight::Medium),
        text_motion: Some(motion),
        ..style(14.0)
    };
    let lay_title =
        |style: TextStyle| lay(resolver(), "Sayaç", style, Constraints::Infinity, TextOverflow::Ellipsis, 1);
    let plain = lay_title(title(0.0, TextMotion::Static));
    let xs = glyph_xs(&plain);
    assert_eq!(xs.len(), 5);
    let spaced = lay_title(title(0.1, TextMotion::Static));
    assert_eq!(glyph_xs(&spaced), xs);
    assert!(near(spaced.get_horizontal_position(4, true), plain.get_horizontal_position(4, true)));
    // Compose still measures it as letter spaced: half a pixel wider (so StaticLayout won't wrap it).
    let widest = |r: &TextLayoutResult| r.multi_paragraph.intrinsics().max_intrinsic_width();
    assert!(near(widest(&spaced), widest(&plain) + 0.5));
    // Linear text (TextMotion.Animated) keeps the tenths: half of one before each letter, half after.
    let animated = glyph_xs(&lay_title(title(0.1, TextMotion::Animated)));
    assert!(
        xs.iter().zip(&animated).enumerate().all(|(i, (x, a))| near(*a, x + 0.05 + 0.1 * i as f32)),
        "{animated:?}"
    );
    // 0.1 em (1.4 px): one whole pixel after each letter.
    let em = glyph_xs(&lay_title(TextStyle { letter_spacing: 0.1.em(), ..title(0.0, TextMotion::Static) }));
    assert!(xs.iter().zip(&em).enumerate().all(|(i, (x, e))| near(*e, x + i as f32)), "{em:?}");
}
