//! Text segmentation, in UTF-8 byte offsets: grapheme clusters (what a caret steps over and a font draws whole —
//! an emoji ZWJ sequence, a flag, a keycap, a letter and its marks), words and sentences (ICU's UAX #29, tailored
//! by the content's locale), the Unicode emoji properties, the script a character is written in, and Compose's
//! StringHelpers (`findFollowingBreak`, `findPrecedingBreak`, paragraph bounds, the code point or emoji before an
//! offset). Line breaking (UAX #14) is the line breaker's.

use crate::intl::Locale;
use crate::range::TextRange;
use icu_locale_core::LanguageIdentifier;
use icu_properties::props::{
    BidiClass, Emoji, EmojiComponent, EmojiModifier, EmojiModifierBase, EmojiPresentation, ExtendedPictographic,
    RegionalIndicator, Script, VariationSelector,
};
use icu_properties::{CodePointMapData, CodePointSetData, PropertyNamesShort};
use icu_segmenter::options::{SentenceBreakOptions, WordBreakOptions, WordType};
use icu_segmenter::{GraphemeClusterSegmenter, SentenceSegmenter, WordSegmenter};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

// ---- Grapheme clusters ----------------------------------------------------------------------------------------

/// The grapheme cluster boundaries of `text` (UAX #29's extended grapheme clusters): 0, each boundary, the
/// length (none but 0 for empty text).
pub fn grapheme_breaks(text: &str) -> impl Iterator<Item = usize> + '_ {
    GraphemeClusterSegmenter::new().segment_str(text)
}

/// The grapheme clusters of `text`, as ranges.
pub fn graphemes(text: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    let mut breaks = grapheme_breaks(text).peekable();
    std::iter::from_fn(move || {
        let start = breaks.next()?;
        let end = *breaks.peek()?;
        Some(start..end)
    })
}

/// Where segmenting may start for an offset: the start of its paragraph (a line feed always ends a cluster, so a
/// boundary is never decided by what comes before it).
fn safe_start(text: &str, index: usize) -> usize {
    find_paragraph_start(text, index)
}

/// Whether a grapheme cluster starts (or the text ends) at `index`.
pub fn is_grapheme_boundary(text: &str, index: usize) -> bool {
    if index == 0 || index >= text.len() {
        return index <= text.len();
    }
    let from = safe_start(text, index);
    grapheme_breaks(&text[from..]).map(|b| b + from).take_while(|&b| b <= index).any(|b| b == index)
}

/// The end of the grapheme cluster that starts at `index`, or holds it (the C++ Kompoz's `utf8::nextCluster`);
/// the text's length at its end.
pub fn next_grapheme(text: &str, index: usize) -> usize {
    find_following_break(text, index).unwrap_or(text.len())
}

/// The start of the grapheme cluster that ends at `index`, or holds it (the C++ Kompoz's
/// `utf8::previousCluster`); 0 at the text's start.
pub fn previous_grapheme(text: &str, index: usize) -> usize {
    find_preceding_break(text, index).unwrap_or(0)
}

// ---- Compose's StringHelpers ----------------------------------------------------------------------------------

/// The first grapheme cluster boundary after `index` (Compose's String.findFollowingBreak, a character
/// BreakIterator's `following`): None at or past the end (BreakIterator.DONE).
pub fn find_following_break(text: &str, index: usize) -> Option<usize> {
    if index >= text.len() {
        return None;
    }
    let from = safe_start(text, index);
    grapheme_breaks(&text[from..]).map(|b| b + from).find(|&b| b > index)
}

/// The last grapheme cluster boundary before `index` (Compose's String.findPrecedingBreak, a character
/// BreakIterator's `preceding`): None at the start (BreakIterator.DONE).
pub fn find_preceding_break(text: &str, index: usize) -> Option<usize> {
    if index == 0 {
        return None;
    }
    let index = index.min(text.len());
    // The cluster before a paragraph's start ends at it: look from the previous paragraph's start then.
    let from = safe_start(text, index - 1);
    grapheme_breaks(&text[from..]).map(|b| b + from).take_while(|&b| b < index).last()
}

/// Where the paragraph holding `index` starts: just after the line feed before it, else 0 (Compose's
/// CharSequence.findParagraphStart).
pub fn find_paragraph_start(text: &str, index: usize) -> usize {
    let index = index.min(text.len());
    text.as_bytes()[..index].iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1)
}

/// Where the paragraph holding `index` ends: at the line feed after it, else the text's end (Compose's
/// CharSequence.findParagraphEnd).
pub fn find_paragraph_end(text: &str, index: usize) -> usize {
    let index = index.min(text.len());
    text.as_bytes()[index..].iter().position(|&b| b == b'\n').map_or(text.len(), |i| index + i)
}

/// The paragraph holding `index`, its line feed left out (Compose's CharSequence.getParagraphBoundary).
pub fn paragraph_boundary(text: &str, index: usize) -> TextRange {
    TextRange::new(find_paragraph_start(text, index), find_paragraph_end(text, index))
}

/// Where the code point before `index` starts, or the emoji sequence it ends if it ends one (Compose's
/// String.findCodePointOrEmojiStartBefore, EmojiCompat's emoji start as a grapheme cluster's): None at the
/// start.
pub fn find_code_point_or_emoji_start_before(text: &str, index: usize) -> Option<usize> {
    let (at, _) = code_point_before(text, index)?;
    let cluster_start = previous_grapheme(text, index);
    let cluster = &text[cluster_start..index];
    if cluster_start < at && is_emoji_cluster(cluster) { Some(cluster_start) } else { Some(at) }
}

/// The code point starting at `index` and where it starts (None at the end or off a character's start).
pub fn code_point_at(text: &str, index: usize) -> Option<(usize, char)> {
    text.get(index..)?.chars().next().map(|c| (index, c))
}

/// The code point ending at `index` and where it starts (None at the start or off a character's start).
pub fn code_point_before(text: &str, index: usize) -> Option<(usize, char)> {
    let c = text.get(..index)?.chars().next_back()?;
    Some((index - c.len_utf8(), c))
}

/// How many code points `text[range]` has.
pub fn code_point_count(text: &str, range: Range<usize>) -> usize {
    text[range].chars().count()
}

// ---- Words and sentences --------------------------------------------------------------------------------------

/// What a word segment is (ICU's WordType): letters (CJK ideographs too), a number, or something else (spaces,
/// punctuation, symbols).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum WordKind {
    None,
    Number,
    Letter,
}

impl WordKind {
    /// Letters or a number.
    pub fn is_word_like(self) -> bool {
        self != WordKind::None
    }
}

/// A segment between two word boundaries.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WordSegment {
    pub range: Range<usize>,
    pub kind: WordKind,
}

thread_local! {
    /// Word and sentence segmenters by content locale (the tailorings ICU has for some), made once.
    static WORD_SEGMENTERS: RefCell<HashMap<Option<LanguageIdentifier>, Rc<WordSegmenter>>> =
        RefCell::new(HashMap::new());
    static SENTENCE_SEGMENTERS: RefCell<HashMap<Option<LanguageIdentifier>, Rc<SentenceSegmenter>>> =
        RefCell::new(HashMap::new());
}

fn word_segmenter(locale: Option<&Locale>) -> Rc<WordSegmenter> {
    let id = locale.map(|l| l.language_identifier().clone());
    WORD_SEGMENTERS.with(|s| {
        s.borrow_mut()
            .entry(id.clone())
            .or_insert_with(|| {
                let mut options = WordBreakOptions::default();
                options.content_locale = id.as_ref();
                // A locale ICU has no data for segments as the root does.
                let segmenter = WordSegmenter::try_new_auto(options)
                    .unwrap_or_else(|_| WordSegmenter::new_auto(Default::default()).static_to_owned());
                Rc::new(segmenter)
            })
            .clone()
    })
}

fn sentence_segmenter(locale: Option<&Locale>) -> Rc<SentenceSegmenter> {
    let id = locale.map(|l| l.language_identifier().clone());
    SENTENCE_SEGMENTERS.with(|s| {
        s.borrow_mut()
            .entry(id.clone())
            .or_insert_with(|| {
                let mut options = SentenceBreakOptions::default();
                options.content_locale = id.as_ref();
                let segmenter = SentenceSegmenter::try_new(options)
                    .unwrap_or_else(|_| SentenceSegmenter::new(Default::default()).static_to_owned());
                Rc::new(segmenter)
            })
            .clone()
    })
}

/// The word boundaries of `text` (UAX #29, with dictionaries and models for Thai, Lao, Khmer, Burmese and
/// Chinese and Japanese words), tailored for `locale`: 0, each boundary, the length.
pub fn word_breaks(text: &str, locale: Option<&Locale>) -> Vec<usize> {
    word_segmenter(locale).as_borrowed().segment_str(text).collect()
}

/// The segments between `text`'s word boundaries and what each is.
pub fn words(text: &str, locale: Option<&Locale>) -> Vec<WordSegment> {
    let segmenter = word_segmenter(locale);
    let mut out = Vec::new();
    let mut start = None;
    for (at, kind) in segmenter.as_borrowed().segment_str(text).iter_with_word_type() {
        if let Some(from) = start {
            let kind = match kind {
                WordType::Number => WordKind::Number,
                WordType::Letter => WordKind::Letter,
                _ => WordKind::None,
            };
            out.push(WordSegment { range: from..at, kind });
        }
        start = Some(at);
    }
    out
}

/// The sentence boundaries of `text` (UAX #29), tailored for `locale` (Greek's question mark): 0, each boundary,
/// the length.
pub fn sentence_breaks(text: &str, locale: Option<&Locale>) -> Vec<usize> {
    sentence_segmenter(locale).as_borrowed().segment_str(text).collect()
}

// ---- Emoji ----------------------------------------------------------------------------------------------------

/// Unicode's Emoji property: characters that are emoji (some only with U+FE0F after them, as ❤).
pub fn is_emoji(c: char) -> bool {
    CodePointSetData::new::<Emoji>().contains(c)
}

/// Unicode's Emoji_Presentation: emoji shown as emoji without U+FE0F after them (👍, ⌚, the regional
/// indicators), not ❤ or ☺, which are text unless asked for.
pub fn is_emoji_presentation(c: char) -> bool {
    CodePointSetData::new::<EmojiPresentation>().contains(c)
}

/// Unicode's Extended_Pictographic (what emoji ZWJ sequences are made of, assigned or reserved for emoji).
pub fn is_extended_pictographic(c: char) -> bool {
    CodePointSetData::new::<ExtendedPictographic>().contains(c)
}

/// Unicode's Regional_Indicator (two make a flag).
pub fn is_regional_indicator(c: char) -> bool {
    CodePointSetData::new::<RegionalIndicator>().contains(c)
}

/// Unicode's Emoji_Modifier: the five skin tones.
pub fn is_emoji_modifier(c: char) -> bool {
    CodePointSetData::new::<EmojiModifier>().contains(c)
}

/// Unicode's Emoji_Modifier_Base: what a skin tone may follow.
pub fn is_emoji_modifier_base(c: char) -> bool {
    CodePointSetData::new::<EmojiModifierBase>().contains(c)
}

/// Unicode's Emoji_Component: what emoji sequences are made with (digits, #, *, ZWJ, U+20E3, U+FE0F, the
/// regional indicators, skin tones, hair styles, tags).
pub fn is_emoji_component(c: char) -> bool {
    CodePointSetData::new::<EmojiComponent>().contains(c)
}

/// Unicode's Variation_Selector.
pub fn is_variation_selector(c: char) -> bool {
    CodePointSetData::new::<VariationSelector>().contains(c)
}

/// VARIATION SELECTOR-15: show the character before it as text.
pub const TEXT_PRESENTATION_SELECTOR: char = '\u{FE0E}';
/// VARIATION SELECTOR-16: show the character before it as emoji.
pub const EMOJI_PRESENTATION_SELECTOR: char = '\u{FE0F}';
/// ZERO WIDTH JOINER: joins emoji into one (a family).
pub const ZERO_WIDTH_JOINER: char = '\u{200D}';
/// COMBINING ENCLOSING KEYCAP.
pub const COMBINING_ENCLOSING_KEYCAP: char = '\u{20E3}';

/// How a grapheme cluster asks to be shown: as emoji (in a color emoji font), as text, or as its first
/// character's default says.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Presentation {
    Emoji,
    Text,
}

/// Whether a cluster is an emoji sequence (the C++ Kompoz's isEmojiSequence): more than one character, joined by
/// a ZWJ, a flag's regional indicators, a skin tone, a keycap or tags.
pub fn is_emoji_sequence(cluster: &str) -> bool {
    let mut count = 0;
    let mut joined = false;
    for c in cluster.chars() {
        count += 1;
        joined = joined
            || c == ZERO_WIDTH_JOINER
            || is_regional_indicator(c)
            || is_emoji_modifier(c)
            || c == COMBINING_ENCLOSING_KEYCAP
            || ('\u{E0020}'..='\u{E007F}').contains(&c);
    }
    count > 1 && joined
}

/// How a grapheme cluster is shown (the C++ Kompoz's faceFor rule, minikin's): as emoji if it is an emoji
/// sequence, has U+FE0F, or starts with an Emoji_Presentation character — unless U+FE0E asks for text; else as
/// text.
pub fn presentation(cluster: &str) -> Presentation {
    if cluster.contains(TEXT_PRESENTATION_SELECTOR) {
        return Presentation::Text;
    }
    let first = cluster.chars().next();
    if is_emoji_sequence(cluster)
        || cluster.contains(EMOJI_PRESENTATION_SELECTOR)
        || first.is_some_and(is_emoji_presentation)
    {
        Presentation::Emoji
    } else {
        Presentation::Text
    }
}

/// Whether a grapheme cluster is an emoji (shown as one: [`presentation`] says so).
pub fn is_emoji_cluster(cluster: &str) -> bool {
    presentation(cluster) == Presentation::Emoji
}

// ---- Scripts and directions ------------------------------------------------------------------------------------

/// The ISO 15924 code of the script `c` is written in (`Latn`, `Arab`, `Deva`), `Zyyy` for characters of every
/// script (spaces, digits, punctuation, emoji), `Zinh` for those taking the script before them (combining marks),
/// `Zzzz` for unassigned ones.
pub fn script_tag(c: char) -> [u8; 4] {
    let script = CodePointMapData::<Script>::new().get(c);
    PropertyNamesShort::<Script>::new().get(script).and_then(|name| name.as_bytes().try_into().ok()).unwrap_or(*b"Zzzz")
}

/// Whether `c` belongs to no one script (Common or Inherited: spaces, digits, punctuation, marks).
pub fn is_common_or_inherited(c: char) -> bool {
    matches!(CodePointMapData::<Script>::new().get(c), Script::Common | Script::Inherited)
}

/// The script of a stretch of text: its first character's that has one, `Zyyy` if none has.
pub fn script_of(text: &str) -> [u8; 4] {
    text.chars().map(script_tag).find(|t| t != b"Zyyy" && t != b"Zinh").unwrap_or(*b"Zyyy")
}

/// Whether `c` is neutral as to direction or a European digit (bidi class neither L, R, AL nor AN, nor an
/// explicit embedding, override or isolate): what keeps to the font of the text around it.
pub fn is_direction_neutral(c: char) -> bool {
    !matches!(
        CodePointMapData::<BidiClass>::new().get(c),
        BidiClass::LeftToRight
            | BidiClass::RightToLeft
            | BidiClass::ArabicLetter
            | BidiClass::ArabicNumber
            | BidiClass::LeftToRightEmbedding
            | BidiClass::LeftToRightOverride
            | BidiClass::RightToLeftEmbedding
            | BidiClass::RightToLeftOverride
            | BidiClass::PopDirectionalFormat
            | BidiClass::LeftToRightIsolate
            | BidiClass::RightToLeftIsolate
            | BidiClass::FirstStrongIsolate
            | BidiClass::PopDirectionalIsolate
    )
}
