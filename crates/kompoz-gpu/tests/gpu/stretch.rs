//! Stretched layers (Android's stretch overscroll, hwui's StretchEffect): each pixel against hwui's shader worked out
//! here on its own (its SkSL and getShader's uniforms, transcribed below, sampling the layer's pixels as Skia's
//! clamped, linearly filtered image shader does), pulled from each edge, both ways at once, by as much as EdgeEffect
//! pulls and by more; enlarged by a transform; through a blur first; faded; kept from frame to frame while only the
//! stretch changes; and a scene of them against its picture.

use crate::exact::{none, worst_against};
use crate::golden::matches_golden;
use crate::harness::{Gpu, Pixels, WatchedDevice, max_difference};
use crate::scenes::{checker, draw_image, linear, rgba, rounded, through_layer, xywh};
use kompoz_gpu::Renderer;
use kompoz_gpu::device::{ClearColor, Device, Format, IRect, TextureUsage};
use kompoz_graphics::{BlurEffect, Color, Matrix, Offset, Rect, RenderEffect, Size, StretchEffect, TileMode};
use kompoz_ui::DisplayList;
use kompoz_ui::graphics_context::{CornerRadii, GraphicsContext};

const TARGET: TextureUsage = TextureUsage::RENDER_TARGET.union(TextureUsage::SAMPLED);

// ---- hwui's shader, on its own -------------------------------------------------------------------------------------

/// Where hwui's stretch shader samples the content for the layer px `coord` of a `w` x `h` layer stretched by `dir`
/// (StretchEffect.cpp's stretchShader `main`, with the uniforms getShader sets: uStretchAffectedDist = the size,
/// uDistanceStretched = size / (1 + |dir|), uInverseDistanceStretched = 1 / size, uDistDiff = that less the size,
/// uScroll = 0, uInterpolationStrength = 0.7).
fn hwui_sample_at(coord: (f32, f32), dir: (f32, f32), w: f32, h: f32) -> (f32, f32) {
    fn axis(in_pos: f32, overscroll: f32, size: f32) -> f32 {
        let affected = size;
        let inverse = 1.0 / size;
        let distance_stretched = size / (1.0 + overscroll.abs());
        let distance_diff = distance_stretched - size;
        let strength = 0.7f32;
        let mix = |a: f32, b: f32, t: f32| a * (1.0 - t) + b * t;
        if overscroll > 0.0 {
            if in_pos <= affected {
                let offset_pos = affected - in_pos;
                let variation = mix(1.0, offset_pos * inverse, strength);
                distance_stretched - offset_pos / (1.0 + overscroll * variation)
            } else {
                distance_diff + in_pos
            }
        } else if overscroll < 0.0 {
            let reverse = size - affected;
            if in_pos >= reverse {
                let offset_pos = in_pos - reverse;
                let variation = mix(1.0, offset_pos * inverse, strength);
                size - (distance_stretched - offset_pos / (1.0 + (-overscroll) * variation))
            } else {
                -distance_diff + in_pos
            }
        } else {
            in_pos
        }
    }
    (axis(coord.0, dir.0, w), axis(coord.1, dir.1, h))
}

/// The pixels `content` holds (premultiplied, 0..255) at the point (x, y) of its texels, as Skia's image shader
/// samples it clamped and linearly filtered.
fn sample_clamped(content: &Pixels, x: f32, y: f32) -> [f32; 4] {
    let (w, h) = (content.width as f32, content.height as f32);
    let (x, y) = (x.clamp(0.5, w - 0.5) - 0.5, y.clamp(0.5, h - 0.5) - 0.5);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let texel = |i: f32, j: f32| {
        let (i, j) = ((i as i32).min(content.width - 1), (j as i32).min(content.height - 1));
        content.at(i, j).map(|v| v as f32)
    };
    let (a, b, c, d) = (texel(x0, y0), texel(x0 + 1.0, y0), texel(x0, y0 + 1.0), texel(x0 + 1.0, y0 + 1.0));
    std::array::from_fn(|k| {
        let top = a[k] + (b[k] - a[k]) * fx;
        let bottom = c[k] + (d[k] - c[k]) * fx;
        top + (bottom - top) * fy
    })
}

/// What a stretched layer's pixel comes to over white: the warped content (stored in 8 bits) faded by `alpha`, over
/// white.
fn over_white(s: [f32; 4], alpha: f32) -> [u8; 4] {
    let stored = s.map(|v| v.round());
    let a = stored[3] * alpha;
    let rgb = |v: f32| (v * alpha + 255.0 - a).round().clamp(0.0, 255.0) as u8;
    [rgb(stored[0]), rgb(stored[1]), rgb(stored[2]), 255]
}

/// The layer's px a target pixel's middle is at, and where hwui's shader takes its content from, in `content`'s
/// texels (`k` of them a px), for a layer at `at` (target pixels, `k` of them a px) of `w` x `h` px.
fn reference(
    content: &Pixels,
    k: f32,
    at: (f32, f32),
    size: (f32, f32),
    dir: (f32, f32),
    alpha: f32,
) -> impl Fn(i32, i32) -> [u8; 4] {
    move |x, y| {
        let local = ((x as f32 + 0.5 - at.0) / k, (y as f32 + 0.5 - at.1) / k);
        if local.0 < 0.0 || local.1 < 0.0 || local.0 > size.0 || local.1 > size.1 {
            return [255; 4];
        }
        let (u, v) = hwui_sample_at(local, dir, size.0, size.1);
        over_white(sample_clamped(content, u * k, v * k), alpha)
    }
}

// ---- What is stretched ---------------------------------------------------------------------------------------------

const W: f32 = 96.0;
const H: f32 = 64.0;

/// A card of `W` x `H` px from `o`: 8 px stripes across and down in four colors, a bar of each, a translucent and a
/// clear part (whole pixels: its pixels are known exactly).
fn card(c: &mut dyn GraphicsContext, o: Offset) {
    let r = |x: f32, y: f32, w: f32, h: f32| xywh(o.x + x, o.y + y, w, h);
    let colors =
        [rgba(0.9, 0.2, 0.2, 1.0), rgba(0.2, 0.6, 0.9, 1.0), rgba(0.95, 0.8, 0.1, 1.0), rgba(0.2, 0.7, 0.3, 1.0)];
    for i in 0..12 {
        c.fill_round_rect(r(i as f32 * 8.0, 0.0, 8.0, 32.0), CornerRadii::Zero, colors[i % 4]);
    }
    for j in 0..4 {
        c.fill_round_rect(r(0.0, 32.0 + j as f32 * 8.0, 64.0, 8.0), CornerRadii::Zero, colors[(j + 1) % 4]);
    }
    c.fill_round_rect(r(64.0, 32.0, 16.0, 32.0), CornerRadii::Zero, rgba(0.1, 0.1, 0.12, 0.5)); // (translucent)
    // (Its last 16 x 32 px clear: white shows through.)
    c.fill_round_rect(r(4.0, 4.0, 3.0, 56.0), CornerRadii::Zero, Color::Black);
    c.fill_round_rect(r(2.0, 12.0, 92.0, 2.0), CornerRadii::Zero, Color::White);
}

/// The card's pixels (premultiplied), `k` texels a px, as a layer holds them.
fn card_pixels(g: &mut Gpu, k: f32) -> Pixels {
    let (w, h) = ((W * k) as i32, (H * k) as i32);
    g.render_cleared(w, h, ClearColor::TRANSPARENT, |c| {
        let mut m = Matrix::new();
        m.scale(k, k, 1.0);
        c.push_transform(&m);
        card(c, Offset::Zero);
        c.pop_transform();
    })
}

/// The card stretched by `dir` in a layer at (16, 16), faded by `alpha`, `k` target pixels a px.
fn stretched_card(g: &mut Gpu, k: f32, dir: (f32, f32), alpha: f32) -> Pixels {
    let (w, h) = (((W + 32.0) * k) as i32, ((H + 32.0) * k) as i32);
    g.render(w, h, |c| {
        let mut m = Matrix::new();
        m.scale(k, k, 1.0);
        c.push_transform(&m);
        let stretch = StretchEffect::new(Offset::new(dir.0, dir.1), W, H);
        c.push_stretch_layer(xywh(16.0, 16.0, W, H), &stretch, None, alpha);
        card(c, Offset::new(16.0, 16.0));
        c.fill_round_rect(xywh(0.0, 0.0, 200.0, 12.0), CornerRadii::Zero, Color::Black); // (outside: cut away)
        c.pop_layer();
        c.pop_transform();
    })
}

/// Pulled from the top (EdgeEffect's most, and more), the bottom, the left, the right, two ways at once.
const PULLS: [(f32, f32); 7] =
    [(0.0, 0.032), (0.0, 0.3), (0.0, -0.3), (0.3, 0.0), (-0.3, 0.0), (0.2, -0.15), (-0.5, 0.5)];

/// Each pixel of a stretched layer is hwui's: its content where hwui's shader takes it from, clamped to the layer and
/// filtered linearly, over what is under it. At a pixel a px; enlarged twice by a transform (the layer's texels
/// twice as many, the warp in its own px); faded.
pub fn stretched_layers_are_hwuis(g: &mut Gpu) {
    let content = card_pixels(g, 1.0);
    let mut worst_seen = 0;
    for dir in PULLS {
        let p = stretched_card(g, 1.0, dir, 1.0);
        let (worst, at) = worst_against(&p, reference(&content, 1.0, (16.0, 16.0), (W, H), dir, 1.0), none);
        worst_seen = worst_seen.max(worst);
        check!(worst <= 2, "  pulled {dir:?}: off by {worst} at {at:?}");
    }
    // (It does stretch: pulled from the top, the content's first stripe's black bar ends further down.)
    let p = stretched_card(g, 1.0, (0.0, 0.3), 1.0);
    let plain = stretched_card(g, 1.0, (0.0, 0.0), 1.0);
    check!(max_difference(p.at(21, 77), plain.at(21, 77)) > 100, "  {:?} {:?}", p.at(21, 77), plain.at(21, 77));
    check!(p.at(21, 16) == plain.at(21, 16)); // (the edge pulled stays)
    let twice = card_pixels(g, 2.0);
    for dir in [(0.0, 0.3), (-0.3, 0.2)] {
        let p = stretched_card(g, 2.0, dir, 1.0);
        let (worst, at) = worst_against(&p, reference(&twice, 2.0, (32.0, 32.0), (W, H), dir, 1.0), none);
        worst_seen = worst_seen.max(worst);
        check!(worst <= 2, "  twice as big, pulled {dir:?}: off by {worst} at {at:?}");
    }
    let p = stretched_card(g, 1.0, (0.25, 0.0), 0.6);
    let (worst, at) = worst_against(&p, reference(&content, 1.0, (16.0, 16.0), (W, H), (0.25, 0.0), 0.6), none);
    check!(worst <= 2, "  faded: off by {worst} at {at:?}");
    println!("  stretched layers: at most {} off hwui's", worst_seen.max(worst));
}

/// A layer with a render effect too is stretched after it, as hwui stretches its snapshot through its image filter:
/// the blurred layer's pixels (the blur reaching past its bounds) warped over its bounds, clamped to where the blur
/// shows.
pub fn stretch_comes_after_the_render_effect(g: &mut Gpu) {
    let blur = BlurEffect(3.0, 3.0, TileMode::Decal);
    let draw = |c: &mut Renderer, stretch: Option<StretchEffect>, effect: &RenderEffect| {
        match stretch {
            Some(s) => c.push_stretch_layer(xywh(16.0, 16.0, W, H), &s, Some(effect), 1.0),
            None => c.push_effect_layer(xywh(16.0, 16.0, W, H), effect, 1.0),
        }
        card(c, Offset::new(16.0, 16.0));
        c.pop_layer();
    };
    let (w, h) = ((W + 32.0) as i32, (H + 32.0) as i32);
    // (What the blur makes, on its own: the content the stretch warps.)
    let blurred = g.render_cleared(w, h, ClearColor::TRANSPARENT, |c| draw(c, None, &blur));
    for dir in [(0.0, 0.3), (-0.3, 0.0)] {
        let stretch = StretchEffect::new(Offset::new(dir.0, dir.1), W, H);
        let p = g.render(w, h, |c| draw(c, Some(stretch), &blur));
        // (Drawn where the blur shows: three sigmas past the bounds, whole pixels.)
        let reach = (3.0 * (0.57735 * 3.0 + 0.5f32)).ceil();
        let shows = xywh(16.0 - reach, 16.0 - reach, W + 2.0 * reach, H + 2.0 * reach);
        let want = |x: i32, y: i32| {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            if px < shows.left || py < shows.top || px > shows.right || py > shows.bottom {
                return [255; 4];
            }
            let (u, v) = hwui_sample_at((px - 16.0, py - 16.0), dir, W, H);
            over_white(sample_clamped(&blurred, u + 16.0, v + 16.0), 1.0)
        };
        let (worst, at) = worst_against(&p, want, none);
        check!(worst <= 2, "  blurred, pulled {dir:?}: off by {worst} at {at:?}");
        check!(p.at(13, 40)[1] < 250, "  {:?}", p.at(13, 40)); // (it shows past the bounds)
    }
}

/// A stretched layer whose content stays the same is kept from frame to frame, the stretch changing each frame (an
/// overscroll animating): composited from its kept pixels, warped anew, it draws as drawn afresh.
pub fn stretched_layers_are_kept_as_the_stretch_changes(g: &mut Gpu) {
    let (w, h) = (160, 120);
    let device = WatchedDevice::new(&g.device, false);
    let mut renderer = Renderer::new(Box::new(device)).expect("a renderer");
    let mut list = DisplayList::new();
    let record = |into: &mut DisplayList, pull: f32, ink: Color| {
        into.begin(Size::new(w as f32, h as f32));
        into.begin_node(1);
        into.fill_round_rect(xywh(0.0, 0.0, w as f32, h as f32), CornerRadii::Zero, rgba(0.95, 0.95, 0.97, 1.0));
        into.begin_node(2);
        let stretch = StretchEffect::new(Offset::new(0.0, pull), W, H);
        into.push_stretch_layer(xywh(20.0, 24.0, W, H), &stretch, None, 1.0);
        card(into, Offset::new(20.0, 24.0));
        into.fill_round_rect(xywh(60.0, 30.0, 10.0, 10.0), CornerRadii::Zero, ink);
        into.pop_layer();
        into.end_node();
        into.fill_round_rect(xywh(130.0, 100.0, 20.0, 10.0), CornerRadii::Zero, rgba(0.3, 0.3, 0.3, 1.0));
        into.end_node();
        into.end()
    };
    let kept = g.device.create_texture(w, h, Format::Rgba8, TARGET).expect("a texture");
    let fresh = g.device.create_texture(w, h, Format::Rgba8, TARGET).expect("a texture");
    let ink = rgba(0.5, 0.1, 0.6, 1.0);
    let black = Color::Black;
    let steps =
        [(0.0, ink), (0.02, ink), (0.08, ink), (0.15, ink), (0.15, black), (0.1, black), (0.05, black), (0.0, ink)];
    let (mut kept_layers, mut damaged) = (0, 0);
    let mut before: Option<Pixels> = None;
    for (i, &(pull, ink)) in steps.iter().enumerate() {
        renderer.device_mut().begin_frame();
        renderer.kept_layers = 0;
        let damage = record(&mut list, pull, ink);
        if let Some(d) = damage {
            damaged += 1;
            // (A stretch alone changes the layer's bounds, no more.)
            if i > 0 && steps[i - 1].1 == ink {
                check!(d.left >= 18.0 && d.top >= 22.0 && d.right <= 118.0 && d.bottom <= 90.0, "  step {i}: {d:?}");
            }
            renderer.draw_frame(kept.id(), &list, true);
        }
        kept_layers += renderer.kept_layers;
        let mut afresh = DisplayList::new();
        record(&mut afresh, pull, ink);
        renderer.draw_frame(fresh.id(), &afresh, false);
        renderer.flush();
        let read = |r: &mut Renderer, t: kompoz_gpu::device::TextureId| Pixels {
            width: w,
            height: h,
            rgba: r.device_mut().read_pixels(t, IRect::new(0, 0, w, h)),
        };
        let a = read(&mut renderer, kept.id());
        let b = read(&mut renderer, fresh.id());
        renderer.device_mut().end_frame(false);
        renderer.end_frame();
        let (worst, at) = worst_against(&a, |x, y| b.at(x, y), none);
        check!(worst <= 1, "  step {i}: off by {worst} at {at:?}");
        if let Some(p) = &before {
            check!(p != &a, "  step {i}: the same as the step before");
        }
        before = Some(a);
    }
    check!(damaged == steps.len(), "  {damaged} frames drawn");
    // (Kept in each frame but the first, those whose content changed and the one after such a change: a layer is
    // kept once what is in it stays the same.)
    check!(kept_layers == 4, "  {kept_layers} layers kept");
}

/// Stretched layers as graphics layers draw them (through draw_layer): pulled each way, cut to a rounded outline
/// around them, turned, faded, blurred first; against their picture.
pub fn stretch_scene(c: &mut Renderer) {
    let image = checker();
    let (a, b) = (rgba(0.95, 0.45, 0.1, 1.0), rgba(0.3, 0.1, 0.7, 1.0));
    let tile = |c: &mut dyn GraphicsContext, r: Rect| {
        c.fill_gradient(r, CornerRadii::Zero, &linear(&[a, b]), r);
        draw_image(c, &image, xywh(0.0, 0.0, 16.0, 16.0), xywh(r.left + 4.0, r.top + 4.0, 24.0, 24.0), 1.0);
        for i in 0..4 {
            let y = r.top + 6.0 + i as f32 * (r.height() - 12.0) / 3.0;
            c.fill_round_rect(xywh(r.left + 32.0, y - 1.0, r.width() - 38.0, 2.0), CornerRadii::Zero, Color::White);
        }
    };
    let layer = |c: &mut Renderer, bounds: Rect, dir: (f32, f32), set: &dyn Fn(&mut kompoz_ui::draw::LayerDraw)| {
        through_layer(
            c,
            bounds,
            |l| {
                l.stretch = StretchEffect::new(Offset::new(dir.0, dir.1), bounds.width(), bounds.height());
                set(l);
            },
            |c| tile(c, bounds),
        );
    };
    layer(c, xywh(8.0, 8.0, 84.0, 52.0), (0.0, 0.25), &|_| {});
    layer(c, xywh(100.0, 8.0, 84.0, 52.0), (0.0, -0.25), &|_| {});
    layer(c, xywh(8.0, 68.0, 84.0, 52.0), (0.3, 0.0), &|l| {
        l.clip = true;
        l.outline = rounded(84.0, 52.0, 16.0);
    });
    layer(c, xywh(100.0, 68.0, 84.0, 52.0), (-0.3, 0.1), &|l| l.alpha = 0.6);
    layer(c, xywh(14.0, 132.0, 72.0, 48.0), (0.0, 0.2), &|l| l.geometry.rotation_z = 12.0);
    layer(c, xywh(104.0, 132.0, 76.0, 48.0), (0.0, -0.2), &|l| {
        l.render_effect = Some(BlurEffect(2.0, 2.0, TileMode::Clamp));
    });
}

pub fn stretched_layers_match_their_picture(g: &mut Gpu) {
    let p = g.render(192, 192, stretch_scene);
    matches_golden("stretch", &p);
}
