//! Compose's `androidx.compose.material3.adaptive.layout` (adaptive-layout 1.2.0, Compose for desktop's actuals):
//! the three-pane scaffolds and what they lay out by.

// (WP8 helper: the layout's internals — texts, delegable semantics, remember helpers)
mod animate_bounds_modifier;
pub(crate) mod internal;
// (WP8 helper: pane expansion, its drag handle and animation helpers)
mod animate_modifier_utils;
mod animate_with_fading_modifier;
mod drag_to_resize_state;
#[cfg(test)]
mod expansion_tests;
mod levitated_pane_scrim;
mod list_detail_pane_scaffold;
mod pane;
mod pane_adapted_value;
mod pane_expansion_draggable_modifier;
mod pane_expansion_state;
mod pane_margins;
mod pane_motion;
mod pane_scaffold;
mod pane_scaffold_directive;
mod pane_scaffold_role;
mod predictive_back_scale_state;
mod three_pane_motion;
mod three_pane_scaffold;
mod three_pane_scaffold_scope;
mod three_pane_scaffold_state;
mod three_pane_scaffold_value;

pub use levitated_pane_scrim::{LevitatedPaneScrim, LevitatedPaneScrimDefaults};
pub use list_detail_pane_scaffold::{
    ListDetailPaneScaffold, ListDetailPaneScaffoldDefaults, ListDetailPaneScaffoldRole, StateListDetailPaneScaffold,
    StateSupportingPaneScaffold, SupportingPaneScaffold, SupportingPaneScaffoldDefaults, SupportingPaneScaffoldRole,
};
pub use pane::{AnimatedPaneOverride, AnimatedPaneOverrideScope, AnimatedPaneScope, LocalAnimatedPaneOverride};
pub use pane_adapted_value::{
    AdaptStrategy, BoundScrim, PaneAdaptedValue, PaneScrim, ThreePaneScaffoldAdaptStrategies,
};
pub use pane_expansion_state::{
    PaneExpansionAnchor, PaneExpansionAnchorOffset, PaneExpansionAnchorOffsetDirection, PaneExpansionState,
    PaneExpansionStateKey, PaneExpansionStateKeyProvider, remember_pane_expansion_state,
    remember_pane_expansion_state_of_key_provider,
};
pub use pane_motion::{
    PaneMotion, PaneMotionData, PaneMotionDefaults, PaneMotionType, PaneScaffoldMotionDataProvider,
    PaneScaffoldMotionDataProviderExt,
};
pub use pane_scaffold::{
    PaneScaffoldPaneScope, PaneScaffoldParentData, PaneScaffoldScope, PaneScaffoldScopeDraggable,
    PaneScaffoldTransitionScope, SemanticsProperties,
};
pub use pane_scaffold_directive::{
    HingePolicy, PaneScaffoldDirective, calculate_pane_scaffold_directive,
    calculate_pane_scaffold_directive_with_two_panes_on_medium_width,
};
pub use pane_scaffold_role::{DynPaneScaffoldRole, PaneScaffoldRole, ThreePaneScaffoldRole};
pub use three_pane_motion::ThreePaneMotion;
pub use three_pane_scaffold::{
    LocalThreePaneScaffoldOverride, ThreePaneDragHandle, ThreePanePane, ThreePaneScaffoldOverride,
    ThreePaneScaffoldOverrideScope,
};
pub use three_pane_scaffold_scope::{ThreePaneScaffoldPaneScope, ThreePaneScaffoldScope};
pub use three_pane_scaffold_state::{MutableThreePaneScaffoldState, ThreePaneScaffoldState};
pub use three_pane_scaffold_value::{
    PaneScaffoldHorizontalOrder, PaneScaffoldValue, ThreePaneScaffoldDestinationItem, ThreePaneScaffoldHorizontalOrder,
    ThreePaneScaffoldValue, TwoPaneExpansionStateKeyImpl, calculate_three_pane_scaffold_value,
    calculate_three_pane_scaffold_value_of_destination,
};
