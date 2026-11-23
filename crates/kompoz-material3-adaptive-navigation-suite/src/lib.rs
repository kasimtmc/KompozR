//! KompozR's Material 3 navigation suite, as Jetpack Compose's
//! `androidx.compose.material3.adaptive.navigationsuite` (material3-adaptive-navigation-suite 1.5.0-alpha17): a
//! scaffold whose navigation is a bar, a rail or a drawer to suit the window.
//!
//! - [`NavigationSuiteScaffold`]: the screen and its navigation component of a [`NavigationSuiteType`] (by default
//!   the one the window suits, [`NavigationSuiteScaffoldDefaults::navigation_suite_type`]) holding
//!   [`NavigationSuiteItem`]s, an optional primary action, hidden and shown by a [`NavigationSuiteScaffoldState`]
//!   ([`remember_navigation_suite_scaffold_state`]);
//! - its parts: [`NavigationSuiteScaffoldLayout`], [`NavigationSuite`], [`NavigationSuiteItem`];
//! - the older API of items given to a [`NavigationSuiteScope`] by a layout type ([`NavigationSuiteScaffoldWithLayoutType`],
//!   [`NavigationSuiteScaffoldLayoutWithLayoutType`], [`NavigationSuiteWithLayoutType`],
//!   [`NavigationSuiteScaffoldDefaults::calculate_from_adaptive_info`]);
//! - colors ([`NavigationSuiteDefaults`], [`NavigationSuiteColors`], [`NavigationSuiteItemColors`]) and the override
//!   API (Compose's experimental `…Override`s, their scopes and composition locals).
//!
//! Compose's `@ExperimentalMaterial3AdaptiveNavigationSuiteApi` opt-in annotation has nothing to port (Kotlin's
//! `@OptIn` is accepted and ignored by the Kotlin syntax).

#![allow(non_upper_case_globals, non_snake_case)]

mod navigation_suite_scaffold;
mod window_size_class_helper;

pub use navigation_suite_scaffold::{
    DefaultNavigationSuiteScaffoldOverride, DefaultNavigationSuiteScaffoldWithPrimaryActionOverride,
    LocalNavigationSuiteScaffoldOverride, LocalNavigationSuiteScaffoldWithPrimaryActionOverride, NavigationSuite,
    NavigationSuiteColors, NavigationSuiteDefaults, NavigationSuiteItem, NavigationSuiteItemColors,
    NavigationSuiteScaffold, NavigationSuiteScaffoldDefaults, NavigationSuiteScaffoldLayout,
    NavigationSuiteScaffoldLayoutWithLayoutType, NavigationSuiteScaffoldOverride, NavigationSuiteScaffoldOverrideScope,
    NavigationSuiteScaffoldState, NavigationSuiteScaffoldValue, NavigationSuiteScaffoldWithLayoutType,
    NavigationSuiteScaffoldWithPrimaryActionOverride, NavigationSuiteScaffoldWithPrimaryActionOverrideScope,
    NavigationSuiteScope, NavigationSuiteType, NavigationSuiteWithLayoutType, remember_navigation_suite_scaffold_state,
};

/// What an app using the navigation suite imports (Compose's `androidx.compose.material3.adaptive.navigationsuite`).
pub mod prelude {
    pub use crate::{
        DefaultNavigationSuiteScaffoldOverride, DefaultNavigationSuiteScaffoldWithPrimaryActionOverride,
        LocalNavigationSuiteScaffoldOverride, LocalNavigationSuiteScaffoldWithPrimaryActionOverride, NavigationSuite,
        NavigationSuiteColors, NavigationSuiteDefaults, NavigationSuiteItem, NavigationSuiteItemColors,
        NavigationSuiteScaffold, NavigationSuiteScaffoldDefaults, NavigationSuiteScaffoldLayout,
        NavigationSuiteScaffoldLayoutWithLayoutType, NavigationSuiteScaffoldOverride,
        NavigationSuiteScaffoldOverrideScope, NavigationSuiteScaffoldState, NavigationSuiteScaffoldValue,
        NavigationSuiteScaffoldWithLayoutType, NavigationSuiteScaffoldWithPrimaryActionOverride,
        NavigationSuiteScaffoldWithPrimaryActionOverrideScope, NavigationSuiteScope, NavigationSuiteType,
        NavigationSuiteWithLayoutType, remember_navigation_suite_scaffold_state,
    };
}
