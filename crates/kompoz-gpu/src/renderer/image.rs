//! Images (C++'s imageTexture, drawImage, drawImageMask, drawIntoBitmap, readBackBitmap, releaseBitmaps): a bitmap's
//! texture, made from its pixels premultiplied (so that what filtering mixes is weighed by how much of it there is,
//! as Skia samples: a transparent pixel's color doesn't bleed into its neighbors), 8 bits a channel or half floats
//! (F16), mipmapped where it is sampled so; kept while the bitmap holds what it held; drawn as an IMAGE primitive,
//! sampled as its FilterQuality says, or (an Alpha8 one as a mask) as a GLYPH primitive in a brush.
//!
//! Bitmaps are drawn into here too (a canvas over an ImageBitmap: kompoz-ui's BitmapRenderer, which
//! [`RendererBitmaps`] is): into a texture of their own, kept as the bitmap's where its config keeps 8 or 16 bits a
//! channel with alpha (its pixels read back from it when they are read), else read back into it at once.

use super::{Inputs, Renderer};
use crate::device::{ClearColor, Filter, Format, IRect, Sampler, Texture, TextureId, TextureUsage};
use crate::shaders::{GLYPH, IMAGE, Prim};
use kompoz_graphics::{BlendMode, Brush, Color, FilterQuality, ImageBitmap, ImageBitmapConfig, Rect, WeakImageBitmap};
use kompoz_ui::draw::{BitmapRenderer, Picture};
use kompoz_ui::graphics_context::CornerRadii;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

/// A bitmap's texture.
#[derive(Debug)]
pub(super) struct ImageTexture {
    pub(super) texture: Texture,
    /// The bitmap, held weakly: gone, so is its texture (at the frame's end).
    pub(super) token: WeakImageBitmap,
    pub(super) mipmapped: bool,
    /// What of the bitmap it holds (ImageBitmap::version).
    pub(super) version: u64,
    /// Drawn into here: the bitmap's pixels may be this texture's alone (read back when they are read).
    pub(super) kept: bool,
}

/// What was drawn into bitmaps while the renderer was busy drawing (a canvas over a bitmap gone in the middle of a
/// frame's drawing: see [`RendererBitmaps`]), in order, the bitmaps held weakly.
type Waiting = Rc<RefCell<Vec<(WeakImageBitmap, Picture)>>>;

/// The bitmaps' textures, by their ids, and those replaced this frame.
#[derive(Debug, Default)]
pub(super) struct ImageState {
    pub(super) textures: HashMap<u64, ImageTexture>,
    /// Bitmaps' textures drawn again: gone after the frame (its draws read them).
    pub(super) retired: Vec<Texture>,
    /// Shared with the renderer's [`RendererBitmaps`]: drawn into their bitmaps before anything is drawn from a
    /// bitmap, read back from one or flushed.
    waiting: Waiting,
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

/// A half float's bits as a float.
pub(super) fn from_half(h: u16) -> f32 {
    let (e, m) = ((h >> 10) & 0x1f, (h & 0x3ff) as f32);
    let v = match e {
        0 => m * (-24.0f32).exp2(),
        31 if m != 0.0 => f32::NAN,
        31 => f32::INFINITY,
        _ => (m + 1024.0) * (e as f32 - 25.0).exp2(),
    };
    if h & 0x8000 != 0 { -v } else { v }
}

/// Half floats' bits (as a texture's texels are read back) as floats.
fn halves_to_floats(texels: &[u8]) -> Vec<f32> {
    texels.as_chunks::<2>().0.iter().map(|&b| from_half(u16::from_ne_bytes(b))).collect()
}

impl Renderer {
    /// Its texture: the one it has here while it holds what the bitmap does (and is mipmapped if that is asked for),
    /// else made from its pixels (read back first, where only this renderer had them): premultiplied, 8 bits a
    /// channel or (F16) half floats. What waits to be drawn into bitmaps is drawn first.
    pub(super) fn image_texture(&mut self, image: &ImageBitmap, mipmapped: bool) -> Option<TextureId> {
        self.draw_waiting_bitmaps();
        let version = image.version();
        if let Some(t) = self.images.textures.get(&image.id())
            && t.version == version
            && (t.mipmapped || !mipmapped)
        {
            return Some(t.texture.id());
        }
        // (Read back here rather than through the bitmap, which would ask the renderer set for it: this one, busy.)
        if !image.pixels_current() {
            self.read_back_bitmap(image);
        }
        let (w, h) = (image.width(), image.height());
        let f16 = image.config() == ImageBitmapConfig::F16;
        let usage = if mipmapped { TextureUsage::SAMPLED | TextureUsage::MIPMAPPED } else { TextureUsage::SAMPLED };
        let texture = self.device.create_texture(w, h, if f16 { Format::Rgba16f } else { Format::Rgba8 }, usage)?;
        let n = w as usize * h as usize * 4;
        if f16 {
            let straight = image.floats();
            let mut halves = vec![0u8; n * 2];
            for (i, px) in straight.as_chunks::<4>().0.iter().take(n / 4).enumerate() {
                for k in 0..4 {
                    let v = if k < 3 { px[k] * px[3] } else { px[3] };
                    halves[(i * 4 + k) * 2..(i * 4 + k) * 2 + 2].copy_from_slice(&to_half(v).to_ne_bytes());
                }
            }
            self.device.upload(texture.id(), IRect::new(0, 0, w, h), &halves, w as usize * 8);
        } else {
            let mut premultiplied = image.pixels()[..n].to_vec();
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
        let entry = ImageTexture { texture, token: image.downgrade(), mipmapped, version, kept: false };
        if let Some(before) = self.images.textures.insert(image.id(), entry) {
            self.images.retired.push(before.texture);
        }
        Some(id)
    }

    /// What waits to be drawn into bitmaps (see [`RendererBitmaps`]), drawn into them now, in order (those still
    /// about).
    pub(super) fn draw_waiting_bitmaps(&mut self) {
        loop {
            let waiting = std::mem::take(&mut *self.images.waiting.borrow_mut());
            if waiting.is_empty() {
                return;
            }
            for (bitmap, draws) in waiting {
                if let Some(bitmap) = bitmap.upgrade() {
                    self.draw_into_bitmap(&bitmap, &draws);
                }
            }
        }
    }

    /// `texture` into the target as it is, texel for pixel (written, not blended; drawn into a bitmap over what it
    /// held).
    fn copy_in(&mut self, texture: TextureId) {
        let (w, h) = (texture.width() as f32, texture.height() as f32);
        let p = Prim {
            bounds: [0.0, 0.0, w, h],
            shape: [0.0, 0.0, w, h], // (an image's texels go across its shape)
            uv: [0.0, 0.0, 1.0, 1.0],
            params: [1.0, 0.0, 0.0, 0.0],
            ..Prim::default()
        };
        let sampler = Sampler { filter: Filter::Nearest, ..Sampler::default() }; // (texel for pixel, on any GPU)
        self.add(IMAGE, &p, Inputs { opaque: true, ..Inputs::texture(texture, sampler) });
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
    r.add(IMAGE, &p, Inputs::texture(texture, sampler_for(quality)));
}

/// The sampler for a FilterQuality: nearest for None and High (the cubic is the shader's, each texel at its
/// center), else bilinear; from the mipmaps at Medium.
fn sampler_for(quality: FilterQuality) -> Sampler {
    let cubic = quality == FilterQuality::High;
    let filter = if quality == FilterQuality::None || cubic { Filter::Nearest } else { Filter::Linear };
    Sampler { filter, mipmaps: quality == FilterQuality::Medium, ..Sampler::default() }
}

/// An Alpha8 bitmap as a mask (as Android draws one): the brush, covering each pixel as much as the bitmap's alpha
/// there says, sampled as draw_image samples (a glyph's quad, its texture the bitmap's).
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
    let Some(texture) = r.image_texture(image, quality == FilterQuality::Medium) else { return };
    if alpha <= 0.0 {
        return;
    }
    let s = r.scale;
    let (x, y) = (dst.left * s - r.ox, dst.top * s - r.oy);
    let mut p = r.painted(brush, brush_area, alpha, 1.0);
    p.bounds = [x, y, x + dst.width() * s, y + dst.height() * s];
    p.shape = p.bounds;
    let (iw, ih) = (image.width() as f32, image.height() as f32);
    p.uv = [src.left / iw, src.top / ih, src.right / iw, src.bottom / ih];
    p.params = [0.0, if quality == FilterQuality::High { 1.0 } else { 0.0 }, 0.5 / iw, 0.5 / ih];
    r.add(GLYPH, &p, Inputs::texture(texture, sampler_for(quality)));
}

impl Renderer {
    /// A bitmap drawn into (a canvas over an ImageBitmap): `draws` over what it holds, a unit a pixel, into a new
    /// texture of its own; kept here where the bitmap keeps 8 or 16 bits a channel with alpha (Argb8888, Gpu, F16:
    /// the bitmap told its pixels are the renderer's, read back when they are read) and Alpha8 (its alpha alone,
    /// black, as it reads back and is sampled), else read back into it at once (flushing what is recorded), kept as
    /// its config keeps it. [`flush`](Renderer::flush) submits the drawing.
    pub fn draw_into_bitmap(&mut self, bitmap: &ImageBitmap, draws: &Picture) {
        let (w, h) = (bitmap.width(), bitmap.height());
        let config = bitmap.config();
        let f16 = config == ImageBitmapConfig::F16;
        let alpha8 = config == ImageBitmapConfig::Alpha8;
        let kept = bitmap.has_alpha()
            && matches!(
                config,
                ImageBitmapConfig::Argb8888
                    | ImageBitmapConfig::Gpu
                    | ImageBitmapConfig::F16
                    | ImageBitmapConfig::Alpha8
            );
        let Some(before) = self.image_texture(bitmap, false) else { return };
        let format = if f16 { Format::Rgba16f } else { Format::Rgba8 };
        let usage = TextureUsage::SAMPLED | TextureUsage::RENDER_TARGET;
        let Some(fresh) = self.device.create_texture(w, h, format, usage) else { return };
        let s0 = self.scale;
        self.scale = 1.0; // (a unit a pixel)
        self.begin_target(fresh.id(), Some(ClearColor::TRANSPARENT));
        self.copy_in(before);
        draws.replay(self);
        if alpha8 {
            // Its alpha alone (black, as an Alpha8 one reads back and is sampled).
            super::push_blend_mode(self, BlendMode::SrcIn);
            let all = Rect::new(0.0, 0.0, w as f32, h as f32);
            super::prims::fill_round_rect(self, all, CornerRadii::Zero, Color::Black);
            super::pop_blend_mode(self);
        }
        self.end_target();
        self.scale = s0;
        if kept {
            bitmap.drawn_on_renderer();
            let entry = ImageTexture {
                texture: fresh,
                token: bitmap.downgrade(),
                mipmapped: false,
                version: bitmap.version(),
                kept: true,
            };
            if let Some(before) = self.images.textures.insert(bitmap.id(), entry) {
                self.images.retired.push(before.texture);
            }
            return;
        }
        self.flush();
        let pixels = self.device.read_pixels(fresh.id(), IRect::new(0, 0, w, h));
        if f16 {
            bitmap.store_floats(&halves_to_floats(&pixels), true);
        } else {
            bitmap.store(&pixels, true);
        }
        self.images.retired.push(fresh);
    }

    /// The pixels of a bitmap only this renderer has, read back into it (flushing what is recorded); what waits to be
    /// drawn into bitmaps drawn first.
    pub fn read_back_bitmap(&mut self, bitmap: &ImageBitmap) {
        self.draw_waiting_bitmaps();
        let Some(t) = self.images.textures.get(&bitmap.id()) else { return };
        if t.version != bitmap.version() {
            return;
        }
        let texture = t.texture.id();
        self.flush();
        let pixels = self.device.read_pixels(texture, IRect::new(0, 0, bitmap.width(), bitmap.height()));
        if texture.format() == Format::Rgba16f {
            bitmap.store_floats(&halves_to_floats(&pixels), false);
        } else {
            bitmap.store(&pixels, false);
        }
    }

    /// Every bitmap's pixels that only this renderer has, read back into them (before it goes).
    pub fn release_bitmaps(&mut self) {
        self.draw_waiting_bitmaps();
        let kept: Vec<ImageBitmap> = self
            .images
            .textures
            .values()
            .filter(|t| t.kept)
            .filter_map(|t| t.token.upgrade())
            .filter(|b| !b.pixels_current())
            .collect();
        for b in &kept {
            self.read_back_bitmap(b);
        }
    }
}

/// What draws into bitmaps on the GPU (kompoz-ui's [`BitmapRenderer`], set with
/// [`set_bitmap_renderer`](kompoz_ui::draw::set_bitmap_renderer); the C++ Kompoz app's AppBitmaps): the renderer's
/// [`draw_into_bitmap`](Renderer::draw_into_bitmap) in a frame of its own between frames, or in the frame going on
/// (see [`set_in_frame`](RendererBitmaps::set_in_frame)); pixels read back, and released, the same way.
///
/// While the renderer is drawing (borrowed: a canvas over a bitmap went in the middle of a frame's drawing, which
/// holds it) what is drawn into a bitmap waits in it, and is drawn into the bitmap before anything is drawn from a
/// bitmap or read back, at the latest as the frame is flushed: in the order it was drawn, before what is drawn after
/// it. Pixels read meanwhile (from what draws the frame) are those the bitmap held before.
pub struct RendererBitmaps {
    renderer: Weak<RefCell<Renderer>>,
    waiting: Waiting,
    in_frame: Cell<bool>,
}

impl RendererBitmaps {
    /// Drawing into bitmaps with `renderer`.
    pub fn new(renderer: &Rc<RefCell<Renderer>>) -> Rc<RendererBitmaps> {
        let waiting = renderer.borrow().images.waiting.clone();
        Rc::new(RendererBitmaps { renderer: Rc::downgrade(renderer), waiting, in_frame: Cell::new(false) })
    }

    /// Whether the platform is in a frame (between the device's begin_frame and end_frame): what is drawn into
    /// bitmaps then is drawn in that frame, not in one of its own.
    pub fn set_in_frame(&self, on: bool) {
        self.in_frame.set(on);
    }

    /// `then` with the renderer, in a frame of its own unless one is going on (and flushed then if `flush`); false if
    /// the renderer is busy (true if it is gone: nothing is drawn by it any more).
    fn with_renderer(&self, flush: bool, then: impl FnOnce(&mut Renderer)) -> bool {
        let Some(renderer) = self.renderer.upgrade() else { return true };
        let Ok(mut r) = renderer.try_borrow_mut() else { return false };
        let own = !self.in_frame.get();
        if own {
            r.device_mut().begin_frame();
        }
        then(&mut r);
        if own {
            if flush {
                r.flush();
            }
            r.device_mut().end_frame(false);
        }
        true
    }
}

impl BitmapRenderer for RendererBitmaps {
    fn draw(&self, bitmap: &ImageBitmap, draws: &Picture) {
        let drawn = self.with_renderer(true, |r| {
            r.draw_waiting_bitmaps();
            r.draw_into_bitmap(bitmap, draws);
        });
        if !drawn {
            self.waiting.borrow_mut().push((bitmap.downgrade(), draws.clone()));
        }
    }

    fn read_back(&self, bitmap: &ImageBitmap) {
        self.with_renderer(false, |r| r.read_back_bitmap(bitmap));
    }

    fn release(&self) {
        self.with_renderer(false, |r| r.release_bitmaps());
    }
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

    #[test]
    fn half_floats_come_back_as_they_went() {
        for v in [0.0f32, 1.0, -2.0, 0.5, 65504.0, 5.960_464_5e-8, 0.333_251_95, -6.103_515_6e-5] {
            assert_eq!(from_half(to_half(v)), v);
        }
        assert_eq!(from_half(0x7c00), f32::INFINITY);
        assert_eq!(from_half(0xfc00), f32::NEG_INFINITY);
        assert!(from_half(0x7e00).is_nan());
        assert_eq!(from_half(0x8000).to_bits(), (-0.0f32).to_bits()); // (the sign of zero kept)
        let bytes: Vec<u8> = [0x3c00u16, 0x3800].iter().flat_map(|h| h.to_ne_bytes()).collect();
        assert_eq!(halves_to_floats(&bytes), vec![1.0, 0.5]);
    }
}
