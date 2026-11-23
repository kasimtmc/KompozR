//! Morphs between polygons (androidx.graphics.shapes' Morph and PolygonMeasure): both outlines measured by length,
//! their corners matched, their cubics cut to match one to one.

use super::cubic::{Cubic, MutableCubic};
use super::features::Feature;
use super::mapping::{DoubleMapper, ProgressableFeature, feature_mapper};
use super::rounded_polygon::RoundedPolygon;
use super::utils::*;

/// How long a cubic is, about (Compose's LengthMeasurer): the lengths of the chords of three pieces of it (at
/// least 98.5% of a circular arc's, the worst case of the shapes).
const SEGMENTS: usize = 3;

/// Where along `cubic` its length reaches `threshold` (its t), and that length; past its end, 1 and its length.
fn closest_progress_to(cubic: &Cubic, threshold: f32) -> (f32, f32) {
    let mut total = 0.0;
    let mut remainder = threshold;
    let mut prev = Point::new(cubic.anchor0_x(), cubic.anchor0_y());
    for i in 1..=SEGMENTS {
        let progress = i as f32 / SEGMENTS as f32;
        let point = cubic.point_on_curve(progress);
        let segment = (point - prev).get_distance();
        if segment >= remainder {
            return (progress - (1.0 - remainder / segment) / SEGMENTS as f32, threshold);
        }
        remainder -= segment;
        total += segment;
        prev = point;
    }
    (1.0, total)
}

fn measure_cubic(c: &Cubic) -> f32 {
    closest_progress_to(c, f32::INFINITY).1
}

fn find_cubic_cut_point(c: &Cubic, m: f32) -> f32 {
    closest_progress_to(c, m).0
}

/// A cubic, its length, and where along its outline (0..1) it starts and ends (Compose's MeasuredCubic).
#[derive(Clone, Debug)]
struct MeasuredCubic {
    cubic: Cubic,
    measured_size: f32,
    start_outline_progress: f32,
    end_outline_progress: f32,
}

impl MeasuredCubic {
    fn new(cubic: Cubic, start_outline_progress: f32, end_outline_progress: f32) -> MeasuredCubic {
        assert!(
            end_outline_progress >= start_outline_progress,
            "endOutlineProgress is expected to be equal or greater than startOutlineProgress"
        );
        MeasuredCubic { cubic, measured_size: measure_cubic(&cubic), start_outline_progress, end_outline_progress }
    }

    /// Cut in two where its outline is at `cut_outline_progress` (kept within its own).
    fn cut_at_progress(&self, cut_outline_progress: f32) -> (MeasuredCubic, MeasuredCubic) {
        let bounded = cut_outline_progress.clamp(self.start_outline_progress, self.end_outline_progress);
        let outline_progress_size = self.end_outline_progress - self.start_outline_progress;
        let progress_from_start = bounded - self.start_outline_progress;
        let relative_progress = progress_from_start / outline_progress_size;
        let t = find_cubic_cut_point(&self.cubic, relative_progress * self.measured_size);
        assert!((0.0..=1.0).contains(&t), "Cubic cut point is expected to be between 0 and 1");
        let (c1, c2) = self.cubic.split(t);
        (
            MeasuredCubic::new(c1, self.start_outline_progress, bounded),
            MeasuredCubic::new(c2, bounded, self.end_outline_progress),
        )
    }
}

/// A polygon's cubics measured along its outline, and its corners' progress (Compose's MeasuredPolygon).
struct MeasuredPolygon {
    cubics: Vec<MeasuredCubic>,
    features: Vec<ProgressableFeature>,
}

impl MeasuredPolygon {
    fn new(features: Vec<ProgressableFeature>, cubics: &[Cubic], outline_progress: &[f32]) -> MeasuredPolygon {
        assert!(
            outline_progress.len() == cubics.len() + 1,
            "Outline progress size is expected to be the cubics size + 1"
        );
        assert!(outline_progress[0] == 0.0, "First outline progress value is expected to be zero");
        assert!(
            outline_progress[outline_progress.len() - 1] == 1.0,
            "Last outline progress value is expected to be one"
        );
        let mut measured = Vec::new();
        let mut start = 0.0;
        for (index, cubic) in cubics.iter().enumerate() {
            // (empty ones left out)
            if outline_progress[index + 1] - outline_progress[index] > DISTANCE_EPSILON {
                measured.push(MeasuredCubic::new(*cubic, start, outline_progress[index + 1]));
                // the next starts exactly where this one ends
                start = outline_progress[index + 1];
            }
        }
        // (ones left out at the end: the last ends at 1)
        let last = measured.len() - 1;
        assert!(
            1.0 >= measured[last].start_outline_progress,
            "endOutlineProgress is expected to be equal or greater than startOutlineProgress"
        );
        measured[last].end_outline_progress = 1.0;
        MeasuredPolygon { cubics: measured, features }
    }

    /// The same polygon starting at `cutting_point` along its outline: the cubic there cut, the rest after it, the
    /// progress shifted to start at 0 (Compose's cutAndShift).
    fn cut_and_shift(self, cutting_point: f32) -> MeasuredPolygon {
        assert!((0.0..=1.0).contains(&cutting_point), "Cutting point is expected to be between 0 and 1");
        if cutting_point < DISTANCE_EPSILON {
            return self;
        }
        let n = self.cubics.len();
        let target_index = self
            .cubics
            .iter()
            .position(|c| (c.start_outline_progress..=c.end_outline_progress).contains(&cutting_point))
            .expect("Index -1 out of bounds");
        let (b1, b2) = self.cubics[target_index].cut_at_progress(cutting_point);
        let mut ret_cubics = vec![b2.cubic];
        for i in 1..n {
            ret_cubics.push(self.cubics[(i + target_index) % n].cubic);
        }
        ret_cubics.push(b1.cubic);
        let ret_outline_progress: Vec<f32> = (0..n + 2)
            .map(|index| match index {
                0 => 0.0,
                i if i == n + 1 => 1.0,
                i => {
                    let cubic_index = (target_index + i - 1) % n;
                    positive_modulo(self.cubics[cubic_index].end_outline_progress - cutting_point, 1.0)
                }
            })
            .collect();
        let new_features = self
            .features
            .iter()
            .map(|f| ProgressableFeature {
                progress: positive_modulo(f.progress - cutting_point, 1.0),
                feature: f.feature.clone(),
            })
            .collect();
        MeasuredPolygon::new(new_features, &ret_cubics, &ret_outline_progress)
    }

    /// Its cubics measured by length, each corner at the middle of its middle cubic (Compose's measurePolygon).
    fn measure_polygon(polygon: &RoundedPolygon) -> MeasuredPolygon {
        let mut cubics = Vec::new();
        let mut feature_to_cubic: Vec<(&Feature, usize)> = Vec::new();
        for feature in polygon.features() {
            for (cubic_index, cubic) in feature.cubics().iter().enumerate() {
                if feature.is_corner() && cubic_index == feature.cubics().len() / 2 {
                    feature_to_cubic.push((feature, cubics.len()));
                }
                cubics.push(*cubic);
            }
        }
        let mut measures = Vec::with_capacity(cubics.len() + 1);
        let mut measure = 0.0;
        measures.push(measure);
        for cubic in &cubics {
            let m = measure_cubic(cubic);
            assert!(m >= 0.0, "Measured cubic is expected to be greater or equal to zero");
            measure += m;
            measures.push(measure);
        }
        let total_measure = measure;
        let outline_progress: Vec<f32> = measures.iter().map(|m| m / total_measure).collect();
        let features = feature_to_cubic
            .iter()
            .map(|&(feature, ix)| ProgressableFeature {
                progress: positive_modulo((outline_progress[ix] + outline_progress[ix + 1]) / 2.0, 1.0),
                feature: feature.clone(),
            })
            .collect();
        MeasuredPolygon::new(features, &cubics, &outline_progress)
    }
}

/// A morph from one polygon to another (Compose's Morph): their cubics matched one to one (cut where one has more,
/// corners to corners of their kind, nearest first), each pair mixed as the progress says.
#[derive(Clone, Debug)]
pub struct Morph {
    start: RoundedPolygon,
    end: RoundedPolygon,
    morph_match: Vec<(Cubic, Cubic)>,
}

impl Morph {
    /// The morph from `start` to `end`.
    ///
    /// Kotlin: `Morph(start: RoundedPolygon, end: RoundedPolygon)`
    pub fn new(start: RoundedPolygon, end: RoundedPolygon) -> Morph {
        let morph_match = Morph::match_polygons(&start, &end);
        Morph { start, end, morph_match }
    }

    pub fn start(&self) -> &RoundedPolygon {
        &self.start
    }
    pub fn end(&self) -> &RoundedPolygon {
        &self.end
    }
    /// The matched cubics, the start's and the end's of each pair.
    pub fn morph_match(&self) -> &[(Cubic, Cubic)] {
        &self.morph_match
    }

    /// The bounds of both shapes, left, top, right, bottom (as [`RoundedPolygon::calculate_bounds`]).
    pub fn calculate_bounds(&self, approximate: bool) -> [f32; 4] {
        let [min_x, min_y, max_x, max_y] = self.start.calculate_bounds(approximate);
        let b = self.end.calculate_bounds(approximate);
        [min_x.min(b[0]), min_y.min(b[1]), max_x.max(b[2]), max_y.max(b[3])]
    }

    /// The square either shape fits in at any rotation (as [`RoundedPolygon::calculate_max_bounds`]).
    pub fn calculate_max_bounds(&self) -> [f32; 4] {
        let [min_x, min_y, max_x, max_y] = self.start.calculate_max_bounds();
        let b = self.end.calculate_max_bounds();
        [min_x.min(b[0]), min_y.min(b[1]), max_x.max(b[2]), max_y.max(b[3])]
    }

    /// The shape `progress` of the way (0 the start, 1 the end; a little past either for an overshoot), its last
    /// cubic ending exactly where the first starts.
    pub fn as_cubics(&self, progress: f32) -> Vec<Cubic> {
        let mut out = Vec::with_capacity(self.morph_match.len());
        let mut first: Option<Cubic> = None;
        let mut last: Option<Cubic> = None;
        for (a, b) in &self.morph_match {
            let cubic = Cubic::from_points(std::array::from_fn(|i| interpolate(a.points[i], b.points[i], progress)));
            if first.is_none() {
                first = Some(cubic);
            }
            if let Some(l) = last {
                out.push(l);
            }
            last = Some(cubic);
        }
        if let (Some(l), Some(f)) = (last, first) {
            out.push(Cubic(
                l.anchor0_x(),
                l.anchor0_y(),
                l.control0_x(),
                l.control0_y(),
                l.control1_x(),
                l.control1_y(),
                f.anchor0_x(),
                f.anchor0_y(),
            ));
        }
        out
    }

    /// Each of the shape's cubics `progress` of the way, in `mutable_cubic`, handed to `callback` in turn (no
    /// cubics made).
    pub fn for_each_cubic(
        &self,
        progress: f32,
        mutable_cubic: &mut MutableCubic,
        mut callback: impl FnMut(&MutableCubic),
    ) {
        for (a, b) in &self.morph_match {
            mutable_cubic.interpolate(a, b, progress);
            callback(mutable_cubic);
        }
    }

    /// The pairs: both measured, their corners mapped, the second cut and turned to start where the first's start
    /// maps to, then walked together, each cubic cut where the other's ends first (Compose's match).
    fn match_polygons(p1: &RoundedPolygon, p2: &RoundedPolygon) -> Vec<(Cubic, Cubic)> {
        let measured1 = MeasuredPolygon::measure_polygon(p1);
        let measured2 = MeasuredPolygon::measure_polygon(p2);
        let double_mapper: DoubleMapper = feature_mapper(&measured1.features, &measured2.features);
        let polygon2_cut_point = double_mapper.map(0.0);
        let bs1 = measured1;
        let bs2 = measured2.cut_and_shift(polygon2_cut_point);
        let mut ret = Vec::new();
        let (mut i1, mut i2) = (0, 0);
        let mut b1 = bs1.cubics.get(i1).cloned();
        i1 += 1;
        let mut b2 = bs2.cubics.get(i2).cloned();
        i2 += 1;
        while let (Some(c1), Some(c2)) = (&b1, &b2) {
            // ends, in the first shape's progress
            let b1a = if i1 == bs1.cubics.len() { 1.0 } else { c1.end_outline_progress };
            let b2a = if i2 == bs2.cubics.len() {
                1.0
            } else {
                double_mapper.map_back(positive_modulo(c2.end_outline_progress + polygon2_cut_point, 1.0))
            };
            let minb = b1a.min(b2a);
            let (seg1, new_b1) = if b1a > minb + ANGLE_EPSILON {
                let (a, b) = c1.cut_at_progress(minb);
                (a, Some(b))
            } else {
                let next = bs1.cubics.get(i1).cloned();
                i1 += 1;
                (c1.clone(), next)
            };
            let (seg2, new_b2) = if b2a > minb + ANGLE_EPSILON {
                let (a, b) = c2.cut_at_progress(positive_modulo(double_mapper.map(minb) - polygon2_cut_point, 1.0));
                (a, Some(b))
            } else {
                let next = bs2.cubics.get(i2).cloned();
                i2 += 1;
                (c2.clone(), next)
            };
            ret.push((seg1.cubic, seg2.cubic));
            b1 = new_b1;
            b2 = new_b2;
        }
        assert!(b1.is_none() && b2.is_none(), "Expected both Polygon's Cubic to be fully matched");
        ret
    }
}
