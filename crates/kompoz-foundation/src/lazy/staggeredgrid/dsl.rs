//! `LazyVerticalStaggeredGrid` and `LazyHorizontalStaggeredGrid` and what their block builds (Compose's
//! LazyStaggeredGridDsl.kt, LazyStaggeredGridIntervalContent.kt and LazyStaggeredGridSpan.kt's span provider): the
//! items, as intervals — one item, a count of them, a list's — each with its keys, content types, spans and
//! content.

use super::cells::{LazyStaggeredGridSlots, LazyStaggeredGridSlotsProvider, StaggeredGridCells, StaggeredGridItemSpan};
use super::grid::LazyStaggeredGrid;
use super::state::{LazyStaggeredGridState, remember_lazy_staggered_grid_state};
use crate::gestures::{FlingBehavior, Orientation, ScrollableDefaults};
use crate::layout::{Arrangement, HorizontalArrangement, PaddingValues, VerticalArrangement};
use crate::lazy::LazyListItems;
use crate::lazy::layout::{IntervalList, LazyKey, LazyLayoutInterval, LazyLayoutIntervalContent, MutableIntervalList};
use crate::overscroll::{OverscrollEffect, remember_overscroll_effect};
use kompoz_graphics::{Constraints, Dp, LayoutDirection};
use kompoz_runtime::{kompozable, named, remember_keyed};
use kompoz_ui::Modifier;
use kompoz_ui::layout::SlotKey;
use std::rc::Rc;

/// An interval of a staggered grid's content (Compose's internal LazyStaggeredGridInterval): its keys, content
/// types, spans and items' content, by the index in it.
#[doc(hidden)]
pub struct LazyStaggeredGridInterval {
    pub(crate) key: Option<Rc<dyn Fn(i32) -> LazyKey>>,
    pub(crate) content_type: Rc<dyn Fn(i32) -> Option<SlotKey>>,
    pub(crate) span: Option<Rc<dyn Fn(i32) -> StaggeredGridItemSpan>>,
    pub(crate) item: Rc<dyn Fn(i32)>,
}

impl LazyLayoutInterval for LazyStaggeredGridInterval {
    fn key(&self, index: i32) -> Option<LazyKey> {
        self.key.as_ref().map(|k| k(index))
    }
    fn content_type(&self, index: i32) -> Option<SlotKey> {
        (self.content_type)(index)
    }
}

/// What a `LazyVerticalStaggeredGrid`'s or `LazyHorizontalStaggeredGrid`'s block adds its items to (Compose's
/// LazyStaggeredGridScope, the interval content it builds): `item`, `items` (a count), `items_list` and
/// `items_indexed` (a list's). An item goes under the shortest lane, or takes the whole line
/// ([`StaggeredGridItemSpan::FullLine`]); its key keeps its state and place as the items change; its content type
/// lets an item reuse the composition of an item of the same type that left.
#[derive(Default)]
pub struct LazyStaggeredGridScope {
    pub(crate) intervals: MutableIntervalList<LazyStaggeredGridInterval>,
}

impl LazyLayoutIntervalContent for LazyStaggeredGridScope {
    type Interval = LazyStaggeredGridInterval;
    fn intervals(&self) -> &MutableIntervalList<LazyStaggeredGridInterval> {
        &self.intervals
    }
}

impl LazyStaggeredGridScope {
    /// The content `content` builds (Compose's `LazyStaggeredGridIntervalContent(content)`).
    pub(crate) fn of(content: &dyn Fn(&mut LazyStaggeredGridScope)) -> LazyStaggeredGridScope {
        let mut scope = LazyStaggeredGridScope::default();
        content(&mut scope);
        scope
    }

    /// Whether item `item_index` takes the whole line (Compose's LazyStaggeredGridSpanProvider's `isFullSpan`).
    pub(crate) fn is_full_span(&self, item_index: i32) -> bool {
        if item_index < 0 || item_index >= self.intervals.size() {
            return false;
        }
        let interval = self.intervals.get(item_index);
        let local_index = item_index - interval.start_index;
        interval.value.span.as_ref().is_some_and(|span| span(local_index) == StaggeredGridItemSpan::FullLine)
    }

    /// Adds `count` items of an interval (Compose's `items(count)`'s body).
    fn add_items(
        &mut self,
        count: i32,
        key: Option<Rc<dyn Fn(i32) -> LazyKey>>,
        content_type: Option<Rc<dyn Fn(i32) -> Option<SlotKey>>>,
        span: Option<Rc<dyn Fn(i32) -> StaggeredGridItemSpan>>,
        item_content: Rc<dyn Fn(i32)>,
    ) {
        self.intervals.add_interval(
            count,
            LazyStaggeredGridInterval {
                key,
                content_type: content_type.unwrap_or_else(|| Rc::new(|_| None)),
                span,
                item: item_content,
            },
        );
    }
}

#[named]
impl LazyStaggeredGridScope {
    /// One item (Compose's `item`): of `key` (else its index's), `content_type`, `span` (a lane when none).
    pub fn item(
        &mut self,
        #[default(None)] key: Option<LazyKey>,
        #[default(None)] content_type: Option<SlotKey>,
        #[default(None)] span: Option<StaggeredGridItemSpan>,
        content: impl Fn() + 'static,
    ) {
        self.add_items(
            1,
            key.map(|k| Rc::new(move |_: i32| k.clone()) as Rc<dyn Fn(i32) -> LazyKey>),
            Some(Rc::new(move |_| content_type.clone())),
            span.map(|span| Rc::new(move |_: i32| span) as Rc<dyn Fn(i32) -> StaggeredGridItemSpan>),
            Rc::new(move |_| content()),
        );
    }

    /// `count` items (Compose's `items(count)`): each one's key, content type and span by its index (none: its
    /// index's key, no type, a lane), its content given its index.
    pub fn items(
        &mut self,
        count: i32,
        #[default(None)] key: Option<Rc<dyn Fn(i32) -> LazyKey>>,
        #[default(None)] content_type: Option<Rc<dyn Fn(i32) -> Option<SlotKey>>>,
        #[default(None)] span: Option<Rc<dyn Fn(i32) -> StaggeredGridItemSpan>>,
        item_content: impl Fn(i32) + 'static,
    ) {
        self.add_items(count, key, content_type, span, Rc::new(item_content));
    }

    /// A list's items (Compose's `items(items: List<T>)`): each one's key, content type and span by its element,
    /// its content given its element.
    /// Kotlin: `items(items: List<T>, key: ((item: T) -> Any)? = null, contentType: (item: T) -> Any? = { null }, span: ((item: T) -> StaggeredGridItemSpan)? = null, itemContent: @Composable LazyStaggeredGridItemScope.(item: T) -> Unit)`
    pub fn items_list<T: 'static>(
        &mut self,
        items: impl LazyListItems<T>,
        #[default(None)] key: Option<Rc<dyn for<'a> Fn(&'a T) -> LazyKey>>,
        #[default(None)] content_type: Option<Rc<dyn for<'a> Fn(&'a T) -> Option<SlotKey>>>,
        #[default(None)] span: Option<Rc<dyn for<'a> Fn(&'a T) -> StaggeredGridItemSpan>>,
        item_content: impl for<'a> Fn(&'a T) + 'static,
    ) {
        let items = items.into_items();
        let count = items.len() as i32;
        let key = key.map(|key| {
            let items = items.clone();
            Rc::new(move |index: i32| key(&items[index as usize])) as Rc<dyn Fn(i32) -> LazyKey>
        });
        let content_type: Rc<dyn Fn(i32) -> Option<SlotKey>> = match content_type {
            Some(content_type) => {
                let items = items.clone();
                Rc::new(move |index| content_type(&items[index as usize]))
            }
            None => Rc::new(|_| None),
        };
        let span = span.map(|span| {
            let items = items.clone();
            Rc::new(move |index: i32| span(&items[index as usize])) as Rc<dyn Fn(i32) -> StaggeredGridItemSpan>
        });
        self.add_items(
            count,
            key,
            Some(content_type),
            span,
            Rc::new(move |index| item_content(&items[index as usize])),
        );
    }

    /// A list's items with their indices (Compose's `itemsIndexed`).
    /// Kotlin: `itemsIndexed(items: List<T>, key: ((index: Int, item: T) -> Any)? = null, contentType: (index: Int, item: T) -> Any? = { _, _ -> null }, span: ((index: Int, item: T) -> StaggeredGridItemSpan)? = null, itemContent: @Composable LazyStaggeredGridItemScope.(index: Int, item: T) -> Unit)`
    pub fn items_indexed<T: 'static>(
        &mut self,
        items: impl LazyListItems<T>,
        #[default(None)] key: Option<Rc<dyn for<'a> Fn(i32, &'a T) -> LazyKey>>,
        #[default(None)] content_type: Option<Rc<dyn for<'a> Fn(i32, &'a T) -> Option<SlotKey>>>,
        #[default(None)] span: Option<Rc<dyn for<'a> Fn(i32, &'a T) -> StaggeredGridItemSpan>>,
        item_content: impl for<'a> Fn(i32, &'a T) + 'static,
    ) {
        let items = items.into_items();
        let count = items.len() as i32;
        let key = key.map(|key| {
            let items = items.clone();
            Rc::new(move |index: i32| key(index, &items[index as usize])) as Rc<dyn Fn(i32) -> LazyKey>
        });
        let content_type: Rc<dyn Fn(i32) -> Option<SlotKey>> = match content_type {
            Some(content_type) => {
                let items = items.clone();
                Rc::new(move |index| content_type(index, &items[index as usize]))
            }
            None => Rc::new(|_| None),
        };
        let span = span.map(|span| {
            let items = items.clone();
            Rc::new(move |index: i32| span(index, &items[index as usize])) as Rc<dyn Fn(i32) -> StaggeredGridItemSpan>
        });
        self.add_items(
            count,
            key,
            Some(content_type),
            span,
            Rc::new(move |index| item_content(index, &items[index as usize])),
        );
    }
}

/// A staggered grid of `columns` scrolling vertically (Compose's `LazyVerticalStaggeredGrid`): each item goes under
/// the shortest column (a full-line one under the longest, across them all), only those in view composed and laid
/// out; its `content` block adds the items ([`LazyStaggeredGridScope`]). `state` (remembered by default) scrolls it
/// and tells where it is; `content_padding` pads the content; `reverse_layout` puts item 0 at the bottom;
/// `vertical_item_spacing` is the space between items of a column, `horizontal_arrangement`'s spacing the space
/// between columns; `fling_behavior`, `user_scroll_enabled`, `overscroll_effect` as `LazyColumn`.
#[kompozable]
pub fn LazyVerticalStaggeredGrid(
    columns: StaggeredGridCells,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] state: Option<LazyStaggeredGridState>,
    #[default(PaddingValues::new(Dp(0.0), Dp(0.0), Dp(0.0), Dp(0.0)))] content_padding: PaddingValues,
    #[default(false)] reverse_layout: bool,
    #[default(Dp(0.0))] vertical_item_spacing: Dp,
    #[default(Arrangement::spaced_by(Dp(0.0)))] horizontal_arrangement: HorizontalArrangement,
    #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
    #[default(true)] user_scroll_enabled: bool,
    #[default(None)] overscroll_effect: Option<Option<Rc<dyn OverscrollEffect>>>,
    content: impl Fn(&mut LazyStaggeredGridScope) + 'static,
) {
    let state = match state {
        Some(state) => state,
        None => remember_lazy_staggered_grid_state(),
    };
    let fling_behavior = match fling_behavior {
        Some(f) => f,
        None => ScrollableDefaults::fling_behavior(),
    };
    let overscroll_effect = match overscroll_effect {
        Some(e) => e,
        None => remember_overscroll_effect(),
    };
    // (Compose's `rememberColumnSlots`)
    let slots = {
        let (c, h, p) = (columns.clone(), horizontal_arrangement.clone(), content_padding.clone());
        remember_keyed((columns, horizontal_arrangement.clone(), content_padding.clone()), move || {
            let (columns, horizontal_arrangement, content_padding) = (c.clone(), h.clone(), p.clone());
            Rc::new(LazyStaggeredGridSlotsProvider::new(move |density, constraints| {
                assert!(
                    constraints.max_width != Constraints::Infinity,
                    "LazyVerticalStaggeredGrid's width should be bound by parent."
                );
                let horizontal_padding = content_padding.calculate_start_padding(LayoutDirection::Ltr)
                    + content_padding.calculate_end_padding(LayoutDirection::Ltr);
                let grid_width = constraints.max_width - density.round_to_px(horizontal_padding);
                let sizes = columns.calculate_cross_axis_cell_sizes(
                    density,
                    grid_width,
                    density.round_to_px(horizontal_arrangement.spacing()),
                );
                let mut positions = vec![0; sizes.len()];
                // (left to right: the placement mirrors)
                horizontal_arrangement.arrange(density, grid_width, &sizes, LayoutDirection::Ltr, &mut positions);
                LazyStaggeredGridSlots { positions, sizes }
            }))
        })
    };
    LazyStaggeredGrid(
        state,
        Orientation::Vertical,
        slots,
        modifier,
        content_padding,
        reverse_layout,
        fling_behavior,
        user_scroll_enabled,
        overscroll_effect,
        vertical_item_spacing,
        horizontal_arrangement.spacing(),
        Rc::new(content),
    );
}

/// A staggered grid of `rows` scrolling horizontally (Compose's `LazyHorizontalStaggeredGrid`): as
/// [`LazyVerticalStaggeredGrid`], along its width (right to left mirrored); `horizontal_item_spacing` is the space
/// between items of a row, `vertical_arrangement`'s spacing the space between rows.
#[kompozable]
pub fn LazyHorizontalStaggeredGrid(
    rows: StaggeredGridCells,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] state: Option<LazyStaggeredGridState>,
    #[default(PaddingValues::new(Dp(0.0), Dp(0.0), Dp(0.0), Dp(0.0)))] content_padding: PaddingValues,
    #[default(false)] reverse_layout: bool,
    #[default(Arrangement::spaced_by(Dp(0.0)))] vertical_arrangement: VerticalArrangement,
    #[default(Dp(0.0))] horizontal_item_spacing: Dp,
    #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
    #[default(true)] user_scroll_enabled: bool,
    #[default(None)] overscroll_effect: Option<Option<Rc<dyn OverscrollEffect>>>,
    content: impl Fn(&mut LazyStaggeredGridScope) + 'static,
) {
    let state = match state {
        Some(state) => state,
        None => remember_lazy_staggered_grid_state(),
    };
    let fling_behavior = match fling_behavior {
        Some(f) => f,
        None => ScrollableDefaults::fling_behavior(),
    };
    let overscroll_effect = match overscroll_effect {
        Some(e) => e,
        None => remember_overscroll_effect(),
    };
    // (Compose's `rememberRowSlots`)
    let slots = {
        let (r, v, p) = (rows.clone(), vertical_arrangement.clone(), content_padding.clone());
        remember_keyed((rows, vertical_arrangement.clone(), content_padding.clone()), move || {
            let (rows, vertical_arrangement, content_padding) = (r.clone(), v.clone(), p.clone());
            Rc::new(LazyStaggeredGridSlotsProvider::new(move |density, constraints| {
                assert!(
                    constraints.max_height != Constraints::Infinity,
                    "LazyHorizontalStaggeredGrid's height should be bound by parent."
                );
                let vertical_padding =
                    content_padding.calculate_top_padding() + content_padding.calculate_bottom_padding();
                let grid_height = constraints.max_height - density.round_to_px(vertical_padding);
                let sizes = rows.calculate_cross_axis_cell_sizes(
                    density,
                    grid_height,
                    density.round_to_px(vertical_arrangement.spacing()),
                );
                let mut positions = vec![0; sizes.len()];
                vertical_arrangement.arrange(density, grid_height, &sizes, &mut positions);
                LazyStaggeredGridSlots { positions, sizes }
            }))
        })
    };
    LazyStaggeredGrid(
        state,
        Orientation::Horizontal,
        slots,
        modifier,
        content_padding,
        reverse_layout,
        fling_behavior,
        user_scroll_enabled,
        overscroll_effect,
        horizontal_item_spacing,
        vertical_arrangement.spacing(),
        Rc::new(content),
    );
}
