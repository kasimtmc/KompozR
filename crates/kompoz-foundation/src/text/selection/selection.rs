//! A selection across selectables (Compose's Selection.kt).

use kompoz_text::{ResolvedTextDirection, TextRange};

/// One end of a selection (Compose's Selection.AnchorInfo): the selectable it is in, its offset there (a UTF-8
/// byte offset into the selectable's text) and the direction of the text at it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AnchorInfo {
    /// The direction of the text at the edge of the selection.
    pub direction: ResolvedTextDirection,
    /// The offset of the edge in its selectable's text.
    pub offset: usize,
    /// The id of the [`Selectable`](super::Selectable) it is in.
    pub selectable_id: u64,
}

impl AnchorInfo {
    pub fn new(direction: ResolvedTextDirection, offset: usize, selectable_id: u64) -> AnchorInfo {
        AnchorInfo { direction, offset, selectable_id }
    }
}

/// A selection (Compose's Selection): from its start to its end, each in a selectable of a selection container,
/// and whether the handles were dragged across each other (the start after the end; within one selectable that is
/// `start.offset > end.offset`, across several it is worked out once and kept here).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Selection {
    pub start: AnchorInfo,
    pub end: AnchorInfo,
    pub handles_crossed: bool,
}

impl Selection {
    pub fn new(start: AnchorInfo, end: AnchorInfo, handles_crossed: bool) -> Selection {
        Selection { start, end, handles_crossed }
    }

    /// This selection followed by `other` (Compose's `merge`): from this start to the other's end, or, crossed,
    /// from the other's start to this end.
    pub fn merge(&self, other: Option<&Selection>) -> Selection {
        let Some(other) = other else { return *self };
        if self.handles_crossed || other.handles_crossed {
            Selection {
                start: if other.handles_crossed { other.start } else { other.end },
                end: if self.handles_crossed { self.end } else { self.start },
                handles_crossed: true,
            }
        } else {
            Selection { end: other.end, ..*self }
        }
    }

    /// Its offsets as a range (Compose's `toTextRange`): meaningful within one selectable.
    pub fn to_text_range(&self) -> TextRange {
        TextRange::new(self.start.offset, self.end.offset)
    }
}

/// `lhs` merged with `rhs`, either may be none (Compose's `merge(lhs, rhs)`).
pub fn merge(lhs: Option<&Selection>, rhs: Option<&Selection>) -> Option<Selection> {
    match lhs {
        Some(l) => Some(l.merge(rhs)),
        None => rhs.copied(),
    }
}
