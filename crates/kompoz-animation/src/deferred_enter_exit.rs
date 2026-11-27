//! What a deferred phase of a transition does to content by hand (Compose's experimental DeferredEnterExitTransition.kt
//! and AnimatedContent's `MutableContentTransform`): while a [`DeferredTransitionState`](crate::DeferredTransitionState)
//! defers (a predictive back gesture, a drag), a [`MutableTransform`]'s block sets the content's alpha, scale, transform
//! origin, offset and veil through a [`MutableTransformScope`] as the gesture goes; when the transition takes over, its
//! animations start from where the block left the content (its values and velocities handed over).

use crate::vector::{AnimationVector1D, AnimationVector2D};
use kompoz_graphics::{Color, IntOffset, IntSize, Offset};
use kompoz_runtime::clock::Instant;
use kompoz_runtime::{MutableState, mutable_state_of, named};
use kompoz_ui::input::{VelocityTracker, VelocityTracker1D};
use kompoz_ui::layout::{LayoutCoordinates, TransformOrigin};
use kompoz_ui::modifier::{ModifierLocal, modifier_local_of};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

thread_local! {
    /// The milliseconds the handed-over velocities are tracked against, in tests (Compose's `@VisibleForTesting
    /// testTimeSource`); none: the monotonic clock's (Compose's `TimeSource.Monotonic`).
    pub(crate) static TEST_TIME_SOURCE: RefCell<Option<Rc<dyn Fn() -> i64>>> = const { RefCell::new(None) };
}

/// A [`MutableTransform`]'s block: it sets the content's transform through the scope, given the content's full size.
pub type MutableTransformBlock = Rc<dyn Fn(&mut dyn MutableTransformScope, IntSize)>;

/// What a deferred phase does to one content by hand (Compose's experimental MutableTransform): its block (set by
/// [`update`](Self::update)) runs as the content is laid out while the transition defers; `veil_match_parent_size`
/// says whether a veil it sets covers the content's parent; `offset_velocity_provider` gives the velocity of the
/// offset handed over (else it is tracked from the offsets the block set). Clones are the same transform.
#[derive(Clone)]
pub struct MutableTransform {
    inner: Rc<MutableTransformInner>,
}

struct MutableTransformInner {
    veil_match_parent_size: Cell<bool>,
    offset_velocity_provider: RefCell<Option<Rc<dyn Fn() -> Offset>>>,
    block: RefCell<Option<MutableTransformBlock>>,
}

impl PartialEq for MutableTransform {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

/// A [`MutableTransform`] (Compose's `MutableTransform(veilMatchParentSize, offsetVelocityProvider, block)`).
///
/// Kotlin: `MutableTransform(veilMatchParentSize: Boolean = false, offsetVelocityProvider: (() -> Offset)? = null, block: (TransformScope.(fullSize: IntSize) -> Unit)? = null)`
#[named(type)]
pub fn MutableTransform(
    #[default(false)] veil_match_parent_size: bool,
    #[default(None)] offset_velocity_provider: Option<Rc<dyn Fn() -> Offset>>,
    #[default(None)] block: Option<MutableTransformBlock>,
) -> MutableTransform {
    new_mutable_transform(veil_match_parent_size, offset_velocity_provider, block)
}

fn new_mutable_transform(
    veil_match_parent_size: bool,
    offset_velocity_provider: Option<Rc<dyn Fn() -> Offset>>,
    block: Option<MutableTransformBlock>,
) -> MutableTransform {
    MutableTransform {
        inner: Rc::new(MutableTransformInner {
            veil_match_parent_size: Cell::new(veil_match_parent_size),
            offset_velocity_provider: RefCell::new(offset_velocity_provider),
            block: RefCell::new(block),
        }),
    }
}

impl MutableTransform {
    /// Sets the block run while the transition defers (Compose's `update`): it sets the content's transform through
    /// the [`MutableTransformScope`], given the content's full size.
    pub fn update(&self, block: impl Fn(&mut dyn MutableTransformScope, IntSize) + 'static) {
        *self.inner.block.borrow_mut() = Some(Rc::new(block));
    }

    pub(crate) fn veil_match_parent_size(&self) -> bool {
        self.inner.veil_match_parent_size.get()
    }

    pub(crate) fn block(&self) -> Option<MutableTransformBlock> {
        self.inner.block.borrow().clone()
    }

    fn offset_velocity_provider(&self) -> Option<Rc<dyn Fn() -> Offset>> {
        self.inner.offset_velocity_provider.borrow().clone()
    }
}

/// What a [`MutableTransform`]'s block sets on its content (Compose's experimental TransformScope, another name here:
/// foundation's gesture `TransformScope` holds it in the prelude): its alpha, scale,
/// transform origin, offset and veil color, each combined with what the transition animates (alpha and scale
/// multiplied, the offset added, the transform origin and the veil replacing). What the block doesn't set stays the
/// transition's.
pub trait MutableTransformScope {
    fn alpha(&self) -> f32;
    fn set_alpha(&mut self, alpha: f32);
    fn scale(&self) -> f32;
    fn set_scale(&mut self, scale: f32);
    fn transform_origin(&self) -> TransformOrigin;
    fn set_transform_origin(&mut self, transform_origin: TransformOrigin);
    fn offset(&self) -> IntOffset;
    fn set_offset(&mut self, offset: IntOffset);
    fn veil(&self) -> Color;
    fn set_veil(&mut self, veil: Color);
}

/// Compose's TransformScopeImpl: the values set, as states (a layer reading them is updated), and which were set.
pub(crate) struct TransformScopeImpl {
    is_alpha_mutated: MutableState<bool>,
    alpha: MutableState<f32>,
    is_scale_mutated: MutableState<bool>,
    scale: MutableState<f32>,
    is_offset_mutated: Cell<bool>,
    offset: Cell<IntOffset>,
    is_transform_origin_mutated: MutableState<bool>,
    transform_origin: MutableState<(f32, f32)>,
    is_veil_mutated: MutableState<bool>,
    veil: MutableState<Color>,
}

impl TransformScopeImpl {
    fn new() -> TransformScopeImpl {
        TransformScopeImpl {
            is_alpha_mutated: mutable_state_of(false),
            alpha: mutable_state_of(1.0),
            is_scale_mutated: mutable_state_of(false),
            scale: mutable_state_of(1.0),
            is_offset_mutated: Cell::new(false),
            offset: Cell::new(IntOffset::Zero),
            is_transform_origin_mutated: mutable_state_of(false),
            transform_origin: mutable_state_of((0.5, 0.5)),
            is_veil_mutated: mutable_state_of(false),
            veil: mutable_state_of(Color::Transparent),
        }
    }

    fn reset(&self) {
        self.is_alpha_mutated.set(false);
        self.is_scale_mutated.set(false);
        self.is_offset_mutated.set(false);
        self.is_transform_origin_mutated.set(false);
        self.is_veil_mutated.set(false);
    }
}

/// The scope a block is given: the shared state's [`TransformScopeImpl`].
struct MutableTransformScopeRef<'a>(&'a TransformScopeImpl);

impl MutableTransformScope for MutableTransformScopeRef<'_> {
    fn alpha(&self) -> f32 {
        self.0.alpha.get()
    }
    fn set_alpha(&mut self, alpha: f32) {
        self.0.alpha.set(alpha);
        self.0.is_alpha_mutated.set(true);
    }
    fn scale(&self) -> f32 {
        self.0.scale.get()
    }
    fn set_scale(&mut self, scale: f32) {
        self.0.scale.set(scale);
        self.0.is_scale_mutated.set(true);
    }
    fn transform_origin(&self) -> TransformOrigin {
        let (x, y) = self.0.transform_origin.get();
        TransformOrigin::new(x, y)
    }
    fn set_transform_origin(&mut self, transform_origin: TransformOrigin) {
        self.0.transform_origin.set((transform_origin.pivot_fraction_x, transform_origin.pivot_fraction_y));
        self.0.is_transform_origin_mutated.set(true);
    }
    fn offset(&self) -> IntOffset {
        self.0.offset.get()
    }
    fn set_offset(&mut self, offset: IntOffset) {
        self.0.offset.set(offset);
        self.0.is_offset_mutated.set(true);
    }
    fn veil(&self) -> Color {
        self.0.veil.get()
    }
    fn set_veil(&mut self, veil: Color) {
        self.0.veil.set(veil);
        self.0.is_veil_mutated.set(true);
    }
}

/// The modifier local an enter/exit modifier provides its [`SharedMutableTransformState`] by (Compose's internal
/// `ModifierLocalSharedMutableTransformState`): what a shared element inside reads.
pub(crate) static ModifierLocalSharedMutableTransformState: ModifierLocal<Option<SharedMutableTransformState>> =
    modifier_local_of(|| None);

/// What a deferred phase did to a content, shared by its enter/exit modifiers (Compose's internal
/// SharedMutableTransformState): whether the transition defers (`is_mutating`), the block's [`MutableTransform`], the
/// values the content was last given while it deferred (where the transition's animations start from when it takes
/// over: the hand-off), the velocities of the scale and the offset. Clones are the same state.
#[derive(Clone)]
pub(crate) struct SharedMutableTransformState {
    inner: Rc<SharedInner>,
}

struct SharedInner {
    is_mutating: MutableState<bool>,
    is_handoff_active: MutableState<bool>,
    last_mutable_data: RefCell<Option<MutableTransform>>,
    mutable_data: RefCell<Option<MutableTransform>>,
    transform_scope: TransformScopeImpl,
    start_time: Instant,
    parent_layout_coordinates: RefCell<Option<LayoutCoordinates>>,
    last_veil: Cell<Color>,
    last_alpha: Cell<f32>,
    last_scale: Cell<f32>,
    last_transform_origin: Cell<TransformOrigin>,
    last_slide: Cell<IntOffset>,
    last_manual_scale: Cell<f32>,
    last_manual_slide: Cell<IntOffset>,
    scale_velocity_tracker: RefCell<Option<VelocityTracker1D>>,
    offset_velocity_tracker: RefCell<Option<VelocityTracker>>,
}

impl PartialEq for SharedMutableTransformState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl SharedMutableTransformState {
    pub(crate) fn new() -> SharedMutableTransformState {
        SharedMutableTransformState {
            inner: Rc::new(SharedInner {
                is_mutating: mutable_state_of(false),
                is_handoff_active: mutable_state_of(false),
                last_mutable_data: RefCell::new(None),
                mutable_data: RefCell::new(None),
                transform_scope: TransformScopeImpl::new(),
                start_time: Instant::now(),
                parent_layout_coordinates: RefCell::new(None),
                last_veil: Cell::new(Color::Transparent),
                last_alpha: Cell::new(1.0),
                last_scale: Cell::new(1.0),
                last_transform_origin: Cell::new(TransformOrigin::Center),
                last_slide: Cell::new(IntOffset::Zero),
                last_manual_scale: Cell::new(1.0),
                last_manual_slide: Cell::new(IntOffset::Zero),
                scale_velocity_tracker: RefCell::new(None),
                offset_velocity_tracker: RefCell::new(None),
            }),
        }
    }

    /// Whether the transition defers (a state).
    pub(crate) fn is_mutating(&self) -> bool {
        self.inner.is_mutating.get()
    }

    /// Compose's `isMutating` setter: the hand-off starts as the deferring stops.
    pub(crate) fn set_is_mutating(&self, value: bool) {
        let was = self.inner.is_mutating.peek();
        if was && !value {
            self.inner.is_handoff_active.set(true);
        } else if value {
            self.inner.is_handoff_active.set(false);
        }
        if was != value {
            self.inner.is_mutating.set(value);
        }
    }

    /// Whether the transition's animations start from what the block left (a state).
    pub(crate) fn is_handoff_active(&self) -> bool {
        self.inner.is_handoff_active.get()
    }

    pub(crate) fn mutable_data(&self) -> Option<MutableTransform> {
        self.inner.mutable_data.borrow().clone()
    }

    pub(crate) fn set_mutable_data(&self, value: Option<MutableTransform>) {
        if let Some(v) = &value {
            *self.inner.last_mutable_data.borrow_mut() = Some(v.clone());
        }
        *self.inner.mutable_data.borrow_mut() = value;
    }

    fn has_block(&self) -> bool {
        self.mutable_data().is_some_and(|d| d.block().is_some())
    }

    /// The scale the block set (1 if none: Compose's `activeScale`).
    pub(crate) fn active_scale(&self) -> f32 {
        let s = &self.inner.transform_scope;
        if s.is_scale_mutated.get() { s.scale.get() } else { 1.0 }
    }

    /// The offset the block set (Compose's `activeOffset`).
    pub(crate) fn active_offset(&self) -> IntOffset {
        let s = &self.inner.transform_scope;
        if s.is_offset_mutated.get() { s.offset.get() } else { IntOffset::Zero }
    }

    /// The transform origin the block set (Compose's `activeTransformOrigin`).
    pub(crate) fn active_transform_origin(&self) -> TransformOrigin {
        let s = &self.inner.transform_scope;
        if s.is_transform_origin_mutated.get() {
            let (x, y) = s.transform_origin.get();
            TransformOrigin::new(x, y)
        } else {
            TransformOrigin::Center
        }
    }

    fn current_millis(&self) -> i64 {
        let test = TEST_TIME_SOURCE.with(|t| t.borrow().clone());
        match test {
            Some(source) => source(),
            None => self.inner.start_time.elapsed().as_millis() as i64,
        }
    }

    /// The coordinates of the content's enter/exit layout (Compose's `parentLayoutCoordinates`).
    pub(crate) fn parent_layout_coordinates(&self) -> Option<LayoutCoordinates> {
        self.inner.parent_layout_coordinates.borrow().clone()
    }

    pub(crate) fn set_parent_layout_coordinates(&self, coordinates: LayoutCoordinates) {
        *self.inner.parent_layout_coordinates.borrow_mut() = Some(coordinates);
    }

    pub(crate) fn last_veil(&self) -> Color {
        self.inner.last_veil.get()
    }
    pub(crate) fn last_alpha(&self) -> f32 {
        self.inner.last_alpha.get()
    }
    pub(crate) fn last_scale(&self) -> f32 {
        self.inner.last_scale.get()
    }
    pub(crate) fn last_transform_origin(&self) -> TransformOrigin {
        self.inner.last_transform_origin.get()
    }
    pub(crate) fn last_slide(&self) -> IntOffset {
        self.inner.last_slide.get()
    }
    /// The scale the block itself last set (Compose's `lastManualScale`).
    pub(crate) fn last_manual_scale(&self) -> f32 {
        self.inner.last_manual_scale.get()
    }
    /// The offset the block itself last set (Compose's `lastManualSlide`).
    pub(crate) fn last_manual_slide(&self) -> IntOffset {
        self.inner.last_manual_slide.get()
    }

    pub(crate) fn veil_requires_animation(&self) -> bool {
        (self.has_block() && self.inner.transform_scope.is_veil_mutated.get()) || self.last_veil() != Color::Transparent
    }
    pub(crate) fn alpha_requires_animation(&self) -> bool {
        (self.has_block() && self.inner.transform_scope.is_alpha_mutated.get()) || self.last_alpha() != 1.0
    }
    pub(crate) fn scale_requires_animation(&self) -> bool {
        (self.has_block() && self.inner.transform_scope.is_scale_mutated.get()) || self.last_scale() != 1.0
    }
    /// Whether the transform origin needs animating (Compose's `transformOriginRequiresAnimation`).
    #[cfg_attr(not(test), expect(dead_code, reason = "Compose 1.12.1 declares it and reads it nowhere"))]
    pub(crate) fn transform_origin_requires_animation(&self) -> bool {
        (self.has_block() && self.inner.transform_scope.is_transform_origin_mutated.get())
            || self.last_transform_origin() != TransformOrigin::Center
    }
    pub(crate) fn slide_requires_animation(&self) -> bool {
        (self.has_block() && self.inner.transform_scope.is_offset_mutated.get()) || self.last_slide() != IntOffset::Zero
    }

    pub(crate) fn veil_handoff_value(&self) -> Option<Color> {
        self.is_handoff_active().then(|| self.last_veil())
    }
    pub(crate) fn alpha_handoff_value(&self) -> Option<f32> {
        self.is_handoff_active().then(|| self.last_alpha())
    }
    pub(crate) fn scale_handoff_value(&self) -> Option<f32> {
        self.is_handoff_active().then(|| self.last_scale())
    }
    pub(crate) fn transform_origin_handoff_value(&self) -> Option<TransformOrigin> {
        self.is_handoff_active().then(|| self.last_transform_origin())
    }
    pub(crate) fn slide_handoff_value(&self) -> Option<IntOffset> {
        self.is_handoff_active().then(|| self.last_slide())
    }

    /// The scale's velocity handed over (Compose's `scaleHandoffVelocity`).
    pub(crate) fn scale_handoff_velocity(&self) -> Option<AnimationVector1D> {
        if !self.is_handoff_active() {
            return None;
        }
        let v = self.inner.scale_velocity_tracker.borrow().as_ref().map(|t| t.calculate_velocity());
        Some(AnimationVector1D(v.filter(|v| !v.is_nan()).unwrap_or(0.0)))
    }

    /// The offset's velocity handed over (Compose's `slideHandoffVelocity`): the provider's, else the tracked one.
    pub(crate) fn slide_handoff_velocity(&self) -> Option<AnimationVector2D> {
        if !self.is_handoff_active() {
            return None;
        }
        let provided = self.inner.last_mutable_data.borrow().as_ref().and_then(|d| d.offset_velocity_provider());
        if let Some(v) = provided.map(|p| p()).filter(|v| v.is_specified()) {
            return Some(AnimationVector2D(v.x, v.y));
        }
        let vel = self.inner.offset_velocity_tracker.borrow().as_ref().map(|t| t.calculate_velocity());
        let (x, y) = vel.map_or((0.0, 0.0), |v| (v.x, v.y));
        Some(AnimationVector2D(if x.is_nan() { 0.0 } else { x }, if y.is_nan() { 0.0 } else { y }))
    }

    fn track_scale_velocity(&self, value: f32) {
        let now = self.current_millis();
        let mut tracker = self.inner.scale_velocity_tracker.borrow_mut();
        tracker.get_or_insert_with(|| VelocityTracker1D::new(false)).add_data_point(now, value);
    }

    fn track_slide_velocity(&self, value: IntOffset) {
        let now = self.current_millis();
        let mut tracker = self.inner.offset_velocity_tracker.borrow_mut();
        tracker.get_or_insert_with(VelocityTracker::new).add_position(now, Offset::new(value.x as f32, value.y as f32));
    }

    /// Runs the block while deferring (Compose's `evaluateTransformBlock`).
    pub(crate) fn evaluate_transform_block(&self, full_size: IntSize) {
        if self.is_mutating()
            && let Some(block) = self.mutable_data().and_then(|d| d.block())
        {
            block(&mut MutableTransformScopeRef(&self.inner.transform_scope), full_size);
        }
    }

    /// The alpha drawn: the transition's times the block's (Compose's `combinedAlpha`).
    pub(crate) fn combined_alpha(&self, transition_value: f32) -> f32 {
        let s = &self.inner.transform_scope;
        let mutating = self.is_mutating();
        let is_mutated = mutating && s.is_alpha_mutated.get();
        let combined = transition_value * if is_mutated { s.alpha.get() } else { 1.0 };
        if mutating {
            self.inner.last_alpha.set(combined);
        }
        combined
    }

    /// The scale drawn (Compose's `combinedScale`).
    pub(crate) fn combined_scale(&self, transition_value: f32) -> f32 {
        let s = &self.inner.transform_scope;
        let mutating = self.is_mutating();
        let is_mutated = mutating && s.is_scale_mutated.get();
        let combined = transition_value * if is_mutated { s.scale.get() } else { 1.0 };
        if mutating {
            self.inner.last_scale.set(combined);
            self.inner.last_manual_scale.set(if is_mutated { s.scale.get() } else { 1.0 });
            if is_mutated {
                self.track_scale_velocity(combined);
            }
        }
        combined
    }

    /// The transform origin drawn (Compose's `combinedTransformOrigin`).
    pub(crate) fn combined_transform_origin(&self, transition_value: TransformOrigin) -> TransformOrigin {
        let s = &self.inner.transform_scope;
        let mutating = self.is_mutating();
        let combined = if mutating && s.is_transform_origin_mutated.get() {
            let (x, y) = s.transform_origin.get();
            TransformOrigin::new(x, y)
        } else {
            transition_value
        };
        if mutating {
            self.inner.last_transform_origin.set(combined);
        }
        combined
    }

    /// The offset placed (Compose's `combinedSlide`): the block run first.
    pub(crate) fn combined_slide(&self, transition_value: IntOffset, full_size: IntSize) -> IntOffset {
        self.evaluate_transform_block(full_size);
        let s = &self.inner.transform_scope;
        let mutating = self.is_mutating();
        let is_mutated = mutating && s.is_offset_mutated.get();
        let combined = transition_value + if is_mutated { s.offset.get() } else { IntOffset::Zero };
        if mutating {
            self.inner.last_slide.set(combined);
            self.inner.last_manual_slide.set(if is_mutated { s.offset.get() } else { IntOffset::Zero });
            if is_mutated {
                self.track_slide_velocity(combined);
            }
        }
        combined
    }

    /// The veil drawn (Compose's `combinedVeil`).
    pub(crate) fn combined_veil(&self, transition_value: Color) -> Color {
        let s = &self.inner.transform_scope;
        let mutating = self.is_mutating();
        let combined = if mutating && s.is_veil_mutated.get() { s.veil.get() } else { transition_value };
        if mutating {
            self.inner.last_veil.set(combined);
        }
        combined
    }

    /// Back to no deferring (Compose's `clear`).
    pub(crate) fn clear(&self) {
        self.inner.is_handoff_active.set(false);
        self.set_is_mutating(false);
        // (the setter above may have started a hand-off)
        self.inner.is_handoff_active.set(false);
        self.inner.transform_scope.reset();
        self.inner.last_veil.set(Color::Transparent);
        self.inner.last_alpha.set(1.0);
        self.inner.last_scale.set(1.0);
        if let Some(t) = self.inner.scale_velocity_tracker.borrow_mut().as_mut() {
            t.reset_tracking();
        }
        self.inner.last_transform_origin.set(TransformOrigin::Center);
        self.inner.last_slide.set(IntOffset::Zero);
        self.inner.last_manual_scale.set(1.0);
        self.inner.last_manual_slide.set(IntOffset::Zero);
        if let Some(t) = self.inner.offset_velocity_tracker.borrow_mut().as_mut() {
            t.reset_tracking();
        }
        *self.inner.last_mutable_data.borrow_mut() = None;
        *self.inner.mutable_data.borrow_mut() = None;
    }
}

/// What a [`DeferredAnimatedContent`](crate::DeferredTransition::DeferredAnimatedContent)'s deferred phase does to
/// its two contents by hand (Compose's experimental MutableContentTransform): the content leaving
/// ([`initial_content_transform`](Self::initial_content_transform)) and the one coming in
/// ([`target_content_transform`](Self::target_content_transform)). Made by [`MutableContentTransform`].
#[derive(Clone)]
pub struct MutableContentTransform {
    pub(crate) target_transform: MutableTransform,
    pub(crate) initial_transform: MutableTransform,
}

impl PartialEq for MutableContentTransform {
    fn eq(&self, other: &Self) -> bool {
        self.target_transform == other.target_transform && self.initial_transform == other.initial_transform
    }
}

impl MutableContentTransform {
    /// The block of the content leaving (Compose's `initialContentTransform`).
    pub fn initial_content_transform(&self, block: impl Fn(&mut dyn MutableTransformScope, IntSize) + 'static) {
        self.initial_transform.update(block);
    }

    /// The block of the content coming in (Compose's `targetContentTransform`).
    pub fn target_content_transform(&self, block: impl Fn(&mut dyn MutableTransformScope, IntSize) + 'static) {
        self.target_transform.update(block);
    }
}

/// A [`MutableContentTransform`] set up by `block` (Compose's experimental `MutableContentTransform(…) { }`): veils
/// covering the contents' parent or not, the providers of the offsets' velocities handed over.
///
/// Kotlin: `MutableContentTransform(initialVeilMatchParentSize: Boolean = false, targetVeilMatchParentSize: Boolean = false, initialOffsetVelocityProvider: (() -> Offset)? = null, targetOffsetVelocityProvider: (() -> Offset)? = null, block: MutableContentTransform.() -> Unit = {})`
#[named(type)]
pub fn MutableContentTransform(
    #[default(false)] initial_veil_match_parent_size: bool,
    #[default(false)] target_veil_match_parent_size: bool,
    #[default(None)] initial_offset_velocity_provider: Option<Rc<dyn Fn() -> Offset>>,
    #[default(None)] target_offset_velocity_provider: Option<Rc<dyn Fn() -> Offset>>,
    #[default(|_: &MutableContentTransform| {})] block: impl FnOnce(&MutableContentTransform),
) -> MutableContentTransform {
    let transform = MutableContentTransform {
        target_transform: new_mutable_transform(target_veil_match_parent_size, target_offset_velocity_provider, None),
        initial_transform: new_mutable_transform(
            initial_veil_match_parent_size,
            initial_offset_velocity_provider,
            None,
        ),
    };
    block(&transform);
    transform
}
