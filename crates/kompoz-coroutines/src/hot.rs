//! Hot flows. `MutableStateFlow` holds a value any thread may set (a worker finishing a download); composables read
//! it with `collect_as_state()` and coroutines with `collect`, both on the UI thread. It is conflated, as in
//! Kotlin: a collector that falls behind sees the latest value, and setting the value it already has does nothing.
//!
//! `MutableSharedFlow`: a stream every collector gets the values of, from when it starts (and the last `replay`
//! ones before): one-off events; `try_emit` from any thread. `share_in` and `state_in` share a cold flow as
//! [`SharingStarted`] says: `Eagerly`, `Lazily`, or `WhileSubscribed`, collecting it only while someone collects
//! what it is shared as (and a while after the last one left: a configuration change doesn't restart it).

use crate::executor::{CoroutineScope, CoroutineStart, Job};
use crate::flow::{BoxFuture, Flow, FlowChildren, flow, race};
use crate::time::delay;
use kompoz_runtime::named_args;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::future::Future;
use std::sync::Arc;
use std::task::{Poll, Waker};
use std::time::Duration;

/// Who collects a hot flow: what `WhileSubscribed` watches.
#[derive(Default)]
pub(crate) struct Subscriptions {
    inner: Mutex<(usize, Vec<Waker>)>,
}

impl Subscriptions {
    fn change(&self, f: impl FnOnce(&mut usize)) {
        let wakers = {
            let mut g = self.inner.lock();
            f(&mut g.0);
            std::mem::take(&mut g.1)
        };
        for w in wakers {
            w.wake();
        }
    }
    pub(crate) fn add(&self) {
        self.change(|n| *n += 1);
    }
    pub(crate) fn remove(&self) {
        self.change(|n| *n -= 1);
    }
    pub(crate) fn count(&self) -> usize {
        self.inner.lock().0
    }
    /// Waits until `wanted` says yes to the count.
    pub(crate) fn wait_for(
        self: &Arc<Self>,
        wanted: impl Fn(usize) -> bool + 'static,
    ) -> impl Future<Output = ()> + 'static {
        let s = self.clone();
        std::future::poll_fn(move |cx| {
            let mut g = s.inner.lock();
            if wanted(g.0) {
                Poll::Ready(())
            } else {
                g.1.push(cx.waker().clone());
                Poll::Pending
            }
        })
    }
}

/// A subscription while it lives.
struct Subscribed(Arc<Subscriptions>);
impl Subscribed {
    fn new(s: &Arc<Subscriptions>) -> Subscribed {
        s.add();
        Subscribed(s.clone())
    }
}
impl Drop for Subscribed {
    fn drop(&mut self) {
        self.0.remove();
    }
}

/// When `share_in` and `state_in` collect the flow they share (Kotlin's SharingStarted).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SharingStarted {
    /// At once, never stopped.
    Eagerly,
    /// When the first collector comes, never stopped.
    Lazily,
    /// While someone collects, and `stop_timeout` after the last one left.
    WhileSubscribed { stop_timeout: Duration },
}

impl SharingStarted {
    /// `WhileSubscribed(stopTimeout)`.
    #[allow(non_snake_case)]
    pub fn WhileSubscribed(stop_timeout: Duration) -> SharingStarted {
        SharingStarted::WhileSubscribed { stop_timeout }
    }
}

/// Collects `up` into `sink` in `scope` as `started` says, watching who collects what it is shared as.
fn share<T: 'static>(
    scope: &CoroutineScope,
    started: SharingStarted,
    up: Flow<T>,
    sink: impl Fn(T) + 'static,
    subscriptions: Arc<Subscriptions>,
) -> Job {
    let sink = std::rc::Rc::new(sink);
    // (Kotlin's `launchSharing`: `start = if (started == SharingStarted.Eagerly) CoroutineStart.DEFAULT else
    // CoroutineStart.UNDISPATCHED` — a lazily started one waits for subscribers from the start)
    let start = match started {
        SharingStarted::Eagerly => CoroutineStart::DEFAULT,
        _ => CoroutineStart::UNDISPATCHED,
    };
    let sharing = async move {
        let collect = move || {
            let sink = sink.clone();
            up.collect(move |v| sink(v))
        };
        match started {
            SharingStarted::Eagerly => collect().await,
            SharingStarted::Lazily => {
                subscriptions.wait_for(|n| n > 0).await;
                collect().await;
            }
            SharingStarted::WhileSubscribed { stop_timeout } => loop {
                subscriptions.wait_for(|n| n > 0).await;
                let running = FlowChildren::new();
                // (kotlinx's `collectLatest` of the sharing command: `transformLatest`, UNDISPATCHED)
                running.launch(CoroutineStart::UNDISPATCHED, collect());
                loop {
                    subscriptions.wait_for(|n| n == 0).await;
                    race(delay(stop_timeout), subscriptions.wait_for(|n| n > 0)).await;
                    if subscriptions.count() == 0 {
                        break; // gone for longer than the timeout: stopped (the value kept)
                    }
                }
                drop(running);
            },
        }
    };
    named_args!(scope.launch(start = start, coroutine = sharing))
}

/// What a flow's operators take, for a hot flow's values too (through its `as_flow`).
pub trait AsFlow<T: 'static> {
    /// Its values as a [`Flow`] (each collection a subscription of its own).
    fn as_flow(&self) -> Flow<T>;

    fn collect(&self, on_value: impl FnMut(T) + 'static) -> BoxFuture {
        self.as_flow().collect(on_value)
    }
    /// [`Flow::collect_async`].
    ///
    /// Kotlin: `collect(collector: suspend (T) -> Unit)`
    fn collect_async<Fut: Future<Output = ()> + 'static>(&self, on_value: impl FnMut(T) -> Fut + 'static) -> BoxFuture {
        self.as_flow().collect_async(on_value)
    }
    fn map<R: 'static>(&self, f: impl Fn(T) -> R + 'static) -> Flow<R> {
        self.as_flow().map(f)
    }
    fn filter(&self, keep: impl Fn(&T) -> bool + 'static) -> Flow<T> {
        self.as_flow().filter(keep)
    }
    fn on_each(&self, action: impl Fn(&T) + 'static) -> Flow<T> {
        self.as_flow().on_each(action)
    }
    fn take(&self, count: usize) -> Flow<T> {
        self.as_flow().take(count)
    }
    fn debounce(&self, timeout: Duration) -> Flow<T> {
        self.as_flow().debounce(timeout)
    }
    fn flat_map_latest<R: 'static>(&self, transform: impl Fn(T) -> Flow<R> + 'static) -> Flow<R> {
        self.as_flow().flat_map_latest(transform)
    }
    fn launch_in(&self, scope: &CoroutineScope) -> Job {
        self.as_flow().launch_in(scope)
    }
}

impl<T: 'static> AsFlow<T> for Flow<T> {
    fn as_flow(&self) -> Flow<T> {
        self.clone()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// StateFlow

struct StateCell<T> {
    value: T,
    version: u64,
    waiters: Vec<Waker>,
}

pub(crate) struct StateFlowInner<T> {
    cell: Mutex<StateCell<T>>,
    subscriptions: Arc<Subscriptions>,
}

/// A read-only view of a [`MutableStateFlow`] (Kotlin's `asStateFlow()`). Clones share the value.
pub struct StateFlow<T> {
    pub(crate) inner: Arc<StateFlowInner<T>>,
}

impl<T> Clone for StateFlow<T> {
    fn clone(&self) -> Self {
        StateFlow { inner: self.inner.clone() }
    }
}

/// The same flow, not the same value.
impl<T> PartialEq for StateFlow<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T: Clone + PartialEq + 'static> StateFlow<T> {
    pub fn value(&self) -> T {
        self.inner.cell.lock().value.clone()
    }

    /// How many collect it now.
    pub fn subscription_count(&self) -> usize {
        self.inner.subscriptions.count()
    }

    fn notify(&self, waiters: Vec<Waker>) {
        for w in waiters {
            w.wake();
        }
    }
}

impl<T: Clone + PartialEq + 'static> AsFlow<T> for StateFlow<T> {
    fn as_flow(&self) -> Flow<T> {
        let inner = self.inner.clone();
        flow(move |emit| {
            let inner = inner.clone();
            async move {
                let _subscribed = Subscribed::new(&inner.subscriptions);
                let (mut last, mut seen) = {
                    let c = inner.cell.lock();
                    (c.value.clone(), c.version)
                };
                emit.emit(last.clone()).await;
                loop {
                    let i = inner.clone();
                    let (next, version) = std::future::poll_fn(move |cx| {
                        let mut c = i.cell.lock();
                        if c.version != seen {
                            Poll::Ready((c.value.clone(), c.version))
                        } else {
                            c.waiters.push(cx.waker().clone());
                            Poll::Pending
                        }
                    })
                    .await;
                    seen = version;
                    if next != last {
                        last = next;
                        emit.emit(last.clone()).await;
                    }
                }
            }
        })
    }
}

/// A value any thread may set, collected as a flow (Kotlin's MutableStateFlow). Clones share the value.
pub struct MutableStateFlow<T> {
    flow: StateFlow<T>,
}

impl<T> Clone for MutableStateFlow<T> {
    fn clone(&self) -> Self {
        MutableStateFlow { flow: self.flow.clone() }
    }
}

impl<T> std::ops::Deref for MutableStateFlow<T> {
    type Target = StateFlow<T>;
    fn deref(&self) -> &StateFlow<T> {
        &self.flow
    }
}

impl<T: Clone + PartialEq + 'static> AsFlow<T> for MutableStateFlow<T> {
    fn as_flow(&self) -> Flow<T> {
        self.flow.as_flow()
    }
}

impl<T: Clone + PartialEq + 'static> MutableStateFlow<T> {
    pub fn new(value: T) -> Self {
        MutableStateFlow {
            flow: StateFlow {
                inner: Arc::new(StateFlowInner {
                    cell: Mutex::new(StateCell { value, version: 0, waiters: Vec::new() }),
                    subscriptions: Arc::default(),
                }),
            },
        }
    }

    /// Sets the value (nothing happens if it is the one it has).
    pub fn set_value(&self, value: T) {
        let waiters = {
            let mut c = self.inner.cell.lock();
            if c.value == value {
                return;
            }
            c.value = value;
            c.version += 1;
            std::mem::take(&mut c.waiters)
        };
        self.notify(waiters);
    }

    /// Sets the value to `transform` of it, atomically: retried if another thread set it meanwhile.
    pub fn update(&self, transform: impl Fn(&T) -> T) {
        loop {
            let (current, version) = {
                let c = self.inner.cell.lock();
                (c.value.clone(), c.version)
            };
            let next = transform(&current);
            let waiters = {
                let mut c = self.inner.cell.lock();
                if c.version != version {
                    continue;
                }
                if c.value == next {
                    return;
                }
                c.value = next;
                c.version += 1;
                std::mem::take(&mut c.waiters)
            };
            self.notify(waiters);
            return;
        }
    }

    /// Sets it to `update` if it is `expect` (whether it was).
    pub fn compare_and_set(&self, expect: &T, update: T) -> bool {
        let waiters = {
            let mut c = self.inner.cell.lock();
            if c.value != *expect {
                return false;
            }
            if c.value == update {
                return true;
            }
            c.value = update;
            c.version += 1;
            std::mem::take(&mut c.waiters)
        };
        self.notify(waiters);
        true
    }

    pub fn as_state_flow(&self) -> StateFlow<T> {
        self.flow.clone()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// SharedFlow

struct Subscriber<T> {
    pending: VecDeque<T>,
    waker: Option<Waker>,
}

struct SharedCell<T> {
    cache: VecDeque<T>,
    replay: usize,
    subscribers: Vec<Arc<Mutex<Subscriber<T>>>>,
}

pub(crate) struct SharedFlowInner<T> {
    cell: Mutex<SharedCell<T>>,
    subscriptions: Arc<Subscriptions>,
}

/// A read-only view of a [`MutableSharedFlow`]. Clones are the same flow.
pub struct SharedFlow<T> {
    inner: Arc<SharedFlowInner<T>>,
}

impl<T> Clone for SharedFlow<T> {
    fn clone(&self) -> Self {
        SharedFlow { inner: self.inner.clone() }
    }
}

impl<T> PartialEq for SharedFlow<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T: Clone + Send + 'static> SharedFlow<T> {
    /// The values a new collector gets first.
    pub fn replay_cache(&self) -> Vec<T> {
        self.inner.cell.lock().cache.iter().cloned().collect()
    }
    pub fn subscription_count(&self) -> usize {
        self.inner.subscriptions.count()
    }
}

/// Takes its subscriber out when the collection ends.
struct Unsubscribe<T>(Arc<SharedFlowInner<T>>, Arc<Mutex<Subscriber<T>>>);
impl<T> Drop for Unsubscribe<T> {
    fn drop(&mut self) {
        self.0.cell.lock().subscribers.retain(|s| !Arc::ptr_eq(s, &self.1));
    }
}

impl<T: Clone + Send + 'static> AsFlow<T> for SharedFlow<T> {
    fn as_flow(&self) -> Flow<T> {
        let inner = self.inner.clone();
        flow(move |emit| {
            let inner = inner.clone();
            async move {
                let subscriber = {
                    let mut c = inner.cell.lock();
                    let s = Arc::new(Mutex::new(Subscriber { pending: c.cache.clone(), waker: None }));
                    c.subscribers.push(s.clone());
                    s
                };
                let _unsubscribe = Unsubscribe(inner.clone(), subscriber.clone());
                let _subscribed = Subscribed::new(&inner.subscriptions);
                loop {
                    let s = subscriber.clone();
                    let next = std::future::poll_fn(move |cx| {
                        let mut g = s.lock();
                        match g.pending.pop_front() {
                            Some(v) => Poll::Ready(v),
                            None => {
                                g.waker = Some(cx.waker().clone());
                                Poll::Pending
                            }
                        }
                    })
                    .await;
                    emit.emit(next).await;
                }
            }
        })
    }
}

/// A stream of values every collector gets (Kotlin's MutableSharedFlow); `replay` are kept for newcomers.
pub struct MutableSharedFlow<T> {
    flow: SharedFlow<T>,
}

impl<T> Clone for MutableSharedFlow<T> {
    fn clone(&self) -> Self {
        MutableSharedFlow { flow: self.flow.clone() }
    }
}

impl<T> std::ops::Deref for MutableSharedFlow<T> {
    type Target = SharedFlow<T>;
    fn deref(&self) -> &SharedFlow<T> {
        &self.flow
    }
}

impl<T: Clone + Send + 'static> AsFlow<T> for MutableSharedFlow<T> {
    fn as_flow(&self) -> Flow<T> {
        self.flow.as_flow()
    }
}

impl<T: Clone + Send + 'static> MutableSharedFlow<T> {
    pub fn new(replay: usize) -> Self {
        MutableSharedFlow {
            flow: SharedFlow {
                inner: Arc::new(SharedFlowInner {
                    cell: Mutex::new(SharedCell { cache: VecDeque::new(), replay, subscribers: Vec::new() }),
                    subscriptions: Arc::default(),
                }),
            },
        }
    }

    /// Hands `value` to every collector (from any thread); never waits.
    pub fn try_emit(&self, value: T) -> bool {
        let wakers: Vec<Waker> = {
            let mut c = self.inner.cell.lock();
            if c.replay > 0 {
                c.cache.push_back(value.clone());
                while c.cache.len() > c.replay {
                    c.cache.pop_front();
                }
            }
            c.subscribers
                .iter()
                .filter_map(|s| {
                    let mut g = s.lock();
                    g.pending.push_back(value.clone());
                    g.waker.take()
                })
                .collect()
        };
        for w in wakers {
            w.wake();
        }
        true
    }

    pub async fn emit(&self, value: T) {
        self.try_emit(value);
    }

    pub fn reset_replay_cache(&self) {
        self.inner.cell.lock().cache.clear();
    }

    pub fn as_shared_flow(&self) -> SharedFlow<T> {
        self.flow.clone()
    }
}

impl<T: 'static> Flow<T> {
    /// A [`StateFlow`] of its values, collected in `scope` as `started` says, `initial` until the first.
    pub fn state_in(&self, scope: &CoroutineScope, started: SharingStarted, initial: T) -> StateFlow<T>
    where
        T: Clone + PartialEq + Send,
    {
        let state = MutableStateFlow::new(initial);
        let s = state.clone();
        share(scope, started, self.clone(), move |v| s.set_value(v), state.inner.subscriptions.clone());
        state.as_state_flow()
    }

    /// A [`SharedFlow`] of its values, collected in `scope` as `started` says, the last `replay` kept.
    pub fn share_in(&self, scope: &CoroutineScope, started: SharingStarted, replay: usize) -> SharedFlow<T>
    where
        T: Clone + Send,
    {
        let shared = MutableSharedFlow::new(replay);
        let s = shared.clone();
        share(
            scope,
            started,
            self.clone(),
            move |v| {
                s.try_emit(v);
            },
            shared.inner.subscriptions.clone(),
        );
        shared.as_shared_flow()
    }
}
