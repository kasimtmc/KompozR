//! Compose's `androidx.compose.material3.adaptive.navigation` (adaptive-navigation 1.2.0): the three-pane scaffolds'
//! navigators, back behavior, and the scaffolds driven by a navigator with back handling (Android's, ported for
//! KompozR's desktop back).

mod navigable_scaffolds;
mod three_pane_scaffold_navigator;

pub use navigable_scaffolds::{
    NavigableListDetailPaneScaffold, NavigableSupportingPaneScaffold, ThreePaneScaffoldPredictiveBackHandler,
};
pub use three_pane_scaffold_navigator::{
    BackNavigationBehavior, DynThreePaneScaffoldNavigator, NavigatorContentKey, ThreePaneScaffoldNavigator,
    ThreePaneScaffoldNavigatorExt, remember_list_detail_pane_scaffold_navigator,
    remember_list_detail_pane_scaffold_navigator_of_any, remember_supporting_pane_scaffold_navigator,
    remember_supporting_pane_scaffold_navigator_of_any,
};
