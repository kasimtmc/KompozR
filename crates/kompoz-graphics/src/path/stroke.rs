//! Strokes: how a path is stroked (its width, caps, joins, miter limit and path effect), its curves cut into lines,
//! and a stroke turned into the outline it covers (joins, caps), to be filled by the non-zero rule like any fill.

use super::effect::{PathEffect, path_effected};
use super::path::{Path, PathCommand};
use crate::geometry::Offset;

/// How an open contour's ends are drawn (Compose's StrokeCap): cut square at the end, round, or square past it by
/// half the width.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum StrokeCap {
    #[default]
    Butt,
    Round,
    Square,
}

/// How a stroke's segments are joined (Compose's StrokeJoin): their outer edges met (a miter, up to the miter
/// limit), a round corner, or cut straight across.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum StrokeJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// How a path is drawn: filled, or stroked `width` wide (half each side) with its caps, joins and miter limit, or as
/// a hairline (one pixel wide, whatever the transform: the width unused); through its path effect first.
#[derive(Clone, PartialEq, Debug)]
pub struct PathStroke {
    /// 0: fill (unless a hairline).
    pub width: f32,
    pub cap: StrokeCap,
    pub join: StrokeJoin,
    /// How long a miter may be, over half the width, before it is beveled.
    pub miter: f32,
    pub hairline: bool,
    pub path_effect: Option<PathEffect>,
}

impl Default for PathStroke {
    fn default() -> Self {
        PathStroke {
            width: 0.0,
            cap: StrokeCap::Butt,
            join: StrokeJoin::Miter,
            miter: PathStroke::DefaultMiter,
            hairline: false,
            path_effect: None,
        }
    }
}

impl PathStroke {
    /// Compose's Stroke.DefaultMiter.
    pub const DefaultMiter: f32 = 4.0;

    /// A stroke `width` wide, its other properties the defaults.
    pub fn new(width: f32) -> PathStroke {
        PathStroke { width, ..PathStroke::default() }
    }

    /// Whether it strokes (else it fills).
    pub fn strokes(&self) -> bool {
        self.hairline || self.width > 0.0
    }

    /// What drawing `path` this way covers at `res_scale` (a positive number of) pixels a unit, as a path filled by
    /// its fill type (as Android's Paint.getFillPath): the path effect first (at those pixels), then, for a stroke,
    /// the outline it covers, its curves within DEFAULT_FLATTEN_TOLERANCE pixels, filled non-zero (a hairline one
    /// pixel wide, its corners round).
    pub fn fill_path(&self, path: &Path, res_scale: f32) -> Path {
        let mut stroke = self.clone();
        let effected = path_effected(path, &mut stroke, res_scale);
        if !stroke.strokes() {
            return effected.into_owned();
        }
        let tolerance = DEFAULT_FLATTEN_TOLERANCE / res_scale;
        let contours = flatten_path(&effected, tolerance, |p| p);
        let outline = if stroke.hairline {
            stroke_outline(&contours, 1.0 / res_scale, stroke.cap, StrokeJoin::Round, stroke.miter, tolerance)
        } else {
            stroke_outline(&contours, stroke.width, stroke.cap, stroke.join, stroke.miter, tolerance)
        };
        let mut out = Path::new();
        for c in &outline {
            let Some((first, rest)) = c.points.split_first() else { continue };
            out.move_to(first.x, first.y);
            for p in rest {
                out.line_to(p.x, p.y);
            }
            out.close();
        }
        out
    }
}

/// How far a curve cut into lines may stray from it, in pixels.
pub const DEFAULT_FLATTEN_TOLERANCE: f32 = 0.2;

/// A polyline: a contour's curves cut into lines, closed or not.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Contour {
    pub points: Vec<Offset>,
    pub closed: bool,
}

fn dot(a: Offset, b: Offset) -> f32 {
    a.x * b.x + a.y * b.y
}
fn cross(a: Offset, b: Offset) -> f32 {
    a.x * b.y - a.y * b.x
}
fn length(a: Offset) -> f32 {
    a.x.hypot(a.y)
}
fn normalized(a: Offset) -> Offset {
    let l = length(a);
    if l > 0.0 { a * (1.0 / l) } else { Offset::new(1.0, 0.0) }
}
/// To the left of travel on the screen (y down): (1, 0) has (0, -1).
fn left_of(d: Offset) -> Offset {
    Offset::new(d.y, -d.x)
}

/// How many lines a curve of degree `degree` whose control polygon bends by `bend` (the largest
/// |p[i] - 2 p[i+1] + p[i+2]|) needs to stay within `tolerance` (Wang's formula).
fn pieces(bend: f32, degree: i32, tolerance: f32) -> i32 {
    let n = ((degree * (degree - 1)) as f32 / 8.0 * bend / tolerance).sqrt();
    (n.ceil() as i32).clamp(1, 1000)
}

/// The path's contours as polylines, each point first put where `place` says (where it is drawn), its curves cut
/// finely enough (by Wang's formula) that no point there is more than `tolerance` off the curve.
pub fn flatten_path(path: &Path, tolerance: f32, place: impl Fn(Offset) -> Offset) -> Vec<Contour> {
    let mut out: Vec<Contour> = Vec::new();
    let mut current = Offset::Zero;
    let mut start = Offset::Zero;
    let add = |out: &mut Vec<Contour>, current: &mut Offset, p: Offset| {
        if out.last().is_none_or(|c| c.closed) {
            // Drawing on after close(): from its start.
            out.push(Contour { points: vec![*current], closed: false });
        }
        out.last_mut().expect("a contour").points.push(p);
        *current = p;
    };
    for c in path.commands() {
        match *c {
            PathCommand::Move(p) => {
                let end = place(p);
                out.push(Contour { points: vec![end], closed: false });
                current = end;
                start = end;
            }
            PathCommand::Line(p) => add(&mut out, &mut current, place(p)),
            PathCommand::Quadratic(c1, p) => {
                let (p0, p1, end) = (current, place(c1), place(p));
                let n = pieces(length(p0 - p1 * 2.0 + end), 2, tolerance);
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let u = 1.0 - t;
                    add(&mut out, &mut current, p0 * (u * u) + p1 * (2.0 * u * t) + end * (t * t));
                }
            }
            PathCommand::Conic(c1, p, w) => {
                // (As a quadratic bounds how many pieces; each point on the conic itself.)
                let (p0, p1, end) = (current, place(c1), place(p));
                let n = pieces(length(p0 - p1 * 2.0 + end), 2, tolerance);
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let u = 1.0 - t;
                    let (a, b, d) = (u * u, 2.0 * w * u * t, t * t);
                    add(&mut out, &mut current, (p0 * a + p1 * b + end * d) * (1.0 / (a + b + d)));
                }
            }
            PathCommand::Cubic(c1, c2, p) => {
                let (p0, p1, p2, end) = (current, place(c1), place(c2), place(p));
                let n = pieces(length(p0 - p1 * 2.0 + p2).max(length(p1 - p2 * 2.0 + end)), 3, tolerance);
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let u = 1.0 - t;
                    add(
                        &mut out,
                        &mut current,
                        p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + end * (t * t * t),
                    );
                }
            }
            PathCommand::Close => {
                if let Some(last) = out.last_mut()
                    && !last.closed
                {
                    last.closed = true;
                }
                current = start;
            }
        }
    }
    out
}

/// Points round `center` from `from` (a vector from it) turning by `angle` radians (screen clockwise if positive),
/// the first one left out, within `tolerance` of the circle.
fn arc(out: &mut Vec<Offset>, center: Offset, from: Offset, angle: f32, tolerance: f32) {
    let r = length(from);
    if r <= 0.0 {
        return;
    }
    let step = 2.0 * (1.0 - tolerance / r).max(-1.0).acos();
    let n = 1.max((angle.abs() / step.max(1e-3)).ceil() as i32);
    let c = (angle / n as f32).cos();
    let s = (angle / n as f32).sin();
    let mut v = from;
    for _ in 0..n {
        v = Offset::new(v.x * c - v.y * s, v.x * s + v.y * c);
        out.push(center + v);
    }
}

/// The points one side of a stroke takes round the vertex p where it turns from `din` to `dout` (unit directions),
/// that side being to the left of travel `half` off the line; the segments on either side are `in_length` and
/// `out_length` long.
#[allow(clippy::too_many_arguments)]
fn join(
    points: &mut Vec<Offset>,
    p: Offset,
    din: Offset,
    dout: Offset,
    in_length: f32,
    out_length: f32,
    half: f32,
    kind: StrokeJoin,
    miter_limit: f32,
    tolerance: f32,
) {
    let n_in = left_of(din) * half;
    let n_out = left_of(dout) * half;
    let turn = cross(din, dout);
    let along = dot(din, dout);
    if turn.abs() < 1e-6 && along > 0.0 {
        // Straight on.
        points.push(p + n_in);
        return;
    }
    if turn < 0.0 {
        // The inner side. Where the two offset lines meet within both segments, that point: the outline doesn't
        // cross itself (whatever it covered twice would count twice in a pixel's coverage, and an edge would look
        // heavier). Two such points on a segment that pass each other fold the outline back along its offset, which
        // covers nothing. Otherwise (a segment shorter than the turn needs) round through the vertex, so that it
        // winds the same way.
        let mid = (n_in + n_out) * 0.5;
        let m = length(mid);
        let reach = half * turn.abs() / (1.0 + along); // half tan(turn / 2)
        if along > -0.99 && m > 1e-6 && reach <= in_length.min(out_length) {
            points.push(p + mid * (half * half / (m * m)));
            return;
        }
        points.push(p + n_in);
        points.push(p);
        points.push(p + n_out);
        return;
    }
    points.push(p + n_in);
    match kind {
        StrokeJoin::Miter => {
            let mid = (n_in + n_out) * 0.5;
            let m = length(mid);
            if m > 1e-6 && half / m <= miter_limit {
                points.push(p + mid * (half * half / (m * m)));
            }
        }
        StrokeJoin::Round => {
            arc(points, p, n_in, cross(n_in, n_out).atan2(dot(n_in, n_out)), tolerance);
            points.pop();
        }
        StrokeJoin::Bevel => {}
    }
    points.push(p + n_out);
}

/// The end of an open stroke at p going `dir`: from its left side round to its right.
fn cap(points: &mut Vec<Offset>, p: Offset, dir: Offset, half: f32, kind: StrokeCap, tolerance: f32) {
    let n = left_of(dir) * half;
    match kind {
        StrokeCap::Butt => {}
        StrokeCap::Square => {
            points.push(p + n + dir * half);
            points.push(p - n + dir * half);
        }
        StrokeCap::Round => {
            arc(points, p, n, std::f32::consts::PI, tolerance);
            points.pop();
        }
    }
}

/// The outline a stroke `width` wide covers along `contours` (closed polygons, filled by the non-zero rule): miter
/// joins beyond `miter_limit` (the miter's length over half the width, as Skia and FreeType have it) become bevels;
/// a contour that goes nowhere is a dot for round and square caps and nothing for butt ones, as in Skia. Round joins
/// and caps stay within `tolerance`.
pub fn stroke_outline(
    contours: &[Contour],
    width: f32,
    cap_kind: StrokeCap,
    join_kind: StrokeJoin,
    miter_limit: f32,
    tolerance: f32,
) -> Vec<Contour> {
    let mut out = Vec::new();
    let half = width / 2.0;
    if half <= 0.0 {
        return out;
    }
    for c in contours {
        // Points that go somewhere (a closed contour's last one isn't its first again).
        let mut p: Vec<Offset> = Vec::new();
        for &q in &c.points {
            if p.last().is_none_or(|&last| length(q - last) > 1e-4) {
                p.push(q);
            }
        }
        if c.closed && p.len() > 1 && length(p[0] - p[p.len() - 1]) <= 1e-4 {
            p.pop();
        }
        if p.len() == 1 || (p.len() == 2 && c.closed && length(p[1] - p[0]) <= 1e-4) {
            // A dot.
            if cap_kind == StrokeCap::Butt {
                continue;
            }
            let mut dot = Contour { points: Vec::new(), closed: true };
            if cap_kind == StrokeCap::Round {
                arc(&mut dot.points, p[0], Offset::new(half, 0.0), 2.0 * std::f32::consts::PI, tolerance);
            } else {
                dot.points = vec![
                    p[0] + Offset::new(-half, -half),
                    p[0] + Offset::new(half, -half),
                    p[0] + Offset::new(half, half),
                    p[0] + Offset::new(-half, half),
                ];
            }
            out.push(dot);
            continue;
        }
        if p.is_empty() {
            continue;
        }
        let n = p.len();
        if c.closed && n >= 2 {
            // Two loops, one each side, going opposite ways: what lies between them is covered.
            let dirs: Vec<Offset> = (0..n).map(|i| normalized(p[(i + 1) % n] - p[i])).collect();
            let lengths: Vec<f32> = (0..n).map(|i| length(p[(i + 1) % n] - p[i])).collect();
            let mut left = Contour { points: Vec::new(), closed: true };
            let mut right = Contour { points: Vec::new(), closed: true };
            for i in 0..n {
                let before = (i + n - 1) % n;
                join(
                    &mut left.points,
                    p[i],
                    dirs[before],
                    dirs[i],
                    lengths[before],
                    lengths[i],
                    half,
                    join_kind,
                    miter_limit,
                    tolerance,
                );
            }
            for k in 0..n {
                let i = n - 1 - k;
                let before = (i + n - 1) % n; // backwards: its left is the other side
                join(
                    &mut right.points,
                    p[i],
                    -dirs[i],
                    -dirs[before],
                    lengths[i],
                    lengths[before],
                    half,
                    join_kind,
                    miter_limit,
                    tolerance,
                );
            }
            out.push(left);
            out.push(right);
            continue;
        }
        // Open: one side forwards, the end's cap, the other side backwards, the start's cap.
        let dirs: Vec<Offset> = (0..n - 1).map(|i| normalized(p[i + 1] - p[i])).collect();
        let lengths: Vec<f32> = (0..n - 1).map(|i| length(p[i + 1] - p[i])).collect();
        let mut o = vec![p[0] + left_of(dirs[0]) * half];
        for i in 1..n - 1 {
            join(
                &mut o,
                p[i],
                dirs[i - 1],
                dirs[i],
                lengths[i - 1],
                lengths[i],
                half,
                join_kind,
                miter_limit,
                tolerance,
            );
        }
        o.push(p[n - 1] + left_of(dirs[n - 2]) * half);
        cap(&mut o, p[n - 1], dirs[n - 2], half, cap_kind, tolerance);
        o.push(p[n - 1] - left_of(dirs[n - 2]) * half);
        for k in 1..n - 1 {
            let i = n - 1 - k;
            join(
                &mut o,
                p[i],
                -dirs[i],
                -dirs[i - 1],
                lengths[i],
                lengths[i - 1],
                half,
                join_kind,
                miter_limit,
                tolerance,
            );
        }
        o.push(p[0] - left_of(dirs[0]) * half);
        cap(&mut o, p[0], -dirs[0], half, cap_kind, tolerance);
        out.push(Contour { points: o, closed: true });
    }
    out
}
