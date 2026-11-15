//! Line breaking: UAX #14 opportunities (a selection of Unicode's LineBreakTest), strictness and phrases in
//! Japanese and Korean, Thai by icu's model, email addresses and URLs as minikin breaks them, and the strategies
//! over a measured fixture (the algorithm side of the C++ Kompoz's line_break_strategies_pick_where_lines_break,
//! the line side of justified_lines_stretch_at_their_spaces and the measuring side of
//! min_intrinsic_widths_are_the_widest_words): greedy and optimal lines, splitting words wider than a line,
//! hanging spaces, tabs, mandatory breaks, justification squeezing spaces, the hyphenation hook.

use kompoz_text::line_break::*;
use kompoz_text::style::Hyphens;
use kompoz_text::style::line_break::{Strategy, Strictness, WordBreak};
use kompoz_text::{LineBreak, Locale};
use std::ops::Range;

/// Text measured as the fake backend does: every code point half an em wide; spans of their own size.
struct Fixture<'a> {
    text: &'a str,
    size: f32,
    spans: Vec<(Range<usize>, f32)>,
}

impl<'a> Fixture<'a> {
    fn new(text: &'a str, size: f32) -> Fixture<'a> {
        Fixture { text, size, spans: Vec::new() }
    }
    fn size_at(&self, offset: usize) -> f32 {
        self.spans.iter().rev().find(|(r, _)| r.contains(&offset)).map_or(self.size, |s| s.1)
    }
}

impl LineMeasure for Fixture<'_> {
    fn width(&self, range: Range<usize>) -> f32 {
        self.text[range.clone()].char_indices().map(|(i, _)| self.size_at(range.start + i) * 0.5).sum()
    }
    fn width_with_hyphens(&self, range: Range<usize>, start: StartHyphenEdit, end: EndHyphenEdit) -> f32 {
        let extra = (start.text().chars().count() + end.text().chars().count()) as f32 * self.size * 0.5;
        let replaced = if end == EndHyphenEdit::ReplaceWithHyphen { self.size * 0.5 } else { 0.0 };
        self.width(range) + extra - replaced
    }
    fn font_size(&self, range: Range<usize>) -> f32 {
        self.text[range.clone()].char_indices().map(|(i, _)| self.size_at(range.start + i)).fold(self.size, f32::max)
    }
}

fn params<'a>(line_break: LineBreak, width: f32) -> LineBreakParams<'a> {
    LineBreakParams { line_break, locale: Locale::new("en"), ..LineBreakParams::new(LineWidths::uniform(width)) }
}

/// Each line's visible text.
fn visible<'a>(text: &'a str, lines: &[BrokenLine]) -> Vec<&'a str> {
    lines.iter().map(|l| &text[l.range.start..l.visible_end]).collect()
}

/// The lines of `text` 5 px a character (size 10) in `width`.
fn lines_of(text: &str, width: f32, line_break: LineBreak) -> Vec<&str> {
    visible(text, &break_lines(text, &Fixture::new(text, 10.0), &params(line_break, width)))
}

fn offsets(text: &str, line_break: LineBreak, locale: &str) -> Vec<usize> {
    break_opportunities(text, line_break, &Locale::new(locale)).iter().map(|p| p.offset).collect()
}

#[test]
fn line_break_strategies_pick_where_lines_break() {
    // Characters 5 px wide (size 10): a 30 px line holds 6, a 60 px one 12.
    // Simple (and Unspecified): each line as full as it goes, leaving "cc" alone.
    for strategy in [LineBreak::Simple, LineBreak::Unspecified] {
        assert_eq!(lines_of("aaa bb cc ddddd", 30.0, strategy), ["aaa bb", "cc", "ddddd"]);
        assert_eq!(lines_of("aaa bbb ccc ddd e", 60.0, strategy), ["aaa bbb ccc", "ddd e"]);
    }
    // Paragraph (HighQuality): the least uneven ends, the last line free.
    assert_eq!(lines_of("aaa bb cc ddddd", 30.0, LineBreak::Paragraph), ["aaa", "bb cc", "ddddd"]);
    assert_eq!(lines_of("aaa bbb ccc ddd e", 60.0, LineBreak::Paragraph), ["aaa bbb ccc", "ddd e"]);
    // Heading (Balanced): as even as it gets without another line, the last one too.
    assert_eq!(lines_of("aaa bb cc ddddd", 30.0, LineBreak::Heading), ["aaa", "bb cc", "ddddd"]);
    assert_eq!(lines_of("aaa bbb ccc ddd e", 60.0, LineBreak::Heading), ["aaa bbb", "ccc ddd e"]);
    // A word wider than the line: split at the longest piece that fits, whatever the strategy.
    for strategy in [LineBreak::Simple, LineBreak::Heading, LineBreak::Paragraph] {
        let text = "aa bbbbbbbbbb c";
        let lines = break_lines(text, &Fixture::new(text, 10.0), &params(strategy, 30.0));
        let ranges: Vec<Range<usize>> = lines.iter().map(|l| l.range.clone()).collect();
        assert_eq!(ranges, [0..3, 3..9, 9..15], "{strategy:?}");
    }
    // The first line's indent counts: 20 px of 40 leaves room for "aaaa" alone.
    let text = "aaaa bb cccc";
    let indented = LineBreakParams {
        line_break: LineBreak::Paragraph,
        locale: Locale::new("en"),
        ..LineBreakParams::new(LineWidths::indented(40.0, 20.0, 0.0))
    };
    let lines = break_lines(text, &Fixture::new(text, 10.0), &indented);
    assert_eq!(lines.len(), 2);
    assert_eq!((lines[0].range.clone(), lines[0].visible_end), (0..5, 4));
    let text = "aaa bbb ccc ddd e";
    let balanced = break_lines(text, &Fixture::new(text, 10.0), &params(LineBreak::Heading, 60.0));
    assert_eq!((balanced.len(), balanced[0].visible_end), (2, 7));
    assert!(
        LineBreak::Heading.strategy == Strategy::Balanced
            && LineBreak::Paragraph.strategy == Strategy::HighQuality
            && LineBreak::Simple.strategy == Strategy::Simple
            && LineBreak::Unspecified.strategy == Strategy::Unspecified
    );
}

#[test]
fn balanced_high_quality_and_simple_differ_on_a_measured_paragraph() {
    let strategies = |text: &str, width: f32| {
        let fixture = Fixture::new(text, 10.0);
        [LineBreak::Simple, LineBreak::Paragraph, LineBreak::Heading]
            .map(|line_break| break_lines(text, &fixture, &params(line_break, width)))
    };
    // In 40 px Simple leaves "cc" alone; HighQuality and Balanced even the first two lines out.
    let text = "aaaa bbb cc dddddd eeee f ggggg hh";
    let [simple, high, balanced] = strategies(text, 40.0);
    assert_eq!(visible(text, &simple), ["aaaa bbb", "cc", "dddddd", "eeee f", "ggggg hh"]);
    assert_eq!(visible(text, &high), ["aaaa", "bbb cc", "dddddd", "eeee f", "ggggg hh"]);
    assert_eq!(visible(text, &balanced), visible(text, &high));
    // In 70 px only Balanced moves "f" down to the short last line.
    let [simple, high, balanced] = strategies(text, 70.0);
    assert_eq!(visible(text, &simple), ["aaaa bbb cc", "dddddd eeee f", "ggggg hh"]);
    assert_eq!(visible(text, &high), visible(text, &simple));
    assert_eq!(visible(text, &balanced), ["aaaa bbb cc", "dddddd eeee", "f ggggg hh"]);
    let text = "Kırk yıl kâhır ol, bir gün şâkir ol";
    let [simple, high, balanced] = strategies(text, 70.0);
    assert_eq!(visible(text, &simple), ["Kırk yıl kâhır", "ol, bir gün", "şâkir ol"]);
    assert_eq!(visible(text, &high), visible(text, &simple));
    assert_eq!(visible(text, &balanced), ["Kırk yıl", "kâhır ol, bir", "gün şâkir ol"]);
    // Every strategy needs as many lines, none overflows, and each leaves the least room it scores.
    let slack = |lines: &[BrokenLine], last: bool| {
        let n = if last { lines.len() } else { lines.len() - 1 };
        lines[..n].iter().map(|l| (70.0 - l.width).powi(2)).sum::<f32>()
    };
    for lines in [&simple, &high, &balanced] {
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|l| l.width <= 70.0));
        assert_eq!(lines.last().unwrap().range.end, text.len());
    }
    assert!(slack(&high, false) <= slack(&simple, false));
    assert!(slack(&balanced, true) < slack(&simple, true) && slack(&balanced, true) < slack(&high, true));
}

#[test]
fn justified_lines_break_at_their_spaces_which_hang_at_the_end() {
    // 5 px a letter: "aa bb cc" (40) fills the first line, 2 px short of 42; its two spaces stretch.
    let text = "aa bb cc dd ee";
    let lines = break_lines(text, &Fixture::new(text, 10.0), &params(LineBreak::Simple, 42.0));
    assert_eq!(visible(text, &lines), ["aa bb cc", "dd ee"]);
    assert_eq!((lines[0].range.clone(), lines[0].width), (0..9, 40.0));
    assert_eq!(stretchable_spaces(text, 0, 9), [2, 5]);
    assert!(!lines[0].hard_break && !lines[1].hard_break);
    // Right to left the same, in bytes.
    let hebrew = "אא בב גג דד הה";
    let lines = break_lines(hebrew, &Fixture::new(hebrew, 10.0), &params(LineBreak::Simple, 42.0));
    assert_eq!(visible(hebrew, &lines), ["אא בב גג", "דד הה"]);
    assert_eq!(lines[0].range, 0..15);
    // Spaces ending a line hang past it: both after "ww", and only the gap between the words stretches.
    let text = "qq ww  ee rr";
    let lines = break_lines(text, &Fixture::new(text, 10.0), &params(LineBreak::Simple, 42.0));
    assert_eq!((lines[0].range.clone(), lines[0].visible_end, lines[0].width), (0..7, 5, 25.0));
    assert_eq!(stretchable_spaces(text, 0, 7), [2]);
    // Not wrapped (an infinite width): one line.
    let text = "jj kk ll mm";
    assert_eq!(lines_of(text, f32::INFINITY, LineBreak::Paragraph), ["jj kk ll mm"]);
    assert_eq!(lines_of(text, f32::INFINITY, LineBreak::Simple), ["jj kk ll mm"]);
    // Spaces never overflow a line, however many.
    assert_eq!(lines_of("aaaaaa      b", 30.0, LineBreak::Simple), ["aaaaaa", "b"]);
}

#[test]
fn justified_text_may_squeeze_its_spaces_by_a_third() {
    // 70 px of text with two spaces in 68: a justified line may give up 2 px of its spaces' 10/3 px.
    let text = "aaaa bbbb cccc dd";
    let mut justified = params(LineBreak::Paragraph, 68.0);
    justified.justified = true;
    let lines = break_lines(text, &Fixture::new(text, 10.0), &justified);
    assert_eq!(visible(text, &lines), ["aaaa bbbb cccc", "dd"]);
    assert_eq!(lines[0].width, 70.0);
    // Not justified, the line can't overflow.
    assert_eq!(lines_of(text, 68.0, LineBreak::Paragraph), ["aaaa bbbb", "cccc dd"]);
    // Nor can more than a third of a space go.
    let mut tight = params(LineBreak::Paragraph, 66.0);
    tight.justified = true;
    assert_eq!(visible(text, &break_lines(text, &Fixture::new(text, 10.0), &tight)), ["aaaa bbbb", "cccc dd"]);
}

#[test]
fn mandatory_breaks_end_lines() {
    let lines_of = |text: &str| {
        let lines = break_lines(text, &Fixture::new(text, 10.0), &params(LineBreak::Paragraph, 100.0));
        lines.iter().map(|l| (l.range.clone(), l.visible_end, l.hard_break)).collect::<Vec<_>>()
    };
    assert_eq!(lines_of("ab\ncd"), [(0..3, 2, true), (3..5, 5, false)]);
    // A text ending in a line break has an empty line after it.
    assert_eq!(lines_of("ab\n"), [(0..3, 2, true), (3..3, 3, false)]);
    assert_eq!(lines_of(""), [(0..0, 0, false)]);
    assert_eq!(lines_of("a\n\nb"), [(0..2, 1, true), (2..3, 2, true), (3..4, 4, false)]);
    // Carriage return and line feed together, the line separator, the next line, a form feed.
    assert_eq!(lines_of("a\r\nb"), [(0..3, 1, true), (3..4, 4, false)]);
    assert_eq!(lines_of("a\u{2028}b"), [(0..4, 1, true), (4..5, 5, false)]);
    assert_eq!(lines_of("a\u{0085}b\u{000C}c"), [(0..3, 1, true), (3..5, 4, true), (5..6, 6, false)]);
    let mandatory: Vec<bool> =
        break_opportunities("a b\nc", LineBreak::Simple, &Locale::new("en")).iter().map(|p| p.mandatory).collect();
    assert_eq!(mandatory, [false, true, false]);
    // Lines between mandatory breaks wrap on their own, line widths counting on.
    let text = "aaa bbb\nccc ddd";
    let params = LineBreakParams {
        line_break: LineBreak::Simple,
        locale: Locale::new("en"),
        ..LineBreakParams::new(LineWidths::new([20.0, 40.0, 20.0], 40.0))
    };
    assert_eq!(visible(text, &break_lines(text, &Fixture::new(text, 10.0), &params)), ["aaa", "bbb", "ccc", "ddd"]);
}

#[test]
fn tabs_go_to_their_stops_and_break_greedily() {
    // "aa" ends at 10 px, the tab takes "bb" to 20; " cc" would end at 45 of 40.
    let text = "aa\tbb cc";
    for strategy in [LineBreak::Simple, LineBreak::Paragraph, LineBreak::Heading] {
        let lines = break_lines(text, &Fixture::new(text, 10.0), &params(strategy, 40.0));
        assert_eq!(visible(text, &lines), ["aa\tbb", "cc"]);
        assert_eq!(lines[0].width, 30.0);
    }
    let stops = TabStops::default();
    assert_eq!((stops.next_tab(0.0), stops.next_tab(10.0), stops.next_tab(20.0)), (20.0, 20.0, 40.0));
    let own = TabStops::new([15.0, 50.0], 30.0);
    assert_eq!((own.next_tab(10.0), own.next_tab(15.0), own.next_tab(50.0)), (15.0, 50.0, 60.0));
    let mut custom = params(LineBreak::Simple, 100.0);
    custom.tab_stops = own;
    let lines = break_lines("a\tb\tc", &Fixture::new("a\tb\tc", 10.0), &custom);
    assert_eq!(lines[0].width, 55.0); // a, a stop at 15, b to 20, the stop at 50, c
}

#[test]
fn words_wider_than_a_line_split_between_grapheme_clusters() {
    // Families of five code points each, 25 px: whole, two in 60 px.
    let family = "👨\u{200D}👩\u{200D}👧";
    let text = format!("{family}{family}{family}{family}{family}");
    let lines = break_lines(&text, &Fixture::new(&text, 10.0), &params(LineBreak::Simple, 60.0));
    let ranges: Vec<Range<usize>> = lines.iter().map(|l| l.range.clone()).collect();
    let f = family.len();
    assert_eq!(ranges, [0..2 * f, 2 * f..4 * f, 4 * f..5 * f]);
    // A cluster wider than the line still takes one.
    let lines = break_lines(&text, &Fixture::new(&text, 10.0), &params(LineBreak::Paragraph, 10.0));
    assert_eq!(lines.len(), 5);
    assert!(lines.iter().all(|l| l.range.len() == f));
    // An accent stays with its letter, a skin tone with its hand.
    let text = "e\u{0301}e\u{0301}e\u{0301} 👍🏽👍🏽";
    let lines = break_lines(text, &Fixture::new(text, 10.0), &params(LineBreak::Simple, 10.0));
    assert_eq!(visible(text, &lines), ["e\u{0301}", "e\u{0301}", "e\u{0301}", "👍🏽", "👍🏽"]);
    // Emoji in lines break as words do.
    assert_eq!(offsets("a 👍🏽 b 👨\u{200D}👩\u{200D}👧 c", LineBreak::Simple, "en"), [2, 11, 13, 32, 33]);
}

#[test]
fn turkish_arabic_and_hebrew_break_at_their_spaces_in_bytes() {
    // 8 px a character (size 16): "kelimeler" (72) is the widest word.
    let text = "kısa kelimeler burada";
    let lines = break_lines(text, &Fixture::new(text, 16.0), &params(LineBreak::Simple, 72.0));
    assert_eq!(visible(text, &lines), ["kısa", "kelimeler", "burada"]);
    assert_eq!(lines.iter().map(|l| l.range.start).collect::<Vec<_>>(), [0, 6, 16]);
    assert_eq!(lines_of("İstanbul'da ılık bir gün", 60.0, LineBreak::Simple), ["İstanbul'da", "ılık bir gün"]);
    assert_eq!(lines_of("مرحبا بالعالم الجميل", 50.0, LineBreak::Simple), ["مرحبا", "بالعالم", "الجميل"]);
    assert_eq!(lines_of("שלום 123 עולם", 45.0, LineBreak::Simple), ["שלום 123", "עולם"]);
    // Hyphens break after them; a soft hyphen doesn't (hyphenation's business).
    assert_eq!(offsets("state-of-the-art", LineBreak::Simple, "en"), [6, 9, 13, 16]);
    assert_eq!(offsets("soft\u{00AD}hyphen", LineBreak::Simple, "en"), [12]);
}

#[test]
fn thai_breaks_between_its_words_by_icus_model() {
    assert_eq!(offsets("ทุกสองสัปดาห์", LineBreak::Simple, "th"), [9, 18, 39]);
    assert_eq!(offsets("ภาษาไทยง่ายนิดเดียว", LineBreak::Simple, "th"), [12, 21, 33, 57]);
    let text = "ภาษาไทยง่ายนิดเดียว";
    // 8 code points a line: "นิดเดียว" just fits.
    assert_eq!(lines_of(text, 40.0, LineBreak::Simple), ["ภาษาไทย", "ง่าย", "นิดเดียว"]);
}

#[test]
fn japanese_breaks_by_strictness_and_phrase() {
    let text = "ちょっと待ってください。";
    let with = |strictness, word_break| LineBreak::new(Strategy::Simple, strictness, word_break);
    // Strict keeps small kana off a line's start; normal (Japanese's default) and loose allow it.
    assert_eq!(offsets(text, with(Strictness::Strict, WordBreak::Default), "ja"), [9, 12, 18, 21, 24, 27, 30, 36]);
    assert_eq!(
        offsets(text, with(Strictness::Normal, WordBreak::Default), "ja"),
        [3, 6, 9, 12, 15, 18, 21, 24, 27, 30, 36]
    );
    assert_eq!(
        offsets(text, with(Strictness::Default, WordBreak::Default), "ja"),
        offsets(text, with(Strictness::Normal, WordBreak::Default), "ja")
    );
    assert_eq!(
        offsets(text, with(Strictness::Default, WordBreak::Default), "en"),
        offsets(text, with(Strictness::Strict, WordBreak::Default), "ja")
    );
    // Phrases: particles and endings stay with their word.
    let phrase = with(Strictness::Default, WordBreak::Phrase);
    assert_eq!(offsets(text, phrase, "ja"), [12, 36]);
    assert_eq!(offsets("私は東京に行きます", phrase, "ja"), [6, 15, 27]);
    assert_eq!(offsets("こんにちは世界", phrase, "ja"), [15, 21]);
    assert_eq!(offsets("コンピューターを使う", phrase, "ja"), [24, 30]);
    // Lines break between phrases: "私は東京に" in 25 px, "行きます" after it.
    let text = "私は東京に行きます";
    let mut ja = params(LineBreak::Heading, 25.0);
    ja.locale = Locale::new("ja");
    assert_eq!(visible(text, &break_lines(text, &Fixture::new(text, 10.0), &ja)), ["私は東京に", "行きます"]);
    // A phrase wider than a line breaks where words would before splitting characters.
    let text = "東京都庁舎展望室";
    let mut narrow = params(LineBreak::new(Strategy::Simple, Strictness::Default, WordBreak::Phrase), 15.0);
    narrow.locale = Locale::new("ja");
    let lines = break_lines(text, &Fixture::new(text, 10.0), &narrow);
    assert!(lines.iter().all(|l| l.width <= 15.0));
    assert_eq!(lines.iter().map(|l| l.range.len()).sum::<usize>(), text.len());
}

#[test]
fn korean_phrases_keep_words_whole() {
    let text = "한국어 텍스트입니다";
    assert_eq!(offsets(text, LineBreak::Simple, "ko"), [3, 6, 10, 13, 16, 19, 22, 25, 28]);
    assert_eq!(offsets(text, LineBreak::Heading, "ko"), [10, 28]);
    // Phrase breaking is for Japanese and Korean only.
    assert_eq!(offsets(text, LineBreak::Heading, "en"), offsets(text, LineBreak::Simple, "ko"));
}

#[test]
fn email_addresses_and_urls_break_as_minikin_breaks_them() {
    let text = "see https://example.com/a/b now";
    let points = break_opportunities(text, LineBreak::Simple, &Locale::new("en"));
    let inside: Vec<usize> = points.iter().filter(|p| p.inside_email_or_url).map(|p| p.offset).collect();
    // After "https:" and "//", before ".com", "/a" and "/b".
    assert_eq!(inside, [10, 12, 19, 23, 25]);
    assert!(points.iter().find(|p| p.offset == 28).is_some_and(|p| p.email_or_url && !p.inside_email_or_url));
    assert_eq!(offsets("mail me@example.com ok", LineBreak::Simple, "en"), [5, 15, 20, 22]);
    // Greedily, the URL goes to its own line rather than breaking inside; there, it breaks where it must.
    assert_eq!(lines_of(text, 60.0, LineBreak::Simple), ["see", "https://", "example.com", "/a/b now"]);
    assert_eq!(lines_of("see me@example.com", 100.0, LineBreak::Simple), ["see me@example.com"]);
    assert_eq!(lines_of("see me@example.com", 60.0, LineBreak::Simple), ["see", "me@example", ".com"]);
}

#[test]
fn inline_content_is_never_broken_inside() {
    let text = "ab \u{FFFC}\u{FFFC} cd \u{FFFC} ef";
    let placeholder = 3..9;
    let mut p = params(LineBreak::Simple, 20.0);
    p.unbreakable = std::slice::from_ref(&placeholder);
    let lines = break_lines(text, &Fixture::new(text, 10.0), &p);
    assert!(lines.iter().all(|l| l.range.start <= 3 || l.range.start >= 9));
    let unbroken = params(LineBreak::Simple, 20.0);
    assert!(break_lines(text, &Fixture::new(text, 10.0), &unbroken).iter().any(|l| l.range.start == 6));
}

/// Hyphenates at soft hyphens, and inside "hyphenation" as a pattern would.
struct Patterns;

impl Hyphenator for Patterns {
    fn hyphenate(&self, word: &str, _: &Locale) -> Vec<(usize, HyphenationType)> {
        if word == "hyphenation" {
            return vec![(2, HyphenationType::BreakAndInsertHyphen), (6, HyphenationType::BreakAndInsertHyphen)];
        }
        word.match_indices('\u{00AD}').map(|(i, _)| (i + 2, HyphenationType::BreakAndInsertHyphen)).collect()
    }
}

#[test]
fn hyphens_auto_hyphenates_with_a_hyphenator_and_nothing_else() {
    let text = "a hyphenation";
    let mut p = params(LineBreak::Simple, 60.0);
    // No hyphenation, or no hyphenator: the word moves to the next line.
    assert_eq!(visible(text, &break_lines(text, &Fixture::new(text, 10.0), &p)), ["a", "hyphenation"]);
    p.hyphens = Hyphens::Auto;
    assert_eq!(visible(text, &break_lines(text, &Fixture::new(text, 10.0), &p)), ["a", "hyphenation"]);
    p.hyphenator = Some(&Patterns);
    let lines = break_lines(text, &Fixture::new(text, 10.0), &p);
    // "a hyphen‐" is 45 px with its hyphen.
    assert_eq!(visible(text, &lines), ["a hyphen", "ation"]);
    assert_eq!((lines[0].end_hyphen, lines[0].width), (EndHyphenEdit::InsertHyphen, 45.0));
    assert_eq!(lines[1].start_hyphen, StartHyphenEdit::NoEdit);
    p.hyphens = Hyphens::None;
    assert_eq!(visible(text, &break_lines(text, &Fixture::new(text, 10.0), &p)), ["a", "hyphenation"]);
    // The optimal breakers weigh a hyphen against an uneven line.
    p.hyphens = Hyphens::Auto;
    p.line_break = LineBreak::Paragraph;
    let lines = break_lines(text, &Fixture::new(text, 10.0), &p);
    assert_eq!(visible(text, &lines), ["a hyphen", "ation"]);
    // A soft hyphen breaks only when hyphenating.
    let text = "un\u{00AD}believable";
    p.line_break = LineBreak::Simple;
    p.widths = LineWidths::uniform(40.0);
    let lines = break_lines(text, &Fixture::new(text, 10.0), &p);
    assert_eq!(visible(text, &lines)[0], "un\u{00AD}");
    assert_eq!(lines[0].end_hyphen, EndHyphenEdit::InsertHyphen);
    assert_eq!(HyphenationType::BreakAndInsertHyphenAtNextLine.start_edit(), StartHyphenEdit::InsertHyphen);
    assert_eq!(HyphenationType::BreakAndInsertHyphenAtNextLine.end_edit(), EndHyphenEdit::NoEdit);
    assert_eq!(EndHyphenEdit::InsertMaqaf.text(), "\u{05BE}");
}

#[test]
fn min_intrinsic_width_is_the_widest_unbreakable_piece() {
    // 8 px a character: "kelimeler".
    let text = "kısa kelimeler burada";
    let en = Locale::new("en");
    assert_eq!(min_intrinsic_width(text, &Fixture::new(text, 16.0), LineBreak::Simple, &en, &[]), 72.0);
    // A word in a bigger size: its own width.
    let text = "ince KALIN ince";
    let mut styled = Fixture::new(text, 16.0);
    styled.spans.push((5..10, 32.0));
    assert_eq!(min_intrinsic_width(text, &styled, LineBreak::Simple, &en, &[]), 80.0);
    assert_eq!(styled.font_size(0..text.len()), 32.0);
    // Unbreakable ranges join pieces.
    let text = "ab cd ef";
    let joined = 1..4;
    let unbreakable = std::slice::from_ref(&joined);
    assert_eq!(min_intrinsic_width(text, &Fixture::new(text, 10.0), LineBreak::Simple, &en, unbreakable), 25.0);
}

#[test]
fn visible_ends_leave_hanging_spaces_and_line_breaks_out() {
    assert_eq!(visible_end("ab  \n", 0, 5), 2);
    assert_eq!(visible_end("ab\u{3000}", 0, 5), 2);
    assert_eq!(visible_end("ab\u{00A0}", 0, 4), 4); // no-break space stays
    assert_eq!(visible_end("ab\t", 0, 3), 3); // a tab stays
    assert_eq!(visible_end("   ", 1, 3), 1);
    assert!(is_line_end_space('\u{2002}') && !is_line_end_space('\u{2007}') && is_stretchable_space(' '));
    let widths = LineWidths::new([10.0, 30.0], 20.0);
    assert_eq!((widths.at(0), widths.at(1), widths.at(5), widths.min()), (10.0, 30.0, 20.0, 10.0));
}

/// The breaks Unicode's LineBreakTest gives, for a selection of its lines (tests/data/LineBreakTest.txt): strict
/// breaking, UAX #14's default (Compose's LineBreak.Simple is normal: small kana may start a line).
#[test]
fn line_break_test_cases_break_as_unicode_says() {
    let data = include_str!("data/LineBreakTest.txt");
    let strict = LineBreak::new(Strategy::Simple, Strictness::Strict, WordBreak::Default);
    let mut cases = 0;
    for line in data.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let spec = line.split('#').next().unwrap();
        let mut text = String::new();
        let mut breaks = Vec::new();
        for token in spec.split_whitespace() {
            match token {
                "÷" if !text.is_empty() => breaks.push(text.len()),
                "÷" | "×" => {}
                hex => text.push(char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap()),
            }
        }
        assert_eq!(offsets(&text, strict, "en"), breaks, "{line}");
        cases += 1;
    }
    assert!(cases > 300);
}
