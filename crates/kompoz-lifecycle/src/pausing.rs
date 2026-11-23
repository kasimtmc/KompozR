//! lifecycle-common's deprecated `whenStateAtLeast` and its named forms (PausingDispatcher.jvm.kt): a block run on
//! a pausing dispatcher, suspended (not cancelled) while the lifecycle is below a state (its LifecycleController and
//! DispatchQueue) — here a [`DispatchGate`] in the block's context: the block and what it launches in its scope
//! aren't polled while it is paused.

use crate::lifecycle::{Lifecycle, LifecycleEvent, LifecycleObserver, LifecycleOwner, LifecycleState};
use kompoz_coroutines::{CoroutineContext, CoroutineScope, DispatchGate, coroutine_scope, current_job, with_context};
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::task::Waker;

/// Kotlin's `DispatchQueue`: paused at first; what is dispatched while paused waits (here: the coroutines aren't
/// polled, their wakers kept) until resumed or finished.
#[derive(Default)]
struct DispatchQueue {
    paused: Cell<bool>,
    finished: Cell<bool>,
    waiting: RefCell<Vec<Waker>>,
}

impl DispatchQueue {
    fn new() -> DispatchQueue {
        DispatchQueue { paused: Cell::new(true), ..Default::default() }
    }

    fn pause(&self) {
        self.paused.set(true);
    }

    fn resume(&self) {
        if !self.paused.get() {
            return;
        }
        assert!(!self.finished.get(), "Cannot resume a finished dispatcher");
        self.paused.set(false);
        self.drain();
    }

    fn finish(&self) {
        self.finished.set(true);
        self.drain();
    }

    fn can_run(&self) -> bool {
        self.finished.get() || !self.paused.get()
    }

    fn drain(&self) {
        if self.can_run() {
            let waiting = std::mem::take(&mut *self.waiting.borrow_mut());
            for w in waiting {
                w.wake();
            }
        }
    }

    /// The gate of the block's coroutines (Kotlin's `PausingDispatcher.dispatch`: queued while paused).
    fn gate(self: &Rc<Self>) -> DispatchGate {
        let queue = Rc::downgrade(self);
        DispatchGate::new(move |waker| {
            let Some(queue) = queue.upgrade() else { return true };
            if queue.can_run() {
                return true;
            }
            let mut waiting = queue.waiting.borrow_mut();
            if !waiting.iter().any(|w| w.will_wake(waker)) {
                waiting.push(waker.clone());
            }
            false
        })
    }
}

/// Kotlin's `Lifecycle.whenStateAtLeast`: `block` runs (in a `coroutine_scope` of its own) only while `lifecycle` is
/// at least `min_state`, suspended while it is below; destroyed, the caller's coroutine is cancelled.
pub(crate) async fn when_state_at_least<T: 'static, Fut: Future<Output = T> + 'static>(
    lifecycle: Lifecycle,
    min_state: LifecycleState,
    block: impl FnOnce(CoroutineScope) -> Fut + 'static,
) -> T {
    let job = current_job();
    let queue = Rc::new(DispatchQueue::new());
    // Kotlin's LifecycleController.
    let observer: Option<Rc<dyn LifecycleObserver>> = if lifecycle.current_state() == LifecycleState::Destroyed {
        job.cancel();
        queue.finish();
        None
    } else {
        let (q, j) = (queue.clone(), job.clone());
        let observer: Rc<dyn LifecycleObserver> = Rc::new(move |source: &dyn LifecycleOwner, _: LifecycleEvent| {
            let state = source.lifecycle().current_state();
            if state == LifecycleState::Destroyed {
                j.cancel();
                q.finish();
            } else if state < min_state {
                q.pause();
            } else {
                q.resume();
            }
        });
        lifecycle.add_observer(observer.clone());
        Some(observer)
    };
    /// Kotlin's `finally { controller.finish() }`.
    struct Finish(Lifecycle, Option<Rc<dyn LifecycleObserver>>, Rc<DispatchQueue>);
    impl Drop for Finish {
        fn drop(&mut self) {
            if let Some(observer) = self.1.take() {
                self.0.remove_observer(observer);
            }
            self.2.finish();
        }
    }
    let _finish = Finish(lifecycle, observer, queue.clone());
    with_context(CoroutineContext::of(queue.gate()), coroutine_scope(block)).await
}

impl Lifecycle {
    /// Runs `block` while the lifecycle is at least `min_state`, suspended (not cancelled) while it is below; the
    /// caller is cancelled if it is destroyed (deprecated as Compose's: prefer `repeat_on_lifecycle`, or
    /// `with_state_at_least` for work that doesn't suspend).
    #[deprecated(note = "whenStateAtLeast has been deprecated because it runs the block on a pausing dispatcher \
                         that suspends, rather than cancels work when the lifecycle state goes below the given state. \
                         Use withStateAtLeast for non-suspending work that needs to run only once when the Lifecycle \
                         changes.")]
    pub fn when_state_at_least<
        T: 'static,
        Fut: Future<Output = T> + 'static,
        B: FnOnce(CoroutineScope) -> Fut + 'static,
    >(
        &self,
        min_state: LifecycleState,
        block: B,
    ) -> impl Future<Output = T> + 'static + use<T, Fut, B> {
        when_state_at_least(self.clone(), min_state, block)
    }

    /// [`when_state_at_least`](Self::when_state_at_least) `CREATED` (deprecated as Compose's).
    #[deprecated(note = "whenCreated has been deprecated because it runs the block on a pausing dispatcher that \
                         suspends, rather than cancels work when the lifecycle state goes below the given state. Use \
                         withCreated for non-suspending work that needs to run only once when the Lifecycle changes.")]
    pub fn when_created<T: 'static, Fut: Future<Output = T> + 'static, B: FnOnce(CoroutineScope) -> Fut + 'static>(
        &self,
        block: B,
    ) -> impl Future<Output = T> + 'static + use<T, Fut, B> {
        when_state_at_least(self.clone(), LifecycleState::Created, block)
    }

    /// [`when_state_at_least`](Self::when_state_at_least) `STARTED` (deprecated as Compose's).
    #[deprecated(note = "whenStarted has been deprecated because it runs the block on a pausing dispatcher that \
                         suspends, rather than cancels work when the lifecycle state goes below the given state. Use \
                         withStarted for non-suspending work that needs to run only once when the Lifecycle changes.")]
    pub fn when_started<T: 'static, Fut: Future<Output = T> + 'static, B: FnOnce(CoroutineScope) -> Fut + 'static>(
        &self,
        block: B,
    ) -> impl Future<Output = T> + 'static + use<T, Fut, B> {
        when_state_at_least(self.clone(), LifecycleState::Started, block)
    }

    /// [`when_state_at_least`](Self::when_state_at_least) `RESUMED` (deprecated as Compose's).
    #[deprecated(note = "whenResumed has been deprecated because it runs the block on a pausing dispatcher that \
                         suspends, rather than cancels work when the lifecycle state goes below the given state. Use \
                         withResumed for non-suspending work that needs to run only once when the Lifecycle changes.")]
    pub fn when_resumed<T: 'static, Fut: Future<Output = T> + 'static, B: FnOnce(CoroutineScope) -> Fut + 'static>(
        &self,
        block: B,
    ) -> impl Future<Output = T> + 'static + use<T, Fut, B> {
        when_state_at_least(self.clone(), LifecycleState::Resumed, block)
    }
}
