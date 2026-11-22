//! A carousel's state and what its items know as they are drawn (Compose's `CarouselState.kt`).

use kompoz_animation::{AnimationSpec, MutatePriority, animate, spring};
use kompoz_foundation::gestures::{LocalFuture, Orientation, ScrollBlock, ScrollScope, ScrollableState};
use kompoz_foundation::lazy::layout::LazyLayoutScrollScope;
use kompoz_foundation::pager::PagerState;
use kompoz_graphics::Rect;
use kompoz_runtime::{
    MutableState, SavedValue, Saver, kompozable, mutable_float_state_of, mutable_state_of, named, named_args,
    remember_saveable_with, without_read_observation,
};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------
// CarouselState

/// The state of every kind of carousel (Compose's `CarouselState`), a [`ScrollableState`]: the item closest to its
/// snapped position ([`current_item`](CarouselState::current_item)), scrolling to an item at once or animated. Made
/// by [`CarouselState()`](fn@CarouselState), or remembered and saved by [`remember_carousel_state`]. Clones are the
/// same state; two are equal when they are.
#[derive(Clone)]
pub struct CarouselState {
    inner: Rc<CarouselStateInner>,
}

struct CarouselStateInner {
    pager_state: CarouselPagerState,
}

impl PartialEq for CarouselState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for CarouselState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "CarouselState(currentItem={})",
            without_read_observation(|| self.inner.pager_state.pager_state.current_page())
        )
    }
}

/// A carousel's state at `current_item`, `current_item_offset_fraction` of an item from its snapped position (-0.5
/// to 0.5), of the items `item_count` tells (Compose's `CarouselState(currentItem, currentItemOffsetFraction,
/// itemCount)`).
/// Kotlin: `CarouselState(currentItem: Int = 0, currentItemOffsetFraction: Float = 0f, itemCount: () -> Int)`
pub fn CarouselState(
    current_item: i32,
    current_item_offset_fraction: f32,
    item_count: impl Fn() -> i32 + 'static,
) -> CarouselState {
    CarouselState::new(current_item, current_item_offset_fraction, Rc::new(item_count))
}

impl CarouselState {
    /// A state at `current_item`, of the items `item_count` tells.
    pub fn new(current_item: i32, current_item_offset_fraction: f32, item_count: Rc<dyn Fn() -> i32>) -> CarouselState {
        CarouselState {
            inner: Rc::new(CarouselStateInner {
                pager_state: CarouselPagerState::new(current_item, current_item_offset_fraction, item_count),
            }),
        }
    }

    /// The pager the carousel lays its items out with (Compose's internal `pagerState`).
    pub(crate) fn pager_state(&self) -> &CarouselPagerState {
        &self.inner.pager_state
    }

    /// The item closest to the snapped position (a state; it changes as the carousel scrolls by gesture or
    /// animation; Compose's `currentItem`, the pager's `currentPage`).
    pub fn current_item(&self) -> i32 {
        self.inner.pager_state.pager_state.current_page()
    }

    /// Jumps to `item` at once (Compose's `scrollToItem`).
    pub fn scroll_to_item(&self, item: i32) -> impl Future<Output = ()> + 'static {
        named_args!(self.inner.pager_state.pager_state.scroll_to_page(item, page_offset_fraction = 0.0))
    }

    /// The saver of [`remember_carousel_state`] (Compose's `CarouselState.Saver`): the current item, its offset
    /// fraction and the item count (restored as a count that doesn't change until composed again).
    pub fn saver() -> Saver<CarouselState> {
        Saver::new(
            |s: &CarouselState| {
                let pager = &s.inner.pager_state.pager_state;
                SavedValue::list([
                    SavedValue::from(without_read_observation(|| pager.current_page())),
                    SavedValue::from(without_read_observation(|| pager.current_page_offset_fraction())),
                    SavedValue::from(without_read_observation(|| pager.page_count())),
                ])
            },
            |v| {
                let list = v.as_list()?;
                let item = list.first()?.as_int()? as i32;
                let fraction = list.get(1)?.as_f64()? as f32;
                let count = list.get(2)?.as_int()? as i32;
                Some(CarouselState::new(item, fraction, Rc::new(move || count)))
            },
        )
    }
}

#[named]
impl CarouselState {
    /// Glides to `item` along `animation_spec` (Compose's `animateScrollToItem`): an item too far from the current
    /// one is jumped close to first, so the items between aren't all composed; the distance takes each item's snap
    /// offset into account (items snap at different places where the keylines shift).
    pub fn animate_scroll_to_item(
        &self,
        item: i32,
        #[default(Arc::new(spring::<f32>()))] animation_spec: Arc<dyn AnimationSpec<f32>>,
    ) -> impl Future<Output = ()> + 'static {
        let pager = self.inner.pager_state.pager_state.clone();
        async move {
            if (item == pager.current_page() && pager.current_page_offset_fraction() == 0.0) || pager.page_count() == 0
            {
                return;
            }

            let page_count = pager.page_count();
            let target_page = if page_count > 0 { item.clamp(0, page_count - 1) } else { 0 };
            let p = pager.clone();
            pager
                .scroll_block(
                    MutatePriority::Default,
                    Box::new(move |scope: Rc<dyn ScrollScope>| {
                        Box::pin(async move {
                            // (the target page updated as it goes)
                            let lazy_scope = LazyLayoutScrollScope(&p, scope.clone());
                            animate_scroll_to_page(lazy_scope, &p, target_page, 0.0, animation_spec, |page| {
                                p.update_target_page(&*scope, page)
                            })
                            .await;
                        })
                    }),
                )
                .await;
        }
    }
}

impl ScrollableState for CarouselState {
    fn scroll_block(&self, scroll_priority: MutatePriority, block: ScrollBlock) -> LocalFuture<'_, ()> {
        self.inner.pager_state.pager_state.scroll_block(scroll_priority, block)
    }

    fn dispatch_raw_delta(&self, delta: f32) -> f32 {
        self.inner.pager_state.pager_state.dispatch_raw_delta(delta)
    }

    fn is_scroll_in_progress(&self) -> bool {
        self.inner.pager_state.pager_state.is_scroll_in_progress()
    }
}

/// A [`CarouselState`] remembered and saved where it is called (Compose's `rememberCarouselState`): at
/// `initial_item`, of the items `item_count` tells (the function the latest given).
/// Kotlin: `rememberCarouselState(initialItem: Int = 0, itemCount: () -> Int)`
#[kompozable]
pub fn remember_carousel_state(
    #[default(0)] initial_item: i32,
    item_count: impl Fn() -> i32 + 'static,
) -> CarouselState {
    let item_count: Rc<dyn Fn() -> i32> = Rc::new(item_count);
    let first = item_count.clone();
    let state = remember_saveable_with(&CarouselState::saver(), move || CarouselState::new(initial_item, 0.0, first));
    state.inner.pager_state.page_count_state.set(item_count);
    state
}

// ---------------------------------------------------------------------------------------------------------------
// CarouselPagerState

/// The least and the most a page's offset fraction is saved as.
pub(crate) const MinPageOffset: f32 = -0.5;
pub(crate) const MaxPageOffset: f32 = 0.5;

/// A pager state whose page count is a state of the latest count function (Compose's internal
/// `CarouselPagerState`, a `PagerState` subclass overriding `pageCount` to read its `pageCountState`). KompozR's
/// `PagerState` isn't open: this holds one made to read the count through `page_count_state`.
pub(crate) struct CarouselPagerState {
    pub(crate) pager_state: PagerState,
    pub(crate) page_count_state: PageCountState,
}

impl CarouselPagerState {
    pub(crate) fn new(
        current_page: i32,
        current_page_offset_fraction: f32,
        updated_page_count: Rc<dyn Fn() -> i32>,
    ) -> CarouselPagerState {
        let page_count_state = PageCountState::new(updated_page_count);
        let count = page_count_state.clone();
        let pager_state = PagerState::new(current_page, current_page_offset_fraction, Rc::new(move || count.get()()));
        CarouselPagerState { pager_state, page_count_state }
    }

    /// Compose's `CarouselPagerState.Saver`: the current page, its fraction within -0.5..0.5 and the count (Compose
    /// saves the count function itself and restores it cast to `Int`; the count is saved here). Unused, as Compose's.
    #[allow(dead_code)]
    pub(crate) fn saver() -> Saver<Rc<CarouselPagerState>> {
        Saver::new(
            |s: &Rc<CarouselPagerState>| {
                SavedValue::list([
                    SavedValue::from(without_read_observation(|| s.pager_state.current_page())),
                    SavedValue::from(
                        without_read_observation(|| s.pager_state.current_page_offset_fraction())
                            .clamp(MinPageOffset, MaxPageOffset),
                    ),
                    SavedValue::from(without_read_observation(|| s.page_count_state.get()())),
                ])
            },
            |v| {
                let list = v.as_list()?;
                let page = list.first()?.as_int()? as i32;
                let fraction = list.get(1)?.as_f64()? as f32;
                let count = list.get(2)?.as_int()? as i32;
                Some(Rc::new(CarouselPagerState::new(page, fraction, Rc::new(move || count))))
            },
        )
    }
}

/// Compose's `pageCountState = mutableStateOf(updatedPageCount)`: the latest count function as a state. A function
/// isn't `Send`, so it is kept beside a state of its version; readers are told when a new function gives another
/// count than the last one (as KompozR's `PagerState` page count: Compose's state of a lambda changes with each new
/// lambda, which a fresh Rust closure at every composition would make a write at every composition).
#[derive(Clone)]
pub(crate) struct PageCountState {
    value: Rc<RefCell<Rc<dyn Fn() -> i32>>>,
    generation: MutableState<u64>,
}

impl PageCountState {
    fn new(page_count: Rc<dyn Fn() -> i32>) -> PageCountState {
        PageCountState { value: Rc::new(RefCell::new(page_count)), generation: mutable_state_of(0) }
    }

    /// The count function (a state read; Compose's `pageCountState.value`).
    pub(crate) fn get(&self) -> Rc<dyn Fn() -> i32> {
        self.generation.get();
        self.value.borrow().clone()
    }

    /// A new count function (Compose's `pageCountState.value = …`).
    pub(crate) fn set(&self, page_count: Rc<dyn Fn() -> i32>) {
        let old = self.value.replace(page_count.clone());
        let changed = without_read_observation(|| old() != page_count());
        if changed {
            self.generation.set(self.generation.peek().wrapping_add(1));
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Animated scroll

/// How many pages away an animated scroll jumps closer first.
const MaxPagesForAnimateScroll: i32 = 3;

/// Glides a carousel's scroll scope to `target_page` (Compose's private `LazyLayoutScrollScope.animateScrollToPage`
/// of the carousel): unlike the pager's, the distance includes the difference of the current and target pages'
/// snap offsets (keylines shifting make them differ), plus `target_page_offset_to_snapped_position`.
async fn animate_scroll_to_page(
    scope: Rc<dyn LazyLayoutScrollScope>,
    pager_state: &PagerState,
    target_page: i32,
    target_page_offset_to_snapped_position: f32,
    animation_spec: Arc<dyn AnimationSpec<f32>>,
    update_target_page: impl Fn(i32),
) {
    update_target_page(target_page);
    let first = scope.first_visible_item_index();
    let last = scope.last_visible_item_index();
    let forward = target_page > first;
    let visible_pages = last - first + 1;
    if ((forward && target_page > last) || (!forward && target_page < first))
        && (target_page - first).abs() >= MaxPagesForAnimateScroll
    {
        // A viewport away from the destination first, if it can.
        let pre_jump_position =
            if forward { (target_page - visible_pages).max(first) } else { (target_page + visible_pages).min(first) };
        scope.snap_to_item(pre_jump_position, 0);
    }

    // The pages' distance less what the first one scrolled, plus the offset asked for, plus the snap offsets'
    // difference.
    let displacement = calculate_scroll_distance_to(pager_state, pager_state.current_page(), target_page)
        + target_page_offset_to_snapped_position;

    let previous_value = Rc::new(Cell::new(0.0f32));
    let (s, p) = (scope.clone(), previous_value);
    named_args!(animate(
        0.0,
        displacement,
        animation_spec = animation_spec,
        block = move |current_value: f32, _velocity: f32| {
            let delta = current_value - p.get();
            let consumed = s.scroll_by(delta);
            p.set(p.get() + consumed);
        }
    ))
    .await;
}

/// The distance from `current_page`'s snapped position to `target_page`'s (Compose's private
/// `PagerState.calculateScrollDistanceTo`): the pages between at the page size and spacing, plus the difference of
/// their snap offsets.
fn calculate_scroll_distance_to(state: &PagerState, current_page: i32, target_page: i32) -> f32 {
    let layout_info = state.layout_info();
    let layout_size = if layout_info.orientation() == Orientation::Horizontal {
        layout_info.viewport_size().width
    } else {
        layout_info.viewport_size().height
    };
    let page_count = state.page_count();
    let snap_position = layout_info.snap_position();
    // The snap offsets of the current and target pages.
    let current_page_snap_offset = snap_position.position(
        layout_size,
        layout_info.page_size(),
        layout_info.before_content_padding(),
        layout_info.after_content_padding(),
        current_page,
        page_count,
    );
    let target_page_snap_offset = snap_position.position(
        layout_size,
        layout_info.page_size(),
        layout_info.before_content_padding(),
        layout_info.after_content_padding(),
        target_page,
        page_count,
    );
    let snap_offset_diff = current_page_snap_offset - target_page_snap_offset;
    let target_page_diff = target_page - current_page;
    let page_size_with_spacing = layout_info.page_size() as f32 + layout_info.page_spacing() as f32;
    (target_page_diff as f32 * page_size_with_spacing) + snap_offset_diff as f32
}

// ---------------------------------------------------------------------------------------------------------------
// CarouselItemDrawInfo

/// What a carousel item knows of its size as it is drawn (Compose's sealed interface `CarouselItemDrawInfo` and its
/// one implementation `CarouselItemDrawInfoImpl`: a value here, its properties states): its size along the main
/// axis, the least and most it gets, and the rect it is masked to, in pixels. They change as the carousel scrolls:
/// read them where items draw or lay out, not in a composition (which would recompose at every change).
#[derive(Clone)]
pub struct CarouselItemDrawInfo {
    pub(crate) size_state: MutableState<f32>,
    pub(crate) min_size_state: MutableState<f32>,
    pub(crate) max_size_state: MutableState<f32>,
    pub(crate) mask_rect_state: MutableState<Rect>,
}

impl PartialEq for CarouselItemDrawInfo {
    fn eq(&self, other: &Self) -> bool {
        self.size_state == other.size_state
    }
}

impl fmt::Debug for CarouselItemDrawInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CarouselItemDrawInfo")
            .field("size", &self.size_state.peek())
            .field("min_size", &self.min_size_state.peek())
            .field("max_size", &self.max_size_state.peek())
            .field("mask_rect", &self.mask_rect_state.peek())
            .finish()
    }
}

impl CarouselItemDrawInfo {
    pub(crate) fn new() -> CarouselItemDrawInfo {
        CarouselItemDrawInfo {
            size_state: mutable_float_state_of(0.0),
            min_size_state: mutable_float_state_of(0.0),
            max_size_state: mutable_float_state_of(0.0),
            mask_rect_state: mutable_state_of(Rect::Zero),
        }
    }

    /// The item's size along the main axis, px (Compose's `size`).
    pub fn size(&self) -> f32 {
        self.size_state.get()
    }

    /// The least it is along the main axis (as it scrolls off the sides), px (Compose's `minSize`).
    pub fn min_size(&self) -> f32 {
        self.min_size_state.get()
    }

    /// The most it is along the main axis (at a focal position), px (Compose's `maxSize`).
    pub fn max_size(&self) -> f32 {
        self.max_size_state.get()
    }

    /// The rect it is clipped by (Compose's `maskRect`).
    pub fn mask_rect(&self) -> Rect {
        self.mask_rect_state.get()
    }
}
