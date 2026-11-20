//! The motion scheme, as Compose's MotionScheme.kt: how a theme's components move — six springs, spatial (what
//! moves and resizes) and effects (what fades and changes color), each default, fast and slow — in Material's
//! standard scheme or its Expressive one, which bounces.
//!
//! Kotlin's `MotionScheme` is an interface of generic functions (`fun <T> defaultSpatialSpec():
//! FiniteAnimationSpec<T>`): a spec made without knowing the type it animates. Here the scheme is a value
//! ([`MotionScheme`]) of six such specs ([`AnyTypeAnimationSpec`]: a spring, a tween or a snap, which need nothing
//! of their type), made specs of a type where they are asked for (`scheme.default_spatial_spec::<Dp>()`); a scheme
//! of one's own implements [`MotionSchemeSpecs`].

use crate::material_theme::MaterialTheme;
use crate::tokens::{ExpressiveMotionTokens, MotionSchemeKeyTokens, StandardMotionTokens};
use kompoz_animation::{
    AnimationSpec, AnimationVector1D, AnimationVector2D, AnimationVector3D, AnimationVector4D, FiniteAnimationSpec,
    SnapSpec, SpringSpec, TweenSpec, TwoWayConverter, VectorizedAnimationSpec, VectorizedFiniteAnimationSpec,
    VectorizedSnapSpec, VectorizedSpringSpec, VectorizedTweenSpec,
};
use std::any::Any;
use std::fmt;
use std::marker::PhantomData;
use std::sync::{Arc, OnceLock};

/// A finite spec of no type in particular (what Kotlin's generic `fun <T> …Spec(): FiniteAnimationSpec<T>` makes):
/// it is a spec of any type ([`AnyTypeSpec`]). Springs (without a visibility threshold of their own: the type's
/// default, as a generic one has), tweens and snaps are.
pub trait AnyTypeAnimationSpec: Send + Sync + 'static {
    #[doc(hidden)]
    fn vectorize_1d(&self) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector1D>>;
    #[doc(hidden)]
    fn vectorize_2d(&self) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector2D>>;
    #[doc(hidden)]
    fn vectorize_3d(&self) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector3D>>;
    #[doc(hidden)]
    fn vectorize_4d(&self) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector4D>>;
    #[doc(hidden)]
    fn as_any(&self) -> &dyn Any;
    /// Whether `other` is a spec equal to this one.
    #[doc(hidden)]
    fn equals(&self, other: &dyn Any) -> bool;
}

macro_rules! any_type_spec {
    ($ty:ident, |$s:ident| $make:expr) => {
        impl<T: 'static> AnyTypeAnimationSpec for $ty<T>
        where
            $ty<T>: PartialEq + Send + Sync,
        {
            fn vectorize_1d(&self) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector1D>> {
                let $s = self;
                Box::new($make)
            }
            fn vectorize_2d(&self) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector2D>> {
                let $s = self;
                Box::new($make)
            }
            fn vectorize_3d(&self) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector3D>> {
                let $s = self;
                Box::new($make)
            }
            fn vectorize_4d(&self) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector4D>> {
                let $s = self;
                Box::new($make)
            }
            fn as_any(&self) -> &dyn Any {
                self
            }
            fn equals(&self, other: &dyn Any) -> bool {
                other.downcast_ref::<Self>().is_some_and(|o| o == self)
            }
        }
    };
}

any_type_spec!(SpringSpec, |s| VectorizedSpringSpec::new(s.damping_ratio, s.stiffness, None));
any_type_spec!(TweenSpec, |s| VectorizedTweenSpec::new(s.duration_millis, s.delay, s.easing.clone()));
any_type_spec!(SnapSpec, |s| VectorizedSnapSpec::new(s.delay));

impl PartialEq for dyn AnyTypeAnimationSpec {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other.as_any())
    }
}

/// An [`AnyTypeAnimationSpec`] as a spec of `T`s (what Kotlin's cast of a scheme's spec is).
pub struct AnyTypeSpec<T> {
    spec: Arc<dyn AnyTypeAnimationSpec>,
    _t: PhantomData<fn() -> T>,
}

impl<T> AnyTypeSpec<T> {
    pub fn new(spec: Arc<dyn AnyTypeAnimationSpec>) -> AnyTypeSpec<T> {
        AnyTypeSpec { spec, _t: PhantomData }
    }
}

impl<T> Clone for AnyTypeSpec<T> {
    fn clone(&self) -> Self {
        AnyTypeSpec::new(self.spec.clone())
    }
}

impl<T> PartialEq for AnyTypeSpec<T> {
    fn eq(&self, other: &Self) -> bool {
        *self.spec == *other.spec
    }
}

impl<T: 'static> AnimationSpec<T> for AnyTypeSpec<T> {
    fn vectorize_1d(
        &self,
        _: &TwoWayConverter<T, AnimationVector1D>,
    ) -> Box<dyn VectorizedAnimationSpec<AnimationVector1D>> {
        self.spec.vectorize_1d()
    }
    fn vectorize_2d(
        &self,
        _: &TwoWayConverter<T, AnimationVector2D>,
    ) -> Box<dyn VectorizedAnimationSpec<AnimationVector2D>> {
        self.spec.vectorize_2d()
    }
    fn vectorize_3d(
        &self,
        _: &TwoWayConverter<T, AnimationVector3D>,
    ) -> Box<dyn VectorizedAnimationSpec<AnimationVector3D>> {
        self.spec.vectorize_3d()
    }
    fn vectorize_4d(
        &self,
        _: &TwoWayConverter<T, AnimationVector4D>,
    ) -> Box<dyn VectorizedAnimationSpec<AnimationVector4D>> {
        self.spec.vectorize_4d()
    }
    fn as_any(&self) -> &dyn Any {
        // (equal to another of the same spec, whatever its type)
        self.spec.as_any()
    }
    fn equals(&self, other: &dyn Any) -> bool {
        self.spec.equals(other)
    }
}

impl<T: 'static> FiniteAnimationSpec<T> for AnyTypeSpec<T> {
    fn vectorize_finite_1d(
        &self,
        _: &TwoWayConverter<T, AnimationVector1D>,
    ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector1D>> {
        self.spec.vectorize_1d()
    }
    fn vectorize_finite_2d(
        &self,
        _: &TwoWayConverter<T, AnimationVector2D>,
    ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector2D>> {
        self.spec.vectorize_2d()
    }
    fn vectorize_finite_3d(
        &self,
        _: &TwoWayConverter<T, AnimationVector3D>,
    ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector3D>> {
        self.spec.vectorize_3d()
    }
    fn vectorize_finite_4d(
        &self,
        _: &TwoWayConverter<T, AnimationVector4D>,
    ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector4D>> {
        self.spec.vectorize_4d()
    }
}

/// What a motion scheme is made of (Compose's MotionScheme interface): its six specs, of any type.
pub trait MotionSchemeSpecs: Send + Sync + 'static {
    /// What moves and resizes, by default.
    fn default_spatial(&self) -> Arc<dyn AnyTypeAnimationSpec>;
    /// What moves and resizes quickly (small components: a switch, a button).
    fn fast_spatial(&self) -> Arc<dyn AnyTypeAnimationSpec>;
    /// What moves and resizes slowly (what covers the screen).
    fn slow_spatial(&self) -> Arc<dyn AnyTypeAnimationSpec>;
    /// What fades or changes color, by default.
    fn default_effects(&self) -> Arc<dyn AnyTypeAnimationSpec>;
    fn fast_effects(&self) -> Arc<dyn AnyTypeAnimationSpec>;
    fn slow_effects(&self) -> Arc<dyn AnyTypeAnimationSpec>;
}

/// How a theme's components move (Compose's MotionScheme): Material's [`standard`](MotionScheme::standard) springs
/// (critically damped) or [`expressive`](MotionScheme::expressive) ones (spatial ones bounce), or a scheme of one's
/// own ([`MotionScheme::new`]). Each spec is asked for as a spec of the type it animates:
/// `MaterialTheme::motion_scheme().fast_spatial_spec::<Dp>()`. Equal only to itself and its copies (Kotlin's objects).
#[derive(Clone)]
pub struct MotionScheme(Arc<dyn MotionSchemeSpecs>);

impl MotionScheme {
    /// A scheme of one's own (Kotlin's `object : MotionScheme { … }`).
    pub fn new(specs: impl MotionSchemeSpecs) -> MotionScheme {
        MotionScheme(Arc::new(specs))
    }

    /// Material's standard scheme (Compose's `MotionScheme.standard()`): critically damped springs, the same one
    /// each time.
    pub fn standard() -> MotionScheme {
        static STANDARD: OnceLock<MotionScheme> = OnceLock::new();
        STANDARD
            .get_or_init(|| {
                MotionScheme::new(Springs::of(
                    (
                        StandardMotionTokens::SpringDefaultSpatialDamping,
                        StandardMotionTokens::SpringDefaultSpatialStiffness,
                    ),
                    (StandardMotionTokens::SpringFastSpatialDamping, StandardMotionTokens::SpringFastSpatialStiffness),
                    (StandardMotionTokens::SpringSlowSpatialDamping, StandardMotionTokens::SpringSlowSpatialStiffness),
                    (
                        StandardMotionTokens::SpringDefaultEffectsDamping,
                        StandardMotionTokens::SpringDefaultEffectsStiffness,
                    ),
                    (StandardMotionTokens::SpringFastEffectsDamping, StandardMotionTokens::SpringFastEffectsStiffness),
                    (StandardMotionTokens::SpringSlowEffectsDamping, StandardMotionTokens::SpringSlowEffectsStiffness),
                ))
            })
            .clone()
    }

    /// Material Expressive's scheme (Compose's `MotionScheme.expressive()`): its spatial springs bounce, the same
    /// one each time.
    pub fn expressive() -> MotionScheme {
        static EXPRESSIVE: OnceLock<MotionScheme> = OnceLock::new();
        EXPRESSIVE
            .get_or_init(|| {
                MotionScheme::new(Springs::of(
                    (
                        ExpressiveMotionTokens::SpringDefaultSpatialDamping,
                        ExpressiveMotionTokens::SpringDefaultSpatialStiffness,
                    ),
                    (
                        ExpressiveMotionTokens::SpringFastSpatialDamping,
                        ExpressiveMotionTokens::SpringFastSpatialStiffness,
                    ),
                    (
                        ExpressiveMotionTokens::SpringSlowSpatialDamping,
                        ExpressiveMotionTokens::SpringSlowSpatialStiffness,
                    ),
                    (
                        ExpressiveMotionTokens::SpringDefaultEffectsDamping,
                        ExpressiveMotionTokens::SpringDefaultEffectsStiffness,
                    ),
                    (
                        ExpressiveMotionTokens::SpringFastEffectsDamping,
                        ExpressiveMotionTokens::SpringFastEffectsStiffness,
                    ),
                    (
                        ExpressiveMotionTokens::SpringSlowEffectsDamping,
                        ExpressiveMotionTokens::SpringSlowEffectsStiffness,
                    ),
                ))
            })
            .clone()
    }

    /// What it is made of.
    pub fn specs(&self) -> &dyn MotionSchemeSpecs {
        &*self.0
    }

    /// Compose's `defaultSpatialSpec<T>()`.
    pub fn default_spatial_spec<T: 'static>(&self) -> Arc<dyn FiniteAnimationSpec<T>> {
        Arc::new(AnyTypeSpec::new(self.0.default_spatial()))
    }
    /// Compose's `fastSpatialSpec<T>()`.
    pub fn fast_spatial_spec<T: 'static>(&self) -> Arc<dyn FiniteAnimationSpec<T>> {
        Arc::new(AnyTypeSpec::new(self.0.fast_spatial()))
    }
    /// Compose's `slowSpatialSpec<T>()`.
    pub fn slow_spatial_spec<T: 'static>(&self) -> Arc<dyn FiniteAnimationSpec<T>> {
        Arc::new(AnyTypeSpec::new(self.0.slow_spatial()))
    }
    /// Compose's `defaultEffectsSpec<T>()`.
    pub fn default_effects_spec<T: 'static>(&self) -> Arc<dyn FiniteAnimationSpec<T>> {
        Arc::new(AnyTypeSpec::new(self.0.default_effects()))
    }
    /// Compose's `fastEffectsSpec<T>()`.
    pub fn fast_effects_spec<T: 'static>(&self) -> Arc<dyn FiniteAnimationSpec<T>> {
        Arc::new(AnyTypeSpec::new(self.0.fast_effects()))
    }
    /// Compose's `slowEffectsSpec<T>()`.
    pub fn slow_effects_spec<T: 'static>(&self) -> Arc<dyn FiniteAnimationSpec<T>> {
        Arc::new(AnyTypeSpec::new(self.0.slow_effects()))
    }

    /// A token's spec in this scheme (Compose's internal `MotionScheme.fromToken`).
    #[allow(dead_code, clippy::wrong_self_convention)] // (Compose's internal `fromToken`; the components', as they come)
    pub(crate) fn from_token<T: 'static>(&self, value: MotionSchemeKeyTokens) -> Arc<dyn FiniteAnimationSpec<T>> {
        match value {
            MotionSchemeKeyTokens::DefaultSpatial => self.default_spatial_spec(),
            MotionSchemeKeyTokens::FastSpatial => self.fast_spatial_spec(),
            MotionSchemeKeyTokens::SlowSpatial => self.slow_spatial_spec(),
            MotionSchemeKeyTokens::DefaultEffects => self.default_effects_spec(),
            MotionSchemeKeyTokens::FastEffects => self.fast_effects_spec(),
            MotionSchemeKeyTokens::SlowEffects => self.slow_effects_spec(),
        }
    }
}

impl MotionSchemeKeyTokens {
    /// The token's spec in the theme's motion scheme (Compose's internal `MotionSchemeKeyTokens.value()`).
    #[allow(dead_code)] // (the components', used as they come)
    pub(crate) fn value<T: 'static>(self) -> Arc<dyn FiniteAnimationSpec<T>> {
        MaterialTheme::motion_scheme().from_token(self)
    }
}

impl PartialEq for MotionScheme {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for MotionScheme {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if *self == MotionScheme::standard() {
            f.write_str("MotionScheme.standard()")
        } else if *self == MotionScheme::expressive() {
            f.write_str("MotionScheme.expressive()")
        } else {
            write!(f, "MotionScheme({:p})", Arc::as_ptr(&self.0))
        }
    }
}

/// Six springs of no type (Compose's StandardMotionSchemeImpl and ExpressiveMotionSchemeImpl: `spring<Any>`).
struct Springs([Arc<dyn AnyTypeAnimationSpec>; 6]);

impl Springs {
    fn of(
        default_spatial: (f32, f32),
        fast_spatial: (f32, f32),
        slow_spatial: (f32, f32),
        default_effects: (f32, f32),
        fast_effects: (f32, f32),
        slow_effects: (f32, f32),
    ) -> Springs {
        let spring = |(damping_ratio, stiffness): (f32, f32)| -> Arc<dyn AnyTypeAnimationSpec> {
            Arc::new(SpringSpec::<()>::new(damping_ratio, stiffness, None))
        };
        Springs([
            spring(default_spatial),
            spring(fast_spatial),
            spring(slow_spatial),
            spring(default_effects),
            spring(fast_effects),
            spring(slow_effects),
        ])
    }
}

impl MotionSchemeSpecs for Springs {
    fn default_spatial(&self) -> Arc<dyn AnyTypeAnimationSpec> {
        self.0[0].clone()
    }
    fn fast_spatial(&self) -> Arc<dyn AnyTypeAnimationSpec> {
        self.0[1].clone()
    }
    fn slow_spatial(&self) -> Arc<dyn AnyTypeAnimationSpec> {
        self.0[2].clone()
    }
    fn default_effects(&self) -> Arc<dyn AnyTypeAnimationSpec> {
        self.0[3].clone()
    }
    fn fast_effects(&self) -> Arc<dyn AnyTypeAnimationSpec> {
        self.0[4].clone()
    }
    fn slow_effects(&self) -> Arc<dyn AnyTypeAnimationSpec> {
        self.0[5].clone()
    }
}
