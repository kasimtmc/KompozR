//! Animators and interpolators read from XML (Compose's XmlAnimatorParser.android.kt): `<set>`, `<objectAnimator>`
//! (a property and its values, a path's two properties, or `<propertyValuesHolder>`s with `<keyframe>`s), and the
//! interpolator elements; each value's type taken from `valueType`, else from its values (colors), else a float.

use super::resources::{AttrDef, Resources, TypedArray, XmlResourceError, attrs};
use super::xml::{XmlEvent, XmlPullParser};
use crate::res::animator_resources::{
    accelerate_decelerate_easing, accelerate_easing, accelerate_easing_of, anticipate_easing,
    anticipate_overshoot_easing, bounce_easing, cycle_easing, decelerate_easing, decelerate_easing_of,
    load_interpolator_resource, overshoot_easing, path_interpolator_easing,
};
use crate::res::path_interpolator::PathInterpolator;
use crate::vector::animator::{
    Animator, AnimatorSet, Keyframe, ObjectAnimator, Ordering, PropertyValuesHolder, PropertyValuesHolder1D,
    PropertyValuesHolder2D,
};
use kompoz_animation::{CubicBezierEasing, Easing, LinearEasing, RepeatMode};
use kompoz_graphics::{Color, PathNode, add_path_nodes, to_path};
use std::sync::Arc;

pub(crate) const TagSet: &str = "set";
pub(crate) const TagObjectAnimator: &str = "objectAnimator";
const TagPropertyValuesHolder: &str = "propertyValuesHolder";
const TagKeyframe: &str = "keyframe";

const ValueTypeFloat: i32 = 0;
const ValueTypeInt: i32 = 1;
const ValueTypePath: i32 = 2;
const ValueTypeColor: i32 = 3;
const ValueTypeUndefined: i32 = 4;

const RepeatModeReverse: i32 = 2;

/// A value's type (Compose's ValueType).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ValueType {
    Float,
    Int,
    Color,
    Path,
}

const FallbackValueType: ValueType = ValueType::Float;

// (Compose's AndroidVectorResources' styleables, in their order)
const STYLEABLE_ANIMATOR: &[&AttrDef] = &[
    &attrs::INTERPOLATOR,
    &attrs::DURATION,
    &attrs::START_OFFSET,
    &attrs::REPEAT_COUNT,
    &attrs::REPEAT_MODE,
    &attrs::VALUE_FROM,
    &attrs::VALUE_TO,
    &attrs::VALUE_TYPE,
];
const STYLEABLE_ANIMATOR_INTERPOLATOR: usize = 0;
const STYLEABLE_ANIMATOR_DURATION: usize = 1;
const STYLEABLE_ANIMATOR_START_OFFSET: usize = 2;
const STYLEABLE_ANIMATOR_REPEAT_COUNT: usize = 3;
const STYLEABLE_ANIMATOR_REPEAT_MODE: usize = 4;
const STYLEABLE_ANIMATOR_VALUE_FROM: usize = 5;
const STYLEABLE_ANIMATOR_VALUE_TO: usize = 6;
const STYLEABLE_ANIMATOR_VALUE_TYPE: usize = 7;

const STYLEABLE_ANIMATOR_SET: &[&AttrDef] = &[&attrs::ORDERING];
const STYLEABLE_ANIMATOR_SET_ORDERING: usize = 0;

const STYLEABLE_PROPERTY_VALUES_HOLDER: &[&AttrDef] =
    &[&attrs::VALUE_FROM, &attrs::VALUE_TO, &attrs::VALUE_TYPE, &attrs::PROPERTY_NAME];
const STYLEABLE_PROPERTY_VALUES_HOLDER_VALUE_FROM: usize = 0;
const STYLEABLE_PROPERTY_VALUES_HOLDER_VALUE_TO: usize = 1;
const STYLEABLE_PROPERTY_VALUES_HOLDER_VALUE_TYPE: usize = 2;
const STYLEABLE_PROPERTY_VALUES_HOLDER_PROPERTY_NAME: usize = 3;

const STYLEABLE_KEYFRAME: &[&AttrDef] =
    &[&attrs::VALUE, &attrs::INTERPOLATOR, &attrs::VALUE_TYPE, &attrs::FRACTION_ATTR];
const STYLEABLE_KEYFRAME_VALUE: usize = 0;
const STYLEABLE_KEYFRAME_INTERPOLATOR: usize = 1;
const STYLEABLE_KEYFRAME_VALUE_TYPE: usize = 2;
const STYLEABLE_KEYFRAME_FRACTION: usize = 3;

const STYLEABLE_PROPERTY_ANIMATOR: &[&AttrDef] =
    &[&attrs::PROPERTY_NAME, &attrs::PATH_DATA, &attrs::PROPERTY_X_NAME, &attrs::PROPERTY_Y_NAME];
const STYLEABLE_PROPERTY_ANIMATOR_PROPERTY_NAME: usize = 0;
const STYLEABLE_PROPERTY_ANIMATOR_PATH_DATA: usize = 1;
const STYLEABLE_PROPERTY_ANIMATOR_PROPERTY_X_NAME: usize = 2;
const STYLEABLE_PROPERTY_ANIMATOR_PROPERTY_Y_NAME: usize = 3;

const STYLEABLE_ANTICIPATEOVERSHOOT_INTERPOLATOR: &[&AttrDef] = &[&attrs::TENSION, &attrs::EXTRA_TENSION];
const STYLEABLE_ANTICIPATEOVERSHOOT_INTERPOLATOR_TENSION: usize = 0;
const STYLEABLE_ANTICIPATEOVERSHOOT_INTERPOLATOR_EXTRA_TENSION: usize = 1;
const STYLEABLE_ACCELERATE_INTERPOLATOR: &[&AttrDef] = &[&attrs::FACTOR];
const STYLEABLE_DECELERATE_INTERPOLATOR: &[&AttrDef] = &[&attrs::FACTOR];
const STYLEABLE_CYCLE_INTERPOLATOR: &[&AttrDef] = &[&attrs::CYCLES];
const STYLEABLE_OVERSHOOT_INTERPOLATOR: &[&AttrDef] = &[&attrs::TENSION];
const STYLEABLE_PATH_INTERPOLATOR: &[&AttrDef] =
    &[&attrs::CONTROL_X_1, &attrs::CONTROL_Y_1, &attrs::CONTROL_X_2, &attrs::CONTROL_Y_2, &attrs::PATH_DATA];
const STYLEABLE_PATH_INTERPOLATOR_CONTROL_X_1: usize = 0;
const STYLEABLE_PATH_INTERPOLATOR_CONTROL_Y_1: usize = 1;
const STYLEABLE_PATH_INTERPOLATOR_CONTROL_X_2: usize = 2;
const STYLEABLE_PATH_INTERPOLATOR_CONTROL_Y_2: usize = 3;
const STYLEABLE_PATH_INTERPOLATOR_PATH_DATA: usize = 4;

fn null_pointer(what: &str) -> XmlResourceError {
    XmlResourceError::new("NullPointerException", format!("{what} is missing"))
}

/// A keyframe's value of whatever type it was read as (Compose's `Keyframe<Any>`).
#[derive(Clone, Debug)]
enum AnyValue {
    Float(f32),
    Int(i32),
    Color(Color),
    Path(Vec<PathNode>),
}

/// Compose's `TypedArray.getInterpolator`: the interpolator resource an attribute refers to, else `default_value`.
fn get_interpolator(
    a: &TypedArray,
    res: Resources,
    index: usize,
    default_value: Arc<dyn Easing>,
) -> Result<Arc<dyn Easing>, XmlResourceError> {
    match a.get_resource_id(index) {
        None => Ok(default_value),
        Some(resource) => load_interpolator_resource(res, &resource),
    }
}

/// Compose's `parseKeyframe`: a `<keyframe>`'s fraction, interpolator (the holder's by default) and value, of the
/// holder's type, else its own (its `valueType`, else a color if its value is one), else a float; and that type.
fn parse_keyframe(
    parser: &XmlPullParser,
    res: Resources,
    holder_value_type: Option<ValueType>,
    default_interpolator: &Arc<dyn Easing>,
) -> Result<(Keyframe<AnyValue>, ValueType), XmlResourceError> {
    let a = res.obtain_attributes(parser, STYLEABLE_KEYFRAME)?;
    let inferred_value_type = match holder_value_type {
        Some(t) => t,
        None => {
            let value_type = a.get_int(STYLEABLE_KEYFRAME_VALUE_TYPE, ValueTypeUndefined)?;
            let value = a.peek_value(STYLEABLE_KEYFRAME_VALUE).ok_or_else(|| null_pointer("a keyframe's value"))?;
            infer_value_type(value_type, &[value.is_color()]).unwrap_or(FallbackValueType)
        }
    };
    let fraction = a.get_float(STYLEABLE_KEYFRAME_FRACTION, 0.0)?;
    let interpolator = get_interpolator(&a, res, STYLEABLE_KEYFRAME_INTERPOLATOR, default_interpolator.clone())?;
    let keyframe = get_keyframe(&a, fraction, interpolator, inferred_value_type, STYLEABLE_KEYFRAME_VALUE)?;
    Ok((keyframe, inferred_value_type))
}

/// Compose's `TypedArray.getKeyframe`: the value at `value_index` read as `value_type`.
fn get_keyframe(
    a: &TypedArray,
    fraction: f32,
    interpolator: Arc<dyn Easing>,
    value_type: ValueType,
    value_index: usize,
) -> Result<Keyframe<AnyValue>, XmlResourceError> {
    let value = match value_type {
        ValueType::Float => AnyValue::Float(a.get_float(value_index, 0.0)?),
        ValueType::Int => AnyValue::Int(a.get_int(value_index, 0)?),
        ValueType::Color => AnyValue::Color(Color::from_argb(a.get_color(value_index, 0)? as u32)),
        ValueType::Path => AnyValue::Path(add_path_nodes(a.get_string(value_index).as_deref().unwrap_or(""))),
    };
    Ok(Keyframe { fraction, value, interpolator })
}

/// Compose's `parsePropertyValuesHolder`: a `<propertyValuesHolder>` and its `<keyframe>`s.
fn parse_property_values_holder(
    parser: &mut XmlPullParser,
    res: Resources,
    interpolator: &Arc<dyn Easing>,
) -> Result<PropertyValuesHolder, XmlResourceError> {
    let a = res.obtain_attributes(parser, STYLEABLE_PROPERTY_VALUES_HOLDER)?;
    let property_name = a
        .get_string(STYLEABLE_PROPERTY_VALUES_HOLDER_PROPERTY_NAME)
        .ok_or_else(|| null_pointer("a propertyValuesHolder's propertyName"))?;
    get_property_values_holder_1d(
        &a,
        property_name,
        STYLEABLE_PROPERTY_VALUES_HOLDER_VALUE_TYPE,
        STYLEABLE_PROPERTY_VALUES_HOLDER_VALUE_FROM,
        STYLEABLE_PROPERTY_VALUES_HOLDER_VALUE_TO,
        interpolator,
        |value_type, keyframes| {
            let mut vt: Option<ValueType> = None;
            parser.for_each_child_of(TagPropertyValuesHolder, |p| {
                if p.event_type() == XmlEvent::StartTag && p.name() == TagKeyframe {
                    let (keyframe, keyframe_value_type) = parse_keyframe(p, res, value_type, interpolator)?;
                    if vt.is_none() {
                        vt = Some(keyframe_value_type);
                    }
                    keyframes.push(keyframe);
                }
                Ok(())
            })?;
            // (this decides the holder's type)
            Ok(vt.or(value_type).unwrap_or(FallbackValueType))
        },
    )
}

/// Compose's `inferValueType`: the `valueType` written, else a color if every value is one, else none.
fn infer_value_type(value_type: i32, is_color: &[bool]) -> Option<ValueType> {
    match value_type {
        ValueTypeFloat => Some(ValueType::Float),
        ValueTypeInt => Some(ValueType::Int),
        ValueTypeColor => Some(ValueType::Color),
        ValueTypePath => Some(ValueType::Path),
        _ => is_color.iter().all(|c| *c).then_some(ValueType::Color),
    }
}

/// Compose's `TypedArray.getPropertyValuesHolder1D` (of an `<objectAnimator>` or a `<propertyValuesHolder>`): its
/// type inferred, `valueFrom` and `valueTo` keyframes at 0 and 1, the keyframes `parse_keyframes` reads (it gives
/// the type in the end), sorted by fraction.
fn get_property_values_holder_1d(
    a: &TypedArray,
    property_name: String,
    value_type_index: usize,
    value_from_index: usize,
    value_to_index: usize,
    interpolator: &Arc<dyn Easing>,
    parse_keyframes: impl FnOnce(Option<ValueType>, &mut Vec<Keyframe<AnyValue>>) -> Result<ValueType, XmlResourceError>,
) -> Result<PropertyValuesHolder, XmlResourceError> {
    let value_type = a.get_int(value_type_index, ValueTypeUndefined)?;
    let value_from = a.peek_value(value_from_index);
    let has_from = value_from.is_some();
    let color_from = value_from.is_some_and(|v| v.is_color());
    let value_to = a.peek_value(value_to_index);
    let has_to = value_to.is_some();
    let color_to = value_to.is_some_and(|v| v.is_color());
    let mut inferred_value_type = infer_value_type(value_type, &[color_from, color_to]);
    let mut keyframes = Vec::new();
    if inferred_value_type.is_none() && (has_from || has_to) {
        inferred_value_type = Some(ValueType::Float);
    }
    if let Some(t) = inferred_value_type {
        if has_from {
            keyframes.push(get_keyframe(a, 0.0, interpolator.clone(), t, value_from_index)?);
        }
        if has_to {
            keyframes.push(get_keyframe(a, 1.0, interpolator.clone(), t, value_to_index)?);
        }
    }
    let inferred_value_type = parse_keyframes(inferred_value_type, &mut keyframes)?;
    // (Kotlin's sortBy: stable)
    keyframes.sort_by(|x, y| x.fraction.total_cmp(&y.fraction));
    holder_of(property_name, inferred_value_type, keyframes)
}

/// The holder of `value_type` of the keyframes (Compose's unchecked cast of them: a keyframe read as another type
/// fails here, as Kotlin's cast does where it is used).
fn holder_of(
    property_name: String,
    value_type: ValueType,
    keyframes: Vec<Keyframe<AnyValue>>,
) -> Result<PropertyValuesHolder, XmlResourceError> {
    fn typed<T>(
        keyframes: Vec<Keyframe<AnyValue>>,
        name: &str,
        of: impl Fn(AnyValue) -> Option<T>,
    ) -> Result<Vec<Keyframe<T>>, XmlResourceError> {
        keyframes
            .into_iter()
            .map(|k| {
                let value = of(k.value.clone()).ok_or_else(|| {
                    XmlResourceError::new("ClassCastException", format!("{:?} cannot be cast to {name}", k.value))
                })?;
                Ok(Keyframe { fraction: k.fraction, value, interpolator: k.interpolator })
            })
            .collect()
    }
    Ok(match value_type {
        ValueType::Float => PropertyValuesHolder::Float(Arc::new(PropertyValuesHolder1D {
            animator_keyframes: typed(keyframes, "Float", |v| if let AnyValue::Float(f) = v { Some(f) } else { None })?,
            property_name,
        })),
        ValueType::Int => PropertyValuesHolder::Int(Arc::new(PropertyValuesHolder1D {
            animator_keyframes: typed(keyframes, "Int", |v| if let AnyValue::Int(i) = v { Some(i) } else { None })?,
            property_name,
        })),
        ValueType::Color => PropertyValuesHolder::Color(Arc::new(PropertyValuesHolder1D {
            animator_keyframes: typed(keyframes, "Color", |v| if let AnyValue::Color(c) = v { Some(c) } else { None })?,
            property_name,
        })),
        ValueType::Path => PropertyValuesHolder::Path(Arc::new(PropertyValuesHolder1D {
            animator_keyframes: typed(keyframes, "List<PathNode>", |v| {
                if let AnyValue::Path(p) = v { Some(p) } else { None }
            })?,
            property_name,
        })),
    })
}

fn convert_repeat_mode(repeat_mode: i32) -> RepeatMode {
    if repeat_mode == RepeatModeReverse { RepeatMode::Reverse } else { RepeatMode::Restart }
}

/// Compose's `parseObjectAnimator`: an `<objectAnimator>`: its interpolator (accelerate-decelerate by default), its
/// holders (a path's two properties; or its property and `<propertyValuesHolder>`s), duration (300), start offset,
/// repeat count and mode.
pub(crate) fn parse_object_animator(
    parser: &mut XmlPullParser,
    res: Resources,
) -> Result<ObjectAnimator, XmlResourceError> {
    let a = res.obtain_attributes(parser, STYLEABLE_ANIMATOR)?;
    let oa = res.obtain_attributes(parser, STYLEABLE_PROPERTY_ANIMATOR)?;
    let interpolator = get_interpolator(&a, res, STYLEABLE_ANIMATOR_INTERPOLATOR, accelerate_decelerate_easing())?;
    let mut holders = Vec::new();
    if let Some(path_data) = oa.get_string(STYLEABLE_PROPERTY_ANIMATOR_PATH_DATA) {
        // 2D: a path, and the properties of its x and y.
        holders.push(PropertyValuesHolder::TwoD(Arc::new(PropertyValuesHolder2D {
            x_property_name: oa
                .get_string(STYLEABLE_PROPERTY_ANIMATOR_PROPERTY_X_NAME)
                .ok_or_else(|| null_pointer("an objectAnimator's propertyXName"))?,
            y_property_name: oa
                .get_string(STYLEABLE_PROPERTY_ANIMATOR_PROPERTY_Y_NAME)
                .ok_or_else(|| null_pointer("an objectAnimator's propertyYName"))?,
            path_data: add_path_nodes(&path_data),
            interpolator: interpolator.clone(),
        })));
    } else {
        // 1D: a property, valueFrom and valueTo.
        if let Some(property_name) = oa.get_string(STYLEABLE_PROPERTY_ANIMATOR_PROPERTY_NAME) {
            holders.push(get_property_values_holder_1d(
                &a,
                property_name,
                STYLEABLE_ANIMATOR_VALUE_TYPE,
                STYLEABLE_ANIMATOR_VALUE_FROM,
                STYLEABLE_ANIMATOR_VALUE_TO,
                &interpolator,
                |vt, _| Ok(vt.unwrap_or(FallbackValueType)),
            )?);
        }
        // <propertyValuesHolder>s in it.
        parser.for_each_child_of(TagObjectAnimator, |p| {
            if p.event_type() == XmlEvent::StartTag && p.name() == TagPropertyValuesHolder {
                holders.push(parse_property_values_holder(p, res, &interpolator)?);
            }
            Ok(())
        })?;
    }
    Ok(ObjectAnimator {
        duration: a.get_int(STYLEABLE_ANIMATOR_DURATION, 300)?,
        start_delay: a.get_int(STYLEABLE_ANIMATOR_START_OFFSET, 0)?,
        repeat_count: a.get_int(STYLEABLE_ANIMATOR_REPEAT_COUNT, 0)?,
        repeat_mode: convert_repeat_mode(a.get_int(STYLEABLE_ANIMATOR_REPEAT_MODE, 0)?),
        holders,
    })
}

/// Compose's `parseAnimatorSet`: a `<set>` of `<set>`s and `<objectAnimator>`s, together unless its ordering
/// isn't 0.
pub(crate) fn parse_animator_set(parser: &mut XmlPullParser, res: Resources) -> Result<AnimatorSet, XmlResourceError> {
    let a = res.obtain_attributes(parser, STYLEABLE_ANIMATOR_SET)?;
    let ordering = a.get_int(STYLEABLE_ANIMATOR_SET_ORDERING, 0)?;
    let mut animators = Vec::new();
    parser.for_each_child_of(TagSet, |p| {
        if p.event_type() == XmlEvent::StartTag {
            match p.name() {
                TagSet => animators.push(Animator::Set(parse_animator_set(p, res)?)),
                TagObjectAnimator => animators.push(Animator::Object(parse_object_animator(p, res)?)),
                _ => {}
            }
        }
        Ok(())
    })?;
    Ok(AnimatorSet { animators, ordering: if ordering != 0 { Ordering::Sequentially } else { Ordering::Together } })
}

/// Compose's `parseInterpolator`: an interpolator element as an easing (another: "Unknown interpolator").
pub(crate) fn parse_interpolator(
    parser: &mut XmlPullParser,
    res: Resources,
) -> Result<Arc<dyn Easing>, XmlResourceError> {
    let name = parser.name().to_string();
    Ok(match name.as_str() {
        "linearInterpolator" => Arc::new(LinearEasing),
        "accelerateInterpolator" => {
            let a = res.obtain_attributes(parser, STYLEABLE_ACCELERATE_INTERPOLATOR)?;
            let factor = a.get_float(0, 1.0)?;
            if factor == 1.0 { accelerate_easing() } else { accelerate_easing_of(factor) }
        }
        "decelerateInterpolator" => {
            let a = res.obtain_attributes(parser, STYLEABLE_DECELERATE_INTERPOLATOR)?;
            let factor = a.get_float(0, 1.0)?;
            if factor == 1.0 { decelerate_easing() } else { decelerate_easing_of(factor) }
        }
        "accelerateDecelerateInterpolator" => accelerate_decelerate_easing(),
        "cycleInterpolator" => {
            let a = res.obtain_attributes(parser, STYLEABLE_CYCLE_INTERPOLATOR)?;
            cycle_easing(a.get_float(0, 1.0)?)
        }
        "anticipateInterpolator" => {
            let a = res.obtain_attributes(parser, STYLEABLE_ANTICIPATEOVERSHOOT_INTERPOLATOR)?;
            anticipate_easing(a.get_float(STYLEABLE_ANTICIPATEOVERSHOOT_INTERPOLATOR_TENSION, 2.0)?)
        }
        "overshootInterpolator" => {
            let a = res.obtain_attributes(parser, STYLEABLE_OVERSHOOT_INTERPOLATOR)?;
            overshoot_easing(a.get_float(0, 2.0)?)
        }
        "anticipateOvershootInterpolator" => {
            let a = res.obtain_attributes(parser, STYLEABLE_ANTICIPATEOVERSHOOT_INTERPOLATOR)?;
            anticipate_overshoot_easing(
                a.get_float(STYLEABLE_ANTICIPATEOVERSHOOT_INTERPOLATOR_TENSION, 2.0)?,
                a.get_float(STYLEABLE_ANTICIPATEOVERSHOOT_INTERPOLATOR_EXTRA_TENSION, 1.5)?,
            )
        }
        "bounceInterpolator" => bounce_easing(),
        "pathInterpolator" => {
            let a = res.obtain_attributes(parser, STYLEABLE_PATH_INTERPOLATOR)?;
            let illegal = |m: &str| XmlResourceError::new("IllegalArgumentException", m);
            if let Some(path_data) = a.get_string(STYLEABLE_PATH_INTERPOLATOR_PATH_DATA) {
                let path = to_path(&add_path_nodes(&path_data));
                path_interpolator_easing(PathInterpolator::of_path(&path).map_err(illegal)?)
            } else if !a.has_value(STYLEABLE_PATH_INTERPOLATOR_CONTROL_X_2)
                || !a.has_value(STYLEABLE_PATH_INTERPOLATOR_CONTROL_Y_2)
            {
                path_interpolator_easing(
                    PathInterpolator::quad(
                        a.get_float(STYLEABLE_PATH_INTERPOLATOR_CONTROL_X_1, 0.0)?,
                        a.get_float(STYLEABLE_PATH_INTERPOLATOR_CONTROL_Y_1, 0.0)?,
                    )
                    .map_err(illegal)?,
                )
            } else {
                Arc::new(CubicBezierEasing(
                    a.get_float(STYLEABLE_PATH_INTERPOLATOR_CONTROL_X_1, 0.0)?,
                    a.get_float(STYLEABLE_PATH_INTERPOLATOR_CONTROL_Y_1, 0.0)?,
                    a.get_float(STYLEABLE_PATH_INTERPOLATOR_CONTROL_X_2, 1.0)?,
                    a.get_float(STYLEABLE_PATH_INTERPOLATOR_CONTROL_Y_2, 1.0)?,
                ))
            }
        }
        _ => return Err(XmlResourceError::new("RuntimeException", format!("Unknown interpolator: {name}"))),
    })
}
