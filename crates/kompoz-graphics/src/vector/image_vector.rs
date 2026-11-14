//! Vector images (Compose's ImageVector): a tree of paths ([`VectorPath`]) and groups ([`VectorGroup`]) in a
//! viewport of its own, made by an [`ImageVectorBuilder`], Compose's defaults for them (Vector.kt), and what a
//! painter needs to know of one before it draws it (whether it paints in one color, the filter it is drawn
//! through).

use super::builder::{PathBuilder, PathData};
use super::node::PathNode;
use super::parser::add_path_nodes;
use super::to_path::to_path;
use crate::brush::{BlendMode, Brush, ColorFilter, SolidColor};
use crate::color::Color;
use crate::image::ImageBitmapConfig;
use crate::matrix::Matrix;
use crate::path::{Path, PathFillType, PathMeasure, StrokeCap, StrokeJoin};
use crate::unit::Dp;
use kompoz_runtime::named;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

// ---- Compose's defaults (Vector.kt) ------------------------------------------------------------------------------

pub const DefaultGroupName: &str = "";
pub const DefaultRotation: f32 = 0.0;
pub const DefaultPivotX: f32 = 0.0;
pub const DefaultPivotY: f32 = 0.0;
pub const DefaultScaleX: f32 = 1.0;
pub const DefaultScaleY: f32 = 1.0;
pub const DefaultTranslationX: f32 = 0.0;
pub const DefaultTranslationY: f32 = 0.0;
pub const DefaultPathName: &str = "";
/// A stroke's width where none is given (Compose's: 0, a hairline).
pub const DefaultStrokeLineWidth: f32 = 0.0;
pub const DefaultStrokeLineMiter: f32 = 4.0;
pub const DefaultTrimPathStart: f32 = 0.0;
pub const DefaultTrimPathEnd: f32 = 1.0;
pub const DefaultTrimPathOffset: f32 = 0.0;
pub const DefaultStrokeLineCap: StrokeCap = StrokeCap::Butt;
pub const DefaultStrokeLineJoin: StrokeJoin = StrokeJoin::Miter;
pub const DefaultTintBlendMode: BlendMode = BlendMode::SrcIn;
pub const DefaultTintColor: Color = Color::Transparent;
pub const DefaultFillType: PathFillType = PathFillType::NonZero;

// ---- Paths and groups --------------------------------------------------------------------------------------------

/// One path of a vector (Compose's VectorPath), in the vector's viewport units: its data, filled with `fill` (none:
/// not) and stroked with `stroke` (none: not; a width of 0 a hairline), each at its alpha, only the piece from
/// `trim_path_start` to `trim_path_end` drawn (fractions of its first contour's length, both moved on by
/// `trim_path_offset`, wrapping round its end, as Android's). Gradients are in the units it is drawn in. Its
/// default is Compose's defaults; equal by all of it.
#[derive(Clone, PartialEq, Debug)]
pub struct VectorPath {
    pub name: String,
    pub path_data: Vec<PathNode>,
    pub path_fill_type: PathFillType,
    pub fill: Option<Brush>,
    pub fill_alpha: f32,
    pub stroke: Option<Brush>,
    pub stroke_alpha: f32,
    pub stroke_line_width: f32,
    pub stroke_line_cap: StrokeCap,
    pub stroke_line_join: StrokeJoin,
    pub stroke_line_miter: f32,
    pub trim_path_start: f32,
    pub trim_path_end: f32,
    pub trim_path_offset: f32,
}

impl Default for VectorPath {
    fn default() -> VectorPath {
        VectorPath {
            name: DefaultPathName.into(),
            path_data: Vec::new(),
            path_fill_type: DefaultFillType,
            fill: None,
            fill_alpha: 1.0,
            stroke: None,
            stroke_alpha: 1.0,
            stroke_line_width: DefaultStrokeLineWidth,
            stroke_line_cap: DefaultStrokeLineCap,
            stroke_line_join: DefaultStrokeLineJoin,
            stroke_line_miter: DefaultStrokeLineMiter,
            trim_path_start: DefaultTrimPathStart,
            trim_path_end: DefaultTrimPathEnd,
            trim_path_offset: DefaultTrimPathOffset,
        }
    }
}

impl VectorPath {
    /// The path it draws (Compose's PathComponent's render path): its data's, in its fill type; trimmed, the piece
    /// of its first contour from its trim start to its trim end, as Android's PathMeasure measures it (round its
    /// end where the start comes after the end; filled non-zero, as Compose's render path is).
    pub fn render_path(&self) -> Path {
        let mut path = to_path(&self.path_data);
        path.fill_type = self.path_fill_type;
        if self.trim_path_start == DefaultTrimPathStart && self.trim_path_end == DefaultTrimPathEnd {
            return path;
        }
        let mut measure = PathMeasure::new();
        measure.set_path(&path, false);
        let length = measure.length();
        // (Kotlin's % on floats: the dividend's sign, as Rust's)
        let start = ((self.trim_path_start + self.trim_path_offset) % 1.0) * length;
        let end = ((self.trim_path_end + self.trim_path_offset) % 1.0) * length;
        let mut trimmed = Path::new();
        if start > end {
            let mut piece = Path::new();
            measure.get_segment(start, length, &mut piece, true);
            trimmed.add_path(&piece, crate::geometry::Offset::Zero);
            piece.reset();
            measure.get_segment(0.0, end, &mut piece, true);
            trimmed.add_path(&piece, crate::geometry::Offset::Zero);
        } else {
            measure.get_segment(start, end, &mut trimmed, true);
        }
        trimmed
    }
}

/// A group of a vector's paths and groups (Compose's VectorGroup): what is in it scaled and turned (degrees,
/// clockwise) about its pivot, then moved, and clipped to its clip path (in its own units; none: not clipped), all
/// in its parent's units. Its default is Compose's defaults; equal by all of it.
#[derive(Clone, PartialEq, Debug)]
pub struct VectorGroup {
    pub name: String,
    pub rotation: f32,
    pub pivot_x: f32,
    pub pivot_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub translation_x: f32,
    pub translation_y: f32,
    pub clip_path_data: Vec<PathNode>,
    /// Drawn in order.
    pub children: Vec<VectorNode>,
}

impl Default for VectorGroup {
    fn default() -> VectorGroup {
        VectorGroup {
            name: DefaultGroupName.into(),
            rotation: DefaultRotation,
            pivot_x: DefaultPivotX,
            pivot_y: DefaultPivotY,
            scale_x: DefaultScaleX,
            scale_y: DefaultScaleY,
            translation_x: DefaultTranslationX,
            translation_y: DefaultTranslationY,
            clip_path_data: Vec::new(),
            children: Vec::new(),
        }
    }
}

impl VectorGroup {
    /// How many it holds (Compose's size).
    pub fn size(&self) -> usize {
        self.children.len()
    }
    /// What it holds at `index` (Compose's get), if it holds that many.
    pub fn get(&self, index: usize) -> Option<&VectorNode> {
        self.children.get(index)
    }
    pub fn iter(&self) -> std::slice::Iter<'_, VectorNode> {
        self.children.iter()
    }

    /// What it draws what is in it through, in its parent's units (Compose's GroupComponent's group matrix): moved
    /// by its translation and pivot, turned, scaled, moved back from its pivot.
    pub fn group_matrix(&self) -> Matrix {
        let mut m = Matrix::new();
        m.translate(self.translation_x + self.pivot_x, self.translation_y + self.pivot_y, 0.0);
        m.rotate_z(self.rotation);
        m.scale(self.scale_x, self.scale_y, 1.0);
        m.translate(-self.pivot_x, -self.pivot_y, 0.0);
        m
    }
}

impl std::ops::Index<usize> for VectorGroup {
    type Output = VectorNode;
    fn index(&self, index: usize) -> &VectorNode {
        &self.children[index]
    }
}

impl<'a> IntoIterator for &'a VectorGroup {
    type Item = &'a VectorNode;
    type IntoIter = std::slice::Iter<'a, VectorNode>;
    fn into_iter(self) -> Self::IntoIter {
        self.children.iter()
    }
}

/// What a group holds (Compose's VectorNode): a path or a group.
// (a vector's nodes are mostly paths: boxing them would cost an allocation each to save nothing)
#[allow(clippy::large_enum_variant)]
#[derive(Clone, PartialEq, Debug)]
pub enum VectorNode {
    Path(VectorPath),
    Group(VectorGroup),
}

impl VectorNode {
    pub fn path(&self) -> Option<&VectorPath> {
        match self {
            VectorNode::Path(p) => Some(p),
            VectorNode::Group(_) => None,
        }
    }
    pub fn group(&self) -> Option<&VectorGroup> {
        match self {
            VectorNode::Group(g) => Some(g),
            VectorNode::Path(_) => None,
        }
    }
}

impl From<VectorPath> for VectorNode {
    fn from(path: VectorPath) -> VectorNode {
        VectorNode::Path(path)
    }
}
impl From<VectorGroup> for VectorNode {
    fn from(group: VectorGroup) -> VectorNode {
        VectorNode::Group(group)
    }
}

// ---- One color ---------------------------------------------------------------------------------------------------

/// Compose's GroupComponent.markTintFor…: whether every color a group paints with is a color of one rgb (no
/// gradients), and that color (unspecified where none, or not one).
struct Tint {
    tintable: bool,
    color: Color,
}

impl Tint {
    fn of(group: &VectorGroup) -> Tint {
        let mut t = Tint { tintable: true, color: Color::Unspecified };
        for node in group {
            match node {
                VectorNode::Path(p) => {
                    t.mark_brush(p.fill.as_ref());
                    t.mark_brush(p.stroke.as_ref());
                }
                VectorNode::Group(g) => {
                    let child = Tint::of(g);
                    if child.tintable && t.tintable {
                        t.mark_color(child.color);
                    } else {
                        t.mark_not_tintable();
                    }
                }
            }
        }
        t
    }

    fn mark_brush(&mut self, brush: Option<&Brush>) {
        if !self.tintable {
            return;
        }
        if let Some(brush) = brush {
            match brush.solid_color() {
                Some(c) => self.mark_color(c),
                // (a gradient needs its own colors in the bitmap)
                None => self.mark_not_tintable(),
            }
        }
    }

    fn mark_color(&mut self, c: Color) {
        if !self.tintable || c.is_unspecified() {
            return;
        }
        if self.color.is_unspecified() {
            self.color = c;
        } else if !(self.color.red() == c.red() && self.color.green() == c.green() && self.color.blue() == c.blue()) {
            self.mark_not_tintable();
        }
    }

    fn mark_not_tintable(&mut self) {
        self.tintable = false;
        self.color = Color::Unspecified;
    }
}

/// Whether a bitmap drawn through `filter` shows the same whether it holds its colors or its alpha alone (Compose's
/// tintableWithAlphaMask): no filter, or a tint by SrcIn or SrcOver.
pub fn tintable_with_alpha_mask(filter: Option<&ColorFilter>) -> bool {
    match filter {
        None => true,
        Some(ColorFilter::BlendMode { blend_mode, .. }) => {
            *blend_mode == BlendMode::SrcIn || *blend_mode == BlendMode::SrcOver
        }
        Some(_) => false,
    }
}

// ---- Vector images -----------------------------------------------------------------------------------------------

struct ImageVectorData {
    name: String,
    default_width: Dp,
    default_height: Dp,
    viewport_width: f32,
    viewport_height: f32,
    root: VectorGroup,
    tint_color: Color,
    tint_blend_mode: BlendMode,
    auto_mirror: bool,
    id: u64,
    tint: Tint,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A vector image (Compose's ImageVector): a tree of paths and groups ([`root`](ImageVector::root)) in a viewport of
/// its own (24 x 24 for Material icons), its size where nothing else gives one, the color it is tinted with and how
/// (unspecified: it draws in its own colors), and whether it is drawn mirrored in a right-to-left layout (a back
/// arrow points to the start). Made by an [`ImageVectorBuilder`] ([`ImageVector::Builder`]), from SVG path data
/// ([`from_svg_path`](ImageVector::from_svg_path)) or an Android vector drawable
/// ([`from_vector_xml`](ImageVector::from_vector_xml)). Copies share it; equal by all of it, not by its id.
#[derive(Clone)]
pub struct ImageVector {
    data: Arc<ImageVectorData>,
}

impl ImageVector {
    /// A builder (Compose's ImageVector.Builder), every parameter given; Compose's defaults: name
    /// [`DefaultGroupName`], tint color `Color::Unspecified`, tint blend mode `BlendMode::SrcIn`, not auto mirrored
    /// ([`ImageVectorBuilder::new`] takes them).
    #[allow(clippy::too_many_arguments)]
    pub fn Builder(
        name: &str,
        default_width: Dp,
        default_height: Dp,
        viewport_width: f32,
        viewport_height: f32,
        tint_color: Color,
        tint_blend_mode: BlendMode,
        auto_mirror: bool,
    ) -> ImageVectorBuilder {
        ImageVectorBuilder {
            name: name.into(),
            default_width,
            default_height,
            viewport_width,
            viewport_height,
            tint_color,
            tint_blend_mode,
            auto_mirror,
            nodes: vec![VectorGroup::default()],
            is_consumed: false,
        }
    }

    /// One path of SVG path data filled black (as Compose's Material icons are), its default size its viewport's in
    /// dp.
    pub fn from_svg_path(path_data: &str, viewport_width: f32, viewport_height: f32) -> ImageVector {
        ImageVectorBuilder::new(Dp(viewport_width), Dp(viewport_height), viewport_width, viewport_height)
            .add_path_of(VectorPath {
                path_data: add_path_nodes(path_data),
                fill: Some(SolidColor(Color::Black)),
                ..VectorPath::default()
            })
            .build()
    }

    pub fn name(&self) -> &str {
        &self.data.name
    }
    pub fn default_width(&self) -> Dp {
        self.data.default_width
    }
    pub fn default_height(&self) -> Dp {
        self.data.default_height
    }
    pub fn viewport_width(&self) -> f32 {
        self.data.viewport_width
    }
    pub fn viewport_height(&self) -> f32 {
        self.data.viewport_height
    }
    pub fn root(&self) -> &VectorGroup {
        &self.data.root
    }
    /// The color it is tinted with (unspecified: none).
    pub fn tint_color(&self) -> Color {
        self.data.tint_color
    }
    pub fn tint_blend_mode(&self) -> BlendMode {
        self.data.tint_blend_mode
    }
    pub fn auto_mirror(&self) -> bool {
        self.data.auto_mirror
    }
    /// Its own, among all built (Compose's genId): what is drawn of it is kept by it.
    pub fn id(&self) -> u64 {
        self.data.id
    }

    /// Whether everything it paints is in one rgb, no gradients (Compose's GroupComponent.isTintable of its root).
    pub fn is_tintable(&self) -> bool {
        self.data.tint.tintable
    }
    /// That one color (Compose's GroupComponent.tintColor of its root): unspecified where it paints nothing, or not
    /// in one rgb.
    pub fn root_tint_color(&self) -> Color {
        self.data.tint.color
    }
    /// Whether it paints in one color (Compose's VectorComponent's isOneColor): drawn as an alpha mask where its
    /// filters allow ([`cache_bitmap_config`](ImageVector::cache_bitmap_config)).
    pub fn is_one_color(&self) -> bool {
        self.is_tintable() && self.root_tint_color().is_specified()
    }
    /// Its tint as a filter (Compose's VectorPainter's intrinsic color filter): none unless its tint color is
    /// specified.
    pub fn intrinsic_color_filter(&self) -> Option<ColorFilter> {
        self.tint_color().is_specified().then(|| ColorFilter::tint(self.tint_color(), self.tint_blend_mode()))
    }
    /// What a painter keeps it drawn in, drawing it through `color_filter` (Compose's VectorComponent): an Alpha8
    /// bitmap where it is one color and both its own tint and that filter show the same through its alpha alone
    /// ([`tintable_with_alpha_mask`]), else an Argb8888 one.
    pub fn cache_bitmap_config(&self, color_filter: Option<&ColorFilter>) -> ImageBitmapConfig {
        let intrinsic = self.intrinsic_color_filter();
        if self.is_one_color() && tintable_with_alpha_mask(intrinsic.as_ref()) && tintable_with_alpha_mask(color_filter)
        {
            ImageBitmapConfig::Alpha8
        } else {
            ImageBitmapConfig::Argb8888
        }
    }
    /// What a painter draws that bitmap through (Compose's VectorComponent): `color_filter`, else its own tint, else
    /// (an Alpha8 one) a tint of its one color, opaque.
    pub fn draw_color_filter(&self, color_filter: Option<&ColorFilter>) -> Option<ColorFilter> {
        if let Some(f) = color_filter {
            return Some(*f);
        }
        if let Some(f) = self.intrinsic_color_filter() {
            return Some(f);
        }
        (self.cache_bitmap_config(None) == ImageBitmapConfig::Alpha8).then(|| {
            let c = self.root_tint_color();
            ColorFilter::tint(Color::new_in(c.red(), c.green(), c.blue(), 1.0, c.color_space()), BlendMode::SrcIn)
        })
    }
}

/// Compose's equals: by all of it but its id.
impl PartialEq for ImageVector {
    fn eq(&self, o: &ImageVector) -> bool {
        let (x, y) = (&*self.data, &*o.data);
        Arc::ptr_eq(&self.data, &o.data)
            || (x.name == y.name
                && x.default_width == y.default_width
                && x.default_height == y.default_height
                && x.viewport_width == y.viewport_width
                && x.viewport_height == y.viewport_height
                && x.root == y.root
                && x.tint_color == y.tint_color
                && x.tint_blend_mode == y.tint_blend_mode
                && x.auto_mirror == y.auto_mirror)
    }
}

impl fmt::Debug for ImageVector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let d = &*self.data;
        f.debug_struct("ImageVector")
            .field("name", &d.name)
            .field("default_width", &d.default_width)
            .field("default_height", &d.default_height)
            .field("viewport_width", &d.viewport_width)
            .field("viewport_height", &d.viewport_height)
            .field("root", &d.root)
            .field("tint_color", &d.tint_color)
            .field("tint_blend_mode", &d.tint_blend_mode)
            .field("auto_mirror", &d.auto_mirror)
            .finish()
    }
}

// ---- The builder -------------------------------------------------------------------------------------------------

/// Builds a vector (Compose's ImageVector.Builder): paths drawn in order, in groups (nested) that move, scale, turn
/// and clip what is in them. Compose's calls (`add_group`, `add_path`, `group`, `path`) take their parameters named,
/// with defaults, in a `#[kompozable]` body (`b.add_path(nodes, fill = Some(SolidColor(red)))`); from plain code they
/// take what has no default. The `…_of` ones take a whole [`VectorPath`] or [`VectorGroup`]. All of them chain:
///
/// ```
/// # use kompoz_graphics::*;
/// let badge = ImageVectorBuilder::new(Dp(48.0), Dp(48.0), 24.0, 24.0)
///     .add_path_of(VectorPath {
///         path_data: add_path_nodes("M12,2a10,10 0 1,0 0.01,0z"),
///         fill: Some(SolidColor(Color::hex(0x1E88E5))),
///         ..VectorPath::default()
///     })
///     .group_of(VectorGroup { rotation: 45.0, pivot_x: 12.0, pivot_y: 12.0, ..VectorGroup::default() }, |b| {
///         b.path_of(
///             VectorPath { stroke: Some(SolidColor(Color::White)), stroke_line_width: 2.0, ..VectorPath::default() },
///             |p| {
///                 p.move_to(7.0, 12.0).line_to(17.0, 12.0);
///             },
///         );
///     })
///     .build();
/// assert_eq!(badge.root().size(), 2);
/// ```
///
/// Single use, as Compose's: a call after [`build`](ImageVectorBuilder::build) panics, as Compose throws.
#[derive(Clone, Debug)]
pub struct ImageVectorBuilder {
    name: String,
    default_width: Dp,
    default_height: Dp,
    viewport_width: f32,
    viewport_height: f32,
    tint_color: Color,
    tint_blend_mode: BlendMode,
    auto_mirror: bool,
    /// The root, then the open groups.
    nodes: Vec<VectorGroup>,
    is_consumed: bool,
}

impl ImageVectorBuilder {
    /// A builder with Compose's defaults for the rest (no name, no tint, not auto mirrored).
    pub fn new(default_width: Dp, default_height: Dp, viewport_width: f32, viewport_height: f32) -> ImageVectorBuilder {
        ImageVector::Builder(
            DefaultGroupName,
            default_width,
            default_height,
            viewport_width,
            viewport_height,
            Color::Unspecified,
            DefaultTintBlendMode,
            false,
        )
    }

    #[track_caller]
    fn ensure_not_consumed(&self) {
        if self.is_consumed {
            panic!("ImageVector.Builder is single use, create a new instance to create a new ImageVector");
        }
    }

    fn current_group(&mut self) -> &mut VectorGroup {
        // (the root is always there: clear_group keeps it)
        let last = self.nodes.len() - 1;
        &mut self.nodes[last]
    }

    /// What is added after it goes in `group` (after the children it has), inside the open groups, until its
    /// [`clear_group`](ImageVectorBuilder::clear_group).
    #[track_caller]
    pub fn add_group_of(&mut self, group: VectorGroup) -> &mut ImageVectorBuilder {
        self.ensure_not_consumed();
        self.nodes.push(group);
        self
    }

    /// The group last added closed: what is added after it goes in the one it is in. With none open, nothing (where
    /// Compose's throws).
    #[track_caller]
    pub fn clear_group(&mut self) -> &mut ImageVectorBuilder {
        self.ensure_not_consumed();
        if self.nodes.len() > 1 {
            let popped = self.nodes.pop().unwrap_or_default();
            self.current_group().children.push(VectorNode::Group(popped));
        }
        self
    }

    /// `path` added to the open group.
    #[track_caller]
    pub fn add_path_of(&mut self, path: VectorPath) -> &mut ImageVectorBuilder {
        self.ensure_not_consumed();
        self.current_group().children.push(VectorNode::Path(path));
        self
    }

    /// `group`, what `block` adds in it.
    #[track_caller]
    pub fn group_of(
        &mut self,
        group: VectorGroup,
        block: impl FnOnce(&mut ImageVectorBuilder),
    ) -> &mut ImageVectorBuilder {
        self.add_group_of(group);
        block(self);
        self.clear_group()
    }

    /// A path of what `path_builder` makes, in the paints of `path` (its own data replaced).
    #[track_caller]
    pub fn path_of(
        &mut self,
        path: VectorPath,
        path_builder: impl FnOnce(&mut PathBuilder),
    ) -> &mut ImageVectorBuilder {
        self.add_path_of(VectorPath { path_data: PathData(path_builder), ..path })
    }

    /// The vector (the groups still open closed).
    #[track_caller]
    pub fn build(&mut self) -> ImageVector {
        self.ensure_not_consumed();
        while self.nodes.len() > 1 {
            self.clear_group();
        }
        let root = std::mem::take(&mut self.nodes[0]);
        self.is_consumed = true;
        let tint = Tint::of(&root);
        ImageVector {
            data: Arc::new(ImageVectorData {
                name: std::mem::take(&mut self.name),
                default_width: self.default_width,
                default_height: self.default_height,
                viewport_width: self.viewport_width,
                viewport_height: self.viewport_height,
                root,
                tint_color: self.tint_color,
                tint_blend_mode: self.tint_blend_mode,
                auto_mirror: self.auto_mirror,
                id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
                tint,
            }),
        }
    }
}

#[named]
#[allow(clippy::too_many_arguments)]
impl ImageVectorBuilder {
    /// Compose's addGroup: a group of these (Compose's defaults) open, what is added after it in it.
    #[track_caller]
    pub fn add_group(
        &mut self,
        #[default(DefaultGroupName)] name: &str,
        #[default(DefaultRotation)] rotate: f32,
        #[default(DefaultPivotX)] pivot_x: f32,
        #[default(DefaultPivotY)] pivot_y: f32,
        #[default(DefaultScaleX)] scale_x: f32,
        #[default(DefaultScaleY)] scale_y: f32,
        #[default(DefaultTranslationX)] translation_x: f32,
        #[default(DefaultTranslationY)] translation_y: f32,
        #[default(Vec::new())] clip_path_data: Vec<PathNode>,
    ) -> &mut ImageVectorBuilder {
        self.add_group_of(VectorGroup {
            name: name.into(),
            rotation: rotate,
            pivot_x,
            pivot_y,
            scale_x,
            scale_y,
            translation_x,
            translation_y,
            clip_path_data,
            children: Vec::new(),
        })
    }

    /// Compose's addPath: a path of `path_data` in these paints (Compose's defaults: neither filled nor stroked).
    #[track_caller]
    pub fn add_path(
        &mut self,
        path_data: Vec<PathNode>,
        #[default(DefaultFillType)] path_fill_type: PathFillType,
        #[default(DefaultPathName)] name: &str,
        #[default(None)] fill: Option<Brush>,
        #[default(1.0)] fill_alpha: f32,
        #[default(None)] stroke: Option<Brush>,
        #[default(1.0)] stroke_alpha: f32,
        #[default(DefaultStrokeLineWidth)] stroke_line_width: f32,
        #[default(DefaultStrokeLineCap)] stroke_line_cap: StrokeCap,
        #[default(DefaultStrokeLineJoin)] stroke_line_join: StrokeJoin,
        #[default(DefaultStrokeLineMiter)] stroke_line_miter: f32,
        #[default(DefaultTrimPathStart)] trim_path_start: f32,
        #[default(DefaultTrimPathEnd)] trim_path_end: f32,
        #[default(DefaultTrimPathOffset)] trim_path_offset: f32,
    ) -> &mut ImageVectorBuilder {
        self.add_path_of(VectorPath {
            name: name.into(),
            path_data,
            path_fill_type,
            fill,
            fill_alpha,
            stroke,
            stroke_alpha,
            stroke_line_width,
            stroke_line_cap,
            stroke_line_join,
            stroke_line_miter,
            trim_path_start,
            trim_path_end,
            trim_path_offset,
        })
    }

    /// Compose's Builder.group: a group of these, what `block` adds in it.
    #[track_caller]
    pub fn group(
        &mut self,
        #[default(DefaultGroupName)] name: &str,
        #[default(DefaultRotation)] rotate: f32,
        #[default(DefaultPivotX)] pivot_x: f32,
        #[default(DefaultPivotY)] pivot_y: f32,
        #[default(DefaultScaleX)] scale_x: f32,
        #[default(DefaultScaleY)] scale_y: f32,
        #[default(DefaultTranslationX)] translation_x: f32,
        #[default(DefaultTranslationY)] translation_y: f32,
        #[default(Vec::new())] clip_path_data: Vec<PathNode>,
        block: impl FnOnce(&mut ImageVectorBuilder),
    ) -> &mut ImageVectorBuilder {
        let group = VectorGroup {
            name: name.into(),
            rotation: rotate,
            pivot_x,
            pivot_y,
            scale_x,
            scale_y,
            translation_x,
            translation_y,
            clip_path_data,
            children: Vec::new(),
        };
        self.group_of(group, block)
    }

    /// Compose's Builder.path: a path of what `path_builder` makes, in these paints.
    #[track_caller]
    pub fn path(
        &mut self,
        #[default(DefaultPathName)] name: &str,
        #[default(None)] fill: Option<Brush>,
        #[default(1.0)] fill_alpha: f32,
        #[default(None)] stroke: Option<Brush>,
        #[default(1.0)] stroke_alpha: f32,
        #[default(DefaultStrokeLineWidth)] stroke_line_width: f32,
        #[default(DefaultStrokeLineCap)] stroke_line_cap: StrokeCap,
        #[default(DefaultStrokeLineJoin)] stroke_line_join: StrokeJoin,
        #[default(DefaultStrokeLineMiter)] stroke_line_miter: f32,
        #[default(DefaultFillType)] path_fill_type: PathFillType,
        path_builder: impl FnOnce(&mut PathBuilder),
    ) -> &mut ImageVectorBuilder {
        let path = VectorPath {
            name: name.into(),
            fill,
            fill_alpha,
            stroke,
            stroke_alpha,
            stroke_line_width,
            stroke_line_cap,
            stroke_line_join,
            stroke_line_miter,
            path_fill_type,
            ..VectorPath::default()
        };
        self.path_of(path, path_builder)
    }
}
