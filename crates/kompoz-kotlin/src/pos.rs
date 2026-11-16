//! Positions in the Kotlin text and the Rust spans they stand for.
//!
//! A position is a byte offset into the text the front end was given plus the line and column it is at in the
//! file the text came from: `kompoz!`'s text keeps the file's lines and columns (the macro rebuilds the gaps
//! between tokens from their line and column), `kompoz_file!`'s is the `.kt` file itself. Columns count
//! characters (Unicode scalar values), as rustc's spans do — not bytes, not Kotlin's UTF-16 units — so a Kotlin
//! line:column and the Rust error at the same token agree.

use proc_macro2::Span;

/// A place in the text: its byte offset, and the 1-based line and column (in characters) it is at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pos {
    pub offset: usize,
    pub line: u32,
    pub column: u32,
}

impl Pos {
    /// The start of a text whose first character is at `line`:`column` of its file.
    pub const fn start(line: u32, column: u32) -> Pos {
        Pos { offset: 0, line, column }
    }

    /// The position after `text` (which starts here).
    pub fn advanced_by(self, text: &str) -> Pos {
        let mut p = self;
        for c in text.chars() {
            p = p.advanced_over(c);
        }
        p
    }

    /// The position after the character `c` (which is here).
    pub fn advanced_over(self, c: char) -> Pos {
        let offset = self.offset + c.len_utf8();
        if c == '\n' {
            Pos { offset, line: self.line + 1, column: 1 }
        } else {
            Pos { offset, line: self.line, column: self.column + 1 }
        }
    }
}

impl std::fmt::Display for Pos {
    /// `line:column`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// A stretch of the text: from `start` to just past its end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Loc {
    pub start: Pos,
    pub end: Pos,
}

impl Loc {
    pub const fn new(start: Pos, end: Pos) -> Loc {
        Loc { start, end }
    }

    /// An empty stretch at `at`.
    pub const fn at(at: Pos) -> Loc {
        Loc { start: at, end: at }
    }

    /// From the start of `self` to the end of `other` (which comes after it).
    pub fn to(self, other: Loc) -> Loc {
        Loc { start: self.start, end: other.end }
    }

    /// Its text in `text` (the text positions are offsets into).
    pub fn text(self, text: &str) -> &str {
        &text[self.start.offset..self.end.offset]
    }
}

/// Where in the Rust source an offset of the Kotlin text is: the span each emitted token gets (so rustc's errors,
/// and `#[track_caller]`'s identity of a call, land on Kotlin lines) and where the front end's own diagnostics
/// are reported.
pub trait SpanMap {
    /// The span of the token the offset is in (its own span: the hygiene the user's token has), or of the
    /// nearest token when the offset is between tokens.
    fn span_at(&self, offset: usize) -> Span;

    /// The span for a stretch: its start's (stable Rust can't join spans).
    fn span_of(&self, loc: Loc) -> Span {
        self.span_at(loc.start.offset)
    }
}

/// Every offset at one span: a `.kt` file read by `kompoz_file!` (its tokens have no spans of their own; the
/// diagnostics name the file's line and column in their message), and tests.
#[derive(Clone, Copy, Debug)]
pub struct OneSpan(pub Span);

impl SpanMap for OneSpan {
    fn span_at(&self, _offset: usize) -> Span {
        self.0
    }
}

impl Default for OneSpan {
    fn default() -> Self {
        OneSpan(Span::call_site())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_count_characters() {
        let p = Pos::start(3, 5).advanced_by("ğü😀\nx");
        assert_eq!(p, Pos { offset: 2 + 2 + 4 + 1 + 1, line: 4, column: 2 });
        let loc = Loc::new(Pos::start(1, 1), Pos::start(1, 1).advanced_by("ğü"));
        assert_eq!(loc.text("ğüş"), "ğü");
    }
}
