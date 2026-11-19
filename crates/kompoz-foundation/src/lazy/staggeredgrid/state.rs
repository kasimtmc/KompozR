//! A lazy staggered grid's state (Compose's LazyStaggeredGridState.kt, LazyStaggeredGridScrollPosition.kt,
//! LazyStaggeredGridScrollScope.kt): each lane's first visible item and how far it is scrolled out of view (the
//! smallest of them the state's first visible item), kept by the first item's key as items change; which lane
//! each item went to; what its last layout showed; scrolling (a `ScrollableState` whose deltas measure the grid
//! again at once), scrolling to an item at once or animated; prefetching the next item of each lane as it scrolls.

use super::grid::LazyStaggeredGridItemProvider;
use super::lane_info::{LaneFullSpan, LaneUnset, LazyStaggeredGridLaneInfo};
use super::layout_info::LazyStaggeredGridLayoutInfo;
use super::measure::{LazyStaggeredGridMeasureResult, LazyStaggeredGridMeasuredItem, fast_round_to_int};
use crate::gestures::{
    DefaultScrollableState, LocalFuture, Orientation, ScrollBlock, ScrollIndicatorState, ScrollScope, ScrollableState,
};
use crate::interaction::{InteractionSource, MutableInteractionSource};
use crate::lazy::layout::{
    AwaitFirstLayoutModifier, LazyKey, LazyLayoutBeyondBoundsInfo, LazyLayoutItemAnimator, LazyLayoutItemProvider,
    LazyLayoutMeasuredItem, LazyLayoutNearestRangeState, LazyLayoutPinnedItemList, LazyLayoutPrefetchState,
    LazyLayoutScrollDeltaBetweenPasses, LazyLayoutScrollScope, LazyLayoutScrollScopeState, ObservableScopeInvalidator,
    PrefetchHandle, Remeasurement, animate_scroll_to_item, find_index_by_key,
};
use kompoz_animation::MutatePriority;
use kompoz_graphics::Constraints;
use kompoz_runtime::{
    MutableState, SavedValue, Saver, kompozable, mutable_state_of, named, remember_saveable_with,
    without_read_observation,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::future::Future;
use std::rc::{Rc, Weak};

// ---------------------------------------------------------------------------------------------------------------
// The scroll position

/// The items whose keys are known around the first visible one: a window of 90, and 200 more each side.
const NearestItemsSlidingWindowSize: i32 = 90;
const NearestItemsExtraItemCount: i32 = 200;

/// The indices of each lane's first item, from one item (Compose's `fillIndices`: the state's `fillNearestIndices`).
type FillIndices = Box<dyn Fn(i32, usize) -> Vec<i32>>;

/// Where a staggered grid is scrolled to (Compose's LazyStaggeredGridScrollPosition): each lane's first visible
/// item and its offset, the smallest of them (states: what readers see), the first item's key.
pub(crate) struct LazyStaggeredGridScrollPosition {
    indices: RefCell<Vec<i32>>,
    index: MutableState<i32>,
    scroll_offsets: RefCell<Vec<i32>>,
    scroll_offset: MutableState<i32>,
    had_first_not_empty_layout: Cell<bool>,
    last_known_first_item_key: RefCell<Option<LazyKey>>,
    nearest_range_state: LazyLayoutNearestRangeState,
    fill_indices: FillIndices,
}

/// The first visible item: the smallest index of the lanes' (0 when a lane is empty or at the start).
fn calculate_first_visible_index(indices: &[i32]) -> i32 {
    let mut min_index = i32::MAX;
    for &index in indices {
        // (-1: a lane with no items, more lanes than items: the grid is at item 0)
        if index <= 0 {
            return 0;
        }
        min_index = min_index.min(index);
    }
    if min_index == i32::MAX { 0 } else { min_index }
}

/// The first visible item's offset: the smallest of the lanes where it is first.
fn calculate_first_visible_scroll_offset(indices: &[i32], offsets: &[i32]) -> i32 {
    let mut min_offset = i32::MAX;
    let smallest_index = calculate_first_visible_index(indices);
    for lane in 0..offsets.len() {
        if indices[lane] == smallest_index {
            min_offset = min_offset.min(offsets[lane]);
        }
    }
    if min_offset == i32::MAX { 0 } else { min_offset }
}

impl LazyStaggeredGridScrollPosition {
    fn new(initial_indices: Vec<i32>, initial_offsets: Vec<i32>, fill_indices: FillIndices) -> Self {
        let index = calculate_first_visible_index(&initial_indices);
        let offset = calculate_first_visible_scroll_offset(&initial_indices, &initial_offsets);
        let nearest = initial_indices.iter().copied().min().unwrap_or(0);
        LazyStaggeredGridScrollPosition {
            indices: RefCell::new(initial_indices),
            index: mutable_state_of(index),
            scroll_offsets: RefCell::new(initial_offsets),
            scroll_offset: mutable_state_of(offset),
            had_first_not_empty_layout: Cell::new(false),
            last_known_first_item_key: RefCell::new(None),
            nearest_range_state: LazyLayoutNearestRangeState::new(
                nearest,
                NearestItemsSlidingWindowSize,
                NearestItemsExtraItemCount,
            ),
            fill_indices,
        }
    }

    pub(crate) fn indices(&self) -> Vec<i32> {
        self.indices.borrow().clone()
    }

    pub(crate) fn scroll_offsets(&self) -> Vec<i32> {
        self.scroll_offsets.borrow().clone()
    }

    /// From a measure (Compose's `updateFromMeasureResult`).
    fn update_from_measure_result(&self, result: &LazyStaggeredGridMeasureResult) {
        let first_visible_index = calculate_first_visible_index(&result.first_visible_item_indices);
        *self.last_known_first_item_key.borrow_mut() =
            result.visible_items_info.iter().find(|i| i.index() == first_visible_index).map(|i| i.key_ref().clone());
        self.nearest_range_state.update(first_visible_index);
        if self.had_first_not_empty_layout.get() || result.total_items_count > 0 {
            self.had_first_not_empty_layout.set(true);
            without_read_observation(|| {
                self.update(result.first_visible_item_indices.clone(), result.first_visible_item_scroll_offsets.clone())
            });
        }
    }

    fn update_scroll_offset(&self, scroll_offsets: Vec<i32>) {
        let offset = calculate_first_visible_scroll_offset(&self.indices.borrow(), &scroll_offsets);
        *self.scroll_offsets.borrow_mut() = scroll_offsets;
        self.scroll_offset.set(offset);
    }

    /// A direct request (Compose's `requestPositionAndForgetLastKnownKey`): the lanes filled around the item.
    fn request_position_and_forget_last_known_key(&self, index: i32, scroll_offset: i32) {
        let lane_count = self.indices.borrow().len();
        let new_indices = (self.fill_indices)(index, lane_count);
        let new_offsets = vec![scroll_offset; new_indices.len()];
        self.update(new_indices, new_offsets);
        self.nearest_range_state.update(index);
        *self.last_known_first_item_key.borrow_mut() = None;
    }

    /// The first item found again by its key (Compose's `updateScrollPositionIfTheFirstItemWasMoved`).
    fn update_scroll_position_if_the_first_item_was_moved(
        &self,
        item_provider: &dyn LazyLayoutItemProvider,
        indices: &[i32],
    ) -> Vec<i32> {
        let key = self.last_known_first_item_key.borrow().clone();
        let new_index = find_index_by_key(item_provider, key.as_ref(), indices.first().copied().unwrap_or(0));
        if !indices.contains(&new_index) {
            self.nearest_range_state.update(new_index);
            let new_indices = without_read_observation(|| (self.fill_indices)(new_index, indices.len()));
            *self.indices.borrow_mut() = new_indices.clone();
            self.index.set(calculate_first_visible_index(&new_indices));
            new_indices
        } else {
            indices.to_vec()
        }
    }

    fn update(&self, indices: Vec<i32>, offsets: Vec<i32>) {
        self.index.set(calculate_first_visible_index(&indices));
        self.scroll_offset.set(calculate_first_visible_scroll_offset(&indices, &offsets));
        *self.indices.borrow_mut() = indices;
        *self.scroll_offsets.borrow_mut() = offsets;
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The state

/// A lazy staggered grid's state (Compose's LazyStaggeredGridState): where it is scrolled to, what its last layout
/// showed, scrolling it. A [`ScrollableState`]: its deltas measure the grid again at once; `scroll_to_item`,
/// `animate_scroll_to_item`, `request_scroll_to_item`. Remembered and saved by
/// [`remember_lazy_staggered_grid_state`]. Clones are the same state; two are equal when they are.
#[derive(Clone)]
pub struct LazyStaggeredGridState {
    pub(crate) inner: Rc<LazyStaggeredGridStateInner>,
}

pub(crate) struct LazyStaggeredGridStateInner {
    /// Whether it was measured in a lookahead pass (Compose's `hasLookaheadOccurred`): in a lookahead scope.
    pub(crate) has_lookahead_occurred: Cell<bool>,
    /// The last approach pass's result, in a lookahead scope (Compose's `approachLayoutInfo`).
    pub(crate) approach_layout_info: RefCell<Option<Rc<LazyStaggeredGridMeasureResult>>>,
    /// What the approach scrolls by besides the lookahead's scroll (Compose's `_lazyLayoutScrollDeltaBetweenPasses`).
    scroll_delta_between_passes: LazyLayoutScrollDeltaBetweenPasses,
    pub(crate) scroll_position: LazyStaggeredGridScrollPosition,
    layout_info: RefCell<Rc<LazyStaggeredGridMeasureResult>>,
    layout_info_generation: MutableState<u64>,
    measured: Cell<bool>,
    pub(crate) lane_info: RefCell<LazyStaggeredGridLaneInfo>,
    can_scroll_forward: MutableState<bool>,
    can_scroll_backward: MutableState<bool>,
    pub(crate) remeasurement: Remeasurement,
    pub(crate) await_layout_modifier: AwaitFirstLayoutModifier,
    pub(crate) beyond_bounds_info: Rc<LazyLayoutBeyondBoundsInfo>,
    pub(crate) prefetching_enabled: Cell<bool>,
    pub(crate) prefetch_state: LazyLayoutPrefetchState,
    scrollable_state: RefCell<Option<DefaultScrollableState>>,
    scroll_to_be_consumed: Cell<f32>,
    pub(crate) measure_pass_count: Cell<i32>,
    prefetch_base_index: Cell<i32>,
    current_item_prefetch_handles: RefCell<HashMap<i32, PrefetchHandle>>,
    pub(crate) mutable_interaction_source: MutableInteractionSource,
    pub(crate) pinned_items: LazyLayoutPinnedItemList,
    pub(crate) item_animator: Rc<LazyLayoutItemAnimator<LazyStaggeredGridMeasuredItem>>,
    pub(crate) placement_scope_invalidator: ObservableScopeInvalidator,
    pub(crate) measurement_scope_invalidator: ObservableScopeInvalidator,
}

impl PartialEq for LazyStaggeredGridState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for LazyStaggeredGridState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "LazyStaggeredGridState(firstVisibleItemIndex={}, firstVisibleItemScrollOffset={})",
            self.inner.scroll_position.index.peek(),
            self.inner.scroll_position.scroll_offset.peek()
        )
    }
}

impl Default for LazyStaggeredGridState {
    fn default() -> Self {
        LazyStaggeredGridState::new(0, 0)
    }
}

/// A state scrolled to item `initial_first_visible_item_index`, `initial_first_visible_item_offset` px into it
/// (Compose's `LazyStaggeredGridState(initialFirstVisibleItemIndex, initialFirstVisibleItemOffset)`).
/// Kotlin: `LazyStaggeredGridState(initialFirstVisibleItemIndex: Int = 0, initialFirstVisibleItemOffset: Int = 0)`
pub fn LazyStaggeredGridState(
    initial_first_visible_item_index: i32,
    initial_first_visible_item_offset: i32,
) -> LazyStaggeredGridState {
    LazyStaggeredGridState::new(initial_first_visible_item_index, initial_first_visible_item_offset)
}

impl LazyStaggeredGridState {
    /// A state scrolled to item `initial_first_visible_item_index`, `initial_first_visible_item_offset` px into it.
    pub fn new(
        initial_first_visible_item_index: i32,
        initial_first_visible_item_offset: i32,
    ) -> LazyStaggeredGridState {
        LazyStaggeredGridState::with_lanes(
            vec![initial_first_visible_item_index],
            vec![initial_first_visible_item_offset],
        )
    }

    /// A state of each lane's first item and its offset (Compose's internal constructor).
    fn with_lanes(initial_first_visible_items: Vec<i32>, initial_first_visible_offsets: Vec<i32>) -> Self {
        let inner = Rc::new_cyclic(|me: &Weak<LazyStaggeredGridStateInner>| {
            let weak = me.clone();
            let fill: FillIndices = Box::new(move |item_index, lane_count| match weak.upgrade() {
                Some(inner) => fill_nearest_indices(&inner, item_index, lane_count),
                None => vec![item_index; lane_count],
            });
            LazyStaggeredGridStateInner {
                has_lookahead_occurred: Cell::new(false),
                approach_layout_info: RefCell::new(None),
                scroll_delta_between_passes: LazyLayoutScrollDeltaBetweenPasses::new(),
                scroll_position: LazyStaggeredGridScrollPosition::new(
                    initial_first_visible_items,
                    initial_first_visible_offsets,
                    fill,
                ),
                layout_info: RefCell::new(Rc::new(LazyStaggeredGridMeasureResult::empty())),
                layout_info_generation: mutable_state_of(0),
                measured: Cell::new(false),
                lane_info: RefCell::new(LazyStaggeredGridLaneInfo::default()),
                can_scroll_forward: mutable_state_of(false),
                can_scroll_backward: mutable_state_of(false),
                remeasurement: Remeasurement::default(),
                await_layout_modifier: AwaitFirstLayoutModifier::default(),
                beyond_bounds_info: Rc::new(LazyLayoutBeyondBoundsInfo::default()),
                prefetching_enabled: Cell::new(true),
                // (no nested prefetch of its own: Compose's `LazyLayoutPrefetchState(prefetchScheduler)`)
                prefetch_state: LazyLayoutPrefetchState::new(None),
                scrollable_state: RefCell::new(None),
                scroll_to_be_consumed: Cell::new(0.0),
                measure_pass_count: Cell::new(0),
                prefetch_base_index: Cell::new(-1),
                current_item_prefetch_handles: RefCell::new(HashMap::new()),
                mutable_interaction_source: MutableInteractionSource::new(),
                pinned_items: LazyLayoutPinnedItemList::new(),
                item_animator: Rc::new(LazyLayoutItemAnimator::default()),
                placement_scope_invalidator: ObservableScopeInvalidator::default(),
                measurement_scope_invalidator: ObservableScopeInvalidator::default(),
            }
        });
        let weak = Rc::downgrade(&inner);
        *inner.scrollable_state.borrow_mut() =
            Some(crate::gestures::ScrollableState(move |delta| match weak.upgrade() {
                Some(inner) => -LazyStaggeredGridState { inner }.on_scroll(-delta),
                None => 0.0,
            }));
        LazyStaggeredGridState { inner }
    }

    fn scrollable_state(&self) -> DefaultScrollableState {
        self.inner.scrollable_state.borrow().clone().expect("its scrollable state")
    }

    /// The first item in view: the smallest index of the lanes' first ones (a state; Compose's
    /// `firstVisibleItemIndex`).
    pub fn first_visible_item_index(&self) -> i32 {
        self.inner.scroll_position.index.get()
    }

    /// How far it is scrolled out of view, px (a state; Compose's `firstVisibleItemScrollOffset`).
    pub fn first_visible_item_scroll_offset(&self) -> i32 {
        self.inner.scroll_position.scroll_offset.get()
    }

    /// What the last layout showed (Compose's `layoutInfo`; a state).
    pub fn layout_info(&self) -> LazyStaggeredGridLayoutInfo {
        self.inner.layout_info_generation.get();
        self.inner.layout_info.borrow().layout_info()
    }

    fn measure_result(&self) -> Rc<LazyStaggeredGridMeasureResult> {
        self.inner.layout_info_generation.get();
        self.inner.layout_info.borrow().clone()
    }

    fn set_measure_result(&self, result: Rc<LazyStaggeredGridMeasureResult>) {
        *self.inner.layout_info.borrow_mut() = result;
        self.inner.measured.set(true);
        self.inner.layout_info_generation.set(self.inner.layout_info_generation.peek().wrapping_add(1));
    }

    /// The drags of the grid (Compose's `interactionSource`).
    pub fn interaction_source(&self) -> InteractionSource {
        self.inner.mutable_interaction_source.as_interaction_source()
    }

    /// The lanes of the last measure (Compose's `laneCount`).
    pub(crate) fn lane_count(&self) -> i32 {
        self.measure_result().slots.sizes.len() as i32
    }

    /// The items whose keys are known (a state).
    pub(crate) fn nearest_range(&self) -> std::ops::Range<i32> {
        self.inner.scroll_position.nearest_range_state.value()
    }

    /// The scroll a measure applies (Compose's `scrollToBeConsumed(isLookingAhead)`): the lookahead pass's (or
    /// outside a lookahead scope) the scroll; the approach's after a lookahead, what it scrolled back of that.
    pub(crate) fn scroll_to_be_consumed(&self, is_looking_ahead: bool) -> f32 {
        if is_looking_ahead || !self.inner.has_lookahead_occurred.get() {
            self.inner.scroll_to_be_consumed.get()
        } else {
            self.inner.scroll_delta_between_passes.scroll_delta_between_passes()
        }
    }

    /// Takes a scroll of `distance` (Compose's `onScroll`).
    fn on_scroll(&self, distance: f32) -> f32 {
        let inner = &self.inner;
        if (distance < 0.0 && !inner.can_scroll_forward.peek()) || (distance > 0.0 && !inner.can_scroll_backward.peek())
        {
            return 0.0;
        }
        assert!(inner.scroll_to_be_consumed.get().abs() <= 0.5, "entered drag with non-zero pending scroll");
        inner.scroll_to_be_consumed.set(inner.scroll_to_be_consumed.get() + distance);

        if inner.scroll_to_be_consumed.get().abs() > 0.5 {
            let pre_scroll_to_be_consumed = inner.scroll_to_be_consumed.get();
            let int_delta = fast_round_to_int(inner.scroll_to_be_consumed.get());
            let current = inner.layout_info.borrow().clone();
            let mut scrolled_layout_info = current
                .copy_with_scroll_delta_without_remeasure(int_delta, !inner.has_lookahead_occurred.get())
                .map(Rc::new);
            let approach = inner.approach_layout_info.borrow().clone();
            if scrolled_layout_info.is_some()
                && let Some(approach) = approach
            {
                // (the lookahead's scrolled without a measure: the approach's too if it can be — they may differ —
                // else both are measured)
                match approach.copy_with_scroll_delta_without_remeasure(int_delta, true) {
                    Some(scrolled_approach) => {
                        *inner.approach_layout_info.borrow_mut() = Some(Rc::new(scrolled_approach))
                    }
                    None => scrolled_layout_info = None,
                }
            }
            match scrolled_layout_info {
                Some(scrolled) => {
                    self.apply_measure_result(scrolled.clone(), inner.has_lookahead_occurred.get(), true);
                    // (no measure needed: placed again only)
                    inner.placement_scope_invalidator.invalidate_scope();
                    self.notify_prefetch(pre_scroll_to_be_consumed - inner.scroll_to_be_consumed.get(), &scrolled);
                }
                None => {
                    inner.remeasurement.force_remeasure();
                    let result = inner.layout_info.borrow().clone();
                    self.notify_prefetch(pre_scroll_to_be_consumed - inner.scroll_to_be_consumed.get(), &result);
                }
            }
        }

        if inner.scroll_to_be_consumed.get().abs() <= 0.5 {
            distance
        } else {
            let scroll_consumed = distance - inner.scroll_to_be_consumed.get();
            inner.scroll_to_be_consumed.set(0.0);
            scroll_consumed
        }
    }

    /// Prefetches the next item of each lane in the scroll's direction (Compose's `notifyPrefetch`): from the last
    /// item in view (the first going back), each lane's next one, premeasured in its lanes; handles not used
    /// cancelled.
    fn notify_prefetch(&self, delta: f32, info: &LazyStaggeredGridMeasureResult) {
        let inner = &self.inner;
        if !inner.prefetching_enabled.get() || info.visible_items_info.is_empty() {
            return;
        }
        let scrolling_forward = delta < 0.0;
        let prefetch_index = if scrolling_forward {
            info.visible_items_info.last().unwrap().index()
        } else {
            info.visible_items_info[0].index()
        };
        if prefetch_index == inner.prefetch_base_index.get() {
            // (prefetched from this one already)
            return;
        }
        inner.prefetch_base_index.set(prefetch_index);

        let mut prefetch_handles_used: HashSet<i32> = HashSet::new();
        let mut target_index = prefetch_index;
        let slots = &info.slots;
        let lane_count = slots.sizes.len() as i32;
        for lane in 0..lane_count {
            let previous_index = target_index;
            // (the next item of each lane)
            target_index = {
                let lane_info = inner.lane_info.borrow();
                if scrolling_forward {
                    lane_info.find_next_item_index(previous_index, lane)
                } else {
                    lane_info.find_previous_item_index(previous_index, lane)
                }
            };
            if !(0..info.total_items_count).contains(&target_index) || prefetch_handles_used.contains(&target_index) {
                break;
            }
            prefetch_handles_used.insert(target_index);
            if inner.current_item_prefetch_handles.borrow().contains_key(&target_index) {
                continue;
            }
            let is_full_span = info.span_provider.is_full_span(target_index);
            let slot = if is_full_span { 0 } else { lane } as usize;
            let span = if is_full_span { lane_count } else { 1 };
            let cross_axis_size = if span == 1 {
                slots.sizes[slot]
            } else {
                let start = slots.positions[slot];
                let end_slot = slot + span as usize - 1;
                slots.positions[end_slot] + slots.sizes[end_slot] - start
            };
            let constraints = if info.orientation() == Orientation::Vertical {
                Constraints::fixed_width(cross_axis_size)
            } else {
                Constraints::fixed_height(cross_axis_size)
            };
            let handle = inner.prefetch_state.schedule_precomposition_and_premeasure(target_index, constraints, None);
            inner.current_item_prefetch_handles.borrow_mut().insert(target_index, handle);
        }
        // (the handles not used now cancelled)
        inner.current_item_prefetch_handles.borrow_mut().retain(|key, handle| {
            let used = prefetch_handles_used.contains(key);
            if !used {
                handle.cancel();
            }
            used
        });
    }

    /// The items in view changed away from where prefetching started: its handles cancelled (Compose's
    /// `cancelPrefetchIfVisibleItemsChanged`).
    fn cancel_prefetch_if_visible_items_changed(&self, info: &LazyStaggeredGridMeasureResult) {
        let inner = &self.inner;
        let items = &info.visible_items_info;
        if inner.prefetch_base_index.get() != -1 && !items.is_empty() {
            let base = inner.prefetch_base_index.get();
            if base < items[0].index() || base > items.last().unwrap().index() {
                inner.prefetch_base_index.set(-1);
                for (_, handle) in inner.current_item_prefetch_handles.borrow_mut().drain() {
                    handle.cancel();
                }
            }
        }
    }

    /// Takes a measure's result (Compose's `applyMeasureResult`).
    pub(crate) fn apply_measure_result(
        &self,
        result: Rc<LazyStaggeredGridMeasureResult>,
        is_looking_ahead: bool,
        visible_items_stayed_the_same: bool,
    ) {
        let inner = &self.inner;
        if !is_looking_ahead && inner.has_lookahead_occurred.get() {
            // (after a lookahead pass: the approach's result, kept apart; the delta between the passes done with
            // once the approach is where the lookahead is)
            *inner.approach_layout_info.borrow_mut() = Some(result.clone());
            without_read_observation(|| {
                let delta = &inner.scroll_delta_between_passes;
                if delta.is_active()
                    && result.first_visible_item_indices == inner.scroll_position.indices()
                    && result.first_visible_item_scroll_offsets == inner.scroll_position.scroll_offsets()
                {
                    delta.stop();
                }
            });
            return;
        }
        if is_looking_ahead {
            inner.has_lookahead_occurred.set(true);
        }
        inner.scroll_to_be_consumed.set(inner.scroll_to_be_consumed.get() - result.consumed_scroll);
        self.set_measure_result(result.clone());
        if visible_items_stayed_the_same {
            inner.scroll_position.update_scroll_offset(result.first_visible_item_scroll_offsets.clone());
        } else {
            inner.scroll_position.update_from_measure_result(&result);
            self.cancel_prefetch_if_visible_items_changed(&result);
        }
        inner.can_scroll_backward.set(result.can_scroll_backward());
        inner.can_scroll_forward.set(result.can_scroll_forward);
        if is_looking_ahead && let Some(scope) = &result.coroutine_scope {
            inner.scroll_delta_between_passes.update_scroll_delta_for_approach(
                result.scroll_back_amount,
                result.density,
                scope,
            );
        }
        inner.measure_pass_count.set(inner.measure_pass_count.get() + 1);
    }

    pub(crate) fn update_scroll_position_if_the_first_item_was_moved(
        &self,
        item_provider: &dyn LazyStaggeredGridItemProvider,
        first_item_index: &[i32],
    ) -> Vec<i32> {
        self.inner.scroll_position.update_scroll_position_if_the_first_item_was_moved(item_provider, first_item_index)
    }

    /// Compose's `snapToItemInternal`: an item in view moved to by offsets (the lanes as they are), else the lanes
    /// filled around it.
    pub(crate) fn snap_to_item_internal(&self, index: i32, scroll_offset: i32, force_remeasure: bool) {
        let inner = &self.inner;
        let position = &inner.scroll_position;
        let position_changed = position.index.peek() != index || position.scroll_offset.peek() != scroll_offset;
        if position_changed {
            inner.item_animator.reset();
        }
        let result = inner.layout_info.borrow().clone();
        let layout_info = result.layout_info();
        let visible_item = layout_info.find_visible_item(index);
        match visible_item {
            Some(visible_item) if position_changed => {
                let current_offset = if layout_info.orientation == Orientation::Vertical {
                    visible_item.offset.y
                } else {
                    visible_item.offset.x
                };
                let delta = current_offset + scroll_offset;
                let offsets = result.first_visible_item_scroll_offsets.iter().map(|o| o + delta).collect();
                position.update_scroll_offset(offsets);
            }
            _ => position.request_position_and_forget_last_known_key(index, scroll_offset),
        }
        if force_remeasure {
            inner.remeasurement.force_remeasure();
        } else {
            inner.measurement_scope_invalidator.invalidate_scope();
        }
    }

    /// The saver of [`remember_lazy_staggered_grid_state`] (Compose's `LazyStaggeredGridState.Saver`): each lane's
    /// first item and offset.
    pub fn saver() -> Saver<LazyStaggeredGridState> {
        Saver::new(
            |s: &LazyStaggeredGridState| {
                let ints = |v: Vec<i32>| SavedValue::list(v.into_iter().map(SavedValue::from));
                SavedValue::list([
                    ints(s.inner.scroll_position.indices()),
                    ints(s.inner.scroll_position.scroll_offsets()),
                ])
            },
            |v| {
                let list = v.as_list()?;
                let ints = |v: &SavedValue| -> Option<Vec<i32>> {
                    v.as_list()?.iter().map(|i| i.as_int().map(|i| i as i32)).collect()
                };
                let indices = ints(list.first()?)?;
                let offsets = ints(list.get(1)?)?;
                Some(LazyStaggeredGridState::with_lanes(indices, offsets))
            },
        )
    }
}

/// The lanes' first items around `item_index` (Compose's `fillNearestIndices`): a full-line one in all of them;
/// else the item in its lane (the first when not known), the items before it in the lanes before, after in those
/// after.
fn fill_nearest_indices(inner: &LazyStaggeredGridStateInner, item_index: i32, lane_count: usize) -> Vec<i32> {
    let mut indices = vec![0; lane_count];
    if inner.layout_info.borrow().span_provider.is_full_span(item_index) {
        indices.fill(item_index);
        return indices;
    }
    let mut lane_info = inner.lane_info.borrow_mut();
    // (the lanes moved to hold it)
    lane_info.ensure_valid_index(item_index + lane_count as i32);
    let target_lane_index = match lane_info.get_lane(item_index) {
        // (never set, or an old full-line one)
        LaneUnset | LaneFullSpan => 0,
        previous_lane => {
            assert!(previous_lane >= 0, "Expected positive lane number, got {previous_lane} instead.");
            previous_lane.min(lane_count as i32) as usize
        }
    };
    // (the lanes before)
    let mut current_item_index = item_index;
    for lane in (0..target_lane_index).rev() {
        indices[lane] = lane_info.find_previous_item_index(current_item_index, lane as i32);
        if indices[lane] == LaneUnset {
            indices[..lane].fill(-1);
            break;
        }
        current_item_index = indices[lane];
    }
    indices[target_lane_index] = item_index;
    // (the lanes after)
    current_item_index = item_index;
    for (lane, index) in indices.iter_mut().enumerate().skip(target_lane_index + 1) {
        *index = lane_info.find_next_item_index(current_item_index, lane as i32);
        current_item_index = *index;
    }
    indices
}

#[named]
impl LazyStaggeredGridState {
    /// Scrolls to item `index`, `scroll_offset` px into it, at once, in a mutation of its own (Compose's
    /// `scrollToItem`).
    pub fn scroll_to_item(&self, index: i32, #[default(0)] scroll_offset: i32) -> impl Future<Output = ()> + 'static {
        let state = self.clone();
        async move {
            let s = state.clone();
            state
                .scroll_block(
                    MutatePriority::Default,
                    Box::new(move |_| {
                        Box::pin(async move {
                            s.snap_to_item_internal(index, scroll_offset, true);
                        })
                    }),
                )
                .await;
        }
    }

    /// Glides to item `index`, `scroll_offset` px into it (Compose's `animateScrollToItem`): a far item jumped close
    /// to first (more than a hundred lines' items away).
    pub fn animate_scroll_to_item(
        &self,
        index: i32,
        #[default(0)] scroll_offset: i32,
    ) -> impl Future<Output = ()> + 'static {
        let state = self.clone();
        async move {
            let layout_info = state.inner.layout_info.borrow().clone();
            let num_of_items_to_teleport = 100 * layout_info.slots.sizes.len() as i32;
            let density = layout_info.density;
            let s = state.clone();
            state
                .scroll_block(
                    MutatePriority::Default,
                    Box::new(move |scope| {
                        Box::pin(async move {
                            let scope = LazyLayoutScrollScope(&s, scope);
                            animate_scroll_to_item(scope, index, scroll_offset, num_of_items_to_teleport, density)
                                .await;
                        })
                    }),
                )
                .await;
        }
    }

    /// Scrolls to item `index` at the next measure, without a mutation (Compose's `requestScrollToItem`): a scroll
    /// running is cancelled.
    pub fn request_scroll_to_item(&self, index: i32, #[default(0)] scroll_offset: i32) {
        if self.is_scroll_in_progress() {
            let scope = self.inner.layout_info.borrow().coroutine_scope.clone();
            if let Some(scope) = scope {
                let state = self.clone();
                scope.launch(async move {
                    state.scroll_block(MutatePriority::Default, Box::new(|_| Box::pin(async {}))).await;
                });
            }
        }
        self.snap_to_item_internal(index, scroll_offset, false);
    }
}

impl ScrollableState for LazyStaggeredGridState {
    fn scroll_block(&self, scroll_priority: MutatePriority, block: ScrollBlock) -> LocalFuture<'_, ()> {
        let state = self.clone();
        Box::pin(async move {
            if !state.inner.measured.get() {
                state.inner.await_layout_modifier.wait_for_first_layout().await;
            }
            let scrollable = state.scrollable_state();
            scrollable.scroll_block(scroll_priority, block).await;
        })
    }

    fn dispatch_raw_delta(&self, delta: f32) -> f32 {
        self.scrollable_state().dispatch_raw_delta(delta)
    }

    fn is_scroll_in_progress(&self) -> bool {
        self.scrollable_state().is_scroll_in_progress()
    }

    fn can_scroll_forward(&self) -> bool {
        self.inner.can_scroll_forward.get()
    }

    fn can_scroll_backward(&self) -> bool {
        self.inner.can_scroll_backward.get()
    }

    fn last_scrolled_forward(&self) -> bool {
        self.scrollable_state().last_scrolled_forward()
    }

    fn last_scrolled_backward(&self) -> bool {
        self.scrollable_state().last_scrolled_backward()
    }

    fn scroll_indicator_state(&self) -> Option<Rc<dyn ScrollIndicatorState>> {
        Some(Rc::new(StaggeredScrollIndicatorState(self.clone())))
    }
}

/// What a scrollbar shows of a staggered grid (Compose's `scrollIndicatorState`): guessed from the average item
/// over the lanes.
struct StaggeredScrollIndicatorState(LazyStaggeredGridState);

impl StaggeredScrollIndicatorState {
    fn calculate_scroll_offset(&self) -> i32 {
        let info = self.0.layout_info();
        if info.total_items_count == 0 {
            return 0;
        }
        (info.visible_items_average_size() * self.0.first_visible_item_index()) / self.0.lane_count()
            + self.0.first_visible_item_scroll_offset()
    }
}

impl ScrollIndicatorState for StaggeredScrollIndicatorState {
    fn scroll_offset(&self) -> i32 {
        let info = self.0.layout_info();
        if info.reverse_layout {
            info.calculate_content_size(self.0.lane_count())
                - info.single_axis_viewport_size()
                - self.calculate_scroll_offset()
        } else {
            self.calculate_scroll_offset()
        }
    }
    fn content_size(&self) -> i32 {
        self.0.layout_info().calculate_content_size(self.0.lane_count())
    }
    fn viewport_size(&self) -> i32 {
        self.0.layout_info().single_axis_viewport_size()
    }
}

/// A state scrolled to item `initial_first_visible_item_index`, remembered and saved where it is called (Compose's
/// `rememberLazyStaggeredGridState`).
#[kompozable]
pub fn remember_lazy_staggered_grid_state(
    #[default(0)] initial_first_visible_item_index: i32,
    #[default(0)] initial_first_visible_item_scroll_offset: i32,
) -> LazyStaggeredGridState {
    remember_saveable_with(&LazyStaggeredGridState::saver(), move || {
        LazyStaggeredGridState::new(initial_first_visible_item_index, initial_first_visible_item_scroll_offset)
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The scroll scope

impl LazyLayoutScrollScopeState for LazyStaggeredGridState {
    /// A staggered grid's scroll scope as a lazy layout's (Compose's `LazyLayoutScrollScope(state:
    /// LazyStaggeredGridState, scrollScope)`): the distance to an item out of view guessed by lines of the average
    /// item.
    fn lazy_layout_scroll_scope(&self, scroll_scope: Rc<dyn ScrollScope>) -> Rc<dyn LazyLayoutScrollScope> {
        Rc::new(StaggeredGridScrollScope { state: self.clone(), scroll_scope })
    }
}

struct StaggeredGridScrollScope {
    state: LazyStaggeredGridState,
    scroll_scope: Rc<dyn ScrollScope>,
}

impl ScrollScope for StaggeredGridScrollScope {
    fn scroll_by(&self, pixels: f32) -> f32 {
        self.scroll_scope.scroll_by(pixels)
    }
}

impl LazyLayoutScrollScope for StaggeredGridScrollScope {
    fn first_visible_item_index(&self) -> i32 {
        self.state.first_visible_item_index()
    }
    fn first_visible_item_scroll_offset(&self) -> i32 {
        self.state.first_visible_item_scroll_offset()
    }
    fn last_visible_item_index(&self) -> i32 {
        self.state.layout_info().items().last().map_or(0, |i| i.index)
    }
    fn item_count(&self) -> i32 {
        self.state.layout_info().total_items_count
    }
    fn snap_to_item(&self, index: i32, offset: i32) {
        self.state.snap_to_item_internal(index, offset, true);
    }
    fn calculate_distance_to(&self, target_index: i32, target_offset: i32) -> i32 {
        let layout_info = self.state.layout_info();
        if layout_info.items().is_empty() {
            return 0;
        }
        let visible_item = layout_info.items().iter().find(|i| i.index == target_index);
        let distance = match visible_item {
            None => {
                let average_main_axis_item_size = layout_info.visible_items_average_size();
                let lane_count = self.state.lane_count();
                let line_diff = target_index / lane_count - self.first_visible_item_index() / lane_count;
                average_main_axis_item_size * line_diff - self.first_visible_item_scroll_offset()
            }
            Some(item) => {
                if layout_info.orientation == Orientation::Vertical {
                    item.offset.y
                } else {
                    item.offset.x
                }
            }
        };
        distance + target_offset
    }
}
