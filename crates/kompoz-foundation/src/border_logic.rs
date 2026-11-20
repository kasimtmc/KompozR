//! A border's drawing and its cache apart from a draw node (Compose's internal foundation/border/BorderLogic.kt,
//! what a style's border draws with): an "inner" border, its outer edge on the outline, worked out again for another
//! brush or outline, its width read as it is drawn (so an animated width redraws without working it out again).

use kompoz_graphics::{
    BlendMode, Brush, CornerRadius, IntSize, Offset, Outline, Path, PathDirection, PathOperation, Rect, RoundRect, Size,
};
use kompoz_ui::draw::{DefaultBlendMode, DrawScope, Fill, GraphicsLayer, Stroke};
use kompoz_ui::graphics_context::ClipOp;
use kompoz_ui::layout::CompositingStrategy;

/// What a border draws, worked out for an outline and a brush (Compose's cached `drawBorder` lambdas).
enum BorderDraw {
    /// A generic outline's (Compose's createDrawGenericBorder): the path filled where the border fills it, else
    /// stroked twice the width less what is outside it, in an offscreen layer.
    Generic { path: Path, mask_path: Path, bounds: Rect, bounds_size: IntSize },
    /// A rounded rect of one radius (Compose's simple createDrawRoundRectBorder).
    SimpleRoundRect(RoundRect),
    /// A rounded rect of several radii: a ring path, made again for another width.
    RoundRect { round_rect: RoundRect, last_stroke_width: f32, ring: Option<Path> },
    /// A rect (Compose's createDrawRectBorder).
    Rect(Rect),
}

/// A border's drawing logic (Compose's BorderLogic): one per border drawn, kept across draws.
#[derive(Default)]
pub(crate) struct BorderLogic {
    last_brush: Option<Brush>,
    last_outline: Option<Outline>,
    draw: Option<BorderDraw>,
}

fn min_dimension(rr: &RoundRect) -> f32 {
    rr.width().min(rr.height())
}

/// Compose's CornerRadius.shrink: less `value`, not below 0.
fn shrink(c: CornerRadius, value: f32) -> CornerRadius {
    CornerRadius::new((c.x - value).max(0.0), (c.y - value).max(0.0))
}

fn stroke(width: f32) -> Stroke {
    Stroke::new(width, Stroke::DefaultMiter, Stroke::DefaultCap, Stroke::DefaultJoin, None)
}

impl BorderLogic {
    /// Draws a border `width` px wide (read now) painted by `brush` along `outline`, moved by `offset`; a layer
    /// from `layer_provider` for a generic outline (Compose's `drawBorder`).
    pub(crate) fn draw_border(
        &mut self,
        scope: &mut DrawScope<'_>,
        width: f32,
        brush: &Brush,
        layer_provider: &mut dyn FnMut() -> GraphicsLayer,
        outline: &Outline,
        offset: Offset,
    ) {
        if self.last_brush.as_ref() != Some(brush) || self.last_outline.as_ref() != Some(outline) || self.draw.is_none()
        {
            self.last_brush = Some(brush.clone());
            self.last_outline = Some(outline.clone());
            self.draw = Some(match outline {
                Outline::Generic(path) => {
                    let bounds = path.get_bounds();
                    let mut bounds_path = Path::new();
                    bounds_path.add_rect(bounds, PathDirection::CounterClockwise);
                    let mut mask_path = Path::new();
                    mask_path.op(&bounds_path, path, PathOperation::Difference);
                    let bounds_size = IntSize::new(bounds.width().ceil() as i32, bounds.height().ceil() as i32);
                    BorderDraw::Generic { path: path.clone(), mask_path, bounds, bounds_size }
                }
                Outline::Rounded(rr) if rr.is_simple() => BorderDraw::SimpleRoundRect(*rr),
                Outline::Rounded(rr) => {
                    BorderDraw::RoundRect { round_rect: *rr, last_stroke_width: f32::NAN, ring: None }
                }
                Outline::Rectangle(rect) => BorderDraw::Rect(*rect),
            });
        }
        let stroke_width = width.max(0.0);
        let draw = self.draw.as_mut().expect("a border drawing");
        if offset == Offset::Zero {
            draw_with(draw, scope, stroke_width, brush, layer_provider);
        } else {
            scope.translate_with(offset.x, offset.y, |s| draw_with(draw, s, stroke_width, brush, layer_provider));
        }
    }
}

fn draw_with(
    draw: &mut BorderDraw,
    scope: &mut DrawScope<'_>,
    stroke_width: f32,
    brush: &Brush,
    layer_provider: &mut dyn FnMut() -> GraphicsLayer,
) {
    match draw {
        BorderDraw::Generic { path, mask_path, bounds, bounds_size } => {
            let fill_area = stroke_width * 2.0 > bounds.min_dimension();
            if fill_area {
                scope.draw_path_with(path, brush.clone(), 1.0, Fill, None, DefaultBlendMode);
                return;
            }
            let layer = layer_provider();
            layer.set_compositing_strategy(CompositingStrategy::Offscreen);
            let (left, top, size) = (bounds.left, bounds.top, *bounds_size);
            scope.translate_with(left, top, |s| {
                s.record_with(&layer, size, |r| {
                    r.translate_with(-left, -top, |t| {
                        t.draw_path_with(
                            path,
                            brush.clone(),
                            1.0,
                            stroke(stroke_width * 2.0).into(),
                            None,
                            DefaultBlendMode,
                        );
                        let (w, h) = (t.size().width, t.size().height);
                        let center = t.center();
                        t.scale_with((w + 1.0) / w, (h + 1.0) / h, center, |c| {
                            c.draw_path_with(mask_path, brush.clone(), 1.0, Fill, None, BlendMode::Clear);
                        });
                    });
                });
                s.draw_layer(&layer);
            });
        }
        BorderDraw::SimpleRoundRect(rr) => {
            let half = stroke_width / 2.0;
            let fill_area = stroke_width * 2.0 > min_dimension(rr);
            let corner = rr.top_left_corner_radius;
            let top_left = Offset::new(rr.left, rr.top);
            let size = Size::new(rr.width(), rr.height());
            if fill_area {
                scope.draw_round_rect_with(brush.clone(), top_left, size, corner, 1.0, Fill, None, DefaultBlendMode);
            } else if corner.x < half {
                let (l, t, r, b) =
                    (rr.left + stroke_width, rr.top + stroke_width, rr.right - stroke_width, rr.bottom - stroke_width);
                scope.clip_rect_with(l, t, r, b, ClipOp::Difference, |s| {
                    s.draw_round_rect_with(brush.clone(), top_left, size, corner, 1.0, Fill, None, DefaultBlendMode);
                });
            } else {
                scope.draw_round_rect_with(
                    brush.clone(),
                    Offset::new(rr.left + half, rr.top + half),
                    Size::new(rr.width() - stroke_width, rr.height() - stroke_width),
                    shrink(corner, half),
                    1.0,
                    stroke(stroke_width).into(),
                    None,
                    DefaultBlendMode,
                );
            }
        }
        BorderDraw::RoundRect { round_rect, last_stroke_width, ring } => {
            let fill_area = stroke_width * 2.0 > min_dimension(round_rect);
            if *last_stroke_width != stroke_width || ring.is_none() {
                *ring = Some(create_round_rect_path(round_rect, stroke_width, fill_area));
                *last_stroke_width = stroke_width;
            }
            let ring = ring.as_ref().expect("a ring");
            scope.draw_path_with(ring, brush.clone(), 1.0, Fill, None, DefaultBlendMode);
        }
        BorderDraw::Rect(rect) => {
            let fill_area = stroke_width * 2.0 > rect.min_dimension();
            let (top_left, size) = if fill_area {
                (rect.top_left(), rect.size())
            } else {
                (
                    Offset::new(rect.left + stroke_width / 2.0, rect.top + stroke_width / 2.0),
                    Size::new(rect.width() - stroke_width, rect.height() - stroke_width),
                )
            };
            let style = if fill_area { Fill } else { stroke(stroke_width).into() };
            scope.draw_rect_with(brush.clone(), top_left, size, 1.0, style, None, DefaultBlendMode);
        }
    }
}

/// A rounded rect less its inside by `stroke_width` (Compose's createRoundRectPath): the whole of it where the
/// border fills it.
fn create_round_rect_path(rr: &RoundRect, stroke_width: f32, fill_area: bool) -> Path {
    let mut outer = Path::new();
    outer.add_round_rect(rr, PathDirection::CounterClockwise);
    if fill_area {
        return outer;
    }
    let inset = RoundRect {
        left: rr.left + stroke_width,
        top: rr.top + stroke_width,
        right: rr.right - stroke_width,
        bottom: rr.bottom - stroke_width,
        top_left_corner_radius: shrink(rr.top_left_corner_radius, stroke_width),
        top_right_corner_radius: shrink(rr.top_right_corner_radius, stroke_width),
        bottom_right_corner_radius: shrink(rr.bottom_right_corner_radius, stroke_width),
        bottom_left_corner_radius: shrink(rr.bottom_left_corner_radius, stroke_width),
    };
    let mut inner = Path::new();
    inner.add_round_rect(&inset, PathDirection::CounterClockwise);
    let mut ring = Path::new();
    ring.op(&outer, &inner, PathOperation::Difference);
    ring
}
