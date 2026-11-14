//! Boolean operations on paths (Compose's Path.op and PathOperation).

use super::curves;
use super::path::{Path, PathCommand, PathFillType, append_piece, piece_start};
use crate::geometry::Offset;
use std::ops::{Add, BitAnd, BitOr, BitXor, Sub};

/// How Path::op combines two shapes (Compose's PathOperation): what is in the first but not the second, in both,
/// in either, in exactly one, or in the second but not the first.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PathOperation {
    Difference,
    Intersect,
    Union,
    Xor,
    ReverseDifference,
}

fn combined(op: PathOperation, a: bool, b: bool) -> bool {
    match op {
        PathOperation::Difference => a && !b,
        PathOperation::Intersect => a && b,
        PathOperation::Union => a || b,
        PathOperation::Xor => a != b,
        PathOperation::ReverseDifference => b && !a,
    }
}

fn distance(a: Offset, b: Offset) -> f32 {
    (b.x - a.x).hypot(b.y - a.y)
}
fn mix(a: Offset, b: Offset, t: f32) -> Offset {
    Offset::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}
fn cross(u: Offset, v: Offset) -> f32 {
    u.x * v.y - u.y * v.x
}

/// A segment of a path (a contour's closing line too) and the point it starts from.
struct Segment {
    c: PathCommand,
    from: Offset,
}

impl Segment {
    fn end(&self) -> Offset {
        self.c.end().unwrap_or(Offset::Zero)
    }
}

fn segments_of(path: &Path) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut current = Offset::Zero;
    let mut start = Offset::Zero;
    let mut open = false;
    let close_contour = |out: &mut Vec<Segment>, current: &mut Offset, start: Offset, open: &mut bool| {
        if *open && (current.x != start.x || current.y != start.y) {
            out.push(Segment { c: PathCommand::Line(start), from: *current });
        }
        *open = false;
        *current = start;
    };
    for c in path.commands() {
        match *c {
            PathCommand::Move(p) => {
                close_contour(&mut out, &mut current, start, &mut open);
                current = p;
                start = p;
            }
            PathCommand::Close => close_contour(&mut out, &mut current, start, &mut open),
            _ => {
                out.push(Segment { c: *c, from: current });
                current = c.end().unwrap_or(current);
                open = true;
            }
        }
    }
    close_contour(&mut out, &mut current, start, &mut open);
    out
}

/// How many chords keep within `tolerance` of a segment (its second differences bound how far a chord strays:
/// d / 8 / n^2 for a quadratic, 3 d / 4 / n^2 for a cubic).
fn chords_for(s: &Segment, tolerance: f32) -> i32 {
    let dd = |a: Offset, b: Offset, e: Offset| (a.x - 2.0 * b.x + e.x).hypot(a.y - 2.0 * b.y + e.y);
    let n = match s.c {
        PathCommand::Quadratic(c1, p) | PathCommand::Conic(c1, p, _) => (dd(s.from, c1, p) / (8.0 * tolerance)).sqrt(),
        PathCommand::Cubic(c1, c2, p) => (0.75 * dd(s.from, c1, c2).max(dd(c1, c2, p)) / tolerance).sqrt(),
        _ => 1.0,
    };
    (n.ceil() as i32).clamp(1, 512)
}

/// A straight piece of a segment: between `t0` and `t1` along segment `seg`.
#[derive(Clone, Copy)]
struct Side {
    from: Offset,
    to: Offset,
    seg: usize,
    t0: f32,
    t1: f32,
}

impl Path {
    /// This path becomes `a` combined with `b` (each by its fill type): closed contours with what is inside on their
    /// left, made of the pieces of a's and b's segments that bound the result (a curve's piece the same curve);
    /// false if nothing is left.
    ///
    /// Done on the segments cut into fine chords (within a hundredth of a unit of the curves) that remember where
    /// along their segment they lie: every chord split where another crosses it, the pieces kept whose two sides
    /// differ in the result (by where the segments themselves pass, exactly), turned to have the inside on their
    /// left, chained into contours; then each run of pieces of one segment becomes that segment's piece (a curve's
    /// the curve between those two places).
    pub fn op(&mut self, a: &Path, b: &Path, operation: PathOperation) -> bool {
        let mut segs = segments_of(a);
        segs.extend(segments_of(b));
        const TOLERANCE: f32 = 0.01;
        let mut sides: Vec<Side> = Vec::new();
        for (i, s) in segs.iter().enumerate() {
            let n = chords_for(s, TOLERANCE);
            let mut prev = s.from;
            for k in 1..=n {
                let t = k as f32 / n as f32;
                let q = if k == n { s.end() } else { piece_start(&s.c, s.from, t) };
                sides.push(Side { from: prev, to: q, seg: i, t0: (k - 1) as f32 / n as f32, t1: t });
                prev = q;
            }
        }
        // Where along each side (0..1) it is split.
        let mut cuts: Vec<Vec<f32>> = vec![Vec::new(); sides.len()];
        const EPS: f32 = 1e-6;
        for i in 0..sides.len() {
            let s = sides[i];
            let d1 = Offset::new(s.to.x - s.from.x, s.to.y - s.from.y);
            for j in i + 1..sides.len() {
                let t = sides[j];
                if s.from.x.max(s.to.x) < t.from.x.min(t.to.x) - EPS
                    || t.from.x.max(t.to.x) < s.from.x.min(s.to.x) - EPS
                    || s.from.y.max(s.to.y) < t.from.y.min(t.to.y) - EPS
                    || t.from.y.max(t.to.y) < s.from.y.min(s.to.y) - EPS
                {
                    continue;
                }
                let d2 = Offset::new(t.to.x - t.from.x, t.to.y - t.from.y);
                let w = Offset::new(t.from.x - s.from.x, t.from.y - s.from.y);
                let denom = cross(d1, d2);
                let l1 = d1.x * d1.x + d1.y * d1.y;
                let l2 = d2.x * d2.x + d2.y * d2.y;
                if denom.abs() > 1e-9 * (l1 * l2).sqrt() {
                    // Crossing lines.
                    let u = cross(w, d2) / denom;
                    let v = cross(w, d1) / denom;
                    if u > EPS && u < 1.0 - EPS && v > -EPS && v < 1.0 + EPS {
                        cuts[i].push(u);
                    }
                    if v > EPS && v < 1.0 - EPS && u > -EPS && u < 1.0 + EPS {
                        cuts[j].push(v);
                    }
                } else if cross(w, d1).abs() <= 1e-6 * l1.sqrt() + EPS {
                    // On one line: each splits at the other's ends.
                    let along =
                        |p: Offset, from: Offset, d: Offset, l: f32| ((p.x - from.x) * d.x + (p.y - from.y) * d.y) / l;
                    for p in [t.from, t.to] {
                        let u = if l1 > 0.0 { along(p, s.from, d1, l1) } else { 0.0 };
                        if u > EPS && u < 1.0 - EPS {
                            cuts[i].push(u);
                        }
                    }
                    for p in [s.from, s.to] {
                        let v = if l2 > 0.0 { along(p, t.from, d2, l2) } else { 0.0 };
                        if v > EPS && v < 1.0 - EPS {
                            cuts[j].push(v);
                        }
                    }
                }
            }
        }
        // The pieces whose two sides differ in the result, turned to have the inside on their left.
        let in_a = |q: Offset| curves::contains(a.commands(), a.fill_type, q);
        let in_b = |q: Offset| curves::contains(b.commands(), b.fill_type, q);
        let mut kept: Vec<Side> = Vec::new();
        for (s, c) in sides.iter().zip(cuts.iter_mut()) {
            c.sort_by(f32::total_cmp);
            c.insert(0, 0.0);
            c.push(1.0);
            for k in 0..c.len() - 1 {
                if c[k + 1] - c[k] < EPS {
                    continue;
                }
                let p = mix(s.from, s.to, c[k]);
                let q = mix(s.from, s.to, c[k + 1]);
                let len = distance(p, q);
                if len < 1e-5 {
                    continue;
                }
                let ta = s.t0 + (s.t1 - s.t0) * c[k];
                let tb = s.t0 + (s.t1 - s.t0) * c[k + 1];
                // Either side of the segment itself, where it passes between the two ends.
                let seg = &segs[s.seg];
                let mid = piece_start(&seg.c, seg.from, (ta + tb) / 2.0);
                let normal = Offset::new(-(q.y - p.y) / len, (q.x - p.x) / len);
                let e = 1e-3f32.min(len * 0.25);
                let left = Offset::new(mid.x + normal.x * e, mid.y + normal.y * e);
                let right = Offset::new(mid.x - normal.x * e, mid.y - normal.y * e);
                let in_left = combined(operation, in_a(left), in_b(left));
                let in_right = combined(operation, in_a(right), in_b(right));
                if in_left == in_right {
                    continue;
                }
                kept.push(if in_left {
                    Side { from: p, to: q, seg: s.seg, t0: ta, t1: tb }
                } else {
                    Side { from: q, to: p, seg: s.seg, t0: tb, t1: ta }
                });
            }
        }
        // The same piece twice (sides the two paths share): one is enough.
        let same = |u: Offset, v: Offset| (u.x - v.x).abs() < 1e-4 && (u.y - v.y).abs() < 1e-4;
        let mut pieces: Vec<Side> = Vec::new();
        for s in kept {
            if !pieces.iter().any(|t| same(s.from, t.from) && same(s.to, t.to)) {
                pieces.push(s);
            }
        }
        // Chained end to start into closed contours.
        self.reset();
        self.fill_type = PathFillType::NonZero;
        let mut used = vec![false; pieces.len()];
        for first in 0..pieces.len() {
            if used[first] {
                continue;
            }
            let mut chain = vec![first];
            used[first] = true;
            let mut end = pieces[first].to;
            let mut previous = pieces[first].from;
            while !same(end, pieces[first].from) {
                // Where pieces meet at a point (shapes touching at a corner), the one turning most towards the inside
                // keeps to the same shape: each one gets its own contour.
                let mut next = None;
                let mut best_turn = -10.0f32;
                let incoming = Offset::new(end.x - previous.x, end.y - previous.y);
                for (k, piece) in pieces.iter().enumerate() {
                    if used[k] || !same(piece.from, end) {
                        continue;
                    }
                    let out = Offset::new(piece.to.x - piece.from.x, piece.to.y - piece.from.y);
                    let mut turn = cross(incoming, out).atan2(incoming.x * out.x + incoming.y * out.y);
                    // (A hair under π: nearly straight back counts as straight back.)
                    #[allow(clippy::approx_constant)]
                    let back = 3.14159f32;
                    if turn > back {
                        turn = -turn; // straight back: the last choice
                    }
                    if turn > best_turn {
                        best_turn = turn;
                        next = Some(k);
                    }
                }
                let Some(next) = next else { break }; // (shouldn't happen: an open end)
                previous = pieces[next].from;
                chain.push(next);
                used[next] = true;
                end = pieces[next].to;
            }
            if chain.len() < 2 {
                continue;
            }
            // Started where a run of one segment's pieces starts (not in the middle of one, which would split it in
            // two).
            let continues = |before: &Side, after: &Side| {
                after.seg == before.seg
                    && (after.t0 - before.t1).abs() <= 1e-5
                    && (after.t1 > after.t0) == (before.t1 > before.t0)
            };
            let len = chain.len();
            if let Some(k) = (0..len).find(|&k| !continues(&pieces[chain[(k + len - 1) % len]], &pieces[chain[k]])) {
                chain.rotate_left(k);
            }
            // Runs of pieces of one segment, one after another along it: that segment's piece.
            self.move_to(pieces[chain[0]].from.x, pieces[chain[0]].from.y);
            let mut k = 0;
            while k < len {
                let s = pieces[chain[k]];
                let t0 = s.t0;
                let mut t1 = s.t1;
                let mut r = k + 1;
                while r < len && continues(&pieces[chain[r - 1]], &pieces[chain[r]]) {
                    t1 = pieces[chain[r]].t1;
                    r += 1;
                }
                let seg = &segs[s.seg];
                let to = pieces[chain[r - 1]].to;
                if let PathCommand::Line(_) = seg.c {
                    self.line_to(to.x, to.y);
                } else {
                    // The curve between the two places (turned round if run backwards), ending where the run does.
                    let mut piece = Path::new();
                    piece.move_to(0.0, 0.0);
                    append_piece(&mut piece, &seg.c, seg.from, t0.min(t1), t0.max(t1));
                    let mut c = *piece.commands().last().expect("a piece after its move");
                    if t1 < t0
                        && let PathCommand::Cubic(c1, c2, p) = c
                    {
                        c = PathCommand::Cubic(c2, c1, p);
                    }
                    match c {
                        PathCommand::Quadratic(c1, _) => self.quadratic_to(c1.x, c1.y, to.x, to.y),
                        PathCommand::Conic(c1, _, w) => self.conic_to(c1.x, c1.y, to.x, to.y, w),
                        PathCommand::Cubic(c1, c2, _) => self.cubic_to(c1.x, c1.y, c2.x, c2.y, to.x, to.y),
                        _ => self.line_to(to.x, to.y),
                    }
                }
                k = r;
            }
            self.close();
        }
        !self.is_empty()
    }

    /// `a` combined with `b` (Compose's Path.combine).
    pub fn combine(operation: PathOperation, a: &Path, b: &Path) -> Path {
        let mut out = Path::new();
        out.op(a, b, operation);
        out
    }
}

macro_rules! path_operator {
    ($trait:ident, $method:ident, $operation:expr) => {
        impl $trait for &Path {
            type Output = Path;
            fn $method(self, o: &Path) -> Path {
                Path::combine($operation, self, o)
            }
        }
        impl $trait for Path {
            type Output = Path;
            fn $method(self, o: Path) -> Path {
                Path::combine($operation, &self, &o)
            }
        }
    };
}

// Compose's operators: plus and or a union, minus a difference, and an intersection, xor what is in exactly one.
path_operator!(Add, add, PathOperation::Union);
path_operator!(Sub, sub, PathOperation::Difference);
path_operator!(BitOr, bitor, PathOperation::Union);
path_operator!(BitAnd, bitand, PathOperation::Intersect);
path_operator!(BitXor, bitxor, PathOperation::Xor);
