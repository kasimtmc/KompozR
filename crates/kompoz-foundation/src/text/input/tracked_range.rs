//! A style tracked by a [`TextFieldBuffer`](super::TextFieldBuffer) (Compose's TrackedRange) and how its range
//! takes in text inserted at its ends (Compose's ExpandPolicy).

use std::marker::PhantomData;

/// A style added to a [`TextFieldBuffer`](super::TextFieldBuffer) with `add_style` (Compose's TrackedRange): a
/// handle to it, its properties read and changed through the buffer (`text_range`, `expand_policy`, `is_valid`,
/// `span_style`, `paragraph_style` and their setters), which follow the text's edits. A style whose range is
/// deleted away is removed (no longer valid). Its life is the buffer's: don't keep it.
pub struct TrackedRange<T> {
    pub(crate) creator_id: u64,
    pub(crate) entry_id: u32,
    _style: PhantomData<fn() -> T>,
}

impl<T> TrackedRange<T> {
    pub(crate) fn new(creator_id: u64, entry_id: u32) -> TrackedRange<T> {
        TrackedRange { creator_id, entry_id, _style: PhantomData }
    }
}

impl<T> Clone for TrackedRange<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for TrackedRange<T> {}

impl<T> PartialEq for TrackedRange<T> {
    fn eq(&self, other: &Self) -> bool {
        self.creator_id == other.creator_id && self.entry_id == other.entry_id
    }
}

impl<T> Eq for TrackedRange<T> {}

impl<T> std::fmt::Debug for TrackedRange<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "TrackedRange({}:{})", self.creator_id, self.entry_id)
    }
}

/// Whether a [`TrackedRange`] takes in text inserted exactly at its start, its end, both or neither (Compose's
/// ExpandPolicy).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ExpandPolicy {
    start_expands: bool,
    end_expands: bool,
}

impl ExpandPolicy {
    /// Text inserted at its start or end goes outside it.
    pub const InsideOnly: ExpandPolicy = ExpandPolicy { start_expands: false, end_expands: false };
    /// Text inserted at its start is taken in.
    pub const AtStart: ExpandPolicy = ExpandPolicy { start_expands: true, end_expands: false };
    /// Text inserted at its end is taken in.
    pub const AtEnd: ExpandPolicy = ExpandPolicy { start_expands: false, end_expands: true };
    /// Text inserted at either end is taken in.
    pub const AtBoth: ExpandPolicy = ExpandPolicy { start_expands: true, end_expands: true };

    pub(crate) fn new(start_expands: bool, end_expands: bool) -> ExpandPolicy {
        ExpandPolicy { start_expands, end_expands }
    }

    pub(crate) fn start_expands(&self) -> bool {
        self.start_expands
    }

    pub(crate) fn end_expands(&self) -> bool {
        self.end_expands
    }
}
