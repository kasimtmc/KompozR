//! The scenes compared with their pictures in tests/golden (the C++ Kompoz's scenesMatchTheirPictures), drawn
//! through GraphicsContext as C++'s are, its rects (x, y, width, height) as `xywh`; and what the other tests draw
//! with them.

use crate::golden::matches_golden;
use crate::harness::Gpu;
use kompoz_gpu::Renderer;
use kompoz_graphics::{
    BlendMode, Brush, Color, CornerRadius, Matrix, Offset, Path, PathDirection, PathFillType, PathStroke, Rect,
    RoundRect, Shadow, StrokeCap, StrokeJoin, TileMode,
};
use kompoz_ui::graphics_context::{CornerRadii, GraphicsContext};

/// A rect as C++'s are given: its top left and size.
pub fn xywh(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x, y, x + w, y + h)
}

/// An sRGB color (C++'s Color{r, g, b, a}).
pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color::new(r, g, b, a)
}

/// The same color at another alpha (C++'s copy(alpha)).
pub fn alpha(c: Color, a: f32) -> Color {
    Color::new(c.red(), c.green(), c.blue(), a)
}

/// A stroke `width` wide with its cap, join and miter limit (C++'s PathStroke{width, cap, join, miter}).
pub fn stroke(width: f32, cap: StrokeCap, join: StrokeJoin, miter: f32) -> PathStroke {
    PathStroke { width, cap, join, miter, ..PathStroke::default() }
}

/// A fill.
pub fn fill() -> PathStroke {
    PathStroke::default()
}

/// A shadow of `color`, moved, blurred and spread (C++'s Shadow{color, offset, blurRadius, spread}).
pub fn shadow(color: Color, offset: Offset, blur_radius: f32, spread: f32) -> Shadow {
    Shadow { color, offset, blur_radius, spread, ..Shadow::None }
}

/// The blend modes in C++'s order (its BlendMode's), as the swatches of the blend-modes picture go.
pub const BLEND_MODES: [BlendMode; 29] = [
    BlendMode::Src,
    BlendMode::SrcOver,
    BlendMode::SrcIn,
    BlendMode::SrcAtop,
    BlendMode::DstIn,
    BlendMode::Modulate,
    BlendMode::Multiply,
    BlendMode::Screen,
    BlendMode::Darken,
    BlendMode::Lighten,
    BlendMode::Plus,
    BlendMode::Clear,
    BlendMode::Dst,
    BlendMode::DstOver,
    BlendMode::SrcOut,
    BlendMode::DstOut,
    BlendMode::DstAtop,
    BlendMode::Xor,
    BlendMode::Overlay,
    BlendMode::ColorDodge,
    BlendMode::ColorBurn,
    BlendMode::Hardlight,
    BlendMode::Softlight,
    BlendMode::Difference,
    BlendMode::Exclusion,
    BlendMode::Hue,
    BlendMode::Saturation,
    BlendMode::Color,
    BlendMode::Luminosity,
];

/// A transform about (cx, cy): `change` made there.
pub fn about(cx: f32, cy: f32, change: impl FnOnce(&mut Matrix)) -> Matrix {
    let mut m = Matrix::new(); // (each step before the one above it)
    m.translate(cx, cy, 0.0);
    change(&mut m);
    m.translate(-cx, -cy, 0.0);
    m
}

/// Five points, drawn in one go: it crosses itself.
pub fn star(cx: f32, cy: f32, r: f32) -> Path {
    let mut p = Path::new();
    for i in 0..5 {
        let a = -std::f32::consts::PI / 2.0 + i as f32 * 4.0 * std::f32::consts::PI / 5.0;
        if i == 0 {
            p.move_to(cx + r * a.cos(), cy + r * a.sin());
        } else {
            p.line_to(cx + r * a.cos(), cy + r * a.sin());
        }
    }
    p.close();
    p
}

/// An oval's path (counterclockwise, as C++'s addOval).
pub fn oval(r: Rect) -> Path {
    let mut p = Path::new();
    p.add_oval(r, PathDirection::CounterClockwise);
    p
}

/// A rounded rect's path, its corners of one radius (C++'s addRoundRect(rect, radius)).
pub fn round_rect(r: Rect, radius: f32) -> Path {
    let mut p = Path::new();
    p.add_round_rect(&RoundRect::from_rect_radius(r, CornerRadius::circular(radius)), PathDirection::CounterClockwise);
    p
}

pub fn rects(c: &mut Renderer) {
    let (red, blue) = (rgba(0.86, 0.15, 0.15, 1.0), rgba(0.15, 0.3, 0.86, 1.0));
    c.fill_round_rect(xywh(8.0, 8.0, 40.0, 30.0), CornerRadii::Zero, red); // square, whole pixels
    c.fill_round_rect(xywh(56.5, 8.25, 40.0, 30.0), CornerRadii::Zero, blue); // square, fractions
    c.fill_round_rect(xywh(104.0, 8.0, 40.0, 30.0), CornerRadii::all(8.0), red); // rounded
    c.fill_round_rect(xywh(152.4, 8.6, 32.0, 30.0), CornerRadii::new(16.0, 2.0, 10.0, 0.0), blue); // corners of their own
    c.fill_round_rect(xywh(8.0, 48.0, 60.0, 60.0), CornerRadii::all(30.0), alpha(red, 0.5)); // a circle, half alpha
    c.fill_round_rect(xywh(40.0, 70.0, 60.0, 60.0), CornerRadii::all(30.0), alpha(blue, 0.5)); // overlapping
    c.stroke_round_rect(xywh(110.0, 48.0, 70.0, 40.0), CornerRadii::all(10.0), 1.0, red); // hairline-ish
    c.stroke_round_rect(xywh(110.0, 96.0, 70.0, 40.0), CornerRadii::all(14.0), 6.0, blue); // thick
    c.stroke_round_rect(xywh(8.0, 140.0, 60.0, 40.0), CornerRadii::Zero, 0.5, red); // thinner than a pixel
    c.stroke_round_rect(xywh(80.3, 140.3, 60.0, 40.0), CornerRadii::all(4.0), 2.5, blue); // fractions
}

pub fn lines(c: &mut Renderer) {
    let ink = rgba(0.1, 0.1, 0.1, 1.0);
    for i in 0..6 {
        let angle = i as f32 * 18.0 * std::f32::consts::PI / 180.0;
        let w = 0.5 + i as f32 * 1.1;
        let y = 20.0 + i as f32 * 8.0;
        c.draw_line(Offset::new(20.0, y), Offset::new(20.0 + 150.0 * angle.cos(), y + 150.0 * angle.sin()), w, ink);
    }
    let teal = rgba(0.0, 0.5, 0.5, 1.0);
    c.draw_arc(xywh(20.0, 110.0, 70.0, 70.0), -90.0, 270.0, 6.0, teal, false);
    c.draw_arc(xywh(110.0, 110.0, 70.0, 70.0), 30.0, 200.0, 8.0, alpha(teal, 0.7), true);
    c.draw_arc(xywh(125.0, 125.0, 40.0, 40.0), 0.0, 360.0, 2.0, ink, false);
}

pub fn shadows(c: &mut Renderer) {
    let shade = rgba(0.0, 0.0, 0.0, 0.5);
    let white = Color::White;
    c.fill_round_rect(xywh(0.0, 0.0, 192.0, 192.0), CornerRadii::Zero, rgba(0.95, 0.95, 0.97, 1.0));
    c.draw_shadow(
        xywh(20.0, 20.0, 60.0, 40.0),
        CornerRadii::all(8.0),
        &shadow(shade, Offset::new(0.0, 4.0), 8.0, 0.0),
        false,
    );
    c.fill_round_rect(xywh(20.0, 20.0, 60.0, 40.0), CornerRadii::all(8.0), white);
    // No blur, spread.
    c.draw_shadow(
        xywh(110.0, 20.0, 60.0, 40.0),
        CornerRadii::all(20.0),
        &shadow(shade, Offset::new(4.0, 4.0), 0.0, 2.0),
        false,
    );
    c.fill_round_rect(xywh(110.0, 20.0, 60.0, 40.0), CornerRadii::all(20.0), white);
    let reddish = shadow(rgba(0.8, 0.2, 0.2, 0.8), Offset::Zero, 16.0, 4.0);
    c.draw_shadow(xywh(20.0, 100.0, 60.0, 60.0), CornerRadii::all(30.0), &reddish, false);
    c.fill_round_rect(xywh(20.0, 100.0, 60.0, 60.0), CornerRadii::all(30.0), white);
    c.fill_round_rect(xywh(110.0, 100.0, 60.0, 60.0), CornerRadii::all(12.0), white);
    // Inner.
    c.draw_shadow(
        xywh(110.0, 100.0, 60.0, 60.0),
        CornerRadii::all(12.0),
        &shadow(shade, Offset::new(3.0, 3.0), 6.0, 2.0),
        true,
    );
}

pub fn paths(c: &mut Renderer) {
    let (ink, green, red) = (rgba(0.15, 0.15, 0.2, 1.0), rgba(0.1, 0.55, 0.3, 1.0), rgba(0.8, 0.2, 0.2, 1.0));
    let ink_brush = Brush::from(ink);
    let none = Rect::Zero;
    let s1 = star(30.0, 32.0, 26.0);
    c.draw_path(&s1, Offset::Zero, &Brush::from(green), none, &fill()); // non-zero: the middle filled
    let mut s2 = star(90.0, 32.0, 26.0);
    s2.fill_type = PathFillType::EvenOdd;
    c.draw_path(&s2, Offset::Zero, &Brush::from(green), none, &fill()); // even-odd: the middle left out
    let circle = oval(xywh(126.0, 8.0, 52.0, 48.0));
    c.draw_path(&circle, Offset::Zero, &Brush::from(alpha(red, 0.6)), none, &fill());
    // Strokes: caps (butt, round, square), joins (miter, round, bevel), a miter past its limit.
    let caps = [StrokeCap::Butt, StrokeCap::Round, StrokeCap::Square];
    let joins = [StrokeJoin::Miter, StrokeJoin::Round, StrokeJoin::Bevel];
    for i in 0..3 {
        let x = i as f32 * 60.0;
        let mut z = Path::new();
        z.move_to(14.0 + x, 112.0);
        z.line_to(34.0 + x, 76.0);
        z.line_to(54.0 + x, 112.0);
        c.draw_path(&z, Offset::Zero, &ink_brush, none, &stroke(8.0, caps[i], joins[i], 4.0));
    }
    let mut sharp = Path::new(); // too sharp for a miter limit of 2: beveled
    sharp.move_to(10.0, 150.0);
    sharp.line_to(60.0, 140.0);
    sharp.line_to(10.0, 130.0);
    c.draw_path(&sharp, Offset::Zero, &Brush::from(red), none, &stroke(6.0, StrokeCap::Butt, StrokeJoin::Miter, 2.0));
    let mut curve = Path::new();
    curve.move_to(72.0, 180.0);
    curve.cubic_to(90.0, 120.0, 120.0, 200.0, 140.0, 130.0);
    c.draw_path(&curve, Offset::Zero, &ink_brush, none, &stroke(3.0, StrokeCap::Round, StrokeJoin::Round, 4.0));
    let mut thin = Path::new();
    thin.move_to(150.0, 130.0);
    thin.quadratic_to(185.0, 140.0, 160.0, 186.0);
    c.draw_path(&thin, Offset::Zero, &ink_brush, none, &stroke(0.5, StrokeCap::Butt, StrokeJoin::Miter, 4.0));
    let mut dot = Path::new(); // a contour that goes nowhere: a dot for round caps
    dot.move_to(176.0, 120.0);
    dot.line_to(176.0, 120.0);
    c.draw_path(&dot, Offset::Zero, &Brush::from(red), none, &stroke(10.0, StrokeCap::Round, StrokeJoin::Round, 4.0));
    let ring = round_rect(xywh(100.0, 140.0, 40.0, 40.0), 10.0);
    c.draw_path(&ring, Offset::Zero, &Brush::from(green), none, &stroke(4.0, StrokeCap::Butt, StrokeJoin::Miter, 4.0));
}

/// Every mode, a swatch each: an orange square over a blue to green ring.
pub fn blend_modes(c: &mut Renderer) {
    for (m, &mode) in BLEND_MODES.iter().enumerate() {
        let (x, y) = ((m % 6) as f32 * 32.0 + 2.0, (m / 6) as f32 * 38.0 + 2.0);
        let ring = Brush::linear_gradient(
            &[rgba(0.1, 0.3, 0.9, 1.0), rgba(0.2, 0.8, 0.4, 0.6)],
            Offset::Zero,
            Offset::Infinite,
            TileMode::Clamp,
        );
        c.fill_gradient(xywh(x, y, 28.0, 28.0), CornerRadii::all(14.0), &ring, xywh(x, y, 28.0, 28.0));
        c.push_blend_mode(mode);
        c.fill_round_rect(xywh(x + 8.0, y + 8.0, 18.0, 18.0), CornerRadii::all(3.0), rgba(0.95, 0.55, 0.1, 0.85));
        c.pop_blend_mode();
    }
}

/// The scenes against their pictures: those drawn by what the renderer's core draws (rects, lines, shadows of
/// rounded rects, paths, blend modes). The others' (text, images, layers, gradients, transforms, effects and
/// density) come with the parts that draw them.
pub fn scenes_match_their_pictures(g: &mut Gpu) {
    let scenes: [(&str, fn(&mut Renderer)); 5] =
        [("rects", rects), ("lines", lines), ("shadows", shadows), ("paths", paths), ("blend-modes", blend_modes)];
    for (name, draw) in scenes {
        let p = g.render(192, 192, draw);
        matches_golden(name, &p);
    }
}
