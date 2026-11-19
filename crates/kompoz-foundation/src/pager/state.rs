//! A pager's state (Compose's PagerState.kt, PagerScrollPosition.kt, PagerScrollScope.kt, PagerCacheWindowLogic.kt,
//! LazyLayoutSemanticState.kt, PagerBeyondBoundsModifier.kt): the current page (the one closest to its snap
//! position) and how far it is from there as a fraction of a page, kept by the page's key as pages change; the page
//! it last settled on and the page it is heading to; what its last layout showed; scrolling (a `ScrollableState`
//! of whole pixels, kept between its first and last snap positions, measured again only when the pages in view
//! change); scrolling to a page at once or animated; prefetching through a cache window of one page ahead.

use super::layout_info::PagerLayoutInfo;
use super::measure::{MaxPageOffset, MinPageOffset, PagerMeasureResult};
use crate::gestures::{
    DefaultScrollableState, FlingBehavior, LocalFuture, Orientation, ScrollBlock, ScrollIndicatorState, ScrollScope,
    ScrollableState,
};
use crate::interaction::{InteractionSource, MutableInteractionSource};
use crate::lazy::layout::{
    AwaitFirstLayoutModifier, CacheWindowLogic, CacheWindowScope, InvalidIndex, LazyKey, LazyLayoutBeyondBoundsInfo,
    LazyLayoutBeyondBoundsState, LazyLayoutCacheWindow, LazyLayoutItemProvider, LazyLayoutNearestRangeState,
    LazyLayoutPinnedItemList, LazyLayoutPrefetchState, LazyLayoutScrollScope, LazyLayoutScrollScopeState,
    LazyLayoutSemanticState, NestedPrefetchScope, ObservableScopeInvalidator, PrefetchHandle, PrefetchResultScope,
    Remeasurement, find_index_by_key,
};
use kompoz_animation::{AnimationSpec, MutatePriority, animate, spring};
use kompoz_graphics::{Constraints, Density, Dp, Offset};
use kompoz_runtime::{
    DerivedState, MutableState, SavedValue, Saver, derived_state_of, kompozable, mutable_state_of, named,
    remember_saveable_with, without_read_observation,
};
use kompoz_ui::semantics::CollectionInfo;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::rc::{Rc, Weak};
use std::sync::Arc;

/// The items whose keys are known around the current page: a window of 30, and 100 more each side.
const NearestItemsSlidingWindowSize: i32 = 30;
const NearestItemsExtraItemCount: i32 = 100;

/// A drag of at least this (or half a page) counts as turning the page (Compose's `DefaultPositionThreshold`).
const DefaultPositionThreshold: Dp = Dp(56.0);
/// An animated scroll further than this jumps close first (Compose's `MaxPagesForAnimateScroll`).
const MaxPagesForAnimateScroll: i32 = 3;

// ---------------------------------------------------------------------------------------------------------------
// The scroll position

/// Where a pager is scrolled to (Compose's PagerScrollPosition): the current page and its offset fraction
/// (states), the current page's key (to find it again when pages change), the range of keys known.
struct PagerScrollPosition {
    current_page: MutableState<i32>,
    current_page_offset_fraction: MutableState<f32>,
    had_first_not_empty_layout: Cell<bool>,
    last_known_current_page_key: RefCell<Option<LazyKey>>,
    nearest_range_state: LazyLayoutNearestRangeState,
}

impl PagerScrollPosition {
    fn new(current_page: i32, current_page_offset_fraction: f32) -> Self {
        PagerScrollPosition {
            current_page: mutable_state_of(current_page),
            current_page_offset_fraction: mutable_state_of(current_page_offset_fraction),
            had_first_not_empty_layout: Cell::new(false),
            last_known_current_page_key: RefCell::new(None),
            nearest_range_state: LazyLayoutNearestRangeState::new(
                current_page,
                NearestItemsSlidingWindowSize,
                NearestItemsExtraItemCount,
            ),
        }
    }

    fn update_from_measure_result(&self, result: &PagerMeasureResult) {
        *self.last_known_current_page_key.borrow_mut() = result.current_page.as_ref().map(|p| p.key.clone());
        // (the initial page kept until a measure with pages)
        if self.had_first_not_empty_layout.get() || !result.visible_pages_info.is_empty() {
            self.had_first_not_empty_layout.set(true);
            self.update(result.current_page.as_ref().map_or(0, |p| p.index), result.current_page_offset_fraction);
        }
    }

    fn request_position_and_forget_last_known_key(&self, index: i32, offset_fraction: f32) {
        self.update(index, offset_fraction);
        *self.last_known_current_page_key.borrow_mut() = None;
    }

    /// The current page found again by its key (Compose's `matchPageWithKey`).
    fn match_page_with_key(&self, item_provider: &dyn LazyLayoutItemProvider, index: i32) -> i32 {
        let key = self.last_known_current_page_key.borrow().clone();
        let new_index = find_index_by_key(item_provider, key.as_ref(), index);
        if index != new_index {
            self.current_page.set(new_index);
            self.nearest_range_state.update(index);
        }
        new_index
    }

    fn update(&self, page: i32, offset_fraction: f32) {
        self.current_page.set(page);
        self.nearest_range_state.update(page);
        self.current_page_offset_fraction.set(offset_fraction);
    }

    fn update_current_page_offset_fraction(&self, offset_fraction: f32) {
        self.current_page_offset_fraction.set(offset_fraction);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The state

/// A pager's state (Compose's PagerState): its current, settled and target pages, how far the current page is from
/// where it snaps, what its last layout showed, scrolling it. A [`ScrollableState`]; `scroll_to_page`,
/// `animate_scroll_to_page`, `request_scroll_to_page`. Made by [`PagerState()`](fn@PagerState) or remembered and
/// saved by [`remember_pager_state`]; its page count is a function read where needed (a state's change is seen).
/// Clones are the same state; two are equal when they are.
#[derive(Clone)]
pub struct PagerState {
    pub(crate) inner: Rc<PagerStateInner>,
}

pub(crate) struct PagerStateInner {
    page_count: RefCell<Rc<dyn Fn() -> i32>>,
    page_count_generation: MutableState<u64>,
    /// Whether it was measured in a lookahead pass (Compose's `hasLookaheadOccurred`): in a lookahead scope.
    has_lookahead_occurred: Cell<bool>,
    /// The last approach pass's result, in a lookahead scope (Compose's `approachLayoutInfo`).
    approach_layout_info: RefCell<Option<Rc<PagerMeasureResult>>>,
    /// The last gesture's move from its down to its up (Compose's `upDownDifference`).
    pub(crate) up_down_difference: MutableState<Offset>,
    scroll_position: PagerScrollPosition,
    first_visible_page: Cell<i32>,
    first_visible_page_offset: Cell<i32>,
    max_scroll_offset: Cell<i64>,
    min_scroll_offset: Cell<i64>,
    accumulator: Cell<f32>,
    previous_pass_delta: Cell<f32>,
    scrollable_state: RefCell<Option<DefaultScrollableState>>,
    layout_with_measurement: Cell<i32>,
    layout_without_measurement: Cell<i32>,
    pub(crate) prefetching_enabled: Cell<bool>,
    layout_info: RefCell<Rc<PagerMeasureResult>>,
    layout_info_generation: MutableState<u64>,
    measured: Cell<bool>,
    pub(crate) density: Cell<Density>,
    latest_page_size_with_spacing: Cell<i32>,
    internal_interaction_source: MutableInteractionSource,
    programmatic_scroll_target_page: MutableState<i32>,
    settled_page_state: MutableState<i32>,
    /// Compose's `settledPage` and `targetPage`: derived states (structural equality).
    settled_page: DerivedState<i32>,
    target_page: DerivedState<i32>,
    pub(crate) prefetch_state: LazyLayoutPrefetchState,
    pub(crate) cache_window_logic: Rc<CacheWindowLogic>,
    cache_window_scope: Rc<PagerCacheWindowScope>,
    pub(crate) beyond_bounds_info: Rc<LazyLayoutBeyondBoundsInfo>,
    pub(crate) await_layout_modifier: AwaitFirstLayoutModifier,
    pub(crate) remeasurement: Remeasurement,
    pub(crate) premeasure_constraints: Cell<Constraints>,
    pub(crate) pinned_pages: LazyLayoutPinnedItemList,
    pub(crate) placement_scope_invalidator: ObservableScopeInvalidator,
    pub(crate) measurement_scope_invalidator: ObservableScopeInvalidator,
    can_scroll_forward: MutableState<bool>,
    can_scroll_backward: MutableState<bool>,
    is_last_scroll_forward: MutableState<bool>,
    is_last_scroll_backward: MutableState<bool>,
    /// The fling the pager composed with flings with (the pages' nested scroll connection settles the pager with
    /// what an inner fling leaves: the C++ Kompoz's behaviour, see `PagerDefaults::page_nested_scroll_connection`).
    pub(crate) fling_behavior: RefCell<Option<Rc<dyn FlingBehavior>>>,
}

impl PartialEq for PagerState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for PagerState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "PagerState(currentPage={}, currentPageOffsetFraction={})",
            self.inner.scroll_position.current_page.peek(),
            self.inner.scroll_position.current_page_offset_fraction.peek()
        )
    }
}

/// A pager's state at `current_page`, `current_page_offset_fraction` of a page from where it snaps (-0.5 to 0.5),
/// of `page_count` pages (Compose's `PagerState(currentPage, currentPageOffsetFraction, pageCount)`).
/// Kotlin: `PagerState(currentPage: Int = 0, currentPageOffsetFraction: Float = 0f, pageCount: () -> Int)`
pub fn PagerState(
    current_page: i32,
    current_page_offset_fraction: f32,
    page_count: impl Fn() -> i32 + 'static,
) -> PagerState {
    PagerState::new(current_page, current_page_offset_fraction, Rc::new(page_count))
}

/// The pager's cache window: one page ahead, none behind (Compose's `pagerCacheWindow`).
struct PagerCacheWindow(Weak<PagerStateInner>);

impl LazyLayoutCacheWindow for PagerCacheWindow {
    fn calculate_ahead_window(&self, _density: &Density, _viewport: i32) -> i32 {
        self.0.upgrade().map_or(0, |s| s.latest_page_size_with_spacing.get())
    }
    fn calculate_behind_window(&self, _density: &Density, _viewport: i32) -> i32 {
        0
    }
}

impl PagerState {
    /// A state at `current_page`, of the pages `page_count` tells.
    pub fn new(current_page: i32, current_page_offset_fraction: f32, page_count: Rc<dyn Fn() -> i32>) -> PagerState {
        assert!(
            (-0.5..=0.5).contains(&current_page_offset_fraction),
            "currentPageOffsetFraction {current_page_offset_fraction} is not within the range -0.5 to 0.5"
        );
        let inner = Rc::new_cyclic(|me: &Weak<PagerStateInner>| {
            let weak = me.clone();
            let prefetch_state = LazyLayoutPrefetchState::new(Some(Rc::new(move |scope: &dyn NestedPrefetchScope| {
                if let Some(s) = weak.upgrade() {
                    // (the first visible page, as a nested pager)
                    scope.schedule_precomposition(without_read_observation(|| s.first_visible_page.get()));
                }
            })));
            let window: Rc<dyn LazyLayoutCacheWindow> = Rc::new(PagerCacheWindow(me.clone()));
            // (computed through the state, which holds them: read only while it lives)
            let state_of = |me: &Weak<PagerStateInner>| {
                let me = me.clone();
                move || PagerState { inner: me.upgrade().expect("a pager state's derived state read while it lives") }
            };
            let settled = state_of(me);
            let settled_page = derived_state_of(move || {
                let s = settled();
                if s.is_scroll_in_progress() { s.inner.settled_page_state.get() } else { s.current_page() }
            });
            let target = state_of(me);
            let target_page = derived_state_of(move || target().calculate_target_page());
            PagerStateInner {
                page_count: RefCell::new(page_count),
                page_count_generation: mutable_state_of(0),
                has_lookahead_occurred: Cell::new(false),
                approach_layout_info: RefCell::new(None),
                up_down_difference: mutable_state_of(Offset::Zero),
                scroll_position: PagerScrollPosition::new(current_page, current_page_offset_fraction),
                first_visible_page: Cell::new(current_page),
                first_visible_page_offset: Cell::new(0),
                max_scroll_offset: Cell::new(i64::MAX),
                min_scroll_offset: Cell::new(0),
                accumulator: Cell::new(0.0),
                previous_pass_delta: Cell::new(0.0),
                scrollable_state: RefCell::new(None),
                layout_with_measurement: Cell::new(0),
                layout_without_measurement: Cell::new(0),
                prefetching_enabled: Cell::new(true),
                layout_info: RefCell::new(Rc::new(PagerMeasureResult::empty())),
                layout_info_generation: mutable_state_of(0),
                measured: Cell::new(false),
                density: Cell::new(Density::new(1.0, 1.0)),
                latest_page_size_with_spacing: Cell::new(0),
                internal_interaction_source: MutableInteractionSource::new(),
                programmatic_scroll_target_page: mutable_state_of(-1),
                settled_page_state: mutable_state_of(current_page),
                settled_page,
                target_page,
                cache_window_logic: CacheWindowLogic::new(window, false),
                cache_window_scope: Rc::new(PagerCacheWindowScope {
                    state: me.clone(),
                    layout_info: RefCell::new(Rc::new(PagerMeasureResult::empty())),
                }),
                prefetch_state,
                beyond_bounds_info: Rc::new(LazyLayoutBeyondBoundsInfo::default()),
                await_layout_modifier: AwaitFirstLayoutModifier::default(),
                remeasurement: Remeasurement::default(),
                premeasure_constraints: Cell::new(Constraints::default()),
                pinned_pages: LazyLayoutPinnedItemList::new(),
                placement_scope_invalidator: ObservableScopeInvalidator::default(),
                measurement_scope_invalidator: ObservableScopeInvalidator::default(),
                can_scroll_forward: mutable_state_of(false),
                can_scroll_backward: mutable_state_of(false),
                is_last_scroll_forward: mutable_state_of(false),
                is_last_scroll_backward: mutable_state_of(false),
                fling_behavior: RefCell::new(None),
            }
        });
        let weak = Rc::downgrade(&inner);
        *inner.scrollable_state.borrow_mut() =
            Some(crate::gestures::ScrollableState(move |delta| match weak.upgrade() {
                Some(inner) => PagerState { inner }.perform_scroll(delta),
                None => 0.0,
            }));
        PagerState { inner }
    }

    fn scrollable_state(&self) -> DefaultScrollableState {
        self.inner.scrollable_state.borrow().clone().expect("its scrollable state")
    }

    /// How many pages it has (Compose's `pageCount`; what its function reads is observed).
    pub fn page_count(&self) -> i32 {
        self.inner.page_count_generation.get();
        let page_count = self.inner.page_count.borrow().clone();
        page_count()
    }

    /// A new page count function (Compose's `pageCountState.value = pageCount`): its readers told when it gives
    /// another count than the last one (Compose's state of the lambda changes with the lambda).
    pub(crate) fn set_page_count(&self, page_count: Rc<dyn Fn() -> i32>) {
        let old = self.inner.page_count.replace(page_count.clone());
        let changed = without_read_observation(|| old() != page_count());
        if changed {
            self.inner.page_count_generation.set(self.inner.page_count_generation.peek().wrapping_add(1));
        }
    }

    /// The page closest to its snap position (a state; Compose's `currentPage`).
    pub fn current_page(&self) -> i32 {
        self.inner.scroll_position.current_page.get()
    }

    /// How far the current page is from its snap position, a fraction of a page from -0.5 to 0.5 (a state; Compose's
    /// `currentPageOffsetFraction`).
    pub fn current_page_offset_fraction(&self) -> f32 {
        self.inner.scroll_position.current_page_offset_fraction.get()
    }

    /// The page it last settled on: the current page when not scrolling (Compose's `settledPage`, a derived
    /// state: up to date when read right after a scroll, its readers told only when it changes).
    pub fn settled_page(&self) -> i32 {
        self.inner.settled_page.get()
    }

    /// The page a scroll heads to: an animated scroll's or fling's target, a drag's next page once past the
    /// threshold, else the current page (Compose's `targetPage`, a derived state as `settled_page`).
    pub fn target_page(&self) -> i32 {
        self.inner.target_page.get()
    }

    fn calculate_target_page(&self) -> i32 {
        let final_page = if !self.is_scroll_in_progress() {
            self.current_page()
        } else if self.inner.programmatic_scroll_target_page.get() != -1 {
            self.inner.programmatic_scroll_target_page.get()
        } else if self.current_page_offset_fraction().abs() >= self.position_threshold_fraction().abs() {
            // (a scroll: past the threshold, the next page its way)
            if self.last_scrolled_forward() {
                self.inner.first_visible_page.get() + 1
            } else {
                self.inner.first_visible_page.get()
            }
        } else {
            self.current_page()
        };
        self.coerce_in_page_range(final_page)
    }

    /// What the last layout showed (Compose's `layoutInfo`; a state).
    pub fn layout_info(&self) -> PagerLayoutInfo {
        self.inner.layout_info_generation.get();
        self.inner.layout_info.borrow().layout_info()
    }

    fn set_measure_result(&self, result: Rc<PagerMeasureResult>) {
        *self.inner.layout_info.borrow_mut() = result;
        self.inner.measured.set(true);
        self.inner.layout_info_generation.set(self.inner.layout_info_generation.peek().wrapping_add(1));
    }

    /// The last measure's result (a state read).
    fn measure_result(&self) -> Rc<PagerMeasureResult> {
        self.inner.layout_info_generation.get();
        self.inner.layout_info.borrow().clone()
    }

    /// The drags of the pager (Compose's `interactionSource`).
    pub fn interaction_source(&self) -> InteractionSource {
        self.inner.internal_interaction_source.as_interaction_source()
    }

    pub(crate) fn internal_interaction_source(&self) -> MutableInteractionSource {
        self.inner.internal_interaction_source.clone()
    }

    pub(crate) fn page_spacing(&self) -> i32 {
        self.measure_result().page_spacing
    }

    pub(crate) fn page_size(&self) -> i32 {
        self.measure_result().page_size
    }

    pub(crate) fn page_size_with_spacing(&self) -> i32 {
        self.page_size() + self.page_spacing()
    }

    pub(crate) fn density(&self) -> Density {
        self.inner.density.get()
    }

    pub(crate) fn first_visible_page(&self) -> i32 {
        self.inner.first_visible_page.get()
    }

    pub(crate) fn first_visible_page_offset(&self) -> i32 {
        self.inner.first_visible_page_offset.get()
    }

    pub(crate) fn min_scroll_offset(&self) -> i64 {
        self.inner.min_scroll_offset.get()
    }

    pub(crate) fn max_scroll_offset(&self) -> i64 {
        self.inner.max_scroll_offset.get()
    }

    /// How much of a page a drag turns it by (Compose's `positionThresholdFraction`): 56 dp or half a page.
    pub(crate) fn position_threshold_fraction(&self) -> f32 {
        let page_size = self.page_size() as f32;
        let min_threshold = self.density().to_px(DefaultPositionThreshold).min(page_size / 2.0);
        min_threshold / page_size
    }

    /// The items whose keys are known (a state).
    pub(crate) fn nearest_range(&self) -> std::ops::Range<i32> {
        self.inner.scroll_position.nearest_range_state.value()
    }

    /// The scroll position in px (Compose's `currentAbsoluteScrollOffset`).
    pub(crate) fn current_absolute_scroll_offset(&self) -> i64 {
        let page_size_with_spacing = self.page_size_with_spacing() as i64;
        let current_page_offset = self.current_page() as i64 * page_size_with_spacing;
        let offset_fraction =
            (self.current_page_offset_fraction() as f64 * page_size_with_spacing as f64).round() as i64;
        current_page_offset + offset_fraction
    }

    /// Takes a scroll of `delta` px (Compose's `performScroll`): in whole pixels (the fractions accumulated), kept
    /// within the first and last snap positions; applied without a measure when the pages in view stay.
    fn perform_scroll(&self, delta: f32) -> f32 {
        let inner = &self.inner;
        let current_scroll_position = self.current_absolute_scroll_offset();
        let decimal_accumulation = delta + inner.accumulator.get();
        let decimal_accumulation_int = decimal_accumulation.round() as i64;
        inner.accumulator.set(decimal_accumulation - decimal_accumulation_int as f32);

        // (nothing to scroll)
        if delta.abs() < 1e-4 {
            return delta;
        }
        let updated_scroll_position = current_scroll_position + decimal_accumulation_int;
        let coerced_scroll_position =
            updated_scroll_position.clamp(inner.min_scroll_offset.get(), inner.max_scroll_offset.get());
        let changed = updated_scroll_position != coerced_scroll_position;
        let scroll_delta = coerced_scroll_position - current_scroll_position;
        inner.previous_pass_delta.set(scroll_delta as f32);
        if scroll_delta != 0 {
            inner.is_last_scroll_forward.set(scroll_delta > 0);
            inner.is_last_scroll_backward.set(scroll_delta < 0);
        }

        let current = inner.layout_info.borrow().clone();
        let mut scrolled_layout_info =
            current.copy_with_scroll_delta_without_remeasure(-(scroll_delta as i32)).map(Rc::new);
        let approach = inner.approach_layout_info.borrow().clone();
        if scrolled_layout_info.is_some()
            && let Some(approach) = approach
        {
            // (the lookahead's scrolled without a measure: the approach's too if it can be — they may differ — else
            // both are measured)
            match approach.copy_with_scroll_delta_without_remeasure(-(scroll_delta as i32)) {
                Some(scrolled_approach) => *inner.approach_layout_info.borrow_mut() = Some(Rc::new(scrolled_approach)),
                None => scrolled_layout_info = None,
            }
        }
        match scrolled_layout_info {
            Some(scrolled) => {
                self.apply_measure_result(scrolled, inner.has_lookahead_occurred.get(), true);
                // (no measure needed: placed again only)
                inner.placement_scope_invalidator.invalidate_scope();
                inner.layout_without_measurement.set(inner.layout_without_measurement.get() + 1);
            }
            None => {
                self.apply_scroll_delta_to_position(scroll_delta as i32);
                inner.remeasurement.force_remeasure();
                inner.layout_with_measurement.set(inner.layout_with_measurement.get() + 1);
            }
        }
        // (what it took)
        if changed { scroll_delta as f32 } else { delta }
    }

    /// The position moved by `delta` px (Compose's PagerScrollPosition's `applyScrollDelta`).
    fn apply_scroll_delta_to_position(&self, delta: i32) {
        let page_size_with_spacing = self.page_size_with_spacing();
        let fraction_update =
            if page_size_with_spacing == 0 { 0.0 } else { delta as f32 / page_size_with_spacing as f32 };
        let position = &self.inner.scroll_position;
        position.current_page_offset_fraction.set(position.current_page_offset_fraction.peek() + fraction_update);
    }

    /// Jumps to `page`, `offset_fraction` from its snap position (Compose's `snapToItem`): measured at once, or at
    /// the next measure.
    pub(crate) fn snap_to_item(&self, page: i32, offset_fraction: f32, force_remeasure: bool) {
        let position = &self.inner.scroll_position;
        let position_changed =
            position.current_page.peek() != page || position.current_page_offset_fraction.peek() != offset_fraction;
        if position_changed {
            // (the cache window refilled at the next scroll)
            self.inner.cache_window_logic.reset_strategy();
        }
        position.request_position_and_forget_last_known_key(page, offset_fraction);
        if force_remeasure {
            self.inner.remeasurement.force_remeasure();
        } else {
            self.inner.measurement_scope_invalidator.invalidate_scope();
        }
    }

    /// Takes a measure's result (Compose's `applyMeasureResult`).
    pub(crate) fn apply_measure_result(
        &self,
        result: Rc<PagerMeasureResult>,
        is_looking_ahead: bool,
        visible_items_stayed_the_same: bool,
    ) {
        let inner = &self.inner;
        // (the number of items a nested prefetch of this pager prefetches)
        inner.prefetch_state.set_ideal_nested_prefetch_count(result.visible_pages_info.len() as i32);
        inner.latest_page_size_with_spacing.set(result.page_size + result.page_spacing);
        if !is_looking_ahead && inner.has_lookahead_occurred.get() {
            // (after a lookahead pass: the approach's result, kept apart)
            *inner.approach_layout_info.borrow_mut() = Some(result);
            return;
        }
        if is_looking_ahead {
            inner.has_lookahead_occurred.set(true);
        }
        if visible_items_stayed_the_same {
            inner.scroll_position.update_current_page_offset_fraction(result.current_page_offset_fraction);
        } else {
            inner.scroll_position.update_from_measure_result(&result);
            if inner.prefetching_enabled.get() {
                self.cache_window_on_visible_items_changed(result.clone());
            }
        }
        self.set_measure_result(result.clone());
        inner.can_scroll_forward.set(result.can_scroll_forward);
        inner.can_scroll_backward.set(result.can_scroll_backward());
        if let Some(first) = &result.first_visible_page {
            inner.first_visible_page.set(first.index);
        }
        inner.first_visible_page_offset.set(result.first_visible_page_scroll_offset);
        self.try_run_prefetch(&result);
        let page_count = without_read_observation(|| self.page_count());
        let max = result.layout_info().calculate_new_max_scroll_offset(page_count);
        inner.max_scroll_offset.set(max);
        inner.min_scroll_offset.set(calculate_new_min_scroll_offset(&result, page_count).min(max));
    }

    /// Prefetches as a scroll goes (Compose's `tryRunPrefetch`, its cache window on): not when all pages are
    /// composed anyway, nor for under half a pixel, nor against the gesture.
    fn try_run_prefetch(&self, result: &Rc<PagerMeasureResult>) {
        without_read_observation(|| {
            let inner = &self.inner;
            if !inner.prefetching_enabled.get() {
                return;
            }
            if result.beyond_viewport_page_count >= self.page_count() {
                return;
            }
            let previous_pass_delta = inner.previous_pass_delta.get();
            if previous_pass_delta.abs() <= 0.5 {
                return;
            }
            if !self.is_gesture_action_matches_scroll(previous_pass_delta) {
                return;
            }
            self.cache_window_on_scroll(previous_pass_delta, result.clone());
        });
    }

    /// Whether the scroll goes the gesture's way, or there is no gesture (Compose's `isGestureActionMatchesScroll`).
    fn is_gesture_action_matches_scroll(&self, scroll_delta: f32) -> bool {
        let difference = self.inner.up_down_difference.get();
        let matches = if self.layout_info().orientation == Orientation::Vertical {
            sign(scroll_delta) == sign(-difference.y)
        } else {
            sign(scroll_delta) == sign(-difference.x)
        };
        matches || self.is_not_gesture_action()
    }

    /// No gesture moved it (Compose's `isNotGestureAction`).
    pub(crate) fn is_not_gesture_action(&self) -> bool {
        let difference = self.inner.up_down_difference.get();
        difference.x as i32 == 0 && difference.y as i32 == 0
    }

    /// Compose's PagerCacheWindowLogic `onScroll`: the window filled the scroll's way (its delta the other sign).
    fn cache_window_on_scroll(&self, delta: f32, result: Rc<PagerMeasureResult>) {
        *self.inner.cache_window_scope.layout_info.borrow_mut() = result;
        let scope: Rc<dyn CacheWindowScope> = self.inner.cache_window_scope.clone();
        self.inner.cache_window_logic.on_scroll(&scope, -delta);
    }

    /// Compose's PagerCacheWindowLogic `onVisibleItemsChanged`.
    fn cache_window_on_visible_items_changed(&self, result: Rc<PagerMeasureResult>) {
        *self.inner.cache_window_scope.layout_info.borrow_mut() = result;
        let scope: Rc<dyn CacheWindowScope> = self.inner.cache_window_scope.clone();
        self.inner.cache_window_logic.on_visible_items_updated(&scope);
    }

    fn coerce_in_page_range(&self, page: i32) -> i32 {
        let page_count = self.page_count();
        if page_count > 0 { page.clamp(0, page_count - 1) } else { 0 }
    }

    /// The current page found again by its key (Compose's `matchScrollPositionWithKey`).
    pub(crate) fn match_scroll_position_with_key(
        &self,
        item_provider: &dyn LazyLayoutItemProvider,
        current_page: i32,
    ) -> i32 {
        self.inner.scroll_position.match_page_with_key(item_provider, current_page)
    }

    /// How many pages `page` is from where the current one snaps (Compose's `getOffsetDistanceInPages`): read in a
    /// layer block, pages that move away can shrink.
    pub fn get_offset_distance_in_pages(&self, page: i32) -> f32 {
        let page_count = self.page_count();
        assert!((0..=page_count).contains(&page), "page {page} is not within the range 0 to {page_count}");
        page as f32 - self.current_page() as f32 - self.current_page_offset_fraction()
    }

    /// Makes `page` current in a scroll (Compose's `ScrollScope.updateCurrentPage`): at once.
    pub fn update_current_page(&self, _scope: &dyn ScrollScope, page: i32, page_offset_fraction: f32) {
        self.snap_to_item(page, page_offset_fraction, true);
    }

    /// Tells where a scroll heads (Compose's `ScrollScope.updateTargetPage`): the target page until it ends.
    pub fn update_target_page(&self, _scope: &dyn ScrollScope, target_page: i32) {
        let page = self.coerce_in_page_range(target_page);
        self.inner.programmatic_scroll_target_page.set(page);
    }

    /// Waits for the first layout (Compose's `awaitScrollDependencies`).
    async fn await_scroll_dependencies(&self) {
        if !self.inner.measured.get() {
            self.inner.await_layout_modifier.wait_for_first_layout().await;
        }
    }

    /// The saver of [`remember_pager_state`] (Compose's `DefaultPagerState.Saver`): the current page, its fraction
    /// and the page count (restored as a count that doesn't change until composed again).
    pub fn saver() -> Saver<PagerState> {
        Saver::new(
            |s: &PagerState| {
                SavedValue::list([
                    SavedValue::from(s.inner.scroll_position.current_page.peek()),
                    SavedValue::from(
                        s.inner.scroll_position.current_page_offset_fraction.peek().clamp(MinPageOffset, MaxPageOffset),
                    ),
                    SavedValue::from(without_read_observation(|| s.page_count())),
                ])
            },
            |v| {
                let list = v.as_list()?;
                let page = list.first()?.as_int()? as i32;
                let fraction = list.get(1)?.as_f64()? as f32;
                let count = list.get(2)?.as_int()? as i32;
                Some(PagerState::new(page, fraction, Rc::new(move || count)))
            },
        )
    }
}

#[named]
impl PagerState {
    /// Scrolls to `page` at once, `page_offset_fraction` from its snap position (Compose's `scrollToPage`).
    pub fn scroll_to_page(
        &self,
        page: i32,
        #[default(0.0)] page_offset_fraction: f32,
    ) -> impl Future<Output = ()> + 'static {
        let state = self.clone();
        async move {
            let s = state.clone();
            state
                .scroll_block(
                    MutatePriority::Default,
                    Box::new(move |_| {
                        Box::pin(async move {
                            s.await_scroll_dependencies().await;
                            assert!(
                                (-0.5..=0.5).contains(&page_offset_fraction),
                                "pageOffsetFraction {page_offset_fraction} is not within the range -0.5 to 0.5"
                            );
                            let target_page = s.coerce_in_page_range(page);
                            s.snap_to_item(target_page, page_offset_fraction, true);
                        })
                    }),
                )
                .await;
        }
    }

    /// Scrolls to `page` at the next measure, without a mutation (Compose's `requestScrollToPage`): a scroll
    /// running is cancelled.
    pub fn request_scroll_to_page(&self, page: i32, #[default(0.0)] page_offset_fraction: f32) {
        if self.is_scroll_in_progress() {
            let scope = self.inner.layout_info.borrow().coroutine_scope.clone();
            if let Some(scope) = scope {
                let state = self.clone();
                scope.launch(async move {
                    state.scroll_block(MutatePriority::Default, Box::new(|_| Box::pin(async {}))).await;
                });
            }
        }
        self.snap_to_item(page, page_offset_fraction, false);
    }

    /// Glides to `page`, `page_offset_fraction` from its snap position, along `animation_spec` (Compose's
    /// `animateScrollToPage`): a page more than 3 away jumped close first; it is the target page meanwhile.
    pub fn animate_scroll_to_page(
        &self,
        page: i32,
        #[default(0.0)] page_offset_fraction: f32,
        #[default(Arc::new(spring::<f32>()))] animation_spec: Arc<dyn AnimationSpec<f32>>,
    ) -> impl Future<Output = ()> + 'static {
        let state = self.clone();
        async move {
            if (page == state.current_page() && state.current_page_offset_fraction() == page_offset_fraction)
                || state.page_count() == 0
            {
                return;
            }
            state.await_scroll_dependencies().await;
            assert!(
                (-0.5..=0.5).contains(&page_offset_fraction),
                "pageOffsetFraction {page_offset_fraction} is not within the range -0.5 to 0.5"
            );
            let target_page = state.coerce_in_page_range(page);
            let target_page_offset_to_snapped_position = page_offset_fraction * state.page_size_with_spacing() as f32;
            let s = state.clone();
            state
                .scroll_block(
                    MutatePriority::Default,
                    Box::new(move |scope| {
                        Box::pin(async move {
                            let lazy_scope = LazyLayoutScrollScope(&s, scope.clone());
                            animate_scroll_to_page(
                                lazy_scope,
                                target_page,
                                target_page_offset_to_snapped_position,
                                animation_spec,
                                |page| s.update_target_page(&*scope, page),
                            )
                            .await;
                        })
                    }),
                )
                .await;
        }
    }
}

/// Glides a pager's scroll scope to `target_page` (Compose's `LazyLayoutScrollScope.animateScrollToPage`).
async fn animate_scroll_to_page(
    scope: Rc<dyn LazyLayoutScrollScope>,
    target_page: i32,
    target_page_offset_to_snapped_position: f32,
    animation_spec: Arc<dyn AnimationSpec<f32>>,
    update_target_page: impl Fn(i32),
) {
    update_target_page(target_page);
    let first = scope.first_visible_item_index();
    let last = scope.last_visible_item_index();
    let forward = target_page > first;
    let visible_pages = last - first + 1;
    if ((forward && target_page > last) || (!forward && target_page < first))
        && (target_page - first).abs() >= MaxPagesForAnimateScroll
    {
        // (a viewport away from the page first, if it can)
        let pre_jump_position =
            if forward { (target_page - visible_pages).max(first) } else { (target_page + visible_pages).min(first) };
        scope.snap_to_item(pre_jump_position, 0);
    }
    // (the pages' distance, less what the first one scrolled, plus the fraction asked for)
    let displacement = scope.calculate_distance_to(target_page, 0) as f32 + target_page_offset_to_snapped_position;
    let previous_value = Rc::new(Cell::new(0.0f32));
    let (s, p) = (scope.clone(), previous_value);
    kompoz_runtime::named_args!(animate(
        0.0,
        displacement,
        animation_spec = animation_spec,
        block = move |current_value: f32, _velocity: f32| {
            let delta = current_value - p.get();
            let consumed = s.scroll_by(delta);
            p.set(p.get() + consumed);
        }
    ))
    .await;
}

/// The least a pager can scroll (Compose's `calculateNewMinScrollOffset`): the first page's snap position.
fn calculate_new_min_scroll_offset(result: &PagerMeasureResult, page_count: i32) -> i64 {
    let viewport = result.viewport_size();
    let layout_size = if result.orientation == Orientation::Horizontal { viewport.width } else { viewport.height };
    result
        .snap_position
        .position(
            layout_size,
            result.page_size,
            result.before_content_padding(),
            result.after_content_padding,
            0,
            page_count,
        )
        .clamp(0, layout_size.max(0)) as i64
}

fn sign(v: f32) -> f32 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    }
}

impl ScrollableState for PagerState {
    /// Compose's `scroll`: after the first layout; the settled page kept as the current one when a scroll starts;
    /// the programmatic target forgotten when it ends.
    fn scroll_block(&self, scroll_priority: MutatePriority, block: ScrollBlock) -> LocalFuture<'_, ()> {
        let state = self.clone();
        Box::pin(async move {
            state.await_scroll_dependencies().await;
            if !state.is_scroll_in_progress() {
                state.inner.settled_page_state.set(state.current_page());
            }
            let scrollable = state.scrollable_state();
            scrollable.scroll_block(scroll_priority, block).await;
            state.inner.programmatic_scroll_target_page.set(-1);
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
        self.inner.is_last_scroll_forward.get()
    }

    fn last_scrolled_backward(&self) -> bool {
        self.inner.is_last_scroll_backward.get()
    }

    fn scroll_indicator_state(&self) -> Option<Rc<dyn ScrollIndicatorState>> {
        Some(Rc::new(PagerScrollIndicatorState(self.clone())))
    }
}

/// What a scrollbar shows of a pager (Compose's PagerState `scrollIndicatorState`).
struct PagerScrollIndicatorState(PagerState);

impl PagerScrollIndicatorState {
    fn calculate_scroll_offset(&self) -> i32 {
        let state = &self.0;
        let total = state.page_size_with_spacing() as i64 * state.first_visible_page() as i64
            + state.first_visible_page_offset() as i64;
        total.min(i32::MAX as i64) as i32
    }
}

impl ScrollIndicatorState for PagerScrollIndicatorState {
    fn scroll_offset(&self) -> i32 {
        let info = self.0.layout_info();
        if info.reverse_layout {
            info.calculate_content_size(self.0.page_count())
                - info.main_axis_viewport_size()
                - self.calculate_scroll_offset()
        } else {
            self.calculate_scroll_offset()
        }
    }
    fn content_size(&self) -> i32 {
        self.0.layout_info().calculate_content_size(self.0.page_count())
    }
    fn viewport_size(&self) -> i32 {
        self.0.layout_info().main_axis_viewport_size()
    }
}

/// A pager's state at `initial_page`, of the pages `page_count` tells, remembered and saved where it is called
/// (Compose's `rememberPagerState`): the page count function the latest given.
#[kompozable]
pub fn remember_pager_state(
    #[default(0)] initial_page: i32,
    #[default(0.0)] initial_page_offset_fraction: f32,
    page_count: impl Fn() -> i32 + 'static,
) -> PagerState {
    let page_count: Rc<dyn Fn() -> i32> = Rc::new(page_count);
    let first = page_count.clone();
    let state = remember_saveable_with(&PagerState::saver(), move || {
        PagerState::new(initial_page, initial_page_offset_fraction, first)
    });
    state.set_page_count(page_count);
    state
}

// ---------------------------------------------------------------------------------------------------------------
// The cache window

/// The cache window's view of a pager's last layout (Compose's PagerCacheWindowScope): the pages beyond the
/// viewport are lines too.
struct PagerCacheWindowScope {
    state: Weak<PagerStateInner>,
    layout_info: RefCell<Rc<PagerMeasureResult>>,
}

impl PagerCacheWindowScope {
    fn line_at(&self, index_in_visible_lines: i32) -> Option<(i32, LazyKey)> {
        let info = self.layout_info.borrow();
        let before = info.extra_pages_before.len() as i32;
        let visible = info.visible_pages_info.len() as i32;
        let page = if index_in_visible_lines < before {
            info.extra_pages_before.get(index_in_visible_lines as usize)
        } else if index_in_visible_lines < before + visible {
            info.visible_pages_info.get((index_in_visible_lines - before) as usize)
        } else {
            info.extra_pages_after.get((index_in_visible_lines - before - visible) as usize)
        };
        page.map(|p| (p.index, p.key.clone()))
    }
}

impl CacheWindowScope for PagerCacheWindowScope {
    fn total_items_count(&self) -> i32 {
        self.state.upgrade().map_or(0, |inner| PagerState { inner }.page_count())
    }
    fn visible_line_count(&self) -> i32 {
        let info = self.layout_info.borrow();
        (info.extra_pages_before.len() + info.visible_pages_info.len() + info.extra_pages_after.len()) as i32
    }
    fn has_visible_items(&self) -> bool {
        !self.layout_info.borrow().visible_pages_info.is_empty()
    }
    fn main_axis_extra_space_start(&self) -> i32 {
        let info = self.layout_info.borrow();
        let Some(first) = info.visible_pages_info.first() else { return 0 };
        // (how much of the first page peeks out at the start: positive)
        (first.offset() + info.before_content_padding()).min(0).abs()
    }
    fn main_axis_extra_space_end(&self) -> i32 {
        let info = self.layout_info.borrow();
        let Some(last) = info.visible_pages_info.last() else { return 0 };
        let last_item_overflow_offset = last.offset() + info.page_size + info.page_spacing;
        (last_item_overflow_offset - info.viewport_end_offset).abs()
    }
    fn first_visible_line_index(&self) -> i32 {
        let info = self.layout_info.borrow();
        let Some(first) = info.visible_pages_info.first() else { return InvalidIndex };
        (first.index as i64 - info.beyond_viewport_page_count as i64).max(0) as i32
    }
    fn last_visible_line_index(&self) -> i32 {
        let info = self.layout_info.borrow();
        let Some(last) = info.visible_pages_info.last() else { return InvalidIndex };
        let total = self.total_items_count() as i64;
        (last.index as i64 + info.beyond_viewport_page_count as i64).min(total - 1) as i32
    }
    fn main_axis_viewport_size(&self) -> i32 {
        self.layout_info.borrow().layout_info().main_axis_viewport_size()
    }
    fn density(&self) -> Option<Density> {
        Some(self.layout_info.borrow().density)
    }
    fn schedule_prefetch(&self, line_index: i32, on_item_prefetched: Rc<dyn Fn(i32, i32)>) -> Vec<PrefetchHandle> {
        let Some(inner) = self.state.upgrade() else { return Vec::new() };
        let (child_constraints, page_size) = {
            let info = self.layout_info.borrow();
            (info.child_constraints, info.page_size)
        };
        vec![inner.prefetch_state.schedule_precomposition_and_premeasure_with_priority(
            line_index,
            child_constraints,
            true,
            Some(Rc::new(move |result: &dyn PrefetchResultScope| on_item_prefetched(result.index(), page_size))),
        )]
    }
    fn get_visible_item_size(&self, _index_in_visible_lines: i32) -> i32 {
        self.layout_info.borrow().page_size
    }
    fn get_visible_item_line(&self, index_in_visible_lines: i32) -> i32 {
        self.line_at(index_in_visible_lines).map_or(InvalidIndex, |(index, _)| index)
    }
    fn get_visible_line_key(&self, index_in_visible_lines: i32) -> Option<LazyKey> {
        self.line_at(index_in_visible_lines).map(|(_, key)| key)
    }
    fn get_last_index_in_line(&self, line_index: i32) -> i32 {
        line_index
    }
    fn get_last_line_index(&self) -> i32 {
        if self.layout_info.borrow().visible_pages_info.is_empty() {
            return InvalidIndex;
        }
        self.total_items_count() - 1
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The scroll scope, semantics and beyond bounds

impl LazyLayoutScrollScopeState for PagerState {
    /// A pager's scroll scope as a lazy layout's (Compose's `LazyLayoutScrollScope(state: PagerState,
    /// scrollScope)`): by pages, the distance kept within the scroll's bounds.
    fn lazy_layout_scroll_scope(&self, scroll_scope: Rc<dyn ScrollScope>) -> Rc<dyn LazyLayoutScrollScope> {
        Rc::new(PagerScrollScope { state: self.clone(), scroll_scope })
    }
}

struct PagerScrollScope {
    state: PagerState,
    scroll_scope: Rc<dyn ScrollScope>,
}

impl ScrollScope for PagerScrollScope {
    fn scroll_by(&self, pixels: f32) -> f32 {
        self.scroll_scope.scroll_by(pixels)
    }
}

impl LazyLayoutScrollScope for PagerScrollScope {
    fn first_visible_item_index(&self) -> i32 {
        self.state.first_visible_page()
    }
    fn first_visible_item_scroll_offset(&self) -> i32 {
        self.state.first_visible_page_offset()
    }
    fn last_visible_item_index(&self) -> i32 {
        self.state.layout_info().pages().last().expect("List is empty.").index
    }
    fn item_count(&self) -> i32 {
        self.state.page_count()
    }
    fn snap_to_item(&self, index: i32, offset: i32) {
        let page_size_with_spacing = self.state.page_size_with_spacing() as f32;
        let offset_fraction = if page_size_with_spacing == 0.0 { 0.0 } else { offset as f32 / page_size_with_spacing };
        self.state.snap_to_item(index, offset_fraction, true);
    }
    fn calculate_distance_to(&self, target_index: i32, target_offset: i32) -> i32 {
        let state = &self.state;
        let page_size_with_spacing = state.page_size_with_spacing() as f32;
        let displacement = ((target_index - state.current_page()) as f32 * page_size_with_spacing
            - state.current_page_offset_fraction() * page_size_with_spacing
            + target_offset as f32)
            .round() as i64;
        // (kept within the layout's bounds)
        let current = state.current_absolute_scroll_offset();
        let current_scroll_with_displacement_applied =
            (current + displacement).clamp(state.min_scroll_offset(), state.max_scroll_offset());
        (current_scroll_with_displacement_applied - current) as i32
    }
}

/// What the pager's semantics read of its state (Compose's pager `LazyLayoutSemanticState`).
pub(crate) struct PagerSemanticState {
    pub(crate) state: PagerState,
    pub(crate) is_vertical: bool,
}

impl LazyLayoutSemanticState for PagerSemanticState {
    fn viewport(&self) -> i32 {
        let info = self.state.layout_info();
        if info.orientation == Orientation::Vertical { info.viewport_size.height } else { info.viewport_size.width }
    }
    fn content_padding(&self) -> i32 {
        let info = self.state.layout_info();
        info.before_content_padding + info.after_content_padding
    }
    fn scroll_offset(&self) -> f32 {
        self.state.current_absolute_scroll_offset() as f32
    }
    fn max_scroll_offset(&self) -> f32 {
        self.state.layout_info().calculate_new_max_scroll_offset(self.state.page_count()) as f32
    }
    fn collection_info(&self) -> CollectionInfo {
        let count = self.state.page_count();
        if self.is_vertical {
            CollectionInfo { row_count: count, column_count: 1 }
        } else {
            CollectionInfo { row_count: 1, column_count: count }
        }
    }
    fn scroll_to_item(&self, index: i32) -> LocalFuture<'static, ()> {
        let state = self.state.clone();
        Box::pin(async move { state.scroll_to_page(index).await })
    }
}

/// What the pager tells its beyond-bounds layout (Compose's PagerBeyondBoundsState).
pub(crate) struct PagerBeyondBoundsState {
    pub(crate) state: PagerState,
    pub(crate) beyond_viewport_page_count: i32,
}

impl LazyLayoutBeyondBoundsState for PagerBeyondBoundsState {
    fn item_count(&self) -> i32 {
        self.state.page_count()
    }
    fn has_visible_items(&self) -> bool {
        !self.state.layout_info().pages().is_empty()
    }
    fn first_placed_index(&self) -> i32 {
        (self.state.first_visible_page() - self.beyond_viewport_page_count).max(0)
    }
    fn last_placed_index(&self) -> i32 {
        let last = self.state.layout_info().pages().last().unwrap().index;
        (self.item_count() - 1).min(last + self.beyond_viewport_page_count)
    }
    fn items_per_viewport(&self) -> i32 {
        let info = self.state.layout_info();
        if info.pages().is_empty() {
            return 0;
        }
        let viewport_size = info.main_axis_viewport_size();
        let average_item_size = info.page_size + info.page_spacing;
        if average_item_size == 0 {
            return 1;
        }
        (viewport_size / average_item_size).max(1)
    }
}

/// To the next page, animated (Compose's `animateToNextPage`).
pub(crate) async fn animate_to_next_page(state: &PagerState) {
    if state.current_page() + 1 < state.page_count() {
        state.animate_scroll_to_page(state.current_page() + 1).await;
    }
}

/// To the previous page, animated (Compose's `animateToPreviousPage`).
pub(crate) async fn animate_to_previous_page(state: &PagerState) {
    if state.current_page() > 0 {
        state.animate_scroll_to_page(state.current_page() - 1).await;
    }
}
