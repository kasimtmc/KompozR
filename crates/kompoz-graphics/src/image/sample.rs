//! What size an image is decoded at for the size it is wanted at, and scaling one down.

use super::bitmap::ImageBitmap;

/// The size an image is wanted at, in pixels (0: any), as Coil's size and scale: a bigger image may be decoded
/// smaller, by a power of two, but never below it, still filling it one way (`fill` false, as a fitted image does)
/// or both ways (`fill` true, as a cropped one).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct ImageRequestSize {
    pub width: i32,
    pub height: i32,
    pub fill: bool,
}

impl ImageRequestSize {
    pub const fn new(width: i32, height: i32, fill: bool) -> ImageRequestSize {
        ImageRequestSize { width, height, fill }
    }

    /// No size: the whole image wanted.
    pub const fn any(&self) -> bool {
        self.width <= 0 && self.height <= 0
    }
}

/// The most pixels an image is decoded to each way, as a texture can hold.
pub const MAX_IMAGE_DIMENSION: i32 = 4096;

/// How much the image would be scaled to meet the size (below 1: it is bigger than needed).
fn scale_for(width: i32, height: i32, size: ImageRequestSize) -> f64 {
    let (sx, sy) = (size.width as f64 / width as f64, size.height as f64 / height as f64);
    if size.any() {
        1.0
    } else if size.width <= 0 {
        sy
    } else if size.height <= 0 {
        sx
    } else if size.fill {
        sx.max(sy)
    } else {
        sx.min(sy)
    }
}

/// How much a `width` x `height` image can be scaled down for `size`: 1, 2, 4, ... And at least so far that it is
/// at most [`MAX_IMAGE_DIMENSION`] pixels each way.
pub fn image_sample_size(width: i32, height: i32, size: ImageRequestSize) -> i32 {
    if width <= 0 || height <= 0 {
        return 1;
    }
    let scale = scale_for(width, height, size);
    let mut sample: i32 = 1;
    // 1/(2 sample) of it is still enough.
    while sample < 1 << 16 && scale * sample as f64 * 2.0 <= 1.0 {
        sample *= 2;
    }
    let over = |n: i32, sample: i32| (n as i64 + sample as i64 - 1) / sample as i64 > MAX_IMAGE_DIMENSION as i64;
    while sample < 1 << 16 && (over(width, sample) || over(height, sample)) {
        sample *= 2;
    }
    sample
}

/// Whether a `width` x `height` image is big enough for `size` (never for any size: that one wants the whole
/// image).
pub fn image_covers(width: i32, height: i32, size: ImageRequestSize) -> bool {
    width > 0 && height > 0 && !size.any() && scale_for(width, height, size) <= 1.0 + 1e-9
}

/// The image 1/`sample` the size (rounded up), each pixel the average of the ones it covers, weighted by their
/// alpha (a transparent one adds no color): for decoders that can't scale down as they decode. A sample of 1 or
/// less: the image itself.
pub fn scale_down(image: &ImageBitmap, sample: i32) -> ImageBitmap {
    if sample <= 1 {
        return image.clone();
    }
    let (w, h) = (image.width() as usize, image.height() as usize);
    scale_down_rgba(w, h, &image.pixels(), sample as usize)
        .and_then(|(ow, oh, out)| ImageBitmap::from_pixels(ow as i32, oh as i32, out))
        .unwrap_or_else(|| image.clone())
}

/// `w` x `h` straight RGBA pixels 1/`sample` the size, as [`scale_down`] makes them.
pub(super) fn scale_down_rgba(w: usize, h: usize, input: &[u8], sample: usize) -> Option<(usize, usize, Vec<u8>)> {
    if w == 0 || h == 0 || sample == 0 || input.len() < w * h * 4 {
        return None;
    }
    let (ow, oh) = (w.div_ceil(sample), h.div_ceil(sample));
    let mut out = vec![0u8; ow * oh * 4];
    for oy in 0..oh {
        for ox in 0..ow {
            let (mut r, mut g, mut b, mut a, mut n) = (0u64, 0u64, 0u64, 0u64, 0u64);
            for y in oy * sample..h.min((oy + 1) * sample) {
                for x in ox * sample..w.min((ox + 1) * sample) {
                    let p = &input[(y * w + x) * 4..(y * w + x) * 4 + 4];
                    let pa = p[3] as u64;
                    r += p[0] as u64 * pa;
                    g += p[1] as u64 * pa;
                    b += p[2] as u64 * pa;
                    a += pa;
                    n += 1;
                }
            }
            let q = &mut out[(oy * ow + ox) * 4..(oy * ow + ox) * 4 + 4];
            // (No alpha at all: no color.)
            let color = |c: u64| (c + a / 2).checked_div(a).unwrap_or(0) as u8;
            q[0] = color(r);
            q[1] = color(g);
            q[2] = color(b);
            q[3] = ((a + n / 2) / n) as u8;
        }
    }
    Some((ow, oh, out))
}
