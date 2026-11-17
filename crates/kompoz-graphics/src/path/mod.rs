//! Paths (Compose's Path, an Android Path: Skia's SkPath): building them, their bounds, convexity and insides,
//! transforms, boolean operations, iterators, hit testers, lengths along them, path effects, strokes, outlines, and
//! the roots and bounds of cubic Béziers along an axis.

mod bezier;
mod curves;
mod effect;
mod hit_test;
mod interval_tree;
mod iterator;
mod measure;
mod ops;
mod outline;
#[allow(clippy::module_inception)]
mod path;
mod stroke;

pub use bezier::{
    compute_cubic_vertical_bounds, compute_horizontal_bounds, evaluate_cubic, evaluate_y, find_first_cubic_root,
    find_first_root,
};
pub use effect::{PathEffect, StampedPathEffectStyle, path_effected, res_scale_of};
pub use hit_test::PathHitTester;
pub use interval_tree::{Interval, IntervalTree};
pub use iterator::{ConicEvaluation, PathIterator, PathSegment, PathSegmentType};
pub use measure::PathMeasure;
pub use ops::PathOperation;
pub use outline::Outline;
pub use path::{Path, PathCommand, PathDirection, PathFillType};
pub use stroke::{Contour, DEFAULT_FLATTEN_TOLERANCE, PathStroke, StrokeCap, StrokeJoin, flatten_path, stroke_outline};
