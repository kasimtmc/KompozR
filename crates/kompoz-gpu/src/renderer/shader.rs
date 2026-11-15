//! Image shaders and bakes (C++'s paintedShader's image case, bakedShader): an image shader's paint is its bitmap's
//! texture in texture1 (premultiplied, sampled bilinearly, tiled as it says; transparent past a decal edge), where
//! the map in its table row puts the pixel; a composite of composites is baked first, drawn into a texture of its
//! own, then painted from that.

use super::{Inputs, MAX_LAYER, PaintTexture, Renderer};
use crate::device::{ClearColor, Filter, Format, Sampler, Wrap};
use crate::shaders::{FILL, IMAGE_PAINT, Prim};
use kompoz_graphics::{Matrix, Projective, Rect, Shader, TileMode};

/// A tile mode as a sampler wraps (decal: clamped, made transparent past the edge in the shader).
fn wrap(mode: TileMode) -> Wrap {
    match mode {
        TileMode::Repeated => Wrap::Repeat,
        TileMode::Mirror => Wrap::Mirror,
        _ => Wrap::Clamp,
    }
}

impl Renderer {
    /// An image shader's paint at `alpha`, the pixel taken through `map` (from the target's px to the shader's own
    /// space), then to the image's uv: its texture in texture1, sampled bilinearly and wrapped as its tile modes say
    /// (flags 16 and 32: transparent past its left and right, top and bottom).
    pub(super) fn image_paint(&mut self, shader: &Shader, map: Projective, alpha: f32) -> Prim {
        let Some(image) = shader.image() else { return Prim::default() };
        let Some(texture) = self.image_texture(image, false) else { return Prim::default() };
        let map = Projective::scaling(1.0 / image.width() as f32, 1.0 / image.height() as f32) * map; // to its uv
        let sampler = Sampler {
            filter: Filter::Linear,
            wrap_u: wrap(shader.tile_mode()),
            wrap_v: wrap(shader.tile_mode_y()),
            mipmaps: false,
        };
        self.paint_textures.push(PaintTexture { texture, sampler });
        let decal = (if shader.tile_mode() == TileMode::Decal { 16 } else { 0 })
            | (if shader.tile_mode_y() == TileMode::Decal { 32 } else { 0 });
        let index = (self.paint_textures.len() - 1) as f32;
        let row = self.shader_row(None, &map);
        Prim {
            color: [1.0, 1.0, 1.0, alpha],
            paint: [IMAGE_PAINT as f32, (8 | decal) as f32, index, row as f32],
            ..Prim::default()
        }
    }

    /// A composite of composites baked: what the area covers in this target's px (at most MAX_LAYER across, lower
    /// in resolution if it would be more) drawn into a texture of its own (in half floats if the target is), the
    /// second shader onto the first by its blend mode (the renderer's own blending, each a paint of its own), then
    /// painted from that texture. `enlarged` and `before` as painted_shader's.
    pub(super) fn baked_shader(
        &mut self,
        shader: &Shader,
        area: Rect,
        alpha: f32,
        enlarged: f32,
        before: Option<&Projective>,
    ) -> Prim {
        let (Some(dst), Some(src)) = (shader.dst(), shader.src()) else { return Prim::default() };
        let k = enlarged * self.scale;
        let (x0, y0) = ((area.left * self.scale - self.ox) * enlarged, (area.top * self.scale - self.oy) * enlarged);
        let (w, h) = (area.width() * k, area.height() * k);
        let r = (MAX_LAYER as f32 / w.max(h).max(1.0)).min(1.0);
        let (bw, bh) = (((w * r).ceil() as i32).max(1), ((h * r).ceil() as i32).max(1));
        let fine = self.pass_index().is_some_and(|p| self.passes[p].target.format() == Format::Rgba16f);
        let Some(bake) = self.acquire(bw, bh, if fine { Format::Rgba16f } else { Format::Rgba8 }) else {
            return Prim::default();
        };
        // Its children, through its own transform (after theirs).
        let through = |one: &Shader| shader.transform().map_or_else(|| one.clone(), |t: &Matrix| one.transformed(t));
        let (dst, src) = (through(dst), through(src));
        let (s0, ox0, oy0) = (self.scale, self.ox, self.oy);
        self.begin_target(bake, Some(ClearColor::TRANSPARENT));
        // The bake's px: the area's px from its top left, r times as many.
        self.scale = s0 * enlarged * r;
        self.ox = (ox0 * enlarged + x0) * r;
        self.oy = (oy0 * enlarged + y0) * r;
        let fill = |renderer: &mut Renderer, s: &Shader| {
            let mut q = renderer.painted_shader(s, area, 1.0, 1.0, None);
            q.bounds = [-1.0, -1.0, bw as f32 + 1.0, bh as f32 + 1.0];
            q.shape = [0.0, 0.0, bw as f32, bh as f32];
            renderer.add(FILL, &q, Inputs::default());
        };
        fill(self, &dst);
        self.blends.push(shader.blend_mode());
        fill(self, &src);
        self.blends.pop();
        self.end_target();
        self.scale = s0;
        // This target's px to the bake's uv.
        let (tw, th) = (bake.width() as f32, bake.height() as f32);
        let mut map = Projective { m: [r / tw, 0.0, -x0 * r / tw, 0.0, r / th, -y0 * r / th, 0.0, 0.0, 1.0] };
        if let Some(before) = before {
            map = map * *before;
        }
        self.paint_textures.push(PaintTexture { texture: bake, sampler: Sampler::default() });
        let index = (self.paint_textures.len() - 1) as f32;
        let row = self.shader_row(None, &map);
        Prim { color: [1.0, 1.0, 1.0, alpha], paint: [IMAGE_PAINT as f32, 8.0, index, row as f32], ..Prim::default() }
    }
}
