//! A pane's bounds animated by the scaffold's motion progress, as Compose's adaptive-layout AnimateBoundsModifier.kt
//! (1.2.0, internal): an approach layout measuring and placing its layout, during the motion, at the bounds between
//! where it was and where it goes (its lookahead size and position) that the bounds spec gives at the progress.

use super::animate_modifier_utils::{Bounds, InvalidIntRect, convert_offset_to_current_coordinates, lookahead_offset};
use super::pane_motion::int_rect_to_vector;
use kompoz_animation::{Animation, AnimationVector4D, FiniteAnimationSpec, TargetBasedAnimation};
use kompoz_graphics::{IntOffset, IntRect, IntSize};
use kompoz_ui::layout::{
    ApproachLayoutModifierNode, ApproachMeasureScope, LayoutCoordinates, LayoutModifierNode, LookaheadScope,
    Measurable, MeasureResult, MeasureScope, PlacementScope,
};
use kompoz_ui::{Constraints, Modifier, ModifierNode, ModifierNodeElement, node_capabilities};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

/// `modifier` animating its layout's bounds by `animate_fraction` along `animation_spec` while `enabled`, offsets
/// converted by `scale_conversion` (the predictive back scale), in `lookahead_scope` (Compose's internal
/// `Modifier.animateBounds`).
pub(crate) fn animate_bounds(
    modifier: Modifier,
    animate_fraction: Rc<dyn Fn() -> f32>,
    animation_spec: Arc<dyn FiniteAnimationSpec<IntRect>>,
    scale_conversion: Rc<dyn Fn(IntOffset) -> IntOffset>,
    lookahead_scope: Rc<dyn LookaheadScope>,
    enabled: bool,
) -> Modifier {
    modifier.then_element(AnimateBoundsElement {
        animate_fraction,
        animation_spec,
        scale_conversion,
        lookahead_scope,
        enabled,
    })
}

/// Compose's `AnimateBoundsElement`: equal with the same lambdas (by identity), spec, scope and enabling.
struct AnimateBoundsElement {
    animate_fraction: Rc<dyn Fn() -> f32>,
    animation_spec: Arc<dyn FiniteAnimationSpec<IntRect>>,
    scale_conversion: Rc<dyn Fn(IntOffset) -> IntOffset>,
    lookahead_scope: Rc<dyn LookaheadScope>,
    enabled: bool,
}

impl PartialEq for AnimateBoundsElement {
    fn eq(&self, other: &Self) -> bool {
        self.enabled == other.enabled
            && Rc::ptr_eq(&self.animate_fraction, &other.animate_fraction)
            && *self.animation_spec == *other.animation_spec
            && Rc::ptr_eq(&self.scale_conversion, &other.scale_conversion)
            && Rc::ptr_eq(&self.lookahead_scope, &other.lookahead_scope)
    }
}

impl ModifierNodeElement for AnimateBoundsElement {
    type Node = AnimateBoundsNode;
    fn create(&self) -> AnimateBoundsNode {
        AnimateBoundsNode {
            animate_fraction: self.animate_fraction.clone(),
            scale_conversion: self.scale_conversion.clone(),
            lookahead_scope: self.lookahead_scope.clone(),
            enabled: self.enabled,
            bounds_tracker: Rc::new(BoundsTracker::new(self.animation_spec.clone())),
        }
    }
    fn update(&self, node: &mut AnimateBoundsNode) {
        node.animate_fraction = self.animate_fraction.clone();
        *node.bounds_tracker.animation_spec.borrow_mut() = self.animation_spec.clone();
        node.scale_conversion = self.scale_conversion.clone();
        node.lookahead_scope = self.lookahead_scope.clone();
        node.enabled = self.enabled;
    }
    fn name(&self) -> &'static str {
        "animateBounds"
    }
}

/// Compose's private `AnimateBoundsNode`.
struct AnimateBoundsNode {
    animate_fraction: Rc<dyn Fn() -> f32>,
    scale_conversion: Rc<dyn Fn(IntOffset) -> IntOffset>,
    lookahead_scope: Rc<dyn LookaheadScope>,
    enabled: bool,
    bounds_tracker: Rc<BoundsTracker>,
}

impl ModifierNode for AnimateBoundsNode {
    node_capabilities!(layout, approach_layout);
}

impl LayoutModifierNode for AnimateBoundsNode {
    /// Only called during lookahead: a "no-op" measuring that updates the target size and offset.
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = measurable.measure(constraints);
        let (width, height) = (placeable.width(), placeable.height());
        self.bounds_tracker.update_target_size(IntSize::new(width, height));
        let (tracker, lookahead_scope, enabled) =
            (self.bounds_tracker.clone(), self.lookahead_scope.clone(), self.enabled);
        scope.layout(width, height, move |p: &PlacementScope| {
            if p.coordinates().is_some() {
                tracker.update_target_offset(lookahead_offset(p, &*lookahead_scope));
                if !enabled {
                    tracker.update_and_get_current_bounds(1.0);
                }
                p.place_at(&placeable, IntOffset::Zero, 0.0);
            }
        })
    }
}

impl ApproachLayoutModifierNode for AnimateBoundsNode {
    fn is_measurement_approach_in_progress(&mut self, _lookahead_size: IntSize) -> bool {
        self.enabled && (self.animate_fraction)() != 1.0
    }

    fn is_placement_approach_in_progress(&mut self, _scope: &PlacementScope, _: &LayoutCoordinates) -> bool {
        self.enabled && (self.animate_fraction)() != 1.0
    }

    fn approach_measure(
        &mut self,
        scope: &ApproachMeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        // Workaround (b/435756530): Avoid approaching measure if the animation is not enabled.
        if !self.enabled {
            let placeable = measurable.measure(constraints);
            return scope.layout(placeable.width(), placeable.height(), move |p: &PlacementScope| {
                p.place_at(&placeable, IntOffset::Zero, 0.0);
            });
        }
        // The current animating fraction's size and offset toward the target ones updated in measure().
        let current_bounds = self.bounds_tracker.update_and_get_current_bounds((self.animate_fraction)());
        let placeable = measurable.measure(Constraints::fixed(current_bounds.width(), current_bounds.height()));
        let (scale_conversion, lookahead_scope) = (self.scale_conversion.clone(), self.lookahead_scope.clone());
        scope.layout(placeable.width(), placeable.height(), move |p: &PlacementScope| {
            if p.coordinates().is_some() {
                // Workaround(b/413692430): the offset converted by the parent's scaling (the predictive back
                // scale), the coordinates of the approach being transformed.
                let scaled_offset = scale_conversion(current_bounds.top_left());
                p.place_at(&placeable, convert_offset_to_current_coordinates(p, scaled_offset, &*lookahead_scope), 0.0);
            }
        })
    }
}

/// Where a pane was and goes, and where it is at a progress (Compose's private `BoundsTracker`).
struct BoundsTracker {
    animation_spec: RefCell<Arc<dyn FiniteAnimationSpec<IntRect>>>,
    origin: Cell<Bounds>,
    target: Cell<Bounds>,
    current: Cell<IntRect>,
    bounds_animation: RefCell<Option<TargetBasedAnimation<IntRect, AnimationVector4D>>>,
}

impl BoundsTracker {
    fn new(animation_spec: Arc<dyn FiniteAnimationSpec<IntRect>>) -> BoundsTracker {
        BoundsTracker {
            animation_spec: RefCell::new(animation_spec),
            origin: Cell::new(Bounds::default()),
            target: Cell::new(Bounds::default()),
            current: Cell::new(InvalidIntRect),
            bounds_animation: RefCell::new(None),
        }
    }

    fn update_target_size(&self, new_size: IntSize) {
        let mut target = self.target.get();
        if target.size == new_size {
            return;
        }
        // TODO(conradchen): Handle the interruption better when the target size changes during the animation
        let mut origin = self.origin.get();
        let current = self.current.get();
        origin.size = if current != InvalidIntRect { current.size() } else { new_size };
        self.origin.set(origin);
        target.size = new_size;
        self.target.set(target);
    }

    fn update_target_offset(&self, new_offset: IntOffset) {
        let mut target = self.target.get();
        if target.top_left == new_offset {
            return;
        }
        // TODO(conradchen): Handle the interruption better when the target position changes during the animation
        let mut origin = self.origin.get();
        let current = self.current.get();
        origin.top_left = if current != InvalidIntRect { current.top_left() } else { new_offset };
        self.origin.set(origin);
        target.top_left = new_offset;
        self.target.set(target);
    }

    fn update_and_get_current_bounds(&self, fraction: f32) -> IntRect {
        let current = if fraction == 0.0 {
            self.origin.get().rect()
        } else if fraction == 1.0 {
            self.target.get().rect()
        } else {
            self.update_bounds_animation_if_needed();
            self.bounds_animation.borrow().as_ref().map_or(InvalidIntRect, |animation| {
                animation.get_value_from_nanos((animation.duration_nanos() as f64 * fraction as f64) as i64)
            })
        };
        self.current.set(current);
        current
    }

    fn update_bounds_animation_if_needed(&self) {
        let (origin, target) = (self.origin.get(), self.target.get());
        if !origin.is_valid() || !target.is_valid() {
            return;
        }
        let stale = self
            .bounds_animation
            .borrow()
            .as_ref()
            .is_none_or(|a| a.initial_value() != origin.rect() || a.target_value() != target.rect());
        if stale {
            let spec = self.animation_spec.borrow().clone();
            *self.bounds_animation.borrow_mut() =
                Some(TargetBasedAnimation::new(&*spec, int_rect_to_vector(), origin.rect(), target.rect(), None));
        }
    }
}
