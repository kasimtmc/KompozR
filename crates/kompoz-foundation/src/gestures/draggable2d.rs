//! Dragging both ways, as Compose's Draggable2D.kt: a [`Draggable2DState`] takes 2D drag deltas inside a mutation
//! ([`Draggable2DState::drag_block`]); `Modifier.draggable2d` gives it the user's drags, any way they go (a drag
//! gesture node with no orientation: [`DragGestureCore`] as `draggable`'s).

// (Compose's parameter lists: the methods of a `#[named]` trait carry no attributes of their own)
#![allow(clippy::too_many_arguments)]

use super::draggable::{DragDelta, DragGestureCore, DragGestureOwner, DragStopped, DraggableStateEq, ForEachDelta};
use super::scrollable_state::LocalFuture;
use crate::interaction::MutableInteractionSource;
use kompoz_animation::{MutatePriority, MutatorMutex};
use kompoz_graphics::{IntSize, Offset};
use kompoz_runtime::{kompozable, named, named_args, remember, remember_updated_state};
use kompoz_ui::input::{
    IndirectPointerEvent, IndirectPointerInputModifierNode, PointerEvent, PointerEventPass, PointerInputModifierNode,
    PointerType,
};
use kompoz_ui::unit::Velocity;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, node_capabilities};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// The state

/// What a 2D drag mutation drags with (Compose's Drag2DScope).
pub trait Drag2DScope {
    /// Drags by `pixels` (Compose's `dragBy`).
    fn drag_by(&self, pixels: Offset);
}

/// A block dragging in a [`Drag2DScope`] (Compose's `suspend Drag2DScope.() -> Unit`).
pub type Drag2DBlock = Box<dyn FnOnce(Rc<dyn Drag2DScope>) -> LocalFuture<'static, ()>>;

/// A state that is dragged both ways (Compose's Draggable2DState): `drag_block` runs a block taking deltas in its
/// [`Drag2DScope`], one block at a time (a newer one of at least its priority cancels an older);
/// `dispatch_raw_delta` drags outside of that. A cheap handle, equal to its clones (what `draggable2d` compares it
/// by).
pub trait Draggable2DState: Any + DraggableStateEq {
    /// Runs `block` as the mutation of `drag_priority` (Compose's `drag`). Through
    /// [`Draggable2DStateExtensions::drag`] with a closure.
    fn drag_block(&self, drag_priority: MutatePriority, block: Drag2DBlock) -> LocalFuture<'_, ()>;

    /// Drags by `delta` at once, outside of any mutation (Compose's `dispatchRawDelta`).
    fn dispatch_raw_delta(&self, delta: Offset);
}

impl PartialEq for dyn Draggable2DState {
    fn eq(&self, other: &dyn Draggable2DState) -> bool {
        self.draggable_state_eq(DraggableStateEq::as_any(other))
    }
}

impl fmt::Debug for dyn Draggable2DState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Draggable2DState")
    }
}

/// A state shared as any kind of 2D draggable state: its state's calls.
impl Draggable2DState for Rc<dyn Draggable2DState> {
    fn drag_block(&self, drag_priority: MutatePriority, block: Drag2DBlock) -> LocalFuture<'_, ()> {
        (**self).drag_block(drag_priority, block)
    }
    fn dispatch_raw_delta(&self, delta: Offset) {
        (**self).dispatch_raw_delta(delta)
    }
}

/// The calls of any [`Draggable2DState`] (Compose's `drag` with a lambda).
#[named(trait Draggable2DStateExtensions)]
impl<S: Draggable2DState> S {
    /// Runs `block` as the mutation of `drag_priority` (Compose's `drag { … }`).
    fn drag<F, Fut>(
        &self,
        #[default(MutatePriority::Default)] drag_priority: MutatePriority,
        block: F,
    ) -> impl Future<Output = ()>
    where
        F: FnOnce(Rc<dyn Drag2DScope>) -> Fut + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        self.drag_block(drag_priority, Box::new(move |scope| Box::pin(block(scope))))
    }
}

/// A 2D draggable state of a closure taking the deltas (Compose's DefaultDraggable2DState, made by
/// [`Draggable2DState()`]).
#[derive(Clone)]
pub struct DefaultDraggable2DState {
    inner: Rc<DefaultDraggable2DInner>,
}

struct DefaultDraggable2DInner {
    on_delta: Rc<dyn Fn(Offset)>,
    drag_2d_mutex: MutatorMutex,
}

impl PartialEq for DefaultDraggable2DState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for DefaultDraggable2DState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DefaultDraggable2DState({:p})", Rc::as_ptr(&self.inner))
    }
}

struct DefaultDrag2DScope(Rc<dyn Fn(Offset)>);

impl Drag2DScope for DefaultDrag2DScope {
    fn drag_by(&self, pixels: Offset) {
        (self.0)(pixels)
    }
}

impl Draggable2DState for DefaultDraggable2DState {
    fn drag_block(&self, drag_priority: MutatePriority, block: Drag2DBlock) -> LocalFuture<'_, ()> {
        let scope: Rc<dyn Drag2DScope> = Rc::new(DefaultDrag2DScope(self.inner.on_delta.clone()));
        Box::pin(named_args!(self.inner.drag_2d_mutex.mutate_with(scope, drag_priority, block)))
    }

    fn dispatch_raw_delta(&self, delta: Offset) {
        (self.inner.on_delta)(delta)
    }
}

/// A 2D draggable state of `on_delta`, given each drag delta px (Compose's `Draggable2DState(onDelta)`).
pub fn Draggable2DState(on_delta: impl Fn(Offset) + 'static) -> DefaultDraggable2DState {
    DefaultDraggable2DState {
        inner: Rc::new(DefaultDraggable2DInner { on_delta: Rc::new(on_delta), drag_2d_mutex: MutatorMutex::new() }),
    }
}

/// A [`Draggable2DState()`] remembered where it is called, calling the latest `on_delta` (Compose's
/// `rememberDraggable2DState`).
#[kompozable]
pub fn remember_draggable2d_state(on_delta: impl Fn(Offset) + 'static) -> DefaultDraggable2DState {
    let on_delta_state = remember_updated_state!(Rc::new(on_delta) as Rc<dyn Fn(Offset)>);
    remember(move || Draggable2DState(move |delta| (on_delta_state.get())(delta)))
}

// ---------------------------------------------------------------------------------------------------------------
// Modifier.draggable2d

/// The 2D draggable modifier.
#[named(trait Draggable2DModifiers)]
impl Modifier {
    /// Gives `state` the user's drags, any way they go (Compose's `Modifier.draggable2D`): after the touch slop (at
    /// once with `start_drag_immediately`), each delta px, the finger's (the other way with `reverse_direction`);
    /// `on_drag_started` is told where it started, `on_drag_stopped` the velocity it was let go with (as it was,
    /// not reversed: Compose's); the drags are emitted to `interaction_source` as
    /// [`DragInteraction`](crate::interaction::DragInteraction)s. A mouse drags it too. Not `enabled`: nothing.
    fn draggable2d(
        self,
        state: impl Draggable2DState,
        #[default(true)] enabled: bool,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        #[default(false)] start_drag_immediately: bool,
        #[default(None)] on_drag_started: Option<Rc<dyn Fn(Offset)>>,
        #[default(None)] on_drag_stopped: Option<Rc<dyn Fn(Velocity)>>,
        #[default(false)] reverse_direction: bool,
    ) -> Modifier {
        self.then_element(Draggable2DElement {
            state: Rc::new(state),
            enabled,
            interaction_source,
            start_drag_immediately,
            on_drag_started,
            on_drag_stopped,
            reverse_direction,
        })
    }
}

struct Draggable2DElement {
    state: Rc<dyn Draggable2DState>,
    enabled: bool,
    interaction_source: Option<MutableInteractionSource>,
    start_drag_immediately: bool,
    on_drag_started: Option<Rc<dyn Fn(Offset)>>,
    on_drag_stopped: Option<Rc<dyn Fn(Velocity)>>,
    reverse_direction: bool,
}

fn same_rc<T: ?Sized>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

impl PartialEq for Draggable2DElement {
    fn eq(&self, o: &Self) -> bool {
        *self.state == *o.state
            && self.enabled == o.enabled
            && self.interaction_source == o.interaction_source
            && self.start_drag_immediately == o.start_drag_immediately
            && same_rc(&self.on_drag_started, &o.on_drag_started)
            && same_rc(&self.on_drag_stopped, &o.on_drag_stopped)
            && self.reverse_direction == o.reverse_direction
    }
}

/// `draggable2d`'s: any pointer drags it (Compose's Draggable2DElement.CanDrag).
fn can_drag_any(_: PointerType) -> bool {
    true
}

impl ModifierNodeElement for Draggable2DElement {
    type Node = Draggable2DNode;
    fn create(&self) -> Draggable2DNode {
        Draggable2DNode::new(self)
    }
    fn update(&self, node: &mut Draggable2DNode) {
        node.core.update(self);
    }
    fn name(&self) -> &'static str {
        "draggable2D"
    }
}

/// `draggable2d`'s node (Compose's Draggable2DNode): a drag gesture node of no orientation opening its state's
/// mutation for the drags.
pub struct Draggable2DNode {
    core: Rc<Draggable2DCore>,
}

struct Draggable2DCore {
    drag: Rc<DragGestureCore>,
    state: RefCell<Rc<dyn Draggable2DState>>,
    start_drag_immediately: Cell<bool>,
    reverse_direction: Cell<bool>,
    on_drag_started: RefCell<Option<Rc<dyn Fn(Offset)>>>,
    on_drag_stopped: RefCell<Option<Rc<dyn Fn(Velocity)>>>,
}

impl Draggable2DNode {
    fn new(e: &Draggable2DElement) -> Draggable2DNode {
        let drag = DragGestureCore::new(Rc::new(can_drag_any), e.enabled, e.interaction_source.clone(), None);
        let core = Rc::new(Draggable2DCore {
            drag: drag.clone(),
            state: RefCell::new(e.state.clone()),
            start_drag_immediately: Cell::new(e.start_drag_immediately),
            reverse_direction: Cell::new(e.reverse_direction),
            on_drag_started: RefCell::new(e.on_drag_started.clone()),
            on_drag_stopped: RefCell::new(e.on_drag_stopped.clone()),
        });
        let owner: Rc<dyn DragGestureOwner> = core.clone();
        drag.set_owner(&owner);
        Draggable2DNode { core }
    }
}

impl Draggable2DCore {
    /// Compose's Draggable2DNode.update.
    fn update(&self, e: &Draggable2DElement) {
        let mut reset_pointer_input_handling = false;
        if **self.state.borrow() != *e.state {
            *self.state.borrow_mut() = e.state.clone();
            reset_pointer_input_handling = true;
        }
        if self.reverse_direction.get() != e.reverse_direction {
            self.reverse_direction.set(e.reverse_direction);
            reset_pointer_input_handling = true;
        }
        *self.on_drag_started.borrow_mut() = e.on_drag_started.clone();
        *self.on_drag_stopped.borrow_mut() = e.on_drag_stopped.clone();
        self.start_drag_immediately.set(e.start_drag_immediately);
        self.drag.update(
            Rc::new(can_drag_any),
            e.enabled,
            e.interaction_source.clone(),
            None,
            reset_pointer_input_handling,
        );
    }
}

impl DragGestureOwner for Draggable2DCore {
    fn drag(self: Rc<Self>, for_each_delta: ForEachDelta) -> LocalFuture<'static, ()> {
        Box::pin(async move {
            let state = self.state.borrow().clone();
            let me = self.clone();
            state
                .drag_block(
                    MutatePriority::UserInput,
                    Box::new(move |scope| {
                        Box::pin(async move {
                            let process: Rc<dyn Fn(DragDelta)> = Rc::new(move |d: DragDelta| {
                                let delta = if me.reverse_direction.get() { d.delta * -1.0 } else { d.delta };
                                scope.drag_by(delta);
                            });
                            for_each_delta(process).await;
                        })
                    }),
                )
                .await;
        })
    }

    fn on_drag_started(&self, started_position: Offset) {
        let on_drag_started = self.on_drag_started.borrow().clone();
        if let Some(f) = on_drag_started {
            f(started_position);
        }
    }

    fn on_drag_stopped(&self, event: DragStopped) {
        let on_drag_stopped = self.on_drag_stopped.borrow().clone();
        if let Some(f) = on_drag_stopped {
            f(event.velocity);
        }
    }

    fn start_drag_immediately(&self) -> bool {
        self.start_drag_immediately.get()
    }
}

impl ModifierNode for Draggable2DNode {
    node_capabilities!(pointer_input, indirect_pointer_input);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.core.drag.on_attach(cx);
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.core.drag.on_detach();
    }
}

impl PointerInputModifierNode for Draggable2DNode {
    fn on_pointer_event(&mut self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        self.core.drag.on_pointer_event(event, pass, bounds);
    }
    fn on_cancel_pointer_input(&mut self) {
        self.core.drag.on_cancel_pointer_input();
    }
}

impl IndirectPointerInputModifierNode for Draggable2DNode {
    fn on_indirect_pointer_event(&mut self, event: &IndirectPointerEvent, pass: PointerEventPass) {
        self.core.drag.on_indirect_pointer_event(event, pass);
    }
    fn on_cancel_indirect_pointer_input(&mut self) {
        self.core.drag.on_cancel_indirect_pointer_input();
    }
}
