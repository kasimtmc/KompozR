//! What the GPU tests draw with: a device made without a window (drawing only into textures), shared by the tests
//! and the renderers drawing on it; the device tests' pipelines (tests/gpu/shaders: quads of a color, or of a
//! texture); and `render`, which draws a scene with the renderer into a new target and reads it back, as the C++
//! Kompoz's `Gpu` does (`render_quads` the same with the test pipelines).

use kompoz_gpu::Renderer;
use kompoz_gpu::device::{
    self, Backend, BlendState, BufferRef, ClearColor, Device, Filter, Format, GpuFrameTime, IRect, Pipeline,
    PipelineDesc, PipelineFactory, PipelineId, Sampler, ShaderCode, Surface, Texture, TextureId, TextureUsage,
    VertexAttribute, VertexFormat,
};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

/// A picture read back: RGBA, premultiplied, the top row first.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Pixels {
    pub width: i32,
    pub height: i32,
    pub rgba: Vec<u8>,
}

impl Pixels {
    pub fn at(&self, x: i32, y: i32) -> [u8; 4] {
        let i = (y as usize * self.width as usize + x as usize) * 4;
        self.rgba[i..i + 4].try_into().unwrap()
    }
}

/// The largest difference of two pixels' channels (the golden pictures').
pub use kompoz_gpu::golden::max_difference;

/// Within `tolerance` of the opaque color (0..255 each).
pub fn near(p: [u8; 4], r: u8, g: u8, b: u8, tolerance: i32) -> bool {
    max_difference(p, [r, g, b, 255]) <= tolerance
}

/// (MSL's entry points can't be called `main`, which is C++'s: the Metal shaders' are named for what they are.)
const MSL: bool = cfg!(target_vendor = "apple");

pub static QUAD_VERTEX: ShaderCode = ShaderCode {
    spirv: include_bytes!("shaders/quad.vert.spv"),
    msl: include_str!("shaders/quad.vert.metal"),
    wgsl: "",
    entry: if MSL { "quad_vertex" } else { "main" },
};
pub static COLOR_FRAGMENT: ShaderCode = ShaderCode {
    spirv: include_bytes!("shaders/color.frag.spv"),
    msl: include_str!("shaders/color.frag.metal"),
    wgsl: "",
    entry: if MSL { "color_fragment" } else { "main" },
};
pub static TEXTURE_FRAGMENT: ShaderCode = ShaderCode {
    spirv: include_bytes!("shaders/texture.frag.spv"),
    msl: include_str!("shaders/texture.frag.metal"),
    wgsl: "",
    entry: if MSL { "texture_fragment" } else { "main" },
};

/// A quad's instance: its rectangle (left, top, right, bottom in px) and its color (premultiplied).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Quad {
    pub rect: [f32; 4],
    pub color: [f32; 4],
}

impl Quad {
    /// At (x, y), w x h, of an opaque color.
    pub fn new(x: f32, y: f32, w: f32, h: f32, r: f32, g: f32, b: f32) -> Quad {
        Quad { rect: [x, y, x + w, y + h], color: [r, g, b, 1.0] }
    }

    /// The same at alpha `a` (premultiplied).
    pub fn alpha(self, a: f32) -> Quad {
        let [r, g, b, alpha] = self.color;
        Quad { color: [r * a, g * a, b * a, alpha * a], ..self }
    }
}

/// A pipeline of the test shaders: quads of a color (fragment `color`) or of a texture (`texture`).
pub fn quad_pipeline(
    device: &mut dyn Device,
    fragment: &ShaderCode,
    blend: BlendState,
    constants: Vec<i32>,
) -> Option<Pipeline> {
    let mut desc = quad_desc(fragment);
    desc.blend = blend;
    desc.constants = constants;
    device.create_pipeline(&desc)
}

/// The test shaders' pipeline description, drawing into RGBA8 with source over.
pub fn quad_desc(fragment: &ShaderCode) -> PipelineDesc<'_> {
    let mut desc = PipelineDesc::new(&QUAD_VERTEX, fragment);
    desc.instance_attributes = vec![
        VertexAttribute { location: 0, format: VertexFormat::Float4, offset: 0 },
        VertexAttribute { location: 1, format: VertexFormat::Float4, offset: 16 },
    ];
    desc.instance_stride = 32;
    desc.textures = usize::from(std::ptr::eq(fragment, &TEXTURE_FRAGMENT));
    desc
}

/// The device tests' pipelines.
pub struct Pipelines {
    /// Quads of their colors, over what is there.
    pub color: Pipeline,
    /// The same, red and blue swapped (a specialization constant).
    pub swapped: Pipeline,
    /// Quads of a texture, written as they are (blending off).
    pub copy: Pipeline,
}

impl Pipelines {
    fn new(device: &mut dyn Device) -> Option<Pipelines> {
        Some(Pipelines {
            color: quad_pipeline(device, &COLOR_FRAGMENT, BlendState::SRC_OVER, Vec::new())?,
            swapped: quad_pipeline(device, &COLOR_FRAGMENT, BlendState::SRC_OVER, vec![1])?,
            copy: quad_pipeline(device, &TEXTURE_FRAGMENT, BlendState::OFF, Vec::new())?,
        })
    }
}

/// `quads` drawn with `pipeline` in the pass begun on `target`, the texture coordinates going over `source` (left,
/// top, right, bottom) across each.
pub fn draw_quads(device: &mut dyn Device, pipeline: PipelineId, target: TextureId, quads: &[Quad], source: [f32; 4]) {
    let (w, h) = (target.width() as f32, target.height() as f32);
    // (Into clip space as the renderer's: flipped where y goes down in it.)
    let to_clip = kompoz_gpu::shaders::to_clip(w, h, device.clip_y_down());
    let uniforms: Vec<u8> = to_clip.iter().chain(&source).flat_map(|v| v.to_ne_bytes()).collect();
    let instances: Vec<u8> =
        quads.iter().flat_map(|q| q.rect.iter().chain(&q.color)).flat_map(|v| v.to_ne_bytes()).collect();
    device.set_pipeline(pipeline);
    device.set_uniforms(&uniforms);
    let buffer = device.write_frame_data(&instances, 16);
    device.draw(buffer, 6, quads.len() as u32);
}

/// `quads` drawn over `target` in a pass of their own (loading what it holds).
pub fn fill(device: &mut dyn Device, pipeline: PipelineId, target: TextureId, quads: &[Quad]) {
    device.begin_pass(target, None, &[], None);
    draw_quads(device, pipeline, target, quads, [0.0; 4]);
    device.end_pass();
}

/// `source` drawn over all of `target` as it is (nearest texels, scaled to fit), in the pass begun on it.
pub fn blit(device: &mut dyn Device, copy: PipelineId, source: TextureId, target: TextureId) {
    let sampler = Sampler { filter: Filter::Nearest, ..Sampler::default() };
    device.set_texture(0, source, sampler);
    let (w, h) = (target.width() as f32, target.height() as f32);
    draw_quads(device, copy, target, &[Quad::new(0.0, 0.0, w, h, 1.0, 1.0, 1.0)], [0.0, 0.0, 1.0, 1.0]);
}

/// What a scene draws with: the device, the test pipelines, the target (cleared) and the scale (px per dp).
pub struct Frame<'a> {
    pub device: &'a mut dyn Device,
    pub pipelines: &'a Pipelines,
    pub target: TextureId,
    pub scale: f32,
}

impl Frame<'_> {
    /// dp in px.
    pub fn dp(&self, v: f32) -> f32 {
        v * self.scale
    }

    /// `quads` over the target with the color pipeline.
    pub fn fill(&mut self, quads: &[Quad]) {
        fill(self.device, self.pipelines.color.id(), self.target, quads);
    }
}

/// The calls of a [`Device`] that pass straight to the shared one in `self.<field>`, all but `create_texture` and
/// `create_pipeline`.
macro_rules! passed_on {
    ($($field:tt).+) => {
        fn backend(&self) -> Backend {
            self.$($field).+.borrow().backend()
        }
        fn device_name(&self) -> String {
            self.$($field).+.borrow().device_name()
        }
        fn validation_errors(&self) -> u32 {
            self.$($field).+.borrow().validation_errors()
        }
        fn clip_y_down(&self) -> bool {
            self.$($field).+.borrow().clip_y_down()
        }
        fn pipeline_factory(&self) -> Option<Arc<dyn PipelineFactory>> {
            self.$($field).+.borrow().pipeline_factory()
        }
        fn swapchain_format(&self) -> Format {
            self.$($field).+.borrow().swapchain_format()
        }
        fn begin_frame(&mut self) {
            self.$($field).+.borrow_mut().begin_frame()
        }
        fn swapchain_texture(&mut self) -> Option<TextureId> {
            self.$($field).+.borrow_mut().swapchain_texture()
        }
        fn swapchain_turns(&self) -> i32 {
            self.$($field).+.borrow().swapchain_turns()
        }
        fn swapchain_age(&self) -> i32 {
            self.$($field).+.borrow().swapchain_age()
        }
        fn surface_changed(&mut self) {
            self.$($field).+.borrow_mut().surface_changed()
        }
        fn write_frame_data(&mut self, data: &[u8], alignment: usize) -> Option<BufferRef> {
            self.$($field).+.borrow_mut().write_frame_data(data, alignment)
        }
        fn upload(&mut self, texture: TextureId, region: IRect, pixels: &[u8], row_bytes: usize) {
            self.$($field).+.borrow_mut().upload(texture, region, pixels, row_bytes)
        }
        fn begin_pass(&mut self, target: TextureId, clear: Option<ClearColor>, reads: &[TextureId], area: Option<IRect>) {
            self.$($field).+.borrow_mut().begin_pass(target, clear, reads, area)
        }
        fn set_pipeline(&mut self, pipeline: PipelineId) {
            self.$($field).+.borrow_mut().set_pipeline(pipeline)
        }
        fn set_scissor(&mut self, rect: IRect) {
            self.$($field).+.borrow_mut().set_scissor(rect)
        }
        fn set_uniforms(&mut self, data: &[u8]) {
            self.$($field).+.borrow_mut().set_uniforms(data)
        }
        fn set_texture(&mut self, slot: usize, texture: TextureId, sampler: Sampler) {
            self.$($field).+.borrow_mut().set_texture(slot, texture, sampler)
        }
        fn draw(&mut self, instances: Option<BufferRef>, vertex_count: u32, instance_count: u32) {
            self.$($field).+.borrow_mut().draw(instances, vertex_count, instance_count)
        }
        fn end_pass(&mut self) {
            self.$($field).+.borrow_mut().end_pass()
        }
        fn generate_mipmaps(&mut self, texture: TextureId) {
            self.$($field).+.borrow_mut().generate_mipmaps(texture)
        }
        fn discard(&mut self, texture: TextureId) {
            self.$($field).+.borrow_mut().discard(texture)
        }
        fn copy_texture(&mut self, source: TextureId, region: IRect, destination: TextureId, x: i32, y: i32) {
            self.$($field).+.borrow_mut().copy_texture(source, region, destination, x, y)
        }
        fn read_pixels(&mut self, texture: TextureId, region: IRect) -> Vec<u8> {
            self.$($field).+.borrow_mut().read_pixels(texture, region)
        }
        fn reads_pixels_at_once(&self) -> bool {
            self.$($field).+.borrow().reads_pixels_at_once()
        }
        fn read_pixels_async(&mut self, texture: TextureId, region: IRect, done: Box<dyn FnOnce(Vec<u8>)>) {
            self.$($field).+.borrow_mut().read_pixels_async(texture, region, done)
        }
        fn end_frame(&mut self, present: bool) {
            self.$($field).+.borrow_mut().end_frame(present)
        }
        fn set_gpu_timing(&mut self, on: bool) -> bool {
            self.$($field).+.borrow_mut().set_gpu_timing(on)
        }
        fn take_gpu_times(&mut self) -> Vec<GpuFrameTime> {
            self.$($field).+.borrow_mut().take_gpu_times()
        }
        fn wait_idle(&mut self) {
            self.$($field).+.borrow_mut().wait_idle()
        }
    };
}

/// The one device, shared by the tests and the renderers drawing on it (as C++'s tests hand a renderer a `Device&`).
#[derive(Clone)]
pub struct SharedDevice(Rc<RefCell<Box<dyn Device>>>);

impl Device for SharedDevice {
    passed_on!(0);

    fn create_texture(&mut self, width: i32, height: i32, format: Format, usage: TextureUsage) -> Option<Texture> {
        self.0.borrow_mut().create_texture(width, height, format, usage)
    }
    fn create_pipeline(&mut self, desc: &PipelineDesc<'_>) -> Option<Pipeline> {
        self.0.borrow_mut().create_pipeline(desc)
    }
}

/// A device passing all to the shared one, watched: what it refuses (a variant of the shader, as an Adreno 6xx's
/// Vulkan compiler couldn't make one) and the coverage masks (R16F textures) it made (C++'s WatchedDevice).
pub struct WatchedDevice {
    device: SharedDevice,
    refuse_variants: bool,
    pub refused: Rc<Cell<i32>>,
    pub masks: Rc<Cell<i32>>,
}

impl WatchedDevice {
    pub fn new(device: &SharedDevice, refuse_variants: bool) -> WatchedDevice {
        WatchedDevice { device: device.clone(), refuse_variants, refused: Rc::default(), masks: Rc::default() }
    }
}

impl Device for WatchedDevice {
    passed_on!(device.0);

    fn create_texture(&mut self, width: i32, height: i32, format: Format, usage: TextureUsage) -> Option<Texture> {
        if format == Format::R16f {
            self.masks.set(self.masks.get() + 1);
        }
        self.device.create_texture(width, height, format, usage)
    }
    fn create_pipeline(&mut self, desc: &PipelineDesc<'_>) -> Option<Pipeline> {
        if self.refuse_variants && !desc.constants.is_empty() {
            self.refused.set(self.refused.get() + 1);
            return None;
        }
        self.device.create_pipeline(desc)
    }
}

/// `draw` with `renderer` into a new w x h target, cleared to `clear`, in a frame of its own; read back
/// (premultiplied). C++'s renderBy.
pub fn render_by(
    renderer: &mut Renderer,
    w: i32,
    h: i32,
    draw: impl FnOnce(&mut Renderer),
    clear: ClearColor,
) -> Pixels {
    renderer.device_mut().begin_frame();
    let target = renderer
        .device_mut()
        .create_texture(w, h, Format::Rgba8, TextureUsage::SAMPLED | TextureUsage::RENDER_TARGET)
        .expect("a render target");
    renderer.begin_target(target.id(), Some(clear));
    draw(renderer);
    renderer.end_target();
    renderer.flush();
    let rgba = renderer.device_mut().read_pixels(target.id(), IRect::new(0, 0, w, h));
    renderer.device_mut().end_frame(false);
    renderer.end_frame();
    Pixels { width: w, height: h, rgba }
}

/// A device (one per process: the tests run in sequence on it), a renderer on it, and what the device tests draw
/// with.
pub struct Gpu {
    pub pipelines: Pipelines,
    pub device: SharedDevice,
    pub renderer: Renderer,
}

impl Gpu {
    /// A device drawing only into textures (lavapipe in CI), keeping its pipelines in `pipeline_cache` if given; why
    /// not if this system has none.
    pub fn start(pipeline_cache: Option<PathBuf>) -> Result<Gpu, String> {
        let surface = Surface { drawable_size: Some(Box::new(|| (64, 64))), pipeline_cache, ..Surface::default() };
        let mut device = SharedDevice(Rc::new(RefCell::new(device::create(device::NATIVE_BACKEND, surface)?)));
        let pipelines = Pipelines::new(&mut device).ok_or("the test pipelines couldn't be made")?;
        let renderer = Renderer::new(Box::new(device.clone())).ok_or("the renderer couldn't be made")?;
        Ok(Gpu { pipelines, device, renderer })
    }

    /// `draw` with the renderer on a w x h px target (cleared white), in a frame of its own, read back
    /// (premultiplied): C++'s `Gpu::render` (its px per dp the scene's own: `dp`).
    pub fn render(&mut self, w: i32, h: i32, draw: impl FnOnce(&mut Renderer)) -> Pixels {
        render_by(&mut self.renderer, w, h, draw, ClearColor::WHITE)
    }

    /// The same on a target cleared to `clear`.
    pub fn render_cleared(&mut self, w: i32, h: i32, clear: ClearColor, draw: impl FnOnce(&mut Renderer)) -> Pixels {
        render_by(&mut self.renderer, w, h, draw, clear)
    }

    /// Whether the device keeps its pipelines in the file it is given: where the driver's pipeline cache holds them
    /// (lavapipe's and SwiftShader's hold nothing).
    pub fn keeps_pipelines(&self) -> bool {
        let name = self.device.device_name();
        !name.contains("llvmpipe") && !name.contains("SwiftShader")
    }

    /// `draw` with the test pipelines on a w x h px target cleared to `clear` at `scale` px per dp, in a frame of its
    /// own, read back (premultiplied).
    pub fn render_quads(
        &mut self,
        w: i32,
        h: i32,
        scale: f32,
        draw: impl FnOnce(&mut Frame<'_>),
        clear: ClearColor,
    ) -> Pixels {
        let device: &mut dyn Device = &mut self.device;
        device.begin_frame();
        let target = device
            .create_texture(w, h, Format::Rgba8, TextureUsage::SAMPLED | TextureUsage::RENDER_TARGET)
            .expect("a render target");
        device.begin_pass(target.id(), Some(clear), &[], None);
        device.end_pass();
        draw(&mut Frame { device, pipelines: &self.pipelines, target: target.id(), scale });
        let rgba = device.read_pixels(target.id(), IRect::new(0, 0, w, h));
        device.end_frame(false);
        Pixels { width: w, height: h, rgba }
    }
}
