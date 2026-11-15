//! Bitmap glyphs (CBDT, sbix, EBDT strikes) scaled to the size drawn as the C++ Kompoz scaled them
//! (font_system.cpp's colorBitmap): from the strike that suits the size (the smallest at least as big, else the
//! biggest), each target pixel the average of the strike's pixels it covers (a box filter over premultiplied
//! colour, as FreeType decodes it), made straight again; PNGs decoded by kompoz-graphics and premultiplied as
//! FreeType's PNG loader premultiplies them. A coverage mask (a monochrome or gray strike) stays coverage.

use super::color::ColorImage;
use super::raster::Coverage;
use kompoz_graphics::decode_png;
use skrifa::font::glyph::bitmap::{Origin, Pixels};
use skrifa::{Font, GlyphId};

/// A bitmap glyph at a size: colour (straight RGBA) or coverage.
#[derive(Clone, PartialEq, Debug)]
pub enum BitmapImage {
    Color(ColorImage),
    Coverage(Coverage),
}

/// FreeType's multiply_alpha (its PNG loader's premultiplication): color · alpha / 255, rounded.
pub fn multiply_alpha(alpha: u8, color: u8) -> u8 {
    let temp = alpha as u32 * color as u32 + 0x80;
    ((temp + (temp >> 8)) >> 8) as u8
}

/// `glyph`'s bitmap in `font` drawn `px` px (None if no strike has it, or it can't be decoded).
pub fn bitmap_glyph(font: &Font, glyph: u32, px: u32) -> Option<BitmapImage> {
    let bitmap = font.bitmap_strikes().select_bitmap(GlyphId::new(glyph), Some(px as f32))?;
    let (sw, sh) = (bitmap.width() as usize, bitmap.height() as usize);
    // Premultiplied RGBA, or coverage.
    let (pixels, color): (Vec<u8>, bool) = match bitmap.pixels() {
        Pixels::Png(data) => {
            let image = decode_png(data)?;
            let (w, h) = (image.width as usize, image.height as usize);
            if w != sw || h != sh {
                return None;
            }
            let mut rgba = image.rgba;
            for p in rgba.as_chunks_mut::<4>().0 {
                let a = p[3];
                if a == 0 {
                    p.copy_from_slice(&[0; 4]);
                } else if a != 255 {
                    for c in &mut p[..3] {
                        *c = multiply_alpha(a, *c);
                    }
                }
            }
            (rgba, true)
        }
        Pixels::Bgra(bgra) => {
            let mut rgba = Vec::with_capacity(sw * sh * 4);
            for y in 0..sh as u32 {
                for x in 0..sw as u32 {
                    let [b, g, r, a] = bgra.pixel(x, y)?;
                    rgba.extend_from_slice(&[r, g, b, a]);
                }
            }
            (rgba, true)
        }
        Pixels::Mask(mask) => {
            let mut alpha = vec![0u8; mask.decoded_len()];
            if !mask.decode_into(&mut alpha) || alpha.len() != sw * sh {
                return None;
            }
            (alpha, false)
        }
    };
    let strike = bitmap.ppem_y().max(1) as f32;
    let k = px as f32 / strike;
    let units = px as f32 / font.units_per_em().max(1) as f32;
    // Where the image's top left corner is from the origin, in the strike's pixels.
    let top_in_strike = match bitmap.origin() {
        Origin::TopLeft => bitmap.inner_y_bearing(),
        Origin::BottomLeft => bitmap.inner_y_bearing() + sh as i32,
    };
    let left = (bitmap.x_bearing() as f32 * units + bitmap.inner_x_bearing() as f32 * k).round() as i32;
    let top = (bitmap.y_bearing() as f32 * units + top_in_strike as f32 * k).round() as i32;
    if sw == 0 || sh == 0 {
        return Some(BitmapImage::Coverage(Coverage { left, top, ..Coverage::default() }));
    }
    let w = ((sw as f32 * k).round() as usize).max(1);
    let h = ((sh as f32 * k).round() as usize).max(1);
    if w > 1024 || h > 1024 {
        return None;
    }
    let channels = if color { 4 } else { 1 };
    let mut out = vec![0u8; w * h * channels];
    for y in 0..h {
        for x in 0..w {
            // The source pixels this one covers (at least one).
            let x0 = x * sw / w;
            let x1 = ((x + 1) * sw / w).max(x0 + 1);
            let y0 = y * sh / h;
            let y1 = ((y + 1) * sh / h).max(y0 + 1);
            let mut sum = [0.0f32; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = (sy * sw + sx) * channels;
                    for (c, s) in sum.iter_mut().enumerate().take(channels) {
                        *s += pixels[i + c] as f32;
                    }
                }
            }
            let n = ((x1 - x0) * (y1 - y0)) as f32;
            let o = (y * w + x) * channels;
            if color {
                let a = sum[3] / n;
                for c in 0..3 {
                    out[o + c] = if a > 0.0 { (sum[c] / n * 255.0 / a).min(255.0) as u8 } else { 0 };
                }
                out[o + 3] = a as u8;
            } else {
                out[o] = (sum[0] / n) as u8;
            }
        }
    }
    Some(if color {
        BitmapImage::Color(ColorImage { left, top, width: w, height: h, rgba: out })
    } else {
        BitmapImage::Coverage(Coverage { left, top, width: w, height: h, alpha: out })
    })
}
