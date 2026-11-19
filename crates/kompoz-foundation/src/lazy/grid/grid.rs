//! The lazy grid both `LazyVerticalGrid` and `LazyHorizontalGrid` are (Compose's LazyGrid.kt,
//! LazyGridItemProvider.kt, LazyGridBeyondBoundsModifier.kt, LazySemantics.kt): a `LazyLayout` of the block's
//! items, its measure policy (the cells worked out for the constraints, the first visible item's line found again
//! by key), its modifiers (remeasurement, waiting for the first layout, semantics, beyond-bounds layout, item
//! animations, a scrollable area of its state).

use super::cells::LazyGridSlotsProvider;
use super::dsl::LazyGridScope;
use super::measure::{
    LazyGridMeasuredItem, LazyGridMeasuredItemProvider, LazyGridMeasuredLineProvider, MeasureArgs,
    PrefetchInfoRetriever, child_constraints, keep_around_items, measure_lazy_grid,
};
use super::prefetch_strategy::cache_window_logic_of;
use super::state::LazyGridState;
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
    DerivedState, MutableState, derived_state_of_with_policy, kompozable, kompozable_block, mutable_state_of,
    never_equal_policy, remember, remember_keyed, without_read_observation,
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

/// A grid's items (Compose's LazyGridItemProvider): the layout's item provider with the grid's key index map, its
/// sticky headers and its content (which works out the lines from the spans).
pub(crate) trait LazyGridItemProvider: LazyLayoutItemProvider {
    fn key_index_map(&self) -> Rc<dyn LazyLayoutKeyIndexMap>;
    fn header_indexes(&self) -> Vec<i32>;
    /// The interval content, and with it the span layout (Compose's `spanLayoutProvider`).
    fn content(&self) -> &Rc<LazyGridScope>;
}

/// Compose's LazyGridItemProviderImpl: the block's interval content, the keys around the first visible item.
struct LazyGridItemProviderImpl {
    state: LazyGridState,
    interval_content: Rc<LazyGridScope>,
    key_index_map: Rc<NearestRangeKeyIndexMap>,
}

impl LazyLayoutItemProvider for LazyGridItemProviderImpl {
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
            .downcast_ref::<LazyGridItemProviderImpl>()
            .is_some_and(|o| Rc::ptr_eq(&o.interval_content, &self.interval_content))
    }
}

impl LazyGridItemProvider for LazyGridItemProviderImpl {
    fn key_index_map(&self) -> Rc<dyn LazyLayoutKeyIndexMap> {
        self.key_index_map.clone()
    }
    fn header_indexes(&self) -> Vec<i32> {
        self.interval_content.header_indexes.clone()
    }
    fn content(&self) -> &Rc<LazyGridScope> {
        &self.interval_content
    }
}

/// The block a grid was last composed with, read as a state (as the list's `LatestContent`).
struct LatestContent {
    content: RefCell<Option<Rc<dyn for<'a> Fn(&'a mut LazyGridScope)>>>,
    generation: MutableState<u64>,
}

impl LatestContent {
    fn set(&self, content: Rc<dyn for<'a> Fn(&'a mut LazyGridScope)>) {
        *self.content.borrow_mut() = Some(content);
        self.generation.set(self.generation.peek().wrapping_add(1));
    }

    fn get(&self) -> Rc<dyn for<'a> Fn(&'a mut LazyGridScope)> {
        self.generation.get();
        self.content.borrow().clone().expect("the grid's block")
    }
}

/// The grid's items as the layout reads them (Compose's `rememberLazyGridItemProviderLambda`): derived from the
/// block (run again when what it reads changes) and the range of keys around the first visible item.
#[kompozable]
fn remember_lazy_grid_item_provider_lambda(
    state: LazyGridState,
    content: Rc<dyn for<'a> Fn(&'a mut LazyGridScope)>,
) -> (ItemProviderLambda, Rc<dyn Fn() -> Rc<dyn LazyGridItemProvider>>) {
    let latest_content =
        remember(|| Rc::new(LatestContent { content: RefCell::new(None), generation: mutable_state_of(0) }));
    latest_content.set(content);
    remember_keyed(state.clone(), move || {
        let interval_content_state: DerivedState<Rc<LazyGridScope>> =
            derived_state_of_with_policy(never_equal_policy(), move || {
                let content = latest_content.get();
                Rc::new(LazyGridScope::of(&*content))
            });
        let item_provider_state: DerivedState<Rc<LazyGridItemProviderImpl>> =
            derived_state_of_with_policy(never_equal_policy(), move || {
                let interval_content = interval_content_state.get();
                let map = NearestRangeKeyIndexMap::new(state.nearest_range(), &*interval_content);
                Rc::new(LazyGridItemProviderImpl {
                    state: state.clone(),
                    interval_content,
                    key_index_map: Rc::new(map),
                })
            });
        let s = item_provider_state.clone();
        let layout_lambda: ItemProviderLambda = Rc::new(move || s.get() as Rc<dyn LazyLayoutItemProvider>);
        let grid_lambda: Rc<dyn Fn() -> Rc<dyn LazyGridItemProvider>> =
            Rc::new(move || item_provider_state.get() as Rc<dyn LazyGridItemProvider>);
        (layout_lambda, grid_lambda)
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The grid

/// The grid `LazyVerticalGrid` and `LazyHorizontalGrid` are (Compose's `LazyGrid`).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn LazyGrid(
    modifier: Modifier,
    state: LazyGridState,
    slots: Rc<LazyGridSlotsProvider>,
    content_padding: PaddingValues,
    reverse_layout: bool,
    is_vertical: bool,
    fling_behavior: Rc<dyn FlingBehavior>,
    user_scroll_enabled: bool,
    overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
    vertical_arrangement: VerticalArrangement,
    horizontal_arrangement: HorizontalArrangement,
    content: Rc<dyn for<'a> Fn(&'a mut LazyGridScope)>,
) {
    let (item_provider_lambda, grid_item_provider_lambda) =
        remember_lazy_grid_item_provider_lambda(state.clone(), content);
    let semantic_state = {
        let s = state.clone();
        remember_keyed((state.clone(), reverse_layout), move || {
            Rc::new(GridSemanticState::new(s)) as Rc<dyn LazyLayoutSemanticState>
        })
    };
    let coroutine_scope = remember_coroutine_scope();
    let platform = LocalPlatform.current();
    // (sticky headers always: KompozR has no scroll capture)
    let sticky_headers_enabled = true;
    let measure_policy = {
        let keys = (
            state.clone(),
            Rc::as_ptr(&slots) as usize,
            content_padding.clone(),
            reverse_layout,
            is_vertical,
            horizontal_arrangement.clone(),
            vertical_arrangement.clone(),
        );
        let (s, p, grid) = (state.clone(), content_padding.clone(), grid_item_provider_lambda.clone());
        let (harr, varr) = (horizontal_arrangement.clone(), vertical_arrangement.clone());
        let (scope, platform, slots) = (coroutine_scope.clone(), platform.clone(), slots.clone());
        remember_keyed(keys, move || {
            lazy_grid_measure_policy(GridMeasureConfig {
                item_provider_lambda: grid,
                state: s,
                slots,
                content_padding: p,
                reverse_layout,
                is_vertical,
                horizontal_arrangement: Some(harr),
                vertical_arrangement: Some(varr),
                coroutine_scope: scope,
                platform: Some(platform),
                sticky_items_placement: sticky_headers_enabled,
            })
        })
    };
    let orientation = if is_vertical { Orientation::Vertical } else { Orientation::Horizontal };

    let sticky_size = {
        let s = state.clone();
        remember_keyed(state.clone(), move || {
            Rc::new(move || s.measure_result().sticking_items_combined_size) as Rc<dyn Fn() -> i32>
        })
    };
    let bring_into_view_spec = remember_lazy_layout_bring_into_view_spec(reverse_layout, is_vertical, sticky_size);
    let beyond_bounds_state = {
        let s = state.clone();
        remember_keyed(state.clone(), move || {
            Rc::new(LazyGridBeyondBoundsState { state: s }) as Rc<dyn LazyLayoutBeyondBoundsState>
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

/// What the grid's measure policy is made of.
struct GridMeasureConfig {
    item_provider_lambda: Rc<dyn Fn() -> Rc<dyn LazyGridItemProvider>>,
    state: LazyGridState,
    slots: Rc<LazyGridSlotsProvider>,
    content_padding: PaddingValues,
    reverse_layout: bool,
    is_vertical: bool,
    horizontal_arrangement: Option<HorizontalArrangement>,
    vertical_arrangement: Option<VerticalArrangement>,
    coroutine_scope: CoroutineScope,
    platform: Option<PlatformHandle>,
    sticky_items_placement: bool,
}

/// The grid's measure policy (Compose's `rememberLazyGridMeasurePolicy`'s): the paddings resolved, the cells
/// worked out, the first visible item's line found again by key, measured, the result applied to the state (and
/// the cache window's lines kept around).
fn lazy_grid_measure_policy(c: GridMeasureConfig) -> Rc<dyn LazyLayoutMeasurePolicy> {
    Rc::new(move |scope: &LazyLayoutMeasureScope, container_constraints: Constraints| -> MeasureResult {
        let state = &c.state;
        state.inner.measurement_scope_invalidator.attach_to_scope();
        let is_looking_ahead = scope.is_looking_ahead();
        // (whether the lookahead pass ran: in a lookahead scope)
        let is_in_lookahead_scope = state.inner.has_lookahead_occurred.get() || is_looking_ahead;
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
        let content = item_provider.content().clone();
        let resolved_slots = c.slots.invoke(&density, content_constraints);
        let slots_per_line = resolved_slots.sizes.len() as i32;
        content.span_layout_provider.set_slots_per_line(slots_per_line);

        let space_between_lines_dp = if is_vertical {
            c.vertical_arrangement.as_ref().expect("null verticalArrangement when isVertical == true").spacing()
        } else {
            c.horizontal_arrangement.as_ref().expect("null horizontalArrangement when isVertical == false").spacing()
        };
        let space_between_lines = density.round_to_px(space_between_lines_dp);
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

        let reverse_layout = c.reverse_layout;
        let animator = state.inner.item_animator.clone();
        let measured_item_provider = LazyGridMeasuredItemProvider::new(
            item_provider.clone(),
            scope,
            space_between_lines,
            move |index, key, content_type, cross_axis_size, main_axis_spacing, placeables, constraints, lane, span| {
                LazyGridMeasuredItem::new(
                    index,
                    key,
                    is_vertical,
                    cross_axis_size,
                    main_axis_spacing,
                    reverse_layout,
                    layout_direction,
                    before_content_padding,
                    after_content_padding,
                    placeables,
                    visual_item_offset,
                    content_type,
                    animator.clone(),
                    constraints,
                    lane,
                    span,
                )
            },
        );
        let measured_line_provider = LazyGridMeasuredLineProvider::new(
            is_vertical,
            resolved_slots.clone(),
            items_count,
            space_between_lines,
            &measured_item_provider,
            item_provider.clone(),
        );
        let prefetch_info_retriever: PrefetchInfoRetriever = {
            let (content, slots) = (content.clone(), resolved_slots.clone());
            Rc::new(move |line| {
                let line_configuration = content.get_line_configuration(line);
                let mut slot = 0;
                let mut result = Vec::with_capacity(line_configuration.spans.len());
                for (index, span) in (line_configuration.first_item_index..).zip(&line_configuration.spans) {
                    let span = span.current_line_span();
                    result.push((index, child_constraints(&slots, is_vertical, slot, span)));
                    slot += span;
                }
                result
            })
        };
        let line_index_provider: Rc<dyn Fn(i32) -> i32> = {
            let content = content.clone();
            Rc::new(move |item_index| content.get_line_index_of_item(item_index))
        };

        let (first_visible_line_index, first_visible_line_scroll_offset) = without_read_observation(|| {
            let index = state
                .update_scroll_position_if_the_first_item_was_moved(&*item_provider, state.first_visible_item_index());
            if index < items_count || items_count <= 0 {
                (content.get_line_index_of_item(index), state.first_visible_item_scroll_offset())
            } else {
                // (fewer items than the index scrolled to)
                (content.get_line_index_of_item(items_count - 1), 0)
            }
        });

        let pinned_items = calculate_lazy_layout_pinned_indices(
            &*item_provider,
            &state.inner.pinned_items,
            &state.inner.beyond_bounds_info,
        );

        // (the lookahead pass scrolls; the approach after it by what it scrolled back of that, animated away)
        let scroll_to_be_consumed = if is_looking_ahead || !is_in_lookahead_scope {
            state.inner.scroll_to_be_consumed.get()
        } else {
            state.scroll_delta_between_passes()
        };
        let approach_layout_info = state.inner.approach_layout_info.borrow().as_ref().map(|r| r.layout_info());

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
        let measure_result = measure_lazy_grid(
            &measured_line_provider,
            &measured_item_provider,
            MeasureArgs {
                items_count,
                main_axis_available_size,
                before_content_padding,
                after_content_padding,
                space_between_lines,
                first_visible_line_index,
                first_visible_line_scroll_offset,
                scroll_to_be_consumed,
                constraints: content_constraints,
                is_vertical,
                vertical_arrangement: c.vertical_arrangement.clone(),
                horizontal_arrangement: c.horizontal_arrangement.clone(),
                reverse_layout,
                density,
                item_animator: state.inner.item_animator.clone(),
                slots_per_line,
                pinned_items,
                is_in_lookahead_scope,
                is_looking_ahead,
                approach_layout_info,
                coroutine_scope: c.coroutine_scope.clone(),
                placement_scope_invalidator: state.inner.placement_scope_invalidator.clone(),
                platform: c.platform.clone(),
                prefetch_info_retriever,
                line_index_provider,
                sticky_items_placement,
                layout: &layout,
            },
        );
        let result = Rc::new(measure_result);
        state.apply_measure_result(result.clone(), is_looking_ahead, false);
        // (the cache window's lines kept around after the strategy saw the result)
        if let Some(logic) = cache_window_logic_of(&*state.inner.prefetch_strategy) {
            keep_around_items(&logic, &result.visible_items_info, &measured_line_provider);
        }
        result.measure_result.clone()
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Beyond bounds and semantics

/// What the grid tells its beyond-bounds layout (Compose's LazyGridBeyondBoundsState).
struct LazyGridBeyondBoundsState {
    state: LazyGridState,
}

impl LazyLayoutBeyondBoundsState for LazyGridBeyondBoundsState {
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
        let line_average_size = info.visible_lines_average_main_axis_size();
        if line_average_size == 0 {
            return 1;
        }
        (viewport_size / line_average_size).max(1)
    }
}

/// What the grid's semantics read of its state (Compose's `rememberLazyGridSemanticState`): its collection's
/// rows and columns aren't told (-1, as Compose).
struct GridSemanticState {
    state: LazyGridState,
}

impl GridSemanticState {
    fn new(state: LazyGridState) -> GridSemanticState {
        GridSemanticState { state }
    }
}

impl LazyLayoutSemanticState for GridSemanticState {
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
