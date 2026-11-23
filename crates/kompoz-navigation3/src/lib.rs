//! KompozR's Navigation 3, as navigation3-runtime 1.2's, navigation3-ui 1.2's and lifecycle-viewmodel-navigation3
//! 2.11's (their common source sets; the defaults the Android actuals give, as Compose Multiplatform's desktop has
//! them):
//!
//! - [`runtime`] (androidx.navigation3.runtime) — [`NavKey`](runtime::NavKey), [`NavEntry`](runtime::NavEntry) and its
//!   metadata, the [`entry_provider`](runtime::entry_provider) DSL (`entry<K> { … }`),
//!   [`NavBackStack`](runtime::NavBackStack) and [`remember_nav_back_stack`](runtime::remember_nav_back_stack),
//!   [`NavEntryDecorator`](runtime::NavEntryDecorator)s and
//!   [`remember_decorated_nav_entries`](runtime::remember_decorated_nav_entries), the saveable state holder's
//!   decorator; deep links ([`runtime::deeplink`]) and results between screens;
//! - [`scene`] (androidx.navigation3.scene) — scenes, overlay scenes, scene strategies (a single pane, a dialog) and
//!   scene decorator strategies, the scene state, scenes in the navigation event history;
//! - [`ui`] (androidx.navigation3.ui) — [`NavDisplay`](ui::NavDisplay): scenes animated (a navigation, a pop, a
//!   predictive back gesture's pop through kompoz-navigationevent), shared elements between scenes,
//!   [`LocalNavAnimatedContentScope`](ui::LocalNavAnimatedContentScope);
//! - [`viewmodel`] (lifecycle-viewmodel-navigation3, androidx.lifecycle.viewmodel.navigation3) — the decorator giving
//!   each entry a ViewModel store of its own, cleared as the entry is popped.
//!
//! Compose's module split: Navigation 3's artifacts don't depend on Navigation 2's (kompoz-navigation); one crate here
//! holds its runtime, its ui and lifecycle-viewmodel-navigation3's decorator, a module each (as kompoz-lifecycle holds
//! lifecycle-runtime-compose, kompoz-navigationevent navigationevent-compose).
//!
//! What Kotlin writes `T : Any` for a back stack's keys is [`NavEntryKey`](runtime::NavEntryKey) here (`NavKey`s,
//! `Rc<dyn NavKey>`, texts, numbers); Kotlin's `Any` content and scene keys are
//! [`SlotKey`](kompoz_ui::layout::SlotKey)s; an entry's `Map<String, Any>` metadata is
//! [`Metadata`](runtime::Metadata); `NavDisplay`'s transition states are [`AnyScene`](ui::AnyScene)s (Kotlin's
//! `Scene<*>`).

#![allow(non_snake_case, non_upper_case_globals)]

pub mod runtime;
pub mod scene;
pub mod ui;
pub mod viewmodel;

pub use runtime::*;
pub use scene::*;
pub use ui::*;
pub use viewmodel::*;
