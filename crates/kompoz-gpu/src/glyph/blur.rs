//! Text shadows' glyphs: coverage blurred on the CPU as the C++ Kompoz blurred it (raster.hpp's blurCoverage, used
//! by font_system.cpp's blurredGlyph): a Gaussian of Skia's sigma for the blur radius, made of three box blurs each
//! way, over the coverage padded by three sigma.

use super::raster::Coverage;

/// Skia's (and Android's) sigma for a blur radius in px: 0.57735 · radius + 0.5.
pub fn blur_sigma(radius: f32) -> f32 {
    0.57735 * radius + 0.5
}

/// How far a blur of `sigma` reaches past what it blurs: three sigma, up to a whole pixel.
pub fn blur_padding(sigma: f32) -> usize {
    (sigma * 3.0).ceil() as usize
}

/// The widths of the three box blurs approximating a Gaussian of `sigma`, as half widths (each box 2k + 1 wide):
/// the odd width below the ideal one for the first `m` passes, two wider for the others (blurCoverage's).
pub fn box_radii(sigma: f32) -> [usize; 3] {
    let ideal = (12.0 * sigma * sigma / 3.0 + 1.0).sqrt();
    let mut wl = ideal.floor() as i32;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let m = ((12.0 * sigma * sigma - 3.0 * (wl * wl) as f32 - 12.0 * wl as f32 - 9.0) / (-4.0 * wl as f32 - 4.0))
        .round() as i32;
    std::array::from_fn(|pass| (((if (pass as i32) < m { wl } else { wl + 2 }) - 1) / 2) as usize)
}

/// `a` (`width` × `height`, rows top down) blurred by a Gaussian of `sigma` in place: three box blurs across, then
/// down, each pass; beyond the edges as at them.
pub fn blur_coverage(a: &mut [f32], width: usize, height: usize, sigma: f32) {
    if width == 0 || height == 0 {
        return;
    }
    let mut t = vec![0.0f32; a.len()];
    let at = |v: &[f32], x: isize, y: isize| {
        v[y.clamp(0, height as isize - 1) as usize * width + x.clamp(0, width as isize - 1) as usize]
    };
    for k in box_radii(sigma) {
        let k = k as isize;
        let n = (2 * k + 1) as f32;
        for y in 0..height as isize {
            let mut sum = 0.0f32;
            for x in -k..=k {
                sum += at(a, x, y);
            }
            for x in 0..width as isize {
                t[y as usize * width + x as usize] = sum / n;
                sum += at(a, x + k + 1, y) - at(a, x - k, y);
            }
        }
        for x in 0..width as isize {
            let mut sum = 0.0f32;
            for y in -k..=k {
                sum += at(&t, x, y);
            }
            for y in 0..height as isize {
                a[y as usize * width + x as usize] = sum / n;
                sum += at(&t, x, y + k + 1) - at(&t, x, y - k);
            }
        }
    }
}

/// `coverage` blurred by `radius` px (a whole number, as C++ rounded it): padded by three sigma each way, blurred,
/// rounded back to bytes. As it is for a radius of 0 or nothing to blur.
pub fn blurred(coverage: &Coverage, radius: u32) -> Coverage {
    if radius == 0 || coverage.is_empty() {
        return coverage.clone();
    }
    let sigma = blur_sigma(radius as f32);
    let pad = blur_padding(sigma);
    let (w, h) = (coverage.width + 2 * pad, coverage.height + 2 * pad);
    let mut a = vec![0.0f32; w * h];
    for row in 0..coverage.height {
        for col in 0..coverage.width {
            a[(row + pad) * w + col + pad] = coverage.alpha[row * coverage.width + col] as f32 / 255.0;
        }
    }
    blur_coverage(&mut a, w, h, sigma);
    Coverage {
        left: coverage.left - pad as i32,
        top: coverage.top + pad as i32,
        width: w,
        height: h,
        alpha: a.iter().map(|&v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect(),
    }
}
