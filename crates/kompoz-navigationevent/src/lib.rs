//! KompozR's navigation events, as androidx.navigationevent 1.1's (navigationevent's commonMain — the desktop
//! artifact, 1.0.1, has no code of its own but a lock — and navigationevent-compose's):
//!
//! - navigationevent — [`NavigationEvent`] (a gesture's step or a press, from any platform), the
//!   [`NavigationEventDispatcher`] tree and its [`NavigationEventDispatcherOwner`], [`NavigationEventHandler`] (what
//!   a part of the UI does with back and forward gestures, and what it tells of its navigation: its
//!   [`NavigationEventInfo`]s), [`NavigationEventInput`] (what turns a platform's signals into events) and
//!   [`DirectNavigationEventInput`], the priorities (overlay handlers before the content's), the gesture's
//!   [`NavigationEventTransitionState`] and the [`NavigationEventHistory`], the completion fallbacks;
//! - navigationevent-compose (androidx.navigationevent.compose) — [`LocalNavigationEventDispatcherOwner`],
//!   [`remember_navigation_event_dispatcher_owner`], [`NavigationEventState`] and
//!   [`remember_navigation_event_state`], the [`NavigationEventHandler`](NavigationEventHandler()),
//!   [`NavigationBackHandler`] and [`NavigationForwardHandler`] composables.
//!
//! Compose's module split (Compose UI depends on navigationevent; activity-compose's `BackHandler` and
//! `PredictiveBackHandler`, kompoz-ui's `back`, are built on it). The host (kompoz-ui's) is the root owner, as Compose
//! for desktop's window is: its root dispatcher gets the platform's back (keys, the back button, gestures) through
//! its inputs, and is provided as [`LocalNavigationEventDispatcherOwner`].
//!
//! ```no_run
//! # use kompoz_navigationevent::*;
//! # use kompoz_runtime::*;
//! #[derive(Clone, PartialEq, Debug)]
//! struct Screen(&'static str);
//! impl NavigationEventInfo for Screen {}
//!
//! #[kompozable]
//! fn Detail(close: impl Fn() + 'static) {
//!     let state = remember_navigation_event_state(Screen("detail"), back_info = vec![Screen("list")]);
//!     NavigationBackHandler(state.clone(), on_back_completed = close);
//!     // state.transition_state(): the gesture's progress, for an animation
//! }
//! ```

#![allow(non_snake_case, non_upper_case_globals)]

mod compose;
mod dispatcher;
mod event;
mod handler;
mod input;
mod processor;

pub use compose::{
    LocalNavigationEventDispatcherOwner, LocalNavigationEventDispatcherOwnerLocal, NavigationBackHandler,
    NavigationEventHandler, NavigationEventState, NavigationForwardHandler,
    default_parent_navigation_event_dispatcher_owner, remember_navigation_event_dispatcher_owner,
    remember_navigation_event_state,
};
pub use dispatcher::{
    NavigationEventDispatcher, NavigationEventDispatcherArgs, NavigationEventDispatcherOwner, OnBackCompletedFallback,
    OnForwardCompletedFallback,
};
pub use event::{
    DynEq, NavigationEvent, NavigationEventHistory, NavigationEventInfo, NavigationEventInfoNone,
    NavigationEventTransitionState,
};
pub use handler::{NavigationEventHandler, NavigationEventHandlerCallbacks};
pub use input::{DirectNavigationEventInput, NavigationEventInput, NavigationEventInputCallbacks};
