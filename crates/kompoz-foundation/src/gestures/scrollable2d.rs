//! Scrolling both ways, as Compose's Scrollable2D.kt (with AbstractScrollableNode.kt's parts it uses):
//! `Modifier.scrollable2d` gives a [`Scrollable2DState`] the user's drags any way they go and their flings, through
//! the nested scroll chain and the overscroll effect ([`ScrollingLogic2D`], the 2D [`ScrollLogic`]: drag → pre
//! scroll → the state → post scroll → the overscroll), and accessibility's scrolls. As Compose's, it takes no mouse
//! wheel, trackpad pan, key or bring-into-view request (those go on to what is around it).

use super::draggable::{DragDelta, DragGestureCore, DragGestureOwner, DragStopped, ForEachDelta};
use super::fling::{DefaultFlingBehavior, FlingBehavior, ScrollableDefaultFlingBehavior, same_fling_behavior};
use super::scrollable::{
    NestedScrollScope, ScrollLogic, ScrollLogicDyn, ScrollableNestedScrollConnection, can_drag_calculation,
};
use super::scrollable_state::{LocalFuture, ScrollScope};
use super::scrollable2d_state::{Scroll2DScope, Scrollable2DState};
use crate::interaction::MutableInteractionSource;
use crate::overscroll::{OverscrollEffect, PerformFling, same_effect};
use kompoz_animation::{MutatePriority, VectorConverter, animate_value, spline_based_decay};
use kompoz_coroutines::current_job;
use kompoz_graphics::{Density, IntSize, Offset};
use kompoz_runtime::{named, named_args};
use kompoz_ui::input::{
    IndirectPointerEvent, IndirectPointerInputModifierNode, NestedScrollConnection, NestedScrollDispatcher,
    NestedScrollSource, PointerEvent, PointerEventPass, PointerInputModifierNode, nested_scroll_modifier_node,
};
use kompoz_ui::semantics::{SemanticsConfiguration, SemanticsModifierNode};
use kompoz_ui::unit::Velocity;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------
// Modifier.scrollable2d

/// The 2D scrollable modifier.
#[named(trait Scrollable2DModifiers)]
impl Modifier {
    /// Scrolls `state` with the user's drags, any way they go, and their flings (`fling_behavior`, else
    /// [`ScrollableDefaults::fling_behavior`](super::ScrollableDefaults::fling_behavior)'s spline), through the
    /// nested scroll chain; what the chain and the state leave goes to `overscroll_effect` (none: no overscroll;
    /// drawn where `Modifier.overscroll` puts it). Accessibility scrolls it too. A mouse doesn't drag it, and (as
    /// Compose's) the mouse wheel and trackpad pans go past it. Drags are emitted to `interaction_source`. Not
    /// `enabled`: no user scrolling (Compose's `Modifier.scrollable2D`).
    fn scrollable2d(
        self,
        state: impl Scrollable2DState,
        #[default(true)] enabled: bool,
        #[default(None)] overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
        #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
    ) -> Modifier {
        self.then_element(Scrollable2DElement {
            state: Rc::new(state),
            overscroll_effect,
            enabled,
            fling_behavior,
            interaction_source,
        })
    }
}

struct Scrollable2DElement {
    state: Rc<dyn Scrollable2DState>,
    overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
    enabled: bool,
    fling_behavior: Option<Rc<dyn FlingBehavior>>,
    interaction_source: Option<MutableInteractionSource>,
}

impl PartialEq for Scrollable2DElement {
    fn eq(&self, o: &Self) -> bool {
        *self.state == *o.state
            && same_effect(&self.overscroll_effect, &o.overscroll_effect)
            && self.enabled == o.enabled
            && same_fling_behavior(&self.fling_behavior, &o.fling_behavior)
            && self.interaction_source == o.interaction_source
    }
}

impl ModifierNodeElement for Scrollable2DElement {
    type Node = Scrollable2DNode;
    fn create(&self) -> Scrollable2DNode {
        Scrollable2DNode::new(self)
    }
    fn update(&self, node: &mut Scrollable2DNode) {
        node.core.update(self);
    }
    fn name(&self) -> &'static str {
        "scrollable2D"
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The scrolling logic

/// The 2D scroll scope of no state (Compose's NoOpScrollScope): it takes everything.
struct NoOpScroll2DScope;

impl Scroll2DScope for NoOpScroll2DScope {
    fn scroll_by(&self, delta: Offset) -> Offset {
        delta
    }
}

/// A block scrolling in a [`NestedScrollScope`].
type NestedScrollBlock = Box<dyn FnOnce(Rc<dyn NestedScrollScope>) -> LocalFuture<'static, ()>>;

/// Everything of a 2D scrollable's scrolling (Compose's ScrollingLogic2D): the nested scroll chain, the state, the
/// overscroll effect, the fling.
pub(crate) struct ScrollingLogic2D {
    me: Weak<ScrollingLogic2D>,
    scrollable_state: RefCell<Rc<dyn Scrollable2DState>>,
    overscroll_effect: RefCell<Option<Rc<dyn OverscrollEffect>>>,
    fling_behavior: RefCell<Rc<dyn FlingBehavior>>,
    nested_scroll_dispatcher: RefCell<NestedScrollDispatcher>,
    is_scrollable_node_attached: Rc<dyn Fn() -> bool>,
    is_flinging: Cell<bool>,
    latest_scroll_source: Cell<NestedScrollSource>,
    /// The state's scope of the mutation under way (Compose's `outerStateScope`).
    outer_state_scope: RefCell<Rc<dyn Scroll2DScope>>,
}

/// The nested scroll scope of a 2D scrolling logic.
struct Logic2DNestedScrollScope(Weak<ScrollingLogic2D>);

impl NestedScrollScope for Logic2DNestedScrollScope {
    fn scroll_by(&self, offset: Offset, source: NestedScrollSource) -> Offset {
        let Some(logic) = self.0.upgrade() else { return Offset::Zero };
        let outer = logic.outer_state_scope.borrow().clone();
        logic.perform_scroll(&*outer, offset, source)
    }

    fn scroll_by_with_overscroll(&self, offset: Offset, source: NestedScrollSource) -> Offset {
        let Some(logic) = self.0.upgrade() else { return Offset::Zero };
        logic.latest_scroll_source.set(source);
        let overscroll = logic.overscroll_effect.borrow().clone();
        match overscroll {
            Some(overscroll) if logic.should_dispatch_overscroll(offset) => {
                let l = logic.clone();
                overscroll.apply_to_scroll(offset, logic.latest_scroll_source.get(), &move |delta| {
                    let outer = l.outer_state_scope.borrow().clone();
                    l.perform_scroll(&*outer, delta, l.latest_scroll_source.get())
                })
            }
            _ => {
                let outer = logic.outer_state_scope.borrow().clone();
                logic.perform_scroll(&*outer, offset, source)
            }
        }
    }
}

impl ScrollingLogic2D {
    pub(crate) fn new(
        scrollable_state: Rc<dyn Scrollable2DState>,
        overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
        fling_behavior: Rc<dyn FlingBehavior>,
        nested_scroll_dispatcher: NestedScrollDispatcher,
        is_scrollable_node_attached: Rc<dyn Fn() -> bool>,
    ) -> Rc<ScrollingLogic2D> {
        Rc::new_cyclic(|me| ScrollingLogic2D {
            me: me.clone(),
            scrollable_state: RefCell::new(scrollable_state),
            overscroll_effect: RefCell::new(overscroll_effect),
            fling_behavior: RefCell::new(fling_behavior),
            nested_scroll_dispatcher: RefCell::new(nested_scroll_dispatcher),
            is_scrollable_node_attached,
            is_flinging: Cell::new(false),
            latest_scroll_source: Cell::new(NestedScrollSource::UserInput),
            outer_state_scope: RefCell::new(Rc::new(NoOpScroll2DScope)),
        })
    }

    fn rc(&self) -> Rc<ScrollingLogic2D> {
        self.me.upgrade().expect("kompoz: a 2D scrollable's logic outlived it")
    }

    fn state(&self) -> Rc<dyn Scrollable2DState> {
        self.scrollable_state.borrow().clone()
    }

    /// The nested scroll chain, the state, the chain again (Compose's `Scroll2DScope.performScroll`).
    fn perform_scroll(&self, scope: &dyn Scroll2DScope, delta: Offset, source: NestedScrollSource) -> Offset {
        let dispatcher = self.nested_scroll_dispatcher.borrow().clone();
        let consumed_by_pre_scroll = dispatcher.dispatch_pre_scroll(delta, source);
        let scroll_available_after_pre_scroll = delta - consumed_by_pre_scroll;
        let consumed_by_self_scroll = scope.scroll_by(scroll_available_after_pre_scroll);
        let delta_available_after_scroll = scroll_available_after_pre_scroll - consumed_by_self_scroll;
        let consumed_by_post_scroll =
            dispatcher.dispatch_post_scroll(consumed_by_self_scroll, delta_available_after_scroll, source);
        consumed_by_pre_scroll + consumed_by_self_scroll + consumed_by_post_scroll
    }

    /// Whether the overscroll effect gets a delta: the state can scroll that way (Compose's
    /// `shouldDispatchOverscroll`).
    fn should_dispatch_overscroll(&self, offset: Offset) -> bool {
        self.state().can_scroll(offset)
    }

    fn dispatch_raw_delta(&self, scroll: Offset) -> Offset {
        self.state().dispatch_raw_delta(scroll)
    }

    /// A drag's end: its fling through the chain and the overscroll (Compose's `onScrollStopped`).
    pub(crate) async fn on_scroll_stopped(self: Rc<Self>, initial_velocity: Velocity) {
        let available_velocity = initial_velocity;
        let logic = self.clone();
        let perform_fling: PerformFling = Rc::new(move |velocity: Velocity| {
            let logic = logic.clone();
            Box::pin(async move {
                let dispatcher = logic.nested_scroll_dispatcher.borrow().clone();
                let pre_consumed_by_parent = dispatcher.dispatch_pre_fling(velocity).await;
                let available = velocity - pre_consumed_by_parent;
                let velocity_left = logic.clone().do_fling_animation(available).await;
                let consumed_post = dispatcher.dispatch_post_fling(available - velocity_left, velocity_left).await;
                let total_left = velocity_left - consumed_post;
                velocity - total_left
            })
        });
        let overscroll = self.overscroll_effect.borrow().clone();
        match overscroll {
            Some(overscroll)
                if self.should_dispatch_overscroll(Offset::new(initial_velocity.x, initial_velocity.y)) =>
            {
                overscroll.apply_to_fling(available_velocity, perform_fling).await;
            }
            _ => {
                perform_fling(available_velocity).await;
            }
        }
    }

    /// Opens a scrolling session with nested scrolling and overscroll (Compose's `scroll`): the state's mutation,
    /// `block` given the nested scroll scope.
    fn scroll(&self, scroll_priority: MutatePriority, block: NestedScrollBlock) -> LocalFuture<'static, ()> {
        let logic = self.rc();
        let state = self.state();
        Box::pin(async move {
            let l = logic.clone();
            state
                .scroll_block(
                    scroll_priority,
                    Box::new(move |scope| {
                        *l.outer_state_scope.borrow_mut() = scope;
                        block(Rc::new(Logic2DNestedScrollScope(Rc::downgrade(&l))))
                    }),
                )
                .await;
        })
    }

    /// Compose's `update`: whether its pointer input should be reset.
    fn update(
        &self,
        scrollable_state: Rc<dyn Scrollable2DState>,
        overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
        fling_behavior: Rc<dyn FlingBehavior>,
        nested_scroll_dispatcher: NestedScrollDispatcher,
    ) -> bool {
        let mut reset_pointer_input_handling = false;
        if **self.scrollable_state.borrow() != *scrollable_state {
            *self.scrollable_state.borrow_mut() = scrollable_state;
            reset_pointer_input_handling = true;
        }
        *self.overscroll_effect.borrow_mut() = overscroll_effect;
        *self.fling_behavior.borrow_mut() = fling_behavior;
        *self.nested_scroll_dispatcher.borrow_mut() = nested_scroll_dispatcher;
        reset_pointer_input_handling
    }

    /// Accessibility's scroll by `offset`: animated along a spring, its own priority none higher than the default
    /// (Compose's `semanticsScrollBy`): how far it went.
    async fn semantics_scroll_by(self: Rc<Self>, offset: Offset) -> Offset {
        let previous_value = Rc::new(Cell::new(Offset::Zero));
        let p = previous_value.clone();
        self.scroll(
            MutatePriority::Default,
            Box::new(move |scope| {
                Box::pin(async move {
                    let block = move |current_value: Offset, _: Offset| {
                        let delta = current_value - p.get();
                        let consumed = scope.scroll_by(delta, NestedScrollSource::UserInput);
                        p.set(p.get() + consumed);
                    };
                    named_args!(animate_value(Offset::VectorConverter, Offset::Zero, offset, block = block)).await;
                })
            }),
        )
        .await;
        previous_value.get()
    }
}

/// Clears `is_flinging` however the fling ends (Compose's `finally`).
struct FlingingFlag(Rc<ScrollingLogic2D>);

impl Drop for FlingingFlag {
    fn drop(&mut self) {
        self.0.is_flinging.set(false);
    }
}

/// Kotlin's `Float.sign`: 0 for 0.
fn sign(v: f32) -> f32 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        v
    }
}

/// The direction of a fling's velocity, radians (Compose's `Velocity.angleRad`).
fn angle_rad(v: Velocity) -> f32 {
    v.y.atan2(v.x)
}

/// How fast a fling goes, px/s (Compose's `Velocity.magnitude`).
fn magnitude(v: Velocity) -> f32 {
    (v.x * v.x + v.y * v.y).sqrt()
}

/// The scope a fling behavior scrolls a 2D scrollable in: each step's pixels along the fling's direction, through
/// the overscroll, how far that went told as a length (Compose's `flingScope`).
struct Fling2DScope {
    logic: Rc<ScrollingLogic2D>,
    nested: Rc<dyn NestedScrollScope>,
    available: Velocity,
}

impl Fling2DScope {
    /// Compose's `Float.toDecomposedOffset`.
    fn to_decomposed_offset(&self, pixels: f32) -> Offset {
        let angle = angle_rad(self.available);
        if angle.is_nan() {
            Offset::new(0.0, pixels)
        } else {
            Offset::new(
                (angle.cos() * pixels).abs() * sign(self.available.x),
                (angle.sin() * pixels).abs() * sign(self.available.y),
            )
        }
    }
}

impl ScrollScope for Fling2DScope {
    fn scroll_by(&self, pixels: f32) -> f32 {
        let pixels_offset = self.to_decomposed_offset(pixels);
        // The node left: the fling is cancelled (Compose's FlingCancellationException; Kotlin throws, this cancels
        // its coroutine, which ends at its next wait).
        if pixels_offset != Offset::Zero && !(self.logic.is_scrollable_node_attached)() {
            current_job().cancel();
            return 0.0;
        }
        let consumed_offset = self.nested.scroll_by_with_overscroll(pixels_offset, NestedScrollSource::SideEffect);
        consumed_offset.get_distance()
    }
}

/// Compose's `Float.toDecomposedVelocity`.
fn to_decomposed_velocity(available: Velocity, v: f32) -> Velocity {
    let angle = angle_rad(available);
    if angle.is_nan() {
        Velocity::new(0.0, v)
    } else {
        Velocity::new((angle.cos() * v).abs() * sign(available.x), (angle.sin() * v).abs() * sign(available.y))
    }
}

impl ScrollLogic for ScrollingLogic2D {
    fn is_flinging(&self) -> bool {
        self.is_flinging.get()
    }

    /// Compose's `performRawScroll`: nothing while the state scrolls itself.
    fn perform_raw_scroll(&self, scroll: Offset) -> Offset {
        if self.state().is_scroll_in_progress() { Offset::Zero } else { self.dispatch_raw_delta(scroll) }
    }

    /// Compose's `doFlingAnimation`: the fling behavior run along the velocity's magnitude in a mutation of its
    /// own, each step decomposed along its direction; the velocity left, decomposed the same way.
    fn do_fling_animation(self: Rc<Self>, available: Velocity) -> LocalFuture<'static, Velocity> {
        Box::pin(async move {
            let result = Rc::new(Cell::new(available));
            self.is_flinging.set(true);
            let _flag = FlingingFlag(self.clone());
            let (logic, r) = (self.clone(), result.clone());
            self.scroll(
                MutatePriority::Default,
                Box::new(move |nested| {
                    Box::pin(async move {
                        let fling_scope: Rc<dyn ScrollScope> =
                            Rc::new(Fling2DScope { logic: logic.clone(), nested, available });
                        let fling_behavior = logic.fling_behavior.borrow().clone();
                        let result_velocity = fling_behavior.perform_fling(fling_scope, magnitude(available)).await;
                        r.set(to_decomposed_velocity(available, result_velocity));
                    })
                }),
            )
            .await;
            result.get()
        })
    }

    /// Compose's `shouldScrollImmediately`: a touch catches a scroll or an overscroll under way.
    fn should_scroll_immediately(&self) -> bool {
        self.state().is_scroll_in_progress()
            || self.overscroll_effect.borrow().as_ref().is_some_and(|o| o.is_in_progress())
    }
}

impl ScrollLogicDyn for ScrollingLogic2D {
    fn is_flinging(&self) -> bool {
        ScrollLogic::is_flinging(self)
    }
    fn perform_raw_scroll(&self, scroll: Offset) -> Offset {
        ScrollLogic::perform_raw_scroll(self, scroll)
    }
    fn do_fling_animation_dyn(&self, available: Velocity) -> LocalFuture<'static, Velocity> {
        self.rc().do_fling_animation(available)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The node

/// `scrollable2d`'s node (Compose's Scrollable2DNode with its AbstractScrollableNode and DragGestureNode): it
/// delegates to its nested scroll link.
pub struct Scrollable2DNode {
    core: Rc<Scrollable2DCore>,
}

struct Scrollable2DCore {
    me: Weak<Scrollable2DCore>,
    drag: Rc<DragGestureCore>,
    overscroll_effect: RefCell<Option<Rc<dyn OverscrollEffect>>>,
    fling_behavior: RefCell<Option<Rc<dyn FlingBehavior>>>,
    default_fling_behavior: Rc<DefaultFlingBehavior>,
    scroll_logic: Rc<ScrollingLogic2D>,
    nested_scroll_dispatcher: NestedScrollDispatcher,
    nested_scroll_connection: Rc<ScrollableNestedScrollConnection>,
    delegated: Cell<bool>,
    scroll_by_action: RefCell<Option<Rc<dyn Fn(f32, f32) -> bool>>>,
    scroll_by_offset_action: RefCell<Option<Rc<dyn Fn(Offset) -> LocalFuture<'static, Offset>>>>,
    handle: RefCell<Option<NodeHandle>>,
}

impl Scrollable2DNode {
    fn new(e: &Scrollable2DElement) -> Scrollable2DNode {
        let core = Rc::new_cyclic(|me: &Weak<Scrollable2DCore>| {
            let drag =
                DragGestureCore::new(Rc::new(can_drag_calculation), e.enabled, e.interaction_source.clone(), None);
            // (a placeholder fling, its density told once it is attached)
            let default_fling_behavior =
                Rc::new(DefaultFlingBehavior::new(Arc::new(spline_based_decay::<f32>(Density::new(1.0, 1.0))), None));
            let nested_scroll_dispatcher = NestedScrollDispatcher::new();
            let fling: Rc<dyn FlingBehavior> = match &e.fling_behavior {
                Some(f) => f.clone(),
                None => default_fling_behavior.clone(),
            };
            let attached = me.clone();
            let scroll_logic = ScrollingLogic2D::new(
                e.state.clone(),
                e.overscroll_effect.clone(),
                fling,
                nested_scroll_dispatcher.clone(),
                Rc::new(move || attached.upgrade().is_some_and(|c| c.is_attached())),
            );
            let logic: Rc<dyn ScrollLogicDyn> = scroll_logic.clone();
            let nested_scroll_connection = Rc::new(ScrollableNestedScrollConnection::new(logic, e.enabled));
            Scrollable2DCore {
                me: me.clone(),
                drag,
                overscroll_effect: RefCell::new(e.overscroll_effect.clone()),
                fling_behavior: RefCell::new(e.fling_behavior.clone()),
                default_fling_behavior,
                scroll_logic,
                nested_scroll_dispatcher,
                nested_scroll_connection,
                delegated: Cell::new(false),
                scroll_by_action: RefCell::new(None),
                scroll_by_offset_action: RefCell::new(None),
                handle: RefCell::new(None),
            }
        });
        let owner: Rc<dyn DragGestureOwner> = core.clone();
        core.drag.set_owner(&owner);
        Scrollable2DNode { core }
    }
}

impl Scrollable2DCore {
    fn cx(&self) -> Option<NodeCx> {
        self.handle.borrow().as_ref().and_then(|h| h.cx())
    }

    fn is_attached(&self) -> bool {
        self.cx().is_some_and(|c| c.is_attached())
    }

    fn enabled(&self) -> bool {
        self.drag.enabled.get()
    }

    /// Its nested scroll link, delegated the first time it is attached (Compose's
    /// `initializeNestedScrollingDelegation`, as it makes the node).
    fn delegate_once(&self, cx: &NodeCx) {
        if self.delegated.replace(true) {
            return;
        }
        let connection: Rc<dyn NestedScrollConnection> = self.nested_scroll_connection.clone();
        cx.delegate(nested_scroll_modifier_node(connection, Some(self.nested_scroll_dispatcher.clone())));
    }

    /// Compose's AbstractScrollableNode `onAttach`.
    fn on_attach(&self, cx: &NodeCx) {
        *self.handle.borrow_mut() = Some(cx.handle());
        self.drag.on_attach(cx);
        self.delegate_once(cx);
        self.update_default_fling_behavior();
    }

    fn update_default_fling_behavior(&self) {
        let Some(cx) = self.cx().filter(|c| c.is_attached()) else { return };
        self.default_fling_behavior.update_density(cx.density());
    }

    /// Compose's AbstractScrollableNode `onDensityChange`.
    fn on_density_change(&self) {
        self.drag.on_cancel_pointer_input();
        self.drag.on_cancel_pointer_input();
        self.update_default_fling_behavior();
    }

    /// Compose's Scrollable2DNode `update` (with AbstractScrollableNode's and DragGestureNode's).
    fn update(&self, e: &Scrollable2DElement) {
        // AbstractScrollableNode.update
        if self.enabled() != e.enabled {
            self.nested_scroll_connection.enabled.set(e.enabled);
            self.clear_scroll_semantics_actions();
            if let Some(cx) = self.cx() {
                cx.invalidate_semantics();
            }
        }
        *self.overscroll_effect.borrow_mut() = e.overscroll_effect.clone();
        *self.fling_behavior.borrow_mut() = e.fling_behavior.clone();
        // A new fling behavior: the one it uses.
        let resolved_fling_behavior: Rc<dyn FlingBehavior> = match &e.fling_behavior {
            Some(f) => f.clone(),
            None => self.default_fling_behavior.clone(),
        };
        let reset_pointer_input_handling = self.scroll_logic.update(
            e.state.clone(),
            e.overscroll_effect.clone(),
            resolved_fling_behavior,
            self.nested_scroll_dispatcher.clone(),
        );
        self.drag.update(
            Rc::new(can_drag_calculation),
            e.enabled,
            e.interaction_source.clone(),
            None,
            reset_pointer_input_handling,
        );
    }

    /// Compose's AbstractScrollableNode `onPointerEvent`: a drag of a pointer that drags it (no mouse wheel or
    /// trackpad logic: a 2D scrollable makes none).
    fn on_pointer_event(&self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        if event.changes.iter().any(|c| self.drag.can_drag(c.r#type)) {
            self.drag.on_pointer_event(event, pass, bounds);
        }
        if self.enabled() {
            self.drag.initialize_pointer_input_gesture_coordination();
        }
    }

    /// Compose's `setScrollSemanticsActions`.
    fn set_scroll_semantics_actions(&self) {
        let me = self.me.clone();
        *self.scroll_by_action.borrow_mut() = Some(Rc::new(move |x, y| {
            if let Some(core) = me.upgrade()
                && let Some(scope) = core.cx().filter(|c| c.is_attached()).map(|c| c.coroutine_scope())
            {
                let logic = core.scroll_logic.clone();
                scope.launch(async move {
                    logic.semantics_scroll_by(Offset::new(x, y)).await;
                });
            }
            true
        }));
        let logic = self.scroll_logic.clone();
        *self.scroll_by_offset_action.borrow_mut() = Some(Rc::new(move |offset| {
            let logic = logic.clone();
            Box::pin(async move { logic.semantics_scroll_by(offset).await })
        }));
    }

    /// Compose's `clearScrollSemanticsActions`.
    fn clear_scroll_semantics_actions(&self) {
        *self.scroll_by_action.borrow_mut() = None;
        *self.scroll_by_offset_action.borrow_mut() = None;
    }
}

impl DragGestureOwner for Scrollable2DCore {
    /// Compose's Scrollable2DNode `drag`: the deltas through the overscroll, in a user's mutation.
    fn drag(self: Rc<Self>, for_each_delta: ForEachDelta) -> LocalFuture<'static, ()> {
        self.scroll_logic.scroll(
            MutatePriority::UserInput,
            Box::new(move |scope| {
                Box::pin(async move {
                    let process: Rc<dyn Fn(DragDelta)> = Rc::new(move |d: DragDelta| {
                        scope.scroll_by_with_overscroll(d.delta, NestedScrollSource::UserInput);
                    });
                    for_each_delta(process).await;
                })
            }),
        )
    }

    fn on_drag_started(&self, _started_position: Offset) {}

    /// Compose's Scrollable2DNode `onDragStopped`: the fling, in the nested scroll chain's scope.
    fn on_drag_stopped(&self, event: DragStopped) {
        if !self.is_attached() {
            return;
        }
        let logic = self.scroll_logic.clone();
        self.nested_scroll_dispatcher.coroutine_scope().launch(async move {
            logic.on_scroll_stopped(event.velocity).await;
        });
    }

    fn start_drag_immediately(&self) -> bool {
        self.scroll_logic.should_scroll_immediately()
    }
}

impl ModifierNode for Scrollable2DNode {
    node_capabilities!(pointer_input, indirect_pointer_input, semantics);

    fn should_auto_invalidate(&self) -> bool {
        false
    }

    fn on_attach(&mut self, cx: &NodeCx) {
        self.core.on_attach(cx);
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.core.drag.on_detach();
    }
}

impl PointerInputModifierNode for Scrollable2DNode {
    fn on_pointer_event(&mut self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        self.core.on_pointer_event(event, pass, bounds);
    }
    fn on_cancel_pointer_input(&mut self) {
        self.core.drag.on_cancel_pointer_input();
    }
    fn on_density_change(&mut self) {
        self.core.on_density_change();
    }
}

impl IndirectPointerInputModifierNode for Scrollable2DNode {
    fn on_indirect_pointer_event(&mut self, event: &IndirectPointerEvent, pass: PointerEventPass) {
        self.core.drag.on_indirect_pointer_event(event, pass);
    }
    fn on_cancel_indirect_pointer_input(&mut self) {
        self.core.drag.on_cancel_indirect_pointer_input();
    }
}

impl SemanticsModifierNode for Scrollable2DNode {
    /// Compose's AbstractScrollableNode `applySemantics`: accessibility's scrolls while it is enabled.
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        let core = &self.core;
        if core.enabled()
            && (core.scroll_by_action.borrow().is_none() || core.scroll_by_offset_action.borrow().is_none())
        {
            core.set_scroll_semantics_actions();
        }
        if let Some(action) = core.scroll_by_action.borrow().clone() {
            config.scroll_by(move |x, y| action(x, y));
        }
        if let Some(action) = core.scroll_by_offset_action.borrow().clone() {
            config.scroll_by_offset(move |offset| action(offset));
        }
    }
}
