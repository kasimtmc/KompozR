//! Compose for desktop's `androidx.compose.foundation.v2`: the [`ScrollbarAdapter`] scrollbars take (v2/Scrollbar.skiko.kt),
//! in `f64` (a list of millions of items, or content measured in fractions), and its adapters of a scroll state, a
//! lazy list's and a lazy grid's.

use crate::gestures::{LocalFuture, Orientation, ScrollableStateExtensions};
use crate::lazy::LazyListState;
use crate::lazy::grid::{LazyGridItemInfo, LazyGridState};
use crate::scroll::ScrollState;
use kompoz_runtime::{DerivedState, derived_state_of};

/// How a scrollbar reads and scrolls its scrollable (Compose for desktop's `v2.ScrollbarAdapter`): where the
/// content is scrolled to, how big it and the viewport are, along the scrollbar — px usually, any unit as long as
/// they agree —, and jumping to an offset.
pub trait ScrollbarAdapter {
    /// How far the content is scrolled from its start (Compose's `scrollOffset`).
    fn scroll_offset(&self) -> f64;
    /// The content's size along the scrollbar (Compose's `contentSize`).
    fn content_size(&self) -> f64;
    /// The viewport's size along the scrollbar (Compose's `viewportSize`).
    fn viewport_size(&self) -> f64;
    /// Jumps to `scroll_offset`, kept within the scroll range (Compose's suspending `scrollTo`).
    fn scroll_to(&self, scroll_offset: f64) -> LocalFuture<'static, ()>;
    /// The largest offset (Compose's extension `maxScrollOffset`): what of the content doesn't fit, 0 at least.
    fn max_scroll_offset(&self) -> f64 {
        (self.content_size() - self.viewport_size()).max(0.0)
    }
}

/// The adapter of a scroll state (Compose's `ScrollableScrollbarAdapter`: `verticalScroll`/`horizontalScroll`'s).
pub(crate) struct ScrollableScrollbarAdapter {
    pub(crate) scroll_state: ScrollState,
}

impl ScrollbarAdapter for ScrollableScrollbarAdapter {
    fn scroll_offset(&self) -> f64 {
        self.scroll_state.value() as f64
    }
    fn scroll_to(&self, scroll_offset: f64) -> LocalFuture<'static, ()> {
        let state = self.scroll_state.clone();
        Box::pin(async move {
            state.scroll_to(scroll_offset.round() as i32).await;
        })
    }
    /// (not quite the content's when it is smaller than the viewport — the max value 0 —, which the scrollbar
    /// doesn't mind: it shows nothing then)
    fn content_size(&self) -> f64 {
        self.scroll_state.max_value() as f64 + self.viewport_size()
    }
    fn viewport_size(&self) -> f64 {
        self.scroll_state.viewport_size() as f64
    }
}

/// The adapter of a value-based field's scroll state (Compose for desktop's `TextFieldScrollbarAdapter`).
pub(crate) struct TextFieldScrollbarAdapter {
    pub(crate) scroll_state: crate::text::TextFieldScrollState,
}

impl ScrollbarAdapter for TextFieldScrollbarAdapter {
    fn scroll_offset(&self) -> f64 {
        self.scroll_state.offset() as f64
    }
    fn content_size(&self) -> f64 {
        self.scroll_state.max_offset() as f64 + self.viewport_size()
    }
    fn viewport_size(&self) -> f64 {
        self.scroll_state.viewport_size() as f64
    }
    fn scroll_to(&self, scroll_offset: f64) -> LocalFuture<'static, ()> {
        let state = self.scroll_state.clone();
        Box::pin(async move {
            state.set_offset((scroll_offset as f32).clamp(0.0, state.max_offset().max(0.0)));
        })
    }
}

/// A line of a lazy layout in view (Compose's `LazyLineContentAdapter.VisibleLine`): its index and offset.
pub(crate) struct VisibleLine {
    index: i32,
    offset: i32,
}

/// What lazy layouts of lines (rows of a vertical one, columns of a horizontal one) tell their adapter (Compose's
/// abstract `LazyLineContentAdapter`'s members).
pub(crate) trait LazyLines {
    fn viewport_size(&self) -> f64;
    fn first_visible_line(&self) -> Option<VisibleLine>;
    fn total_line_count(&self) -> i32;
    /// The content padding before and after, along it.
    fn content_padding(&self) -> i32;
    fn snap_to_line(&self, line_index: i32, scroll_offset: i32) -> LocalFuture<'static, ()>;
    fn scroll_by(&self, value: f32) -> LocalFuture<'static, ()>;
    fn average_visible_line_size(&self) -> f64;
    fn line_spacing(&self) -> i32;
}

/// The adapter of a lazy layout of lines (Compose's `LazyLineContentAdapter`): its content's size estimated from
/// the lines in view (their average size, a derived state), its offset from the first line in view; scrolled by
/// the distance when it is within a viewport (items of other sizes don't make it jump), else snapped to the line.
pub(crate) struct LazyLineContentAdapter<L: LazyLines> {
    lines: std::rc::Rc<L>,
    average_visible_line_size: DerivedState<f64>,
}

impl<L: LazyLines + 'static> LazyLineContentAdapter<L> {
    pub(crate) fn new(lines: L) -> LazyLineContentAdapter<L> {
        let lines = std::rc::Rc::new(lines);
        let l = lines.clone();
        let average_visible_line_size =
            derived_state_of(move || if l.total_line_count() == 0 { 0.0 } else { l.average_visible_line_size() });
        LazyLineContentAdapter { lines, average_visible_line_size }
    }

    fn average_visible_line_size_with_spacing(&self) -> f64 {
        self.average_visible_line_size.get() + self.lines.line_spacing() as f64
    }

    fn snap_to(&self, scroll_offset: f64) -> LocalFuture<'static, ()> {
        let coerced = scroll_offset.clamp(0.0, self.max_scroll_offset().max(0.0));
        let with_spacing = self.average_visible_line_size_with_spacing();
        let index = ((coerced / with_spacing) as i32).max(0).min(self.lines.total_line_count() - 1);
        let offset = ((coerced - index as f64 * with_spacing) as i32).max(0);
        self.lines.snap_to_line(index, offset)
    }
}

impl<L: LazyLines + 'static> ScrollbarAdapter for LazyLineContentAdapter<L> {
    fn scroll_offset(&self) -> f64 {
        match self.lines.first_visible_line() {
            None => 0.0,
            Some(line) => line.index as f64 * self.average_visible_line_size_with_spacing() - line.offset as f64,
        }
    }
    fn content_size(&self) -> f64 {
        let total_line_count = self.lines.total_line_count();
        self.average_visible_line_size.get() * total_line_count as f64
            + (self.lines.line_spacing() * (total_line_count - 1).max(0)) as f64
            + self.lines.content_padding() as f64
    }
    fn viewport_size(&self) -> f64 {
        self.lines.viewport_size()
    }
    fn scroll_to(&self, scroll_offset: f64) -> LocalFuture<'static, ()> {
        let distance = scroll_offset - self.scroll_offset();
        // (within a viewport scrolled by the distance — items of other sizes don't make it jump —, else jumped to at
        // once without composing what is between)
        if distance.abs() <= self.viewport_size() {
            self.lines.scroll_by(distance as f32)
        } else {
            self.snap_to(scroll_offset)
        }
    }
}

/// A lazy list's lines (Compose's `LazyListScrollbarAdapter`): its items.
pub(crate) struct LazyListLines {
    pub(crate) state: LazyListState,
}

impl LazyListLines {
    /// Compose's heuristic leaving out the header stuck at the top (it is in view with its own index, and overlaps
    /// what follows): the index in the items in view of the first that isn't stuck, if any.
    fn first_floating_visible_item_index(&self) -> Option<usize> {
        let info = self.state.layout_info();
        let items = info.visible_items_info();
        match items.len() {
            0 => None,
            1 => Some(0),
            _ => {
                let (first, second) = (&items[0], &items[1]);
                if first.index() < second.index() - 1
                    || first.offset() + first.size() + info.main_axis_item_spacing() > second.offset()
                {
                    Some(1)
                } else {
                    Some(0)
                }
            }
        }
    }
}

impl LazyLines for LazyListLines {
    fn viewport_size(&self) -> f64 {
        let info = self.state.layout_info();
        let size = info.viewport_size();
        (if info.orientation() == Orientation::Vertical { size.height } else { size.width }) as f64
    }
    fn first_visible_line(&self) -> Option<VisibleLine> {
        let i = self.first_floating_visible_item_index()?;
        let item = self.state.layout_info().visible_items_info()[i].clone();
        Some(VisibleLine { index: item.index(), offset: item.offset() })
    }
    fn total_line_count(&self) -> i32 {
        self.state.layout_info().total_items_count()
    }
    fn content_padding(&self) -> i32 {
        let info = self.state.layout_info();
        info.before_content_padding() + info.after_content_padding()
    }
    fn snap_to_line(&self, line_index: i32, scroll_offset: i32) -> LocalFuture<'static, ()> {
        let state = self.state.clone();
        Box::pin(async move {
            kompoz_runtime::named_args!(state.scroll_to_item(line_index, scroll_offset = scroll_offset)).await
        })
    }
    fn scroll_by(&self, value: f32) -> LocalFuture<'static, ()> {
        let state = self.state.clone();
        Box::pin(async move {
            state.scroll_by(value).await;
        })
    }
    fn average_visible_line_size(&self) -> f64 {
        let Some(first_floating) = self.first_floating_visible_item_index() else { return 0.0 };
        let info = self.state.layout_info();
        let items = info.visible_items_info();
        let first = &items[first_floating];
        let last = items.last().expect("an item in view");
        let count = (items.len() - first_floating) as i32;
        (last.offset() + last.size() - first.offset() - (count - 1) * info.main_axis_item_spacing()) as f64
            / count as f64
    }
    fn line_spacing(&self) -> i32 {
        self.state.layout_info().main_axis_item_spacing()
    }
}

/// A lazy grid's lines (Compose's `LazyGridScrollbarAdapter`): its rows (vertical) or columns (horizontal), items
/// leaving (on no line) left out.
pub(crate) struct LazyGridLines {
    pub(crate) state: LazyGridState,
    is_vertical: bool,
}

impl LazyGridLines {
    pub(crate) fn new(state: LazyGridState) -> LazyGridLines {
        let is_vertical = state.layout_info().orientation() == Orientation::Vertical;
        LazyGridLines { state, is_vertical }
    }
    fn line(&self, item: &LazyGridItemInfo) -> i32 {
        if self.is_vertical { item.row() } else { item.column() }
    }
    fn line_is_known(&self, item: &LazyGridItemInfo) -> bool {
        self.line(item) != if self.is_vertical { LazyGridItemInfo::UnknownRow } else { LazyGridItemInfo::UnknownColumn }
    }
    fn main_axis_size(&self, item: &LazyGridItemInfo) -> i32 {
        if self.is_vertical { item.size().height } else { item.size().width }
    }
    fn main_axis_offset(&self, item: &LazyGridItemInfo) -> i32 {
        if self.is_vertical { item.offset().y } else { item.offset().x }
    }
    fn line_of_index(&self, index: i32) -> i32 {
        index / self.state.slots_per_line()
    }
    fn index_of_first_in_line(&self, line: i32) -> i32 {
        line * self.state.slots_per_line()
    }
}

impl LazyLines for LazyGridLines {
    fn viewport_size(&self) -> f64 {
        let info = self.state.layout_info();
        let size = info.viewport_size();
        (if info.orientation() == Orientation::Vertical { size.height } else { size.width }) as f64
    }
    fn first_visible_line(&self) -> Option<VisibleLine> {
        let items = self.state.layout_info().visible_items_info();
        let first = items.iter().find(|i| self.line_is_known(i))?;
        Some(VisibleLine { index: self.line(first), offset: self.main_axis_offset(first) })
    }
    fn total_line_count(&self) -> i32 {
        let item_count = self.state.layout_info().total_items_count();
        if item_count == 0 { 0 } else { self.line_of_index(item_count - 1) + 1 }
    }
    fn content_padding(&self) -> i32 {
        let info = self.state.layout_info();
        info.before_content_padding() + info.after_content_padding()
    }
    fn snap_to_line(&self, line_index: i32, scroll_offset: i32) -> LocalFuture<'static, ()> {
        let state = self.state.clone();
        let index = self.index_of_first_in_line(line_index);
        Box::pin(async move {
            kompoz_runtime::named_args!(state.scroll_to_item(index, scroll_offset = scroll_offset)).await
        })
    }
    fn scroll_by(&self, value: f32) -> LocalFuture<'static, ()> {
        let state = self.state.clone();
        Box::pin(async move {
            state.scroll_by(value).await;
        })
    }
    fn average_visible_line_size(&self) -> f64 {
        let items = self.state.layout_info().visible_items_info();
        let Some(first) = items.iter().find(|i| self.line_is_known(i)) else { return 0.0 };
        let last = items.iter().rev().find(|i| self.line_is_known(i)).expect("a known line");
        // (the size of the last line: its tallest item)
        let last_line = self.line(last);
        let last_line_size = items
            .iter()
            .rev()
            .filter(|i| self.line_is_known(i))
            .take_while(|i| self.line(i) == last_line)
            .map(|i| self.main_axis_size(i))
            .max()
            .unwrap_or(0);
        let line_count = self.line(last) - self.line(first) + 1;
        let line_spacing_sum = (line_count - 1) * self.line_spacing();
        (self.main_axis_offset(last) + last_line_size - self.main_axis_offset(first) - line_spacing_sum) as f64
            / line_count as f64
    }
    fn line_spacing(&self) -> i32 {
        self.state.layout_info().main_axis_item_spacing()
    }
}
