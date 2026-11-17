//! How long motion takes (Compose's MotionDurationScale): a coroutine context element animations read.

use kompoz_coroutines::CoroutineContext;
use std::rc::Rc;

/// The multiplier of animations' durations (Compose's MotionDurationScale, a coroutine context element): 1 plays
/// them in real time, 0 ends them at their next frame, 10 makes a 100 ms one take 1000 ms. Animations read it from
/// their coroutine's context (`with_context(MotionDurationScale::new(2.0), …)`), 1 where there is none (the desktop
/// has none; Android's window gives the system's animator scale). Clones are the same scale.
#[derive(Clone)]
pub struct MotionDurationScale {
    scale_factor: Rc<dyn Fn() -> f32>,
}

impl MotionDurationScale {
    /// A fixed scale (Compose's objects overriding `scaleFactor` with a constant).
    pub fn new(scale_factor: f32) -> MotionDurationScale {
        MotionDurationScale { scale_factor: Rc::new(move || scale_factor) }
    }

    /// A scale read each time it is asked for (Android's, from a state the system setting updates).
    pub fn from_fn(scale_factor: impl Fn() -> f32 + 'static) -> MotionDurationScale {
        MotionDurationScale { scale_factor: Rc::new(scale_factor) }
    }

    /// The multiplier now (non-negative).
    pub fn scale_factor(&self) -> f32 {
        (self.scale_factor)()
    }
}

impl std::fmt::Debug for MotionDurationScale {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "MotionDurationScale({})", self.scale_factor())
    }
}

/// As Kotlin's element is a context of its own: `with_context(scale, block)`.
impl From<MotionDurationScale> for CoroutineContext {
    fn from(scale: MotionDurationScale) -> CoroutineContext {
        CoroutineContext::of(scale)
    }
}
