//! A vector made by composable content (Compose's `androidx.compose.ui.graphics.vector` composables): `Group` and
//! `Path` called in the content of [`remember_vector_painter_content`] make its tree; what the content reads (a
//! state, an animation) composes it again, and made the same it stays the same vector (an equal painter, its
//! bitmaps kept). [`RenderVectorGroup`] makes a vector's tree again as such content, each group and path with the
//! values a [`VectorConfig`] of its name gives (an animated vector's).

use super::painter::VectorPainter;
use crate::platform::LocalDensity;
use kompoz_graphics::{
    BlendMode, Brush, Color, DefaultFillType, DefaultGroupName, DefaultPathName, DefaultPivotX, DefaultPivotY,
    DefaultRotation, DefaultScaleX, DefaultScaleY, DefaultStrokeLineCap, DefaultStrokeLineJoin, DefaultStrokeLineMiter,
    DefaultStrokeLineWidth, DefaultTranslationX, DefaultTranslationY, DefaultTrimPathEnd, DefaultTrimPathOffset,
    DefaultTrimPathStart, Dp, ImageVector, ImageVectorBuilder, PathFillType, PathNode, StrokeCap, StrokeJoin,
    VectorGroup, VectorNode, VectorPath,
};
use kompoz_runtime::{kompozable, remember};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Compose's RootGroupName: the name of a vector made by content, unless given.
pub const RootGroupName: &str = "VectorRootGroup";

thread_local! {
    /// What `Group` and `Path` add to now (the innermost vector's content being composed).
    static CONTENT: RefCell<Vec<Rc<RefCell<ImageVectorBuilder>>>> = const { RefCell::new(Vec::new()) };
}

fn current() -> Option<Rc<RefCell<ImageVectorBuilder>>> {
    CONTENT.with(|c| c.borrow().last().cloned())
}

/// A vector made by composable `content` (Compose's rememberVectorPainter { Group { Path() } }), given the viewport's
/// size: `Group` and `Path` in it make its tree. Without a viewport, its default size in px (`LocalDensity`), as
/// Compose's. Made the same as last time, the same vector (its painter equal, its bitmaps kept).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn remember_vector_painter_content(
    default_width: Dp,
    default_height: Dp,
    #[default(f32::NAN)] viewport_width: f32,
    #[default(f32::NAN)] viewport_height: f32,
    #[default(RootGroupName.to_string())] name: String,
    #[default(Color::Unspecified)] tint_color: Color,
    #[default(BlendMode::SrcIn)] tint_blend_mode: BlendMode,
    #[default(false)] auto_mirror: bool,
    content: impl Fn(f32, f32),
) -> VectorPainter {
    let density = LocalDensity.current();
    let vw = if viewport_width.is_nan() { density.to_px(default_width) } else { viewport_width };
    let vh = if viewport_height.is_nan() { density.to_px(default_height) } else { viewport_height };
    let builder = Rc::new(RefCell::new(ImageVector::Builder(
        name.as_str(),
        default_width,
        default_height,
        vw,
        vh,
        tint_color,
        tint_blend_mode,
        auto_mirror,
    )));
    CONTENT.with(|c| c.borrow_mut().push(builder.clone()));
    content(vw, vh);
    CONTENT.with(|c| c.borrow_mut().pop());
    let made = builder.borrow_mut().build();
    let last: Rc<RefCell<Option<ImageVector>>> = remember(|| Rc::new(RefCell::new(None)));
    let vector = {
        let mut last = last.borrow_mut();
        if last.as_ref() != Some(&made) {
            *last = Some(made);
        }
        last.clone().expect("a vector")
    };
    VectorPainter::new(vector, density)
}

/// A group in a vector's content (Compose's Group): its transform and clip, what `content` makes inside it.
/// Outside a vector's content it adds nothing.
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
pub fn Group(
    #[default(DefaultGroupName.to_string())] name: String,
    #[default(DefaultRotation)] rotation: f32,
    #[default(DefaultPivotX)] pivot_x: f32,
    #[default(DefaultPivotY)] pivot_y: f32,
    #[default(DefaultScaleX)] scale_x: f32,
    #[default(DefaultScaleY)] scale_y: f32,
    #[default(DefaultTranslationX)] translation_x: f32,
    #[default(DefaultTranslationY)] translation_y: f32,
    #[default(Vec::new())] clip_path_data: Vec<PathNode>,
    content: impl Fn(),
) {
    let group = VectorGroup {
        name,
        rotation,
        pivot_x,
        pivot_y,
        scale_x,
        scale_y,
        translation_x,
        translation_y,
        clip_path_data,
        children: Vec::new(),
    };
    add_group(group, content);
}

/// A path in a vector's content (Compose's Path). Outside a vector's content it adds nothing.
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
pub fn Path(
    path_data: Vec<PathNode>,
    #[default(DefaultFillType)] path_fill_type: PathFillType,
    #[default(DefaultPathName.to_string())] name: String,
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
) {
    add_path(VectorPath {
        name,
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
    });
}

/// `group` (its name, transform and clip: not its children) in the vector's content being composed, what `content`
/// makes inside it (`Group` for plain code).
pub fn add_group(group: VectorGroup, content: impl FnOnce()) {
    let Some(builder) = current() else { return };
    builder.borrow_mut().add_group_of(VectorGroup { children: Vec::new(), ..group });
    content();
    builder.borrow_mut().clear_group();
}

/// `path` in the vector's content being composed (`Path` for plain code).
pub fn add_path(path: VectorPath) {
    if let Some(builder) = current() {
        builder.borrow_mut().add_path_of(path);
    }
}

/// What a [`VectorConfig`] can draw a group or a path with in place of its own value (Compose's VectorProperty).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum VectorProperty {
    Rotation,
    PivotX,
    PivotY,
    ScaleX,
    ScaleY,
    TranslateX,
    TranslateY,
    /// A path's data, or a group's clip path.
    PathData,
    Fill,
    FillAlpha,
    Stroke,
    StrokeLineWidth,
    StrokeAlpha,
    TrimPathStart,
    TrimPathEnd,
    TrimPathOffset,
}

/// The values a group or a path is drawn with (Compose's VectorConfig): its own, but for those a config of one's own
/// gives (each kind of value its method; by default the group's or path's own).
pub trait VectorConfig {
    fn get_float(&self, property: VectorProperty, default_value: f32) -> f32 {
        let _ = property;
        default_value
    }
    fn get_path_data(&self, property: VectorProperty, default_value: Vec<PathNode>) -> Vec<PathNode> {
        let _ = property;
        default_value
    }
    fn get_brush(&self, property: VectorProperty, default_value: Option<Brush>) -> Option<Brush> {
        let _ = property;
        default_value
    }
}

/// The config of none: every value its own.
struct NoConfig;

impl VectorConfig for NoConfig {}

/// `group`'s paths and groups as `Path` and `Group` in a vector's content (Compose's RenderVectorGroup), each drawn
/// with the config of its name in `configs` (a path's cap, join, miter and fill type, a group's name, its own).
#[kompozable(inline)]
pub fn RenderVectorGroup(
    group: VectorGroup,
    #[default(HashMap::new())] configs: HashMap<String, Rc<dyn VectorConfig>>,
) {
    render_vector_group(&group, &configs);
}

/// [`RenderVectorGroup`] for plain code.
pub fn render_vector_group(group: &VectorGroup, configs: &HashMap<String, Rc<dyn VectorConfig>>) {
    use VectorProperty as P;
    for node in group {
        let name = match node {
            VectorNode::Path(p) => &p.name,
            VectorNode::Group(g) => &g.name,
        };
        let config: Rc<dyn VectorConfig> = configs.get(name).cloned().unwrap_or_else(|| Rc::new(NoConfig));
        let c = &*config;
        match node {
            VectorNode::Path(p) => add_path(VectorPath {
                name: p.name.clone(),
                path_data: c.get_path_data(P::PathData, p.path_data.clone()),
                path_fill_type: p.path_fill_type,
                fill: c.get_brush(P::Fill, p.fill.clone()),
                fill_alpha: c.get_float(P::FillAlpha, p.fill_alpha),
                stroke: c.get_brush(P::Stroke, p.stroke.clone()),
                stroke_alpha: c.get_float(P::StrokeAlpha, p.stroke_alpha),
                stroke_line_width: c.get_float(P::StrokeLineWidth, p.stroke_line_width),
                stroke_line_cap: p.stroke_line_cap,
                stroke_line_join: p.stroke_line_join,
                stroke_line_miter: p.stroke_line_miter,
                trim_path_start: c.get_float(P::TrimPathStart, p.trim_path_start),
                trim_path_end: c.get_float(P::TrimPathEnd, p.trim_path_end),
                trim_path_offset: c.get_float(P::TrimPathOffset, p.trim_path_offset),
            }),
            VectorNode::Group(g) => add_group(
                VectorGroup {
                    name: g.name.clone(),
                    rotation: c.get_float(P::Rotation, g.rotation),
                    pivot_x: c.get_float(P::PivotX, g.pivot_x),
                    pivot_y: c.get_float(P::PivotY, g.pivot_y),
                    scale_x: c.get_float(P::ScaleX, g.scale_x),
                    scale_y: c.get_float(P::ScaleY, g.scale_y),
                    translation_x: c.get_float(P::TranslateX, g.translation_x),
                    translation_y: c.get_float(P::TranslateY, g.translation_y),
                    clip_path_data: c.get_path_data(P::PathData, g.clip_path_data.clone()),
                    children: Vec::new(),
                },
                || render_vector_group(g, configs),
            ),
        }
    }
}
