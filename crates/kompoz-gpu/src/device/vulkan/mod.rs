//! The hardware layer on Vulkan (Linux, Windows, Android), as the C++ Kompoz's `vulkan_device.cpp`: Vulkan 1.1 with
//! `VK_KHR_swapchain` and nothing else required, so that it runs on every Vulkan Android device. The loader comes
//! from the system ([`ash::Entry::load`]); memory from our own allocator ([`alloc`]).
//!
//! Textures remember their layout, and passes say which textures they read, so the barriers between uploading,
//! drawing into and sampling a texture are put in here. Classic render passes (cached by format and how they
//! start) with framebuffers cached per texture; one descriptor set layout (six combined image samplers at bindings
//! 1..6) and 128 bytes of push constants for every pipeline; no MSAA (anti-aliasing is the shaders'). Frames in
//! flight: two, each with its own command buffers, descriptor pools and memory for what the CPU writes once.
//!
//! The only `unsafe` code of the crate is here, each block saying why it holds.

#![allow(unsafe_code)]

pub mod alloc;
mod frame;
mod instance;
mod pass;
mod pipeline;
mod swapchain;
mod texture;
mod timing;

pub use ash;

use self::alloc::GpuAllocator;
use self::frame::{FRAMES_IN_FLIGHT, Frame};
use self::instance::{Choice, Instance, pick_physical_device};
use self::pipeline::PipelineMaker;
use self::swapchain::Swapchain;
use self::texture::Textures;
use crate::device::{
    Backend, BufferRef, ClearColor, Device, Format, GpuFrameTime, IRect, MAX_TEXTURES, MAX_UNIFORMS, Pipeline,
    PipelineDesc, PipelineFactory, PipelineId, RetiredQueue, Sampler, Surface, Texture, TextureId, TextureUsage, log,
};
use ash::vk;
use std::ffi::CStr;
use std::path::PathBuf;
use std::sync::Arc;

/// What presenting with Vulkan needs from the window system (a platform layer fills it in).
pub struct VulkanSurface {
    /// The instance extensions presenting needs (`VK_KHR_surface` and the window system's own).
    pub instance_extensions: Vec<&'static CStr>,
    /// The surface (a `VkSurfaceKHR`) made for an instance; none if it can't be.
    pub create_surface: Box<dyn Fn(&ash::Entry, &ash::Instance) -> Option<vk::SurfaceKHR>>,
    /// The native window that surface is made for, where the system can give the window another (Android's
    /// `ANativeWindow`, a new one each time the app comes back from the background; 0 while it is away): the surface
    /// made again for a new one. None: the same for good.
    pub native_window: Option<Box<dyn Fn() -> usize>>,
    /// The surface's size is the drawable's, whatever the surface's `currentExtent` says: Android's, where the
    /// loader reports the size of the window's buffers as the last swapchain set them (its
    /// `native_window_set_buffers_dimensions`), not the view's new size — a SurfaceView the keyboard or a split
    /// screen resized would get a swapchain of the old size, stretched over it. False: `currentExtent`'s (the
    /// drawable's only where the surface leaves it to the swapchain).
    pub size_is_drawable: bool,
}

/// A surface presenting nowhere (`VK_EXT_headless_surface`, which lavapipe and Mesa's drivers have): the swapchain's
/// path without a display, for headless runs and tests (`KOMPOZ_HEADLESS_SWAPCHAIN=1`). It has no size of its own:
/// the swapchain takes the drawable's.
pub fn headless_surface() -> VulkanSurface {
    VulkanSurface {
        instance_extensions: vec![ash::khr::surface::NAME, ash::ext::headless_surface::NAME],
        create_surface: Box::new(|entry, instance| {
            let loader = ash::ext::headless_surface::Instance::new(entry, instance);
            let info = vk::HeadlessSurfaceCreateInfoEXT::default();
            // SAFETY: the instance was made with VK_EXT_headless_surface (this surface's instance_extensions); the
            // info is a valid default (no flags).
            unsafe { loader.create_headless_surface(&info, None) }.ok()
        }),
        native_window: None,
        size_is_drawable: false,
    }
}

/// Whether the loader has [`headless_surface`]'s extensions (Mesa's drivers on Linux do; mesa-dist-win's lavapipe on
/// Windows doesn't): a headless run's swapchain only where it does.
pub fn has_headless_surface() -> bool {
    // SAFETY: the loader loaded and let go here; only queried.
    let Ok(entry) = (unsafe { ash::Entry::load() }) else { return false };
    // SAFETY: a query of the loader just loaded.
    let available = unsafe { entry.enumerate_instance_extension_properties(None) }.unwrap_or_default();
    [ash::khr::surface::NAME, ash::ext::headless_surface::NAME]
        .iter()
        .all(|e| available.iter().any(|a| a.extension_name_as_c_str() == Ok(*e)))
}

/// The Vulkan device.
pub struct VulkanDevice {
    choice: Choice,
    device: ash::Device,
    swapchain_fn: Option<ash::khr::swapchain::Device>,
    queue: vk::Queue,
    allocator: GpuAllocator,
    maker: Arc<PipelineMaker>,
    set_layout: vk::DescriptorSetLayout,
    pipeline_layout: vk::PipelineLayout,
    /// Where the pipeline cache is kept between runs (none: not kept).
    cache_file: Option<PathBuf>,
    /// Presented with its alpha (a transparent window's), else opaque.
    transparent: bool,
    /// The cache's data's size when it was made (with the file's) or kept.
    cache_size: usize,
    retired: Arc<RetiredQueue>,
    drawable_size: Option<Box<dyn Fn() -> (i32, i32)>>,
    /// Said so by an acquire or a present (a new native window at the old one's address).
    surface_lost: bool,

    textures: Textures,
    samplers: Vec<(Sampler, vk::Sampler)>,
    timing: bool,
    timed: Vec<GpuFrameTime>,
    swapchain: Swapchain,

    frames: [Frame; FRAMES_IN_FLIGHT],
    frame_index: usize,
    in_frame: bool,
    cmd: vk::CommandBuffer,
    /// The next submit waits for the swapchain image to be acquired.
    wait_image: bool,
    /// Something in the command buffer since it was started (a barrier at least).
    recorded: bool,

    /// The pass's target.
    target: Option<TextureId>,
    /// Where the pass draws (its render area): scissors stay within it.
    pass_area: IRect,
    bound_pipeline: vk::Pipeline,
    bound: [(Option<TextureId>, Sampler); MAX_TEXTURES],
    textures_dirty: bool,

    /// (Last: let go of after everything made on it.)
    instance: Instance,
}

impl VulkanDevice {
    /// A device presenting to `surface`'s window (or drawing only into textures without one); why not if this
    /// system can't have one. `KOMPOZ_GPU_VALIDATION=1` turns the validation layer on, `KOMPOZ_GPU_DEVICE=<part of a
    /// name>` picks the GPU.
    pub fn new(surface: Surface) -> Result<VulkanDevice, String> {
        let Surface { vulkan, drawable_size, pipeline_cache, transparent } = surface;
        let instance = Instance::new(vulkan)?;
        let presents = instance.surface_fn.is_some();
        let Some(choice) = pick_physical_device(&instance) else {
            return Err(if presents {
                "no GPU that can present to the window with Vulkan 1.1"
            } else {
                "no GPU with Vulkan 1.1"
            }
            .into());
        };
        let priorities = [1.0];
        let queues = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(choice.queue_family)
            .queue_priorities(&priorities)];
        let extensions = [ash::khr::swapchain::NAME.as_ptr()];
        let info = vk::DeviceCreateInfo::default().queue_create_infos(&queues).enabled_extension_names(if presents {
            &extensions
        } else {
            &[]
        });
        // SAFETY: the physical device is the instance's, the queue family one of its, the extension one it has.
        let device = unsafe { instance.instance.create_device(choice.physical, &info, None) }
            .map_err(|e| format!("vkCreateDevice failed ({})", e.as_raw()))?;
        // SAFETY: the device was made with one queue of this family.
        let queue = unsafe { device.get_device_queue(choice.queue_family, 0) };
        let swapchain_fn = presents.then(|| ash::khr::swapchain::Device::new(&instance.instance, &device));
        let allocator = GpuAllocator::new(&instance.instance, choice.physical, &choice.properties);

        // One binding layout for every pipeline: textures at bindings 1..=MAX_TEXTURES, uniforms as push constants.
        let bindings: Vec<vk::DescriptorSetLayoutBinding> = (0..MAX_TEXTURES as u32)
            .map(|i| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(i + 1)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            })
            .collect();
        // SAFETY: valid infos for the device; on failure what was made is destroyed before it (destroying a null
        // handle does nothing).
        let (set_layout, pipeline_layout, cache) = unsafe {
            let set_layout = device
                .create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None);
            let set_layouts = [*set_layout.as_ref().unwrap_or(&vk::DescriptorSetLayout::null())];
            let ranges = [vk::PushConstantRange::default()
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                .size(MAX_UNIFORMS as u32)];
            let pipeline_layout = device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts).push_constant_ranges(&ranges),
                None,
            );
            // The pipelines compiled in earlier runs, if this GPU and driver made them.
            let cache = Self::load_pipeline_cache(&device, pipeline_cache.as_deref(), &choice.properties);
            match (set_layout, pipeline_layout, cache) {
                (Ok(s), Ok(p), Ok(c)) => (s, p, c),
                (s, p, c) => {
                    device.destroy_pipeline_cache(c.unwrap_or_default(), None);
                    device.destroy_pipeline_layout(p.unwrap_or_default(), None);
                    device.destroy_descriptor_set_layout(s.unwrap_or_default(), None);
                    device.destroy_device(None);
                    return Err("the device's layouts couldn't be made".into());
                }
            }
        };
        let retired = Arc::new(RetiredQueue::default());
        let maker = Arc::new(PipelineMaker::new(device.clone(), pipeline_layout, cache, retired.clone()));
        let mut made = VulkanDevice {
            choice,
            device,
            swapchain_fn,
            queue,
            allocator,
            maker,
            set_layout,
            pipeline_layout,
            cache_file: pipeline_cache,
            transparent,
            cache_size: 0,
            retired,
            drawable_size,
            surface_lost: false,
            textures: Textures::default(),
            samplers: Vec::new(),
            timing: false,
            timed: Vec::new(),
            swapchain: Swapchain::default(),
            frames: Default::default(),
            frame_index: 0,
            in_frame: false,
            cmd: vk::CommandBuffer::null(),
            wait_image: false,
            recorded: false,
            target: None,
            pass_area: IRect::default(),
            bound_pipeline: vk::Pipeline::null(),
            bound: [(None, Sampler::default()); MAX_TEXTURES],
            textures_dirty: true,
            instance,
        };
        // (What the driver took of the file.)
        made.read_cache_size();
        made.make_frames().map_err(|e| format!("the device's frames couldn't be made ({})", e.as_raw()))?;
        log(&format!("kompoz: GPU: Vulkan on {}", made.choice.device_name()));
        Ok(made)
    }

    /// The device's memory in use, for tests and diagnostics.
    pub fn memory_stats(&self) -> alloc::Stats {
        self.allocator.stats()
    }

    /// What records outside passes (uploads, copies, mipmaps, readbacks, a pass's start) needs: a frame begun (its
    /// command buffer recording) and no pass going (copies and barriers can't be in a render pass).
    fn expect_outside_passes(&self, call: &str) {
        assert!(self.in_frame && self.target.is_none(), "Device::{call} needs a frame begun and no pass going");
    }

    /// What records in a pass needs: a pass begun.
    fn expect_pass(&self, call: &str) {
        assert!(self.target.is_some(), "Device::{call} needs a pass begun");
    }
}

impl Drop for VulkanDevice {
    fn drop(&mut self) {
        // SAFETY: waits for the device; afterwards nothing it made is in use, and each object is destroyed once
        // (destroying a null handle does nothing).
        unsafe {
            let _ = self.device.device_wait_idle();
        }
        self.collect_retired();
        for i in 0..FRAMES_IN_FLIGHT {
            self.run_deletions(i);
        }
        self.destroy_swapchain();
        for texture in self.textures.drain() {
            self.destroy_texture(texture);
        }
        let (passes, pipelines) = self.maker.shut();
        // SAFETY: as above.
        unsafe {
            for frame in &mut self.frames {
                for chunk in frame.chunks.drain(..) {
                    self.device.destroy_buffer(chunk.buffer, None);
                    self.allocator.free(&self.device, chunk.allocation);
                }
                for pool in frame.descriptor_pools.drain(..) {
                    self.device.destroy_descriptor_pool(pool, None);
                }
                self.device.destroy_command_pool(frame.pool, None);
                self.device.destroy_fence(frame.fence, None);
                self.device.destroy_semaphore(frame.image_available, None);
                self.device.destroy_query_pool(frame.timestamps, None);
            }
            for pipeline in pipelines {
                self.device.destroy_pipeline(pipeline, None);
            }
            for pass in passes {
                self.device.destroy_render_pass(pass, None);
            }
            for (_, sampler) in self.samplers.drain(..) {
                self.device.destroy_sampler(sampler, None);
            }
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_descriptor_set_layout(self.set_layout, None);
            self.device.destroy_pipeline_cache(self.maker.cache, None);
            self.allocator.destroy(&self.device);
            self.device.destroy_device(None);
        }
    }
}

impl Device for VulkanDevice {
    fn backend(&self) -> Backend {
        Backend::Vulkan
    }

    fn device_name(&self) -> String {
        self.choice.device_name()
    }

    fn validation_errors(&self) -> u32 {
        self.instance.validation_errors()
    }

    fn clip_y_down(&self) -> bool {
        true
    }

    fn create_texture(&mut self, width: i32, height: i32, format: Format, usage: TextureUsage) -> Option<Texture> {
        let id = self.make_texture(width, height, format, usage)?;
        Some(Texture::new(id, self.retired.clone()))
    }

    fn create_pipeline(&mut self, desc: &PipelineDesc<'_>) -> Option<Pipeline> {
        self.maker.create_pipeline(desc)
    }

    /// (The pipeline cache is synchronized by the driver.)
    fn pipeline_factory(&self) -> Option<Arc<dyn PipelineFactory>> {
        Some(self.maker.clone())
    }

    fn swapchain_format(&self) -> Format {
        self.swapchain.format
    }

    fn begin_frame(&mut self) {
        self.start_frame();
    }

    fn swapchain_texture(&mut self) -> Option<TextureId> {
        self.acquire()
    }

    fn swapchain_turns(&self) -> i32 {
        self.swapchain.turns
    }

    fn swapchain_age(&self) -> i32 {
        if self.swapchain.acquired { self.swapchain.age } else { 0 }
    }

    fn surface_changed(&mut self) {
        if self.instance.window.as_ref().is_some_and(|w| w.native_window.is_some()) && !self.swapchain.acquired {
            self.surface_current();
        }
    }

    fn write_frame_data(&mut self, data: &[u8], alignment: usize) -> Option<BufferRef> {
        // (Outside frames the frame's memory may still be the GPU's.)
        assert!(self.in_frame, "Device::write_frame_data needs a frame begun");
        self.write_data(data, alignment)
    }

    fn upload(&mut self, texture: TextureId, region: IRect, pixels: &[u8], row_bytes: usize) {
        self.expect_outside_passes("upload");
        self.upload_texels(texture, region, pixels, row_bytes);
    }

    fn begin_pass(&mut self, target: TextureId, clear: Option<ClearColor>, reads: &[TextureId], area: Option<IRect>) {
        self.expect_outside_passes("begin_pass");
        self.start_pass(target, clear, reads, area);
    }

    fn set_pipeline(&mut self, pipeline: PipelineId) {
        self.expect_pass("set_pipeline");
        self.use_pipeline(pipeline);
    }

    fn set_scissor(&mut self, rect: IRect) {
        self.expect_pass("set_scissor");
        self.set_pass_scissor(rect);
    }

    fn set_uniforms(&mut self, data: &[u8]) {
        self.expect_pass("set_uniforms");
        self.push_uniforms(data);
    }

    fn set_texture(&mut self, slot: usize, texture: TextureId, sampler: Sampler) {
        self.bind_texture(slot, texture, sampler);
    }

    fn draw(&mut self, instances: Option<BufferRef>, vertex_count: u32, instance_count: u32) {
        self.expect_pass("draw");
        self.draw_instances(instances, vertex_count, instance_count);
    }

    fn end_pass(&mut self) {
        self.expect_pass("end_pass");
        self.finish_pass();
    }

    fn generate_mipmaps(&mut self, texture: TextureId) {
        self.expect_outside_passes("generate_mipmaps");
        self.make_mipmaps(texture);
    }

    fn discard(&mut self, texture: TextureId) {
        if let Some(t) = self.textures.get_mut(texture) {
            t.discarded = true;
        }
    }

    fn copy_texture(&mut self, source: TextureId, region: IRect, destination: TextureId, x: i32, y: i32) {
        self.expect_outside_passes("copy_texture");
        self.copy_texels(source, region, destination, x, y);
    }

    fn read_pixels(&mut self, texture: TextureId, region: IRect) -> Vec<u8> {
        self.expect_outside_passes("read_pixels");
        self.read_texels(texture, region)
    }

    fn end_frame(&mut self, present: bool) {
        if !self.in_frame {
            return;
        }
        self.expect_outside_passes("end_frame");
        self.collect_retired();
        let acquired = self.swapchain.acquired;
        let present = present && acquired;
        let (device, cmd) = (&self.device, self.cmd);
        let index = self.swapchain.index as usize;
        if acquired && let Some(image) = self.swapchain.images.get(index).and_then(|&id| self.textures.get_mut(id)) {
            // SAFETY: the frame's command buffer records outside passes.
            unsafe { texture::transition(device, cmd, image, vk::ImageLayout::PRESENT_SRC_KHR) };
        }
        self.mark_end_of_frame();
        let fence = self.frames[self.frame_index].fence;
        // SAFETY: the fence is the frame's, signaled (waited for at the frame's start), and submitted with it below.
        unsafe {
            let _ = self.device.reset_fences(&[fence]);
        }
        let s = &mut self.swapchain;
        if acquired && !present {
            // An image was acquired but isn't shown: it must still be given back, so it is presented as drawn (the
            // window gets it only if something was), holding what isn't known any more.
            s.presented_at[index] = 0;
        } else if present {
            s.presents += 1;
            s.presented_at[index] = s.presents;
        }
        self.submit(fence, acquired);
        if acquired {
            self.present();
        }
        self.in_frame = false;
        self.swapchain.acquired = false;
        if self.maker.made.load(std::sync::atomic::Ordering::Relaxed) {
            self.keep_pipeline_cache();
        }
    }

    fn set_gpu_timing(&mut self, on: bool) -> bool {
        self.time_gpu(on)
    }

    fn take_gpu_times(&mut self) -> Vec<GpuFrameTime> {
        std::mem::take(&mut self.timed)
    }

    fn wait_idle(&mut self) {
        // SAFETY: waits for the device.
        unsafe {
            let _ = self.device.device_wait_idle();
        }
    }
}
