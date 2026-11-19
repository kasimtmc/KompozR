//! Animator and interpolator resources (Compose's AnimatorResources.android.kt): an animator's XML read
//! (`<set>` or `<objectAnimator>`), an interpolator's (the framework's common ones as Compose's easings), and
//! Android's interpolators as easings (Compose's AccelerateDecelerateEasing, AccelerateEasing, … ; Android's own
//! anticipate-overshoot, bounce and path interpolators).

use super::path_interpolator::PathInterpolator;
use crate::vector::animator::Animator;
use crate::vector::compat::animator_parser::{TagObjectAnimator, TagSet, parse_animator_set, parse_object_animator};
use crate::vector::compat::resources::{Resource, Resources, XmlResourceError};
use kompoz_animation::{Easing, FastOutLinearInEasing, FastOutSlowInEasing, LinearEasing, LinearOutSlowInEasing};
use std::f64::consts::PI;
use std::sync::Arc;

/// Compose's `loadAnimatorResource`: a `<set>` or an `<objectAnimator>` (another tag: "Unknown tag").
pub(crate) fn load_animator_resource(res: Resources, resource: &Resource) -> Result<Animator, XmlResourceError> {
    let mut parser = res.get_xml(resource)?.seek_to_start_tag()?;
    match parser.name() {
        TagSet => Ok(Animator::Set(parse_animator_set(&mut parser, res)?)),
        TagObjectAnimator => Ok(Animator::Object(parse_object_animator(&mut parser, res)?)),
        name => Err(XmlResourceError::new("XmlPullParserException", format!("Unknown tag: {name}"))),
    }
}

/// Compose's AccelerateDecelerateEasing (Android's AccelerateDecelerateInterpolator, an animator's default):
/// cos((x + 1)π) / 2 + 0.5.
pub(crate) fn accelerate_decelerate_easing() -> Arc<dyn Easing> {
    Arc::new(|x: f32| (((x + 1.0) as f64 * PI).cos() / 2.0 + 0.5) as f32)
}

/// Compose's AccelerateEasing: x².
pub(crate) fn accelerate_easing() -> Arc<dyn Easing> {
    Arc::new(|x: f32| x * x)
}

/// Compose's `AccelerateEasing(factor)`: x to the power of twice the factor.
pub(crate) fn accelerate_easing_of(factor: f32) -> Arc<dyn Easing> {
    let power = factor * 2.0;
    Arc::new(move |x: f32| (x as f64).powf(power as f64) as f32)
}

/// Compose's `AnticipateEasing(tension)`: x²((tension + 1)x - tension).
pub(crate) fn anticipate_easing(tension: f32) -> Arc<dyn Easing> {
    Arc::new(move |x: f32| x * x * ((tension + 1.0) * x - tension))
}

/// Android's AnticipateOvershootInterpolator (Compose's `AnticipateOvershootEasing(tension, extraTension)`).
pub(crate) fn anticipate_overshoot_easing(tension: f32, extra_tension: f32) -> Arc<dyn Easing> {
    let tension = tension * extra_tension;
    let a = move |t: f32| t * t * ((tension + 1.0) * t - tension);
    let o = move |t: f32| t * t * ((tension + 1.0) * t + tension);
    Arc::new(move |t: f32| if t < 0.5 { 0.5 * a(t * 2.0) } else { 0.5 * (o(t * 2.0 - 2.0) + 2.0) })
}

/// Android's BounceInterpolator (Compose's BounceEasing).
pub(crate) fn bounce_easing() -> Arc<dyn Easing> {
    let bounce = |t: f32| t * t * 8.0;
    Arc::new(move |t: f32| {
        let t = t * 1.1226;
        if t < 0.3535 {
            bounce(t)
        } else if t < 0.7408 {
            bounce(t - 0.54719) + 0.7
        } else if t < 0.9644 {
            bounce(t - 0.8526) + 0.9
        } else {
            bounce(t - 1.0435) + 0.95
        }
    })
}

/// Compose's `CycleEasing(cycle)`: sin(2 · cycle · π · x).
pub(crate) fn cycle_easing(cycle: f32) -> Arc<dyn Easing> {
    let c = 2.0 * cycle;
    Arc::new(move |x: f32| (c as f64 * PI * x as f64).sin() as f32)
}

/// Compose's DecelerateEasing: 1 - (1 - x)².
pub(crate) fn decelerate_easing() -> Arc<dyn Easing> {
    Arc::new(|x: f32| 1.0 - (1.0 - x) * (1.0 - x))
}

/// Compose's `DecelerateEasing(factor)`: 1 - (1 - x) to the power of twice the factor.
pub(crate) fn decelerate_easing_of(factor: f32) -> Arc<dyn Easing> {
    let power = 2.0 * factor;
    Arc::new(move |x: f32| 1.0 - ((1.0 - x) as f64).powf(power as f64) as f32)
}

/// Compose's `OvershootEasing(tension)`: with t = x - 1, t²((tension + 1)t + tension) + 1.
pub(crate) fn overshoot_easing(tension: f32) -> Arc<dyn Easing> {
    Arc::new(move |x: f32| {
        let t = x - 1.0;
        t * t * ((tension + 1.0) * t + tension) + 1.0
    })
}

/// Android's PathInterpolator as an easing (Compose's `TimeInterpolator.toEasing()`).
pub(crate) fn path_interpolator_easing(interpolator: PathInterpolator) -> Arc<dyn Easing> {
    Arc::new(move |x: f32| interpolator.get_interpolation(x))
}

/// Compose's builtinInterpolators: the framework's interpolators it has easings of.
fn builtin_interpolator(resource: &Resource) -> Option<Arc<dyn Easing>> {
    let Resource::Named(r) = resource else { return None };
    if r.package.as_deref() != Some("android") || r.is_theme_attribute {
        return None;
    }
    Some(match (r.r#type.as_str(), r.name.as_str()) {
        ("anim", "linear_interpolator") | ("interpolator", "linear") => Arc::new(LinearEasing),
        ("interpolator", "fast_out_linear_in") => Arc::new(FastOutLinearInEasing),
        ("interpolator", "fast_out_slow_in") => Arc::new(FastOutSlowInEasing),
        ("interpolator", "linear_out_slow_in") => Arc::new(LinearOutSlowInEasing),
        _ => return None,
    })
}

/// Compose's `loadInterpolatorResource`: a framework one Compose has an easing of, else its XML read.
pub(crate) fn load_interpolator_resource(
    res: Resources,
    resource: &Resource,
) -> Result<Arc<dyn Easing>, XmlResourceError> {
    if let Some(easing) = builtin_interpolator(resource) {
        return Ok(easing);
    }
    let mut parser = res.get_xml(resource)?.seek_to_start_tag()?;
    crate::vector::compat::animator_parser::parse_interpolator(&mut parser, res)
}
