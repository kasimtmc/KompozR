//! A lazy grid's state (Compose's LazyGridState.kt, LazyGridScrollPosition.kt, LazyGridScrollScope.kt): where it
//! is scrolled to (the first visible item — the first of its line — and how far its line is scrolled out of view,
//! kept by the item's key as items change), what its last layout showed, scrolling (a `ScrollableState` whose
//! deltas measure the grid again at once), scrolling to an item at once or animated, and prefetching lines as it
//! scrolls.

use super::grid::LazyGridItemProvider;
use super::layout_info::LazyGridLayoutInfo;
use super::measure::{LazyGridMeasureResult, LazyGridMeasuredItem, fast_round_to_int};
use super::prefetch_strategy::{
    GridLines, LazyGridCacheWindowPrefetchStrategy, LazyGridPrefetchResultScopeImpl, LazyGridPrefetchScope,
    LazyGridPrefetchStrategy, cache_window_logic_of, cache_window_strategy_of, lazy_grid_prefetch_strategy,
};
use crate::gestures::{
    DefaultScrollableState, LocalFuture, Orientation, ScrollBlock, ScrollIndicatorState, ScrollScope, ScrollableState,
};
use crate::interaction::{InteractionSource, MutableInteractionSource};
use crate::lazy::layout::{
    AwaitFirstLayoutModifier, LazyKey, LazyLayoutBeyondBoundsInfo, LazyLayoutCacheWindow, LazyLayoutItemAnimator,
    LazyLayoutNearestRangeState, LazyLayoutPinnedItemList, LazyLayoutPrefetchState, LazyLayoutScrollDeltaBetweenPasses,
    LazyLayoutScrollScope, LazyLayoutScrollScopeState, NestedPrefetchScope, ObservableScopeInvalidator, PrefetchHandle,
    PrefetchResultScope, Remeasurement, animate_scroll_to_item, find_index_by_key,
};
use kompoz_animation::MutatePriority;
use kompoz_graphics::Density;
use kompoz_runtime::{
    MutableState, SavedValue, Saver, kompozable, mutable_confined_state_of_with_policy, mutable_state_of, named,
    never_equal_policy, remember, remember_saveable_keyed_with, remember_saveable_with, without_read_observation,
};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::rc::{Rc, Weak};

// ---------------------------------------------------------------------------------------------------------------
// The scroll position

/// The items whose keys are known around the first visible one: a window of 90, and 200 more each side.
const NearestItemsSlidingWindowSize: i32 = 90;
const NearestItemsExtraItemCount: i32 = 200;

/// Where a grid is scrolled to (Compose's LazyGridScrollPosition): the first visible item's index and how far its
/// line is scrolled out (states), the key it had (to find it again when items change), the range of keys known.
struct LazyGridScrollPosition {
    index: MutableState<i32>,
    scroll_offset: MutableState<i32>,
    had_first_not_empty_layout: Cell<bool>,
    last_known_first_item_key: RefCell<Option<LazyKey>>,
    nearest_range_state: LazyLayoutNearestRangeState,
}

impl LazyGridScrollPosition {
    fn new(initial_index: i32, initial_scroll_offset: i32) -> Self {
        LazyGridScrollPosition {
            index: mutable_state_of(initial_index),
            scroll_offset: mutable_state_of(initial_scroll_offset),
            had_first_not_empty_layout: Cell::new(false),
            last_known_first_item_key: RefCell::new(None),
            nearest_range_state: LazyLayoutNearestRangeState::new(
                initial_index,
                NearestItemsSlidingWindowSize,
                NearestItemsExtraItemCount,
            ),
        }
    }

    /// From a measure (Compose's `updateFromMeasureResult`): the first item of the first line and its offset from
    /// the first measure with items on (the initial ones kept until then).
    fn update_from_measure_result(&self, result: &LazyGridMeasureResult) {
        *self.last_known_first_item_key.borrow_mut() =
            result.first_visible_line_first_item().map(|i| i.key_ref().clone());
        if self.had_first_not_empty_layout.get() || result.total_items_count > 0 {
            self.had_first_not_empty_layout.set(true);
            let scroll_offset = result.first_visible_line_scroll_offset;
            assert!(scroll_offset >= 0, "scrollOffset should be non-negative ({scroll_offset})");
            let first_index = result.first_visible_line_first_item().map_or(0, |i| i.index_value());
            self.update(first_index, scroll_offset);
        }
    }

    fn update_scroll_offset(&self, scroll_offset: i32) {
        assert!(scroll_offset >= 0, "scrollOffset should be non-negative");
        self.scroll_offset.set(scroll_offset);
    }

    /// A direct request: no key to follow until the next measure (Compose's
    /// `requestPositionAndForgetLastKnownKey`).
    fn request_position_and_forget_last_known_key(&self, index: i32, scroll_offset: i32) {
        self.update(index, scroll_offset);
        *self.last_known_first_item_key.borrow_mut() = None;
    }

    /// The first item found again by its key, if items before it changed (Compose's
    /// `updateScrollPositionIfTheFirstItemWasMoved`).
    fn update_scroll_position_if_the_first_item_was_moved(
        &self,
        item_provider: &dyn LazyGridItemProvider,
        index: i32,
    ) -> i32 {
        let key = self.last_known_first_item_key.borrow().clone();
        let new_index = find_index_by_key(item_provider, key.as_ref(), index);
        if index != new_index {
            self.index.set(new_index);
            self.nearest_range_state.update(index);
        }
        new_index
    }

    fn update(&self, index: i32, scroll_offset: i32) {
        assert!(index >= 0, "Index should be non-negative");
        self.index.set(index);
        self.nearest_range_state.update(index);
        self.scroll_offset.set(scroll_offset);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The state

/// A lazy grid's state (Compose's LazyGridState): where it is scrolled to, what its last layout showed, scrolling
/// it. A [`ScrollableState`]: its deltas measure the grid again at once (what it took is what it could scroll);
/// `scroll_to_item`, `animate_scroll_to_item`, `request_scroll_to_item`. Remembered and saved by
/// [`remember_lazy_grid_state`]. Clones are the same state; two are equal when they are.
#[derive(Clone)]
pub struct LazyGridState {
    pub(crate) inner: Rc<LazyGridStateInner>,
}

pub(crate) struct LazyGridStateInner {
    pub(crate) prefetch_strategy: Rc<dyn LazyGridPrefetchStrategy>,
    /// Whether it was measured in a lookahead pass (Compose's `hasLookaheadOccurred`): in a lookahead scope.
    pub(crate) has_lookahead_occurred: Cell<bool>,
    /// The last approach pass's result, in a lookahead scope (Compose's `approachLayoutInfo`).
    pub(crate) approach_layout_info: RefCell<Option<Rc<LazyGridMeasureResult>>>,
    /// What the approach scrolls by besides the lookahead's scroll (Compose's `_lazyLayoutScrollDeltaBetweenPasses`).
    scroll_delta_between_passes: LazyLayoutScrollDeltaBetweenPasses,
    execute_requests_in_high_priority_mode: Cell<bool>,
    scroll_position: LazyGridScrollPosition,
    /// The last measure's result (Compose's `layoutInfoState = mutableStateOf(…, neverEqualPolicy())`; confined: a measure
    /// result isn't `Send`).
    layout_info: MutableState<Rc<LazyGridMeasureResult>>,
    /// Whether a measure ran (the result isn't the empty one).
    measured: Cell<bool>,
    internal_interaction_source: MutableInteractionSource,
    pub(crate) scroll_to_be_consumed: Cell<f32>,
    scrollable_state: RefCell<Option<DefaultScrollableState>>,
    pub(crate) num_measure_passes: Cell<i32>,
    pub(crate) prefetching_enabled: Cell<bool>,
    pub(crate) remeasurement: Remeasurement,
    pub(crate) await_layout_modifier: AwaitFirstLayoutModifier,
    pub(crate) item_animator: Rc<LazyLayoutItemAnimator<LazyGridMeasuredItem>>,
    pub(crate) beyond_bounds_info: Rc<LazyLayoutBeyondBoundsInfo>,
    pub(crate) prefetch_state: LazyLayoutPrefetchState,
    prefetch_scope: RefCell<Option<Rc<dyn LazyGridPrefetchScope>>>,
    pub(crate) pinned_items: LazyLayoutPinnedItemList,
    pub(crate) measurement_scope_invalidator: ObservableScopeInvalidator,
    pub(crate) placement_scope_invalidator: ObservableScopeInvalidator,
    can_scroll_forward: MutableState<bool>,
    can_scroll_backward: MutableState<bool>,
}

impl PartialEq for LazyGridState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for LazyGridState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "LazyGridState(firstVisibleItemIndex={}, firstVisibleItemScrollOffset={})",
            self.inner.scroll_position.index.peek(),
            self.inner.scroll_position.scroll_offset.peek()
        )
    }
}

impl Default for LazyGridState {
    fn default() -> Self {
        LazyGridState::new(0, 0)
    }
}

/// A state scrolled to item `first_visible_item_index` and `first_visible_item_scroll_offset` px into its line
/// (Compose's `LazyGridState(firstVisibleItemIndex, firstVisibleItemScrollOffset)`).
/// Kotlin: `LazyGridState(firstVisibleItemIndex: Int = 0, firstVisibleItemScrollOffset: Int = 0)`
pub fn LazyGridState(first_visible_item_index: i32, first_visible_item_scroll_offset: i32) -> LazyGridState {
    LazyGridState::new(first_visible_item_index, first_visible_item_scroll_offset)
}

impl LazyGridState {
    /// A state scrolled to item `first_visible_item_index`, `first_visible_item_scroll_offset` px into its line.
    pub fn new(first_visible_item_index: i32, first_visible_item_scroll_offset: i32) -> LazyGridState {
        LazyGridState::with_prefetch_strategy(
            first_visible_item_index,
            first_visible_item_scroll_offset,
            lazy_grid_prefetch_strategy(),
        )
    }

    /// The same prefetching with `prefetch_strategy` (Compose's constructor taking one).
    pub fn with_prefetch_strategy(
        first_visible_item_index: i32,
        first_visible_item_scroll_offset: i32,
        prefetch_strategy: Rc<dyn LazyGridPrefetchStrategy>,
    ) -> LazyGridState {
        let inner = Rc::new_cyclic(|me: &Weak<LazyGridStateInner>| {
            let weak = me.clone();
            let prefetch_state = LazyLayoutPrefetchState::new(Some(Rc::new(move |scope: &dyn NestedPrefetchScope| {
                if let Some(s) = weak.upgrade() {
                    let first = without_read_observation(|| s.scroll_position.index.get());
                    s.prefetch_strategy.on_nested_prefetch(scope, first);
                }
            })));
            LazyGridStateInner {
                prefetch_strategy,
                has_lookahead_occurred: Cell::new(false),
                approach_layout_info: RefCell::new(None),
                scroll_delta_between_passes: LazyLayoutScrollDeltaBetweenPasses::new(),
                execute_requests_in_high_priority_mode: Cell::new(false),
                scroll_position: LazyGridScrollPosition::new(
                    first_visible_item_index,
                    first_visible_item_scroll_offset,
                ),
                layout_info: mutable_confined_state_of_with_policy(
                    Rc::new(LazyGridMeasureResult::empty()),
                    never_equal_policy(),
                ),
                measured: Cell::new(false),
                internal_interaction_source: MutableInteractionSource::new(),
                scroll_to_be_consumed: Cell::new(0.0),
                scrollable_state: RefCell::new(None),
                num_measure_passes: Cell::new(0),
                prefetching_enabled: Cell::new(true),
                remeasurement: Remeasurement::default(),
                await_layout_modifier: AwaitFirstLayoutModifier::default(),
                item_animator: Rc::new(LazyLayoutItemAnimator::default()),
                beyond_bounds_info: Rc::new(LazyLayoutBeyondBoundsInfo::default()),
                prefetch_state,
                prefetch_scope: RefCell::new(None),
                pinned_items: LazyLayoutPinnedItemList::new(),
                measurement_scope_invalidator: ObservableScopeInvalidator::default(),
                placement_scope_invalidator: ObservableScopeInvalidator::default(),
                can_scroll_forward: mutable_state_of(false),
                can_scroll_backward: mutable_state_of(false),
            }
        });
        // Its scrollable state: a delta is the grid's scroll the other way (Compose's `ScrollableState { -onScroll(-it) }`).
        let weak = Rc::downgrade(&inner);
        *inner.scrollable_state.borrow_mut() =
            Some(crate::gestures::ScrollableState(move |delta| match weak.upgrade() {
                Some(inner) => -LazyGridState { inner }.on_scroll(-delta),
                None => 0.0,
            }));
        let weak = Rc::downgrade(&inner);
        *inner.prefetch_scope.borrow_mut() = Some(Rc::new(GridPrefetchScope(weak)));
        LazyGridState { inner }
    }

    /// The same prefetching what `cache_window` reaches (Compose's `LazyGridState(cacheWindow, …)`).
    /// Kotlin: `LazyGridState(cacheWindow: LazyLayoutCacheWindow, firstVisibleItemIndex: Int = 0, firstVisibleItemScrollOffset: Int = 0)`
    pub fn with_cache_window(
        cache_window: Rc<dyn LazyLayoutCacheWindow>,
        first_visible_item_index: i32,
        first_visible_item_scroll_offset: i32,
    ) -> LazyGridState {
        LazyGridState::with_prefetch_strategy(
            first_visible_item_index,
            first_visible_item_scroll_offset,
            Rc::new(LazyGridCacheWindowPrefetchStrategy::new(cache_window)),
        )
    }

    fn scrollable_state(&self) -> DefaultScrollableState {
        self.inner.scrollable_state.borrow().clone().expect("its scrollable state")
    }

    /// The first item in view, the first of its line (a state; Compose's `firstVisibleItemIndex`).
    pub fn first_visible_item_index(&self) -> i32 {
        self.inner.scroll_position.index.get()
    }

    /// How far its line is scrolled out of view, px (a state; Compose's `firstVisibleItemScrollOffset`).
    pub fn first_visible_item_scroll_offset(&self) -> i32 {
        self.inner.scroll_position.scroll_offset.get()
    }

    /// What the last layout showed (Compose's `layoutInfo`): read, it is a state changing with each layout and
    /// scroll.
    pub fn layout_info(&self) -> LazyGridLayoutInfo {
        self.inner.layout_info.with(|result| result.layout_info())
    }

    /// The last measure's result (a state read).
    pub(crate) fn measure_result(&self) -> Rc<LazyGridMeasureResult> {
        self.inner.layout_info.get()
    }

    fn set_measure_result(&self, result: Rc<LazyGridMeasureResult>) {
        self.inner.layout_info.set(result);
        self.inner.measured.set(true);
    }

    /// The drags of the grid (Compose's `interactionSource`).
    pub fn interaction_source(&self) -> InteractionSource {
        self.inner.internal_interaction_source.as_interaction_source()
    }

    pub(crate) fn internal_interaction_source(&self) -> MutableInteractionSource {
        self.inner.internal_interaction_source.clone()
    }

    /// The cells of a line in the last measure (Compose's `slotsPerLine`).
    pub(crate) fn slots_per_line(&self) -> i32 {
        self.measure_result().slots_per_line
    }

    /// The density of the last measure.
    pub(crate) fn density(&self) -> Density {
        self.inner.layout_info.peek_with(|result| result.density)
    }

    /// The items whose keys are known (a state).
    pub(crate) fn nearest_range(&self) -> std::ops::Range<i32> {
        self.inner.scroll_position.nearest_range_state.value()
    }

    /// Compose's `snapToItemIndexInternal`: a real jump forgets the animations' positions and the cache window.
    pub(crate) fn snap_to_item_index_internal(&self, index: i32, scroll_offset: i32, force_remeasure: bool) {
        let position = &self.inner.scroll_position;
        let position_changed = position.index.peek() != index || position.scroll_offset.peek() != scroll_offset;
        // (the same position, kept as items change: an item's offset change is animated then)
        if position_changed {
            self.inner.item_animator.reset();
            if let Some(logic) = cache_window_logic_of(&*self.inner.prefetch_strategy) {
                logic.reset_strategy();
            }
        }
        position.request_position_and_forget_last_known_key(index, scroll_offset);
        if force_remeasure {
            self.inner.remeasurement.force_remeasure();
        } else {
            self.inner.measurement_scope_invalidator.invalidate_scope();
        }
    }

    /// Takes a scroll of `distance` (Compose's `onScroll`, positive back): measured at once (or applied without a
    /// measure when the items in view stay the same), prefetching told; what it took.
    pub(crate) fn on_scroll(&self, distance: f32) -> f32 {
        let inner = &self.inner;
        if (distance < 0.0 && !inner.can_scroll_forward.peek()) || (distance > 0.0 && !inner.can_scroll_backward.peek())
        {
            return 0.0;
        }
        assert!(inner.scroll_to_be_consumed.get().abs() <= 0.5, "entered drag with non-zero pending scroll");
        inner.scroll_to_be_consumed.set(inner.scroll_to_be_consumed.get() + distance);

        // (consumed by the measure below; under half a pixel it isn't)
        if inner.scroll_to_be_consumed.get().abs() > 0.5 {
            let pre_scroll_to_be_consumed = inner.scroll_to_be_consumed.get();
            let int_delta = fast_round_to_int(inner.scroll_to_be_consumed.get());
            let current = inner.layout_info.peek();
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
                    self.notify_prefetch_on_scroll(
                        pre_scroll_to_be_consumed - inner.scroll_to_be_consumed.get(),
                        &scrolled,
                    );
                }
                None => {
                    inner.remeasurement.force_remeasure();
                    let result = inner.layout_info.peek();
                    self.notify_prefetch_on_scroll(
                        pre_scroll_to_be_consumed - inner.scroll_to_be_consumed.get(),
                        &result,
                    );
                }
            }
        }

        // (the measure consumed it)
        if inner.scroll_to_be_consumed.get().abs() <= 0.5 {
            // All taken (the fraction kept for later).
            distance
        } else {
            let scroll_consumed = distance - inner.scroll_to_be_consumed.get();
            // (the rest given back: nested scrolling's)
            inner.scroll_to_be_consumed.set(0.0);
            scroll_consumed
        }
    }

    fn notify_prefetch_on_scroll(&self, delta: f32, result: &LazyGridMeasureResult) {
        if self.inner.prefetching_enabled.get()
            && let Some(scope) = self.inner.prefetch_scope.borrow().clone()
        {
            self.tell_cache_window_lines(result);
            self.inner.prefetch_strategy.on_scroll(&scope, delta, &result.layout_info());
        }
    }

    /// A cache window asks of the lines what Compose reaches by casting the layout info to the measure result.
    fn tell_cache_window_lines(&self, result: &LazyGridMeasureResult) {
        if let Some(strategy) = cache_window_strategy_of(&*self.inner.prefetch_strategy) {
            strategy.set_lines(GridLines {
                prefetch_info_retriever: result.prefetch_info_retriever.clone(),
                line_index_provider: result.line_index_provider.clone(),
            });
        }
    }

    /// Takes a measure's result (Compose's `applyMeasureResult`).
    pub(crate) fn apply_measure_result(
        &self,
        result: Rc<LazyGridMeasureResult>,
        is_looking_ahead: bool,
        visible_items_stayed_the_same: bool,
    ) {
        let inner = &self.inner;
        // (the number of items a nested prefetch of this grid prefetches)
        inner.prefetch_state.set_ideal_nested_prefetch_count(result.visible_items_info.len() as i32);
        if !is_looking_ahead && inner.has_lookahead_occurred.get() {
            // (after a lookahead pass: the approach's result, kept apart; the delta between the passes done with
            // once the approach is where the lookahead is — both scrolled alike)
            *inner.approach_layout_info.borrow_mut() = Some(result.clone());
            without_read_observation(|| {
                let delta = &inner.scroll_delta_between_passes;
                if delta.is_active()
                    && result.first_visible_line_scroll_offset == inner.scroll_position.scroll_offset.get()
                    && result.first_visible_line_first_item().map(|i| i.index_value())
                        == Some(inner.scroll_position.index.get())
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
        inner.can_scroll_backward.set(result.can_scroll_backward());
        inner.can_scroll_forward.set(result.can_scroll_forward);
        if visible_items_stayed_the_same {
            inner.scroll_position.update_scroll_offset(result.first_visible_line_scroll_offset);
        } else {
            inner.scroll_position.update_from_measure_result(&result);
            if inner.prefetching_enabled.get()
                && let Some(scope) = inner.prefetch_scope.borrow().clone()
            {
                self.tell_cache_window_lines(&result);
                inner.prefetch_strategy.on_visible_items_updated(&scope, &result.layout_info());
            }
        }
        if is_looking_ahead && let Some(scope) = &result.coroutine_scope {
            inner.scroll_delta_between_passes.update_scroll_delta_for_approach(
                result.scroll_back_amount,
                result.density,
                scope,
            );
        }
        inner.num_measure_passes.set(inner.num_measure_passes.get() + 1);
    }

    /// The scroll the approach pass measures with after a lookahead pass (Compose's `scrollDeltaBetweenPasses`).
    pub(crate) fn scroll_delta_between_passes(&self) -> f32 {
        self.inner.scroll_delta_between_passes.scroll_delta_between_passes()
    }

    pub(crate) fn update_scroll_position_if_the_first_item_was_moved(
        &self,
        item_provider: &dyn LazyGridItemProvider,
        first_item_index: i32,
    ) -> i32 {
        self.inner.scroll_position.update_scroll_position_if_the_first_item_was_moved(item_provider, first_item_index)
    }

    /// The saver of [`remember_lazy_grid_state`] (Compose's `LazyGridState.Saver`): the first visible item and its
    /// offset.
    pub fn saver() -> Saver<LazyGridState> {
        Self::saver_with(Rc::new(LazyGridState::new))
    }

    fn saver_with(make: Rc<dyn Fn(i32, i32) -> LazyGridState>) -> Saver<LazyGridState> {
        Saver::new(
            |s: &LazyGridState| {
                SavedValue::list([
                    SavedValue::from(s.inner.scroll_position.index.peek()),
                    SavedValue::from(s.inner.scroll_position.scroll_offset.peek()),
                ])
            },
            move |v| {
                let list = v.as_list()?;
                let index = list.first()?.as_int()? as i32;
                let offset = list.get(1)?.as_int()? as i32;
                Some(make(index, offset))
            },
        )
    }

    /// The items a far animated scroll jumps over first: a hundred lines' (Compose's `numOfItemsToTeleport`).
    fn num_of_items_to_teleport(&self) -> i32 {
        100 * self.slots_per_line()
    }
}

#[named]
impl LazyGridState {
    /// Scrolls to item `index`, its line `scroll_offset` px into view, at once, in a mutation of its own
    /// (Compose's `scrollToItem`).
    pub fn scroll_to_item(&self, index: i32, #[default(0)] scroll_offset: i32) -> impl Future<Output = ()> + 'static {
        let state = self.clone();
        async move {
            let s = state.clone();
            state
                .scroll_block(
                    MutatePriority::Default,
                    Box::new(move |_| {
                        Box::pin(async move {
                            s.snap_to_item_index_internal(index, scroll_offset, true);
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
            let scope = self.inner.layout_info.peek().coroutine_scope.clone();
            if let Some(scope) = scope {
                let state = self.clone();
                scope.launch(async move {
                    state.scroll_block(MutatePriority::Default, Box::new(|_| Box::pin(async {}))).await;
                });
            }
        }
        self.snap_to_item_index_internal(index, scroll_offset, false);
    }

    /// Glides to item `index`, `scroll_offset` px into its line (Compose's `animateScrollToItem`): a far item
    /// jumped close to first (more than a hundred lines away), then a spring heading where the item really is once
    /// it shows; it stops where the grid ends.
    pub fn animate_scroll_to_item(
        &self,
        index: i32,
        #[default(0)] scroll_offset: i32,
    ) -> impl Future<Output = ()> + 'static {
        let state = self.clone();
        async move {
            let s = state.clone();
            state
                .scroll_block(
                    MutatePriority::Default,
                    Box::new(move |scope| {
                        Box::pin(async move {
                            let density = s.density();
                            let teleport = s.num_of_items_to_teleport();
                            let scope = LazyLayoutScrollScope(&s, scope);
                            animate_scroll_to_item(scope, index, scroll_offset, teleport, density).await;
                        })
                    }),
                )
                .await;
        }
    }
}

impl ScrollableState for LazyGridState {
    fn scroll_block(&self, scroll_priority: MutatePriority, block: ScrollBlock) -> LocalFuture<'_, ()> {
        let state = self.clone();
        Box::pin(async move {
            // (scrolled before it was measured: waits for its first layout)
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
        Some(Rc::new(GridScrollIndicatorState(self.clone())))
    }
}

/// What a scrollbar shows of a grid (Compose's LazyGridState `scrollIndicatorState`): its content and offset
/// guessed from the average line.
struct GridScrollIndicatorState(LazyGridState);

impl GridScrollIndicatorState {
    /// Compose's `calculateScrollOffset`: the first visible item's line (a sticking header's: the line before the
    /// first real one) times the average line.
    fn calculate_scroll_offset(&self) -> i32 {
        let info = self.0.layout_info();
        let visible_items = info.items();
        let orientation = info.orientation;
        let first_visible_item_index = self.0.first_visible_item_index();
        let first_visible_item_line_index = visible_items
            .iter()
            .find(|i| i.index == first_visible_item_index)
            .map_or(-1, |i| i.line_index(orientation));
        let line_for_offset = if first_visible_item_line_index != -1 {
            first_visible_item_line_index
        } else {
            let first_valid_line_index =
                visible_items.iter().find(|i| i.line_index(orientation) != -1).map_or(0, |i| i.line_index(orientation));
            (first_valid_line_index - 1).max(0)
        };
        info.visible_lines_average_main_axis_size() * line_for_offset + self.0.first_visible_item_scroll_offset()
    }
}

impl ScrollIndicatorState for GridScrollIndicatorState {
    fn scroll_offset(&self) -> i32 {
        let info = self.0.layout_info();
        if info.reverse_layout {
            info.calculate_content_size() - info.single_axis_viewport_size() - self.calculate_scroll_offset()
        } else {
            self.calculate_scroll_offset()
        }
    }
    fn content_size(&self) -> i32 {
        self.0.layout_info().calculate_content_size()
    }
    fn viewport_size(&self) -> i32 {
        self.0.layout_info().single_axis_viewport_size()
    }
}

/// The state's prefetch scope (Compose's `prefetchScope`): a line's items composed and measured in the constraints
/// of their cells in the last measure; once all are, the line's sizes told.
struct GridPrefetchScope(Weak<LazyGridStateInner>);

impl LazyGridPrefetchScope for GridPrefetchScope {
    fn schedule_line_prefetch(
        &self,
        line_index: i32,
        on_prefetch_finished: Option<Rc<dyn Fn(&dyn super::prefetch_strategy::LazyGridPrefetchResultScope)>>,
    ) -> Vec<PrefetchHandle> {
        let Some(inner) = self.0.upgrade() else { return Vec::new() };
        // (not read: a scroll prefetching doesn't make what scrolls read the result; in a lookahead scope, the
        // approach's, what is composed)
        let measure_result = if inner.has_lookahead_occurred.get() {
            match inner.approach_layout_info.borrow().clone() {
                Some(r) => r,
                None => return Vec::new(),
            }
        } else {
            inner.layout_info.peek()
        };
        let orientation = measure_result.orientation;
        let items_in_line_info = (measure_result.prefetch_info_retriever)(line_index);
        let line_item_count = items_in_line_info.len();
        let item_sizes: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));
        let completed_count = Rc::new(Cell::new(1usize));
        let mut prefetch_handles = Vec::with_capacity(line_item_count);
        for (index, constraints) in items_in_line_info {
            let (sizes, completed, on_finished) =
                (item_sizes.clone(), completed_count.clone(), on_prefetch_finished.clone());
            let on_premeasured: Rc<dyn Fn(&dyn PrefetchResultScope)> =
                Rc::new(move |result: &dyn PrefetchResultScope| {
                    let item_main_axis_item_size: i32 = (0..result.placeables_count())
                        .map(|i| {
                            let size = result.get_size(i);
                            if orientation == Orientation::Vertical { size.height } else { size.width }
                        })
                        .sum();
                    if on_finished.is_some() {
                        sizes.borrow_mut().push(item_main_axis_item_size);
                    }
                    // (all the line's items prefetched: its sizes told)
                    if completed.get() == line_item_count {
                        if let Some(on_finished) = &on_finished {
                            let sizes = sizes.borrow().clone();
                            on_finished(&LazyGridPrefetchResultScopeImpl { line_index, main_axis_sizes: sizes });
                        }
                    } else {
                        completed.set(completed.get() + 1);
                    }
                });
            prefetch_handles.push(inner.prefetch_state.schedule_precomposition_and_premeasure_with_priority(
                index,
                constraints,
                inner.execute_requests_in_high_priority_mode.get(),
                Some(on_premeasured),
            ));
        }
        prefetch_handles
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Remembering

/// A state scrolled to item `initial_first_visible_item_index`, remembered and saved where it is called (Compose's
/// `rememberLazyGridState`).
#[kompozable]
pub fn remember_lazy_grid_state(
    #[default(0)] initial_first_visible_item_index: i32,
    #[default(0)] initial_first_visible_item_scroll_offset: i32,
) -> LazyGridState {
    remember_saveable_with(&LazyGridState::saver(), move || {
        LazyGridState::new(initial_first_visible_item_index, initial_first_visible_item_scroll_offset)
    })
}

/// The same with a prefetch strategy (Compose's `rememberLazyGridState(…, prefetchStrategy)`).
/// Kotlin: `rememberLazyGridState(initialFirstVisibleItemIndex: Int = 0, initialFirstVisibleItemScrollOffset: Int = 0, prefetchStrategy: LazyGridPrefetchStrategy = remember { LazyGridPrefetchStrategy() })`
#[kompozable]
pub fn remember_lazy_grid_state_with_prefetch_strategy(
    #[default(0)] initial_first_visible_item_index: i32,
    #[default(0)] initial_first_visible_item_scroll_offset: i32,
    #[default(None)] prefetch_strategy: Option<Rc<dyn LazyGridPrefetchStrategy>>,
) -> LazyGridState {
    let prefetch_strategy = match prefetch_strategy {
        Some(s) => s,
        None => remember(lazy_grid_prefetch_strategy),
    };
    let strategy = prefetch_strategy.clone();
    let saver =
        LazyGridState::saver_with(Rc::new(move |i, o| LazyGridState::with_prefetch_strategy(i, o, strategy.clone())));
    remember_saveable_keyed_with(StrategyKey(prefetch_strategy.clone()), &saver, move || {
        LazyGridState::with_prefetch_strategy(
            initial_first_visible_item_index,
            initial_first_visible_item_scroll_offset,
            prefetch_strategy,
        )
    })
}

/// The same with a cache window (Compose's `rememberLazyGridState(cacheWindow, …)`).
/// Kotlin: `rememberLazyGridState(cacheWindow: LazyLayoutCacheWindow, initialFirstVisibleItemIndex: Int = 0, initialFirstVisibleItemScrollOffset: Int = 0)`
#[kompozable]
pub fn remember_lazy_grid_state_with_cache_window(
    cache_window: Rc<dyn LazyLayoutCacheWindow>,
    #[default(0)] initial_first_visible_item_index: i32,
    #[default(0)] initial_first_visible_item_scroll_offset: i32,
) -> LazyGridState {
    let window = cache_window.clone();
    let saver = LazyGridState::saver_with(Rc::new(move |i, o| LazyGridState::with_cache_window(window.clone(), i, o)));
    remember_saveable_keyed_with(WindowKey(cache_window.clone()), &saver, move || {
        LazyGridState::with_cache_window(
            cache_window,
            initial_first_visible_item_index,
            initial_first_visible_item_scroll_offset,
        )
    })
}

/// A strategy as a key: the same one.
#[derive(Clone)]
struct StrategyKey(Rc<dyn LazyGridPrefetchStrategy>);

impl PartialEq for StrategyKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// A cache window as a key: the same one.
#[derive(Clone)]
struct WindowKey(Rc<dyn LazyLayoutCacheWindow>);

impl PartialEq for WindowKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The scroll scope

impl LazyLayoutScrollScopeState for LazyGridState {
    /// A grid's scroll scope as a lazy layout's (Compose's `LazyLayoutScrollScope(state: LazyGridState,
    /// scrollScope)`): the distance to an item out of view guessed by lines of the average line.
    fn lazy_layout_scroll_scope(&self, scroll_scope: Rc<dyn ScrollScope>) -> Rc<dyn LazyLayoutScrollScope> {
        Rc::new(GridScrollScope { state: self.clone(), scroll_scope })
    }
}

struct GridScrollScope {
    state: LazyGridState,
    scroll_scope: Rc<dyn ScrollScope>,
}

impl ScrollScope for GridScrollScope {
    fn scroll_by(&self, pixels: f32) -> f32 {
        self.scroll_scope.scroll_by(pixels)
    }
}

impl LazyLayoutScrollScope for GridScrollScope {
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
        self.state.snap_to_item_index_internal(index, offset, true);
    }
    fn calculate_distance_to(&self, target_index: i32, target_offset: i32) -> i32 {
        let layout_info = self.state.layout_info();
        if layout_info.items().is_empty() {
            return 0;
        }
        let first = self.first_visible_item_index();
        let distance = if target_index < first || target_index > self.last_visible_item_index() {
            let slots_per_line = self.state.slots_per_line();
            let average_line_main_axis_size = layout_info.visible_lines_average_main_axis_size();
            let before = target_index < first;
            let lines_diff =
                (target_index - first + (slots_per_line - 1) * if before { -1 } else { 1 }) / slots_per_line;
            average_line_main_axis_size * lines_diff - self.first_visible_item_scroll_offset()
        } else {
            let visible_item = layout_info.items().iter().find(|i| i.index == target_index);
            visible_item
                .map_or(0, |i| if layout_info.orientation == Orientation::Vertical { i.offset.y } else { i.offset.x })
        };
        distance + target_offset
    }
}
