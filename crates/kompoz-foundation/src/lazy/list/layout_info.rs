//! What a lazy list's last layout showed (Compose's LazyListLayoutInfo.kt and LazyListItemInfo.kt).

use crate::gestures::Orientation;
use crate::lazy::layout::LazyKey;
use kompoz_graphics::{Density, IntSize};
use kompoz_ui::layout::SlotKey;
use std::rc::Rc;

/// An item in view (Compose's LazyListItemInfo): its index, key, content type, where it is along the list (px from
/// where the content starts, past the content padding; negative when partly scrolled out) and its size along it.
#[derive(Clone, PartialEq, Debug)]
pub struct LazyListItemInfo {
    pub(crate) index: i32,
    pub(crate) key: LazyKey,
    pub(crate) offset: i32,
    pub(crate) size: i32,
    pub(crate) content_type: Option<SlotKey>,
    /// A sticking header, not scrolled with the others.
    pub(crate) non_scrollable: bool,
}

impl LazyListItemInfo {
    pub fn index(&self) -> i32 {
        self.index
    }
    pub fn key(&self) -> LazyKey {
        self.key.clone()
    }
    pub fn offset(&self) -> i32 {
        self.offset
    }
    pub fn size(&self) -> i32 {
        self.size
    }
    pub fn content_type(&self) -> Option<SlotKey> {
        self.content_type.clone()
    }
}

/// What a lazy list's last layout showed (Compose's LazyListLayoutInfo): the items in view (by index; a sticking
/// header first), the part of the content in view in the items' offsets (from minus the padding before it to the
/// viewport's length less that), how many items there are, the viewport's size, the list's orientation, whether
/// it is reversed, its content padding and the spacing between items, px.
#[derive(Clone, PartialEq, Debug)]
pub struct LazyListLayoutInfo {
    pub(crate) visible_items_info: Rc<Vec<LazyListItemInfo>>,
    pub(crate) viewport_start_offset: i32,
    pub(crate) viewport_end_offset: i32,
    pub(crate) total_items_count: i32,
    pub(crate) viewport_size: IntSize,
    pub(crate) orientation: Orientation,
    pub(crate) reverse_layout: bool,
    pub(crate) before_content_padding: i32,
    pub(crate) after_content_padding: i32,
    pub(crate) main_axis_item_spacing: i32,
    /// The density it was measured in (none before the first measure).
    pub(crate) density: Option<Density>,
}

impl Default for LazyListLayoutInfo {
    fn default() -> Self {
        LazyListLayoutInfo {
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
            density: None,
        }
    }
}

impl LazyListLayoutInfo {
    pub fn visible_items_info(&self) -> Vec<LazyListItemInfo> {
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

    /// The items in view, without copying them.
    pub(crate) fn items(&self) -> &[LazyListItemInfo] {
        &self.visible_items_info
    }

    /// The average size of the items in view, with the spacing (Compose's `visibleItemsAverageSize`).
    pub(crate) fn visible_items_average_size(&self) -> i32 {
        let items = self.items();
        if items.is_empty() {
            return 0;
        }
        let sum: i32 = items.iter().map(|i| i.size).sum();
        sum / items.len() as i32 + self.main_axis_item_spacing
    }

    /// The content's length guessed from the average item (Compose's `calculateContentSize`).
    pub(crate) fn calculate_content_size(&self) -> i32 {
        let content_padding = self.before_content_padding + self.after_content_padding;
        if self.total_items_count == 0 {
            return content_padding;
        }
        let content_size_without_spacing =
            (self.visible_items_average_size() - self.main_axis_item_spacing) * self.total_items_count;
        let total_spacing = (self.total_items_count - 1) * self.main_axis_item_spacing;
        content_size_without_spacing + total_spacing + content_padding
    }

    /// The viewport's length along the list (Compose's `singleAxisViewportSize`).
    pub(crate) fn single_axis_viewport_size(&self) -> i32 {
        if self.orientation == Orientation::Vertical { self.viewport_size.height } else { self.viewport_size.width }
    }
}
