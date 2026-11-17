//! Flow: a cold stream of values, produced anew for each collector by a coroutine that waits in `emit` until the
//! collector has taken the value (Kotlin's `flow { }`). Operators make new flows; `state_in` turns one into a
//! `StateFlow` kept by a scope (a ViewModel's):
//!
//! ```no_run
//! # use kompoz_coroutines::*;
//! # use std::time::Duration;
//! # fn search(q: String) -> Flow<String> { flow_of([q]) }
//! # fn screen(query: MutableStateFlow<String>, scope: CoroutineScope) {
//! let ticks = flow(|emit| async move {
//!     for i in 0.. {
//!         emit.emit(i).await;
//!         delay_ms(1000).await;
//!     }
//! });
//! let results: StateFlow<String> = query
//!     .debounce(Duration::from_millis(300))
//!     .flat_map_latest(search)
//!     .state_in(&scope, SharingStarted::Eagerly, String::new());
//! # }
//! ```
//!
//! Everything runs on the UI thread, in the collecting coroutine; cancelling it stops the producer too (its
//! future is dropped, running its destructors).

use crate::executor::{CoroutineScope, Dispatcher, Job};
use crate::time::delay;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Poll, Waker};
use std::time::Duration;

pub(crate) type BoxFuture = Pin<Box<dyn Future<Output = ()>>>;

/// What a flow's producer emits to: `emit.emit(value).await` carries on once the collector took the value.
pub struct FlowCollector<T> {
    sink: Rc<dyn Fn(T) -> BoxFuture>,
}

impl<T> Clone for FlowCollector<T> {
    fn clone(&self) -> Self {
        FlowCollector { sink: self.sink.clone() }
    }
}

impl<T> FlowCollector<T> {
    pub(crate) fn new(sink: impl Fn(T) -> BoxFuture + 'static) -> Self {
        FlowCollector { sink: Rc::new(sink) }
    }

    /// Hands `value` to the collector; done once it took it.
    pub fn emit(&self, value: T) -> BoxFuture {
        (self.sink)(value)
    }
}

/// A cold stream of values (Kotlin's Flow). Clones are the same flow.
pub struct Flow<T> {
    producer: Rc<dyn Fn(FlowCollector<T>) -> BoxFuture>,
}

impl<T> Clone for Flow<T> {
    fn clone(&self) -> Self {
        Flow { producer: self.producer.clone() }
    }
}

/// The same flow (what `collect_as_state` and `LaunchedEffect` keys compare).
impl<T> PartialEq for Flow<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.producer, &other.producer)
    }
}

/// A flow made by a coroutine: `flow(|emit| async move { emit.emit(1).await; })`.
pub fn flow<T: 'static, Fut: Future<Output = ()> + 'static>(
    producer: impl Fn(FlowCollector<T>) -> Fut + 'static,
) -> Flow<T> {
    Flow { producer: Rc::new(move |c| Box::pin(producer(c))) }
}

/// A flow of these values (Kotlin's flowOf).
pub fn flow_of<T: Clone + 'static>(values: impl IntoIterator<Item = T>) -> Flow<T> {
    let values: Rc<Vec<T>> = Rc::new(values.into_iter().collect());
    flow(move |emit| {
        let values = values.clone();
        async move {
            for v in values.iter() {
                emit.emit(v.clone()).await;
            }
        }
    })
}

/// A flow of nothing (Kotlin's emptyFlow).
pub fn empty_flow<T: 'static>() -> Flow<T> {
    flow(|_| async {})
}

/// Waits for a [`Signal`] to be raised.
#[derive(Default)]
struct SignalState {
    raised: bool,
    waker: Option<Waker>,
}

/// One coroutine waits for others to raise it (combine, flat_map_latest, debounce).
#[derive(Clone, Default)]
pub(crate) struct Signal(Rc<RefCell<SignalState>>);

impl Signal {
    pub(crate) fn raise(&self) {
        let w = {
            let mut s = self.0.borrow_mut();
            s.raised = true;
            s.waker.take()
        };
        if let Some(w) = w {
            w.wake();
        }
    }
    pub(crate) fn wait(&self) -> impl Future<Output = ()> + 'static {
        let s = self.0.clone();
        std::future::poll_fn(move |cx| {
            let mut s = s.borrow_mut();
            if s.raised {
                s.raised = false;
                Poll::Ready(())
            } else {
                s.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        })
    }
}

/// Done when either is (the other dropped).
pub(crate) async fn race(a: impl Future<Output = ()>, b: impl Future<Output = ()>) {
    let (mut a, mut b) = (Box::pin(a), Box::pin(b));
    std::future::poll_fn(move |cx| {
        if a.as_mut().poll(cx).is_ready() || b.as_mut().poll(cx).is_ready() { Poll::Ready(()) } else { Poll::Pending }
    })
    .await
}

/// Coroutines a flow operator collects its sources in, cancelled when it goes.
pub(crate) struct FlowChildren(pub(crate) CoroutineScope);

impl FlowChildren {
    pub(crate) fn new() -> FlowChildren {
        FlowChildren(CoroutineScope::new(&Dispatcher::current()))
    }
    pub(crate) fn launch(&self, f: impl Future<Output = ()> + 'static) -> Job {
        self.0.launch(f)
    }
}

impl Drop for FlowChildren {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

impl<T: 'static> Flow<T> {
    /// Runs the producer into `collector`.
    pub(crate) fn run(&self, collector: FlowCollector<T>) -> BoxFuture {
        (self.producer)(collector)
    }

    /// Calls `on_value` with each value, until the flow ends (or the collecting coroutine is cancelled).
    pub fn collect(&self, on_value: impl FnMut(T) + 'static) -> BoxFuture {
        let on_value = RefCell::new(on_value);
        self.run(FlowCollector::new(move |v| {
            (on_value.borrow_mut())(v);
            Box::pin(std::future::ready(()))
        }))
    }

    /// The same with a suspending `on_value`: the producer waits until it is done with each value.
    pub fn collect_async<Fut: Future<Output = ()> + 'static>(
        &self,
        on_value: impl FnMut(T) -> Fut + 'static,
    ) -> BoxFuture {
        let on_value = Rc::new(RefCell::new(on_value));
        self.run(FlowCollector::new(move |v| {
            let fut = (on_value.borrow_mut())(v);
            Box::pin(fut)
        }))
    }

    /// A flow of `f` of each value.
    pub fn map<R: 'static>(&self, f: impl Fn(T) -> R + 'static) -> Flow<R> {
        let (up, f) = (self.clone(), Rc::new(f));
        flow(move |down: FlowCollector<R>| {
            let f = f.clone();
            up.collect_async(move |v| down.emit(f(v)))
        })
    }

    /// The values `keep` says yes to.
    pub fn filter(&self, keep: impl Fn(&T) -> bool + 'static) -> Flow<T> {
        let (up, keep) = (self.clone(), Rc::new(keep));
        flow(move |down: FlowCollector<T>| {
            let keep = keep.clone();
            up.collect_async(move |v| if keep(&v) { down.emit(v) } else { Box::pin(std::future::ready(())) })
        })
    }

    /// Calls `action` with each value as it goes by.
    pub fn on_each(&self, action: impl Fn(&T) + 'static) -> Flow<T> {
        let (up, action) = (self.clone(), Rc::new(action));
        flow(move |down: FlowCollector<T>| {
            let action = action.clone();
            up.collect_async(move |v| {
                action(&v);
                down.emit(v)
            })
        })
    }

    /// Any number of values of each: `transform(v, emit)` emits them.
    pub fn transform<R: 'static, Fut: Future<Output = ()> + 'static>(
        &self,
        transform: impl Fn(T, FlowCollector<R>) -> Fut + 'static,
    ) -> Flow<R> {
        let (up, transform) = (self.clone(), Rc::new(transform));
        flow(move |down: FlowCollector<R>| {
            let transform = transform.clone();
            up.collect_async(move |v| transform(v, down.clone()))
        })
    }

    /// The first `count` values; then the producer is stopped.
    pub fn take(&self, count: usize) -> Flow<T> {
        let up = self.clone();
        flow(move |down: FlowCollector<T>| {
            let up = up.clone();
            async move {
                if count == 0 {
                    return;
                }
                let taken = Rc::new(std::cell::Cell::new(0usize));
                let done = Signal::default();
                let d = done.clone();
                let collecting = up.collect_async(move |v| {
                    let n = taken.get() + 1;
                    if n > count {
                        return Box::pin(std::future::pending::<()>()) as BoxFuture; // (dropped below)
                    }
                    taken.set(n);
                    let (down, d) = (down.clone(), d.clone());
                    Box::pin(async move {
                        down.emit(v).await;
                        if n == count {
                            d.raise();
                        }
                    })
                });
                race(done.wait(), collecting).await;
            }
        })
    }

    /// Without values equal to the one before.
    pub fn distinct_until_changed(&self) -> Flow<T>
    where
        T: PartialEq + Clone,
    {
        let up = self.clone();
        flow(move |down: FlowCollector<T>| {
            let last: Rc<RefCell<Option<T>>> = Rc::new(RefCell::new(None));
            up.collect_async(move |v| {
                if last.borrow().as_ref() == Some(&v) {
                    return Box::pin(std::future::ready(())) as BoxFuture;
                }
                *last.borrow_mut() = Some(v.clone());
                down.emit(v)
            })
        })
    }

    /// The values that rest for `timeout` before another comes (and the last one when the flow ends).
    pub fn debounce(&self, timeout: Duration) -> Flow<T> {
        let up = self.clone();
        let timeout = timeout.as_nanos().min(i64::MAX as u128) as i64;
        flow(move |down: FlowCollector<T>| {
            let up = up.clone();
            async move {
                struct Shared<T> {
                    latest: Option<T>,
                    at: i64,
                    done: bool,
                }
                let shared = Rc::new(RefCell::new(Shared { latest: None, at: 0, done: false }));
                let signal = Signal::default();
                let dispatcher = Dispatcher::current();
                let source = FlowChildren::new();
                let (s, sig, d) = (shared.clone(), signal.clone(), dispatcher.clone());
                source.launch(async move {
                    let (s2, sig2) = (s.clone(), sig.clone());
                    up.collect(move |v| {
                        let mut sh = s2.borrow_mut();
                        sh.latest = Some(v);
                        sh.at = d.now();
                        sig2.raise();
                    })
                    .await;
                    s.borrow_mut().done = true;
                    sig.raise();
                });
                loop {
                    let (has, done, at) = {
                        let sh = shared.borrow();
                        (sh.latest.is_some(), sh.done, sh.at)
                    };
                    if has {
                        let now = dispatcher.now();
                        if done || now >= at + timeout {
                            let v = shared.borrow_mut().latest.take().unwrap();
                            down.emit(v).await;
                        } else {
                            race(delay(Duration::from_nanos((at + timeout - now) as u64)), signal.wait()).await;
                        }
                    } else if done {
                        return;
                    } else {
                        signal.wait().await;
                    }
                }
            }
        })
    }

    /// For each value the flow `transform` makes of it, collected until the next value comes (the one before
    /// cancelled).
    pub fn flat_map_latest<R: 'static>(&self, transform: impl Fn(T) -> Flow<R> + 'static) -> Flow<R> {
        let (up, transform) = (self.clone(), Rc::new(transform));
        flow(move |down: FlowCollector<R>| {
            let (up, transform) = (up.clone(), transform.clone());
            async move {
                struct Shared<R> {
                    items: Vec<R>,
                    inner: Option<FlowChildren>,
                    inner_done: bool,
                    done: bool,
                }
                let shared =
                    Rc::new(RefCell::new(Shared { items: Vec::new(), inner: None, inner_done: true, done: false }));
                // The inner collection holds `shared`, and `shared` holds it: cancelled when this goes.
                struct CancelInner<R>(Rc<RefCell<Shared<R>>>);
                impl<R> Drop for CancelInner<R> {
                    fn drop(&mut self) {
                        let inner = self.0.borrow_mut().inner.take();
                        drop(inner);
                    }
                }
                let _cancel_inner = CancelInner(shared.clone());
                let signal = Signal::default();
                let source = FlowChildren::new();
                let (s, sig) = (shared.clone(), signal.clone());
                source.launch(async move {
                    let (s2, sig2) = (s.clone(), sig.clone());
                    up.collect(move |v| {
                        let inner_flow = transform(v);
                        let inner = FlowChildren::new();
                        let scope = inner.0.clone();
                        let old = {
                            let mut sh = s2.borrow_mut();
                            sh.items.clear();
                            sh.inner_done = false;
                            sh.inner.replace(inner)
                        };
                        drop(old); // the one before cancelled
                        let (s3, sig3) = (s2.clone(), sig2.clone());
                        let (s4, sig4) = (s2.clone(), sig2.clone());
                        scope.launch(async move {
                            inner_flow
                                .collect(move |item| {
                                    s3.borrow_mut().items.push(item);
                                    sig3.raise();
                                })
                                .await;
                            s4.borrow_mut().inner_done = true;
                            sig4.raise();
                        });
                    })
                    .await;
                    s.borrow_mut().done = true;
                    sig.raise();
                });
                loop {
                    let next = {
                        let mut sh = shared.borrow_mut();
                        if sh.items.is_empty() { None } else { Some(sh.items.remove(0)) }
                    };
                    match next {
                        Some(item) => down.emit(item).await,
                        None => {
                            let finished = {
                                let sh = shared.borrow();
                                sh.done && sh.inner_done
                            };
                            if finished {
                                return;
                            }
                            signal.wait().await;
                        }
                    }
                }
            }
        })
    }

    /// Calls `on_value` with each value, the call for the value before cancelled where it waits when the next comes
    /// (Kotlin's collectLatest); done once the flow ends and the last call is done.
    pub fn collect_latest<Fut: Future<Output = ()> + 'static>(
        &self,
        on_value: impl Fn(T) -> Fut + 'static,
    ) -> BoxFuture {
        let on_value = Rc::new(on_value);
        self.flat_map_latest(move |v| {
            let (on_value, value) = (on_value.clone(), RefCell::new(Some(v)));
            flow(move |_: FlowCollector<()>| {
                let call = value.borrow_mut().take().map(|v| on_value(v));
                async move {
                    if let Some(call) = call {
                        call.await;
                    }
                }
            })
        })
        .collect(|_| {})
    }

    /// Collects it in `scope`, doing nothing with the values (Kotlin's launchIn: what `on_each` does is the point).
    pub fn launch_in(&self, scope: &CoroutineScope) -> Job {
        scope.launch(self.collect(|_| {}))
    }
}

/// The latest values of two flows, combined, each time either has a new one (once both have one).
pub fn combine<A: Clone + 'static, B: Clone + 'static, R: 'static>(
    a: &Flow<A>,
    b: &Flow<B>,
    combine: impl Fn(&A, &B) -> R + 'static,
) -> Flow<R> {
    let (a, b, combine) = (a.clone(), b.clone(), Rc::new(combine));
    flow(move |down: FlowCollector<R>| {
        let (a, b, combine) = (a.clone(), b.clone(), combine.clone());
        async move {
            struct Shared<A, B> {
                a: Option<A>,
                b: Option<B>,
                dirty: bool,
                done: u8,
            }
            let shared = Rc::new(RefCell::new(Shared { a: None, b: None, dirty: false, done: 0 }));
            let signal = Signal::default();
            let sources = FlowChildren::new();
            let (s, sig) = (shared.clone(), signal.clone());
            sources.launch(async move {
                let (s2, sig2) = (s.clone(), sig.clone());
                a.collect(move |v| {
                    let mut sh = s2.borrow_mut();
                    sh.a = Some(v);
                    sh.dirty = true;
                    sig2.raise();
                })
                .await;
                s.borrow_mut().done += 1;
                sig.raise();
            });
            let (s, sig) = (shared.clone(), signal.clone());
            sources.launch(async move {
                let (s2, sig2) = (s.clone(), sig.clone());
                b.collect(move |v| {
                    let mut sh = s2.borrow_mut();
                    sh.b = Some(v);
                    sh.dirty = true;
                    sig2.raise();
                })
                .await;
                s.borrow_mut().done += 1;
                sig.raise();
            });
            loop {
                let value = {
                    let mut sh = shared.borrow_mut();
                    match (&sh.a, &sh.b) {
                        (Some(x), Some(y)) if sh.dirty => {
                            let r = combine(x, y);
                            sh.dirty = false;
                            Some(r)
                        }
                        _ => None,
                    }
                };
                match value {
                    Some(r) => down.emit(r).await,
                    None => {
                        if shared.borrow().done == 2 {
                            return;
                        }
                        signal.wait().await;
                    }
                }
            }
        }
    })
}
