//! Which way a selection extends across selectables (Compose's SelectionMode.kt).

use kompoz_graphics::{Offset, Rect};
use std::cmp::Ordering;

/// Which way a selection dragged across selectables extends (Compose's SelectionMode): by rows (the end dragged
/// down selects the rows above first) or by columns.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SelectionMode {
    Vertical,
    Horizontal,
}

impl SelectionMode {
    /// Where a handle at `position` is for a selectable of `bounds` (Compose's `compare`): on it (Equal), before
    /// its start (Less: the selectable's start is where it is) or after its end (Greater).
    pub fn compare(self, position: Offset, bounds: Rect) -> Ordering {
        if contains_inclusive(bounds, position) {
            return Ordering::Equal;
        }
        match self {
            SelectionMode::Vertical => {
                // Above it, or at its left and not below it: its start.
                if position.y < bounds.top || (position.x < bounds.left && position.y < bounds.bottom) {
                    return Ordering::Less;
                }
            }
            SelectionMode::Horizontal => {
                // At its left, or above it and not at its right: its start.
                if position.x < bounds.left || (position.y < bounds.top && position.x < bounds.right) {
                    return Ordering::Less;
                }
            }
        }
        Ordering::Greater
    }

    /// Whether a selection from `start` to `end` (in the container) takes in a selectable of `bounds` (Compose's
    /// `isSelected`): either end is on it, or they are on its two sides.
    pub fn is_selected(self, bounds: Rect, start: Offset, end: Offset) -> bool {
        if contains_inclusive(bounds, start) || contains_inclusive(bounds, end) {
            return true;
        }
        (self.compare(start, bounds) == Ordering::Greater) ^ (self.compare(end, bounds) == Ordering::Greater)
    }
}

/// Whether `offset` is in `rect`, its right and bottom edges included (Compose's `Rect.containsInclusive`): a
/// handle put at a selectable's right edge is on it.
pub(crate) fn contains_inclusive(rect: Rect, offset: Offset) -> bool {
    (rect.left..=rect.right).contains(&offset.x) && (rect.top..=rect.bottom).contains(&offset.y)
}
