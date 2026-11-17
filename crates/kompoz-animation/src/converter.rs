//! Values to vectors and back (Compose's VectorConverters.kt) and the distances under which an animation of a type
//! counts as there (VisibilityThresholds.kt).

use crate::vector::{AnimationVector, AnimationVector1D, AnimationVector2D, AnimationVector4D};
use kompoz_graphics::{Dp, DpOffset, DpSize, IntOffset, IntSize, Offset, Rect, Size};
use std::sync::Arc;

enum Convert<A, B> {
    Fn(fn(A) -> B),
    Shared(Arc<dyn Fn(A) -> B + Send + Sync>),
}

impl<A, B> Clone for Convert<A, B> {
    fn clone(&self) -> Self {
        match self {
            Convert::Fn(f) => Convert::Fn(*f),
            Convert::Shared(f) => Convert::Shared(f.clone()),
        }
    }
}

impl<A, B> Convert<A, B> {
    fn call(&self, a: A) -> B {
        match self {
            Convert::Fn(f) => f(a),
            Convert::Shared(f) => f(a),
        }
    }
    fn same(&self, other: &Convert<A, B>) -> bool {
        match (self, other) {
            (Convert::Fn(a), Convert::Fn(b)) => std::ptr::fn_addr_eq(*a, *b),
            (Convert::Shared(a), Convert::Shared(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

/// Turns a value into an animation vector and back (Compose's TwoWayConverter). Two are equal when they are the
/// same converter (a clone of it, or one of the same functions), as Compose compares its objects.
pub struct TwoWayConverter<T, V> {
    convert_to_vector: Convert<T, V>,
    convert_from_vector: Convert<V, T>,
    /// The type a predefined one is of (`f32::VectorConverter`'s "f32"): what it is told apart by.
    tag: Option<&'static str>,
}

impl<T, V> Clone for TwoWayConverter<T, V> {
    fn clone(&self) -> Self {
        TwoWayConverter {
            convert_to_vector: self.convert_to_vector.clone(),
            convert_from_vector: self.convert_from_vector.clone(),
            tag: self.tag,
        }
    }
}

impl<T, V> PartialEq for TwoWayConverter<T, V> {
    fn eq(&self, other: &Self) -> bool {
        match (self.tag, other.tag) {
            (Some(a), Some(b)) => a == b,
            (None, None) => {
                self.convert_to_vector.same(&other.convert_to_vector)
                    && self.convert_from_vector.same(&other.convert_from_vector)
            }
            _ => false,
        }
    }
}

impl<T, V> std::fmt::Debug for TwoWayConverter<T, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("TwoWayConverter")
    }
}

impl<T, V: AnimationVector> TwoWayConverter<T, V> {
    /// A converter of two functions (what can be a `const`: the converters below).
    pub const fn new(convert_to_vector: fn(T) -> V, convert_from_vector: fn(V) -> T) -> TwoWayConverter<T, V> {
        TwoWayConverter {
            convert_to_vector: Convert::Fn(convert_to_vector),
            convert_from_vector: Convert::Fn(convert_from_vector),
            tag: None,
        }
    }

    /// One of the predefined converters, told apart by `tag`.
    const fn predefined(tag: &'static str, to: fn(T) -> V, from: fn(V) -> T) -> TwoWayConverter<T, V> {
        TwoWayConverter { convert_to_vector: Convert::Fn(to), convert_from_vector: Convert::Fn(from), tag: Some(tag) }
    }

    /// The vector of `value`.
    pub fn convert_to_vector(&self, value: T) -> V {
        self.convert_to_vector.call(value)
    }

    /// The value of `vector`.
    pub fn convert_from_vector(&self, vector: V) -> T {
        self.convert_from_vector.call(vector)
    }

    /// The vector of `value`, all zeros (Compose's createZeroVectorFrom).
    pub fn create_zero_vector_from(&self, value: T) -> V {
        let mut v = self.convert_to_vector(value);
        v.reset();
        v
    }
}

/// Compose's `TwoWayConverter(convertToVector, convertFromVector)`: a converter of two closures.
pub fn TwoWayConverter<T, V: AnimationVector>(
    convert_to_vector: impl Fn(T) -> V + Send + Sync + 'static,
    convert_from_vector: impl Fn(V) -> T + Send + Sync + 'static,
) -> TwoWayConverter<T, V> {
    TwoWayConverter {
        convert_to_vector: Convert::Shared(Arc::new(convert_to_vector)),
        convert_from_vector: Convert::Shared(Arc::new(convert_from_vector)),
        tag: None,
    }
}

/// The converter of a type (Compose's `Float.VectorConverter`, `Offset.VectorConverter`, …: `f32::VectorConverter`
/// here, with this trait in scope).
pub trait VectorConverter: Sized + 'static {
    /// The vector it converts to.
    type Vector: AnimationVector;
    const VectorConverter: TwoWayConverter<Self, Self::Vector>;
}

impl VectorConverter for f32 {
    type Vector = AnimationVector1D;
    const VectorConverter: TwoWayConverter<f32, AnimationVector1D> =
        TwoWayConverter::predefined("f32", AnimationVector1D, |it| it.value);
}

impl VectorConverter for i32 {
    type Vector = AnimationVector1D;
    const VectorConverter: TwoWayConverter<i32, AnimationVector1D> =
        TwoWayConverter::predefined("i32", |it| AnimationVector1D(it as f32), |it| it.value as i32);
}

impl VectorConverter for Rect {
    type Vector = AnimationVector4D;
    const VectorConverter: TwoWayConverter<Rect, AnimationVector4D> = TwoWayConverter::predefined(
        "Rect",
        |it| AnimationVector4D(it.left, it.top, it.right, it.bottom),
        |it| Rect::new(it.v1, it.v2, it.v3, it.v4),
    );
}

impl VectorConverter for Dp {
    type Vector = AnimationVector1D;
    const VectorConverter: TwoWayConverter<Dp, AnimationVector1D> =
        TwoWayConverter::predefined("Dp", |it| AnimationVector1D(it.value()), |it| Dp(it.value));
}

impl VectorConverter for DpOffset {
    type Vector = AnimationVector2D;
    const VectorConverter: TwoWayConverter<DpOffset, AnimationVector2D> = TwoWayConverter::predefined(
        "DpOffset",
        |it| AnimationVector2D(it.x.value(), it.y.value()),
        |it| DpOffset::new(Dp(it.v1), Dp(it.v2)),
    );
}

/// (Compose 1.12 has no converter of a DpSize; KompozR's is DpOffset's, its width and height.)
impl VectorConverter for DpSize {
    type Vector = AnimationVector2D;
    const VectorConverter: TwoWayConverter<DpSize, AnimationVector2D> = TwoWayConverter::predefined(
        "DpSize",
        |it| AnimationVector2D(it.width.value(), it.height.value()),
        |it| DpSize::new(Dp(it.v1), Dp(it.v2)),
    );
}

impl VectorConverter for Size {
    type Vector = AnimationVector2D;
    const VectorConverter: TwoWayConverter<Size, AnimationVector2D> =
        TwoWayConverter::predefined("Size", |it| AnimationVector2D(it.width, it.height), |it| Size::new(it.v1, it.v2));
}

impl VectorConverter for Offset {
    type Vector = AnimationVector2D;
    const VectorConverter: TwoWayConverter<Offset, AnimationVector2D> =
        TwoWayConverter::predefined("Offset", |it| AnimationVector2D(it.x, it.y), |it| Offset::new(it.v1, it.v2));
}

impl VectorConverter for IntOffset {
    type Vector = AnimationVector2D;
    const VectorConverter: TwoWayConverter<IntOffset, AnimationVector2D> = TwoWayConverter::predefined(
        "IntOffset",
        |it| AnimationVector2D(it.x as f32, it.y as f32),
        |it| IntOffset::new(fast_round_to_int(it.v1), fast_round_to_int(it.v2)),
    );
}

impl VectorConverter for IntSize {
    type Vector = AnimationVector2D;
    const VectorConverter: TwoWayConverter<IntSize, AnimationVector2D> = TwoWayConverter::predefined(
        "IntSize",
        |it| AnimationVector2D(it.width as f32, it.height as f32),
        |it| IntSize::new(fast_round_to_int(it.v1).max(0), fast_round_to_int(it.v2).max(0)),
    );
}

/// Compose's fastRoundToInt (Math.round: half up, NaN to 0, saturated).
pub(crate) fn fast_round_to_int(v: f32) -> i32 {
    // (in doubles: a float plus a half is exact there, as Java's rounds)
    if v.is_nan() { 0 } else { (v as f64 + 0.5).floor() as i32 }
}

// ---- Visibility thresholds ------------------------------------------------------------------------------------

const DpVisibilityThreshold: f32 = 0.4;
const PxVisibilityThreshold: f32 = 1.0;

/// How close to its target an animation of a type is close enough to be there (Compose's `Offset.VisibilityThreshold`,
/// …: `Offset::VisibilityThreshold` here, with this trait in scope).
pub trait VisibilityThreshold: Sized {
    const VisibilityThreshold: Self;
}

impl VisibilityThreshold for IntOffset {
    const VisibilityThreshold: IntOffset = IntOffset::new(1, 1);
}
impl VisibilityThreshold for Offset {
    const VisibilityThreshold: Offset = Offset::new(PxVisibilityThreshold, PxVisibilityThreshold);
}
impl VisibilityThreshold for i32 {
    const VisibilityThreshold: i32 = 1;
}
impl VisibilityThreshold for Dp {
    const VisibilityThreshold: Dp = Dp(DpVisibilityThreshold);
}
impl VisibilityThreshold for DpOffset {
    const VisibilityThreshold: DpOffset = DpOffset::new(Dp(DpVisibilityThreshold), Dp(DpVisibilityThreshold));
}
/// (KompozR's, as DpOffset's: Compose 1.12 has none for a DpSize.)
impl VisibilityThreshold for DpSize {
    const VisibilityThreshold: DpSize = DpSize::new(Dp(DpVisibilityThreshold), Dp(DpVisibilityThreshold));
}
impl VisibilityThreshold for Size {
    const VisibilityThreshold: Size = Size::new(PxVisibilityThreshold, PxVisibilityThreshold);
}
impl VisibilityThreshold for IntSize {
    const VisibilityThreshold: IntSize = IntSize::new(1, 1);
}
impl VisibilityThreshold for Rect {
    const VisibilityThreshold: Rect =
        Rect::new(PxVisibilityThreshold, PxVisibilityThreshold, PxVisibilityThreshold, PxVisibilityThreshold);
}

/// The visibility threshold of a converter's values, one float for each of its vector's (Compose's internal
/// VisibilityThresholdMap: Int, IntSize, IntOffset 1; Float 0.01; Rect, Size, Offset 1 px; Dp, DpOffset 0.4 dp), if
/// it is one of these converters.
pub fn visibility_threshold_of<T: 'static, V: AnimationVector>(converter: &TwoWayConverter<T, V>) -> Option<f32> {
    use std::any::Any;
    let c: &dyn Any = converter;
    fn is<T: VectorConverter>(c: &dyn std::any::Any) -> bool {
        c.downcast_ref::<TwoWayConverter<T, T::Vector>>().is_some_and(|c| *c == T::VectorConverter)
    }
    if is::<i32>(c) || is::<IntSize>(c) || is::<IntOffset>(c) {
        Some(1.0)
    } else if is::<f32>(c) {
        Some(0.01)
    } else if is::<Rect>(c) || is::<Size>(c) || is::<Offset>(c) {
        Some(PxVisibilityThreshold)
    } else if is::<Dp>(c) || is::<DpOffset>(c) || is::<DpSize>(c) {
        Some(DpVisibilityThreshold)
    } else {
        None
    }
}
