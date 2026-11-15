//! The renderer's shaders (`shaders/*.slang`, C++ Kompoz's as they are), compiled by `cargo xtask shaders` for every
//! backend: SPIR-V 1.3 (Vulkan 1.1), MSL (Metal) and WGSL (WebGPU), committed in `generated/` with what the
//! generated module includes of them ([`ShaderSource`]s and each shader's version); and what the shaders read, laid
//! out as they read it ([`layout`]'s instances, uniforms and constants).

#![deny(unsafe_code)]

#[rustfmt::skip]
mod generated;
pub mod layout;

pub use generated::*;
pub use layout::*;

use std::fmt;

/// One entry point's code on every backend (what a device's shader code is made from): SPIR-V as Slang wrote it,
/// MSL and WGSL source, and its entry point's name (the same on every backend).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ShaderSource {
    pub spirv: &'static [u8],
    pub msl: &'static str,
    pub wgsl: &'static str,
    pub entry: &'static str,
}

/// The magic number a SPIR-V module starts with.
const SPIRV_MAGIC: u32 = 0x0723_0203;

impl ShaderSource {
    /// Its SPIR-V as words (what Vulkan takes), read in the byte order its magic number says it was written in
    /// (Slang writes little-endian).
    pub fn spirv_words(&self) -> Vec<u32> {
        let big = self.spirv.get(..4) == Some(&SPIRV_MAGIC.to_be_bytes()[..]);
        let (words, _) = self.spirv.as_chunks::<4>();
        words.iter().map(|&b| if big { u32::from_be_bytes(b) } else { u32::from_le_bytes(b) }).collect()
    }
}

impl fmt::Debug for ShaderSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ShaderSource({}: {} B SPIR-V, {} B MSL, {} B WGSL)",
            self.entry,
            self.spirv.len(),
            self.msl.len(),
            self.wgsl.len()
        )
    }
}

/// All the shaders' code in eight bytes (C++'s Renderer::shadersVersion, of xtask's versions): a pipeline cache kept
/// for other code holds nothing these can use (and grows with each).
pub const VERSION: u64 = {
    let mut version = PRIMITIVES_VERSION;
    version = (version ^ PATH_VERSION).wrapping_mul(0x0000_0100_0000_01b3);
    (version ^ EFFECTS_VERSION).wrapping_mul(0x0000_0100_0000_01b3)
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_point_is_there_on_every_backend() {
        let all = [PRIMITIVES_VS, PRIMITIVES_VS_MESH, PRIMITIVES_FS, PATH_VS, PATH_FS, EFFECTS_VS, EFFECTS_FS];
        for s in all {
            let words = s.spirv_words();
            // SPIR-V 1.3, whole words.
            assert!(s.spirv.len() % 4 == 0 && words[0] == SPIRV_MAGIC && words[1] == 0x0001_0300, "{s:?}");
            assert!(s.msl.contains(s.entry) && s.wgsl.contains(&format!("fn {}(", s.entry)), "{s:?}");
            // WGSL bound as the WebGPU backend binds it.
            assert!(s.wgsl.starts_with("diagnostic(off, derivative_uniformity);\n"), "{s:?}");
        }
        assert!(PRIMITIVES_FS.wgsl.contains("@binding(0) @group(0) var<uniform> "));
        assert!(PRIMITIVES_FS.wgsl.contains("@binding(11) @group(0) var texture5_texture_0 : texture_2d<f32>;"));
        assert!(PRIMITIVES_FS.wgsl.contains("@binding(12) @group(0) var texture5_sampler_0 : sampler;"));
        assert_eq!([PRIMITIVES_VS.entry, PRIMITIVES_VS_MESH.entry, PRIMITIVES_FS.entry], ["vs", "vsMesh", "fs"]);
        let versions = [PRIMITIVES_VERSION, PATH_VERSION, EFFECTS_VERSION, VERSION];
        assert!(versions.iter().enumerate().all(|(i, v)| !versions[..i].contains(v)));
    }
}
