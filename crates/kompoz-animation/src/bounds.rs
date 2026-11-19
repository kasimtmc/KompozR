//! `Modifier.animate_bounds`, as Compose's `AnimateBoundsModifier.kt`, and [`BoundsTransform`] (Compose's fun
//! interface of `SharedTransitionScope.kt`): a layout in a [`LookaheadScope`] that glides — its size and its place in
//! the scope — from where it was to where the lookahead pass puts it, along the spec its bounds transform gives for
//! each change.

use crate::converter::fast_round_to_int;
use crate::lookahead_debug::{
    IS_LOOKAHEAD_ANIMATION_VISUAL_DEBUGGING_ENABLED, LocalLookaheadAnimationVisualDebugColor,
    LocalLookaheadAnimationVisualDebugConfig, LookaheadAnimationVisualDebugHelper,
};
use crate::{Animatable, AnimationSpec, AnimationVector4D, FiniteAnimationSpec, Spring, VectorConverter};
use crate::{VisibilityThreshold, spring};
use kompoz_coroutines::{CoroutineScope, CoroutineStart};
use kompoz_graphics::{Dp, IntSize, LayoutDirection, Offset, Rect, Size};
use kompoz_runtime::{MutableState, mutable_state_of, named, named_args};
use kompoz_text::{FontFamilyResolver, TextMeasurer};
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode};
use kompoz_ui::layout::{ApproachLayoutModifierNode, ApproachMeasureScope, LayoutCoordinates, LayoutModifierNode};
use kompoz_ui::layout::{Measurable, MeasureResult, MeasureScope, PlacementScope, approach_lookahead_measure};
use kompoz_ui::node_capabilities;
use kompoz_ui::platform::LocalFontFamilyResolver;
use kompoz_ui::{Constraints, LookaheadScope, Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------
// BoundsTransform

/// What spec an animation of bounds goes along, given where it starts and where it is going (Compose's fun interface
/// `BoundsTransform`): `animate_bounds` (and shared elements) ask it as each animation starts. A closure
/// `|initial, target| spec` is one; [`BoundsTransform`]`(…)` makes one of a closure as Kotlin's SAM constructor does.
pub trait BoundsTransform {
    /// The spec of an animation from `initial_bounds` to `target_bounds` (Compose's `createAnimationSpec`).
    fn create_animation_spec(&self, initial_bounds: Rect, target_bounds: Rect) -> Arc<dyn FiniteAnimationSpec<Rect>>;
}

impl<F: Fn(Rect, Rect) -> S, S: FiniteAnimationSpec<Rect>> BoundsTransform for F {
    fn create_animation_spec(&self, initial_bounds: Rect, target_bounds: Rect) -> Arc<dyn FiniteAnimationSpec<Rect>> {
        Arc::new(self(initial_bounds, target_bounds))
    }
}

/// A bounds transform of a closure (Kotlin's SAM constructor `BoundsTransform { initial, target -> spec }`), shared.
///
/// Kotlin: `BoundsTransform(block: (initialBounds: Rect, targetBounds: Rect) -> FiniteAnimationSpec<Rect>)`
pub fn BoundsTransform<S: FiniteAnimationSpec<Rect>>(
    block: impl Fn(Rect, Rect) -> S + 'static,
) -> Rc<dyn BoundsTransform> {
    Rc::new(block)
}

thread_local! {
    /// The one default bounds transform (so the elements given it stay equal).
    static DEFAULT_BOUNDS_TRANSFORM: Rc<dyn BoundsTransform> = Rc::new(|_: Rect, _: Rect| {
        named_args!(spring(
            damping_ratio = Spring::DampingRatioNoBouncy,
            stiffness = Spring::StiffnessMediumLow,
            visibility_threshold = Some(Rect::VisibilityThreshold)
        ))
    });
}

/// The bounds transform `animate_bounds` (and shared elements) use unless given one (Compose's private
/// `DefaultBoundsTransform`): a spring that doesn't bounce, medium-low stiff, there within a pixel each way.
pub(crate) fn DefaultBoundsTransform() -> Rc<dyn BoundsTransform> {
    DEFAULT_BOUNDS_TRANSFORM.with(|t| t.clone())
}

// ---------------------------------------------------------------------------------------------------------------
// The modifier

/// The modifiers animating a layout's bounds in a lookahead scope (Compose's `Modifier.animateBounds`).
#[named(trait BoundsModifiers)]
impl Modifier {
    /// Animates the layout's changes of size and of place in `lookahead_scope` (Compose's `animateBounds`): the
    /// scope's coordinates are what a change is seen in — a layout that moves with the whole scope (the scope
    /// scrolled) doesn't animate. Each change starts an animation from where it is shown along the spec
    /// `bounds_transform` gives for its start and end; the size along the way is the layout's (its parent sees it),
    /// and what comes after it is measured at it. A layout modifier that would change its parent or its content at
    /// once can be given as `modifier`, between the two: the change then reaches both gradually. Changes of position
    /// under a motion frame of reference (a scroll's) are applied at once unless `animate_motion_frame_of_reference`
    /// (then a layout in a scroller chases the scroll). Keep `lookahead_scope` the same while it animates.
    ///
    /// Kotlin: `animateBounds(lookaheadScope: LookaheadScope, modifier: Modifier = Modifier, boundsTransform: BoundsTransform = DefaultBoundsTransform, animateMotionFrameOfReference: Boolean = false)`
    fn animate_bounds(
        self,
        lookahead_scope: Rc<dyn LookaheadScope>,
        #[default(Modifier)] modifier: Modifier,
        #[default(DefaultBoundsTransform())] bounds_transform: Rc<dyn BoundsTransform>,
        #[default(false)] animate_motion_frame_of_reference: bool,
    ) -> Modifier {
        self.then_element(BoundsAnimationElement {
            lookahead_scope: lookahead_scope.clone(),
            bounds_transform: bounds_transform.clone(),
            // (measured with the constraints given: its layout still the animated lookahead size)
            resolve_measure_constraints: incoming_constraints,
            animate_motion_frame_of_reference,
        })
        .then(modifier)
        .then_element(BoundsAnimationElement {
            lookahead_scope,
            bounds_transform,
            // (what it wraps measured at the animated size)
            resolve_measure_constraints: animated_size_constraints,
            animate_motion_frame_of_reference,
        })
    }
}

/// The outer node measures with the constraints it is given.
fn incoming_constraints(_animated_size: IntSize, constraints: Constraints) -> Constraints {
    constraints
}

/// The inner one with the animated size, fixed.
fn animated_size_constraints(animated_size: IntSize, _constraints: Constraints) -> Constraints {
    Constraints::fixed(animated_size.width, animated_size.height)
}

/// How a bounds animation node chooses the constraints it measures with (Compose's `onChooseMeasureConstraints`).
type ChooseMeasureConstraints = fn(IntSize, Constraints) -> Constraints;

/// `animate_bounds`' element (Compose's BoundsAnimationElement): equal with the same scope, bounds transform and
/// choice of constraints.
struct BoundsAnimationElement {
    lookahead_scope: Rc<dyn LookaheadScope>,
    bounds_transform: Rc<dyn BoundsTransform>,
    resolve_measure_constraints: ChooseMeasureConstraints,
    animate_motion_frame_of_reference: bool,
}

impl PartialEq for BoundsAnimationElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.lookahead_scope, &other.lookahead_scope)
            && Rc::ptr_eq(&self.bounds_transform, &other.bounds_transform)
            && std::ptr::fn_addr_eq(self.resolve_measure_constraints, other.resolve_measure_constraints)
            && self.animate_motion_frame_of_reference == other.animate_motion_frame_of_reference
    }
}

impl ModifierNodeElement for BoundsAnimationElement {
    type Node = BoundsAnimationModifierNode;
    fn create(&self) -> BoundsAnimationModifierNode {
        BoundsAnimationModifierNode {
            lookahead_scope: self.lookahead_scope.clone(),
            bounds_transform: self.bounds_transform.clone(),
            on_choose_measure_constraints: self.resolve_measure_constraints,
            animate_motion_frame_of_reference: self.animate_motion_frame_of_reference,
            direct_manipulation_parents_dirty: true,
            bounds_animation: Rc::new(BoundsTransformDeferredAnimation::new()),
            text_measurer: None,
            current_resolver: None,
            current_density: None,
            current_layout_direction: None,
            handle: None,
        }
    }
    fn update(&self, node: &mut BoundsAnimationModifierNode) {
        node.lookahead_scope = self.lookahead_scope.clone();
        node.bounds_transform = self.bounds_transform.clone();
        node.on_choose_measure_constraints = self.resolve_measure_constraints;
        node.animate_motion_frame_of_reference = self.animate_motion_frame_of_reference;
    }
    fn name(&self) -> &'static str {
        "boundsAnimation"
    }
}

/// `animate_bounds`' node (Compose's BoundsAnimationModifierNode), an approach layout: its measurement approaches its
/// lookahead size, its placement its lookahead position in the scope, both along the bounds animation.
struct BoundsAnimationModifierNode {
    lookahead_scope: Rc<dyn LookaheadScope>,
    bounds_transform: Rc<dyn BoundsTransform>,
    on_choose_measure_constraints: ChooseMeasureConstraints,
    animate_motion_frame_of_reference: bool,
    /// Whether the layouts placing under a motion frame of reference above it are to be found again.
    direct_manipulation_parents_dirty: bool,
    bounds_animation: Rc<BoundsTransformDeferredAnimation>,
    /// What writes the key while debugging, made with the font resolver, density and direction it was then.
    text_measurer: Option<TextMeasurer>,
    current_resolver: Option<Rc<dyn FontFamilyResolver>>,
    current_density: Option<kompoz_graphics::Density>,
    current_layout_direction: Option<LayoutDirection>,
    handle: Option<NodeHandle>,
}

impl ModifierNode for BoundsAnimationModifierNode {
    node_capabilities!(layout, approach_layout, draw);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        self.direct_manipulation_parents_dirty = true;
    }
}

impl LayoutModifierNode for BoundsAnimationModifierNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        approach_lookahead_measure(scope, measurable, constraints)
    }
}

impl ApproachLayoutModifierNode for BoundsAnimationModifierNode {
    fn is_measurement_approach_in_progress(&mut self, lookahead_size: IntSize) -> bool {
        // (the target size: whether an approach is expected)
        self.bounds_animation.update_target_size(lookahead_size.to_size());
        !self.bounds_animation.is_idle()
    }

    fn is_placement_approach_in_progress(
        &mut self,
        scope: &PlacementScope,
        _lookahead_coordinates: &LayoutCoordinates,
    ) -> bool {
        if IS_LOOKAHEAD_ANIMATION_VISUAL_DEBUGGING_ENABLED && self.bounds_animation.debug_helper.borrow().is_none() {
            *self.bounds_animation.debug_helper.borrow_mut() =
                Some(Rc::new(LookaheadAnimationVisualDebugHelper::new()));
        }
        // (with the size and the offset known, an animation may start)
        if let Some(coroutine_scope) = self.handle.as_ref().and_then(|h| h.coroutine_scope()) {
            self.bounds_animation.update_target_offset_and_animate(
                &*self.lookahead_scope,
                scope,
                &coroutine_scope,
                self.direct_manipulation_parents_dirty,
                self.animate_motion_frame_of_reference,
                &*self.bounds_transform,
            );
        }
        self.direct_manipulation_parents_dirty = self.animate_motion_frame_of_reference;
        !self.bounds_animation.is_idle()
    }

    fn approach_measure(
        &mut self,
        scope: &ApproachMeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let animation = &self.bounds_animation;
        // (no animated value before the first placement, which knows the bounds: the current size; measured for
        // intrinsics before the approach check, the lookahead size)
        let current_size = animation.current_size.get();
        let fallback_size = if current_size.is_unspecified() { scope.lookahead_size().to_size() } else { current_size };
        let animated = animation.value().map_or(fallback_size, |r| r.size());
        let animated_size = round_to_int_size(animated);
        let chosen_constraints = (self.on_choose_measure_constraints)(animated_size, constraints);
        let placeable = measurable.measure(chosen_constraints);
        // (within the chosen constraints: its parent doesn't center it, which wouldn't animate)
        let size = chosen_constraints.constrain_size(animated_size);
        let (lookahead_scope, animation, include_mfr) =
            (self.lookahead_scope.clone(), self.bounds_animation.clone(), self.animate_motion_frame_of_reference);
        scope.layout(size.width, size.height, move |p: &PlacementScope| {
            let animated_bounds = animation.value();
            let position_in_scope = p.coordinates().map(|coordinates| {
                lookahead_scope.lookahead_scope_coordinates(p).local_position_of_with(
                    &coordinates,
                    Offset::Zero,
                    include_mfr,
                )
            });
            let top_left = match animated_bounds {
                Some(bounds) => {
                    animation.update_current_bounds(bounds.top_left(), bounds.size());
                    bounds.top_left()
                }
                None => animation.current_bounds().map_or(Offset::Zero, |b| b.top_left()),
            };
            let at = position_in_scope.map_or(Offset::Zero, |position| top_left - position);
            p.place(&placeable, fast_round_to_int(at.x), fast_round_to_int(at.y));
        })
    }
}

impl DrawModifierNode for BoundsAnimationModifierNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        scope.draw_content();
        if !IS_LOOKAHEAD_ANIMATION_VISUAL_DEBUGGING_ENABLED {
            return;
        }
        // (drawn by the inner node only: the one measuring at the animated size)
        let is_inner_node =
            (self.on_choose_measure_constraints)(IntSize::Zero, Constraints::default()).has_fixed_width();
        if !is_inner_node {
            return;
        }
        let Some(handle) = self.handle.clone() else { return };
        let config = handle.current_value_of(&LocalLookaheadAnimationVisualDebugConfig);
        if !config.is_enabled {
            return;
        }
        if self.current_density.is_none() {
            self.current_density = Some(handle.density());
            self.current_layout_direction = Some(handle.layout_direction());
        }
        let Some(helper) = self.bounds_animation.debug_helper.borrow().clone() else { return };
        self.update_text_measurer(handle.current_value_of(&LocalFontFamilyResolver));
        let stroke_width = scope.density().to_px(Dp(2.5));
        let key = self.bounds_animation.key();
        let animation = &self.bounds_animation;
        if animation.is_idle() {
            helper.draw_inactive_visualizations(
                scope,
                config.inactive_element_color,
                config.is_show_key_label_enabled,
                stroke_width,
                &key,
                self.text_measurer.as_ref(),
            );
        } else if let Some(value) = animation.value() {
            let center = scope.size().center();
            helper.draw_local_visualizations(
                scope,
                handle.current_value_of(&LocalLookaheadAnimationVisualDebugColor),
                animation.target_offset.get(),
                animation.target_size.get(),
                value,
                center,
                config.is_show_key_label_enabled,
                stroke_width,
                &key,
                self.text_measurer.as_ref(),
            );
        }
    }
}

impl BoundsAnimationModifierNode {
    /// The text measurer for the key, made again with another font resolver (Compose's `updateTextMeasurer`).
    fn update_text_measurer(&mut self, font_family_resolver: Rc<dyn FontFamilyResolver>) {
        let stale = self.current_resolver.as_ref().is_none_or(|r| !Rc::ptr_eq(r, &font_family_resolver));
        if self.text_measurer.is_none() || stale {
            self.text_measurer = Some(TextMeasurer::new(
                font_family_resolver.clone(),
                self.current_density.unwrap_or_default(),
                self.current_layout_direction.unwrap_or(LayoutDirection::Ltr),
                kompoz_text::DefaultCacheSize,
            ));
            self.current_resolver = Some(font_family_resolver);
        }
    }
}

/// A size rounded to whole pixels, each way half up (Compose's `Size.roundToIntSize`).
fn round_to_int_size(size: Size) -> IntSize {
    IntSize::new(fast_round_to_int(size.width), fast_round_to_int(size.height))
}

/// An offset rounded to whole pixels, each way half up (Compose's `Offset.round`).
fn round_offset(offset: Offset) -> (i32, i32) {
    (fast_round_to_int(offset.x), fast_round_to_int(offset.y))
}

// ---------------------------------------------------------------------------------------------------------------
// The animation

/// The bounds animation of an approach node (Compose's BoundsTransformDeferredAnimation): its target — the lookahead
/// size, then the lookahead position in the scope — captured as the approach is asked, an animation started from
/// where it is shown when it changes; the offsets of the layouts above it placing under a motion frame of reference
/// (scrollers) kept apart, so scrolling isn't animated.
pub(crate) struct BoundsTransformDeferredAnimation {
    animatable: RefCell<Option<Animatable<Rect, AnimationVector4D>>>,
    target_size: Cell<Size>,
    target_offset: Cell<Offset>,
    is_pending: Cell<bool>,
    pub(crate) debug_helper: RefCell<Option<Rc<LookaheadAnimationVisualDebugHelper>>>,
    /// Where it is shown (Compose's `currentPosition`, `currentSize`: unspecified before the first pass).
    current_position: Cell<Offset>,
    current_size: Cell<Size>,
    /// Its value while it animates (Compose's `animatedValue`, a state: its readers follow it).
    animated_value: MutableState<Option<Rect>>,
    /// The layouts above it placing under a motion frame of reference, and the offset they add up to.
    direct_manipulation_parents: RefCell<Vec<LayoutCoordinates>>,
    additional_offset: Cell<Offset>,
}

impl BoundsTransformDeferredAnimation {
    pub(crate) fn new() -> BoundsTransformDeferredAnimation {
        BoundsTransformDeferredAnimation {
            animatable: RefCell::new(None),
            target_size: Cell::new(Size::Unspecified),
            target_offset: Cell::new(Offset::Unspecified),
            is_pending: Cell::new(false),
            debug_helper: RefCell::new(None),
            current_position: Cell::new(Offset::Unspecified),
            current_size: Cell::new(Size::Unspecified),
            animated_value: mutable_state_of(None),
            direct_manipulation_parents: RefCell::new(Vec::new()),
            additional_offset: Cell::new(Offset::Zero),
        }
    }

    /// The key its debugging writes (Compose's `toString().substring(60)`, the object's hash: here its address).
    fn key(&self) -> String {
        format!("{:x}", self as *const Self as usize)
    }

    /// Takes the lookahead size: a new one makes an animation pending; the first is where it starts (Compose's
    /// `updateTargetSize`).
    pub(crate) fn update_target_size(&self, size: Size) {
        let target = self.target_size.get();
        if target.is_specified() && round_to_int_size(size) != round_to_int_size(target) {
            self.is_pending.set(true);
        }
        self.target_size.set(size);
        if self.current_size.get().is_unspecified() {
            self.current_size.set(size);
        }
    }

    /// Takes the lookahead position: a new one makes an animation pending; the first is where it starts (Compose's
    /// `updateTargetOffset`).
    fn update_target_offset(&self, offset: Offset) {
        let target = self.target_offset.get();
        if target.is_specified() && round_offset(offset) != round_offset(target) {
            self.is_pending.set(true);
        }
        self.target_offset.set(offset);
        if self.current_position.get().is_unspecified() {
            self.current_position.set(offset);
        }
    }

    /// Where it is shown, once it was (Compose's `currentBounds`).
    pub(crate) fn current_bounds(&self) -> Option<Rect> {
        let (size, position) = (self.current_size.get(), self.current_position.get());
        (position.is_specified() && size.is_specified()).then(|| Rect::from_offset_size(position, size))
    }

    pub(crate) fn update_current_bounds(&self, position: Offset, size: Size) {
        self.current_position.set(position);
        self.current_size.set(size);
    }

    /// Whether it rests (Compose's `isIdle`): nothing pending, not running.
    pub(crate) fn is_idle(&self) -> bool {
        !self.is_pending.get() && !self.animatable.borrow().as_ref().is_some_and(|a| a.is_running())
    }

    /// Its value while it animates (Compose's `value`), none at rest.
    pub(crate) fn value(&self) -> Option<Rect> {
        if self.is_idle() { None } else { self.animated_value.get() }
    }

    /// Takes the lookahead position in the scope from a placement, and animates to the target when it changed
    /// (Compose's `updateTargetOffsetAndAnimate`).
    pub(crate) fn update_target_offset_and_animate(
        &self,
        lookahead_scope: &dyn LookaheadScope,
        placement_scope: &PlacementScope,
        coroutine_scope: &CoroutineScope,
        direct_manipulation_parents_dirty: bool,
        include_motion_frame_of_reference: bool,
        bounds_transform: &dyn BoundsTransform,
    ) {
        let Some(coordinates) = placement_scope.coordinates() else { return };
        let lookahead_scope_coordinates = lookahead_scope.lookahead_scope_coordinates(placement_scope);
        let mut delta = Offset::Zero;
        if !include_motion_frame_of_reference && direct_manipulation_parents_dirty {
            // (the offsets the layouts up to the scope placing under a motion frame of reference add, kept as the
            // layout changes: what scrolling moves it by)
            let mut parents = self.direct_manipulation_parents.borrow_mut();
            let mut current = coordinates.clone();
            let mut index = 0;
            while lookahead_scope.to_lookahead_coordinates(&current) != lookahead_scope_coordinates {
                if current.introduces_motion_frame_of_reference() {
                    if parents.len() == index {
                        delta = delta + current.position_in_parent();
                        parents.push(current.clone());
                    } else if parents[index] != current {
                        delta = delta - parents[index].position_in_parent();
                        parents[index] = current.clone();
                        delta = delta + current.position_in_parent();
                    }
                    index += 1;
                }
                match current.parent_coordinates() {
                    Some(parent) => current = parent,
                    None => break,
                }
            }
            while parents.len() > index {
                let last = parents.pop().expect("a parent past the index");
                delta = delta - last.position_in_parent();
            }
        }
        self.additional_offset.set(self.additional_offset.get() + delta);
        let target_offset = lookahead_scope.local_lookahead_position_of(
            &lookahead_scope_coordinates,
            &coordinates,
            Offset::Zero,
            include_motion_frame_of_reference,
        );
        self.update_target_offset(target_offset + self.additional_offset.get());
        let animated = self.animate(coroutine_scope, bounds_transform).translate(-self.additional_offset.get());
        self.animated_value.set(Some(animated));
    }

    /// Starts an animation to the target when one is pending (not on the first pass: the lookahead bounds are where
    /// it starts); its value now (Compose's `animate`).
    fn animate(&self, coroutine_scope: &CoroutineScope, bounds_transform: &dyn BoundsTransform) -> Rect {
        let (target_offset, target_size) = (self.target_offset.get(), self.target_size.get());
        if target_offset.is_specified() && target_size.is_specified() {
            // (made as soon as it can be: the first lookahead bounds are where an animation starts)
            let target = Rect::from_offset_size(target_offset, target_size);
            let anim = self
                .animatable
                .borrow_mut()
                .get_or_insert_with(|| Animatable::new(target, Rect::VectorConverter, None, "Animatable"))
                .clone();
            if self.is_pending.get() {
                self.is_pending.set(false);
                let current = self.current_bounds().expect("kompoz: a bounds animation pending without its bounds");
                let spec = bounds_transform.create_animation_spec(current, target);
                if IS_LOOKAHEAD_ANIMATION_VISUAL_DEBUGGING_ENABLED
                    && let Some(helper) = self.debug_helper.borrow().as_ref()
                {
                    helper.calculate_path(&spec, current, target, anim.velocity());
                }
                // (started at once, so the approach checks know it runs)
                let spec: Arc<dyn AnimationSpec<Rect>> = spec;
                let running = anim.clone();
                named_args!(coroutine_scope.launch(start = CoroutineStart::UNDISPATCHED, async move {
                    named_args!(running.animate_to(target, animation_spec = spec)).await;
                }));
            }
            return anim.value();
        }
        self.animatable.borrow().as_ref().map_or(Rect::Zero, |a| a.value())
    }
}
