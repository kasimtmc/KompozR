//! Animated vector drawables (Compose's AnimatedImageVector): a vector and how its named groups', paths' and its
//! own properties change over time, read from an Android `<animated-vector>` in one file.

use super::image_vector::ImageVector;
use super::node::{PathNode, lerp_path_nodes};
use super::parser::add_path_nodes;
use super::vector_xml::{VectorValues, vector_of};
use super::xml::{XmlNode, leading_float, read_xml_tree, xml_color, xml_number};
use crate::color::{Color, animated_color};
use crate::path::{compute_cubic_vertical_bounds, evaluate_cubic, find_first_cubic_root};
use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::fmt;
use std::sync::Arc;

// ---- Android's interpolators -------------------------------------------------------------------------------------

/// Compose's CubicBezierEasing: the curve from (0, 0) to (1, 1) through (a, b) and (c, d), its y where its x is the
/// fraction, kept within the curve's own bounds.
#[derive(Clone, Copy, Debug)]
struct CubicBezierEasing {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    min: f32,
    max: f32,
}

/// 1.0's ulp (Compose's OneUlpAt1).
const OneUlpAt1: f32 = 1.192_092_9e-7;

impl CubicBezierEasing {
    const fn new(a: f32, b: f32, c: f32, d: f32) -> CubicBezierEasing {
        CubicBezierEasing { a, b, c, d, min: f32::NAN, max: f32::NAN }
    }

    /// Its bounds worked out (Compose's init).
    fn bounded(self) -> CubicBezierEasing {
        let (min, max) = compute_cubic_vertical_bounds(0.0, self.b, self.d, 1.0);
        CubicBezierEasing { min, max, ..self }
    }

    fn transform(&self, fraction: f32) -> f32 {
        if fraction > 0.0 && fraction < 1.0 {
            let f = fraction.max(OneUlpAt1);
            let t = find_first_cubic_root(0.0 - f, self.a - f, self.c - f, 1.0 - f);
            if t.is_nan() {
                // (where Compose throws: none of Android's curves has no solution)
                return fraction;
            }
            // (Kotlin's fastCoerceIn)
            let mut y = evaluate_cubic(self.b, self.d, t);
            if y < self.min {
                y = self.min;
            }
            if y > self.max {
                y = self.max;
            }
            y
        } else {
            fraction
        }
    }
}

/// How an animator's fraction goes, as Compose plays Android's interpolators.
#[derive(Clone, Copy, Debug)]
enum Interpolator {
    Linear,
    /// AccelerateInterpolator: t².
    Accelerate,
    /// DecelerateInterpolator: 1 - (1 - t)².
    Decelerate,
    /// AccelerateDecelerateInterpolator (an animator's default): cos((t + 1)π) / 2 + 0.5.
    AccelerateDecelerate,
    Cubic(CubicBezierEasing),
}

impl Interpolator {
    /// Android's interpolator `name` (`@android:interpolator/fast_out_slow_in`, `@android:anim/linear_interpolator`,
    /// …): fast_out_slow_in, fast_out_linear_in and linear_out_slow_in as Compose's easings, linear, accelerate (and
    /// accelerate_quad), decelerate, accelerate_decelerate; another accelerate_decelerate, the default.
    fn named(name: &str) -> Interpolator {
        let is = |s: &str| name.contains(s);
        if is("fast_out_slow_in") {
            Interpolator::Cubic(CubicBezierEasing::new(0.4, 0.0, 0.2, 1.0).bounded())
        } else if is("fast_out_linear_in") {
            Interpolator::Cubic(CubicBezierEasing::new(0.4, 0.0, 1.0, 1.0).bounded())
        } else if is("linear_out_slow_in") {
            Interpolator::Cubic(CubicBezierEasing::new(0.0, 0.0, 0.2, 1.0).bounded())
        } else if is("linear") {
            Interpolator::Linear
        } else if is("accelerate_decelerate") {
            Interpolator::AccelerateDecelerate
        } else if is("accelerate") {
            Interpolator::Accelerate
        } else if is("decelerate") {
            Interpolator::Decelerate
        } else {
            Interpolator::AccelerateDecelerate
        }
    }

    fn transform(&self, t: f32) -> f32 {
        match self {
            Interpolator::Linear => t,
            Interpolator::Accelerate => t * t,
            Interpolator::Decelerate => 1.0 - (1.0 - t) * (1.0 - t),
            Interpolator::AccelerateDecelerate => (((t as f64 + 1.0) * PI).cos() / 2.0 + 0.5) as f32,
            Interpolator::Cubic(e) => e.transform(t),
        }
    }
}

// ---- Animators ---------------------------------------------------------------------------------------------------

/// What a property goes between.
#[derive(Clone, Debug)]
enum Values {
    Number(f32, f32),
    Color(Color, Color),
    Path(Vec<PathNode>, Vec<PathNode>),
}

/// One property of a named element going from one value to another (an `<objectAnimator>` or one of its
/// `<propertyValuesHolder>`s), at its time in the whole.
#[derive(Clone, Debug)]
struct Animator {
    target: String,
    property: String,
    values: Values,
    start: f32,
    duration: f32,
    interpolator: Interpolator,
    /// After the first time.
    repeats: i32,
    /// repeatMode="reverse".
    reverses: bool,
}

impl Animator {
    fn end(&self) -> f32 {
        self.start + self.duration * (self.repeats + 1) as f32
    }

    /// How far along it is at `t` (0..1, through its interpolator): nothing before its start, its last after its
    /// end.
    fn fraction(&self, t: f32) -> Option<f32> {
        if t < self.start {
            return None;
        }
        let local = t - self.start;
        let mut turn = if self.duration > 0.0 { (local / self.duration).floor() as i32 } else { self.repeats + 1 };
        let mut f = 1.0;
        if turn <= self.repeats {
            f = (local - turn as f32 * self.duration) / self.duration;
        } else {
            turn = self.repeats;
        }
        if self.reverses && turn % 2 == 1 {
            f = 1.0 - f;
        }
        Some(self.interpolator.transform(f))
    }
}

/// Reads an animator element (an `<objectAnimator>`, a `<set>` of them, together or sequentially) for `target` from
/// `start` into `out`; returns where it ends.
fn read_animators(n: &XmlNode, target: &str, start: f32, vector: &XmlNode, out: &mut Vec<Animator>) -> f32 {
    if n.name == "set" {
        let sequential = n.attribute("ordering") == "sequentially";
        let mut end = start;
        for c in &n.children {
            let child_end = read_animators(c, target, if sequential { end } else { start }, vector, out);
            end = end.max(child_end);
        }
        return end;
    }
    if n.name != "objectAnimator" {
        return start;
    }
    let repeats = xml_number(&n.attributes, "repeatCount", 0.0);
    let base = Animator {
        target: target.to_string(),
        property: String::new(),
        values: Values::Number(0.0, 0.0),
        start: start + xml_number(&n.attributes, "startOffset", 0.0),
        duration: xml_number(&n.attributes, "duration", 300.0),
        interpolator: Interpolator::named(n.attribute("interpolator")),
        // (infinite: played once)
        repeats: if repeats.is_finite() && repeats > 0.0 { repeats as i32 } else { 0 },
        reverses: n.attribute("repeatMode") == "reverse",
    };
    // Its own values: where an animator without valueFrom starts.
    let element = vector.named(target);
    let mut add = |at: &BTreeMap<String, String>| {
        let value = |key: &str| at.get(key).map_or("", String::as_str);
        let property = value("propertyName");
        if property.is_empty() {
            return;
        }
        let (kind, mut from, to) = (value("valueType"), value("valueFrom"), value("valueTo"));
        if from.is_empty()
            && let Some(e) = element
        {
            from = e.attribute(property);
        }
        if to.is_empty() {
            return;
        }
        let values = if kind == "pathType" || property == "pathData" {
            Values::Path(add_path_nodes(from), add_path_nodes(to))
        } else if kind == "colorType" || to.starts_with('#') {
            Values::Color(xml_color(from).unwrap_or(Color::Transparent), xml_color(to).unwrap_or(Color::Transparent))
        } else {
            Values::Number(leading_float(from).unwrap_or(0.0), leading_float(to).unwrap_or(0.0))
        };
        out.push(Animator { property: property.to_string(), values, ..base.clone() });
    };
    add(&n.attributes);
    for holder in n.children.iter().filter(|h| h.name == "propertyValuesHolder") {
        add(&holder.attributes);
    }
    base.end()
}

// ---- Animated vectors --------------------------------------------------------------------------------------------

struct AnimatedData {
    vector: XmlNode,
    /// In the order they start.
    animators: Vec<Animator>,
    duration: f32,
    start: ImageVector,
}

/// An animated vector drawable (Compose's AnimatedImageVector): a vector ([`image_vector`](Self::image_vector),
/// as it starts) and how its named groups', paths' and its own properties change over time
/// ([`vector_at`](Self::vector_at)). Copies share it; equal when they are the same one.
#[derive(Clone)]
pub struct AnimatedImageVector {
    data: Arc<AnimatedData>,
}

impl AnimatedImageVector {
    /// An Android `<animated-vector>` in one file (as Android Studio and Shape Shifter write it): its `<vector>` in
    /// `<aapt:attr name="android:drawable">`, each `<target android:name>`'s animation in `<aapt:attr
    /// name="android:animation">`: `<objectAnimator>`s (a propertyName, or `<propertyValuesHolder>`s; valueFrom,
    /// else the element's own value, and valueTo; valueType float, color or path; duration, startOffset,
    /// repeatCount, repeatMode, Android's interpolators: linear, accelerate, decelerate, accelerate_decelerate,
    /// fast_out_slow_in, fast_out_linear_in, linear_out_slow_in) in `<set>`s together or sequentially. Animated:
    /// groups' rotation, pivot, scale, translation; paths' pathData (morphed between path data of the same
    /// commands, else switched halfway: [`lerp_path_nodes`]), fill and stroke colors (as Compose animates colors,
    /// [`animated_color`]) and alphas, strokeWidth, trimPath…; clip-paths' pathData; the vector's alpha. Not read:
    /// animators in files of their own (`@animator/…`), keyframes, path interpolators; an infinite repeat plays
    /// once. None if it isn't one.
    pub fn from_animated_vector_xml(xml: &str) -> Option<AnimatedImageVector> {
        let root = read_xml_tree(xml)?;
        if root.name != "animated-vector" {
            return None;
        }
        let vector = root.find("vector")?.clone();
        let mut animators = Vec::new();
        let mut duration = 0.0f32;
        for target in root.children.iter().filter(|t| t.name == "target") {
            // <aapt:attr name="android:animation">
            for attr in target.children.iter().filter(|a| a.name == "attr") {
                for animator in &attr.children {
                    let end = read_animators(animator, target.attribute("name"), 0.0, &vector, &mut animators);
                    duration = duration.max(end);
                }
            }
        }
        animators.sort_by(|a, b| a.start.total_cmp(&b.start));
        let start = vector_of(&vector, &VectorValues::default());
        Some(AnimatedImageVector { data: Arc::new(AnimatedData { vector, animators, duration, start }) })
    }

    /// How long all of it takes to play (Compose's totalDuration), to the nearest millisecond.
    pub fn total_duration_millis(&self) -> i32 {
        self.data.duration.round() as i32
    }

    /// The vector as it starts (Compose's imageVector).
    pub fn image_vector(&self) -> ImageVector {
        self.data.start.clone()
    }

    /// The vector `millis` into its animations: each property as its animators have it then (one started later
    /// over one started earlier), held at its last value after its animator ends, its own before one starts.
    pub fn vector_at(&self, millis: f32) -> ImageVector {
        let mut values = VectorValues::default();
        for a in &self.data.animators {
            let Some(f) = a.fraction(millis) else { continue };
            let key = (a.target.clone(), a.property.clone());
            match &a.values {
                Values::Number(from, to) => {
                    values.numbers.insert(key, from + (to - from) * f);
                }
                Values::Color(from, to) => {
                    values.colors.insert(key, animated_color(*from, *to, f));
                }
                Values::Path(from, to) => {
                    values.paths.insert(a.target.clone(), lerp_path_nodes(from, to, f));
                }
            }
        }
        vector_of(&self.data.vector, &values)
    }
}

/// The same one.
impl PartialEq for AnimatedImageVector {
    fn eq(&self, o: &AnimatedImageVector) -> bool {
        Arc::ptr_eq(&self.data, &o.data)
    }
}

impl fmt::Debug for AnimatedImageVector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("AnimatedImageVector")
            .field("total_duration_millis", &self.total_duration_millis())
            .field("image_vector", &self.data.start)
            .finish()
    }
}
