//! The lazy staggered grid both `LazyVerticalStaggeredGrid` and `LazyHorizontalStaggeredGrid` are (Compose's
//! LazyStaggeredGrid.kt, LazyStaggeredGridItemProvider.kt, LazyStaggeredGridMeasurePolicy.kt,
//! LazyStaggeredGridSemantics.kt, LazyStaggeredGridBeyondBoundsModifier.kt): a `LazyLayout` of the block's items,
//! its measure policy (the lanes worked out for the constraints, the paddings resolved), its modifiers
//! (remeasurement, waiting for the first layout, semantics, beyond-bounds layout, item animations, a scrollable area
//! of its state).

use super::cells::LazyStaggeredGridSlotsProvider;
use super::dsl::LazyStaggeredGridScope;
use super::measure::measure_staggered_grid;
use super::state::LazyStaggeredGridState;
use crate::clip_scrollable_container::check_scrollable_container_constraints;
use crate::gestures::{FlingBehavior, LocalFuture, Orientation};
use crate::layout::PaddingValues;
use crate::lazy::layout::{
    ItemProviderLambda, LazyKey, LazyLayout, LazyLayoutBeyondBoundsState, LazyLayoutIntervalContent,
    LazyLayoutItemProvider, LazyLayoutKeyIndexMap, LazyLayoutMeasurePolicy, LazyLayoutMeasureScope,
    LazyLayoutPinnableItem, LazyLayoutSemanticState, NearestRangeKeyIndexMap, calculate_lazy_layout_pinned_indices,
    estimated_lazy_max_scroll_offset, estimated_lazy_scroll_offset, lazy_layout_beyond_bounds_modifier,
    lazy_layout_item_animator, lazy_layout_semantics,
};
use crate::overscroll::OverscrollEffect;
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_graphics::{Dp, IntOffset, LayoutDirection};
use kompoz_runtime::{
    DerivedState, MutableState, derived_state_of_with_policy, kompozable, kompozable_block, mutable_state_of,
    never_equal_policy, remember, remember_keyed,
};
use kompoz_ui::layout::SlotKey;
use kompoz_ui::platform::{LocalPlatform, PlatformHandle};
use kompoz_ui::semantics::CollectionInfo;
use kompoz_ui::{Constraints, MeasureResult, Modifier};
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// The item provider

/// A staggered grid's items (Compose's LazyStaggeredGridItemProvider): the layout's item provider with the grid's
/// key index map and its content (which tells full-line items: Compose's `spanProvider`).
pub(crate) trait LazyStaggeredGridItemProvider: LazyLayoutItemProvider {
    fn key_index_map(&self) -> Rc<dyn LazyLayoutKeyIndexMap>;
    fn span_provider(&self) -> &Rc<LazyStaggeredGridScope>;
}

/// Compose's LazyStaggeredGridItemProviderImpl.
struct LazyStaggeredGridItemProviderImpl {
    state: LazyStaggeredGridState,
    interval_content: Rc<LazyStaggeredGridScope>,
    key_index_map: Rc<NearestRangeKeyIndexMap>,
}

impl LazyLayoutItemProvider for LazyStaggeredGridItemProviderImpl {
    fn item_count(&self) -> i32 {
        self.interval_content.item_count()
    }

    fn item(&self, index: i32, key: &LazyKey) {
        let content = self.interval_content.clone();
        let pinned_items = self.state.inner.pinned_items.clone();
        let key = key.clone();
        kompozable_block!({
            LazyLayoutPinnableItem(Some(key), index, pinned_items, move || {
                content.with_interval(index, |local_index, interval| (interval.item)(local_index));
            });
        });
    }

    fn get_key(&self, index: i32) -> LazyKey {
        self.key_index_map.get_key(index).unwrap_or_else(|| self.interval_content.get_key(index))
    }

    fn get_content_type(&self, index: i32) -> Option<SlotKey> {
        self.interval_content.get_content_type(index)
    }

    fn get_index(&self, key: &LazyKey) -> i32 {
        self.key_index_map.get_index(key)
    }

    /// The same items: the same interval content (Compose's `equals`).
    fn same_items(&self, other: &dyn LazyLayoutItemProvider) -> bool {
        let other: &dyn Any = other;
        other
            .downcast_ref::<LazyStaggeredGridItemProviderImpl>()
            .is_some_and(|o| Rc::ptr_eq(&o.interval_content, &self.interval_content))
    }
}

impl LazyStaggeredGridItemProvider for LazyStaggeredGridItemProviderImpl {
    fn key_index_map(&self) -> Rc<dyn LazyLayoutKeyIndexMap> {
        self.key_index_map.clone()
    }
    fn span_provider(&self) -> &Rc<LazyStaggeredGridScope> {
        &self.interval_content
    }
}

/// The block a grid was last composed with, read as a state (as the list's `LatestContent`).
struct LatestContent {
    content: RefCell<Option<Rc<dyn for<'a> Fn(&'a mut LazyStaggeredGridScope)>>>,
    generation: MutableState<u64>,
}

impl LatestContent {
    fn set(&self, content: Rc<dyn for<'a> Fn(&'a mut LazyStaggeredGridScope)>) {
        *self.content.borrow_mut() = Some(content);
        self.generation.set(self.generation.peek().wrapping_add(1));
    }

    fn get(&self) -> Rc<dyn for<'a> Fn(&'a mut LazyStaggeredGridScope)> {
        self.generation.get();
        self.content.borrow().clone().expect("the grid's block")
    }
}

/// The grid's items as the layout reads them (Compose's `rememberStaggeredGridItemProviderLambda`).
#[kompozable]
fn remember_staggered_grid_item_provider_lambda(
    state: LazyStaggeredGridState,
    content: Rc<dyn for<'a> Fn(&'a mut LazyStaggeredGridScope)>,
) -> (ItemProviderLambda, Rc<dyn Fn() -> Rc<dyn LazyStaggeredGridItemProvider>>) {
    let latest_content =
        remember(|| Rc::new(LatestContent { content: RefCell::new(None), generation: mutable_state_of(0) }));
    latest_content.set(content);
    remember_keyed(state.clone(), move || {
        let interval_content_state: DerivedState<Rc<LazyStaggeredGridScope>> =
            derived_state_of_with_policy(never_equal_policy(), move || {
                let content = latest_content.get();
                Rc::new(LazyStaggeredGridScope::of(&*content))
            });
        let item_provider_state: DerivedState<Rc<LazyStaggeredGridItemProviderImpl>> =
            derived_state_of_with_policy(never_equal_policy(), move || {
                let interval_content = interval_content_state.get();
                let map = NearestRangeKeyIndexMap::new(state.nearest_range(), &*interval_content);
                Rc::new(LazyStaggeredGridItemProviderImpl {
                    state: state.clone(),
                    interval_content,
                    key_index_map: Rc::new(map),
                })
            });
        let s = item_provider_state.clone();
        let layout_lambda: ItemProviderLambda = Rc::new(move || s.get() as Rc<dyn LazyLayoutItemProvider>);
        let grid_lambda: Rc<dyn Fn() -> Rc<dyn LazyStaggeredGridItemProvider>> =
            Rc::new(move || item_provider_state.get() as Rc<dyn LazyStaggeredGridItemProvider>);
        (layout_lambda, grid_lambda)
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The grid

/// The grid `LazyVerticalStaggeredGrid` and `LazyHorizontalStaggeredGrid` are (Compose's `LazyStaggeredGrid`).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn LazyStaggeredGrid(
    state: LazyStaggeredGridState,
    orientation: Orientation,
    slots: Rc<LazyStaggeredGridSlotsProvider>,
    modifier: Modifier,
    content_padding: PaddingValues,
    reverse_layout: bool,
    fling_behavior: Rc<dyn FlingBehavior>,
    user_scroll_enabled: bool,
    overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
    main_axis_spacing: Dp,
    cross_axis_spacing: Dp,
    content: Rc<dyn for<'a> Fn(&'a mut LazyStaggeredGridScope)>,
) {
    let (item_provider_lambda, grid_item_provider_lambda) =
        remember_staggered_grid_item_provider_lambda(state.clone(), content);
    let coroutine_scope = remember_coroutine_scope();
    let platform = LocalPlatform.current();
    let measure_policy = {
        let keys = (
            state.clone(),
            Rc::as_ptr(&grid_item_provider_lambda) as *const () as usize,
            content_padding.clone(),
            reverse_layout,
            orientation,
            main_axis_spacing,
            cross_axis_spacing,
            Rc::as_ptr(&slots) as usize,
        );
        let (s, p, grid) = (state.clone(), content_padding.clone(), grid_item_provider_lambda.clone());
        let (scope, platform, slots) = (coroutine_scope.clone(), platform.clone(), slots.clone());
        remember_keyed(keys, move || {
            staggered_grid_measure_policy(StaggeredGridMeasureConfig {
                state: s,
                item_provider_lambda: grid,
                content_padding: p,
                reverse_layout,
                orientation,
                main_axis_spacing,
                coroutine_scope: scope,
                slots,
                platform: Some(platform),
            })
        })
    };
    let semantic_state = {
        let s = state.clone();
        remember_keyed((state.clone(), reverse_layout), move || {
            Rc::new(StaggeredGridSemanticState { state: s }) as Rc<dyn LazyLayoutSemanticState>
        })
    };
    let beyond_bounds_state = {
        let s = state.clone();
        remember_keyed(state.clone(), move || {
            Rc::new(LazyStaggeredGridBeyondBoundsState { state: s }) as Rc<dyn LazyLayoutBeyondBoundsState>
        })
    };

    let mut m = modifier.then(state.inner.remeasurement.modifier()).then(state.inner.await_layout_modifier.modifier());
    m = lazy_layout_semantics(
        m,
        item_provider_lambda.clone(),
        semantic_state,
        orientation,
        user_scroll_enabled,
        reverse_layout,
    );
    if user_scroll_enabled {
        m = lazy_layout_beyond_bounds_modifier(
            m,
            beyond_bounds_state,
            state.inner.beyond_bounds_info.clone(),
            reverse_layout,
            orientation,
        );
    }
    m = lazy_layout_item_animator(m, state.inner.item_animator.clone());
    // (no bring-into-view spec of its own: the local's)
    m = crate::scrollable_area::scrollable_area_of(
        m,
        Rc::new(state.clone()),
        orientation,
        user_scroll_enabled,
        reverse_layout,
        Some(fling_behavior),
        Some(state.inner.mutable_interaction_source.clone()),
        None,
        false,
        overscroll_effect,
    );
    LazyLayout(
        item_provider = item_provider_lambda,
        modifier = m,
        prefetch_state = Some(state.inner.prefetch_state.clone()),
        measure_policy = measure_policy,
    );
}

/// What the staggered grid's measure policy is made of.
struct StaggeredGridMeasureConfig {
    state: LazyStaggeredGridState,
    item_provider_lambda: Rc<dyn Fn() -> Rc<dyn LazyStaggeredGridItemProvider>>,
    content_padding: PaddingValues,
    reverse_layout: bool,
    orientation: Orientation,
    main_axis_spacing: Dp,
    coroutine_scope: CoroutineScope,
    slots: Rc<LazyStaggeredGridSlotsProvider>,
    platform: Option<PlatformHandle>,
}

/// The paddings before and after the content along the grid, and at its start across (Compose's `beforePadding`,
/// `afterPadding`, `startPadding`).
fn paddings(
    p: &PaddingValues,
    orientation: Orientation,
    reverse_layout: bool,
    layout_direction: LayoutDirection,
) -> (Dp, Dp, Dp) {
    match orientation {
        Orientation::Vertical => {
            let (before, after) = if reverse_layout {
                (p.calculate_bottom_padding(), p.calculate_top_padding())
            } else {
                (p.calculate_top_padding(), p.calculate_bottom_padding())
            };
            (before, after, p.calculate_start_padding(layout_direction))
        }
        Orientation::Horizontal => {
            let (before, after) = if reverse_layout {
                (p.calculate_end_padding(layout_direction), p.calculate_start_padding(layout_direction))
            } else {
                (p.calculate_start_padding(layout_direction), p.calculate_end_padding(layout_direction))
            };
            (before, after, p.calculate_top_padding())
        }
    }
}

/// The staggered grid's measure policy (Compose's `rememberStaggeredGridMeasurePolicy`'s).
fn staggered_grid_measure_policy(c: StaggeredGridMeasureConfig) -> Rc<dyn LazyLayoutMeasurePolicy> {
    Rc::new(move |scope: &LazyLayoutMeasureScope, constraints: Constraints| -> MeasureResult {
        let state = &c.state;
        state.inner.measurement_scope_invalidator.attach_to_scope();
        // (lookahead is stage 11's: never here)
        let is_looking_ahead = false;
        let is_in_lookahead_scope = state.inner.has_lookahead_occurred.get() || is_looking_ahead;
        check_scrollable_container_constraints(constraints, c.orientation);
        let density = scope.density();
        let resolved_slots = c.slots.invoke(&density, constraints);
        let is_vertical = c.orientation == Orientation::Vertical;
        let item_provider = (c.item_provider_lambda)();
        let layout_direction = scope.layout_direction();

        let (before, after, start) = paddings(&c.content_padding, c.orientation, c.reverse_layout, layout_direction);
        let before_content_padding = density.round_to_px(before);
        let after_content_padding = density.round_to_px(after);
        let start_content_padding = density.round_to_px(start);

        let max_main_axis_size = if is_vertical { constraints.max_height } else { constraints.max_width };
        let main_axis_available_size = max_main_axis_size - before_content_padding - after_content_padding;
        let content_offset = if is_vertical {
            IntOffset::new(start_content_padding, before_content_padding)
        } else {
            IntOffset::new(before_content_padding, start_content_padding)
        };
        let horizontal_padding = density.round_to_px(
            c.content_padding.calculate_start_padding(layout_direction)
                + c.content_padding.calculate_end_padding(layout_direction),
        );
        let vertical_padding = density
            .round_to_px(c.content_padding.calculate_top_padding() + c.content_padding.calculate_bottom_padding());

        let pinned_items = calculate_lazy_layout_pinned_indices(
            &*item_provider,
            &state.inner.pinned_items,
            &state.inner.beyond_bounds_info,
        );
        let measure_constraints = Constraints::new(
            constraints.constrain_width(horizontal_padding),
            constraints.max_width,
            constraints.constrain_height(vertical_padding),
            constraints.max_height,
        );
        let result = measure_staggered_grid(
            scope,
            state,
            pinned_items,
            item_provider,
            resolved_slots,
            measure_constraints,
            is_vertical,
            c.reverse_layout,
            content_offset,
            main_axis_available_size,
            density.round_to_px(c.main_axis_spacing),
            before_content_padding,
            after_content_padding,
            c.coroutine_scope.clone(),
            c.platform.clone(),
            is_in_lookahead_scope,
            is_looking_ahead,
        );
        let result = Rc::new(result);
        state.apply_measure_result(result.clone(), is_looking_ahead, false);
        result.measure_result.clone()
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Beyond bounds and semantics

/// Compose's LazyStaggeredGridBeyondBoundsState.
struct LazyStaggeredGridBeyondBoundsState {
    state: LazyStaggeredGridState,
}

impl LazyLayoutBeyondBoundsState for LazyStaggeredGridBeyondBoundsState {
    fn item_count(&self) -> i32 {
        self.state.layout_info().total_items_count
    }
    fn has_visible_items(&self) -> bool {
        !self.state.layout_info().items().is_empty()
    }
    fn first_placed_index(&self) -> i32 {
        self.state.first_visible_item_index()
    }
    fn last_placed_index(&self) -> i32 {
        self.state.layout_info().items().last().unwrap().index
    }
    fn items_per_viewport(&self) -> i32 {
        let info = self.state.layout_info();
        if info.items().is_empty() {
            return 0;
        }
        let viewport_size = info.single_axis_viewport_size();
        let line_average_size = info.visible_items_average_size();
        if line_average_size == 0 {
            return 1;
        }
        (viewport_size / line_average_size).max(1)
    }
}

/// Compose's `rememberLazyStaggeredGridSemanticState`: rows and columns not told (-1).
struct StaggeredGridSemanticState {
    state: LazyStaggeredGridState,
}

impl LazyLayoutSemanticState for StaggeredGridSemanticState {
    fn viewport(&self) -> i32 {
        let info = self.state.layout_info();
        if info.orientation == Orientation::Vertical { info.viewport_size.height } else { info.viewport_size.width }
    }
    fn content_padding(&self) -> i32 {
        let info = self.state.layout_info();
        info.before_content_padding + info.after_content_padding
    }
    fn scroll_offset(&self) -> f32 {
        estimated_lazy_scroll_offset(
            self.state.first_visible_item_index(),
            self.state.first_visible_item_scroll_offset(),
        )
    }
    fn max_scroll_offset(&self) -> f32 {
        use crate::gestures::ScrollableState;
        estimated_lazy_max_scroll_offset(
            self.state.first_visible_item_index(),
            self.state.first_visible_item_scroll_offset(),
            self.state.can_scroll_forward(),
        )
    }
    fn collection_info(&self) -> CollectionInfo {
        CollectionInfo { row_count: -1, column_count: -1 }
    }
    fn scroll_to_item(&self, index: i32) -> LocalFuture<'static, ()> {
        let state = self.state.clone();
        Box::pin(async move { state.scroll_to_item(index).await })
    }
}
