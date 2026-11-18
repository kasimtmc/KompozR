//! A lazy list's state (Compose's LazyListState.kt, LazyListScrollPosition.kt, LazyListScrollScope.kt): where it
//! is scrolled to (the first visible item and how far it is scrolled out of view, kept by the item's key as items
//! change), what its last layout showed, scrolling (a `ScrollableState` whose deltas measure the list again at
//! once), scrolling to an item at once or animated, and prefetching as it scrolls.

use super::layout_info::LazyListLayoutInfo;
use super::list::LazyListItemProvider;
use super::measure::{LazyListMeasureResult, LazyListMeasuredItem, fast_round_to_int};
use super::prefetch_strategy::{
    LazyListCacheWindowStrategy, LazyListPrefetchResultScope, LazyListPrefetchResultScopeImpl, LazyListPrefetchScope,
    LazyListPrefetchStrategy, cache_window_logic_of, lazy_list_prefetch_strategy,
};
use crate::gestures::{
    DefaultScrollableState, LocalFuture, Orientation, ScrollBlock, ScrollIndicatorState, ScrollScope, ScrollableState,
};
use crate::interaction::{InteractionSource, MutableInteractionSource};
use crate::lazy::layout::{
    AwaitFirstLayoutModifier, LazyKey, LazyLayoutBeyondBoundsInfo, LazyLayoutCacheWindow, LazyLayoutItemAnimator,
    LazyLayoutNearestRangeState, LazyLayoutPinnedItemList, LazyLayoutPrefetchState, LazyLayoutScrollScope,
    LazyLayoutScrollScopeState, NestedPrefetchScope, ObservableScopeInvalidator, PrefetchHandle, PrefetchResultScope,
    Remeasurement, animate_scroll_to_item, find_index_by_key,
};
use kompoz_animation::MutatePriority;
use kompoz_graphics::Density;
use kompoz_runtime::{
    MutableState, SavedValue, Saver, kompozable, mutable_state_of, named, remember, remember_saveable_keyed_with,
    remember_saveable_with, without_read_observation,
};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::rc::{Rc, Weak};

// ---------------------------------------------------------------------------------------------------------------
// The scroll position

/// The items whose keys are known around the first visible one: a window of 30, and 100 more each side.
const NearestItemsSlidingWindowSize: i32 = 30;
const NearestItemsExtraItemCount: i32 = 100;

/// Where a list is scrolled to (Compose's LazyListScrollPosition): the first visible item's index and how far it
/// is scrolled out (states), the key it had (to find it again when items change), the range of keys known.
struct LazyListScrollPosition {
    index: MutableState<i32>,
    scroll_offset: MutableState<i32>,
    had_first_not_empty_layout: Cell<bool>,
    last_known_first_item_key: RefCell<Option<LazyKey>>,
    nearest_range_state: LazyLayoutNearestRangeState,
}

impl LazyListScrollPosition {
    fn new(initial_index: i32, initial_scroll_offset: i32) -> Self {
        LazyListScrollPosition {
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

    /// From a measure (Compose's `updateFromMeasureResult`): the index and offset of the first item from the first
    /// measure with items on (the initial ones kept until then).
    fn update_from_measure_result(&self, result: &LazyListMeasureResult) {
        *self.last_known_first_item_key.borrow_mut() = result.first_visible_item.as_ref().map(|i| i.key_ref().clone());
        if self.had_first_not_empty_layout.get() || result.total_items_count > 0 {
            self.had_first_not_empty_layout.set(true);
            let scroll_offset = result.first_visible_item_scroll_offset;
            assert!(scroll_offset >= 0, "scrollOffset should be non-negative");
            let first_index = result.first_visible_item.as_ref().map_or(0, |i| i.index_value());
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
        item_provider: &dyn LazyListItemProvider,
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
        assert!(index >= 0, "Index should be non-negative ({index})");
        self.index.set(index);
        self.nearest_range_state.update(index);
        self.scroll_offset.set(scroll_offset);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The state

/// A lazy list's state (Compose's LazyListState): where it is scrolled to, what its last layout showed, scrolling
/// it. A [`ScrollableState`]: its deltas measure the list again at once (what it took is what it could scroll);
/// `scroll_to_item`, `animate_scroll_to_item`, `request_scroll_to_item`. Remembered and saved by
/// [`remember_lazy_list_state`]. Clones are the same state; two are equal when they are.
#[derive(Clone)]
pub struct LazyListState {
    pub(crate) inner: Rc<LazyListStateInner>,
}

pub(crate) struct LazyListStateInner {
    pub(crate) prefetch_strategy: Rc<dyn LazyListPrefetchStrategy>,
    /// Lookahead (stage 11's): never yet.
    pub(crate) has_lookahead_occurred: Cell<bool>,
    execute_requests_in_high_priority_mode: Cell<bool>,
    scroll_position: LazyListScrollPosition,
    /// The last measure's result, its readers told through the counter (Compose's `layoutInfoState`, a state of the
    /// result with a never-equal policy).
    layout_info: RefCell<Rc<LazyListMeasureResult>>,
    layout_info_generation: MutableState<u64>,
    /// Whether a measure ran (the result isn't the empty one).
    measured: Cell<bool>,
    internal_interaction_source: MutableInteractionSource,
    pub(crate) scroll_to_be_consumed: Cell<f32>,
    pub(crate) skip_item_placement_animation: Cell<bool>,
    scrollable_state: RefCell<Option<DefaultScrollableState>>,
    pub(crate) num_measure_passes: Cell<i32>,
    pub(crate) prefetching_enabled: Cell<bool>,
    pub(crate) remeasurement: Remeasurement,
    pub(crate) await_layout_modifier: AwaitFirstLayoutModifier,
    pub(crate) item_animator: Rc<LazyLayoutItemAnimator<LazyListMeasuredItem>>,
    pub(crate) beyond_bounds_info: Rc<LazyLayoutBeyondBoundsInfo>,
    pub(crate) prefetch_state: LazyLayoutPrefetchState,
    prefetch_scope: RefCell<Option<Rc<dyn LazyListPrefetchScope>>>,
    pub(crate) pinned_items: LazyLayoutPinnedItemList,
    pub(crate) measurement_scope_invalidator: ObservableScopeInvalidator,
    pub(crate) placement_scope_invalidator: ObservableScopeInvalidator,
    can_scroll_forward: MutableState<bool>,
    can_scroll_backward: MutableState<bool>,
}

impl PartialEq for LazyListState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for LazyListState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "LazyListState(firstVisibleItemIndex={}, firstVisibleItemScrollOffset={})",
            self.inner.scroll_position.index.peek(),
            self.inner.scroll_position.scroll_offset.peek()
        )
    }
}

impl Default for LazyListState {
    fn default() -> Self {
        LazyListState::new(0, 0)
    }
}

/// A state scrolled to item `first_visible_item_index` and `first_visible_item_scroll_offset` px into it (Compose's
/// `LazyListState(firstVisibleItemIndex, firstVisibleItemScrollOffset)`).
/// Kotlin: `LazyListState(firstVisibleItemIndex: Int = 0, firstVisibleItemScrollOffset: Int = 0)`
pub fn LazyListState(first_visible_item_index: i32, first_visible_item_scroll_offset: i32) -> LazyListState {
    LazyListState::new(first_visible_item_index, first_visible_item_scroll_offset)
}

impl LazyListState {
    /// A state scrolled to item `first_visible_item_index`, `first_visible_item_scroll_offset` px into it.
    pub fn new(first_visible_item_index: i32, first_visible_item_scroll_offset: i32) -> LazyListState {
        LazyListState::with_prefetch_strategy(
            first_visible_item_index,
            first_visible_item_scroll_offset,
            lazy_list_prefetch_strategy(),
        )
    }

    /// The same prefetching with `prefetch_strategy` (Compose's constructor taking one).
    pub fn with_prefetch_strategy(
        first_visible_item_index: i32,
        first_visible_item_scroll_offset: i32,
        prefetch_strategy: Rc<dyn LazyListPrefetchStrategy>,
    ) -> LazyListState {
        let inner = Rc::new_cyclic(|me: &Weak<LazyListStateInner>| {
            let weak = me.clone();
            let prefetch_state = LazyLayoutPrefetchState::new(Some(Rc::new(move |scope: &dyn NestedPrefetchScope| {
                if let Some(s) = weak.upgrade() {
                    let first = without_read_observation(|| s.scroll_position.index.get());
                    s.prefetch_strategy.on_nested_prefetch(scope, first);
                }
            })));
            LazyListStateInner {
                prefetch_strategy,
                has_lookahead_occurred: Cell::new(false),
                execute_requests_in_high_priority_mode: Cell::new(false),
                scroll_position: LazyListScrollPosition::new(
                    first_visible_item_index,
                    first_visible_item_scroll_offset,
                ),
                layout_info: RefCell::new(Rc::new(LazyListMeasureResult::empty())),
                layout_info_generation: mutable_state_of(0),
                measured: Cell::new(false),
                internal_interaction_source: MutableInteractionSource::new(),
                scroll_to_be_consumed: Cell::new(0.0),
                skip_item_placement_animation: Cell::new(false),
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
        // Its scrollable state: a delta is the list's scroll the other way (Compose's `ScrollableState { -onScroll(-it) }`).
        let weak = Rc::downgrade(&inner);
        *inner.scrollable_state.borrow_mut() =
            Some(crate::gestures::ScrollableState(move |delta| match weak.upgrade() {
                Some(inner) => -LazyListState { inner }.on_scroll(-delta),
                None => 0.0,
            }));
        let weak = Rc::downgrade(&inner);
        *inner.prefetch_scope.borrow_mut() = Some(Rc::new(ListPrefetchScope(weak)));
        LazyListState { inner }
    }

    /// The same prefetching what `cache_window` reaches (Compose's `LazyListState(cacheWindow, …)`).
    /// Kotlin: `LazyListState(cacheWindow: LazyLayoutCacheWindow, firstVisibleItemIndex: Int = 0, firstVisibleItemScrollOffset: Int = 0)`
    pub fn with_cache_window(
        cache_window: Rc<dyn LazyLayoutCacheWindow>,
        first_visible_item_index: i32,
        first_visible_item_scroll_offset: i32,
    ) -> LazyListState {
        LazyListState::with_prefetch_strategy(
            first_visible_item_index,
            first_visible_item_scroll_offset,
            Rc::new(LazyListCacheWindowStrategy::new(cache_window)),
        )
    }

    fn scrollable_state(&self) -> DefaultScrollableState {
        self.inner.scrollable_state.borrow().clone().expect("its scrollable state")
    }

    /// The first item in view (a state: reading it recomposes as it changes; Compose's `firstVisibleItemIndex`).
    pub fn first_visible_item_index(&self) -> i32 {
        self.inner.scroll_position.index.get()
    }

    /// How far it is scrolled out of view, px (a state; Compose's `firstVisibleItemScrollOffset`).
    pub fn first_visible_item_scroll_offset(&self) -> i32 {
        self.inner.scroll_position.scroll_offset.get()
    }

    /// What the last layout showed (Compose's `layoutInfo`): read, it is a state changing with each layout and
    /// scroll (read it in `derived_state_of` or `snapshot_flow` to be told only of what matters).
    pub fn layout_info(&self) -> LazyListLayoutInfo {
        self.inner.layout_info_generation.get();
        self.inner.layout_info.borrow().layout_info()
    }

    /// The last measure's result (a state read).
    pub(crate) fn measure_result(&self) -> Rc<LazyListMeasureResult> {
        self.inner.layout_info_generation.get();
        self.inner.layout_info.borrow().clone()
    }

    fn set_measure_result(&self, result: Rc<LazyListMeasureResult>) {
        *self.inner.layout_info.borrow_mut() = result;
        self.inner.measured.set(true);
        self.inner.layout_info_generation.set(self.inner.layout_info_generation.peek().wrapping_add(1));
    }

    /// The drags of the list (Compose's `interactionSource`).
    pub fn interaction_source(&self) -> InteractionSource {
        self.inner.internal_interaction_source.as_interaction_source()
    }

    pub(crate) fn internal_interaction_source(&self) -> MutableInteractionSource {
        self.inner.internal_interaction_source.clone()
    }

    /// The density of the last measure.
    pub(crate) fn density(&self) -> Density {
        without_read_observation(|| self.inner.layout_info.borrow().density)
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
            // (moved: the cache window refilled at the next scroll)
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
        inner.execute_requests_in_high_priority_mode.set(true);
        inner.scroll_to_be_consumed.set(inner.scroll_to_be_consumed.get() + distance);

        // (consumed by the measure below; under half a pixel it isn't)
        if inner.scroll_to_be_consumed.get().abs() > 0.5 {
            let pre_scroll_to_be_consumed = inner.scroll_to_be_consumed.get();
            let int_delta = fast_round_to_int(inner.scroll_to_be_consumed.get());
            let current = inner.layout_info.borrow().clone();
            let scrolled_layout_info = current
                .copy_with_scroll_delta_without_remeasure(int_delta, !inner.has_lookahead_occurred.get())
                .map(Rc::new);
            match scrolled_layout_info {
                Some(scrolled) => {
                    self.apply_measure_result(scrolled.clone(), inner.has_lookahead_occurred.get(), true);
                    // (no measure needed: placed again only)
                    inner.placement_scope_invalidator.invalidate_scope();
                    self.notify_prefetch_on_scroll(
                        pre_scroll_to_be_consumed - inner.scroll_to_be_consumed.get(),
                        &scrolled.layout_info(),
                    );
                }
                None => {
                    inner.remeasurement.force_remeasure();
                    let info = without_read_observation(|| self.layout_info());
                    self.notify_prefetch_on_scroll(
                        pre_scroll_to_be_consumed - inner.scroll_to_be_consumed.get(),
                        &info,
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

    fn notify_prefetch_on_scroll(&self, delta: f32, layout_info: &LazyListLayoutInfo) {
        if self.inner.prefetching_enabled.get()
            && let Some(scope) = self.inner.prefetch_scope.borrow().clone()
        {
            self.inner.prefetch_strategy.on_scroll(&scope, delta, layout_info);
        }
    }

    /// Takes a measure's result (Compose's `applyMeasureResult`).
    pub(crate) fn apply_measure_result(
        &self,
        result: Rc<LazyListMeasureResult>,
        is_looking_ahead: bool,
        visible_items_stayed_the_same: bool,
    ) {
        let inner = &self.inner;
        // (the number of items a nested prefetch of this list prefetches)
        inner.prefetch_state.set_ideal_nested_prefetch_count(result.visible_items_info.len() as i32);
        if is_looking_ahead {
            inner.has_lookahead_occurred.set(true);
        }
        inner.can_scroll_backward.set(result.can_scroll_backward());
        inner.can_scroll_forward.set(result.can_scroll_forward);
        inner.scroll_to_be_consumed.set(inner.scroll_to_be_consumed.get() - result.consumed_scroll);
        self.set_measure_result(result.clone());
        if visible_items_stayed_the_same {
            inner.scroll_position.update_scroll_offset(result.first_visible_item_scroll_offset);
        } else {
            inner.scroll_position.update_from_measure_result(&result);
            if inner.prefetching_enabled.get()
                && let Some(scope) = inner.prefetch_scope.borrow().clone()
            {
                inner.prefetch_strategy.on_visible_items_updated(&scope, &result.layout_info());
            }
        }
        inner.num_measure_passes.set(inner.num_measure_passes.get() + 1);
    }

    pub(crate) fn update_scroll_position_if_the_first_item_was_moved(
        &self,
        item_provider: &dyn LazyListItemProvider,
        first_item_index: i32,
    ) -> i32 {
        self.inner.scroll_position.update_scroll_position_if_the_first_item_was_moved(item_provider, first_item_index)
    }

    /// The saver of [`remember_lazy_list_state`] (Compose's `LazyListState.Saver`): the first visible item and its
    /// offset.
    pub fn saver() -> Saver<LazyListState> {
        Self::saver_with(Rc::new(LazyListState::new))
    }

    fn saver_with(make: Rc<dyn Fn(i32, i32) -> LazyListState>) -> Saver<LazyListState> {
        Saver::new(
            |s: &LazyListState| {
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
}

#[named]
impl LazyListState {
    /// Scrolls to item `index`, `scroll_offset` px into it, at once, in a mutation of its own (Compose's
    /// `scrollToItem`): laid out at once.
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
    /// running is cancelled; the measure in this frame (from a measure or a composition) takes it.
    pub fn request_scroll_to_item(&self, index: i32, #[default(0)] scroll_offset: i32) {
        // (a scroll running cancelled)
        if self.is_scroll_in_progress() {
            let scope = self.inner.layout_info.borrow().coroutine_scope.clone();
            if let Some(scope) = scope {
                let state = self.clone();
                scope.launch(async move {
                    state.scroll_block(MutatePriority::Default, Box::new(|_| Box::pin(async {}))).await;
                });
            }
        }
        self.snap_to_item_index_internal(index, scroll_offset, false);
    }

    /// Glides to item `index`, `scroll_offset` px into it (Compose's `animateScrollToItem`): a far item jumped close
    /// to first (more than a hundred items away), then a spring heading where the item really is once it shows;
    /// it stops where the list ends. Item placement animations are skipped meanwhile.
    pub fn animate_scroll_to_item(
        &self,
        index: i32,
        #[default(0)] scroll_offset: i32,
    ) -> impl Future<Output = ()> + 'static {
        struct Skip(LazyListState);
        impl Drop for Skip {
            fn drop(&mut self) {
                self.0.inner.skip_item_placement_animation.set(false);
            }
        }
        let state = self.clone();
        async move {
            state.inner.skip_item_placement_animation.set(true);
            let _skip = Skip(state.clone());
            let s = state.clone();
            state
                .scroll_block(
                    MutatePriority::Default,
                    Box::new(move |scope| {
                        Box::pin(async move {
                            let density = s.density();
                            let scope = LazyLayoutScrollScope(&s, scope);
                            animate_scroll_to_item(scope, index, scroll_offset, NumberOfItemsToTeleport, density).await;
                        })
                    }),
                )
                .await;
        }
    }
}

/// Items further than this are jumped to near them first.
const NumberOfItemsToTeleport: i32 = 100;

impl ScrollableState for LazyListState {
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
        Some(Rc::new(ListScrollIndicatorState(self.clone())))
    }
}

/// What a scrollbar shows of a list (Compose's LazyListState `scrollIndicatorState`): its content and offset
/// guessed from the average item.
struct ListScrollIndicatorState(LazyListState);

impl ListScrollIndicatorState {
    fn calculate_scroll_offset(&self) -> i32 {
        self.0.layout_info().visible_items_average_size() * self.0.first_visible_item_index()
            + self.0.first_visible_item_scroll_offset()
    }
}

impl ScrollIndicatorState for ListScrollIndicatorState {
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

/// The state's prefetch scope (Compose's `prefetchScope`): items composed and measured in the last measure's
/// child constraints.
struct ListPrefetchScope(Weak<LazyListStateInner>);

impl LazyListPrefetchScope for ListPrefetchScope {
    fn schedule_prefetch(
        &self,
        index: i32,
        on_prefetch_finished: Option<Rc<dyn Fn(&dyn LazyListPrefetchResultScope)>>,
    ) -> PrefetchHandle {
        let Some(inner) = self.0.upgrade() else {
            return LazyLayoutPrefetchState::default().schedule_precomposition(index);
        };
        // (not read: a scroll prefetching doesn't make what scrolls read the result)
        let last_measure_result = inner.layout_info.borrow().clone();
        let orientation = last_measure_result.orientation;
        let on_premeasured: Option<Rc<dyn Fn(&dyn PrefetchResultScope)>> = on_prefetch_finished.map(|on_finished| {
            Rc::new(move |result: &dyn PrefetchResultScope| {
                let main_axis_item_size: i32 = (0..result.placeables_count())
                    .map(|i| {
                        let size = result.get_size(i);
                        if orientation == Orientation::Vertical { size.height } else { size.width }
                    })
                    .sum();
                on_finished(&LazyListPrefetchResultScopeImpl {
                    index: result.index(),
                    main_axis_size: main_axis_item_size,
                });
            }) as Rc<dyn Fn(&dyn PrefetchResultScope)>
        });
        inner.prefetch_state.schedule_precomposition_and_premeasure_with_priority(
            index,
            last_measure_result.child_constraints,
            inner.execute_requests_in_high_priority_mode.get(),
            on_premeasured,
        )
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Remembering

/// A state scrolled to item `initial_first_visible_item_index`, remembered and saved where it is called (Compose's
/// `rememberLazyListState`).
#[kompozable]
pub fn remember_lazy_list_state(
    #[default(0)] initial_first_visible_item_index: i32,
    #[default(0)] initial_first_visible_item_scroll_offset: i32,
) -> LazyListState {
    remember_saveable_with(&LazyListState::saver(), move || {
        LazyListState::new(initial_first_visible_item_index, initial_first_visible_item_scroll_offset)
    })
}

/// The same with a prefetch strategy (Compose's `rememberLazyListState(…, prefetchStrategy)`).
/// Kotlin: `rememberLazyListState(initialFirstVisibleItemIndex: Int = 0, initialFirstVisibleItemScrollOffset: Int = 0, prefetchStrategy: LazyListPrefetchStrategy = remember { LazyListPrefetchStrategy() })`
#[kompozable]
pub fn remember_lazy_list_state_with_prefetch_strategy(
    #[default(0)] initial_first_visible_item_index: i32,
    #[default(0)] initial_first_visible_item_scroll_offset: i32,
    #[default(None)] prefetch_strategy: Option<Rc<dyn LazyListPrefetchStrategy>>,
) -> LazyListState {
    let prefetch_strategy = match prefetch_strategy {
        Some(s) => s,
        None => remember(lazy_list_prefetch_strategy),
    };
    let strategy = prefetch_strategy.clone();
    let saver =
        LazyListState::saver_with(Rc::new(move |i, o| LazyListState::with_prefetch_strategy(i, o, strategy.clone())));
    remember_saveable_keyed_with(StrategyKey(prefetch_strategy.clone()), &saver, move || {
        LazyListState::with_prefetch_strategy(
            initial_first_visible_item_index,
            initial_first_visible_item_scroll_offset,
            prefetch_strategy,
        )
    })
}

/// The same with a cache window (Compose's `rememberLazyListState(cacheWindow, …)`).
/// Kotlin: `rememberLazyListState(cacheWindow: LazyLayoutCacheWindow, initialFirstVisibleItemIndex: Int = 0, initialFirstVisibleItemScrollOffset: Int = 0)`
#[kompozable]
pub fn remember_lazy_list_state_with_cache_window(
    cache_window: Rc<dyn LazyLayoutCacheWindow>,
    #[default(0)] initial_first_visible_item_index: i32,
    #[default(0)] initial_first_visible_item_scroll_offset: i32,
) -> LazyListState {
    let window = cache_window.clone();
    let saver = LazyListState::saver_with(Rc::new(move |i, o| LazyListState::with_cache_window(window.clone(), i, o)));
    remember_saveable_keyed_with(WindowKey(cache_window.clone()), &saver, move || {
        LazyListState::with_cache_window(
            cache_window,
            initial_first_visible_item_index,
            initial_first_visible_item_scroll_offset,
        )
    })
}

/// A strategy as a key: the same one.
#[derive(Clone)]
struct StrategyKey(Rc<dyn LazyListPrefetchStrategy>);

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

impl LazyLayoutScrollScopeState for LazyListState {
    /// A list's scroll scope as a lazy layout's (Compose's `LazyLayoutScrollScope(state: LazyListState,
    /// scrollScope)`): what animating to an item scrolls in.
    fn lazy_layout_scroll_scope(&self, scroll_scope: Rc<dyn ScrollScope>) -> Rc<dyn LazyLayoutScrollScope> {
        Rc::new(ListScrollScope { state: self.clone(), scroll_scope })
    }
}

struct ListScrollScope {
    state: LazyListState,
    scroll_scope: Rc<dyn ScrollScope>,
}

impl ScrollScope for ListScrollScope {
    fn scroll_by(&self, pixels: f32) -> f32 {
        self.scroll_scope.scroll_by(pixels)
    }
}

impl LazyLayoutScrollScope for ListScrollScope {
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
            let average_size = layout_info.visible_items_average_size();
            let indexes_diff = target_index - first;
            average_size * indexes_diff - self.first_visible_item_scroll_offset()
        } else {
            layout_info.items().iter().find(|i| i.index == target_index).map_or(0, |i| i.offset)
        };
        distance + target_offset
    }
}
