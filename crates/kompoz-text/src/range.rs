//! A range of a text (Compose's TextRange): selections, compositions, annotations' extents.

use kompoz_runtime::KotlinProperties;
use std::fmt;

/// A range of a text, `start` to `end` (Compose's TextRange): UTF-8 byte offsets on character boundaries, as every
/// offset in KompozR's text is (`&text[range.min()..range.max()]` is what it covers). `start` may be past `end`: a
/// selection made backwards.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, KotlinProperties)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

/// Compose's `TextRange(start, end)` and `TextRange(index)` (a caret): Kotlin's arguments, one as itself, two as a
/// tuple ([`TextRangeArgs`]); plain code calls [`TextRange::new`] and [`TextRange::at`].
#[allow(non_snake_case)]
pub fn TextRange(args: impl TextRangeArgs) -> TextRange {
    args.text_range()
}

/// What [`TextRange()`](fn@TextRange) is given, each overload by its types.
pub trait TextRangeArgs {
    fn text_range(self) -> TextRange;
}

/// Kotlin: `TextRange(index: Int)`
impl TextRangeArgs for usize {
    fn text_range(self) -> TextRange {
        TextRange::at(self)
    }
}

/// Kotlin: `TextRange(start: Int, end: Int)`
impl TextRangeArgs for (usize, usize) {
    fn text_range(self) -> TextRange {
        TextRange::new(self.0, self.1)
    }
}

/// Kotlin's Int (what its lengths and indices are): not negative (Compose's "start and end cannot be negative").
fn offset_of(value: i32) -> usize {
    assert!(value >= 0, "start and end cannot be negative. [start: {value}]");
    value as usize
}

/// Kotlin's `TextRange(index: Int)` given an Int.
impl TextRangeArgs for i32 {
    fn text_range(self) -> TextRange {
        TextRange::at(offset_of(self))
    }
}

/// Kotlin's `TextRange(start: Int, end: Int)` given Ints.
impl TextRangeArgs for (i32, i32) {
    fn text_range(self) -> TextRange {
        TextRange::new(offset_of(self.0), offset_of(self.1))
    }
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

/// Compose's `CharSequence.substring(range: TextRange)` (androidx.compose.ui.text) on a text: what `range` covers of
/// it (`textFieldState.text.substring(selection)`).
pub trait StringTextRangeExt {
    /// Kotlin: `substring(range: TextRange): String`
    fn substring_of_range(&self, range: TextRange) -> String;
}

impl<S: AsRef<str> + ?Sized> StringTextRangeExt for S {
    fn substring_of_range(&self, range: TextRange) -> String {
        range.substring(self.as_ref()).to_string()
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
