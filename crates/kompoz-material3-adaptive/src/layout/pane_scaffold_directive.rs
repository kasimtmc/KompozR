//! What a pane scaffold lays out by, as Compose's adaptive-layout PaneScaffoldDirective.kt and its internal
//! WindowSizeClassHelper.kt (1.2.0): [`PaneScaffoldDirective`] (partitions, spacers, default pane sizes, excluded
//! bounds), [`calculate_pane_scaffold_directive`] from the window's adaptive info, [`HingePolicy`].

use crate::posture::{Posture, PostureHingeBounds};
use crate::window_adaptive_info::WindowAdaptiveInfo;
use crate::window_core::WindowSizeClass;
use kompoz_graphics::{Dp, Rect};
use kompoz_runtime::kt::KtToString;
use std::fmt;

// ---- WindowSizeClassHelper.kt (internal) ----

/// The lower bounds of the width size classes (Compose's internal `DpWidthSizeClasses`, `WindowSizeClass.WidthSizeClasses`).
pub(crate) mod width_size_classes {
    use super::Dp;
    use crate::window_core::WindowSizeClass;
    pub const Compact: Dp = Dp(0.0);
    pub const Medium: Dp = Dp(WindowSizeClass::WIDTH_DP_MEDIUM_LOWER_BOUND as f32);
    pub const Expanded: Dp = Dp(WindowSizeClass::WIDTH_DP_EXPANDED_LOWER_BOUND as f32);
    #[allow(dead_code)]
    pub const Large: Dp = Dp(1200.0);
    #[allow(dead_code)]
    pub const ExtraLarge: Dp = Dp(1600.0);
}

/// The lower bounds of the height size classes (Compose's internal `DpHeightSizeClasses`).
pub(crate) mod height_size_classes {
    use super::Dp;
    use crate::window_core::WindowSizeClass;
    #[allow(dead_code)]
    pub const Compact: Dp = Dp(0.0);
    #[allow(dead_code)]
    pub const Medium: Dp = Dp(WindowSizeClass::HEIGHT_DP_MEDIUM_LOWER_BOUND as f32);
    pub const Expanded: Dp = Dp(WindowSizeClass::HEIGHT_DP_EXPANDED_LOWER_BOUND as f32);
}

/// A size class's lower bounds in dp (Compose's internal `WindowSizeClass.minWidth` / `minHeight`).
pub(crate) fn min_width(size_class: &WindowSizeClass) -> Dp {
    Dp(size_class.min_width_dp() as f32)
}

pub(crate) fn min_height(size_class: &WindowSizeClass) -> Dp {
    Dp(size_class.min_height_dp() as f32)
}

// ---- PaneScaffoldDirective.kt ----

/// The recommended directive for the window (Compose's `calculatePaneScaffoldDirective`): one horizontal
/// partition on compact and medium widths, two (24 dp apart) on expanded, three on larger ones (panes 412 dp, not
/// 360); two vertical partitions (24 dp apart) in tabletop posture or on a single-partition window of expanded
/// height; the vertical hinges the policy avoids excluded.
///
/// Kotlin: `calculatePaneScaffoldDirective(windowAdaptiveInfo: WindowAdaptiveInfo, verticalHingePolicy: HingePolicy = HingePolicy.AvoidSeparating): PaneScaffoldDirective`
#[kompoz_runtime::named]
pub fn calculate_pane_scaffold_directive(
    window_adaptive_info: &WindowAdaptiveInfo,
    #[default(HingePolicy::AvoidSeparating)] vertical_hinge_policy: HingePolicy,
) -> PaneScaffoldDirective {
    let width = min_width(&window_adaptive_info.window_size_class);
    let (max_horizontal_partitions, horizontal_partition_spacer_size, default_pane_preferred_width) =
        if width == width_size_classes::Compact || width == width_size_classes::Medium {
            (1, Dp(0.0), PaneScaffoldDirective::DefaultPreferredWidth)
        } else if width == width_size_classes::Expanded {
            (2, Dp(24.0), PaneScaffoldDirective::DefaultPreferredWidth)
        } else {
            (3, Dp(24.0), PaneScaffoldDirective::DefaultPreferredWidthXL)
        };
    // TODO(conradchen): Confirm the table top mode settings
    let (max_vertical_partitions, vertical_partition_spacer_size) = if window_adaptive_info.window_posture.is_tabletop
        || (max_horizontal_partitions == 1
            && min_height(&window_adaptive_info.window_size_class) == height_size_classes::Expanded)
    {
        (2, Dp(24.0))
    } else {
        (1, Dp(0.0))
    };
    PaneScaffoldDirective {
        max_horizontal_partitions,
        horizontal_partition_spacer_size,
        max_vertical_partitions,
        vertical_partition_spacer_size,
        default_pane_preferred_width,
        default_pane_preferred_height: PaneScaffoldDirective::DefaultPreferredHeight,
        excluded_bounds: get_excluded_vertical_bounds(&window_adaptive_info.window_posture, vertical_hinge_policy),
    }
}

/// The same with two horizontal partitions (24 dp apart) on medium widths too, and one vertical partition there
/// unless in tabletop posture (Compose's `calculatePaneScaffoldDirectiveWithTwoPanesOnMediumWidth`).
///
/// Kotlin: `calculatePaneScaffoldDirectiveWithTwoPanesOnMediumWidth(windowAdaptiveInfo: WindowAdaptiveInfo, verticalHingePolicy: HingePolicy = HingePolicy.AvoidSeparating): PaneScaffoldDirective`
#[kompoz_runtime::named]
pub fn calculate_pane_scaffold_directive_with_two_panes_on_medium_width(
    window_adaptive_info: &WindowAdaptiveInfo,
    #[default(HingePolicy::AvoidSeparating)] vertical_hinge_policy: HingePolicy,
) -> PaneScaffoldDirective {
    let is_medium_width = min_width(&window_adaptive_info.window_size_class) == width_size_classes::Medium;
    let is_table_top = window_adaptive_info.window_posture.is_tabletop;
    let d = kompoz_runtime::named_args!(calculate_pane_scaffold_directive(
        window_adaptive_info,
        vertical_hinge_policy = vertical_hinge_policy
    ));
    PaneScaffoldDirective {
        max_horizontal_partitions: if is_medium_width { 2 } else { d.max_horizontal_partitions },
        horizontal_partition_spacer_size: if is_medium_width { Dp(24.0) } else { d.horizontal_partition_spacer_size },
        max_vertical_partitions: if is_medium_width && !is_table_top { 1 } else { d.max_vertical_partitions },
        vertical_partition_spacer_size: if is_medium_width && !is_table_top {
            Dp(0.0)
        } else {
            d.vertical_partition_spacer_size
        },
        ..d
    }
}

fn get_excluded_vertical_bounds(posture: &Posture, hinge_policy: HingePolicy) -> Vec<Rect> {
    match hinge_policy {
        HingePolicy::AvoidSeparating => posture.separating_vertical_hinge_bounds(),
        HingePolicy::AvoidOccluding => posture.occluding_vertical_hinge_bounds(),
        HingePolicy::AlwaysAvoid => posture.all_vertical_hinge_bounds(),
        _ => Vec::new(),
    }
}

/// What a pane scaffold lays out by (Compose's `PaneScaffoldDirective`): how many panes side by side
/// (`max_horizontal_partitions`) and the spacer between them, how many stacked (`max_vertical_partitions`) and
/// the spacer between those, a pane's default preferred width and height, and the bounds no pane is put over
/// (hinges, in window coordinates). Equal by its values.
#[derive(Clone, PartialEq, Debug)]
pub struct PaneScaffoldDirective {
    pub max_horizontal_partitions: i32,
    pub horizontal_partition_spacer_size: Dp,
    pub max_vertical_partitions: i32,
    pub vertical_partition_spacer_size: Dp,
    pub default_pane_preferred_width: Dp,
    pub default_pane_preferred_height: Dp,
    pub excluded_bounds: Vec<Rect>,
}

/// Kotlin: `PaneScaffoldDirective(maxHorizontalPartitions: Int, horizontalPartitionSpacerSize: Dp, maxVerticalPartitions: Int, verticalPartitionSpacerSize: Dp, defaultPanePreferredWidth: Dp, defaultPanePreferredHeight: Dp, excludedBounds: List<Rect>)`
#[kompoz_runtime::named(type)]
pub fn PaneScaffoldDirective(
    max_horizontal_partitions: i32,
    horizontal_partition_spacer_size: Dp,
    max_vertical_partitions: i32,
    vertical_partition_spacer_size: Dp,
    default_pane_preferred_width: Dp,
    #[default(PaneScaffoldDirective::DefaultPreferredHeight)] default_pane_preferred_height: Dp,
    excluded_bounds: Vec<Rect>,
) -> PaneScaffoldDirective {
    PaneScaffoldDirective {
        max_horizontal_partitions,
        horizontal_partition_spacer_size,
        max_vertical_partitions,
        vertical_partition_spacer_size,
        default_pane_preferred_width,
        default_pane_preferred_height,
        excluded_bounds,
    }
}

#[kompoz_runtime::named]
#[allow(clippy::too_many_arguments)]
impl PaneScaffoldDirective {
    pub(crate) const DefaultPreferredWidth: Dp = Dp(360.0);
    pub(crate) const DefaultPreferredWidthXL: Dp = Dp(412.0);
    pub(crate) const DefaultPreferredHeight: Dp = Dp(420.0);

    /// The default directive (Compose's `PaneScaffoldDirective.Default`): one pane, 360 by 420 dp preferred, no
    /// spacers, nothing excluded.
    pub fn Default() -> PaneScaffoldDirective {
        PaneScaffoldDirective {
            max_horizontal_partitions: 1,
            horizontal_partition_spacer_size: Dp(0.0),
            max_vertical_partitions: 1,
            vertical_partition_spacer_size: Dp(0.0),
            default_pane_preferred_width: Self::DefaultPreferredWidth,
            default_pane_preferred_height: Self::DefaultPreferredHeight,
            excluded_bounds: Vec::new(),
        }
    }

    /// A copy with the values given (Kotlin's `copy`).
    ///
    /// Kotlin: `copy(maxHorizontalPartitions: Int = this.maxHorizontalPartitions, horizontalPartitionSpacerSize: Dp = this.horizontalPartitionSpacerSize, maxVerticalPartitions: Int = this.maxVerticalPartitions, verticalPartitionSpacerSize: Dp = this.verticalPartitionSpacerSize, defaultPanePreferredWidth: Dp = this.defaultPanePreferredWidth, excludedBounds: List<Rect> = this.excludedBounds, defaultPanePreferredHeight: Dp = this.defaultPanePreferredHeight): PaneScaffoldDirective`
    pub fn copy(
        &self,
        #[default(self.max_horizontal_partitions)] max_horizontal_partitions: i32,
        #[default(self.horizontal_partition_spacer_size)] horizontal_partition_spacer_size: Dp,
        #[default(self.max_vertical_partitions)] max_vertical_partitions: i32,
        #[default(self.vertical_partition_spacer_size)] vertical_partition_spacer_size: Dp,
        #[default(self.default_pane_preferred_width)] default_pane_preferred_width: Dp,
        #[default(self.excluded_bounds.clone())] excluded_bounds: Vec<Rect>,
        #[default(self.default_pane_preferred_height)] default_pane_preferred_height: Dp,
    ) -> PaneScaffoldDirective {
        PaneScaffoldDirective {
            max_horizontal_partitions,
            horizontal_partition_spacer_size,
            max_vertical_partitions,
            vertical_partition_spacer_size,
            default_pane_preferred_width,
            default_pane_preferred_height,
            excluded_bounds,
        }
    }

    /// Whether it lays out one pane at a time (Compose's internal `isSinglePaneLayout`).
    pub(crate) fn is_single_pane_layout(&self) -> bool {
        self.max_horizontal_partitions == 1
    }
}

/// Kotlin's `toString`.
impl fmt::Display for PaneScaffoldDirective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PaneScaffoldDirective(maxHorizontalPartitions={}, horizontalPartitionSpacerSize={}, \
             maxVerticalPartitions={}, verticalPartitionSpacerSize={}, defaultPanePreferredWidth={}, \
             defaultPanePreferredHeight={}, number of excluded bounds={})",
            self.max_horizontal_partitions,
            self.horizontal_partition_spacer_size.kt_to_string(),
            self.max_vertical_partitions,
            self.vertical_partition_spacer_size.kt_to_string(),
            self.default_pane_preferred_width.kt_to_string(),
            self.default_pane_preferred_height.kt_to_string(),
            self.excluded_bounds.len()
        )
    }
}

impl KtToString for PaneScaffoldDirective {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&self.to_string());
    }
}

/// How hinges are addressed in an adaptive layout (Compose's value class `HingePolicy`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct HingePolicy(i32);

impl HingePolicy {
    /// Always avoid where hinges are.
    pub const AlwaysAvoid: HingePolicy = HingePolicy(0);
    /// Avoid separating hinges (and occluding ones, which are separating too).
    pub const AvoidSeparating: HingePolicy = HingePolicy(1);
    /// Avoid only occluding hinges.
    pub const AvoidOccluding: HingePolicy = HingePolicy(2);
    /// Never avoid any hinges, separating or not.
    pub const NeverAvoid: HingePolicy = HingePolicy(3);
}

/// Kotlin's `toString` (its own names: `HingePolicy.AvoidOccludingAndSeparating` for AvoidSeparating, …).
impl fmt::Display for HingePolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match *self {
            HingePolicy::AlwaysAvoid => "AlwaysAvoid",
            HingePolicy::AvoidSeparating => "AvoidOccludingAndSeparating",
            HingePolicy::AvoidOccluding => "AvoidOccludingOnly",
            HingePolicy::NeverAvoid => "NeverAvoid",
            _ => "",
        };
        write!(f, "HingePolicy.{name}")
    }
}

impl KtToString for HingePolicy {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&self.to_string());
    }
}
