//! PNG decoded to 8-bit straight RGBA in sRGB, as libpng's simplified API (png_image, PNG_FORMAT_RGBA,
//! PNG_IMAGE_FLAG_16BIT_sRGB) decodes it for the C++ Kompoz: the file's gamma (its gAMA chunk; an sRGB chunk, or
//! none, sRGB's) corrected to sRGB's where they differ, as libpng's tables do it; 16 bits rounded to 8.

use super::codec::{RgbaImage, guarded, size_ok};
use std::io::Cursor;

/// libpng's PNG_GAMMA_sRGB: the screen's gamma, 2.2 (times 100000).
const SCREEN_GAMMA: f64 = 220_000.0;
/// libpng's PNG_GAMMA_sRGB_INVERSE: sRGB's file gamma, 1/2.2 (times 100000).
const SRGB_FILE_GAMMA: u32 = 45_455;

/// A PNG file's pixels (any color type and bit depth, interlaced or not; an animated one's default image) as
/// straight RGBA, 8 bits a channel; none if they can't be read (or the image is beyond 32768 pixels each way or
/// 2^26 in all).
pub fn decode_png(bytes: &[u8]) -> Option<RgbaImage> {
    guarded(|| {
        let mut decoder = png::Decoder::new(Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::EXPAND);
        // (The size is checked below; beyond the default 64 MB is fine within it.)
        decoder.set_limits(png::Limits { bytes: usize::MAX });
        decoder.set_ignore_text_chunk(true);
        let header = decoder.read_header_info().ok()?;
        if !size_ok(header.width as u64, header.height as u64) {
            return None;
        }
        let mut reader = decoder.read_info().ok()?;
        let mut buffer = vec![0u8; reader.output_buffer_size()?];
        let frame = reader.next_frame(&mut buffer).ok()?;
        let info = reader.info();
        let (w, h) = (frame.width as usize, frame.height as usize);
        let gamma = Gamma::of(info);
        let channels = frame.color_type.samples();
        let sixteen = frame.bit_depth == png::BitDepth::Sixteen;
        let (color, alpha) = match frame.color_type {
            png::ColorType::Grayscale => (1, None),
            png::ColorType::GrayscaleAlpha => (1, Some(1)),
            png::ColorType::Rgb => (3, None),
            png::ColorType::Rgba => (3, Some(3)),
            // (Expanded: none comes indexed.)
            png::ColorType::Indexed => return None,
        };
        let bytes_per_sample = if sixteen { 2 } else { 1 };
        let mut rgba = vec![0u8; w * h * 4];
        let line = frame.line_size;
        for y in 0..h {
            let row = buffer.get(y * line..y * line + w * channels * bytes_per_sample)?;
            for x in 0..w {
                let px = &row[x * channels * bytes_per_sample..(x + 1) * channels * bytes_per_sample];
                let sample = |i: usize| -> (u8, Option<u16>) {
                    if sixteen {
                        let (hi, lo) = (px[i * 2], px[i * 2 + 1]);
                        (scale_16(hi, lo), Some(u16::from_be_bytes([hi, lo])))
                    } else {
                        (px[i], None)
                    }
                };
                let o = &mut rgba[(y * w + x) * 4..(y * w + x) * 4 + 4];
                for (c, out) in o[..3].iter_mut().enumerate() {
                    let (byte, wide) = sample(if color == 1 { 0 } else { c });
                    *out = match wide {
                        Some(v) => gamma.correct_16(v, byte),
                        None => gamma.correct_8(byte),
                    };
                }
                o[3] = match alpha {
                    Some(a) => sample(a).0,
                    None => 255,
                };
            }
        }
        Some(RgbaImage { width: w as i32, height: h as i32, rgba })
    })
}

/// libpng's png_do_scale_16_to_8: a 16-bit sample rounded to 8 bits (as v * 255 / 65535).
fn scale_16(hi: u8, lo: u8) -> u8 {
    let tmp = hi as i32;
    (tmp + (((lo as i32 - tmp + 128) * 65535) >> 24)) as u8
}

/// The gamma correction libpng makes from the file's gamma to sRGB's: none, or its tables.
struct Gamma {
    /// 8 bits to 8 (png_build_8bit_table).
    table_8: Option<[u8; 256]>,
    /// 16 bits, cut to 16 - `shift`, to 8 (png_build_16to8_table).
    table_16: Option<(u32, Vec<u8>)>,
}

impl Gamma {
    fn of(info: &png::Info) -> Gamma {
        // The file's gamma (libpng 1.6.59's png_resolve_file_gamma, PNGv3's precedence): sRGB's where an sRGB chunk
        // says so, else its gAMA chunk's, else sRGB's (as PNG_IMAGE_FLAG_16BIT_sRGB has it for 16 bits too).
        let file_gamma = match info.gama_chunk.map(|g| g.into_scaled()) {
            _ if info.srgb.is_some() => SRGB_FILE_GAMMA,
            Some(g) if (1..=i32::MAX as u32).contains(&g) => g,
            _ => SRGB_FILE_GAMMA,
        } as f64;
        // png_gamma_threshold: corrected unless file times screen gamma is within 5% of 1 (or past what libpng's
        // fixed point holds).
        let product = (SCREEN_GAMMA * file_gamma / 1e5 + 0.5).floor();
        if product <= i32::MAX as f64 && !significant(product) {
            return Gamma { table_8: None, table_16: None };
        }
        // png_reciprocal2: the exponent from the file's encoding to the screen's (0 where it overflows).
        let correction = fixed(1e15 / SCREEN_GAMMA / file_gamma);
        let mut table_8 = [0u8; 256];
        for (i, t) in table_8.iter_mut().enumerate() {
            *t = if i > 0 && i < 255 && significant(correction) {
                (255.0 * (i as f64 / 255.0).powf(correction * 1e-5) + 0.5).floor() as u8
            } else {
                i as u8
            };
        }
        // png_build_16to8_table: for each 8-bit output, the largest input (cut to 16 - shift bits, the shift from
        // the significant bits, at least 5: 11 bits kept) that comes out nearest it.
        let significant = match &info.sbit {
            Some(bits) if !bits.is_empty() => {
                let rgb = matches!(info.color_type, png::ColorType::Rgb | png::ColorType::Rgba);
                let colors = if rgb { &bits[..bits.len().min(3)] } else { &bits[..1] };
                colors.iter().copied().max().unwrap_or(0) as u32
            }
            _ => 0,
        };
        let mut shift = if significant > 0 && significant < 16 { 16 - significant } else { 0 };
        shift = shift.clamp(16 - 11, 8);
        let max = (1u32 << (16 - shift)) - 1;
        let mut table_16 = vec![255u8; 1 << (16 - shift)];
        let mut last = 0u32;
        // (png_reciprocal of the correction: the exponent back from the screen's encoding to the file's.)
        let back = if correction != 0.0 { fixed(1e10 / correction) } else { 0.0 };
        for i in 0..255u32 {
            let out = i * 257;
            let bound = gamma_16bit_correct(out + 128, back);
            let bound = (bound * max + 32768) / 65535 + 1;
            while last < bound && (last as usize) < table_16.len() {
                table_16[last as usize] = i as u8;
                last += 1;
            }
        }
        Gamma { table_8: Some(table_8), table_16: Some((shift, table_16)) }
    }

    fn correct_8(&self, v: u8) -> u8 {
        self.table_8.map_or(v, |t| t[v as usize])
    }

    /// A 16-bit sample corrected to 8 bits; `scaled` it uncorrected.
    fn correct_16(&self, v: u16, scaled: u8) -> u8 {
        match &self.table_16 {
            Some((shift, table)) => table[(v >> shift) as usize],
            None => scaled,
        }
    }
}

/// libpng's fixed point (times 100000) of a value: rounded, 0 past an i32.
fn fixed(v: f64) -> f64 {
    let r = (v + 0.5).floor();
    if (i32::MIN as f64..=i32::MAX as f64).contains(&r) { r } else { 0.0 }
}

/// libpng's png_gamma_significant: further than 5% (PNG_GAMMA_THRESHOLD_FIXED) from 1.
fn significant(gamma: f64) -> bool {
    !(95_000.0..=105_000.0).contains(&gamma)
}

/// libpng's png_gamma_16bit_correct: a 16-bit value through the exponent `gamma` (times 100000).
fn gamma_16bit_correct(value: u32, gamma: f64) -> u32 {
    if value > 0 && value < 65535 {
        (65535.0 * (value as f64 / 65535.0).powf(gamma * 1e-5) + 0.5).floor() as u32
    } else {
        value
    }
}
