//! Kotlin's numbers: conversions and bounds ([`KtNumber`]: `toInt()`, `roundToInt()`, `coerceIn(…)`), sums
//! ([`KtSum`]), `maxOf`/`minOf`, and `kotlin.math` ([`sin`], [`sqrt`], [`PI`]…; a `Float` in, a `Float` out).

use super::panic_kt;
use super::range::kt_compare;
use std::cmp::Ordering;

/// Kotlin's `PI`.
pub const PI: f64 = std::f64::consts::PI;
/// Kotlin's `E`.
pub const E: f64 = std::f64::consts::E;

/// What a sum adds: an integer wraps on overflow, as Kotlin's.
pub trait KtSum: Sized {
    fn kt_zero() -> Self;
    fn kt_add(self, other: Self) -> Self;
}

macro_rules! sum_ints {
    ($($t:ty),*) => {$(
        impl KtSum for $t {
            fn kt_zero() -> $t {
                0
            }
            fn kt_add(self, other: $t) -> $t {
                self.wrapping_add(other)
            }
        }
    )*};
}
sum_ints!(i8, i16, i32, i64, u8, u16, u32, u64, isize, usize);

impl KtSum for f32 {
    fn kt_zero() -> f32 {
        0.0
    }
    fn kt_add(self, other: f32) -> f32 {
        self + other
    }
}
impl KtSum for f64 {
    fn kt_zero() -> f64 {
        0.0
    }
    fn kt_add(self, other: f64) -> f64 {
        self + other
    }
}

/// Kotlin's `maxOf(a, b)`: the greater (a NaN wins, as Java's `Math.max`; on a tie the first).
/// Kotlin: `maxOf(a: T, b: T): T`
pub fn max_of<T: PartialOrd>(a: T, b: T) -> T {
    if kt_compare(&a, &b) == Ordering::Less { b } else { a }
}

/// Kotlin's `minOf(a, b)`: the lesser (a NaN wins; on a tie the first).
/// Kotlin: `minOf(a: T, b: T): T`
pub fn min_of<T: PartialOrd>(a: T, b: T) -> T {
    let a_nan = a.partial_cmp(&a).is_none();
    let b_nan = b.partial_cmp(&b).is_none();
    if a_nan {
        return a;
    }
    if b_nan {
        return b;
    }
    if kt_compare(&b, &a) == Ordering::Less { b } else { a }
}

/// Kotlin's conversions and bounds of a number.
pub trait KtNumber: Copy + PartialOrd {
    /// Kotlin: `toInt(): Int`
    fn to_int(self) -> i32;
    /// Kotlin: `toLong(): Long`
    fn to_long(self) -> i64;
    /// Kotlin: `toFloat(): Float`
    fn to_float(self) -> f32;
    /// Kotlin: `toDouble(): Double`
    fn to_double(self) -> f64;
    /// Kotlin: `toShort(): Short`
    fn to_short(self) -> i16 {
        self.to_long() as i16
    }
    /// Kotlin: `toByte(): Byte`
    fn to_byte(self) -> i8 {
        self.to_long() as i8
    }
    /// Kotlin: `coerceIn(minimumValue: T, maximumValue: T): T`
    #[track_caller]
    fn coerce_in(self, minimum_value: Self, maximum_value: Self) -> Self {
        if minimum_value > maximum_value {
            panic_kt(
                "IllegalArgumentException",
                "Cannot coerce value to an empty range: maximum is less than minimum.",
            );
        }
        if self < minimum_value {
            minimum_value
        } else if self > maximum_value {
            maximum_value
        } else {
            self
        }
    }
    /// Kotlin: `coerceAtLeast(minimumValue: T): T`
    fn coerce_at_least(self, minimum_value: Self) -> Self {
        if self < minimum_value { minimum_value } else { self }
    }
    /// Kotlin: `coerceAtMost(maximumValue: T): T`
    fn coerce_at_most(self, maximum_value: Self) -> Self {
        if self > maximum_value { maximum_value } else { self }
    }
}

macro_rules! numbers {
    ($($t:ty),*) => {$(
        impl KtNumber for $t {
            fn to_int(self) -> i32 {
                self as i32
            }
            fn to_long(self) -> i64 {
                self as i64
            }
            fn to_float(self) -> f32 {
                self as f32
            }
            fn to_double(self) -> f64 {
                self as f64
            }
        }
    )*};
}
numbers!(i8, i16, i32, i64, u8, u16, u32, u64, isize, usize, f32, f64);

/// Kotlin's `Int.toChar()`: the UTF-16 unit of that value (a surrogate's is U+FFFD).
/// Kotlin: `toChar(): Char`
pub fn to_char(code: i32) -> char {
    char::from_u32(code as u16 as u32).unwrap_or('\u{FFFD}')
}

/// `kotlin.math` on a `Float` or a `Double` (the same type back), and Kotlin's rounding.
pub trait KtFloat: Copy + PartialOrd {
    fn to_f64(self) -> f64;
    fn from_f64(v: f64) -> Self;
    /// Kotlin's `roundToInt()`: half up (Java's `Math.round`), within `Int`'s range; a NaN panics as Kotlin throws.
    /// Kotlin: `roundToInt(): Int`
    #[track_caller]
    fn round_to_int(self) -> i32 {
        let v = self.to_f64();
        if v.is_nan() {
            panic_kt("IllegalArgumentException", "Cannot round NaN value.");
        }
        (v + 0.5).floor().clamp(i32::MIN as f64, i32::MAX as f64) as i32
    }
    /// Kotlin: `roundToLong(): Long`
    #[track_caller]
    fn round_to_long(self) -> i64 {
        let v = self.to_f64();
        if v.is_nan() {
            panic_kt("IllegalArgumentException", "Cannot round NaN value.");
        }
        (v + 0.5).floor() as i64
    }
    /// Kotlin: `pow(x: T): T`
    fn pow(self, x: Self) -> Self {
        Self::from_f64(self.to_f64().powf(x.to_f64()))
    }
    /// Kotlin: `absoluteValue: T`
    fn absolute_value(self) -> Self {
        Self::from_f64(self.to_f64().abs())
    }
    /// Kotlin: `sign: T`
    fn sign(self) -> Self {
        sign(self)
    }
}

impl KtFloat for f32 {
    fn to_f64(self) -> f64 {
        self as f64
    }
    fn from_f64(v: f64) -> f32 {
        v as f32
    }
    fn pow(self, x: f32) -> f32 {
        self.powf(x)
    }
}
impl KtFloat for f64 {
    fn to_f64(self) -> f64 {
        self
    }
    fn from_f64(v: f64) -> f64 {
        v
    }
}

// (`kotlin.math`'s functions of one Float or Double, written out: the Kotlin index reads functions, not what a
// macro makes of them)
/// Kotlin's `sin(x)`.
/// Kotlin: `sin(x: Float): Float`
pub fn sin<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::sin(x.to_f64()))
}

/// Kotlin's `cos(x)`.
/// Kotlin: `cos(x: Float): Float`
pub fn cos<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::cos(x.to_f64()))
}

/// Kotlin's `tan(x)`.
/// Kotlin: `tan(x: Float): Float`
pub fn tan<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::tan(x.to_f64()))
}

/// Kotlin's `asin(x)`.
/// Kotlin: `asin(x: Float): Float`
pub fn asin<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::asin(x.to_f64()))
}

/// Kotlin's `acos(x)`.
/// Kotlin: `acos(x: Float): Float`
pub fn acos<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::acos(x.to_f64()))
}

/// Kotlin's `atan(x)`.
/// Kotlin: `atan(x: Float): Float`
pub fn atan<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::atan(x.to_f64()))
}

/// Kotlin's `sinh(x)`.
/// Kotlin: `sinh(x: Float): Float`
pub fn sinh<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::sinh(x.to_f64()))
}

/// Kotlin's `cosh(x)`.
/// Kotlin: `cosh(x: Float): Float`
pub fn cosh<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::cosh(x.to_f64()))
}

/// Kotlin's `tanh(x)`.
/// Kotlin: `tanh(x: Float): Float`
pub fn tanh<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::tanh(x.to_f64()))
}

/// Kotlin's `sqrt(x)`.
/// Kotlin: `sqrt(x: Float): Float`
pub fn sqrt<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::sqrt(x.to_f64()))
}

/// Kotlin's `cbrt(x)`.
/// Kotlin: `cbrt(x: Float): Float`
pub fn cbrt<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::cbrt(x.to_f64()))
}

/// Kotlin's `exp(x)`.
/// Kotlin: `exp(x: Float): Float`
pub fn exp<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::exp(x.to_f64()))
}

/// Kotlin's `ln(x)`.
/// Kotlin: `ln(x: Float): Float`
pub fn ln<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::ln(x.to_f64()))
}

/// Kotlin's `log10(x)`.
/// Kotlin: `log10(x: Float): Float`
pub fn log10<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::log10(x.to_f64()))
}

/// Kotlin's `log2(x)`.
/// Kotlin: `log2(x: Float): Float`
pub fn log2<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::log2(x.to_f64()))
}

/// Kotlin's `floor(x)`.
/// Kotlin: `floor(x: Float): Float`
pub fn floor<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::floor(x.to_f64()))
}

/// Kotlin's `ceil(x)`.
/// Kotlin: `ceil(x: Float): Float`
pub fn ceil<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::ceil(x.to_f64()))
}

/// Kotlin's `truncate(x)`.
/// Kotlin: `truncate(x: Float): Float`
pub fn truncate<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::trunc(x.to_f64()))
}

/// Kotlin's `round(x)`: to the nearest, a half to the even one (Java's `Math.rint`).
/// Kotlin: `round(x: Float): Float`
pub fn round<F: KtFloat>(x: F) -> F {
    F::from_f64(f64::round_ties_even(x.to_f64()))
}

/// Kotlin's `sign(x)`: -1, 0 or 1 (a NaN's is NaN, a zero keeps its sign).
pub fn sign<F: KtFloat>(x: F) -> F {
    let v = x.to_f64();
    F::from_f64(if v.is_nan() || v == 0.0 { v } else { v.signum() })
}
/// Kotlin: `atan2(y: Float, x: Float): Float`
pub fn atan2<F: KtFloat>(y: F, x: F) -> F {
    F::from_f64(y.to_f64().atan2(x.to_f64()))
}
/// Kotlin: `hypot(x: Float, y: Float): Float`
pub fn hypot<F: KtFloat>(x: F, y: F) -> F {
    F::from_f64(x.to_f64().hypot(y.to_f64()))
}
/// Kotlin: `pow(x: Float, y: Float): Float`
pub fn pow<F: KtFloat>(x: F, y: F) -> F {
    x.pow(y)
}

/// Kotlin's `abs(x)` of any number (an `Int`'s smallest stays itself, as Kotlin's).
pub trait KtAbs: Sized {
    fn kt_abs(self) -> Self;
}
macro_rules! abs_ints {
    ($($t:ty),*) => {$(
        impl KtAbs for $t {
            fn kt_abs(self) -> $t {
                self.wrapping_abs()
            }
        }
    )*};
}
abs_ints!(i8, i16, i32, i64, isize);
impl KtAbs for f32 {
    fn kt_abs(self) -> f32 {
        self.abs()
    }
}
impl KtAbs for f64 {
    fn kt_abs(self) -> f64 {
        self.abs()
    }
}

/// Kotlin: `abs(x: Int): Int`
pub fn abs<N: KtAbs>(x: N) -> N {
    x.kt_abs()
}

/// `kotlin.math`'s `max(a, b)`: as [`max_of`].
/// Kotlin: `max(a: T, b: T): T`
pub fn max<T: PartialOrd>(a: T, b: T) -> T {
    max_of(a, b)
}

/// `kotlin.math`'s `min(a, b)`: as [`min_of`].
/// Kotlin: `min(a: T, b: T): T`
pub fn min<T: PartialOrd>(a: T, b: T) -> T {
    min_of(a, b)
}
