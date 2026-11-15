//! Bitmaps, the pixels read from them, content scales, the size an image is decoded at, and image files decoded
//! (the C++ Kompoz's tests about them; its PNG gamma numbers are libpng 1.6.59's, read through its simplified API as
//! the C++ read them).

use kompoz_graphics::colorspace::{ColorSpace, ColorSpaces, Illuminant, Rgb, same_space};
use kompoz_graphics::*;
use kompoz_runtime::kompozable_block;
use std::sync::Arc;

// ---- Helpers ------------------------------------------------------------------------------------------------------

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn data(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).expect("a test file")
}

/// The pixel at (x, y), RGBA.
fn rgba_at(image: &ImageBitmap, x: i32, y: i32) -> [u8; 4] {
    let p = image.pixels();
    let i = (y * image.width() + x) as usize * 4;
    [p[i], p[i + 1], p[i + 2], p[i + 3]]
}

fn argb_at(b: &ImageBitmap, x: i32, y: i32) -> u32 {
    let mut c = [0u32];
    kompozable_block!({
        b.read_pixels(&mut c, start_x = x, start_y = y, width = 1, height = 1);
    });
    c[0]
}

/// Its RGB within `tolerance` of `rgb`, and opaque.
fn near_rgb(p: [u8; 4], rgb: u32, tolerance: i32) -> bool {
    let want = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8];
    (0..3).all(|c| (p[c] as i32 - want[c] as i32).abs() <= tolerance) && p[3] == 255
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in bytes {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

/// `raw` as a zlib stream of stored blocks.
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = if raw.is_empty() { vec![&[]] } else { raw.chunks(65535).collect() };
    for (i, block) in blocks.iter().enumerate() {
        let len = block.len() as u16;
        z.push((i + 1 == blocks.len()) as u8);
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&(b << 16 | a).to_be_bytes());
    z
}

/// A PNG of its parts: its header's fields, the chunks before its data (gAMA, sRGB, sBIT, PLTE, tRNS, acTL,
/// fcTL…), its data as written (each row's filter byte first; an interlaced one's passes one after another), and
/// any chunks after it.
#[allow(clippy::too_many_arguments)]
fn png_file(
    w: u32,
    h: u32,
    depth: u8,
    color: u8,
    interlaced: bool,
    before: &[(&[u8; 4], Vec<u8>)],
    raw: &[u8],
    after: &[(&[u8; 4], Vec<u8>)],
) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8; 4], body: &[u8]| {
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        let mut typed = kind.to_vec();
        typed.extend_from_slice(body);
        out.extend_from_slice(&typed);
        out.extend_from_slice(&crc32(&typed).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[depth, color, 0, 0, interlaced as u8]);
    chunk(b"IHDR", &ihdr);
    for (kind, body) in before {
        chunk(kind, body);
    }
    chunk(b"IDAT", &zlib_stored(raw));
    for (kind, body) in after {
        chunk(kind, body);
    }
    chunk(b"IEND", &[]);
    out
}

/// Rows with no filter, each its samples as given.
fn rows(rows: &[Vec<u8>]) -> Vec<u8> {
    rows.iter().flat_map(|r| std::iter::once(0).chain(r.iter().copied())).collect()
}

fn gama(g: u32) -> (&'static [u8; 4], Vec<u8>) {
    (b"gAMA", g.to_be_bytes().to_vec())
}

/// Values big-endian.
fn be16(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

fn png_rgba(bytes: &[u8]) -> Vec<[u8; 4]> {
    decode_png(bytes).expect("a PNG").rgba.as_chunks::<4>().0.to_vec()
}

/// A BMP of its parts: a 40-byte header, what follows it (masks, palette) and the rows as they are in the file
/// (padded to 4 bytes; the bottom one first when the height is positive).
fn bmp(w: i32, h: i32, bpp: u16, compression: u32, extra: &[u8], rows: &[u8], colors: u32) -> Vec<u8> {
    let mut out = b"BM".to_vec();
    let offset = 14 + 40 + extra.len() as u32;
    for v in [offset + rows.len() as u32, 0, offset, 40, w as u32, h as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&bpp.to_le_bytes());
    for v in [compression, rows.len() as u32, 2835, 2835, colors, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(extra);
    out.extend_from_slice(rows);
    out
}

// ---- Bitmaps ------------------------------------------------------------------------------------------------------

#[test]
fn bitmaps_are_composes() {
    let srgb: &Rgb = &ColorSpaces::Srgb;
    let clear = ImageBitmap::new(4, 3, ImageBitmapConfig::Argb8888, true, srgb);
    assert!(clear.config() == ImageBitmapConfig::Argb8888 && clear.has_alpha());
    assert!(argb_at(&clear, 1, 1) == 0 && clear.version() == 1);
    // Compose's defaults: Argb8888, with alpha, in sRGB.
    let by_default = kompozable_block!({ ImageBitmap(4, 3) });
    assert!(by_default.config() == ImageBitmapConfig::Argb8888 && by_default.has_alpha());
    assert!(same_space(by_default.color_space(), &ColorSpaces::Srgb) && by_default.width() == 4);
    let rgb565 = kompozable_block!({ ImageBitmap(4, 3, config = ImageBitmapConfig::Rgb565) });
    assert_eq!(argb_at(&rgb565, 2, 2), 0xFF000000);
    assert!(!ImageBitmap::new(4, 3, ImageBitmapConfig::Rgb565, true, srgb).has_alpha());
    assert_eq!(argb_at(&ImageBitmap::new(4, 3, ImageBitmapConfig::Argb8888, false, srgb), 0, 0), 0xFF000000);
    let alpha8 = ImageBitmap::new(4, 3, ImageBitmapConfig::Alpha8, false, srgb);
    assert!(argb_at(&alpha8, 0, 0) == 0 && alpha8.has_alpha());
    let f16 = ImageBitmap::new(2, 1, ImageBitmapConfig::F16, false, srgb);
    assert_eq!(*f16.floats(), vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    assert!(ImageBitmap::try_new(0, 3, ImageBitmapConfig::Argb8888, true, srgb).is_none());
    assert!(ImageBitmap::try_new(MAX_IMAGE_DIMENSION + 1, 3, ImageBitmapConfig::Argb8888, true, srgb).is_none());
    let zero = std::panic::AssertUnwindSafe(|| ImageBitmap::new(0, 3, ImageBitmapConfig::Argb8888, true, srgb));
    assert!(std::panic::catch_unwind(zero).is_err());
    // Its color space: a built-in one as it is, another kept.
    let p3 = ImageBitmap::new(1, 1, ImageBitmapConfig::Argb8888, true, &ColorSpaces::DisplayP3);
    assert!(same_space(p3.color_space(), &ColorSpaces::DisplayP3));
    let extended = ImageBitmap::new(4, 4, ImageBitmapConfig::F16, true, &ColorSpaces::LinearExtendedSrgb);
    assert!(same_space(extended.color_space(), &ColorSpaces::LinearExtendedSrgb));
    let own = ColorSpaces::DisplayP3.adapt(Illuminant::D50);
    let own = own.as_rgb().expect("an Rgb");
    let adapted = ImageBitmap::new(1, 1, ImageBitmapConfig::Argb8888, true, own);
    assert!(adapted.color_space().equals(own) && !same_space(adapted.color_space(), own));
    // Copies are the same bitmap; another is another.
    assert!(clear.clone() == clear && clear != ImageBitmap::new(4, 3, ImageBitmapConfig::Argb8888, true, srgb));
    assert!(clear.id() != by_default.id());
    clear.prepare_to_draw();

    let two =
        ImageBitmap::from_pixels(2, 2, vec![255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 255, 10, 20, 30, 0]).unwrap();
    assert!(ImageBitmap::from_pixels(2, 2, vec![0; 15]).is_none() && ImageBitmap::from_pixels(0, 2, vec![]).is_none());
    // From the second, rows three apart.
    let mut all = vec![7u32; 9];
    kompozable_block!({
        two.read_pixels(&mut all, 0, 0, 2, 2, buffer_offset = 1, stride = Some(3));
    });
    assert_eq!(all[..6], [7, 0xFFFF0000, 0x8000FF00, 7, 0xFF0000FF, 0x000A141E]);
    // Past its edge: nothing there ((2, 1) left as it was).
    kompozable_block!({
        two.read_pixels(&mut all, start_x = 1, start_y = 1, width = 2, height = 2);
    });
    assert!(all[0] == 0x000A141E && all[1] == 0xFFFF0000);
    // Past the buffer's end: nothing either.
    let mut short = [0u32; 3];
    two.read_pixels(&mut short);
    assert_eq!(short, [0xFFFF0000, 0x8000FF00, 0xFF0000FF]);
    let map = two.to_pixel_map();
    assert!(map.width == 2 && map.height == 2 && map.stride == 2);
    assert_eq!(map.get(1, 0), Color::from_argb(0x8000FF00));
    assert_eq!(map.get(0, 1), Color::hex(0x0000FF));
    let part = kompozable_block!({ two.to_pixel_map(start_x = 1, width = 1) });
    assert!(part.width == 1 && part.get(0, 1) == Color::from_argb(0x000A141E));

    // What each config keeps of what is drawn (premultiplied, as a renderer gives it).
    let stored = |config: ImageBitmapConfig, has_alpha: bool, premultiplied: [u8; 4]| {
        let b = ImageBitmap::new(1, 1, config, has_alpha, srgb);
        b.store(&premultiplied, true);
        (argb_at(&b, 0, 0), b.version())
    };
    // (Straight: 127.5 and 63.75.)
    assert_eq!(stored(ImageBitmapConfig::Argb8888, true, [100, 50, 0, 200]), (0xC8804000, 2));
    assert_eq!(stored(ImageBitmapConfig::Gpu, true, [100, 50, 0, 200]).0, 0xC8804000);
    // (Over black.)
    assert_eq!(stored(ImageBitmapConfig::Argb8888, false, [100, 50, 0, 200]).0, 0xFF643200);
    assert_eq!(stored(ImageBitmapConfig::Alpha8, true, [100, 50, 0, 200]).0, 0xC8000000);
    // 12/31, 12/63, 0; rounded (as Skia stores them): 16/31, 17/63, 4/31.
    assert_eq!(stored(ImageBitmapConfig::Rgb565, true, [100, 50, 0, 200]).0, 0xFF633100);
    assert_eq!(stored(ImageBitmapConfig::Rgb565, true, [128, 70, 30, 255]).0, 0xFF844521);
    let fine = ImageBitmap::new(1, 1, ImageBitmapConfig::F16, true, srgb);
    // (Past 1 too.)
    fine.store_floats(&[0.5004 * 0.5, 2.0 * 0.5, 0.0, 0.5], true);
    let f = fine.floats();
    assert!(f.len() == 4 && near(f[0], 1025.0 / 2048.0, 1e-7) && near(f[1], 2.0, 1e-6) && near(f[3], 0.5, 1e-7));
    assert_eq!(argb_at(&fine, 0, 0), 0x8080FF00);
    // Too few pixels: nothing kept. Kept without a draw: the same version.
    let b = ImageBitmap::new(2, 1, ImageBitmapConfig::Argb8888, true, srgb);
    b.store(&[255; 7], true);
    assert!(b.version() == 1 && argb_at(&b, 0, 0) == 0);
    b.store(&[255; 8], false);
    assert!(b.version() == 1 && argb_at(&b, 1, 0) == 0xFFFFFFFF && b.pixels_current());
    // Drawn into where only a renderer has its pixels: a new version, the CPU copy behind it.
    b.drawn_on_renderer();
    assert!(b.version() == 2 && !b.pixels_current());
}

// ---- Sizes and scales ---------------------------------------------------------------------------------------------

#[test]
fn a_bitmap_held_weakly_lives_only_while_held() {
    let srgb: &Rgb = &ColorSpaces::Srgb;
    let bitmap = ImageBitmap::new(2, 2, ImageBitmapConfig::Argb8888, true, srgb);
    let other = ImageBitmap::new(2, 2, ImageBitmapConfig::Argb8888, true, srgb);
    let weak = bitmap.downgrade();
    assert!(weak.is_alive() && weak.is_of(&bitmap) && !weak.is_of(&other));
    assert_eq!(weak.upgrade(), Some(bitmap.clone()));
    assert!(weak == bitmap.clone().downgrade() && weak != other.downgrade());
    drop(bitmap);
    assert!(!weak.is_alive() && weak.upgrade().is_none());
    assert_eq!(format!("{weak:?}"), "WeakImageBitmap(gone)");
}

#[test]
fn images_are_decoded_smaller_by_powers_of_two_never_below_the_size() {
    let size = ImageRequestSize::new;
    // Fitted: 1000 x 750. Cropped: 2000 x 1500 covers it.
    assert_eq!(image_sample_size(4000, 3000, size(1000, 1000, false)), 4);
    assert_eq!(image_sample_size(4000, 3000, size(1000, 1000, true)), 2);
    assert!(
        image_sample_size(4000, 3000, size(500, 0, false)) == 8
            && image_sample_size(4000, 3000, size(0, 1500, false)) == 2
    );
    // 1000 would be too narrow.
    assert_eq!(image_sample_size(4000, 3000, size(1001, 0, false)), 2);
    assert!(image_sample_size(300, 200, size(1000, 1000, false)) == 1);
    assert!(image_sample_size(4000, 3000, ImageRequestSize::default()) == 1);
    // At most MAX_IMAGE_DIMENSION across: 2500.
    assert_eq!(image_sample_size(10000, 100, ImageRequestSize::default()), 4);
    assert!(image_covers(1000, 750, size(1000, 1000, false)) && !image_covers(1000, 750, size(1000, 1000, true)));
    assert!(image_covers(2000, 1500, size(1000, 1000, true)) && !image_covers(5, 5, ImageRequestSize::default()));
    assert!(!image_covers(999, 750, size(1000, 0, false)) && ImageRequestSize::default().any());
    // Scaled down by averaging, weighted by alpha (a transparent pixel adds no color).
    let image = ImageBitmap::from_pixels(
        3,
        2,
        vec![255, 0, 0, 255, 0, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 255, 255, 255, 0, 255, 0, 255, 0],
    )
    .unwrap();
    let half = scale_down(&image, 2);
    assert!(half.width() == 2 && half.height() == 1);
    assert_eq!(*half.pixels(), vec![170, 0, 85, 191, 0, 255, 0, 128]);
    assert!(scale_down(&image, 1) == image);
}

#[test]
fn content_scales_are_composes() {
    let (photo, box_) = (Size::new(200.0, 100.0), Size::new(100.0, 100.0));
    let scale = |s: ContentScale| s.compute_scale_factor(photo, box_);
    // 2:1 into a square: fitted, letterboxed (100 x 50); cropped, filling it and showing the middle half.
    assert_eq!(scale(ContentScale::Fit), ScaleFactor::new(0.5, 0.5));
    assert_eq!(photo * scale(ContentScale::Fit), Size::new(100.0, 50.0));
    assert_eq!(scale(ContentScale::Crop), ScaleFactor::new(1.0, 1.0));
    assert_eq!(scale(ContentScale::FillBounds), ScaleFactor::new(0.5, 1.0));
    assert_eq!(scale(ContentScale::FillWidth), ScaleFactor::new(0.5, 0.5));
    assert_eq!(scale(ContentScale::FillHeight), ScaleFactor::new(1.0, 1.0));
    // Inside: as it is unless too big.
    assert_eq!(scale(ContentScale::Inside), ScaleFactor::new(0.5, 0.5));
    assert_eq!(ContentScale::Inside.compute_scale_factor(Size::new(50.0, 20.0), box_), ScaleFactor::new(1.0, 1.0));
    assert!(
        ContentScale::None == ContentScale::FixedScale(1.0) && scale(ContentScale::None) == ScaleFactor::new(1.0, 1.0)
    );
    assert_eq!(scale(ContentScale::FixedScale(3.0)), ScaleFactor::new(3.0, 3.0));
    // One's own: equal only to itself.
    struct Thirds;
    impl ComputeScaleFactor for Thirds {
        fn compute_scale_factor(&self, src_size: Size, dst_size: Size) -> ScaleFactor {
            ScaleFactor::new(dst_size.width / src_size.width / 3.0, 1.0 / 3.0)
        }
    }
    let thirds = ContentScale::Custom(Arc::new(Thirds));
    assert_eq!(scale(thirds.clone()), ScaleFactor::new(0.5 / 3.0, 1.0 / 3.0));
    assert!(
        thirds == thirds.clone() && thirds != ContentScale::Custom(Arc::new(Thirds)) && thirds != ContentScale::Fit
    );
    // ScaleFactor's arithmetic.
    let f = ScaleFactor::new(2.0, 4.0);
    assert!(f * 0.5 == ScaleFactor::new(1.0, 2.0) && f / 2.0 == ScaleFactor::new(1.0, 2.0));
    assert!(f * Size::new(3.0, 3.0) == Size::new(6.0, 12.0) && Size::new(4.0, 4.0) / f == Size::new(2.0, 1.0));
    assert_eq!(lerp_scale_factor(f, ScaleFactor::new(4.0, 0.0), 0.5), ScaleFactor::new(3.0, 2.0));
    assert!(ScaleFactor::Unspecified.is_unspecified() && f.is_specified());
    assert_eq!(ScaleFactor::Unspecified.take_or_else(|| f), f);
    assert_eq!(f.copy(None, Some(1.0)), ScaleFactor::new(2.0, 1.0));
    assert_eq!(format!("{f:?}"), "ScaleFactor(2.0, 4.0)");
}

// ---- Decoding -----------------------------------------------------------------------------------------------------

/// 64x32 pictures, the left half #D32F2F and the right half #1976D2 (the C++ test's, written by libjpeg-turbo at
/// quality 95 without chroma subsampling, and by libwebp, lossless): decoded as they are, and scaled down.
#[test]
fn images_are_decoded() {
    let check = |bytes: &[u8], size: ImageRequestSize, w: i32, h: i32, tolerance: i32| {
        let image = decode_image(bytes, size).expect("decoded");
        assert_eq!((image.width(), image.height()), (w, h));
        assert!(near_rgb(rgba_at(&image, w / 4, h / 2), 0xD32F2F, tolerance), "{:?}", rgba_at(&image, w / 4, h / 2));
        assert!(near_rgb(rgba_at(&image, w * 3 / 4, h / 2), 0x1976D2, tolerance));
    };
    let (jpeg, webp) = (data("halves.jpg"), data("halves.webp"));
    check(&jpeg, ImageRequestSize::default(), 64, 32, 6);
    check(&jpeg, ImageRequestSize::new(32, 16, false), 32, 16, 6);
    check(&jpeg, ImageRequestSize::new(8, 4, false), 8, 4, 6);
    check(&webp, ImageRequestSize::default(), 64, 32, 0);
    check(&webp, ImageRequestSize::new(16, 8, false), 16, 8, 1);
    // Not a picture after all.
    assert!(decode_image(&[0xFF, 0xD8, 0xFF, 0, 1, 2, 3, 4, 5, 6, 7, 8], ImageRequestSize::default()).is_none());
    assert!(
        decode_to_image_bitmap(&jpeg).is_some_and(|b| b.width() == 64 && b.config() == ImageBitmapConfig::Argb8888)
    );
}

/// PNG written and read back as it was; BMP read in each of its kinds; what isn't either, or is cut short, not
/// read.
#[test]
fn png_and_bmp_are_read() {
    // 3 x 2.
    let rgba = vec![255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 10, 20, 30, 40, 200, 100, 50, 255, 1, 2, 3, 4];
    let png = encode_png(3, 2, &rgba).unwrap();
    let back = decode_png(&png).unwrap();
    assert!(back.width == 3 && back.height == 2 && back.rgba == rgba);
    // (Through the image loader too.)
    let image = decode_image(&png, ImageRequestSize::default()).unwrap();
    assert!(image.width() == 3 && image.height() == 2 && *image.pixels() == rgba);
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("kompoz-graphics-image-test.png");
    save_png(&path, 3, 2, &rgba).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), png);
    std::fs::remove_file(&path).unwrap();
    assert!(encode_png(0, 2, &rgba).is_none() && encode_png(3, 3, &rgba).is_none());
    // 16 bits saying nothing of their gamma: sRGB, as written (libpng alone would take them as linear).
    let deep = png_file(2, 1, 16, 2, false, &[], &rows(&[be16(&[0x8080, 0x4040, 0xFFFF, 0, 0x1010, 0xC0C0])]), &[]);
    assert_eq!(decode_png(&deep).unwrap().rgba, vec![0x80, 0x40, 0xFF, 255, 0, 0x10, 0xC0, 255]);

    let read = |bytes: &[u8]| decode_image(bytes, ImageRequestSize::default()).map(|b| b.pixels().to_vec());
    // 24 bits, the bottom row first, each row padded to 4 bytes.
    let b24 = decode_bmp(&bmp(2, 2, 24, 0, &[], &[3, 2, 1, 6, 5, 4, 0, 0, 30, 20, 10, 60, 50, 40, 0, 0], 0)).unwrap();
    assert!(b24.width == 2 && b24.height == 2);
    assert_eq!(b24.rgba, vec![10, 20, 30, 255, 40, 50, 60, 255, 1, 2, 3, 255, 4, 5, 6, 255]);
    // 32 bits, the top row first (a negative height): the fourth byte is alpha, unless it is 0 everywhere.
    assert_eq!(read(&bmp(2, -1, 32, 0, &[], &[3, 2, 1, 128, 6, 5, 4, 0], 0)), Some(vec![1, 2, 3, 128, 4, 5, 6, 0]));
    assert_eq!(read(&bmp(2, -1, 32, 0, &[], &[3, 2, 1, 0, 6, 5, 4, 0], 0)), Some(vec![1, 2, 3, 255, 4, 5, 6, 255]));
    // A palette: 8, 4 and 1 bits an index (red, green, blue: BGRx).
    let palette = [0, 0, 255, 0, 0, 255, 0, 0, 255, 0, 0, 0];
    let want = vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255];
    assert_eq!(read(&bmp(3, 1, 8, 0, &palette, &[0, 1, 2, 0], 3)), Some(want.clone()));
    assert_eq!(read(&bmp(3, 1, 4, 0, &palette, &[0x01, 0x20, 0, 0], 3)), Some(want));
    assert_eq!(read(&bmp(2, 1, 1, 0, &palette[..8], &[0x40, 0, 0, 0], 2)), Some(vec![255, 0, 0, 255, 0, 255, 0, 255]));
    // 16 bits by bit fields (5-6-5), and by default 5-5-5.
    let masks565 = [0x00, 0xF8, 0, 0, 0xE0, 0x07, 0, 0, 0x1F, 0, 0, 0];
    assert_eq!(
        read(&bmp(2, 1, 16, 3, &masks565, &[0x00, 0xF8, 0x1F, 0x00], 0)),
        Some(vec![255, 0, 0, 255, 0, 0, 255, 255])
    );
    assert_eq!(read(&bmp(1, 1, 16, 0, &[], &[0xE0, 0x03, 0, 0], 0)), Some(vec![0, 255, 0, 255]));
    // Not read: run-length encoded, cut short, neither.
    assert_eq!(read(&bmp(2, 1, 8, 1, &palette, &[0, 1, 2, 0], 3)), None);
    let mut cut = bmp(2, 2, 24, 0, &[], &[7; 16], 0);
    cut.pop();
    assert_eq!(read(&cut), None);
    assert_eq!(read(&png[..png.len() / 2]), None);
    assert_eq!(read(b"GIF89a\0\0\0\0\0\0"), None);
}

/// A PNG's gamma corrected to sRGB's as libpng's simplified API corrects it (its gAMA chunk; an sRGB chunk over
/// it; none: sRGB), byte for byte as libpng 1.6.59: 8 bits by its 8-bit table, 16 by its 16-to-8 table (11 bits
/// of the input, or the significant bits sBIT says, at most 8 dropped), alpha left as it is.
#[test]
fn png_gamma_is_corrected_as_libpngs() {
    let ramp8 = rows(&[(0..=255).collect()]);
    let at = |pixels: &[[u8; 4]], i: &[usize]| i.iter().map(|&i| pixels[i]).collect::<Vec<_>>();
    let gray = |v: u8, a: u8| [v, v, v, a];
    // Linear (gAMA 1.0) and 2.2 data, made sRGB's.
    let linear = png_rgba(&png_file(256, 1, 8, 0, false, &[gama(100_000)], &ramp8, &[]));
    assert_eq!(
        at(&linear, &[0, 1, 10, 64, 128, 200, 254, 255]),
        [0, 21, 59, 136, 186, 228, 255, 255].map(|v| gray(v, 255))
    );
    let dark = png_rgba(&png_file(256, 1, 8, 0, false, &[gama(220_000)], &ramp8, &[]));
    assert_eq!(at(&dark, &[1, 10, 64, 128, 200]), [81, 131, 192, 221, 243].map(|v| gray(v, 255)));
    // sRGB's own (within 5% of it), an sRGB chunk over a gAMA one, or none: as they are.
    for before in [vec![gama(45_455)], vec![gama(47_000)], vec![gama(100_000), (b"sRGB", vec![0])], vec![]] {
        let same = png_rgba(&png_file(256, 1, 8, 0, false, &before, &ramp8, &[]));
        assert!(same.iter().enumerate().all(|(i, p)| *p == gray(i as u8, 255)));
    }
    // A palette's colors corrected, its alpha not; a 4-bit gray's scaled to 8 bits first; an RGBA's alpha not.
    let palette: Vec<u8> = (0..=255u8).flat_map(|i| [i, 255 - i, i / 3]).collect();
    let indexed = png_rgba(&png_file(
        256,
        1,
        8,
        3,
        false,
        &[gama(100_000), (b"PLTE", palette), (b"tRNS", (0..=255).collect())],
        &ramp8,
        &[],
    ));
    assert_eq!(
        at(&indexed, &[0, 10, 128, 254]),
        [[0, 255, 0, 0], [59, 250, 34, 10], [186, 186, 112, 128], [255, 21, 154, 254]]
    );
    let nibbles: Vec<u8> = (0..8u8).map(|i| (2 * i) << 4 | (2 * i + 1)).collect();
    let gray4 = png_rgba(&png_file(16, 1, 4, 0, false, &[gama(100_000)], &rows(&[nibbles]), &[]));
    assert_eq!(at(&gray4, &[0, 1, 10]), [gray(0, 255), gray(74, 255), gray(212, 255)]);
    let rgba8: Vec<u8> = (0..=255u8).flat_map(|v| [v, 255 - v, v / 2, v]).collect();
    let rgba8 = png_rgba(&png_file(256, 1, 8, 6, false, &[gama(100_000)], &rows(&[rgba8]), &[]));
    assert_eq!(
        at(&rgba8, &[0, 10, 128, 254]),
        [[0, 255, 0, 0], [59, 250, 43, 10], [186, 186, 136, 128], [255, 21, 186, 254]]
    );

    // 16 bits: every 7th value.
    let values: Vec<u16> = (0..=65535u32).step_by(7).map(|v| v as u16).chain([65535]).collect();
    let ramp16 = rows(&[be16(&values)]);
    let picks = [0, 1, 100, 938, 4681, 9000, 9362];
    let deep = |before: &[(&[u8; 4], Vec<u8>)]| {
        png_rgba(&png_file(values.len() as u32, 1, 16, 0, false, before, &ramp16, &[]))
    };
    assert_eq!(at(&deep(&[gama(100_000)]), &picks), [0, 0, 31, 89, 186, 250, 255].map(|v| gray(v, 255)));
    assert_eq!(at(&deep(&[]), &picks), [0, 0, 3, 26, 127, 245, 255].map(|v| gray(v, 255)));
    // 10 significant bits: 6 dropped.
    assert_eq!(
        at(&deep(&[gama(100_000), (b"sBIT", vec![10])]), &picks),
        [0, 0, 30, 89, 186, 250, 255].map(|v| gray(v, 255))
    );
    // A gray's tRNS: that value transparent.
    let keyed = deep(&[gama(100_000), (b"tRNS", 700u16.to_be_bytes().to_vec())]);
    assert_eq!(at(&keyed, &[99, 100, 101]), [gray(31, 255), gray(31, 0), gray(32, 255)]);
    let rgba16: Vec<u16> = values.iter().flat_map(|&v| [v, 65535 - v, v / 2, v]).collect();
    let rgba16 = |before: &[(&[u8; 4], Vec<u8>)]| {
        png_rgba(&png_file(values.len() as u32, 1, 16, 6, false, before, &rows(&[be16(&rgba16)]), &[]))
    };
    assert_eq!(
        at(&rgba16(&[]), &picks),
        [
            [0, 255, 0, 0],
            [0, 255, 0, 0],
            [3, 252, 1, 3],
            [26, 229, 13, 26],
            [127, 128, 64, 127],
            [245, 10, 123, 245],
            [255, 0, 127, 255]
        ]
    );
    assert_eq!(
        at(&rgba16(&[gama(100_000)]), &picks),
        [
            [0, 255, 0, 0],
            [0, 255, 0, 0],
            [31, 254, 22, 3],
            [89, 243, 65, 26],
            [186, 186, 136, 127],
            [250, 58, 183, 245],
            [255, 0, 186, 255]
        ]
    );
}

/// PNGs of every kind: gray of 1, 2, 4, 8 and 16 bits, with alpha or a tRNS color, RGB, palettes, Adam7-interlaced;
/// an animated one's default image (its first frame, or an image apart from its frames, as libpng reads it).
#[test]
fn pngs_of_every_kind_are_read() {
    let gray = |v: u8| [v, v, v, 255];
    // Gray, 1 and 2 bits: scaled to 8.
    assert_eq!(
        png_rgba(&png_file(8, 1, 1, 0, false, &[], &rows(&[vec![0b1010_0000]]), &[]))[..3],
        [gray(255), gray(0), gray(255)]
    );
    assert_eq!(
        png_rgba(&png_file(4, 1, 2, 0, false, &[], &rows(&[vec![0b00_01_10_11]]), &[])),
        [gray(0), gray(85), gray(170), gray(255)]
    );
    // Gray with alpha; RGB with a tRNS color; a 2-bit palette.
    assert_eq!(
        png_rgba(&png_file(2, 1, 8, 4, false, &[], &rows(&[vec![10, 20, 30, 40]]), &[])),
        [[10, 10, 10, 20], [30, 30, 30, 40]]
    );
    let keyed =
        png_file(2, 1, 8, 2, false, &[(b"tRNS", vec![0, 1, 0, 2, 0, 3])], &rows(&[vec![1, 2, 3, 1, 2, 4]]), &[]);
    assert_eq!(png_rgba(&keyed), [[1, 2, 3, 0], [1, 2, 4, 255]]);
    let palette = (b"PLTE", vec![255, 0, 0, 0, 255, 0, 0, 0, 255]);
    let indexed =
        png_file(3, 1, 2, 3, false, &[palette.clone(), (b"tRNS", vec![128])], &rows(&[vec![0b00_01_10_00]]), &[]);
    assert_eq!(png_rgba(&indexed), [[255, 0, 0, 128], [0, 255, 0, 255], [0, 0, 255, 255]]);
    // An index past its palette: black, opaque (as libpng expands it).
    assert_eq!(png_rgba(&png_file(1, 1, 8, 3, false, &[palette], &rows(&[vec![7]]), &[])), [[0, 0, 0, 255]]);

    // Adam7-interlaced: 13 x 7, its passes' pixels where they go.
    let (w, h) = (13usize, 7usize);
    let pixel = |x: usize, y: usize| [(x * 19) as u8, (y * 36) as u8, ((x * y) % 256) as u8, (255 - x) as u8];
    let passes = [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)];
    let mut raw = Vec::new();
    for (x0, y0, dx, dy) in passes {
        if x0 >= w {
            continue;
        }
        for y in (y0..h).step_by(dy) {
            raw.push(0);
            for x in (x0..w).step_by(dx) {
                raw.extend_from_slice(&pixel(x, y));
            }
        }
    }
    let interlaced = png_rgba(&png_file(w as u32, h as u32, 8, 6, true, &[], &raw, &[]));
    assert!((0..h).all(|y| (0..w).all(|x| interlaced[y * w + x] == pixel(x, y))));

    // Animated: two frames, the first the default image (red); or one frame (blue) and a default image apart.
    let red = rows(&[[255, 0, 0, 255].repeat(4), [255, 0, 0, 255].repeat(4)]);
    let blue = rows(&[[0, 0, 255, 255].repeat(4), [0, 0, 255, 255].repeat(4)]);
    let fctl = |seq: u32| {
        let mut b = Vec::new();
        for v in [seq, 4, 2, 0, 0] {
            b.extend_from_slice(&v.to_be_bytes());
        }
        b.extend_from_slice(&[0, 1, 0, 10, 0, 0]);
        (b"fcTL", b)
    };
    let fdat = |seq: u32, raw: &[u8]| {
        let mut b = seq.to_be_bytes().to_vec();
        b.extend_from_slice(&zlib_stored(raw));
        (b"fdAT", b)
    };
    let actl = |frames: u32| (b"acTL", [frames.to_be_bytes(), 0u32.to_be_bytes()].concat());
    let first = png_file(4, 2, 8, 6, false, &[actl(2), fctl(0)], &red, &[fctl(1), fdat(2, &blue)]);
    assert!(png_rgba(&first).iter().all(|p| *p == [255, 0, 0, 255]));
    let apart = png_file(4, 2, 8, 6, false, &[actl(1)], &red, &[fctl(0), fdat(1, &blue)]);
    assert!(png_rgba(&apart).iter().all(|p| *p == [255, 0, 0, 255]));
}

/// JPEGs (gray, CMYK, progressive; EXIF orientation not applied, as BitmapFactory and libjpeg-turbo don't) and
/// WebPs (lossy with alpha; an animated one's first frame), 16 x 8, their halves of two colors.
#[test]
fn jpegs_and_webps_of_every_kind_are_read() {
    let halves = |name: &str| {
        let image = decode_to_image_bitmap(&data(name)).expect(name);
        assert_eq!((image.width(), image.height()), (16, 8), "{name}");
        (rgba_at(&image, 4, 4), rgba_at(&image, 12, 4))
    };
    let (left, right) = halves("gray.jpg");
    assert!(near_rgb(left, 0x404040, 2) && near_rgb(right, 0xC0C0C0, 2));
    // CMYK (Adobe's, inverted): (0, 200, 200, 40) and (200, 100, 0, 40) of 255.
    let (left, right) = halves("cmyk.jpg");
    assert!(near_rgb(left, 0xD72E2E, 3) && near_rgb(right, 0x2E83D7, 3), "{left:?} {right:?}");
    for name in ["progressive.jpg", "rotated.jpg"] {
        let (left, right) = halves(name);
        assert!(near_rgb(left, 0xD32F2F, 6) && near_rgb(right, 0x1976D2, 6), "{name}");
    }
    let (left, right) = halves("alpha_lossy.webp");
    assert!(near_rgb(left, 0xD32F2F, 6));
    assert!(near_rgb([right[0], right[1], right[2], 255], 0x1976D2, 6) && (right[3] as i32 - 128).abs() <= 1);
    let (left, right) = halves("animated.webp");
    assert!(left == [255, 0, 0, 255] && right == [0, 0, 255, 255]);
}

/// Bytes cut short anywhere, or changed, are read or not, but never take the decoder down.
#[test]
fn malformed_images_are_not_read() {
    let bmp24 = bmp(2, 2, 24, 0, &[], &[3, 2, 1, 6, 5, 4, 0, 0, 30, 20, 10, 60, 50, 40, 0, 0], 0);
    let png = encode_png(3, 2, &[9; 24]).unwrap();
    let files =
        ["halves.jpg", "halves.webp", "gray.jpg", "cmyk.jpg", "progressive.jpg", "alpha_lossy.webp", "animated.webp"]
            .map(data)
            .into_iter()
            .chain([bmp24, png]);
    for bytes in files {
        for len in 0..bytes.len() {
            let _ = decode_image(&bytes[..len], ImageRequestSize::new(4, 4, false));
        }
        for at in 0..bytes.len() {
            for flip in [0xFF, 0x80, 0x01] {
                let mut changed = bytes.clone();
                changed[at] ^= flip;
                let _ = decode_image(&changed, ImageRequestSize::default());
            }
        }
    }
    // A BMP claiming more than it has, or far more than is read.
    let mut huge = bmp(1, 1, 24, 0, &[], &[0; 4], 0);
    huge[18..22].copy_from_slice(&100_000i32.to_le_bytes());
    assert!(decode_bmp(&huge).is_none());
    let mut tall = bmp(1, 1, 24, 0, &[], &[0; 4], 0);
    tall[22..26].copy_from_slice(&i32::MIN.to_le_bytes());
    assert!(decode_bmp(&tall).is_none());
    // A PNG beyond 2^26 pixels: not read (nor allocated).
    let mut big = encode_png(1, 1, &[0; 4]).unwrap();
    big[16..24].copy_from_slice(&[0, 0, 0x80, 0, 0, 0, 0x80, 0]);
    let crc = crc32(&big[12..29]);
    big[29..33].copy_from_slice(&crc.to_be_bytes());
    assert!(decode_png(&big).is_none());
}
