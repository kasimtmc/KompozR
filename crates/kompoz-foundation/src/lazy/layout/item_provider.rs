//! What a lazy layout's items are (Compose's LazyLayoutItemProvider.kt, LazyLayoutKeyIndexMap.kt,
//! LazyLayoutNearestRangeState.kt): how many there are, each one's key, content type and content, and the index of
//! a key — found around the items in view, so an item keeps its place when the items before it change.

use super::interval::{IntervalList, LazyLayoutInterval, LazyLayoutIntervalContent};
use super::key::{LazyKey, get_default_lazy_layout_key};
use kompoz_runtime::{MutableState, mutable_state_of};
use kompoz_ui::layout::SlotKey;
use std::collections::HashMap;
use std::ops::Range;

/// The items of a lazy layout (Compose's LazyLayoutItemProvider): their count, and each one's content, key and
/// content type. Made anew when the items change (a new one is a new set of items; the layout's items compare it
/// with the last one through [`same_items`](Self::same_items) to tell whether they compose again).
pub trait LazyLayoutItemProvider: std::any::Any {
    /// How many items there are (in view or not).
    fn item_count(&self) -> i32;

    /// Composes item `index`, whose key is `key` (Compose's `@Composable Item`): called in the item's own
    /// composition, inside its saveable state (its `remember_saveable` values kept by its key).
    fn item(&self, index: i32, key: &LazyKey);

    /// What kind of content item `index` has (Compose's `getContentType`): items of the same type reuse the
    /// compositions of items that left. None by default.
    fn get_content_type(&self, index: i32) -> Option<SlotKey> {
        let _ = index;
        None
    }

    /// Item `index`'s key: its state is kept by it, and it keeps its place by it when items before it come and go
    /// (Compose's `getKey`). Its index's [`DefaultLazyKey`](super::DefaultLazyKey) by default.
    fn get_key(&self, index: i32) -> LazyKey {
        get_default_lazy_layout_key(index)
    }

    /// The index of the item of `key`, -1 if there is none (Compose's `getIndex`; only around the items in view is
    /// enough).
    fn get_index(&self, key: &LazyKey) -> i32 {
        let _ = key;
        -1
    }

    /// Whether `other` gives the same items, so an item composed for one isn't composed again for the other
    /// (Compose's `equals` of an item provider). By default only itself does.
    fn same_items(&self, other: &dyn LazyLayoutItemProvider) -> bool {
        std::ptr::addr_eq(self as *const Self, other as *const dyn LazyLayoutItemProvider)
    }
}

/// Where the item of `key` is now (Compose's `findIndexByKey`): still at `last_known_index`, at the index the
/// provider finds, or `last_known_index` if it can't find it (or there is no key, or no item).
pub(crate) fn find_index_by_key(
    provider: &dyn LazyLayoutItemProvider,
    key: Option<&LazyKey>,
    last_known_index: i32,
) -> i32 {
    let Some(key) = key else { return last_known_index };
    if provider.item_count() == 0 {
        // (no real item during the last measure)
        return last_known_index;
    }
    if last_known_index < provider.item_count() && *key == provider.get_key(last_known_index) {
        return last_known_index;
    }
    let new_index = provider.get_index(key);
    if new_index != -1 {
        return new_index;
    }
    last_known_index
}

/// The keys and indices of the items around those in view (Compose's LazyLayoutKeyIndexMap).
pub trait LazyLayoutKeyIndexMap {
    /// The index of `key`, -1 if not found.
    fn get_index(&self, key: &LazyKey) -> i32;
    /// The key of item `index`, if known.
    fn get_key(&self, index: i32) -> Option<LazyKey>;
}

/// A key index map of the items in `nearest_range` of `content` (Compose's `LazyLayoutKeyIndexMap(range,
/// content)`).
pub fn LazyLayoutKeyIndexMap<C: LazyLayoutIntervalContent>(
    item_index_range: Range<i32>,
    interval_content: &C,
) -> NearestRangeKeyIndexMap {
    NearestRangeKeyIndexMap::new(item_index_range, interval_content)
}

/// The keys of the items in a range, by index and the other way (Compose's NearestRangeKeyIndexMap).
#[derive(Default)]
pub struct NearestRangeKeyIndexMap {
    map: HashMap<LazyKey, i32>,
    keys: Vec<LazyKey>,
    keys_start_index: i32,
}

impl NearestRangeKeyIndexMap {
    /// The keys of the items in `nearest_range` that `interval_content` has.
    pub fn new<C: LazyLayoutIntervalContent>(nearest_range: Range<i32>, interval_content: &C) -> Self {
        let list = interval_content.intervals();
        let first = nearest_range.start;
        assert!(first >= 0, "negative nearestRange.first");
        let last = (nearest_range.end - 1).min(list.size() - 1);
        if last < first {
            return NearestRangeKeyIndexMap::default();
        }
        let size = (last - first + 1) as usize;
        let mut keys = Vec::with_capacity(size);
        let mut map = HashMap::with_capacity(size);
        list.for_each(first, last, &mut |interval| {
            let start = first.max(interval.start_index);
            let end = last.min(interval.start_index + interval.size - 1);
            for i in start..=end {
                let key =
                    interval.value.key(i - interval.start_index).unwrap_or_else(|| get_default_lazy_layout_key(i));
                map.insert(key.clone(), i);
                keys.push(key);
            }
        });
        NearestRangeKeyIndexMap { map, keys, keys_start_index: first }
    }
}

impl LazyLayoutKeyIndexMap for NearestRangeKeyIndexMap {
    fn get_index(&self, key: &LazyKey) -> i32 {
        self.map.get(key).copied().unwrap_or(-1)
    }

    fn get_key(&self, index: i32) -> Option<LazyKey> {
        let i = index - self.keys_start_index;
        if i < 0 {
            return None;
        }
        self.keys.get(i as usize).cloned()
    }
}

/// The items whose keys are mapped: a window of `sliding_window_size` items around the first visible one, and
/// `extra_item_count` more each side (Compose's LazyLayoutNearestRangeState). A state: reading it recomposes as
/// the window moves.
pub(crate) struct LazyLayoutNearestRangeState {
    value: MutableState<(i32, i32)>,
    sliding_window_size: i32,
    extra_item_count: i32,
    last_first_visible_item: std::cell::Cell<i32>,
}

impl LazyLayoutNearestRangeState {
    pub(crate) fn new(first_visible_item: i32, sliding_window_size: i32, extra_item_count: i32) -> Self {
        let range = calculate_nearest_items_range(first_visible_item, sliding_window_size, extra_item_count);
        LazyLayoutNearestRangeState {
            value: mutable_state_of((range.start, range.end)),
            sliding_window_size,
            extra_item_count,
            last_first_visible_item: std::cell::Cell::new(first_visible_item),
        }
    }

    /// The range now (a state read).
    pub(crate) fn value(&self) -> Range<i32> {
        let (start, end) = self.value.get();
        start..end
    }

    pub(crate) fn update(&self, first_visible_item: i32) {
        if first_visible_item != self.last_first_visible_item.get() {
            self.last_first_visible_item.set(first_visible_item);
            let range =
                calculate_nearest_items_range(first_visible_item, self.sliding_window_size, self.extra_item_count);
            self.value.set((range.start, range.end));
        }
    }
}

/// The window of the first visible item (multiples of the window size, so it moves by whole windows), widened by
/// the extra items each side.
fn calculate_nearest_items_range(
    first_visible_item: i32,
    sliding_window_size: i32,
    extra_item_count: i32,
) -> Range<i32> {
    let sliding_window_start = sliding_window_size * (first_visible_item / sliding_window_size);
    let start = (sliding_window_start - extra_item_count).max(0);
    let end = sliding_window_start + sliding_window_size + extra_item_count;
    start..end
}
