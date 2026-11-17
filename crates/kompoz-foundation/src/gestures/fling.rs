//! Flings, as Compose's FlingBehavior.kt and Scrollable.kt's DefaultFlingBehavior: what scrolls on after a drag is
//! let go with velocity — Android's spline decay by default, for the screen's density.

use super::scrollable_state::{LocalFuture, ScrollScope};
use kompoz_animation::{
    AnimationScope, AnimationVector1D, DecayAnimationSpec, float_animation_state, remember_spline_based_decay,
    spline_based_decay,
};
use kompoz_coroutines::{catch_cancellation, with_context};
use kompoz_graphics::Density;
use kompoz_runtime::{kompozable, named_args, remember_keyed};
use kompoz_ui::MotionDurationScale;
use kompoz_ui::platform::LocalDensity;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// How a scrollable flings (Compose's FlingBehavior): given the velocity a drag was let go with, it scrolls in the
/// scope it is given (the scrollable's mutation, through its nested scroll and overscroll) and gives back the
/// velocity left (what the scrollable's nested scroll parents get to fling on with). Shared as
/// `Rc<dyn FlingBehavior>`; two are equal when their types say so (by default: the same one).
pub trait FlingBehavior: Any + FlingBehaviorEq {
    /// Flings from `initial_velocity` px/s along the scrollable's axis: the velocity left (Compose's
    /// `ScrollScope.performFling`).
    fn perform_fling(&self, scope: Rc<dyn ScrollScope>, initial_velocity: f32) -> LocalFuture<'_, f32>;

    /// It as a scrollable's default fling (Compose's ScrollableDefaultFlingBehavior: what a scrollable keeps for
    /// its density and a mouse wheel doesn't set off).
    #[doc(hidden)]
    fn as_scrollable_default(&self) -> Option<&dyn ScrollableDefaultFlingBehavior> {
        None
    }
}

/// A fling that knows where it settles (Compose's TargetedFlingBehavior: a snapping one): it tells the distance
/// left to its settling offset as it goes.
pub trait TargetedFlingBehavior: FlingBehavior {
    /// Flings from `initial_velocity` px/s, telling `on_remaining_distance_updated` the distance left to where it
    /// settles each step (it may change as it goes); the velocity left (Compose's `performFling(initialVelocity,
    /// onRemainingDistanceUpdated)`).
    /// Kotlin: `performFling(initialVelocity: Float, onRemainingDistanceUpdated: (Float) -> Unit)`
    fn perform_fling_with_remaining(
        &self,
        scope: Rc<dyn ScrollScope>,
        initial_velocity: f32,
        on_remaining_distance_updated: Rc<dyn Fn(f32)>,
    ) -> LocalFuture<'_, f32>;
}

impl fmt::Debug for dyn TargetedFlingBehavior {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TargetedFlingBehavior")
    }
}

/// How fling behaviors compare (a blanket impl for every `PartialEq` one).
#[doc(hidden)]
pub trait FlingBehaviorEq {
    fn fling_behavior_eq(&self, other: &dyn Any) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T: PartialEq + Any> FlingBehaviorEq for T {
    fn fling_behavior_eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|o| self == o)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl PartialEq for dyn FlingBehavior {
    fn eq(&self, other: &dyn FlingBehavior) -> bool {
        self.fling_behavior_eq(other.as_any())
    }
}

impl fmt::Debug for dyn FlingBehavior {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "FlingBehavior")
    }
}

/// Whether two optional fling behaviors are the same.
pub(crate) fn same_fling_behavior(a: &Option<Rc<dyn FlingBehavior>>, b: &Option<Rc<dyn FlingBehavior>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => **a == **b,
        (None, None) => true,
        _ => false,
    }
}

/// A scrollable's default fling, made before its density is known and told it once it is (Compose's internal
/// ScrollableDefaultFlingBehavior).
#[doc(hidden)]
pub trait ScrollableDefaultFlingBehavior {
    /// Its parameters for `density` (Compose's `updateDensity`).
    fn update_density(&self, density: Density) {
        let _ = density;
    }
}

/// Whether a mouse wheel's scroll ends with a fling of `fling_behavior`: not the default one (Compose's
/// `shouldBeTriggeredByMouseWheel`: only snapping's own flings, a pager's, run after a wheel).
pub(crate) fn should_be_triggered_by_mouse_wheel(fling_behavior: &dyn FlingBehavior) -> bool {
    fling_behavior.as_scrollable_default().is_none()
}

/// The fling of a decay (Compose's DefaultFlingBehavior): Android's spline for the screen's density by default; it
/// scrolls by each frame's step and stops where the scroll doesn't take it (an end). Equal only to itself.
pub struct DefaultFlingBehavior {
    fling_decay: RefCell<Arc<dyn DecayAnimationSpec<f32>>>,
    motion_duration_scale: MotionDurationScale,
    /// The frames its last fling ran (Compose's `lastAnimationCycleCount`, for tests).
    pub last_animation_cycle_count: Cell<i32>,
}

impl PartialEq for DefaultFlingBehavior {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl DefaultFlingBehavior {
    /// A fling along `fling_decay`, its durations scaled by `motion_duration_scale` (Compose's
    /// `DefaultFlingBehavior(flingDecay, motionDurationScale = DefaultScrollMotionDurationScale)`: 1 by default,
    /// whatever the system's animator scale).
    pub fn new(
        fling_decay: Arc<dyn DecayAnimationSpec<f32>>,
        motion_duration_scale: Option<MotionDurationScale>,
    ) -> DefaultFlingBehavior {
        DefaultFlingBehavior {
            fling_decay: RefCell::new(fling_decay),
            motion_duration_scale: motion_duration_scale.unwrap_or_else(default_scroll_motion_duration_scale),
            last_animation_cycle_count: Cell::new(0),
        }
    }
}

/// Compose's DefaultScrollMotionDurationScale: 1.
pub(crate) fn default_scroll_motion_duration_scale() -> MotionDurationScale {
    MotionDurationScale::new(1.0)
}

impl FlingBehavior for DefaultFlingBehavior {
    fn perform_fling(&self, scope: Rc<dyn ScrollScope>, initial_velocity: f32) -> LocalFuture<'_, f32> {
        self.last_animation_cycle_count.set(0);
        let fling_decay = self.fling_decay.borrow().clone();
        Box::pin(with_context(self.motion_duration_scale.clone(), async move {
            // (a threshold: the spline gives NaNs below it)
            if initial_velocity.abs() <= 1.0 {
                return initial_velocity;
            }
            let velocity_left = Rc::new(Cell::new(initial_velocity));
            let last_value = Rc::new(Cell::new(0.0f32));
            let animation_state = named_args!(float_animation_state(0.0, initial_velocity = initial_velocity));
            let (state, v, last) = (animation_state.clone(), velocity_left.clone(), last_value.clone());
            let cycles = Rc::new(Cell::new(0));
            let c = cycles.clone();
            let block: Box<dyn for<'a> FnMut(&'a AnimationScope<f32, AnimationVector1D>)> =
                Box::new(move |s: &AnimationScope<f32, AnimationVector1D>| {
                    let delta = s.value() - last.get();
                    let consumed = scope.scroll_by(delta);
                    last.set(s.value());
                    v.set(s.velocity());
                    // Rounding left aside, anything unconsumed stops it.
                    if (delta - consumed).abs() > 0.5 {
                        s.cancel_animation();
                    }
                    c.set(c.get() + 1);
                });
            let fling = catch_cancellation(move |_| async move {
                named_args!(state.animate_decay(fling_decay, false, block)).await
            })
            .await;
            self.last_animation_cycle_count.set(cycles.get());
            if fling.is_err() {
                // (Kotlin's catch of the fling's cancellation: its scrollable's node left, say)
                velocity_left.set(animation_state.velocity());
            }
            velocity_left.get()
        }))
    }

    fn as_scrollable_default(&self) -> Option<&dyn ScrollableDefaultFlingBehavior> {
        Some(self)
    }
}

impl ScrollableDefaultFlingBehavior for DefaultFlingBehavior {
    fn update_density(&self, density: Density) {
        *self.fling_decay.borrow_mut() = Arc::new(spline_based_decay::<f32>(density));
    }
}

/// A scrollable's own default fling (Compose's `platformScrollableDefaultFlingBehavior`): the spline at density 1
/// until the scrollable tells it its own.
pub(crate) fn platform_scrollable_default_fling_behavior() -> Rc<DefaultFlingBehavior> {
    Rc::new(DefaultFlingBehavior::new(Arc::new(spline_based_decay::<f32>(Density::new(1.0, 1.0))), None))
}

/// The default fling remembered for this screen's density (Compose's `rememberPlatformDefaultFlingBehavior`).
#[kompozable]
pub(crate) fn remember_platform_default_fling_behavior() -> Rc<dyn FlingBehavior> {
    let fling_spec = remember_spline_based_decay::<f32>();
    let density = LocalDensity.current();
    remember_keyed(density, move || {
        Rc::new(DefaultFlingBehavior::new(Arc::new(fling_spec), None)) as Rc<dyn FlingBehavior>
    })
}
