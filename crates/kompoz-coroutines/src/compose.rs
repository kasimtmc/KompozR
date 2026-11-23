//! Coroutines in the composition: `LaunchedEffect`, `remember_coroutine_scope`, `produce_state`,
//! `collect_as_state`, `snapshot_flow`, and the effect context they run in.

use crate::context::CoroutineContext;
use crate::executor::{CoroutineScope, Dispatcher};
use crate::flow::{Flow, flow};
use crate::hot::{AsFlow, StateFlow};
use kompoz_runtime::{
    DisposableEffect, KompozitionLocal, MutableState, RememberObserver, Snapshot, State, StateKey, group, kompozable,
    kompozable_block, mutable_state_of, observe_reads, on_dispose, remember, remember_keyed, remember_observer,
};
use std::collections::HashSet;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Poll, Waker};

/// The dispatcher the host runs coroutines on: what `LaunchedEffect` and `remember_coroutine_scope` launch on.
/// The host provides it at the root.
pub static LocalDispatcher: KompozitionLocal<Option<Dispatcher>> = KompozitionLocal::new(|| None);

fn dispatcher() -> Dispatcher {
    LocalDispatcher.current().expect("kompoz: no dispatcher provided (LocalDispatcher): the host provides one")
}

/// What the composition's effects run with (Compose's `Recomposer.effectCoroutineContext`, the composer's
/// `applyCoroutineContext`): the host gives it at the root (`KompozHost::with_effect_coroutine_context`; a test rule's
/// has its `InfiniteAnimationPolicy`, an app's window its `MotionDurationScale`), and `LaunchedEffect`,
/// `remember_coroutine_scope`, `produce_state` and the modifier nodes' coroutine scopes run in it. Subcompositions see
/// the same. Read it with [`current_effect_coroutine_context`].
pub static LocalEffectCoroutineContext: KompozitionLocal<CoroutineContext> =
    KompozitionLocal::new_static(CoroutineContext::new);

/// The coroutine context the composition's effects run with, where this composes (Compose's
/// `currentComposer.applyCoroutineContext`): see [`LocalEffectCoroutineContext`].
pub fn current_effect_coroutine_context() -> CoroutineContext {
    LocalEffectCoroutineContext.current()
}

/// Runs `effect` when it enters the composition and again whenever `keys` change (several: a tuple), cancelling
/// the run before; cancelled when it leaves. Launched as Compose launches it (`scope.launch(block = task)` in the
/// composition's effect context, dispatched): it starts once the frame that composed it is done composing and laying
/// out, and what it sets before it first suspends shows in the next frame.
///
/// ```no_run
/// # use kompoz_coroutines::*;
/// # use kompoz_runtime::*;
/// # fn main() {}
/// #[kompozable]
/// fn Clock(seconds: MutableState<i32>) {
///     LaunchedEffect((), async move {
///         loop {
///             delay_ms(1000).await;
///             seconds.update(|s| *s += 1);
///         }
///     });
/// }
/// ```
#[kompozable(inline)]
pub fn LaunchedEffect<K: PartialEq + 'static>(keys: K, effect: impl Future<Output = ()> + 'static) {
    let d = dispatcher();
    let context = current_effect_coroutine_context();
    DisposableEffect(keys, move || {
        let scope = CoroutineScope::new_with_context(&d, context);
        scope.launch(effect);
        on_dispose(move || scope.cancel())
    });
}

/// A scope whose coroutines are cancelled when it leaves the composition.
struct ScopeHolder(CoroutineScope);
impl RememberObserver for ScopeHolder {
    fn on_forgotten(&self) {
        self.0.cancel();
    }
    fn on_abandoned(&self) {
        self.0.cancel();
    }
}

/// A scope bound to the composition: its coroutines are cancelled when the caller leaves it. Launch from event
/// handlers with it: `scope.launch(async move { … })`. It runs them in the composition's effect context (Compose's
/// `rememberCoroutineScope`).
#[track_caller]
pub fn remember_coroutine_scope() -> CoroutineScope {
    let d = dispatcher();
    let context = current_effect_coroutine_context();
    remember_observer(move || ScopeHolder(CoroutineScope::new_with_context(&d, context))).0.clone()
}

/// A state a coroutine produces (restarted when `keys` change): `producer` gets the state and sets it.
#[kompozable]
pub fn produce_state<T, K, Fut>(initial: T, keys: K, producer: impl FnOnce(MutableState<T>) -> Fut) -> T
where
    T: PartialEq + Clone + Send + Sync + 'static,
    K: PartialEq + 'static,
    Fut: Future<Output = ()> + 'static,
{
    let state = remember(|| mutable_state_of(initial));
    LaunchedEffect(keys, producer(state.clone()));
    state.get()
}

impl<T: PartialEq + Clone + Send + Sync + 'static> Flow<T> {
    /// Its latest value as state (`initial` until the first; Compose's `collectAsState(initial)`): what reads it
    /// recomposes as it changes.
    #[track_caller]
    pub fn collect_as_state(&self, initial: T) -> State<T> {
        group(|| {
            let state = remember_keyed(self.clone(), || mutable_state_of(initial));
            let (flow, s) = (self.clone(), state.clone());
            let collecting = flow.collect(move |v| s.set(v));
            let key = self.clone();
            kompozable_block!({
                LaunchedEffect(key, collecting);
            });
            State::from(state)
        })
    }
}

impl<T: PartialEq + Clone + Send + Sync + 'static> StateFlow<T> {
    /// Its value as state (Compose's `collectAsState()`): what reads it recomposes as it changes. Another flow: the
    /// new one's value at once.
    #[track_caller]
    pub fn collect_as_state(&self) -> State<T> {
        group(|| {
            let state = remember_keyed(self.clone(), || mutable_state_of(self.value()));
            let s = state.clone();
            let collecting = self.as_flow().collect(move |v| s.set(v));
            let key = self.clone();
            kompozable_block!({
                LaunchedEffect(key, collecting);
            });
            State::from(state)
        })
    }
}

/// Wakes a coroutine when a state it read changes.
struct ChangeWatch {
    keys: parking_lot::Mutex<(HashSet<StateKey>, bool, Option<Waker>)>,
}

/// Compose's `snapshotFlow { … }`: a flow of the block's value now and whenever a state it read changes it to a
/// different one.
pub fn snapshot_flow<T: PartialEq + Clone + 'static>(block: impl Fn() -> T + 'static) -> Flow<T> {
    let block = Rc::new(block);
    flow(move |emit| {
        let block = block.clone();
        async move {
            let watch = Arc::new(ChangeWatch { keys: parking_lot::Mutex::new((HashSet::new(), false, None)) });
            let w = watch.clone();
            let _observer = Snapshot::register_apply_observer(move |changed| {
                let waker = {
                    let mut g = w.keys.lock();
                    if changed.iter().any(|k| g.0.contains(k)) {
                        g.1 = true;
                        g.2.take()
                    } else {
                        None
                    }
                };
                if let Some(waker) = waker {
                    waker.wake();
                }
            });
            let mut last: Option<T> = None;
            loop {
                // What it reads is watched as it is read: a change after a read wakes it, one before is read.
                {
                    let mut g = watch.keys.lock();
                    g.0.clear();
                    g.1 = false;
                }
                let w = watch.clone();
                let value = observe_reads(
                    Rc::new(move |k| {
                        w.keys.lock().0.insert(k);
                    }),
                    || block(),
                );
                if last.as_ref() != Some(&value) {
                    last = Some(value.clone());
                    emit.emit(value).await;
                }
                let w = watch.clone();
                std::future::poll_fn(move |cx| {
                    let mut g = w.keys.lock();
                    if g.1 {
                        Poll::Ready(())
                    } else {
                        g.2 = Some(cx.waker().clone());
                        Poll::Pending
                    }
                })
                .await;
            }
        }
    })
}
