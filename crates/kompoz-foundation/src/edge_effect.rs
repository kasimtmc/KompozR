//! Android's EdgeEffect (android.widget.EdgeEffect, API 37) as its stretch type behaves (Android 12's
//! overscroll, which Compose's stretch overscroll drives through EdgeEffectCompat): a pull's distance (a fraction
//! of the effect's height, at most 1), let go into a spring back (its natural frequency and damping, a linear end
//! when close), a fling absorbed into the same spring; drawn by stretching the layer it is drawn into
//! ([`GraphicsLayer::stretch`], hwui's RenderNode.stretch) with the vector `draw` works out through the canvas
//! rotation the edge is drawn with.
//!
//! The glow type (before Android 12) is not here: KompozR's overscroll is the stretch (stage 10's decision D6).
//! Android's "animations off" (ValueAnimator.areAnimatorsEnabled false: no effect) is Android's system setting,
//! stage 15's. The time is the host's clock ([`kompoz_ui::NodeCx::dispatcher`]), what
//! `AnimationUtils.currentAnimationTimeMillis` is to a view.

use kompoz_ui::draw::GraphicsLayer;

/// Below this, a spring's velocity is still (EdgeEffect's VELOCITY_THRESHOLD).
const VELOCITY_THRESHOLD: f64 = 0.01;
/// The velocity of the linear end of a spring back (LINEAR_VELOCITY_TAKE_OVER).
const LINEAR_VELOCITY_TAKE_OVER: f32 = 200.0;
/// Below this distance, px, a spring is at rest (VALUE_THRESHOLD).
const VALUE_THRESHOLD: f64 = 0.001;
/// Below this distance, px, the spring ends linearly (LINEAR_DISTANCE_TAKE_OVER).
const LINEAR_DISTANCE_TAKE_OVER: f64 = 8.0;
/// The spring's natural frequency (NATURAL_FREQUENCY).
const NATURAL_FREQUENCY: f64 = 24.657;
/// Its damping ratio (DAMPING_RATIO).
const DAMPING_RATIO: f64 = 0.98;
/// What an absorbed fling's velocity is scaled by (ON_ABSORB_VELOCITY_ADJUSTMENT).
const ON_ABSORB_VELOCITY_ADJUSTMENT: f32 = 13.0;
/// The stretch's linear part (LINEAR_STRETCH_INTENSITY).
const LINEAR_STRETCH_INTENSITY: f32 = 0.016;
/// Its exponential part (EXP_STRETCH_INTENSITY).
const EXP_STRETCH_INTENSITY: f32 = 0.016;
/// How much of a scroll the exponential part takes (SCROLL_DIST_AFFECTED_BY_EXP_STRETCH).
const SCROLL_DIST_AFFECTED_BY_EXP_STRETCH: f32 = 0.33;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Idle,
    Pull,
    Recede,
}

/// An edge's effect (Android's EdgeEffect of the stretch type): sized by its width (along the edge) and height
/// (into the content), pulled, let go, absorbing flings, drawn into a layer as its stretch.
#[derive(Clone, Debug)]
pub(crate) struct EdgeEffect {
    distance: f32,
    velocity: f32,
    pull_distance: f32,
    start_time: i64,
    state: State,
    width: f32,
    height: f32,
    displacement: f32,
}

impl EdgeEffect {
    pub(crate) fn new() -> EdgeEffect {
        EdgeEffect {
            distance: 0.0,
            velocity: 0.0,
            pull_distance: 0.0,
            start_time: 0,
            state: State::Idle,
            width: 0.0,
            height: 0.0,
            displacement: 0.5,
        }
    }

    /// Its size: `width` along the edge, `height` into the content (EdgeEffect's `setSize`; the glow's bounds left
    /// out).
    pub(crate) fn set_size(&mut self, width: i32, height: i32) {
        self.width = width as f32;
        self.height = height as f32;
    }

    /// Whether it is at rest (EdgeEffect's `isFinished`).
    pub(crate) fn is_finished(&self) -> bool {
        self.state == State::Idle
    }

    /// Ends it at once (EdgeEffect's `finish`).
    pub(crate) fn finish(&mut self) {
        self.state = State::Idle;
        self.distance = 0.0;
        self.velocity = 0.0;
    }

    /// Pulled by `delta_distance` (a fraction of its height) at `now` ms (EdgeEffect's `onPull`): the stretch is
    /// drawn from its middle whatever the `displacement` along it (only the glow moves to where it is pulled, and
    /// the pull's timing is the glow's too).
    fn on_pull(&mut self, delta_distance: f32, _displacement: f32, now: i64) {
        if self.state != State::Pull {
            // Restore the pull distance to the fraction it is currently showing: "catch" the current stretch.
            self.pull_distance = self.distance;
        }
        self.state = State::Pull;
        self.start_time = now;
        self.pull_distance += delta_distance;
        // Don't allow stretch beyond 1.
        self.pull_distance = self.pull_distance.min(1.0);
        self.distance = self.pull_distance.max(0.0);
        self.velocity = 0.0;
        if self.distance == 0.0 {
            self.state = State::Idle;
        }
    }

    /// Pulled by `delta_distance`, never below nothing: the part of it that pulled it (EdgeEffect's
    /// `onPullDistance`), at `now` ms.
    pub(crate) fn on_pull_distance(&mut self, delta_distance: f32, displacement: f32, now: i64) -> f32 {
        let final_distance = (delta_distance + self.distance).max(0.0);
        let delta = final_distance - self.distance;
        if delta == 0.0 && self.distance == 0.0 {
            return 0.0; // No pull, don't do anything.
        }
        self.on_pull(delta, displacement, now);
        delta
    }

    /// How far it is pulled, a fraction of its height (EdgeEffect's `getDistance`).
    pub(crate) fn distance(&self) -> f32 {
        self.distance
    }

    /// Let go: a pull springs back (EdgeEffect's `onRelease`), at `now` ms.
    pub(crate) fn on_release(&mut self, now: i64) {
        self.pull_distance = 0.0;
        if self.state != State::Pull {
            return;
        }
        self.state = State::Recede;
        self.velocity = 0.0;
        self.start_time = now;
    }

    /// A fling of `velocity` px/s hit the edge: it springs from there (EdgeEffect's `onAbsorb`), at `now` ms.
    pub(crate) fn on_absorb(&mut self, velocity: i32, now: i64) {
        self.state = State::Recede;
        self.velocity = velocity as f32 * ON_ABSORB_VELOCITY_ADJUSTMENT;
        self.start_time = now;
    }

    /// Draws it into the layer the canvas records into, turned `rotation_degrees` about the canvas' origin (as
    /// Compose's StretchOverscrollNode turns the canvas for each edge: 0 the top, 90 the right, 180 the bottom,
    /// 270 the left), the layer at the origin: steps a spring back, then stretches the layer by the vector its
    /// pull makes through that rotation (EdgeEffect's `draw` on a RecordingCanvas). Whether it needs drawing
    /// again (`now` ms).
    pub(crate) fn draw(&mut self, rotation_degrees: f32, layer: &GraphicsLayer, now: i64) -> bool {
        let went_idle = self.step(now);
        if self.distance != 0.0 {
            let mut points = [
                0.0,
                0.0, // top-left
                self.width,
                0.0, // top-right
                self.width,
                self.height, // bottom-right
                0.0,
                self.height, // bottom-left
                self.width * self.displacement,
                0.0, // drag start point
                self.width * self.displacement,
                self.height * self.distance, // drag point
            ];
            map_points_rotated(rotation_degrees, &mut points);
            // (the RenderNode's left and top: Compose's is set at 0, 0)
            let left = min4(points[0], points[2], points[4], points[6]);
            let top = min4(points[1], points[3], points[5], points[7]);
            let right = max4(points[0], points[2], points[4], points[6]);
            let bottom = max4(points[1], points[3], points[5], points[7]);
            // (rotations of increments of 90 degrees)
            let x = points[10] - points[8];
            let width = right - left;
            let vec_x = damp_stretch_vector((x / width).clamp(-1.0, 1.0));
            let y = points[11] - points[9];
            let height = bottom - top;
            let vec_y = damp_stretch_vector((y / height).clamp(-1.0, 1.0));
            let has_valid_vectors = vec_x.is_finite() && vec_y.is_finite();
            if right > left && bottom > top && self.width > 0.0 && self.height > 0.0 && has_valid_vectors {
                layer.stretch(vec_x, vec_y, self.width, self.height);
            }
        }
        self.state != State::Idle || went_idle
    }

    /// Steps it to `now` ms: a spring back moves on, and one that arrived goes idle — whether it did (one last frame
    /// to draw). EdgeEffect's `draw` does it as it draws (an arrived spring goes idle after it): done first, a frame
    /// drawn again at the same time (a check, a test's look) draws what it drew, deciding the same, and nothing
    /// shows the difference (an arrived spring stretches nothing).
    pub(crate) fn step(&mut self, now: i64) -> bool {
        if self.state == State::Recede {
            self.update_spring(now);
        }
        if self.state == State::Recede && self.distance == 0.0 && self.velocity == 0.0 {
            self.state = State::Idle;
            return true;
        }
        false
    }

    /// A step of the spring back to `now` ms (EdgeEffect's `updateSpring`).
    fn update_spring(&mut self, now: i64) {
        let delta_t = (now - self.start_time) as f32 / 1000.0; // millis to seconds
        if delta_t < 0.001 {
            return; // Must have at least 1 ms difference
        }
        self.start_time = now;
        if self.velocity.abs() <= LINEAR_VELOCITY_TAKE_OVER
            && ((self.distance * self.height).abs() as f64) < LINEAR_DISTANCE_TAKE_OVER
            && java_signum(self.velocity) == -java_signum(self.distance)
        {
            // Close: interpolate linearly, so it arrives sooner than the spring would.
            self.velocity = java_signum(self.velocity) * LINEAR_VELOCITY_TAKE_OVER;
            let target_distance = self.distance + (self.velocity * delta_t / self.height);
            if java_signum(target_distance) != java_signum(self.distance) {
                // Arrived.
                self.distance = 0.0;
                self.velocity = 0.0;
            } else {
                self.distance = target_distance;
            }
            return;
        }
        let damped_freq = NATURAL_FREQUENCY * (1.0 - DAMPING_RATIO * DAMPING_RATIO).sqrt();
        // Always underdamped.
        let delta_t = delta_t as f64;
        let cos_coeff = (self.distance * self.height) as f64;
        let sin_coeff = (1.0 / damped_freq)
            * (DAMPING_RATIO * NATURAL_FREQUENCY * (self.distance * self.height) as f64 + self.velocity as f64);
        let distance = std::f64::consts::E.powf(-DAMPING_RATIO * NATURAL_FREQUENCY * delta_t)
            * (cos_coeff * (damped_freq * delta_t).cos() + sin_coeff * (damped_freq * delta_t).sin());
        let velocity = distance * (-NATURAL_FREQUENCY) * DAMPING_RATIO
            + std::f64::consts::E.powf(-DAMPING_RATIO * NATURAL_FREQUENCY * delta_t)
                * (-damped_freq * cos_coeff * (damped_freq * delta_t).sin()
                    + damped_freq * sin_coeff * (damped_freq * delta_t).cos());
        self.distance = distance as f32 / self.height;
        self.velocity = velocity as f32;
        if self.distance > 1.0 {
            self.distance = 1.0;
            self.velocity = 0.0;
        }
        if self.is_at_equilibrium() {
            self.distance = 0.0;
            self.velocity = 0.0;
        }
    }

    /// EdgeEffect's `isAtEquilibrium`: never stretched the other way, and still once very close.
    fn is_at_equilibrium(&self) -> bool {
        let displacement = (self.distance * self.height) as f64; // px
        let velocity = self.velocity as f64;
        displacement < 0.0 || (velocity.abs() < VELOCITY_THRESHOLD && displacement < VALUE_THRESHOLD)
    }
}

/// EdgeEffect's `dampStretchVector`: a linear and an exponential part.
fn damp_stretch_vector(normalized_vec: f32) -> f32 {
    let sign = if normalized_vec > 0.0 { 1.0 } else { -1.0 };
    let overscroll = normalized_vec.abs();
    let linear_intensity = LINEAR_STRETCH_INTENSITY * overscroll;
    let scalar = std::f64::consts::E / SCROLL_DIST_AFFECTED_BY_EXP_STRETCH as f64;
    let exp_intensity = EXP_STRETCH_INTENSITY as f64 * (1.0 - (-(overscroll as f64) * scalar).exp());
    sign * (linear_intensity as f64 + exp_intensity) as f32
}

/// Java's `Math.signum(float)`: 0 (of its sign) for a zero, the NaN for a NaN.
fn java_signum(v: f32) -> f32 {
    if v == 0.0 || v.is_nan() { v } else { v.signum() }
}

fn min4(a: f32, b: f32, c: f32, d: f32) -> f32 {
    a.min(b).min(c).min(d)
}

fn max4(a: f32, b: f32, c: f32, d: f32) -> f32 {
    a.max(b).max(c).max(d)
}

/// Maps `points` (x, y pairs) through a canvas turned `degrees` about its origin, as Skia's `setRotate` makes the
/// matrix (its sine and cosine snapped to 0 when nearly 0: SkScalarSinSnapToZero).
fn map_points_rotated(degrees: f32, points: &mut [f32]) {
    let radians = degrees.to_radians();
    let snap = |v: f32| if v.abs() <= 1.0 / 4096.0 { 0.0 } else { v };
    let (sin, cos) = (snap(radians.sin()), snap(radians.cos()));
    for p in points.as_chunks_mut::<2>().0 {
        let (x, y) = (p[0], p[1]);
        p[0] = x * cos - y * sin;
        p[1] = x * sin + y * cos;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pull_holds_and_a_release_springs_back() {
        let mut e = EdgeEffect::new();
        e.set_size(400, 800);
        assert_eq!(e.on_pull_distance(0.1, 0.5, 0), 0.1);
        assert!(!e.is_finished() && e.distance() == 0.1);
        // Pulled back past nothing: only as far as it was.
        assert_eq!(e.on_pull_distance(-0.3, 0.5, 10), -0.1);
        assert!(e.is_finished() && e.distance() == 0.0);
        e.on_pull_distance(0.2, 0.5, 20);
        e.on_release(20);
        let layer = GraphicsLayer::new(None);
        let mut t = 20;
        let mut last = e.distance();
        while e.draw(0.0, &layer, t) {
            assert!(e.distance() <= last + 1e-6);
            last = e.distance();
            t += 16;
            assert!(t < 5000, "it springs back");
        }
        assert_eq!(e.distance(), 0.0);
    }

    #[test]
    fn the_vector_follows_the_rotation() {
        assert_eq!(damp_stretch_vector(0.0), -0.0);
        let layer = GraphicsLayer::new(None);
        let mut top = EdgeEffect::new();
        top.set_size(400, 800);
        top.on_pull_distance(0.25, 0.5, 0);
        top.draw(0.0, &layer, 0);
        let s = layer.stretch_effect();
        assert!(s.stretch_direction().y > 0.0 && s.stretch_direction().x == 0.0);
        let layer = GraphicsLayer::new(None);
        let mut right = EdgeEffect::new();
        right.set_size(800, 400); // (the left and right effects are sized across)
        right.on_pull_distance(0.25, 0.5, 0);
        right.draw(90.0, &layer, 0);
        let s = layer.stretch_effect();
        assert!(s.stretch_direction().x < 0.0 && s.stretch_direction().y == 0.0);
        assert!((s.stretch_direction().x + damp_stretch_vector(0.25)).abs() < 1e-6);
    }
}
