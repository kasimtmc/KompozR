//! Content that comes in and goes (Compose's AnimatedVisibility.kt): [`AnimatedVisibility`] of a boolean or of a
//! [`MutableTransitionState`], its Row and Column forms (other defaults: across, down), `Transition::
//! AnimatedVisibility` of a state of a transition, `DeferredTransition::DeferredAnimatedVisibility`; the content's
//! [`AnimatedVisibilityScope`] (its own transition, `animate_enter_exit` for its children's own transitions). Content
//! on its way out takes no input (KompozR's, the user's choice over Compose 1.12: C++ Kompoz's `ignoringInput`).

use crate::deferred_enter_exit::{MutableTransform, SharedMutableTransformState};
use crate::enter_exit::{
    EnterExitState, EnterTransition, ExitTransition, always_enabled, create_modifier,
    deferred_transition_cleanup_effect, track_active_enter, track_active_exit,
};
use crate::transition::{
    DeferredTransition, MutableTransitionState, Transition, remember_transition, update_transition,
};
use crate::{
    AnimationValue, expand_horizontally, expand_in, expand_vertically, fade_in, fade_out, shrink_horizontally,
    shrink_out, shrink_vertically,
};
use kompoz_coroutines::{produce_state, snapshot_flow};
use kompoz_graphics::IntSize;
use kompoz_runtime::{
    MutableState, group, kompozable, mutable_state_of, named, remember, remember_keyed, remember_updated_state,
};
use kompoz_ui::input::{PointerEvent, PointerEventPass, PointerInputModifierNode};
use kompoz_ui::layout::{
    IntrinsicMeasurable, IntrinsicMeasureScope, Measurable, MeasurePolicy, MeasureResult, MeasureScope, PlacementScope,
    layout_into,
};
use kompoz_ui::{Constraints, Modifier, ModifierNode, ModifierNodeElement, node_capabilities};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---- The scope ----------------------------------------------------------------------------------------------------

/// What an [`AnimatedVisibility`]'s content is given (Compose's AnimatedVisibilityScope): its own transition, of
/// [`EnterExitState`]s, and [`animate_enter_exit`](Self::animate_enter_exit) for children coming in and going their
/// own ways. Clones are the same scope.
#[derive(Clone)]
pub struct AnimatedVisibilityScope {
    pub(crate) inner: Rc<AnimatedVisibilityScopeImpl>,
}

/// Compose's internal AnimatedVisibilityScopeImpl: the transition, the content's size where it is going (its
/// lookahead size in a lookahead scope), the shared mutable transform state of a deferred phase.
pub(crate) struct AnimatedVisibilityScopeImpl {
    transition: RefCell<Transition<EnterExitState>>,
    pub(crate) target_size: MutableState<IntSize>,
    pub(crate) shared_mutable_transform_state: SharedMutableTransformState,
}

impl PartialEq for AnimatedVisibilityScope {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl AnimatedVisibilityScope {
    pub(crate) fn new(transition: Transition<EnterExitState>) -> AnimatedVisibilityScope {
        AnimatedVisibilityScope {
            inner: Rc::new(AnimatedVisibilityScopeImpl {
                transition: RefCell::new(transition),
                target_size: mutable_state_of(IntSize::Zero),
                shared_mutable_transform_state: SharedMutableTransformState::new(),
            }),
        }
    }

    /// The content's own transition (Compose's `transition`): PreEnter → Visible as it comes in, Visible → PostExit
    /// as it goes; animate values of the content along it (`transition.animate_float(…)`, …).
    pub fn transition(&self) -> Transition<EnterExitState> {
        self.inner.transition.borrow().clone()
    }
}

#[named]
impl AnimatedVisibilityScope {
    /// `modifier` with an enter and an exit of its own (Compose's `Modifier.animateEnterExit` of the scope): what it
    /// is on comes in by `enter` and goes by `exit` as the content does, along the content's transition, after the
    /// content's own (the content stays until these are done too). Composable.
    ///
    /// Kotlin: `animateEnterExit(enter: EnterTransition = fadeIn(), exit: ExitTransition = fadeOut(), label: String = "animateEnterExit")`
    #[track_caller]
    pub fn animate_enter_exit(
        &self,
        modifier: Modifier,
        #[default(fade_in())] enter: EnterTransition,
        #[default(fade_out())] exit: ExitTransition,
        #[default("animateEnterExit".to_string())] label: String,
    ) -> Modifier {
        let transition = self.transition();
        group(move || modifier.then(create_modifier(&transition, enter, exit, true, always_enabled(), None, &label)))
    }
}

// ---- The composables ---------------------------------------------------------------------------------------------

/// Shows `content` while `visible` (Compose's `AnimatedVisibility(visible, …)`): it comes in by `enter` (by default
/// faded in and expanded from the bottom end) and goes by `exit` (shrunk and faded out), then leaves the composition.
/// The content is in a layout of its own, its children one over the other; its scope has its transition and
/// `animate_enter_exit`. In a lookahead scope, it is laid out where it is going in the lookahead pass (no size once
/// it is going), animated in the main one. Content on its way out takes no input (taps, scrolls, hovers, focus
/// moves): KompozR's, as C++ Kompoz, unlike Compose 1.12. `label` names its transition for tools.
///
/// Kotlin: `AnimatedVisibility(visible: Boolean, modifier: Modifier = Modifier, enter: EnterTransition = fadeIn() + expandIn(), exit: ExitTransition = shrinkOut() + fadeOut(), label: String = "AnimatedVisibility", content: @Composable AnimatedVisibilityScope.() -> Unit)`
#[kompozable]
pub fn AnimatedVisibility(
    visible: bool,
    #[default(Modifier)] modifier: Modifier,
    #[default(fade_in() + expand_in())] enter: EnterTransition,
    #[default(shrink_out() + fade_out())] exit: ExitTransition,
    #[default("AnimatedVisibility".to_string())] label: String,
    content: impl Fn(&AnimatedVisibilityScope) + 'static,
) {
    let transition = update_transition(visible, Some(label));
    animated_visibility_impl(&transition, Rc::new(|v: &bool| *v), modifier, enter, exit, None, content);
}

/// [`AnimatedVisibility`] in a Row (Compose's `RowScope.AnimatedVisibility(visible, …)`): it comes in faded and
/// expanded across, goes faded and shrunk across by default. (Kotlin's receiver scope: the Kotlin syntax calls it
/// for `AnimatedVisibility(…)` inside `Row { }`.)
///
/// Kotlin: `RowScope.AnimatedVisibility(visible: Boolean, modifier: Modifier = Modifier, enter: EnterTransition = fadeIn() + expandHorizontally(), exit: ExitTransition = fadeOut() + shrinkHorizontally(), label: String = "AnimatedVisibility", content: @Composable AnimatedVisibilityScope.() -> Unit)`
#[kompozable]
pub fn RowAnimatedVisibility(
    visible: bool,
    #[default(Modifier)] modifier: Modifier,
    #[default(fade_in() + expand_horizontally())] enter: EnterTransition,
    #[default(fade_out() + shrink_horizontally())] exit: ExitTransition,
    #[default("AnimatedVisibility".to_string())] label: String,
    content: impl Fn(&AnimatedVisibilityScope) + 'static,
) {
    let transition = update_transition(visible, Some(label));
    animated_visibility_impl(&transition, Rc::new(|v: &bool| *v), modifier, enter, exit, None, content);
}

/// [`AnimatedVisibility`] in a Column (Compose's `ColumnScope.AnimatedVisibility(visible, …)`): it comes in faded
/// and expanded down, goes faded and shrunk up by default. (Kotlin's receiver scope: the Kotlin syntax calls it for
/// `AnimatedVisibility(…)` inside `Column { }`.)
///
/// Kotlin: `ColumnScope.AnimatedVisibility(visible: Boolean, modifier: Modifier = Modifier, enter: EnterTransition = fadeIn() + expandVertically(), exit: ExitTransition = fadeOut() + shrinkVertically(), label: String = "AnimatedVisibility", content: @Composable AnimatedVisibilityScope.() -> Unit)`
#[kompozable]
pub fn ColumnAnimatedVisibility(
    visible: bool,
    #[default(Modifier)] modifier: Modifier,
    #[default(fade_in() + expand_vertically())] enter: EnterTransition,
    #[default(fade_out() + shrink_vertically())] exit: ExitTransition,
    #[default("AnimatedVisibility".to_string())] label: String,
    content: impl Fn(&AnimatedVisibilityScope) + 'static,
) {
    let transition = update_transition(visible, Some(label));
    animated_visibility_impl(&transition, Rc::new(|v: &bool| *v), modifier, enter, exit, None, content);
}

/// Shows `content` as `visible_state` says (Compose's `AnimatedVisibility(visibleState, …)`): its target set to
/// true brings it in, to false takes it out; its current state and `is_idle` tell where it is (a state whose
/// current state is false and target true comes in as it first composes).
///
/// Kotlin: `AnimatedVisibility(visibleState: MutableTransitionState<Boolean>, modifier: Modifier = Modifier, enter: EnterTransition = fadeIn() + expandIn(), exit: ExitTransition = fadeOut() + shrinkOut(), label: String = "AnimatedVisibility", content: @Composable AnimatedVisibilityScope.() -> Unit)`
#[kompozable]
pub fn AnimatedVisibilityWithState(
    visible_state: MutableTransitionState<bool>,
    #[default(Modifier)] modifier: Modifier,
    #[default(fade_in() + expand_in())] enter: EnterTransition,
    #[default(fade_out() + shrink_out())] exit: ExitTransition,
    #[default("AnimatedVisibility".to_string())] label: String,
    content: impl Fn(&AnimatedVisibilityScope) + 'static,
) {
    let transition = remember_transition(visible_state, Some(label));
    animated_visibility_impl(&transition, Rc::new(|v: &bool| *v), modifier, enter, exit, None, content);
}

/// [`AnimatedVisibilityWithState`] in a Row (Compose's `RowScope.AnimatedVisibility(visibleState, …)`).
///
/// Kotlin: `RowScope.AnimatedVisibility(visibleState: MutableTransitionState<Boolean>, modifier: Modifier = Modifier, enter: EnterTransition = expandHorizontally() + fadeIn(), exit: ExitTransition = shrinkHorizontally() + fadeOut(), label: String = "AnimatedVisibility", content: @Composable AnimatedVisibilityScope.() -> Unit)`
#[kompozable]
pub fn RowAnimatedVisibilityWithState(
    visible_state: MutableTransitionState<bool>,
    #[default(Modifier)] modifier: Modifier,
    #[default(expand_horizontally() + fade_in())] enter: EnterTransition,
    #[default(shrink_horizontally() + fade_out())] exit: ExitTransition,
    #[default("AnimatedVisibility".to_string())] label: String,
    content: impl Fn(&AnimatedVisibilityScope) + 'static,
) {
    let transition = remember_transition(visible_state, Some(label));
    animated_visibility_impl(&transition, Rc::new(|v: &bool| *v), modifier, enter, exit, None, content);
}

/// [`AnimatedVisibilityWithState`] in a Column (Compose's `ColumnScope.AnimatedVisibility(visibleState, …)`).
///
/// Kotlin: `ColumnScope.AnimatedVisibility(visibleState: MutableTransitionState<Boolean>, modifier: Modifier = Modifier, enter: EnterTransition = expandVertically() + fadeIn(), exit: ExitTransition = shrinkVertically() + fadeOut(), label: String = "AnimatedVisibility", content: @Composable AnimatedVisibilityScope.() -> Unit)`
#[kompozable]
pub fn ColumnAnimatedVisibilityWithState(
    visible_state: MutableTransitionState<bool>,
    #[default(Modifier)] modifier: Modifier,
    #[default(expand_vertically() + fade_in())] enter: EnterTransition,
    #[default(shrink_vertically() + fade_out())] exit: ExitTransition,
    #[default("AnimatedVisibility".to_string())] label: String,
    content: impl Fn(&AnimatedVisibilityScope) + 'static,
) {
    let transition = remember_transition(visible_state, Some(label));
    animated_visibility_impl(&transition, Rc::new(|v: &bool| *v), modifier, enter, exit, None, content);
}

#[named]
impl<S: AnimationValue> Transition<S> {
    /// Shows `content` while `visible` says this transition's state shows it (Compose's experimental
    /// `Transition<T>.AnimatedVisibility(visible, …)`): it comes in and goes along this transition, which runs until
    /// it is in or out. Composable.
    ///
    /// Kotlin: `AnimatedVisibility(visible: (T) -> Boolean, modifier: Modifier = Modifier, enter: EnterTransition = fadeIn() + expandIn(), exit: ExitTransition = shrinkOut() + fadeOut(), content: @Composable AnimatedVisibilityScope.() -> Unit)`
    #[track_caller]
    pub fn AnimatedVisibility(
        &self,
        visible: impl Fn(&S) -> bool + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(fade_in() + expand_in())] enter: EnterTransition,
        #[default(shrink_out() + fade_out())] exit: ExitTransition,
        content: impl Fn(&AnimatedVisibilityScope) + 'static,
    ) {
        kompoz_runtime::kompozable_block!({
            TransitionAnimatedVisibility(self.clone(), visible, modifier, enter, exit, None, content);
        });
    }
}

#[named]
impl<S: AnimationValue> DeferredTransition<S> {
    /// [`Transition::AnimatedVisibility`] of a deferred transition (Compose's experimental
    /// `DeferredTransition<T>.DeferredAnimatedVisibility`): while its state defers, `mutable_transform`'s block
    /// sets the content's alpha, scale, offset… by hand; the transition takes over from there. Composable.
    ///
    /// Kotlin: `DeferredAnimatedVisibility(visible: (T) -> Boolean, modifier: Modifier = Modifier, enter: EnterTransition = fadeIn() + expandIn(), exit: ExitTransition = shrinkOut() + fadeOut(), mutableTransform: MutableTransform? = null, content: @Composable AnimatedVisibilityScope.() -> Unit)`
    #[track_caller]
    pub fn DeferredAnimatedVisibility(
        &self,
        visible: impl Fn(&S) -> bool + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(fade_in() + expand_in())] enter: EnterTransition,
        #[default(shrink_out() + fade_out())] exit: ExitTransition,
        #[default(None)] mutable_transform: Option<MutableTransform>,
        content: impl Fn(&AnimatedVisibilityScope) + 'static,
    ) {
        kompoz_runtime::kompozable_block!({
            TransitionAnimatedVisibility(
                self.transition().clone(),
                visible,
                modifier,
                enter,
                exit,
                mutable_transform,
                content,
            );
        });
    }
}

/// The restartable body of `Transition::AnimatedVisibility` and `DeferredTransition::DeferredAnimatedVisibility` (a
/// scope of its own, as Compose's composable extension functions are).
#[kompozable]
fn TransitionAnimatedVisibility<S: AnimationValue>(
    transition: Transition<S>,
    visible: impl Fn(&S) -> bool + 'static,
    modifier: Modifier,
    enter: EnterTransition,
    exit: ExitTransition,
    mutable_transform: Option<MutableTransform>,
    content: impl Fn(&AnimatedVisibilityScope) + 'static,
) {
    animated_visibility_impl(&transition, Rc::new(visible), modifier, enter, exit, mutable_transform, content);
}

// ---- The implementation -------------------------------------------------------------------------------------------

/// A state's visibility (Compose's `visible: (T) -> Boolean`).
pub(crate) type Visible<S> = Rc<dyn Fn(&S) -> bool>;

/// Compose's internal `AnimatedVisibilityImpl`: in the lookahead pass, no size once it is going.
#[track_caller]
fn animated_visibility_impl<S: AnimationValue>(
    transition: &Transition<S>,
    visible: Visible<S>,
    modifier: Modifier,
    enter: EnterTransition,
    exit: ExitTransition,
    mutable_transform: Option<MutableTransform>,
    content: impl Fn(&AnimatedVisibilityScope) + 'static,
) {
    let (t, v) = (transition.clone(), visible.clone());
    let modifier = modifier.layout(move |scope, measurable, constraints| {
        let placeable = measurable.measure(constraints);
        let (w, h) = if scope.is_looking_ahead() && !v(&t.target_state()) {
            (0, 0)
        } else {
            (placeable.width(), placeable.height())
        };
        scope.layout(w, h, move |p: &PlacementScope| p.place(&placeable, 0, 0))
    });
    animated_enter_exit_impl(
        transition,
        visible,
        modifier,
        enter,
        exit,
        Rc::new(|current, target| current == target && target == EnterExitState::PostExit),
        None,
        mutable_transform,
        content,
    );
}

/// What a content is told of its lookahead size (Compose's internal OnLookaheadMeasured).
pub(crate) type OnLookaheadMeasured = Rc<dyn Fn(IntSize)>;

/// When a content's layout leaves after its exit (Compose's `shouldDisposeBlock`), of its transition's current and
/// target states.
pub(crate) type ShouldDispose = Rc<dyn Fn(EnterExitState, EnterExitState) -> bool>;

/// Whether a content's transition is out (Compose's private `exitFinished`).
fn exit_finished(transition: &Transition<EnterExitState>) -> bool {
    transition.current_state() == EnterExitState::PostExit && transition.target_state() == EnterExitState::PostExit
}

/// The content of a transition's state while `visible` says it is shown, or it is on its way (Compose's internal
/// `AnimatedEnterExitImpl`): a child transition of [`EnterExitState`]s, the content's layout with its enter/exit
/// modifier, kept until its exit is done and `should_dispose_block` says it can go.
#[allow(clippy::too_many_arguments)]
#[track_caller]
pub(crate) fn animated_enter_exit_impl<S: AnimationValue>(
    transition: &Transition<S>,
    visible: Visible<S>,
    modifier: Modifier,
    enter: EnterTransition,
    exit: ExitTransition,
    should_dispose_block: ShouldDispose,
    on_lookahead_measured: Option<OnLookaheadMeasured>,
    mutable_transform_data: Option<MutableTransform>,
    content: impl Fn(&AnimatedVisibilityScope) + 'static,
) {
    let transition = transition.clone();
    group(move || {
        let local_pending_target_state = transition.pending_target_state();
        let has_been_pending = remember(|| mutable_state_of(false));
        let h = has_been_pending.clone();
        deferred_transition_cleanup_effect(&transition, move || h.set(false));
        if local_pending_target_state.as_ref().is_some_and(|p| visible(p)) {
            has_been_pending.set(true);
        }
        let shown = visible(&transition.target_state())
            || visible(&transition.current_state())
            || local_pending_target_state.as_ref().is_some_and(|p| visible(p))
            || (has_been_pending.get() && transition.current_state() != transition.target_state())
            || transition.is_seeking()
            || transition.has_initial_value_animations();
        if !shown {
            return;
        }
        group(move || {
            let (t, v) = (transition.clone(), visible.clone());
            let child_transition = kompoz_runtime::named_args!(
                transition.create_child_transition(label = "EnterExitTransition".to_string(), move |it: &S| {
                    target_enter_exit(&t, &v, it)
                })
            );
            // (tracked here, they survive the layout leaving when an exit is done: an interruption A → B → A after
            // it keeps the exit's ends)
            let active_enter = track_active_enter(&child_transition, enter);
            let active_exit = track_active_exit(&child_transition, exit);
            let should_dispose_updated = remember_updated_state!(should_dispose_block.clone());
            let initial = should_dispose_block(child_transition.current_state(), child_transition.target_state());
            let child = child_transition.clone();
            let should_dispose_after_exit = produce_state(initial, (), move |state| {
                let flow = {
                    let child = child.clone();
                    snapshot_flow(move || exit_finished(&child))
                };
                async move {
                    flow.collect(move |finished| {
                        let value = if finished {
                            (should_dispose_updated.get())(child.current_state(), child.target_state())
                        } else {
                            false
                        };
                        state.set(value);
                    })
                    .await;
                }
            });
            if exit_finished(&child_transition) && should_dispose_after_exit {
                return;
            }
            group(move || {
                let c = child_transition.clone();
                let scope = remember_keyed(transition.clone(), move || AnimatedVisibilityScope::new(c));
                scope.inner.shared_mutable_transform_state.set_mutable_data(mutable_transform_data);
                let enter_exit = create_modifier(
                    &child_transition,
                    active_enter,
                    active_exit,
                    false,
                    always_enabled(),
                    Some(scope.inner.shared_mutable_transform_state.clone()),
                    "Built-in",
                );
                let lookahead_measured = match on_lookahead_measured {
                    Some(on) => Modifier.layout(move |s, measurable, constraints| {
                        let placeable = measurable.measure(constraints);
                        let (w, h) = (placeable.width(), placeable.height());
                        if s.is_looking_ahead() {
                            on(IntSize::new(w, h));
                        }
                        s.layout(w, h, move |p: &PlacementScope| p.place(&placeable, 0, 0))
                    }),
                    None => Modifier,
                };
                // (D5: on its way out it takes no input)
                let leaving = child_transition.target_state() == EnterExitState::PostExit;
                let input = if leaving { Modifier.then_element(LeavingContentElement) } else { Modifier };
                let s = scope.clone();
                let policy = remember(move || {
                    Rc::new(AnimatedEnterExitMeasurePolicy { scope: s, has_lookahead_occurred: Cell::new(false) })
                        as Rc<dyn MeasurePolicy>
                });
                layout_into(modifier.then(enter_exit).then(lookahead_measured).then(input), policy, false, move || {
                    content(&scope)
                });
            });
        });
    });
}

/// The [`EnterExitState`] a state of the parent transition is (Compose's private `targetEnterExit`): Visible when
/// shown; PostExit once it has been shown (or it is pending), else PreEnter.
#[track_caller]
fn target_enter_exit<S: AnimationValue>(
    transition: &Transition<S>,
    visible: &Visible<S>,
    target_state: &S,
) -> EnterExitState {
    let id = Rc::as_ptr(&transition.as_any()) as *const () as usize;
    kompoz_runtime::key(&id, || {
        if transition.is_seeking() {
            return if visible(target_state) {
                EnterExitState::Visible
            } else if visible(&transition.current_state()) {
                EnterExitState::PostExit
            } else {
                EnterExitState::PreEnter
            };
        }
        let has_been_visible = remember(|| mutable_state_of(false));
        let local_pending_target_state = transition.pending_target_state();
        if visible(&transition.current_state()) || local_pending_target_state.as_ref().is_some_and(|p| visible(p)) {
            has_been_visible.set(true);
        }
        if visible(target_state) {
            EnterExitState::Visible
        } else if local_pending_target_state.as_ref().is_some_and(|p| visible(p)) {
            EnterExitState::PreEnter
        } else if has_been_visible.get() {
            EnterExitState::PostExit
        } else {
            EnterExitState::PreEnter
        }
    })
}

/// Compose's private AnimatedEnterExitMeasurePolicy: the children one over the other at the top start, as big as the
/// biggest; the size where it is going told to the scope (the lookahead one when there is a lookahead).
struct AnimatedEnterExitMeasurePolicy {
    scope: AnimatedVisibilityScope,
    has_lookahead_occurred: Cell<bool>,
}

impl MeasurePolicy for AnimatedEnterExitMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        let mut max_width = 0;
        let mut max_height = 0;
        let placeables: Vec<_> = measurables
            .iter()
            .map(|m| {
                let p = m.measure(constraints);
                max_width = max_width.max(p.width());
                max_height = max_height.max(p.height());
                p
            })
            .collect();
        if scope.is_looking_ahead() {
            self.has_lookahead_occurred.set(true);
            self.scope.inner.target_size.set(IntSize::new(max_width, max_height));
        } else if !self.has_lookahead_occurred.get() {
            // (not in a lookahead scope)
            self.scope.inner.target_size.set(IntSize::new(max_width, max_height));
        }
        scope.layout(max_width, max_height, move |p: &PlacementScope| {
            for placeable in &placeables {
                p.place(placeable, 0, 0);
            }
        })
    }

    fn min_intrinsic_width(&self, _: &IntrinsicMeasureScope, m: &[&dyn IntrinsicMeasurable], height: i32) -> i32 {
        m.iter().map(|m| m.min_intrinsic_width(height)).max().unwrap_or(0)
    }
    fn min_intrinsic_height(&self, _: &IntrinsicMeasureScope, m: &[&dyn IntrinsicMeasurable], width: i32) -> i32 {
        m.iter().map(|m| m.min_intrinsic_height(width)).max().unwrap_or(0)
    }
    fn max_intrinsic_width(&self, _: &IntrinsicMeasureScope, m: &[&dyn IntrinsicMeasurable], height: i32) -> i32 {
        m.iter().map(|m| m.max_intrinsic_width(height)).max().unwrap_or(0)
    }
    fn max_intrinsic_height(&self, _: &IntrinsicMeasureScope, m: &[&dyn IntrinsicMeasurable], width: i32) -> i32 {
        m.iter().map(|m| m.max_intrinsic_height(width)).max().unwrap_or(0)
    }
}

// ---- D5: leaving content takes no input -----------------------------------------------------------------------------

/// What a content on its way out is given (KompozR's, the user's choice over Compose 1.12, as C++ Kompoz's
/// `ignoringInput`): a pointer input node saying its layout takes no input — hit tests pass it by, focus searches
/// skip what is in it. Only there while the content leaves (a node of no use otherwise: it would take hits).
#[derive(PartialEq)]
pub(crate) struct LeavingContentElement;

impl ModifierNodeElement for LeavingContentElement {
    type Node = LeavingContentNode;
    fn create(&self) -> LeavingContentNode {
        LeavingContentNode
    }
    fn update(&self, _: &mut LeavingContentNode) {}
    fn name(&self) -> &'static str {
        "leavingContent"
    }
}

pub(crate) struct LeavingContentNode;

impl ModifierNode for LeavingContentNode {
    node_capabilities!(pointer_input);
}

impl PointerInputModifierNode for LeavingContentNode {
    fn on_pointer_event(&mut self, _: &PointerEvent, _: PointerEventPass, _: IntSize) {}
    fn on_cancel_pointer_input(&mut self) {}
    fn blocks_input(&self) -> bool {
        true
    }
}
