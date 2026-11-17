//! Keyframes along a monotonic spline (Compose's internal VectorizedMonoSplineKeyframesSpec): what
//! `keyframesWithSpline` vectorizes to.

use crate::easing::{Easing, LinearEasing};
use crate::mono_spline::MonoSpline;
use crate::vector::AnimationVector;
use crate::vectorized::{
    VectorizedAnimationSpec, VectorizedDurationBasedAnimationSpec, VectorizedFiniteAnimationSpec, clamp_play_time,
    duration_based_nanos, int_binary_search,
};
use crate::{MillisToNanos, SecondsToMillis};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

struct Cache<V> {
    times: Option<Vec<f32>>,
    mono_spline: Option<MonoSpline>,
    values: Option<Vec<Vec<f32>>>,
    last_initial_value: Option<V>,
    last_target_value: Option<V>,
}

/// Keyframes joined by a monotonic cubic spline, eased between keyframes' times.
pub(crate) struct VectorizedMonoSplineKeyframesSpec<V> {
    timestamps: Vec<i32>,
    keyframes: HashMap<i32, (V, Arc<dyn Easing>)>,
    duration_millis: i32,
    delay_millis: i32,
    periodic_bias: f32,
    cache: RefCell<Cache<V>>,
}

impl<V: AnimationVector> VectorizedMonoSplineKeyframesSpec<V> {
    pub(crate) fn new(
        timestamps: Vec<i32>,
        keyframes: HashMap<i32, (V, Arc<dyn Easing>)>,
        duration_millis: i32,
        delay_millis: i32,
        periodic_bias: f32,
    ) -> Self {
        VectorizedMonoSplineKeyframesSpec {
            timestamps,
            keyframes,
            duration_millis,
            delay_millis,
            periodic_bias,
            cache: RefCell::new(Cache {
                times: None,
                mono_spline: None,
                values: None,
                last_initial_value: None,
                last_target_value: None,
            }),
        }
    }

    fn init(&self, initial_value: V, target_value: V) {
        let mut cache = self.cache.borrow_mut();
        if cache.times.is_none() {
            cache.times = Some(self.timestamps.iter().map(|&t| t as f32 / SecondsToMillis as f32).collect());
        }
        if cache.mono_spline.is_none()
            || cache.last_initial_value != Some(initial_value)
            || cache.last_target_value != Some(target_value)
        {
            let initial_changed = cache.last_initial_value != Some(initial_value);
            let target_changed = cache.last_target_value != Some(target_value);
            cache.last_initial_value = Some(initial_value);
            cache.last_target_value = Some(target_value);
            let dimension = V::SIZE;
            let floats = |v: V| (0..dimension).map(|i| v.get(i)).collect::<Vec<f32>>();
            match &mut cache.values {
                None => {
                    let values = self
                        .timestamps
                        .iter()
                        .map(|&timestamp| match self.keyframes.get(&timestamp) {
                            None if timestamp == 0 => floats(initial_value),
                            None if timestamp == self.duration_millis => floats(target_value),
                            Some((vector, _)) => floats(*vector),
                            None => unreachable!("a keyframe time without a keyframe"),
                        })
                        .collect();
                    cache.values = Some(values);
                }
                Some(values) => {
                    if initial_changed && !self.keyframes.contains_key(&0) {
                        let index = int_binary_search(&self.timestamps, 0);
                        values[index as usize] = floats(initial_value);
                    }
                    if target_changed && !self.keyframes.contains_key(&self.duration_millis) {
                        let index = int_binary_search(&self.timestamps, self.duration_millis);
                        values[index as usize] = floats(target_value);
                    }
                }
            }
            let spline =
                MonoSpline::new(cache.times.clone().unwrap(), cache.values.clone().unwrap(), self.periodic_bias);
            cache.mono_spline = Some(spline);
        }
    }

    fn get_easing(&self, index: usize) -> Arc<dyn Easing> {
        let timestamp = self.timestamps[index];
        self.keyframes.get(&timestamp).map(|(_, e)| e.clone()).unwrap_or_else(|| Arc::new(LinearEasing))
    }

    fn get_eased_time_from_index(&self, index: i32, time_millis: i32) -> f32 {
        if index >= self.timestamps.len() as i32 - 1 {
            return time_millis as f32 / SecondsToMillis as f32;
        }
        let time_min = self.timestamps[index as usize];
        let time_max = self.timestamps[index as usize + 1];
        if time_millis == time_min {
            return time_min as f32 / SecondsToMillis as f32;
        }
        let time_range = time_max - time_min;
        let easing = self.get_easing(index as usize);
        let raw_fraction = (time_millis - time_min) as f32 / time_range as f32;
        let eased_fraction = easing.transform(raw_fraction);
        (time_range as f32 * eased_fraction + time_min as f32) / SecondsToMillis as f32
    }

    fn find_entry_for_time_millis(&self, time_millis: i32) -> i32 {
        let index = int_binary_search(&self.timestamps, time_millis);
        if index < -1 { -(index + 2) } else { index }
    }
}

impl<V: AnimationVector> VectorizedAnimationSpec<V> for VectorizedMonoSplineKeyframesSpec<V> {
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: V, target_value: V, _: V) -> V {
        let play_time_millis = play_time_nanos / MillisToNanos;
        let clamped_play_time = clamp_play_time(self.duration_millis, self.delay_millis, play_time_millis) as i32;
        if let Some((vector, _)) = self.keyframes.get(&clamped_play_time) {
            return *vector;
        }
        if clamped_play_time >= self.duration_millis {
            return target_value;
        } else if clamped_play_time <= 0 {
            return initial_value;
        }
        self.init(initial_value, target_value);
        let index = self.find_entry_for_time_millis(clamped_play_time);
        let mut value = initial_value.new_instance();
        let time = self.get_eased_time_from_index(index, clamped_play_time);
        self.cache.borrow().mono_spline.as_ref().unwrap().get_pos(time, &mut value, index.max(0) as usize);
        value
    }

    fn get_velocity_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: V,
        target_value: V,
        initial_velocity: V,
    ) -> V {
        let play_time_millis = play_time_nanos / MillisToNanos;
        let clamped_play_time = clamp_play_time(self.duration_millis, self.delay_millis, play_time_millis) as i32;
        self.init(initial_value, target_value);
        let index = self.find_entry_for_time_millis(clamped_play_time);
        let mut velocity = initial_velocity.new_instance();
        let time = self.get_eased_time_from_index(index, clamped_play_time);
        self.cache.borrow().mono_spline.as_ref().unwrap().get_slope(time, &mut velocity, index.max(0) as usize);
        velocity
    }

    fn get_duration_nanos(&self, _: V, _: V, _: V) -> i64 {
        duration_based_nanos(self.duration_millis, self.delay_millis)
    }
}
impl<V: AnimationVector> VectorizedFiniteAnimationSpec<V> for VectorizedMonoSplineKeyframesSpec<V> {}
impl<V: AnimationVector> VectorizedDurationBasedAnimationSpec<V> for VectorizedMonoSplineKeyframesSpec<V> {
    fn duration_millis(&self) -> i32 {
        self.duration_millis
    }
    fn delay_millis(&self) -> i32 {
        self.delay_millis
    }
}
