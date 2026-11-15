//! What the shaders read, laid out as they read it (C++'s renderer.hpp): the per-instance attributes of each
//! pipeline (one buffer, no per-vertex attributes: the vertex shaders build their quads from the vertex index), the
//! uniforms (Vulkan's push constants: float4s, at most 128 bytes) and the primitives' specialization constants; and
//! their bytes, written float by float (no reinterpretation of memory: the GPU reads them in the CPU's byte order).

/// What the GPU reads as it is: float4s, nothing between them.
pub trait GpuData {
    /// Its size in bytes (its shader struct's).
    const SIZE: usize;
    /// Appends its bytes to `out`, in the CPU's byte order.
    fn write_to(&self, out: &mut Vec<u8>);
}

/// The bytes of `items`, one after another (an instance buffer's, or a uniform block's).
pub fn bytes_of<T: GpuData>(items: &[T]) -> Vec<u8> {
    let mut out = Vec::with_capacity(items.len() * T::SIZE);
    for item in items {
        item.write_to(&mut out);
    }
    out
}

/// A struct of float4s that the GPU reads as it is, its fields in the shader's order.
macro_rules! float4s {
    ($(#[$meta:meta])* $name:ident { $($(#[$field_meta:meta])* $field:ident),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Debug)]
        pub struct $name {
            $($(#[$field_meta])* pub $field: [f32; 4],)*
        }

        impl $name {
            /// Its fields' names, in order: what the shader calls them.
            pub const FIELDS: &'static [&'static str] = &[$(stringify!($field)),*];
        }

        impl GpuData for $name {
            const SIZE: usize = $name::FIELDS.len() * 16;
            fn write_to(&self, out: &mut Vec<u8>) {
                $(for v in self.$field {
                    out.extend_from_slice(&v.to_ne_bytes());
                })*
            }
        }
    };
}

// ---- primitives.slang -------------------------------------------------------------------------------------------

/// A primitive's kind (`Prim::extra[2]`), as primitives.slang has them (see there for what each reads).
pub const FILL: i32 = 0;
pub const STROKE: i32 = 1;
pub const LINE: i32 = 2;
pub const ARC: i32 = 3;
pub const SHADOW: i32 = 4;
pub const GLYPH: i32 = 5;
pub const IMAGE: i32 = 6;
pub const LAYER: i32 = 7;
pub const PATH: i32 = 8;
pub const ELEVATION: i32 = 9;
pub const MESH: i32 = 10;

/// What covers a primitive (`Prim::paint[0]`): its color (0), or a paint.
pub const LINEAR: i32 = 1;
pub const RADIAL: i32 = 2;
pub const SWEEP: i32 = 3;
pub const IMAGE_PAINT: i32 = 4;
pub const COMPOSITE_PAINT: i32 = 5;

/// A primitive's flags (`Prim::extra[3]`), as primitives.slang reads them: a layer cut to a path's coverage mask
/// (texture1) rather than its rounded rect.
pub const FLAG_OUTLINE: i32 = 1;
/// A glyph's or an image's quad reaching past its texels, its edges anti-aliased (turned or in perspective).
pub const FLAG_EDGED: i32 = 2;
/// A path's paint where the pixel is in the path's own space (radii, uv and shape: the rows of the map there).
pub const FLAG_PAINT_MAPPED: i32 = 4;
/// A layer's alpha in its coverage (a backdrop drawn in place of what is there: Src, mixed by it).
pub const FLAG_MIXED: i32 = 8;
/// Not anti-aliased: coverage at the pixel's middle alone.
pub const FLAG_ALIASED: i32 = 16;
/// In perspective (its transform's w not the same everywhere).
pub const FLAG_PERSPECTIVE: i32 = 32;

/// Vertices a primitive's quad is drawn with (two triangles; `vs`), and a mesh's triangle (`vsMesh`).
pub const QUAD_VERTICES: u32 = 6;
pub const MESH_VERTICES: u32 = 3;

float4s! {
    /// One primitive, one instanced quad (primitives.slang's Instance; C++'s Renderer::Prim): 12 float4s.
    Prim {
        /// The quad: x0, y0, x1, y1.
        bounds,
        shape,
        radii,
        /// Premultiplied.
        color,
        params,
        uv,
        /// a, b, c, d: x' = a x + c y + tx, y' = b x + d y + ty.
        xform,
        /// tx, ty, kind, flags.
        extra,
        paint,
        gradient,
        /// How many color filters, their row in the table; how many clips, their row.
        filter,
        /// g, h, i: the transform's w = g x + h y + i.
        projection,
    }
}

impl Default for Prim {
    /// Nothing, untransformed (C++'s Prim{}).
    fn default() -> Prim {
        Prim {
            bounds: [0.0; 4],
            shape: [0.0; 4],
            radii: [0.0; 4],
            color: [0.0; 4],
            params: [0.0; 4],
            uv: [0.0; 4],
            xform: [1.0, 0.0, 0.0, 1.0],
            extra: [0.0; 4],
            paint: [0.0; 4],
            gradient: [0.0; 4],
            filter: [0.0; 4],
            projection: [0.0, 0.0, 1.0, 0.0],
        }
    }
}

impl Prim {
    /// Its kind (extra.z).
    pub fn kind(&self) -> i32 {
        (self.extra[2] + 0.5) as i32
    }
    /// Its flags (extra.w).
    pub fn flags(&self) -> i32 {
        (self.extra[3] + 0.5) as i32
    }
}

float4s! {
    /// The primitives' uniforms (primitives.slang's Uniforms).
    PrimitiveUniforms {
        /// clip = position * to_clip.xy + to_clip.zw.
        to_clip,
        /// A texel's size in the coverage mask (xy), the table's (zw: z its width's inverse).
        texels,
        /// The blend mode (x), how the coverage goes in (y), a texel of the target's copy (zw).
        blend,
    }
}

/// The primitives' specialization constants (primitives.slang's KINDS to FLAGS, constant ids 0 to 5; C++'s
/// ShaderVariant): what a pipeline's batches need of the fragment shader. The default takes everything (the
/// über-shader's).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PrimitiveConstants {
    /// The kinds drawn: bit 1 << kind.
    pub kinds: i32,
    /// The paints: bit 1 << paint kind (bit 0: the color alone).
    pub paints: i32,
    /// Cut by clips.
    pub clipped: i32,
    /// Through color filters.
    pub filtered: i32,
    /// How the coverage goes in; -1: as the uniforms' blend.y says.
    pub how: i32,
    /// The instances' flags that may be set.
    pub flags: i32,
}

impl Default for PrimitiveConstants {
    fn default() -> PrimitiveConstants {
        PrimitiveConstants { kinds: 0x7ff, paints: 0x3f, clipped: 1, filtered: 1, how: -1, flags: 0xff }
    }
}

impl PrimitiveConstants {
    /// Their names in the shader, by constant id.
    pub const NAMES: [&'static str; 6] = ["KINDS", "PAINTS", "CLIPPED", "FILTERED", "HOW", "FLAGS"];

    /// Their values by constant id.
    pub fn values(&self) -> [i32; 6] {
        [self.kinds, self.paints, self.clipped, self.filtered, self.how, self.flags]
    }
}

// ---- path.slang -------------------------------------------------------------------------------------------------

float4s! {
    /// One edge of a path in the coverage mask, and the path's part of the mask (path.slang's Instance; C++'s
    /// Renderer::EdgeInstance).
    EdgeInstance {
        /// x0, y0, x1, y1.
        edge,
        /// The path's part of the mask: x0, y0, x1, y1.
        region,
        /// x: not anti-aliased (1).
        mode,
    }
}

float4s! {
    /// The coverage pass's uniforms (path.slang's Uniforms).
    PathUniforms {
        /// clip = position * to_clip.xy + to_clip.zw.
        to_clip,
    }
}

// ---- effects.slang ----------------------------------------------------------------------------------------------

/// An effect pass (`EffectUniforms::params[0]`), as effects.slang has them.
pub const PREPARE: i32 = 0;
pub const GAUSS: i32 = 1;
pub const PROGRESSIVE: i32 = 2;

float4s! {
    /// What an effect pass draws (effects.slang's Instance).
    EffectInstance {
        /// x0, y0, x1, y1.
        rect,
    }
}

float4s! {
    /// An effect pass's uniforms (effects.slang's Uniforms; C++'s EffectDraw::uniforms): eight float4s, 128 bytes.
    EffectUniforms {
        /// clip = position * to_clip.xy + to_clip.zw.
        to_clip,
        /// The source texel a destination texel starts at: t * map.xy + map.zw.
        map,
        /// The source's own texels: x0, y0, x1, y1 (PROGRESSIVE: the bounds, in the destination's).
        valid,
        /// The pass, the tile mode, the axis (0 across, 1 down), sigma (PROGRESSIVE: the widest radius, px).
        params,
        /// GAUSS: how many texels each way (PROGRESSIVE: texels a px); a source texel's size (zw).
        extra,
        /// PROGRESSIVE: along a line (1), out from a center (2) or a mask (3); the stops' row, how many.
        mask,
        /// The line's start and end, or the center and how far out (texels from `origin`).
        geometry,
        /// Where the layer's top left is in the destination; a table texel's size (zw).
        origin,
    }
}

/// `to_clip` for a target `width` x `height` px (the shaders' positions are its pixels, y down): clip y down where
/// the backend's clip space is (Vulkan), else up.
pub fn to_clip(width: f32, height: f32, clip_y_down: bool) -> [f32; 4] {
    if clip_y_down { [2.0 / width, 2.0 / height, -1.0, -1.0] } else { [2.0 / width, -2.0 / height, -1.0, 1.0] }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaders::{EFFECTS_FS, EFFECTS_VS, PATH_VS, PRIMITIVES_FS, PRIMITIVES_VS, ShaderSource};
    use std::collections::HashMap;

    /// What a SPIR-V module says of its interface: names, member names, locations, member offsets, spec ids.
    #[derive(Default)]
    struct Interface {
        names: HashMap<u32, String>,
        member_names: HashMap<(u32, u32), String>,
        locations: HashMap<u32, u32>,
        offsets: HashMap<(u32, u32), u32>,
        spec_ids: HashMap<u32, u32>,
    }

    fn string(words: &[u32]) -> String {
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).take_while(|&b| b != 0).collect();
        String::from_utf8(bytes).expect("a UTF-8 name")
    }

    fn interface(shader: &ShaderSource) -> Interface {
        let words = shader.spirv_words();
        assert_eq!(words[0], 0x0723_0203);
        let mut out = Interface::default();
        let mut at = 5;
        while at < words.len() {
            let (count, op) = ((words[at] >> 16) as usize, words[at] & 0xffff);
            let w = &words[at + 1..at + count];
            // OpName, OpMemberName, OpDecorate (Location, SpecId), OpMemberDecorate (Offset).
            match op {
                5 => {
                    out.names.insert(w[0], string(&w[1..]));
                }
                6 => {
                    out.member_names.insert((w[0], w[1]), string(&w[2..]));
                }
                71 if w[1] == 30 => {
                    out.locations.insert(w[0], w[2]);
                }
                71 if w[1] == 1 => {
                    out.spec_ids.insert(w[0], w[2]);
                }
                72 if w[2] == 35 => {
                    out.offsets.insert((w[0], w[1]), w[3]);
                }
                _ => {}
            }
            at += count.max(1);
        }
        out
    }

    /// The instance attributes' names by location (the vertex shader's `instance.<field>` inputs).
    fn instance_fields(shader: &ShaderSource) -> Vec<String> {
        let i = interface(shader);
        let mut fields: Vec<(u32, String)> = i
            .locations
            .iter()
            .filter_map(|(id, &location)| Some((location, i.names.get(id)?.strip_prefix("instance.")?.to_string())))
            .collect();
        fields.sort();
        assert!(fields.iter().enumerate().all(|(n, (location, _))| *location == n as u32));
        fields.into_iter().map(|(_, name)| name).collect()
    }

    /// The uniform block's members by offset, in the shader's names.
    fn uniform_members(shader: &ShaderSource) -> Vec<(u32, String)> {
        let i = interface(shader);
        let block = i.names.iter().find(|(_, n)| n.as_str() == "Uniforms_std430").map(|(&id, _)| id).expect("uniforms");
        let mut members: Vec<(u32, String)> = i
            .offsets
            .iter()
            .filter(|((ty, _), _)| *ty == block)
            .map(|(&(_, member), &offset)| (offset, i.member_names[&(block, member)].clone()))
            .collect();
        members.sort();
        members
    }

    /// `to_clip` as the shaders spell it.
    fn shader_name(field: &str) -> String {
        let mut out = String::new();
        let mut upper = false;
        for c in field.chars() {
            if c == '_' {
                upper = true;
            } else {
                out.push(if upper { c.to_ascii_uppercase() } else { c });
                upper = false;
            }
        }
        out
    }

    fn laid_out(fields: &[&str]) -> Vec<(u32, String)> {
        fields.iter().enumerate().map(|(i, f)| (i as u32 * 16, shader_name(f))).collect()
    }

    #[test]
    #[cfg_attr(miri, ignore)] // (reads the shaders' SPIR-V: slow there, and what Miri checks is the bytes below)
    fn instances_and_uniforms_are_laid_out_as_the_shaders_read_them() {
        assert_eq!(instance_fields(&PRIMITIVES_VS), Prim::FIELDS);
        assert_eq!(instance_fields(&PATH_VS), EdgeInstance::FIELDS);
        assert_eq!(instance_fields(&EFFECTS_VS), EffectInstance::FIELDS);
        assert_eq!(uniform_members(&PRIMITIVES_FS), laid_out(PrimitiveUniforms::FIELDS));
        assert_eq!(uniform_members(&PATH_VS), laid_out(PathUniforms::FIELDS)); // (its fragment shader reads none)
        assert_eq!(uniform_members(&EFFECTS_FS), laid_out(EffectUniforms::FIELDS));
        // The specialization constants by id.
        let i = interface(&PRIMITIVES_FS);
        let mut ids: Vec<(u32, &str)> = i.spec_ids.iter().map(|(id, &spec)| (spec, i.names[id].as_str())).collect();
        ids.sort();
        assert_eq!(ids, PrimitiveConstants::NAMES.iter().enumerate().map(|(n, &s)| (n as u32, s)).collect::<Vec<_>>());
    }

    #[test]
    fn sizes_are_the_shaders() {
        assert_eq!(Prim::SIZE, 192);
        assert_eq!(EdgeInstance::SIZE, 48);
        assert_eq!(EffectInstance::SIZE, 16);
        assert_eq!(PrimitiveUniforms::SIZE, 48);
        assert_eq!(PathUniforms::SIZE, 16);
        assert_eq!(EffectUniforms::SIZE, 128); // (all that push constants hold)
        assert_eq!(std::mem::size_of::<Prim>(), Prim::SIZE);
        assert_eq!(std::mem::size_of::<EffectUniforms>(), EffectUniforms::SIZE);
    }

    #[test]
    fn bytes_are_the_fields_floats_in_order() {
        let mut p = Prim { bounds: [1.0, 2.0, 3.0, 4.0], projection: [0.5, -0.25, 1.0, 0.0], ..Prim::default() };
        p.extra = [10.0, 20.0, PATH as f32, (FLAG_ALIASED | FLAG_PERSPECTIVE) as f32];
        let q = Prim { color: [0.25; 4], ..Prim::default() };
        let bytes = bytes_of(&[p, q]);
        assert_eq!(bytes.len(), 2 * Prim::SIZE);
        let floats: Vec<f32> = bytes.as_chunks::<4>().0.iter().map(|&b| f32::from_ne_bytes(b)).collect();
        assert_eq!(floats[0..4], [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(floats[24..28], [1.0, 0.0, 0.0, 1.0]); // xform: the identity
        assert_eq!(floats[28..32], [10.0, 20.0, 8.0, 48.0]);
        assert_eq!(floats[44..48], [0.5, -0.25, 1.0, 0.0]);
        assert_eq!(floats[48 + 12..48 + 16], [0.25; 4]);
        assert!(p.kind() == PATH && p.flags() == FLAG_ALIASED | FLAG_PERSPECTIVE);
        let e = EdgeInstance { edge: [0.5, 1.0, 2.5, 7.0], region: [0.0, 0.0, 8.0, 8.0], mode: [1.0, 0.0, 0.0, 0.0] };
        let floats: Vec<f32> = bytes_of(&[e]).as_chunks::<4>().0.iter().map(|&b| f32::from_ne_bytes(b)).collect();
        assert_eq!(floats, [0.5, 1.0, 2.5, 7.0, 0.0, 0.0, 8.0, 8.0, 1.0, 0.0, 0.0, 0.0]);
        assert!(bytes_of::<EffectInstance>(&[]).is_empty());
    }

    #[test]
    fn clip_space_is_the_targets() {
        // Its corners at clip's: (0, 0) top left, y down on Vulkan, up elsewhere.
        let at = |c: [f32; 4], x: f32, y: f32| (x * c[0] + c[2], y * c[1] + c[3]);
        let down = to_clip(200.0, 100.0, true);
        assert!(at(down, 0.0, 0.0) == (-1.0, -1.0) && at(down, 200.0, 100.0) == (1.0, 1.0));
        let up = to_clip(200.0, 100.0, false);
        assert!(at(up, 0.0, 0.0) == (-1.0, 1.0) && at(up, 200.0, 100.0) == (1.0, -1.0));
        assert_eq!(PrimitiveConstants::default().values(), [0x7ff, 0x3f, 1, 1, -1, 0xff]);
    }
}
