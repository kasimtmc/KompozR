//! How a lazy list prefetches (Compose's LazyListPrefetchStrategy.kt and LazyListCacheWindowStrategy.kt): by
//! default the item next in the scroll's direction, scheduled as the list scrolls (urgent when the next frame's
//! scroll would show it), cancelled when the direction changes; with a cache window, as far as the window reaches.
//! Nested lists in a prefetched item prefetch their first items.

use super::layout_info::LazyListLayoutInfo;
use crate::lazy::layout::{
    CacheWindowLogic, CacheWindowScope, InvalidIndex, LazyKey, LazyLayoutCacheWindow, NestedPrefetchScope,
    PrefetchHandle, UnspecifiedNestedPrefetchCount,
};
use kompoz_graphics::Density;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What a lazy list's prefetch strategy schedules through (Compose's LazyListPrefetchScope).
pub trait LazyListPrefetchScope {
    /// Schedules item `index` to be composed and measured ahead; `on_prefetch_finished` once it is.
    fn schedule_prefetch(
        &self,
        index: i32,
        on_prefetch_finished: Option<Rc<dyn Fn(&dyn LazyListPrefetchResultScope)>>,
    ) -> PrefetchHandle;
}

/// What a prefetch finished tells (Compose's LazyListPrefetchResultScope): the item and its size along the list.
pub trait LazyListPrefetchResultScope {
    fn index(&self) -> i32;
    fn main_axis_size(&self) -> i32;
}

pub(crate) struct LazyListPrefetchResultScopeImpl {
    pub(crate) index: i32,
    pub(crate) main_axis_size: i32,
}

impl LazyListPrefetchResultScope for LazyListPrefetchResultScopeImpl {
    fn index(&self) -> i32 {
        self.index
    }
    fn main_axis_size(&self) -> i32 {
        self.main_axis_size
    }
}

/// How a lazy list prefetches (Compose's LazyListPrefetchStrategy): told when it scrolls, when its items in view
/// change, and asked what to prefetch as a nested list in an item another layout prefetches.
pub trait LazyListPrefetchStrategy: Any {
    /// The list scrolled by `delta` (negative forward), `layout_info` its layout now.
    fn on_scroll(&self, scope: &Rc<dyn LazyListPrefetchScope>, delta: f32, layout_info: &LazyListLayoutInfo);
    /// The list measured with other items in view.
    fn on_visible_items_updated(&self, scope: &Rc<dyn LazyListPrefetchScope>, layout_info: &LazyListLayoutInfo);
    /// What to prefetch as a nested list, its first visible item `first_visible_item_index`.
    fn on_nested_prefetch(&self, scope: &dyn NestedPrefetchScope, first_visible_item_index: i32);
}

/// The default strategy (Compose's `LazyListPrefetchStrategy(nestedPrefetchItemCount)`): the next item in the
/// scroll's direction; as a nested list, its first `nested_prefetch_item_count` items until the outer layout knows
/// how many it shows.
/// Kotlin: `LazyListPrefetchStrategy(nestedPrefetchItemCount: Int = 2)`
#[kompoz_runtime::named]
pub fn lazy_list_prefetch_strategy(#[default(2)] nested_prefetch_item_count: i32) -> Rc<dyn LazyListPrefetchStrategy> {
    Rc::new(DefaultLazyListPrefetchStrategy::new(nested_prefetch_item_count))
}

const UnsetItemCount: i32 = -1;

/// Compose's DefaultLazyListPrefetchStrategy.
pub(crate) struct DefaultLazyListPrefetchStrategy {
    initial_nested_prefetch_item_count: i32,
    index_to_prefetch: Cell<i32>,
    current_prefetch_handle: RefCell<Option<PrefetchHandle>>,
    was_scrolling_forward: Cell<bool>,
    previous_pass_item_count: Cell<i32>,
    previous_pass_delta: Cell<f32>,
}

impl DefaultLazyListPrefetchStrategy {
    pub(crate) fn new(initial_nested_prefetch_item_count: i32) -> Self {
        DefaultLazyListPrefetchStrategy {
            initial_nested_prefetch_item_count,
            index_to_prefetch: Cell::new(-1),
            current_prefetch_handle: RefCell::new(None),
            was_scrolling_forward: Cell::new(false),
            previous_pass_item_count: Cell::new(UnsetItemCount),
            previous_pass_delta: Cell::new(0.0),
        }
    }

    fn reset_prefetch_state(&self) {
        self.index_to_prefetch.set(-1);
        if let Some(h) = self.current_prefetch_handle.borrow_mut().take() {
            h.cancel();
        }
    }

    fn evaluate_prefetch_for_cancellation(&self, layout_info: &LazyListLayoutInfo, current: i32, forward: bool) {
        if current != -1 && !layout_info.items().is_empty() {
            let expected = calculate_index_to_prefetch(layout_info, forward);
            if current != expected {
                self.reset_prefetch_state();
            }
        }
    }
}

fn calculate_index_to_prefetch(layout_info: &LazyListLayoutInfo, scrolling_forward: bool) -> i32 {
    let items = layout_info.items();
    if scrolling_forward { items.last().unwrap().index + 1 } else { items.first().unwrap().index - 1 }
}

impl LazyListPrefetchStrategy for DefaultLazyListPrefetchStrategy {
    fn on_scroll(&self, scope: &Rc<dyn LazyListPrefetchScope>, delta: f32, layout_info: &LazyListLayoutInfo) {
        if !layout_info.items().is_empty() {
            let scrolling_forward = delta < 0.0;
            let index_to_prefetch = calculate_index_to_prefetch(layout_info, scrolling_forward);
            if index_to_prefetch >= 0 && index_to_prefetch < layout_info.total_items_count {
                if index_to_prefetch != self.index_to_prefetch.get() {
                    if self.was_scrolling_forward.get() != scrolling_forward {
                        // (the direction changed: the item prefetched last won't show soon, let go)
                        self.reset_prefetch_state();
                    }
                    self.was_scrolling_forward.set(scrolling_forward);
                    self.index_to_prefetch.set(index_to_prefetch);
                    *self.current_prefetch_handle.borrow_mut() = Some(scope.schedule_prefetch(index_to_prefetch, None));
                }
                // (with the same delta next frame, would it show? urgent then)
                if scrolling_forward {
                    let last_item = layout_info.items().last().unwrap();
                    let spacing = layout_info.main_axis_item_spacing;
                    let distance_to_prefetch_item =
                        last_item.offset + last_item.size + spacing - layout_info.viewport_end_offset;
                    if (distance_to_prefetch_item as f32) < -delta
                        && let Some(h) = self.current_prefetch_handle.borrow().as_ref()
                    {
                        h.mark_as_urgent();
                    }
                } else {
                    let first_item = &layout_info.items()[0];
                    let distance_to_prefetch_item = layout_info.viewport_start_offset - first_item.offset;
                    if (distance_to_prefetch_item as f32) < delta
                        && let Some(h) = self.current_prefetch_handle.borrow().as_ref()
                    {
                        h.mark_as_urgent();
                    }
                }
            }
        }
        self.previous_pass_delta.set(delta);
    }

    fn on_visible_items_updated(&self, scope: &Rc<dyn LazyListPrefetchScope>, layout_info: &LazyListLayoutInfo) {
        self.evaluate_prefetch_for_cancellation(
            layout_info,
            self.index_to_prefetch.get(),
            self.was_scrolling_forward.get(),
        );
        let current_pass_item_count = layout_info.total_items_count;
        // (the item count changed: prefetched again)
        if self.previous_pass_item_count.get() != UnsetItemCount
            && self.previous_pass_delta.get() != 0.0
            && self.previous_pass_item_count.get() != current_pass_item_count
            && !layout_info.items().is_empty()
        {
            let index_to_prefetch = calculate_index_to_prefetch(layout_info, self.previous_pass_delta.get() < 0.0);
            if index_to_prefetch >= 0 && index_to_prefetch < current_pass_item_count {
                self.index_to_prefetch.set(index_to_prefetch);
                *self.current_prefetch_handle.borrow_mut() = Some(scope.schedule_prefetch(index_to_prefetch, None));
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

/// Nested lists have more than one item in view: 2 is the least that makes sense.
const DefaultNestedPrefetchCount: i32 = 2;

/// A list's strategy with a cache window (Compose's LazyListCacheWindowStrategy).
pub(crate) struct LazyListCacheWindowStrategy {
    pub(crate) logic: Rc<CacheWindowLogic>,
    cache_window_scope: Rc<LazyListCacheWindowScope>,
}

impl LazyListCacheWindowStrategy {
    pub(crate) fn new(cache_window: Rc<dyn LazyLayoutCacheWindow>) -> Self {
        LazyListCacheWindowStrategy {
            logic: CacheWindowLogic::new(cache_window, true),
            cache_window_scope: Rc::new(LazyListCacheWindowScope::default()),
        }
    }

    fn apply_window_scope(
        &self,
        scope: &Rc<dyn LazyListPrefetchScope>,
        layout_info: &LazyListLayoutInfo,
    ) -> Rc<dyn CacheWindowScope> {
        *self.cache_window_scope.layout_info.borrow_mut() = layout_info.clone();
        *self.cache_window_scope.prefetch_scope.borrow_mut() = Some(scope.clone());
        self.cache_window_scope.clone()
    }
}

impl LazyListPrefetchStrategy for LazyListCacheWindowStrategy {
    fn on_scroll(&self, scope: &Rc<dyn LazyListPrefetchScope>, delta: f32, layout_info: &LazyListLayoutInfo) {
        let window_scope = self.apply_window_scope(scope, layout_info);
        self.logic.on_scroll(&window_scope, delta);
    }

    fn on_visible_items_updated(&self, scope: &Rc<dyn LazyListPrefetchScope>, layout_info: &LazyListLayoutInfo) {
        let window_scope = self.apply_window_scope(scope, layout_info);
        self.logic.on_visible_items_updated(&window_scope);
    }

    fn on_nested_prefetch(&self, scope: &dyn NestedPrefetchScope, first_visible_item_index: i32) {
        let count = if scope.nested_prefetch_item_count() == -1 {
            DefaultNestedPrefetchCount
        } else {
            scope.nested_prefetch_item_count()
        };
        for i in 0..count {
            scope.schedule_precomposition(first_visible_item_index + i);
        }
    }
}

/// The cache window's view of a list's last layout (Compose's LazyListCacheWindowScope).
#[derive(Default)]
struct LazyListCacheWindowScope {
    layout_info: RefCell<LazyListLayoutInfo>,
    prefetch_scope: RefCell<Option<Rc<dyn LazyListPrefetchScope>>>,
}

impl CacheWindowScope for LazyListCacheWindowScope {
    fn total_items_count(&self) -> i32 {
        self.layout_info.borrow().total_items_count
    }
    fn visible_line_count(&self) -> i32 {
        self.layout_info.borrow().items().len() as i32
    }
    fn has_visible_items(&self) -> bool {
        !self.layout_info.borrow().items().is_empty()
    }
    fn main_axis_extra_space_start(&self) -> i32 {
        let info = self.layout_info.borrow();
        let first = &info.items()[0];
        // (how much of the first item peeks out at the start: positive)
        (first.offset + info.before_content_padding).min(0).abs()
    }
    fn main_axis_extra_space_end(&self) -> i32 {
        let info = self.layout_info.borrow();
        let last = info.items().last().unwrap();
        // (how much of the last item peeks out at the end: positive)
        (last.offset + last.size + info.main_axis_item_spacing - info.viewport_end_offset).abs()
    }
    fn first_visible_line_index(&self) -> i32 {
        self.layout_info.borrow().items()[0].index
    }
    fn last_visible_line_index(&self) -> i32 {
        self.layout_info.borrow().items().last().unwrap().index
    }
    fn main_axis_viewport_size(&self) -> i32 {
        self.layout_info.borrow().single_axis_viewport_size()
    }
    fn density(&self) -> Option<Density> {
        self.layout_info.borrow().density
    }
    fn schedule_prefetch(&self, line_index: i32, on_item_prefetched: Rc<dyn Fn(i32, i32)>) -> Vec<PrefetchHandle> {
        let Some(scope) = self.prefetch_scope.borrow().clone() else { return Vec::new() };
        vec![scope.schedule_prefetch(
            line_index,
            Some(Rc::new(move |result: &dyn LazyListPrefetchResultScope| {
                on_item_prefetched(result.index(), result.main_axis_size())
            })),
        )]
    }
    fn get_visible_item_size(&self, index_in_visible_lines: i32) -> i32 {
        self.layout_info.borrow().items()[index_in_visible_lines as usize].size
    }
    fn get_visible_item_line(&self, index_in_visible_lines: i32) -> i32 {
        self.layout_info.borrow().items()[index_in_visible_lines as usize].index
    }
    fn get_visible_line_key(&self, index_in_visible_lines: i32) -> LazyKey {
        self.layout_info.borrow().items()[index_in_visible_lines as usize].key.clone()
    }
    fn get_last_index_in_line(&self, line_index: i32) -> i32 {
        line_index
    }
    fn get_last_line_index(&self) -> i32 {
        let count = self.total_items_count();
        if count == 0 { InvalidIndex } else { count - 1 }
    }
}

/// The cache window logic of a strategy, if it is a cache window's.
pub(crate) fn cache_window_logic_of(strategy: &dyn LazyListPrefetchStrategy) -> Option<Rc<CacheWindowLogic>> {
    let any: &dyn Any = strategy;
    any.downcast_ref::<LazyListCacheWindowStrategy>().map(|s| s.logic.clone())
}
