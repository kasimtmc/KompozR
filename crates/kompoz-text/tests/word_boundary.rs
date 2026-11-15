//! Word boundaries for selection as Compose finds them on Android (WordIterator, getWordStart/getWordEnd,
//! WordSegmentFinder), over icu's word segmenter.

use kompoz_text::word_boundary::*;
use kompoz_text::{Locale, TextRange};

fn word(text: &str, offset: usize) -> &str {
    let range = word_boundary(text, offset, &Locale::new("en"));
    &text[range.min()..range.max()]
}

#[test]
fn a_word_is_selected_around_an_offset() {
    let text = "abc def-ghi 123";
    assert_eq!(word(text, 1), "abc");
    assert_eq!(word(text, 0), "abc");
    // At a word's end, the word before; at its start, the word after.
    assert_eq!(word_boundary(text, 3, &Locale::new("en")), TextRange::new(0, 3));
    assert_eq!(word_boundary(text, 4, &Locale::new("en")), TextRange::new(4, 7));
    // A hyphen separates words.
    assert_eq!(word(text, 5), "def");
    assert_eq!(word(text, 9), "ghi");
    assert_eq!(word(text, 13), "123");
    assert_eq!(word(text, text.len()), "123");
    // Letters and digits run together; an apostrophe inside a word stays in it (UAX #29).
    assert_eq!(word("abc123 x", 2), "abc123");
    assert_eq!(word("can't stop", 1), "can't");
    // Turkish, in bytes.
    assert_eq!(word("İyi günler dilerim", 2), "İyi");
    assert_eq!(word_boundary("İyi günler dilerim", 8, &Locale::new("tr")), TextRange::new(5, 12));
}

#[test]
fn spaces_are_no_word_and_punctuation_runs_are_one() {
    // Between two spaces: a caret.
    assert_eq!(word_boundary("abc  def", 4, &Locale::new("en")), TextRange::new(4, 4));
    // Inside a run of punctuation: the run.
    assert_eq!(word_boundary("abc,,, def", 4, &Locale::new("en")), TextRange::new(3, 6));
    assert_eq!(word("abc... def", 5), "...");
    assert!(is_punctuation(',') && is_punctuation('«') && is_punctuation('_') && !is_punctuation('$'));
}

#[test]
fn letters_and_emoji_run_together_but_hiragana_and_katakana_part() {
    // An emoji next to a letter is one word with it (Compose treats emoji as letters).
    assert_eq!(word("hi 😀x there", 7), "😀x");
    assert_eq!(word("a 👍🏽 b", 2), "👍🏽");
    // Kanji and katakana are letters: one word.
    assert_eq!(word("日本語テキスト", 3), "日本語テキスト");
    // Hiragana meeting katakana is a boundary.
    let text = "すごいカメラ";
    let ja = Locale::new("ja");
    assert_eq!(word_boundary(text, 3, &ja), TextRange::new(0, 9));
    assert_eq!(word_boundary(text, 12, &ja), TextRange::new(9, 18));
}

#[test]
fn word_iterator_walks_boundaries_as_composes() {
    let text = "Hello, World!";
    let words = WordIterator::new(text, &Locale::new("en"));
    assert_eq!(words.next_boundary(0), Some(5));
    assert_eq!(words.next_boundary(5), Some(6));
    assert_eq!(words.prev_boundary(7), Some(6));
    assert_eq!(words.prev_boundary(0), None);
    assert_eq!(words.next_boundary(text.len()), None);
    assert!(words.is_on_punctuation(5) && words.is_after_punctuation(6) && !words.is_on_punctuation(0));
    assert_eq!(words.punctuation_beginning(12), Some(12));
    assert_eq!(words.punctuation_end(12), Some(13));
    assert_eq!(words.prev_word_beginning_on_two_words_boundary(3), Some(0));
    assert_eq!(words.next_word_end_on_two_word_boundary(8), Some(12));
    assert_eq!(words.prev_word_beginning_on_two_words_boundary(6), None);
    assert_eq!(words.word_start(9), 7);
    assert_eq!(words.word_end(9), 12);
    assert_eq!(words.text(), text);
}

#[test]
fn word_segments_leave_spaces_out() {
    // "Hello", ",", "World", "!".
    let text = "Hello, World!";
    let finder = WordSegmentFinder::new(WordIterator::new(text, &Locale::new("en")));
    assert_eq!(finder.next_start_boundary(0), Some(5));
    assert_eq!(finder.next_end_boundary(0), Some(5));
    assert_eq!(finder.next_start_boundary(5), Some(7)); // the space skipped
    assert_eq!(finder.previous_end_boundary(7), Some(6));
    assert_eq!(finder.previous_start_boundary(12), Some(7));
    assert_eq!(finder.next_end_boundary(12), Some(13));
    assert_eq!(finder.next_start_boundary(12), None);
    assert_eq!(finder.previous_start_boundary(0), None);
    assert_eq!(finder.previous_end_boundary(5), None);
}
