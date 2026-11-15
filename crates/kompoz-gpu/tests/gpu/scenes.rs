//! The scenes compared with their pictures in tests/golden (the C++ Kompoz's scenesMatchTheirPictures), drawn
//! through GraphicsContext as C++'s are (directly, and through graphics layers as C++'s detail::drawThroughLayer),
//! its rects (x, y, width, height) as `xywh`, its text as C++'s font system laid it out (see fonts); and what the
//! other tests draw with them. A scene at another px per dp than 1 gives C++'s dp coordinates through `dp`.

use crate::fonts::{Font, text, text_at};
use crate::golden::matches_golden;
use crate::harness::Gpu;
use kompoz_gpu::Renderer;
use kompoz_graphics::{
    BlendMode, BlurEffect, Brush, Color, ColorFilter, ColorMatrix, CornerRadius, DefaultFilterQuality, ImageBitmap,
    Matrix, Offset, Outline, Path, PathDirection, PathFillType, PathStroke, Rect, RenderEffect, RoundRect, Shadow,
    StrokeCap, StrokeJoin, TileMode,
};
use kompoz_ui::draw::{LayerDraw, draw_layer};
use kompoz_ui::graphics_context::{CornerRadii, GraphicsContext, LayerTransform};

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

/// C++'s drawText(text, baseline, font, color): `s` in `font` from `baseline`.
pub fn draw_text(c: &mut dyn GraphicsContext, s: &str, baseline: Offset, font: Font, color: Color) {
    c.draw_text(&text(s, baseline, font), color);
}

pub fn text_scene(c: &mut Renderer) {
    let ink = rgba(0.1, 0.1, 0.1, 1.0);
    draw_text(c, "Kompoz", Offset::new(8.0, 28.0), Font::new(24.0), ink);
    draw_text(c, "İçerik ğüşöç ÇĞİÖŞÜ", Offset::new(8.0, 56.0), Font::new(16.0), ink);
    draw_text(c, "Bold 0123", Offset::new(8.0, 84.0), Font::bold(18.0), rgba(0.6, 0.1, 0.1, 1.0));
    c.draw_text_shadow(&text("Shadow", Offset::new(10.0, 122.0), Font::new(28.0)), rgba(0.0, 0.0, 0.0, 0.6), 4.0);
    draw_text(c, "Shadow", Offset::new(8.0, 120.0), Font::new(28.0), rgba(0.2, 0.3, 0.8, 1.0));
    draw_text(c, "small 11 sp", Offset::new(8.0, 150.0), Font::new(11.0), alpha(ink, 0.7));
    draw_text(c, "spaced", Offset::new(8.0, 176.0), Font::new(14.0).spaced(3.0), ink);
}

/// A 16 x 16 checker of 4 x 4 squares, orange and blue, its right half at half alpha.
pub fn checker() -> ImageBitmap {
    thread_local! {
        // (One image, as C++'s static: its texture made once.)
        static CHECKER: ImageBitmap = {
            let mut px = vec![0u8; 16 * 16 * 4];
            for y in 0..16 {
                for x in 0..16 {
                    let p = &mut px[(y * 16 + x) * 4..][..4];
                    let on = ((x / 4) + (y / 4)) % 2 == 0;
                    p.copy_from_slice(&[
                        if on { 230 } else { 30 },
                        if on { 120 } else { 30 },
                        if on { 20 } else { 160 },
                        if x < 8 { 255 } else { 128 },
                    ]);
                }
            }
            ImageBitmap::from_pixels(16, 16, px).expect("a 16 x 16 image")
        };
    }
    CHECKER.with(ImageBitmap::clone)
}

/// C++'s drawImage(image, src, dst, alpha) (its default quality).
pub fn draw_image(c: &mut dyn GraphicsContext, image: &ImageBitmap, src: Rect, dst: Rect, alpha: f32) {
    c.draw_image(image, src, dst, alpha, DefaultFilterQuality);
}

pub fn images(c: &mut Renderer) {
    let image = checker();
    draw_image(c, &image, xywh(0.0, 0.0, 16.0, 16.0), xywh(8.0, 8.0, 64.0, 64.0), 1.0); // scaled up
    draw_image(c, &image, xywh(0.0, 0.0, 16.0, 16.0), xywh(80.5, 8.5, 16.0, 16.0), 1.0); // fractions
    draw_image(c, &image, xywh(4.0, 4.0, 8.0, 8.0), xywh(110.0, 8.0, 70.0, 70.0), 0.5); // part of it, half alpha
    draw_image(c, &image, xywh(0.0, 0.0, 16.0, 16.0), xywh(8.0, 90.0, 8.0, 8.0), 1.0); // scaled down
}

pub fn layers(c: &mut Renderer) {
    let (red, blue) = (rgba(0.86, 0.15, 0.15, 1.0), rgba(0.15, 0.3, 0.86, 1.0));
    c.push_layer(xywh(8.0, 8.0, 80.0, 60.0), CornerRadii::Zero, 0.5, LayerTransform::Identity); // half alpha over what overlaps inside it
    c.fill_round_rect(xywh(8.0, 8.0, 50.0, 40.0), CornerRadii::Zero, red);
    c.fill_round_rect(xywh(30.0, 20.0, 50.0, 40.0), CornerRadii::Zero, blue);
    c.pop_layer();
    c.push_layer(xywh(100.0, 8.0, 80.0, 60.0), CornerRadii::all(20.0), 1.0, LayerTransform::Identity); // rounded: the content cut to it
    c.fill_round_rect(xywh(90.0, 0.0, 100.0, 80.0), CornerRadii::Zero, blue);
    c.pop_layer();
    let turned = LayerTransform { rotation_z: 30.0, ..LayerTransform::Identity };
    c.push_layer(xywh(20.0, 90.0, 60.0, 40.0), CornerRadii::all(6.0), 0.9, turned);
    c.fill_round_rect(xywh(20.0, 90.0, 60.0, 40.0), CornerRadii::Zero, red);
    draw_text(c, "turn", Offset::new(28.0, 116.0), Font::new(16.0), Color::White);
    c.pop_layer();
    let scaled = LayerTransform { scale_x: 1.4, scale_y: 1.4, translation_x: 4.0, ..LayerTransform::Identity };
    c.push_layer(xywh(110.0, 100.0, 50.0, 40.0), CornerRadii::Zero, 1.0, scaled);
    c.fill_round_rect(xywh(110.0, 100.0, 50.0, 40.0), CornerRadii::all(8.0), blue);
    c.push_layer(xywh(120.0, 110.0, 30.0, 20.0), CornerRadii::all(4.0), 0.5, LayerTransform::Identity); // nested
    c.fill_round_rect(xywh(120.0, 110.0, 30.0, 20.0), CornerRadii::Zero, Color::White);
    c.pop_layer();
    c.pop_layer();
    c.push_clip(xywh(8.0, 150.0, 60.0, 30.0)); // clipped and moved
    c.push_translation(Offset::new(10.0, 5.0));
    c.fill_round_rect(xywh(0.0, 140.0, 80.0, 40.0), CornerRadii::all(10.0), red);
    c.pop_translation();
    c.pop_clip();
}

pub fn outline_layers(c: &mut Renderer) {
    let (blue, orange) = (rgba(0.15, 0.3, 0.86, 1.0), rgba(0.9, 0.6, 0.1, 1.0));
    // (The outline from the bounds' top left.)
    c.push_outline_layer(xywh(2.0, 2.0, 80.0, 80.0), &star(40.0, 40.0, 38.0), 1.0, LayerTransform::Identity);
    c.fill_round_rect(xywh(2.0, 2.0, 80.0, 40.0), CornerRadii::Zero, blue);
    c.fill_round_rect(xywh(2.0, 42.0, 80.0, 40.0), CornerRadii::Zero, orange);
    c.pop_layer();
    let turned = LayerTransform { rotation_z: 20.0, ..LayerTransform::Identity };
    let mut heart = Path::new();
    heart.move_to(40.0, 72.0);
    heart.cubic_to(-10.0, 40.0, 10.0, 0.0, 40.0, 22.0);
    heart.cubic_to(70.0, 0.0, 90.0, 40.0, 40.0, 72.0);
    heart.close();
    c.push_outline_layer(xywh(100.0, 100.0, 80.0, 80.0), &heart, 0.8, turned); // turned, half see-through
    c.fill_round_rect(xywh(100.0, 100.0, 80.0, 80.0), CornerRadii::Zero, rgba(0.8, 0.2, 0.3, 1.0));
    draw_text(c, "love", Offset::new(118.0, 140.0), Font::new(16.0), Color::White);
    c.pop_layer();
}

/// C++'s Brush::linearGradient(colors): corner to corner of what it is drawn over.
pub fn linear(colors: &[Color]) -> Brush {
    Brush::linear_gradient(colors, Offset::Zero, Offset::Infinite, TileMode::Clamp)
}

/// C++'s Brush::horizontalGradient(colors): across all of it.
pub fn horizontal(colors: &[Color]) -> Brush {
    Brush::horizontal_gradient(colors, 0.0, f32::INFINITY, TileMode::Clamp)
}

/// C++'s Brush::verticalGradient(colors): down all of it.
pub fn vertical(colors: &[Color]) -> Brush {
    Brush::vertical_gradient(colors, 0.0, f32::INFINITY, TileMode::Clamp)
}

/// C++'s Brush::radialGradient(colors): from the middle out to half the shorter side.
pub fn radial(colors: &[Color]) -> Brush {
    Brush::radial_gradient(colors, Offset::Unspecified, f32::INFINITY, TileMode::Clamp)
}

pub fn gradients(c: &mut Renderer) {
    let (a, b, d) = (rgba(0.95, 0.45, 0.1, 1.0), rgba(0.3, 0.1, 0.7, 1.0), rgba(0.1, 0.7, 0.6, 1.0));
    let r = xywh(8.0, 8.0, 80.0, 50.0);
    c.fill_gradient(r, CornerRadii::all(12.0), &linear(&[a, b]), r); // corner to corner
    let r = xywh(100.0, 8.0, 84.0, 50.0);
    c.fill_gradient(r, CornerRadii::all(25.0), &Brush::horizontal_gradient(&[a, d, b], 0.0, 30.0, TileMode::Mirror), r);
    let r = xywh(8.0, 66.0, 60.0, 60.0);
    c.fill_gradient(r, CornerRadii::all(30.0), &radial(&[Color::White, a, b]), r);
    let r = xywh(76.0, 66.0, 60.0, 60.0);
    c.fill_gradient(r, CornerRadii::all(8.0), &Brush::sweep_gradient(&[a, d, b, a], Offset::Unspecified), r);
    // A gradient stroke, as a BorderStroke with a brush draws one.
    let r = xywh(146.0, 70.0, 40.0, 52.0);
    c.draw_path(
        &round_rect(r, 12.0),
        Offset::Zero,
        &vertical(&[a, b]),
        r,
        &stroke(4.0, StrokeCap::Butt, StrokeJoin::Miter, 4.0),
    );
    c.draw_text_brush(
        &text("Gradient", Offset::new(8.0, 168.0), Font::bold(30.0)),
        &horizontal(&[a, d, b]),
        xywh(8.0, 140.0, 150.0, 34.0),
    );
    let r = xywh(160.0, 136.0, 24.0, 48.0);
    c.fill_gradient(r, CornerRadii::all(4.0), &vertical(&[alpha(a, 0.0), a]), r); // to see-through
}

/// Through push_transform: turned, skewed, stretched, in perspective.
pub fn transforms(c: &mut Renderer) {
    let image = checker();
    let (a, b, ink) = (rgba(0.95, 0.45, 0.1, 1.0), rgba(0.3, 0.1, 0.7, 1.0), rgba(0.1, 0.1, 0.12, 1.0));
    c.push_transform(&about(50.0, 34.0, |m| m.rotate_z(-20.0))); // turned text, a gradient across it
    c.draw_text_brush(
        &text("Kompoz", Offset::new(6.0, 42.0), Font::bold(22.0)),
        &horizontal(&[a, b]),
        xywh(6.0, 20.0, 88.0, 28.0),
    );
    c.pop_transform();
    let mut skew = Matrix::new(); // x leans with y
    skew.set(1, 0, 0.5);
    skew.translate(-14.0, 0.0, 0.0);
    c.push_transform(&skew);
    c.fill_round_rect(xywh(110.0, 8.0, 40.0, 40.0), CornerRadii::all(8.0), rgba(0.15, 0.5, 0.85, 1.0));
    c.stroke_round_rect(xywh(110.0, 8.0, 40.0, 40.0), CornerRadii::all(8.0), 2.0, ink);
    c.pop_transform();
    c.push_transform(&about(150.0, 80.0, |m| m.scale(2.5, 0.6, 1.0))); // a circle stretched, a line through it
    c.fill_round_rect(xywh(138.0, 68.0, 24.0, 24.0), CornerRadii::all(12.0), rgba(0.1, 0.6, 0.35, 1.0));
    c.draw_line(Offset::new(136.0, 80.0), Offset::new(164.0, 80.0), 1.0, ink);
    c.draw_arc(xywh(140.0, 70.0, 20.0, 20.0), 0.0, 270.0, 2.0, Color::White, true);
    c.pop_transform();
    // A card in perspective: turned about y, seen from in front.
    let card = about(50.0, 130.0, |m| {
        let mut far = Matrix::new(); // the right side further away: w grows to the right
        far.set(0, 3, 0.006);
        far *= &*m;
        *m = far;
        m.rotate_z(4.0);
    });
    c.push_transform(&card);
    c.draw_shadow(
        xywh(14.0, 100.0, 72.0, 60.0),
        CornerRadii::all(10.0),
        &shadow(rgba(0.0, 0.0, 0.0, 0.4), Offset::new(0.0, 4.0), 6.0, 0.0),
        false,
    );
    c.fill_round_rect(xywh(14.0, 100.0, 72.0, 60.0), CornerRadii::all(10.0), Color::White);
    draw_image(c, &image, xywh(0.0, 0.0, 16.0, 16.0), xywh(20.0, 106.0, 24.0, 24.0), 1.0);
    draw_text(c, "card", Offset::new(50.0, 124.0), Font::new(14.0), ink);
    let r = xywh(22.0, 134.0, 60.0, 20.0);
    c.draw_path(&oval(r), Offset::Zero, &linear(&[a, b]), r, &stroke(3.0, StrokeCap::Butt, StrokeJoin::Miter, 4.0));
    c.pop_transform();
    // Clipped to a turned rect, and a layer inside a turn.
    c.push_transform(&about(150.0, 150.0, |m| m.rotate_z(30.0)));
    c.push_clip(xywh(128.0, 128.0, 44.0, 44.0));
    c.fill_gradient(
        xywh(100.0, 100.0, 100.0, 100.0),
        CornerRadii::Zero,
        &radial(&[Color::White, a, b]),
        xywh(120.0, 120.0, 60.0, 60.0),
    );
    c.pop_clip();
    c.push_layer(xywh(134.0, 134.0, 32.0, 32.0), CornerRadii::all(6.0), 0.7, LayerTransform::Identity);
    c.fill_round_rect(xywh(120.0, 140.0, 60.0, 20.0), CornerRadii::Zero, ink);
    c.pop_layer();
    c.pop_transform();
}

/// A layer over `bounds` (its pivot the middle) set as `set` says, `content` drawn through it (C++'s
/// detail::drawThroughLayer).
pub fn through_layer(
    c: &mut dyn GraphicsContext,
    bounds: Rect,
    set: impl FnOnce(&mut LayerDraw),
    mut content: impl FnMut(&mut dyn GraphicsContext),
) {
    let mut l = LayerDraw::plain(bounds);
    set(&mut l);
    draw_layer(c, &l, &mut content);
}

/// A rounded outline of `w` x `h` with corners of `radius`.
fn rounded(w: f32, h: f32, radius: f32) -> Outline {
    Outline::Rounded(RoundRect::from_rect_radius(xywh(0.0, 0.0, w, h), CornerRadius::circular(radius)))
}

/// graphicsLayer's: in 3D, faded, filtered, blended.
pub fn graphics_layers(c: &mut Renderer) {
    let image = checker();
    let (a, b, ink) = (rgba(0.95, 0.45, 0.1, 1.0), rgba(0.3, 0.1, 0.7, 1.0), rgba(0.1, 0.1, 0.12, 1.0));
    let card = |c: &mut dyn GraphicsContext, r: Rect| {
        c.fill_gradient(r, CornerRadii::all(10.0), &linear(&[a, b]), r);
        draw_image(c, &image, xywh(0.0, 0.0, 16.0, 16.0), xywh(r.left + 8.0, r.top + 8.0, 24.0, 24.0), 1.0);
        draw_text(c, "3D", Offset::new(r.left + 40.0, r.top + 28.0), Font::new(16.0), Color::White);
    };
    let r = xywh(12.0, 10.0, 76.0, 56.0);
    through_layer(c, r, |l| l.geometry.rotation_x = 40.0, |c| card(c, r));
    let r = xywh(104.0, 10.0, 76.0, 56.0);
    through_layer(
        c,
        r,
        |l| {
            l.geometry.rotation_y = -45.0;
            l.clip = true;
            l.outline = rounded(76.0, 56.0, 18.0);
        },
        |c| {
            card(c, r);
            c.fill_round_rect(xywh(104.0, 10.0, 30.0, 56.0), CornerRadii::Zero, alpha(ink, 0.4));
        },
    );
    let r = xywh(12.0, 90.0, 76.0, 56.0);
    through_layer(
        c,
        r,
        |l| {
            l.geometry.rotation_z = -12.0;
            l.geometry.scale_x = 0.9;
            l.alpha = 0.6;
        },
        |c| {
            card(c, r);
            c.fill_round_rect(xywh(50.0, 100.0, 40.0, 40.0), CornerRadii::all(20.0), rgba(0.1, 0.6, 0.4, 1.0));
        },
    );
    let mut gray = ColorMatrix::default();
    gray.set_to_saturation(0.0);
    let r = xywh(104.0, 90.0, 76.0, 40.0);
    through_layer(c, r, |l| l.color_filter = Some(ColorFilter::color_matrix(gray)), |c| card(c, r));
    let r = xywh(12.0, 156.0, 168.0, 30.0);
    c.fill_gradient(r, CornerRadii::all(6.0), &horizontal(&[rgba(0.9, 0.9, 0.3, 1.0), rgba(0.3, 0.8, 0.9, 1.0)]), r);
    through_layer(
        c,
        xywh(20.0, 150.0, 152.0, 42.0),
        |l| {
            l.blend_mode = BlendMode::Multiply;
            l.geometry.rotation_x = -30.0;
        },
        |c| {
            c.fill_round_rect(xywh(20.0, 150.0, 152.0, 42.0), CornerRadii::all(21.0), rgba(0.9, 0.3, 0.5, 1.0));
            draw_text(c, "Multiply", Offset::new(64.0, 177.0), Font::new(16.0), Color::White);
        },
    );
}

/// Render effects: a blur's tile modes, wide, moved, cut, turned.
pub fn effects(c: &mut Renderer) {
    let image = checker();
    let (a, b) = (rgba(0.95, 0.45, 0.1, 1.0), rgba(0.3, 0.1, 0.7, 1.0));
    let card = |c: &mut dyn GraphicsContext, r: Rect| {
        c.fill_gradient(r, CornerRadii::all(8.0), &linear(&[a, b]), r);
        draw_image(c, &image, xywh(0.0, 0.0, 16.0, 16.0), xywh(r.left + 6.0, r.top + 6.0, 18.0, 18.0), 1.0);
        c.fill_round_rect(
            xywh(r.right - 16.0, r.top + 4.0, 8.0, r.height() - 8.0),
            CornerRadii::all(4.0),
            Color::White,
        );
    };
    let layer = |c: &mut Renderer, bounds: Rect, set: &dyn Fn(&mut LayerDraw)| {
        through_layer(c, bounds, set, |c| card(c, bounds));
    };
    let tiles = [TileMode::Clamp, TileMode::Decal, TileMode::Repeated, TileMode::Mirror];
    for (i, &tile) in tiles.iter().enumerate() {
        // Each cut to what is around it, to show how its edges go.
        layer(c, xywh(12.0 + i as f32 * 44.0, 12.0, 36.0, 30.0), &|l| {
            l.render_effect = Some(BlurEffect(3.0, 3.0, tile));
            l.clip = true;
            l.outline = Outline::Rectangle(xywh(-4.0, -4.0, 44.0, 38.0));
        });
    }
    layer(c, xywh(16.0, 62.0, 70.0, 40.0), &|l| l.render_effect = Some(BlurEffect(18.0, 6.0, TileMode::Decal)));
    layer(c, xywh(112.0, 62.0, 56.0, 40.0), &|l| {
        let blur = BlurEffect(2.0, 2.0, TileMode::Decal);
        l.render_effect = Some(RenderEffect::offset_effect(Some(&blur), Offset::new(6.0, 6.0)));
        l.alpha = 0.7;
    });
    layer(c, xywh(16.0, 122.0, 70.0, 50.0), &|l| {
        l.render_effect = Some(BlurEffect(5.0, 5.0, TileMode::Clamp));
        l.clip = true;
        l.outline = rounded(70.0, 50.0, 25.0);
    });
    layer(c, xywh(108.0, 122.0, 64.0, 50.0), &|l| {
        l.render_effect = Some(BlurEffect(2.0, 2.0, TileMode::Decal));
        l.geometry.rotation_z = 20.0;
        l.geometry.rotation_x = 30.0;
    });
}

/// C++'s fractionalDensity's px per dp: a phone's.
pub const DENSITY: f32 = 2.625;

/// At 2.625 px per dp, as a phone's (C++'s dp through `dp`).
pub fn density(c: &mut Renderer) {
    let dp = |v: f32| v * DENSITY;
    let r = |x: f32, y: f32, w: f32, h: f32| xywh(dp(x), dp(y), dp(w), dp(h));
    c.fill_round_rect(r(4.0, 4.0, 30.0, 20.0), CornerRadii::all(dp(6.0)), rgba(0.2, 0.5, 0.3, 1.0));
    c.stroke_round_rect(r(38.0, 4.0, 30.0, 20.0), CornerRadii::all(dp(6.0)), dp(1.0), rgba(0.2, 0.2, 0.2, 1.0));
    c.draw_text(&text_at("2.625x", Offset::new(4.0, 44.0), Font::new(14.0), DENSITY), rgba(0.1, 0.1, 0.1, 1.0));
    c.draw_shadow(
        r(4.0, 52.0, 30.0, 14.0),
        CornerRadii::all(dp(4.0)),
        &shadow(rgba(0.0, 0.0, 0.0, 0.4), Offset::new(0.0, dp(2.0)), dp(4.0), 0.0),
        false,
    );
    c.fill_round_rect(r(4.0, 52.0, 30.0, 14.0), CornerRadii::all(dp(4.0)), Color::White);
    c.draw_line(Offset::new(dp(40.0), dp(52.0)), Offset::new(dp(68.0), dp(66.0)), dp(1.0), rgba(0.6, 0.1, 0.1, 1.0));
}

/// What transforms_carry_everything and the other tests draw: one of nearly everything (at `k` px per dp; a shadow
/// if `with_shadow`).
pub fn mixed(c: &mut Renderer, k: f32, with_shadow: bool) {
    let image = checker();
    let dp = |v: f32| v * k;
    let r = |x: f32, y: f32, w: f32, h: f32| xywh(dp(x), dp(y), dp(w), dp(h));
    let o = |x: f32, y: f32| Offset::new(dp(x), dp(y));
    let (red, blue, green, ink) =
        (rgba(0.86, 0.15, 0.15, 1.0), rgba(0.15, 0.3, 0.86, 1.0), rgba(0.1, 0.55, 0.3, 1.0), rgba(0.1, 0.1, 0.12, 1.0));
    if with_shadow {
        let s = shadow(rgba(0.0, 0.0, 0.0, 0.5), o(0.0, 3.0), dp(5.0), 0.0);
        c.draw_shadow(r(52.0, 4.0, 36.0, 20.0), CornerRadii::all(dp(6.0)), &s, false);
    }
    c.fill_gradient(r(6.0, 6.0, 38.0, 22.0), CornerRadii::all(dp(6.0)), &linear(&[red, blue]), r(6.0, 6.0, 38.0, 22.0));
    c.stroke_round_rect(r(52.0, 4.0, 36.0, 20.0), CornerRadii::all(dp(8.0)), dp(3.0), green);
    c.draw_line(o(8.0, 36.0), o(44.0, 48.0), dp(3.0), ink);
    c.draw_arc(r(54.0, 32.0, 30.0, 30.0), -60.0, 200.0, dp(4.0), rgba(0.5, 0.2, 0.6, 1.0), true);
    c.draw_text_brush(
        &text_at("Kompoz", Offset::new(6.0, 92.0), Font::new(13.0), k),
        &horizontal(&[ink, red]),
        r(6.0, 80.0, 50.0, 14.0),
    );
    draw_image(c, &image, xywh(0.0, 0.0, 16.0, 16.0), r(66.0, 72.0, 16.0, 16.0), 1.0);
    let s = star(dp(26.0), dp(64.0), dp(13.0));
    c.draw_path(&s, Offset::Zero, &radial(&[Color::White, blue]), r(13.0, 51.0, 26.0, 26.0), &fill());
    let mut curve = Path::new();
    curve.move_to(dp(40.0), dp(70.0));
    curve.cubic_to(dp(50.0), dp(50.0), dp(60.0), dp(90.0), dp(70.0), dp(60.0));
    c.draw_path(
        &curve,
        Offset::Zero,
        &Brush::from(red),
        Rect::Zero,
        &stroke(dp(2.5), StrokeCap::Round, StrokeJoin::Round, 4.0),
    );
    c.push_layer(r(42.0, 76.0, 22.0, 16.0), CornerRadii::all(dp(4.0)), 0.6, LayerTransform::Identity);
    c.fill_round_rect(r(36.0, 70.0, 40.0, 30.0), CornerRadii::Zero, rgba(0.95, 0.6, 0.1, 1.0));
    c.pop_layer();
    c.push_clip(r(70.0, 40.0, 20.0, 14.0));
    c.fill_round_rect(r(60.0, 30.0, 40.0, 40.0), CornerRadii::all(dp(10.0)), rgba(0.0, 0.5, 0.5, 1.0));
    c.pop_clip();
    c.push_blend_mode(BlendMode::Multiply); // (read from the target, under where it goes)
    c.fill_round_rect(r(30.0, 20.0, 20.0, 14.0), CornerRadii::Zero, rgba(0.9, 0.8, 0.2, 1.0));
    c.pop_blend_mode();
}

/// The scenes against their pictures (C++'s scenesMatchTheirPictures), each with its size.
pub const SCENES: [(&str, i32, i32, fn(&mut Renderer)); 14] = [
    ("rects", 192, 192, rects),
    ("lines", 192, 192, lines),
    ("shadows", 192, 192, shadows),
    ("text", 192, 192, text_scene),
    ("images", 192, 192, images),
    ("layers", 192, 192, layers),
    ("paths", 192, 192, paths),
    ("outline-layers", 192, 192, outline_layers),
    ("gradients", 192, 192, gradients),
    ("blend-modes", 192, 192, blend_modes),
    ("transforms", 192, 192, transforms),
    ("graphics-layers", 192, 196, graphics_layers),
    ("effects", 192, 192, effects),
    ("density", 192, 192, density),
];

pub fn scenes_match_their_pictures(g: &mut Gpu) {
    for (name, w, h, draw) in SCENES {
        let p = g.render(w, h, draw);
        matches_golden(name, &p);
    }
}
