//! Content that changes with a state, the old one going as the new comes in (Compose's AnimatedContent.kt):
//! [`AnimatedContent`], `Transition::AnimatedContent`, `DeferredTransition::DeferredAnimatedContent`; what a change does
//! ([`ContentTransform`]: an enter of the new content `together_with` an exit of the old, a z-index, a
//! [`SizeTransform`] the container's size animates by — `using`), picked from the states in an
//! [`AnimatedContentTransitionScope`] (which also slides into and out of the container); the content's
//! [`AnimatedContentScope`].

use crate::animated_visibility::{AnimatedVisibilityScope, animated_enter_exit_impl};
use crate::converter::VectorConverter;
use crate::deferred_enter_exit::MutableContentTransform;
use crate::enter_exit::{
    EnterExitState, EnterTransition, ExitTransition, medium_low_spring, medium_low_threshold_spring, slide_in_of,
    slide_out_of,
};
use crate::spec::FiniteAnimationSpec;
use crate::transition::{DeferredAnimation, DeferredTransition, Segment, Transition, update_transition};
use crate::vector::AnimationVector2D;
use crate::{AnimationValue, fade_in, fade_out, scale_in, tween};
use kompoz_graphics::{IntOffset, IntSize, LayoutDirection};
use kompoz_runtime::{
    DisposableEffect, MutableState, SnapshotStateList, State, group, kompozable, mutable_state_list_of,
    mutable_state_of, named, on_dispose, remember, remember_keyed,
};
use kompoz_ui::layout::{
    IntrinsicMeasurable, IntrinsicMeasureScope, LayoutModifierNode, Measurable, MeasurePolicy, MeasureResult,
    MeasureScope, ParentDataModifierNode, Placeable, PlacementScope, SlotKey, layout_into,
};
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::{Alignment, Constraints, Modifier, ModifierNode, ModifierNodeElement, NodeCx, node_capabilities};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

// ---- SizeTransform, ContentTransform -----------------------------------------------------------------------------

/// How the container's size animates from the old content's to the new one's (Compose's SizeTransform): whether
/// the content is clipped to the size along the way, and the spec of each change (of its initial and target sizes).
pub trait SizeTransform {
    /// Whether the content is clipped to the size along the way.
    fn clip(&self) -> bool;
    /// The spec of a change from `initial_size` to `target_size`.
    fn create_animation_spec(
        &self,
        initial_size: IntSize,
        target_size: IntSize,
    ) -> Arc<dyn FiniteAnimationSpec<IntSize>>;
}

/// Compose's private SizeTransformImpl.
struct SizeTransformImpl {
    clip: bool,
    size_animation_spec: Rc<dyn Fn(IntSize, IntSize) -> Arc<dyn FiniteAnimationSpec<IntSize>>>,
}

impl SizeTransform for SizeTransformImpl {
    fn clip(&self) -> bool {
        self.clip
    }
    fn create_animation_spec(
        &self,
        initial_size: IntSize,
        target_size: IntSize,
    ) -> Arc<dyn FiniteAnimationSpec<IntSize>> {
        (self.size_animation_spec)(initial_size, target_size)
    }
}

impl fmt::Debug for dyn SizeTransform {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SizeTransform(clip={})", self.clip())
    }
}

/// The spec of a [`SizeTransform`]'s changes unless told otherwise (Compose's default `sizeAnimationSpec`): a spring,
/// medium-low stiff, there within a pixel.
pub fn default_size_animation_spec(_: IntSize, _: IntSize) -> Arc<dyn FiniteAnimationSpec<IntSize>> {
    Arc::new(medium_low_threshold_spring::<IntSize>())
}

/// A [`SizeTransform`] (Compose's `SizeTransform(clip, sizeAnimationSpec)`): clipped unless not `clip`, each change
/// along what `size_animation_spec` gives for its sizes (Kotlin's defaults: clipped, a spring —
/// [`default_size_animation_spec`]; a trait and a function of one name take no Rust defaults).
///
/// Kotlin: `SizeTransform(clip: Boolean = true, sizeAnimationSpec: (initialSize: IntSize, targetSize: IntSize) -> FiniteAnimationSpec<IntSize> = { _, _ -> spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = IntSize.VisibilityThreshold) })`
pub fn SizeTransform(
    clip: bool,
    size_animation_spec: impl Fn(IntSize, IntSize) -> Arc<dyn FiniteAnimationSpec<IntSize>> + 'static,
) -> Rc<dyn SizeTransform> {
    Rc::new(SizeTransformImpl { clip, size_animation_spec: Rc::new(size_animation_spec) })
}

/// What a change of an AnimatedContent does (Compose's ContentTransform): the new content comes in by
/// `target_content_enter`, the old goes by `initial_content_exit`; the new is drawn at `target_content_z_index`
/// against the old (0: over it, as the newer); the container's size animates by `size_transform` (none: it is as big
/// as the contents shown). Clones are the same transform.
#[derive(Clone)]
pub struct ContentTransform {
    inner: Rc<ContentTransformInner>,
}

struct ContentTransformInner {
    target_content_enter: EnterTransition,
    initial_content_exit: ExitTransition,
    target_content_z_index: MutableState<f32>,
    size_transform: RefCell<Option<Rc<dyn SizeTransform>>>,
}

impl PartialEq for ContentTransform {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for ContentTransform {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // (Compose's internal toDebugString)
        write!(
            f,
            "ContentTransform(targetContentEnter={}, initialContentExit={}, targetContentZIndex={:?}, sizeTransform={:?})",
            self.inner.target_content_enter,
            self.inner.initial_content_exit,
            self.inner.target_content_z_index.peek(),
            self.inner.size_transform.borrow()
        )
    }
}

/// A [`ContentTransform`] (Compose's constructor `ContentTransform(targetContentEnter, initialContentExit,
/// targetContentZIndex, sizeTransform)`).
///
/// Kotlin: `ContentTransform(targetContentEnter: EnterTransition, initialContentExit: ExitTransition, targetContentZIndex: Float = 0f, sizeTransform: SizeTransform? = SizeTransform())`
#[named(type)]
pub fn ContentTransform(
    target_content_enter: EnterTransition,
    initial_content_exit: ExitTransition,
    #[default(0.0)] target_content_z_index: f32,
    #[default(Some(default_size_transform()))] size_transform: Option<Rc<dyn SizeTransform>>,
) -> ContentTransform {
    content_transform_of(target_content_enter, initial_content_exit, target_content_z_index, size_transform)
}

fn default_size_transform() -> Rc<dyn SizeTransform> {
    Rc::new(SizeTransformImpl { clip: true, size_animation_spec: Rc::new(default_size_animation_spec) })
}

fn content_transform_of(
    target_content_enter: EnterTransition,
    initial_content_exit: ExitTransition,
    target_content_z_index: f32,
    size_transform: Option<Rc<dyn SizeTransform>>,
) -> ContentTransform {
    ContentTransform {
        inner: Rc::new(ContentTransformInner {
            target_content_enter,
            initial_content_exit,
            target_content_z_index: mutable_state_of(target_content_z_index),
            size_transform: RefCell::new(size_transform),
        }),
    }
}

impl ContentTransform {
    /// `enter` together with `exit`, the defaults otherwise (what `together_with` makes).
    pub(crate) fn new(enter: EnterTransition, exit: ExitTransition) -> ContentTransform {
        content_transform_of(enter, exit, 0.0, Some(default_size_transform()))
    }

    /// How the new content comes in.
    pub fn target_content_enter(&self) -> EnterTransition {
        self.inner.target_content_enter.clone()
    }

    /// How the old content goes.
    pub fn initial_content_exit(&self) -> ExitTransition {
        self.inner.initial_content_exit.clone()
    }

    /// The new content's z-index against the old (a state).
    pub fn target_content_z_index(&self) -> f32 {
        self.inner.target_content_z_index.get()
    }

    /// Sets it (Compose's `var targetContentZIndex`).
    pub fn set_target_content_z_index(&self, z_index: f32) {
        self.inner.target_content_z_index.set(z_index);
    }

    /// How the container's size animates (none: it doesn't).
    pub fn size_transform(&self) -> Option<Rc<dyn SizeTransform>> {
        self.inner.size_transform.borrow().clone()
    }
}

// ---- AnimatedContentTransitionScope ----------------------------------------------------------------------------------

/// A way content slides into or out of its container (Compose's `AnimatedContentTransitionScope.SlideDirection`):
/// toward the left, the right, up, down, the start or the end (the last two by the layout direction).
///
/// Kotlin: `AnimatedContentTransitionScope.SlideDirection`
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlideDirection(i32);

impl SlideDirection {
    pub const Left: SlideDirection = SlideDirection(0);
    pub const Right: SlideDirection = SlideDirection(1);
    pub const Up: SlideDirection = SlideDirection(2);
    pub const Down: SlideDirection = SlideDirection(3);
    pub const Start: SlideDirection = SlideDirection(4);
    pub const End: SlideDirection = SlideDirection(5);
}

impl fmt::Display for SlideDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self.0 {
            0 => "Left",
            1 => "Right",
            2 => "Up",
            3 => "Down",
            4 => "Start",
            5 => "End",
            _ => "Invalid",
        })
    }
}

impl fmt::Debug for SlideDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Compose's internal AnimatedContentTransitionScopeImpl: the transition, the alignment and direction, the size
/// measured, the sizes where each content is going, the container's size animation.
pub(crate) struct AnimatedContentTransitionScopeImpl<S> {
    transition: Transition<S>,
    content_alignment: RefCell<Alignment>,
    layout_direction: Cell<LayoutDirection>,
    measured_size: MutableState<IntSize>,
    target_size_map: RefCell<Vec<(S, MutableState<IntSize>)>>,
    animated_size: RefCell<Option<State<IntSize>>>,
}

impl<S: AnimationValue> AnimatedContentTransitionScopeImpl<S> {
    fn new(transition: Transition<S>, content_alignment: Alignment, layout_direction: LayoutDirection) -> Self {
        AnimatedContentTransitionScopeImpl {
            transition,
            content_alignment: RefCell::new(content_alignment),
            layout_direction: Cell::new(layout_direction),
            measured_size: mutable_state_of(IntSize::Zero),
            target_size_map: RefCell::new(Vec::new()),
            animated_size: RefCell::new(None),
        }
    }

    fn target_size_of(&self, state: &S) -> Option<IntSize> {
        let size = self.target_size_map.borrow().iter().find(|(s, _)| s == state).map(|(_, v)| v.clone());
        size.map(|s| s.get())
    }

    /// The container's size now: the animated one, else the one measured (Compose's private `currentSize`).
    fn current_size(&self) -> IntSize {
        let animated = self.animated_size.borrow().clone();
        animated.map_or_else(|| self.measured_size.get(), |s| s.get())
    }

    fn calculate_offset(&self, full_size: IntSize, current_size: IntSize) -> IntOffset {
        self.content_alignment.borrow().align(full_size, current_size, LayoutDirection::Ltr)
    }

    fn is_left(&self, d: SlideDirection) -> bool {
        let rtl = self.layout_direction.get() == LayoutDirection::Rtl;
        d == SlideDirection::Left || d == SlideDirection::Start && !rtl || d == SlideDirection::End && rtl
    }

    fn is_right(&self, d: SlideDirection) -> bool {
        let rtl = self.layout_direction.get() == LayoutDirection::Rtl;
        d == SlideDirection::Right || d == SlideDirection::Start && rtl || d == SlideDirection::End && !rtl
    }
}

/// Where an AnimatedContent's change is picked (Compose's AnimatedContentTransitionScope, a `Transition.Segment`):
/// the states it goes from and to (`initial_state`, `target_state`, `is_transitioning_to`), `using` a
/// [`SizeTransform`], the container's alignment, transitions sliding into and out of the container. Clones are the
/// same scope.
pub struct AnimatedContentTransitionScope<S> {
    root: Rc<AnimatedContentTransitionScopeImpl<S>>,
    /// Compose's PendingAnimatedContentTransitionScope: other states than the transition's segment.
    overrides: Option<(S, S)>,
}

impl<S: Clone> Clone for AnimatedContentTransitionScope<S> {
    fn clone(&self) -> Self {
        AnimatedContentTransitionScope { root: self.root.clone(), overrides: self.overrides.clone() }
    }
}

impl<S: AnimationValue> PartialEq for AnimatedContentTransitionScope<S> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.root, &other.root) && self.overrides == other.overrides
    }
}

impl<S: AnimationValue> AnimatedContentTransitionScope<S> {
    /// The state the change goes from.
    pub fn initial_state(&self) -> S {
        match &self.overrides {
            Some((initial, _)) => initial.clone(),
            None => self.root.transition.segment().initial_state(),
        }
    }

    /// The state the change goes to.
    pub fn target_state(&self) -> S {
        match &self.overrides {
            Some((_, target)) => target.clone(),
            None => self.root.transition.segment().target_state(),
        }
    }

    /// Whether the change goes from `state` to `target_state` (Compose's infix `S.isTransitioningTo(targetState)` of
    /// the segment).
    pub fn is_transitioning_to(&self, state: S, target_state: S) -> bool {
        Segment::new(self.initial_state(), self.target_state()).is_transitioning_to(state, target_state)
    }

    /// The container's alignment of the contents (Compose's `contentAlignment`).
    pub fn content_alignment(&self) -> Alignment {
        self.root.content_alignment.borrow().clone()
    }

    /// `content_transform` with the container's size animating by `size_transform` (none: not animating; Compose's
    /// infix `ContentTransform.using(sizeTransform)`).
    pub fn using(
        &self,
        content_transform: ContentTransform,
        size_transform: Option<Rc<dyn SizeTransform>>,
    ) -> ContentTransform {
        *content_transform.inner.size_transform.borrow_mut() = size_transform;
        content_transform
    }

    fn with_states(&self, initial: S, target: S) -> AnimatedContentTransitionScope<S> {
        AnimatedContentTransitionScope { root: self.root.clone(), overrides: Some((initial, target)) }
    }
}

/// What `slide_into_container`'s and `slide_out_of_container`'s `initial_offset`/`target_offset` default to: the
/// whole slide (Compose's `{ it }`).
fn full_slide(it: i32) -> i32 {
    it
}

/// A spring of IntOffset's visibility threshold (Compose's default of the slides into and out of the container).
fn default_container_slide_spec() -> Arc<dyn FiniteAnimationSpec<IntOffset>> {
    let s = crate::spring::<IntOffset>();
    Arc::new(crate::SpringSpec::new(
        s.damping_ratio,
        s.stiffness,
        Some(crate::VisibilityThreshold::VisibilityThreshold),
    ))
}

#[named]
impl<S: AnimationValue> AnimatedContentTransitionScope<S> {
    /// The new content sliding into the container toward `towards` (Compose's `slideIntoContainer`): from where it is
    /// just out of the container (the slide's whole length, which `initial_offset` may change), as the container
    /// is now (during a size animation, its size then) and its alignment place it.
    ///
    /// Kotlin: `slideIntoContainer(towards: AnimatedContentTransitionScope.SlideDirection, animationSpec: FiniteAnimationSpec<IntOffset> = spring(visibilityThreshold = IntOffset.VisibilityThreshold), initialOffset: (offsetForFullSlide: Int) -> Int = { it })`
    pub fn slide_into_container(
        &self,
        towards: SlideDirection,
        #[default(default_container_slide_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<IntOffset>>,
        #[default(full_slide)] initial_offset: impl Fn(i32) -> i32 + 'static,
    ) -> EnterTransition {
        let root = self.root.clone();
        if root.is_left(towards) {
            slide_in_of(
                animation_spec,
                Rc::new(move |full: IntSize| {
                    let it = full.width;
                    let current = root.current_size();
                    IntOffset::new(
                        initial_offset(current.width - root.calculate_offset(IntSize::new(it, it), current).x),
                        0,
                    )
                }),
            )
        } else if root.is_right(towards) {
            slide_in_of(
                animation_spec,
                Rc::new(move |full: IntSize| {
                    let it = full.width;
                    let current = root.current_size();
                    IntOffset::new(initial_offset(-root.calculate_offset(IntSize::new(it, it), current).x - it), 0)
                }),
            )
        } else if towards == SlideDirection::Up {
            slide_in_of(
                animation_spec,
                Rc::new(move |full: IntSize| {
                    let it = full.height;
                    let current = root.current_size();
                    IntOffset::new(
                        0,
                        initial_offset(current.height - root.calculate_offset(IntSize::new(it, it), current).y),
                    )
                }),
            )
        } else if towards == SlideDirection::Down {
            slide_in_of(
                animation_spec,
                Rc::new(move |full: IntSize| {
                    let it = full.height;
                    let current = root.current_size();
                    IntOffset::new(0, initial_offset(-root.calculate_offset(IntSize::new(it, it), current).y - it))
                }),
            )
        } else {
            EnterTransition::None
        }
    }

    /// The old content sliding out of the container toward `towards` (Compose's `slideOutOfContainer`): to where it
    /// is just out of it (`target_offset` may change that), as the new content's size and the alignment place it.
    ///
    /// Kotlin: `slideOutOfContainer(towards: AnimatedContentTransitionScope.SlideDirection, animationSpec: FiniteAnimationSpec<IntOffset> = spring(visibilityThreshold = IntOffset.VisibilityThreshold), targetOffset: (offsetForFullSlide: Int) -> Int = { it })`
    pub fn slide_out_of_container(
        &self,
        towards: SlideDirection,
        #[default(default_container_slide_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<IntOffset>>,
        #[default(full_slide)] target_offset: impl Fn(i32) -> i32 + 'static,
    ) -> ExitTransition {
        let root = self.root.clone();
        // (the target size may be 0, an empty composable's)
        let target_size = move |root: &AnimatedContentTransitionScopeImpl<S>| {
            root.target_size_of(&root.transition.target_state()).unwrap_or(IntSize::Zero)
        };
        if root.is_left(towards) {
            slide_out_of(
                animation_spec,
                Rc::new(move |full: IntSize| {
                    let it = full.width;
                    let t = target_size(&root);
                    IntOffset::new(target_offset(-root.calculate_offset(IntSize::new(it, it), t).x - it), 0)
                }),
            )
        } else if root.is_right(towards) {
            slide_out_of(
                animation_spec,
                Rc::new(move |full: IntSize| {
                    let it = full.width;
                    let t = target_size(&root);
                    IntOffset::new(target_offset(-root.calculate_offset(IntSize::new(it, it), t).x + t.width), 0)
                }),
            )
        } else if towards == SlideDirection::Up {
            slide_out_of(
                animation_spec,
                Rc::new(move |full: IntSize| {
                    let it = full.height;
                    let t = target_size(&root);
                    IntOffset::new(0, target_offset(-root.calculate_offset(IntSize::new(it, it), t).y - it))
                }),
            )
        } else if towards == SlideDirection::Down {
            slide_out_of(
                animation_spec,
                Rc::new(move |full: IntSize| {
                    let it = full.height;
                    let t = target_size(&root);
                    IntOffset::new(0, target_offset(-root.calculate_offset(IntSize::new(it, it), t).y + t.height))
                }),
            )
        } else {
            ExitTransition::None
        }
    }
}

// ---- AnimatedContentScope ------------------------------------------------------------------------------------------

/// What an AnimatedContent's content is given (Compose's AnimatedContentScope): its [`AnimatedVisibilityScope`] (it
/// derefs to it: its transition, `animate_enter_exit`). Clones are the same scope.
#[derive(Clone, PartialEq)]
pub struct AnimatedContentScope {
    scope: AnimatedVisibilityScope,
}

impl std::ops::Deref for AnimatedContentScope {
    type Target = AnimatedVisibilityScope;
    fn deref(&self) -> &AnimatedVisibilityScope {
        &self.scope
    }
}

impl From<AnimatedContentScope> for AnimatedVisibilityScope {
    fn from(scope: AnimatedContentScope) -> AnimatedVisibilityScope {
        scope.scope
    }
}

impl AnimatedContentScope {
    /// It as the [`AnimatedVisibilityScope`] it is.
    pub fn animated_visibility_scope(&self) -> &AnimatedVisibilityScope {
        &self.scope
    }
}

// ---- The composables -----------------------------------------------------------------------------------------------

/// What an AnimatedContent's change does unless told otherwise (Compose's default `transitionSpec`): the new content
/// fades in and scales up from 0.92 over 220 ms after 90, the old fades out over 90.
pub fn default_content_transform<S>(_: &AnimatedContentTransitionScope<S>) -> ContentTransform {
    let enter_spec = || -> Arc<dyn FiniteAnimationSpec<f32>> {
        Arc::new(kompoz_runtime::named_args!(tween(220, delay_millis = 90)))
    };
    let enter = kompoz_runtime::named_args!(fade_in(animation_spec = enter_spec()))
        + kompoz_runtime::named_args!(scale_in(animation_spec = enter_spec(), initial_scale = 0.92));
    let exit_spec: Arc<dyn FiniteAnimationSpec<f32>> = Arc::new(kompoz_runtime::named_args!(tween(90)));
    enter.together_with(kompoz_runtime::named_args!(fade_out(animation_spec = exit_spec)))
}

/// A content's key: what tells contents apart (Compose's `contentKey: (targetState: S) -> Any?`); none: the state
/// itself (Kotlin's default `{ it }`).
pub type ContentKey<S> = Option<Rc<dyn Fn(&S) -> SlotKey>>;

/// Shows `content` of `target_state`, the old content going as the new comes in when it changes (Compose's
/// `AnimatedContent`): `transition_spec` picks what each change does from its states (by default the new fades and
/// scales in, the old fades out, the container's size animating), `content_alignment` places the contents in the
/// container, `content_key` tells contents apart (by default the states themselves: content of an equal key is
/// the same content, given the new state, not a change), `label` names its transition for tools. Each content gets
/// its [`AnimatedContentScope`] and its state. What leaves takes no input on its way out (KompozR's, as C++ Kompoz,
/// unlike Compose 1.12).
///
/// Kotlin: `AnimatedContent(targetState: S, modifier: Modifier = Modifier, transitionSpec: AnimatedContentTransitionScope<S>.() -> ContentTransform = { (fadeIn(animationSpec = tween(220, delayMillis = 90)) + scaleIn(initialScale = 0.92f, animationSpec = tween(220, delayMillis = 90))).togetherWith(fadeOut(animationSpec = tween(90))) }, contentAlignment: Alignment = Alignment.TopStart, label: String = "AnimatedContent", contentKey: (targetState: S) -> Any? = { it }, content: @Composable AnimatedContentScope.(targetState: S) -> Unit)`
#[kompozable]
pub fn AnimatedContent<S: AnimationValue>(
    target_state: S,
    #[default(Modifier)] modifier: Modifier,
    #[default(default_content_transform)] transition_spec: impl Fn(&AnimatedContentTransitionScope<S>) -> ContentTransform
    + 'static,
    #[default(Alignment::TopStart)] content_alignment: Alignment,
    #[default("AnimatedContent".to_string())] label: String,
    #[default(None)] content_key: Option<Rc<dyn Fn(&S) -> SlotKey>>,
    content: impl Fn(&AnimatedContentScope, S) + 'static,
) {
    let transition = update_transition(target_state, Some(label));
    animated_content_impl(
        &transition,
        modifier,
        Rc::new(transition_spec),
        content_alignment,
        content_key,
        Rc::new(|_: &AnimatedContentTransitionScope<S>| None),
        Rc::new(content),
    );
}

#[named]
impl<S: AnimationValue> Transition<S> {
    /// [`AnimatedContent`] of this transition's states (Compose's `Transition<S>.AnimatedContent`): the contents come
    /// and go along it (a [`SeekableTransitionState`](crate::SeekableTransitionState)'s seeks them). Composable.
    ///
    /// Kotlin: `AnimatedContent(modifier: Modifier = Modifier, transitionSpec: AnimatedContentTransitionScope<S>.() -> ContentTransform = { (fadeIn(animationSpec = tween(220, delayMillis = 90)) + scaleIn(initialScale = 0.92f, animationSpec = tween(220, delayMillis = 90))).togetherWith(fadeOut(animationSpec = tween(90))) }, contentAlignment: Alignment = Alignment.TopStart, contentKey: (targetState: S) -> Any? = { it }, content: @Composable AnimatedContentScope.(targetState: S) -> Unit)`
    #[track_caller]
    pub fn AnimatedContent(
        &self,
        #[default(Modifier)] modifier: Modifier,
        #[default(default_content_transform)] transition_spec: impl Fn(
            &AnimatedContentTransitionScope<S>,
        ) -> ContentTransform
        + 'static,
        #[default(Alignment::TopStart)] content_alignment: Alignment,
        #[default(None)] content_key: Option<Rc<dyn Fn(&S) -> SlotKey>>,
        content: impl Fn(&AnimatedContentScope, S) + 'static,
    ) {
        kompoz_runtime::kompozable_block!({
            TransitionAnimatedContent(
                self.clone(),
                modifier,
                transition_spec,
                content_alignment,
                content_key,
                |_: &AnimatedContentTransitionScope<S>| None,
                content,
            );
        });
    }
}

#[named]
impl<S: AnimationValue> DeferredTransition<S> {
    /// [`Transition::AnimatedContent`] of a deferred transition (Compose's experimental
    /// `DeferredTransition<S>.DeferredAnimatedContent`): while its state defers, `mutable_transform_spec`'s
    /// [`MutableContentTransform`] (of the change pending) moves the two contents by hand; the transition takes over
    /// from there. Composable.
    ///
    /// Kotlin: `DeferredAnimatedContent(modifier: Modifier = Modifier, transitionSpec: AnimatedContentTransitionScope<S>.() -> ContentTransform = { (fadeIn(animationSpec = tween(220, delayMillis = 90)) + scaleIn(initialScale = 0.92f, animationSpec = tween(220, delayMillis = 90))).togetherWith(fadeOut(animationSpec = tween(90))) }, contentAlignment: Alignment = Alignment.TopStart, contentKey: (targetState: S) -> Any? = { it }, mutableTransformSpec: AnimatedContentTransitionScope<S>.() -> MutableContentTransform? = { null }, content: @Composable AnimatedContentScope.(targetState: S) -> Unit)`
    #[track_caller]
    pub fn DeferredAnimatedContent(
        &self,
        #[default(Modifier)] modifier: Modifier,
        #[default(default_content_transform)] transition_spec: impl Fn(
            &AnimatedContentTransitionScope<S>,
        ) -> ContentTransform
        + 'static,
        #[default(Alignment::TopStart)] content_alignment: Alignment,
        #[default(None)] content_key: Option<Rc<dyn Fn(&S) -> SlotKey>>,
        #[default(|_: &AnimatedContentTransitionScope<S>| None)] mutable_transform_spec: impl Fn(
            &AnimatedContentTransitionScope<S>,
        ) -> Option<
            MutableContentTransform,
        > + 'static,
        content: impl Fn(&AnimatedContentScope, S) + 'static,
    ) {
        kompoz_runtime::kompozable_block!({
            TransitionAnimatedContent(
                self.transition().clone(),
                modifier,
                transition_spec,
                content_alignment,
                content_key,
                mutable_transform_spec,
                content,
            );
        });
    }
}

/// The restartable body of `Transition::AnimatedContent` and `DeferredTransition::DeferredAnimatedContent` (a scope
/// of its own, as Compose's composable extension functions are).
#[kompozable]
fn TransitionAnimatedContent<S: AnimationValue>(
    transition: Transition<S>,
    modifier: Modifier,
    transition_spec: impl Fn(&AnimatedContentTransitionScope<S>) -> ContentTransform + 'static,
    content_alignment: Alignment,
    content_key: ContentKey<S>,
    mutable_transform_spec: impl Fn(&AnimatedContentTransitionScope<S>) -> Option<MutableContentTransform> + 'static,
    content: impl Fn(&AnimatedContentScope, S) + 'static,
) {
    animated_content_impl(
        &transition,
        modifier,
        Rc::new(transition_spec),
        content_alignment,
        content_key,
        Rc::new(mutable_transform_spec),
        Rc::new(content),
    );
}

// ---- The implementation -------------------------------------------------------------------------------------------

type TransitionSpec<S> = Rc<dyn Fn(&AnimatedContentTransitionScope<S>) -> ContentTransform>;
type MutableTransformSpec<S> = Rc<dyn Fn(&AnimatedContentTransitionScope<S>) -> Option<MutableContentTransform>>;
type ContentFn<S> = Rc<dyn Fn(&AnimatedContentScope, S)>;

/// The keys of the contents (Compose's `contentKey(it)`): the given function's, or, for the default (`{ it }`, the
/// state itself, which needn't hash here), an id given to each state as it is first seen — equal states, equal ids.
pub(crate) struct ContentKeys<S> {
    key: ContentKey<S>,
    ids: Vec<(S, u64)>,
    next: u64,
}

impl<S: AnimationValue> ContentKeys<S> {
    pub(crate) fn new() -> ContentKeys<S> {
        ContentKeys { key: None, ids: Vec::new(), next: 0 }
    }

    pub(crate) fn key_of(&mut self, state: &S) -> SlotKey {
        if let Some(key) = &self.key {
            return key(state);
        }
        if let Some((_, id)) = self.ids.iter().find(|(s, _)| s == state) {
            return SlotKey::new(*id);
        }
        let id = self.next;
        self.next += 1;
        self.ids.push((state.clone(), id));
        SlotKey::new(id)
    }

    /// Forgets the states not in `keep` (their ids are of no use: their content left).
    pub(crate) fn retain(&mut self, keep: impl Fn(&S) -> bool) {
        self.ids.retain(|(s, _)| keep(s));
    }

    pub(crate) fn set_key(&mut self, key: ContentKey<S>) {
        self.key = key;
    }
}

/// The index of the first state whose key is `key` (Kotlin's `indexOfFirst { contentKey(it) == … }`).
fn index_of_key<S: AnimationValue>(list: &[S], keys: &RefCell<ContentKeys<S>>, key: &SlotKey) -> Option<usize> {
    list.iter().position(|s| keys.borrow_mut().key_of(s) == *key)
}

/// Compose's internal `Transition<S>.AnimatedContentImpl`.
#[track_caller]
fn animated_content_impl<S: AnimationValue>(
    transition: &Transition<S>,
    modifier: Modifier,
    transition_spec: TransitionSpec<S>,
    content_alignment: Alignment,
    content_key: ContentKey<S>,
    mutable_transform_spec: MutableTransformSpec<S>,
    content: ContentFn<S>,
) {
    let this = transition.clone();
    group(move || {
        let layout_direction = LocalLayoutDirection.current();
        let (t, a) = (this.clone(), content_alignment.clone());
        let root = remember_keyed(this.clone(), move || {
            Rc::new(AnimatedContentTransitionScopeImpl::new(t, a, layout_direction))
        });
        let root_scope = AnimatedContentTransitionScope { root: root.clone(), overrides: None };
        let current = this.current_state();
        let c = current.clone();
        let currently_visible: SnapshotStateList<S> = remember_keyed(this.clone(), move || mutable_state_list_of([c]));
        let pending = this.pending_target_state();
        let content_map: Rc<RefCell<Vec<S>>> =
            remember_keyed((this.clone(), pending.clone()), || Rc::new(RefCell::new(Vec::new())));
        let keys = remember_keyed(this.clone(), || Rc::new(RefCell::new(ContentKeys::<S>::new())));
        keys.borrow_mut().set_key(content_key);
        // (the latest of what the caller gives: what the contents' closures call)
        let latest = remember(|| Rc::new(RefCell::new(None::<(TransitionSpec<S>, ContentFn<S>)>)));
        *latest.borrow_mut() = Some((transition_spec.clone(), content.clone()));

        // (tools may change the current state directly: then only it and the target are shown)
        if !currently_visible.with(|l| l.contains(&current)) {
            currently_visible.clear();
            currently_visible.add(current.clone());
        }
        let target = this.target_state();
        if current == target && pending.is_none() {
            if currently_visible.len() != 1 || currently_visible.get(0).as_ref() != Some(&current) {
                currently_visible.clear();
                currently_visible.add(current.clone());
            }
            let mut map = content_map.borrow_mut();
            if map.len() != 1 || map.contains(&current) {
                map.clear();
            }
            *root.content_alignment.borrow_mut() = content_alignment.clone();
            root.layout_direction.set(layout_direction);
        }
        if let Some(p) = &pending
            && *p != current
        {
            // (one of the same key is replaced)
            let key = keys.borrow_mut().key_of(p);
            let list = currently_visible.to_vec();
            match index_of_key(&list, &keys, &key) {
                None => currently_visible.add(p.clone()),
                Some(id) if list[id] != *p => currently_visible.set(id, p.clone()),
                Some(_) => {}
            }
        }
        // (the target at the end, placed last: on top unless its z-index says otherwise)
        if current != target {
            let key = keys.borrow_mut().key_of(&target);
            let list = currently_visible.to_vec();
            match index_of_key(&list, &keys, &key) {
                None => currently_visible.add(target.clone()),
                Some(id) if list[id] != target || id != list.len() - 1 => {
                    currently_visible.update(|l| {
                        l.remove(id);
                        l.push(target.clone());
                    });
                }
                Some(_) => {}
            }
        }
        {
            let list = currently_visible.to_vec();
            let (c, t, p) = (current.clone(), target.clone(), pending.clone());
            keys.borrow_mut().retain(|s| list.contains(s) || *s == c || *s == t || p.as_ref() == Some(s));
        }
        let pending_scope = {
            let rs = root_scope.clone();
            let t = target.clone();
            remember_keyed(pending.clone(), move || pending.clone().map(|p| rs.with_states(t, p)))
        };
        let mutable_content_transform_data = {
            let (ps, spec) = (pending_scope.clone(), mutable_transform_spec.clone());
            remember_keyed(pending_scope.clone(), move || ps.as_ref().and_then(|ps| spec(ps)))
        };
        let pending = this.pending_target_state();
        {
            let mut map = content_map.borrow_mut();
            if !map.contains(&target) || !map.contains(&current) || pending.as_ref().is_some_and(|p| !map.contains(p)) {
                *map = currently_visible.to_vec();
            }
        }
        let segment = this.segment();
        let ts = transition_spec.clone();
        let rs = root_scope.clone();
        let content_transform = remember_keyed((root_scope.clone(), segment.clone(), pending.clone()), move || ts(&rs));
        let size_modifier = create_size_animation_modifier(&root, &content_transform);
        let policy = remember(|| {
            Rc::new(AnimatedContentMeasurePolicy { root: root.clone() as Rc<dyn ContentAlignmentOf> })
                as Rc<dyn MeasurePolicy>
        });
        let entry = ContentEntry {
            transition: this.clone(),
            root: root.clone(),
            root_scope,
            pending_scope,
            mutable_content_transform_data,
            currently_visible: currently_visible.clone(),
            latest,
        };
        layout_into(modifier.then(size_modifier), policy, false, move || {
            for state in currently_visible.to_vec() {
                let key = keys.borrow_mut().key_of(&state);
                kompoz_runtime::key(&key, || {
                    if content_map.borrow().contains(&state) {
                        entry.compose(state.clone());
                    }
                });
            }
        });
    });
}

/// What each content is composed with (Compose's `contentMap`'s lambdas share these).
struct ContentEntry<S> {
    transition: Transition<S>,
    root: Rc<AnimatedContentTransitionScopeImpl<S>>,
    root_scope: AnimatedContentTransitionScope<S>,
    pending_scope: Option<AnimatedContentTransitionScope<S>>,
    mutable_content_transform_data: Option<MutableContentTransform>,
    currently_visible: SnapshotStateList<S>,
    latest: Rc<RefCell<Option<(TransitionSpec<S>, ContentFn<S>)>>>,
}

impl<S: AnimationValue> ContentEntry<S> {
    /// The content of `state_for_content` (a `contentMap` lambda of Compose's).
    #[track_caller]
    fn compose(&self, state_for_content: S) {
        group(|| {
            let (transition_spec, content) = self.latest.borrow().clone().expect("set as composed");
            let this = &self.transition;
            let pending_target_state = this.pending_target_state();
            let segment = this.segment();
            let target_state = this.target_state();
            let current_state = this.current_state();
            let is_pending = pending_target_state.as_ref() == Some(&state_for_content);
            let spec_on_enter = {
                let (ps, rs, spec, seg, s) = (
                    self.pending_scope.clone(),
                    self.root_scope.clone(),
                    transition_spec.clone(),
                    segment.clone(),
                    state_for_content.clone(),
                );
                remember_keyed(is_pending, move || {
                    if is_pending && let Some(ps) = &ps {
                        spec(ps)
                    } else if s != seg.initial_state() && s != seg.target_state() {
                        spec(&rs.with_states(seg.initial_state(), s))
                    } else {
                        spec(&rs)
                    }
                })
            };
            let exit = {
                let has_pending_scope = self.pending_scope.is_some();
                let (rs, spec, seg, s) =
                    (self.root_scope.clone(), transition_spec.clone(), segment.clone(), state_for_content.clone());
                remember_keyed((segment.target_state() == state_for_content, is_pending), move || {
                    if seg.target_state() == s || (is_pending && has_pending_scope) {
                        ExitTransition::None
                    } else if s != seg.initial_state() && s != seg.target_state() {
                        spec(&rs.with_states(s, seg.initial_state())).initial_content_exit()
                    } else {
                        spec(&rs).initial_content_exit()
                    }
                })
            };
            let is_target = state_for_content == target_state;
            let child_data = remember(|| Rc::new(ChildData::new(is_target)));
            child_data.is_target.set(is_target);
            child_data.is_pending_target.set(is_pending && !is_target && state_for_content != current_state);
            let hold = exit.data.hold;
            let mutable_transform = self.mutable_content_transform_data.as_ref().and_then(|t| {
                if is_pending {
                    Some(t.target_transform.clone())
                } else if is_target {
                    Some(t.initial_transform.clone())
                } else {
                    None
                }
            });
            let s = state_for_content.clone();
            let modifier = Modifier
                .then_element(ZIndexModifierElement {
                    z_index: spec_on_enter.target_content_z_index(),
                    state_for_content: Rc::new(state_for_content.clone()),
                    eq: |a, b| a.downcast_ref::<S>() == b.downcast_ref::<S>(),
                })
                .then_element(ChildDataElement { data: child_data });
            let (cv, root) = (self.currently_visible.clone(), self.root.clone());
            animated_enter_exit_impl(
                this,
                Rc::new(move |it: &S| *it == s),
                modifier,
                spec_on_enter.target_content_enter(),
                exit,
                Rc::new(move |current, target| {
                    current == EnterExitState::PostExit && target == EnterExitState::PostExit && !hold
                }),
                None,
                mutable_transform,
                move |scope: &AnimatedVisibilityScope| {
                    let (cv, root2, s) = (cv.clone(), root.clone(), state_for_content.clone());
                    let keys = (scope.clone(), state_for_content.clone());
                    kompoz_runtime::kompozable_block!({
                        DisposableEffect(keys, move || {
                            on_dispose(move || {
                                cv.retain(|v| *v != s);
                                root2.target_size_map.borrow_mut().retain(|(v, _)| *v != s);
                            })
                        });
                    });
                    {
                        let mut map = root.target_size_map.borrow_mut();
                        let size = scope.inner.target_size.clone();
                        match map.iter_mut().find(|(v, _)| *v == state_for_content) {
                            Some(slot) => slot.1 = size,
                            None => map.push((state_for_content.clone(), size)),
                        }
                    }
                    let sc = scope.clone();
                    let content_scope = remember(move || AnimatedContentScope { scope: sc });
                    content(&content_scope, state_for_content.clone());
                },
            );
        });
    }
}

/// What a content tells the AnimatedContent's layout (Compose's ChildData, a parent data): whether it is the target's
/// (measured first: it makes the size) or a pending target's (not counted in the size). States: a change measures
/// again.
pub(crate) struct ChildData {
    is_target: MutableState<bool>,
    is_pending_target: MutableState<bool>,
}

impl ChildData {
    fn new(is_target: bool) -> ChildData {
        ChildData { is_target: mutable_state_of(is_target), is_pending_target: mutable_state_of(false) }
    }
}

fn child_data_of(measurable: &dyn Measurable) -> Option<Rc<ChildData>> {
    measurable.parent_data()?.downcast_ref::<Rc<ChildData>>().cloned()
}

struct ChildDataElement {
    data: Rc<ChildData>,
}

impl PartialEq for ChildDataElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.data, &other.data)
    }
}

impl ModifierNodeElement for ChildDataElement {
    type Node = ChildDataNode;
    fn create(&self) -> ChildDataNode {
        ChildDataNode { data: self.data.clone() }
    }
    fn update(&self, node: &mut ChildDataNode) {
        node.data = self.data.clone();
    }
    fn name(&self) -> &'static str {
        "ChildData"
    }
}

struct ChildDataNode {
    data: Rc<ChildData>,
}

impl ModifierNode for ChildDataNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for ChildDataNode {
    fn modify_parent_data(&mut self, _: &kompoz_graphics::Density, _: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        Some(Box::new(self.data.clone()))
    }
}

/// Compose's private ZIndexModifierElement: the content placed at its z-index.
struct ZIndexModifierElement {
    z_index: f32,
    state_for_content: Rc<dyn Any>,
    eq: fn(&dyn Any, &dyn Any) -> bool,
}

impl PartialEq for ZIndexModifierElement {
    fn eq(&self, other: &Self) -> bool {
        self.z_index == other.z_index && (self.eq)(&*self.state_for_content, &*other.state_for_content)
    }
}

impl ModifierNodeElement for ZIndexModifierElement {
    type Node = ZIndexModifierNode;
    fn create(&self) -> ZIndexModifierNode {
        ZIndexModifierNode { z_index: self.z_index }
    }
    fn update(&self, node: &mut ZIndexModifierNode) {
        node.z_index = self.z_index;
    }
    fn name(&self) -> &'static str {
        "targetContentZIndex"
    }
}

struct ZIndexModifierNode {
    z_index: f32,
}

impl ModifierNode for ZIndexModifierNode {
    node_capabilities!(layout);
}

impl LayoutModifierNode for ZIndexModifierNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = measurable.measure(constraints);
        let z_index = self.z_index;
        let (w, h) = (placeable.width(), placeable.height());
        scope.layout(w, h, move |p: &PlacementScope| {
            kompoz_runtime::named_args!(p.place(&placeable, 0, 0, z_index = z_index))
        })
    }
}

/// The alignment of an AnimatedContent's contents, whatever its state's type.
trait ContentAlignmentOf {
    fn content_alignment(&self) -> Alignment;
    fn set_measured_size(&self, size: IntSize);
}

impl<S: AnimationValue> ContentAlignmentOf for AnimatedContentTransitionScopeImpl<S> {
    fn content_alignment(&self) -> Alignment {
        self.content_alignment.borrow().clone()
    }
    fn set_measured_size(&self, size: IntSize) {
        self.measured_size.set(size);
    }
}

/// Compose's private AnimatedContentMeasurePolicy: the target's content measured first (it makes the size in the
/// lookahead pass), the others after; in the main pass as big as the contents shown (a pending target's left out);
/// each placed by the alignment.
struct AnimatedContentMeasurePolicy {
    root: Rc<dyn ContentAlignmentOf>,
}

impl MeasurePolicy for AnimatedContentMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        let alignment = self.root.content_alignment();
        let datas: Vec<Option<Rc<ChildData>>> = measurables.iter().map(|m| child_data_of(*m)).collect();
        let mut placeables: Vec<Option<Placeable>> = vec![None; measurables.len()];
        let mut target_size = IntSize::Zero;
        // (the target first, placed on top unless its z-index says otherwise)
        for ((measurable, data), slot) in measurables.iter().zip(&datas).zip(placeables.iter_mut()) {
            if data.as_ref().is_some_and(|d| d.is_target.get()) {
                let p = measurable.measure(constraints);
                target_size = IntSize::new(p.width(), p.height());
                *slot = Some(p);
            }
        }
        // (the others after: they don't make the size of a size animation)
        for (measurable, slot) in measurables.iter().zip(placeables.iter_mut()) {
            if slot.is_none() {
                *slot = Some(measurable.measure(constraints));
            }
        }
        let placeables: Vec<Placeable> = placeables.into_iter().map(|p| p.expect("measured")).collect();
        let (max_width, max_height) = if scope.is_looking_ahead() {
            (target_size.width, target_size.height)
        } else {
            let mut w = 0;
            let mut h = 0;
            for (p, data) in placeables.iter().zip(&datas) {
                if !data.as_ref().is_some_and(|d| d.is_pending_target.get()) {
                    w = w.max(p.width());
                    h = h.max(p.height());
                }
            }
            (w, h)
        };
        if !scope.is_looking_ahead() {
            // (the measured size, of the approach only)
            self.root.set_measured_size(IntSize::new(max_width, max_height));
        }
        scope.layout(max_width, max_height, move |p: &PlacementScope| {
            for placeable in &placeables {
                let offset = alignment.align(
                    IntSize::new(placeable.width(), placeable.height()),
                    IntSize::new(max_width, max_height),
                    LayoutDirection::Ltr,
                );
                p.place(placeable, offset.x, offset.y);
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

// ---- The container's size -------------------------------------------------------------------------------------------

/// Compose's internal `createSizeAnimationModifier`: while the transition runs with a size transform, the size
/// animates (clipped unless the transform says not), else it is observed.
#[track_caller]
fn create_size_animation_modifier<S: AnimationValue>(
    root: &Rc<AnimatedContentTransitionScopeImpl<S>>,
    content_transform: &ContentTransform,
) -> Modifier {
    let root = root.clone();
    let content_transform = content_transform.clone();
    group(move || {
        let should_animate_size = remember_keyed(root.transition.clone(), || Rc::new(Cell::new(false)));
        let size_transform = content_transform.size_transform();
        let transition = &root.transition;
        if transition.current_state() == transition.target_state() {
            should_animate_size.set(false);
        } else if size_transform.is_some() {
            should_animate_size.set(true);
        }
        let (size_animation, clip) = if should_animate_size.get() {
            let size_animation = group(|| transition.create_deferred_animation(IntSize::VectorConverter));
            let no_clip = size_transform.as_ref().is_some_and(|s| !s.clip());
            let clip =
                remember_keyed(
                    size_animation.clone(),
                    move || {
                        if no_clip { Modifier } else { Modifier.clip_to_bounds() }
                    },
                );
            (Some(size_animation), clip)
        } else {
            *root.animated_size.borrow_mut() = None;
            (None, Modifier)
        };
        clip.then_element(SizeModifierElement { size_animation, size_transform, scope: root.clone() })
    })
}

/// Compose's private SizeModifierElement (its size transform is the node's directly: a new one updates the node,
/// which measures again, as Compose's state read in measure does).
struct SizeModifierElement<S> {
    size_animation: Option<DeferredAnimation<S, IntSize, AnimationVector2D>>,
    size_transform: Option<Rc<dyn SizeTransform>>,
    scope: Rc<AnimatedContentTransitionScopeImpl<S>>,
}

impl<S> PartialEq for SizeModifierElement<S> {
    fn eq(&self, other: &Self) -> bool {
        self.size_animation == other.size_animation
            && match (&self.size_transform, &other.size_transform) {
                (None, None) => true,
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                _ => false,
            }
    }
}

impl<S: AnimationValue> ModifierNodeElement for SizeModifierElement<S> {
    type Node = SizeModifierNode<S>;
    fn create(&self) -> SizeModifierNode<S> {
        SizeModifierNode {
            size_animation: self.size_animation.clone(),
            size_transform: self.size_transform.clone(),
            scope: self.scope.clone(),
            last_size: Rc::new(Cell::new(None)),
        }
    }
    fn update(&self, node: &mut SizeModifierNode<S>) {
        node.size_animation = self.size_animation.clone();
        node.size_transform = self.size_transform.clone();
        node.scope = self.scope.clone();
    }
    fn name(&self) -> &'static str {
        "sizeTransform"
    }
}

/// Compose's private SizeModifierNode: in the lookahead pass the content's size; in the main pass, while animating,
/// the size animates from where it last was to the target content's size; the content placed by the alignment.
struct SizeModifierNode<S> {
    size_animation: Option<DeferredAnimation<S, IntSize, AnimationVector2D>>,
    size_transform: Option<Rc<dyn SizeTransform>>,
    scope: Rc<AnimatedContentTransitionScopeImpl<S>>,
    /// The size it last had (Compose's `lastSize`, UnspecifiedSize as none): an interrupted change goes on from it.
    last_size: Rc<Cell<Option<IntSize>>>,
}

impl<S: AnimationValue> ModifierNode for SizeModifierNode<S> {
    node_capabilities!(layout);

    fn on_reset(&mut self) {
        self.last_size.set(None);
    }

    fn on_attach(&mut self, _: &NodeCx) {}
}

impl<S: AnimationValue> LayoutModifierNode for SizeModifierNode<S> {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = measurable.measure(constraints);
        let current_size = IntSize::new(placeable.width(), placeable.height());
        let measured_size = if scope.is_looking_ahead() {
            current_size
        } else if let Some(size_animation) = &self.size_animation {
            let (root, last, transform) = (self.scope.clone(), self.last_size.clone(), self.size_transform.clone());
            let (root2, last2) = (self.scope.clone(), self.last_size.clone());
            let size = size_animation.animate(
                move |segment| {
                    let root_initial = root.transition.segment().initial_state();
                    let initial = if segment.initial_state() == root_initial {
                        last.get().unwrap_or(current_size)
                    } else {
                        root.target_size_of(&segment.initial_state()).unwrap_or(IntSize::Zero)
                    };
                    let target = root.target_size_of(&segment.target_state()).unwrap_or(IntSize::Zero);
                    match &transform {
                        Some(t) => t.create_animation_spec(initial, target),
                        None => Arc::new(medium_low_spring::<IntSize>()),
                    }
                },
                // (from the approach's size to the lookahead's)
                move |state| {
                    if *state == root2.transition.segment().initial_state() {
                        last2.get().unwrap_or(current_size)
                    } else {
                        root2.target_size_of(state).unwrap_or(IntSize::Zero)
                    }
                },
            );
            *self.scope.animated_size.borrow_mut() = Some(size.clone());
            let value = size.value();
            self.last_size.set(Some(value));
            value
        } else {
            // (observing)
            self.last_size.set(Some(current_size));
            current_size
        };
        let alignment = self.scope.content_alignment.borrow().clone();
        scope.layout(measured_size.width, measured_size.height, move |p: &PlacementScope| {
            let offset = alignment.align(
                IntSize::new(placeable.width(), placeable.height()),
                measured_size,
                LayoutDirection::Ltr,
            );
            p.place_at(&placeable, offset, 0.0);
        })
    }

    // (Compose's LayoutModifierNodeWithPassThroughIntrinsics)
    fn min_intrinsic_width(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, height: i32) -> i32 {
        m.min_intrinsic_width(height)
    }
    fn min_intrinsic_height(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        m.min_intrinsic_height(width)
    }
    fn max_intrinsic_width(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, height: i32) -> i32 {
        m.max_intrinsic_width(height)
    }
    fn max_intrinsic_height(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        m.max_intrinsic_height(width)
    }
}
