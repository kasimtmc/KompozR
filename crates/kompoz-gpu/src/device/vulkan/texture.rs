//! Textures: images with their views, the layout each is in (all its mip levels'), and the framebuffers made for
//! it; uploads, mipmaps, copies and readbacks, with the barriers between them.

use super::VulkanDevice;
use super::alloc::{Allocation, MemoryUsage, Resource};
use crate::device::{Format, IRect, TextureId, TextureUsage, pass_label};
use ash::vk;

/// A format's Vulkan one.
pub(super) fn vk_format(format: Format) -> vk::Format {
    match format {
        Format::Rgba8 => vk::Format::R8G8B8A8_UNORM,
        Format::Bgra8 => vk::Format::B8G8R8A8_UNORM,
        // (A color attachment that blends, everywhere: Vulkan's required formats.)
        Format::R16f => vk::Format::R16_SFLOAT,
        // (Sampled without filtering, everywhere.)
        Format::Rgba32f => vk::Format::R32G32B32A32_SFLOAT,
        // (A color attachment that blends, filtered: required too.)
        Format::Rgba16f => vk::Format::R16G16B16A16_SFLOAT,
    }
}

/// A texture's image and what goes with it.
pub(super) struct VkTexture {
    pub id: TextureId,
    pub image: vk::Image,
    pub view: vk::ImageView,
    /// None: a swapchain image (the swapchain owns it).
    pub allocation: Option<Allocation>,
    /// All its mip levels'.
    pub layout: vk::ImageLayout,
    pub mip_levels: u32,
    /// What it was made for (a swapchain image: a render target).
    pub usage: TextureUsage,
    pub framebuffers: Vec<(vk::RenderPass, vk::Framebuffer)>,
    /// What it holds no longer matters ([`crate::device::Device::discard`]).
    pub discarded: bool,
}

/// The textures there are, by slot: a slot's generation changes when its texture goes, so that an old id finds
/// nothing.
#[derive(Default)]
pub(super) struct Textures {
    slots: Vec<(u32, Option<VkTexture>)>,
    free: Vec<u32>,
}

impl Textures {
    /// A slot for a texture of this size and format, and its id.
    pub fn reserve(&mut self, width: i32, height: i32, format: Format) -> TextureId {
        let slot = self.free.pop().unwrap_or_else(|| {
            self.slots.push((0, None));
            self.slots.len() as u32 - 1
        });
        TextureId::new(slot, self.slots[slot as usize].0, width, height, format)
    }

    /// The texture into the slot reserved for it.
    pub fn fill(&mut self, texture: VkTexture) {
        let slot = texture.id.slot() as usize;
        self.slots[slot].1 = Some(texture);
    }

    pub fn get(&self, id: TextureId) -> Option<&VkTexture> {
        match self.slots.get(id.slot() as usize) {
            Some((generation, Some(t))) if *generation == id.generation() => Some(t),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: TextureId) -> Option<&mut VkTexture> {
        match self.slots.get_mut(id.slot() as usize) {
            Some((generation, Some(t))) if *generation == id.generation() => Some(t),
            _ => None,
        }
    }

    /// Two different textures at once.
    pub fn pair_mut(&mut self, a: TextureId, b: TextureId) -> Option<(&mut VkTexture, &mut VkTexture)> {
        if a.slot() == b.slot() {
            return None;
        }
        self.get(a)?;
        self.get(b)?;
        let (low, high) = (a.slot().min(b.slot()) as usize, a.slot().max(b.slot()) as usize);
        let (head, tail) = self.slots.split_at_mut(high);
        let (first, second) = (head[low].1.as_mut()?, tail[0].1.as_mut()?);
        Some(if a.slot() < b.slot() { (first, second) } else { (second, first) })
    }

    /// The texture taken out (its slot free for another, with a new generation), or a slot reserved for one that
    /// was never made given back.
    pub fn remove(&mut self, id: TextureId) -> Option<VkTexture> {
        let (generation, texture) = self.slots.get_mut(id.slot() as usize)?;
        if *generation != id.generation() {
            return None;
        }
        *generation = generation.wrapping_add(1);
        self.free.push(id.slot());
        texture.take()
    }

    /// Every texture taken out.
    pub fn drain(&mut self) -> Vec<VkTexture> {
        self.free.clear();
        self.slots.drain(..).filter_map(|(_, t)| t).collect()
    }
}

/// The pipeline stage and access a layout's users have, for barriers.
fn stage_of(layout: vk::ImageLayout) -> (vk::PipelineStageFlags, vk::AccessFlags) {
    match layout {
        vk::ImageLayout::TRANSFER_DST_OPTIMAL => (vk::PipelineStageFlags::TRANSFER, vk::AccessFlags::TRANSFER_WRITE),
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL => (vk::PipelineStageFlags::TRANSFER, vk::AccessFlags::TRANSFER_READ),
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => {
            (vk::PipelineStageFlags::FRAGMENT_SHADER, vk::AccessFlags::SHADER_READ)
        }
        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL => (
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        ),
        vk::ImageLayout::PRESENT_SRC_KHR => (vk::PipelineStageFlags::BOTTOM_OF_PIPE, vk::AccessFlags::empty()),
        _ => (vk::PipelineStageFlags::TOP_OF_PIPE, vk::AccessFlags::empty()),
    }
}

/// Moves a texture to `layout`, waiting for what used it before (always: a texture drawn into twice in a row is
/// waited on too).
///
/// # Safety
/// `cmd` is `device`'s, recording, outside a render pass; the texture's image is alive.
pub(super) unsafe fn transition(
    device: &ash::Device,
    cmd: vk::CommandBuffer,
    t: &mut VkTexture,
    layout: vk::ImageLayout,
) {
    let (src_stage, src_access) = stage_of(t.layout);
    let (dst_stage, dst_access) = stage_of(layout);
    let barrier = vk::ImageMemoryBarrier::default()
        .src_access_mask(src_access)
        .dst_access_mask(dst_access)
        .old_layout(t.layout)
        .new_layout(layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(t.image)
        .subresource_range(color_range(0, vk::REMAINING_MIP_LEVELS));
    // SAFETY: the caller's promise.
    unsafe {
        device.cmd_pipeline_barrier(cmd, src_stage, dst_stage, vk::DependencyFlags::empty(), &[], &[], &[barrier])
    };
    t.layout = layout;
}

/// Whether `region` is within the texture (a copy beyond it would write past its memory: refused).
pub(super) fn within(region: IRect, id: TextureId) -> bool {
    region.x >= 0
        && region.y >= 0
        && region.width >= 0
        && region.height >= 0
        && region.x.checked_add(region.width).is_some_and(|right| right <= id.width())
        && region.y.checked_add(region.height).is_some_and(|bottom| bottom <= id.height())
}

fn color_range(level: u32, levels: u32) -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .base_mip_level(level)
        .level_count(levels)
        .layer_count(1)
}

fn color_layers(level: u32) -> vk::ImageSubresourceLayers {
    vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).mip_level(level).layer_count(1)
}

impl VulkanDevice {
    pub(super) fn make_view(&self, image: vk::Image, format: vk::Format, levels: u32) -> vk::ImageView {
        let info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(color_range(0, levels));
        // SAFETY: the image is the device's, made with this format and levels.
        unsafe { self.device.create_image_view(&info, None) }.unwrap_or_default()
    }

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
        let mip_levels = if usage.contains(TextureUsage::MIPMAPPED) { width.max(height).ilog2() + 1 } else { 1 };
        let mut image_usage = vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::TRANSFER_DST;
        if usage.contains(TextureUsage::SAMPLED) {
            image_usage |= vk::ImageUsageFlags::SAMPLED;
        }
        if usage.contains(TextureUsage::RENDER_TARGET) {
            image_usage |= vk::ImageUsageFlags::COLOR_ATTACHMENT;
        }
        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk_format(format))
            .extent(vk::Extent3D { width: width as u32, height: height as u32, depth: 1 })
            .mip_levels(mip_levels)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(image_usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        // SAFETY: a valid image info (formats and usages every Vulkan device has).
        let image = unsafe { self.device.create_image(&info, None) }.ok()?;
        // SAFETY: the image is the device's and has no memory.
        let allocation =
            match unsafe { self.allocator.allocate(&self.device, Resource::Image(image), MemoryUsage::GPU) } {
                Ok(allocation) => allocation,
                Err(_) => {
                    // SAFETY: nothing uses it.
                    unsafe { self.device.destroy_image(image, None) };
                    return None;
                }
            };
        let view = self.make_view(image, vk_format(format), mip_levels);
        let id = self.textures.reserve(width, height, format);
        self.textures.fill(VkTexture {
            id,
            image,
            view,
            allocation: Some(allocation),
            layout: vk::ImageLayout::UNDEFINED,
            mip_levels,
            usage,
            framebuffers: Vec::new(),
            discarded: false,
        });
        Some(id)
    }

    /// A texture's image, view, framebuffers and memory destroyed (the GPU done with them).
    pub(super) fn destroy_texture(&mut self, t: VkTexture) {
        // SAFETY: the GPU is done with them (a deletion runs after its frame's fence), and nothing else refers to them.
        unsafe {
            for (_, framebuffer) in &t.framebuffers {
                self.device.destroy_framebuffer(*framebuffer, None);
            }
            self.device.destroy_image_view(t.view, None);
            if let Some(allocation) = t.allocation {
                self.device.destroy_image(t.image, None);
                self.allocator.free(&self.device, allocation);
            }
        }
    }

    pub(super) fn upload_texels(&mut self, id: TextureId, region: IRect, pixels: &[u8], row_bytes: usize) {
        assert!(within(region, id), "upload: {region:?} isn't within {id:?}");
        if region.is_empty() {
            return;
        }
        let row = region.width as usize * id.format().bytes_per_texel();
        let size = row * region.height as usize;
        assert!(
            row_bytes >= row && pixels.len() >= row_bytes * (region.height as usize - 1) + row,
            "upload: {} bytes, {row_bytes} a row, for {}x{} texels of {:?}",
            pixels.len(),
            region.width,
            region.height,
            id.format()
        );
        let Some((buffer, offset, memory)) = self.frame_memory(size, 16) else { return };
        let swap = id.format() == Format::Bgra8;
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
        let (device, cmd) = (&self.device, self.cmd);
        let Some(t) = self.textures.get_mut(id) else { return };
        let copy = vk::BufferImageCopy::default()
            .buffer_offset(offset)
            .image_subresource(color_layers(0))
            .image_offset(vk::Offset3D { x: region.x, y: region.y, z: 0 })
            .image_extent(vk::Extent3D { width: region.width as u32, height: region.height as u32, depth: 1 });
        // SAFETY: the frame's command buffer records outside passes (uploads are made there); the buffer is the
        // frame's, alive until its fence; the region is within the image (the caller's).
        unsafe {
            transition(device, cmd, t, vk::ImageLayout::TRANSFER_DST_OPTIMAL);
            device.cmd_copy_buffer_to_image(cmd, buffer, t.image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[copy]);
        }
        self.recorded = true;
    }

    pub(super) fn make_mipmaps(&mut self, id: TextureId) {
        if self.textures.get(id).is_none_or(|t| t.mip_levels <= 1) {
            return;
        }
        self.mark(vk::PipelineStageFlags::TOP_OF_PIPE, format!("mipmaps {}x{}", id.width(), id.height()));
        let (device, cmd) = (&self.device, self.cmd);
        let Some(t) = self.textures.get_mut(id) else { return };
        let image = t.image;
        // Each level read once written: from TRANSFER_DST to TRANSFER_SRC, then blitted, halved, into the next.
        let to_source = |level: u32| {
            let barrier = vk::ImageMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image)
                .subresource_range(color_range(level, 1));
            // SAFETY: as below.
            unsafe {
                device.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier],
                )
            };
        };
        // SAFETY: the frame's command buffer records outside passes; every level is within the image.
        unsafe {
            transition(device, cmd, t, vk::ImageLayout::TRANSFER_DST_OPTIMAL);
            let (mut w, mut h) = (id.width(), id.height());
            for level in 1..t.mip_levels {
                to_source(level - 1);
                let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
                let blit = vk::ImageBlit::default()
                    .src_subresource(color_layers(level - 1))
                    .src_offsets([vk::Offset3D::default(), vk::Offset3D { x: w, y: h, z: 1 }])
                    .dst_subresource(color_layers(level))
                    .dst_offsets([vk::Offset3D::default(), vk::Offset3D { x: nw, y: nh, z: 1 }]);
                device.cmd_blit_image(
                    cmd,
                    image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[blit],
                    vk::Filter::LINEAR,
                );
                (w, h) = (nw, nh);
            }
            to_source(t.mip_levels - 1);
        }
        t.layout = vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
        self.recorded = true;
        self.mark(vk::PipelineStageFlags::BOTTOM_OF_PIPE, String::new());
    }

    pub(super) fn copy_texels(&mut self, source: TextureId, region: IRect, destination: TextureId, x: i32, y: i32) {
        let at = IRect::new(x, y, region.width, region.height);
        assert!(
            source.format() == destination.format() && within(region, source) && within(at, destination),
            "copy_texture: {region:?} of {source:?} to {at:?} of {destination:?}"
        );
        if region.is_empty() {
            return;
        }
        self.mark(vk::PipelineStageFlags::TOP_OF_PIPE, format!("copy {}x{}", region.width, region.height));
        let (device, cmd) = (&self.device, self.cmd);
        let copy = vk::ImageCopy::default()
            .src_subresource(color_layers(0))
            .dst_subresource(color_layers(0))
            .src_offset(vk::Offset3D { x: region.x, y: region.y, z: 0 })
            .dst_offset(vk::Offset3D { x, y, z: 0 })
            .extent(vk::Extent3D { width: region.width as u32, height: region.height as u32, depth: 1 });
        if let Some((src, dst)) = self.textures.pair_mut(source, destination) {
            // SAFETY: the frame's command buffer records outside passes; the regions are within the images.
            unsafe {
                transition(device, cmd, src, vk::ImageLayout::TRANSFER_SRC_OPTIMAL);
                transition(device, cmd, dst, vk::ImageLayout::TRANSFER_DST_OPTIMAL);
                device.cmd_copy_image(
                    cmd,
                    src.image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    dst.image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[copy],
                );
            }
            self.recorded = true;
        }
        self.mark(vk::PipelineStageFlags::BOTTOM_OF_PIPE, String::new());
    }

    /// The texels of `region`, read back once what was recorded so far is done (the frame going on after it in
    /// another command buffer).
    pub(super) fn read_texels(&mut self, id: TextureId, region: IRect) -> Vec<u8> {
        assert!(within(region, id), "read_pixels: {region:?} isn't within {id:?}");
        if region.is_empty() || self.textures.get(id).is_none() {
            return Vec::new();
        }
        let size = region.width as u64 * region.height as u64 * id.format().bytes_per_texel() as u64;
        let info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(vk::BufferUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        // SAFETY: a valid buffer info.
        let Ok(buffer) = (unsafe { self.device.create_buffer(&info, None) }) else { return Vec::new() };
        // SAFETY: the buffer is the device's and has no memory.
        let allocation =
            match unsafe { self.allocator.allocate(&self.device, Resource::Buffer(buffer), MemoryUsage::READBACK) } {
                Ok(allocation) => allocation,
                Err(_) => {
                    // SAFETY: nothing uses it.
                    unsafe { self.device.destroy_buffer(buffer, None) };
                    return Vec::new();
                }
            };
        let (device, cmd) = (&self.device, self.cmd);
        let t = self.textures.get_mut(id).expect("checked above");
        let before = t.layout;
        let copy = vk::BufferImageCopy::default()
            .image_subresource(color_layers(0))
            .image_offset(vk::Offset3D { x: region.x, y: region.y, z: 0 })
            .image_extent(vk::Extent3D { width: region.width as u32, height: region.height as u32, depth: 1 });
        // SAFETY: the frame's command buffer records outside passes; the region is within the image, the buffer as
        // big as it.
        unsafe {
            transition(device, cmd, t, vk::ImageLayout::TRANSFER_SRC_OPTIMAL);
            device.cmd_copy_image_to_buffer(cmd, t.image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, buffer, &[copy]);
            if before != vk::ImageLayout::UNDEFINED {
                transition(device, cmd, t, before);
            }
        }
        // Submitted now with a fence of its own, waited for, and the frame started again in another command buffer.
        // SAFETY: a valid fence info.
        let fence = unsafe { self.device.create_fence(&vk::FenceCreateInfo::default(), None) }.unwrap_or_default();
        self.submit(fence, false);
        let mut out = vec![0u8; size as usize];
        // SAFETY: the fence is signaled once the copy is done; the buffer's memory is mapped (host-visible), as big as
        // `size` from where `mapped` points, and made visible to the CPU by the invalidation.
        unsafe {
            let _ = self.device.wait_for_fences(&[fence], true, u64::MAX);
            self.device.destroy_fence(fence, None);
            let _ = self.allocator.invalidate(&self.device, &allocation);
            if let Some(mapped) = self.allocator.mapped(&allocation) {
                std::ptr::copy_nonoverlapping(mapped.as_ptr(), out.as_mut_ptr(), out.len());
            }
            self.device.destroy_buffer(buffer, None);
            self.allocator.free(&self.device, allocation);
        }
        if id.format() == Format::Bgra8 {
            for texel in out.as_chunks_mut::<4>().0 {
                texel.swap(0, 2);
            }
        }
        self.start_commands();
        out
    }

    /// A pass's label for the GPU's times.
    pub(super) fn pass_label_of(&self, id: TextureId) -> String {
        let window = self.textures.get(id).is_some_and(|t| t.allocation.is_none());
        pass_label(id.width(), id.height(), id.format(), window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texture(id: TextureId) -> VkTexture {
        VkTexture {
            id,
            image: vk::Image::null(),
            view: vk::ImageView::null(),
            allocation: None,
            layout: vk::ImageLayout::UNDEFINED,
            mip_levels: 1,
            usage: TextureUsage::SAMPLED,
            framebuffers: Vec::new(),
            discarded: false,
        }
    }

    #[test]
    fn slots_are_used_again_under_new_generations() {
        let mut textures = Textures::default();
        let a = textures.reserve(4, 4, Format::Rgba8);
        textures.fill(texture(a));
        let b = textures.reserve(8, 2, Format::R16f);
        textures.fill(texture(b));
        assert_eq!((a.slot(), b.slot()), (0, 1));
        assert!(textures.get(a).is_some() && textures.get(b).is_some());
        assert!(textures.remove(a).is_some());
        assert!(textures.get(a).is_none() && textures.remove(a).is_none());
        // The slot again, with another generation: the old id finds nothing.
        let c = textures.reserve(16, 16, Format::Rgba8);
        textures.fill(texture(c));
        assert_eq!((c.slot(), c.generation()), (0, a.generation() + 1));
        assert!(textures.get(a).is_none());
        assert_eq!(textures.get(c).map(|t| t.id), Some(c));
        // Two at once, either way round; not one twice.
        let (x, y) = textures.pair_mut(b, c).unwrap();
        assert_eq!((x.id, y.id), (b, c));
        let (x, y) = textures.pair_mut(c, b).unwrap();
        assert_eq!((x.id, y.id), (c, b));
        assert!(textures.pair_mut(c, c).is_none() && textures.pair_mut(a, b).is_none());
        assert_eq!(textures.drain().len(), 2);
    }
}
