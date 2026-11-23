//! KompozR's Material 3 adaptive layouts, as Jetpack Compose's `androidx.compose.material3.adaptive` 1.2.0
//! (Compose for desktop's actuals where the platforms differ) and androidx.window's window-core 1.5.0:
//!
//! - the window: its size class ([`WindowSizeClass`], in [`window_core`]) and posture ([`Posture`]),
//!   [`current_window_adaptive_info`];
//! - [`layout`] (Compose's `adaptive-layout`): the three-pane scaffolds ([`ListDetailPaneScaffold`],
//!   [`SupportingPaneScaffold`], [`ThreePaneScaffold`]), what they lay out by ([`PaneScaffoldDirective`],
//!   [`ThreePaneScaffoldValue`], [`calculate_pane_scaffold_directive`]), their panes ([`AnimatedPane`]) and motion,
//!   the draggable pane expansion ([`PaneExpansionState`], `Modifier.pane_expansion_draggable`);
//! - [`navigation`] (Compose's `adaptive-navigation`): the scaffolds' navigators
//!   ([`ThreePaneScaffoldNavigator`], [`remember_list_detail_pane_scaffold_navigator`]) and back behavior.
//!
//! Its modifiers and scopes extend through traits: `use kompoz_material3_adaptive::prelude::*`.

#![allow(non_upper_case_globals, non_snake_case)]

pub mod layout;
pub mod navigation;
mod posture;
mod window_adaptive_info;
pub mod window_core;

pub use posture::{HingeInfo, Posture, PostureHingeBounds};
pub use window_adaptive_info::{
    WindowAdaptiveInfo, current_window_adaptive_info, current_window_adaptive_info_v2, current_window_dp_size,
    current_window_size,
};
#[allow(deprecated)]
pub use window_core::{WindowHeightSizeClass, WindowWidthSizeClass};
pub use window_core::{WindowSizeClass, WindowSizeClassSelectors};

/// What an app using the adaptive layouts imports: the window's info, the scaffolds, their navigators, the
/// extension traits.
pub mod prelude {
    pub use crate::layout::*;
    pub use crate::navigation::*;
    #[allow(deprecated)]
    pub use crate::window_core::{WindowHeightSizeClass, WindowWidthSizeClass};
    pub use crate::window_core::{WindowSizeClass, WindowSizeClassSelectors};
    pub use crate::{
        HingeInfo, Posture, PostureHingeBounds, WindowAdaptiveInfo, current_window_adaptive_info,
        current_window_adaptive_info_v2, current_window_dp_size, current_window_size,
    };
}
