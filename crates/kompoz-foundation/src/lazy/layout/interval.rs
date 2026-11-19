//! What a lazy layout's DSL builds (Compose's IntervalList.kt and LazyLayoutIntervalContent.kt): a list of
//! intervals, each a run of items sharing how their keys, content types and content are made, found by an item's
//! index with a binary search.

use super::key::{LazyKey, get_default_lazy_layout_key};
use kompoz_ui::layout::SlotKey;
use std::cell::Cell;

/// One interval of an [`IntervalList`] (Compose's `IntervalList.Interval`): its first item's index, how many items
/// it has, and what it holds for them.
pub struct Interval<T> {
    /// The index of the first item in it.
    pub start_index: i32,
    /// How many items it has (more than 0).
    pub size: i32,
    /// What represents it.
    pub value: T,
}

impl<T> Interval<T> {
    /// Panics, as Compose's precondition throws, on a negative start or an empty interval.
    pub fn new(start_index: i32, size: i32, value: T) -> Interval<T> {
        assert!(start_index >= 0, "startIndex should be >= 0");
        assert!(size > 0, "size should be > 0");
        Interval { start_index, size, value }
    }

    fn contains(&self, index: i32) -> bool {
        index >= self.start_index && index < self.start_index + self.size
    }
}

/// Intervals of items, one after the other from index 0 (Compose's IntervalList).
pub trait IntervalList<T> {
    /// The items of all its intervals.
    fn size(&self) -> i32;
    /// The interval holding item `index` (panics outside `0..size`).
    fn get(&self, index: i32) -> &Interval<T>;
    /// The intervals holding the items `from_index..=to_index`, in order.
    fn for_each(&self, from_index: i32, to_index: i32, block: &mut dyn FnMut(&Interval<T>));
}

/// An [`IntervalList`] built by adding intervals at its end (Compose's MutableIntervalList).
pub struct MutableIntervalList<T> {
    intervals: Vec<Interval<T>>,
    size: i32,
    /// The interval found last (most lookups are near each other).
    last_interval: Cell<Option<usize>>,
}

impl<T> Default for MutableIntervalList<T> {
    fn default() -> Self {
        MutableIntervalList { intervals: Vec::new(), size: 0, last_interval: Cell::new(None) }
    }
}

impl<T> MutableIntervalList<T> {
    pub fn new() -> MutableIntervalList<T> {
        MutableIntervalList::default()
    }

    /// Adds an interval of `size` items holding `value` (none for 0; panics for a negative size).
    pub fn add_interval(&mut self, size: i32, value: T) {
        assert!(size >= 0, "size should be >=0");
        if size == 0 {
            return;
        }
        self.intervals.push(Interval::new(self.size, size, value));
        self.size += size;
    }

    fn check_index_bounds(&self, index: i32) {
        if index < 0 || index >= self.size {
            panic!("Index {index}, size {}", self.size);
        }
    }

    fn interval_for_index(&self, item_index: i32) -> usize {
        if let Some(last) = self.last_interval.get()
            && self.intervals[last].contains(item_index)
        {
            return last;
        }
        let found = binary_search(&self.intervals, item_index);
        self.last_interval.set(Some(found));
        found
    }
}

impl<T> IntervalList<T> for MutableIntervalList<T> {
    fn size(&self) -> i32 {
        self.size
    }

    fn get(&self, index: i32) -> &Interval<T> {
        self.check_index_bounds(index);
        &self.intervals[self.interval_for_index(index)]
    }

    fn for_each(&self, from_index: i32, to_index: i32, block: &mut dyn FnMut(&Interval<T>)) {
        self.check_index_bounds(from_index);
        self.check_index_bounds(to_index);
        assert!(to_index >= from_index, "toIndex ({to_index}) should be not smaller than fromIndex ({from_index})");
        let mut interval_index = binary_search(&self.intervals, from_index);
        let mut item_index = self.intervals[interval_index].start_index;
        while item_index <= to_index {
            let interval = &self.intervals[interval_index];
            block(interval);
            item_index += interval.size;
            interval_index += 1;
        }
    }
}

/// The interval holding `item_index` (Compose's `MutableVector.binarySearch`).
fn binary_search<T>(intervals: &[Interval<T>], item_index: i32) -> usize {
    let mut left = 0i32;
    let mut right = intervals.len() as i32 - 1;
    while left < right {
        let middle = left + (right - left) / 2;
        let middle_value = intervals[middle as usize].start_index;
        if middle_value == item_index {
            return middle as usize;
        }
        if middle_value < item_index {
            left = middle + 1;
            // (the left one not past the value)
            if item_index < intervals[left as usize].start_index {
                return middle as usize;
            }
        } else {
            right = middle - 1;
        }
    }
    left as usize
}

/// What an interval of a lazy layout's content tells of its items (Compose's `LazyLayoutIntervalContent.Interval`):
/// their keys and content types, by their index in the interval.
pub trait LazyLayoutInterval {
    /// The key of its item `index` (local to it), none if it gives no keys (Compose's `key`, a null function).
    fn key(&self, index: i32) -> Option<LazyKey> {
        let _ = index;
        None
    }
    /// The content type of its item `index` (Compose's `type`).
    fn content_type(&self, index: i32) -> Option<SlotKey> {
        let _ = index;
        None
    }
}

/// A lazy layout's content as intervals (Compose's LazyLayoutIntervalContent): what its DSL scope builds, with
/// the keys and content types of its items by their global index.
pub trait LazyLayoutIntervalContent {
    type Interval: LazyLayoutInterval;

    fn intervals(&self) -> &MutableIntervalList<Self::Interval>;

    /// The items of all the intervals (Compose's `itemCount`).
    fn item_count(&self) -> i32 {
        self.intervals().size()
    }

    /// The key of item `index`: its interval's, or the default key of its index (Compose's `getKey`).
    fn get_key(&self, index: i32) -> LazyKey {
        let interval = self.intervals().get(index);
        interval.value.key(index - interval.start_index).unwrap_or_else(|| get_default_lazy_layout_key(index))
    }

    /// The content type of item `index` (Compose's `getContentType`).
    fn get_content_type(&self, index: i32) -> Option<SlotKey> {
        let interval = self.intervals().get(index);
        interval.value.content_type(index - interval.start_index)
    }

    /// Calls `block` with item `global_index`'s index in its interval and the interval's value (Compose's
    /// `withInterval`).
    fn with_interval<R>(&self, global_index: i32, block: impl FnOnce(i32, &Self::Interval) -> R) -> R
    where
        Self: Sized,
    {
        let interval = self.intervals().get(global_index);
        block(global_index - interval.start_index, &interval.value)
    }

    /// [`with_interval`](Self::with_interval) whose `block` suspends (Compose's inline `withInterval`: its lambda,
    /// run in place, suspends with the coroutine it is called in).
    #[doc(hidden)]
    #[allow(async_fn_in_trait)]
    async fn with_interval_async<R>(&self, global_index: i32, block: impl AsyncFnOnce(i32, &Self::Interval) -> R) -> R
    where
        Self: Sized,
    {
        let interval = self.intervals().get(global_index);
        block(global_index - interval.start_index, &interval.value).await
    }
}
