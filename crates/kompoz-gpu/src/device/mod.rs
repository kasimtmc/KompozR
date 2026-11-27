//! The GPU renderer's hardware layer, as the C++ Kompoz's `platform/gpu/device.hpp`: the little of a graphics API
//! the renderer needs, implemented once for each native API (Vulkan on Linux, Windows and Android; Metal on Apple's
//! systems and WebGPU on the web, each the only one there: [`NATIVE_BACKEND`]). It is explicit like those APIs and
//! small: textures, pipelines made up front, render passes that say which textures they read, and per-frame memory
//! for what the CPU hands the GPU once. The renderer does the rest.
//!
//! Which backends a build has is the target's: [`vulkan`] (and `ash`) where Vulkan is the API, nowhere else (not on
//! Apple's systems, where Metal is, nor on the web, which has no Vulkan loader); `metal` on Apple's systems; the
//! WebGPU device comes with its own module, and until a build has one, [`create`] says it has no such backend. What presenting needs
//! from each window system is in [`Surface`]; reading pixels back is synchronous where the API can wait
//! ([`Device::read_pixels`]) and through a callback everywhere ([`Device::read_pixels_async`]: WebGPU can only map a
//! buffer asynchronously).
//!
//! ```text
//! device.begin_frame();
//! device.upload(atlas.id(), region, &pixels, row_bytes);          // outside passes
//! let target = device.swapchain_texture()?;
//! device.begin_pass(target, Some(clear), &[atlas.id()], None);
//! device.set_pipeline(pipeline.id());
//! device.set_texture(0, atlas.id(), sampler);
//! let instances = device.write_frame_data(&prims, 16);
//! device.draw(instances, 6, count);                               // instanced quads, made in the vertex shader
//! device.end_pass();
//! device.end_frame(true);
//! ```
//!
//! Textures and pipelines are opaque handles ([`TextureId`], [`PipelineId`]) the calls take, owned by RAII values
//! ([`Texture`], [`Pipeline`]) whose drop hands them back to the device, which destroys them once the GPU is done
//! with the frames that may use them.
//!
//! The environment:
//! - `KOMPOZ_GPU_VALIDATION=1`: the API's validation on where it has one (Vulkan's `VK_LAYER_KHRONOS_validation`;
//!   Metal's API validation is the environment's `MTL_DEBUG_LAYER=1`, which Metal reads itself: see `metal`),
//!   its messages logged and its errors counted ([`Device::validation_errors`]: tests want none).
//! - `KOMPOZ_GPU_DEVICE=<part of a name>`: the GPU whose name has it ("Intel", "RTX", "llvmpipe"), else an
//!   integrated one (it saves power), else a discrete one, else any.
//! - `KOMPOZ_GPU_REQUIRED=1` ([`gpu_required`]): no device is a failure (CI), not a reason to skip, for the tests.

#[cfg(target_vendor = "apple")]
pub mod metal;
mod pipeline_cache;
#[cfg(not(any(target_family = "wasm", target_vendor = "apple")))]
pub mod vulkan;
#[cfg(target_family = "wasm")]
pub mod webgpu;

pub use pipeline_cache::{
    FNV1A_BASIS, fnv1a, fnv1a_from, pipeline_cache_contents, pipeline_cache_file, read_pipeline_cache,
    write_pipeline_cache,
};

use std::fmt;
use std::ops::BitOr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

/// The graphics APIs a device is made on.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Backend {
    Vulkan,
    Metal,
    WebGpu,
}

/// The graphics API this target draws with: Metal on Apple's systems, WebGPU on the web, Vulkan elsewhere.
pub const NATIVE_BACKEND: Backend = if cfg!(target_vendor = "apple") {
    Backend::Metal
} else if cfg!(target_family = "wasm") {
    Backend::WebGpu
} else {
    Backend::Vulkan
};

/// Whether this build of KompozR has a device for `backend` (Vulkan's where it is the target's API, Metal's on Apple's
/// systems, WebGPU's on the web).
pub const fn has_backend(backend: Backend) -> bool {
    match backend {
        Backend::Vulkan => cfg!(not(any(target_family = "wasm", target_vendor = "apple"))),
        Backend::Metal => cfg!(target_vendor = "apple"),
        Backend::WebGpu => cfg!(target_family = "wasm"),
    }
}

impl Backend {
    /// Its name, as messages and the C++ Kompoz's `KOMPOZ_GPU` have it.
    pub fn name(self) -> &'static str {
        match self {
            Backend::Vulkan => "vulkan",
            Backend::Metal => "metal",
            Backend::WebGpu => "webgpu",
        }
    }
}

/// 8 bits a channel, not sRGB-encoded: colors are blended as they are stored, as Android and Skia do (and as Kompoz
/// always has). `R16f`: one 16-bit float channel, drawn into with additive blending (the path renderer's coverage).
/// `Rgba32f`: four 32-bit floats, uploaded and read with nearest filtering (gradients' stops). `Rgba16f`: four 16-bit
/// floats, drawn into and blended as `Rgba8` is (a progressive blur's mask, finer than 8 bits).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Format {
    Rgba8,
    Bgra8,
    R16f,
    Rgba32f,
    Rgba16f,
}

impl Format {
    /// A texel's size in bytes.
    pub const fn bytes_per_texel(self) -> usize {
        match self {
            Format::Rgba32f => 16,
            Format::Rgba16f => 8,
            Format::R16f => 2,
            Format::Rgba8 | Format::Bgra8 => 4,
        }
    }
}

/// What a texture is made for. `MIPMAPPED`: copies halved and halved again down to a texel, made by
/// [`Device::generate_mipmaps`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct TextureUsage(u8);

impl TextureUsage {
    pub const SAMPLED: TextureUsage = TextureUsage(1);
    pub const RENDER_TARGET: TextureUsage = TextureUsage(2);
    pub const MIPMAPPED: TextureUsage = TextureUsage(4);

    /// Whether it has all of `other`'s.
    pub const fn contains(self, other: TextureUsage) -> bool {
        self.0 & other.0 == other.0
    }

    /// Both's (`|`, in constants).
    pub const fn union(self, other: TextureUsage) -> TextureUsage {
        TextureUsage(self.0 | other.0)
    }
}

impl BitOr for TextureUsage {
    type Output = TextureUsage;
    fn bitor(self, other: TextureUsage) -> TextureUsage {
        TextureUsage(self.0 | other.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Filter {
    Nearest,
    #[default]
    Linear,
}

/// Beyond the edges: the edge texel, the texture again, or mirrored. (Transparent past them, which not every API's
/// samplers can do, is the shaders'.)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Wrap {
    #[default]
    Clamp,
    Repeat,
    Mirror,
}

/// How a texture is sampled. `mipmaps`: from a mipmapped texture's copy nearest the size it is drawn at (else its
/// full size).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Sampler {
    pub filter: Filter,
    pub wrap_u: Wrap,
    pub wrap_v: Wrap,
    pub mipmaps: bool,
}

/// An entry point's code for each backend (generated from the Slang shaders): SPIR-V, MSL source, WGSL source.
#[derive(Clone, Copy, Debug)]
pub struct ShaderCode {
    pub spirv: &'static [u8],
    pub msl: &'static str,
    pub wgsl: &'static str,
    pub entry: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum VertexFormat {
    Float,
    Float2,
    Float3,
    Float4,
    UInt4,
}

/// An attribute of each instance: where the vertex shader finds it (`location`) and where it is in the instance.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct VertexAttribute {
    pub location: u32,
    pub format: VertexFormat,
    pub offset: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BlendFactor {
    Zero,
    One,
    SrcAlpha,
    OneMinusSrcAlpha,
    DstAlpha,
    OneMinusDstAlpha,
    SrcColor,
    OneMinusSrcColor,
    DstColor,
    OneMinusDstColor,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BlendOp {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

/// How what a pipeline draws mixes with what is there. [`BlendState::OFF`]: the color written as it is. The
/// default: premultiplied source over.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BlendState {
    pub enabled: bool,
    pub src_color: BlendFactor,
    pub dst_color: BlendFactor,
    pub src_alpha: BlendFactor,
    pub dst_alpha: BlendFactor,
    pub color_op: BlendOp,
    pub alpha_op: BlendOp,
}

impl BlendState {
    /// Premultiplied source over.
    pub const SRC_OVER: BlendState = BlendState {
        enabled: true,
        src_color: BlendFactor::One,
        dst_color: BlendFactor::OneMinusSrcAlpha,
        src_alpha: BlendFactor::One,
        dst_alpha: BlendFactor::OneMinusSrcAlpha,
        color_op: BlendOp::Add,
        alpha_op: BlendOp::Add,
    };
    /// The color written as it is.
    pub const OFF: BlendState = BlendState { enabled: false, ..BlendState::SRC_OVER };

    /// The blend unit on, with these factors and adding.
    pub const fn factors(
        src_color: BlendFactor,
        dst_color: BlendFactor,
        src_alpha: BlendFactor,
        dst_alpha: BlendFactor,
    ) -> BlendState {
        BlendState {
            enabled: true,
            src_color,
            dst_color,
            src_alpha,
            dst_alpha,
            color_op: BlendOp::Add,
            alpha_op: BlendOp::Add,
        }
    }
}

impl Default for BlendState {
    fn default() -> BlendState {
        BlendState::SRC_OVER
    }
}

/// The most bytes of uniforms a pipeline has ([`Device::set_uniforms`]).
pub const MAX_UNIFORMS: usize = 128;
/// The most textures a pipeline samples.
pub const MAX_TEXTURES: usize = 6;

/// A pipeline to make. Every pipeline binds the same way: up to [`MAX_UNIFORMS`] bytes of uniforms both stages see
/// ([`Device::set_uniforms`]), `textures` textures each with its own sampler (bindings 1, 2, … in the shaders), and
/// one buffer of per-instance attributes (no per-vertex ones: the vertex shader makes each instance's vertices from
/// the vertex index).
#[derive(Clone, Debug)]
pub struct PipelineDesc<'a> {
    pub vertex: &'a ShaderCode,
    pub fragment: &'a ShaderCode,
    pub instance_attributes: Vec<VertexAttribute>,
    /// An instance's size in bytes (0: no instance buffer).
    pub instance_stride: u32,
    pub textures: usize,
    /// The slots (bits) whose textures are read texel by texel, with nearest samplers: they may be `Rgba32f`, which
    /// not every GPU filters (WebGPU binds them so).
    pub texel_textures: u8,
    pub blend: BlendState,
    /// The format of the targets it draws into.
    pub target: Format,
    /// The fragment shader's specialization constants 0, 1, … (its `[vk::constant_id]` ones, all ints): what this
    /// pipeline's shader is made for. Those not given keep their defaults.
    pub constants: Vec<i32>,
}

impl<'a> PipelineDesc<'a> {
    /// A pipeline of these shaders drawing into `Rgba8` with source over, without instances, textures or constants.
    pub fn new(vertex: &'a ShaderCode, fragment: &'a ShaderCode) -> PipelineDesc<'a> {
        PipelineDesc {
            vertex,
            fragment,
            instance_attributes: Vec::new(),
            instance_stride: 0,
            textures: 0,
            texel_textures: 0,
            blend: BlendState::SRC_OVER,
            target: Format::Rgba8,
            constants: Vec::new(),
        }
    }
}

/// Memory the CPU wrote this frame and the GPU reads once: where it is, for [`Device::draw`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BufferRef {
    buffer: u64,
    offset: u64,
}

impl BufferRef {
    /// The backend's buffer and the offset in it.
    pub(crate) fn new(buffer: u64, offset: u64) -> BufferRef {
        BufferRef { buffer, offset }
    }

    pub(crate) fn buffer(self) -> u64 {
        self.buffer
    }

    /// Where it starts in its buffer, in bytes.
    pub fn offset(self) -> u64 {
        self.offset
    }
}

/// A rectangle of whole pixels.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct IRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl IRect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> IRect {
        IRect { x, y, width, height }
    }

    pub const fn is_empty(self) -> bool {
        self.width <= 0 || self.height <= 0
    }
}

/// What a pass clears its target to (premultiplied).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct ClearColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ClearColor {
    pub const WHITE: ClearColor = ClearColor::new(1.0, 1.0, 1.0, 1.0);
    pub const TRANSPARENT: ClearColor = ClearColor::new(0.0, 0.0, 0.0, 0.0);

    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> ClearColor {
        ClearColor { r, g, b, a }
    }
}

/// A texture: which of the device's it is, and its size and format (so that what holds the id knows them).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TextureId {
    slot: u32,
    generation: u32,
    width: i32,
    height: i32,
    format: Format,
}

// (what the backends make them with: none is built yet for Apple's systems and the web)
#[cfg_attr(any(target_family = "wasm", target_vendor = "apple"), allow(dead_code))]
impl TextureId {
    pub(crate) fn new(slot: u32, generation: u32, width: i32, height: i32, format: Format) -> TextureId {
        TextureId { slot, generation, width, height, format }
    }

    pub(crate) fn slot(self) -> u32 {
        self.slot
    }

    pub(crate) fn generation(self) -> u32 {
        self.generation
    }

    pub fn width(self) -> i32 {
        self.width
    }

    pub fn height(self) -> i32 {
        self.height
    }

    pub fn format(self) -> Format {
        self.format
    }

    /// All of it.
    pub fn bounds(self) -> IRect {
        IRect::new(0, 0, self.width, self.height)
    }
}

/// A pipeline of the device's.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PipelineId(u64);

#[cfg_attr(any(target_family = "wasm", target_vendor = "apple"), allow(dead_code))]
impl PipelineId {
    pub(crate) fn new(raw: u64) -> PipelineId {
        PipelineId(raw)
    }

    pub(crate) fn raw(self) -> u64 {
        self.0
    }
}

/// What was let go of, for the device to destroy once the GPU is done with it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Retired {
    Texture(TextureId),
    Pipeline(PipelineId),
}

/// Where dropped textures and pipelines go (from any thread: pipelines are made on others too).
#[derive(Default)]
pub(crate) struct RetiredQueue(Mutex<Vec<Retired>>);

#[cfg_attr(any(target_family = "wasm", target_vendor = "apple"), allow(dead_code))]
impl RetiredQueue {
    fn push(&self, retired: Retired) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push(retired);
    }

    /// What was let go of since the last call.
    pub(crate) fn take(&self) -> Vec<Retired> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

/// A texture the device made: dropped, it is destroyed once the GPU is done with the frames that may use it.
pub struct Texture {
    id: TextureId,
    retired: Arc<RetiredQueue>,
}

#[cfg_attr(any(target_family = "wasm", target_vendor = "apple"), allow(dead_code))]
impl Texture {
    pub(crate) fn new(id: TextureId, retired: Arc<RetiredQueue>) -> Texture {
        Texture { id, retired }
    }

    pub fn id(&self) -> TextureId {
        self.id
    }

    pub fn width(&self) -> i32 {
        self.id.width
    }

    pub fn height(&self) -> i32 {
        self.id.height
    }

    pub fn format(&self) -> Format {
        self.id.format
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        self.retired.push(Retired::Texture(self.id));
    }
}

impl fmt::Debug for Texture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Texture").field(&self.id).finish()
    }
}

/// A pipeline the device made: dropped, it is destroyed once the GPU is done with the frames that may use it.
pub struct Pipeline {
    id: PipelineId,
    retired: Arc<RetiredQueue>,
}

#[cfg_attr(any(target_family = "wasm", target_vendor = "apple"), allow(dead_code))]
impl Pipeline {
    pub(crate) fn new(id: PipelineId, retired: Arc<RetiredQueue>) -> Pipeline {
        Pipeline { id, retired }
    }

    pub fn id(&self) -> PipelineId {
        self.id
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        self.retired.push(Retired::Pipeline(self.id));
    }
}

impl fmt::Debug for Pipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Pipeline").field(&self.id).finish()
    }
}

/// Makes pipelines on any thread while the device goes on with its frames (the renderer warms its pipelines on
/// another); what a device gives where its API can ([`Device::pipeline_factory`]).
pub trait PipelineFactory: Send + Sync {
    /// The pipeline, or none if the driver couldn't make it (or the device is gone).
    fn create_pipeline(&self, desc: &PipelineDesc<'_>) -> Option<Pipeline>;
}

/// What the window system gives a backend to present to (filled by the platform layer); the default draws only
/// into textures. Each target has its backend's field: `vulkan` where Vulkan is the API, `metal` on Apple's
/// systems, `webgpu` on the web.
#[derive(Default)]
pub struct Surface {
    /// Vulkan: what presenting needs; none to draw only into textures.
    #[cfg(not(any(target_family = "wasm", target_vendor = "apple")))]
    pub vulkan: Option<vulkan::VulkanSurface>,
    /// Metal: the layer presented to; none to draw only into textures.
    #[cfg(target_vendor = "apple")]
    pub metal: Option<MetalSurface>,
    /// WebGPU: the device the page acquired and the canvas presented to (WebGPU's device is asked for
    /// asynchronously, before the app starts: a WebGPU device can't be made without it).
    #[cfg(target_family = "wasm")]
    pub webgpu: Option<WebGpuSurface>,
    /// The drawable's size in pixels.
    pub drawable_size: Option<Box<dyn Fn() -> (i32, i32)>>,
    /// A file where the pipelines' compiled shaders are kept between runs, read when the device is made and written
    /// when it has made more (Vulkan's pipeline cache); none to compile them every run.
    pub pipeline_cache: Option<PathBuf>,
}

/// What presenting with Metal needs from the window system: the view's `CAMetalLayer`, which the device configures
/// (its device, BGRA8, not only a framebuffer: the renderer copies from its drawables) and takes its drawables from
/// each frame.
#[cfg(target_vendor = "apple")]
pub struct MetalSurface {
    /// The `CAMetalLayer *` (an Objective-C object); the platform keeps a reference to it for as long as the device
    /// lives (its view's).
    pub layer: std::ptr::NonNull<std::ffi::c_void>,
}

/// What WebGPU needs from the page, as handles of the web platform's JavaScript glue (its table of the page's
/// objects): the `GPUDevice` the page asked for before the app started, and the canvas presented to.
#[cfg(target_family = "wasm")]
pub struct WebGpuSurface {
    /// The `GPUDevice`'s handle (its adapter's features and limits are the device's).
    pub device: u32,
    /// The `<canvas>`'s handle (its `webgpu` context configured by the device); none to draw only into textures.
    pub canvas: Option<u32>,
}

/// A pass's time in a [`GpuFrameTime`]: what it drew into ([`pass_label`]) and how long it took.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct GpuPassTime {
    pub label: String,
    pub ms: f64,
}

/// The GPU's time for a frame ([`Device::set_gpu_timing`]): what it was busy with, its passes' times together (not
/// the waits between its submits), and each pass's, named by what it draws into ("window", "1080x2400",
/// "2048x2048 R16F"), or "copy WxH", "mipmaps WxH".
#[derive(Clone, PartialEq, Debug, Default)]
pub struct GpuFrameTime {
    pub ms: f64,
    pub passes: Vec<GpuPassTime>,
}

/// A pass's name in a [`GpuFrameTime`].
pub fn pass_label(width: i32, height: i32, format: Format, window: bool) -> String {
    if window {
        return "window".to_string();
    }
    let suffix = match format {
        Format::Rgba8 => "",
        Format::Bgra8 => " BGRA8",
        Format::R16f => " R16F",
        Format::Rgba32f => " RGBA32F",
        Format::Rgba16f => " RGBA16F",
    };
    format!("{width}x{height}{suffix}")
}

static LOGGER: RwLock<Option<fn(&str)>> = RwLock::new(None);

/// Where the hardware layer's messages go (stderr until the platform says).
pub fn set_logger(logger: Option<fn(&str)>) {
    *LOGGER.write().unwrap_or_else(|e| e.into_inner()) = logger;
}

/// A message of the hardware layer's.
pub fn log(message: &str) {
    match *LOGGER.read().unwrap_or_else(|e| e.into_inner()) {
        Some(logger) => logger(message),
        None => eprintln!("{message}"),
    }
}

/// Whether the environment variable is set to 1.
pub(crate) fn env_is_one(name: &str) -> bool {
    kompoz_runtime::environment::flag(name)
}

/// `KOMPOZ_GPU_REQUIRED=1`: no device is a failure (CI), not a reason to skip.
pub fn gpu_required() -> bool {
    env_is_one("KOMPOZ_GPU_REQUIRED")
}

/// Which GPU the devices made from here on draw on where there are two (C++'s Application `gpuPriority`, b96a026;
/// Skiko's `skiko.gpu.priority`): the one the screen is on where Linux tells (its firmware's VGA device), else an
/// integrated one ([`Auto`](GpuPriority::Auto)); an integrated one ([`Integrated`](GpuPriority::Integrated)); a
/// discrete one, else an integrated one ([`Discrete`](GpuPriority::Discrete)). `KOMPOZ_GPU_DEVICE` names one instead.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum GpuPriority {
    #[default]
    Auto,
    Integrated,
    Discrete,
}

static GPU_PRIORITY: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// The GPU the devices made from here on draw on (the app's, set as it starts).
pub fn set_gpu_priority(priority: GpuPriority) {
    GPU_PRIORITY.store(priority as u8, std::sync::atomic::Ordering::Relaxed);
}

/// The GPU the devices made from here on draw on.
pub fn gpu_priority() -> GpuPriority {
    match GPU_PRIORITY.load(std::sync::atomic::Ordering::Relaxed) {
        1 => GpuPriority::Integrated,
        2 => GpuPriority::Discrete,
        _ => GpuPriority::Auto,
    }
}

/// The hardware layer: one per window (or none, drawing only into textures), used from the thread that made it.
pub trait Device {
    fn backend(&self) -> Backend;
    fn device_name(&self) -> String;
    /// Errors the API's validation reported so far (with `KOMPOZ_GPU_VALIDATION=1`; tests want none).
    fn validation_errors(&self) -> u32 {
        0
    }
    /// In clip space y goes up (Metal, WebGPU) or down (Vulkan): the renderer flips its projection for the second.
    fn clip_y_down(&self) -> bool;

    /// A texture (its texels undefined until uploaded or drawn); none if there is no memory for it.
    fn create_texture(&mut self, width: i32, height: i32, format: Format, usage: TextureUsage) -> Option<Texture>;
    /// A pipeline; none if the driver couldn't make it.
    fn create_pipeline(&mut self, desc: &PipelineDesc<'_>) -> Option<Pipeline>;
    /// What makes pipelines on another thread while this one goes on, where the API can (Vulkan).
    fn pipeline_factory(&self) -> Option<Arc<dyn PipelineFactory>> {
        None
    }
    /// The format the swapchain's images have (pipelines drawing into them are made for it).
    fn swapchain_format(&self) -> Format;

    /// Starts a frame (waiting for the GPU to be done with the one before the last). Calls below need a frame begun.
    fn begin_frame(&mut self);
    /// The image presented at the end of the frame, at the drawable's size (turned: see
    /// [`swapchain_turns`](Device::swapchain_turns)); none if there is nothing to present to now (minimized, the
    /// surface lost, or no window).
    fn swapchain_texture(&mut self) -> Option<TextureId>;
    /// How many quarter turns (clockwise) the window's content is drawn turned by into the swapchain's images, which
    /// are then the display's own way up (as wide as the drawable is high for an odd number): where the system
    /// leaves turning to the app, so that the compositor doesn't turn every frame (Android's Vulkan, the screen
    /// turned from its natural way up).
    fn swapchain_turns(&self) -> i32 {
        0
    }
    /// How many frames ago the swapchain image [`swapchain_texture`](Device::swapchain_texture) gave was last
    /// presented, holding what was drawn into it then (1: the frame before; drawn over where things have changed
    /// since, as Android's renderer does with a buffer's age); 0 if what it holds isn't known (a new one, or a
    /// platform that doesn't keep it): all of it drawn over.
    fn swapchain_age(&self) -> i32 {
        0
    }
    /// The window's native window went away or is another (Vulkan's `VulkanSurface::native_window` tells which),
    /// between frames: what was made for the old one is let go of now, as the platform must before the system takes
    /// it (Android's surfaceDestroyed), not at the next frame.
    fn surface_changed(&mut self) {}
    /// `data` copied into this frame's memory (at a multiple of `alignment`, 16 at least), for [`draw`](Device::draw);
    /// none if there is no memory for it.
    fn write_frame_data(&mut self, data: &[u8], alignment: usize) -> Option<BufferRef>;
    /// Texels (in the texture's format, RGBA8 for `Bgra8` as [`read_pixels`](Device::read_pixels) gives them back;
    /// `row_bytes` apart) into part of a texture; outside passes.
    fn upload(&mut self, texture: TextureId, region: IRect, pixels: &[u8], row_bytes: usize);
    /// Draws into `target` (cleared first if `clear`), sampling `reads` (each read before by what was drawn into
    /// it). A pass starts with nothing bound: its pipeline, uniforms and textures are set in it (as Metal's and
    /// WebGPU's encoders have them). With `area`, it draws only there (its scissor stays within it): the rest of the
    /// target is kept as it was, and a clear may leave it so; a tiling GPU then loads and stores only that part
    /// (Vulkan's render area).
    fn begin_pass(&mut self, target: TextureId, clear: Option<ClearColor>, reads: &[TextureId], area: Option<IRect>);
    fn set_pipeline(&mut self, pipeline: PipelineId);
    fn set_scissor(&mut self, rect: IRect);
    /// Up to [`MAX_UNIFORMS`] bytes (a multiple of 4) both shader stages see.
    fn set_uniforms(&mut self, data: &[u8]);
    fn set_texture(&mut self, slot: usize, texture: TextureId, sampler: Sampler);
    /// `instance_count` instances of `vertex_count` vertices (triangles), their attributes from `instances`.
    fn draw(&mut self, instances: Option<BufferRef>, vertex_count: u32, instance_count: u32);
    fn end_pass(&mut self);
    /// A mipmapped texture's smaller copies, each from the one before (filtered), after its pixels are uploaded;
    /// outside passes.
    fn generate_mipmaps(&mut self, texture: TextureId);
    /// What `texture` holds no longer matters: its next pass that doesn't clear needn't load it, a tiling GPU's
    /// cheapest start (Vulkan's and Metal's "don't care"; WebGPU, which has none, clears). For a pass that draws over
    /// all of it; outside passes.
    fn discard(&mut self, texture: TextureId) {
        let _ = texture;
    }
    /// Part of one texture into another of the same format, at (x, y); outside passes.
    fn copy_texture(&mut self, source: TextureId, region: IRect, destination: TextureId, x: i32, y: i32);
    /// A texture's pixels (RGBA8 for `Rgba8` and `Bgra8`, else its own format's texels: half floats for `Rgba16f`;
    /// the top row first, rows packed), waiting for what was drawn into it so far; outside passes. Only where the
    /// API can wait for the GPU ([`reads_pixels_at_once`](Device::reads_pixels_at_once): Vulkan, Metal); a backend
    /// that can't (WebGPU, whose buffers map asynchronously) has none to give here (empty): it reads them with
    /// [`read_pixels_async`](Device::read_pixels_async).
    fn read_pixels(&mut self, texture: TextureId, region: IRect) -> Vec<u8>;
    /// Whether [`read_pixels`](Device::read_pixels) gives the pixels (the API can wait for the GPU): all but
    /// WebGPU's.
    fn reads_pixels_at_once(&self) -> bool {
        true
    }
    /// A texture's pixels as [`read_pixels`](Device::read_pixels) gives them, handed to `done` once the GPU has them
    /// (what was drawn into it so far); outside passes, on any backend. `done` runs on this thread: at once where
    /// the API can wait (the default: [`read_pixels`](Device::read_pixels)), else later, from the platform's loop,
    /// when the copy's buffer is mapped (WebGPU's `mapAsync`), after the frame's work is submitted; empty if the
    /// device went away meanwhile.
    fn read_pixels_async(&mut self, texture: TextureId, region: IRect, done: Box<dyn FnOnce(Vec<u8>)>) {
        done(self.read_pixels(texture, region));
    }
    /// Submits the frame, and shows the swapchain image if `present` (and it was asked for).
    fn end_frame(&mut self, present: bool);
    /// Times the GPU's work from the next frame on (or no longer), where the API can (Vulkan's timestamps); false if
    /// it can't.
    fn set_gpu_timing(&mut self, on: bool) -> bool {
        let _ = on;
        false
    }
    /// The frames timed (that drew something) whose GPU work is done since the last call, oldest first: a frame or
    /// two after their [`end_frame`](Device::end_frame).
    fn take_gpu_times(&mut self) -> Vec<GpuFrameTime> {
        Vec::new()
    }
    /// Waits for the GPU to finish everything (before resources it uses go).
    fn wait_idle(&mut self);
}

/// A device for `backend` presenting to `surface`; why not if this system can't have one, or this build has no such
/// backend ([`has_backend`]: each target has its own API's only).
pub fn create(backend: Backend, surface: Surface) -> Result<Box<dyn Device>, String> {
    #[cfg(not(any(target_family = "wasm", target_vendor = "apple")))]
    if backend == Backend::Vulkan {
        return Ok(Box::new(vulkan::VulkanDevice::new(surface)?));
    }
    #[cfg(target_vendor = "apple")]
    if backend == Backend::Metal {
        return Ok(Box::new(metal::MetalDevice::new(surface)?));
    }
    #[cfg(target_family = "wasm")]
    if backend == Backend::WebGpu {
        return Ok(Box::new(webgpu::WebGpuDevice::new(surface)?));
    }
    drop(surface);
    Err(format!("this build of KompozR has no {} backend", backend.name()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_usages_are_as_cpp() {
        let sizes =
            [Format::Rgba8, Format::Bgra8, Format::R16f, Format::Rgba32f, Format::Rgba16f].map(Format::bytes_per_texel);
        assert_eq!(sizes, [4, 4, 2, 16, 8]);
        let usage = TextureUsage::SAMPLED | TextureUsage::MIPMAPPED;
        assert!(usage.contains(TextureUsage::SAMPLED) && usage.contains(TextureUsage::MIPMAPPED));
        assert!(!usage.contains(TextureUsage::RENDER_TARGET));
        assert_eq!(usage, TextureUsage::MIPMAPPED.union(TextureUsage::SAMPLED));
        assert_eq!(BlendState::default(), BlendState::SRC_OVER);
        assert_eq!(BlendState::OFF, BlendState { enabled: false, ..BlendState::default() });
        assert!(
            IRect::new(1, 2, 0, 5).is_empty()
                && IRect::new(1, 2, 3, -1).is_empty()
                && !IRect::new(0, 0, 1, 1).is_empty()
        );
    }

    #[test]
    fn each_target_has_its_own_backend() {
        let native = NATIVE_BACKEND;
        assert_eq!(native, if cfg!(target_os = "linux") { Backend::Vulkan } else { native });
        assert_eq!(has_backend(Backend::Metal), cfg!(target_vendor = "apple"));
        assert_eq!(has_backend(Backend::WebGpu), cfg!(target_family = "wasm"));
        for missing in [Backend::Vulkan, Backend::Metal, Backend::WebGpu].into_iter().filter(|&b| !has_backend(b)) {
            let why = create(missing, Surface::default()).err().unwrap();
            assert_eq!(why, format!("this build of KompozR has no {} backend", missing.name()));
        }
    }

    #[test]
    fn passes_are_named_by_their_targets() {
        assert_eq!(pass_label(1080, 2400, Format::Rgba8, false), "1080x2400");
        assert_eq!(pass_label(2048, 2048, Format::R16f, false), "2048x2048 R16F");
        assert_eq!(pass_label(4, 4, Format::Bgra8, false), "4x4 BGRA8");
        assert_eq!(pass_label(4, 4, Format::Rgba32f, false), "4x4 RGBA32F");
        assert_eq!(pass_label(4, 4, Format::Rgba16f, false), "4x4 RGBA16F");
        assert_eq!(pass_label(4, 4, Format::Bgra8, true), "window");
    }

    #[test]
    fn handles_let_go_of_go_back_to_their_device() {
        let retired = Arc::new(RetiredQueue::default());
        let id = TextureId::new(3, 7, 640, 480, Format::Rgba16f);
        let texture = Texture::new(id, retired.clone());
        assert_eq!((texture.width(), texture.height(), texture.format()), (640, 480, Format::Rgba16f));
        assert_eq!(texture.id().bounds(), IRect::new(0, 0, 640, 480));
        let pipeline = Pipeline::new(PipelineId::new(0xdead_beef), retired.clone());
        assert!(retired.take().is_empty());
        drop(texture);
        // (Pipelines go from any thread: they are made on others.)
        std::thread::spawn(move || drop(pipeline)).join().unwrap();
        assert_eq!(retired.take(), [Retired::Texture(id), Retired::Pipeline(PipelineId::new(0xdead_beef))]);
        assert!(retired.take().is_empty());
    }
}
