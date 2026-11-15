//! `Modifier.border`, as Compose's: a stroke inside the bounds along a shape, in a color or a brush, over the
//! content; what it draws worked out once for a size (Compose's BorderModifierNode and its cache).

use kompoz_graphics::colorspace::ColorSpaces;
use kompoz_graphics::{
    BlendMode, BorderStroke, Brush, Color, ColorFilter, CornerRadius, Density, Dp, ImageBitmap, ImageBitmapConfig,
    IntOffset, IntSize, LayoutDirection, Offset, Outline, Path, PathDirection, PathOperation, Rect, RectangleShape,
    RoundRect, Shape, Size, SolidColor,
};
use kompoz_runtime::named;
use kompoz_ui::draw::{Canvas, ContentDrawScope, DrawModifierNode, DrawScope, DrawStyle, Fill, Stroke};
use kompoz_ui::graphics_context::ClipOp;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, node_capabilities};
use std::sync::Arc;

/// The border modifiers (Compose's `Modifier.border` overloads).
#[named(trait BorderModifiers)]
impl Modifier {
    /// A border `width` wide in `color` along `shape` (Compose's `border(width, color, shape)`).
    fn border(self, width: Dp, color: Color, #[default(Arc::new(RectangleShape))] shape: Arc<dyn Shape>) -> Modifier {
        self.then_element(BorderElement { width, brush: SolidColor(color), shape })
    }

    /// [`border`](BorderModifiers::border) with every parameter, for plain code.
    fn border_with(self, width: Dp, color: Color, shape: impl Shape + 'static) -> Modifier {
        self.then_element(BorderElement { width, brush: SolidColor(color), shape: Arc::new(shape) })
    }

    /// A border `width` wide painted by `brush` along `shape` (Compose's `border(width, brush, shape)`).
    fn border_brush(self, width: Dp, brush: Brush, shape: impl Shape + 'static) -> Modifier {
        self.then_element(BorderElement { width, brush, shape: Arc::new(shape) })
    }

    /// The border `border` says along `shape` (Compose's `border(border, shape)`).
    fn border_stroke(
        self,
        border: BorderStroke,
        #[default(Arc::new(RectangleShape))] shape: Arc<dyn Shape>,
    ) -> Modifier {
        self.then_element(BorderElement { width: border.width, brush: border.brush, shape })
    }

    /// [`border_stroke`](BorderModifiers::border_stroke) with every parameter, for plain code.
    fn border_stroke_with(self, border: BorderStroke, shape: impl Shape + 'static) -> Modifier {
        self.then_element(BorderElement { width: border.width, brush: border.brush, shape: Arc::new(shape) })
    }
}

struct BorderElement {
    width: Dp,
    brush: Brush,
    shape: Arc<dyn Shape>,
}

impl PartialEq for BorderElement {
    fn eq(&self, o: &Self) -> bool {
        self.width == o.width && self.brush == o.brush && *self.shape == *o.shape
    }
}

impl ModifierNodeElement for BorderElement {
    type Node = BorderModifierNode;
    fn create(&self) -> BorderModifierNode {
        BorderModifierNode { width: self.width, brush: self.brush.clone(), shape: self.shape.clone(), cache: None }
    }
    fn update(&self, node: &mut BorderModifierNode) {
        if node.width != self.width || node.brush != self.brush || *node.shape != *self.shape {
            node.cache = None;
        }
        node.width = self.width;
        node.brush = self.brush.clone();
        node.shape = self.shape.clone();
    }
    fn name(&self) -> &'static str {
        "border"
    }
}

/// What a border draws over the content, worked out for a size.
#[derive(Clone)]
enum BorderDraw {
    /// Nothing (no width, or no size).
    Nothing,
    /// A path filled (a generic outline the border fills, a rounded rect's ring).
    Path(Path),
    /// A generic outline's ring drawn into a bitmap, drawn at its bounds through a filter (a color's tint).
    Bitmap { image: ImageBitmap, at: Offset, size: IntSize, color_filter: Option<ColorFilter> },
    /// A simple rounded rect: filled, outside a clip of its inside, or stroked along it.
    RoundRect { corner: CornerRadius, kind: RoundKind, top_left: Offset, size: Size, stroke_width: f32 },
    /// A rect filled or stroked.
    Rect { top_left: Offset, size: Size, style: DrawStyle },
}

#[derive(Clone, Copy, PartialEq)]
enum RoundKind {
    Fill,
    Clipped,
    Stroked,
}

/// The node of `Modifier.border` (Compose's BorderModifierNode): what it draws worked out again for another size,
/// density or direction, or another width, brush or shape. (Compose's node also sets the `Shape` semantics property,
/// which the semantics package adds.)
pub struct BorderModifierNode {
    width: Dp,
    brush: Brush,
    shape: Arc<dyn Shape>,
    cache: Option<((Size, Density, LayoutDirection), BorderDraw)>,
}

impl ModifierNode for BorderModifierNode {
    node_capabilities!(draw);
}

/// Compose's CornerRadius.shrink.
fn shrink(c: CornerRadius, by: f32) -> CornerRadius {
    CornerRadius::new((c.x - by).max(0.0), (c.y - by).max(0.0))
}

impl BorderModifierNode {
    fn work_out(&self, size: Size, density: &Density, direction: LayoutDirection) -> BorderDraw {
        let width_px = density.to_px(self.width);
        if !(width_px >= 0.0 && size.min_dimension() > 0.0) {
            return BorderDraw::Nothing;
        }
        let stroke_width =
            (if self.width == Dp::Hairline { 1.0 } else { width_px.ceil() }).min((size.min_dimension() / 2.0).ceil());
        let half = stroke_width / 2.0;
        let top_left = Offset::new(half, half);
        let border_size = Size::new(size.width - stroke_width, size.height - stroke_width);
        let fill_area = stroke_width * 2.0 > size.min_dimension();
        match self.shape.create_outline(size, direction, density) {
            Outline::Generic(path) => self.generic(path, fill_area, stroke_width, density),
            Outline::Rounded(rr) if rr.is_simple() => {
                let corner = rr.top_left_corner_radius;
                let kind = if fill_area {
                    RoundKind::Fill
                } else if corner.x < half {
                    RoundKind::Clipped
                } else {
                    RoundKind::Stroked
                };
                BorderDraw::RoundRect { corner, kind, top_left, size: border_size, stroke_width }
            }
            Outline::Rounded(rr) => {
                let mut outer = Path::new();
                outer.add_round_rect(&rr, PathDirection::CounterClockwise);
                if fill_area {
                    return BorderDraw::Path(outer);
                }
                let inset = RoundRect::from_rect(
                    Rect::new(stroke_width, stroke_width, rr.width() - stroke_width, rr.height() - stroke_width),
                    shrink(rr.top_left_corner_radius, stroke_width),
                    shrink(rr.top_right_corner_radius, stroke_width),
                    shrink(rr.bottom_right_corner_radius, stroke_width),
                    shrink(rr.bottom_left_corner_radius, stroke_width),
                );
                let mut inner = Path::new();
                inner.add_round_rect(&inset, PathDirection::CounterClockwise);
                let mut ring = Path::new();
                ring.op(&outer, &inner, PathOperation::Difference);
                BorderDraw::Path(ring)
            }
            Outline::Rectangle(_) => {
                let (at, s, style) = if fill_area {
                    (Offset::Zero, size, Fill)
                } else {
                    (
                        top_left,
                        border_size,
                        Stroke::new(stroke_width, Stroke::DefaultMiter, Stroke::DefaultCap, Stroke::DefaultJoin, None)
                            .into(),
                    )
                };
                BorderDraw::Rect { top_left: at, size: s, style }
            }
        }
    }

    /// A generic outline's border (Compose's drawGenericBorder): the path filled where the border fills it, else its
    /// ring — the path stroked twice the width, less what is outside it — drawn into a bitmap (an Alpha8 one tinted
    /// for a color).
    fn generic(&self, path: Path, fill_area: bool, stroke_width: f32, density: &Density) -> BorderDraw {
        if fill_area {
            return BorderDraw::Path(path);
        }
        let (config, color_filter) = match self.brush.solid_color() {
            Some(c) => (
                ImageBitmapConfig::Alpha8,
                Some(ColorFilter::tint(
                    Color::new_in(c.red(), c.green(), c.blue(), 1.0, c.color_space()),
                    BlendMode::SrcIn,
                )),
            ),
            None => (ImageBitmapConfig::Argb8888, None),
        };
        let bounds = path.get_bounds();
        let mut bounds_path = Path::new();
        bounds_path.add_rect(bounds, PathDirection::CounterClockwise);
        let mut mask = Path::new();
        mask.op(&bounds_path, &path, PathOperation::Difference);
        let size = IntSize::new(bounds.width().ceil() as i32, bounds.height().ceil() as i32);
        let Some(image) = ImageBitmap::try_new(size.width, size.height, config, true, &ColorSpaces::Srgb) else {
            return BorderDraw::Nothing;
        };
        {
            let mut canvas = Canvas::from_image(&image);
            let draw_size = size.to_size();
            let mut cache = DrawScope::over_canvas(&mut canvas, draw_size, LayoutDirection::Ltr, *density);
            cache.draw_rect_with(Color::Black, Offset::Zero, draw_size, 1.0, Fill, None, BlendMode::Clear);
            let brush = self.brush.clone();
            cache.translate_with(-bounds.left, -bounds.top, |s| {
                let stroke = Stroke::new(
                    stroke_width * 2.0,
                    Stroke::DefaultMiter,
                    Stroke::DefaultCap,
                    Stroke::DefaultJoin,
                    None,
                );
                s.draw_path_with(&path, brush.clone(), 1.0, stroke.into(), None, BlendMode::SrcOver);
                let (w, h) = (s.size().width, s.size().height);
                let center = s.center();
                s.scale_with((w + 1.0) / w, (h + 1.0) / h, center, |s| {
                    s.draw_path_with(&mask, brush.clone(), 1.0, Fill, None, BlendMode::Clear);
                });
            });
        }
        image.prepare_to_draw();
        BorderDraw::Bitmap { image, at: bounds.top_left(), size, color_filter }
    }
}

impl DrawModifierNode for BorderModifierNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        let key = (scope.size(), scope.density(), scope.layout_direction());
        let draw = match &self.cache {
            Some((k, d)) if *k == key => d.clone(),
            _ => {
                let d = self.work_out(key.0, &key.1, key.2);
                self.cache = Some((key, d.clone()));
                d
            }
        };
        scope.draw_content();
        let brush = self.brush.clone();
        match draw {
            BorderDraw::Nothing => {}
            BorderDraw::Path(path) => scope.draw_path(&path, brush),
            BorderDraw::Bitmap { image, at, size, color_filter } => {
                scope.translate_with(at.x, at.y, |s| {
                    s.draw_image_rect_with(
                        &image,
                        IntOffset::Zero,
                        size,
                        IntOffset::Zero,
                        size,
                        1.0,
                        Fill,
                        color_filter,
                        BlendMode::SrcOver,
                        kompoz_graphics::DefaultFilterQuality,
                    );
                });
            }
            BorderDraw::RoundRect { corner, kind, top_left, size, stroke_width } => match kind {
                RoundKind::Fill => {
                    let full = scope.size();
                    scope.draw_round_rect_with(brush, Offset::Zero, full, corner, 1.0, Fill, None, BlendMode::SrcOver);
                }
                RoundKind::Clipped => {
                    let full = scope.size();
                    let (r, b) = (full.width - stroke_width, full.height - stroke_width);
                    scope.clip_rect_with(stroke_width, stroke_width, r, b, ClipOp::Difference, |s| {
                        s.draw_round_rect_with(brush, Offset::Zero, full, corner, 1.0, Fill, None, BlendMode::SrcOver);
                    });
                }
                RoundKind::Stroked => {
                    let stroke =
                        Stroke::new(stroke_width, Stroke::DefaultMiter, Stroke::DefaultCap, Stroke::DefaultJoin, None);
                    let corner = shrink(corner, stroke_width / 2.0);
                    scope.draw_round_rect_with(
                        brush,
                        top_left,
                        size,
                        corner,
                        1.0,
                        stroke.into(),
                        None,
                        BlendMode::SrcOver,
                    );
                }
            },
            BorderDraw::Rect { top_left, size, style } => {
                scope.draw_rect_with(brush, top_left, size, 1.0, style, None, BlendMode::SrcOver);
            }
        }
    }

    fn on_measure_result_changed(&mut self) {
        self.cache = None;
    }
}
