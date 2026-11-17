//! The vectors animations run on (Compose's AnimationVectors.kt): one to four floats.

use crate::converter::TwoWayConverter;
use crate::decay::{DecayAnimationSpec, VectorizedDecayAnimationSpec};
use crate::spec::{AnimationSpec, DurationBasedAnimationSpec, FiniteAnimationSpec};
use crate::vectorized::{VectorizedAnimationSpec, VectorizedDurationBasedAnimationSpec, VectorizedFiniteAnimationSpec};
use kompoz_runtime::KotlinProperties;
use kompoz_runtime::kt::KtToString;
use std::fmt;

mod sealed {
    pub trait Sealed {}
}

/// What animations animate: a value converted to one to four floats (Compose's AnimationVector, a sealed class:
/// [`AnimationVector1D`] to [`AnimationVector4D`] are all there are). Vectors are values here (`Copy`); Compose's
/// are objects its specs reuse.
pub trait AnimationVector:
    Copy + PartialEq + Default + fmt::Debug + fmt::Display + Send + Sync + 'static + sealed::Sealed
{
    /// How many floats (Compose's internal `size`).
    const SIZE: usize;

    /// The float at `index` (0 past its size).
    fn get(&self, index: usize) -> f32;

    /// Sets the float at `index` (nothing past its size).
    fn set(&mut self, index: usize, value: f32);

    /// How many floats.
    fn size(&self) -> usize {
        Self::SIZE
    }

    /// A vector of its kind, all zeros (Compose's newInstance).
    fn new_instance(&self) -> Self {
        Self::default()
    }

    /// Sets every float to 0.
    fn reset(&mut self) {
        *self = Self::default();
    }

    /// Copies `source`'s floats (Compose's copyFrom).
    fn copy_from(&mut self, source: &Self) {
        *self = *source;
    }

    #[doc(hidden)]
    fn vectorize<T>(
        spec: &(impl AnimationSpec<T> + ?Sized),
        converter: &TwoWayConverter<T, Self>,
    ) -> Box<dyn VectorizedAnimationSpec<Self>>;
    #[doc(hidden)]
    fn vectorize_finite<T>(
        spec: &(impl FiniteAnimationSpec<T> + ?Sized),
        converter: &TwoWayConverter<T, Self>,
    ) -> Box<dyn VectorizedFiniteAnimationSpec<Self>>;
    #[doc(hidden)]
    fn vectorize_duration_based<T>(
        spec: &(impl DurationBasedAnimationSpec<T> + ?Sized),
        converter: &TwoWayConverter<T, Self>,
    ) -> Box<dyn VectorizedDurationBasedAnimationSpec<Self>>;
    #[doc(hidden)]
    fn vectorize_decay<T>(
        spec: &(impl DecayAnimationSpec<T> + ?Sized),
        converter: &TwoWayConverter<T, Self>,
    ) -> Box<dyn VectorizedDecayAnimationSpec<Self>>;
}

/// One float (Compose's AnimationVector1D).
#[derive(Clone, Copy, PartialEq, Default, Debug, KotlinProperties)]
pub struct AnimationVector1D {
    pub value: f32,
}

/// Two floats (Compose's AnimationVector2D).
#[derive(Clone, Copy, PartialEq, Default, Debug, KotlinProperties)]
pub struct AnimationVector2D {
    pub v1: f32,
    pub v2: f32,
}

/// Three floats (Compose's AnimationVector3D).
#[derive(Clone, Copy, PartialEq, Default, Debug, KotlinProperties)]
pub struct AnimationVector3D {
    pub v1: f32,
    pub v2: f32,
    pub v3: f32,
}

/// Four floats (Compose's AnimationVector4D).
#[derive(Clone, Copy, PartialEq, Default, Debug, KotlinProperties)]
pub struct AnimationVector4D {
    pub v1: f32,
    pub v2: f32,
    pub v3: f32,
    pub v4: f32,
}

/// Compose's `AnimationVector1D(initVal)`.
pub const fn AnimationVector1D(init_val: f32) -> AnimationVector1D {
    AnimationVector1D { value: init_val }
}

/// Compose's `AnimationVector2D(v1, v2)`.
pub const fn AnimationVector2D(v1: f32, v2: f32) -> AnimationVector2D {
    AnimationVector2D { v1, v2 }
}

/// Compose's `AnimationVector3D(v1, v2, v3)`.
pub const fn AnimationVector3D(v1: f32, v2: f32, v3: f32) -> AnimationVector3D {
    AnimationVector3D { v1, v2, v3 }
}

/// Compose's `AnimationVector4D(v1, v2, v3, v4)`.
pub const fn AnimationVector4D(v1: f32, v2: f32, v3: f32, v4: f32) -> AnimationVector4D {
    AnimationVector4D { v1, v2, v3, v4 }
}

/// Compose's `AnimationVector(…)`, its overloads by their arguments: one float, or a tuple of two to four.
pub fn AnimationVector<A: AnimationVectorArgs>(args: A) -> A::Vector {
    args.vector()
}

/// What [`AnimationVector()`](fn@AnimationVector) is given: Kotlin's arguments, one as itself, several as a tuple.
pub trait AnimationVectorArgs {
    type Vector: AnimationVector;
    fn vector(self) -> Self::Vector;
}
/// Kotlin: `AnimationVector(v1: Float)`
impl AnimationVectorArgs for f32 {
    type Vector = AnimationVector1D;
    fn vector(self) -> AnimationVector1D {
        AnimationVector1D(self)
    }
}
/// Kotlin: `AnimationVector(v1: Float, v2: Float)`
impl AnimationVectorArgs for (f32, f32) {
    type Vector = AnimationVector2D;
    fn vector(self) -> AnimationVector2D {
        AnimationVector2D(self.0, self.1)
    }
}
/// Kotlin: `AnimationVector(v1: Float, v2: Float, v3: Float)`
impl AnimationVectorArgs for (f32, f32, f32) {
    type Vector = AnimationVector3D;
    fn vector(self) -> AnimationVector3D {
        AnimationVector3D(self.0, self.1, self.2)
    }
}
/// Kotlin: `AnimationVector(v1: Float, v2: Float, v3: Float, v4: Float)`
impl AnimationVectorArgs for (f32, f32, f32, f32) {
    type Vector = AnimationVector4D;
    fn vector(self) -> AnimationVector4D {
        AnimationVector4D(self.0, self.1, self.2, self.3)
    }
}

macro_rules! vector_impl {
    ($ty:ident, $size:expr, $suffix:ident, [$($i:expr => $f:ident),*]) => {
        impl sealed::Sealed for $ty {}
        impl AnimationVector for $ty {
            const SIZE: usize = $size;
            fn get(&self, index: usize) -> f32 {
                match index {
                    $($i => self.$f,)*
                    _ => 0.0,
                }
            }
            fn set(&mut self, index: usize, value: f32) {
                match index {
                    $($i => self.$f = value,)*
                    _ => {}
                }
            }
            fn vectorize<T>(
                spec: &(impl AnimationSpec<T> + ?Sized),
                converter: &TwoWayConverter<T, Self>,
            ) -> Box<dyn VectorizedAnimationSpec<Self>> {
                paste_vectorize!(spec, vectorize, $suffix, converter)
            }
            fn vectorize_finite<T>(
                spec: &(impl FiniteAnimationSpec<T> + ?Sized),
                converter: &TwoWayConverter<T, Self>,
            ) -> Box<dyn VectorizedFiniteAnimationSpec<Self>> {
                paste_vectorize!(spec, vectorize_finite, $suffix, converter)
            }
            fn vectorize_duration_based<T>(
                spec: &(impl DurationBasedAnimationSpec<T> + ?Sized),
                converter: &TwoWayConverter<T, Self>,
            ) -> Box<dyn VectorizedDurationBasedAnimationSpec<Self>> {
                paste_vectorize!(spec, vectorize_duration_based, $suffix, converter)
            }
            fn vectorize_decay<T>(
                spec: &(impl DecayAnimationSpec<T> + ?Sized),
                converter: &TwoWayConverter<T, Self>,
            ) -> Box<dyn VectorizedDecayAnimationSpec<Self>> {
                paste_vectorize!(spec, vectorize_decay, $suffix, converter)
            }
        }
    };
}

/// `spec.<method>_<suffix>(converter)`: the spec's method for this vector's size.
macro_rules! paste_vectorize {
    ($spec:ident, vectorize, d1, $c:ident) => {
        $spec.vectorize_1d($c)
    };
    ($spec:ident, vectorize, d2, $c:ident) => {
        $spec.vectorize_2d($c)
    };
    ($spec:ident, vectorize, d3, $c:ident) => {
        $spec.vectorize_3d($c)
    };
    ($spec:ident, vectorize, d4, $c:ident) => {
        $spec.vectorize_4d($c)
    };
    ($spec:ident, vectorize_finite, d1, $c:ident) => {
        $spec.vectorize_finite_1d($c)
    };
    ($spec:ident, vectorize_finite, d2, $c:ident) => {
        $spec.vectorize_finite_2d($c)
    };
    ($spec:ident, vectorize_finite, d3, $c:ident) => {
        $spec.vectorize_finite_3d($c)
    };
    ($spec:ident, vectorize_finite, d4, $c:ident) => {
        $spec.vectorize_finite_4d($c)
    };
    ($spec:ident, vectorize_duration_based, d1, $c:ident) => {
        $spec.vectorize_duration_based_1d($c)
    };
    ($spec:ident, vectorize_duration_based, d2, $c:ident) => {
        $spec.vectorize_duration_based_2d($c)
    };
    ($spec:ident, vectorize_duration_based, d3, $c:ident) => {
        $spec.vectorize_duration_based_3d($c)
    };
    ($spec:ident, vectorize_duration_based, d4, $c:ident) => {
        $spec.vectorize_duration_based_4d($c)
    };
    ($spec:ident, vectorize_decay, d1, $c:ident) => {
        $spec.vectorize_decay_1d($c)
    };
    ($spec:ident, vectorize_decay, d2, $c:ident) => {
        $spec.vectorize_decay_2d($c)
    };
    ($spec:ident, vectorize_decay, d3, $c:ident) => {
        $spec.vectorize_decay_3d($c)
    };
    ($spec:ident, vectorize_decay, d4, $c:ident) => {
        $spec.vectorize_decay_4d($c)
    };
}

vector_impl!(AnimationVector1D, 1, d1, [0 => value]);
vector_impl!(AnimationVector2D, 2, d2, [0 => v1, 1 => v2]);
vector_impl!(AnimationVector3D, 3, d3, [0 => v1, 1 => v2, 2 => v3]);
vector_impl!(AnimationVector4D, 4, d4, [0 => v1, 1 => v2, 2 => v3, 3 => v4]);

/// As Kotlin writes it (`AnimationVector1D: value = 1.0`).
impl fmt::Display for AnimationVector1D {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AnimationVector1D: value = {}", self.value.kt_to_string())
    }
}
impl fmt::Display for AnimationVector2D {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AnimationVector2D: v1 = {}, v2 = {}", self.v1.kt_to_string(), self.v2.kt_to_string())
    }
}
impl fmt::Display for AnimationVector3D {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "AnimationVector3D: v1 = {}, v2 = {}, v3 = {}",
            self.v1.kt_to_string(),
            self.v2.kt_to_string(),
            self.v3.kt_to_string()
        )
    }
}
impl fmt::Display for AnimationVector4D {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "AnimationVector4D: v1 = {}, v2 = {}, v3 = {}, v4 = {}",
            self.v1.kt_to_string(),
            self.v2.kt_to_string(),
            self.v3.kt_to_string(),
            self.v4.kt_to_string()
        )
    }
}
