//! `Modifier.pointer_input`, as Compose's `SuspendingPointerInputFilter.kt`: a coroutine per node, started by the
//! node's first event, restarted when its keys change, cancelled when the node is detached. It awaits events in
//! [`AwaitPointerEventScope`]s; an event resumes the coroutine awaiting it in place (while the event is
//! dispatched), so what it consumed is what the next handler sees.

use super::hit_test::minimum_touch_target_padding;
use super::{PointerEvent, PointerEventPass, PointerInputChange, PointerInputModifierNode, when_node_released};
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle};
use crate::node_capabilities;
use crate::platform::{LocalViewConfiguration, ViewConfiguration};
use kompoz_coroutines::{CoroutineStart, Delay, Dispatcher, Job, delay_ms};
use kompoz_graphics::{Density, IntSize, Size};
use kompoz_runtime::named_args;
use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

/// A pointer input block, type-erased.
type Handler = Rc<dyn Fn(PointerInputScope) -> Pin<Box<dyn Future<Output = ()>>>>;

fn handler_of<F, Fut>(block: F) -> Handler
where
    F: Fn(PointerInputScope) -> Fut + 'static,
    Fut: Future<Output = ()> + 'static,
{
    Rc::new(move |scope| Box::pin(block(scope)))
}

/// The time a timeout waits before its last few milliseconds (Compose's `WITH_TIMEOUT_MICRO_DELAY_MILLIS`).
const WITH_TIMEOUT_MICRO_DELAY_MILLIS: i64 = 8;

/// An await of an event: the pass it waits for, the event once handed, the coroutine to wake.
struct Awaiter {
    pass: PointerEventPass,
    event: RefCell<Option<PointerEvent>>,
    waker: RefCell<Option<Waker>>,
}

/// An `await_pointer_event_scope` block running (Compose's PointerEventHandlerCoroutine): what it awaits.
#[derive(Default)]
struct HandlerCoroutine {
    awaiter: RefCell<Option<Rc<Awaiter>>>,
}

impl HandlerCoroutine {
    /// Hands `event` to it if it awaits this pass, waking it (Compose's `offerPointerEvent`).
    fn offer(&self, event: &PointerEvent, pass: PointerEventPass) {
        let awaiter = self.awaiter.borrow().clone();
        if let Some(a) = awaiter
            && a.pass == pass
        {
            *self.awaiter.borrow_mut() = None;
            *a.event.borrow_mut() = Some(event.clone());
            let waker = a.waker.borrow_mut().take();
            if let Some(w) = waker {
                w.wake();
            }
        }
    }
}

/// What a node and its scopes share: its coroutine, its blocks awaiting events, the event under way.
struct Shared {
    handle: RefCell<Option<NodeHandle>>,
    dispatcher: RefCell<Option<Dispatcher>>,
    handler: RefCell<Handler>,
    job: RefCell<Option<Job>>,
    current_event: RefCell<PointerEvent>,
    handlers: RefCell<Vec<Rc<HandlerCoroutine>>>,
    last_pointer_event: RefCell<Option<PointerEvent>>,
    bounds_size: Cell<IntSize>,
    intercept_out_of_bounds_child_events: Cell<bool>,
}

impl Shared {
    fn new(handler: Handler) -> Rc<Shared> {
        Rc::new(Shared {
            handle: RefCell::new(None),
            dispatcher: RefCell::new(None),
            handler: RefCell::new(handler),
            job: RefCell::new(None),
            current_event: RefCell::new(PointerEvent::default()),
            handlers: RefCell::new(Vec::new()),
            last_pointer_event: RefCell::new(None),
            bounds_size: Cell::new(IntSize::Zero),
            intercept_out_of_bounds_child_events: Cell::new(false),
        })
    }

    fn handle(&self) -> Option<NodeHandle> {
        self.handle.borrow().clone()
    }

    fn view_configuration(&self) -> ViewConfiguration {
        match self.handle() {
            Some(h) => h.current_value_of(&LocalViewConfiguration),
            None => ViewConfiguration::default(),
        }
    }

    fn density(&self) -> Density {
        self.handle().map_or_else(Density::default, |h| h.density())
    }

    fn extended_touch_padding(&self) -> Size {
        minimum_touch_target_padding(self.bounds_size.get(), self.view_configuration().minimum_touch_target_size)
    }

    /// Compose's `onPointerEvent`, run once the node isn't borrowed: the block started if it isn't, then the
    /// event handed to what awaits it.
    fn on_pointer_event(self: &Rc<Self>, event: PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        self.bounds_size.set(bounds);
        if pass == PointerEventPass::Initial {
            *self.current_event.borrow_mut() = event.clone();
        }
        if self.job.borrow().is_none() {
            self.launch();
        }
        self.dispatch(&event, pass);
        let all_up = event.changes.iter().all(|c| c.changed_to_up_ignore_consumed());
        *self.last_pointer_event.borrow_mut() = if all_up { None } else { Some(event) };
    }

    /// Starts the block in the node's coroutine scope; it runs until it first waits (Compose's UNDISPATCHED
    /// launch).
    fn launch(self: &Rc<Self>) {
        let Some(scope) = self.handle().and_then(|h| h.coroutine_scope()) else { return };
        *self.dispatcher.borrow_mut() = Some(scope.dispatcher());
        let handler = self.handler.borrow().clone();
        let block = handler(PointerInputScope(self.clone()));
        // (set before it runs: what it does at once may ask for it)
        // (Compose: `start = CoroutineStart.UNDISPATCHED` required so handler doesn't miss first event)
        let job = named_args!(scope.launch(start = CoroutineStart::UNDISPATCHED, coroutine = block));
        *self.job.borrow_mut() = Some(job);
    }

    /// Hands the event to the blocks awaiting this pass, each resumed before the next is offered it (Main: the
    /// newest first, as Compose's `forEachReversed`).
    fn dispatch(&self, event: &PointerEvent, pass: PointerEventPass) {
        let mut handlers = self.handlers.borrow().clone();
        if pass == PointerEventPass::Main {
            handlers.reverse();
        }
        let dispatcher = self.dispatcher.borrow().clone();
        for h in handlers {
            match &dispatcher {
                Some(d) => d.run_woken_by(|| h.offer(event, pass)),
                None => h.offer(event, pass),
            }
        }
    }

    /// Compose's `onCancelPointerInput`: the pointers still pressed let go, consumed, in the three passes.
    fn on_cancel(&self) {
        let Some(last) = self.last_pointer_event.borrow().clone() else { return };
        if last.changes.iter().all(|c| !c.pressed) {
            return;
        }
        let changes = last
            .changes
            .iter()
            .map(|old| {
                let mut c = PointerInputChange::new(
                    old.id,
                    old.uptime_millis,
                    old.position,
                    false,
                    old.uptime_millis,
                    old.position,
                    old.pressed,
                    old.pressed,
                )
                .with_type(old.r#type);
                c.pressure = old.pressure;
                c
            })
            .collect();
        let cancel = PointerEvent::new(changes);
        *self.current_event.borrow_mut() = cancel.clone();
        self.dispatch(&cancel, PointerEventPass::Initial);
        self.dispatch(&cancel, PointerEventPass::Main);
        self.dispatch(&cancel, PointerEventPass::Final);
        *self.last_pointer_event.borrow_mut() = None;
    }

    /// Cancels the block; the next event starts it again (Compose's `resetPointerInputHandler`).
    fn reset(&self) {
        let job = self.job.borrow_mut().take();
        if let Some(job) = job {
            job.cancel();
        }
    }
}

/// What a pointer input block gets (Compose's PointerInputScope): its node's size, density and view
/// configuration, and [`await_pointer_event_scope`](PointerInputScope::await_pointer_event_scope) to await events.
/// A cheap handle: clone it into what the block launches.
#[derive(Clone)]
pub struct PointerInputScope(Rc<Shared>);

impl PointerInputScope {
    /// The node's size, px.
    pub fn size(&self) -> IntSize {
        self.0.bounds_size.get()
    }

    /// How far a finger's touch reaches past each side of a node smaller than the minimum touch target: half of
    /// what it lacks (Compose's `extendedTouchPadding`).
    pub fn extended_touch_padding(&self) -> Size {
        self.0.extended_touch_padding()
    }

    /// The gesture thresholds where the node was composed (LocalViewConfiguration), read when asked.
    pub fn view_configuration(&self) -> ViewConfiguration {
        self.0.view_configuration()
    }

    /// The node's density (PointerInputScope is a Density in Compose).
    pub fn density(&self) -> Density {
        self.0.density()
    }

    /// Whether the node sees its children's events outside its bounds (Compose's
    /// `interceptOutOfBoundsChildEvents`).
    pub fn intercept_out_of_bounds_child_events(&self) -> bool {
        self.0.intercept_out_of_bounds_child_events.get()
    }

    pub fn set_intercept_out_of_bounds_child_events(&self, intercept: bool) {
        self.0.intercept_out_of_bounds_child_events.set(intercept);
    }

    /// Runs `block` awaiting events (Compose's `awaitPointerEventScope`): what it gives comes back. Events that
    /// come while no block awaits one are missed.
    pub async fn await_pointer_event_scope<R, F, Fut>(&self, block: F) -> R
    where
        F: FnOnce(AwaitPointerEventScope) -> Fut,
        Fut: Future<Output = R>,
    {
        let handler = Rc::new(HandlerCoroutine::default());
        self.0.handlers.borrow_mut().push(handler.clone());
        /// Takes the block's handler out however it ends (done, or cancelled where it waits).
        struct Remove(Rc<Shared>, Rc<HandlerCoroutine>);
        impl Drop for Remove {
            fn drop(&mut self) {
                self.0.handlers.borrow_mut().retain(|h| !Rc::ptr_eq(h, &self.1));
            }
        }
        let _remove = Remove(self.0.clone(), handler.clone());
        block(AwaitPointerEventScope { shared: self.0.clone(), handler }).await
    }
}

impl fmt::Debug for PointerInputScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PointerInputScope(size: {:?})", self.size())
    }
}

/// The time given to [`AwaitPointerEventScope::with_timeout`] ran out (Compose's
/// PointerEventTimeoutCancellationException).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PointerEventTimeoutCancellation {
    pub time_millis: i64,
}

impl fmt::Display for PointerEventTimeoutCancellation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Timed out waiting for {} ms", self.time_millis)
    }
}
impl std::error::Error for PointerEventTimeoutCancellation {}

/// Where a pointer input block awaits events (Compose's AwaitPointerEventScope). A cheap handle.
#[derive(Clone)]
pub struct AwaitPointerEventScope {
    shared: Rc<Shared>,
    handler: Rc<HandlerCoroutine>,
}

impl AwaitPointerEventScope {
    /// The node's size, px.
    pub fn size(&self) -> IntSize {
        self.shared.bounds_size.get()
    }

    /// See [`PointerInputScope::extended_touch_padding`].
    pub fn extended_touch_padding(&self) -> Size {
        self.shared.extended_touch_padding()
    }

    /// The event being dispatched (its Initial pass's), or the last one (Compose's `currentEvent`).
    pub fn current_event(&self) -> PointerEvent {
        self.shared.current_event.borrow().clone()
    }

    pub fn view_configuration(&self) -> ViewConfiguration {
        self.shared.view_configuration()
    }

    pub fn density(&self) -> Density {
        self.shared.density()
    }

    /// `.await`s the next event in the Main pass (Compose's `awaitPointerEvent()`).
    pub fn await_pointer_event(&self) -> AwaitPointerEvent {
        self.await_pointer_event_with(PointerEventPass::Main)
    }

    /// `.await`s the next event in `pass` (Compose's `awaitPointerEvent(pass)`).
    pub fn await_pointer_event_with(&self, pass: PointerEventPass) -> AwaitPointerEvent {
        AwaitPointerEvent { handler: self.handler.clone(), pass, awaiter: None }
    }

    /// Runs `block`, given up after `time_millis` of the frame clock (Compose's `withTimeout`): `Err` then, the
    /// block dropped where it waits.
    pub async fn with_timeout<T, F, Fut>(
        &self,
        time_millis: i64,
        block: F,
    ) -> Result<T, PointerEventTimeoutCancellation>
    where
        F: FnOnce(AwaitPointerEventScope) -> Fut,
        Fut: Future<Output = T>,
    {
        let mut block = std::pin::pin!(block(self.clone()));
        // Compose's two delays: all but the last few milliseconds, then those.
        let mut first: Option<Delay> = Some(delay_ms((time_millis - WITH_TIMEOUT_MICRO_DELAY_MILLIS).max(0) as u64));
        let mut second: Option<Delay> = None;
        std::future::poll_fn(move |cx| {
            if let Some(d) = first.as_mut()
                && Pin::new(d).poll(cx).is_ready()
            {
                first = None;
                second = Some(delay_ms(WITH_TIMEOUT_MICRO_DELAY_MILLIS as u64));
            }
            if let Some(d) = second.as_mut()
                && Pin::new(d).poll(cx).is_ready()
            {
                return Poll::Ready(Err(PointerEventTimeoutCancellation { time_millis }));
            }
            block.as_mut().poll(cx).map(Ok)
        })
        .await
    }

    /// The same, `None` when the time runs out (Compose's `withTimeoutOrNull`).
    pub async fn with_timeout_or_null<T, F, Fut>(&self, time_millis: i64, block: F) -> Option<T>
    where
        F: FnOnce(AwaitPointerEventScope) -> Fut,
        Fut: Future<Output = T>,
    {
        self.with_timeout(time_millis, block).await.ok()
    }
}

impl fmt::Debug for AwaitPointerEventScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AwaitPointerEventScope(size: {:?})", self.size())
    }
}

/// The next event in a pass: `.await` it ([`AwaitPointerEventScope::await_pointer_event`]).
pub struct AwaitPointerEvent {
    handler: Rc<HandlerCoroutine>,
    pass: PointerEventPass,
    awaiter: Option<Rc<Awaiter>>,
}

impl Future for AwaitPointerEvent {
    type Output = PointerEvent;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<PointerEvent> {
        match &self.awaiter {
            None => {
                let a = Rc::new(Awaiter {
                    pass: self.pass,
                    event: RefCell::new(None),
                    waker: RefCell::new(Some(cx.waker().clone())),
                });
                *self.handler.awaiter.borrow_mut() = Some(a.clone());
                self.awaiter = Some(a);
                Poll::Pending
            }
            Some(a) => {
                let event = a.event.borrow_mut().take();
                match event {
                    Some(e) => {
                        self.awaiter = None;
                        Poll::Ready(e)
                    }
                    None => {
                        *a.waker.borrow_mut() = Some(cx.waker().clone());
                        Poll::Pending
                    }
                }
            }
        }
    }
}

impl Drop for AwaitPointerEvent {
    /// Cancelled where it waits: no longer the one awaiting.
    fn drop(&mut self) {
        if let Some(a) = &self.awaiter {
            let mut current = self.handler.awaiter.borrow_mut();
            if current.as_ref().is_some_and(|c| Rc::ptr_eq(c, a)) {
                *current = None;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The node and the modifier

/// Keys a pointer input block restarts on, of any type.
trait AnyKeys {
    fn eq_keys(&self, other: &dyn Any) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<K: PartialEq + 'static> AnyKeys for K {
    fn eq_keys(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<K>().is_some_and(|o| self == o)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn same_keys(a: &Option<Rc<dyn AnyKeys>>, b: &Option<Rc<dyn AnyKeys>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.eq_keys(b.as_any()),
        _ => false,
    }
}

/// The node of `Modifier.pointer_input` (Compose's SuspendingPointerInputModifierNode): its block runs as a
/// coroutine from its first event; delegate to one (`cx.delegate(SuspendingPointerInputModifierNode::new(…))`) for
/// a node of one's own that takes gestures.
pub struct SuspendingPointerInputModifierNode {
    shared: Rc<Shared>,
    keys: Option<Rc<dyn AnyKeys>>,
    handler_type: TypeId,
}

impl SuspendingPointerInputModifierNode {
    /// A node running `block` (Compose's `SuspendingPointerInputModifierNode { … }`).
    pub fn new<F, Fut>(block: F) -> SuspendingPointerInputModifierNode
    where
        F: Fn(PointerInputScope) -> Fut + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        SuspendingPointerInputModifierNode {
            shared: Shared::new(handler_of(block)),
            keys: None,
            handler_type: TypeId::of::<F>(),
        }
    }

    /// Its block replaced: the running one cancelled, the next event starts this one (Compose's
    /// `pointerInputEventHandler` setter).
    pub fn set_pointer_input_event_handler<F, Fut>(&mut self, block: F)
    where
        F: Fn(PointerInputScope) -> Fut + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        self.reset_pointer_input_handler();
        *self.shared.handler.borrow_mut() = handler_of(block);
        self.handler_type = TypeId::of::<F>();
    }

    /// Cancels the running block; the next event starts it again (Compose's `resetPointerInputHandler`).
    pub fn reset_pointer_input_handler(&self) {
        self.shared.reset();
    }

    /// Its scope (what its block gets).
    pub fn scope(&self) -> PointerInputScope {
        PointerInputScope(self.shared.clone())
    }

    /// Compose's `update`: keys that changed (or a block of another type) restart it; the block is the new one.
    fn update(&mut self, keys: Option<Rc<dyn AnyKeys>>, handler: Handler, handler_type: TypeId) {
        let needs_reset = !same_keys(&self.keys, &keys) || self.handler_type != handler_type;
        self.keys = keys;
        self.handler_type = handler_type;
        if needs_reset {
            self.reset_pointer_input_handler();
        }
        *self.shared.handler.borrow_mut() = handler;
    }
}

impl ModifierNode for SuspendingPointerInputModifierNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        *self.shared.handle.borrow_mut() = Some(cx.handle());
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        // Its coroutine scope is cancelled as it is marked detached (the block with it, no node borrowed then);
        // it starts anew if attached again.
        self.shared.job.borrow_mut().take();
    }

    node_capabilities!(pointer_input);
}

impl PointerInputModifierNode for SuspendingPointerInputModifierNode {
    fn on_pointer_event(&mut self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        let (shared, event) = (self.shared.clone(), event.clone());
        when_node_released(move || shared.on_pointer_event(event, pass, bounds));
    }

    fn on_cancel_pointer_input(&mut self) {
        let shared = self.shared.clone();
        when_node_released(move || shared.on_cancel());
    }

    fn intercept_out_of_bounds_child_events(&self) -> bool {
        self.shared.intercept_out_of_bounds_child_events.get()
    }

    fn on_density_change(&mut self) {
        self.reset_pointer_input_handler();
    }

    fn on_view_configuration_change(&mut self) {
        self.reset_pointer_input_handler();
    }
}

struct SuspendPointerInputElement {
    keys: Rc<dyn AnyKeys>,
    handler: Handler,
    handler_type: TypeId,
}

impl PartialEq for SuspendPointerInputElement {
    fn eq(&self, other: &Self) -> bool {
        self.keys.eq_keys(other.keys.as_any())
            && self.handler_type == other.handler_type
            && Rc::ptr_eq(&self.handler, &other.handler)
    }
}

impl ModifierNodeElement for SuspendPointerInputElement {
    type Node = SuspendingPointerInputModifierNode;
    fn create(&self) -> SuspendingPointerInputModifierNode {
        SuspendingPointerInputModifierNode {
            shared: Shared::new(self.handler.clone()),
            keys: Some(self.keys.clone()),
            handler_type: self.handler_type,
        }
    }
    fn update(&self, node: &mut SuspendingPointerInputModifierNode) {
        node.update(Some(self.keys.clone()), self.handler.clone(), self.handler_type);
    }
    fn name(&self) -> &'static str {
        "pointerInput"
    }
}

impl Modifier {
    /// Runs `block` as a coroutine taking the layout's pointer events (Compose's `Modifier.pointerInput`): it
    /// starts with the first event, keeps running while `keys` (several: a tuple) stay equal — a new block with the
    /// same keys replaces the old one only for the next start — and is cancelled when they change or the layout
    /// goes. Positions are local to the layout.
    ///
    /// ```no_run
    /// # use kompoz_ui::Modifier;
    /// # use kompoz_ui::input::*;
    /// Modifier.pointer_input((), |scope: PointerInputScope| async move {
    ///     scope
    ///         .await_pointer_event_scope(|s| async move {
    ///             loop {
    ///                 let event = s.await_pointer_event().await;
    ///                 event.changes.iter().for_each(|c| c.consume());
    ///             }
    ///         })
    ///         .await
    /// });
    /// ```
    pub fn pointer_input<K, F, Fut>(self, keys: K, block: F) -> Modifier
    where
        K: PartialEq + 'static,
        F: Fn(PointerInputScope) -> Fut + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        self.then_element(SuspendPointerInputElement {
            keys: Rc::new(keys),
            handler: handler_of(block),
            handler_type: TypeId::of::<F>(),
        })
    }
}
