//! A transform of points (Compose's Matrix), operation for operation as Compose's.

use crate::geometry::{Offset, Rect};

/// 4x4, `values[row * 4 + column]`, a point taken as the row (x, y, z, 1) times it, so row 3 is the translation and
/// column 3 the perspective. As in Compose, `translate`, `scale` and `rotate_z` come before what the matrix did
/// already (as a canvas's do), `rotate_x` and `rotate_y` after it. Drawn through, it acts in the plane: its z row
/// and column are left out, as Android leaves them out.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Matrix {
    pub values: [f32; 16],
}

impl Default for Matrix {
    fn default() -> Self {
        Matrix::IDENTITY
    }
}

/// The sine and cosine of `degrees`, worked out in double as Compose does.
fn sin_cos(degrees: f32) -> (f32, f32) {
    let r = degrees as f64 * std::f64::consts::PI / 180.0;
    (r.sin() as f32, r.cos() as f32)
}

impl Matrix {
    pub const ScaleX: usize = 0;
    pub const SkewY: usize = 1;
    pub const Perspective0: usize = 3;
    pub const SkewX: usize = 4;
    pub const ScaleY: usize = 5;
    pub const Perspective1: usize = 7;
    pub const ScaleZ: usize = 10;
    pub const TranslateX: usize = 12;
    pub const TranslateY: usize = 13;
    pub const TranslateZ: usize = 14;
    pub const Perspective2: usize = 15;

    pub const IDENTITY: Matrix =
        Matrix { values: [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0] };

    pub fn new() -> Matrix {
        Matrix::IDENTITY
    }

    pub fn get(&self, row: usize, column: usize) -> f32 {
        self.values[row * 4 + column]
    }
    pub fn set(&mut self, row: usize, column: usize, v: f32) {
        self.values[row * 4 + column] = v;
    }

    /// Where `point` goes (divided by its w; where that is 0, the origin).
    pub fn map(&self, p: Offset) -> Offset {
        let m = |r, c| self.get(r, c);
        let inverse = 1.0 / (m(0, 3) * p.x + m(1, 3) * p.y + m(3, 3));
        let w = if inverse.is_finite() { inverse } else { 0.0 };
        Offset::new(w * (m(0, 0) * p.x + m(1, 0) * p.y + m(3, 0)), w * (m(0, 1) * p.x + m(1, 1) * p.y + m(3, 1)))
    }

    /// The bounds of where the rectangle's corners go.
    pub fn map_rect(&self, r: Rect) -> Rect {
        let pts =
            [self.map(r.top_left()), self.map(r.bottom_left()), self.map(r.top_right()), self.map(r.bottom_right())];
        let (mut l, mut t, mut rr, mut b) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        for p in pts {
            l = l.min(p.x);
            t = t.min(p.y);
            rr = rr.max(p.x);
            b = b.max(p.y);
        }
        Rect::new(l, t, rr, b)
    }

    /// This, then `m`.
    pub fn times_assign(&mut self, o: &Matrix) {
        let mut out = Matrix::IDENTITY;
        for row in 0..4 {
            for col in 0..4 {
                out.set(
                    row,
                    col,
                    self.get(row, 0) * o.get(0, col)
                        + self.get(row, 1) * o.get(1, col)
                        + self.get(row, 2) * o.get(2, col)
                        + self.get(row, 3) * o.get(3, col),
                );
            }
        }
        *self = out;
    }

    /// Left as it is if it has no inverse.
    pub fn invert(&mut self) {
        let m = |r, c| self.get(r, c);
        let (a00, a01, a02, a03) = (m(0, 0), m(0, 1), m(0, 2), m(0, 3));
        let (a10, a11, a12, a13) = (m(1, 0), m(1, 1), m(1, 2), m(1, 3));
        let (a20, a21, a22, a23) = (m(2, 0), m(2, 1), m(2, 2), m(2, 3));
        let (a30, a31, a32, a33) = (m(3, 0), m(3, 1), m(3, 2), m(3, 3));
        let b00 = a00 * a11 - a01 * a10;
        let b01 = a00 * a12 - a02 * a10;
        let b02 = a00 * a13 - a03 * a10;
        let b03 = a01 * a12 - a02 * a11;
        let b04 = a01 * a13 - a03 * a11;
        let b05 = a02 * a13 - a03 * a12;
        let b06 = a20 * a31 - a21 * a30;
        let b07 = a20 * a32 - a22 * a30;
        let b08 = a20 * a33 - a23 * a30;
        let b09 = a21 * a32 - a22 * a31;
        let b10 = a21 * a33 - a23 * a31;
        let b11 = a22 * a33 - a23 * a32;
        let det = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;
        if det == 0.0 {
            return;
        }
        let k = 1.0 / det;
        self.values = [
            (a11 * b11 - a12 * b10 + a13 * b09) * k,
            (-a01 * b11 + a02 * b10 - a03 * b09) * k,
            (a31 * b05 - a32 * b04 + a33 * b03) * k,
            (-a21 * b05 + a22 * b04 - a23 * b03) * k,
            (-a10 * b11 + a12 * b08 - a13 * b07) * k,
            (a00 * b11 - a02 * b08 + a03 * b07) * k,
            (-a30 * b05 + a32 * b02 - a33 * b01) * k,
            (a20 * b05 - a22 * b02 + a23 * b01) * k,
            (a10 * b10 - a11 * b08 + a13 * b06) * k,
            (-a00 * b10 + a01 * b08 - a03 * b06) * k,
            (a30 * b04 - a31 * b02 + a33 * b00) * k,
            (-a20 * b04 + a21 * b02 - a23 * b00) * k,
            (-a10 * b09 + a11 * b07 - a12 * b06) * k,
            (a00 * b09 - a01 * b07 + a02 * b06) * k,
            (-a30 * b03 + a31 * b01 - a32 * b00) * k,
            (a20 * b03 - a21 * b01 + a22 * b00) * k,
        ];
    }

    pub fn reset(&mut self) {
        *self = Matrix::IDENTITY;
    }

    pub fn is_identity(&self) -> bool {
        *self == Matrix::IDENTITY
    }

    /// After what it does: columns 1 and 2 mixed.
    pub fn rotate_x(&mut self, degrees: f32) {
        let (s, c) = sin_cos(degrees);
        for row in 0..4 {
            let (a1, a2) = (self.get(row, 1), self.get(row, 2));
            self.set(row, 1, a1 * c - a2 * s);
            self.set(row, 2, a1 * s + a2 * c);
        }
    }

    /// After what it does: columns 0 and 2 mixed.
    pub fn rotate_y(&mut self, degrees: f32) {
        let (s, c) = sin_cos(degrees);
        for row in 0..4 {
            let (a0, a2) = (self.get(row, 0), self.get(row, 2));
            self.set(row, 0, a0 * c + a2 * s);
            self.set(row, 2, -a0 * s + a2 * c);
        }
    }

    /// Clockwise on the screen, before what it does: rows 0 and 1 mixed.
    pub fn rotate_z(&mut self, degrees: f32) {
        let (s, c) = sin_cos(degrees);
        for col in 0..4 {
            let (a0, a1) = (self.get(0, col), self.get(1, col));
            self.set(0, col, c * a0 + s * a1);
            self.set(1, col, -s * a0 + c * a1);
        }
    }

    pub fn scale(&mut self, x: f32, y: f32, z: f32) {
        for col in 0..4 {
            self.values[col] *= x;
            self.values[4 + col] *= y;
            self.values[8 + col] *= z;
        }
    }

    pub fn translate(&mut self, x: f32, y: f32, z: f32) {
        for col in 0..4 {
            let v = self.get(3, col) + self.get(0, col) * x + self.get(1, col) * y + self.get(2, col) * z;
            self.set(3, col, v);
        }
    }

    /// Scaled, then turned about x, y and z, then moved, all about the pivot (what graphicsLayer does).
    #[allow(clippy::too_many_arguments)]
    pub fn reset_to_pivoted_transform(
        &mut self,
        pivot_x: f32,
        pivot_y: f32,
        translation_x: f32,
        translation_y: f32,
        translation_z: f32,
        rotation_x: f32,
        rotation_y: f32,
        rotation_z: f32,
        scale_x: f32,
        scale_y: f32,
        scale_z: f32,
    ) {
        let (sx, cx) = sin_cos(rotation_x);
        let v11 = cx;
        let mut v12 = sx;
        let v21 = -sx;
        let mut v22 = cx;
        let v31 = translation_y * cx - translation_z * sx;
        let mut v32 = translation_y * sx + translation_z * cx;
        let (sy, cy) = sin_cos(rotation_y);
        let mut v00 = cy;
        let mut v02 = -sy;
        let mut v10 = v12 * sy;
        v12 *= cy;
        let v20 = v22 * sy;
        v22 *= cy;
        let v30 = translation_x * cy + v32 * sy;
        v32 = -translation_x * sy + v32 * cy;
        let (sz, cz) = sin_cos(rotation_z);
        let a10 = v10;
        v10 = -sz * v00 + cz * v10;
        v00 = cz * v00 + sz * a10;
        let v01 = sz * v11;
        let v11 = v11 * cz;
        let a12 = v12;
        v12 = -sz * v02 + cz * a12;
        v02 = cz * v02 + sz * a12;
        let (v00, v01, v02) = (v00 * scale_x, v01 * scale_x, v02 * scale_x);
        let (v10, v11, v12) = (v10 * scale_y, v11 * scale_y, v12 * scale_y);
        let (v20, v21, v22) = (v20 * scale_z, v21 * scale_z, v22 * scale_z);
        self.values = [
            v00,
            v01,
            v02,
            0.0,
            v10,
            v11,
            v12,
            0.0,
            v20,
            v21,
            v22,
            0.0,
            -pivot_x * v00 - pivot_y * v10 + v30 + pivot_x,
            -pivot_x * v01 - pivot_y * v11 + v31 + pivot_y,
            -pivot_x * v02 - pivot_y * v12 + v32,
            1.0,
        ];
    }
}

impl std::ops::MulAssign<&Matrix> for Matrix {
    fn mul_assign(&mut self, o: &Matrix) {
        self.times_assign(o);
    }
}
