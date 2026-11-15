//! The device's tests: the C++ Kompoz's (texturesReadBackAsUploaded, readbacksLeaveTheFrameGoing, gpuTimesComeBack,
//! passAreasClearOnlyThere, discardedTexturesAreDrawnOver, pipelinesAreKeptBetweenRuns), drawn with the test
//! pipelines where those drew with the renderer, and what else the renderer relies on: scissors, pipelines'
//! blending, constants and targets, pipelines made on other threads, mipmaps, copies, frame memory beyond a chunk,
//! textures made and let go of over frames (the allocator's blocks, dedicated memories and deferred destruction).
//! Then the golden comparison itself, against the pictures there are.

use crate::golden::{self, Difference, Outcome};
use crate::harness::{
    COLOR_FRAGMENT, Frame, Gpu, Pixels, Quad, blit, draw_quads, fill, near, quad_desc, quad_pipeline,
};
use kompoz_gpu::device::{
    BlendFactor, BlendOp, BlendState, ClearColor, Device, Filter, Format, IRect, Sampler, TextureUsage, fnv1a,
    read_pipeline_cache, write_pipeline_cache,
};
use std::path::PathBuf;
use std::time::Instant;

const TARGET: TextureUsage = TextureUsage::RENDER_TARGET.union(TextureUsage::SAMPLED);

/// Rectangles over each other, some translucent, at fractions of a pixel: what timed and kept frames draw.
fn rects(f: &mut Frame<'_>) {
    let quads = [
        Quad::new(f.dp(8.0), f.dp(8.0), f.dp(80.0), f.dp(60.0), 1.0, 0.0, 0.0),
        Quad::new(f.dp(40.0), f.dp(30.0), f.dp(100.0), f.dp(90.0), 0.0, 0.5, 1.0).alpha(0.5),
        Quad::new(f.dp(120.0), f.dp(120.0), f.dp(64.0), f.dp(64.0), 0.2, 0.8, 0.2),
        Quad::new(f.dp(0.5), f.dp(150.25), f.dp(30.5), f.dp(20.0), 0.0, 0.0, 0.0),
    ];
    f.fill(&quads);
}

/// The same with red and blue swapped by the pipeline's constant.
fn swapped(f: &mut Frame<'_>) {
    let quads = [
        Quad::new(10.0, 10.0, 100.0, 50.0, 1.0, 0.5, 0.0),
        Quad::new(60.0, 40.0, 90.0, 90.0, 0.0, 0.0, 1.0).alpha(0.75),
    ];
    fill(f.device, f.pipelines.swapped.id(), f.target, &quads);
}

/// A texture drawn over the target, scaled.
fn textured(f: &mut Frame<'_>) {
    let texture = f.device.create_texture(4, 4, Format::Rgba8, TextureUsage::SAMPLED).expect("a texture");
    let texels: Vec<u8> = (0..64u32).map(|i| (i * 53 % 256) as u8 | 0x0f).collect();
    f.device.upload(texture.id(), IRect::new(0, 0, 4, 4), &texels, 16);
    f.device.begin_pass(f.target, None, &[texture.id()], None);
    blit(f.device, f.pipelines.copy.id(), texture.id(), f.target);
    f.device.end_pass();
}

/// Texels uploaded read back as they were, in every format: a part uploaded from a wider picture (its rows apart as
/// that picture's), a part read back from where it is, BGRA in its order, half floats exactly (subnormals and the
/// sign of zero too).
pub fn textures_read_back_as_uploaded(g: &mut Gpu) {
    let (w, h) = (4usize, 3usize);
    let halves: [u16; 11] = [0x3c00, 0xc000, 0x0400, 0x7bff, 0x0001, 0x03ff, 0x3555, 0x8000, 0x2e66, 0xfbff, 0x0000];
    let floats: [f32; 11] = [1.0, -2.5, 1e-30, 3.4e38, 0.1, -0.0, 65504.0, 1e-7, -1e20, 0.333, 7.0];
    for format in [Format::Rgba8, Format::Bgra8, Format::R16f, Format::Rgba16f, Format::Rgba32f] {
        let texel = format.bytes_per_texel();
        let row = texel * w;
        // Two pictures of w x h texels: the texture's, and the one a part of is put over it.
        let picture = |seed: usize| {
            let mut bytes: Vec<u8> = (0..row * h).map(|i| (i * 37 + seed) as u8).collect();
            if matches!(format, Format::R16f | Format::Rgba16f) {
                for (i, half) in bytes.as_chunks_mut::<2>().0.iter_mut().enumerate() {
                    *half = halves[(i * 3 + seed) % halves.len()].to_ne_bytes();
                }
            }
            if format == Format::Rgba32f {
                for (i, float) in bytes.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                    *float = floats[(i * 3 + seed) % floats.len()].to_ne_bytes();
                }
            }
            bytes
        };
        let (first, second) = (picture(0), picture(5));
        let mut want = first.clone();
        let part = IRect::new(1, 1, 2, 2);
        for y in part.y as usize..(part.y + part.height) as usize {
            let at = y * row + part.x as usize * texel;
            let from = (y - part.y as usize) * row;
            want[at..at + part.width as usize * texel]
                .copy_from_slice(&second[from..from + part.width as usize * texel]);
        }
        let t = g.device.create_texture(w as i32, h as i32, format, TARGET).expect("a texture");
        g.device.begin_frame();
        g.device.upload(t.id(), IRect::new(0, 0, w as i32, h as i32), &first, row);
        g.device.upload(t.id(), part, &second, row);
        let all = g.device.read_pixels(t.id(), IRect::new(0, 0, w as i32, h as i32));
        let some = g.device.read_pixels(t.id(), IRect::new(1, 2, 3, 1));
        g.device.end_frame(false);
        let (whole, piece) = (all == want, some == want[2 * row + texel..]);
        check!(whole && piece, "  {format:?}: read back {}", if whole { "a part wrong" } else { "wrong" });
    }
}

/// A readback in the middle of a frame (as a bitmap drawn into is read) leaves the frame going: what is drawn after
/// it is drawn, from the frame's memory handed out after it.
pub fn readbacks_leave_the_frame_going(g: &mut Gpu) {
    let n = 16;
    let first = g.device.create_texture(n, n, Format::Rgba8, TARGET).expect("a texture");
    let second = g.device.create_texture(n, n, Format::Rgba8, TARGET).expect("a texture");
    let color = g.pipelines.color.id();
    let d: &mut dyn Device = &mut g.device;
    d.begin_frame();
    d.begin_pass(first.id(), Some(ClearColor::WHITE), &[], None);
    draw_quads(d, color, first.id(), &[Quad::new(2.0, 2.0, 8.0, 8.0, 1.0, 0.0, 0.0)], [0.0; 4]);
    d.end_pass();
    let a = Pixels { width: n, height: n, rgba: d.read_pixels(first.id(), IRect::new(0, 0, n, n)) };
    d.begin_pass(second.id(), Some(ClearColor::WHITE), &[], None);
    draw_quads(d, color, second.id(), &[Quad::new(6.0, 6.0, 8.0, 8.0, 0.0, 0.0, 1.0)], [0.0; 4]);
    d.end_pass();
    let b = Pixels { width: n, height: n, rgba: d.read_pixels(second.id(), IRect::new(0, 0, n, n)) };
    d.end_frame(false);
    check!(near(a.at(4, 4), 255, 0, 0, 2) && near(a.at(12, 12), 255, 255, 255, 2));
    check!(near(b.at(10, 10), 0, 0, 255, 2) && near(b.at(3, 3), 255, 255, 255, 2));
}

/// The GPU's time for each frame, where the device measures it: frames that drew come back a frame or two later,
/// with their passes named by their targets, each within the frame's time; none once it is turned off.
pub fn gpu_times_come_back(g: &mut Gpu) {
    if !g.device.set_gpu_timing(true) {
        println!("  (the GPU's time isn't measured on this device)");
        return;
    }
    let mut times = Vec::new();
    // A frame's wall time, drawn and read back: its GPU work's within it.
    let mut longest = 0.0f64;
    for _ in 0..16 {
        if times.len() >= 3 {
            break;
        }
        let start = Instant::now();
        g.render_quads(192, 192, 1.0, rects, ClearColor::WHITE);
        longest = longest.max(start.elapsed().as_secs_f64() * 1000.0);
        times.extend(g.device.take_gpu_times());
    }
    check!(times.len() >= 3, "  {} frames timed", times.len());
    for t in &times {
        check!(t.ms >= 0.0 && t.ms <= longest + 0.5, "  a frame took {} ms, the longest {longest} ms", t.ms);
        check!(t.passes.iter().any(|p| p.label == "192x192" && p.ms >= 0.0 && p.ms <= t.ms + 1e-3), "  {t:?}");
    }
    g.device.set_gpu_timing(false);
    // (Those timed before still coming back.)
    for _ in 0..4 {
        g.render_quads(192, 192, 1.0, rects, ClearColor::WHITE);
        g.device.take_gpu_times();
    }
    for _ in 0..4 {
        g.render_quads(192, 192, 1.0, rects, ClearColor::WHITE);
    }
    check!(g.device.take_gpu_times().is_empty());
}

/// A pass with an area clears there (Vulkan's render area), the rest of the target kept.
pub fn pass_areas_clear_only_there(g: &mut Gpu) {
    let (w, h) = (48, 32);
    g.device.begin_frame();
    let target = g.device.create_texture(w, h, Format::Rgba8, TARGET).expect("a texture");
    g.device.begin_pass(target.id(), Some(ClearColor::new(1.0, 0.0, 0.0, 1.0)), &[], None);
    g.device.end_pass();
    g.device.begin_pass(target.id(), Some(ClearColor::new(0.0, 0.0, 1.0, 1.0)), &[], Some(IRect::new(10, 6, 20, 12)));
    g.device.end_pass();
    let p = Pixels { width: w, height: h, rgba: g.device.read_pixels(target.id(), IRect::new(0, 0, w, h)) };
    g.device.end_frame(false);
    let mut wrong = 0;
    for y in 0..h {
        for x in 0..w {
            let inside = (10..30).contains(&x) && (6..18).contains(&y);
            if !if inside { near(p.at(x, y), 0, 0, 255, 2) } else { near(p.at(x, y), 255, 0, 0, 2) } {
                wrong += 1;
            }
        }
    }
    check!(wrong == 0, "  {wrong} pixels wrong");
}

/// A texture discarded, then drawn over whole: its pass needn't load it, and what it draws is all there is (whether
/// the driver loaded it, cleared it or left anything under it).
pub fn discarded_textures_are_drawn_over(g: &mut Gpu) {
    let (w, h) = (40, 30);
    let frame = g.device.create_texture(w, h, Format::Rgba8, TARGET).expect("a texture");
    let out = g.device.create_texture(w, h, Format::Rgba8, TARGET).expect("a texture");
    let kept = g.render_quads(
        w,
        h,
        1.0,
        |f| {
            let (copy, color) = (f.pipelines.copy.id(), f.pipelines.color.id());
            let d = &mut *f.device;
            // (What it held before: never to show.)
            d.begin_pass(out.id(), Some(ClearColor::new(1.0, 0.0, 0.0, 1.0)), &[], None);
            d.end_pass();
            d.begin_pass(frame.id(), Some(ClearColor::new(0.0, 1.0, 0.0, 1.0)), &[], None);
            draw_quads(d, color, frame.id(), &[Quad::new(4.0, 4.0, 10.0, 8.0, 0.0, 0.0, 1.0)], [0.0; 4]);
            d.end_pass();
            d.discard(out.id());
            d.begin_pass(out.id(), None, &[frame.id()], None);
            blit(d, copy, frame.id(), out.id());
            d.end_pass();
            d.begin_pass(f.target, None, &[frame.id()], None);
            blit(d, copy, frame.id(), f.target);
            d.end_pass();
        },
        ClearColor::WHITE,
    );
    g.device.begin_frame();
    let shown = g.device.read_pixels(out.id(), IRect::new(0, 0, w, h));
    g.device.end_frame(false);
    check!(shown == kept.rgba);
    check!(near(kept.at(5, 5), 0, 0, 255, 0) && near(kept.at(20, 20), 0, 255, 0, 0));
}

/// A scissor stays within its pass's area, and nothing is drawn outside either.
pub fn scissors_stay_within_the_pass(g: &mut Gpu) {
    let (w, h) = (48, 32);
    let p = g.render_quads(
        w,
        h,
        1.0,
        |f| {
            let color = f.pipelines.color.id();
            f.device.begin_pass(f.target, None, &[], Some(IRect::new(8, 8, 32, 16)));
            f.device.set_scissor(IRect::new(-10, 0, 34, 100));
            draw_quads(f.device, color, f.target, &[Quad::new(0.0, 0.0, 48.0, 32.0, 0.0, 0.0, 1.0)], [0.0; 4]);
            // A scissor of nothing draws nothing.
            f.device.set_scissor(IRect::new(30, 10, 0, 5));
            draw_quads(f.device, color, f.target, &[Quad::new(0.0, 0.0, 48.0, 32.0, 1.0, 0.0, 0.0)], [0.0; 4]);
            f.device.end_pass();
        },
        ClearColor::WHITE,
    );
    let mut wrong = 0;
    for y in 0..h {
        for x in 0..w {
            let inside = (8..24).contains(&x) && (8..24).contains(&y);
            if !if inside { near(p.at(x, y), 0, 0, 255, 0) } else { near(p.at(x, y), 255, 255, 255, 0) } {
                wrong += 1;
            }
        }
    }
    check!(wrong == 0, "  {wrong} pixels wrong");
}

/// Pipelines draw as described: their blending (factors and operations, or none), their specialization constants,
/// and their target's format (BGRA, read back in RGBA order).
pub fn pipelines_are_made_as_described(g: &mut Gpu) {
    use BlendFactor::{One, OneMinusSrcAlpha};
    let plus = BlendState::factors(One, One, One, One);
    let min = BlendState { color_op: BlendOp::Min, alpha_op: BlendOp::Min, ..plus };
    // Over a dark gray: a translucent red, premultiplied, at alpha 0.4.
    let under = ClearColor::new(0.2, 0.2, 0.2, 1.0);
    let quad = Quad { rect: [0.0, 0.0, 4.0, 4.0], color: [0.4, 0.0, 0.0, 0.4] };
    let cases: [(BlendState, [u8; 4]); 4] = [
        (BlendState::factors(One, OneMinusSrcAlpha, One, OneMinusSrcAlpha), [133, 31, 31, 255]),
        (plus, [153, 51, 51, 255]),
        (min, [51, 0, 0, 102]),
        (BlendState::OFF, [102, 0, 0, 102]),
    ];
    for (blend, want) in cases {
        let pipeline = quad_pipeline(&mut g.device, &COLOR_FRAGMENT, blend, Vec::new()).expect("a pipeline");
        let p = g.render_quads(4, 4, 1.0, |f| fill(f.device, pipeline.id(), f.target, &[quad]), under);
        let got = p.at(2, 2);
        check!(
            got.iter().zip(want).all(|(&a, b)| (a as i32 - b as i32).abs() <= 1),
            "  {blend:?}: {got:?}, not {want:?}"
        );
    }
    // The constant swaps red and blue.
    let p = g.render_quads(
        8,
        8,
        1.0,
        |f| fill(f.device, f.pipelines.swapped.id(), f.target, &[Quad::new(0.0, 0.0, 8.0, 8.0, 1.0, 0.5, 0.0)]),
        ClearColor::WHITE,
    );
    check!(near(p.at(4, 4), 0, 128, 255, 1), "  {:?}", p.at(4, 4));
    // Into BGRA: red is red.
    let mut desc = quad_desc(&COLOR_FRAGMENT);
    desc.target = Format::Bgra8;
    let bgra = g.device.create_pipeline(&desc).expect("a pipeline");
    let target = g.device.create_texture(8, 8, Format::Bgra8, TARGET).expect("a texture");
    g.device.begin_frame();
    g.device.begin_pass(target.id(), Some(ClearColor::new(0.0, 1.0, 0.0, 1.0)), &[], None);
    draw_quads(&mut g.device, bgra.id(), target.id(), &[Quad::new(0.0, 0.0, 4.0, 8.0, 1.0, 0.0, 0.0)], [0.0; 4]);
    g.device.end_pass();
    let p = Pixels { width: 8, height: 8, rgba: g.device.read_pixels(target.id(), IRect::new(0, 0, 8, 8)) };
    g.device.end_frame(false);
    check!(near(p.at(1, 1), 255, 0, 0, 0) && near(p.at(6, 1), 0, 255, 0, 0), "  {:?} {:?}", p.at(1, 1), p.at(6, 1));
}

/// Pipelines made on other threads at once (as the renderer warms them) draw as those made on the device's.
pub fn pipelines_are_made_on_other_threads(g: &mut Gpu) {
    let Some(factory) = g.device.pipeline_factory() else {
        println!("  (this device makes pipelines on its own thread only)");
        return;
    };
    let threads: Vec<_> = (0..4)
        .map(|i| {
            let factory = factory.clone();
            std::thread::spawn(move || {
                let mut desc = quad_desc(&COLOR_FRAGMENT);
                desc.constants = vec![i % 2];
                factory.create_pipeline(&desc)
            })
        })
        .collect();
    let made: Vec<_> = threads.into_iter().map(|t| t.join().ok().flatten()).collect();
    check!(made.iter().all(Option::is_some));
    for (i, pipeline) in made.iter().enumerate() {
        let Some(pipeline) = pipeline else { continue };
        let p = g.render_quads(
            8,
            8,
            1.0,
            |f| fill(f.device, pipeline.id(), f.target, &[Quad::new(0.0, 0.0, 8.0, 8.0, 1.0, 0.0, 0.0)]),
            ClearColor::WHITE,
        );
        let want = if i % 2 == 1 { [0, 0, 255] } else { [255, 0, 0] };
        check!(near(p.at(3, 3), want[0], want[1], want[2], 0), "  pipeline {i}: {:?}", p.at(3, 3));
    }
}

/// A mipmapped texture's smaller copies, each half the one before: drawn at an eighth of its size, a texture of four
/// colored quarters is their average; at a quarter, the quarters themselves.
pub fn mipmaps_halve_the_texture(g: &mut Gpu) {
    let n = 8;
    let texture = g
        .device
        .create_texture(n, n, Format::Rgba8, TextureUsage::SAMPLED | TextureUsage::MIPMAPPED)
        .expect("a texture");
    let quarter = |x: i32, y: i32| -> [u8; 4] {
        match (x < n / 2, y < n / 2) {
            (true, true) => [255, 0, 0, 255],
            (false, true) => [0, 255, 0, 255],
            (true, false) => [0, 0, 255, 255],
            (false, false) => [255, 255, 255, 255],
        }
    };
    let texels: Vec<u8> = (0..n * n).flat_map(|i| quarter(i % n, i / n)).collect();
    let mipmapped = Sampler { filter: Filter::Linear, mipmaps: true, ..Sampler::default() };
    let draw = |f: &mut Frame<'_>| {
        f.device.upload(texture.id(), IRect::new(0, 0, n, n), &texels, n as usize * 4);
        f.device.generate_mipmaps(texture.id());
        f.device.begin_pass(f.target, None, &[texture.id()], None);
        f.device.set_texture(0, texture.id(), mipmapped);
        let (w, h) = (f.target.width() as f32, f.target.height() as f32);
        draw_quads(
            f.device,
            f.pipelines.copy.id(),
            f.target,
            &[Quad::new(0.0, 0.0, w, h, 1.0, 1.0, 1.0)],
            [0.0, 0.0, 1.0, 1.0],
        );
        f.device.end_pass();
    };
    let one = g.render_quads(1, 1, 1.0, draw, ClearColor::TRANSPARENT);
    check!(near(one.at(0, 0), 128, 128, 128, 2), "  {:?}", one.at(0, 0));
    let two = g.render_quads(2, 2, 1.0, draw, ClearColor::TRANSPARENT);
    let quarters = [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(x, y)| two.at(x, y));
    check!(quarters == [(0, 0), (n - 1, 0), (0, n - 1), (n - 1, n - 1)].map(|(x, y)| quarter(x, y)), "  {quarters:?}");
}

/// Part of a texture copied into another, at a place of its own; the rest of it as it was.
pub fn copies_move_texels(g: &mut Gpu) {
    let n = 16;
    let a = g.device.create_texture(n, n, Format::Rgba8, TARGET).expect("a texture");
    let b = g.device.create_texture(n, n, Format::Rgba8, TARGET).expect("a texture");
    let texels: Vec<u8> = (0..n * n * 4).map(|i| (i * 7 + 3) as u8).collect();
    g.device.begin_frame();
    g.device.upload(a.id(), IRect::new(0, 0, n, n), &texels, n as usize * 4);
    g.device.begin_pass(b.id(), Some(ClearColor::TRANSPARENT), &[], None);
    g.device.end_pass();
    let region = IRect::new(2, 3, 5, 4);
    g.device.copy_texture(a.id(), region, b.id(), 7, 9);
    let copied = g.device.read_pixels(b.id(), IRect::new(0, 0, n, n));
    g.device.end_frame(false);
    let mut wrong = 0;
    for y in 0..n {
        for x in 0..n {
            let (sx, sy) = (x - 7 + region.x, y - 9 + region.y);
            let inside = (7..7 + region.width).contains(&x) && (9..9 + region.height).contains(&y);
            let at = |px: i32, py: i32| (py * n + px) as usize * 4;
            let want = if inside { &texels[at(sx, sy)..at(sx, sy) + 4] } else { &[0, 0, 0, 0][..] };
            wrong += (&copied[at(x, y)..at(x, y) + 4] != want) as usize;
        }
    }
    check!(wrong == 0, "  {wrong} texels wrong");
}

/// Frame memory beyond a chunk (4 MB) comes in a chunk as big as asked, and is drawn from; what is handed out is
/// aligned as asked.
pub fn frame_memory_grows_as_asked(g: &mut Gpu) {
    let (w, h) = (400, 375);
    // A quad on each pixel, 32 bytes each: 4.8 MB.
    let quads: Vec<Quad> = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f32, (i / w) as f32);
            if (i % w + i / w) % 2 == 0 {
                Quad::new(x, y, 1.0, 1.0, 0.0, 0.0, 0.0)
            } else {
                Quad::new(x, y, 1.0, 1.0, 0.0, 0.0, 1.0)
            }
        })
        .collect();
    let mut aligned = true;
    let p = g.render_quads(
        w,
        h,
        1.0,
        |f| {
            for alignment in [16, 64, 256, 4096] {
                aligned &= f
                    .device
                    .write_frame_data(&[1, 2, 3], alignment)
                    .is_some_and(|b| b.offset() % alignment as u64 == 0);
            }
            f.fill(&quads);
        },
        ClearColor::WHITE,
    );
    check!(aligned);
    let mut wrong = 0;
    for y in 0..h {
        for x in 0..w {
            let want = if (x + y) % 2 == 0 { [0, 0, 0] } else { [0, 0, 255] };
            wrong += !near(p.at(x, y), want[0], want[1], want[2], 0) as usize;
        }
    }
    check!(wrong == 0, "  {wrong} pixels wrong");
}

/// Textures of many sizes made, filled, read back and let go of over frames: each holds what was put in it (none
/// shares another's memory), small ones sharing blocks, big ones with memories of their own, those let go of
/// destroyed once their frames are done.
pub fn textures_come_and_go(g: &mut Gpu) {
    let sizes =
        [(1, 1), (3, 5), (16, 16), (64, 33), (128, 128), (257, 100), (512, 512), (700, 800), (1100, 700), (31, 2000)];
    let mut live = Vec::new();
    let mut wrong = Vec::new();
    for round in 0..6usize {
        g.device.begin_frame();
        for (i, &(w, h)) in sizes.iter().enumerate() {
            if (i + round) % 3 == 0 {
                continue;
            }
            let texture = g.device.create_texture(w, h, Format::Rgba8, TextureUsage::SAMPLED).expect("a texture");
            let seed = round * 31 + i;
            let texels: Vec<u8> = (0..(w * h * 4) as usize).map(|k| (k * 13 + seed) as u8).collect();
            g.device.upload(texture.id(), IRect::new(0, 0, w, h), &texels, w as usize * 4);
            live.push((texture, texels));
        }
        // Each read back whole, the newest and the oldest together.
        for (texture, texels) in &live {
            if g.device.read_pixels(texture.id(), IRect::new(0, 0, texture.width(), texture.height())) != *texels {
                wrong.push((round, texture.width(), texture.height()));
            }
        }
        // Every other one let go of (in this frame: destroyed once it is done).
        let mut k = 0;
        live.retain(|_| {
            k += 1;
            k % 2 == 0
        });
        g.device.end_frame(false);
    }
    check!(wrong.is_empty(), "  read back wrong (round, width, height): {wrong:?}");
}

/// A device keeps the pipelines it made in the file it is given, and one started from that file draws as one that
/// compiled them anew; a file cut short, or another's, is passed over as none (and written anew).
pub fn pipelines_are_kept_between_runs() {
    // (FNV-1a's own.)
    check!(fnv1a(b"a") == 0xaf63dc4c8601ec8c);
    let file = std::env::temp_dir().join(format!("kompoz_gpu_tests_pipelines_{}", std::process::id()));
    let pictures = |keeps: &mut bool| -> Vec<Pixels> {
        let mut g = match Gpu::start(Some(file.clone())) {
            Ok(g) => g,
            Err(why) => {
                check!(false, "  no device: {why}");
                return Vec::new();
            }
        };
        *keeps = g.keeps_pipelines();
        let out: Vec<Pixels> = [rects as fn(&mut Frame<'_>), swapped, textured]
            .into_iter()
            .map(|draw| g.render_quads(192, 192, 1.0, draw, ClearColor::WHITE))
            .collect();
        check!(g.device.validation_errors() == 0);
        // (Kept at a frame's end, the app going on.)
        check!(read_pipeline_cache(&file).is_empty() != *keeps);
        out
    };
    let mut keeps = false;
    let fresh = pictures(&mut keeps);
    let kept = read_pipeline_cache(&file);
    check!(kept.is_empty() != keeps);
    if !kept.is_empty() {
        // All of it taken, and nothing compiled: the file isn't written again.
        let modified = |file: &PathBuf| std::fs::metadata(file).and_then(|m| m.modified()).ok();
        let written = modified(&file);
        check!(pictures(&mut keeps) == fresh);
        check!(modified(&file) == written);
        // Cut short, or a byte changed (the data's last, the file's first): none read; then the whole file again.
        // (Not byte for byte the first: a driver's binaries may differ as it compiled them or took them from a cache
        // of its own, as Mesa's do.)
        let length = std::fs::metadata(&file).map_or(0, |m| m.len());
        let _ = std::fs::OpenOptions::new().write(true).open(&file).and_then(|f| f.set_len(length / 2));
        check!(read_pipeline_cache(&file).is_empty());
        check!(pictures(&mut keeps) == fresh);
        check!(!read_pipeline_cache(&file).is_empty());
        let length = std::fs::metadata(&file).map_or(0, |m| m.len());
        for at in [length - 1, 0] {
            let mut bytes = std::fs::read(&file).unwrap_or_default();
            if let Some(byte) = bytes.get_mut(at as usize) {
                *byte = !*byte;
            }
            let _ = std::fs::write(&file, &bytes);
            check!(read_pipeline_cache(&file).is_empty());
            check!(pictures(&mut keeps) == fresh);
            check!(!read_pipeline_cache(&file).is_empty());
        }
        // Whole, but another driver's (its header's version a bit off) or not one at all: passed over, and written
        // anew.
        let mut other = kept.clone();
        other[4] ^= 1;
        for written in [other, vec![0x5a; kept.len()]] {
            write_pipeline_cache(&file, &written);
            check!(read_pipeline_cache(&file) == written);
            check!(pictures(&mut keeps) == fresh);
            let again = read_pipeline_cache(&file);
            check!(!again.is_empty() && again != written);
        }
    }
    let _ = std::fs::remove_file(&file);
}

/// The golden comparison: every picture there is read, of its size; a picture matches itself, and pixels off by 3
/// still match while those off by more are counted, the picture as it came and where it differs written beside it;
/// updating writes it anew.
pub fn goldens_compare_as_they_should() {
    let names = [
        "rects",
        "lines",
        "shadows",
        "text",
        "images",
        "layers",
        "paths",
        "outline-layers",
        "gradients",
        "blend-modes",
        "transforms",
        "graphics-layers",
        "effects",
        "density",
    ];
    for name in names {
        let picture = golden::load_png(&golden::golden_dir().join(format!("{name}.png")));
        let size = picture.as_ref().map(|p| (p.width, p.height));
        let want = if name == "graphics-layers" { (192, 196) } else { (192, 192) };
        check!(size == Some(want), "  {name}: {size:?}");
    }
    let Some(rects) = golden::load_png(&golden::golden_dir().join("rects.png")) else { return };
    check!(golden::difference(&rects, &rects) == Some(Difference { worst: 0, count: 0 }));
    let mut off = rects.clone();
    for i in 0..100 {
        let at = i * 1337 % off.rgba.len();
        off.rgba[at] = if off.rgba[at] >= 128 { off.rgba[at] - 3 } else { off.rgba[at] + 3 };
    }
    check!(golden::difference(&rects, &off) == Some(Difference { worst: 3, count: 0 }));
    let changed = [(5, 7), (100, 3), (191, 191), (0, 0), (64, 128)];
    for (x, y) in changed {
        let at = (y * 192 + x) * 4 + 1;
        off.rgba[at] = if off.rgba[at] >= 128 { off.rgba[at] - 4 } else { off.rgba[at] + 4 };
    }
    check!(golden::difference(&rects, &off) == Some(Difference { worst: 4, count: 5 }));

    let dir = std::env::temp_dir().join(format!("kompoz_gpu_tests_goldens_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    check!(golden::save(&dir.join("rects.png"), &rects));
    check!(golden::match_golden_in(&dir, "rects", &rects, false) == Outcome::Matches(Difference::default()));
    check!(golden::match_golden_in(&dir, "rects", &off, false) == Outcome::Differs(Difference { worst: 4, count: 5 }));
    check!(golden::load_png(&dir.join("rects.actual.png")).as_ref() == Some(&off));
    let diff = golden::load_png(&dir.join("rects.diff.png"));
    let reds = diff.map_or(0, |d| d.rgba.as_chunks::<4>().0.iter().filter(|p| **p == [255, 0, 0, 255]).count());
    check!(reds == changed.len(), "  {reds} red pixels");
    let small = Pixels { width: 2, height: 2, rgba: vec![0; 16] };
    check!(golden::match_golden_in(&dir, "rects", &small, false) == Outcome::NoPicture);
    check!(golden::match_golden_in(&dir, "none", &rects, false) == Outcome::NoPicture);
    check!(golden::match_golden_in(&dir, "rects", &off, true) == Outcome::Written);
    check!(golden::load_png(&dir.join("rects.png")).as_ref() == Some(&off));
    let _ = std::fs::remove_dir_all(&dir);
}
