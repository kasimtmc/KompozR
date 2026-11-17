//! A lazy list's measure (Compose's LazyListMeasure.kt, LazyListMeasuredItem.kt, LazyListMeasuredItemProvider.kt,
//! LazyListMeasureResult.kt): from the first visible item and its offset, the scroll to apply, items measured
//! back and forth until the viewport (and its content padding) is filled — scrolling back when the end leaves
//! room — the extra items kept (beyond bounds, pinned), arranged when they all fit, animated, sticky headers
//! applied; the result is the new scroll position and the layout info.

use super::layout_info::{LazyListItemInfo, LazyListLayoutInfo};
use super::list::LazyListItemProvider;
use crate::gestures::Orientation;
use crate::layout::{HorizontalArrangement, VerticalArrangement};
use crate::lazy::layout::{
    LazyKey, LazyLayoutItemAnimation, LazyLayoutItemAnimator, LazyLayoutKeyIndexMap, LazyLayoutMeasureScope,
    LazyLayoutMeasuredItem, LazyLayoutMeasuredItemProvider, PlaceablesCache, StickyItemsPlacement, apply_sticky_items,
    updated_visible_items,
};
use kompoz_coroutines::CoroutineScope;
use kompoz_graphics::{Density, IntOffset, IntSize, LayoutDirection};
use kompoz_ui::layout::SlotKey;
use kompoz_ui::platform::PlatformHandle;
use kompoz_ui::{Constraints, HorizontalAlignment, MeasureResult, Placeable, PlacementScope, VerticalAlignment};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// A measured item

const Unset: i32 = i32::MIN;

/// An item of a lazy list as measured (Compose's LazyListMeasuredItem): its placeables one after the other along
/// the list, positioned in place.
pub(crate) struct LazyListMeasuredItem {
    index: i32,
    placeables: Vec<Placeable>,
    is_vertical: bool,
    horizontal_alignment: Option<HorizontalAlignment>,
    vertical_alignment: Option<VerticalAlignment>,
    layout_direction: LayoutDirection,
    reverse_layout: bool,
    before_content_padding: i32,
    after_content_padding: i32,
    visual_offset: IntOffset,
    key: LazyKey,
    content_type: Option<SlotKey>,
    animator: Rc<LazyLayoutItemAnimator<LazyListMeasuredItem>>,
    constraints: Constraints,
    /// Its length along the list.
    pub(crate) size: i32,
    pub(crate) cross_axis_size: i32,
    horizontal_axis_size: i32,
    vertical_axis_size: i32,
    horizontal_axis_spacing: i32,
    vertical_axis_spacing: i32,
    offset: Cell<i32>,
    pub(crate) non_scrollable_item: Cell<bool>,
    main_axis_layout_size: Cell<i32>,
    min_main_axis_offset: Cell<i32>,
    max_main_axis_offset: Cell<i32>,
    /// x then y of each placeable.
    placeable_offsets: RefCell<Vec<i32>>,
}

impl LazyListMeasuredItem {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        index: i32,
        placeables: Vec<Placeable>,
        is_vertical: bool,
        horizontal_alignment: Option<HorizontalAlignment>,
        vertical_alignment: Option<VerticalAlignment>,
        layout_direction: LayoutDirection,
        reverse_layout: bool,
        before_content_padding: i32,
        after_content_padding: i32,
        spacing: i32,
        visual_offset: IntOffset,
        key: LazyKey,
        content_type: Option<SlotKey>,
        animator: Rc<LazyLayoutItemAnimator<LazyListMeasuredItem>>,
        constraints: Constraints,
    ) -> LazyListMeasuredItem {
        let mut main_axis_size = 0;
        let mut max_cross_axis = 0;
        for p in &placeables {
            main_axis_size += if is_vertical { p.height() } else { p.width() };
            max_cross_axis = max_cross_axis.max(if !is_vertical { p.height() } else { p.width() });
        }
        let (size, cross_axis_size) = (main_axis_size, max_cross_axis);
        let (vertical_axis_spacing, vertical_axis_size, horizontal_axis_size, horizontal_axis_spacing) =
            if is_vertical { (spacing, size, cross_axis_size, 0) } else { (0, cross_axis_size, size, spacing) };
        let count = placeables.len();
        LazyListMeasuredItem {
            index,
            placeables,
            is_vertical,
            horizontal_alignment,
            vertical_alignment,
            layout_direction,
            reverse_layout,
            before_content_padding,
            after_content_padding,
            visual_offset,
            key,
            content_type,
            animator,
            constraints,
            size,
            cross_axis_size,
            horizontal_axis_size,
            vertical_axis_size,
            horizontal_axis_spacing,
            vertical_axis_spacing,
            offset: Cell::new(0),
            non_scrollable_item: Cell::new(false),
            main_axis_layout_size: Cell::new(Unset),
            min_main_axis_offset: Cell::new(0),
            max_main_axis_offset: Cell::new(0),
            placeable_offsets: RefCell::new(vec![0; count * 2]),
        }
    }

    pub(crate) fn index_value(&self) -> i32 {
        self.index
    }

    pub(crate) fn key_ref(&self) -> &LazyKey {
        &self.key
    }

    /// Where it is along the list (from its content start).
    pub(crate) fn offset(&self) -> i32 {
        self.offset.get()
    }

    /// Its size along the list with the spacing after it (none below 0).
    pub(crate) fn main_axis_size_with_spacings(&self) -> i32 {
        let v = if self.is_vertical {
            self.vertical_axis_size + self.vertical_axis_spacing
        } else {
            self.horizontal_axis_size + self.horizontal_axis_spacing
        };
        v.max(0)
    }

    /// Positions it at `main_axis_offset` along the list, its placeables aligned across (Compose's `position`).
    pub(crate) fn position_at(&self, main_axis_offset: i32, layout_width: i32, layout_height: i32) {
        self.offset.set(main_axis_offset);
        self.main_axis_layout_size.set(if self.is_vertical { layout_height } else { layout_width });
        let mut main_axis_offset = main_axis_offset;
        let mut offsets = self.placeable_offsets.borrow_mut();
        for (index, placeable) in self.placeables.iter().enumerate() {
            let at = index * 2;
            if self.is_vertical {
                offsets[at] = self
                    .horizontal_alignment
                    .as_ref()
                    .expect("null horizontalAlignment when isVertical == true")
                    .align(placeable.width(), layout_width, self.layout_direction);
                offsets[at + 1] = main_axis_offset;
                main_axis_offset += placeable.height();
            } else {
                offsets[at] = main_axis_offset;
                offsets[at + 1] = self
                    .vertical_alignment
                    .as_ref()
                    .expect("null verticalAlignment when isVertical == false")
                    .align(placeable.height(), layout_height);
                main_axis_offset += placeable.width();
            }
        }
        self.min_main_axis_offset.set(-self.before_content_padding);
        self.max_main_axis_offset.set(self.main_axis_layout_size.get() + self.after_content_padding);
    }

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
        self.offset.set(self.offset.get() + delta);
        {
            let mut offsets = self.placeable_offsets.borrow_mut();
            for (index, o) in offsets.iter_mut().enumerate() {
                // (x and y pairs: the main axis' every other one)
                let odd = index & 1 != 0;
                if (self.is_vertical && odd) || (!self.is_vertical && !odd) {
                    *o += delta;
                }
            }
        }
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
    /// start and end are both out of the viewport cancelled), mirrored for a reversed list, moved by the visual
    /// offset (the padding), through its animation's layer if it has one.
    pub(crate) fn place(&self, scope: &PlacementScope, is_looking_ahead: bool) {
        assert!(self.main_axis_layout_size.get() != Unset, "position() should be called first");
        for (index, placeable) in self.placeables.iter().enumerate() {
            let min_offset = self.min_main_axis_offset.get() - self.placeable_main_axis_size(placeable);
            let max_offset = self.max_main_axis_offset.get();
            let mut offset = self.get_offset(index);
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
                        // (both where it goes and where it is out of view: the animation is cancelled)
                        let target_main = self.main_axis(target_offset);
                        let animated_main = self.main_axis(animated_offset);
                        if (target_main <= min_offset && animated_main <= min_offset)
                            || (target_main >= max_offset && animated_main >= max_offset)
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
    pub(crate) fn info(&self) -> LazyListItemInfo {
        LazyListItemInfo {
            index: self.index,
            key: self.key.clone(),
            offset: self.offset.get(),
            size: self.size,
            content_type: self.content_type.clone(),
            non_scrollable: self.non_scrollable_item.get(),
        }
    }
}

thread_local! {
    /// The layer block of an item without animations (Compose's `placeWithLayer(offset)`: DefaultLayerBlock), one
    /// for all, so placing again isn't a new layer.
    static DEFAULT_LAYER_BLOCK: kompoz_ui::layout::LayerBlock = Rc::new(|_: &mut kompoz_ui::layout::GraphicsLayerScope| {});
}

fn default_layer_block() -> kompoz_ui::layout::LayerBlock {
    DEFAULT_LAYER_BLOCK.with(|b| b.clone())
}

impl LazyLayoutMeasuredItem for LazyListMeasuredItem {
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
        0
    }
    fn span(&self) -> i32 {
        1
    }
    fn get_offset(&self, placeable_index: usize) -> IntOffset {
        if placeable_index == 0 && self.placeables.is_empty() {
            let offset = self.offset.get();
            return if self.is_vertical { IntOffset::new(0, offset) } else { IntOffset::new(offset, 0) };
        }
        let offsets = self.placeable_offsets.borrow();
        IntOffset::new(offsets[placeable_index * 2], offsets[placeable_index * 2 + 1])
    }
    fn position(&self, horizontal_axis_offset: i32, _vertical_axis_offset: i32, layout_width: i32, layout_height: i32) {
        self.position_at(horizontal_axis_offset, layout_width, layout_height);
    }
    fn make_non_scrollable(&self) {
        self.non_scrollable_item.set(true);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The measured item provider

/// What makes a list's items into measured items (Compose's LazyListMeasuredItemProvider): composed, measured
/// unbounded along the list, with the spacing after all but the last.
pub(crate) struct LazyListMeasuredItemProvider<'a> {
    pub(crate) child_constraints: Constraints,
    item_provider: Rc<dyn LazyListItemProvider>,
    measure_scope: &'a LazyLayoutMeasureScope,
    placeables_cache: PlaceablesCache,
    create_item: Box<dyn Fn(i32, LazyKey, Option<SlotKey>, Vec<Placeable>, Constraints) -> LazyListMeasuredItem + 'a>,
}

impl<'a> LazyListMeasuredItemProvider<'a> {
    pub(crate) fn new(
        constraints: Constraints,
        is_vertical: bool,
        item_provider: Rc<dyn LazyListItemProvider>,
        measure_scope: &'a LazyLayoutMeasureScope,
        create_item: impl Fn(i32, LazyKey, Option<SlotKey>, Vec<Placeable>, Constraints) -> LazyListMeasuredItem + 'a,
    ) -> Self {
        // (the main axis isn't bounded)
        let child_constraints = Constraints::new(
            0,
            if is_vertical { constraints.max_width } else { Constraints::Infinity },
            0,
            if !is_vertical { constraints.max_height } else { Constraints::Infinity },
        );
        LazyListMeasuredItemProvider {
            child_constraints,
            item_provider,
            measure_scope,
            placeables_cache: PlaceablesCache::default(),
            create_item: Box::new(create_item),
        }
    }

    /// Item `index` composed and measured (in the child constraints unless given).
    pub(crate) fn get_and_measure(&self, index: i32) -> Rc<LazyListMeasuredItem> {
        self.get_and_measure_in(index, self.child_constraints)
    }

    fn get_and_measure_in(&self, index: i32, constraints: Constraints) -> Rc<LazyListMeasuredItem> {
        let key = self.item_provider.get_key(index);
        let content_type = self.item_provider.get_content_type(index);
        let placeables = self.placeables_cache.get_placeables(self.measure_scope, index, constraints);
        Rc::new((self.create_item)(index, key, content_type, placeables, constraints))
    }

    /// Composes item `index` to keep it around (a cache window's), if there is one.
    pub(crate) fn keep_around(&self, index: i32) {
        if index >= 0 && index < self.item_provider.item_count() {
            self.measure_scope.compose(index);
        }
    }

    pub(crate) fn key_index_map(&self) -> Rc<dyn LazyLayoutKeyIndexMap> {
        self.item_provider.key_index_map()
    }

    pub(crate) fn header_indexes(&self) -> Vec<i32> {
        self.item_provider.header_indexes()
    }
}

impl LazyLayoutMeasuredItemProvider<LazyListMeasuredItem> for LazyListMeasuredItemProvider<'_> {
    fn get_and_measure(
        &self,
        index: i32,
        _lane: i32,
        _span: i32,
        constraints: Constraints,
    ) -> Rc<LazyListMeasuredItem> {
        self.get_and_measure_in(index, constraints)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The measure result

/// What a lazy list's measure gives (Compose's LazyListMeasureResult): the scroll position and the layout info,
/// the measure result to place, what scroll it took.
#[derive(Clone)]
pub(crate) struct LazyListMeasureResult {
    pub(crate) first_visible_item: Option<Rc<LazyListMeasuredItem>>,
    pub(crate) first_visible_item_scroll_offset: i32,
    pub(crate) can_scroll_forward: bool,
    pub(crate) consumed_scroll: f32,
    pub(crate) measure_result: MeasureResult,
    /// A gesture's scroll back at the end, in a lookahead pass (stage 11's lookahead reads it).
    #[allow(dead_code)]
    pub(crate) scroll_back_amount: f32,
    pub(crate) remeasure_needed: bool,
    pub(crate) coroutine_scope: Option<CoroutineScope>,
    pub(crate) density: Density,
    pub(crate) child_constraints: Constraints,
    pub(crate) sticking_items_combined_size: i32,
    pub(crate) visible_items_info: Vec<Rc<LazyListMeasuredItem>>,
    pub(crate) viewport_start_offset: i32,
    pub(crate) viewport_end_offset: i32,
    pub(crate) total_items_count: i32,
    pub(crate) reverse_layout: bool,
    pub(crate) orientation: Orientation,
    pub(crate) after_content_padding: i32,
    pub(crate) main_axis_item_spacing: i32,
}

impl LazyListMeasureResult {
    /// The result before the first measure (Compose's EmptyLazyListMeasureResult).
    pub(crate) fn empty() -> LazyListMeasureResult {
        LazyListMeasureResult {
            first_visible_item: None,
            first_visible_item_scroll_offset: 0,
            can_scroll_forward: false,
            consumed_scroll: 0.0,
            measure_result: MeasureResult::new(0, 0, Vec::new(), |_| {}),
            scroll_back_amount: 0.0,
            remeasure_needed: false,
            coroutine_scope: None,
            density: Density::new(1.0, 1.0),
            child_constraints: Constraints::default(),
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

    pub(crate) fn can_scroll_backward(&self) -> bool {
        self.first_visible_item.as_ref().map_or(0, |i| i.index) != 0 || self.first_visible_item_scroll_offset != 0
    }

    pub(crate) fn viewport_size(&self) -> IntSize {
        IntSize::new(self.measure_result.width, self.measure_result.height)
    }

    pub(crate) fn before_content_padding(&self) -> i32 {
        -self.viewport_start_offset
    }

    /// The layout info it is (Compose's LazyListLayoutInfo, the result itself).
    pub(crate) fn layout_info(&self) -> LazyListLayoutInfo {
        LazyListLayoutInfo {
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
            density: self.coroutine_scope.as_ref().map(|_| self.density),
        }
    }

    /// The same result scrolled by `delta` without a measure, if the items in view stay the same (Compose's
    /// `copyWithScrollDeltaWithoutRemeasure`).
    pub(crate) fn copy_with_scroll_delta_without_remeasure(
        &self,
        delta: i32,
        update_animations: bool,
    ) -> Option<LazyListMeasureResult> {
        let first_visible_item = self.first_visible_item.as_ref()?;
        if self.remeasure_needed || self.visible_items_info.is_empty() {
            return None;
        }
        // (the delta would change the first visible item)
        let new_offset = self.first_visible_item_scroll_offset - delta;
        if new_offset < 0 || new_offset >= first_visible_item.main_axis_size_with_spacings() {
            return None;
        }
        let first = self.visible_items_info.first().unwrap();
        let last = self.visible_items_info.last().unwrap();
        if first.non_scrollable_item.get() || last.non_scrollable_item.get() {
            // (headers need the measure)
            return None;
        }
        let can_apply = if delta < 0 {
            // forward
            let delta_to_first_item_change =
                first.offset() + first.main_axis_size_with_spacings() - self.viewport_start_offset;
            let delta_to_last_item_change =
                last.offset() + last.main_axis_size_with_spacings() - self.viewport_end_offset;
            delta_to_first_item_change.min(delta_to_last_item_change) > -delta
        } else {
            // backward
            let delta_to_first_item_change = self.viewport_start_offset - first.offset();
            let delta_to_last_item_change = self.viewport_end_offset - last.offset();
            delta_to_first_item_change.min(delta_to_last_item_change) > delta
        };
        if !can_apply {
            return None;
        }
        for item in &self.visible_items_info {
            item.apply_scroll_delta(delta, update_animations);
        }
        Some(LazyListMeasureResult {
            first_visible_item_scroll_offset: new_offset,
            // (scrolled back: it can scroll forward now)
            can_scroll_forward: self.can_scroll_forward || delta > 0,
            consumed_scroll: delta as f32,
            ..self.clone()
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The measure

/// What `measure_lazy_list` needs besides the items.
pub(crate) struct MeasureArgs<'a> {
    pub(crate) items_count: i32,
    pub(crate) main_axis_available_size: i32,
    pub(crate) before_content_padding: i32,
    pub(crate) after_content_padding: i32,
    pub(crate) space_between_items: i32,
    pub(crate) first_visible_item_index: i32,
    pub(crate) first_visible_item_scroll_offset: i32,
    pub(crate) scroll_to_be_consumed: f32,
    pub(crate) constraints: Constraints,
    pub(crate) is_vertical: bool,
    pub(crate) vertical_arrangement: Option<VerticalArrangement>,
    pub(crate) horizontal_arrangement: Option<HorizontalArrangement>,
    pub(crate) reverse_layout: bool,
    pub(crate) density: Density,
    pub(crate) item_animator: Rc<LazyLayoutItemAnimator<LazyListMeasuredItem>>,
    pub(crate) beyond_bounds_item_count: i32,
    pub(crate) pinned_items: Vec<i32>,
    pub(crate) has_lookahead_occurred: bool,
    pub(crate) is_looking_ahead: bool,
    pub(crate) coroutine_scope: CoroutineScope,
    pub(crate) placement_scope_invalidator: crate::lazy::layout::ObservableScopeInvalidator,
    pub(crate) platform: Option<PlatformHandle>,
    pub(crate) sticky_items_placement: Option<&'a dyn StickyItemsPlacement>,
    pub(crate) should_run_item_animation: bool,
    /// Makes the layout of a size, its placement.
    pub(crate) layout: &'a dyn Fn(i32, i32, Box<dyn Fn(&PlacementScope)>) -> MeasureResult,
}

/// Measures a lazy list (Compose's `measureLazyList`): see the module's doc.
pub(crate) fn measure_lazy_list(
    measured_item_provider: &LazyListMeasuredItemProvider<'_>,
    a: MeasureArgs<'_>,
) -> LazyListMeasureResult {
    assert!(a.before_content_padding >= 0, "invalid beforeContentPadding");
    assert!(a.after_content_padding >= 0, "invalid afterContentPadding");
    let orientation = if a.is_vertical { Orientation::Vertical } else { Orientation::Horizontal };

    if a.items_count <= 0 {
        // (no items: the scroll reset, a size of zero)
        let mut layout_width = a.constraints.min_width;
        let mut layout_height = a.constraints.min_height;
        let mut positioned: Vec<Rc<LazyListMeasuredItem>> = Vec::new();
        a.item_animator.on_measured(
            0,
            layout_width,
            layout_height,
            &mut positioned,
            measured_item_provider.key_index_map(),
            measured_item_provider,
            a.is_vertical,
            a.is_looking_ahead,
            1,
            a.has_lookahead_occurred,
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
        return LazyListMeasureResult {
            first_visible_item: None,
            first_visible_item_scroll_offset: 0,
            can_scroll_forward: false,
            consumed_scroll: 0.0,
            measure_result: (a.layout)(layout_width, layout_height, Box::new(|_| {})),
            scroll_back_amount: 0.0,
            remeasure_needed: false,
            coroutine_scope: Some(a.coroutine_scope.clone()),
            density: a.density,
            child_constraints: measured_item_provider.child_constraints,
            sticking_items_combined_size: 0,
            visible_items_info: Vec::new(),
            viewport_start_offset: -a.before_content_padding,
            viewport_end_offset: a.main_axis_available_size + a.after_content_padding,
            total_items_count: 0,
            reverse_layout: a.reverse_layout,
            orientation,
            after_content_padding: a.after_content_padding,
            main_axis_item_spacing: a.space_between_items,
        };
    }

    let mut current_first_item_index = a.first_visible_item_index;
    let mut current_first_item_scroll_offset = a.first_visible_item_scroll_offset;
    if current_first_item_index >= a.items_count {
        // (fewer items than the index scrolled to)
        current_first_item_index = a.items_count - 1;
        current_first_item_scroll_offset = 0;
    }

    // The scroll applied by this measure.
    let mut scroll_delta = fast_round_to_int(a.scroll_to_be_consumed);
    // (all of it applied; what can't be is found later)
    current_first_item_scroll_offset -= scroll_delta;
    // (before the first item: not that far)
    if current_first_item_index == 0 && current_first_item_scroll_offset < 0 {
        scroll_delta += current_first_item_scroll_offset;
        current_first_item_scroll_offset = 0;
    }

    let mut visible_items: Vec<Rc<LazyListMeasuredItem>> = Vec::new();
    let min_offset = -a.before_content_padding + if a.space_between_items < 0 { a.space_between_items } else { 0 };
    let max_offset = a.main_axis_available_size;

    // Items in the start padding composed too, a negative spacing neutralised (the item before composed).
    current_first_item_scroll_offset += min_offset;
    let mut max_cross_axis = 0;
    // (items composed only to know their size, out of view in the end: a measure again disposes of them)
    let mut remeasure_needed = false;

    // Scrolled back, or items in the start padding: the items before composed.
    while current_first_item_scroll_offset < 0 && current_first_item_index > 0 {
        let previous = current_first_item_index - 1;
        let measured_item = measured_item_provider.get_and_measure(previous);
        max_cross_axis = max_cross_axis.max(measured_item.cross_axis_size);
        current_first_item_scroll_offset += measured_item.main_axis_size_with_spacings();
        visible_items.insert(0, measured_item);
        current_first_item_index = previous;
    }
    // (scrolled back past the first item: not all of the scroll taken)
    if current_first_item_scroll_offset < min_offset {
        let not_consumed_scroll_delta = min_offset - current_first_item_scroll_offset;
        current_first_item_scroll_offset = min_offset;
        scroll_delta -= not_consumed_scroll_delta;
    }
    // (the padding neutralised again)
    current_first_item_scroll_offset -= min_offset;

    let mut index = current_first_item_index;
    let max_main_axis = (max_offset + a.after_content_padding).max(0);
    let mut current_main_axis_offset = -current_first_item_scroll_offset;

    // The items composed going back skipped first.
    let mut index_in_visible_items = 0;
    while index_in_visible_items < visible_items.len() {
        if current_main_axis_offset >= max_main_axis {
            // (out of the bounds: not in view)
            visible_items.remove(index_in_visible_items);
            remeasure_needed = true;
        } else {
            index += 1;
            current_main_axis_offset += visible_items[index_in_visible_items].main_axis_size_with_spacings();
            index_in_visible_items += 1;
        }
    }

    // Then forward until the viewport is filled (at least one item, even if all are out of view: a padding bigger
    // than the viewport).
    while index < a.items_count
        && (current_main_axis_offset < max_main_axis || current_main_axis_offset <= 0 || visible_items.is_empty())
    {
        let measured_item = measured_item_provider.get_and_measure(index);
        current_main_axis_offset += measured_item.main_axis_size_with_spacings();
        if current_main_axis_offset <= min_offset && index != a.items_count - 1 {
            // (out of view: the first visible item moves on)
            current_first_item_index = index + 1;
            current_first_item_scroll_offset -= measured_item.main_axis_size_with_spacings();
            remeasure_needed = true;
        } else {
            max_cross_axis = max_cross_axis.max(measured_item.cross_axis_size);
            visible_items.push(measured_item);
        }
        index += 1;
    }

    let pre_scroll_back_scroll_delta = scroll_delta;
    // The viewport not filled from the first visible item: scrolled back if there are items before it.
    if current_main_axis_offset < max_offset {
        let to_scroll_back = max_offset - current_main_axis_offset;
        current_first_item_scroll_offset -= to_scroll_back;
        current_main_axis_offset += to_scroll_back;
        while current_first_item_scroll_offset < a.before_content_padding && current_first_item_index > 0 {
            let previous_index = current_first_item_index - 1;
            let measured_item = measured_item_provider.get_and_measure(previous_index);
            max_cross_axis = max_cross_axis.max(measured_item.cross_axis_size);
            current_first_item_scroll_offset += measured_item.main_axis_size_with_spacings();
            visible_items.insert(0, measured_item);
            current_first_item_index = previous_index;
        }
        scroll_delta += to_scroll_back;
        if current_first_item_scroll_offset < 0 {
            scroll_delta += current_first_item_scroll_offset;
            current_main_axis_offset += current_first_item_scroll_offset;
            current_first_item_scroll_offset = 0;
        }
    }

    // The scroll taken: less than asked when there weren't items enough, more when items resized or the data set
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

    assert!(current_first_item_scroll_offset >= 0, "negative currentFirstItemScrollOffset");
    let visible_items_scroll_offset = -current_first_item_scroll_offset;
    let mut first_item = visible_items[0].clone();

    // Items wholly in the start padding (composed to fill it) aren't the first visible one.
    if a.before_content_padding > 0 || a.space_between_items < 0 {
        let mut i = 0;
        while i < visible_items.len() {
            let size = visible_items[i].main_axis_size_with_spacings();
            if current_first_item_scroll_offset != 0
                && size <= current_first_item_scroll_offset
                && i != visible_items.len() - 1
            {
                current_first_item_scroll_offset -= size;
                first_item = visible_items[i + 1].clone();
                i += 1;
            } else {
                break;
            }
        }
    }

    // The extra items before and after.
    let extra_items_before = create_items_before_list(
        current_first_item_index,
        measured_item_provider,
        a.beyond_bounds_item_count,
        &a.pinned_items,
    );
    for i in &extra_items_before {
        max_cross_axis = max_cross_axis.max(i.cross_axis_size);
    }
    let extra_items_after = create_items_after_list(
        &visible_items,
        measured_item_provider,
        a.items_count,
        a.beyond_bounds_item_count,
        &a.pinned_items,
    );
    for i in &extra_items_after {
        max_cross_axis = max_cross_axis.max(i.cross_axis_size);
    }

    let no_extra_items =
        Rc::ptr_eq(&first_item, &visible_items[0]) && extra_items_before.is_empty() && extra_items_after.is_empty();

    let mut layout_width =
        a.constraints.constrain_width(if a.is_vertical { max_cross_axis } else { current_main_axis_offset });
    let mut layout_height =
        a.constraints.constrain_height(if a.is_vertical { current_main_axis_offset } else { max_cross_axis });

    let mut positioned_items = calculate_items_offsets(
        &visible_items,
        &extra_items_before,
        &extra_items_after,
        layout_width,
        layout_height,
        current_main_axis_offset,
        max_offset,
        visible_items_scroll_offset,
        a.is_vertical,
        &a.vertical_arrangement,
        &a.horizontal_arrangement,
        a.reverse_layout,
        &a.density,
    );

    // (the placement animation skipped while animating to an item: Compose's isSkipItemPlacementAnimationFix)
    if a.should_run_item_animation {
        a.item_animator.on_measured(
            consumed_scroll as i32,
            layout_width,
            layout_height,
            &mut positioned_items,
            measured_item_provider.key_index_map(),
            measured_item_provider,
            a.is_vertical,
            a.is_looking_ahead,
            1,
            a.has_lookahead_occurred,
            current_first_item_scroll_offset,
            current_main_axis_offset,
            &a.coroutine_scope,
            &a.platform,
        );
    }

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
    let header_indexes = measured_item_provider.header_indexes();
    let sticking_items = apply_sticky_items(
        a.sticky_items_placement,
        visible_items.first().map_or(0, |i| i.index),
        visible_items.last().map_or(0, |i| i.index),
        &mut positioned_items,
        &header_indexes,
        a.before_content_padding,
        a.after_content_padding,
        layout_width,
        layout_height,
        a.is_vertical,
        &mut |index| measured_item_provider.get_and_measure(index),
    );

    let first_visible_index =
        if no_extra_items { positioned_items.first().map(|i| i.index) } else { visible_items.first().map(|i| i.index) };
    let last_visible_index =
        if no_extra_items { positioned_items.last().map(|i| i.index) } else { visible_items.last().map(|i| i.index) };

    let visible_items_info = updated_visible_items(
        first_visible_index.unwrap_or(0),
        last_visible_index.unwrap_or(0),
        &positioned_items,
        &sticking_items,
    );
    let sticking_items_combined_size: i32 = sticking_items.iter().map(|i| i.size).sum();
    let invalidator = a.placement_scope_invalidator.clone();
    let is_looking_ahead = a.is_looking_ahead;
    let placement = Box::new(move |scope: &PlacementScope| {
        // (Compose tags this placement as the motion frame of reference's — lookahead's approach, stage 11's)
        for item in &positioned_items {
            item.place(scope, is_looking_ahead);
        }
        // (the sticking items after the others)
        for item in &sticking_items {
            item.place(scope, is_looking_ahead);
        }
        // (read while placing: the state places again by invalidating it)
        invalidator.attach_to_scope();
    });
    LazyListMeasureResult {
        first_visible_item: Some(first_item),
        first_visible_item_scroll_offset: current_first_item_scroll_offset,
        can_scroll_forward: index < a.items_count || current_main_axis_offset > max_offset,
        consumed_scroll,
        measure_result: (a.layout)(layout_width, layout_height, placement),
        scroll_back_amount,
        remeasure_needed,
        coroutine_scope: Some(a.coroutine_scope.clone()),
        density: a.density,
        child_constraints: measured_item_provider.child_constraints,
        sticking_items_combined_size,
        visible_items_info,
        viewport_start_offset: -a.before_content_padding,
        viewport_end_offset: max_offset + a.after_content_padding,
        total_items_count: a.items_count,
        reverse_layout: a.reverse_layout,
        orientation,
        after_content_padding: a.after_content_padding,
        main_axis_item_spacing: a.space_between_items,
    }
}

/// Java's `Math.round(float)` (Compose's `fastRoundToInt`): half up.
pub(crate) fn fast_round_to_int(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

/// The extra items after those in view: the beyond-bounds count, then the pinned ones past it (Compose's
/// `createItemsAfterList`).
fn create_items_after_list(
    visible_items: &[Rc<LazyListMeasuredItem>],
    provider: &LazyListMeasuredItemProvider<'_>,
    items_count: i32,
    beyond_bounds_item_count: i32,
    pinned_items: &[i32],
) -> Vec<Rc<LazyListMeasuredItem>> {
    let mut list = Vec::new();
    let last = visible_items.last().unwrap().index;
    let mut end = (last + beyond_bounds_item_count).min(items_count - 1);
    for i in last + 1..=end {
        list.push(provider.get_and_measure(i));
    }
    // (in increasing order)
    if let Some(l) = list.last() {
        let l: &Rc<LazyListMeasuredItem> = l;
        if l.index > end {
            end = l.index;
        }
    }
    for &index in pinned_items {
        if index > end {
            list.push(provider.get_and_measure(index));
        }
    }
    list
}

/// The extra items before those in view (Compose's `createItemsBeforeList`).
fn create_items_before_list(
    current_first_item_index: i32,
    provider: &LazyListMeasuredItemProvider<'_>,
    beyond_bounds_item_count: i32,
    pinned_items: &[i32],
) -> Vec<Rc<LazyListMeasuredItem>> {
    let mut list = Vec::new();
    let start = (current_first_item_index - beyond_bounds_item_count).max(0);
    for i in (start..current_first_item_index).rev() {
        list.push(provider.get_and_measure(i));
    }
    for &index in pinned_items.iter().rev() {
        if index < start {
            list.push(provider.get_and_measure(index));
        }
    }
    list
}

/// Positions the items (Compose's `calculateItemsOffsets`): by the arrangement when they don't fill the viewport,
/// else one after the other from the scroll offset, the extra items each side.
#[allow(clippy::too_many_arguments)]
fn calculate_items_offsets(
    items: &[Rc<LazyListMeasuredItem>],
    extra_items_before: &[Rc<LazyListMeasuredItem>],
    extra_items_after: &[Rc<LazyListMeasuredItem>],
    layout_width: i32,
    layout_height: i32,
    final_main_axis_offset: i32,
    max_offset: i32,
    items_scroll_offset: i32,
    is_vertical: bool,
    vertical_arrangement: &Option<VerticalArrangement>,
    horizontal_arrangement: &Option<HorizontalArrangement>,
    reverse_layout: bool,
    density: &Density,
) -> Vec<Rc<LazyListMeasuredItem>> {
    let main_axis_layout_size = if is_vertical { layout_height } else { layout_width };
    let has_spare_space = final_main_axis_offset < main_axis_layout_size.min(max_offset);
    if has_spare_space {
        assert!(items_scroll_offset == 0, "non-zero itemsScrollOffset");
    }
    let mut positioned_items = Vec::with_capacity(items.len() + extra_items_before.len() + extra_items_after.len());
    if has_spare_space {
        assert!(extra_items_before.is_empty() && extra_items_after.is_empty(), "no extra items");
        let items_count = items.len();
        let reverse_aware = |i: usize| if !reverse_layout { i } else { items_count - i - 1 };
        let sizes: Vec<i32> = (0..items_count).map(|i| items[reverse_aware(i)].size).collect();
        let mut offsets = vec![0; items_count];
        if is_vertical {
            vertical_arrangement.as_ref().expect("null verticalArrangement when isVertical == true").arrange(
                density,
                main_axis_layout_size,
                &sizes,
                &mut offsets,
            );
        } else {
            // (left to right: mirrored by placing relative)
            horizontal_arrangement.as_ref().expect("null horizontalArrangement when isVertical == false").arrange(
                density,
                main_axis_layout_size,
                &sizes,
                LayoutDirection::Ltr,
                &mut offsets,
            );
        }
        let indices: Vec<usize> =
            if !reverse_layout { (0..offsets.len()).collect() } else { (0..offsets.len()).rev().collect() };
        for index in indices {
            let absolute_offset = offsets[index];
            // (reversed, the offsets are in the items' reversed order)
            let item = &items[reverse_aware(index)];
            let relative_offset = if reverse_layout {
                // (back to the scroll direction)
                main_axis_layout_size - absolute_offset - item.size
            } else {
                absolute_offset
            };
            item.position_at(relative_offset, layout_width, layout_height);
            positioned_items.push(item.clone());
        }
    } else {
        let mut current_main_axis = items_scroll_offset;
        for it in extra_items_before {
            current_main_axis -= it.main_axis_size_with_spacings();
            it.position_at(current_main_axis, layout_width, layout_height);
            positioned_items.push(it.clone());
        }
        current_main_axis = items_scroll_offset;
        for it in items {
            it.position_at(current_main_axis, layout_width, layout_height);
            positioned_items.push(it.clone());
            current_main_axis += it.main_axis_size_with_spacings();
        }
        for it in extra_items_after {
            it.position_at(current_main_axis, layout_width, layout_height);
            positioned_items.push(it.clone());
            current_main_axis += it.main_axis_size_with_spacings();
        }
    }
    positioned_items
}

/// What the cache window's keep around composes (Compose's `CacheWindowLogic.keepAroundItems`): the items of the
/// window on both sides of those in view.
pub(crate) fn keep_around_items(
    logic: &crate::lazy::layout::CacheWindowLogic,
    visible_items_list: &[Rc<LazyListMeasuredItem>],
    measured_item_provider: &LazyListMeasuredItemProvider<'_>,
) {
    if logic.has_valid_bounds() && !visible_items_list.is_empty() {
        let first_visible_item_index = visible_items_list[0].index;
        let last_visible_item_index = visible_items_list.last().unwrap().index;
        // (also when directions change: kept items become prefetched ahead)
        for item in logic.prefetch_window_start_line()..first_visible_item_index {
            measured_item_provider.keep_around(item);
        }
        for item in (last_visible_item_index + 1)..=logic.prefetch_window_end_line() {
            measured_item_provider.keep_around(item);
        }
    }
}
