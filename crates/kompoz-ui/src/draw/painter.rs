//! Painters (Compose's `androidx.compose.ui.graphics.painter`): something drawn into whatever size it is given —
//! a bitmap ([`BitmapPainter`]), a vector ([`VectorPainter`]: drawn into a bitmap it keeps, as Compose's
//! VectorPainter and the C++ Kompoz draw one), a color, a brush, or drawing of one's own — and `Modifier.paint`,
//! which sizes a layout by a painter and draws it (Compose's PainterModifier).

use super::scope::transformed_in;
use super::{Canvas, ContentDrawScope, DrawModifierNode, DrawScope, Fill, Stroke};
use crate::alignment::Alignment;
use crate::graphics_context::CornerRadii;
use crate::graphics_context::LayerTransform;
use crate::layout::{
    IntrinsicMeasurable, IntrinsicMeasureScope, LayoutModifierNode, Measurable, MeasureResult, MeasureScope,
};
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement};
use crate::node_capabilities;
use crate::platform::LocalDensity;
use crate::unit::Constraints;
use kompoz_graphics::colorspace::ColorSpaces;
use kompoz_graphics::{
    BlendMode, Brush, Color, ColorFilter, ContentScale, Density, FilterQuality, ImageBitmap, ImageBitmapConfig,
    ImageVector, IntOffset, IntSize, LayoutDirection, MAX_IMAGE_DIMENSION, Matrix, Offset, Rect, Size, VectorGroup,
    VectorNode, VectorPath, to_path,
};
use kompoz_runtime::{kompozable, named};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

/// Something drawn into whatever size it is given (Compose's Painter): its intrinsic size (unspecified: none, as a
/// color's) sizes `Image` and `Modifier.paint`. A painter that fades itself (`applies_alpha`) or filters its colors
/// itself (`applies_color_filter`) is given the alpha and the filter; for one that doesn't, [`draw`](dyn
/// Painter::draw) draws it through a layer of them. Built-in painters are equal when what they draw is; others only
/// to themselves (`painter_eq`).
pub trait Painter: Any {
    /// Its size where nothing else gives one, px (`Size::Unspecified`: none).
    fn intrinsic_size(&self) -> Size;
    /// Draws it into `scope` (its size the painter's), faded by `alpha` and through `color_filter` where it
    /// applies them itself (else 1 and none).
    fn on_draw(&self, scope: &mut DrawScope<'_>, alpha: f32, color_filter: Option<&ColorFilter>);
    /// Whether it fades itself (Compose's applyAlpha returning true).
    fn applies_alpha(&self) -> bool {
        false
    }
    /// Whether it filters its colors itself (Compose's applyColorFilter returning true).
    fn applies_color_filter(&self) -> bool {
        false
    }
    fn as_any(&self) -> &dyn Any;
    /// Whether it draws what `other` draws (by default: it is `other`).
    fn painter_eq(&self, other: &dyn Painter) -> bool {
        std::ptr::eq(self.as_any() as *const dyn Any as *const (), other.as_any() as *const dyn Any as *const ())
    }
}

impl PartialEq for dyn Painter {
    fn eq(&self, other: &dyn Painter) -> bool {
        self.painter_eq(other)
    }
}

impl fmt::Debug for dyn Painter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Painter(intrinsic_size={:?})", self.intrinsic_size())
    }
}

impl dyn Painter {
    /// Draws it into the top left `size` of `scope` (Compose's Painter.draw): faded by `alpha` and through
    /// `color_filter`, by itself where it applies them, else through a layer of them; nothing for an alpha of 0 or
    /// an empty size.
    pub fn draw(&self, scope: &mut DrawScope<'_>, size: Size, alpha: f32, color_filter: Option<&ColorFilter>) {
        let (right, bottom) = (scope.size.width - size.width, scope.size.height - size.height);
        scope.inset(0.0, 0.0, right, bottom, |s| {
            if !(alpha > 0.0 && size.width > 0.0 && size.height > 0.0) {
                return;
            }
            let layer_alpha = !self.applies_alpha() && alpha < 1.0;
            let layer_filter = if self.applies_color_filter() { None } else { color_filter };
            let own_alpha = if self.applies_alpha() { alpha } else { 1.0 };
            let own_filter = if self.applies_color_filter() { color_filter } else { None };
            if layer_alpha || layer_filter.is_some() {
                let at = Rect::from_offset_size(s.origin, size);
                if let Some(f) = layer_filter {
                    s.canvas.push_color_filter(f);
                }
                let a = if layer_alpha { alpha } else { 1.0 };
                s.canvas.push_layer(at, CornerRadii::Zero, a, LayerTransform::Identity);
                self.on_draw(s, own_alpha, own_filter);
                s.canvas.pop_layer();
                if layer_filter.is_some() {
                    s.canvas.pop_color_filter();
                }
            } else {
                self.on_draw(s, own_alpha, own_filter);
            }
        });
    }
}

// ---- Bitmaps, colors, brushes, drawing of one's own ---------------------------------------------------------------

/// A bitmap's pixels from `src_offset`, `src_size`, sampled as `filter_quality` says (Compose's BitmapPainter): its
/// intrinsic size the source's.
#[derive(Clone, PartialEq, Debug)]
pub struct BitmapPainter {
    pub image: ImageBitmap,
    pub src_offset: IntOffset,
    pub src_size: IntSize,
    pub filter_quality: FilterQuality,
}

/// Compose's `BitmapPainter(image, srcOffset = …, srcSize = …, filterQuality = …)`: all of it, sampled Low, by
/// default.
#[named(type)]
pub fn BitmapPainter(
    image: ImageBitmap,
    #[default(IntOffset::Zero)] src_offset: IntOffset,
    #[default(IntSize::new(-1, -1))] src_size: IntSize,
    #[default(FilterQuality::Low)] filter_quality: FilterQuality,
) -> BitmapPainter {
    let src_size =
        if src_size == IntSize::new(-1, -1) { IntSize::new(image.width(), image.height()) } else { src_size };
    BitmapPainter::new(image, src_offset, src_size, filter_quality)
}

impl BitmapPainter {
    /// Every parameter given. Panics, as Compose throws, for a source outside the bitmap.
    #[track_caller]
    pub fn new(image: ImageBitmap, src_offset: IntOffset, src_size: IntSize, filter_quality: FilterQuality) -> Self {
        assert!(
            src_offset.x >= 0
                && src_offset.y >= 0
                && src_size.width >= 0
                && src_size.height >= 0
                && src_size.width <= image.width()
                && src_size.height <= image.height(),
            "kompoz: a BitmapPainter's source must lie within its bitmap"
        );
        BitmapPainter { image, src_offset, src_size, filter_quality }
    }

    /// All of `image`, sampled as `filter_quality` says.
    pub fn with_quality(image: ImageBitmap, filter_quality: FilterQuality) -> Self {
        let size = IntSize::new(image.width(), image.height());
        BitmapPainter::new(image, IntOffset::Zero, size, filter_quality)
    }
}

impl Painter for BitmapPainter {
    fn intrinsic_size(&self) -> Size {
        self.src_size.to_size()
    }
    fn on_draw(&self, scope: &mut DrawScope<'_>, alpha: f32, color_filter: Option<&ColorFilter>) {
        let s = scope.size;
        let dst = IntSize::new(s.width.round() as i32, s.height.round() as i32);
        scope.draw_image_rect_with(
            &self.image,
            self.src_offset,
            self.src_size,
            IntOffset::Zero,
            dst,
            alpha,
            Fill,
            color_filter.copied(),
            BlendMode::SrcOver,
            self.filter_quality,
        );
    }
    fn applies_alpha(&self) -> bool {
        true
    }
    fn applies_color_filter(&self) -> bool {
        true
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn painter_eq(&self, other: &dyn Painter) -> bool {
        other.as_any().downcast_ref::<BitmapPainter>().is_some_and(|o| o == self)
    }
}

/// A color filling what it is given (Compose's ColorPainter): no intrinsic size.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ColorPainter {
    pub color: Color,
}

/// Compose's `ColorPainter(color)`.
pub fn ColorPainter(color: Color) -> ColorPainter {
    ColorPainter { color }
}

impl Painter for ColorPainter {
    fn intrinsic_size(&self) -> Size {
        Size::Unspecified
    }
    fn on_draw(&self, scope: &mut DrawScope<'_>, alpha: f32, color_filter: Option<&ColorFilter>) {
        let size = scope.size;
        scope.draw_rect_with(self.color, Offset::Zero, size, alpha, Fill, color_filter.copied(), BlendMode::SrcOver);
    }
    fn applies_alpha(&self) -> bool {
        true
    }
    fn applies_color_filter(&self) -> bool {
        true
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn painter_eq(&self, other: &dyn Painter) -> bool {
        other.as_any().downcast_ref::<ColorPainter>().is_some_and(|o| o == self)
    }
}

/// A brush filling what it is given (Compose's BrushPainter): its intrinsic size the brush's.
#[derive(Clone, PartialEq, Debug)]
pub struct BrushPainter {
    pub brush: Brush,
}

/// Compose's `BrushPainter(brush)`.
pub fn BrushPainter(brush: impl Into<Brush>) -> BrushPainter {
    BrushPainter { brush: brush.into() }
}

impl Painter for BrushPainter {
    fn intrinsic_size(&self) -> Size {
        self.brush.intrinsic_size()
    }
    fn on_draw(&self, scope: &mut DrawScope<'_>, alpha: f32, color_filter: Option<&ColorFilter>) {
        let size = scope.size;
        let cf = color_filter.copied();
        scope.draw_rect_with(self.brush.clone(), Offset::Zero, size, alpha, Fill, cf, BlendMode::SrcOver);
    }
    fn applies_alpha(&self) -> bool {
        true
    }
    fn applies_color_filter(&self) -> bool {
        true
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn painter_eq(&self, other: &dyn Painter) -> bool {
        other.as_any().downcast_ref::<BrushPainter>().is_some_and(|o| o == self)
    }
}

/// Drawing of one's own as a painter (the C++ Kompoz's Painter::custom): `on_draw` into the size it is given (an
/// alpha below 1 or a color filter through a layer), its intrinsic size as given. Equal only to itself.
pub struct DrawPainter {
    on_draw: Rc<dyn Fn(&mut DrawScope<'_>)>,
    intrinsic_size: Size,
}

impl DrawPainter {
    pub fn new(intrinsic_size: Size, on_draw: impl Fn(&mut DrawScope<'_>) + 'static) -> DrawPainter {
        DrawPainter { on_draw: Rc::new(on_draw), intrinsic_size }
    }
}

impl Painter for DrawPainter {
    fn intrinsic_size(&self) -> Size {
        self.intrinsic_size
    }
    fn on_draw(&self, scope: &mut DrawScope<'_>, _: f32, _: Option<&ColorFilter>) {
        (self.on_draw)(scope);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---- Vectors -------------------------------------------------------------------------------------------------------

/// A vector drawn into the size it is given (Compose's VectorPainter): through a bitmap it was drawn into and kept
/// (for each size and config), as a mask in its one color where it paints in one, tinted by a color filter, else by
/// its own tint; mirrored right to left if it says so. Its intrinsic size its default size in px. Equal when the
/// vectors are.
#[derive(Clone)]
pub struct VectorPainter {
    pub vector: ImageVector,
    intrinsic_size: Size,
}

impl VectorPainter {
    /// `vector` at `density` (its default size in px).
    pub fn new(vector: ImageVector, density: Density) -> VectorPainter {
        let size = Size::new(density.to_px(vector.default_width()), density.to_px(vector.default_height()));
        VectorPainter { vector, intrinsic_size: size }
    }
}

impl PartialEq for VectorPainter {
    fn eq(&self, other: &Self) -> bool {
        self.vector == other.vector && self.intrinsic_size == other.intrinsic_size
    }
}

impl fmt::Debug for VectorPainter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("VectorPainter").field("vector", &self.vector).finish()
    }
}

impl Painter for VectorPainter {
    fn intrinsic_size(&self) -> Size {
        self.intrinsic_size
    }
    fn on_draw(&self, scope: &mut DrawScope<'_>, alpha: f32, color_filter: Option<&ColorFilter>) {
        draw_vector(scope, &self.vector, alpha, color_filter);
    }
    fn applies_alpha(&self) -> bool {
        true
    }
    fn applies_color_filter(&self) -> bool {
        true
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn painter_eq(&self, other: &dyn Painter) -> bool {
        other.as_any().downcast_ref::<VectorPainter>().is_some_and(|o| o == self)
    }
}

/// `image` as a painter at the composition's density (Compose's rememberVectorPainter(image)): equal vectors make
/// equal painters, so nothing needs keeping.
#[kompozable]
pub fn remember_vector_painter(image: ImageVector) -> VectorPainter {
    VectorPainter::new(image, LocalDensity.current())
}

/// A bitmap a vector was drawn into: its size (px, as asked), its config.
struct Drawn {
    width: f32,
    height: f32,
    config: ImageBitmapConfig,
    bitmap: ImageBitmap,
}

/// Bitmaps a vector keeps (for as many sizes and configs), and vectors whose bitmaps are kept.
const DRAWN_KEPT: usize = 8;
const VECTORS_KEPT: usize = 64;

thread_local! {
    /// The bitmaps vectors were drawn into, by vector id, the last drawn last (Compose keeps them in its painter's
    /// DrawCache; equal painters share them here).
    static DRAWN: RefCell<Vec<(u64, Vec<Drawn>)>> = const { RefCell::new(Vec::new()) };
    /// Which vector each of those bitmaps holds.
    static DRAWN_INTO: RefCell<HashMap<u64, u64>> = RefCell::new(HashMap::new());
}

/// The id of the vector a painter drew into `bitmap` (none if none: for tests and backends that read it).
pub fn vector_drawn_into(bitmap: &ImageBitmap) -> Option<u64> {
    DRAWN_INTO.with(|d| d.borrow().get(&bitmap.id()).copied())
}

/// The bitmap `vector` was drawn into at `width` x `height` px in `config`, kept.
fn kept_bitmap(vector: u64, width: f32, height: f32, config: ImageBitmapConfig) -> Option<ImageBitmap> {
    DRAWN.with(|d| {
        let mut d = d.borrow_mut();
        let at = d.iter().position(|(id, _)| *id == vector)?;
        let entry = d.remove(at);
        let found = entry
            .1
            .iter()
            .find(|x| x.width == width && x.height == height && x.config == config)
            .map(|x| x.bitmap.clone());
        d.push(entry);
        found
    })
}

fn keep_bitmap(vector: u64, drawn: Drawn) {
    DRAWN_INTO.with(|d| d.borrow_mut().insert(drawn.bitmap.id(), vector));
    let forget =
        |bitmaps: &[Drawn]| DRAWN_INTO.with(|d| bitmaps.iter().for_each(|b| _ = d.borrow_mut().remove(&b.bitmap.id())));
    DRAWN.with(|d| {
        let mut d = d.borrow_mut();
        match d.iter().position(|(id, _)| *id == vector) {
            Some(at) => {
                let mut entry = d.remove(at);
                entry.1.push(drawn);
                if entry.1.len() > DRAWN_KEPT {
                    let gone = entry.1.remove(0);
                    forget(std::slice::from_ref(&gone));
                }
                d.push(entry);
            }
            None => {
                d.push((vector, vec![drawn]));
                if d.len() > VECTORS_KEPT {
                    let (_, gone) = d.remove(0);
                    forget(&gone);
                }
            }
        }
    });
}

/// A vector path drawn: its render path (trimmed) filled and stroked in its own paints.
fn draw_path_node(s: &mut DrawScope<'_>, p: &VectorPath) {
    let path = p.render_path();
    if let Some(fill) = &p.fill {
        s.draw_path_with(&path, fill.clone(), p.fill_alpha, Fill, None, BlendMode::SrcOver);
    }
    if let Some(stroke) = &p.stroke {
        let style = Stroke::new(p.stroke_line_width, p.stroke_line_miter, p.stroke_line_cap, p.stroke_line_join, None);
        s.draw_path_with(&path, stroke.clone(), p.stroke_alpha, style.into(), None, BlendMode::SrcOver);
    }
}

/// A vector group drawn (Compose's GroupComponent): what is in it through its matrix and clip path.
fn draw_group_node(s: &mut DrawScope<'_>, g: &VectorGroup) {
    let children = |s: &mut DrawScope<'_>| {
        for node in g {
            match node {
                VectorNode::Path(p) => draw_path_node(s, p),
                VectorNode::Group(child) => draw_group_node(s, child),
            }
        }
    };
    let m = g.group_matrix();
    if m.is_identity() && g.clip_path_data.is_empty() {
        return children(s); // (nothing to transform or clip by)
    }
    s.with_transform(
        |t| {
            if !m.is_identity() {
                t.transform(m);
            }
            if !g.clip_path_data.is_empty() {
                t.clip_path(to_path(&g.clip_path_data));
            }
        },
        children,
    );
}

/// Draws `vector` into `scope`'s size as Compose's VectorComponent does (the C++ Kompoz's drawVector): into a bitmap
/// of whole pixels around the size (just the size within a hundredth of a pixel of it), the vector scaled to fill
/// it, kept for each size and config; an Alpha8 one (one color, its filters taking a mask) drawn as a mask; through
/// `color_filter`, else the vector's tint, else (a mask) its color; mirrored about its middle right to left if it
/// auto-mirrors.
pub fn draw_vector(scope: &mut DrawScope<'_>, vector: &ImageVector, alpha: f32, color_filter: Option<&ColorFilter>) {
    if alpha <= 0.0 {
        return;
    }
    let Size { width, height } = scope.size;
    let (vw, vh) = (vector.viewport_width(), vector.viewport_height());
    if !(width > 0.0 && height > 0.0) || vw <= 0.0 || vh <= 0.0 {
        return;
    }
    let config = vector.cache_bitmap_config(color_filter);
    let pixels =
        |v: f32| if (v - v.round()).abs() < 0.01 && v.round() >= 1.0 { v.round() as i32 } else { v.ceil() as i32 };
    let (w, h) = (pixels(width), pixels(height));
    if w > MAX_IMAGE_DIMENSION || h > MAX_IMAGE_DIMENSION {
        return;
    }
    let bitmap = match kept_bitmap(vector.id(), width, height, config) {
        Some(b) => b,
        None => {
            let bitmap = ImageBitmap::new(w, h, config, true, &ColorSpaces::Srgb);
            {
                let mut canvas = Canvas::from_image(&bitmap);
                let density = scope.density;
                let mut cache =
                    DrawScope::over_canvas(&mut canvas, Size::new(w as f32, h as f32), LayoutDirection::Ltr, density);
                cache.scale_with(width / vw, height / vh, Offset::Zero, |s| draw_group_node(s, vector.root()));
            }
            keep_bitmap(vector.id(), Drawn { width, height, config, bitmap: bitmap.clone() });
            bitmap
        }
    };
    let filter = vector.draw_color_filter(color_filter);
    let draw = |s: &mut DrawScope<'_>| {
        let all = IntSize::new(w, h);
        s.draw_image_rect_with(
            &bitmap,
            IntOffset::Zero,
            all,
            IntOffset::Zero,
            all,
            alpha,
            Fill,
            filter,
            BlendMode::SrcOver,
            kompoz_graphics::DefaultFilterQuality,
        );
    };
    if vector.auto_mirror() && scope.layout_direction == LayoutDirection::Rtl {
        let c = scope.center();
        let mut m = Matrix::new();
        m.translate(c.x, c.y, 0.0);
        m.scale(-1.0, 1.0, 1.0);
        m.translate(-c.x, -c.y, 0.0);
        transformed_in(scope, &m, draw);
    } else {
        draw(scope);
    }
}

// ---- Modifier.paint -------------------------------------------------------------------------------------------------

#[named]
impl Modifier {
    /// Draws `painter` behind the content (Compose's `Modifier.paint`): scaled by `content_scale` and placed by
    /// `alignment` in the layout, faded by `alpha`, through `color_filter`; with `size_to_intrinsics` the layout is as
    /// big as the painter (scaled to what it may be) unless its size is fixed.
    pub fn paint(
        self,
        painter: Rc<dyn Painter>,
        #[default(true)] size_to_intrinsics: bool,
        #[default(Alignment::Center)] alignment: Alignment,
        #[default(ContentScale::Inside)] content_scale: ContentScale,
        #[default(1.0)] alpha: f32,
        #[default(None)] color_filter: Option<ColorFilter>,
    ) -> Modifier {
        self.paint_with(painter, size_to_intrinsics, alignment, content_scale, alpha, color_filter)
    }
}

impl Modifier {
    /// [`paint`](Modifier::paint) with every parameter.
    pub fn paint_with(
        self,
        painter: Rc<dyn Painter>,
        size_to_intrinsics: bool,
        alignment: Alignment,
        content_scale: ContentScale,
        alpha: f32,
        color_filter: Option<ColorFilter>,
    ) -> Modifier {
        self.then_element(PainterElement { painter, size_to_intrinsics, alignment, content_scale, alpha, color_filter })
    }
}

struct PainterElement {
    painter: Rc<dyn Painter>,
    size_to_intrinsics: bool,
    alignment: Alignment,
    content_scale: ContentScale,
    alpha: f32,
    color_filter: Option<ColorFilter>,
}

impl PartialEq for PainterElement {
    fn eq(&self, o: &Self) -> bool {
        *self.painter == *o.painter
            && self.size_to_intrinsics == o.size_to_intrinsics
            && self.alignment == o.alignment
            && self.content_scale == o.content_scale
            && self.alpha == o.alpha
            && self.color_filter == o.color_filter
    }
}

impl ModifierNodeElement for PainterElement {
    type Node = PainterNode;
    fn create(&self) -> PainterNode {
        PainterNode {
            painter: self.painter.clone(),
            size_to_intrinsics: self.size_to_intrinsics,
            alignment: self.alignment.clone(),
            content_scale: self.content_scale.clone(),
            alpha: self.alpha,
            color_filter: self.color_filter,
        }
    }
    fn update(&self, node: &mut PainterNode) {
        *node = self.create();
    }
    fn name(&self) -> &'static str {
        "paint"
    }
}

/// The node of `Modifier.paint` (Compose's PainterNode).
pub struct PainterNode {
    painter: Rc<dyn Painter>,
    size_to_intrinsics: bool,
    alignment: Alignment,
    content_scale: ContentScale,
    alpha: f32,
    color_filter: Option<ColorFilter>,
}

fn specified_width(s: Size) -> Option<f32> {
    (!s.width.is_nan() && s.width.is_finite()).then_some(s.width)
}

fn specified_height(s: Size) -> Option<f32> {
    (!s.height.is_nan() && s.height.is_finite()).then_some(s.height)
}

/// Kotlin's roundToInt.
fn round(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

impl PainterNode {
    fn use_intrinsic_size(&self) -> bool {
        let s = self.painter.intrinsic_size();
        self.size_to_intrinsics && !s.width.is_nan() && !s.height.is_nan()
    }

    fn calculate_scaled_size(&self, dst: Size) -> Size {
        if !self.use_intrinsic_size() {
            return dst;
        }
        let i = self.painter.intrinsic_size();
        let src = Size::new(specified_width(i).unwrap_or(dst.width), specified_height(i).unwrap_or(dst.height));
        if dst.width != 0.0 && dst.height != 0.0 {
            let f = self.content_scale.compute_scale_factor(src, dst);
            Size::new(src.width * f.scale_x, src.height * f.scale_y)
        } else {
            Size::Zero
        }
    }

    fn modify_constraints(&self, c: Constraints) -> Constraints {
        let bounded = c.has_bounded_width() && c.has_bounded_height();
        let fixed = c.has_fixed_width() && c.has_fixed_height();
        if (!self.use_intrinsic_size() && bounded) || fixed {
            return Constraints::new(c.max_width, c.max_width, c.max_height, c.max_height);
        }
        let i = self.painter.intrinsic_size();
        let iw = specified_width(i).map_or(c.min_width, round);
        let ih = specified_height(i).map_or(c.min_height, round);
        let (cw, ch) = (c.constrain_width(iw), c.constrain_height(ih));
        let scaled = self.calculate_scaled_size(Size::new(cw as f32, ch as f32));
        let min_width = c.constrain_width(round(scaled.width));
        let min_height = c.constrain_height(round(scaled.height));
        Constraints::new(min_width, c.max_width, min_height, c.max_height)
    }
}

impl ModifierNode for PainterNode {
    node_capabilities!(layout, draw);
}

impl LayoutModifierNode for PainterNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = measurable.measure(self.modify_constraints(constraints));
        scope.layout(placeable.width(), placeable.height(), move |p| p.place_relative(&placeable, 0, 0))
    }

    fn min_intrinsic_width(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, height: i32) -> i32 {
        if self.use_intrinsic_size() {
            let c = self.modify_constraints(Constraints::new(0, Constraints::Infinity, 0, height));
            c.min_width.max(m.min_intrinsic_width(height))
        } else {
            m.min_intrinsic_width(height)
        }
    }
    fn max_intrinsic_width(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, height: i32) -> i32 {
        if self.use_intrinsic_size() {
            let c = self.modify_constraints(Constraints::new(0, Constraints::Infinity, 0, height));
            c.min_width.max(m.max_intrinsic_width(height))
        } else {
            m.max_intrinsic_width(height)
        }
    }
    fn min_intrinsic_height(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        if self.use_intrinsic_size() {
            let c = self.modify_constraints(Constraints::new(0, width, 0, Constraints::Infinity));
            c.min_height.max(m.min_intrinsic_height(width))
        } else {
            m.min_intrinsic_height(width)
        }
    }
    fn max_intrinsic_height(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        if self.use_intrinsic_size() {
            let c = self.modify_constraints(Constraints::new(0, width, 0, Constraints::Infinity));
            c.min_height.max(m.max_intrinsic_height(width))
        } else {
            m.max_intrinsic_height(width)
        }
    }
}

impl DrawModifierNode for PainterNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        let i = self.painter.intrinsic_size();
        let size = scope.size;
        let src = Size::new(specified_width(i).unwrap_or(size.width), specified_height(i).unwrap_or(size.height));
        let scaled = if size.width != 0.0 && size.height != 0.0 {
            let f = self.content_scale.compute_scale_factor(src, size);
            Size::new(src.width * f.scale_x, src.height * f.scale_y)
        } else {
            Size::Zero
        };
        let at = self.alignment.align(
            IntSize::new(round(scaled.width), round(scaled.height)),
            IntSize::new(round(size.width), round(size.height)),
            scope.layout_direction,
        );
        let (painter, alpha, cf) = (self.painter.clone(), self.alpha, self.color_filter);
        scope.scope.translate_with(at.x as f32, at.y as f32, |s| painter.draw(s, scaled, alpha, cf.as_ref()));
        scope.draw_content();
    }
}
