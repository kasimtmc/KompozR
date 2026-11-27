//! The hardware layer on WebGPU, the web's, as the C++ Kompoz's `webgpu_device.cpp` (which ran on Dawn's
//! emdawnwebgpu): through our own JavaScript glue (kompoz-web; [`sys`] wraps the calls, webgpu.js makes them).
//! WebGPU's clip space has y up and its textures their top row first, as uploads and the renderer have them:
//! nothing is flipped, the renderer's projection is ([`Device::clip_y_down`] false).
//!
//! Each frame records one command encoder: uploads (staged in the frame's memory, rows 256 bytes apart as WebGPU
//! wants them) and copies between passes, each pass a render pass. The frame's memory is CPU memory mirrored by GPU
//! buffers, written to them (queue writes, which come before the commands submitted after them) just before the
//! commands are submitted. Uniforms live there too, each draw's at its own dynamic offset of the bind group (binding
//! 0; texture slot i at bindings 2i + 1 and 2i + 2, as `cargo xtask shaders` binds the WGSL). Bind groups are made
//! once a frame for what is bound. Texel slots ([`PipelineDesc::texel_textures`]: `Rgba32f`, which WebGPU doesn't
//! filter) are bound as unfilterable floats with non-filtering samplers. Mipmaps are drawn, each level from the one
//! before. WebGPU validates everything: its errors are counted ([`Device::validation_errors`]).
//!
//! The `GPUDevice` is the page's: asked for asynchronously before the app starts (webgpu.js's `requestGpu`, a
//! loader step), handed over by the platform ([`WebGpuSurface`]) or found among the page's objects (the loader's,
//! `"gpu.device"`). Read-back is asynchronous only ([`Device::read_pixels_async`]): the copy's buffer is mapped
//! after the commands so far are submitted, and [`finish_reads`] hands each read its pixels once its buffer is
//! mapped (the platform's loop calls it when the page says a mapping settled). Presenting is the browser's: the
//! canvas shows what was drawn into its texture when the frame's callback returns.

mod sys;

use crate::device::{
    Backend, BlendFactor, BlendOp, BufferRef, ClearColor, Device, Filter, Format, GpuFrameTime, GpuPassTime, IRect,
    MAX_TEXTURES, MAX_UNIFORMS, Pipeline, PipelineDesc, PipelineId, Retired, RetiredQueue, Sampler, Surface, Texture,
    TextureId, TextureUsage, VertexFormat, WebGpuSurface, Wrap, log, pass_label,
};
use kompoz_web::JsObject;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;

/// A frame's memory comes in pieces this big (or as big as asked).
const CHUNK_SIZE: usize = 4 << 20;
/// Where uniforms start in the frame's memory (WebGPU's least `minUniformBufferOffsetAlignment`).
const UNIFORM_ALIGN: usize = 256;
/// A buffer's rows in copies to and from textures.
const ROW_ALIGN: usize = 256;
/// The most passes of a frame timed.
const TIMED_PASSES: usize = 256;
/// The frames timed at once (a frame goes untimed while all are being read).
const TIMED_FRAMES: usize = 4;

/// Mipmaps: each level drawn from the one before, its texels the four below it averaged (a triangle over the target,
/// sampled bilinearly at each texel's middle).
const MIPMAP_SHADER: &str = r"
@group(0) @binding(0) var source : texture_2d<f32>;
@group(0) @binding(1) var bilinear : sampler;
struct Varyings { @builtin(position) position : vec4f, @location(0) uv : vec2f };
@vertex fn vs(@builtin(vertex_index) i : u32) -> Varyings {
    let corner = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    var out : Varyings;
    out.position = vec4f(corner.x * 2 - 1, 1 - corner.y * 2, 0, 1);
    out.uv = corner;
    return out;
}
@fragment fn fs(v : Varyings) -> @location(0) vec4f { return textureSampleLevel(source, bilinear, v.uv, 0); }
";

/// webgpu.js's number for a format.
fn format_code(format: Format) -> u32 {
    match format {
        Format::Rgba8 => 0,
        Format::Bgra8 => 1,
        Format::R16f => 2,
        Format::Rgba32f => 3,
        Format::Rgba16f => 4,
    }
}

fn vertex_format_code(format: VertexFormat) -> u32 {
    match format {
        VertexFormat::Float => 0,
        VertexFormat::Float2 => 1,
        VertexFormat::Float3 => 2,
        VertexFormat::Float4 => 3,
        VertexFormat::UInt4 => 4,
    }
}

fn factor_code(factor: BlendFactor) -> u32 {
    match factor {
        BlendFactor::Zero => 0,
        BlendFactor::One => 1,
        BlendFactor::SrcAlpha => 2,
        BlendFactor::OneMinusSrcAlpha => 3,
        BlendFactor::DstAlpha => 4,
        BlendFactor::OneMinusDstAlpha => 5,
        BlendFactor::SrcColor => 6,
        BlendFactor::OneMinusSrcColor => 7,
        BlendFactor::DstColor => 8,
        BlendFactor::OneMinusDstColor => 9,
    }
}

fn op_code(op: BlendOp) -> u32 {
    match op {
        BlendOp::Add => 0,
        BlendOp::Subtract => 1,
        BlendOp::ReverseSubtract => 2,
        BlendOp::Min => 3,
        BlendOp::Max => 4,
    }
}

fn wrap_code(wrap: Wrap) -> u32 {
    match wrap {
        Wrap::Clamp => 0,
        Wrap::Repeat => 1,
        Wrap::Mirror => 2,
    }
}

const fn aligned(n: usize, alignment: usize) -> usize {
    n.div_ceil(alignment) * alignment
}

/// A texture of the device's (or the canvas's this frame).
struct Slot {
    texture: JsObject,
    generation: u32,
    levels: u32,
    /// The canvas's: not destroyed here (it is the canvas's), only let go of at the frame's end.
    window: bool,
    /// [`Device::discard`]ed: its next pass that doesn't clear clears (WebGPU has no "don't care").
    discarded: bool,
}

struct PipelineEntry {
    pipeline: JsObject,
    layout: JsObject,
    textures: usize,
}

/// A piece of the frame's memory: CPU memory, mirrored by a GPU buffer written up to where it is used (flushed)
/// before commands are submitted.
struct Chunk {
    buffer: JsObject,
    memory: Vec<u8>,
    used: usize,
    flushed: usize,
}

/// A bind group made this frame, for what it binds.
struct Group {
    layout: u32,
    uniforms: u32,
    textures: [Option<TextureId>; MAX_TEXTURES],
    samplers: [Sampler; MAX_TEXTURES],
    group: JsObject,
}

/// Mipmaps' pipelines: their shader, layout and sampler, a pipeline for each format.
struct Mipmaps {
    module: JsObject,
    layout: JsObject,
    bilinear: u32,
    pipelines: Vec<(Format, JsObject)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TimedState {
    Free,
    Writing,
    Mapping,
}

/// A frame's passes' starts and ends written into a query set, resolved into a buffer and read back once done.
struct TimedFrame {
    queries: JsObject,
    resolved: JsObject,
    /// The read of the resolved times (made for each frame read: a read-back buffer is destroyed once read).
    read: Option<JsObject>,
    /// Pass i's: queries 2i and 2i + 1.
    labels: Vec<String>,
    state: TimedState,
}

/// A read waiting for its buffer to be mapped.
struct Read {
    device: u32,
    buffer: JsObject,
    height: usize,
    row: usize,
    stride: usize,
    bgra: bool,
    done: Box<dyn FnOnce(Vec<u8>)>,
}

thread_local! {
    /// The reads of every device on this thread waiting for their buffers.
    static READS: RefCell<Vec<Read>> = const { RefCell::new(Vec::new()) };
    static DEVICES: Cell<u32> = const { Cell::new(0) };
}

/// Hands each read whose buffer is mapped by now its pixels (top row first, rows packed; RGBA8 for `Bgra8`), and
/// each whose mapping failed nothing; the others wait. For the platform's loop, when the page says a mapping
/// settled (the glue's `onSettled`), and for the test harness after it waited for them: never from inside the
/// device's own calls, so that what `done` does may use the device.
pub fn finish_reads() {
    let ready: Vec<(Read, Option<usize>)> = READS.with(|reads| {
        let mut reads = reads.borrow_mut();
        let mut ready = Vec::new();
        let mut i = 0;
        while i < reads.len() {
            match sys::gpu_map_state(reads[i].buffer.raw()) {
                -1 => i += 1,
                n if n < 0 => ready.push((reads.remove(i), None)),
                n => ready.push((reads.remove(i), Some(n as usize))),
            }
        }
        ready
    });
    for (read, length) in ready {
        let mut pixels = Vec::new();
        if let Some(length) = length {
            let mapped = sys::map_take(read.buffer.raw(), length);
            if mapped.len() >= read.stride * (read.height.max(1) - 1) + read.row {
                pixels.reserve_exact(read.row * read.height);
                for y in 0..read.height {
                    pixels.extend_from_slice(&mapped[y * read.stride..y * read.stride + read.row]);
                }
                if read.bgra {
                    for texel in pixels.as_chunks_mut::<4>().0 {
                        texel.swap(0, 2);
                    }
                }
            } else {
                log("WebGPU: a read-back came back short");
            }
        }
        (read.done)(pixels);
    }
}

/// Whether reads are waiting for their buffers (the page's `glue.pending` has their mappings).
pub fn reads_waiting() -> bool {
    READS.with(|reads| !reads.borrow().is_empty())
}

/// The WebGPU device.
pub struct WebGpuDevice {
    /// The page's `GPUDevice` (a handle of our own to it).
    device: JsObject,
    /// Which device this is, for its reads.
    id: u32,
    name: String,
    timestamps: bool,
    /// The canvas presented to (the platform's handle) and the format its context is configured with.
    canvas: Option<u32>,
    canvas_format: Format,
    configured: (i32, i32),
    drawable_size: Option<Box<dyn Fn() -> (i32, i32)>>,
    retired: Arc<RetiredQueue>,

    slots: Vec<Option<Slot>>,
    /// Each slot's generation, kept while it is free.
    generations: Vec<u32>,
    free: Vec<u32>,
    pipelines: HashMap<u64, PipelineEntry>,
    next_pipeline: u64,
    modules: Vec<(usize, JsObject)>,
    samplers: Vec<(Sampler, JsObject)>,
    mipmaps: Option<Mipmaps>,

    chunks: Vec<Chunk>,
    encoder: Option<JsObject>,
    pass: Option<JsObject>,
    target: Option<TextureId>,
    /// Where the pass draws: scissors stay within it.
    pass_area: IRect,
    /// The canvas's texture this frame.
    window: Option<TextureId>,
    in_frame: bool,

    pipeline: Option<PipelineId>,
    /// The uniforms the next draw binds: a frame memory buffer, at an offset.
    uniforms: Option<(u32, u32)>,
    bound: [(Option<TextureId>, Sampler); MAX_TEXTURES],
    groups: Vec<Group>,

    timing: bool,
    timed_frames: Vec<TimedFrame>,
    timed_now: Option<usize>,
    timed: Vec<GpuFrameTime>,
}

impl WebGpuDevice {
    /// A device on the page's `GPUDevice` (the surface's, else the loader's), presenting to the surface's canvas if
    /// it has one (else drawing only into textures); why not if the page has no WebGPU device.
    pub fn new(surface: Surface) -> Result<WebGpuDevice, String> {
        let Surface { webgpu, drawable_size, .. } = surface;
        let (device, canvas) = match webgpu {
            Some(WebGpuSurface { device, canvas }) => (JsObject::share_raw(device).ok_or("no WebGPU device")?, canvas),
            None => {
                let device = kompoz_web::page_object("gpu.device").ok_or_else(|| {
                    sys::unavailable().unwrap_or_else(|| "the page asked for no WebGPU device".to_string())
                })?;
                (device, None)
            }
        };
        let features = sys::gpu_device_attach(device.raw());
        let name = sys::device_name(device.raw());
        let canvas_format = if sys::gpu_preferred_format() == 0 { Format::Rgba8 } else { Format::Bgra8 };
        let id = DEVICES.with(|d| {
            d.set(d.get() + 1);
            d.get()
        });
        Ok(WebGpuDevice {
            device,
            id,
            name,
            timestamps: features & 1 != 0,
            canvas,
            canvas_format,
            configured: (0, 0),
            drawable_size,
            retired: Arc::default(),
            slots: Vec::new(),
            generations: Vec::new(),
            free: Vec::new(),
            pipelines: HashMap::new(),
            next_pipeline: 1,
            modules: Vec::new(),
            samplers: Vec::new(),
            mipmaps: None,
            chunks: Vec::new(),
            encoder: None,
            pass: None,
            target: None,
            pass_area: IRect::default(),
            window: None,
            in_frame: false,
            pipeline: None,
            uniforms: None,
            bound: [(None, Sampler::default()); MAX_TEXTURES],
            groups: Vec::new(),
            timing: false,
            timed_frames: Vec::new(),
            timed_now: None,
            timed: Vec::new(),
        })
    }

    fn raw(&self) -> u32 {
        self.device.raw()
    }

    fn slot(&self, id: TextureId) -> Option<&Slot> {
        self.slots.get(id.slot() as usize)?.as_ref().filter(|s| s.generation == id.generation())
    }

    fn slot_mut(&mut self, id: TextureId) -> Option<&mut Slot> {
        self.slots.get_mut(id.slot() as usize)?.as_mut().filter(|s| s.generation == id.generation())
    }

    /// The texture's handle (0 if it is gone).
    fn texture(&self, id: TextureId) -> u32 {
        self.slot(id).map_or(0, |s| s.texture.raw())
    }

    fn add_slot(
        &mut self,
        texture: JsObject,
        width: i32,
        height: i32,
        format: Format,
        levels: u32,
        window: bool,
    ) -> TextureId {
        let index = match self.free.pop() {
            Some(i) => i,
            None => {
                self.slots.push(None);
                self.generations.push(0);
                (self.slots.len() - 1) as u32
            }
        };
        let generation = self.generations[index as usize] + 1;
        self.generations[index as usize] = generation;
        self.slots[index as usize] = Some(Slot { texture, generation, levels, window, discarded: false });
        TextureId::new(index, generation, width, height, format)
    }

    fn remove_slot(&mut self, id: TextureId) {
        let Some(slot) = self.slots.get_mut(id.slot() as usize) else { return };
        if slot.as_ref().is_none_or(|s| s.generation != id.generation()) {
            return;
        }
        let slot = slot.take().expect("checked above");
        if slot.window {
            drop(slot.texture); // (the canvas's: only let go of)
        } else {
            sys::gpu_destroy_texture(slot.texture.into_raw());
        }
        self.groups.retain(|g| !g.textures.contains(&Some(id)));
        self.free.push(id.slot());
    }

    /// The encoder recording now (a new one if the frame has none).
    fn encoder(&mut self) -> u32 {
        if self.encoder.is_none() {
            self.encoder = JsObject::from_raw(sys::gpu_command_encoder(self.raw()));
        }
        self.encoder.as_ref().map_or(0, JsObject::raw)
    }

    /// The frame's memory written to its buffers, the commands so far submitted; then what was let go of destroyed
    /// (WebGPU destroys it once the work submitted with it is done).
    fn submit(&mut self) {
        let device = self.raw();
        for c in &mut self.chunks {
            if c.used > c.flushed {
                let (from, to) = (c.flushed & !3, aligned(c.used, 4)); // (writes are 4-byte aligned)
                sys::write_buffer(device, c.buffer.raw(), from as u64, &c.memory[from..to]);
            }
            c.flushed = c.used;
        }
        if let Some(encoder) = self.encoder.take() {
            sys::gpu_submit(device, encoder.into_raw());
        }
        self.collect_retired();
    }

    fn collect_retired(&mut self) {
        for retired in self.retired.take() {
            match retired {
                Retired::Texture(id) => self.remove_slot(id),
                Retired::Pipeline(id) => {
                    self.pipelines.remove(&id.raw());
                }
            }
        }
    }

    /// `size` bytes of the frame's memory at a multiple of `alignment` (16 at least): the buffer, the offset.
    fn allocate(&mut self, size: usize, alignment: usize) -> Option<(usize, usize)> {
        let alignment = alignment.max(16);
        for (i, c) in self.chunks.iter_mut().enumerate() {
            let start = aligned(c.used, alignment);
            if start + size <= c.memory.len() {
                c.used = start + size;
                return Some((i, start));
            }
        }
        let length = aligned(CHUNK_SIZE.max(size), 4);
        let buffer = JsObject::from_raw(sys::gpu_create_buffer(self.raw(), length as f64, 0))?;
        self.chunks.push(Chunk { buffer, memory: vec![0; length], used: size, flushed: 0 });
        Some((self.chunks.len() - 1, 0))
    }

    fn module(&mut self, code: &'static str) -> u32 {
        let key = code.as_ptr() as usize;
        if let Some((_, m)) = self.modules.iter().find(|(k, _)| *k == key) {
            return m.raw();
        }
        let Some(m) = sys::shader_module(self.raw(), code) else { return 0 };
        let raw = m.raw();
        self.modules.push((key, m));
        raw
    }

    fn sampler(&mut self, s: Sampler) -> u32 {
        if let Some((_, made)) = self.samplers.iter().find(|(d, _)| *d == s) {
            return made.raw();
        }
        // (Mipmaps: the nearest one, as Skia's FilterQuality::Medium and the other backends take.)
        let linear = u32::from(s.filter == Filter::Linear);
        let lod_max = if s.mipmaps { 32.0 } else { 0.0 };
        let raw = sys::gpu_sampler(self.raw(), linear, wrap_code(s.wrap_u), wrap_code(s.wrap_v), lod_max);
        let Some(made) = JsObject::from_raw(raw) else { return 0 };
        self.samplers.push((s, made));
        raw
    }

    /// The timestamps of a pass of this frame's, if it is timed (and has room for them): its query set and its
    /// first query.
    fn time_pass(&mut self, label: String) -> (u32, u32) {
        let Some(f) = self.timed_now.and_then(|i| self.timed_frames.get_mut(i)) else { return (0, 0) };
        if f.labels.len() >= TIMED_PASSES {
            return (0, 0);
        }
        let first = f.labels.len() as u32 * 2;
        f.labels.push(label);
        (f.queries.raw(), first)
    }

    /// The frames read back since: each pass from its start to its end, the frame theirs together (nanoseconds; a
    /// browser rounds them).
    fn collect_times(&mut self) {
        for f in &mut self.timed_frames {
            if f.state != TimedState::Mapping {
                continue;
            }
            let Some(read) = &f.read else {
                f.state = TimedState::Free;
                continue;
            };
            let n = sys::gpu_map_state(read.raw());
            if n == -1 {
                continue;
            }
            if n >= 0 {
                let bytes = sys::map_take(read.raw(), n as usize);
                let times: Vec<u64> = bytes.as_chunks::<8>().0.iter().map(|b| u64::from_le_bytes(*b)).collect();
                let mut frame = GpuFrameTime::default();
                for (i, label) in f.labels.iter().enumerate() {
                    let (Some(&a), Some(&b)) = (times.get(2 * i), times.get(2 * i + 1)) else { break };
                    let ms = (b.max(a) - a) as f64 / 1e6;
                    frame.passes.push(GpuPassTime { label: label.clone(), ms });
                    frame.ms += ms;
                }
                self.timed.push(frame);
            }
            f.read = None;
            f.state = TimedState::Free;
        }
    }

    /// The bind group for what is bound now: one made earlier this frame for the same, or a new one; none if the
    /// pipeline samples textures and none is bound.
    fn bind_group(&mut self, layout: u32, textures_used: usize, uniforms: u32) -> Option<u32> {
        let mut textures = [None; MAX_TEXTURES];
        let mut samplers = [Sampler::default(); MAX_TEXTURES];
        // (Unused slots: any texture bound, as the shaders don't read them.)
        let any = self.bound.iter().find(|(t, _)| t.is_some_and(|t| self.slot(t).is_some())).copied();
        for i in 0..textures_used {
            let (t, s) = match self.bound[i] {
                (Some(t), s) if self.slot(t).is_some() => (t, s),
                _ => match any {
                    Some((Some(t), s)) => (t, s),
                    _ => return None,
                },
            };
            textures[i] = Some(t);
            samplers[i] = s;
        }
        if let Some(g) = self
            .groups
            .iter()
            .find(|g| g.layout == layout && g.uniforms == uniforms && g.textures == textures && g.samplers == samplers)
        {
            return Some(g.group.raw());
        }
        let mut entries = vec![[0, 0, uniforms, MAX_UNIFORMS as u32]];
        for i in 0..textures_used {
            let texture = self.texture(textures[i].expect("set above"));
            let sampler = self.sampler(samplers[i]);
            entries.push([2 * i as u32 + 1, 1, texture, u32::MAX]);
            entries.push([2 * i as u32 + 2, 2, sampler, 0]);
        }
        let group = sys::bind_group(self.raw(), layout, &entries)?;
        let raw = group.raw();
        self.groups.push(Group { layout, uniforms, textures, samplers, group });
        Some(raw)
    }

    fn mipmap_pipeline(&mut self, format: Format) -> Option<u32> {
        if self.mipmaps.is_none() {
            let module = sys::shader_module(self.raw(), MIPMAP_SHADER)?;
            let layout = JsObject::from_raw(sys::gpu_mipmap_layout(self.raw()))?;
            let bilinear = self.sampler(Sampler { filter: Filter::Linear, ..Sampler::default() });
            self.mipmaps = Some(Mipmaps { module, layout, bilinear, pipelines: Vec::new() });
        }
        let m = self.mipmaps.as_ref().expect("made above");
        if let Some((_, p)) = m.pipelines.iter().find(|(f, _)| *f == format) {
            return Some(p.raw());
        }
        // (A pipeline drawing one triangle over the target, the level before sampled, written as it is.)
        let desc = [format_code(format), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let module = m.module.raw();
        let pipeline = sys::render_pipeline(self.raw(), m.layout.raw(), (module, "vs"), (module, "fs"), &desc)?;
        let raw = pipeline.raw();
        self.mipmaps.as_mut().expect("made above").pipelines.push((format, pipeline));
        Some(raw)
    }

    fn expect_outside_passes(&self, call: &str) {
        assert!(self.in_frame && self.target.is_none(), "Device::{call} needs a frame begun and no pass going");
    }

    fn expect_pass(&self, call: &str) {
        assert!(self.target.is_some(), "Device::{call} needs a pass begun");
    }
}

impl Drop for WebGpuDevice {
    fn drop(&mut self) {
        // Its reads get nothing (the device went away meanwhile), now.
        let id = self.id;
        let gone: Vec<Read> = READS.with(|reads| {
            let mut reads = reads.borrow_mut();
            let (gone, kept) = std::mem::take(&mut *reads).into_iter().partition(|r| r.device == id);
            *reads = kept;
            gone
        });
        for read in gone {
            (read.done)(Vec::new());
        }
        self.pass = None;
        self.encoder = None;
        self.groups.clear();
        let ids: Vec<TextureId> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|s| TextureId::new(i as u32, s.generation, 0, 0, Format::Rgba8)))
            .collect();
        for id in ids {
            self.remove_slot(id);
        }
        for c in self.chunks.drain(..) {
            sys::gpu_destroy_buffer(c.buffer.into_raw());
        }
        for f in self.timed_frames.drain(..) {
            sys::gpu_destroy_query_set(f.queries.into_raw());
            sys::gpu_destroy_buffer(f.resolved.into_raw());
        }
    }
}

impl Device for WebGpuDevice {
    fn backend(&self) -> Backend {
        Backend::WebGpu
    }

    fn device_name(&self) -> String {
        self.name.clone()
    }

    fn validation_errors(&self) -> u32 {
        sys::gpu_device_errors(self.raw())
    }

    fn clip_y_down(&self) -> bool {
        false
    }

    fn create_texture(&mut self, width: i32, height: i32, format: Format, usage: TextureUsage) -> Option<Texture> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let mipmapped = usage.contains(TextureUsage::MIPMAPPED);
        // (Mipmaps are drawn: a mipmapped texture is drawn into too.)
        let attachment = u32::from(usage.contains(TextureUsage::RENDER_TARGET) || mipmapped);
        let levels = if mipmapped { u32::BITS - (width.max(height) as u32).leading_zeros() } else { 1 };
        let raw =
            sys::gpu_create_texture(self.raw(), width as u32, height as u32, format_code(format), attachment, levels);
        let texture = JsObject::from_raw(raw)?;
        let id = self.add_slot(texture, width, height, format, levels, false);
        Some(Texture::new(id, self.retired.clone()))
    }

    fn create_pipeline(&mut self, desc: &PipelineDesc<'_>) -> Option<Pipeline> {
        let layout = JsObject::from_raw(sys::gpu_bind_group_layout(
            self.raw(),
            desc.textures as u32,
            u32::from(desc.texel_textures),
            MAX_UNIFORMS as u32,
        ))?;
        let b = desc.blend;
        let mut words = vec![
            format_code(desc.target),
            u32::from(b.enabled),
            factor_code(b.src_color),
            factor_code(b.dst_color),
            op_code(b.color_op),
            factor_code(b.src_alpha),
            factor_code(b.dst_alpha),
            op_code(b.alpha_op),
            desc.instance_stride,
            desc.instance_attributes.len() as u32,
        ];
        for a in &desc.instance_attributes {
            words.extend([a.location, vertex_format_code(a.format), a.offset]);
        }
        // (WGSL's overrides, named by their @id: 0, 1, …)
        words.push(desc.constants.len() as u32);
        words.extend(desc.constants.iter().map(|&c| c as u32));
        let vertex = self.module(desc.vertex.wgsl);
        let fragment = self.module(desc.fragment.wgsl);
        let pipeline = sys::render_pipeline(
            self.raw(),
            layout.raw(),
            (vertex, desc.vertex.entry),
            (fragment, desc.fragment.entry),
            &words,
        )?;
        let id = self.next_pipeline;
        self.next_pipeline += 1;
        self.pipelines.insert(id, PipelineEntry { pipeline, layout, textures: desc.textures });
        Some(Pipeline::new(PipelineId::new(id), self.retired.clone()))
    }

    fn swapchain_format(&self) -> Format {
        self.canvas_format
    }

    fn begin_frame(&mut self) {
        self.collect_times();
        self.timed_now = None;
        if self.timing
            && let Some(i) = self.timed_frames.iter().position(|f| f.state == TimedState::Free)
        {
            let f = &mut self.timed_frames[i];
            f.state = TimedState::Writing;
            f.labels.clear();
            self.timed_now = Some(i);
        }
        for c in &mut self.chunks {
            c.used = 0;
            c.flushed = 0;
        }
        self.groups.clear();
        if let Some(w) = self.window.take() {
            self.remove_slot(w);
        }
        self.collect_retired();
        self.encoder();
        self.in_frame = true;
    }

    fn swapchain_texture(&mut self) -> Option<TextureId> {
        let canvas = self.canvas?;
        if let Some(w) = self.window {
            return Some(w);
        }
        let (w, h) = self.drawable_size.as_ref().map_or((0, 0), |size| size());
        if w <= 0 || h <= 0 {
            return None; // (minimized)
        }
        if (w, h) != self.configured {
            let format = format_code(self.canvas_format);
            if sys::gpu_canvas_configure(self.raw(), canvas, format, w as u32, h as u32) == 0 {
                return None;
            }
            self.configured = (w, h);
        }
        let Some(texture) = JsObject::from_raw(sys::gpu_canvas_texture(canvas)) else {
            self.configured = (0, 0); // (configured again next frame)
            return None;
        };
        let id = self.add_slot(texture, w, h, self.canvas_format, 1, true);
        self.window = Some(id);
        Some(id)
    }

    fn write_frame_data(&mut self, data: &[u8], alignment: usize) -> Option<BufferRef> {
        // (Outside frames the frame's memory may be being written for the frame before.)
        assert!(self.in_frame, "Device::write_frame_data needs a frame begun");
        let (chunk, offset) = self.allocate(data.len(), alignment)?;
        let c = &mut self.chunks[chunk];
        c.memory[offset..offset + data.len()].copy_from_slice(data);
        Some(BufferRef::new(u64::from(c.buffer.raw()), offset as u64))
    }

    fn upload(&mut self, texture: TextureId, region: IRect, pixels: &[u8], row_bytes: usize) {
        self.expect_outside_passes("upload");
        if region.is_empty() || self.slot(texture).is_none() {
            return;
        }
        let row = region.width as usize * texture.format().bytes_per_texel();
        let rows = region.height as usize;
        assert!(
            row_bytes >= row && pixels.len() >= row_bytes * (rows - 1) + row,
            "upload: {} bytes, {row_bytes} a row, for {}x{} texels of {:?}",
            pixels.len(),
            region.width,
            region.height,
            texture.format()
        );
        let stride = aligned(row, ROW_ALIGN);
        let Some((chunk, offset)) = self.allocate(stride * rows, ROW_ALIGN) else { return };
        let swap = texture.format() == Format::Bgra8; // (the pixels are RGBA)
        let c = &mut self.chunks[chunk];
        for y in 0..rows {
            let out = &mut c.memory[offset + y * stride..offset + y * stride + row];
            let source = &pixels[y * row_bytes..y * row_bytes + row];
            if swap {
                for (o, s) in out.as_chunks_mut::<4>().0.iter_mut().zip(source.as_chunks::<4>().0) {
                    *o = [s[2], s[1], s[0], s[3]];
                }
            } else {
                out.copy_from_slice(source);
            }
        }
        let buffer = c.buffer.raw();
        let target = self.texture(texture);
        let encoder = self.encoder();
        sys::gpu_copy_buffer_to_texture(
            encoder,
            buffer,
            offset as f64,
            stride as u32,
            target,
            region.x as u32,
            region.y as u32,
            region.width as u32,
            rows as u32,
        );
    }

    fn begin_pass(&mut self, target: TextureId, clear: Option<ClearColor>, reads: &[TextureId], area: Option<IRect>) {
        self.expect_outside_passes("begin_pass");
        let _ = reads; // (WebGPU orders what passes read and write itself)
        let window = self.window == Some(target);
        let label = pass_label(target.width(), target.height(), target.format(), window);
        let (queries, first) = self.time_pass(label);
        let Some(slot) = self.slot_mut(target) else { return };
        // A clear of only an area (the rest kept): the target loaded, the area drawn over with the color (WebGPU's
        // clears are of whole targets), but for RGBA32F, which doesn't blend (a texture only uploaded, in practice).
        let all = target.bounds();
        let partial = area.filter(|a| *a != all && target.format() != Format::Rgba32f);
        // (No "don't care" in WebGPU: a discarded texture is cleared.)
        let load_clear = (clear.is_some() && partial.is_none()) || slot.discarded;
        let area_clear = if slot.discarded { None } else { clear.zip(partial) };
        slot.discarded = false;
        let texture = slot.texture.raw();
        let c = clear.unwrap_or_default();
        let encoder = self.encoder();
        let pass = sys::gpu_begin_pass(
            encoder,
            texture,
            0,
            u32::from(load_clear),
            f64::from(c.r),
            f64::from(c.g),
            f64::from(c.b),
            f64::from(c.a),
            queries,
            first,
        );
        if let Some((c, a)) = area_clear {
            let (x0, y0) = (a.x.max(0), a.y.max(0));
            let (x1, y1) = ((a.x + a.width).min(all.width).max(x0), (a.y + a.height).min(all.height).max(y0));
            sys::gpu_pass_clear_area(
                self.raw(),
                pass,
                format_code(target.format()),
                x0 as u32,
                y0 as u32,
                (x1 - x0) as u32,
                (y1 - y0) as u32,
                f64::from(c.r),
                f64::from(c.g),
                f64::from(c.b),
                f64::from(c.a),
            );
        }
        self.pass = JsObject::from_raw(pass);
        self.target = Some(target);
        self.pipeline = None;
        self.uniforms = None;
        self.bound = [(None, Sampler::default()); MAX_TEXTURES];
        // (Its area only bounds its scissors here: all of the target is loaded and stored, or cleared.)
        self.pass_area = area.unwrap_or(target.bounds());
        self.set_scissor(self.pass_area);
    }

    fn set_pipeline(&mut self, pipeline: PipelineId) {
        self.expect_pass("set_pipeline");
        if self.pipeline == Some(pipeline) {
            return;
        }
        let (Some(pass), Some(p)) = (&self.pass, self.pipelines.get(&pipeline.raw())) else { return };
        sys::gpu_pass_pipeline(pass.raw(), p.pipeline.raw());
        self.pipeline = Some(pipeline);
    }

    fn set_scissor(&mut self, r: IRect) {
        self.expect_pass("set_scissor");
        let (Some(target), Some(pass)) = (self.target, &self.pass) else { return };
        let a = self.pass_area;
        let x0 = 0.max(r.x).max(a.x);
        let y0 = 0.max(r.y).max(a.y);
        let x1 = x0.max((r.x + r.width).min(target.width()).min(a.x + a.width));
        let y1 = y0.max((r.y + r.height).min(target.height()).min(a.y + a.height));
        sys::gpu_pass_scissor(pass.raw(), x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32);
    }

    fn set_uniforms(&mut self, data: &[u8]) {
        self.expect_pass("set_uniforms");
        let Some((chunk, offset)) = self.allocate(MAX_UNIFORMS, UNIFORM_ALIGN) else { return };
        let c = &mut self.chunks[chunk];
        let n = data.len().min(MAX_UNIFORMS);
        c.memory[offset..offset + MAX_UNIFORMS].fill(0);
        c.memory[offset..offset + n].copy_from_slice(&data[..n]);
        self.uniforms = Some((c.buffer.raw(), offset as u32));
    }

    fn set_texture(&mut self, slot: usize, texture: TextureId, sampler: Sampler) {
        if slot < MAX_TEXTURES {
            self.bound[slot] = (Some(texture), sampler);
        }
    }

    fn draw(&mut self, instances: Option<BufferRef>, vertex_count: u32, instance_count: u32) {
        self.expect_pass("draw");
        if instance_count == 0 {
            return;
        }
        let Some((layout, textures)) =
            self.pipeline.and_then(|p| self.pipelines.get(&p.raw())).map(|p| (p.layout.raw(), p.textures))
        else {
            return;
        };
        if self.uniforms.is_none() {
            // (None set: zeros, as unset push constants read.)
            self.set_uniforms(&[]);
        }
        let Some((buffer, offset)) = self.uniforms else { return };
        let Some(group) = self.bind_group(layout, textures, buffer) else { return };
        let Some(pass) = &self.pass else { return };
        let pass = pass.raw();
        sys::gpu_pass_bind_group(pass, group, offset as i32);
        if let Some(b) = instances {
            sys::gpu_pass_vertex_buffer(pass, b.buffer() as u32, b.offset() as f64);
        }
        sys::gpu_pass_draw(pass, vertex_count, instance_count);
    }

    fn end_pass(&mut self) {
        self.expect_pass("end_pass");
        if let Some(pass) = self.pass.take() {
            sys::gpu_pass_end(pass.into_raw());
        }
        self.target = None;
    }

    fn generate_mipmaps(&mut self, texture: TextureId) {
        self.expect_outside_passes("generate_mipmaps");
        let Some(levels) = self.slot(texture).map(|s| s.levels) else { return };
        if levels <= 1 {
            return;
        }
        let Some(pipeline) = self.mipmap_pipeline(texture.format()) else { return };
        let (layout, bilinear) = {
            let m = self.mipmaps.as_ref().expect("made by mipmap_pipeline");
            (m.layout.raw(), m.bilinear)
        };
        let handle = self.texture(texture);
        for level in 1..levels {
            let entries = [[0, 1, handle, level - 1], [1, 2, bilinear, 0]];
            let Some(group) = sys::bind_group(self.raw(), layout, &entries) else { return };
            let label = format!("mipmaps {}x{}", texture.width(), texture.height());
            let (queries, first) = self.time_pass(label);
            let encoder = self.encoder();
            let pass = sys::gpu_begin_pass(encoder, handle, level, 1, 0.0, 0.0, 0.0, 0.0, queries, first);
            sys::gpu_pass_pipeline(pass, pipeline);
            sys::gpu_pass_bind_group(pass, group.raw(), -1);
            sys::gpu_pass_draw(pass, 3, 1);
            sys::gpu_pass_end(pass);
        }
    }

    fn discard(&mut self, texture: TextureId) {
        if let Some(s) = self.slot_mut(texture) {
            s.discarded = true;
        }
    }

    fn copy_texture(&mut self, source: TextureId, region: IRect, destination: TextureId, x: i32, y: i32) {
        self.expect_outside_passes("copy_texture");
        let (from, to) = (self.texture(source), self.texture(destination));
        if region.is_empty() || from == 0 || to == 0 {
            return;
        }
        let encoder = self.encoder();
        sys::gpu_copy_texture_to_texture(
            encoder,
            from,
            region.x as u32,
            region.y as u32,
            to,
            x as u32,
            y as u32,
            region.width as u32,
            region.height as u32,
        );
    }

    /// (WebGPU can't wait for the GPU: none, as [`Device::read_pixels`] says; [`Device::read_pixels_async`] reads.)
    fn read_pixels(&mut self, texture: TextureId, region: IRect) -> Vec<u8> {
        self.expect_outside_passes("read_pixels");
        let _ = (texture, region);
        Vec::new()
    }

    fn reads_pixels_at_once(&self) -> bool {
        false
    }

    /// The copy into a buffer recorded, the commands so far submitted (the rest of the frame recorded after them),
    /// and the buffer's mapping started: [`finish_reads`] hands `done` the pixels once it is mapped.
    fn read_pixels_async(&mut self, texture: TextureId, region: IRect, done: Box<dyn FnOnce(Vec<u8>)>) {
        self.expect_outside_passes("read_pixels_async");
        let handle = self.texture(texture);
        if region.is_empty() || handle == 0 {
            return done(Vec::new());
        }
        let row = region.width as usize * texture.format().bytes_per_texel();
        let stride = aligned(row, ROW_ALIGN);
        let size = (stride * region.height as usize).max(4);
        let Some(buffer) = JsObject::from_raw(sys::gpu_create_buffer(self.raw(), size as f64, 1)) else {
            return done(Vec::new());
        };
        let encoder = self.encoder();
        sys::gpu_copy_texture_to_buffer(
            encoder,
            handle,
            region.x as u32,
            region.y as u32,
            region.width as u32,
            region.height as u32,
            buffer.raw(),
            stride as u32,
        );
        self.submit();
        sys::gpu_map_read(buffer.raw(), size as f64);
        READS.with(|reads| {
            reads.borrow_mut().push(Read {
                device: self.id,
                buffer,
                height: region.height as usize,
                row,
                stride,
                bgra: texture.format() == Format::Bgra8,
                done,
            })
        });
        if self.in_frame {
            self.encoder(); // (the rest of the frame)
        }
    }

    fn end_frame(&mut self, present: bool) {
        if !self.in_frame {
            return;
        }
        self.expect_outside_passes("end_frame");
        let timed = self.timed_now.take();
        let times = timed.map_or(0, |i| self.timed_frames[i].labels.len() * 2);
        if times > 0 {
            let i = timed.expect("timed");
            let read = JsObject::from_raw(sys::gpu_create_buffer(self.raw(), (times * 8) as f64, 1));
            let encoder = self.encoder();
            let f = &mut self.timed_frames[i];
            sys::gpu_resolve_queries(encoder, f.queries.raw(), times as u32, f.resolved.raw());
            if let Some(read) = &read {
                sys::gpu_copy_buffer_to_buffer(encoder, f.resolved.raw(), read.raw(), (times * 8) as f64);
            }
            f.read = read;
        }
        self.submit();
        if let Some(i) = timed {
            let f = &mut self.timed_frames[i];
            match (&f.read, times) {
                (Some(read), t) if t > 0 => {
                    sys::gpu_map_read(read.raw(), (t * 8) as f64);
                    f.state = TimedState::Mapping;
                }
                _ => f.state = TimedState::Free,
            }
        }
        // (Presenting is the browser's: the canvas shows its texture when the frame's callback returns.)
        let _ = present;
        if let Some(w) = self.window.take() {
            self.remove_slot(w);
        }
        self.in_frame = false;
    }

    fn set_gpu_timing(&mut self, on: bool) -> bool {
        if on && !self.timestamps {
            return false;
        }
        self.timing = on;
        while on && self.timed_frames.len() < TIMED_FRAMES {
            let queries = JsObject::from_raw(sys::gpu_query_set(self.raw(), (TIMED_PASSES * 2) as u32));
            let resolved = JsObject::from_raw(sys::gpu_create_buffer(self.raw(), (TIMED_PASSES * 16) as f64, 2));
            let (Some(queries), Some(resolved)) = (queries, resolved) else { return false };
            self.timed_frames.push(TimedFrame {
                queries,
                resolved,
                read: None,
                labels: Vec::new(),
                state: TimedState::Free,
            });
        }
        true
    }

    fn take_gpu_times(&mut self) -> Vec<GpuFrameTime> {
        self.collect_times();
        std::mem::take(&mut self.timed)
    }

    /// (The page can't wait for its GPU; it needn't: WebGPU destroys what submitted work uses once that is done.
    /// What was let go of is destroyed now, between frames.)
    fn wait_idle(&mut self) {
        if !self.in_frame {
            self.collect_retired();
        }
    }
}
