//! A range of a text (Compose's TextRange): selections, compositions, annotations' extents.

use std::fmt;

/// A range of a text, `start` to `end` (Compose's TextRange): UTF-8 byte offsets on character boundaries, as every
/// offset in KompozR's text is (`&text[range.min()..range.max()]` is what it covers). `start` may be past `end`: a
/// selection made backwards.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

impl TextRange {
    /// A caret at 0.
    pub const Zero: TextRange = TextRange { start: 0, end: 0 };

    pub const fn new(start: usize, end: usize) -> TextRange {
        TextRange { start, end }
    }
    /// A caret at `index` (Compose's TextRange(index)).
    pub const fn at(index: usize) -> TextRange {
        TextRange { start: index, end: index }
    }
    /// The smaller end.
    pub fn min(&self) -> usize {
        self.start.min(self.end)
    }
    /// The larger end.
    pub fn max(&self) -> usize {
        self.start.max(self.end)
    }
    /// A caret: no text in it.
    pub fn collapsed(&self) -> bool {
        self.start == self.end
    }
    /// Made backwards: its start past its end.
    pub fn reversed(&self) -> bool {
        self.start > self.end
    }
    /// How many bytes it covers.
    pub fn length(&self) -> usize {
        self.max() - self.min()
    }
    /// Whether the two share any text (touching ends don't).
    pub fn intersects(&self, other: TextRange) -> bool {
        self.min() < other.max() && other.min() < self.max()
    }
    /// Whether `other` is all inside this (Compose's `contains(other)`).
    pub fn contains(&self, other: TextRange) -> bool {
        self.min() <= other.min() && other.max() <= self.max()
    }
    /// Whether `offset` is in it (its end not: Compose's `contains(offset)`).
    pub fn contains_offset(&self, offset: usize) -> bool {
        (self.min()..self.max()).contains(&offset)
    }
    /// Both ends kept within `minimum..=maximum` (Compose's coerceIn).
    pub fn coerce_in(&self, minimum: usize, maximum: usize) -> TextRange {
        TextRange::new(self.start.clamp(minimum, maximum), self.end.clamp(minimum, maximum))
    }
    /// What it covers of `text` (Compose's `CharSequence.substring(range)`).
    pub fn substring<'a>(&self, text: &'a str) -> &'a str {
        &text[self.min()..self.max()]
    }
}

/// `TextRange(start, end)`, as Compose writes it.
impl fmt::Debug for TextRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TextRange({}, {})", self.start, self.end)
    }
}

/// `start..end`.
impl From<std::ops::Range<usize>> for TextRange {
    fn from(r: std::ops::Range<usize>) -> TextRange {
        TextRange::new(r.start, r.end)
    }
}
