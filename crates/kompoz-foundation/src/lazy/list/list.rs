//! The lazy list both `LazyColumn` and `LazyRow` are (Compose's LazyList.kt, LazyListItemProvider.kt,
//! LazyListBeyondBoundsModifier.kt, LazyListSemantics.kt, LazyLayoutSemanticState.kt): a `LazyLayout` of the
//! block's items, its measure policy, its modifiers (remeasurement, waiting for the first layout, semantics,
//! beyond-bounds layout, item animations, a scrollable area of its state).

use super::dsl::LazyListScope;
use super::item_scope::{LazyItemScopeImpl, LocalLazyItemScope};
use super::measure::{
    LazyListMeasuredItem, LazyListMeasuredItemProvider, MeasureArgs, keep_around_items, measure_lazy_list,
};
use super::prefetch_strategy::cache_window_logic_of;
use super::state::LazyListState;
use crate::clip_scrollable_container::check_scrollable_container_constraints;
use crate::gestures::{FlingBehavior, LocalFuture, Orientation};
use crate::layout::{HorizontalArrangement, PaddingValues, VerticalArrangement};
use crate::lazy::layout::{
    ItemProviderLambda, LazyKey, LazyLayout, LazyLayoutBeyondBoundsState, LazyLayoutIntervalContent,
    LazyLayoutItemProvider, LazyLayoutKeyIndexMap, LazyLayoutMeasurePolicy, LazyLayoutMeasureScope,
    LazyLayoutPinnableItem, LazyLayoutSemanticState, NearestRangeKeyIndexMap, StickToTopPlacement,
    StickyItemsPlacement, calculate_lazy_layout_pinned_indices, estimated_lazy_max_scroll_offset,
    estimated_lazy_scroll_offset, lazy_layout_beyond_bounds_modifier, lazy_layout_item_animator, lazy_layout_semantics,
    remember_lazy_layout_bring_into_view_spec,
};
use crate::overscroll::OverscrollEffect;
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_graphics::IntOffset;
use kompoz_runtime::{
    DerivedState, KompozitionLocalProvider, derived_state_of, derived_state_of_with_policy, kompozable,
    kompozable_block, never_equal_policy, remember_keyed, remember_updated_state, without_read_observation,
};
use kompoz_ui::layout::SlotKey;
use kompoz_ui::platform::{LocalPlatform, PlatformHandle};
use kompoz_ui::semantics::CollectionInfo;
use kompoz_ui::{Constraints, HorizontalAlignment, MeasureResult, Modifier, VerticalAlignment};
use std::any::Any;
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// The item provider

/// A list's items (Compose's LazyListItemProvider): the layout's item provider with the list's key index map, its
/// sticky headers and its item scope.
pub(crate) trait LazyListItemProvider: LazyLayoutItemProvider {
    fn key_index_map(&self) -> Rc<dyn LazyLayoutKeyIndexMap>;
    fn header_indexes(&self) -> Vec<i32>;
    fn item_scope(&self) -> &LazyItemScopeImpl;
}

/// Compose's LazyListItemProviderImpl: the block's interval content, the keys around the first visible item.
struct LazyListItemProviderImpl {
    state: LazyListState,
    interval_content: Rc<LazyListScope>,
    item_scope: LazyItemScopeImpl,
    key_index_map: Rc<NearestRangeKeyIndexMap>,
}

impl LazyLayoutItemProvider for LazyListItemProviderImpl {
    fn item_count(&self) -> i32 {
        self.interval_content.item_count()
    }

    fn item(&self, index: i32, key: &LazyKey) {
        let content = self.interval_content.clone();
        let item_scope = self.item_scope.clone();
        let pinned_items = self.state.inner.pinned_items.clone();
        let key = key.clone();
        kompozable_block!({
            LazyLayoutPinnableItem(Some(key), index, pinned_items, move || {
                let content = content.clone();
                KompozitionLocalProvider(&[LocalLazyItemScope.provides(Some(item_scope.clone()))], move || {
                    content.with_interval(index, |local_index, interval| (interval.item)(local_index));
                });
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

    /// The same items: the same interval content (Compose's `equals`), so items skip when only the key range
    /// moved.
    fn same_items(&self, other: &dyn LazyLayoutItemProvider) -> bool {
        let other: &dyn Any = other;
        other
            .downcast_ref::<LazyListItemProviderImpl>()
            .is_some_and(|o| Rc::ptr_eq(&o.interval_content, &self.interval_content))
    }
}

impl LazyListItemProvider for LazyListItemProviderImpl {
    fn key_index_map(&self) -> Rc<dyn LazyLayoutKeyIndexMap> {
        self.key_index_map.clone()
    }
    fn header_indexes(&self) -> Vec<i32> {
        self.interval_content.header_indexes().to_vec()
    }
    fn item_scope(&self) -> &LazyItemScopeImpl {
        &self.item_scope
    }
}

/// The list's items as the layout reads them (Compose's `rememberLazyListItemProviderLambda`): derived from the
/// block (run again when what it reads changes) and the range of keys around the first visible item.
#[kompozable]
fn remember_lazy_list_item_provider_lambda(
    state: LazyListState,
    content: Rc<dyn for<'a> Fn(&'a mut LazyListScope)>,
) -> (ItemProviderLambda, Rc<dyn Fn() -> Rc<dyn LazyListItemProvider>>) {
    let latest_content = remember_updated_state!(content);
    remember_keyed(state.clone(), move || {
        let scope = LazyItemScopeImpl::default();
        let interval_content_state: DerivedState<Rc<LazyListScope>> =
            derived_state_of_with_policy(never_equal_policy(), move || {
                let content = latest_content.get();
                Rc::new(LazyListScope::of(&*content))
            });
        let item_provider_state: DerivedState<Rc<LazyListItemProviderImpl>> =
            derived_state_of_with_policy(never_equal_policy(), move || {
                let interval_content = interval_content_state.get();
                let map = NearestRangeKeyIndexMap::new(state.nearest_range(), &*interval_content);
                Rc::new(LazyListItemProviderImpl {
                    state: state.clone(),
                    interval_content,
                    item_scope: scope.clone(),
                    key_index_map: Rc::new(map),
                })
            });
        let s = item_provider_state.clone();
        let layout_lambda: ItemProviderLambda = Rc::new(move || s.get() as Rc<dyn LazyLayoutItemProvider>);
        let list_lambda: Rc<dyn Fn() -> Rc<dyn LazyListItemProvider>> =
            Rc::new(move || item_provider_state.get() as Rc<dyn LazyListItemProvider>);
        (layout_lambda, list_lambda)
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The list

/// Items laid out beyond the bounds by default (Compose's `defaultLazyListBeyondBoundsItemCount`: 0).
const DefaultLazyListBeyondBoundsItemCount: i32 = 0;

/// The list `LazyColumn` and `LazyRow` are (Compose's `LazyList`).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn LazyList(
    modifier: Modifier,
    state: LazyListState,
    content_padding: PaddingValues,
    reverse_layout: bool,
    is_vertical: bool,
    fling_behavior: Rc<dyn FlingBehavior>,
    user_scroll_enabled: bool,
    overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
    horizontal_alignment: Option<HorizontalAlignment>,
    vertical_arrangement: Option<VerticalArrangement>,
    vertical_alignment: Option<VerticalAlignment>,
    horizontal_arrangement: Option<HorizontalArrangement>,
    content: Rc<dyn for<'a> Fn(&'a mut LazyListScope)>,
) {
    let beyond_bounds_item_count = DefaultLazyListBeyondBoundsItemCount;
    let (item_provider_lambda, list_item_provider_lambda) =
        remember_lazy_list_item_provider_lambda(state.clone(), content);
    let semantic_state = {
        let s = state.clone();
        remember_keyed((state.clone(), is_vertical), move || {
            Rc::new(ListSemanticState::new(s, is_vertical)) as Rc<dyn LazyLayoutSemanticState>
        })
    };
    let coroutine_scope = remember_coroutine_scope();
    let platform = LocalPlatform.current();
    // (sticky headers always: KompozR has no scroll capture)
    let sticky_headers_enabled = true;
    let measure_policy = {
        let keys = (
            state.clone(),
            content_padding.clone(),
            reverse_layout,
            is_vertical,
            beyond_bounds_item_count,
            horizontal_alignment.clone(),
            vertical_alignment.clone(),
            horizontal_arrangement.clone(),
            vertical_arrangement.clone(),
            sticky_headers_enabled,
        );
        let (s, p, list) = (state.clone(), content_padding.clone(), list_item_provider_lambda.clone());
        let (ha, va, harr, varr) = (
            horizontal_alignment.clone(),
            vertical_alignment.clone(),
            horizontal_arrangement.clone(),
            vertical_arrangement.clone(),
        );
        let (scope, platform) = (coroutine_scope.clone(), platform.clone());
        remember_keyed(keys, move || {
            lazy_list_measure_policy(ListMeasureConfig {
                item_provider_lambda: list,
                state: s,
                content_padding: p,
                reverse_layout,
                is_vertical,
                beyond_bounds_item_count,
                horizontal_alignment: ha,
                vertical_alignment: va,
                horizontal_arrangement: harr,
                vertical_arrangement: varr,
                coroutine_scope: scope,
                platform: Some(platform),
                sticky_items_placement: sticky_headers_enabled,
            })
        })
    };
    let orientation = if is_vertical { Orientation::Vertical } else { Orientation::Horizontal };

    let beyond_bounds_state = {
        let s = state.clone();
        remember_keyed((state.clone(), beyond_bounds_item_count), move || {
            Rc::new(LazyListBeyondBoundsState { state: s, beyond_bounds_item_count })
                as Rc<dyn LazyLayoutBeyondBoundsState>
        })
    };
    let sticky_size = {
        let s = state.clone();
        remember_keyed(state.clone(), move || {
            Rc::new(move || s.measure_result().sticking_items_combined_size) as Rc<dyn Fn() -> i32>
        })
    };
    let bring_into_view_spec = remember_lazy_layout_bring_into_view_spec(reverse_layout, is_vertical, sticky_size);

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
    m = crate::scrollable_area::scrollable_area_of(
        m,
        Rc::new(state.clone()),
        orientation,
        user_scroll_enabled,
        reverse_layout,
        Some(fling_behavior),
        Some(state.internal_interaction_source()),
        Some(bring_into_view_spec),
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

/// What the list's measure policy is made of.
struct ListMeasureConfig {
    item_provider_lambda: Rc<dyn Fn() -> Rc<dyn LazyListItemProvider>>,
    state: LazyListState,
    content_padding: PaddingValues,
    reverse_layout: bool,
    is_vertical: bool,
    beyond_bounds_item_count: i32,
    horizontal_alignment: Option<HorizontalAlignment>,
    vertical_alignment: Option<VerticalAlignment>,
    horizontal_arrangement: Option<HorizontalArrangement>,
    vertical_arrangement: Option<VerticalArrangement>,
    coroutine_scope: CoroutineScope,
    platform: Option<PlatformHandle>,
    sticky_items_placement: bool,
}

/// The list's measure policy (Compose's `rememberLazyListMeasurePolicy`'s): the paddings resolved, the items'
/// viewport told to their scope, the first visible item found again by key, measured, the result applied to the
/// state (and the cache window's items kept around).
fn lazy_list_measure_policy(c: ListMeasureConfig) -> Rc<dyn LazyLayoutMeasurePolicy> {
    Rc::new(move |scope: &LazyLayoutMeasureScope, container_constraints: Constraints| -> MeasureResult {
        let state = &c.state;
        state.inner.measurement_scope_invalidator.attach_to_scope();
        let is_looking_ahead = scope.is_looking_ahead();
        // (whether the lookahead pass ran: in a lookahead scope)
        let has_lookahead_occurred = state.inner.has_lookahead_occurred.get() || is_looking_ahead;
        let is_vertical = c.is_vertical;
        check_scrollable_container_constraints(
            container_constraints,
            if is_vertical { Orientation::Vertical } else { Orientation::Horizontal },
        );
        let layout_direction = scope.layout_direction();
        let density = scope.density();
        // The content paddings (horizontal ones mirrored by placing relative).
        let start_padding = if is_vertical {
            density.round_to_px(c.content_padding.calculate_left_padding(layout_direction))
        } else {
            density.round_to_px(c.content_padding.calculate_start_padding(layout_direction))
        };
        let end_padding = if is_vertical {
            density.round_to_px(c.content_padding.calculate_right_padding(layout_direction))
        } else {
            density.round_to_px(c.content_padding.calculate_end_padding(layout_direction))
        };
        let top_padding = density.round_to_px(c.content_padding.calculate_top_padding());
        let bottom_padding = density.round_to_px(c.content_padding.calculate_bottom_padding());
        let total_vertical_padding = top_padding + bottom_padding;
        let total_horizontal_padding = start_padding + end_padding;
        let total_main_axis_padding = if is_vertical { total_vertical_padding } else { total_horizontal_padding };
        let before_content_padding = match (is_vertical, c.reverse_layout) {
            (true, false) => top_padding,
            (true, true) => bottom_padding,
            (false, false) => start_padding,
            (false, true) => end_padding,
        };
        let after_content_padding = total_main_axis_padding - before_content_padding;
        let content_constraints = container_constraints.offset(-total_horizontal_padding, -total_vertical_padding);

        let item_provider = (c.item_provider_lambda)();
        // (the scope the items' content reads)
        item_provider.item_scope().set_max_size(content_constraints.max_width, content_constraints.max_height);

        let space_between_items_dp = if is_vertical {
            c.vertical_arrangement.as_ref().expect("null verticalArrangement when isVertical == true").spacing()
        } else {
            c.horizontal_arrangement.as_ref().expect("null horizontalAlignment when isVertical == false").spacing()
        };
        let space_between_items = density.round_to_px(space_between_items_dp);
        let items_count = item_provider.item_count();

        // (negative when the content padding is more than the constraints)
        let main_axis_available_size = if is_vertical {
            container_constraints.max_height - total_vertical_padding
        } else {
            container_constraints.max_width - total_horizontal_padding
        };
        let visual_item_offset = if !c.reverse_layout || main_axis_available_size > 0 {
            IntOffset::new(start_padding, top_padding)
        } else {
            // (reversed, the paddings more than the space: the layout is 0 long, the start padding moved by the
            // space they take beyond it)
            IntOffset::new(
                if is_vertical { start_padding } else { start_padding + main_axis_available_size },
                if is_vertical { top_padding + main_axis_available_size } else { top_padding },
            )
        };

        let (ha, va, reverse_layout) = (c.horizontal_alignment.clone(), c.vertical_alignment.clone(), c.reverse_layout);
        let animator = state.inner.item_animator.clone();
        let measured_item_provider = LazyListMeasuredItemProvider::new(
            content_constraints,
            is_vertical,
            item_provider.clone(),
            scope,
            move |index, key, content_type, placeables, constraints| {
                // (the spacing after all items but the last: the measure takes it into account)
                let spacing = if index == items_count - 1 { 0 } else { space_between_items };
                LazyListMeasuredItem::new(
                    index,
                    placeables,
                    is_vertical,
                    ha.clone(),
                    va.clone(),
                    layout_direction,
                    reverse_layout,
                    before_content_padding,
                    after_content_padding,
                    spacing,
                    visual_item_offset,
                    key,
                    content_type,
                    animator.clone(),
                    constraints,
                )
            },
        );

        let (first_visible_item_index, first_visible_scroll_offset) = without_read_observation(|| {
            let index = state
                .update_scroll_position_if_the_first_item_was_moved(&*item_provider, state.first_visible_item_index());
            (index, state.first_visible_item_scroll_offset())
        });

        let pinned_items = calculate_lazy_layout_pinned_indices(
            &*item_provider,
            &state.inner.pinned_items,
            &state.inner.beyond_bounds_info,
        );

        // (the lookahead pass scrolls; the approach after it by what it scrolled back of that, animated away)
        let scroll_to_be_consumed = if is_looking_ahead || !has_lookahead_occurred {
            state.inner.scroll_to_be_consumed.get()
        } else {
            state.scroll_delta_between_passes()
        };

        let sticky = StickToTopPlacement;
        let sticky_items_placement: Option<&dyn StickyItemsPlacement> =
            if c.sticky_items_placement { Some(&sticky) } else { None };
        let layout = |width: i32, height: i32, placement: Box<dyn Fn(&kompoz_ui::PlacementScope)>| {
            scope.layout(
                container_constraints.constrain_width(width + total_horizontal_padding),
                container_constraints.constrain_height(height + total_vertical_padding),
                placement,
            )
        };
        let measure_result = measure_lazy_list(
            &measured_item_provider,
            MeasureArgs {
                items_count,
                main_axis_available_size,
                before_content_padding,
                after_content_padding,
                space_between_items,
                first_visible_item_index,
                first_visible_item_scroll_offset: first_visible_scroll_offset,
                scroll_to_be_consumed,
                constraints: content_constraints,
                is_vertical,
                vertical_arrangement: c.vertical_arrangement.clone(),
                horizontal_arrangement: c.horizontal_arrangement.clone(),
                reverse_layout,
                density,
                item_animator: state.inner.item_animator.clone(),
                beyond_bounds_item_count: c.beyond_bounds_item_count,
                pinned_items,
                has_lookahead_occurred,
                is_looking_ahead,
                coroutine_scope: c.coroutine_scope.clone(),
                placement_scope_invalidator: state.inner.placement_scope_invalidator.clone(),
                platform: c.platform.clone(),
                sticky_items_placement,
                should_run_item_animation: !state.inner.skip_item_placement_animation.get(),
                layout: &layout,
            },
        );
        let result = Rc::new(measure_result);
        state.apply_measure_result(result.clone(), is_looking_ahead, false);
        // (the cache window's items kept around after the strategy saw the result)
        if let Some(logic) = cache_window_logic_of(&*state.inner.prefetch_strategy) {
            keep_around_items(&logic, &result.visible_items_info, &measured_item_provider);
        }
        result.measure_result.clone()
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Beyond bounds and semantics

/// What the list tells its beyond-bounds layout (Compose's LazyListBeyondBoundsState).
struct LazyListBeyondBoundsState {
    state: LazyListState,
    beyond_bounds_item_count: i32,
}

impl LazyLayoutBeyondBoundsState for LazyListBeyondBoundsState {
    fn item_count(&self) -> i32 {
        self.state.layout_info().total_items_count
    }
    fn has_visible_items(&self) -> bool {
        !self.state.layout_info().items().is_empty()
    }
    fn first_placed_index(&self) -> i32 {
        (self.state.first_visible_item_index() - self.beyond_bounds_item_count).max(0)
    }
    fn last_placed_index(&self) -> i32 {
        let info = self.state.layout_info();
        (info.items().last().unwrap().index + self.beyond_bounds_item_count).min(info.total_items_count - 1)
    }
    fn items_per_viewport(&self) -> i32 {
        let info = self.state.layout_info();
        if info.items().is_empty() {
            return 0;
        }
        let viewport_size = info.single_axis_viewport_size();
        let average_item_size = info.visible_items_average_size();
        if average_item_size == 0 {
            return 1;
        }
        (viewport_size / average_item_size).max(1)
    }
}

/// What the list's semantics read of its state (Compose's `LazyLayoutSemanticState(state, isVertical)`).
struct ListSemanticState {
    state: LazyListState,
    is_vertical: bool,
    total_items_count: DerivedState<i32>,
}

impl ListSemanticState {
    fn new(state: LazyListState, is_vertical: bool) -> ListSemanticState {
        let s = state.clone();
        ListSemanticState {
            state,
            is_vertical,
            total_items_count: derived_state_of(move || s.layout_info().total_items_count),
        }
    }
}

impl LazyLayoutSemanticState for ListSemanticState {
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
        let count = self.total_items_count.get();
        if self.is_vertical {
            CollectionInfo { row_count: count, column_count: 1 }
        } else {
            CollectionInfo { row_count: 1, column_count: count }
        }
    }
    fn scroll_to_item(&self, index: i32) -> LocalFuture<'static, ()> {
        let state = self.state.clone();
        Box::pin(async move { state.scroll_to_item(index).await })
    }
}
