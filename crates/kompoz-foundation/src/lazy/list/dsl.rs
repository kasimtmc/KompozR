//! `LazyColumn` and `LazyRow` and what their block builds (Compose's LazyDsl.kt and LazyListIntervalContent.kt):
//! the items, as intervals — one item, a count of them, a list's, a sticky header — each with its keys, content
//! types and content.
//!
//! ```ignore
//! LazyColumn(modifier = Modifier.fill_max_size(), content = move |this: &mut LazyListScope| {
//!     this.item(|| Text("Başlık"));
//!     this.items_list(todos.clone(), key = Some(Rc::new(|t: &Todo| LazyKey::new(t.id))), item_content = |t: &Todo| TodoRow(t.clone()));
//! });
//! ```

use super::list::LazyList;
use super::state::{LazyListState, remember_lazy_list_state};
use crate::gestures::{FlingBehavior, ScrollableDefaults};
use crate::layout::{Arrangement, HorizontalArrangement, PaddingValues, VerticalArrangement};
use crate::lazy::layout::{IntervalList, LazyKey, LazyLayoutInterval, LazyLayoutIntervalContent, MutableIntervalList};
use crate::overscroll::{OverscrollEffect, remember_overscroll_effect};
use kompoz_graphics::Dp;
use kompoz_runtime::{SavedValue, SnapshotStateList, kompozable, named};
use kompoz_ui::layout::SlotKey;
use kompoz_ui::{Alignment, HorizontalAlignment, Modifier, VerticalAlignment};
use std::hash::Hash;
use std::rc::Rc;

/// An interval of a list's content (Compose's internal LazyListInterval): its keys, content types and items'
/// content, by the index in it.
#[doc(hidden)]
pub struct LazyListInterval {
    pub(crate) key: Option<Rc<dyn Fn(i32) -> LazyKey>>,
    pub(crate) content_type: Rc<dyn Fn(i32) -> Option<SlotKey>>,
    pub(crate) item: Rc<dyn Fn(i32)>,
}

impl LazyLayoutInterval for LazyListInterval {
    fn key(&self, index: i32) -> Option<LazyKey> {
        self.key.as_ref().map(|k| k(index))
    }
    fn content_type(&self, index: i32) -> Option<SlotKey> {
        (self.content_type)(index)
    }
}

/// What a `LazyColumn`'s or `LazyRow`'s block adds its items to (Compose's LazyListScope, the interval content it
/// builds): `item`, `items` (a count), `items_list` and `items_indexed` (a list's), `sticky_header`. An item's key
/// keeps its state and its place as the items change (by default its index); its content type lets an item reuse
/// the composition of an item of the same type that left.
#[derive(Default)]
pub struct LazyListScope {
    pub(crate) intervals: MutableIntervalList<LazyListInterval>,
    pub(crate) header_indexes: Vec<i32>,
}

impl LazyLayoutIntervalContent for LazyListScope {
    type Interval = LazyListInterval;
    fn intervals(&self) -> &MutableIntervalList<LazyListInterval> {
        &self.intervals
    }
}

impl LazyListScope {
    /// The content `content` builds (Compose's `LazyListIntervalContent(content)`).
    pub(crate) fn of(content: &dyn Fn(&mut LazyListScope)) -> LazyListScope {
        let mut scope = LazyListScope::default();
        content(&mut scope);
        scope
    }

    /// The indices of the sticky headers.
    pub(crate) fn header_indexes(&self) -> &[i32] {
        &self.header_indexes
    }
}

/// What a list's items can be given as (Kotlin's `List<T>` and `Array<T>`): a vector, a slice, a shared one, a
/// snapshot state list (read where the block runs: the list composes again as it changes).
pub trait LazyListItems<T> {
    fn into_items(self) -> Rc<Vec<T>>;
}

impl<T: 'static> LazyListItems<T> for Vec<T> {
    fn into_items(self) -> Rc<Vec<T>> {
        Rc::new(self)
    }
}

impl<T: Clone + 'static> LazyListItems<T> for &Vec<T> {
    fn into_items(self) -> Rc<Vec<T>> {
        Rc::new(self.clone())
    }
}

impl<T: Clone + 'static> LazyListItems<T> for &[T] {
    fn into_items(self) -> Rc<Vec<T>> {
        Rc::new(self.to_vec())
    }
}

impl<T: Clone + 'static, const N: usize> LazyListItems<T> for [T; N] {
    fn into_items(self) -> Rc<Vec<T>> {
        Rc::new(self.to_vec())
    }
}

impl<T: 'static> LazyListItems<T> for Rc<Vec<T>> {
    fn into_items(self) -> Rc<Vec<T>> {
        self
    }
}

impl<T: Clone + Send + Sync + 'static> LazyListItems<T> for SnapshotStateList<T> {
    fn into_items(self) -> Rc<Vec<T>> {
        Rc::new(self.to_vec())
    }
}

impl<T: Clone + Send + Sync + 'static> LazyListItems<T> for &SnapshotStateList<T> {
    fn into_items(self) -> Rc<Vec<T>> {
        Rc::new(self.to_vec())
    }
}

/// A key function of a list's items for `items_list` (Rust's way to write Kotlin's `key = { it.id }`): the key
/// `key` makes of an item.
pub fn item_key<T: 'static, K: Hash + Eq + Clone + Into<SavedValue> + 'static>(
    key: impl Fn(&T) -> K + 'static,
) -> Option<Rc<dyn for<'a> Fn(&'a T) -> LazyKey>> {
    Some(Rc::new(move |item: &T| LazyKey::new(key(item))))
}

/// A key function of counted items for `items` (Kotlin's `key = { index -> … }`).
pub fn index_key<K: Hash + Eq + Clone + Into<SavedValue> + 'static>(
    key: impl Fn(i32) -> K + 'static,
) -> Option<Rc<dyn Fn(i32) -> LazyKey>> {
    Some(Rc::new(move |index| LazyKey::new(key(index))))
}

/// A key function of a list's items with their indices for `items_indexed` (Kotlin's `key = { index, item -> … }`).
pub fn indexed_key<T: 'static, K: Hash + Eq + Clone + Into<SavedValue> + 'static>(
    key: impl Fn(i32, &T) -> K + 'static,
) -> Option<Rc<dyn for<'a> Fn(i32, &'a T) -> LazyKey>> {
    Some(Rc::new(move |index, item: &T| LazyKey::new(key(index, item))))
}

#[named]
impl LazyListScope {
    /// One item (Compose's `item`): of `key` (else its index's), of `content_type`.
    pub fn item(
        &mut self,
        #[default(None)] key: Option<LazyKey>,
        #[default(None)] content_type: Option<SlotKey>,
        content: impl Fn() + 'static,
    ) {
        self.intervals.add_interval(
            1,
            LazyListInterval {
                key: key.map(|k| Rc::new(move |_: i32| k.clone()) as Rc<dyn Fn(i32) -> LazyKey>),
                content_type: Rc::new(move |_| content_type.clone()),
                item: Rc::new(move |_| content()),
            },
        );
    }

    /// `count` items (Compose's `items(count)`): each one's key and content type by its index (none: its index's
    /// key, no type), its content given its index.
    pub fn items(
        &mut self,
        count: i32,
        #[default(None)] key: Option<Rc<dyn Fn(i32) -> LazyKey>>,
        #[default(None)] content_type: Option<Rc<dyn Fn(i32) -> Option<SlotKey>>>,
        item_content: impl Fn(i32) + 'static,
    ) {
        self.intervals.add_interval(
            count,
            LazyListInterval {
                key,
                content_type: content_type.unwrap_or_else(|| Rc::new(|_| None)),
                item: Rc::new(item_content),
            },
        );
    }

    /// A list's items (Compose's `items(items: List<T>)`): each one's key and content type by its element, its
    /// content given its element.
    /// Kotlin: `items(items: List<T>, key: ((item: T) -> Any)? = null, contentType: (item: T) -> Any? = { null }, itemContent: @Composable LazyItemScope.(item: T) -> Unit)`
    pub fn items_list<T: 'static>(
        &mut self,
        items: impl LazyListItems<T>,
        #[default(None)] key: Option<Rc<dyn for<'a> Fn(&'a T) -> LazyKey>>,
        #[default(None)] content_type: Option<Rc<dyn for<'a> Fn(&'a T) -> Option<SlotKey>>>,
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
        let item: Rc<dyn Fn(i32)> = Rc::new(move |index| item_content(&items[index as usize]));
        self.intervals.add_interval(count, LazyListInterval { key, content_type, item });
    }

    /// A list's items with their indices (Compose's `itemsIndexed`).
    /// Kotlin: `itemsIndexed(items: List<T>, key: ((index: Int, item: T) -> Any)? = null, contentType: (index: Int, item: T) -> Any? = { _, _ -> null }, itemContent: @Composable LazyItemScope.(index: Int, item: T) -> Unit)`
    pub fn items_indexed<T: 'static>(
        &mut self,
        items: impl LazyListItems<T>,
        #[default(None)] key: Option<Rc<dyn for<'a> Fn(i32, &'a T) -> LazyKey>>,
        #[default(None)] content_type: Option<Rc<dyn for<'a> Fn(i32, &'a T) -> Option<SlotKey>>>,
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
        let item: Rc<dyn Fn(i32)> = Rc::new(move |index| item_content(index, &items[index as usize]));
        self.intervals.add_interval(count, LazyListInterval { key, content_type, item });
    }

    /// A header that stays at the start of the list while the items after it scroll under it, until the next one
    /// pushes it out (Compose's `stickyHeader`); its content is given its index.
    pub fn sticky_header(
        &mut self,
        #[default(None)] key: Option<LazyKey>,
        #[default(None)] content_type: Option<SlotKey>,
        content: impl Fn(i32) + 'static,
    ) {
        let header_index = self.intervals.size();
        self.header_indexes.push(header_index);
        self.intervals.add_interval(
            1,
            LazyListInterval {
                key: key.map(|k| Rc::new(move |_: i32| k.clone()) as Rc<dyn Fn(i32) -> LazyKey>),
                content_type: Rc::new(move |_| content_type.clone()),
                item: Rc::new(move |_| content(header_index)),
            },
        );
    }
}

/// A vertically scrolling list composing and laying out only the items in view (Compose's `LazyColumn`): its
/// `content` block adds the items ([`LazyListScope`]). `state` (remembered by default) scrolls it and tells where
/// it is; `content_padding` pads the content, scrolling under it; `reverse_layout` puts item 0 at the bottom (the
/// arrangement then Bottom by default); `vertical_arrangement` puts the items along (its spacing between them;
/// when they all fit, where they are), `horizontal_alignment` across; `fling_behavior` (the default fling),
/// `user_scroll_enabled` (scrolled only from code and by bringing into view when not), `overscroll_effect`
/// (`None`: `remember_overscroll_effect()`; `Some(None)`: none).
#[kompozable]
pub fn LazyColumn(
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] state: Option<LazyListState>,
    #[default(PaddingValues::new(Dp(0.0), Dp(0.0), Dp(0.0), Dp(0.0)))] content_padding: PaddingValues,
    #[default(false)] reverse_layout: bool,
    #[default(None)] vertical_arrangement: Option<VerticalArrangement>,
    #[default(Alignment::Start)] horizontal_alignment: HorizontalAlignment,
    #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
    #[default(true)] user_scroll_enabled: bool,
    #[default(None)] overscroll_effect: Option<Option<Rc<dyn OverscrollEffect>>>,
    content: impl Fn(&mut LazyListScope) + 'static,
) {
    let state = match state {
        Some(state) => state,
        None => remember_lazy_list_state(),
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
    LazyList(
        modifier,
        state,
        content_padding,
        reverse_layout,
        true,
        fling_behavior,
        user_scroll_enabled,
        overscroll_effect,
        Some(horizontal_alignment),
        Some(vertical_arrangement),
        None,
        None,
        Rc::new(content),
    );
}

/// A horizontally scrolling list composing and laying out only the items in view (Compose's `LazyRow`): as
/// [`LazyColumn`], along its width (right to left mirrored); `reverse_layout` puts item 0 at the end
/// (`horizontal_arrangement` then End by default), `vertical_alignment` aligns the items across.
#[kompozable]
pub fn LazyRow(
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] state: Option<LazyListState>,
    #[default(PaddingValues::new(Dp(0.0), Dp(0.0), Dp(0.0), Dp(0.0)))] content_padding: PaddingValues,
    #[default(false)] reverse_layout: bool,
    #[default(None)] horizontal_arrangement: Option<HorizontalArrangement>,
    #[default(Alignment::Top)] vertical_alignment: VerticalAlignment,
    #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
    #[default(true)] user_scroll_enabled: bool,
    #[default(None)] overscroll_effect: Option<Option<Rc<dyn OverscrollEffect>>>,
    content: impl Fn(&mut LazyListScope) + 'static,
) {
    let state = match state {
        Some(state) => state,
        None => remember_lazy_list_state(),
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
    LazyList(
        modifier,
        state,
        content_padding,
        reverse_layout,
        false,
        fling_behavior,
        user_scroll_enabled,
        overscroll_effect,
        None,
        None,
        Some(vertical_alignment),
        Some(horizontal_arrangement),
        Rc::new(content),
    );
}
