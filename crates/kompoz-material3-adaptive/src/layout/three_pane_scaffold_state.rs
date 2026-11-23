//! A three-pane scaffold's state, as Compose's adaptive-layout ThreePaneScaffoldState.kt (1.2.0): the value it is
//! at and going to, and how far ([`ThreePaneScaffoldState`]); the one an app moves itself
//! ([`MutableThreePaneScaffoldState`]: snap, seek, animate — predictive back's seeking too).

use super::three_pane_scaffold_value::ThreePaneScaffoldValue;
use kompoz_animation::{FiniteAnimationSpec, MutatorMutex, SeekableTransitionState, Transition, remember_transition};
use kompoz_runtime::{MutableState, kompozable, mutable_state_of, named_args};
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

/// The state of a three-pane scaffold (Compose's sealed class `ThreePaneScaffoldState`): the value it is at, the one
/// it is going to, how far it is between them, whether a predictive back gesture moves it.
/// [`MutableThreePaneScaffoldState`] is the one an app moves; a navigator gives one too.
pub trait ThreePaneScaffoldState {
    /// The value it is at (the transition's current state).
    fn current_state(&self) -> ThreePaneScaffoldValue;

    /// The value it is going to.
    fn target_state(&self) -> ThreePaneScaffoldValue;

    /// How far it is from the current value to the target, 0 to 1.
    fn progress_fraction(&self) -> f32;

    /// Whether a predictive back gesture is moving it.
    fn is_predictive_back_in_progress(&self) -> bool;

    /// The transition of its values (Compose's internal composable `rememberTransition`).
    #[doc(hidden)]
    fn remember_transition(&self) -> Transition<ThreePaneScaffoldValue>;
}

/// A scaffold state an app moves itself (Compose's `MutableThreePaneScaffoldState`): from `initial_scaffold_value`,
/// snapped, seeked or animated to another value (one move at a time: a new one cancels the running one). A cheap
/// handle: clones are the same state.
#[derive(Clone)]
pub struct MutableThreePaneScaffoldState {
    inner: Rc<Inner>,
}

struct Inner {
    transition_state: SeekableTransitionState<ThreePaneScaffoldValue>,
    is_predictive_back_in_progress: MutableState<bool>,
    mutator_mutex: MutatorMutex,
}

/// Kotlin: `MutableThreePaneScaffoldState(initialScaffoldValue: ThreePaneScaffoldValue)`
pub fn MutableThreePaneScaffoldState(initial_scaffold_value: ThreePaneScaffoldValue) -> MutableThreePaneScaffoldState {
    MutableThreePaneScaffoldState::new(initial_scaffold_value)
}

/// Equal when the same state.
impl PartialEq for MutableThreePaneScaffoldState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl MutableThreePaneScaffoldState {
    /// A state at `initial_scaffold_value`.
    pub fn new(initial_scaffold_value: ThreePaneScaffoldValue) -> MutableThreePaneScaffoldState {
        MutableThreePaneScaffoldState {
            inner: Rc::new(Inner {
                transition_state: SeekableTransitionState::new(initial_scaffold_value),
                is_predictive_back_in_progress: mutable_state_of(false),
                mutator_mutex: MutatorMutex::new(),
            }),
        }
    }
}

#[kompoz_runtime::named]
impl MutableThreePaneScaffoldState {
    /// Puts it at `target_state`, current and target, at once (Compose's `snapTo`).
    pub fn snap_to(&self, target_state: ThreePaneScaffoldValue) -> impl Future<Output = ()> + 'static {
        let inner = self.inner.clone();
        let mutex = inner.mutator_mutex.clone();
        let body = async move {
            inner.is_predictive_back_in_progress.set(false);
            inner.transition_state.snap_to(target_state).await;
        };
        async move {
            named_args!(mutex.mutate(block = body)).await;
        }
    }

    /// Puts it `fraction` of the way to `target_state` (Compose's `seekTo`), said to be a predictive back
    /// gesture's or not.
    ///
    /// Kotlin: `seekTo(fraction: Float, targetState: ThreePaneScaffoldValue = this.targetState, isPredictiveBackInProgress: Boolean = false)`
    pub fn seek_to(
        &self,
        fraction: f32,
        #[default(self.target_state())] target_state: ThreePaneScaffoldValue,
        #[default(false)] is_predictive_back_in_progress: bool,
    ) -> impl Future<Output = ()> + 'static {
        let inner = self.inner.clone();
        let mutex = inner.mutator_mutex.clone();
        let body = async move {
            inner.is_predictive_back_in_progress.set(is_predictive_back_in_progress);
            named_args!(inner.transition_state.seek_to(fraction, target_state = target_state)).await;
        };
        async move {
            named_args!(mutex.mutate(block = body)).await;
        }
    }

    /// Animates it to `target_state` along `animation_spec` (none: the transition's own) (Compose's
    /// `animateTo`); not said to be a predictive back gesture's once done or cancelled.
    ///
    /// Kotlin: `animateTo(targetState: ThreePaneScaffoldValue = this.targetState, animationSpec: FiniteAnimationSpec<Float>? = null, isPredictiveBackInProgress: Boolean = false)`
    pub fn animate_to(
        &self,
        #[default(self.target_state())] target_state: ThreePaneScaffoldValue,
        #[default(None)] animation_spec: Option<Arc<dyn FiniteAnimationSpec<f32>>>,
        #[default(false)] is_predictive_back_in_progress: bool,
    ) -> impl Future<Output = ()> + 'static {
        let inner = self.inner.clone();
        let mutex = inner.mutator_mutex.clone();
        let body = async move {
            // (Kotlin's `try { … } finally { isPredictiveBackInProgress = false }`: a guard, run when cancelled too)
            struct Finally(MutableState<bool>);
            impl Drop for Finally {
                fn drop(&mut self) {
                    self.0.set(false);
                }
            }
            let _finally = Finally(inner.is_predictive_back_in_progress.clone());
            inner.is_predictive_back_in_progress.set(is_predictive_back_in_progress);
            named_args!(
                inner.transition_state.animate_to(target_state = target_state, animation_spec = animation_spec)
            )
            .await;
        };
        async move {
            named_args!(mutex.mutate(block = body)).await;
        }
    }
}

impl ThreePaneScaffoldState for MutableThreePaneScaffoldState {
    fn current_state(&self) -> ThreePaneScaffoldValue {
        self.inner.transition_state.current_state()
    }

    fn target_state(&self) -> ThreePaneScaffoldValue {
        self.inner.transition_state.target_state()
    }

    fn progress_fraction(&self) -> f32 {
        self.inner.transition_state.fraction()
    }

    fn is_predictive_back_in_progress(&self) -> bool {
        self.inner.is_predictive_back_in_progress.get()
    }

    fn remember_transition(&self) -> Transition<ThreePaneScaffoldValue> {
        remember_scaffold_transition(self.inner.transition_state.clone())
    }
}

#[kompozable]
fn remember_scaffold_transition(
    state: SeekableTransitionState<ThreePaneScaffoldValue>,
) -> Transition<ThreePaneScaffoldValue> {
    remember_transition(state, label = Some("ThreePaneScaffoldState".to_string()))
}
