//! A layout fading out where it was and in where it goes as the scaffold moves, as Compose's internal
//! AnimateWithFadingModifier.kt (adaptive-layout 1.2.0): [`animate_with_fading`].

use super::animate_modifier_utils::{InvalidOffset, lookahead_offset};
use kompoz_animation::{
    Animation, AnimationVector1D, FiniteAnimationSpec, TargetBasedAnimation, TweenSpec, VectorConverter, tween,
};
use kompoz_graphics::{Constraints, IntOffset, IntSize};
use kompoz_runtime::named_args;
use kompoz_ui::layout::{
    ApproachLayoutModifierNode, ApproachMeasureScope, LayoutCoordinates, LayoutModifierNode, LookaheadScope,
    Measurable, MeasureResult, MeasureScope, PlacementScope, approach_lookahead_measure,
};
use kompoz_ui::modifier::{ModifierNode, ModifierNodeElement};
use kompoz_ui::{Modifier, node_capabilities};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

/// `modifier` fading the layout as it moves to another place in `lookahead_scope` while `animate_fraction` goes from
/// 0 to 1, when `enabled` (Compose's internal `Modifier.animateWithFading`): the first half of the
/// `fading_animation_spec` (by default a tween) fades it out where it was, the second fades it in where it goes.
pub(crate) fn animate_with_fading(
    modifier: Modifier,
    enabled: bool,
    animate_fraction: Rc<dyn Fn() -> f32>,
    lookahead_scope: Rc<dyn LookaheadScope>,
    fading_animation_spec: Option<Arc<dyn FiniteAnimationSpec<f32>>>,
) -> Modifier {
    let fading_animation_spec = fading_animation_spec.unwrap_or_else(|| {
        let spec: TweenSpec<f32> = named_args!(tween());
        Arc::new(spec)
    });
    modifier.then_element(AnimateWithFadingElement {
        animate_fraction,
        lookahead_scope,
        enabled,
        fading_animation_spec,
    })
}

struct AnimateWithFadingElement {
    animate_fraction: Rc<dyn Fn() -> f32>,
    lookahead_scope: Rc<dyn LookaheadScope>,
    enabled: bool,
    fading_animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
}

/// Compose's `equals`: the same fraction lambda and lookahead scope (by identity), `enabled` and spec equal.
impl PartialEq for AnimateWithFadingElement {
    fn eq(&self, other: &Self) -> bool {
        self.enabled == other.enabled
            && Rc::ptr_eq(&self.animate_fraction, &other.animate_fraction)
            && std::ptr::addr_eq(Rc::as_ptr(&self.lookahead_scope), Rc::as_ptr(&other.lookahead_scope))
            && *self.fading_animation_spec == *other.fading_animation_spec
    }
}

impl ModifierNodeElement for AnimateWithFadingElement {
    type Node = AnimateWithFadingNode;

    fn create(&self) -> AnimateWithFadingNode {
        AnimateWithFadingNode {
            animate_fraction: self.animate_fraction.clone(),
            lookahead_scope: self.lookahead_scope.clone(),
            enabled: self.enabled,
            offsets: Rc::new(Cell::new((InvalidOffset, InvalidOffset))),
            animation: create_animation(&*self.fading_animation_spec),
            fading_animation_spec: self.fading_animation_spec.clone(),
        }
    }

    fn update(&self, node: &mut AnimateWithFadingNode) {
        node.animate_fraction = self.animate_fraction.clone();
        node.lookahead_scope = self.lookahead_scope.clone();
        node.enabled = self.enabled;
        node.set_fading_animation_spec(self.fading_animation_spec.clone());
    }

    fn name(&self) -> &'static str {
        "animateWithFading"
    }
}

/// The node (Compose's private `AnimateWithFadingNode`): an approach layout placing what it wraps with an alpha.
struct AnimateWithFadingNode {
    animate_fraction: Rc<dyn Fn() -> f32>,
    lookahead_scope: Rc<dyn LookaheadScope>,
    enabled: bool,
    /// The original and the target offsets (Compose's `originalOffset` and `targetOffset`), shared with the
    /// placement block, which reads them when it runs as Compose's does.
    offsets: Rc<Cell<(IntOffset, IntOffset)>>,
    fading_animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    animation: Rc<TargetBasedAnimation<f32, AnimationVector1D>>,
}

/// The fade from -1 to 1 along `spec` (Compose's private `FiniteAnimationSpec<Float>.createAnimation`): its value's
/// magnitude the alpha, its sign which place it is shown at.
fn create_animation(spec: &dyn FiniteAnimationSpec<f32>) -> Rc<TargetBasedAnimation<f32, AnimationVector1D>> {
    Rc::new(TargetBasedAnimation::new(spec, f32::VectorConverter, -1.0, 1.0, None))
}

impl AnimateWithFadingNode {
    fn set_fading_animation_spec(&mut self, value: Arc<dyn FiniteAnimationSpec<f32>>) {
        self.animation = create_animation(&*value);
        self.fading_animation_spec = value;
    }

    fn update_target_offset(&mut self, new_offset: IntOffset) {
        let (_, target_offset) = self.offsets.get();
        if target_offset == new_offset {
            return;
        }
        self.offsets.set((target_offset, new_offset));
    }
}

impl ModifierNode for AnimateWithFadingNode {
    node_capabilities!(layout, approach_layout);
}

impl LayoutModifierNode for AnimateWithFadingNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        approach_lookahead_measure(scope, measurable, constraints)
    }
}

impl ApproachLayoutModifierNode for AnimateWithFadingNode {
    fn is_measurement_approach_in_progress(&mut self, _lookahead_size: IntSize) -> bool {
        false
    }

    fn is_placement_approach_in_progress(
        &mut self,
        scope: &PlacementScope,
        _lookahead_coordinates: &LayoutCoordinates,
    ) -> bool {
        self.update_target_offset(lookahead_offset(scope, &*self.lookahead_scope));
        let (original_offset, target_offset) = self.offsets.get();
        self.enabled
            && original_offset != InvalidOffset
            && original_offset != target_offset
            && (self.animate_fraction)() != 1.0
    }

    fn approach_measure(
        &mut self,
        scope: &ApproachMeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let current_animated_value = animation_value(&self.animation, (self.animate_fraction)());
        let placeable = measurable.measure(constraints);
        let offsets = self.offsets.clone();
        scope.layout(placeable.width(), placeable.height(), move |p: &PlacementScope| {
            if p.coordinates().is_some() {
                // (Kotlin's Int arithmetic wraps: an offset not known yet subtracts as it does there.)
                let (original_offset, target_offset) = offsets.get();
                let at = if current_animated_value > 0.0 {
                    IntOffset::Zero
                } else {
                    IntOffset::new(
                        original_offset.x.wrapping_sub(target_offset.x),
                        original_offset.y.wrapping_sub(target_offset.y),
                    )
                };
                let alpha = current_animated_value.abs();
                named_args!(p.place_with_layer(&placeable, at.x, at.y, layer_block = move |layer| layer.alpha = alpha));
            }
        })
    }
}

/// The animation's value `progress` of the way through it (Compose's private `TargetBasedAnimation.getValue`).
fn animation_value(animation: &TargetBasedAnimation<f32, AnimationVector1D>, progress: f32) -> f32 {
    animation.get_value_from_nanos((animation.duration_nanos() as f32 * progress) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fade_goes_from_minus_one_through_zero_to_one() {
        let spec: TweenSpec<f32> = named_args!(tween(easing = Arc::new(kompoz_animation::LinearEasing)));
        let animation = create_animation(&spec);
        assert_eq!(animation_value(&animation, 0.0), -1.0);
        assert!(animation_value(&animation, 0.5).abs() < 1e-4);
        assert_eq!(animation_value(&animation, 1.0), 1.0);
    }
}
