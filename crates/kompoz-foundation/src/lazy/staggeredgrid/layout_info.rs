//! What a lazy staggered grid's last layout showed (Compose's LazyStaggeredGridLayoutInfo and
//! LazyStaggeredGridItemInfo in LazyStaggeredGridMeasureResult.kt).

use crate::gestures::Orientation;
use crate::lazy::layout::LazyKey;
use kompoz_graphics::{Density, IntOffset, IntSize};
use kompoz_ui::layout::SlotKey;
use std::rc::Rc;

/// An item in view (Compose's LazyStaggeredGridItemInfo): where it is (px from where the content starts, past the
/// content padding), its index, its lane (the first for a full-line item), key, size and content type.
#[derive(Clone, PartialEq, Debug)]
pub struct LazyStaggeredGridItemInfo {
    pub(crate) offset: IntOffset,
    pub(crate) index: i32,
    pub(crate) lane: i32,
    pub(crate) key: LazyKey,
    pub(crate) size: IntSize,
    pub(crate) content_type: Option<SlotKey>,
}

impl LazyStaggeredGridItemInfo {
    pub fn offset(&self) -> IntOffset {
        self.offset
    }
    pub fn index(&self) -> i32 {
        self.index
    }
    pub fn lane(&self) -> i32 {
        self.lane
    }
    pub fn key(&self) -> LazyKey {
        self.key.clone()
    }
    pub fn size(&self) -> IntSize {
        self.size
    }
    pub fn content_type(&self) -> Option<SlotKey> {
        self.content_type.clone()
    }
}

/// What a lazy staggered grid's last layout showed (Compose's LazyStaggeredGridLayoutInfo): its orientation, the
/// items in view (by index), how many items there are, the viewport's size, the part of the content in view along
/// the grid (from minus the padding before it to the viewport's length less that), its content padding, the
/// spacing between items of a lane, whether it is reversed.
#[derive(Clone, PartialEq, Debug)]
pub struct LazyStaggeredGridLayoutInfo {
    pub(crate) orientation: Orientation,
    pub(crate) visible_items_info: Rc<Vec<LazyStaggeredGridItemInfo>>,
    pub(crate) total_items_count: i32,
    pub(crate) viewport_size: IntSize,
    pub(crate) viewport_start_offset: i32,
    pub(crate) viewport_end_offset: i32,
    pub(crate) before_content_padding: i32,
    pub(crate) after_content_padding: i32,
    pub(crate) main_axis_item_spacing: i32,
    pub(crate) reverse_layout: bool,
    /// The density it was measured in.
    pub(crate) density: Density,
}

impl Default for LazyStaggeredGridLayoutInfo {
    fn default() -> Self {
        LazyStaggeredGridLayoutInfo {
            orientation: Orientation::Horizontal,
            visible_items_info: Rc::new(Vec::new()),
            total_items_count: 0,
            viewport_size: IntSize::Zero,
            viewport_start_offset: 0,
            viewport_end_offset: 0,
            before_content_padding: 0,
            after_content_padding: 0,
            main_axis_item_spacing: 0,
            reverse_layout: false,
            density: Density::new(1.0, 1.0),
        }
    }
}

impl LazyStaggeredGridLayoutInfo {
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }
    pub fn visible_items_info(&self) -> Vec<LazyStaggeredGridItemInfo> {
        (*self.visible_items_info).clone()
    }
    pub fn total_items_count(&self) -> i32 {
        self.total_items_count
    }
    pub fn viewport_size(&self) -> IntSize {
        self.viewport_size
    }
    pub fn viewport_start_offset(&self) -> i32 {
        self.viewport_start_offset
    }
    pub fn viewport_end_offset(&self) -> i32 {
        self.viewport_end_offset
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
    pub fn reverse_layout(&self) -> bool {
        self.reverse_layout
    }

    /// The items in view, without copying them.
    pub(crate) fn items(&self) -> &[LazyStaggeredGridItemInfo] {
        &self.visible_items_info
    }

    /// The item `item_index` if it is in view (Compose's `findVisibleItem`).
    pub(crate) fn find_visible_item(&self, item_index: i32) -> Option<&LazyStaggeredGridItemInfo> {
        let items = self.items();
        let (first, last) = (items.first()?, items.last()?);
        if item_index < first.index || item_index > last.index {
            return None;
        }
        items.binary_search_by(|i| i.index.cmp(&item_index)).ok().map(|at| &items[at])
    }

    /// The average size of the items in view along the grid, with the spacing (Compose's
    /// `visibleItemsAverageSize`).
    pub(crate) fn visible_items_average_size(&self) -> i32 {
        let items = self.items();
        if items.is_empty() {
            return 0;
        }
        let sum: i32 = items
            .iter()
            .map(|i| if self.orientation == Orientation::Vertical { i.size.height } else { i.size.width })
            .sum();
        sum / items.len() as i32 + self.main_axis_item_spacing
    }

    /// The content's length guessed from the average item over `lane_count` lanes (Compose's
    /// `calculateContentSize`).
    pub(crate) fn calculate_content_size(&self, lane_count: i32) -> i32 {
        let content_padding = self.before_content_padding + self.after_content_padding;
        if self.total_items_count == 0 || lane_count <= 0 {
            return content_padding;
        }
        let content_size_with_spacing =
            (self.visible_items_average_size() * self.total_items_count) / lane_count - self.main_axis_item_spacing;
        content_size_with_spacing + content_padding
    }

    /// The viewport's length along the grid (Compose's `singleAxisViewportSize`).
    pub(crate) fn single_axis_viewport_size(&self) -> i32 {
        if self.orientation == Orientation::Vertical { self.viewport_size.height } else { self.viewport_size.width }
    }
}
