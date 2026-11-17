//! Laying out beyond the bounds (Compose's LazyLayoutBeyondBoundsModifierLocal.kt, LazyLayoutBeyondBoundsState.kt,
//! LazyLayoutBeyondBoundsInfo.kt): a focus search moving past the items in view asks the lazy layout for more
//! items that way, one at a time (measured at once), until it finds a target or there are none (at most two views'
//! worth); they go again once it is done. The items asked for and the pinned ones are what the measure keeps
//! composed besides those in view.

use super::item_provider::{LazyLayoutItemProvider, find_index_by_key};
use super::pinnable::{LazyLayoutPinnedItemList, PinnedItem};
use crate::gestures::Orientation;
use kompoz_graphics::LayoutDirection;
use kompoz_ui::focus::{BeyondBoundsDirection, BeyondBoundsLayout, BeyondBoundsScope};
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, node_capabilities};
use std::cell::RefCell;
use std::rc::Rc;

/// What a lazy layout tells its beyond-bounds node (Compose's LazyLayoutBeyondBoundsState).
pub(crate) trait LazyLayoutBeyondBoundsState {
    fn item_count(&self) -> i32;
    fn has_visible_items(&self) -> bool;
    fn first_placed_index(&self) -> i32;
    fn last_placed_index(&self) -> i32;
    fn items_per_viewport(&self) -> i32;
}

/// A range of items laid out beyond the bounds (Compose's `LazyLayoutBeyondBoundsInfo.Interval`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Interval {
    pub(crate) start: i32,
    pub(crate) end: i32,
}

impl Interval {
    fn new(start: i32, end: i32) -> Interval {
        assert!(start >= 0, "negative start index");
        assert!(end >= start, "end index greater than start");
        Interval { start, end }
    }
}

/// The ranges of items laid out beyond the bounds now (Compose's LazyLayoutBeyondBoundsInfo): what the measure
/// keeps besides the items in view.
#[derive(Default)]
pub(crate) struct LazyLayoutBeyondBoundsInfo {
    beyond_bounds_items: RefCell<Vec<Interval>>,
}

impl LazyLayoutBeyondBoundsInfo {
    pub(crate) fn add_interval(&self, start: i32, end: i32) -> Interval {
        let interval = Interval::new(start, end);
        self.beyond_bounds_items.borrow_mut().push(interval);
        interval
    }

    pub(crate) fn remove_interval(&self, interval: Interval) {
        let mut items = self.beyond_bounds_items.borrow_mut();
        if let Some(at) = items.iter().position(|i| *i == interval) {
            items.remove(at);
        }
    }

    pub(crate) fn has_intervals(&self) -> bool {
        !self.beyond_bounds_items.borrow().is_empty()
    }

    /// The first index of them all.
    pub(crate) fn start(&self) -> i32 {
        let min_index = self.beyond_bounds_items.borrow().iter().map(|i| i.start).min().expect("intervals");
        assert!(min_index >= 0, "negative minIndex");
        min_index
    }

    /// The last index of them all.
    pub(crate) fn end(&self) -> i32 {
        self.beyond_bounds_items.borrow().iter().map(|i| i.end).max().expect("intervals")
    }
}

/// The indices the measure keeps besides the items in view (Compose's `calculateLazyLayoutPinnedIndices`): the
/// pinned items (found by key) and the beyond-bounds ones, in order.
pub(crate) fn calculate_lazy_layout_pinned_indices(
    item_provider: &dyn LazyLayoutItemProvider,
    pinned_item_list: &LazyLayoutPinnedItemList,
    beyond_bounds_info: &LazyLayoutBeyondBoundsInfo,
) -> Vec<i32> {
    let pinned_items = pinned_item_list.to_list();
    if !beyond_bounds_info.has_intervals() && pinned_items.is_empty() {
        return Vec::new();
    }
    let item_count = item_provider.item_count();
    let (beyond_bounds_start, beyond_bounds_end) = if beyond_bounds_info.has_intervals() {
        (beyond_bounds_info.start(), beyond_bounds_info.end().min(item_count - 1))
    } else {
        (1, 0) // (an empty range)
    };
    let mut indices = Vec::new();
    for item in &pinned_items {
        let index = find_index_by_key(item_provider, item.key().as_ref(), item.index());
        if index >= beyond_bounds_start && index <= beyond_bounds_end {
            continue;
        }
        if index < 0 || index >= item_count {
            continue;
        }
        indices.push(index);
    }
    for i in beyond_bounds_start..=beyond_bounds_end {
        indices.push(i);
    }
    indices.sort();
    indices
}

/// Lays out at most this many views' worth of items beyond the bounds.
const BeyondBoundsViewportFactor: i32 = 2;

/// The lazy layout's beyond-bounds layout (Compose's `lazyLayoutBeyondBoundsModifier`).
pub(crate) fn lazy_layout_beyond_bounds_modifier(
    modifier: Modifier,
    state: Rc<dyn LazyLayoutBeyondBoundsState>,
    beyond_bounds_info: Rc<LazyLayoutBeyondBoundsInfo>,
    reverse_layout: bool,
    orientation: Orientation,
) -> Modifier {
    modifier.then_element(LazyLayoutBeyondBoundsModifierElement {
        state,
        beyond_bounds_info,
        reverse_layout,
        orientation,
    })
}

struct LazyLayoutBeyondBoundsModifierElement {
    state: Rc<dyn LazyLayoutBeyondBoundsState>,
    beyond_bounds_info: Rc<LazyLayoutBeyondBoundsInfo>,
    reverse_layout: bool,
    orientation: Orientation,
}

impl PartialEq for LazyLayoutBeyondBoundsModifierElement {
    fn eq(&self, o: &Self) -> bool {
        Rc::ptr_eq(&self.state, &o.state)
            && Rc::ptr_eq(&self.beyond_bounds_info, &o.beyond_bounds_info)
            && self.reverse_layout == o.reverse_layout
            && self.orientation == o.orientation
    }
}

impl ModifierNodeElement for LazyLayoutBeyondBoundsModifierElement {
    type Node = LazyLayoutBeyondBoundsProviderModifierNode;
    fn create(&self) -> LazyLayoutBeyondBoundsProviderModifierNode {
        LazyLayoutBeyondBoundsProviderModifierNode {
            state: self.state.clone(),
            beyond_bounds_info: self.beyond_bounds_info.clone(),
            reverse_layout: self.reverse_layout,
            orientation: self.orientation,
            cx: None,
        }
    }
    fn update(&self, node: &mut LazyLayoutBeyondBoundsProviderModifierNode) {
        node.state = self.state.clone();
        node.beyond_bounds_info = self.beyond_bounds_info.clone();
        node.reverse_layout = self.reverse_layout;
        node.orientation = self.orientation;
    }
    fn name(&self) -> &'static str {
        "lazyLayoutBeyondBoundsModifier"
    }
}

/// Compose's LazyLayoutBeyondBoundsProviderModifierNode: the layout's beyond-bounds layout.
struct LazyLayoutBeyondBoundsProviderModifierNode {
    state: Rc<dyn LazyLayoutBeyondBoundsState>,
    beyond_bounds_info: Rc<LazyLayoutBeyondBoundsInfo>,
    reverse_layout: bool,
    orientation: Orientation,
    cx: Option<NodeCx>,
}

impl ModifierNode for LazyLayoutBeyondBoundsProviderModifierNode {
    // (not a layout node as Compose's is, which only measures what it wraps: the search holds this node while the
    // layout measures again)
    node_capabilities!(beyond_bounds_layout);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.cx = Some(cx.clone());
    }

    fn on_detach(&mut self, _: &NodeCx) {
        self.cx = None;
    }
}

impl LazyLayoutBeyondBoundsProviderModifierNode {
    fn is_forward(&self, direction: BeyondBoundsDirection) -> bool {
        let layout_direction = self.cx.as_ref().map_or(LayoutDirection::Ltr, |cx| cx.layout_direction());
        match direction {
            BeyondBoundsDirection::Before => false,
            BeyondBoundsDirection::After => true,
            BeyondBoundsDirection::Above => self.reverse_layout,
            BeyondBoundsDirection::Below => !self.reverse_layout,
            BeyondBoundsDirection::Left => match layout_direction {
                LayoutDirection::Ltr => self.reverse_layout,
                LayoutDirection::Rtl => !self.reverse_layout,
            },
            BeyondBoundsDirection::Right => match layout_direction {
                LayoutDirection::Ltr => !self.reverse_layout,
                LayoutDirection::Rtl => self.reverse_layout,
            },
        }
    }

    fn is_opposite_to_orientation(&self, direction: BeyondBoundsDirection) -> bool {
        match direction {
            BeyondBoundsDirection::Above | BeyondBoundsDirection::Below => self.orientation == Orientation::Horizontal,
            BeyondBoundsDirection::Left | BeyondBoundsDirection::Right => self.orientation == Orientation::Vertical,
            BeyondBoundsDirection::Before | BeyondBoundsDirection::After => false,
        }
    }

    fn has_more_content(&self, interval: Interval, direction: BeyondBoundsDirection) -> bool {
        if self.is_opposite_to_orientation(direction) {
            return false;
        }
        if self.is_forward(direction) { interval.end < self.state.item_count() - 1 } else { interval.start > 0 }
    }

    fn add_next_interval(&self, current: Interval, direction: BeyondBoundsDirection) -> Interval {
        let (mut start, mut end) = (current.start, current.end);
        if self.is_forward(direction) {
            end += 1;
        } else {
            start -= 1;
        }
        self.beyond_bounds_info.add_interval(start, end)
    }

    fn remeasure_sync(&self) {
        if let Some(cx) = &self.cx {
            cx.remeasure_sync();
        }
    }
}

impl BeyondBoundsLayout for LazyLayoutBeyondBoundsProviderModifierNode {
    fn layout(
        &mut self,
        direction: BeyondBoundsDirection,
        block: &mut dyn FnMut(&BeyondBoundsScope) -> Option<bool>,
    ) -> Option<bool> {
        // An empty list, one with nothing in view (no room for an item), or one not laid out yet: no more.
        if self.state.item_count() <= 0 || !self.state.has_visible_items() || self.cx.is_none() {
            return block(&BeyondBoundsScope { has_more_content: false });
        }
        // (a new interval each time: this is re-entrant)
        let start_index =
            if self.is_forward(direction) { self.state.last_placed_index() } else { self.state.first_placed_index() };
        let mut interval = self.beyond_bounds_info.add_interval(start_index, start_index);
        let mut found = None;
        let max_items_to_layout =
            (BeyondBoundsViewportFactor * self.state.items_per_viewport()).min(self.state.item_count());
        let mut items_created_so_far = 0;
        while found.is_none()
            && self.has_more_content(interval, direction)
            && items_created_so_far < max_items_to_layout
        {
            // One more item beyond the bounds.
            let next = self.add_next_interval(interval, direction);
            self.beyond_bounds_info.remove_interval(interval);
            interval = next;
            items_created_so_far += 1;
            self.remeasure_sync();
            // (the items beyond the bounds are there now)
            found = block(&BeyondBoundsScope { has_more_content: self.has_more_content(interval, direction) });
        }
        // The items beyond the bounds go again.
        self.beyond_bounds_info.remove_interval(interval);
        self.remeasure_sync();
        found
    }
}
