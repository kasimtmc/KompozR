//! How a lazy grid prefetches (Compose's LazyGridPrefetchStrategy.kt and LazyGridCacheWindowStrategy.kt): by
//! default the line next in the scroll's direction, scheduled as the grid scrolls (urgent when the next frame's
//! scroll would show it), cancelled when the direction changes; with a cache window, as far as the window reaches.
//! Nested lazy layouts in a prefetched item prefetch their first items.

use super::layout_info::LazyGridLayoutInfo;
use crate::gestures::Orientation;
use crate::lazy::layout::{
    CacheWindowLogic, CacheWindowScope, InvalidIndex, LazyKey, LazyLayoutCacheWindow, NestedPrefetchScope,
    PrefetchHandle, UnspecifiedNestedPrefetchCount,
};
use kompoz_graphics::Density;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What a lazy grid's prefetch strategy schedules through (Compose's LazyGridPrefetchScope).
pub trait LazyGridPrefetchScope {
    /// Schedules the items of line `line_index` to be composed and measured ahead; `on_prefetch_finished` once
    /// they all are (Compose's two `scheduleLinePrefetch`: without one, none).
    fn schedule_line_prefetch(
        &self,
        line_index: i32,
        on_prefetch_finished: Option<Rc<dyn Fn(&dyn LazyGridPrefetchResultScope)>>,
    ) -> Vec<PrefetchHandle>;
}

/// What a line's prefetch finished tells (Compose's LazyGridPrefetchResultScope): the line, its items' count and
/// each one's size along the grid.
pub trait LazyGridPrefetchResultScope {
    fn line_item_count(&self) -> i32;
    fn line_index(&self) -> i32;
    fn get_main_axis_size(&self, item_index_in_line: i32) -> i32;
}

pub(crate) struct LazyGridPrefetchResultScopeImpl {
    pub(crate) line_index: i32,
    pub(crate) main_axis_sizes: Vec<i32>,
}

impl LazyGridPrefetchResultScope for LazyGridPrefetchResultScopeImpl {
    fn line_item_count(&self) -> i32 {
        self.main_axis_sizes.len() as i32
    }
    fn line_index(&self) -> i32 {
        self.line_index
    }
    fn get_main_axis_size(&self, item_index_in_line: i32) -> i32 {
        self.main_axis_sizes[item_index_in_line as usize]
    }
}

/// How a lazy grid prefetches (Compose's LazyGridPrefetchStrategy): told when it scrolls, when its items in view
/// change, and asked what to prefetch as a nested layout in an item another layout prefetches.
pub trait LazyGridPrefetchStrategy: Any {
    /// The grid scrolled by `delta` (negative forward), `layout_info` its layout now.
    fn on_scroll(&self, scope: &Rc<dyn LazyGridPrefetchScope>, delta: f32, layout_info: &LazyGridLayoutInfo);
    /// The grid measured with other items in view.
    fn on_visible_items_updated(&self, scope: &Rc<dyn LazyGridPrefetchScope>, layout_info: &LazyGridLayoutInfo);
    /// What to prefetch as a nested grid, its first visible item `first_visible_item_index`.
    fn on_nested_prefetch(&self, scope: &dyn NestedPrefetchScope, first_visible_item_index: i32);
}

/// The default strategy (Compose's `LazyGridPrefetchStrategy(nestedPrefetchItemCount)`): the next line in the
/// scroll's direction; as a nested grid, its first `nested_prefetch_item_count` items until the outer layout knows
/// how many it shows.
/// Kotlin: `LazyGridPrefetchStrategy(nestedPrefetchItemCount: Int = 2)`
#[kompoz_runtime::named]
pub fn lazy_grid_prefetch_strategy(#[default(2)] nested_prefetch_item_count: i32) -> Rc<dyn LazyGridPrefetchStrategy> {
    Rc::new(DefaultLazyGridPrefetchStrategy::new(nested_prefetch_item_count))
}

const UnsetItemCount: i32 = -1;

/// Compose's DefaultLazyGridPrefetchStrategy.
struct DefaultLazyGridPrefetchStrategy {
    initial_nested_prefetch_item_count: i32,
    line_to_prefetch: Cell<i32>,
    current_line_prefetch_handles: RefCell<Vec<PrefetchHandle>>,
    was_scrolling_forward: Cell<bool>,
    previous_pass_item_count: Cell<i32>,
    previous_pass_delta: Cell<f32>,
}

impl DefaultLazyGridPrefetchStrategy {
    fn new(initial_nested_prefetch_item_count: i32) -> Self {
        DefaultLazyGridPrefetchStrategy {
            initial_nested_prefetch_item_count,
            line_to_prefetch: Cell::new(-1),
            current_line_prefetch_handles: RefCell::new(Vec::new()),
            was_scrolling_forward: Cell::new(false),
            previous_pass_item_count: Cell::new(UnsetItemCount),
            previous_pass_delta: Cell::new(0.0),
        }
    }

    fn reset_prefetch_state(&self) {
        self.line_to_prefetch.set(-1);
        for h in self.current_line_prefetch_handles.borrow_mut().drain(..) {
            h.cancel();
        }
    }

    fn evaluate_prefetch_for_cancellation(
        &self,
        layout_info: &LazyGridLayoutInfo,
        current_prefetching_line_index: i32,
        scrolling_forward: bool,
    ) {
        if current_prefetching_line_index != -1 && !layout_info.items().is_empty() {
            let expected_line_to_prefetch = calculate_line_index_to_prefetch(layout_info, scrolling_forward);
            if current_prefetching_line_index != expected_line_to_prefetch {
                self.reset_prefetch_state();
            }
        }
    }

    fn schedule(&self, scope: &Rc<dyn LazyGridPrefetchScope>, line_to_prefetch: i32) {
        self.line_to_prefetch.set(line_to_prefetch);
        let handles = scope.schedule_line_prefetch(line_to_prefetch, None);
        *self.current_line_prefetch_handles.borrow_mut() = handles;
    }
}

fn line_of(item: &super::layout_info::LazyGridItemInfo, orientation: Orientation) -> i32 {
    if orientation == Orientation::Vertical { item.row } else { item.column }
}

fn calculate_line_index_to_prefetch(layout_info: &LazyGridLayoutInfo, scrolling_forward: bool) -> i32 {
    let items = layout_info.items();
    if scrolling_forward {
        line_of(items.last().unwrap(), layout_info.orientation) + 1
    } else {
        line_of(&items[0], layout_info.orientation) - 1
    }
}

fn calculate_closest_next_item_to_prefetch(layout_info: &LazyGridLayoutInfo, scrolling_forward: bool) -> i32 {
    let items = layout_info.items();
    if scrolling_forward { items.last().unwrap().index + 1 } else { items[0].index - 1 }
}

impl LazyGridPrefetchStrategy for DefaultLazyGridPrefetchStrategy {
    fn on_scroll(&self, scope: &Rc<dyn LazyGridPrefetchScope>, delta: f32, layout_info: &LazyGridLayoutInfo) {
        if !layout_info.items().is_empty() {
            let scrolling_forward = delta < 0.0;
            let line_to_prefetch = calculate_line_index_to_prefetch(layout_info, scrolling_forward);
            let closest_next_item_to_prefetch = calculate_closest_next_item_to_prefetch(layout_info, scrolling_forward);
            if closest_next_item_to_prefetch >= 0 && closest_next_item_to_prefetch < layout_info.total_items_count {
                if line_to_prefetch != self.line_to_prefetch.get() && line_to_prefetch >= 0 {
                    if self.was_scrolling_forward.get() != scrolling_forward {
                        // (the direction changed: the line prefetched last won't show soon, let go)
                        for h in self.current_line_prefetch_handles.borrow().iter() {
                            h.cancel();
                        }
                    }
                    self.was_scrolling_forward.set(scrolling_forward);
                    self.schedule(scope, line_to_prefetch);
                }
                // (with the same delta next frame, would it show? urgent then)
                let orientation = layout_info.orientation;
                if scrolling_forward {
                    let last_item = layout_info.items().last().unwrap();
                    let item_size = last_item.size_on_main_axis(orientation);
                    let item_spacing = layout_info.main_axis_item_spacing;
                    let distance_to_prefetch_item =
                        last_item.offset_on_main_axis(orientation) + item_size + item_spacing
                            - layout_info.viewport_end_offset;
                    if (distance_to_prefetch_item as f32) < -delta {
                        for h in self.current_line_prefetch_handles.borrow().iter() {
                            h.mark_as_urgent();
                        }
                    }
                } else {
                    let first_item = &layout_info.items()[0];
                    let distance_to_prefetch_item =
                        layout_info.viewport_start_offset - first_item.offset_on_main_axis(orientation);
                    if (distance_to_prefetch_item as f32) < delta {
                        for h in self.current_line_prefetch_handles.borrow().iter() {
                            h.mark_as_urgent();
                        }
                    }
                }
            }
        }
        self.previous_pass_delta.set(delta);
    }

    fn on_visible_items_updated(&self, scope: &Rc<dyn LazyGridPrefetchScope>, layout_info: &LazyGridLayoutInfo) {
        self.evaluate_prefetch_for_cancellation(
            layout_info,
            self.line_to_prefetch.get(),
            self.was_scrolling_forward.get(),
        );
        let current_pass_item_count = layout_info.total_items_count;
        // (the item count changed: prefetched again)
        if self.previous_pass_item_count.get() != UnsetItemCount
            && self.previous_pass_delta.get() != 0.0
            && self.previous_pass_item_count.get() != current_pass_item_count
            && !layout_info.items().is_empty()
        {
            let forward = self.previous_pass_delta.get() < 0.0;
            let line_to_prefetch = calculate_line_index_to_prefetch(layout_info, forward);
            let closest_next_item_to_prefetch = calculate_closest_next_item_to_prefetch(layout_info, forward);
            if closest_next_item_to_prefetch >= 0
                && closest_next_item_to_prefetch < layout_info.total_items_count
                && line_to_prefetch != self.line_to_prefetch.get()
                && line_to_prefetch >= 0
            {
                self.schedule(scope, line_to_prefetch);
            }
        }
        self.previous_pass_item_count.set(current_pass_item_count);
    }

    fn on_nested_prefetch(&self, scope: &dyn NestedPrefetchScope, first_visible_item_index: i32) {
        let count = if scope.nested_prefetch_item_count() == UnspecifiedNestedPrefetchCount {
            self.initial_nested_prefetch_item_count
        } else {
            scope.nested_prefetch_item_count()
        };
        for i in 0..count {
            scope.schedule_precomposition(first_visible_item_index + i);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The cache window strategy

/// A grid's strategy with a cache window (Compose's LazyGridCacheWindowPrefetchStrategy): by lines.
pub(crate) struct LazyGridCacheWindowPrefetchStrategy {
    pub(crate) logic: Rc<CacheWindowLogic>,
    cache_window_scope: Rc<LazyGridCacheWindowScope>,
}

impl LazyGridCacheWindowPrefetchStrategy {
    pub(crate) fn new(cache_window: Rc<dyn LazyLayoutCacheWindow>) -> Self {
        LazyGridCacheWindowPrefetchStrategy {
            logic: CacheWindowLogic::new(cache_window, true),
            cache_window_scope: Rc::new(LazyGridCacheWindowScope::default()),
        }
    }

    fn apply_window_scope(
        &self,
        scope: &Rc<dyn LazyGridPrefetchScope>,
        layout_info: &LazyGridLayoutInfo,
    ) -> Rc<dyn CacheWindowScope> {
        *self.cache_window_scope.layout_info.borrow_mut() = layout_info.clone();
        *self.cache_window_scope.prefetch_scope.borrow_mut() = Some(scope.clone());
        self.cache_window_scope.clone()
    }
}

impl LazyGridPrefetchStrategy for LazyGridCacheWindowPrefetchStrategy {
    fn on_scroll(&self, scope: &Rc<dyn LazyGridPrefetchScope>, delta: f32, layout_info: &LazyGridLayoutInfo) {
        let window_scope = self.apply_window_scope(scope, layout_info);
        self.logic.on_scroll(&window_scope, delta);
    }

    fn on_visible_items_updated(&self, scope: &Rc<dyn LazyGridPrefetchScope>, layout_info: &LazyGridLayoutInfo) {
        let window_scope = self.apply_window_scope(scope, layout_info);
        self.logic.on_visible_items_updated(&window_scope);
    }

    fn on_nested_prefetch(&self, scope: &dyn NestedPrefetchScope, first_visible_item_index: i32) {
        for i in 0..scope.nested_prefetch_item_count() {
            scope.schedule_precomposition(first_visible_item_index + i);
        }
    }
}

/// The cache window's view of a grid's last layout (Compose's LazyGridCacheWindowScope): by lines.
#[derive(Default)]
struct LazyGridCacheWindowScope {
    layout_info: RefCell<LazyGridLayoutInfo>,
    prefetch_scope: RefCell<Option<Rc<dyn LazyGridPrefetchScope>>>,
    /// The last measure's line of an item and items of a line (a measure result's own, which Compose reaches by
    /// casting the layout info to it).
    measure_result: RefCell<Option<GridLines>>,
}

/// What the cache window asks a grid's measure result about its lines.
#[derive(Clone)]
pub(crate) struct GridLines {
    pub(crate) prefetch_info_retriever: super::measure::PrefetchInfoRetriever,
    pub(crate) line_index_provider: Rc<dyn Fn(i32) -> i32>,
}

impl LazyGridCacheWindowScope {
    fn line_index(&self, item: &super::layout_info::LazyGridItemInfo) -> i32 {
        item.line_index(self.layout_info.borrow().orientation)
    }
}

impl CacheWindowScope for LazyGridCacheWindowScope {
    fn total_items_count(&self) -> i32 {
        self.layout_info.borrow().total_items_count
    }
    fn visible_line_count(&self) -> i32 {
        self.last_visible_line_index() - self.first_visible_line_index() + 1
    }
    fn has_visible_items(&self) -> bool {
        !self.layout_info.borrow().items().is_empty()
    }
    fn main_axis_extra_space_start(&self) -> i32 {
        let info = self.layout_info.borrow();
        let first = &info.items()[0];
        // (how much of the first item peeks out at the start: positive)
        (first.offset_on_main_axis(info.orientation) + info.before_content_padding).min(0).abs()
    }
    fn main_axis_extra_space_end(&self) -> i32 {
        let info = self.layout_info.borrow();
        let last = info.items().last().unwrap();
        // (how much of the last item peeks out at the end: positive)
        let last_item_overflow_offset = last.offset_on_main_axis(info.orientation)
            + last.size_on_main_axis(info.orientation)
            + info.main_axis_item_spacing;
        (last_item_overflow_offset - info.viewport_end_offset).abs()
    }
    fn first_visible_line_index(&self) -> i32 {
        let info = self.layout_info.borrow();
        self.line_index(&info.items()[0])
    }
    fn last_visible_line_index(&self) -> i32 {
        let info = self.layout_info.borrow();
        self.line_index(info.items().last().unwrap())
    }
    fn main_axis_viewport_size(&self) -> i32 {
        self.layout_info.borrow().single_axis_viewport_size()
    }
    fn density(&self) -> Option<Density> {
        self.layout_info.borrow().density
    }
    fn schedule_prefetch(&self, line_index: i32, on_item_prefetched: Rc<dyn Fn(i32, i32)>) -> Vec<PrefetchHandle> {
        let Some(scope) = self.prefetch_scope.borrow().clone() else { return Vec::new() };
        scope.schedule_line_prefetch(
            line_index,
            Some(Rc::new(move |result: &dyn LazyGridPrefetchResultScope| {
                // (Compose keeps the last item's size here, `maxOf` of one value: the same)
                let mut tallest_element = i32::MIN;
                for i in 0..result.line_item_count() {
                    tallest_element = result.get_main_axis_size(i);
                }
                if tallest_element != i32::MIN {
                    on_item_prefetched(line_index, tallest_element);
                }
            })),
        )
    }
    fn get_visible_item_size(&self, index_in_visible_lines: i32) -> i32 {
        let line_index = index_in_visible_lines + self.first_visible_line_index();
        let info = self.layout_info.borrow();
        info.items()
            .iter()
            .filter(|i| self.line_index(i) == line_index)
            .map(|i| i.size_on_main_axis(info.orientation))
            .fold(0, i32::max)
    }
    fn get_visible_item_line(&self, index_in_visible_lines: i32) -> i32 {
        self.first_visible_line_index() + index_in_visible_lines
    }
    fn get_visible_line_key(&self, index_in_visible_lines: i32) -> Option<LazyKey> {
        // (the line's first item's key stands for it)
        let line_index = index_in_visible_lines + self.first_visible_line_index();
        let info = self.layout_info.borrow();
        info.items().iter().find(|i| self.line_index(i) == line_index).map(|i| i.key.clone())
    }
    fn get_last_index_in_line(&self, line_index: i32) -> i32 {
        let Some(lines) = self.measure_result.borrow().clone() else { return InvalidIndex };
        let items_in_line = (lines.prefetch_info_retriever)(line_index);
        match items_in_line.first() {
            None => InvalidIndex,
            // (the line's first item and its count)
            Some((first, _)) => first + items_in_line.len() as i32 - 1,
        }
    }
    fn get_last_line_index(&self) -> i32 {
        let Some(lines) = self.measure_result.borrow().clone() else { return InvalidIndex };
        if self.total_items_count() == 0 {
            return InvalidIndex;
        }
        (lines.line_index_provider)(self.total_items_count() - 1)
    }
}

impl LazyGridCacheWindowPrefetchStrategy {
    /// The last measure's lines, for the scope's questions about them.
    pub(crate) fn set_lines(&self, lines: GridLines) {
        *self.cache_window_scope.measure_result.borrow_mut() = Some(lines);
    }
}

/// The cache window logic of a strategy, if it is a cache window's.
pub(crate) fn cache_window_logic_of(strategy: &dyn LazyGridPrefetchStrategy) -> Option<Rc<CacheWindowLogic>> {
    let any: &dyn Any = strategy;
    any.downcast_ref::<LazyGridCacheWindowPrefetchStrategy>().map(|s| s.logic.clone())
}

/// The cache window strategy of a strategy, if it is one.
pub(crate) fn cache_window_strategy_of(
    strategy: &dyn LazyGridPrefetchStrategy,
) -> Option<&LazyGridCacheWindowPrefetchStrategy> {
    let any: &dyn Any = strategy;
    any.downcast_ref::<LazyGridCacheWindowPrefetchStrategy>()
}
