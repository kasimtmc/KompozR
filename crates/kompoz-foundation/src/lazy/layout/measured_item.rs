//! What the lazy layouts' measures share about a measured item (Compose's LazyLayoutMeasuredItem.kt and
//! LazyLayoutStickyItems.kt): its key, its placeables and their offsets, its sizes along each axis, its lane — the
//! item animator and sticky headers work on these for lists, grids and staggered grids alike.

use super::key::LazyKey;
use super::lazy_layout::LazyLayoutMeasureScope;
use kompoz_graphics::IntOffset;
use kompoz_ui::{Constraints, Placeable};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// An item measured by a lazy layout's measure (Compose's LazyLayoutMeasuredItem): held as `Rc` by the measure's
/// lists, positioned in place (its offsets in cells).
pub(crate) trait LazyLayoutMeasuredItem {
    fn index(&self) -> i32;
    fn key(&self) -> LazyKey;
    fn placeables(&self) -> &[Placeable];
    fn horizontal_axis_size(&self) -> i32;
    fn vertical_axis_size(&self) -> i32;
    fn horizontal_axis_spacing(&self) -> i32;
    fn vertical_axis_spacing(&self) -> i32;
    /// The constraints its placeables were measured in.
    fn constraints(&self) -> Constraints;
    /// Its lane (a grid's) and how many lanes it spans.
    fn lane(&self) -> i32;
    fn span(&self) -> i32;
    /// Where placeable `placeable_index` goes, from the layout's content start.
    fn get_offset(&self, placeable_index: usize) -> IntOffset;
    /// Positions it (its main axis offset first for a list).
    fn position(&self, horizontal_axis_offset: i32, vertical_axis_offset: i32, layout_width: i32, layout_height: i32);
    /// Marks it as not scrolled by a scroll applied without a measure (a sticky item, an item moving away).
    fn make_non_scrollable(&self);

    fn placeables_count(&self) -> usize {
        self.placeables().len()
    }

    /// What placeable `index` told its parent (an `animate_item`'s specs).
    fn get_parent_data(&self, index: usize) -> Option<Rc<dyn std::any::Any>> {
        self.placeables()[index].parent_data()
    }

    fn main_axis_size_with_spacings(&self, is_vertical: bool) -> i32 {
        if is_vertical {
            self.vertical_axis_size() + self.vertical_axis_spacing()
        } else {
            self.horizontal_axis_size() + self.horizontal_axis_spacing()
        }
    }
}

/// What measures items by index for the item animator (Compose's LazyLayoutMeasuredItemProvider): it measures an
/// item of a lane and span within constraints.
pub(crate) trait LazyLayoutMeasuredItemProvider<T> {
    fn get_and_measure(&self, index: i32, lane: i32, span: i32, constraints: Constraints) -> Rc<T>;
}

/// The placeables of the items measured in a measure, so an item asked for again isn't measured again (Compose's
/// LazyLayoutMeasuredItemProvider's cache).
#[derive(Default)]
pub(crate) struct PlaceablesCache {
    placeables: RefCell<HashMap<i32, Vec<Placeable>>>,
}

impl PlaceablesCache {
    /// Item `index`'s layouts composed and measured in `constraints` (Compose's `getPlaceables`).
    pub(crate) fn get_placeables(
        &self,
        scope: &LazyLayoutMeasureScope,
        index: i32,
        constraints: Constraints,
    ) -> Vec<Placeable> {
        if let Some(cached) = self.placeables.borrow().get(&index) {
            return cached.clone();
        }
        let placeables: Vec<Placeable> = scope.compose(index).iter().map(|m| m.measure(constraints)).collect();
        self.placeables.borrow_mut().insert(index, placeables.clone());
        placeables
    }
}

/// The items in view: those placed between the first and last visible indices, and the sticking ones, in index
/// order (Compose's `updatedVisibleItems`).
pub(crate) fn updated_visible_items<T: LazyLayoutMeasuredItem>(
    first_visible_index: i32,
    last_visible_index: i32,
    positioned_items: &[Rc<T>],
    sticking_items: &[Rc<T>],
) -> Vec<Rc<T>> {
    if positioned_items.is_empty() {
        return Vec::new();
    }
    let mut final_visible_items: Vec<Rc<T>> = sticking_items.to_vec();
    for item in positioned_items {
        if item.index() >= first_visible_index && item.index() <= last_visible_index {
            final_visible_items.push(item.clone());
        }
    }
    final_visible_items.sort_by_key(|i| i.index());
    final_visible_items
}

// ---------------------------------------------------------------------------------------------------------------
// Sticky items

/// Where sticky items (a list's headers) stay (Compose's StickyItemsPlacement).
pub(crate) trait StickyItemsPlacement {
    /// Which sticky items stick, given the visible range.
    fn get_sticking_indices(
        &self,
        first_visible_item_index: i32,
        last_visible_item_index: i32,
        sticky_items: &[i32],
    ) -> Vec<i32>;

    /// Where a sticking item goes along the main axis.
    #[allow(clippy::too_many_arguments)]
    fn calculate_sticking_item_offset(
        &self,
        visible_sticky_offsets: &[(i32, i32)],
        item_index: i32,
        item_size: i32,
        item_offset: i32,
        before_content_padding: i32,
        after_content_padding: i32,
        layout_width: i32,
        layout_height: i32,
        is_vertical: bool,
    ) -> i32;
}

/// Sticky items stay at the top until the next one pushes them out (Compose's `StickToTopPlacement`).
pub(crate) struct StickToTopPlacement;

impl StickyItemsPlacement for StickToTopPlacement {
    fn get_sticking_indices(
        &self,
        first_visible_item_index: i32,
        last_visible_item_index: i32,
        sticky_items: &[i32],
    ) -> Vec<i32> {
        if last_visible_item_index - first_visible_item_index < 0 || sticky_items.is_empty() {
            return Vec::new();
        }
        // The sticking one: the last sticky item at or before the first visible item.
        let mut current_header_index = -1;
        for &index in sticky_items {
            if index <= first_visible_item_index {
                current_header_index = index;
            } else {
                break;
            }
        }
        if current_header_index == -1 { Vec::new() } else { vec![current_header_index] }
    }

    fn calculate_sticking_item_offset(
        &self,
        visible_sticky_offsets: &[(i32, i32)],
        item_index: i32,
        item_size: i32,
        item_offset: i32,
        before_content_padding: i32,
        _after_content_padding: i32,
        _layout_width: i32,
        _layout_height: i32,
        _is_vertical: bool,
    ) -> i32 {
        // The next sticky item's offset: it pushes this one out.
        let next_sticky_item_offset =
            visible_sticky_offsets.iter().find(|(index, _)| *index != item_index).map_or(i32::MIN, |(_, o)| *o);
        let mut updated_item_offset =
            if item_offset == i32::MIN { -before_content_padding } else { (-before_content_padding).max(item_offset) };
        if next_sticky_item_offset != i32::MIN {
            updated_item_offset = updated_item_offset.min(next_sticky_item_offset - item_size);
        }
        updated_item_offset
    }
}

fn main_axis_offset<T: LazyLayoutMeasuredItem>(item: &T, is_vertical: bool) -> i32 {
    let offset = item.get_offset(0);
    if is_vertical { offset.y } else { offset.x }
}

/// Takes the sticking items out of the positioned ones (measuring those not composed), positioned where they stick
/// and made not scrollable (Compose's `applyStickyItems`): the sticking items, placed after the others.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_sticky_items<T: LazyLayoutMeasuredItem>(
    placement: Option<&dyn StickyItemsPlacement>,
    first_visible_item_index: i32,
    last_visible_item_index: i32,
    positioned_items: &mut Vec<Rc<T>>,
    sticky_items: &[i32],
    before_content_padding: i32,
    after_content_padding: i32,
    layout_width: i32,
    layout_height: i32,
    is_vertical: bool,
    get_and_measure: &mut dyn FnMut(i32) -> Rc<T>,
) -> Vec<Rc<T>> {
    let Some(placement) = placement else { return Vec::new() };
    if positioned_items.is_empty() || sticky_items.is_empty() {
        return Vec::new();
    }
    let sticking_items =
        placement.get_sticking_indices(first_visible_item_index, last_visible_item_index, sticky_items);
    let mut positioned_sticking_items = Vec::new();
    let visible_sticky_offsets: Vec<(i32, i32)> = positioned_items
        .iter()
        .filter(|i| sticky_items.contains(&i.index()))
        .map(|i| (i.index(), main_axis_offset(&**i, is_vertical)))
        .collect();
    for sticking_index in sticking_items {
        let item_index = positioned_items.iter().position(|i| i.index() == sticking_index);
        let item = match item_index {
            None => get_and_measure(sticking_index),
            Some(at) => positioned_items.remove(at),
        };
        let offset = placement.calculate_sticking_item_offset(
            &visible_sticky_offsets,
            sticking_index,
            item.main_axis_size_with_spacings(is_vertical),
            if item_index.is_none() { i32::MIN } else { main_axis_offset(&*item, is_vertical) },
            before_content_padding,
            after_content_padding,
            layout_width,
            layout_height,
            is_vertical,
        );
        item.make_non_scrollable();
        item.position(offset, 0, layout_width, layout_height);
        positioned_sticking_items.push(item);
    }
    positioned_sticking_items
}
