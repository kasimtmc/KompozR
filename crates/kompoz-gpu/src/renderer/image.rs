//! Images (C++'s imageTexture, drawImage, drawImageMask, drawIntoBitmap, readBackBitmap, releaseBitmaps): a bitmap's
//! texture, made from its pixels premultiplied (so that what filtering mixes is weighed by how much of it there is,
//! as Skia samples: a transparent pixel's color doesn't bleed into its neighbors), 8 bits a channel or half floats
//! (F16), mipmapped where it is sampled so; kept while the bitmap holds what it held; drawn as an IMAGE primitive,
//! sampled as its FilterQuality says.
//!
//! An Alpha8 bitmap as a mask and the bitmaps drawn into (BitmapRenderer) are drawn by stage 7's images package: until
//! then a mask is GraphicsContext's default (a layer, the image over the brush by DstIn) and nothing is drawn into a
//! bitmap here.

use super::{Inputs, Renderer};
use crate::device::{Filter, Format, IRect, Sampler, Texture, TextureId, TextureUsage};
use crate::shaders::{IMAGE, Prim};
use kompoz_graphics::{Brush, FilterQuality, ImageBitmap, ImageBitmapConfig, Rect, WeakImageBitmap};
use kompoz_ui::draw::Picture;
use kompoz_ui::graphics_context::default_draw_image_mask;
use std::collections::HashMap;

/// A bitmap's texture.
#[derive(Debug)]
pub(super) struct ImageTexture {
    pub(super) texture: Texture,
    /// The bitmap, held weakly: gone, so is its texture (at the frame's end).
    pub(super) token: WeakImageBitmap,
    pub(super) mipmapped: bool,
    /// What of the bitmap it holds (ImageBitmap::version).
    pub(super) version: u64,
    /// A bitmap drawn into here: whose pixels only this texture may have.
    #[allow(dead_code)] // (read by stage 7's images package: release_bitmaps)
    pub(super) kept: Option<WeakImageBitmap>,
}

/// The bitmaps' textures, by their ids, and those replaced this frame.
#[derive(Debug, Default)]
pub(super) struct ImageState {
    pub(super) textures: HashMap<u64, ImageTexture>,
    /// Bitmaps' textures drawn again: gone after the frame (its draws read them).
    pub(super) retired: Vec<Texture>,
}

/// A float as a half float's bits, rounded to the nearest.
pub(super) fn to_half(v: f32) -> u16 {
    let f = v.to_bits();
    let sign = ((f >> 16) & 0x8000) as u16;
    let e = (f >> 23) & 0xff;
    if e == 0xff {
        return sign | 0x7c00 | if f & 0x7f_ffff != 0 { 0x200 } else { 0 };
    }
    let a = v.abs();
    if a >= 65520.0 {
        return sign | 0x7c00;
    }
    if a < 6.103_515_6e-5 {
        return sign | (a * 16_777_216.0).round() as u16; // subnormal: steps of 2^-24
    }
    // a = m 2^exp, m in [1, 2).
    let exp = ((a.to_bits() >> 23) & 0xff) as i32 - 127;
    let m = a / (exp as f32).exp2();
    let mut mantissa = ((m - 1.0) * 1024.0).round() as u32;
    let mut he = (exp + 15) as u32;
    if mantissa == 1024 {
        mantissa = 0;
        he += 1;
    }
    sign | (he << 10) as u16 | mantissa as u16
}

impl Renderer {
    /// Its texture: the one it has here while it holds what the bitmap does (and is mipmapped if that is asked for),
    /// else made from its pixels: premultiplied, 8 bits a channel or (F16) half floats.
    pub(super) fn image_texture(&mut self, image: &ImageBitmap, mipmapped: bool) -> Option<TextureId> {
        let version = image.version();
        if let Some(t) = self.images.textures.get(&image.id())
            && t.version == version
            && (t.mipmapped || !mipmapped)
        {
            return Some(t.texture.id());
        }
        let (w, h) = (image.width(), image.height());
        let f16 = image.config() == ImageBitmapConfig::F16;
        let usage = if mipmapped { TextureUsage::SAMPLED | TextureUsage::MIPMAPPED } else { TextureUsage::SAMPLED };
        let texture = self.device.create_texture(w, h, if f16 { Format::Rgba16f } else { Format::Rgba8 }, usage)?;
        let n = w as usize * h as usize * 4;
        if f16 {
            let straight = image.floats(); // (read back if only a renderer has them)
            let mut halves = vec![0u8; n * 2];
            for (i, px) in straight.as_chunks::<4>().0.iter().take(n / 4).enumerate() {
                for k in 0..4 {
                    let v = if k < 3 { px[k] * px[3] } else { px[3] };
                    halves[(i * 4 + k) * 2..(i * 4 + k) * 2 + 2].copy_from_slice(&to_half(v).to_ne_bytes());
                }
            }
            self.device.upload(texture.id(), IRect::new(0, 0, w, h), &halves, w as usize * 8);
        } else {
            let mut premultiplied = image.pixels()[..n].to_vec(); // (read back if only a renderer has them)
            for px in premultiplied.as_chunks_mut::<4>().0 {
                let a = px[3] as u32;
                for c in &mut px[..3] {
                    *c = ((*c as u32 * a + 127) / 255) as u8;
                }
            }
            self.device.upload(texture.id(), IRect::new(0, 0, w, h), &premultiplied, w as usize * 4);
        }
        if mipmapped {
            self.device.generate_mipmaps(texture.id());
        }
        let id = texture.id();
        // (In place of one without mipmaps, or of what it held before.)
        let entry = ImageTexture { texture, token: image.downgrade(), mipmapped, version, kept: None };
        if let Some(before) = self.images.textures.insert(image.id(), entry) {
            self.images.retired.push(before.texture);
        }
        Some(id)
    }
}

/// Sampled as `quality` says: the nearest texel, bilinearly, bilinearly from the mipmap level nearest the size it is
/// drawn at (Skia's Medium), or a Mitchell cubic of the sixteen nearest (Skia's High, in the shader: each texel at its
/// center).
pub(super) fn draw_image(
    r: &mut Renderer,
    image: &ImageBitmap,
    src: Rect,
    dst: Rect,
    alpha: f32,
    quality: FilterQuality,
) {
    let Some(texture) = r.image_texture(image, quality == FilterQuality::Medium) else { return };
    if alpha <= 0.0 {
        return;
    }
    let s = r.scale;
    let (x, y) = (dst.left * s - r.ox, dst.top * s - r.oy);
    let bounds = [x, y, x + dst.width() * s, y + dst.height() * s];
    let (iw, ih) = (image.width() as f32, image.height() as f32);
    let cubic = quality == FilterQuality::High;
    let p = Prim {
        bounds,
        shape: bounds,
        uv: [src.left / iw, src.top / ih, src.right / iw, src.bottom / ih],
        params: [alpha, if cubic { 1.0 } else { 0.0 }, 0.5 / iw, 0.5 / ih],
        ..Prim::default()
    };
    let filter = if quality == FilterQuality::None || cubic { Filter::Nearest } else { Filter::Linear };
    let sampler = Sampler { filter, mipmaps: quality == FilterQuality::Medium, ..Sampler::default() };
    r.add(IMAGE, &p, Inputs::texture(texture, sampler));
}

/// An Alpha8 bitmap as a mask (as Android draws one). Drawn by stage 7's images package (a GLYPH primitive, its
/// texture the bitmap's): until then GraphicsContext's default.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_image_mask(
    r: &mut Renderer,
    image: &ImageBitmap,
    src: Rect,
    dst: Rect,
    brush: &Brush,
    brush_area: Rect,
    alpha: f32,
    quality: FilterQuality,
) {
    default_draw_image_mask(r, image, src, dst, brush, brush_area, alpha, quality);
}

impl Renderer {
    /// A bitmap drawn into (a canvas over an ImageBitmap): `draws` over what it holds, a unit a pixel, into a new
    /// texture of its own; kept here where the bitmap keeps 8 or 16 bits a channel with alpha, else read back into it
    /// at once. Drawn by stage 7's images package: until then nothing is drawn into it.
    pub fn draw_into_bitmap(&mut self, bitmap: &ImageBitmap, draws: &Picture) {
        let _ = (bitmap, draws);
    }

    /// The pixels of a bitmap only this renderer has, read back into it (flushing what is recorded). Read by stage
    /// 7's images package: until then the renderer keeps no bitmap's pixels of its own, and none is read.
    pub fn read_back_bitmap(&mut self, bitmap: &ImageBitmap) {
        let _ = bitmap;
    }

    /// Every bitmap's pixels that only this renderer has, read back before it goes. Read by stage 7's images
    /// package: until then it has none.
    pub fn release_bitmaps(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_floats_round_to_the_nearest() {
        assert_eq!(to_half(0.0), 0);
        assert_eq!(to_half(1.0), 0x3c00);
        assert_eq!(to_half(-2.0), 0xc000);
        assert_eq!(to_half(0.5), 0x3800);
        assert_eq!(to_half(65504.0), 0x7bff);
        assert_eq!(to_half(1e6), 0x7c00);
        assert_eq!(to_half(f32::NEG_INFINITY), 0xfc00);
        assert_eq!(to_half(f32::NAN) & 0x7e00, 0x7e00);
        assert_eq!(to_half(1.0 + 1.0 / 2048.0 + 1e-6), 0x3c01); // (past half a step: up)
        assert_eq!(to_half(5.960_464_5e-8), 1); // the least subnormal
        assert_eq!(to_half(0.333_333_34), 0x3555);
    }
}
