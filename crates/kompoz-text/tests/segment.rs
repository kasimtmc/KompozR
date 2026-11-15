//! Segmentation: grapheme clusters keeping emoji sequences whole (the C++ Kompoz's
//! grapheme_clusters_keep_emoji_sequences_whole, its cluster part; editing by clusters is the text field's, stage
//! 10), emoji presentation (some_characters_are_emoji_without_a_variation_selector), Compose's StringHelpers, words
//! and sentences by ICU, scripts and directions.

use kompoz_text::segment::*;
use kompoz_text::{Locale, TextRange};

fn clusters(s: &str) -> Vec<&str> {
    graphemes(s).map(|r| &s[r]).collect()
}

/// The clusters as the C++ test walked them: from each end to the next.
fn walked(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        let j = next_grapheme(s, i);
        out.push(&s[i..j]);
        i = j;
    }
    out
}

#[test]
fn some_characters_are_emoji_without_a_variation_selector() {
    for c in ['\u{1F44D}', '\u{231A}', '\u{1F1F9}', '\u{1F600}', '\u{1FAF8}', '\u{2B50}', '\u{1F004}'] {
        assert!(is_emoji_presentation(c), "{c:?}");
    }
    // Text unless U+FE0F follows (❤, ☺, ©, 🌡), and what is no emoji at all (U+1FAF9 is unassigned in the
    // Unicode the C++ table had; ICU's 17 has it as an emoji, shown as one).
    for c in ['A', '\u{2764}', '\u{263A}', '\u{00A9}', '\u{1F321}', '\u{2319}'] {
        assert!(!is_emoji_presentation(c), "{c:?}");
    }
    assert!(is_emoji_presentation('\u{1FAF9}'));
    assert!(is_emoji('\u{2764}') && is_emoji('\u{00A9}') && !is_emoji('A') && is_emoji('1'));
    assert!(is_extended_pictographic('\u{2764}') && !is_extended_pictographic('1'));
    assert!(is_regional_indicator('\u{1F1F9}') && !is_regional_indicator('T'));
    assert!(is_emoji_modifier('\u{1F3FD}') && is_emoji_modifier_base('\u{1F44D}') && !is_emoji_modifier_base('A'));
    assert!(is_emoji_component('\u{200D}') && is_emoji_component('\u{20E3}') && is_emoji_component('#'));
    assert!(is_variation_selector('\u{FE0F}') && is_variation_selector('\u{E0100}') && !is_variation_selector('a'));
    // How clusters are shown: a selector asks, sequences are emoji, else the first character's default.
    assert_eq!(presentation("\u{2764}"), Presentation::Text);
    assert_eq!(presentation("\u{2764}\u{FE0F}"), Presentation::Emoji);
    assert_eq!(presentation("\u{1F44D}"), Presentation::Emoji);
    assert_eq!(presentation("\u{1F44D}\u{FE0E}"), Presentation::Text);
    assert_eq!(presentation("1\u{FE0F}\u{20E3}"), Presentation::Emoji);
    assert_eq!(presentation("1"), Presentation::Text);
    assert_eq!(presentation("\u{1F1F9}\u{1F1F7}"), Presentation::Emoji);
    assert!(
        is_emoji_sequence("\u{1F44D}\u{1F3FD}") && !is_emoji_sequence("\u{1F44D}") && !is_emoji_sequence("e\u{301}")
    );
}

#[test]
fn grapheme_clusters_keep_emoji_sequences_whole() {
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"; // man, woman, girl: one family
    let turkey = "\u{1F1F9}\u{1F1F7}";
    let germany = "\u{1F1E9}\u{1F1EA}";
    let thumbs = "\u{1F44D}\u{1F3FD}";
    let keycap = "1\u{FE0F}\u{20E3}";
    let heart = "\u{2764}\u{FE0F}";
    let england = "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}";
    let accented = "e\u{0301}";
    let s = format!("a{family}b");
    assert_eq!(walked(&s), vec!["a", family, "b"]);
    assert_eq!(clusters(&s), vec!["a", family, "b"]);
    let flags = format!("{turkey}{germany}");
    assert_eq!(walked(&flags), vec![turkey, germany]);
    let many = format!("{thumbs}{keycap}{heart}{england}{accented}x");
    assert_eq!(walked(&many), vec![thumbs, keycap, heart, england, accented, "x"]);
    let mixed = format!("ab{family}");
    // From its end and from inside it too.
    assert!(previous_grapheme(&mixed, mixed.len()) == 2 && previous_grapheme(&mixed, 5) == 2);
    assert_eq!(next_grapheme(&mixed, 5), mixed.len());
    assert!(is_grapheme_boundary(&mixed, 2) && !is_grapheme_boundary(&mixed, 6) && is_grapheme_boundary(&mixed, 0));
    assert!(is_grapheme_boundary(&mixed, mixed.len()) && !is_grapheme_boundary(&mixed, mixed.len() + 1));
    // Devanagari: a consonant, a virama and a consonant are one cluster (UAX #29's conjuncts), the vowel sign
    // with it.
    assert_eq!(clusters("क्षि नम"), vec!["क्षि", " ", "न", "म"]);
    // A carriage return and a line feed are one; each paragraph segments on its own.
    assert_eq!(clusters("a\r\nb"), vec!["a", "\r\n", "b"]);
    assert_eq!(walked("\u{1F1F9}\n\u{1F1F7}\u{1F1F9}"), vec!["\u{1F1F9}", "\n", "\u{1F1F7}\u{1F1F9}"]);
}

#[test]
fn string_helpers_find_breaks_as_composes_do() {
    let text = "ab\u{1F468}\u{200D}\u{1F469}c";
    let family_end = text.len() - 1;
    // following: the first boundary after; DONE at the end.
    assert_eq!(find_following_break(text, 0), Some(1));
    assert_eq!(find_following_break(text, 2), Some(family_end));
    assert_eq!(find_following_break(text, 4), Some(family_end));
    assert_eq!(find_following_break(text, family_end), Some(text.len()));
    assert_eq!(find_following_break(text, text.len()), None);
    // preceding: the last boundary before; DONE at the start.
    assert_eq!(find_preceding_break(text, text.len()), Some(family_end));
    assert_eq!(find_preceding_break(text, family_end), Some(2));
    assert_eq!(find_preceding_break(text, 1), Some(0));
    assert_eq!(find_preceding_break(text, 0), None);
    // Across a line feed.
    assert_eq!(find_preceding_break("a\nb", 2), Some(1));
    assert_eq!(find_following_break("a\nb", 1), Some(2));
    // Paragraphs.
    let p = "one\ntwo\n\nthree";
    assert_eq!((find_paragraph_start(p, 5), find_paragraph_end(p, 5)), (4, 7));
    assert_eq!((find_paragraph_start(p, 4), find_paragraph_end(p, 3)), (4, 3));
    assert_eq!(paragraph_boundary(p, 8), TextRange::new(8, 8));
    assert_eq!(paragraph_boundary(p, p.len()), TextRange::new(9, p.len()));
    assert_eq!(paragraph_boundary(p, 0), TextRange::new(0, 3));
    // The code point before, or the emoji it ends.
    assert_eq!(find_code_point_or_emoji_start_before(text, family_end), Some(2));
    assert_eq!(find_code_point_or_emoji_start_before(text, text.len()), Some(family_end));
    assert_eq!(find_code_point_or_emoji_start_before("e\u{301}", 3), Some(1));
    assert_eq!(find_code_point_or_emoji_start_before("ab", 0), None);
    assert_eq!(code_point_before("aç", 3), Some((1, 'ç')));
    assert_eq!(code_point_at("aç", 1), Some((1, 'ç')));
    assert_eq!(code_point_at("aç", 2), None);
    assert_eq!(code_point_count("aç👍", 0..7), 3);
}

#[test]
fn words_and_sentences_are_icus() {
    let text = "Merhaba dünya, 2026'da!";
    let words = words(text, None);
    let like: Vec<&str> = words.iter().filter(|w| w.kind.is_word_like()).map(|w| &text[w.range.clone()]).collect();
    // An apostrophe joins letters (and digits) but not a number and letters (UAX #29's WB6/7, WB11/12).
    assert_eq!(like, vec!["Merhaba", "dünya", "2026", "da"]);
    assert_eq!(words.iter().find(|w| &text[w.range.clone()] == "2026").unwrap().kind, WordKind::Number);
    assert_eq!(words.iter().find(|w| &text[w.range.clone()] == "dünya").unwrap().kind, WordKind::Letter);
    assert_eq!(word_breaks("ab cd", None), vec![0, 2, 3, 5]);
    // Thai has no spaces between words: its dictionary finds them.
    let thai = "สวัสดีครับ";
    assert!(word_breaks(thai, None).len() > 2);
    // Sentences; Greek's question mark is a semicolon.
    assert_eq!(sentence_breaks("Bir. İki? Üç", None), vec![0, 5, 11, 15]);
    assert_eq!(sentence_breaks("hello; world", None), vec![0, 12]);
    let greek = Locale::new("el");
    assert_eq!(sentence_breaks("hello; world", Some(&greek)), vec![0, 7, 12]);
}

#[test]
fn scripts_and_directions_of_characters() {
    assert_eq!(&script_tag('a'), b"Latn");
    assert_eq!(&script_tag('م'), b"Arab");
    assert_eq!(&script_tag('न'), b"Deva");
    assert_eq!(&script_tag(' '), b"Zyyy");
    assert_eq!(&script_tag('\u{301}'), b"Zinh");
    assert_eq!(&script_of("12 שלום"), b"Hebr");
    assert_eq!(&script_of("12 !"), b"Zyyy");
    assert!(is_common_or_inherited('1') && is_common_or_inherited('\u{301}') && !is_common_or_inherited('a'));
    assert!(is_direction_neutral(' ') && is_direction_neutral('1') && is_direction_neutral('!'));
    assert!(!is_direction_neutral('a') && !is_direction_neutral('ש') && !is_direction_neutral('م'));
    assert!(!is_direction_neutral('\u{0661}') && !is_direction_neutral('\u{2067}'));
}
