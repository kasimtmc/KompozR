//! Material Color Utilities' QuantizerWu.

use std::collections::HashMap;

/// Wu's color quantizer (Material Color Utilities' QuantizerWu): boxes of a 32-level RGB cube, cut where the
/// variance falls most, up to as many as asked; each box's mean color.
pub struct QuantizerWu {
    weights: Vec<i64>,
    moments_r: Vec<i64>,
    moments_g: Vec<i64>,
    moments_b: Vec<i64>,
    moments: Vec<f64>,
}

const SIDE: usize = 33;
const SIZE: usize = SIDE * SIDE * SIDE;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    Red,
    Green,
    Blue,
}

/// A box of the cube, from r0, g0, b0 (out) to r1, g1, b1 (in), and its volume.
#[derive(Clone, Copy, Default)]
struct Box {
    r0: usize,
    r1: usize,
    g0: usize,
    g1: usize,
    b0: usize,
    b1: usize,
    vol: usize,
}

fn index(r: usize, g: usize, b: usize) -> usize {
    (r * SIDE + g) * SIDE + b
}

fn red(c: u32) -> usize {
    (c >> 16 & 0xff) as usize
}
fn green(c: u32) -> usize {
    (c >> 8 & 0xff) as usize
}
fn blue(c: u32) -> usize {
    (c & 0xff) as usize
}

/// A box's sum of `m` (the cumulative moments).
fn volume<T: Copy + std::ops::Add<Output = T> + std::ops::Sub<Output = T>>(c: &Box, m: &[T]) -> T {
    m[index(c.r1, c.g1, c.b1)] - m[index(c.r1, c.g1, c.b0)] - m[index(c.r1, c.g0, c.b1)] + m[index(c.r1, c.g0, c.b0)]
        - m[index(c.r0, c.g1, c.b1)]
        + m[index(c.r0, c.g1, c.b0)]
        + m[index(c.r0, c.g0, c.b1)]
        - m[index(c.r0, c.g0, c.b0)]
}

/// The part of a box's sum below its lower bound on `axis`.
fn bottom(c: &Box, axis: Axis, m: &[i64]) -> i64 {
    match axis {
        Axis::Red => {
            -m[index(c.r0, c.g1, c.b1)] + m[index(c.r0, c.g1, c.b0)] + m[index(c.r0, c.g0, c.b1)]
                - m[index(c.r0, c.g0, c.b0)]
        }
        Axis::Green => {
            -m[index(c.r1, c.g0, c.b1)] + m[index(c.r1, c.g0, c.b0)] + m[index(c.r0, c.g0, c.b1)]
                - m[index(c.r0, c.g0, c.b0)]
        }
        Axis::Blue => {
            -m[index(c.r1, c.g1, c.b0)] + m[index(c.r1, c.g0, c.b0)] + m[index(c.r0, c.g1, c.b0)]
                - m[index(c.r0, c.g0, c.b0)]
        }
    }
}

/// The part of a box's sum up to `at` on `axis`, from its upper bounds on the others.
fn top(c: &Box, axis: Axis, at: usize, m: &[i64]) -> i64 {
    match axis {
        Axis::Red => {
            m[index(at, c.g1, c.b1)] - m[index(at, c.g1, c.b0)] - m[index(at, c.g0, c.b1)] + m[index(at, c.g0, c.b0)]
        }
        Axis::Green => {
            m[index(c.r1, at, c.b1)] - m[index(c.r1, at, c.b0)] - m[index(c.r0, at, c.b1)] + m[index(c.r0, at, c.b0)]
        }
        Axis::Blue => {
            m[index(c.r1, c.g1, at)] - m[index(c.r1, c.g0, at)] - m[index(c.r0, c.g1, at)] + m[index(c.r0, c.g0, at)]
        }
    }
}

impl QuantizerWu {
    /// Up to `max_colors` colors (opaque ARGB) of the pixels' colors and their counts.
    pub fn quantize(counts: &HashMap<u32, u32>, max_colors: usize) -> Vec<u32> {
        let mut wu = QuantizerWu {
            weights: vec![0; SIZE],
            moments_r: vec![0; SIZE],
            moments_g: vec![0; SIZE],
            moments_b: vec![0; SIZE],
            moments: vec![0.0; SIZE],
        };
        for (&c, &n) in counts {
            let i = index((red(c) >> 3) + 1, (green(c) >> 3) + 1, (blue(c) >> 3) + 1);
            let (r, g, b) = (red(c) as i64, green(c) as i64, blue(c) as i64);
            wu.weights[i] += n as i64;
            wu.moments_r[i] += n as i64 * r;
            wu.moments_g[i] += n as i64 * g;
            wu.moments_b[i] += n as i64 * b;
            wu.moments[i] += n as f64 * (r * r + g * g + b * b) as f64;
        }
        wu.compute_moments();
        let boxes = wu.create_boxes(max_colors);
        let mut colors = Vec::new();
        for b in &boxes {
            let w = volume(b, &wu.weights);
            if w <= 0 {
                continue;
            }
            let mean = |m: &[i64]| (volume(b, m) / w).clamp(0, 255) as u32;
            colors.push(0xff00_0000 | mean(&wu.moments_r) << 16 | mean(&wu.moments_g) << 8 | mean(&wu.moments_b));
        }
        colors
    }

    /// Each cell becomes the sum of the cells at or below it on every axis.
    fn compute_moments(&mut self) {
        for r in 1..SIDE {
            let mut area = [0i64; SIDE];
            let mut area_r = [0i64; SIDE];
            let mut area_g = [0i64; SIDE];
            let mut area_b = [0i64; SIDE];
            let mut area2 = [0f64; SIDE];
            for g in 1..SIDE {
                let (mut line, mut line_r, mut line_g, mut line_b) = (0i64, 0i64, 0i64, 0i64);
                let mut line2 = 0f64;
                for b in 1..SIDE {
                    let i = index(r, g, b);
                    let below = index(r - 1, g, b);
                    line += self.weights[i];
                    line_r += self.moments_r[i];
                    line_g += self.moments_g[i];
                    line_b += self.moments_b[i];
                    line2 += self.moments[i];
                    area[b] += line;
                    area_r[b] += line_r;
                    area_g[b] += line_g;
                    area_b[b] += line_b;
                    area2[b] += line2;
                    self.weights[i] = self.weights[below] + area[b];
                    self.moments_r[i] = self.moments_r[below] + area_r[b];
                    self.moments_g[i] = self.moments_g[below] + area_g[b];
                    self.moments_b[i] = self.moments_b[below] + area_b[b];
                    self.moments[i] = self.moments[below] + area2[b];
                }
            }
        }
    }

    fn variance(&self, c: &Box) -> f64 {
        let r = volume(c, &self.moments_r) as f64;
        let g = volume(c, &self.moments_g) as f64;
        let b = volume(c, &self.moments_b) as f64;
        volume(c, &self.moments) - (r * r + g * g + b * b) / volume(c, &self.weights) as f64
    }

    /// Where along `axis` cutting `c` leaves the two halves' means farthest apart, and how far; None: nowhere.
    #[allow(clippy::too_many_arguments)]
    fn maximize(
        &self,
        c: &Box,
        axis: Axis,
        first: usize,
        last: usize,
        whole_r: i64,
        whole_g: i64,
        whole_b: i64,
        whole_w: i64,
    ) -> (Option<usize>, f64) {
        let bottom_r = bottom(c, axis, &self.moments_r);
        let bottom_g = bottom(c, axis, &self.moments_g);
        let bottom_b = bottom(c, axis, &self.moments_b);
        let bottom_w = bottom(c, axis, &self.weights);
        let mut best = 0.0;
        let mut cut = None;
        for i in first..last {
            let mut half_r = (bottom_r + top(c, axis, i, &self.moments_r)) as f64;
            let mut half_g = (bottom_g + top(c, axis, i, &self.moments_g)) as f64;
            let mut half_b = (bottom_b + top(c, axis, i, &self.moments_b)) as f64;
            let mut half_w = (bottom_w + top(c, axis, i, &self.weights)) as f64;
            if half_w == 0.0 {
                continue;
            }
            let mut score = (half_r * half_r + half_g * half_g + half_b * half_b) / half_w;
            half_r = whole_r as f64 - half_r;
            half_g = whole_g as f64 - half_g;
            half_b = whole_b as f64 - half_b;
            half_w = whole_w as f64 - half_w;
            if half_w == 0.0 {
                continue;
            }
            score += (half_r * half_r + half_g * half_g + half_b * half_b) / half_w;
            if score > best {
                best = score;
                cut = Some(i);
            }
        }
        (cut, best)
    }

    /// Cuts `one` in two (`one` and the returned box) where it is best cut; None where it cannot be.
    fn cut(&self, one: &mut Box) -> Option<Box> {
        let whole_r = volume(one, &self.moments_r);
        let whole_g = volume(one, &self.moments_g);
        let whole_b = volume(one, &self.moments_b);
        let whole_w = volume(one, &self.weights);
        let (cut_r, max_r) = self.maximize(one, Axis::Red, one.r0 + 1, one.r1, whole_r, whole_g, whole_b, whole_w);
        let (cut_g, max_g) = self.maximize(one, Axis::Green, one.g0 + 1, one.g1, whole_r, whole_g, whole_b, whole_w);
        let (cut_b, max_b) = self.maximize(one, Axis::Blue, one.b0 + 1, one.b1, whole_r, whole_g, whole_b, whole_w);
        let axis = if max_r >= max_g && max_r >= max_b {
            cut_r?;
            Axis::Red
        } else if max_g >= max_r && max_g >= max_b {
            Axis::Green
        } else {
            Axis::Blue
        };
        let mut two = Box { r1: one.r1, g1: one.g1, b1: one.b1, ..Box::default() };
        match axis {
            Axis::Red => {
                one.r1 = cut_r?;
                (two.r0, two.g0, two.b0) = (one.r1, one.g0, one.b0);
            }
            Axis::Green => {
                one.g1 = cut_g?;
                (two.r0, two.g0, two.b0) = (one.r0, one.g1, one.b0);
            }
            Axis::Blue => {
                one.b1 = cut_b?;
                (two.r0, two.g0, two.b0) = (one.r0, one.g0, one.b1);
            }
        }
        one.vol = (one.r1 - one.r0) * (one.g1 - one.g0) * (one.b1 - one.b0);
        two.vol = (two.r1 - two.r0) * (two.g1 - two.g0) * (two.b1 - two.b0);
        Some(two)
    }

    fn create_boxes(&self, max_colors: usize) -> Vec<Box> {
        let mut boxes = vec![Box::default(); max_colors];
        let mut variances = vec![0.0; max_colors];
        boxes[0] = Box { r0: 0, r1: SIDE - 1, g0: 0, g1: SIDE - 1, b0: 0, b1: SIDE - 1, vol: 0 };
        let mut count = max_colors;
        let mut next = 0;
        let mut i = 1;
        while i < max_colors {
            let mut one = boxes[next];
            if let Some(two) = self.cut(&mut one) {
                boxes[next] = one;
                boxes[i] = two;
                variances[next] = if one.vol > 1 { self.variance(&one) } else { 0.0 };
                variances[i] = if two.vol > 1 { self.variance(&two) } else { 0.0 };
            } else {
                variances[next] = 0.0;
                i -= 1;
            }
            next = 0;
            let mut most = variances[0];
            for (k, &v) in variances.iter().enumerate().take(i + 1).skip(1) {
                if v > most {
                    most = v;
                    next = k;
                }
            }
            if most <= 0.0 {
                count = i + 1;
                break;
            }
            i += 1;
        }
        boxes.truncate(count);
        boxes
    }
}
