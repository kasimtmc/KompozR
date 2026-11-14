//! Path effects, as Skia's (SkDashPath, SkCornerPathEffect, SkPath1DPathEffect and SkComposePathEffect): Compose's
//! PathEffect.

use super::curves::NEARLY_ZERO;
use super::measure::{ContourMeasure, distance, finite, length, measure_contours};
use super::path::{Path, PathCommand};
use super::stroke::{PathStroke, StrokeCap};
use crate::geometry::Offset;
use std::borrow::Cow;
use std::sync::Arc;

/// How stamped_path_effect lays each copy of its shape on the path (Compose's StampedPathEffectStyle, Skia's
/// SkPath1DPathEffect styles): moved there, also turned the way the path goes, or bent along it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum StampedPathEffectStyle {
    #[default]
    Translate,
    Rotate,
    Morph,
}

#[derive(PartialEq, Debug)]
enum Node {
    /// (SkDashImpl) Its intervals, its phase within them, where the first dash starts.
    Dash {
        intervals: Vec<f32>,
        phase: f32,
        initial_dash_length: f32,
        interval_length: f32,
        initial_dash_index: usize,
    },
    Corner {
        radius: f32,
    },
    /// (SkPath1DPathEffectImpl) Its shape, advance, offset along each contour, style.
    Stamp {
        shape: Path,
        advance: f32,
        initial_offset: f32,
        style: StampedPathEffectStyle,
    },
    /// Inner's effect, then outer's.
    Chain {
        outer: Arc<Node>,
        inner: Arc<Node>,
    },
}

/// What a path goes through before it is drawn (Compose's PathEffect, Skia's): dashes, rounded corners, copies of a
/// shape stamped along it, or one effect after another. Applied as the path is drawn, at the pixels it is drawn at
/// (as Skia applies a paint's effect). One made of what Skia refuses (dashes of an odd count or none, a negative
/// interval, nothing on; a corner of no radius; a stamp of no advance or an empty shape) is none, as Skia's: it
/// leaves the path as it is. Equal when made the same way.
#[derive(Clone, Debug, Default)]
pub struct PathEffect(Option<Arc<Node>>);

impl PartialEq for PathEffect {
    fn eq(&self, o: &PathEffect) -> bool {
        match (&self.0, &o.0) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b) || a == b,
            _ => false,
        }
    }
}

impl PathEffect {
    /// None: the path as it is.
    pub fn none() -> PathEffect {
        PathEffect(None)
    }

    pub fn is_none(&self) -> bool {
        self.0.is_none()
    }

    /// On, off, on… (in pairs, an odd last one left out as Android leaves it; none negative, not all 0), `phase` into
    /// the pattern (its length's remainder; negative: from its end), starting again on each contour; a closed one
    /// joins its last dash to its first. Strokes only: a fill is drawn as it is.
    pub fn dash_path_effect(intervals: &[f32], phase: f32) -> PathEffect {
        let intervals = &intervals[..intervals.len() & !1]; // (Android's: an even count, the odd one left out)
        // (SkDashPath::ValidDashPath)
        if intervals.len() < 2 {
            return PathEffect::none();
        }
        let mut length = 0.0f32;
        for &v in intervals {
            if v < 0.0 {
                return PathEffect::none();
            }
            length += v;
        }
        if length.is_nan() || length <= 0.0 || !phase.is_finite() || !length.is_finite() {
            return PathEffect::none();
        }
        // (SkDashPath::CalcDashParameters) The phase within the pattern, a negative one from its end.
        let mut phase = phase;
        if phase < 0.0 {
            phase = -phase;
            if phase > length {
                phase %= length;
            }
            phase = length - phase;
            if phase == length {
                phase = 0.0;
            }
        } else if phase >= length {
            phase %= length;
        }
        // (find_first_interval)
        let (mut initial_dash_index, mut initial_dash_length) = (0, intervals[0]);
        let mut left = phase;
        for (i, &gap) in intervals.iter().enumerate() {
            if left > gap || (left == gap && gap != 0.0) {
                left -= gap;
            } else {
                initial_dash_index = i;
                initial_dash_length = gap - left;
                break;
            }
        }
        PathEffect(Some(Arc::new(Node::Dash {
            intervals: intervals.to_vec(),
            phase,
            initial_dash_length,
            interval_length: length,
            initial_dash_index,
        })))
    }

    /// Each corner between two lines cut back by `radius` (half each line at most) and joined by a quadratic through
    /// it; curves as they are.
    pub fn corner_path_effect(radius: f32) -> PathEffect {
        if !radius.is_finite() || radius <= 0.0 {
            return PathEffect::none();
        }
        PathEffect(Some(Arc::new(Node::Corner { radius })))
    }

    /// `shape` every `advance` along each contour, filled (the stroke becomes a fill); a phase of p puts the first one
    /// at advance - p.
    pub fn stamped_path_effect(shape: &Path, advance: f32, phase: f32, style: StampedPathEffectStyle) -> PathEffect {
        if advance <= 0.0 || !advance.is_finite() || !phase.is_finite() || shape.is_empty() {
            return PathEffect::none();
        }
        // The phase turned into an offset along the path (as PostScript's).
        let mut phase = phase;
        if phase < 0.0 {
            phase = -phase;
            if phase > advance {
                phase %= advance;
            }
        } else {
            if phase > advance {
                phase %= advance;
            }
            phase = advance - phase;
        }
        if phase >= advance {
            phase = 0.0;
        }
        PathEffect(Some(Arc::new(Node::Stamp { shape: shape.clone(), advance, initial_offset: phase, style })))
    }

    /// `inner`'s effect, then `outer`'s on what it made.
    pub fn chain_path_effect(outer: &PathEffect, inner: &PathEffect) -> PathEffect {
        match (&outer.0, &inner.0) {
            (None, _) => inner.clone(),
            (_, None) => outer.clone(),
            (Some(o), Some(i)) => PathEffect(Some(Arc::new(Node::Chain { outer: o.clone(), inner: i.clone() }))),
        }
    }

    /// What the effect makes of `src` drawn with `stroke` at `res_scale` pixels a unit (Skia's filterPath): into
    /// `dst`, `stroke` changed if it draws the result otherwise (a stamp's, a dashed line's, filled); false if it
    /// leaves the path as it is. (What it makes winds non-zero, as a Skia path builder's does.)
    pub fn filter_path(&self, dst: &mut Path, src: &Path, stroke: &mut PathStroke, res_scale: f32) -> bool {
        self.0.as_ref().is_some_and(|n| filter(n, dst, src, stroke, res_scale))
    }

    /// How far past the bounds of a path what it makes of it can reach (as Skia's computeFastBounds outsets them for
    /// an effect): a stamp's corner farthest from where it is put (turned along the path or bent round it, no
    /// farther); a dash or a corner, not past them.
    pub fn reach(&self) -> f32 {
        fn of(n: &Node) -> f32 {
            match n {
                Node::Stamp { shape, .. } => {
                    let b = shape.get_bounds();
                    let mut far = 0.0f32;
                    for x in [b.left, b.right] {
                        for y in [b.top, b.bottom] {
                            far = far.max(x.hypot(y));
                        }
                    }
                    far
                }
                Node::Chain { outer, inner } => of(outer) + of(inner),
                Node::Dash { .. } | Node::Corner { .. } => 0.0,
            }
        }
        self.0.as_deref().map_or(0.0, of)
    }
}

fn filter(node: &Node, dst: &mut Path, src: &Path, stroke: &mut PathStroke, res_scale: f32) -> bool {
    match node {
        Node::Dash { intervals, initial_dash_length, interval_length, initial_dash_index, .. } => dash_filter(
            dst,
            src,
            stroke,
            res_scale,
            intervals,
            *initial_dash_length,
            *interval_length,
            *initial_dash_index,
        ),
        Node::Corner { radius } => corner_filter(dst, src, *radius),
        Node::Stamp { shape, advance, initial_offset, style } => {
            stamp_filter(dst, src, stroke, shape, *advance, *initial_offset, *style)
        }
        Node::Chain { outer, inner } => {
            // (SkComposePathEffect)
            if filter(inner, dst, src, stroke, res_scale) {
                let tmp = std::mem::take(dst);
                return filter(outer, dst, &tmp, stroke, res_scale);
            }
            filter(outer, dst, src, stroke, res_scale)
        }
    }
}

/// The scale a matrix draws strokes and path effects at (Skia's ComputeResScaleForStroking): its larger column's
/// length (1 if none).
pub fn res_scale_of(scale_x: f32, skew_y: f32, skew_x: f32, scale_y: f32) -> f32 {
    let sx = length(scale_x, skew_y);
    let sy = length(skew_x, scale_y);
    if sx.is_finite() && sy.is_finite() {
        let scale = sx.max(sy);
        if scale > 0.0 {
            return scale.min(1e5);
        }
    }
    1.0
}

/// `stroke`'s path effect on `path` at `res_scale` (as Skia's FillPathWithPaint applies it): the path to draw (`path`
/// itself where the effect leaves it as it is) and how, in `stroke`, its effect gone.
pub fn path_effected<'a>(path: &'a Path, stroke: &mut PathStroke, res_scale: f32) -> Cow<'a, Path> {
    let effect = stroke.path_effect.take();
    let mut effected = Path::new();
    if let Some(effect) = effect
        && effect.filter_path(&mut effected, path, stroke, res_scale)
    {
        return Cow::Owned(effected);
    }
    Cow::Borrowed(path)
}

// ---- A path's verbs as Skia's iterator gives them ------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Verb {
    Move,
    Line,
    Quad,
    Conic,
    Cubic,
    Close,
}

#[derive(Clone, Copy, Debug)]
struct Rec {
    verb: Verb,
    pts: [Offset; 4],
    w: f32,
}

/// A path's verbs as SkPath::Iter gives them (not forced closed): each with its points (the one it starts from
/// first), a closing line before a close that doesn't end where its contour began, a trailing move left out.
struct Iter<'a> {
    c: &'a [PathCommand],
    i: usize,
    move_to: Offset,
    last: Offset,
    rec: Rec,
}

impl<'a> Iter<'a> {
    fn new(path: &'a Path) -> Iter<'a> {
        Iter {
            c: path.commands(),
            i: 0,
            move_to: Offset::Zero,
            last: Offset::Zero,
            rec: Rec { verb: Verb::Close, pts: [Offset::Zero; 4], w: 1.0 },
        }
    }

    fn next(&mut self) -> Option<Rec> {
        let c = *self.c.get(self.i)?;
        let r = &mut self.rec;
        match c {
            PathCommand::Move(p) => {
                if self.i + 1 >= self.c.len() {
                    return None; // a trailing move
                }
                r.verb = Verb::Move;
                self.move_to = p;
                self.last = p;
                r.pts[0] = p;
            }
            PathCommand::Line(p) => {
                r.verb = Verb::Line;
                r.pts[0] = self.last;
                r.pts[1] = p;
                self.last = p;
            }
            PathCommand::Quadratic(c1, p) => {
                r.verb = Verb::Quad;
                r.pts[0] = self.last;
                r.pts[1] = c1;
                r.pts[2] = p;
                r.w = 1.0;
                self.last = p;
            }
            PathCommand::Conic(c1, p, w) => {
                r.verb = Verb::Conic;
                r.pts[0] = self.last;
                r.pts[1] = c1;
                r.pts[2] = p;
                r.w = w;
                self.last = p;
            }
            PathCommand::Cubic(c1, c2, p) => {
                r.verb = Verb::Cubic;
                r.pts[0] = self.last;
                r.pts[1] = c1;
                r.pts[2] = c2;
                r.pts[3] = p;
                self.last = p;
            }
            PathCommand::Close => {
                if self.last != self.move_to && !self.last.x.is_nan() && !self.last.y.is_nan() {
                    // The line back first.
                    r.verb = Verb::Line;
                    r.pts[0] = self.last;
                    r.pts[1] = self.move_to;
                    self.last = self.move_to;
                    return Some(*r);
                }
                r.verb = Verb::Close;
                r.pts[0] = self.move_to;
                self.last = self.move_to;
            }
        }
        self.i += 1;
        Some(*r)
    }

    /// Whether the contour being given ends with a close (SkPath::Iter::isClosedContour).
    fn is_closed_contour(&self) -> bool {
        for k in self.i..self.c.len() {
            match self.c[k] {
                PathCommand::Close => return true,
                PathCommand::Move(_) if k > self.i => return false,
                _ => {}
            }
        }
        false
    }
}

/// Its two points, if a path is one line (SkPath::isLine).
fn is_line(path: &Path) -> Option<[Offset; 2]> {
    match path.commands() {
        [PathCommand::Move(a), PathCommand::Line(b)] => Some([*a, *b]),
        _ => None,
    }
}

fn add_conic(dst: &mut Path, c: Offset, to: Offset, w: f32) {
    dst.conic_to(c.x, c.y, to.x, to.y, w);
}

// ---- Dashes (SkDashPath) ------------------------------------------------------------------------------------------

const MAX_DASH_COUNT: f32 = 1_000_000.0;

fn is_even(x: usize) -> bool {
    (x & 1) == 0
}

/// (SkDashPath's SpecialLineRec) A line dashed with butt caps: each dash its rectangle, filled.
struct SpecialLine {
    pts: [Offset; 2],
    tangent: Offset,
    normal: Offset,
    path_length: f32,
}

impl SpecialLine {
    fn new(src: &Path, stroke: &mut PathStroke, interval_count: usize, interval_length: f32) -> Option<SpecialLine> {
        if stroke.hairline {
            return None;
        }
        let pts = is_line(src)?;
        if stroke.cap != StrokeCap::Butt {
            return None;
        }
        let path_length = distance(pts[0], pts[1]);
        let mut tangent = pts[1] - pts[0];
        if tangent.x == 0.0 && tangent.y == 0.0 {
            return None;
        }
        tangent = tangent * (1.0 / path_length);
        if !finite(tangent) {
            return None;
        }
        let normal = Offset::new(tangent.y, -tangent.x) * (stroke.width / 2.0); // (RotateCCW)
        let pt_count = (path_length * interval_count as f32 / interval_length).min(MAX_DASH_COUNT);
        if pt_count.is_nan() {
            return None;
        }
        *stroke = PathStroke::default(); // filled: the dashes are their own outlines
        Some(SpecialLine { pts, tangent, normal, path_length })
    }

    fn add_segment(&self, d0: f32, mut d1: f32, dst: &mut Path) {
        if d1 > self.path_length {
            d1 = self.path_length;
        }
        let (p0, t, n) = (self.pts[0], self.tangent, self.normal);
        let x0 = p0.x + t.x * d0;
        let x1 = p0.x + t.x * d1;
        let y0 = p0.y + t.y * d0;
        let y1 = p0.y + t.y * d1;
        dst.move_to(x0 + n.x, y0 + n.y);
        dst.line_to(x1 + n.x, y1 + n.y);
        dst.line_to(x1 - n.x, y1 - n.y);
        dst.line_to(x0 - n.x, y0 - n.y);
    }
}

/// (SkDashPath::InternalFilter, without a cull rect)
#[allow(clippy::too_many_arguments)]
fn dash_filter(
    dst: &mut Path,
    source: &Path,
    stroke: &mut PathStroke,
    res_scale: f32,
    intervals: &[f32],
    initial_dash_length: f32,
    interval_length: f32,
    initial_dash_index: usize,
) -> bool {
    if source.is_empty() {
        return true;
    }
    if !stroke.strokes() {
        return false; // a fill is not dashed
    }
    let count = intervals.len();
    let mut zero_line = Path::new();
    let mut src = source;
    if let Some(mut line) = is_line(source)
        && line[0] == line[1]
    {
        // (cull_path: a zero-length line made a hair long)
        line[1].x += 1.001f32.max(line[1].x) * NEARLY_ZERO;
        zero_line.move_to(line[0].x, line[0].y);
        zero_line.line_to(line[1].x, line[1].y);
        src = &zero_line;
    }
    let special = SpecialLine::new(src, stroke, count >> 1, interval_length);
    let mut dash_count = 0.0f32;
    for cm in measure_contours(src, false, res_scale) {
        let mut skip_first_segment = cm.closed;
        let mut added_segment = false;
        let length = cm.length;
        let mut index = initial_dash_index;
        dash_count += length * (count >> 1) as f32 / interval_length;
        if dash_count > MAX_DASH_COUNT {
            dst.reset();
            return false;
        }
        let mut distance = 0.0f64;
        let mut dlen = initial_dash_length as f64;
        while distance < length as f64 {
            added_segment = false;
            if is_even(index) && !skip_first_segment {
                added_segment = true;
                match &special {
                    Some(line) => line.add_segment(distance as f32, (distance + dlen) as f32, dst),
                    None => {
                        cm.get_segment(distance as f32, (distance + dlen) as f32, dst, true);
                    }
                }
            }
            distance += dlen;
            skip_first_segment = false;
            index += 1;
            if index == count {
                index = 0;
            }
            dlen = intervals[index] as f64;
        }
        // A closed contour ending in a dash joins the first one (skipped at the start).
        if cm.closed && is_even(initial_dash_index) && initial_dash_length >= 0.0 {
            cm.get_segment(0.0, initial_dash_length, dst, !added_segment);
        }
    }
    true
}

// ---- Corners (SkCornerPathEffect) ---------------------------------------------------------------------------------

/// (SkCornerPathEffectImpl::onFilterPath)
fn corner_filter(dst: &mut Path, src: &Path, radius: f32) -> bool {
    if radius <= 0.0 {
        return false;
    }
    // (ComputeStep) The step from a towards b a corner takes: radius long, or half the way if it is shorter than two.
    let step = |a: Offset, b: Offset, s: &mut Offset| {
        let d = distance(a, b);
        *s = b - a;
        if d <= radius * 2.0 {
            *s = *s * 0.5;
            return false;
        }
        *s = *s * (radius / d);
        true
    };
    let mut prev_valid = true;
    let mut have_prev = false;
    let mut prev_verb = Verb::Close;
    let (mut move_to, mut last_corner, mut first_step, mut s) =
        (Offset::Zero, Offset::Zero, Offset::Zero, Offset::Zero);
    let mut iter = Iter::new(src);
    while let Some(r) = iter.next() {
        let p = r.pts;
        match r.verb {
            Verb::Move => {
                if have_prev && prev_verb == Verb::Line {
                    dst.line_to(last_corner.x, last_corner.y); // the open contour before ends
                }
                if iter.is_closed_contour() {
                    move_to = p[0];
                    prev_valid = false;
                } else {
                    dst.move_to(p[0].x, p[0].y);
                    prev_valid = true;
                }
            }
            Verb::Line => {
                let draw_segment = step(p[0], p[1], &mut s);
                if !prev_valid {
                    dst.move_to(move_to.x + s.x, move_to.y + s.y);
                } else {
                    dst.quadratic_to(p[0].x, p[0].y, p[0].x + s.x, p[0].y + s.y);
                }
                if draw_segment {
                    dst.line_to(p[1].x - s.x, p[1].y - s.y);
                }
                last_corner = p[1];
                prev_valid = true;
            }
            Verb::Quad | Verb::Conic | Verb::Cubic => {
                // Curves as they are.
                if !prev_valid {
                    dst.move_to(p[0].x, p[0].y);
                    prev_valid = true;
                }
                match r.verb {
                    Verb::Quad => {
                        dst.quadratic_to(p[1].x, p[1].y, p[2].x, p[2].y);
                        last_corner = p[2];
                    }
                    Verb::Conic => {
                        add_conic(dst, p[1], p[2], r.w);
                        last_corner = p[2];
                    }
                    _ => {
                        dst.cubic_to(p[1].x, p[1].y, p[2].x, p[2].y, p[3].x, p[3].y);
                        last_corner = p[3];
                    }
                }
                first_step = Offset::Zero;
            }
            Verb::Close => {
                if first_step.x != 0.0 || first_step.y != 0.0 {
                    dst.quadratic_to(
                        last_corner.x,
                        last_corner.y,
                        last_corner.x + first_step.x,
                        last_corner.y + first_step.y,
                    );
                }
                dst.close();
                prev_valid = false;
            }
        }
        if have_prev && prev_verb == Verb::Move {
            first_step = s;
        }
        prev_verb = r.verb;
        have_prev = true;
    }
    if prev_valid {
        dst.line_to(last_corner.x, last_corner.y);
    }
    true
}

// ---- Stamps (SkPath1DPathEffect) ----------------------------------------------------------------------------------

/// A point of the stamp at `dist` along the contour, bent along it (morphpoints).
fn morph_point(cm: &ContourMeasure, dist: f32, s: Offset) -> Option<Offset> {
    let (pos, tan) = cm.get_pos_tan(dist + s.x)?;
    // setSinCos(tan.y, tan.x), preTranslate(-s.x, 0), postTranslate(pos): (s.x, s.y) mapped.
    let tx = tan.x * -s.x + pos.x;
    let ty = tan.y * -s.x + pos.y;
    Some(Offset::new(tan.x * s.x + -tan.y * s.y + tx, tan.y * s.x + tan.x * s.y + ty))
}

/// (SkPath1DPathEffectImpl) The stamp at each advance along each contour, filled.
fn stamp_filter(
    dst: &mut Path,
    src: &Path,
    stroke: &mut PathStroke,
    shape: &Path,
    advance: f32,
    initial_offset: f32,
    style: StampedPathEffectStyle,
) -> bool {
    *stroke = PathStroke::default();
    const MAX_ITERATIONS: i32 = 100_000; // (MAX_REASONABLE_ITERATIONS)
    for cm in measure_contours(src, false, 1.0) {
        let mut governor = MAX_ITERATIONS;
        let mut d = initial_offset;
        while d < cm.length {
            governor -= 1;
            if governor < 0 {
                break;
            }
            match style {
                StampedPathEffectStyle::Translate => {
                    if let Some((pos, _)) = cm.get_pos_tan(d) {
                        dst.add_path(shape, pos);
                    }
                }
                StampedPathEffectStyle::Rotate => {
                    if let Some((pos, tan)) = cm.get_pos_tan(d) {
                        let map = |q: Offset| {
                            Offset::new(tan.x * q.x - tan.y * q.y + pos.x, tan.y * q.x + tan.x * q.y + pos.y)
                        };
                        for c in shape.commands() {
                            match c.map(map) {
                                PathCommand::Move(to) => dst.move_to(to.x, to.y),
                                PathCommand::Line(to) => dst.line_to(to.x, to.y),
                                PathCommand::Quadratic(p1, to) => dst.quadratic_to(p1.x, p1.y, to.x, to.y),
                                PathCommand::Conic(p1, to, w) => add_conic(dst, p1, to, w),
                                PathCommand::Cubic(p1, p2, to) => dst.cubic_to(p1.x, p1.y, p2.x, p2.y, to.x, to.y),
                                PathCommand::Close => dst.close(),
                            }
                        }
                    }
                }
                StampedPathEffectStyle::Morph => {
                    // (morphpath: a line bent as a quadratic)
                    let mut iter = Iter::new(shape);
                    while let Some(r) = iter.next() {
                        let p = r.pts;
                        let morph = |from: usize, n: usize| -> Option<Vec<Offset>> {
                            (0..n).map(|k| morph_point(&cm, d, p[from + k])).collect()
                        };
                        match r.verb {
                            Verb::Move => {
                                if let Some(q) = morph(0, 1) {
                                    dst.move_to(q[0].x, q[0].y);
                                }
                            }
                            Verb::Line => {
                                let mid = Offset::new((p[0].x + p[1].x) * 0.5, (p[0].y + p[1].y) * 0.5); // (sk_float_midpoint)
                                if let (Some(q0), Some(q1)) = (morph_point(&cm, d, mid), morph_point(&cm, d, p[1])) {
                                    dst.quadratic_to(q0.x, q0.y, q1.x, q1.y);
                                }
                            }
                            Verb::Quad => {
                                if let Some(q) = morph(1, 2) {
                                    dst.quadratic_to(q[0].x, q[0].y, q[1].x, q[1].y);
                                }
                            }
                            Verb::Conic => {
                                if let Some(q) = morph(1, 2) {
                                    add_conic(dst, q[0], q[1], r.w);
                                }
                            }
                            Verb::Cubic => {
                                if let Some(q) = morph(1, 3) {
                                    dst.cubic_to(q[0].x, q[0].y, q[1].x, q[1].y, q[2].x, q[2].y);
                                }
                            }
                            Verb::Close => dst.close(),
                        }
                    }
                }
            }
            d += advance;
        }
        if governor < 0 {
            return false;
        }
    }
    true
}
