//! A path's segments in order (Compose's PathIterator and PathSegment).

use super::curves::Conic;
use super::path::{Path, PathCommand};
use crate::geometry::Offset;
use std::collections::VecDeque;

/// How a path's iterator gives its conics (Compose's PathIterator.ConicEvaluation): as they are, or as quadratics.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum ConicEvaluation {
    AsConic,
    #[default]
    AsQuadratics,
}

/// What a segment is (Compose's PathSegment.Type). Done is what Compose's iterator gives past its end; this one
/// gives None there instead.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PathSegmentType {
    Move,
    Line,
    Quadratic,
    Conic,
    Cubic,
    Close,
    #[default]
    Done,
}

/// One segment of a path as its iterator gives it (Compose's PathSegment): its points, the point it starts from
/// first (a move's: its point; a close's: none), and a conic's weight (0 for the others).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct PathSegment {
    pub r#type: PathSegmentType,
    pub points: Vec<Offset>,
    pub weight: f32,
}

impl PathSegment {
    pub fn new(r#type: PathSegmentType, points: Vec<Offset>, weight: f32) -> PathSegment {
        PathSegment { r#type, points, weight }
    }
}

/// A path's segments in order (Compose's PathIterator): conics as they are (AsConic), or each as quadratics within
/// `tolerance` of it (AsQuadratics, the default: as Skia approximates them).
///
/// ```
/// # use kompoz_graphics::*;
/// let mut path = Path::new();
/// path.add_oval(Rect::new(0.0, 0.0, 20.0, 10.0), PathDirection::CounterClockwise);
/// for segment in path.iterator() {
///     assert_ne!(segment.r#type, PathSegmentType::Conic);
/// }
/// ```
#[derive(Clone, Debug)]
pub struct PathIterator<'a> {
    path: &'a Path,
    conic_evaluation: ConicEvaluation,
    tolerance: f32,
    index: usize,
    current: Offset,
    pending: VecDeque<PathSegment>, // a conic's quadratics yet to give
}

impl<'a> PathIterator<'a> {
    pub fn new(path: &'a Path, conic_evaluation: ConicEvaluation, tolerance: f32) -> PathIterator<'a> {
        PathIterator { path, conic_evaluation, tolerance, index: 0, current: Offset::Zero, pending: VecDeque::new() }
    }

    pub fn path(&self) -> &'a Path {
        self.path
    }
    pub fn conic_evaluation(&self) -> ConicEvaluation {
        self.conic_evaluation
    }
    pub fn tolerance(&self) -> f32 {
        self.tolerance
    }

    /// How many segments it gives in all (conics as the quadratics they become, unless `include_converted_conics` is
    /// false: then a conic counts once).
    pub fn calculate_size(&self, include_converted_conics: bool) -> usize {
        let mut size = 0;
        let mut current = Offset::Zero;
        for c in self.path.commands() {
            match *c {
                PathCommand::Conic(c1, p, w)
                    if self.conic_evaluation == ConicEvaluation::AsQuadratics && include_converted_conics =>
                {
                    size += 1 << Conic::new([current, c1, p], w).quad_pow2(self.tolerance);
                }
                _ => size += 1,
            }
            if let Some(end) = c.end() {
                current = end;
            }
        }
        size
    }

    pub fn has_next(&self) -> bool {
        !self.pending.is_empty() || self.index < self.path.commands().len()
    }
}

impl Iterator for PathIterator<'_> {
    type Item = PathSegment;

    fn next(&mut self) -> Option<PathSegment> {
        use PathSegmentType as Type;
        if let Some(s) = self.pending.pop_front() {
            return Some(s);
        }
        let c = *self.path.commands().get(self.index)?;
        self.index += 1;
        let from = self.current;
        if let Some(end) = c.end() {
            self.current = end;
        }
        Some(match c {
            PathCommand::Move(p) => PathSegment::new(Type::Move, vec![p], 0.0),
            PathCommand::Line(p) => PathSegment::new(Type::Line, vec![from, p], 0.0),
            PathCommand::Quadratic(c1, p) => PathSegment::new(Type::Quadratic, vec![from, c1, p], 0.0),
            PathCommand::Cubic(c1, c2, p) => PathSegment::new(Type::Cubic, vec![from, c1, c2, p], 0.0),
            PathCommand::Close => PathSegment::new(Type::Close, Vec::new(), 0.0),
            PathCommand::Conic(c1, p, w) => {
                if self.conic_evaluation == ConicEvaluation::AsConic {
                    return Some(PathSegment::new(Type::Conic, vec![from, c1, p], w));
                }
                let conic = Conic::new([from, c1, p], w);
                let pts = conic.chop_into_quads_pow2(conic.quad_pow2(self.tolerance));
                for i in 0..(pts.len() - 1) / 2 {
                    self.pending.push_back(PathSegment::new(
                        Type::Quadratic,
                        vec![pts[2 * i], pts[2 * i + 1], pts[2 * i + 2]],
                        0.0,
                    ));
                }
                return self.next();
            }
        })
    }
}

impl Path {
    /// Its segments (Compose's Path.iterator()): conics as quadratics within a quarter of a unit of them.
    pub fn iterator(&self) -> PathIterator<'_> {
        PathIterator::new(self, ConicEvaluation::AsQuadratics, 0.25)
    }

    /// Its segments, conics as `conic_evaluation` says (as quadratics within `tolerance` of them).
    pub fn iterator_with(&self, conic_evaluation: ConicEvaluation, tolerance: f32) -> PathIterator<'_> {
        PathIterator::new(self, conic_evaluation, tolerance)
    }
}
