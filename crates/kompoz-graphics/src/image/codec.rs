//! Image files decoded into bitmaps (JPEG, PNG, WebP, BMP), scaled down for the size they are wanted at, and PNG
//! written. Pure Rust everywhere: the `png`, `zune-jpeg` and `image-webp` crates, and BMP read here (where the C++
//! Kompoz used each platform's decoders: BitmapFactory, WIC, the browser, libpng, libjpeg-turbo, libwebp).

use super::bitmap::ImageBitmap;
use super::sample::{ImageRequestSize, image_sample_size, scale_down_rgba};
use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Pixels as straight RGBA bytes, the top row first.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RgbaImage {
    pub width: i32,
    pub height: i32,
    pub rgba: Vec<u8>,
}

impl RgbaImage {
    /// As a bitmap (Argb8888, sRGB).
    pub fn into_bitmap(self) -> Option<ImageBitmap> {
        ImageBitmap::from_pixels(self.width, self.height, self.rgba)
    }

    /// 1/`sample` the size, as [`scale_down`](super::scale_down) makes it (a sample of 1 or less: as it is).
    pub fn scaled_down(self, sample: i32) -> RgbaImage {
        if sample <= 1 {
            return self;
        }
        match scale_down_rgba(self.width as usize, self.height as usize, &self.rgba, sample as usize) {
            Some((w, h, rgba)) => RgbaImage { width: w as i32, height: h as i32, rgba },
            None => self,
        }
    }
}

/// What is decoded at most: 32768 pixels each way and 2^26 in all (256 MB), more than is drawn (images are scaled
/// down to [`MAX_IMAGE_DIMENSION`](super::MAX_IMAGE_DIMENSION)) and not more than memory takes.
pub(super) fn size_ok(width: u64, height: u64) -> bool {
    width > 0 && height > 0 && width <= 32768 && height <= 32768 && width * height <= 1 << 26
}

/// `decode` run where a decoder of malformed bytes may panic: none then.
pub(super) fn guarded<T>(decode: impl FnOnce() -> Option<T>) -> Option<T> {
    catch_unwind(AssertUnwindSafe(decode)).ok().flatten()
}

/// An image file's bytes decoded (JPEG, PNG, WebP, BMP, by what the bytes are), scaled down for `size` as
/// [`image_sample_size`] says (by averaging, as [`scale_down`](super::scale_down) does); none if they can't be.
/// Straight RGBA in sRGB: a PNG's gAMA chunk honored, as libpng's simplified API does (else sRGB; no ICC profile
/// or chromaticities read, as libpng's, libjpeg-turbo's and libwebp's weren't), no EXIF orientation applied (as
/// BitmapFactory and libjpeg-turbo don't); an animated PNG's or WebP's first frame.
pub fn decode_image(bytes: &[u8], size: ImageRequestSize) -> Option<ImageBitmap> {
    if bytes.len() < 12 {
        return None;
    }
    let image = if bytes[..3] == [0xFF, 0xD8, 0xFF] {
        decode_jpeg(bytes)
    } else if &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        decode_webp(bytes)
    } else if bytes.starts_with(PNG_SIGNATURE) {
        super::png_decoder::decode_png(bytes)
    } else if bytes.starts_with(b"BM") {
        super::bmp_decoder::decode_bmp(bytes)
    } else {
        None
    }?;
    let sample = image_sample_size(image.width, image.height, size);
    image.scaled_down(sample).into_bitmap()
}

/// An image file's bytes decoded whole (Compose's ByteArray.decodeToImageBitmap; where it throws, none).
pub fn decode_to_image_bitmap(bytes: &[u8]) -> Option<ImageBitmap> {
    decode_image(bytes, ImageRequestSize::default())
}

pub(super) const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// A JPEG (baseline or progressive; gray, YCbCr, RGB, CMYK or YCCK) as RGBA.
fn decode_jpeg(bytes: &[u8]) -> Option<RgbaImage> {
    use zune_jpeg::JpegDecoder;
    use zune_jpeg::zune_core::bytestream::ZCursor;
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;
    guarded(|| {
        // (RGB, made RGBA here: zune-jpeg 0.5 writes a CMYK one's RGBA 3 bytes a pixel.)
        let options = DecoderOptions::default()
            .jpeg_set_out_colorspace(ColorSpace::RGB)
            .set_max_width(32768)
            .set_max_height(32768);
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
        decoder.decode_headers().ok()?;
        let info = decoder.info()?;
        let (w, h) = (info.width as u64, info.height as u64);
        if !size_ok(w, h) || decoder.output_colorspace()? != ColorSpace::RGB {
            return None;
        }
        let mut rgb = vec![0u8; decoder.output_buffer_size()?];
        decoder.decode_into(&mut rgb).ok()?;
        let n = (w * h) as usize;
        let rgb = rgb.get(..n * 3)?;
        let mut rgba = Vec::with_capacity(n * 4);
        for p in rgb.as_chunks::<3>().0 {
            rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
        }
        Some(RgbaImage { width: w as i32, height: h as i32, rgba })
    })
}

/// A WebP (lossy, lossless, with alpha; an animated one's first frame, over transparent) as RGBA.
fn decode_webp(bytes: &[u8]) -> Option<RgbaImage> {
    use image_webp::WebPDecoder;
    guarded(|| {
        let mut decoder = WebPDecoder::new(Cursor::new(bytes)).ok()?;
        let (w, h) = decoder.dimensions();
        if !size_ok(w as u64, h as u64) {
            return None;
        }
        let mut pixels = vec![0u8; decoder.output_buffer_size()?];
        decoder.read_image(&mut pixels).ok()?;
        let n = w as usize * h as usize;
        let rgba = if decoder.has_alpha() {
            pixels
        } else {
            let mut rgba = Vec::with_capacity(n * 4);
            for p in pixels.as_chunks::<3>().0 {
                rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
            }
            rgba
        };
        (rgba.len() == n * 4).then_some(RgbaImage { width: w as i32, height: h as i32, rgba })
    })
}

/// Straight RGBA pixels (`width` x `height`, the top row first) as a PNG file's bytes; none for a size of 0 or too
/// few pixels.
pub fn encode_png(width: i32, height: i32, rgba: &[u8]) -> Option<Vec<u8>> {
    if width <= 0 || height <= 0 || rgba.len() < width as usize * height as usize * 4 {
        return None;
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(&rgba[..width as usize * height as usize * 4]).ok()?;
    writer.finish().ok()?;
    Some(out)
}

/// The same written to `path`.
pub fn save_png(path: impl AsRef<std::path::Path>, width: i32, height: i32, rgba: &[u8]) -> std::io::Result<()> {
    let bytes = encode_png(width, height, rgba)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no pixels to write"))?;
    std::fs::write(path, bytes)
}
