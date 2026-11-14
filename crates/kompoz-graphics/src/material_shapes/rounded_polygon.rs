//! Polygons with rounded corners (androidx.graphics.shapes' RoundedPolygon, CornerRounding, the shapes of Shapes.kt
//! and PolygonValidation): vertices, each corner rounded by a circular arc and smoothed into its edges, as cubics
//! grouped into features.

use super::cubic::{Cubic, PointTransformer};
use super::features::Feature;
use super::utils::*;
use std::fmt;

/// How a polygon's corner is rounded (Compose's CornerRounding): by a circle of `radius` (0: sharp), its arc
/// extended into the edges by `smoothing` (0: the arc alone; 1: no arc, the flanking curves meeting in the middle).
/// The radius is in the polygon's own units (scaled with it).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct CornerRounding {
    pub radius: f32,
    pub smoothing: f32,
}

impl CornerRounding {
    /// A sharp corner.
    pub const Unrounded: CornerRounding = CornerRounding { radius: 0.0, smoothing: 0.0 };

    pub const fn new(radius: f32, smoothing: f32) -> CornerRounding {
        CornerRounding { radius, smoothing }
    }
}

/// A polygon with rounded corners (Compose's RoundedPolygon): its outline as [`Feature`]s (corners and the edges
/// between them) and their cubics, and a center (what it is made and turned round). Equal by its features.
#[derive(Clone)]
pub struct RoundedPolygon {
    features: Vec<Feature>,
    center: Point,
    cubics: Vec<Cubic>,
}

impl PartialEq for RoundedPolygon {
    fn eq(&self, o: &RoundedPolygon) -> bool {
        self.features == o.features
    }
}

/// Compose's toString.
impl fmt::Debug for RoundedPolygon {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let list = |v: Vec<String>| v.join(", ");
        write!(
            f,
            "[RoundedPolygon. Cubics = {} || Features = {} || Center = ({:?}, {:?})]",
            list(self.cubics.iter().map(|c| format!("{c:?}")).collect()),
            list(self.features.iter().map(|c| format!("{c:?}")).collect()),
            self.center.x,
            self.center.y
        )
    }
}

impl RoundedPolygon {
    /// Of these features and center: its cubics flattened (a first corner of three cubics split in its middle, the
    /// outline starting there; curves of no length left out, the last ending exactly where the first starts).
    /// Panics, as Compose throws, unless they are contiguous.
    pub(crate) fn of(features: Vec<Feature>, center: Point) -> RoundedPolygon {
        let mut cubics = Vec::new();
        let mut first_cubic: Option<Cubic> = None;
        let mut last_cubic: Option<Cubic> = None;
        let mut split_start: Option<Vec<Cubic>> = None;
        let mut split_end: Option<Vec<Cubic>> = None;
        if let Some(f) = features.first()
            && f.cubics().len() == 3
        {
            let (start, end) = f.cubics()[1].split(0.5);
            split_start = Some(vec![f.cubics()[0], start]);
            split_end = Some(vec![end, f.cubics()[2]]);
        }
        // one past the features, for the first one's split start
        for i in 0..=features.len() {
            let feature_cubics: &[Cubic] = if i == 0 && split_end.is_some() {
                split_end.as_deref().unwrap_or_default()
            } else if i == features.len() {
                match &split_start {
                    Some(s) => s,
                    None => break,
                }
            } else {
                features[i].cubics()
            };
            for cubic in feature_cubics {
                if !cubic.zero_length() {
                    if let Some(l) = last_cubic {
                        cubics.push(l);
                    }
                    last_cubic = Some(*cubic);
                    if first_cubic.is_none() {
                        first_cubic = Some(*cubic);
                    }
                } else if let Some(l) = &mut last_cubic {
                    // (several of no length in a row: the last one's end kept, not to break the outline)
                    l.points[6] = cubic.anchor1_x();
                    l.points[7] = cubic.anchor1_y();
                }
            }
        }
        match (last_cubic, first_cubic) {
            (Some(l), Some(f)) => cubics.push(Cubic(
                l.anchor0_x(),
                l.anchor0_y(),
                l.control0_x(),
                l.control0_y(),
                l.control1_x(),
                l.control1_y(),
                f.anchor0_x(),
                f.anchor0_y(),
            )),
            // (empty: a point at its center)
            _ => cubics.push(Cubic(center.x, center.y, center.x, center.y, center.x, center.y, center.x, center.y)),
        }
        let mut prev = cubics[cubics.len() - 1];
        for cubic in &cubics {
            if (cubic.anchor0_x() - prev.anchor1_x()).abs() > DISTANCE_EPSILON
                || (cubic.anchor0_y() - prev.anchor1_y()).abs() > DISTANCE_EPSILON
            {
                panic!(
                    "RoundedPolygon must be contiguous, with the anchor points of all curves matching the anchor \
                     points of the preceding and succeeding cubics"
                );
            }
            prev = *cubic;
        }
        RoundedPolygon { features, center, cubics }
    }

    /// A regular polygon (Compose's RoundedPolygon(numVertices, …)): `num_vertices` vertices `radius` from the
    /// center, the first at 3 o'clock, each corner rounded by `rounding` or its own of `per_vertex_rounding`
    /// (Compose's defaults: radius 1, center (0, 0), unrounded). Panics, as Compose throws, for fewer than 3 or
    /// roundings not one a vertex.
    pub fn new(
        num_vertices: i32,
        radius: f32,
        center_x: f32,
        center_y: f32,
        rounding: CornerRounding,
        per_vertex_rounding: Option<&[CornerRounding]>,
    ) -> RoundedPolygon {
        RoundedPolygon::from_vertices(
            &vertices_from_num_verts(num_vertices, radius, center_x, center_y),
            rounding,
            per_vertex_rounding,
            Some(Point::new(center_x, center_y)),
        )
    }

    /// The polygon through `vertices` (x, y pairs, in order round it), each corner rounded by `rounding` or its own
    /// of `per_vertex_rounding`, a side's two roundings (then smoothings) shrunk to fit it; its center given or (none)
    /// the vertices' average. Panics, as Compose throws, for fewer than 3 vertices, an odd number of values, or
    /// roundings not one a vertex.
    pub fn from_vertices(
        vertices: &[f32],
        rounding: CornerRounding,
        per_vertex_rounding: Option<&[CornerRounding]>,
        center: Option<Point>,
    ) -> RoundedPolygon {
        if vertices.len() < 6 {
            panic!("Polygons must have at least 3 vertices");
        }
        if vertices.len() % 2 == 1 {
            panic!("The vertices array should have even size");
        }
        if per_vertex_rounding.is_some_and(|p| p.len() * 2 != vertices.len()) {
            panic!(
                "perVertexRounding list should be either null or the same size as the number of vertices \
                 (vertices.size / 2)"
            );
        }
        let n = vertices.len() / 2;
        let vertex = |i: usize| Point::new(vertices[i * 2], vertices[i * 2 + 1]);
        let rounded_corners: Vec<RoundedCorner> = (0..n)
            .map(|i| {
                let vtx_rounding = per_vertex_rounding.map_or(rounding, |p| p[i]);
                RoundedCorner::new(vertex((i + n - 1) % n), vertex(i), vertex((i + 1) % n), Some(vtx_rounding))
            })
            .collect();
        // Each side's room for its two corners' cuts: how much of their round cuts, then of their smoothing.
        let cut_adjusts: Vec<(f32, f32)> = (0..n)
            .map(|ix| {
                let expected_round_cut =
                    rounded_corners[ix].expected_round_cut + rounded_corners[(ix + 1) % n].expected_round_cut;
                let expected_cut = rounded_corners[ix].expected_cut() + rounded_corners[(ix + 1) % n].expected_cut();
                let (vtx, next_vtx) = (vertex(ix), vertex((ix + 1) % n));
                let side_size = distance(vtx.x - next_vtx.x, vtx.y - next_vtx.y);
                if expected_round_cut > side_size {
                    (side_size / expected_round_cut, 0.0)
                } else if expected_cut > side_size {
                    (1.0, (side_size - expected_round_cut) / (expected_cut - expected_round_cut))
                } else {
                    (1.0, 1.0)
                }
            })
            .collect();
        let corners: Vec<Vec<Cubic>> = (0..n)
            .map(|i| {
                // the side from the previous corner, then the one to the next
                let allowed: Vec<f32> = (0..2)
                    .map(|delta| {
                        let (round_cut_ratio, cut_ratio) = cut_adjusts[(i + n - 1 + delta) % n];
                        let c = &rounded_corners[i];
                        c.expected_round_cut * round_cut_ratio + (c.expected_cut() - c.expected_round_cut) * cut_ratio
                    })
                    .collect();
                rounded_corners[i].get_cubics(allowed[0], allowed[1])
            })
            .collect();
        let mut features = Vec::with_capacity(2 * n);
        for i in 0..n {
            let convex = convex(vertex((i + n - 1) % n), vertex(i), vertex((i + 1) % n));
            features.push(Feature::corner(corners[i].clone(), convex));
            let (end, next) = (corners[i][corners[i].len() - 1], corners[(i + 1) % n][0]);
            features.push(Feature::edge(vec![Cubic::straight_line(
                end.anchor1_x(),
                end.anchor1_y(),
                next.anchor0_x(),
                next.anchor0_y(),
            )]));
        }
        let center = center.unwrap_or_else(|| calculate_center(vertices));
        RoundedPolygon::of(features, center)
    }

    /// The polygon of these features (Compose's RoundedPolygon(features, …)), which say how a morph maps it; its
    /// center given or (none) its cubics' first anchors' average. Panics, as Compose throws, for fewer than 2.
    pub fn from_features(features: Vec<Feature>, center: Option<Point>) -> RoundedPolygon {
        assert!(features.len() >= 2, "Polygons must have at least 2 features");
        let center = center.unwrap_or_else(|| {
            let vertices: Vec<f32> =
                features.iter().flat_map(|f| f.cubics()).flat_map(|c| [c.anchor0_x(), c.anchor0_y()]).collect();
            calculate_center(&vertices)
        });
        RoundedPolygon::of(features, center)
    }

    /// Close to a circle (Compose's RoundedPolygon.circle): a regular polygon of `num_vertices`, each corner rounded
    /// by `radius` (Compose's defaults: 8 vertices, radius 1, center (0, 0)). Panics for fewer than 3 vertices.
    pub fn circle(num_vertices: i32, radius: f32, center_x: f32, center_y: f32) -> RoundedPolygon {
        if num_vertices < 3 {
            panic!("Circle must have at least three vertices");
        }
        // half the angle between two vertices
        let theta = FLOAT_PI / num_vertices as f32;
        let polygon_radius = radius / cos(theta);
        RoundedPolygon::new(num_vertices, polygon_radius, center_x, center_y, CornerRounding::new(radius, 0.0), None)
    }

    /// A rectangle `width` by `height` round its center (Compose's RoundedPolygon.rectangle; defaults: 2 by 2,
    /// unrounded, at (0, 0)), its corners from the bottom right, clockwise.
    pub fn rectangle(
        width: f32,
        height: f32,
        rounding: CornerRounding,
        per_vertex_rounding: Option<&[CornerRounding]>,
        center_x: f32,
        center_y: f32,
    ) -> RoundedPolygon {
        let left = center_x - width / 2.0;
        let top = center_y - height / 2.0;
        let right = center_x + width / 2.0;
        let bottom = center_y + height / 2.0;
        RoundedPolygon::from_vertices(
            &[right, bottom, left, bottom, left, top, right, top],
            rounding,
            per_vertex_rounding,
            Some(Point::new(center_x, center_y)),
        )
    }

    /// A star (Compose's RoundedPolygon.star): `num_vertices_per_radius` tips on `radius` and as many notches on
    /// `inner_radius`, alternating; its tips rounded by `rounding`, its notches by `inner_rounding` (none: the same),
    /// or each its own (Compose's defaults: radius 1, inner radius 0.5, unrounded, at (0, 0)). Panics, as Compose
    /// throws, for radii not above 0 or an inner radius not below the radius.
    #[allow(clippy::too_many_arguments)]
    pub fn star(
        num_vertices_per_radius: i32,
        radius: f32,
        inner_radius: f32,
        rounding: CornerRounding,
        inner_rounding: Option<CornerRounding>,
        per_vertex_rounding: Option<&[CornerRounding]>,
        center_x: f32,
        center_y: f32,
    ) -> RoundedPolygon {
        if radius <= 0.0 || inner_radius <= 0.0 {
            panic!("Star radii must both be greater than 0");
        }
        if inner_radius >= radius {
            panic!("innerRadius must be less than radius");
        }
        let alternating = per_vertex_rounding
            .is_none()
            .then_some(inner_rounding)
            .flatten()
            .map(|inner| (0..num_vertices_per_radius).flat_map(|_| [rounding, inner]).collect::<Vec<_>>());
        RoundedPolygon::from_vertices(
            &star_vertices_from_num_verts(num_vertices_per_radius, radius, inner_radius, center_x, center_y),
            rounding,
            per_vertex_rounding.or(alternating.as_deref()),
            Some(Point::new(center_x, center_y)),
        )
    }

    /// A pill (Compose's RoundedPolygon.pill): a `width` by `height` rect whose shorter sides are semicircles
    /// (defaults: 2 by 1, no smoothing, at (0, 0)). Panics, as Compose throws, for a side not above 0.
    pub fn pill(width: f32, height: f32, smoothing: f32, center_x: f32, center_y: f32) -> RoundedPolygon {
        if !(width > 0.0 && height > 0.0) {
            panic!("Pill shapes must have positive width and height");
        }
        let (w_half, h_half) = (width / 2.0, height / 2.0);
        RoundedPolygon::from_vertices(
            &[
                w_half + center_x,
                h_half + center_y,
                -w_half + center_x,
                h_half + center_y,
                -w_half + center_x,
                -h_half + center_y,
                w_half + center_x,
                -h_half + center_y,
            ],
            CornerRounding::new(w_half.min(h_half), smoothing),
            None,
            Some(Point::new(center_x, center_y)),
        )
    }

    /// A pill with tips and notches along its outline, as a star round a circle (Compose's
    /// RoundedPolygon.pillStar): `num_vertices_per_radius` of each, the notches `inner_radius_ratio` of the way
    /// out; on the round ends the inner ones (`vertex_spacing` 0) or the outer ones (1) spaced as along the edges;
    /// starting `start_location` of the way round (Compose's defaults: 2 by 1, 8, 0.5, unrounded, 0.5, 0, at (0,
    /// 0)). Panics, as Compose throws, for a side not above 0 or a ratio not within (0, 1].
    #[allow(clippy::too_many_arguments)]
    pub fn pill_star(
        width: f32,
        height: f32,
        num_vertices_per_radius: i32,
        inner_radius_ratio: f32,
        rounding: CornerRounding,
        inner_rounding: Option<CornerRounding>,
        per_vertex_rounding: Option<&[CornerRounding]>,
        vertex_spacing: f32,
        start_location: f32,
        center_x: f32,
        center_y: f32,
    ) -> RoundedPolygon {
        if !(width > 0.0 && height > 0.0) {
            panic!("Pill shapes must have positive width and height");
        }
        if !(inner_radius_ratio > 0.0 && inner_radius_ratio <= 1.0) {
            panic!("innerRadius must be between 0 and 1");
        }
        let alternating = per_vertex_rounding
            .is_none()
            .then_some(inner_rounding)
            .flatten()
            .map(|inner| (0..num_vertices_per_radius).flat_map(|_| [rounding, inner]).collect::<Vec<_>>());
        RoundedPolygon::from_vertices(
            &pill_star_vertices_from_num_verts(
                num_vertices_per_radius,
                width,
                height,
                inner_radius_ratio,
                vertex_spacing,
                start_location,
                center_x,
                center_y,
            ),
            rounding,
            per_vertex_rounding.or(alternating.as_deref()),
            Some(Point::new(center_x, center_y)),
        )
    }

    pub fn features(&self) -> &[Feature] {
        &self.features
    }
    /// Its outline, its features' cubics flattened.
    pub fn cubics(&self) -> &[Cubic] {
        &self.cubics
    }
    pub fn center_x(&self) -> f32 {
        self.center.x
    }
    pub fn center_y(&self) -> f32 {
        self.center.y
    }

    /// Its points (and center) moved by `f`.
    pub fn transformed(&self, f: impl PointTransformer) -> RoundedPolygon {
        let (cx, cy) = f.transform(self.center.x, self.center.y);
        let features = self.features.iter().map(|feature| feature.transformed(&f)).collect();
        RoundedPolygon::of(features, Point::new(cx, cy))
    }

    /// Moved and scaled into the unit square (0, 0 to 1, 1), its proportions kept, centered the way it has room.
    pub fn normalized(&self) -> RoundedPolygon {
        let bounds = self.calculate_bounds(true);
        let width = bounds[2] - bounds[0];
        let height = bounds[3] - bounds[1];
        let side = width.max(height);
        let offset_x = (side - width) / 2.0 - bounds[0];
        let offset_y = (side - height) / 2.0 - bounds[1];
        self.transformed(move |x: f32, y: f32| ((x + offset_x) / side, (y + offset_y) / side))
    }

    /// The square, left, top, right, bottom, round its center that holds it at any rotation: as far out as the
    /// furthest of its cubics' starts and middles.
    pub fn calculate_max_bounds(&self) -> [f32; 4] {
        let (cx, cy) = (self.center.x, self.center.y);
        let mut max_dist_squared = 0.0f32;
        for cubic in &self.cubics {
            let anchor_distance = distance_squared(cubic.anchor0_x() - cx, cubic.anchor0_y() - cy);
            let middle = cubic.point_on_curve(0.5);
            let middle_distance = distance_squared(middle.x - cx, middle.y - cy);
            max_dist_squared = max_dist_squared.max(anchor_distance.max(middle_distance));
        }
        let d = max_dist_squared.sqrt();
        [cx - d, cy - d, cx + d, cy + d]
    }

    /// Its bounds, left, top, right, bottom: its cubics' own, or (`approximate`, Compose's default) their anchors'
    /// and controls'. (As Compose's, the right and bottom start from Kotlin's Float.MIN_VALUE, the smallest positive
    /// float.)
    pub fn calculate_bounds(&self, approximate: bool) -> [f32; 4] {
        let (mut min_x, mut min_y) = (f32::MAX, f32::MAX);
        let (mut max_x, mut max_y) = (KOTLIN_FLOAT_MIN_VALUE, KOTLIN_FLOAT_MIN_VALUE);
        for cubic in &self.cubics {
            let b = cubic.calculate_bounds(approximate);
            min_x = min_x.min(b[0]);
            min_y = min_y.min(b[1]);
            max_x = max_x.max(b[2]);
            max_y = max_y.max(b[3]);
        }
        [min_x, min_y, max_x, max_y]
    }

    /// Turned clockwise (in drawing coordinates) where its cubics go round the other way (Compose's
    /// PolygonValidator.fix: its features reversed, the first kept first).
    pub(crate) fn fixed_orientation(self) -> RoundedPolygon {
        let signed_area: f32 =
            self.cubics.iter().map(|c| (c.anchor1_x() - c.anchor0_x()) * (c.anchor1_y() + c.anchor0_y())).sum::<f32>();
        if signed_area < 0.0 {
            return self;
        }
        let mut reversed = vec![self.features[0].reversed()];
        reversed.extend(self.features[1..].iter().rev().map(Feature::reversed));
        RoundedPolygon::of(reversed, self.center)
    }
}

/// The vertices' average.
fn calculate_center(vertices: &[f32]) -> Point {
    let (mut cumulative_x, mut cumulative_y) = (0.0f32, 0.0f32);
    for &[x, y] in vertices.as_chunks::<2>().0 {
        cumulative_x += x;
        cumulative_y += y;
    }
    let n = (vertices.len() / 2) as f32;
    Point::new(cumulative_x / n, cumulative_y / n)
}

/// A corner of a polygon and how it is rounded (Compose's RoundedCorner): first how much it would cut off each
/// side, then (given how much it may) its cubics.
struct RoundedCorner {
    p0: Point,
    p1: Point,
    p2: Point,
    d1: Point,
    d2: Point,
    corner_radius: f32,
    smoothing: f32,
    expected_round_cut: f32,
}

impl RoundedCorner {
    fn new(p0: Point, p1: Point, p2: Point, rounding: Option<CornerRounding>) -> RoundedCorner {
        let v01 = p0 - p1;
        let v21 = p2 - p1;
        let d01 = v01.get_distance();
        let d21 = v21.get_distance();
        if d01 > 0.0 && d21 > 0.0 {
            let d1 = v01 / d01;
            let d2 = v21 / d21;
            let corner_radius = rounding.map_or(0.0, |r| r.radius);
            let smoothing = rounding.map_or(0.0, |r| r.smoothing);
            // the angle at p1
            let cos_angle = dot(d1, d2);
            let sin_angle = (1.0 - square(cos_angle)).sqrt();
            // how far along a side to cut for the radius (tan(A/2) = sinA / (1 + cosA) = radius / cut)
            let expected_round_cut =
                if sin_angle as f64 > 1e-3 { corner_radius * (cos_angle + 1.0) / sin_angle } else { 0.0 };
            RoundedCorner { p0, p1, p2, d1, d2, corner_radius, smoothing, expected_round_cut }
        } else {
            // (a side of no length: nothing to round)
            let zero = Point::new(0.0, 0.0);
            RoundedCorner {
                p0,
                p1,
                p2,
                d1: zero,
                d2: zero,
                corner_radius: 0.0,
                smoothing: 0.0,
                expected_round_cut: 0.0,
            }
        }
    }

    /// The cut smoothing asks for (0: the round cut; 1: twice it).
    fn expected_cut(&self) -> f32 {
        (1.0 + self.smoothing) * self.expected_round_cut
    }

    /// Its cubics, cutting at most `allowed_cut0` off the side before and `allowed_cut1` off the side after: a point
    /// where it isn't rounded, else a flanking curve, a circular arc and another.
    fn get_cubics(&self, allowed_cut0: f32, allowed_cut1: f32) -> Vec<Cubic> {
        // the smaller cut sets the radius; more room on a side goes to its smoothing
        let allowed_cut = allowed_cut0.min(allowed_cut1);
        if self.expected_round_cut < DISTANCE_EPSILON
            || allowed_cut < DISTANCE_EPSILON
            || self.corner_radius < DISTANCE_EPSILON
        {
            return vec![Cubic::straight_line(self.p1.x, self.p1.y, self.p1.x, self.p1.y)];
        }
        let actual_round_cut = allowed_cut.min(self.expected_round_cut);
        let actual_smoothing0 = self.calculate_actual_smoothing_value(allowed_cut0);
        let actual_smoothing1 = self.calculate_actual_smoothing_value(allowed_cut1);
        let actual_r = self.corner_radius * actual_round_cut / self.expected_round_cut;
        let center_distance = (square(actual_r) + square(actual_round_cut)).sqrt();
        let center = self.p1 + get_direction((self.d1 + self.d2) / 2.0) * center_distance;
        let circle_intersection0 = self.p1 + self.d1 * actual_round_cut;
        let circle_intersection2 = self.p1 + self.d2 * actual_round_cut;
        let flanking0 = compute_flanking_curve(
            actual_round_cut,
            actual_smoothing0,
            self.p1,
            self.p0,
            circle_intersection0,
            circle_intersection2,
            center,
            actual_r,
        );
        let flanking2 = compute_flanking_curve(
            actual_round_cut,
            actual_smoothing1,
            self.p1,
            self.p2,
            circle_intersection2,
            circle_intersection0,
            center,
            actual_r,
        )
        .reverse();
        vec![
            flanking0,
            Cubic::circular_arc(
                center.x,
                center.y,
                flanking0.anchor1_x(),
                flanking0.anchor1_y(),
                flanking2.anchor0_x(),
                flanking2.anchor0_y(),
            ),
            flanking2,
        ]
    }

    /// The smoothing there is room for, cutting `allowed_cut`: all of it, part of it, or none.
    fn calculate_actual_smoothing_value(&self, allowed_cut: f32) -> f32 {
        if allowed_cut > self.expected_cut() {
            self.smoothing
        } else if allowed_cut > self.expected_round_cut {
            self.smoothing * (allowed_cut - self.expected_round_cut) / (self.expected_cut() - self.expected_round_cut)
        } else {
            0.0
        }
    }
}

/// The curve from the side (from `side_start` to `corner`), cut `actual_round_cut` and more for smoothing, into the
/// circle round `circle_center` of `actual_r`, ending on it smoothly (Compose's computeFlankingCurve).
#[allow(clippy::too_many_arguments)]
fn compute_flanking_curve(
    actual_round_cut: f32,
    actual_smoothing_values: f32,
    corner: Point,
    side_start: Point,
    circle_segment_intersection: Point,
    other_circle_segment_intersection: Point,
    circle_center: Point,
    actual_r: f32,
) -> Cubic {
    let side_direction = get_direction(side_start - corner);
    let curve_start = corner + side_direction * actual_round_cut * (1.0 + actual_smoothing_values);
    // a part of the circle's section, the less the smoother
    let p = interpolate_point(
        circle_segment_intersection,
        (circle_segment_intersection + other_circle_segment_intersection) / 2.0,
        actual_smoothing_values,
    );
    // it ends on the circle
    let curve_end = circle_center + direction_vector(p.x - circle_center.x, p.y - circle_center.y) * actual_r;
    // its control there where the circle's tangent meets the side
    let circle_tangent = rotate90(curve_end - circle_center);
    let anchor_end =
        line_intersection(side_start, side_direction, curve_end, circle_tangent).unwrap_or(circle_segment_intersection);
    // its other control two thirds of the way there (as design tools have it)
    let anchor_start = (curve_start + anchor_end * 2.0) / 3.0;
    Cubic::of_points(curve_start, anchor_start, anchor_end, curve_end)
}

/// Where the line through `p0` along `d0` meets the one through `p1` along `d1`, if they meet.
fn line_intersection(p0: Point, d0: Point, p1: Point, d1: Point) -> Option<Point> {
    let rotated_d1 = rotate90(d1);
    let den = dot(d0, rotated_d1);
    if den.abs() < DISTANCE_EPSILON {
        return None;
    }
    let num = dot(p1 - p0, rotated_d1);
    // (relatively too: abs(den / num) < epsilon, without dividing)
    if den.abs() < DISTANCE_EPSILON * num.abs() {
        return None;
    }
    let k = num / den;
    Some(p0 + d0 * k)
}

fn vertices_from_num_verts(num_vertices: i32, radius: f32, center_x: f32, center_y: f32) -> Vec<f32> {
    let mut result = Vec::with_capacity(num_vertices.max(0) as usize * 2);
    for i in 0..num_vertices {
        let vertex = radial_to_cartesian(radius, FLOAT_PI / num_vertices as f32 * 2.0 * i as f32, Point::Zero)
            + Point::new(center_x, center_y);
        result.push(vertex.x);
        result.push(vertex.y);
    }
    result
}

fn star_vertices_from_num_verts(
    num_vertices_per_radius: i32,
    radius: f32,
    inner_radius: f32,
    center_x: f32,
    center_y: f32,
) -> Vec<f32> {
    let n = num_vertices_per_radius as f32;
    let mut result = Vec::with_capacity(num_vertices_per_radius.max(0) as usize * 4);
    for i in 0..num_vertices_per_radius {
        let vertex = radial_to_cartesian(radius, FLOAT_PI / n * 2.0 * i as f32, Point::Zero);
        result.push(vertex.x + center_x);
        result.push(vertex.y + center_y);
        let vertex = radial_to_cartesian(inner_radius, FLOAT_PI / n * (2 * i + 1) as f32, Point::Zero);
        result.push(vertex.x + center_x);
        result.push(vertex.y + center_y);
    }
    result
}

/// A pill star's vertices: placed by how far along the pill's outline each is, in whichever of its sections (the
/// side edges, the four quarter circles, the top and bottom edges) that falls.
#[allow(clippy::too_many_arguments)]
fn pill_star_vertices_from_num_verts(
    num_vertices_per_radius: i32,
    width: f32,
    height: f32,
    inner_radius: f32,
    vertex_spacing: f32,
    start_location: f32,
    center_x: f32,
    center_y: f32,
) -> Vec<f32> {
    let endcap_radius = width.min(height);
    let v_seg_len = (height - width).max(0.0);
    let h_seg_len = (width - height).max(0.0);
    let v_seg_half = v_seg_len / 2.0;
    let h_seg_half = h_seg_len / 2.0;
    let circle_perimeter = TWO_PI * endcap_radius * interpolate(inner_radius, 1.0, vertex_spacing);
    let perimeter = 2.0 * h_seg_len + 2.0 * v_seg_len + circle_perimeter;
    // where each section starts along the outline
    let mut sections = [0.0f32; 11];
    sections[1] = v_seg_len / 2.0;
    sections[2] = sections[1] + circle_perimeter / 4.0;
    sections[3] = sections[2] + h_seg_len;
    sections[4] = sections[3] + circle_perimeter / 4.0;
    sections[5] = sections[4] + v_seg_len;
    sections[6] = sections[5] + circle_perimeter / 4.0;
    sections[7] = sections[6] + h_seg_len;
    sections[8] = sections[7] + circle_perimeter / 4.0;
    sections[9] = sections[8] + v_seg_len / 2.0;
    sections[10] = perimeter;
    let t_per_vertex = perimeter / (2 * num_vertices_per_radius) as f32;
    let mut inner = false;
    let mut curr_sec_index = 0usize;
    let mut sec_start = 0.0f32;
    let mut sec_end = sections[1];
    let mut t = start_location * perimeter;
    let mut result = Vec::with_capacity(num_vertices_per_radius.max(0) as usize * 4);
    let rect_br = Point::new(h_seg_half, v_seg_half);
    let rect_bl = Point::new(-h_seg_half, v_seg_half);
    let rect_tl = Point::new(-h_seg_half, -v_seg_half);
    let rect_tr = Point::new(h_seg_half, -v_seg_half);
    for _ in 0..num_vertices_per_radius * 2 {
        // (past 0 again where it started after 0)
        let bounded_t = t % perimeter;
        if bounded_t < sec_start {
            curr_sec_index = 0;
        }
        while bounded_t >= sections[(curr_sec_index + 1) % sections.len()] {
            curr_sec_index = (curr_sec_index + 1) % sections.len();
            sec_start = sections[curr_sec_index];
            sec_end = sections[(curr_sec_index + 1) % sections.len()];
        }
        let t_in_section = bounded_t - sec_start;
        let t_proportion = t_in_section / (sec_end - sec_start);
        let curr_radius = if inner { endcap_radius * inner_radius } else { endcap_radius };
        let vertex = match curr_sec_index {
            0 => Point::new(curr_radius, t_proportion * v_seg_half),
            1 => radial_to_cartesian(curr_radius, t_proportion * FLOAT_PI / 2.0, Point::Zero) + rect_br,
            2 => Point::new(h_seg_half - t_proportion * h_seg_len, curr_radius),
            3 => {
                radial_to_cartesian(curr_radius, FLOAT_PI / 2.0 + (t_proportion * FLOAT_PI / 2.0), Point::Zero)
                    + rect_bl
            }
            4 => Point::new(-curr_radius, v_seg_half - t_proportion * v_seg_len),
            5 => radial_to_cartesian(curr_radius, FLOAT_PI + (t_proportion * FLOAT_PI / 2.0), Point::Zero) + rect_tl,
            6 => Point::new(-h_seg_half + t_proportion * h_seg_len, -curr_radius),
            7 => {
                radial_to_cartesian(curr_radius, FLOAT_PI * 1.5 + (t_proportion * FLOAT_PI / 2.0), Point::Zero)
                    + rect_tr
            }
            _ => Point::new(curr_radius, -v_seg_half + t_proportion * v_seg_half),
        };
        result.push(vertex.x + center_x);
        result.push(vertex.y + center_y);
        t += t_per_vertex;
        inner = !inner;
    }
    result
}
