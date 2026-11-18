//! What is zoomed, panned and turned, as Compose's TransformableState.kt: a [`TransformableState`] takes
//! transformations inside a mutation ([`TransformableState::transform_block`], one at a time, a newer one of at
//! least its priority cancelling an older), in its [`TransformScope`]; [`TransformableState()`] makes one of a
//! closure; [`TransformableStateExtensions`] animate or apply transformations from code.

// (Compose's parameter lists: the methods of a `#[named]` trait carry no attributes of their own)
#![allow(clippy::too_many_arguments)]

use super::scrollable_state::{LocalFuture, ScrollableStateEq};
use kompoz_animation::{
    AnimationConstants, AnimationScope, AnimationSpec, AnimationState, AnimationVector, AnimationVector1D,
    AnimationVector2D, AnimationVector3D, AnimationVector4D, MutatePriority, MutatorMutex, Spring, SpringSpec,
    TwoWayConverter, VectorConverter, VectorizedAnimationSpec, VectorizedFiniteAnimationSpec, float_animation_state,
};
use kompoz_graphics::Offset;
use kompoz_runtime::{MutableState, kompozable, mutable_state_of, named, named_args, remember, remember_updated_state};
use std::any::Any;
use std::cell::Cell;
use std::fmt;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

/// What a transformation mutation transforms with (Compose's TransformScope).
pub trait TransformScope {
    /// Zooms by `zoom_change` (a factor), pans by `pan_change` px and turns by `rotation_change` degrees clockwise
    /// (Compose's `transformBy`).
    /// Kotlin: `transformBy(zoomChange: Float = 1f, panChange: Offset = Offset.Zero, rotationChange: Float = 0f)`
    fn transform_by(&self, zoom_change: f32, pan_change: Offset, rotation_change: f32);

    /// The same about `centroid` (px, `Offset::Unspecified` for none): where zooming and turning happen around
    /// (Compose's `transformByWithCentroid`; by default it leaves the centroid out).
    /// Kotlin: `transformByWithCentroid(centroid: Offset = Offset.Unspecified, zoomChange: Float = 1f, panChange: Offset = Offset.Zero, rotationChange: Float = 0f)`
    fn transform_by_with_centroid(&self, centroid: Offset, zoom_change: f32, pan_change: Offset, rotation_change: f32) {
        let _ = centroid;
        self.transform_by(zoom_change, pan_change, rotation_change)
    }
}

/// A block transforming in a [`TransformScope`] (Compose's `suspend TransformScope.() -> Unit`).
pub type TransformBlock = Box<dyn FnOnce(Rc<dyn TransformScope>) -> LocalFuture<'static, ()>>;

/// A state that is zoomed, panned and turned (Compose's TransformableState): `transform_block` runs a block
/// transforming in its [`TransformScope`], one at a time; whether one runs now. A cheap handle, equal to its clones
/// (what `transformable` compares it by).
pub trait TransformableState: Any + ScrollableStateEq {
    /// Runs `block` as the mutation of `transform_priority` (Compose's `transform`). Through
    /// [`TransformableStateExtensions::transform`] with a closure.
    fn transform_block(&self, transform_priority: MutatePriority, block: TransformBlock) -> LocalFuture<'_, ()>;

    /// Whether a gesture or a program transforms it now (Compose's `isTransformInProgress`).
    fn is_transform_in_progress(&self) -> bool;
}

impl PartialEq for dyn TransformableState {
    fn eq(&self, other: &dyn TransformableState) -> bool {
        self.scrollable_state_eq(ScrollableStateEq::as_any(other))
    }
}

impl fmt::Debug for dyn TransformableState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TransformableState")
    }
}

/// A state shared as any kind of transformable state: its state's calls.
impl TransformableState for Rc<dyn TransformableState> {
    fn transform_block(&self, transform_priority: MutatePriority, block: TransformBlock) -> LocalFuture<'_, ()> {
        (**self).transform_block(transform_priority, block)
    }
    fn is_transform_in_progress(&self) -> bool {
        (**self).is_transform_in_progress()
    }
}

/// A spring of low stiffness (Compose's `SpringSpec(stiffness = Spring.StiffnessLow)`, the animations' default).
fn low_spring<T: Clone + PartialEq + Send + Sync + 'static>() -> SpringSpec<T> {
    SpringSpec::new(Spring::DampingRatioNoBouncy, Spring::StiffnessLow, None)
}

/// The calls of any [`TransformableState`] (Compose's `transform` with a lambda and TransformableState.kt's
/// extensions).
#[named(trait TransformableStateExtensions)]
impl<S: TransformableState> S {
    /// Runs `block` as the mutation of `transform_priority` (Compose's `transform { … }`).
    fn transform<F, Fut>(
        &self,
        #[default(MutatePriority::Default)] transform_priority: MutatePriority,
        block: F,
    ) -> impl Future<Output = ()>
    where
        F: FnOnce(Rc<dyn TransformScope>) -> Fut + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        self.transform_block(transform_priority, Box::new(move |scope| Box::pin(block(scope))))
    }

    /// Zooms by `zoom_factor` (over 0) along `animation_spec`, about `centroid` (Compose's `animateZoomBy`).
    fn animate_zoom_by(
        &self,
        zoom_factor: f32,
        #[default(Arc::new(low_spring::<f32>()))] animation_spec: Arc<dyn AnimationSpec<f32>>,
        #[default(Offset::Unspecified)] centroid: Offset,
    ) -> impl Future<Output = ()> {
        assert!(zoom_factor > 0.0, "zoom value should be greater than 0");
        self.transform_block(
            MutatePriority::Default,
            Box::new(move |scope| {
                Box::pin(async move {
                    let previous = Rc::new(Cell::new(1.0f32));
                    let p = previous.clone();
                    let block: Box<dyn for<'a> FnMut(&'a AnimationScope<f32, AnimationVector1D>)> =
                        Box::new(move |a| {
                            let scale_factor = if p.get() == 0.0 { 1.0 } else { a.value() / p.get() };
                            scope.transform_by_with_centroid(centroid, scale_factor, Offset::Zero, 0.0);
                            p.set(a.value());
                        });
                    let state = named_args!(float_animation_state(previous.get()));
                    named_args!(state.animate_to(zoom_factor, animation_spec = animation_spec, block = block)).await;
                })
            }),
        )
    }

    /// Turns by `degrees` clockwise along `animation_spec`, about `centroid` (Compose's `animateRotateBy`).
    fn animate_rotate_by(
        &self,
        degrees: f32,
        #[default(Arc::new(low_spring::<f32>()))] animation_spec: Arc<dyn AnimationSpec<f32>>,
        #[default(Offset::Unspecified)] centroid: Offset,
    ) -> impl Future<Output = ()> {
        self.transform_block(
            MutatePriority::Default,
            Box::new(move |scope| {
                Box::pin(async move {
                    let previous = Rc::new(Cell::new(0.0f32));
                    let p = previous.clone();
                    let block: Box<dyn for<'a> FnMut(&'a AnimationScope<f32, AnimationVector1D>)> =
                        Box::new(move |a| {
                            let delta = a.value() - p.get();
                            scope.transform_by_with_centroid(centroid, 1.0, Offset::Zero, delta);
                            p.set(a.value());
                        });
                    let state = named_args!(float_animation_state(previous.get()));
                    named_args!(state.animate_to(degrees, animation_spec = animation_spec, block = block)).await;
                })
            }),
        )
    }

    /// Pans by `offset` px along `animation_spec`, about `centroid` (Compose's `animatePanBy`).
    fn animate_pan_by(
        &self,
        offset: Offset,
        #[default(Arc::new(low_spring::<Offset>()))] animation_spec: Arc<dyn AnimationSpec<Offset>>,
        #[default(Offset::Unspecified)] centroid: Offset,
    ) -> impl Future<Output = ()> {
        self.transform_block(
            MutatePriority::Default,
            Box::new(move |scope| {
                Box::pin(async move {
                    let previous = Rc::new(Cell::new(Offset::Zero));
                    let p = previous.clone();
                    let block: Box<dyn for<'a> FnMut(&'a AnimationScope<Offset, AnimationVector2D>)> =
                        Box::new(move |a| {
                            let delta = a.value() - p.get();
                            scope.transform_by_with_centroid(centroid, 1.0, delta, 0.0);
                            p.set(a.value());
                        });
                    let state = AnimationState::of(Offset::VectorConverter, previous.get());
                    named_args!(state.animate_to(offset, animation_spec = animation_spec, block = block)).await;
                })
            }),
        )
    }

    /// Zooms by `zoom_factor` (over 0), pans by `pan_offset` px and turns by `rotation_degrees`, each along its
    /// spec, together, about `centroid` (Compose's `animateBy`).
    fn animate_by(
        &self,
        zoom_factor: f32,
        pan_offset: Offset,
        rotation_degrees: f32,
        #[default(Arc::new(low_spring::<f32>()))] zoom_animation_spec: Arc<dyn AnimationSpec<f32>>,
        #[default(Arc::new(low_spring::<Offset>()))] pan_animation_spec: Arc<dyn AnimationSpec<Offset>>,
        #[default(Arc::new(low_spring::<f32>()))] rotation_animation_spec: Arc<dyn AnimationSpec<f32>>,
        #[default(Offset::Unspecified)] centroid: Offset,
    ) -> impl Future<Output = ()> {
        assert!(zoom_factor > 0.0, "zoom value should be greater than 0");
        let previous_state = AnimationData { zoom: 1.0, offset: Offset::Zero, degrees: 0.0 };
        let target_state = AnimationData { zoom: zoom_factor, offset: pan_offset, degrees: rotation_degrees };
        let animation_spec: Arc<dyn AnimationSpec<AnimationData>> = Arc::new(DelegatingAnimationSpec {
            zoom_animation_spec,
            offset_animation_spec: pan_animation_spec,
            rotation_animation_spec,
        });
        self.transform_block(
            MutatePriority::Default,
            Box::new(move |scope| {
                Box::pin(async move {
                    let previous = Rc::new(Cell::new(previous_state));
                    let p = previous.clone();
                    let block: Box<dyn for<'a> FnMut(&'a AnimationScope<AnimationData, AnimationVector4D>)> =
                        Box::new(move |a| {
                            let value = a.value();
                            let prev = p.get();
                            scope.transform_by_with_centroid(
                                centroid,
                                if prev.zoom == 0.0 { 1.0 } else { value.zoom / prev.zoom },
                                value.offset - prev.offset,
                                value.degrees - prev.degrees,
                            );
                            p.set(value);
                        });
                    let state = AnimationState::new(
                        animation_data_converter(),
                        previous.get(),
                        Some(animation_data_converter().convert_to_vector(ZeroAnimationVelocity)),
                        AnimationConstants::UnspecifiedTime,
                        AnimationConstants::UnspecifiedTime,
                        false,
                    );
                    named_args!(state.animate_to(target_state, animation_spec = animation_spec, block = block)).await;
                })
            }),
        )
    }

    /// Zooms by `zoom_factor` at once, about `centroid` (Compose's `zoomBy`).
    fn zoom_by(&self, zoom_factor: f32, #[default(Offset::Unspecified)] centroid: Offset) -> impl Future<Output = ()> {
        self.transform_block(
            MutatePriority::Default,
            Box::new(move |scope| {
                Box::pin(async move {
                    scope.transform_by_with_centroid(centroid, zoom_factor, Offset::Zero, 0.0);
                })
            }),
        )
    }

    /// Turns by `degrees` at once, about `centroid` (Compose's `rotateBy`).
    fn rotate_by(&self, degrees: f32, #[default(Offset::Unspecified)] centroid: Offset) -> impl Future<Output = ()> {
        self.transform_block(
            MutatePriority::Default,
            Box::new(move |scope| {
                Box::pin(async move {
                    scope.transform_by_with_centroid(centroid, 1.0, Offset::Zero, degrees);
                })
            }),
        )
    }

    /// Pans by `offset` px at once, about `centroid` (Compose's `panBy`).
    fn pan_by(&self, offset: Offset, #[default(Offset::Unspecified)] centroid: Offset) -> impl Future<Output = ()> {
        self.transform_block(
            MutatePriority::Default,
            Box::new(move |scope| {
                Box::pin(async move {
                    scope.transform_by_with_centroid(centroid, 1.0, offset, 0.0);
                })
            }),
        )
    }

    /// Stops what transforms it now unless its priority is higher than `termination_priority`, by taking the
    /// mutation with nothing to do (Compose's `stopTransformation`).
    fn stop_transformation(
        &self,
        #[default(MutatePriority::Default)] termination_priority: MutatePriority,
    ) -> impl Future<Output = ()> {
        self.transform_block(termination_priority, Box::new(|_| Box::pin(async {})))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// animateBy's value and spec

/// What `animate_by` animates (Compose's AnimationData): the zoom, the pan, the turn.
#[derive(Clone, Copy, PartialEq, Debug)]
struct AnimationData {
    zoom: f32,
    offset: Offset,
    degrees: f32,
}

const ZeroAnimationVelocity: AnimationData = AnimationData { zoom: 0.0, offset: Offset::Zero, degrees: 0.0 };

/// Compose's AnimationDataConverter.
fn animation_data_converter() -> TwoWayConverter<AnimationData, AnimationVector4D> {
    TwoWayConverter::new(
        |it| AnimationVector4D(it.zoom, it.offset.x, it.offset.y, it.degrees),
        |it| AnimationData { zoom: it.v1, offset: Offset::new(it.v2, it.v3), degrees: it.v4 },
    )
}

/// The zoom's, the pan's and the turn's specs run side by side (Compose's DelegatingAnimationSpec): as long as the
/// longest of them.
struct DelegatingAnimationSpec {
    zoom_animation_spec: Arc<dyn AnimationSpec<f32>>,
    offset_animation_spec: Arc<dyn AnimationSpec<Offset>>,
    rotation_animation_spec: Arc<dyn AnimationSpec<f32>>,
}

impl DelegatingAnimationSpec {
    fn vectorize_into<V: AnimationVector>(
        &self,
        converter: &TwoWayConverter<AnimationData, V>,
    ) -> DelegatingVectorized<V> {
        DelegatingVectorized {
            converter: converter.clone(),
            zoom: self.zoom_animation_spec.vectorize(&f32::VectorConverter),
            offset: self.offset_animation_spec.vectorize(&Offset::VectorConverter),
            rotation: self.rotation_animation_spec.vectorize(&f32::VectorConverter),
        }
    }
}

impl AnimationSpec<AnimationData> for DelegatingAnimationSpec {
    fn vectorize_1d(
        &self,
        c: &TwoWayConverter<AnimationData, AnimationVector1D>,
    ) -> Box<dyn VectorizedAnimationSpec<AnimationVector1D>> {
        Box::new(self.vectorize_into(c))
    }
    fn vectorize_2d(
        &self,
        c: &TwoWayConverter<AnimationData, AnimationVector2D>,
    ) -> Box<dyn VectorizedAnimationSpec<AnimationVector2D>> {
        Box::new(self.vectorize_into(c))
    }
    fn vectorize_3d(
        &self,
        c: &TwoWayConverter<AnimationData, AnimationVector3D>,
    ) -> Box<dyn VectorizedAnimationSpec<AnimationVector3D>> {
        Box::new(self.vectorize_into(c))
    }
    fn vectorize_4d(
        &self,
        c: &TwoWayConverter<AnimationData, AnimationVector4D>,
    ) -> Box<dyn VectorizedAnimationSpec<AnimationVector4D>> {
        Box::new(self.vectorize_into(c))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    /// (Compose's DelegatingAnimationSpec has no `equals` of its own: only itself.)
    fn equals(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| std::ptr::eq(o, self))
    }
}

/// DelegatingAnimationSpec on a converter's vectors (Compose's object of VectorizedFiniteAnimationSpec).
struct DelegatingVectorized<V: AnimationVector> {
    converter: TwoWayConverter<AnimationData, V>,
    zoom: Box<dyn VectorizedAnimationSpec<AnimationVector1D>>,
    offset: Box<dyn VectorizedAnimationSpec<AnimationVector2D>>,
    rotation: Box<dyn VectorizedAnimationSpec<AnimationVector1D>>,
}

/// An AnimationData's parts as vectors.
fn parts(d: AnimationData) -> (AnimationVector1D, AnimationVector2D, AnimationVector1D) {
    (AnimationVector1D(d.zoom), AnimationVector2D(d.offset.x, d.offset.y), AnimationVector1D(d.degrees))
}

impl<V: AnimationVector> DelegatingVectorized<V> {
    fn data(&self, v: V) -> AnimationData {
        self.converter.convert_from_vector(v)
    }

    fn pack(&self, zoom: AnimationVector1D, offset: AnimationVector2D, rotation: AnimationVector1D) -> V {
        self.converter.convert_to_vector(AnimationData {
            zoom: zoom.value,
            offset: Offset::new(offset.v1, offset.v2),
            degrees: rotation.value,
        })
    }
}

impl<V: AnimationVector> VectorizedAnimationSpec<V> for DelegatingVectorized<V> {
    fn get_duration_nanos(&self, initial_value: V, target_value: V, initial_velocity: V) -> i64 {
        let (iz, io, ir) = parts(self.data(initial_value));
        let (tz, to, tr) = parts(self.data(target_value));
        let (vz, vo, vr) = parts(self.data(initial_velocity));
        self.zoom
            .get_duration_nanos(iz, tz, vz)
            .max(self.offset.get_duration_nanos(io, to, vo))
            .max(self.rotation.get_duration_nanos(ir, tr, vr))
    }

    fn get_velocity_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: V,
        target_value: V,
        initial_velocity: V,
    ) -> V {
        let (iz, io, ir) = parts(self.data(initial_value));
        let (tz, to, tr) = parts(self.data(target_value));
        let (vz, vo, vr) = parts(self.data(initial_velocity));
        self.pack(
            self.zoom.get_velocity_from_nanos(play_time_nanos, iz, tz, vz),
            self.offset.get_velocity_from_nanos(play_time_nanos, io, to, vo),
            self.rotation.get_velocity_from_nanos(play_time_nanos, ir, tr, vr),
        )
    }

    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: V, target_value: V, initial_velocity: V) -> V {
        let (iz, io, ir) = parts(self.data(initial_value));
        let (tz, to, tr) = parts(self.data(target_value));
        let (vz, vo, vr) = parts(self.data(initial_velocity));
        self.pack(
            self.zoom.get_value_from_nanos(play_time_nanos, iz, tz, vz),
            self.offset.get_value_from_nanos(play_time_nanos, io, to, vo),
            self.rotation.get_value_from_nanos(play_time_nanos, ir, tr, vr),
        )
    }
}

impl<V: AnimationVector> VectorizedFiniteAnimationSpec<V> for DelegatingVectorized<V> {}

// ---------------------------------------------------------------------------------------------------------------
// The default state

/// A transformable state of a closure taking each transformation (Compose's DefaultTransformableState, made by
/// [`TransformableState()`]).
#[derive(Clone)]
pub struct DefaultTransformableState {
    inner: Rc<DefaultInner>,
}

/// What a default state calls with each transformation: the centroid (px, or `Offset::Unspecified`), the zoom, the
/// pan (px) and the turn (degrees clockwise).
pub type OnTransformation = Rc<dyn Fn(Offset, f32, Offset, f32)>;

struct DefaultInner {
    on_transformation: OnTransformation,
    transform_mutex: MutatorMutex,
    is_transforming_state: MutableState<bool>,
}

impl PartialEq for DefaultTransformableState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for DefaultTransformableState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DefaultTransformableState({:p})", Rc::as_ptr(&self.inner))
    }
}

struct DefaultTransformScope(OnTransformation);

impl TransformScope for DefaultTransformScope {
    fn transform_by(&self, zoom_change: f32, pan_change: Offset, rotation_change: f32) {
        self.transform_by_with_centroid(Offset::Unspecified, zoom_change, pan_change, rotation_change)
    }

    fn transform_by_with_centroid(&self, centroid: Offset, zoom_change: f32, pan_change: Offset, rotation_change: f32) {
        (self.0)(centroid, zoom_change, pan_change, rotation_change)
    }
}

/// Puts `is_transforming` back when the mutation ends however (Compose's `finally`).
struct TransformingFlag(MutableState<bool>);

impl Drop for TransformingFlag {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

impl TransformableState for DefaultTransformableState {
    fn transform_block(&self, transform_priority: MutatePriority, block: TransformBlock) -> LocalFuture<'_, ()> {
        let inner = self.inner.clone();
        let scope: Rc<dyn TransformScope> = Rc::new(DefaultTransformScope(inner.on_transformation.clone()));
        let mutation = named_args!(inner.transform_mutex.mutate_with(scope, transform_priority, {
            let inner = inner.clone();
            move |scope: Rc<dyn TransformScope>| async move {
                inner.is_transforming_state.set(true);
                let _flag = TransformingFlag(inner.is_transforming_state.clone());
                block(scope).await;
            }
        }));
        Box::pin(mutation)
    }

    fn is_transform_in_progress(&self) -> bool {
        self.inner.is_transforming_state.get()
    }
}

/// A transformable state of `on_transformation`, given the centroid (`Offset::Unspecified` when there is none),
/// the zoom, the pan and the turn of each transformation (Compose's `TransformableState(onTransformation)` of four
/// parameters; its deprecated one of three is a closure leaving the centroid out).
pub fn TransformableState(on_transformation: impl Fn(Offset, f32, Offset, f32) + 'static) -> DefaultTransformableState {
    DefaultTransformableState {
        inner: Rc::new(DefaultInner {
            on_transformation: Rc::new(on_transformation),
            transform_mutex: MutatorMutex::new(),
            is_transforming_state: mutable_state_of(false),
        }),
    }
}

/// A [`TransformableState()`] remembered where it is called, calling the latest `on_transformation` (Compose's
/// `rememberTransformableState`).
#[kompozable]
pub fn remember_transformable_state(
    on_transformation: impl Fn(Offset, f32, Offset, f32) + 'static,
) -> DefaultTransformableState {
    let lambda_state = remember_updated_state::<OnTransformation>(Rc::new(on_transformation));
    remember(move || TransformableState(move |c, z, p, r| (lambda_state.get())(c, z, p, r)))
}
