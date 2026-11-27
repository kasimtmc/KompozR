//! The hardware layer on Metal (macOS, iOS), as the C++ Kompoz's `metal_device.mm`, through objc2's bindings of
//! Metal and QuartzCore. Metal's clip space has y up ([`Device::clip_y_down`] false: the renderer's projection isn't
//! flipped) and its textures their top row first, as uploads and the renderer have them: nothing else is flipped.
//!
//! Each frame records into command buffers of one queue, in order: uploads, copies and mipmaps in blit encoders, each
//! pass in a render encoder (Metal tracks what each reads after what wrote it, so no barriers are put in); a readback
//! commits what is recorded and waits (Metal can), the frame going on in the next. Per-frame memory is shared buffers
//! the CPU writes, used again once the GPU is done with the frame ([`FRAMES_IN_FLIGHT`]). Shaders are compiled from
//! their MSL source (`cargo xtask shaders`' as committed: uniforms at buffer 0, instances at buffer 1, texture and
//! sampler i at index i, specialization constants as function constants 0, 1, …), without fast math (the same
//! arithmetic as the other backends). Pipelines are made on any thread ([`Device::pipeline_factory`]: Metal's device
//! is thread-safe) and kept between runs in a binary archive ([`pipeline`]).
//!
//! Validation: Metal's API validation layer is the process environment's `MTL_DEBUG_LAYER=1`, read when Metal makes
//! its first device; `KOMPOZ_GPU_VALIDATION=1` asks for it as it does on Vulkan, and counts what Metal reports to the
//! device itself (shaders that don't compile, command buffers that fail, with each encoder's status).
//!
//! The `unsafe` code of the Metal backend is here, each block saying why it holds: most is objc2's, whose methods
//! that index Metal's tables or read memory through pointers are unsafe.

#![allow(unsafe_code)]

mod frame;
mod pass;
mod pipeline;
mod texture;

pub use objc2_metal;
pub use objc2_quartz_core;

use self::frame::{FRAMES_IN_FLIGHT, Frame};
use self::pipeline::{Maker, PipelineCache};
use self::texture::Textures;
use crate::device::{
    Backend, BufferRef, ClearColor, Device, Format, GpuFrameTime, IRect, MAX_TEXTURES, Pipeline, PipelineDesc,
    PipelineFactory, PipelineId, RetiredQueue, Sampler, Surface, Texture, TextureId, TextureUsage, env_is_one, log,
};
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSError;
use objc2_metal::{
    MTLBlitCommandEncoder, MTLCommandBuffer, MTLCommandQueue, MTLCreateSystemDefaultDevice, MTLDevice, MTLPixelFormat,
    MTLRenderCommandEncoder, MTLRenderPipelineState, MTLSamplerState,
};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// The Metal device.
pub struct MetalDevice {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    /// Makes the pipelines (here and on other threads) and keeps them.
    maker: Arc<Maker>,
    retired: Arc<RetiredQueue>,
    /// Errors Metal reported (from its threads too: command buffers' completion handlers count into it).
    errors: Arc<AtomicU32>,
    /// Command buffers made with each encoder's status in their errors (`KOMPOZ_GPU_VALIDATION=1`).
    validating: bool,

    /// The layer presented to (none: drawing only into textures), and the drawable's size it was last given.
    layer: Option<Retained<CAMetalLayer>>,
    layer_size: (i32, i32),
    drawable_size: Option<Box<dyn Fn() -> (i32, i32)>>,
    /// This frame's drawable, and its texture's id (a slot of `textures` for the frame).
    drawable: Option<Retained<ProtocolObject<dyn CAMetalDrawable>>>,
    window: Option<TextureId>,

    textures: Textures,
    samplers: Vec<(Sampler, Retained<ProtocolObject<dyn MTLSamplerState>>)>,
    /// The pipelines clearing part of a target, by its format.
    clears: Vec<(Format, Retained<ProtocolObject<dyn MTLRenderPipelineState>>)>,

    frames: [Frame; FRAMES_IN_FLIGHT],
    frame_index: usize,
    in_frame: bool,
    /// The command buffer recording, whether anything is in it yet, and (timed) the pass, copy or mipmaps it holds.
    commands: Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    recorded: bool,
    label: Option<String>,
    blit: Option<Retained<ProtocolObject<dyn MTLBlitCommandEncoder>>>,
    pass: Option<Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>>,
    timing: bool,
    timed: Vec<GpuFrameTime>,

    /// The pass's target, where it draws (scissors stay within it), and whether its scissor holds nothing (Metal takes
    /// no empty scissor: nothing is drawn instead).
    target: Option<TextureId>,
    pass_area: IRect,
    scissor_empty: bool,
    bound: [(Option<TextureId>, Sampler); MAX_TEXTURES],
}

/// An `NSError`'s description, for messages.
fn describe(error: Option<&NSError>) -> String {
    error.map_or_else(|| "?".to_string(), |e| e.localizedDescription().to_string())
}

/// Metal's pixel format for a texture's.
fn mtl_format(format: Format) -> MTLPixelFormat {
    match format {
        Format::Rgba8 => MTLPixelFormat::RGBA8Unorm,
        Format::Bgra8 => MTLPixelFormat::BGRA8Unorm,
        Format::R16f => MTLPixelFormat::R16Float,
        Format::Rgba32f => MTLPixelFormat::RGBA32Float,
        Format::Rgba16f => MTLPixelFormat::RGBA16Float,
    }
}

impl MetalDevice {
    /// A device presenting to `surface`'s layer (or drawing only into textures without one); why not if this system
    /// has no Metal device. `KOMPOZ_GPU_DEVICE=<part of a name>` picks the GPU (on macOS, where there may be several).
    pub fn new(surface: Surface) -> Result<MetalDevice, String> {
        let Surface { metal, drawable_size, pipeline_cache } = surface;
        autoreleasepool(|_| {
            let validating = env_is_one("KOMPOZ_GPU_VALIDATION");
            if validating && !env_is_one("MTL_DEBUG_LAYER") {
                // (Set by the process's environment: Metal reads it once, as it makes its first device, and this
                // process's environment can't be changed safely while other threads may read it.)
                log("kompoz: GPU: Metal's validation layer is on with MTL_DEBUG_LAYER=1 in the environment");
            }
            let device = Self::pick_device().ok_or("no Metal device")?;
            let queue = device.newCommandQueue().ok_or("Metal: no command queue")?;
            let layer = metal.map(|surface| {
                // SAFETY: the platform hands a `CAMetalLayer *` it keeps alive for as long as the device lives
                // (MetalSurface); retained here as well.
                let layer = unsafe { Retained::retain(surface.layer.as_ptr().cast::<CAMetalLayer>()) }
                    .expect("MetalSurface's layer isn't null");
                layer.setDevice(Some(&device));
                layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
                // (The renderer copies from it: blend modes read what is below.)
                layer.setFramebufferOnly(false);
                layer
            });
            let errors = Arc::new(AtomicU32::new(0));
            let retired = Arc::new(RetiredQueue::default());
            let cache = pipeline_cache.and_then(|file| PipelineCache::open(&device, file));
            let maker = Arc::new(Maker::new(device.clone(), cache, errors.clone(), retired.clone()));
            log(&format!("kompoz: GPU: Metal on {}", device.name()));
            Ok(MetalDevice {
                device,
                queue,
                maker,
                retired,
                errors,
                validating,
                layer,
                layer_size: (0, 0),
                drawable_size,
                drawable: None,
                window: None,
                textures: Textures::default(),
                samplers: Vec::new(),
                clears: Vec::new(),
                frames: Default::default(),
                frame_index: 0,
                in_frame: false,
                commands: None,
                recorded: false,
                label: None,
                blit: None,
                pass: None,
                timing: false,
                timed: Vec::new(),
                target: None,
                pass_area: IRect::default(),
                scissor_empty: false,
                bound: [(None, Sampler::default()); MAX_TEXTURES],
            })
        })
    }

    /// The GPU `KOMPOZ_GPU_DEVICE` names (part of its name), else the system's default one.
    fn pick_device() -> Option<Retained<ProtocolObject<dyn MTLDevice>>> {
        #[cfg(target_os = "macos")]
        if let Some(wanted) = kompoz_runtime::environment::var("KOMPOZ_GPU_DEVICE").filter(|w| !w.is_empty()) {
            let all = objc2_metal::MTLCopyAllDevices();
            if let Some(device) = all.iter().find(|d| d.name().to_string().contains(&wanted)) {
                return Some(device);
            }
        }
        MTLCreateSystemDefaultDevice()
    }

    /// What records outside passes (uploads, copies, mipmaps, readbacks, a pass's start) needs: a frame begun and no
    /// pass going.
    fn expect_outside_passes(&self, call: &str) {
        assert!(self.in_frame && self.target.is_none(), "Device::{call} needs a frame begun and no pass going");
    }

    /// What records in a pass needs: a pass begun.
    fn expect_pass(&self, call: &str) {
        assert!(self.target.is_some(), "Device::{call} needs a pass begun");
    }

    /// The render encoder of the pass going.
    fn encoder(&self) -> &ProtocolObject<dyn MTLRenderCommandEncoder> {
        self.pass.as_deref().expect("a pass begun")
    }
}

impl Drop for MetalDevice {
    fn drop(&mut self) {
        // (Their completion handlers count into this, and what they use goes with it.)
        autoreleasepool(|_| {
            if self.in_frame {
                self.end_frame(false);
            }
            self.wait_idle();
            // (Pipelines made on other threads after the last frame kept too.)
            self.maker.keep_pipelines();
        });
    }
}

impl Device for MetalDevice {
    fn backend(&self) -> Backend {
        Backend::Metal
    }

    fn device_name(&self) -> String {
        self.device.name().to_string()
    }

    fn validation_errors(&self) -> u32 {
        self.errors.load(Ordering::Relaxed)
    }

    fn clip_y_down(&self) -> bool {
        false
    }

    fn create_texture(&mut self, width: i32, height: i32, format: Format, usage: TextureUsage) -> Option<Texture> {
        let id = autoreleasepool(|_| self.make_texture(width, height, format, usage))?;
        Some(Texture::new(id, self.retired.clone()))
    }

    fn create_pipeline(&mut self, desc: &PipelineDesc<'_>) -> Option<Pipeline> {
        self.maker.make(desc)
    }

    /// (Metal's device makes pipelines on any thread.)
    fn pipeline_factory(&self) -> Option<Arc<dyn PipelineFactory>> {
        Some(self.maker.clone())
    }

    fn swapchain_format(&self) -> Format {
        Format::Bgra8
    }

    fn begin_frame(&mut self) {
        autoreleasepool(|_| self.start_frame());
    }

    fn swapchain_texture(&mut self) -> Option<TextureId> {
        autoreleasepool(|_| self.acquire())
    }

    fn write_frame_data(&mut self, data: &[u8], alignment: usize) -> Option<BufferRef> {
        // (Outside frames the frame's memory may still be the GPU's.)
        assert!(self.in_frame, "Device::write_frame_data needs a frame begun");
        autoreleasepool(|_| self.write_data(data, alignment))
    }

    fn upload(&mut self, texture: TextureId, region: IRect, pixels: &[u8], row_bytes: usize) {
        self.expect_outside_passes("upload");
        autoreleasepool(|_| self.upload_texels(texture, region, pixels, row_bytes));
    }

    fn begin_pass(&mut self, target: TextureId, clear: Option<ClearColor>, reads: &[TextureId], area: Option<IRect>) {
        self.expect_outside_passes("begin_pass");
        // (Metal tracks what a pass reads after what wrote it: `reads` needs nothing more here.)
        let _ = reads;
        autoreleasepool(|_| self.start_pass(target, clear, area));
    }

    fn set_pipeline(&mut self, pipeline: PipelineId) {
        self.expect_pass("set_pipeline");
        if let Some(state) = self.maker.pipeline_state(pipeline) {
            self.encoder().setRenderPipelineState(&state);
        }
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
        self.expect_pass("set_texture");
        autoreleasepool(|_| self.bind_texture(slot, texture, sampler));
    }

    fn draw(&mut self, instances: Option<BufferRef>, vertex_count: u32, instance_count: u32) {
        self.expect_pass("draw");
        self.draw_instances(instances, vertex_count, instance_count);
    }

    fn end_pass(&mut self) {
        self.expect_pass("end_pass");
        autoreleasepool(|_| self.finish_pass());
    }

    fn generate_mipmaps(&mut self, texture: TextureId) {
        self.expect_outside_passes("generate_mipmaps");
        autoreleasepool(|_| self.make_mipmaps(texture));
    }

    fn discard(&mut self, texture: TextureId) {
        if let Some(t) = self.textures.get_mut(texture) {
            t.discarded = true;
        }
    }

    fn copy_texture(&mut self, source: TextureId, region: IRect, destination: TextureId, x: i32, y: i32) {
        self.expect_outside_passes("copy_texture");
        autoreleasepool(|_| self.copy_texels(source, region, destination, x, y));
    }

    fn read_pixels(&mut self, texture: TextureId, region: IRect) -> Vec<u8> {
        self.expect_outside_passes("read_pixels");
        autoreleasepool(|_| self.read_texels(texture, region))
    }

    fn end_frame(&mut self, present: bool) {
        if !self.in_frame {
            return;
        }
        self.expect_outside_passes("end_frame");
        autoreleasepool(|_| self.finish_frame(present));
    }

    fn set_gpu_timing(&mut self, on: bool) -> bool {
        // (Command buffers' GPU start and end times: each pass, copy and mipmap chain of a timed frame is recorded
        // into a command buffer of its own, and timed by it.)
        self.timing = on;
        true
    }

    fn take_gpu_times(&mut self) -> Vec<GpuFrameTime> {
        std::mem::take(&mut self.timed)
    }

    fn wait_idle(&mut self) {
        for frame in &mut self.frames {
            if let Some(submitted) = &frame.submitted {
                submitted.waitUntilCompleted();
            }
        }
    }
}
