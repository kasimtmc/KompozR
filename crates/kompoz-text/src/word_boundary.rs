//! Word boundaries for selection (Compose's WordIterator, WordBoundary and WordSegmentFinder on Android), over
//! icu's word segmenter (UAX #29 with its dictionaries: Chinese and Japanese words, Thai, Lao, Khmer, Burmese):
//! a double click or a long press selects the word around an offset; letters, digits and emoji run together
//! across ICU's boundaries; hiragana meeting katakana is a boundary; a run of punctuation is a word of its own;
//! spaces are no word.

use crate::intl::Locale;
use crate::range::TextRange;
use icu_properties::props::{Emoji, EmojiPresentation, GeneralCategory};
use icu_properties::{CodePointMapData, CodePointSetData};
use icu_segmenter::WordSegmenter;
use icu_segmenter::options::WordBreakOptions;

/// ICU's word boundaries of `text` (icu's word segmenter with its dictionaries and models, tailored for
/// `locale`), 0 and the text's length included.
pub(crate) fn word_boundaries(text: &str, locale: &Locale) -> Vec<usize> {
    let mut options = WordBreakOptions::default();
    options.content_locale = Some(locale.language_identifier());
    match WordSegmenter::try_new_auto(options) {
        Ok(segmenter) => segmenter.as_borrowed().segment_str(text).collect(),
        Err(_) => WordSegmenter::new_auto(Default::default()).segment_str(text).collect(),
    }
}

/// Walks `text`'s word boundaries (Compose's WordIterator, after Android's): ICU's boundaries, those between two
/// letters, digits or emoji left out but where hiragana meets katakana. Offsets are bytes; None where Java's
/// BreakIterator says DONE.
#[derive(Clone, Debug)]
pub struct WordIterator<'a> {
    text: &'a str,
    boundaries: Vec<usize>,
}

impl<'a> WordIterator<'a> {
    /// The word boundaries of `text` in `locale`.
    pub fn new(text: &'a str, locale: &Locale) -> WordIterator<'a> {
        WordIterator { text, boundaries: word_boundaries(text, locale) }
    }

    /// The text it walks.
    pub fn text(&self) -> &'a str {
        self.text
    }

    fn check(&self, offset: usize) {
        assert!(
            offset <= self.text.len() && self.text.is_char_boundary(offset),
            "kompoz: invalid offset {offset}; valid offsets are character boundaries in [0, {}]",
            self.text.len()
        );
    }

    /// The start of the character before `offset`.
    fn before(&self, offset: usize) -> Option<usize> {
        self.text[..offset].char_indices().next_back().map(|(i, _)| i)
    }

    /// The start of the character after the one at `offset`.
    fn after(&self, offset: usize) -> usize {
        offset + self.text[offset..].chars().next().map_or(0, char::len_utf8)
    }

    /// ICU's first boundary after `offset`.
    fn following(&self, offset: usize) -> Option<usize> {
        let i = self.boundaries.partition_point(|&b| b <= offset);
        self.boundaries.get(i).copied()
    }

    /// ICU's last boundary before `offset`.
    fn preceding(&self, offset: usize) -> Option<usize> {
        let i = self.boundaries.partition_point(|&b| b < offset);
        i.checked_sub(1).map(|i| self.boundaries[i])
    }

    /// The next boundary after `offset` (Compose's nextBoundary).
    pub fn next_boundary(&self, offset: usize) -> Option<usize> {
        self.check(offset);
        let mut offset = offset;
        loop {
            let following = self.following(offset)?;
            let before = self.before(following);
            if before.is_some_and(|b| self.is_on_letter_or_digit_or_emoji(b))
                && self.is_on_letter_or_digit_or_emoji(following)
                && !self.is_hiragana_katakana_boundary(following)
            {
                offset = following;
                continue;
            }
            return Some(following);
        }
    }

    /// The boundary before `offset` (Compose's prevBoundary).
    pub fn prev_boundary(&self, offset: usize) -> Option<usize> {
        self.check(offset);
        let mut offset = offset;
        loop {
            let preceding = self.preceding(offset)?;
            if self.is_on_letter_or_digit_or_emoji(preceding)
                && self.is_after_letter_or_digit_or_emoji(preceding)
                && !self.is_hiragana_katakana_boundary(preceding)
            {
                offset = preceding;
                continue;
            }
            return Some(preceding);
        }
    }

    /// The start of the word `offset` is in or starts; at a boundary between two words, the start of the one
    /// before (Compose's getPrevWordBeginningOnTwoWordsBoundary).
    pub fn prev_word_beginning_on_two_words_boundary(&self, offset: usize) -> Option<usize> {
        self.beginning(offset, true)
    }

    /// The end of the word `offset` is in or ends; at a boundary between two words, the end of the one after
    /// (Compose's getNextWordEndOnTwoWordBoundary).
    pub fn next_word_end_on_two_word_boundary(&self, offset: usize) -> Option<usize> {
        self.end(offset, true)
    }

    /// The start of the run of punctuation `offset` is in (Compose's getPunctuationBeginning).
    pub fn punctuation_beginning(&self, offset: usize) -> Option<usize> {
        self.check(offset);
        let mut result = Some(offset);
        while let Some(r) = result {
            if self.is_punctuation_start_boundary(r) {
                break;
            }
            result = self.prev_boundary(r);
        }
        result
    }

    /// The end of the run of punctuation `offset` is in (Compose's getPunctuationEnd).
    pub fn punctuation_end(&self, offset: usize) -> Option<usize> {
        self.check(offset);
        let mut result = Some(offset);
        while let Some(r) = result {
            if self.is_punctuation_end_boundary(r) {
                break;
            }
            result = self.next_boundary(r);
        }
        result
    }

    /// Whether the character before `offset` is punctuation.
    pub fn is_after_punctuation(&self, offset: usize) -> bool {
        offset > 0 && offset <= self.text.len() && self.before(offset).is_some_and(|b| is_punctuation(self.char_at(b)))
    }

    /// Whether the character at `offset` is punctuation.
    pub fn is_on_punctuation(&self, offset: usize) -> bool {
        offset < self.text.len() && is_punctuation(self.char_at(offset))
    }

    fn char_at(&self, offset: usize) -> char {
        self.text[offset..].chars().next().expect("a character at the offset")
    }

    fn beginning(&self, offset: usize, prev_word_on_two_words_boundary: bool) -> Option<usize> {
        self.check(offset);
        if self.is_on_letter_or_digit_or_emoji(offset) {
            if self.is_boundary(offset)
                && (!self.is_after_letter_or_digit_or_emoji(offset) || !prev_word_on_two_words_boundary)
            {
                Some(offset)
            } else {
                self.prev_boundary(offset)
            }
        } else if self.is_after_letter_or_digit_or_emoji(offset) {
            self.prev_boundary(offset)
        } else {
            None
        }
    }

    fn end(&self, offset: usize, next_word_on_two_words_boundary: bool) -> Option<usize> {
        self.check(offset);
        if self.is_after_letter_or_digit_or_emoji(offset) {
            if self.is_boundary(offset)
                && (!self.is_on_letter_or_digit_or_emoji(offset) || !next_word_on_two_words_boundary)
            {
                Some(offset)
            } else {
                self.next_boundary(offset)
            }
        } else if self.is_on_letter_or_digit_or_emoji(offset) {
            self.next_boundary(offset)
        } else {
            None
        }
    }

    fn is_punctuation_start_boundary(&self, offset: usize) -> bool {
        self.is_on_punctuation(offset) && !self.is_after_punctuation(offset)
    }

    fn is_punctuation_end_boundary(&self, offset: usize) -> bool {
        !self.is_on_punctuation(offset) && self.is_after_punctuation(offset)
    }

    /// Whether the character before `offset` is a letter, a digit or part of an emoji.
    fn is_after_letter_or_digit_or_emoji(&self, offset: usize) -> bool {
        offset > 0
            && offset <= self.text.len()
            && self.before(offset).is_some_and(|b| self.is_on_letter_or_digit_or_emoji(b))
    }

    /// Whether the character at `offset` is a letter, a digit or part of an emoji (Java's isLetterOrDigit, a
    /// character outside the Basic Multilingual Plane, or an emoji as EmojiCompat finds them).
    fn is_on_letter_or_digit_or_emoji(&self, offset: usize) -> bool {
        if offset >= self.text.len() {
            return false;
        }
        let c = self.char_at(offset);
        is_letter_or_digit(c) || c as u32 > 0xFFFF || self.is_emoji_at(offset, c)
    }

    /// Whether the character at `offset` is part of an emoji: one shown as an emoji by default, one a variation
    /// selector asks to be, or a joiner, variation selector or keycap mark continuing one.
    fn is_emoji_at(&self, offset: usize, c: char) -> bool {
        let emoji = CodePointSetData::new::<Emoji>();
        if CodePointSetData::new::<EmojiPresentation>().contains(c) {
            return true;
        }
        let next = self.text[offset + c.len_utf8()..].chars().next();
        if emoji.contains(c) && next == Some('\u{FE0F}') {
            return true;
        }
        if matches!(c, '\u{FE0F}' | '\u{200D}' | '\u{20E3}') {
            return self.before(offset).is_some_and(|b| {
                let p = self.char_at(b);
                emoji.contains(p) || p == '\u{FE0F}'
            });
        }
        false
    }

    /// ICU's boundary at `offset`, unless letters, digits or emoji run through it or hiragana meets katakana
    /// around it (Compose's isBoundary).
    fn is_boundary(&self, offset: usize) -> bool {
        self.check(offset);
        let icu = offset == 0 || self.boundaries.binary_search(&offset).is_ok();
        let after = if offset < self.text.len() { self.after(offset) } else { offset };
        icu && !(self.is_on_letter_or_digit_or_emoji(offset)
            && self.before(offset).is_some_and(|b| self.is_on_letter_or_digit_or_emoji(b))
            && self.is_on_letter_or_digit_or_emoji(after))
            && !(offset > 0
                && after < self.text.len()
                && (self.is_hiragana_katakana_boundary(offset) || self.is_hiragana_katakana_boundary(after)))
    }

    /// Whether the characters before and at `offset` are hiragana and katakana, either way round.
    fn is_hiragana_katakana_boundary(&self, offset: usize) -> bool {
        let (Some(b), true) = (self.before(offset), offset < self.text.len()) else { return false };
        let (p, c) = (self.char_at(b), self.char_at(offset));
        (is_hiragana(p) && is_katakana(c)) || (is_hiragana(c) && is_katakana(p))
    }

    /// The start of the word at `offset`; `offset` itself where there is none (Compose's getWordStart):
    /// punctuation's run start on punctuation.
    pub fn word_start(&self, offset: usize) -> usize {
        let previous = self.prev_boundary(offset);
        let start = if previous.is_some_and(|p| self.is_on_punctuation(p)) {
            self.punctuation_beginning(offset)
        } else {
            self.prev_word_beginning_on_two_words_boundary(offset)
        };
        start.unwrap_or(offset)
    }

    /// The end of the word at `offset`; `offset` itself where there is none (Compose's getWordEnd):
    /// punctuation's run end after punctuation.
    pub fn word_end(&self, offset: usize) -> usize {
        let next = self.next_boundary(offset);
        let end = if next.is_some_and(|n| self.is_after_punctuation(n)) {
            self.punctuation_end(offset)
        } else {
            self.next_word_end_on_two_word_boundary(offset)
        };
        end.unwrap_or(offset)
    }

    /// The word at `offset` (Compose's Paragraph.getWordBoundary): its start and end, a collapsed range at
    /// `offset` in spaces.
    pub fn word_boundary(&self, offset: usize) -> TextRange {
        TextRange::new(self.word_start(offset), self.word_end(offset))
    }
}

/// The word around `offset` in `text` (Compose's getWordBoundary); see [`WordIterator`].
pub fn word_boundary(text: &str, offset: usize, locale: &Locale) -> TextRange {
    WordIterator::new(text, locale).word_boundary(offset)
}

/// Finds the words of a text for a range query (Compose's WordSegmentFinder, Android's SegmentFinder): its
/// segments are the words, numbers, emoji and runs of punctuation between [`WordIterator`]'s boundaries; spaces
/// belong to none ("Hello, World!" is "Hello", ",", "World", "!").
#[derive(Clone, Debug)]
pub struct WordSegmentFinder<'a> {
    words: WordIterator<'a>,
}

impl<'a> WordSegmentFinder<'a> {
    pub fn new(words: WordIterator<'a>) -> WordSegmentFinder<'a> {
        WordSegmentFinder { words }
    }

    fn whitespace_at(&self, offset: usize) -> bool {
        self.words.text[offset..].chars().next().is_some_and(is_java_whitespace)
    }

    fn whitespace_before(&self, offset: usize) -> bool {
        self.words.text[..offset].chars().next_back().is_some_and(is_java_whitespace)
    }

    /// The start of a segment before `offset`.
    pub fn previous_start_boundary(&self, offset: usize) -> Option<usize> {
        let mut boundary = offset;
        loop {
            boundary = self.words.prev_boundary(boundary)?;
            if !self.whitespace_at(boundary) {
                return Some(boundary);
            }
        }
    }

    /// The end of a segment before `offset`.
    pub fn previous_end_boundary(&self, offset: usize) -> Option<usize> {
        let mut boundary = offset;
        loop {
            boundary = self.words.prev_boundary(boundary).filter(|&b| b > 0)?;
            if !self.whitespace_before(boundary) {
                return Some(boundary);
            }
        }
    }

    /// The start of a segment after `offset`.
    pub fn next_start_boundary(&self, offset: usize) -> Option<usize> {
        let mut boundary = offset;
        loop {
            boundary = self.words.next_boundary(boundary).filter(|&b| b < self.words.text.len())?;
            if !self.whitespace_at(boundary) {
                return Some(boundary);
            }
        }
    }

    /// The end of a segment after `offset`.
    pub fn next_end_boundary(&self, offset: usize) -> Option<usize> {
        let mut boundary = offset;
        loop {
            boundary = self.words.next_boundary(boundary)?;
            if !self.whitespace_before(boundary) {
                return Some(boundary);
            }
        }
    }
}

/// Java's Character.isLetterOrDigit: a letter (Lu, Ll, Lt, Lm, Lo) or a decimal digit (Nd).
fn is_letter_or_digit(c: char) -> bool {
    matches!(
        CodePointMapData::<GeneralCategory>::new().get(c),
        GeneralCategory::UppercaseLetter
            | GeneralCategory::LowercaseLetter
            | GeneralCategory::TitlecaseLetter
            | GeneralCategory::ModifierLetter
            | GeneralCategory::OtherLetter
            | GeneralCategory::DecimalNumber
    )
}

/// Punctuation as Compose's WordIterator.isPunctuation: connector, dash, open, close, initial and final quote,
/// and other punctuation.
pub fn is_punctuation(c: char) -> bool {
    matches!(
        CodePointMapData::<GeneralCategory>::new().get(c),
        GeneralCategory::ConnectorPunctuation
            | GeneralCategory::DashPunctuation
            | GeneralCategory::ClosePunctuation
            | GeneralCategory::FinalPunctuation
            | GeneralCategory::InitialPunctuation
            | GeneralCategory::OtherPunctuation
            | GeneralCategory::OpenPunctuation
    )
}

/// Java's Character.isWhitespace: space, line and paragraph separators but the no-break ones, and the ASCII
/// controls tab, line feed, vertical tab, form feed, carriage return and the four information separators.
fn is_java_whitespace(c: char) -> bool {
    match CodePointMapData::<GeneralCategory>::new().get(c) {
        GeneralCategory::SpaceSeparator | GeneralCategory::LineSeparator | GeneralCategory::ParagraphSeparator => {
            !matches!(c, '\u{00A0}' | '\u{2007}' | '\u{202F}')
        }
        _ => matches!(c, '\t' | '\n' | '\u{000B}' | '\u{000C}' | '\r' | '\u{001C}'..='\u{001F}'),
    }
}

/// Java's Character.UnicodeBlock HIRAGANA.
fn is_hiragana(c: char) -> bool {
    ('\u{3040}'..='\u{309F}').contains(&c)
}

/// Java's Character.UnicodeBlock KATAKANA.
fn is_katakana(c: char) -> bool {
    ('\u{30A0}'..='\u{30FF}').contains(&c)
}
