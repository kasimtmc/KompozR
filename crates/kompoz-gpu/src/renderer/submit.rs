//! Submitting what was recorded (C++'s Renderer::flush, drawMasks, execute): the glyphs, the white texel and the
//! frame's table uploaded, the paths' coverage drawn into the masks, then each pass; and the pipelines its batches
//! are drawn with, made once each (a variant of the shader for what a batch needs, the whole shader where the driver
//! can't make one), made ahead on a thread of their own from the list of those an app's earlier runs made.

use super::{Batch, EffectDraw, Mask, Renderer, STOPS_WIDTH, ShaderVariant};
use crate::device::{
    BlendFactor, BlendOp, BlendState, BufferRef, ClearColor, Filter, Format, IRect, Pipeline, PipelineDesc, PipelineId,
    Sampler, ShaderCode, TextureId, TextureUsage, VertexAttribute, VertexFormat,
};
use crate::shaders::{
    EFFECTS_FS, EFFECTS_VS, EdgeInstance, GpuData, MESH_VERTICES, PATH_FS, PATH_VS, PRIMITIVES_FS, PRIMITIVES_VS,
    PRIMITIVES_VS_MESH, PathUniforms, Prim, PrimitiveUniforms, QUAD_VERTICES, STRETCH_FS, STRETCH_VS, ShaderSource,
    bytes_of, to_clip,
};
use kompoz_graphics::{BlendMode, Projective};
use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;

/// An entry point's code as the device takes it.
const fn code(s: ShaderSource) -> ShaderCode {
    ShaderCode { spirv: s.spirv, msl: s.msl, wgsl: s.wgsl, entry: s.entry }
}

static VERTEX: ShaderCode = code(PRIMITIVES_VS);
static FRAGMENT: ShaderCode = code(PRIMITIVES_FS);
static MESH_VERTEX: ShaderCode = code(PRIMITIVES_VS_MESH);
static COVERAGE_VERTEX: ShaderCode = code(PATH_VS);
static COVERAGE_FRAGMENT: ShaderCode = code(PATH_FS);
static EFFECT_VERTEX: ShaderCode = code(EFFECTS_VS);
static EFFECT_FRAGMENT: ShaderCode = code(EFFECTS_FS);
static STRETCH_VERTEX: ShaderCode = code(STRETCH_VS);
static STRETCH_FRAGMENT: ShaderCode = code(STRETCH_FS);

// ---- Blend modes ---------------------------------------------------------------------------------------------------

/// How a blend mode is drawn: by the blend unit (`state`), the shader putting the coverage in as `how` says (see
/// shaders/primitives.slang's finish), or worked out in the shader from a copy of the target (`reads_target`, the
/// blend unit off).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct BlendPlan {
    pub(super) state: BlendState,
    pub(super) how: i32,
    pub(super) reads_target: bool,
}

pub(super) fn plan_for(mode: BlendMode, opaque: bool) -> BlendPlan {
    use BlendFactor as F;
    let unit = |src, dst, src_alpha, dst_alpha, how| BlendPlan {
        state: BlendState::factors(src, dst, src_alpha, dst_alpha),
        how,
        reads_target: false,
    };
    if opaque {
        return BlendPlan { state: BlendState::OFF, how: 0, reads_target: false };
    }
    match mode {
        // The coverage multiplied in mixes these with what is there exactly.
        BlendMode::SrcOver => unit(F::One, F::OneMinusSrcAlpha, F::One, F::OneMinusSrcAlpha, 0),
        BlendMode::DstOver => unit(F::OneMinusDstAlpha, F::One, F::OneMinusDstAlpha, F::One, 0),
        BlendMode::SrcAtop => unit(F::DstAlpha, F::OneMinusSrcAlpha, F::DstAlpha, F::OneMinusSrcAlpha, 0),
        BlendMode::DstOut => unit(F::Zero, F::OneMinusSrcAlpha, F::Zero, F::OneMinusSrcAlpha, 0),
        BlendMode::Xor => unit(F::OneMinusDstAlpha, F::OneMinusSrcAlpha, F::OneMinusDstAlpha, F::OneMinusSrcAlpha, 0),
        BlendMode::Plus => unit(F::One, F::One, F::One, F::One, 0),
        BlendMode::Screen => unit(F::One, F::OneMinusSrcColor, F::One, F::OneMinusSrcAlpha, 0),
        // These mix towards a source that changes nothing; Clear by the coverage alone.
        BlendMode::Modulate => unit(F::Zero, F::SrcColor, F::Zero, F::SrcAlpha, 1),
        BlendMode::DstIn => unit(F::Zero, F::SrcAlpha, F::Zero, F::SrcAlpha, 2),
        BlendMode::Clear => unit(F::Zero, F::OneMinusSrcAlpha, F::Zero, F::OneMinusSrcAlpha, 3),
        // Src, SrcIn, SrcOut, DstAtop and the separable and non-separable modes.
        _ => BlendPlan { state: BlendState::OFF, how: 4, reads_target: true },
    }
}

// ---- Pipelines -----------------------------------------------------------------------------------------------------

/// What a pipeline is made for: the target's format, the blend unit's state, vsMesh's triangles (not vs's quads), the
/// shader's variant and how the coverage goes in (the blend plan's).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) struct PipelineKey {
    format: Format,
    blend: BlendState,
    mesh: bool,
    variant: ShaderVariant,
    how: i32,
}

const FORMATS: [Format; 5] = [Format::Rgba8, Format::Bgra8, Format::R16f, Format::Rgba32f, Format::Rgba16f];
const FACTORS: [BlendFactor; 10] = [
    BlendFactor::Zero,
    BlendFactor::One,
    BlendFactor::SrcAlpha,
    BlendFactor::OneMinusSrcAlpha,
    BlendFactor::DstAlpha,
    BlendFactor::OneMinusDstAlpha,
    BlendFactor::SrcColor,
    BlendFactor::OneMinusSrcColor,
    BlendFactor::DstColor,
    BlendFactor::OneMinusDstColor,
];
const OPS: [BlendOp; 5] = [BlendOp::Add, BlendOp::Subtract, BlendOp::ReverseSubtract, BlendOp::Min, BlendOp::Max];

/// Where `v` is in `all`.
fn index_of<T: PartialEq>(all: &[T], v: &T) -> usize {
    all.iter().position(|x| x == v).unwrap_or(0)
}

impl PipelineKey {
    /// Its line in the list warm_pipelines keeps: fifteen numbers.
    fn line(&self) -> String {
        let (b, v) = (&self.blend, &self.variant);
        format!(
            "{} {} {} {} {} {} {} {} {} {} {} {} {} {} {}\n",
            index_of(&FORMATS, &self.format),
            b.enabled as i32,
            index_of(&FACTORS, &b.src_color),
            index_of(&FACTORS, &b.dst_color),
            index_of(&FACTORS, &b.src_alpha),
            index_of(&FACTORS, &b.dst_alpha),
            index_of(&OPS, &b.color_op),
            index_of(&OPS, &b.alpha_op),
            self.mesh as i32,
            v.kinds,
            v.paints,
            v.flags,
            v.clipped as i32,
            v.filtered as i32,
            self.how
        )
    }

    /// The keys a list holds, each once, in its order (what isn't one passed over: a list cut short).
    fn parse_list(text: &str) -> Vec<PipelineKey> {
        let numbers: Vec<i64> = text.split_whitespace().map_while(|w| w.parse().ok()).collect();
        let mut keys = Vec::new();
        for n in numbers.as_chunks::<15>().0 {
            let factor = |i: i64| FACTORS.get(i as usize).copied();
            let op = |i: i64| OPS.get(i as usize).copied();
            let (Some(format), Some(src_color), Some(dst_color), Some(src_alpha), Some(dst_alpha)) =
                (FORMATS.get(n[0] as usize).copied(), factor(n[2]), factor(n[3]), factor(n[4]), factor(n[5]))
            else {
                continue;
            };
            let (Some(color_op), Some(alpha_op)) = (op(n[6]), op(n[7])) else { continue };
            let key = PipelineKey {
                format,
                blend: BlendState {
                    enabled: n[1] != 0,
                    src_color,
                    dst_color,
                    src_alpha,
                    dst_alpha,
                    color_op,
                    alpha_op,
                },
                mesh: n[8] != 0,
                variant: ShaderVariant {
                    kinds: n[9] as u16,
                    paints: n[10] as u8,
                    flags: n[11] as u8,
                    clipped: n[12] != 0,
                    filtered: n[13] != 0,
                },
                how: n[14] as i32,
            };
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        keys
    }

    /// The pipeline's description, made for its variant (the specialization constants in primitives.slang's order).
    fn desc(&self) -> PipelineDesc<'static> {
        let mut desc = PipelineDesc::new(if self.mesh { &MESH_VERTEX } else { &VERTEX }, &FRAGMENT);
        desc.instance_attributes =
            (0..12).map(|i| VertexAttribute { location: i, format: VertexFormat::Float4, offset: i * 16 }).collect();
        desc.instance_stride = Prim::SIZE as u32;
        desc.textures = 6;
        desc.texel_textures = 1 << 2; // (the table)
        desc.blend = self.blend;
        desc.target = self.format;
        let v = &self.variant;
        desc.constants =
            vec![v.kinds as i32, v.paints as i32, v.clipped as i32, v.filtered as i32, self.how, v.flags as i32];
        desc
    }
}

/// The pipeline for `key`, made with `create`; where the driver can't make its variant (as an Adreno 6xx's compiler
/// couldn't one), the whole shader.
fn make_pipeline(key: &PipelineKey, mut create: impl FnMut(&PipelineDesc<'_>) -> Option<Pipeline>) -> Option<Pipeline> {
    let mut desc = key.desc();
    create(&desc).or_else(|| {
        desc.constants.clear();
        create(&desc)
    })
}

/// The pipelines made (none: the device couldn't), those still to make on the warming thread, the one it is making,
/// and the list file with what is in it.
#[derive(Default)]
struct Made {
    pipelines: Vec<(PipelineKey, Option<Pipeline>)>,
    warming: VecDeque<PipelineKey>,
    warming_now: Option<PipelineKey>,
    list: Option<PathBuf>,
    listed: Vec<PipelineKey>,
}

/// The renderer's pipelines, shared with the thread that warms them.
#[derive(Default)]
pub(super) struct Pipelines {
    made: Arc<(Mutex<Made>, Condvar)>,
    warmer: Option<JoinHandle<()>>,
}

fn lock(made: &Mutex<Made>) -> MutexGuard<'_, Made> {
    made.lock().unwrap_or_else(|e| e.into_inner())
}

impl Renderer {
    /// The pipelines this app's renderer made before, as `list` keeps them (made, then written there as each is
    /// first made): made again on a thread of their own, in the order they were first needed, where the device can
    /// (its pipeline factory), so that a frame needing one takes it made. (A driver that keeps none in its pipeline
    /// cache, or a new shader after an update, made each in a frame: 8 to 20 ms on a Redmi Note 9 Pro, each.)
    pub fn warm_pipelines(&mut self, list: &Path) {
        let keys = PipelineKey::parse_list(&std::fs::read_to_string(list).unwrap_or_default());
        let shared = self.pipelines.made.clone();
        let mut made = lock(&shared.0);
        made.list = Some(list.to_path_buf());
        made.listed = keys.clone();
        let Some(factory) = self.device.pipeline_factory() else { return };
        if keys.is_empty() || self.pipelines.warmer.is_some() {
            return;
        }
        made.warming = keys.into();
        drop(made);
        self.pipelines.warmer = Some(std::thread::spawn(move || {
            loop {
                let key = {
                    let mut made = lock(&shared.0);
                    let Some(key) = made.warming.pop_front() else { return };
                    made.warming_now = Some(key);
                    key
                };
                let pipeline = make_pipeline(&key, |desc| factory.create_pipeline(desc));
                let mut made = lock(&shared.0);
                made.pipelines.push((key, pipeline));
                made.warming_now = None;
                drop(made);
                shared.1.notify_all();
            }
        }));
    }

    /// The warming thread stopped (what it had still to make left), before the renderer goes.
    pub(super) fn stop_warming(&mut self) {
        lock(&self.pipelines.made.0).warming.clear();
        if let Some(warmer) = self.pipelines.warmer.take() {
            let _ = warmer.join();
        }
    }

    /// The pipeline for a batch (made once); none if this device can't make one.
    fn pipeline(&mut self, key: PipelineKey) -> Option<PipelineId> {
        let shared = self.pipelines.made.clone();
        {
            let mut made = lock(&shared.0);
            loop {
                if let Some((_, p)) = made.pipelines.iter().find(|(k, _)| *k == key) {
                    return p.as_ref().map(Pipeline::id);
                }
                if made.warming_now != Some(key) {
                    break;
                }
                made = shared.1.wait(made).unwrap_or_else(|e| e.into_inner()); // being made on the warming thread
            }
            made.warming.retain(|k| *k != key); // (still to make there: made here, now)
        }
        let pipeline = make_pipeline(&key, |desc| self.device.create_pipeline(desc));
        let mut made = lock(&shared.0);
        if let Some(list) = made.list.clone()
            && !made.listed.contains(&key)
        {
            // (Kept for the next run's warm_pipelines.)
            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(list) {
                let _ = file.write_all(key.line().as_bytes());
            }
            made.listed.push(key);
        }
        let id = pipeline.as_ref().map(Pipeline::id);
        made.pipelines.push((key, pipeline));
        id
    }

    /// The coverage pipeline (shaders/path.slang): each edge's area added into an R16F mask.
    fn coverage(&mut self) -> Option<PipelineId> {
        if self.coverage_pipeline.is_none() {
            let mut desc = PipelineDesc::new(&COVERAGE_VERTEX, &COVERAGE_FRAGMENT);
            desc.instance_attributes =
                (0..3).map(|i| VertexAttribute { location: i, format: VertexFormat::Float4, offset: i * 16 }).collect();
            desc.instance_stride = EdgeInstance::SIZE as u32;
            desc.blend = BlendState::factors(BlendFactor::One, BlendFactor::One, BlendFactor::One, BlendFactor::One);
            desc.target = Format::R16f;
            self.coverage_pipeline = self.device.create_pipeline(&desc);
        }
        self.coverage_pipeline.as_ref().map(Pipeline::id)
    }

    // ---- Submitting ------------------------------------------------------------------------------------------------

    /// Submits what the targets ended so far recorded (glyphs and images uploaded first).
    pub fn flush(&mut self) {
        self.draw_waiting_bitmaps(); // (what was drawn into bitmaps while the frame was drawn: see RendererBitmaps)
        if self.order.is_empty() {
            return;
        }
        // What changed on the atlas since the last flush, then the passes.
        self.upload_atlas();
        if !self.white_uploaded {
            self.device.upload(self.white.id(), IRect::new(0, 0, 1, 1), &[255; 4], 4);
            self.white_uploaded = true;
        }
        // The table's rows recorded since the last flush (all of them into a bigger table).
        let rows = self.table_rows();
        if rows > self.stops_uploaded {
            if self.stops_texture.as_ref().is_none_or(|t| t.height() < rows) {
                let height = 64.max((rows + 63) / 64 * 64);
                self.stops_texture =
                    self.device.create_texture(STOPS_WIDTH as i32, height, Format::Rgba32f, TextureUsage::SAMPLED);
                self.stops_uploaded = 0;
            }
            if let Some(table) = self.stops_texture.as_ref().map(|t| t.id()) {
                let from = self.stops_uploaded as usize * STOPS_WIDTH * 4;
                let bytes: Vec<u8> = self.stops[from..].iter().flat_map(|v| v.to_ne_bytes()).collect();
                let region = IRect::new(0, self.stops_uploaded, STOPS_WIDTH as i32, rows - self.stops_uploaded);
                self.device.upload(table, region, &bytes, STOPS_WIDTH * 16);
                self.stops_uploaded = rows;
            }
        }
        self.draw_masks();
        for index in std::mem::take(&mut self.order) {
            self.execute(index);
        }
        // Passes still being drawn into keep their places (and the shelves theirs); the others go.
        if self.targets.is_empty() {
            self.passes.clear();
            self.shelf = super::MaskShelf::default();
        }
    }

    /// The coverage of the paths of the passes about to run.
    fn draw_masks(&mut self) {
        let y_down = self.device.clip_y_down();
        // The paths new to the kept ones, into their spots (clear: the mask is cleared only when it starts over).
        if !self.path_edges.is_empty()
            && let Some(mask) = self.path_mask.as_ref().map(|t| t.id())
            && let Some(pipeline) = self.coverage()
        {
            let buffer = self.device.write_frame_data(&bytes_of(&self.path_edges), 16);
            let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, 0, 0);
            for e in &self.path_edges {
                x0 = x0.min(e.region[0] as i32);
                y0 = y0.min(e.region[1] as i32);
                x1 = x1.max(e.region[2] as i32);
                y1 = y1.max(e.region[3] as i32);
            }
            let uniforms = PathUniforms { to_clip: to_clip(mask.width() as f32, mask.height() as f32, y_down) };
            let area = if self.path_mask_clear { mask.bounds() } else { IRect::new(x0, y0, x1 - x0, y1 - y0) };
            self.device.begin_pass(mask, self.path_mask_clear.then_some(ClearColor::TRANSPARENT), &[], Some(area));
            self.device.set_pipeline(pipeline);
            self.device.set_uniforms(&bytes_of(&[uniforms]));
            self.device.set_scissor(area);
            self.device.draw(buffer, QUAD_VERTICES, self.path_edges.len() as u32);
            self.device.end_pass();
            self.path_edges.clear();
            self.path_mask_clear = false;
        }
        for g in 0..=self.shelf.generation {
            // This generation's paths in the passes going now, cleared where they are first (and the texel around
            // each: sampled linearly, a layer's cut reads it).
            let mut edges = Vec::new();
            let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, 0, 0);
            for &index in &self.order {
                let pass = &self.passes[index];
                for (e, &generation) in pass.edges.iter().zip(&pass.edge_generations) {
                    if generation == g {
                        let r = &e.region;
                        x0 = x0.min(r[0] as i32 - 1);
                        y0 = y0.min(r[1] as i32 - 1);
                        x1 = x1.max(r[2] as i32 + 1);
                        y1 = y1.max(r[3] as i32 + 1);
                        edges.push(*e);
                    }
                }
            }
            if edges.is_empty() {
                continue;
            }
            let g = g as usize;
            if self.masks.len() <= g {
                self.masks.resize_with(g + 1, || None);
            }
            let (want_w, want_h) = (self.shelf.width, self.shelf.height);
            let small = self.masks[g].as_ref().is_none_or(|m| m.width() < want_w || m.height() < want_h);
            if small {
                let (w, h) = self.masks[g].as_ref().map_or((0, 0), |m| (m.width(), m.height()));
                self.masks[g] = self.device.create_texture(
                    want_w.max(w),
                    want_h.max(h),
                    Format::R16f,
                    TextureUsage::SAMPLED | TextureUsage::RENDER_TARGET,
                );
            }
            let Some(mask) = self.masks[g].as_ref().map(|t| t.id()) else { continue };
            let Some(pipeline) = self.coverage() else { continue };
            let buffer = self.device.write_frame_data(&bytes_of(&edges), 16);
            let uniforms = PathUniforms { to_clip: to_clip(mask.width() as f32, mask.height() as f32, y_down) };
            let (ax, ay) = (x0.max(0), y0.max(0));
            let area = IRect::new(ax, ay, x1 - ax, y1 - ay);
            self.device.begin_pass(mask, Some(ClearColor::TRANSPARENT), &[], Some(area));
            self.device.set_pipeline(pipeline);
            self.device.set_uniforms(&bytes_of(&[uniforms]));
            self.device.set_scissor(area);
            self.device.draw(buffer, QUAD_VERTICES, edges.len() as u32);
            self.device.end_pass();
        }
    }

    /// The effects' pipeline (shaders/effects.slang).
    fn effect_pipeline(&mut self, target: Format) -> Option<PipelineId> {
        if self.effect_pipeline.is_none() {
            let mut desc = PipelineDesc::new(&EFFECT_VERTEX, &EFFECT_FRAGMENT);
            desc.instance_attributes = vec![VertexAttribute { location: 0, format: VertexFormat::Float4, offset: 0 }];
            desc.instance_stride = 16;
            desc.textures = 2;
            desc.texel_textures = 1 << 1; // (the table: a progressive blur's stops, or its mask)
            desc.blend = BlendState::OFF;
            desc.target = target;
            self.effect_pipeline = self.device.create_pipeline(&desc);
        }
        self.effect_pipeline.as_ref().map(Pipeline::id)
    }

    /// The stretch pipeline (shaders/stretch.slang): an effect pass's instance, its one texture.
    fn stretch_pipeline(&mut self, target: Format) -> Option<PipelineId> {
        if self.stretch_pipeline.is_none() {
            let mut desc = PipelineDesc::new(&STRETCH_VERTEX, &STRETCH_FRAGMENT);
            desc.instance_attributes = vec![VertexAttribute { location: 0, format: VertexFormat::Float4, offset: 0 }];
            desc.instance_stride = 16;
            desc.textures = 1;
            desc.blend = BlendState::OFF;
            desc.target = target;
            self.stretch_pipeline = self.device.create_pipeline(&desc);
        }
        self.stretch_pipeline.as_ref().map(Pipeline::id)
    }

    /// A stretched layer's pass: `rect` of `target` made from its content (`source`, filtered linearly).
    fn stretch(&mut self, target: TextureId, e: &EffectDraw) {
        let Some(pipeline) = self.stretch_pipeline(target.format()) else { return };
        let rect =
            [e.rect.x as f32, e.rect.y as f32, (e.rect.x + e.rect.width) as f32, (e.rect.y + e.rect.height) as f32];
        let instance: Vec<u8> = rect.iter().flat_map(|v| v.to_ne_bytes()).collect();
        let buffer = self.device.write_frame_data(&instance, 16);
        let mut uniforms = e.uniforms;
        uniforms[..4].copy_from_slice(&to_clip(
            target.width() as f32,
            target.height() as f32,
            self.device.clip_y_down(),
        ));
        let bytes: Vec<u8> = uniforms.iter().flat_map(|v| v.to_ne_bytes()).collect();
        self.device.begin_pass(target, Some(ClearColor::TRANSPARENT), &[e.source], None);
        self.device.set_pipeline(pipeline);
        self.device.set_uniforms(&bytes);
        self.device.set_texture(0, e.source, Sampler { filter: Filter::Linear, ..Sampler::default() });
        self.device.set_scissor(e.rect);
        self.device.draw(buffer, QUAD_VERTICES, 1);
        self.device.end_pass();
    }

    /// Runs a pass: a copy, an effect's (a stretch's), or its batches (the instances all at once; then each mask generation: the
    /// batches that read it, the target cleared only before the first).
    fn execute(&mut self, index: usize) {
        let pass = &self.passes[index];
        let (target, clear) = (pass.target, pass.clear);
        if let Some(copy) = pass.copy {
            self.device.copy_texture(copy.from, copy.region, target, copy.x, copy.y);
            return;
        }
        if let Some(e) = pass.effect
            && e.stretch
        {
            return self.stretch(target, &e);
        }
        if let Some(e) = pass.effect {
            let Some(pipeline) = self.effect_pipeline(target.format()) else { return };
            let rect =
                [e.rect.x as f32, e.rect.y as f32, (e.rect.x + e.rect.width) as f32, (e.rect.y + e.rect.height) as f32];
            let instance: Vec<u8> = rect.iter().flat_map(|v| v.to_ne_bytes()).collect();
            let buffer = self.device.write_frame_data(&instance, 16);
            let mut uniforms = e.uniforms;
            uniforms[..4].copy_from_slice(&to_clip(
                target.width() as f32,
                target.height() as f32,
                self.device.clip_y_down(),
            ));
            let table = match (e.mask, e.table, &self.stops_texture) {
                (Some(mask), _, _) => mask,
                (None, true, Some(t)) => t.id(),
                _ => self.white.id(),
            };
            uniforms[30] = 1.0 / table.width() as f32;
            uniforms[31] = 1.0 / table.height() as f32;
            let bytes: Vec<u8> = uniforms.iter().flat_map(|v| v.to_ne_bytes()).collect();
            let filter = if e.linear { Filter::Linear } else { Filter::Nearest };
            self.device.begin_pass(target, Some(ClearColor::TRANSPARENT), &[e.source, table], None);
            self.device.set_pipeline(pipeline);
            self.device.set_uniforms(&bytes);
            self.device.set_texture(0, e.source, Sampler { filter, ..Sampler::default() });
            self.device.set_texture(1, table, Sampler { filter: Filter::Nearest, ..Sampler::default() });
            self.device.set_scissor(e.rect);
            self.device.draw(buffer, QUAD_VERTICES, 1);
            self.device.end_pass();
            return;
        }
        if pass.batches.is_empty() && clear.is_none() {
            return; // (nothing drawn: what it holds stays)
        }
        let pass = &mut self.passes[index];
        let prims = std::mem::take(&mut pass.prims);
        let batches = std::mem::take(&mut pass.batches);
        let masked = !pass.edges.is_empty();
        pass.edges.clear();
        pass.edge_generations.clear();
        let instances = if prims.is_empty() { None } else { self.device.write_frame_data(&bytes_of(&prims), 16) };
        let (w, h) = (target.width() as f32, target.height() as f32);
        let y_down = self.device.clip_y_down();
        let white = self.white.id();
        let table = self.stops_texture.as_ref().map(|t| t.id());
        let nearest = Sampler { filter: Filter::Nearest, ..Sampler::default() };
        // A pass in each generation its batches are in (each reads that generation's mask).
        let first_generation = batches.first().map_or(0, |b| b.generation);
        let last_generation = batches.last().map_or(0, |b| b.generation);
        let mut batch = 0;
        for g in first_generation..=last_generation {
            let mask = match self.masks.get(g as usize) {
                Some(Some(m)) if masked => m.id(),
                _ => white,
            };
            let first = g == first_generation;
            let mut reads: Vec<TextureId> = Vec::new();
            let first_batch = batch;
            while batch < batches.len() && batches[batch].generation == g {
                batch += 1;
            }
            let mut reads_target = false;
            for b in &batches[first_batch..batch] {
                let t0 = if b.mask == Mask::Texture0 { mask } else { b.texture };
                let t1 = if b.mask == Mask::Texture1 { mask } else { b.texture1 };
                for t in [t0, t1, b.texture5] {
                    if !reads.contains(&t) {
                        reads.push(t);
                    }
                }
                reads_target |= plan_for(b.blend, b.opaque).reads_target;
            }
            if let Some(t) = table {
                reads.push(t);
            }
            // (The clips' paths are read from the coverage mask by any primitive in them.)
            if masked && !reads.contains(&mask) {
                reads.push(mask);
            }
            let dst_copy = if reads_target { self.dst_copy_for(target) } else { None };
            if let Some(copy) = dst_copy {
                reads.push(copy);
            }
            // Only where its batches draw, kept around it (a tiling GPU then loads and stores no more): all of the
            // target where it is cleared.
            let mut drawn: Option<IRect> = None;
            if !(first && clear.is_some()) {
                for b in &batches[first_batch..batch] {
                    let s = b.scissor;
                    drawn = Some(match drawn {
                        None => s,
                        Some(d) => {
                            let (x0, y0) = (d.x.min(s.x), d.y.min(s.y));
                            IRect::new(
                                x0,
                                y0,
                                (d.x + d.width).max(s.x + s.width) - x0,
                                (d.y + d.height).max(s.y + s.height) - y0,
                            )
                        }
                    });
                }
            }
            self.device.begin_pass(target, if first { clear } else { None }, &reads, drawn);
            let mut uniforms = PrimitiveUniforms {
                to_clip: to_clip(w, h, y_down),
                texels: [
                    1.0 / mask.width() as f32,
                    1.0 / mask.height() as f32,
                    table.map_or(0.0, |t| 1.0 / t.width() as f32), // (the table's width: its texels are fetched)
                    0.0,
                ],
                // A texel of the target's copy (which may be bigger than the target: one copy for all).
                blend: [
                    0.0,
                    0.0,
                    dst_copy.map_or(0.0, |c| 1.0 / c.width() as f32),
                    dst_copy.map_or(0.0, |c| 1.0 / c.height() as f32),
                ],
            };
            let all = target.bounds();
            for b in &batches[first_batch..batch] {
                let plan = plan_for(b.blend, b.opaque);
                uniforms.blend[0] = super::paint::blend_index(b.blend) as f32;
                uniforms.blend[1] = plan.how as f32;
                let key = PipelineKey {
                    format: target.format(),
                    blend: plan.state,
                    mesh: b.mesh,
                    variant: b.variant,
                    how: plan.how,
                };
                let Some(pipeline) = self.pipeline(key) else { continue }; // (none for it on this device: logged when it failed)
                // (Again in each pass the batch is drawn in: a pass starts with nothing bound.)
                let bound = Bound {
                    pipeline,
                    uniforms: bytes_of(&[uniforms]),
                    mask,
                    table: table.unwrap_or(white),
                    dst_copy: dst_copy.unwrap_or(white),
                };
                self.bind(&bound, b, nearest);
                let corners = if b.mesh { MESH_VERTICES } else { QUAD_VERTICES };
                let Some(instances) = instances else { continue };
                let at = |k: u32| {
                    Some(BufferRef::new(instances.buffer(), instances.offset() + k as u64 * Prim::SIZE as u64))
                };
                if !plan.reads_target {
                    self.device.set_scissor(b.scissor);
                    self.device.draw(at(b.first), corners, b.count);
                    continue;
                }
                // Each one on what is there just before it: the pass ends, the pixels under it are copied, the pass
                // goes on.
                for k in b.first..b.first + b.count {
                    let under = under(&prims[k as usize], b);
                    let under = super::intersect(super::intersect(under, b.scissor), all);
                    if under.is_empty() {
                        continue;
                    }
                    let Some(copy) = dst_copy else { continue };
                    self.device.end_pass();
                    self.device.copy_texture(target, under, copy, under.x, under.y);
                    self.device.begin_pass(target, None, &reads, drawn); // (the batches after it draw in it too)
                    self.bind(&bound, b, nearest);
                    self.device.set_scissor(under);
                    self.device.draw(at(k), corners, 1);
                }
            }
            self.device.end_pass();
        }
    }

    /// The copy of the target's pixels a blend mode that reads them reads, as big as the biggest target so far (of
    /// this one's format).
    fn dst_copy_for(&mut self, target: TextureId) -> Option<TextureId> {
        let fits = self.dst_copy.as_ref().is_some_and(|c| {
            c.width() >= target.width() && c.height() >= target.height() && c.format() == target.format()
        });
        if !fits {
            let (w, h) = self.dst_copy.as_ref().map_or((0, 0), |c| (c.width(), c.height()));
            self.dst_copy = self.device.create_texture(
                target.width().max(w),
                target.height().max(h),
                target.format(),
                TextureUsage::SAMPLED,
            );
        }
        self.dst_copy.as_ref().map(|c| c.id())
    }

    /// A batch's pipeline, uniforms and textures, in the pass begun.
    fn bind(&mut self, bound: &Bound, b: &Batch, nearest: Sampler) {
        let d = &mut self.device;
        d.set_pipeline(bound.pipeline);
        d.set_uniforms(&bound.uniforms);
        if b.mask == Mask::Texture0 {
            d.set_texture(0, bound.mask, nearest);
        } else {
            d.set_texture(0, b.texture, b.sampler);
        }
        if b.mask == Mask::Texture1 {
            d.set_texture(1, bound.mask, Sampler::default()); // (a layer's cut, which may be turned: filtered)
        } else {
            d.set_texture(1, b.texture1, b.sampler1);
        }
        d.set_texture(2, bound.table, nearest);
        d.set_texture(3, bound.dst_copy, nearest);
        d.set_texture(4, bound.mask, nearest);
        d.set_texture(5, b.texture5, b.sampler5);
    }
}

/// What a pass binds for each batch besides the batch's own textures.
struct Bound {
    pipeline: PipelineId,
    uniforms: Vec<u8>,
    mask: TextureId,
    table: TextureId,
    dst_copy: TextureId,
}

/// The pixels a primitive may draw on, where it is (part of it behind the eye: all of the batch's scissor).
fn under(q: &Prim, b: &Batch) -> IRect {
    let place = Projective {
        m: [
            q.xform[0],
            q.xform[2],
            q.extra[0],
            q.xform[1],
            q.xform[3],
            q.extra[1],
            q.projection[0],
            q.projection[1],
            q.projection[2],
        ],
    };
    let (mut x0, mut y0, mut x1, mut y1) = (1e30f32, 1e30f32, -1e30f32, -1e30f32);
    let mut behind = false;
    for c in 0..(if b.mesh { 3 } else { 4 }) {
        let (d, w) = if b.mesh {
            // (A triangle's vertices, where they are: x, y and w.)
            let xyw = [
                [q.bounds[0], q.bounds[1], q.bounds[2]],
                [q.bounds[3], q.shape[0], q.shape[1]],
                [q.shape[2], q.shape[3], q.radii[0]],
            ][c];
            (kompoz_graphics::Offset::new(xyw[0] / xyw[2], xyw[1] / xyw[2]), xyw[2])
        } else {
            place.apply_w(q.bounds[if c & 1 != 0 { 2 } else { 0 }], q.bounds[if c & 2 != 0 { 3 } else { 1 }])
        };
        behind |= w <= 0.0;
        (x0, y0, x1, y1) = (x0.min(d.x), y0.min(d.y), x1.max(d.x), y1.max(d.y));
    }
    if behind {
        return b.scissor;
    }
    let (fx, fy) = (x0.floor() as i32, y0.floor() as i32);
    IRect::new(fx, fy, x1.ceil() as i32 - fx, y1.ceil() as i32 - fy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_modes_go_through_the_blend_unit_where_it_can() {
        // Fixed function where the coverage multiplied in mixes exactly; the shader's tricks for three; the rest
        // read the target.
        for mode in [BlendMode::SrcOver, BlendMode::DstOver, BlendMode::SrcAtop, BlendMode::Xor, BlendMode::Screen] {
            let plan = plan_for(mode, false);
            assert!(plan.state.enabled && plan.how == 0 && !plan.reads_target, "{mode:?}");
        }
        assert_eq!(plan_for(BlendMode::Modulate, false).how, 1);
        assert_eq!(plan_for(BlendMode::DstIn, false).how, 2);
        assert_eq!(plan_for(BlendMode::Clear, false).how, 3);
        for mode in [BlendMode::Src, BlendMode::SrcIn, BlendMode::Multiply, BlendMode::Hue] {
            assert_eq!(plan_for(mode, false), BlendPlan { state: BlendState::OFF, how: 4, reads_target: true });
        }
        // Opaque: written as it is.
        assert_eq!(
            plan_for(BlendMode::Multiply, true),
            BlendPlan { state: BlendState::OFF, how: 0, reads_target: false }
        );
    }

    #[test]
    fn pipelines_are_listed_as_they_are_read_back() {
        let a = PipelineKey {
            format: Format::Rgba16f,
            blend: BlendState::factors(
                BlendFactor::Zero,
                BlendFactor::SrcColor,
                BlendFactor::Zero,
                BlendFactor::SrcAlpha,
            ),
            mesh: true,
            variant: ShaderVariant { kinds: 0x401, paints: 0x3f, flags: 48, clipped: true, filtered: false },
            how: 1,
        };
        let b = PipelineKey {
            format: Format::Rgba8,
            blend: BlendState::OFF,
            mesh: false,
            variant: ShaderVariant::default(),
            how: 4,
        };
        let list = format!("{}{}{}", a.line(), b.line(), a.line());
        assert_eq!(a.line(), "4 1 0 6 0 2 0 0 1 1025 63 48 1 0 1\n");
        assert_eq!(PipelineKey::parse_list(&list), [a, b]);
        // Cut short, or not a list: what is whole of it.
        assert_eq!(PipelineKey::parse_list(&list[..list.len() - 4]), [a, b]);
        assert_eq!(PipelineKey::parse_list("not a list"), []);
        // Made for its variant: the constants in the shader's order.
        assert_eq!(a.desc().constants, [0x401, 0x3f, 1, 0, 1, 48]);
        assert_eq!((a.desc().instance_stride, a.desc().instance_attributes.len()), (192, 12));
    }
}
