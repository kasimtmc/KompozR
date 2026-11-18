//! `LazyVerticalGrid` and `LazyHorizontalGrid` and what their block builds (Compose's LazyGridDsl.kt and
//! LazyGridIntervalContent.kt): the items, as intervals — one item, a count of them, a list's, a sticky header —
//! each with its keys, spans, content types and content.
//!
//! ```ignore
//! LazyVerticalGrid(columns = GridCells::Fixed(3), content = move |this: &mut LazyGridScope| {
//!     this.item(span = Some(Rc::new(|s: &LazyGridItemSpanScope| GridItemSpan(s.max_line_span()))), content = || Text("Başlık"));
//!     this.items(100, item_content = |i| Text(format!("{i}")));
//! });
//! ```

use super::cells::{GridCells, GridItemSpan, LazyGridItemSpanScope, LazyGridSlotsProvider};
use super::grid::LazyGrid;
use super::span_layout_provider::{LazyGridSpanLayoutProvider, LineConfiguration};
use super::state::{LazyGridState, remember_lazy_grid_state};
use crate::gestures::{FlingBehavior, ScrollableDefaults};
use crate::layout::{Arrangement, HorizontalArrangement, PaddingValues, VerticalArrangement};
use crate::lazy::LazyListItems;
use crate::lazy::layout::{IntervalList, LazyKey, LazyLayoutInterval, LazyLayoutIntervalContent, MutableIntervalList};
use crate::overscroll::{OverscrollEffect, remember_overscroll_effect};
use kompoz_graphics::Dp;
use kompoz_runtime::{kompozable, named, remember_keyed};
use kompoz_ui::Modifier;
use kompoz_ui::layout::SlotKey;
use std::rc::Rc;

/// A span function of an interval's items (Compose's `LazyGridItemSpanScope.(Int) -> GridItemSpan`).
type SpanFn = Rc<dyn for<'s> Fn(&'s LazyGridItemSpanScope, i32) -> GridItemSpan>;

/// An interval of a grid's content (Compose's internal LazyGridInterval): its keys, spans, content types and
/// items' content, by the index in it.
#[doc(hidden)]
pub struct LazyGridInterval {
    pub(crate) key: Option<Rc<dyn Fn(i32) -> LazyKey>>,
    pub(crate) span: SpanFn,
    pub(crate) content_type: Rc<dyn Fn(i32) -> Option<SlotKey>>,
    pub(crate) item: Rc<dyn Fn(i32)>,
}

impl LazyLayoutInterval for LazyGridInterval {
    fn key(&self, index: i32) -> Option<LazyKey> {
        self.key.as_ref().map(|k| k(index))
    }
    fn content_type(&self, index: i32) -> Option<SlotKey> {
        (self.content_type)(index)
    }
}

/// The span of an item without one: a cell.
fn default_span() -> SpanFn {
    Rc::new(|_, _| GridItemSpan(1))
}

/// What a `LazyVerticalGrid`'s or `LazyHorizontalGrid`'s block adds its items to (Compose's LazyGridScope, the
/// interval content it builds): `item`, `items` (a count), `items_list` and `items_indexed` (a list's),
/// `sticky_header`. An item's span is the cells of its line it takes (1 by default; it goes on the next line when
/// the cells left are fewer); its key keeps its state and its place as the items change (by default its index);
/// its content type lets an item reuse the composition of an item of the same type that left.
#[derive(Default)]
pub struct LazyGridScope {
    pub(crate) intervals: MutableIntervalList<LazyGridInterval>,
    pub(crate) has_custom_spans: bool,
    pub(crate) header_indexes: Vec<i32>,
    pub(crate) span_layout_provider: LazyGridSpanLayoutProvider,
}

impl LazyLayoutIntervalContent for LazyGridScope {
    type Interval = LazyGridInterval;
    fn intervals(&self) -> &MutableIntervalList<LazyGridInterval> {
        &self.intervals
    }
}

impl LazyGridScope {
    /// Adds `count` items of an interval (Compose's `items(count)`'s body).
    fn add_items(
        &mut self,
        count: i32,
        key: Option<Rc<dyn Fn(i32) -> LazyKey>>,
        span: Option<SpanFn>,
        content_type: Option<Rc<dyn Fn(i32) -> Option<SlotKey>>>,
        item_content: Rc<dyn Fn(i32)>,
    ) {
        if span.is_some() {
            self.has_custom_spans = true;
        }
        self.intervals.add_interval(
            count,
            LazyGridInterval {
                key,
                span: span.unwrap_or_else(default_span),
                content_type: content_type.unwrap_or_else(|| Rc::new(|_| None)),
                item: item_content,
            },
        );
    }

    /// The content `content` builds (Compose's `LazyGridIntervalContent(content)`).
    pub(crate) fn of(content: &dyn Fn(&mut LazyGridScope)) -> LazyGridScope {
        let mut scope = LazyGridScope::default();
        content(&mut scope);
        scope
    }

    /// The items of line `line_index`.
    pub(crate) fn get_line_configuration(&self, line_index: i32) -> LineConfiguration {
        self.span_layout_provider.get_line_configuration(self, line_index)
    }

    /// The line of item `item_index`.
    pub(crate) fn get_line_index_of_item(&self, item_index: i32) -> i32 {
        self.span_layout_provider.get_line_index_of_item(self, item_index)
    }

    /// Item `item_index`'s span with `max_span` cells left on its line.
    pub(crate) fn span_of(&self, item_index: i32, max_span: i32) -> i32 {
        self.span_layout_provider.span_of(self, item_index, max_span)
    }
}

#[named]
impl LazyGridScope {
    /// One item (Compose's `item`): of `key` (else its index's), `span` (a cell when none), `content_type`.
    pub fn item(
        &mut self,
        #[default(None)] key: Option<LazyKey>,
        #[default(None)] span: Option<Rc<dyn for<'s> Fn(&'s LazyGridItemSpanScope) -> GridItemSpan>>,
        #[default(None)] content_type: Option<SlotKey>,
        content: impl Fn() + 'static,
    ) {
        if span.is_some() {
            self.has_custom_spans = true;
        }
        self.intervals.add_interval(
            1,
            LazyGridInterval {
                key: key.map(|k| Rc::new(move |_: i32| k.clone()) as Rc<dyn Fn(i32) -> LazyKey>),
                span: match span {
                    Some(span) => Rc::new(move |scope, _| span(scope)),
                    None => default_span(),
                },
                content_type: Rc::new(move |_| content_type.clone()),
                item: Rc::new(move |_| content()),
            },
        );
    }

    /// `count` items (Compose's `items(count)`): each one's key, span and content type by its index (none: its
    /// index's key, a cell, no type), its content given its index.
    pub fn items(
        &mut self,
        count: i32,
        #[default(None)] key: Option<Rc<dyn Fn(i32) -> LazyKey>>,
        #[default(None)] span: Option<Rc<dyn for<'s> Fn(&'s LazyGridItemSpanScope, i32) -> GridItemSpan>>,
        #[default(None)] content_type: Option<Rc<dyn Fn(i32) -> Option<SlotKey>>>,
        item_content: impl Fn(i32) + 'static,
    ) {
        self.add_items(count, key, span, content_type, Rc::new(item_content));
    }

    /// A list's items (Compose's `items(items: List<T>)`): each one's key, span and content type by its element,
    /// its content given its element.
    /// Kotlin: `items(items: List<T>, key: ((item: T) -> Any)? = null, span: (LazyGridItemSpanScope.(item: T) -> GridItemSpan)? = null, contentType: (item: T) -> Any? = { null }, itemContent: @Composable LazyGridItemScope.(item: T) -> Unit)`
    pub fn items_list<T: 'static>(
        &mut self,
        items: impl LazyListItems<T>,
        #[default(None)] key: Option<Rc<dyn for<'a> Fn(&'a T) -> LazyKey>>,
        #[default(None)] span: Option<Rc<dyn for<'s, 'a> Fn(&'s LazyGridItemSpanScope, &'a T) -> GridItemSpan>>,
        #[default(None)] content_type: Option<Rc<dyn for<'a> Fn(&'a T) -> Option<SlotKey>>>,
        item_content: impl for<'a> Fn(&'a T) + 'static,
    ) {
        let items = items.into_items();
        let count = items.len() as i32;
        let key = key.map(|key| {
            let items = items.clone();
            Rc::new(move |index: i32| key(&items[index as usize])) as Rc<dyn Fn(i32) -> LazyKey>
        });
        let span = span.map(|span| {
            let items = items.clone();
            Rc::new(move |scope: &LazyGridItemSpanScope, index: i32| span(scope, &items[index as usize])) as SpanFn
        });
        let content_type: Rc<dyn Fn(i32) -> Option<SlotKey>> = match content_type {
            Some(content_type) => {
                let items = items.clone();
                Rc::new(move |index| content_type(&items[index as usize]))
            }
            None => Rc::new(|_| None),
        };
        self.add_items(
            count,
            key,
            span,
            Some(content_type),
            Rc::new(move |index| item_content(&items[index as usize])),
        );
    }

    /// A list's items with their indices (Compose's `itemsIndexed`).
    /// Kotlin: `itemsIndexed(items: List<T>, key: ((index: Int, item: T) -> Any)? = null, span: (LazyGridItemSpanScope.(index: Int, item: T) -> GridItemSpan)? = null, contentType: (index: Int, item: T) -> Any? = { _, _ -> null }, itemContent: @Composable LazyGridItemScope.(index: Int, item: T) -> Unit)`
    pub fn items_indexed<T: 'static>(
        &mut self,
        items: impl LazyListItems<T>,
        #[default(None)] key: Option<Rc<dyn for<'a> Fn(i32, &'a T) -> LazyKey>>,
        #[default(None)] span: Option<Rc<dyn for<'s, 'a> Fn(&'s LazyGridItemSpanScope, i32, &'a T) -> GridItemSpan>>,
        #[default(None)] content_type: Option<Rc<dyn for<'a> Fn(i32, &'a T) -> Option<SlotKey>>>,
        item_content: impl for<'a> Fn(i32, &'a T) + 'static,
    ) {
        let items = items.into_items();
        let count = items.len() as i32;
        let key = key.map(|key| {
            let items = items.clone();
            Rc::new(move |index: i32| key(index, &items[index as usize])) as Rc<dyn Fn(i32) -> LazyKey>
        });
        let span = span.map(|span| {
            let items = items.clone();
            Rc::new(move |scope: &LazyGridItemSpanScope, index: i32| span(scope, index, &items[index as usize]))
                as SpanFn
        });
        let content_type: Rc<dyn Fn(i32) -> Option<SlotKey>> = match content_type {
            Some(content_type) => {
                let items = items.clone();
                Rc::new(move |index| content_type(index, &items[index as usize]))
            }
            None => Rc::new(|_| None),
        };
        self.add_items(
            count,
            key,
            span,
            Some(content_type),
            Rc::new(move |index| item_content(index, &items[index as usize])),
        );
    }

    /// A header that stays at the start of the grid while the items after it scroll under it, until the next one
    /// pushes it out (Compose's `stickyHeader`): it takes a whole line; its content is given its index.
    pub fn sticky_header(
        &mut self,
        #[default(None)] key: Option<LazyKey>,
        #[default(None)] content_type: Option<SlotKey>,
        content: impl Fn(i32) + 'static,
    ) {
        let header_index = self.intervals.size();
        self.header_indexes.push(header_index);
        self.add_items(
            1,
            key.map(|k| Rc::new(move |_: i32| k.clone()) as Rc<dyn Fn(i32) -> LazyKey>),
            Some(Rc::new(|scope: &LazyGridItemSpanScope, _| GridItemSpan(scope.max_line_span()))),
            Some(Rc::new(move |_| content_type.clone())),
            Rc::new(move |_| content(header_index)),
        );
    }
}

/// A grid of `columns` scrolling vertically, composing and laying out only the lines in view (Compose's
/// `LazyVerticalGrid`): its `content` block adds the items ([`LazyGridScope`]), line after line, each taking its
/// span of the columns. `state` (remembered by default) scrolls it and tells where it is; `content_padding` pads the
/// content, scrolling under it; `reverse_layout` puts item 0 at the bottom (the arrangement then Bottom by default);
/// `vertical_arrangement` puts the lines along (its spacing between them; when they all fit, where they are),
/// `horizontal_arrangement` the columns across (its spacing between them, taken from the width before the columns
/// share it); `fling_behavior`, `user_scroll_enabled`, `overscroll_effect` (`None`: `remember_overscroll_effect()`;
/// `Some(None)`: none) as `LazyColumn`.
#[kompozable]
pub fn LazyVerticalGrid(
    columns: GridCells,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] state: Option<LazyGridState>,
    #[default(PaddingValues::new(Dp(0.0), Dp(0.0), Dp(0.0), Dp(0.0)))] content_padding: PaddingValues,
    #[default(false)] reverse_layout: bool,
    #[default(None)] vertical_arrangement: Option<VerticalArrangement>,
    #[default(Arrangement::Start)] horizontal_arrangement: HorizontalArrangement,
    #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
    #[default(true)] user_scroll_enabled: bool,
    #[default(None)] overscroll_effect: Option<Option<Rc<dyn OverscrollEffect>>>,
    content: impl Fn(&mut LazyGridScope) + 'static,
) {
    let state = match state {
        Some(state) => state,
        None => remember_lazy_grid_state(),
    };
    let vertical_arrangement =
        vertical_arrangement.unwrap_or(if !reverse_layout { Arrangement::Top } else { Arrangement::Bottom });
    let fling_behavior = match fling_behavior {
        Some(f) => f,
        None => ScrollableDefaults::fling_behavior(),
    };
    let overscroll_effect = match overscroll_effect {
        Some(e) => e,
        None => remember_overscroll_effect(),
    };
    // (Compose's `rememberColumnWidthSums`)
    let slots = {
        let (c, h) = (columns.clone(), horizontal_arrangement.clone());
        remember_keyed((columns, horizontal_arrangement.clone()), move || {
            Rc::new(LazyGridSlotsProvider::column_width_sums(c, h))
        })
    };
    LazyGrid(
        modifier,
        state,
        slots,
        content_padding,
        reverse_layout,
        true,
        fling_behavior,
        user_scroll_enabled,
        overscroll_effect,
        vertical_arrangement,
        horizontal_arrangement,
        Rc::new(content),
    );
}

/// A grid of `rows` scrolling horizontally (Compose's `LazyHorizontalGrid`): as [`LazyVerticalGrid`], along its
/// width (right to left mirrored); `reverse_layout` puts item 0 at the end (`horizontal_arrangement` then End by
/// default), `vertical_arrangement` puts the rows across.
#[kompozable]
pub fn LazyHorizontalGrid(
    rows: GridCells,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] state: Option<LazyGridState>,
    #[default(PaddingValues::new(Dp(0.0), Dp(0.0), Dp(0.0), Dp(0.0)))] content_padding: PaddingValues,
    #[default(false)] reverse_layout: bool,
    #[default(None)] horizontal_arrangement: Option<HorizontalArrangement>,
    #[default(Arrangement::Top)] vertical_arrangement: VerticalArrangement,
    #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
    #[default(true)] user_scroll_enabled: bool,
    #[default(None)] overscroll_effect: Option<Option<Rc<dyn OverscrollEffect>>>,
    content: impl Fn(&mut LazyGridScope) + 'static,
) {
    let state = match state {
        Some(state) => state,
        None => remember_lazy_grid_state(),
    };
    let horizontal_arrangement =
        horizontal_arrangement.unwrap_or(if !reverse_layout { Arrangement::Start } else { Arrangement::End });
    let fling_behavior = match fling_behavior {
        Some(f) => f,
        None => ScrollableDefaults::fling_behavior(),
    };
    let overscroll_effect = match overscroll_effect {
        Some(e) => e,
        None => remember_overscroll_effect(),
    };
    // (Compose's `rememberRowHeightSums`)
    let slots = {
        let (r, v) = (rows.clone(), vertical_arrangement.clone());
        remember_keyed((rows, vertical_arrangement.clone()), move || {
            Rc::new(LazyGridSlotsProvider::row_height_sums(r, v))
        })
    };
    LazyGrid(
        modifier,
        state,
        slots,
        content_padding,
        reverse_layout,
        false,
        fling_behavior,
        user_scroll_enabled,
        overscroll_effect,
        vertical_arrangement,
        horizontal_arrangement,
        Rc::new(content),
    );
}
