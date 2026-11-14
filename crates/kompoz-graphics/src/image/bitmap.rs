//! Bitmaps (Compose's ImageBitmap) and pixels read from them (Compose's PixelMap).

use super::sample::MAX_IMAGE_DIMENSION;
use crate::color::Color;
use crate::colorspace::{ColorSpace, ColorSpaces, MIN_ID, Rgb};
use kompoz_runtime::named;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// How a bitmap keeps its pixels (Compose's ImageBitmapConfig, Android's Bitmap.Config): 8 bits a channel
/// (Argb8888; Gpu: the same, kept by the GPU), alpha alone (Alpha8: drawn in a paint's color as a mask), 5, 6 and 5
/// bits of red, green and blue and no alpha (Rgb565), half floats (F16: more than 8 bits, and past 0..1).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum ImageBitmapConfig {
    #[default]
    Argb8888,
    Alpha8,
    Rgb565,
    F16,
    Gpu,
}

/// A bitmap's color space: a built-in one, or its own copy of another.
enum BitmapColorSpace {
    BuiltIn(&'static Rgb),
    Own(Box<Rgb>),
}

/// What a bitmap holds now, and what its CPU copy holds.
struct Pixels {
    /// What it holds: another each time it is drawn into.
    version: u64,
    /// What `rgba` (and `floats`) hold.
    cpu_version: u64,
    /// Straight RGBA, 8 bits a channel, as its config keeps them (Alpha8: black; Rgb565: 5-6-5 values).
    rgba: Arc<Vec<u8>>,
    /// F16's, straight RGBA, at half floats' precision.
    floats: Arc<Vec<f32>>,
}

struct BitmapData {
    width: i32,
    height: i32,
    config: ImageBitmapConfig,
    has_alpha: bool,
    color_space: BitmapColorSpace,
    id: u64,
    pixels: Mutex<Pixels>,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A bitmap (Compose's ImageBitmap): decoded pixels ([`decode_image`](super::decode_image),
/// [`from_pixels`](ImageBitmap::from_pixels)), or pixels to draw into ([`ImageBitmap::new`], then a canvas over
/// it). Clones are the same bitmap: what is drawn into one shows in all; two are equal when they are the same one.
/// Its pixels are kept in sRGB, as they are shown, whatever color space it says it is in.
#[derive(Clone)]
pub struct ImageBitmap {
    data: Arc<BitmapData>,
}

impl ImageBitmap {
    /// Its pixels clear (black and opaque where it has no alpha: Rgb565, or `has_alpha` false, as Android clears
    /// an opaque bitmap), `config` as it keeps them, in `color_space` (Compose's ImageBitmap(width, height, config,
    /// hasAlpha, colorSpace): an RGB one, as Android's bitmaps are). Panics, as Compose throws, for a width or height
    /// of 0 or less; also for one over [`MAX_IMAGE_DIMENSION`], more than a renderer holds (see
    /// [`try_new`](ImageBitmap::try_new)).
    #[track_caller]
    pub fn new(width: i32, height: i32, config: ImageBitmapConfig, has_alpha: bool, color_space: &Rgb) -> ImageBitmap {
        match ImageBitmap::try_new(width, height, config, has_alpha, color_space) {
            Some(bitmap) => bitmap,
            None => {
                panic!("an ImageBitmap's width and height must be 1 to {MAX_IMAGE_DIMENSION}, not {width}x{height}")
            }
        }
    }

    /// The same, none for a width or height outside 1..=[`MAX_IMAGE_DIMENSION`].
    pub fn try_new(
        width: i32,
        height: i32,
        config: ImageBitmapConfig,
        has_alpha: bool,
        color_space: &Rgb,
    ) -> Option<ImageBitmap> {
        if !(1..=MAX_IMAGE_DIMENSION).contains(&width) || !(1..=MAX_IMAGE_DIMENSION).contains(&height) {
            return None;
        }
        let has_alpha = config == ImageBitmapConfig::Alpha8 || (config != ImageBitmapConfig::Rgb565 && has_alpha);
        let n = width as usize * height as usize * 4;
        let mut rgba = vec![0u8; n];
        let mut floats = Vec::new();
        if !has_alpha {
            rgba.iter_mut().skip(3).step_by(4).for_each(|a| *a = 255);
        }
        if config == ImageBitmapConfig::F16 {
            floats = vec![0.0f32; n];
            if !has_alpha {
                floats.iter_mut().skip(3).step_by(4).for_each(|a| *a = 1.0);
            }
        }
        Some(ImageBitmap::make(width, height, config, has_alpha, own_color_space(color_space), rgba, floats))
    }

    /// A bitmap of these pixels: straight RGBA, 8 bits a channel, the top row first, Argb8888 in sRGB. None for a
    /// width or height of 0 or less, or too few pixels.
    pub fn from_pixels(width: i32, height: i32, rgba: Vec<u8>) -> Option<ImageBitmap> {
        if width <= 0 || height <= 0 || rgba.len() < width as usize * height as usize * 4 {
            return None;
        }
        let mut rgba = rgba;
        rgba.truncate(width as usize * height as usize * 4);
        let space = BitmapColorSpace::BuiltIn(&ColorSpaces::Srgb);
        Some(ImageBitmap::make(width, height, ImageBitmapConfig::Argb8888, true, space, rgba, Vec::new()))
    }

    fn make(
        width: i32,
        height: i32,
        config: ImageBitmapConfig,
        has_alpha: bool,
        color_space: BitmapColorSpace,
        rgba: Vec<u8>,
        floats: Vec<f32>,
    ) -> ImageBitmap {
        let pixels = Pixels { version: 1, cpu_version: 1, rgba: Arc::new(rgba), floats: Arc::new(floats) };
        ImageBitmap {
            data: Arc::new(BitmapData {
                width,
                height,
                config,
                has_alpha,
                color_space,
                id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
                pixels: Mutex::new(pixels),
            }),
        }
    }

    pub fn width(&self) -> i32 {
        self.data.width
    }
    pub fn height(&self) -> i32 {
        self.data.height
    }
    pub fn config(&self) -> ImageBitmapConfig {
        self.data.config
    }
    /// Alpha8: yes; Rgb565: no.
    pub fn has_alpha(&self) -> bool {
        self.data.has_alpha
    }
    /// The color space it says it is in (sRGB unless made in another).
    pub fn color_space(&self) -> &Rgb {
        match &self.data.color_space {
            BitmapColorSpace::BuiltIn(space) => space,
            BitmapColorSpace::Own(space) => space,
        }
    }

    /// Its own, for as long as it lives (no other bitmap's).
    pub fn id(&self) -> u64 {
        self.data.id
    }

    /// What it holds: another each time it is drawn into.
    pub fn version(&self) -> u64 {
        self.lock().version
    }

    fn lock(&self) -> MutexGuard<'_, Pixels> {
        // (A panic while it was held left the pixels whole: each change is made at once.)
        self.data.pixels.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Its pixels as straight RGBA, 8 bits a channel, the top row first (Alpha8's black; F16's kept to 0..1): what
    /// its CPU copy holds now ([`pixels_current`](ImageBitmap::pixels_current) says whether that is all that was
    /// drawn into it).
    pub fn pixels(&self) -> Arc<Vec<u8>> {
        self.lock().rgba.clone()
    }

    /// F16's pixels as straight RGBA at half floats' precision (another config's: none).
    pub fn floats(&self) -> Arc<Vec<f32>> {
        self.lock().floats.clone()
    }

    /// Android's hint to upload it now (Compose's prepareToDraw): the renderer uploads it when it is drawn.
    pub fn prepare_to_draw(&self) {}

    /// Its CPU copy holds what it holds (nothing drawn into it by a renderer since that was read back).
    pub fn pixels_current(&self) -> bool {
        let p = self.lock();
        p.cpu_version == p.version
    }

    /// For renderers: what was drawn into it, premultiplied RGBA 8 bits a channel, kept as its config keeps it
    /// (straight; Alpha8 its alpha alone; Rgb565 over black, rounded to 5, 6 and 5 bits as Skia stores them; an
    /// opaque one's over black), and, if `drawn`, a new version. Too few pixels: nothing.
    pub fn store(&self, premultiplied: &[u8], drawn: bool) {
        if premultiplied.len() < self.width() as usize * self.height() as usize * 4 {
            return;
        }
        self.keep(|i, c| premultiplied[i * 4 + c] as f32 / 255.0, drawn);
    }

    /// The same from floats (premultiplied RGBA, 0..1 and past it: an F16 one keeps what is past).
    pub fn store_floats(&self, premultiplied: &[f32], drawn: bool) {
        if premultiplied.len() < self.width() as usize * self.height() as usize * 4 {
            return;
        }
        self.keep(|i, c| premultiplied[i * 4 + c], drawn);
    }

    /// For renderers: drawn into where only the renderer has its pixels (a new version, the CPU copy behind it).
    pub fn drawn_on_renderer(&self) {
        self.lock().version += 1;
    }

    /// Premultiplied RGBA (`at(i, channel)`) kept as its config keeps it.
    fn keep(&self, at: impl Fn(usize, usize) -> f32, drawn: bool) {
        let n = self.width() as usize * self.height() as usize;
        let config = self.config();
        let f16 = config == ImageBitmapConfig::F16;
        let has_alpha = self.has_alpha();
        let mut rgba = vec![0u8; n * 4];
        let mut floats = if f16 { vec![0.0f32; n * 4] } else { Vec::new() };
        for i in 0..n {
            let (r, g, b, mut a) = (at(i, 0), at(i, 1), at(i, 2), at(i, 3));
            let q = &mut rgba[i * 4..i * 4 + 4];
            match config {
                ImageBitmapConfig::Alpha8 => {
                    q[3] = byte(a);
                }
                ImageBitmapConfig::Rgb565 => {
                    // (Premultiplied, the alpha left out: over black; rounded to 5, 6 and 5 bits, as Skia stores
                    // them.)
                    let r5 = (r.clamp(0.0, 1.0) * 31.0).round() as i32;
                    let g6 = (g.clamp(0.0, 1.0) * 63.0).round() as i32;
                    let b5 = (b.clamp(0.0, 1.0) * 31.0).round() as i32;
                    q[0] = (r5 as f64 * 255.0 / 31.0).round() as u8;
                    q[1] = (g6 as f64 * 255.0 / 63.0).round() as u8;
                    q[2] = (b5 as f64 * 255.0 / 31.0).round() as u8;
                    q[3] = 255;
                }
                _ => {
                    if !has_alpha {
                        a = 1.0; // (opaque: what was drawn over black)
                    }
                    let k = if a > 0.0 { 1.0 / a } else { 0.0 };
                    if f16 {
                        let s = &mut floats[i * 4..i * 4 + 4];
                        s[0] = half(r * k);
                        s[1] = half(g * k);
                        s[2] = half(b * k);
                        s[3] = half(a);
                        q[0] = byte(s[0]);
                        q[1] = byte(s[1]);
                        q[2] = byte(s[2]);
                        q[3] = byte(s[3]);
                    } else {
                        // (As the renderer's 8 bits divide back out.)
                        let a8 = byte(a);
                        let k8 = if a8 > 0 { 255.0 / a8 as f32 } else { 0.0 };
                        q[0] = byte(byte(r) as f32 / 255.0 * k8);
                        q[1] = byte(byte(g) as f32 / 255.0 * k8);
                        q[2] = byte(byte(b) as f32 / 255.0 * k8);
                        q[3] = a8;
                    }
                }
            }
        }
        let mut p = self.lock();
        p.rgba = Arc::new(rgba);
        if f16 {
            p.floats = Arc::new(floats);
        }
        if drawn {
            p.version += 1;
        }
        p.cpu_version = p.version;
    }
}

#[named]
#[allow(clippy::too_many_arguments)]
impl ImageBitmap {
    /// `width` x `height` of its pixels from (start_x, start_y) as non-premultiplied ARGB in sRGB (Compose's
    /// readPixels), into `buffer` from `buffer_offset`, rows `stride` apart (none: the width read). Where it reads
    /// outside the bitmap, or would write outside `buffer`, nothing (where Compose throws).
    pub fn read_pixels(
        &self,
        buffer: &mut [u32],
        #[default(0)] start_x: i32,
        #[default(0)] start_y: i32,
        #[default(self.width())] width: i32,
        #[default(self.height())] height: i32,
        #[default(0)] buffer_offset: usize,
        #[default(None)] stride: Option<i32>,
    ) {
        self.read_into(buffer, start_x, start_y, width, height, buffer_offset, stride.unwrap_or(width));
    }

    /// Its pixels read into a [`PixelMap`] (Compose's toPixelMap, into a buffer of its own).
    pub fn to_pixel_map(
        &self,
        #[default(0)] start_x: i32,
        #[default(0)] start_y: i32,
        #[default(self.width())] width: i32,
        #[default(self.height())] height: i32,
    ) -> PixelMap {
        let (w, h) = (width.max(0), height.max(0));
        let mut buffer = vec![0u32; w as usize * h as usize];
        self.read_into(&mut buffer, start_x, start_y, w, h, 0, w);
        PixelMap { buffer, width: w, height: h, buffer_offset: 0, stride: w }
    }
}

impl ImageBitmap {
    /// readPixels' work.
    #[allow(clippy::too_many_arguments)]
    fn read_into(
        &self,
        buffer: &mut [u32],
        start_x: i32,
        start_y: i32,
        width: i32,
        height: i32,
        buffer_offset: usize,
        stride: i32,
    ) {
        let p = self.pixels();
        let (bw, bh) = (self.width() as i64, self.height() as i64);
        let stride = stride.max(0) as usize;
        for y in 0..height.max(0) {
            for x in 0..width.max(0) {
                let (sx, sy) = (start_x as i64 + x as i64, start_y as i64 + y as i64);
                if sx < 0 || sy < 0 || sx >= bw || sy >= bh {
                    continue;
                }
                let at = (sy * bw + sx) as usize * 4;
                let q = &p[at..at + 4];
                let to = (y as usize).checked_mul(stride).and_then(|r| r.checked_add(buffer_offset + x as usize));
                if let Some(out) = to.and_then(|i| buffer.get_mut(i)) {
                    *out = (q[3] as u32) << 24 | (q[0] as u32) << 16 | (q[1] as u32) << 8 | q[2] as u32;
                }
            }
        }
    }
}

/// Compose's ImageBitmap(width, height, config, hasAlpha, colorSpace), with its defaults, in a `#[kompozable]` body:
/// `ImageBitmap(4, 3, config = ImageBitmapConfig::Rgb565)` (from plain code: [`ImageBitmap::new`]). Panics as that
/// does.
#[named]
pub fn ImageBitmap(
    width: i32,
    height: i32,
    #[default(ImageBitmapConfig::Argb8888)] config: ImageBitmapConfig,
    #[default(true)] has_alpha: bool,
    #[default(&ColorSpaces::Srgb)] color_space: &Rgb,
) -> ImageBitmap {
    ImageBitmap::new(width, height, config, has_alpha, color_space)
}

impl PartialEq for ImageBitmap {
    /// The same bitmap.
    fn eq(&self, o: &ImageBitmap) -> bool {
        Arc::ptr_eq(&self.data, &o.data)
    }
}
impl Eq for ImageBitmap {}

impl Hash for ImageBitmap {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data.id.hash(state);
    }
}

impl fmt::Debug for ImageBitmap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ImageBitmap(#{}, {}x{}, {:?})", self.id(), self.width(), self.height(), self.config())
    }
}

/// Pixels read from a bitmap (Compose's PixelMap): `get(x, y)` the color at (x, y).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PixelMap {
    /// ARGB.
    pub buffer: Vec<u32>,
    pub width: i32,
    pub height: i32,
    pub buffer_offset: usize,
    pub stride: i32,
}

impl PixelMap {
    /// The color at (x, y) (Compose's get); outside the buffer, transparent.
    pub fn get(&self, x: i32, y: i32) -> Color {
        let i = self.buffer_offset as i64 + y as i64 * self.stride as i64 + x as i64;
        let argb = usize::try_from(i).ok().and_then(|i| self.buffer.get(i)).copied().unwrap_or(0);
        Color::from_argb(argb)
    }
}

/// `space` kept by a bitmap: the built-in one where it is one, else a copy.
fn own_color_space(space: &Rgb) -> BitmapColorSpace {
    let id = space.id();
    if id != MIN_ID
        && let Some(built_in) = ColorSpaces::get_color_space(id).as_rgb()
        && built_in.equals(space)
    {
        return BitmapColorSpace::BuiltIn(built_in);
    }
    BitmapColorSpace::Own(Box::new(space.clone()))
}

/// Rounded to what a half float holds (11 significant bits, as far as it reaches).
fn half(v: f32) -> f32 {
    if !v.is_finite() || v == 0.0 {
        return v;
    }
    let a = v.abs().min(65504.0);
    // frexp's exponent (a = m 2^e, m in 0.5..1); subnormals below 2^-14: steps of 2^-24.
    let e = (((a.to_bits() >> 23) & 0xff) as i32 - 126).max(-13);
    let step = 2f32.powi(e - 11);
    ((a / step).round() * step).copysign(v)
}

fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}
