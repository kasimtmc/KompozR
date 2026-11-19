//! A lazy staggered grid's measure (Compose's LazyStaggeredGridMeasure.kt and LazyStaggeredGridMeasureResult.kt):
//! from each lane's first visible item and its offset, the scroll applied; items measured back up while there is
//! room before the first ones (the item of the largest index first: indices grow down the grid), then down from
//! the lane of the smallest offset, each item under its shortest lane (a full-line one under the longest, across
//! all), until every lane fills the viewport; scrolled back when the end leaves room. Lanes found misaligned (items
//! removed or resized) start the measure again once from a clean slate; the items out of view are dropped, the
//! pinned ones kept, the rest positioned lane by lane.

use super::cells::LazyStaggeredGridSlots;
use super::dsl::LazyStaggeredGridScope;
use super::grid::LazyStaggeredGridItemProvider;
use super::lane_info::{LaneFullSpan, LaneUnset};
use super::layout_info::{LazyStaggeredGridItemInfo, LazyStaggeredGridLayoutInfo};
use super::state::LazyStaggeredGridState;
use crate::gestures::Orientation;
use crate::lazy::layout::{
    LazyKey, LazyLayoutItemAnimation, LazyLayoutItemAnimator, LazyLayoutKeyIndexMap, LazyLayoutMeasureScope,
    LazyLayoutMeasuredItem, LazyLayoutMeasuredItemProvider, PlaceablesCache,
};
use kompoz_coroutines::CoroutineScope;
use kompoz_graphics::{Density, IntOffset, IntSize};
use kompoz_ui::layout::SlotKey;
use kompoz_ui::platform::PlatformHandle;
use kompoz_ui::{Constraints, MeasureResult, Placeable, PlacementScope};
use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// A measured item

const Unset: i32 = i32::MIN;

/// An item of a lazy staggered grid as measured (Compose's LazyStaggeredGridMeasuredItem): its placeables one over
/// the other in its lane (or the line), positioned in place.
pub(crate) struct LazyStaggeredGridMeasuredItem {
    index: i32,
    key: LazyKey,
    placeables: Vec<Placeable>,
    is_vertical: bool,
    lane: i32,
    span: i32,
    before_content_padding: i32,
    after_content_padding: i32,
    content_type: Option<SlotKey>,
    animator: Rc<LazyLayoutItemAnimator<LazyStaggeredGridMeasuredItem>>,
    constraints: Constraints,
    /// Whether it shows (one measured only to place the others isn't placed).
    pub(crate) is_visible: Cell<bool>,
    /// Its length along the grid: its longest placeable.
    pub(crate) main_axis_size: i32,
    horizontal_axis_size: i32,
    vertical_axis_size: i32,
    horizontal_axis_spacing: i32,
    vertical_axis_spacing: i32,
    cross_axis_size: i32,
    main_axis_layout_size: Cell<i32>,
    min_main_axis_offset: Cell<i32>,
    max_main_axis_offset: Cell<i32>,
    /// Not moved by a scroll applied without a measure (animated away).
    pub(crate) non_scrollable_item: Cell<bool>,
    offset: Cell<IntOffset>,
}

impl LazyStaggeredGridMeasuredItem {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        index: i32,
        key: LazyKey,
        placeables: Vec<Placeable>,
        is_vertical: bool,
        spacing: i32,
        lane: i32,
        span: i32,
        before_content_padding: i32,
        after_content_padding: i32,
        content_type: Option<SlotKey>,
        animator: Rc<LazyLayoutItemAnimator<LazyStaggeredGridMeasuredItem>>,
        constraints: Constraints,
    ) -> LazyStaggeredGridMeasuredItem {
        let main_axis_size =
            placeables.iter().map(|p| if is_vertical { p.height() } else { p.width() }).max().unwrap_or(0);
        let cross_axis_size =
            placeables.iter().map(|p| if is_vertical { p.width() } else { p.height() }).max().unwrap_or(0);
        let (vertical_axis_spacing, vertical_axis_size, horizontal_axis_size, horizontal_axis_spacing) = if is_vertical
        {
            (spacing, main_axis_size, cross_axis_size, 0)
        } else {
            (0, cross_axis_size, main_axis_size, spacing)
        };
        LazyStaggeredGridMeasuredItem {
            index,
            key,
            placeables,
            is_vertical,
            lane,
            span,
            before_content_padding,
            after_content_padding,
            content_type,
            animator,
            constraints,
            is_visible: Cell::new(true),
            main_axis_size,
            horizontal_axis_size,
            vertical_axis_size,
            horizontal_axis_spacing,
            vertical_axis_spacing,
            cross_axis_size,
            main_axis_layout_size: Cell::new(Unset),
            min_main_axis_offset: Cell::new(0),
            max_main_axis_offset: Cell::new(0),
            non_scrollable_item: Cell::new(false),
            offset: Cell::new(IntOffset::Zero),
        }
    }

    pub(crate) fn key_ref(&self) -> &LazyKey {
        &self.key
    }

    /// Its size along the grid with the spacing after it (none below 0).
    pub(crate) fn main_axis_size_with_spacings(&self) -> i32 {
        let v = if self.is_vertical {
            self.vertical_axis_size + self.vertical_axis_spacing
        } else {
            self.horizontal_axis_size + self.horizontal_axis_spacing
        };
        v.max(0)
    }

    pub(crate) fn size(&self) -> IntSize {
        if self.is_vertical {
            IntSize::new(self.cross_axis_size, self.main_axis_size)
        } else {
            IntSize::new(self.main_axis_size, self.cross_axis_size)
        }
    }

    /// Its offset along the grid (Compose's `mainAxisOffset`).
    pub(crate) fn main_axis_offset(&self) -> i32 {
        let o = self.offset.get();
        if !self.is_vertical { o.x } else { o.y }
    }

    /// Positions it at `main_axis` along the grid and `cross_axis` across, the layout `main_axis_layout_size` long
    /// (Compose's `position`).
    pub(crate) fn position_at(&self, main_axis: i32, cross_axis: i32, main_axis_layout_size: i32) {
        self.main_axis_layout_size.set(main_axis_layout_size);
        self.min_main_axis_offset.set(-self.before_content_padding);
        self.max_main_axis_offset.set(main_axis_layout_size + self.after_content_padding);
        self.offset.set(if self.is_vertical {
            IntOffset::new(cross_axis, main_axis)
        } else {
            IntOffset::new(main_axis, cross_axis)
        });
    }

    /// The layout's final length along the grid (Compose's `updateMainAxisLayoutSize`).
    pub(crate) fn update_main_axis_layout_size(&self, main_axis_layout_size: i32) {
        self.main_axis_layout_size.set(main_axis_layout_size);
        self.max_main_axis_offset.set(main_axis_layout_size + self.after_content_padding);
    }

    /// Moves it by `delta` for a scroll applied without a measure (Compose's `applyScrollDelta`).
    pub(crate) fn apply_scroll_delta(&self, delta: i32, update_animations: bool) {
        if self.non_scrollable_item.get() {
            return;
        }
        self.offset.set(self.copy_main(self.offset.get(), |m| m + delta));
        if update_animations {
            for index in 0..self.placeables.len() {
                if let Some(animation) = self.animator.get_animation(&self.key, index) {
                    let target = animation.target_offset.get();
                    animation.target_offset.set(self.copy_main(target, |m| m + delta));
                }
            }
        }
    }

    fn main_axis(&self, o: IntOffset) -> i32 {
        if self.is_vertical { o.y } else { o.x }
    }

    fn copy_main(&self, o: IntOffset, map: impl FnOnce(i32) -> i32) -> IntOffset {
        if self.is_vertical { IntOffset::new(o.x, map(o.y)) } else { IntOffset::new(map(o.x), o.y) }
    }

    fn placeable_main_axis_size(&self, p: &Placeable) -> i32 {
        if self.is_vertical { p.height() } else { p.width() }
    }

    /// Places it (Compose's `place`): each placeable at its offset plus its animation's delta (an animation whose
    /// item and animated place are both out of view cancelled), mirrored for a reversed grid, moved by the content
    /// offset (the padding), relative (mirrored right to left), through its animation's layer if it has one.
    pub(crate) fn place(
        &self,
        scope: &PlacementScope,
        reverse_layout: bool,
        content_offset: IntOffset,
        is_looking_ahead: bool,
    ) {
        assert!(self.main_axis_layout_size.get() != Unset, "position() should be called first");
        for (index, placeable) in self.placeables.iter().enumerate() {
            let min_offset = self.min_main_axis_offset.get() - self.placeable_main_axis_size(placeable);
            let max_offset = self.max_main_axis_offset.get();
            let mut offset = self.offset.get();
            let animation = self.animator.get_animation(&self.key, index);
            let layer = match &animation {
                Some(animation) => {
                    if is_looking_ahead {
                        // (no animation in the lookahead pass)
                        animation.lookahead_offset.set(offset);
                    } else {
                        let target_offset =
                            if animation.lookahead_offset.get() != LazyLayoutItemAnimation::NotInitialized {
                                animation.lookahead_offset.get()
                            } else {
                                offset
                            };
                        let animated_offset = target_offset + animation.placement_delta();
                        let main = self.main_axis(offset);
                        let animated_main = self.main_axis(animated_offset);
                        if (main <= min_offset && animated_main <= min_offset)
                            || (main >= max_offset && animated_main >= max_offset)
                        {
                            animation.cancel_placement_animation();
                        }
                        offset = animated_offset;
                    }
                    animation.layer()
                }
                None => None,
            };
            if reverse_layout {
                let layout_size = self.main_axis_layout_size.get();
                let size = self.placeable_main_axis_size(placeable);
                offset = self.copy_main(offset, |m| layout_size - m - size);
            }
            offset = offset + content_offset;
            if !is_looking_ahead && let Some(animation) = &animation {
                animation.placement_offset.set(offset);
            }
            match &layer {
                Some(layer) => scope.place_relative_with_graphics_layer(placeable, offset, layer, 0.0),
                None => scope.place_relative_with_layer_block(placeable, offset, default_layer_block(), 0.0),
            }
        }
    }

    /// What the layout info tells of it.
    pub(crate) fn info(&self) -> LazyStaggeredGridItemInfo {
        LazyStaggeredGridItemInfo {
            offset: self.offset.get(),
            index: self.index,
            lane: self.lane,
            key: self.key.clone(),
            size: self.size(),
            content_type: self.content_type.clone(),
        }
    }
}

thread_local! {
    /// The layer block of an item without animations (Compose's `placeRelativeWithLayer(offset)`), one for all.
    static DEFAULT_LAYER_BLOCK: kompoz_ui::layout::LayerBlock = Rc::new(|_: &mut kompoz_ui::layout::GraphicsLayerScope| {});
}

fn default_layer_block() -> kompoz_ui::layout::LayerBlock {
    DEFAULT_LAYER_BLOCK.with(|b| b.clone())
}

impl LazyLayoutMeasuredItem for LazyStaggeredGridMeasuredItem {
    fn index(&self) -> i32 {
        self.index
    }
    fn key(&self) -> LazyKey {
        self.key.clone()
    }
    fn placeables(&self) -> &[Placeable] {
        &self.placeables
    }
    fn horizontal_axis_size(&self) -> i32 {
        self.horizontal_axis_size
    }
    fn vertical_axis_size(&self) -> i32 {
        self.vertical_axis_size
    }
    fn horizontal_axis_spacing(&self) -> i32 {
        self.horizontal_axis_spacing
    }
    fn vertical_axis_spacing(&self) -> i32 {
        self.vertical_axis_spacing
    }
    fn constraints(&self) -> Constraints {
        self.constraints
    }
    fn lane(&self) -> i32 {
        self.lane
    }
    fn span(&self) -> i32 {
        self.span
    }
    fn get_offset(&self, _placeable_index: usize) -> IntOffset {
        self.offset.get()
    }
    /// (the main axis offset first, the cross axis one second)
    fn position(&self, horizontal_axis_offset: i32, vertical_axis_offset: i32, layout_width: i32, layout_height: i32) {
        self.position_at(
            horizontal_axis_offset,
            vertical_axis_offset,
            if self.is_vertical { layout_height } else { layout_width },
        );
    }
    fn make_non_scrollable(&self) {
        self.non_scrollable_item.set(true);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The measured item provider

/// The lanes an item takes (Compose's SpanRange): from `start` to before `end`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct SpanRange {
    pub(crate) start: i32,
    pub(crate) end: i32,
}

impl SpanRange {
    pub(crate) fn new(lane: i32, span: i32) -> SpanRange {
        SpanRange { start: lane, end: lane + span }
    }

    pub(crate) fn size(&self) -> i32 {
        self.end - self.start
    }

    pub(crate) fn is_full_span(&self) -> bool {
        self.size() != 1
    }

    /// What the lane info records of it: its lane, or the full line.
    pub(crate) fn lane_info(&self) -> i32 {
        if self.is_full_span() { LaneFullSpan } else { self.start }
    }

    fn lanes(&self) -> std::ops::Range<i32> {
        self.start..self.end
    }
}

/// An item's maker: its index, lane, span, key, content type, placeables and constraints to its measured item.
type CreateItem<'a> =
    dyn Fn(i32, i32, i32, LazyKey, Option<SlotKey>, Vec<Placeable>, Constraints) -> LazyStaggeredGridMeasuredItem + 'a;

/// What makes a staggered grid's items into measured items (Compose's LazyStaggeredGridMeasureProvider): composed
/// and measured in the lanes they take.
pub(crate) struct LazyStaggeredGridMeasureProvider<'a> {
    is_vertical: bool,
    item_provider: Rc<dyn LazyStaggeredGridItemProvider>,
    measure_scope: &'a LazyLayoutMeasureScope,
    resolved_slots: Rc<LazyStaggeredGridSlots>,
    placeables_cache: PlaceablesCache,
    create_item: Box<CreateItem<'a>>,
}

impl<'a> LazyStaggeredGridMeasureProvider<'a> {
    pub(crate) fn new(
        is_vertical: bool,
        item_provider: Rc<dyn LazyStaggeredGridItemProvider>,
        measure_scope: &'a LazyLayoutMeasureScope,
        resolved_slots: Rc<LazyStaggeredGridSlots>,
        create_item: impl Fn(
            i32,
            i32,
            i32,
            LazyKey,
            Option<SlotKey>,
            Vec<Placeable>,
            Constraints,
        ) -> LazyStaggeredGridMeasuredItem
        + 'a,
    ) -> Self {
        LazyStaggeredGridMeasureProvider {
            is_vertical,
            item_provider,
            measure_scope,
            resolved_slots,
            placeables_cache: PlaceablesCache::default(),
            create_item: Box::new(create_item),
        }
    }

    /// The constraints of an item from lane `slot` spanning `span` lanes: fixed across.
    fn child_constraints(&self, slot: i32, span: i32) -> Constraints {
        let slots = &self.resolved_slots;
        let cross_axis_size = if span == 1 {
            slots.sizes[slot as usize]
        } else {
            let start = slots.positions[slot as usize];
            let end_slot = (slot + span - 1) as usize;
            let end = slots.positions[end_slot] + slots.sizes[end_slot];
            end - start
        };
        if self.is_vertical {
            Constraints::fixed_width(cross_axis_size)
        } else {
            Constraints::fixed_height(cross_axis_size)
        }
    }

    /// Item `index` composed and measured in the lanes of `span` (Compose's `getAndMeasure(index, span)`).
    pub(crate) fn get_and_measure_span(&self, index: i32, span: SpanRange) -> Rc<LazyStaggeredGridMeasuredItem> {
        let key = self.item_provider.get_key(index);
        let content_type = self.item_provider.get_content_type(index);
        let slot_count = self.resolved_slots.sizes.len() as i32;
        let span_start = span.start.min(slot_count - 1);
        let span_size = span.size().min(slot_count - span_start);
        let constraints = self.child_constraints(span_start, span_size);
        let placeables = self.placeables_cache.get_placeables(self.measure_scope, index, constraints);
        Rc::new((self.create_item)(index, span_start, span_size, key, content_type, placeables, constraints))
    }

    pub(crate) fn key_index_map(&self) -> Rc<dyn LazyLayoutKeyIndexMap> {
        self.item_provider.key_index_map()
    }
}

impl LazyLayoutMeasuredItemProvider<LazyStaggeredGridMeasuredItem> for LazyStaggeredGridMeasureProvider<'_> {
    fn get_and_measure(
        &self,
        index: i32,
        lane: i32,
        span: i32,
        constraints: Constraints,
    ) -> Rc<LazyStaggeredGridMeasuredItem> {
        let key = self.item_provider.get_key(index);
        let content_type = self.item_provider.get_content_type(index);
        let placeables = self.placeables_cache.get_placeables(self.measure_scope, index, constraints);
        Rc::new((self.create_item)(index, lane, span, key, content_type, placeables, constraints))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The measure result

/// What a lazy staggered grid's measure gives (Compose's LazyStaggeredGridMeasureResult): each lane's first visible
/// item and its offset, the measure result to place, what scroll it took, the layout info.
#[derive(Clone)]
pub(crate) struct LazyStaggeredGridMeasureResult {
    pub(crate) first_visible_item_indices: Vec<i32>,
    pub(crate) first_visible_item_scroll_offsets: Vec<i32>,
    pub(crate) consumed_scroll: f32,
    pub(crate) measure_result: MeasureResult,
    /// What its approach would scroll back at the end, in a lookahead pass (the state animates it away between
    /// the passes).
    pub(crate) scroll_back_amount: f32,
    pub(crate) can_scroll_forward: bool,
    pub(crate) is_vertical: bool,
    pub(crate) remeasure_needed: bool,
    pub(crate) slots: Rc<LazyStaggeredGridSlots>,
    /// The content whose spans tell full-line items (Compose's `spanProvider`).
    pub(crate) span_provider: Rc<LazyStaggeredGridScope>,
    pub(crate) density: Density,
    pub(crate) total_items_count: i32,
    pub(crate) visible_items_info: Vec<Rc<LazyStaggeredGridMeasuredItem>>,
    pub(crate) viewport_size: IntSize,
    pub(crate) viewport_start_offset: i32,
    pub(crate) viewport_end_offset: i32,
    pub(crate) before_content_padding: i32,
    pub(crate) after_content_padding: i32,
    pub(crate) main_axis_item_spacing: i32,
    pub(crate) coroutine_scope: Option<CoroutineScope>,
    pub(crate) reverse_layout: bool,
}

impl LazyStaggeredGridMeasureResult {
    /// The result before the first measure (Compose's EmptyLazyStaggeredGridLayoutInfo).
    pub(crate) fn empty() -> LazyStaggeredGridMeasureResult {
        LazyStaggeredGridMeasureResult {
            first_visible_item_indices: Vec::new(),
            first_visible_item_scroll_offsets: Vec::new(),
            consumed_scroll: 0.0,
            measure_result: MeasureResult::new(0, 0, Vec::new(), |_| {}),
            scroll_back_amount: 0.0,
            can_scroll_forward: false,
            is_vertical: false,
            remeasure_needed: false,
            slots: Rc::new(LazyStaggeredGridSlots::default()),
            span_provider: Rc::new(LazyStaggeredGridScope::default()),
            density: Density::new(1.0, 1.0),
            total_items_count: 0,
            visible_items_info: Vec::new(),
            viewport_size: IntSize::Zero,
            viewport_start_offset: 0,
            viewport_end_offset: 0,
            before_content_padding: 0,
            after_content_padding: 0,
            main_axis_item_spacing: 0,
            coroutine_scope: None,
            reverse_layout: false,
        }
    }

    /// Whether it can scroll back: the first item not at the start (Compose's `canScrollBackward`).
    pub(crate) fn can_scroll_backward(&self) -> bool {
        match (self.first_visible_item_indices.first(), self.first_visible_item_scroll_offsets.first()) {
            (Some(&index), Some(&offset)) => !(index == 0 && offset <= 0),
            // (Compose reads index 0 of an empty array only before the first measure: it can't)
            _ => false,
        }
    }

    pub(crate) fn orientation(&self) -> Orientation {
        if self.is_vertical { Orientation::Vertical } else { Orientation::Horizontal }
    }

    /// The layout info it is (Compose's LazyStaggeredGridLayoutInfo, the result itself).
    pub(crate) fn layout_info(&self) -> LazyStaggeredGridLayoutInfo {
        LazyStaggeredGridLayoutInfo {
            orientation: self.orientation(),
            visible_items_info: Rc::new(self.visible_items_info.iter().map(|i| i.info()).collect()),
            total_items_count: self.total_items_count,
            viewport_size: self.viewport_size,
            viewport_start_offset: self.viewport_start_offset,
            viewport_end_offset: self.viewport_end_offset,
            before_content_padding: self.before_content_padding,
            after_content_padding: self.after_content_padding,
            main_axis_item_spacing: self.main_axis_item_spacing,
            reverse_layout: self.reverse_layout,
            density: self.density,
        }
    }

    /// The same result scrolled by `delta` without a measure, if no item crosses the start or the viewport's ends
    /// (Compose's `copyWithScrollDeltaWithoutRemeasure`).
    pub(crate) fn copy_with_scroll_delta_without_remeasure(
        &self,
        delta: i32,
        update_animations: bool,
    ) -> Option<LazyStaggeredGridMeasureResult> {
        if self.remeasure_needed
            || self.visible_items_info.is_empty()
            || self.first_visible_item_indices.is_empty()
            || self.first_visible_item_scroll_offsets.is_empty()
        {
            return None;
        }
        let main_axis_max = self.viewport_end_offset - self.after_content_padding;
        for it in &self.visible_items_info {
            let offset = it.main_axis_offset();
            // (not scrolled with the others; or crossing the start: the first visible items change)
            if it.non_scrollable_item.get() || (offset <= 0) != (offset + delta <= 0) {
                return None;
            }
            if offset <= self.viewport_start_offset {
                // (going out of view at the start)
                let can_apply = if delta < 0 {
                    offset + it.main_axis_size_with_spacings() - self.viewport_start_offset > -delta
                } else {
                    self.viewport_start_offset - offset > delta
                };
                if !can_apply {
                    return None;
                }
            }
            // (partly in view at the end)
            if offset + it.main_axis_size_with_spacings() >= main_axis_max {
                let can_apply = if delta < 0 {
                    offset + it.main_axis_size_with_spacings() - self.viewport_end_offset > -delta
                } else {
                    self.viewport_end_offset - offset > delta
                };
                if !can_apply {
                    return None;
                }
            }
        }
        for it in &self.visible_items_info {
            it.apply_scroll_delta(delta, update_animations);
        }
        Some(LazyStaggeredGridMeasureResult {
            first_visible_item_scroll_offsets: self
                .first_visible_item_scroll_offsets
                .iter()
                .map(|o| o - delta)
                .collect(),
            consumed_scroll: delta as f32,
            // (scrolled back: it can scroll forward now)
            can_scroll_forward: self.can_scroll_forward || delta > 0,
            ..self.clone()
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The measure

/// What the measure works with (Compose's LazyStaggeredGridMeasureContext).
pub(crate) struct LazyStaggeredGridMeasureContext<'a> {
    pub(crate) state: LazyStaggeredGridState,
    pub(crate) pinned_items: Vec<i32>,
    pub(crate) item_provider: Rc<dyn LazyStaggeredGridItemProvider>,
    pub(crate) resolved_slots: Rc<LazyStaggeredGridSlots>,
    pub(crate) constraints: Constraints,
    pub(crate) is_vertical: bool,
    pub(crate) measure_scope: &'a LazyLayoutMeasureScope,
    pub(crate) main_axis_available_size: i32,
    pub(crate) content_offset: IntOffset,
    pub(crate) before_content_padding: i32,
    pub(crate) after_content_padding: i32,
    pub(crate) reverse_layout: bool,
    pub(crate) main_axis_spacing: i32,
    pub(crate) coroutine_scope: CoroutineScope,
    pub(crate) platform: Option<PlatformHandle>,
    pub(crate) is_in_lookahead_scope: bool,
    pub(crate) is_looking_ahead: bool,
    /// The last approach pass's items (in a lookahead scope, after one).
    pub(crate) approach_visible_items: Option<Vec<LazyStaggeredGridItemInfo>>,
    pub(crate) measured_item_provider: LazyStaggeredGridMeasureProvider<'a>,
    pub(crate) lane_count: i32,
}

/// Measures a lazy staggered grid (Compose's `measureStaggeredGrid`): the lanes' first items found again (the
/// grid's lanes changed: worked out anew), then measured.
#[allow(clippy::too_many_arguments)]
pub(crate) fn measure_staggered_grid(
    measure_scope: &LazyLayoutMeasureScope,
    state: &LazyStaggeredGridState,
    pinned_items: Vec<i32>,
    item_provider: Rc<dyn LazyStaggeredGridItemProvider>,
    resolved_slots: Rc<LazyStaggeredGridSlots>,
    constraints: Constraints,
    is_vertical: bool,
    reverse_layout: bool,
    content_offset: IntOffset,
    main_axis_available_size: i32,
    main_axis_spacing: i32,
    before_content_padding: i32,
    after_content_padding: i32,
    coroutine_scope: CoroutineScope,
    platform: Option<PlatformHandle>,
    is_in_lookahead_scope: bool,
    is_looking_ahead: bool,
) -> LazyStaggeredGridMeasureResult {
    let animator = state.inner.item_animator.clone();
    let measured_item_provider = LazyStaggeredGridMeasureProvider::new(
        is_vertical,
        item_provider.clone(),
        measure_scope,
        resolved_slots.clone(),
        move |index, lane, span, key, content_type, placeables, constraints| {
            LazyStaggeredGridMeasuredItem::new(
                index,
                key,
                placeables,
                is_vertical,
                main_axis_spacing,
                lane,
                span,
                before_content_padding,
                after_content_padding,
                content_type,
                animator.clone(),
                constraints,
            )
        },
    );
    let lane_count = resolved_slots.sizes.len() as i32;
    let context = LazyStaggeredGridMeasureContext {
        state: state.clone(),
        pinned_items,
        item_provider: item_provider.clone(),
        resolved_slots,
        constraints,
        is_vertical,
        measure_scope,
        main_axis_available_size,
        content_offset,
        before_content_padding,
        after_content_padding,
        reverse_layout,
        main_axis_spacing,
        coroutine_scope,
        platform,
        is_in_lookahead_scope,
        is_looking_ahead,
        // (the last approach pass's items: those past the lookahead's kept for it)
        approach_visible_items: state
            .inner
            .approach_layout_info
            .borrow()
            .as_ref()
            .map(|r| r.visible_items_info.iter().map(|i| i.info()).collect()),
        measured_item_provider,
        lane_count,
    };

    // The scroll position up to date.
    let current_indices = state.inner.scroll_position.indices();
    let first_visible_indices =
        state.update_scroll_position_if_the_first_item_was_moved(&*item_provider, &current_indices);
    let first_visible_offsets = state.inner.scroll_position.scroll_offsets();

    let initial_item_indices = if first_visible_indices.len() as i32 == lane_count {
        first_visible_indices
    } else {
        // (the grid resized, or the first measure: the indices adjusted)
        state.inner.lane_info.borrow_mut().reset();
        let mut indices = vec![0; lane_count as usize];
        for lane in 0..lane_count as usize {
            indices[lane] = if lane < first_visible_indices.len() && first_visible_indices[lane] != LaneUnset {
                first_visible_indices[lane]
            } else if lane == 0 {
                0
            } else {
                max_in_range(&indices, SpanRange::new(0, lane as i32)) + 1
            };
            // (the lanes in the right range)
            state.inner.lane_info.borrow_mut().set_lane(indices[lane], lane as i32);
        }
        indices
    };
    let initial_item_offsets = if first_visible_offsets.len() as i32 == lane_count {
        first_visible_offsets
    } else {
        // (the offsets matched to those set before)
        let mut offsets = vec![0; lane_count as usize];
        for lane in 0..lane_count as usize {
            offsets[lane] = if lane < first_visible_offsets.len() {
                first_visible_offsets[lane]
            } else if lane == 0 {
                0
            } else {
                offsets[lane - 1]
            };
        }
        offsets
    };

    let initial_scroll_delta = fast_round_to_int(state.scroll_to_be_consumed(is_looking_ahead));
    context.measure(initial_scroll_delta, initial_item_indices, initial_item_offsets, true)
}

impl LazyStaggeredGridMeasureContext<'_> {
    fn is_full_span(&self, item_index: i32) -> bool {
        self.item_provider.span_provider().is_full_span(item_index)
    }

    /// The lanes item `item_index` takes going into `lane` (Compose's `getSpanRange`): all for a full-line one.
    fn get_span_range(&self, item_index: i32, lane: i32) -> SpanRange {
        let is_full_span = self.is_full_span(item_index);
        let span = if is_full_span { self.lane_count } else { 1 };
        let target_lane = if is_full_span { 0 } else { lane };
        SpanRange::new(target_lane, span)
    }

    fn find_previous_item_index(&self, item: i32, lane: i32) -> i32 {
        self.state.inner.lane_info.borrow().find_previous_item_index(item, lane)
    }

    fn set_lane(&self, item_index: i32, lane: i32) {
        self.state.inner.lane_info.borrow_mut().set_lane(item_index, lane);
    }

    fn get_lane(&self, item_index: i32) -> i32 {
        self.state.inner.lane_info.borrow().get_lane(item_index)
    }

    fn get_gaps(&self, item_index: i32) -> Option<Vec<i32>> {
        self.state.inner.lane_info.borrow().get_gaps(item_index)
    }

    fn set_gaps(&self, item_index: i32, gaps: Option<Vec<i32>>) {
        self.state.inner.lane_info.borrow_mut().set_gaps(item_index, gaps);
    }

    fn reset_lanes(&self) {
        self.state.inner.lane_info.borrow_mut().reset();
    }

    fn layout(&self, width: i32, height: i32, placement: Box<dyn Fn(&PlacementScope)>) -> MeasureResult {
        self.measure_scope.layout(width, height, placement)
    }

    /// Compose's `measure`: see the module's doc.
    fn measure(
        &self,
        initial_scroll_delta: i32,
        initial_item_indices: Vec<i32>,
        initial_item_offsets: Vec<i32>,
        can_restart_measure: bool,
    ) -> LazyStaggeredGridMeasureResult {
        let item_count = self.item_provider.item_count();
        let lane_count = self.lane_count;
        let animator = self.state.inner.item_animator.clone();

        if item_count <= 0 || lane_count == 0 {
            let mut layout_width = self.constraints.min_width;
            let mut layout_height = self.constraints.min_height;
            let mut positioned: Vec<Rc<LazyStaggeredGridMeasuredItem>> = Vec::new();
            animator.on_measured(
                0,
                layout_width,
                layout_height,
                &mut positioned,
                self.measured_item_provider.key_index_map(),
                &self.measured_item_provider,
                self.is_vertical,
                self.is_looking_ahead,
                lane_count,
                self.is_in_lookahead_scope,
                0,
                0,
                &self.coroutine_scope,
                &self.platform,
            );
            if !self.is_looking_ahead {
                let disappearing_items_size = animator.min_size_to_fit_disappearing_items();
                if disappearing_items_size != IntSize::Zero {
                    layout_width = self.constraints.constrain_width(disappearing_items_size.width);
                    layout_height = self.constraints.constrain_height(disappearing_items_size.height);
                }
            }
            return LazyStaggeredGridMeasureResult {
                first_visible_item_indices: initial_item_indices,
                first_visible_item_scroll_offsets: initial_item_offsets,
                consumed_scroll: 0.0,
                measure_result: self.layout(layout_width, layout_height, Box::new(|_| {})),
                scroll_back_amount: 0.0,
                can_scroll_forward: false,
                is_vertical: self.is_vertical,
                remeasure_needed: false,
                slots: self.resolved_slots.clone(),
                span_provider: self.item_provider.span_provider().clone(),
                density: self.measure_scope.density(),
                total_items_count: item_count,
                visible_items_info: Vec::new(),
                viewport_size: IntSize::new(self.constraints.min_width, self.constraints.min_height),
                viewport_start_offset: -self.before_content_padding,
                viewport_end_offset: self.main_axis_available_size + self.after_content_padding,
                before_content_padding: self.before_content_padding,
                after_content_padding: self.after_content_padding,
                main_axis_item_spacing: self.main_axis_spacing,
                coroutine_scope: Some(self.coroutine_scope.clone()),
                reverse_layout: self.reverse_layout,
            };
        }

        // The scroll applied by this measure.
        let mut scroll_delta = initial_scroll_delta;
        let mut first_item_indices = initial_item_indices;
        let mut first_item_offsets = initial_item_offsets;
        // (items composed only to know their size, out of view in the end: a measure again disposes of them)
        let mut remeasure_needed = false;

        // (the item count lower than before: the indices in range)
        self.ensure_indices_in_range(&mut first_item_indices, item_count);

        // (all of the scroll applied; what can't be is found later)
        offset_by(&mut first_item_offsets, -scroll_delta);

        let mut measured_items: Vec<VecDeque<Rc<LazyStaggeredGridMeasuredItem>>> =
            (0..lane_count).map(|_| VecDeque::with_capacity(16)).collect();

        // Items in the start padding composed too (taken off again before going forward).
        offset_by(&mut first_item_offsets, -self.before_content_padding);

        let main_axis_spacing = self.main_axis_spacing;
        let has_space_before_first = |indices: &[i32], offsets: &[i32]| {
            indices
                .iter()
                .zip(offsets)
                .any(|(&item_index, &item_offset)| item_offset < (-main_axis_spacing).max(0) && item_index > 0)
        };

        let mut lane_to_check_for_gaps = -1;

        // Scrolled back, or items in the start padding: the items before composed.
        while has_space_before_first(&first_item_indices, &first_item_offsets) {
            // (indices grow down the grid: the lane with the largest one has room before it first)
            let lane_index = index_of_max_value(&first_item_indices);
            let item_index = first_item_indices[lane_index];

            // (lanes above the chosen one were measured wrong — an item removed or resized: moved back to it)
            for i in 0..first_item_offsets.len() {
                if first_item_indices[i] != first_item_indices[lane_index]
                    && first_item_offsets[i] < first_item_offsets[lane_index]
                {
                    first_item_offsets[i] = first_item_offsets[lane_index];
                }
            }

            let previous_item_index = self.find_previous_item_index(item_index, lane_index as i32);
            if previous_item_index < 0 {
                lane_to_check_for_gaps = lane_index as i32;
                break;
            }

            let span_range = self.get_span_range(previous_item_index, lane_index as i32);
            self.set_lane(previous_item_index, span_range.lane_info());
            let measured_item = self.measured_item_provider.get_and_measure_span(previous_item_index, span_range);

            let offset = max_in_range(&first_item_offsets, span_range);
            let gaps = if span_range.is_full_span() { self.get_gaps(previous_item_index) } else { None };
            for lane in span_range.lanes() {
                let lane = lane as usize;
                first_item_indices[lane] = previous_item_index;
                let gap = gaps.as_ref().map_or(0, |g| g[lane]);
                let new_offset = offset + measured_item.main_axis_size_with_spacings() + gap;
                first_item_offsets[lane] = new_offset;
                // (out of view: composed for nothing)
                if self.main_axis_available_size + new_offset <= 0 {
                    remeasure_needed = true;
                }
            }
        }

        // Whether the lanes' starts don't line up (Compose's `misalignedStart`).
        let misaligned_start = |indices: &[i32], offsets: &[i32], reference_lane: usize| -> bool {
            // (each lane laid out all its items, but the offsets don't match)
            for lane in 0..indices.len() {
                if self.find_previous_item_index(indices[lane], lane as i32) == LaneUnset
                    && offsets[lane] != offsets[reference_lane]
                {
                    return true;
                }
            }
            // (some lanes miss items, and there is no room for them)
            for lane in 0..indices.len() {
                if self.find_previous_item_index(indices[lane], lane as i32) != LaneUnset
                    && offsets[lane] >= offsets[reference_lane]
                {
                    return true;
                }
            }
            // (the first item isn't in the first lane)
            let first_item_lane = self.get_lane(0);
            first_item_lane != 0 && first_item_lane != LaneUnset && first_item_lane != LaneFullSpan
        };

        // (the start padding included still)
        let min_offset = -self.before_content_padding;

        // Scrolled back past the first items: not all of the scroll taken.
        if first_item_offsets[0] < min_offset {
            let not_consumed_scroll_delta = min_offset - first_item_offsets[0];
            let by = min_offset - first_item_offsets[0];
            offset_by(&mut first_item_offsets, by);
            scroll_delta -= not_consumed_scroll_delta;
        }

        // (the start padding neutralised again)
        offset_by(&mut first_item_offsets, self.before_content_padding);

        if lane_to_check_for_gaps == -1 {
            lane_to_check_for_gaps = first_item_indices.iter().position(|&i| i == 0).map_or(-1, |at| at as i32);
        }

        // (the lanes lined up after the measure back? if not, measured again from a clean slate)
        if lane_to_check_for_gaps != -1 {
            let lane = lane_to_check_for_gaps as usize;
            if misaligned_start(&first_item_indices, &first_item_offsets, lane) && can_restart_measure {
                self.reset_lanes();
                return self.measure(
                    scroll_delta,
                    vec![-1; first_item_indices.len()],
                    vec![first_item_offsets[lane]; first_item_offsets.len()],
                    false,
                );
            }
        }

        // Down from the first items found (measured again here for their order; the layout caches them).
        let mut current_item_indices = first_item_indices.clone();
        let mut current_item_offsets: Vec<i32> = first_item_offsets.iter().map(|o| -o).collect();

        let min_visible_offset = min_offset + self.main_axis_spacing;
        let max_offset = (self.main_axis_available_size + self.after_content_padding).max(0);

        // (each lane's first item measured in index order: a lane is chosen from the sizes and offsets of all
        // items before)
        let mut initial_items_measured = 0;
        let mut initial_lane_to_measure = index_of_min_value(&current_item_indices, i32::MIN);
        while initial_lane_to_measure != -1 && initial_items_measured < lane_count {
            let lane_index = initial_lane_to_measure;
            let item_index = current_item_indices[lane_index as usize];

            initial_lane_to_measure = index_of_min_value(&current_item_indices, item_index);
            initial_items_measured += 1;

            if item_index < 0 {
                continue;
            }

            let span_range = self.get_span_range(item_index, lane_index);
            let measured_item = self.measured_item_provider.get_and_measure_span(item_index, span_range);

            self.set_lane(item_index, span_range.lane_info());
            let offset = max_in_range(&current_item_offsets, span_range);
            let mut gaps = if span_range.is_full_span() {
                Some(self.get_gaps(item_index).unwrap_or_else(|| vec![0; lane_count as usize]))
            } else {
                None
            };
            for lane in span_range.lanes() {
                let lane = lane as usize;
                if let Some(gaps) = &mut gaps {
                    gaps[lane] = offset - current_item_offsets[lane];
                }
                current_item_offsets[lane] = offset + measured_item.main_axis_size_with_spacings();
                current_item_indices[lane] = item_index;
                measured_items[lane].push_back(measured_item.clone());
            }

            // (both its ends out of view: measured to place the others, not placed)
            if offset < min_visible_offset && current_item_offsets[span_range.start as usize] <= min_visible_offset {
                measured_item.is_visible.set(false);
                remeasure_needed = true;
            }

            if span_range.is_full_span() {
                self.set_gaps(item_index, gaps);
                // (a full-line item covers the other lanes: they aren't measured here)
                initial_items_measured = lane_count;
            }
        }

        // Then forward until the viewport is filled (at least one item, even if all are out of view: a padding
        // bigger than the viewport).
        while current_item_offsets.iter().any(|&o| o < max_offset || o <= 0)
            || measured_items.iter().all(|l| l.is_empty())
        {
            let current_lane_index = index_of_min_value(&current_item_offsets, i32::MIN);
            let previous_item_index = *current_item_indices.iter().max().unwrap();
            let item_index = previous_item_index + 1;
            if item_index >= item_count {
                break;
            }

            let span_range = self.get_span_range(item_index, current_lane_index);
            self.set_lane(item_index, span_range.lane_info());
            let measured_item = self.measured_item_provider.get_and_measure_span(item_index, span_range);

            let offset = max_in_range(&current_item_offsets, span_range);
            let mut gaps = if span_range.is_full_span() {
                Some(self.get_gaps(item_index).unwrap_or_else(|| vec![0; lane_count as usize]))
            } else {
                None
            };
            for lane in span_range.lanes() {
                let lane = lane as usize;
                if let Some(gaps) = &mut gaps {
                    gaps[lane] = offset - current_item_offsets[lane];
                }
                current_item_indices[lane] = item_index;
                current_item_offsets[lane] = offset + measured_item.main_axis_size_with_spacings();
                measured_items[lane].push_back(measured_item.clone());
            }
            self.set_gaps(item_index, gaps);

            // (both its ends out of view: not placed)
            if offset < min_visible_offset && current_item_offsets[span_range.start as usize] <= min_visible_offset {
                measured_item.is_visible.set(false);
            }
        }

        // The items out of view dropped, the first indices and offsets moved on.
        for lane_index in 0..measured_items.len() {
            let lane_items = &mut measured_items[lane_index];
            while lane_items.len() > 1 && !lane_items.front().unwrap().is_visible.get() {
                let item = lane_items.pop_front().unwrap();
                let gaps = if item.span != 1 { self.get_gaps(item.index) } else { None };
                first_item_offsets[lane_index] -=
                    item.main_axis_size_with_spacings() + gaps.as_ref().map_or(0, |g| g[lane_index]);
            }
            first_item_indices[lane_index] = lane_items.front().map_or(LaneUnset, |i| i.index);
        }

        // (no spacing after the last item)
        if current_item_indices.iter().any(|&i| i == item_count - 1) {
            offset_by(&mut current_item_offsets, -self.main_axis_spacing);
        }

        let pre_scroll_back_scroll_delta = scroll_delta;
        // The viewport not filled from the first items: scrolled back if there are items before them.
        if current_item_offsets.iter().all(|&o| o < self.main_axis_available_size) {
            let max_offset_lane = index_of_max_value(&current_item_offsets);
            let to_scroll_back = self.main_axis_available_size - current_item_offsets[max_offset_lane];
            offset_by(&mut first_item_offsets, -to_scroll_back);
            offset_by(&mut current_item_offsets, to_scroll_back);

            let mut gap_detected = false;
            while first_item_offsets.iter().any(|&o| o < self.before_content_padding) {
                // (the lane of the smallest offset first: unlike going up at first, the items are distributed
                // already and the gaps would be wrong if moved)
                let mut lane_index = index_of_min_value(&first_item_offsets, i32::MIN) as usize;
                let next_lane_index = index_of_max_value(&first_item_indices);
                if lane_index != next_lane_index {
                    if first_item_offsets[lane_index] == first_item_offsets[next_lane_index] {
                        // (no gap: the lane where the item would go)
                        lane_index = next_lane_index;
                    } else {
                        // (misaligned: measured to the end, then again from there)
                        gap_detected = true;
                    }
                }

                let current_index =
                    if first_item_indices[lane_index] == -1 { item_count } else { first_item_indices[lane_index] };
                let previous_index = self.find_previous_item_index(current_index, lane_index as i32);
                if previous_index < 0 {
                    if (gap_detected || misaligned_start(&first_item_indices, &first_item_offsets, lane_index))
                        && can_restart_measure
                    {
                        self.reset_lanes();
                        return self.measure(
                            scroll_delta,
                            vec![-1; first_item_indices.len()],
                            vec![first_item_offsets[lane_index]; first_item_offsets.len()],
                            false,
                        );
                    }
                    break;
                }

                let span_range = self.get_span_range(previous_index, lane_index as i32);
                self.set_lane(previous_index, span_range.lane_info());
                let measured_item = self.measured_item_provider.get_and_measure_span(previous_index, span_range);

                let offset = max_in_range(&first_item_offsets, span_range);
                let gaps = if span_range.is_full_span() { self.get_gaps(previous_index) } else { None };
                for lane in span_range.lanes() {
                    let lane = lane as usize;
                    if first_item_offsets[lane] != offset {
                        // (items under a full-line one don't line up with it: skipped, measured again)
                        gap_detected = true;
                    }
                    measured_items[lane].push_front(measured_item.clone());
                    first_item_indices[lane] = previous_index;
                    let gap = gaps.as_ref().map_or(0, |g| g[lane]);
                    first_item_offsets[lane] = offset + measured_item.main_axis_size_with_spacings() + gap;
                }
            }

            // (lanes misaligned: measured again from here, the items redistributed going down)
            if gap_detected && can_restart_measure {
                self.reset_lanes();
                return self.measure(scroll_delta, first_item_indices, first_item_offsets, false);
            }

            scroll_delta += to_scroll_back;

            let min_offset_lane = index_of_min_value(&first_item_offsets, i32::MIN) as usize;
            if first_item_offsets[min_offset_lane] < 0 {
                let offset_value = first_item_offsets[min_offset_lane];
                scroll_delta += offset_value;
                offset_by(&mut current_item_offsets, offset_value);
                offset_by(&mut first_item_offsets, -offset_value);
            }
        }

        // The scroll taken: less than asked when there weren't items enough, more when items resized or the data
        // set shrank.
        let scroll_to_be_consumed = self.state.scroll_to_be_consumed(self.is_looking_ahead);
        let rounded = fast_round_to_int(scroll_to_be_consumed);
        let consumed_scroll = if rounded.signum() == scroll_delta.signum() && rounded.abs() >= scroll_delta.abs() {
            scroll_delta as f32
        } else {
            scroll_to_be_consumed
        };
        let unconsumed_scroll = scroll_to_be_consumed - consumed_scroll;
        // (a gesture's scroll back at the end, in the lookahead pass)
        let scroll_back_amount =
            if self.is_looking_ahead && scroll_delta > pre_scroll_back_scroll_delta && unconsumed_scroll <= 0.0 {
                (scroll_delta - pre_scroll_back_scroll_delta) as f32 + unconsumed_scroll
            } else {
                0.0
            };
        let mut item_scroll_offsets: Vec<i32> = first_item_offsets.iter().map(|o| -o).collect();

        // Items wholly in the start padding (composed to fill it) aren't the first visible ones.
        if self.before_content_padding > self.main_axis_spacing {
            for lane_index in 0..measured_items.len() {
                let lane_items = &measured_items[lane_index];
                for i in 0..lane_items.len() {
                    let item = &lane_items[i];
                    let gaps = self.get_gaps(item.index);
                    let size = item.main_axis_size_with_spacings() + gaps.as_ref().map_or(0, |g| g[lane_index]);
                    if i != lane_items.len() - 1
                        && first_item_offsets[lane_index] != 0
                        && first_item_offsets[lane_index] >= size
                    {
                        first_item_offsets[lane_index] -= size;
                        first_item_indices[lane_index] = lane_items[i + 1].index;
                    } else {
                        break;
                    }
                }
            }
        }

        // The placement.
        let content_padding = self.before_content_padding + self.after_content_padding;
        let max_current = *current_item_offsets.iter().max().unwrap();
        let mut layout_width = if self.is_vertical {
            self.constraints.max_width
        } else {
            self.constraints.constrain_width(max_current + content_padding)
        };
        let mut layout_height = if self.is_vertical {
            self.constraints.constrain_height(max_current + content_padding)
        } else {
            self.constraints.max_height
        };

        // (the offsets go from -before padding to the size and the after padding: the layout's length the same)
        let main_axis_layout_size = (if self.is_vertical { layout_height } else { layout_width })
            .min(self.main_axis_available_size)
            - self.before_content_padding
            + self.after_content_padding;

        let mut extra_item_offset = item_scroll_offsets[0];
        let extra_items_before = self.calculate_extra_items(
            |it| {
                extra_item_offset -= it.main_axis_size_with_spacings();
                it.position_at(extra_item_offset, 0, main_axis_layout_size);
            },
            |item_index| {
                let lane = self.get_lane(item_index);
                if lane == LaneUnset || lane == LaneFullSpan {
                    measured_items.iter().all(|l| l.front().map_or(-1, |i| i.index) > item_index)
                } else {
                    measured_items[lane as usize].front().map_or(-1, |i| i.index) > item_index
                }
            },
            true,
        );

        let visible_items = self.calculate_visible_items(
            &mut measured_items,
            &mut item_scroll_offsets,
            main_axis_layout_size,
            min_offset,
            max_offset,
        );

        let mut extra_item_offset = item_scroll_offsets[0];
        let items_retained_for_lookahead = self.items_retained_for_lookahead(
            visible_items.last().map_or(-1, |i| i.index),
            item_count,
            self.is_looking_ahead,
            |item, cross_axis| {
                item.position_at(extra_item_offset, cross_axis, main_axis_layout_size);
                extra_item_offset += item.main_axis_size_with_spacings();
            },
        );
        let extra_items_after = self.calculate_extra_items(
            |it| {
                it.position_at(extra_item_offset, 0, main_axis_layout_size);
                extra_item_offset += it.main_axis_size_with_spacings();
            },
            |item_index| {
                if item_index >= item_count {
                    return false;
                }
                // (those kept for the lookahead left out)
                if items_retained_for_lookahead.iter().any(|it| it.index == item_index) {
                    return false;
                }
                let lane = self.get_lane(item_index);
                if lane == LaneUnset || lane == LaneFullSpan {
                    current_item_indices.iter().all(|&i| i < item_index)
                } else {
                    current_item_indices[lane as usize] < item_index
                }
            },
            false,
        );

        let mut positioned_items: Vec<Rc<LazyStaggeredGridMeasuredItem>> = Vec::new();
        positioned_items.extend(extra_items_before);
        positioned_items.extend(visible_items.iter().cloned());
        positioned_items.extend(items_retained_for_lookahead);
        positioned_items.extend(extra_items_after);

        animator.on_measured(
            consumed_scroll as i32,
            layout_width,
            layout_height,
            &mut positioned_items,
            self.measured_item_provider.key_index_map(),
            &self.measured_item_provider,
            self.is_vertical,
            self.is_looking_ahead,
            lane_count,
            self.is_in_lookahead_scope,
            *first_item_offsets.iter().min().unwrap(),
            max_current + content_padding,
            &self.coroutine_scope,
            &self.platform,
        );

        if !self.is_looking_ahead {
            let disappearing_items_size = animator.min_size_to_fit_disappearing_items();
            if disappearing_items_size != IntSize::Zero {
                let old_main_axis_size = if self.is_vertical { layout_height } else { layout_width };
                layout_width = self.constraints.constrain_width(layout_width.max(disappearing_items_size.width));
                layout_height = self.constraints.constrain_height(layout_height.max(disappearing_items_size.height));
                let new_main_axis_size = if self.is_vertical { layout_height } else { layout_width };
                if new_main_axis_size != old_main_axis_size {
                    for item in &positioned_items {
                        item.update_main_axis_layout_size(new_main_axis_size);
                    }
                }
            }
        }

        // (it scrolls forward only while the last item isn't wholly in view)
        let can_scroll_forward = current_item_offsets.iter().any(|&o| o > self.main_axis_available_size)
            || current_item_indices.iter().all(|&i| i < item_count - 1);

        let (reverse_layout, content_offset, is_looking_ahead) =
            (self.reverse_layout, self.content_offset, self.is_looking_ahead);
        let invalidator = self.state.inner.placement_scope_invalidator.clone();
        let placement = Box::new(move |scope: &PlacementScope| {
            // (a motion frame of reference's placement: it scrolls — what reads these offsets may apply their changes
            // at once rather than animate them, not to chase the scroll)
            scope.with_motion_frame_of_reference_placement(|scope| {
                for item in &positioned_items {
                    item.place(scope, reverse_layout, content_offset, is_looking_ahead);
                }
            });
            // (read while placing: the state places again by invalidating it)
            invalidator.attach_to_scope();
        });
        LazyStaggeredGridMeasureResult {
            first_visible_item_indices: first_item_indices,
            first_visible_item_scroll_offsets: first_item_offsets,
            consumed_scroll,
            measure_result: self.layout(layout_width, layout_height, placement),
            scroll_back_amount,
            can_scroll_forward,
            is_vertical: self.is_vertical,
            remeasure_needed,
            slots: self.resolved_slots.clone(),
            span_provider: self.item_provider.span_provider().clone(),
            density: self.measure_scope.density(),
            total_items_count: item_count,
            visible_items_info: visible_items,
            viewport_size: IntSize::new(layout_width, layout_height),
            viewport_start_offset: min_offset,
            viewport_end_offset: max_offset,
            before_content_padding: self.before_content_padding,
            after_content_padding: self.after_content_padding,
            main_axis_item_spacing: self.main_axis_spacing,
            coroutine_scope: Some(self.coroutine_scope.clone()),
            reverse_layout: self.reverse_layout,
        }
    }

    /// Positions the measured items lane by lane in index order, those in view kept (Compose's
    /// `calculateVisibleItems`).
    fn calculate_visible_items(
        &self,
        measured_items: &mut [VecDeque<Rc<LazyStaggeredGridMeasuredItem>>],
        item_scroll_offsets: &mut [i32],
        main_axis_layout_size: i32,
        min_offset: i32,
        max_offset: i32,
    ) -> Vec<Rc<LazyStaggeredGridMeasuredItem>> {
        let mut positioned_items = Vec::with_capacity(measured_items.iter().map(|l| l.len()).sum());
        while measured_items.iter().any(|l| !l.is_empty()) {
            // (the next item to position)
            let lane_index = index_of_min_by(measured_items, |l| l.front().map_or(i32::MAX, |i| i.index));
            let item = measured_items[lane_index].pop_front().unwrap();
            if item.lane != lane_index as i32 {
                continue;
            }
            let span_range = SpanRange::new(item.lane, item.span);
            let main_axis_offset = max_in_range(item_scroll_offsets, span_range);
            let cross_axis_offset = self.resolved_slots.positions[lane_index];
            let min_edge = main_axis_offset;
            let max_edge = main_axis_offset + item.main_axis_size;
            // (positioned only when in view)
            if max_edge >= min_offset && min_edge <= max_offset {
                item.position_at(main_axis_offset, cross_axis_offset, main_axis_layout_size);
                positioned_items.push(item.clone());
            }
            for lane in span_range.lanes() {
                item_scroll_offsets[lane as usize] = main_axis_offset + item.main_axis_size_with_spacings();
            }
        }
        positioned_items
    }

    /// The items the last approach pass showed after those in view, kept in a lookahead pass (Compose's
    /// `itemsRetainedForLookahead`).
    fn items_retained_for_lookahead(
        &self,
        last_visible_item_index: i32,
        items_count: i32,
        is_looking_ahead: bool,
        mut position: impl FnMut(&LazyStaggeredGridMeasuredItem, i32),
    ) -> Vec<Rc<LazyStaggeredGridMeasuredItem>> {
        let mut list: Vec<Rc<LazyStaggeredGridMeasuredItem>> = Vec::new();
        if !is_looking_ahead {
            return list;
        }
        let Some(approach_visible_items) = &self.approach_visible_items else { return list };
        if approach_visible_items.is_empty() {
            return list;
        }
        // (the first item past those in view: the last ones may be items placed for their animation)
        let mut first_item = None;
        for i in (0..approach_visible_items.len()).rev() {
            if approach_visible_items[i].index > last_visible_item_index
                && (i == 0 || approach_visible_items[i - 1].index <= last_visible_item_index)
            {
                first_item = Some(&approach_visible_items[i]);
                break;
            }
        }
        let last_visible_item = approach_visible_items.last().unwrap();
        if let Some(first_item) = first_item {
            for i in first_item.index..=last_visible_item.index.min(items_count - 1) {
                if !list.iter().any(|it| it.index == i) {
                    let lane = approach_visible_items.iter().find(|it| it.index == i).map_or(0, |it| it.lane);
                    let span_range = self.get_span_range(i, lane);
                    let item = self.measured_item_provider.get_and_measure_span(i, span_range);
                    // (where it was across: its disappearance moves along the grid only)
                    let cross_axis_offset = self.resolved_slots.positions.get(lane as usize).copied().unwrap_or(0);
                    position(&item, cross_axis_offset);
                    list.push(item);
                }
            }
        }
        list
    }

    /// The pinned items `filter` keeps, measured and positioned by `position` (Compose's `calculateExtraItems`):
    /// back to front before those in view.
    fn calculate_extra_items(
        &self,
        mut position: impl FnMut(&LazyStaggeredGridMeasuredItem),
        filter: impl Fn(i32) -> bool,
        before_visible_bounds: bool,
    ) -> Vec<Rc<LazyStaggeredGridMeasuredItem>> {
        let mut result = Vec::new();
        let indices: Vec<i32> = if before_visible_bounds {
            self.pinned_items.iter().rev().copied().collect()
        } else {
            self.pinned_items.clone()
        };
        for index in indices {
            if filter(index) {
                let span_range = self.get_span_range(index, 0);
                let measured_item = self.measured_item_provider.get_and_measure_span(index, span_range);
                position(&measured_item);
                result.push(measured_item);
            }
        }
        result
    }

    /// Each lane's first index brought back in range of the items and the lanes they can be in (Compose's
    /// `ensureIndicesInRange`): last lanes first, so the last items stay in the last lanes.
    fn ensure_indices_in_range(&self, indices: &mut [i32], item_count: i32) {
        for i in (0..indices.len()).rev() {
            while indices[i] >= item_count
                || !self.state.inner.lane_info.borrow().assigned_to_lane(indices[i], i as i32)
            {
                indices[i] = self.find_previous_item_index(indices[i], i as i32);
            }
            if indices[i] >= 0 {
                let item_index = indices[i];
                // (a full-line item that became a lane's: back to the first lane it spanned)
                if !self.is_full_span(item_index) {
                    let lane = if self.get_lane(item_index) == LaneFullSpan {
                        let target_lane = indices.iter().position(|&it| it == item_index).unwrap();
                        for (lane, index) in indices.iter_mut().enumerate().take(i + 1).skip(target_lane + 1) {
                            if *index == item_index {
                                *index = self.find_previous_item_index(item_index, lane as i32);
                            }
                        }
                        target_lane as i32
                    } else {
                        i as i32
                    };
                    self.set_lane(item_index, lane);
                }
            }
        }
    }
}

/// Java's `Math.round(float)` (Compose's `fastRoundToInt`): half up.
pub(crate) fn fast_round_to_int(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

fn offset_by(values: &mut [i32], delta: i32) {
    for v in values {
        *v += delta;
    }
}

fn max_in_range(values: &[i32], range: SpanRange) -> i32 {
    range.lanes().map(|i| values[i as usize]).fold(i32::MIN, i32::max)
}

/// The index of the smallest value above `min_bound` (-1: none) (Compose's `indexOfMinValue`).
pub(crate) fn index_of_min_value(values: &[i32], min_bound: i32) -> i32 {
    let mut result = -1;
    let mut min = i32::MAX;
    for (i, &v) in values.iter().enumerate() {
        if v > min_bound && v < min {
            min = v;
            result = i as i32;
        }
    }
    result
}

fn index_of_min_by<T>(values: &[T], block: impl Fn(&T) -> i32) -> usize {
    let mut result = 0;
    let mut min = i32::MAX;
    for (i, v) in values.iter().enumerate() {
        let value = block(v);
        if min > value {
            min = value;
            result = i;
        }
    }
    result
}

fn index_of_max_value(values: &[i32]) -> usize {
    let mut result = 0;
    let mut max = i32::MIN;
    for (i, &v) in values.iter().enumerate() {
        if max < v {
            max = v;
            result = i;
        }
    }
    result
}
