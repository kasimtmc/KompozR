//! Scrolling, as Compose's Scrollable.kt and AbstractScrollableNode.kt: `Modifier.scrollable` gives a
//! [`ScrollableState`] the user's drags, flings, mouse wheel, trackpad pans, Page Up and Down, accessibility's
//! scrolls and bring-into-view requests along one axis; [`ScrollingLogic`] takes each delta through the nested
//! scroll chain and the overscroll effect (drag → pre scroll → the state → post scroll → the overscroll), a fling
//! through them with its [`FlingBehavior`].

// (Compose's parameter lists: the methods of a `#[named]` trait carry no attributes of their own)
#![allow(clippy::too_many_arguments)]

use super::Orientation;
use super::bring_into_view_spec::{BringIntoViewSpec, same_spec};
use super::content_in_view::{ContentInViewCore, ContentInViewNode};
use super::draggable::{DragGestureCore, DragGestureOwner, DragStopped, ForEachDelta};
use super::fling::{
    DefaultFlingBehavior, FlingBehavior, ScrollableDefaultFlingBehavior, platform_scrollable_default_fling_behavior,
    remember_platform_default_fling_behavior, same_fling_behavior, should_be_triggered_by_mouse_wheel,
};
use super::mouse_wheel::{MouseWheelScrollingLogic, NonTouchScrollingLogic, TrackpadScrollingLogic};
use super::scrollable_state::{LocalFuture, ScrollScope, ScrollableState};
use crate::interaction::MutableInteractionSource;
use crate::overscroll::{OverscrollEffect, PerformFling, same_effect};
use crate::relocation::{BringIntoViewResponder, BringIntoViewResponderNode};
use kompoz_animation::{MutatePriority, animate};
use kompoz_coroutines::current_job;
use kompoz_graphics::{IntSize, LayoutDirection, Offset};
use kompoz_runtime::{named, named_args};
use kompoz_ui::focus::{FocusTargetNode, Focusability};
use kompoz_ui::input::{
    IndirectPointerEvent, IndirectPointerInputModifierNode, Key, KeyEvent, KeyEventType, KeyInputModifierNode,
    NestedScrollConnection, NestedScrollDispatcher, NestedScrollSource, PointerEvent, PointerEventPass,
    PointerEventType, PointerInputModifierNode, PointerType, nested_scroll_modifier_node,
};
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::semantics::{SemanticsConfiguration, SemanticsModifierNode};
use kompoz_ui::unit::Velocity;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// A drag's angle past which it is vertical (Compose's VerticalAxisThresholdAngle, π/4).
const VerticalAxisThresholdAngle: f32 = std::f32::consts::FRAC_PI_4;

// ---------------------------------------------------------------------------------------------------------------
// Modifier.scrollable

/// The scrollable modifier.
#[named(trait ScrollableModifiers)]
impl Modifier {
    /// Scrolls `state` with the user's drags along `orientation` and their flings (`fling_behavior`, else
    /// [`ScrollableDefaults::fling_behavior`]'s), the mouse wheel and a trackpad's pans, Page Up and Page Down when
    /// it has the focus, accessibility's scrolls, and the bring-into-view requests of what is in it (by
    /// `bring_into_view_spec`, else [`LocalBringIntoViewSpec`](super::LocalBringIntoViewSpec)'s) (Compose's
    /// `Modifier.scrollable`). Each delta is in the finger's direction (positive: down or right), the other way
    /// with `reverse_direction`; through the nested scroll chain, what the chain and the state leave goes to
    /// `overscroll_effect` (none: no overscroll; drawn where `Modifier.overscroll` puts it). A mouse doesn't drag
    /// it. Drags are emitted to `interaction_source`. Not `enabled`: no user scrolling (the state still scrolls
    /// from code and bring-into-view).
    fn scrollable(
        self,
        state: impl ScrollableState,
        orientation: Orientation,
        #[default(None)] overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
        #[default(true)] enabled: bool,
        #[default(false)] reverse_direction: bool,
        #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        #[default(None)] bring_into_view_spec: Option<Rc<dyn BringIntoViewSpec>>,
    ) -> Modifier {
        self.then_element(ScrollableElement {
            state: Rc::new(state),
            orientation,
            overscroll_effect,
            enabled,
            reverse_direction,
            fling_behavior,
            interaction_source,
            bring_into_view_spec,
        })
    }
}

pub(crate) struct ScrollableElement {
    pub(crate) state: Rc<dyn ScrollableState>,
    pub(crate) orientation: Orientation,
    pub(crate) overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
    pub(crate) enabled: bool,
    pub(crate) reverse_direction: bool,
    pub(crate) fling_behavior: Option<Rc<dyn FlingBehavior>>,
    pub(crate) interaction_source: Option<MutableInteractionSource>,
    pub(crate) bring_into_view_spec: Option<Rc<dyn BringIntoViewSpec>>,
}

impl PartialEq for ScrollableElement {
    fn eq(&self, o: &Self) -> bool {
        *self.state == *o.state
            && self.orientation == o.orientation
            && same_effect(&self.overscroll_effect, &o.overscroll_effect)
            && self.enabled == o.enabled
            && self.reverse_direction == o.reverse_direction
            && same_fling_behavior(&self.fling_behavior, &o.fling_behavior)
            && self.interaction_source == o.interaction_source
            && same_spec(&self.bring_into_view_spec, &o.bring_into_view_spec)
    }
}

impl ModifierNodeElement for ScrollableElement {
    type Node = ScrollableNode;
    fn create(&self) -> ScrollableNode {
        ScrollableNode::new(self)
    }
    fn update(&self, node: &mut ScrollableNode) {
        node.core.update(self);
    }
    fn name(&self) -> &'static str {
        "scrollable"
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The defaults

/// The defaults of `scrollable` (Compose's ScrollableDefaults).
pub struct ScrollableDefaults;

impl ScrollableDefaults {
    /// Whether a scrollable's input is reversed so the content moves with the finger (Compose's
    /// `reverseDirection`): always once (a finger moves the content, not the viewport), the other way for
    /// `reverse_scrolling`, and once more horizontally right to left.
    pub fn reverse_direction(
        layout_direction: LayoutDirection,
        orientation: Orientation,
        reverse_scrolling: bool,
    ) -> bool {
        let mut reverse_direction = !reverse_scrolling;
        let is_rtl = layout_direction == LayoutDirection::Rtl;
        if is_rtl && orientation != Orientation::Vertical {
            reverse_direction = !reverse_direction;
        }
        reverse_direction
    }

    /// The natural fling, remembered for this screen's density (Compose's composable
    /// `ScrollableDefaults.flingBehavior()`).
    #[track_caller]
    pub fn fling_behavior() -> Rc<dyn FlingBehavior> {
        kompoz_runtime::kompozable_block!(remember_platform_default_fling_behavior())
    }

    /// Compose's deprecated composable `ScrollableDefaults.overscrollEffect()`: the platform's effect remembered
    /// here, one that does nothing where the deprecated configuration turns it off (use
    /// `remember_overscroll_effect`).
    #[track_caller]
    pub fn overscroll_effect() -> Rc<dyn OverscrollEffect> {
        kompoz_runtime::kompozable_block!(match crate::overscroll_android::remember_platform_overscroll_effect() {
            Some(effect) => effect,
            None => Rc::new(NoOpOverscrollEffect),
        })
    }
}

/// An overscroll effect doing nothing (Compose's ScrollableDefaults.NoOpOverscrollEffect).
#[derive(PartialEq)]
struct NoOpOverscrollEffect;

impl OverscrollEffect for NoOpOverscrollEffect {
    fn apply_to_scroll(
        &self,
        delta: Offset,
        _source: NestedScrollSource,
        perform_scroll: &dyn Fn(Offset) -> Offset,
    ) -> Offset {
        perform_scroll(delta)
    }
    fn apply_to_fling(&self, velocity: Velocity, perform_fling: PerformFling) -> LocalFuture<'_, ()> {
        Box::pin(async move {
            perform_fling(velocity).await;
        })
    }
    fn is_in_progress(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The scrolling logic

/// A scroll scope taking offsets through the nested scroll chain (Compose's NestedScrollScope): `scroll_by` the
/// chain and the state, `scroll_by_with_overscroll` through the overscroll effect too.
pub trait NestedScrollScope {
    fn scroll_by(&self, offset: Offset, source: NestedScrollSource) -> Offset;
    fn scroll_by_with_overscroll(&self, offset: Offset, source: NestedScrollSource) -> Offset;
}

/// A block scrolling in a [`NestedScrollScope`].
pub(crate) type NestedScrollBlock = Box<dyn FnOnce(Rc<dyn NestedScrollScope>) -> LocalFuture<'static, ()>>;

/// What a scrollable node scrolls by, shared with the 2D one (Compose's ScrollLogic).
pub(crate) trait ScrollLogic {
    fn is_flinging(&self) -> bool;
    fn perform_raw_scroll(&self, scroll: Offset) -> Offset;
    fn do_fling_animation(self: Rc<Self>, available: Velocity) -> LocalFuture<'static, Velocity>;
    fn should_scroll_immediately(&self) -> bool;
}

/// The scroll scope of no state (Compose's NoOpScrollScope): it takes everything.
struct NoOpScrollScope;

impl ScrollScope for NoOpScrollScope {
    fn scroll_by(&self, pixels: f32) -> f32 {
        pixels
    }
}

/// Everything of a scrollable's scrolling (Compose's ScrollingLogic): the nested scroll chain, the state, the
/// overscroll effect, the fling; deltas mapped onto its axis and reversed as it says.
pub(crate) struct ScrollingLogic {
    me: Weak<ScrollingLogic>,
    pub(crate) scrollable_state: RefCell<Rc<dyn ScrollableState>>,
    overscroll_effect: RefCell<Option<Rc<dyn OverscrollEffect>>>,
    fling_behavior: RefCell<Rc<dyn FlingBehavior>>,
    orientation: Cell<Orientation>,
    reverse_direction: Cell<bool>,
    nested_scroll_dispatcher: RefCell<NestedScrollDispatcher>,
    /// The node told what each scroll took (Compose's OnScrollChangedDispatcher).
    on_scroll_changed: Rc<dyn Fn(Offset)>,
    is_scrollable_node_attached: Rc<dyn Fn() -> bool>,
    is_flinging: Cell<bool>,
    latest_scroll_source: Cell<NestedScrollSource>,
    /// The state's scope of the mutation under way (Compose's `outerStateScope`).
    outer_state_scope: RefCell<Rc<dyn ScrollScope>>,
}

/// The nested scroll scope of a scrolling logic.
struct LogicNestedScrollScope(Weak<ScrollingLogic>);

impl NestedScrollScope for LogicNestedScrollScope {
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
            Some(overscroll) if logic.should_dispatch_overscroll() => {
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

impl ScrollingLogic {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        scrollable_state: Rc<dyn ScrollableState>,
        overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
        fling_behavior: Rc<dyn FlingBehavior>,
        orientation: Orientation,
        reverse_direction: bool,
        nested_scroll_dispatcher: NestedScrollDispatcher,
        on_scroll_changed: Rc<dyn Fn(Offset)>,
        is_scrollable_node_attached: Rc<dyn Fn() -> bool>,
    ) -> Rc<ScrollingLogic> {
        Rc::new_cyclic(|me| ScrollingLogic {
            me: me.clone(),
            scrollable_state: RefCell::new(scrollable_state),
            overscroll_effect: RefCell::new(overscroll_effect),
            fling_behavior: RefCell::new(fling_behavior),
            orientation: Cell::new(orientation),
            reverse_direction: Cell::new(reverse_direction),
            nested_scroll_dispatcher: RefCell::new(nested_scroll_dispatcher),
            on_scroll_changed,
            is_scrollable_node_attached,
            is_flinging: Cell::new(false),
            latest_scroll_source: Cell::new(NestedScrollSource::UserInput),
            outer_state_scope: RefCell::new(Rc::new(NoOpScrollScope)),
        })
    }

    fn rc(&self) -> Rc<ScrollingLogic> {
        self.me.upgrade().expect("kompoz: a scrollable's logic outlived it")
    }

    fn state(&self) -> Rc<dyn ScrollableState> {
        self.scrollable_state.borrow().clone()
    }

    fn horizontal(&self) -> bool {
        self.orientation.get() == Orientation::Horizontal
    }

    /// A delta along its axis as an offset (Compose's `Float.toOffset`).
    pub(crate) fn float_to_offset(&self, v: f32) -> Offset {
        if v == 0.0 {
            Offset::Zero
        } else if self.horizontal() {
            Offset::new(v, 0.0)
        } else {
            Offset::new(0.0, v)
        }
    }

    /// The offset with its other axis taken off (Compose's `singleAxisOffset`).
    pub(crate) fn single_axis_offset(&self, o: Offset) -> Offset {
        if self.horizontal() { Offset::new(o.x, 0.0) } else { Offset::new(0.0, o.y) }
    }

    /// Its axis' part (Compose's `Offset.toFloat`).
    pub(crate) fn offset_to_float(&self, o: Offset) -> f32 {
        if self.horizontal() { o.x } else { o.y }
    }

    /// The delta of a 2D one along its main axis by its angle: its own axis' if it goes mostly that way, else
    /// nothing (Compose's `toSingleAxisDeltaFromAngle`).
    pub(crate) fn to_single_axis_delta_from_angle(&self, o: Offset) -> f32 {
        let angle = o.y.abs().atan2(o.x.abs());
        if angle >= VerticalAxisThresholdAngle {
            if self.orientation.get() == Orientation::Vertical { o.y } else { 0.0 }
        } else if self.horizontal() {
            o.x
        } else {
            0.0
        }
    }

    pub(crate) fn float_to_velocity(&self, v: f32) -> Velocity {
        if v == 0.0 {
            Velocity::Zero
        } else if self.horizontal() {
            Velocity::new(v, 0.0)
        } else {
            Velocity::new(0.0, v)
        }
    }

    fn velocity_to_float(&self, v: Velocity) -> f32 {
        if self.horizontal() { v.x } else { v.y }
    }

    fn single_axis_velocity(&self, v: Velocity) -> Velocity {
        if self.horizontal() { Velocity::new(v.x, 0.0) } else { Velocity::new(0.0, v.y) }
    }

    fn update_velocity(&self, v: Velocity, new_value: f32) -> Velocity {
        if self.horizontal() { Velocity::new(new_value, v.y) } else { Velocity::new(v.x, new_value) }
    }

    pub(crate) fn reverse_float_if_needed(&self, v: f32) -> f32 {
        if self.reverse_direction.get() { -v } else { v }
    }

    pub(crate) fn reverse_offset_if_needed(&self, o: Offset) -> Offset {
        if self.reverse_direction.get() { o * -1.0 } else { o }
    }

    pub(crate) fn is_vertical(&self) -> bool {
        self.orientation.get() == Orientation::Vertical
    }

    /// The nested scroll chain, the state (on its axis, reversed as it says), the chain again (Compose's
    /// `ScrollScope.performScroll`).
    fn perform_scroll(&self, scope: &dyn ScrollScope, delta: Offset, source: NestedScrollSource) -> Offset {
        let dispatcher = self.nested_scroll_dispatcher.borrow().clone();
        let consumed_by_pre_scroll = dispatcher.dispatch_pre_scroll(delta, source);
        let scroll_available_after_pre_scroll = delta - consumed_by_pre_scroll;
        let single_axis_delta_for_self_scroll = self
            .offset_to_float(self.reverse_offset_if_needed(self.single_axis_offset(scroll_available_after_pre_scroll)));
        // Taken along its axis.
        let consumed_by_self_scroll =
            self.reverse_offset_if_needed(self.float_to_offset(scope.scroll_by(single_axis_delta_for_self_scroll)));
        (self.on_scroll_changed)(consumed_by_self_scroll);
        let delta_available_after_scroll = scroll_available_after_pre_scroll - consumed_by_self_scroll;
        let consumed_by_post_scroll =
            dispatcher.dispatch_post_scroll(consumed_by_self_scroll, delta_available_after_scroll, source);
        consumed_by_pre_scroll + consumed_by_self_scroll + consumed_by_post_scroll
    }

    /// Whether the overscroll effect gets the deltas: the state can scroll, or the effect shows (Compose's
    /// `shouldDispatchOverscroll`).
    fn should_dispatch_overscroll(&self) -> bool {
        let state = self.state();
        state.can_scroll_forward()
            || state.can_scroll_backward()
            || self.overscroll_effect.borrow().as_ref().is_some_and(|o| o.is_in_progress())
    }

    fn dispatch_raw_delta(&self, scroll: Offset) -> Offset {
        let state = self.state();
        self.float_to_offset(self.reverse_float_if_needed(
            state.dispatch_raw_delta(self.reverse_float_if_needed(self.offset_to_float(scroll))),
        ))
    }

    /// A drag's or a wheel's end: its fling through the chain and the overscroll (Compose's `onScrollStopped`); a
    /// mouse wheel's only for a fling of one's own (snapping's).
    pub(crate) async fn on_scroll_stopped(self: Rc<Self>, initial_velocity: Velocity, is_mouse_wheel: bool) {
        if is_mouse_wheel && !should_be_triggered_by_mouse_wheel(&*self.fling_behavior.borrow().clone()) {
            return;
        }
        let available_velocity = self.single_axis_velocity(initial_velocity);
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
            Some(overscroll) if self.should_dispatch_overscroll() => {
                overscroll.apply_to_fling(available_velocity, perform_fling).await;
            }
            _ => {
                perform_fling(available_velocity).await;
            }
        }
    }

    /// Opens a scrolling session with nested scrolling and overscroll (Compose's `scroll`): the state's mutation,
    /// `block` given the nested scroll scope.
    pub(crate) fn scroll(&self, scroll_priority: MutatePriority, block: NestedScrollBlock) -> LocalFuture<'static, ()> {
        let logic = self.rc();
        let state = self.state();
        Box::pin(async move {
            let l = logic.clone();
            state
                .scroll_block(
                    scroll_priority,
                    Box::new(move |scope| {
                        *l.outer_state_scope.borrow_mut() = scope;
                        block(Rc::new(LogicNestedScrollScope(Rc::downgrade(&l))))
                    }),
                )
                .await;
        })
    }

    /// Compose's `update`: whether its pointer input should be reset.
    pub(crate) fn update(
        &self,
        scrollable_state: Rc<dyn ScrollableState>,
        orientation: Orientation,
        overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
        reverse_direction: bool,
        fling_behavior: Rc<dyn FlingBehavior>,
        nested_scroll_dispatcher: NestedScrollDispatcher,
    ) -> bool {
        let mut reset_pointer_input_handling = false;
        if **self.scrollable_state.borrow() != *scrollable_state {
            *self.scrollable_state.borrow_mut() = scrollable_state;
            reset_pointer_input_handling = true;
        }
        *self.overscroll_effect.borrow_mut() = overscroll_effect;
        if self.orientation.get() != orientation {
            self.orientation.set(orientation);
            reset_pointer_input_handling = true;
        }
        if self.reverse_direction.get() != reverse_direction {
            self.reverse_direction.set(reverse_direction);
            reset_pointer_input_handling = true;
        }
        *self.fling_behavior.borrow_mut() = fling_behavior;
        *self.nested_scroll_dispatcher.borrow_mut() = nested_scroll_dispatcher;
        reset_pointer_input_handling
    }

    /// Accessibility's scroll by `offset`: animated, its own priority none higher than the default (Compose's
    /// `semanticsScrollBy`): how far it went.
    pub(crate) async fn semantics_scroll_by(self: Rc<Self>, offset: Offset) -> Offset {
        let previous_value = Rc::new(Cell::new(0.0f32));
        let target = self.offset_to_float(offset);
        let (logic, p) = (self.clone(), previous_value.clone());
        self.scroll(
            MutatePriority::Default,
            Box::new(move |scope| {
                Box::pin(async move {
                    let block = move |current_value: f32, _: f32| {
                        let delta = current_value - p.get();
                        let consumed = logic.reverse_float_if_needed(logic.offset_to_float(scope.scroll_by(
                            logic.float_to_offset(logic.reverse_float_if_needed(delta)),
                            NestedScrollSource::UserInput,
                        )));
                        p.set(p.get() + consumed);
                    };
                    named_args!(animate(0.0, target, block = block)).await;
                })
            }),
        )
        .await;
        self.float_to_offset(previous_value.get())
    }
}

/// Clears `is_flinging` however the fling ends (Compose's `finally`).
struct FlingingFlag(Rc<ScrollingLogic>);

impl Drop for FlingingFlag {
    fn drop(&mut self) {
        self.0.is_flinging.set(false);
    }
}

/// The scope a fling behavior scrolls in: the fling's offsets through the overscroll, reversed back (Compose's
/// `reverseScope`).
struct ReverseScope {
    logic: Rc<ScrollingLogic>,
    nested: Rc<dyn NestedScrollScope>,
}

impl ScrollScope for ReverseScope {
    fn scroll_by(&self, pixels: f32) -> f32 {
        // The node left: the fling is cancelled (Compose's FlingCancellationException), so what is left of its
        // velocity goes to the nested scroll parents. (Kotlin throws; this cancels its coroutine, which ends at its
        // next wait.)
        if pixels.abs() != 0.0 && !(self.logic.is_scrollable_node_attached)() {
            current_job().cancel();
            return 0.0;
        }
        let l = &self.logic;
        l.reverse_float_if_needed(l.offset_to_float(self.nested.scroll_by_with_overscroll(
            l.reverse_offset_if_needed(l.float_to_offset(pixels)),
            NestedScrollSource::SideEffect,
        )))
    }
}

impl ScrollLogic for ScrollingLogic {
    fn is_flinging(&self) -> bool {
        self.is_flinging.get()
    }

    /// Compose's `performRawScroll`: nothing while the state scrolls itself.
    fn perform_raw_scroll(&self, scroll: Offset) -> Offset {
        if self.state().is_scroll_in_progress() { Offset::Zero } else { self.dispatch_raw_delta(scroll) }
    }

    /// Compose's `doFlingAnimation`: the fling behavior run in a mutation of its own, the velocity left.
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
                        let reverse_scope: Rc<dyn ScrollScope> = Rc::new(ReverseScope { logic: logic.clone(), nested });
                        let fling_behavior = logic.fling_behavior.borrow().clone();
                        let left = fling_behavior
                            .perform_fling(
                                reverse_scope,
                                logic.reverse_float_if_needed(logic.velocity_to_float(available)),
                            )
                            .await;
                        r.set(logic.update_velocity(r.get(), logic.reverse_float_if_needed(left)));
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

/// The scrollable's link in the nested scroll chain (Compose's ScrollableNestedScrollConnection): what its children
/// leave scrolls it, their fling flings it.
pub(crate) struct ScrollableNestedScrollConnection {
    scrolling_logic: Rc<dyn ScrollLogicDyn>,
    pub(crate) enabled: Cell<bool>,
}

/// [`ScrollLogic`] behind an `Rc` (its fling takes the `Rc`).
pub(crate) trait ScrollLogicDyn {
    fn is_flinging(&self) -> bool;
    fn perform_raw_scroll(&self, scroll: Offset) -> Offset;
    fn do_fling_animation_dyn(&self, available: Velocity) -> LocalFuture<'static, Velocity>;
}

impl ScrollLogicDyn for ScrollingLogic {
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

impl ScrollableNestedScrollConnection {
    pub(crate) fn new(scrolling_logic: Rc<dyn ScrollLogicDyn>, enabled: bool) -> ScrollableNestedScrollConnection {
        ScrollableNestedScrollConnection { scrolling_logic, enabled: Cell::new(enabled) }
    }
}

impl NestedScrollConnection for ScrollableNestedScrollConnection {
    fn on_post_scroll(&self, _consumed: Offset, available: Offset, _source: NestedScrollSource) -> Offset {
        if self.enabled.get() { self.scrolling_logic.perform_raw_scroll(available) } else { Offset::Zero }
    }

    fn on_post_fling(&self, _consumed: Velocity, available: Velocity) -> kompoz_ui::input::FlingFuture<'_> {
        Box::pin(async move {
            if self.enabled.get() {
                let velocity_left = if self.scrolling_logic.is_flinging() {
                    Velocity::Zero
                } else {
                    self.scrolling_logic.do_fling_animation_dyn(available).await
                };
                available - velocity_left
            } else {
                Velocity::Zero
            }
        })
    }
}

/// Which pointers drag a scrollable: not a mouse (Compose's CanDragCalculation).
pub(crate) fn can_drag_calculation(pointer_type: PointerType) -> bool {
    pointer_type != PointerType::Mouse
}

// ---------------------------------------------------------------------------------------------------------------
// The node

/// `scrollable`'s node (Compose's ScrollableNode with its AbstractScrollableNode and DragGestureNode): it delegates
/// to its nested scroll link, a focus target that takes no focus itself, its bring-into-view node and responder.
pub struct ScrollableNode {
    pub(crate) core: Rc<ScrollableCore>,
}

pub(crate) struct ScrollableCore {
    me: Weak<ScrollableCore>,
    pub(crate) drag: Rc<DragGestureCore>,
    overscroll_effect: RefCell<Option<Rc<dyn OverscrollEffect>>>,
    fling_behavior: RefCell<Option<Rc<dyn FlingBehavior>>>,
    default_fling_behavior: Rc<DefaultFlingBehavior>,
    pub(crate) scroll_logic: Rc<ScrollingLogic>,
    nested_scroll_dispatcher: NestedScrollDispatcher,
    nested_scroll_connection: Rc<ScrollableNestedScrollConnection>,
    bring_into_view_spec: RefCell<Option<Rc<dyn BringIntoViewSpec>>>,
    focus_target: RefCell<Option<DelegatedNode<FocusTargetNode>>>,
    pub(crate) content_in_view: RefCell<Option<Rc<ContentInViewCore>>>,
    delegated: Cell<bool>,
    scroll_by_action: RefCell<Option<Rc<dyn Fn(f32, f32) -> bool>>>,
    scroll_by_offset_action: RefCell<Option<Rc<dyn Fn(Offset) -> LocalFuture<'static, Offset>>>>,
    created_mouse_wheel_scrolling_logic: Cell<bool>,
    created_trackpad_scrolling_logic: Cell<bool>,
    mouse_wheel_scrolling_logic: RefCell<Option<Rc<MouseWheelScrollingLogic>>>,
    trackpad_scrolling_logic: RefCell<Option<Rc<TrackpadScrollingLogic>>>,
    handle: RefCell<Option<NodeHandle>>,
}

impl ScrollableNode {
    pub(crate) fn new(e: &ScrollableElement) -> ScrollableNode {
        let core = Rc::new_cyclic(|me: &Weak<ScrollableCore>| {
            let drag = DragGestureCore::new(
                Rc::new(can_drag_calculation),
                e.enabled,
                e.interaction_source.clone(),
                Some(e.orientation),
            );
            let default_fling_behavior = platform_scrollable_default_fling_behavior();
            let nested_scroll_dispatcher = NestedScrollDispatcher::new();
            let (on_changed, attached) = (me.clone(), me.clone());
            let fling: Rc<dyn FlingBehavior> = match &e.fling_behavior {
                Some(f) => f.clone(),
                None => default_fling_behavior.clone(),
            };
            let scroll_logic = ScrollingLogic::new(
                e.state.clone(),
                e.overscroll_effect.clone(),
                fling,
                e.orientation,
                e.reverse_direction,
                nested_scroll_dispatcher.clone(),
                Rc::new(move |delta| {
                    if let Some(core) = on_changed.upgrade() {
                        core.dispatch_scroll_delta_info(delta);
                    }
                }),
                Rc::new(move || attached.upgrade().is_some_and(|c| c.is_attached())),
            );
            let logic: Rc<dyn ScrollLogicDyn> = scroll_logic.clone();
            let nested_scroll_connection = Rc::new(ScrollableNestedScrollConnection::new(logic, e.enabled));
            ScrollableCore {
                me: me.clone(),
                drag,
                overscroll_effect: RefCell::new(e.overscroll_effect.clone()),
                fling_behavior: RefCell::new(e.fling_behavior.clone()),
                default_fling_behavior,
                scroll_logic,
                nested_scroll_dispatcher,
                nested_scroll_connection,
                bring_into_view_spec: RefCell::new(e.bring_into_view_spec.clone()),
                focus_target: RefCell::new(None),
                content_in_view: RefCell::new(None),
                delegated: Cell::new(false),
                scroll_by_action: RefCell::new(None),
                scroll_by_offset_action: RefCell::new(None),
                created_mouse_wheel_scrolling_logic: Cell::new(false),
                created_trackpad_scrolling_logic: Cell::new(false),
                mouse_wheel_scrolling_logic: RefCell::new(None),
                trackpad_scrolling_logic: RefCell::new(None),
                handle: RefCell::new(None),
            }
        });
        let owner: Rc<dyn DragGestureOwner> = core.clone();
        core.drag.set_owner(&owner);
        ScrollableNode { core }
    }
}

impl ScrollableCore {
    fn cx(&self) -> Option<NodeCx> {
        self.handle.borrow().as_ref().and_then(|h| h.cx())
    }

    fn is_attached(&self) -> bool {
        self.cx().is_some_and(|c| c.is_attached())
    }

    fn enabled(&self) -> bool {
        self.drag.enabled.get()
    }

    /// Compose's `dispatchScrollDeltaInfo`.
    fn dispatch_scroll_delta_info(&self, delta: Offset) {
        if let Some(cx) = self.cx().filter(|c| c.is_attached()) {
            cx.dispatch_on_scroll_changed(delta);
        }
    }

    /// Its delegates, made the first time it is attached (Compose makes them as it makes the node): its focus
    /// target, its bring-into-view node, its nested scroll link, its responder.
    fn delegate_once(&self, cx: &NodeCx) {
        if self.delegated.replace(true) {
            return;
        }
        let focus_target = cx.delegate(FocusTargetNode::new(Focusability::Never, None));
        let focus_cx = focus_target.cx();
        *self.focus_target.borrow_mut() = Some(focus_target);
        let content_in_view = ContentInViewNode::new(
            self.scroll_logic.orientation.get(),
            self.scroll_logic.clone(),
            self.scroll_logic.reverse_direction.get(),
            self.bring_into_view_spec.borrow().clone(),
            Rc::new(move || focus_cx.focused_rect()),
        );
        let core = content_in_view.core.clone();
        cx.delegate(content_in_view);
        *self.content_in_view.borrow_mut() = Some(core.clone());
        let connection: Rc<dyn NestedScrollConnection> = self.nested_scroll_connection.clone();
        cx.delegate(nested_scroll_modifier_node(connection, Some(self.nested_scroll_dispatcher.clone())));
        let responder: Rc<dyn BringIntoViewResponder> = core;
        cx.delegate(BringIntoViewResponderNode::new(responder));
    }

    /// Compose's AbstractScrollableNode `onAttach`.
    fn on_attach(&self, cx: &NodeCx) {
        *self.handle.borrow_mut() = Some(cx.handle());
        self.drag.on_attach(cx);
        self.delegate_once(cx);
        let density = cx.density();
        if let Some(l) = self.mouse_wheel_scrolling_logic.borrow().as_ref() {
            l.update_density(density);
        }
        if let Some(l) = self.trackpad_scrolling_logic.borrow().as_ref() {
            l.update_density(density);
        }
        self.update_default_fling_behavior();
    }

    fn update_default_fling_behavior(&self) {
        let Some(cx) = self.cx().filter(|c| c.is_attached()) else { return };
        self.default_fling_behavior.update_density(cx.density());
    }

    /// Compose's AbstractScrollableNode `onDensityChange`.
    fn on_density_change(&self) {
        self.drag.on_cancel_pointer_input();
        let density = self.cx().map_or_else(kompoz_graphics::Density::default, |c| c.density());
        if let Some(l) = self.mouse_wheel_scrolling_logic.borrow().as_ref() {
            l.update_density(density);
        }
        if let Some(l) = self.trackpad_scrolling_logic.borrow().as_ref() {
            l.update_density(density);
        }
        self.on_cancel_pointer_input();
        self.update_default_fling_behavior();
    }

    /// Compose's ScrollableNode `update` (with AbstractScrollableNode's and DragGestureNode's).
    pub(crate) fn update(&self, e: &ScrollableElement) {
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
            e.orientation,
            e.overscroll_effect.clone(),
            e.reverse_direction,
            resolved_fling_behavior,
            self.nested_scroll_dispatcher.clone(),
        );
        *self.bring_into_view_spec.borrow_mut() = e.bring_into_view_spec.clone();
        if let Some(c) = self.content_in_view.borrow().as_ref() {
            c.update(e.orientation, e.reverse_direction, e.bring_into_view_spec.clone());
        }
        let orientation = if self.scroll_logic.is_vertical() { Orientation::Vertical } else { Orientation::Horizontal };
        self.drag.update(
            Rc::new(can_drag_calculation),
            e.enabled,
            e.interaction_source.clone(),
            Some(orientation),
            reset_pointer_input_handling,
        );
    }

    fn initialize_mouse_wheel_scrolling_logic(&self) {
        if !self.created_mouse_wheel_scrolling_logic.get() {
            let Some(cx) = self.cx() else { return };
            let me = self.me.clone();
            let logic = MouseWheelScrollingLogic::new(
                self.scroll_logic.clone(),
                super::mouse_wheel::platform_scroll_config(&cx),
                Rc::new(move |velocity| {
                    if let Some(core) = me.upgrade() {
                        core.on_mouse_wheel_scroll_stopped(velocity);
                    }
                }),
                cx.density(),
            );
            *self.mouse_wheel_scrolling_logic.borrow_mut() = Some(logic);
            self.created_mouse_wheel_scrolling_logic.set(true);
        }
        let logic = self.mouse_wheel_scrolling_logic.borrow().clone();
        if let (Some(logic), Some(scope)) = (logic, self.cx().map(|c| c.coroutine_scope())) {
            logic.start_receiving_events(&scope);
        }
    }

    fn initialize_trackpad_scrolling_logic(&self) {
        if !self.created_trackpad_scrolling_logic.get() {
            let Some(cx) = self.cx() else { return };
            let me = self.me.clone();
            let logic = TrackpadScrollingLogic::new(
                self.scroll_logic.clone(),
                Rc::new(move |velocity| {
                    if let Some(core) = me.upgrade() {
                        core.on_trackpad_scroll_stopped(velocity);
                    }
                }),
                cx.density(),
            );
            *self.trackpad_scrolling_logic.borrow_mut() = Some(logic);
            self.created_trackpad_scrolling_logic.set(true);
        }
        let logic = self.trackpad_scrolling_logic.borrow().clone();
        if let (Some(logic), Some(scope)) = (logic, self.cx().map(|c| c.coroutine_scope())) {
            logic.start_receiving_events(&scope);
        }
    }

    /// Compose's `onMouseWheelScrollStopped`.
    fn on_mouse_wheel_scroll_stopped(&self, velocity: Velocity) {
        let logic = self.scroll_logic.clone();
        self.nested_scroll_dispatcher.coroutine_scope().launch(async move {
            logic.on_scroll_stopped(velocity, true).await;
        });
    }

    /// Compose's `onTrackpadScrollStopped`.
    fn on_trackpad_scroll_stopped(&self, velocity: Velocity) {
        let logic = self.scroll_logic.clone();
        self.nested_scroll_dispatcher.coroutine_scope().launch(async move {
            logic.on_scroll_stopped(velocity, false).await;
        });
    }

    /// Compose's AbstractScrollableNode `onPointerEvent`.
    fn on_pointer_event(&self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        if event.changes.iter().any(|c| self.drag.can_drag(c.r#type)) {
            self.drag.on_pointer_event(event, pass, bounds);
        }
        if self.enabled() {
            self.drag.initialize_pointer_input_gesture_coordination();
            if pass == PointerEventPass::Initial && event.r#type == PointerEventType::Scroll {
                self.initialize_mouse_wheel_scrolling_logic();
            }
            let wheel = self.mouse_wheel_scrolling_logic.borrow().clone();
            if let Some(w) = wheel {
                w.on_pointer_event(event, pass, bounds);
            }
            if pass == PointerEventPass::Initial
                && matches!(
                    event.r#type,
                    PointerEventType::PanStart | PointerEventType::PanMove | PointerEventType::PanEnd
                )
            {
                self.initialize_trackpad_scrolling_logic();
            }
            let trackpad = self.trackpad_scrolling_logic.borrow().clone();
            if let Some(t) = trackpad {
                t.on_pointer_event(event, pass, bounds);
            }
        }
    }

    fn on_cancel_pointer_input(&self) {
        self.drag.on_cancel_pointer_input();
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

    /// Page Up and Page Down: a viewport's length (Compose's ScrollableNode `onKeyEvent`).
    fn on_key_event(&self, event: &KeyEvent) -> bool {
        if !(self.enabled()
            && (event.key == Key::PageDown || event.key == Key::PageUp)
            && event.r#type == KeyEventType::KeyDown
            && !event.is_ctrl_pressed())
        {
            return false;
        }
        let viewport = self.content_in_view.borrow().as_ref().map_or(IntSize::Zero, |c| c.viewport_size_or_zero());
        let scroll_amount = if self.scroll_logic.is_vertical() {
            let height = viewport.height as f32;
            Offset::new(0.0, if event.key == Key::PageUp { height } else { -height })
        } else {
            let width = viewport.width as f32;
            Offset::new(if event.key == Key::PageUp { width } else { -width }, 0.0)
        };
        // (a coroutine per key press)
        if let Some(scope) = self.cx().filter(|c| c.is_attached()).map(|c| c.coroutine_scope()) {
            let logic = self.scroll_logic.clone();
            scope.launch(logic.scroll(
                MutatePriority::UserInput,
                Box::new(move |scope| {
                    Box::pin(async move {
                        scope.scroll_by(scroll_amount, NestedScrollSource::UserInput);
                    })
                }),
            ));
        }
        true
    }
}

impl DragGestureOwner for ScrollableCore {
    /// Compose's ScrollableNode `drag`: the deltas through the overscroll, in a user's mutation (an indirect
    /// pointer's not reversed back: its deltas aren't a finger's on the content).
    fn drag(self: Rc<Self>, for_each_delta: ForEachDelta) -> LocalFuture<'static, ()> {
        let logic = self.scroll_logic.clone();
        logic.clone().scroll(
            MutatePriority::UserInput,
            Box::new(move |scope| {
                Box::pin(async move {
                    let process: Rc<dyn Fn(super::draggable::DragDelta)> = Rc::new(move |d| {
                        let invert_indirect_pointer = if d.is_indirect_pointer_event { -1.0 } else { 1.0 };
                        scope.scroll_by_with_overscroll(
                            logic.single_axis_offset(d.delta) * invert_indirect_pointer,
                            NestedScrollSource::UserInput,
                        );
                    });
                    for_each_delta(process).await;
                })
            }),
        )
    }

    fn on_drag_started(&self, _started_position: Offset) {}

    /// Compose's ScrollableNode `onDragStopped`: the fling, in the nested scroll chain's scope.
    fn on_drag_stopped(&self, event: DragStopped) {
        if !self.is_attached() {
            return;
        }
        let logic = self.scroll_logic.clone();
        let invert_indirect_pointer = if event.is_indirect_pointer_event { -1.0 } else { 1.0 };
        self.nested_scroll_dispatcher.coroutine_scope().launch(async move {
            logic.on_scroll_stopped(event.velocity * invert_indirect_pointer, false).await;
        });
    }

    fn start_drag_immediately(&self) -> bool {
        self.scroll_logic.should_scroll_immediately()
    }
}

impl ModifierNode for ScrollableNode {
    node_capabilities!(pointer_input, indirect_pointer_input, semantics, key_input);

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

impl PointerInputModifierNode for ScrollableNode {
    fn on_pointer_event(&mut self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        self.core.on_pointer_event(event, pass, bounds);
    }
    fn on_cancel_pointer_input(&mut self) {
        self.core.on_cancel_pointer_input();
    }
    fn on_density_change(&mut self) {
        self.core.on_density_change();
    }
}

impl IndirectPointerInputModifierNode for ScrollableNode {
    fn on_indirect_pointer_event(&mut self, event: &IndirectPointerEvent, pass: PointerEventPass) {
        self.core.drag.on_indirect_pointer_event(event, pass);
    }
    fn on_cancel_indirect_pointer_input(&mut self) {
        self.core.drag.on_cancel_indirect_pointer_input();
    }
}

impl KeyInputModifierNode for ScrollableNode {
    fn on_key_event(&mut self, event: &KeyEvent) -> bool {
        self.core.on_key_event(event)
    }
    fn on_pre_key_event(&mut self, _event: &KeyEvent) -> bool {
        false
    }
}

impl SemanticsModifierNode for ScrollableNode {
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
