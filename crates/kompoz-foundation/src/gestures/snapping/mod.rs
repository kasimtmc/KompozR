//! Snapping, as Compose's `androidx.compose.foundation.gestures.snapping`: a fling that settles with an item lined
//! up ([`snap_fling_behavior`], [`remember_snap_fling_behavior`]) — where to approach and where to snap told by a
//! [`SnapLayoutInfoProvider`] (a lazy list's: [`lazy_list_snap_layout_info_provider`]), the item's place in the
//! layout by a [`SnapPosition`].

mod lazy_list;
mod snap_fling_behavior;

pub use lazy_list::{lazy_list_snap_layout_info_provider, remember_snap_fling_behavior_for_list};
pub use snap_fling_behavior::{SnapFlingBehavior, remember_snap_fling_behavior, snap_fling_behavior};
// (the grids' and the pager's snapping take them too)
#[allow(unused_imports)]
pub(crate) use snap_fling_behavior::{FinalSnappingItem, calculate_final_offset, calculate_final_snapping_item};

use std::fmt;
use std::rc::Rc;

/// What a snapping fling asks of the layout it snaps (Compose's SnapLayoutInfoProvider): how far to go before
/// snapping, and the offset to the item to snap to.
pub trait SnapLayoutInfoProvider {
    /// How far to fling before snapping, given the fling's velocity and how far a decay would take it (Compose's
    /// `calculateApproachOffset`; that far by default).
    fn calculate_approach_offset(&self, velocity: f32, decay_offset: f32) -> f32 {
        let _ = velocity;
        decay_offset
    }

    /// The offset to the item to snap to, for the velocity left after the approach (Compose's
    /// `calculateSnapOffset`): the closest one for a slow one, the next or previous one else.
    fn calculate_snap_offset(&self, velocity: f32) -> f32;
}

/// Works out where an item snaps to (Compose's SnapPosition's `position`).
pub trait SnapPositionCalculator {
    /// Where an item of `item_size` snaps to, from the layout's start (past `before_content_padding`).
    fn position(
        &self,
        layout_size: i32,
        item_size: i32,
        before_content_padding: i32,
        after_content_padding: i32,
        item_index: i32,
        item_count: i32,
    ) -> i32;
}

/// Where in the layout an item snaps to (Compose's SnapPosition): its middle to the middle, its start to the start,
/// its end to the end, or a place of one's own.
#[derive(Clone)]
pub enum SnapPosition {
    Center,
    Start,
    End,
    Custom(Rc<dyn SnapPositionCalculator>),
}

impl PartialEq for SnapPosition {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SnapPosition::Center, SnapPosition::Center)
            | (SnapPosition::Start, SnapPosition::Start)
            | (SnapPosition::End, SnapPosition::End) => true,
            (SnapPosition::Custom(a), SnapPosition::Custom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for SnapPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            SnapPosition::Center => "Center",
            SnapPosition::Start => "Start",
            SnapPosition::End => "End",
            SnapPosition::Custom(_) => "Custom",
        })
    }
}

impl SnapPosition {
    /// Where an item of `item_size` snaps to, from the layout's start past its padding (Compose's `position`).
    pub fn position(
        &self,
        layout_size: i32,
        item_size: i32,
        before_content_padding: i32,
        after_content_padding: i32,
        item_index: i32,
        item_count: i32,
    ) -> i32 {
        match self {
            SnapPosition::Center => {
                let available_layout_space = layout_size - before_content_padding - after_content_padding;
                // (the middle of the space, half an item back)
                available_layout_space / 2 - item_size / 2
            }
            SnapPosition::Start => 0,
            SnapPosition::End => {
                let available_layout_space = layout_size - before_content_padding - after_content_padding;
                available_layout_space - item_size
            }
            SnapPosition::Custom(c) => c.position(
                layout_size,
                item_size,
                before_content_padding,
                after_content_padding,
                item_index,
                item_count,
            ),
        }
    }
}

/// How far an item at `item_offset` is from where it snaps to (Compose's `calculateDistanceToDesiredSnapPosition`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn calculate_distance_to_desired_snap_position(
    main_axis_view_port_size: i32,
    before_content_padding: i32,
    after_content_padding: i32,
    item_size: i32,
    item_offset: i32,
    item_index: i32,
    snap_position: &SnapPosition,
    item_count: i32,
) -> f32 {
    let desired_distance = snap_position.position(
        main_axis_view_port_size,
        item_size,
        before_content_padding,
        after_content_padding,
        item_index,
        item_count,
    ) as f32;
    item_offset as f32 - desired_distance
}
