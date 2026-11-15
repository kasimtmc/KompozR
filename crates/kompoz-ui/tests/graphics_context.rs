//! GraphicsContext's provided methods: the primitives each draws through, with C++'s numbers.

mod common;
use common::*;

use kompoz_graphics::colorspace::ColorSpaces;
use kompoz_graphics::*;
use kompoz_ui::graphics_context::*;
use std::rc::Rc;

/// One call a recorder took, with its arguments.
#[derive(Clone, PartialEq, Debug)]
enum Call {
    FillRoundRect(Rect, CornerRadii, Color),
    StrokeRoundRect(Rect, CornerRadii, f32, Color),
    Line(Offset, Offset, f32, Color),
    Text(String, Color),
    Arc(Rect, f32, f32, f32, Color, bool),
    Image(Rect, Rect, f32, FilterQuality),
    PushLayer(Rect, CornerRadii, f32, LayerTransform),
    PopLayer,
    PushClip(Rect),
    PushClipRoundRect(Rect, CornerRadii, ClipOp),
    PushClipPath(Path, Offset, ClipOp),
    PopClip,
    PushTranslation(Offset),
    PopTranslation,
    PushTransform(Matrix),
    PopTransform,
    PushColorFilter(ColorFilter),
    PopColorFilter,
    Vertices(Offset, BlendMode, Brush, Rect, f32),
    Path(Path, Offset, Brush, Rect, PathStroke),
    // What the recorders that hook more record.
    PushEffectLayer(Rect, RenderEffect, f32),
    PushBlendMode(BlendMode),
    PopBlendMode,
    FillGradient(Rect, CornerRadii, Brush, Rect),
    OutlineLayer(Rect, Path, f32, LayerTransform),
    Shadow(Rect, CornerRadii, Shadow, bool),
    ShadowPath(Path, Offset, Shadow, bool),
}

/// A recorder of the required methods (and `$extra` methods of its own): what it is asked to draw, in order.
macro_rules! recorder {
    ($name:ident { $($extra:tt)* }) => {
        #[derive(Default)]
        #[allow(dead_code)] // (what a recorder overrides reads its fields)
        struct $name {
            calls: Vec<Call>,
            reject: bool,
            light: ShadowLight,
        }

        impl GraphicsContext for $name {
            fn fill_round_rect(&mut self, rect: Rect, radii: CornerRadii, color: Color) {
                self.calls.push(Call::FillRoundRect(rect, radii, color));
            }
            fn stroke_round_rect(&mut self, rect: Rect, radii: CornerRadii, width: f32, color: Color) {
                self.calls.push(Call::StrokeRoundRect(rect, radii, width, color));
            }
            fn draw_line(&mut self, from: Offset, to: Offset, width: f32, color: Color) {
                self.calls.push(Call::Line(from, to, width, color));
            }
            fn draw_text(&mut self, text: &TextDraw, color: Color) {
                self.calls.push(Call::Text(text.text.to_string(), color));
            }
            fn draw_arc(&mut self, oval: Rect, start: f32, sweep: f32, width: f32, color: Color, round_cap: bool) {
                self.calls.push(Call::Arc(oval, start, sweep, width, color, round_cap));
            }
            fn draw_image(&mut self, _: &ImageBitmap, src: Rect, dst: Rect, alpha: f32, quality: FilterQuality) {
                self.calls.push(Call::Image(src, dst, alpha, quality));
            }
            fn push_layer(&mut self, bounds: Rect, radii: CornerRadii, alpha: f32, transform: LayerTransform) {
                self.calls.push(Call::PushLayer(bounds, radii, alpha, transform));
            }
            fn pop_layer(&mut self) {
                self.calls.push(Call::PopLayer);
            }
            fn push_clip(&mut self, rect: Rect) {
                self.calls.push(Call::PushClip(rect));
            }
            fn push_clip_round_rect(&mut self, rect: Rect, radii: CornerRadii, op: ClipOp) {
                self.calls.push(Call::PushClipRoundRect(rect, radii, op));
            }
            fn push_clip_path(&mut self, path: &Path, origin: Offset, op: ClipOp) {
                self.calls.push(Call::PushClipPath(path.clone(), origin, op));
            }
            fn pop_clip(&mut self) {
                self.calls.push(Call::PopClip);
            }
            fn push_translation(&mut self, by: Offset) {
                self.calls.push(Call::PushTranslation(by));
            }
            fn pop_translation(&mut self) {
                self.calls.push(Call::PopTranslation);
            }
            fn push_transform(&mut self, matrix: &Matrix) {
                self.calls.push(Call::PushTransform(*matrix));
            }
            fn pop_transform(&mut self) {
                self.calls.push(Call::PopTransform);
            }
            fn push_color_filter(&mut self, filter: &ColorFilter) {
                self.calls.push(Call::PushColorFilter(*filter));
            }
            fn pop_color_filter(&mut self) {
                self.calls.push(Call::PopColorFilter);
            }
            fn draw_vertices(
                &mut self,
                _: &Vertices,
                origin: Offset,
                blend_mode: BlendMode,
                brush: &Brush,
                brush_area: Rect,
                alpha: f32,
            ) {
                self.calls.push(Call::Vertices(origin, blend_mode, brush.clone(), brush_area, alpha));
            }
            fn draw_path(&mut self, path: &Path, origin: Offset, brush: &Brush, brush_area: Rect, stroke: &PathStroke) {
                self.calls.push(Call::Path(path.clone(), origin, brush.clone(), brush_area, stroke.clone()));
            }
            fn light(&self) -> ShadowLight {
                self.light
            }
            $($extra)*
        }
    };
}

/// What a backend draws itself besides the primitives: effect layers, blend modes, gradients, outline layers, and
/// what it rejects.
macro_rules! backend_methods {
    () => {
        fn push_effect_layer(&mut self, bounds: Rect, effect: &RenderEffect, alpha: f32) {
            self.calls.push(Call::PushEffectLayer(bounds, effect.clone(), alpha));
        }
        fn push_blend_mode(&mut self, mode: BlendMode) {
            self.calls.push(Call::PushBlendMode(mode));
        }
        fn pop_blend_mode(&mut self) {
            self.calls.push(Call::PopBlendMode);
        }
        fn fill_gradient(&mut self, rect: Rect, radii: CornerRadii, brush: &Brush, brush_area: Rect) {
            self.calls.push(Call::FillGradient(rect, radii, brush.clone(), brush_area));
        }
        fn push_outline_layer(&mut self, bounds: Rect, outline: &Path, alpha: f32, transform: LayerTransform) {
            self.calls.push(Call::OutlineLayer(bounds, outline.clone(), alpha, transform));
        }
        fn quick_reject(&self, _: Rect) -> bool {
            self.reject
        }
    };
}

// Only the required methods: the rest as provided.
recorder!(Bare {});
// A backend's own methods too; shadows as provided.
recorder!(Recorder { backend_methods!(); });
// Shadows recorded as asked for, not drawn.
recorder!(Hooked {
    backend_methods!();
    fn draw_shadow(&mut self, rect: Rect, radii: CornerRadii, shadow: &Shadow, inner: bool) {
        self.calls.push(Call::Shadow(rect, radii, shadow.clone(), inner));
    }
    fn draw_shadow_path(&mut self, outline: &Path, origin: Offset, shadow: &Shadow, inner: bool) {
        self.calls.push(Call::ShadowPath(outline.clone(), origin, shadow.clone(), inner));
    }
});

const RECT: Rect = Rect::new(10.0, 20.0, 110.0, 70.0);

fn ltr() -> (LayoutDirection, Density) {
    (LayoutDirection::Ltr, Density::default())
}

fn with_alpha(c: Color, alpha: f32) -> Color {
    Color::new_in(c.red(), c.green(), c.blue(), alpha, c.color_space())
}

/// A triangle across the top middle and the bottom corners.
fn triangle() -> GenericShape {
    GenericShape(|p, s, _| {
        p.move_to(s.width / 2.0, 0.0);
        p.line_to(s.width, s.height);
        p.line_to(0.0, s.height);
    })
}

fn triangle_path(w: f32, h: f32, at: Offset) -> Path {
    let mut p = Path::new();
    p.move_to(at.x + w / 2.0, at.y);
    p.line_to(at.x + w, at.y + h);
    p.line_to(at.x, at.y + h);
    p.close();
    p
}

/// Rounded corners that are no circles (10 across, 5 down).
fn elliptical() -> OutlineShape {
    OutlineShape(|s, _, _| Outline::Rounded(RoundRect::from_rect_radius(s.to_rect(), CornerRadius::new(10.0, 5.0))))
}

fn round_rect_path(rect: Rect, corner: CornerRadius) -> Path {
    let mut p = Path::new();
    p.add_round_rect(&RoundRect::from_rect_radius(rect, corner), PathDirection::CounterClockwise);
    p
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

fn near_rect(a: Rect, b: Rect) -> bool {
    near(a.left, b.left) && near(a.top, b.top) && near(a.right, b.right) && near(a.bottom, b.bottom)
}

// ---- Values ------------------------------------------------------------------------------------------------------

fn corner_radii_helpers() {
    let r = CornerRadii::new(1.0, 2.0, 3.0, 4.0);
    assert!(!r.uniform() && CornerRadii::all(5.0).uniform() && CornerRadii::Zero.uniform());
    assert_eq!(r.max(), 4.0);
    assert_eq!(r.scaled(2.0), CornerRadii::new(2.0, 4.0, 6.0, 8.0));
    assert_eq!(r.grown(1.5), CornerRadii::new(2.5, 3.5, 4.5, 5.5));
    assert_eq!(CornerRadii::from(3.0), CornerRadii::all(3.0));
    assert_eq!(CornerRadii::default(), CornerRadii::Zero);
}

fn values_and_defaults() {
    let t = LayerTransform::default();
    assert!(t.is_identity() && t.origin == Offset::new(0.5, 0.5));
    assert!(!LayerTransform { rotation_z: 90.0, ..t }.is_identity());
    assert!(LayerTransform { origin: Offset::Zero, ..t }.is_identity());

    let s = Shadow::default();
    assert_eq!(s.color, Color::Black);
    assert_eq!((s.alpha, s.blend_mode, s.brush.is_none()), (1.0, BlendMode::SrcOver, true));
    assert_eq!(
        Shadow::new(Color::Red, Offset::new(1.0, 2.0), 3.0, 4.0),
        Shadow::new(Color::Red, Offset::new(1.0, 2.0), 3.0, 4.0)
    );
    assert_ne!(
        Shadow::new(Color::Red, Offset::Zero, 3.0, 4.0),
        Shadow { alpha: 0.5, ..Shadow::new(Color::Red, Offset::Zero, 3.0, 4.0) }
    );

    let l = ShadowLight::default();
    assert_eq!((l.center, l.z, l.radius, l.ambient_alpha, l.spot_alpha), (Offset::Zero, 500.0, 800.0, 0.039, 0.19));
    assert_eq!(ElevationShadow::flat(8.0).plane, [0.0, 0.0, 8.0]);
    assert_eq!(DefaultShadowColor, Color::Black);
    assert_eq!(
        (NodeRecording::default(), LayerContent::default(), ClipOp::default()),
        (NodeRecording::Other, LayerContent::Draw, ClipOp::Intersect)
    );

    // A run of text: equal as long as it shows the same (its glyph runs the same Rc, or of the same content).
    let style = Rc::new(TextRunStyle::new(14.0));
    let text = kompoz_text::FakeBackend.text_draw("Hello", Offset::new(1.0, 12.0), style.clone(), false);
    assert!(!text.rtl && text.size == Size::new(35.0, 17.5) && text.font_size() == 14.0);
    assert_eq!(text.ink_rect(), Rect::new(1.0, 12.0 - 11.2, 1.0 + 35.0, 12.0 + 2.8));
    let printed = format!("{text:?}");
    assert!(printed.contains("Hello") && printed.contains("12.0") && printed.contains("35.0"), "{printed}");
    assert_eq!(text, text.clone());
    let again = kompoz_text::FakeBackend.text_draw("Hello", Offset::new(1.0, 12.0), style, false);
    assert!(!Rc::ptr_eq(&text.glyphs, &again.glyphs) && text == again);
    let mut moved: Vec<GlyphRun> = text.glyphs.to_vec();
    moved[0].glyphs[1].x += 1.0;
    let reshaped = TextDraw::new(
        text.text.clone(),
        text.baseline,
        text.size,
        text.ink_bounds,
        false,
        text.style.clone(),
        moved.into(),
    );
    assert_ne!(text, reshaped);
    assert_eq!(reshaped, reshaped.clone());
    let bolder = TextDraw {
        style: Rc::new(TextRunStyle { font_weight: kompoz_text::FontWeight::Bold, ..(*text.style).clone() }),
        ..text.clone()
    };
    assert_ne!(text, bolder);
}

// ---- Shapes ------------------------------------------------------------------------------------------------------

fn fill_shape_by_outline() {
    let (ld, d) = ltr();
    let mut g = Recorder::default();
    g.fill_shape(RECT, &RectangleShape, ld, &d, Color::Red);
    g.fill_shape(RECT, &RoundedCornerShape(8.0), ld, &d, Color::Red);
    g.fill_shape(RECT, &CircleShape, ld, &d, Color::Red);
    g.fill_shape(RECT, &triangle(), ld, &d, Color::Red);
    g.fill_shape(RECT, &elliptical(), ld, &d, Color::Red);
    assert_eq!(
        g.calls,
        vec![
            Call::FillRoundRect(RECT, CornerRadii::Zero, Color::Red),
            Call::FillRoundRect(RECT, CornerRadii::all(8.0), Color::Red),
            Call::FillRoundRect(RECT, CornerRadii::all(25.0), Color::Red),
            Call::Path(
                triangle_path(100.0, 50.0, Offset::Zero),
                RECT.top_left(),
                SolidColor(Color::Red),
                RECT,
                PathStroke::default()
            ),
            Call::Path(
                round_rect_path(Rect::new(0.0, 0.0, 100.0, 50.0), CornerRadius::new(10.0, 5.0)),
                RECT.top_left(),
                SolidColor(Color::Red),
                RECT,
                PathStroke::default()
            ),
        ]
    );
}

fn fill_shape_with_a_brush() {
    let (ld, d) = ltr();
    let brush = Brush::horizontal_gradient(&[Color::Red, Color::Blue], 0.0, f32::INFINITY, TileMode::Clamp);
    let area = Rect::new(0.0, 0.0, 200.0, 100.0);
    let mut g = Recorder::default();
    g.fill_shape_brush(RECT, &RoundedCornerShape(8.0), ld, &d, &brush, area);
    g.fill_shape_brush(RECT, &triangle(), ld, &d, &brush, area);
    assert_eq!(
        g.calls,
        vec![
            Call::FillGradient(RECT, CornerRadii::all(8.0), brush.clone(), area),
            Call::Path(
                triangle_path(100.0, 50.0, Offset::Zero),
                RECT.top_left(),
                brush.clone(),
                area,
                PathStroke::default()
            ),
        ]
    );
}

fn rtl_shapes_mirror() {
    let d = Density::default();
    let mut g = Recorder::default();
    let shape = RoundedCornerShape::new(4.0, 8.0, 0.0, 0.0);
    g.fill_shape(RECT, &shape, LayoutDirection::Ltr, &d, Color::Red);
    g.fill_shape(RECT, &shape, LayoutDirection::Rtl, &d, Color::Red);
    assert_eq!(
        g.calls,
        vec![
            Call::FillRoundRect(RECT, CornerRadii::new(4.0, 8.0, 0.0, 0.0), Color::Red),
            Call::FillRoundRect(RECT, CornerRadii::new(8.0, 4.0, 0.0, 0.0), Color::Red),
        ]
    );
}

fn stroke_shape_inside_the_bounds() {
    let (ld, d) = ltr();
    let mut g = Recorder::default();
    g.stroke_shape(RECT, &RoundedCornerShape(8.0), ld, &d, 4.0, Color::Red);
    g.stroke_shape(RECT, &triangle(), ld, &d, 4.0, Color::Red);
    g.stroke_shape(RECT, &elliptical(), ld, &d, 4.0, Color::Red);
    let stroke = PathStroke { width: 4.0, join: StrokeJoin::Round, ..PathStroke::default() };
    let inner = Rect::new(2.0, 2.0, 98.0, 48.0);
    assert_eq!(
        g.calls,
        vec![
            Call::StrokeRoundRect(RECT, CornerRadii::all(8.0), 4.0, Color::Red),
            // The triangle in 96 x 46, moved half the width in.
            Call::Path(
                triangle_path(96.0, 46.0, Offset::new(2.0, 2.0)),
                RECT.top_left(),
                SolidColor(Color::Red),
                RECT,
                stroke.clone()
            ),
            // The corners half the width smaller.
            Call::Path(
                round_rect_path(inner, CornerRadius::new(8.0, 3.0)),
                RECT.top_left(),
                SolidColor(Color::Red),
                RECT,
                stroke.clone()
            ),
        ]
    );

    let brush = Brush::vertical_gradient(&[Color::Red, Color::Blue], 0.0, f32::INFINITY, TileMode::Clamp);
    let area = Rect::new(0.0, 0.0, 300.0, 300.0);
    let mut g = Recorder::default();
    g.stroke_shape_brush(RECT, &RoundedCornerShape(8.0), ld, &d, 4.0, &brush, area);
    g.stroke_shape_brush(RECT, &RectangleShape, ld, &d, 120.0, &brush, area);
    assert_eq!(
        g.calls,
        vec![
            Call::Path(
                round_rect_path(inner, CornerRadius::circular(6.0)),
                RECT.top_left(),
                brush.clone(),
                area,
                stroke
            ),
            // Wider than the rect: no room inside, the path a point at the middle.
            Call::Path(
                round_rect_path(Rect::new(60.0, 60.0, 60.0, 60.0), CornerRadius::Zero),
                RECT.top_left(),
                brush.clone(),
                area,
                PathStroke { width: 120.0, join: StrokeJoin::Round, ..PathStroke::default() }
            ),
        ]
    );
}

fn push_shape_layer_by_outline() {
    let (ld, d) = ltr();
    let t = LayerTransform { scale_x: 2.0, ..LayerTransform::Identity };
    let mut g = Recorder::default();
    g.push_shape_layer(RECT, &CircleShape, ld, &d, 0.5, t);
    g.push_shape_layer(RECT, &triangle(), ld, &d, 0.5, LayerTransform::Identity);
    assert_eq!(
        g.calls,
        vec![
            Call::PushLayer(RECT, CornerRadii::all(25.0), 0.5, t),
            Call::OutlineLayer(RECT, triangle_path(100.0, 50.0, Offset::Zero), 0.5, LayerTransform::Identity),
        ]
    );
    // The provided outline layer: one clipped to the bounds.
    let mut g = Bare::default();
    g.push_shape_layer(RECT, &triangle(), ld, &d, 0.5, t);
    assert_eq!(g.calls, vec![Call::PushLayer(RECT, CornerRadii::Zero, 0.5, t)]);
}

fn push_clip_shape_cases() {
    let (ld, d) = ltr();
    let mut g = Recorder::default();
    g.push_clip_shape(RECT, &RectangleShape, ld, &d, ClipOp::Intersect);
    g.push_clip_shape(RECT, &RectangleShape, ld, &d, ClipOp::Difference);
    g.push_clip_shape(RECT, &RoundedCornerShape(8.0), ld, &d, ClipOp::Intersect);
    g.push_clip_shape(RECT, &triangle(), ld, &d, ClipOp::Difference);
    assert_eq!(
        g.calls,
        vec![
            Call::PushClip(RECT),
            Call::PushClipRoundRect(RECT, CornerRadii::Zero, ClipOp::Difference),
            Call::PushClipRoundRect(RECT, CornerRadii::all(8.0), ClipOp::Intersect),
            Call::PushClipPath(triangle_path(100.0, 50.0, Offset::Zero), RECT.top_left(), ClipOp::Difference),
        ]
    );
}

// ---- Shadows -----------------------------------------------------------------------------------------------------

fn draw_shadow_rings() {
    let color = Color::new(0.0, 0.0, 0.0, 0.8);
    let shadow = Shadow::new(color, Offset::new(2.0, 3.0), 12.0, 1.0);
    let rect = Rect::new(0.0, 0.0, 100.0, 50.0);
    let mut g = Recorder::default();
    g.draw_shadow(rect, CornerRadii::all(4.0), &shadow, false);
    let rings: Vec<Call> = [(6, 13.0), (5, 11.0), (4, 9.0), (3, 7.0), (2, 5.0), (1, 3.0)]
        .iter()
        .map(|&(i, grow)| {
            let t = i as f32 / 6.0;
            Call::FillRoundRect(
                Rect::new(2.0 - grow, 3.0 - grow, 102.0 + grow, 53.0 + grow),
                CornerRadii::all(4.0 + grow),
                with_alpha(color, color.alpha() * (1.0 - t + 1.0 / 6.0) / 3.5),
            )
        })
        .collect();
    assert_eq!(g.calls, rings);
    // The outermost ring the faintest, the innermost a third of the color's alpha.
    let alpha = |c: &Call| if let Call::FillRoundRect(_, _, c) = c { c.alpha() } else { unreachable!() };
    assert!(alpha(&g.calls[0]) < alpha(&g.calls[5]) && (alpha(&g.calls[5]) - 0.8 / 3.5).abs() < 0.004);

    // None inside, none of no color, none that is no bigger than nothing.
    let mut g = Recorder::default();
    g.draw_shadow(rect, CornerRadii::Zero, &shadow, true);
    g.draw_shadow(rect, CornerRadii::Zero, &Shadow { color: Color::Transparent, ..shadow.clone() }, false);
    assert!(g.calls.is_empty());
    let small = Rect::new(0.0, 0.0, 10.0, 10.0);
    g.draw_shadow(small, CornerRadii::Zero, &Shadow::new(color, Offset::Zero, 12.0, -8.0), false);
    assert_eq!(g.calls.len(), 5); // (the innermost, 6 px smaller each side, skipped)
    assert_eq!(
        g.calls[4],
        Call::FillRoundRect(small.inflate(-4.0), CornerRadii::all(-4.0), {
            let t = 2.0 / 6.0;
            with_alpha(color, color.alpha() * (1.0 - t + 1.0 / 6.0) / 3.5)
        })
    );
}

/// A right triangle 40 x 20 from the origin.
fn corner_path() -> Path {
    let mut p = Path::new();
    p.move_to(0.0, 0.0);
    p.line_to(40.0, 0.0);
    p.line_to(40.0, 20.0);
    p.close();
    p
}

fn shadow_path_reach_numbers() {
    let b = Rect::new(0.0, 0.0, 10.0, 10.0);
    let s = Shadow::new(Color::Black, Offset::new(1.0, 2.0), 3.0, 1.0);
    assert_eq!(shadow_path_reach(b, &s, true), Rect::new(-2.0, -2.0, 12.0, 12.0));
    // Moved, spread four times (a miter's reach), blurred three sigmas (0.57735 r + 0.5).
    let e = 3.0 * (0.57735 * 3.0 + 0.5);
    let reach = shadow_path_reach(b, &s, false);
    assert!(near_rect(reach, Rect::new(-3.0 - e, -2.0 - e, 15.0 + e, 16.0 + e)), "{reach:?}");
    assert!(near(e, 6.69615));
    // A negative blur and spread as none.
    let s = Shadow::new(Color::Black, Offset::new(1.0, 2.0), -3.0, -1.0);
    assert_eq!(shadow_path_reach(b, &s, false), Rect::new(1.0, 2.0, 11.0, 12.0));
}

fn draw_shadow_path_drop() {
    let path = corner_path();
    let origin = Offset::new(10.0, 10.0);
    let color = Color::new(1.0, 0.0, 0.0, 0.5);
    let shadow = Shadow::new(color, Offset::new(3.0, 4.0), 6.0, 2.0);
    let opaque = SolidColor(Color::new(1.0, 0.0, 0.0, 1.0));
    let blur = BlurEffect(6.0, 6.0, TileMode::Decal);
    let at = Rect::new(5.0, 6.0, 61.0, 42.0); // (the bounds at the origin moved, and 2 x 4 more each side)
    let reach = blur.output_bounds(at);
    assert_eq!(reach, shadow_path_reach(Rect::new(10.0, 10.0, 50.0, 30.0), &shadow, false));
    let moved = Offset::new(13.0, 14.0);
    let mut g = Recorder::default();
    g.draw_shadow_path(&path, origin, &shadow, false);
    assert_eq!(
        g.calls,
        vec![
            Call::PushLayer(reach, CornerRadii::Zero, color.alpha(), LayerTransform::Identity),
            Call::PushEffectLayer(at, blur.clone(), 1.0),
            Call::Path(path.clone(), moved, opaque.clone(), at, PathStroke::default()),
            Call::PopLayer,
            Call::PushEffectLayer(at, blur.clone(), 1.0),
            Call::Path(path.clone(), moved, opaque.clone(), at, PathStroke::new(4.0)),
            Call::PopLayer,
            Call::PopLayer,
        ]
    );

    // No spread: the fill alone.
    let mut g = Recorder::default();
    g.draw_shadow_path(&path, origin, &Shadow { spread: 0.0, ..shadow.clone() }, false);
    let at = Rect::new(13.0, 14.0, 53.0, 34.0);
    assert_eq!(
        g.calls,
        vec![
            Call::PushLayer(blur.output_bounds(at), CornerRadii::Zero, color.alpha(), LayerTransform::Identity),
            Call::PushEffectLayer(at, blur.clone(), 1.0),
            Call::Path(path.clone(), moved, opaque.clone(), at, PathStroke::default()),
            Call::PopLayer,
            Call::PopLayer,
        ]
    );

    // Nothing where it can't show, of no color, or of no path.
    let mut g = Recorder { reject: true, ..Recorder::default() };
    g.draw_shadow_path(&path, origin, &shadow, false);
    g.draw_shadow_path(&path, origin, &shadow, true);
    g.reject = false;
    g.draw_shadow_path(&path, origin, &Shadow { color: Color::Transparent, ..shadow.clone() }, false);
    g.draw_shadow_path(&Path::new(), origin, &shadow, false);
    assert!(g.calls.is_empty());
}

fn draw_shadow_path_inner() {
    let path = corner_path();
    let origin = Offset::new(10.0, 10.0);
    let color = Color::new(0.0, 0.0, 1.0, 0.5);
    let shadow = Shadow::new(color, Offset::new(3.0, 4.0), 6.0, 2.0);
    let opaque = Color::new(0.0, 0.0, 1.0, 1.0);
    let blur = BlurEffect(6.0, 6.0, TileMode::Decal);
    let at = Rect::new(8.0, 8.0, 52.0, 32.0);
    let hole = Rect::new(11.0, 12.0, 55.0, 36.0);
    let moved = Offset::new(13.0, 14.0);
    let mut g = Recorder::default();
    g.draw_shadow_path(&path, origin, &shadow, true);
    assert_eq!(
        g.calls,
        vec![
            Call::PushClipPath(path.clone(), origin, ClipOp::Intersect),
            Call::PushLayer(at, CornerRadii::Zero, color.alpha(), LayerTransform::Identity),
            Call::FillRoundRect(at, CornerRadii::Zero, opaque),
            Call::PushBlendMode(BlendMode::DstOut),
            Call::PushEffectLayer(hole, blur.clone(), 1.0),
            Call::Path(path.clone(), moved, SolidColor(opaque), hole, PathStroke::default()),
            Call::PushBlendMode(BlendMode::Clear),
            Call::Path(path.clone(), moved, SolidColor(opaque), hole, PathStroke::new(4.0)),
            Call::PopBlendMode,
            Call::PopLayer,
            Call::PopBlendMode,
            Call::PopLayer,
            Call::PopClip,
        ]
    );
    // No spread: no band cleared.
    let mut g = Recorder::default();
    g.draw_shadow_path(&path, origin, &Shadow { spread: -1.0, ..shadow.clone() }, true);
    assert_eq!(g.calls.len(), 10);
    assert!(!g.calls.contains(&Call::PushBlendMode(BlendMode::Clear)));
}

fn draw_shape_shadow_with_a_color() {
    let (ld, d) = ltr();
    let color = Color::new(0.0, 0.0, 0.0, 0.5);
    let shadow = Shadow { alpha: 0.5, ..Shadow::new(color, Offset::new(2.0, 3.0), 4.0, 1.0) };
    let drawn = Shadow::new(with_alpha(color, color.alpha() * 0.5), Offset::new(2.0, 3.0), 4.0, 1.0);
    let bitmap = Rect::new(7.0, 18.0, 117.0, 78.0); // (moved, and blur + spread more each side)
    let mut g = Hooked::default();
    g.draw_shape_shadow(RECT, &RoundedCornerShape(8.0), ld, &d, &shadow, false);
    assert_eq!(
        g.calls,
        vec![Call::PushClip(bitmap), Call::Shadow(RECT, CornerRadii::all(8.0), drawn.clone(), false), Call::PopClip]
    );

    // Inside: not cut; in its blend mode; a path's for a shape of no rounded rect.
    let mut g = Hooked::default();
    let multiply = Shadow { blend_mode: BlendMode::Multiply, ..shadow.clone() };
    g.draw_shape_shadow(RECT, &triangle(), ld, &d, &multiply, true);
    assert_eq!(
        g.calls,
        vec![
            Call::PushBlendMode(BlendMode::Multiply),
            Call::ShadowPath(triangle_path(100.0, 50.0, Offset::Zero), RECT.top_left(), drawn.clone(), true),
            Call::PopBlendMode,
        ]
    );

    // A brush of one color: that color.
    let mut g = Hooked::default();
    g.draw_shape_shadow(
        RECT,
        &RectangleShape,
        ld,
        &d,
        &Shadow { brush: Some(SolidColor(Color::Red)), ..shadow.clone() },
        false,
    );
    assert_eq!(
        g.calls,
        vec![
            Call::PushClip(bitmap),
            Call::Shadow(
                RECT,
                CornerRadii::Zero,
                Shadow { color: with_alpha(Color::Red, 0.5), ..drawn.clone() },
                false
            ),
            Call::PopClip,
        ]
    );
}

fn draw_shape_shadow_with_a_gradient() {
    let (ld, d) = ltr();
    let brush = Brush::linear_gradient(&[Color::Red, Color::Blue], Offset::Zero, Offset::Infinite, TileMode::Clamp);
    let shadow =
        Shadow { brush: Some(brush.clone()), alpha: 0.5, ..Shadow::new(Color::Black, Offset::new(2.0, 3.0), 4.0, 1.0) };
    let white = Shadow::new(Color::White, Offset::new(2.0, 3.0), 4.0, 1.0);
    let bitmap = Rect::new(7.0, 18.0, 117.0, 78.0);
    let mut g = Hooked::default();
    g.draw_shape_shadow(RECT, &RoundedCornerShape(8.0), ld, &d, &shadow, false);
    assert_eq!(
        g.calls,
        vec![
            Call::PushClip(bitmap),
            Call::PushLayer(bitmap, CornerRadii::Zero, 0.5, LayerTransform::Identity),
            Call::Shadow(RECT, CornerRadii::all(8.0), white.clone(), false),
            Call::PushBlendMode(BlendMode::SrcIn),
            Call::FillGradient(bitmap, CornerRadii::Zero, brush.clone(), bitmap),
            Call::PopBlendMode,
            Call::PopLayer,
            Call::PopClip,
        ]
    );
    // Inside: over the shape.
    let mut g = Hooked::default();
    g.draw_shape_shadow(RECT, &RoundedCornerShape(8.0), ld, &d, &Shadow { alpha: 2.0, ..shadow.clone() }, true);
    assert_eq!(
        g.calls,
        vec![
            Call::PushLayer(RECT, CornerRadii::Zero, 1.0, LayerTransform::Identity),
            Call::Shadow(RECT, CornerRadii::all(8.0), white, true),
            Call::PushBlendMode(BlendMode::SrcIn),
            Call::FillGradient(RECT, CornerRadii::Zero, brush, RECT),
            Call::PopBlendMode,
            Call::PopLayer,
        ]
    );
}

fn draw_elevation_shadow_ambient_and_spot() {
    let outline = Outline::Rectangle(Rect::new(0.0, 0.0, 100.0, 50.0));
    let origin = Offset::new(10.0, 20.0);
    let b = RECT;
    // Ambient: z 8, r 4, blur 0.5 r (1 + z / 128) = 2.125, stroked 0.5 (r - blur) = 0.9375 (a spread of half that).
    let ambient = Shadow::new(with_alpha(Color::Black, 0.039), Offset::Zero, 2.125, 0.46875);
    // Spot: z / (Lz - z) and Lz / (Lz - z).
    let (ratio, scale) = (8.0f32 / 492.0, 500.0f32 / 492.0);
    let spot = Shadow::new(with_alpha(Color::Black, 0.19), Offset::Zero, 800.0 * ratio, 0.0);
    let cast = |by: Offset| {
        Rect::new(b.left * scale + by.x, b.top * scale + by.y, b.right * scale + by.x, b.bottom * scale + by.y)
    };
    let mut g = Hooked::default();
    g.draw_elevation_shadow(&outline, origin, &ElevationShadow::flat(8.0));
    assert_eq!(
        g.calls,
        vec![
            Call::Shadow(b, CornerRadii::Zero, ambient.clone(), false),
            Call::Shadow(cast(Offset::Zero), CornerRadii::Zero, spot.clone(), false),
        ]
    );
    assert!(near(2.125, 0.5 * 4.0 * (1.0 + 8.0 / 128.0)) && near(800.0 * ratio, 13.00813));

    // Cast from the light's center; corners scaled with the rect.
    let mut g = Hooked {
        light: ShadowLight { center: Offset::new(100.0, -200.0), ..ShadowLight::default() },
        ..Hooked::default()
    };
    let rounded =
        Outline::Rounded(RoundRect::from_rect_radius(Rect::new(0.0, 0.0, 100.0, 50.0), CornerRadius::circular(6.0)));
    g.draw_elevation_shadow(&rounded, origin, &ElevationShadow::flat(8.0));
    assert_eq!(
        g.calls,
        vec![
            Call::Shadow(b, CornerRadii::all(6.0), ambient.clone(), false),
            Call::Shadow(
                cast(Offset::new(-ratio * 100.0, ratio * 200.0)),
                CornerRadii::all(6.0 * scale),
                spot.clone(),
                false
            ),
        ]
    );

    // A path's, moved to the origin and scaled with it.
    let triangle = triangle_path(100.0, 50.0, Offset::Zero);
    let mut g = Hooked::default();
    g.draw_elevation_shadow(&Outline::Generic(triangle.clone()), origin, &ElevationShadow::flat(8.0));
    let mut scaled = Path::new();
    scaled.move_to(60.0 * scale, 20.0 * scale);
    scaled.line_to(110.0 * scale, 70.0 * scale);
    scaled.line_to(10.0 * scale, 70.0 * scale);
    scaled.close();
    assert_eq!(
        g.calls,
        vec![
            Call::ShadowPath(triangle_path(100.0, 50.0, origin), Offset::Zero, ambient.clone(), false),
            Call::ShadowPath(scaled, Offset::Zero, spot.clone(), false),
        ]
    );

    // The ambient shadow as high as the outline's origin, the spot one as its middle; none for a caster unseen.
    let mut g = Hooked::default();
    let tilted = ElevationShadow { plane: [0.0, 0.1, 0.0], ..ElevationShadow::default() };
    g.draw_elevation_shadow(&outline, Offset::new(10.0, 0.0), &tilted);
    assert_eq!(g.calls.len(), 1);
    assert!(matches!(&g.calls[0], Call::Shadow(_, _, s, false) if near(s.blur_radius, 800.0 * 2.5 / 497.5)));
    let mut g = Hooked::default();
    g.draw_elevation_shadow(&outline, origin, &ElevationShadow { alpha: 0.0, ..ElevationShadow::flat(8.0) });
    g.draw_elevation_shadow(&outline, origin, &ElevationShadow::flat(0.0));
    g.draw_elevation_shadow(&outline, origin, &ElevationShadow::flat(600.0)); // (above the light: an ambient one)
    assert_eq!(g.calls.len(), 1);
    assert!(matches!(&g.calls[0], Call::Shadow(_, _, s, false) if s.color.alpha() < 0.1));
}

fn elevation_shadow_reach() {
    let light = ShadowLight::default();
    let reach = ElevationShadow::flat(8.0).reach(RECT, &light, Offset::Zero);
    assert!(near_rect(reach, Rect::new(-3.845_528, 6.317_073, 125.796_75, 85.146_34)), "{reach:?}");
    // The light's center moves the spot shadow the other way.
    let reach = ElevationShadow::flat(8.0).reach(RECT, &light, Offset::new(492.0, 0.0));
    assert!(near_rect(reach, Rect::new(-11.845_528, 6.317_073, 117.796_75, 85.146_34)), "{reach:?}");
    // Nowhere up: the bounds and a pixel.
    assert_eq!(ElevationShadow::flat(0.0).reach(RECT, &light, Offset::Zero), RECT.inflate(1.0));
    // As high as its highest corner; the ambient reach at most 150.
    let tilted = ElevationShadow { plane: [0.0, 0.0, 400.0], ..ElevationShadow::default() };
    let high = ElevationShadow { plane: [0.0, 4.0, 120.0], ..ElevationShadow::default() };
    assert_eq!(tilted.reach(RECT, &light, Offset::Zero), high.reach(RECT, &light, Offset::Zero));
    let above = ElevationShadow::flat(600.0).reach(RECT, &light, Offset::Zero);
    assert_eq!(above, RECT.inflate(151.0));
}

// ---- The rest of the provided methods ----------------------------------------------------------------------------

fn draw_image_mask_default() {
    let image = ImageBitmap::new(4, 4, ImageBitmapConfig::Alpha8, true, &ColorSpaces::Srgb);
    let (src, dst) = (Rect::new(0.0, 0.0, 4.0, 4.0), Rect::new(10.0, 10.0, 30.0, 30.0));
    let mut g = Recorder::default();
    g.draw_image_mask(&image, src, dst, &SolidColor(Color::Red), dst, 0.5, FilterQuality::High);
    let gradient = Brush::linear_gradient(&[Color::Red, Color::Blue], Offset::Zero, Offset::Infinite, TileMode::Clamp);
    let area = Rect::new(0.0, 0.0, 100.0, 100.0);
    g.draw_image_mask(&image, src, dst, &gradient, area, 1.0, DefaultFilterQuality);
    assert_eq!(
        g.calls,
        vec![
            Call::PushLayer(dst, CornerRadii::Zero, 0.5, LayerTransform::Identity),
            Call::FillRoundRect(dst, CornerRadii::Zero, Color::Red),
            Call::PushBlendMode(BlendMode::DstIn),
            Call::Image(src, dst, 1.0, FilterQuality::High),
            Call::PopBlendMode,
            Call::PopLayer,
            Call::PushLayer(dst, CornerRadii::Zero, 1.0, LayerTransform::Identity),
            Call::FillGradient(dst, CornerRadii::Zero, gradient, area),
            Call::PushBlendMode(BlendMode::DstIn),
            Call::Image(src, dst, 1.0, FilterQuality::Low),
            Call::PopBlendMode,
            Call::PopLayer,
        ]
    );
}

fn draw_text_defaults() {
    let text =
        kompoz_text::FakeBackend.text_draw("Hi", Offset::new(0.0, 10.0), Rc::new(TextRunStyle::new(12.0)), false);
    let brush = Brush::horizontal_gradient(&[Color::Red, Color::Blue], 0.0, f32::INFINITY, TileMode::Clamp);
    let area = Rect::new(0.0, 0.0, 100.0, 10.0);
    let middle = brush.color_at(Offset::new(50.0, 5.0), area);
    assert!(middle != Color::Red && middle != Color::Blue);
    let mut g = Bare::default();
    g.draw_text_brush(&text, &brush, area);
    g.draw_text_brush(&text, &SolidColor(Color::Green), area);
    g.draw_text_shadow(&text, Color::Black, 4.0);
    assert_eq!(
        g.calls,
        vec![
            Call::Text("Hi".into(), middle),
            Call::Text("Hi".into(), Color::Green),
            Call::Text("Hi".into(), Color::Black),
        ]
    );
}

fn bare_defaults() {
    let mut g = Bare::default();
    let effect = BlurEffect(4.0, 4.0, TileMode::Clamp);
    let t = LayerTransform { translation_x: 3.0, ..LayerTransform::Identity };
    g.push_effect_layer(RECT, &effect, 0.5);
    g.draw_backdrop(RECT, &effect, 1.0);
    g.push_blend_mode(BlendMode::Multiply);
    g.pop_blend_mode();
    g.push_anti_alias(false);
    g.pop_anti_alias();
    let brush = Brush::horizontal_gradient(&[Color::Red, Color::Blue], 0.0, f32::INFINITY, TileMode::Clamp);
    let area = Rect::new(0.0, 0.0, 240.0, 10.0);
    g.fill_gradient(RECT, CornerRadii::all(2.0), &brush, area);
    g.push_outline_layer(RECT, &corner_path(), 0.25, t);
    assert_eq!(
        g.calls,
        vec![
            Call::PushLayer(RECT, CornerRadii::Zero, 0.5, LayerTransform::Identity),
            Call::FillRoundRect(RECT, CornerRadii::all(2.0), brush.color_at(RECT.center(), area)),
            Call::PushLayer(RECT, CornerRadii::Zero, 0.25, t),
        ]
    );
    assert!(!g.quick_reject(RECT));
    assert_eq!(g.light(), ShadowLight::default());
    assert_eq!(g.light_center(), Offset::Zero);
    g.light.center = Offset::new(5.0, 6.0);
    assert_eq!(g.light_center(), Offset::new(5.0, 6.0));

    // The node hooks: nothing kept, everything shown.
    g.calls.clear();
    g.begin_node(7);
    g.end_node();
    g.key_layer(9, true);
    assert!(g.ended_node_shows());
    assert_eq!(g.record_node(7, Offset::new(1.0, 2.0), true), NodeRecording::Other);
    assert_eq!(g.layer_content(), LayerContent::Draw);
    assert!(g.calls.is_empty());
}

fn object_safe() {
    let (ld, d) = ltr();
    let mut rec = Recorder::default();
    {
        let g: &mut dyn GraphicsContext = &mut rec;
        g.push_translation(Offset::new(1.0, 2.0));
        g.fill_shape(RECT, &RectangleShape, ld, &d, Color::Red);
        g.draw_line(Offset::Zero, Offset::new(5.0, 5.0), 2.0, Color::Blue);
        g.draw_arc(RECT, 0.0, 90.0, 2.0, Color::Blue, true);
        g.push_transform(&Matrix::new());
        g.pop_transform();
        g.push_color_filter(&ColorFilter::tint(Color::Red, BlendMode::SrcIn));
        g.pop_color_filter();
        g.pop_translation();
    }
    assert_eq!(
        rec.calls,
        vec![
            Call::PushTranslation(Offset::new(1.0, 2.0)),
            Call::FillRoundRect(RECT, CornerRadii::Zero, Color::Red),
            Call::Line(Offset::Zero, Offset::new(5.0, 5.0), 2.0, Color::Blue),
            Call::Arc(RECT, 0.0, 90.0, 2.0, Color::Blue, true),
            Call::PushTransform(Matrix::new()),
            Call::PopTransform,
            Call::PushColorFilter(ColorFilter::tint(Color::Red, BlendMode::SrcIn)),
            Call::PopColorFilter,
            Call::PopTranslation,
        ]
    );
    let boxed: Box<dyn GraphicsContext> = Box::new(Bare::default());
    assert!(boxed.ended_node_shows());
}

tests!(
    corner_radii_helpers,
    values_and_defaults,
    fill_shape_by_outline,
    fill_shape_with_a_brush,
    rtl_shapes_mirror,
    stroke_shape_inside_the_bounds,
    push_shape_layer_by_outline,
    push_clip_shape_cases,
    draw_shadow_rings,
    shadow_path_reach_numbers,
    draw_shadow_path_drop,
    draw_shadow_path_inner,
    draw_shape_shadow_with_a_color,
    draw_shape_shadow_with_a_gradient,
    draw_elevation_shadow_ambient_and_spot,
    elevation_shadow_reach,
    draw_image_mask_default,
    draw_text_defaults,
    bare_defaults,
    object_safe,
);
