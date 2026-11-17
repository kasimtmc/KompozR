//! One ripple, as Compose's RippleAnimation.kt: it fades in (75 ms) while it spreads (225 ms, FastOutSlowIn) from
//! a third of the larger side to its radius, its center drifting from the press to the middle; once let go (and
//! faded in) it fades out (150 ms).

use kompoz_animation::{Animatable, AnimationVector1D, FastOutSlowInEasing, LinearEasing, TweenSpec};
use kompoz_coroutines::coroutine_scope;
use kompoz_graphics::{Color, Density, Dp, Offset, Size};
use kompoz_runtime::{MutableState, mutable_state_of, named_args};
use kompoz_ui::draw::DrawScope;
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

/// How long a ripple fades in, ms (Compose's `FadeInDuration`).
const FadeInDuration: i32 = 75;
/// How long it spreads, ms (Compose's `RadiusDuration`).
const RadiusDuration: i32 = 225;
/// How long it fades out, ms (Compose's `FadeOutDuration`).
const FadeOutDuration: i32 = 150;
/// How far a bounded ripple spreads past the corners (Compose's `BoundedRippleExtraRadius`).
const BoundedRippleExtraRadius: Dp = Dp(10.0);

/// A ripple's animations (Compose's RippleAnimation): `origin` is where it was pressed (none: the middle, an
/// unbounded one's), `radius` how far it spreads.
pub(crate) struct RippleAnimation {
    origin: Cell<Option<Offset>>,
    radius: f32,
    bounded: bool,
    start_radius: Cell<Option<f32>>,
    target_center: Cell<Option<Offset>>,
    animated_alpha: Animatable<f32, AnimationVector1D>,
    animated_radius_percent: Animatable<f32, AnimationVector1D>,
    animated_center_percent: Animatable<f32, AnimationVector1D>,
    finish_signal: Rc<Signal>,
    finished_fading_in: MutableState<bool>,
    finish_requested: MutableState<bool>,
}

impl RippleAnimation {
    pub(crate) fn new(origin: Option<Offset>, radius: f32, bounded: bool) -> RippleAnimation {
        RippleAnimation {
            origin: Cell::new(origin),
            radius,
            bounded,
            start_radius: Cell::new(None),
            target_center: Cell::new(None),
            animated_alpha: Animatable(0.0),
            animated_radius_percent: Animatable(0.0),
            animated_center_percent: Animatable(0.0),
            finish_signal: Rc::new(Signal::default()),
            finished_fading_in: mutable_state_of(false),
            finish_requested: mutable_state_of(false),
        }
    }

    /// Fades in and spreads, waits to be let go, fades out (Compose's `animate`).
    pub(crate) async fn animate(self: Rc<Self>) {
        self.fade_in().await;
        self.finished_fading_in.set(true);
        SignalWait(self.finish_signal.clone()).await;
        self.fade_out().await;
    }

    async fn fade_in(&self) {
        let (alpha, radius, center) =
            (self.animated_alpha.clone(), self.animated_radius_percent.clone(), self.animated_center_percent.clone());
        coroutine_scope(move |cs| async move {
            cs.launch(async move {
                let spec = Arc::new(TweenSpec::new(FadeInDuration, 0, LinearEasing));
                named_args!(alpha.animate_to(1.0, animation_spec = spec)).await;
            });
            cs.launch(async move {
                let spec = Arc::new(TweenSpec::new(RadiusDuration, 0, FastOutSlowInEasing));
                named_args!(radius.animate_to(1.0, animation_spec = spec)).await;
            });
            cs.launch(async move {
                let spec = Arc::new(TweenSpec::new(RadiusDuration, 0, LinearEasing));
                named_args!(center.animate_to(1.0, animation_spec = spec)).await;
            });
        })
        .await;
    }

    async fn fade_out(&self) {
        let alpha = self.animated_alpha.clone();
        coroutine_scope(move |cs| async move {
            cs.launch(async move {
                let spec = Arc::new(TweenSpec::new(FadeOutDuration, 0, LinearEasing));
                named_args!(alpha.animate_to(0.0, animation_spec = spec)).await;
            });
        })
        .await;
    }

    /// Lets it go: it fades out once faded in (Compose's `finish`).
    pub(crate) fn finish(&self) {
        self.finish_requested.set(true);
        self.finish_signal.complete();
    }

    /// Draws it in `color` (Compose's `DrawScope.draw`): clipped to the bounds when bounded.
    pub(crate) fn draw(&self, scope: &mut DrawScope, color: Color) {
        let size = scope.size();
        if self.start_radius.get().is_none() {
            self.start_radius.set(Some(get_ripple_start_radius(size)));
        }
        if self.origin.get().is_none() {
            self.origin.set(Some(scope.center()));
        }
        if self.target_center.get().is_none() {
            self.target_center.set(Some(Offset::new(size.width / 2.0, size.height / 2.0)));
        }
        // (let go while still fading in: shown whole at once)
        let alpha = if self.finish_requested.get() && !self.finished_fading_in.get() {
            1.0
        } else {
            self.animated_alpha.value()
        };
        let radius = lerp(self.start_radius.get().unwrap(), self.radius, self.animated_radius_percent.value());
        let (origin, target) = (self.origin.get().unwrap(), self.target_center.get().unwrap());
        let percent = self.animated_center_percent.value();
        let center_offset = Offset::new(lerp(origin.x, target.x, percent), lerp(origin.y, target.y, percent));
        let modulated_color = named_args!(color.copy(alpha = color.alpha() * alpha));
        if self.bounded {
            let (w, h) = (size.width, size.height);
            scope.clip_rect_with(0.0, 0.0, w, h, Default::default(), |s| {
                s.draw_circle_with(
                    modulated_color,
                    radius,
                    center_offset,
                    1.0,
                    Default::default(),
                    None,
                    Default::default(),
                )
            });
        } else {
            scope.draw_circle_with(
                modulated_color,
                radius,
                center_offset,
                1.0,
                Default::default(),
                None,
                Default::default(),
            );
        }
    }
}

/// Where a ripple starts spreading from: a third of the larger side (Compose's `getRippleStartRadius`).
pub(crate) fn get_ripple_start_radius(size: Size) -> f32 {
    size.width.max(size.height) * 0.3
}

/// How far a ripple spreads: to the corners from the middle, and 10 dp further when bounded (Compose's
/// `getRippleEndRadius`).
pub(crate) fn get_ripple_end_radius(density: &Density, bounded: bool, size: Size) -> f32 {
    let radius_covering_bounds = Offset::new(size.width, size.height).get_distance() / 2.0;
    if bounded { radius_covering_bounds + density.to_px(BoundedRippleExtraRadius) } else { radius_covering_bounds }
}

/// Compose's `lerp(start, stop, fraction)` of floats.
fn lerp(start: f32, stop: f32, fraction: f32) -> f32 {
    (1.0 - fraction) * start + fraction * stop
}

/// A signal completed once (Compose's `CompletableDeferred<Unit>`).
#[derive(Default)]
struct Signal {
    done: Cell<bool>,
    waiters: RefCell<Vec<Waker>>,
}

impl Signal {
    fn complete(&self) {
        self.done.set(true);
        for w in self.waiters.borrow_mut().drain(..) {
            w.wake();
        }
    }
}

/// Waits for its signal.
struct SignalWait(Rc<Signal>);

impl Future for SignalWait {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0.done.get() {
            return Poll::Ready(());
        }
        self.0.waiters.borrow_mut().push(cx.waker().clone());
        Poll::Pending
    }
}
