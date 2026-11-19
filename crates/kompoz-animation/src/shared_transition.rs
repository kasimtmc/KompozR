//! Shared elements, as Compose's `SharedTransitionScope.kt`: [`SharedTransitionLayout`] (and
//! [`SharedTransitionScope`](fn@SharedTransitionScope), which leaves the layout to its content) gives its content a
//! [`SharedTransitionScope`]: a lookahead scope in which `shared_element`/`shared_bounds` of the same key in content
//! going and content coming (AnimatedVisibility, AnimatedContent) find each other and move from the one's bounds to
//! the other's, drawn in the scope's overlay meanwhile; `render_in_shared_transition_scope_overlay` puts other content
//! in that overlay; `skip_to_lookahead_size`/`skip_to_lookahead_position` lay content out where it is going.
//!
//! ```ignore
//! SharedTransitionLayout(Modifier, |shared| {
//!     AnimatedContent(open, content = |scope, open: bool| {
//!         let photo = shared.remember_shared_content_state(SlotKey::new("photo"));
//!         Box(modifier = shared.shared_element(Modifier.size(if open { 200.dp() } else { 40.dp() }), photo, scope));
//!     });
//! });
//! ```

use crate::bounds::BoundsTransform;
use crate::bounds_animation::BoundsAnimation;
use crate::lookahead_debug::{
    IS_LOOKAHEAD_ANIMATION_VISUAL_DEBUGGING_ENABLED, LocalLookaheadAnimationVisualDebugConfig,
    LookaheadAnimationVisualDebugHelper,
};
use crate::render_in_overlay::RenderInTransitionOverlayNodeElement;
use crate::shared_content_node::SharedBoundsNodeElement;
use crate::shared_element::{EntryObserver, SharedElement, SharedElementEntry};
use crate::skip_to_lookahead_size::{ScaleToBounds, SkipToLookaheadSizeElement, content_scale_modifier};
use crate::transition::{DeferredAnimation, MutableTransitionState, Transition, remember_transition};
use crate::{
    AnimatedVisibilityScope, EnterExitState, EnterTransition, ExitTransition, Spring, VectorConverter,
    VisibilityThreshold, fade_in, fade_out, spring,
};
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_graphics::{ContentScale, Density, IntSize, LayoutDirection, Offset, Path, Rect, Shape, Size};
use kompoz_runtime::{
    DisposableEffect, MutableState, State, group, kompozable, kompozable_block, mutable_state_of, named, named_args,
    on_dispose, remember, remember_keyed, remember_observer,
};
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode, DrawScope, GraphicsLayer};
use kompoz_ui::layout::{
    LayoutCoordinates, LayoutModifierNode, Measurable, MeasurePolicy, MeasureResult, MeasureScope, PlacementScope,
    SlotKey, layout_into,
};
use kompoz_ui::modifier::ObserverModifierNode;
use kompoz_ui::node_capabilities;
use kompoz_ui::platform::{LocalPlatform, PlatformHandle};
use kompoz_ui::unit::Velocity;
use kompoz_ui::{
    Alignment, Constraints, LookaheadScope, Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle,
};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};
use std::sync::Arc;

// ---- Observed values ------------------------------------------------------------------------------------------------

/// Whether two values are the same for a state holding them (Compose's structural equality policy: objects by
/// identity).
pub(crate) trait Same {
    fn same(&self, other: &Self) -> bool;
}

impl<T: ?Sized> Same for Rc<T> {
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(self, other)
    }
}

impl<T: Same> Same for Option<T> {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (None, None) => true,
            (Some(a), Some(b)) => a.same(b),
            _ => false,
        }
    }
}

impl<T: ?Sized> Same for Weak<T> {
    fn same(&self, other: &Self) -> bool {
        Weak::ptr_eq(self, other)
    }
}

impl<T: ?Sized> Same for Vec<Rc<T>> {
    fn same(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().zip(other).all(|(a, b)| Rc::ptr_eq(a, b))
    }
}

impl Same for PlaceholderSize {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Same for SharedContentState {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Same for GraphicsLayer {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl<S, T, V: crate::AnimationVector> Same for DeferredAnimation<S, T, V> {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl<T: 'static> Same for State<T> {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// A snapshot state of a value that can't be one here (holding `Rc`s): read through [`get`](Observed::get), its
/// readers told when [`set`](Observed::set) changes it (Compose's `mutableStateOf` of an object).
pub(crate) struct Observed<T> {
    value: RefCell<T>,
    version: MutableState<u64>,
}

impl<T: Clone + Same> Observed<T> {
    pub(crate) fn new(value: T) -> Observed<T> {
        Observed { value: RefCell::new(value), version: mutable_state_of(0) }
    }

    /// Its value (a state read).
    pub(crate) fn get(&self) -> T {
        let _ = self.version.get();
        self.value.borrow().clone()
    }

    /// Its value, not read as a state.
    pub(crate) fn peek(&self) -> T {
        self.value.borrow().clone()
    }

    /// Sets it; its readers are told if it is another value.
    pub(crate) fn set(&self, value: T) {
        if self.value.borrow().same(&value) {
            return;
        }
        *self.value.borrow_mut() = value;
        self.version.set(self.version.peek().wrapping_add(1));
    }
}

// ---- PlaceholderSize, ResizeMode, OverlayClip ----------------------------------------------------------------------

#[derive(Clone)]
enum PlaceholderKind {
    Animated,
    Content,
    Custom(Rc<dyn Fn(IntSize, IntSize) -> IntSize>),
}

/// The size a shared element's layout takes in its parent's layout during the transition (Compose's fun interface
/// `SharedTransitionScope.PlaceholderSize`): [`ContentSize`](PlaceholderSize::ContentSize), its content's size where
/// it is going (its lookahead size: its parent lays it out as at the end at once); or
/// [`AnimatedSize`](PlaceholderSize::AnimatedSize), its size along the animation (its parent follows it); or any of
/// the two by [`PlaceholderSize`](fn@PlaceholderSize)`(|content_size, animated_size| …)`.
///
/// Kotlin: `SharedTransitionScope.PlaceholderSize`
#[derive(Clone)]
pub struct PlaceholderSize {
    kind: PlaceholderKind,
}

impl PlaceholderSize {
    /// The size along the animation (Compose's `PlaceholderSize.AnimatedSize`).
    pub const AnimatedSize: PlaceholderSize = PlaceholderSize { kind: PlaceholderKind::Animated };
    /// The content's lookahead size (Compose's `PlaceholderSize.ContentSize`).
    pub const ContentSize: PlaceholderSize = PlaceholderSize { kind: PlaceholderKind::Content };

    /// The size of the layout, from the content's lookahead size and its animated size (Compose's `calculateSize`).
    pub fn calculate_size(&self, content_size: IntSize, animated_size: IntSize) -> IntSize {
        match &self.kind {
            PlaceholderKind::Animated => animated_size,
            PlaceholderKind::Content => content_size,
            PlaceholderKind::Custom(f) => f(content_size, animated_size),
        }
    }
}

impl PartialEq for PlaceholderSize {
    fn eq(&self, other: &Self) -> bool {
        match (&self.kind, &other.kind) {
            (PlaceholderKind::Animated, PlaceholderKind::Animated) => true,
            (PlaceholderKind::Content, PlaceholderKind::Content) => true,
            (PlaceholderKind::Custom(a), PlaceholderKind::Custom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for PlaceholderSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            PlaceholderKind::Animated => "AnimatedSize",
            PlaceholderKind::Content => "ContentSize",
            PlaceholderKind::Custom(_) => "PlaceholderSize",
        })
    }
}

/// A placeholder size of a closure (Kotlin's SAM constructor `PlaceholderSize { contentSize, animatedSize -> … }`).
///
/// Kotlin: `PlaceholderSize(block: (contentSize: IntSize, animatedSize: IntSize) -> IntSize)`
pub fn PlaceholderSize(block: impl Fn(IntSize, IntSize) -> IntSize + 'static) -> PlaceholderSize {
    PlaceholderSize { kind: PlaceholderKind::Custom(Rc::new(block)) }
}

#[derive(Clone, PartialEq, Debug)]
enum ResizeKind {
    ScaleToBounds(ScaleToBounds),
    RemeasureToBounds,
}

/// How `shared_bounds`' content takes the bounds along the transition (Compose's sealed interface
/// `SharedTransitionScope.ResizeMode`): [`scale_to_bounds`](ResizeMode::scale_to_bounds) — laid out at its size
/// where it is going and scaled into the bounds (the default: text stays laid out as it ends) —, or
/// [`RemeasureToBounds`](ResizeMode::RemeasureToBounds) — measured at the bounds each frame (its children laid out
/// again each time).
///
/// Kotlin: `SharedTransitionScope.ResizeMode`
#[derive(Clone, PartialEq, Debug)]
pub struct ResizeMode {
    kind: ResizeKind,
}

impl ResizeMode {
    /// Measured at the animated bounds each frame (Compose's `ResizeMode.RemeasureToBounds`).
    pub const RemeasureToBounds: ResizeMode = ResizeMode { kind: ResizeKind::RemeasureToBounds };
}

impl ResizeMode {
    /// Laid out at its lookahead size and scaled into the bounds by `content_scale`, placed by `alignment`
    /// (Compose's `ResizeMode.scaleToBounds`; Kotlin's defaults are `ContentScale::FillWidth`,
    /// `Alignment::Center`).
    ///
    /// Kotlin: `scaleToBounds(contentScale: ContentScale = ContentScale.FillWidth, alignment: Alignment = Alignment.Center)`
    pub fn scale_to_bounds(content_scale: ContentScale, alignment: Alignment) -> ResizeMode {
        ResizeMode { kind: ResizeKind::ScaleToBounds(ScaleToBounds { content_scale, alignment }) }
    }
}

/// Where a shared element drawn in the overlay is clipped (Compose's interface `SharedTransitionScope.OverlayClip`):
/// in the overlay it is out of its parents' clips; a clip path in the scope's coordinates gives it one. The scope's
/// [`OverlayClip`](SharedTransitionScope::OverlayClip) makes one of a shape; by default a shared element is clipped
/// as the shared element it is in (its parent shared content state's clip), if any.
pub trait OverlayClip {
    /// The path the shared element of `shared_content_state` is clipped to in the overlay, its `bounds` (in the
    /// scope) where they are now; none for no clip (Compose's `getClipPath`).
    ///
    /// Kotlin: `getClipPath(sharedContentState: SharedContentState, bounds: Rect, layoutDirection: LayoutDirection, density: Density): Path?`
    fn get_clip_path(
        &self,
        shared_content_state: &SharedContentState,
        bounds: Rect,
        layout_direction: LayoutDirection,
        density: Density,
    ) -> Option<Path>;
}

/// A clip of a shape (Compose's private ShapeBasedClip): the shape's outline in the bounds.
struct ShapeBasedClip {
    clip_shape: Arc<dyn Shape>,
}

impl OverlayClip for ShapeBasedClip {
    fn get_clip_path(
        &self,
        _: &SharedContentState,
        bounds: Rect,
        ld: LayoutDirection,
        density: Density,
    ) -> Option<Path> {
        let mut path = Path::new();
        path.add_outline(&self.clip_shape.create_outline(bounds.size(), ld, &density));
        path.translate(bounds.top_left());
        Some(path)
    }
}

/// The parent shared element's clip (Compose's private `ParentClip`).
struct ParentClipImpl;

impl OverlayClip for ParentClipImpl {
    fn get_clip_path(&self, state: &SharedContentState, _: Rect, _: LayoutDirection, _: Density) -> Option<Path> {
        state.parent_shared_content_state().and_then(|p| p.clip_path_in_overlay())
    }
}

thread_local! {
    static PARENT_CLIP: Rc<dyn OverlayClip> = Rc::new(ParentClipImpl);
}

/// The clip a shared element takes in the overlay unless given one (Compose's private `ParentClip`): its parent's.
pub(crate) fn ParentClip() -> Rc<dyn OverlayClip> {
    PARENT_CLIP.with(|c| c.clone())
}

// ---- SharedContentConfig, SharedContentState ------------------------------------------------------------------------

/// How a shared content state takes part (Compose's interface `SharedTransitionScope.SharedContentConfig`): all
/// defaults as a plain configuration has them; implement what to change.
pub trait SharedContentConfig {
    /// Whether the shared element of `state` takes part in the transition now (Compose's member extension property
    /// `SharedContentState.isEnabled`; true by default): a disabled one is as if not shared.
    ///
    /// Kotlin: `isEnabled: Boolean`
    fn is_enabled(&self, state: &SharedContentState) -> bool {
        let _ = state;
        true
    }

    /// Whether it stays enabled while its animation runs, once [`is_enabled`](Self::is_enabled) says no (Compose's
    /// `shouldKeepEnabledForOngoingAnimation`; true by default).
    fn should_keep_enabled_for_ongoing_animation(&self) -> bool {
        true
    }

    /// Whether a deferred transition's manual phase (a predictive back's) transforms it with its content
    /// (Compose's experimental `permitTransformDuringDeferredTransition`; true by default).
    fn permit_transform_during_deferred_transition(&self) -> bool {
        true
    }

    /// Where the shared element of `state` goes once the one it was going to was removed during the transition
    /// (Compose's member extension `SharedContentState.alternativeTargetBoundsInTransitionScopeAfterRemoval`):
    /// `target_bounds_before_removal` where that one was, in the scope of `shared_transition_layout_size`; none (the
    /// default) ends its animation where it is.
    ///
    /// Kotlin: `alternativeTargetBoundsInTransitionScopeAfterRemoval(targetBoundsBeforeRemoval: Rect, sharedTransitionLayoutSize: Size): Rect?`
    fn alternative_target_bounds_in_transition_scope_after_removal(
        &self,
        state: &SharedContentState,
        target_bounds_before_removal: Rect,
        shared_transition_layout_size: Size,
    ) -> Option<Rect> {
        let _ = (state, target_bounds_before_removal, shared_transition_layout_size);
        None
    }
}

/// The scope's default configuration (Compose's private object CachedSharedContentConfig).
struct CachedSharedContentConfig;
impl SharedContentConfig for CachedSharedContentConfig {}

/// The defaults' configuration (Compose's `SharedTransitionDefaults.SharedContentConfig` object).
struct DefaultSharedContentConfig;
impl SharedContentConfig for DefaultSharedContentConfig {}

/// A configuration not transformed during a deferred transition (Compose's anonymous object of
/// `SharedContentConfig(false)`).
struct NotTransformedDuringDeferredTransition;
impl SharedContentConfig for NotTransformedDuringDeferredTransition {
    fn permit_transform_during_deferred_transition(&self) -> bool {
        false
    }
}

thread_local! {
    static CACHED_CONFIG: Rc<dyn SharedContentConfig> = Rc::new(CachedSharedContentConfig);
    static DEFAULT_CONFIG: Rc<dyn SharedContentConfig> = Rc::new(DefaultSharedContentConfig);
    static DEFAULT_BOUNDS_TRANSFORM: Rc<dyn BoundsTransform> = Rc::new(|_: Rect, _: Rect| {
        named_args!(spring(
            stiffness = Spring::StiffnessMediumLow,
            visibility_threshold = Some(Rect::VisibilityThreshold)
        ))
    });
}

/// The defaults of shared elements (Compose's object SharedTransitionDefaults).
pub struct SharedTransitionDefaults;

impl SharedTransitionDefaults {
    /// A spring medium-low stiff, there within a pixel (Compose's `SharedTransitionDefaults.BoundsTransform`); one
    /// instance.
    pub fn BoundsTransform() -> Rc<dyn BoundsTransform> {
        DEFAULT_BOUNDS_TRANSFORM.with(|t| t.clone())
    }

    /// A configuration of all the defaults (Compose's object `SharedTransitionDefaults.SharedContentConfig`); one
    /// instance.
    pub fn SharedContentConfig() -> Rc<dyn SharedContentConfig> {
        DEFAULT_CONFIG.with(|c| c.clone())
    }
}

struct SharedContentStateInner {
    key: SlotKey,
    /// Compose's `config`, a state.
    config: Observed<Rc<dyn SharedContentConfig>>,
    /// The entry of its `shared_element`/`shared_bounds` (Compose's `internalState`, a state): weak here — the
    /// entry belongs to the composition, which drops it once its call leaves (Compose's keeps the last one).
    internal_state: Observed<Option<Weak<SharedElementEntry>>>,
}

/// A shared element's state, of its key (Compose's `SharedTransitionScope.SharedContentState`, made by
/// [`remember_shared_content_state`](SharedTransitionScope::remember_shared_content_state)): given to
/// `shared_element`/`shared_bounds`, it says whether a match is found and where its content is clipped in the
/// overlay. Clones are the same state.
#[derive(Clone)]
pub struct SharedContentState {
    inner: Rc<SharedContentStateInner>,
}

impl PartialEq for SharedContentState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for SharedContentState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SharedContentState({:?})", self.inner.key)
    }
}

impl SharedContentState {
    pub(crate) fn new(key: SlotKey, config: Rc<dyn SharedContentConfig>) -> SharedContentState {
        SharedContentState {
            inner: Rc::new(SharedContentStateInner {
                key,
                config: Observed::new(config),
                internal_state: Observed::new(None),
            }),
        }
    }

    /// Its key: the shared elements of one key in a scope match (Compose's `key`).
    pub fn key(&self) -> SlotKey {
        self.inner.key.clone()
    }

    /// Whether another shared element of its key is found for its transition (Compose's `isMatchFound`): known as
    /// soon as it can be, a deferred match too, so animations set up in composition can read it.
    pub fn is_match_found(&self) -> bool {
        self.internal_state().is_some_and(|e| e.shared_element().found_match())
    }

    /// Starts its transition with a velocity (a fling's), which dies down along the bounds animation's spring
    /// (Compose's `prepareTransitionWithInitialVelocity`): call it just before the change that starts it. Nothing if
    /// it isn't enabled.
    pub fn prepare_transition_with_initial_velocity(&self, initial_velocity: Velocity) {
        let entry = self.non_null_internal_state();
        if !self.config().is_enabled(self) {
            return;
        }
        entry.shared_element().update_exit_velocity(initial_velocity);
    }

    /// The path its content is clipped to in the overlay (in the scope), while it is drawn there (Compose's
    /// `clipPathInOverlay`): what a nested shared element is clipped to by default.
    pub fn clip_path_in_overlay(&self) -> Option<Path> {
        self.non_null_internal_state().clip_path_in_overlay.borrow().clone()
    }

    /// The state of the shared element its own is in, if any (Compose's `parentSharedContentState`).
    pub fn parent_shared_content_state(&self) -> Option<SharedContentState> {
        self.non_null_internal_state().parent_state().map(|p| p.user_state())
    }

    /// Whether its bounds animation was set up (Compose's internal `isAnimating`).
    pub(crate) fn is_animating(&self) -> bool {
        self.internal_state().is_some_and(|e| e.bounds_animation().has_animation_state())
    }

    pub(crate) fn config(&self) -> Rc<dyn SharedContentConfig> {
        self.inner.config.get()
    }

    pub(crate) fn set_config(&self, config: Rc<dyn SharedContentConfig>) {
        self.inner.config.set(config);
    }

    /// Whether its configuration enables it (Compose's internal `isEnabledByUser`).
    pub(crate) fn is_enabled_by_user(&self) -> bool {
        let config = self.config();
        config.is_enabled(self) || (self.is_animating() && config.should_keep_enabled_for_ongoing_animation())
    }

    pub(crate) fn internal_state(&self) -> Option<Rc<SharedElementEntry>> {
        self.inner.internal_state.get().and_then(|w| w.upgrade())
    }

    pub(crate) fn set_internal_state(&self, entry: &Rc<SharedElementEntry>) {
        self.inner.internal_state.set(Some(Rc::downgrade(entry)));
    }

    fn non_null_internal_state(&self) -> Rc<SharedElementEntry> {
        self.internal_state().expect(
            "Error: SharedContentState has not been added to a sharedElement/sharedBounds modifier yet. Therefore \
             the internal state has not been initialized.",
        )
    }
}

// ---- LayerRenderer ------------------------------------------------------------------------------------------------

/// What draws in the scope's overlay (Compose's internal LayerRenderer): shared elements' entries and
/// `render_in_shared_transition_scope_overlay`'s nodes, by their z index.
pub(crate) trait LayerRenderer {
    /// The entry of the shared element it is in.
    fn parent_state(&self) -> Option<Rc<SharedElementEntry>>;
    fn draw_in_overlay(&self, scope: &mut DrawScope, platform: &PlatformHandle);
    fn z_index(&self) -> f32;
    /// Itself as a shared element's entry, if it is one.
    fn as_entry(&self) -> Option<&SharedElementEntry> {
        None
    }
}

/// What a renderer is sorted by (Compose's LayerRendererComparator): a top-level shared element of z index 0
/// before what else is at 0.
fn renderer_order(renderer: &dyn LayerRenderer) -> f32 {
    let z = renderer.z_index();
    if z == 0.0 && renderer.as_entry().is_some() && renderer.parent_state().is_none() { -1.0 } else { z }
}

/// Whether two renderers are the same one.
fn same_renderer(a: &Rc<dyn LayerRenderer>, b: &Rc<dyn LayerRenderer>) -> bool {
    std::ptr::addr_eq(Rc::as_ptr(a), Rc::as_ptr(b))
}

/// Sorted in place by insertion (Compose's private `sortWith`): the list is small and mostly sorted.
fn sort_renderers(renderers: &mut [Rc<dyn LayerRenderer>]) {
    for i in 1..renderers.len() {
        let current = renderers[i].clone();
        let order = renderer_order(&*current);
        let mut j = i;
        while j > 0 && renderer_order(&*renderers[j - 1]).total_cmp(&order).is_gt() {
            renderers[j] = renderers[j - 1].clone();
            j -= 1;
        }
        renderers[j] = current;
    }
}

// ---- The scope ----------------------------------------------------------------------------------------------------

/// Compose's internal SharedTransitionScopeImpl: the lookahead scope it is (by delegation), its shared elements by
/// key, whether a transition is active, the overlay's renderers, the coordinates of its root.
pub(crate) struct SharedTransitionScopeImpl {
    lookahead_scope: Rc<dyn LookaheadScope>,
    pub(crate) coroutine_scope: CoroutineScope,
    /// Asks the root to draw the overlay again (Compose's `invalidateOverlay`), while it is attached.
    pub(crate) invalidate_overlay: RefCell<Option<Rc<dyn Fn()>>>,
    is_transition_active: MutableState<bool>,
    pub(crate) debug_helper: RefCell<Option<Rc<LookaheadAnimationVisualDebugHelper>>>,
    nullable_root: RefCell<Option<LayoutCoordinates>>,
    nullable_lookahead_root: RefCell<Option<LayoutCoordinates>>,
    renderers_version: MutableState<i32>,
    renderers: RefCell<Vec<Rc<dyn LayerRenderer>>>,
    last_sorted_version: Cell<i32>,
    /// Its shared elements by key (Compose's `mutableStateMapOf`: observed, to follow transitions starting and all
    /// ending as elements come and go), in the order they came.
    shared_elements: Observed<Vec<(SlotKey, Rc<SharedElement>)>>,
    /// `{ isTransitionActive }`, one instance (the default `enabled`/`render_in_overlay`: elements stay equal).
    transition_active: Rc<dyn Fn() -> bool>,
    weak_self: Weak<SharedTransitionScopeImpl>,
}

impl Same for Vec<(SlotKey, Rc<SharedElement>)> {
    fn same(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().zip(other).all(|(a, b)| Rc::ptr_eq(&a.1, &b.1))
    }
}

impl LookaheadScope for SharedTransitionScopeImpl {
    fn to_lookahead_coordinates(&self, coordinates: &LayoutCoordinates) -> LayoutCoordinates {
        self.lookahead_scope.to_lookahead_coordinates(coordinates)
    }
    fn lookahead_scope_coordinates(&self, placement_scope: &PlacementScope) -> LayoutCoordinates {
        self.lookahead_scope.lookahead_scope_coordinates(placement_scope)
    }
    fn local_lookahead_position_of(
        &self,
        coordinates: &LayoutCoordinates,
        source_coordinates: &LayoutCoordinates,
        relative_to_source: Offset,
        include_motion_frame_of_reference: bool,
    ) -> Offset {
        self.lookahead_scope.local_lookahead_position_of(
            coordinates,
            source_coordinates,
            relative_to_source,
            include_motion_frame_of_reference,
        )
    }
    fn lookahead_scope_coordinates_of(&self, source_coordinates: &LayoutCoordinates) -> LayoutCoordinates {
        self.lookahead_scope.lookahead_scope_coordinates_of(source_coordinates)
    }
}

impl SharedTransitionScopeImpl {
    fn new(lookahead_scope: Rc<dyn LookaheadScope>, coroutine_scope: CoroutineScope) -> Rc<SharedTransitionScopeImpl> {
        Rc::new_cyclic(|weak: &Weak<SharedTransitionScopeImpl>| {
            let w = weak.clone();
            let is_transition_active = mutable_state_of(false);
            SharedTransitionScopeImpl {
                lookahead_scope,
                coroutine_scope,
                invalidate_overlay: RefCell::new(None),
                is_transition_active,
                debug_helper: RefCell::new(None),
                nullable_root: RefCell::new(None),
                nullable_lookahead_root: RefCell::new(None),
                renderers_version: mutable_state_of(0),
                renderers: RefCell::new(Vec::new()),
                last_sorted_version: Cell::new(-1),
                shared_elements: Observed::new(Vec::new()),
                transition_active: Rc::new(move || w.upgrade().is_some_and(|s| s.is_transition_active())),
                weak_self: weak.clone(),
            }
        })
    }

    fn rc(&self) -> Rc<SharedTransitionScopeImpl> {
        self.weak_self.upgrade().expect("the scope itself")
    }

    /// Whether a shared transition is active (Compose's `isTransitionActive`, a state).
    pub(crate) fn is_transition_active(&self) -> bool {
        self.is_transition_active.get()
    }

    /// The coordinates of the scope's root, as the main pass placed it (Compose's `root`).
    pub(crate) fn root(&self) -> LayoutCoordinates {
        self.nullable_root().expect(
            "Error: Uninitialized LayoutCoordinates. Please make sure when using the SharedTransitionScope \
             composable function, the modifier passed to the child content is being used, or use \
             SharedTransitionLayout instead.",
        )
    }

    pub(crate) fn nullable_root(&self) -> Option<LayoutCoordinates> {
        self.nullable_root.borrow().clone()
    }

    /// The coordinates of the scope's root, as the lookahead pass placed it (Compose's `lookaheadRoot`).
    pub(crate) fn lookahead_root(&self) -> LayoutCoordinates {
        self.nullable_lookahead_root.borrow().clone().expect(
            "Error: Uninitialized LayoutCoordinates. Please make sure when using the SharedTransitionScope \
             composable function, the modifier passed to the child content is being used, or use \
             SharedTransitionLayout instead.",
        )
    }

    /// Its shared elements (a state read: Compose's `sharedElementsIterator`).
    fn shared_elements_iter(&self) -> Vec<Rc<SharedElement>> {
        self.shared_elements.get().into_iter().map(|(_, e)| e).collect()
    }

    fn shared_elements_for(&self, key: &SlotKey) -> Rc<SharedElement> {
        let mut map = self.shared_elements.get();
        if let Some((_, e)) = map.iter().find(|(k, _)| k == key) {
            return e.clone();
        }
        let element = SharedElement::new(key.clone(), &self.rc());
        map.push((key.clone(), element.clone()));
        self.shared_elements.set(map);
        element
    }

    /// What the root observes (Compose's `observeAnimatingBlock`): a shared element animating or transformed by hand
    /// (in a deferred phase the automatic animation is held back but the elements are moved by the gesture: the
    /// transition stays active, keeping the target and bounds for the hand-off and the overlay drawn).
    fn observe_animating_block(&self) -> bool {
        self.shared_elements_iter().iter().any(|e| e.is_animating() || e.is_mutating())
    }

    /// Finds whether a transition is active: a matched shared element animating or transformed by hand (Compose's
    /// `updateTransitionActiveness`); as it ends the matches are dropped where none is left to keep.
    pub(crate) fn update_transition_activeness(&self) {
        let shared_elements = self.shared_elements_iter();
        let mut is_active = false;
        for element in &shared_elements {
            element.update_match();
            // (true for an animating element that lost its match, too: ActiveMatchRemovedDuringTransition)
            is_active = is_active || (element.found_match() && (element.is_animating() || element.is_mutating()));
        }
        if is_active != self.is_transition_active.peek() {
            self.is_transition_active.set(is_active);
            if !is_active {
                self.attach_lookahead_animation_visual_debug_helper();
                for element in &shared_elements {
                    element.on_shared_transition_finished();
                }
            } else {
                self.detach_lookahead_animation_visual_debug_helper();
            }
        }
    }

    fn attach_lookahead_animation_visual_debug_helper(&self) {
        if IS_LOOKAHEAD_ANIMATION_VISUAL_DEBUGGING_ENABLED && let Some(helper) = self.debug_helper.borrow().clone() {
            helper.on_attach(&self.coroutine_scope);
        }
    }

    fn detach_lookahead_animation_visual_debug_helper(&self) {
        if IS_LOOKAHEAD_ANIMATION_VISUAL_DEBUGGING_ENABLED && let Some(helper) = self.debug_helper.borrow().clone() {
            helper.on_detach(&self.coroutine_scope);
        }
    }

    /// The overlay to be sorted again (Compose's `zOrderChanged`).
    pub(crate) fn z_order_changed(&self) {
        self.renderers_version.set(self.renderers_version.peek() + 1);
    }

    /// Draws the overlay: its renderers by z index (Compose's `drawInOverlay`).
    fn draw_in_overlay(&self, scope: &mut DrawScope, platform: &PlatformHandle) {
        let version = self.renderers_version.get();
        if self.last_sorted_version.get() != version {
            sort_renderers(&mut self.renderers.borrow_mut());
            self.last_sorted_version.set(version);
        }
        let renderers = self.renderers.borrow().clone();
        for renderer in renderers {
            renderer.draw_in_overlay(scope, platform);
        }
    }

    /// An entry left the composition (Compose's `onEntryRemoved`): its shared element goes once it has no entries
    /// left after the composition (a coroutine's turn).
    pub(crate) fn on_entry_removed(&self, entry: &Rc<SharedElementEntry>) {
        let shared_element = entry.shared_element();
        shared_element.remove_entry(entry);
        self.update_transition_activeness();
        let as_renderer: Rc<dyn LayerRenderer> = entry.clone();
        self.renderers.borrow_mut().retain(|r| !same_renderer(r, &as_renderer));
        self.z_order_changed();
        if shared_element.all_entries().is_empty() {
            let scope = self.weak_self.clone();
            // (dispatched: it runs after this composition's changes, so an entry of the key they add keeps it)
            self.coroutine_scope.launch(async move {
                if shared_element.all_entries().is_empty()
                    && let Some(scope) = scope.upgrade()
                {
                    let mut map = scope.shared_elements.get();
                    map.retain(|(k, _)| *k != shared_element.key);
                    scope.shared_elements.set(map);
                    shared_element.release();
                }
            });
        }
    }

    /// An entry entered the composition (Compose's `onEntryAdded`): drawn in the overlay after its shared element's
    /// others.
    pub(crate) fn on_entry_added(&self, entry: &Rc<SharedElementEntry>) {
        let shared_element = entry.shared_element();
        shared_element.add_entry(entry.clone());
        self.update_transition_activeness();
        let as_renderer: Rc<dyn LayerRenderer> = entry.clone();
        {
            let mut renderers = self.renderers.borrow_mut();
            let id = renderers
                .iter()
                .position(|r| r.as_entry().is_some_and(|e| Rc::ptr_eq(&e.shared_element(), &shared_element)));
            match id {
                Some(id) if id + 1 < renderers.len() => renderers.insert(id + 1, as_renderer),
                _ => renderers.push(as_renderer),
            }
        }
        self.z_order_changed();
    }

    pub(crate) fn on_layer_renderer_created(&self, renderer: Rc<dyn LayerRenderer>) {
        self.renderers.borrow_mut().push(renderer);
        self.z_order_changed();
    }

    pub(crate) fn on_layer_renderer_removed(&self, renderer: &Rc<dyn LayerRenderer>) {
        self.renderers.borrow_mut().retain(|r| !same_renderer(r, renderer));
        self.z_order_changed();
    }

    /// Lets go of what it holds as it leaves the composition (KompozR's: what Compose's garbage collector does for
    /// the shared elements' state machines holding their entries' nodes).
    fn release(&self) {
        for element in self.shared_elements.peek().into_iter().map(|(_, e)| e) {
            element.release();
        }
    }
}

/// What a shared transition's content is given (Compose's `SharedTransitionScope`, a [`LookaheadScope`]): whether
/// a transition is active, the modifiers that share content between the content going and coming
/// ([`shared_element`](Self::shared_element), [`shared_bounds`](Self::shared_bounds),
/// [`shared_element_with_caller_managed_visibility`](Self::shared_element_with_caller_managed_visibility)), those
/// that lay content out where it is going ([`skip_to_lookahead_size`](Self::skip_to_lookahead_size),
/// [`skip_to_lookahead_position`](Self::skip_to_lookahead_position)) and
/// [`render_in_shared_transition_scope_overlay`](Self::render_in_shared_transition_scope_overlay); shared content
/// states by key. Kotlin's member extensions (`Modifier.sharedElement(…)` in the scope) are its methods taking the
/// modifier first. Clones are the same scope.
#[derive(Clone)]
pub struct SharedTransitionScope {
    pub(crate) inner: Rc<SharedTransitionScopeImpl>,
}

impl PartialEq for SharedTransitionScope {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl LookaheadScope for SharedTransitionScope {
    fn to_lookahead_coordinates(&self, coordinates: &LayoutCoordinates) -> LayoutCoordinates {
        self.inner.to_lookahead_coordinates(coordinates)
    }
    fn lookahead_scope_coordinates(&self, placement_scope: &PlacementScope) -> LayoutCoordinates {
        self.inner.lookahead_scope_coordinates(placement_scope)
    }
    fn local_lookahead_position_of(
        &self,
        coordinates: &LayoutCoordinates,
        source_coordinates: &LayoutCoordinates,
        relative_to_source: Offset,
        include_motion_frame_of_reference: bool,
    ) -> Offset {
        self.inner.local_lookahead_position_of(
            coordinates,
            source_coordinates,
            relative_to_source,
            include_motion_frame_of_reference,
        )
    }
    fn lookahead_scope_coordinates_of(&self, source_coordinates: &LayoutCoordinates) -> LayoutCoordinates {
        self.inner.lookahead_scope_coordinates_of(source_coordinates)
    }
}

/// The scope as the lookahead scope it is (Kotlin's `SharedTransitionScope : LookaheadScope`:
/// `animateBounds(this@SharedTransitionLayout)`).
impl From<SharedTransitionScope> for Rc<dyn LookaheadScope> {
    fn from(scope: SharedTransitionScope) -> Rc<dyn LookaheadScope> {
        scope.inner
    }
}

impl SharedTransitionScope {
    /// Whether a shared transition is active: a matched shared element of the scope animating (Compose's
    /// `isTransitionActive`; a state read).
    pub fn is_transition_active(&self) -> bool {
        self.inner.is_transition_active()
    }

    /// `{ is_transition_active }`, one instance: the default of `enabled`/`render_in_overlay`.
    fn transition_active(&self) -> Rc<dyn Fn() -> bool> {
        self.inner.transition_active.clone()
    }
}

#[named]
impl SharedTransitionScope {
    /// `modifier` laid out at its lookahead size while `enabled` (by default while a transition is active) (Compose's
    /// `Modifier.skipToLookaheadSize`): measured at its lookahead constraints along the approach, so what is in it is
    /// laid out as it will be while its parent's size animates (text that doesn't reflow on the way).
    ///
    /// Kotlin: `skipToLookaheadSize(enabled: () -> Boolean = { isTransitionActive })`
    pub fn skip_to_lookahead_size(
        &self,
        modifier: Modifier,
        #[default(self.transition_active())] enabled: Rc<dyn Fn() -> bool>,
    ) -> Modifier {
        modifier.then_element(SkipToLookaheadSizeElement { scale_to_bounds: None, is_enabled: enabled })
    }

    /// `modifier` placed at its lookahead position while `enabled` (by default while a transition is active)
    /// (Compose's `Modifier.skipToLookaheadPosition`): where it is going in the scope at once, whatever its parent's
    /// placement along the way.
    ///
    /// Kotlin: `skipToLookaheadPosition(enabled: () -> Boolean = { isTransitionActive })`
    pub fn skip_to_lookahead_position(
        &self,
        modifier: Modifier,
        #[default(self.transition_active())] enabled: Rc<dyn Fn() -> bool>,
    ) -> Modifier {
        let scope = self.clone();
        let placing = enabled.clone();
        let placement_in_progress: kompoz_ui::layout::PlacementApproachInProgress =
            Rc::new(move |_: &PlacementScope, _: &LayoutCoordinates| placing());
        named_args!(modifier.approach_layout(
            is_measurement_approach_in_progress = |_| false,
            is_placement_approach_in_progress = Some(placement_in_progress),
            approach_measure = move |s: &kompoz_ui::ApproachMeasureScope, m: &dyn Measurable, c: Constraints| {
                let placeable = m.measure(c);
                let (scope, enabled) = (scope.clone(), enabled.clone());
                s.layout(placeable.width(), placeable.height(), move |p: &PlacementScope| {
                    if !enabled() {
                        p.place(&placeable, 0, 0);
                        return;
                    }
                    let Some(it) = p.coordinates() else {
                        p.place(&placeable, 0, 0);
                        return;
                    };
                    let lookahead_scope_coordinates = scope.lookahead_scope_coordinates(p);
                    let target =
                        scope.local_lookahead_position_of(&lookahead_scope_coordinates, &it, Offset::Zero, true);
                    let actual = lookahead_scope_coordinates.local_position_of(&it, Offset::Zero);
                    let delta = target - actual;
                    let offset = it.local_position_of(&lookahead_scope_coordinates, delta)
                        - it.local_position_of(&lookahead_scope_coordinates, Offset::Zero);
                    p.place(
                        &placeable,
                        crate::converter::fast_round_to_int(offset.x),
                        crate::converter::fast_round_to_int(offset.y),
                    );
                })
            }
        ))
    }

    /// `modifier` drawn in the scope's overlay while `render_in_overlay` (by default while a transition is active),
    /// at `z_index_in_overlay` among the shared elements there (Compose's
    /// `Modifier.renderInSharedTransitionScopeOverlay`): over the moving shared elements of a lower z index and under
    /// those of a higher one (shared elements are at 0 by default), where it is; drawn where it is otherwise, never
    /// in both.
    ///
    /// Kotlin: `renderInSharedTransitionScopeOverlay(zIndexInOverlay: Float = 0f, renderInOverlay: () -> Boolean = { isTransitionActive })`
    pub fn render_in_shared_transition_scope_overlay(
        &self,
        modifier: Modifier,
        #[default(0.0)] z_index_in_overlay: f32,
        #[default(self.transition_active())] render_in_overlay: Rc<dyn Fn() -> bool>,
    ) -> Modifier {
        modifier.then_element(RenderInTransitionOverlayNodeElement {
            shared_transition_scope: self.inner.clone(),
            render_in_overlay,
            z_index_in_overlay,
        })
    }

    /// `modifier` a shared element of `shared_content_state`'s key (Compose's `Modifier.sharedElement`): while
    /// another of the key in content going (or coming) by `animated_visibility_scope`'s transition matches it, the one
    /// becoming visible moves (and resizes: measured at the bounds) from the other's bounds to its own along the
    /// spec `bounds_transform` gives, drawn in the overlay by `render_in_overlay_during_transition` at
    /// `z_index_in_overlay`, clipped by `clip_in_overlay_during_transition` (its parent shared element's clip by
    /// default); the one going isn't drawn. Its layout's size in its parent meanwhile is `placeholder_size`'s.
    /// Composable.
    ///
    /// Kotlin: `sharedElement(sharedContentState: SharedContentState, animatedVisibilityScope: AnimatedVisibilityScope, boundsTransform: BoundsTransform = SharedTransitionDefaults.BoundsTransform, placeholderSize: PlaceholderSize = ContentSize, renderInOverlayDuringTransition: Boolean = true, zIndexInOverlay: Float = 0f, clipInOverlayDuringTransition: OverlayClip = ParentClip)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn shared_element(
        &self,
        modifier: Modifier,
        shared_content_state: SharedContentState,
        animated_visibility_scope: &AnimatedVisibilityScope,
        #[default(SharedTransitionDefaults::BoundsTransform())] bounds_transform: Rc<dyn BoundsTransform>,
        #[default(PlaceholderSize::ContentSize)] placeholder_size: PlaceholderSize,
        #[default(true)] render_in_overlay_during_transition: bool,
        #[default(0.0)] z_index_in_overlay: f32,
        #[default(ParentClip())] clip_in_overlay_during_transition: Rc<dyn OverlayClip>,
    ) -> Modifier {
        let this = self.clone();
        let transition = animated_visibility_scope.transition();
        group(move || {
            this.shared_bounds_impl(
                modifier,
                shared_content_state,
                Visibility::Transition(transition),
                bounds_transform,
                placeholder_size,
                true,
                render_in_overlay_during_transition,
                z_index_in_overlay,
                clip_in_overlay_during_transition,
            )
        })
    }

    /// `modifier` shared bounds of `shared_content_state`'s key (Compose's `Modifier.sharedBounds`): as
    /// [`shared_element`](Self::shared_element), but both contents are drawn in the moving bounds, the going one
    /// by `exit` and the coming one by `enter` (faded by default), each resized by `resize_mode` (scaled into them
    /// from its lookahead size by default). Composable.
    ///
    /// Kotlin: `sharedBounds(sharedContentState: SharedContentState, animatedVisibilityScope: AnimatedVisibilityScope, enter: EnterTransition = fadeIn(), exit: ExitTransition = fadeOut(), boundsTransform: BoundsTransform = SharedTransitionDefaults.BoundsTransform, resizeMode: ResizeMode = scaleToBounds(ContentScale.FillWidth, Center), placeholderSize: PlaceholderSize = ContentSize, renderInOverlayDuringTransition: Boolean = true, zIndexInOverlay: Float = 0f, clipInOverlayDuringTransition: OverlayClip = ParentClip)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn shared_bounds(
        &self,
        modifier: Modifier,
        shared_content_state: SharedContentState,
        animated_visibility_scope: &AnimatedVisibilityScope,
        #[default(fade_in())] enter: EnterTransition,
        #[default(fade_out())] exit: ExitTransition,
        #[default(SharedTransitionDefaults::BoundsTransform())] bounds_transform: Rc<dyn BoundsTransform>,
        #[default(ResizeMode::scale_to_bounds(ContentScale::FillWidth, Alignment::Center))] resize_mode: ResizeMode,
        #[default(PlaceholderSize::ContentSize)] placeholder_size: PlaceholderSize,
        #[default(true)] render_in_overlay_during_transition: bool,
        #[default(0.0)] z_index_in_overlay: f32,
        #[default(ParentClip())] clip_in_overlay_during_transition: Rc<dyn OverlayClip>,
    ) -> Modifier {
        let this = self.clone();
        let transition = animated_visibility_scope.transition();
        group(move || {
            let key = shared_content_state.key();
            let state = shared_content_state.clone();
            let bounds = this.shared_bounds_impl(
                modifier,
                shared_content_state,
                Visibility::Transition(transition.clone()),
                bounds_transform,
                placeholder_size,
                false,
                render_in_overlay_during_transition,
                z_index_in_overlay,
                clip_in_overlay_during_transition,
            );
            // (whether a match is found isn't known yet as this composes: the content's enter/exit and scaling are
            // enabled by it later in the frame — further in the composition, or in a measure or a placement that
            // subcomposes)
            let matched = state.clone();
            let enter_exit = crate::enter_exit::create_modifier(
                &transition,
                enter,
                exit,
                true,
                Rc::new(move || matched.is_match_found()),
                None,
                &format!("enter/exit for {key:?}"),
            );
            let scaled = match resize_mode.kind {
                ResizeKind::ScaleToBounds(scale_to_bounds) => {
                    content_scale_modifier(Modifier, scale_to_bounds, Rc::new(move || state.is_match_found()))
                }
                ResizeKind::RemeasureToBounds => Modifier,
            };
            bounds.then(enter_exit).then(scaled)
        })
    }

    /// `modifier` a shared element whose visibility the caller manages (Compose's
    /// `Modifier.sharedElementWithCallerManagedVisibility`): `visible` says whether it is the one becoming visible —
    /// the shared elements of a key not shown at once by AnimatedVisibility/AnimatedContent. Otherwise as
    /// [`shared_element`](Self::shared_element). Composable.
    ///
    /// Kotlin: `sharedElementWithCallerManagedVisibility(sharedContentState: SharedContentState, visible: Boolean, boundsTransform: BoundsTransform = SharedTransitionDefaults.BoundsTransform, placeholderSize: PlaceholderSize = ContentSize, renderInOverlayDuringTransition: Boolean = true, zIndexInOverlay: Float = 0f, clipInOverlayDuringTransition: OverlayClip = ParentClip)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn shared_element_with_caller_managed_visibility(
        &self,
        modifier: Modifier,
        shared_content_state: SharedContentState,
        visible: bool,
        #[default(SharedTransitionDefaults::BoundsTransform())] bounds_transform: Rc<dyn BoundsTransform>,
        #[default(PlaceholderSize::ContentSize)] placeholder_size: PlaceholderSize,
        #[default(true)] render_in_overlay_during_transition: bool,
        #[default(0.0)] z_index_in_overlay: f32,
        #[default(ParentClip())] clip_in_overlay_during_transition: Rc<dyn OverlayClip>,
    ) -> Modifier {
        let this = self.clone();
        group(move || {
            this.shared_bounds_impl(
                modifier,
                shared_content_state,
                Visibility::Caller(visible),
                bounds_transform,
                placeholder_size,
                true,
                render_in_overlay_during_transition,
                z_index_in_overlay,
                clip_in_overlay_during_transition,
            )
        })
    }

    /// A clip of `clip_shape` in the overlay: the shape's outline in the shared element's bounds (Compose's
    /// `OverlayClip(clipShape)`).
    ///
    /// Kotlin: `OverlayClip(clipShape: Shape)`
    pub fn OverlayClip(&self, clip_shape: impl Shape + 'static) -> Rc<dyn OverlayClip> {
        Rc::new(ShapeBasedClip { clip_shape: Arc::new(clip_shape) })
    }

    /// The shared content state of `key`, remembered where it is called (made again for another key), taking
    /// `config` (Compose's `rememberSharedContentState(key)` and `rememberSharedContentState(key, config)`).
    /// Composable.
    ///
    /// Kotlin: `rememberSharedContentState(key: Any, config: SharedContentConfig = SharedTransitionDefaults.SharedContentConfig)`
    #[track_caller]
    pub fn remember_shared_content_state(
        &self,
        key: SlotKey,
        #[default(SharedTransitionDefaults::SharedContentConfig())] config: Rc<dyn SharedContentConfig>,
    ) -> SharedContentState {
        group(move || {
            let k = key.clone();
            let c = config.clone();
            let state = remember_keyed(key, move || SharedContentState::new(k, c));
            state.set_config(config);
            state
        })
    }

    /// A configuration of the defaults, or one not transformed during a deferred transition's manual phase
    /// (Compose's `SharedContentConfig()` and experimental
    /// `SharedContentConfig(permitTransformDuringDeferredTransition)`).
    ///
    /// Kotlin: `SharedContentConfig(permitTransformDuringDeferredTransition: Boolean = true)`
    pub fn SharedContentConfig(
        &self,
        #[default(true)] permit_transform_during_deferred_transition: bool,
    ) -> Rc<dyn SharedContentConfig> {
        if permit_transform_during_deferred_transition {
            CACHED_CONFIG.with(|c| c.clone())
        } else {
            Rc::new(NotTransformedDuringDeferredTransition)
        }
    }
}

impl SharedTransitionScope {
    /// Shared bounds whose visibility the caller manages (Compose's internal
    /// `Modifier.sharedBoundsWithCallerManagedVisibility`): as
    /// [`shared_element_with_caller_managed_visibility`](Self::shared_element_with_caller_managed_visibility), but
    /// drawn whether shown or not, with no enter/exit of its own (one put after it in the chain applies). Composable.
    #[cfg_attr(not(test), expect(dead_code, reason = "Compose 1.12.1's internal, which nothing there calls yet"))]
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn shared_bounds_with_caller_managed_visibility(
        &self,
        modifier: Modifier,
        shared_content_state: SharedContentState,
        visible: bool,
        bounds_transform: Rc<dyn BoundsTransform>,
        placeholder_size: PlaceholderSize,
        render_in_overlay_during_transition: bool,
        z_index_in_overlay: f32,
        clip_in_overlay_during_transition: Rc<dyn OverlayClip>,
    ) -> Modifier {
        let this = self.clone();
        group(move || {
            this.shared_bounds_impl(
                modifier,
                shared_content_state,
                Visibility::Caller(visible),
                bounds_transform,
                placeholder_size,
                false,
                render_in_overlay_during_transition,
                z_index_in_overlay,
                clip_in_overlay_during_transition,
            )
        })
    }

    /// What `shared_element`, `shared_bounds` and `shared_element_with_caller_managed_visibility` share (Compose's
    /// private `sharedBoundsImpl`): the key's shared element, the bounds animation on a child transition of the
    /// visibility (or a transition of its own of the caller's visibility), the entry remembered here, its node.
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    fn shared_bounds_impl(
        &self,
        modifier: Modifier,
        shared_content_state: SharedContentState,
        visibility: Visibility,
        bounds_transform: Rc<dyn BoundsTransform>,
        placeholder_size: PlaceholderSize,
        render_only_when_visible: bool,
        render_in_overlay_during_transition: bool,
        z_index_in_overlay: f32,
        clip_in_overlay_during_transition: Rc<dyn OverlayClip>,
    ) -> Modifier {
        let scope = self.inner.clone();
        let key = shared_content_state.key();
        let entry = kompoz_runtime::key(&key, || {
            let shared_element = {
                let (s, k) = (scope.clone(), key.clone());
                remember(move || s.shared_elements_for(&k))
            };
            let parent_id = match &visibility {
                Visibility::Transition(t) => t.identity(),
                Visibility::Caller(_) => 0,
            };
            let bounds_animation = kompoz_runtime::key(&parent_id, || {
                let bounds_transition: Transition<bool> = match &visibility {
                    Visibility::Transition(parent) => named_args!(parent.create_child_transition(
                        label = format!("{key:?}"),
                        transform_to_child_state = |s: &EnterExitState| *s == EnterExitState::Visible
                    )),
                    Visibility::Caller(target_state) => group(|| {
                        let target_state = *target_state;
                        let se = shared_element.clone();
                        let transition_state = remember(move || {
                            // (other shared elements of the key there already: an animation is likely, so the
                            // initial state the other way; it ends at once if none is needed)
                            let initial = if se.enabled_entries().is_empty() { target_state } else { !target_state };
                            MutableTransitionState(initial)
                        });
                        transition_state.set_target_state(target_state);
                        remember_transition(transition_state)
                    }),
                };
                let active = scope.is_transition_active();
                let animation =
                    kompoz_runtime::key(&active, || bounds_transition.create_deferred_animation(Rect::VectorConverter));
                let (s, t, a, b) =
                    (scope.clone(), bounds_transition.clone(), animation.clone(), bounds_transform.clone());
                let momentum = shared_element.momentum_animation_offset.clone();
                let bounds_animation =
                    remember_keyed(bounds_transition.clone(), move || BoundsAnimation::new(&s, t, a, b, momentum));
                bounds_animation.update_animation(animation, bounds_transform.clone());
                bounds_animation
            });
            remember_shared_element_state(
                shared_element,
                bounds_animation,
                placeholder_size,
                render_only_when_visible,
                shared_content_state,
                clip_in_overlay_during_transition,
                z_index_in_overlay,
                render_in_overlay_during_transition,
            )
        });
        modifier.then_element(SharedBoundsNodeElement { shared_element_state: entry })
    }
}

/// Whose visibility a shared element follows: its AnimatedVisibilityScope's transition (visible at `Visible`), or
/// the caller's.
enum Visibility {
    Transition(Transition<EnterExitState>),
    Caller(bool),
}

/// The entry remembered here, its properties updated (Compose's private `rememberSharedElementState`).
#[track_caller]
#[allow(clippy::too_many_arguments)]
fn remember_shared_element_state(
    shared_element: Rc<SharedElement>,
    bounds_animation: Rc<BoundsAnimation>,
    placeholder_size: PlaceholderSize,
    render_only_when_visible: bool,
    shared_content_state: SharedContentState,
    clip_in_overlay_during_transition: Rc<dyn OverlayClip>,
    z_index_in_overlay: f32,
    render_in_overlay_during_transition: bool,
) -> Rc<SharedElementEntry> {
    let observer = {
        let (se, ba, ps, cs, clip) = (
            shared_element.clone(),
            bounds_animation.clone(),
            placeholder_size.clone(),
            shared_content_state.clone(),
            clip_in_overlay_during_transition.clone(),
        );
        remember_observer(move || {
            EntryObserver(Rc::new(SharedElementEntry::new(
                se,
                ba,
                ps,
                render_only_when_visible,
                clip,
                render_in_overlay_during_transition,
                cs,
                z_index_in_overlay,
            )))
        })
    };
    let entry = observer.0.clone();
    shared_content_state.set_internal_state(&entry);
    // (any of them changed: updated)
    entry.set_shared_element(shared_element);
    entry.set_render_only_when_visible(render_only_when_visible);
    entry.set_bounds_animation(bounds_animation);
    entry.set_placeholder_size(placeholder_size);
    entry.set_overlay_clip(clip_in_overlay_during_transition);
    entry.set_z_index(z_index_in_overlay);
    entry.set_render_in_overlay_during_transition(render_in_overlay_during_transition);
    entry.set_user_state(shared_content_state);
    entry
}

// ---- The root -----------------------------------------------------------------------------------------------------

/// The modifier `SharedTransitionScope`'s content puts on its root (Compose's private
/// SharedTransitionScopeRootModifierElement): equal for the same scope.
struct SharedTransitionScopeRootModifierElement {
    shared_transition_scope: Rc<SharedTransitionScopeImpl>,
}

impl PartialEq for SharedTransitionScopeRootModifierElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.shared_transition_scope, &other.shared_transition_scope)
    }
}

impl ModifierNodeElement for SharedTransitionScopeRootModifierElement {
    type Node = SharedTransitionScopeRootModifierNode;
    fn create(&self) -> SharedTransitionScopeRootModifierNode {
        SharedTransitionScopeRootModifierNode { shared_scope: self.shared_transition_scope.clone(), handle: None }
    }
    fn update(&self, node: &mut SharedTransitionScopeRootModifierNode) {
        if !Rc::ptr_eq(&node.shared_scope, &self.shared_transition_scope) {
            let scope = self.shared_transition_scope.clone();
            if let Some(handle) = &node.handle {
                handle.observe_reads(|| scope.observe_animating_block());
            }
        }
        node.shared_scope = self.shared_transition_scope.clone();
    }
    fn name(&self) -> &'static str {
        "SharedTransitionScopeRootModifier"
    }
}

/// The root's node (Compose's private SharedTransitionScopeRootModifierNode): it knows the root's coordinates in
/// both passes, observes whether a shared element animates (a transition active or not), and draws the overlay over
/// its content.
struct SharedTransitionScopeRootModifierNode {
    shared_scope: Rc<SharedTransitionScopeImpl>,
    handle: Option<NodeHandle>,
}

impl ModifierNode for SharedTransitionScopeRootModifierNode {
    node_capabilities!(layout, draw, observer);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        let scope = self.shared_scope.clone();
        cx.observe_reads(|| scope.observe_animating_block());
        let handle = cx.handle();
        *self.shared_scope.invalidate_overlay.borrow_mut() = Some(Rc::new(move || handle.invalidate_draw()));
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        *self.shared_scope.invalidate_overlay.borrow_mut() = None;
    }
}

impl LayoutModifierNode for SharedTransitionScopeRootModifierNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = measurable.measure(constraints);
        let (shared_scope, handle) = (self.shared_scope.clone(), self.handle.clone());
        scope.layout(placeable.width(), placeable.height(), move |p: &PlacementScope| {
            if let Some(coordinates) = p.coordinates() {
                if !p.is_looking_ahead() {
                    *shared_scope.nullable_root.borrow_mut() = Some(coordinates.clone());
                    let debugging = IS_LOOKAHEAD_ANIMATION_VISUAL_DEBUGGING_ENABLED
                        && handle
                            .as_ref()
                            .is_some_and(|h| h.current_value_of(&LocalLookaheadAnimationVisualDebugConfig).is_enabled);
                    if debugging {
                        let helper = shared_scope
                            .debug_helper
                            .borrow_mut()
                            .get_or_insert_with(|| Rc::new(LookaheadAnimationVisualDebugHelper::new()))
                            .clone();
                        helper.update_drawing_coordinates(
                            coordinates.find_root_coordinates().position_in_root(),
                            shared_scope.lookahead_root().size(),
                        );
                    }
                } else {
                    *shared_scope.nullable_lookahead_root.borrow_mut() = Some(coordinates);
                }
            }
            p.place(&placeable, 0, 0);
        })
    }
}

impl ObserverModifierNode for SharedTransitionScopeRootModifierNode {
    fn on_observed_reads_changed(&mut self) {
        self.shared_scope.update_transition_activeness();
        let scope = self.shared_scope.clone();
        if let Some(handle) = &self.handle {
            handle.observe_reads(|| scope.observe_animating_block());
        }
    }
}

impl DrawModifierNode for SharedTransitionScopeRootModifierNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        scope.draw_content();
        let Some(handle) = self.handle.clone() else { return };
        let platform = handle.current_value_of(&LocalPlatform);
        let shared_scope = &self.shared_scope;
        if IS_LOOKAHEAD_ANIMATION_VISUAL_DEBUGGING_ENABLED {
            let config = handle.current_value_of(&LocalLookaheadAnimationVisualDebugConfig);
            let helper = shared_scope.debug_helper.borrow().clone();
            let marked = config.is_enabled && shared_scope.is_transition_active();
            if marked && let Some(helper) = &helper {
                helper.draw_overlay(scope, config.overlay_color);
            }
            shared_scope.draw_in_overlay(scope, &platform);
            if marked && let Some(helper) = &helper {
                helper.draw_global_visualizations(scope);
            }
        } else {
            shared_scope.draw_in_overlay(scope, &platform);
        }
    }
}

// ---- The composables ----------------------------------------------------------------------------------------------

/// A lookahead scope for shared elements whose layout is the content's (Compose's `SharedTransitionScope(content)`):
/// `content` gets the [`SharedTransitionScope`] and the modifier its root layout must take (the overlay is drawn
/// over that layout and the scope's coordinates are its). [`SharedTransitionLayout`] is the common case.
///
/// Kotlin: `SharedTransitionScope(content: @Composable SharedTransitionScope.(Modifier) -> Unit)`
#[kompozable]
pub fn SharedTransitionScope(content: impl Fn(&SharedTransitionScope, Modifier) + 'static) {
    kompoz_ui::LookaheadScope(move |lookahead: &Rc<dyn LookaheadScope>| {
        let coroutine_scope = remember_coroutine_scope();
        let lookahead = lookahead.clone();
        let shared_scope = remember(move || SharedTransitionScope {
            inner: SharedTransitionScopeImpl::new(lookahead, coroutine_scope),
        });
        // (the shared elements' state machines let go of their entries' nodes as the scope leaves)
        let released = shared_scope.inner.clone();
        kompozable_block!({
            DisposableEffect(shared_scope.clone(), move || on_dispose(move || released.release()));
        });
        let root = Modifier.then_element(SharedTransitionScopeRootModifierElement {
            shared_transition_scope: shared_scope.inner.clone(),
        });
        content(&shared_scope, root);
    });
}

/// A layout whose content shares elements between what goes and what comes (Compose's `SharedTransitionLayout`):
/// its content gets the [`SharedTransitionScope`] and is laid out as a Box's is, at the top start, one over the
/// other; the shared elements move in its overlay, drawn over it. `modifier` is before the scope's own (it sizes,
/// places and clips the overlay too).
///
/// Kotlin: `SharedTransitionLayout(modifier: Modifier = Modifier, content: @Composable SharedTransitionScope.() -> Unit)`
#[kompozable]
pub fn SharedTransitionLayout(
    #[default(Modifier)] modifier: Modifier,
    content: impl Fn(&SharedTransitionScope) + 'static,
) {
    let content = Rc::new(content);
    SharedTransitionScope(move |scope: &SharedTransitionScope, shared_transition_modifier: Modifier| {
        let (scope, content) = (scope.clone(), content.clone());
        let policy = remember(|| Rc::new(crate::crossfade::CrossfadeBoxPolicy) as Rc<dyn MeasurePolicy>);
        layout_into(modifier.clone().then(shared_transition_modifier), policy, false, move || content(&scope));
    });
}
