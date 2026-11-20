//! What the components' layouts share, as Compose's internal LayoutUtil.kt: the size of a placeable that may be
//! missing, and subtracting from a constraint that may be infinite.
//!
//! Compose's `IntrinsicMeasurable.layoutId` (the `LayoutIdParentData`'s id) is kompoz-ui's own
//! [`IntrinsicMeasurable::layout_id`](kompoz_ui::layout::IntrinsicMeasurable::layout_id).

use kompoz_ui::Constraints;
use kompoz_ui::layout::Placeable;

/// The size of a placeable that may be missing (Compose's `Placeable?.widthOrZero` and `heightOrZero`).
pub(crate) trait PlaceableOrZero {
    /// Its width, 0 where there is none.
    fn width_or_zero(&self) -> i32;
    /// Its height, 0 where there is none.
    fn height_or_zero(&self) -> i32;
}

impl PlaceableOrZero for Option<&Placeable> {
    fn width_or_zero(&self) -> i32 {
        self.map_or(0, Placeable::width)
    }

    fn height_or_zero(&self) -> i32 {
        self.map_or(0, Placeable::height)
    }
}

impl PlaceableOrZero for Option<Placeable> {
    fn width_or_zero(&self) -> i32 {
        self.as_ref().width_or_zero()
    }

    fn height_or_zero(&self) -> i32 {
        self.as_ref().height_or_zero()
    }
}

/// Subtracting from a constraint (Compose's `Int.subtractConstraintSafely`).
pub(crate) trait SubtractConstraintSafely {
    /// `self - other`, at least 0; [`Constraints::Infinity`] stays infinite.
    fn subtract_constraint_safely(self, other: i32) -> i32;
}

impl SubtractConstraintSafely for i32 {
    fn subtract_constraint_safely(self, other: i32) -> i32 {
        if self == Constraints::Infinity {
            return self;
        }
        (self - other).max(0)
    }
}
