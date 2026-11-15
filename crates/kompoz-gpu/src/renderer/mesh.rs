//! Meshes (C++'s drawVertices): each triangle a MESH primitive drawn by vsMesh, its vertices where this target's
//! pixels are (x, y and w, through the transform: the rasterizer clips what is behind the eye and works the texture
//! coordinates and colors out in perspective), not anti-aliased, as Skia fills them; painted with the brush's color
//! opaque, or its shader where the texture coordinates are, through the vertex colors by the blend mode.
//!
//! Drawn by stage 7's images package: until then a mesh draws nothing.

use super::Renderer;
use kompoz_graphics::{BlendMode, Brush, Offset, Rect, Vertices};

/// A triangle mesh. Drawn by stage 7's images package: until then nothing.
pub(super) fn draw_vertices(
    r: &mut Renderer,
    vertices: &Vertices,
    origin: Offset,
    blend_mode: BlendMode,
    brush: &Brush,
    brush_area: Rect,
    alpha: f32,
) {
    let _ = (r, vertices, origin, blend_mode, brush, brush_area, alpha);
}
