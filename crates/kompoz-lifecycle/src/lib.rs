//! KompozR's lifecycles, as androidx.lifecycle 2.11's (Compose Multiplatform's desktop artifacts):
//!
//! - lifecycle-common — [`Lifecycle`] with its [`LifecycleState`]s (Kotlin's `Lifecycle.State`) and
//!   [`LifecycleEvent`]s (`Lifecycle.Event`), its observers ([`LifecycleObserver`], [`LifecycleEventObserver`] — a
//!   closure `|source, event| …` —, [`DefaultLifecycleObserver`]), [`LifecycleOwner`], the lifecycle's flows
//!   (`current_state_flow`, `event_flow`) and coroutine scope (`coroutine_scope`, an owner's `lifecycle_scope`);
//! - lifecycle-runtime — [`LifecycleRegistry`] (a lifecycle its owner moves), `repeat_on_lifecycle`,
//!   `with_state_at_least` / `with_created` / `with_started` / `with_resumed`, a flow's `flow_with_lifecycle`;
//! - lifecycle-runtime-compose (androidx.lifecycle.compose) — [`LocalLifecycleOwner`], [`LifecycleEventEffect`],
//!   [`LifecycleStartEffect`], [`LifecycleResumeEffect`], `collect_as_state_with_lifecycle`,
//!   `current_state_as_state`, [`drop_unless_started`] / [`drop_unless_resumed`], [`remember_lifecycle_owner`].
//!
//! The host (kompoz-ui's) is the app's lifecycle owner, provided as [`LocalLifecycleOwner`]: resumed while its
//! window has the focus, started while it is shown without it, created while it is hidden or minimized, destroyed
//! as it goes (what the platform tells it).
//!
//! ```no_run
//! # use kompoz_lifecycle::*;
//! # use kompoz_coroutines::StateFlow;
//! # use kompoz_runtime::kompozable;
//! #[kompozable]
//! fn Unread(count: StateFlow<i32>, shown: impl Fn() + 'static) {
//!     LifecycleEventEffect(LifecycleEvent::OnResume, on_event = shown);
//!     let unread = count.collect_as_state_with_lifecycle(); // not collected in the background
//!     # let _ = unread;
//! }
//! ```

#![allow(non_snake_case, non_upper_case_globals)]

mod compose;
mod lifecycle;
mod pausing;
mod registry;
mod repeat;

pub use compose::{
    FlowLifecycleExtensions, LifecycleEventEffect, LifecyclePauseOrDisposeEffectResult, LifecycleResumeEffect,
    LifecycleResumePauseEffectScope, LifecycleStartEffect, LifecycleStartStopEffectScope,
    LifecycleStopOrDisposeEffectResult, LocalLifecycleOwner, StateFlowLifecycleExtensions, drop_unless_resumed,
    drop_unless_started, remember_lifecycle_owner,
};
pub use lifecycle::{
    DefaultLifecycleObserver, Lifecycle, LifecycleCoroutineScope, LifecycleEvent, LifecycleEventObserver,
    LifecycleImpl, LifecycleObserver, LifecycleOwner, LifecycleOwnerExtensions, LifecycleState,
};
pub use registry::LifecycleRegistry;
pub use repeat::{LifecycleDestroyedException, LifecycleFlowExtensions};
