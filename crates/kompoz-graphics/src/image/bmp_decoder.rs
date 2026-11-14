//! BMP read (as the C++ Kompoz's own reader): 1, 4, 8, 16, 24 and 32 bits a pixel, uncompressed or with bit
//! fields; OS/2's headers too.

use super::codec::{RgbaImage, size_ok};

/// A channel from a pixel by its mask, as a byte.
fn channel(pixel: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let max = (mask >> shift) as u64;
    (((pixel & mask) >> shift) as u64 * 255 / max) as u8
}

/// A BMP file's pixels as straight RGBA; none if they can't be read (run-length encoded, a JPEG or PNG inside, cut
/// short, beyond 32768 pixels each way or 2^26 in all).
pub fn decode_bmp(d: &[u8]) -> Option<RgbaImage> {
    let n = d.len();
    let u16_at = |at: usize| -> u32 { d.get(at..at + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as u32) };
    let u32_at = |at: usize| -> u32 { u16_at(at) | u16_at(at + 2) << 16 };
    if n < 26 || d[0] != b'B' || d[1] != b'M' {
        return None;
    }
    let (offset, header) = (u32_at(10) as usize, u32_at(14) as usize);
    let (width, height, bpp, compression, colors) = if header == 12 {
        // OS/2's: 16-bit sizes, 3-byte palette entries.
        (u16_at(18) as i64, u16_at(20) as u16 as i16 as i64, u16_at(24), 0, 0)
    } else if (40..=124).contains(&header) && n >= 54 {
        (u32_at(18) as i32 as i64, u32_at(22) as i32 as i64, u16_at(28), u32_at(30), u32_at(46))
    } else {
        return None;
    };
    let entry = if header == 12 { 3 } else { 4 };
    let palette_at = 14 + header;
    // (Else the bottom row first.)
    let top_down = height < 0;
    let height = height.abs();
    if width < 0 || !size_ok(width as u64, height as u64) {
        return None;
    }
    // Red, green, blue, alpha.
    let mut masks = [0u32; 4];
    if compression == 3 || compression == 6 {
        // BI_BITFIELDS, BI_ALPHABITFIELDS: the masks after the header's first 40 bytes.
        if bpp != 16 && bpp != 32 {
            return None;
        }
        let count = if compression == 6 || header >= 56 { 4 } else { 3 };
        if n < 54 + count * 4 {
            return None;
        }
        for (i, m) in masks.iter_mut().take(count).enumerate() {
            *m = u32_at(54 + i * 4);
        }
    } else if compression != 0 {
        // Run-length encoded, or a JPEG or PNG inside: not read.
        return None;
    } else if bpp == 16 {
        masks = [0x7C00, 0x03E0, 0x001F, 0];
    } else if bpp == 32 {
        // The fourth byte taken as alpha, as most write it, unless it is 0 everywhere.
        masks = [0xFF0000, 0xFF00, 0xFF, 0xFF000000];
    }
    // RGB.
    let mut palette: Vec<u8> = Vec::new();
    if bpp == 1 || bpp == 4 || bpp == 8 {
        let count = if colors != 0 { colors as usize } else { 1 << bpp };
        if count > 256 || palette_at + count * entry > n {
            return None;
        }
        for i in 0..count {
            let e = &d[palette_at + i * entry..];
            palette.extend_from_slice(&[e[2], e[1], e[0]]);
        }
    } else if bpp != 16 && bpp != 24 && bpp != 32 {
        return None;
    }
    let (w, h) = (width as usize, height as usize);
    let stride = (w * bpp as usize).div_ceil(32) * 4;
    if offset > n || (n - offset) / stride < h {
        return None;
    }
    let mut rgba = vec![0u8; w * h * 4];
    let mut alpha_seen = false;
    for y in 0..h {
        let row_at = offset + stride * if top_down { y } else { h - 1 - y };
        let row = &d[row_at..row_at + stride];
        for x in 0..w {
            let o = &mut rgba[(y * w + x) * 4..(y * w + x) * 4 + 4];
            if !palette.is_empty() {
                let index = match bpp {
                    8 => row[x] as usize,
                    4 => ((row[x / 2] >> if x % 2 == 1 { 0 } else { 4 }) & 15) as usize,
                    _ => ((row[x / 8] >> (7 - x % 8)) & 1) as usize,
                };
                // (Past the palette: black.)
                if let Some(rgb) = palette.get(index * 3..index * 3 + 3) {
                    o[..3].copy_from_slice(rgb);
                }
                o[3] = 255;
            } else if bpp == 24 {
                o.copy_from_slice(&[row[x * 3 + 2], row[x * 3 + 1], row[x * 3], 255]);
            } else {
                let pixel = if bpp == 16 { u16_at(row_at + x * 2) } else { u32_at(row_at + x * 4) };
                o[0] = channel(pixel, masks[0]);
                o[1] = channel(pixel, masks[1]);
                o[2] = channel(pixel, masks[2]);
                o[3] = if masks[3] != 0 { channel(pixel, masks[3]) } else { 255 };
                alpha_seen = alpha_seen || (masks[3] != 0 && o[3] != 0);
            }
        }
    }
    if masks[3] != 0 && !alpha_seen {
        // An alpha of 0 everywhere: not alpha after all.
        rgba.iter_mut().skip(3).step_by(4).for_each(|a| *a = 255);
    }
    Some(RgbaImage { width: w as i32, height: h as i32, rgba })
}
