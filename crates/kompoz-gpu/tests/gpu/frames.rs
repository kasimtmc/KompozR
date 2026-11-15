//! What text and frames come to (the C++ Kompoz's checks of them): everything drawn through a transform is what is
//! drawn without it, moved, turned or enlarged; each kind of primitive draws the same pixels alone as among the
//! others; a kept frame that only what changed is drawn into (its scrolled content moved, not drawn again) is the
//! frame drawn afresh; a kept frame blitted onto a window's image is its texels, turned as the display is.

use crate::exact::{none, worst_against};
use crate::fonts::{Font, text_at};
use crate::harness::{Gpu, Pixels, WatchedDevice, near};
use crate::scenes::{about, fill, linear, mixed, rgba, shadow, star, xywh};
use kompoz_gpu::Renderer;
use kompoz_gpu::device::{ClearColor, Device, Format, IRect, TextureUsage};
use kompoz_graphics::{
    BlendMode, BlurEffect, Brush, Color, ColorFilter, CornerRadius, Matrix, Offset, Outline, Path, Rect, RoundRect,
    Size, TileMode,
};
use kompoz_ui::DisplayList;
use kompoz_ui::graphics_context::{ClipOp, CornerRadii, ElevationShadow, GraphicsContext, LayerTransform};

const TARGET: TextureUsage = TextureUsage::RENDER_TARGET.union(TextureUsage::SAMPLED);

/// Everything drawn through a transform is the same as without it, moved: turned a quarter about the middle, each
/// pixel is where the turn takes it; mirrored, where the mirror does; enlarged twice, it is what 2 px per dp draws
/// (shadows aside: their blur grows with the transform, as in Skia).
pub fn transforms_carry_everything(g: &mut Gpu) {
    let n = 96;
    let plain = g.render(n, n, |c| mixed(c, 1.0, true));
    let mut through = |m: Matrix, size: i32, k: f32, shadow: bool| {
        g.render(size, size, |c| {
            c.push_transform(&m);
            mixed(c, k, shadow);
            c.pop_transform();
        })
    };
    let turned = through(about(48.0, 48.0, |m| m.rotate_z(90.0)), n, 1.0, true);
    let (worst, at) = worst_against(&plain, |x, y| turned.at(n - 1 - y, x), none);
    check!(worst <= 3, "  turned a quarter: off by {worst} at {at:?}");
    let mirrored = through(about(48.0, 48.0, |m| m.scale(-1.0, 1.0, 1.0)), n, 1.0, true);
    let (worst, at) = worst_against(&plain, |x, y| mirrored.at(n - 1 - x, y), none);
    check!(worst <= 3, "  mirrored: off by {worst} at {at:?}");
    let mut twice = Matrix::new();
    twice.scale(2.0, 2.0, 1.0);
    let enlarged = through(twice, 2 * n, 1.0, false);
    // At 2 px per dp, inside translations: the same (the matrix is in dp, the translations around it and inside
    // it).
    let plain2 = g.render(2 * n, 2 * n, |c| mixed(c, 2.0, true));
    let turned2 = g.render(2 * n, 2 * n, |c| {
        c.push_translation(Offset::new(-20.0, 0.0));
        // (About (48, 48) dp once the two moves are undone.)
        c.push_transform(&about(116.0, 96.0, |m| m.rotate_z(90.0)));
        c.push_translation(Offset::new(20.0, 0.0));
        mixed(c, 2.0, true);
        c.pop_translation();
        c.pop_transform();
        c.pop_translation();
    });
    let (worst, at) = worst_against(&plain2, |x, y| turned2.at(2 * n - 1 - y, x), none);
    check!(worst <= 3, "  turned a quarter at 2 px per dp: off by {worst} at {at:?}");
    let dense = g.render(2 * n, 2 * n, |c| mixed(c, 2.0, false));
    let (worst, at) = worst_against(&dense, |x, y| enlarged.at(x, y), none);
    check!(worst <= 3, "  enlarged twice: off by {worst} at {at:?}");
}

/// Each kind of primitive draws the same pixels whatever it shares its batch with (the variant of the shader made
/// for the batch's kinds): alone, and among one of every other kind, each in a cell of its own, at a phone's 2.75 px
/// a dp.
pub fn primitives_draw_alone_as_among_others(g: &mut Gpu) {
    const K: f32 = 2.75;
    let (cell, columns) = (40, 5); // (dp)
    let dp = |v: f32| v * K;
    let r = |x: f32, y: f32, w: f32, h: f32| xywh(dp(x), dp(y), dp(w), dp(h));
    let o = |x: f32, y: f32| Offset::new(dp(x), dp(y));
    let all_of = |v: f32| CornerRadii::all(dp(v));
    let rounded = |w: f32, h: f32, radius: f32| {
        Outline::Rounded(RoundRect::from_rect_radius(r(0.0, 0.0, w, h), CornerRadius::circular(dp(radius))))
    };
    let (blue, black) = (rgba(0.2, 0.4, 0.9, 1.0), rgba(0.0, 0.0, 0.0, 1.0));
    let star_path = star(dp(14.0), dp(14.0), dp(13.0));
    let raised =
        |z: f32| ElevationShadow { plane: [0.0, 0.0, dp(z)], ambient_color: black, spot_color: black, alpha: 1.0 };
    type Draw<'a> = Box<dyn Fn(&mut Renderer) + 'a>;
    let kinds: Vec<(&str, Draw)> = vec![
        ("a rounded rect", Box::new(|c| c.fill_round_rect(r(6.0, 6.0, 26.0, 20.0), all_of(6.0), blue))),
        ("a stroke", Box::new(|c| c.stroke_round_rect(r(6.0, 6.0, 26.0, 20.0), all_of(6.0), dp(2.3), blue))),
        (
            "a gradient",
            Box::new(|c| {
                let area = r(6.0, 6.0, 26.0, 20.0);
                c.fill_gradient(area, all_of(4.0), &linear(&[blue, rgba(0.9, 0.3, 0.2, 1.0)]), area);
            }),
        ),
        ("text", Box::new(|c| c.draw_text(&text_at("Kq", Offset::new(8.0, 24.0), Font::new(14.0), K), black))),
        (
            "a shadow",
            Box::new(|c| {
                let s = shadow(rgba(0.0, 0.0, 0.0, 0.5), o(1.0, 2.0), dp(4.0), 0.0);
                c.draw_shadow(r(8.0, 8.0, 22.0, 16.0), all_of(5.0), &s, false);
            }),
        ),
        (
            "an elevation shadow",
            Box::new(|c| c.draw_elevation_shadow(&rounded(22.0, 16.0, 6.0), o(8.0, 8.0), &raised(6.0))),
        ),
        (
            "a star's elevation shadow",
            Box::new(|c| {
                let s = ElevationShadow { plane: [0.0, 0.0, dp(4.0)], ..ElevationShadow::default() };
                c.draw_elevation_shadow(&Outline::Generic(star_path.clone()), o(6.0, 6.0), &s);
            }),
        ),
        ("an arc", Box::new(|c| c.draw_arc(r(8.0, 6.0, 22.0, 22.0), 30.0, 250.0, dp(3.0), blue, true))),
        ("a line", Box::new(|c| c.draw_line(o(6.0, 8.0), o(32.0, 27.0), dp(1.7), black))),
        ("a path", Box::new(|c| c.draw_path(&star_path, o(6.0, 4.0), &Brush::from(blue), Rect::Zero, &fill()))),
        (
            "a faded rounded layer",
            Box::new(|c| {
                c.push_layer(r(6.0, 6.0, 26.0, 22.0), all_of(7.0), 0.6, LayerTransform::Identity);
                c.fill_round_rect(r(0.0, 0.0, 40.0, 40.0), CornerRadii::Zero, blue);
                c.pop_layer();
            }),
        ),
        (
            "a turned layer",
            Box::new(|c| {
                let t = LayerTransform { scale_x: 0.8, scale_y: 0.8, rotation_z: 12.0, ..LayerTransform::Identity };
                c.push_layer(r(8.0, 8.0, 22.0, 18.0), CornerRadii::Zero, 1.0, t);
                c.fill_round_rect(r(8.0, 8.0, 22.0, 18.0), all_of(4.0), blue);
                c.pop_layer();
            }),
        ),
        (
            "a round clip",
            Box::new(|c| {
                c.push_clip_round_rect(r(6.0, 6.0, 26.0, 22.0), all_of(9.0), ClipOp::Intersect);
                c.fill_round_rect(r(0.0, 0.0, 40.0, 40.0), CornerRadii::Zero, blue);
                c.pop_clip();
            }),
        ),
        (
            "a tinted icon", // (a color filter's)
            Box::new(|c| {
                c.push_color_filter(&ColorFilter::tint(rgba(0.1, 0.6, 0.3, 1.0), BlendMode::SrcIn));
                c.draw_path(&star_path, o(6.0, 4.0), &Brush::from(black), Rect::Zero, &fill());
                c.pop_color_filter();
            }),
        ),
        (
            "a shadowed button in a growing box", // (a FAB coming in from its corner)
            Box::new(|c| {
                c.push_layer(r(20.3, 17.6, 13.4, 11.2), CornerRadii::Zero, 0.9, LayerTransform::Identity);
                c.draw_elevation_shadow(&rounded(26.0, 22.0, 7.0), o(7.0, 6.0), &raised(6.0));
                c.fill_round_rect(r(7.0, 6.0, 26.0, 22.0), all_of(7.0), rgba(0.0, 0.25, 0.25, 1.0));
                c.pop_layer();
            }),
        ),
    ];
    let n = kinds.len() as i32;
    let rows = (n + columns - 1) / columns;
    let (w, h) = (((columns * cell) as f32 * K) as i32, ((rows * cell) as f32 * K) as i32);
    let at = |c: &mut Renderer, i: i32, draw: &Draw| {
        c.push_translation(o((i % columns * cell) as f32, (i / columns * cell) as f32));
        draw(c);
        c.pop_translation();
    };
    let all = g.render(w, h, |c| {
        for (i, (_, draw)) in kinds.iter().enumerate() {
            at(c, i as i32, draw);
        }
    });
    for (i, (name, draw)) in kinds.iter().enumerate() {
        let i = i as i32;
        let alone = g.render(w, h, |c| at(c, i, draw));
        let (x0, y0) = (((i % columns * cell) as f32 * K) as i32, ((i / columns * cell) as f32 * K) as i32);
        let size = (cell as f32 * K) as i32;
        let (worst, at) =
            worst_against(&alone, |x, y| all.at(x, y), |x, y| x < x0 || y < y0 || x >= x0 + size || y >= y0 + size);
        // (Within the pictures' tolerance: a driver may round two variants' float math apart.)
        check!(worst <= 2, "  {name}: off by {worst} at {:?} among the others", (at.0 - x0, at.1 - y0));
    }
}

/// A frame kept from the last (its display list's moves made, its damaged areas drawn again) is the frame drawn
/// afresh, frame after frame, as a list scrolls by whole pixels and by parts of one, across, and as an item in it
/// changes: at 2 px per dp, with text, shadows, a path's shadow and a backdrop over what scrolls.
pub fn kept_frames_move_what_scrolled(g: &mut Gpu) {
    let (w, h) = (240, 320);
    let k = 2.0f32; // (120 x 160 dp)
    let dp = |v: f32| v * k;
    let r = |x: f32, y: f32, w: f32, h: f32| xywh(dp(x), dp(y), dp(w), dp(h));
    // (The whole shader: what is drawn again shaded as all of it is.)
    let mut renderer = Renderer::new(Box::new(WatchedDevice::new(&g.device, true))).expect("a renderer");
    let mut list = DisplayList::new();
    list.set_allow_moves(true);
    let (screen, scroller, item, fab) = (1u64, 2u64, 100u64, 3u64);
    let mut mark = Path::new();
    mark.move_to(0.0, 0.0);
    mark.line_to(dp(10.0), dp(5.0));
    mark.line_to(0.0, dp(10.0));
    mark.close();
    let record = |list: &mut DisplayList, scroll: f32, across: f32, changed: i32| {
        list.begin(Size::new(w as f32, h as f32));
        list.begin_node(screen);
        list.fill_round_rect(r(0.0, 0.0, 120.0, 160.0), CornerRadii::Zero, rgba(0.95, 0.95, 0.97, 1.0));
        list.draw_text(&text_at("Başlık", Offset::new(6.0, 14.0), Font::new(12.0), k), Color::Black);
        list.begin_node(scroller);
        list.push_clip(r(0.0, 20.0, 120.0, 140.0));
        for i in 0..20 {
            list.begin_node(item + i as u64);
            let (x, y) = (6.0 - across, 22.0 + i as f32 * 30.0 - scroll);
            let s = shadow(rgba(0.0, 0.0, 0.0, 0.3), Offset::new(0.0, dp(2.0)), dp(4.0), 0.0);
            list.draw_shadow(r(x, y, 108.0, 26.0), CornerRadii::all(dp(6.0)), &s, false);
            let color =
                if i == changed { rgba(1.0, 0.6, 0.2, 1.0) } else { rgba(0.3, 0.5, 0.9 - i as f32 * 0.03, 1.0) };
            list.fill_round_rect(r(x, y, 108.0, 26.0), CornerRadii::all(dp(6.0)), color);
            let label = format!("Öğe {i}");
            list.draw_text(&text_at(&label, Offset::new(x + 8.0, y + 17.0), Font::new(11.0), k), Color::White);
            let s = shadow(rgba(0.0, 0.0, 0.0, 0.5), Offset::new(dp(1.0), dp(1.0)), dp(1.0), 0.0);
            list.draw_shadow_path(&mark, Offset::new(dp(x + 90.0), dp(y + 8.0)), &s, false);
            list.end_node();
        }
        list.pop_clip();
        // (Frosted: what moved under it, blurred anew.)
        list.draw_backdrop(r(10.0, 100.0, 60.0, 20.0), &BlurEffect(dp(3.0), dp(3.0), TileMode::Clamp), 1.0);
        list.end_node();
        list.begin_node(fab);
        list.fill_round_rect(r(84.0, 124.0, 28.0, 28.0), CornerRadii::all(dp(14.0)), rgba(0.9, 0.2, 0.4, 1.0));
        list.end_node();
        list.end_node();
        list.end()
    };
    let kept = g.device.create_texture(w, h, Format::Rgba8, TARGET).expect("a texture");
    let fresh = g.device.create_texture(w, h, Format::Rgba8, TARGET).expect("a texture");
    let steps: [(f32, f32, i32); 16] = [
        (0.0, 0.0, -1),
        (10.0, 0.0, -1),
        (25.0, 0.0, -1),
        (25.0, 0.0, -1),
        (24.5, 0.0, -1),
        (40.0, 0.0, 3),
        (30.0, 0.0, 3),
        (31.5, 0.0, -1),
        (100.0, 0.0, 5),
        (99.5, 0.0, 5),
        (41.25, 0.0, -1),
        (60.0, 0.0, -1),
        (0.0, 0.0, -1),
        (7.0, 0.0, -1),
        (7.0, 6.0, -1),
        (7.0, -3.5, -1),
    ];
    let mut moves = 0;
    for (scroll, across, changed) in steps {
        renderer.device_mut().begin_frame();
        let damage = record(&mut list, scroll, across, changed);
        if damage.is_some() {
            renderer.draw_frame(kept.id(), &list, true);
        }
        renderer.draw_frame(fresh.id(), &list, false);
        renderer.flush();
        let a = Pixels { width: w, height: h, rgba: renderer.device_mut().read_pixels(kept.id(), kept.id().bounds()) };
        let b =
            Pixels { width: w, height: h, rgba: renderer.device_mut().read_pixels(fresh.id(), fresh.id().bounds()) };
        renderer.device_mut().end_frame(false);
        renderer.end_frame();
        moves += list.moves().len();
        // (Within a 255th: drawn somewhere else, some drivers round an edge's coverage apart, as lavapipe and WARP
        // do.)
        let (worst, at) = worst_against(&a, |x, y| b.at(x, y), none);
        check!(worst <= 1, "  scrolled to {scroll}, {across}: off by {worst} at {at:?}");
    }
    // (All but the first, the one that changed nothing, those by part of a pixel, and those that leave as much to
    // draw as all of the clip: jumps far, or across past the backdrop.)
    check!(moves == 9, "  {moves} moves");
}

/// A texture blitted is its texels, all of it or some regions of it, turned by quarters and flipped into a target
/// the display's way up (Device::swapchain_turns): each pixel the texel turned onto it, the rest of the target as
/// it was.
pub fn blit_copies_textures_as_they_are(g: &mut Gpu) {
    let (w, h) = (40, 30);
    let frame = g.device.create_texture(w, h, Format::Rgba8, TARGET).expect("a texture");
    let id = frame.id();
    let (red, blue) = (rgba(1.0, 0.0, 0.0, 1.0), rgba(0.0, 0.0, 1.0, 1.0));
    let shown = g.render(w, h, |r| {
        r.begin_target(id, Some(ClearColor::new(0.0, 1.0, 0.0, 1.0)));
        r.fill_round_rect(xywh(4.0, 4.0, 10.0, 8.0), CornerRadii::Zero, red);
        r.fill_round_rect(xywh(20.0, 12.0, 14.0, 12.0), CornerRadii::all(3.0), blue);
        r.end_target();
        r.blit(id, 0, false, None);
    });
    g.device.begin_frame();
    let kept = Pixels { width: w, height: h, rgba: g.device.read_pixels(id, IRect::new(0, 0, w, h)) };
    g.device.end_frame(false);
    let (worst, at) = worst_against(&shown, |x, y| kept.at(x, y), none);
    check!(
        worst == 0
            && near(shown.at(8, 7), 255, 0, 0, 2)
            && near(shown.at(26, 18), 0, 0, 255, 2)
            && near(shown.at(1, 28), 0, 255, 0, 2),
        "  blit: off by {worst} at {at:?}"
    );
    for turns in 1..4 {
        let (tw, th) = if turns & 1 != 0 { (h, w) } else { (w, h) };
        let turned = g.render(tw, th, |r| r.blit(id, turns, false, None));
        let (worst, at) = worst_against(
            &turned,
            |x, y| match turns {
                1 => kept.at(y, h - 1 - x),
                2 => kept.at(w - 1 - x, h - 1 - y),
                _ => kept.at(w - 1 - y, x),
            },
            none,
        );
        check!(worst == 0, "  blit turned {turns} times: off by {worst} at {at:?}");
    }
    // Only some regions of it (what changed since a swapchain image was last presented), turned and flipped as all
    // of it is: those texels where they go, the rest of the target as it was.
    let regions = [IRect::new(2, 3, 10, 6), IRect::new(25, 10, 8, 9)];
    let inside =
        |x: i32, y: i32| regions.iter().any(|r| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height);
    for turns in 0..4 {
        for flipped in [false, true] {
            let (tw, th) = if turns & 1 != 0 { (h, w) } else { (w, h) };
            let part = g.render(tw, th, |r| r.blit(id, turns, flipped, Some(&regions)));
            let (worst, at) = worst_against(
                &part,
                |x, y| {
                    let y = if flipped { th - 1 - y } else { y };
                    let (sx, sy) = match turns {
                        0 => (x, y),
                        1 => (y, h - 1 - x),
                        2 => (w - 1 - x, h - 1 - y),
                        _ => (w - 1 - y, x),
                    };
                    if inside(sx, sy) { kept.at(sx, sy) } else { [255; 4] }
                },
                none,
            );
            check!(worst == 0, "  blit of regions turned {turns} times, flipped {flipped}: off by {worst} at {at:?}");
        }
    }
}

/// Text's glyph atlas: a page cleared between frames (more pages than are kept, the others unused) is uploaded
/// again whole before it is drawn from, so text drawn after it is as text drawn by a renderer that had drawn
/// nothing; color glyphs aside, text in a brush is the brush where the glyphs cover; a text shadow is under the
/// text, blurred.
pub fn text_is_drawn_from_the_atlas(g: &mut Gpu) {
    let draw = |c: &mut Renderer| {
        c.draw_text(&text_at("Kompoz ağaç", Offset::new(4.0, 30.0), Font::new(20.0), 1.0), Color::Black);
    };
    let first = g.render(160, 40, draw);
    let mut renderer = Renderer::new(Box::new(g.device.clone())).expect("a renderer");
    let before = crate::harness::render_by(&mut renderer, 160, 40, draw, ClearColor::WHITE);
    check!(before.rgba == first.rgba, "  off by {}", worst_against(&before, |x, y| first.at(x, y), none).0);
    // Many sizes of many glyphs, a frame each: pages fill and the oldest (the text's first) are cleared for reuse.
    let alphabet: String = ('A'..='Z').chain('a'..='z').chain('0'..='9').collect();
    for size in 40..140 {
        crate::harness::render_by(
            &mut renderer,
            32,
            32,
            |c| c.draw_text(&text_at(&alphabet, Offset::new(0.0, 30.0), Font::new(size as f32), 1.0), Color::Black),
            ClearColor::WHITE,
        );
    }
    let again = crate::harness::render_by(&mut renderer, 160, 40, draw, ClearColor::WHITE);
    check!(again.rgba == first.rgba, "  off by {}", worst_against(&again, |x, y| first.at(x, y), none).0);
    // A brush over text: where a glyph covers a pixel fully, the brush's color there.
    let brushed = g.render(160, 40, |c| {
        let t = text_at("HHHH", Offset::new(4.0, 34.0), Font::bold(36.0), 1.0);
        c.draw_text_brush(&t, &Brush::from(rgba(0.0, 0.0, 1.0, 1.0)), xywh(0.0, 0.0, 160.0, 40.0));
    });
    let solid = (0..40).flat_map(|y| (0..160).map(move |x| (x, y))).filter(|&(x, y)| brushed.at(x, y)[0] == 0).count();
    check!(solid > 100, "  {solid} pixels of the brush");
    // A shadow below and right of its text, blurred: past the text's ink, some shade.
    let shadowed = g.render(160, 40, |c| {
        let t = text_at("HH", Offset::new(4.0, 30.0), Font::new(24.0), 1.0);
        c.draw_text_shadow(&t.at(Offset::new(8.0, 34.0)), rgba(0.0, 0.0, 0.0, 1.0), 3.0);
    });
    let shade = |x: i32, y: i32| 255 - shadowed.at(x, y)[0] as i32;
    check!((0..160).all(|x| shade(x, 0) == 0), "  shade at the top");
    check!((0..40).map(|y| shade(40, y)).max().unwrap_or(0) > 0, "  no shade");
}
