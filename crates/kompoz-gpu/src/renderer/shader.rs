//! Image shaders and bakes (C++'s paintedShader's image case, bakedShader): an image shader's paint is its bitmap's
//! texture in texture1 (premultiplied, sampled bilinearly, tiled as it says; transparent past a decal edge), where
//! the map in its table row puts the pixel; a composite of composites is baked first, drawn into a texture of its
//! own, then painted from that.
//!
//! Painted by stage 7's images package: until then both paint nothing (transparent), as a shader without anything to
//! paint does.

use super::Renderer;
use crate::shaders::Prim;
use kompoz_graphics::{Projective, Rect, Shader};

impl Renderer {
    /// An image shader's paint at `alpha`, the pixel taken through `map` (from the target's px to the shader's own
    /// space). Painted by stage 7's images package: until then nothing.
    pub(super) fn image_paint(&mut self, shader: &Shader, map: Projective, alpha: f32) -> Prim {
        let _ = (shader, map, alpha);
        Prim::default()
    }

    /// A composite of composites baked: what the area covers in this target's px drawn into a texture of its own,
    /// the second shader onto the first by its blend mode, then painted from that texture. Baked by stage 7's images
    /// package: until then nothing.
    pub(super) fn baked_shader(
        &mut self,
        shader: &Shader,
        area: Rect,
        alpha: f32,
        enlarged: f32,
        before: Option<&Projective>,
    ) -> Prim {
        let _ = (shader, area, alpha, enlarged, before);
        Prim::default()
    }
}
