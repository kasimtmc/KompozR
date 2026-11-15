//! What the GPU tests draw with: a device made without a window (drawing only into textures), the device tests'
//! pipelines (tests/gpu/shaders: quads of a color, or of a texture), and `render`, which draws a scene into a new
//! target and reads it back, as the C++ Kompoz's `Gpu` does.

use kompoz_gpu::device::{
    self, Backend, BlendState, ClearColor, Device, Filter, Format, IRect, Pipeline, PipelineDesc, PipelineId, Sampler,
    ShaderCode, Surface, TextureId, TextureUsage, VertexAttribute, VertexFormat,
};
use std::path::PathBuf;

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

/// The largest difference of two pixels' channels.
pub fn max_difference(a: [u8; 4], b: [u8; 4]) -> i32 {
    a.iter().zip(b).map(|(&a, b)| (a as i32 - b as i32).abs()).max().unwrap_or(0)
}

/// Within `tolerance` of the opaque color (0..255 each).
pub fn near(p: [u8; 4], r: u8, g: u8, b: u8, tolerance: i32) -> bool {
    max_difference(p, [r, g, b, 255]) <= tolerance
}

pub static QUAD_VERTEX: ShaderCode =
    ShaderCode { spirv: include_bytes!("shaders/quad.vert.spv"), msl: "", wgsl: "", entry: "main" };
pub static COLOR_FRAGMENT: ShaderCode =
    ShaderCode { spirv: include_bytes!("shaders/color.frag.spv"), msl: "", wgsl: "", entry: "main" };
pub static TEXTURE_FRAGMENT: ShaderCode =
    ShaderCode { spirv: include_bytes!("shaders/texture.frag.spv"), msl: "", wgsl: "", entry: "main" };

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
    let uniforms: Vec<u8> =
        [2.0 / w, 2.0 / h, -1.0, -1.0].iter().chain(&source).flat_map(|v| v.to_ne_bytes()).collect();
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

/// A device (one per process: the tests run in sequence on it) and what the tests draw with.
pub struct Gpu {
    pub pipelines: Pipelines,
    pub device: Box<dyn Device>,
}

impl Gpu {
    /// A device drawing only into textures (lavapipe in CI), keeping its pipelines in `pipeline_cache` if given; why
    /// not if this system has none.
    pub fn start(pipeline_cache: Option<PathBuf>) -> Result<Gpu, String> {
        let surface = Surface { drawable_size: Some(Box::new(|| (64, 64))), pipeline_cache, ..Surface::default() };
        let mut device = device::create(Backend::Vulkan, surface)?;
        let pipelines = Pipelines::new(&mut *device).ok_or("the test pipelines couldn't be made")?;
        Ok(Gpu { pipelines, device })
    }

    /// Whether the device keeps its pipelines in the file it is given: where the driver's pipeline cache holds them
    /// (lavapipe's and SwiftShader's hold nothing).
    pub fn keeps_pipelines(&self) -> bool {
        let name = self.device.device_name();
        !name.contains("llvmpipe") && !name.contains("SwiftShader")
    }

    /// `draw` on a w x h px target cleared to `clear` at `scale` px per dp, in a frame of its own, read back
    /// (premultiplied).
    pub fn render(
        &mut self,
        w: i32,
        h: i32,
        scale: f32,
        draw: impl FnOnce(&mut Frame<'_>),
        clear: ClearColor,
    ) -> Pixels {
        let device = &mut *self.device;
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
