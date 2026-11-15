//! Drawing: DrawScope's calls and transforms, Compose's Canvas and Paint, the draw modifiers, graphics layers (the
//! modifier and recorded ones), blurs and shadows, painters and vectors; what reaches a RecordingCanvas, with the
//! C++ Kompoz's numbers.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::draw::vector;
use kompoz_ui::draw::*;
use kompoz_ui::graphics_context::{ClipOp, CornerRadii, GraphicsContext, Shadow, TextDraw};
use kompoz_ui::layout::{CompositingStrategy, GraphicsLayerScope, LayerOutsets, TransformOrigin};
use kompoz_ui::platform::{LocalDensity, LocalLayoutDirection, PlatformCallbacks};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

// ---- Helpers ---------------------------------------------------------------------------------------------------

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5 // (C++'s near)
}

fn near_by(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// Whether `r` is (x, y, w, h), within half a px (C++'s sameRect).
fn same_rect(r: Rect, x: f32, y: f32, w: f32, h: f32) -> bool {
    near(r.left, x) && near(r.top, y) && near(r.width(), w) && near(r.height(), h)
}

fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color::new(r, g, b, a)
}

const RED: Color = Color::new(1.0, 0.0, 0.0, 1.0);
const GREEN: Color = Color::new(0.0, 1.0, 0.0, 1.0);
const BLUE: Color = Color::new(0.0, 0.0, 1.0, 1.0);

/// A path's commands as text: "M0,0 L10,0 Q… K… C… Z", each end to a hundredth.
fn path_text(path: &Path) -> String {
    let num = |v: f32| format!("{}", (v * 100.0).round() / 100.0);
    let mut out = String::new();
    for c in path.commands() {
        if !out.is_empty() {
            out.push(' ');
        }
        let (letter, p) = match *c {
            PathCommand::Move(p) => ("M", p),
            PathCommand::Line(p) => ("L", p),
            PathCommand::Quadratic(_, p) => ("Q", p),
            PathCommand::Conic(_, p, _) => ("K", p),
            PathCommand::Cubic(_, _, p) => ("C", p),
            PathCommand::Close => {
                out.push('Z');
                continue;
            }
        };
        out += &format!("{letter}{},{}", num(p.x), num(p.y));
    }
    out
}

/// How a matrix places a point (C++'s placedOf): its scales, its turn in degrees, its move, and the point it turns
/// about (where it has one).
struct Placed {
    turn: f32,
    pivot: Option<Offset>,
}

fn placed_of(m: &Matrix) -> Placed {
    let (a, b, c, d, tx, ty) = (m.get(0, 0), m.get(0, 1), m.get(1, 0), m.get(1, 1), m.get(3, 0), m.get(3, 1));
    let det = (1.0 - a) * (1.0 - d) - c * b;
    let pivot =
        (det.abs() > 1e-6).then(|| Offset::new(((1.0 - d) * tx + c * ty) / det, (b * tx + (1.0 - a) * ty) / det));
    Placed { turn: b.atan2(a).to_degrees(), pivot }
}

/// A canvas drawn into by `draw` in a 100 x 50 Canvas composable at the origin.
fn canvas_of(width: f32, height: f32, draw: impl Fn(&mut DrawScope) + 'static) -> RecordingCanvas {
    let draw = Rc::new(draw);
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let draw = draw.clone();
        Canvas(Modifier.size_wh(Dp(width), Dp(height)), on_draw = move |d| draw(d));
    }));
    rule.draw()
}

// ---- DrawScope ---------------------------------------------------------------------------------------------------

fn canvas_and_draw_modifiers_draw_in_local_coordinates() {
    // C++ 7588.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(modifier = Modifier.padding(10.dp()), || {
            Canvas(
                Modifier.size_wh(100.dp(), 50.dp()),
                on_draw = |d| {
                    d.draw_rect(Color::Black); // the whole area
                    d.draw_circle(Color::White, radius = 10.0, center = Offset::new(20.0, 25.0));
                },
            );
            Box(modifier = Modifier
                .size(40.dp())
                .draw_with_content(|d| {
                    d.draw_content();
                    d.draw_rect(Color::Black, top_left = Offset::new(5.0, 5.0), size = Size::new(10.0, 10.0));
                })
                .background(Color::White));
        });
    }));
    let c = rule.draw();
    assert_eq!(c.rects.len(), 4);
    assert!(same_rect(c.rects[0], 10.0, 10.0, 100.0, 50.0));
    assert!(same_rect(c.rects[1], 20.0, 25.0, 20.0, 20.0));
    assert!(same_rect(c.rects[2], 10.0, 60.0, 40.0, 40.0)); // the content (background) first
    assert!(same_rect(c.rects[3], 15.0, 65.0, 10.0, 10.0));
}

fn canvas_takes_compose_named_arguments() {
    // C++ 7624: Canvas(modifier, contentDescription) { }: the description is what a screen reader says.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Canvas(
            Modifier.size(30.dp()).test_tag("tuval"),
            content_description = Some("bir daire".into()),
            on_draw = |d| d.draw_circle(Color::Black),
        );
    }));
    assert!(rule.on_node_with_tag("tuval").exists() && rule.on_node_with_content_description("bir daire").exists());
    assert_eq!(rule.draw().rects.len(), 1);
}

fn drawing_takes_compose_named_arguments() {
    // C++ 7636: DrawScope's calls with Compose's names draw what the plain forms draw.
    let mut wave = Path::new();
    wave.move_to(0.0, 20.0);
    wave.cubic_to(30.0, -10.0, 60.0, 50.0, 90.0, 20.0);
    let w = wave.clone();
    let named = KompozTestRule::new(kompozable_lambda!(move || {
        let wave = w.clone();
        Canvas(
            Modifier.size_wh(100.dp(), 50.dp()),
            on_draw = move |d| {
                d.draw_circle(
                    Color::White,
                    radius = 9.0,
                    center = Offset::new(20.0, 25.0),
                    style = Stroke(width = 2.0).into(),
                );
                d.draw_rect(
                    Color::White,
                    top_left = Offset::new(50.0, 10.0),
                    size = Size::new(20.0, 20.0),
                    alpha = 0.5,
                );
                d.draw_line(
                    Color::White,
                    start = Offset::Zero,
                    end = Offset::new(10.0, 10.0),
                    stroke_width = 3.0,
                    cap = StrokeCap::Round,
                    path_effect = Some(PathEffect::dash_path_effect(&[8.0, 6.0], 0.0)),
                );
                d.draw_arc(
                    Color::White,
                    0.0,
                    90.0,
                    true,
                    top_left = Offset::new(80.0, 10.0),
                    size = Size::new(20.0, 20.0),
                );
                d.draw_path(
                    &wave,
                    Color::White,
                    style = Stroke(width = 3.0, cap = StrokeCap::Round, join = StrokeJoin::Round).into(),
                );
                d.rotate(
                    30.0,
                    pivot = Offset::new(50.0, 25.0),
                    block = |r| r.draw_rect(Color::Black, size = Size::new(10.0, 10.0)),
                );
                d.inset_hv(
                    horizontal = 4.0,
                    vertical = 2.0,
                    block = |r| r.draw_oval(Color::Black, size = Size::new(10.0, 10.0)),
                );
                d.clip_rect(
                    left = 10.0,
                    top = 10.0,
                    right = 40.0,
                    bottom = 40.0,
                    block = |r| r.draw_rect(Color::Black),
                );
            },
        );
    }));
    let a = named.draw();
    let b = canvas_of(100.0, 50.0, move |d| {
        let stroke = |w: f32| DrawStyle::Stroke(Stroke::new(w, 4.0, StrokeCap::Butt, StrokeJoin::Miter, None));
        d.draw_circle_with(Color::White, 9.0, Offset::new(20.0, 25.0), 1.0, stroke(2.0), None, DefaultBlendMode);
        d.draw_rect_with(
            Color::White,
            Offset::new(50.0, 10.0),
            Size::new(20.0, 20.0),
            0.5,
            Fill,
            None,
            DefaultBlendMode,
        );
        let dash = PathEffect::dash_path_effect(&[8.0, 6.0], 0.0);
        d.draw_line_with(
            Color::White,
            Offset::Zero,
            Offset::new(10.0, 10.0),
            3.0,
            StrokeCap::Round,
            Some(dash),
            1.0,
            None,
            DefaultBlendMode,
        );
        d.draw_arc_with(
            Color::White,
            0.0,
            90.0,
            true,
            Offset::new(80.0, 10.0),
            Size::new(20.0, 20.0),
            1.0,
            Fill,
            None,
            DefaultBlendMode,
        );
        let round = DrawStyle::Stroke(Stroke::new(3.0, 4.0, StrokeCap::Round, StrokeJoin::Round, None));
        d.draw_path_with(&wave, Color::White, 1.0, round, None, DefaultBlendMode);
        d.rotate_with(30.0, Offset::new(50.0, 25.0), |r| {
            r.draw_rect_with(Color::Black, Offset::Zero, Size::new(10.0, 10.0), 1.0, Fill, None, DefaultBlendMode)
        });
        d.inset(4.0, 2.0, 4.0, 2.0, |r| {
            r.draw_oval_with(Color::Black, Offset::Zero, Size::new(10.0, 10.0), 1.0, Fill, None, DefaultBlendMode)
        });
        d.clip_rect_with(10.0, 10.0, 40.0, 40.0, ClipOp::Intersect, |r| r.draw_rect(Color::Black));
    });
    assert!(a.rects.len() >= 4 && a.rects == b.rects && a.rect_radii == b.rect_radii);
    assert!(a.paths == b.paths && a.order == b.order && a.stack == b.stack && a.clips == b.clips);
    assert!(a.transforms.len() == b.transforms.len() && a.translations == b.translations);
    let by_name = kompozable_block!({ Stroke(width = 2.0) });
    assert_eq!(by_name, Stroke::new(2.0, 4.0, StrokeCap::Butt, StrokeJoin::Miter, None));
}

fn draw_scope_strokes_sit_on_the_outline() {
    // C++ 8398: as Compose's, half of the stroke each side of the rect or circle.
    let c = canvas_of(100.0, 50.0, |d| {
        let stroke = |w: f32| DrawStyle::Stroke(Stroke::new(w, 4.0, StrokeCap::Butt, StrokeJoin::Miter, None));
        d.draw_circle_with(Color::White, 9.0, Offset::new(20.0, 25.0), 1.0, stroke(2.0), None, DefaultBlendMode);
        d.draw_rect_with(
            Color::White,
            Offset::new(50.0, 10.0),
            Size::new(20.0, 20.0),
            1.0,
            stroke(4.0),
            None,
            DefaultBlendMode,
        );
        let corner = CornerRadius::circular(3.0);
        let at = Offset::new(80.0, 10.0);
        d.draw_round_rect_with(
            Color::White,
            at,
            Size::new(10.0, 20.0),
            corner,
            1.0,
            stroke(2.0),
            None,
            DefaultBlendMode,
        );
    });
    assert_eq!(c.strokes.len(), 3);
    assert!(same_rect(c.strokes[0], 10.0, 15.0, 20.0, 20.0) && near(c.stroke_corners[0].top_left, 10.0));
    assert!(same_rect(c.strokes[1], 48.0, 8.0, 24.0, 24.0) && near(c.stroke_corners[1].top_left, 0.0));
    assert!(same_rect(c.strokes[2], 79.0, 9.0, 12.0, 22.0) && near(c.stroke_corners[2].top_left, 4.0));
}

fn draw_scope_rotation_and_text() {
    // C++ 8417: turned through the context's transform (no layer), about the middle; a text drawn from its
    // baseline in the scope.
    let c = canvas_of(100.0, 100.0, |d| {
        d.rotate(30.0, |r| {
            r.draw_rect_with(Color::Black, Offset::Zero, Size::new(10.0, 10.0), 1.0, Fill, None, DefaultBlendMode)
        });
        d.draw_text(&TextDraw::new("çizim", Offset::new(0.0, 11.2), 14.0, Size::new(35.0, 17.5)), Color::Black);
    });
    assert!(c.layers == 0 && c.transforms.len() == 1 && c.transform_depth == 0);
    let middle = c.transforms[0].map(Offset::new(50.0, 50.0));
    let right = c.transforms[0].map(Offset::new(60.0, 50.0));
    assert!(near_by(middle.x, 50.0, 0.01) && near_by(middle.y, 50.0, 0.01));
    assert!(near_by(right.x, 50.0 + 10.0 * (std::f32::consts::PI / 6.0).cos(), 0.01) && near_by(right.y, 55.0, 0.01));
    assert_eq!(c.texts, ["çizim"]);
}

const P: DrawStyle = Fill;
const SRC: BlendMode = BlendMode::SrcOver;

fn stroke(width: f32) -> DrawStyle {
    DrawStyle::Stroke(Stroke::new(width, 4.0, StrokeCap::Butt, StrokeJoin::Miter, None))
}

fn stroke_of(width: f32, cap: StrokeCap, join: StrokeJoin, effect: Option<PathEffect>) -> DrawStyle {
    DrawStyle::Stroke(Stroke::new(width, 4.0, cap, join, effect))
}

fn vgrad(colors: &[Color]) -> Brush {
    Brush::vertical_gradient(colors, 0.0, f32::INFINITY, TileMode::Clamp)
}

fn hgrad(colors: &[Color]) -> Brush {
    Brush::horizontal_gradient(colors, 0.0, f32::INFINITY, TileMode::Clamp)
}

fn lgrad(colors: &[Color]) -> Brush {
    Brush::linear_gradient(colors, Offset::Zero, Offset::Infinite, TileMode::Clamp)
}

fn tint(c: Color) -> ColorFilter {
    ColorFilter::tint(c, BlendMode::SrcIn)
}

fn sz(w: f32, h: f32) -> Size {
    Size::new(w, h)
}

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

trait CopyAlpha {
    fn copy_alpha(self, alpha: f32) -> Color;
}

impl CopyAlpha for Color {
    fn copy_alpha(self, alpha: f32) -> Color {
        Color::new_in(self.red(), self.green(), self.blue(), alpha, self.color_space())
    }
}

fn paths_take_arcs_and_draw_scopes_clip_to_paths() {
    // C++ 13090's drawing (the paths' own half is kompoz-graphics'): an oval; clipped to a triangle, inside it or
    // outside (the context's clip, no layer); transforms in order.
    let mut triangle = Path::new();
    triangle.move_to(50.0, 20.0);
    triangle.line_to(80.0, 80.0);
    triangle.line_to(20.0, 80.0);
    triangle.close();
    let c = canvas_of(100.0, 100.0, move |d| {
        d.draw_oval_with(RED, at(10.0, 10.0), sz(80.0, 40.0), 1.0, P, None, SRC);
        d.clip_path_with(&triangle, ClipOp::Intersect, |i| i.draw_rect(BLUE));
        d.clip_path_with(&triangle, ClipOp::Difference, |i| i.draw_rect(GREEN));
        d.with_transform(
            |t| {
                t.translate_with(10.0, 20.0);
                let c = t.center();
                t.rotate_with(45.0, c);
            },
            |i| i.draw_rect_with(Color::Black, Offset::Zero, sz(10.0, 10.0), 1.0, P, None, SRC),
        );
        d.with_transform(
            |t| {
                t.inset(10.0, 10.0, 10.0, 10.0); // the clip inside the inset
                t.clip_rect_with(0.0, 0.0, 20.0, 20.0, ClipOp::Intersect);
            },
            |i| i.draw_rect(Color::Black),
        );
    });
    assert!(c.paths.len() == 1 && same_rect(c.paths[0], 10.0, 10.0, 80.0, 40.0));
    assert_eq!(c.clip_shapes.len(), 2);
    let (inside, outside) = (&c.clip_shapes[0], &c.clip_shapes[1]);
    assert!(
        inside.path.is_some() && inside.op == ClipOp::Intersect && same_rect(inside.bounds, 20.0, 20.0, 60.0, 60.0)
    );
    assert!(outside.path.is_some() && outside.op == ClipOp::Difference);
    assert!(c.rects.len() == 4 && c.layers == 0); // the two clipped fills, the turned one, the inset one
    assert!(c.translations.len() == 1 && near(c.translations[0].x, 10.0) && near(c.translations[0].y, 20.0));
    assert_eq!(c.transforms.len(), 1);
    // Turned 45 degrees about the middle, inside the translation.
    let right = c.transforms[0].map(at(60.0, 50.0));
    let k = 10.0 / std::f32::consts::SQRT_2;
    assert!(near_by(right.x, 50.0 + k, 0.01) && near_by(right.y, 50.0 + k, 0.01));
    assert!(same_rect(*c.clips.last().unwrap(), 10.0, 10.0, 20.0, 20.0));
}

fn translations_move_drawn_content() {
    // C++ 27242 (its display list half is the display list's): the content moves with the scope's translation.
    let by = mutable_state_of(10.0f32);
    let b = by.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let b = b.clone();
        Box(
            modifier = Modifier.padding(20.dp()).draw_with_content(move |d| {
                let x = b.get();
                d.translate_with(x, 5.0, |moved| {
                    moved.draw_content(); // the content moves too
                    moved.draw_rect_with(GREEN, Offset::Zero, sz(4.0, 4.0), 1.0, P, None, SRC);
                });
            }),
            content = || Box(modifier = Modifier.size(30.dp()).background(RED)),
        );
    }));
    let c = rule.draw();
    let red: Vec<Rect> = c.rects.iter().zip(&c.rect_colors).filter(|(_, c)| **c == RED).map(|(r, _)| *r).collect();
    assert!(red.len() == 1 && near(red[0].left, 30.0) && near(red[0].top, 25.0));
    assert!(c.rects.len() == 2 && near(c.rects[1].left, 30.0) && near(c.rects[1].top, 25.0)); // moved once
    by.set(50.0);
    rule.wait_for_idle();
    assert!(near(rule.draw().rects[0].left, 70.0));
}

fn canvases_draw_as_composes() {
    // C++ 27599.
    let fill = Paint { color: RED, ..Paint::default() };
    let mut half = fill.clone();
    half.set_alpha(0.5);
    let outline = Paint {
        color: BLUE,
        style: PaintingStyle::Stroke,
        stroke_width: 3.0,
        stroke_cap: StrokeCap::Round,
        stroke_join: StrokeJoin::Bevel,
        stroke_miter_limit: 7.0,
        ..Paint::default()
    };
    let shaded = Paint {
        shader: Some(LinearGradientShader(Offset::Zero, at(100.0, 0.0), &[RED, BLUE], None, TileMode::Clamp)),
        ..Paint::default()
    };
    let img = ImageBitmap::from_pixels(4, 2, vec![255; 32]).unwrap();
    let (f, h, o, s, im) = (fill.clone(), half.clone(), outline.clone(), shaded.clone(), img.clone());
    let draw = move |d: &mut DrawScope| {
        d.draw_into_canvas(|c| {
            c.draw_rect(Rect::new(1.0, 2.0, 11.0, 12.0), &f); // r0: from the canvas's origin
            c.draw_rect_ltrb(30.0, 30.0, 10.0, 20.0, &h); // r1: sorted; the paint's alpha
            c.draw_rect(Rect::new(40.0, 0.0, 60.0, 10.0), &s); // r2: a shader in the canvas's coordinates
            c.draw_rect(Rect::new(0.0, 40.0, 10.0, 50.0), &o); // p0: stroked as the paint says
            c.draw_line(at(0.0, 60.0), at(30.0, 60.0), &o); // p1: a line, round capped
            let square = Paint { color: RED, stroke_width: 2.0, stroke_cap: StrokeCap::Square, ..Paint::default() };
            c.draw_line(at(0.0, 70.0), at(30.0, 70.0), &square); // a line (filled paint: stroked)
            c.draw_round_rect(0.0, 80.0, 20.0, 100.0, 4.0, 6.0, &f); // p2: elliptical corners
            c.draw_oval_ltrb(0.0, 100.0, 30.0, 110.0, &f); // p3
            c.draw_circle(at(50.0, 50.0), 5.0, &f); // r3: a circle
            c.draw_arc(Rect::new(0.0, 120.0, 20.0, 140.0), 0.0, 90.0, true, &f); // p4: a pie slice
            let pi = std::f32::consts::PI;
            c.draw_arc_rad(Rect::new(0.0, 120.0, 20.0, 140.0), pi / 2.0, pi / 2.0, false, &o); // the backend's
            let mut p = Path::new();
            p.add_rect(Rect::new(100.0, 0.0, 110.0, 10.0), PathDirection::CounterClockwise);
            c.draw_path(&p, &f); // p5
            let faint = Paint { color: RED.copy_alpha(0.25), filter_quality: FilterQuality::None, ..Paint::default() };
            c.draw_image(&im, at(60.0, 60.0), &faint);
            let (one, two) = (IntOffset::new(1, 0), IntSize::new(2, 2));
            c.draw_image_rect_with(&im, one, two, IntOffset::new(70, 70), IntSize::new(8, 4), &Paint());
            c.draw_image_rect_with(
                &im,
                IntOffset::Zero,
                IntSize::new(4, 2),
                IntOffset::Zero,
                IntSize::new(4, 2),
                &Paint(),
            );
            let dots = Paint { color: RED, stroke_width: 2.0, ..Paint::default() };
            c.draw_points(PointMode::Points, &[at(150.0, 150.0), at(160.0, 150.0)], &dots); // p6
            c.draw_raw_points(PointMode::Points, &[150.0, 160.0, 170.0, 160.0, 999.0], &dots); // p7 (odd last out)
            c.enable_z();
            c.disable_z();
        });
    };
    let r = padded_canvas(draw);
    assert!(r.rects.len() == 4 && r.paths.len() == 8 && r.lines.len() == 1 && r.images.len() == 3 && r.arcs.len() == 1);
    assert!(same_rect(r.rects[0], 21.0, 12.0, 10.0, 10.0) && r.rect_colors[0] == RED);
    assert!(same_rect(r.rects[1], 30.0, 30.0, 20.0, 10.0) && near(r.rect_colors[1].alpha(), 0.5));
    // At canvas x 50: halfway.
    assert!(
        r.gradients == 1 && near_by(r.rect_colors[2].red(), 0.5, 0.02) && near_by(r.rect_colors[2].blue(), 0.5, 0.02)
    );
    assert!(r.path_strokes[0] == 3.0 && r.path_stroke_styles[0].join == StrokeJoin::Bevel);
    assert_eq!(r.path_stroke_styles[0].miter, 7.0);
    assert!(r.path_stroke_styles[1].cap == StrokeCap::Round && r.path_strokes[1] == 3.0);
    assert!(near(r.lines[0].0.y, 80.0) && near(r.lines[0].0.x, 20.0));
    assert!(path_text(&r.path_shapes[2]).contains('K') && same_rect(r.paths[2], 20.0, 90.0, 20.0, 20.0));
    assert!(same_rect(r.paths[3], 20.0, 110.0, 30.0, 10.0));
    assert!(same_rect(r.rects[3], 65.0, 55.0, 10.0, 10.0) && near(r.rect_radii[3], 5.0));
    assert_eq!(path_text(&r.path_shapes[4]), "M10,130 L20,130 K10,140 Z"); // (from the origin it is drawn at)
    assert!(near(r.arcs[0].start, 90.0) && near(r.arcs[0].sweep, 90.0) && near(r.arcs[0].stroke_width, 3.0));
    assert!(same_rect(r.paths[5], 120.0, 10.0, 10.0, 10.0));
    assert!(same_rect(r.image_destinations[0], 80.0, 70.0, 4.0, 2.0) && r.image_qualities[0] == FilterQuality::None);
    assert!(near(r.image_alphas[0], 0.25));
    assert!(
        same_rect(r.image_sources[1], 1.0, 0.0, 2.0, 2.0) && same_rect(r.image_destinations[1], 90.0, 80.0, 8.0, 4.0)
    );
    assert!(same_rect(r.image_destinations[2], 20.0, 10.0, 4.0, 2.0) && r.image_qualities[2] == DefaultFilterQuality);
    assert!(near(r.paths[6].width(), 12.0) && near(r.paths[7].width(), 22.0) && near(r.paths[7].height(), 2.0));

    // The stack: saves, transforms before what is there, clips (sorted), layers, restores.
    let mut s = padded_canvas(|d| {
        d.draw_into_canvas(|c| {
            let fill = Paint { color: RED, ..Paint::default() };
            c.translate(1.0, 1.0); // (not saved)
            c.restore(); // nothing saved: nothing
            c.save();
            c.translate(10.0, 0.0);
            c.rotate(90.0);
            c.clip_rect_ltrb(40.0, 40.0, 20.0, 20.0, ClipOp::Difference);
            c.draw_rect(Rect::new(0.0, 0.0, 1.0, 1.0), &fill);
            c.restore();
            let layer = Paint {
                color: RED.copy_alpha(0.5),
                blend_mode: BlendMode::Multiply,
                color_filter: Some(tint(BLUE)),
                ..Paint::default()
            };
            c.save_layer(Rect::new(50.0, 50.0, 30.0, 40.0), &layer);
            let mut oval = Path::new();
            oval.add_oval(Rect::new(0.0, 0.0, 10.0, 10.0), PathDirection::CounterClockwise);
            c.clip_path(&oval, ClipOp::Intersect);
            c.restore();
            c.with_save(|c| c.skew(0.5, 0.0));
            c.with_save_layer(Rect::new(0.0, 0.0, 10.0, 10.0), &Paint(), |_| {});
            c.scale_about(2.0, 3.0, 5.0, 5.0); // (not restored: held until the scope ends)
        });
        d.draw_rect_with(RED, Offset::Zero, sz(1.0, 1.0), 1.0, P, None, SRC);
    });
    assert!(s.stack.contains("TTTCcttF")); // the first translate kept by both restores
    assert!(s.stack.contains("FBLCclbf"));
    assert!(s.stack.contains("TtLl"));
    assert!(s.stack.contains("TTTtttt")); // the pivoted scale's three and the first, popped as the scope ends
    assert!(s.transforms.len() == 7 && s.layer_bounds.len() == 2 && s.clip_shapes.len() == 2);
    s.transforms.remove(0); // (the first: not saved)
    let at_ = |p: Offset, x: f32, y: f32| near(p.x, x) && near(p.y, y);
    // In the node's coordinates (where the context is as it draws: its coordinator moved it to (20, 10), which the
    // recording doesn't apply to matrices), what a canvas point goes to, the canvas's origin there (0, 0).
    assert!(at_(s.transforms[0].map(at(5.0, 5.0)), 15.0, 5.0)); // translate
    assert!(at_(s.transforms[1].map(at(10.0, 0.0)), 0.0, 10.0)); // a quarter turn clockwise about the origin
    assert!(same_rect(s.clip_shapes[0].bounds, 40.0, 30.0, 20.0, 20.0) && s.clip_shapes[0].op == ClipOp::Difference);
    assert!(same_rect(s.layer_bounds[0], 50.0, 50.0, 20.0, 10.0) && near(s.layer_alphas[0], 0.5));
    assert!(same_rect(s.clip_shapes[1].bounds, 20.0, 10.0, 10.0, 10.0));
    assert!(at_(s.transforms[2].map(at(0.0, 20.0)), 10.0, 20.0)); // skew: x + 0.5 y
    assert!(at_(s.transforms[3].map(at(5.0, 5.0)), 10.0, 10.0) && at_(s.transforms[4].map(at(1.0, 1.0)), 2.0, 3.0));
    // concat: the matrix's z row and column left out (Android's 3x3); a skew_rad of degrees(radians).
    let mut z = RecordingCanvas::new();
    let mut m = Matrix::new();
    m.rotate_x(30.0);
    m.translate(4.0, 5.0, 0.0);
    {
        let mut c = Canvas::new(&mut z, Offset::Zero);
        c.concat(&m);
        c.skew_rad(0.01, 0.0);
        c.save();
        c.clip_rect(Rect::new(0.0, 0.0, 5.0, 5.0), ClipOp::Intersect);
    } // (gone: what it left restored)
    assert_eq!(z.transforms.len(), 2);
    let got = z.transforms[0];
    assert!(got.get(2, 0) == 0.0 && got.get(2, 1) == 0.0 && got.get(0, 2) == 0.0 && got.get(1, 2) == 0.0);
    assert!(got.get(2, 2) == 1.0 && got.get(3, 2) == 0.0);
    assert!(near(got.get(0, 0), m.get(0, 0)) && near(got.get(1, 1), m.get(1, 1)) && near(got.get(3, 1), m.get(3, 1)));
    assert!(near_by(z.transforms[1].get(1, 0), 0.01f32.to_degrees(), 1e-4));
    assert!(z.stack.ends_with("Cctt"));
    // A block of the scope restores what the canvas left inside it, before its own; the scope's inset keeps its
    // density.
    let inset_density = Rc::new(Cell::new(Density::default()));
    let seen = inset_density.clone();
    let blocks = KompozTestRule::new(kompozable_lambda!(move || {
        let seen = seen.clone();
        Canvas(
            Modifier.size(100.dp()),
            on_draw = move |d| {
                d.translate_with(5.0, 0.0, |i| i.draw_into_canvas(|c| c.translate(1.0, 0.0)));
                d.clip_rect_with(0.0, 0.0, -10.0, -10.0, ClipOp::Intersect, |i| {
                    i.draw_into_canvas(|c| {
                        c.save();
                        c.rotate(5.0);
                    })
                });
                d.inset_all(10.0, |i| seen.set(i.density()));
            },
        );
    }));
    blocks.host().set_density(Density::new(2.0, 1.0));
    blocks.wait_for_idle();
    let b = blocks.draw();
    assert!(b.stack.contains("MTtmCTtc") && inset_density.get() == Density::new(2.0, 1.0));
    assert!(b.clips.len() == 1 && same_rect(b.clips[0], -10.0, -10.0, 10.0, 10.0)); // sorted
    // A scope over a canvas: from its origin, what its draws leave on it left there.
    let mut o = RecordingCanvas::new();
    {
        let mut c = Canvas::new(&mut o, at(100.0, 50.0));
        {
            let mut over = DrawScope::over_canvas(&mut c, sz(20.0, 20.0), LayoutDirection::Ltr, Density::default());
            over.draw_rect(RED);
            over.draw_into_canvas(|k| k.translate(1.0, 1.0));
        }
        c.draw_rect(Rect::new(0.0, 0.0, 5.0, 5.0), &half);
    }
    assert!(o.rects.len() == 2 && same_rect(o.rects[0], 100.0, 50.0, 20.0, 20.0));
    assert!(same_rect(o.rects[1], 100.0, 50.0, 5.0, 5.0) && o.stack == "Tt"); // (its translate there still)
    // Fills with a path effect: rounded where it rounds, never dashed (Skia dashes only strokes).
    let mut e = RecordingCanvas::new();
    {
        let mut c = Canvas::new(&mut e, Offset::Zero);
        let r = Rect::new(0.0, 0.0, 20.0, 20.0);
        c.draw_rect(r, &Paint { path_effect: Some(PathEffect::corner_path_effect(4.0)), ..Paint::default() });
        c.draw_rect(
            r,
            &Paint { path_effect: Some(PathEffect::dash_path_effect(&[2.0, 2.0], 0.0)), ..Paint::default() },
        );
        let dashed = Paint {
            style: PaintingStyle::Stroke,
            stroke_width: 1.0,
            path_effect: Some(PathEffect::dash_path_effect(&[2.0, 2.0], 0.0)),
            ..Paint::default()
        };
        c.draw_rect(r, &dashed);
    }
    assert!(e.path_shapes.len() == 3 && e.rects.is_empty());
    assert!(path_text(&e.path_shapes[0]).contains('Q'));
    assert!(path_text(&e.path_shapes[1]) == "M0,0 L20,0 L20,20 L0,20 Z" && e.path_strokes[1] == 0.0);
    assert!(e.path_shapes[2].commands().len() > 10);
    // A paint's alpha is its color's; equal by value.
    let mut p = Paint();
    p.set_alpha(2.0);
    assert!(p.alpha() == 1.0 && p.color == Color::Black && p.is_anti_alias && p.filter_quality == DefaultFilterQuality);
    assert_eq!(p.stroke_miter_limit, 4.0);
    p.set_alpha(-1.0);
    assert!(p.alpha() == 0.0 && p != Paint() && Paint() == Paint());
    // DrawScope's shapes are sorted too, as Skia sorts them.
    let k = canvas_of(100.0, 100.0, |d| d.draw_rect_with(RED, at(30.0, 30.0), sz(-10.0, -20.0), 1.0, P, None, SRC));
    assert!(k.rects.len() == 1 && same_rect(k.rects[0], 20.0, 10.0, 10.0, 20.0));
}

/// A 200 px Canvas 20 px from the left, 10 down, drawn by `draw`.
fn padded_canvas(draw: impl Fn(&mut DrawScope) + 'static) -> RecordingCanvas {
    let draw = Rc::new(draw);
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let draw = draw.clone();
        let modifier = Modifier.padding_sides_with(20.dp(), 10.dp(), 0.dp(), 0.dp()).size(200.dp());
        Canvas(modifier, on_draw = move |d| draw(d));
    }));
    rule.draw()
}

type Seen = Rc<Cell<Option<(Density, LayoutDirection)>>>;

fn see(s: &Seen, d: &DrawScope) {
    s.set(Some((d.density(), d.layout_direction())));
}

fn draw_scopes_know_the_density_and_direction() {
    // C++ 27776: every draw scope over a node has the screen's density and the node's layout direction:
    // draw_behind, draw_with_content, Canvas, draw_with_cache's (built again when either changes), a recording's, a
    // painter's of one's own.
    let seen: Vec<Seen> = (0..7).map(|_| Rc::new(Cell::new(None))).collect();
    let builds = Rc::new(Cell::new(0));
    let (s3, s4, b) = (seen[3].clone(), seen[4].clone(), builds.clone());
    // (Made once, outside the composition: its element, and its cache, kept when it recomposes.)
    let cached = Modifier.size(10.dp()).draw_with_cache(move |c| {
        b.set(b.get() + 1);
        s4.set(Some((c.density(), c.layout_direction())));
        let s3 = s3.clone();
        c.on_draw_behind(move |d| see(&s3, d))
    });
    let s6 = seen[6].clone();
    let own: Rc<dyn Painter> = Rc::new(DrawPainter::new(sz(10.0, 10.0), move |d| see(&s6, d)));
    let rtl = mutable_state_of(true);
    let (r, sn) = (rtl.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        let (sn, cached, own) = (sn.clone(), cached.clone(), own.clone());
        KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], move || {
            let layer = remember_graphics_layer();
            let (s0, s1, s2, s5) = (sn[0].clone(), sn[1].clone(), sn[2].clone(), sn[5].clone());
            Column(|| {
                Box(modifier = Modifier.size(10.dp()).draw_behind(move |d| see(&s0, d)));
                Box(modifier = Modifier.size(10.dp()).draw_with_content(move |d| {
                    see(&s1, d);
                    let s5 = s5.clone();
                    d.record_with(&layer, IntSize::new(10, 10), move |r| see(&s5, r));
                }));
                Canvas(Modifier.size(10.dp()), on_draw = move |d| see(&s2, d));
                Box(modifier = cached.clone());
                Image(own.clone(), None, modifier = Modifier.size(10.dp()));
            });
        });
    }));
    rule.host().set_density(Density::new(2.5, 1.25));
    rule.wait_for_idle();
    rule.draw();
    let want = Some((Density::new(2.5, 1.25), LayoutDirection::Rtl));
    for (i, s) in seen.iter().enumerate() {
        assert_eq!(s.get(), want, "scope {i}");
    }
    let before = builds.get();
    rule.draw();
    assert_eq!(builds.get(), before); // the same: kept
    rtl.set(false);
    rule.wait_for_idle();
    rule.draw();
    assert!(builds.get() == before + 1 && seen[4].get().unwrap().1 == LayoutDirection::Ltr);
    assert_eq!(seen[6].get().unwrap().1, LayoutDirection::Ltr);
    rule.host().set_density(Density::new(3.0, 1.0));
    rule.wait_for_idle();
    rule.draw();
    assert!(builds.get() == before + 2 && seen[4].get().unwrap().0 == Density::new(3.0, 1.0));
}

fn paints_turn_anti_aliasing_off() {
    // C++ 27832 (its display list half is the display list's): a paint not anti-aliased draws inside
    // push_anti_alias(false), every kind of draw; a DrawScope's draws stay anti-aliased.
    let sharp = Paint { color: RED, is_anti_alias: false, ..Paint::default() };
    let mut r = RecordingCanvas::new();
    {
        let mut c = Canvas::new(&mut r, Offset::Zero);
        c.draw_rect(Rect::new(0.0, 0.0, 10.0, 10.0), &sharp);
        c.draw_line(Offset::Zero, at(10.0, 10.0), &sharp);
        c.draw_round_rect(0.0, 0.0, 10.0, 10.0, 2.0, 3.0, &sharp);
        c.draw_oval(Rect::new(0.0, 0.0, 10.0, 5.0), &sharp);
        c.draw_circle(at(5.0, 5.0), 5.0, &sharp);
        c.draw_arc(Rect::new(0.0, 0.0, 10.0, 10.0), 0.0, 90.0, true, &sharp);
        let mut p = Path::new();
        p.add_rect(Rect::new(0.0, 0.0, 5.0, 5.0), PathDirection::CounterClockwise);
        c.draw_path(&p, &sharp);
        let image = ImageBitmap::new(2, 2, ImageBitmapConfig::Argb8888, true, &colorspace::ColorSpaces::Srgb);
        c.draw_image(&image, Offset::Zero, &sharp);
        c.draw_points(PointMode::Points, &[at(1.0, 1.0)], &sharp);
        c.draw_rect(Rect::new(0.0, 0.0, 10.0, 10.0), &Paint()); // anti-aliased: nothing pushed
    }
    assert_eq!(r.stack, "AaAaAaAaAaAaAaAaAa");
    assert!(r.path_anti_aliased.len() >= 5 && !r.path_anti_aliased[0] && r.anti_alias_stack.is_empty());
    let scope = canvas_of(10.0, 10.0, |d| {
        let mut p = Path::new();
        p.add_rect(Rect::new(0.0, 0.0, 5.0, 5.0), PathDirection::CounterClockwise);
        d.draw_path(&p, Color::Black);
    });
    assert!(!scope.stack.contains('A') && scope.path_anti_aliased == [true]);
}

fn vertices_are_drawn_by_canvases() {
    // C++ 27867's canvas half (Vertices themselves are kompoz-graphics', their display list half the display list's):
    // at the canvas's origin, with the paint's shader (or its color, opaque), alpha, filter and blend mode.
    let mesh = Vertices::new(
        VertexMode::Triangles,
        &[Offset::Zero, at(20.0, 0.0), at(0.0, 20.0)],
        &[Offset::Zero, at(5.0, 0.0), at(0.0, 5.0)],
        &[RED, BLUE, RED],
        &[],
    );
    let mut r = RecordingCanvas::new();
    {
        let mut c = Canvas::new(&mut r, at(30.0, 40.0));
        let faded = Paint {
            color: RED.copy_alpha(0.5),
            blend_mode: BlendMode::Multiply,
            color_filter: Some(tint(BLUE)),
            ..Paint::default()
        };
        c.draw_vertices(&mesh, BlendMode::Screen, &faded);
        let shaded = Paint {
            shader: Some(LinearGradientShader(Offset::Zero, at(5.0, 0.0), &[RED, BLUE], None, TileMode::Clamp)),
            ..Paint::default()
        };
        c.draw_vertices(&mesh, BlendMode::Modulate, &shaded);
        c.draw_vertices(&mesh, BlendMode::Modulate, &Paint { color: RED.copy_alpha(0.0), ..Paint::default() });
    }
    assert!(r.meshes.len() == 2 && r.order == "vv");
    let m0 = &r.meshes[0];
    assert!(
        m0.vertices == mesh && m0.origin == at(30.0, 40.0) && m0.blend_mode == BlendMode::Screen && near(m0.alpha, 0.5)
    );
    assert!(m0.brush.solid_color() == Some(RED) && m0.paint_blend == BlendMode::Multiply && m0.filters.len() == 1);
    assert!(same_rect(m0.brush_area, 30.0, 40.0, 0.0, 0.0));
    assert!(matches!(r.meshes[1].brush.kind(), BrushKind::Shader(_)) && near(r.meshes[1].alpha, 1.0));
}

fn path_effects_dash_and_round_strokes() {
    // C++ 28045.
    let c = canvas_of(100.0, 100.0, |d| {
        let dash = PathEffect::dash_path_effect(&[10.0, 5.0], 0.0);
        d.draw_line_with(
            Color::Black,
            Offset::Zero,
            at(40.0, 0.0),
            2.0,
            StrokeCap::Butt,
            Some(dash.clone()),
            1.0,
            None,
            SRC,
        );
        let five = PathEffect::dash_path_effect(&[10.0, 5.0], 5.0);
        d.draw_line_with(
            Color::Black,
            at(0.0, 10.0),
            at(40.0, 10.0),
            2.0,
            StrokeCap::Round,
            Some(five),
            1.0,
            None,
            SRC,
        );
        let mut square = Path::new();
        square.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CounterClockwise);
        let corner = Some(PathEffect::corner_path_effect(4.0));
        d.draw_path_with(
            &square,
            Color::Black,
            1.0,
            stroke_of(1.0, StrokeCap::Butt, StrokeJoin::Miter, corner),
            None,
            SRC,
        );
        let dashed = stroke_of(2.0, StrokeCap::Butt, StrokeJoin::Miter, Some(dash));
        d.draw_circle_with(Color::Black, 10.0, at(50.0, 50.0), 1.0, dashed, None, SRC);
        d.draw_rect_with(Color::Black, Offset::Zero, sz(30.0, 30.0), 1.0, stroke(2.0), None, SRC); // no effect
        let chained = PathEffect::chain_path_effect(
            &PathEffect::dash_path_effect(&[4.0, 4.0], 0.0),
            &PathEffect::corner_path_effect(6.0),
        );
        let style = stroke_of(1.0, StrokeCap::Butt, StrokeJoin::Miter, Some(chained));
        d.draw_round_rect_with(Color::Black, Offset::Zero, sz(30.0, 30.0), CornerRadius::Zero, 1.0, style, None, SRC);
        let three = Some(PathEffect::dash_path_effect(&[3.0, 3.0], 0.0));
        let dashes = stroke_of(1.0, StrokeCap::Butt, StrokeJoin::Miter, three);
        d.draw_arc_with(Color::Black, 0.0, 90.0, false, Offset::Zero, sz(20.0, 20.0), 1.0, dashes, None, SRC);
    });
    assert!(c.path_shapes.len() == 6 && c.strokes.len() == 1);
    // A line with butt ends: each dash its rectangle, filled (Skia's way with a line).
    assert_eq!(
        path_text(&c.path_shapes[0]),
        "M0,-1 L10,-1 L10,1 L0,1 M15,-1 L25,-1 L25,1 L15,1 M30,-1 L40,-1 L40,1 L30,1"
    );
    assert_eq!(c.path_strokes[0], 0.0);
    assert_eq!(path_text(&c.path_shapes[1]), "M0,10 L5,10 M10,10 L20,10 M25,10 L35,10"); // 5 into the pattern
    // Each corner cut back 4 and rounded through it, from the first side cut back (counterclockwise).
    assert_eq!(path_text(&c.path_shapes[2]), "M0,4 L0,16 Q4,20 L16,20 Q20,16 L20,4 Q16,0 L4,0 Q0,4 Z");
    // As Android's canvas makes them, clockwise: the circle from its right, dashed from there downwards; the rect
    // from its top left (rounded, then dashed from its first side cut back).
    let dashes = c.path_shapes[3].commands();
    let first = match dashes[0] {
        PathCommand::Move(p) => p,
        _ => panic!("a move"),
    };
    assert!(near_by(first.x, 50.58, 0.01) && near_by(first.y, 59.98, 0.01));
    assert!(matches!(dashes[1], PathCommand::Conic(..)));
    let last = dashes.last().unwrap().end().unwrap();
    assert!(near_by(last.x, 55.33, 0.01) && near_by(last.y, 58.46, 0.01));
    let chained = path_text(&c.path_shapes[4]);
    assert!(chained.starts_with("M14,0 L18,0") && chained.ends_with("M6,0 L10,0"));
    let length = |p: &Path| {
        let mut m = PathMeasure::new();
        m.set_path(p, false);
        let mut total = m.length();
        while m.next_contour() {
            total += m.length();
        }
        total
    };
    assert!(near_by(length(&c.path_shapes[3]), 2.0 * std::f32::consts::PI * 10.0 * 2.0 / 3.0, 1.5));
    assert!(chained.contains('Q') && chained.find('M') != chained.rfind('M')); // rounded, then dashed
    assert!(near_by(length(&c.path_shapes[5]), 9.0, 0.3)); // a quarter circle (15.7) in 3 on, 3 off
}

fn draw_scopes_take_composes_parameters() {
    // C++ 28092 (its display list half is the display list's).
    let ramp = hgrad(&[RED, BLUE]);
    let c = canvas_of(100.0, 100.0, move |d| {
        assert!(d.density().density == 1.0 && d.layout_direction() == LayoutDirection::Ltr);
        let r = ramp.clone();
        d.draw_rect_with(RED, Offset::Zero, sz(10.0, 10.0), 0.5, P, None, SRC); // r0: faded
        d.draw_rect_with(r.clone(), at(10.0, 0.0), sz(10.0, 10.0), 0.5, P, None, BlendMode::Multiply); // r1
        d.draw_circle_with(RED, 5.0, at(30.0, 5.0), 1.0, P, Some(tint(BLUE)), SRC); // r2: filtered
        d.draw_round_rect_with(RED, at(40.0, 0.0), sz(10.0, 10.0), CornerRadius::circular(3.0), 1.0, P, None, SRC); // r3
        d.draw_round_rect_with(RED, at(50.0, 0.0), sz(10.0, 10.0), CornerRadius::new(2.0, 4.0), 0.25, P, None, SRC); // p0
        d.draw_rect_with(RED, at(60.0, 0.0), sz(10.0, 10.0), 1.0, DrawStyle::Stroke(Stroke::default()), None, SRC); // p1
        let bevel = DrawStyle::Stroke(Stroke::new(2.0, 10.0, StrokeCap::Square, StrokeJoin::Bevel, None));
        d.draw_rect_with(RED, at(70.0, 0.0), sz(10.0, 10.0), 1.0, bevel, None, SRC); // p2: bevelled
        d.draw_line_with(r, at(0.0, 20.0), at(40.0, 20.0), 3.0, StrokeCap::Butt, None, 0.5, None, SRC); // p3: a brush
        d.draw_line(RED, at(0.0, 30.0), at(40.0, 30.0)); // p4: a hairline
        d.draw_line_with(RED, at(0.0, 40.0), at(40.0, 40.0), 2.0, StrokeCap::Square, None, 0.5, None, BlendMode::Plus);
        d.draw_oval_with(BLUE, at(0.0, 50.0), sz(20.0, 10.0), 0.5, stroke(1.0), None, SRC); // p5
        d.draw_points(&[at(80.0, 80.0), at(90.0, 90.0)], PointMode::Points, RED); // p6: hairline dots
        d.draw_line_with(RED, at(0.0, 60.0), at(40.0, 60.0), 2.0, StrokeCap::Butt, None, 1.0, None, SRC); // p7
    });
    assert!(c.rects.len() == 4 && c.paths.len() == 8);
    assert!(path_text(&c.path_shapes[7]) == "M0,60 L40,60" && c.path_stroke_styles[7].cap == StrokeCap::Butt);
    assert_eq!(c.path_strokes[7], 2.0);
    assert!(near(c.rect_colors[0].alpha(), 0.5) && c.rect_blends[1] == BlendMode::Multiply && c.gradients == 1);
    assert!(c.rect_colors[2] == BLUE && near(c.rect_radii[3], 3.0)); // tinted; a circle's corners
    assert!(near(c.path_colors[0].alpha(), 0.25) && path_text(&c.path_shapes[0]).contains('K'));
    assert!(c.path_stroke_styles[1].hairline && c.path_strokes[1] == 0.0);
    let bevel = &c.path_stroke_styles[2];
    assert!(
        bevel.width == 2.0 && bevel.miter == 10.0 && bevel.cap == StrokeCap::Square && bevel.join == StrokeJoin::Bevel
    );
    assert!(!bevel.hairline);
    assert!(c.path_strokes[3] == 3.0 && near(c.path_colors[3].alpha(), 0.5)); // the brush's colors faded
    assert!(c.path_stroke_styles[4].hairline);
    assert!(c.lines.len() == 1 && near(c.line_colors[0].alpha(), 0.5));
    assert!(near(c.path_colors[5].alpha(), 0.5) && c.path_strokes[5] == 1.0);
    assert!(near(c.paths[6].width(), 11.0) && c.path_strokes[6] == 0.0); // two 1 px dots, 10 apart
    // Arcs: a pie slice, filled across its ends, its outline through the center, an arc; a turn and more: a whole
    // oval filled, outlined round and on.
    let a = canvas_of(100.0, 100.0, |d| {
        let arc = |d: &mut DrawScope, sweep: f32, center: bool, size: Size, style: DrawStyle| {
            d.draw_arc_with(RED, 0.0, sweep, center, Offset::Zero, size, 1.0, style, None, SRC)
        };
        arc(d, 90.0, true, sz(20.0, 20.0), P);
        arc(d, 90.0, false, sz(20.0, 20.0), P);
        arc(d, 90.0, true, sz(20.0, 20.0), stroke(1.0));
        arc(d, 90.0, false, sz(20.0, 20.0), stroke_of(2.0, StrokeCap::Round, StrokeJoin::Miter, None)); // the backend's
        arc(d, 90.0, false, sz(20.0, 10.0), stroke(2.0)); // an ellipse's: a path
        arc(d, 400.0, false, sz(20.0, 20.0), P);
        arc(d, 400.0, false, sz(20.0, 20.0), stroke_of(1.0, StrokeCap::Square, StrokeJoin::Miter, None));
    });
    assert!(a.path_shapes.len() == 6 && a.arcs.len() == 1);
    assert_eq!(path_text(&a.path_shapes[0]), "M10,10 L20,10 K10,20 Z"); // a quarter circle: one conic
    assert_eq!(path_text(&a.path_shapes[1]), "M20,10 K10,20");
    assert!(path_text(&a.path_shapes[2]) == "M10,10 L20,10 K10,20 Z" && a.path_strokes[2] == 1.0);
    assert!(a.arcs[0].sweep == 90.0 && near(a.arcs[0].stroke_width, 2.0));
    assert_eq!(path_text(&a.path_shapes[3]), "M20,5 K10,10");
    let mut oval = Path::new(); // (as Android's canvas makes it: clockwise, from its right)
    oval.add_oval(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::Clockwise);
    assert_eq!(a.path_shapes[4], oval);
    let mut around = PathMeasure::new(); // a turn and 40 degrees, in one contour
    around.set_path(&a.path_shapes[5], false);
    assert!(near_by(around.length(), 2.0 * std::f32::consts::PI * 10.0 * 400.0 / 360.0, 0.5) && !around.next_contour());
    // Images: at their size; part of one into a rect, sampled as asked; all of it by default (Compose's).
    let photo = ImageBitmap::from_pixels(8, 6, vec![200; 8 * 6 * 4]).unwrap();
    let im = canvas_of(100.0, 100.0, move |d| {
        d.draw_image_with(&photo, at(5.0, 5.0), 0.5, P, None, SRC);
        let (src_at, src_size) = (IntOffset::new(2, 1), IntSize::new(4, 3));
        let (dst_at, dst_size) = (IntOffset::new(20, 20), IntSize::new(40, 30));
        let (cf, q) = (Some(tint(BLUE)), FilterQuality::None);
        d.draw_image_rect_with(&photo, src_at, src_size, dst_at, dst_size, 1.0, P, cf, BlendMode::Src, q);
        d.draw_image_rect(&photo); // all of it, at its size
    });
    assert_eq!(im.images.len(), 3);
    assert!(
        same_rect(im.image_sources[0], 0.0, 0.0, 8.0, 6.0) && same_rect(im.image_destinations[0], 5.0, 5.0, 8.0, 6.0)
    );
    assert_eq!(im.image_qualities[0], FilterQuality::Low);
    assert!(
        same_rect(im.image_sources[1], 2.0, 1.0, 4.0, 3.0)
            && same_rect(im.image_destinations[1], 20.0, 20.0, 40.0, 30.0)
    );
    assert!(im.image_qualities[1] == FilterQuality::None && im.image_filters[1].len() == 1);
    assert_eq!(im.image_blends[1], BlendMode::Src);
    assert!(
        same_rect(im.image_sources[2], 0.0, 0.0, 8.0, 6.0) && same_rect(im.image_destinations[2], 0.0, 0.0, 8.0, 6.0)
    );
}

fn stamped_path_effects_are_drawn_filled() {
    // C++ 28241's drawing (the effect itself is kompoz-graphics'): a stamped stroke draws its stamps filled.
    let mut boxed = Path::new();
    boxed.add_rect(Rect::new(0.0, -1.0, 4.0, 1.0), PathDirection::CounterClockwise);
    let mut line = Path::new();
    line.move_to(0.0, 0.0);
    line.line_to(100.0, 0.0);
    let c = canvas_of(120.0, 20.0, move |d| {
        let effect = PathEffect::stamped_path_effect(&boxed, 20.0, 0.0, StampedPathEffectStyle::Translate);
        let style = stroke_of(3.0, StrokeCap::Butt, StrokeJoin::Miter, Some(effect));
        d.draw_path_with(&line, Color::Black, 1.0, style, None, SRC);
    });
    assert!(c.path_strokes.len() == 1 && c.path_strokes[0] == 0.0);
}

fn draw_points_as_dots_lines_or_one_line() {
    // C++ 28939.
    let points = [Offset::Zero, at(10.0, 0.0), at(10.0, 10.0), at(0.0, 10.0), at(5.0, 5.0)];
    let c = canvas_of(100.0, 100.0, move |d| {
        let mut pts = |mode: PointMode, width: f32, cap: StrokeCap| {
            d.draw_points_with(&points, mode, Color::Black, width, cap, None, 1.0, None, SRC)
        };
        pts(PointMode::Points, 4.0, StrokeCap::Round);
        pts(PointMode::Points, 4.0, StrokeCap::Butt);
        pts(PointMode::Lines, 2.0, StrokeCap::Butt);
        pts(PointMode::Polygon, 2.0, StrokeCap::Butt);
    });
    assert_eq!(c.path_shapes.len(), 4);
    let dots = path_text(&c.path_shapes[0]);
    assert!(dots.matches('M').count() == 5 && dots.contains('K') && c.path_strokes[0] == 0.0); // ovals: conics
    assert!(path_text(&c.path_shapes[1]).starts_with("M-2,-2 L-2,2 L2,2 L2,-2 Z") && c.path_strokes[1] == 0.0);
    assert!(path_text(&c.path_shapes[2]) == "M0,0 L10,0 M10,10 L0,10" && c.path_strokes[2] == 2.0); // no partner
    assert_eq!(path_text(&c.path_shapes[3]), "M0,0 L10,0 L10,10 L0,10 L5,5");
}

fn round_capped_lines_are_drawn_as_paths() {
    // C++ 28998.
    let c = canvas_of(50.0, 50.0, |d| {
        d.draw_line_with(Color::Black, Offset::Zero, at(40.0, 0.0), 4.0, StrokeCap::Square, None, 1.0, None, SRC);
        d.draw_line_with(Color::Black, at(0.0, 10.0), at(40.0, 10.0), 4.0, StrokeCap::Round, None, 1.0, None, SRC);
        d.draw_line_with(Color::Black, at(0.0, 20.0), at(40.0, 20.0), 4.0, StrokeCap::Butt, None, 1.0, None, SRC);
    });
    assert!(c.lines.len() == 1 && c.path_shapes.len() == 2 && c.path_strokes.len() == 2 && c.path_strokes[0] == 4.0);
    assert!(c.path_stroke_styles[0].cap == StrokeCap::Round && c.path_stroke_styles[1].cap == StrokeCap::Butt);
}

// ---- Layers, clips, alpha, effects -------------------------------------------------------------------------------

/// Places what it wraps `x`, `y` px from where it is, its size its own (Compose's Modifier.offset, which the layout
/// modifiers package adds).
fn moved(m: Modifier, x: i32, y: i32) -> Modifier {
    m.layout(move |scope, measurable, constraints| {
        let p = measurable.measure(constraints);
        scope.layout(p.width(), p.height(), move |pl| pl.place(&p, x, y))
    })
}

/// What `content` draws in a 400 x 400 window, seeing only `visible` there if given.
fn look(content: impl Content + 'static, visible: Option<Rect>) -> RecordingCanvas {
    let rule = KompozTestRule::with_viewport(content, IntSize::new(400, 400));
    let mut canvas = RecordingCanvas::new();
    canvas.visible = visible;
    rule.host().draw(&mut canvas);
    canvas
}

fn layer(f: impl FnOnce(&mut GraphicsLayerScope)) -> GraphicsLayerScope {
    let mut l = GraphicsLayerScope::default();
    f(&mut l);
    l
}

fn rounded_clip_is_a_clip_and_alpha_a_layer() {
    // C++ 3272.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Box(modifier = Modifier.size(40.dp()).clip(CircleShape).background(Color::Black));
            Box(modifier = Modifier.size(40.dp()).clip(RectangleShape).background(Color::Black)); // a plain clip rect
            Box(modifier = Modifier.size(40.dp()).alpha(0.5), content = || Text("yarım"));
            Box(modifier = Modifier.size(40.dp()).alpha(1.0), content = || Text("tam"));
            Box(modifier = Modifier.size(40.dp()).alpha(0.0), content = || Text("görünmez"));
        });
    }));
    let c = rule.draw();
    assert!(c.layers == 1 && c.depth == 0); // the half alpha
    assert!(
        c.clip_shapes.len() == 1 && c.clip_shapes[0].radii == CornerRadii::all(20.0) && c.clip_shapes[0].path.is_none()
    );
    assert_eq!(c.texts, ["yarım", "tam"]);
}

/// A platform whose renderer records what it is asked to draw.
struct Offscreen {
    asked: Cell<Size>,
    drawn: RefCell<Vec<Offset>>,
}

impl PlatformCallbacks for Offscreen {
    fn render_image(&self, size: Size, draw: &dyn Fn(&mut dyn GraphicsContext)) -> Option<ImageBitmap> {
        self.asked.set(size);
        let mut offscreen = RecordingCanvas::new();
        draw(&mut offscreen);
        self.drawn.borrow_mut().extend(offscreen.rects.iter().map(|r| r.top_left()));
        let (w, h) = (size.width as i32, size.height as i32);
        ImageBitmap::from_pixels(w, h, vec![255; (w * h * 4) as usize])
    }
}

fn graphics_layer_records_content_and_draws_it_again() {
    // C++ 3419.
    let kept: Rc<RefCell<Option<GraphicsLayer>>> = Rc::new(RefCell::new(None));
    let k = kept.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = remember_graphics_layer();
        *k.borrow_mut() = Some(l.clone());
        Column(|| {
            Box(
                modifier = Modifier.padding(10.dp()).size_wh(100.dp(), 40.dp()).draw_with_content(move |d| {
                    d.record(&l, |d| d.draw_content());
                    d.draw_layer(&l); // where it is
                    l.set_top_left(IntOffset::new(0, 50)); // and a faded copy under it
                    l.set_alpha(0.5);
                    d.draw_layer(&l);
                    l.set_properties(GraphicsLayerProperties::default());
                }),
                content = || {
                    Text("kayıt");
                    Box(modifier = Modifier.size(20.dp()).background(RED));
                },
            );
        });
    }));
    let c = rule.draw();
    // The content only through the layer: twice, 50 px apart, the copy in a half faded layer.
    let texts: Vec<f32> = c.texts.iter().zip(&c.baselines).filter(|(t, _)| *t == "kayıt").map(|(_, b)| b.y).collect();
    let rects: Vec<f32> = c.rects.iter().zip(&c.rect_colors).filter(|(_, c)| **c == RED).map(|(r, _)| r.top).collect();
    assert!(texts.len() == 2 && near(texts[1] - texts[0], 50.0) && rects.len() == 2);
    assert!(near(rects[0], 10.0) && near(rects[1], 60.0));
    let faded = (0..c.layer_bounds.len()).any(|i| {
        near(c.layer_alphas[i], 0.5) && near(c.layer_bounds[i].top, 60.0) && near(c.layer_bounds[i].height(), 40.0)
    });
    let layer = kept.borrow().clone().unwrap();
    assert!(faded && layer.size() == IntSize::new(100, 40));
    // As pixels: the platform draws the recording, from its top left; nothing without one.
    assert!(layer.to_image_bitmap().is_none());
    let platform = Rc::new(Offscreen { asked: Cell::new(Size::Zero), drawn: RefCell::new(Vec::new()) });
    rule.host().set_callbacks(platform.clone());
    let shot = layer.to_image_bitmap().unwrap();
    assert!(shot.width() == 100 && shot.height() == 40 && near(platform.asked.get().width, 100.0));
    assert!(platform.drawn.borrow().len() == 1 && *platform.drawn.borrow() == [Offset::Zero]);
}

fn graphics_layer_transforms_without_a_layer_and_fades_through_one() {
    // C++ 8456: a block read as the layer is worked out: a new angle draws it again without composing.
    let angle = mutable_state_of(0.0f32);
    let a = angle.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let a = a.clone();
        Column(|| {
            Box(modifier = Modifier.size(40.dp()).graphics_layer_with(layer(|l| l.rotation_z = 45.0)));
            Box(modifier = Modifier.size(40.dp()).graphics_layer_with(GraphicsLayerScope::default())); // nothing
            Box(modifier = Modifier.size(40.dp()).graphics_layer_with(layer(|l| l.alpha = 0.5))); // not transformed
            Box(modifier = Modifier.size(40.dp()).graphics_layer_block(move |l| l.rotation_z = a.get()));
        });
    }));
    let before = rule.draw();
    assert!(before.layers == 1 && before.transformed_layers == 0 && before.transforms.len() == 1);
    assert!(near_by(placed_of(&before.transforms[0]).turn, 45.0, 0.01));
    let recompositions = rule.host().stats().recompositions;
    angle.set(30.0);
    rule.wait_for_idle();
    assert_eq!(rule.host().stats().recompositions, recompositions);
    let after = rule.draw();
    assert!(after.layers == 1 && after.transforms.len() == 2);
    assert!(near_by(placed_of(&after.transforms[1]).turn, 30.0, 0.01));
}

fn graphics_layer_takes_compose_named_arguments() {
    // C++ 8481.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Box(modifier = Modifier.size(40.dp()).graphics_layer(rotation_z = 45.0));
            Box(modifier =
                Modifier.size(40.dp()).graphics_layer(alpha = 0.5, clip = true, shape = Arc::new(CircleShape)));
        });
    }));
    let c = rule.draw();
    assert!(c.layers == 1 && c.transforms.len() == 1 && c.layer_alphas == [0.5]);
    assert!(near_by(placed_of(&c.transforms[0]).turn, 45.0, 0.01));
}

fn graphics_layer_matrix_is_androids() {
    // C++ 9094, through a layer's properties (its pivot a fraction of its size; px: the camera 576 px away).
    let at_ = |p: Offset, x: f32, y: f32| near_by(p.x, x, 0.01) && near_by(p.y, y, 0.01);
    let scope = |size: f32, origin: TransformOrigin, f: &dyn Fn(&mut GraphicsLayerScope)| {
        let mut l = GraphicsLayerScope::new(sz(size, size), Density::default(), LayoutDirection::Ltr);
        l.transform_origin = origin;
        f(&mut l);
        l
    };
    let flat_set = |l: &mut GraphicsLayerScope| {
        l.scale_x = 2.0;
        l.scale_y = 3.0;
        l.translation_x = 10.0;
        l.translation_y = 20.0;
        l.rotation_z = 90.0;
    };
    let flat = scope(100.0, TransformOrigin::Center, &flat_set); // scaled, turned a quarter, moved
    assert!(at_(flat.matrix().map(at(60.0, 50.0)), 50.0 + 10.0, 50.0 + 20.0 + 20.0));
    // Tipped back 60 degrees about x: a point 100 below the pivot comes forward, 100 above goes back.
    let zero = TransformOrigin::new(0.0, 0.0);
    let tipped = scope(100.0, zero, &|l| l.rotation_x = 60.0).matrix();
    let (c, s) = (0.5f32, 3.0f32.sqrt() / 2.0);
    assert!(at_(tipped.map(at(0.0, 100.0)), 0.0, c * 100.0 / (1.0 - s * 100.0 / 576.0)));
    assert!(at_(tipped.map(at(0.0, -100.0)), 0.0, -c * 100.0 / (1.0 + s * 100.0 / 576.0)));
    // Turned 60 degrees about y: the right side goes back.
    let swung = scope(100.0, zero, &|l| l.rotation_y = 60.0).matrix();
    assert!(at_(swung.map(at(100.0, 0.0)), c * 100.0 / (1.0 + s * 100.0 / 576.0), 0.0));
    assert!(at_(swung.map(at(-100.0, 0.0)), -c * 100.0 / (1.0 - s * 100.0 / 576.0), 0.0));
    // The 3D path turns about z as the flat one does.
    let both = scope(100.0, TransformOrigin::Center, &|l| {
        flat_set(l);
        l.rotation_x = 1e-4;
    });
    for p in [Offset::Zero, at(80.0, 30.0), at(-40.0, 70.0)] {
        let a = flat.matrix().map(p);
        assert!(at_(both.matrix().map(p), a.x, a.y));
    }
}

fn graphics_layer_goes_offscreen_only_where_it_must() {
    // C++ 9121: offscreen only faded (Auto), blended, filtered, or Offscreen; transformed and clipped otherwise
    // straight through the context; ModulateAlpha fades each draw. A GraphicsLayer drawn takes the same.
    let boxed = |l: GraphicsLayerScope| {
        kompozable_lambda!(
            move || Box(modifier = Modifier.size(40.dp()).graphics_layer_with(l.clone()).background(RED))
        )
    };
    let turned = look(
        boxed(layer(|l| {
            l.rotation_y = 30.0;
            l.clip = true;
        })),
        None,
    );
    assert!(turned.layers == 0 && turned.transforms.len() == 1 && turned.clips.len() == 1);
    let faded = look(boxed(layer(|l| l.alpha = 0.5)), None);
    assert!(faded.layers == 1 && near(faded.layer_alphas[0], 0.5) && faded.rect_colors.len() == 1);
    assert_eq!(faded.rect_colors[0].alpha(), 1.0);
    let modulated = look(
        boxed(layer(|l| {
            l.alpha = 0.5;
            l.compositing_strategy = CompositingStrategy::ModulateAlpha;
        })),
        None,
    );
    assert!(modulated.layers == 0 && near_by(modulated.rect_colors[0].alpha(), 0.5, 0.01)); // the draw faded
    let blended = look(boxed(layer(|l| l.blend_mode = BlendMode::Multiply)), None);
    assert!(blended.layers == 1 && blended.rect_blends == [BlendMode::Multiply]);
    let mut gray = ColorMatrix::new();
    gray.set_to_saturation(0.0);
    let filtered = look(boxed(layer(|l| l.color_filter = Some(ColorFilter::color_matrix(gray)))), None);
    assert!(filtered.layers == 1 && filtered.rect_colors[0].red() < 1.0 && filtered.rect_colors[0].green() > 0.0);
    let forced = look(boxed(layer(|l| l.compositing_strategy = CompositingStrategy::Offscreen)), None);
    assert_eq!(forced.layers, 1);
    // Faded and clipped to a rounded shape: the layer's own corners; with outsets: bigger, cut to the shape.
    let rounded_layer = |outsets: LayerOutsets| {
        layer(|l| {
            l.alpha = 0.5;
            l.shape = Arc::new(RoundedCornerShape(8.0f32));
            l.clip = true;
            l.outsets = outsets;
        })
    };
    let rounded = look(boxed(rounded_layer(LayerOutsets::Zero)), None);
    assert!(rounded.layers == 1 && near(rounded.layer_corners[0].top_left, 8.0) && rounded.clip_shapes.is_empty());
    let four = LayerOutsets::new(4.dp(), 4.dp(), 4.dp(), 4.dp());
    let outset = look(boxed(rounded_layer(four)), None);
    assert!(
        outset.layers == 1
            && same_rect(outset.layer_bounds[0], -4.0, -4.0, 48.0, 48.0)
            && outset.layer_outlines.len() == 1
    );
    // The size, read in a block.
    let seen = Rc::new(Cell::new(Size::Zero));
    let s = seen.clone();
    look(
        kompozable_lambda!(move || {
            let s = s.clone();
            Box(modifier = Modifier.size_wh(30.dp(), 20.dp()).graphics_layer_block(move |l| s.set(l.size())));
        }),
        None,
    );
    assert_eq!(seen.get(), sz(30.0, 20.0));
    // A GraphicsLayer: turned about its pivot, clipped to its rounded outline as it fades.
    let recorded = look(
        kompozable_lambda!(|| {
            let layer = remember_graphics_layer();
            Box(modifier = Modifier.size(100.dp()).draw_with_content(move |d| {
                d.record_with(&layer, IntSize::new(60, 40), |i| i.draw_rect(BLUE));
                layer.set_rotation_z(90.0);
                layer.set_pivot_offset(Offset::Zero);
                layer.set_alpha(0.5);
                layer.set_clip(true);
                layer.set_round_rect_outline(Offset::Zero, sz(60.0, 40.0), 10.0);
                d.draw_layer(&layer);
                layer.set_properties(GraphicsLayerProperties::default());
            }));
        }),
        None,
    );
    assert!(recorded.transforms.len() == 1 && recorded.layers == 1 && near(recorded.layer_corners[0].top_left, 10.0));
    let t = placed_of(&recorded.transforms[0]);
    let pivot = t.pivot.unwrap();
    assert!(near_by(t.turn, 90.0, 0.01) && near_by(pivot.x, 0.0, 0.01) && near_by(pivot.y, 0.0, 0.01));
}

fn scale_and_rotate_are_layers() {
    // C++ 10474 (its primitive states are the runtime's): a scale of 1 is no transform.
    let count = mutable_state_of(1);
    let n = count.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let scale = if n.get() > 1 { 2.0 } else { 1.0 };
        Row(|| {
            Box(modifier = Modifier.size(40.dp()).scale(scale).background(Color::Black));
            Box(modifier = Modifier.size(40.dp()).rotate(45.0).background(Color::Black));
        });
    }));
    let c = rule.draw();
    assert!(c.transforms.len() == 1 && c.layers == 0);
    count.set(2);
    rule.wait_for_idle();
    assert_eq!(rule.draw().transforms.len(), 2);
}

fn blend_modes_fade_an_offscreen_layer() {
    // C++ 30862 (its display list half is the display list's, its tints kompoz-graphics'): a list's bottom edge
    // faded out: the content, then a gradient keeping only what is under its alpha (DstIn), in a layer of its own.
    let offscreen = mutable_state_of(true);
    let o = offscreen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let o = o.clone();
        Box(
            modifier = Modifier
                .size_wh(100.dp(), 200.dp())
                .graphics_layer_block(move |l| {
                    l.compositing_strategy =
                        if o.get() { CompositingStrategy::Offscreen } else { CompositingStrategy::Auto };
                })
                .draw_with_content(|d| {
                    d.draw_content();
                    let fade = vgrad(&[Color::Black, Color::Transparent]);
                    d.draw_rect_with(fade, Offset::Zero, Size::Unspecified, 1.0, Fill, None, BlendMode::DstIn);
                    let mut corner = Path::new();
                    corner.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0), PathDirection::CounterClockwise);
                    d.draw_path_with(&corner, Color::Black, 1.0, Fill, None, BlendMode::Clear);
                    d.draw_circle_with(Color::Black, 5.0, Offset::new(50.0, 50.0), 1.0, Fill, None, BlendMode::DstOut);
                }),
            content = || Box(modifier = Modifier.fill_max_size().background(Color::hex(0x6750A4))),
        );
    }));
    let c = rule.draw();
    // Its own layer even at full alpha with no transform; the content plain, the mask DstIn.
    assert!(c.layers == 1 && c.layer_bounds.len() == 1 && same_rect(c.layer_bounds[0], 0.0, 0.0, 100.0, 200.0));
    assert_eq!(c.rect_blends, [BlendMode::SrcOver, BlendMode::DstIn, BlendMode::DstOut]);
    assert!(c.path_blends == [BlendMode::Clear] && c.blend_stack.is_empty());
    // Auto: no layer of its own (the blend modes would act on what is behind).
    offscreen.set(false);
    rule.wait_for_idle();
    assert_eq!(rule.draw().layers, 0);
}

fn blur_is_a_graphics_layer_with_a_blur_effect() {
    // C++ 30918 (its display list half is the display list's).
    let radius = mutable_state_of(8.0f32);
    let radius_y = mutable_state_of(8.0f32);
    let edges = mutable_state_of(BlurredEdgeTreatment::Rectangle);
    let (r, ry, e) = (radius.clone(), radius_y.clone(), edges.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let blurred =
            Modifier.padding(50.dp()).size(100.dp()).blur_xy_with(Dp(r.get()), Dp(ry.get()), e.get(), BlurType::Family);
        Box(modifier = blurred.background(Color::Black));
    }));
    // Rectangle: clipped to the bounds, blurred within them, the edges' pixels taken past them.
    let c = rule.draw();
    assert!(
        c.layers == 1
            && c.layer_effects.len() == 1
            && c.layer_effects[0] == Some(BlurEffect(8.0, 8.0, TileMode::Clamp))
    );
    assert!(same_rect(c.layer_bounds[0], 50.0, 50.0, 100.0, 100.0) && c.clips.len() == 1);
    assert!(same_rect(c.clips[0], 50.0, 50.0, 100.0, 100.0));
    // Unbounded: spreading out, fading past the bounds, nothing clipped; each way its own radius.
    edges.set(BlurredEdgeTreatment::Unbounded);
    radius_y.set(4.0);
    rule.wait_for_idle();
    let c = rule.draw();
    assert!(
        c.layer_effects == [Some(BlurEffect(8.0, 4.0, TileMode::Decal))]
            && c.clips.is_empty()
            && c.clip_shapes.is_empty()
    );
    assert!(same_rect(c.layer_bounds[0], 50.0, 50.0, 100.0, 100.0));
    // A shape of its own: cut to it.
    edges.set(BlurredEdgeTreatment::of(CircleShape));
    rule.wait_for_idle();
    let c = rule.draw();
    assert!(c.layer_effects == [Some(BlurEffect(8.0, 4.0, TileMode::Clamp))] && c.clip_shapes.len() == 1);
    assert!(
        near(c.clip_shapes[0].radii.top_left, 50.0) && same_rect(c.clip_shapes[0].bounds, 50.0, 50.0, 100.0, 100.0)
    );
    // As in Compose, blurred only if both radii are: one of 0 leaves the clip alone.
    radius_y.set(0.0);
    edges.set(BlurredEdgeTreatment::Rectangle);
    rule.wait_for_idle();
    let c = rule.draw();
    assert!(c.layers == 0 && c.clips.len() == 1 && c.rects.len() == 1);
    // Unbounded, nothing to blur: as it is.
    edges.set(BlurredEdgeTreatment::Unbounded);
    rule.wait_for_idle();
    let c = rule.draw();
    assert!(c.layers == 0 && c.clips.is_empty() && c.rects.len() == 1);
    let _ = radius;
}

fn modifiers_with_effects_compare_by_them() {
    // C++ 30967's modifiers (the effects themselves are kompoz-graphics'): a composable called with an equal one is
    // skipped.
    assert!(Modifier.blur(4.dp()) == Modifier.blur(4.dp()) && Modifier.blur(4.dp()) != Modifier.blur(5.dp()));
    let unbounded = Modifier.blur_xy_with(4.dp(), 4.dp(), BlurredEdgeTreatment::Unbounded, BlurType::Family);
    assert!(Modifier.blur(4.dp()) != unbounded);
    let offset = |x: f32| Modifier.graphics_layer_with(layer(|l| l.render_effect = Some(OffsetEffect(x, 0.0))));
    assert!(offset(1.0) != offset(2.0) && offset(1.0) == offset(1.0));
    // Blur types compared.
    let typed = |t: BlurType| Modifier.blur_xy_with(4.dp(), 4.dp(), BlurredEdgeTreatment::Rectangle, t);
    assert!(typed(BlurType::Morph) != typed(BlurType::SelfOnly) && typed(BlurType::Morph) == typed(BlurType::Morph));
    // No elevation, no clip: nothing.
    assert!(Modifier.shadow(Dp(0.0)) == Modifier);
}

fn radial(start: f32, end: f32) -> BlurRadiusSpec {
    BlurRadiusSpec::radial_gradient(Dp(start), Dp(end), Offset::Unspecified, Dp::Unspecified)
}

fn blur_with_a_radius_spec_is_a_layer_as_composes() {
    // C++ 31042.
    let edges = mutable_state_of(BlurredEdgeTreatment::Rectangle);
    let alpha = mutable_state_of(1.0f32);
    let spec = mutable_state_of(BlurRadiusSpec::vertical_gradient(Dp(0.0), Dp(12.0)));
    let (e, a, s) = (edges.clone(), alpha.clone(), spec.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let blurred = Modifier.padding(50.dp()).size_wh(100.dp(), 60.dp()).blur_spec_with(
            s.get(),
            e.get(),
            a.get(),
            BlurType::Family,
        );
        Box(modifier = blurred.background(Color::Black));
    }));
    let d = Density::default();
    let effect = |tile: TileMode| {
        Some(BlurRadiusSpec::vertical_gradient(Dp(0.0), Dp(12.0)).create_render_effect(sz(100.0, 60.0), tile, &d))
    };
    let c = rule.draw();
    assert!(c.layers == 1 && c.layer_effects[0] == effect(TileMode::Clamp));
    assert!(
        c.clips.len() == 1
            && same_rect(c.clips[0], 50.0, 50.0, 100.0, 60.0)
            && same_rect(c.layer_bounds[0], 50.0, 50.0, 100.0, 60.0)
    );
    edges.set(BlurredEdgeTreatment::Unbounded);
    alpha.set(0.5);
    rule.wait_for_idle();
    let c = rule.draw();
    assert!(
        c.layers == 1
            && c.layer_effects[0] == effect(TileMode::Decal)
            && c.clips.is_empty()
            && near(c.layer_alphas[0], 0.5)
    );
    // A uniform 0: no effect; faded, a layer all the same.
    spec.set(BlurRadiusSpec::uniform(Dp(0.0)));
    rule.wait_for_idle();
    let c = rule.draw();
    assert!(c.layers == 1 && c.layer_effects[0].is_none() && near(c.layer_alphas[0], 0.5));
    // Compared by value; a block isn't, and is read while drawing, with the size.
    let uniform = |alpha: f32| {
        Modifier.blur_spec_with(
            BlurRadiusSpec::uniform(Dp(3.0)),
            BlurredEdgeTreatment::Rectangle,
            alpha,
            BlurType::Family,
        )
    };
    assert!(Modifier.blur_spec(BlurRadiusSpec::uniform(Dp(3.0))) == uniform(1.0) && uniform(1.0) != uniform(0.5));
    let radius = mutable_state_of(4.0f32);
    let seen = Rc::new(Cell::new(Size::Zero));
    let compositions = Rc::new(Cell::new(0));
    let (r, sn, n) = (radius.clone(), seen.clone(), compositions.clone());
    let blocks = KompozTestRule::new(kompozable_lambda!(move || {
        n.set(n.get() + 1);
        let (r, sn) = (r.clone(), sn.clone());
        Box(modifier = Modifier.size_wh(80.dp(), 40.dp()).blur_block(move |b| {
            sn.set(b.size());
            b.blur_radius_spec = radial(r.get(), 0.0);
            b.edge_treatment = BlurredEdgeTreatment::of(CircleShape);
        }));
    }));
    let c = blocks.draw();
    let radial_effect = |r: f32| Some(radial(r, 0.0).create_render_effect(sz(80.0, 40.0), TileMode::Clamp, &d));
    assert!(c.layers == 1 && c.layer_effects[0] == radial_effect(4.0));
    assert!(seen.get() == sz(80.0, 40.0) && c.clip_shapes.len() == 1 && near(c.clip_shapes[0].radii.top_left, 20.0));
    radius.set(9.0);
    blocks.wait_for_idle();
    let c = blocks.draw();
    assert!(compositions.get() == 1 && c.layer_effects == [radial_effect(9.0)]);
    // A shader's is made with the node's density (its size the node's, px).
    let made_at = Arc::new(parking_lot_free::Slot::default());
    let m = made_at.clone();
    let masked = BlurRadiusSpec::shader(Dp(6.0), move |density: &Density, size: Size| {
        m.set(*density);
        LinearGradientShader(
            Offset::Zero,
            at(0.0, size.height),
            &[Color::Transparent, Color::Black],
            None,
            TileMode::Clamp,
        )
    });
    let k = masked.clone();
    let shaded = KompozTestRule::new(kompozable_lambda!(move || {
        Box(modifier = Modifier.size_wh(80.dp(), 40.dp()).blur_spec(k.clone()));
    }));
    shaded.host().set_density(Density::new(2.0, 1.5));
    shaded.wait_for_idle();
    let f = shaded.draw();
    let at_density = Density::new(2.0, 1.5);
    assert!(made_at.get() == Some(at_density) && f.layers == 1);
    assert_eq!(f.layer_effects[0], Some(masked.create_render_effect(sz(160.0, 80.0), TileMode::Clamp, &at_density)));
}

/// What a test reads from a block run elsewhere (a blur shader's: `Send + Sync`).
mod parking_lot_free {
    use kompoz_graphics::Density;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct Slot(Mutex<Option<Density>>);

    impl Slot {
        pub fn set(&self, d: Density) {
            *self.0.lock().unwrap() = Some(d);
        }
        pub fn get(&self) -> Option<Density> {
            *self.0.lock().unwrap()
        }
    }
}

fn blur_types_blur_the_node_what_it_draws_or_what_is_behind() {
    // C++ 31105.
    let kind = mutable_state_of(BlurType::SelfOnly);
    let edges = mutable_state_of(BlurredEdgeTreatment::Rectangle);
    let (k, e) = (kind.clone(), edges.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (k, e) = (k.clone(), e.clone());
        Box(
            modifier = Modifier.padding_sides_with(20.dp(), 20.dp(), 0.dp(), 0.dp()).size(100.dp()),
            content = move || {
                Box(modifier = Modifier.background(GREEN).size(100.dp())); // behind
                let blurred = Modifier
                    .size(80.dp())
                    .blur_xy_with(6.dp(), 6.dp(), e.get(), k.get())
                    .padding(10.dp())
                    .clip(RoundedCornerShape(8.0f32))
                    .graphics_layer_with(layer(|l| l.alpha = 0.5))
                    .background(RED);
                Box(modifier = blurred, content = || Box(modifier = Modifier.size(20.dp()).background(BLUE)));
            },
        );
    }));
    // SelfOnly: what the node draws itself (its background, in its clip and faded) through the blur's layer; its
    // child after it, out of it, sharp, but in the clip and faded as the nodes in between have it.
    let c = rule.draw();
    assert!(c.rects.len() == 3 && c.rect_colors[1].red() == 1.0 && c.rect_colors[2].blue() == 1.0);
    assert_eq!(c.layer_effects, [Some(BlurEffect(6.0, 6.0, TileMode::Clamp)), None, None]);
    assert_eq!(c.rect_depths, [0, 2, 1]); // the child in the faded layer alone
    assert!(near(c.layer_alphas[1], 0.5) && near(c.layer_alphas[2], 0.5));
    assert!(c.clip_shapes.len() == 2 && same_rect(c.clip_shapes[0].bounds, 30.0, 30.0, 60.0, 60.0));
    assert!(same_rect(c.clip_shapes[1].bounds, 30.0, 30.0, 60.0, 60.0));
    assert!(same_rect(c.rects[2], 30.0, 30.0, 20.0, 20.0) && c.backdrops.is_empty());
    // Morph: what is behind blurred within the node's shape, then the node over it, sharp.
    kind.set(BlurType::Morph);
    rule.wait_for_idle();
    let c = rule.draw();
    assert!(c.backdrops.len() == 1 && same_rect(c.backdrops[0].bounds, 20.0, 20.0, 80.0, 80.0));
    assert!(c.backdrops[0].effect == BlurEffect(6.0, 6.0, TileMode::Clamp) && near(c.backdrops[0].alpha, 1.0));
    assert!(c.order == "rbrr" && c.clips.len() == 1 && same_rect(c.clips[0], 20.0, 20.0, 80.0, 80.0));
    assert_eq!(c.layer_effects, [None]); // (the graphics layer's, faded)
    // Unbounded: not cut, spreading out.
    edges.set(BlurredEdgeTreatment::Unbounded);
    rule.wait_for_idle();
    let c = rule.draw();
    assert!(
        c.backdrops.len() == 1 && c.backdrops[0].effect == BlurEffect(6.0, 6.0, TileMode::Decal) && c.clips.is_empty()
    );
    // Family: as Compose's.
    kind.set(BlurType::Family);
    rule.wait_for_idle();
    let c = rule.draw();
    assert!(
        c.backdrops.is_empty()
            && c.layer_effects == [Some(BlurEffect(6.0, 6.0, TileMode::Decal)), None]
            && c.rect_depths[2] == 2
    );
    // With a spec (and a block): its alpha on what is behind.
    let glass = look(
        kompozable_lambda!(|| {
            Box(modifier = Modifier.size(100.dp()).blur_block(|b| {
                b.blur_radius_spec = BlurRadiusSpec::vertical_gradient(Dp(0.0), Dp(10.0));
                b.alpha = 0.4;
                b.blur_type = BlurType::Morph;
            }));
        }),
        None,
    );
    let d = Density::default();
    let effect = BlurRadiusSpec::vertical_gradient(Dp(0.0), Dp(10.0)).create_render_effect(
        sz(100.0, 100.0),
        TileMode::Clamp,
        &d,
    );
    assert!(glass.backdrops.len() == 1 && near_by(glass.backdrops[0].alpha, 0.4, 1e-6) && glass.layers == 0);
    assert_eq!(glass.backdrops[0].effect, effect);
}

fn elevation_shadows_go_with_graphics_layers() {
    // C++ 31208 (positions in the layer's own coordinates: the coordinator moves the context to the layer).
    let flat = look(
        kompozable_lambda!(|| {
            let l = layer(|l| {
                l.shadow_elevation = 6.0;
                l.ambient_shadow_color = RED;
                l.shape = Arc::new(RoundedCornerShape(8.0f32));
            });
            Box(modifier = moved(Modifier, 20, 30).size_wh(60.dp(), 40.dp()).graphics_layer_with(l).background(RED));
        }),
        None,
    );
    // Flat: its shape's outline, as high as its elevation, before its content, not clipped.
    assert!(
        flat.elevation_shadows.len() == 1
            && flat.order.starts_with("er")
            && flat.clip_shapes.is_empty()
            && flat.layers == 0
    );
    let e = &flat.elevation_shadows[0];
    assert!(e.shadow.plane == [0.0, 0.0, 6.0] && e.shadow.ambient_color == RED && e.shadow.spot_color == Color::Black);
    assert!(same_rect(e.bounds, 20.0, 30.0, 60.0, 40.0) && e.shadow.alpha == 1.0);
    assert!(matches!(&e.outline, Outline::Rounded(rr) if near(rr.top_left_corner_radius.x, 8.0)));
    // Faded: its alpha (seen through), the shadow outside its layer.
    let faded = look(
        kompozable_lambda!(|| {
            let l = layer(|l| {
                l.alpha = 0.5;
                l.shadow_elevation = 3.0;
            });
            Box(modifier = Modifier.size(40.dp()).graphics_layer_with(l));
        }),
        None,
    );
    assert!(
        faded.elevation_shadows.len() == 1
            && near_by(faded.elevation_shadows[0].shadow.alpha, 0.5, 1e-6)
            && faded.layers == 1
    );
    // Turned in 3D: each point as high as Android's true 3D transform puts it (turned about x, then y, about the
    // pivot: its middle, (40, 30) in its own coordinates), raised by the elevation.
    let tilted = look(
        kompozable_lambda!(|| {
            let l = layer(|l| {
                l.shadow_elevation = 6.0;
                l.rotation_x = 30.0;
                l.rotation_y = 20.0;
            });
            Box(modifier = moved(Modifier, 100, 100).size_wh(80.dp(), 60.dp()).graphics_layer_with(l));
        }),
        None,
    );
    assert_eq!(tilted.elevation_shadows.len(), 1);
    let p = tilted.elevation_shadows[0].shadow.plane;
    let (a, b) = (-(30.0f32.to_radians().cos()) * 20.0f32.to_radians().sin(), 30.0f32.to_radians().sin());
    assert!(near_by(p[0], a, 1e-4) && near_by(p[1], b, 1e-4) && near_by(p[2], 6.0 - a * 40.0 - b * 30.0, 1e-3));
    assert_eq!(tilted.transforms.len(), 1);
    // Off the screen, its shadow on it: drawn (its reach counted); further off: not.
    let edge = |x: i32| {
        let content = kompozable_lambda!(move || {
            let l = layer(|l| l.shadow_elevation = 24.0);
            Box(modifier = moved(Modifier, x, 100).size(40.dp()).graphics_layer_with(l));
        });
        look(content, Some(Rect::new(0.0, 0.0, 400.0, 400.0))).elevation_shadows.len()
    };
    assert!(edge(405) == 1 && edge(560) == 0);
    // A GraphicsLayer's.
    let recorded = look(
        kompozable_lambda!(|| {
            let layer = remember_graphics_layer();
            Box(modifier = Modifier.size(100.dp()).draw_with_content(move |d| {
                d.record_with(&layer, IntSize::new(60, 40), |i| i.draw_rect(BLUE));
                layer.set_shadow_elevation(5.0);
                layer.set_round_rect_outline(Offset::Zero, sz(60.0, 40.0), 6.0);
                d.draw_layer(&layer);
                layer.set_properties(GraphicsLayerProperties::default());
            }));
        }),
        None,
    );
    assert!(recorded.elevation_shadows.len() == 1 && near_by(recorded.elevation_shadows[0].elevation(), 5.0, 1e-6));
    assert!(
        matches!(&recorded.elevation_shadows[0].outline, Outline::Rounded(rr) if near(rr.top_left_corner_radius.x, 6.0))
    );
}

fn graphics_layer_render_effect_draws_through_an_effect_layer() {
    // C++ 31415.
    let rounded = look(
        kompozable_lambda!(|| {
            let l = layer(|l| {
                l.alpha = 0.5;
                l.shape = Arc::new(RoundedCornerShape(8.0f32));
                l.clip = true;
                l.render_effect = Some(BlurEffect(3.0, 3.0, TileMode::Clamp));
            });
            Box(modifier = Modifier.size(40.dp()).graphics_layer_with(l).background(RED));
        }),
        None,
    );
    // Faded and clipped to a rounded shape: the effect's layer at that alpha, cut by a clip around it.
    assert!(rounded.layers == 1 && rounded.layer_effects[0] == Some(BlurEffect(3.0, 3.0, TileMode::Clamp)));
    assert!(
        near(rounded.layer_alphas[0], 0.5)
            && rounded.clip_shapes.len() == 1
            && near(rounded.clip_shapes[0].radii.top_left, 8.0)
    );
    assert_eq!(rounded.layer_corners[0].max(), 0.0);
    // Not clipped: no clip; its outsets give it room.
    let spread = look(
        kompozable_lambda!(|| {
            let l = layer(|l| {
                l.render_effect = Some(OffsetEffect(4.0, 0.0));
                l.outsets = LayerOutsets::new(2.dp(), 2.dp(), 2.dp(), 2.dp());
            });
            Box(modifier = Modifier.size(40.dp()).graphics_layer_with(l).background(RED));
        }),
        None,
    );
    assert!(spread.layers == 1 && spread.layer_effects[0] == Some(OffsetEffect(4.0, 0.0)) && spread.clips.is_empty());
    assert!(spread.clip_shapes.is_empty() && same_rect(spread.layer_bounds[0], -2.0, -2.0, 44.0, 44.0));
    // Off the screen, but blurred into it: drawn.
    let off_screen = look(
        kompozable_lambda!(|| {
            let l = layer(|l| l.render_effect = Some(BlurEffect(20.0, 20.0, TileMode::Decal)));
            Box(modifier = moved(Modifier, -60, 0).size(40.dp()).graphics_layer_with(l).background(RED));
        }),
        Some(Rect::new(0.0, 0.0, 400.0, 400.0)),
    );
    assert_eq!(off_screen.layers, 1);
    // A GraphicsLayer's.
    let recorded = look(
        kompozable_lambda!(|| {
            let layer = remember_graphics_layer();
            Box(modifier = Modifier.size(100.dp()).draw_with_content(move |d| {
                d.record_with(&layer, IntSize::new(60, 40), |i| i.draw_rect(BLUE));
                layer.set_render_effect(Some(BlurEffect(5.0, 5.0, TileMode::Decal)));
                d.draw_layer(&layer);
                layer.set_properties(GraphicsLayerProperties::default());
            }));
        }),
        None,
    );
    assert!(recorded.layers == 1 && recorded.layer_effects[0] == Some(BlurEffect(5.0, 5.0, TileMode::Decal)));
    assert!(same_rect(recorded.layer_bounds[0], 0.0, 0.0, 60.0, 40.0));
}

// ---- Caches, backgrounds, borders, shadows ------------------------------------------------------------------------

fn brushes_paths_and_draw_with_cache_reach_the_canvas() {
    // C++ 9362.
    let builds = Rc::new(Cell::new(0));
    let wide = mutable_state_of(false);
    let (b, w) = (builds.clone(), wide.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (b, width) = (b.clone(), if w.get() { 80.dp() } else { 60.dp() });
        Column(|| {
            let gradient = vgrad(&[Color::Black, Color::White]);
            Box(modifier = Modifier.size(50.dp()).background_brush(gradient, shape = Arc::new(CircleShape)));
            Box(modifier = Modifier.size(50.dp()).background_brush(SolidColor(Color::Black))); // a plain fill
            Box(modifier = Modifier.size_wh(width, 40.dp()).draw_with_cache(move |cache| {
                b.set(b.get() + 1);
                let mut path = Path::new();
                path.move_to(0.0, 0.0);
                path.line_to(cache.size().width, cache.size().height);
                cache.on_draw_behind(move |d| d.draw_path_with(&path, Color::Black, 1.0, stroke(2.0), None, SRC))
            }));
        });
    }));
    let mut first = RecordingCanvas::new();
    rule.host().draw(&mut first);
    rule.host().draw(&mut first);
    assert_eq!(first.gradients, 2); // drawn twice
    assert_eq!(builds.get(), 1); // the cache survives draws
    assert!(first.paths.len() == 2 && same_rect(first.paths[0], 0.0, 100.0, 60.0, 40.0));
    wide.set(true);
    rule.wait_for_idle();
    let second = rule.draw();
    assert_eq!(builds.get(), 2);
    assert!(second.paths.len() == 1 && same_rect(second.paths[0], 0.0, 100.0, 80.0, 40.0));
}

fn shadows_take_colors_and_clip_and_borders_take_brushes() {
    // C++ 13175 (Compose's shadow is an element of its own: not equal to a graphics layer of its properties).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let half_blue = BLUE.copy_alpha(0.5);
        Column(|| {
            let r16 = Arc::new(RoundedCornerShape(16.0f32)) as Arc<dyn Shape>;
            Box(modifier = Modifier.size(100.dp()).shadow(8.dp(), shape = r16.clone())); // black, clipped (Compose's)
            Box(modifier = Modifier.size(100.dp()).shadow(
                8.dp(),
                shape = r16,
                clip = Some(false),
                ambient_color = RED,
                spot_color = RED,
            ));
            Box(modifier = Modifier.size(100.dp()).shadow_with(8.dp(), RectangleShape, true, half_blue, half_blue)); // a plain clip
            Box(modifier =
                Modifier.size(100.dp()).border_stroke_with(BorderStroke(2.dp(), RED), RoundedCornerShape(10.0f32)));
            Box(modifier = Modifier
                .size(100.dp())
                .border_stroke_with(BorderStroke(4.dp(), lgrad(&[RED, BLUE])), RoundedCornerShape(10.0f32)));
        });
    }));
    let c = rule.draw();
    // An elevation shadow each, of its shape, in its colors.
    assert!(c.elevation_shadows.len() == 3 && c.layers == 0);
    let e = &c.elevation_shadows;
    assert!(
        near_by(e[0].elevation(), 8.0, 1e-4)
            && e[0].shadow.ambient_color == Color::Black
            && e[0].shadow.spot_color == Color::Black
    );
    assert!(matches!(&e[0].outline, Outline::Rounded(rr) if near(rr.top_left_corner_radius.x, 16.0)));
    assert!(same_rect(e[0].bounds, 0.0, 0.0, 100.0, 100.0));
    assert!(
        e[1].shadow.ambient_color == RED
            && e[1].shadow.spot_color == RED
            && same_rect(e[1].bounds, 0.0, 100.0, 100.0, 100.0)
    );
    assert!(
        (e[2].shadow.ambient_color.alpha() * 255.0).round() == 128.0 && matches!(e[2].outline, Outline::Rectangle(_))
    );
    assert!(c.clip_shapes.len() == 1 && same_rect(c.clip_shapes[0].bounds, 0.0, 0.0, 100.0, 100.0));
    assert!(near(c.clip_shapes[0].radii.top_left, 16.0));
    assert!(c.clips.len() == 1 && same_rect(c.clips[0], 0.0, 200.0, 100.0, 100.0));
    // No elevation, no clip: nothing.
    assert!(Modifier.shadow(Dp(0.0)) == Modifier);
    // A colored border is a stroke; a brush's a path stroked half its width inside.
    assert!(c.strokes.len() == 1 && c.stroke_colors[0] == RED);
    assert!(c.paths.len() == 1 && same_rect(c.paths[0], 2.0, 402.0, 96.0, 96.0));
}

fn drop_and_inner_shadows_of_shapes() {
    // C++ 29839 (its display list half is the display list's).
    let soft = Shadow::new(rgba(0.0, 0.0, 0.0, 0.4), at(0.0, 6.0), 10.0, 2.0);
    let s = soft.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Column(modifier = Modifier.padding(20.dp()), move || {
            let r12 = RoundedCornerShape(12.0f32);
            let drop = Modifier.size_wh(80.dp(), 40.dp()).drop_shadow(r12.clone(), s.clone());
            Box(modifier = drop.background_with(RED, r12));
            let hard = Shadow::new(Color::Black, at(1.0, 2.0), 4.0, 0.0);
            let inner = Modifier.size_wh(80.dp(), 40.dp()).background(GREEN).inner_shadow(CircleShape, hard);
            Box(modifier = inner, content = || Box(modifier = Modifier.size(10.dp()).background(BLUE)));
            Box(modifier = Modifier.size(30.dp()).drop_shadow(MaterialShapes::Heart().to_shape(0), s.clone()));
        });
    }));
    let c = rule.draw();
    assert!(c.shadows.len() == 2 && c.path_shadows.len() == 1);
    let drop = &c.shadows[0];
    assert!(same_rect(drop.rect, 20.0, 20.0, 80.0, 40.0) && !drop.inner && drop.radii == CornerRadii::all(12.0));
    assert_eq!(drop.shadow, soft);
    let inner = &c.shadows[1];
    // A circle: half the shorter side.
    assert!(
        inner.inner
            && near(inner.rect.top, 60.0)
            && inner.radii == CornerRadii::all(20.0)
            && inner.shadow.blur_radius == 4.0
    );
    // An outline shape: its own outline (inside its 30 px bounds), not a rounded rect.
    let heart = &c.path_shadows[0];
    assert!(!heart.inner && heart.shadow == soft && heart.bounds.left >= 20.0 && heart.bounds.right <= 50.5);
    assert!(heart.bounds.top >= 100.0 && heart.bounds.bottom <= 130.5 && heart.bounds.width() > 20.0);
    // Before what comes after: the drop shadow under its background, the inner one over the background before it
    // and under the content inside.
    assert!(c.order.starts_with("sr") && c.order.contains("rsr"));
}

fn a_changed_shadow_is_a_changed_modifier() {
    // C++ 29876.
    let blur = mutable_state_of(2.0f32);
    let spread = mutable_state_of(0.0f32);
    let (b, s) = (blur.clone(), spread.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let shadow = Shadow::new(Color::Black, Offset::Zero, b.get(), s.get());
        Text("gölgeli", modifier = Modifier.drop_shadow(RectangleShape, shadow));
    }));
    let drawn = || rule.draw().shadows.first().map(|s| s.shadow.clone()).unwrap_or_default();
    assert_eq!(drawn().blur_radius, 2.0);
    blur.set(8.0);
    rule.wait_for_idle();
    assert_eq!(drawn().blur_radius, 8.0);
    spread.set(3.0);
    rule.wait_for_idle();
    assert_eq!(drawn().spread, 3.0);
}

fn drop_shadows_take_brushes_alphas_and_blend_modes() {
    // C++ 31360 (its display list half is the display list's).
    let boxed = |inner: bool, shadow: Shadow| {
        kompozable_lambda!(move || {
            let m = moved(Modifier, 20, 20).size(40.dp());
            let m = if inner {
                m.inner_shadow(RectangleShape, shadow.clone())
            } else {
                m.drop_shadow(RectangleShape, shadow.clone())
            };
            Box(modifier = m);
        })
    };
    // A color: faded by its alpha, its blend mode around it; cut where Compose's bitmap ends (its radius and
    // spread out, moved).
    let plain = Shadow { alpha: 0.5, blend_mode: BlendMode::Multiply, ..Shadow::new(RED, at(3.0, 4.0), 6.0, 2.0) };
    let p = look(boxed(false, plain), None);
    assert!(p.shadows.len() == 1 && (p.shadows[0].shadow.color.alpha() * 255.0).round() == 128.0 && p.layers == 0);
    assert_eq!(p.shadows[0].blend, BlendMode::Multiply);
    assert!(p.clips.len() == 1 && same_rect(p.clips[0], 20.0 - 8.0 + 3.0, 20.0 - 8.0 + 4.0, 56.0, 56.0));
    // A brush: in a layer at its alpha, the shadow (white), then the brush SrcIn over it, over what it covers.
    let brush = lgrad(&[RED, BLUE]);
    let painted_shadow = Shadow { brush: Some(brush.clone()), alpha: 0.5, ..Shadow::new(RED, Offset::Zero, 6.0, 0.0) };
    let painted = look(boxed(false, painted_shadow), None);
    assert!(
        painted.layers == 1
            && near_by(painted.layer_alphas[0], 0.5, 1e-6)
            && same_rect(painted.layer_bounds[0], 14.0, 14.0, 52.0, 52.0)
    );
    assert!(
        painted.shadows.len() == 1
            && painted.shadows[0].shadow.color == Color::White
            && painted.shadows[0].blend == BlendMode::SrcOver
    );
    assert!(
        painted.gradients == 1
            && painted.rect_blends == [BlendMode::SrcIn]
            && same_rect(painted.rects[0], 14.0, 14.0, 52.0, 52.0)
    );
    assert_eq!(painted.order, "sr");
    // Inside: the brush over the shape, not cut.
    let inner_shadow = Shadow { brush: Some(brush), ..Shadow::new(Color::Black, Offset::Zero, 6.0, 0.0) };
    let inner = look(boxed(true, inner_shadow), None);
    assert!(inner.layers == 1 && same_rect(inner.layer_bounds[0], 20.0, 20.0, 40.0, 40.0) && inner.clips.is_empty());
    // A solid brush is its color.
    let solid_shadow = Shadow { brush: Some(SolidColor(BLUE)), ..Shadow::new(Color::Black, Offset::Zero, 6.0, 0.0) };
    let solid = look(boxed(false, solid_shadow), None);
    assert!(solid.layers == 0 && solid.shadows.len() == 1 && solid.shadows[0].shadow.color == BLUE);
    // A canvas told a brushed one can draw it its own way.
    assert!(painted.brushed_shadows.len() == 1 && solid.brushed_shadows.is_empty());
}

// ---- Painters, images, vectors --------------------------------------------------------------------------------------

/// Compose's Icons.Filled.Add: a plus in a 24 x 24 viewport, black.
fn add_icon() -> ImageVector {
    ImageVector::from_svg_path("M19,13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z", 24.0, 24.0)
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn painters_fill_bounded_space_mirror_and_fade() {
    // C++ 10352.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let fill: Rc<dyn Painter> = Rc::new(ColorPainter(RED));
        let own: Rc<dyn Painter> = Rc::new(DrawPainter::new(sz(30.0, 30.0), |d| d.draw_rect(GREEN)));
        Column(|| {
            // No size of its own, not sized by it: as big as it may be (not fixed, but bounded).
            let bounded = Modifier.size_in_with(0.dp(), 0.dp(), 120.dp(), 50.dp()).test_tag("sınırlı");
            Box(modifier = bounded.paint_with(fill, false, Alignment::Center, ContentScale::Inside, 1.0, None));
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
                let plus: Rc<dyn Painter> = Rc::new(VectorPainter::new(add_icon(), Density::default()));
                let m = Modifier.size(48.dp()).test_tag("sağ");
                Box(modifier = m.paint_with(plus, true, Alignment::TopStart, ContentScale::Inside, 1.0, None));
            });
            Image(own, Some("yarı".into()), modifier = Modifier.test_tag("yarı"), alpha = 0.5);
        });
    }));
    let (bounded, right, half) = (bounds(&rule, "sınırlı"), bounds(&rule, "sağ"), bounds(&rule, "yarı"));
    assert!(near(bounded.width(), 120.0) && near(bounded.height(), 50.0));
    let c = rule.draw();
    // TopStart is the top right in a right-to-left layout.
    assert!(c.vector_bounds.iter().any(|v| same_rect(*v, right.left + 24.0, right.top, 24.0, 24.0)));
    // A painter of one's own fades through a layer of its size.
    let faded = (0..c.layer_bounds.len()).any(|i| {
        let b = c.layer_bounds[i];
        same_rect(b, half.left, half.top, half.width(), half.height()) && near(c.layer_alphas[i], 0.5)
    });
    assert!(near(half.width(), 30.0) && faded);
}

fn a_brush_painter_takes_its_color_filter() {
    // C++ 16830's drawing (the filtered brush itself is kompoz-graphics'): painted, the filtered brush. (A gradient
    // to the far corner: a horizontal one's intrinsic size is 0 high, as Compose's, and an Image of it draws
    // nothing.)
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let gradient: Rc<dyn Painter> = Rc::new(BrushPainter(lgrad(&[Color::Black, Color::White])));
        let lit = ColorFilter::lighting(rgba(0.0, 0.0, 0.0, 1.0), rgba(0.0, 0.5, 0.0, 0.0));
        Image(
            gradient,
            Some("degrade".into()),
            modifier = Modifier.size(100.dp()).test_tag("boya"),
            color_filter = Some(lit),
        );
    }));
    let c = rule.draw();
    let green = c
        .rect_colors
        .iter()
        .any(|c| near_by(c.green(), 0.5, 0.02) && near_by(c.red(), 0.0, 0.02) && near_by(c.blue(), 0.0, 0.02));
    assert!(green);
}

fn images_draw_through_a_color_filter() {
    // C++ 29802's drawing (the filtered copies are kompoz-graphics').
    let photo = ImageBitmap::from_pixels(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 128]).unwrap();
    let filter = mutable_state_of(Some(tint(BLUE)));
    let tick = mutable_state_of(0);
    let (p, f, t) = (photo.clone(), filter.clone(), tick.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (photo, cf) = (p.clone(), f.get());
        let _ = t.get();
        Column(|| {
            let m = Modifier.size_wh(20.dp(), 10.dp());
            BitmapImage(
                photo.clone(),
                Some("resim".into()),
                modifier = m,
                content_scale = ContentScale::FillBounds,
                color_filter = cf,
            );
            let lit = Some(ColorFilter::lighting(Color::Black, Color::Transparent));
            Canvas(
                Modifier.size_wh(20.dp(), 10.dp()),
                on_draw = move |d| {
                    let (one, all) = (IntSize::new(2, 1), IntSize::new(20, 10));
                    d.draw_image_rect_with(
                        &photo,
                        IntOffset::Zero,
                        one,
                        IntOffset::Zero,
                        all,
                        1.0,
                        Fill,
                        lit,
                        SRC,
                        DefaultFilterQuality,
                    );
                },
            );
        });
    }));
    // The image itself is drawn, the filter in effect around it (the backend applies it per pixel).
    let c = rule.draw();
    assert!(c.images == [photo.id(), photo.id()]);
    assert!(
        c.image_filters[0] == [tint(BLUE)]
            && c.image_filters[1] == [ColorFilter::lighting(Color::Black, Color::Transparent)]
    );
    assert!(c.filter_stack.is_empty()); // (only around the image)
    tick.set(1); // recomposed: the same
    rule.wait_for_idle();
    let again = rule.draw();
    assert!(again.images == c.images && again.image_filters == c.image_filters);
    filter.set(None); // no filter: none around the image
    rule.wait_for_idle();
    let plain = rule.draw();
    assert!(plain.images[0] == photo.id() && plain.image_filters[0].is_empty());
}

fn test_image(width: i32, height: i32) -> ImageBitmap {
    ImageBitmap::from_pixels(width, height, vec![128; (width * height * 4) as usize]).unwrap()
}

fn image_sizes_follow_the_aspect_ratio() {
    // C++ 3238.
    let photo = test_image(640, 400);
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let photo = photo.clone();
            Column(|| {
                BitmapImage(photo.clone(), Some("natural".into()), modifier = Modifier.test_tag("natural"));
                BitmapImage(photo.clone(), Some("wide".into()), modifier = Modifier.fill_max_width().test_tag("wide"));
                BitmapImage(photo.clone(), Some("fixed".into()), modifier = Modifier.size(100.dp()).test_tag("fixed"));
            });
        }),
        IntSize::new(320, 2000),
    );
    let natural = bounds(&rule, "natural");
    assert!(near(natural.width(), 320.0) && near(natural.height(), 200.0)); // scaled down to fit, aspect kept
    assert!(near(bounds(&rule, "wide").height(), 200.0));
    assert!(near(bounds(&rule, "fixed").height(), 100.0));
}

fn content_scale_places_the_image() {
    // C++ 3255, as Compose draws it: Crop scales the whole image up and cuts it to the bounds (the C++ Kompoz drew
    // the middle of the image into them).
    let photo = test_image(200, 100); // 2:1 into a 100 x 100 box
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let photo = photo.clone();
        Row(|| {
            BitmapImage(
                photo.clone(),
                Some("fit".into()),
                modifier = Modifier.size(100.dp()),
                content_scale = ContentScale::Fit,
            );
            BitmapImage(
                photo.clone(),
                Some("crop".into()),
                modifier = Modifier.size(100.dp()),
                content_scale = ContentScale::Crop,
            );
        });
    }));
    let c = rule.draw();
    assert_eq!(c.images.len(), 2);
    let (fit, crop) = (c.image_destinations[0], c.image_destinations[1]);
    assert!(near(fit.top, 25.0) && near(fit.height(), 50.0) && near(fit.width(), 100.0)); // letterboxed
    assert!(same_rect(crop, 100.0 - 50.0, 0.0, 200.0, 100.0)); // fills, scaled up, centered...
    assert!(c.clips.iter().any(|r| same_rect(*r, 100.0, 0.0, 100.0, 100.0))); // ...and cut to its bounds
}

/// What drawing a vector into `size` px at `density` drew: on the screen, its bitmap, and into the bitmap.
struct VectorDrawing {
    screen: RecordingCanvas,
    bitmap: RecordingCanvas,
    image: Option<ImageBitmap>,
}

fn draw_vector_at(
    v: &ImageVector,
    size: Size,
    density: f32,
    filter: Option<ColorFilter>,
    direction: LayoutDirection,
) -> VectorDrawing {
    let mut screen = RecordingCanvas::new();
    {
        let mut scope = DrawScope::new(&mut screen, size, direction, Density::new(density, 1.0));
        draw_vector(&mut scope, v, 1.0, filter.as_ref());
    }
    let image = screen.drawn_images.last().cloned();
    let mut bitmap = RecordingCanvas::new();
    if let Some(image) = &image {
        for draws in pending_draws(image) {
            draws.replay(&mut bitmap);
        }
    }
    VectorDrawing { screen, bitmap, image }
}

fn simple(v: &ImageVector, size: f32) -> VectorDrawing {
    draw_vector_at(v, sz(size, size), 1.0, None, LayoutDirection::Ltr)
}

fn square(data: &str, fill: Option<Brush>, fill_alpha: f32) -> VectorPath {
    VectorPath { path_data: add_path_nodes(data), fill, fill_alpha, ..VectorPath::default() }
}

fn image_vector_builder_draws_paths_in_their_own_paints() {
    // C++'s image_vector_builder_draws_paths_in_their_own_paints, drawing half (Icon is Material's: a VectorImage
    // tinted by a filter here).
    let (red, blue, green) = (Color::hex(0xC03030), Color::hex(0x3050C0), Color::hex(0x30A030));
    let builder = |name: &str| {
        ImageVector::Builder(name, 48.dp(), 48.dp(), 24.0, 24.0, Color::Unspecified, BlendMode::SrcIn, false)
    };
    let make_two = || {
        builder("iki")
            .add_path_of(square("M0,0h12v12h-12z", Some(SolidColor(red)), 1.0))
            .add_path_of(VectorPath {
                path_data: add_path_nodes("M12,12h12v12h-12z"),
                stroke: Some(SolidColor(blue)),
                stroke_line_width: 2.0,
                ..VectorPath::default()
            })
            .build()
    };
    let two = make_two();
    let small = |tint: Color, blend: BlendMode, mirror: bool| {
        ImageVector::Builder("", 24.dp(), 24.dp(), 24.0, 24.0, tint, blend, mirror)
    };
    let fills = small(Color::Unspecified, BlendMode::SrcIn, false)
        .add_path_of(square("M0,0h12v12h-12z", Some(SolidColor(red)), 1.0))
        .add_path_of(square("M0,12h12v12h-12z", Some(SolidColor(red)), 0.5))
        .build();
    let arrow = small(Color::Unspecified, BlendMode::SrcIn, true)
        .add_path_of(square("M0,0h12v12h-12z", Some(SolidColor(red)), 1.0))
        .build();
    // Equal by what it is, not by its id: the same built twice, equal painters.
    let again = make_two();
    assert!(again == two && again.id() != two.id());
    assert!(VectorPainter::new(again, Density::default()) == VectorPainter::new(two.clone(), Density::default()));
    let rtl = mutable_state_of(false);
    let (t, f, a, r) = (two.clone(), fills.clone(), arrow.clone(), rtl.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (two, fills, arrow) = (t.clone(), f.clone(), a.clone());
        let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        Column(move || {
            VectorImage(two.clone(), Some("iki".into()), modifier = Modifier.test_tag("resim")); // its default size
            let m = Modifier.size(24.dp());
            VectorImage(two.clone(), None, modifier = m.clone().test_tag("ton"), color_filter = Some(tint(green))); // its stroke too
            VectorImage(fills.clone(), None, modifier = m.clone().test_tag("dolgu"), color_filter = Some(tint(green)));
            VectorImage(fills.clone(), None, modifier = m.clone().test_tag("yarı"));
            VectorImage(two.clone(), None, modifier = m.clone().test_tag("süzgeç"), color_filter = Some(tint(blue)));
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], move || {
                VectorImage(arrow.clone(), None, modifier = Modifier.size(24.dp()).test_tag("ok"));
            });
        });
    }));
    let image = bounds(&rule, "resim");
    assert!(near(image.width(), 48.0) && near(image.height(), 48.0));
    let c = rule.draw();
    // Each drawn as the bitmap it was drawn into: one in its own colors as an image (white, as a filter makes it),
    // a one-color one as a mask in its color (or tinted).
    let drawn_as = |tag: &str| {
        let b = bounds(&rule, tag);
        (0..c.vector_bounds.len())
            .find(|&i| near(c.vector_bounds[i].left, b.left) && near(c.vector_bounds[i].top, b.top))
            .map(|i| (c.vector_colors[i], c.vector_bounds[i]))
    };
    assert_eq!(c.vectors.len(), 6);
    assert!(
        c.vectors.iter().filter(|&&v| v == two.id()).count() == 3
            && c.vectors.iter().filter(|&&v| v == fills.id()).count() == 2
    );
    let resim = drawn_as("resim").unwrap();
    assert!(resim.0 == Color::White && near(resim.1.width(), 48.0));
    assert!(drawn_as("ton").unwrap().0 == green && drawn_as("dolgu").unwrap().0 == green);
    let opaque_red = red.copy_alpha(1.0);
    assert!(drawn_as("yarı").unwrap().0 == opaque_red && drawn_as("süzgeç").unwrap().0 == blue);
    assert_eq!(c.mask_colors.len(), 3); // the fills (twice) and the arrow
    // What was drawn into them: in its own paints, twice the viewport (its stroke in the viewport's units).
    let d = simple(&two, 48.0);
    assert!(d.image.as_ref().unwrap().config() == ImageBitmapConfig::Argb8888 && d.bitmap.paths.len() == 2);
    assert!(
        d.bitmap.path_colors[0] == red && near(d.bitmap.path_strokes[0], 0.0) && near(d.bitmap.paths[0].width(), 12.0)
    );
    assert!(
        d.bitmap.path_colors[1] == blue && near(d.bitmap.path_strokes[1], 2.0) && near(d.bitmap.paths[1].left, 12.0)
    );
    let f = simple(&fills, 24.0);
    assert!(f.image.as_ref().unwrap().config() == ImageBitmapConfig::Alpha8 && f.bitmap.paths.len() == 2);
    assert!(near_by(f.bitmap.path_colors[1].alpha(), 0.5, 0.01));
    // Its bitmap whole pixels around its size, just its size within a hundredth of a pixel of it; kept for each
    // size and config (the same one again where they are the same).
    let ltr = LayoutDirection::Ltr;
    let near48 = draw_vector_at(&fills, sz(48.004, 48.004), 2.0, None, ltr);
    let past48 = draw_vector_at(&fills, sz(48.02, 48.02), 2.0, None, ltr);
    let again48 = draw_vector_at(&fills, sz(48.004, 48.004), 2.0, Some(tint(green)), ltr);
    let modulated =
        draw_vector_at(&fills, sz(48.004, 48.004), 2.0, Some(ColorFilter::tint(green, BlendMode::Modulate)), ltr);
    let (n48, p48, a48, m48) =
        (near48.image.unwrap(), past48.image.unwrap(), again48.image.unwrap(), modulated.image.unwrap());
    assert!(n48.width() == 48 && p48.width() == 49 && a48 == n48 && again48.bitmap.paths.len() == 2);
    assert!(m48.config() == ImageBitmapConfig::Argb8888 && m48 != n48); // (a filter a mask can't take)
    // One color however opaque its paths are (the mask holds how much), drawn in that color opaque; a gradient in a
    // group of its own makes it not one color.
    let faint = small(Color::Unspecified, BlendMode::SrcIn, false)
        .add_path_of(square("M0,0h12v12h-12z", Some(SolidColor(red.copy_alpha(0.5))), 1.0))
        .build();
    let mixed = small(Color::Unspecified, BlendMode::SrcIn, false)
        .add_path_of(square("M0,0h12v12h-12z", Some(SolidColor(red)), 1.0))
        .group_of(VectorGroup::default(), |b| {
            b.add_path_of(square("M12,12h4v4z", Some(hgrad(&[Color::Black, Color::White])), 1.0));
        })
        .build();
    let faint_drawn = simple(&faint, 24.0);
    assert!(
        faint_drawn.image.unwrap().config() == ImageBitmapConfig::Alpha8
            && faint_drawn.screen.mask_colors == [opaque_red]
    );
    assert_eq!(simple(&mixed, 24.0).image.unwrap().config(), ImageBitmapConfig::Argb8888);
    // Through the painter's filter, else its own tint (a mask still when it is SrcIn or SrcOver), else its color.
    let tinted = small(blue, BlendMode::SrcOver, false)
        .add_path_of(square("M0,0h12v12h-12z", Some(SolidColor(red)), 1.0))
        .build();
    let multiplied = small(blue, BlendMode::Modulate, false)
        .add_path_of(square("M0,0h12v12h-12z", Some(SolidColor(red)), 1.0))
        .build();
    let own = simple(&tinted, 24.0);
    let painter = draw_vector_at(&tinted, sz(24.0, 24.0), 1.0, Some(tint(green)), ltr);
    let multiply = simple(&multiplied, 24.0);
    assert!(own.image.unwrap().config() == ImageBitmapConfig::Alpha8 && own.screen.mask_colors == [blue]);
    assert_eq!(painter.screen.mask_colors, [green]);
    assert!(multiply.image.unwrap().config() == ImageBitmapConfig::Argb8888);
    assert_eq!(multiply.screen.image_filters, [vec![ColorFilter::tint(blue, BlendMode::Modulate)]]);
    // Mirrored right to left: about its middle (the matrix in the image's coordinates: its top left goes to its
    // right).
    let mirrored = || {
        rule.draw()
            .transforms
            .iter()
            .any(|m| near_by(m.values[Matrix::ScaleX], -1.0, 0.001) && near_by(m.map(Offset::Zero).x, 24.0, 0.01))
    };
    assert!(!mirrored());
    rtl.set(true);
    rule.wait_for_idle();
    assert!(mirrored());
}

fn vector_paths_trim_and_vectors_come_from_composable_content() {
    // C++'s vector_paths_trim_and_vectors_come_from_composable_content (its trim and XML halves are kompoz-graphics').
    let ink = Color::hex(0x2040A0);
    let turn = mutable_state_of(0.0f32);
    let other = mutable_state_of(0);
    let painters: Rc<RefCell<Vec<VectorPainter>>> = Rc::new(RefCell::new(Vec::new()));
    let remembered = Rc::new(Cell::new(0));
    let made = Rc::new(Cell::new(0));
    let (tn, o, ps, rm, md) = (turn.clone(), other.clone(), painters.clone(), remembered.clone(), made.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let _ = o.get();
            let (tn, rm, md) = (tn.clone(), rm.clone(), md.clone());
            // Two in one composable: each its own.
            let red = remember_vector_painter_content(
                24.dp(),
                24.dp(),
                viewport_width = 24.0,
                viewport_height = 24.0,
                content = |_, _| {
                    vector::Path(add_path_nodes("M0,0h4v4h-4z"), fill = Some(SolidColor(Color::hex(0xFF0000))));
                },
            );
            let blue = remember_vector_painter_content(
                24.dp(),
                24.dp(),
                viewport_width = 24.0,
                viewport_height = 24.0,
                content = |_, _| {
                    vector::Path(add_path_nodes("M0,0h4v4h-4z"), fill = Some(SolidColor(Color::hex(0x0000FF))));
                },
            );
            let seen = remember_vector_painter_content(
                48.dp(),
                48.dp(),
                viewport_width = 24.0,
                viewport_height = 24.0,
                content = move |_, _| {
                    let (rm, md) = (rm.clone(), md.clone());
                    vector::Group(
                        rotation = tn.get(),
                        pivot_x = 12.0,
                        pivot_y = 12.0,
                        content = move || {
                            let n = remember(|| {
                                md.set(md.get() + 1);
                                md.get()
                            }); // kept inside it
                            rm.set(n);
                            vector::Path(
                                add_path_nodes("M12,2L12,6"),
                                stroke = Some(SolidColor(ink)),
                                stroke_line_width = 2.0,
                            );
                        },
                    );
                },
            );
            *ps.borrow_mut() = vec![red, blue, seen.clone()];
            Image(Rc::new(seen), Some("dönen".into()), modifier = Modifier.test_tag("dönen"));
        }),
        IntSize::new(100, 100),
    );
    let first = painters.borrow().clone();
    let first_remembered = remembered.get();
    other.set(1);
    rule.wait_for_idle();
    let now = painters.borrow().clone();
    assert!(now[2] == first[2] && remembered.get() == first_remembered); // composed again, made the same
    assert!(now[0] == first[0] && now[1] == first[1] && now[0] != now[1]);
    turn.set(90.0);
    rule.wait_for_idle();
    let now = painters.borrow().clone();
    assert!(now[2] != first[2] && remembered.get() == first_remembered);
    let turned_line = |degrees: f32| {
        let v = ImageVector::Builder(
            vector::RootGroupName,
            48.dp(),
            48.dp(),
            24.0,
            24.0,
            Color::Unspecified,
            BlendMode::SrcIn,
            false,
        )
        .group_of(VectorGroup { rotation: degrees, pivot_x: 12.0, pivot_y: 12.0, ..VectorGroup::default() }, |b| {
            b.add_path_of(VectorPath {
                path_data: add_path_nodes("M12,2L12,6"),
                stroke: Some(SolidColor(ink)),
                stroke_line_width: 2.0,
                ..VectorPath::default()
            });
        })
        .build();
        VectorPainter::new(v, Density::default())
    };
    assert!(now[2] == turned_line(90.0) && now[2] != turned_line(0.0));
    // Its group's matrix now a quarter turn: its line from the right edge in.
    let c = rule.draw();
    let turned = c.drawn_images.iter().any(|image| {
        let mut drawn = RecordingCanvas::new();
        for draws in pending_draws(image) {
            draws.replay(&mut drawn);
        }
        drawn
            .transforms
            .iter()
            .any(|m| near_by(m.map(at(12.0, 2.0)).x, 22.0, 0.001) && near_by(m.map(at(12.0, 2.0)).y, 12.0, 0.001))
    });
    assert!(turned);
    // Its name, tint and mirroring as given; without a viewport, its default size in px; what is given to the
    // content the viewport's size. A group or path outside a vector's content adds nothing.
    let given = Rc::new(Cell::new(Size::Zero));
    let named: Rc<RefCell<Option<VectorPainter>>> = Rc::new(RefCell::new(None));
    let no_viewport: Rc<RefCell<Option<VectorPainter>>> = Rc::new(RefCell::new(None));
    let (g, nm, us) = (given.clone(), named.clone(), no_viewport.clone());
    let _more = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            vector::Path(add_path_nodes("M0,0h1v1z"));
            vector::Group(content = || {});
            let painter = remember_vector_painter_content(
                24.dp(),
                24.dp(),
                viewport_width = 12.0,
                viewport_height = 12.0,
                name = "rozet".to_string(),
                tint_color = Color::Black,
                tint_blend_mode = BlendMode::SrcOver,
                auto_mirror = true,
                content = |_, _| {
                    vector::Group(
                        name = "g".to_string(),
                        content = || {
                            vector::Path(add_path_nodes("M0,0h4v4z"), fill = Some(SolidColor(Color::White)));
                        },
                    );
                },
            );
            *nm.borrow_mut() = Some(painter);
            let (g, us) = (g.clone(), us.clone());
            KompozitionLocalProvider(&[LocalDensity.provides(Density::new(2.0, 1.0))], move || {
                let g = g.clone();
                *us.borrow_mut() =
                    Some(remember_vector_painter_content(24.dp(), 16.dp(), content = move |w, h| g.set(sz(w, h))));
            });
        }),
        IntSize::new(100, 100),
    );
    let expected = ImageVector::Builder("rozet", 24.dp(), 24.dp(), 12.0, 12.0, Color::Black, BlendMode::SrcOver, true)
        .group_of(VectorGroup { name: "g".into(), ..VectorGroup::default() }, |b| {
            b.add_path_of(square("M0,0h4v4z", Some(SolidColor(Color::White)), 1.0));
        })
        .build();
    assert!(named.borrow().as_ref().unwrap().vector == expected);
    let blank = ImageVector::Builder(
        vector::RootGroupName,
        24.dp(),
        16.dp(),
        48.0,
        32.0,
        Color::Unspecified,
        BlendMode::SrcIn,
        false,
    )
    .build();
    assert!(no_viewport.borrow().as_ref().unwrap().vector == blank && given.get() == sz(48.0, 32.0));
}

/// A config giving every value of its own (C++'s `Everything`).
struct Everything;

impl vector::VectorConfig for Everything {
    fn get_float(&self, p: vector::VectorProperty, d: f32) -> f32 {
        use vector::VectorProperty as P;
        match p {
            P::Rotation => 30.0,
            P::PivotX => 1.0,
            P::PivotY => 2.0,
            P::ScaleX => 3.0,
            P::ScaleY => 4.0,
            P::TranslateX => 5.0,
            P::TranslateY => 6.0,
            P::FillAlpha => 0.5,
            P::StrokeLineWidth => 7.0,
            P::StrokeAlpha => 0.25,
            P::TrimPathStart => 0.1,
            P::TrimPathEnd => 0.9,
            P::TrimPathOffset => 0.2,
            _ => d,
        }
    }
    fn get_path_data(&self, _: vector::VectorProperty, _: Vec<PathNode>) -> Vec<PathNode> {
        add_path_nodes("M1,1h2")
    }
    fn get_brush(&self, p: vector::VectorProperty, d: Option<Brush>) -> Option<Brush> {
        if p == vector::VectorProperty::Stroke { Some(SolidColor(Color::hex(0x3050C0))) } else { d }
    }
}

/// A config taking the fill away (C++'s `FillOnly`).
struct NoFill;

impl vector::VectorConfig for NoFill {
    fn get_brush(&self, p: vector::VectorProperty, d: Option<Brush>) -> Option<Brush> {
        if p == vector::VectorProperty::Fill { None } else { d }
    }
}

fn render_vector_group_draws_a_tree_through_configs() {
    // C++'s render_vector_group_draws_a_tree_through_configs.
    let (red, blue) = (Color::hex(0xC03030), Color::hex(0x3050C0));
    let line = VectorPath {
        name: "çizgi".into(),
        path_data: add_path_nodes("M2,2h20"),
        path_fill_type: PathFillType::EvenOdd,
        stroke: Some(SolidColor(red)),
        stroke_line_width: 2.0,
        stroke_line_cap: StrokeCap::Round,
        stroke_line_join: StrokeJoin::Bevel,
        stroke_line_miter: 2.0,
        ..VectorPath::default()
    };
    let spinning = VectorGroup {
        name: "dönen".into(),
        rotation: 10.0,
        pivot_x: 12.0,
        pivot_y: 12.0,
        clip_path_data: add_path_nodes("M0,0h24v24z"),
        ..VectorGroup::default()
    };
    let kare = VectorPath { name: "kare".into(), ..square("M0,0h4v4z", Some(SolidColor(red)), 1.0) };
    let icon = ImageVectorBuilder::new(24.dp(), 24.dp(), 24.0, 24.0)
        .group_of(spinning.clone(), |b| {
            b.add_path_of(line.clone());
        })
        .add_path_of(kare.clone())
        .build();
    let painters: Rc<RefCell<Vec<VectorPainter>>> = Rc::new(RefCell::new(Vec::new()));
    let (ic, ps) = (icon.clone(), painters.clone());
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (root, root2) = (ic.root().clone(), ic.root().clone());
        let plain = remember_vector_painter_content(
            24.dp(),
            24.dp(),
            viewport_width = 24.0,
            viewport_height = 24.0,
            content = move |_, _| {
                vector::RenderVectorGroup(root.clone());
            },
        );
        let configured = remember_vector_painter_content(
            24.dp(),
            24.dp(),
            viewport_width = 24.0,
            viewport_height = 24.0,
            content = move |_, _| {
                let all: Rc<dyn vector::VectorConfig> = Rc::new(Everything);
                let mut configs: std::collections::HashMap<String, Rc<dyn vector::VectorConfig>> =
                    std::collections::HashMap::new();
                configs.insert("dönen".into(), all.clone());
                configs.insert("çizgi".into(), all);
                configs.insert("kare".into(), Rc::new(NoFill));
                vector::RenderVectorGroup(root2.clone(), configs = configs);
            },
        );
        *ps.borrow_mut() = vec![plain, configured];
    }));
    let builder = || {
        ImageVector::Builder(
            vector::RootGroupName,
            24.dp(),
            24.dp(),
            24.0,
            24.0,
            Color::Unspecified,
            BlendMode::SrcIn,
            false,
        )
    };
    let as_is = builder()
        .group_of(spinning, |b| {
            b.add_path_of(line.clone());
        })
        .add_path_of(kare.clone())
        .build();
    let through = builder()
        .group_of(
            VectorGroup {
                name: "dönen".into(),
                rotation: 30.0,
                pivot_x: 1.0,
                pivot_y: 2.0,
                scale_x: 3.0,
                scale_y: 4.0,
                translation_x: 5.0,
                translation_y: 6.0,
                clip_path_data: add_path_nodes("M1,1h2"),
                ..VectorGroup::default()
            },
            |b| {
                b.add_path_of(VectorPath {
                    path_data: add_path_nodes("M1,1h2"),
                    fill_alpha: 0.5,
                    stroke: Some(SolidColor(blue)),
                    stroke_alpha: 0.25,
                    stroke_line_width: 7.0,
                    trim_path_start: 0.1,
                    trim_path_end: 0.9,
                    trim_path_offset: 0.2,
                    ..line.clone()
                });
            },
        )
        .add_path_of(VectorPath { fill: None, ..kare })
        .build();
    let p = painters.borrow();
    assert!(p[0].vector == as_is && p[1].vector == through);
}

/// A renderer that fills what it is asked to draw into white (and counts the draws of the one bitmap it watches).
struct Fake {
    watched: ImageBitmap,
    drawn: RefCell<Vec<usize>>,
}

impl BitmapRenderer for Fake {
    fn draw(&self, bitmap: &ImageBitmap, draws: &Picture) {
        if *bitmap != self.watched {
            return;
        }
        self.drawn.borrow_mut().push(draws.len());
        bitmap.store(&vec![255; (bitmap.width() * bitmap.height() * 4) as usize], true);
    }
}

fn bitmaps_are_drawn_into_by_canvases() {
    // C++'s bitmaps_are_composes, its drawing half (bitmaps themselves are kompoz-graphics', the display list's
    // damage the display list's): drawn into, kept while there is no renderer, drawn by one as it comes; by one there,
    // as the canvas goes; an Alpha8 one drawn as a mask.
    let argb_at = |b: &ImageBitmap, x: i32, y: i32| {
        let p = b.pixels();
        let i = ((y * b.width() + x) * 4) as usize;
        u32::from_be_bytes([p[i + 3], p[i], p[i + 1], p[i + 2]])
    };
    let target = ImageBitmap::new(3, 3, ImageBitmapConfig::Argb8888, true, &colorspace::ColorSpaces::Srgb);
    {
        let mut c = Canvas::from_image(&target);
        c.draw_rect(Rect::new(0.0, 0.0, 1.0, 1.0), &Paint());
        c.draw_circle(at(1.0, 1.0), 1.0, &Paint());
    }
    {
        let _c = Canvas::from_image(&target); // (drew nothing: nothing to do)
    }
    assert!(target.version() == 1 && argb_at(&target, 0, 0) == 0 && pending_draws(&target).len() == 1);
    let fake = Rc::new(Fake { watched: target.clone(), drawn: RefCell::new(Vec::new()) });
    set_bitmap_renderer(Some(fake.clone()));
    assert!(*fake.drawn.borrow() == [2] && target.version() == 2 && argb_at(&target, 0, 0) == 0xFFFFFFFF);
    assert!(pending_draws(&target).is_empty());
    {
        let mut c = Canvas::from_image(&target);
        c.draw_line(Offset::Zero, at(3.0, 3.0), &Paint { stroke_width: 1.0, ..Paint::default() });
    }
    assert!(*fake.drawn.borrow() == [2, 1] && target.version() == 3);
    {
        // A scope over it.
        let mut c = Canvas::from_image(&target);
        let mut over = DrawScope::over_canvas(&mut c, sz(3.0, 3.0), LayoutDirection::Ltr, Density::default());
        over.draw_rect(Color::Black);
    }
    assert_eq!(fake.drawn.borrow().len(), 3);
    set_bitmap_renderer(None);
    // Drawn into while there is no renderer, then let go of: what waits for it doesn't keep it (as C++'s), and a
    // renderer coming finds nothing to draw into it.
    let gone = ImageBitmap::new(2, 2, ImageBitmapConfig::Argb8888, true, &colorspace::ColorSpaces::Srgb);
    {
        let mut c = Canvas::from_image(&gone);
        c.draw_rect(Rect::new(0.0, 0.0, 1.0, 1.0), &Paint());
    }
    let weak = gone.downgrade();
    assert_eq!(pending_draws(&gone).len(), 1);
    drop(gone);
    assert!(weak.upgrade().is_none());
    set_bitmap_renderer(Some(fake.clone()));
    assert_eq!(fake.drawn.borrow().len(), 3);
    set_bitmap_renderer(None);
    // An Alpha8 bitmap drawn as a mask (draw_image_mask): in black from a DrawScope (as Compose's), in a canvas's
    // paint.
    let mask = ImageBitmap::new(4, 4, ImageBitmapConfig::Alpha8, true, &colorspace::ColorSpaces::Srgb);
    let clear = ImageBitmap::new(4, 3, ImageBitmapConfig::Argb8888, true, &colorspace::ColorSpaces::Srgb);
    let mut r = RecordingCanvas::new();
    {
        let mut d = DrawScope::at(&mut r, at(10.0, 10.0), sz(20.0, 20.0), LayoutDirection::Ltr, Density::default());
        d.draw_image_with(&mask, at(2.0, 2.0), 0.5, Fill, None, SRC);
    }
    {
        let mut c = Canvas::new(&mut r, at(10.0, 10.0));
        c.draw_image(&mask, Offset::Zero, &Paint { color: rgba(0.0, 0.0, 1.0, 0.75), ..Paint::default() });
        c.draw_image(&clear, Offset::Zero, &Paint()); // (an Argb8888 one as it is)
    }
    assert!(r.images.len() == 3 && r.mask_colors.len() == 2 && r.rects.is_empty() && r.stack.is_empty());
    assert!(
        r.mask_colors[0] == Color::Black
            && near(r.mask_alphas[0], 0.5)
            && same_rect(r.image_destinations[0], 12.0, 12.0, 4.0, 4.0)
    );
    assert!(r.mask_colors[1] == BLUE && near(r.mask_alphas[1], 0.75) && near(r.image_alphas[2], 1.0));
    // A backend without one of its own draws it as the default does: the brush in a layer, the mask over it by
    // DstIn.
    let mut by_default = RecordingCanvas::new();
    let dst = Rect::new(12.0, 12.0, 16.0, 16.0);
    kompoz_ui::graphics_context::default_draw_image_mask(
        &mut by_default,
        &mask,
        Rect::new(0.0, 0.0, 4.0, 4.0),
        dst,
        &SolidColor(BLUE),
        dst,
        0.75,
        FilterQuality::Low,
    );
    assert!(
        by_default.stack == "LBbl"
            && by_default.rects.len() == 1
            && by_default.images.len() == 1
            && by_default.blend_stack.is_empty()
    );
    assert!(
        by_default.rect_colors[0] == BLUE
            && same_rect(by_default.rects[0], 12.0, 12.0, 4.0, 4.0)
            && near(by_default.layer_alphas[0], 0.75)
    );
    assert!(
        by_default.order == "r"
            && by_default.image_blends[0] == BlendMode::DstIn
            && near(by_default.image_alphas[0], 1.0)
    );
}

fn a_picture_replays_what_was_drawn_into_it() {
    // KompozR's: a recording replays every call, the shape shadows' outlines with them, and a layer's light comes from
    // the context replayed into.
    let mut picture = Picture::new();
    {
        let mut d = DrawScope::new(&mut picture, sz(50.0, 50.0), LayoutDirection::Ltr, Density::default());
        d.draw_rect_with(RED, at(5.0, 5.0), sz(10.0, 10.0), 1.0, Fill, None, BlendMode::Multiply);
        d.clip_rect_with(0.0, 0.0, 20.0, 20.0, ClipOp::Intersect, |i| i.draw_circle(BLUE));
        d.context().draw_shape_shadow(
            Rect::new(0.0, 0.0, 30.0, 30.0),
            &CircleShape,
            LayoutDirection::Ltr,
            &Density::default(),
            &Shadow::new(Color::Black, Offset::Zero, 4.0, 0.0),
            false,
        );
    }
    assert!(!picture.is_empty());
    let mut a = RecordingCanvas::new();
    picture.replay(&mut a);
    let mut b = RecordingCanvas::new();
    picture.replay(&mut b);
    assert!(a.rects == b.rects && a.stack == b.stack && a.rect_blends[0] == BlendMode::Multiply);
    assert!(
        a.rects.len() == 2 && a.clips.len() == 2 && a.shadows.len() == 1 && near(a.shadows[0].radii.top_left, 15.0)
    );
}

fn a_cache_reads_and_records_what_it_builds() {
    // KompozR's: what a cache's build reads builds it again (and nothing else), and a layer recorded while it builds
    // keeps the content, drawn each frame from the cache.
    let color = mutable_state_of(RED);
    let builds = Rc::new(Cell::new(0));
    let (c, b) = (color.clone(), builds.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (c, b) = (c.clone(), b.clone());
        Box(
            modifier = Modifier.size(40.dp()).draw_with_cache(move |cache| {
                b.set(b.get() + 1);
                let paint = c.get();
                let layer = cache.obtain_graphics_layer();
                cache.record(&layer, |d| d.draw_content());
                cache.on_draw_with_content(move |d| {
                    d.draw_rect(paint);
                    d.draw_layer(&layer);
                })
            }),
            content = || Box(modifier = Modifier.size(10.dp()).background(BLUE)),
        );
    }));
    let first = rule.draw();
    assert!(builds.get() == 1 && first.rect_colors == [RED, BLUE]);
    let recompositions = rule.host().stats().recompositions;
    color.set(GREEN);
    rule.wait_for_idle();
    let second = rule.draw();
    assert!(
        builds.get() == 2
            && second.rect_colors == [GREEN, BLUE]
            && rule.host().stats().recompositions == recompositions
    );
}

fn drawn_texts_are_the_texts_drawn() {
    // C++'s rule's drawnTexts; and DrawScope's text calls from where the scope is.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Text("bir");
            Box(modifier = Modifier.size(50.dp()).draw_behind(|d| {
                d.inset_all(5.0, |i| {
                    let text = TextDraw::new("iki", at(0.0, 8.0), 10.0, sz(15.0, 12.5));
                    i.draw_text(&text, BLUE);
                    let area = Rect::new(0.0, 0.0, 15.0, 12.5);
                    i.draw_text_brush(&text, &hgrad(&[RED, BLUE]), area);
                    i.draw_text_shadow(&text, Color::Black, 2.0);
                })
            }));
        });
    }));
    assert_eq!(rule.drawn_texts(), ["bir", "iki", "iki"]);
    let c = rule.draw();
    // (Under the 18 px line of the text.)
    assert!(near(c.baselines[1].x, 5.0) && near(c.baselines[1].y, 18.0 + 5.0 + 8.0) && c.colors[1] == BLUE);
    assert!(c.brush_areas.len() == 1 && same_rect(c.brush_areas[0], 5.0, 23.0, 15.0, 12.5));
    assert!(c.text_order == "ttts" && near(c.shadow_blurs[0], 2.0));
}

tests!(
    canvas_and_draw_modifiers_draw_in_local_coordinates,
    canvas_takes_compose_named_arguments,
    drawing_takes_compose_named_arguments,
    draw_scope_strokes_sit_on_the_outline,
    draw_scope_rotation_and_text,
    paths_take_arcs_and_draw_scopes_clip_to_paths,
    translations_move_drawn_content,
    canvases_draw_as_composes,
    draw_scopes_know_the_density_and_direction,
    paints_turn_anti_aliasing_off,
    vertices_are_drawn_by_canvases,
    path_effects_dash_and_round_strokes,
    draw_scopes_take_composes_parameters,
    stamped_path_effects_are_drawn_filled,
    draw_points_as_dots_lines_or_one_line,
    round_capped_lines_are_drawn_as_paths,
    rounded_clip_is_a_clip_and_alpha_a_layer,
    graphics_layer_records_content_and_draws_it_again,
    graphics_layer_transforms_without_a_layer_and_fades_through_one,
    graphics_layer_takes_compose_named_arguments,
    graphics_layer_matrix_is_androids,
    graphics_layer_goes_offscreen_only_where_it_must,
    scale_and_rotate_are_layers,
    blend_modes_fade_an_offscreen_layer,
    blur_is_a_graphics_layer_with_a_blur_effect,
    modifiers_with_effects_compare_by_them,
    blur_with_a_radius_spec_is_a_layer_as_composes,
    blur_types_blur_the_node_what_it_draws_or_what_is_behind,
    elevation_shadows_go_with_graphics_layers,
    graphics_layer_render_effect_draws_through_an_effect_layer,
    brushes_paths_and_draw_with_cache_reach_the_canvas,
    shadows_take_colors_and_clip_and_borders_take_brushes,
    drop_and_inner_shadows_of_shapes,
    a_changed_shadow_is_a_changed_modifier,
    drop_shadows_take_brushes_alphas_and_blend_modes,
    painters_fill_bounded_space_mirror_and_fade,
    a_brush_painter_takes_its_color_filter,
    images_draw_through_a_color_filter,
    image_sizes_follow_the_aspect_ratio,
    content_scale_places_the_image,
    image_vector_builder_draws_paths_in_their_own_paints,
    vector_paths_trim_and_vectors_come_from_composable_content,
    render_vector_group_draws_a_tree_through_configs,
    bitmaps_are_drawn_into_by_canvases,
    a_picture_replays_what_was_drawn_into_it,
    a_cache_reads_and_records_what_it_builds,
    drawn_texts_are_the_texts_drawn,
);
