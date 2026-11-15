//! Projective transforms of the plane (C++'s gpu::Projective): what a canvas's transforms make of its pixels, a
//! Matrix's plane part, perspective included; what the display list maps areas with and the GPU renderer draws
//! through.

use crate::geometry::{Offset, Rect};
use crate::matrix::Matrix;
use crate::path::Contour;
use std::ops::Mul;

/// A projective transform of the plane: (x, y, 1) to (x', y', w), row by row, the point x' / w, y' / w.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Projective {
    pub m: [f32; 9],
}

impl Default for Projective {
    fn default() -> Projective {
        Projective::Identity
    }
}

/// How far a point may be from the eye's plane (w) and still be in front of it, where polygons are projected.
const NEAR: f32 = 1e-4;

impl Projective {
    pub const Identity: Projective = Projective { m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0] };

    pub const fn translation(x: f32, y: f32) -> Projective {
        Projective { m: [1.0, 0.0, x, 0.0, 1.0, y, 0.0, 0.0, 1.0] }
    }

    pub const fn scaling(x: f32, y: f32) -> Projective {
        Projective { m: [x, 0.0, 0.0, 0.0, y, 0.0, 0.0, 0.0, 1.0] }
    }

    /// A Matrix's plane part (its z row and column left out, as Android leaves them out); where w is the same
    /// everywhere, 1 (in front, whatever its sign was).
    pub fn of(t: &Matrix) -> Projective {
        let v = &t.values;
        let mut p = Projective {
            m: [
                v[Matrix::ScaleX],
                v[Matrix::SkewX],
                v[Matrix::TranslateX],
                v[Matrix::SkewY],
                v[Matrix::ScaleY],
                v[Matrix::TranslateY],
                v[Matrix::Perspective0],
                v[Matrix::Perspective1],
                v[Matrix::Perspective2],
            ],
        };
        if p.affine() && p.m[8] != 0.0 && p.m[8] != 1.0 {
            let w = v[Matrix::Perspective2];
            p.m.iter_mut().for_each(|x| *x /= w);
        }
        p
    }

    pub fn is_identity(&self) -> bool {
        *self == Projective::Identity
    }

    /// Only scaled and moved: axis-aligned rects stay so.
    pub fn axis_aligned(&self) -> bool {
        self.m[1] == 0.0 && self.m[3] == 0.0 && self.m[6] == 0.0 && self.m[7] == 0.0
    }

    /// No perspective: w the same everywhere.
    pub fn affine(&self) -> bool {
        self.m[6] == 0.0 && self.m[7] == 0.0
    }

    /// Where (x, y) goes.
    pub fn apply(&self, x: f32, y: f32) -> Offset {
        self.apply_w(x, y).0
    }

    /// Where (x, y) goes, and its w there (the point is behind the eye where that isn't positive; where it is 0,
    /// the origin).
    pub fn apply_w(&self, x: f32, y: f32) -> (Offset, f32) {
        let m = &self.m;
        let q = m[6] * x + m[7] * y + m[8];
        let k = if q != 0.0 { 1.0 / q } else { 0.0 };
        (Offset::new((m[0] * x + m[1] * y + m[2]) * k, (m[3] * x + m[4] * y + m[5]) * k), q)
    }

    /// The bounding box of the mapped rect; if part of it is behind the eye, all of the plane.
    pub fn map_rect(&self, r: Rect) -> Rect {
        let m = &self.m;
        if self.axis_aligned() && m[8] != 0.0 {
            // Only scaled and moved: no need for the corners.
            let (x0, x1) = ((m[0] * r.left + m[2]) / m[8], (m[0] * r.right + m[2]) / m[8]);
            let (y0, y1) = ((m[4] * r.top + m[5]) / m[8], (m[4] * r.bottom + m[5]) / m[8]);
            return Rect::new(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
        }
        let affine = self.affine(); // (w the same everywhere: in front whatever its sign)
        let (mut l, mut t, mut rr, mut bb) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        for c in 0..4 {
            let x = if c & 1 != 0 { r.right } else { r.left };
            let y = if c & 2 != 0 { r.bottom } else { r.top };
            let w = m[6] * x + m[7] * y + m[8];
            let in_front = if affine { w != 0.0 } else { w > 0.0 }; // (NaN: not)
            if !in_front {
                return Rect::new(-1e30, -1e30, 1e30, 1e30); // behind the eye
            }
            let (px, py) = ((m[0] * x + m[1] * y + m[2]) / w, (m[3] * x + m[4] * y + m[5]) / w);
            (l, t, rr, bb) = (l.min(px), t.min(py), rr.max(px), bb.max(py));
        }
        Rect::new(l, t, rr, bb)
    }

    /// Its inverse; the identity if there is none.
    pub fn inverse(&self) -> Projective {
        let a = &self.m;
        let c0 = a[4] * a[8] - a[5] * a[7];
        let c1 = a[5] * a[6] - a[3] * a[8];
        let c2 = a[3] * a[7] - a[4] * a[6];
        let det = a[0] * c0 + a[1] * c1 + a[2] * c2;
        if det == 0.0 || !det.is_finite() {
            return Projective::Identity;
        }
        let k = 1.0 / det;
        Projective {
            m: [
                c0 * k,
                (a[2] * a[7] - a[1] * a[8]) * k,
                (a[1] * a[5] - a[2] * a[4]) * k,
                c1 * k,
                (a[0] * a[8] - a[2] * a[6]) * k,
                (a[2] * a[3] - a[0] * a[5]) * k,
                c2 * k,
                (a[1] * a[6] - a[0] * a[7]) * k,
                (a[0] * a[4] - a[1] * a[3]) * k,
            ],
        }
    }

    /// How much it enlarges lengths at (x, y), the most in any direction.
    pub fn max_scale(&self, x: f32, y: f32) -> f32 {
        self.stretch(x, y).0
    }

    /// How much it enlarges lengths at (x, y), the least in any direction.
    pub fn min_scale(&self, x: f32, y: f32) -> f32 {
        self.stretch(x, y).1
    }

    /// The singular values of the map's derivative at (x, y): how much it stretches lengths there, the most and the
    /// least.
    fn stretch(&self, x: f32, y: f32) -> (f32, f32) {
        let m = &self.m;
        let (q, w) = self.apply_w(x, y);
        if w == 0.0 {
            return (0.0, 0.0);
        }
        let (j00, j01) = ((m[0] - m[6] * q.x) / w, (m[1] - m[7] * q.x) / w);
        let (j10, j11) = ((m[3] - m[6] * q.y) / w, (m[4] - m[7] * q.y) / w);
        let t = j00 * j00 + j01 * j01 + j10 * j10 + j11 * j11;
        let det = (j00 * j11 - j01 * j10).abs();
        let root = (t * t - 4.0 * det * det).max(0.0).sqrt();
        (((t + root) / 2.0).sqrt(), ((t - root) / 2.0).max(0.0).sqrt())
    }

    /// Polygons (each closed) mapped through it, cut where they pass behind the eye (Sutherland-Hodgman at w just
    /// in front of it); one left with fewer than two points goes.
    pub fn project(&self, polygons: &[Contour]) -> Vec<Contour> {
        let m = &self.m;
        let mapped = |q: Offset| {
            (m[0] * q.x + m[1] * q.y + m[2], m[3] * q.x + m[4] * q.y + m[5], m[6] * q.x + m[7] * q.y + m[8])
        };
        let mut out = Vec::new();
        let mut clipped: Vec<(f32, f32, f32)> = Vec::new();
        for c in polygons {
            let Some(&last) = c.points.last() else { continue };
            if c.points.len() < 2 {
                continue;
            }
            // Each point in homogeneous coordinates, the polygon cut to w >= NEAR.
            clipped.clear();
            let mut prev = mapped(last);
            for &p in &c.points {
                let cur = mapped(p);
                let (inside, was_inside) = (cur.2 >= NEAR, prev.2 >= NEAR);
                if inside != was_inside {
                    let t = (NEAR - prev.2) / (cur.2 - prev.2);
                    clipped.push((prev.0 + (cur.0 - prev.0) * t, prev.1 + (cur.1 - prev.1) * t, NEAR));
                }
                if inside {
                    clipped.push(cur);
                }
                prev = cur;
            }
            if clipped.len() < 2 {
                continue;
            }
            out.push(Contour {
                points: clipped.iter().map(|&(x, y, w)| Offset::new(x / w, y / w)).collect(),
                closed: true,
            });
        }
        out
    }

    /// Polylines mapped through it as they are, closed ones as [`project`](Projective::project) maps them and open
    /// ones open: cut where they pass behind the eye into the runs in front of it (a hairline's lines, which a closing
    /// line would add to). A single point in front stays one.
    pub fn project_polylines(&self, polylines: &[Contour]) -> Vec<Contour> {
        let m = &self.m;
        let mapped = |q: Offset| {
            (m[0] * q.x + m[1] * q.y + m[2], m[3] * q.x + m[4] * q.y + m[5], m[6] * q.x + m[7] * q.y + m[8])
        };
        let seen = |(x, y, w): (f32, f32, f32)| Offset::new(x / w, y / w);
        let mut out = Vec::new();
        for c in polylines {
            if c.closed {
                out.extend(self.project(std::slice::from_ref(c)));
                continue;
            }
            let mut run: Vec<Offset> = Vec::new();
            let mut prev: Option<(f32, f32, f32)> = None;
            for &p in &c.points {
                let cur = mapped(p);
                let inside = cur.2 >= NEAR;
                if let Some(prev) = prev
                    && inside != (prev.2 >= NEAR)
                {
                    let t = (NEAR - prev.2) / (cur.2 - prev.2);
                    run.push(seen((prev.0 + (cur.0 - prev.0) * t, prev.1 + (cur.1 - prev.1) * t, NEAR)));
                    if !inside {
                        out.push(Contour { points: std::mem::take(&mut run), closed: false });
                    }
                }
                if inside {
                    run.push(seen(cur));
                }
                prev = Some(cur);
            }
            if run.len() >= 2 || (run.len() == 1 && c.points.len() == 1) {
                out.push(Contour { points: run, closed: false });
            }
        }
        out
    }
}

impl Mul for Projective {
    type Output = Projective;

    /// `rhs`, then `self`.
    fn mul(self, rhs: Projective) -> Projective {
        let (a, b) = (&self.m, &rhs.m);
        let mut out = Projective { m: [0.0; 9] };
        for r in 0..3 {
            for c in 0..3 {
                out.m[r * 3 + c] = a[r * 3] * b[c] + a[r * 3 + 1] * b[3 + c] + a[r * 3 + 2] * b[6 + c];
            }
        }
        out
    }
}
