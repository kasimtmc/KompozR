//! The paragraph engine over the fake backend (the C++ Kompoz's FakeTextMeasurer: half an em a code point, ascent
//! 0.8, descent 0.2, leading 0.25): what TextMeasurer lays out and how it is drawn, as the C++ Kompoz's tests saw it
//! (text_measurer_lays_text_out_for_drawing, line_height_spaces_lines_but_not_above_the_first_or_below_the_last,
//! line_height_style_aligns_and_trims_lines, justified_lines_stretch_at_their_spaces,
//! text_indent_moves_first_and_other_lines_in, line_break_strategies_pick_where_lines_break,
//! min_lines_start_and_middle_ellipsis_and_baseline_shift, styled_text_of_both_directions_is_ordered_as_one_line,
//! letter_spacing_widens_text_and_where_its_carets_are), with Compose's and Android's answers where they differ
//! (each said where); then carets at line ends and after a trailing line feed, bidi carets and selection paths,
//! ellipsis in right-to-left text, line height styles, placeholders, paragraph styles, intrinsics, the range a
//! rect takes, hyphen edits, the layout cache, and the draw list kept from frame to frame.

use kompoz_graphics::*;
use kompoz_text::line_break::{EndHyphenEdit, HyphenationType, Hyphenator, set_hyphenator};
use kompoz_text::style::line_height_style::{Alignment, Mode, Trim};
use kompoz_text::*;
use std::rc::Rc;

fn fake() -> Rc<dyn FontFamilyResolver> {
    Rc::new(FakeFontFamilyResolver::new())
}

fn measurer() -> TextMeasurer {
    TextMeasurer::new(fake(), Density::new(1.0, 1.0), LayoutDirection::Ltr, 8)
}

/// `text` laid out by a measurer over the fake: in `style`, `width` px at most (or Infinity), the rest Compose's
/// defaults.
fn lay(text: impl Into<AnnotatedString>, style: TextStyle, width: i32) -> TextLayoutResult {
    lay_with(text, style, width, TextOverflow::Clip, true, DefaultMaxLines, Constraints::Infinity)
}

fn lay_with(
    text: impl Into<AnnotatedString>,
    style: TextStyle,
    width: i32,
    overflow: TextOverflow,
    soft_wrap: bool,
    max_lines: usize,
    height: i32,
) -> TextLayoutResult {
    let m = measurer();
    m.measure_with(
        text,
        style,
        overflow,
        soft_wrap,
        max_lines,
        Vec::new(),
        Constraints::new(0, width, 0, height),
        LayoutDirection::Ltr,
        m.default_density(),
        m.default_font_family_resolver().clone(),
        false,
    )
}

/// `text` laid out exactly `width` px wide.
fn lay_fixed(text: impl Into<AnnotatedString>, style: TextStyle, width: i32) -> TextLayoutResult {
    let m = measurer();
    m.measure_with(
        text,
        style,
        TextOverflow::Clip,
        true,
        DefaultMaxLines,
        Vec::new(),
        Constraints::fixed_width(width),
        LayoutDirection::Ltr,
        m.default_density(),
        m.default_font_family_resolver().clone(),
        false,
    )
}

/// A style of that font size, its paragraphs going the way their first strong character goes (C++'s default;
/// Compose's text goes the layout's way unless told).
fn content(px: f32) -> TextStyle {
    TextStyle { text_direction: TextDirection::Content, ..size(px) }
}

fn size(px: f32) -> TextStyle {
    TextStyle { font_size: px.sp(), ..TextStyle::Default }
}

/// What a layout draws: each run's text, where its baseline starts and whether it goes right to left.
fn drawn(r: &TextLayoutResult) -> Vec<(String, f32, f32, bool)> {
    r.multi_paragraph
        .paint_ops(&TextPaintStyle::default())
        .into_iter()
        .filter_map(|op| match op {
            TextPaintOp::Text { text, .. } => Some((text.text.to_string(), text.baseline.x, text.baseline.y, text.rtl)),
            _ => None,
        })
        .collect()
}

fn texts(r: &TextLayoutResult) -> Vec<String> {
    drawn(r).into_iter().map(|d| d.0).collect()
}

/// Where the run drawing `text` starts (x, baseline y).
fn at(r: &TextLayoutResult, text: &str) -> (f32, f32) {
    let d = drawn(r);
    let found = d.iter().find(|d| d.0 == text).unwrap_or_else(|| panic!("no {text:?} drawn among {d:?}"));
    (found.1, found.2)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

fn same_rect(r: Rect, left: f32, top: f32, right: f32, bottom: f32) -> bool {
    near(r.left, left) && near(r.top, top) && near(r.right, right) && near(r.bottom, bottom)
}

fn bold(text: &str) -> (SpanStyle, String) {
    (SpanStyle { font_weight: Some(FontWeight::Bold), ..SpanStyle::Default }, text.to_string())
}

/// Plain pieces and styled ones, in order.
fn styled(pieces: &[(Option<SpanStyle>, &str)]) -> AnnotatedString {
    build_annotated_string(|b| {
        for (style, text) in pieces {
            match style {
                Some(s) => b.with_style(s.clone(), |b| b.append(*text)),
                None => b.append(*text),
            }
        }
    })
}

#[test]
fn text_measurer_lays_text_out() {
    // C++'s text_measurer_lays_text_out_for_drawing, its measuring part. A code point is half the font size wide;
    // ascent 0.8, line height 1.25 of it. Compose's line end takes the trailing space (C++'s left it out: 7);
    // the visible end leaves it out.
    let one = lay("Merhaba dünya", size(20.0), Constraints::Infinity);
    assert_eq!((one.size, one.line_count()), (IntSize::new(130, 25), 1));
    assert!(near(one.first_baseline(), 16.0) && !one.has_visual_overflow());
    let two = lay("Merhaba dünya", TextStyle { text_align: TextAlign::Center, ..size(20.0) }, 80);
    assert_eq!((two.line_count(), two.size), (2, IntSize::new(80, 50))); // wrapped: as wide as it may be
    assert_eq!((two.get_line_start(0), two.get_line_end(0, false), two.get_line_end(0, true)), (0, 8, 7));
    assert_eq!((two.get_line_start(1), two.get_line_end(1, false)), (8, 14));
    assert!(near(two.get_line_top(1), 25.0) && near(two.get_line_bottom(1), 50.0));
    assert!(near(two.get_line_baseline(1), 41.0) && near(two.last_baseline(), 41.0));
    // Centered in 80.
    assert!(near(two.get_line_left(0), 5.0) && near(two.get_line_left(1), 15.0) && near(two.get_line_right(1), 65.0));
    assert_eq!((two.get_line_for_offset(3), two.get_line_for_offset(10), two.get_line_for_offset(14)), (0, 1, 1));
    assert_eq!(two.get_line_for_vertical_position(10.0), 0);
    assert_eq!(two.get_line_for_vertical_position(30.0), 1);
    assert_eq!(two.get_line_for_vertical_position(99.0), 1);
    assert_eq!(two.get_offset_for_position(Offset::new(20.0, 30.0)), 8);
    assert_eq!(two.get_offset_for_position(Offset::new(16.0, 5.0)), 1);
    assert_eq!(two.get_offset_for_position(Offset::new(200.0, 30.0)), 14);
    // "d", then "ü" (two bytes).
    assert!(same_rect(two.get_bounding_box(8), 15.0, 25.0, 25.0, 50.0));
    assert!(same_rect(two.get_bounding_box(9), 25.0, 25.0, 35.0, 50.0));
    assert!(same_rect(two.get_cursor_rect(9), 25.0, 25.0, 25.0, 50.0));
    // Selecting "hab " of the first line and "dü" of the second: Android's selection path also takes the room
    // past a line it goes on from (to the paragraph's edge) and before the line it ends on.
    assert!(same_rect(two.get_path_for_range(3, 6).get_bounds(), 35.0, 0.0, 65.0, 25.0));
    assert!(same_rect(two.get_path_for_range(3, 10).get_bounds(), 0.0, 0.0, 85.0, 50.0));
    assert!(two.get_path_for_range(4, 4).is_empty());
    let cut = lay_with("Merhaba dünya", size(20.0), 80, TextOverflow::Ellipsis, true, 1, Constraints::Infinity);
    assert!(cut.line_count() == 1 && cut.is_line_ellipsized(0) && cut.did_overflow_height());
    assert!(!cut.did_overflow_width() && cut.has_visual_overflow());
    assert_eq!(texts(&cut), ["Merhaba…"]);
    assert_eq!((cut.get_line_end(0, false), cut.get_line_end(0, true)), (14, 7));
    let wide =
        lay_with("Merhaba dünya", size(20.0), 80, TextOverflow::Clip, false, DefaultMaxLines, Constraints::Infinity);
    assert!(wide.did_overflow_width() && !wide.did_overflow_height() && wide.line_count() == 1);
    let tall = lay_with("a\nb\nc", size(20.0), Constraints::Infinity, TextOverflow::Clip, true, DefaultMaxLines, 60);
    assert!(tall.line_count() == 3 && tall.did_overflow_height() && !tall.is_line_ellipsized(2));
    assert_eq!(texts(&two), ["Merhaba", "dünya"]);
    assert_eq!(at(&two, "dünya"), (15.0, 41.0));
}

#[test]
fn line_height_spaces_lines_but_not_above_the_first_or_below_the_last() {
    // C++'s test, through a measurer. 10 px: ascent 8, line 12.5 (descent and leading 4.5 below). 30 px lines
    // gain 17.5: 64% above, 36% below; the first keeps its top, the last its bottom.
    let lh = |h: f32| TextStyle { line_height: h.sp(), ..size(10.0) };
    let three = lay("a\nb\nc", lh(30.0), Constraints::Infinity);
    assert_eq!(at(&three, "a").1, 8.0);
    assert!(near(at(&three, "b").1, 38.0) && near(at(&three, "c").1, 68.0)); // baseline to baseline: 30
    assert_eq!(three.size.height, 73); // 18.8 + 30 + 23.7
    assert!(near(three.get_line_baseline(1), 38.0));
    // Where one line ends and the next begins: the 17.5 gained, 36% below the first line.
    assert!(near(three.get_line_bottom(0), 8.0 + 4.5 + 6.3) && near(three.get_line_top(1), 18.8));
    assert_eq!(lay("tek", lh(30.0), Constraints::Infinity).size.height, 13); // one line: as it was
    // Shorter: 4.5 lost, 64% from below the line.
    let tight = lay("d\ne", lh(8.0), Constraints::Infinity);
    assert!(near(at(&tight, "d").1, 8.0) && near(at(&tight, "e").1 - at(&tight, "d").1, 8.0));
    assert_eq!(tight.size.height, 21);
    assert!(near(tight.get_line_bottom(0), 8.0 + 4.5 - 2.88));
    // Line heights are sp: they grow with the font scale (the fake's font sizes too, D12).
    let m = measurer();
    let scaled = m.measure_with(
        "a\nb",
        lh(30.0),
        TextOverflow::Clip,
        true,
        DefaultMaxLines,
        Vec::new(),
        Constraints::default(),
        LayoutDirection::Ltr,
        Density::new(1.0, 2.0),
        fake(),
        false,
    );
    assert!(near(at(&scaled, "b").1 - at(&scaled, "a").1, 60.0));
}

#[test]
fn line_height_style_aligns_and_trims_lines() {
    // C++'s test, through a measurer. 10 px: ascent 8, line 12.5; a 30 px line gains 17.5: none of it above
    // (Top), half (Center) or all (Bottom); untrimmed, every line is 30 tall.
    let style = |h: f32, alignment: Alignment, trim: Trim, mode: Mode| TextStyle {
        line_height: h.sp(),
        line_height_style: Some(LineHeightStyle::new(alignment, trim, mode)),
        ..size(10.0)
    };
    let check = |text: &str, s: TextStyle, first: f32, second: f32, height: i32| {
        let r = lay(text, s, Constraints::Infinity);
        let (a, b) = (&text[0..1], &text[2..3]);
        assert!(near(at(&r, a).1, first) && near(at(&r, b).1, second) && r.size.height == height, "{text}: {r:?}");
    };
    check("a\nb", style(30.0, Alignment::Top, Trim::None, Mode::Fixed), 8.0, 38.0, 60);
    check("c\nd", style(30.0, Alignment::Center, Trim::None, Mode::Fixed), 16.75, 46.75, 60);
    check("e\nf", style(30.0, Alignment::Bottom, Trim::None, Mode::Fixed), 25.5, 55.5, 60);
    // A trimmed first top or last bottom keeps the font's: 8.75 less (51.25, rounded up).
    check("g\nh", style(30.0, Alignment::Center, Trim::FirstLineTop, Mode::Fixed), 8.0, 38.0, 52);
    check("i\nj", style(30.0, Alignment::Center, Trim::LastLineBottom, Mode::Fixed), 16.75, 46.75, 52);
    // Shorter than the font: Fixed takes 8.5 off the line (64% from below, as the ascent stands) but pads the
    // text's top and bottom for the glyphs it would cut (Compose's LineHeightStyleSpan paddings; C++ cut them:
    // 4 high, the baseline at 4.94); Minimum doesn't shrink the line.
    let fixed = lay("k", style(4.0, Alignment::Proportional, Trim::None, Mode::Fixed), Constraints::Infinity);
    assert!(fixed.size.height == 13 && near(at(&fixed, "k").1, 8.0));
    let minimum = lay("l", style(4.0, Alignment::Proportional, Trim::None, Mode::Minimum), Constraints::Infinity);
    assert!(minimum.size.height == 13 && near(at(&minimum, "l").1, 8.0));
    // Tight: no padding, the line its line height.
    let tight = lay("m\nn", style(8.0, Alignment::Proportional, Trim::None, Mode::Tight), Constraints::Infinity);
    assert!(near(tight.get_line_top(1), 8.0 + 4.5 - 2.88) && tight.size.height == 21);
}

#[test]
fn justified_lines_stretch_at_their_spaces() {
    // C++'s test. 5 px a letter: "aa bb cc" (40) fills the first line, 2 px short of 42: a pixel more in each gap.
    let justify = || TextStyle { text_align: TextAlign::Justify, ..size(10.0) };
    let left = lay("aa bb cc dd ee", justify(), 42);
    assert_eq!(texts(&left), ["aa ", "bb ", "cc", "dd ee"]);
    assert!(at(&left, "aa ").0 == 0.0 && at(&left, "bb ").0 == 16.0 && at(&left, "cc").0 == 32.0);
    assert_eq!(at(&left, "dd ee").0, 0.0); // the paragraph's last line: at its start
    assert_eq!(left.size.width, 42);
    // Carets follow the words.
    assert!(near(left.get_cursor_rect(3).left, 16.0) && near(left.get_cursor_rect(8).left, 42.0));
    assert!(near(left.get_line_right(0), 42.0) && near(left.get_line_right(1), 25.0));
    // Right to left (by its content: Compose 1.12's text goes the layout's way unless told), from the right edge,
    // the last line at the right; a stretched space goes with the word before it, on its left (C++ put the gap
    // past it: "אא " at 27, "בב " at 11).
    let rtl = lay("אא בב גג דד הה", TextStyle { text_direction: TextDirection::Content, ..justify() }, 42);
    assert!(at(&rtl, "אא ").0 == 26.0 && at(&rtl, "בב ").0 == 10.0 && at(&rtl, "גג").0 == 0.0);
    assert_eq!(at(&rtl, "דד הה").0, 17.0);
    // A line of both directions is justified too (Android justifies it; C++ left it as it was), its runs apart.
    let mixed = lay("ff וו gg hh ii", justify(), 42);
    assert_eq!(texts(&mixed), ["ff ", "וו", " ", "gg", "hh ii"]);
    assert!(at(&mixed, "ff ").0 == 0.0 && at(&mixed, "וו").0 == 16.0 && at(&mixed, "gg").0 == 32.0);
    // Not wrapped: nothing to justify.
    let unwrapped =
        lay_with("jj kk ll mm", justify(), 42, TextOverflow::Clip, false, DefaultMaxLines, Constraints::Infinity);
    assert_eq!(texts(&unwrapped), ["jj kk ll mm"]);
    // Two spaces at the line's end: both hang (they are line 0's), so the gap between the words takes it all.
    let spaced = lay("qq ww  ee rr", justify(), 42);
    assert_eq!((spaced.get_line_end(0, false), spaced.get_line_end(0, true)), (7, 5));
    assert_eq!(at(&spaced, "ww").0, 32.0);
}

#[test]
fn text_indent_moves_first_and_other_lines_in() {
    // C++'s test, through a measurer.
    let indent =
        |first: f32, rest: f32| TextStyle { text_indent: Some(TextIndent::new(first.sp(), rest.sp())), ..size(10.0) };
    let measured = lay("aa bb cc", TextStyle { text_align: TextAlign::End, ..indent(10.0, 0.0) }, 42);
    assert!(measured.line_count() == 2 && measured.size.width == 42); // as wide as it may be
    let both = lay("aa bb cc dd", indent(10.0, 5.0), 42); // the first line has 32 px
    assert!(at(&both, "aa bb").0 == 10.0 && at(&both, "cc dd").0 == 5.0);
    let later = lay("pp rr ss tt", indent(10.0, 0.0), 42); // "pp rr ss" (40) only fits later lines
    assert!(at(&later, "pp rr").0 == 10.0 && at(&later, "ss tt").0 == 0.0);
    let every = lay("ee\nff", indent(6.0, 0.0), Constraints::Infinity); // every paragraph's first line
    assert!(at(&every, "ee").0 == 6.0 && at(&every, "ff").0 == 6.0);
    let rtl = lay_fixed("אא בב", TextStyle { text_direction: TextDirection::Content, ..indent(10.0, 0.0) }, 42); // from the right
    assert_eq!(at(&rtl, "אא בב").0, 42.0 - 10.0 - 25.0);
    assert_eq!(lay("gg", indent(10.0, 0.0), Constraints::Infinity).size.width, 20); // its indent and its text
    // Cut where the indented line ends.
    let cut = lay_with(
        "hhhhhhhhhh",
        indent(10.0, 0.0),
        42,
        TextOverflow::Ellipsis,
        false,
        DefaultMaxLines,
        Constraints::Infinity,
    );
    assert_eq!(at(&cut, "hhhhh…").0, 10.0);
    // In sp: twice as far in at twice the font scale (the fake's text twice as big too, D12).
    let m = measurer();
    let scaled = m.measure_with(
        "gg",
        indent(10.0, 0.0),
        TextOverflow::Clip,
        true,
        DefaultMaxLines,
        Vec::new(),
        Constraints::default(),
        LayoutDirection::Ltr,
        Density::new(1.0, 2.0),
        fake(),
        false,
    );
    assert_eq!(at(&scaled, "gg").0, 20.0);
}

#[test]
fn line_break_strategies_pick_where_lines_break() {
    // C++'s test's text side: characters 5 px wide (size 10), a 30 px line holds 6, a 60 px one 12.
    let lines = |text: &str, width: i32, line_break: LineBreak| {
        texts(&lay(text, TextStyle { line_break, ..size(10.0) }, width))
    };
    // Simple (the default): each line as full as it goes, leaving "cc" alone.
    assert_eq!(lines("aaa bb cc ddddd", 30, LineBreak::Unspecified), ["aaa bb", "cc", "ddddd"]);
    assert_eq!(lines("aaa bbb ccc ddd e", 60, LineBreak::Simple), ["aaa bbb ccc", "ddd e"]);
    // Paragraph (HighQuality): the least uneven ends, the last line free.
    assert_eq!(lines("aaa bb cc ddddd", 30, LineBreak::Paragraph), ["aaa", "bb cc", "ddddd"]);
    assert_eq!(lines("aaa bbb ccc ddd e", 60, LineBreak::Paragraph), ["aaa bbb ccc", "ddd e"]);
    // Heading (Balanced): as even as it gets without another line, the last one too.
    assert_eq!(lines("aaa bb cc ddddd", 30, LineBreak::Heading), ["aaa", "bb cc", "ddddd"]);
    assert_eq!(lines("aaa bbb ccc ddd e", 60, LineBreak::Heading), ["aaa bbb", "ccc ddd e"]);
    // A word wider than the line: split, whatever the strategy.
    for b in [LineBreak::Heading, LineBreak::Paragraph] {
        let r = lay("aa bbbbbbbbbb c", TextStyle { line_break: b, ..size(10.0) }, 30);
        assert_eq!(r.line_count(), 3);
        assert_eq!((r.get_line_start(1), r.get_line_end(1, false), r.get_line_end(2, false)), (3, 9, 15));
    }
    // The first line's indent counts: 20 px of 40 leaves room for "aaaa" alone, not "aaaa bb".
    let indented = lay(
        "aaaa bb cccc",
        TextStyle {
            text_indent: Some(TextIndent::new(20.0.sp(), 0.0.sp())),
            line_break: LineBreak::Paragraph,
            ..size(10.0)
        },
        40,
    );
    assert!(indented.line_count() == 2 && indented.get_line_end(0, true) == 4);
    let balanced = lay("aaa bbb ccc ddd e", TextStyle { line_break: LineBreak::Heading, ..size(10.0) }, 60);
    assert!(balanced.line_count() == 2 && balanced.get_line_end(0, true) == 7);
}

#[test]
fn start_and_middle_ellipsis_and_baseline_shift() {
    // C++'s min_lines_start_and_middle_ellipsis_and_baseline_shift, its layout parts (min lines are BasicText's).
    // The start cut: its end kept (5 letters and the ellipsis fill 30).
    let start = lay_with("abcdefghij", size(10.0), 30, TextOverflow::StartEllipsis, true, 1, Constraints::Infinity);
    assert_eq!(texts(&start), ["…", "fghij"]);
    assert!(start.is_line_ellipsized(0) && at(&start, "fghij").0 == 5.0);
    // The middle cut (Android's: the end gets half of what is left first, the start the rest).
    let middle = lay_with(
        "klmnopqrst",
        size(10.0),
        30,
        TextOverflow::MiddleEllipsis,
        false,
        DefaultMaxLines,
        Constraints::Infinity,
    );
    assert_eq!(texts(&middle), ["klm", "…", "st"]);
    // More lines: no cut inside.
    let more = lay_with(
        "uvw xyz uvw",
        size(10.0),
        30,
        TextOverflow::MiddleEllipsis,
        true,
        DefaultMaxLines,
        Constraints::Infinity,
    );
    assert!(more.size.height == 38 && !texts(&more).iter().any(|t| t.contains('…')));
    // Superscript: half its ascent (4) up, the line taller above; subscript: 4 down, taller below.
    let shift = |b: BaselineShift| Some(SpanStyle { baseline_shift: Some(b), ..SpanStyle::Default });
    let formula = styled(&[
        (None, "x"),
        (shift(BaselineShift::Superscript), "2"),
        (None, "H"),
        (shift(BaselineShift::Subscript), "o"),
    ]);
    let r = lay(formula, size(10.0), Constraints::Infinity);
    assert!(at(&r, "x").1 == 12.0 && at(&r, "2").1 == 8.0 && at(&r, "H").1 == 12.0 && at(&r, "o").1 == 16.0);
    assert!(near(r.get_line_bottom(0), 12.0 + 4.5 + 4.0));
}

#[test]
fn styled_text_of_both_directions_is_ordered_as_one_line() {
    // C++'s test's geometry, 16 px: 8 px a character; paragraphs by their content, as C++'s.
    let b = |s: &str| -> (Option<SpanStyle>, String) {
        let (style, text) = bold(s);
        (Some(style), text)
    };
    let pieces =
        |p: &[(Option<SpanStyle>, String)]| styled(&p.iter().map(|(s, t)| (s.clone(), t.as_str())).collect::<Vec<_>>());
    // Left to right, a bold Hebrew word and a plain one: the two read right to left together.
    let ltr =
        lay(pieces(&[(None, "abc ".into()), b("אב"), (None, " גד".into())]), content(16.0), Constraints::Infinity);
    let d = drawn(&ltr);
    assert_eq!(
        d.iter().map(|d| (d.0.as_str(), d.1, d.3)).collect::<Vec<_>>(),
        [("abc ", 0.0, false), (" גד", 32.0, true), ("אב", 56.0, true)]
    );
    // Carets as Android's: where the Hebrew begins, the primary caret after the space before it (C++ put it at
    // the Hebrew's right edge, 72), the secondary there; after ב at its left; at the end, the line's right edge
    // (C++: the Hebrew's left, 32).
    assert!(near(ltr.get_cursor_rect(4).left, 32.0) && near(ltr.get_horizontal_position(4, false), 72.0));
    assert!(near(ltr.get_cursor_rect(8).left, 56.0) && near(ltr.get_cursor_rect(13).left, 72.0));
    assert!(near(ltr.get_horizontal_position(13, false), 32.0));
    // Selecting א highlights it where it is drawn: the right half of "אב".
    assert!(same_rect(ltr.get_path_for_range(4, 6).get_bounds(), 64.0, 0.0, 72.0, 20.0));
    assert_eq!(ltr.get_bidi_run_direction(5), ResolvedTextDirection::Rtl);
    assert_eq!(ltr.get_bidi_run_direction(1), ResolvedTextDirection::Ltr);
    assert_eq!(ltr.get_paragraph_direction(5), ResolvedTextDirection::Ltr);
    // Right to left, a bold Latin word and a plain one: left to right together, on the left (96 px of 200).
    let rtl = lay_fixed(pieces(&[(None, "שלום ".into()), b("abc"), (None, " def".into())]), content(16.0), 200);
    assert!(at(&rtl, "abc").0 == 104.0 && at(&rtl, " def").0 == 128.0 && at(&rtl, "שלום ").0 == 160.0);
    assert_eq!(rtl.get_paragraph_direction(0), ResolvedTextDirection::Rtl);
    // A bold piece of both directions is cut where they meet.
    let cut =
        lay(pieces(&[(None, "ab".into()), b("cd הו"), (None, " זח".into())]), content(16.0), Constraints::Infinity);
    assert!(at(&cut, "cd ").0 == 16.0 && at(&cut, " זח").0 == 40.0 && at(&cut, "הו").0 == 64.0);
    // The caret where ה begins: after the space (Android's primary; C++ 80); after ו at its left edge.
    assert!(near(cut.get_cursor_rect(5).left, 40.0) && near(cut.get_cursor_rect(9).left, 64.0));
    // " זח" goes right to left: its space on the right.
    assert!(same_rect(cut.get_path_for_range(9, 10).get_bounds(), 56.0, 0.0, 64.0, 20.0));
    // A right-to-left first line's indent stays on its right, at the left edge too.
    let indented = lay_fixed(
        pieces(&[(None, "עולם ".into()), b("xyz"), (None, " uvw".into())]),
        TextStyle {
            text_align: TextAlign::Left,
            text_indent: Some(TextIndent::new(16.0.sp(), 0.0.sp())),
            ..content(16.0)
        },
        200,
    );
    assert!(at(&indented, "xyz").0 == 0.0 && at(&indented, " uvw").0 == 24.0 && at(&indented, "עולם ").0 == 56.0);
}

#[test]
fn letter_spacing_widens_text_and_where_its_carets_are() {
    // C++'s letter_spacing_widens_text_and_where_its_carets_are, its measurer part: "abcd" at 10 px, 3 px apart
    // (half before each letter, half after). Compose's max intrinsic width adds half a pixel where letters are
    // spaced (so StaticLayout won't wrap it), so the measurer makes it 33 wide (C++: 32).
    let r = lay("abcd", TextStyle { letter_spacing: 3.0.sp(), ..size(10.0) }, Constraints::Infinity);
    assert_eq!(r.size.width, 33);
    assert!(near(r.get_cursor_rect(2).left, 16.0) && near(r.multi_paragraph.width(), 33.0));
    let spans =
        styled(&[(None, "a"), (Some(SpanStyle { letter_spacing: 4.0.sp(), ..SpanStyle::Default }), "b"), (None, "c")]);
    let r = lay(spans, size(10.0), Constraints::Infinity);
    assert!(near(r.multi_paragraph.intrinsics().max_intrinsic_width(), 19.5));
    // A glyph moves in by half the spacing.
    let glyphs = r.multi_paragraph.paint_ops(&TextPaintStyle::default());
    let TextPaintOp::Text { text, .. } = &glyphs[1] else { panic!("a text") };
    assert!(text.text.as_ref() == "b" && near(text.glyphs[0].glyphs[0].x, 2.0) && near(text.size.width, 9.0));
}

#[test]
fn carets_at_line_ends_and_after_a_trailing_line_feed() {
    let r = lay("ab\n", size(10.0), Constraints::Infinity);
    // The line feed ends the first line; an empty line follows, as tall as the text's.
    assert_eq!(r.line_count(), 2);
    assert_eq!((r.get_line_start(1), r.get_line_end(1, false)), (3, 3));
    assert!(same_rect(r.get_cursor_rect(3), 0.0, 12.5, 0.0, 25.0));
    assert!(same_rect(r.get_cursor_rect(2), 10.0, 0.0, 10.0, 12.5));
    assert_eq!(r.size.height, 25);
    // An empty text: one line, its caret at the start.
    let empty = lay("", size(10.0), Constraints::Infinity);
    assert!(empty.line_count() == 1 && empty.size == IntSize::new(0, 13));
    assert!(same_rect(empty.get_cursor_rect(0), 0.0, 0.0, 0.0, 12.5));
    // Right to left, centered or at the end: an empty line's caret where its text would start.
    let rtl = lay_fixed("אב\n", TextStyle { text_direction: TextDirection::Rtl, ..size(10.0) }, 100);
    assert!(near(rtl.get_cursor_rect(5).left, 100.0) && near(rtl.get_cursor_rect(0).left, 100.0));
    let centered = lay_fixed("ab\n", TextStyle { text_align: TextAlign::Center, ..size(10.0) }, 100);
    assert!(near(centered.get_cursor_rect(3).left, 50.0));
    // A wrapped line's end: the next line's start (downstream); the box of the space it hangs (upstream on its
    // line) collapses at the line's visible right, as Compose's TextLayout has it.
    let wrapped = lay("aa bb", size(10.0), 15);
    assert_eq!(wrapped.get_line_for_offset(3), 1);
    assert!(same_rect(wrapped.get_cursor_rect(3), 0.0, 12.5, 0.0, 25.0));
    assert!(same_rect(wrapped.get_bounding_box(2), 10.0, 0.0, 10.0, 12.5));
    assert!(same_rect(wrapped.get_bounding_box(1), 5.0, 0.0, 10.0, 12.5));
    // A line height applies to the empty last line too, its bottom trimmed, its top not (Compose's last line
    // metrics).
    let lh = lay("a\n", TextStyle { line_height: 30.0.sp(), ..size(10.0) }, Constraints::Infinity);
    assert!(near(lh.get_line_bottom(0), 18.8) && near(lh.get_line_bottom(1), 18.8 + 30.0 - 6.3));
}

#[test]
fn ellipsis_in_right_to_left_text() {
    // The end of a right-to-left line is its left: the ellipsis there, the line at the right.
    let r = lay_with("אבגדהוזחטי", content(10.0), 30, TextOverflow::Ellipsis, true, 1, Constraints::Infinity);
    assert!(r.is_line_ellipsized(0) && r.did_overflow_height());
    assert_eq!(texts(&r), ["אבגדה…"]);
    let d = drawn(&r);
    assert!(d[0].3 && near(d[0].1, 0.0));
    // Its glyphs in visual order: the ellipsis leftmost, then ה to א.
    let ops = r.multi_paragraph.paint_ops(&TextPaintStyle::default());
    let TextPaintOp::Text { text, .. } = &ops[0] else { panic!("a text") };
    let ids: Vec<u32> = text.glyphs.iter().flat_map(|g| g.glyphs.iter().map(|g| g.id)).collect();
    assert_eq!(ids, ['…' as u32, 'ה' as u32, 'ד' as u32, 'ג' as u32, 'ב' as u32, 'א' as u32]);
    // A start ellipsis in right-to-left text keeps its end, on the left; the ellipsis on the right.
    let start = lay_with("אבגדהוזחטי", content(10.0), 30, TextOverflow::StartEllipsis, true, 1, Constraints::Infinity);
    assert_eq!(texts(&start), ["וזחטי", "…"]);
    assert!(near(at(&start, "…").0, 25.0) && near(at(&start, "וזחטי").0, 0.0));
    // Ellipsized where the height ends (three lines of 12.5 in 30: two lines, the second cut).
    let tall = lay_with("aa bb cc dd ee", size(10.0), 30, TextOverflow::Ellipsis, true, DefaultMaxLines, 30);
    assert!(tall.line_count() == 2 && tall.is_line_ellipsized(1) && tall.did_overflow_height());
    assert_eq!(texts(&tall), ["aa bb", "cc dd…"]);
}

#[test]
fn placeholders_take_their_room() {
    // Inline content's room: 20 by 30 px above the baseline in "a␣b" (the placeholder over the middle character);
    // the line grows to hold it.
    let m = measurer();
    let p = |align: PlaceholderVerticalAlign| vec![Range::new(Placeholder::new(20.0.sp(), 30.0.sp(), align), 1, 2)];
    let lay_p = |placeholders: Vec<Range<Placeholder>>| {
        m.measure_with(
            "a b",
            size(10.0),
            TextOverflow::Clip,
            true,
            DefaultMaxLines,
            placeholders,
            Constraints::default(),
            LayoutDirection::Ltr,
            m.default_density(),
            fake(),
            false,
        )
    };
    let above = lay_p(p(PlaceholderVerticalAlign::AboveBaseline));
    assert!(near(above.get_line_baseline(0), 30.0) && above.size == IntSize::new(30, 35));
    assert!(same_rect(above.placeholder_rects()[0].unwrap(), 5.0, 0.0, 25.0, 30.0));
    assert_eq!(texts(&above), ["a", "b"]);
    assert_eq!(at(&above, "b").0, 25.0);
    let top = lay_p(p(PlaceholderVerticalAlign::Top));
    assert!(near(top.get_line_baseline(0), 8.0) && near(top.get_line_bottom(0), 30.0));
    assert!(same_rect(top.placeholder_rects()[0].unwrap(), 5.0, 0.0, 25.0, 30.0));
    let center = lay_p(p(PlaceholderVerticalAlign::TextCenter));
    let r = center.placeholder_rects()[0].unwrap();
    assert!(near((r.top + r.bottom) / 2.0, center.get_line_baseline(0) - 3.0) && near(r.height(), 30.0));
    // Em lengths are of the font size where it is; a placeholder isn't broken inside, and one cut off has no rect.
    let em = m.measure_with(
        "ab cd",
        size(10.0),
        TextOverflow::Clip,
        true,
        1,
        vec![Range::new(Placeholder::new(2.0.em(), 1.0.em(), PlaceholderVerticalAlign::Bottom), 3, 5)],
        Constraints::new(0, 30, 0, Constraints::Infinity),
        LayoutDirection::Ltr,
        m.default_density(),
        fake(),
        false,
    );
    assert!(em.line_count() == 1 && em.placeholder_rects()[0].is_none() && em.did_overflow_height());
}

#[test]
fn paragraph_styles_lay_their_pieces_out_one_under_another() {
    // A centered paragraph between two plain ones (C++'s paragraph_styles_give_parts_of_a_text_their_own_settings,
    // its layout): each its own lines, offsets and lines mapped across them.
    let text = AnnotatedString::with_styles(
        "sol orta sağ",
        Vec::new(),
        vec![Range::new(ParagraphStyle { text_align: TextAlign::Center, ..ParagraphStyle::Default }, 4, 9)],
    );
    let r = lay_fixed(text, size(10.0), 100);
    assert_eq!(r.line_count(), 3);
    assert_eq!(r.multi_paragraph.paragraph_info_list().len(), 3);
    assert_eq!(texts(&r), ["sol ", "orta ", "sağ"]);
    assert!(at(&r, "sol ").0 == 0.0 && near(at(&r, "orta ").0, 40.0) && at(&r, "sağ").0 == 0.0);
    assert!(near(at(&r, "orta ").1, 20.5) && near(at(&r, "sağ").1, 33.0));
    assert_eq!((r.get_line_start(1), r.get_line_end(1, false), r.get_line_start(2)), (4, 9, 9));
    assert_eq!(r.get_line_for_offset(10), 2);
    assert_eq!(r.get_offset_for_position(Offset::new(46.0, 15.0)), 5);
    assert!(same_rect(r.get_cursor_rect(10), 5.0, 25.0, 5.0, 37.5));
    assert!(same_rect(r.get_bounding_box(4), 40.0, 12.5, 45.0, 25.0));
    assert_eq!(r.get_word_boundary(6), TextRange::new(4, 8));
    // A paragraph of its own direction.
    let rtl = AnnotatedString::with_styles(
        "abc",
        Vec::new(),
        vec![Range::new(ParagraphStyle { text_direction: TextDirection::Rtl, ..ParagraphStyle::Default }, 0, 3)],
    );
    let r = lay_fixed(rtl, size(10.0), 100);
    assert!(at(&r, "abc").0 == 85.0 && r.get_paragraph_direction(0) == ResolvedTextDirection::Rtl);
    // max_lines over all of them: the pieces past it left out.
    let three = AnnotatedString::with_styles(
        "bir\niki\nüç",
        Vec::new(),
        vec![Range::new(ParagraphStyle { text_align: TextAlign::End, ..ParagraphStyle::Default }, 4, 8)],
    );
    let r = lay_with(three, size(10.0), 100, TextOverflow::Clip, true, 2, Constraints::Infinity);
    assert!(r.line_count() == 2 && r.did_overflow_height() && r.size.height == 25);
}

#[test]
fn intrinsic_widths_are_composes() {
    let intrinsics = |text: &str, style: TextStyle| {
        let s = style.resolve_defaults(LayoutDirection::Ltr);
        let i = MultiParagraphIntrinsics::new(&text.into(), &s, &[], Density::new(1.0, 1.0), &fake(), true);
        (i.min_intrinsic_width(), i.max_intrinsic_width())
    };
    // The widest of the ten longest pieces between line breaks, its trailing space with it; all of it unwrapped
    // (its widest line), rounded up.
    assert_eq!(intrinsics("kısa kelimeler burada", size(16.0)), (80.0, 168.0));
    assert_eq!(intrinsics("bir\niki üç", size(10.0)), (20.0, 30.0));
    // The first line's indent goes with every piece; letter spacing adds half a pixel to the max.
    let indent = TextStyle { text_indent: Some(TextIndent::new(10.0.sp(), 0.0.sp())), ..size(10.0) };
    assert_eq!(intrinsics("gg hh", indent), (25.0, 35.0));
    assert_eq!(intrinsics("ab", TextStyle { letter_spacing: 1.0.sp(), ..size(10.0) }), (12.0, 12.5));
    // Tabs reach their stops (every 20 px).
    assert_eq!(intrinsics("a\tb", size(10.0)).1, 25.0);
}

#[test]
fn range_for_rect_takes_characters_or_words() {
    let r = lay("ab cd ef", size(10.0), Constraints::Infinity);
    let rect = Rect::new(7.0, 0.0, 22.0, 10.0);
    let chars = r.get_range_for_rect(rect, TextGranularity::Character, &TextInclusionStrategy::ContainsCenter);
    assert_eq!(chars, TextRange::new(1, 4));
    let words = r.get_range_for_rect(rect, TextGranularity::Word, &TextInclusionStrategy::AnyOverlap);
    assert_eq!(words, TextRange::new(0, 5));
    let all = r.get_range_for_rect(
        Rect::new(0.0, 0.0, 40.0, 13.0),
        TextGranularity::Word,
        &TextInclusionStrategy::ContainsAll,
    );
    assert_eq!(all, TextRange::new(0, 8));
    assert_eq!(
        r.get_range_for_rect(
            Rect::new(0.0, 30.0, 40.0, 40.0),
            TextGranularity::Character,
            &TextInclusionStrategy::AnyOverlap
        ),
        TextRange::Zero
    );
}

struct Halves;

impl Hyphenator for Halves {
    fn hyphenate(&self, word: &str, _: &kompoz_text::Locale) -> Vec<(usize, HyphenationType)> {
        let half = word.char_indices().nth(word.chars().count() / 2).map_or(0, |(i, _)| i);
        vec![(half, HyphenationType::BreakAndInsertHyphen)]
    }
}

#[test]
fn hyphen_edits_are_measured_and_drawn() {
    // With a hyphenator installed, `Hyphens.Auto` breaks a word at its point: the line ends in a hyphen, drawn
    // in the face of the text before it.
    set_hyphenator(Some(Rc::new(Halves)));
    let style = TextStyle { hyphens: Hyphens::Auto, ..size(10.0) };
    let r = lay("aaaaaaaa", style, 30);
    set_hyphenator(None);
    assert_eq!(texts(&r), ["aaaa\u{2010}", "aaaa"]);
    assert!(near(r.get_line_right(0), 25.0));
    let _ = EndHyphenEdit::InsertHyphen;
}

#[test]
fn layouts_are_kept_and_their_draw_lists_too() {
    let m = measurer();
    let measure = |color: Color| {
        m.measure_with(
            "abc",
            TextStyle { color, ..size(10.0) },
            TextOverflow::Clip,
            true,
            DefaultMaxLines,
            Vec::new(),
            Constraints::default(),
            LayoutDirection::Ltr,
            m.default_density(),
            m.default_font_family_resolver().clone(),
            false,
        )
    };
    let a = measure(Color::Red);
    // Another color lays out alike: the same layout, its input the new one.
    let b = measure(Color::Blue);
    assert!(Rc::ptr_eq(&a.multi_paragraph, &b.multi_paragraph) && b.layout_input.style.color == Color::Blue);
    // Drawn twice, its glyph runs are the same.
    let first = a.multi_paragraph.paint_ops(&TextPaintStyle::default());
    let second = b.multi_paragraph.paint_ops(&TextPaintStyle::default());
    let (TextPaintOp::Text { text: x, .. }, TextPaintOp::Text { text: y, .. }) = (&first[0], &second[0]) else {
        panic!("texts")
    };
    assert!(Rc::ptr_eq(&x.glyphs, &y.glyphs) && x == y);
}

#[test]
fn font_padding_tabs_backgrounds_and_decorations() {
    // With the fonts' padding kept and no line height style, a line height is Android's LineHeightSpan: every
    // line that tall, its descent grown as much as the whole (4.5 of 12.5: 10.8 of 30).
    let padded = TextStyle {
        line_height: 30.0.sp(),
        platform_style: Some(PlatformTextStyle::with_include_font_padding(true)),
        ..size(10.0)
    };
    let r = lay("a\nb", padded, Constraints::Infinity);
    assert!(r.size.height == 60 && near(r.first_baseline(), 19.2) && near(r.get_line_top(1), 30.0));
    // A tab reaches the next stop (every 20 px).
    let tab = lay("a\tb", size(10.0), Constraints::Infinity);
    assert!(near(tab.get_horizontal_position(2, true), 20.0) && tab.size.width == 25);
    // A span's background behind its run, as tall as the line; an underline 2 px below the baseline, a line
    // through 0.3 ascents above it (the fake's), a pixel thick, as wide as the run.
    let decorated = styled(&[
        (None, "ab"),
        (
            Some(SpanStyle {
                background: Color::Red,
                text_decoration: Some(TextDecoration::combine(&[
                    TextDecoration::Underline,
                    TextDecoration::LineThrough,
                ])),
                ..SpanStyle::Default
            }),
            "cd",
        ),
    ]);
    let r = lay(decorated, size(10.0), Constraints::Infinity);
    let ops = r.multi_paragraph.paint_ops(&TextPaintStyle::default());
    let backgrounds: Vec<Rect> = ops
        .iter()
        .filter_map(|op| match op {
            TextPaintOp::Background { rect, color } if *color == Color::Red => Some(*rect),
            _ => None,
        })
        .collect();
    assert!(backgrounds.len() == 1 && same_rect(backgrounds[0], 10.0, 0.0, 20.0, 12.5));
    let lines: Vec<(Offset, Offset, f32)> = ops
        .iter()
        .filter_map(|op| match op {
            TextPaintOp::Line { from, to, thickness, .. } => Some((*from, *to, *thickness)),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 2);
    assert!(near(lines[0].0.y, 10.0) && near(lines[0].0.x, 10.5) && near(lines[0].1.x, 19.5) && near(lines[0].2, 1.0));
    assert!(near(lines[1].0.y, 8.0 - 2.4));
    // A draw call's decoration goes where the spans gave none: under "ab" too.
    let underlined = r
        .multi_paragraph
        .paint_ops(&TextPaintStyle { decoration: Some(TextDecoration::Underline), ..TextPaintStyle::default() });
    assert_eq!(underlined.iter().filter(|op| matches!(op, TextPaintOp::Line { .. })).count(), 3);
}

#[test]
fn skewed_text_ink_leans_as_androids() {
    // TextGeometricTransform(skewX = -0.25) as Android draws it (TextPaint.textSkewX, Skia's y down): what is above
    // the baseline leans right, what is below it left, a quarter of its height; 0.25 the other way.
    let ink = |skew_x: f32| {
        let style =
            TextStyle { text_geometric_transform: Some(TextGeometricTransform::new(1.0, skew_x)), ..size(10.0) };
        let r = lay("l", style, Constraints::Infinity);
        let ops = r.multi_paragraph.paint_ops(&TextPaintStyle::default());
        ops.iter()
            .find_map(|op| match op {
                TextPaintOp::Text { text, .. } => Some(text.ink_bounds),
                _ => None,
            })
            .expect("drawn")
    };
    let (plain, right, left) = (ink(0.0), ink(-0.25), ink(0.25));
    assert!(plain.top < 0.0 && plain.bottom > 0.0);
    assert!(near(right.right, plain.right - plain.top * 0.25) && near(right.left, plain.left - plain.bottom * 0.25));
    assert!(near(left.left, plain.left + plain.top * 0.25) && near(left.right, plain.right + plain.bottom * 0.25));
}
