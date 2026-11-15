//! FreeType's anti-aliased rasterizer (`smooth`: ftgrays.c with ftsmooth.c's bitmap box), ported to give the same
//! bytes: an outline held as FreeType holds one (points in 26.6 fixed px, y up, tagged on the curve or a quadratic's
//! or a cubic's control), its curves cut as FreeType cuts them, its edges accumulated into cells of cover and area
//! with FreeType's integer arithmetic, swept into 8-bit coverage of the exact area of each pixel inside it, by the
//! non-zero or the even-odd rule. A glyph whose contours overlap is rendered four times finer each way and averaged,
//! as FreeType renders one.
//!
//! FreeType is under the FreeType License (or the GPL); this follows its code.

/// A point in 26.6 fixed px (64ths of a pixel), y up: FreeType's FT_Vector in an outline.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct FixedPoint {
    pub x: i32,
    pub y: i32,
}

impl FixedPoint {
    pub const fn new(x: i32, y: i32) -> FixedPoint {
        FixedPoint { x, y }
    }

    /// The point nearest (x, y) px.
    pub fn from_px(x: f32, y: f32) -> FixedPoint {
        FixedPoint { x: (x * 64.0).round() as i32, y: (y * 64.0).round() as i32 }
    }
}

/// What a point of an outline is (FreeType's curve tags): on the curve, a quadratic's control (two in a row have
/// an on-curve point between them), or one of a cubic's two controls.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PointTag {
    On,
    Conic,
    Cubic,
}

/// Which points an outline covers: those its contours wind round a number of times other than zero, or an odd
/// number of times.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

/// A 2×2 matrix in 16.16 fixed point (FreeType's FT_Matrix): x' = xx·x + xy·y, y' = yx·x + yy·y.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FixedMatrix {
    pub xx: i64,
    pub xy: i64,
    pub yx: i64,
    pub yy: i64,
}

impl FixedMatrix {
    pub const IDENTITY: FixedMatrix = FixedMatrix { xx: 0x10000, xy: 0, yx: 0, yy: 0x10000 };

    /// The matrix of these factors, each cut to 16.16 as C's conversion of `f · 65536` to an integer cuts it
    /// (towards zero: what C++'s `FT_Fixed(f * 65536)` gave FreeType).
    pub fn from_f32(xx: f32, xy: f32, yx: f32, yy: f32) -> FixedMatrix {
        let fixed = |v: f32| (v * 65536.0) as i64;
        FixedMatrix { xx: fixed(xx), xy: fixed(xy), yx: fixed(yx), yy: fixed(yy) }
    }

    pub fn is_identity(&self) -> bool {
        *self == FixedMatrix::IDENTITY
    }
}

/// a · b in 16.16, rounded half away from zero (FreeType's FT_MulFix).
pub fn mul_fix(a: i64, b: i64) -> i64 {
    let ab = a.wrapping_mul(b);
    (ab.wrapping_add(0x8000).wrapping_add(ab >> 63)) >> 16
}

/// The smallest box (26.6) holding every point of an outline, its controls too (FreeType's control box).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct FixedBox {
    pub x_min: i32,
    pub y_min: i32,
    pub x_max: i32,
    pub y_max: i32,
}

/// An outline as FreeType holds one (FT_Outline): its points with their tags, each contour by the index of its last
/// point, its fill rule, and whether its contours overlap (a glyph that says so is rendered four times finer).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct FixedOutline {
    pub points: Vec<FixedPoint>,
    pub tags: Vec<PointTag>,
    pub contours: Vec<usize>,
    pub fill_rule: FillRule,
    pub overlap: bool,
}

/// What an outline's decomposition emits (FreeType's FT_Outline_Funcs), in 26.6.
pub trait FixedSink {
    fn move_to(&mut self, to: FixedPoint);
    fn line_to(&mut self, to: FixedPoint);
    fn conic_to(&mut self, control: FixedPoint, to: FixedPoint);
    fn cubic_to(&mut self, control1: FixedPoint, control2: FixedPoint, to: FixedPoint);
}

impl FixedOutline {
    pub fn new() -> FixedOutline {
        FixedOutline::default()
    }

    /// Whether it has no points.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    fn push(&mut self, p: FixedPoint, tag: PointTag) {
        self.points.push(p);
        self.tags.push(tag);
    }

    /// Ends the contour being added, if it has points.
    pub fn close(&mut self) {
        let start = self.contours.last().map_or(0, |&end| end + 1);
        if self.points.len() > start {
            self.contours.push(self.points.len() - 1);
        }
    }

    /// Starts a contour at `p` (the one before it ended).
    pub fn move_to(&mut self, p: FixedPoint) {
        self.close();
        self.push(p, PointTag::On);
    }

    pub fn line_to(&mut self, p: FixedPoint) {
        self.push(p, PointTag::On);
    }

    pub fn conic_to(&mut self, control: FixedPoint, p: FixedPoint) {
        self.push(control, PointTag::Conic);
        self.push(p, PointTag::On);
    }

    pub fn cubic_to(&mut self, control1: FixedPoint, control2: FixedPoint, p: FixedPoint) {
        self.push(control1, PointTag::Cubic);
        self.push(control2, PointTag::Cubic);
        self.push(p, PointTag::On);
    }

    /// A closed polygon through `points`.
    pub fn add_polygon(&mut self, points: impl IntoIterator<Item = FixedPoint>) {
        self.close();
        for p in points {
            self.push(p, PointTag::On);
        }
        self.close();
    }

    /// `other`'s contours added after its own.
    pub fn append(&mut self, other: &FixedOutline) {
        self.close();
        let base = self.points.len();
        self.points.extend_from_slice(&other.points);
        self.tags.extend_from_slice(&other.tags);
        self.contours.extend(other.contours.iter().map(|&end| end + base));
        self.overlap |= other.overlap;
    }

    /// Its control box (None if it has no points).
    pub fn cbox(&self) -> Option<FixedBox> {
        let first = *self.points.first()?;
        let mut b = FixedBox { x_min: first.x, y_min: first.y, x_max: first.x, y_max: first.y };
        for p in &self.points[1..] {
            b.x_min = b.x_min.min(p.x);
            b.y_min = b.y_min.min(p.y);
            b.x_max = b.x_max.max(p.x);
            b.y_max = b.y_max.max(p.y);
        }
        Some(b)
    }

    /// Moved by (dx, dy) (26.6).
    pub fn translate(&mut self, dx: i32, dy: i32) {
        for p in &mut self.points {
            p.x = p.x.wrapping_add(dx);
            p.y = p.y.wrapping_add(dy);
        }
    }

    /// Each point through `m` (FreeType's FT_Outline_Transform).
    pub fn transform(&mut self, m: &FixedMatrix) {
        for p in &mut self.points {
            let (x, y) = (p.x as i64, p.y as i64);
            p.x = (mul_fix(x, m.xx) + mul_fix(y, m.xy)) as i32;
            p.y = (mul_fix(x, m.yx) + mul_fix(y, m.yy)) as i32;
        }
    }

    /// Its contours as moves, lines and curves into `sink`, as FreeType's FT_Outline_Decompose gives them: a contour
    /// starting on a control starts at its last point if that is on the curve, else halfway between the two; two
    /// quadratic controls in a row have their midpoint between them (C's integer halving); every contour is closed by
    /// a line back to its start. False (having emitted what came before) for an outline that is not one.
    pub fn decompose(&self, sink: &mut impl FixedSink) -> bool {
        if self.tags.len() != self.points.len() {
            return false;
        }
        let half = |a: FixedPoint, b: FixedPoint| {
            FixedPoint::new(((a.x as i64 + b.x as i64) / 2) as i32, ((a.y as i64 + b.y as i64) / 2) as i32)
        };
        let mut first = 0usize;
        for &last in &self.contours {
            if last < first || last >= self.points.len() {
                return false;
            }
            let mut limit = last;
            let v_last = self.points[last];
            let mut v_start = self.points[first];
            // The index of the point last consumed (one before `first` when the first point is a control).
            let mut point = first as isize;
            match self.tags[first] {
                PointTag::Cubic => return false,
                PointTag::Conic => {
                    if self.tags[last] == PointTag::On {
                        v_start = v_last;
                        if limit == 0 {
                            return false;
                        }
                        limit -= 1;
                    } else {
                        v_start = half(v_start, v_last);
                    }
                    point -= 1;
                }
                PointTag::On => {}
            }
            sink.move_to(v_start);
            let limit = limit as isize;
            let mut closed = false;
            while point < limit {
                point += 1;
                let i = point as usize;
                match self.tags[i] {
                    PointTag::On => sink.line_to(self.points[i]),
                    PointTag::Conic => {
                        let mut control = self.points[i];
                        loop {
                            if point < limit {
                                point += 1;
                                let j = point as usize;
                                let vec = self.points[j];
                                match self.tags[j] {
                                    PointTag::On => {
                                        sink.conic_to(control, vec);
                                        break;
                                    }
                                    PointTag::Conic => {
                                        sink.conic_to(control, half(control, vec));
                                        control = vec;
                                    }
                                    PointTag::Cubic => return false,
                                }
                            } else {
                                sink.conic_to(control, v_start);
                                closed = true;
                                break;
                            }
                        }
                        if closed {
                            break;
                        }
                    }
                    PointTag::Cubic => {
                        if point + 1 > limit || self.tags[i + 1] != PointTag::Cubic {
                            return false;
                        }
                        point += 2;
                        let (c1, c2) = (self.points[i], self.points[i + 1]);
                        if point <= limit {
                            sink.cubic_to(c1, c2, self.points[point as usize]);
                        } else {
                            sink.cubic_to(c1, c2, v_start);
                            closed = true;
                            break;
                        }
                    }
                }
            }
            if !closed {
                sink.line_to(v_start);
            }
            first = last + 1;
        }
        true
    }

    /// The pixel box FreeType gives its bitmap (ft_glyphslot_preset_bitmap for gray rendering): its control box's
    /// edges out to whole pixels. None for an outline with no points (an empty bitmap at the origin).
    pub fn pixel_box(&self) -> Option<PixelBox> {
        let c = self.cbox()?;
        let (x_min, y_min) = (c.x_min >> 6, c.y_min >> 6);
        let x_max = (c.x_max >> 6) + (((c.x_max & 63) + 63) >> 6);
        let y_max = (c.y_max >> 6) + (((c.y_max & 63) + 63) >> 6);
        Some(PixelBox { left: x_min, top: y_max, width: (x_max - x_min) as u32, height: (y_max - y_min) as u32 })
    }

    /// Its coverage in its pixel box, as FreeType renders an outline glyph (ft_smooth_render, gray): moved so its
    /// box's bottom left is the bitmap's origin, rasterized (four times finer and averaged when its contours
    /// overlap). Rows top down.
    pub fn render(&self) -> Coverage {
        let Some(b) = self.pixel_box() else {
            return Coverage::default();
        };
        if b.width == 0 || b.height == 0 || b.width >= 0x10000 || b.height >= 0x10000 {
            return Coverage { left: b.left, top: b.top, ..Coverage::default() };
        }
        let mut placed = self.clone();
        placed.translate(-b.left * 64, -(b.top - b.height as i32) * 64);
        let alpha = placed.rasterize(b.width as usize, b.height as usize);
        Coverage { left: b.left, top: b.top, width: b.width as usize, height: b.height as usize, alpha }
    }

    /// Its coverage of a `width` × `height` bitmap whose bottom left corner is the outline's origin (rows top down):
    /// what lies outside is left out.
    pub fn rasterize(&self, width: usize, height: usize) -> Vec<u8> {
        if width == 0 || height == 0 {
            return Vec::new();
        }
        if !self.overlap {
            let mut cells = Cells::new(width, height);
            if !self.decompose(&mut cells) {
                return vec![0; width * height];
            }
            return cells.sweep(self.fill_rule);
        }
        // Overlapping contours: four times finer each way (FreeType's ft_smooth_raster_overlap), each fine pixel's
        // coverage rounded to a sixteenth and added up, 256 cut to 255.
        const SCALE: usize = 4;
        if width * SCALE > 0x7FFF {
            return vec![0; width * height];
        }
        let mut fine = self.clone();
        for p in &mut fine.points {
            p.x = p.x.wrapping_mul(SCALE as i32);
            p.y = p.y.wrapping_mul(SCALE as i32);
        }
        let mut cells = Cells::new(width * SCALE, height * SCALE);
        let mut out = vec![0u8; width * height];
        if !fine.decompose(&mut cells) {
            return out;
        }
        let fine_alpha = cells.sweep(self.fill_rule);
        for (y, row) in fine_alpha.chunks_exact(width * SCALE).enumerate() {
            for (x, &c) in row.iter().enumerate() {
                let cover = (c as u32 + (SCALE * SCALE / 2) as u32) / (SCALE * SCALE) as u32;
                let d = &mut out[(y / SCALE) * width + x / SCALE];
                let sum = *d as u32 + cover;
                *d = (sum - (sum >> 8)) as u8;
            }
        }
        out
    }
}

/// Where a bitmap goes from a glyph's origin, in whole pixels: its left edge, its top edge (y up), its size.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct PixelBox {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

/// 8-bit coverage of a bitmap placed from a glyph's origin: `left` px to the right, its top `top` px above (y up, as
/// FreeType's bitmap_left and bitmap_top), `width` × `height`, rows top down.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Coverage {
    pub left: i32,
    pub top: i32,
    pub width: usize,
    pub height: usize,
    pub alpha: Vec<u8>,
}

impl Coverage {
    /// Whether it covers no pixel (no size, or nothing above zero).
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// The coverage at (x, y) (0 outside).
    pub fn at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return 0;
        }
        self.alpha[y as usize * self.width + x as usize]
    }
}

// ---- The cells (ftgrays.c) ----------------------------------------------------------------------------------------

/// Subpixel bits of a cell's coordinates.
const PIXEL_BITS: i32 = 8;
const ONE_PIXEL: i64 = 1 << PIXEL_BITS;

fn upscale(v: i32) -> i64 {
    v as i64 * (ONE_PIXEL >> 6)
}
fn trunc(v: i64) -> i32 {
    (v >> PIXEL_BITS) as i32
}
fn fract(v: i64) -> i32 {
    (v & (ONE_PIXEL - 1)) as i32
}

/// FreeType's FT_UDIV: a / b by the reciprocal `b_r` (0xFFFFFFFF / b), the product's top half.
fn udiv(a: i64, b_r: i64) -> i32 {
    ((a as u64).wrapping_mul(b_r as u64) >> 32) as i32
}

/// The edges' cells over a bitmap, each pixel's cover (how far edges cross it vertically, signed) and area (cover
/// times twice the distance from the pixel's left), one column left of the bitmap for what lies left of it; the
/// pen's position and its cell, as FreeType's gray worker keeps them.
struct Cells {
    width: usize,
    height: usize,
    cover: Vec<i32>,
    area: Vec<i32>,
    /// The current cell; None when it lies outside (FreeType's null cell).
    cell: Option<usize>,
    x: i64,
    y: i64,
}

impl Cells {
    fn new(width: usize, height: usize) -> Cells {
        let n = (width + 1) * height;
        Cells { width, height, cover: vec![0; n], area: vec![0; n], cell: None, x: 0, y: 0 }
    }

    /// FreeType's gray_set_cell: cells left of the bitmap gather in its column -1, others outside go nowhere.
    fn set_cell(&mut self, ex: i32, ey: i32) {
        self.cell = if ey < 0 || ey as usize >= self.height || (ex >= 0 && ex as usize >= self.width) {
            None
        } else {
            let column = (ex.max(-1) + 1) as usize;
            Some(ey as usize * (self.width + 1) + column)
        };
    }

    /// FreeType's FT_INTEGRATE.
    fn integrate(&mut self, cover: i32, area: i32) {
        if let Some(i) = self.cell {
            self.cover[i] = self.cover[i].wrapping_add(cover);
            self.area[i] = self.area[i].wrapping_add(cover.wrapping_mul(area));
        }
    }

    /// FreeType's gray_render_line (its 64-bit version): a line from the pen to (to_x, to_y), cell by cell.
    fn render_line(&mut self, to_x: i64, to_y: i64) {
        let (mut ey1, ey2) = (trunc(self.y), trunc(to_y));
        let max_ey = self.height as i32;
        if (ey1 >= max_ey && ey2 >= max_ey) || (ey1 < 0 && ey2 < 0) {
            self.x = to_x;
            self.y = to_y;
            return;
        }
        let (mut ex1, ex2) = (trunc(self.x), trunc(to_x));
        let (mut fx1, mut fy1) = (fract(self.x), fract(self.y));
        let (dx, dy) = (to_x - self.x, to_y - self.y);
        let one = ONE_PIXEL as i32;
        if ex1 == ex2 && ey1 == ey2 {
            // Inside one cell.
        } else if dy == 0 {
            // Horizontal: no cover, the pen's cell moves on.
            self.set_cell(ex2, ey2);
            self.x = to_x;
            self.y = to_y;
            return;
        } else if dx == 0 {
            if dy > 0 {
                loop {
                    self.integrate(one - fy1, fx1 * 2);
                    fy1 = 0;
                    ey1 += 1;
                    self.set_cell(ex1, ey1);
                    if ey1 == ey2 {
                        break;
                    }
                }
            } else {
                loop {
                    self.integrate(-fy1, fx1 * 2);
                    fy1 = one;
                    ey1 -= 1;
                    self.set_cell(ex1, ey1);
                    if ey1 == ey2 {
                        break;
                    }
                }
            }
        } else {
            // Where the line leaves each cell, from `prod`: which side, and the exact place.
            let mut prod = dx * fy1 as i64 - dy * fx1 as i64;
            let dx_r = if ex1 != ex2 { 0xFFFF_FFFF / dx } else { 0 };
            let dy_r = if ey1 != ey2 { 0xFFFF_FFFF / dy } else { 0 };
            loop {
                if prod - dx * ONE_PIXEL > 0 && prod <= 0 {
                    // Out through the left.
                    let fy2 = udiv(-prod, -dx_r);
                    prod -= dy * ONE_PIXEL;
                    self.integrate(fy2 - fy1, fx1);
                    fx1 = one;
                    fy1 = fy2;
                    ex1 -= 1;
                } else if prod - dx * ONE_PIXEL + dy * ONE_PIXEL > 0 && prod - dx * ONE_PIXEL <= 0 {
                    // Up.
                    prod -= dx * ONE_PIXEL;
                    let fx2 = udiv(-prod, dy_r);
                    self.integrate(one - fy1, fx1 + fx2);
                    fx1 = fx2;
                    fy1 = 0;
                    ey1 += 1;
                } else if prod + dy * ONE_PIXEL >= 0 && prod - dx * ONE_PIXEL + dy * ONE_PIXEL <= 0 {
                    // Right.
                    prod += dy * ONE_PIXEL;
                    let fy2 = udiv(prod, dx_r);
                    self.integrate(fy2 - fy1, fx1 + one);
                    fx1 = 0;
                    fy1 = fy2;
                    ex1 += 1;
                } else {
                    // Down.
                    let fx2 = udiv(prod, -dy_r);
                    prod += dx * ONE_PIXEL;
                    self.integrate(-fy1, fx1 + fx2);
                    fx1 = fx2;
                    fy1 = one;
                    ey1 -= 1;
                }
                self.set_cell(ex1, ey1);
                if ex1 == ex2 && ey1 == ey2 {
                    break;
                }
            }
        }
        let (fx2, fy2) = (fract(to_x), fract(to_y));
        self.integrate(fy2 - fy1, fx1 + fx2);
        self.x = to_x;
        self.y = to_y;
    }

    /// FreeType's gray_render_conic: a quadratic as 2^n lines by forward differences in 64-bit fixed point, n such
    /// that the curve's deviation from them is at most a sixteenth of a pixel.
    fn render_conic(&mut self, control: FixedPoint, to: FixedPoint) {
        let (p0x, p0y) = (self.x, self.y);
        let (p1x, p1y) = (upscale(control.x), upscale(control.y));
        let (p2x, p2y) = (upscale(to.x), upscale(to.y));
        let max_ey = self.height as i32;
        if (trunc(p0y) >= max_ey && trunc(p1y) >= max_ey && trunc(p2y) >= max_ey)
            || (trunc(p0y) < 0 && trunc(p1y) < 0 && trunc(p2y) < 0)
        {
            self.x = p2x;
            self.y = p2y;
            return;
        }
        let (bx, by) = (p1x - p0x, p1y - p0y);
        let (ax, ay) = (p2x - p1x - bx, p2y - p1y - by);
        let mut d = ax.abs().max(ay.abs());
        if d <= ONE_PIXEL / 4 {
            self.render_line(p2x, p2y);
            return;
        }
        let mut shift = 16u32;
        loop {
            d >>= 2;
            shift -= 1;
            if d <= ONE_PIXEL / 4 {
                break;
            }
        }
        let mut count = 0x10000u32 >> shift;
        let left_shift = |a: i64, b: u32| ((a as u64).wrapping_shl(b)) as i64;
        let mut rx = left_shift(ax, shift + shift);
        let mut ry = left_shift(ay, shift + shift);
        let mut qx = left_shift(bx, shift + 17).wrapping_add(rx);
        let mut qy = left_shift(by, shift + 17).wrapping_add(ry);
        rx = rx.wrapping_mul(2);
        ry = ry.wrapping_mul(2);
        let mut px = left_shift(p0x, 32);
        let mut py = left_shift(p0y, 32);
        loop {
            px = px.wrapping_add(qx);
            py = py.wrapping_add(qy);
            qx = qx.wrapping_add(rx);
            qy = qy.wrapping_add(ry);
            self.render_line(px >> 32, py >> 32);
            count -= 1;
            if count == 0 {
                break;
            }
        }
    }

    /// FreeType's gray_render_cubic: a cubic split in halves until its controls lie within half a pixel of its
    /// chord's trisection points, each piece then a line.
    fn render_cubic(&mut self, control1: FixedPoint, control2: FixedPoint, to: FixedPoint) {
        // Pieces still to draw, the next one last: [end, control2, control1, start] each (shared ends).
        let mut stack = [(0i64, 0i64); 16 * 3 + 1];
        stack[0] = (upscale(to.x), upscale(to.y));
        stack[1] = (upscale(control2.x), upscale(control2.y));
        stack[2] = (upscale(control1.x), upscale(control1.y));
        stack[3] = (self.x, self.y);
        let max_ey = self.height as i32;
        if stack[..4].iter().all(|p| trunc(p.1) >= max_ey) || stack[..4].iter().all(|p| trunc(p.1) < 0) {
            self.x = stack[0].0;
            self.y = stack[0].1;
            return;
        }
        let mut arc = 0usize;
        loop {
            let a = &stack[arc..arc + 4];
            let flat = (2 * a[0].0 - 3 * a[1].0 + a[3].0).abs() <= ONE_PIXEL / 2
                && (2 * a[0].1 - 3 * a[1].1 + a[3].1).abs() <= ONE_PIXEL / 2
                && (a[0].0 - 3 * a[2].0 + 2 * a[3].0).abs() <= ONE_PIXEL / 2
                && (a[0].1 - 3 * a[2].1 + 2 * a[3].1).abs() <= ONE_PIXEL / 2;
            if flat || arc + 6 >= stack.len() {
                let (x, y) = stack[arc];
                self.render_line(x, y);
                if arc == 0 {
                    return;
                }
                arc -= 3;
            } else {
                split_cubic(&mut stack[arc..arc + 7]);
                arc += 3;
            }
        }
    }

    /// FreeType's gray_sweep: each row's cells left to right, the cover so far giving the pixels between them and
    /// cover less area each cell's own, through the fill rule (FT_FILL_RULE).
    fn sweep(&self, fill_rule: FillRule) -> Vec<u8> {
        let fill: i32 = match fill_rule {
            FillRule::NonZero => i32::MIN,
            FillRule::EvenOdd => 0x100,
        };
        let coverage = |area: i32| -> u8 {
            let mut c = area >> (PIXEL_BITS * 2 + 1 - 8);
            if c & fill != 0 {
                c = !c;
            }
            if c > 255 && fill & i32::MIN != 0 {
                c = 255;
            }
            c as u8
        };
        let mut out = vec![0u8; self.width * self.height];
        for y in 0..self.height {
            // (FreeType's row y counts up from the bitmap's bottom.)
            let row = &mut out[(self.height - 1 - y) * self.width..(self.height - y) * self.width];
            let cells = y * (self.width + 1);
            let mut cover: i32 = 0;
            for column in 0..=self.width {
                let i = cells + column;
                cover = cover.wrapping_add(self.cover[i].wrapping_mul((ONE_PIXEL * 2) as i32));
                if column > 0 {
                    row[column - 1] = coverage(cover.wrapping_sub(self.area[i]));
                }
            }
        }
        out
    }
}

/// FreeType's gray_split_cubic: the cubic at base[0..4] (end first) as two, at base[0..4] and base[3..7].
fn split_cubic(base: &mut [(i64, i64)]) {
    for axis in 0..2 {
        let get = |b: &[(i64, i64)], i: usize| if axis == 0 { b[i].0 } else { b[i].1 };
        let set = |b: &mut [(i64, i64)], i: usize, v: i64| {
            if axis == 0 { b[i].0 = v } else { b[i].1 = v }
        };
        let end = get(base, 3);
        set(base, 6, end);
        let mut a = get(base, 0) + get(base, 1);
        let b = get(base, 1) + get(base, 2);
        let mut c = get(base, 2) + get(base, 3);
        set(base, 5, c >> 1);
        c += b;
        set(base, 4, c >> 2);
        set(base, 1, a >> 1);
        a += b;
        set(base, 2, a >> 2);
        set(base, 3, (a + c) >> 3);
    }
}

impl FixedSink for Cells {
    fn move_to(&mut self, to: FixedPoint) {
        let (x, y) = (upscale(to.x), upscale(to.y));
        self.set_cell(trunc(x), trunc(y));
        self.x = x;
        self.y = y;
    }

    fn line_to(&mut self, to: FixedPoint) {
        self.render_line(upscale(to.x), upscale(to.y));
    }

    fn conic_to(&mut self, control: FixedPoint, to: FixedPoint) {
        self.render_conic(control, to);
    }

    fn cubic_to(&mut self, control1: FixedPoint, control2: FixedPoint, to: FixedPoint) {
        self.render_cubic(control1, control2, to);
    }
}
