//! A value animated as it changes (Compose's animation-core AnimateAsState.kt, and animation's
//! `animateColorAsState`): `animate_float_as_state`, `animate_dp_as_state`, … `animate_value_as_state`,
//! `animate_color_as_state` give a state of an [`Animatable`] animated to each new target the composition gives,
//! from where it is.

use crate::animatable::Animatable;
use crate::color::ColorVectorConverter;
use crate::converter::{TwoWayConverter, VectorConverter, VisibilityThreshold};
use crate::spec::{AnimationSpec, SpringSpec, spring};
use crate::state::AnimationValue;
use crate::vector::{AnimationVector, AnimationVector4D};
use kompoz_coroutines::{Channel, CoroutineScope, LaunchedEffect, coroutine_scope};
use kompoz_graphics::{Color, Dp, IntOffset, IntSize, Offset, Rect, Size};
use kompoz_runtime::{SideEffect, State, kompozable, named_args, remember, remember_keyed, remember_updated_state};
use std::rc::Rc;
use std::sync::{Arc, LazyLock};

/// What `animate_float_as_state` animates with unless told otherwise (Compose's `defaultAnimation`, told apart by
/// identity: given, the visibility threshold makes the spring).
static DefaultAnimation: LazyLock<Arc<dyn AnimationSpec<f32> + Send + Sync>> =
    LazyLock::new(|| Arc::new(spring::<f32>()));

/// Compose's DefaultFloatVisibilityThreshold.
const DefaultFloatVisibilityThreshold: f32 = 0.01;

fn default_animation() -> Arc<dyn AnimationSpec<f32>> {
    DefaultAnimation.clone()
}

fn default_spring<T: AnimationValue + VisibilityThreshold>() -> Arc<dyn AnimationSpec<T>> {
    Arc::new(SpringSpec::new(spring::<T>().damping_ratio, spring::<T>().stiffness, Some(T::VisibilityThreshold)))
}

/// A float animated to `target_value` each time it changes (Compose's `animateFloatAsState`): along
/// `animation_spec` (a spring by default, of `visibility_threshold`), from where it is (a new target on the way
/// goes on from there); `finished_listener` told its value when it arrives. Its value as a state: read in drawing
/// or layout, only that runs again as it animates. `label` tells it apart in tools.
#[kompozable]
pub fn animate_float_as_state(
    target_value: f32,
    #[default(default_animation())] animation_spec: Arc<dyn AnimationSpec<f32>>,
    #[default(DefaultFloatVisibilityThreshold)] visibility_threshold: f32,
    #[default("FloatAnimation".to_string())] label: String,
    #[default(None)] finished_listener: Option<Rc<dyn Fn(f32)>>,
) -> State<f32> {
    let resolved_anim_spec = if Arc::ptr_eq(&animation_spec, &default_animation()) {
        remember_keyed(visibility_threshold.to_bits(), move || {
            let spec: Arc<dyn AnimationSpec<f32>> = Arc::new(SpringSpec::new(
                spring::<f32>().damping_ratio,
                spring::<f32>().stiffness,
                Some(visibility_threshold),
            ));
            spec
        })
    } else {
        animation_spec
    };
    // (the default threshold isn't passed on: it would override the spec's own)
    let threshold =
        if visibility_threshold == DefaultFloatVisibilityThreshold { None } else { Some(visibility_threshold) };
    value_as_state(target_value, f32::VectorConverter, resolved_anim_spec, threshold, label, finished_listener)
}

/// A [`Dp`] animated to each new target (Compose's `animateDpAsState`): a spring of its visibility threshold by
/// default.
#[kompozable]
pub fn animate_dp_as_state(
    target_value: Dp,
    #[default(default_spring::<Dp>())] animation_spec: Arc<dyn AnimationSpec<Dp>>,
    #[default("DpAnimation".to_string())] label: String,
    #[default(None)] finished_listener: Option<Rc<dyn Fn(Dp)>>,
) -> State<Dp> {
    value_as_state(target_value, Dp::VectorConverter, animation_spec, None, label, finished_listener)
}

/// A [`Size`] animated to each new target (Compose's `animateSizeAsState`).
#[kompozable]
pub fn animate_size_as_state(
    target_value: Size,
    #[default(default_spring::<Size>())] animation_spec: Arc<dyn AnimationSpec<Size>>,
    #[default("SizeAnimation".to_string())] label: String,
    #[default(None)] finished_listener: Option<Rc<dyn Fn(Size)>>,
) -> State<Size> {
    value_as_state(target_value, Size::VectorConverter, animation_spec, None, label, finished_listener)
}

/// An [`Offset`] animated to each new target (Compose's `animateOffsetAsState`).
#[kompozable]
pub fn animate_offset_as_state(
    target_value: Offset,
    #[default(default_spring::<Offset>())] animation_spec: Arc<dyn AnimationSpec<Offset>>,
    #[default("OffsetAnimation".to_string())] label: String,
    #[default(None)] finished_listener: Option<Rc<dyn Fn(Offset)>>,
) -> State<Offset> {
    value_as_state(target_value, Offset::VectorConverter, animation_spec, None, label, finished_listener)
}

/// A [`Rect`] animated to each new target (Compose's `animateRectAsState`).
#[kompozable]
pub fn animate_rect_as_state(
    target_value: Rect,
    #[default(default_spring::<Rect>())] animation_spec: Arc<dyn AnimationSpec<Rect>>,
    #[default("RectAnimation".to_string())] label: String,
    #[default(None)] finished_listener: Option<Rc<dyn Fn(Rect)>>,
) -> State<Rect> {
    value_as_state(target_value, Rect::VectorConverter, animation_spec, None, label, finished_listener)
}

/// An `Int` animated to each new target (Compose's `animateIntAsState`).
#[kompozable]
pub fn animate_int_as_state(
    target_value: i32,
    #[default(default_spring::<i32>())] animation_spec: Arc<dyn AnimationSpec<i32>>,
    #[default("IntAnimation".to_string())] label: String,
    #[default(None)] finished_listener: Option<Rc<dyn Fn(i32)>>,
) -> State<i32> {
    value_as_state(target_value, i32::VectorConverter, animation_spec, None, label, finished_listener)
}

/// An [`IntOffset`] animated to each new target (Compose's `animateIntOffsetAsState`).
#[kompozable]
pub fn animate_int_offset_as_state(
    target_value: IntOffset,
    #[default(default_spring::<IntOffset>())] animation_spec: Arc<dyn AnimationSpec<IntOffset>>,
    #[default("IntOffsetAnimation".to_string())] label: String,
    #[default(None)] finished_listener: Option<Rc<dyn Fn(IntOffset)>>,
) -> State<IntOffset> {
    value_as_state(target_value, IntOffset::VectorConverter, animation_spec, None, label, finished_listener)
}

/// An [`IntSize`] animated to each new target (Compose's `animateIntSizeAsState`).
#[kompozable]
pub fn animate_int_size_as_state(
    target_value: IntSize,
    #[default(default_spring::<IntSize>())] animation_spec: Arc<dyn AnimationSpec<IntSize>>,
    #[default("IntSizeAnimation".to_string())] label: String,
    #[default(None)] finished_listener: Option<Rc<dyn Fn(IntSize)>>,
) -> State<IntSize> {
    value_as_state(target_value, IntSize::VectorConverter, animation_spec, None, label, finished_listener)
}

/// A [`Color`] animated to each new target, in Oklab (Compose animation's `animateColorAsState`): converted back
/// into the target's color space.
#[kompozable]
pub fn animate_color_as_state(
    target_value: Color,
    #[default(Arc::new(spring::<Color>()))] animation_spec: Arc<dyn AnimationSpec<Color>>,
    #[default("ColorAnimation".to_string())] label: String,
    #[default(None)] finished_listener: Option<Rc<dyn Fn(Color)>>,
) -> State<Color> {
    let color_space = target_value.color_space();
    let converter: TwoWayConverter<Color, AnimationVector4D> =
        remember_keyed(color_space.id(), move || Color::VectorConverter(color_space));
    value_as_state(target_value, converter, animation_spec, None, label, finished_listener)
}

/// A value of any type animated to each new target through its converter (Compose's `animateValueAsState`): a
/// `visibility_threshold` given replaces a spring spec's own. Its value as a state.
#[kompozable]
pub fn animate_value_as_state<T: AnimationValue, V: AnimationVector>(
    target_value: T,
    type_converter: TwoWayConverter<T, V>,
    #[default(Arc::new(spring::<T>()))] animation_spec: Arc<dyn AnimationSpec<T>>,
    #[default(None)] visibility_threshold: Option<T>,
    #[default("ValueAnimation".to_string())] label: String,
    #[default(None)] finished_listener: Option<Rc<dyn Fn(T)>>,
) -> State<T> {
    value_as_state(target_value, type_converter, animation_spec, visibility_threshold, label, finished_listener)
}

/// [`animate_value_as_state`]'s body (what the others call, all arguments given).
#[kompozable]
fn value_as_state<T: AnimationValue, V: AnimationVector>(
    target_value: T,
    type_converter: TwoWayConverter<T, V>,
    animation_spec: Arc<dyn AnimationSpec<T>>,
    visibility_threshold: Option<T>,
    label: String,
    finished_listener: Option<Rc<dyn Fn(T)>>,
) -> State<T> {
    let (initial, threshold) = (target_value.clone(), visibility_threshold.clone());
    let animatable = remember(move || Animatable::new(initial, type_converter, threshold, label));
    let listener = remember_updated_state!(finished_listener);
    let spec = match (&visibility_threshold, animation_spec.as_any().downcast_ref::<SpringSpec<T>>()) {
        (Some(threshold), Some(spring)) if spring.visibility_threshold.as_ref() != Some(threshold) => {
            let s: Arc<dyn AnimationSpec<T>> =
                Arc::new(SpringSpec::new(spring.damping_ratio, spring.stiffness, Some(threshold.clone())));
            s
        }
        _ => animation_spec,
    };
    let anim_spec = remember_updated_state!(spec);
    let channel: Channel<T> = remember(Channel::conflated);
    let ch = channel.clone();
    SideEffect(move || {
        ch.try_send(target_value);
    });
    let (ch, a) = (channel.clone(), animatable.clone());
    LaunchedEffect(channel, async move {
        coroutine_scope(move |scope: CoroutineScope| async move {
            while let Some(target) = ch.receive().await {
                // (two targets sent before this resumes: the last; an animation is no frame late)
                let new_target = ch.try_receive().unwrap_or(target);
                let (a, spec, listener) = (a.clone(), anim_spec.clone(), listener.clone());
                scope.launch(async move {
                    if new_target != a.target_value() {
                        named_args!(a.animate_to(new_target, animation_spec = spec.get())).await;
                        if let Some(l) = listener.get() {
                            l(a.value());
                        }
                    }
                });
            }
        })
        .await;
    });
    animatable.as_state()
}
