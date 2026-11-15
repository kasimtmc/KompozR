//! Frames in flight: each with its command pool (several command buffers: a readback splits a frame), its fence,
//! the semaphore its swapchain image is acquired with, its descriptor pools, its memory for what the CPU writes once
//! (bump-allocated chunks), what is destroyed once its GPU work is done, and its timestamps.

use super::VulkanDevice;
use super::alloc::{Allocation, MemoryUsage, Resource};
use super::texture::{VkTexture, transition};
use crate::device::{BufferRef, MAX_TEXTURES, Retired, TextureId};
use ash::vk;
use ash::vk::Handle;
use std::ptr::NonNull;

/// Frames the CPU records while the GPU works on the ones before.
pub(super) const FRAMES_IN_FLIGHT: usize = 2;
/// Per-frame memory comes in chunks of this (or one as big as asked).
const CHUNK_SIZE: u64 = 4 << 20;
/// Descriptor sets in each of a frame's pools (more pools are made when a frame binds more).
const SETS_PER_POOL: u32 = 4096;

/// A buffer of frame memory, mapped, handed out from its start each frame.
pub(super) struct Chunk {
    pub buffer: vk::Buffer,
    pub allocation: Allocation,
    pub mapped: NonNull<u8>,
    pub size: u64,
    pub used: u64,
}

/// What is destroyed once the GPU is done with the frame that let go of it.
pub(super) enum Deletion {
    Texture(VkTexture),
    Pipeline(vk::Pipeline),
}

#[derive(Default)]
pub(super) struct Frame {
    pub pool: vk::CommandPool,
    /// Allocated so far (a readback splits a frame).
    pub buffers: Vec<vk::CommandBuffer>,
    pub next_buffer: usize,
    pub fence: vk::Fence,
    pub image_available: vk::Semaphore,
    pub descriptor_pools: Vec<vk::DescriptorPool>,
    pub next_pool: usize,
    pub chunks: Vec<Chunk>,
    pub deletions: Vec<Deletion>,
    /// Timed ([`crate::device::Device::set_gpu_timing`]): its timestamps, each a pass's start (its label) or end
    /// (empty), the frame's start and end around them.
    pub timestamps: vk::QueryPool,
    pub timed: bool,
    pub marks: Vec<String>,
}

impl VulkanDevice {
    /// The frames' command pools, fences, semaphores and first descriptor pools.
    pub(super) fn make_frames(&mut self) -> Result<(), vk::Result> {
        for i in 0..FRAMES_IN_FLIGHT {
            let pool_info = vk::CommandPoolCreateInfo::default()
                .flags(vk::CommandPoolCreateFlags::TRANSIENT)
                .queue_family_index(self.choice.queue_family);
            // SAFETY: valid infos for the device; what is made is destroyed in drop.
            unsafe {
                self.frames[i].pool = self.device.create_command_pool(&pool_info, None)?;
                self.frames[i].fence = self
                    .device
                    .create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED), None)?;
                self.frames[i].image_available =
                    self.device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?;
            }
            let pool = self.make_descriptor_pool()?;
            self.frames[i].descriptor_pools.push(pool);
        }
        Ok(())
    }

    fn make_descriptor_pool(&self) -> Result<vk::DescriptorPool, vk::Result> {
        let sizes = [vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(SETS_PER_POOL * MAX_TEXTURES as u32)];
        let info = vk::DescriptorPoolCreateInfo::default().max_sets(SETS_PER_POOL).pool_sizes(&sizes);
        // SAFETY: a valid pool info.
        unsafe { self.device.create_descriptor_pool(&info, None) }
    }

    /// What was let go of (dropped textures and pipelines) into the current frame's deletions: destroyed once the
    /// GPU is done with it, and what refers to it now forgotten.
    pub(super) fn collect_retired(&mut self) {
        for retired in self.retired.take() {
            match retired {
                Retired::Texture(id) => {
                    if let Some(texture) = self.textures.remove(id) {
                        self.forget_texture(id);
                        self.frames[self.frame_index].deletions.push(Deletion::Texture(texture));
                    }
                }
                Retired::Pipeline(id) => {
                    let pipeline = vk::Pipeline::from_raw(id.raw());
                    if self.maker.forget(pipeline) {
                        if self.bound_pipeline == pipeline {
                            self.bound_pipeline = vk::Pipeline::null();
                        }
                        self.frames[self.frame_index].deletions.push(Deletion::Pipeline(pipeline));
                    }
                }
            }
        }
    }

    /// A texture no longer drawn into or bound.
    pub(super) fn forget_texture(&mut self, id: TextureId) {
        if self.target == Some(id) {
            self.target = None;
        }
        for bound in &mut self.bound {
            if bound.0 == Some(id) {
                bound.0 = None;
                self.textures_dirty = true;
            }
        }
    }

    pub(super) fn run_deletions(&mut self, frame: usize) {
        for deletion in std::mem::take(&mut self.frames[frame].deletions) {
            match deletion {
                Deletion::Texture(texture) => self.destroy_texture(texture),
                // SAFETY: the GPU is done with the frame that let go of it.
                Deletion::Pipeline(pipeline) => unsafe { self.device.destroy_pipeline(pipeline, None) },
            }
        }
    }

    pub(super) fn start_frame(&mut self) {
        self.collect_retired();
        self.frame_index = (self.frame_index + 1) % FRAMES_IN_FLIGHT;
        let i = self.frame_index;
        // SAFETY: the fence is the frame's; once it is signaled the GPU is done with the frame's command buffers,
        // descriptor sets and memory, which are then reset.
        unsafe {
            let _ = self.device.wait_for_fences(&[self.frames[i].fence], true, u64::MAX);
        }
        self.read_timestamps(i);
        self.run_deletions(i);
        let frame = &mut self.frames[i];
        // SAFETY: as above.
        unsafe {
            let _ = self.device.reset_command_pool(frame.pool, vk::CommandPoolResetFlags::empty());
            for pool in &frame.descriptor_pools {
                let _ = self.device.reset_descriptor_pool(*pool, vk::DescriptorPoolResetFlags::empty());
            }
        }
        frame.next_buffer = 0;
        frame.next_pool = 0;
        for chunk in &mut frame.chunks {
            chunk.used = 0;
        }
        self.in_frame = true;
        self.swapchain.acquired = false;
        self.wait_image = false;
        self.start_commands();
        self.frames[i].timed = self.timing && self.frames[i].timestamps != vk::QueryPool::null();
        if self.frames[i].timed {
            // SAFETY: the command buffer records, outside passes; the pool has this many queries.
            unsafe {
                self.device.cmd_reset_query_pool(self.cmd, self.frames[i].timestamps, 0, super::timing::TIMESTAMPS)
            };
            self.mark(vk::PipelineStageFlags::TOP_OF_PIPE, "frame".to_string());
        }
    }

    /// The frame's next command buffer begun.
    pub(super) fn start_commands(&mut self) {
        let frame = &mut self.frames[self.frame_index];
        if frame.next_buffer == frame.buffers.len() {
            let info = vk::CommandBufferAllocateInfo::default()
                .command_pool(frame.pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);
            // SAFETY: the pool is the frame's.
            let buffer = unsafe { self.device.allocate_command_buffers(&info) }
                .map_or_else(|_| vk::CommandBuffer::null(), |b| b[0]);
            frame.buffers.push(buffer);
        }
        self.cmd = frame.buffers[frame.next_buffer];
        frame.next_buffer += 1;
        self.recorded = false;
        let info = vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        // SAFETY: the command buffer is the frame's and not recording (its pool was reset, or it is new).
        unsafe {
            let _ = self.device.begin_command_buffer(self.cmd, &info);
        }
        self.bound_pipeline = vk::Pipeline::null();
        self.textures_dirty = true;
    }

    /// Submits what was recorded so far (waiting for the swapchain image the first time, and signaling the semaphore
    /// presenting waits for at the last), signaling `fence`.
    pub(super) fn submit(&mut self, fence: vk::Fence, present: bool) {
        let wait_semaphores = [self.frames[self.frame_index].image_available];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = [self.cmd];
        let signal = self.swapchain.render_finished.get(self.swapchain.index as usize).copied();
        let signal_semaphores: Vec<vk::Semaphore> = if present { signal.into_iter().collect() } else { Vec::new() };
        let mut info =
            vk::SubmitInfo::default().command_buffers(&command_buffers).signal_semaphores(&signal_semaphores);
        if self.wait_image {
            info = info.wait_semaphores(&wait_semaphores).wait_dst_stage_mask(&wait_stages);
            self.wait_image = false;
        }
        // SAFETY: the command buffer is recording (begun by start_commands) and ends here; the semaphores and fence
        // are the device's (the image's was signaled by its acquisition).
        unsafe {
            let _ = self.device.end_command_buffer(self.cmd);
            let _ = self.device.queue_submit(self.queue, &[info], fence);
        }
    }

    /// `size` bytes of this frame's memory at a multiple of `alignment` (16 at least): its buffer, where in it, and
    /// the bytes to write.
    pub(super) fn frame_memory(&mut self, size: usize, alignment: usize) -> Option<(vk::Buffer, u64, &mut [u8])> {
        let alignment = alignment.max(16) as u64;
        let size = size as u64;
        let frame = self.frame_index;
        let found =
            self.frames[frame].chunks.iter().position(|c| c.used.div_ceil(alignment) * alignment + size <= c.size);
        let index = match found {
            Some(index) => index,
            None => {
                let chunk = self.make_chunk(size.max(CHUNK_SIZE))?;
                self.frames[frame].chunks.push(chunk);
                self.frames[frame].chunks.len() - 1
            }
        };
        let chunk = &mut self.frames[frame].chunks[index];
        let start = chunk.used.div_ceil(alignment) * alignment;
        chunk.used = start + size;
        // SAFETY: the chunk's memory is mapped for good, `size` bytes from `start` are within it, handed out once
        // this frame (bump-allocated), and the GPU doesn't read them until the frame is submitted; the slice lives
        // no longer than the borrow of the device, and the chunk as long as the device.
        let bytes = unsafe { std::slice::from_raw_parts_mut(chunk.mapped.as_ptr().add(start as usize), size as usize) };
        Some((chunk.buffer, start, bytes))
    }

    fn make_chunk(&mut self, size: u64) -> Option<Chunk> {
        let info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        // SAFETY: a valid buffer info.
        let buffer = unsafe { self.device.create_buffer(&info, None) }.ok()?;
        // SAFETY: the buffer is the device's, without memory.
        let allocation =
            match unsafe { self.allocator.allocate(&self.device, Resource::Buffer(buffer), MemoryUsage::UPLOAD) } {
                Ok(allocation) => allocation,
                Err(_) => {
                    // SAFETY: nothing uses it.
                    unsafe { self.device.destroy_buffer(buffer, None) };
                    return None;
                }
            };
        let Some(mapped) = self.allocator.mapped(&allocation) else {
            // SAFETY: nothing uses them.
            unsafe {
                self.device.destroy_buffer(buffer, None);
                self.allocator.free(&self.device, allocation);
            }
            return None;
        };
        Some(Chunk { buffer, allocation, mapped, size, used: 0 })
    }

    pub(super) fn write_data(&mut self, data: &[u8], alignment: usize) -> Option<BufferRef> {
        let (buffer, offset, bytes) = self.frame_memory(data.len(), alignment)?;
        bytes.copy_from_slice(data);
        Some(BufferRef::new(buffer.as_raw(), offset))
    }

    /// A descriptor set for the textures bound (each slot unbound given slot 0's), bound.
    pub(super) fn bind_textures(&mut self) {
        let Some(first) = self.bound[0].0 else { return }; // (nothing bound yet)
        let mut infos = [vk::DescriptorImageInfo::default(); MAX_TEXTURES];
        for (info, bound) in infos.iter_mut().zip(self.bound) {
            let (id, sampler) = match bound {
                (Some(id), sampler) if self.textures.get(id).is_some() => (id, sampler),
                _ => (first, self.bound[0].1),
            };
            let Some(view) = self.textures.get(id).map(|t| t.view) else { return };
            *info = vk::DescriptorImageInfo::default()
                .sampler(self.sampler(sampler))
                .image_view(view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        }
        let Some(set) = self.allocate_set() else { return };
        let writes: Vec<vk::WriteDescriptorSet> = (0..MAX_TEXTURES)
            .map(|i| {
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(i as u32 + 1)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(std::slice::from_ref(&infos[i]))
            })
            .collect();
        // SAFETY: the set is this frame's, written before it is bound; its views and samplers are alive through the
        // frame (destruction waits for the frame's fence).
        unsafe {
            self.device.update_descriptor_sets(&writes, &[]);
            self.device.cmd_bind_descriptor_sets(
                self.cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[set],
                &[],
            );
        }
        self.textures_dirty = false;
    }

    /// A descriptor set from the frame's pools (another pool made when they are full).
    fn allocate_set(&mut self) -> Option<vk::DescriptorSet> {
        let layouts = [self.set_layout];
        loop {
            let frame = self.frame_index;
            if self.frames[frame].next_pool == self.frames[frame].descriptor_pools.len() {
                let pool = self.make_descriptor_pool().ok()?;
                self.frames[frame].descriptor_pools.push(pool);
            }
            let pool = self.frames[frame].descriptor_pools[self.frames[frame].next_pool];
            let info = vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts);
            // SAFETY: the pool is the frame's, the layout the device's.
            match unsafe { self.device.allocate_descriptor_sets(&info) } {
                Ok(sets) => return sets.first().copied(),
                Err(vk::Result::ERROR_OUT_OF_POOL_MEMORY | vk::Result::ERROR_FRAGMENTED_POOL) => {
                    self.frames[frame].next_pool += 1
                }
                Err(_) => return None,
            }
        }
    }

    /// The textures a pass reads made readable, outside it.
    pub(super) fn make_readable(&mut self, reads: &[TextureId], target: TextureId) {
        for &read in reads {
            if read == target {
                continue;
            }
            let (device, cmd) = (&self.device, self.cmd);
            if let Some(t) = self.textures.get_mut(read) {
                // SAFETY: the frame's command buffer records outside passes.
                unsafe { transition(device, cmd, t, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) };
                self.recorded = true;
            }
        }
    }
}
