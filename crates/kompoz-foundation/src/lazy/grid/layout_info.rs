//! What a lazy grid's last layout showed (Compose's LazyGridLayoutInfo.kt and LazyGridItemInfo.kt).

use crate::gestures::Orientation;
use crate::lazy::layout::LazyKey;
use kompoz_graphics::{Density, IntOffset, IntSize};
use kompoz_ui::layout::SlotKey;
use std::rc::Rc;

/// An item in view (Compose's LazyGridItemInfo): its index, key, content type, where it is (px from where the
/// content starts, past the content padding; negative along the grid when partly scrolled out), its size, its row
/// and column (-1 for an item not on a line: one sticking or moving away), and its span.
#[derive(Clone, PartialEq, Debug)]
pub struct LazyGridItemInfo {
    pub(crate) index: i32,
    pub(crate) key: LazyKey,
    pub(crate) offset: IntOffset,
    pub(crate) row: i32,
    pub(crate) column: i32,
    pub(crate) size: IntSize,
    pub(crate) content_type: Option<SlotKey>,
    pub(crate) span: i32,
}

impl LazyGridItemInfo {
    /// The row or column of an item not on a line (Compose's `UnknownRow`, `UnknownColumn`).
    pub const UnknownRow: i32 = -1;
    pub const UnknownColumn: i32 = -1;

    pub fn index(&self) -> i32 {
        self.index
    }
    pub fn key(&self) -> LazyKey {
        self.key.clone()
    }
    pub fn offset(&self) -> IntOffset {
        self.offset
    }
    pub fn row(&self) -> i32 {
        self.row
    }
    pub fn column(&self) -> i32 {
        self.column
    }
    pub fn size(&self) -> IntSize {
        self.size
    }
    pub fn content_type(&self) -> Option<SlotKey> {
        self.content_type.clone()
    }
    pub fn span(&self) -> i32 {
        self.span
    }

    /// Its line: its row in a vertical grid, its column in a horizontal one (Compose's `lineIndex`).
    pub(crate) fn line_index(&self, orientation: Orientation) -> i32 {
        if orientation == Orientation::Vertical { self.row } else { self.column }
    }

    pub(crate) fn offset_on_main_axis(&self, orientation: Orientation) -> i32 {
        if orientation == Orientation::Vertical { self.offset.y } else { self.offset.x }
    }

    pub(crate) fn size_on_main_axis(&self, orientation: Orientation) -> i32 {
        if orientation == Orientation::Vertical { self.size.height } else { self.size.width }
    }
}

/// What a lazy grid's last layout showed (Compose's LazyGridLayoutInfo): the items in view (by index; a sticking
/// header among them), the part of the content in view in the items' offsets along the grid (from minus the
/// padding before it to the viewport's length less that), how many items there are, the viewport's size, the
/// grid's orientation, whether it is reversed, its content padding and the spacing between lines, px, and the
/// cells of a line (`max_span`).
#[derive(Clone, PartialEq, Debug)]
pub struct LazyGridLayoutInfo {
    pub(crate) visible_items_info: Rc<Vec<LazyGridItemInfo>>,
    pub(crate) viewport_start_offset: i32,
    pub(crate) viewport_end_offset: i32,
    pub(crate) total_items_count: i32,
    pub(crate) viewport_size: IntSize,
    pub(crate) orientation: Orientation,
    pub(crate) reverse_layout: bool,
    pub(crate) before_content_padding: i32,
    pub(crate) after_content_padding: i32,
    pub(crate) main_axis_item_spacing: i32,
    pub(crate) max_span: i32,
    /// The density it was measured in (none before the first measure).
    pub(crate) density: Option<Density>,
}

impl Default for LazyGridLayoutInfo {
    fn default() -> Self {
        LazyGridLayoutInfo {
            visible_items_info: Rc::new(Vec::new()),
            viewport_start_offset: 0,
            viewport_end_offset: 0,
            total_items_count: 0,
            viewport_size: IntSize::Zero,
            orientation: Orientation::Vertical,
            reverse_layout: false,
            before_content_padding: 0,
            after_content_padding: 0,
            main_axis_item_spacing: 0,
            max_span: 0,
            density: None,
        }
    }
}

impl LazyGridLayoutInfo {
    pub fn visible_items_info(&self) -> Vec<LazyGridItemInfo> {
        (*self.visible_items_info).clone()
    }
    pub fn viewport_start_offset(&self) -> i32 {
        self.viewport_start_offset
    }
    pub fn viewport_end_offset(&self) -> i32 {
        self.viewport_end_offset
    }
    pub fn total_items_count(&self) -> i32 {
        self.total_items_count
    }
    pub fn viewport_size(&self) -> IntSize {
        self.viewport_size
    }
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }
    pub fn reverse_layout(&self) -> bool {
        self.reverse_layout
    }
    pub fn before_content_padding(&self) -> i32 {
        self.before_content_padding
    }
    pub fn after_content_padding(&self) -> i32 {
        self.after_content_padding
    }
    pub fn main_axis_item_spacing(&self) -> i32 {
        self.main_axis_item_spacing
    }
    pub fn max_span(&self) -> i32 {
        self.max_span
    }

    /// The items in view, without copying them.
    pub(crate) fn items(&self) -> &[LazyGridItemInfo] {
        &self.visible_items_info
    }

    /// The average length of the lines in view along the grid, with the spacing (Compose's
    /// `visibleLinesAverageMainAxisSize`): items moving away (on no line) left out.
    pub(crate) fn visible_lines_average_main_axis_size(&self) -> i32 {
        let is_vertical = self.orientation == Orientation::Vertical;
        let visible_items = self.items();
        if visible_items.is_empty() {
            return 0;
        }
        let line_of = |index: usize| if is_vertical { visible_items[index].row } else { visible_items[index].column };
        let mut total_lines_main_axis_size = 0;
        let mut lines_count = 0;
        let mut line_start_index = 0;
        while line_start_index < visible_items.len() {
            let current_line = line_of(line_start_index);
            if current_line == -1 {
                // (items moving away)
                line_start_index += 1;
                continue;
            }
            let mut line_main_axis_size = 0;
            let mut line_end_index = line_start_index;
            while line_end_index < visible_items.len() && line_of(line_end_index) == current_line {
                let size = visible_items[line_end_index].size;
                line_main_axis_size = line_main_axis_size.max(if is_vertical { size.height } else { size.width });
                line_end_index += 1;
            }
            total_lines_main_axis_size += line_main_axis_size;
            lines_count += 1;
            line_start_index = line_end_index;
        }
        // (all of them moving away: Compose would divide by zero; no line, only the spacing)
        if lines_count == 0 {
            return self.main_axis_item_spacing;
        }
        total_lines_main_axis_size / lines_count + self.main_axis_item_spacing
    }

    /// The viewport's length along the grid (Compose's `singleAxisViewportSize`).
    pub(crate) fn single_axis_viewport_size(&self) -> i32 {
        if self.orientation == Orientation::Vertical { self.viewport_size.height } else { self.viewport_size.width }
    }

    /// The content's length guessed from the average line (Compose's `calculateContentSize`).
    pub(crate) fn calculate_content_size(&self) -> i32 {
        let content_padding = self.before_content_padding + self.after_content_padding;
        if self.total_items_count == 0 {
            return content_padding;
        }
        let total_lines_count = (self.total_items_count as f32 / self.max_span as f32).ceil() as i32;
        let content_size_without_spacing =
            (self.visible_lines_average_main_axis_size() - self.main_axis_item_spacing) * total_lines_count;
        let total_spacing = (total_lines_count - 1) * self.main_axis_item_spacing;
        content_size_without_spacing + total_spacing + content_padding
    }
}
