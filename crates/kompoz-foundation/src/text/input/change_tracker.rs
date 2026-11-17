//! The changes made to a text buffer (Compose's `ChangeTracker`, and `TextFieldBuffer.ChangeList` with its
//! `forEachChange` extensions).

use kompoz_text::TextRange;

/// The changes made to a [`TextFieldBuffer`](super::TextFieldBuffer) (Compose's `TextFieldBuffer.ChangeList`):
/// non-overlapping ranges in order, each with the range of the original text it replaced.
pub trait ChangeList {
    /// How many changes there are.
    fn change_count(&self) -> usize;

    /// The range of the text now that change `change_index` covers.
    fn get_range(&self, change_index: usize) -> TextRange;

    /// The range of the original text that change `change_index` replaced.
    fn get_original_range(&self, change_index: usize) -> TextRange;

    /// Each change in order, with the range it covers and what it replaced (Compose's `forEachChange`); the count
    /// read again each time, as changes may be made meanwhile.
    fn for_each_change(&self, mut block: impl FnMut(TextRange, TextRange))
    where
        Self: Sized,
    {
        let mut i = 0;
        while i < self.change_count() {
            block(self.get_range(i), self.get_original_range(i));
            i += 1;
        }
    }

    /// Each change from the last (Compose's `forEachChangeReversed`): changing the text of one doesn't move those
    /// still to come.
    fn for_each_change_reversed(&self, mut block: impl FnMut(TextRange, TextRange))
    where
        Self: Sized,
    {
        let mut i = self.change_count();
        while i > 0 {
            i -= 1;
            block(self.get_range(i), self.get_original_range(i));
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Change {
    pre_start: usize,
    pre_end: usize,
    original_start: usize,
    original_end: usize,
    is_from_hardware_source: bool,
}

/// Keeps track of the changes made to a text buffer, merging adjacent and overlapping ones (Compose's
/// ChangeTracker).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ChangeTracker {
    changes: Vec<Change>,
}

impl ChangeTracker {
    /// No changes.
    pub fn new() -> ChangeTracker {
        ChangeTracker::default()
    }

    /// The text from `pre_start` to `pre_end` (of the text as it is, all changes so far made) replaced by
    /// `post_length` bytes: recorded in the original text's terms, merged with the changes it touches, those
    /// after it moved (Compose's `trackChange`).
    pub fn track_change(
        &mut self,
        pre_start: usize,
        pre_end: usize,
        post_length: usize,
        is_from_hardware_source: bool,
    ) {
        if pre_start == pre_end && post_length == 0 {
            return;
        }
        let pre_min = pre_start.min(pre_end);
        let pre_max = pre_start.max(pre_end);
        let post_delta = post_length as isize - (pre_max - pre_min) as isize;
        let shift = |v: usize| (v as isize + post_delta) as usize;

        let mut temp: Vec<Change> = Vec::with_capacity(self.changes.len() + 1);
        let mut recorded = false;
        let mut merged: Option<Change> = None;
        for change in std::mem::take(&mut self.changes) {
            let within = |v: usize, lo: usize, hi: usize| lo <= v && v <= hi;
            if within(change.pre_start, pre_min, pre_max)
                || within(change.pre_end, pre_min, pre_max)
                || within(pre_min, change.pre_start, change.pre_end)
                || within(pre_max, change.pre_start, change.pre_end)
            {
                match &mut merged {
                    None => merged = Some(change),
                    Some(m) => {
                        m.pre_end = change.pre_end;
                        m.original_end = change.original_end;
                        m.is_from_hardware_source |= change.is_from_hardware_source;
                    }
                }
                continue;
            }
            if change.pre_start > pre_max && !recorded {
                Self::append_new_change(
                    &mut temp,
                    merged.take(),
                    pre_min,
                    pre_max,
                    post_delta,
                    is_from_hardware_source,
                );
                recorded = true;
            }
            let mut change = change;
            if recorded {
                change.pre_start = shift(change.pre_start);
                change.pre_end = shift(change.pre_end);
            }
            temp.push(change);
        }
        if !recorded {
            Self::append_new_change(&mut temp, merged.take(), pre_min, pre_max, post_delta, is_from_hardware_source);
        }
        self.changes = temp;
    }

    fn append_new_change(
        temp: &mut Vec<Change>,
        merged: Option<Change>,
        pre_min: usize,
        pre_max: usize,
        post_delta: isize,
        is_from_hardware_source: bool,
    ) {
        let mut original_delta = temp.last().map_or(0, |c| c.pre_end as isize - c.original_end as isize);
        let change = match merged {
            None => {
                let original_start = (pre_min as isize - original_delta) as usize;
                let original_end = original_start + (pre_max - pre_min);
                Change {
                    pre_start: pre_min,
                    pre_end: (pre_max as isize + post_delta) as usize,
                    original_start,
                    original_end,
                    is_from_hardware_source,
                }
            }
            Some(mut change) => {
                change.is_from_hardware_source |= is_from_hardware_source;
                if change.pre_start > pre_min {
                    change.pre_start = pre_min;
                    change.original_start = pre_min;
                }
                if pre_max > change.pre_end {
                    original_delta = change.pre_end as isize - change.original_end as isize;
                    change.pre_end = pre_max;
                    change.original_end = (pre_max as isize - original_delta) as usize;
                }
                change.pre_end = (change.pre_end as isize + post_delta) as usize;
                change
            }
        };
        temp.push(change);
    }

    /// Forgets the changes.
    pub fn clear_changes(&mut self) {
        self.changes.clear();
    }

    /// Whether change `change_index` came from a physical keyboard.
    pub fn is_from_hardware_source(&self, change_index: usize) -> bool {
        self.changes[change_index].is_from_hardware_source
    }
}

impl ChangeList for ChangeTracker {
    fn change_count(&self) -> usize {
        self.changes.len()
    }

    fn get_range(&self, change_index: usize) -> TextRange {
        let c = &self.changes[change_index];
        TextRange::new(c.pre_start, c.pre_end)
    }

    fn get_original_range(&self, change_index: usize) -> TextRange {
        let c = &self.changes[change_index];
        TextRange::new(c.original_start, c.original_end)
    }
}

/// `ChangeList(changes=[(0,1)->(0,2), …])`, as Compose's.
impl std::fmt::Debug for ChangeTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("ChangeList(changes=[")?;
        for (i, c) in self.changes.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "({},{})->({},{})", c.original_start, c.original_end, c.pre_start, c.pre_end)?;
        }
        f.write_str("])")
    }
}
