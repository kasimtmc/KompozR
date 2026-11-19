//! A pager's measure (Compose's PagerMeasure.kt, PagerMeasurePolicy.kt, MeasuredPage.kt, PagerMeasureResult.kt):
//! from the current page and how far it is from where it snaps, the first visible page found; pages (all of the
//! page size) measured back and forth until the viewport is filled, scrolling back when the end leaves room; the
//! pages beyond the viewport and the pinned ones kept; arranged when they all fit; the new current page the one
//! closest to its snap position, its offset from there a fraction of a page.

use super::layout_info::{PageInfo, PageSize, PagerLayoutInfo};
use super::state::PagerState;
use crate::clip_scrollable_container::check_scrollable_container_constraints;
use crate::gestures::Orientation;
use crate::gestures::snapping::{SnapPosition, calculate_distance_to_desired_snap_position};
use crate::layout::{Arrangement, Horizontal, PaddingValues, Vertical};
use crate::lazy::layout::{
    LazyKey, LazyLayoutItemProvider, LazyLayoutMeasurePolicy, LazyLayoutMeasureScope, ObservableScopeInvalidator,
    calculate_lazy_layout_pinned_indices,
};
use kompoz_coroutines::CoroutineScope;
use kompoz_graphics::{Density, Dp, IntOffset, IntSize, LayoutDirection};
use kompoz_runtime::without_read_observation;
use kompoz_ui::{Constraints, HorizontalAlignment, MeasureResult, Placeable, PlacementScope, VerticalAlignment};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// The least and most a current page's offset fraction is (Compose's `MinPageOffset`, `MaxPageOffset`).
pub(crate) const MinPageOffset: f32 = -0.5;
pub(crate) const MaxPageOffset: f32 = 0.5;

// ---------------------------------------------------------------------------------------------------------------
// A measured page

const Unset: i32 = i32::MIN;

/// A page as measured (Compose's MeasuredPage): its placeables one after the other along the pager, aligned across.
pub(crate) struct MeasuredPage {
    pub(crate) index: i32,
    pub(crate) size: i32,
    placeables: Vec<Placeable>,
    visual_offset: IntOffset,
    pub(crate) key: LazyKey,
    is_vertical: bool,
    horizontal_alignment: Option<HorizontalAlignment>,
    vertical_alignment: Option<VerticalAlignment>,
    layout_direction: LayoutDirection,
    reverse_layout: bool,
    pub(crate) cross_axis_size: i32,
    /// x then y of each placeable.
    placeable_offsets: RefCell<Vec<i32>>,
    offset: Cell<i32>,
    main_axis_layout_size: Cell<i32>,
}

impl MeasuredPage {
    #[allow(clippy::too_many_arguments)]
    fn new(
        index: i32,
        size: i32,
        placeables: Vec<Placeable>,
        visual_offset: IntOffset,
        key: LazyKey,
        orientation: Orientation,
        horizontal_alignment: Option<HorizontalAlignment>,
        vertical_alignment: Option<VerticalAlignment>,
        layout_direction: LayoutDirection,
        reverse_layout: bool,
    ) -> MeasuredPage {
        let is_vertical = orientation == Orientation::Vertical;
        let cross_axis_size =
            placeables.iter().map(|p| if !is_vertical { p.height() } else { p.width() }).max().unwrap_or(0).max(0);
        let count = placeables.len();
        MeasuredPage {
            index,
            size,
            placeables,
            visual_offset,
            key,
            is_vertical,
            horizontal_alignment,
            vertical_alignment,
            layout_direction,
            reverse_layout,
            cross_axis_size,
            placeable_offsets: RefCell::new(vec![0; count * 2]),
            offset: Cell::new(0),
            main_axis_layout_size: Cell::new(Unset),
        }
    }

    /// Where it is along the pager (Compose's `offset`).
    pub(crate) fn offset(&self) -> i32 {
        self.offset.get()
    }

    /// Positions it at `offset` along the pager, its placeables one after the other, aligned across (Compose's
    /// `position`).
    pub(crate) fn position(&self, offset: i32, layout_width: i32, layout_height: i32) {
        self.offset.set(offset);
        self.main_axis_layout_size.set(if self.is_vertical { layout_height } else { layout_width });
        let mut main_axis_offset = offset;
        let mut offsets = self.placeable_offsets.borrow_mut();
        for (index, placeable) in self.placeables.iter().enumerate() {
            let at = index * 2;
            if self.is_vertical {
                offsets[at] = self.horizontal_alignment.as_ref().expect("null horizontalAlignment").align(
                    placeable.width(),
                    layout_width,
                    self.layout_direction,
                );
                offsets[at + 1] = main_axis_offset;
                main_axis_offset += placeable.height();
            } else {
                offsets[at] = main_axis_offset;
                offsets[at + 1] = self
                    .vertical_alignment
                    .as_ref()
                    .expect("null verticalAlignment")
                    .align(placeable.height(), layout_height);
                main_axis_offset += placeable.width();
            }
        }
    }

    /// Places it (Compose's `place`): mirrored for a reversed pager, moved by the visual offset (the padding).
    pub(crate) fn place(&self, scope: &PlacementScope) {
        assert!(self.main_axis_layout_size.get() != Unset, "position() should be called first");
        let offsets = self.placeable_offsets.borrow();
        for (index, placeable) in self.placeables.iter().enumerate() {
            let mut offset = IntOffset::new(offsets[index * 2], offsets[index * 2 + 1]);
            if self.reverse_layout {
                let layout_size = self.main_axis_layout_size.get();
                offset = if self.is_vertical {
                    IntOffset::new(offset.x, layout_size - offset.y - placeable.height())
                } else {
                    IntOffset::new(layout_size - offset.x - placeable.width(), offset.y)
                };
            }
            offset = offset + self.visual_offset;
            if self.is_vertical {
                scope.place_with_layer_block(placeable, offset, default_layer_block(), 0.0);
            } else {
                scope.place_relative_with_layer_block(placeable, offset, default_layer_block(), 0.0);
            }
        }
    }

    /// Moves it by `delta` for a scroll applied without a measure (Compose's `applyScrollDelta`).
    pub(crate) fn apply_scroll_delta(&self, delta: i32) {
        self.offset.set(self.offset.get() + delta);
        for (index, o) in self.placeable_offsets.borrow_mut().iter_mut().enumerate() {
            // (x and y pairs: the main axis' every other one)
            if (self.is_vertical && index % 2 == 1) || (!self.is_vertical && index % 2 == 0) {
                *o += delta;
            }
        }
    }

    pub(crate) fn info(&self) -> PageInfo {
        PageInfo { index: self.index, key: self.key.clone(), offset: self.offset.get() }
    }
}

thread_local! {
    /// The layer block of a page (Compose's `placeWithLayer(offset)`), one for all.
    static DEFAULT_LAYER_BLOCK: kompoz_ui::layout::LayerBlock = Rc::new(|_: &mut kompoz_ui::layout::GraphicsLayerScope| {});
}

fn default_layer_block() -> kompoz_ui::layout::LayerBlock {
    DEFAULT_LAYER_BLOCK.with(|b| b.clone())
}

// ---------------------------------------------------------------------------------------------------------------
// The measure result

/// What a pager's measure gives (Compose's PagerMeasureResult): the pages in view, the first visible page and its
/// offset, the current page and its offset fraction, the layout info.
#[derive(Clone)]
pub(crate) struct PagerMeasureResult {
    pub(crate) visible_pages_info: Vec<Rc<MeasuredPage>>,
    pub(crate) page_size: i32,
    pub(crate) page_spacing: i32,
    pub(crate) after_content_padding: i32,
    pub(crate) orientation: Orientation,
    pub(crate) viewport_start_offset: i32,
    pub(crate) viewport_end_offset: i32,
    pub(crate) reverse_layout: bool,
    pub(crate) beyond_viewport_page_count: i32,
    pub(crate) first_visible_page: Option<Rc<MeasuredPage>>,
    pub(crate) current_page: Option<Rc<MeasuredPage>>,
    pub(crate) current_page_offset_fraction: f32,
    pub(crate) first_visible_page_scroll_offset: i32,
    pub(crate) can_scroll_forward: bool,
    pub(crate) snap_position: SnapPosition,
    pub(crate) measure_result: MeasureResult,
    pub(crate) remeasure_needed: bool,
    pub(crate) extra_pages_before: Vec<Rc<MeasuredPage>>,
    pub(crate) extra_pages_after: Vec<Rc<MeasuredPage>>,
    pub(crate) coroutine_scope: Option<CoroutineScope>,
    pub(crate) density: Density,
    pub(crate) child_constraints: Constraints,
}

impl PagerMeasureResult {
    /// The result before the first measure (Compose's EmptyLayoutInfo).
    pub(crate) fn empty() -> PagerMeasureResult {
        PagerMeasureResult {
            visible_pages_info: Vec::new(),
            page_size: 0,
            page_spacing: 0,
            after_content_padding: 0,
            orientation: Orientation::Horizontal,
            viewport_start_offset: 0,
            viewport_end_offset: 0,
            reverse_layout: false,
            beyond_viewport_page_count: 0,
            first_visible_page: None,
            current_page: None,
            current_page_offset_fraction: 0.0,
            first_visible_page_scroll_offset: 0,
            can_scroll_forward: false,
            snap_position: SnapPosition::Start,
            measure_result: MeasureResult::new(0, 0, Vec::new(), |_| {}),
            remeasure_needed: false,
            extra_pages_before: Vec::new(),
            extra_pages_after: Vec::new(),
            coroutine_scope: None,
            density: Density::new(1.0, 1.0),
            child_constraints: Constraints::default(),
        }
    }

    pub(crate) fn viewport_size(&self) -> IntSize {
        IntSize::new(self.measure_result.width, self.measure_result.height)
    }

    pub(crate) fn before_content_padding(&self) -> i32 {
        -self.viewport_start_offset
    }

    pub(crate) fn can_scroll_backward(&self) -> bool {
        self.first_visible_page.as_ref().map_or(0, |p| p.index) != 0 || self.first_visible_page_scroll_offset != 0
    }

    /// The layout info it is (Compose's PagerLayoutInfo, the result itself).
    pub(crate) fn layout_info(&self) -> PagerLayoutInfo {
        PagerLayoutInfo {
            visible_pages_info: Rc::new(self.visible_pages_info.iter().map(|p| p.info()).collect()),
            page_size: self.page_size,
            page_spacing: self.page_spacing,
            viewport_start_offset: self.viewport_start_offset,
            viewport_end_offset: self.viewport_end_offset,
            before_content_padding: self.before_content_padding(),
            after_content_padding: self.after_content_padding,
            viewport_size: self.viewport_size(),
            orientation: self.orientation,
            reverse_layout: self.reverse_layout,
            beyond_viewport_page_count: self.beyond_viewport_page_count,
            snap_position: self.snap_position.clone(),
        }
    }

    /// The same result scrolled by `delta` without a measure, if the first visible and the current page stay
    /// (Compose's `copyWithScrollDeltaWithoutRemeasure`).
    pub(crate) fn copy_with_scroll_delta_without_remeasure(&self, delta: i32) -> Option<PagerMeasureResult> {
        let page_size_with_spacing = self.page_size + self.page_spacing;
        let new_offset = self.first_visible_page_scroll_offset - delta;
        if self.remeasure_needed
            || self.visible_pages_info.is_empty()
            || self.first_visible_page.is_none()
            || new_offset < 0
            || new_offset >= page_size_with_spacing
        {
            return None;
        }
        let delta_fraction =
            if page_size_with_spacing != 0 { delta as f32 / page_size_with_spacing as f32 } else { 0.0 };
        let new_current_page_offset_fraction = self.current_page_offset_fraction - delta_fraction;
        // (the current page would change)
        if self.current_page.is_none()
            || new_current_page_offset_fraction >= MaxPageOffset
            || new_current_page_offset_fraction <= MinPageOffset
        {
            return None;
        }
        let first = self.visible_pages_info.first().unwrap();
        let last = self.visible_pages_info.last().unwrap();
        let can_apply = if delta < 0 {
            // forward
            let delta_to_first_item_change = first.offset() + page_size_with_spacing - self.viewport_start_offset;
            let delta_to_last_item_change = last.offset() + page_size_with_spacing - self.viewport_end_offset;
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
        for page in self.visible_pages_info.iter().chain(&self.extra_pages_before).chain(&self.extra_pages_after) {
            page.apply_scroll_delta(delta);
        }
        Some(PagerMeasureResult {
            current_page_offset_fraction: new_current_page_offset_fraction,
            first_visible_page_scroll_offset: new_offset,
            // (scrolled back: it can scroll forward now)
            can_scroll_forward: self.can_scroll_forward || delta > 0,
            ..self.clone()
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The measure

/// What `measure_pager` needs besides the scope.
pub(crate) struct PagerMeasureArgs<'a> {
    pub(crate) page_count: i32,
    pub(crate) pager_item_provider: Rc<dyn LazyLayoutItemProvider>,
    pub(crate) main_axis_available_size: i32,
    pub(crate) before_content_padding: i32,
    pub(crate) after_content_padding: i32,
    pub(crate) space_between_pages: i32,
    pub(crate) current_page: i32,
    pub(crate) current_page_offset: i32,
    pub(crate) constraints: Constraints,
    pub(crate) orientation: Orientation,
    pub(crate) vertical_alignment: Option<VerticalAlignment>,
    pub(crate) horizontal_alignment: Option<HorizontalAlignment>,
    pub(crate) reverse_layout: bool,
    pub(crate) visual_page_offset: IntOffset,
    pub(crate) page_available_size: i32,
    pub(crate) beyond_viewport_page_count: i32,
    pub(crate) pinned_pages: Vec<i32>,
    pub(crate) snap_position: SnapPosition,
    pub(crate) placement_scope_invalidator: ObservableScopeInvalidator,
    pub(crate) coroutine_scope: CoroutineScope,
    pub(crate) density: Density,
    pub(crate) layout: &'a dyn Fn(i32, i32, Box<dyn Fn(&PlacementScope)>) -> MeasureResult,
}

/// Measures a pager (Compose's `measurePager`): see the module's doc.
pub(crate) fn measure_pager(scope: &LazyLayoutMeasureScope, a: PagerMeasureArgs<'_>) -> PagerMeasureResult {
    assert!(a.before_content_padding >= 0, "negative beforeContentPadding");
    assert!(a.after_content_padding >= 0, "negative afterContentPadding");
    let page_size_with_spacing = (a.page_available_size + a.space_between_pages).max(0);
    // (at most the pages there are)
    let coerced_beyond_viewport_page_count = a.beyond_viewport_page_count.min(a.page_count);
    let is_vertical = a.orientation == Orientation::Vertical;
    let child_constraints = Constraints::new(
        0,
        if is_vertical { a.constraints.max_width } else { a.page_available_size },
        0,
        if !is_vertical { a.constraints.max_height } else { a.page_available_size },
    );

    if a.page_count <= 0 {
        return PagerMeasureResult {
            visible_pages_info: Vec::new(),
            page_size: a.page_available_size,
            page_spacing: a.space_between_pages,
            after_content_padding: a.after_content_padding,
            orientation: a.orientation,
            viewport_start_offset: -a.before_content_padding,
            viewport_end_offset: a.main_axis_available_size + a.after_content_padding,
            reverse_layout: false,
            beyond_viewport_page_count: coerced_beyond_viewport_page_count,
            first_visible_page: None,
            current_page: None,
            current_page_offset_fraction: 0.0,
            first_visible_page_scroll_offset: 0,
            can_scroll_forward: false,
            snap_position: a.snap_position,
            measure_result: (a.layout)(a.constraints.min_width, a.constraints.min_height, Box::new(|_| {})),
            remeasure_needed: false,
            extra_pages_before: Vec::new(),
            extra_pages_after: Vec::new(),
            coroutine_scope: Some(a.coroutine_scope),
            density: a.density,
            child_constraints,
        };
    }

    let layout_direction = scope.layout_direction();
    let placeables_cache: RefCell<HashMap<i32, Vec<Placeable>>> = RefCell::new(HashMap::new());
    let get_and_measure = |index: i32| -> Rc<MeasuredPage> {
        let key = a.pager_item_provider.get_key(index);
        let cached = placeables_cache.borrow().get(&index).cloned();
        let placeables = match cached {
            Some(p) => p,
            None => {
                let placeables: Vec<Placeable> =
                    scope.compose(index).iter().map(|m| m.measure(child_constraints)).collect();
                placeables_cache.borrow_mut().insert(index, placeables.clone());
                placeables
            }
        };
        Rc::new(MeasuredPage::new(
            index,
            a.page_available_size,
            placeables,
            a.visual_page_offset,
            key,
            a.orientation,
            a.horizontal_alignment.clone(),
            a.vertical_alignment.clone(),
            layout_direction,
            a.reverse_layout,
        ))
    };

    // The first visible page from the current one (the scroll applied to its offset already).
    let mut first_visible_page = a.current_page;
    let mut first_visible_page_offset = a.current_page_offset;
    while first_visible_page > 0 && first_visible_page_offset > 0 {
        first_visible_page -= 1;
        first_visible_page_offset -= page_size_with_spacing;
    }
    // (the scroll offset the other sign)
    let first_visible_page_scroll_offset = -first_visible_page_offset;

    let mut current_first_page = first_visible_page;
    let mut current_first_page_scroll_offset = first_visible_page_scroll_offset;
    if current_first_page >= a.page_count {
        // (fewer pages than the one scrolled to)
        current_first_page = a.page_count - 1;
        current_first_page_scroll_offset = 0;
    }

    let mut visible_pages: Vec<Rc<MeasuredPage>> = Vec::new();
    let min_offset = -a.before_content_padding + if a.space_between_pages < 0 { a.space_between_pages } else { 0 };
    let max_offset = a.main_axis_available_size;

    // Pages in the start padding composed too, a negative spacing neutralised.
    current_first_page_scroll_offset += min_offset;
    let mut max_cross_axis = 0;

    // Scrolled back, or pages in the start padding: the pages before composed.
    while current_first_page_scroll_offset < 0 && current_first_page > 0 {
        let previous = current_first_page - 1;
        let measured_page = get_and_measure(previous);
        max_cross_axis = max_cross_axis.max(measured_page.cross_axis_size);
        visible_pages.insert(0, measured_page);
        current_first_page_scroll_offset += page_size_with_spacing;
        current_first_page = previous;
    }
    if current_first_page_scroll_offset < min_offset {
        current_first_page_scroll_offset = min_offset;
    }
    // (the padding neutralised again)
    current_first_page_scroll_offset -= min_offset;

    let mut index = current_first_page;
    let max_main_axis = (max_offset + a.after_content_padding).max(0);
    let mut current_main_axis_offset = -current_first_page_scroll_offset;
    let mut remeasure_needed = false;

    // The pages composed going back skipped first.
    let mut index_in_visible_items = 0;
    while index_in_visible_items < visible_pages.len() {
        if current_main_axis_offset >= max_main_axis {
            visible_pages.remove(index_in_visible_items);
            remeasure_needed = true;
        } else {
            index += 1;
            current_main_axis_offset += page_size_with_spacing;
            index_in_visible_items += 1;
        }
    }

    // Then forward until the viewport is filled.
    while index < a.page_count
        && (current_main_axis_offset < max_main_axis || current_main_axis_offset <= 0 || visible_pages.is_empty())
    {
        let measured_page = get_and_measure(index);
        // (no spacing after the last page)
        current_main_axis_offset +=
            if index == a.page_count - 1 { a.page_available_size } else { page_size_with_spacing };
        if current_main_axis_offset <= min_offset && index != a.page_count - 1 {
            // (out of view: the first page moves on)
            current_first_page = index + 1;
            current_first_page_scroll_offset -= page_size_with_spacing;
            remeasure_needed = true;
        } else {
            max_cross_axis = max_cross_axis.max(measured_page.cross_axis_size);
            visible_pages.push(measured_page);
        }
        index += 1;
    }

    // The viewport not filled: scrolled back if there are pages before.
    if current_main_axis_offset < max_offset {
        let to_scroll_back = max_offset - current_main_axis_offset;
        current_first_page_scroll_offset -= to_scroll_back;
        current_main_axis_offset += to_scroll_back;
        while current_first_page_scroll_offset < a.before_content_padding && current_first_page > 0 {
            let previous_index = current_first_page - 1;
            let measured_page = get_and_measure(previous_index);
            max_cross_axis = max_cross_axis.max(measured_page.cross_axis_size);
            visible_pages.insert(0, measured_page);
            current_first_page_scroll_offset += page_size_with_spacing;
            current_first_page = previous_index;
        }
        if current_first_page_scroll_offset < 0 {
            current_main_axis_offset += current_first_page_scroll_offset;
            current_first_page_scroll_offset = 0;
        }
    }

    assert!(current_first_page_scroll_offset >= 0, "invalid currentFirstPageScrollOffset");
    let visible_pages_scroll_offset = -current_first_page_scroll_offset;
    let mut first_page = visible_pages[0].clone();

    // Pages wholly in the start padding aren't the first visible one.
    if a.before_content_padding > 0 || a.space_between_pages < 0 {
        let mut i = 0;
        while i < visible_pages.len() {
            let size = page_size_with_spacing;
            if current_first_page_scroll_offset != 0
                && size <= current_first_page_scroll_offset
                && i != visible_pages.len() - 1
            {
                current_first_page_scroll_offset -= size;
                first_page = visible_pages[i + 1].clone();
                i += 1;
            } else {
                break;
            }
        }
    }

    // The pages beyond the viewport and the pinned ones.
    let extra_pages_before = {
        let mut list = Vec::new();
        let start = (current_first_page - coerced_beyond_viewport_page_count).max(0);
        for i in (start..current_first_page).rev() {
            list.push(get_and_measure(i));
        }
        for &page_index in &a.pinned_pages {
            if page_index < start {
                list.push(get_and_measure(page_index));
            }
        }
        list
    };
    for p in &extra_pages_before {
        max_cross_axis = max_cross_axis.max(p.cross_axis_size);
    }
    let extra_pages_after = {
        let mut list = Vec::new();
        let current_last_page = visible_pages.last().unwrap().index;
        let end = coerced_beyond_viewport_page_count.min(a.page_count - current_last_page - 1) + current_last_page;
        for i in current_last_page + 1..=end {
            list.push(get_and_measure(i));
        }
        for &page_index in &a.pinned_pages {
            if page_index > end && page_index < a.page_count {
                list.push(get_and_measure(page_index));
            }
        }
        list
    };
    for p in &extra_pages_after {
        max_cross_axis = max_cross_axis.max(p.cross_axis_size);
    }

    let no_extra_pages =
        Rc::ptr_eq(&first_page, &visible_pages[0]) && extra_pages_before.is_empty() && extra_pages_after.is_empty();

    let layout_width =
        a.constraints.constrain_width(if is_vertical { max_cross_axis } else { current_main_axis_offset });
    let layout_height =
        a.constraints.constrain_height(if is_vertical { current_main_axis_offset } else { max_cross_axis });

    let positioned_pages = calculate_pages_offsets(
        &visible_pages,
        &extra_pages_before,
        &extra_pages_after,
        layout_width,
        layout_height,
        current_main_axis_offset,
        max_offset,
        visible_pages_scroll_offset,
        a.orientation,
        a.reverse_layout,
        &a.density,
        a.space_between_pages,
        a.page_available_size,
    );

    let first_index = visible_pages[0].index;
    let last_index = visible_pages.last().unwrap().index;
    let visible_pages_info: Vec<Rc<MeasuredPage>> = if no_extra_pages {
        positioned_pages.clone()
    } else {
        positioned_pages.iter().filter(|p| p.index >= first_index && p.index <= last_index).cloned().collect()
    };
    let positioned_pages_before: Vec<Rc<MeasuredPage>> = if extra_pages_before.is_empty() {
        Vec::new()
    } else {
        positioned_pages.iter().filter(|p| p.index < first_index).cloned().collect()
    };
    let positioned_pages_after: Vec<Rc<MeasuredPage>> = if extra_pages_after.is_empty() {
        Vec::new()
    } else {
        positioned_pages.iter().filter(|p| p.index > last_index).cloned().collect()
    };

    let layout_size = a.main_axis_available_size + a.before_content_padding + a.after_content_padding;
    let new_current_page = calculate_new_current_page(
        layout_size,
        &visible_pages_info,
        a.before_content_padding,
        a.after_content_padding,
        a.page_available_size,
        &a.snap_position,
        a.page_count,
    );
    let snap_offset = a.snap_position.position(
        layout_size,
        a.page_available_size,
        a.before_content_padding,
        a.after_content_padding,
        new_current_page.as_ref().map_or(0, |p| p.index),
        a.page_count,
    );
    let current_page_position_offset = new_current_page.as_ref().map_or(0, |p| p.offset());
    let current_page_offset_fraction = if page_size_with_spacing == 0 {
        0.0
    } else {
        ((snap_offset - current_page_position_offset) as f32 / page_size_with_spacing as f32)
            .clamp(MinPageOffset, MaxPageOffset)
    };

    let invalidator = a.placement_scope_invalidator.clone();
    let pages_to_place = positioned_pages.clone();
    let placement = Box::new(move |scope: &PlacementScope| {
        // (a motion frame of reference's placement: it scrolls — what reads these offsets may apply their changes at
        // once rather than animate them, not to chase the scroll)
        scope.with_motion_frame_of_reference_placement(|scope| {
            for page in &pages_to_place {
                page.place(scope);
            }
        });
        // (read while placing: the state places again by invalidating it)
        invalidator.attach_to_scope();
    });
    PagerMeasureResult {
        visible_pages_info,
        page_size: a.page_available_size,
        page_spacing: a.space_between_pages,
        after_content_padding: a.after_content_padding,
        orientation: a.orientation,
        viewport_start_offset: -a.before_content_padding,
        viewport_end_offset: max_offset + a.after_content_padding,
        reverse_layout: a.reverse_layout,
        beyond_viewport_page_count: coerced_beyond_viewport_page_count,
        first_visible_page: Some(first_page),
        current_page: new_current_page,
        current_page_offset_fraction,
        first_visible_page_scroll_offset: current_first_page_scroll_offset,
        can_scroll_forward: index < a.page_count || current_main_axis_offset > max_offset,
        snap_position: a.snap_position,
        measure_result: (a.layout)(layout_width, layout_height, placement),
        remeasure_needed,
        extra_pages_before: positioned_pages_before,
        extra_pages_after: positioned_pages_after,
        coroutine_scope: Some(a.coroutine_scope),
        density: a.density,
        child_constraints,
    }
}

/// The page closest to its snap position (Compose's `calculateNewCurrentPage`).
fn calculate_new_current_page(
    viewport_size: i32,
    visible_pages_info: &[Rc<MeasuredPage>],
    before_content_padding: i32,
    after_content_padding: i32,
    item_size: i32,
    snap_position: &SnapPosition,
    page_count: i32,
) -> Option<Rc<MeasuredPage>> {
    // (Kotlin's maxBy: the first of the largest)
    let mut best: Option<(&Rc<MeasuredPage>, f32)> = None;
    for page in visible_pages_info {
        let v = -calculate_distance_to_desired_snap_position(
            viewport_size,
            before_content_padding,
            after_content_padding,
            item_size,
            page.offset(),
            page.index,
            snap_position,
            page_count,
        )
        .abs();
        if best.is_none_or(|(_, b)| v > b) {
            best = Some((page, v));
        }
    }
    best.map(|(p, _)| p.clone())
}

/// Positions the pages (Compose's `calculatePagesOffsets`): spaced by the page spacing when they don't fill the
/// viewport, else one after the other from the scroll offset, the extra pages each side.
#[allow(clippy::too_many_arguments)]
fn calculate_pages_offsets(
    pages: &[Rc<MeasuredPage>],
    extra_pages_before: &[Rc<MeasuredPage>],
    extra_pages_after: &[Rc<MeasuredPage>],
    layout_width: i32,
    layout_height: i32,
    final_main_axis_offset: i32,
    max_offset: i32,
    pages_scroll_offset: i32,
    orientation: Orientation,
    reverse_layout: bool,
    density: &Density,
    space_between_pages: i32,
    page_available_size: i32,
) -> Vec<Rc<MeasuredPage>> {
    let page_size_with_spacing = page_available_size + space_between_pages;
    let main_axis_layout_size = if orientation == Orientation::Vertical { layout_height } else { layout_width };
    let has_spare_space = final_main_axis_offset < main_axis_layout_size.min(max_offset);
    if has_spare_space {
        assert!(pages_scroll_offset == 0, "non-zero pagesScrollOffset={pages_scroll_offset}");
    }
    let mut positioned_pages = Vec::with_capacity(pages.len() + extra_pages_before.len() + extra_pages_after.len());
    if has_spare_space {
        assert!(extra_pages_before.is_empty() && extra_pages_after.is_empty(), "No extra pages");
        let pages_count = pages.len();
        let reverse_aware = |i: usize| if !reverse_layout { i } else { pages_count - i - 1 };
        let sizes = vec![page_available_size; pages_count];
        let mut offsets = vec![0; pages_count];
        let spacing = Dp(space_between_pages as f32 / density.density);
        if orientation == Orientation::Vertical {
            Arrangement::<Vertical>::spaced_by(spacing).arrange(density, main_axis_layout_size, &sizes, &mut offsets);
        } else {
            // (left to right: mirrored by placing relative)
            Arrangement::<Horizontal>::spaced_by(spacing).arrange(
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
            // (reversed, the offsets are in the pages' reversed order)
            let page = &pages[reverse_aware(index)];
            let relative_offset =
                if reverse_layout { main_axis_layout_size - absolute_offset - page.size } else { absolute_offset };
            page.position(relative_offset, layout_width, layout_height);
            positioned_pages.push(page.clone());
        }
    } else {
        let mut current_main_axis = pages_scroll_offset;
        for it in extra_pages_before {
            current_main_axis -= page_size_with_spacing;
            it.position(current_main_axis, layout_width, layout_height);
            positioned_pages.push(it.clone());
        }
        current_main_axis = pages_scroll_offset;
        for it in pages {
            it.position(current_main_axis, layout_width, layout_height);
            positioned_pages.push(it.clone());
            current_main_axis += page_size_with_spacing;
        }
        for it in extra_pages_after {
            it.position(current_main_axis, layout_width, layout_height);
            positioned_pages.push(it.clone());
            current_main_axis += page_size_with_spacing;
        }
    }
    positioned_pages
}

/// Where the current page is from the layout's start (Compose's `SnapPosition.currentPageOffset`): its snap
/// position less its offset fraction of a page.
#[allow(clippy::too_many_arguments)]
pub(crate) fn current_page_offset(
    snap_position: &SnapPosition,
    layout_size: i32,
    page_size: i32,
    space_between_pages: i32,
    before_content_padding: i32,
    after_content_padding: i32,
    current_page: i32,
    current_page_offset_fraction: f32,
    page_count: i32,
) -> i32 {
    let snap_offset = snap_position.position(
        layout_size,
        page_size,
        before_content_padding,
        after_content_padding,
        current_page,
        page_count,
    );
    (snap_offset as f32 - current_page_offset_fraction * (page_size + space_between_pages) as f32).round() as i32
}

// ---------------------------------------------------------------------------------------------------------------
// The measure policy

/// What the pager's measure policy is made of.
pub(crate) struct PagerMeasureConfig {
    pub(crate) item_provider_lambda: Rc<dyn Fn() -> Rc<dyn LazyLayoutItemProvider>>,
    pub(crate) state: PagerState,
    pub(crate) content_padding: PaddingValues,
    pub(crate) reverse_layout: bool,
    pub(crate) orientation: Orientation,
    pub(crate) beyond_viewport_page_count: i32,
    pub(crate) page_spacing: Dp,
    pub(crate) page_size: PageSize,
    pub(crate) horizontal_alignment: Option<HorizontalAlignment>,
    pub(crate) vertical_alignment: Option<VerticalAlignment>,
    pub(crate) snap_position: SnapPosition,
    pub(crate) coroutine_scope: CoroutineScope,
}

/// The pager's measure policy (Compose's `rememberPagerMeasurePolicy`'s): the paddings resolved, the page size
/// worked out, the current page found again by key and its offset from its snap position, measured, the result
/// applied to the state (and the cache window's pages kept around).
pub(crate) fn pager_measure_policy(c: PagerMeasureConfig) -> Rc<dyn LazyLayoutMeasurePolicy> {
    Rc::new(move |scope: &LazyLayoutMeasureScope, container_constraints: Constraints| -> MeasureResult {
        let state = &c.state;
        state.inner.measurement_scope_invalidator.attach_to_scope();
        let is_vertical = c.orientation == Orientation::Vertical;
        check_scrollable_container_constraints(container_constraints, c.orientation);
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

        state.inner.density.set(density);
        let space_between_pages = density.round_to_px(c.page_spacing);

        // (negative when the content padding is more than the constraints)
        let main_axis_available_size = if is_vertical {
            container_constraints.max_height - total_vertical_padding
        } else {
            container_constraints.max_width - total_horizontal_padding
        };
        let visual_item_offset = if !c.reverse_layout || main_axis_available_size > 0 {
            IntOffset::new(start_padding, top_padding)
        } else {
            IntOffset::new(
                if is_vertical { start_padding } else { start_padding + main_axis_available_size },
                if is_vertical { top_padding + main_axis_available_size } else { top_padding },
            )
        };

        let page_available_size =
            c.page_size.calculate_main_axis_page_size(&density, main_axis_available_size, space_between_pages).max(0);

        state.inner.premeasure_constraints.set(Constraints::new(
            0,
            if is_vertical { content_constraints.max_width } else { page_available_size },
            0,
            if !is_vertical { content_constraints.max_height } else { page_available_size },
        ));
        let item_provider = (c.item_provider_lambda)();
        // (Compose's `pageCount()`: the state's page count, which the item provider's pages are made of)
        let page_count = state.page_count();

        let layout_size = main_axis_available_size + before_content_padding + after_content_padding;
        let (current_page, current_page_offset) = without_read_observation(|| {
            let current_page = state.match_scroll_position_with_key(&*item_provider, state.current_page());
            let offset = current_page_offset(
                &c.snap_position,
                layout_size,
                page_available_size,
                space_between_pages,
                before_content_padding,
                after_content_padding,
                state.current_page(),
                state.current_page_offset_fraction(),
                page_count,
            );
            (current_page, offset)
        });

        let pinned_pages = calculate_lazy_layout_pinned_indices(
            &*item_provider,
            &state.inner.pinned_pages,
            &state.inner.beyond_bounds_info,
        );

        let layout = |width: i32, height: i32, placement: Box<dyn Fn(&PlacementScope)>| {
            scope.layout(
                container_constraints.constrain_width(width + total_horizontal_padding),
                container_constraints.constrain_height(height + total_vertical_padding),
                placement,
            )
        };
        let measure_result = measure_pager(
            scope,
            PagerMeasureArgs {
                page_count,
                pager_item_provider: item_provider,
                main_axis_available_size,
                before_content_padding,
                after_content_padding,
                space_between_pages,
                current_page,
                current_page_offset,
                constraints: content_constraints,
                orientation: c.orientation,
                vertical_alignment: c.vertical_alignment.clone(),
                horizontal_alignment: c.horizontal_alignment.clone(),
                reverse_layout: c.reverse_layout,
                visual_page_offset: visual_item_offset,
                page_available_size,
                beyond_viewport_page_count: c.beyond_viewport_page_count,
                pinned_pages,
                snap_position: c.snap_position.clone(),
                placement_scope_invalidator: state.inner.placement_scope_invalidator.clone(),
                coroutine_scope: c.coroutine_scope.clone(),
                density,
                layout: &layout,
            },
        );
        let result = Rc::new(measure_result);
        state.apply_measure_result(result.clone(), scope.is_looking_ahead(), false);
        // (the cache window's pages kept around after the logic saw the result: Compose's `keepAroundItems`)
        let logic = &state.inner.cache_window_logic;
        if logic.has_valid_bounds() && !result.visible_pages_info.is_empty() {
            let first_visible_page_index = result.visible_pages_info[0].index;
            let last_visible_page_index = result.visible_pages_info.last().unwrap().index;
            for item in logic.prefetch_window_start_line()..first_visible_page_index {
                scope.compose(item);
            }
            for item in (last_visible_page_index + 1)..=logic.prefetch_window_end_line() {
                scope.compose(item);
            }
        }
        result.measure_result.clone()
    })
}
