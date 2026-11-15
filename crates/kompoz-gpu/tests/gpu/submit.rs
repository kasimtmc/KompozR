//! How what is recorded is submitted (the C++ Kompoz's checks of it): a device that can't make the shader's variants
//! is drawn on with the whole shader, the same pictures; a scene drawn twice in a frame, its rows of the frame's
//! table elsewhere the second time, is the same; a frame's paths share coverage masks, generation by generation, and
//! frames after start them over; a path drawn again whole pixels on keeps its coverage.

use crate::exact::{none, worst_against};
use crate::harness::{Gpu, Pixels, WatchedDevice, max_difference, render_by};
use crate::scenes::{blend_modes, effects, fill, lines, oval, paths, rects, rgba, shadows, star, text_scene, xywh};
use kompoz_gpu::Renderer;
use kompoz_gpu::device::{ClearColor, Device, Format, IRect, TextureUsage};
use kompoz_graphics::{
    BlendMode, Brush, Color, ColorFilter, CornerRadius, Offset, Outline, Path, PathFillType, Rect, RoundRect,
};
use kompoz_ui::graphics_context::{ClipOp, CornerRadii, ElevationShadow, GraphicsContext, LayerTransform};

const TARGET: TextureUsage = TextureUsage::RENDER_TARGET.union(TextureUsage::SAMPLED);

/// The device can't make the shader's variants: the renderer draws with the whole shader instead, the same pictures.
pub fn variants_fall_back_to_the_whole_shader(g: &mut Gpu) {
    let device = WatchedDevice::new(&g.device, true);
    let refused = device.refused.clone();
    let mut renderer = Renderer::new(Box::new(device)).expect("a renderer");
    // (C++'s rects, text, blend modes and effects, and more.)
    let scenes: [fn(&mut Renderer); 7] = [rects, text_scene, blend_modes, effects, paths, lines, shadows];
    for (i, draw) in scenes.into_iter().enumerate() {
        let expected = g.render(192, 192, draw);
        let actual = render_by(&mut renderer, 192, 192, draw, ClearColor::WHITE);
        // (Within the pictures' tolerance: a driver may round the two shaders' float math apart.)
        let (worst, at) = worst_against(&actual, |x, y| expected.at(x, y), none);
        check!(worst <= 2, "  scene {i}, the whole shader: off by {worst} at {at:?}");
    }
    check!(refused.get() > 0);
}

/// The pipelines a renderer made are listed in the file it is given, and a renderer warming them from that list
/// (on a thread of its own) draws as the first did, listing none again.
pub fn pipelines_are_warmed_from_their_list(g: &mut Gpu) {
    let list = std::env::temp_dir().join(format!("kompoz_gpu_tests_pipeline_list_{}", std::process::id()));
    let _ = std::fs::remove_file(&list);
    let scenes: [fn(&mut Renderer); 3] = [rects, blend_modes, paths];
    let pictures = |renderer: &mut Renderer| -> Vec<Pixels> {
        scenes.iter().map(|&draw| render_by(renderer, 192, 192, draw, ClearColor::WHITE)).collect()
    };
    let mut first = Renderer::new(Box::new(g.device.clone())).expect("a renderer");
    first.warm_pipelines(&list); // (none listed yet: listed as they are made)
    let drawn = pictures(&mut first);
    drop(first);
    let listed = std::fs::read_to_string(&list).unwrap_or_default();
    let lines = listed.lines().count();
    check!(lines > 0 && listed.lines().all(|l| l.split_whitespace().count() == 15), "  {listed:?}");
    let mut warmed = Renderer::new(Box::new(g.device.clone())).expect("a renderer");
    warmed.warm_pipelines(&list);
    check!(pictures(&mut warmed) == drawn);
    drop(warmed);
    check!(std::fs::read_to_string(&list).unwrap_or_default().lines().count() == lines);
    let _ = std::fs::remove_file(&list);
}

/// The same drawn twice in a frame (a kept frame's area and all of it, say) into two targets, its clips and color
/// filters in other rows of the frame's table the second time (many more before them): the same pixels. (An Adreno
/// 618 sampling the table read other rows than its coordinates said, the further down the further off: its texels
/// are fetched.)
pub fn drawn_twice_in_a_frame_alike(g: &mut Gpu) {
    let k = 2.75f32;
    let (w, h) = ((120.0 * k) as i32, (100.0 * k) as i32);
    let black = rgba(0.0, 0.0, 0.0, 1.0);
    let star_path = star(8.0 * k, 8.0 * k, 7.0 * k);
    let dp = |x: f32, y: f32, width: f32, height: f32| xywh(x * k, y * k, width * k, height * k);
    let scene = |c: &mut Renderer, grow: f32| {
        c.fill_round_rect(dp(0.0, 0.0, 120.0, 100.0), CornerRadii::Zero, rgba(0.11, 0.12, 0.12, 1.0));
        c.fill_round_rect(dp(8.0, 50.0, 104.0, 40.0), CornerRadii::all(12.0 * k), rgba(0.2, 0.25, 0.5, 1.0));
        // A FAB coming in from its corner: a box growing over it, its button cut to its shape.
        let s = 26.0 * grow;
        c.push_layer(dp(90.0 - s, 74.0 - s, s, s), CornerRadii::Zero, 1.0, LayerTransform::Identity);
        c.push_clip(dp(90.0 - s, 74.0 - s, s, s));
        let button = RoundRect::from_rect_radius(dp(0.0, 0.0, 26.0, 26.0), CornerRadius::circular(7.0 * k));
        let elevation =
            ElevationShadow { plane: [0.0, 0.0, 6.0 * k], ambient_color: black, spot_color: black, alpha: 1.0 };
        c.draw_elevation_shadow(&Outline::Rounded(button), Offset::new(64.0 * k, 48.0 * k), &elevation);
        c.push_clip_round_rect(dp(64.0, 48.0, 26.0, 26.0), CornerRadii::all(7.0 * k), ClipOp::Intersect);
        c.fill_round_rect(dp(64.0, 48.0, 26.0, 26.0), CornerRadii::Zero, rgba(0.0, 0.25, 0.25, 1.0));
        c.push_color_filter(&ColorFilter::tint(rgba(0.6, 0.9, 0.9, 1.0), BlendMode::SrcIn));
        c.draw_path(&star_path, Offset::new(69.0 * k, 53.0 * k), &Brush::from(black), Rect::Zero, &fill());
        c.pop_color_filter();
        c.pop_clip();
        c.pop_clip();
        c.pop_layer();
    };
    let y0 = (40.0 * k) as i32 + 1; // (what is compared: below the corner the second one fills the table from)
    for grow in [0.1f32, 0.35, 0.6, 1.0] {
        g.device.begin_frame();
        let a = g.device.create_texture(w, h, Format::Rgba8, TARGET).expect("a texture");
        let b = g.device.create_texture(w, h, Format::Rgba8, TARGET).expect("a texture");
        let r = &mut g.renderer;
        r.begin_target(a.id(), Some(ClearColor::WHITE));
        r.push_clip(dp(0.0, 40.0, 120.0, 60.0)); // (an area of a kept frame: whole pixels)
        scene(r, grow);
        r.pop_clip();
        r.end_target();
        r.begin_target(b.id(), Some(ClearColor::WHITE));
        for i in 0..60 {
            r.push_clip_round_rect(dp(1.0, 1.0, 5.0, 5.0), CornerRadii::all(2.0 * k), ClipOp::Intersect);
            r.push_color_filter(&ColorFilter::tint(rgba(0.5, 0.5, i as f32 / 60.0, 1.0), BlendMode::SrcIn));
            r.fill_round_rect(dp(0.0, 0.0, 8.0, 8.0), CornerRadii::Zero, black);
            r.pop_color_filter();
            r.pop_clip();
        }
        scene(r, grow);
        r.end_target();
        r.flush();
        let pa = Pixels { width: w, height: h, rgba: g.device.read_pixels(a.id(), IRect::new(0, 0, w, h)) };
        let pb = Pixels { width: w, height: h, rgba: g.device.read_pixels(b.id(), IRect::new(0, 0, w, h)) };
        g.device.end_frame(false);
        g.renderer.end_frame();
        let (worst, at) = worst_against(&pa, |x, y| pb.at(x, y), |_, y| y < y0);
        check!(worst <= 1, "  grown {grow}: off by {worst} at {at:?}");
    }
}

/// A frame's paths all go into one coverage mask, drawn once before the passes; past what a mask holds, into more
/// generations, each a mask of its own: the same picture as frames that each held fewer. Frames after it start the
/// masks over (a mask a generation, not one more a frame), and a path's mask is cleared a texel around it too (a
/// layer's cut, filtered, reads that far: what the frame before left there doesn't show).
pub fn paths_share_their_masks(g: &mut Gpu) {
    let (size, count) = (512, 24); // (a region of about 512 px: nine to a mask)
    let triangle = |i: i32| {
        let mut p = Path::new();
        p.move_to((i * size / count) as f32, 0.0);
        p.line_to(size as f32, size as f32);
        p.line_to(0.0, (size - i * 10) as f32);
        p.close();
        p
    };
    let color = |i: i32| rgba((i % 5) as f32 / 4.0, (i % 3) as f32 / 2.0, (i % 7) as f32 / 6.0, 1.0);
    let paint = |c: &mut Renderer, from: i32, to: i32| {
        for i in from..to {
            c.draw_path(&triangle(i), Offset::Zero, &Brush::from(color(i)), Rect::Zero, &fill());
        }
    };
    let device = WatchedDevice::new(&g.device, false);
    let masks = device.masks.clone();
    let mut renderer = Renderer::new(Box::new(device)).expect("a renderer");
    let all = render_by(&mut renderer, size, size, |c| paint(c, 0, count), ClearColor::WHITE);
    check!(masks.get() >= 3, "  {} masks", masks.get());
    // The same, eight a frame, into one texture.
    let kept = g.device.create_texture(size, size, Format::Rgba8, TARGET).expect("a texture");
    for from in (0..count).step_by(8) {
        renderer.device_mut().begin_frame();
        renderer.begin_target(kept.id(), (from == 0).then_some(ClearColor::WHITE));
        paint(&mut renderer, from, from + 8);
        renderer.end_target();
        renderer.flush();
        renderer.device_mut().end_frame(false);
        renderer.end_frame();
    }
    renderer.device_mut().begin_frame();
    let in_frames =
        Pixels { width: size, height: size, rgba: renderer.device_mut().read_pixels(kept.id(), kept.id().bounds()) };
    renderer.device_mut().end_frame(false);
    // (Within a 255th: frames apart, a variant of the shader may round apart.)
    let (worst, at) = worst_against(&all, |x, y| in_frames.at(x, y), none);
    check!(worst <= 1, "  in frames: off by {worst} at {at:?}");
    // Frames of one path each: the masks made already do.
    let made = masks.get();
    for i in 0..6 {
        render_by(&mut renderer, 64, 64, |c| paint(c, i, i + 1), ClearColor::WHITE);
    }
    check!(masks.get() == made, "  {} masks, {made} before", masks.get());
    // A turned layer cut by a small path (its mask sampled between texels, so past its edge), after a frame whose
    // path covered all of the mask's corner: as a renderer that had drawn nothing before draws it.
    let cut = |c: &mut Renderer| {
        let turned = LayerTransform { rotation_z: 30.0, ..LayerTransform::Identity };
        c.push_outline_layer(xywh(12.0, 12.0, 40.0, 40.0), &star(20.0, 20.0, 19.0), 1.0, turned);
        c.fill_round_rect(xywh(0.0, 0.0, 64.0, 64.0), CornerRadii::Zero, rgba(0.8, 0.2, 0.2, 1.0));
        c.pop_layer();
    };
    render_by(
        &mut renderer,
        size,
        size,
        |c| {
            let mut whole = Path::new();
            whole.add_rect(xywh(0.0, 0.0, size as f32, size as f32), Default::default());
            c.draw_path(&whole, Offset::Zero, &Brush::from(Color::Black), Rect::Zero, &fill());
        },
        ClearColor::WHITE,
    );
    let after = render_by(&mut renderer, 64, 64, cut, ClearColor::WHITE);
    let mut fresh = Renderer::new(Box::new(g.device.clone())).expect("a renderer");
    let first = render_by(&mut fresh, 64, 64, cut, ClearColor::WHITE);
    check!(after.rgba == first.rgba, "  off by {}", worst_against(&after, |x, y| first.at(x, y), none).0);
}

/// A path drawn again at whole pixels from where it was takes the coverage it had (kept: the same pixels, moved);
/// a fraction of a pixel on, it is rasterized anew. More than the kept mask holds (sixteen of these): those past it
/// drawn as any path, the frame's paths kept as long as they fill most of it; started over (after a frame) for
/// others.
pub fn a_path_keeps_its_coverage(g: &mut Gpu) {
    let mut s = star(20.0, 20.0, 16.0);
    s.fill_type = PathFillType::EvenOdd;
    let mut draw = |at: Offset| {
        g.renderer.path_count = 0;
        g.renderer.kept_paths = 0;
        let out = g.render(60, 60, |c| {
            c.draw_path(&s, at, &Brush::from(rgba(0.1, 0.55, 0.3, 1.0)), Rect::Zero, &fill());
        });
        (out, g.renderer.kept_paths)
    };
    let (first, kept0) = draw(Offset::new(3.3, 2.6));
    let (moved, kept1) = draw(Offset::new(10.3, 7.6));
    let (shifted, kept2) = draw(Offset::new(10.55, 7.6));
    check!(kept0 == 0 && kept1 == 1 && kept2 == 0, "  kept {kept0}, {kept1}, {kept2}");
    let (mut worst, mut differ) = (0, 0);
    for y in 0..50 {
        for x in 0..50 {
            worst = worst.max(max_difference(first.at(x, y), moved.at(x + 7, y + 5)));
            differ += (max_difference(moved.at(x + 7, y + 5), shifted.at(x + 7, y + 5)) > 8) as i32;
        }
    }
    check!(worst == 0 && differ > 0, "  off by {worst}, {differ} pixels differ");
    let mut kept = Vec::new();
    let mut frames = Vec::new();
    for frame in 0..6 {
        g.renderer.kept_paths = 0;
        frames.push(g.render(510, 510, |c| {
            for i in 0..20 {
                let w = 500.0 - i as f32 - if frame >= 3 { 0.5 } else { 0.0 };
                let p = oval(xywh(4.0, 4.0, w, 500.0));
                c.draw_path(&p, Offset::Zero, &Brush::from(rgba(0.1, 0.55, 0.3, 0.05)), Rect::Zero, &fill());
            }
        }));
        kept.push(g.renderer.kept_paths);
    }
    // (Of the first ones, as many as there was room for beside the earlier tests' paths.)
    check!(
        kept[0] == 0 && kept[1] > 0 && kept[2] == kept[1] && kept[3] == 0 && kept[4] == 0 && kept[5] == 16,
        "  kept {kept:?}"
    );
    let mut worst = 0;
    for y in (0..510).step_by(3) {
        for x in (0..510).step_by(3) {
            worst = worst
                .max(max_difference(frames[0].at(x, y), frames[2].at(x, y)))
                .max(max_difference(frames[3].at(x, y), frames[5].at(x, y)));
        }
    }
    check!(worst <= 1, "  off by {worst}"); // (those kept at a 64th of a pixel from where they were first)
}
