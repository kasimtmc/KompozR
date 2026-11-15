//! Layers, effects and shadows (the C++ Kompoz's checks of them): a layer's alpha is exact; graphics layers composite
//! as Compose's; effect layers blur as Skia does (against a Gaussian worked out here), progressively as Compose's,
//! backdrops too; paths' shadows are Compose's, and kept; elevation shadows are Android's; a layer is kept from its
//! first frame, and kept layers draw as drawn afresh; layer textures come in few sizes. (The effects and
//! graphics-layers scenes are compared with their pictures in scenes.) Tests at another px per dp than 1 give C++'s
//! dp coordinates through `k`.

use crate::exact::{none, worst_against};
use crate::fonts::{Font, text_at};
use crate::harness::{Gpu, Pixels, WatchedDevice, max_difference, near};
use crate::scenes::{about, fill, linear, mixed, oval, rgba, rounded, shadow, star, through_layer, xywh};
use kompoz_gpu::Renderer;
use kompoz_gpu::device::{ClearColor, Device, Format, IRect, TextureUsage};
use kompoz_graphics::{
    BlendMode, BlurEffect, BlurRadiusSpec, BlurStop, Brush, Color, ColorFilter, ColorMatrix, CompositeShader, Density,
    Dp, LayoutDirection, LinearGradientShader, Matrix, Offset, OffsetEffect, Outline, Path, PathStroke, Rect,
    RenderEffect, RoundedCornerShape, Size, TileMode, blend,
};
use kompoz_ui::DisplayList;
use kompoz_ui::graphics_context::{
    ClipOp, CornerRadii, ElevationShadow, GraphicsContext, LayerTransform, ShadowLight, default_draw_shadow_path,
};
use kompoz_ui::layout::CompositingStrategy;

const TARGET: TextureUsage = TextureUsage::RENDER_TARGET.union(TextureUsage::SAMPLED);

// ---- Exact checks -------------------------------------------------------------------------------------------------

/// A layer's alpha fades what is in it as one: what overlaps inside it doesn't show through.
pub fn layer_alpha_is_exact(g: &mut Gpu) {
    let p = g.render(32, 32, |c| {
        c.push_layer(xywh(0.0, 0.0, 32.0, 32.0), CornerRadii::Zero, 0.5, LayerTransform::Identity);
        c.fill_round_rect(xywh(4.0, 4.0, 24.0, 24.0), CornerRadii::Zero, rgba(1.0, 0.0, 0.0, 1.0));
        c.fill_round_rect(xywh(8.0, 8.0, 16.0, 16.0), CornerRadii::Zero, rgba(0.0, 0.0, 1.0, 1.0)); // over the red
        c.pop_layer();
    });
    check!(near(p.at(6, 6), 255, 128, 128, 2)); // half red over white
    check!(near(p.at(16, 16), 128, 128, 255, 2)); // the blue alone, at half: the red under it doesn't show
    check!(near(p.at(1, 1), 255, 255, 255, 2));
}

/// Graphics layers composite as Compose's: turned half way about y, mirrored; faded once (Auto) or each draw
/// (ModulateAlpha); blended by Multiply; filtered.
pub fn graphics_layers_composite_as_compose(g: &mut Gpu) {
    let n = 96;
    let plain = g.render(n, n, |c| mixed(c, 1.0, true));
    let flipped = g.render(n, n, |c| {
        through_layer(c, xywh(0.0, 0.0, 96.0, 96.0), |l| l.geometry.rotation_y = 180.0, |c| mixed(c, 1.0, true));
    });
    let (worst, at) = worst_against(&plain, |x, y| flipped.at(n - 1 - x, y), none);
    check!(worst <= 3, "  turned half way about y: off by {worst} at {at:?}");
    let mut overlapping = |strategy: CompositingStrategy| {
        g.render(32, 32, |c| {
            through_layer(
                c,
                xywh(0.0, 0.0, 32.0, 32.0),
                |l| {
                    l.alpha = 0.5;
                    l.strategy = strategy;
                },
                |c| {
                    c.fill_round_rect(xywh(0.0, 0.0, 20.0, 32.0), CornerRadii::Zero, rgba(1.0, 0.0, 0.0, 1.0));
                    c.fill_round_rect(xywh(12.0, 0.0, 20.0, 32.0), CornerRadii::Zero, rgba(0.0, 0.0, 1.0, 1.0));
                },
            )
        })
    };
    let once = overlapping(CompositingStrategy::Auto);
    let twice = overlapping(CompositingStrategy::ModulateAlpha);
    // The blue alone over the overlap, at half; or the red at half, the blue at half over it.
    check!(near(once.at(16, 16), 128, 128, 255, 2) && near(once.at(4, 16), 255, 128, 128, 2));
    check!(near(twice.at(16, 16), 128, 64, 191, 2) && near(twice.at(4, 16), 255, 128, 128, 2));
    let (under, over) = (rgba(0.2, 0.6, 0.9, 1.0), rgba(0.9, 0.5, 0.2, 1.0));
    let multiplied = g.render(16, 16, |c| {
        c.fill_round_rect(xywh(0.0, 0.0, 16.0, 16.0), CornerRadii::Zero, under);
        through_layer(
            c,
            xywh(0.0, 0.0, 16.0, 16.0),
            |l| l.blend_mode = BlendMode::Multiply,
            |c| c.fill_round_rect(xywh(0.0, 0.0, 16.0, 16.0), CornerRadii::Zero, over),
        );
    });
    let byte = |v: f32| (v * 255.0).round() as u8;
    let want = blend(over, under, BlendMode::Multiply);
    check!(near(multiplied.at(8, 8), byte(want.red()), byte(want.green()), byte(want.blue()), 2));
    let mut gray = ColorMatrix::new();
    gray.set_to_saturation(0.0);
    let grayed = g.render(16, 16, |c| {
        through_layer(
            c,
            xywh(0.0, 0.0, 16.0, 16.0),
            |l| l.color_filter = Some(ColorFilter::color_matrix(gray)),
            |c| c.fill_round_rect(xywh(0.0, 0.0, 16.0, 16.0), CornerRadii::Zero, over),
        );
    });
    let g2 = ColorFilter::color_matrix(gray).apply(over);
    check!(near(grayed.at(8, 8), byte(g2.red()), byte(g2.green()), byte(g2.blue()), 2));
}

// ---- Models of blurs ----------------------------------------------------------------------------------------------

/// A picture as floats (premultiplied RGBA), w x h.
#[derive(Clone)]
struct Image {
    w: i32,
    h: i32,
    rgba: Vec<f32>,
}

impl Image {
    fn new(w: i32, h: i32) -> Image {
        Image { w, h, rgba: vec![0.0; (w * h * 4) as usize] }
    }

    fn at(&self, x: i32, y: i32) -> &[f32] {
        let i = ((y * self.w + x) * 4) as usize;
        &self.rgba[i..i + 4]
    }

    fn at_mut(&mut self, x: i32, y: i32) -> &mut [f32] {
        let i = ((y * self.w + x) * 4) as usize;
        &mut self.rgba[i..i + 4]
    }

    /// Its alpha, pixel by pixel.
    fn alpha(&self) -> Vec<f32> {
        self.rgba.chunks(4).map(|p| p[3]).collect()
    }
}

/// Opaque fills over nothing, whole pixels.
fn painted(w: i32, h: i32, fills: &[(IRect, Color)]) -> Image {
    let mut out = Image::new(w, h);
    for (r, c) in fills {
        for y in r.y.max(0)..h.min(r.y + r.height) {
            for x in r.x.max(0)..w.min(r.x + r.width) {
                out.at_mut(x, y).copy_from_slice(&[c.red(), c.green(), c.blue(), 1.0]);
            }
        }
    }
    out
}

/// What Skia makes of a layer's pixels blurred: the pixels past `valid` as `tile` says, a discrete Gaussian across
/// (weights exp(-i^2 / 2 sigma^2) out to ceil(3 sigma), adding up to 1), then one down; nothing past three sigmas
/// beyond `valid`.
fn gaussian(input: &Image, valid: IRect, tile: TileMode, sigma_x: f32, sigma_y: f32) -> Image {
    let tiled = |t: i32, low: i32, n: i32| -> i32 {
        if t >= low && t < low + n {
            return t;
        }
        let m = t - low;
        match tile {
            TileMode::Clamp => t.clamp(low, low + n - 1),
            TileMode::Repeated => low + m.rem_euclid(n),
            TileMode::Mirror => {
                let k = m.rem_euclid(2 * n);
                low + if k < n { k } else { 2 * n - 1 - k }
            }
            TileMode::Decal => -1,
        }
    };
    let kernel = |sigma: f32| -> Vec<f64> {
        let reach = if sigma > 0.0 { (3.0 * sigma).ceil() as i32 } else { 0 };
        let mut k: Vec<f64> = (-reach..=reach)
            .map(|i| if sigma > 0.0 { (-(i * i) as f64 / (2.0 * sigma as f64 * sigma as f64)).exp() } else { 1.0 })
            .collect();
        let sum: f64 = k.iter().sum();
        k.iter_mut().for_each(|v| *v /= sum);
        k
    };
    let (kx, ky) = (kernel(sigma_x), kernel(sigma_y));
    let (rx, ry) = ((kx.len() / 2) as i32, (ky.len() / 2) as i32);
    let source = |x: i32, y: i32, c: usize| -> f64 {
        let (sx, sy) = (tiled(x, valid.x, valid.width), tiled(y, valid.y, valid.height));
        if sx < 0 || sy < 0 || sx >= input.w || sy >= input.h { 0.0 } else { input.at(sx, sy)[c] as f64 }
    };
    let mut out = Image::new(input.w, input.h);
    for y in (valid.y - ry).max(0)..input.h.min(valid.y + valid.height + ry) {
        for x in (valid.x - rx).max(0)..input.w.min(valid.x + valid.width + rx) {
            for c in 0..4 {
                let mut v = 0.0;
                for j in -ry..=ry {
                    let mut row = 0.0;
                    for i in -rx..=rx {
                        row += kx[(i + rx) as usize] * source(x + i, y + j, c);
                    }
                    v += ky[(j + ry) as usize] * row;
                }
                out.at_mut(x, y)[c] = v as f32;
            }
        }
    }
    out
}

/// As drawn over white, 8 bits a channel.
fn over_white(image: &Image) -> Pixels {
    let mut rgba = Vec::with_capacity((image.w * image.h * 4) as usize);
    for p in image.rgba.chunks(4) {
        for c in 0..3 {
            rgba.push(((p[c] + (1.0 - p[3])).clamp(0.0, 1.0) * 255.0).round() as u8);
        }
        rgba.push(255);
    }
    Pixels { width: image.w, height: image.h, rgba }
}

/// Android's sigma for a radius in px.
fn sigma_of(radius_px: f32) -> f32 {
    0.57735 * radius_px + 0.5
}

/// What Compose's progressive blur makes of a layer's pixels (its BlurShaders, done here in double): across, then
/// down, each pixel by its radius (`widest` px times where it lies between the stops, from `fraction`), the pixels
/// sampled linearly (nothing past the layer's, `bounds`), the taps past the bounds on the axis left out and counted
/// when `decal`. `unit`: pixels a px.
fn progressive_of(
    input: &Image,
    bounds: IRect,
    widest: f32,
    stops: &[(f32, f32)],
    fraction: &dyn Fn(f64, f64) -> f64,
    decal: bool,
    unit: f64,
) -> Image {
    let intensity = |t: f64| -> f64 {
        if t <= stops[0].0 as f64 {
            return stops[0].1 as f64;
        }
        for w in stops.windows(2) {
            let (a, b) = (w[0].0 as f64, w[1].0 as f64);
            if t >= a && t <= b {
                return w[0].1 as f64 + (w[1].1 - w[0].1) as f64 * ((t - a) / (b - a).max(0.0001)).clamp(0.0, 1.0);
            }
        }
        stops[stops.len() - 1].1 as f64
    };
    let pass = |from: &Image, down: bool| -> Image {
        let sample = |x: f64, y: f64, c: usize| -> f64 {
            let (fx, fy) = (x - 0.5, y - 0.5);
            let (x0, y0) = (fx.floor() as i32, fy.floor() as i32);
            let (ax, ay) = (fx - x0 as f64, fy - y0 as f64);
            let mut v = 0.0;
            for j in 0..2 {
                for i in 0..2 {
                    let (px, py) = (x0 + i, y0 + j);
                    let w = (if i == 1 { ax } else { 1.0 - ax }) * (if j == 1 { ay } else { 1.0 - ay });
                    if px >= 0 && py >= 0 && px < from.w && py < from.h {
                        v += w * from.at(px, py)[c] as f64;
                    }
                }
            }
            v
        };
        let (low, high) = if down {
            (bounds.y as f64, (bounds.y + bounds.height) as f64)
        } else {
            (bounds.x as f64, (bounds.x + bounds.width) as f64)
        };
        let mut out = Image::new(from.w, from.h);
        for y in 0..from.h {
            for x in 0..from.w {
                let (cx, cy) = (x as f64 + 0.5, y as f64 + 0.5);
                let at = fraction((cx - bounds.x as f64).max(0.0), (cy - bounds.y as f64).max(0.0));
                let radius = widest as f64 * intensity(at).clamp(0.0, 1.0);
                let r = radius.floor();
                let mut sum = [0.0f64; 4];
                for (c, s) in sum.iter_mut().enumerate() {
                    *s = sample(cx, cy, c);
                }
                let mut weights = 1.0;
                if r >= 1.0 {
                    let sigma = (radius / 2.0).max(1.0);
                    let g = |d: f64| (-d * d / (2.0 * sigma * sigma)).exp();
                    let mut tap = |off: f64, w: f64| {
                        for side in [-1.0, 1.0] {
                            let px = cx + if down { 0.0 } else { side * off * unit };
                            let py = cy + if down { side * off * unit } else { 0.0 };
                            let along = if down { py } else { px };
                            let inside = along >= low && along < high;
                            weights += w * if inside || decal { 1.0 } else { 0.0 };
                            if inside {
                                for (c, s) in sum.iter_mut().enumerate() {
                                    *s += w * sample(px, py, c);
                                }
                            }
                        }
                    };
                    let mut i = 1.0;
                    while i < 150.0 && i < r {
                        tap(i + g(i + 1.0) / (g(i) + g(i + 1.0)), g(i) + g(i + 1.0));
                        i += 2.0;
                    }
                    if r as i32 % 2 == 1 && r < 150.0 {
                        tap(r, g(r));
                    }
                }
                for (o, s) in out.at_mut(x, y).iter_mut().zip(sum) {
                    *o = (s / weights) as f32;
                }
            }
        }
        out
    };
    let mut cut = Image::new(input.w, input.h);
    for y in bounds.y..bounds.y + bounds.height {
        for x in bounds.x..bounds.x + bounds.width {
            cut.at_mut(x, y).copy_from_slice(input.at(x, y));
        }
    }
    pass(&pass(&cut, false), true)
}

/// `image` moved by (dx, dy) whole pixels (what leaves it gone).
fn moved(image: &Image, dx: i32, dy: i32) -> Image {
    let mut out = Image::new(image.w, image.h);
    for y in 0..image.h {
        for x in 0..image.w {
            if x - dx >= 0 && x - dx < image.w && y - dy >= 0 && y - dy < image.h {
                out.at_mut(x, y).copy_from_slice(image.at(x - dx, y - dy));
            }
        }
    }
    out
}

/// What is drawn against what is wanted, within `tolerance` but where `skip` says.
fn report(what: &str, got: &Pixels, want: &Pixels, tolerance: i32, skip: impl Fn(i32, i32) -> bool) {
    let (worst, at) = worst_against(got, |x, y| want.at(x, y), skip);
    if std::env::var_os("KOMPOZ_BLUR_WORST").is_some() {
        println!("  {what}: {worst}");
    }
    check!(worst <= tolerance, "  {what}: off by {worst} at {at:?}");
}

fn scaled(s: f32) -> Matrix {
    let mut m = Matrix::new();
    m.scale(s, s, 1.0);
    m
}

/// Effect layers blur as Skia does: the edges as the tile mode says, a Gaussian of Android's sigma, spreading three
/// sigmas; wide ones (scaled down) close to it; offsets, chains, transforms, a clip around them and layers too big
/// for a texture; progressive blurs as Compose's; backdrops.
pub fn blurs_are_gaussian(g: &mut Gpu) {
    let n = 96;
    let bounds = IRect::new(24, 24, 48, 48);
    let fills = [
        (IRect::new(24, 24, 20, 48), rgba(0.9, 0.1, 0.1, 1.0)),
        (IRect::new(44, 30, 28, 14), rgba(0.1, 0.2, 0.9, 1.0)),
        (IRect::new(50, 56, 10, 10), rgba(0.1, 0.7, 0.2, 1.0)),
        (IRect::new(60, 66, 12, 6), rgba(0.95, 0.8, 0.1, 1.0)), // (the bottom unlike the top, the right unlike the left)
    ];
    let paint_fills = |c: &mut Renderer| {
        for (r, col) in &fills {
            c.fill_round_rect(xywh(r.x as f32, r.y as f32, r.width as f32, r.height as f32), CornerRadii::Zero, *col);
        }
    };
    let content = |c: &mut Renderer| {
        paint_fills(c);
        let black = rgba(0.0, 0.0, 0.0, 1.0);
        c.fill_round_rect(xywh(0.0, 0.0, 96.0, 20.0), CornerRadii::Zero, black); // outside the bounds: cut away
        c.fill_round_rect(xywh(74.0, 30.0, 10.0, 30.0), CornerRadii::Zero, black); // (and in the layer's spare pixels)
    };
    let base = painted(n, n, &fills);
    // (At k px a dp, C++'s dp coordinates through k: the layer over the same px, the radii k times.)
    let mut blurred_by = |effect: &RenderEffect| {
        g.render(n, n, |c| {
            c.push_effect_layer(xywh(24.0, 24.0, 48.0, 48.0), effect, 1.0);
            content(c);
            c.pop_layer();
        })
    };
    let all = |_: i32, _: i32| false;
    // Each tile mode, narrow (no scaling down) and each way its own radius; in bounds of another size too, whose
    // periods a GPU dividing by a reciprocal can come out just under.
    for tile in [TileMode::Clamp, TileMode::Decal, TileMode::Repeated, TileMode::Mirror] {
        let p = blurred_by(&BlurEffect(4.0, 2.5, tile));
        report("narrow", &p, &over_white(&gaussian(&base, bounds, tile, sigma_of(4.0), sigma_of(2.5))), 2, all);
    }
    for tile in [TileMode::Clamp, TileMode::Decal, TileMode::Repeated, TileMode::Mirror] {
        let small = IRect::new(30, 30, 36, 30);
        let q = g.render(n, n, |c| {
            c.push_effect_layer(xywh(30.0, 30.0, 36.0, 30.0), &BlurEffect(3.0, 3.0, tile), 1.0);
            content(c);
            c.pop_layer();
        });
        let mut within = base.clone(); // (what is drawn within the bounds)
        for y in 0..n {
            for x in 0..n {
                if x < small.x || x >= small.x + small.width || y < small.y || y >= small.y + small.height {
                    within.at_mut(x, y).fill(0.0);
                }
            }
        }
        report(
            "narrow, 36 x 30",
            &q,
            &over_white(&gaussian(&within, small, tile, sigma_of(3.0), sigma_of(3.0))),
            2,
            all,
        );
    }
    let mut blurred_by = |effect: &RenderEffect| {
        g.render(n, n, |c| {
            c.push_effect_layer(xywh(24.0, 24.0, 48.0, 48.0), effect, 1.0);
            content(c);
            c.pop_layer();
        })
    };
    // One way only; at 2 px per dp (the sigma in px).
    report(
        "across only",
        &blurred_by(&BlurEffect(5.0, 0.0, TileMode::Clamp)),
        &over_white(&gaussian(&base, bounds, TileMode::Clamp, sigma_of(5.0), 0.0)),
        2,
        all,
    );
    report(
        "at 2 px a dp",
        &blurred_by(&BlurEffect(3.0 * 2.0, 3.0 * 2.0, TileMode::Decal)),
        &over_white(&gaussian(&base, bounds, TileMode::Decal, sigma_of(6.0), sigma_of(6.0))),
        2,
        all,
    );
    // Wide: scaled down 2, 4, 8, 16 times (two passes), close to the Gaussian it stands for (the means of blocks and
    // the bilinear filtering back up aren't quite Gaussians: within 3).
    for r in [9.0, 20.0, 40.0, 80.0] {
        let p = blurred_by(&BlurEffect(r, r, TileMode::Decal));
        report("wide", &p, &over_white(&gaussian(&base, bounds, TileMode::Decal, sigma_of(r), sigma_of(r))), 3, all);
        let clamped = blurred_by(&BlurEffect(r, r, TileMode::Clamp));
        report(
            "wide, clamped",
            &clamped,
            &over_white(&gaussian(&base, bounds, TileMode::Clamp, sigma_of(r), sigma_of(r))),
            3,
            all,
        );
    }
    // Wide ones have the variance of their Gaussian. Measured across on the edges of half planes blurred across,
    // each row's a pixel further on (scaling down and back up isn't the same at every phase): the Gaussian at the
    // level, as its kernel cuts it, and the variances of the blocks' means and of the bilinear filtering back up add
    // up to what they stand for (which they do before the cut).
    let variance = |p: &Pixels| {
        let mut total = 0.0;
        for y in 0..p.height {
            let (mut sum, mut mean, mut second) = (0.0f64, 0.0f64, 0.0f64);
            for x in 0..p.width - 1 {
                let d = (p.at(x + 1, y)[0] as f64 - p.at(x, y)[0] as f64) / 255.0;
                let at = (x + 1) as f64;
                sum += d;
                mean += d * at;
                second += d * at * at;
            }
            mean /= sum;
            total += second / sum - mean * mean;
        }
        total / p.height as f64
    };
    let truncated = |g: f64| {
        // A discrete Gaussian's variance out to ceil(3 g).
        let reach = (3.0 * g).ceil() as i32;
        let (mut w, mut m) = (0.0, 0.0);
        for i in -reach..=reach {
            let e = (-(i * i) as f64 / (2.0 * g * g)).exp();
            w += e;
            m += (i * i) as f64 * e;
        }
        m / w
    };
    let s = |r: f32| sigma_of(r) as f64;
    let wides = [
        (BlurEffect(40.0, 0.0, TileMode::Clamp), 8, (s(40.0) * s(40.0) - 63.0 / 12.0 - 63.0 / 6.0).sqrt() / 8.0),
        (BlurEffect(80.0, 0.0, TileMode::Clamp), 16, (s(80.0) * s(80.0) - 255.0 / 12.0 - 255.0 / 6.0).sqrt() / 16.0),
        // (The second at the first's level, 8: all of its variance gone into scaling back up, the rest from the
        // first.)
        (
            RenderEffect::blur_effect(Some(&BlurEffect(30.0, 0.0, TileMode::Clamp)), 3.0, 0.0, TileMode::Clamp),
            8,
            (s(30.0) * s(30.0) - 63.0 / 12.0 - (63.0 / 6.0 - s(3.0) * s(3.0))).sqrt() / 8.0,
        ),
    ];
    for (effect, step, level) in wides {
        let edges = g.render(400, 16, |c| {
            c.push_effect_layer(xywh(0.0, 0.0, 400.0, 16.0), &effect, 1.0);
            for y in 0..16 {
                c.fill_round_rect(
                    xywh(0.0, y as f32, (200 + y) as f32, 1.0),
                    CornerRadii::Zero,
                    rgba(0.0, 0.0, 0.0, 1.0),
                );
            }
            c.pop_layer();
        });
        // (Clamp: the rows are as they are above and below, the half plane goes on to the left.)
        let step = step as f64;
        let (got, want) = (variance(&edges), truncated(level) * step * step + (step * step - 1.0) / 4.0);
        if std::env::var_os("KOMPOZ_BLUR_WORST").is_some() {
            println!("  variance: {got}, {want}");
        }
        // (8 bits a channel: the far tails' least values gone.)
        check!((got - want).abs() < 0.004 * want + 1.0, "  variance {got}, wanted {want}");
    }
    let mut blurred_by = |effect: &RenderEffect| {
        g.render(n, n, |c| {
            c.push_effect_layer(xywh(24.0, 24.0, 48.0, 48.0), effect, 1.0);
            content(c);
            c.pop_layer();
        })
    };
    // Moved (before or after the blur: the same), whole pixels and a half.
    let shifted = moved(&gaussian(&base, bounds, TileMode::Clamp, sigma_of(3.0), sigma_of(3.0)), 7, -4);
    let offset = OffsetEffect(7.0, -4.0);
    report(
        "moved, then blurred",
        &blurred_by(&RenderEffect::blur_effect(Some(&offset), 3.0, 3.0, TileMode::Clamp)),
        &over_white(&shifted),
        2,
        all,
    );
    let blur3 = BlurEffect(3.0, 3.0, TileMode::Clamp);
    report(
        "blurred, then moved",
        &blurred_by(&RenderEffect::offset_effect(Some(&blur3), Offset::new(7.0, -4.0))),
        &over_white(&shifted),
        2,
        all,
    );
    let blur_half = BlurEffect(1.5 * 2.0, 1.5 * 2.0, TileMode::Clamp);
    report(
        "moved at 2 px a dp",
        &blurred_by(&RenderEffect::offset_effect(Some(&blur_half), Offset::new(3.5 * 2.0, -2.0 * 2.0))),
        &over_white(&shifted),
        2,
        all,
    );
    // Twice: as once by the sum of the variances, the second at what the first spread to.
    let both = sigma_of(3.0).hypot(sigma_of(4.0));
    let first = BlurEffect(3.0, 3.0, TileMode::Decal);
    report(
        "twice",
        &blurred_by(&RenderEffect::blur_effect(Some(&first), 4.0, 4.0, TileMode::Decal)),
        &over_white(&gaussian(&base, bounds, TileMode::Decal, both, both)),
        2,
        all,
    );
    // A narrow one after a wide one: on what that scaled down.
    let wide = sigma_of(30.0).hypot(sigma_of(3.0));
    let first = BlurEffect(30.0, 30.0, TileMode::Decal);
    report(
        "narrow after wide",
        &blurred_by(&RenderEffect::blur_effect(Some(&first), 3.0, 3.0, TileMode::Decal)),
        &over_white(&gaussian(&base, bounds, TileMode::Decal, wide, wide)),
        3,
        all,
    );
    // Enlarged by a transform: blurred and moved in the layer's pixels, then enlarged (the layer drawn at twice them).
    let enlarged = g.render(n, n, |c| {
        c.push_transform(&scaled(2.0));
        let blurred = BlurEffect(2.0, 2.0, TileMode::Decal);
        c.push_effect_layer(
            xywh(12.0, 12.0, 24.0, 24.0),
            &RenderEffect::offset_effect(Some(&blurred), Offset::new(1.5, -1.0)),
            1.0,
        );
        c.push_transform(&scaled(0.5));
        content(c);
        c.pop_transform();
        c.pop_layer();
        c.pop_transform();
    });
    let big = gaussian(&base, bounds, TileMode::Decal, 2.0 * sigma_of(2.0), 2.0 * sigma_of(2.0));
    report("enlarged", &enlarged, &over_white(&moved(&big, 3, -2)), 2, all);
    // Progressive (Compose's): down from sharp to blurred, out from a center through stops, along a line across a
    // corner, each edge treatment; at 2 px a dp; wider than 150 px (the radii scaled down to it); then blurred again
    // as one.
    let mut progressive = |spec: &BlurRadiusSpec,
                           edges: TileMode,
                           k: f32,
                           widest: f32,
                           stops: &[(f32, f32)],
                           fraction: &dyn Fn(f64, f64) -> f64| {
        let effect = spec.create_render_effect(Size::new(48.0, 48.0), edges, &Density::new(k, 1.0));
        let got = g.render(n, n, |c| {
            c.push_effect_layer(xywh(24.0, 24.0, 48.0, 48.0), &effect, 1.0);
            content(c);
            c.pop_layer();
        });
        (got, over_white(&progressive_of(&base, bounds, widest, stops, fraction, edges == TileMode::Decal, 1.0)))
    };
    let down = |_: f64, y: f64| (y / 48.0).clamp(0.0, 1.0);
    for edges in [TileMode::Clamp, TileMode::Decal] {
        let (got, want) = progressive(
            &BlurRadiusSpec::vertical_gradient(Dp(0.0), Dp(12.0)),
            edges,
            1.0,
            12.0,
            &[(0.0, 0.0), (1.0, 1.0)],
            &down,
        );
        report("progressive down", &got, &want, 2, all);
    }
    let radial = BlurRadiusSpec::radial_gradient_stops(
        &[BlurStop::new(0.0, Dp(8.0)), BlurStop::new(0.5, Dp(0.0)), BlurStop::new(1.0, Dp(16.0))],
        Offset::new(20.0, 28.0),
        Dp(24.0),
    );
    let (got, want) =
        progressive(&radial, TileMode::Clamp, 1.0, 16.0, &[(0.0, 0.5), (0.5, 0.0), (1.0, 1.0)], &|x, y| {
            ((x - 20.0).hypot(y - 28.0) / 24.0).clamp(0.0, 1.0)
        });
    report("progressive out from a center", &got, &want, 2, all);
    // (Radii that no pixel's comes out a whole number of: there a float and a double may round a tap's worth apart,
    // Compose's as much as ours.)
    let corner = BlurRadiusSpec::linear_gradient(Offset::Zero, Offset::new(47.0, 37.0), Dp(10.3), Dp(2.1));
    let (got, want) = progressive(&corner, TileMode::Decal, 1.0, 10.3, &[(0.0, 1.0), (1.0, 2.1 / 10.3)], &|x, y| {
        ((x * 47.0 + y * 37.0) / (47.0 * 47.0 + 37.0 * 37.0)).clamp(0.0, 1.0)
    });
    report("progressive across a corner", &got, &want, 2, all);
    let (got, want) = progressive(
        &BlurRadiusSpec::vertical_gradient(Dp(0.0), Dp(4.0)),
        TileMode::Clamp,
        2.0,
        8.0,
        &[(0.0, 0.0), (1.0, 1.0)],
        &down,
    );
    report("progressive at 2 px a dp", &got, &want, 2, all);
    let (got, want) = progressive(
        &BlurRadiusSpec::horizontal_gradient(Dp(200.0), Dp(50.0)),
        TileMode::Decal,
        1.0,
        150.0,
        &[(0.0, 1.0), (1.0, 0.25)],
        &|x, _| (x / 48.0).clamp(0.0, 1.0),
    );
    report("progressive wider than 150 px", &got, &want, 2, all);
    // A shader's alpha the radius over the widest (Compose's BlurRadiusSpec.shader): a gradient down as the gradient
    // spec; two gradients' product (a composite, baked) at 2 px a dp, where 8 bits would put a radius of a whole
    // pixel a pixel further.
    let down_mask = BlurRadiusSpec::shader(Dp(12.0), |_, size| {
        LinearGradientShader(
            Offset::Zero,
            Offset::new(0.0, size.height),
            &[Color::Transparent, Color::Black],
            None,
            TileMode::Clamp,
        )
    });
    for edges in [TileMode::Clamp, TileMode::Decal] {
        let (got, want) = progressive(&down_mask, edges, 1.0, 12.0, &[(0.0, 0.0), (1.0, 1.0)], &down);
        report("progressive by a shader", &got, &want, 2, all);
    }
    let product_mask = BlurRadiusSpec::shader(Dp(4.0), |_, size| {
        CompositeShader(
            LinearGradientShader(
                Offset::Zero,
                Offset::new(0.0, size.height),
                &[Color::Transparent, Color::Black],
                None,
                TileMode::Clamp,
            ),
            LinearGradientShader(
                Offset::Zero,
                Offset::new(size.width, 0.0),
                &[Color::Transparent, Color::Black],
                None,
                TileMode::Clamp,
            ),
            BlendMode::DstIn,
        )
    });
    let (got, want) = progressive(&product_mask, TileMode::Clamp, 2.0, 8.0, &[(0.0, 0.0), (1.0, 1.0)], &|x, y| {
        (x / 48.0).clamp(0.0, 1.0) * (y / 48.0).clamp(0.0, 1.0)
    });
    report("progressive by a composite shader at 2 px a dp", &got, &want, 2, all);
    // In a layer a transform enlarges: in the layer's pixels (each two of the screen's), then enlarged.
    let bigger = g.render(n, n, |c| {
        c.push_transform(&scaled(2.0));
        let effect = BlurRadiusSpec::vertical_gradient(Dp(0.0), Dp(6.0)).create_render_effect(
            Size::new(24.0, 24.0),
            TileMode::Clamp,
            &Density::new(1.0, 1.0),
        );
        c.push_effect_layer(xywh(12.0, 12.0, 24.0, 24.0), &effect, 1.0);
        c.push_transform(&scaled(0.5));
        content(c);
        c.pop_transform();
        c.pop_layer();
        c.pop_transform();
    });
    let want = progressive_of(&base, bounds, 6.0, &[(0.0, 0.0), (1.0, 1.0)], &down, false, 2.0);
    report("progressive, enlarged", &bigger, &over_white(&want), 2, all);
    // The input of others: blurred, then moved.
    let chained = g.render(n, n, |c| {
        let p = BlurRadiusSpec::vertical_gradient(Dp(0.0), Dp(12.0)).create_render_effect(
            Size::new(48.0, 48.0),
            TileMode::Decal,
            &Density::new(1.0, 1.0),
        );
        let blurred = RenderEffect::blur_effect(Some(&p), 2.0, 2.0, TileMode::Decal);
        c.push_effect_layer(
            xywh(24.0, 24.0, 48.0, 48.0),
            &RenderEffect::offset_effect(Some(&blurred), Offset::new(5.0, -3.0)),
            1.0,
        );
        content(c);
        c.pop_layer();
    });
    let then = gaussian(
        &progressive_of(&base, bounds, 12.0, &[(0.0, 0.0), (1.0, 1.0)], &down, true, 1.0),
        IRect::new(0, 0, n, n),
        TileMode::Decal,
        sigma_of(2.0),
        sigma_of(2.0),
    );
    report("progressive, blurred, moved", &chained, &over_white(&moved(&then, 5, -3)), 2, all);
    // Backdrops: what is drawn so far under a rect blurred (from all around it), drawn back in a clip to it, and what
    // is drawn after over it sharp; faded; in a turned clip; in a path's clip (a pass ended for it, the clip's mask
    // made again for the next); in a layer, that layer's.
    let mut backdrop = |alpha: f32, clip: &dyn Fn(&mut Renderer), unclip: &dyn Fn(&mut Renderer)| {
        g.render(n, n, |c| {
            paint_fills(c);
            clip(c);
            c.draw_backdrop(xywh(30.0, 30.0, 40.0, 36.0), &BlurEffect(4.0, 4.0, TileMode::Clamp), alpha);
            c.fill_round_rect(xywh(46.0, 44.0, 8.0, 8.0), CornerRadii::Zero, rgba(1.0, 1.0, 1.0, 1.0)); // over it, sharp
            unclip(c);
        })
    };
    let behind = gaussian(&base, IRect::new(0, 0, n, n), TileMode::Clamp, sigma_of(4.0), sigma_of(4.0));
    let glass = |alpha: f32, inside: &dyn Fn(i32, i32) -> bool| {
        let mut want = base.clone();
        for y in 0..n {
            for x in 0..n {
                if !inside(x, y) {
                    continue;
                }
                for k in 0..4 {
                    want.at_mut(x, y)[k] = want.at(x, y)[k] * (1.0 - alpha) + behind.at(x, y)[k] * alpha;
                }
                if (46..54).contains(&x) && (44..52).contains(&y) {
                    want.at_mut(x, y).fill(1.0);
                }
            }
        }
        over_white(&want)
    };
    let rect = |x: i32, y: i32| (30..70).contains(&x) && (30..66).contains(&y);
    let clip_rect = |c: &mut Renderer| c.push_clip(xywh(30.0, 30.0, 40.0, 36.0));
    let unclip = |c: &mut Renderer| c.pop_clip();
    report("backdrop", &backdrop(1.0, &clip_rect, &unclip), &glass(1.0, &rect), 2, all);
    report("backdrop, faded", &backdrop(0.5, &clip_rect, &unclip), &glass(0.5, &rect), 2, all);
    let ring = oval(xywh(28.0, 26.0, 44.0, 44.0));
    let in_ring = |x: i32, y: i32| (x as f64 + 0.5 - 50.0).hypot(y as f64 + 0.5 - 48.0) < 22.0;
    let clip_ring = |c: &mut Renderer| {
        c.push_clip_path(&ring, Offset::Zero, ClipOp::Intersect);
        // (In the clip before it: as it is, the clip's mask made for it.)
        c.fill_round_rect(xywh(50.0, 56.0, 10.0, 10.0), CornerRadii::Zero, rgba(0.1, 0.7, 0.2, 1.0));
    };
    report("backdrop in a path's clip", &backdrop(1.0, &clip_ring, &unclip), &glass(1.0, &in_ring), 2, |x, y| {
        ((x as f64 + 0.5 - 50.0).hypot(y as f64 + 0.5 - 48.0) - 22.0).abs() < 1.5 // (its edge: coverage)
    });
    let turned = backdrop(
        1.0,
        &|c: &mut Renderer| {
            c.push_transform(&about(50.0, 48.0, |m| m.rotate_z(30.0)));
            c.push_clip(xywh(30.0, 30.0, 40.0, 36.0));
        },
        &|c: &mut Renderer| {
            c.pop_clip();
            c.pop_transform();
        },
    );
    // (Inside the turned rect, well in from its edges, the blur of what is behind on the screen; the white square
    // turned with it: left out.)
    let in_turned = |x: i32, y: i32| {
        let (dx, dy) = (x as f64 + 0.5 - 50.0, y as f64 + 0.5 - 48.0);
        let (c, s) = ((-std::f64::consts::PI / 6.0).cos(), (-std::f64::consts::PI / 6.0).sin());
        let (u, v) = (dx * c - dy * s + 50.0, dx * s + dy * c + 48.0);
        u > 32.0 && u < 68.0 && v > 32.0 && v < 64.0 && !(u > 44.0 && u < 56.0 && v > 42.0 && v < 54.0)
    };
    report("backdrop in a turned clip", &turned, &over_white(&behind), 2, |x, y| !in_turned(x, y));
    let layered = g.render(n, n, |c| {
        c.fill_round_rect(xywh(0.0, 0.0, 96.0, 96.0), CornerRadii::Zero, rgba(0.0, 0.0, 0.0, 1.0)); // under the layer
        c.push_layer(xywh(0.0, 0.0, 96.0, 96.0), CornerRadii::Zero, 1.0, LayerTransform::Identity);
        paint_fills(c);
        c.push_clip(xywh(30.0, 30.0, 40.0, 36.0));
        c.draw_backdrop(xywh(30.0, 30.0, 40.0, 36.0), &BlurEffect(4.0, 4.0, TileMode::Clamp), 1.0);
        c.pop_clip();
        c.pop_layer();
    });
    let mut in_layer = base.clone();
    for y in 0..n {
        for x in 0..n {
            if rect(x, y) {
                in_layer.at_mut(x, y).copy_from_slice(behind.at(x, y));
            }
            in_layer.at_mut(x, y)[3] = 1.0; // (over black)
        }
    }
    report("backdrop in a layer", &layered, &over_white(&in_layer), 2, all);
    // Faded, and cut by a rounded clip around it: what the blur made, cut.
    let cut = g.render(n, n, |c| {
        c.push_clip_round_rect(xywh(24.0, 24.0, 48.0, 48.0), CornerRadii::all(16.0), ClipOp::Intersect);
        c.push_effect_layer(xywh(24.0, 24.0, 48.0, 48.0), &BlurEffect(4.0, 4.0, TileMode::Clamp), 0.5);
        content(c);
        c.pop_layer();
        c.pop_clip();
    });
    let mut half = gaussian(&base, bounds, TileMode::Clamp, sigma_of(4.0), sigma_of(4.0));
    half.rgba.iter_mut().for_each(|v| *v *= 0.5);
    // Inside the clip (a pixel in from its edge) what the blur made; outside it, nothing.
    let inside = |x: i32, y: i32, by: f32| {
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        let (cx, cy) = (px.clamp(40.0, 56.0), py.clamp(40.0, 56.0));
        px > 24.0 + by && px < 72.0 - by && py > 24.0 + by && py < 72.0 - by && (px - cx).hypot(py - cy) < 16.0 - by
    };
    report("faded, cut", &cut, &over_white(&half), 2, |x, y| !inside(x, y, 1.0));
    let (outside, at) = worst_against(&cut, |_, _| [255; 4], |x, y| inside(x, y, -1.0));
    check!(outside == 0, "  faded, cut: off by {outside} outside it at {at:?}");
    // Too big for a texture: blurred around what shows, the same there.
    let tall = g.render(n, n, |c| {
        c.push_effect_layer(xywh(24.0, -6000.0, 48.0, 12096.0), &BlurEffect(4.0, 4.0, TileMode::Decal), 1.0);
        content(c);
        c.pop_layer();
    });
    let tall_bounds = IRect::new(24, -6000, 48, 12096);
    let mut tall_fills = vec![(IRect::new(0, 0, 96, 20), rgba(0.0, 0.0, 0.0, 1.0))];
    tall_fills.extend_from_slice(&fills);
    let mut tall_base = painted(n, n, &tall_fills);
    for y in 0..n {
        for x in 0..n {
            if !(24..72).contains(&x) {
                tall_base.at_mut(x, y).fill(0.0); // (only what is within its bounds across)
            }
        }
    }
    report(
        "too big for a texture",
        &tall,
        &over_white(&gaussian(&tall_base, tall_bounds, TileMode::Decal, sigma_of(4.0), sigma_of(4.0))),
        2,
        all,
    );
}

// ---- Shadows ------------------------------------------------------------------------------------------------------

/// A path's coverage drawn sharp, as an image's alpha.
fn mask_of(g: &mut Gpu, w: i32, h: i32, draw: impl FnOnce(&mut Renderer)) -> Image {
    let p = g.render_cleared(w, h, ClearColor::TRANSPARENT, draw);
    let mut out = Image::new(w, h);
    for y in 0..h {
        for x in 0..w {
            out.at_mut(x, y)[3] = p.at(x, y)[3] as f32 / 255.0;
        }
    }
    out
}

/// A path's shadows are Compose's: under it, its fill and its stroke 2 * spread wide (mitered) each blurred, the
/// two over each other, moved, faded by the color's alpha; inside it (cut to it), the color where its hole (worn
/// away by the spread, moved) blurred isn't. Against those masks drawn sharp here, blurred on the CPU.
pub fn path_shadows_are_composes(g: &mut Gpu) {
    let n = 96;
    let mut star10 = Path::new();
    for i in 0..10 {
        let a = i as f64 * std::f64::consts::PI / 5.0 - std::f64::consts::PI / 2.0;
        let r = if i % 2 == 1 { 14.0 } else { 32.0 };
        let (x, y) = ((48.0 + r * a.cos()) as f32, (46.0 + r * a.sin()) as f32);
        if i == 0 {
            star10.move_to(x, y);
        } else {
            star10.line_to(x, y);
        }
    }
    star10.close();
    let color = rgba(0.2, 0.3, 0.8, 0.7);
    let painted_alpha = |alpha: &[f32]| {
        let mut out = Image::new(n, n);
        for (i, &a) in alpha.iter().enumerate() {
            let a = a * color.alpha();
            out.rgba[i * 4..i * 4 + 4].copy_from_slice(&[color.red() * a, color.green() * a, color.blue() * a, a]);
        }
        over_white(&out)
    };
    let blurred = |i: &Image, radius: f32| {
        if radius > 0.0 {
            gaussian(i, IRect::new(0, 0, n, n), TileMode::Decal, sigma_of(radius), sigma_of(radius))
        } else {
            i.clone()
        }
    };
    let black = Brush::from(Color::Black);
    // (Not blurred: sharp, its edges the path's.)
    for (spread, radius) in [(0.0f32, 5.0f32), (3.0, 5.0), (0.0, 0.0)] {
        let sh = shadow(color, Offset::new(4.0, 6.0), radius, spread);
        let at = Offset::new(4.0, 6.0);
        let fill_mask = mask_of(g, n, n, |c| c.draw_path(&star10, at, &black, Rect::Zero, &fill()));
        let band = mask_of(g, n, n, |c| c.draw_path(&star10, at, &black, Rect::Zero, &PathStroke::new(2.0 * spread)));
        let (f, b) = (blurred(&fill_mask, radius).alpha(), blurred(&band, radius).alpha());
        let drop: Vec<f32> =
            f.iter().zip(&b).map(|(&f, &b)| if spread > 0.0 { 1.0 - (1.0 - f) * (1.0 - b) } else { f }).collect();
        let got = g.render(n, n, |c| c.draw_shadow_path(&star10, Offset::Zero, &sh, false));
        let want = painted_alpha(&drop);
        let (worst, w_at) = worst_against(&got, |x, y| want.at(x, y), none);
        check!(worst <= 2, "  drop shadow (spread {spread}): off by {worst} at {w_at:?}");
        // Inside: cut to the star; its hole worn away by the spread.
        let clip = mask_of(g, n, n, |c| c.draw_path(&star10, Offset::Zero, &black, Rect::Zero, &fill()));
        let hole = mask_of(g, n, n, |c| {
            c.draw_path(&star10, at, &black, Rect::Zero, &fill());
            c.push_blend_mode(BlendMode::Clear);
            if spread > 0.0 {
                c.draw_path(&star10, at, &black, Rect::Zero, &PathStroke::new(2.0 * spread));
            }
            c.pop_blend_mode();
        });
        let (h, inn) = (blurred(&hole, radius).alpha(), clip.alpha());
        let inside: Vec<f32> = inn.iter().zip(&h).map(|(&i, &h)| i * (1.0 - h)).collect();
        let got = g.render(n, n, |c| c.draw_shadow_path(&star10, Offset::Zero, &sh, true));
        let want = painted_alpha(&inside);
        let (worst, w_at) = worst_against(&got, |x, y| want.at(x, y), none);
        check!(worst <= 2, "  inner shadow (spread {spread}): off by {worst} at {w_at:?}");
    }
    // A brush: its color at each pixel over what the shadow covers (out to its radius and spread), times the
    // shadow (as drawn white), times its alpha.
    let card = xywh(24.0, 30.0, 40.0, 26.0);
    let brush = linear(&[rgba(0.9, 0.2, 0.1, 1.0), rgba(0.1, 0.3, 0.9, 1.0)]);
    let plain = shadow(Color::White, Offset::new(2.0, 3.0), 6.0, 2.0);
    let shape = RoundedCornerShape(8.0);
    let density = Density::new(1.0, 1.0);
    let white = mask_of(g, n, n, |c| c.draw_shape_shadow(card, &shape, LayoutDirection::Ltr, &density, &plain, false));
    let brushed = kompoz_graphics::Shadow { brush: Some(brush.clone()), alpha: 0.8, ..plain.clone() };
    let got = g.render(n, n, |c| c.draw_shape_shadow(card, &shape, LayoutDirection::Ltr, &density, &brushed, false));
    let out = 6.0 + 2.0;
    let area = xywh(card.left - out + 2.0, card.top - out + 3.0, card.width() + 2.0 * out, card.height() + 2.0 * out);
    let mut want = Image::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let c = brush.color_at(Offset::new(x as f32 + 0.5, y as f32 + 0.5), area);
            let a = white.at(x, y)[3] * 0.8;
            want.at_mut(x, y).copy_from_slice(&[c.red() * a, c.green() * a, c.blue() * a, a]);
        }
    }
    let want = over_white(&want);
    let (worst, at) = worst_against(&got, |x, y| want.at(x, y), none);
    check!(worst <= 2, "  a brushed shadow: off by {worst} at {at:?}");
    // A shape along its bounds, at fractions of a pixel, not blurred: its edges cut once.
    let mut boxed_path = Path::new();
    boxed_path.add_rect(xywh(20.5, 30.25, 40.3, 25.6), Default::default());
    let boxed = mask_of(g, n, n, |c| c.draw_path(&boxed_path, Offset::new(3.0, 2.0), &black, Rect::Zero, &fill()));
    let got = g.render(n, n, |c| {
        c.draw_shadow_path(&boxed_path, Offset::Zero, &shadow(color, Offset::new(3.0, 2.0), 0.0, 0.0), false)
    });
    let want = painted_alpha(&boxed.alpha());
    let (worst, at) = worst_against(&got, |x, y| want.at(x, y), none);
    check!(worst <= 2, "  a box's shadow: off by {worst} at {at:?}");
}

/// A path's shadow is drawn from the mask it had in a frame before (as Compose keeps a dropShadow's blurred bitmap)
/// when only where it is by whole pixels or its color changed: as drawn without one (GraphicsContext's), outer and
/// inner; made again for another fraction of a pixel.
pub fn path_shadows_are_kept(g: &mut Gpu) {
    let s = star(20.0, 20.0, 16.0);
    let mut draw = |at: Offset, color: Color, inner: bool, plain: bool| {
        g.renderer.path_shadows = 0;
        g.renderer.kept_shadows = 0;
        let sh = shadow(color, Offset::new(2.0, 3.0), 6.0, 2.0);
        let out = g.render(70, 70, |c| {
            if plain {
                default_draw_shadow_path(c, &s, at, &sh, inner);
            } else {
                c.draw_shadow_path(&s, at, &sh, inner);
            }
        });
        (out, g.renderer.kept_shadows)
    };
    let (violet, teal) = (rgba(0.4, 0.1, 0.7, 0.8), rgba(0.1, 0.6, 0.5, 0.6));
    for inner in [false, true] {
        let (first, kept0) = draw(Offset::new(3.3, 2.6), violet, inner, false);
        let (plain, _) = draw(Offset::new(3.3, 2.6), violet, inner, true);
        let (moved, kept1) = draw(Offset::new(10.3, 7.6), violet, inner, false);
        let (tinted, kept2) = draw(Offset::new(10.3, 7.6), teal, inner, false);
        let (plain_tinted, _) = draw(Offset::new(10.3, 7.6), teal, inner, true);
        let (_, kept3) = draw(Offset::new(10.55, 7.6), violet, inner, false);
        check!(kept0 == 0 && kept1 == 1 && kept2 == 1 && kept3 == 0, "  kept {kept0} {kept1} {kept2} {kept3}");
        let worst = worst_against(&first, |x, y| plain.at(x, y), none)
            .0
            .max(worst_against(&tinted, |x, y| plain_tinted.at(x, y), none).0);
        let mut moves = 0;
        for y in 0..60 {
            for x in 0..60 {
                moves = moves.max(max_difference(first.at(x, y), moved.at(x + 7, y + 5)));
            }
        }
        // (A 255th or two: the color rounded once with its coverage, not in a layer first.)
        let what = if inner { "inner" } else { "drop" };
        check!(
            worst <= 2 && moves == 0,
            "  {what} path shadow: off by {worst} from one drawn without a mask, {moves} moved"
        );
    }
}

/// Elevation shadows are Android's: a rounded rect's ambient and spot shadows (against a per-pixel model of Skia's
/// analytic ones), turned, seen through, in a layer and in one a transform enlarges; any other outline blurred as
/// Skia does, in perspective too. At 2.625 px a dp, as a phone's (C++'s dp through k; the light's lengths k times).
pub fn elevation_shadows_are_androids(g: &mut Gpu) {
    let n = 160;
    let k = 2.625f32;
    let light = ShadowLight {
        center: Offset::new(n as f32 / 2.0, 0.0),
        z: 500.0 * k,
        radius: 800.0 * k,
        ..ShadowLight::default()
    };
    g.renderer.set_light(light);
    struct Caster {
        rect: Rect, // dp
        radius: f32,
        elevation: f32,
        alpha: f32,
        turn: f32, // degrees about its middle
        ambient: Color,
        spot: Color,
    }
    // The model: coverage of a rounded rect's falloff (distances in px), at a px of the target.
    let falloff = |d: f64, blur: f64, hole: f64| {
        if d <= 0.0 || (hole > 0.0 && d >= hole) {
            return 0.0;
        }
        let t = (d / blur).clamp(0.0, 1.0);
        ((-4.0 * (1.0 - t) * (1.0 - t)).exp() - 0.018).max(0.0)
    };
    // How far in from its edge.
    let in_round_rect = |x: f64, y: f64, l: f64, t: f64, r: f64, b: f64, radius: f64| {
        let (cx, cy, hx, hy) = ((l + r) / 2.0, (t + b) / 2.0, (r - l) / 2.0, (b - t) / 2.0);
        let (qx, qy) = ((x - cx).abs() - (hx - radius), (y - cy).abs() - (hy - radius));
        -(qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius)
    };
    let expected = |c: &Caster| {
        let k = k as f64;
        let mut out = Image::new(n, n);
        let (cx, cy) =
            ((c.rect.left + c.rect.width() / 2.0) as f64 * k, (c.rect.top + c.rect.height() / 2.0) as f64 * k);
        let turn = c.turn as f64 * std::f64::consts::PI / 180.0;
        let (cs, sn) = (turn.cos(), turn.sin());
        let (l, t, r, b) =
            (c.rect.left as f64 * k, c.rect.top as f64 * k, c.rect.right as f64 * k, c.rect.bottom as f64 * k);
        let rad = (c.radius.min(c.rect.width().min(c.rect.height()) / 2.0)) as f64 * k;
        let z = c.elevation as f64 * k;
        let opaque = c.alpha >= 1.0;
        // Ambient (device px, the caster's frame: turned back about its middle).
        let inset = (z / 2.0).min(150.0);
        let blur_a = inset * (1.0 + z / 128.0);
        // Spot: cast from the light.
        let (lx, ly, lz, lr) = (light.center.x as f64, light.center.y as f64, light.z as f64, light.radius as f64);
        let ratio = (z / (lz - z)).clamp(0.0, 0.95);
        let sc = (lz / (lz - z)).clamp(1.0, 1.95);
        let blur_s = lr * ratio;
        // The cast caster: its middle cast, its size scaled (a turned rect stays one, turned the same).
        let (ccx, ccy) = (sc * cx - ratio * lx, sc * cy - ratio * ly);
        let (hw, hh, crad) = ((r - l) / 2.0 * sc, (b - t) / 2.0 * sc, rad * sc);
        // (Offsets measured in the caster's frame: its middle's move turned back.)
        let mx = (ccx - cx) * cs + (ccy - cy) * sn;
        let my = -(ccx - cx) * sn + (ccy - cy) * cs;
        let (dl, dr2) = (mx - (hw - (r - l) / 2.0), mx + (hw - (r - l) / 2.0));
        let (dt, db) = (my - (hh - (b - t) / 2.0), my + (hh - (b - t) / 2.0));
        let max_offset = if rad <= 0.0 {
            dl.abs().max(dt.abs()).max(dr2.abs()).max(db.abs())
        } else {
            let drr = crad - rad;
            let (ux, uy, vx, vy) = (dl + drr, dt + drr, dr2 - drr, db - drr);
            (ux * ux + uy * uy).max(vx * vx + vy * vy).sqrt() + drr
        };
        let inset_s = blur_s + blur_s.max(max_offset);
        for y in 0..n {
            for x in 0..n {
                let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                // Into the caster's frame (turned back about its middle).
                let ax = cx + (px - cx) * cs + (py - cy) * sn;
                let ay = cy - (px - cx) * sn + (py - cy) * cs;
                let d_a = in_round_rect(ax, ay, l - inset, t - inset, r + inset, b + inset, rad + inset);
                let fa = falloff(d_a, blur_a, if opaque { inset } else { -1.0 });
                let sx = ccx + (px - ccx) * cs + (py - ccy) * sn;
                let sy = ccy - (px - ccx) * sn + (py - ccy) * cs;
                let d_s = in_round_rect(
                    sx,
                    sy,
                    ccx - hw - blur_s,
                    ccy - hh - blur_s,
                    ccx + hw + blur_s,
                    ccy + hh + blur_s,
                    crad + blur_s,
                );
                let fs = falloff(d_s, 2.0 * blur_s, if opaque { inset_s } else { -1.0 });
                let aa = c.ambient.alpha() as f64 * light.ambient_alpha as f64 * c.alpha as f64 * fa;
                let sa = c.spot.alpha() as f64 * light.spot_alpha as f64 * c.alpha as f64 * fs;
                // The two over each other (premultiplied).
                let mix = |a: f32, s: f32| (s as f64 * sa + a as f64 * aa * (1.0 - sa)) as f32;
                out.at_mut(x, y).copy_from_slice(&[
                    mix(c.ambient.red(), c.spot.red()),
                    mix(c.ambient.green(), c.spot.green()),
                    mix(c.ambient.blue(), c.spot.blue()),
                    (sa + aa * (1.0 - sa)) as f32,
                ]);
            }
        }
        over_white(&out)
    };
    let px_rect = |r: Rect| Rect::new(r.left * k, r.top * k, r.right * k, r.bottom * k);
    let black = rgba(0.0, 0.0, 0.0, 1.0);
    let tint = rgba(0.8, 0.1, 0.5, 1.0);
    let casters = [
        Caster {
            rect: xywh(8.0, 14.0, 28.0, 20.0),
            radius: 6.0,
            elevation: 6.0,
            alpha: 1.0,
            turn: 0.0,
            ambient: black,
            spot: black,
        }, // left of the light
        Caster {
            rect: xywh(30.0, 30.0, 24.0, 16.0),
            radius: 6.0,
            elevation: 12.0,
            alpha: 1.0,
            turn: 0.0,
            ambient: black,
            spot: black,
        }, // right of it, higher
        Caster {
            rect: xywh(10.0, 30.0, 20.0, 20.0),
            radius: 10.0,
            elevation: 8.0,
            alpha: 1.0,
            turn: 0.0,
            ambient: black,
            spot: tint,
        }, // a circle; a colored spot
        Caster {
            rect: xywh(12.0, 12.0, 30.0, 20.0),
            radius: 0.0,
            elevation: 4.0,
            alpha: 1.0,
            turn: 0.0,
            ambient: black,
            spot: black,
        }, // a rect
        Caster {
            rect: xywh(14.0, 16.0, 26.0, 22.0),
            radius: 5.0,
            elevation: 8.0,
            alpha: 0.6,
            turn: 0.0,
            ambient: black,
            spot: black,
        }, // seen through: under it too
        Caster {
            rect: xywh(16.0, 18.0, 26.0, 18.0),
            radius: 4.0,
            elevation: 6.0,
            alpha: 1.0,
            turn: 30.0,
            ambient: tint,
            spot: black,
        }, // turned
    ];
    for c in &casters {
        let got = g.render(n, n, |ctx| {
            let at = px_rect(c.rect);
            if c.turn != 0.0 {
                let mid = at.center();
                ctx.push_transform(&about(mid.x, mid.y, |m| m.rotate_z(c.turn)));
            }
            let o = if c.radius > 0.0 {
                rounded(at.width(), at.height(), c.radius * k)
            } else {
                Outline::Rectangle(Rect::new(0.0, 0.0, at.width(), at.height()))
            };
            let es = ElevationShadow {
                plane: [0.0, 0.0, c.elevation * k],
                ambient_color: c.ambient,
                spot_color: c.spot,
                alpha: c.alpha,
            };
            ctx.draw_elevation_shadow(&o, at.top_left(), &es);
            if c.turn != 0.0 {
                ctx.pop_transform();
            }
        });
        let want = expected(c);
        let (worst, at) = worst_against(&got, |x, y| want.at(x, y), none);
        check!(worst <= 2, "  elevation shadow ({} dp up, turned {}): off by {worst} at {at:?}", c.elevation, c.turn);
    }
    // In an offscreen layer: the light where the window's is (the layer moved; a narrow light, so that where it is
    // shows), and in one a transform enlarges, as Android works it out in the layer's px, then enlarges it (its spot
    // shadow as one drawn at twice the density, the light's center where it lands).
    g.renderer.set_light(ShadowLight { radius: 30.0 * k, ..light });
    let mut in_layer = |layered: bool| {
        g.render(n, n, |ctx| {
            if layered {
                ctx.push_layer(px_rect(xywh(7.0, 9.0, 40.0, 34.0)), CornerRadii::Zero, 1.0, LayerTransform::Identity);
            }
            ctx.draw_elevation_shadow(
                &rounded(22.0 * k, 14.0 * k, 4.0 * k),
                Offset::new(16.0 * k, 18.0 * k),
                &ElevationShadow { plane: [0.0, 0.0, 24.0 * k], ambient_color: black, spot_color: black, alpha: 1.0 },
            );
            if layered {
                ctx.pop_layer();
            }
        })
    };
    let (screen, layer) = (in_layer(false), in_layer(true));
    g.renderer.set_light(light);
    let (worst, at) = worst_against(&layer, |x, y| screen.at(x, y), none);
    check!(worst <= 2, "  elevation shadow in a layer: off by {worst} at {at:?}");
    let spot_only = |plane_z: f32| ElevationShadow {
        plane: [0.0, 0.0, plane_z],
        ambient_color: Color::Transparent,
        spot_color: black,
        alpha: 1.0,
    };
    let enlarged = g.render(n, n, |ctx| {
        ctx.push_transform(&scaled(2.0));
        ctx.push_layer(px_rect(xywh(0.0, 0.0, 30.0, 30.0)), CornerRadii::Zero, 1.0, LayerTransform::Identity);
        ctx.draw_elevation_shadow(
            &rounded(14.0 * k, 10.0 * k, 3.0 * k),
            Offset::new(6.0 * k, 8.0 * k),
            &spot_only(6.0 * k),
        );
        ctx.pop_layer();
        ctx.pop_transform();
    });
    // (At twice the density: the light's lengths twice as many px, its center where it was on the window.)
    let k2 = 2.0 * k;
    g.renderer.set_light(ShadowLight { z: 500.0 * k2, radius: 800.0 * k2, ..light });
    let twice = g.render(n, n, |ctx| {
        ctx.draw_elevation_shadow(
            &rounded(14.0 * k2, 10.0 * k2, 3.0 * k2),
            Offset::new(6.0 * k2, 8.0 * k2),
            &spot_only(6.0 * k2),
        );
    });
    g.renderer.set_light(light);
    let (worst, at) = worst_against(&enlarged, |x, y| twice.at(x, y), none);
    check!(worst <= 2, "  elevation shadow in an enlarged layer: off by {worst} at {at:?}");
    // Any other outline: blurred as Skia does (the ambient shadow its outline stroked and filled, the spot one cast),
    // against those masks drawn sharp here.
    let mut star10 = Path::new();
    for i in 0..10 {
        let a = i as f64 * std::f64::consts::PI / 5.0 - std::f64::consts::PI / 2.0;
        let r = if i % 2 == 1 { 6.0 } else { 14.0 };
        let (x, y) = ((30.0 + r * a.cos()) as f32 * k, (28.0 + r * a.sin()) as f32 * k);
        if i == 0 {
            star10.move_to(x, y);
        } else {
            star10.line_to(x, y);
        }
    }
    star10.close();
    let elevation = 8.0;
    let z = elevation * k;
    let outset = (z / 2.0).min(150.0);
    let blur_a = 0.5 * outset * (1.0 + z / 128.0);
    let stroke = 0.5 * (outset - blur_a);
    let black_brush = Brush::from(black);
    let ambient_mask = mask_of(g, n, n, |c| {
        c.draw_path(&star10, Offset::Zero, &black_brush, Rect::Zero, &fill());
        if stroke > 0.0 {
            c.draw_path(&star10, Offset::Zero, &black_brush, Rect::Zero, &PathStroke::new(stroke));
        }
    });
    let (lz, ratio) = (light.z, z / (light.z - z));
    let (sc, blur_s) = (lz / (lz - z), light.radius * ratio);
    let spot_mask = mask_of(g, n, n, |c| {
        let mut m = Matrix::new(); // (the cast: scaled about the light's foot)
        m.translate(-ratio * light.center.x, -ratio * light.center.y, 0.0);
        m.scale(sc, sc, 1.0);
        c.push_transform(&m);
        c.draw_path(&star10, Offset::Zero, &black_brush, Rect::Zero, &fill());
        c.pop_transform();
    });
    let whole = IRect::new(0, 0, n, n);
    let a = gaussian(&ambient_mask, whole, TileMode::Decal, sigma_of(blur_a), sigma_of(blur_a));
    let sp = gaussian(&spot_mask, whole, TileMode::Decal, sigma_of(blur_s), sigma_of(blur_s));
    let mut both = Image::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let (aa, ss) = (a.at(x, y)[3] * light.ambient_alpha, sp.at(x, y)[3] * light.spot_alpha);
            both.at_mut(x, y)[3] = ss + aa * (1.0 - ss);
        }
    }
    let flat = |z: f32| ElevationShadow::flat(z);
    let got = g.render(n, n, |c| c.draw_elevation_shadow(&Outline::Generic(star10.clone()), Offset::Zero, &flat(z)));
    let want = over_white(&both);
    let (worst, at) = worst_against(&got, |x, y| want.at(x, y), none);
    check!(worst <= 2, "  a star's elevation shadow: off by {worst} at {at:?}");
    // Its ambient shadow alone, at full strength (the stroke under the blur showing).
    g.renderer.set_light(ShadowLight { ambient_alpha: 1.0, ..light });
    let got = g.render(n, n, |c| {
        let es =
            ElevationShadow { plane: [0.0, 0.0, z], ambient_color: black, spot_color: Color::Transparent, alpha: 1.0 };
        c.draw_elevation_shadow(&Outline::Generic(star10.clone()), Offset::Zero, &es);
    });
    g.renderer.set_light(light);
    let mut ambient_only = Image::new(n, n);
    for y in 0..n {
        for x in 0..n {
            ambient_only.at_mut(x, y)[3] = a.at(x, y)[3];
        }
    }
    let want = over_white(&ambient_only);
    let (worst, at) = worst_against(&got, |x, y| want.at(x, y), none);
    check!(worst <= 2, "  a star's ambient shadow: off by {worst} at {at:?}");
    // In perspective (further to the right, to the left, down; and turned half round as well): the spot shadow of a
    // right triangle (mirrored, it wouldn't be the same) its corners cast from the light (each at its own height),
    // blurred.
    let bx = px_rect(xywh(10.0, 14.0, 30.0, 20.0));
    for (row, w, turn) in [(0usize, 0.004f32, 0.0f32), (0, -0.004, 0.0), (1, 0.005, 0.0), (0, 0.004, 180.0)] {
        let m = if turn != 0.0 { about(bx.center().x, bx.center().y, |m| m.rotate_z(turn)) } else { Matrix::new() };
        let mut seen = Matrix::new();
        seen.set(row, 3, w / k); // (w = 1 + x (or y) dp times that)
        seen.times_assign(&m);
        let corners = [Offset::new(bx.left, bx.top), Offset::new(bx.right, bx.top), Offset::new(bx.left, bx.bottom)];
        let cast: Vec<Offset> = corners
            .iter()
            .map(|&q| {
                let d = seen.map(q);
                let (lx, ly, rr) = (light.center.x as f64, light.center.y as f64, (z / (lz - z)) as f64);
                Offset::new((d.x as f64 - (lx - d.x as f64) * rr) as f32, (d.y as f64 - (ly - d.y as f64) * rr) as f32)
            })
            .collect();
        let cut = mask_of(g, n, n, |c| {
            let mut p = Path::new();
            p.move_to(cast[0].x, cast[0].y);
            p.line_to(cast[1].x, cast[1].y);
            p.line_to(cast[2].x, cast[2].y);
            p.close();
            c.draw_path(&p, Offset::Zero, &black_brush, Rect::Zero, &fill());
        });
        let cut_blur = gaussian(&cut, whole, TileMode::Decal, sigma_of(blur_s), sigma_of(blur_s));
        let mut spot_alone = Image::new(n, n);
        for y in 0..n {
            for x in 0..n {
                spot_alone.at_mut(x, y)[3] = cut_blur.at(x, y)[3] * light.spot_alpha;
            }
        }
        let mut triangle = Path::new();
        triangle.move_to(0.0, 0.0);
        triangle.line_to(bx.width(), 0.0);
        triangle.line_to(0.0, bx.height());
        triangle.close();
        let got = g.render(n, n, |c| {
            c.push_transform(&seen);
            c.draw_elevation_shadow(&Outline::Generic(triangle.clone()), bx.top_left(), &spot_only(z));
            c.pop_transform();
        });
        let want = over_white(&spot_alone);
        let (worst, at) = worst_against(&got, |x, y| want.at(x, y), none);
        let axis = if row == 1 { 'y' } else { 'x' };
        check!(worst <= 3, "  elevation shadow in perspective ({axis} {w}, turned {turn}): off by {worst} at {at:?}");
    }
    g.renderer.set_light(ShadowLight::default());
}

// ---- Kept layers and the pool -------------------------------------------------------------------------------------

/// A layer seen for the first time (a fade starting) is kept from that frame: in the next, with nothing in it
/// changed, it is composited from those pixels, not drawn whole again.
pub fn a_layer_is_kept_from_its_first_frame(g: &mut Gpu) {
    let mut renderer = Renderer::new(Box::new(g.device.clone())).expect("a renderer");
    let mut list = DisplayList::new();
    let (screen, fading) = (1u64, 2u64);
    let record = |list: &mut DisplayList, alpha: f32| {
        list.begin(Size::new(80.0, 60.0));
        list.begin_node(screen);
        list.fill_round_rect(xywh(0.0, 0.0, 80.0, 60.0), CornerRadii::Zero, Color::White);
        list.begin_node(fading);
        if alpha < 1.0 {
            list.push_layer(xywh(10.0, 10.0, 50.0, 30.0), CornerRadii::all(4.0), alpha, LayerTransform::Identity);
        }
        list.fill_round_rect(xywh(10.0, 10.0, 50.0, 30.0), CornerRadii::Zero, rgba(0.3, 0.5, 0.9, 1.0));
        if alpha < 1.0 {
            list.pop_layer();
        }
        list.end_node();
        list.end_node();
        list.end()
    };
    let target = g.device.create_texture(80, 60, Format::Rgba8, TARGET).expect("a texture");
    let mut kept = Vec::new();
    for alpha in [1.0, 0.8, 0.6, 0.4] {
        renderer.device_mut().begin_frame();
        renderer.kept_layers = 0;
        if record(&mut list, alpha).is_some() {
            renderer.draw_frame(target.id(), &list, true);
        }
        renderer.flush();
        renderer.device_mut().end_frame(false);
        renderer.end_frame();
        kept.push(renderer.kept_layers);
    }
    check!(kept == [0, 0, 1, 1], "  kept {kept:?}"); // (none yet; the layer just come; kept since)
}

/// A frame of what kept layers draw is what the same drawn afresh is, step by step: faded, turned, moved, moved
/// by a fraction, a part changed, an effect's blur grown, a layer grown, a layer's bounds wider.
pub fn kept_layers_draw_as_drawn_afresh(g: &mut Gpu) {
    let (w, h) = (240, 200);
    let k = 2.0f32; // (120 x 100 dp)
    let device = WatchedDevice::new(&g.device, true); // (the whole shader: what is drawn again shaded as all of it is)
    let mut renderer = Renderer::new(Box::new(device)).expect("a renderer");
    let mut list = DisplayList::new();
    let (screen, card, frosted, turned, twins, corner) = (1u64, 2u64, 3u64, 4u64, 5u64, 6u64);
    let star_path = star(8.0 * k, 8.0 * k, 7.0 * k);
    #[derive(Clone, Copy)]
    struct Step {
        alpha: f32,
        turn: f32,
        x: f32,
        frosted_look: bool, // what is in the blurred layer
        corners: bool,      // a mark at two corners of the card: two areas over it
        twin_alpha: f32,    // the two alike layers'
        text: bool,         // the card's text's color (a part of it changed)
        blur: f32,          // the frosted layer's blur
        grown: f32,         // the turned layer's scale
        wide: bool,         // the card's layer wider (what is in it the same)
    }
    let first = Step {
        alpha: 1.0,
        turn: 0.0,
        x: 10.0,
        frosted_look: false,
        corners: false,
        twin_alpha: 1.0,
        text: false,
        blur: 3.0,
        grown: 1.0,
        wide: false,
    };
    let r = |x: f32, y: f32, w: f32, h: f32| xywh(x * k, y * k, w * k, h * k);
    let record = |into: &mut DisplayList, s: &Step| {
        into.begin(Size::new(w as f32, h as f32));
        into.begin_node(screen);
        into.fill_round_rect(r(0.0, 0.0, 120.0, 100.0), CornerRadii::Zero, rgba(0.95, 0.95, 0.97, 1.0));
        into.begin_node(card);
        // (Part of a pixel wide.)
        into.push_layer(
            r(s.x, 10.0, if s.wide { 66.0 } else { 59.75 }, 44.0),
            CornerRadii::all(8.0 * k),
            s.alpha,
            LayerTransform::Identity,
        );
        into.fill_round_rect(r(s.x, 10.0, 60.0, 44.0), CornerRadii::Zero, rgba(0.3, 0.5, 0.9, 1.0));
        let text = text_at("Kart", Offset::new(s.x + 6.0, 30.0), Font::new(12.0), k);
        into.draw_text(&text, if s.text { rgba(1.0, 0.9, 0.3, 1.0) } else { Color::White });
        into.draw_path(
            &star_path,
            Offset::new((s.x + 40.0) * k, 30.0 * k),
            &Brush::from(rgba(1.0, 0.8, 0.2, 1.0)),
            Rect::Zero,
            &fill(),
        );
        into.pop_layer();
        into.end_node();
        into.begin_node(frosted);
        into.push_effect_layer(r(8.0, 60.0, 50.0, 30.0), &BlurEffect(s.blur * k, s.blur * k, TileMode::Clamp), 1.0);
        let look = if s.frosted_look { rgba(0.9, 0.2, 0.3, 1.0) } else { rgba(0.2, 0.7, 0.3, 1.0) };
        into.fill_round_rect(r(12.0, 64.0, 20.0, 20.0), CornerRadii::all(4.0 * k), look);
        into.draw_text(&text_at("Bulanık", Offset::new(14.0, 82.0), Font::new(10.0), k), Color::Black);
        into.pop_layer();
        into.end_node();
        into.begin_node(turned);
        let tf = LayerTransform { scale_x: s.grown, scale_y: s.grown, rotation_z: s.turn, ..LayerTransform::Identity };
        into.push_layer(r(76.0, 58.0, 34.0, 34.0), CornerRadii::Zero, 1.0, tf);
        into.draw_shadow(
            r(80.0, 62.0, 24.0, 24.0),
            CornerRadii::all(6.0 * k),
            &shadow(rgba(0.0, 0.0, 0.0, 0.4), Offset::new(0.0, 2.0 * k), 3.0 * k, 0.0),
            false,
        );
        into.fill_round_rect(r(80.0, 62.0, 24.0, 24.0), CornerRadii::all(6.0 * k), rgba(0.8, 0.4, 0.1, 1.0));
        into.pop_layer();
        into.end_node();
        into.begin_node(twins);
        into.push_layer(r(90.0, 6.0, 10.0, 10.0), CornerRadii::all(3.0 * k), s.twin_alpha, LayerTransform::Identity);
        into.fill_round_rect(r(90.0, 6.0, 10.0, 10.0), CornerRadii::Zero, rgba(0.1, 0.6, 0.2, 1.0));
        into.pop_layer();
        into.push_layer(r(104.0, 6.0, 10.0, 10.0), CornerRadii::all(3.0 * k), s.twin_alpha, LayerTransform::Identity);
        into.fill_round_rect(r(104.0, 6.0, 10.0, 10.0), CornerRadii::Zero, rgba(0.6, 0.1, 0.5, 1.0));
        into.pop_layer();
        into.end_node();
        into.begin_node(corner);
        if s.corners {
            into.fill_round_rect(r(s.x - 2.0, 8.0, 4.0, 4.0), CornerRadii::Zero, Color::Black);
            into.fill_round_rect(r(s.x + 58.0, 52.0, 4.0, 4.0), CornerRadii::Zero, Color::Black);
        }
        into.end_node();
        into.end_node();
        into.end()
    };
    let kept = g.device.create_texture(w, h, Format::Rgba8, TARGET).expect("a texture");
    let fresh = g.device.create_texture(w, h, Format::Rgba8, TARGET).expect("a texture");
    // (From the tenth step on: a quarter of a pixel on, as wide in pixels.)
    let later = Step { alpha: 1.0, x: 15.625, frosted_look: true, ..first };
    let steps = [
        first,
        Step { corners: true, twin_alpha: 0.9, ..first }, // (the card the same: drawn whole in the first area, kept for the second)
        Step { alpha: 0.8, twin_alpha: 0.8, ..first },
        Step { alpha: 0.6, turn: 10.0, ..first },
        Step { alpha: 0.6, turn: 25.0, x: 12.0, ..first },
        Step { alpha: 0.6, turn: 25.0, x: 12.0, frosted_look: true, ..first },
        Step { alpha: 0.6, turn: 25.0, x: 12.0, frosted_look: true, ..first },
        Step { alpha: 0.4, turn: 40.0, x: 15.5, frosted_look: true, corners: true, ..first },
        Step { alpha: 1.0, x: 15.5, frosted_look: true, ..first },
        later,                                                                       // by a quarter of a pixel
        Step { text: true, ..later },                                                // part of the card changed
        Step { text: true, alpha: 0.7, ..later },                                    // and it faded
        Step { text: true, alpha: 0.7, blur: 5.0, ..later },                         // the blur grew
        Step { text: true, alpha: 0.7, blur: 5.0, grown: 1.6, ..later },             // the turned layer grew
        Step { text: true, alpha: 0.7, blur: 5.0, grown: 1.6, wide: true, ..later }, // the card's layer wider
    ];
    let mut kept_layers = 0;
    for (i, step) in steps.iter().enumerate() {
        renderer.device_mut().begin_frame();
        renderer.kept_layers = 0;
        if record(&mut list, step).is_some() {
            renderer.draw_frame(kept.id(), &list, true);
        }
        kept_layers += renderer.kept_layers;
        let mut afresh = DisplayList::new(); // (a recording of its own: its layers are none kept)
        record(&mut afresh, step);
        renderer.draw_frame(fresh.id(), &afresh, false);
        renderer.flush();
        let a =
            Pixels { width: w, height: h, rgba: renderer.device_mut().read_pixels(kept.id(), IRect::new(0, 0, w, h)) };
        let b =
            Pixels { width: w, height: h, rgba: renderer.device_mut().read_pixels(fresh.id(), IRect::new(0, 0, w, h)) };
        renderer.device_mut().end_frame(false);
        renderer.end_frame();
        let (worst, at) = worst_against(&a, |x, y| b.at(x, y), none);
        check!(worst <= 1, "  step {i}: off by {worst} at {at:?}");
    }
    check!(kept_layers >= 10, "  {kept_layers} layers kept");
}

/// A layer's texture is one of a few sizes (as Skia's approximate fit), so a layer whose size changes from frame to
/// frame (clipped to the screen as it scrolls in) takes one kept unused rather than a new one each frame; those kept
/// unused go when memory is trimmed.
pub fn layer_textures_come_in_few_sizes(g: &mut Gpu) {
    let frame = |g: &mut Gpu, height: f32| {
        g.renderer.textures_made = 0;
        let out = g.render(64, 1100, |c| {
            c.push_layer(xywh(0.0, 0.0, 60.0, height), CornerRadii::Zero, 0.5, LayerTransform::Identity);
            c.fill_round_rect(xywh(0.0, 0.0, 60.0, height), CornerRadii::Zero, rgba(0.1, 0.55, 0.3, 1.0));
            c.pop_layer();
        });
        (out, g.renderer.textures_made)
    };
    g.renderer.trim_memory(); // (none left by the tests before)
    let mut made = 0;
    let mut height = 600.0;
    while height <= 1000.0 {
        made += frame(g, height).1;
        height += 40.0;
    }
    check!(made == 1, "  {made} made"); // (64 x 1024 for all)
    let (out, again) = frame(g, 1000.0);
    check!(again == 0 && near(out.at(30, 999), 140, 198, 166, 2) && near(out.at(30, 1001), 255, 255, 255, 2));
    g.renderer.trim_memory();
    check!(frame(g, 1000.0).1 == 1);
}
