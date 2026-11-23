//! The navigation suite's own copy of the size classes in dp, as Compose's internal WindowSizeClassHelper.kt of
//! material3-adaptive-navigation-suite (the adaptive crate keeps one of its own): the width and height size classes'
//! lower bounds ([`DpWidthSizeClasses`], [`DpHeightSizeClasses`]) and a window size class's bounds in dp
//! ([`WindowSizeClassDp`]).

use kompoz_graphics::Dp;
use kompoz_material3_adaptive::WindowSizeClass;

/// The lower bounds of the width size classes, in dp (Compose's internal object `DpWidthSizeClasses`): Compact 0,
/// Medium 600, Expanded 840, Large 1200, Extra Large 1600 dp — breakpoints intended to align with the window size
/// class definitions.
pub(crate) struct DpWidthSizeClasses;

#[allow(dead_code)]
impl DpWidthSizeClasses {
    /// The lower bound of the Compact width size class: a window narrower than [`Medium`](Self::Medium) is compact.
    pub(crate) const Compact: Dp = Dp(0.0);
    /// The lower bound of the Medium width size class (window-core's `WIDTH_DP_MEDIUM_LOWER_BOUND`).
    pub(crate) const Medium: Dp = Dp(WindowSizeClass::WIDTH_DP_MEDIUM_LOWER_BOUND as f32);
    /// The lower bound of the Expanded width size class (window-core's `WIDTH_DP_EXPANDED_LOWER_BOUND`).
    pub(crate) const Expanded: Dp = Dp(WindowSizeClass::WIDTH_DP_EXPANDED_LOWER_BOUND as f32);
    /// The lower bound of the Large width size class (the V2 definition of the default width size classes).
    pub(crate) const Large: Dp = Dp(1200.0);
    /// The lower bound of the Extra-Large width size class (the V2 definition).
    pub(crate) const ExtraLarge: Dp = Dp(1600.0);
}

/// The lower bounds of the height size classes, in dp (Compose's internal object `DpHeightSizeClasses`): Compact 0,
/// Medium 480, Expanded 900 dp.
pub(crate) struct DpHeightSizeClasses;

#[allow(dead_code)]
impl DpHeightSizeClasses {
    /// The lower bound of the Compact height size class: a window lower than [`Medium`](Self::Medium) is compact.
    pub(crate) const Compact: Dp = Dp(0.0);
    /// The lower bound of the Medium height size class (window-core's `HEIGHT_DP_MEDIUM_LOWER_BOUND`).
    pub(crate) const Medium: Dp = Dp(WindowSizeClass::HEIGHT_DP_MEDIUM_LOWER_BOUND as f32);
    /// The lower bound of the Expanded height size class (window-core's `HEIGHT_DP_EXPANDED_LOWER_BOUND`).
    pub(crate) const Expanded: Dp = Dp(WindowSizeClass::HEIGHT_DP_EXPANDED_LOWER_BOUND as f32);
}

/// The set of width size classes in dp (Compose's internal `WindowSizeClass.Companion.WidthSizeClasses`).
pub(crate) type WidthSizeClasses = DpWidthSizeClasses;

/// The set of height size classes in dp (Compose's internal `WindowSizeClass.Companion.HeightSizeClasses`).
pub(crate) type HeightSizeClasses = DpHeightSizeClasses;

/// A window size class's lower bounds in [`Dp`] (Compose's internal extension properties `WindowSizeClass.minWidth`
/// and `minHeight`): what tells which size class a window belongs to.
pub(crate) trait WindowSizeClassDp {
    /// The lower bound of the class's width.
    fn min_width(&self) -> Dp;
    /// The lower bound of the class's height.
    fn min_height(&self) -> Dp;
}

impl WindowSizeClassDp for WindowSizeClass {
    fn min_width(&self) -> Dp {
        Dp(self.min_width_dp() as f32)
    }

    fn min_height(&self) -> Dp {
        Dp(self.min_height_dp() as f32)
    }
}
