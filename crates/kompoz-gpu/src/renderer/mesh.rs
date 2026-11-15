//! Meshes (C++'s drawVertices): each triangle a MESH primitive drawn by vsMesh, its vertices where this target's
//! pixels are (x, y and w, through the transform: the rasterizer clips what is behind the eye and works the texture
//! coordinates and colors out in perspective), not anti-aliased, as Skia fills them; painted with the brush's color
//! opaque, or its shader where the texture coordinates are, through the vertex colors by the blend mode.

use super::paint::{blend_index, premultiplied, with_alpha};
use super::{Inputs, Renderer};
use crate::shaders::{MESH, Prim};
use kompoz_graphics::{BlendMode, Brush, BrushKind, Color, Matrix, Offset, Projective, Rect, ShaderKind, Vertices};

/// A triangle mesh: each triangle one primitive (vsMesh). The paint: the brush's color opaque, or its shader where
/// the texture coordinates are (from `origin`; none: where the vertices are), through `before` to this target's px
/// and the shader's own map from there; through the vertex colors (if any) by `blend_mode`; faded by `alpha`.
pub(super) fn draw_vertices(
    r: &mut Renderer,
    vertices: &Vertices,
    origin: Offset,
    blend_mode: BlendMode,
    brush: &Brush,
    brush_area: Rect,
    alpha: f32,
) {
    let triangles = vertices.triangles();
    if triangles.is_empty() || alpha <= 0.0 {
        return;
    }
    let n = vertices.vertex_count();
    if triangles.iter().any(|t| t.iter().any(|&i| i >= n)) {
        return;
    }
    let textured = !vertices.texture_coordinates().is_empty();
    let colors = vertices.colors();
    let sampled = |i: usize| {
        let t = if textured { vertices.texture_coordinate(i) } else { vertices.position(i) };
        Offset::new(origin.x + t.x, origin.y + t.y)
    };
    let shaded = matches!(brush.kind(), BrushKind::Composite(_) | BrushKind::Shader(_));
    let stops = brush.color_stops();
    let mut paint = if brush.is_solid() || (!shaded && brush.transform().is_none() && stops.len() < 2) {
        let c = stops.first().map_or(Color::Transparent, |s| s.color);
        Prim { color: premultiplied(with_alpha(c, 1.0)), ..Prim::default() }
    } else {
        let mut shader = brush.create_shader(brush_area.size());
        let mut area = brush_area;
        let mut enlarged = 1.0f32;
        let composite = |k: Option<ShaderKind>| k == Some(ShaderKind::Composite);
        let nested = shader.kind() == ShaderKind::Composite
            && (composite(shader.dst().map(|s| s.kind())) || composite(shader.src().map(|s| s.kind())));
        if nested {
            // A composite of composites is baked (see painted_shader): over the texture coordinates' bounds (its
            // points from the area's top left still), as finely as the mesh spreads them over the screen (as many
            // texels as px, up to 8 a px of this target's), sampled between its texels.
            let (mut l, mut t, mut right, mut b) = (1e30f32, 1e30f32, -1e30f32, -1e30f32);
            for tri in &triangles {
                let mut s = [Offset::Zero; 3];
                let mut u = [Offset::Zero; 3];
                let mut seen = true;
                for k in 0..3 {
                    let i = tri[k];
                    u[k] = sampled(i);
                    (l, t, right, b) = (l.min(u[k].x), t.min(u[k].y), right.max(u[k].x), b.max(u[k].y));
                    let p = vertices.position(i);
                    let (q, w) =
                        r.transform.apply_w((origin.x + p.x) * r.scale - r.ox, (origin.y + p.y) * r.scale - r.oy);
                    s[k] = q;
                    seen = seen && w > 0.0;
                }
                // How far a px of texture coordinate goes on the screen: the map between them's size (its
                // Frobenius norm, at least its largest stretch).
                let (a1, b1) = (u[1].x - u[0].x, u[2].x - u[0].x);
                let (c1, d1) = (u[1].y - u[0].y, u[2].y - u[0].y);
                let det = a1 * d1 - b1 * c1;
                if !seen || det.abs() < 1e-12 {
                    continue;
                }
                let (sx1, sx2, sy1, sy2) = (s[1].x - s[0].x, s[2].x - s[0].x, s[1].y - s[0].y, s[2].y - s[0].y);
                let (j00, j01) = ((sx1 * d1 - sx2 * c1) / det, (sx2 * a1 - sx1 * b1) / det);
                let (j10, j11) = ((sy1 * d1 - sy2 * c1) / det, (sy2 * a1 - sy1 * b1) / det);
                let norm = (j00 * j00 + j01 * j01 + j10 * j10 + j11 * j11).sqrt() / r.scale;
                enlarged = enlarged.max(norm);
            }
            enlarged = enlarged.min(8.0);
            area = Rect::new(l, t, right, b);
            let mut shift = Matrix::new();
            shift.translate(brush_area.left - area.left, brush_area.top - area.top, 0.0);
            shader = shader.transformed(&shift);
        }
        // (The point the paint is asked for: px to this target's px, enlarged.)
        let k = r.scale * enlarged;
        let to_pixels = Projective { m: [k, 0.0, -r.ox * enlarged, 0.0, k, -r.oy * enlarged, 0.0, 0.0, 1.0] };
        r.painted_shader(&shader, area, 1.0, enlarged, Some(&to_pixels))
    };
    paint.params = [blend_index(blend_mode) as f32, alpha, if colors.is_empty() { 0.0 } else { 1.0 }, 0.0];
    let m = r.transform.m;
    for tri in &triangles {
        let mut p = paint;
        let mut xyw = [[0.0f32; 3]; 3];
        let mut uv = [[0.0f32; 2]; 3];
        let mut rgba = [[0.0f32; 4]; 3];
        for k in 0..3 {
            let i = tri[k];
            let at = vertices.position(i);
            let (x, y) = ((origin.x + at.x) * r.scale - r.ox, (origin.y + at.y) * r.scale - r.oy);
            xyw[k] = [m[0] * x + m[1] * y + m[2], m[3] * x + m[4] * y + m[5], m[6] * x + m[7] * y + m[8]];
            let q = sampled(i);
            uv[k] = [q.x, q.y];
            if let Some(&argb) = colors.get(i) {
                rgba[k] = premultiplied(Color::from_argb(argb));
            }
        }
        p.bounds = [xyw[0][0], xyw[0][1], xyw[0][2], xyw[1][0]];
        p.shape = [xyw[1][1], xyw[1][2], xyw[2][0], xyw[2][1]];
        p.radii = [xyw[2][2], uv[0][0], uv[0][1], uv[1][0]];
        p.uv = [uv[1][1], uv[2][0], uv[2][1], 0.0];
        p.xform = rgba[0];
        p.extra = rgba[1];
        p.projection = rgba[2];
        r.add(MESH, &p, Inputs { in_target_pixels: true, ..Inputs::default() });
    }
}
