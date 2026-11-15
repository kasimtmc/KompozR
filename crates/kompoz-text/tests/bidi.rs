//! Bidirectional text: paragraph directions as Compose resolves them on Android, UAX #9 levels (explicit
//! embeddings, overrides, isolates, numbers, bracket pairs) in the manner of Unicode's BidiCharacterTest, lines in
//! visual order, and what carets need (the C++ Kompoz's bidi_orders_runs_as_uax9_does,
//! bidi_bracket_pairs_go_with_what_they_hold and bidi_isolates_keep_their_direction_inside_and_stay_out_of_the_way,
//! on the full algorithm).

use kompoz_text::bidi::*;
use kompoz_text::style::{ResolvedTextDirection, TextDirection};
use kompoz_text::{Locale, LocaleList};

use ResolvedTextDirection::{Ltr, Rtl};

/// A line's runs left to right, each its text and `<|` for right to left or `>|` (the C++ test's notation).
fn order(text: &str, rtl: bool) -> String {
    let paragraph = BidiParagraph::with_direction(text, if rtl { Rtl } else { Ltr });
    let line = paragraph.line(text, 0..text.len());
    line.runs().iter().map(|r| format!("{}{}", &text[r.range.clone()], if r.is_rtl() { "<|" } else { ">|" })).collect()
}

/// A line's runs left to right, `<` before a right-to-left one's text.
fn runs(text: &str, rtl: bool) -> Vec<String> {
    let paragraph = BidiParagraph::with_direction(text, if rtl { Rtl } else { Ltr });
    let line = paragraph.line(text, 0..text.len());
    line.runs().iter().map(|r| format!("{}{}", if r.is_rtl() { "<" } else { "" }, &text[r.range.clone()])).collect()
}

/// The characters' levels (the levels of explicit embedding controls, which rule X9 removes, as `None`) and the
/// characters left to right on screen as indices (removed ones left out): BidiCharacterTest's two columns.
fn resolved(text: &str, direction: ResolvedTextDirection) -> (Vec<Option<u8>>, Vec<usize>) {
    let removed = |c: char| matches!(c, '\u{202A}'..='\u{202E}' | '\u{200B}' | '\u{FEFF}');
    let paragraph = BidiParagraph::with_direction(text, direction);
    let line = paragraph.line(text, 0..text.len());
    let starts: Vec<(usize, char)> = text.char_indices().collect();
    let levels = starts.iter().map(|&(i, c)| (!removed(c)).then(|| line.level_at(i))).collect();
    let index_of = |offset: usize| starts.iter().position(|&(i, _)| i == offset).unwrap();
    let mut visual = Vec::new();
    for run in line.runs() {
        let mut chars: Vec<usize> =
            text[run.range.clone()].char_indices().map(|(i, _)| index_of(run.range.start + i)).collect();
        if run.is_rtl() {
            chars.reverse();
        }
        visual.extend(chars.into_iter().filter(|&k| !removed(starts[k].1)));
    }
    (levels, visual)
}

#[test]
fn bidi_orders_runs_as_uax9_does() {
    assert_eq!(order("abc def", false), "abc def>|");
    assert_eq!(order("abc אב גד xyz", false), "abc >|אב גד<| xyz>|"); // the space between Hebrew words is Hebrew
    assert_eq!(order("אב abc", true), "abc>|אב <|"); // a right-to-left paragraph: Latin on the left
    assert_eq!(order("مرحبا 123", false), "123>|مرحبا <|"); // numbers after Arabic go with it
    assert_eq!(order("שלום 123", false), "123>|שלום <|");
    assert_eq!(order("abc 123", true), "abc 123>|");
    assert_eq!(order("אב ", false), "אב<| >|"); // a space ending the line takes the paragraph's direction
    assert_eq!(order("", true), "<|");
    assert_eq!(first_strong_direction("123 שלום"), Some(Rtl));
    assert_eq!(first_strong_direction("123 abc"), Some(Ltr));
    assert_eq!(first_strong_direction("123"), None);
    assert_eq!(resolve_paragraph_direction("…", TextDirection::ContentOrRtl, &LocaleList::empty()), Rtl);
}

#[test]
fn bidi_bracket_pairs_go_with_what_they_hold() {
    // Hebrew in brackets after Hebrew: the closing bracket stays in the Hebrew run.
    assert_eq!(order("אב(גד) x", false), "אב(גד)<| x>|");
    assert_eq!(order("a(b)א", true), "א<|a(b)>|"); // Latin in brackets after Latin, in Hebrew
    // Text of the level's direction inside: the pair takes it, Hebrew around or not.
    assert_eq!(order("א(ב c)ג", false), "א<|(>|ב<| c)>|ג<|");
    assert_eq!(order("x (אב) y", false), "x (>|אב<|) y>|"); // only Hebrew inside, but Latin before
    // A closing bracket closes the nearest open one of its kind and what is open inside it; one with nothing to
    // close is a neutral.
    assert_eq!(order("אב(גד[הו)ז] x", false), "אב(גד[הו)ז<|] x>|");
    assert_eq!(order("אב(12) x", false), ")<|12>|אב(<| x>|"); // numbers inside count as right to left
    assert_eq!(order("אב() x", false), "אב<|() x>|"); // nothing strong inside: neutrals
    // Outer pairs first: the inner one sees the outer bracket (left to right) before it, not the Hebrew.
    assert_eq!(order("א([ג] b)", false), "א<|([>|ג<|] b)>|");
    assert_eq!(order("אב\u{2329}גד\u{3009} x", false), "אב\u{2329}גד\u{3009}<| x>|"); // U+2329 is U+3008
}

#[test]
fn bidi_isolates_keep_their_direction_inside_and_stay_out_of_the_way() {
    let ahmet = "\u{0623}\u{062D}\u{0645}\u{062F}"; // أحمد
    let ali = "\u{0639}\u{0644}\u{064A}"; // علي
    let (rli, lri, fsi, pdi) = ("\u{2067}", "\u{2066}", "\u{2068}", "\u{2069}");
    let v = |parts: &[&str]| parts.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    // Without an isolate the number after the Arabic name goes with it (an Arabic number, shown to its left).
    assert_eq!(runs(&format!("Yorum {ahmet} 3 kez"), false), v(&["Yorum ", "3", &format!("<{ahmet} "), " kez"]));
    // Isolated, the name is right to left on its own and the number stays in the sentence.
    assert_eq!(
        runs(&format!("Yorum {rli}{ahmet}{pdi} 3 kez"), false),
        v(&[&format!("Yorum {rli}"), &format!("<{ahmet}"), &format!("{pdi} 3 kez")])
    );
    // FSI takes the direction of what it holds.
    assert_eq!(
        runs(&format!("a {fsi}{ahmet}{pdi} b"), false),
        v(&[&format!("a {fsi}"), &format!("<{ahmet}"), &format!("{pdi} b")])
    );
    assert_eq!(
        runs(&format!("{ahmet} {fsi}abc 12{pdi} {ahmet}"), true),
        v(&[&format!("<{pdi} {ahmet}"), "abc 12", &format!("<{ahmet} {fsi}")])
    );
    // LRI keeps what it holds left to right even when it opens with a neutral.
    assert_eq!(
        runs(&format!("{ahmet} {lri}-1 (x){pdi}"), true),
        v(&[&format!("<{pdi}"), "-1 (x)", &format!("<{ahmet} {lri}")])
    );
    // An initiator without a PDI isolates to the end; isolates nest.
    assert_eq!(runs(&format!("a {rli}{ahmet} 5"), false), v(&[&format!("a {rli}"), "5", &format!("<{ahmet} ")]));
    assert_eq!(
        runs(&format!("x {rli}{ahmet} {lri}ok{pdi}{pdi} y"), false),
        v(&[&format!("x {rli}"), &format!("<{pdi}"), "ok", &format!("<{ahmet} {lri}"), &format!("{pdi} y")])
    );
    // FSI round two Arabic words and a number: right to left as a whole.
    assert_eq!(
        runs(&format!("a {fsi}{ahmet} 12 {ali}{pdi} b"), false),
        v(&[&format!("a {fsi}"), &format!("< {ali}"), "12", &format!("<{ahmet} "), &format!("{pdi} b")])
    );
    // ...and one opening with Arabic and going on in Latin: the Latin shown first (to the left).
    assert_eq!(
        runs(&format!("a {fsi}{ahmet} abc{pdi} b"), false),
        v(&[&format!("a {fsi}"), "abc", &format!("<{ahmet} "), &format!("{pdi} b")])
    );
    // LRI in a left-to-right paragraph: left to right inside, the Arabic word on its own.
    assert_eq!(
        runs(&format!("a {lri}b {ahmet} c{pdi} d"), false),
        v(&[&format!("a {lri}"), "b ", &format!("<{ahmet}"), " c", &format!("{pdi} d")])
    );
    // A space and an isolate's mark ending the line go back to the paragraph's level.
    assert_eq!(
        runs(&format!("a {rli}{ahmet} {lri}"), false),
        v(&[&format!("a {rli}"), &format!("<{ahmet}"), &format!(" {lri}")])
    );
    // What an isolate holds doesn't decide the paragraph's direction.
    assert_eq!(first_strong_direction(&format!("{rli}{ahmet}{pdi} says hi")), Some(Ltr));
    assert_eq!(first_strong_direction(&format!("{ahmet} says hi")), Some(Rtl));
    assert_eq!(first_strong_direction(&format!("{lri}abc{pdi}{ahmet}")), Some(Rtl));
}

#[test]
fn explicit_embeddings_and_overrides_raise_levels_as_uax9_does() {
    // RLE: Latin embedded right to left is at level 2, shown in its own order.
    assert_eq!(resolved("\u{202B}abc\u{202C}", Ltr), (vec![None, Some(2), Some(2), Some(2), None], vec![1, 2, 3]));
    // RLO: what it holds is right to left whatever it is.
    assert_eq!(
        resolved("a\u{202E}bc\u{202C}d", Ltr),
        (vec![Some(0), None, Some(1), Some(1), None, Some(0)], vec![0, 3, 2, 5])
    );
    // LRO in a right-to-left paragraph: Hebrew forced left to right at level 2.
    assert_eq!(
        resolved("\u{05D0}\u{202D}\u{05D1}\u{05D2}\u{202C}", Rtl),
        (vec![Some(1), None, Some(2), Some(2), None], vec![2, 3, 0])
    );
    // LRE: Hebrew inside goes up to 3, a European number after it to 4 (rule I1).
    assert_eq!(resolved("\u{202A}\u{05D0}1\u{202C}", Ltr), (vec![None, Some(3), Some(4), None], vec![2, 1]));
    // PDF with nothing to close is ignored; an embedding closes at the paragraph's end.
    assert_eq!(resolved("a\u{202C}b", Ltr).0, vec![Some(0), None, Some(0)]);
    assert_eq!(resolved("\u{202B}a b", Ltr).0, vec![None, Some(2), Some(2), Some(2)]);
}

#[test]
fn numbers_and_neutrals_resolve_as_uax9_does() {
    // A European number after Hebrew: level 2 inside the Hebrew; its separators with it (W4).
    assert_eq!(resolved("\u{05D0} 1.5", Ltr), (vec![Some(1), Some(1), Some(2), Some(2), Some(2)], vec![2, 3, 4, 1, 0]));
    // A terminator next to a number joins it (W5).
    assert_eq!(resolved("\u{05D0} 12%", Ltr).0, vec![Some(1), Some(1), Some(2), Some(2), Some(2)]);
    // European numbers after Latin are Latin (W7).
    assert_eq!(resolved("a 12", Rtl).0, vec![Some(2), Some(2), Some(2), Some(2)]);
    // Arabic-Indic digits are Arabic numbers: level 2 in left-to-right text; the space before them the
    // paragraph's.
    assert_eq!(resolved("a \u{0661}\u{0662}", Ltr), (vec![Some(0), Some(0), Some(2), Some(2)], vec![0, 1, 2, 3]));
    // European digits after Arabic letters become Arabic numbers (W2): "12" after "أ" stays left to right inside.
    assert_eq!(resolved("\u{0623} 12", Rtl), (vec![Some(1), Some(1), Some(2), Some(2)], vec![2, 3, 1, 0]));
    // A mark takes the type of the character before it (W1): a Hebrew point is right to left.
    assert_eq!(resolved("\u{05D0}\u{05B7}", Ltr), (vec![Some(1), Some(1)], vec![1, 0]));
    // ...but after an isolate's PDI it is a neutral, here between Latin and the end: left to right.
    assert_eq!(resolved("a\u{2067}\u{05D0}\u{2069}\u{0300}", Ltr).0, vec![Some(0), Some(0), Some(1), Some(0), Some(0)]);
    // An FSI with nothing strong inside is an LRI: its number at level 2 in a right-to-left paragraph.
    assert_eq!(
        resolved("\u{05D0} \u{2068}12\u{2069}", Rtl),
        (vec![Some(1), Some(1), Some(1), Some(2), Some(2), Some(1)], vec![5, 3, 4, 2, 1, 0])
    );
    // A tab and the spaces before it go back to the paragraph's level (rule L1), splitting the Hebrew.
    assert_eq!(resolved("\u{05D0} \t\u{05D1}", Ltr), (vec![Some(1), Some(0), Some(0), Some(1)], vec![0, 1, 2, 3]));
}

#[test]
fn paragraphs_take_their_direction_as_compose_does_on_android() {
    let none = LocaleList::empty();
    let arabic = LocaleList::from_tags("ar-EG");
    assert_eq!(resolve_paragraph_direction("abc", TextDirection::Rtl, &none), Rtl);
    assert_eq!(resolve_paragraph_direction("שלום", TextDirection::Ltr, &none), Ltr);
    assert_eq!(resolve_paragraph_direction("12 שלום", TextDirection::ContentOrLtr, &none), Rtl);
    assert_eq!(resolve_paragraph_direction("12", TextDirection::ContentOrLtr, &none), Ltr);
    assert_eq!(resolve_paragraph_direction("12", TextDirection::ContentOrRtl, &none), Rtl);
    // Content: the first strong character, else the locale's direction.
    assert_eq!(resolve_paragraph_direction("abc", TextDirection::Content, &arabic), Ltr);
    assert_eq!(resolve_paragraph_direction("12", TextDirection::Content, &arabic), Rtl);
    assert_eq!(resolve_paragraph_direction("12", TextDirection::Content, &LocaleList::from_tags("tr-TR")), Ltr);
    for (tag, rtl) in [
        ("ar", true),
        ("fa-IR", true),
        ("he", true),
        ("ur-PK", true),
        ("pa-PK", true),
        ("pa-IN", false),
        ("az-Arab", true),
        ("uz-AF", true),
        ("tr", false),
        ("en-US", false),
        ("ja", false),
        ("sr-Latn", false),
    ] {
        assert_eq!(locale_is_rtl(&Locale::new(tag)), rtl, "{tag}");
    }
    let paragraph = BidiParagraph::new("abc אבג", TextDirection::ContentOrRtl, &none);
    assert_eq!(paragraph.direction(), Ltr);
    assert_eq!(paragraph.base_level(), 0);
    assert!(!paragraph.is_pure_ltr() && !paragraph.is_single_direction());
    assert!(BidiParagraph::with_direction("abc", Ltr).is_pure_ltr());
    assert!(BidiParagraph::with_direction("אבג", Rtl).is_single_direction());
}

#[test]
fn level_runs_are_what_a_paragraph_is_shaped_in() {
    let text = "ab אב 12 cd";
    let paragraph = BidiParagraph::with_direction(text, Ltr);
    let runs: Vec<(&str, u8)> =
        paragraph.level_runs(0..text.len()).iter().map(|r| (&text[r.range.clone()], r.level)).collect();
    assert_eq!(runs, vec![("ab ", 0), ("אב ", 1), ("12", 2), (" cd", 0)]);
    // Within a range, cut at its ends.
    let some: Vec<(&str, u8)> = paragraph.level_runs(1..5).iter().map(|r| (&text[r.range.clone()], r.level)).collect();
    assert_eq!(some, vec![("b ", 0), ("א", 1)]);
    assert_eq!(paragraph.level_runs(5..5), vec![BidiRun { range: 5..5, level: 1 }]);
    // Pieces at levels put in visual order (rule L2).
    assert_eq!(reorder_visual(&[0, 1, 2, 0]), vec![0, 2, 1, 3]);
    assert_eq!(reorder_visual(&[1, 1, 2, 2, 1]), vec![4, 2, 3, 1, 0]);
    assert_eq!(reorder_visual(&[]), Vec::<usize>::new());
}

#[test]
fn lines_are_ordered_on_their_own_with_their_trailing_spaces_at_the_paragraphs_level() {
    let text = "abc אב גד xyz";
    let paragraph = BidiParagraph::with_direction(text, Ltr);
    // A line ending inside the Hebrew: its space after "אב" goes back to the paragraph's level.
    let first = paragraph.line(text, 0..9);
    let runs: Vec<(&str, bool)> = first.runs().iter().map(|r| (&text[r.range.clone()], r.is_rtl())).collect();
    assert_eq!(runs, vec![("abc ", false), ("אב", true), (" ", false)]);
    assert_eq!(first.level_at(8), 0);
    assert_eq!(paragraph.level_at(8), 1);
    let second = paragraph.line(text, 9..text.len());
    let runs: Vec<(&str, bool)> = second.runs().iter().map(|r| (&text[r.range.clone()], r.is_rtl())).collect();
    assert_eq!(runs, vec![("גד", true), (" xyz", false)]);
    assert_eq!(second.logical_runs()[0].range, 9..13);
    assert_eq!(second.paragraph_direction(), Ltr);
}

#[test]
fn carets_go_where_android_puts_the_primary_and_secondary_ones() {
    // "abc אבג" left to right, 10 px a character: "abc " on the left (0–40), the Hebrew right to left (40–70).
    let text = "abc אבג";
    let paragraph = BidiParagraph::with_direction(text, Ltr);
    let line = paragraph.line(text, 0..text.len());
    let width = |r: std::ops::Range<usize>| text[r].chars().count() as f32 * 10.0;
    assert_eq!(line.runs().len(), 2);
    // Inside a run both carets agree.
    assert_eq!(line.horizontal(2, true, width), 20.0);
    assert_eq!(line.horizontal(2, false, width), 20.0);
    assert_eq!(line.horizontal(6, true, width), 60.0); // between א and ב
    // At the boundary: the primary caret after the space, the secondary at the Hebrew's right edge.
    assert!(line.is_level_boundary(4) && line.primary_is_trailing_previous(4));
    assert_eq!(line.horizontal(4, true, width), 40.0);
    assert_eq!(line.horizontal(4, false, width), 70.0);
    // At the end: the primary at the line's right edge, the secondary after the last Hebrew letter.
    assert!(line.is_level_boundary(text.len()) && !line.primary_is_trailing_previous(text.len()));
    assert_eq!(line.horizontal(text.len(), true, width), 70.0);
    assert_eq!(line.horizontal(text.len(), false, width), 40.0);
    assert_eq!(line.horizontal(0, true, width), 0.0);
    assert!(!line.is_level_boundary(0) && !line.is_level_boundary(6));
    // Directions of runs (Compose's getBidiRunDirection); past the end of a mixed line, left to right.
    assert_eq!(line.run_direction(5), Rtl);
    assert_eq!(line.run_direction(1), Ltr);
    assert_eq!(line.run_direction(text.len()), Ltr);
    assert_eq!(line.run_index_at(4), Some(1));

    // A right-to-left paragraph: its start on the right.
    let text = "אב cd";
    let paragraph = BidiParagraph::with_direction(text, Rtl);
    let line = paragraph.line(text, 0..text.len());
    let width = |r: std::ops::Range<usize>| text[r].chars().count() as f32 * 10.0;
    // On screen: "cd" (0–20), then " בא" (20–50).
    assert_eq!(line.horizontal(0, true, width), 50.0);
    assert_eq!(line.horizontal(2, true, width), 40.0);
    // Where the Latin starts: the primary caret after the space (its left edge), the secondary at "c"'s left.
    assert_eq!(line.horizontal(5, true, width), 20.0);
    assert_eq!(line.horizontal(5, false, width), 0.0);
    assert_eq!(line.horizontal(text.len(), true, width), 0.0);
    assert_eq!(line.horizontal(text.len(), false, width), 20.0);
    // All of a line right to left: past its end too.
    let text = "אבג";
    let line = BidiParagraph::with_direction(text, Rtl).line(text, 0..text.len());
    assert_eq!(line.run_direction(text.len()), Rtl);
    assert!(line.is_single_run() && !line.is_level_boundary(2));
}

#[test]
fn mirrored_characters_are_unicodes() {
    assert_eq!(mirrored_char('('), Some(')'));
    assert_eq!(mirrored_char('»'), Some('«'));
    assert_eq!(mirrored_char('<'), Some('>'));
    assert_eq!(mirrored_char('a'), None);
    assert!(is_mirrored('∛') && mirrored_char('∛').is_none());
    assert!(!is_mirrored('-'));
    assert_eq!(level_direction(3), Rtl);
    assert_eq!(direction_level(Rtl), 1);
}
