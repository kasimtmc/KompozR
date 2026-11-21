//! Brushes, shaders, blend modes, color matrices and filters, vertices and render effects (the C++ Kompoz's tests
//! about their values, with Compose's behaviour where the C++ simplified it).

use kompoz_graphics::colorspace::ColorSpaces;
use kompoz_graphics::*;
use kompoz_runtime::kompozable_block;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---- Helpers ------------------------------------------------------------------------------------------------------

fn o(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() < tolerance
}

/// A rect from its left, top, width and height (as the C++ wrote them).
fn xywh(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x, y, x + w, y + h)
}

fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color::new(r, g, b, a)
}

fn with_alpha(c: Color, alpha: f32) -> Color {
    kompozable_block!(c.copy(alpha = alpha))
}

/// Each component along in sRGB, as a gradient goes.
fn along_gradient(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        lerp(a.red(), b.red(), t),
        lerp(a.green(), b.green(), t),
        lerp(a.blue(), b.blue(), t),
        lerp(a.alpha(), b.alpha(), t),
    )
}

/// Within a hundredth each.
fn same(p: Color, q: Color) -> bool {
    near(p.red(), q.red(), 0.01)
        && near(p.green(), q.green(), 0.01)
        && near(p.blue(), q.blue(), 0.01)
        && near(p.alpha(), q.alpha(), 0.01)
}

/// Within two of 8 bits (the colors in and out are of 8 bits a channel).
fn near_color(c: Color, want: Color) -> bool {
    let e = 2.0 / 255.0;
    near(c.red(), want.red(), e)
        && near(c.green(), want.green(), e)
        && near(c.blue(), want.blue(), e)
        && near(c.alpha(), want.alpha(), e)
}

fn panics(f: impl FnOnce()) -> bool {
    catch_unwind(AssertUnwindSafe(f)).is_err()
}

const RED: Color = Color::new(1.0, 0.0, 0.0, 1.0);
const GREEN: Color = Color::new(0.0, 1.0, 0.0, 1.0);
const BLUE: Color = Color::new(0.0, 0.0, 1.0, 1.0);
const INF: f32 = f32::INFINITY;
const CLAMP: TileMode = TileMode::Clamp;

fn horizontal(colors: &[Color]) -> Brush {
    Brush::horizontal_gradient(colors, 0.0, INF, CLAMP)
}

// ---- Gradients -----------------------------------------------------------------------------------------------

#[test]
fn brush_gradients_interpolate_in_the_painted_area() {
    let (red, blue, green) = (Color::hex(0xFF0000), Color::hex(0x0000FF), Color::hex(0x00FF00));
    let area = xywh(100.0, 50.0, 200.0, 100.0);
    let h = horizontal(&[red, blue]);
    assert!(same(h.color_at(o(100.0, 60.0), area), red));
    assert!(same(h.color_at(o(200.0, 60.0), area), along_gradient(red, blue, 0.5)));
    assert!(same(h.color_at(o(500.0, 60.0), area), blue)); // clamped past the end
    let v = Brush::vertical_gradient(&[red, green, blue], 0.0, 100.0, CLAMP);
    assert!(same(v.color_at(o(0.0, 100.0), area), green));
    let d = Brush::linear_gradient(&[red, blue], Offset::Zero, Offset::Infinite, CLAMP); // to the bottom right
    assert!(same(d.color_at(o(300.0, 150.0), area), blue));
    let radial = Brush::radial_gradient(&[red, blue], Offset::Unspecified, INF, CLAMP); // centered, radius 50
    assert!(same(radial.color_at(o(200.0, 100.0), area), red));
    assert!(same(radial.color_at(o(200.0, 125.0), area), along_gradient(red, blue, 0.5)));
    let sweep = Brush::sweep_gradient(&[red, blue], Offset::Unspecified);
    assert!(same(sweep.color_at(o(250.0, 100.01), area), red)); // 3 o'clock
    assert!(same(sweep.color_at(o(100.0, 100.01), area), along_gradient(red, blue, 0.5))); // 9 o'clock: half way
    let stops =
        Brush::linear_gradient_stops(&[(0.0, red), (0.2, green), (1.0, blue)], o(0.0, 0.0), o(100.0, 0.0), CLAMP);
    assert!(same(stops.color_at(o(110.0, 0.0), area), along_gradient(red, green, 0.5)));
    assert!(same(stops.color_at(o(160.0, 0.0), area), along_gradient(green, blue, 0.5)));
    assert!(SolidColor(red).is_solid() && SolidColor(red) == SolidColor(red) && h != v);
    assert_eq!(Brush::from(red), SolidColor(red));
}

#[test]
fn a_linear_gradient_of_a_list_is_the_slices() {
    // (WP5) Kotlin's `Brush.linearGradient(colors: List<Color>, …)`: the slice's gradient.
    let (red, blue) = (Color::hex(0xFF0000), Color::hex(0x0000FF));
    assert_eq!(
        Brush::linear_gradient_of(vec![red, blue], Offset::Zero, Offset::Infinite, CLAMP),
        Brush::linear_gradient(&[red, blue], Offset::Zero, Offset::Infinite, CLAMP)
    );
}

#[test]
fn brush_geometry_resolves_on_the_area() {
    let (red, blue) = (Color::hex(0xFF0000), Color::hex(0x0000FF));
    let area = xywh(100.0, 50.0, 200.0, 100.0);
    let at = |a: Offset, b: Offset| near(a.x, b.x, 1e-4) && near(a.y, b.y, 1e-4);
    // to the far corner
    let d = Brush::linear_gradient(&[red, blue], Offset::Zero, Offset::Infinite, CLAMP).geometry(area);
    assert!(at(d.start, o(100.0, 50.0)) && at(d.end, o(300.0, 150.0)));
    let h = Brush::horizontal_gradient(&[red, blue], 20.0, INF, CLAMP).geometry(area); // on to the far edge
    assert!(at(h.start, o(120.0, 50.0)) && at(h.end, o(300.0, 50.0)));
    let r = Brush::radial_gradient(&[red, blue], Offset::Unspecified, INF, CLAMP).geometry(area); // the nearer edge
    assert!(at(r.start, o(200.0, 100.0)) && r.radius == 50.0);
    let own = Brush::radial_gradient(&[red, blue], o(10.0, 20.0), 30.0, CLAMP).geometry(area);
    assert!(at(own.start, o(110.0, 70.0)) && own.radius == 30.0);
    let s = Brush::sweep_gradient(&[red, blue], Offset::Unspecified).geometry(area);
    assert!(at(s.start, o(200.0, 100.0)));
    assert_eq!(
        Brush::radial_gradient(&[red, blue], Offset::Unspecified, INF, CLAMP).parameter_at(o(200.0, 125.0), area),
        0.5
    );
    assert_eq!(Brush::horizontal_gradient(&[red, blue], 20.0, INF, CLAMP).parameter_at(o(210.0, 0.0), area), 0.5);
}

#[test]
fn gradient_tile_modes_go_on_past_the_ends() {
    let (a, b) = (RED, BLUE);
    // 0..10 from red to blue, painting 0..100
    let at = |mode: TileMode, x: f32| {
        Brush::horizontal_gradient(&[a, b], 0.0, 10.0, mode).color_at(o(x, 5.0), xywh(0.0, 0.0, 100.0, 10.0))
    };
    let quarter = along_gradient(a, b, 0.25);
    assert!(same(at(TileMode::Clamp, 2.5), quarter) && same(at(TileMode::Clamp, 32.5), b));
    assert!(same(at(TileMode::Clamp, -5.0), a));
    assert!(same(at(TileMode::Repeated, 12.5), quarter) && same(at(TileMode::Repeated, 32.5), quarter));
    assert!(same(at(TileMode::Repeated, -7.5), quarter));
    assert!(same(at(TileMode::Mirror, 12.5), along_gradient(a, b, 0.75)) && same(at(TileMode::Mirror, 22.5), quarter));
    assert!(same(at(TileMode::Mirror, -2.5), quarter));
    assert!(same(at(TileMode::Decal, 2.5), quarter) && at(TileMode::Decal, 12.5).alpha() == 0.0);
    assert!(at(TileMode::Decal, -1.0).alpha() == 0.0);
    // A radial one too; and the mode makes brushes differ.
    let rings = Brush::radial_gradient(&[a, b], o(50.0, 50.0), 10.0, TileMode::Repeated);
    assert!(same(rings.color_at(o(62.5, 50.0), xywh(0.0, 0.0, 100.0, 100.0)), quarter));
    assert!(
        Brush::horizontal_gradient(&[a, b], 0.0, 10.0, TileMode::Mirror)
            != Brush::horizontal_gradient(&[a, b], 0.0, 10.0, CLAMP)
    );
    assert!(TileMode::Decal.is_supported() && TileMode::default() == TileMode::Clamp);
}

// ---- Blend modes ---------------------------------------------------------------------------------------------

#[test]
fn blend_modes_follow_the_compositing_spec() {
    let gray = |v: f32| rgba(v, v, v, 1.0);
    assert!(near_color(blend(gray(0.5), gray(0.4), BlendMode::Multiply), gray(0.2)));
    assert!(near_color(blend(gray(0.5), gray(0.4), BlendMode::Screen), gray(0.7)));
    assert!(near_color(blend(gray(0.8), gray(0.3), BlendMode::Overlay), gray(0.48))); // the backdrop's hard light
    assert!(near_color(blend(gray(0.3), gray(0.8), BlendMode::Hardlight), gray(0.48)));
    assert!(near_color(blend(gray(0.5), gray(0.4), BlendMode::ColorDodge), gray(0.8)));
    assert!(near_color(blend(gray(0.5), gray(0.6), BlendMode::ColorBurn), gray(0.2)));
    assert!(near_color(blend(gray(0.25), gray(0.5), BlendMode::Softlight), gray(0.375)));
    let soft = 0.16 + 0.5 * (((16.0 * 0.16 - 12.0) * 0.16 + 4.0) * 0.16 - 0.16);
    assert!(near_color(blend(gray(0.75), gray(0.16), BlendMode::Softlight), gray(soft)));
    assert!(near_color(blend(gray(0.2), gray(0.7), BlendMode::Difference), gray(0.5)));
    assert!(near_color(blend(gray(0.5), gray(0.5), BlendMode::Exclusion), gray(0.5)));
    assert!(near_color(blend(gray(0.3), gray(0.6), BlendMode::Darken), gray(0.3)));
    assert!(near_color(blend(gray(0.3), gray(0.6), BlendMode::Lighten), gray(0.6)));
    // Non-separable: red onto gray.
    let red = RED;
    assert!(near_color(blend(red, gray(0.5), BlendMode::Hue), gray(0.5))); // gray's saturation (none), luminosity
    assert!(near_color(blend(red, gray(0.5), BlendMode::Color), rgba(1.0, 0.2 / 0.7, 0.2 / 0.7, 1.0))); // clipped
    assert!(near_color(blend(red, gray(0.5), BlendMode::Saturation), gray(0.5))); // no hue to keep
    assert!(near_color(blend(red, gray(0.5), BlendMode::Luminosity), gray(0.3))); // gray at red's luminosity
    // Alpha: the source covers what it covers.
    let half_red = rgba(1.0, 0.0, 0.0, 0.5);
    assert!(near_color(blend(half_red, BLUE, BlendMode::SrcOver), rgba(0.5, 0.0, 0.5, 1.0)));
    assert!(near_color(blend(half_red, BLUE, BlendMode::Multiply), rgba(0.0, 0.0, 0.5, 1.0))); // black, over half
    assert!(near_color(blend(half_red, rgba(0.0, 0.0, 1.0, 0.5), BlendMode::Xor), rgba(0.5, 0.0, 0.5, 0.5)));
    assert_eq!(blend(red, gray(0.5), BlendMode::Clear).alpha(), 0.0);
    assert!(near_color(blend(red, gray(0.5), BlendMode::Dst), gray(0.5)));
    let plus = blend(rgba(1.0, 0.0, 0.0, 0.25), rgba(0.5, 0.5, 0.5, 0.5), BlendMode::Plus);
    assert!(near_color(plus, rgba((0.25 + 0.25) / 0.75, 0.25 / 0.75, 0.25 / 0.75, 0.75)));
    // Compose's 29, in its order.
    assert_eq!(BlendMode::default(), BlendMode::SrcOver);
    assert_eq!(BlendMode::Luminosity as i32, 28);
    assert!(BlendMode::Hue.is_supported());
}

// What takes a color as sRGB takes another space's in sRGB: blending, color matrices, color filters.
#[test]
fn colors_of_other_spaces_are_taken_in_srgb() {
    let p3 = Color::new_in(0.5, 0.2, 0.8, 1.0, &ColorSpaces::DisplayP3);
    let srgb = p3.convert(&ColorSpaces::Srgb);
    let gray = Color::from_argb(0xFF808080);
    let raw = rgba(p3.red(), p3.green(), p3.blue(), 1.0); // (its numbers taken as sRGB's: another color)
    assert_ne!(srgb, raw);
    assert_eq!(blend(p3, gray, BlendMode::Multiply), blend(srgb, gray, BlendMode::Multiply));
    assert_ne!(blend(p3, gray, BlendMode::Multiply), blend(raw, gray, BlendMode::Multiply));
    assert_eq!(blend(gray, p3, BlendMode::Screen), blend(gray, srgb, BlendMode::Screen));
    assert_eq!(ColorMatrix::new().apply(p3), srgb);
    assert_eq!(ColorFilter::color_matrix(ColorMatrix::new()).apply(p3), srgb);
    assert_eq!(
        ColorFilter::lighting(p3, Color::Transparent).apply(Color::White),
        ColorFilter::lighting(srgb, Color::Transparent).apply(Color::White)
    );
    assert_eq!(ColorFilter::tint(p3, BlendMode::SrcIn).apply(Color::White), srgb);
}

// ---- Shaders and shader brushes ------------------------------------------------------------------------------

#[test]
fn shaders_are_composes() {
    let (red, blue, green) = (RED, BLUE, GREEN);
    let even = LinearGradientShader(o(10.0, 0.0), o(30.0, 0.0), &[red, green, blue], None, CLAMP);
    assert!(even.kind() == ShaderKind::Linear && even.stops().len() == 3 && near(even.stops()[1].offset, 0.5, 1e-6));
    assert!(same(even.color_at(o(15.0, 7.0)), along_gradient(red, green, 0.5)));
    assert!(same(even.color_at(o(0.0, 0.0)), red) && same(even.color_at(o(40.0, 0.0)), blue));
    let placed = LinearGradientShader(o(0.0, 0.0), o(0.0, 100.0), &[red, blue], Some(&[0.2, 0.4]), TileMode::Mirror);
    assert!(same(placed.color_at(o(0.0, 30.0)), along_gradient(red, blue, 0.5)));
    assert!(same(placed.color_at(o(0.0, 170.0)), along_gradient(red, blue, 0.5)));
    let repeated = LinearGradientShader(o(0.0, 0.0), o(10.0, 0.0), &[red, blue], None, TileMode::Repeated);
    assert!(same(repeated.color_at(o(12.5, 0.0)), along_gradient(red, blue, 0.25)));
    let decal = LinearGradientShader(o(0.0, 0.0), o(10.0, 0.0), &[red, blue], None, TileMode::Decal);
    assert_eq!(decal.color_at(o(12.0, 0.0)).alpha(), 0.0);
    let radial = RadialGradientShader(o(50.0, 50.0), 20.0, &[red, blue], None, CLAMP);
    assert!(
        radial.kind() == ShaderKind::Radial && same(radial.color_at(o(50.0, 60.0)), along_gradient(red, blue, 0.5))
    );
    assert!(near(radial.radius(), 20.0, 1e-6));
    let sweep = SweepGradientShader(o(0.0, 0.0), &[red, blue], None);
    // clockwise from 3 o'clock: 6 o'clock a quarter round
    assert!(same(sweep.color_at(o(0.0, 10.0)), along_gradient(red, blue, 0.25)));
    // As Compose's (Android's): two colors at least without stops, as many stops as colors with them.
    assert!(panics(|| {
        LinearGradientShader(o(0.0, 0.0), o(1.0, 0.0), &[red], None, CLAMP);
    }));
    assert!(panics(|| {
        RadialGradientShader(o(0.0, 0.0), 1.0, &[red, blue], Some(&[0.0]), CLAMP);
    }));
    assert!(!panics(|| {
        SweepGradientShader(o(0.0, 0.0), &[red], Some(&[0.0]));
    }));
    // An image: a pixel a pixel, bilinearly between pixel centers (premultiplied: a clear pixel adds no color).
    let img = ImageBitmap::from_pixels(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 0]).unwrap();
    let image = |x: TileMode, y: TileMode| ImageShader(img.clone(), x, y);
    let clamp = image(CLAMP, CLAMP);
    assert!(clamp.kind() == ShaderKind::Image && clamp.image() == Some(&img) && clamp.tile_mode_y() == CLAMP);
    assert!(same(clamp.color_at(o(0.5, 0.5)), red) && same(clamp.color_at(o(1.0, 0.5)), with_alpha(red, 0.5)));
    assert!(same(clamp.color_at(o(-3.0, 0.5)), red) && clamp.color_at(o(5.0, 0.5)).alpha() == 0.0);
    let repeated = image(TileMode::Repeated, CLAMP);
    assert!(same(repeated.color_at(o(2.5, 0.5)), red) && repeated.color_at(o(3.5, 0.5)).alpha() == 0.0);
    assert_eq!(image(TileMode::Mirror, CLAMP).color_at(o(2.5, 0.5)).alpha(), 0.0);
    assert!(same(image(TileMode::Mirror, CLAMP).color_at(o(3.5, 0.5)), red));
    assert_eq!(image(TileMode::Decal, CLAMP).color_at(o(-0.5, 0.5)).alpha(), 0.0);
    assert!(same(image(CLAMP, TileMode::Decal).color_at(o(0.5, 0.75)), with_alpha(red, 0.75)));
    // Through a transform, and painted by a brush: what it painted at p, at matrix(p).
    let mut right = Matrix::new();
    right.translate(10.0, 0.0, 0.0);
    assert!(same(clamp.transformed(&right).color_at(o(10.5, 0.5)), red));
    let tiles = ShaderBrush(image(TileMode::Repeated, TileMode::Repeated));
    assert!(same(tiles.color_at(o(102.5, 50.5), xywh(100.0, 50.0, 20.0, 20.0)), red));
    assert_eq!(tiles.color_at(o(103.5, 50.5), xywh(100.0, 50.0, 20.0, 20.0)).alpha(), 0.0);
    // A composite: src onto dst.
    let both = CompositeShader(
        LinearGradientShader(o(0.0, 0.0), o(10.0, 0.0), &[red, red], None, CLAMP),
        LinearGradientShader(o(0.0, 0.0), o(10.0, 0.0), &[with_alpha(blue, 0.0), blue], None, CLAMP),
        BlendMode::SrcOver,
    );
    assert!(both.kind() == ShaderKind::Composite && both.blend_mode() == BlendMode::SrcOver);
    assert_eq!(both.dst().unwrap().stops()[0].color, red);
    assert!(same(both.color_at(o(5.0, 0.0)), blend(with_alpha(blue, 0.5), red, BlendMode::SrcOver)));
    assert!(same(both.color_at(o(10.0, 0.0)), blue));
    let dst_in = CompositeShader(both.dst().unwrap().clone(), both.src().unwrap().clone(), BlendMode::DstIn);
    assert!(same(dst_in.color_at(o(5.0, 0.0)), with_alpha(red, 0.5)));
    // Transforms: what it painted at p, at matrix(p); a second after the first.
    let mut right = Matrix::new();
    right.translate(10.0, 0.0, 0.0);
    let mut twice = Matrix::new();
    twice.scale(2.0, 2.0, 1.0);
    let moved = even.transformed(&right);
    assert!(moved.transform().is_some() && even.transform().is_none());
    assert!(same(moved.color_at(o(25.0, 0.0)), along_gradient(red, green, 0.5)));
    assert!(same(moved.transformed(&twice).color_at(o(50.0, 0.0)), along_gradient(red, green, 0.5))); // (15 + 10) * 2
    assert!(same(even.transformed(&twice).transformed(&right).color_at(o(40.0, 0.0)), along_gradient(red, green, 0.5)));
    assert!(even.transformed(&Matrix::new()) == even && even.transformed(&Matrix::new()).transform().is_none());
    // Equal by what they are.
    assert_eq!(LinearGradientShader(o(10.0, 0.0), o(30.0, 0.0), &[red, green, blue], None, CLAMP), even);
    assert_ne!(even, moved);
    assert_ne!(radial, RadialGradientShader(o(50.0, 50.0), 21.0, &[red, blue], None, CLAMP));
    assert_ne!(both, dst_in);
    // An image's by being the same bitmap, and by its tile modes.
    assert_eq!(image(CLAMP, CLAMP), clamp);
    assert_ne!(image(TileMode::Repeated, CLAMP), clamp);
    assert_ne!(image(CLAMP, TileMode::Repeated), clamp);
    let copy = ImageBitmap::from_pixels(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 0]).unwrap();
    assert_ne!(ImageShader(copy, CLAMP, CLAMP), clamp);
}

#[test]
fn shader_brushes_are_composes() {
    let (red, blue, green) = (RED, BLUE, GREEN);
    let made = Arc::new(AtomicUsize::new(0));
    let made_for = Arc::new(std::sync::Mutex::new(Size::Zero));
    let own = {
        let (made, made_for) = (made.clone(), made_for.clone());
        Brush::shader_brush(
            move |size| {
                made.fetch_add(1, Ordering::SeqCst);
                *made_for.lock().unwrap() = size;
                LinearGradientShader(o(0.0, 0.0), o(size.width, 0.0), &[red, blue], None, CLAMP)
            },
            Size::new(40.0, 20.0),
        )
    };
    assert!(matches!(own.kind(), BrushKind::Shader(_)) && !own.is_solid());
    let area = xywh(100.0, 50.0, 100.0, 20.0);
    assert!(same(own.color_at(o(150.0, 60.0), area), along_gradient(red, blue, 0.5)));
    assert!(made.load(Ordering::SeqCst) == 1 && near(made_for.lock().unwrap().width, 100.0, 1e-6));
    // the same size: not made again
    assert!(
        same(own.color_at(o(125.0, 60.0), area), along_gradient(red, blue, 0.25)) && made.load(Ordering::SeqCst) == 1
    );
    assert!(same(own.color_at(o(110.0, 60.0), xywh(100.0, 50.0, 20.0, 20.0)), along_gradient(red, blue, 0.5)));
    assert_eq!(made.load(Ordering::SeqCst), 2);
    let copy = own.clone();
    assert!(
        copy == own
            && own != Brush::shader_brush(|_| SweepGradientShader(o(0.0, 0.0), &[RED, BLUE], None), Size::Unspecified)
    );
    assert!(near(own.intrinsic_size().width, 40.0, 1e-6) && near(own.intrinsic_size().height, 20.0, 1e-6));
    // Faded: what it paints as opaque as the paint's alpha says.
    let half = own.faded(0.5);
    assert!(near(half.shader_alpha(), 0.5, 1e-6) && half != own);
    assert!(same(half.color_at(o(150.0, 60.0), area), with_alpha(along_gradient(red, blue, 0.5), 0.5)));
    assert!(near(horizontal(&[red, blue]).faded(0.5).shader_alpha(), 1.0, 1e-6)); // (a gradient's alpha: its colors)
    // One shader whatever the size.
    let rings = RadialGradientShader(o(5.0, 5.0), 5.0, &[red, blue], None, TileMode::Repeated);
    let fixed = ShaderBrush(rings.clone());
    assert!(fixed.create_shader(Size::new(300.0, 300.0)) == rings);
    assert!(same(fixed.color_at(o(17.5, 5.0), xywh(0.0, 0.0, 300.0, 300.0)), along_gradient(red, blue, 0.5)));
    assert_eq!(fixed, ShaderBrush(RadialGradientShader(o(5.0, 5.0), 5.0, &[red, blue], None, TileMode::Repeated)));
    assert_ne!(fixed, ShaderBrush(RadialGradientShader(o(5.0, 5.0), 6.0, &[red, blue], None, TileMode::Repeated)));
    let maker = rings.clone();
    assert_ne!(fixed, Brush::shader_brush(move |_| maker.clone(), Size::Unspecified)); // equal by its shader
    // Transforms: a gradient's or a shader's, in what it paints; a color has none.
    let mut down = Matrix::new();
    down.translate(0.0, 10.0, 0.0);
    let h = Brush::horizontal_gradient(&[red, blue], 0.0, 20.0, CLAMP);
    let mut turn = Matrix::new();
    turn.rotate_z(90.0);
    let turned = h.with_transform(Some(turn));
    let square = xywh(100.0, 50.0, 20.0, 20.0);
    assert!(
        turned.transform().is_some() && same(turned.color_at(o(100.0, 55.0), square), along_gradient(red, blue, 0.25))
    );
    assert!(near(turned.parameter_at(o(100.0, 55.0), square), 0.25, 1e-4));
    assert!(SolidColor(red).with_transform(Some(down)).transform().is_none());
    assert!(
        h.with_transform(Some(Matrix::new())).transform().is_none() && h.with_transform(None).transform().is_none()
    );
    assert_eq!(
        h.with_transform(Some(down)).create_shader(Size::new(20.0, 20.0)),
        h.create_shader(Size::new(20.0, 20.0)).transformed(&down)
    );
    assert!(same(
        fixed.with_transform(Some(down)).color_at(o(17.5, 15.0), xywh(0.0, 0.0, 300.0, 300.0)),
        along_gradient(red, blue, 0.5)
    ));
    assert!(h.with_transform(Some(down)) != h && h.with_transform(Some(down)) == h.with_transform(Some(down)));
    // A brush's shader: a gradient's where it lies on that size; a color's a gradient of it.
    let hs = h.create_shader(Size::new(50.0, 30.0));
    assert!(hs.kind() == ShaderKind::Linear && hs.start() == o(0.0, 0.0) && hs.end() == o(20.0, 0.0));
    assert_eq!(hs.stops(), h.color_stops().as_slice());
    let solid = SolidColor(green).create_shader(Size::new(50.0, 30.0));
    assert!(
        solid.kind() == ShaderKind::Linear && same(solid.color_at(o(7.0, 7.0)), green) && solid.end() == o(0.0, 30.0)
    );
    let r = Brush::radial_gradient(&[red, blue], Offset::Unspecified, INF, CLAMP).create_shader(Size::new(50.0, 30.0));
    assert!(r.kind() == ShaderKind::Radial && r.start() == o(25.0, 15.0) && near(r.radius(), 15.0, 1e-6));
    assert_eq!(
        Brush::sweep_gradient(&[red, blue], o(3.0, 4.0)).create_shader(Size::new(50.0, 30.0)).start(),
        o(3.0, 4.0)
    );
    // Composites (Brush.composite): their two made shaders composited; equal by what they composite.
    let fading_blue = horizontal(&[with_alpha(blue, 0.0), blue]);
    let over = Brush::composite(&SolidColor(red), &fading_blue, BlendMode::SrcOver);
    let os = over.create_shader(Size::new(10.0, 10.0));
    assert!(os.kind() == ShaderKind::Composite && os.blend_mode() == BlendMode::SrcOver);
    assert_eq!(*os.dst().unwrap(), SolidColor(red).create_shader(Size::new(10.0, 10.0)));
    assert!(same(
        over.color_at(o(5.0, 5.0), xywh(0.0, 0.0, 10.0, 10.0)),
        blend(with_alpha(blue, 0.5), red, BlendMode::SrcOver)
    ));
    assert_eq!(over, Brush::composite(&SolidColor(red), &fading_blue, BlendMode::SrcOver));
    assert_ne!(over, Brush::composite(&SolidColor(red), &fading_blue, BlendMode::Multiply));
    assert_ne!(over, Brush::composite(&SolidColor(green), &fading_blue, BlendMode::SrcOver));
    // Intrinsic sizes (Compose's): a linear gradient's between its ends where they are finite, a radial one's diameter.
    let hz = Brush::horizontal_gradient(&[red, blue], 5.0, 25.0, CLAMP).intrinsic_size();
    assert!(near(hz.width, 20.0, 1e-6) && near(hz.height, 0.0, 1e-6));
    let far = Brush::horizontal_gradient(&[red, blue], 5.0, INF, CLAMP).intrinsic_size();
    assert!(far.width.is_nan() && near(far.height, 0.0, 1e-6));
    let corner = Brush::linear_gradient(&[red, blue], Offset::Zero, Offset::Infinite, CLAMP).intrinsic_size();
    assert!(corner.width.is_nan() && corner.height.is_nan());
    assert!(near(
        Brush::radial_gradient(&[red, blue], Offset::Unspecified, 7.0, CLAMP).intrinsic_size().width,
        14.0,
        1e-6
    ));
    assert!(Brush::radial_gradient(&[red, blue], Offset::Unspecified, INF, CLAMP).intrinsic_size().height.is_nan());
    assert!(Brush::sweep_gradient(&[red, blue], Offset::Unspecified).intrinsic_size().width.is_nan());
    assert!(SolidColor(red).intrinsic_size().width.is_nan() && fixed.intrinsic_size().width.is_nan());
    assert!(over.intrinsic_size().width.is_nan());
}

// lerp of brushes (Compose's Interpolatable): colors mix; gradients of a kind mix colors, stops and geometry; a color
// with a gradient as that gradient in the color; anything else switches half way.
#[test]
fn brushes_lerp_as_composes() {
    let (red, blue, green) = (RED, BLUE, GREEN);
    let area = xywh(0.0, 0.0, 100.0, 100.0);
    let mixed = lerp_brush(&SolidColor(red), &SolidColor(blue), 0.25);
    assert!(mixed.is_solid() && mixed.solid_color() == Some(lerp_color(red, blue, 0.25)));
    // Two linear gradients: colors index by index, the shorter's last again; ends mixed.
    let a = Brush::linear_gradient(&[red, green], o(0.0, 0.0), o(10.0, 0.0), CLAMP);
    let b = Brush::linear_gradient(&[blue, blue, red], o(0.0, 10.0), o(20.0, 0.0), CLAMP);
    let ab = lerp_brush(&a, &b, 0.5);
    let BrushKind::LinearGradient(g) = ab.kind() else { panic!("a linear gradient") };
    assert_eq!(g.colors, vec![lerp_color(red, blue, 0.5), lerp_color(green, blue, 0.5), lerp_color(green, red, 0.5)]);
    assert!(g.stops.is_none()); // (colors spread evenly: evenly again)
    assert!(near(ab.color_stops()[1].offset, 0.5, 1e-6));
    let geometry = ab.geometry(area);
    assert!(geometry.start == o(0.0, 5.0) && geometry.end == o(15.0, 0.0));
    // Given stops mix (both given; one without: none); an end at the far edge switches half way.
    let s1 = Brush::linear_gradient_stops(&[(0.0, red), (0.2, blue)], o(0.0, 0.0), o(10.0, 0.0), CLAMP);
    let s2 = Brush::linear_gradient_stops(&[(0.4, red), (0.6, blue)], o(0.0, 0.0), o(INF, 0.0), CLAMP);
    let s12 = lerp_brush(&s1, &s2, 0.25);
    let stops = s12.color_stops();
    assert!(near(stops[0].offset, 0.1, 1e-5) && near(stops[1].offset, 0.3, 1e-5));
    assert!(s12.geometry(area).end == o(10.0, 0.0) && lerp_brush(&s1, &s2, 0.75).geometry(area).end == o(100.0, 0.0));
    let BrushKind::LinearGradient(g) = lerp_brush(&s1, &a, 0.5).kind().clone() else { panic!() };
    assert!(g.stops.is_none());
    // Tile modes switch half way; centers and radii mix (Compose's: plainly, an unspecified center as NaN).
    let r1 = Brush::radial_gradient(&[red, blue], o(10.0, 10.0), 10.0, TileMode::Repeated);
    let r2 = Brush::radial_gradient(&[red, blue], o(30.0, 10.0), 30.0, CLAMP);
    let r12 = lerp_brush(&r1, &r2, 0.25);
    assert!(r12.tile_mode() == TileMode::Repeated && lerp_brush(&r1, &r2, 0.5).tile_mode() == TileMode::Clamp);
    assert!(r12.geometry(area).start == o(15.0, 10.0) && near(r12.geometry(area).radius, 15.0, 1e-5));
    let centered = Brush::radial_gradient(&[red, blue], Offset::Unspecified, INF, CLAMP);
    let BrushKind::RadialGradient(g) = lerp_brush(&r1, &centered, 0.25).kind().clone() else { panic!() };
    assert!(g.center.is_unspecified() && g.radius.is_infinite());
    assert_eq!(lerp_brush(&r1, &centered, 0.25).geometry(area).start, o(50.0, 50.0));
    // A color and a gradient: the gradient, its colors toward the color (either way round).
    let toward = lerp_brush(&SolidColor(green), &Brush::horizontal_gradient(&[red, blue], 0.0, 10.0, CLAMP), 0.75);
    let BrushKind::LinearGradient(g) = toward.kind() else { panic!("a linear gradient") };
    assert_eq!(g.colors, vec![lerp_color(red, green, 0.25), lerp_color(blue, green, 0.25)]);
    assert!(same(g.colors[0], lerp_color(green, red, 0.75)) && same(g.colors[1], lerp_color(green, blue, 0.75)));
    assert_eq!(toward.geometry(area).end, o(10.0, 0.0));
    let back = lerp_brush(&Brush::horizontal_gradient(&[red, blue], 0.0, 10.0, CLAMP), &SolidColor(green), 0.25);
    assert_eq!(back.color_stops()[0].color, lerp_color(red, green, 0.25));
    // Toward nothing: toward transparent.
    assert_eq!(SolidColor(red).lerp(None, 0.5), Some(SolidColor(lerp_color(red, Color::Transparent, 0.5))));
    // Anything else switches half way: other kinds, shader brushes.
    let sweep = Brush::sweep_gradient(&[red, blue], Offset::Unspecified);
    let own = ShaderBrush(SweepGradientShader(o(0.0, 0.0), &[red, blue], None));
    assert_eq!(lerp_brush(&a, &sweep, 0.49), a);
    assert_eq!(lerp_brush(&a, &sweep, 0.5), sweep);
    assert!(
        lerp_brush(&own, &SolidColor(red), 0.4) == own && lerp_brush(&own, &SolidColor(red), 0.6) == SolidColor(red)
    );
    assert!(lerp_brush(&a, &a, 0.3) == a);
    let mixed_sweep = lerp_brush(&sweep, &Brush::sweep_gradient(&[green, red], Offset::Unspecified), 0.5);
    let BrushKind::SweepGradient(g) = mixed_sweep.kind() else { panic!("a sweep gradient") };
    assert_eq!(g.colors[0], lerp_color(red, green, 0.5));
}

#[test]
fn brushes_print_as_composes() {
    assert_eq!(
        format!("{:?}", SolidColor(Color::Red)),
        "SolidColor(value=Color(1.0, 0.0, 0.0, 1.0, sRGB IEC61966-2.1))"
    );
    let h = Brush::horizontal_gradient(&[Color::Red, Color::Blue], 0.0, 10.0, CLAMP);
    assert!(format!("{h:?}").starts_with("LinearGradient(colors=[Color(1.0"));
    assert!(
        format!("{h:?}")
            .ends_with("stops=null, start=Offset { x: 0.0, y: 0.0 }, end=Offset { x: 10.0, y: 0.0 }, tileMode=Clamp)")
    );
    let r = Brush::radial_gradient(&[Color::Red, Color::Blue], Offset::Unspecified, INF, CLAMP);
    assert!(format!("{r:?}").ends_with("stops=null, tileMode=Clamp)"));
}

// ---- Border strokes ------------------------------------------------------------------------------------------

#[test]
fn border_strokes_take_a_color_or_a_brush() {
    let thin = BorderStroke(1.dp(), Color::Red);
    assert!(thin.width == 1.dp() && thin.brush == SolidColor(Color::Red));
    let gradient = Brush::horizontal_gradient(&[Color::Red, Color::Blue], 0.0, INF, CLAMP);
    let fancy = BorderStroke(2.dp(), gradient.clone());
    assert_eq!(fancy.brush, gradient);
    assert_eq!(kompozable_block!({ thin.copy(width = 3.dp()) }), BorderStroke(3.dp(), Color::Red));
    assert_eq!(thin.copy(), thin);
    assert_ne!(thin, fancy);
}

// ---- Vertices ------------------------------------------------------------------------------------------------

#[test]
fn vertices_are_composes() {
    let red = RED;
    let five = [o(0.0, 0.0), o(10.0, 0.0), o(10.0, 10.0), o(0.0, 10.0), o(5.0, 15.0)];
    let colors = [red; 5];
    let mesh = |mode, indices: &[i32]| Vertices::new(mode, &five, &five, &colors, indices);
    assert_eq!(mesh(VertexMode::Triangles, &[]).triangles(), vec![[0, 1, 2]]);
    assert_eq!(mesh(VertexMode::TriangleStrip, &[]).triangles(), vec![[0, 1, 2], [1, 2, 3], [2, 3, 4]]);
    assert_eq!(mesh(VertexMode::TriangleFan, &[]).triangles(), vec![[0, 1, 2], [0, 2, 3], [0, 3, 4]]);
    assert_eq!(mesh(VertexMode::Triangles, &[0, 2, 3, 4, 3, 2]).triangles(), vec![[0, 2, 3], [4, 3, 2]]);
    assert_eq!(mesh(VertexMode::TriangleFan, &[4, 0, 1]).triangles(), vec![[4, 0, 1]]);
    // Kept as Compose keeps them: x, y pairs, ARGB colors, 16 bit indices.
    let m = mesh(VertexMode::Triangles, &[2, 1, 0]);
    assert_eq!(&m.positions()[..4], &[0.0, 0.0, 10.0, 0.0]);
    assert!(m.colors().iter().all(|&c| c == 0xFFFF0000) && m.indices() == [2, 1, 0]);
    assert_eq!(m.vertex_count(), 5);
    assert_eq!(m.position(4), o(5.0, 15.0));
    assert_eq!(m.bounds(), Rect::new(0.0, 0.0, 10.0, 15.0));
    // As Compose throws: lists that don't match, an index out of range.
    assert!(panics(|| {
        Vertices::new(VertexMode::Triangles, &five, &[o(0.0, 0.0)], &colors, &[]);
    }));
    assert!(panics(|| {
        Vertices::new(VertexMode::Triangles, &five, &five, &[red], &[]);
    }));
    assert!(panics(|| {
        Vertices::new(VertexMode::Triangles, &five, &five, &colors, &[0, 9, 1]);
    }));
    assert!(panics(|| {
        Vertices::new(VertexMode::Triangles, &five, &five, &colors, &[-1]);
    }));
    // From arrays: x, y pairs (an odd float left out), ARGB colors.
    let raw = Vertices::from_arrays(
        VertexMode::TriangleStrip,
        &[0.0, 0.0, 10.0, 0.0, 0.0, 10.0, 99.0],
        &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 7.0],
        &[0x80FF0000, 0xFF0000FF, 0xFFFFFFFF],
        &[2, 1, 0],
    );
    assert!(raw.vertex_count() == 3 && raw.position(2) == o(0.0, 10.0) && raw.indices() == [2, 1, 0]);
    assert_eq!(raw.texture_coordinate(1), o(1.0, 0.0));
    assert!(raw.colors()[0] == 0x80FF0000 && raw.colors()[1] == 0xFF0000FF);
    assert_eq!(raw.triangles(), vec![[2, 1, 0]]);
    assert!(mesh(VertexMode::Triangles, &[]) == mesh(VertexMode::Triangles, &[]));
    assert!(mesh(VertexMode::Triangles, &[]) != mesh(VertexMode::TriangleFan, &[]));
}

// ---- Color matrices and filters ------------------------------------------------------------------------------

#[test]
fn color_filters_tint_matrices_and_lighting() {
    let (red, blue, half_blue) = (rgba(1.0, 0.0, 0.0, 0.5), BLUE, rgba(0.0, 0.0, 1.0, 0.5));
    let tint = ColorFilter::tint;
    let gray = |v: f32| rgba(v, v, v, 1.0);
    assert!(near_color(tint(blue, BlendMode::SrcIn).apply(red), rgba(0.0, 0.0, 1.0, 0.5))); // its shape in the color
    assert!(near_color(tint(half_blue, BlendMode::SrcAtop).apply(RED), rgba(0.5, 0.0, 0.5, 1.0)));
    assert!(near_color(tint(half_blue, BlendMode::Src).apply(red), half_blue));
    assert!(near_color(tint(half_blue, BlendMode::DstIn).apply(RED), rgba(1.0, 0.0, 0.0, 0.5)));
    assert!(near_color(
        tint(rgba(0.5, 1.0, 1.0, 1.0), BlendMode::Modulate).apply(rgba(1.0, 0.5, 0.0, 1.0)),
        rgba(0.5, 0.5, 0.0, 1.0)
    ));
    assert!(near_color(tint(gray(0.5), BlendMode::Multiply).apply(rgba(1.0, 1.0, 0.0, 1.0)), rgba(0.5, 0.5, 0.0, 1.0)));
    assert!(near_color(tint(gray(0.5), BlendMode::Screen).apply(RED), rgba(1.0, 0.5, 0.5, 1.0)));
    assert!(near_color(tint(gray(0.5), BlendMode::Darken).apply(RED), rgba(0.5, 0.0, 0.0, 1.0)));
    assert!(near_color(tint(gray(0.5), BlendMode::Lighten).apply(RED), rgba(1.0, 0.5, 0.5, 1.0)));
    assert!(near_color(tint(gray(0.5), BlendMode::Plus).apply(rgba(0.75, 0.0, 0.0, 1.0)), rgba(1.0, 0.5, 0.5, 1.0)));
    assert!(near_color(tint(half_blue, BlendMode::SrcOver).apply(RED), rgba(0.5, 0.0, 0.5, 1.0)));
    // nothing there: nothing
    assert!(near_color(tint(blue, BlendMode::SrcIn).apply(rgba(1.0, 0.0, 0.0, 0.0)), rgba(0.0, 0.0, 0.0, 0.0)));
    let mut saturation = ColorMatrix::new();
    saturation.set_to_saturation(0.0);
    assert!(near_color(ColorFilter::color_matrix(saturation).apply(RED), rgba(0.213, 0.213, 0.213, 1.0)));
    let mut scale = ColorMatrix::new();
    scale.set_to_scale(0.5, 1.0, 1.0, 1.0);
    scale.times_assign(&saturation); // gray first, then halve the red
    assert!(near_color(scale.apply(GREEN), rgba(0.3575, 0.715, 0.715, 1.0)));
    scale.reset();
    assert_eq!(scale, ColorMatrix::default());
    let lighting = ColorFilter::lighting(rgba(0.5, 1.0, 1.0, 1.0), rgba(0.1, 0.25, 0.0, 0.0));
    assert!(near_color(lighting.apply(rgba(1.0, 0.5, 0.0, 0.8)), rgba(0.6, 0.75, 0.0, 0.8)));
    // Where it matters that the colors are premultiplied, and what each mode does with alpha.
    assert!(near_color(tint(rgba(0.0, 0.0, 0.4, 1.0), BlendMode::SrcIn).apply(red), rgba(0.0, 0.0, 0.4, 0.5)));
    let half_gray = rgba(0.5, 0.5, 0.5, 0.5);
    assert!(near_color(
        tint(half_gray, BlendMode::Multiply).apply(rgba(1.0, 1.0, 0.0, 1.0)),
        rgba(0.75, 0.75, 0.0, 1.0)
    ));
    assert!(near_color(tint(gray(0.5), BlendMode::Screen).apply(rgba(0.5, 0.0, 0.0, 1.0)), rgba(0.75, 0.5, 0.5, 1.0)));
    assert!(near_color(tint(half_blue, BlendMode::SrcAtop).apply(red), rgba(0.5, 0.0, 0.5, 0.5)));
    // A matrix's fifth column is added (in 0..255, as Android's), and kept when another goes before it.
    let mut lift = ColorMatrix::new();
    lift.values[4] = 0.1 * 255.0;
    assert!(near_color(lift.apply(rgba(0.5, 0.0, 0.0, 1.0)), rgba(0.6, 0.0, 0.0, 1.0)));
    let mut half = ColorMatrix::new();
    half.set_to_scale(0.5, 1.0, 1.0, 1.0);
    half.values[9] = 0.2 * 255.0; // and lifts green
    lift *= &half;
    assert!(near_color(lift.apply(RED), rgba(0.6, 0.2, 0.0, 1.0)));
    // Compose's inversion: -1 times each channel, plus 255.
    let invert = ColorMatrix::from_values([
        -1.0, 0.0, 0.0, 0.0, 255.0, 0.0, -1.0, 0.0, 0.0, 255.0, 0.0, 0.0, -1.0, 0.0, 255.0, 0.0, 0.0, 0.0, 1.0, 0.0,
    ]);
    assert!(near_color(invert.apply(rgba(0.25, 1.0, 0.0, 1.0)), rgba(0.75, 0.0, 1.0, 1.0)));
    // A brush filtered: each of its colors.
    let filtered = tint(blue, BlendMode::SrcIn).apply_brush(&horizontal(&[RED, rgba(0.0, 1.0, 0.0, 0.5)]));
    let BrushKind::LinearGradient(g) = filtered.kind() else { panic!("a linear gradient") };
    assert!(near_color(g.colors[0], BLUE) && near_color(g.colors[1], half_blue));
    assert!(near_color(tint(blue, BlendMode::SrcIn).apply_brush(&SolidColor(RED)).solid_color().unwrap(), BLUE));
    // Equal by what they are; printed as Compose's.
    assert_eq!(tint(blue, BlendMode::SrcIn), ColorFilter::BlendMode { color: blue, blend_mode: BlendMode::SrcIn });
    assert_ne!(tint(blue, BlendMode::SrcIn), tint(blue, BlendMode::SrcOver));
    assert!(
        format!("{:?}", tint(blue, BlendMode::SrcIn)).starts_with("BlendModeColorFilter(color=Color(0.0, 0.0, 1.0")
    );
}

/// The filtered copy of `images_draw_through_a_color_filter` (what a backend filtering images on the CPU draws; the
/// drawing itself is stage 5's).
#[test]
fn images_filter_into_copies_kept_while_they_live() {
    let photo = ImageBitmap::from_pixels(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 128]).unwrap();
    let blue = ColorFilter::tint(BLUE, BlendMode::SrcIn);
    // Made once, then kept.
    let tinted = blue.apply_image(&photo);
    assert_eq!(tinted.id(), blue.apply_image(&photo).id());
    assert!(tinted.id() != photo.id() && tinted.width() == 2 && tinted.config() == ImageBitmapConfig::Argb8888);
    let px = tinted.pixels();
    assert!(px[0] == 0 && px[2] == 255 && px[3] == 255 && px[4] == 0 && px[6] == 255 && px[7] == 128);
    // Another filter: its own copy.
    let green = ColorFilter::tint(GREEN, BlendMode::SrcIn).apply_image(&photo);
    assert!(green.id() != tinted.id() && green.pixels()[1] == 255 && green.pixels()[2] == 0);
    // The bitmap drawn into since: filtered again.
    photo.store(&[0, 0, 0, 0, 0, 0, 0, 255], true);
    let again = blue.apply_image(&photo);
    assert!(again.id() != tinted.id() && again.pixels()[3] == 0 && again.pixels()[7] == 255);
    assert_eq!(blue.apply_image(&photo).id(), again.id());
    // Another bitmap of the same pixels: its own.
    let twin = ImageBitmap::from_pixels(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 128]).unwrap();
    assert_ne!(blue.apply_image(&twin).id(), tinted.id());
    // A matrix's and a light's too.
    let mut saturation = ColorMatrix::new();
    saturation.set_to_saturation(0.0);
    let gray = ColorFilter::color_matrix(saturation).apply_image(&twin);
    assert!(gray.pixels()[0] == gray.pixels()[1] && gray.pixels()[1] == gray.pixels()[2] && gray.pixels()[0] > 0);
    let dark = ColorFilter::lighting(Color::Black, Color::Transparent).apply_image(&twin);
    assert!(dark.pixels()[0] == 0 && dark.pixels()[3] == 255 && dark.pixels()[7] == 128);
}

#[test]
fn color_matrices_turn_scale_and_convert_as_composes() {
    // (Compose's ColorMatrix(floatArrayOf(…)): fewer values, the rest the identity's)
    assert!(
        ColorMatrix::from_slice(&[2.0, 0.0, 0.0]).values[0] == 2.0 && ColorMatrix::from_slice(&[2.0]).values[6] == 1.0
    );
    let mut m = ColorMatrix::new();
    assert_eq!(m.get(3, 3), 1.0);
    m.set(0, 4, 3.0);
    assert_eq!(m.values[4], 3.0);
    let mut copy = ColorMatrix::new();
    copy.set_from(&m);
    assert_eq!(copy, m);
    // Turned about an axis, its sine and cosine as Compose approximates them.
    let mut r = ColorMatrix::new();
    r.set_to_rotate_red(90.0);
    assert!(near(r.get(1, 1), 0.0, 1e-3) && near(r.get(1, 2), 1.0, 1e-3) && near(r.get(2, 1), -1.0, 1e-3));
    assert!(r.get(0, 0) == 1.0 && r.get(3, 3) == 1.0);
    let mut g = ColorMatrix::new();
    g.set_to_rotate_green(180.0);
    assert!(near(g.get(0, 0), -1.0, 1e-3) && near(g.get(0, 2), 0.0, 1e-3));
    let mut b = ColorMatrix::new();
    b.set_to_rotate_blue(30.0);
    assert!(near(b.get(0, 0), 0.866, 3e-3) && near(b.get(0, 1), 0.5, 3e-3) && near(b.get(1, 0), -0.5, 3e-3));
    // RGB to YUV and back (libjpeg's): about the identity.
    let mut yuv = ColorMatrix::new();
    yuv.convert_rgb_to_yuv();
    assert_eq!(yuv.get(1, 1), -0.33126);
    let mut rgb = ColorMatrix::new();
    rgb.convert_yuv_to_rgb();
    let mut round = rgb;
    round *= &yuv;
    for row in 0..3 {
        for column in 0..3 {
            assert!(near(round.get(row, column), if row == column { 1.0 } else { 0.0 }, 1e-3));
        }
    }
}

// ---- Render effects ------------------------------------------------------------------------------------------

#[test]
fn render_effects_chain_and_reach_past_the_bounds() {
    let e = RenderEffect::offset_effect(Some(&BlurEffect(2.0, 3.0, TileMode::Mirror)), o(5.0, -1.0));
    assert_eq!(
        e.steps(),
        &[
            RenderEffectStep::Blur { radius_x: 2.0, radius_y: 3.0, edge_treatment: TileMode::Mirror },
            RenderEffectStep::Offset(o(5.0, -1.0)),
        ]
    );
    assert_eq!(RenderEffect::blur_effect(None, 4.0, 4.0, CLAMP), BlurEffect(4.0, 4.0, CLAMP));
    assert_ne!(BlurEffect(4.0, 4.0, CLAMP), BlurEffect(4.0, 4.0, TileMode::Decal));
    assert_eq!(OffsetEffect(1.0, 2.0), RenderEffect::offset_effect(None, o(1.0, 2.0)));
    assert_ne!(OffsetEffect(1.0, 2.0), OffsetEffect(2.0, 1.0));
    // (none that way)
    assert!(matches!(BlurEffect(-3.0, 2.0, CLAMP).steps()[0], RenderEffectStep::Blur { radius_x: 0.0, .. }));
    assert!(e.is_supported());
    // Where it may reach: three sigmas (0.57735 r + 0.5 px) each way, then moved.
    let out = e.output_bounds(xywh(10.0, 10.0, 20.0, 20.0));
    let (x, y) = (3.0 * (0.57735 * 2.0 + 0.5), 3.0 * (0.57735 * 3.0 + 0.5));
    assert!(near(out.left, 10.0 - x + 5.0, 1e-4) && near(out.top, 10.0 - y - 1.0, 1e-4));
    assert!(near(out.width(), 20.0 + 2.0 * x, 1e-4) && near(out.height(), 20.0 + 2.0 * y, 1e-4));
    assert_eq!(BlurEffect(0.0, 0.0, CLAMP).output_bounds(xywh(1.0, 2.0, 3.0, 4.0)), xywh(1.0, 2.0, 3.0, 4.0));
}

#[test]
fn blur_radius_specs_make_compose_effects() {
    let density = Density::new(2.0, 1.0);
    let progressive = |e: &RenderEffect| match e.steps() {
        [RenderEffectStep::ProgressiveBlur { progressive, .. }] => Some(progressive.clone()),
        _ => None,
    };
    let edges = |e: &RenderEffect| match e.steps() {
        [RenderEffectStep::ProgressiveBlur { edge_treatment, .. }] => Some(*edge_treatment),
        _ => None,
    };
    let size = Size::new(100.0, 50.0);
    // Uniform: the platform's blur (radii in px by the density), clamped unless decal (repeated and mirrored edges
    // taken as clamped, as Compose's).
    let uniform = BlurRadiusSpec::uniform(3.dp());
    assert_eq!(uniform.create_render_effect(size, CLAMP, &density), BlurEffect(6.0, 6.0, CLAMP));
    assert_eq!(uniform.create_render_effect(size, TileMode::Mirror, &density), BlurEffect(6.0, 6.0, CLAMP));
    assert_eq!(uniform.create_render_effect(size, TileMode::Decal, &density), BlurEffect(6.0, 6.0, TileMode::Decal));
    // Down, across, along a line: from the top, the left, the start; its size's far side.
    let down = BlurRadiusSpec::vertical_gradient(0.dp(), 6.dp()).create_render_effect(size, TileMode::Decal, &density);
    let p = progressive(&down).unwrap();
    assert!(!p.radial && p.start == o(0.0, 0.0) && p.end == o(0.0, 50.0) && p.stops == vec![(0.0, 0.0), (1.0, 12.0)]);
    assert_eq!(edges(&down), Some(TileMode::Decal));
    let across = BlurRadiusSpec::horizontal_gradient(2.dp(), 1.dp()).create_render_effect(size, CLAMP, &density);
    let p = progressive(&across).unwrap();
    assert!(p.end == o(100.0, 0.0) && p.stops == vec![(0.0, 4.0), (1.0, 2.0)]);
    let along = BlurRadiusSpec::linear_gradient(o(10.0, 5.0), o(60.0, 40.0), 3.dp(), 9.dp());
    let p = progressive(&along.create_render_effect(size, CLAMP, &density)).unwrap();
    assert!(p.start == o(10.0, 5.0) && p.end == o(60.0, 40.0));
    // Out from a center: its middle and half its shorter side unless set (a fall-off that isn't positive as unset).
    let out = BlurRadiusSpec::radial_gradient(8.dp(), 0.dp(), Offset::Unspecified, Dp::Unspecified);
    let p = progressive(&out.create_render_effect(size, CLAMP, &density)).unwrap();
    assert!(p.radial && p.start == o(50.0, 25.0) && near(p.radius, 25.0, 1e-6));
    let around = BlurRadiusSpec::radial_gradient_stops(
        &[BlurStop::new(0.0, 2.dp()), BlurStop::new(1.0, 6.dp())],
        o(10.0, 10.0),
        15.dp(),
    );
    let p = progressive(&around.create_render_effect(size, CLAMP, &density)).unwrap();
    assert!(p.start == o(10.0, 10.0) && near(p.radius, 30.0, 1e-6));
    assert_eq!(BlurRadiusSpec::radial_gradient(8.dp(), 0.dp(), Offset::Unspecified, (-4).dp()), out);
    // Stops sorted by their fractions (stably), the fractions kept to 0..1, radii below 0 taken as 0.
    let sorted = BlurRadiusSpec::vertical_gradient_stops(&[
        BlurStop::new(0.8, 4.dp()),
        BlurStop::new(0.2, 8.dp()),
        BlurStop::new(0.2, 2.dp()),
        BlurStop::new(1.5, (-3).dp()),
    ]);
    let p = progressive(&sorted.create_render_effect(Size::new(10.0, 10.0), CLAMP, &Density::default())).unwrap();
    assert_eq!(p.stops, vec![(0.2, 8.0), (0.2, 2.0), (0.8, 4.0), (1.0, 0.0)]);
    // Nothing to blur: an effect that changes nothing.
    let none = BlurRadiusSpec::vertical_gradient(0.dp(), 0.dp());
    assert_eq!(none.create_render_effect(Size::new(10.0, 10.0), CLAMP, &density), OffsetEffect(0.0, 0.0));
    assert_eq!(
        BlurRadiusSpec::vertical_gradient(0.dp(), 12.dp()),
        BlurRadiusSpec::vertical_gradient_stops(&[BlurStop::new(0.0, 0.dp()), BlurStop::new(1.0, 12.dp())])
    );
    assert_ne!(
        BlurRadiusSpec::vertical_gradient(0.dp(), 12.dp()),
        BlurRadiusSpec::horizontal_gradient(0.dp(), 12.dp())
    );
    // It reaches as far as its widest radius, and a pixel.
    assert_eq!(down.output_bounds(xywh(0.0, 0.0, 100.0, 50.0)), xywh(-13.0, -13.0, 126.0, 76.0));
    // A shader's: each pixel by the widest times its alpha, the shader made for the layer's size and density.
    let seen: Arc<std::sync::Mutex<Option<(Density, Size)>>> = Arc::default();
    let masked = {
        let seen = seen.clone();
        BlurRadiusSpec::shader(5.dp(), move |density, size| {
            *seen.lock().unwrap() = Some((*density, size));
            LinearGradientShader(o(0.0, 0.0), o(size.width, 0.0), &[Color::Transparent, Color::Black], None, CLAMP)
        })
    };
    let at = Density::new(2.5, 1.25);
    let by_shader = masked.create_render_effect(Size::new(80.0, 40.0), TileMode::Decal, &at);
    let p = progressive(&by_shader).unwrap();
    let want = LinearGradientShader(o(0.0, 0.0), o(80.0, 0.0), &[Color::Transparent, Color::Black], None, CLAMP);
    assert!(p.shader == Some(want) && p.stops == vec![(0.0, 12.5), (1.0, 12.5)]);
    assert_eq!(*seen.lock().unwrap(), Some((at, Size::new(80.0, 40.0))));
    assert_eq!(edges(&by_shader), Some(TileMode::Decal));
    assert_eq!(by_shader.output_bounds(xywh(0.0, 0.0, 80.0, 40.0)), xywh(-13.5, -13.5, 107.0, 67.0));
    assert!(progressive(&down).unwrap().shader.is_none());
    let empty = BlurRadiusSpec::shader(0.dp(), |_, _| SweepGradientShader(o(0.0, 0.0), &[RED, BLUE], None));
    assert_eq!(empty.create_render_effect(Size::new(10.0, 10.0), CLAMP, &density), OffsetEffect(0.0, 0.0));
    let same_spec = masked.clone();
    assert!(same_spec == masked && masked != BlurRadiusSpec::uniform(5.dp()));
    assert_ne!(masked, BlurRadiusSpec::shader(5.dp(), |_, _| SweepGradientShader(o(0.0, 0.0), &[RED, BLUE], None)));
}

#[test]
fn filter_qualities_are_composes() {
    assert_eq!(DefaultFilterQuality, FilterQuality::Low);
    assert_eq!(
        [FilterQuality::None, FilterQuality::Low, FilterQuality::Medium, FilterQuality::High].map(|q| q.value()),
        [0, 1, 2, 3]
    );
}
