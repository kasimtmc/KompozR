//! A lazy grid's measure (Compose's LazyGridMeasure.kt, LazyGridMeasuredItem.kt, LazyGridMeasuredLine.kt,
//! LazyGridMeasuredItemProvider.kt, LazyGridMeasuredLineProvider.kt, LazyGridMeasureResult.kt): as a list's, by
//! lines — from the first visible line and its offset, the scroll to apply, lines measured back and forth until the
//! viewport (and its content padding) is filled, scrolling back when the end leaves room; each line's items
//! measured in the cells they span, the line as long as its longest item; the pinned items kept, the lines
//! arranged when they all fit, animated, sticky headers applied.

use super::cells::{GridItemSpan, LazyGridSlots};
use super::grid::LazyGridItemProvider;
use super::layout_info::{LazyGridItemInfo, LazyGridLayoutInfo};
use crate::gestures::Orientation;
use crate::layout::{HorizontalArrangement, VerticalArrangement};
use crate::lazy::layout::{
    LazyKey, LazyLayoutItemAnimation, LazyLayoutItemAnimator, LazyLayoutKeyIndexMap, LazyLayoutMeasureScope,
    LazyLayoutMeasuredItem, LazyLayoutMeasuredItemProvider, ObservableScopeInvalidator, PlaceablesCache,
    StickyItemsPlacement, apply_sticky_items, updated_visible_items,
};
use kompoz_coroutines::CoroutineScope;
use kompoz_graphics::{Density, IntOffset, IntSize, LayoutDirection};
use kompoz_ui::layout::SlotKey;
use kompoz_ui::platform::PlatformHandle;
use kompoz_ui::{Constraints, MeasureResult, Placeable, PlacementScope};
use std::cell::Cell;
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// A measured item

const Unset: i32 = i32::MIN;

/// An item of a lazy grid as measured (Compose's LazyGridMeasuredItem): its placeables one over the other in its
/// cells, positioned in place on its line.
pub(crate) struct LazyGridMeasuredItem {
    index: i32,
    key: LazyKey,
    is_vertical: bool,
    /// Its size across: the cells it spans.
    cross_axis_size: i32,
    reverse_layout: bool,
    layout_direction: LayoutDirection,
    before_content_padding: i32,
    after_content_padding: i32,
    placeables: Vec<Placeable>,
    visual_offset: IntOffset,
    content_type: Option<SlotKey>,
    animator: Rc<LazyLayoutItemAnimator<LazyGridMeasuredItem>>,
    constraints: Constraints,
    lane: i32,
    span: i32,
    /// Its length along the grid: its longest placeable.
    pub(crate) main_axis_size: i32,
    horizontal_axis_size: i32,
    vertical_axis_size: i32,
    horizontal_axis_spacing: i32,
    vertical_axis_spacing: i32,
    main_axis_layout_size: Cell<i32>,
    min_main_axis_offset: Cell<i32>,
    max_main_axis_offset: Cell<i32>,
    offset: Cell<IntOffset>,
    row: Cell<i32>,
    column: Cell<i32>,
    /// Not moved by a scroll applied without a measure (sticking, or animated away).
    pub(crate) non_scrollable_item: Cell<bool>,
}

impl LazyGridMeasuredItem {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        index: i32,
        key: LazyKey,
        is_vertical: bool,
        cross_axis_size: i32,
        main_axis_spacing: i32,
        reverse_layout: bool,
        layout_direction: LayoutDirection,
        before_content_padding: i32,
        after_content_padding: i32,
        placeables: Vec<Placeable>,
        visual_offset: IntOffset,
        content_type: Option<SlotKey>,
        animator: Rc<LazyLayoutItemAnimator<LazyGridMeasuredItem>>,
        constraints: Constraints,
        lane: i32,
        span: i32,
    ) -> LazyGridMeasuredItem {
        let main_axis_size =
            placeables.iter().map(|p| if is_vertical { p.height() } else { p.width() }).max().unwrap_or(0).max(0);
        let (vertical_axis_spacing, vertical_axis_size, horizontal_axis_size, horizontal_axis_spacing) = if is_vertical
        {
            (main_axis_spacing, main_axis_size, cross_axis_size, 0)
        } else {
            (0, cross_axis_size, main_axis_size, main_axis_spacing)
        };
        LazyGridMeasuredItem {
            index,
            key,
            is_vertical,
            cross_axis_size,
            reverse_layout,
            layout_direction,
            before_content_padding,
            after_content_padding,
            placeables,
            visual_offset,
            content_type,
            animator,
            constraints,
            lane,
            span,
            main_axis_size,
            horizontal_axis_size,
            vertical_axis_size,
            horizontal_axis_spacing,
            vertical_axis_spacing,
            main_axis_layout_size: Cell::new(Unset),
            min_main_axis_offset: Cell::new(0),
            max_main_axis_offset: Cell::new(0),
            offset: Cell::new(IntOffset::Zero),
            row: Cell::new(LazyGridItemInfo::UnknownRow),
            column: Cell::new(LazyGridItemInfo::UnknownColumn),
            non_scrollable_item: Cell::new(false),
        }
    }

    pub(crate) fn index_value(&self) -> i32 {
        self.index
    }

    pub(crate) fn key_ref(&self) -> &LazyKey {
        &self.key
    }

    /// Its size along the grid with the spacing after it (none below 0).
    pub(crate) fn main_axis_size_with_spacings(&self) -> i32 {
        if self.is_vertical {
            self.vertical_axis_size + self.vertical_axis_spacing
        } else {
            self.horizontal_axis_size + self.horizontal_axis_spacing
        }
    }

    /// Its size (Compose's `size`).
    pub(crate) fn size(&self) -> IntSize {
        if self.is_vertical {
            IntSize::new(self.cross_axis_size, self.main_axis_size)
        } else {
            IntSize::new(self.main_axis_size, self.cross_axis_size)
        }
    }

    /// Its offset along the grid.
    pub(crate) fn offset_on_main_axis(&self) -> i32 {
        let o = self.offset.get();
        if self.is_vertical { o.y } else { o.x }
    }

    /// Its line: the row of a vertical grid, the column of a horizontal one.
    pub(crate) fn line_index(&self) -> i32 {
        if self.is_vertical { self.row.get() } else { self.column.get() }
    }

    /// Positions it at `main_axis_offset` along the grid and `cross_axis_offset` across (mirrored right to left in
    /// a vertical grid), on row `row` and column `column` (Compose's `position`).
    pub(crate) fn position_at(
        &self,
        main_axis_offset: i32,
        cross_axis_offset: i32,
        layout_width: i32,
        layout_height: i32,
        row: i32,
        column: i32,
    ) {
        self.main_axis_layout_size.set(if self.is_vertical { layout_height } else { layout_width });
        let cross_axis_layout_size = if self.is_vertical { layout_width } else { layout_height };
        let cross_axis_offset = if self.is_vertical && self.layout_direction == LayoutDirection::Rtl {
            cross_axis_layout_size - cross_axis_offset - self.cross_axis_size
        } else {
            cross_axis_offset
        };
        self.offset.set(if self.is_vertical {
            IntOffset::new(cross_axis_offset, main_axis_offset)
        } else {
            IntOffset::new(main_axis_offset, cross_axis_offset)
        });
        self.row.set(row);
        self.column.set(column);
        self.min_main_axis_offset.set(-self.before_content_padding);
        self.max_main_axis_offset.set(self.main_axis_layout_size.get() + self.after_content_padding);
    }

    /// The layout's final length along the grid (Compose's `updateMainAxisLayoutSize`): a reversed grid places
    /// from it.
    pub(crate) fn update_main_axis_layout_size(&self, main_axis_layout_size: i32) {
        self.main_axis_layout_size.set(main_axis_layout_size);
        self.max_main_axis_offset.set(main_axis_layout_size + self.after_content_padding);
    }

    /// Moves it by `delta` for a scroll applied without a measure (Compose's `applyScrollDelta`), its animations'
    /// targets with it.
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
    /// item and animated place are both out of the viewport cancelled), mirrored for a reversed grid, moved by the
    /// visual offset (the padding), through its animation's layer if it has one.
    pub(crate) fn place(&self, scope: &PlacementScope, is_looking_ahead: bool) {
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
                        // (both where it is and where it is animated out of view: the animation is cancelled)
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
            if self.reverse_layout {
                let layout_size = self.main_axis_layout_size.get();
                let size = self.placeable_main_axis_size(placeable);
                offset = self.copy_main(offset, |m| layout_size - m - size);
            }
            offset = offset + self.visual_offset;
            if !is_looking_ahead && let Some(animation) = &animation {
                animation.placement_offset.set(offset);
            }
            match (&layer, self.is_vertical) {
                (Some(layer), true) => scope.place_with_graphics_layer(placeable, offset, layer, 0.0),
                (Some(layer), false) => scope.place_relative_with_graphics_layer(placeable, offset, layer, 0.0),
                (None, true) => scope.place_with_layer_block(placeable, offset, default_layer_block(), 0.0),
                (None, false) => scope.place_relative_with_layer_block(placeable, offset, default_layer_block(), 0.0),
            }
        }
    }

    /// What the layout info tells of it.
    pub(crate) fn info(&self) -> LazyGridItemInfo {
        LazyGridItemInfo {
            index: self.index,
            key: self.key.clone(),
            offset: self.offset.get(),
            row: self.row.get(),
            column: self.column.get(),
            size: self.size(),
            content_type: self.content_type.clone(),
            span: self.span,
        }
    }
}

thread_local! {
    /// The layer block of an item without animations (Compose's `placeWithLayer(offset)`), one for all.
    static DEFAULT_LAYER_BLOCK: kompoz_ui::layout::LayerBlock = Rc::new(|_: &mut kompoz_ui::layout::GraphicsLayerScope| {});
}

fn default_layer_block() -> kompoz_ui::layout::LayerBlock {
    DEFAULT_LAYER_BLOCK.with(|b| b.clone())
}

impl LazyLayoutMeasuredItem for LazyGridMeasuredItem {
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
    /// (the main axis offset first, the cross axis one second, on no line)
    fn position(&self, horizontal_axis_offset: i32, vertical_axis_offset: i32, layout_width: i32, layout_height: i32) {
        self.position_at(
            horizontal_axis_offset,
            vertical_axis_offset,
            layout_width,
            layout_height,
            LazyGridItemInfo::UnknownRow,
            LazyGridItemInfo::UnknownColumn,
        );
    }
    fn make_non_scrollable(&self) {
        self.non_scrollable_item.set(true);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// A measured line

/// A line of a lazy grid as measured (Compose's LazyGridMeasuredLine): its items in their cells, as long as the
/// longest.
pub(crate) struct LazyGridMeasuredLine {
    pub(crate) index: i32,
    pub(crate) items: Vec<Rc<LazyGridMeasuredItem>>,
    slots: Rc<LazyGridSlots>,
    spans: Vec<GridItemSpan>,
    is_vertical: bool,
    /// Its length along the grid: its longest item.
    pub(crate) main_axis_size: i32,
    /// With the spacing after it (none below 0).
    pub(crate) main_axis_size_with_spacings: i32,
}

impl LazyGridMeasuredLine {
    fn new(
        index: i32,
        items: Vec<Rc<LazyGridMeasuredItem>>,
        slots: Rc<LazyGridSlots>,
        spans: Vec<GridItemSpan>,
        is_vertical: bool,
        main_axis_spacing: i32,
    ) -> LazyGridMeasuredLine {
        let max_main_axis = items.iter().map(|i| i.main_axis_size).max().unwrap_or(0).max(0);
        LazyGridMeasuredLine {
            index,
            items,
            slots,
            spans,
            is_vertical,
            main_axis_size: max_main_axis,
            main_axis_size_with_spacings: (max_main_axis + main_axis_spacing).max(0),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Positions its items at `offset` along the grid, each in its first cell (Compose's `position`).
    pub(crate) fn position(&self, offset: i32, layout_width: i32, layout_height: i32) -> &[Rc<LazyGridMeasuredItem>] {
        let mut used_span = 0;
        for (item_index, item) in self.items.iter().enumerate() {
            let span = self.spans[item_index].current_line_span();
            let start_slot = used_span;
            item.position_at(
                offset,
                self.slots.positions[start_slot as usize],
                layout_width,
                layout_height,
                if self.is_vertical { self.index } else { start_slot },
                if self.is_vertical { start_slot } else { self.index },
            );
            used_span += span;
        }
        &self.items
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The providers

/// The grid's items' maker: an item's index, key, content type, size across, spacing after, placeables,
/// constraints, lane and span to its measured item.
type CreateItem<'a> =
    dyn Fn(i32, LazyKey, Option<SlotKey>, i32, i32, Vec<Placeable>, Constraints, i32, i32) -> LazyGridMeasuredItem + 'a;

/// What makes a grid's items into measured items (Compose's LazyGridMeasuredItemProvider): composed and measured in
/// the constraints of their cells.
pub(crate) struct LazyGridMeasuredItemProvider<'a> {
    item_provider: Rc<dyn LazyGridItemProvider>,
    measure_scope: &'a LazyLayoutMeasureScope,
    default_main_axis_spacing: i32,
    placeables_cache: PlaceablesCache,
    create_item: Box<CreateItem<'a>>,
}

impl<'a> LazyGridMeasuredItemProvider<'a> {
    pub(crate) fn new(
        item_provider: Rc<dyn LazyGridItemProvider>,
        measure_scope: &'a LazyLayoutMeasureScope,
        default_main_axis_spacing: i32,
        create_item: impl Fn(
            i32,
            LazyKey,
            Option<SlotKey>,
            i32,
            i32,
            Vec<Placeable>,
            Constraints,
            i32,
            i32,
        ) -> LazyGridMeasuredItem
        + 'a,
    ) -> Self {
        LazyGridMeasuredItemProvider {
            item_provider,
            measure_scope,
            default_main_axis_spacing,
            placeables_cache: PlaceablesCache::default(),
            create_item: Box::new(create_item),
        }
    }

    /// Item `index` composed and measured in `constraints` (fixed across), of `lane` and `span`, with
    /// `main_axis_spacing` after it (Compose's `getAndMeasure`).
    pub(crate) fn get_and_measure_with_spacing(
        &self,
        index: i32,
        constraints: Constraints,
        lane: i32,
        span: i32,
        main_axis_spacing: i32,
    ) -> Rc<LazyGridMeasuredItem> {
        let key = self.item_provider.get_key(index);
        let content_type = self.item_provider.get_content_type(index);
        let placeables = self.placeables_cache.get_placeables(self.measure_scope, index, constraints);
        let cross_axis_size = if constraints.has_fixed_width() {
            constraints.min_width
        } else {
            assert!(constraints.has_fixed_height(), "does not have fixed height");
            constraints.min_height
        };
        Rc::new((self.create_item)(
            index,
            key,
            content_type,
            cross_axis_size,
            main_axis_spacing,
            placeables,
            constraints,
            lane,
            span,
        ))
    }

    pub(crate) fn key_index_map(&self) -> Rc<dyn LazyLayoutKeyIndexMap> {
        self.item_provider.key_index_map()
    }

    pub(crate) fn header_indices(&self) -> Vec<i32> {
        self.item_provider.header_indexes()
    }
}

impl LazyLayoutMeasuredItemProvider<LazyGridMeasuredItem> for LazyGridMeasuredItemProvider<'_> {
    fn get_and_measure(&self, index: i32, lane: i32, span: i32, constraints: Constraints) -> Rc<LazyGridMeasuredItem> {
        self.get_and_measure_with_spacing(index, constraints, lane, span, self.default_main_axis_spacing)
    }
}

/// Measures a grid's lines (Compose's LazyGridMeasuredLineProvider): a line's items found by their spans, each
/// measured in the cells it spans.
pub(crate) struct LazyGridMeasuredLineProvider<'a, 'b> {
    is_vertical: bool,
    slots: Rc<LazyGridSlots>,
    grid_items_count: i32,
    space_between_lines: i32,
    measured_item_provider: &'b LazyGridMeasuredItemProvider<'a>,
    item_provider: Rc<dyn LazyGridItemProvider>,
}

impl<'a, 'b> LazyGridMeasuredLineProvider<'a, 'b> {
    pub(crate) fn new(
        is_vertical: bool,
        slots: Rc<LazyGridSlots>,
        grid_items_count: i32,
        space_between_lines: i32,
        measured_item_provider: &'b LazyGridMeasuredItemProvider<'a>,
        item_provider: Rc<dyn LazyGridItemProvider>,
    ) -> Self {
        LazyGridMeasuredLineProvider {
            is_vertical,
            slots,
            grid_items_count,
            space_between_lines,
            measured_item_provider,
            item_provider,
        }
    }

    /// The constraints of an item from `start_slot` spanning `span` cells: fixed across, unbounded along.
    pub(crate) fn child_constraints(&self, start_slot: i32, span: i32) -> Constraints {
        child_constraints(&self.slots, self.is_vertical, start_slot, span)
    }

    /// Item `index`'s span on a line of its own.
    pub(crate) fn span_of(&self, index: i32) -> i32 {
        let content = self.item_provider.content();
        content.span_of(index, content.span_layout_provider.slots_per_line())
    }

    /// Line `line_index` composed and measured (Compose's `getAndMeasure`): the spacing after all lines but the
    /// last.
    pub(crate) fn get_and_measure(&self, line_index: i32) -> LazyGridMeasuredLine {
        let line_configuration = self.item_provider.content().get_line_configuration(line_index);
        let line_items_count = line_configuration.spans.len() as i32;
        let main_axis_spacing = if line_items_count == 0
            || line_configuration.first_item_index + line_items_count == self.grid_items_count
        {
            0
        } else {
            self.space_between_lines
        };
        let mut start_slot = 0;
        let mut items = Vec::with_capacity(line_items_count as usize);
        for (i, span) in line_configuration.spans.iter().enumerate() {
            let span = span.current_line_span();
            let constraints = self.child_constraints(start_slot, span);
            items.push(self.measured_item_provider.get_and_measure_with_spacing(
                line_configuration.first_item_index + i as i32,
                constraints,
                start_slot,
                span,
                main_axis_spacing,
            ));
            start_slot += span;
        }
        LazyGridMeasuredLine::new(
            line_index,
            items,
            self.slots.clone(),
            line_configuration.spans,
            self.is_vertical,
            main_axis_spacing,
        )
    }

    /// Composes line `line_index` to keep it around (a cache window's).
    pub(crate) fn keep_around(&self, line_index: i32) {
        self.get_and_measure(line_index);
    }
}

/// The constraints of an item of a grid of `slots` from `start_slot` spanning `span` cells.
pub(crate) fn child_constraints(slots: &LazyGridSlots, is_vertical: bool, start_slot: i32, span: i32) -> Constraints {
    let start = start_slot as usize;
    let cross_axis_size = if span == 1 {
        slots.sizes[start]
    } else {
        let end_slot = (start_slot + span - 1) as usize;
        slots.positions[end_slot] + slots.sizes[end_slot] - slots.positions[start]
    }
    .max(0);
    if is_vertical { Constraints::fixed_width(cross_axis_size) } else { Constraints::fixed_height(cross_axis_size) }
}

// ---------------------------------------------------------------------------------------------------------------
// The measure result

/// A line's items and their constraints, for prefetching it (Compose's `prefetchInfoRetriever`).
pub(crate) type PrefetchInfoRetriever = Rc<dyn Fn(i32) -> Vec<(i32, Constraints)>>;

/// What a measure result keeps of its first visible line (Compose keeps the line).
#[derive(Clone)]
pub(crate) struct FirstVisibleLine {
    pub(crate) index: i32,
    pub(crate) first_item: Option<Rc<LazyGridMeasuredItem>>,
    pub(crate) main_axis_size_with_spacings: i32,
}

/// What a lazy grid's measure gives (Compose's LazyGridMeasureResult): the scroll position and the layout info,
/// the measure result to place, what scroll it took.
#[derive(Clone)]
pub(crate) struct LazyGridMeasureResult {
    pub(crate) first_visible_line: Option<FirstVisibleLine>,
    pub(crate) first_visible_line_scroll_offset: i32,
    pub(crate) can_scroll_forward: bool,
    pub(crate) consumed_scroll: f32,
    pub(crate) measure_result: MeasureResult,
    /// A gesture's scroll back at the end, in a lookahead pass (stage 11's lookahead reads it).
    #[allow(dead_code)]
    pub(crate) scroll_back_amount: f32,
    pub(crate) remeasure_needed: bool,
    pub(crate) coroutine_scope: Option<CoroutineScope>,
    pub(crate) density: Density,
    pub(crate) slots_per_line: i32,
    pub(crate) prefetch_info_retriever: PrefetchInfoRetriever,
    pub(crate) line_index_provider: Rc<dyn Fn(i32) -> i32>,
    pub(crate) sticking_items_combined_size: i32,
    pub(crate) visible_items_info: Vec<Rc<LazyGridMeasuredItem>>,
    pub(crate) viewport_start_offset: i32,
    pub(crate) viewport_end_offset: i32,
    pub(crate) total_items_count: i32,
    pub(crate) reverse_layout: bool,
    pub(crate) orientation: Orientation,
    pub(crate) after_content_padding: i32,
    pub(crate) main_axis_item_spacing: i32,
}

impl LazyGridMeasureResult {
    /// The result before the first measure (Compose's EmptyLazyGridLayoutInfo).
    pub(crate) fn empty() -> LazyGridMeasureResult {
        LazyGridMeasureResult {
            first_visible_line: None,
            first_visible_line_scroll_offset: 0,
            can_scroll_forward: false,
            consumed_scroll: 0.0,
            measure_result: MeasureResult::new(0, 0, Vec::new(), |_| {}),
            scroll_back_amount: 0.0,
            remeasure_needed: false,
            coroutine_scope: None,
            density: Density::new(1.0, 1.0),
            slots_per_line: 0,
            prefetch_info_retriever: Rc::new(|_| Vec::new()),
            line_index_provider: Rc::new(|_| -1),
            sticking_items_combined_size: 0,
            visible_items_info: Vec::new(),
            viewport_start_offset: 0,
            viewport_end_offset: 0,
            total_items_count: 0,
            reverse_layout: false,
            orientation: Orientation::Vertical,
            after_content_padding: 0,
            main_axis_item_spacing: 0,
        }
    }

    /// The first visible line's index, if any.
    pub(crate) fn first_visible_line_index(&self) -> Option<i32> {
        self.first_visible_line.as_ref().map(|l| l.index)
    }

    /// The first visible line's first item, if any.
    pub(crate) fn first_visible_line_first_item(&self) -> Option<&Rc<LazyGridMeasuredItem>> {
        self.first_visible_line.as_ref().and_then(|l| l.first_item.as_ref())
    }

    pub(crate) fn can_scroll_backward(&self) -> bool {
        self.first_visible_line_index().unwrap_or(0) != 0 || self.first_visible_line_scroll_offset != 0
    }

    pub(crate) fn viewport_size(&self) -> IntSize {
        IntSize::new(self.measure_result.width, self.measure_result.height)
    }

    pub(crate) fn before_content_padding(&self) -> i32 {
        -self.viewport_start_offset
    }

    /// The layout info it is (Compose's LazyGridLayoutInfo, the result itself).
    pub(crate) fn layout_info(&self) -> LazyGridLayoutInfo {
        LazyGridLayoutInfo {
            visible_items_info: Rc::new(self.visible_items_info.iter().map(|i| i.info()).collect()),
            viewport_start_offset: self.viewport_start_offset,
            viewport_end_offset: self.viewport_end_offset,
            total_items_count: self.total_items_count,
            viewport_size: self.viewport_size(),
            orientation: self.orientation,
            reverse_layout: self.reverse_layout,
            before_content_padding: self.before_content_padding(),
            after_content_padding: self.after_content_padding,
            main_axis_item_spacing: self.main_axis_item_spacing,
            max_span: self.slots_per_line,
            density: self.coroutine_scope.as_ref().map(|_| self.density),
        }
    }

    /// The same result scrolled by `delta` without a measure, if the items in view stay the same (Compose's
    /// `copyWithScrollDeltaWithoutRemeasure`).
    pub(crate) fn copy_with_scroll_delta_without_remeasure(
        &self,
        delta: i32,
        update_animations: bool,
    ) -> Option<LazyGridMeasureResult> {
        let first_line_size_with_spacings = self.first_visible_line.as_ref()?.main_axis_size_with_spacings;
        if self.remeasure_needed || self.visible_items_info.is_empty() {
            return None;
        }
        // (the delta would change the first visible line)
        let new_offset = self.first_visible_line_scroll_offset - delta;
        if new_offset < 0 || new_offset >= first_line_size_with_spacings {
            return None;
        }
        let first = self.visible_items_info.first().unwrap();
        let last = self.visible_items_info.last().unwrap();
        if first.non_scrollable_item.get() || last.non_scrollable_item.get() {
            // (they need the measure)
            return None;
        }
        let can_apply = if delta < 0 {
            // forward
            let delta_to_first_item_change =
                first.offset_on_main_axis() + first.main_axis_size_with_spacings() - self.viewport_start_offset;
            let delta_to_last_item_change =
                last.offset_on_main_axis() + last.main_axis_size_with_spacings() - self.viewport_end_offset;
            delta_to_first_item_change.min(delta_to_last_item_change) > -delta
        } else {
            // backward
            let delta_to_first_item_change = self.viewport_start_offset - first.offset_on_main_axis();
            let delta_to_last_item_change = self.viewport_end_offset - last.offset_on_main_axis();
            delta_to_first_item_change.min(delta_to_last_item_change) > delta
        };
        if !can_apply {
            return None;
        }
        for item in &self.visible_items_info {
            item.apply_scroll_delta(delta, update_animations);
        }
        Some(LazyGridMeasureResult {
            first_visible_line_scroll_offset: new_offset,
            // (scrolled back: it can scroll forward now)
            can_scroll_forward: self.can_scroll_forward || delta > 0,
            consumed_scroll: delta as f32,
            ..self.clone()
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The measure

/// What `measure_lazy_grid` needs besides the providers.
pub(crate) struct MeasureArgs<'a> {
    pub(crate) items_count: i32,
    pub(crate) main_axis_available_size: i32,
    pub(crate) before_content_padding: i32,
    pub(crate) after_content_padding: i32,
    pub(crate) space_between_lines: i32,
    pub(crate) first_visible_line_index: i32,
    pub(crate) first_visible_line_scroll_offset: i32,
    pub(crate) scroll_to_be_consumed: f32,
    pub(crate) constraints: Constraints,
    pub(crate) is_vertical: bool,
    pub(crate) vertical_arrangement: Option<VerticalArrangement>,
    pub(crate) horizontal_arrangement: Option<HorizontalArrangement>,
    pub(crate) reverse_layout: bool,
    pub(crate) density: Density,
    pub(crate) item_animator: Rc<LazyLayoutItemAnimator<LazyGridMeasuredItem>>,
    pub(crate) slots_per_line: i32,
    pub(crate) pinned_items: Vec<i32>,
    pub(crate) is_in_lookahead_scope: bool,
    pub(crate) is_looking_ahead: bool,
    /// The last approach pass's layout (stage 11's lookahead: none yet).
    pub(crate) approach_layout_info: Option<LazyGridLayoutInfo>,
    pub(crate) coroutine_scope: CoroutineScope,
    pub(crate) placement_scope_invalidator: ObservableScopeInvalidator,
    pub(crate) platform: Option<PlatformHandle>,
    pub(crate) prefetch_info_retriever: PrefetchInfoRetriever,
    pub(crate) line_index_provider: Rc<dyn Fn(i32) -> i32>,
    pub(crate) sticky_items_placement: Option<&'a dyn StickyItemsPlacement>,
    /// Makes the layout of a size, its placement.
    pub(crate) layout: &'a dyn Fn(i32, i32, Box<dyn Fn(&PlacementScope)>) -> MeasureResult,
}

/// Measures a lazy grid (Compose's `measureLazyGrid`): see the module's doc.
pub(crate) fn measure_lazy_grid(
    measured_line_provider: &LazyGridMeasuredLineProvider<'_, '_>,
    measured_item_provider: &LazyGridMeasuredItemProvider<'_>,
    a: MeasureArgs<'_>,
) -> LazyGridMeasureResult {
    assert!(a.before_content_padding >= 0, "negative beforeContentPadding");
    assert!(a.after_content_padding >= 0, "negative afterContentPadding");
    let orientation = if a.is_vertical { Orientation::Vertical } else { Orientation::Horizontal };

    if a.items_count <= 0 {
        // (no items: the scroll reset, a size of zero)
        let mut layout_width = a.constraints.min_width;
        let mut layout_height = a.constraints.min_height;
        let mut positioned: Vec<Rc<LazyGridMeasuredItem>> = Vec::new();
        a.item_animator.on_measured(
            0,
            layout_width,
            layout_height,
            &mut positioned,
            measured_item_provider.key_index_map(),
            measured_item_provider,
            a.is_vertical,
            a.is_looking_ahead,
            a.slots_per_line,
            a.is_in_lookahead_scope,
            0,
            0,
            &a.coroutine_scope,
            &a.platform,
        );
        if !a.is_looking_ahead {
            let disappearing_items_size = a.item_animator.min_size_to_fit_disappearing_items();
            if disappearing_items_size != IntSize::Zero {
                layout_width = a.constraints.constrain_width(disappearing_items_size.width);
                layout_height = a.constraints.constrain_height(disappearing_items_size.height);
            }
        }
        return LazyGridMeasureResult {
            first_visible_line: None,
            first_visible_line_scroll_offset: 0,
            can_scroll_forward: false,
            consumed_scroll: 0.0,
            measure_result: (a.layout)(layout_width, layout_height, Box::new(|_| {})),
            scroll_back_amount: 0.0,
            remeasure_needed: false,
            coroutine_scope: Some(a.coroutine_scope.clone()),
            density: a.density,
            slots_per_line: a.slots_per_line,
            prefetch_info_retriever: a.prefetch_info_retriever.clone(),
            line_index_provider: a.line_index_provider.clone(),
            sticking_items_combined_size: 0,
            visible_items_info: Vec::new(),
            viewport_start_offset: -a.before_content_padding,
            viewport_end_offset: a.main_axis_available_size + a.after_content_padding,
            total_items_count: 0,
            reverse_layout: a.reverse_layout,
            orientation,
            after_content_padding: a.after_content_padding,
            main_axis_item_spacing: a.space_between_lines,
        };
    }

    let mut current_first_line_index = a.first_visible_line_index;
    let mut current_first_line_scroll_offset = a.first_visible_line_scroll_offset;

    // The scroll applied by this measure.
    let mut scroll_delta = fast_round_to_int(a.scroll_to_be_consumed);
    // (all of it applied; what can't be is found later)
    current_first_line_scroll_offset -= scroll_delta;
    // (before the first line: not that far)
    if current_first_line_index == 0 && current_first_line_scroll_offset < 0 {
        scroll_delta += current_first_line_scroll_offset;
        current_first_line_scroll_offset = 0;
    }

    let mut visible_lines: Vec<LazyGridMeasuredLine> = Vec::new();
    let min_offset = -a.before_content_padding + if a.space_between_lines < 0 { a.space_between_lines } else { 0 };
    let max_offset = a.main_axis_available_size;

    // Lines in the start padding composed too, a negative spacing neutralised (the line before composed).
    current_first_line_scroll_offset += min_offset;

    // Scrolled back, or lines in the start padding: the lines before composed.
    while current_first_line_scroll_offset < 0 && current_first_line_index > 0 {
        let previous = current_first_line_index - 1;
        let measured_line = measured_line_provider.get_and_measure(previous);
        current_first_line_scroll_offset += measured_line.main_axis_size_with_spacings;
        visible_lines.insert(0, measured_line);
        current_first_line_index = previous;
    }
    // (scrolled back past the first line: not all of the scroll taken)
    if current_first_line_scroll_offset < min_offset {
        let not_consumed_scroll_delta = min_offset - current_first_line_scroll_offset;
        current_first_line_scroll_offset = min_offset;
        scroll_delta -= not_consumed_scroll_delta;
    }
    // (the padding neutralised again)
    current_first_line_scroll_offset -= min_offset;

    let mut index = current_first_line_index;
    let max_main_axis = (max_offset + a.after_content_padding).max(0);
    let mut current_main_axis_offset = -current_first_line_scroll_offset;
    // (lines composed only to know their size, out of view in the end: a measure again disposes of them)
    let mut remeasure_needed = false;

    // The lines composed going back skipped first.
    let mut index_in_visible_lines = 0;
    while index_in_visible_lines < visible_lines.len() {
        if current_main_axis_offset >= max_main_axis {
            // (out of the bounds: not in view)
            visible_lines.remove(index_in_visible_lines);
            remeasure_needed = true;
        } else {
            index += 1;
            current_main_axis_offset += visible_lines[index_in_visible_lines].main_axis_size_with_spacings;
            index_in_visible_lines += 1;
        }
    }

    // Then forward until the viewport is filled (at least one line, even if all are out of view: a padding bigger
    // than the viewport).
    while index < a.items_count
        && (current_main_axis_offset < max_main_axis || current_main_axis_offset <= 0 || visible_lines.is_empty())
    {
        let measured_line = measured_line_provider.get_and_measure(index);
        if measured_line.is_empty() {
            break;
        }
        current_main_axis_offset += measured_line.main_axis_size_with_spacings;
        if current_main_axis_offset <= min_offset && measured_line.items.last().unwrap().index != a.items_count - 1 {
            // (out of view: the first visible line moves on)
            current_first_line_index = index + 1;
            current_first_line_scroll_offset -= measured_line.main_axis_size_with_spacings;
            remeasure_needed = true;
        } else {
            visible_lines.push(measured_line);
        }
        index += 1;
    }

    let pre_scroll_back_scroll_delta = scroll_delta;
    // The viewport not filled from the first visible line: scrolled back if there are lines before it.
    if current_main_axis_offset < max_offset {
        let to_scroll_back = max_offset - current_main_axis_offset;
        current_first_line_scroll_offset -= to_scroll_back;
        current_main_axis_offset += to_scroll_back;
        while current_first_line_scroll_offset < a.before_content_padding && current_first_line_index > 0 {
            let previous_index = current_first_line_index - 1;
            let measured_line = measured_line_provider.get_and_measure(previous_index);
            current_first_line_scroll_offset += measured_line.main_axis_size_with_spacings;
            visible_lines.insert(0, measured_line);
            current_first_line_index = previous_index;
        }
        scroll_delta += to_scroll_back;
        if current_first_line_scroll_offset < 0 {
            scroll_delta += current_first_line_scroll_offset;
            current_main_axis_offset += current_first_line_scroll_offset;
            current_first_line_scroll_offset = 0;
        }
    }

    // The scroll taken: less than asked when there weren't lines enough, more when lines resized or the data set
    // shrank.
    let rounded = fast_round_to_int(a.scroll_to_be_consumed);
    let consumed_scroll = if rounded.signum() == scroll_delta.signum() && rounded.abs() >= scroll_delta.abs() {
        scroll_delta as f32
    } else {
        a.scroll_to_be_consumed
    };
    let unconsumed_scroll = a.scroll_to_be_consumed - consumed_scroll;
    // (a gesture's scroll back at the end, in the lookahead pass)
    let scroll_back_amount =
        if a.is_looking_ahead && scroll_delta > pre_scroll_back_scroll_delta && unconsumed_scroll <= 0.0 {
            (scroll_delta - pre_scroll_back_scroll_delta) as f32 + unconsumed_scroll
        } else {
            0.0
        };

    assert!(current_first_line_scroll_offset >= 0, "negative initial offset");
    let visible_lines_scroll_offset = -current_first_line_scroll_offset;
    let mut first_line_at = if visible_lines.is_empty() { None } else { Some(0) };

    let first_item_index = visible_lines.first().and_then(|l| l.items.first()).map_or(0, |i| i.index);
    let last_item_index = visible_lines.last().and_then(|l| l.items.last()).map_or(0, |i| i.index);
    let extra_items_before =
        calculate_extra_items(&a.pinned_items, measured_item_provider, measured_line_provider, |i| {
            (0..first_item_index).contains(&i)
        });

    let lines_retained_for_lookahead = lines_retained_for_lookahead(
        last_item_index,
        a.items_count,
        measured_line_provider,
        a.is_looking_ahead,
        &visible_lines,
        a.approach_layout_info.as_ref(),
    );

    let extra_items_after =
        calculate_extra_items(&a.pinned_items, measured_item_provider, measured_line_provider, |i| {
            ((last_item_index + 1)..a.items_count).contains(&i)
                && (!a.is_looking_ahead
                    || !lines_retained_for_lookahead.iter().any(|line| line.items.iter().any(|item| item.index == i)))
        });

    // Lines wholly in the start padding (composed to fill it) aren't the first visible one.
    if a.before_content_padding > 0 || a.space_between_lines < 0 {
        let mut i = 0;
        while i < visible_lines.len() {
            let size = visible_lines[i].main_axis_size_with_spacings;
            if current_first_line_scroll_offset != 0
                && size <= current_first_line_scroll_offset
                && i != visible_lines.len() - 1
            {
                current_first_line_scroll_offset -= size;
                first_line_at = Some(i + 1);
                i += 1;
            } else {
                break;
            }
        }
    }
    let first_visible_line = first_line_at.map(|at| {
        let line = &visible_lines[at];
        FirstVisibleLine {
            index: line.index,
            first_item: line.items.first().cloned(),
            main_axis_size_with_spacings: line.main_axis_size_with_spacings,
        }
    });

    let mut layout_width =
        if a.is_vertical { a.constraints.max_width } else { a.constraints.constrain_width(current_main_axis_offset) };
    let mut layout_height =
        if a.is_vertical { a.constraints.constrain_height(current_main_axis_offset) } else { a.constraints.max_height };

    let mut lines: Vec<&LazyGridMeasuredLine> = visible_lines.iter().collect();
    lines.extend(lines_retained_for_lookahead.iter());
    let mut positioned_items = calculate_items_offsets(
        &lines,
        &extra_items_before,
        &extra_items_after,
        layout_width,
        layout_height,
        current_main_axis_offset,
        max_offset,
        visible_lines_scroll_offset,
        a.is_vertical,
        &a.vertical_arrangement,
        &a.horizontal_arrangement,
        a.reverse_layout,
        &a.density,
    );

    a.item_animator.on_measured(
        consumed_scroll as i32,
        layout_width,
        layout_height,
        &mut positioned_items,
        measured_item_provider.key_index_map(),
        measured_item_provider,
        a.is_vertical,
        a.is_looking_ahead,
        a.slots_per_line,
        a.is_in_lookahead_scope,
        current_first_line_scroll_offset,
        current_main_axis_offset,
        &a.coroutine_scope,
        &a.platform,
    );

    if !a.is_looking_ahead {
        let disappearing_items_size = a.item_animator.min_size_to_fit_disappearing_items();
        if disappearing_items_size != IntSize::Zero {
            let old_main_axis_size = if a.is_vertical { layout_height } else { layout_width };
            layout_width = a.constraints.constrain_width(layout_width.max(disappearing_items_size.width));
            layout_height = a.constraints.constrain_height(layout_height.max(disappearing_items_size.height));
            let new_main_axis_size = if a.is_vertical { layout_height } else { layout_width };
            if new_main_axis_size != old_main_axis_size {
                for item in &positioned_items {
                    item.update_main_axis_layout_size(new_main_axis_size);
                }
            }
        }
    }

    // The sticky items.
    let header_indexes = measured_item_provider.header_indices();
    let sticking_items = apply_sticky_items(
        a.sticky_items_placement,
        first_item_index,
        last_item_index,
        &mut positioned_items,
        &header_indexes,
        a.before_content_padding,
        a.after_content_padding,
        layout_width,
        layout_height,
        a.is_vertical,
        &mut |index| {
            let span = measured_line_provider.span_of(index);
            let child_constraints = measured_line_provider.child_constraints(0, span);
            measured_item_provider.get_and_measure(index, 0, span, child_constraints)
        },
    );

    let visible_items_info =
        updated_visible_items(first_item_index, last_item_index, &positioned_items, &sticking_items);
    let sticking_items_combined_size: i32 = sticking_items.iter().map(|i| i.main_axis_size).sum();
    let invalidator = a.placement_scope_invalidator.clone();
    let is_looking_ahead = a.is_looking_ahead;
    let placement = Box::new(move |scope: &PlacementScope| {
        // (Compose tags this placement as the motion frame of reference's — lookahead's approach, stage 11's)
        for item in &positioned_items {
            item.place(scope, is_looking_ahead);
        }
        for item in &sticking_items {
            item.place(scope, is_looking_ahead);
        }
        // (read while placing: the state places again by invalidating it)
        invalidator.attach_to_scope();
    });
    LazyGridMeasureResult {
        first_visible_line,
        first_visible_line_scroll_offset: current_first_line_scroll_offset,
        can_scroll_forward: last_item_index != a.items_count - 1 || current_main_axis_offset > max_offset,
        consumed_scroll,
        measure_result: (a.layout)(layout_width, layout_height, placement),
        scroll_back_amount,
        remeasure_needed,
        coroutine_scope: Some(a.coroutine_scope.clone()),
        density: a.density,
        slots_per_line: a.slots_per_line,
        prefetch_info_retriever: a.prefetch_info_retriever.clone(),
        line_index_provider: a.line_index_provider.clone(),
        sticking_items_combined_size,
        visible_items_info,
        viewport_start_offset: -a.before_content_padding,
        viewport_end_offset: a.main_axis_available_size + a.after_content_padding,
        total_items_count: a.items_count,
        reverse_layout: a.reverse_layout,
        orientation,
        after_content_padding: a.after_content_padding,
        main_axis_item_spacing: a.space_between_lines,
    }
}

/// Java's `Math.round(float)` (Compose's `fastRoundToInt`): half up.
pub(crate) fn fast_round_to_int(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

/// The pinned items `filter` keeps, each measured on a line of its own (Compose's `calculateExtraItems`).
fn calculate_extra_items(
    pinned_items: &[i32],
    measured_item_provider: &LazyGridMeasuredItemProvider<'_>,
    measured_line_provider: &LazyGridMeasuredLineProvider<'_, '_>,
    filter: impl Fn(i32) -> bool,
) -> Vec<Rc<LazyGridMeasuredItem>> {
    let mut items = Vec::new();
    for &index in pinned_items {
        if filter(index) {
            let span = measured_line_provider.span_of(index);
            let constraints = measured_line_provider.child_constraints(0, span);
            items.push(measured_item_provider.get_and_measure(index, 0, span, constraints));
        }
    }
    items
}

/// The lines the last approach pass showed after those in view, kept in a lookahead pass so their items aren't
/// disposed of too early (Compose's `linesRetainedForLookahead`; stage 11's lookahead: none without it).
fn lines_retained_for_lookahead(
    last_visible_item_index: i32,
    items_count: i32,
    measured_line_provider: &LazyGridMeasuredLineProvider<'_, '_>,
    is_looking_ahead: bool,
    visible_lines: &[LazyGridMeasuredLine],
    last_approach_layout_info: Option<&LazyGridLayoutInfo>,
) -> Vec<LazyGridMeasuredLine> {
    let mut list: Vec<LazyGridMeasuredLine> = Vec::new();
    if !is_looking_ahead {
        return list;
    }
    let Some(info) = last_approach_layout_info else { return list };
    let items = info.items();
    if items.is_empty() {
        return list;
    }
    // (the first item past those in view: the last ones may be items placed for their animation)
    let mut first_item = None;
    for i in (0..items.len()).rev() {
        if items[i].index > last_visible_item_index && (i == 0 || items[i - 1].index <= last_visible_item_index) {
            first_item = Some(&items[i]);
            break;
        }
    }
    let last_visible_item = items.last().unwrap();
    let mut line_index = visible_lines.last().map_or(0, |l| l.index + 1);
    if let Some(first_item) = first_item {
        for i in first_item.index..=last_visible_item.index.min(items_count - 1) {
            if !list.iter().any(|l| l.items.iter().any(|it| it.index == i)) {
                list.push(measured_line_provider.get_and_measure(line_index));
                line_index += 1;
            }
        }
    }
    list
}

/// Positions the lines' items (Compose's `calculateItemsOffsets`): by the arrangement when the lines don't fill the
/// viewport, else one line after the other from the scroll offset, the extra items each side.
#[allow(clippy::too_many_arguments)]
fn calculate_items_offsets(
    lines: &[&LazyGridMeasuredLine],
    items_before: &[Rc<LazyGridMeasuredItem>],
    items_after: &[Rc<LazyGridMeasuredItem>],
    layout_width: i32,
    layout_height: i32,
    final_main_axis_offset: i32,
    max_offset: i32,
    first_line_scroll_offset: i32,
    is_vertical: bool,
    vertical_arrangement: &Option<VerticalArrangement>,
    horizontal_arrangement: &Option<HorizontalArrangement>,
    reverse_layout: bool,
    density: &Density,
) -> Vec<Rc<LazyGridMeasuredItem>> {
    let main_axis_layout_size = if is_vertical { layout_height } else { layout_width };
    let has_spare_space = final_main_axis_offset < main_axis_layout_size.min(max_offset);
    if has_spare_space {
        assert!(first_line_scroll_offset == 0, "non-zero firstLineScrollOffset");
    }
    let mut positioned_items = Vec::with_capacity(lines.iter().map(|l| l.items.len()).sum());
    if has_spare_space {
        assert!(items_before.is_empty() && items_after.is_empty(), "no items");
        let lines_count = lines.len();
        let reverse_aware = |i: usize| if !reverse_layout { i } else { lines_count - i - 1 };
        let sizes: Vec<i32> = (0..lines_count).map(|i| lines[reverse_aware(i)].main_axis_size).collect();
        let mut offsets = vec![0; lines_count];
        if is_vertical {
            vertical_arrangement.as_ref().expect("null verticalArrangement").arrange(
                density,
                main_axis_layout_size,
                &sizes,
                &mut offsets,
            );
        } else {
            // (left to right: mirrored by placing relative)
            horizontal_arrangement.as_ref().expect("null horizontalArrangement").arrange(
                density,
                main_axis_layout_size,
                &sizes,
                LayoutDirection::Ltr,
                &mut offsets,
            );
        }
        let indices: Vec<usize> =
            if reverse_layout { (0..offsets.len()).rev().collect() } else { (0..offsets.len()).collect() };
        for index in indices {
            let absolute_offset = offsets[index];
            // (reversed, the offsets are in the lines' reversed order)
            let line = lines[reverse_aware(index)];
            let relative_offset = if reverse_layout {
                // (back to the scroll direction)
                main_axis_layout_size - absolute_offset - line.main_axis_size
            } else {
                absolute_offset
            };
            positioned_items.extend(line.position(relative_offset, layout_width, layout_height).iter().cloned());
        }
    } else {
        let mut current_main_axis = first_line_scroll_offset;
        for it in items_before.iter().rev() {
            current_main_axis -= it.main_axis_size_with_spacings();
            it.position(current_main_axis, 0, layout_width, layout_height);
            positioned_items.push(it.clone());
        }
        current_main_axis = first_line_scroll_offset;
        for line in lines {
            positioned_items.extend(line.position(current_main_axis, layout_width, layout_height).iter().cloned());
            current_main_axis += line.main_axis_size_with_spacings;
        }
        for it in items_after {
            it.position(current_main_axis, 0, layout_width, layout_height);
            positioned_items.push(it.clone());
            current_main_axis += it.main_axis_size_with_spacings();
        }
    }
    positioned_items
}

/// What the cache window's keep around composes (Compose's `CacheWindowLogic.keepAroundItems` of a grid): the
/// lines of the window on both sides of those in view.
pub(crate) fn keep_around_items(
    logic: &crate::lazy::layout::CacheWindowLogic,
    visible_items_list: &[Rc<LazyGridMeasuredItem>],
    measured_line_provider: &LazyGridMeasuredLineProvider<'_, '_>,
) {
    if logic.has_valid_bounds() && !visible_items_list.is_empty() {
        let first_visible_line = visible_items_list[0].line_index();
        let last_visible_line = visible_items_list.last().unwrap().line_index();
        // (also when directions change: kept lines become prefetched ahead)
        for line in logic.prefetch_window_start_line()..first_visible_line {
            measured_line_provider.keep_around(line);
        }
        for line in (last_visible_line + 1)..=logic.prefetch_window_end_line() {
            measured_line_provider.keep_around(line);
        }
    }
}
