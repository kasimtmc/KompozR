//! The GPU renderer (C++ Kompoz's platform/gpu/renderer): kompoz-ui's GraphicsContext drawn with the device
//! layer on any of its backends. Every primitive is a quad whose pixels the fragment shader covers exactly from the
//! shape's distance (shaders/primitives.slang); paths are covered by their edges' area in a coverage mask
//! (shaders/path.slang); text comes from the glyph atlas.
//!
//! Drawing is recorded, then submitted by [`flush`](Renderer::flush): each target (the kept frame, the swapchain
//! image, a layer's texture) gets a pass of batched quads, and a layer's pass runs before the one that composites
//! it.
//!
//! ```text
//! renderer.begin_target(frame, None);     // draws into `frame` until end_target()
//! host.draw(&mut renderer);
//! renderer.end_target();
//! renderer.flush();
//! ```
//!
//! Lengths are px, as kompoz-ui's; within a layer the renderer works in the layer's texels (its resolution: a
//! transform that enlarges it keeps it sharp), and so in a bake.
//!
//! Its parts, as C++'s renderer.cpp has them: this module (the state, targets, `add` and the scissor), `submit`
//! (flush, the passes, the coverage masks, pipelines and their warming), `prims` (rounded rects, lines, arcs,
//! shadows), `paint` (colors and gradients, the frame's table), `clip`, `path` (coverage masks, kept paths), `layer`
//! (layers, their pool, kept layers), `effect`, `shadow`, `image`, `shader` (image shaders and bakes), `mesh`,
//! `text`, `frame` (frames kept and moved, blits), and `context` (the GraphicsContext the app draws through, each
//! call handed to its part).

#![deny(unsafe_code)]

mod clip;
mod context;
mod effect;
mod frame;
mod image;
mod layer;
mod mesh;
mod paint;
mod path;
mod prims;
mod shader;
mod shadow;
mod submit;
mod text;

pub use image::RendererBitmaps;

use crate::device::{ClearColor, Device, IRect, Pipeline, Sampler, Texture, TextureId, TextureUsage};
use crate::path::Edge;
use crate::shaders::{EdgeInstance, GLYPH, IMAGE, LAYER, MESH, PATH, Prim};
use kompoz_graphics::{BlendMode, ColorFilter, Contour, Projective, Rect};
use kompoz_ui::graphics_context::ShadowLight;
use std::collections::HashMap;
use std::ops::BitOrAssign;
use std::rc::Rc;

/// Px: a bigger layer is drawn straight into its target (C++'s kMaxLayer).
const MAX_LAYER: i32 = 4096;
/// A generation's coverage mask is at least this big each way (px), and at least its target's size.
const MASK_SIZE: i32 = 2048;
/// The kept paths' mask each way (px).
const KEPT_MASK: i32 = 2048;
/// The most a kept path covers: 512 x 512 px, and half the kept mask each way.
const KEPT_MOST: i32 = 512;
/// The table's width in texels (gradients' stops, clips, color filters, shaders' maps: a row each): 128 stops.
const STOPS_WIDTH: usize = 256;

/// Where a batch's textures come from: its own, or the coverage mask (made when the pass runs).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Mask {
    #[default]
    None,
    Texture0,
    Texture1,
}

/// What a batch's primitives need of the fragment shader (its specialization constants: see
/// shaders/primitives.slang): their kinds and paints (bits), whether they are clipped or filtered, the flags they
/// set. A pipeline is made for each one met (and blend mode).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
struct ShaderVariant {
    kinds: u16,
    paints: u8,
    flags: u8,
    clipped: bool,
    filtered: bool,
}

impl BitOrAssign for ShaderVariant {
    fn bitor_assign(&mut self, o: ShaderVariant) {
        self.kinds |= o.kinds;
        self.paints |= o.paints;
        self.flags |= o.flags;
        self.clipped |= o.clipped;
        self.filtered |= o.filtered;
    }
}

/// Primitives drawn together: the same textures and samplers, blend mode, scissor and mask generation.
#[derive(Clone, Copy, Debug)]
struct Batch {
    texture: TextureId,
    texture1: TextureId,
    mask: Mask,
    /// Written as it is, not blended.
    opaque: bool,
    blend: BlendMode,
    scissor: IRect,
    /// The pass's masks it reads (see Pass).
    generation: u32,
    first: u32,
    count: u32,
    /// texture's (an image's, as its FilterQuality says).
    sampler: Sampler,
    /// texture1's (an image shader's, as its tile modes say).
    sampler1: Sampler,
    /// Its primitives a mesh's triangles (three vertices each, by vsMesh).
    mesh: bool,
    /// A composite paint's source image (white if none).
    texture5: TextureId,
    sampler5: Sampler,
    /// Its primitives' together.
    variant: ShaderVariant,
}

/// The paths of the passes recorded until a flush with none still open are rasterized into one coverage mask, all
/// of them in one go before the passes run (a mask pass between one pass and the next held a tiling GPU up at each).
/// When it is full a new generation starts, in a mask of its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct MaskShelf {
    /// What a generation has.
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    row: i32,
    generation: u32,
}

/// A pass of an effect (shaders/effects.slang): `rect` of the target made from `source` (filtered linearly, or
/// not), with this frame's table if it reads stops, or a mask of its own in the table's place.
#[derive(Clone, Copy, Debug)]
struct EffectDraw {
    source: TextureId,
    rect: IRect,
    uniforms: [f32; 32],
    linear: bool,
    table: bool,
    mask: Option<TextureId>,
}

/// Part of a texture copied into a pass's target, at (x, y), instead of drawing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct PassCopy {
    from: TextureId,
    region: IRect,
    x: i32,
    y: i32,
}

/// What is drawn into a target in one go: its primitives batched, its paths' edges (rasterized into the coverage
/// mask first); or an effect's pass, or a copy, instead.
#[derive(Debug)]
struct Pass {
    target: TextureId,
    clear: Option<ClearColor>,
    prims: Vec<Prim>,
    batches: Vec<Batch>,
    edges: Vec<EdgeInstance>,
    /// The generation each edge is in.
    edge_generations: Vec<u32>,
    /// Set: this instead of the rest.
    effect: Option<EffectDraw>,
    /// Set: that part of that texture copied into the target, instead.
    copy: Option<PassCopy>,
}

impl Pass {
    fn new(target: TextureId, clear: Option<ClearColor>) -> Pass {
        Pass {
            target,
            clear,
            prims: Vec::new(),
            batches: Vec::new(),
            edges: Vec::new(),
            edge_generations: Vec::new(),
            effect: None,
            copy: None,
        }
    }
}

/// A clip in effect: the pixels it leaves (whole ones), and the clips that cut them finer (a rect turned, say), if
/// any: their chain.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Clip {
    scissor: IRect,
    chain: Option<usize>,
}

/// What a primitive's coverage is cut to (or to what is outside it): a rounded rect in its own space, which
/// `to_local` maps the target's pixels to; or a path, its polygons in the target's pixels, filled by the non-zero or
/// even-odd rule, rasterized into the pass's coverage mask within `region` (the pixels they reach, within the clips
/// around).
#[derive(Clone, Debug, Default)]
struct ClipElement {
    rect: [f32; 4],
    radii: [f32; 4],
    to_local: Projective,
    outside: bool,
    /// A path's (shared by the chains inside it).
    polygons: Option<Rc<Vec<Contour>>>,
    even_odd: bool,
    region: IRect,
}

/// The clips in effect, outermost first, and their row in this frame's table once a primitive in them is drawn (a
/// chain with paths gets a row for each mask generation it is drawn in).
#[derive(Clone, Debug, Default)]
struct ClipChain {
    elements: Vec<ClipElement>,
    paths: bool,
    row: Option<i32>,
    /// The row's.
    generation: u32,
    /// And its pass's (a pass split by a backdrop starts its mask anew).
    pass: Option<usize>,
}

/// What push_translation and push_transform change: the pixel origin, and the transform after it.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Placement {
    ox: f32,
    oy: f32,
    transform: Projective,
}

/// A target being drawn into: its pass, and the drawing state outside it.
#[derive(Debug)]
struct Target {
    pass: usize,
    /// The pixel origin.
    ox: f32,
    oy: f32,
    transform: Projective,
    window_from_target: Projective,
    clips: Vec<Clip>,
    placements: Vec<Placement>,
    blends: Vec<BlendMode>,
    filters: Vec<ColorFilter>,
    anti_alias: Vec<bool>,
}

/// What a primitive samples and how it goes onto the target (C++'s add() arguments past the primitive): its
/// textures (none: white), which of them is the coverage mask, texture's sampler; written as it is (`opaque`), or
/// already in the target's pixels (a path's coverage, a mesh's vertices: the transform not applied again).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
struct Inputs {
    texture: Option<TextureId>,
    texture1: Option<TextureId>,
    mask: Mask,
    sampler: Sampler,
    opaque: bool,
    in_target_pixels: bool,
}

impl Inputs {
    fn texture(texture: TextureId, sampler: Sampler) -> Inputs {
        Inputs { texture: Some(texture), sampler, ..Inputs::default() }
    }
}

/// Image paints' textures this frame (an image shader's, a shader baked), as `paint.z` indexes them.
#[derive(Clone, Copy, PartialEq, Debug)]
struct PaintTexture {
    texture: TextureId,
    sampler: Sampler,
}

/// Paths' coverage kept from frame to frame (draw_path's): a path drawn again as it was, but whole pixels on
/// (scrolled, its screen faded in and out), takes what it covered, laid out and rasterized once. In a mask of its
/// own, filled shelf by shelf; started over (cleared) between frames once full. By the path, its stroke, its
/// placement but for whole pixels.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct KeptPath {
    /// In the mask.
    at: IRect,
    /// The pixels it covers, from its placement's whole pixels.
    bounds: IRect,
    /// The frame it was last drawn in.
    drawn: u64,
}

/// The renderer: [`GraphicsContext`](kompoz_ui::graphics_context::GraphicsContext) drawn on a [`Device`], which it
/// owns (the platform reaches it through [`device_mut`](Renderer::device_mut) for its frames and swapchain).
pub struct Renderer {
    /// Layers pushed (KOMPOZ_PROFILE).
    pub layer_count: i32,
    /// Of them, composited from the pixels they had in a frame before.
    pub kept_layers: i32,
    /// Paths drawn (KOMPOZ_PROFILE).
    pub path_count: i32,
    /// Of them, with the coverage they had in a frame before.
    pub kept_paths: i32,
    /// Textures made for layers and effects (KOMPOZ_PROFILE): none kept unused fitted.
    pub textures_made: i32,
    /// Paths' shadows drawn (KOMPOZ_PROFILE).
    pub path_shadows: i32,
    /// Of them, from the mask they had in a frame before.
    pub kept_shadows: i32,

    /// The target's px a unit of what is drawn: 1, but in a layer (its resolution) and a bake.
    scale: f32,
    /// What elevation shadows are cast from (the platform's, through set_light).
    light: ShadowLight,
    /// What quads that sample nothing bind.
    white: Texture,
    white_uploaded: bool,
    passes: Vec<Pass>,
    /// Ended passes, in the order they run.
    order: Vec<usize>,
    /// The targets being drawn into, innermost last.
    targets: Vec<Target>,
    /// The current target's pixel origin.
    ox: f32,
    oy: f32,
    /// The current target's pixels after the origin: what push_transform makes of them.
    transform: Projective,
    /// Where the current target's pixels are in the window's (the light's).
    window_from_target: Projective,
    clips: Vec<Clip>,
    /// push_translation's and push_transform's.
    placements: Vec<Placement>,
    /// This frame's.
    clip_chains: Vec<ClipChain>,
    /// push_blend_mode's in this target, the innermost last.
    blends: Vec<BlendMode>,
    /// push_anti_alias's in this target.
    anti_alias: Vec<bool>,
    /// push_color_filter's in this target, the innermost last.
    filters: Vec<ColorFilter>,
    /// This frame's, a table row each (after the stops').
    filter_chains: Vec<Vec<ColorFilter>>,
    filter_rows: Vec<i32>,
    /// This frame's table (gradients' stops, clips, filters, shaders' maps), a row of STOPS_WIDTH RGBA32F texels
    /// each, and its texture.
    stops: Vec<f32>,
    stop_brushes: Vec<kompoz_graphics::Brush>,
    /// Each one's row.
    stop_rows: Vec<i32>,
    /// Rows.
    stops_uploaded: i32,
    stops_texture: Option<Texture>,
    paint_textures: Vec<PaintTexture>,
    edge_scratch: Vec<Edge>,
    /// Started over once no pass is open: a clip's row lives no longer than its target.
    shelf: MaskShelf,
    /// A generation's coverage mask each, as big as it needed so far.
    masks: Vec<Option<Texture>>,
    kept_path_map: HashMap<u64, KeptPath>,
    path_mask: Option<Texture>,
    path_shelf: MaskShelf,
    /// The paths new to the kept ones, rasterized before the passes.
    path_edges: Vec<EdgeInstance>,
    /// Full: see end_frame.
    path_mask_full: bool,
    path_mask_clear: bool,
    path_frame: u64,
    /// Of the kept mask, by this frame's paths.
    paths_drawn_area: i64,
    pipelines: submit::Pipelines,
    /// A target's pixels under a draw whose blend mode reads them.
    dst_copy: Option<Texture>,
    /// The coverage pipeline (shaders/path.slang).
    coverage_pipeline: Option<Pipeline>,
    effect_pipeline: Option<Pipeline>,
    layers: layer::LayerState,
    shadows: shadow::ShadowState,
    images: image::ImageState,
    text: text::TextState,
    /// Last: what is above hands its textures back to it as it goes.
    device: Box<dyn Device>,
}

/// `a` and `b`'s common part (empty: nothing there).
fn intersect(a: IRect, b: IRect) -> IRect {
    let (x0, y0) = (a.x.max(b.x), a.y.max(b.y));
    let (x1, y1) = ((a.x + a.width).min(b.x + b.width), (a.y + a.height).min(b.y + b.height));
    IRect::new(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0))
}

/// The whole pixels `r` touches.
fn pixels_of(r: Rect) -> IRect {
    let (x0, y0) = (r.left.floor() as i32, r.top.floor() as i32);
    IRect::new(x0, y0, r.right.ceil() as i32 - x0, r.bottom.ceil() as i32 - y0)
}

impl Renderer {
    /// A renderer drawing on `device`; none if the device can't make the little it needs (a 1x1 texture).
    pub fn new(mut device: Box<dyn Device>) -> Option<Renderer> {
        let white = device.create_texture(1, 1, crate::device::Format::Rgba8, TextureUsage::SAMPLED)?;
        Some(Renderer {
            layer_count: 0,
            kept_layers: 0,
            path_count: 0,
            kept_paths: 0,
            textures_made: 0,
            path_shadows: 0,
            kept_shadows: 0,
            scale: 1.0,
            light: ShadowLight::default(),
            white,
            white_uploaded: false,
            passes: Vec::new(),
            order: Vec::new(),
            targets: Vec::new(),
            ox: 0.0,
            oy: 0.0,
            transform: Projective::Identity,
            window_from_target: Projective::Identity,
            clips: Vec::new(),
            placements: Vec::new(),
            clip_chains: Vec::new(),
            blends: Vec::new(),
            anti_alias: Vec::new(),
            filters: Vec::new(),
            filter_chains: Vec::new(),
            filter_rows: Vec::new(),
            stops: Vec::new(),
            stop_brushes: Vec::new(),
            stop_rows: Vec::new(),
            stops_uploaded: 0,
            stops_texture: None,
            paint_textures: Vec::new(),
            edge_scratch: Vec::new(),
            shelf: MaskShelf::default(),
            masks: Vec::new(),
            kept_path_map: HashMap::new(),
            path_mask: None,
            path_shelf: MaskShelf::default(),
            path_edges: Vec::new(),
            path_mask_full: false,
            path_mask_clear: true,
            path_frame: 1,
            paths_drawn_area: 0,
            pipelines: submit::Pipelines::default(),
            dst_copy: None,
            coverage_pipeline: None,
            effect_pipeline: None,
            layers: layer::LayerState::default(),
            shadows: shadow::ShadowState::default(),
            images: image::ImageState::default(),
            text: text::TextState::default(),
            device,
        })
    }

    /// The device it draws on.
    pub fn device(&self) -> &dyn Device {
        &*self.device
    }

    /// The device it draws on, for the platform's frames (begin_frame, the swapchain's texture, end_frame).
    pub fn device_mut(&mut self) -> &mut dyn Device {
        &mut *self.device
    }

    /// Its shaders' code in eight bytes (`cargo xtask shaders`' versions of them): a pipeline cache kept for other
    /// code holds nothing these can use (and grows with each).
    pub fn shaders_version() -> u64 {
        crate::shaders::VERSION
    }

    /// The light elevation shadows are cast from (the window's: the platform's theme).
    pub fn set_light(&mut self, light: ShadowLight) {
        self.light = light;
    }

    // ---- Targets --------------------------------------------------------------------------------------------------

    /// Draws into `target` until end_target(), cleared first to `clear` if given.
    pub fn begin_target(&mut self, target: TextureId, clear: Option<ClearColor>) {
        if self.targets.is_empty() {
            let area = target.width() as i64 * target.height() as i64;
            self.layers.surface_area = self.layers.surface_area.max(area);
        }
        self.targets.push(Target {
            pass: self.passes.len(),
            ox: self.ox,
            oy: self.oy,
            transform: self.transform,
            window_from_target: self.window_from_target,
            clips: std::mem::take(&mut self.clips),
            placements: std::mem::take(&mut self.placements),
            // (What is drawn into it starts plain: its composite gets the modes and filters outside.)
            blends: std::mem::take(&mut self.blends),
            filters: std::mem::take(&mut self.filters),
            anti_alias: std::mem::take(&mut self.anti_alias),
        });
        self.passes.push(Pass::new(target, clear));
        self.ox = 0.0;
        self.oy = 0.0;
        self.transform = Projective::Identity;
        self.window_from_target = Projective::Identity; // (a layer's: set by push_layer)
    }

    /// Ends the target last begun: what was drawn into it is submitted by the next flush.
    pub fn end_target(&mut self) {
        let Some(t) = self.targets.pop() else { return };
        self.order.push(t.pass);
        self.ox = t.ox;
        self.oy = t.oy;
        self.transform = t.transform;
        self.window_from_target = t.window_from_target;
        self.clips = t.clips;
        self.placements = t.placements;
        self.blends = t.blends;
        self.filters = t.filters;
        self.anti_alias = t.anti_alias;
    }

    /// The pixels the current clip leaves of the current target (none outside a target).
    fn scissor(&self) -> IRect {
        let Some(t) = self.targets.last() else { return IRect::default() };
        let all = self.passes[t.pass].target.bounds();
        match self.clips.last() {
            Some(clip) => intersect(clip.scissor, all),
            None => all,
        }
    }

    /// The current pass (there is a target).
    fn pass_index(&self) -> Option<usize> {
        self.targets.last().map(|t| t.pass)
    }

    /// Not anti-aliased, as push_anti_alias(false) asks.
    fn aliased(&self) -> bool {
        self.anti_alias.last() == Some(&false)
    }

    /// How many times its pixels something around the px point (x, y) is to be rasterized so that the current
    /// transform shows it sharp: 1 when that doesn't scale it, else its largest stretch there, rounded up to an
    /// eighth of an octave (text scaled by an animation is laid out at a few sizes, not anew each frame).
    fn raster_scale(&self, x: f32, y: f32) -> f32 {
        if self.transform.is_identity() {
            return 1.0;
        }
        let s = self.transform.max_scale(x, y);
        if s.is_nan() || s <= 0.0 || (s - 1.0).abs() < 1e-3 {
            return 1.0;
        }
        ((s.log2() * 8.0 - 1e-3).ceil() / 8.0).exp2().clamp(1.0 / 16.0, 16.0)
    }

    /// The bounding box in the target's pixels of the px rect through the current transform, or none if part of it
    /// is behind the eye.
    fn mapped(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Option<Rect> {
        let (mut l, mut t, mut r, mut b) = (1e30f32, 1e30f32, -1e30f32, -1e30f32);
        for c in 0..4 {
            let (q, w) = self.transform.apply_w(if c & 1 != 0 { x1 } else { x0 }, if c & 2 != 0 { y1 } else { y0 });
            if w <= 0.0 {
                return None;
            }
            (l, t, r, b) = (l.min(q.x), t.min(q.y), r.max(q.x), b.max(q.y));
        }
        Some(Rect::new(l, t, r, b))
    }

    /// Records a primitive: its own transform (the prim's) goes through the current one, unless it is in the
    /// target's pixels already (a path's coverage, an opaque fill).
    fn add(&mut self, kind: i32, prim: &Prim, inputs: Inputs) {
        let Some(pass_index) = self.pass_index() else { return }; // (drawn outside begin_target/end_target)
        let clip = self.scissor();
        if clip.is_empty() {
            return;
        }
        let Inputs { texture, mut texture1, mask, sampler, opaque, in_target_pixels } = inputs;
        let blend = if opaque { BlendMode::SrcOver } else { self.blends.last().copied().unwrap_or(BlendMode::SrcOver) };
        if blend == BlendMode::Dst {
            return; // (changes nothing)
        }
        let mut p = *prim;
        if kind != MESH {
            p.extra[2] = kind as f32; // (a mesh's extra is a vertex's color: vsMesh knows what it is)
        }
        if kind != PATH && kind != MESH && !opaque && !in_target_pixels && !self.transform.is_identity() {
            // Its own transform, then the current one.
            let own = Projective {
                m: [
                    p.xform[0],
                    p.xform[2],
                    p.extra[0],
                    p.xform[1],
                    p.xform[3],
                    p.extra[1],
                    p.projection[0],
                    p.projection[1],
                    p.projection[2],
                ],
            };
            let t = self.transform * own;
            p.xform = [t.m[0], t.m[3], t.m[1], t.m[4]];
            p.extra[0] = t.m[2];
            p.extra[1] = t.m[5];
            p.projection = [t.m[6], t.m[7], t.m[8], 0.0];
            // The quad reaches a device pixel past the shape where that is smallest (it reaches one of its own
            // pixels past it already); glyphs and images turned get anti-aliased edges.
            let mut least = 0.0f32;
            for c in 0..4 {
                let x = p.bounds[if c & 1 != 0 { 2 } else { 0 }];
                let y = p.bounds[if c & 2 != 0 { 3 } else { 1 }];
                let (_, w) = t.apply_w(x, y);
                let m = if w > 0.0 { t.min_scale(x, y) } else { 0.0 };
                if m > 0.0 {
                    least = if least > 0.0 { least.min(m) } else { m };
                }
            }
            let pad = if least > 0.0 { (1.0 / least).min(1e4) } else { 1.0 };
            let flags = p.extra[3] as i32;
            let grow = |p: &mut Prim, by: f32| {
                p.bounds = [p.bounds[0] - by, p.bounds[1] - by, p.bounds[2] + by, p.bounds[3] + by];
            };
            if kind == GLYPH || kind == IMAGE {
                if !t.axis_aligned() {
                    p.extra[3] = (flags | 2) as f32;
                    grow(&mut p, pad);
                }
            } else if !(kind == LAYER && flags & 1 != 0) && pad > 1.0 {
                grow(&mut p, pad - 1.0);
            }
        }
        if kind != MESH && self.aliased() {
            // (A mesh isn't anti-aliased anyway.)
            p.extra[3] = ((p.extra[3] as i32) | 16) as f32;
            // A rect square to the pixels: its edges where Skia rounds them, to the nearest pixel edge (a pixel
            // whose middle is on its left or top edge out, on its right or bottom one in), in its own coordinates
            // (its transform taken in above): no middle lies on an edge then.
            let square = kind == crate::shaders::FILL
                && p.radii.iter().all(|&r| r <= 0.0)
                && p.xform[1] == 0.0
                && p.xform[2] == 0.0
                && p.projection[0] == 0.0
                && p.projection[1] == 0.0
                && p.projection[2] == 1.0
                && p.xform[0] != 0.0
                && p.xform[3] != 0.0;
            if square {
                // (To the device and back.)
                let snap = |v: f32, a: f32, t: f32| ((a * v + t + 0.5).floor() - t) / a;
                p.shape[0] = snap(p.shape[0], p.xform[0], p.extra[0]);
                p.shape[2] = snap(p.shape[2], p.xform[0], p.extra[0]);
                p.shape[1] = snap(p.shape[1], p.xform[3], p.extra[1]);
                p.shape[3] = snap(p.shape[3], p.xform[3], p.extra[1]);
                if p.shape[0] > p.shape[2] {
                    p.shape.swap(0, 2);
                }
                if p.shape[1] > p.shape[3] {
                    p.shape.swap(1, 3);
                }
            }
        }
        if let Some(chain) = self.clips.last().and_then(|c| c.chain)
            && !opaque
            && (kind == MESH || !self.clear_of_clips(&p, chain))
        {
            p.filter[2] = self.clip_chains[chain].elements.len().min(STOPS_WIDTH / 6) as f32;
            p.filter[3] = self.clip_row(chain) as f32;
        }
        if !self.filters.is_empty() && !opaque {
            let (count, row) = self.filter_row();
            p.filter[0] = count as f32;
            p.filter[1] = row as f32;
        }
        let (mut sampler1, mut sampler5, mut texture5) = (Sampler::default(), Sampler::default(), None);
        let paint_kind = (p.paint[0] + 0.5) as i32;
        // An image paint's texture.
        let image = |index: f32| {
            if index < 0.0 || index as usize >= self.paint_textures.len() {
                return None;
            }
            Some(self.paint_textures[index as usize])
        };
        if paint_kind == crate::shaders::IMAGE_PAINT
            && texture1.is_none()
            && let Some(t) = image(p.paint[2])
        {
            (texture1, sampler1) = (Some(t.texture), t.sampler);
        }
        if paint_kind == crate::shaders::COMPOSITE_PAINT {
            // (A composite's: its destination's, its source's.)
            if texture1.is_none()
                && let Some(t) = image(p.paint[1])
            {
                (texture1, sampler1) = (Some(t.texture), t.sampler);
            }
            if let Some(t) = image(p.paint[2]) {
                (texture5, sampler5) = (Some(t.texture), t.sampler);
            }
        }
        let white = self.white.id();
        let (texture, texture1, texture5) =
            (texture.unwrap_or(white), texture1.unwrap_or(white), texture5.unwrap_or(white));
        let generation = self.shelf.generation;
        // In perspective: its pixels' steps are worked out where they are (flag 32, in primitives.slang).
        if kind != MESH && (p.projection[0] != 0.0 || p.projection[1] != 0.0) {
            p.extra[3] = ((p.extra[3] as i32) | 32) as f32;
        }
        // What it needs of the fragment shader (a composite paint: its two paints may be any). (A mesh's extra is
        // its vertex's color: its flags may be any.)
        let variant = ShaderVariant {
            kinds: 1 << kind,
            paints: if paint_kind == crate::shaders::COMPOSITE_PAINT { 0x3f } else { 1u8 << paint_kind },
            flags: if kind == MESH { 0xff } else { p.extra[3] as i32 as u8 },
            clipped: p.filter[2] > 0.0,
            filtered: p.filter[0] > 0.0,
        };
        let pass = &mut self.passes[pass_index];
        if let Some(last) = pass.batches.last_mut()
            && last.texture == texture
            && last.texture1 == texture1
            && last.mask == mask
            && last.opaque == opaque
            && last.blend == blend
            && last.scissor == clip
            && last.generation == generation
            && last.sampler == sampler
            && last.sampler1 == sampler1
            && last.mesh == (kind == MESH)
            && last.texture5 == texture5
            && last.sampler5 == sampler5
        {
            pass.prims.push(p);
            last.count += 1;
            last.variant |= variant;
            return;
        }
        pass.batches.push(Batch {
            texture,
            texture1,
            mask,
            opaque,
            blend,
            scissor: clip,
            generation,
            first: pass.prims.len() as u32,
            count: 1,
            sampler,
            sampler1,
            mesh: kind == MESH,
            texture5,
            sampler5,
            variant,
        });
        pass.prims.push(p);
    }

    /// The color filters in effect, the innermost first, in a row of the table (once a frame for each chain): how
    /// many, and the row.
    fn filter_row(&mut self) -> (usize, i32) {
        let chain: Vec<ColorFilter> = self.filters.iter().rev().copied().collect();
        let count = chain.len().min(STOPS_WIDTH / 6);
        if let Some(i) = self.filter_chains.iter().position(|c| *c == chain) {
            return (count, self.filter_rows[i]);
        }
        let row = self.table_rows();
        let at = self.stops.len();
        self.stops.resize(at + STOPS_WIDTH * 4, 0.0);
        for (i, f) in chain.iter().take(STOPS_WIDTH / 6).enumerate() {
            let t = &mut self.stops[at + i * 24..at + i * 24 + 24]; // six texels
            match *f {
                ColorFilter::ColorMatrix(m) => {
                    t[0] = 1.0;
                    for r in 0..4 {
                        for k in 0..4 {
                            t[4 + r * 4 + k] = m.values[r * 5 + k];
                        }
                        t[20 + r] = m.values[r * 5 + 4] / 255.0; // (Android's offsets are 0..255)
                    }
                }
                ColorFilter::Lighting { multiply, add } => {
                    t[0] = 2.0;
                    t[4..8].copy_from_slice(&multiply.srgb_components());
                    t[8..12].copy_from_slice(&add.srgb_components());
                }
                ColorFilter::BlendMode { color, blend_mode } => {
                    t[1] = paint::blend_index(blend_mode) as f32;
                    t[4..8].copy_from_slice(&color.srgb_components());
                }
            }
        }
        self.filter_chains.push(chain);
        self.filter_rows.push(row);
        (count, row)
    }

    /// The table's rows so far: the next one's index.
    fn table_rows(&self) -> i32 {
        (self.stops.len() / (STOPS_WIDTH * 4)) as i32
    }

    // ---- After each frame -------------------------------------------------------------------------------------------

    /// After each frame: the frame's table and chains go; textures of images that are gone, layer targets unused for
    /// a while, kept layers and paths' shadows unused for a while go; the kept paths start over once full.
    pub fn end_frame(&mut self) {
        self.images.retired.clear();
        self.stops.clear();
        self.stop_brushes.clear();
        self.stop_rows.clear();
        self.paint_textures.clear();
        self.filter_chains.clear();
        self.filter_rows.clear();
        self.clip_chains.clear();
        self.stops_uploaded = 0;
        self.images.textures.retain(|_, t| t.token.is_alive());
        self.evict_pool();
        self.end_frame_shadows();
        self.age_kept_layers();
        // The kept paths started over once full, unless this frame's paths fill most of it themselves (then kept
        // as they are: those in it are drawn from it, the rest as any path, rather than all of them rasterized again
        // each frame).
        if self.path_mask_full && self.paths_drawn_area * 2 < KEPT_MASK as i64 * KEPT_MASK as i64 {
            self.kept_path_map.clear();
            self.path_shelf = MaskShelf::default();
            self.path_mask_full = false;
            self.path_mask_clear = true;
        }
        self.path_frame += 1;
        self.paths_drawn_area = 0;
    }

    /// The layer targets kept unused go now (the app gone to the background, the system short of memory), as
    /// Android's renderer purges its unused scratch textures then; and the paths' shadows' masks.
    pub fn trim_memory(&mut self) {
        self.shadows.kept.clear();
        self.layers.pool.retain(|p| p.in_use);
    }
}

// ---- Blend modes, anti-aliasing, color filters, translations and transforms ---------------------------------------

fn push_blend_mode(r: &mut Renderer, mode: BlendMode) {
    r.blends.push(mode);
}

fn pop_blend_mode(r: &mut Renderer) {
    r.blends.pop();
}

fn push_anti_alias(r: &mut Renderer, on: bool) {
    r.anti_alias.push(on);
}

fn pop_anti_alias(r: &mut Renderer) {
    r.anti_alias.pop();
}

fn push_color_filter(r: &mut Renderer, filter: &ColorFilter) {
    r.filters.push(*filter);
}

fn pop_color_filter(r: &mut Renderer) {
    r.filters.pop();
}

fn push_translation(r: &mut Renderer, by: kompoz_graphics::Offset) {
    r.placements.push(Placement { ox: r.ox, oy: r.oy, transform: r.transform });
    r.ox -= by.x * r.scale;
    r.oy -= by.y * r.scale;
}

/// Ends a translation or a transform.
fn pop_translation(r: &mut Renderer) {
    let Some(p) = r.placements.pop() else { return };
    (r.ox, r.oy, r.transform) = (p.ox, p.oy, p.transform);
}

fn push_transform(r: &mut Renderer, matrix: &kompoz_graphics::Matrix) {
    r.placements.push(Placement { ox: r.ox, oy: r.oy, transform: r.transform });
    let t = Projective::of(matrix);
    if t.affine() && t.m[0] == 1.0 && t.m[1] == 0.0 && t.m[3] == 0.0 && t.m[4] == 1.0 && t.m[8] == 1.0 {
        // Only moved.
        r.ox -= t.m[2] * r.scale;
        r.oy -= t.m[5] * r.scale;
        return;
    }
    // In the target's pixels: the map from what is drawn (x * scale - origin), the matrix, and back.
    let s = r.scale;
    let unit = Projective { m: [s, 0.0, -r.ox, 0.0, s, -r.oy, 0.0, 0.0, 1.0] };
    r.transform = r.transform * unit * t * unit.inverse();
}

fn quick_reject(r: &Renderer, rect: Rect) -> bool {
    let visible = r.scissor();
    let s = r.scale;
    let Some(on) = r.mapped(rect.left * s - r.ox, rect.top * s - r.oy, rect.right * s - r.ox, rect.bottom * s - r.oy)
    else {
        return false; // partly behind the eye: whatever shows of it may be anywhere
    };
    on.right <= visible.x as f32
        || on.bottom <= visible.y as f32
        || on.left >= (visible.x + visible.width) as f32
        || on.top >= (visible.y + visible.height) as f32
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.stop_warming();
        self.device.wait_idle();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rects_meet_where_they_overlap() {
        assert_eq!(intersect(IRect::new(0, 0, 10, 10), IRect::new(5, 6, 10, 10)), IRect::new(5, 6, 5, 4));
        assert!(intersect(IRect::new(0, 0, 4, 4), IRect::new(8, 8, 2, 2)).is_empty());
        assert_eq!(pixels_of(Rect::new(0.5, 1.0, 2.25, 3.0)), IRect::new(0, 1, 3, 2));
    }

    #[test]
    fn variants_gather_what_their_batches_need() {
        let mut v = ShaderVariant { kinds: 1, paints: 1, ..ShaderVariant::default() };
        v |= ShaderVariant { kinds: 1 << PATH, paints: 2, flags: 16, clipped: true, filtered: false };
        assert_eq!(v, ShaderVariant { kinds: 1 | 1 << PATH, paints: 3, flags: 16, clipped: true, filtered: false });
    }
}
