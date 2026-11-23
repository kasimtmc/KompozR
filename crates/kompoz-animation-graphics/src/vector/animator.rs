//! An animated vector's animators (Compose's Animator.kt): object animators (one or more property values holders
//! over a duration, after a start delay, repeated), sets of them (together or one after the other); their property
//! values gathered by property with the time each starts at (a `Timestamp`), played by a `Transition<Boolean>` as
//! states (a keyframes spec per timestamp, combined, reversed going back; path data by a time state through the
//! keyframes), a group's or path's states a `StateVectorConfig`.

use super::specs::{combined, reversed};
use kompoz_animation::{
    Easing, FiniteAnimationSpec, KeyframesSpec, KeyframesSpecConfig, LinearEasing, RepeatMode, RepeatableSpec, Segment,
    StartOffset, Transition, TweenSpec,
};
use kompoz_graphics::{Brush, Color, PathNode, SolidColor};
use kompoz_runtime::kt::panic_kt;
use kompoz_runtime::{State, derived_state_of, group, named_args, remember_keyed};
use kompoz_ui::draw::vector::{VectorConfig, VectorProperty};
use std::rc::Rc;
use std::sync::Arc;

/// An infinite repeat count (Android's ValueAnimator.INFINITE).
pub(crate) const RepeatCountInfinite: i32 = -1;

/// An animator (Compose's sealed Animator): an object animator or a set.
#[derive(Clone, Debug)]
pub(crate) enum Animator {
    Object(ObjectAnimator),
    Set(AnimatorSet),
}

/// The property values of an animator's properties, by property name in the order they were first met (Compose's
/// `MutableScatterMap<String, PropertyValues<*>>`).
pub(crate) type PropertyValuesMap = Vec<(String, PropertyValues)>;

impl Animator {
    /// How long it takes with its start delays and repeats (Compose's `totalDuration`; Kotlin's Int arithmetic,
    /// wrapping; an infinite repeat `Int.MAX_VALUE`).
    pub(crate) fn total_duration(&self) -> i32 {
        match self {
            Animator::Object(a) => a.total_duration(),
            Animator::Set(s) => s.total_duration(),
        }
    }

    /// Its property values, each property's timestamps from `parent_delay` (Compose's `collectPropertyValues`).
    pub(crate) fn collect_property_values(
        &self,
        map: &mut PropertyValuesMap,
        overall_duration: i32,
        parent_delay: i32,
    ) {
        match self {
            Animator::Object(a) => a.collect_property_values(map, overall_duration, parent_delay),
            Animator::Set(s) => s.collect_property_values(map, overall_duration, parent_delay),
        }
    }

    /// Its properties' states in `transition` as a config (Compose's composable `createVectorConfig`, `Configure`
    /// in it): its property values gathered once for `overall_duration` (remembered by it alone, as Compose's), each
    /// property's state made; a property no group or path has panics (Compose's "Unknown propertyName").
    #[track_caller]
    pub(crate) fn create_vector_config(
        &self,
        transition: &Transition<bool>,
        overall_duration: i32,
    ) -> StateVectorConfig {
        group(|| {
            let mut config = StateVectorConfig::default();
            self.configure(transition, &mut config, overall_duration);
            config
        })
    }

    #[track_caller]
    fn configure(&self, transition: &Transition<bool>, config: &mut StateVectorConfig, overall_duration: i32) {
        group(|| {
            let map: Rc<PropertyValuesMap> = remember_keyed(overall_duration, || {
                let mut map = PropertyValuesMap::new();
                self.collect_property_values(&mut map, overall_duration, 0);
                // (Compose sorts them at each composition: stable, so once is the same)
                for (_, values) in &mut map {
                    values.sort_timestamps();
                }
                Rc::new(map)
            });
            for (property_name, values) in map.iter() {
                let state = values.create_state(transition, property_name, overall_duration);
                config.set(property_name, state);
            }
        })
    }
}

/// One `<objectAnimator>` (Compose's ObjectAnimator): its duration, start delay, repeat count and mode, and its
/// property values holders.
#[derive(Clone, Debug)]
pub(crate) struct ObjectAnimator {
    pub(crate) duration: i32,
    pub(crate) start_delay: i32,
    pub(crate) repeat_count: i32,
    pub(crate) repeat_mode: RepeatMode,
    pub(crate) holders: Vec<PropertyValuesHolder>,
}

impl ObjectAnimator {
    /// A holder's timestamp at `time_millis` with this animator's duration and repeats.
    fn timestamp<T>(&self, time_millis: i32, holder: Arc<PropertyValuesHolder1D<T>>) -> Timestamp<T> {
        Timestamp {
            time_millis,
            duration_millis: self.duration,
            repeat_count: self.repeat_count,
            repeat_mode: self.repeat_mode,
            holder,
        }
    }

    fn total_duration(&self) -> i32 {
        if self.repeat_count == RepeatCountInfinite {
            i32::MAX
        } else {
            self.start_delay.wrapping_add(self.duration.wrapping_mul(self.repeat_count.wrapping_add(1)))
        }
    }

    fn collect_property_values(&self, map: &mut PropertyValuesMap, _overall_duration: i32, parent_delay: i32) {
        let time_millis = parent_delay.wrapping_add(self.start_delay);
        let stamp = |holder| self.timestamp(time_millis, holder);
        let stamp_color = |holder| self.timestamp(time_millis, holder);
        let stamp_path = |holder| self.timestamp(time_millis, holder);
        for holder in &self.holders {
            match holder {
                // (Compose: "TODO(b/178978971): Implement path animation")
                PropertyValuesHolder::TwoD(_) => {}
                PropertyValuesHolder::Float(h) => match values_of(map, &h.property_name, PropertyValues::Float) {
                    PropertyValues::Float(t) => t.push(stamp(h.clone())),
                    other => class_cast(other, "FloatPropertyValues"),
                },
                PropertyValuesHolder::Color(h) => match values_of(map, &h.property_name, PropertyValues::Color) {
                    PropertyValues::Color(t) => t.push(stamp_color(h.clone())),
                    other => class_cast(other, "ColorPropertyValues"),
                },
                PropertyValuesHolder::Path(h) => match values_of(map, &h.property_name, PropertyValues::Path) {
                    PropertyValues::Path(t) => t.push(stamp_path(h.clone())),
                    other => class_cast(other, "PathPropertyValues"),
                },
                // (Compose: "Not implemented since AVD does not use any Int property.")
                PropertyValuesHolder::Int(_) => {}
            }
        }
    }
}

/// The property values of `name` in `map`, new ones (`make` of none) if it has none.
fn values_of<'a, T>(
    map: &'a mut PropertyValuesMap,
    name: &str,
    make: impl FnOnce(Vec<Timestamp<T>>) -> PropertyValues,
) -> &'a mut PropertyValues {
    let at = match map.iter().position(|(n, _)| n == name) {
        Some(at) => at,
        None => {
            map.push((name.to_string(), make(Vec::new())));
            map.len() - 1
        }
    };
    &mut map[at].1
}

/// Kotlin's `as` of property values of another type.
fn class_cast(values: &PropertyValues, to: &str) -> ! {
    let from = match values {
        PropertyValues::Float(_) => "FloatPropertyValues",
        PropertyValues::Color(_) => "ColorPropertyValues",
        PropertyValues::Path(_) => "PathPropertyValues",
    };
    panic_kt("ClassCastException", &format!("class {from} cannot be cast to class {to}"))
}

/// Children played together or one after the other (Compose's Ordering).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Ordering {
    Together,
    Sequentially,
}

/// A `<set>` (Compose's AnimatorSet).
#[derive(Clone, Debug)]
pub(crate) struct AnimatorSet {
    pub(crate) animators: Vec<Animator>,
    pub(crate) ordering: Ordering,
}

impl AnimatorSet {
    fn total_duration(&self) -> i32 {
        match self.ordering {
            // (fastMaxBy: the first of the longest)
            Ordering::Together => self.animators.iter().map(Animator::total_duration).max().unwrap_or(0),
            Ordering::Sequentially => self.animators.iter().fold(0i32, |sum, a| sum.wrapping_add(a.total_duration())),
        }
    }

    fn collect_property_values(&self, map: &mut PropertyValuesMap, overall_duration: i32, parent_delay: i32) {
        match self.ordering {
            Ordering::Together => {
                for animator in &self.animators {
                    animator.collect_property_values(map, overall_duration, parent_delay);
                }
            }
            Ordering::Sequentially => {
                let mut accumulated_delay = parent_delay;
                for animator in &self.animators {
                    animator.collect_property_values(map, overall_duration, accumulated_delay);
                    accumulated_delay = accumulated_delay.wrapping_add(animator.total_duration());
                }
            }
        }
    }
}

// ---- Property values holders --------------------------------------------------------------------------------------

/// A value at a fraction of an animator's duration, eased to (Compose's Keyframe).
#[derive(Clone)]
pub(crate) struct Keyframe<T> {
    pub(crate) fraction: f32,
    pub(crate) value: T,
    pub(crate) interpolator: Arc<dyn Easing>,
}

impl<T: std::fmt::Debug> std::fmt::Debug for Keyframe<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Keyframe").field("fraction", &self.fraction).field("value", &self.value).finish()
    }
}

/// One property's keyframes (Compose's PropertyValuesHolder1D: Float, Int, Color or Path ones).
#[derive(Debug)]
pub(crate) struct PropertyValuesHolder1D<T> {
    pub(crate) property_name: String,
    pub(crate) animator_keyframes: Vec<Keyframe<T>>,
}

/// Two properties moved along a path (Compose's PropertyValuesHolder2D: read, not played, as Compose's).
#[cfg_attr(not(test), expect(dead_code, reason = "read as Compose reads it, played by none (Compose's b/178978971)"))]
pub(crate) struct PropertyValuesHolder2D {
    pub(crate) x_property_name: String,
    pub(crate) y_property_name: String,
    pub(crate) path_data: Vec<PathNode>,
    pub(crate) interpolator: Arc<dyn Easing>,
}

impl std::fmt::Debug for PropertyValuesHolder2D {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("PropertyValuesHolder2D")
            .field("x_property_name", &self.x_property_name)
            .field("y_property_name", &self.y_property_name)
            .field("path_data", &self.path_data)
            .finish()
    }
}

/// A property values holder (Compose's sealed PropertyValuesHolder).
#[derive(Clone, Debug)]
pub(crate) enum PropertyValuesHolder {
    #[cfg_attr(not(test), expect(dead_code, reason = "read, not played, as Compose's"))]
    TwoD(Arc<PropertyValuesHolder2D>),
    Float(Arc<PropertyValuesHolder1D<f32>>),
    #[cfg_attr(not(test), expect(dead_code, reason = "read, not played: no AVD property is an Int (Compose's)"))]
    Int(Arc<PropertyValuesHolder1D<i32>>),
    Color(Arc<PropertyValuesHolder1D<Color>>),
    Path(Arc<PropertyValuesHolder1D<Vec<PathNode>>>),
}

impl<T: Clone + 'static> PropertyValuesHolder1D<T> {
    /// Its keyframes as a keyframes spec over `duration` (Compose's `asKeyframeSpec`): each at its fraction of it
    /// (truncated to a millisecond), eased by its interpolator to the next.
    fn as_keyframe_spec(&self, duration: i32) -> KeyframesSpec<T> {
        let mut config = KeyframesSpecConfig::new();
        config.duration_millis = duration;
        for keyframe in &self.animator_keyframes {
            config
                .at(keyframe.value.clone(), (duration as f32 * keyframe.fraction) as i32)
                .using(keyframe.interpolator.clone());
        }
        KeyframesSpec::new(config)
    }
}

impl PropertyValuesHolder1D<Vec<PathNode>> {
    /// The path data `fraction` of the way (Compose's PropertyValuesHolderPath.interpolate): between the keyframes
    /// around it, eased by the later one's interpolator.
    fn interpolate(&self, fraction: f32) -> Vec<PathNode> {
        let k = &self.animator_keyframes;
        let first = k.iter().position(|kf| kf.fraction >= fraction).map_or(-1, |i| i as i64);
        let index = (first - 1).max(0) as usize;
        if index + 1 >= k.len() {
            panic_kt("IndexOutOfBoundsException", &format!("Index {} out of bounds for length {}", index + 1, k.len()));
        }
        let easing = &k[index + 1].interpolator;
        let inner_fraction = easing.transform(coerce_in(
            (fraction - k[index].fraction) / (k[index + 1].fraction - k[index].fraction),
            0.0,
            1.0,
        ));
        lerp_nodes(&k[index].value, &k[index + 1].value, inner_fraction)
    }
}

/// Kotlin's fastCoerceIn (a NaN passes).
fn coerce_in(v: f32, min: f32, max: f32) -> f32 {
    let mut v = v;
    if v < min {
        v = min;
    }
    if v > max {
        v = max;
    }
    v
}

// ---- Timestamps and property values -------------------------------------------------------------------------------

/// A holder's keyframes at a time in the whole (Compose's Timestamp).
pub(crate) struct Timestamp<T> {
    pub(crate) time_millis: i32,
    pub(crate) duration_millis: i32,
    pub(crate) repeat_count: i32,
    pub(crate) repeat_mode: RepeatMode,
    pub(crate) holder: Arc<PropertyValuesHolder1D<T>>,
}

impl<T: Clone + 'static> Timestamp<T> {
    /// Its keyframes spec, repeated as it says (an infinite repeat `Int.MAX_VALUE` times).
    fn as_animation_spec(&self) -> Arc<dyn FiniteAnimationSpec<T>> {
        let spec = self.holder.as_keyframe_spec(self.duration_millis);
        if self.repeat_count == 0 {
            Arc::new(spec)
        } else {
            let iterations =
                if self.repeat_count == RepeatCountInfinite { i32::MAX } else { self.repeat_count.wrapping_add(1) };
            Arc::new(RepeatableSpec::new(iterations, spec, self.repeat_mode, StartOffset::default()))
        }
    }
}

/// A property's timestamps (Compose's FloatPropertyValues, ColorPropertyValues, PathPropertyValues).
pub(crate) enum PropertyValues {
    Float(Vec<Timestamp<f32>>),
    Color(Vec<Timestamp<Color>>),
    Path(Vec<Timestamp<Vec<PathNode>>>),
}

/// A property's state (`State<Float>`, `State<Color>`, `State<List<PathNode>>`).
#[derive(Clone)]
pub(crate) enum PropertyState {
    Float(State<f32>),
    Color(State<Color>),
    Path(State<Vec<PathNode>>),
}

/// The spec a property of `timestamps` plays with in a segment (Compose's `createAnimationSpec`): its timestamps'
/// specs combined, reversed over `overall_duration` going back.
pub(crate) fn create_animation_spec<T: Clone + 'static>(
    timestamps: &[Timestamp<T>],
    overall_duration: i32,
    segment: &Segment<bool>,
) -> Arc<dyn FiniteAnimationSpec<T>> {
    let spec = combined(timestamps.iter().map(|t| (t.time_millis, t.as_animation_spec())).collect());
    if segment.target_state() { spec } else { reversed(spec, overall_duration) }
}

/// The value a property starts at (its first timestamp's first keyframe's) and ends at (its last's last).
pub(crate) fn end_value<T: Clone>(timestamps: &[Timestamp<T>], at_end: bool) -> T {
    let (Some(first), Some(last)) = (timestamps.first(), timestamps.last()) else {
        panic_kt("NoSuchElementException", "List is empty.")
    };
    let keyframe = if at_end { last.holder.animator_keyframes.last() } else { first.holder.animator_keyframes.first() };
    match keyframe {
        Some(k) => k.value.clone(),
        None => panic_kt("NoSuchElementException", "List is empty."),
    }
}

impl PropertyValues {
    pub(crate) fn sort_timestamps(&mut self) {
        match self {
            PropertyValues::Float(t) => t.sort_by_key(|t| t.time_millis),
            PropertyValues::Color(t) => t.sort_by_key(|t| t.time_millis),
            PropertyValues::Path(t) => t.sort_by_key(|t| t.time_millis),
        }
    }

    /// Its state in `transition` (Compose's composable `createState`): a float's `animateFloat`, a color's
    /// `animateColor`, of its specs, from its first value to its last; path data's a derived state of a time
    /// animated linearly over `overall_duration`.
    #[track_caller]
    fn create_state(&self, transition: &Transition<bool>, property_name: &str, overall_duration: i32) -> PropertyState {
        match self {
            PropertyValues::Float(t) => PropertyState::Float(named_args!(transition.animate_float(
                transition_spec = |segment: &Segment<bool>| create_animation_spec(t, overall_duration, segment),
                label = property_name.to_string(),
                target_value_by_state = |at_end: &bool| end_value(t, *at_end),
            ))),
            PropertyValues::Color(t) => PropertyState::Color(named_args!(transition.animate_color(
                transition_spec = |segment: &Segment<bool>| create_animation_spec(t, overall_duration, segment),
                label = property_name.to_string(),
                target_value_by_state = |at_end: &bool| end_value(t, *at_end),
            ))),
            PropertyValues::Path(t) => {
                let time_state = named_args!(transition.animate_float(
                    transition_spec = move |segment: &Segment<bool>| path_time_spec(overall_duration, segment),
                    label = property_name.to_string(),
                    target_value_by_state = move |at_end: &bool| if *at_end { overall_duration as f32 } else { 0.0 },
                ));
                let timestamps: Vec<PathTimestamp> = t.iter().map(PathTimestamp::of).collect();
                // (unremembered, as Compose's: b/279909531)
                PropertyState::Path(State::from(derived_state_of(move || {
                    interpolate_path(&timestamps, time_state.value())
                })))
            }
        }
    }
}

/// The spec of path data's time (Compose's): linear over the whole, reversed going back.
pub(crate) fn path_time_spec(overall_duration: i32, segment: &Segment<bool>) -> Arc<dyn FiniteAnimationSpec<f32>> {
    let spec: Arc<dyn FiniteAnimationSpec<f32>> = Arc::new(TweenSpec::<f32>::new(overall_duration, 0, LinearEasing));
    if segment.target_state() { spec } else { reversed(spec, overall_duration) }
}

/// A path timestamp as the time state's derived state keeps it.
#[derive(Clone)]
pub(crate) struct PathTimestamp {
    time_millis: i32,
    duration_millis: i32,
    repeat_count: i32,
    repeat_mode: RepeatMode,
    holder: Arc<PropertyValuesHolder1D<Vec<PathNode>>>,
}

impl PathTimestamp {
    pub(crate) fn of(t: &Timestamp<Vec<PathNode>>) -> PathTimestamp {
        PathTimestamp {
            time_millis: t.time_millis,
            duration_millis: t.duration_millis,
            repeat_count: t.repeat_count,
            repeat_mode: t.repeat_mode,
            holder: t.holder.clone(),
        }
    }
}

/// The path data `time_millis` in (Compose's PathPropertyValues.interpolate): the last timestamp started by then
/// (else the first), how far into it, through its repeats (back and forth when reversing).
pub(crate) fn interpolate_path(timestamps: &[PathTimestamp], time_millis: f32) -> Vec<PathNode> {
    let Some(timestamp) = timestamps.iter().rev().find(|t| t.time_millis as f32 <= time_millis).or(timestamps.first())
    else {
        panic_kt("NoSuchElementException", "List is empty.")
    };
    let mut fraction = (time_millis - timestamp.time_millis as f32) / timestamp.duration_millis as f32;
    if timestamp.repeat_count != 0 {
        let mut count = 0i32;
        while fraction > 1.0 {
            let less = fraction - 1.0;
            // (past 2^24 a float less 1 is itself: Compose's loop never ends there; this one stops)
            if less == fraction {
                break;
            }
            fraction = less;
            count = count.wrapping_add(1);
        }
        if timestamp.repeat_mode == RepeatMode::Reverse && count % 2 != 0 {
            fraction = 1.0 - fraction;
        }
    }
    timestamp.holder.interpolate(fraction)
}

// ---- The config of a group's or path's states ---------------------------------------------------------------------

/// A group's or path's animated properties' states (Compose's StateVectorConfig): what it is drawn with in place
/// of its own values, read where it is drawn (a state read: a frame of the animation composes it again).
#[derive(Clone, Default)]
pub(crate) struct StateVectorConfig {
    rotation: Option<State<f32>>,
    pivot_x: Option<State<f32>>,
    pivot_y: Option<State<f32>>,
    scale_x: Option<State<f32>>,
    scale_y: Option<State<f32>>,
    translate_x: Option<State<f32>>,
    translate_y: Option<State<f32>>,
    path_data: Option<State<Vec<PathNode>>>,
    fill_color: Option<State<Color>>,
    stroke_color: Option<State<Color>>,
    stroke_width: Option<State<f32>>,
    stroke_alpha: Option<State<f32>>,
    fill_alpha: Option<State<f32>>,
    trim_path_start: Option<State<f32>>,
    trim_path_end: Option<State<f32>>,
    trim_path_offset: Option<State<f32>>,
}

impl StateVectorConfig {
    /// `property_name`'s state set (Compose's `when (propertyName)` in `Configure`; another name panics as Compose's
    /// IllegalStateException; a state of another type as Kotlin's cast).
    pub(crate) fn set(&mut self, property_name: &str, state: PropertyState) {
        let float = |state: PropertyState| match state {
            PropertyState::Float(s) => Some(s),
            _ => cast_failed(property_name, "State<Float>"),
        };
        match property_name {
            "rotation" => self.rotation = float(state),
            "pivotX" => self.pivot_x = float(state),
            "pivotY" => self.pivot_y = float(state),
            "scaleX" => self.scale_x = float(state),
            "scaleY" => self.scale_y = float(state),
            "translateX" => self.translate_x = float(state),
            "translateY" => self.translate_y = float(state),
            "fillAlpha" => self.fill_alpha = float(state),
            "strokeWidth" => self.stroke_width = float(state),
            "strokeAlpha" => self.stroke_alpha = float(state),
            "trimPathStart" => self.trim_path_start = float(state),
            "trimPathEnd" => self.trim_path_end = float(state),
            "trimPathOffset" => self.trim_path_offset = float(state),
            "fillColor" => {
                self.fill_color = match state {
                    PropertyState::Color(s) => Some(s),
                    _ => cast_failed(property_name, "State<Color>"),
                }
            }
            "strokeColor" => {
                self.stroke_color = match state {
                    PropertyState::Color(s) => Some(s),
                    _ => cast_failed(property_name, "State<Color>"),
                }
            }
            "pathData" => {
                self.path_data = match state {
                    PropertyState::Path(s) => Some(s),
                    _ => cast_failed(property_name, "State<List<PathNode>>"),
                }
            }
            _ => panic_kt("IllegalStateException", &format!("Unknown propertyName: {property_name}")),
        }
    }

    /// `config`'s states over these (Compose's `merge`).
    pub(crate) fn merge(&mut self, config: StateVectorConfig) {
        macro_rules! take {
            ($($f:ident),*) => {
                $(if config.$f.is_some() { self.$f = config.$f; })*
            };
        }
        take!(
            rotation,
            pivot_x,
            pivot_y,
            scale_x,
            scale_y,
            translate_x,
            translate_y,
            path_data,
            fill_color,
            stroke_color,
            stroke_width,
            stroke_alpha,
            fill_alpha,
            trim_path_start,
            trim_path_end,
            trim_path_offset
        );
    }

    fn float_state(&self, property: VectorProperty) -> Option<&State<f32>> {
        use VectorProperty as P;
        match property {
            P::Rotation => self.rotation.as_ref(),
            P::PivotX => self.pivot_x.as_ref(),
            P::PivotY => self.pivot_y.as_ref(),
            P::ScaleX => self.scale_x.as_ref(),
            P::ScaleY => self.scale_y.as_ref(),
            P::TranslateX => self.translate_x.as_ref(),
            P::TranslateY => self.translate_y.as_ref(),
            P::FillAlpha => self.fill_alpha.as_ref(),
            P::StrokeLineWidth => self.stroke_width.as_ref(),
            P::StrokeAlpha => self.stroke_alpha.as_ref(),
            P::TrimPathStart => self.trim_path_start.as_ref(),
            P::TrimPathEnd => self.trim_path_end.as_ref(),
            P::TrimPathOffset => self.trim_path_offset.as_ref(),
            P::PathData | P::Fill | P::Stroke => None,
        }
    }
}

fn cast_failed(property_name: &str, to: &str) -> ! {
    panic_kt("ClassCastException", &format!("{property_name}'s state cannot be cast to {to}"))
}

/// Compose's `getOrDefault`: a state's value where it has one (a color as a solid brush), else the default.
impl VectorConfig for StateVectorConfig {
    fn get_float(&self, property: VectorProperty, default_value: f32) -> f32 {
        self.float_state(property).map_or(default_value, State::value)
    }
    fn get_path_data(&self, property: VectorProperty, default_value: Vec<PathNode>) -> Vec<PathNode> {
        match (property, &self.path_data) {
            (VectorProperty::PathData, Some(s)) => s.value(),
            _ => default_value,
        }
    }
    fn get_brush(&self, property: VectorProperty, default_value: Option<Brush>) -> Option<Brush> {
        let state = match property {
            VectorProperty::Fill => &self.fill_color,
            VectorProperty::Stroke => &self.stroke_color,
            _ => return default_value,
        };
        match state {
            Some(s) => Some(SolidColor(s.value())),
            None => default_value,
        }
    }
}

// ---- Path data morphs ---------------------------------------------------------------------------------------------

/// Kotlin's `lerp` of floats (androidx.compose.ui.util's).
fn lerp(start: f32, stop: f32, fraction: f32) -> f32 {
    (1.0 - fraction) * start + fraction * stop
}

/// Two lists of path data `fraction` between (Compose's private lerp of lists): node by node, as many as the
/// shorter has.
pub(crate) fn lerp_nodes(start: &[PathNode], stop: &[PathNode], fraction: f32) -> Vec<PathNode> {
    start.iter().zip(stop).map(|(a, b)| lerp_node(a, b, fraction)).collect()
}

/// Two nodes of the same kind `fraction` between (Compose's private lerp of PathNodes): their numbers; an arc's
/// flags the start's. Of different kinds: a panic, as Compose's `require`.
fn lerp_node(start: &PathNode, stop: &PathNode, f: f32) -> PathNode {
    use PathNode as N;
    let different = || -> ! { panic_kt("IllegalArgumentException", "start and stop path nodes have different types") };
    match (*start, *stop) {
        (N::RelativeMoveTo { dx, dy }, N::RelativeMoveTo { dx: dx2, dy: dy2 }) => {
            N::RelativeMoveTo { dx: lerp(dx, dx2, f), dy: lerp(dy, dy2, f) }
        }
        (N::RelativeMoveTo { .. }, _) => different(),
        (N::MoveTo { x, y }, N::MoveTo { x: x2, y: y2 }) => N::MoveTo { x: lerp(x, x2, f), y: lerp(y, y2, f) },
        (N::MoveTo { .. }, _) => different(),
        (N::RelativeLineTo { dx, dy }, N::RelativeLineTo { dx: dx2, dy: dy2 }) => {
            N::RelativeLineTo { dx: lerp(dx, dx2, f), dy: lerp(dy, dy2, f) }
        }
        (N::RelativeLineTo { .. }, _) => different(),
        (N::LineTo { x, y }, N::LineTo { x: x2, y: y2 }) => N::LineTo { x: lerp(x, x2, f), y: lerp(y, y2, f) },
        (N::LineTo { .. }, _) => different(),
        (N::RelativeHorizontalTo { dx }, N::RelativeHorizontalTo { dx: dx2 }) => {
            N::RelativeHorizontalTo { dx: lerp(dx, dx2, f) }
        }
        (N::RelativeHorizontalTo { .. }, _) => different(),
        (N::HorizontalTo { x }, N::HorizontalTo { x: x2 }) => N::HorizontalTo { x: lerp(x, x2, f) },
        (N::HorizontalTo { .. }, _) => different(),
        (N::RelativeVerticalTo { dy }, N::RelativeVerticalTo { dy: dy2 }) => {
            N::RelativeVerticalTo { dy: lerp(dy, dy2, f) }
        }
        (N::RelativeVerticalTo { .. }, _) => different(),
        (N::VerticalTo { y }, N::VerticalTo { y: y2 }) => N::VerticalTo { y: lerp(y, y2, f) },
        (N::VerticalTo { .. }, _) => different(),
        (
            N::RelativeCurveTo { dx1, dy1, dx2, dy2, dx3, dy3 },
            N::RelativeCurveTo { dx1: e1, dy1: f1, dx2: e2, dy2: f2, dx3: e3, dy3: f3 },
        ) => N::RelativeCurveTo {
            dx1: lerp(dx1, e1, f),
            dy1: lerp(dy1, f1, f),
            dx2: lerp(dx2, e2, f),
            dy2: lerp(dy2, f2, f),
            dx3: lerp(dx3, e3, f),
            dy3: lerp(dy3, f3, f),
        },
        (N::RelativeCurveTo { .. }, _) => different(),
        (N::CurveTo { x1, y1, x2, y2, x3, y3 }, N::CurveTo { x1: a1, y1: b1, x2: a2, y2: b2, x3: a3, y3: b3 }) => {
            N::CurveTo {
                x1: lerp(x1, a1, f),
                y1: lerp(y1, b1, f),
                x2: lerp(x2, a2, f),
                y2: lerp(y2, b2, f),
                x3: lerp(x3, a3, f),
                y3: lerp(y3, b3, f),
            }
        }
        (N::CurveTo { .. }, _) => different(),
        (
            N::RelativeReflectiveCurveTo { dx1, dy1, dx2, dy2 },
            N::RelativeReflectiveCurveTo { dx1: e1, dy1: f1, dx2: e2, dy2: f2 },
        ) => N::RelativeReflectiveCurveTo {
            dx1: lerp(dx1, e1, f),
            dy1: lerp(dy1, f1, f),
            dx2: lerp(dx2, e2, f),
            dy2: lerp(dy2, f2, f),
        },
        (N::RelativeReflectiveCurveTo { .. }, _) => different(),
        (N::ReflectiveCurveTo { x1, y1, x2, y2 }, N::ReflectiveCurveTo { x1: a1, y1: b1, x2: a2, y2: b2 }) => {
            N::ReflectiveCurveTo { x1: lerp(x1, a1, f), y1: lerp(y1, b1, f), x2: lerp(x2, a2, f), y2: lerp(y2, b2, f) }
        }
        (N::ReflectiveCurveTo { .. }, _) => different(),
        (N::RelativeQuadTo { dx1, dy1, dx2, dy2 }, N::RelativeQuadTo { dx1: e1, dy1: f1, dx2: e2, dy2: f2 }) => {
            N::RelativeQuadTo {
                dx1: lerp(dx1, e1, f),
                dy1: lerp(dy1, f1, f),
                dx2: lerp(dx2, e2, f),
                dy2: lerp(dy2, f2, f),
            }
        }
        (N::RelativeQuadTo { .. }, _) => different(),
        (N::QuadTo { x1, y1, x2, y2 }, N::QuadTo { x1: a1, y1: b1, x2: a2, y2: b2 }) => {
            N::QuadTo { x1: lerp(x1, a1, f), y1: lerp(y1, b1, f), x2: lerp(x2, a2, f), y2: lerp(y2, b2, f) }
        }
        (N::QuadTo { .. }, _) => different(),
        (N::RelativeReflectiveQuadTo { dx, dy }, N::RelativeReflectiveQuadTo { dx: dx2, dy: dy2 }) => {
            N::RelativeReflectiveQuadTo { dx: lerp(dx, dx2, f), dy: lerp(dy, dy2, f) }
        }
        (N::RelativeReflectiveQuadTo { .. }, _) => different(),
        (N::ReflectiveQuadTo { x, y }, N::ReflectiveQuadTo { x: x2, y: y2 }) => {
            N::ReflectiveQuadTo { x: lerp(x, x2, f), y: lerp(y, y2, f) }
        }
        (N::ReflectiveQuadTo { .. }, _) => different(),
        (
            N::RelativeArcTo {
                horizontal_ellipse_radius: a,
                vertical_ellipse_radius: b,
                theta,
                is_more_than_half,
                is_positive_arc,
                arc_start_dx,
                arc_start_dy,
            },
            N::RelativeArcTo {
                horizontal_ellipse_radius: a2,
                vertical_ellipse_radius: b2,
                theta: theta2,
                arc_start_dx: dx2,
                arc_start_dy: dy2,
                ..
            },
        ) => N::RelativeArcTo {
            horizontal_ellipse_radius: lerp(a, a2, f),
            vertical_ellipse_radius: lerp(b, b2, f),
            theta: lerp(theta, theta2, f),
            is_more_than_half,
            is_positive_arc,
            arc_start_dx: lerp(arc_start_dx, dx2, f),
            arc_start_dy: lerp(arc_start_dy, dy2, f),
        },
        (N::RelativeArcTo { .. }, _) => different(),
        (
            N::ArcTo {
                horizontal_ellipse_radius: a,
                vertical_ellipse_radius: b,
                theta,
                is_more_than_half,
                is_positive_arc,
                arc_start_x,
                arc_start_y,
            },
            N::ArcTo {
                horizontal_ellipse_radius: a2,
                vertical_ellipse_radius: b2,
                theta: theta2,
                arc_start_x: x2,
                arc_start_y: y2,
                ..
            },
        ) => N::ArcTo {
            horizontal_ellipse_radius: lerp(a, a2, f),
            vertical_ellipse_radius: lerp(b, b2, f),
            theta: lerp(theta, theta2, f),
            is_more_than_half,
            is_positive_arc,
            arc_start_x: lerp(arc_start_x, x2, f),
            arc_start_y: lerp(arc_start_y, y2, f),
        },
        (N::ArcTo { .. }, _) => different(),
        // (Compose's `PathNode.Close -> PathNode.Close`: no check of the stop)
        (N::Close, _) => N::Close,
    }
}
