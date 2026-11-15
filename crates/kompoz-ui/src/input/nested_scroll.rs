//! Nested scrolling, as Compose's `nestedscroll` package: a scrollable offers what it is about to scroll (and
//! fling) to the nested scroll connections around it before using it, outermost first, and what it left after,
//! innermost first. `Modifier.nested_scroll(connection, dispatcher)` is a link of the chain; its dispatcher is how a
//! scrollable under it (or a custom gesture) talks to the links above.

use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, TraversableNode};
use crate::node_capabilities;
use crate::unit::Velocity;
use kompoz_coroutines::{CoroutineScope, Dispatcher};
use kompoz_graphics::Offset;
use kompoz_runtime::named;
use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

static NESTED_SCROLL_KEY: &str = "androidx.compose.ui.input.nestedscroll.NestedScrollNode";

/// What a fling callback gives back: the velocity it used, once it is done.
pub type FlingFuture<'a> = Pin<Box<dyn Future<Output = Velocity> + 'a>>;

/// Where a scroll comes from (Compose's NestedScrollSource).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NestedScrollSource {
    /// The user: a drag, a wheel, a key.
    UserInput,
    /// Something else: a fling, an animation.
    SideEffect,
}

impl NestedScrollSource {
    /// Compose's deprecated name of [`UserInput`](NestedScrollSource::UserInput).
    pub const Drag: NestedScrollSource = NestedScrollSource::UserInput;
    /// Compose's deprecated name of [`SideEffect`](NestedScrollSource::SideEffect).
    pub const Fling: NestedScrollSource = NestedScrollSource::SideEffect;
    /// Compose's deprecated name of [`UserInput`](NestedScrollSource::UserInput).
    pub const Wheel: NestedScrollSource = NestedScrollSource::UserInput;
}

/// A link of the nested scroll chain (Compose's NestedScrollConnection): it may take part of a scroll before what
/// is under it does (`on_pre_scroll`), and of what that left (`on_post_scroll`); the same for flings, which take
/// time (`on_pre_fling`, `on_post_fling` give futures). Each gives back what it used. Nothing, by default.
pub trait NestedScrollConnection {
    fn on_pre_scroll(&self, available: Offset, source: NestedScrollSource) -> Offset {
        let _ = (available, source);
        Offset::Zero
    }
    fn on_post_scroll(&self, consumed: Offset, available: Offset, source: NestedScrollSource) -> Offset {
        let _ = (consumed, available, source);
        Offset::Zero
    }
    /// A fling is about to start with `available`: what this uses of it (a suspending call in Compose).
    fn on_pre_fling(&self, available: Velocity) -> FlingFuture<'_> {
        let _ = available;
        Box::pin(async { Velocity::Zero })
    }
    /// A fling ended having used `consumed`, `available` left.
    fn on_post_fling(&self, consumed: Velocity, available: Velocity) -> FlingFuture<'_> {
        let _ = (consumed, available);
        Box::pin(async { Velocity::Zero })
    }
}

/// What a dispatcher knows of the node it is attached to.
#[derive(Default)]
struct DispatcherState {
    node: RefCell<Option<NodeHandle>>,
    last_known_parent: RefCell<Option<NodeCx>>,
    scope: RefCell<Option<CoroutineScope>>,
    /// Detached from its node after having been attached: its coroutine scope is a cancelled one.
    cancelled: RefCell<Option<Dispatcher>>,
}

/// Sends scrolls and flings to the nested scroll chain above the node it is given to (Compose's
/// NestedScrollDispatcher): `Modifier.nested_scroll(connection, Some(dispatcher))`. Clones are the same
/// dispatcher; remember it.
#[derive(Clone, Default)]
pub struct NestedScrollDispatcher(Rc<DispatcherState>);

impl PartialEq for NestedScrollDispatcher {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for NestedScrollDispatcher {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NestedScrollDispatcher({:p})", Rc::as_ptr(&self.0))
    }
}

impl NestedScrollDispatcher {
    pub fn new() -> NestedScrollDispatcher {
        NestedScrollDispatcher::default()
    }

    /// The nearest link above its node, while it is attached (Compose's `parent`).
    fn parent(&self) -> Option<NodeCx> {
        let cx = self.0.node.borrow().as_ref().and_then(|h| h.cx())?;
        parent_of(&cx)
    }

    /// Where to launch the flings its scrolls start (Compose's `coroutineScope`): the outermost active scope of
    /// the chain above its node, or its node's. Panics before it is given to a `nested_scroll`.
    pub fn coroutine_scope(&self) -> CoroutineScope {
        let node = self.0.node.borrow().as_ref().and_then(|h| h.cx());
        if let Some(cx) = node {
            return nested_coroutine_scope(&cx);
        }
        if let Some(d) = self.0.cancelled.borrow().as_ref() {
            let scope = CoroutineScope::new(d);
            scope.cancel();
            return scope;
        }
        match self.0.scope.borrow().as_ref() {
            Some(s) => s.clone(),
            None => panic!(
                "in order to access nested coroutine scope you need to attach dispatcher to the `Modifier.nestedScroll` \
                 first."
            ),
        }
    }

    /// Offers the chain above `available` before scrolling: what it used (Compose's `dispatchPreScroll`).
    pub fn dispatch_pre_scroll(&self, available: Offset, source: NestedScrollSource) -> Offset {
        self.parent().map_or(Offset::Zero, |p| pre_scroll(&p, available, source))
    }

    /// Tells the chain above what the scroll used and left: what it used of that (Compose's `dispatchPostScroll`).
    pub fn dispatch_post_scroll(&self, consumed: Offset, available: Offset, source: NestedScrollSource) -> Offset {
        self.parent().map_or(Offset::Zero, |p| post_scroll(&p, consumed, available, source))
    }

    /// Offers the chain above a fling's velocity before flinging (Compose's `dispatchPreFling`).
    pub fn dispatch_pre_fling(&self, available: Velocity) -> FlingFuture<'static> {
        match self.parent() {
            Some(p) => pre_fling(p, available),
            None => Box::pin(async { Velocity::Zero }),
        }
    }

    /// Tells the chain above what the fling used and left (Compose's `dispatchPostFling`): the parent it last
    /// had, if its node was detached meanwhile.
    pub fn dispatch_post_fling(&self, consumed: Velocity, available: Velocity) -> FlingFuture<'static> {
        let parent = self.parent().or_else(|| self.0.last_known_parent.borrow().clone());
        match parent {
            Some(p) => post_fling(p, consumed, available),
            None => Box::pin(async { Velocity::Zero }),
        }
    }
}

/// The node of `Modifier.nested_scroll` (Compose's NestedScrollNode): a link of the chain, found by the links
/// under it as their nearest traversable ancestor of its key.
pub struct NestedScrollNode {
    connection: Rc<dyn NestedScrollConnection>,
    resolved_dispatcher: NestedScrollDispatcher,
    last_known_parent: Option<NodeCx>,
    handle: Option<NodeHandle>,
}

/// A nested scroll node to delegate to (Compose's `nestedScrollModifierNode`).
pub fn nested_scroll_modifier_node(
    connection: Rc<dyn NestedScrollConnection>,
    dispatcher: Option<NestedScrollDispatcher>,
) -> NestedScrollNode {
    NestedScrollNode {
        connection,
        resolved_dispatcher: dispatcher.unwrap_or_default(),
        last_known_parent: None,
        handle: None,
    }
}

fn connection_of(cx: &NodeCx) -> Option<Rc<dyn NestedScrollConnection>> {
    cx.with::<NestedScrollNode, _>(|n| n.connection.clone())
}

/// The nearest link above, while attached (Compose's `parentNestedScrollNode`).
fn parent_of(cx: &NodeCx) -> Option<NodeCx> {
    if cx.is_attached() { cx.find_nearest_ancestor(&NESTED_SCROLL_KEY) } else { None }
}

/// Compose's NestedScrollNode `onPreScroll`: the parent first, then its own connection with what is left.
fn pre_scroll(cx: &NodeCx, available: Offset, source: NestedScrollSource) -> Offset {
    let parent_consumed = parent_of(cx).map_or(Offset::Zero, |p| pre_scroll(&p, available, source));
    let self_consumed =
        connection_of(cx).map_or(Offset::Zero, |c| c.on_pre_scroll(available - parent_consumed, source));
    parent_consumed + self_consumed
}

/// Compose's `onPostScroll`: its own connection first, then the parent with what is left.
fn post_scroll(cx: &NodeCx, consumed: Offset, available: Offset, source: NestedScrollSource) -> Offset {
    let self_consumed = connection_of(cx).map_or(Offset::Zero, |c| c.on_post_scroll(consumed, available, source));
    let parent_consumed = parent_of(cx)
        .map_or(Offset::Zero, |p| post_scroll(&p, consumed + self_consumed, available - self_consumed, source));
    self_consumed + parent_consumed
}

fn pre_fling(cx: NodeCx, available: Velocity) -> FlingFuture<'static> {
    Box::pin(async move {
        let parent_consumed = match parent_of(&cx) {
            Some(p) => pre_fling(p, available).await,
            None => Velocity::Zero,
        };
        let self_consumed = match connection_of(&cx) {
            Some(c) => c.on_pre_fling(available - parent_consumed).await,
            None => Velocity::Zero,
        };
        parent_consumed + self_consumed
    })
}

fn post_fling(cx: NodeCx, consumed: Velocity, available: Velocity) -> FlingFuture<'static> {
    Box::pin(async move {
        let self_consumed = match connection_of(&cx) {
            Some(c) => c.on_post_fling(consumed, available).await,
            None => Velocity::Zero,
        };
        let parent = if cx.is_attached() {
            parent_of(&cx)
        } else {
            cx.with::<NestedScrollNode, _>(|n| n.last_known_parent.clone()).flatten()
        };
        let parent_consumed = match parent {
            Some(p) => post_fling(p, consumed + self_consumed, available - self_consumed).await,
            None => Velocity::Zero,
        };
        self_consumed + parent_consumed
    })
}

/// The chain's outermost active scope at `cx`, or its own dispatcher's (Compose's `nestedCoroutineScope`).
fn nested_coroutine_scope(cx: &NodeCx) -> CoroutineScope {
    if let Some(p) = parent_of(cx) {
        let parent_scope = nested_coroutine_scope(&p);
        if parent_scope.is_active() {
            return parent_scope;
        }
    }
    let own = cx.with::<NestedScrollNode, _>(|n| n.resolved_dispatcher.0.scope.borrow().clone()).flatten();
    own.unwrap_or_else(|| {
        panic!("in order to access nested coroutine scope you need to attach dispatcher to the `Modifier.nestedScroll` first.")
    })
}

/// The nearest attached link above (Compose's `findNearestAttachedAncestor`).
fn find_nearest_attached_ancestor(cx: &NodeCx) -> Option<NodeCx> {
    let mut found = None;
    cx.traverse_ancestors(&NESTED_SCROLL_KEY, &mut |a| {
        if a.is_attached() {
            found = Some(a.clone());
            false
        } else {
            true
        }
    });
    found
}

impl NestedScrollNode {
    /// The connection it links in.
    pub fn connection(&self) -> Rc<dyn NestedScrollConnection> {
        self.connection.clone()
    }

    /// Its dispatcher (the one given, or its own).
    pub fn dispatcher(&self) -> NestedScrollDispatcher {
        self.resolved_dispatcher.clone()
    }

    fn update_dispatcher_fields(&mut self, cx: &NodeCx) {
        let d = &self.resolved_dispatcher.0;
        *d.node.borrow_mut() = Some(cx.handle());
        *d.last_known_parent.borrow_mut() = None;
        *d.cancelled.borrow_mut() = None;
        self.last_known_parent = None;
        *d.scope.borrow_mut() = Some(cx.coroutine_scope());
    }

    fn reset_dispatcher_fields(&self) {
        let d = &self.resolved_dispatcher.0;
        let mine = {
            let node = d.node.borrow();
            match (node.as_ref(), self.handle.as_ref()) {
                (Some(a), Some(b)) => std::rc::Weak::ptr_eq(&a.cell, &b.cell),
                _ => false,
            }
        };
        if mine {
            *d.node.borrow_mut() = None;
            let scope = d.scope.borrow_mut().take();
            *d.cancelled.borrow_mut() = scope.map(|s| s.dispatcher());
        }
    }

    /// Compose's `updateNode`: the new connection, and dispatcher.
    pub fn update_node(
        &mut self,
        connection: Rc<dyn NestedScrollConnection>,
        dispatcher: Option<NestedScrollDispatcher>,
    ) {
        self.connection = connection;
        self.reset_dispatcher_fields();
        match dispatcher {
            None => self.resolved_dispatcher = NestedScrollDispatcher::new(),
            Some(d) if d != self.resolved_dispatcher => self.resolved_dispatcher = d,
            Some(_) => {}
        }
        let cx = self.handle.as_ref().and_then(|h| h.cx()).filter(|c| c.is_attached());
        if let Some(cx) = cx {
            self.update_dispatcher_fields(&cx);
        }
    }
}

impl ModifierNode for NestedScrollNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        self.update_dispatcher_fields(cx);
    }

    fn on_detach(&mut self, cx: &NodeCx) {
        self.last_known_parent = find_nearest_attached_ancestor(cx);
        *self.resolved_dispatcher.0.last_known_parent.borrow_mut() = self.last_known_parent.clone();
        self.reset_dispatcher_fields();
    }

    node_capabilities!(traversable);
}

impl TraversableNode for NestedScrollNode {
    fn traverse_key(&self) -> &dyn Any {
        &NESTED_SCROLL_KEY
    }
}

struct NestedScrollElement {
    connection: Rc<dyn NestedScrollConnection>,
    dispatcher: Option<NestedScrollDispatcher>,
}

impl PartialEq for NestedScrollElement {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(Rc::as_ptr(&self.connection), Rc::as_ptr(&other.connection))
            && self.dispatcher == other.dispatcher
    }
}

impl ModifierNodeElement for NestedScrollElement {
    type Node = NestedScrollNode;
    fn create(&self) -> NestedScrollNode {
        nested_scroll_modifier_node(self.connection.clone(), self.dispatcher.clone())
    }
    fn update(&self, node: &mut NestedScrollNode) {
        node.update_node(self.connection.clone(), self.dispatcher.clone());
    }
    fn name(&self) -> &'static str {
        "nestedScroll"
    }
}

#[named]
impl Modifier {
    /// Links the layout into the nested scroll chain (Compose's `Modifier.nestedScroll`): `connection` takes part
    /// in the scrolls and flings of what is under it; `dispatcher` sends those of a custom gesture here to the
    /// links above.
    pub fn nested_scroll(
        self,
        connection: Rc<dyn NestedScrollConnection>,
        #[default(None)] dispatcher: Option<NestedScrollDispatcher>,
    ) -> Modifier {
        self.then_element(NestedScrollElement { connection, dispatcher })
    }
}
