//! Which lane each item of a staggered grid went to (Compose's LazyStaggeredGridLaneInfo.kt): a window of item
//! indices (moved with the scroll; at most 2^17 wide) to each item's lane — unset, a lane, or the full line — and
//! the gaps above full-line items, so items go back to the lanes they were in when scrolled back to.

use std::collections::VecDeque;

/// An item whose lane isn't known (Compose's `LaneUnset`).
pub(crate) const LaneUnset: i32 = -1;
/// An item taking the full line (Compose's `LaneFullSpan`).
pub(crate) const LaneFullSpan: i32 = -2;

/// The most items a window holds (2^17, the power of 2 closest to 100 000).
const MaxCapacity: i32 = 131_072;

/// A full-line item and the gaps above it in each lane.
struct SpannedItem {
    index: i32,
    gaps: Vec<i32>,
}

/// Compose's LazyStaggeredGridLaneInfo.
pub(crate) struct LazyStaggeredGridLaneInfo {
    anchor: i32,
    /// Each item's lane plus one (0: unset).
    lanes: Vec<i32>,
    spanned_items: VecDeque<SpannedItem>,
}

impl Default for LazyStaggeredGridLaneInfo {
    fn default() -> Self {
        LazyStaggeredGridLaneInfo { anchor: 0, lanes: vec![0; 16], spanned_items: VecDeque::new() }
    }
}

impl LazyStaggeredGridLaneInfo {
    pub(crate) fn set_lane(&mut self, item_index: i32, lane: i32) {
        assert!(item_index >= 0, "Negative lanes are not supported");
        self.ensure_valid_index(item_index);
        self.lanes[(item_index - self.anchor) as usize] = lane + 1;
    }

    pub(crate) fn get_lane(&self, item_index: i32) -> i32 {
        if item_index < self.lower_bound() || item_index >= self.upper_bound() {
            return LaneUnset;
        }
        self.lanes[(item_index - self.anchor) as usize] - 1
    }

    /// Whether the item can be in `target_lane`: it is, or its lane isn't known, or it takes the full line.
    pub(crate) fn assigned_to_lane(&self, item_index: i32, target_lane: i32) -> bool {
        let lane = self.get_lane(item_index);
        lane == target_lane || lane == LaneUnset || lane == LaneFullSpan
    }

    pub(crate) fn upper_bound(&self) -> i32 {
        self.anchor + self.lanes.len() as i32
    }

    pub(crate) fn lower_bound(&self) -> i32 {
        self.anchor
    }

    pub(crate) fn reset(&mut self) {
        self.lanes.fill(0);
        self.spanned_items.clear();
    }

    /// The item before `item_index` that can be in `target_lane` (-1: none).
    pub(crate) fn find_previous_item_index(&self, item_index: i32, target_lane: i32) -> i32 {
        for i in (0..item_index).rev() {
            if self.assigned_to_lane(i, target_lane) {
                return i;
            }
        }
        -1
    }

    /// The item after `item_index` that can be in `target_lane` (the window's end: none known).
    pub(crate) fn find_next_item_index(&self, item_index: i32, target_lane: i32) -> i32 {
        for i in item_index + 1..self.upper_bound() {
            if self.assigned_to_lane(i, target_lane) {
                return i;
            }
        }
        self.upper_bound()
    }

    /// Makes the window hold `requested_index` (Compose's `ensureValidIndex`): grown, or moved so the index is in
    /// its middle; the full-line items out of it forgotten.
    pub(crate) fn ensure_valid_index(&mut self, requested_index: i32) {
        let requested_capacity = requested_index - self.anchor;
        if (0..MaxCapacity).contains(&requested_capacity) {
            // (the simplest: grown to it)
            self.ensure_capacity(requested_capacity + 1, 0);
        } else {
            // Moved: the requested index in the middle.
            let old_anchor = self.anchor;
            self.anchor = (requested_index - (self.lanes.len() as i32 / 2)).max(0);
            let mut delta = self.anchor - old_anchor;
            let size = self.lanes.len() as i32;
            if delta >= 0 {
                // (what is kept moved down, the rest unset)
                if delta < size {
                    self.lanes.copy_within(delta as usize..size as usize, 0);
                }
                let from = (size - delta).max(0) as usize;
                self.lanes[from..].fill(0);
            } else {
                delta = -delta;
                if size + delta < MaxCapacity {
                    // (grown, the room at the start)
                    self.ensure_capacity(size + delta + 1, delta);
                } else {
                    // (what fits moved up, the start unset)
                    if delta < size {
                        self.lanes.copy_within(0..(size - delta) as usize, delta as usize);
                    }
                    let to = size.min(delta) as usize;
                    self.lanes[..to].fill(0);
                }
            }
        }
        // (the full-line items out of the window forgotten)
        while self.spanned_items.front().is_some_and(|i| i.index < self.lower_bound()) {
            self.spanned_items.pop_front();
        }
        while self.spanned_items.back().is_some_and(|i| i.index > self.upper_bound()) {
            self.spanned_items.pop_back();
        }
    }

    pub(crate) fn set_gaps(&mut self, item_index: i32, gaps: Option<Vec<i32>>) {
        match (self.search_by_index(item_index), gaps) {
            (Err(_), None) => {}
            (Err(insertion_index), Some(gaps)) => {
                self.spanned_items.insert(insertion_index, SpannedItem { index: item_index, gaps })
            }
            (Ok(found), None) => {
                self.spanned_items.remove(found);
            }
            (Ok(found), Some(gaps)) => self.spanned_items[found].gaps = gaps,
        }
    }

    pub(crate) fn get_gaps(&self, item_index: i32) -> Option<Vec<i32>> {
        self.search_by_index(item_index).ok().map(|found| self.spanned_items[found].gaps.clone())
    }

    /// Grows the window to `capacity` (doubling), what it held moved by `new_offset`.
    fn ensure_capacity(&mut self, capacity: i32, new_offset: i32) {
        assert!(
            capacity <= MaxCapacity,
            "Requested item capacity {capacity} is larger than max supported: {MaxCapacity}!"
        );
        if (self.lanes.len() as i32) < capacity {
            let mut new_size = self.lanes.len() as i32;
            while new_size < capacity {
                new_size *= 2;
            }
            let mut lanes = vec![0; new_size as usize];
            let offset = new_offset as usize;
            // (Kotlin's copyInto: as much as fits)
            let count = self.lanes.len().min(lanes.len() - offset);
            lanes[offset..offset + count].copy_from_slice(&self.lanes[..count]);
            self.lanes = lanes;
        }
    }

    /// The full-line item of `index` (its place), or where it would go.
    fn search_by_index(&self, index: i32) -> Result<usize, usize> {
        self.spanned_items.binary_search_by(|i| i.index.cmp(&index))
    }
}
