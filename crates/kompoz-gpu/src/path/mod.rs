//! A path's geometry for the GPU renderer, in the target's pixels (C++'s path_geometry): its curves cut into lines
//! fine enough that no point is more than [`TOLERANCE`] px off the curve (Wang's formula: kompoz-graphics'
//! [`flatten_path`]), a stroke turned into the outline it covers ([`stroke_outline`]), both put through the
//! renderer's transform ([`Projective::project`]), and the edges of the polygons that make, which the renderer
//! accumulates into a coverage mask on the GPU (shaders/path.slang) to be filled by the non-zero or even-odd rule.

#![deny(unsafe_code)]

mod edges;
mod hairline;

pub use edges::{Edge, append_edges};
pub use hairline::aliased_hairline;
pub use kompoz_graphics::{Contour, Projective, flatten_path, stroke_outline};

use kompoz_graphics::{DEFAULT_FLATTEN_TOLERANCE, Offset, Path, PathStroke, StrokeJoin};

/// How far a path's lines may stray from its curves where it is drawn, in px (C++'s kTolerance).
pub const TOLERANCE: f32 = DEFAULT_FLATTEN_TOLERANCE;

/// Where a path's points are in the target: (p + origin) * scale - offset, its curves cut finely enough to stay within
/// `tolerance` px there (finer where a transform enlarges them after). C++'s PathPlacement.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PathPlacement {
    pub origin: Offset,
    /// The target's px per unit of the path (a layer's resolution).
    pub scale: f32,
    /// The target's pixel origin.
    pub offset: Offset,
    pub tolerance: f32,
}

impl Default for PathPlacement {
    fn default() -> PathPlacement {
        PathPlacement { origin: Offset::Zero, scale: 1.0, offset: Offset::Zero, tolerance: TOLERANCE }
    }
}

impl PathPlacement {
    pub fn new(origin: Offset, scale: f32, offset: Offset) -> PathPlacement {
        PathPlacement { origin, scale, offset, tolerance: TOLERANCE }
    }

    /// Where the path's point (x, y) is in the target.
    pub fn at(&self, p: Offset) -> Offset {
        Offset::new(
            (p.x + self.origin.x) * self.scale - self.offset.x,
            (p.y + self.origin.y) * self.scale - self.offset.y,
        )
    }
}

/// The path's contours as polylines in the target's pixels (C++'s flattenPath).
pub fn flatten(path: &Path, place: &PathPlacement) -> Vec<Contour> {
    flatten_path(path, place.tolerance, |p| place.at(p))
}

/// A path's polygons in the target's pixels, through `transform` (what the target's pixels after its origin go
/// through), each closed, to be filled (C++'s Renderer::polygonsOf): its stroke's outline if `stroke` strokes (its
/// path effect already applied), cut finely enough for where the transform enlarges it most (its stroke included). A
/// hairline is a pixel wide where it is drawn (through the transform first, its open contours left open), its
/// corners round (Skia's hairlines join nothing: one line per segment), or, `aliased`, the pixels Skia's aliased
/// hairline takes.
pub fn polygons_of(
    path: &Path,
    place: &PathPlacement,
    stroke: &PathStroke,
    transform: &Projective,
    aliased: bool,
) -> Vec<Contour> {
    let mut place = *place;
    let transformed = !transform.is_identity();
    if transformed {
        let b = path.get_bounds();
        let reach = stroke.width * place.scale;
        let (from, to) = (place.at(b.top_left()), place.at(b.bottom_right()));
        let (x0, y0, x1, y1) = (from.x - reach, from.y - reach, to.x + reach, to.y + reach);
        let most = (0..4).fold(0.0f32, |most, c| {
            most.max(transform.max_scale(if c & 1 != 0 { x1 } else { x0 }, if c & 2 != 0 { y1 } else { y0 }))
        });
        place.tolerance = TOLERANCE / most.clamp(1.0, 64.0);
    }
    let mut contours = flatten(path, &place);
    if stroke.hairline {
        if transformed {
            // (Open ones open: C++ projected them as polygons, which closed an open curve with a line.)
            contours = transform.project_polylines(&contours);
        }
        if aliased {
            return aliased_hairline(&contours, stroke.cap);
        }
        return stroke_outline(&contours, 1.0, stroke.cap, StrokeJoin::Round, stroke.miter, place.tolerance);
    }
    if stroke.width > 0.0 {
        contours = stroke_outline(
            &contours,
            stroke.width * place.scale,
            stroke.cap,
            stroke.join,
            stroke.miter,
            place.tolerance,
        );
    }
    if transformed { transform.project(&contours) } else { contours }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kompoz_graphics::{Matrix, PathDirection, Rect, StrokeCap};

    fn at(p: Offset, x: f32, y: f32) -> bool {
        (p.x - x).abs() < 1e-3 && (p.y - y).abs() < 1e-3
    }

    /// How far the middles of a polyline's lines are from a circle of radius `r` around (r, r): the farthest.
    fn worst_off_circle(points: &[Offset], r: f32) -> f32 {
        points
            .windows(2)
            .map(|l| ((l[0].x + l[1].x) / 2.0 - r).hypot((l[0].y + l[1].y) / 2.0 - r))
            .fold(0.0, |worst, d| worst.max((d - r).abs()))
    }

    #[test]
    fn curves_are_flattened_closely() {
        // Curves cut into lines stay within TOLERANCE of the curve, in the target's pixels.
        let mut circle = Path::new();
        circle.add_oval(Rect::new(0.0, 0.0, 100.0, 100.0), PathDirection::CounterClockwise);
        for scale in [1.0, 2.625, 4.0] {
            let contours = flatten(&circle, &PathPlacement::new(Offset::Zero, scale, Offset::Zero));
            assert_eq!(contours.len(), 1);
            let worst = worst_off_circle(&contours[0].points, 50.0 * scale);
            assert!(worst <= TOLERANCE + 0.05, "{scale}: {worst}");
        }
        // Finer when asked (a transform enlarges it after): within that.
        let fine = flatten(&circle, &PathPlacement { tolerance: 0.02, ..PathPlacement::default() });
        let worst = worst_off_circle(&fine[0].points, 50.0);
        assert!(worst <= 0.02 + 0.02, "{worst}");
        // Placed: moved, scaled, the target's origin taken off.
        let place = PathPlacement::new(Offset::new(1.0, 2.0), 2.0, Offset::new(10.0, 20.0));
        assert!(at(place.at(Offset::new(4.0, 5.0)), 0.0, -6.0));
        let placed = flatten(&circle, &place);
        assert!(at(placed[0].points[0], 100.0 * 2.0 + 2.0 - 10.0, 50.0 * 2.0 + 4.0 - 20.0));
    }

    #[test]
    fn projections_are_exact() {
        // The renderer's projective transforms: composed, inverted, how much they stretch; polygons projected
        // through one are cut where they pass behind the eye.
        let m = Projective::translation(10.0, 0.0) * Projective::scaling(3.0, 0.5); // scaled, then moved
        assert!(at(m.apply(2.0, 4.0), 16.0, 2.0));
        assert!(at(m.inverse().apply(16.0, 2.0), 2.0, 4.0));
        assert!((m.max_scale(0.0, 0.0) - 3.0).abs() < 1e-4 && (m.min_scale(0.0, 0.0) - 0.5).abs() < 1e-4);
        assert!(m.axis_aligned() && !m.is_identity());
        let mut halved = Matrix::new();
        halved.set(3, 3, 2.0); // w = 2 everywhere: half size, and w made 1
        let h = Projective::of(&halved);
        assert!(h.m[8] == 1.0 && at(h.apply(8.0, 6.0), 4.0, 3.0));
        let mut seen = Matrix::new();
        seen.set(0, 3, 0.2); // w = 1 + x / 5: behind the eye left of x = -5
        let p = Projective::of(&seen);
        assert!((p.max_scale(5.0, 0.0) - 0.5).abs() < 1e-4); // twice as far at x = 5: half as big
        let square = Contour {
            points: vec![
                Offset::new(-10.0, -10.0),
                Offset::new(10.0, -10.0),
                Offset::new(10.0, 10.0),
                Offset::new(-10.0, 10.0),
            ],
            closed: true,
        };
        let cut = p.project(std::slice::from_ref(&square));
        assert_eq!(cut.len(), 1);
        let finite = cut[0].points.iter().all(|q| q.x.is_finite() && q.y.is_finite());
        let right = cut[0].points.iter().all(|q| q.x > -6e4); // (the cut is just in front of the eye: far, but finite)
        // The two corners behind: two points where its edges pass w = 0.
        assert!(finite && right && cut[0].points.len() == 4);
        assert!(cut[0].points.iter().any(|&q| at(q, 10.0 / 3.0, 10.0 / 3.0)));
        // Wholly behind the eye: nothing; a polygon of one point: nothing.
        assert!(
            Projective::scaling(1.0, 1.0).project(&[Contour { points: vec![Offset::Zero], closed: true }]).is_empty()
        );
        let behind = Projective { m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0] };
        assert!(behind.project(&[square]).is_empty());
        // No inverse: the identity.
        assert!(Projective::scaling(0.0, 1.0).inverse().is_identity());
    }

    #[test]
    fn polygons_are_cut_as_finely_as_they_are_seen() {
        let mut circle = Path::new();
        circle.add_oval(Rect::new(0.0, 0.0, 10.0, 10.0), PathDirection::CounterClockwise);
        let place = PathPlacement::default();
        let fill = PathStroke::default();
        let plain = polygons_of(&circle, &place, &fill, &Projective::Identity, false);
        assert_eq!(plain, flatten(&circle, &place));
        // Enlarged 8 times after: cut 8 times as finely (as finely as at 8 times the size), then enlarged.
        let enlarged = polygons_of(&circle, &place, &fill, &Projective::scaling(8.0, 8.0), false);
        let big = flatten(&circle, &PathPlacement { tolerance: TOLERANCE / 8.0, ..place });
        assert_eq!(enlarged[0].points.len(), big[0].points.len());
        assert!(at(enlarged[0].points[0], 80.0, 40.0));
        assert!(worst_off_circle(&enlarged[0].points, 40.0) <= TOLERANCE + 0.05);
        // A stroke: its outline, `scale` times as wide; a hairline a pixel wide where it is seen.
        let mut line = Path::new();
        line.move_to(0.0, 0.0);
        line.line_to(10.0, 0.0);
        let doubled = PathPlacement::new(Offset::Zero, 2.0, Offset::Zero);
        let stroked = polygons_of(&line, &doubled, &PathStroke::new(1.0), &Projective::Identity, false);
        let ys: Vec<f32> = stroked[0].points.iter().map(|p| p.y).collect();
        assert!(ys.iter().all(|y| y.abs() == 1.0));
        let hairline = PathStroke { hairline: true, ..PathStroke::default() };
        let seen = polygons_of(&line, &place, &hairline, &Projective::scaling(4.0, 4.0), false);
        assert!(seen.len() == 1 && seen[0].points.len() == 4); // (open: its butt ends, no line back)
        assert!(seen[0].points.iter().all(|p| p.y.abs() == 0.5 && (0.0..=40.0).contains(&p.x)));
        // Aliased: Skia's band of pixels.
        let aliased = polygons_of(&line, &place, &hairline, &Projective::Identity, true);
        assert_eq!(aliased, aliased_hairline(&flatten(&line, &place), StrokeCap::Butt));
    }
}
