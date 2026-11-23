//! lifecycle-runtime-compose (androidx.lifecycle.compose): [`LocalLifecycleOwner`], the lifecycle effects, flows
//! collected as state while the lifecycle is active, `currentStateAsState`, `dropUnlessResumed`/`dropUnlessStarted`,
//! `rememberLifecycleOwner`.

mod effect;
mod flow;
mod owner;

pub use effect::{
    LifecycleEventEffect, LifecyclePauseOrDisposeEffectResult, LifecycleResumeEffect, LifecycleResumePauseEffectScope,
    LifecycleStartEffect, LifecycleStartStopEffectScope, LifecycleStopOrDisposeEffectResult,
};
pub use flow::{FlowLifecycleExtensions, StateFlowLifecycleExtensions};
pub use owner::{drop_unless_resumed, drop_unless_started, remember_lifecycle_owner};

use crate::lifecycle::{Lifecycle, LifecycleOwner, LifecycleState};
use kompoz_coroutines::{AsFlow, LaunchedEffect};
use kompoz_runtime::{KompozitionLocal, State, group, kompozable_block, mutable_state_of, remember_keyed};
use std::rc::Rc;

/// The nearest lifecycle owner (Compose's `LocalLifecycleOwner`): the host's (the app's, following its window)
/// unless a navigation entry or [`remember_lifecycle_owner`] gives another below. A static local (Compose's
/// `staticCompositionLocalOf`); not provided, reading it panics ("CompositionLocal LocalLifecycleOwner not present").
pub static LocalLifecycleOwner: KompozitionLocal<Rc<dyn LifecycleOwner>> =
    KompozitionLocal::new_static(|| panic!("CompositionLocal LocalLifecycleOwner not present"));

impl Lifecycle {
    /// Its state as composition state (Compose's `Lifecycle.currentStateAsState()`, of its
    /// [`current_state_flow`](Self::current_state_flow) collected): the caller recomposes as it changes.
    #[track_caller]
    pub fn current_state_as_state(&self) -> State<LifecycleState> {
        group(|| {
            let flow = self.current_state_flow();
            let state = remember_keyed(flow.clone(), || mutable_state_of(flow.value()));
            let s = state.clone();
            let collecting = flow.as_flow().collect(move |v| s.set(v));
            kompozable_block!({
                LaunchedEffect(flow, collecting);
            });
            State::from(state)
        })
    }
}
