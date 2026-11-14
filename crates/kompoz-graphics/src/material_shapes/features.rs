//! Features (androidx.graphics.shapes' Feature and FeatureDetector): cubics grouped into edges, convex corners and
//! concave corners, what a morph maps between shapes.

use super::cubic::{Cubic, PointTransformer};
use super::utils::*;
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FeatureKind {
    Edge,
    Corner { convex: bool },
}

/// Cubics grouped (Compose's Feature): an edge (straight, ignored by a morph's default mapping), a convex corner
/// (outward) or a concave one (inward). A [`Morph`](super::Morph) maps corners to corners of their kind. Equal by
/// its cubics and kind.
#[derive(Clone, PartialEq)]
pub struct Feature {
    cubics: Vec<Cubic>,
    kind: FeatureKind,
}

impl Feature {
    pub(crate) fn edge(cubics: Vec<Cubic>) -> Feature {
        Feature { cubics, kind: FeatureKind::Edge }
    }
    pub(crate) fn corner(cubics: Vec<Cubic>, convex: bool) -> Feature {
        Feature { cubics, kind: FeatureKind::Corner { convex } }
    }

    /// Cubics a morph's default mapping ignores (any indentation). Panics, as Compose throws, for none or ones not
    /// continuous.
    pub fn build_ignorable_feature(cubics: Vec<Cubic>) -> Feature {
        Feature::edge(cubics).validated()
    }
    /// An edge of one cubic.
    pub fn build_edge(cubic: Cubic) -> Feature {
        Feature::edge(vec![cubic])
    }
    /// A convex corner (outward). Panics as [`build_ignorable_feature`](Feature::build_ignorable_feature).
    pub fn build_convex_corner(cubics: Vec<Cubic>) -> Feature {
        Feature::corner(cubics, true).validated()
    }
    /// A concave corner (inward). Panics as [`build_ignorable_feature`](Feature::build_ignorable_feature).
    pub fn build_concave_corner(cubics: Vec<Cubic>) -> Feature {
        Feature::corner(cubics, false).validated()
    }

    fn validated(self) -> Feature {
        assert!(!self.cubics.is_empty(), "Features need at least one cubic.");
        let continuous = self.cubics.windows(2).all(|w| {
            (w[1].anchor0_x() - w[0].anchor1_x()).abs() <= DISTANCE_EPSILON
                && (w[1].anchor0_y() - w[0].anchor1_y()).abs() <= DISTANCE_EPSILON
        });
        assert!(
            continuous,
            "Feature must be continuous, with the anchor points of all cubics matching the anchor points of the \
             preceding and succeeding cubics"
        );
        self
    }

    pub fn cubics(&self) -> &[Cubic] {
        &self.cubics
    }

    /// Its points moved by `f`.
    pub fn transformed(&self, f: &dyn PointTransformer) -> Feature {
        Feature { cubics: self.cubics.iter().map(|c| c.transformed(f)).collect(), kind: self.kind }
    }

    /// Its cubics the other way, in reverse order (a corner's convexity flipped, as Compose's).
    pub fn reversed(&self) -> Feature {
        let cubics = self.cubics.iter().rev().map(Cubic::reverse).collect();
        let kind = match self.kind {
            FeatureKind::Edge => FeatureKind::Edge,
            FeatureKind::Corner { convex } => FeatureKind::Corner { convex: !convex },
        };
        Feature { cubics, kind }
    }

    /// Ignored by a morph's mapping (an edge).
    pub fn is_ignorable_feature(&self) -> bool {
        self.kind == FeatureKind::Edge
    }
    pub fn is_edge(&self) -> bool {
        self.kind == FeatureKind::Edge
    }
    pub fn is_convex_corner(&self) -> bool {
        self.kind == FeatureKind::Corner { convex: true }
    }
    pub fn is_concave_corner(&self) -> bool {
        self.kind == FeatureKind::Corner { convex: false }
    }
    pub(crate) fn is_corner(&self) -> bool {
        matches!(self.kind, FeatureKind::Corner { .. })
    }
    pub(crate) fn convex(&self) -> Option<bool> {
        match self.kind {
            FeatureKind::Corner { convex } => Some(convex),
            FeatureKind::Edge => None,
        }
    }
}

/// Compose's toString.
impl fmt::Debug for Feature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            FeatureKind::Edge => f.write_str("Edge"),
            FeatureKind::Corner { convex } => {
                f.write_str("Corner: cubics=")?;
                for (i, c) in self.cubics.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "[{c:?}]")?;
                }
                write!(f, " convex={convex}")
            }
        }
    }
}

/// Cubics as features (Compose's detectFeatures): each one an edge where it is straight, else a corner; straight
/// ones in a row (and empty ones) merged, but the first and the last kept as they are; an empty corner between two
/// that don't continue smoothly. The cubics should be continuous.
pub(crate) fn detect_features(cubics: &[Cubic]) -> Vec<Feature> {
    let Some(&first) = cubics.first() else { return Vec::new() };
    let mut out = Vec::new();
    let mut current = first;
    // one round, in which (current == last, next == first) is the last
    for i in 0..cubics.len() {
        let next = cubics[(i + 1) % cubics.len()];
        if i < cubics.len() - 1 && aligns_ish_with(&current, &next) {
            current = extend(&current, &next);
            continue;
        }
        out.push(as_feature(&current, &next));
        if !smoothes_into_ish(&current, &next) {
            out.push(as_feature(&Cubic::empty(current.anchor1_x(), current.anchor1_y()), &next));
        }
        current = next;
    }
    out
}

/// An edge where `c` is straight, else a corner as convex as its turn to `next`.
fn as_feature(c: &Cubic, next: &Cubic) -> Feature {
    if straight_ish(c) { Feature::edge(vec![*c]) } else { Feature::corner(vec![*c], c.convex_to(next)) }
}

/// Close to a straight line (one of no length isn't).
fn straight_ish(c: &Cubic) -> bool {
    !c.zero_length()
        && collinear_ish(
            c.anchor0_x(),
            c.anchor0_y(),
            c.anchor1_x(),
            c.anchor1_y(),
            c.control0_x(),
            c.control0_y(),
            RELAXED_DISTANCE_EPSILON,
        )
        && collinear_ish(
            c.anchor0_x(),
            c.anchor0_y(),
            c.anchor1_x(),
            c.anchor1_y(),
            c.control1_x(),
            c.control1_y(),
            RELAXED_DISTANCE_EPSILON,
        )
}

/// Whether `next` goes on smoothly from `c` (its first control `c`'s second reflected).
fn smoothes_into_ish(c: &Cubic, next: &Cubic) -> bool {
    collinear_ish(
        c.control1_x(),
        c.control1_y(),
        next.control0_x(),
        next.control0_y(),
        c.anchor1_x(),
        c.anchor1_y(),
        RELAXED_DISTANCE_EPSILON,
    )
}

/// Whether all of `c`'s points line up with `next`'s (or either has no length).
fn aligns_ish_with(c: &Cubic, next: &Cubic) -> bool {
    straight_ish(c) && straight_ish(next) && smoothes_into_ish(c, next) || c.zero_length() || next.zero_length()
}

/// `a` extended to `b`'s second anchor.
fn extend(a: &Cubic, b: &Cubic) -> Cubic {
    if a.zero_length() {
        Cubic(
            a.anchor0_x(),
            a.anchor0_y(),
            b.control0_x(),
            b.control0_y(),
            b.control1_x(),
            b.control1_y(),
            b.anchor1_x(),
            b.anchor1_y(),
        )
    } else {
        Cubic(
            a.anchor0_x(),
            a.anchor0_y(),
            a.control0_x(),
            a.control0_y(),
            a.control1_x(),
            a.control1_y(),
            b.anchor1_x(),
            b.anchor1_y(),
        )
    }
}
