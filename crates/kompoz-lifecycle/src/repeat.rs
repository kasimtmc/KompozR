//! lifecycle-runtime's suspending calls: `repeatOnLifecycle` (RepeatOnLifecycle.kt), `withStateAtLeast` and its
//! named forms (WithLifecycleState.kt), `flowWithLifecycle` (FlowExt.kt).

use crate::lifecycle::{Lifecycle, LifecycleEvent, LifecycleObserver, LifecycleOwner, LifecycleState, RemoveObserver};
use kompoz_coroutines::sync::Mutex;
use kompoz_coroutines::{BufferOverflow, Channel, CoroutineScope, Flow, Job, cancel_current, coroutine_scope, flow};
use kompoz_runtime::named;
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::task::{Poll, Waker};

/// What `with_state_at_least` and its forms fail with when the lifecycle is destroyed before the state is reached
/// (Compose's `LifecycleDestroyedException`, a CancellationException: the coroutine awaiting it is cancelled).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LifecycleDestroyedException;

impl std::fmt::Display for LifecycleDestroyedException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LifecycleDestroyedException")
    }
}

impl std::error::Error for LifecycleDestroyedException {}

/// Wakes a coroutine waiting on an observer (Kotlin's `suspendCancellableCoroutine`'s continuation).
#[derive(Default)]
struct Resume {
    done: Cell<bool>,
    waker: RefCell<Option<Waker>>,
}

impl Resume {
    fn resume(&self) {
        self.done.set(true);
        if let Some(w) = self.waker.borrow_mut().take() {
            w.wake();
        }
    }

    async fn wait(self: Rc<Self>) {
        std::future::poll_fn(move |cx| {
            if self.done.get() {
                Poll::Ready(())
            } else {
                *self.waker.borrow_mut() = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await
    }
}

/// Cancels a job when dropped (Kotlin's `finally { launchedJob?.cancel() }`).
struct CancelJob(Rc<RefCell<Option<Job>>>);

impl Drop for CancelJob {
    fn drop(&mut self) {
        let job = self.0.borrow_mut().take();
        if let Some(job) = job {
            job.cancel();
        }
    }
}

impl Lifecycle {
    /// Runs `block` in a coroutine of its own each time the lifecycle reaches `state`, cancelling it each time it
    /// goes below (Compose's `repeatOnLifecycle`): `STARTED` runs it on `ON_START`, cancels it on `ON_STOP`, again
    /// on the next `ON_START`. Suspends the caller until the lifecycle is destroyed (done at once if it is already);
    /// cancelling the caller cancels it all. Each run starts once the last one is done.
    ///
    /// # Panics
    /// With `INITIALIZED` ("repeatOnLifecycle cannot start work with the INITIALIZED lifecycle state.").
    pub fn repeat_on_lifecycle<Fut: Future<Output = ()> + 'static, B: Fn(CoroutineScope) -> Fut + 'static>(
        &self,
        state: LifecycleState,
        block: B,
    ) -> impl Future<Output = ()> + 'static + use<Fut, B> {
        assert!(
            state != LifecycleState::Initialized,
            "repeatOnLifecycle cannot start work with the INITIALIZED lifecycle state."
        );
        let lifecycle = self.clone();
        let block = Rc::new(block);
        async move {
            if lifecycle.current_state() == LifecycleState::Destroyed {
                return;
            }
            coroutine_scope(move |scope: CoroutineScope| async move {
                // (Kotlin's withContext(Dispatchers.Main.immediate): what runs here runs on the UI thread already)
                if lifecycle.current_state() == LifecycleState::Destroyed {
                    return;
                }
                let launched: Rc<RefCell<Option<Job>>> = Rc::new(RefCell::new(None));
                let _cancel = CancelJob(launched.clone());
                let resume = Rc::new(Resume::default());
                let start_work_event = LifecycleEvent::up_to(state);
                let cancel_work_event = LifecycleEvent::down_from(state);
                let mutex = Mutex(false);
                let (l, r) = (launched.clone(), resume.clone());
                let observer: Rc<dyn LifecycleObserver> =
                    Rc::new(move |_: &dyn LifecycleOwner, event: LifecycleEvent| {
                        if Some(event) == start_work_event {
                            let (block, mutex) = (block.clone(), mutex.clone());
                            let job = scope.launch(async move {
                                mutex.with_lock(None, move || coroutine_scope(move |s| block(s))).await;
                            });
                            *l.borrow_mut() = Some(job);
                            return;
                        }
                        if Some(event) == cancel_work_event {
                            let job = l.borrow_mut().take();
                            if let Some(job) = job {
                                job.cancel();
                            }
                        }
                        if event == LifecycleEvent::OnDestroy {
                            r.resume();
                        }
                    });
                lifecycle.add_observer(observer.clone());
                let _remove = RemoveObserver(lifecycle.clone(), Some(observer));
                resume.wait().await;
            })
            .await
        }
    }

    /// Runs `block` once the lifecycle is at least `state`, at once if it is (Compose's `withStateAtLeast`): its
    /// value. Destroyed before: the caller is cancelled ([`LifecycleDestroyedException`]).
    ///
    /// # Panics
    /// Below `CREATED` ("target state must be CREATED or greater, found …").
    pub fn with_state_at_least<R: 'static, B: FnOnce() -> R + 'static>(
        &self,
        state: LifecycleState,
        block: B,
    ) -> impl Future<Output = R> + 'static + use<R, B> {
        assert!(state >= LifecycleState::Created, "target state must be CREATED or greater, found {state}");
        self.with_state_at_least_unchecked(state, block)
    }

    /// [`with_state_at_least`](Self::with_state_at_least) `CREATED`.
    pub fn with_created<R: 'static, B: FnOnce() -> R + 'static>(
        &self,
        block: B,
    ) -> impl Future<Output = R> + 'static + use<R, B> {
        self.with_state_at_least_unchecked(LifecycleState::Created, block)
    }

    /// [`with_state_at_least`](Self::with_state_at_least) `STARTED`.
    pub fn with_started<R: 'static, B: FnOnce() -> R + 'static>(
        &self,
        block: B,
    ) -> impl Future<Output = R> + 'static + use<R, B> {
        self.with_state_at_least_unchecked(LifecycleState::Started, block)
    }

    /// [`with_state_at_least`](Self::with_state_at_least) `RESUMED`.
    pub fn with_resumed<R: 'static, B: FnOnce() -> R + 'static>(
        &self,
        block: B,
    ) -> impl Future<Output = R> + 'static + use<R, B> {
        self.with_state_at_least_unchecked(LifecycleState::Resumed, block)
    }

    /// Kotlin's `withStateAtLeastUnchecked`: at once when the state is reached (no dispatch is needed on the UI
    /// thread), else when an observer hears the event up to it.
    fn with_state_at_least_unchecked<R: 'static, B: FnOnce() -> R + 'static>(
        &self,
        state: LifecycleState,
        block: B,
    ) -> impl Future<Output = R> + 'static + use<R, B> {
        let lifecycle = self.clone();
        async move {
            let current = lifecycle.current_state();
            if current == LifecycleState::Destroyed {
                return cancel_current().await; // LifecycleDestroyedException
            }
            if current >= state {
                return block();
            }
            let result: Rc<RefCell<Option<R>>> = Rc::new(RefCell::new(None));
            let destroyed = Rc::new(Cell::new(false));
            let resume = Rc::new(Resume::default());
            let (res, d, r) = (result.clone(), destroyed.clone(), resume.clone());
            let block = RefCell::new(Some(block));
            let me: Rc<RefCell<Option<Rc<dyn LifecycleObserver>>>> = Rc::new(RefCell::new(None));
            let (me2, l) = (me.clone(), lifecycle.clone());
            let observer: Rc<dyn LifecycleObserver> = Rc::new(move |_: &dyn LifecycleOwner, event: LifecycleEvent| {
                if Some(event) == LifecycleEvent::up_to(state) {
                    if let Some(me) = me2.borrow_mut().take() {
                        l.remove_observer(me);
                    }
                    if let Some(block) = block.borrow_mut().take() {
                        *res.borrow_mut() = Some(block());
                    }
                    r.resume();
                } else if event == LifecycleEvent::OnDestroy {
                    if let Some(me) = me2.borrow_mut().take() {
                        l.remove_observer(me);
                    }
                    d.set(true);
                    r.resume();
                }
            });
            *me.borrow_mut() = Some(observer.clone());
            lifecycle.add_observer(observer.clone());
            // (cancelled meanwhile: the observer removed — Kotlin's invokeOnCancellation)
            let _remove = RemoveObserver(lifecycle.clone(), Some(observer));
            resume.wait().await;
            if destroyed.get() {
                return cancel_current().await; // LifecycleDestroyedException
            }
            let value = result.borrow_mut().take();
            value.expect("the block ran")
        }
    }
}

/// What lifecycle-runtime adds to a flow (Compose's `Flow.flowWithLifecycle`).
#[named(trait LifecycleFlowExtensions)]
impl<T: 'static> Flow<T> {
    /// The flow's values while `lifecycle` is at least `min_active_state`: the upstream collected each time it gets
    /// there, cancelled each time it goes below; done once it is destroyed.
    fn flow_with_lifecycle(
        &self,
        lifecycle: Lifecycle,
        #[default(LifecycleState::Started)] min_active_state: LifecycleState,
    ) -> Flow<T> {
        let upstream = self.clone();
        flow(move |collector| {
            let (upstream, lifecycle) = (upstream.clone(), lifecycle.clone());
            async move {
                // (Kotlin's callbackFlow: what the upstream gives sent to a buffered channel, closed when the
                // lifecycle is destroyed; the collector takes from it)
                let channel = Channel::<T>::new(64, BufferOverflow::Suspend);
                let ch = channel.clone();
                coroutine_scope(move |scope: CoroutineScope| async move {
                    let (producer, closing) = (ch.clone(), ch.clone());
                    scope.launch(async move {
                        lifecycle
                            .repeat_on_lifecycle(min_active_state, move |_| {
                                let (upstream, producer) = (upstream.clone(), producer.clone());
                                async move {
                                    upstream
                                        .collect_async(move |v| {
                                            let producer = producer.clone();
                                            async move {
                                                producer.send(v).await;
                                            }
                                        })
                                        .await
                                }
                            })
                            .await;
                        closing.close();
                    });
                    while let Some(v) = ch.receive().await {
                        collector.emit(v).await;
                    }
                })
                .await
            }
        })
    }
}
