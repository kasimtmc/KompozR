//! Kotlin's ranges and progressions: `1..10` ([`range_to`]), `0 until n` and `0..<n` ([`until`]), `10 downTo 1`
//! ([`down_to`]), `r step 2`, `r.reversed()`, `x in r` ([`is_in`]), `for (i in r)`.

use super::panic_kt;
use std::cmp::Ordering;

/// What a progression goes over: Kotlin's `Int`, `Long`, `Char` (and the smaller and unsigned integers, which
/// Kotlin widens). Its values as `i128`, where every step fits.
pub trait KtStep: KtRangeBound + Copy + 'static {
    /// The smallest value (what `until` below it is empty at).
    const MIN: i128;
    fn to_i128(self) -> i128;
    /// The value of `v`, or none (a `char` that is a surrogate, which Rust's `char` can't be).
    fn from_i128(v: i128) -> Option<Self>;
}

macro_rules! steps {
    ($($t:ty),*) => {$(
        impl KtStep for $t {
            const MIN: i128 = <$t>::MIN as i128;
            fn to_i128(self) -> i128 {
                self as i128
            }
            fn from_i128(v: i128) -> Option<Self> {
                <$t>::try_from(v).ok()
            }
        }
    )*};
}
steps!(i8, i16, i32, i64, u8, u16, u32, u64, isize, usize);

impl KtStep for char {
    const MIN: i128 = 0;
    fn to_i128(self) -> i128 {
        self as i128
    }
    fn from_i128(v: i128) -> Option<char> {
        u32::try_from(v).ok().and_then(char::from_u32)
    }
}

/// Kotlin's ranges and progressions: `IntRange`/`IntProgression` (and `LongRange`, `CharRange`, … : of any
/// [`KtStep`]), from `first` to `last` (both in) by `step`, `last` the last value actually reached; and the ranges of
/// what isn't counted (`0.0..1.0`, `"a".."m"`, `0.0..<1.0`: Kotlin's `ClosedRange` and `OpenEndRange`), only asked
/// what is in them. Iterating a counted one gives its values; a char progression passes over the surrogates (no
/// `char` is one).
#[derive(Clone, Copy, Debug)]
pub struct KtProgression<T> {
    first: T,
    last: T,
    step: i64,
    /// A range (`a..b`, `a until b`), not a progression (`downTo`, `step`, `reversed`): its text and equality are
    /// a range's.
    range: bool,
    /// `a..<b` of what isn't counted: `last` is left out.
    open_end: bool,
}

/// `a % b` kept positive, as Kotlin's `mod` (b > 0).
fn modulo(a: i128, b: i128) -> i128 {
    a.rem_euclid(b)
}

/// The last value a progression from `start` toward `end` by `step` reaches (Kotlin's getProgressionLastElement).
fn last_element(start: i128, end: i128, step: i128) -> i128 {
    let diff = |a: i128, b: i128, c: i128| modulo(modulo(a, c) - modulo(b, c), c);
    if step > 0 {
        if start >= end { end } else { end - diff(end, start, step) }
    } else if start <= end {
        end
    } else {
        end + diff(start, end, -step)
    }
}

impl<T> KtProgression<T> {
    /// `first..last`.
    pub fn range(first: T, last: T) -> KtProgression<T> {
        KtProgression { first, last, step: 1, range: true, open_end: false }
    }

    /// `start..<end` of what isn't counted (Kotlin's `OpenEndRange`).
    pub fn open_end(start: T, end_exclusive: T) -> KtProgression<T> {
        KtProgression { first: start, last: end_exclusive, step: 1, range: true, open_end: true }
    }

    pub fn step(&self) -> i64 {
        self.step
    }
    /// Whether it is a range (`a..b`) rather than another progression.
    pub fn is_range(&self) -> bool {
        self.range
    }
    /// Whether its end is left out (`a..<b` of what isn't counted).
    pub fn is_open_end(&self) -> bool {
        self.open_end
    }
    /// Its first value, borrowed.
    pub fn start_ref(&self) -> &T {
        &self.first
    }
    /// Its last value (or the end it leaves out), borrowed.
    pub fn last_ref(&self) -> &T {
        &self.last
    }
}

impl<T: Clone> KtProgression<T> {
    /// Its first value (Kotlin's `first` property; the field, also when it is empty).
    pub fn first(&self) -> T {
        self.first.clone()
    }
    /// Its last value (Kotlin's `last` property).
    pub fn last(&self) -> T {
        self.last.clone()
    }
    /// Kotlin's `start` (a range's).
    pub fn start(&self) -> T {
        self.first.clone()
    }
    /// Kotlin's `endInclusive` (a range's).
    pub fn end_inclusive(&self) -> T {
        self.last.clone()
    }
}

impl<T: KtRangeBound> KtProgression<T> {
    /// `x in r`: one of its values (of a counted one), between its bounds (of another).
    pub fn contains(&self, value: T) -> bool {
        T::kt_contains(self, &value)
    }
    pub fn is_empty(&self) -> bool {
        T::kt_is_empty(self)
    }
}

impl<T: KtStep> KtProgression<T> {
    /// Kotlin's `IntProgression.fromClosedRange(start, end, step)`: `last` the last value reached. Panics, as
    /// Kotlin throws, for a step of zero.
    #[track_caller]
    pub fn from_closed_range(start: T, end: T, step: i64) -> KtProgression<T> {
        if step == 0 {
            panic_kt("IllegalArgumentException", "Step must be non-zero.");
        }
        if step == i64::MIN {
            panic_kt(
                "IllegalArgumentException",
                "Step must be greater than Long.MIN_VALUE to avoid overflow on negation.",
            );
        }
        let last = last_element(start.to_i128(), end.to_i128(), step as i128);
        let last = T::from_i128(last).unwrap_or(end);
        KtProgression { first: start, last, step, range: false, open_end: false }
    }

    /// Kotlin's `endExclusive` (a range's): one past its last.
    pub fn end_exclusive(&self) -> T {
        T::from_i128(self.last.to_i128() + 1).unwrap_or(self.last)
    }

    /// `r step n`: the same bounds by `n` (the direction kept). Panics, as Kotlin throws, for `n <= 0`.
    #[track_caller]
    pub fn step_by(self, step: i64) -> KtProgression<T> {
        if step <= 0 {
            panic_kt("IllegalArgumentException", &format!("Step must be positive, was: {step}."));
        }
        KtProgression::from_closed_range(self.first, self.last, if self.step > 0 { step } else { -step })
    }

    /// `r.reversed()`: from its last to its first.
    pub fn reversed(self) -> KtProgression<T> {
        KtProgression::from_closed_range(self.last, self.first, -self.step)
    }

    /// How many values it has.
    pub fn count(&self) -> i32 {
        if self.is_empty() {
            return 0;
        }
        let n = (self.last.to_i128() - self.first.to_i128()) / self.step as i128 + 1;
        n.min(i32::MAX as i128) as i32
    }

    pub fn iter(&self) -> KtProgressionIter<T> {
        KtProgressionIter {
            next: self.first.to_i128(),
            last: self.last.to_i128(),
            step: self.step as i128,
            done: self.is_empty(),
            _t: std::marker::PhantomData,
        }
    }
}

/// Kotlin's: two ranges are equal by their bounds, two progressions by their bounds and steps, two empty ones
/// always (of one kind).
impl<T: KtRangeBound> PartialEq for KtProgression<T> {
    fn eq(&self, other: &Self) -> bool {
        self.range == other.range
            && self.open_end == other.open_end
            && (self.is_empty() && other.is_empty()
                || self.first == other.first && self.last == other.last && self.step == other.step)
    }
}

/// A progression's values in order.
#[derive(Clone, Debug)]
pub struct KtProgressionIter<T> {
    next: i128,
    last: i128,
    step: i128,
    done: bool,
    _t: std::marker::PhantomData<T>,
}

impl<T: KtStep> Iterator for KtProgressionIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        loop {
            if self.done {
                return None;
            }
            let v = self.next;
            if v == self.last {
                self.done = true;
            } else {
                self.next += self.step;
            }
            if let Some(v) = T::from_i128(v) {
                return Some(v);
            }
        }
    }
}

impl<T: KtStep> IntoIterator for KtProgression<T> {
    type Item = T;
    type IntoIter = KtProgressionIter<T>;
    fn into_iter(self) -> KtProgressionIter<T> {
        self.iter()
    }
}

impl<T: KtStep> IntoIterator for &KtProgression<T> {
    type Item = T;
    type IntoIter = KtProgressionIter<T>;
    fn into_iter(self) -> KtProgressionIter<T> {
        self.iter()
    }
}

/// What a range's bounds are: a counted type (a [`KtStep`]: its range a progression), or anything else ordered
/// (`Float`, `Double`, `String`: only asked what is in it). `a until b`/`a..<b` of a counted type ends at `b - 1`,
/// of another leaves `b` out.
pub trait KtRangeBound: Clone + PartialOrd {
    fn kt_until(start: Self, end: Self) -> KtProgression<Self>;
    fn kt_contains(range: &KtProgression<Self>, value: &Self) -> bool;
    fn kt_is_empty(range: &KtProgression<Self>) -> bool;
}

macro_rules! counted {
    ($($t:ty),*) => {$(
        impl KtRangeBound for $t {
            fn kt_until(start: $t, end: $t) -> KtProgression<$t> {
                // (below the smallest value: empty, as Kotlin's IntRange.EMPTY is 1..0)
                match <$t as KtStep>::from_i128(end.to_i128() - 1) {
                    Some(last) => KtProgression::range(start, last),
                    None => KtProgression::range(
                        <$t as KtStep>::from_i128(1).unwrap(),
                        <$t as KtStep>::from_i128(0).unwrap(),
                    ),
                }
            }
            fn kt_contains(r: &KtProgression<$t>, value: &$t) -> bool {
                if Self::kt_is_empty(r) {
                    return false;
                }
                let (v, first, last, step) = (value.to_i128(), r.first.to_i128(), r.last.to_i128(), r.step as i128);
                let within = if step > 0 { first <= v && v <= last } else { last <= v && v <= first };
                within && (v - first) % step == 0
            }
            fn kt_is_empty(r: &KtProgression<$t>) -> bool {
                if r.step > 0 { r.first > r.last } else { r.first < r.last }
            }
        }
    )*};
}
counted!(i8, i16, i32, i64, u8, u16, u32, u64, isize, usize, char);

/// A range of what isn't counted.
macro_rules! ordered {
    ($($t:ty),*) => {$(
        impl KtRangeBound for $t {
            fn kt_until(start: $t, end: $t) -> KtProgression<$t> {
                KtProgression::open_end(start, end)
            }
            fn kt_contains(r: &KtProgression<$t>, value: &$t) -> bool {
                ordered_contains(r, value)
            }
            fn kt_is_empty(r: &KtProgression<$t>) -> bool {
                ordered_is_empty(r)
            }
        }
    )*};
}
ordered!(f32, f64, String);

impl KtRangeBound for &str {
    fn kt_until(start: Self, end: Self) -> KtProgression<Self> {
        KtProgression::open_end(start, end)
    }
    fn kt_contains(r: &KtProgression<Self>, value: &Self) -> bool {
        ordered_contains(r, value)
    }
    fn kt_is_empty(r: &KtProgression<Self>) -> bool {
        ordered_is_empty(r)
    }
}

/// Whether a range of what isn't counted holds `value`: between its bounds (its end left out of an open one).
pub fn ordered_contains<T: PartialOrd>(r: &KtProgression<T>, value: &T) -> bool {
    r.first <= *value && if r.open_end { *value < r.last } else { *value <= r.last }
}

/// Whether a range of what isn't counted holds nothing.
pub fn ordered_is_empty<T: PartialOrd>(r: &KtProgression<T>) -> bool {
    let order = r.first.partial_cmp(&r.last);
    if r.open_end {
        order != Some(std::cmp::Ordering::Less)
    } else {
        !matches!(order, Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal))
    }
}

/// Kotlin's `a..b`.
pub fn range_to<T: KtRangeBound>(start: T, end_inclusive: T) -> KtProgression<T> {
    KtProgression::range(start, end_inclusive)
}

/// Kotlin's `a until b` and `a..<b`.
pub fn until<T: KtRangeBound>(start: T, end: T) -> KtProgression<T> {
    T::kt_until(start, end)
}

/// Kotlin's `a downTo b`.
pub fn down_to<T: KtStep>(start: T, end: T) -> KtProgression<T> {
    KtProgression::from_closed_range(start, end, -1)
}

/// Kotlin's `r step n`.
#[track_caller]
pub fn step<T: KtStep>(progression: KtProgression<T>, step: i64) -> KtProgression<T> {
    progression.step_by(step)
}

/// What `x in c` asks: a range's, a collection's, a map's keys, a string's parts.
pub trait KtContains<X: ?Sized> {
    fn kt_contains(&self, value: &X) -> bool;
}

impl<T: KtRangeBound> KtContains<T> for KtProgression<T> {
    fn kt_contains(&self, value: &T) -> bool {
        T::kt_contains(self, value)
    }
}
impl<C: KtContains<X> + ?Sized, X: ?Sized> KtContains<X> for &C {
    fn kt_contains(&self, value: &X) -> bool {
        (**self).kt_contains(value)
    }
}

/// Kotlin's `ClosedRange<T>` (and `ClosedFloatingPointRange<T>`, what a `Float` or `Double` `a..b` is: no
/// progression, nothing to count over) as a Rust `RangeInclusive<T>` holds it: its ends, `isEmpty()`,
/// `contains(value)` (`value in r`).
pub trait KtClosedRange {
    type Item;
    /// Kotlin: `start: T`
    fn start(&self) -> Self::Item;
    /// Kotlin: `endInclusive: T`
    fn end_inclusive(&self) -> Self::Item;
    /// Whether it holds nothing: its start not at or below its end (a NaN end makes it empty).
    /// Kotlin: `isEmpty(): Boolean`
    fn is_empty(&self) -> bool;
    /// Kotlin: `contains(value: T): Boolean`
    fn contains(&self, value: Self::Item) -> bool;
}

impl<T: PartialOrd + Clone> KtClosedRange for std::ops::RangeInclusive<T> {
    type Item = T;
    fn start(&self) -> T {
        std::ops::RangeInclusive::start(self).clone()
    }
    fn end_inclusive(&self) -> T {
        self.end().clone()
    }
    fn is_empty(&self) -> bool {
        // (Kotlin's `!(start <= endInclusive)`)
        std::ops::RangeInclusive::start(self).partial_cmp(self.end()).is_none_or(|o| o == Ordering::Greater)
    }
    fn contains(&self, value: T) -> bool {
        std::ops::RangeInclusive::start(self) <= &value && value <= *self.end()
    }
}

impl<T: PartialOrd> KtContains<T> for std::ops::RangeInclusive<T> {
    fn kt_contains(&self, value: &T) -> bool {
        std::ops::RangeInclusive::start(self) <= value && value <= self.end()
    }
}

/// Kotlin's `x in c` (`!in` is its negation).
pub fn is_in<X: ?Sized, C: KtContains<X> + ?Sized>(value: &X, container: &C) -> bool {
    container.kt_contains(value)
}

/// What `withIndex()` gives (Kotlin's `IndexedValue`): `for ((i, v) in list.withIndex())`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IndexedValue<T> {
    pub index: i32,
    pub value: T,
}

impl<T: Clone> IndexedValue<T> {
    pub fn index(&self) -> i32 {
        self.index
    }
    pub fn value(&self) -> T {
        self.value.clone()
    }
    pub fn component1(&self) -> i32 {
        self.index
    }
    pub fn component2(&self) -> T {
        self.value.clone()
    }
}

/// Orders two values as Kotlin's `compareTo` does for what `PartialOrd` leaves unordered (a NaN): after
/// everything, equal to itself (Java's `Double.compare`).
pub(crate) fn kt_compare<T: PartialOrd>(a: &T, b: &T) -> Ordering {
    match a.partial_cmp(b) {
        Some(o) => o,
        None => {
            let a_nan = a.partial_cmp(a).is_none();
            let b_nan = b.partial_cmp(b).is_none();
            match (a_nan, b_nan) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                _ => Ordering::Less,
            }
        }
    }
}
