//! Cache windows (Compose's LazyLayoutCacheWindow.kt and CacheWindowLogic.kt): a lazy layout given one keeps items
//! composed ahead of the scroll (prefetched, as far as the ahead window reaches past the viewport) and behind it
//! (kept, as far as the behind window reaches), instead of the one item the default strategy prefetches.

use super::key::LazyKey;
use super::prefetch::PrefetchHandle;
use kompoz_graphics::{Density, Dp};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::{Rc, Weak};

/// How far past the viewport a lazy layout keeps items (Compose's LazyLayoutCacheWindow), px for a viewport of
/// `viewport` px: ahead of the scroll (prefetched) and behind it (kept). Shared as `Rc<dyn LazyLayoutCacheWindow>`.
pub trait LazyLayoutCacheWindow {
    /// How far ahead, past the viewport's end the scroll goes to (Compose's `calculateAheadWindow`).
    fn calculate_ahead_window(&self, density: &Density, viewport: i32) -> i32 {
        let _ = (density, viewport);
        0
    }
    /// How far behind, past the viewport's other end (Compose's `calculateBehindWindow`).
    fn calculate_behind_window(&self, density: &Density, viewport: i32) -> i32 {
        let _ = (density, viewport);
        0
    }
}

impl fmt::Debug for dyn LazyLayoutCacheWindow {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LazyLayoutCacheWindow")
    }
}

/// A window of fixed lengths (Compose's `LazyLayoutCacheWindow(ahead, behind)`).
/// Kotlin: `LazyLayoutCacheWindow(ahead: Dp = 0.dp, behind: Dp = 0.dp)`
#[kompoz_runtime::named]
pub fn lazy_layout_cache_window(
    #[default(Dp(0.0))] ahead: Dp,
    #[default(Dp(0.0))] behind: Dp,
) -> Rc<dyn LazyLayoutCacheWindow> {
    Rc::new(DpLazyLayoutCacheWindow { ahead, behind })
}

/// A window of fractions of the viewport (Compose's `LazyLayoutCacheWindow(aheadFraction, behindFraction)`).
/// Kotlin: `LazyLayoutCacheWindow(aheadFraction: Float = 0.0f, behindFraction: Float = 0.0f)`
#[kompoz_runtime::named]
pub fn lazy_layout_cache_window_of_fractions(
    #[default(0.0)] ahead_fraction: f32,
    #[default(0.0)] behind_fraction: f32,
) -> Rc<dyn LazyLayoutCacheWindow> {
    Rc::new(FractionLazyLayoutCacheWindow { ahead_fraction, behind_fraction })
}

#[derive(PartialEq)]
struct DpLazyLayoutCacheWindow {
    ahead: Dp,
    behind: Dp,
}

impl LazyLayoutCacheWindow for DpLazyLayoutCacheWindow {
    fn calculate_ahead_window(&self, density: &Density, _: i32) -> i32 {
        density.round_to_px(self.ahead)
    }
    fn calculate_behind_window(&self, density: &Density, _: i32) -> i32 {
        density.round_to_px(self.behind)
    }
}

#[derive(PartialEq)]
struct FractionLazyLayoutCacheWindow {
    ahead_fraction: f32,
    behind_fraction: f32,
}

impl LazyLayoutCacheWindow for FractionLazyLayoutCacheWindow {
    fn calculate_ahead_window(&self, _: &Density, viewport: i32) -> i32 {
        (viewport as f32 * self.ahead_fraction).round() as i32
    }
    fn calculate_behind_window(&self, _: &Density, viewport: i32) -> i32 {
        (viewport as f32 * self.behind_fraction).round() as i32
    }
}

/// What the cache window logic reads of a layout's last layout and schedules through (Compose's CacheWindowScope);
/// lines are a list's items, a grid's rows or columns.
pub(crate) trait CacheWindowScope {
    fn total_items_count(&self) -> i32;
    fn visible_line_count(&self) -> i32;
    fn has_visible_items(&self) -> bool;
    /// How much of the first line is out of view at the start.
    fn main_axis_extra_space_start(&self) -> i32;
    /// How much of the last line is out of view at the end.
    fn main_axis_extra_space_end(&self) -> i32;
    fn first_visible_line_index(&self) -> i32;
    fn last_visible_line_index(&self) -> i32;
    fn main_axis_viewport_size(&self) -> i32;
    fn density(&self) -> Option<Density>;
    /// Prefetches line `line_index`; `on_item_prefetched(index, main_axis_size)` once each item is.
    fn schedule_prefetch(&self, line_index: i32, on_item_prefetched: Rc<dyn Fn(i32, i32)>) -> Vec<PrefetchHandle>;
    fn get_visible_item_size(&self, index_in_visible_lines: i32) -> i32;
    fn get_visible_item_line(&self, index_in_visible_lines: i32) -> i32;
    fn get_visible_line_key(&self, index_in_visible_lines: i32) -> LazyKey;
    fn get_last_index_in_line(&self, line_index: i32) -> i32;
    fn get_last_line_index(&self) -> i32;
}

pub(crate) const InvalidIndex: i32 = -1;
const InvalidItemSize: i32 = -1;
const UnsetItemCount: i32 = -1;

/// A line cached: its key (none for a prefetched one) and its size along the main axis.
struct CachedItem {
    key: Option<LazyKey>,
    main_axis_size: i32,
}

/// What the logic keeps between passes.
struct WindowState {
    prefetch_window_handles: HashMap<i32, Vec<PrefetchHandle>>,
    window_cache_with_items: HashMap<i32, CachedItem>,
    previous_pass_delta: f32,
    previous_pass_item_count: i32,
    has_updated_visible_items_once: bool,
    prefetch_window_start_line: i32,
    prefetch_window_end_line: i32,
    prefetch_window_start_extra_space: i32,
    prefetch_window_end_extra_space: i32,
    should_refill_window: bool,
    items_count: i32,
}

/// The window's bookkeeping (Compose's CacheWindowLogic): the lines kept or prefetched around the visible ones
/// and the space each side still to fill; prefetching ahead as the scroll goes, keeping behind what was in view.
pub(crate) struct CacheWindowLogic {
    cache_window: Rc<dyn LazyLayoutCacheWindow>,
    enable_initial_prefetch: bool,
    state: RefCell<WindowState>,
    me: Weak<CacheWindowLogic>,
}

impl CacheWindowLogic {
    pub(crate) fn new(
        cache_window: Rc<dyn LazyLayoutCacheWindow>,
        enable_initial_prefetch: bool,
    ) -> Rc<CacheWindowLogic> {
        Rc::new_cyclic(|me| CacheWindowLogic {
            cache_window,
            enable_initial_prefetch,
            state: RefCell::new(WindowState {
                prefetch_window_handles: HashMap::new(),
                window_cache_with_items: HashMap::new(),
                previous_pass_delta: 0.0,
                previous_pass_item_count: UnsetItemCount,
                has_updated_visible_items_once: false,
                prefetch_window_start_line: i32::MAX,
                prefetch_window_end_line: i32::MIN,
                prefetch_window_start_extra_space: 0,
                prefetch_window_end_extra_space: 0,
                should_refill_window: false,
                items_count: 0,
            }),
            me: me.clone(),
        })
    }

    pub(crate) fn prefetch_window_start_line(&self) -> i32 {
        self.state.borrow().prefetch_window_start_line
    }

    pub(crate) fn prefetch_window_end_line(&self) -> i32 {
        self.state.borrow().prefetch_window_end_line
    }

    /// The layout scrolled by `delta` (Compose's `onScroll`): kept behind, prefetched ahead.
    pub(crate) fn on_scroll(&self, scope: &Rc<dyn CacheWindowScope>, delta: f32) {
        self.fill_cache_window_backward(scope, delta);
        self.fill_cache_window_forward(scope, delta);
        self.state.borrow_mut().previous_pass_delta = delta;
    }

    /// The layout measured with other items in view (Compose's `onVisibleItemsUpdated`).
    pub(crate) fn on_visible_items_updated(&self, scope: &Rc<dyn CacheWindowScope>) {
        let first_time = !self.state.borrow().has_updated_visible_items_once;
        if first_time && self.enable_initial_prefetch {
            let prefetch_forward_window = scope
                .density()
                .map_or(0, |d| self.cache_window.calculate_ahead_window(&d, scope.main_axis_viewport_size()));
            let mut s = self.state.borrow_mut();
            // (no window ahead: nothing to fill)
            if prefetch_forward_window != 0 {
                s.should_refill_window = true;
            }
            s.has_updated_visible_items_once = true;
        }
        let previous_count = self.state.borrow().previous_pass_item_count;
        if previous_count != UnsetItemCount && previous_count != scope.total_items_count() {
            self.on_dataset_changed(scope);
        }
        self.state.borrow_mut().items_count = scope.total_items_count();
        // The visible items' sizes cached (any that was visible and left is kept or let go by the window, any that
        // changed size refills it).
        if scope.has_visible_items() {
            for i in 0..scope.visible_line_count() {
                let index = scope.get_visible_item_line(i);
                if index != InvalidIndex {
                    self.cache_visible_items_info(index, scope.get_visible_line_key(i), scope.get_visible_item_size(i));
                }
            }
            let (refill, forward) = {
                let s = self.state.borrow();
                (s.should_refill_window, s.previous_pass_delta <= 0.0)
            };
            if refill {
                self.refill_window(scope, forward);
                self.state.borrow_mut().should_refill_window = false;
            }
        } else {
            // (nothing in view: the data set is empty; the window starts over when items come)
            self.reset_strategy();
        }
        self.state.borrow_mut().previous_pass_item_count = scope.total_items_count();
    }

    fn on_dataset_changed(&self, scope: &Rc<dyn CacheWindowScope>) {
        self.state.borrow_mut().should_refill_window = true;
        if scope.has_visible_items() {
            let last_line_index = scope.get_last_line_index();
            let (delta, items_count) = {
                let mut s = self.state.borrow_mut();
                s.prefetch_window_start_line = s.prefetch_window_start_line.max(0);
                if last_line_index != InvalidIndex {
                    s.prefetch_window_end_line = s.prefetch_window_end_line.min(last_line_index);
                }
                (s.previous_pass_delta, s.items_count)
            };
            if delta <= 0.0 {
                self.remove_out_of_bounds_items(scope.last_visible_line_index(), items_count - 1);
            } else {
                self.remove_out_of_bounds_items(0, scope.first_visible_line_index());
            }
        }
    }

    /// Whether the window has been filled (Compose's `hasValidBounds`).
    pub(crate) fn has_valid_bounds(&self) -> bool {
        let s = self.state.borrow();
        s.prefetch_window_start_line != i32::MAX && s.prefetch_window_end_line != i32::MIN
    }

    fn fill_cache_window_backward(&self, scope: &Rc<dyn CacheWindowScope>, delta: f32) {
        if !scope.has_visible_items() {
            return;
        }
        let viewport = scope.main_axis_viewport_size();
        let keep_around_window = scope.density().map_or(0, |d| self.cache_window.calculate_behind_window(&d, viewport));
        self.state.borrow_mut().items_count = scope.total_items_count();
        self.on_keep_around(
            scope.first_visible_line_index(),
            scope.last_visible_line_index(),
            scope.main_axis_extra_space_end(),
            scope.main_axis_extra_space_start(),
            keep_around_window,
            delta,
            scope.total_items_count(),
        );
    }

    fn fill_cache_window_forward(&self, scope: &Rc<dyn CacheWindowScope>, delta: f32) {
        if !scope.has_visible_items() {
            return;
        }
        let viewport = scope.main_axis_viewport_size();
        let prefetch_forward_window =
            scope.density().map_or(0, |d| self.cache_window.calculate_ahead_window(&d, viewport));
        self.on_prefetch_forward(
            scope,
            scope.first_visible_line_index(),
            scope.last_visible_line_index(),
            prefetch_forward_window,
            scope.main_axis_extra_space_end(),
            scope.main_axis_extra_space_start(),
            delta,
            delta <= 0.0,
        );
    }

    fn refill_window(&self, scope: &Rc<dyn CacheWindowScope>, refill_forward: bool) {
        if !scope.has_visible_items() {
            return;
        }
        let viewport = scope.main_axis_viewport_size();
        let prefetch_forward_window =
            scope.density().map_or(0, |d| self.cache_window.calculate_ahead_window(&d, viewport));
        self.on_prefetch_forward(
            scope,
            scope.first_visible_line_index(),
            scope.last_visible_line_index(),
            prefetch_forward_window,
            scope.main_axis_extra_space_end(),
            scope.main_axis_extra_space_start(),
            0.0,
            refill_forward,
        );
    }

    /// Forgets the window (Compose's `resetStrategy`): a jump, or no items.
    pub(crate) fn reset_strategy(&self) {
        let handles: Vec<Vec<PrefetchHandle>> = {
            let mut s = self.state.borrow_mut();
            s.prefetch_window_start_line = i32::MAX;
            s.prefetch_window_end_line = i32::MIN;
            s.prefetch_window_start_extra_space = 0;
            s.prefetch_window_end_extra_space = 0;
            s.should_refill_window = false;
            s.window_cache_with_items.clear();
            s.prefetch_window_handles.drain().map(|(_, v)| v).collect()
        };
        for h in handles.iter().flatten() {
            h.cancel();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn on_prefetch_forward(
        &self,
        scope: &Rc<dyn CacheWindowScope>,
        visible_window_start: i32,
        visible_window_end: i32,
        prefetch_forward_window: i32,
        main_axis_extra_space_end: i32,
        main_axis_extra_space_start: i32,
        scroll_delta: f32,
        apply_forward_prefetch: bool,
    ) {
        let changed_scroll_direction = sign(scroll_delta) != sign(self.state.borrow().previous_pass_delta);
        if apply_forward_prefetch {
            // Scrolling forward: from the last visible line.
            {
                let mut s = self.state.borrow_mut();
                if changed_scroll_direction || s.should_refill_window {
                    s.prefetch_window_end_extra_space = prefetch_forward_window - main_axis_extra_space_end;
                    s.prefetch_window_end_line = visible_window_end;
                } else {
                    s.prefetch_window_end_extra_space = (s.prefetch_window_end_extra_space
                        + scroll_delta.abs().round() as i32)
                        .min(prefetch_forward_window - main_axis_extra_space_end);
                }
            }
            loop {
                let (extra, end_line, items_count) = {
                    let s = self.state.borrow();
                    (s.prefetch_window_end_extra_space, s.prefetch_window_end_line, s.items_count)
                };
                let last_in_line = scope.get_last_index_in_line(end_line);
                if !(extra > 0 && last_in_line != InvalidIndex && last_in_line < items_count - 1) {
                    break;
                }
                // (with the same delta next frame, would the item be needed? then it is urgent)
                let is_urgent = end_line + 1 == visible_window_end + 1
                    && scroll_delta != 0.0
                    && scroll_delta.abs() >= main_axis_extra_space_end as f32;
                // (none available to fill the window: done)
                let item_size = self.get_item_size_or_prefetch(scope, end_line + 1, is_urgent);
                if item_size == InvalidItemSize {
                    break;
                }
                let mut s = self.state.borrow_mut();
                s.prefetch_window_end_line += 1;
                s.prefetch_window_end_extra_space -= item_size;
            }
        } else {
            // Scrolling back: from the first visible line.
            {
                let mut s = self.state.borrow_mut();
                if changed_scroll_direction || s.should_refill_window {
                    s.prefetch_window_start_extra_space = prefetch_forward_window - main_axis_extra_space_start;
                    s.prefetch_window_start_line = visible_window_start;
                } else {
                    s.prefetch_window_start_extra_space = (s.prefetch_window_start_extra_space
                        + scroll_delta.abs().round() as i32)
                        .min(prefetch_forward_window - main_axis_extra_space_start);
                }
            }
            loop {
                let (extra, start_line) = {
                    let s = self.state.borrow();
                    (s.prefetch_window_start_extra_space, s.prefetch_window_start_line)
                };
                if !(extra > 0 && start_line > 0) {
                    break;
                }
                let is_urgent = start_line - 1 == visible_window_start - 1
                    && scroll_delta != 0.0
                    && scroll_delta.abs() >= main_axis_extra_space_start as f32;
                let item_size = self.get_item_size_or_prefetch(scope, start_line - 1, is_urgent);
                if item_size == InvalidItemSize {
                    break;
                }
                let mut s = self.state.borrow_mut();
                s.prefetch_window_start_line -= 1;
                s.prefetch_window_start_extra_space -= item_size;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn on_keep_around(
        &self,
        visible_window_start: i32,
        visible_window_end: i32,
        main_axis_extra_space_end: i32,
        main_axis_extra_space_start: i32,
        keep_around_window: i32,
        scroll_delta: f32,
        items_count: i32,
    ) {
        if scroll_delta <= 0.0 {
            // Scrolling forward: kept from the first visible line back.
            let start_line = {
                let mut s = self.state.borrow_mut();
                s.prefetch_window_start_extra_space = keep_around_window - main_axis_extra_space_start;
                s.prefetch_window_start_line = visible_window_start;
                while s.prefetch_window_start_extra_space > 0 && s.prefetch_window_start_line > 0 {
                    let Some(size) =
                        s.window_cache_with_items.get(&(s.prefetch_window_start_line - 1)).map(|c| c.main_axis_size)
                    else {
                        break;
                    };
                    s.prefetch_window_start_line -= 1;
                    s.prefetch_window_start_extra_space -= size;
                }
                s.prefetch_window_start_line
            };
            self.remove_out_of_bounds_items(0, start_line - 1);
        } else {
            // Scrolling back: kept from the last visible line on.
            let end_line = {
                let mut s = self.state.borrow_mut();
                s.prefetch_window_end_extra_space = keep_around_window - main_axis_extra_space_end;
                s.prefetch_window_end_line = visible_window_end;
                while s.prefetch_window_end_extra_space > 0 && s.prefetch_window_end_line < items_count - 1 {
                    let Some(size) =
                        s.window_cache_with_items.get(&(s.prefetch_window_end_line + 1)).map(|c| c.main_axis_size)
                    else {
                        break;
                    };
                    s.prefetch_window_end_line += 1;
                    s.prefetch_window_end_extra_space -= size;
                }
                s.prefetch_window_end_line
            };
            self.remove_out_of_bounds_items(end_line + 1, items_count - 1);
        }
    }

    /// Line `index`'s size if it is cached, else its prefetch scheduled (if it isn't) and none yet (Compose's
    /// `getItemSizeOrPrefetch`).
    fn get_item_size_or_prefetch(&self, scope: &Rc<dyn CacheWindowScope>, index: i32, is_urgent: bool) -> i32 {
        let cached = self.state.borrow().window_cache_with_items.get(&index).map(|c| c.main_axis_size);
        if let Some(size) = cached {
            return size;
        }
        let scheduled = self.state.borrow().prefetch_window_handles.get(&index).cloned();
        match scheduled {
            Some(handles) => {
                // (scheduled, not done yet)
                if is_urgent {
                    for h in &handles {
                        h.mark_as_urgent();
                    }
                }
            }
            None => {
                let handles = scope.schedule_prefetch(index, self.on_item_prefetched_callback(scope));
                if is_urgent {
                    for h in &handles {
                        h.mark_as_urgent();
                    }
                }
                self.state.borrow_mut().prefetch_window_handles.insert(index, handles);
            }
        }
        InvalidItemSize
    }

    fn on_item_prefetched_callback(&self, scope: &Rc<dyn CacheWindowScope>) -> Rc<dyn Fn(i32, i32)> {
        let me = self.me.clone();
        let scope = Rc::downgrade(scope);
        Rc::new(move |index, size| {
            if let (Some(me), Some(scope)) = (me.upgrade(), scope.upgrade()) {
                me.on_item_prefetched(&scope, index, size);
            }
        })
    }

    fn cache_prefetched_item(&self, index: i32, size: i32) {
        let mut s = self.state.borrow_mut();
        let item = match s.window_cache_with_items.remove(&index) {
            Some(mut c) => {
                c.main_axis_size = size;
                c.key = None;
                c
            }
            None => CachedItem { key: None, main_axis_size: size },
        };
        s.window_cache_with_items.insert(index, item);
        if index > s.prefetch_window_end_line {
            s.prefetch_window_end_line = index;
            s.prefetch_window_end_extra_space -= size;
        } else if index < s.prefetch_window_start_line {
            s.prefetch_window_start_line = index;
            s.prefetch_window_start_extra_space -= size;
        }
    }

    fn cache_visible_items_info(&self, index: i32, key: LazyKey, size: i32) {
        let handles = {
            let mut s = self.state.borrow_mut();
            if let Some(cached) = s.window_cache_with_items.get(&index)
                && (cached.main_axis_size != size || cached.key.as_ref() != Some(&key))
            {
                s.should_refill_window = true;
            }
            s.window_cache_with_items.insert(index, CachedItem { key: Some(key), main_axis_size: size });
            // (a visible item cached: its handle no longer needed)
            s.prefetch_window_start_line = s.prefetch_window_start_line.min(index);
            s.prefetch_window_end_line = s.prefetch_window_end_line.max(index);
            s.prefetch_window_handles.remove(&index)
        };
        for h in handles.iter().flatten() {
            h.cancel();
        }
    }

    fn remove_out_of_bounds_items(&self, start_line: i32, end_line: i32) {
        let removed: Vec<Vec<PrefetchHandle>> = {
            let mut s = self.state.borrow_mut();
            let in_range = |i: &i32| *i >= start_line && *i <= end_line;
            let mut indices: HashSet<i32> = s.prefetch_window_handles.keys().copied().filter(in_range).collect();
            indices.extend(s.window_cache_with_items.keys().copied().filter(in_range));
            let mut removed = Vec::new();
            for i in indices {
                if let Some(h) = s.prefetch_window_handles.remove(&i) {
                    removed.push(h);
                }
                s.window_cache_with_items.remove(&i);
            }
            removed
        };
        for h in removed.iter().flatten() {
            h.cancel();
        }
    }

    fn on_item_prefetched(&self, scope: &Rc<dyn CacheWindowScope>, index: i32, item_size: i32) {
        self.cache_prefetched_item(index, item_size);
        self.schedule_next_item_if_needed(scope);
    }

    fn schedule_next_item_if_needed(&self, scope: &Rc<dyn CacheWindowScope>) {
        let next = {
            let s = self.state.borrow();
            if sign(s.previous_pass_delta) <= 0.0 {
                // (scrolling forward)
                if s.prefetch_window_end_extra_space > 0 { s.prefetch_window_end_line + 1 } else { -1 }
            } else if s.prefetch_window_start_extra_space > 0 {
                s.prefetch_window_start_line - 1
            } else {
                -1
            }
        };
        let items_count = self.state.borrow().items_count;
        if next > 0 {
            let last_in_line = scope.get_last_index_in_line(next);
            if last_in_line != InvalidIndex && last_in_line < items_count {
                let handles = scope.schedule_prefetch(next, self.on_item_prefetched_callback(scope));
                self.state.borrow_mut().prefetch_window_handles.insert(next, handles);
            }
        }
    }
}

/// Kotlin's `Float.sign`: -1, 0 or 1 (0 for 0 and -0).
fn sign(v: f32) -> f32 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    }
}
