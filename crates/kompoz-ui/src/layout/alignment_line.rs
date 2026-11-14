//! Alignment lines: positions a layout tells its parent about (Compose's AlignmentLine), and the two baselines.

use std::fmt;
use std::hash::{Hash, Hasher};

/// A line a layout provides and its parent can align by: a horizontal one (a y, as the baselines) or a vertical
/// one (an x). Lines of a layout's children pass up to it, moved by where they were placed; where several give
/// the same line, `merger` picks the value (Compose's AlignmentLine). Known by its name: give each its own.
#[derive(Clone, Copy)]
pub struct AlignmentLine {
    name: &'static str,
    horizontal: bool,
    merger: fn(i32, i32) -> i32,
}

impl AlignmentLine {
    /// Where a line isn't (Compose's `AlignmentLine.Unspecified`).
    pub const Unspecified: i32 = i32::MIN;

    pub const fn new_horizontal(name: &'static str, merger: fn(i32, i32) -> i32) -> AlignmentLine {
        AlignmentLine { name, horizontal: true, merger }
    }
    pub const fn new_vertical(name: &'static str, merger: fn(i32, i32) -> i32) -> AlignmentLine {
        AlignmentLine { name, horizontal: false, merger }
    }
    pub fn name(&self) -> &'static str {
        self.name
    }
    /// Whether it is a horizontal line (a y position).
    pub fn is_horizontal(&self) -> bool {
        self.horizontal
    }
    /// The value of the line where two children give `a` and `b`.
    pub fn merge(&self, a: i32, b: i32) -> i32 {
        (self.merger)(a, b)
    }
}

impl PartialEq for AlignmentLine {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.horizontal == other.horizontal
    }
}
impl Eq for AlignmentLine {}

impl Hash for AlignmentLine {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.horizontal.hash(state);
    }
}

impl fmt::Debug for AlignmentLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// A horizontal alignment line: a y position (Compose's HorizontalAlignmentLine).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct HorizontalAlignmentLine(pub AlignmentLine);

impl HorizontalAlignmentLine {
    pub const fn new(name: &'static str, merger: fn(i32, i32) -> i32) -> HorizontalAlignmentLine {
        HorizontalAlignmentLine(AlignmentLine::new_horizontal(name, merger))
    }
}

/// A vertical alignment line: an x position (Compose's VerticalAlignmentLine).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct VerticalAlignmentLine(pub AlignmentLine);

impl VerticalAlignmentLine {
    pub const fn new(name: &'static str, merger: fn(i32, i32) -> i32) -> VerticalAlignmentLine {
        VerticalAlignmentLine(AlignmentLine::new_vertical(name, merger))
    }
}

impl From<HorizontalAlignmentLine> for AlignmentLine {
    fn from(l: HorizontalAlignmentLine) -> AlignmentLine {
        l.0
    }
}
impl From<VerticalAlignmentLine> for AlignmentLine {
    fn from(l: VerticalAlignmentLine) -> AlignmentLine {
        l.0
    }
}

fn min(a: i32, b: i32) -> i32 {
    a.min(b)
}
fn max(a: i32, b: i32) -> i32 {
    a.max(b)
}

/// The baseline of a layout's first line of text: the topmost of its children's (Compose's FirstBaseline).
pub const FirstBaseline: HorizontalAlignmentLine = HorizontalAlignmentLine::new("FirstBaseline", min);

/// The baseline of its last line: the bottommost (Compose's LastBaseline).
pub const LastBaseline: HorizontalAlignmentLine = HorizontalAlignmentLine::new("LastBaseline", max);
