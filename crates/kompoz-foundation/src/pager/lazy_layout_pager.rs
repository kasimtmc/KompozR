//! The pager both `HorizontalPager` and `VerticalPager` are (Compose's LazyLayoutPager.kt, PagerSemantics.kt): a
//! `LazyLayout` of the pages, its measure policy, its modifiers (remeasurement, waiting for the first layout,
//! semantics and paging actions, beyond-bounds layout, a scrollable area of its state with the pager's fling and
//! bring-into-view spec, the drag direction, the pages' nested scroll connection).

use super::layout_info::PageSize;
use super::measure::{PagerMeasureConfig, pager_measure_policy};
use super::pager::pager_semantics;
use super::state::{PagerBeyondBoundsState, PagerSemanticState, PagerState};
use crate::gestures::snapping::SnapPosition;
use crate::gestures::{
    AwaitPointerEventScopeGestures, BringIntoViewSpec, FlingBehavior, LocalBringIntoViewSpec, LocalFuture, Orientation,
    PointerInputScopeGestures, ScrollScope, ScrollableState, TargetedFlingBehavior,
};
use crate::layout::PaddingValues;
use crate::lazy::layout::{
    ItemProviderLambda, LazyKey, LazyLayout, LazyLayoutBeyondBoundsState, LazyLayoutInterval,
    LazyLayoutIntervalContent, LazyLayoutItemProvider, LazyLayoutKeyIndexMap, LazyLayoutPinnableItem,
    LazyLayoutSemanticState, MutableIntervalList, NearestRangeKeyIndexMap, lazy_layout_beyond_bounds_modifier,
    lazy_layout_semantics,
};
use crate::overscroll::OverscrollEffect;
use kompoz_coroutines::remember_coroutine_scope;
use kompoz_graphics::{Dp, LayoutDirection, Offset};
use kompoz_runtime::{
    DerivedState, derived_state_of_with_policy, kompozable, kompozable_block, never_equal_policy, remember_keyed,
    remember_updated_state,
};
use kompoz_ui::input::{NestedScrollConnection, PointerEventPass, PointerInputScope};
use kompoz_ui::layout::SlotKey;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::{HorizontalAlignment, Modifier, VerticalAlignment};
use std::any::Any;
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// The item provider

/// The pages' one interval (Compose's PagerIntervalContent): their keys and content.
#[doc(hidden)]
pub struct PagerIntervalContent {
    key: Option<Rc<dyn Fn(i32) -> LazyKey>>,
    item: Rc<dyn Fn(i32)>,
}

impl LazyLayoutInterval for PagerIntervalContent {
    fn key(&self, index: i32) -> Option<LazyKey> {
        self.key.as_ref().map(|k| k(index))
    }
}

/// Compose's PagerLayoutIntervalContent: all pages one interval.
struct PagerLayoutIntervalContent {
    intervals: MutableIntervalList<PagerIntervalContent>,
}

impl LazyLayoutIntervalContent for PagerLayoutIntervalContent {
    type Interval = PagerIntervalContent;
    fn intervals(&self) -> &MutableIntervalList<PagerIntervalContent> {
        &self.intervals
    }
}

/// Compose's PagerLazyLayoutItemProvider.
struct PagerLazyLayoutItemProvider {
    state: PagerState,
    interval_content: Rc<PagerLayoutIntervalContent>,
    key_index_map: Rc<dyn LazyLayoutKeyIndexMap>,
}

impl LazyLayoutItemProvider for PagerLazyLayoutItemProvider {
    fn item_count(&self) -> i32 {
        self.interval_content.item_count()
    }

    fn item(&self, index: i32, key: &LazyKey) {
        let content = self.interval_content.clone();
        let pinned_pages = self.state.inner.pinned_pages.clone();
        let key = key.clone();
        kompozable_block!({
            LazyLayoutPinnableItem(Some(key), index, pinned_pages, move || {
                content.with_interval(index, |local_index, interval| (interval.item)(local_index));
            });
        });
    }

    fn get_key(&self, index: i32) -> LazyKey {
        self.key_index_map.get_key(index).unwrap_or_else(|| self.interval_content.get_key(index))
    }

    fn get_content_type(&self, _index: i32) -> Option<SlotKey> {
        None
    }

    fn get_index(&self, key: &LazyKey) -> i32 {
        self.key_index_map.get_index(key)
    }

    fn same_items(&self, other: &dyn LazyLayoutItemProvider) -> bool {
        let other: &dyn Any = other;
        other
            .downcast_ref::<PagerLazyLayoutItemProvider>()
            .is_some_and(|o| Rc::ptr_eq(&o.interval_content, &self.interval_content))
    }
}

/// The pages as the layout reads them (Compose's `rememberPagerItemProviderLambda`): derived from the content, the
/// key and the page count (run again when they change), and the range of keys around the current page.
#[kompozable]
fn remember_pager_item_provider_lambda(
    state: PagerState,
    page_content: Rc<dyn Fn(i32)>,
    key: Option<Rc<dyn Fn(i32) -> LazyKey>>,
) -> ItemProviderLambda {
    let latest_content = remember_updated_state!(page_content);
    let latest_key = remember_updated_state!(key);
    remember_keyed(state.clone(), move || {
        let s = state.clone();
        let interval_content_state: DerivedState<Rc<PagerLayoutIntervalContent>> =
            derived_state_of_with_policy(never_equal_policy(), move || {
                let (item, key) = (latest_content.get(), latest_key.get());
                let mut intervals = MutableIntervalList::new();
                intervals.add_interval(s.page_count(), PagerIntervalContent { key, item });
                Rc::new(PagerLayoutIntervalContent { intervals })
            });
        let item_provider_state: DerivedState<Rc<PagerLazyLayoutItemProvider>> =
            derived_state_of_with_policy(never_equal_policy(), move || {
                let interval_content = interval_content_state.get();
                let map = NearestRangeKeyIndexMap::new(state.nearest_range(), &*interval_content);
                Rc::new(PagerLazyLayoutItemProvider {
                    state: state.clone(),
                    interval_content,
                    key_index_map: Rc::new(map),
                })
            });
        Rc::new(move || item_provider_state.get() as Rc<dyn LazyLayoutItemProvider>) as ItemProviderLambda
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The pager

/// The pager `HorizontalPager` and `VerticalPager` are (Compose's `Pager`).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn Pager(
    modifier: Modifier,
    state: PagerState,
    content_padding: PaddingValues,
    reverse_layout: bool,
    orientation: Orientation,
    fling_behavior: Rc<dyn TargetedFlingBehavior>,
    user_scroll_enabled: bool,
    overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
    beyond_viewport_page_count: i32,
    page_spacing: Dp,
    page_size: PageSize,
    page_nested_scroll_connection: Rc<dyn NestedScrollConnection>,
    key: Option<Rc<dyn Fn(i32) -> LazyKey>>,
    horizontal_alignment: HorizontalAlignment,
    vertical_alignment: VerticalAlignment,
    snap_position: SnapPosition,
    page_content: Rc<dyn Fn(i32)>,
) {
    assert!(
        beyond_viewport_page_count >= 0,
        "beyondViewportPageCount should be greater than or equal to 0, you selected {beyond_viewport_page_count}"
    );
    let pager_item_provider = remember_pager_item_provider_lambda(state.clone(), page_content, key);
    let coroutine_scope = remember_coroutine_scope();
    let measure_policy = {
        let keys = (
            state.clone(),
            content_padding.clone(),
            reverse_layout,
            orientation,
            horizontal_alignment.clone(),
            vertical_alignment.clone(),
            page_spacing,
            page_size.clone(),
            snap_position.clone(),
            beyond_viewport_page_count,
            coroutine_scope.clone(),
        );
        let (s, p, provider) = (state.clone(), content_padding.clone(), pager_item_provider.clone());
        let (ha, va, size, snap, scope) = (
            horizontal_alignment.clone(),
            vertical_alignment.clone(),
            page_size.clone(),
            snap_position.clone(),
            coroutine_scope.clone(),
        );
        remember_keyed(keys, move || {
            pager_measure_policy(PagerMeasureConfig {
                item_provider_lambda: provider,
                state: s,
                content_padding: p,
                reverse_layout,
                orientation,
                beyond_viewport_page_count,
                page_spacing,
                page_size: size,
                horizontal_alignment: Some(ha),
                vertical_alignment: Some(va),
                snap_position: snap,
                coroutine_scope: scope,
            })
        })
    };
    let semantic_state = {
        let s = state.clone();
        let is_vertical = orientation == Orientation::Vertical;
        remember_keyed((state.clone(), is_vertical), move || {
            Rc::new(PagerSemanticState { state: s, is_vertical }) as Rc<dyn LazyLayoutSemanticState>
        })
    };
    let resolved_fling_behavior = {
        let s = state.clone();
        let f = fling_behavior.clone();
        remember_keyed((state.clone(), Rc::as_ptr(&fling_behavior) as *const () as usize), move || {
            Rc::new(PagerWrapperFlingBehavior { original_fling_behavior: f, pager_state: s }) as Rc<dyn FlingBehavior>
        })
    };
    // (what the pages' nested scroll connection settles the pager with)
    *state.inner.fling_behavior.borrow_mut() = Some(resolved_fling_behavior.clone());

    let default_bring_into_view_spec = LocalBringIntoViewSpec.current();
    let layout_direction = LocalLayoutDirection.current();
    let pager_bring_into_view_spec = {
        let (s, d) = (state.clone(), default_bring_into_view_spec.clone());
        remember_keyed(
            (state.clone(), Rc::as_ptr(&default_bring_into_view_spec) as *const () as usize, layout_direction),
            move || {
                Rc::new(PagerBringIntoViewSpec { pager_state: s, default_bring_into_view_spec: d, layout_direction })
                    as Rc<dyn BringIntoViewSpec>
            },
        )
    };
    let beyond_bounds_state = {
        let s = state.clone();
        remember_keyed((state.clone(), beyond_viewport_page_count), move || {
            Rc::new(PagerBeyondBoundsState { state: s, beyond_viewport_page_count })
                as Rc<dyn LazyLayoutBeyondBoundsState>
        })
    };

    let mut m = modifier.then(state.inner.remeasurement.modifier()).then(state.inner.await_layout_modifier.modifier());
    m = lazy_layout_semantics(
        m,
        pager_item_provider.clone(),
        semantic_state,
        orientation,
        user_scroll_enabled,
        reverse_layout,
    );
    m = pager_semantics(m, &state, orientation == Orientation::Vertical, coroutine_scope, user_scroll_enabled);
    if user_scroll_enabled {
        m = lazy_layout_beyond_bounds_modifier(
            m,
            beyond_bounds_state,
            state.inner.beyond_bounds_info.clone(),
            reverse_layout,
            orientation,
        );
    }
    m = crate::scrollable_area::scrollable_area_of(
        m,
        Rc::new(state.clone()),
        orientation,
        user_scroll_enabled,
        reverse_layout,
        Some(resolved_fling_behavior),
        Some(state.internal_interaction_source()),
        Some(pager_bring_into_view_spec),
        false,
        overscroll_effect,
    );
    m = drag_direction_detector(m, &state);
    m = m.nested_scroll(page_nested_scroll_connection, None);
    LazyLayout(
        item_provider = pager_item_provider,
        modifier = m,
        prefetch_state = Some(state.inner.prefetch_state.clone()),
        measure_policy = measure_policy,
    );
}

/// Records each gesture's move from down to up on the pager (Compose's `dragDirectionDetector`): what the snapping
/// tells a drag's direction by.
fn drag_direction_detector(modifier: Modifier, state: &PagerState) -> Modifier {
    let state = state.clone();
    modifier.pointer_input(state.clone(), move |scope: PointerInputScope| {
        let state = state.clone();
        async move {
            scope
                .await_each_gesture(|s| {
                    let state = state.clone();
                    async move {
                        let down_event = s.await_first_down_with(false, PointerEventPass::Initial).await;
                        state.inner.up_down_difference.set(Offset::Zero);
                        let up_event_or_cancellation = loop {
                            let event = s.await_pointer_event_with(PointerEventPass::Initial).await;
                            // (all pointers up)
                            if event.changes.iter().all(|c| c.changed_to_up()) {
                                break event.changes[0].clone();
                            }
                        };
                        state.inner.up_down_difference.set(up_event_or_cancellation.position - down_event.position);
                    }
                })
                .await;
        }
    })
}

/// A pager's bring-into-view spec (Compose's PagerBringIntoViewSpec): what the default would scroll, by whole
/// pages; otherwise it settles the pager on a page.
struct PagerBringIntoViewSpec {
    pager_state: PagerState,
    default_bring_into_view_spec: Rc<dyn BringIntoViewSpec>,
    layout_direction: LayoutDirection,
}

impl PartialEq for PagerBringIntoViewSpec {
    fn eq(&self, other: &Self) -> bool {
        self.pager_state == other.pager_state
            && Rc::ptr_eq(&self.default_bring_into_view_spec, &other.default_bring_into_view_spec)
            && self.layout_direction == other.layout_direction
    }
}

impl PagerBringIntoViewSpec {
    /// A horizontal pager right to left scrolls the other way.
    fn should_change_scroll_direction(&self) -> bool {
        self.layout_direction == LayoutDirection::Rtl
            && self.pager_state.layout_info().orientation == Orientation::Horizontal
    }

    fn layout_aware_first_offset(&self) -> i32 {
        let state = &self.pager_state;
        if self.should_change_scroll_direction() {
            -state.first_visible_page_offset() + state.page_size_with_spacing()
        } else {
            state.first_visible_page_offset()
        }
    }

    fn settling_scroll_distance(&self, container_size: f32) -> f32 {
        let state = &self.pager_state;
        let reversed_first_page_scroll = -(self.layout_aware_first_offset() as f32);
        let page = state.page_size_with_spacing() as f32;
        let distance = if self.should_change_scroll_direction() {
            if state.last_scrolled_forward() { reversed_first_page_scroll } else { reversed_first_page_scroll + page }
        } else if state.last_scrolled_forward() {
            reversed_first_page_scroll + page
        } else {
            reversed_first_page_scroll
        };
        distance.clamp(-container_size, container_size)
    }

    fn override_proposed_offset_move(&self, proposed_offset_move: f32) -> f32 {
        let page = self.pager_state.page_size_with_spacing() as f32;
        let mut corrected_offset = -(self.layout_aware_first_offset() as f32);
        // (forward: from the first visible page, as many pages as proposed)
        while proposed_offset_move > 0.0 && corrected_offset < proposed_offset_move {
            corrected_offset += page;
        }
        // (backward: the same the other way)
        while proposed_offset_move < 0.0 && corrected_offset > proposed_offset_move {
            corrected_offset -= page;
        }
        corrected_offset
    }
}

impl BringIntoViewSpec for PagerBringIntoViewSpec {
    fn calculate_scroll_distance(&self, offset: f32, size: f32, container_size: f32) -> f32 {
        let proposed_offset_move =
            self.default_bring_into_view_spec.calculate_scroll_distance(offset, size, container_size);
        // (Int.VisibilityThreshold: 1)
        let is_item_out_view = if offset > 0.0 { offset + size > container_size } else { offset + size <= 1.0 };
        if proposed_offset_move.abs() != 0.0 && is_item_out_view {
            self.override_proposed_offset_move(proposed_offset_move)
        } else if self.pager_state.first_visible_page_offset().abs() < 1 {
            // (settled: nothing)
            0.0
        } else {
            self.settling_scroll_distance(container_size)
        }
    }
}

/// The pager's fling (Compose's PagerWrapperFlingBehavior): the given one, the page it heads to told as the target
/// page as it goes; a fraction under 1e-3 left in the end snapped away.
struct PagerWrapperFlingBehavior {
    original_fling_behavior: Rc<dyn TargetedFlingBehavior>,
    pager_state: PagerState,
}

impl PartialEq for PagerWrapperFlingBehavior {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.original_fling_behavior, &other.original_fling_behavior)
            && self.pager_state == other.pager_state
    }
}

impl FlingBehavior for PagerWrapperFlingBehavior {
    fn perform_fling(&self, scope: Rc<dyn ScrollScope>, initial_velocity: f32) -> LocalFuture<'_, f32> {
        Box::pin(async move {
            let state = self.pager_state.clone();
            let s = scope.clone();
            let result_velocity = self
                .original_fling_behavior
                .perform_fling_with_remaining(
                    scope,
                    initial_velocity,
                    Rc::new(move |remaining_scroll_offset: f32| {
                        let page = state.page_size_with_spacing();
                        let fling_page_displacement =
                            if page != 0 { remaining_scroll_offset / page as f32 } else { 0.0 };
                        let target_page = fling_page_displacement.round() as i32 + state.current_page();
                        state.update_target_page(&*s, target_page);
                    }),
                )
                .await;
            // (done: a rounding error snapped away)
            let state = &self.pager_state;
            let fraction = state.current_page_offset_fraction();
            if fraction != 0.0 && fraction.abs() < 1e-3 {
                state.request_scroll_to_page(state.current_page());
            }
            result_velocity
        })
    }
}
