//! How a paragraph's lines are broken (Compose's LineBreak and its Strategy, Strictness and WordBreak).

/// How a paragraph's lines are broken (Compose's LineBreak): where the breaks go (`strategy`), how strictly
/// Chinese and Japanese text keeps punctuation and small kana off a line's start (`strictness`), and whether
/// Korean and Japanese break between words or phrases only (`word_break`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct LineBreak {
    pub strategy: Strategy,
    pub strictness: Strictness,
    pub word_break: WordBreak,
}

impl LineBreak {
    /// Lines filled in turn, normal strictness: what text input uses (Compose's LineBreak.Simple).
    pub const Simple: LineBreak = LineBreak::new(Strategy::Simple, Strictness::Normal, WordBreak::Default);
    /// Lines as even as they can be, loose, by phrases: titles (Compose's LineBreak.Heading).
    pub const Heading: LineBreak = LineBreak::new(Strategy::Balanced, Strictness::Loose, WordBreak::Phrase);
    /// The least uneven room at the lines' ends, strict: long text (Compose's LineBreak.Paragraph).
    pub const Paragraph: LineBreak = LineBreak::new(Strategy::HighQuality, Strictness::Strict, WordBreak::Default);
    /// Takes the style around it, and in the end `Simple`.
    pub const Unspecified: LineBreak =
        LineBreak::new(Strategy::Unspecified, Strictness::Unspecified, WordBreak::Unspecified);

    pub const fn new(strategy: Strategy, strictness: Strictness, word_break: WordBreak) -> LineBreak {
        LineBreak { strategy, strictness, word_break }
    }
    pub fn is_specified(self) -> bool {
        self != LineBreak::Unspecified
    }
    /// This, or `block`'s where unspecified.
    pub fn take_or_else(self, block: impl FnOnce() -> LineBreak) -> LineBreak {
        if self.is_specified() { self } else { block() }
    }
}

/// Where a paragraph's lines break (Compose's LineBreak.Strategy): `Simple` fills each line in turn;
/// `HighQuality` leaves the least uneven room at the lines' ends (the last line aside); `Balanced` makes the lines
/// as even as it can without adding one.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Strategy {
    #[default]
    Unspecified,
    Simple,
    HighQuality,
    Balanced,
}

/// How strictly a line may start with punctuation or small kana (Compose's LineBreak.Strictness, CSS's
/// line-break): the locale's default, loose, normal or strict.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Strictness {
    #[default]
    Unspecified,
    Default,
    Loose,
    Normal,
    Strict,
}

/// Where Korean and Japanese text breaks (Compose's LineBreak.WordBreak): anywhere the rules allow, or between
/// phrases only.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum WordBreak {
    #[default]
    Unspecified,
    Default,
    Phrase,
}
