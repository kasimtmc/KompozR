//! KompozR's navigation (stage 13): androidx.navigation 2.10's common and runtime parts (navigation-common,
//! navigation-runtime) and navigation-compose (`NavHost`, the composable and dialog navigators, the graph builder's
//! `kompozable` / `composable`, `navigation`, `dialog`), their non-Android actuals written from the common and Android
//! code.
//!
//! - The NavHost: [`NavHost`] (and [`NavHost_of`], [`NavHost_graph`]), [`remember_nav_controller`],
//!   `current_back_stack_entry_as_state`, [`ComposeNavigator`], [`DialogNavigator`], [`DialogHost`],
//!   [`DefaultNavTransitions`] — see the `compose` module's docs.
//! - The graph: [`NavDestination`] (route, arguments, deep links, `hierarchy`, `has_route::<T>()`), [`NavGraph`]
//!   (nodes by id, start destination, nested graphs), built with [`NavGraphBuilder`] / [`NavDestinationBuilder`]
//!   (`navigation(start, route) { … }`, `create_graph`).
//! - Arguments: [`NavType`] (the built-ins as constants, `NavType::IntType` …; one's own through [`NavTypeImpl`],
//!   collections through [`CollectionNavType`]), [`NavArgument`], [`nav_argument`]; arguments live in a
//!   [`SavedState`] ([`BundleGetters`]: `entry.arguments()?.get_int("n")`).
//! - Deep links: [`NavDeepLink`] (URI patterns matched as Compose's regular expressions, actions, MIME types),
//!   [`nav_deep_link`], [`nav_deep_link_of`], [`NavDeepLinkRequest`], [`NavUri`].
//! - Options: [`NavOptions`], [`nav_options`] (`pop_up_to(…) { inclusive; save_state }`, `launch_single_top`,
//!   `restore_state`).
//! - The back stack: [`NavBackStackEntry`] (a `LifecycleOwner` capped by its host's lifecycle; its arguments,
//!   `to_route::<T>()`), [`Navigator`], [`NavigatorState`], [`NavigatorProvider`], [`NavGraphNavigator`],
//!   [`NavController`] (navigating by route, typed route, deep link or request; popping; saved stacks; deep links the
//!   host is opened with; its saved state).
//! - Typed routes: `#[derive(Route)]` on a struct (Compose's `@Serializable` route; its base name the type's name,
//!   `#[route(name = "…")]` Kotlin's `@SerialName`; a field with a default — an `Option`, `#[route(default)]` — or a
//!   collection is a query parameter, the others path segments) whose fields are [`RouteValue`]s (enums with
//!   `#[derive(RouteValue)]`); its pattern [`generate_route_pattern`], a value's route [`generate_route_with_args`].
//!
//! ```
//! use kompoz_navigation::*;
//!
//! #[derive(Route)]
//! struct Detail {
//!     id: i32,
//!     tab: Option<String>,
//! }
//!
//! assert_eq!(generate_route_pattern::<Detail>(&TypeMap::default(), None), "Detail/{id}?tab={tab}");
//! ```

#![allow(non_snake_case, non_upper_case_globals)]

extern crate self as kompoz_navigation;

mod argument;
mod builder;
mod compose;
mod controller;
mod deep_link;
mod destination;
mod entry;
mod java;
mod nav_type;
mod navigator;
mod options;
mod regex;
mod saved_state;
mod serialization;
mod uri;

pub use argument::*;
pub use builder::*;
pub use compose::*;
pub use controller::*;
pub use deep_link::*;
pub use destination::*;
pub use entry::*;
pub use kompoz_macros::{Route, RouteValue};
pub use nav_type::*;
pub use navigator::*;
pub use options::*;
pub use saved_state::*;
pub use serialization::*;
pub use uri::NavUri;
