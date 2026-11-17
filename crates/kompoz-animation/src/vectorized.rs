//! Specs on vectors (Compose's VectorizedAnimationSpec.kt): what an `AnimationSpec` becomes for a converter's
//! vectors, and what animations ask for a value, a velocity and a duration.

use crate::arc_spline::{ArcSpline, ArcSplineArcAbove, ArcSplineArcBelow, ArcSplineArcStartLinear};
use crate::easing::{Easing, FastOutSlowInEasing, LinearEasing};
use crate::float_spec::{FloatAnimationSpec, FloatSpringSpec, FloatTweenSpec};
use crate::spec::{AnimationConstants, RepeatMode, StartOffset};
use crate::spring_simulation::Spring;
use crate::vector::AnimationVector;
use crate::{MillisToNanos, SecondsToMillis};
use kompoz_runtime::kt::panic_kt;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

/// An animation spec on vectors (Compose's VectorizedAnimationSpec): stateless as far as the animation goes — it is
/// given the start, the target and the start velocity each time. Vectors are values here.
pub trait VectorizedAnimationSpec<V: AnimationVector> {
    /// Whether it never ends (infiniteRepeatable's).
    fn is_infinite(&self) -> bool {
        false
    }

    /// The value `play_time_nanos` into it.
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: V, target_value: V, initial_velocity: V) -> V;

    /// The velocity `play_time_nanos` into it.
    fn get_velocity_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: V,
        target_value: V,
        initial_velocity: V,
    ) -> V;

    /// How long it takes.
    fn get_duration_nanos(&self, initial_value: V, target_value: V, initial_velocity: V) -> i64;

    /// The velocity at its end.
    fn get_end_velocity(&self, initial_value: V, target_value: V, initial_velocity: V) -> V {
        self.get_velocity_from_nanos(
            self.get_duration_nanos(initial_value, target_value, initial_velocity),
            initial_value,
            target_value,
            initial_velocity,
        )
    }
}

/// One that ends (Compose's VectorizedFiniteAnimationSpec).
pub trait VectorizedFiniteAnimationSpec<V: AnimationVector>: VectorizedAnimationSpec<V> {}

/// One of a fixed duration after a delay (Compose's VectorizedDurationBasedAnimationSpec): its duration is
/// `(delay_millis + duration_millis)` ms whatever it animates.
pub trait VectorizedDurationBasedAnimationSpec<V: AnimationVector>: VectorizedFiniteAnimationSpec<V> {
    fn duration_millis(&self) -> i32;
    fn delay_millis(&self) -> i32;
}

/// A duration-based spec's duration (Compose's default getDurationNanos of one).
pub(crate) fn duration_based_nanos(duration_millis: i32, delay_millis: i32) -> i64 {
    (delay_millis as i64 + duration_millis as i64) * MillisToNanos
}

/// The play time within a duration-based spec's duration after its delay (Compose's clampPlayTime).
pub(crate) fn clamp_play_time(duration_millis: i32, delay_millis: i32, play_time: i64) -> i64 {
    clamp_i64(play_time - delay_millis as i64, 0, duration_millis as i64)
}

/// Kotlin's fastCoerceIn of a Long: the lower bound first, then the upper.
pub(crate) fn clamp_i64(v: i64, min: i64, max: i64) -> i64 {
    let mut v = v;
    if v < min {
        v = min;
    }
    if v > max {
        v = max;
    }
    v
}

/// Kotlin's `lerp` of floats (animation-core's: `start * (1 - fraction) + stop * fraction`).
pub(crate) fn lerp(start: f32, stop: f32, fraction: f32) -> f32 {
    start * (1.0 - fraction) + stop * fraction
}

// ---- Floats per dimension (VectorizedFloatAnimationSpec) ------------------------------------------------------

/// The float specs of each dimension (Compose's internal Animations).
#[derive(Clone)]
pub(crate) enum Animations {
    /// The same one for every dimension.
    One(Arc<dyn FloatAnimationSpec>),
    /// One for each dimension.
    Each(Vec<Arc<dyn FloatAnimationSpec>>),
}

impl Animations {
    fn get(&self, index: usize) -> &dyn FloatAnimationSpec {
        match self {
            Animations::One(a) => &**a,
            Animations::Each(v) => &*v[index],
        }
    }
}

/// A float spec run on each of a vector's floats (Compose's VectorizedFloatAnimationSpec).
pub struct VectorizedFloatAnimationSpec<V> {
    anims: Animations,
    _v: std::marker::PhantomData<fn() -> V>,
}

impl<V: AnimationVector> VectorizedFloatAnimationSpec<V> {
    /// The same float spec on each float.
    pub fn new(anim: Arc<dyn FloatAnimationSpec>) -> VectorizedFloatAnimationSpec<V> {
        VectorizedFloatAnimationSpec { anims: Animations::One(anim), _v: std::marker::PhantomData }
    }

    pub(crate) fn of(anims: Animations) -> VectorizedFloatAnimationSpec<V> {
        VectorizedFloatAnimationSpec { anims, _v: std::marker::PhantomData }
    }
}

impl<V: AnimationVector> VectorizedAnimationSpec<V> for VectorizedFloatAnimationSpec<V> {
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: V, target_value: V, initial_velocity: V) -> V {
        let mut value = initial_value.new_instance();
        for i in 0..V::SIZE {
            value.set(
                i,
                self.anims.get(i).get_value_from_nanos(
                    play_time_nanos,
                    initial_value.get(i),
                    target_value.get(i),
                    initial_velocity.get(i),
                ),
            );
        }
        value
    }
    fn get_velocity_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: V,
        target_value: V,
        initial_velocity: V,
    ) -> V {
        let mut velocity = initial_velocity.new_instance();
        for i in 0..V::SIZE {
            velocity.set(
                i,
                self.anims.get(i).get_velocity_from_nanos(
                    play_time_nanos,
                    initial_value.get(i),
                    target_value.get(i),
                    initial_velocity.get(i),
                ),
            );
        }
        velocity
    }
    fn get_end_velocity(&self, initial_value: V, target_value: V, initial_velocity: V) -> V {
        let mut velocity = initial_velocity.new_instance();
        for i in 0..V::SIZE {
            velocity.set(
                i,
                self.anims.get(i).get_end_velocity(initial_value.get(i), target_value.get(i), initial_velocity.get(i)),
            );
        }
        velocity
    }
    fn get_duration_nanos(&self, initial_value: V, target_value: V, initial_velocity: V) -> i64 {
        let mut max_duration = 0i64;
        for i in 0..V::SIZE {
            max_duration = max_duration.max(self.anims.get(i).get_duration_nanos(
                initial_value.get(i),
                target_value.get(i),
                initial_velocity.get(i),
            ));
        }
        max_duration
    }
}

impl<V: AnimationVector> VectorizedFiniteAnimationSpec<V> for VectorizedFloatAnimationSpec<V> {}

// ---- Spring and tween -----------------------------------------------------------------------------------------

/// A spring on each float (Compose's VectorizedSpringSpec): each dimension's with its visibility threshold.
pub struct VectorizedSpringSpec<V> {
    pub damping_ratio: f32,
    pub stiffness: f32,
    anim: VectorizedFloatAnimationSpec<V>,
}

impl<V: AnimationVector> VectorizedSpringSpec<V> {
    /// A spring of that damping ratio and stiffness, each dimension with its own visibility threshold (`None`:
    /// floats' 0.01).
    pub fn new(damping_ratio: f32, stiffness: f32, visibility_threshold: Option<V>) -> VectorizedSpringSpec<V> {
        let anims = match visibility_threshold {
            None if damping_ratio == Spring::DampingRatioNoBouncy && stiffness == Spring::StiffnessMedium => {
                Animations::One(default_spring_animation())
            }
            Some(threshold) => Animations::Each(
                (0..V::SIZE)
                    .map(|i| {
                        Arc::new(FloatSpringSpec::new(damping_ratio, stiffness, threshold.get(i)))
                            as Arc<dyn FloatAnimationSpec>
                    })
                    .collect(),
            ),
            None => Animations::One(Arc::new(FloatSpringSpec::new(
                damping_ratio,
                stiffness,
                Spring::DefaultDisplacementThreshold,
            ))),
        };
        VectorizedSpringSpec { damping_ratio, stiffness, anim: VectorizedFloatAnimationSpec::of(anims) }
    }
}

/// The default spring's float spec (Compose's DefaultSpringAnimations), shared.
fn default_spring_animation() -> Arc<dyn FloatAnimationSpec> {
    static DEFAULT: std::sync::OnceLock<Arc<dyn FloatAnimationSpec>> = std::sync::OnceLock::new();
    DEFAULT
        .get_or_init(|| {
            Arc::new(FloatSpringSpec::new(
                Spring::DampingRatioNoBouncy,
                Spring::StiffnessMedium,
                Spring::DefaultDisplacementThreshold,
            ))
        })
        .clone()
}

macro_rules! delegate_to_anim {
    ($ty:ident) => {
        impl<V: AnimationVector> VectorizedAnimationSpec<V> for $ty<V> {
            fn get_value_from_nanos(&self, t: i64, initial_value: V, target_value: V, initial_velocity: V) -> V {
                self.anim.get_value_from_nanos(t, initial_value, target_value, initial_velocity)
            }
            fn get_velocity_from_nanos(&self, t: i64, initial_value: V, target_value: V, initial_velocity: V) -> V {
                self.anim.get_velocity_from_nanos(t, initial_value, target_value, initial_velocity)
            }
            fn get_duration_nanos(&self, initial_value: V, target_value: V, initial_velocity: V) -> i64 {
                delegate_to_anim!(@duration $ty, self, initial_value, target_value, initial_velocity)
            }
            fn get_end_velocity(&self, initial_value: V, target_value: V, initial_velocity: V) -> V {
                delegate_to_anim!(@end $ty, self, initial_value, target_value, initial_velocity)
            }
        }
        impl<V: AnimationVector> VectorizedFiniteAnimationSpec<V> for $ty<V> {}
    };
    (@duration VectorizedSpringSpec, $s:ident, $a:ident, $b:ident, $c:ident) => {
        $s.anim.get_duration_nanos($a, $b, $c)
    };
    (@duration VectorizedTweenSpec, $s:ident, $a:ident, $b:ident, $c:ident) => {{
        let _ = ($a, $b, $c);
        duration_based_nanos($s.duration_millis, $s.delay_millis)
    }};
    (@end VectorizedSpringSpec, $s:ident, $a:ident, $b:ident, $c:ident) => {
        $s.anim.get_end_velocity($a, $b, $c)
    };
    // (a tween's is the interface's default: its velocity at its end)
    (@end VectorizedTweenSpec, $s:ident, $a:ident, $b:ident, $c:ident) => {
        $s.get_velocity_from_nanos($s.get_duration_nanos($a, $b, $c), $a, $b, $c)
    };
}

delegate_to_anim!(VectorizedSpringSpec);

/// A tween on each float (Compose's VectorizedTweenSpec): `duration_millis` after `delay_millis`, along `easing`.
pub struct VectorizedTweenSpec<V> {
    pub duration_millis: i32,
    pub delay_millis: i32,
    pub easing: Arc<dyn Easing>,
    anim: VectorizedFloatAnimationSpec<V>,
}

impl<V: AnimationVector> VectorizedTweenSpec<V> {
    pub fn new(duration_millis: i32, delay_millis: i32, easing: Arc<dyn Easing>) -> VectorizedTweenSpec<V> {
        let anim = VectorizedFloatAnimationSpec::new(Arc::new(FloatTweenSpec::new(
            duration_millis,
            delay_millis,
            easing.clone(),
        )));
        VectorizedTweenSpec { duration_millis, delay_millis, easing, anim }
    }
}

impl<V: AnimationVector> Default for VectorizedTweenSpec<V> {
    /// Compose's defaults: 300 ms, no delay, FastOutSlowIn.
    fn default() -> Self {
        VectorizedTweenSpec::new(AnimationConstants::DefaultDurationMillis, 0, Arc::new(FastOutSlowInEasing))
    }
}

delegate_to_anim!(VectorizedTweenSpec);

impl<V: AnimationVector> VectorizedDurationBasedAnimationSpec<V> for VectorizedTweenSpec<V> {
    fn duration_millis(&self) -> i32 {
        self.duration_millis
    }
    fn delay_millis(&self) -> i32 {
        self.delay_millis
    }
}

// ---- Snap -----------------------------------------------------------------------------------------------------

/// The target at once after a delay (Compose's VectorizedSnapSpec).
pub struct VectorizedSnapSpec<V> {
    pub delay_millis: i32,
    _v: std::marker::PhantomData<fn() -> V>,
}

impl<V: AnimationVector> VectorizedSnapSpec<V> {
    pub fn new(delay_millis: i32) -> VectorizedSnapSpec<V> {
        VectorizedSnapSpec { delay_millis, _v: std::marker::PhantomData }
    }
}

impl<V: AnimationVector> VectorizedAnimationSpec<V> for VectorizedSnapSpec<V> {
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: V, target_value: V, _: V) -> V {
        if play_time_nanos < self.delay_millis as i64 * MillisToNanos { initial_value } else { target_value }
    }
    fn get_velocity_from_nanos(&self, _: i64, _: V, _: V, initial_velocity: V) -> V {
        initial_velocity
    }
    fn get_duration_nanos(&self, _: V, _: V, _: V) -> i64 {
        duration_based_nanos(0, self.delay_millis)
    }
}
impl<V: AnimationVector> VectorizedFiniteAnimationSpec<V> for VectorizedSnapSpec<V> {}
impl<V: AnimationVector> VectorizedDurationBasedAnimationSpec<V> for VectorizedSnapSpec<V> {
    fn duration_millis(&self) -> i32 {
        0
    }
    fn delay_millis(&self) -> i32 {
        self.delay_millis
    }
}

// ---- Keyframes -------------------------------------------------------------------------------------------------

/// How a keyframe's stretch goes between its two values when they are 2D (Compose's ArcMode): along a quarter of
/// an ellipse, above or below, or a straight line.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ArcMode(pub(crate) i32);

impl ArcMode {
    /// Arcing up (the ellipse's quarter above the line).
    pub const ArcAbove: ArcMode = ArcMode(ArcSplineArcAbove);
    /// Arcing down.
    pub const ArcBelow: ArcMode = ArcMode(ArcSplineArcBelow);
    /// A straight line (the default).
    pub const ArcLinear: ArcMode = ArcMode(ArcSplineArcStartLinear);
}

/// A keyframe on vectors (Compose's internal VectorizedKeyframeSpecElementInfo).
#[derive(Clone)]
pub(crate) struct VectorizedKeyframeSpecElementInfo<V> {
    pub(crate) vector_value: V,
    pub(crate) easing: Arc<dyn Easing>,
    pub(crate) arc_mode: ArcMode,
}

/// What a keyframes spec keeps of the last values it was asked about (Compose's lazily set fields).
struct KeyframesCache<V> {
    initialized: bool,
    modes: Vec<i32>,
    times: Vec<f32>,
    last_initial_value: Option<V>,
    last_target_value: Option<V>,
    pos_array: Vec<f32>,
    slope_array: Vec<f32>,
    arc_spline: Option<ArcSpline>,
}

/// Values at given times, eased between (Compose's VectorizedKeyframesSpec): times in ms from 0 (the start
/// value unless a keyframe is there) to `duration_millis` (the target unless one is there); with arc modes, 2D
/// stretches arc.
pub struct VectorizedKeyframesSpec<V> {
    timestamps: Vec<i32>,
    keyframes: HashMap<i32, VectorizedKeyframeSpecElementInfo<V>>,
    duration_millis: i32,
    delay_millis: i32,
    default_easing: Arc<dyn Easing>,
    initial_arc_mode: ArcMode,
    cache: RefCell<KeyframesCache<V>>,
}

/// Sorted keyframe times with 0 and the duration (Compose adds them where no keyframe is).
pub(crate) fn keyframe_timestamps(
    keys: impl Iterator<Item = i32>,
    contains: impl Fn(i32) -> bool,
    duration: i32,
) -> Vec<i32> {
    let mut timestamps: Vec<i32> = keys.collect();
    if !contains(0) {
        timestamps.insert(0, 0);
    }
    if !contains(duration) {
        timestamps.push(duration);
    }
    timestamps.sort();
    timestamps
}

/// androidx.collection's IntList.binarySearch: the index of `element`, or `-(insertion point) - 1`.
pub(crate) fn int_binary_search(list: &[i32], element: i32) -> i32 {
    let (mut low, mut high) = (0i32, list.len() as i32 - 1);
    while low <= high {
        let mid = ((low + high) as u32 >> 1) as i32;
        let mid_val = list[mid as usize];
        if mid_val < element {
            low = mid + 1;
        } else if mid_val > element {
            high = mid - 1;
        } else {
            return mid;
        }
    }
    -(low + 1)
}

impl<V: AnimationVector> VectorizedKeyframesSpec<V> {
    /// Keyframes of `(vector, easing)` at times in ms (Compose's public constructor; straight lines).
    pub fn new(keyframes: HashMap<i32, (V, Arc<dyn Easing>)>, duration_millis: i32, delay_millis: i32) -> Self {
        let timestamps =
            keyframe_timestamps(keyframes.keys().copied(), |t| keyframes.contains_key(&t), duration_millis);
        let infos = keyframes
            .into_iter()
            .map(|(t, (vector_value, easing))| {
                (t, VectorizedKeyframeSpecElementInfo { vector_value, easing, arc_mode: ArcMode::ArcLinear })
            })
            .collect();
        VectorizedKeyframesSpec::internal(
            timestamps,
            infos,
            duration_millis,
            delay_millis,
            Arc::new(LinearEasing),
            ArcMode::ArcLinear,
        )
    }

    pub(crate) fn internal(
        timestamps: Vec<i32>,
        keyframes: HashMap<i32, VectorizedKeyframeSpecElementInfo<V>>,
        duration_millis: i32,
        delay_millis: i32,
        default_easing: Arc<dyn Easing>,
        initial_arc_mode: ArcMode,
    ) -> Self {
        VectorizedKeyframesSpec {
            timestamps,
            keyframes,
            duration_millis,
            delay_millis,
            default_easing,
            initial_arc_mode,
            cache: RefCell::new(KeyframesCache {
                initialized: false,
                modes: Vec::new(),
                times: Vec::new(),
                last_initial_value: None,
                last_target_value: None,
                pos_array: Vec::new(),
                slope_array: Vec::new(),
                arc_spline: None,
            }),
        }
    }

    fn init(&self, initial_value: V, target_value: V) {
        let mut cache = self.cache.borrow_mut();
        let mut requires_arc_spline = cache.arc_spline.is_some();
        if !cache.initialized {
            cache.initialized = true;
            cache.times = self.timestamps.iter().map(|&t| t as f32 / SecondsToMillis as f32).collect();
            cache.modes = self
                .timestamps
                .iter()
                .map(|t| {
                    let mode = self.keyframes.get(t).map(|k| k.arc_mode).unwrap_or(self.initial_arc_mode);
                    if mode != ArcMode::ArcLinear {
                        requires_arc_spline = true;
                    }
                    mode.0
                })
                .collect();
        }
        if !requires_arc_spline {
            return;
        }
        if cache.arc_spline.is_none()
            || cache.last_initial_value != Some(initial_value)
            || cache.last_target_value != Some(target_value)
        {
            cache.last_initial_value = Some(initial_value);
            cache.last_target_value = Some(target_value);
            let dimension_count = V::SIZE % 2 + V::SIZE;
            cache.pos_array = vec![0.0; dimension_count];
            cache.slope_array = vec![0.0; dimension_count];
            let values: Vec<Vec<f32>> = self
                .timestamps
                .iter()
                .map(|&timestamp| {
                    let info = self.keyframes.get(&timestamp);
                    let vector = match info {
                        None if timestamp == 0 => initial_value,
                        None if timestamp == self.duration_millis => target_value,
                        Some(info) => info.vector_value,
                        None => unreachable!("a keyframe time without a keyframe"),
                    };
                    (0..dimension_count).map(|i| vector.get(i)).collect()
                })
                .collect();
            let spline = ArcSpline::new(&cache.modes, &cache.times, &values);
            cache.arc_spline = Some(spline);
        }
    }

    /// The value at that time, and whether it is the spec's own vector (Compose's `valueVector`, which the next
    /// call overwrites): what its velocity, by two of them, makes of it (see `get_velocity_from_nanos`).
    fn value_at(&self, play_time_nanos: i64, initial_value: V, target_value: V) -> (V, bool) {
        let play_time_millis = play_time_nanos / MillisToNanos;
        let clamped_play_time = clamp_play_time(self.duration_millis, self.delay_millis, play_time_millis) as i32;
        if let Some(keyframe) = self.keyframes.get(&clamped_play_time) {
            return (keyframe.vector_value, false);
        }
        if clamped_play_time >= self.duration_millis {
            return (target_value, false);
        } else if clamped_play_time <= 0 {
            return (initial_value, false);
        }
        self.init(initial_value, target_value);
        let mut value = initial_value.new_instance();
        let cache = self.cache.borrow();
        if let Some(spline) = &cache.arc_spline {
            let eased_time = self.get_eased_time(clamped_play_time);
            let mut pos = cache.pos_array.clone();
            spline.get_pos(eased_time, &mut pos);
            for (i, p) in pos.iter().enumerate() {
                value.set(i, *p);
            }
            return (value, true);
        }
        let index = self.find_entry_for_time_millis(clamped_play_time);
        let eased_time = self.get_eased_time_from_index(index, clamped_play_time, true);
        let timestamp_start = self.timestamps[index as usize];
        let start_value = self.keyframes.get(&timestamp_start).map(|k| k.vector_value).unwrap_or(initial_value);
        let timestamp_end = self.timestamps[index as usize + 1];
        let end_value = self.keyframes.get(&timestamp_end).map(|k| k.vector_value).unwrap_or(target_value);
        for i in 0..V::SIZE {
            value.set(i, lerp(start_value.get(i), end_value.get(i), eased_time));
        }
        (value, true)
    }

    fn get_eased_time(&self, time_millis: i32) -> f32 {
        let index = self.find_entry_for_time_millis(time_millis);
        self.get_eased_time_from_index(index, time_millis, false)
    }

    fn get_eased_time_from_index(&self, index: i32, time_millis: i32, as_fraction: bool) -> f32 {
        if index >= self.timestamps.len() as i32 - 1 {
            return time_millis as f32 / SecondsToMillis as f32;
        }
        let time_min = self.timestamps[index as usize];
        let time_max = self.timestamps[index as usize + 1];
        if time_millis == time_min {
            return time_min as f32 / SecondsToMillis as f32;
        }
        let time_range = time_max - time_min;
        let easing = self.keyframes.get(&time_min).map(|k| &k.easing).unwrap_or(&self.default_easing);
        let raw_fraction = (time_millis - time_min) as f32 / time_range as f32;
        let eased_fraction = easing.transform(raw_fraction);
        if as_fraction {
            return eased_fraction;
        }
        (time_range as f32 * eased_fraction + time_min as f32) / SecondsToMillis as f32
    }

    fn find_entry_for_time_millis(&self, time_millis: i32) -> i32 {
        let index = int_binary_search(&self.timestamps, time_millis);
        if index < -1 { -(index + 2) } else { index }
    }
}

impl<V: AnimationVector> VectorizedAnimationSpec<V> for VectorizedKeyframesSpec<V> {
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: V, target_value: V, _: V) -> V {
        self.value_at(play_time_nanos, initial_value, target_value).0
    }

    /// The arcs' slope; else, as Compose's, the difference of the values a millisecond before and now times 1000,
    /// the earlier minus the later — and zero when both are the spec's own vector (Compose's `valueVector`, the
    /// same object both times: the second call overwrote the first's), which is the case between keyframes. Its
    /// velocities are Compose's as they come out, not a slope.
    fn get_velocity_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: V,
        target_value: V,
        initial_velocity: V,
    ) -> V {
        let play_time_millis = play_time_nanos / MillisToNanos;
        let clamped_play_time = clamp_play_time(self.duration_millis, self.delay_millis, play_time_millis);
        if clamped_play_time < 0 {
            return initial_velocity;
        }
        self.init(initial_value, target_value);
        let mut velocity = initial_velocity.new_instance();
        {
            let cache = self.cache.borrow();
            if let Some(spline) = &cache.arc_spline {
                let eased_time = self.get_eased_time(clamped_play_time as i32);
                let mut slope = cache.slope_array.clone();
                spline.get_slope(eased_time, &mut slope);
                for (i, s) in slope.iter().enumerate() {
                    velocity.set(i, *s);
                }
                return velocity;
            }
        }
        let (start_num, start_shared) =
            self.value_at((clamped_play_time - 1) * MillisToNanos, initial_value, target_value);
        let (end_num, end_shared) = self.value_at(clamped_play_time * MillisToNanos, initial_value, target_value);
        // (both the spec's own vector: the same numbers twice, the later ones)
        let start_num = if start_shared && end_shared { end_num } else { start_num };
        for i in 0..V::SIZE {
            velocity.set(i, (start_num.get(i) - end_num.get(i)) * 1000.0);
        }
        velocity
    }

    fn get_duration_nanos(&self, _: V, _: V, _: V) -> i64 {
        duration_based_nanos(self.duration_millis, self.delay_millis)
    }
}
impl<V: AnimationVector> VectorizedFiniteAnimationSpec<V> for VectorizedKeyframesSpec<V> {}
impl<V: AnimationVector> VectorizedDurationBasedAnimationSpec<V> for VectorizedKeyframesSpec<V> {
    fn duration_millis(&self) -> i32 {
        self.duration_millis
    }
    fn delay_millis(&self) -> i32 {
        self.delay_millis
    }
}

// ---- Repeats ---------------------------------------------------------------------------------------------------

/// A duration-based spec repeated forever (Compose's VectorizedInfiniteRepeatableSpec): restarting or reversing
/// each time, after a start offset (a delay, or a fast-forward into it).
pub struct VectorizedInfiniteRepeatableSpec<V: AnimationVector> {
    animation: Box<dyn VectorizedDurationBasedAnimationSpec<V>>,
    repeat_mode: RepeatMode,
    pub(crate) duration_nanos: i64,
    initial_offset_nanos: i64,
}

impl<V: AnimationVector> VectorizedInfiniteRepeatableSpec<V> {
    pub fn new(
        animation: Box<dyn VectorizedDurationBasedAnimationSpec<V>>,
        repeat_mode: RepeatMode,
        initial_start_offset: StartOffset,
    ) -> Self {
        let duration_nanos = (animation.delay_millis() as i64 + animation.duration_millis() as i64) * MillisToNanos;
        VectorizedInfiniteRepeatableSpec {
            animation,
            repeat_mode,
            duration_nanos,
            initial_offset_nanos: initial_start_offset.value * MillisToNanos,
        }
    }

    fn repetition_play_time_nanos(&self, play_time_nanos: i64) -> i64 {
        if play_time_nanos + self.initial_offset_nanos <= 0 {
            0
        } else {
            let post_offset_play_time_nanos = play_time_nanos + self.initial_offset_nanos;
            let repeats_count = post_offset_play_time_nanos / self.duration_nanos;
            if self.repeat_mode == RepeatMode::Restart || repeats_count % 2 == 0 {
                post_offset_play_time_nanos - repeats_count * self.duration_nanos
            } else {
                (repeats_count + 1) * self.duration_nanos - post_offset_play_time_nanos
            }
        }
    }

    fn repetition_start_velocity(&self, play_time_nanos: i64, start: V, start_velocity: V, end: V) -> V {
        if play_time_nanos + self.initial_offset_nanos > self.duration_nanos {
            self.animation.get_velocity_from_nanos(
                self.duration_nanos - self.initial_offset_nanos,
                start,
                end,
                start_velocity,
            )
        } else {
            start_velocity
        }
    }
}

impl<V: AnimationVector> VectorizedAnimationSpec<V> for VectorizedInfiniteRepeatableSpec<V> {
    fn is_infinite(&self) -> bool {
        true
    }
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: V, target_value: V, initial_velocity: V) -> V {
        self.animation.get_value_from_nanos(
            self.repetition_play_time_nanos(play_time_nanos),
            initial_value,
            target_value,
            self.repetition_start_velocity(play_time_nanos, initial_value, initial_velocity, target_value),
        )
    }
    fn get_velocity_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: V,
        target_value: V,
        initial_velocity: V,
    ) -> V {
        self.animation.get_velocity_from_nanos(
            self.repetition_play_time_nanos(play_time_nanos),
            initial_value,
            target_value,
            self.repetition_start_velocity(play_time_nanos, initial_value, initial_velocity, target_value),
        )
    }
    fn get_duration_nanos(&self, _: V, _: V, _: V) -> i64 {
        i64::MAX
    }
}

/// A duration-based spec repeated `iterations` times (Compose's VectorizedRepeatableSpec): restarting or
/// reversing each time, after a start offset.
pub struct VectorizedRepeatableSpec<V: AnimationVector> {
    iterations: i32,
    animation: Box<dyn VectorizedDurationBasedAnimationSpec<V>>,
    repeat_mode: RepeatMode,
    pub(crate) duration_nanos: i64,
    initial_offset_nanos: i64,
}

impl<V: AnimationVector> VectorizedRepeatableSpec<V> {
    /// Fewer than one iteration panics, as Compose's throws.
    #[track_caller]
    pub fn new(
        iterations: i32,
        animation: Box<dyn VectorizedDurationBasedAnimationSpec<V>>,
        repeat_mode: RepeatMode,
        initial_start_offset: StartOffset,
    ) -> Self {
        if iterations < 1 {
            panic_kt("IllegalArgumentException", "Iterations count can't be less than 1");
        }
        let duration_nanos = (animation.delay_millis() as i64 + animation.duration_millis() as i64) * MillisToNanos;
        VectorizedRepeatableSpec {
            iterations,
            animation,
            repeat_mode,
            duration_nanos,
            initial_offset_nanos: initial_start_offset.value * MillisToNanos,
        }
    }

    fn repetition_play_time_nanos(&self, play_time_nanos: i64) -> i64 {
        if play_time_nanos + self.initial_offset_nanos <= 0 {
            0
        } else {
            let post_offset_play_time_nanos = play_time_nanos + self.initial_offset_nanos;
            let repeats_count = (post_offset_play_time_nanos / self.duration_nanos).min(self.iterations as i64 - 1);
            if self.repeat_mode == RepeatMode::Restart || repeats_count % 2 == 0 {
                post_offset_play_time_nanos - repeats_count * self.duration_nanos
            } else {
                (repeats_count + 1) * self.duration_nanos - post_offset_play_time_nanos
            }
        }
    }

    /// As Compose's: past the first repetition, its own velocity at the end of the first — called with its
    /// arguments in Compose's order (`start, startVelocity, end` where it takes the start, the target and the
    /// start velocity).
    fn repetition_start_velocity(&self, play_time_nanos: i64, start: V, start_velocity: V, end: V) -> V {
        if play_time_nanos + self.initial_offset_nanos > self.duration_nanos {
            self.get_velocity_from_nanos(self.duration_nanos - self.initial_offset_nanos, start, start_velocity, end)
        } else {
            start_velocity
        }
    }
}

impl<V: AnimationVector> VectorizedAnimationSpec<V> for VectorizedRepeatableSpec<V> {
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: V, target_value: V, initial_velocity: V) -> V {
        self.animation.get_value_from_nanos(
            self.repetition_play_time_nanos(play_time_nanos),
            initial_value,
            target_value,
            self.repetition_start_velocity(play_time_nanos, initial_value, initial_velocity, target_value),
        )
    }
    fn get_velocity_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: V,
        target_value: V,
        initial_velocity: V,
    ) -> V {
        self.animation.get_velocity_from_nanos(
            self.repetition_play_time_nanos(play_time_nanos),
            initial_value,
            target_value,
            self.repetition_start_velocity(play_time_nanos, initial_value, initial_velocity, target_value),
        )
    }
    fn get_duration_nanos(&self, _: V, _: V, _: V) -> i64 {
        self.iterations as i64 * self.duration_nanos - self.initial_offset_nanos
    }
}
impl<V: AnimationVector> VectorizedFiniteAnimationSpec<V> for VectorizedRepeatableSpec<V> {}

// ---- A start delay (Compose's internal StartDelayVectorizedAnimationSpec) -------------------------------------

/// A spec started `start_delay_nanos` late: the start value and velocity until then.
pub(crate) struct StartDelayVectorizedAnimationSpec<V: AnimationVector> {
    pub(crate) vectorized_animation_spec: Box<dyn VectorizedAnimationSpec<V>>,
    pub(crate) start_delay_nanos: i64,
}

impl<V: AnimationVector> VectorizedAnimationSpec<V> for StartDelayVectorizedAnimationSpec<V> {
    fn is_infinite(&self) -> bool {
        self.vectorized_animation_spec.is_infinite()
    }
    fn get_duration_nanos(&self, initial_value: V, target_value: V, initial_velocity: V) -> i64 {
        self.vectorized_animation_spec.get_duration_nanos(initial_value, target_value, initial_velocity)
            + self.start_delay_nanos
    }
    fn get_velocity_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: V,
        target_value: V,
        initial_velocity: V,
    ) -> V {
        if play_time_nanos < self.start_delay_nanos {
            initial_velocity
        } else {
            self.vectorized_animation_spec.get_velocity_from_nanos(
                play_time_nanos - self.start_delay_nanos,
                initial_value,
                target_value,
                initial_velocity,
            )
        }
    }
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: V, target_value: V, initial_velocity: V) -> V {
        if play_time_nanos < self.start_delay_nanos {
            initial_value
        } else {
            self.vectorized_animation_spec.get_value_from_nanos(
                play_time_nanos - self.start_delay_nanos,
                initial_value,
                target_value,
                initial_velocity,
            )
        }
    }
}
