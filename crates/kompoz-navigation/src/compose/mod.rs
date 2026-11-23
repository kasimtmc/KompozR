//! navigation-compose 2.10 (androidx.navigation.compose; its non-Android actuals written from the common and
//! Android code): [`NavHost`] and its forms ([`NavHost_of`] of typed routes, [`NavHost_graph`] of a graph),
//! [`remember_nav_controller`], `NavController::current_back_stack_entry_as_state`, the navigators of composable
//! and dialog destinations ([`ComposeNavigator`], [`DialogNavigator`], [`DialogHost`]), the graph builder's
//! `kompozable` / `composable`, `navigation` and `dialog` (each of a route, a typed route `_of::<T>` or a route
//! class `_class`), an entry's owners in its content ([`NavBackStackEntry::LocalOwnersProvider`](crate::NavBackStackEntry::LocalOwnersProvider)),
//! [`DefaultNavTransitions`].
//!
//! ```no_run
//! use kompoz_navigation::*;
//! use kompoz_runtime::kompozable;
//!
//! #[kompozable]
//! fn App() {
//!     let nav = remember_nav_controller();
//!     let n = nav.clone();
//!     NavHost(nav, "home", builder = move |graph: &mut NavGraphBuilder| {
//!         let n = n.clone();
//!         graph.kompozable("home", move |_, _| {
//!             let _ = &n; // a screen; n.navigate("detail/7") goes on
//!         });
//!         graph.kompozable("detail/{id}", |_, entry| {
//!             let _id = entry.arguments().and_then(|a| a.get_string("id"));
//!         });
//!     });
//! }
//! ```

mod builder;
mod dialog;
mod host;
mod navigator;
mod provider;

pub use builder::{ComposeNavigatorDestinationBuilder, DialogNavigatorDestinationBuilder};
pub use dialog::{DialogHost, DialogNavigator, DialogNavigatorDestination};
pub use host::{
    DefaultNavTransitions, NavBackStackEntryInfo, NavHost, NavHost_graph, NavHost_of, NavHostEnterTransition,
    NavHostExitTransition, NavHostSizeTransform, remember_nav_controller,
};
pub use navigator::{
    ComposeNavigator, ComposeNavigatorDestination, DestinationContent, DestinationEnterTransition,
    DestinationExitTransition, DestinationPredictiveEnterTransition, DestinationPredictiveExitTransition,
    DestinationSizeTransform, NavTransitionScope,
};
