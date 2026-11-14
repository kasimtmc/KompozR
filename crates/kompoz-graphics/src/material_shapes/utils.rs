//! What the shapes library works with (androidx.graphics.shapes' Utils and Point): points as offsets, Kotlin's
//! float math (worked out in double, as the JVM does), the epsilons it compares with.

use crate::geometry::Offset;

pub(crate) type Point = Offset;

/// When two points are the same, within a reasonable roundoff (not larger than a pixel on any display).
pub(crate) const DISTANCE_EPSILON: f32 = 1e-4;
pub(crate) const ANGLE_EPSILON: f32 = 1e-6;
/// For what people see as collinear, more relaxed than what is.
pub(crate) const RELAXED_DISTANCE_EPSILON: f32 = 5e-3;

pub(crate) const FLOAT_PI: f32 = std::f64::consts::PI as f32;
pub(crate) const TWO_PI: f32 = 2.0 * std::f64::consts::PI as f32;

/// Kotlin's Float.MIN_VALUE: the smallest positive float.
pub(crate) const KOTLIN_FLOAT_MIN_VALUE: f32 = f32::from_bits(1);

// Kotlin's float functions (kotlin.math on a Float: the double's, rounded).
pub(crate) fn cos(x: f32) -> f32 {
    (x as f64).cos() as f32
}
pub(crate) fn sin(x: f32) -> f32 {
    (x as f64).sin() as f32
}
pub(crate) fn tan(x: f32) -> f32 {
    (x as f64).tan() as f32
}
pub(crate) fn atan2(y: f32, x: f32) -> f32 {
    (y as f64).atan2(x as f64) as f32
}

pub(crate) fn distance(x: f32, y: f32) -> f32 {
    (x * x + y * y).sqrt()
}
pub(crate) fn distance_squared(x: f32, y: f32) -> f32 {
    x * x + y * y
}

/// The unit vector from (0, 0) toward (x, y). Panics for none, as Kotlin's require throws.
pub(crate) fn direction_vector(x: f32, y: f32) -> Point {
    let d = distance(x, y);
    assert!(d > 0.0, "Required distance greater than zero");
    Point::new(x / d, y / d)
}

pub(crate) fn direction_vector_of_angle(angle_radians: f32) -> Point {
    Point::new(cos(angle_radians), sin(angle_radians))
}

pub(crate) fn radial_to_cartesian(radius: f32, angle_radians: f32, center: Point) -> Point {
    direction_vector_of_angle(angle_radians) * radius + center
}

pub(crate) fn rotate90(p: Point) -> Point {
    Point::new(-p.y, p.x)
}

pub(crate) fn dot(p: Point, q: Point) -> f32 {
    p.x * q.x + p.y * q.y
}

/// The z of the cross product: whether `q` goes clockwise from `p` (> 0).
pub(crate) fn clockwise(p: Point, q: Point) -> bool {
    p.x * q.y - p.y * q.x > 0.0
}

/// The unit vector of `p`. Panics for none.
pub(crate) fn get_direction(p: Point) -> Point {
    let d = p.get_distance();
    assert!(d > 0.0, "Can't get the direction of a 0-length vector");
    p / d
}

pub(crate) fn square(x: f32) -> f32 {
    x * x
}

/// Between `start` and `stop` (the shapes library's own: `(1 - f) start + f stop`).
pub(crate) fn interpolate(start: f32, stop: f32, fraction: f32) -> f32 {
    (1.0 - fraction) * start + fraction * stop
}

pub(crate) fn interpolate_point(start: Point, stop: Point, fraction: f32) -> Point {
    Point::new(interpolate(start.x, stop.x, fraction), interpolate(start.y, stop.y, fraction))
}

/// `num % mod`, never negative.
pub(crate) fn positive_modulo(num: f32, modulus: f32) -> f32 {
    (num % modulus + modulus) % modulus
}

/// Whether C is on the line through A and B, within `tolerance` (absolute, or relative to the lengths).
#[allow(clippy::too_many_arguments)]
pub(crate) fn collinear_ish(a_x: f32, a_y: f32, b_x: f32, b_y: f32, c_x: f32, c_y: f32, tolerance: f32) -> bool {
    let ab = rotate90(Point::new(b_x - a_x, b_y - a_y));
    let ac = Point::new(c_x - a_x, c_y - a_y);
    let dot_product = dot(ab, ac).abs();
    let relative_tolerance = tolerance * ab.get_distance() * ac.get_distance();
    dot_product < tolerance || dot_product < relative_tolerance
}

/// Whether the corner at `current` is convex (as the library approximates it).
pub(crate) fn convex(previous: Point, current: Point, next: Point) -> bool {
    clockwise(current - previous, next - current)
}

/// Kotlin's Float.toString (Java's): plain decimal from 10^-3 to 10^7, else "d.dddE±n"; the shortest digits.
pub(crate) fn java_float_string(v: f32) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    let a = v.abs();
    if a == 0.0 || (1e-3..1e7).contains(&a) {
        return format!("{v:?}"); // (decimal here, with a ".0" on whole numbers)
    }
    let s = format!("{v:e}"); // "1.5e-5", "1e10"
    let (mantissa, exponent) = s.split_once('e').unwrap_or((&s, "0"));
    let mantissa = if mantissa.contains('.') { mantissa.to_string() } else { format!("{mantissa}.0") };
    format!("{mantissa}E{exponent}")
}

/// Kotlin's String.toFloat (Java's parseFloat): whitespace around it and a trailing f or d allowed. Panics, as it
/// throws, for what isn't a number.
pub(crate) fn java_parse_float(s: &str) -> f32 {
    let t = s.trim();
    let t = t.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(t);
    let t = match t {
        "Infinity" | "+Infinity" => "inf",
        "-Infinity" => "-inf",
        _ => t,
    };
    t.parse::<f32>().unwrap_or_else(|_| panic!("NumberFormatException: For input string: \"{s}\""))
}
