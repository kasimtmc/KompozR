//! Images, shaders, meshes and bitmaps (the C++ Kompoz's imagesAreSampledAsAsked, shadersPaintAsTheyAre,
//! meshesDrawAsSkiaDoes, bitmapsAreDrawnIntoAsAndroidDoes, vectorsDrawAsCompose and the images scene), each against
//! a model worked out on the CPU: images sampled as their FilterQuality says, shaders as Shader::color_at says,
//! meshes as Skia fills them, bitmaps drawn into as Android draws into them.
//!
//! What draws into bitmaps is a renderer of its own on the one device, shared (`Rc<RefCell<Renderer>>`) with the
//! [`RendererBitmaps`] set as kompoz-ui's bitmap renderer: a canvas over a bitmap going while a frame is drawn finds
//! the renderer busy, and what it drew waits in it (as C++'s BitmapHook does it `inFrame`).

use crate::exact::{none, worst_against};
use crate::golden::matches_golden;
use crate::harness::{Gpu, Pixels, near, render_by};
use crate::scenes::xywh;
use kompoz_gpu::Renderer;
use kompoz_gpu::device::ClearColor;
use kompoz_gpu::renderer::RendererBitmaps;
use kompoz_graphics::colorspace::ColorSpaces;
use kompoz_graphics::{
    BlendMode, Brush, Color, ColorFilter, CompositeShader, Density, FilterQuality, ImageBitmap, ImageBitmapConfig,
    ImageShader, ImageVector, LayoutDirection, LinearGradientShader, Matrix, Offset, Path, PathDirection, PathFillType,
    PathStroke, RadialGradientShader, Rect, Shader, ShaderBrush, ShaderKind, Size, SolidColor, TileMode, UnitExt,
    VectorGroup, VectorPath, VertexMode, Vertices, add_path_nodes, blend, to_path,
};
use kompoz_ui::draw::{BitmapRenderer, Canvas, DrawScope, Paint, Picture, draw_vector, set_bitmap_renderer};
use kompoz_ui::graphics_context::{ClipOp, CornerRadii, GraphicsContext};
use std::cell::RefCell;
use std::rc::Rc;

/// A picture of floats, premultiplied RGBA (C++'s Image).
struct Image {
    w: i32,
    rgba: Vec<f32>,
    h: i32,
}

impl Image {
    fn new(w: i32, h: i32) -> Image {
        Image { w, h, rgba: vec![0.0; (w * h * 4) as usize] }
    }

    fn at(&mut self, x: i32, y: i32) -> &mut [f32] {
        let i = ((y * self.w + x) * 4) as usize;
        &mut self.rgba[i..i + 4]
    }
}

/// The picture over white, 8 bits a channel (C++'s overWhite).
fn over_white(image: &Image) -> Pixels {
    let mut rgba = vec![0u8; (image.w * image.h * 4) as usize];
    for (o, q) in rgba.as_chunks_mut::<4>().0.iter_mut().zip(image.rgba.as_chunks::<4>().0) {
        for c in 0..3 {
            o[c] = ((q[c] + (1.0 - q[3])).clamp(0.0, 1.0) * 255.0).round() as u8;
        }
        o[3] = 255;
    }
    Pixels { width: image.w, height: image.h, rgba }
}

/// A color premultiplied, as floats.
fn premultiplied(c: Color) -> [f32; 4] {
    [c.red() * c.alpha(), c.green() * c.alpha(), c.blue() * c.alpha(), c.alpha()]
}

/// The 16 x 16 checker the images scene draws: blocks of 4, its right half at half alpha.
fn checker() -> ImageBitmap {
    let mut px = vec![0u8; 16 * 16 * 4];
    for y in 0..16 {
        for x in 0..16 {
            let p = &mut px[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4];
            let on = ((x / 4) + (y / 4)) % 2 == 0;
            let rgb = if on { [230, 120, 20] } else { [30, 30, 160] };
            p.copy_from_slice(&[rgb[0], rgb[1], rgb[2], if x < 8 { 255 } else { 128 }]);
        }
    }
    ImageBitmap::from_pixels(16, 16, px).unwrap()
}

thread_local! {
    static CHECKER: ImageBitmap = checker();
}

/// The images scene: a checker scaled up, at fractions, part of it at half alpha, scaled down.
pub fn images(c: &mut Renderer) {
    let image = CHECKER.with(|i| i.clone());
    let q = kompoz_graphics::DefaultFilterQuality;
    c.draw_image(&image, xywh(0.0, 0.0, 16.0, 16.0), xywh(8.0, 8.0, 64.0, 64.0), 1.0, q); // scaled up
    c.draw_image(&image, xywh(0.0, 0.0, 16.0, 16.0), xywh(80.5, 8.5, 16.0, 16.0), 1.0, q); // fractions
    c.draw_image(&image, xywh(4.0, 4.0, 8.0, 8.0), xywh(110.0, 8.0, 70.0, 70.0), 0.5, q); // part of it, half alpha
    c.draw_image(&image, xywh(0.0, 0.0, 16.0, 16.0), xywh(8.0, 90.0, 8.0, 8.0), 1.0, q); // scaled down
}

/// The images scene against its picture.
pub fn images_match_their_picture(g: &mut Gpu) {
    let p = g.render(192, 192, images);
    matches_golden("images", &p);
}

/// Mitchell's cubic (B = C = 1/3), as Skia's High samples.
fn mitchell(x: f32) -> f32 {
    let x = x.abs();
    if x < 1.0 {
        (7.0 * x * x * x - 12.0 * x * x + 16.0 / 3.0) / 6.0
    } else if x < 2.0 {
        (-7.0 / 3.0 * x * x * x + 12.0 * x * x - 20.0 * x + 32.0 / 3.0) / 6.0
    } else {
        0.0
    }
}

/// Images are sampled as their FilterQuality says, their texels premultiplied (a transparent texel's color doesn't
/// bleed), against models on the CPU: the nearest texel; bilinear (and both in one target); from the mipmap level the
/// size takes (a 16 x 16 drawn 4 x 4: each pixel its 4 x 4 block's mean; between two, the nearer); Mitchell's cubic
/// of the sixteen nearest. Hairlines are a pixel wide through any scale.
pub fn images_are_sampled_as_asked(g: &mut Gpu) {
    let (iw, ih, n) = (4i32, 4i32, 32i32);
    let mut px = vec![0u8; (iw * ih * 4) as usize];
    for i in 0..(iw * ih) as usize {
        let q = &mut px[i * 4..i * 4 + 4];
        let clear = i % 5 == 2; // transparent, green underneath
        q[0] = if clear { 0 } else { (40 + i * 13) as u8 };
        q[1] = if clear { 255 } else { (200 - i * 9) as u8 };
        q[2] = if clear { 0 } else { (i * 15) as u8 };
        q[3] = if clear {
            0
        } else if i % 3 == 0 {
            255
        } else {
            128
        };
    }
    let image = ImageBitmap::from_pixels(iw, ih, px.clone()).unwrap();
    // Premultiplied, as uploaded (clamped to the edge).
    let texel = |x: i32, y: i32| {
        let (x, y) = (x.clamp(0, iw - 1), y.clamp(0, ih - 1));
        let q = &px[((y * iw + x) * 4) as usize..];
        let mut out = [0.0f32; 4];
        for c in 0..3 {
            out[c] = ((q[c] as u32 * q[3] as u32 + 127) / 255) as f32 / 255.0;
        }
        out[3] = q[3] as f32 / 255.0;
        out
    };
    // u, v in texels.
    let modelled = |sample: &dyn Fn(f32, f32) -> [f32; 4]| {
        let mut out = Image::new(n, n);
        for y in 0..n {
            for x in 0..n {
                let c = sample((x as f32 + 0.5) * iw as f32 / n as f32, (y as f32 + 0.5) * ih as f32 / n as f32);
                out.at(x, y).copy_from_slice(&c);
            }
        }
        over_white(&out)
    };
    let all = xywh(0.0, 0.0, iw as f32, ih as f32);
    let nearest = modelled(&|u, v| texel(u.floor() as i32, v.floor() as i32));
    let bilinear = modelled(&|u, v| {
        let (x, y) = (u - 0.5, v - 0.5);
        let (fx, fy) = (x - x.floor(), y - y.floor());
        let (x0, y0) = (x.floor() as i32, y.floor() as i32);
        let (a, b, c, d) = (texel(x0, y0), texel(x0 + 1, y0), texel(x0, y0 + 1), texel(x0 + 1, y0 + 1));
        std::array::from_fn(|k| (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy)
    });
    let cubic = modelled(&|u, v| {
        let (x, y) = (u - 0.5, v - 0.5);
        let (x0, y0) = (x.floor() as i32, y.floor() as i32);
        let mut out = [0.0f32; 4];
        for j in -1..=2 {
            for i in -1..=2 {
                let w = mitchell((x0 + i) as f32 - x) * mitchell((y0 + j) as f32 - y);
                for (o, t) in out.iter_mut().zip(texel(x0 + i, y0 + j)) {
                    *o += t * w;
                }
            }
        }
        out[3] = out[3].clamp(0.0, 1.0);
        for c in 0..3 {
            out[c] = out[c].clamp(0.0, out[3]);
        }
        out
    });
    let cases = [(FilterQuality::None, &nearest), (FilterQuality::Low, &bilinear), (FilterQuality::High, &cubic)];
    for (quality, want) in cases {
        let got = g.render(n, n, |c| c.draw_image(&image, all, xywh(0.0, 0.0, n as f32, n as f32), 1.0, quality));
        let (worst, at) = worst_against(&got, |x, y| want.at(x, y), none);
        check!(worst <= 2, "  image sampled at quality {quality:?}: off by {worst} at {at:?}");
    }
    // Twice in one target, nearest then bilinear (the same texture, a sampler each): each as alone.
    let both = g.render(2 * n, n, |c| {
        c.draw_image(&image, all, xywh(0.0, 0.0, n as f32, n as f32), 1.0, FilterQuality::None);
        c.draw_image(&image, all, xywh(n as f32, 0.0, n as f32, n as f32), 1.0, FilterQuality::Low);
    });
    let (worst, at) = worst_against(&both, |x, y| if x < n { nearest.at(x, y) } else { bilinear.at(x - n, y) }, none);
    check!(worst <= 2, "  an image nearest and bilinear in one target: off by {worst} at {at:?}");
    // Mipmaps: a 16 x 16 drawn 4 x 4 at Medium, each pixel its block's mean (not at Low: the four texels in its
    // middle).
    let big = 16i32;
    let mut fine = vec![0u8; (big * big * 4) as usize];
    for y in 0..big {
        for x in 0..big {
            let q = &mut fine[((y * big + x) * 4) as usize..((y * big + x) * 4 + 4) as usize];
            // Each block's middle (where Low reads).
            let on = (x % 4 == 1 || x % 4 == 2) && (y % 4 == 1 || y % 4 == 2);
            q.copy_from_slice(&[if on { 255 } else { 0 }, (x * 16) as u8, (y * 16) as u8, if on { 255 } else { 64 }]);
        }
    }
    let checkered = ImageBitmap::from_pixels(big, big, fine.clone()).unwrap();
    let mut means = Image::new(4, 4);
    for y in 0..4 {
        for x in 0..4 {
            for j in 0..4 {
                for i in 0..4 {
                    let q = &fine[(((y * 4 + j) * big + x * 4 + i) * 4) as usize..];
                    let m = means.at(x, y);
                    for c in 0..3 {
                        m[c] += ((q[c] as u32 * q[3] as u32 + 127) / 255) as f32 / 255.0 / 16.0;
                    }
                    m[3] += q[3] as f32 / 255.0 / 16.0;
                }
            }
        }
    }
    let want_means = over_white(&means);
    let whole = xywh(0.0, 0.0, big as f32, big as f32);
    let mut small =
        |q: FilterQuality| g.render(4, 4, |c| c.draw_image(&checkered, whole, xywh(0.0, 0.0, 4.0, 4.0), 1.0, q));
    let (worst, at) = worst_against(&small(FilterQuality::Medium), |x, y| want_means.at(x, y), none);
    check!(worst <= 3, "  a mipmapped image: off by {worst} at {at:?}");
    let (low, _) = worst_against(&small(FilterQuality::Low), |x, y| want_means.at(x, y), none);
    check!(low > 20, "  a 16 x 16 drawn 4 x 4 at Low: only {low} off its blocks' means");
    // Between two levels, the nearer one's alone (not the two blended): a checker of 2 x 2 blocks (its first
    // smaller level a checker of texels, the next one color) drawn 6 x 6, bilinearly from that first level.
    let colors = [[26u8, 64, 217], [230, 38, 26]];
    let mut blocks = vec![0u8; (big * big * 4) as usize];
    for y in 0..big {
        for x in 0..big {
            let q = &mut blocks[((y * big + x) * 4) as usize..((y * big + x) * 4 + 4) as usize];
            q[..3].copy_from_slice(&colors[((x / 2 + y / 2) % 2) as usize]);
            q[3] = 255;
        }
    }
    let blocky = ImageBitmap::from_pixels(big, big, blocks).unwrap();
    let mut nearer = Image::new(6, 6);
    for y in 0..6 {
        for x in 0..6 {
            let (u, v) = ((x as f32 + 0.5) * 8.0 / 6.0 - 0.5, (y as f32 + 0.5) * 8.0 / 6.0 - 0.5);
            let (i0, j0) = (u.floor() as i32, v.floor() as i32);
            let o = nearer.at(x, y);
            for dj in 0..2 {
                for di in 0..2 {
                    let (i, j) = ((i0 + di).clamp(0, 7), (j0 + dj).clamp(0, 7));
                    let wx = if di == 1 { u - i0 as f32 } else { 1.0 - (u - i0 as f32) };
                    let wy = if dj == 1 { v - j0 as f32 } else { 1.0 - (v - j0 as f32) };
                    for c in 0..3 {
                        o[c] += wx * wy * colors[((i + j) % 2) as usize][c] as f32 / 255.0;
                    }
                }
            }
            o[3] = 1.0;
        }
    }
    let want_nearer = over_white(&nearer);
    let got = g.render(6, 6, |c| c.draw_image(&blocky, whole, xywh(0.0, 0.0, 6.0, 6.0), 1.0, FilterQuality::Medium));
    let (worst, at) = worst_against(&got, |x, y| want_nearer.at(x, y), none);
    check!(worst <= 3, "  a mipmapped image between levels: off by {worst} at {at:?}");
    // Hairlines: a pixel wide, enlarged three times (by a transform), turned or not; a 1 px stroke three.
    for turn in [0.0f32, 30.0] {
        for hairline in [true, false] {
            let mut line = Path::new();
            line.move_to(2.0, 10.5);
            line.line_to(30.0, 10.5);
            let stroke = if hairline {
                PathStroke { width: 0.0, hairline: true, ..PathStroke::default() }
            } else {
                PathStroke { width: 1.0, ..PathStroke::default() }
            };
            let got = g.render(96, 64, |c| {
                let mut m = Matrix::new();
                m.scale(3.0, 3.0, 1.0);
                m.translate(16.0, 10.5, 0.0);
                m.rotate_z(turn);
                m.translate(-16.0, -10.5, 0.0);
                c.push_transform(&m);
                c.draw_path(&line, Offset::Zero, &SolidColor(Color::Black), Rect::Zero, &stroke);
                c.pop_transform();
            });
            // Ink across the line at its middle (48, 31.5 px), perpendicular to it.
            let (mut ink, rad) = (0.0f32, turn.to_radians());
            let mut t = -6.0f32;
            while t <= 6.0 {
                let (x, y) = (48.0 - rad.sin() * t, 31.5 + rad.cos() * t);
                ink += (255 - got.at(x as i32, y as i32)[0] as i32) as f32 / 255.0 * 0.05;
                t += 0.05;
            }
            let want = if hairline { 1.0 } else { 3.0 };
            check!(
                (ink - want).abs() < 0.25,
                "  {} turned {turn}: {ink} px of ink across, not {want}",
                if hairline { "a hairline" } else { "a stroke" }
            );
        }
    }
}

/// A matrix of these steps (each before the one above it).
fn steps(change: impl FnOnce(&mut Matrix)) -> Matrix {
    let mut m = Matrix::new();
    change(&mut m);
    m
}

/// The 4 x 3 tile the shader and mesh tests paint with.
fn tile() -> ImageBitmap {
    let mut px = vec![0u8; 4 * 3 * 4];
    for i in 0..12 {
        let q = &mut px[i * 4..i * 4 + 4];
        q.copy_from_slice(&[
            (30 + i * 19) as u8,
            (220 - i * 15) as u8,
            (i % 3 * 110) as u8,
            if i % 4 == 1 { 128 } else { 255 },
        ]);
    }
    ImageBitmap::from_pixels(4, 3, px).unwrap()
}

/// Shaders paint as they say, against Shader::color_at on the CPU at each pixel's middle: an image tiled (repeated
/// across, mirrored down) and its decal edge; a gradient through a turned, stretched transform; two composited
/// (multiplied: baked, then painted; two images); an image shader on a path drawn turned.
pub fn shaders_paint_as_they_are(g: &mut Gpu) {
    let n = 64;
    let tile = tile();
    let model = |at: &dyn Fn(Offset) -> Color, inside: &dyn Fn(Offset) -> bool| {
        let mut out = Image::new(n, n);
        for y in 0..n {
            for x in 0..n {
                let p = Offset::new(x as f32 + 0.5, y as f32 + 0.5);
                if inside(p) {
                    out.at(x, y).copy_from_slice(&premultiplied(at(p)));
                }
            }
        }
        over_white(&out)
    };
    let everywhere = |_: Offset| true;
    let area = xywh(0.0, 0.0, n as f32, n as f32);
    let mut check = |what: &str, brush: &Brush, want: &Pixels, tolerance: i32| {
        let got = g.render(n, n, |c| c.fill_gradient(area, CornerRadii::Zero, brush, area));
        let (worst, at) = worst_against(&got, |x, y| want.at(x, y), none);
        check!(worst <= tolerance, "  {what}: off by {worst} at {at:?}");
    };
    // An image tiled: repeated across, mirrored down; decal: nothing past it.
    let tiled = ImageShader(tile.clone(), TileMode::Repeated, TileMode::Mirror);
    check("an image shader", &ShaderBrush(tiled.clone()), &model(&|p| tiled.color_at(p), &everywhere), 2);
    let decal = ImageShader(tile.clone(), TileMode::Decal, TileMode::Decal).transformed(&steps(|m| {
        m.translate(20.0, 20.0, 0.0);
        m.scale(5.0, 5.0, 1.0);
    }));
    check("a decal image shader", &ShaderBrush(decal.clone()), &model(&|p| decal.color_at(p), &everywhere), 2);
    for (across, down) in [(TileMode::Decal, TileMode::Repeated), (TileMode::Mirror, TileMode::Decal)] {
        let half = ImageShader(tile.clone(), across, down).transformed(&steps(|m| {
            m.translate(20.0, 14.0, 0.0);
            m.scale(3.0, 4.0, 1.0);
        }));
        check(
            "an image shader decal one way",
            &ShaderBrush(half.clone()),
            &model(&|p| half.color_at(p), &everywhere),
            2,
        );
    }
    // A gradient through a turned, stretched transform (the brush's).
    let turned = steps(|m| {
        m.translate(32.0, 32.0, 0.0);
        m.rotate_z(30.0);
        m.scale(1.5, 0.6, 1.0);
        m.translate(-32.0, -32.0, 0.0);
    });
    let plain = Brush::linear_gradient(
        &[Color::new(0.9, 0.1, 0.2, 1.0), Color::new(0.1, 0.4, 0.9, 1.0), Color::new(0.2, 0.8, 0.3, 0.6)],
        Offset::new(16.0, 0.0),
        Offset::new(48.0, 0.0),
        TileMode::Mirror,
    );
    let stretched = plain.with_transform(Some(turned));
    let mut untransformed = turned;
    untransformed.invert();
    check(
        "a transformed gradient",
        &stretched,
        &model(&|p| plain.color_at(untransformed.map(p), area), &everywhere),
        2,
    );
    // An image through a perspective (the shader's): each pixel where the plane takes it.
    let mut leaning = steps(|m| m.scale(4.0, 4.0, 1.0));
    leaning.set(0, 3, 0.004);
    let slanted = ImageShader(tile.clone(), TileMode::Repeated, TileMode::Mirror).transformed(&leaning);
    check(
        "an image shader in perspective",
        &ShaderBrush(slanted.clone()),
        &model(&|p| slanted.color_at(p), &everywhere),
        3,
    );
    // Two composited: a radial gradient multiplied by the image.
    let radial = |a: f32, b: f32| {
        Brush::radial_gradient(
            &[Color::new(1.0, 0.9, 0.2, a), Color::new(0.2, 0.3, 1.0, b)],
            Offset::Unspecified,
            f32::INFINITY,
            TileMode::Clamp,
        )
    };
    let multiplied = Brush::composite(&radial(1.0, 1.0), &ShaderBrush(tiled.clone()), BlendMode::Multiply);
    check("a composite shader", &multiplied, &model(&|p| multiplied.color_at(p, area), &everywhere), 3);
    // A mode that isn't the same either way round, and one color as one of the two.
    let atop = Brush::composite(&radial(0.9, 0.3), &ShaderBrush(tiled.clone()), BlendMode::SrcAtop);
    check("a composite, source atop", &atop, &model(&|p| atop.color_at(p, area), &everywhere), 3);
    let one_color = ShaderBrush(CompositeShader(
        LinearGradientShader(
            Offset::Zero,
            Offset::new(1.0, 0.0),
            &[Color::new(0.2, 0.6, 0.9, 0.8)],
            Some(&[0.0]),
            TileMode::Clamp,
        ),
        ImageShader(tile.clone(), TileMode::Repeated, TileMode::Repeated),
        BlendMode::Overlay,
    ));
    check(
        "a composite of one color and an image",
        &one_color,
        &model(&|p| one_color.color_at(p, area), &everywhere),
        3,
    );
    // Two images of their own sizes, the source's decal edge faded over its own texels.
    let mut wide_px = vec![0u8; 4 * 6 * 5];
    for i in 0..30 {
        wide_px[i * 4..i * 4 + 4].copy_from_slice(&[
            (200 - i * 6) as u8,
            (i * 8) as u8,
            (90 + i % 5 * 30) as u8,
            if i % 3 == 0 { 160 } else { 255 },
        ]);
    }
    let wide = ImageShader(ImageBitmap::from_pixels(6, 5, wide_px).unwrap(), TileMode::Decal, TileMode::Decal)
        .transformed(&steps(|m| {
            m.translate(14.0, 10.0, 0.0);
            m.scale(6.0, 7.0, 1.0);
        }));
    let two_images = ShaderBrush(CompositeShader(
        ImageShader(tile.clone(), TileMode::Repeated, TileMode::Repeated),
        wide,
        BlendMode::SrcOver,
    ));
    check(
        "a composite of two images, one decal",
        &two_images,
        &model(&|p| two_images.color_at(p, area), &everywhere),
        3,
    );
    // A composite of composites (baked, then painted from its texture): the image multiplied, then screened by a
    // gradient.
    let nested = Brush::composite(
        &multiplied,
        &Brush::linear_gradient(
            &[Color::new(0.2, 0.1, 0.6, 0.8), Color::new(0.9, 0.9, 0.2, 0.4)],
            Offset::Zero,
            Offset::new(64.0, 64.0),
            TileMode::Clamp,
        ),
        BlendMode::Screen,
    );
    check("a composite of composites", &nested, &model(&|p| nested.color_at(p, area), &everywhere), 3);
    // An image shader on a path, drawn turned: where each pixel is in the path's space, then in the image.
    let mut disc = Path::new();
    disc.add_oval(xywh(8.0, 8.0, 48.0, 48.0), PathDirection::CounterClockwise);
    let spin = steps(|m| {
        m.translate(32.0, 32.0, 0.0);
        m.rotate_z(20.0);
        m.translate(-32.0, -32.0, 0.0);
    });
    let mut back = spin;
    back.invert();
    let big = ShaderBrush(
        ImageShader(tile.clone(), TileMode::Repeated, TileMode::Repeated)
            .transformed(&steps(|m| m.scale(3.0, 3.0, 1.0))),
    );
    let on_path = g.render(n, n, |c| {
        c.push_transform(&spin);
        c.draw_path(&disc, Offset::Zero, &big, area, &PathStroke::default());
        c.pop_transform();
    });
    let want_path = model(&|p| big.color_at(back.map(p), area), &|p| {
        let q = back.map(p);
        (q.x - 32.0).hypot(q.y - 32.0) < 23.5
    });
    let (worst, at) = worst_against(
        &on_path,
        |x, y| want_path.at(x, y),
        |x, y| {
            let q = back.map(Offset::new(x as f32 + 0.5, y as f32 + 0.5));
            ((q.x - 32.0).hypot(q.y - 32.0) - 24.0).abs() < 1.5 // (its edge: coverage)
        },
    );
    check!(worst <= 3, "  an image shader on a turned path: off by {worst} at {at:?}");
}

/// A mesh drawn, and what is expected of it.
struct MeshCase {
    what: &'static str,
    vertices: Vertices,
    mode: BlendMode,
    paint: Paint,
    transform: Option<Matrix>,
    clip: Option<Rect>,
    /// The target's px a unit of the mesh's (its transform's scale).
    scale: f64,
}

/// Meshes as Skia draws them (Canvas.drawVertices): each pixel whose middle a triangle holds, not anti-aliased; the
/// paint where the texture coordinate is (or its color, opaque) through the vertex colors (premultiplied between
/// them) by the mode, the paint the source; faded; the triangles over each other in turn. Worked out here per pixel,
/// the texture coordinate and color where the pixel is in the mesh's own plane (through the transform taken back: in
/// perspective too). Pixel middles within a sixteenth of a pixel of an edge are left out (either side is right).
///
/// (Compose's Vertices always have texture coordinates and colors: C++'s meshes without colors have transparent ones
/// here, which SrcOver leaves the paint as it is; C++'s at 2 px a dp are drawn through a canvas scaled twice.)
pub fn meshes_draw_as_skia_does(g: &mut Gpu) {
    let n = 64;
    let tile = tile();
    let model = |c: &MeshCase| {
        let mut out = Image::new(n, n);
        let mut edgy = vec![false; (n * n) as usize];
        let mut back = c.transform.unwrap_or_default();
        back.invert();
        let v = &c.vertices;
        let solid = c.paint.shader.is_none();
        for y in 0..n {
            for x in 0..n {
                let q = back.map(Offset::new(x as f32 + 0.5, y as f32 + 0.5));
                if let Some(clip) = c.clip {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    if px < clip.left || px > clip.right || py < clip.top || py > clip.bottom {
                        continue;
                    }
                }
                for tri in v.triangles() {
                    let (a, b, d) = (v.position(tri[0]), v.position(tri[1]), v.position(tri[2]));
                    let twice = (b.x - a.x) as f64 * (d.y - a.y) as f64 - (b.y - a.y) as f64 * (d.x - a.x) as f64;
                    if twice == 0.0 {
                        continue;
                    }
                    let e = |s0: Offset, s1: Offset| {
                        ((s1.x - s0.x) as f64 * (q.y - s0.y) as f64 - (s1.y - s0.y) as f64 * (q.x - s0.x) as f64)
                            / twice
                    };
                    let w = [e(b, d), e(d, a), e(a, b)];
                    // (How far from an edge, in the target's pixels, roughly: barycentric times the edge's height.)
                    let inside = w.iter().all(|&w| w >= 0.0);
                    let pts = [a, b, d];
                    let mut margin = 1e30f64;
                    for k in 0..3 {
                        let (s0, s1) = (pts[(k + 1) % 3], pts[(k + 2) % 3]);
                        let len = ((s1.x - s0.x) as f64).hypot((s1.y - s0.y) as f64);
                        margin = margin.min((w[k] * twice / len.max(1e-9)).abs());
                    }
                    // (Within a sixteenth of a pixel: a GPU may snap a vertex that far, at 4 subpixel bits, the
                    // least Vulkan allows.)
                    if margin * c.scale < 1.0 / 16.0 {
                        edgy[(y * n + x) as usize] = true;
                    }
                    if !inside {
                        continue;
                    }
                    let src = if solid {
                        let p = c.paint.color;
                        Color::new(p.red(), p.green(), p.blue(), 1.0)
                    } else {
                        let mut t = Offset::Zero;
                        for k in 0..3 {
                            let tk = v.texture_coordinate(tri[k]);
                            t = Offset::new(t.x + w[k] as f32 * tk.x, t.y + w[k] as f32 * tk.y);
                        }
                        c.paint.shader.as_ref().unwrap().color_at(t)
                    };
                    let mut p4 = [0.0f64; 4];
                    for k in 0..3 {
                        let ck = Color::from_argb(v.colors()[tri[k]]);
                        let a = ck.alpha() as f64;
                        p4[0] += w[k] * ck.red() as f64 * a;
                        p4[1] += w[k] * ck.green() as f64 * a;
                        p4[2] += w[k] * ck.blue() as f64 * a;
                        p4[3] += w[k] * a;
                    }
                    let dst = if p4[3] > 0.0 {
                        Color::new((p4[0] / p4[3]) as f32, (p4[1] / p4[3]) as f32, (p4[2] / p4[3]) as f32, p4[3] as f32)
                    } else {
                        Color::Transparent
                    };
                    let col = blend(src, dst, c.mode);
                    let alpha = col.alpha() * c.paint.alpha();
                    let o = out.at(x, y);
                    o[0] = col.red() * alpha + o[0] * (1.0 - alpha);
                    o[1] = col.green() * alpha + o[1] * (1.0 - alpha);
                    o[2] = col.blue() * alpha + o[2] * (1.0 - alpha);
                    o[3] = alpha + o[3] * (1.0 - alpha);
                }
            }
        }
        (over_white(&out), edgy)
    };
    let (red, green, blue, gold) = (
        Color::new(0.9, 0.1, 0.2, 1.0),
        Color::new(0.1, 0.8, 0.3, 1.0),
        Color::new(0.1, 0.3, 0.9, 0.7),
        Color::new(1.0, 0.7, 0.1, 1.0),
    );
    let clear = Color::Transparent;
    let white = Paint { color: Color::White, ..Paint::default() };
    let shaded = |shader: Shader| Paint { shader: Some(shader), ..Paint::default() };
    let mut tiled = shaded(ImageShader(tile.clone(), TileMode::Repeated, TileMode::Mirror));
    tiled.set_alpha(0.6);
    let ramp = shaded(LinearGradientShader(
        Offset::new(4.0, 0.0),
        Offset::new(60.0, 0.0),
        &[red, gold, blue],
        None,
        TileMode::Mirror,
    ));
    let composite = shaded(CompositeShader(
        LinearGradientShader(Offset::Zero, Offset::new(0.0, 20.0), &[red, blue], None, TileMode::Clamp),
        ImageShader(tile.clone(), TileMode::Repeated, TileMode::Repeated),
        BlendMode::Screen,
    ));
    let nested = shaded(CompositeShader(
        CompositeShader(
            RadialGradientShader(Offset::new(10.0, 10.0), 14.0, &[red, blue], None, TileMode::Clamp),
            LinearGradientShader(Offset::Zero, Offset::new(20.0, 0.0), &[gold, green], None, TileMode::Clamp),
            BlendMode::Multiply,
        ),
        LinearGradientShader(
            Offset::Zero,
            Offset::new(0.0, 20.0),
            &[Color::new(1.0, 1.0, 1.0, 0.2), Color::new(0.0, 0.0, 0.0, 0.6)],
            None,
            TileMode::Clamp,
        ),
        BlendMode::Overlay,
    ));
    let mut leaning = steps(|m| m.translate(32.0, 32.0, 0.0));
    leaning.set(0, 3, 0.004);
    leaning.rotate_z(10.0);
    leaning.translate(-32.0, -32.0, 0.0);
    let o = |x: f32, y: f32| Offset::new(x, y);
    let fan = [o(32.2, 31.7), o(5.3, 6.1), o(58.6, 4.4), o(60.1, 58.3), o(4.7, 59.2), o(5.3, 6.1)];
    let triangle = [o(4.3, 5.1), o(59.2, 7.4), o(31.6, 58.3)];
    let cases = [
        MeshCase {
            what: "vertex colors, modulated",
            vertices: {
                let at = [o(3.3, 4.2), o(60.4, 6.1), o(58.7, 59.6), o(5.2, 57.3)];
                Vertices::new(VertexMode::Triangles, &at, &at, &[red, green, blue, gold], &[0, 1, 2, 0, 2, 3])
            },
            mode: BlendMode::Modulate,
            paint: white.clone(),
            transform: None,
            clip: None,
            scale: 1.0,
        },
        MeshCase {
            what: "an image through colors, multiplied, a strip",
            vertices: Vertices::new(
                VertexMode::TriangleStrip,
                &[o(4.3, 10.2), o(6.2, 50.7), o(30.1, 8.4), o(33.6, 55.3), o(59.8, 12.3), o(57.1, 52.6)],
                &[o(0.0, 0.0), o(0.0, 12.0), o(8.0, 0.0), o(8.0, 12.0), o(16.0, 0.0), o(16.0, 12.0)],
                &[red, green, blue, gold, red, green],
                &[],
            ),
            mode: BlendMode::Multiply,
            paint: tiled.clone(),
            transform: None,
            clip: None,
            scale: 1.0,
        },
        MeshCase {
            what: "a gradient where the pixels are, a fan, in perspective",
            vertices: Vertices::new(VertexMode::TriangleFan, &fan, &fan, &[clear; 6], &[]),
            mode: BlendMode::SrcOver,
            paint: ramp,
            transform: Some(leaning),
            clip: None,
            scale: 1.0,
        },
        MeshCase {
            what: "a composite, clipped, overlapping",
            vertices: Vertices::new(
                VertexMode::Triangles,
                &[o(2.3, 3.1), o(61.2, 9.4), o(12.6, 60.3), o(58.4, 2.7), o(54.3, 61.1), o(3.6, 33.2)],
                &[o(0.0, 0.0), o(20.0, 0.0), o(0.0, 20.0), o(20.0, 0.0), o(20.0, 20.0), o(0.0, 10.0)],
                &[gold, gold, gold, blue, blue, blue],
                &[],
            ),
            mode: BlendMode::Darken,
            paint: composite,
            transform: None,
            clip: Some(xywh(8.0, 8.0, 48.0, 44.0)),
            scale: 1.0,
        },
        MeshCase {
            what: "a composite of composites (baked as finely as it is spread)",
            vertices: Vertices::new(
                VertexMode::Triangles,
                &triangle,
                &[o(0.0, 0.0), o(20.0, 0.0), o(10.0, 20.0)],
                &[clear; 3],
                &[],
            ),
            mode: BlendMode::SrcOver,
            paint: nested,
            transform: None,
            clip: None,
            scale: 1.0,
        },
        MeshCase {
            what: "an image through colors, scaled twice",
            vertices: Vertices::new(
                VertexMode::Triangles,
                &[o(2.3, 3.1), o(30.2, 4.4), o(6.6, 29.3)],
                &[o(0.0, 0.0), o(12.0, 0.0), o(0.0, 9.0)],
                &[red, green, blue],
                &[],
            ),
            mode: BlendMode::Multiply,
            paint: tiled,
            transform: Some(steps(|m| m.scale(2.0, 2.0, 1.0))),
            clip: None,
            scale: 2.0,
        },
    ];
    for c in &cases {
        let got = g.render(n, n, |ctx| {
            let mut canvas = Canvas::new(ctx, Offset::Zero);
            if let Some(m) = &c.transform {
                canvas.concat(m);
            }
            if let Some(clip) = c.clip {
                canvas.clip_rect(clip, ClipOp::Intersect);
            }
            canvas.draw_vertices(&c.vertices, c.mode, &c.paint);
        });
        let (want, edgy) = model(c);
        let (worst, at) = worst_against(&got, |x, y| want.at(x, y), |x, y| edgy[(y * n + x) as usize]);
        let tolerance = if c.paint.shader.as_ref().is_some_and(|s| s.kind() == ShaderKind::Composite) { 4 } else { 3 };
        check!(worst <= tolerance, "  {}: off by {worst} at {at:?}", c.what);
    }
}

/// A renderer of its own on the one device that bitmaps are drawn into with, set as kompoz-ui's bitmap renderer.
struct Bitmaps {
    renderer: Rc<RefCell<Renderer>>,
    hook: Rc<RendererBitmaps>,
}

impl Bitmaps {
    fn new(g: &Gpu) -> Bitmaps {
        let renderer = Rc::new(RefCell::new(Renderer::new(Box::new(g.device.clone())).expect("a renderer")));
        let hook = RendererBitmaps::new(&renderer);
        set_bitmap_renderer(Some(hook.clone() as Rc<dyn BitmapRenderer>));
        Bitmaps { renderer, hook }
    }

    /// `draw` on a w x h target cleared to `clear`, in a frame of its own: what is drawn into bitmaps meanwhile waits
    /// in the renderer (busy) until it is drawn from them.
    fn render(&self, w: i32, h: i32, clear: ClearColor, draw: impl FnOnce(&mut Renderer)) -> Pixels {
        self.hook.set_in_frame(true);
        let p = render_by(&mut self.renderer.borrow_mut(), w, h, draw, clear);
        self.hook.set_in_frame(false);
        p
    }

    /// Unset: what only its renderer has read back into the bitmaps, its frame's leftovers gone.
    fn done(self) {
        set_bitmap_renderer(None);
        let mut r = self.renderer.borrow_mut();
        r.device_mut().begin_frame();
        r.end_frame();
        r.device_mut().end_frame(false);
    }
}

/// What a bitmap reads back, as straight ARGB, against `want` (each channel within `tolerance`).
fn compare(what: &str, b: &ImageBitmap, want: impl Fn(i32, i32) -> [i32; 4], tolerance: i32) {
    let (w, h) = (b.width(), b.height());
    let mut argb = vec![0u32; (w * h) as usize];
    b.read_pixels(&mut argb);
    let (mut worst, mut at) = (0, (0, 0));
    for y in 0..h {
        for x in 0..w {
            let c = argb[(y * w + x) as usize];
            let got = [(c >> 16 & 255) as i32, (c >> 8 & 255) as i32, (c & 255) as i32, (c >> 24) as i32];
            let want = want(x, y);
            for k in 0..4 {
                if (got[k] - want[k]).abs() > worst {
                    (worst, at) = ((got[k] - want[k]).abs(), (x, y));
                }
            }
        }
    }
    check!(worst <= tolerance, "  {what}: off by {worst} at {at:?}");
}

/// Bitmaps drawn into (Canvas::from_image) by the renderer, as Android draws into them: what is read back is what the
/// same drawing makes on a target of its own; an Argb8888 one kept as the renderer's texture (read back when asked
/// for), drawn into again over what it holds, drawn on the screen before and after in one frame (each time as it
/// was); F16 finer than 8 bits; Alpha8 its alpha alone (kept as a texture too), and drawn in a paint's color; Rgb565
/// and one without alpha over black, opaque, Rgb565 in 5, 6 and 5 bits.
///
/// (C++ also drew the Argb8888 one with its renderer at 2.625 px a dp, a unit a pixel still: KompozR's renderer is
/// in px, a bitmap drawn into always a unit a pixel.)
pub fn bitmaps_are_drawn_into_as_android_does(g: &mut Gpu) {
    let bitmaps = Bitmaps::new(g);
    let n = 48;
    let nf = n as f32;
    let scene = |c: &mut Canvas<'_>| {
        c.draw_rect(xywh(4.0, 4.0, 30.0, 20.0), &Paint { color: Color::new(0.9, 0.2, 0.1, 0.6), ..Paint::default() });
        let gradient = LinearGradientShader(
            Offset::new(16.0, 0.0),
            Offset::new(44.0, 0.0),
            &[Color::new(0.1, 0.6, 0.9, 1.0), Color::new(1.0, 0.9, 0.2, 0.5)],
            None,
            TileMode::Clamp,
        );
        c.draw_circle(Offset::new(30.0, 28.0), 14.0, &Paint { shader: Some(gradient), ..Paint::default() });
        let line = Paint { color: Color::new(0.2, 0.7, 0.3, 1.0), stroke_width: 3.0, ..Paint::default() };
        c.draw_line(Offset::new(2.0, 44.0), Offset::new(46.0, 38.0), &line);
    };
    // What the scene makes on a clear target of its own: straight ARGB, as read_pixels gives it. (By the renderer
    // that draws into the bitmaps, as C++'s: its paths' coverage kept, the same pixels' coverage alike to the bit,
    // where an edge pixel's alpha 1/255 apart is far apart once divided out.)
    let direct = bitmaps.render(n, n, ClearColor::TRANSPARENT, |ctx| scene(&mut Canvas::new(ctx, Offset::Zero)));
    let straight = |x: i32, y: i32| {
        let p = direct.at(x, y);
        let a = p[3];
        let un = |v: u8| if a > 0 { (v as f64 * 255.0 / a as f64).min(255.0).round() as i32 } else { 0 };
        [un(p[0]), un(p[1]), un(p[2]), a as i32]
    };
    // Argb8888: kept as the renderer's texture until it is read back.
    let argb = ImageBitmap::new(n, n, ImageBitmapConfig::Argb8888, true, &ColorSpaces::Srgb);
    let before = argb.version();
    scene(&mut Canvas::from_image(&argb));
    check!(argb.version() == before + 1 && !argb.pixels_current());
    compare("drawn into an Argb8888 bitmap", &argb, straight, 1);
    check!(argb.pixels_current());
    // Drawn into again, over what it holds; on the screen before and after, in one frame.
    let shown = ImageBitmap::new(n, n, ImageBitmapConfig::Argb8888, true, &ColorSpaces::Srgb);
    Canvas::from_image(&shown)
        .draw_rect(xywh(0.0, 0.0, nf, nf), &Paint { color: Color::new(0.2, 0.4, 0.8, 1.0), ..Paint::default() });
    let all = xywh(0.0, 0.0, nf, nf);
    let both = bitmaps.render(2 * n, n, ClearColor::WHITE, |ctx| {
        ctx.draw_image(&shown, all, all, 1.0, kompoz_graphics::DefaultFilterQuality);
        Canvas::from_image(&shown).draw_rect(
            xywh(0.0, 0.0, nf, nf / 2.0),
            &Paint { color: Color::new(0.9, 0.1, 0.1, 1.0), ..Paint::default() },
        );
        ctx.draw_image(&shown, all, xywh(nf, 0.0, nf, nf), 1.0, kompoz_graphics::DefaultFilterQuality);
    });
    check!(
        near(both.at(10, 10), 51, 102, 204, 2)
            && near(both.at(n + 10, 10), 230, 26, 26, 2)
            && near(both.at(n + 10, 40), 51, 102, 204, 2),
        "  drawn into in a frame: {:?} {:?} {:?}",
        both.at(10, 10),
        both.at(n + 10, 10),
        both.at(n + 10, 40)
    );
    // F16: finer than 8 bits (a color finer than them: one in extended sRGB, its components half floats).
    let fine = ImageBitmap::new(n, n, ImageBitmapConfig::F16, true, &ColorSpaces::Srgb);
    let finer = Color::new_in(0.5004, 0.2, 0.9, 1.0, &ColorSpaces::ExtendedSrgb);
    Canvas::from_image(&fine).draw_rect(all, &Paint { color: finer, ..Paint::default() });
    compare("drawn into an F16 bitmap (8 bits of it)", &fine, |_, _| [128, 51, 230, 255], 1);
    let f = fine.floats();
    // (Within a half float's step there, 2^-11: Vulkan may round toward zero as it writes one.)
    check!(
        f.len() == (n * n * 4) as usize && (f[0] - 0.5004).abs() < 0.000489 && (f[0] - 128.0 / 255.0).abs() > 0.001,
        "  an F16 bitmap's first float: {}",
        f.first().copied().unwrap_or(f32::NAN)
    );
    // A color of another space is drawn in sRGB (Display P3's turned into it), not its numbers taken as sRGB's.
    let p3 = Color::new_in(0.5, 0.2, 0.8, 1.0, &ColorSpaces::DisplayP3);
    let in_srgb = p3.convert(&ColorSpaces::Srgb);
    let wide = g.render(8, 8, |ctx| ctx.fill_round_rect(xywh(0.0, 0.0, 8.0, 8.0), CornerRadii::Zero, p3));
    let byte = |v: f32| (v * 255.0).round() as u8;
    check!(
        near(wide.at(4, 4), byte(in_srgb.red()), byte(in_srgb.green()), byte(in_srgb.blue()), 1)
            && !near(wide.at(4, 4), byte(0.5), byte(0.2), byte(0.8), 4)
    );
    // Alpha8: its alpha alone; drawn in a paint's color.
    let mask = ImageBitmap::new(n, n, ImageBitmapConfig::Alpha8, true, &ColorSpaces::Srgb);
    scene(&mut Canvas::from_image(&mask));
    check!(!mask.pixels_current()); // (kept as the renderer's texture too, black)
    compare("drawn into an Alpha8 bitmap", &mask, |x, y| [0, 0, 0, straight(x, y)[3]], 1);
    {
        // Its texture black too (what is sampled of it as an image), drawn into again.
        let again = ImageBitmap::new(n, n, ImageBitmapConfig::Alpha8, true, &ColorSpaces::Srgb);
        scene(&mut Canvas::from_image(&again));
        let raw = bitmaps.render(n, n, ClearColor::WHITE, |ctx| {
            ctx.draw_image(&again, all, all, 1.0, kompoz_graphics::DefaultFilterQuality)
        });
        let (worst, at) = worst_against(
            &raw,
            |x, y| {
                // Black, as opaque as it is, over white.
                let v = ((1.0 - straight(x, y)[3] as f32 / 255.0) * 255.0).round() as u8;
                [v, v, v, 255]
            },
            none,
        );
        check!(worst <= 2, "  an Alpha8 bitmap's texture: off by {worst} at {at:?}");
    }
    let masked = bitmaps.render(n, n, ClearColor::WHITE, |ctx| {
        Canvas::new(ctx, Offset::Zero).draw_image(
            &mask,
            Offset::Zero,
            &Paint { color: Color::new(0.1, 0.5, 0.2, 1.0), ..Paint::default() },
        )
    });
    let (worst, at) = worst_against(
        &masked,
        |x, y| {
            // Green, as opaque as the mask, over white.
            let a = straight(x, y)[3] as f32 / 255.0;
            let over = |c: f32| ((c * a + 1.0 - a) * 255.0).round() as u8;
            [over(0.1), over(0.5), over(0.2), 255]
        },
        none,
    );
    check!(worst <= 2, "  an Alpha8 bitmap in a paint's color: off by {worst} at {at:?}");
    // Rgb565 and Argb8888 without alpha: over black, opaque; Rgb565 in 5, 6 and 5 bits.
    let over_black = |x: i32, y: i32, bits: bool| {
        let p = direct.at(x, y);
        let q = |v: u8, levels: f64| {
            if bits { ((v as f64 / 255.0 * levels).round() * 255.0 / levels).round() as i32 } else { v as i32 }
        };
        [q(p[0], 31.0), q(p[1], 63.0), q(p[2], 31.0), 255]
    };
    let rgb = ImageBitmap::new(n, n, ImageBitmapConfig::Rgb565, true, &ColorSpaces::Srgb);
    scene(&mut Canvas::from_image(&rgb));
    check!(!rgb.has_alpha());
    // (A 5-bit step: 8; the edges rounded either way.)
    compare("drawn into an Rgb565 bitmap", &rgb, |x, y| over_black(x, y, true), 9);
    // On the screen, as it holds them now (its texture made again from them).
    let rgb_shown = bitmaps.render(n, n, ClearColor::WHITE, |ctx| {
        ctx.draw_image(&rgb, all, all, 1.0, kompoz_graphics::DefaultFilterQuality)
    });
    {
        let mut held = vec![0u32; (n * n) as usize];
        rgb.read_pixels(&mut held);
        let (worst, at) = worst_against(
            &rgb_shown,
            |x, y| {
                let c = held[(y * n + x) as usize];
                [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]
            },
            none,
        );
        check!(worst <= 1, "  an Rgb565 bitmap on the screen: off by {worst} at {at:?}");
    }
    let opaque = ImageBitmap::new(n, n, ImageBitmapConfig::Argb8888, false, &ColorSpaces::Srgb);
    scene(&mut Canvas::from_image(&opaque));
    compare("drawn into an opaque Argb8888 bitmap", &opaque, |x, y| over_black(x, y, false), 1);
    // Kept ones still about read back as it goes (released).
    let last = ImageBitmap::new(n, n, ImageBitmapConfig::Argb8888, true, &ColorSpaces::Srgb);
    scene(&mut Canvas::from_image(&last));
    check!(!last.pixels_current());
    bitmaps.done();
    check!(last.pixels_current(), "  a bitmap kept by the renderer wasn't read back as it went");
    compare("a kept bitmap released", &last, straight, 1);
}

/// What drawing `vector` into a 24 dp square at 2 px a dp draws (as Compose's VectorPainter draws one).
fn draw_vector_at(ctx: &mut Renderer, vector: &ImageVector, filter: Option<ColorFilter>, rtl: bool) {
    let direction = if rtl { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
    let mut scope = DrawScope::new(ctx, Size::new(48.0, 48.0), direction, Density::new(2.0, 1.0));
    draw_vector(&mut scope, vector, 1.0, filter.as_ref());
}

/// Vectors as Compose's VectorPainter draws them: into a bitmap their size in pixels (kept), drawn 1:1. One in one
/// color as an Alpha8 mask, in a translucent tint: each pixel as the paths drawn straight in the tint make it (as
/// opaque as the tint and the coverage, not the tint twice); one in its own colors at 2 px a dp, its gradient, its
/// turned group and its clip where they go; mirrored right to left. A mask drawn sampled as an image is (nearest,
/// bilinearly, a cubic).
pub fn vectors_draw_as_compose(g: &mut Gpu) {
    let bitmaps = Bitmaps::new(g);
    let n = 48; // 24 dp
    // Rendered with what is drawn into bitmaps meanwhile in its frame (as the app's frame draws a vector).
    let render = |draw: &dyn Fn(&mut Renderer)| bitmaps.render(n, n, ClearColor::WHITE, draw);
    let builder = |blend: BlendMode, mirror: bool| {
        ImageVector::Builder("", 24.dp(), 24.dp(), 24.0, 24.0, Color::Unspecified, blend, mirror)
    };
    // One color (an even-odd ring, a square at half its alpha), tinted.
    let (ring_data, dot_data) =
        (add_path_nodes("M12,2a10,10 0 1,0 0.001,0zM12,6a6,6 0 1,0 0.001,0z"), add_path_nodes("M10,10h4v4h-4z"));
    let ring = builder(BlendMode::SrcIn, false)
        .add_path_of(VectorPath {
            path_data: ring_data.clone(),
            path_fill_type: PathFillType::EvenOdd,
            fill: Some(SolidColor(Color::Black)),
            ..VectorPath::default()
        })
        .add_path_of(VectorPath {
            path_data: dot_data.clone(),
            fill: Some(SolidColor(Color::Black)),
            fill_alpha: 0.5,
            ..VectorPath::default()
        })
        .build();
    let tint = Color::new(0.1, 0.4, 0.9, 0.5);
    let via_vector = render(&|c| draw_vector_at(c, &ring, Some(ColorFilter::tint(tint, BlendMode::SrcIn)), false));
    let direct = render(&|c| {
        let mut s = DrawScope::new(c, Size::new(48.0, 48.0), LayoutDirection::Ltr, Density::new(2.0, 1.0));
        let mut ring_path = to_path(&ring_data);
        ring_path.fill_type = PathFillType::EvenOdd;
        let scaled = |p: &mut Path| p.transform(&steps(|m| m.scale(2.0, 2.0, 1.0)));
        scaled(&mut ring_path);
        let mut dot = to_path(&dot_data);
        scaled(&mut dot);
        s.draw_path_with(&ring_path, tint, 1.0, kompoz_ui::draw::Fill, None, BlendMode::SrcOver);
        s.draw_path_with(&dot, tint, 0.5, kompoz_ui::draw::Fill, None, BlendMode::SrcOver);
    });
    let (worst, at) = worst_against(&via_vector, |x, y| direct.at(x, y), none);
    let over = |v: f32| ((v * 0.25 + 0.75) * 255.0).round() as u8;
    check!(
        worst <= 2 && near(via_vector.at(24, 24), over(0.1), over(0.4), over(0.9), 2),
        "  a one-color vector tinted: off by {worst} at {at:?} ({:?} in the middle)",
        via_vector.at(24, 24)
    );
    // Its own colors at 2 px a dp: a gradient across its top half; a bar turned a quarter about the middle (from
    // across its lower half to down its left), clipped to its lower half.
    let (red, blue, green) =
        (Color::new(0.9, 0.1, 0.1, 1.0), Color::new(0.1, 0.2, 0.9, 1.0), Color::new(0.1, 0.7, 0.2, 1.0));
    let scene = builder(BlendMode::SrcIn, false)
        .add_path_of(VectorPath {
            path_data: add_path_nodes("M0,0h24v12h-24z"),
            fill: Some(Brush::horizontal_gradient(&[red, blue], 0.0, 24.0, TileMode::Clamp)),
            ..VectorPath::default()
        })
        .group_of(
            VectorGroup {
                rotation: 90.0,
                pivot_x: 12.0,
                pivot_y: 12.0,
                clip_path_data: add_path_nodes("M12,0h12v24h-12z"),
                ..VectorGroup::default()
            },
            |b| {
                b.add_path_of(VectorPath {
                    path_data: add_path_nodes("M2,16h20v4h-20z"),
                    fill: Some(SolidColor(green)),
                    ..VectorPath::default()
                });
            },
        )
        .build();
    let drawn = render(&|c| draw_vector_at(c, &scene, None, false));
    let byte = |v: f32| (v * 255.0).round() as u8;
    let u = 6.25 / 24.0; // (at the pixel's middle, 6.25 dp in; each component along, as Skia's gradients)
    let lerp = |a: f32, b: f32| a + (b - a) * u;
    let across = [lerp(red.red(), blue.red()), lerp(red.green(), blue.green()), lerp(red.blue(), blue.blue())];
    // The gradient (the bar clipped away above the middle); the bar, down the left of the lower half; it ends 2 dp
    // up, nothing right of it.
    check!(near(drawn.at(12, 12), byte(across[0]), byte(across[1]), byte(across[2]), 2), "  {:?}", drawn.at(12, 12));
    check!(near(drawn.at(12, 36), byte(green.red()), byte(green.green()), byte(green.blue()), 2));
    check!(near(drawn.at(12, 46), 255, 255, 255, 2) && near(drawn.at(32, 36), 255, 255, 255, 2));
    // Mirrored right to left: a band at its left drawn at the right.
    let band = builder(BlendMode::SrcIn, true)
        .add_path_of(VectorPath {
            path_data: add_path_nodes("M0,0h8v24h-8z"),
            fill: Some(SolidColor(Color::Black)),
            ..VectorPath::default()
        })
        .build();
    let ltr = render(&|c| draw_vector_at(c, &band, None, false));
    let rtl = render(&|c| draw_vector_at(c, &band, None, true));
    check!(
        near(ltr.at(4, 24), 0, 0, 0, 2)
            && near(ltr.at(44, 24), 255, 255, 255, 2)
            && near(rtl.at(4, 24), 255, 255, 255, 2)
            && near(rtl.at(44, 24), 0, 0, 0, 2)
    );
    // A mask sampled as an image of its alpha is, each way.
    let m = 4;
    let alpha8 = ImageBitmap::new(m, m, ImageBitmapConfig::Alpha8, true, &ColorSpaces::Srgb);
    let argb = ImageBitmap::new(m, m, ImageBitmapConfig::Argb8888, true, &ColorSpaces::Srgb);
    for b in [&alpha8, &argb] {
        let mut c = Canvas::from_image(b);
        for y in 0..m {
            for x in 0..m {
                let a = ((x * 5 + y * 3) % 8) as f32 / 7.0;
                let paint = Paint { color: Color::new(0.0, 0.0, 0.0, a), ..Paint::default() };
                c.draw_rect(xywh(x as f32, y as f32, 1.0, 1.0), &paint);
            }
        }
    }
    for q in [FilterQuality::None, FilterQuality::Low, FilterQuality::High] {
        let (src, dst) = (xywh(0.0, 0.0, m as f32, m as f32), xywh(0.0, 0.0, 48.0, 48.0));
        let masked = render(&|c| c.draw_image_mask(&alpha8, src, dst, &SolidColor(Color::Black), dst, 0.6, q));
        let image = render(&|c| c.draw_image(&argb, src, dst, 0.6, q)); // (faded alike)
        let (worst, at) = worst_against(&masked, |x, y| image.at(x, y), none);
        check!(worst <= 1, "  a mask sampled with quality {q:?}: off by {worst} at {at:?}");
    }
    bitmaps.done();
}

/// What a picture drawn into a bitmap holds is drawn into it once, in order, where the renderer was busy: two
/// pictures waiting for one bitmap, the second over the first, then the bitmap drawn.
pub fn bitmaps_drawn_into_in_a_frame_wait_their_turn(g: &mut Gpu) {
    let bitmaps = Bitmaps::new(g);
    let n = 16;
    let all = xywh(0.0, 0.0, n as f32, n as f32);
    let b = ImageBitmap::new(n, n, ImageBitmapConfig::Argb8888, true, &ColorSpaces::Srgb);
    let p = bitmaps.render(n, n, ClearColor::WHITE, |ctx| {
        let mut first = Picture::new();
        first.fill_round_rect(all, CornerRadii::Zero, Color::new(0.0, 0.0, 1.0, 1.0));
        let mut second = Picture::new();
        second.fill_round_rect(xywh(0.0, 0.0, 8.0, 16.0), CornerRadii::Zero, Color::new(1.0, 0.0, 0.0, 1.0));
        bitmaps.hook.draw(&b, &first);
        bitmaps.hook.draw(&b, &second);
        ctx.draw_image(&b, all, all, 1.0, FilterQuality::None);
    });
    check!(near(p.at(4, 8), 255, 0, 0, 1) && near(p.at(12, 8), 0, 0, 255, 1), "  {:?} {:?}", p.at(4, 8), p.at(12, 8));
    check!(b.version() == 3 && !b.pixels_current());
    bitmaps.done();
    check!(b.pixels_current());
}
