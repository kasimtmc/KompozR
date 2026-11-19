//! AnimatedImageVector (Compose's AnimatedImageVector.kt): a vector and its targets' animators; read from an
//! `<animated-vector>`'s XML (Compose's `loadAnimatedVectorResource`), and seen at a time of playing it
//! ([`vector_at`](AnimatedImageVector::vector_at), KompozR's).

use super::animator::{
    Animator, PathTimestamp, PropertyState, PropertyValues, PropertyValuesMap, StateVectorConfig,
    create_animation_spec, end_value, interpolate_path, path_time_spec,
};
use super::compat::animated_vector_parser::parse_animated_image_vector;
use super::compat::resources::{NoXmlResources, Resource, Resources, XmlResourceError, XmlResources};
use super::compat::xml::read_document;
use kompoz_animation::{
    AnimationVector, ColorVectorConverter, FiniteAnimationSpec, Segment, TwoWayConverter, VectorConverter,
};
use kompoz_graphics::{Color, ImageVector, ImageVectorBuilder, VectorGroup, VectorNode, VectorPath};
use kompoz_runtime::{State, mutable_state_of};
use kompoz_ui::draw::vector::{VectorConfig, VectorProperty};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// One of an animated vector's animations (Compose's AnimatedVectorTarget): the name of the group or path it
/// animates (a `<target android:name>`), and its animator.
#[derive(Clone, Debug)]
pub(crate) struct AnimatedVectorTarget {
    pub(crate) name: String,
    pub(crate) animator: Animator,
}

struct AnimatedData {
    image_vector: ImageVector,
    targets: Vec<AnimatedVectorTarget>,
    total_duration: i32,
}

/// An animated vector drawable (Compose's AnimatedImageVector): its vector (an `<animated-vector>`'s
/// `android:drawable`) and its targets' animators; [`remember_animated_vector_painter`](crate::
/// remember_animated_vector_painter) draws it at its start or its end, animating between them. Read from XML
/// ([`from_animated_vector_xml`](Self::from_animated_vector_xml)): Compose's `animatedVectorResource` comes with
/// KompozR's resources. Copies share it; equal when they are the same one (Compose's has no `equals`).
#[derive(Clone)]
pub struct AnimatedImageVector {
    data: Arc<AnimatedData>,
}

impl AnimatedImageVector {
    pub(crate) fn new(image_vector: ImageVector, targets: Vec<AnimatedVectorTarget>) -> AnimatedImageVector {
        // (fastMaxBy: the longest; none: 0)
        let total_duration = targets.iter().map(|t| t.animator.total_duration()).max().unwrap_or(0);
        AnimatedImageVector { data: Arc::new(AnimatedData { image_vector, targets, total_duration }) }
    }

    /// The vector it animates (Compose's `imageVector`).
    pub fn image_vector(&self) -> ImageVector {
        self.data.image_vector.clone()
    }

    /// How long all its animations take with their start delays and repeats, in ms (Compose's `totalDuration`;
    /// an infinite repeat's `Int.MAX_VALUE`).
    pub fn total_duration(&self) -> i32 {
        self.data.total_duration
    }

    pub(crate) fn targets(&self) -> &[AnimatedVectorTarget] {
        &self.data.targets
    }

    /// An `<animated-vector>` read from its XML as Android's resources are (Compose's `loadAnimatedVectorResource`
    /// with the framework's resources alone): its drawable and its targets' animations inline in `<aapt:attr>`s
    /// (as Android Studio and Shape Shifter write them), animators (`<set>`, `<objectAnimator>`,
    /// `<propertyValuesHolder>`, `<keyframe>`) and interpolators (`@android:interpolator/…`,
    /// `@android:anim/…_interpolator`, inline ones, `pathInterpolator`s). What it refers to elsewhere
    /// (`@animator/…`): [`from_animated_vector_xml_with`](Self::from_animated_vector_xml_with). An error where Android
    /// (or Compose's parser) throws: in Kotlin's syntax, Kotlin's `Result` (`.getOrThrow()`).
    pub fn from_animated_vector_xml(xml: &str) -> Result<AnimatedImageVector, XmlResourceError> {
        AnimatedImageVector::from_animated_vector_xml_with(xml, &NoXmlResources)
    }

    /// [`from_animated_vector_xml`](Self::from_animated_vector_xml), the resources it refers to (its drawable,
    /// animators, interpolators, colors, values, theme attributes) looked up in `resources`.
    pub fn from_animated_vector_xml_with(
        xml: &str,
        resources: &dyn XmlResources,
    ) -> Result<AnimatedImageVector, XmlResourceError> {
        let res = Resources::new(resources);
        let mut parser = res.get_xml(&Resource::Inline(read_document(xml)?))?.seek_to_start_tag()?;
        parse_animated_image_vector(&mut parser, res)
    }

    /// The vector `millis` into playing it from its start (KompozR's: what
    /// [`remember_animated_vector_painter`](crate::remember_animated_vector_painter) draws that far into a play
    /// from its start to its end): each animated property's value then, as its Transition's animation gives it
    /// (Compose's specs and math), its group's or path's own where nothing animates it. A property no group or path
    /// has panics, as the painter's (Compose's "Unknown propertyName").
    pub fn vector_at(&self, millis: f32) -> ImageVector {
        let overall_duration = self.total_duration();
        let play_time_nanos = (millis as f64 * 1_000_000.0) as i64;
        let forward = Segment::new(false, true);
        let mut configs: HashMap<String, StateVectorConfig> = HashMap::new();
        for target in self.targets() {
            let mut map = PropertyValuesMap::new();
            target.animator.collect_property_values(&mut map, overall_duration, 0);
            let mut config = StateVectorConfig::default();
            for (property_name, values) in &mut map {
                values.sort_timestamps();
                let state = values.state_at(play_time_nanos, overall_duration, &forward);
                config.set(property_name, state);
            }
            match configs.get_mut(&target.name) {
                Some(current) => current.merge(config),
                None => _ = configs.insert(target.name.clone(), config),
            }
        }
        let v = &self.data.image_vector;
        let mut builder = ImageVector::Builder(
            v.name(),
            v.default_width(),
            v.default_height(),
            v.viewport_width(),
            v.viewport_height(),
            v.tint_color(),
            v.tint_blend_mode(),
            v.auto_mirror(),
        );
        rebuild(v.root(), &configs, &mut builder);
        builder.build()
    }
}

/// The value of an animation of `T`s `play_time_nanos` in (Compose's TargetBasedAnimation of a transition's
/// animation from rest: no initial velocity; at its end its target).
fn value_at<T: Clone + 'static, V: AnimationVector>(
    spec: Arc<dyn FiniteAnimationSpec<T>>,
    converter: &TwoWayConverter<T, V>,
    initial: T,
    target: T,
    play_time_nanos: i64,
) -> T {
    let vectorized = V::vectorize_finite(&*spec, converter);
    let initial_vector = converter.convert_to_vector(initial);
    let target_vector = converter.convert_to_vector(target.clone());
    let velocity = initial_vector.new_instance();
    let duration = vectorized.get_duration_nanos(initial_vector, target_vector, velocity);
    if play_time_nanos >= duration {
        return target;
    }
    converter.convert_from_vector(vectorized.get_value_from_nanos(
        play_time_nanos,
        initial_vector,
        target_vector,
        velocity,
    ))
}

impl PropertyValues {
    /// A property's value `play_time_nanos` into playing forward, as a state of it (what the painter's state holds
    /// then).
    fn state_at(&self, play_time_nanos: i64, overall_duration: i32, forward: &Segment<bool>) -> PropertyState {
        match self {
            PropertyValues::Float(t) => {
                let spec = create_animation_spec(t, overall_duration, forward);
                let value =
                    value_at(spec, &f32::VectorConverter, end_value(t, false), end_value(t, true), play_time_nanos);
                PropertyState::Float(State::from(mutable_state_of(value)))
            }
            PropertyValues::Color(t) => {
                let spec = create_animation_spec(t, overall_duration, forward);
                let (initial, target) = (end_value(t, false), end_value(t, true));
                let converter = Color::VectorConverter(target.color_space());
                let value = value_at(spec, &converter, initial, target, play_time_nanos);
                PropertyState::Color(State::from(mutable_state_of(value)))
            }
            PropertyValues::Path(t) => {
                let time = value_at(
                    path_time_spec(overall_duration, forward),
                    &f32::VectorConverter,
                    0.0,
                    overall_duration as f32,
                    play_time_nanos,
                );
                let timestamps: Vec<PathTimestamp> = t.iter().map(PathTimestamp::of).collect();
                PropertyState::Path(State::from(mutable_state_of(interpolate_path(&timestamps, time))))
            }
        }
    }
}

/// `group`'s groups and paths into `builder`, each with its config's values (as RenderVectorGroup makes them).
fn rebuild(group: &VectorGroup, configs: &HashMap<String, StateVectorConfig>, builder: &mut ImageVectorBuilder) {
    use VectorProperty as P;
    let none = StateVectorConfig::default();
    for node in group {
        let name = match node {
            VectorNode::Path(p) => &p.name,
            VectorNode::Group(g) => &g.name,
        };
        let c = configs.get(name).unwrap_or(&none);
        match node {
            VectorNode::Path(p) => {
                builder.add_path_of(VectorPath {
                    name: p.name.clone(),
                    path_data: c.get_path_data(P::PathData, p.path_data.clone()),
                    path_fill_type: p.path_fill_type,
                    fill: c.get_brush(P::Fill, p.fill.clone()),
                    fill_alpha: c.get_float(P::FillAlpha, p.fill_alpha),
                    stroke: c.get_brush(P::Stroke, p.stroke.clone()),
                    stroke_alpha: c.get_float(P::StrokeAlpha, p.stroke_alpha),
                    stroke_line_width: c.get_float(P::StrokeLineWidth, p.stroke_line_width),
                    stroke_line_cap: p.stroke_line_cap,
                    stroke_line_join: p.stroke_line_join,
                    stroke_line_miter: p.stroke_line_miter,
                    trim_path_start: c.get_float(P::TrimPathStart, p.trim_path_start),
                    trim_path_end: c.get_float(P::TrimPathEnd, p.trim_path_end),
                    trim_path_offset: c.get_float(P::TrimPathOffset, p.trim_path_offset),
                });
            }
            VectorNode::Group(g) => {
                builder.add_group_of(VectorGroup {
                    name: g.name.clone(),
                    rotation: c.get_float(P::Rotation, g.rotation),
                    pivot_x: c.get_float(P::PivotX, g.pivot_x),
                    pivot_y: c.get_float(P::PivotY, g.pivot_y),
                    scale_x: c.get_float(P::ScaleX, g.scale_x),
                    scale_y: c.get_float(P::ScaleY, g.scale_y),
                    translation_x: c.get_float(P::TranslateX, g.translation_x),
                    translation_y: c.get_float(P::TranslateY, g.translation_y),
                    clip_path_data: c.get_path_data(P::PathData, g.clip_path_data.clone()),
                    children: Vec::new(),
                });
                rebuild(g, configs, builder);
                builder.clear_group();
            }
        }
    }
}

/// The same one.
impl PartialEq for AnimatedImageVector {
    fn eq(&self, other: &AnimatedImageVector) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }
}

impl fmt::Debug for AnimatedImageVector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("AnimatedImageVector")
            .field("total_duration", &self.total_duration())
            .field("image_vector", &self.data.image_vector)
            .field("targets", &self.data.targets)
            .finish()
    }
}
