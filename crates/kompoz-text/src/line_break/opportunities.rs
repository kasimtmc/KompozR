//! Where lines may break: UAX #14 through icu's LineSegmenter with a [`LineBreak`]'s strictness and word break
//! (locale aware: Chinese and Japanese strictness, Korean and Japanese phrases, Thai, Lao, Khmer and Burmese by
//! icu's models), tailored as minikin's WordBreaker: no break after a soft hyphen (hyphenation's business) or a
//! Myanmar virama, email addresses and URLs broken at the Chicago Manual of Style's points.

use crate::intl::Locale;
use crate::style::line_break::{LineBreak, Strictness, WordBreak};
use crate::word_boundary::word_boundaries;
use icu_properties::CodePointMapData;
use icu_properties::props::{GeneralCategory, LineBreak as LineBreakClass, Script};
use icu_segmenter::options::{LineBreakOptions, LineBreakStrictness, LineBreakWordOption};
use icu_segmenter::{GraphemeClusterSegmenter, LineSegmenter};

/// A place a line may, or must, break: before the character at `offset` (the text's length for its end).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BreakOpportunity {
    pub offset: usize,
    /// A line must end here: after a line feed, carriage return (with its line feed), next line, line or
    /// paragraph separator, vertical tab or form feed (UAX #14's BK, CR, LF and NL classes).
    pub mandatory: bool,
    /// The text since the previous opportunity is (part of) an email address or a URL: never hyphenated.
    pub email_or_url: bool,
    /// Inside an email address or a URL (not at its end), at one of the Chicago Manual of Style's points: line
    /// breaking avoids it where it can (minikin's break badness).
    pub inside_email_or_url: bool,
}

/// Where `text` may break into lines (`text.len()` always, last): UAX #14 opportunities and mandatory breaks for
/// `line_break`'s strictness and word break in `locale` (its Unspecified parts taken as Default), tailored as
/// minikin's WordBreaker.
///
/// Strictness: `Default` is the locale's (normal for Chinese and Japanese, strict elsewhere, as ICU's locale
/// rules), `Loose`, `Normal` and `Strict` CSS's line-break values. Word break `Phrase` keeps Korean words whole
/// (breaking at spaces, CSS's keep-all) and Japanese phrases together (a break only where a word that isn't all
/// hiragana begins: particles and endings stay with the word before them); elsewhere it breaks as `Default`.
pub fn break_opportunities(text: &str, line_break: LineBreak, locale: &Locale) -> Vec<BreakOpportunity> {
    let phrase = phrase_kind(line_break, locale);
    let word_option =
        if phrase == Phrase::KeepAll { LineBreakWordOption::KeepAll } else { LineBreakWordOption::Normal };
    let mut points = uax14_opportunities(text, line_break.strictness, word_option, locale);
    if phrase == Phrase::Japanese {
        let words = word_boundaries(text, locale);
        points.retain(|p| p.mandatory || p.offset == text.len() || japanese_phrase_break(text, p.offset, &words));
    }
    points
}

/// The opportunities phrase breaking left out, where a phrase too long for a line breaks before its characters
/// are split (minikin's fallback points); none when `line_break` doesn't break by phrases in `locale`.
pub(crate) fn phrase_fallback(text: &str, line_break: LineBreak, locale: &Locale) -> Vec<usize> {
    if phrase_kind(line_break, locale) == Phrase::None {
        return Vec::new();
    }
    uax14_opportunities(text, line_break.strictness, LineBreakWordOption::Normal, locale)
        .into_iter()
        .filter(|p| !p.mandatory && p.offset < text.len())
        .map(|p| p.offset)
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phrase {
    None,
    KeepAll,
    Japanese,
}

/// How phrase breaking applies (Android's lw=phrase: Japanese and Korean only).
fn phrase_kind(line_break: LineBreak, locale: &Locale) -> Phrase {
    if line_break.word_break != WordBreak::Phrase {
        return Phrase::None;
    }
    match locale.language() {
        "ko" => Phrase::KeepAll,
        "ja" => Phrase::Japanese,
        _ => Phrase::None,
    }
}

/// icu's strictness for a LineBreak strictness in a locale.
fn icu_strictness(strictness: Strictness, locale: &Locale) -> LineBreakStrictness {
    match strictness {
        Strictness::Loose => LineBreakStrictness::Loose,
        Strictness::Normal => LineBreakStrictness::Normal,
        Strictness::Strict => LineBreakStrictness::Strict,
        Strictness::Default | Strictness::Unspecified => match locale.language() {
            "ja" | "zh" => LineBreakStrictness::Normal,
            _ => LineBreakStrictness::Strict,
        },
    }
}

/// icu's line break opportunities after minikin's tailoring.
fn uax14_opportunities(
    text: &str,
    strictness: Strictness,
    word_option: LineBreakWordOption,
    locale: &Locale,
) -> Vec<BreakOpportunity> {
    let mut options = LineBreakOptions::default();
    options.strictness = Some(icu_strictness(strictness, locale));
    options.word_option = Some(word_option);
    options.content_locale = Some(locale.language_identifier());
    let bounds: Vec<usize> = LineSegmenter::new_auto(options).segment_str(text).filter(|&b| b > 0).collect();
    let classes = CodePointMapData::<LineBreakClass>::new();
    let mandatory_after = |offset: usize| {
        text[..offset].chars().next_back().is_some_and(|c| {
            matches!(
                classes.get(c),
                LineBreakClass::MandatoryBreak
                    | LineBreakClass::CarriageReturn
                    | LineBreakClass::LineFeed
                    | LineBreakClass::NextLine
            )
        })
    };
    let mut out = Vec::with_capacity(bounds.len());
    for (offset, email_or_url, inside_email_or_url) in EmailUrlBreaks::new(text, &bounds) {
        let mandatory = mandatory_after(offset);
        if !mandatory && offset < text.len() {
            // Hyphenation breaks after a soft hyphen, drawing the hyphen; a Myanmar virama only stacks.
            let before = text[..offset].chars().next_back();
            if matches!(before, Some('\u{00AD}' | '\u{1039}')) {
                continue;
            }
        }
        out.push(BreakOpportunity { offset, mandatory, email_or_url, inside_email_or_url });
    }
    if out.last().is_none_or(|p| p.offset != text.len()) {
        out.push(BreakOpportunity {
            offset: text.len(),
            mandatory: false,
            email_or_url: false,
            inside_email_or_url: false,
        });
    }
    out
}

/// minikin's WordBreaker over icu's boundaries: from each break, a run of printable ASCII holding an `@`, or a
/// `:` followed by `//`, is an email address or a URL; it breaks only at the Chicago Manual of Style's points
/// (after `:`, `=`, `&` and `//`, before `~ . , - _ ? # % = &` and a single `/`, never after a hyphen), each of
/// them a bad break but its end.
struct EmailUrlBreaks<'a> {
    text: &'a [u8],
    bounds: &'a [usize],
    last: usize,
    scan_offset: usize,
    in_email_or_url: bool,
}

impl<'a> EmailUrlBreaks<'a> {
    fn new(text: &'a str, bounds: &'a [usize]) -> EmailUrlBreaks<'a> {
        EmailUrlBreaks { text: text.as_bytes(), bounds, last: 0, scan_offset: 0, in_email_or_url: false }
    }

    /// The first boundary after `offset`.
    fn following(&self, offset: usize) -> Option<usize> {
        let i = self.bounds.partition_point(|&b| b <= offset);
        self.bounds.get(i).copied()
    }

    fn is_boundary(&self, offset: usize) -> bool {
        offset == 0 || self.bounds.binary_search(&offset).is_ok()
    }

    /// Scans forward from the last break for an email address or URL.
    fn detect(&mut self) {
        if self.last < self.scan_offset {
            return;
        }
        #[derive(PartialEq)]
        enum State {
            Start,
            SawAt,
            SawColon,
            SawColonSlash,
            SawColonSlashSlash,
        }
        let mut state = State::Start;
        let mut i = self.last;
        while i < self.text.len() {
            let c = self.text[i];
            if !(b'!'..=b'~').contains(&c) {
                break;
            }
            state = match (state, c) {
                (State::Start, b'@') => State::SawAt,
                (State::Start, b':') => State::SawColon,
                (State::SawColon, b'/') => State::SawColonSlash,
                (State::SawColonSlash, b'/') => State::SawColonSlashSlash,
                (State::SawColon | State::SawColonSlash, _) => State::Start,
                (s, _) => s,
            };
            i += 1;
        }
        self.in_email_or_url = matches!(state, State::SawAt | State::SawColonSlashSlash);
        if self.in_email_or_url && !self.is_boundary(i) {
            // Combining marks and such ending it belong to it: on to the next boundary.
            i = self.following(i).unwrap_or(self.text.len());
        }
        self.scan_offset = i;
    }

    /// The next break inside an email address or URL.
    fn next_in_email_or_url(&self) -> usize {
        let text = self.text;
        let break_after = |c: u8| matches!(c, b':' | b'=' | b'&');
        let break_before = |c: u8| matches!(c, b'~' | b'.' | b',' | b'-' | b'_' | b'?' | b'#' | b'%' | b'=' | b'&');
        let mut last_char = text[self.last];
        let mut i = self.last + 1;
        while i < self.scan_offset {
            if break_after(last_char) || (last_char == b'/' && i >= self.last + 2 && text[i - 2] == b'/') {
                break;
            }
            let this_char = text[i];
            if last_char != b'-'
                && (break_before(this_char)
                    || (this_char == b'/' && last_char != b'/' && !(i + 1 < self.scan_offset && text[i + 1] == b'/')))
            {
                break;
            }
            last_char = this_char;
            i += 1;
        }
        i
    }
}

impl Iterator for EmailUrlBreaks<'_> {
    /// A break, whether the text before it is in an email address or URL, and whether it is inside one.
    type Item = (usize, bool, bool);
    fn next(&mut self) -> Option<(usize, bool, bool)> {
        if self.last >= self.text.len() {
            return None;
        }
        self.detect();
        let current = if self.in_email_or_url { self.next_in_email_or_url() } else { self.following(self.last)? };
        self.last = current;
        Some((current, self.in_email_or_url, self.in_email_or_url && current < self.scan_offset))
    }
}

/// Grapheme cluster boundaries (0 and the text's length included).
pub(crate) fn grapheme_boundaries(text: &str) -> Vec<usize> {
    GraphemeClusterSegmenter::new().segment_str(text).collect()
}

/// Whether a character is written in Japanese's scripts (kanji, hiragana, katakana, the prolonged sound mark).
fn is_japanese(c: char) -> bool {
    matches!(CodePointMapData::<Script>::new().get(c), Script::Han | Script::Hiragana | Script::Katakana)
        || c == '\u{30FC}'
}

/// Whether a phrase may start at `offset` in Japanese text: outside Japanese always; inside, where a word
/// begins that isn't all hiragana (particles, endings and auxiliaries stay with the word before them, as a
/// bunsetsu does).
fn japanese_phrase_break(text: &str, offset: usize, words: &[usize]) -> bool {
    let before = text[..offset].chars().next_back();
    let after = text[offset..].chars().next();
    if !before.is_some_and(is_japanese) || !after.is_some_and(is_japanese) {
        return true;
    }
    let Ok(i) = words.binary_search(&offset) else { return false };
    let end = words.get(i + 1).copied().unwrap_or(text.len());
    let hiragana = CodePointMapData::<Script>::new();
    !text[offset..end].chars().all(|c| hiragana.get(c) == Script::Hiragana || c == '\u{30FC}')
}

/// Where a word between two breaks starts and ends for hyphenation (minikin's WordBreaker wordStart and
/// wordEnd): opening punctuation and quotes before it and spaces, punctuation and controls after it left out.
pub(crate) fn hyphenation_word(text: &str, start: usize, end: usize) -> (usize, usize) {
    let classes = CodePointMapData::<LineBreakClass>::new();
    let categories = CodePointMapData::<GeneralCategory>::new();
    let mut word_start = start;
    for (i, c) in text[start..end].char_indices() {
        if !matches!(classes.get(c), LineBreakClass::OpenPunctuation | LineBreakClass::Quotation) {
            break;
        }
        word_start = start + i + c.len_utf8();
    }
    let mut word_end = end;
    for (i, c) in text[word_start..end].char_indices().rev() {
        let gc = categories.get(c);
        let skipped = matches!(
            gc,
            GeneralCategory::SpaceSeparator
                | GeneralCategory::Control
                | GeneralCategory::DashPunctuation
                | GeneralCategory::OpenPunctuation
                | GeneralCategory::ClosePunctuation
                | GeneralCategory::ConnectorPunctuation
                | GeneralCategory::OtherPunctuation
                | GeneralCategory::InitialPunctuation
                | GeneralCategory::FinalPunctuation
        );
        if !skipped {
            break;
        }
        word_end = word_start + i;
    }
    (word_start, word_end.max(word_start))
}
