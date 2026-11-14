//! Tests points against a path again and again (Compose's PathHitTester).

use super::curves;
use super::iterator::{ConicEvaluation, PathSegmentType};
use super::path::{Path, PathFillType};
use crate::geometry::{Offset, Rect};

/// Tests points against a path again and again (Compose's PathHitTester), its conics as quadratics within a
/// tolerance.
#[derive(Clone, Debug, Default)]
pub struct PathHitTester {
    path: Path,
    bounds: Rect,
}

impl PathHitTester {
    /// The tolerance Compose's takes by default.
    pub const DefaultTolerance: f32 = 0.5;

    /// One with no path: nothing is inside.
    pub fn new() -> PathHitTester {
        PathHitTester::default()
    }

    /// One for `path`, its conics as quadratics within `tolerance` of them.
    pub fn with_path(path: &Path, tolerance: f32) -> PathHitTester {
        let mut tester = PathHitTester::default();
        tester.update_path(path, tolerance);
        tester
    }

    pub fn update_path(&mut self, path: &Path, tolerance: f32) {
        // Its conics as quadratics within the tolerance, as Compose's hit tester takes them.
        let mut quads = Path::new();
        quads.fill_type = path.fill_type;
        for s in path.iterator_with(ConicEvaluation::AsQuadratics, tolerance) {
            let p = &s.points;
            match s.r#type {
                PathSegmentType::Move => quads.move_to(p[0].x, p[0].y),
                PathSegmentType::Line => quads.line_to(p[1].x, p[1].y),
                PathSegmentType::Quadratic => quads.quadratic_to(p[1].x, p[1].y, p[2].x, p[2].y),
                PathSegmentType::Cubic => quads.cubic_to(p[1].x, p[1].y, p[2].x, p[2].y, p[3].x, p[3].y),
                PathSegmentType::Close => quads.close(),
                PathSegmentType::Conic | PathSegmentType::Done => {}
            }
        }
        self.path = quads;
        self.bounds = path.get_bounds();
    }

    /// As Compose's: inside its bounds (their far edges not), wound round by its segments as they are (an open
    /// contour not closed, a point on an edge not inside), by its fill type.
    pub fn contains(&self, position: Offset) -> bool {
        if self.path.is_empty() || !self.bounds.contains(position) {
            return false;
        }
        let w = curves::winding(self.path.commands(), position, false);
        if self.path.fill_type == PathFillType::EvenOdd { (w & 1) != 0 } else { w != 0 }
    }
}
