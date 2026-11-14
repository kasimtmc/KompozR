//! How a morph maps one shape onto another (androidx.graphics.shapes' FloatMapping and FeatureMapping): progress
//! along one outline to progress along the other, from their corners matched nearest first.

use super::features::Feature;
use super::utils::*;

/// Whether `progress` is within `from..to`, which wraps round 1 where `to` is below `from`.
pub(crate) fn progress_in_range(progress: f32, progress_from: f32, progress_to: f32) -> bool {
    if progress_to >= progress_from {
        (progress_from..=progress_to).contains(&progress)
    } else {
        progress >= progress_from || progress <= progress_to
    }
}

/// `x` mapped from the `x_values` onto the `y_values`, linearly between them, wrapping round 1.
fn linear_map(x_values: &[f32], y_values: &[f32], x: f32) -> f32 {
    assert!((0.0..=1.0).contains(&x), "Invalid progress: {x}");
    let n = x_values.len();
    let start = (0..n)
        .find(|&i| progress_in_range(x, x_values[i], x_values[(i + 1) % n]))
        .expect("Collection contains no element matching the predicate.");
    let end = (start + 1) % n;
    let segment_size_x = positive_modulo(x_values[end] - x_values[start], 1.0);
    let segment_size_y = positive_modulo(y_values[end] - y_values[start], 1.0);
    let position_in_segment =
        if segment_size_x < 0.001 { 0.5 } else { positive_modulo(x - x_values[start], 1.0) / segment_size_x };
    positive_modulo(y_values[start] + segment_size_y * position_in_segment, 1.0)
}

/// Progress in 0..1 on one shape to progress on another and back (Compose's DoubleMapper), from some values
/// mapped, linearly between them and wrapping round.
pub(crate) struct DoubleMapper {
    source_values: Vec<f32>,
    target_values: Vec<f32>,
}

impl DoubleMapper {
    /// Panics, as Compose throws, unless both sides are in 0..1, apart, and go up but for (at most) one wrap.
    pub(crate) fn new(mappings: &[(f32, f32)]) -> DoubleMapper {
        let source_values: Vec<f32> = mappings.iter().map(|m| m.0).collect();
        let target_values: Vec<f32> = mappings.iter().map(|m| m.1).collect();
        validate_progress(&source_values);
        validate_progress(&target_values);
        DoubleMapper { source_values, target_values }
    }
    pub(crate) fn map(&self, x: f32) -> f32 {
        linear_map(&self.source_values, &self.target_values, x)
    }
    pub(crate) fn map_back(&self, x: f32) -> f32 {
        linear_map(&self.target_values, &self.source_values, x)
    }
}

fn joined(p: &[f32]) -> String {
    p.iter().map(|v| java_float_string(*v)).collect::<Vec<_>>().join(", ")
}

/// Progress values in 0..1, apart, going up but for (at most) one wrap round (the last to the first counted).
fn validate_progress(p: &[f32]) {
    let Some(&last) = p.last() else { return };
    let mut prev = last;
    let mut wraps = 0;
    for &curr in p {
        assert!((0.0..1.0).contains(&curr), "FloatMapping - Progress outside of range: {}", joined(p));
        assert!(
            progress_distance(curr, prev) > DISTANCE_EPSILON,
            "FloatMapping - Progress repeats a value: {}",
            joined(p)
        );
        if curr < prev {
            wraps += 1;
            assert!(wraps <= 1, "FloatMapping - Progress wraps more than once: {}", joined(p));
        }
        prev = curr;
    }
}

/// How far apart two progress values are, round the shorter way (0.99 and 0 are 0.01 apart).
pub(crate) fn progress_distance(p1: f32, p2: f32) -> f32 {
    let d = (p1 - p2).abs();
    d.min(1.0 - d)
}

/// A feature and the progress along its shape's outline at it (Compose's ProgressableFeature).
#[derive(Clone, Debug)]
pub(crate) struct ProgressableFeature {
    pub(crate) progress: f32,
    pub(crate) feature: Feature,
}

/// The mapping between two shapes' corners (Compose's featureMapper).
pub(crate) fn feature_mapper(features1: &[ProgressableFeature], features2: &[ProgressableFeature]) -> DoubleMapper {
    // only the corners
    let f1: Vec<&ProgressableFeature> = features1.iter().filter(|f| f.feature.is_corner()).collect();
    let f2: Vec<&ProgressableFeature> = features2.iter().filter(|f| f.feature.is_corner()).collect();
    DoubleMapper::new(&do_mapping(&f1, &f2))
}

/// Progress on the first shape to progress on the second, corners matched nearest first, none mapped twice, no
/// crossings added (Compose's doMapping); sorted by the first.
fn do_mapping(features1: &[&ProgressableFeature], features2: &[&ProgressableFeature]) -> Vec<(f32, f32)> {
    let mut distances = Vec::new();
    for (i1, f1) in features1.iter().enumerate() {
        for (i2, f2) in features2.iter().enumerate() {
            let d = feature_dist_squared(&f1.feature, &f2.feature);
            if d != f32::MAX {
                distances.push((d, i1, i2));
            }
        }
    }
    distances.sort_by(|a, b| a.0.total_cmp(&b.0)); // (stable)
    match distances.as_slice() {
        [] => return vec![(0.0, 0.0), (0.5, 0.5)],
        [(_, i1, i2)] => {
            let (p1, p2) = (features1[*i1].progress, features2[*i2].progress);
            return vec![(p1, p2), ((p1 + 0.5) % 1.0, (p2 + 0.5) % 1.0)];
        }
        _ => {}
    }
    let mut helper = MappingHelper { mapping: Vec::new(), used1: Vec::new(), used2: Vec::new() };
    for (_, i1, i2) in distances {
        helper.add_mapping(i1, features1[i1].progress, i2, features2[i2].progress);
    }
    helper.mapping
}

struct MappingHelper {
    // sorted by the first
    mapping: Vec<(f32, f32)>,
    used1: Vec<usize>,
    used2: Vec<usize>,
}

impl MappingHelper {
    fn add_mapping(&mut self, i1: usize, p1: f32, i2: usize, p2: f32) {
        // not the same feature twice
        if self.used1.contains(&i1) || self.used2.contains(&i2) {
            return;
        }
        let index = self.mapping.binary_search_by(|m| m.0.total_cmp(&p1));
        let insertion = match index {
            Ok(_) => panic!("There can't be two features with the same progress"),
            Err(i) => i,
        };
        let n = self.mapping.len();
        if n >= 1 {
            let (before1, before2) = self.mapping[(insertion + n - 1) % n];
            let (after1, after2) = self.mapping[insertion % n];
            // none too close to another (that would make the mapper unstable)
            if progress_distance(p1, before1) < DISTANCE_EPSILON
                || progress_distance(p1, after1) < DISTANCE_EPSILON
                || progress_distance(p2, before2) < DISTANCE_EPSILON
                || progress_distance(p2, after2) < DISTANCE_EPSILON
            {
                return;
            }
            // no crossing added
            if n > 1 && !progress_in_range(p2, before2, after2) {
                return;
            }
        }
        self.mapping.insert(insertion, (p1, p2));
        self.used1.push(i1);
        self.used2.push(i2);
    }
}

/// How far apart two features are (squared), between their representative points; unmappable (`f32::MAX`) for
/// corners of other convexities.
fn feature_dist_squared(f1: &Feature, f2: &Feature) -> f32 {
    if let (Some(c1), Some(c2)) = (f1.convex(), f2.convex())
        && c1 != c2
    {
        return f32::MAX;
    }
    (feature_representative_point(f1) - feature_representative_point(f2)).get_distance_squared()
}

/// Between a feature's ends.
fn feature_representative_point(feature: &Feature) -> Point {
    let (first, last) = (&feature.cubics()[0], &feature.cubics()[feature.cubics().len() - 1]);
    Point::new((first.anchor0_x() + last.anchor1_x()) / 2.0, (first.anchor0_y() + last.anchor1_y()) / 2.0)
}
