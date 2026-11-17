//! The snapping fling (Compose's SnapFlingBehavior.kt and TargetedFlingBehavior.kt): an approach — a decay when
//! the fling is fast enough to cover the approach offset, else an animation to it along the snap spec — then an
//! animation to the snap offset, along the snap spec, from the velocity the approach left; what is left to go is
//! told as it goes.

use super::SnapLayoutInfoProvider;
use crate::gestures::fling::{TargetedFlingBehavior, default_scroll_motion_duration_scale};
use crate::gestures::{FlingBehavior, LocalFuture, ScrollScope};
use kompoz_animation::{
    AnimationScope, AnimationSpec, AnimationState, AnimationVector1D, DecayAnimationSpec, FloatDecayTarget, Spring,
    SpringSpec, float_animation_state, remember_spline_based_decay,
};
use kompoz_coroutines::with_context;
use kompoz_graphics::{Density, Dp};
use kompoz_runtime::{kompozable, named_args, remember_keyed};
use kompoz_ui::MotionDurationScale;
use kompoz_ui::platform::LocalDensity;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

/// A fling below this goes to the closest item; above it, to the next (or previous) one.
pub(crate) const MinFlingVelocityDp: Dp = Dp(400.0);
const NoDistance: f32 = 0.0;
const NoVelocity: f32 = 0.0;

/// A fling that snaps (Compose's `snapFlingBehavior`): see the module's doc. `decay_animation_spec` approaches
/// fast flings, `snap_animation_spec` approaches slow ones and snaps.
pub fn snap_fling_behavior(
    snap_layout_info_provider: Rc<dyn SnapLayoutInfoProvider>,
    decay_animation_spec: Arc<dyn DecayAnimationSpec<f32>>,
    snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
) -> Rc<dyn TargetedFlingBehavior> {
    Rc::new(SnapFlingBehavior::new(snap_layout_info_provider, decay_animation_spec, snap_animation_spec))
}

/// Compose's SnapFlingBehavior. Equal to another of the same provider and specs.
pub struct SnapFlingBehavior {
    snap_layout_info_provider: Rc<dyn SnapLayoutInfoProvider>,
    decay_animation_spec: Arc<dyn DecayAnimationSpec<f32>>,
    snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
    motion_scale_duration: MotionDurationScale,
}

impl PartialEq for SnapFlingBehavior {
    fn eq(&self, other: &Self) -> bool {
        *self.snap_animation_spec == *other.snap_animation_spec
            && Arc::ptr_eq(&self.decay_animation_spec, &other.decay_animation_spec)
            && Rc::ptr_eq(&self.snap_layout_info_provider, &other.snap_layout_info_provider)
    }
}

impl SnapFlingBehavior {
    pub fn new(
        snap_layout_info_provider: Rc<dyn SnapLayoutInfoProvider>,
        decay_animation_spec: Arc<dyn DecayAnimationSpec<f32>>,
        snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
    ) -> SnapFlingBehavior {
        SnapFlingBehavior {
            snap_layout_info_provider,
            decay_animation_spec,
            snap_animation_spec,
            motion_scale_duration: default_scroll_motion_duration_scale(),
        }
    }

    /// The approach and the snap: what is left to go when done (none unless an end stopped it) and the
    /// animation's state.
    async fn fling(
        &self,
        scope: Rc<dyn ScrollScope>,
        initial_velocity: f32,
        on_remaining_scroll_offset_update: Rc<dyn Fn(f32)>,
    ) -> (f32, AnimationState<f32, AnimationVector1D>) {
        let on_update = on_remaining_scroll_offset_update.clone();
        let result = with_context(self.motion_scale_duration.clone(), async move {
            let decay_offset = self.decay_animation_spec.calculate_float_target_value(0.0, initial_velocity);
            let initial_offset =
                self.snap_layout_info_provider.calculate_approach_offset(initial_velocity, decay_offset);
            assert!(!initial_offset.is_nan(), "calculateApproachOffset returned NaN. Please use a valid value.");
            // (the offset's sign and value made right)
            let remaining_scroll_offset = Rc::new(Cell::new(initial_offset.abs() * sign(initial_velocity)));
            on_update(remaining_scroll_offset.get());
            let (r, u) = (remaining_scroll_offset.clone(), on_update.clone());
            let animation_state = self
                .try_approach(
                    &scope,
                    remaining_scroll_offset.get(),
                    initial_velocity,
                    Rc::new(move |delta| {
                        r.set(r.get() - delta);
                        u(r.get());
                    }),
                )
                .await;
            let final_snap_offset = self.snap_layout_info_provider.calculate_snap_offset(animation_state.velocity());
            assert!(!final_snap_offset.is_nan(), "calculateSnapOffset returned NaN. Please use a valid value.");
            remaining_scroll_offset.set(final_snap_offset);
            let (r, u) = (remaining_scroll_offset.clone(), on_update.clone());
            animate_with_target(
                &scope,
                final_snap_offset,
                final_snap_offset,
                // (the velocity and time of the approach kept)
                named_args!(animation_state.copy_float(value = 0.0)),
                self.snap_animation_spec.clone(),
                Rc::new(move |delta| {
                    r.set(r.get() - delta);
                    u(r.get());
                }),
            )
            .await
        })
        .await;
        // (done, or cancelled)
        on_remaining_scroll_offset_update(0.0);
        result
    }

    /// No velocity or no approach offset: no approach (Compose's `tryApproach`).
    async fn try_approach(
        &self,
        scope: &Rc<dyn ScrollScope>,
        offset: f32,
        velocity: f32,
        update_remaining_scroll_offset: Rc<dyn Fn(f32)>,
    ) -> AnimationState<f32, AnimationVector1D> {
        if offset.abs() == 0.0 || velocity.abs() == 0.0 {
            named_args!(float_animation_state(offset, initial_velocity = velocity))
        } else {
            self.run_approach(scope, offset, velocity, update_remaining_scroll_offset).await.1
        }
    }

    /// A decay when it reaches the offset, else an animation to it (Compose's `runApproach`).
    async fn run_approach(
        &self,
        scope: &Rc<dyn ScrollScope>,
        initial_target_offset: f32,
        initial_velocity: f32,
        on_animation_step: Rc<dyn Fn(f32)>,
    ) -> (f32, AnimationState<f32, AnimationVector1D>) {
        let animation_state = named_args!(float_animation_state(0.0, initial_velocity = initial_velocity));
        if self.is_decay_approach_possible(initial_target_offset, initial_velocity) {
            // (high velocity)
            animate_decay(
                scope,
                initial_target_offset,
                animation_state,
                self.decay_animation_spec.clone(),
                on_animation_step,
            )
            .await
        } else {
            // (low velocity)
            let target_offset = initial_target_offset.abs() * sign(initial_velocity);
            animate_with_target(
                scope,
                target_offset,
                initial_target_offset,
                animation_state,
                self.snap_animation_spec.clone(),
                on_animation_step,
            )
            .await
        }
    }

    fn is_decay_approach_possible(&self, offset: f32, velocity: f32) -> bool {
        let decay_offset = self.decay_animation_spec.calculate_float_target_value(NoDistance, velocity);
        decay_offset.abs() >= offset.abs()
    }
}

impl FlingBehavior for SnapFlingBehavior {
    fn perform_fling(&self, scope: Rc<dyn ScrollScope>, initial_velocity: f32) -> LocalFuture<'_, f32> {
        self.perform_fling_with_remaining(scope, initial_velocity, Rc::new(|_| {}))
    }
}

impl TargetedFlingBehavior for SnapFlingBehavior {
    fn perform_fling_with_remaining(
        &self,
        scope: Rc<dyn ScrollScope>,
        initial_velocity: f32,
        on_remaining_distance_updated: Rc<dyn Fn(f32)>,
    ) -> LocalFuture<'_, f32> {
        Box::pin(async move {
            let (remaining_offset, remaining_state) =
                self.fling(scope, initial_velocity, on_remaining_distance_updated).await;
            // (all used: no velocity on for the nested scroll; else an end stopped it)
            if remaining_offset == 0.0 { NoVelocity } else { remaining_state.velocity() }
        })
    }
}

/// A snapping fling remembered for `snap_layout_info_provider` (Compose's `rememberSnapFlingBehavior`): Android's
/// spline approaches fast flings, a spring (StiffnessMediumLow) slow ones and snaps.
#[kompozable]
pub fn remember_snap_fling_behavior(
    snap_layout_info_provider: Rc<dyn SnapLayoutInfoProvider>,
) -> Rc<dyn TargetedFlingBehavior> {
    let density = LocalDensity.current();
    let high_velocity_approach_spec = remember_spline_based_decay::<f32>();
    let keys = (Rc::as_ptr(&snap_layout_info_provider) as *const () as usize, density.density.to_bits());
    remember_keyed(keys, move || {
        snap_fling_behavior(
            snap_layout_info_provider,
            Arc::new(high_velocity_approach_spec),
            Arc::new(SpringSpec::<f32>::new(Spring::DampingRatioNoBouncy, Spring::StiffnessMediumLow, None)),
        )
    })
}

/// Kotlin's `Float.sign`.
fn sign(v: f32) -> f32 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// `value` brought to `target` from its side (Compose's `coerceToTarget`; 0 for a target of 0).
fn coerce_to_target(value: f32, target: f32) -> f32 {
    if target == 0.0 {
        0.0
    } else if target > 0.0 {
        value.min(target)
    } else {
        value.max(target)
    }
}

/// A decay as far as `target_offset` (Compose's `ScrollScope.animateDecay`): each step scrolled, stopped where the
/// scroll doesn't take it or at the target; what is left and the state.
async fn animate_decay(
    scope: &Rc<dyn ScrollScope>,
    target_offset: f32,
    animation_state: AnimationState<f32, AnimationVector1D>,
    decay_animation_spec: Arc<dyn DecayAnimationSpec<f32>>,
    on_animation_step: Rc<dyn Fn(f32)>,
) -> (f32, AnimationState<f32, AnimationVector1D>) {
    let previous_value = Rc::new(Cell::new(0.0f32));
    let (p, s) = (previous_value.clone(), scope.clone());
    let block: Box<dyn for<'a> FnMut(&'a AnimationScope<f32, AnimationVector1D>)> = Box::new(move |a| {
        let consume_delta = |delta: f32| {
            let consumed = s.scroll_by(delta);
            on_animation_step(consumed);
            if (delta - consumed).abs() > 0.5 {
                a.cancel_animation();
            }
        };
        let value = a.value();
        if value.abs() >= target_offset.abs() {
            let final_value = coerce_to_target(value, target_offset);
            let final_delta = final_value - p.get();
            consume_delta(final_delta);
            a.cancel_animation();
            p.set(final_value);
        } else {
            let delta = value - p.get();
            consume_delta(delta);
            p.set(value);
        }
    });
    let sequential = animation_state.velocity() != 0.0;
    named_args!(animation_state.animate_decay(decay_animation_spec, sequential_animation = sequential, block = block))
        .await;
    (target_offset - previous_value.get(), animation_state)
}

/// An animation to `target_offset` along `animation_spec`, stopped past `cancel_offset` or where the scroll doesn't
/// take it (Compose's `ScrollScope.animateWithTarget`); its velocity brought within the initial one.
async fn animate_with_target(
    scope: &Rc<dyn ScrollScope>,
    target_offset: f32,
    cancel_offset: f32,
    animation_state: AnimationState<f32, AnimationVector1D>,
    animation_spec: Arc<dyn AnimationSpec<f32>>,
    on_animation_step: Rc<dyn Fn(f32)>,
) -> (f32, AnimationState<f32, AnimationVector1D>) {
    let consumed_up_to_now = Rc::new(Cell::new(0.0f32));
    let initial_velocity = animation_state.velocity();
    let (c, s) = (consumed_up_to_now.clone(), scope.clone());
    let block: Box<dyn for<'a> FnMut(&'a AnimationScope<f32, AnimationVector1D>)> = Box::new(move |a| {
        let value = a.value();
        let real_value = coerce_to_target(value, cancel_offset);
        let delta = real_value - c.get();
        let consumed = s.scroll_by(delta);
        on_animation_step(consumed);
        // (stopped where it isn't taken, or at the value it should reach)
        if (delta - consumed).abs() > 0.5 || real_value != value {
            a.cancel_animation();
        }
        c.set(c.get() + consumed);
    });
    let sequential = animation_state.velocity() != 0.0;
    named_args!(animation_state.animate_to(
        target_offset,
        animation_spec = animation_spec,
        sequential_animation = sequential,
        block = block
    ))
    .await;
    // (the velocity kept from growing too large)
    let final_velocity = coerce_to_target(animation_state.velocity(), initial_velocity);
    (target_offset - consumed_up_to_now.get(), named_args!(animation_state.copy_float(velocity = final_velocity)))
}

/// Which item a fling settles on (Compose's FinalSnappingItem, its variants' names).
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FinalSnappingItem {
    ClosestItem,
    NextItem,
    PreviousItem,
}

/// The item a velocity settles on (Compose's `calculateFinalSnappingItem`): the closest below the minimum fling
/// velocity, else the next or the previous.
pub(crate) fn calculate_final_snapping_item(density: &Density, velocity: f32) -> FinalSnappingItem {
    if velocity.abs() < density.to_px(MinFlingVelocityDp) {
        FinalSnappingItem::ClosestItem
    } else if velocity > 0.0 {
        FinalSnappingItem::NextItem
    } else {
        FinalSnappingItem::PreviousItem
    }
}

/// The offset to snap by, of the bounds found (Compose's `calculateFinalOffset`): none where the bound is infinite.
pub(crate) fn calculate_final_offset(snapping_offset: FinalSnappingItem, lower_bound: f32, upper_bound: f32) -> f32 {
    let final_distance = match snapping_offset {
        FinalSnappingItem::ClosestItem => {
            if upper_bound.abs() <= lower_bound.abs() {
                upper_bound
            } else {
                lower_bound
            }
        }
        FinalSnappingItem::NextItem => upper_bound,
        FinalSnappingItem::PreviousItem => lower_bound,
    };
    if final_distance.is_finite() { final_distance } else { NoDistance }
}
