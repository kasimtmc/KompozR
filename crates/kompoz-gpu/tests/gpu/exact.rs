//! What the renderer draws exactly, checked as such (the C++ Kompoz's exact checks): an edge half a pixel in covers
//! half the pixel; a slanted or turned edge cuts each pixel's box as its polygon does; clips, paths and strokes
//! cover what their shapes do; gradients are Brush::color_at at each pixel's middle; blend modes and color filters
//! are blend() and ColorFilter::apply; transforms carry edges exactly; conics, path effects and aliased drawing are
//! as Skia's. Tests at another px per dp than 1 give C++'s dp coordinates through `k`.

use crate::harness::{Gpu, Pixels, max_difference, near};
use crate::scenes::{BLEND_MODES, about, fill, oval, rgba, star, stroke, xywh};
use kompoz_gpu::Renderer;
use kompoz_gpu::device::ClearColor;
use kompoz_gpu::path::{PathPlacement, flatten, stroke_outline};
use kompoz_graphics::{
    BlendMode, Brush, Color, ColorFilter, ColorMatrix, Contour, Density, FilterQuality, GenericShape, ImageBitmap,
    LayoutDirection, Matrix, Offset, Path, PathEffect, PathFillType, PathMeasure, PathOperation, PathStroke, Rect,
    StrokeCap, StrokeJoin, TileMode, blend,
};
use kompoz_ui::draw::{Canvas, Paint, PaintingStyle};
use kompoz_ui::graphics_context::{ClipOp, CornerRadii, GraphicsContext};

/// How much of each pixel of a w x h picture polygons (each closed) cover by the non-zero rule: along 64 lines
/// across each row of pixels, exactly where each line is inside.
pub fn coverage_of(polygons: &[Contour], w: i32, h: i32) -> Vec<f32> {
    let mut out = vec![0.0f32; (w * h) as usize];
    let mut crossings: Vec<(f32, i32)> = Vec::new();
    for y in 0..h {
        for j in 0..64 {
            let line = y as f32 + (j as f32 + 0.5) / 64.0;
            crossings.clear();
            for c in polygons {
                let n = c.points.len();
                for i in 0..n {
                    let (a, b) = (c.points[i], c.points[(i + 1) % n]);
                    if (a.y <= line) != (b.y <= line) {
                        let x = a.x + (b.x - a.x) * (line - a.y) / (b.y - a.y);
                        crossings.push((x, if b.y > a.y { 1 } else { -1 }));
                    }
                }
            }
            crossings.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mut winding = 0;
            for k in 0..crossings.len().saturating_sub(1) {
                winding += crossings[k].1;
                if winding == 0 {
                    continue;
                }
                let (from, to) = (crossings[k].0.max(0.0), crossings[k + 1].0.min(w as f32));
                let mut x = from.floor() as i32;
                while x < w && (x as f32) < to {
                    out[(y * w + x) as usize] += (to.min((x + 1) as f32) - from.max(x as f32)) / 64.0;
                    x += 1;
                }
            }
        }
    }
    out
}

/// The largest difference between two pictures, the second's pixels found where `at` says (and those `skip` says
/// left out), and where it is.
pub fn worst_against(
    a: &Pixels,
    at: impl Fn(i32, i32) -> [u8; 4],
    skip: impl Fn(i32, i32) -> bool,
) -> (i32, (i32, i32)) {
    let mut worst = (0, (0, 0));
    for y in 0..a.height {
        for x in 0..a.width {
            let d = if skip(x, y) { 0 } else { max_difference(a.at(x, y), at(x, y)) };
            if d > worst.0 {
                worst = (d, (x, y));
            }
        }
    }
    worst
}

/// Nothing left out.
pub fn none(_: i32, _: i32) -> bool {
    false
}

/// Pixels within 1.5 px of any of `corners`.
pub fn around(corners: Vec<Offset>) -> impl Fn(i32, i32) -> bool {
    move |x, y| corners.iter().any(|c| (x as f32 + 0.5 - c.x).abs() < 1.5 && (y as f32 + 0.5 - c.y).abs() < 1.5)
}

/// What `want` coverage of red over white leaves of green (and blue), 0..255.
fn over_white(want: f32) -> i32 {
    (255.0 * (1.0 - want.min(1.0))).round() as i32
}

/// How much of the picture is covered (red or black over white): its green's shortfall, in pixels.
fn covered_area(p: &Pixels) -> f32 {
    let mut sum = 0.0;
    for y in 0..p.height {
        for x in 0..p.width {
            sum += (255 - p.at(x, y)[1] as i32) as f32 / 255.0;
        }
    }
    sum
}

/// A straight color premultiplied, as 8-bit channels.
fn premultiplied8(c: Color) -> [u8; 4] {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [b(c.red() * c.alpha()), b(c.green() * c.alpha()), b(c.blue() * c.alpha()), b(c.alpha())]
}

const RED: Color = Color::new(1.0, 0.0, 0.0, 1.0);

pub fn edges_are_covered_exactly(g: &mut Gpu) {
    let p = g.render(64, 32, |c| {
        c.fill_round_rect(xywh(4.0, 4.0, 10.0, 10.0), CornerRadii::Zero, RED); // whole pixels
        c.fill_round_rect(xywh(20.5, 4.0, 10.0, 10.0), CornerRadii::Zero, RED); // half a pixel in
        c.fill_round_rect(xywh(40.0, 4.25, 10.0, 10.0), CornerRadii::Zero, rgba(0.0, 0.0, 1.0, 1.0));
    });
    check!(near(p.at(8, 8), 255, 0, 0, 2)); // inside: the color
    check!(near(p.at(3, 8), 255, 255, 255, 2)); // outside: the background
    check!(near(p.at(4, 8), 255, 0, 0, 2)); // the first pixel, whole
    check!(near(p.at(13, 8), 255, 0, 0, 2));
    check!(near(p.at(14, 8), 255, 255, 255, 2));
    check!(near(p.at(20, 8), 255, 128, 128, 2)); // half covered: half way between
    check!(near(p.at(30, 8), 255, 128, 128, 2));
    check!(near(p.at(25, 8), 255, 0, 0, 2));
    check!(near(p.at(45, 4), 64, 64, 255, 2)); // three quarters covered
}

/// A straight edge at any angle cuts each pixel's box exactly (a slanted line's sides, a turned rect's, as a path's
/// polygon covers it), a corner square to the screen as its two edges' product, and a ring thinner than a pixel
/// covers as much as it is wide.
pub fn edges_cut_pixels_exactly(g: &mut Gpu) {
    let p = g.render(64, 64, |c| {
        c.fill_round_rect(xywh(4.5, 4.25, 8.0, 8.0), CornerRadii::Zero, RED); // a corner half and three quarters in
        c.stroke_round_rect(xywh(20.0, 4.0, 12.0, 12.0), CornerRadii::Zero, 0.5, RED); // half a pixel wide
        c.draw_line(Offset::new(6.0, 40.0), Offset::new(30.0, 26.0), 3.0, RED); // slanted
        c.fill_round_rect(xywh(40.5, 4.25, 8.0, 8.0), CornerRadii::all(0.4), RED); // rounded by less than a pixel
    });
    check!(near(p.at(4, 4), 255, 159, 159, 2)); // 0.5 x 0.75
    // Rounded by 0.4 px, the corner covers a little less than the square one (0.375 - 0.034): the circle's edge
    // alone would make it more.
    check!(p.at(40, 4)[1] >= 159 && p.at(40, 4)[1] <= 175, "  {:?}", p.at(40, 4));
    check!(near(p.at(20, 4), 255, 64, 64, 2)); // 0.5 + 0.5 - 0.25
    check!(near(p.at(26, 4), 255, 128, 128, 2)); // the side: half
    // The line's sides as its polygon covers them (its square caps' corners aside).
    let len = 24.0f32.hypot(-14.0);
    let (ux, uy) = (24.0 / len * 1.5, -14.0 / len * 1.5);
    let quad = |points: Vec<Offset>| Contour { points, closed: true };
    let boxed = vec![
        Offset::new(6.0 - ux + uy, 40.0 - uy - ux),
        Offset::new(30.0 + ux + uy, 26.0 + uy - ux),
        Offset::new(30.0 + ux - uy, 26.0 + uy + ux),
        Offset::new(6.0 - ux - uy, 40.0 - uy + ux),
    ];
    let want = coverage_of(&[quad(boxed.clone())], 64, 64);
    let corners = around(boxed);
    let mut worst = 0;
    for y in 20..48 {
        for x in 0..40 {
            if !corners(x, y) {
                worst = worst.max((p.at(x, y)[1] as i32 - over_white(want[(y * 64 + x) as usize])).abs());
            }
        }
    }
    check!(worst <= 3, "  slanted line: off by {worst}");
    // A rect turned 30 degrees, against its polygon (the corners aside).
    let turn = about(40.0, 40.0, |m| m.rotate_z(30.0));
    let t = g.render(64, 64, |c| {
        c.push_transform(&turn);
        c.fill_round_rect(xywh(30.0, 34.0, 20.0, 12.0), CornerRadii::Zero, RED);
        c.pop_transform();
    });
    let turned: Vec<Offset> =
        [(30.0, 34.0), (50.0, 34.0), (50.0, 46.0), (30.0, 46.0)].map(|(x, y)| turn.map(Offset::new(x, y))).to_vec();
    let want = coverage_of(&[quad(turned.clone())], 64, 64);
    let corners = around(turned);
    let mut worst = 0;
    for y in 0..64 {
        for x in 0..64 {
            if !corners(x, y) {
                worst = worst.max((t.at(x, y)[1] as i32 - over_white(want[(y * 64 + x) as usize])).abs());
            }
        }
    }
    check!(worst <= 3, "  turned rect: off by {worst}");
    // A ring half a pixel wide covers 2 pi r 0.5.
    let ring = g.render(64, 64, |c| c.draw_arc(xywh(12.0, 12.0, 40.0, 40.0), 0.0, 360.0, 0.5, RED, false));
    let area = covered_area(&ring);
    check!((area - 2.0 * std::f32::consts::PI * 20.0 * 0.5).abs() < 0.6, "  {area}");
}

pub fn clips_and_translations_are_exact(g: &mut Gpu) {
    let k = 2.0; // (2 px per dp)
    let p = g.render(32, 32, |c| {
        c.push_clip(xywh(2.0 * k, 2.0 * k, 4.0 * k, 4.0 * k)); // px 4..12
        c.fill_round_rect(xywh(0.0, 0.0, 16.0 * k, 16.0 * k), CornerRadii::Zero, rgba(0.0, 0.5, 0.0, 1.0));
        c.pop_clip();
        c.push_translation(Offset::new(8.0 * k, 8.0 * k));
        c.fill_round_rect(xywh(0.0, 0.0, 2.0 * k, 2.0 * k), CornerRadii::Zero, rgba(0.0, 0.0, 0.0, 1.0)); // px 16..20
        c.pop_translation();
        c.push_clip(xywh(12.0 * k, 2.0 * k, 3.0 * k, 3.0 * k)); // px 24..30
        c.push_clip(xywh(10.0 * k, 0.0, 6.0 * k, 16.0 * k)); // bigger: still only within the one outside it
        c.fill_round_rect(xywh(10.0 * k, 0.0, 6.0 * k, 16.0 * k), CornerRadii::Zero, rgba(0.5, 0.0, 0.5, 1.0));
        c.pop_clip();
        c.pop_clip();
    });
    check!(near(p.at(3, 6), 255, 255, 255, 2));
    check!(near(p.at(4, 6), 0, 128, 0, 2));
    check!(near(p.at(11, 11), 0, 128, 0, 2));
    check!(near(p.at(12, 11), 255, 255, 255, 2));
    check!(near(p.at(16, 16), 0, 0, 0, 2));
    check!(near(p.at(19, 19), 0, 0, 0, 2));
    check!(near(p.at(20, 20), 255, 255, 255, 2));
    check!(near(p.at(26, 6), 128, 0, 128, 2));
    check!(near(p.at(26, 2), 255, 255, 255, 2)); // inside the inner clip, outside the outer one
    check!(near(p.at(22, 6), 255, 255, 255, 2));
    check!(near(p.at(26, 12), 255, 255, 255, 2));
}

pub fn paths_are_covered_exactly(g: &mut Gpu) {
    let none = Rect::Zero;
    let p = g.render(64, 40, |c| {
        let mut square = Path::new(); // as a path: the same coverage as the rect
        square.add_rect(xywh(4.5, 4.0, 10.0, 10.0), Default::default());
        c.draw_path(&square, Offset::Zero, &Brush::from(RED), none, &fill());
        let mut nested = Path::new(); // two squares the same way round: non-zero fills the inner one, even-odd doesn't
        nested.add_rect(xywh(20.0, 4.0, 16.0, 16.0), Default::default());
        nested.add_rect(xywh(24.0, 8.0, 8.0, 8.0), Default::default());
        let blue = Brush::from(rgba(0.0, 0.0, 1.0, 1.0));
        c.draw_path(&nested, Offset::Zero, &blue, none, &fill());
        let mut holed = nested.clone();
        holed.translate(Offset::new(20.0, 0.0));
        holed.fill_type = PathFillType::EvenOdd;
        c.draw_path(&holed, Offset::Zero, &blue, none, &fill());
        let mut line = Path::new(); // a 2 px stroke along whole pixels, butt caps: exactly a 2 px wide rect
        line.move_to(4.0, 30.0);
        line.line_to(20.0, 30.0);
        let black = Brush::from(rgba(0.0, 0.0, 0.0, 1.0));
        c.draw_path(&line, Offset::Zero, &black, none, &stroke(2.0, StrokeCap::Butt, StrokeJoin::Miter, 4.0));
    });
    check!(near(p.at(4, 8), 255, 128, 128, 2)); // half covered
    check!(near(p.at(8, 8), 255, 0, 0, 2));
    check!(near(p.at(14, 8), 255, 128, 128, 2));
    check!(near(p.at(15, 8), 255, 255, 255, 2));
    check!(near(p.at(27, 11), 0, 0, 255, 2)); // non-zero: inside the inner square too
    check!(near(p.at(47, 11), 255, 255, 255, 2)); // even-odd: a hole
    check!(near(p.at(41, 5), 0, 0, 255, 2));
    check!(near(p.at(10, 29), 0, 0, 0, 2));
    check!(near(p.at(10, 30), 0, 0, 0, 2));
    check!(near(p.at(10, 28), 255, 255, 255, 2));
    check!(near(p.at(10, 31), 255, 255, 255, 2));
    check!(near(p.at(3, 29), 255, 255, 255, 2)); // butt: nothing past the ends
    check!(near(p.at(20, 29), 255, 255, 255, 2));
}

/// The worst difference of a picture's green from what `want` coverage of red over white leaves, and where.
fn worst_from(p: &Pixels, want: &[f32]) -> (i32, i32, i32) {
    let mut worst = (0, 0, 0);
    for y in 0..p.height {
        for x in 0..p.width {
            let d = (p.at(x, y)[1] as i32 - over_white(want[(y * p.width + x) as usize])).abs();
            if d > worst.0 {
                worst = (d, x, y);
            }
        }
    }
    worst
}

/// A stroke covers each pixel as much as what its outline encloses does by the non-zero rule: round a curve, along
/// a gently bending line, its outline doesn't cross itself on the inside of the turns, which would count twice there
/// and darken the inner edge.
pub fn strokes_cover_once(g: &mut Gpu) {
    let ring = oval(xywh(6.0, 6.0, 40.0, 40.0));
    let mut wave = Path::new();
    wave.move_to(52.0, 40.0);
    wave.cubic_to(62.0, 4.0, 76.0, 60.0, 90.0, 12.0);
    let strokes = [(&ring, 5.0), (&wave, 7.0)];
    let p = g.render(96, 52, |c| {
        for (path, width) in strokes {
            let s = stroke(width, StrokeCap::Butt, StrokeJoin::Round, 4.0);
            c.draw_path(path, Offset::Zero, &Brush::from(RED), Rect::Zero, &s);
        }
    });
    let mut outline = Vec::new();
    for (path, width) in strokes {
        let flat = flatten(path, &PathPlacement::default());
        outline.extend(stroke_outline(
            &flat,
            width,
            StrokeCap::Butt,
            StrokeJoin::Round,
            4.0,
            kompoz_gpu::path::TOLERANCE,
        ));
    }
    let (worst, wx, wy) = worst_from(&p, &coverage_of(&outline, p.width, p.height));
    check!(worst <= 3, "  strokes: off by {worst} at ({wx}, {wy})");
    // Sharp turns, thick, beveled: a zigzag covers what its segments' rects and its bevels' triangles do (its inner
    // offsets meet within the segments, each folding back past the other).
    let zig = [(8.0, 46.0), (16.0, 8.0), (22.0, 46.0), (32.0, 8.0), (40.0, 46.0)].map(|(x, y)| Offset::new(x, y));
    let mut zig_path = Path::new();
    zig_path.move_to(zig[0].x, zig[0].y);
    for q in &zig[1..] {
        zig_path.line_to(q.x, q.y);
    }
    let half = 5.0;
    let z = g.render(48, 52, |c| {
        let s = stroke(2.0 * half, StrokeCap::Butt, StrokeJoin::Bevel, 4.0);
        c.draw_path(&zig_path, Offset::Zero, &Brush::from(RED), Rect::Zero, &s);
    });
    let unit = |a: Offset, b: Offset| {
        let l = (b.x - a.x).hypot(b.y - a.y);
        Offset::new((b.x - a.x) / l, (b.y - a.y) / l)
    };
    let mut pieces = Vec::new();
    for i in 0..zig.len() - 1 {
        let d = unit(zig[i], zig[i + 1]);
        let n = Offset::new(d.y * half, -d.x * half);
        pieces.push(Contour { points: vec![zig[i] + n, zig[i + 1] + n, zig[i + 1] - n, zig[i] - n], closed: true });
        if i == 0 {
            continue;
        }
        let into = unit(zig[i - 1], zig[i]);
        // The outside of the turn: to the left of travel when turning right.
        let side = if into.x * d.y - into.y * d.x > 0.0 { 1.0 } else { -1.0 };
        let a = Offset::new(zig[i].x + side * into.y * half, zig[i].y - side * into.x * half);
        let b = Offset::new(zig[i].x + side * d.y * half, zig[i].y - side * d.x * half);
        pieces.push(Contour { points: vec![zig[i], a, b], closed: true });
    }
    let (worst, zx, zy) = worst_from(&z, &coverage_of(&pieces, z.width, z.height));
    check!(worst <= 3, "  sharp turns: off by {worst} at ({zx}, {zy})");
    // Narrow Vs of short legs, either way round, and a thin triangle stroked either way round: there the inner
    // offsets would meet past the segments, so the outline goes through the vertices; every point of it lies in what
    // the stroke covers (within half its width of a segment).
    let within = |points: &[(f32, f32)], closed: bool| {
        let mut pts: Vec<Offset> = points.iter().map(|&(x, y)| Offset::new(x, y)).collect();
        let outline = stroke_outline(
            &[Contour { points: pts.clone(), closed }],
            10.0,
            StrokeCap::Butt,
            StrokeJoin::Bevel,
            4.0,
            kompoz_gpu::path::TOLERANCE,
        );
        if closed {
            pts.push(pts[0]);
        }
        let mut farthest = 0.0f32;
        for q in outline.iter().flat_map(|c| &c.points) {
            let mut nearest = 1e30f32;
            for s in pts.windows(2) {
                let (a, b) = (s[0], s[1]);
                let t = (((q.x - a.x) * (b.x - a.x) + (q.y - a.y) * (b.y - a.y))
                    / ((b.x - a.x) * (b.x - a.x) + (b.y - a.y) * (b.y - a.y)))
                    .clamp(0.0, 1.0);
                nearest = nearest.min((q.x - (a.x + (b.x - a.x) * t)).hypot(q.y - (a.y + (b.y - a.y) * t)));
            }
            farthest = farthest.max(nearest);
        }
        !outline.is_empty() && farthest <= 5.001
    };
    check!(within(&[(17.0, 30.0), (20.0, 8.0), (22.0, 30.0)], false));
    check!(within(&[(22.0, 30.0), (20.0, 8.0), (17.0, 30.0)], false));
    check!(within(&[(17.0, 40.0), (20.0, 6.0), (23.0, 40.0)], true));
    check!(within(&[(23.0, 40.0), (20.0, 6.0), (17.0, 40.0)], true));
}

/// Every pixel of a gradient fill is Brush::color_at at its middle (over white): each kind, each tile mode, hard
/// stops, uneven stops, see-through ones; at a phone's density too (the gradient in dp, drawn in px).
pub fn gradients_match_color_at(g: &mut Gpu) {
    let (a, b, c) = (rgba(0.9, 0.2, 0.1, 1.0), rgba(0.1, 0.3, 0.9, 1.0), rgba(0.1, 0.8, 0.2, 0.4));
    let hard = [(0.0, a), (0.5, a), (0.5, b), (1.0, b)];
    let uneven = [(0.1, a), (0.3, c), (0.9, b)];
    for k in [1.0f32, 2.625] {
        let at = |x: f32, y: f32| Offset::new(x * k, y * k);
        let cases: Vec<(&str, Brush)> = vec![
            ("linear clamp", Brush::linear_gradient(&[a, b], at(0.0, 0.0), at(48.0, 24.0), TileMode::Clamp)),
            ("linear repeated", Brush::linear_gradient(&[a, b], at(10.0, 10.0), at(22.0, 16.0), TileMode::Repeated)),
            ("linear mirror", Brush::linear_gradient(&[a, c, b], at(10.0, 10.0), at(22.0, 16.0), TileMode::Mirror)),
            ("linear decal", Brush::linear_gradient(&[a, b], at(10.0, 10.0), at(30.0, 30.0), TileMode::Decal)),
            ("linear hard stops", Brush::linear_gradient_stops(&hard, at(0.0, 0.0), at(48.0, 0.0), TileMode::Clamp)),
            (
                "linear uneven stops",
                Brush::linear_gradient_stops(&uneven, at(0.0, 48.0), at(48.0, 0.0), TileMode::Clamp),
            ),
            ("radial", Brush::radial_gradient(&[a, c, b], Offset::Unspecified, f32::INFINITY, TileMode::Clamp)),
            ("radial repeated", Brush::radial_gradient(&[a, b], at(12.0, 30.0), 10.0 * k, TileMode::Repeated)),
            ("radial decal", Brush::radial_gradient(&[a, b], at(30.0, 20.0), 14.0 * k, TileMode::Decal)),
            ("sweep", Brush::sweep_gradient(&[a, b, c, a], Offset::Unspecified)),
            ("sweep off center", Brush::sweep_gradient_stops(&uneven, at(10.0, 40.0))),
        ];
        let area = xywh(8.0 * k, 8.0 * k, 48.0 * k, 48.0 * k);
        for (name, brush) in cases {
            let size = (64.0 * k).ceil() as i32;
            let p = g.render(size, size, |gc| gc.fill_gradient(area, CornerRadii::Zero, &brush, area));
            let mut worst = (0, 0, 0);
            let (from, to) = ((9.0 * k).ceil() as i32, (55.0 * k).floor() as i32);
            for y in from..to {
                // Inside the rect, away from its anti-aliased edge.
                for x in from..to {
                    let middle = Offset::new(x as f32 + 0.5, y as f32 + 0.5);
                    // Where the gradient jumps (a hard stop, a repeat starting over) within float error of the
                    // pixel's middle, either side's color is right.
                    let t = brush.parameter_at(middle, area);
                    let (before, after) = (brush.color_at_parameter(t - 1e-4), brush.color_at_parameter(t + 1e-4));
                    let jump = (before.red() - after.red())
                        .abs()
                        .max((before.green() - after.green()).abs())
                        .max((before.blue() - after.blue()).abs())
                        .max((before.alpha() - after.alpha()).abs());
                    if jump > 0.02 {
                        continue;
                    }
                    let want = brush.color_at(middle, area);
                    let over = |v: f32| ((v * want.alpha() + (1.0 - want.alpha())) * 255.0).round() as u8;
                    let expect = [over(want.red()), over(want.green()), over(want.blue()), 255];
                    let d = max_difference(p.at(x, y), expect);
                    if d > worst.0 {
                        worst = (d, x, y);
                    }
                }
            }
            check!(worst.0 <= 2, "  {name} at {k} px per dp: off by {} at {},{}", worst.0, worst.1, worst.2);
        }
    }
}

/// Every blend mode draws what blend() works out, onto a see-through target too, and an edge half a pixel in is half
/// way between what was there and that.
pub fn blend_modes_match_blend(g: &mut Gpu) {
    let pairs = [
        (rgba(0.2, 0.5, 0.9, 1.0), rgba(0.9, 0.4, 0.1, 1.0)), // opaque
        (rgba(0.3, 0.8, 0.2, 0.6), rgba(0.8, 0.1, 0.6, 0.5)), // see-through, both
    ];
    for (m, &mode) in BLEND_MODES.iter().enumerate() {
        for (dst, src) in pairs {
            let p = g.render_cleared(32, 16, ClearColor::TRANSPARENT, |c| {
                c.fill_round_rect(xywh(0.0, 0.0, 32.0, 16.0), CornerRadii::Zero, dst);
                c.push_blend_mode(mode);
                c.fill_round_rect(xywh(8.5, 0.0, 16.0, 16.0), CornerRadii::Zero, src); // (its left edge half a pixel in)
                c.pop_blend_mode();
            });
            // What the target holds where the source didn't go, as the shader reads it back: 8 bits a channel,
            // premultiplied, rounded as this GPU rounds (0.9 * 255 can be 229 or 230).
            let there = p.at(4, 8);
            check!(max_difference(there, premultiplied8(dst)) <= 1);
            let da = there[3] as f32 / 255.0;
            let stored = if da > 0.0 {
                Color::new(there[0] as f32 / 255.0 / da, there[1] as f32 / 255.0 / da, there[2] as f32 / 255.0 / da, da)
            } else {
                Color::Transparent
            };
            let want = premultiplied8(blend(src, stored, mode));
            let mut half = [0u8; 4];
            for k in 0..4 {
                half[k] = ((want[k] as f64 + there[k] as f64) / 2.0).round() as u8;
            }
            if mode == BlendMode::Plus {
                // As Skia (Android's Compose) does Plus: the coverage as the source's alpha, then clamped.
                let s = premultiplied8(src);
                for k in 0..4 {
                    half[k] = 255.min((s[k] as f64 / 2.0).round() as i64 + there[k] as i64) as u8;
                }
            }
            let (inside, edge) = (max_difference(p.at(16, 8), want), max_difference(p.at(8, 8), half));
            check!(
                inside <= 2 && edge <= 3,
                "  blend mode {m} ({mode:?}, alpha {}): off by {inside} inside, {edge} at the edge",
                src.alpha()
            );
        }
    }
}

/// A blend mode that reads the target reads it where the pixel is whatever size the target is: the target's copy is
/// one texture, as big as the biggest target so far.
pub fn target_copies_fit_every_target(g: &mut Gpu) {
    let (under, over) = (rgba(0.2, 0.6, 0.9, 1.0), rgba(0.9, 0.5, 0.2, 1.0));
    let want = blend(over, under, BlendMode::Multiply);
    let b = |v: f32| (v * 255.0).round() as u8;
    for size in [200, 16, 120] {
        // Big first: the copy stays that big.
        let s = size as f32;
        let p = g.render(size, size, |c| {
            c.fill_round_rect(xywh(0.0, 0.0, s, s), CornerRadii::Zero, under);
            c.push_blend_mode(BlendMode::Multiply);
            c.fill_round_rect(xywh(0.0, 0.0, s, s), CornerRadii::Zero, over);
            c.pop_blend_mode();
        });
        for (x, y) in [(1, 1), (size / 2, size / 2), (size - 2, size - 2)] {
            check!(near(p.at(x, y), b(want.red()), b(want.green()), b(want.blue()), 2), "  {size}: {:?}", p.at(x, y));
        }
    }
}

/// Color filters change each pixel as ColorFilter::apply does: an image's, a fill's; nested ones the inner one first.
pub fn color_filters_match_apply(g: &mut Gpu) {
    let mut px = vec![0u8; 16 * 8 * 4];
    for y in 0..8 {
        for x in 0..16 {
            let q = &mut px[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4];
            q.copy_from_slice(&[
                (x * 16) as u8,
                (y * 32) as u8,
                (255 - x * 12) as u8,
                if x < 8 { 255 } else { (96 + y * 16) as u8 },
            ]);
        }
    }
    let image = ImageBitmap::from_pixels(16, 8, px.clone()).expect("a bitmap");
    let mut gray = ColorMatrix::new();
    gray.set_to_saturation(0.0);
    // (Its offset as Android's are, 0..255: C++'s 0.1.)
    let swap = ColorMatrix::from_values([
        0.0, 0.0, 1.0, 0.0, 25.5, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.8, 0.0,
    ]);
    let cases: Vec<(&str, Vec<ColorFilter>)> = vec![
        // (Outermost first.)
        ("gray", vec![ColorFilter::color_matrix(gray)]),
        ("matrix with offsets", vec![ColorFilter::color_matrix(swap)]),
        ("lighting", vec![ColorFilter::lighting(rgba(0.5, 1.0, 0.8, 1.0), rgba(0.2, 0.0, 0.1, 0.0))]),
        ("tint", vec![ColorFilter::tint(rgba(0.1, 0.6, 0.3, 0.8), BlendMode::SrcIn)]),
        ("tint multiply", vec![ColorFilter::tint(rgba(0.9, 0.5, 0.2, 1.0), BlendMode::Multiply)]),
        ("tint hue", vec![ColorFilter::tint(rgba(0.1, 0.3, 0.9, 1.0), BlendMode::Hue)]),
        ("tint dst over", vec![ColorFilter::tint(rgba(1.0, 1.0, 0.0, 0.5), BlendMode::DstOver)]),
        (
            "nested",
            vec![ColorFilter::color_matrix(gray), ColorFilter::tint(rgba(1.0, 0.0, 0.0, 1.0), BlendMode::Screen)],
        ),
    ];
    let fill_color = rgba(0.7, 0.2, 0.4, 0.9);
    for (name, filters) in cases {
        let p = g.render_cleared(16, 16, ClearColor::TRANSPARENT, |c| {
            for f in &filters {
                c.push_color_filter(f);
            }
            c.draw_image(&image, xywh(0.0, 0.0, 16.0, 8.0), xywh(0.0, 0.0, 16.0, 8.0), 1.0, FilterQuality::Low);
            c.fill_round_rect(xywh(0.0, 8.0, 16.0, 8.0), CornerRadii::Zero, fill_color);
            for _ in &filters {
                c.pop_color_filter();
            }
        });
        let filtered = |mut c: Color| {
            for f in filters.iter().rev() {
                c = f.apply(c);
            }
            c
        };
        let mut worst = 0;
        for y in 0..8 {
            for x in 0..16 {
                let q = &px[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4];
                let c = Color::new(q[0] as f32 / 255.0, q[1] as f32 / 255.0, q[2] as f32 / 255.0, q[3] as f32 / 255.0);
                worst = worst.max(max_difference(p.at(x as i32, y as i32), premultiplied8(filtered(c))));
            }
        }
        worst = worst.max(max_difference(p.at(8, 12), premultiplied8(filtered(fill_color))));
        check!(worst <= 2, "  {name}: off by {worst}");
    }
}

/// Stretched more one way than the other, edges are still covered exactly (each by the pixel's size across it);
/// turned or seen in perspective, a shape covers as much as its area; a rect clipped to a turned one is that one; a
/// shape partly behind the eye shows what is in front.
pub fn transformed_edges_are_exact(g: &mut Gpu) {
    let stretched = g.render(32, 32, |c| {
        let mut m = Matrix::new();
        m.scale(4.0, 0.5, 1.0);
        c.push_transform(&m);
        c.fill_round_rect(xywh(1.1, 2.3, 1.9, 40.0), CornerRadii::Zero, RED); // x 4.4 to 12, y 1.15 to 21.15
        c.pop_transform();
    });
    check!(near(stretched.at(4, 10), 255, 102, 102, 2)); // 0.6 of it
    check!(near(stretched.at(8, 10), 255, 0, 0, 2));
    check!(near(stretched.at(12, 10), 255, 255, 255, 2));
    check!(near(stretched.at(8, 1), 255, 38, 38, 2)); // 0.85
    check!(near(stretched.at(8, 21), 255, 217, 217, 2)); // 0.15
    // Shrunk to a quarter down: the quad still reaches a pixel past the edge (y 1.9: 0.1 of the pixel above it).
    let shrunk = g.render(16, 16, |c| {
        let mut m = Matrix::new();
        m.scale(1.0, 0.25, 1.0);
        c.push_transform(&m);
        c.fill_round_rect(xywh(2.0, 7.6, 10.0, 40.0), CornerRadii::Zero, RED);
        c.pop_transform();
    });
    check!(near(shrunk.at(6, 1), 255, 230, 230, 2));
    // An arc and a line stretched: each pixel covered within 5 / 255 of what 64 x 64 samples of the shapes through
    // the transform say (the most at the ring's ends, which bend round in 3 px), but for the line's corners, no longer
    // square on the screen, where its two edges' coverages multiplied are only near.
    let stretch = about(32.0, 32.0, |m| m.scale(2.0, 0.75, 1.0));
    let shapes = g.render(64, 64, |c| {
        c.push_transform(&stretch);
        c.draw_arc(xywh(20.0, 18.0, 24.0, 24.0), 0.0, 360.0, 4.0, RED, false); // a ring 10 to 14 from (32, 30)
        c.draw_line(Offset::new(18.0, 50.0), Offset::new(46.0, 56.0), 3.0, RED); // square caps: 1.5 past each end
        c.pop_transform();
    });
    let mut back = stretch;
    back.invert();
    let line_length = 28.0f32.hypot(6.0);
    let inside = |x: f32, y: f32| {
        let p = back.map(Offset::new(x, y));
        let r = (p.x - 32.0).hypot(p.y - 30.0);
        let along = ((p.x - 18.0) * 28.0 + (p.y - 50.0) * 6.0) / line_length;
        let across = ((p.y - 50.0) * 28.0 - (p.x - 18.0) * 6.0) / line_length;
        (10.0..=14.0).contains(&r) || ((-1.5..=line_length + 1.5).contains(&along) && across.abs() <= 1.5)
    };
    let (ux, uy) = (28.0 / line_length * 1.5, 6.0 / line_length * 1.5); // along it, half its width
    let line_corners = vec![
        stretch.map(Offset::new(18.0 - ux + uy, 50.0 - uy - ux)),
        stretch.map(Offset::new(46.0 + ux + uy, 56.0 + uy - ux)),
        stretch.map(Offset::new(46.0 + ux - uy, 56.0 + uy + ux)),
        stretch.map(Offset::new(18.0 - ux - uy, 50.0 - uy + ux)),
    ];
    let skip = around(line_corners);
    let mut worst_shape = (0, 0, 0);
    for y in 0..64 {
        for x in 0..64 {
            if skip(x, y) {
                continue;
            }
            let mut count = 0;
            for j in 0..64 {
                for i in 0..64 {
                    count += inside(x as f32 + (i as f32 + 0.5) / 64.0, y as f32 + (j as f32 + 0.5) / 64.0) as i32;
                }
            }
            let want = (255.0 * (1.0 - count as f32 / 4096.0)).round() as i32;
            let d = (shapes.at(x, y)[1] as i32 - want).abs();
            if d > worst_shape.0 {
                worst_shape = (d, x, y);
            }
        }
    }
    check!(
        worst_shape.0 <= 5,
        "  stretched arc and line: off by {} at ({}, {})",
        worst_shape.0,
        worst_shape.1,
        worst_shape.2
    );
    let turn = about(32.0, 32.0, |m| m.rotate_z(30.0));
    let turned = g.render(64, 64, |c| {
        c.push_transform(&turn);
        c.fill_round_rect(xywh(20.0, 26.0, 24.0, 12.0), CornerRadii::Zero, RED);
        c.pop_transform();
    });
    check!((covered_area(&turned) - 288.0).abs() < 1.5, "  {}", covered_area(&turned));
    // An image turned: its edges anti-aliased as the rect's are.
    let solid = ImageBitmap::from_pixels(4, 4, vec![255; 64]).expect("a bitmap"); // white
    let black = ClearColor::new(0.0, 0.0, 0.0, 1.0);
    let turned_image = g.render_cleared(64, 64, black, |c| {
        c.push_transform(&turn);
        c.draw_image(&solid, xywh(0.0, 0.0, 4.0, 4.0), xywh(20.0, 26.0, 24.0, 12.0), 1.0, FilterQuality::Low);
        c.pop_transform();
    });
    let turned_box = g.render_cleared(64, 64, black, |c| {
        c.push_transform(&turn);
        c.fill_round_rect(xywh(20.0, 26.0, 24.0, 12.0), CornerRadii::Zero, Color::White);
        c.pop_transform();
    });
    let (worst, at) = worst_against(&turned_box, |x, y| turned_image.at(x, y), none);
    check!(worst <= 3, "  turned image: off by {worst} at {at:?}");
    // In perspective: as much as the quad its corners go to (by the shoelace formula).
    let mut seen = Matrix::new();
    seen.set(0, 3, 0.004); // w = 1 + 0.004 x
    seen.set(1, 3, 0.002);
    let r = xywh(8.0, 10.0, 40.0, 36.0);
    let far = g.render(64, 64, |c| {
        c.push_transform(&seen);
        c.fill_round_rect(r, CornerRadii::Zero, RED);
        c.pop_transform();
    });
    let q = [r.top_left(), r.top_right(), r.bottom_right(), r.bottom_left()].map(|p| seen.map(p));
    let area = (0..4).map(|i| q[i].x * q[(i + 1) % 4].y - q[(i + 1) % 4].x * q[i].y).sum::<f32>().abs() / 2.0;
    check!((covered_area(&far) - area).abs() < area * 0.005 + 1.0, "  {} of {area}", covered_area(&far));
    // A path through the same transform: projected on the CPU, covered exactly. The same pixels, but at the corners,
    // which perspective has made other than square (there the two edges' coverages multiplied are only near).
    let far_path = g.render(64, 64, |c| {
        let mut p = Path::new();
        p.add_rect(r, Default::default());
        c.push_transform(&seen);
        c.draw_path(&p, Offset::Zero, &Brush::from(RED), Rect::Zero, &fill());
        c.pop_transform();
    });
    let (worst, at) = worst_against(&far, |x, y| far_path.at(x, y), around(q.to_vec()));
    check!(worst <= 3, "  a path in perspective: off by {worst} at {at:?}");
    // Close to the horizon of a perspective along x (w = 1 + x / 50), where it changes fast, an edge up and down
    // stays so: each pixel it crosses is covered by the distance to it over its derivative exactly at the pixel's
    // middle (in the shape's own units), each side of a thin bar on its own, as worked out here.
    let bars = g.render(64, 16, |c| {
        let mut deep = Matrix::new();
        deep.set(0, 3, 0.02);
        c.push_transform(&deep);
        for x0 in [120.0, 170.0, 230.0] {
            c.fill_round_rect(xywh(x0, -100.0, 12.0, 500.0), CornerRadii::Zero, RED); // 1 to 0.3 px wide at 35 to 41
        }
        c.pop_transform();
    });
    let mut worst_bar = 0;
    for x in 20..42 {
        let px = x as f32 + 0.5;
        let local = px / (1.0 - px / 50.0);
        let along = 1.0 / ((1.0 - px / 50.0) * (1.0 - px / 50.0));
        let mut covered = 0.0f32;
        for x0 in [120.0f32, 170.0, 230.0] {
            covered += ((0.5 - (x0 - local) / along).clamp(0.0, 1.0)
                + (0.5 - (local - x0 - 12.0) / along).clamp(0.0, 1.0)
                - 1.0)
                .max(0.0);
        }
        worst_bar = worst_bar.max((bars.at(x, 8)[1] as i32 - over_white(covered)).abs());
    }
    check!(worst_bar <= 3, "  edges near the horizon: off by {worst_bar}");
    // Clipped to a turned rect, turned or in perspective: the rect itself.
    for m in [turn, seen] {
        let filled = g.render(64, 64, |c| {
            c.push_transform(&m);
            c.fill_round_rect(xywh(14.0, 12.0, 30.0, 22.0), CornerRadii::Zero, RED);
            c.pop_transform();
        });
        let clipped = g.render(64, 64, |c| {
            c.push_transform(&m);
            c.push_clip(xywh(14.0, 12.0, 30.0, 22.0));
            c.fill_round_rect(xywh(-20.0, -20.0, 100.0, 100.0), CornerRadii::Zero, RED);
            c.pop_clip();
            c.pop_transform();
        });
        let (worst, at) = worst_against(&filled, |x, y| clipped.at(x, y), none);
        check!(worst <= 3, "  clipped to a transformed rect: off by {worst} at {at:?}");
    }
    // Partly behind the eye (w = 1 + 0.02 x is below 0 left of x = -50): what is in front shows, as the path cut
    // there shows it, and nothing else.
    let mut behind = Matrix::new();
    behind.set(0, 3, 0.02);
    let wide = xywh(-100.0, 8.0, 140.0, 40.0);
    let front = g.render(64, 64, |c| {
        c.push_transform(&behind);
        c.fill_round_rect(wide, CornerRadii::Zero, RED);
        c.pop_transform();
    });
    let front_path = g.render(64, 64, |c| {
        let mut p = Path::new();
        p.add_rect(wide, Default::default());
        c.push_transform(&behind);
        c.draw_path(&p, Offset::Zero, &Brush::from(RED), Rect::Zero, &fill());
        c.pop_transform();
    });
    let front_corners = vec![behind.map(wide.top_right()), behind.map(wide.bottom_right())];
    let (worst, at) = worst_against(&front, |x, y| front_path.at(x, y), around(front_corners));
    check!(worst <= 3, "  partly behind the eye: off by {worst} at {at:?}");
    check!(near(front.at(10, 20), 255, 0, 0, 2)); // x 10 / 1.2 = 8.3: in front, drawn
    check!(near(front.at(30, 20), 255, 255, 255, 2)); // past its right edge (40 / 1.8 = 22.2)
    // Turned into view, what was out of it isn't skipped.
    let (mut skipped, mut skipped_plain) = (true, false);
    g.render(96, 48, |c| {
        skipped_plain = c.quick_reject(xywh(60.0, 60.0, 4.0, 4.0)); // below the target
        c.push_transform(&about(48.0, 24.0, |m| m.rotate_z(90.0)));
        skipped = c.quick_reject(xywh(60.0, 60.0, 4.0, 4.0)); // turned to (10, 38)
        c.pop_transform();
    });
    check!(skipped_plain && !skipped);
}

/// Two drawings the same, within 2 of each other.
fn same(g: &mut Gpu, what: &str, size: i32, a: &dyn Fn(&mut Renderer), b: &dyn Fn(&mut Renderer)) {
    let pa = g.render(size, size, a);
    let pb = g.render(size, size, b);
    let (worst, at) = worst_against(&pa, |x, y| pb.at(x, y), none);
    check!(worst <= 2, "  {what}: off by {worst} at {at:?}");
}

/// Clips of any shape cut what is drawn in them as that shape covers: a fill clipped to a rounded rect or a path
/// (either fill rule) is the shape filled; clipped to what is outside it, the fill with the shape over it in the
/// background's color; clips inside clips multiply; a path clip turned is the path turned; and the paths of many
/// clips that fill the coverage mask more than once (a new generation of it) are each still where they are.
pub fn clips_cut_as_their_shapes(g: &mut Gpu) {
    let red = rgba(0.9, 0.1, 0.1, 1.0);
    let (all, bx) = (xywh(-10.0, -10.0, 100.0, 100.0), xywh(10.3, 12.6, 44.0, 30.0));
    let radii = CornerRadii::new(12.0, 3.0, 0.0, 20.0);
    let star1 = star(36.0, 36.0, 30.0);
    let mut star2 = star(36.0, 36.0, 30.0);
    star2.fill_type = PathFillType::EvenOdd;
    let none_area = Rect::Zero;
    same(
        g,
        "rounded rect",
        72,
        &|c| {
            c.push_clip_round_rect(bx, radii, ClipOp::Intersect);
            c.fill_round_rect(all, CornerRadii::Zero, red);
            c.pop_clip();
        },
        &|c| c.fill_round_rect(bx, radii, red),
    );
    same(
        g,
        "outside a rounded rect",
        72,
        &|c| {
            c.push_clip_round_rect(bx, radii, ClipOp::Difference);
            c.fill_round_rect(all, CornerRadii::Zero, red);
            c.pop_clip();
        },
        &|c| {
            c.fill_round_rect(all, CornerRadii::Zero, red);
            c.fill_round_rect(bx, radii, Color::White);
        },
    );
    for p in [&star1, &star2] {
        same(
            g,
            "path",
            72,
            &|c| {
                c.push_clip_path(p, Offset::Zero, ClipOp::Intersect);
                c.fill_round_rect(all, CornerRadii::Zero, red);
                c.pop_clip();
            },
            &|c| c.draw_path(p, Offset::Zero, &Brush::from(red), none_area, &fill()),
        );
        same(
            g,
            "outside a path",
            72,
            &|c| {
                c.push_clip_path(p, Offset::new(2.0, 1.0), ClipOp::Difference);
                c.fill_round_rect(all, CornerRadii::Zero, red);
                c.pop_clip();
            },
            &|c| {
                c.fill_round_rect(all, CornerRadii::Zero, red);
                c.draw_path(p, Offset::new(2.0, 1.0), &Brush::from(Color::White), none_area, &fill());
            },
        );
    }
    // A shape that isn't star-shaped about its middle (a C), as a clip and as a layer's outline: the path filled.
    let cee = Path::combine(
        PathOperation::Difference,
        &oval(xywh(6.0, 6.0, 60.0, 60.0)),
        &oval(xywh(22.0, 18.0, 52.0, 36.0)),
    );
    let built = cee.clone();
    let cee_shape = GenericShape(move |p, _, _| p.add_path(&built, Offset::Zero));
    let density = Density::default();
    same(
        g,
        "C clip",
        72,
        &|c| {
            c.push_clip_shape(
                xywh(0.0, 0.0, 72.0, 72.0),
                &cee_shape,
                LayoutDirection::Ltr,
                &density,
                ClipOp::Intersect,
            );
            c.fill_round_rect(all, CornerRadii::Zero, red);
            c.pop_clip();
        },
        &|c| c.draw_path(&cee, Offset::Zero, &Brush::from(red), none_area, &fill()),
    );
    same(
        g,
        "C layer",
        72,
        &|c| {
            let at = xywh(0.0, 0.0, 72.0, 72.0);
            c.push_shape_layer(at, &cee_shape, LayoutDirection::Ltr, &density, 1.0, Default::default());
            c.fill_round_rect(all, CornerRadii::Zero, red);
            c.pop_layer();
        },
        &|c| c.draw_path(&cee, Offset::Zero, &Brush::from(red), none_area, &fill()),
    );
    // Clear of a rounded clip's corners and inside its edges, a primitive is drawn as it is (the clip left out of
    // it: clear_of_clips), to the last bit; reaching a corner, it is cut.
    let (roomy, inner) = (xywh(8.0, 8.0, 56.0, 56.0), xywh(12.0, 20.0, 40.0, 30.0));
    let clear = g.render(72, 72, |c| {
        c.push_clip_round_rect(roomy, CornerRadii::all(10.0), ClipOp::Intersect);
        c.fill_round_rect(inner, CornerRadii::all(4.0), red);
        c.pop_clip();
    });
    let unclipped = g.render(72, 72, |c| c.fill_round_rect(inner, CornerRadii::all(4.0), red));
    check!(worst_against(&clear, |x, y| unclipped.at(x, y), none).0 == 0);
    let cornered = g.render(72, 72, |c| {
        c.push_clip_round_rect(roomy, CornerRadii::all(10.0), ClipOp::Intersect);
        c.fill_round_rect(xywh(6.0, 6.0, 30.0, 30.0), CornerRadii::Zero, red);
        c.pop_clip();
    });
    check!(cornered.at(8, 8)[1] > 200 && cornered.at(20, 20)[1] < 60); // (out past the corner's circle; in)
    // Inside each other: the coverages multiply.
    let a = g.render(72, 72, |c| c.draw_path(&star1, Offset::Zero, &Brush::from(red), none_area, &fill()));
    let b = g.render(72, 72, |c| c.fill_round_rect(bx, radii, red));
    let both = g.render(72, 72, |c| {
        c.push_clip_path(&star1, Offset::Zero, ClipOp::Intersect);
        c.push_clip_round_rect(bx, radii, ClipOp::Intersect);
        c.fill_round_rect(all, CornerRadii::Zero, red);
        c.pop_clip();
        c.pop_clip();
    });
    let mut worst = 0;
    for y in 0..72 {
        for x in 0..72 {
            let ca = (255 - a.at(x, y)[1] as i32) as f32 / (255.0 - 25.5);
            let cb = (255 - b.at(x, y)[1] as i32) as f32 / (255.0 - 25.5);
            let want = (255.0 - ca * cb * (255.0 - 25.5)).round() as i32;
            worst = worst.max((both.at(x, y)[1] as i32 - want).abs());
        }
    }
    check!(worst <= 2, "  clips inside clips: off by {worst}");
    // Turned: the path turned.
    let turn = about(36.0, 36.0, |m| {
        m.rotate_z(25.0);
        m.scale(1.2, 0.8, 1.0);
    });
    same(
        g,
        "turned path",
        72,
        &|c| {
            c.push_transform(&turn);
            c.push_clip_path(&star1, Offset::Zero, ClipOp::Intersect);
            c.fill_round_rect(all, CornerRadii::Zero, red);
            c.pop_clip();
            c.pop_transform();
        },
        &|c| {
            c.push_transform(&turn);
            c.draw_path(&star1, Offset::Zero, &Brush::from(red), none_area, &fill());
            c.pop_transform();
        },
    );
    // Thirty clips each a disc 500 px across, a disc 5 px bigger filled in each (its own coverage in the same
    // generation as the clip's): two generations of the coverage mask (2048 px square: sixteen such discs), each as
    // the disc drawn.
    let discs = |clipped: bool| {
        move |c: &mut Renderer| {
            // (One more first, so that a generation fills up as a clip's path goes in.)
            let first = oval(xywh(400.0, 400.0, 500.0, 500.0));
            c.draw_path(&first, Offset::Zero, &Brush::from(rgba(0.5, 0.5, 0.5, 1.0)), Rect::Zero, &fill());
            for i in 0..30 {
                let (x, y) = ((i % 6) as f32 * 90.0, (i / 6) as f32 * 90.0);
                let disc = oval(xywh(x, y, 500.0, 500.0));
                let color = Brush::from(rgba(i as f32 / 30.0, 0.5, 1.0 - i as f32 / 30.0, 1.0));
                if clipped {
                    let bigger = oval(xywh(x - 5.0, y - 5.0, 510.0, 510.0));
                    c.push_clip_path(&disc, Offset::Zero, ClipOp::Intersect);
                    c.draw_path(&bigger, Offset::Zero, &color, Rect::Zero, &fill());
                    c.pop_clip();
                } else {
                    c.draw_path(&disc, Offset::Zero, &color, Rect::Zero, &fill());
                }
            }
        }
    };
    same(g, "many clips", 960, &discs(true), &discs(false));
}

/// A circle of conics (a path's oval, through a perspective too) covers what the rounded rect of it does, at a
/// phone's density.
pub fn conics_are_exact(g: &mut Gpu) {
    let (n, k) = (96, 2.625f32);
    let ink = rgba(0.2, 0.3, 0.7, 1.0);
    for seen in [false, true] {
        // (C++'s perspective in dp: here in px, the same.)
        let mut m = Matrix::new();
        if seen {
            m.set(0, 3, 0.004 / k); // further to the right
        }
        let circle = g.render(n, n, |c| {
            c.push_transform(&m);
            c.fill_round_rect(xywh(4.0 * k, 4.0 * k, 28.0 * k, 28.0 * k), CornerRadii::all(14.0 * k), ink);
            c.pop_transform();
        });
        let mut round = oval(xywh(4.0 * k, 4.0 * k, 28.0 * k, 28.0 * k));
        round.transform(&m); // (the path itself through the map: conics in perspective)
        let path = g.render(n, n, |c| c.draw_path(&round, Offset::Zero, &Brush::from(ink), Rect::Zero, &fill()));
        let (worst, at) = worst_against(&path, |x, y| circle.at(x, y), none);
        check!(
            worst <= 60,
            "  a circle of conics{}: off by {worst} at {at:?}",
            if seen { " in perspective" } else { "" }
        );
    }
}

/// A path effect applies where the path is drawn: a dash in its own units, at the pixels it is drawn at (Skia's
/// resScale, the transform's scale: a circle dashed finely, as it is seen).
pub fn path_effects_apply_where_drawn(g: &mut Gpu) {
    let k = 2.0;
    let mut line = Path::new();
    line.move_to(2.0 * k, 5.0 * k);
    line.line_to(42.0 * k, 5.0 * k);
    let dashed = g.render(100, 20, |c| {
        let s = PathStroke {
            width: 4.0 * k,
            path_effect: Some(PathEffect::dash_path_effect(&[10.0 * k, 10.0 * k], 0.0)),
            ..PathStroke::default()
        };
        c.draw_path(&line, Offset::Zero, &Brush::from(Color::Black), Rect::Zero, &s);
    });
    check!(
        near(dashed.at(14, 10), 0, 0, 0, 2)
            && near(dashed.at(34, 10), 255, 255, 255, 2)
            && near(dashed.at(54, 10), 0, 0, 0, 2)
            && near(dashed.at(74, 10), 255, 255, 255, 2)
    );
    let mut circle = Path::new();
    circle.add_oval(xywh(1.0, 1.0, 10.0, 10.0), kompoz_graphics::PathDirection::Clockwise);
    let dash = PathEffect::dash_path_effect(&[1.0, 1.0], 0.0);
    // The angles (degrees, 0..360 from the right, clockwise) each dash covers at a scale.
    let covered = |res_scale: f32| {
        let mut out = Path::new();
        let mut s = PathStroke { width: 0.5, ..PathStroke::default() };
        dash.filter_path(&mut out, &circle, &mut s, res_scale);
        let mut on = vec![false; 360];
        let mut m = PathMeasure::new();
        m.set_path_at_res_scale(&out, false, 64.0);
        loop {
            let mut d = 0.0;
            while d <= m.length() {
                let p = m.get_position(d);
                let a = (p.y - 6.0).atan2(p.x - 6.0).to_degrees();
                on[(if a < 0.0 { a + 360.0 } else { a }).floor() as usize % 360] = true;
                d += 0.01;
            }
            if !m.next_contour() {
                break;
            }
        }
        on
    };
    let (fine, coarse) = (covered(8.0), covered(1.0));
    // A degree well inside one of fine's dashes and coarse's gaps (its neighbours too), and the other way round.
    let (mut in_fine, mut in_coarse) = (None, None);
    for a in 2..358 {
        let f = fine[a - 2] && fine[a] && fine[a + 2];
        let c = coarse[a - 2] || coarse[a] || coarse[a + 2];
        let f2 = fine[a - 2] || fine[a] || fine[a + 2];
        let c2 = coarse[a - 2] && coarse[a] && coarse[a + 2];
        if in_fine.is_none() && f && !c {
            in_fine = Some(a);
        }
        if in_coarse.is_none() && c2 && !f2 {
            in_coarse = Some(a);
        }
    }
    check!(in_fine.is_some() && in_coarse.is_some());
    let (Some(in_fine), Some(in_coarse)) = (in_fine, in_coarse) else { return };
    // Drawn 8 times bigger (C++'s 8 px per dp): through a transform, which is what the dash is cut at.
    let drawn = g.render(100, 100, |c| {
        let mut m = Matrix::new();
        m.scale(8.0, 8.0, 1.0);
        c.push_transform(&m);
        let s = PathStroke { width: 0.5, path_effect: Some(dash.clone()), ..PathStroke::default() };
        c.draw_path(&circle, Offset::Zero, &Brush::from(Color::Black), Rect::Zero, &s);
        c.pop_transform();
    });
    let pixel_at = |degrees: usize| {
        let a = (degrees + 1) as f32 * std::f32::consts::PI / 180.0 - std::f32::consts::PI / 360.0; // (the degree's middle)
        drawn.at(((6.0 + 5.0 * a.cos()) * 8.0) as i32, ((6.0 + 5.0 * a.sin()) * 8.0) as i32)
    };
    check!(
        pixel_at(in_fine)[0] < 100 && pixel_at(in_coarse)[0] > 200,
        "  {:?} {:?}",
        pixel_at(in_fine),
        pixel_at(in_coarse)
    );
}

/// Not anti-aliased, each pixel is all in or all out, as Skia decides it: its middle inside the shape (rects
/// between pixels, on their middles, at 2 px a dp; a circle, a ring, a thick line, a path by even-odd, a turned
/// rect); a hairline a pixel each step along its longer axis, wherever it runs, and turning back over itself.
pub fn aliased_drawing_is_skias(g: &mut Gpu) {
    let n = 64;
    let red = Paint { color: rgba(0.8, 0.1, 0.2, 1.0), is_anti_alias: false, ..Paint::default() };
    let stroked = Paint { style: PaintingStyle::Stroke, ..red.clone() };
    let rect_rule = |l: f32, t: f32, r: f32, b: f32| move |x: f32, y: f32| (l < x && x <= r && t < y && y <= b) as i32;
    // Inside where negative; too near to say where within a thousandth.
    let by = |distance: Box<dyn Fn(f32, f32) -> f32>| {
        move |x: f32, y: f32| {
            let d = distance(x, y);
            if d.abs() < 1e-3 { -1 } else { (d < 0.0) as i32 }
        }
    };
    // A point's distance from a segment's side: positive to its right (y down).
    let side = |a: Offset, b: Offset, x: f32, y: f32| {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        ((x - a.x) * dy - (y - a.y) * dx) / dx.hypot(dy)
    };
    type Draw<'a> = Box<dyn Fn(&mut Canvas) + 'a>;
    type Inside<'a> = Box<dyn Fn(f32, f32) -> i32 + 'a>;
    let cases: Vec<(&str, Draw, Inside)> = vec![
        (
            "a rect between pixels",
            Box::new(|c| c.draw_rect(xywh(3.3, 4.7, 20.2, 10.5), &red)),
            Box::new(rect_rule(3.3, 4.7, 23.5, 15.2)),
        ),
        (
            "a rect on pixel middles",
            Box::new(|c| c.draw_rect(xywh(2.5, 6.5, 8.0, 8.0), &red)),
            Box::new(rect_rule(2.5, 6.5, 10.5, 14.5)),
        ),
        (
            "a rect at 2 px a dp",
            Box::new(|c| c.draw_rect(xywh(1.25 * 2.0, 2.25 * 2.0, 10.0 * 2.0, 5.0 * 2.0), &red)),
            Box::new(rect_rule(2.5, 4.5, 22.5, 14.5)),
        ),
        (
            "a circle",
            Box::new(|c| c.draw_circle(Offset::new(30.3, 29.6), 17.7, &red)),
            Box::new(by(Box::new(|x, y| (x - 30.3).hypot(y - 29.6) - 17.7))),
        ),
        (
            "a ring",
            Box::new(|c| c.draw_circle(Offset::new(32.2, 31.1), 20.3, &Paint { stroke_width: 4.4, ..stroked.clone() })),
            Box::new(by(Box::new(|x, y| ((x - 32.2).hypot(y - 31.1) - 20.3).abs() - 2.2))),
        ),
        (
            "a thick line",
            Box::new(|c| {
                c.draw_line(
                    Offset::new(6.2, 50.3),
                    Offset::new(57.1, 13.4),
                    &Paint { stroke_width: 5.3, ..red.clone() },
                )
            }),
            Box::new(by(Box::new(|x, y| {
                let (a, b) = (Offset::new(6.2, 50.3), Offset::new(57.1, 13.4));
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let len = dx.hypot(dy);
                let (ux, uy) = (dx / len, dy / len);
                let along = (x - a.x) * ux + (y - a.y) * uy;
                let across = (x - a.x) * -uy + (y - a.y) * ux;
                (across.abs() - 2.65).max((-along).max(along - len))
            }))),
        ),
        (
            "a path (even-odd)",
            Box::new(|c| {
                let mut p = Path::new();
                p.fill_type = PathFillType::EvenOdd;
                p.move_to(5.3, 5.6);
                p.line_to(58.2, 9.1);
                p.line_to(30.4, 58.7);
                p.close();
                p.move_to(20.1, 15.3);
                p.line_to(40.6, 16.2);
                p.line_to(30.2, 35.4);
                p.close();
                c.draw_path(&p, &red);
            }),
            Box::new(move |x, y| {
                let tri = |a: (f32, f32), b: (f32, f32), d: (f32, f32)| {
                    let (a, b, d) = (Offset::new(a.0, a.1), Offset::new(b.0, b.1), Offset::new(d.0, d.1));
                    let (s0, s1, s2) = (side(a, b, x, y), side(b, d, x, y), side(d, a, x, y));
                    if s0.abs().min(s1.abs()).min(s2.abs()) < 1e-3 {
                        return -1;
                    }
                    ((s0 > 0.0 && s1 > 0.0 && s2 > 0.0) || (s0 < 0.0 && s1 < 0.0 && s2 < 0.0)) as i32
                };
                let outer = tri((5.3, 5.6), (58.2, 9.1), (30.4, 58.7));
                let inner = tri((20.1, 15.3), (40.6, 16.2), (30.2, 35.4));
                if outer < 0 || inner < 0 { -1 } else { outer ^ inner }
            }),
        ),
        (
            "a turned rect",
            Box::new(|c| {
                c.rotate_about(30.0, 32.0, 32.0);
                c.draw_rect(xywh(14.3, 22.6, 35.2, 18.1), &red);
            }),
            Box::new(by(Box::new(|x, y| {
                let a = -30.0f32.to_radians();
                let (cs, sn) = (a.cos(), a.sin());
                let u = 32.0 + (x - 32.0) * cs - (y - 32.0) * sn;
                let v = 32.0 + (x - 32.0) * sn + (y - 32.0) * cs;
                (14.3 - u).max(u - 49.5).max((22.6 - v).max(v - 40.7))
            }))),
        ),
    ];
    let painted = |q: [u8; 4]| near(q, 204, 26, 51, 1);
    let white = |q: [u8; 4]| q[0] > 250 && q[1] > 250 && q[2] > 250;
    for (what, draw, inside) in &cases {
        let got = g.render(n, n, |r| {
            let mut canvas = Canvas::new(r, Offset::Zero);
            draw(&mut canvas);
        });
        let (mut partial, mut wrong, mut at) = (0, 0, (0, 0));
        for y in 0..n {
            for x in 0..n {
                let q = got.at(x, y);
                if !white(q) && !painted(q) {
                    partial += 1;
                    at = (x, y);
                }
                let want = inside(x as f32 + 0.5, y as f32 + 0.5);
                if want >= 0 && (want == 1) != painted(q) && (white(q) || painted(q)) {
                    wrong += 1;
                    at = (x, y);
                }
            }
        }
        check!(partial == 0 && wrong == 0, "  {what}: {partial} pixels partly painted, {wrong} wrong (one at {at:?})");
    }
    // Hairlines: a pixel each step along the longer axis, wherever the line runs.
    for end in [Offset::new(58.6, 21.3), Offset::new(17.2, 60.1), Offset::new(60.4, 60.4)] {
        let start = Offset::new(3.4, 4.6);
        let got = g.render(n, n, |r| {
            let mut canvas = Canvas::new(r, Offset::Zero);
            canvas.draw_line(
                start,
                end,
                &Paint { color: rgba(0.8, 0.1, 0.2, 1.0), is_anti_alias: false, ..Paint::default() },
            );
        });
        let across = (end.x - start.x).abs() >= (end.y - start.y).abs();
        let (mut bad, mut partial) = (0, 0);
        for i in 0..n {
            let mut count = 0;
            for j in 0..n {
                let q = if across { got.at(i, j) } else { got.at(j, i) };
                if !white(q) && !painted(q) {
                    partial += 1;
                }
                count += painted(q) as i32;
            }
            let m = i as f32 + 0.5;
            let (lo, hi) = if across {
                (start.x.min(end.x), start.x.max(end.x))
            } else {
                (start.y.min(end.y), start.y.max(end.y))
            };
            let want = if m > lo && m <= hi {
                1
            } else if m < lo - 0.01 || m > hi + 0.01 {
                0
            } else {
                -1
            };
            if want >= 0 && count != want {
                bad += 1;
            }
        }
        check!(
            bad == 0 && partial == 0,
            "  a hairline to ({}, {}): {bad} steps without one pixel, {partial} pixels partly painted",
            end.x,
            end.y
        );
    }
    // A hairline that turns back over itself: its two segments' pixels together (no gap where they overlap).
    let back = g.render(n, n, |r| {
        let mut canvas = Canvas::new(r, Offset::Zero);
        let mut p = Path::new();
        p.move_to(4.3, 30.2);
        p.line_to(60.1, 31.4);
        p.line_to(4.6, 32.7);
        let hair = Paint {
            style: PaintingStyle::Stroke,
            ..Paint { color: rgba(0.8, 0.1, 0.2, 1.0), is_anti_alias: false, ..Paint::default() }
        };
        canvas.draw_path(&p, &hair);
    });
    let gaps = (5..59).filter(|&x| !(0..n).any(|y| painted(back.at(x, y)))).count();
    check!(gaps == 0, "  a hairline turning back: {gaps} columns empty");
}
