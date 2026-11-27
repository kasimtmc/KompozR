//! Textures: the device's table of them (ids with generations, as the Vulkan backend's), and what blit encoders do
//! with them: uploads from the frame's memory, copies, mipmaps, readbacks.

use super::{MetalDevice, mtl_format};
use crate::device::{Format, IRect, TextureId, TextureUsage, pass_label};
use objc2::Message;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{
    MTLBlitCommandEncoder, MTLBuffer, MTLCommandBuffer, MTLDevice, MTLOrigin, MTLResourceOptions, MTLSize,
    MTLStorageMode, MTLTexture, MTLTextureDescriptor, MTLTextureUsage,
};

/// A texture of the device's.
pub(super) struct MtlTexture {
    pub texture: Retained<ProtocolObject<dyn MTLTexture>>,
    /// Its next pass that doesn't clear needn't load it ([`Device::discard`](crate::device::Device::discard)).
    pub discarded: bool,
}

/// The textures by slot, each slot's generation telling ids of textures gone from those of the one there.
#[derive(Default)]
pub(super) struct Textures {
    slots: Vec<(u32, Option<MtlTexture>)>,
    free: Vec<u32>,
}

impl Textures {
    /// `texture` in a slot of its own, and its id.
    pub fn insert(&mut self, texture: Retained<ProtocolObject<dyn MTLTexture>>, format: Format) -> TextureId {
        let slot = self.free.pop().unwrap_or_else(|| {
            self.slots.push((0, None));
            self.slots.len() as u32 - 1
        });
        let (width, height) = (texture.width() as i32, texture.height() as i32);
        let entry = &mut self.slots[slot as usize];
        entry.1 = Some(MtlTexture { texture, discarded: false });
        TextureId::new(slot, entry.0, width, height, format)
    }

    /// The texture out of its slot (which a later one takes, with its next generation).
    pub fn remove(&mut self, id: TextureId) -> Option<MtlTexture> {
        let entry = self.slots.get_mut(id.slot() as usize).filter(|(g, t)| *g == id.generation() && t.is_some())?;
        entry.0 = entry.0.wrapping_add(1);
        self.free.push(id.slot());
        entry.1.take()
    }

    pub fn get(&self, id: TextureId) -> Option<&MtlTexture> {
        match self.slots.get(id.slot() as usize) {
            Some((generation, Some(t))) if *generation == id.generation() => Some(t),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: TextureId) -> Option<&mut MtlTexture> {
        match self.slots.get_mut(id.slot() as usize) {
            Some((generation, Some(t))) if *generation == id.generation() => Some(t),
            _ => None,
        }
    }

    /// The Metal texture of an id (none if it is gone).
    pub fn texture(&self, id: TextureId) -> Option<Retained<ProtocolObject<dyn MTLTexture>>> {
        self.get(id).map(|t| t.texture.clone())
    }
}

/// Whether `region` is within the texture.
fn within(region: IRect, id: TextureId) -> bool {
    region.x >= 0 && region.y >= 0 && region.x + region.width <= id.width() && region.y + region.height <= id.height()
}

fn origin(x: i32, y: i32) -> MTLOrigin {
    MTLOrigin { x: x as usize, y: y as usize, z: 0 }
}

fn size(region: IRect) -> MTLSize {
    MTLSize { width: region.width as usize, height: region.height as usize, depth: 1 }
}

impl MetalDevice {
    pub(super) fn make_texture(
        &mut self,
        width: i32,
        height: i32,
        format: Format,
        usage: TextureUsage,
    ) -> Option<TextureId> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let mipmapped = usage.contains(TextureUsage::MIPMAPPED);
        // SAFETY: a 2D texture's descriptor of a positive size, in a color format.
        let descriptor = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                mtl_format(format),
                width as usize,
                height as usize,
                mipmapped,
            )
        };
        // (The GPU's own memory: uploads and readbacks go through shared buffers, as blits.)
        descriptor.setStorageMode(MTLStorageMode::Private);
        let mut mtl_usage = MTLTextureUsage::ShaderRead;
        if usage.contains(TextureUsage::RENDER_TARGET) {
            mtl_usage |= MTLTextureUsage::RenderTarget;
        }
        descriptor.setUsage(mtl_usage);
        let texture = self.device.newTextureWithDescriptor(&descriptor)?;
        Some(self.textures.insert(texture, format))
    }

    /// The blit encoder of the command buffer recording (one going until a pass starts or it is committed).
    pub(super) fn blit_encoder(&mut self) -> Retained<ProtocolObject<dyn MTLBlitCommandEncoder>> {
        if self.blit.is_none() {
            self.blit = Some(self.commands().blitCommandEncoder().expect("Metal: a blit encoder"));
        }
        self.recorded = true;
        self.blit.clone().expect("made above")
    }

    pub(super) fn upload_texels(&mut self, id: TextureId, region: IRect, pixels: &[u8], row_bytes: usize) {
        assert!(within(region, id), "upload: {region:?} isn't within {id:?}");
        let Some(texture) = self.textures.texture(id) else { return };
        if region.is_empty() {
            return;
        }
        let row = region.width as usize * id.format().bytes_per_texel();
        let rows = region.height as usize;
        assert!(row_bytes >= row && pixels.len() >= row_bytes * (rows - 1) + row, "upload: too few pixels");
        // The rows packed into the frame's memory, copied from there; a `Bgra8` texture's given as RGBA (as every
        // backend takes them, and reads them back), swapped into its order here.
        let swap = id.format() == Format::Bgra8;
        let staged = self.write_with(row * rows, 16, |memory| {
            for (y, out) in memory.chunks_exact_mut(row).enumerate() {
                let source = &pixels[y * row_bytes..y * row_bytes + row];
                if swap {
                    for (o, s) in out.as_chunks_mut::<4>().0.iter_mut().zip(source.as_chunks::<4>().0) {
                        *o = [s[2], s[1], s[0], s[3]];
                    }
                } else {
                    out.copy_from_slice(source);
                }
            }
        });
        let Some(staged) = staged else { return };
        let buffer = self.chunk(staged).retain();
        let blit = self.blit_encoder();
        // SAFETY: the buffer holds `row * rows` bytes from the offset (just written), the region is within the
        // texture's first level, and the offset a multiple of 16 (of any format's texel).
        unsafe {
            blit.copyFromBuffer_sourceOffset_sourceBytesPerRow_sourceBytesPerImage_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                &buffer,
                staged.offset() as usize,
                row,
                row * rows,
                size(region),
                &texture,
                0,
                0,
                origin(region.x, region.y),
            );
        }
    }

    pub(super) fn make_mipmaps(&mut self, id: TextureId) {
        let Some(texture) = self.textures.texture(id) else { return };
        if texture.mipmapLevelCount() <= 1 {
            return;
        }
        self.time_next(|| format!("mipmaps {}x{}", id.width(), id.height()));
        let blit = self.blit_encoder();
        blit.generateMipmapsForTexture(&texture);
        self.time_done();
    }

    pub(super) fn copy_texels(&mut self, source: TextureId, region: IRect, destination: TextureId, x: i32, y: i32) {
        assert!(within(region, source), "copy_texture: {region:?} isn't within {source:?}");
        assert!(
            within(IRect::new(x, y, region.width, region.height), destination),
            "copy_texture: the copy isn't within {destination:?}"
        );
        assert_eq!(source.format(), destination.format(), "copy_texture: textures of different formats");
        let (Some(from), Some(to)) = (self.textures.texture(source), self.textures.texture(destination)) else {
            return;
        };
        if region.is_empty() {
            return;
        }
        self.time_next(|| format!("copy {}x{}", region.width, region.height));
        let blit = self.blit_encoder();
        // SAFETY: both regions are within their textures' first levels, of the same format.
        unsafe {
            blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                &from,
                0,
                0,
                origin(region.x, region.y),
                size(region),
                &to,
                0,
                0,
                origin(x, y),
            );
        }
        self.time_done();
    }

    /// The texels, copied into a shared buffer, the frame's work so far committed and waited for, the rest of the
    /// frame in another command buffer.
    pub(super) fn read_texels(&mut self, id: TextureId, region: IRect) -> Vec<u8> {
        assert!(within(region, id), "read_pixels: {region:?} isn't within {id:?}");
        let Some(texture) = self.textures.texture(id) else { return Vec::new() };
        if region.is_empty() {
            return Vec::new();
        }
        let row = region.width as usize * id.format().bytes_per_texel();
        let bytes = row * region.height as usize;
        let Some(buffer) = self.device.newBufferWithLength_options(bytes, MTLResourceOptions::StorageModeShared) else {
            return Vec::new();
        };
        let blit = self.blit_encoder();
        // SAFETY: the region is within the texture's first level; the buffer holds its rows packed.
        unsafe {
            blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(
                &texture,
                0,
                0,
                origin(region.x, region.y),
                size(region),
                &buffer,
                0,
                row,
                bytes,
            );
        }
        self.end_blit();
        let commands = self.commands.clone().expect("a frame begun");
        self.commit_commands();
        commands.waitUntilCompleted();
        let mut out = vec![0u8; bytes];
        // SAFETY: the copy into the shared buffer is done (its command buffer completed); `bytes` of it are the CPU's
        // to read from `contents`.
        unsafe { std::ptr::copy_nonoverlapping(buffer.contents().as_ptr().cast::<u8>(), out.as_mut_ptr(), bytes) };
        if id.format() == Format::Bgra8 {
            for texel in out.as_chunks_mut::<4>().0 {
                texel.swap(0, 2);
            }
        }
        out
    }

    /// What a pass into `id` is named in a timed frame's times.
    pub(super) fn pass_label(&self, id: TextureId) -> String {
        pass_label(id.width(), id.height(), id.format(), self.window == Some(id))
    }
}
