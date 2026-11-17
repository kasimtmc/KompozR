//! Compose's RowColumnImpl.kt `LayoutOrientation` and `OrientationIndependentConstraints` (what the flow layouts
//! and FlexBox use of them): constraints along a main and a cross axis, for the layouts that run one way or the
//! other.

use kompoz_ui::Constraints;

/// Which way a layout's main axis runs (Compose's LayoutOrientation).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum LayoutOrientation {
    Horizontal,
    Vertical,
}

/// Constraints along a main and a cross axis (Compose's OrientationIndependentConstraints, a `Constraints` with
/// the main axis as its width): made valid as `Constraints` are.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct OrientationIndependentConstraints(Constraints);

impl OrientationIndependentConstraints {
    pub(crate) fn new(
        main_axis_min: i32,
        main_axis_max: i32,
        cross_axis_min: i32,
        cross_axis_max: i32,
    ) -> OrientationIndependentConstraints {
        OrientationIndependentConstraints(Constraints::new(
            main_axis_min,
            main_axis_max,
            cross_axis_min,
            cross_axis_max,
        ))
    }

    /// `c` seen along `orientation`.
    pub(crate) fn from(c: Constraints, orientation: LayoutOrientation) -> OrientationIndependentConstraints {
        let horizontal = orientation == LayoutOrientation::Horizontal;
        OrientationIndependentConstraints::new(
            if horizontal { c.min_width } else { c.min_height },
            if horizontal { c.max_width } else { c.max_height },
            if horizontal { c.min_height } else { c.min_width },
            if horizontal { c.max_height } else { c.max_width },
        )
    }

    pub(crate) fn main_axis_min(self) -> i32 {
        self.0.min_width
    }
    pub(crate) fn main_axis_max(self) -> i32 {
        self.0.max_width
    }
    pub(crate) fn cross_axis_min(self) -> i32 {
        self.0.min_height
    }
    pub(crate) fn cross_axis_max(self) -> i32 {
        self.0.max_height
    }

    /// These as width and height constraints for `orientation` (Compose's `toBoxConstraints`).
    pub(crate) fn to_box_constraints(self, orientation: LayoutOrientation) -> Constraints {
        if orientation == LayoutOrientation::Horizontal {
            Constraints::new(self.main_axis_min(), self.main_axis_max(), self.cross_axis_min(), self.cross_axis_max())
        } else {
            Constraints::new(self.cross_axis_min(), self.cross_axis_max(), self.main_axis_min(), self.main_axis_max())
        }
    }

    /// A copy with the values given replaced (Compose's `copy`).
    pub(crate) fn copy(
        self,
        main_axis_min: Option<i32>,
        main_axis_max: Option<i32>,
        cross_axis_min: Option<i32>,
        cross_axis_max: Option<i32>,
    ) -> OrientationIndependentConstraints {
        OrientationIndependentConstraints::new(
            main_axis_min.unwrap_or(self.main_axis_min()),
            main_axis_max.unwrap_or(self.main_axis_max()),
            cross_axis_min.unwrap_or(self.cross_axis_min()),
            cross_axis_max.unwrap_or(self.cross_axis_max()),
        )
    }
}
