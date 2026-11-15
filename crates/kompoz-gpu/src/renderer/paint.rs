//! What covers a primitive (C++'s Renderer::painted, paintedShader, shaderRow): its color premultiplied, or a
//! gradient whose stops go into a row of the frame's table (RGBA32F, fetched texel by texel), its geometry in the
//! target's pixels; a shader's paint worked out where each pixel is from the map in the last three texels of its row
//! (a gradient's, a composite's two paints; an image's and a bake's are [`shader`](super::shader)'s).

use super::{Renderer, STOPS_WIDTH};
use crate::shaders::{COMPOSITE_PAINT, LINEAR, Prim, RADIAL, SWEEP};
use kompoz_graphics::{
    BlendMode, Brush, BrushKind, Color, ColorStop, Matrix, Offset, Projective, Rect, Shader, ShaderKind, TileMode,
};

/// A color premultiplied, its components in sRGB as they are drawn (not rounded to 8 bits).
pub(super) fn premultiplied(color: Color) -> [f32; 4] {
    let [r, g, b, a] = color.srgb_components();
    [r * a, g * a, b * a, a]
}

/// The color with another alpha, in its color space.
pub(super) fn with_alpha(c: Color, alpha: f32) -> Color {
    Color::new_in(c.red(), c.green(), c.blue(), alpha, c.color_space())
}

/// A blend mode as the shaders number them (C++'s BlendMode, primitives.slang's blendColors).
pub(super) fn blend_index(mode: BlendMode) -> i32 {
    use BlendMode::*;
    match mode {
        Src => 0,
        SrcOver => 1,
        SrcIn => 2,
        SrcAtop => 3,
        DstIn => 4,
        Modulate => 5,
        Multiply => 6,
        Screen => 7,
        Darken => 8,
        Lighten => 9,
        Plus => 10,
        Clear => 11,
        Dst => 12,
        DstOver => 13,
        SrcOut => 14,
        DstOut => 15,
        DstAtop => 16,
        Xor => 17,
        Overlay => 18,
        ColorDodge => 19,
        ColorBurn => 20,
        Hardlight => 21,
        Softlight => 22,
        Difference => 23,
        Exclusion => 24,
        Hue => 25,
        Saturation => 26,
        Color => 27,
        Luminosity => 28,
    }
}

/// A tile mode as the shaders number them (clamp, repeated, mirror, decal).
pub(super) fn tile_index(mode: TileMode) -> i32 {
    match mode {
        TileMode::Clamp => 0,
        TileMode::Repeated => 1,
        TileMode::Mirror => 2,
        TileMode::Decal => 3,
    }
}

/// A gradient's kind as the shaders number them.
fn gradient_kind(kind: ShaderKind) -> i32 {
    match kind {
        ShaderKind::Linear => LINEAR,
        ShaderKind::Radial => RADIAL,
        _ => SWEEP,
    }
}

/// A color stop's color (straight, sRGB) and offset at two texels of a row starting at `texel`.
fn write_stop(texel: &mut [f32], stop: &ColorStop) {
    texel[..4].copy_from_slice(&stop.color.srgb_components());
    texel[4] = stop.offset;
}

impl Renderer {
    /// A prim painted with `brush` over `area` (px): its color, or its gradient (at `alpha`), in pixels `enlarged`
    /// times.
    pub(super) fn painted(&mut self, brush: &Brush, area: Rect, alpha: f32, enlarged: f32) -> Prim {
        let shaded = matches!(brush.kind(), BrushKind::Composite(_) | BrushKind::Shader(_));
        if shaded || brush.transform().is_some() {
            let shader = brush.create_shader(area.size());
            return self.painted_shader(&shader, area, alpha * brush.shader_alpha(), enlarged, None);
        }
        let stops = brush.color_stops();
        if brush.is_solid() || stops.len() < 2 {
            let c = stops.first().map_or(Color::Transparent, |s| s.color);
            return Prim { color: premultiplied(with_alpha(c, c.alpha() * alpha)), ..Prim::default() };
        }
        // Its stops in this frame's table (once for equal brushes), its geometry in the target's pixels.
        let row = match self.stop_brushes.iter().position(|b| b == brush) {
            Some(i) => self.stop_rows[i],
            None => {
                let row = self.table_rows();
                self.stop_brushes.push(brush.clone());
                self.stop_rows.push(row);
                let at = self.stops.len();
                self.stops.resize(at + STOPS_WIDTH * 4, 0.0);
                for (i, stop) in stops.iter().take(STOPS_WIDTH / 2).enumerate() {
                    write_stop(&mut self.stops[at + i * 8..at + i * 8 + 5], stop);
                }
                row
            }
        };
        let g = brush.geometry(area);
        let s = self.scale;
        let px = |o: Offset| Offset::new((o.x * s - self.ox) * enlarged, (o.y * s - self.oy) * enlarged);
        let (start, end) = (px(g.start), px(g.end));
        let kind = match brush.kind() {
            BrushKind::LinearGradient(_) => LINEAR,
            BrushKind::RadialGradient(_) => RADIAL,
            _ => SWEEP,
        };
        Prim {
            paint: [
                kind as f32,
                tile_index(brush.tile_mode()) as f32,
                stops.len().min(STOPS_WIDTH / 2) as f32,
                row as f32,
            ],
            gradient: [
                start.x,
                start.y,
                if kind == LINEAR { end.x } else { g.radius * s * enlarged },
                if kind == LINEAR { end.y } else { 0.0 },
            ],
            color: [1.0, 1.0, 1.0, alpha],
            ..Prim::default()
        }
    }

    /// A row of the table: `stops` (if any) from its start, then, in its last three texels, the rows of `map` (from
    /// the target's px to where the shader is evaluated).
    pub(super) fn shader_row(&mut self, stops: Option<&[ColorStop]>, map: &Projective) -> i32 {
        let row = self.table_rows();
        let at = self.stops.len();
        self.stops.resize(at + STOPS_WIDTH * 4, 0.0);
        for (i, stop) in stops.unwrap_or_default().iter().take(STOPS_WIDTH / 2 - 2).enumerate() {
            write_stop(&mut self.stops[at + i * 8..at + i * 8 + 5], stop);
        }
        for r in 0..3 {
            let texel = at + (STOPS_WIDTH - 3 + r) * 4;
            self.stops[texel..texel + 3].copy_from_slice(&map.m[r * 3..r * 3 + 3]);
        }
        row
    }

    /// A shader's paint: where each pixel is in it (its px from the area's top left, through its transform taken
    /// back) worked out per pixel from the map in its row; a gradient's stops before that; an image's in texture1
    /// (see [`image_paint`](Renderer::image_paint)); a composite's two the same, blended where the pixel is (a
    /// composite of composites baked first: [`baked_shader`](Renderer::baked_shader)). `before`: what the point the
    /// shader is asked for goes through to be this target's px first (a mesh's texture coordinates).
    pub(super) fn painted_shader(
        &mut self,
        shader: &Shader,
        area: Rect,
        alpha: f32,
        enlarged: f32,
        before: Option<&Projective>,
    ) -> Prim {
        let k = enlarged * self.scale;
        let mut map = Projective {
            m: [
                1.0 / k,
                0.0,
                self.ox / self.scale - area.left,
                0.0,
                1.0 / k,
                self.oy / self.scale - area.top,
                0.0,
                0.0,
                1.0,
            ],
        };
        if let Some(t) = shader.transform() {
            let mut back = *t;
            back.invert();
            map = Projective::of(&back) * map;
        }
        if let Some(before) = before {
            map = map * *before;
        }
        match shader.kind() {
            ShaderKind::Composite => {
                // Worked out where each pixel is (paintAt): the two through the composite's own transform (after
                // theirs), each a paint (one color a stop of it), in a row of the table; a composite in either baked.
                let (Some(dst), Some(src)) = (shader.dst(), shader.src()) else { return Prim::default() };
                if dst.kind() == ShaderKind::Composite || src.kind() == ShaderKind::Composite {
                    return self.baked_shader(shader, area, alpha, enlarged, before);
                }
                let through =
                    |one: &Shader| shader.transform().map_or_else(|| one.clone(), |t: &Matrix| one.transformed(t));
                let (dst, src) = (through(dst), through(src));
                let d = self.child_paint(&dst, area, enlarged, before);
                let s = self.child_paint(&src, area, enlarged, before);
                let row = self.table_rows();
                let at = self.stops.len();
                self.stops.resize(at + STOPS_WIDTH * 4, 0.0);
                self.stops[at..at + 4].copy_from_slice(&d.paint);
                self.stops[at + 4..at + 8].copy_from_slice(&d.gradient);
                self.stops[at + 8..at + 12].copy_from_slice(&s.paint);
                self.stops[at + 12..at + 16].copy_from_slice(&s.gradient);
                self.stops[at + 16] = blend_index(shader.blend_mode()) as f32;
                let image_index =
                    |c: &Prim| if (c.paint[0] + 0.5) as i32 == crate::shaders::IMAGE_PAINT { c.paint[2] } else { -1.0 };
                Prim {
                    paint: [COMPOSITE_PAINT as f32, image_index(&d), image_index(&s), row as f32],
                    color: [1.0, 1.0, 1.0, alpha],
                    ..Prim::default()
                }
            }
            ShaderKind::Image => self.image_paint(shader, map, alpha),
            kind => {
                let stops = shader.stops();
                if stops.len() < 2 {
                    // One color (or none).
                    let c = stops.first().map_or(Color::Transparent, |s| s.color);
                    return Prim { color: premultiplied(with_alpha(c, c.alpha() * alpha)), ..Prim::default() };
                }
                let kind = gradient_kind(kind);
                let row = self.shader_row(Some(stops), &map);
                let (start, end) = (shader.start(), shader.end());
                Prim {
                    paint: [
                        kind as f32,
                        (tile_index(shader.tile_mode()) | 8) as f32,
                        stops.len().min(STOPS_WIDTH / 2 - 2) as f32,
                        row as f32,
                    ],
                    gradient: [
                        start.x,
                        start.y,
                        if kind == LINEAR { end.x } else { shader.radius() },
                        if kind == LINEAR { end.y } else { 0.0 },
                    ],
                    color: [1.0, 1.0, 1.0, alpha],
                    ..Prim::default()
                }
            }
        }
    }

    /// One of a composite's two: its paint, a color as a gradient of one stop.
    fn child_paint(&mut self, one: &Shader, area: Rect, enlarged: f32, before: Option<&Projective>) -> Prim {
        let mut c = self.painted_shader(one, area, 1.0, enlarged, before);
        if (c.paint[0] + 0.5) as i32 == 0 {
            let color = one.stops().first().map_or(Color::Transparent, |s| s.color);
            let row = self.shader_row(Some(&[ColorStop::new(0.0, color)]), &Projective::Identity);
            c.paint = [LINEAR as f32, 0.0, 1.0, row as f32];
            c.gradient = [0.0; 4];
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_modes_are_numbered_as_the_shaders_number_them() {
        // Every mode a number of its own, 0 to 28, as primitives.slang's blendColors has them.
        let all = [
            BlendMode::Clear,
            BlendMode::Src,
            BlendMode::Dst,
            BlendMode::SrcOver,
            BlendMode::DstOver,
            BlendMode::SrcIn,
            BlendMode::DstIn,
            BlendMode::SrcOut,
            BlendMode::DstOut,
            BlendMode::SrcAtop,
            BlendMode::DstAtop,
            BlendMode::Xor,
            BlendMode::Plus,
            BlendMode::Modulate,
            BlendMode::Screen,
            BlendMode::Overlay,
            BlendMode::Darken,
            BlendMode::Lighten,
            BlendMode::ColorDodge,
            BlendMode::ColorBurn,
            BlendMode::Hardlight,
            BlendMode::Softlight,
            BlendMode::Difference,
            BlendMode::Exclusion,
            BlendMode::Multiply,
            BlendMode::Hue,
            BlendMode::Saturation,
            BlendMode::Color,
            BlendMode::Luminosity,
        ];
        let mut numbers: Vec<i32> = all.iter().map(|&m| blend_index(m)).collect();
        numbers.sort();
        assert_eq!(numbers, (0..29).collect::<Vec<_>>());
        assert_eq!((blend_index(BlendMode::Multiply), blend_index(BlendMode::Clear)), (6, 11));
        assert_eq!(tile_index(TileMode::Decal), 3);
        // Premultiplied as it is drawn (its sRGB components as the color keeps them).
        let c = Color::new(1.0, 0.5, 0.0, 0.5);
        let [r, g, b, a] = c.srgb_components();
        assert_eq!(premultiplied(c), [r * a, g * a, b * a, a]);
        assert_eq!(premultiplied(Color::new(1.0, 0.0, 1.0, 1.0)), [1.0, 0.0, 1.0, 1.0]);
    }
}
