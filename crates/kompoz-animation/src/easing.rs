//! How an animation's fraction of its time maps to its fraction of the way (Compose's Easing.kt,
//! EasingFunctions.kt, PathEasing.kt).

use kompoz_graphics::{
    ConicEvaluation, IntervalTree, Path, PathSegment, PathSegmentType, compute_cubic_vertical_bounds,
    compute_horizontal_bounds, evaluate_cubic, evaluate_y, find_first_cubic_root, find_first_root,
};
use kompoz_runtime::kt::panic_kt;
use std::any::Any;
use std::f64::consts::PI;
use std::sync::{Arc, OnceLock};

/// Maps a fraction of an animation's time (0..1) to a fraction of its way (Compose's `fun interface Easing`): 0 and
/// 1 to themselves, between them anything (past 0..1 too: an overshoot). A closure `Fn(f32) -> f32` is one. Specs
/// keep them as `Arc<dyn Easing>`.
pub trait Easing: Send + Sync + 'static {
    /// The fraction of the way at `fraction` of the time.
    fn transform(&self, fraction: f32) -> f32;

    /// Whether `other` is an easing equal to this one (Compose's `equals`: a CubicBezierEasing's by its
    /// parameters, the others by identity, which `Arc::ptr_eq` covers).
    #[doc(hidden)]
    fn equals(&self, other: &dyn Any) -> bool {
        let _ = other;
        false
    }

    #[doc(hidden)]
    fn as_any(&self) -> &dyn Any;
}

impl<F: Fn(f32) -> f32 + Send + Sync + 'static> Easing for F {
    fn transform(&self, fraction: f32) -> f32 {
        self(fraction)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Easing for Arc<dyn Easing> {
    fn transform(&self, fraction: f32) -> f32 {
        (**self).transform(fraction)
    }
    fn equals(&self, other: &dyn Any) -> bool {
        (**self).equals(other)
    }
    fn as_any(&self) -> &dyn Any {
        (**self).as_any()
    }
}

/// Whether two easings are equal as Compose's are: the same one, or equal ones.
pub(crate) fn easing_eq(a: &Arc<dyn Easing>, b: &Arc<dyn Easing>) -> bool {
    Arc::ptr_eq(a, b) || a.equals(b.as_any())
}

/// The easing as an `Arc<dyn Easing>` (a spec's), shared if it is one already.
pub fn arc_easing(easing: impl Easing) -> Arc<dyn Easing> {
    let any: &dyn Any = &easing;
    if let Some(shared) = any.downcast_ref::<Arc<dyn Easing>>() {
        return shared.clone();
    }
    Arc::new(easing)
}

/// One ulp at 1: the least fraction the bezier is solved at (Compose's OneUlpAt1).
const OneUlpAt1: f32 = 1.192_092_9e-7;

/// A cubic Bézier from (0, 0) to (1, 1) with control points (a, b) and (c, d), as CSS's cubic-bezier() (Compose's
/// CubicBezierEasing): the fraction of the time is the curve's x, solved for its parameter, whose y is the
/// fraction of the way. Equal to another by its parameters.
#[derive(Clone, Copy, Debug)]
pub struct CubicBezierEasing {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    /// The y range of the curve (what a result is kept within); computed as it is asked for when not given (the
    /// constants: a `const` can't compute it).
    bounds: Option<(f32, f32)>,
}

/// Compose's `CubicBezierEasing(a, b, c, d)`: control points (a, b) and (c, d); NaN in them panics as Compose's
/// throws.
#[track_caller]
pub fn CubicBezierEasing(a: f32, b: f32, c: f32, d: f32) -> CubicBezierEasing {
    if a.is_nan() || b.is_nan() || c.is_nan() || d.is_nan() {
        panic_kt(
            "IllegalArgumentException",
            &format!("Parameters to CubicBezierEasing cannot be NaN. Actual parameters are: {a}, {b}, {c}, {d}."),
        );
    }
    CubicBezierEasing { a, b, c, d, bounds: Some(compute_cubic_vertical_bounds(0.0, b, d, 1.0)) }
}

impl CubicBezierEasing {
    /// The easing of these control points as a constant (its bounds computed where it is used).
    pub const fn constant(a: f32, b: f32, c: f32, d: f32) -> CubicBezierEasing {
        CubicBezierEasing { a, b, c, d, bounds: None }
    }
}

impl PartialEq for CubicBezierEasing {
    fn eq(&self, other: &Self) -> bool {
        self.a == other.a && self.b == other.b && self.c == other.c && self.d == other.d
    }
}

impl Easing for CubicBezierEasing {
    #[track_caller]
    fn transform(&self, fraction: f32) -> f32 {
        if fraction > 0.0 && fraction < 1.0 {
            let f = fraction.max(OneUlpAt1);
            let t = find_first_cubic_root(0.0 - f, self.a - f, self.c - f, 1.0 - f);
            if t.is_nan() {
                panic_kt(
                    "IllegalArgumentException",
                    &format!(
                        "The cubic curve with parameters ({}, {}, {}, {}) has no solution at {fraction}",
                        self.a, self.b, self.c, self.d
                    ),
                );
            }
            let (min, max) = self.bounds.unwrap_or_else(|| compute_cubic_vertical_bounds(0.0, self.b, self.d, 1.0));
            coerce_in(evaluate_cubic(self.b, self.d, t), min, max)
        } else {
            fraction
        }
    }
    fn equals(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<CubicBezierEasing>().is_some_and(|o| o == self)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Kotlin's fastCoerceIn (a NaN passes).
pub(crate) fn coerce_in(v: f32, min: f32, max: f32) -> f32 {
    let mut v = v;
    if v < min {
        v = min;
    }
    if v > max {
        v = max;
    }
    v
}

/// Speeds up quickly and slows down gradually (Material's standard easing).
pub const FastOutSlowInEasing: CubicBezierEasing = CubicBezierEasing::constant(0.4, 0.0, 0.2, 1.0);
/// At full speed at once, slowing down to the end (incoming elements).
pub const LinearOutSlowInEasing: CubicBezierEasing = CubicBezierEasing::constant(0.0, 0.0, 0.2, 1.0);
/// Speeding up to the end, ending at full speed (outgoing elements).
pub const FastOutLinearInEasing: CubicBezierEasing = CubicBezierEasing::constant(0.4, 0.0, 1.0, 1.0);

/// The fraction of the way is the fraction of the time (Compose's LinearEasing).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct LinearEasing {}
/// The fraction of the way is the fraction of the time (Compose's LinearEasing).
pub const LinearEasing: LinearEasing = LinearEasing {};

/// An elastic start (easings.net's easeInElastic).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct EaseInElastic {}
/// An elastic start (easings.net's easeInElastic).
pub const EaseInElastic: EaseInElastic = EaseInElastic {};

/// An elastic end (easeOutElastic).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct EaseOutElastic {}
/// An elastic end (easeOutElastic).
pub const EaseOutElastic: EaseOutElastic = EaseOutElastic {};

/// Elastic both ways (easeInOutElastic).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct EaseInOutElastic {}
/// Elastic both ways (easeInOutElastic).
pub const EaseInOutElastic: EaseInOutElastic = EaseInOutElastic {};

/// Bouncing at the end (easeOutBounce).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct EaseOutBounce {}
/// Bouncing at the end (easeOutBounce).
pub const EaseOutBounce: EaseOutBounce = EaseOutBounce {};

/// Bouncing at the start (easeInBounce).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct EaseInBounce {}
/// Bouncing at the start (easeInBounce).
pub const EaseInBounce: EaseInBounce = EaseInBounce {};

/// Bouncing both ways (easeInOutBounce).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct EaseInOutBounce {}
/// Bouncing both ways (easeInOutBounce).
pub const EaseInOutBounce: EaseInOutBounce = EaseInOutBounce {};

/// The Easing of a unit type: its function, equal to the others of its type (Compose's are one object each).
macro_rules! easing_impls {
    ($($name:ident => $f:ident),* $(,)?) => {$(
        impl Easing for $name {
            fn transform(&self, fraction: f32) -> f32 {
                $f(fraction)
            }
            fn equals(&self, other: &dyn Any) -> bool {
                other.is::<$name>()
            }
            fn as_any(&self) -> &dyn Any {
                self
            }
        }
    )*};
}

easing_impls!(
    LinearEasing => linear,
    EaseInElastic => ease_in_elastic,
    EaseOutElastic => ease_out_elastic,
    EaseInOutElastic => ease_in_out_elastic,
    EaseOutBounce => ease_out_bounce,
    EaseInBounce => ease_in_bounce,
    EaseInOutBounce => ease_in_out_bounce,
);

/// Kotlin's `Float.pow(Float)`: in doubles.
fn powf(x: f32, y: f32) -> f32 {
    (x as f64).powf(y as f64) as f32
}

fn linear(fraction: f32) -> f32 {
    fraction
}

fn ease_in_elastic(fraction: f32) -> f32 {
    let c4 = (2.0 * PI) / 3.0;
    if fraction == 0.0 {
        0.0
    } else if fraction == 1.0 {
        1.0
    } else {
        (-(powf(2.0, 10.0 * fraction - 10.0) as f64) * (((fraction * 10.0 - 10.75) as f64) * c4).sin()) as f32
    }
}

fn ease_out_elastic(fraction: f32) -> f32 {
    let c4 = (2.0 * PI) / 3.0;
    if fraction == 0.0 {
        0.0
    } else if fraction == 1.0 {
        1.0
    } else {
        (powf(2.0, -10.0 * fraction) as f64 * (((fraction * 10.0 - 0.75) as f64) * c4).sin() + 1.0) as f32
    }
}

fn ease_in_out_elastic(fraction: f32) -> f32 {
    let c5 = (2.0 * PI) / 4.5;
    if fraction == 0.0 {
        0.0
    } else if fraction == 1.0 {
        1.0
    } else if (0.0..=0.5).contains(&fraction) {
        (-(powf(2.0, 20.0 * fraction - 10.0) as f64 * (((20.0 * fraction - 11.125) as f64) * c5).sin()) / 2.0) as f32
    } else {
        ((powf(2.0, -20.0 * fraction + 10.0) as f64 * (((fraction * 20.0 - 11.125) as f64) * c5).sin()) / 2.0) as f32
            + 1.0
    }
}

fn ease_in_bounce(fraction: f32) -> f32 {
    1.0 - ease_out_bounce(1.0 - fraction)
}

fn ease_in_out_bounce(fraction: f32) -> f32 {
    if (fraction as f64) < 0.5 {
        (1.0 - ease_out_bounce(1.0 - 2.0 * fraction)) / 2.0
    } else {
        (1.0 + ease_out_bounce(2.0 * fraction - 1.0)) / 2.0
    }
}

fn ease_out_bounce(fraction: f32) -> f32 {
    let n1 = 7.5625f32;
    let d1 = 2.75f32;
    let mut f = fraction;
    if f < 1.0 / d1 {
        n1 * f * f
    } else if f < 2.0 / d1 {
        f -= 1.5 / d1;
        n1 * f * f + 0.75
    } else if f < 2.5 / d1 {
        f -= 2.25 / d1;
        n1 * f * f + 0.9375
    } else {
        f -= 2.625 / d1;
        n1 * f * f + 0.984375
    }
}

/// easings.net's curves as cubic Béziers (Compose's EasingFunctions.kt).
pub const Ease: CubicBezierEasing = CubicBezierEasing::constant(0.25, 0.1, 0.25, 1.0);
pub const EaseOut: CubicBezierEasing = CubicBezierEasing::constant(0.0, 0.0, 0.58, 1.0);
pub const EaseIn: CubicBezierEasing = CubicBezierEasing::constant(0.42, 0.0, 1.0, 1.0);
pub const EaseInOut: CubicBezierEasing = CubicBezierEasing::constant(0.42, 0.0, 0.58, 1.0);
pub const EaseInSine: CubicBezierEasing = CubicBezierEasing::constant(0.12, 0.0, 0.39, 0.0);
pub const EaseOutSine: CubicBezierEasing = CubicBezierEasing::constant(0.61, 1.0, 0.88, 1.0);
pub const EaseInOutSine: CubicBezierEasing = CubicBezierEasing::constant(0.37, 0.0, 0.63, 1.0);
pub const EaseInCubic: CubicBezierEasing = CubicBezierEasing::constant(0.32, 0.0, 0.67, 0.0);
pub const EaseOutCubic: CubicBezierEasing = CubicBezierEasing::constant(0.33, 1.0, 0.68, 1.0);
pub const EaseInOutCubic: CubicBezierEasing = CubicBezierEasing::constant(0.65, 0.0, 0.35, 1.0);
pub const EaseInQuint: CubicBezierEasing = CubicBezierEasing::constant(0.64, 0.0, 0.78, 0.0);
pub const EaseOutQuint: CubicBezierEasing = CubicBezierEasing::constant(0.22, 1.0, 0.36, 1.0);
pub const EaseInOutQuint: CubicBezierEasing = CubicBezierEasing::constant(0.83, 0.0, 0.17, 1.0);
pub const EaseInCirc: CubicBezierEasing = CubicBezierEasing::constant(0.55, 0.0, 1.0, 0.45);
pub const EaseOutCirc: CubicBezierEasing = CubicBezierEasing::constant(0.0, 0.55, 0.45, 1.0);
pub const EaseInOutCirc: CubicBezierEasing = CubicBezierEasing::constant(0.85, 0.0, 0.15, 1.0);
pub const EaseInQuad: CubicBezierEasing = CubicBezierEasing::constant(0.11, 0.0, 0.5, 0.0);
pub const EaseOutQuad: CubicBezierEasing = CubicBezierEasing::constant(0.5, 1.0, 0.89, 1.0);
pub const EaseInOutQuad: CubicBezierEasing = CubicBezierEasing::constant(0.45, 0.0, 0.55, 1.0);
pub const EaseInQuart: CubicBezierEasing = CubicBezierEasing::constant(0.5, 0.0, 0.75, 0.0);
pub const EaseOutQuart: CubicBezierEasing = CubicBezierEasing::constant(0.25, 1.0, 0.5, 1.0);
pub const EaseInOutQuart: CubicBezierEasing = CubicBezierEasing::constant(0.76, 0.0, 0.24, 1.0);
pub const EaseInExpo: CubicBezierEasing = CubicBezierEasing::constant(0.7, 0.0, 0.84, 0.0);
pub const EaseOutExpo: CubicBezierEasing = CubicBezierEasing::constant(0.16, 1.0, 0.3, 1.0);
pub const EaseInOutExpo: CubicBezierEasing = CubicBezierEasing::constant(0.87, 0.0, 0.13, 1.0);
pub const EaseInBack: CubicBezierEasing = CubicBezierEasing::constant(0.36, 0.0, 0.66, -0.56);
pub const EaseOutBack: CubicBezierEasing = CubicBezierEasing::constant(0.34, 1.56, 0.64, 1.0);
pub const EaseInOutBack: CubicBezierEasing = CubicBezierEasing::constant(0.68, -0.6, 0.32, 1.6);

/// An easing along a path from (0, 0) to (1, 1) (Compose's PathEasing): the fraction of the time is the path's x,
/// the fraction of the way its y there. The path must be continuous along x (no two segments over one x but at
/// their ends), start at x 0 and end at x 1, and have no close; it is read (as quadratics where it has conics) the
/// first time the easing is used, and an invalid one panics then, as Compose's throws. Equal only to itself.
pub struct PathEasing {
    path: Path,
    intervals: OnceLock<IntervalTree<PathSegment>>,
}

/// Compose's `PathEasing(path)`.
pub fn PathEasing(path: Path) -> PathEasing {
    PathEasing { path, intervals: OnceLock::new() }
}

impl PathEasing {
    #[track_caller]
    fn initialize_easing(&self) -> IntervalTree<PathSegment> {
        let mut roots = [0.0f32; 5];
        let mut intervals = IntervalTree::new();
        for segment in self.path.iterator_with(ConicEvaluation::AsQuadratics, 2e-4) {
            if segment.r#type == PathSegmentType::Close {
                panic_kt("IllegalArgumentException", "The path cannot contain a close() command.");
            }
            if segment.r#type != PathSegmentType::Move && segment.r#type != PathSegmentType::Done {
                let (start, end) = compute_horizontal_bounds(&segment, &mut roots);
                intervals.add_interval(start, end, Some(segment));
            }
        }
        if !(intervals.contains(0.0) && intervals.contains(1.0)) {
            panic_kt("IllegalArgumentException", "The easing path must start at 0.0f and end at 1.0f.");
        }
        intervals
    }
}

impl Easing for PathEasing {
    #[track_caller]
    fn transform(&self, fraction: f32) -> f32 {
        if fraction <= 0.0 {
            return 0.0;
        } else if fraction >= 1.0 {
            return 1.0;
        }
        let intervals = self.intervals.get_or_init(|| self.initialize_easing());
        let segment = intervals.find_first_overlap(fraction, fraction).and_then(|i| i.data.as_ref());
        let Some(segment) = segment else {
            panic_kt("IllegalStateException", "The easing path is invalid. Make sure it is continuous on the x axis.")
        };
        let t = find_first_root(segment, fraction);
        if t.is_nan() {
            panic_kt(
                "IllegalStateException",
                "The easing path is invalid. Make sure it does not contain NaN/Infinity values.",
            );
        }
        evaluate_y(segment, t)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl std::fmt::Debug for PathEasing {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("PathEasing")
    }
}
