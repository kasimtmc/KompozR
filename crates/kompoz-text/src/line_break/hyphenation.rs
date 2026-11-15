//! The hyphenation hook (minikin's Hyphenator, HyphenationType and hyphen edits): what breaking a word at a
//! hyphenation point does to the two lines. KompozR ships no hyphenation patterns, so `Hyphens.Auto` breaks
//! nothing more unless a [`Hyphenator`] is given (Android's patterns would plug in here).

use crate::intl::Locale;

/// What breaking a word at one of its points does (minikin's HyphenationType).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum HyphenationType {
    /// Not a hyphenation point.
    DontBreak,
    /// A hyphen added at the end of the line.
    BreakAndInsertHyphen,
    /// An Armenian hyphen (U+058A) added at the end of the line.
    BreakAndInsertArmenianHyphen,
    /// A maqaf (U+05BE) added at the end of the line.
    BreakAndInsertMaqaf,
    /// A Canadian syllabics hyphen (U+1400) added at the end of the line.
    BreakAndInsertUcasHyphen,
    /// Broken as it is (after a hard hyphen; scripts that don't hyphenate with a mark).
    BreakAndDontInsertHyphen,
    /// The character before the break drawn as a hyphen (Catalan's l·l becoming l- l).
    BreakAndReplaceWithHyphen,
    /// Broken as it is, and the next line starts with a hyphen (Polish and Portuguese repeat a hard hyphen).
    BreakAndInsertHyphenAtNextLine,
    /// A zero-width joiner and a hyphen at the end of the line and a joiner at the start of the next (Arabic
    /// letters that join across the break).
    BreakAndInsertHyphenAndZwj,
}

impl HyphenationType {
    /// What the line ending at this point gets (minikin's editForThisLine).
    pub fn end_edit(self) -> EndHyphenEdit {
        match self {
            HyphenationType::DontBreak
            | HyphenationType::BreakAndDontInsertHyphen
            | HyphenationType::BreakAndInsertHyphenAtNextLine => EndHyphenEdit::NoEdit,
            HyphenationType::BreakAndReplaceWithHyphen => EndHyphenEdit::ReplaceWithHyphen,
            HyphenationType::BreakAndInsertArmenianHyphen => EndHyphenEdit::InsertArmenianHyphen,
            HyphenationType::BreakAndInsertMaqaf => EndHyphenEdit::InsertMaqaf,
            HyphenationType::BreakAndInsertUcasHyphen => EndHyphenEdit::InsertUcasHyphen,
            HyphenationType::BreakAndInsertHyphenAndZwj => EndHyphenEdit::InsertZwjAndHyphen,
            HyphenationType::BreakAndInsertHyphen => EndHyphenEdit::InsertHyphen,
        }
    }

    /// What the line starting at this point gets (minikin's editForNextLine).
    pub fn start_edit(self) -> StartHyphenEdit {
        match self {
            HyphenationType::BreakAndInsertHyphenAtNextLine => StartHyphenEdit::InsertHyphen,
            HyphenationType::BreakAndInsertHyphenAndZwj => StartHyphenEdit::InsertZwj,
            _ => StartHyphenEdit::NoEdit,
        }
    }
}

/// What a line ending at a hyphenation point gets at its end (minikin's EndHyphenEdit).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum EndHyphenEdit {
    #[default]
    NoEdit,
    /// Its last character drawn as a hyphen.
    ReplaceWithHyphen,
    InsertHyphen,
    InsertArmenianHyphen,
    InsertMaqaf,
    InsertUcasHyphen,
    InsertZwjAndHyphen,
}

impl EndHyphenEdit {
    /// What is drawn after the line's last character (in place of it for `ReplaceWithHyphen`): U+2010 HYPHEN for
    /// a hyphen, which a face without it draws as U+002D (minikin's choice).
    pub fn text(self) -> &'static str {
        match self {
            EndHyphenEdit::NoEdit => "",
            EndHyphenEdit::ReplaceWithHyphen | EndHyphenEdit::InsertHyphen => "\u{2010}",
            EndHyphenEdit::InsertArmenianHyphen => "\u{058A}",
            EndHyphenEdit::InsertMaqaf => "\u{05BE}",
            EndHyphenEdit::InsertUcasHyphen => "\u{1400}",
            EndHyphenEdit::InsertZwjAndHyphen => "\u{200D}\u{2010}",
        }
    }
}

/// What a line starting at a hyphenation point gets at its start (minikin's StartHyphenEdit).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum StartHyphenEdit {
    #[default]
    NoEdit,
    InsertHyphen,
    InsertZwj,
}

impl StartHyphenEdit {
    /// What is drawn before the line's first character.
    pub fn text(self) -> &'static str {
        match self {
            StartHyphenEdit::NoEdit => "",
            StartHyphenEdit::InsertHyphen => "\u{2010}",
            StartHyphenEdit::InsertZwj => "\u{200D}",
        }
    }
}

/// Finds where words may be hyphenated (minikin's Hyphenator): what `Hyphens.Auto` breaks words with. Line
/// breaking asks it for each word (its leading opening punctuation and quotes and its trailing spaces and
/// punctuation left out, as minikin's WordBreaker does; words in email addresses and URLs, and words over 45
/// characters, aren't hyphenated) and tries its points when the word doesn't fit.
pub trait Hyphenator {
    /// The points inside `word` where it may break, byte offsets (between 0 and its length, on character
    /// boundaries) with what breaking there does; `DontBreak` points are left out or ignored.
    fn hyphenate(&self, word: &str, locale: &Locale) -> Vec<(usize, HyphenationType)>;
}
