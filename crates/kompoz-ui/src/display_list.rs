//! A frame's drawing, recorded, so that the next frame can tell which pixels changed and only those are drawn again;
//! a frame that changes nothing on the screen isn't drawn at all (an animation scrolled out of view, a
//! recomposition that looks the same). C++'s DisplayList; Compose has no such thing (Android's RenderNodes keep what
//! views draw instead).
//!
//! ```text
//! list.begin(viewport);
//! host.draw(&mut list);                  // records
//! if list.end().is_some() {              // something differs from the last frame
//!     for &area in list.areas() {        // where (px)
//!         list.replay(&mut canvas, Some(area));  // into the kept frame, clipped to the area
//!     }
//! }
//! ```
//!
//! Each command knows the screen area it can touch: its own bounds through the transforms of the layers around it,
//! cut by their clips, padded for anti-aliasing. Frames are lined up by the layout nodes that drew them
//! ([`GraphicsContext::begin_node`]), so content that appears or goes doesn't make everything after it look
//! changed: what a node drew (before its first child, after each child) is compared with what it drew last time,
//! and the areas of what differs, before and after, are the damage.
//!
//! Where the kept frame's pixels can be moved ([`DisplayList::set_allow_moves`]), what moved by whole pixels within
//! a rect clip, as a scrolled list does, moves them ([`DisplayList::moves`], before the areas) instead of being
//! drawn again: only what that leaves wrong is, the strip that came into view and what didn't move with it. What
//! moved is the same commands in their own space drawn through a transform that takes them further on the screen
//! (nodes draw in their own space, translated to where they are: a scroll moves that translation).
//!
//! Where nodes are kept ([`DisplayList::set_keep_nodes`]), a node whose drawing didn't change since the last frame
//! (`record_node`'s `unchanged`) isn't drawn again: what it drew last frame is recorded again, exactly as drawing it
//! would record it: its translation to where it now is in its parent, what is in it as it was, through what it is
//! drawn in now (which may have moved: a list scrolled). What its drawing left out or asked (what was clipped away,
//! `quick_reject`'s answers) is recorded with it and asked again there: where any of that comes out otherwise
//! (something scrolled into view), or it can't be redrawn so (the light of a shadow, asked where it was), the node
//! is drawn.
//!
//! Lengths are px; damage comes in whole pixels.

use crate::graphics_context::{
    ClipOp, CornerRadii, ElevationShadow, GraphicsContext, LayerContent, LayerTransform, NodeRecording, Shadow,
    ShadowLight, TextDraw, default_draw_shape_shadow, shadow_path_reach,
};
use kompoz_graphics::{
    BlendMode, Brush, BrushKind, Color, ColorFilter, DefaultFilterQuality, Density, FilterQuality, ImageBitmap,
    LayoutDirection, Matrix, Offset, Outline, Path, PathStroke, Rect, RenderEffect, Shader, ShaderKind, Shape, Size,
    StrokeCap, StrokeJoin, Vertices,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

// ---- Helpers ---------------------------------------------------------------------------------------------------

thread_local! {
    /// The draw commands recorded that can show (not clipped away), by any list on this thread (a layer's drawn
    /// inside a node too), and those copied with kept nodes: a node that adds none drew nothing that shows
    /// (`ended_node_shows`).
    static SHOWN: Cell<u64> = const { Cell::new(0) };
}

/// Each list's own number (layers' keys are their list's too).
static NEXT_LIST: AtomicU64 = AtomicU64::new(1);

/// Numbers by key (a node's, a segment's or layer's), filled again every frame without being freed.
type KeyMap = HashMap<u64, u32, BuildHasherDefault<KeyHasher>>;

/// Keys are mixed already: spread by a multiply (FNV for anything else hashed).
#[derive(Default)]
struct KeyHasher(u64);

impl Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ b as u64).wrapping_mul(0x100000001b3);
        }
    }
    fn write_u64(&mut self, key: u64) {
        self.0 = key.wrapping_mul(0x9E3779B97F4A7C15);
    }
}

/// The overlap, its width and height none (not negative) where there is none (as C++'s Rect::intersect).
fn intersect(a: Rect, b: Rect) -> Rect {
    let (l, t) = (a.left.max(b.left), a.top.max(b.top));
    Rect::new(l, t, a.right.min(b.right).max(l), a.bottom.min(b.bottom).max(t))
}

fn empty(r: Rect) -> bool {
    r.width() <= 0.0 || r.height() <= 0.0
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
}

fn contains(outer: Rect, inner: Rect) -> bool {
    outer.left <= inner.left && outer.top <= inner.top && outer.right >= inner.right && outer.bottom >= inner.bottom
}

fn area(r: Rect) -> f32 {
    if empty(r) { 0.0 } else { r.width() * r.height() }
}

/// Where things moved to are a thousandth of a px off at most (whole px summed in floats).
fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-3
}

fn near_offset(a: Offset, b: Offset) -> bool {
    near(a.x, b.x) && near(a.y, b.y)
}

fn near_rect(a: Rect, b: Rect) -> bool {
    near(a.left, b.left) && near(a.top, b.top) && near(a.right, b.right) && near(a.bottom, b.bottom)
}

fn moved_to(a: Offset, o: Offset, by: Offset) -> bool {
    near(a.x, o.x + by.x) && near(a.y, o.y + by.y)
}

fn moved_rect_to(a: Rect, o: Rect, by: Offset) -> bool {
    moved_to(a.top_left(), o.top_left(), by) && a.width() == o.width() && a.height() == o.height()
}

/// The whole pixels `r` touches.
fn whole_out(r: Rect) -> Rect {
    Rect::new((r.left + 1e-3).floor(), (r.top + 1e-3).floor(), (r.right - 1e-3).ceil(), (r.bottom - 1e-3).ceil())
}

/// The whole pixels all inside `r`.
fn whole_in(r: Rect) -> Rect {
    let (l, t) = ((r.left - 1e-3).ceil(), (r.top - 1e-3).ceil());
    Rect::new(l, t, (r.right + 1e-3).floor().max(l), (r.bottom + 1e-3).floor().max(t))
}

/// `r` added to `rects` (that don't overlap): made one with those it overlaps.
fn join(rects: &mut Vec<Rect>, mut r: Rect) {
    let mut i = 0;
    while i < rects.len() {
        if overlaps(rects[i], r) {
            r = r.union(rects[i]);
            rects.remove(i);
            i = 0; // (grown: it may reach those already passed)
        } else {
            i += 1;
        }
    }
    rects.push(r);
}

/// What is left of `r` outside `hole` (a rect within it, or not), in up to four rects that don't overlap: the bands
/// above and below it all across, those beside it between them.
fn subtract(r: Rect, hole: Rect, out: &mut Vec<Rect>) {
    let h = intersect(hole, r);
    if empty(h) {
        if !empty(r) {
            out.push(r);
        }
        return;
    }
    let mut add = |piece: Rect| {
        if !empty(piece) {
            out.push(piece);
        }
    };
    add(Rect::new(r.left, r.top, r.right, h.top));
    add(Rect::new(r.left, h.bottom, r.right, r.bottom));
    add(Rect::new(r.left, h.top, h.left, h.bottom));
    add(Rect::new(h.right, h.top, r.right, h.bottom));
}

fn mix(h: u64, v: u64) -> u64 {
    h ^ v.wrapping_add(0x9e3779b97f4a7c15).wrapping_add(h << 6).wrapping_add(h >> 2)
}

/// A node's key in a recording: it, and the how many-th time it is drawn in it.
fn node_key(node: u64, occurrence: u32) -> u64 {
    mix(mix(node, occurrence as u64), 0x51ed27)
}

/// What the images in a brush's shader hold (their ids and versions, summed up): one drawn into since is a change.
fn images_in(brush: &Brush, area: Rect) -> u64 {
    fn walk(s: &Shader) -> u64 {
        match s.kind() {
            ShaderKind::Image => s.image().map_or(0, |i| i.id().wrapping_mul(1000003).wrapping_add(i.version())),
            ShaderKind::Composite => s.dst().map_or(0, walk).wrapping_mul(31).wrapping_add(s.src().map_or(0, walk)),
            _ => 0,
        }
    }
    match brush.kind() {
        BrushKind::Shader(_) | BrushKind::Composite(_) => walk(&brush.create_shader(area.size())),
        _ => 0,
    }
}

/// Maps points of what is drawn inside transforms and layers to the screen: (x, y, 1) to (x', y', w), row by row,
/// the point x' / w, y' / w.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Projective {
    pub m: [f32; 9],
}

impl Default for Projective {
    fn default() -> Projective {
        Projective::Identity
    }
}

impl Projective {
    pub const Identity: Projective = Projective { m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0] };

    /// The bounding box of the mapped rect; if part of it is behind the eye, all of the plane.
    pub fn map(&self, r: Rect) -> Rect {
        let m = &self.m;
        if m[6] == 0.0 && m[7] == 0.0 && m[1] == 0.0 && m[3] == 0.0 && m[8] != 0.0 {
            // only scaled and moved: no need for the corners
            let (x0, x1) = ((m[0] * r.left + m[2]) / m[8], (m[0] * r.right + m[2]) / m[8]);
            let (y0, y1) = ((m[4] * r.top + m[5]) / m[8], (m[4] * r.bottom + m[5]) / m[8]);
            return Rect::new(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
        }
        let affine = m[6] == 0.0 && m[7] == 0.0; // (w the same everywhere: in front whatever its sign)
        let (mut l, mut t, mut rr, mut bb) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        for c in 0..4 {
            let x = if c & 1 != 0 { r.right } else { r.left };
            let y = if c & 2 != 0 { r.bottom } else { r.top };
            let w = m[6] * x + m[7] * y + m[8];
            let in_front = if affine { w != 0.0 } else { w > 0.0 }; // (NaN: not)
            if !in_front {
                return Rect::new(-1e30, -1e30, 1e30, 1e30); // behind the eye
            }
            let (px, py) = ((m[0] * x + m[1] * y + m[2]) / w, (m[3] * x + m[4] * y + m[5]) / w);
            (l, t, rr, bb) = (l.min(px), t.min(py), rr.max(px), bb.max(py));
        }
        Rect::new(l, t, rr, bb)
    }

    /// `a` after `b`.
    fn compose(a: &Projective, b: &Projective) -> Projective {
        let mut out = Projective { m: [0.0; 9] };
        for r in 0..3 {
            for c in 0..3 {
                out.m[r * 3 + c] = a.m[r * 3] * b.m[c] + a.m[r * 3 + 1] * b.m[3 + c] + a.m[r * 3 + 2] * b.m[6 + c];
            }
        }
        out
    }

    fn translation(by: Offset) -> Projective {
        Projective { m: [1.0, 0.0, by.x, 0.0, 1.0, by.y, 0.0, 0.0, 1.0] }
    }

    /// A layer's compositing (as backends do it): scaled and rotated around its origin, then moved.
    fn layer(bounds: Rect, t: &LayerTransform) -> Projective {
        if t.is_identity() {
            return Projective::Identity;
        }
        let px = bounds.left + bounds.width() * t.origin.x;
        let py = bounds.top + bounds.height() * t.origin.y;
        let rad = t.rotation_z * std::f32::consts::PI / 180.0;
        let (sn, cs) = rad.sin_cos();
        let (a, b, c, d) = (t.scale_x * cs, t.scale_x * sn, -t.scale_y * sn, t.scale_y * cs);
        Projective {
            m: [
                a,
                c,
                px + t.translation_x - (a * px + c * py),
                b,
                d,
                py + t.translation_y - (b * px + d * py),
                0.0,
                0.0,
                1.0,
            ],
        }
    }

    /// A Matrix's plane part (as backends draw through it).
    fn plane(t: &Matrix) -> Projective {
        let v = &t.values;
        Projective {
            m: [
                v[Matrix::ScaleX],
                v[Matrix::SkewX],
                v[Matrix::TranslateX],
                v[Matrix::SkewY],
                v[Matrix::ScaleY],
                v[Matrix::TranslateY],
                v[Matrix::Perspective0],
                v[Matrix::Perspective1],
                v[Matrix::Perspective2],
            ],
        }
    }

    /// The same but for where it takes things (a translation's difference), in the plane.
    fn same_but_translation(&self, o: &Projective) -> bool {
        let (m, n) = (&self.m, &o.m);
        m[6] == 0.0 && m[7] == 0.0 && m[8] != 0.0 && [0, 1, 3, 4, 6, 7, 8].iter().all(|&i| m[i] == n[i])
    }

    fn square_to_screen(&self) -> bool {
        self.m[1] == 0.0 && self.m[3] == 0.0 && self.m[6] == 0.0 && self.m[7] == 0.0 && self.m[8] == 1.0
    }
}

// ---- Commands --------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    FillRoundRect,
    StrokeRoundRect,
    Line,
    Text,
    TextBrush,
    TextShadow,
    Shadow,
    Arc,
    Image,
    Gradient,
    Path,
    PushLayer,
    PopLayer,
    PushClip,
    PopClip,
    PushTranslation,
    PopTranslation,
    PushBlend,
    PopBlend,
    ShadowPath,
    PushFilter,
    PopFilter,
    PushTransform,
    PopTransform,
    Backdrop,
    ElevationShadow,
    ShapeShadow,
    Vertices,
    ImageMask,
    PushAlias,
    PopAlias,
}

impl Kind {
    fn is_push(self) -> bool {
        matches!(
            self,
            Kind::PushLayer
                | Kind::PushClip
                | Kind::PushTranslation
                | Kind::PushBlend
                | Kind::PushAlias
                | Kind::PushFilter
                | Kind::PushTransform
        )
    }
    fn is_pop(self) -> bool {
        matches!(
            self,
            Kind::PopLayer
                | Kind::PopClip
                | Kind::PopTranslation
                | Kind::PopBlend
                | Kind::PopAlias
                | Kind::PopFilter
                | Kind::PopTransform
        )
    }
}

/// A clip's shape.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum ClipKind {
    #[default]
    Rect,
    RoundRect,
    Path,
}

/// A brushed shape shadow's shape, as its outline over the rect (the shape itself can't be kept), and what it was
/// made in.
#[derive(Clone, PartialEq, Debug)]
struct ShapedShadow {
    outline: Outline,
    layout_direction: LayoutDirection,
    density: Density,
}

/// A shape that is a recorded outline (in the size it was made for), for replaying a shape shadow.
#[derive(Clone, PartialEq, Debug)]
struct RecordedOutline(Outline);

impl Shape for RecordedOutline {
    fn create_outline(&self, _: Size, _: LayoutDirection, _: &Density) -> Outline {
        self.0.clone()
    }
}

const NONE: u32 = u32::MAX;

/// A command recorded. Which fields say what, by kind: `rect` the shape (an image's destination, a layer's or
/// clip's bounds), `rect2` an image's source or a brush's area (an image mask's: `p1` and the size in `p2`), `p1` a
/// baseline, an origin, a line's start or a translation, `p2` a line's end; `f1` an arc's start, an alpha or a blur,
/// `f2` a width, a sweep or a layer's alpha, `f3` an arc's width; `flag` a round cap, a shadow's `inner` or a clip's
/// ClipOp::Difference. Text is kept with its baseline at the origin (the baseline is `p1`).
#[derive(Clone)]
struct Command {
    kind: Kind,
    rect: Rect,
    rect2: Rect,
    f1: f32,
    f2: f32,
    f3: f32,
    radii: CornerRadii,
    color: Color,
    p1: Offset,
    p2: Offset,
    flag: bool,
    clip: ClipKind,
    quality: FilterQuality,
    blend_mode: BlendMode,
    text: Option<TextDraw>,
    image: Option<ImageBitmap>,
    /// What it held (drawn into since: a change); a brush's images', summed up.
    image_version: u64,
    brush: Option<Brush>,
    filter: Option<ColorFilter>,
    /// An effect layer's, a backdrop's.
    effect: Option<RenderEffect>,
    /// A shadow's, a path's or a shape's.
    shadow: Option<Shadow>,
    /// An elevation shadow's (at `p1`).
    elevated: Option<(Outline, ElevationShadow)>,
    /// A brushed shape shadow's (over `rect`).
    shaped: Option<ShapedShadow>,
    /// A path's, a path shadow's, a clip's; an outline layer's outline.
    path: Option<Path>,
    vertices: Option<Vertices>,
    stroke: Option<PathStroke>,
    transform: LayerTransform,
    /// PushTransform's.
    matrix: Matrix,
    /// The transform it is drawn through.
    m: Projective,
    /// What it can touch on the screen (a layer: where it is composited).
    bounds: Rect,
    /// A push: the index after its pop.
    end: u32,
    /// The push it is drawn in (the innermost).
    parent: u32,
    /// A layer's: its node's, the same from frame to frame.
    layer_key: u64,
    /// A layer's: what is drawn in it is what was last frame.
    layer_same: bool,
}

impl Command {
    fn new(kind: Kind) -> Command {
        Command {
            kind,
            rect: Rect::Zero,
            rect2: Rect::Zero,
            f1: 0.0,
            f2: 0.0,
            f3: 0.0,
            radii: CornerRadii::Zero,
            color: Color::Transparent,
            p1: Offset::Zero,
            p2: Offset::Zero,
            flag: false,
            clip: ClipKind::Rect,
            quality: DefaultFilterQuality,
            blend_mode: BlendMode::SrcOver,
            text: None,
            image: None,
            image_version: 0,
            brush: None,
            filter: None,
            effect: None,
            shadow: None,
            elevated: None,
            shaped: None,
            path: None,
            vertices: None,
            stroke: None,
            transform: LayerTransform::Identity,
            matrix: Matrix::IDENTITY,
            m: Projective::Identity,
            bounds: Rect::Zero,
            end: 0,
            parent: NONE,
            layer_key: 0,
            layer_same: false,
        }
    }

    /// The same in all but where it is (its rects and points), where it lands and what it is drawn through.
    fn same_but_place(&self, o: &Command) -> bool {
        self.radii == o.radii
            && self.f1 == o.f1
            && self.f2 == o.f2
            && self.f3 == o.f3
            && self.color == o.color
            && self.flag == o.flag
            && self.clip == o.clip
            && self.quality == o.quality
            && self.blend_mode == o.blend_mode
            && self.image == o.image
            && self.image_version == o.image_version
            && self.stroke == o.stroke
            && self.transform == o.transform
            && self.text == o.text
            && self.brush == o.brush
            && self.vertices == o.vertices
            && self.filter == o.filter
            && self.effect == o.effect
            && self.shadow == o.shadow
            && self.elevated == o.elevated
            && self.shaped == o.shaped
            && self.path == o.path
            && self.matrix == o.matrix
    }

    fn same(&self, o: &Command) -> bool {
        self.kind == o.kind
            && self.bounds == o.bounds
            && self.m == o.m
            && self.rect == o.rect
            && self.rect2 == o.rect2
            && self.p1 == o.p1
            && self.p2 == o.p2
            && self.same_but_place(o)
    }

    /// The same as `o` moved by `by` (in its own space), but for its bounds (a clip may cut them); `with_transform`:
    /// drawn through the same transform too. Which of its fields say where it is goes by kind: its rect (a shape's,
    /// an image's destination, a layer's or clip's bounds), its brush's area (`rect2`, or `p1` for an image
    /// mask's), its first point (a baseline, an origin, a line's start) and its second (a line's end). The rest (an
    /// image's source, a shadow's offset) stays as it was.
    fn same_moved(&self, o: &Command, by: Offset, with_transform: bool) -> bool {
        // (What a backdrop reads moved too: not the same.)
        if self.kind != o.kind || self.kind == Kind::Backdrop || (with_transform && self.m != o.m) {
            return false;
        }
        // An elevation shadow is cast from the light, which stays where it is: moved, it falls otherwise.
        if self.kind == Kind::ElevationShadow && (by.x != 0.0 || by.y != 0.0) {
            return false;
        }
        if self.kind == Kind::PushLayer && (self.path.is_some() || o.path.is_some()) {
            return false; // (an outline's place isn't told)
        }
        let (mut rect_moves, mut brush_moves, mut p1_moves, mut p2_moves) = (false, false, false, false);
        match self.kind {
            Kind::FillRoundRect
            | Kind::StrokeRoundRect
            | Kind::Arc
            | Kind::Shadow
            | Kind::ShapeShadow
            | Kind::Image
            | Kind::PushLayer => rect_moves = true,
            Kind::Gradient => (rect_moves, brush_moves) = (true, true),
            Kind::ImageMask => (rect_moves, p1_moves) = (true, true),
            Kind::PushClip if self.clip == ClipKind::Path => p1_moves = true,
            Kind::PushClip => rect_moves = true,
            Kind::Line => (p1_moves, p2_moves) = (true, true),
            Kind::Text | Kind::TextShadow | Kind::ElevationShadow | Kind::ShadowPath => p1_moves = true,
            Kind::TextBrush | Kind::Path | Kind::Vertices => (p1_moves, brush_moves) = (true, true),
            _ => {}
        }
        // A brush of one color is that wherever its area is (rect2; an image mask's, p1 and p2).
        let solid = self.brush.as_ref().is_some_and(Brush::is_solid) && o.brush.as_ref().is_some_and(Brush::is_solid);
        let any_area = solid && brush_moves;
        let any_mask_area = solid && self.kind == Kind::ImageMask;
        (if rect_moves { moved_rect_to(self.rect, o.rect, by) } else { self.rect == o.rect })
            && (any_area || if brush_moves { moved_rect_to(self.rect2, o.rect2, by) } else { self.rect2 == o.rect2 })
            && (any_mask_area || if p1_moves { moved_to(self.p1, o.p1, by) } else { self.p1 == o.p1 })
            && (any_mask_area || if p2_moves { moved_to(self.p2, o.p2, by) } else { self.p2 == o.p2 })
            && self.same_but_place(o)
    }

    /// Where it is (in its own space), to tell how far it moved; none if it has no place.
    fn place(&self) -> Option<Offset> {
        match self.kind {
            Kind::FillRoundRect
            | Kind::StrokeRoundRect
            | Kind::Arc
            | Kind::Gradient
            | Kind::Shadow
            | Kind::ShapeShadow
            | Kind::Image
            | Kind::ImageMask
            | Kind::PushLayer => Some(self.rect.top_left()),
            Kind::PushClip if self.clip == ClipKind::Path => Some(self.p1),
            Kind::PushClip => Some(self.rect.top_left()),
            Kind::Line
            | Kind::Text
            | Kind::TextBrush
            | Kind::TextShadow
            | Kind::Path
            | Kind::ElevationShadow
            | Kind::Vertices
            | Kind::ShadowPath => Some(self.p1),
            _ => None,
        }
    }

    /// A command that draws (not a push or a pop), touching something.
    fn paints(&self) -> bool {
        !self.kind.is_push() && !self.kind.is_pop() && !empty(self.bounds)
    }
}

/// Pixels of the kept frame to move before areas are drawn again: content that moved by whole pixels within a rect
/// clip, as a scrolled list's does. From `from` (px, whole pixels) by `by`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Move {
    pub from: Rect,
    pub by: Offset,
}

/// What one node drew between its children.
#[derive(Clone, Copy)]
struct Segment {
    key: u64,
    first: u32,
    count: u32,
}

/// A layer's, clip's, translation's or transform's.
#[derive(Clone, Copy, PartialEq)]
struct State {
    m: Projective,
    clip: Rect,
    /// Its command.
    push: u32,
    /// The same but for the layers' own transforms (as they are at rest): what is drawn in a layer is kept if it
    /// shows either way, so that what a layer holds is what its content is, not how far its transform takes it (a
    /// stretch's, growing and shrinking).
    rest_m: Projective,
    rest_clip: Rect,
}

struct OpenNode {
    key: u64,
    /// The child the stretch being drawn follows (its key).
    after: u64,
    /// The draws that can show, recorded so far (by any list: a layer's too), as it began.
    shown_before: u64,
}

/// What happened in a recording besides its commands, before command `at`.
#[derive(Clone, Copy)]
struct Event {
    at: u32,
    kind: EventKind,
}

#[derive(Clone, Copy)]
enum EventKind {
    /// A node begun (its range).
    Begin {
        range: u32,
        node: u64,
    },
    /// The node last begun ended.
    End,
    /// Something it drew clipped away (its reach, px where drawn).
    Culled(Rect),
    /// `quick_reject` asked, and its answer.
    Rejected(Rect),
    Accepted(Rect),
    /// The light's center asked (what is drawn from it doesn't move with the node).
    Light,
}

/// A node's part of a recording: its events and commands, Begin to End; where it was (its top left, if
/// `record_node` said: else it isn't kept) and the state it was drawn in.
#[derive(Clone, Copy)]
struct Range {
    key: u64,
    first_event: u32,
    end_event: u32,
    first_command: u32,
    end_command: u32,
    origin: Option<Offset>,
    entry: State,
    /// Its nodes' ranges: it and the next up to `end_range`.
    first_segment: u32,
    end_segment: u32,
    end_range: u32,
}

/// Of `end()` finding moves: a segment that moved (this frame's, last frame's, by how far).
struct MovedSegment {
    segment: u32,
    last: u32,
    by: Offset,
}

// ---- The list --------------------------------------------------------------------------------------------------

/// A frame's drawing, recorded (C++'s DisplayList): a [`GraphicsContext`] that keeps what is drawn into it as
/// commands with the screen area each can touch, lines frames up by the nodes that drew them, and tells what
/// changed since the last frame (see the module).
pub struct DisplayList {
    recording: Vec<Command>,
    frame: Vec<Command>,
    recording_segments: Vec<Segment>,
    frame_segments: Vec<Segment>,
    states: Vec<State>,
    nodes: Vec<OpenNode>,
    /// How many times each node was drawn so far in the recording.
    occurrences: KeyMap,
    /// `end()`: the last frame's segments by key, and its layers.
    lookup: KeyMap,
    layer_lookup: KeyMap,
    matched: Vec<bool>,
    moved: Vec<MovedSegment>,
    /// Which commands moved (1, then 2 + the move they are in), this frame's and last frame's.
    group: Vec<u32>,
    last_group: Vec<u32>,
    areas: Vec<Rect>,
    moves: Vec<Move>,
    allow_moves: bool,
    keep_nodes: bool,
    kept_nodes: usize,
    drawn_nodes: usize,
    /// Whether the node last ended drew anything that can show.
    ended_shows: bool,
    /// (`quick_reject` and `light_center` tell them too.)
    recording_events: RefCell<Vec<Event>>,
    frame_events: Vec<Event>,
    recording_ranges: Vec<Range>,
    frame_ranges: Vec<Range>,
    /// The ranges being recorded, innermost last.
    open_ranges: Vec<u32>,
    /// The last frame's ranges by key.
    range_lookup: KeyMap,
    /// `record_node`'s, for the range `begin_node` starts.
    next_origin: Option<Offset>,
    /// (`keep()`'s: the nodes it began.)
    kept_begun: Vec<u64>,
    /// This frame's `copy_kept`: last frame's ranges, from, to.
    copied_ranges: Vec<(u32, u32)>,
    /// The layers pushed in the segment being recorded so far.
    layers_in_segment: u32,
    segment_open: bool,
    invalid: bool,
    viewport: Size,
    frame_viewport: Size,
    light: ShadowLight,
    id: u64,
}

impl Default for DisplayList {
    fn default() -> DisplayList {
        DisplayList::new()
    }
}

impl DisplayList {
    /// An empty list; its first frame is damaged all over. Moves and kept nodes are off.
    pub fn new() -> DisplayList {
        let mut list = DisplayList {
            recording: Vec::new(),
            frame: Vec::new(),
            recording_segments: Vec::new(),
            frame_segments: Vec::new(),
            states: Vec::new(),
            nodes: Vec::new(),
            occurrences: KeyMap::default(),
            lookup: KeyMap::default(),
            layer_lookup: KeyMap::default(),
            matched: Vec::new(),
            moved: Vec::new(),
            group: Vec::new(),
            last_group: Vec::new(),
            areas: Vec::new(),
            moves: Vec::new(),
            allow_moves: false,
            keep_nodes: false,
            kept_nodes: 0,
            drawn_nodes: 0,
            ended_shows: true,
            recording_events: RefCell::new(Vec::new()),
            frame_events: Vec::new(),
            recording_ranges: Vec::new(),
            frame_ranges: Vec::new(),
            open_ranges: Vec::new(),
            range_lookup: KeyMap::default(),
            next_origin: None,
            kept_begun: Vec::new(),
            copied_ranges: Vec::new(),
            layers_in_segment: 0,
            segment_open: false,
            invalid: true,
            viewport: Size::Zero,
            frame_viewport: Size::Zero,
            light: ShadowLight::default(),
            id: NEXT_LIST.fetch_add(1, Ordering::Relaxed),
        };
        list.begin(Size::Zero);
        list
    }

    /// Starts recording a frame of `viewport` (px).
    pub fn begin(&mut self, viewport: Size) {
        self.viewport = viewport;
        self.recording.clear();
        self.recording_segments.clear();
        self.occurrences.clear();
        self.recording_events.get_mut().clear();
        self.recording_ranges.clear();
        self.open_ranges.clear();
        self.next_origin = None;
        self.copied_ranges.clear();
        self.kept_nodes = 0;
        self.drawn_nodes = 0;
        let screen = Rect::new(0.0, 0.0, viewport.width, viewport.height);
        self.states.clear();
        self.states.push(State {
            m: Projective::Identity,
            clip: screen,
            push: 0,
            rest_m: Projective::Identity,
            rest_clip: screen,
        });
        self.nodes.clear();
        self.nodes.push(OpenNode { key: 0x6b6f6d706f7a, after: 0, shown_before: 0 }); // the root: what is drawn outside any node
        self.segment_open = false;
    }

    /// Lets `end()` find moves (off at first): the canvas the frame is kept on can move its pixels.
    pub fn set_allow_moves(&mut self, on: bool) {
        self.allow_moves = on;
    }

    /// Keeps what nodes drew from one frame to the next, for `record_node` (off at first).
    pub fn set_keep_nodes(&mut self, on: bool) {
        self.keep_nodes = on;
    }

    /// In the last recording, the nodes `record_node` kept (with all in them).
    pub fn kept_nodes(&self) -> usize {
        self.kept_nodes
    }

    /// In the last recording, the nodes `record_node` had drawn.
    pub fn drawn_nodes(&self) -> usize {
        self.drawn_nodes
    }

    /// The last `end()`'s damage as separate rects (px, whole pixels), each to be drawn again clipped to itself
    /// (after the moves).
    pub fn areas(&self) -> &[Rect] {
        &self.areas
    }

    /// The last `end()`'s moves.
    pub fn moves(&self) -> &[Move] {
        &self.moves
    }

    /// The next frame is damaged all over (the kept pixels were lost).
    pub fn invalidate(&mut self) {
        self.invalid = true;
    }

    /// The commands in the last frame.
    pub fn len(&self) -> usize {
        self.frame.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frame.is_empty()
    }

    /// The light elevation shadows are cast from (the platform sets the window's).
    pub fn set_light(&mut self, light: ShadowLight) {
        self.light = light;
    }

    // ---- Recording ----

    fn add(&mut self, kind: Kind) -> &mut Command {
        if !self.segment_open {
            let n = self.nodes.last().expect("a root node");
            let key = mix(n.key, n.after);
            self.recording_segments.push(Segment { key, first: self.recording.len() as u32, count: 0 });
            self.segment_open = true;
            self.layers_in_segment = 0;
        }
        let segment = self.recording_segments.last_mut().expect("a segment");
        segment.count += 1;
        let segment_key = segment.key;
        let state = *self.states.last().expect("a root state");
        let mut c = Command::new(kind);
        c.m = state.m;
        if self.states.len() > 1 {
            c.parent = state.push;
        }
        if kind == Kind::PushLayer {
            // (its node's n-th, in this recording: another list's layers are others)
            self.layers_in_segment += 1;
            c.layer_key = mix(mix(segment_key, self.layers_in_segment as u64), self.id);
        }
        self.recording.push(c);
        self.recording.last_mut().expect("just pushed")
    }

    fn state(&self) -> State {
        *self.states.last().expect("a root state")
    }

    fn event(&self, kind: EventKind) {
        let at = self.recording.len() as u32;
        self.recording_events.borrow_mut().push(Event { at, kind });
    }

    /// Records a draw command touching `local` (padded by `pad` px), unless that is clipped away.
    fn draw(&mut self, kind: Kind, local: Rect, pad: f32) -> Option<&mut Command> {
        let s = self.state();
        let reach = local.inflate(pad);
        // Two pixels more: anti-aliased edges, and backends snapping to whole pixels.
        let area = intersect(s.m.map(reach).inflate(2.0), s.clip);
        if empty(area) && !self.shows(reach) {
            // Clipped away: it changes no pixel (kept in a layer if it shows there at rest). (A shadow's reach is
            // from the light, not moved with its node.)
            if self.keep_nodes {
                self.event(if kind == Kind::ElevationShadow { EventKind::Light } else { EventKind::Culled(reach) });
            }
            return None;
        }
        SHOWN.set(SHOWN.get() + 1);
        let c = self.add(kind);
        c.bounds = area;
        Some(c)
    }

    /// What reaches `reach` (px where drawn) shows on the screen, now or at rest (`draw()`'s test).
    fn shows(&self, reach: Rect) -> bool {
        let s = self.state();
        !empty(intersect(s.m.map(reach).inflate(2.0), s.clip))
            || !empty(intersect(s.rest_m.map(reach).inflate(2.0), s.rest_clip))
    }

    /// Records text (any kind of it) whose glyphs may reach `blur` further.
    fn text(&mut self, kind: Kind, t: &TextDraw, blur: f32) -> Option<&mut Command> {
        // Glyphs reach past their advances and the line's metrics (italics, accents, emoji).
        let pad = 0.3 * t.size.height.max(t.ascent + t.descent) + 3.0 * blur;
        let r = Rect::new(t.baseline.x, t.baseline.y - t.ascent, t.baseline.x + t.size.width, t.baseline.y + t.descent);
        let c = self.draw(kind, r, pad)?;
        c.p1 = t.baseline;
        c.text = Some(TextDraw { baseline: Offset::Zero, ..t.clone() });
        Some(c)
    }

    /// The state the command just added (a push) draws what is in it in.
    fn push_state(&mut self, m: Projective, rest_m: Projective, clip: Rect, rest_clip: Rect) {
        let push = self.recording.len() as u32 - 1;
        self.states.push(State { m, clip, push, rest_m, rest_clip });
    }

    /// A push that changes nothing of where things land or are cut (a blend mode, anti-aliasing, a color filter).
    fn push_plain(&mut self, kind: Kind) -> &mut Command {
        let s = self.state();
        self.add(kind).bounds = s.clip; // what anything inside can touch (replay skips it when that's outside the area)
        self.push_state(s.m, s.rest_m, s.clip, s.rest_clip);
        self.recording.last_mut().expect("just pushed")
    }

    fn pop(&mut self, kind: Kind) {
        if self.states.len() <= 1 {
            return;
        }
        let push = self.states.pop().expect("a state").push as usize;
        self.add(kind);
        let end = self.recording.len() as u32;
        self.recording[push].end = end;
        if self.recording[push].effect.is_some() {
            let b = self.recording[push].bounds;
            for c in &mut self.recording[push + 1..] {
                if !empty(c.bounds) {
                    c.bounds = b;
                }
            }
        }
    }

    // ---- The frame ----

    /// Ends it: the area whose pixels differ from the last frame's (px), or nothing. That is `areas()`'s bounds;
    /// with moves allowed, what `moves()` leaves to draw.
    pub fn end(&mut self) -> Option<Rect> {
        while self.states.len() > 1 {
            // pushes left open: closed
            let open = self.recording[self.state().push as usize].kind;
            self.pop(match open {
                Kind::PushClip => Kind::PopClip,
                Kind::PushTranslation => Kind::PopTranslation,
                Kind::PushBlend => Kind::PopBlend,
                Kind::PushAlias => Kind::PopAlias,
                Kind::PushFilter => Kind::PopFilter,
                Kind::PushTransform => Kind::PopTransform,
                _ => Kind::PopLayer,
            });
        }
        let screen = Rect::new(0.0, 0.0, self.viewport.width, self.viewport.height);
        self.areas.clear();
        self.moves.clear();
        let afresh = self.invalid || self.viewport != self.frame_viewport;
        // Each layer: what is drawn in it the same as last frame? (Drawn afresh, the pixels a canvas kept of a layer
        // still are its.)
        self.layer_lookup.clear();
        for (i, c) in self.frame.iter().enumerate() {
            if c.kind == Kind::PushLayer {
                self.layer_lookup.entry(c.layer_key).or_insert(i as u32);
            }
        }
        for i in 0..self.recording.len() {
            if self.recording[i].kind == Kind::PushLayer {
                let last = self.layer_lookup.get(&self.recording[i].layer_key).copied();
                let same = last.is_some_and(|last| self.same_in_layer(i, last as usize));
                self.recording[i].layer_same = same;
            }
        }
        if afresh {
            self.areas.push(screen);
        } else {
            self.find_damage(screen);
        }
        std::mem::swap(&mut self.recording, &mut self.frame);
        std::mem::swap(&mut self.recording_segments, &mut self.frame_segments);
        std::mem::swap(self.recording_events.get_mut(), &mut self.frame_events);
        std::mem::swap(&mut self.recording_ranges, &mut self.frame_ranges);
        self.range_lookup.clear();
        for (i, r) in self.frame_ranges.iter().enumerate() {
            // (those record_node placed and that ended)
            if r.origin.is_some() && r.end_event != 0 {
                self.range_lookup.entry(r.key).or_insert(i as u32);
            }
        }
        self.frame_viewport = self.viewport;
        self.invalid = false;
        let first = *self.areas.first()?;
        Some(self.areas.iter().fold(first, |b, a| b.union(*a)))
    }

    /// What each node drew is compared with what it drew last time: what differs is damage where it was and where
    /// it is. So is all of what moved in front of or behind something else: the segments kept are those that follow
    /// each other as they did. The damage then made a few areas.
    fn find_damage(&mut self, screen: Rect) {
        let mut damage = Vec::new();
        self.lookup.clear();
        for (i, s) in self.frame_segments.iter().enumerate() {
            self.lookup.entry(s.key).or_insert(i as u32);
        }
        self.matched.clear();
        self.matched.resize(self.frame_segments.len(), false);
        self.moved.clear();
        let mut last: i64 = -1; // the last frame's segment those kept so far end with
        for si in 0..self.recording_segments.len() {
            let s = self.recording_segments[si];
            let found = self.lookup.get(&s.key).copied();
            let Some(o) = found.filter(|&o| !self.matched[o as usize]) else {
                add_damage(&mut damage, &self.recording, s.first, s.count); // new
                continue;
            };
            self.matched[o as usize] = true;
            let p = self.frame_segments[o as usize];
            if (o as i64) < last {
                // drawn in another order
                add_damage(&mut damage, &self.frame, p.first, p.count);
                add_damage(&mut damage, &self.recording, s.first, s.count);
                continue;
            }
            last = o as i64;
            // What it drew first and last as before stays; the rest changed (or all of it moved).
            let (n, m) = (s.count, p.count);
            let (mut head, mut tail) = (0, 0);
            while head < n
                && head < m
                && self.recording[(s.first + head) as usize].same(&self.frame[(p.first + head) as usize])
            {
                head += 1;
            }
            while tail < n - head
                && tail < m - head
                && self.recording[(s.first + n - 1 - tail) as usize]
                    .same(&self.frame[(p.first + m - 1 - tail) as usize])
            {
                tail += 1;
            }
            if head + tail < n.max(m)
                && self.allow_moves
                && n == m
                && let Some(by) = self.moved_by(s, p)
            {
                self.moved.push(MovedSegment { segment: si as u32, last: o, by });
                continue;
            }
            add_damage(&mut damage, &self.frame, p.first + head, m - head - tail);
            add_damage(&mut damage, &self.recording, s.first + head, n - head - tail);
        }
        for (i, p) in self.frame_segments.iter().enumerate() {
            if !self.matched[i] {
                add_damage(&mut damage, &self.frame, p.first, p.count);
            }
        }
        if !self.moved.is_empty() {
            self.settle_moves(&mut damage);
        }
        // A backdrop that damage reaches (what it reads, or where it shows) is drawn anew with all that is under
        // what it reads.
        let mut widened = !damage.is_empty();
        while widened {
            widened = false;
            for c in &self.recording {
                if c.kind != Kind::Backdrop {
                    continue;
                }
                let all = c.bounds.union(c.rect2);
                let reached = damage.iter().any(|&d| overlaps(d, all));
                let inside = damage.iter().any(|&d| contains(d, all));
                if !reached || inside {
                    continue;
                }
                damage.push(all);
                widened = true;
            }
        }
        // As a few rects: those that overlap made one, then the two that grow least by it while there are too many.
        const MAX_AREAS: usize = 8;
        for d in damage {
            // In whole pixels: a clip's edge then cuts none (what is drawn again there is all of it, not mixed with
            // what was there by how much of the pixel it covers).
            let d = intersect(d, screen);
            if empty(d) {
                continue;
            }
            join(&mut self.areas, intersect(whole_out(d), screen));
        }
        while self.areas.len() > MAX_AREAS {
            let (mut bi, mut bj, mut best) = (0, 1, f32::INFINITY);
            for i in 0..self.areas.len() {
                for j in i + 1..self.areas.len() {
                    let (a, b) = (self.areas[i], self.areas[j]);
                    let grows = area(a.union(b)) - area(a) - area(b);
                    if grows < best {
                        (best, bi, bj) = (grows, i, j);
                    }
                }
            }
            let both = self.areas[bi].union(self.areas[bj]);
            self.areas.remove(bj);
            self.areas[bi] = both;
            let mut i = 0;
            while i < self.areas.len() {
                // (what it now overlaps joins it)
                if i != bi && overlaps(self.areas[i], self.areas[bi]) {
                    self.areas[bi] = self.areas[bi].union(self.areas[i]);
                    self.areas.remove(i);
                    if i < bi {
                        bi -= 1;
                    }
                    i = 0;
                } else {
                    i += 1;
                }
            }
        }
    }

    /// Whether what this frame's layer `push` draws is what last frame's layer `last` drew, in the layer's own space
    /// (moved with it, through its transform: one in it is a push of it, the same, and what is around it is the
    /// layer's too).
    fn same_in_layer(&self, push: usize, last: usize) -> bool {
        let (a, b) = (&self.recording[push], &self.frame[last]);
        if a.end as i64 - push as i64 != b.end as i64 - last as i64 {
            return false;
        }
        let by = a.rect.top_left() - b.rect.top_left();
        // (An elevation shadow in it falls from the light: the layer moved or turned on the screen, it changed.)
        let moved = by != Offset::Zero || a.m != b.m;
        // (what is drawn in it: not its pop, drawn outside)
        (1..(a.end as usize).saturating_sub(push + 1)).all(|k| {
            let c = &self.recording[push + k];
            c.same_moved(&self.frame[last + k], by, false) && !(moved && c.kind == Kind::ElevationShadow)
        })
    }

    /// How far all of segment `s` moved (on the screen) from what last frame's `p` drew: the same commands but for
    /// their place (in their own space) and the transform they are drawn through (the same but for where it takes
    /// them: a node's translation, a scroll's), all moved alike on the screen.
    fn moved_by(&self, s: Segment, p: Segment) -> Option<Offset> {
        let mut screen: Option<Offset> = None;
        for i in 0..s.count {
            let (a, b) = (&self.recording[(s.first + i) as usize], &self.frame[(p.first + i) as usize]);
            let (m, n) = (&a.m.m, &b.m.m);
            if m[6] != 0.0 || m[7] != 0.0 || m[8] == 0.0 || n[6] != 0.0 || n[7] != 0.0 || n[8] != m[8] {
                return None; // (in perspective it moves unevenly)
            }
            if m[0] != n[0] || m[1] != n[1] || m[3] != n[3] || m[4] != n[4] {
                return None; // (scaled or turned otherwise)
            }
            let (Some(now), Some(then)) = (a.place(), b.place()) else {
                // (no place: the same in its own space)
                if !a.same_moved(b, Offset::Zero, false) {
                    return None;
                }
                continue;
            };
            let local = now - then;
            if !a.same_moved(b, local, false) {
                return None;
            }
            // How far it moved on the screen: as far as its place, through the transform, and as far as that moved.
            let on = Offset::new(
                (m[0] * local.x + m[1] * local.y + m[2] - n[2]) / m[8],
                (m[3] * local.x + m[4] * local.y + m[5] - n[5]) / m[8],
            );
            // An elevation shadow is cast from the light, which stays where it is: moved, it falls otherwise.
            if a.kind == Kind::ElevationShadow && (on.x != 0.0 || on.y != 0.0) {
                return None;
            }
            match screen {
                None => screen = Some(on),
                Some(s) if !near_offset(s, on) => return None,
                _ => {}
            }
        }
        screen
    }

    /// Of the segments that moved, those that moved together within a rect clip become moves (their pixels moved,
    /// not drawn again: damage only where that leaves the frame wrong); the rest are damage where they were and
    /// are.
    fn settle_moves(&mut self, damage: &mut Vec<Rect>) {
        // Pushes that change pixels as they are, wherever they are (no clip, no texture of their own).
        let in_place = |c: &Command| {
            matches!(
                c.kind,
                Kind::PushTranslation | Kind::PushBlend | Kind::PushAlias | Kind::PushFilter | Kind::PushTransform
            )
        };
        let square_clip =
            |c: &Command| c.kind == Kind::PushClip && c.clip == ClipKind::Rect && !c.flag && c.m.square_to_screen();
        // The innermost rect clip that stayed around command `i` (in this frame's commands, or the last frame's),
        // with only pushes that change pixels in place between.
        let clip_of = |commands: &[Command], moving: &[u32], i: u32| {
            let mut p = commands[i as usize].parent;
            while p != NONE && moving[p as usize] != 0 {
                p = commands[p as usize].parent;
            }
            while p != NONE && in_place(&commands[p as usize]) {
                p = commands[p as usize].parent;
            }
            if p != NONE && square_clip(&commands[p as usize]) { p } else { NONE }
        };
        self.group.clear();
        self.group.resize(self.recording.len(), 0);
        self.last_group.clear();
        self.last_group.resize(self.frame.len(), 0);
        let segment_of = |list: &DisplayList, k: usize| {
            (list.recording_segments[list.moved[k].segment as usize], list.frame_segments[list.moved[k].last as usize])
        };
        let fill = |list: &mut DisplayList, k: usize, id: u32| {
            let (s, p) = segment_of(list, k);
            list.group[s.first as usize..(s.first + s.count) as usize].fill(id);
            list.last_group[p.first as usize..(p.first + p.count) as usize].fill(id);
        };
        for k in 0..self.moved.len() {
            fill(self, k, 1);
        }
        // Segments that moved alike within the same rect clip move together.
        struct Group {
            clip: u32,
            by: Offset,
            moved: Vec<usize>,
            commands: u32,
            /// The clip's (and those around it), and the whole pixels in it.
            exact: Rect,
            area: Rect,
            ok: bool,
        }
        let mut groups: Vec<Group> = Vec::new();
        let mut rejected: Vec<usize> = Vec::new(); // moved that move with none
        for k in 0..self.moved.len() {
            let (s, last) = segment_of(self, k);
            let mut p = clip_of(&self.recording, &self.group, s.first);
            for i in s.first + 1..s.first + s.count {
                if p == NONE {
                    break;
                }
                if clip_of(&self.recording, &self.group, i) != p {
                    p = NONE; // (all of it in the same one)
                }
            }
            // It was in the same clip as it is.
            let then = clip_of(&self.frame, &self.last_group, last.first);
            if p != NONE && (then == NONE || !self.recording[p as usize].same(&self.frame[then as usize])) {
                p = NONE;
            }
            if p == NONE {
                rejected.push(k);
                continue;
            }
            let by = self.moved[k].by;
            let g = match groups.iter().position(|g| g.clip == p && near_offset(g.by, by)) {
                Some(g) => g,
                None => {
                    groups.push(Group {
                        clip: p,
                        by,
                        moved: Vec::new(),
                        commands: 0,
                        exact: Rect::Zero,
                        area: Rect::Zero,
                        ok: true,
                    });
                    groups.len() - 1
                }
            };
            groups[g].moved.push(k);
            groups[g].commands += s.count;
        }
        let screen = Rect::new(0.0, 0.0, self.viewport.width, self.viewport.height);
        for gi in 0..groups.len() {
            let (clip, commands) = (groups[gi].clip, groups[gi].commands);
            // Of a clip's, the one that moved most.
            let beaten = groups.iter().enumerate().any(|(oi, o)| {
                oi != gi && o.clip == clip && (o.commands > commands || (o.commands == commands && oi < gi))
            });
            let g = &mut groups[gi];
            if beaten {
                g.ok = false;
            }
            // By whole pixels, within the clip in whole pixels (and the rect clips around it).
            if (g.by.x - g.by.x.round()).abs() > 0.01 || (g.by.y - g.by.y.round()).abs() > 0.01 {
                g.ok = false;
            }
            g.by = Offset::new(g.by.x.round(), g.by.y.round());
            let clip = &self.recording[g.clip as usize];
            let mut c = intersect(clip.m.map(clip.rect), screen);
            let mut q = clip.parent;
            while q != NONE && g.ok {
                let qc = &self.recording[q as usize];
                if square_clip(qc) {
                    c = intersect(c, qc.m.map(qc.rect));
                } else if !in_place(qc) {
                    g.ok = false; // (a layer's texture, a clip of another shape)
                }
                q = qc.parent;
            }
            g.exact = c;
            g.area = whole_in(c);
            if empty(g.area)
                || g.by.x.abs() >= g.area.width()
                || g.by.y.abs() >= g.area.height()
                || (g.by.x == 0.0 && g.by.y == 0.0)
            {
                g.ok = false;
            }
        }
        // The largest first; none overlapping one taken.
        let mut order: Vec<usize> = (0..groups.len()).collect();
        order.sort_by(|&a, &b| area(groups[b].area).total_cmp(&area(groups[a].area)));
        let mut taken: Vec<Rect> = Vec::new();
        for (position, &gi) in order.iter().enumerate() {
            let g_area = groups[gi].area;
            if groups[gi].ok && taken.iter().any(|&t| overlaps(t, g_area)) {
                groups[gi].ok = false;
            }
            if !groups[gi].ok {
                continue;
            }
            let id = position as u32 + 2;
            let (mut first, mut last_first) = (NONE, NONE);
            for &k in &groups[gi].moved {
                fill(self, k, id);
                let (s, p) = segment_of(self, k);
                first = first.min(s.first);
                last_first = last_first.min(p.first);
            }
            let by = groups[gi].by;
            // Under it all there, a color: the opaque rect covering the clip (or the white under everything; one that
            // changed is damage over all of it). What else touches the clip, before or after what moved, is damage
            // where it is and where moving would have taken it.
            let cover = cover_before(&self.recording, first as usize, g_area, self.viewport);
            let last_cover = cover_before(&self.frame, last_first as usize, g_area, self.viewport);
            let mut wrong = Vec::new();
            for i in cover.map_or(0, |c| c + 1)..self.recording.len() {
                let c = &self.recording[i];
                if self.group[i] != id && c.paints() && overlaps(c.bounds, g_area) {
                    wrong.push(intersect(c.bounds, g_area));
                }
            }
            for i in last_cover.map_or(0, |c| c + 1)..self.frame.len() {
                let c = &self.frame[i];
                let moved = c.bounds.translate(by);
                if self.last_group[i] != id && c.paints() && overlaps(moved, g_area) {
                    wrong.push(intersect(moved, g_area));
                }
            }
            // The pixels moved: what was where the clip is less how far, to there; the rest of the clip is new.
            let from = intersect(g_area, g_area.translate(-by));
            subtract(g_area, from.translate(by), &mut wrong);
            // Around it, the pixels the clip's edge cuts (none when that is on whole pixels).
            subtract(whole_out(groups[gi].exact), g_area, &mut wrong);
            let mut drawn = Vec::new(); // (as end() makes them areas)
            for &w in &wrong {
                join(&mut drawn, w);
            }
            let worth: f32 = drawn.iter().map(|&d| area(d)).sum();
            if worth >= area(g_area) {
                // as much drawn as without moving
                for &k in &groups[gi].moved {
                    fill(self, k, 1);
                }
                groups[gi].ok = false;
                continue;
            }
            taken.push(g_area);
            self.moves.push(Move { from, by });
            damage.extend(wrong);
        }
        for g in &groups {
            if !g.ok {
                rejected.extend(&g.moved);
            }
        }
        for k in rejected {
            // damage where it was and is
            let (s, p) = segment_of(self, k);
            add_damage(damage, &self.frame, p.first, p.count);
            add_damage(damage, &self.recording, s.first, s.count);
        }
    }

    /// The last recorded frame's last command that paints all of `area` (px) opaquely: a solid opaque rect, square
    /// to the screen, under nothing but translations, scales and rect clips. Nothing before it shows in `area`
    /// (`replay_from` leaves it out); none if no command covers it.
    pub fn opaque_cover(&self, area: Rect) -> Option<usize> {
        cover_before(&self.frame, self.frame.len(), area, self.frame_viewport)
    }

    /// Draws the last recorded frame into `canvas`: what touches `area`, or everything.
    pub fn replay(&self, canvas: &mut dyn GraphicsContext, area: Option<Rect>) {
        self.replay_from(canvas, area, 0);
    }

    /// The same, what comes before command `hidden` (`opaque_cover`'s) left out but its state.
    pub fn replay_from(&self, canvas: &mut dyn GraphicsContext, area: Option<Rect>, hidden: usize) {
        let mut whole = 0; // what comes before it is drawn wherever it is (a layer the canvas keeps)
        let mut i = 0;
        while i < self.frame.len() {
            let c = &self.frame[i];
            let skip = area.is_some_and(|a| i >= whole && !overlaps(c.bounds, a));
            let end = c.end as usize;
            if c.kind.is_push() {
                if (skip || end <= hidden) && end > i {
                    // nothing inside touches the area, or shows
                    i = end;
                    continue;
                }
                if c.kind == Kind::PushLayer {
                    canvas.key_layer(c.layer_key, c.layer_same);
                }
                replay_command(canvas, c, Offset::Zero);
                if c.kind == Kind::PushLayer && end > i {
                    match canvas.layer_content() {
                        LayerContent::Kept => {
                            i = end - 1; // (on to its pop)
                            continue;
                        }
                        LayerContent::DrawAll => whole = whole.max(end),
                        LayerContent::Draw => {}
                    }
                }
            } else if c.kind.is_pop() || (!skip && i >= hidden) {
                replay_command(canvas, c, Offset::Zero);
            }
            i += 1;
        }
    }

    // ---- Kept nodes ----

    /// A node drawn in a range copied whole this frame (it isn't in `occurrences` for it): 1, else 0.
    fn kept_before(&self, node: u64) -> u32 {
        if self.copied_ranges.is_empty() {
            return 0;
        }
        let Some(&last) = self.range_lookup.get(&node_key(node, 0)) else { return 0 };
        self.copied_ranges.iter().any(|&(from, to)| last >= from && last < to) as u32
    }

    /// Last frame's range `index` recorded again, not moved, drawn in the same state: its commands, segments, events
    /// and ranges as they were (but where they go in this recording, and what images hold now).
    fn copy_kept(&mut self, index: u32) {
        let r = self.frame_ranges[index as usize];
        self.segment_open = false; // (the parent's segment ends at its child)
        let commands = self.recording.len() as i64 - r.first_command as i64;
        let events = self.recording_events.get_mut().len() as i64 - r.first_event as i64;
        let segments = self.recording_segments.len() as i64 - r.first_segment as i64;
        let ranges = self.recording_ranges.len() as i64 - index as i64;
        let outer = if self.states.len() > 1 { self.state().push } else { NONE }; // what is around it now
        let moved = |i: u32, by: i64| (i as i64 + by) as u32;
        SHOWN.set(SHOWN.get() + (r.end_command - r.first_command) as u64); // (what it recorded last frame: taken as all showing)
        for i in r.first_command..r.end_command {
            let mut c = self.frame[i as usize].clone();
            c.parent = if c.parent != NONE && c.parent >= r.first_command { moved(c.parent, commands) } else { outer };
            if c.end != 0 {
                c.end = moved(c.end, commands);
            }
            // (What an image holds now: drawn into since, it changed.)
            match c.kind {
                Kind::Image => c.image_version = c.image.as_ref().map_or(0, ImageBitmap::version),
                Kind::ImageMask => {
                    let area = Rect::from_offset_size(c.p1, Size::new(c.p2.x, c.p2.y));
                    let image = c.image.as_ref().map_or(0, ImageBitmap::version);
                    let brush = c.brush.as_ref().map_or(0, |b| images_in(b, area));
                    c.image_version = image.wrapping_mul(1000003).wrapping_add(brush);
                }
                Kind::Gradient | Kind::Path | Kind::Vertices | Kind::TextBrush => {
                    if let Some(b) = &c.brush {
                        c.image_version = images_in(b, c.rect2);
                    }
                }
                _ => {}
            }
            self.recording.push(c);
        }
        for i in r.first_segment..r.end_segment {
            let mut s = self.frame_segments[i as usize];
            s.first = moved(s.first, commands);
            self.recording_segments.push(s);
        }
        let recording_events = self.recording_events.get_mut();
        for i in r.first_event..r.end_event {
            let mut e = self.frame_events[i as usize];
            e.at = moved(e.at, commands);
            if let EventKind::Begin { range, .. } = &mut e.kind {
                *range = moved(*range, ranges);
            }
            recording_events.push(e);
        }
        for i in index..r.end_range {
            let mut n = self.frame_ranges[i as usize];
            n.first_event = moved(n.first_event, events);
            n.end_event = moved(n.end_event, events);
            n.first_command = moved(n.first_command, commands);
            n.end_command = moved(n.end_command, commands);
            n.first_segment = moved(n.first_segment, segments);
            n.end_segment = moved(n.end_segment, segments);
            n.end_range = moved(n.end_range, ranges);
            self.recording_ranges.push(n);
        }
        self.nodes.last_mut().expect("a node").after = r.key; // (what the parent draws next follows this child)
        self.copied_ranges.push((index, r.end_range));
    }

    /// Last frame's range `index` recorded again, its node moved by `by` in what it is drawn in and drawn in the
    /// state it is in now (`exact`: the one it was drawn in), if what it left out and asked comes out as it did; if
    /// not, nothing is recorded.
    fn keep(&mut self, index: u32, by: Offset, exact: bool) -> bool {
        if exact && by.x == 0.0 && by.y == 0.0 {
            // (in the same state: as it was)
            self.copy_kept(index);
            return true;
        }
        // (Replayed into this list: the last frame set aside meanwhile.)
        let frame = std::mem::take(&mut self.frame);
        let events = std::mem::take(&mut self.frame_events);
        let ranges = std::mem::take(&mut self.frame_ranges);
        let same = self.keep_moved(&frame, &events, &ranges, index, by);
        self.frame = frame;
        self.frame_events = events;
        self.frame_ranges = ranges;
        same
    }

    /// Range `r` of the last frame (`frame`, `events`, `ranges`) recorded again as its node draws moved by `by` in
    /// what it is drawn in: as layout nodes draw, in its own space translated to where it is in its parent, so only
    /// that translation changes (there now, or gone at none; added where there was none); what is in it is recorded
    /// as it was (through the state it is drawn in now), what it left out and asked asked again there.
    fn keep_moved(&mut self, frame: &[Command], events: &[Event], ranges: &[Range], index: u32, by: Offset) -> bool {
        let r = ranges[index as usize];
        // Its translation: the first command, popped by the last, with nothing asked before or after it.
        let (first, end) = (r.first_command, r.end_command);
        let own = &events[r.first_event as usize + 1..(r.end_event as usize).saturating_sub(1)];
        let enclosing = first < end
            && frame[first as usize].kind == Kind::PushTranslation
            && frame[first as usize].end == end
            && own.iter().all(|e| e.at > first && e.at < end);
        let was = if enclosing { frame[first as usize].p1 } else { Offset::Zero };
        let whole = |v: f32| if (v - v.round()).abs() < 1e-3 { v.round() } else { v }; // (layout's whole px)
        let to = Offset::new(whole(was.x + by.x), whole(was.y + by.y));
        let translated = to != Offset::Zero;
        // What is recorded from here on goes again if anything comes out otherwise.
        let commands = self.recording.len();
        let segments = self.recording_segments.len();
        let recorded_events = self.recording_events.get_mut().len();
        let recorded_ranges = self.recording_ranges.len();
        let states = self.states.len();
        let nodes = self.nodes.len();
        let open = self.open_ranges.len();
        let segment_open = self.segment_open;
        let layers = self.layers_in_segment;
        self.kept_begun.clear();
        let mut same = true;
        let mut next = first;
        let last_event = r.end_event as usize - 1;
        for (i, e) in events.iter().enumerate().take(r.end_event as usize).skip(r.first_event as usize) {
            while next < e.at {
                if enclosing && next == first {
                    if translated {
                        self.push_translation(to);
                    }
                } else if enclosing && next + 1 == end {
                    if translated {
                        self.pop_translation();
                    }
                } else {
                    replay_command(self, &frame[next as usize], Offset::Zero);
                }
                next += 1;
            }
            if i == last_event && !enclosing && translated {
                self.pop_translation();
            }
            match e.kind {
                EventKind::Begin { range, node } => {
                    // Where it is in its parent: moved, the node kept; as it was, those in it.
                    let moved = if i == r.first_event as usize { by } else { Offset::Zero };
                    self.next_origin = ranges[range as usize].origin.map(|at| at + moved);
                    self.kept_begun.push(node);
                    self.begin_node(node);
                }
                EventKind::End => self.end_node(),
                EventKind::Culled(rect) => {
                    if self.shows(rect) {
                        same = false; // (it was left out: it would be drawn now)
                    } else {
                        self.event(EventKind::Culled(rect));
                    }
                }
                EventKind::Rejected(rect) => same = self.quick_reject(rect),
                EventKind::Accepted(rect) => same = !self.quick_reject(rect),
                // (what is drawn from the light doesn't move with the node)
                EventKind::Light => same = false,
            }
            if !same {
                break;
            }
            if i == r.first_event as usize && !enclosing && translated {
                self.push_translation(to);
            }
        }
        if same {
            return true;
        }
        self.recording.truncate(commands);
        self.recording_segments.truncate(segments);
        self.recording_events.get_mut().truncate(recorded_events);
        self.recording_ranges.truncate(recorded_ranges);
        self.states.truncate(states);
        self.nodes.truncate(nodes);
        self.open_ranges.truncate(open);
        self.segment_open = segment_open;
        self.layers_in_segment = layers;
        for node in std::mem::take(&mut self.kept_begun) {
            if let Some(n) = self.occurrences.get_mut(&node) {
                *n -= 1;
            }
        }
        self.next_origin = None;
        false
    }

    /// The light's center through the inverse of the transform drawn through (where it isn't one, as given).
    fn light_center_here(&self) -> Offset {
        let m = &self.state().m.m;
        let a = m[4] * m[8] - m[5] * m[7];
        let b = m[5] * m[6] - m[3] * m[8];
        let c = m[3] * m[7] - m[4] * m[6];
        let det = m[0] * a + m[1] * b + m[2] * c;
        let center = self.light.center;
        if det.abs() < 1e-12 {
            return center;
        }
        let i = [
            a,
            m[2] * m[7] - m[1] * m[8],
            m[1] * m[5] - m[2] * m[4],
            b,
            m[0] * m[8] - m[2] * m[6],
            m[2] * m[3] - m[0] * m[5],
            c,
            m[1] * m[6] - m[0] * m[7],
            m[0] * m[4] - m[1] * m[3],
        ];
        let (x, y) = (center.x, center.y); // (the screen's px, as the transform's)
        let w = i[6] * x + i[7] * y + i[8];
        if w.abs() <= 1e-12 || w.is_nan() {
            return center;
        }
        Offset::new((i[0] * x + i[1] * y + i[2]) / w, (i[3] * x + i[4] * y + i[5]) / w)
    }

    /// Whether the last frame recorded the same as `other`'s (where things are, within a thousandth of a px): what
    /// kept nodes recorded is what drawing them would have (the keep check). If not, where not.
    pub fn same_frame(&self, other: &DisplayList) -> Result<(), String> {
        if self.frame.len() != other.frame.len() {
            return Err(format!("{} commands, not {}", self.frame.len(), other.frame.len()));
        }
        if self.frame_segments.len() != other.frame_segments.len() {
            return Err("segments differ in number".into());
        }
        for (i, (a, b)) in self.frame_segments.iter().zip(&other.frame_segments).enumerate() {
            if a.key != b.key || a.first != b.first || a.count != b.count {
                return Err(format!("segment {i} differs"));
            }
        }
        for (i, (a, b)) in self.frame.iter().zip(&other.frame).enumerate() {
            // The fields that differ, by name.
            let mut fields = String::new();
            let mut check = |same: bool, name: &str| {
                if !same {
                    if !fields.is_empty() {
                        fields.push_str(", ");
                    }
                    fields.push_str(name);
                }
            };
            check(a.kind == b.kind, "kind");
            check(a.m.m.iter().zip(&b.m.m).all(|(x, y)| near(*x, *y)), "transform");
            check(near_rect(a.bounds, b.bounds), "bounds");
            check(near_rect(a.rect, b.rect), "rect");
            check(near_rect(a.rect2, b.rect2), "rect2");
            check(near_offset(a.p1, b.p1) && near_offset(a.p2, b.p2), "points");
            check(a.radii == b.radii, "radii");
            check(a.f1 == b.f1 && a.f2 == b.f2 && a.f3 == b.f3, "numbers");
            check(a.color == b.color, "color");
            check(a.flag == b.flag && a.clip == b.clip, "flag");
            check(a.quality == b.quality && a.blend_mode == b.blend_mode, "mode");
            check(a.text == b.text, "text");
            check(a.image == b.image && a.image_version == b.image_version, "image");
            check(a.stroke == b.stroke, "stroke");
            check(a.transform == b.transform, "layer transform");
            check(a.brush == b.brush, "brush");
            check(a.vertices == b.vertices, "vertices");
            check(a.filter == b.filter, "filter");
            check(a.effect == b.effect, "effect");
            check(a.shadow == b.shadow, "shadow");
            check(a.elevated == b.elevated, "elevation shadow");
            check(a.shaped == b.shaped, "shape shadow");
            check(a.path == b.path, "path");
            check(a.matrix == b.matrix, "matrix");
            // (A layer's key is its list's too: two lists' differ. It is made from the segments'.)
            check(a.parent == b.parent && a.end == b.end, "nesting");
            if !fields.is_empty() {
                let mut why = String::new();
                let _ = write!(
                    why,
                    "command {i} ({:?}, at ({}, {}, {} x {}) ({}, {})) differs in {fields}",
                    a.kind,
                    a.rect.left,
                    a.rect.top,
                    a.rect.width(),
                    a.rect.height(),
                    a.p1.x,
                    a.p1.y
                );
                return Err(why);
            }
        }
        Ok(())
    }
}

/// What `count` commands from `first` touch on the screen, as one rect (none if nothing).
fn add_damage(damage: &mut Vec<Rect>, commands: &[Command], first: u32, count: u32) {
    let all = commands[first as usize..(first + count) as usize]
        .iter()
        .filter(|c| !(c.kind.is_pop() || (c.kind.is_push() && c.kind != Kind::PushLayer) || empty(c.bounds)))
        .map(|c| c.bounds)
        .reduce(Rect::union);
    if let Some(all) = all {
        damage.push(all);
    }
}

/// The last command before `limit` in `commands` that paints all of `area` opaquely (`opaque_cover`).
fn cover_before(commands: &[Command], limit: usize, area: Rect, screen: Size) -> Option<usize> {
    // All of the pixels `area` touches (an edge of the cover in one would leave it part of what is under; none off
    // the screen, where there are no pixels).
    let need = intersect(whole_out(area), Rect::new(0.0, 0.0, screen.width, screen.height));
    struct Open {
        end: u32,
        /// What the clips so far let through.
        clip: Rect,
        /// Something the cover would show through: a layer, a blend mode, a filter, a clip not a rect.
        hides: bool,
    }
    let mut open: Vec<Open> = Vec::new();
    let mut cover = None;
    const FAR: f32 = 1e9;
    for (i, c) in commands.iter().enumerate().take(limit) {
        while open.last().is_some_and(|o| o.end as usize <= i) {
            open.pop();
        }
        let mut clip = open.last().map_or(Rect::new(-FAR, -FAR, FAR, FAR), |o| o.clip);
        let mut hides = open.last().is_some_and(|o| o.hides);
        match c.kind {
            Kind::PushClip => {
                if c.clip == ClipKind::Rect && !c.flag && c.m.square_to_screen() {
                    clip = intersect(clip, c.m.map(c.rect));
                } else {
                    hides = true;
                }
                open.push(Open { end: c.end, clip, hides });
            }
            // (what is drawn inside a transform comes with it)
            Kind::PushTranslation | Kind::PushTransform | Kind::PushAlias => {
                open.push(Open { end: c.end, clip, hides })
            }
            Kind::PushLayer | Kind::PushBlend | Kind::PushFilter => open.push(Open { end: c.end, clip, hides: true }),
            Kind::FillRoundRect => {
                let r = c.radii;
                let square = r.top_left <= 0.0 && r.top_right <= 0.0 && r.bottom_right <= 0.0 && r.bottom_left <= 0.0;
                if hides || !square || c.color.alpha() < 1.0 || !c.m.square_to_screen() {
                    continue;
                }
                if contains(intersect(c.m.map(c.rect), clip), need) {
                    cover = Some(i);
                }
            }
            _ => {}
        }
    }
    cover
}

/// `c` drawn into `canvas` moved by `by` (its place, as `same_moved` says it; `replay`'s, `keep`'s).
fn replay_command(canvas: &mut dyn GraphicsContext, c: &Command, by: Offset) {
    let r = |rect: Rect| rect.translate(by);
    let p = |o: Offset| o + by;
    let op = if c.flag { ClipOp::Difference } else { ClipOp::Intersect };
    let brush = || c.brush.as_ref().expect("a brush");
    let path = || c.path.as_ref().expect("a path");
    let shadow = || c.shadow.as_ref().expect("a shadow");
    let text = || TextDraw { baseline: p(c.p1), ..c.text.clone().expect("text") };
    match c.kind {
        Kind::PushLayer => match (&c.path, &c.effect) {
            (Some(outline), _) => canvas.push_outline_layer(r(c.rect), outline, c.f2, c.transform), // (the outline from its top left)
            (None, Some(effect)) => canvas.push_effect_layer(r(c.rect), effect, c.f2),
            (None, None) => canvas.push_layer(r(c.rect), c.radii, c.f2, c.transform),
        },
        Kind::PushClip => match c.clip {
            ClipKind::RoundRect => canvas.push_clip_round_rect(r(c.rect), c.radii, op),
            ClipKind::Path => canvas.push_clip_path(path(), p(c.p1), op),
            ClipKind::Rect => canvas.push_clip(r(c.rect)),
        },
        Kind::PushBlend => canvas.push_blend_mode(c.blend_mode),
        Kind::PushAlias => canvas.push_anti_alias(c.flag),
        Kind::PushFilter => canvas.push_color_filter(c.filter.as_ref().expect("a filter")),
        Kind::PushTransform => canvas.push_transform(&c.matrix), // (not moved: keep() doesn't move one)
        Kind::PushTranslation => canvas.push_translation(c.p1),
        Kind::PopBlend => canvas.pop_blend_mode(),
        Kind::PopAlias => canvas.pop_anti_alias(),
        Kind::PopFilter => canvas.pop_color_filter(),
        Kind::PopLayer => canvas.pop_layer(),
        Kind::PopClip => canvas.pop_clip(),
        Kind::PopTranslation => canvas.pop_translation(),
        Kind::PopTransform => canvas.pop_transform(),
        Kind::FillRoundRect => canvas.fill_round_rect(r(c.rect), c.radii, c.color),
        Kind::StrokeRoundRect => canvas.stroke_round_rect(r(c.rect), c.radii, c.f2, c.color),
        Kind::Line => canvas.draw_line(p(c.p1), p(c.p2), c.f2, c.color),
        Kind::Text => canvas.draw_text(&text(), c.color),
        Kind::TextBrush => canvas.draw_text_brush(&text(), brush(), r(c.rect2)),
        Kind::TextShadow => canvas.draw_text_shadow(&text(), c.color, c.f1),
        Kind::Shadow => canvas.draw_shadow(r(c.rect), c.radii, shadow(), c.flag),
        Kind::ShadowPath => canvas.draw_shadow_path(path(), p(c.p1), shadow(), c.flag),
        Kind::Backdrop => canvas.draw_backdrop(r(c.rect), c.effect.as_ref().expect("an effect"), c.f2),
        Kind::ElevationShadow => {
            let (outline, elevated) = c.elevated.as_ref().expect("an elevation shadow");
            canvas.draw_elevation_shadow(outline, p(c.p1), elevated);
        }
        Kind::ShapeShadow => {
            let s = c.shaped.as_ref().expect("a shape");
            let shape = RecordedOutline(s.outline.clone());
            canvas.draw_shape_shadow(r(c.rect), &shape, s.layout_direction, &s.density, shadow(), c.flag);
        }
        Kind::Arc => canvas.draw_arc(r(c.rect), c.f1, c.f2, c.f3, c.color, c.flag),
        Kind::Image => canvas.draw_image(c.image.as_ref().expect("an image"), c.rect2, r(c.rect), c.f1, c.quality),
        Kind::Gradient => canvas.fill_gradient(r(c.rect), c.radii, brush(), r(c.rect2)),
        Kind::Path => canvas.draw_path(path(), p(c.p1), brush(), r(c.rect2), c.stroke.as_ref().expect("a stroke")),
        Kind::Vertices => canvas.draw_vertices(
            c.vertices.as_ref().expect("vertices"),
            p(c.p1),
            c.blend_mode,
            brush(),
            r(c.rect2),
            c.f1,
        ),
        Kind::ImageMask => canvas.draw_image_mask(
            c.image.as_ref().expect("an image"),
            c.rect2,
            r(c.rect),
            brush(),
            Rect::from_offset_size(p(c.p1), Size::new(c.p2.x, c.p2.y)),
            c.f1,
            c.quality,
        ),
    }
}

// ---- The drawing surface ---------------------------------------------------------------------------------------

impl GraphicsContext for DisplayList {
    fn fill_round_rect(&mut self, rect: Rect, radii: CornerRadii, color: Color) {
        if color.alpha() == 0.0 {
            return;
        }
        if let Some(c) = self.draw(Kind::FillRoundRect, rect, 0.0) {
            (c.rect, c.radii, c.color) = (rect, radii, color);
        }
    }

    fn stroke_round_rect(&mut self, rect: Rect, radii: CornerRadii, width: f32, color: Color) {
        if color.alpha() == 0.0 {
            return;
        }
        if let Some(c) = self.draw(Kind::StrokeRoundRect, rect, width / 2.0) {
            (c.rect, c.radii, c.f2, c.color) = (rect, radii, width, color);
        }
    }

    fn draw_line(&mut self, from: Offset, to: Offset, width: f32, color: Color) {
        if color.alpha() == 0.0 {
            return;
        }
        let r = Rect::new(from.x.min(to.x), from.y.min(to.y), from.x.max(to.x), from.y.max(to.y));
        if let Some(c) = self.draw(Kind::Line, r, width) {
            (c.p1, c.p2, c.f2, c.color) = (from, to, width, color);
        }
    }

    fn draw_text(&mut self, text: &TextDraw, color: Color) {
        if text.text.is_empty() || color.alpha() == 0.0 {
            return;
        }
        if let Some(c) = self.text(Kind::Text, text, 0.0) {
            c.color = color;
        }
    }

    fn draw_text_brush(&mut self, text: &TextDraw, brush: &Brush, brush_area: Rect) {
        if text.text.is_empty() {
            return;
        }
        if let Some(c) = self.text(Kind::TextBrush, text, 0.0) {
            (c.brush, c.rect2, c.image_version) = (Some(brush.clone()), brush_area, images_in(brush, brush_area));
        }
    }

    fn draw_text_shadow(&mut self, text: &TextDraw, color: Color, blur_radius: f32) {
        if text.text.is_empty() || color.alpha() == 0.0 {
            return;
        }
        if let Some(c) = self.text(Kind::TextShadow, text, blur_radius.max(0.0)) {
            (c.color, c.f1) = (color, blur_radius);
        }
    }

    fn draw_arc(
        &mut self,
        oval: Rect,
        start_degrees: f32,
        sweep_degrees: f32,
        stroke_width: f32,
        color: Color,
        round_cap: bool,
    ) {
        if color.alpha() == 0.0 {
            return;
        }
        if let Some(c) = self.draw(Kind::Arc, oval, stroke_width / 2.0) {
            (c.rect, c.f1, c.f2, c.f3, c.color, c.flag) =
                (oval, start_degrees, sweep_degrees, stroke_width, color, round_cap);
        }
    }

    fn draw_image(&mut self, image: &ImageBitmap, src: Rect, dst: Rect, alpha: f32, quality: FilterQuality) {
        if alpha <= 0.0 {
            return;
        }
        if let Some(c) = self.draw(Kind::Image, dst, 0.0) {
            c.image_version = image.version();
            c.image = Some(image.clone());
            (c.rect, c.rect2, c.f1, c.quality) = (dst, src, alpha, quality);
        }
    }

    fn draw_image_mask(
        &mut self,
        image: &ImageBitmap,
        src: Rect,
        dst: Rect,
        brush: &Brush,
        brush_area: Rect,
        alpha: f32,
        quality: FilterQuality,
    ) {
        if alpha <= 0.0 {
            return;
        }
        if let Some(c) = self.draw(Kind::ImageMask, dst, 0.0) {
            c.image_version = image.version().wrapping_mul(1000003).wrapping_add(images_in(brush, brush_area));
            c.image = Some(image.clone());
            c.brush = Some(brush.clone());
            (c.rect, c.rect2, c.f1, c.quality) = (dst, src, alpha, quality);
            c.p1 = brush_area.top_left();
            c.p2 = Offset::new(brush_area.width(), brush_area.height());
        }
    }

    fn push_layer(&mut self, bounds: Rect, radii: CornerRadii, alpha: f32, transform: LayerTransform) {
        let s = self.state();
        let m = Projective::compose(&s.m, &Projective::layer(bounds, &transform));
        // Its texture is whole pixels around the bounds, composited with an anti-aliased edge.
        let area = intersect(m.map(bounds.inflate(1.0)).inflate(2.0), s.clip);
        let c = self.add(Kind::PushLayer);
        (c.rect, c.radii, c.f2, c.transform, c.bounds) = (bounds, radii, alpha, transform, area);
        let rest_clip = intersect(s.rest_m.map(bounds.inflate(1.0)).inflate(2.0), s.rest_clip);
        self.push_state(m, s.rest_m, area, rest_clip);
    }

    fn pop_layer(&mut self) {
        self.pop(Kind::PopLayer);
    }

    fn push_clip(&mut self, rect: Rect) {
        let s = self.state();
        let area = intersect(s.m.map(rect).inflate(1.0), s.clip); // backends round it to pixels
        let c = self.add(Kind::PushClip);
        (c.rect, c.bounds) = (rect, area);
        let rest_clip = intersect(s.rest_m.map(rect).inflate(1.0), s.rest_clip);
        self.push_state(s.m, s.rest_m, area, rest_clip);
    }

    fn push_clip_round_rect(&mut self, rect: Rect, radii: CornerRadii, op: ClipOp) {
        let s = self.state();
        // Inside: where the rect is; outside: anywhere the clips around it leave.
        let inside = op == ClipOp::Intersect;
        let area = if inside { intersect(s.m.map(rect).inflate(1.0), s.clip) } else { s.clip };
        let rest = if inside { intersect(s.rest_m.map(rect).inflate(1.0), s.rest_clip) } else { s.rest_clip };
        let c = self.add(Kind::PushClip);
        (c.rect, c.radii, c.clip, c.flag, c.bounds) = (rect, radii, ClipKind::RoundRect, !inside, area);
        self.push_state(s.m, s.rest_m, area, rest);
    }

    fn push_clip_path(&mut self, path: &Path, origin: Offset, op: ClipOp) {
        let s = self.state();
        let b = path.get_bounds().translate(origin);
        let inside = op == ClipOp::Intersect;
        let area = if inside { intersect(s.m.map(b).inflate(1.0), s.clip) } else { s.clip };
        let rest = if inside { intersect(s.rest_m.map(b).inflate(1.0), s.rest_clip) } else { s.rest_clip };
        let c = self.add(Kind::PushClip);
        (c.p1, c.clip, c.flag, c.bounds) = (origin, ClipKind::Path, !inside, area);
        c.path = Some(path.clone());
        self.push_state(s.m, s.rest_m, area, rest);
    }

    fn pop_clip(&mut self) {
        self.pop(Kind::PopClip);
    }

    fn push_translation(&mut self, by: Offset) {
        let s = self.state();
        let shift = Projective::translation(by);
        let c = self.add(Kind::PushTranslation);
        (c.p1, c.bounds) = (by, s.clip); // (what anything inside can touch: replay skips it when that's outside the area)
        let (m, rest_m) = (Projective::compose(&s.m, &shift), Projective::compose(&s.rest_m, &shift));
        self.push_state(m, rest_m, s.clip, s.rest_clip);
    }

    fn pop_translation(&mut self) {
        self.pop(Kind::PopTranslation);
    }

    fn push_transform(&mut self, matrix: &Matrix) {
        let s = self.state();
        let plane = Projective::plane(matrix);
        let c = self.add(Kind::PushTransform);
        (c.matrix, c.bounds) = (*matrix, s.clip); // (what anything inside can touch: replay skips it when that's outside the area)
        let (m, rest_m) = (Projective::compose(&s.m, &plane), Projective::compose(&s.rest_m, &plane));
        self.push_state(m, rest_m, s.clip, s.rest_clip);
    }

    fn pop_transform(&mut self) {
        self.pop(Kind::PopTransform);
    }

    fn push_effect_layer(&mut self, bounds: Rect, effect: &RenderEffect, alpha: f32) {
        let s = self.state();
        let c = self.add(Kind::PushLayer);
        (c.rect, c.f2) = (bounds, alpha);
        c.effect = Some(effect.clone());
        // It shows where the effect takes what it draws. That is all of what it draws within its bounds, off the
        // screen too (a blur brings it in): pop_layer() makes each of those commands' area its own, as a change to
        // any of them changes all of it.
        c.bounds = intersect(s.m.map(effect.output_bounds(bounds)).inflate(2.0), s.clip);
        let clip = s.m.map(bounds.inflate(1.0)).inflate(2.0);
        let rest_clip = s.rest_m.map(bounds.inflate(1.0)).inflate(2.0);
        self.push_state(s.m, s.rest_m, clip, rest_clip);
    }

    /// It shows where the effect takes what is under it, and reads that from as far around it as the effect reaches
    /// (`rect2`: there on the screen): `end()` widens damage that reaches either to both, as what is under it has to
    /// be drawn anew before it is read.
    fn draw_backdrop(&mut self, bounds: Rect, effect: &RenderEffect, alpha: f32) {
        let reads = effect.output_bounds(bounds);
        let on_screen = self.state().m.map(reads).inflate(2.0);
        if let Some(c) = self.draw(Kind::Backdrop, effect.output_bounds(reads), 0.0) {
            (c.rect, c.f2, c.rect2) = (bounds, alpha, on_screen);
            c.effect = Some(effect.clone());
        }
    }

    fn push_blend_mode(&mut self, mode: BlendMode) {
        self.push_plain(Kind::PushBlend).blend_mode = mode;
    }

    fn pop_blend_mode(&mut self) {
        self.pop(Kind::PopBlend);
    }

    fn push_anti_alias(&mut self, on: bool) {
        self.push_plain(Kind::PushAlias).flag = on;
    }

    fn pop_anti_alias(&mut self) {
        self.pop(Kind::PopAlias);
    }

    fn push_color_filter(&mut self, filter: &ColorFilter) {
        self.push_plain(Kind::PushFilter).filter = Some(*filter);
    }

    fn pop_color_filter(&mut self) {
        self.pop(Kind::PopFilter);
    }

    fn quick_reject(&self, rect: Rect) -> bool {
        let s = self.state();
        let rejected = empty(intersect(s.m.map(rect).inflate(1.0), s.clip))
            && empty(intersect(s.rest_m.map(rect).inflate(1.0), s.rest_clip));
        if self.keep_nodes {
            self.event(if rejected { EventKind::Rejected(rect) } else { EventKind::Accepted(rect) });
        }
        rejected
    }

    fn fill_gradient(&mut self, rect: Rect, radii: CornerRadii, brush: &Brush, brush_area: Rect) {
        if let Some(c) = self.draw(Kind::Gradient, rect, 0.0) {
            (c.rect, c.radii, c.rect2, c.image_version) = (rect, radii, brush_area, images_in(brush, brush_area));
            c.brush = Some(brush.clone());
        }
    }

    fn draw_vertices(
        &mut self,
        vertices: &Vertices,
        origin: Offset,
        blend_mode: BlendMode,
        brush: &Brush,
        brush_area: Rect,
        alpha: f32,
    ) {
        if vertices.positions().is_empty() || alpha <= 0.0 {
            return;
        }
        let b = vertices.bounds().translate(origin);
        // (a pixel's reach either way: its edges aren't anti-aliased)
        if let Some(c) = self.draw(Kind::Vertices, b, 1.0) {
            (c.p1, c.f1, c.blend_mode, c.rect2) = (origin, alpha, blend_mode, brush_area);
            c.image_version = images_in(brush, brush_area);
            c.vertices = Some(vertices.clone());
            c.brush = Some(brush.clone());
        }
    }

    fn draw_path(&mut self, path: &Path, origin: Offset, brush: &Brush, brush_area: Rect, stroke: &PathStroke) {
        if path.is_empty() {
            return;
        }
        let b = path.get_bounds().translate(origin);
        // A stroke reaches half its width out, more at mitered corners and square caps.
        let mut pad = 0.0;
        if stroke.hairline {
            pad = 1.0; // (a pixel)
        } else if stroke.width > 0.0 {
            let mut reach = if stroke.join == StrokeJoin::Miter { stroke.miter.max(1.0) } else { 1.0 };
            if stroke.cap == StrokeCap::Square {
                reach = reach.max(std::f32::consts::SQRT_2);
            }
            pad = stroke.width / 2.0 * reach;
        }
        if let Some(effect) = &stroke.path_effect {
            pad += effect.reach(); // (stamps reach past the path)
        }
        if let Some(c) = self.draw(Kind::Path, b, pad) {
            (c.p1, c.rect2, c.image_version) = (origin, brush_area, images_in(brush, brush_area));
            c.path = Some(path.clone());
            c.brush = Some(brush.clone());
            c.stroke = Some(stroke.clone());
        }
    }

    fn draw_shadow(&mut self, rect: Rect, radii: CornerRadii, shadow: &Shadow, inner: bool) {
        if shadow.color.alpha() <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        // A drop shadow reaches its spread, offset and three blur radii out; an inner one stays inside.
        let (mut reach, mut pad) = (rect, 0.0);
        if !inner {
            let grow = shadow.spread.max(0.0);
            let o = shadow.offset;
            reach = Rect::new(
                rect.left - grow + o.x.min(0.0),
                rect.top - grow + o.y.min(0.0),
                rect.right + grow + o.x.max(0.0),
                rect.bottom + grow + o.y.max(0.0),
            );
            pad = 3.0 * shadow.blur_radius.max(0.0);
        }
        if let Some(c) = self.draw(Kind::Shadow, reach, pad) {
            (c.rect, c.radii, c.flag) = (rect, radii, inner);
            c.shadow = Some(shadow.clone());
        }
    }

    fn draw_shadow_path(&mut self, outline: &Path, origin: Offset, shadow: &Shadow, inner: bool) {
        if shadow.color.alpha() <= 0.0 || outline.is_empty() {
            return;
        }
        let rect = outline.get_bounds().translate(origin);
        // As far as drawing it can reach: an inner one within the path, a drop shadow its blur around the path moved
        // and spread (a mitered spread's corners out to the miter limit).
        let reach = if inner { rect } else { shadow_path_reach(rect, shadow, false) };
        if let Some(c) = self.draw(Kind::ShadowPath, reach, 0.0) {
            (c.p1, c.flag) = (origin, inner);
            c.path = Some(outline.clone());
            c.shadow = Some(shadow.clone());
        }
    }

    /// A brushed one (not one color) as it is, so that the canvas draws it its own way if it has one; any other as
    /// the default breaks it down. It reaches as far as its bitmap (where it is cut).
    fn draw_shape_shadow(
        &mut self,
        rect: Rect,
        shape: &dyn Shape,
        layout_direction: LayoutDirection,
        density: &Density,
        shadow: &Shadow,
        inner: bool,
    ) {
        if shadow.brush.as_ref().is_none_or(Brush::is_solid) {
            return default_draw_shape_shadow(self, rect, shape, layout_direction, density, shadow, inner);
        }
        if shadow.alpha <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        let out = shadow.blur_radius.max(0.0) + shadow.spread.max(0.0);
        let reach = if inner { rect } else { rect.translate(shadow.offset).inflate(out) };
        if let Some(c) = self.draw(Kind::ShapeShadow, reach, 0.0) {
            (c.rect, c.flag) = (rect, inner);
            c.shadow = Some(shadow.clone());
            c.shaped = Some(ShapedShadow {
                outline: shape.create_outline(rect.size(), layout_direction, density),
                layout_direction,
                density: *density,
            });
        }
    }

    fn light(&self) -> ShadowLight {
        self.light
    }

    fn light_center(&self) -> Offset {
        if self.keep_nodes {
            self.event(EventKind::Light);
        }
        self.light_center_here()
    }

    /// It reaches as far as Skia's bounds for it say (`ElevationShadow::reach`).
    fn draw_elevation_shadow(&mut self, outline: &Outline, origin: Offset, shadow: &ElevationShadow) {
        let b = outline.bounds().translate(origin);
        let reach = shadow.reach(b, &self.light, self.light_center_here());
        if let Some(c) = self.draw(Kind::ElevationShadow, reach, 0.0) {
            c.p1 = origin;
            c.elevated = Some((outline.clone(), *shadow));
        }
    }

    fn push_outline_layer(&mut self, bounds: Rect, outline: &Path, alpha: f32, transform: LayerTransform) {
        self.push_layer(bounds, CornerRadii::Zero, alpha, transform);
        self.recording.last_mut().expect("just pushed").path = Some(outline.clone()); // replayed as an outline layer
    }

    fn begin_node(&mut self, node: u64) {
        self.segment_open = false;
        // (A node drawn twice: a shared element's overlay, a magnifier's; once in a copied range too.)
        let drawn = self.occurrences.entry(node).or_insert(0);
        let occurrence = *drawn;
        *drawn += 1;
        let key = node_key(node, occurrence + self.kept_before(node));
        self.nodes.push(OpenNode { key, after: 0, shown_before: SHOWN.get() });
        if !self.keep_nodes {
            return;
        }
        let range = self.recording_ranges.len() as u32;
        self.open_ranges.push(range);
        let first_event = self.recording_events.get_mut().len() as u32;
        self.recording_ranges.push(Range {
            key,
            first_event,
            end_event: 0,
            first_command: self.recording.len() as u32,
            end_command: 0,
            origin: self.next_origin.take(),
            entry: self.state(),
            first_segment: self.recording_segments.len() as u32,
            end_segment: 0,
            end_range: 0,
        });
        self.event(EventKind::Begin { range, node });
    }

    fn end_node(&mut self) {
        if self.nodes.len() <= 1 {
            return;
        }
        let ended = self.nodes.pop().expect("a node");
        self.ended_shows = SHOWN.get() != ended.shown_before;
        // What the parent draws next follows this child: a child added before it leaves that alone.
        self.nodes.last_mut().expect("the root").after = ended.key;
        self.segment_open = false;
        if !self.keep_nodes {
            return;
        }
        let Some(open) = self.open_ranges.pop() else { return };
        self.event(EventKind::End);
        let end_event = self.recording_events.get_mut().len() as u32;
        let end_range = self.recording_ranges.len() as u32;
        let r = &mut self.recording_ranges[open as usize];
        r.end_event = end_event;
        r.end_command = self.recording.len() as u32;
        r.end_segment = self.recording_segments.len() as u32;
        r.end_range = end_range;
    }

    fn ended_node_shows(&self) -> bool {
        self.ended_shows
    }

    fn record_node(&mut self, node: u64, origin: Offset, unchanged: bool) -> NodeRecording {
        if !self.keep_nodes {
            return NodeRecording::Other;
        }
        if self.occurrences.get(&node).is_some_and(|&n| n > 0) || self.kept_before(node) > 0 {
            return NodeRecording::Other;
        }
        // (Drawn in the same clips through the same transforms but for where they take it: what it is drawn in
        // moved, a list scrolled; `origin` is where it is in that.)
        if unchanged && let Some(&last) = self.range_lookup.get(&node_key(node, 0)) {
            let r = self.frame_ranges[last as usize];
            let s = self.state();
            let entry = r.entry;
            if entry.clip == s.clip
                && entry.rest_clip == s.rest_clip
                && s.m.same_but_translation(&entry.m)
                && s.rest_m.same_but_translation(&entry.rest_m)
                && let Some(at) = r.origin
                && self.keep(last, origin - at, entry.m == s.m && entry.rest_m == s.rest_m)
            {
                self.kept_nodes += 1;
                return NodeRecording::Kept;
            }
        }
        self.next_origin = Some(origin);
        self.drawn_nodes += 1;
        NodeRecording::Fresh
    }
}
