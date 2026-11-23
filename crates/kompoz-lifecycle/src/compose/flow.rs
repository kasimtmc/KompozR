//! lifecycle-runtime-compose's FlowExt.kt: `collectAsStateWithLifecycle` of a StateFlow and of a Flow.

use crate::compose::LocalLifecycleOwner;
use crate::lifecycle::{Lifecycle, LifecycleOwner, LifecycleState};
use kompoz_coroutines::{AsFlow, CoroutineContext, Flow, LaunchedEffect, StateFlow, with_context};
use kompoz_runtime::{MutableState, State, group, kompozable_block, mutable_state_of, named, remember};
use std::rc::Rc;

/// Kotlin's `produceState(initialValue, flow, lifecycle, minActiveState, context) { lifecycle.repeatOnLifecycle(…) {
/// flow.collect { value = it } } }`: the state, remembered once; the collection restarted when a key changes.
#[track_caller]
fn collect_with_lifecycle<T, K>(
    key: K,
    flow: Flow<T>,
    initial_value: T,
    lifecycle: Lifecycle,
    min_active_state: LifecycleState,
    context: CoroutineContext,
) -> State<T>
where
    T: PartialEq + Clone + Send + Sync + 'static,
    K: PartialEq + 'static,
{
    group(|| {
        let state: MutableState<T> = remember(|| mutable_state_of(initial_value));
        let s = state.clone();
        let keys = (key, lifecycle.clone(), min_active_state, context.clone());
        let producing = async move {
            lifecycle
                .repeat_on_lifecycle(min_active_state, move |_| {
                    let (flow, s, context) = (flow.clone(), s.clone(), context.clone());
                    async move {
                        if context.is_empty() {
                            flow.collect(move |v| s.set(v)).await;
                        } else {
                            with_context(context, flow.collect(move |v| s.set(v))).await;
                        }
                    }
                })
                .await
        };
        kompozable_block!({
            LaunchedEffect(keys, producing);
        });
        State::from(state)
    })
}

/// What lifecycle-runtime-compose adds to a [`StateFlow`]: its value as state while a lifecycle is active.
#[named(trait StateFlowLifecycleExtensions)]
impl<T: PartialEq + Clone + Send + Sync + 'static> StateFlow<T> {
    /// Its value as composition state, collected only while `lifecycle_owner`'s lifecycle is at least
    /// `min_active_state` (Compose's `StateFlow.collectAsStateWithLifecycle`): its value now at first; below the
    /// state the collection stops (no longer a subscriber: a `WhileSubscribed` upstream lets go), back it catches up.
    #[track_caller]
    fn collect_as_state_with_lifecycle(
        &self,
        #[default(LocalLifecycleOwner.current())] lifecycle_owner: Rc<dyn LifecycleOwner>,
        #[default(LifecycleState::Started)] min_active_state: LifecycleState,
        #[default(CoroutineContext::new())] context: CoroutineContext,
    ) -> State<T> {
        collect_with_lifecycle(
            self.clone(),
            self.as_flow(),
            self.value(),
            lifecycle_owner.lifecycle(),
            min_active_state,
            context,
        )
    }

    /// The same, collected while `lifecycle` is active.
    ///
    /// Kotlin: `collectAsStateWithLifecycle(lifecycle: Lifecycle, minActiveState: Lifecycle.State, context: CoroutineContext): State<T>`
    #[track_caller]
    fn collect_as_state_with_lifecycle_of(
        &self,
        lifecycle: Lifecycle,
        #[default(LifecycleState::Started)] min_active_state: LifecycleState,
        #[default(CoroutineContext::new())] context: CoroutineContext,
    ) -> State<T> {
        collect_with_lifecycle(self.clone(), self.as_flow(), self.value(), lifecycle, min_active_state, context)
    }
}

/// What lifecycle-runtime-compose adds to a [`Flow`]: its latest value as state while a lifecycle is active.
#[named(trait FlowLifecycleExtensions)]
impl<T: PartialEq + Clone + Send + Sync + 'static> Flow<T> {
    /// Its latest value as composition state (`initial_value` until the first), collected only while
    /// `lifecycle_owner`'s lifecycle is at least `min_active_state` (Compose's `Flow.collectAsStateWithLifecycle`).
    #[track_caller]
    fn collect_as_state_with_lifecycle(
        &self,
        initial_value: T,
        #[default(LocalLifecycleOwner.current())] lifecycle_owner: Rc<dyn LifecycleOwner>,
        #[default(LifecycleState::Started)] min_active_state: LifecycleState,
        #[default(CoroutineContext::new())] context: CoroutineContext,
    ) -> State<T> {
        collect_with_lifecycle(
            self.clone(),
            self.clone(),
            initial_value,
            lifecycle_owner.lifecycle(),
            min_active_state,
            context,
        )
    }

    /// The same, collected while `lifecycle` is active.
    ///
    /// Kotlin: `collectAsStateWithLifecycle(initialValue: T, lifecycle: Lifecycle, minActiveState: Lifecycle.State, context: CoroutineContext): State<T>`
    #[track_caller]
    fn collect_as_state_with_lifecycle_of(
        &self,
        initial_value: T,
        lifecycle: Lifecycle,
        #[default(LifecycleState::Started)] min_active_state: LifecycleState,
        #[default(CoroutineContext::new())] context: CoroutineContext,
    ) -> State<T> {
        collect_with_lifecycle(self.clone(), self.clone(), initial_value, lifecycle, min_active_state, context)
    }
}
