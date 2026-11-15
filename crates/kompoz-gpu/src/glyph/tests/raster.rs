//! The coverage rasterizer against shapes whose areas are known.

use crate::glyph::raster::{FillRule, FixedMatrix, FixedOutline, FixedPoint, FixedSink, PointTag, mul_fix};

fn p(x: f32, y: f32) -> FixedPoint {
    FixedPoint::from_px(x, y)
}

/// A polygon (px, y up) as an outline.
fn polygon(points: &[(f32, f32)]) -> FixedOutline {
    let mut o = FixedOutline::new();
    o.add_polygon(points.iter().map(|&(x, y)| p(x, y)));
    o
}

/// The rectangle x0..x1 × y0..y1 (px, y up), clockwise or not.
fn rect(x0: f32, y0: f32, x1: f32, y1: f32, clockwise: bool) -> Vec<(f32, f32)> {
    if clockwise { vec![(x0, y0), (x0, y1), (x1, y1), (x1, y0)] } else { vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)] }
}

/// The area of the convex polygon `poly` inside the unit pixel at (x, y) (y up), by clipping it to the pixel.
fn pixel_area(poly: &[(f64, f64)], x: f64, y: f64) -> f64 {
    let mut pts: Vec<(f64, f64)> = poly.to_vec();
    let edges: [(f64, f64, f64); 4] = [(1.0, 0.0, -x), (-1.0, 0.0, x + 1.0), (0.0, 1.0, -y), (0.0, -1.0, y + 1.0)];
    for (a, b, c) in edges {
        let inside = |q: (f64, f64)| a * q.0 + b * q.1 + c >= 0.0;
        let mut out = Vec::new();
        for i in 0..pts.len() {
            let (s, e) = (pts[i], pts[(i + 1) % pts.len()]);
            let (si, ei) = (inside(s), inside(e));
            if si {
                out.push(s);
            }
            if si != ei {
                let ds = a * s.0 + b * s.1 + c;
                let de = a * e.0 + b * e.1 + c;
                let t = ds / (ds - de);
                out.push((s.0 + (e.0 - s.0) * t, s.1 + (e.1 - s.1) * t));
            }
        }
        pts = out;
        if pts.is_empty() {
            return 0.0;
        }
    }
    let n = pts.len();
    ((0..n).map(|i| pts[i].0 * pts[(i + 1) % n].1 - pts[(i + 1) % n].0 * pts[i].1).sum::<f64>() / 2.0).abs()
}

#[test]
fn whole_pixels_are_covered_fully() {
    let c = polygon(&rect(1.0, 1.0, 3.0, 3.0, true)).render();
    assert_eq!((c.left, c.top, c.width, c.height), (1, 3, 2, 2));
    assert_eq!(c.alpha, vec![255; 4]);
    // Either way round, by either rule.
    let mut ccw = polygon(&rect(1.0, 1.0, 3.0, 3.0, false));
    assert_eq!(ccw.render().alpha, vec![255; 4]);
    ccw.fill_rule = FillRule::EvenOdd;
    assert_eq!(ccw.render().alpha, vec![255; 4]);
}

#[test]
fn half_pixels_are_half_covered() {
    // 0.5..2.5 across, 0..1 up: halves at the ends (FreeType's 128 for half a pixel), whole between.
    let c = polygon(&rect(0.5, 0.0, 2.5, 1.0, true)).render();
    assert_eq!((c.left, c.top, c.width, c.height), (0, 1, 3, 1));
    assert_eq!(c.alpha, vec![128, 255, 128]);
    // A quarter: half each way.
    let c = polygon(&rect(0.5, 0.5, 1.5, 1.5, true)).render();
    assert_eq!((c.width, c.height), (2, 2));
    assert_eq!(c.alpha, vec![64, 64, 64, 64]);
}

#[test]
fn pixel_boxes_are_freetypes() {
    // The control box out to whole pixels: floor on the left and bottom, ceiling on the right and top.
    let o = polygon(&rect(-1.25, -0.75, 2.5, 3.0, true));
    let b = o.pixel_box().unwrap();
    assert_eq!((b.left, b.top, b.width, b.height), (-2, 3, 5, 4));
    // Controls count too, whether the curve reaches them or not.
    let mut curve = FixedOutline::new();
    curve.move_to(p(0.0, 0.0));
    curve.conic_to(p(2.0, 5.0), p(4.0, 0.0));
    curve.close();
    let b = curve.pixel_box().unwrap();
    assert_eq!((b.left, b.top, b.width, b.height), (0, 5, 4, 5));
    let c = curve.render();
    // The curve peaks at 2.5: the top two rows hold nothing.
    assert!(c.alpha[..2 * 4].iter().all(|&a| a == 0));
    assert!(FixedOutline::new().pixel_box().is_none());
    assert!(FixedOutline::new().render().is_empty());
}

#[test]
fn coverage_is_the_exact_area() {
    // A triangle and a slanted quadrilateral at fractional positions: each pixel's coverage within a level or two
    // of 255 times its area inside (FreeType's 64ths of a pixel and its integer area arithmetic).
    let shapes: [&[(f32, f32)]; 3] = [
        &[(0.3, 0.2), (6.7, 1.1), (2.9, 5.6)],
        &[(1.1, 0.4), (5.9, 2.2), (4.4, 6.3), (0.6, 4.75)],
        &[(0.5, 0.5), (0.5, 3.5), (7.25, 3.5)],
    ];
    for shape in shapes {
        let outline = polygon(shape);
        let c = outline.render();
        let exact: Vec<(f64, f64)> =
            shape.iter().map(|&(x, y)| ((x * 64.0).round() as f64 / 64.0, (y * 64.0).round() as f64 / 64.0)).collect();
        let mut worst = 0.0f64;
        for row in 0..c.height {
            for col in 0..c.width {
                let x = (c.left + col as i32) as f64;
                let y = (c.top - 1 - row as i32) as f64;
                let want = pixel_area(&exact, x, y) * 255.0;
                let got = c.alpha[row * c.width + col] as f64;
                worst = worst.max((want - got).abs());
            }
        }
        assert!(worst <= 1.5, "{shape:?}: off by {worst}");
    }
}

#[test]
fn circles_cover_their_area() {
    // A circle of radius 6 from quadratics: its coverage adds up to pi r^2 within a few hundredths.
    let (cx, cy, r) = (7.3, 6.6, 6.0f32);
    let mut o = FixedOutline::new();
    let n = 16;
    let k = (std::f32::consts::PI / n as f32).cos();
    for i in 0..n {
        let a0 = i as f32 * 2.0 * std::f32::consts::PI / n as f32;
        let a1 = (i + 1) as f32 * 2.0 * std::f32::consts::PI / n as f32;
        let am = (a0 + a1) / 2.0;
        let on = |a: f32| p(cx + r * a.cos(), cy + r * a.sin());
        if i == 0 {
            o.move_to(on(a0));
        }
        o.conic_to(p(cx + r / k * am.cos(), cy + r / k * am.sin()), on(a1));
    }
    o.close();
    let c = o.render();
    let area: f32 = c.alpha.iter().map(|&a| a as f32 / 255.0).sum();
    let want = std::f32::consts::PI * r * r;
    assert!((area - want).abs() / want < 0.01, "{area} vs {want}");
    // Inside is full, far outside empty.
    assert_eq!(c.at(7 - c.left, c.top - 7), 255);
    assert_eq!(c.at(0, 0), 0);
}

#[test]
fn even_odd_and_non_zero_differ_where_contours_nest() {
    // Two squares, the inner one the same way round: non-zero fills it, even-odd leaves it a hole.
    let mut o = polygon(&rect(0.0, 0.0, 4.0, 4.0, true));
    o.append(&polygon(&rect(1.0, 1.0, 3.0, 3.0, true)));
    let non_zero = o.render();
    assert!(non_zero.alpha.iter().all(|&a| a == 255));
    o.fill_rule = FillRule::EvenOdd;
    let even_odd = o.render();
    assert_eq!(even_odd.at(1, 1), 0);
    assert_eq!(even_odd.at(0, 0), 255);
    // The other way round: a hole by either rule.
    let mut o = polygon(&rect(0.0, 0.0, 4.0, 4.0, true));
    o.append(&polygon(&rect(1.0, 1.0, 3.0, 3.0, false)));
    assert_eq!(o.render().at(1, 1), 0);
    o.fill_rule = FillRule::EvenOdd;
    assert_eq!(o.render().at(1, 1), 0);
}

#[test]
fn overlapping_contours_are_rendered_finer() {
    // Two squares overlapping by half, flagged: rendered four times finer and averaged, which for whole-pixel edges
    // gives the same full coverage; half-pixel edges come out as the sixteenths add up.
    let mut o = polygon(&rect(0.0, 0.0, 3.0, 2.0, true));
    o.append(&polygon(&rect(1.5, 0.0, 4.5, 2.0, true)));
    let plain = o.render();
    o.overlap = true;
    let fine = o.render();
    assert_eq!((fine.width, fine.height), (plain.width, plain.height));
    assert_eq!(fine.alpha, plain.alpha);
    assert_eq!(fine.at(4, 0), 128);
}

#[test]
fn decomposition_is_freetypes() {
    // Records what decomposition emits.
    #[derive(Default)]
    struct Log(Vec<String>);
    impl FixedSink for Log {
        fn move_to(&mut self, to: FixedPoint) {
            self.0.push(format!("M{},{}", to.x, to.y));
        }
        fn line_to(&mut self, to: FixedPoint) {
            self.0.push(format!("L{},{}", to.x, to.y));
        }
        fn conic_to(&mut self, c: FixedPoint, to: FixedPoint) {
            self.0.push(format!("Q{},{} {},{}", c.x, c.y, to.x, to.y));
        }
        fn cubic_to(&mut self, c1: FixedPoint, c2: FixedPoint, to: FixedPoint) {
            self.0.push(format!("C{},{} {},{} {},{}", c1.x, c1.y, c2.x, c2.y, to.x, to.y));
        }
    }
    let fp = FixedPoint::new;
    // All controls: the start halfway between the last and the first (C's halving: towards zero), midpoints
    // between each two, closed back to the start.
    let o = FixedOutline {
        points: vec![fp(0, 0), fp(10, -3), fp(-5, 7)],
        tags: vec![PointTag::Conic; 3],
        contours: vec![2],
        ..FixedOutline::default()
    };
    let mut log = Log::default();
    assert!(o.decompose(&mut log));
    assert_eq!(log.0, ["M-2,3", "Q0,0 5,-1", "Q10,-3 2,2", "Q-5,7 -2,3"]);
    // A control first and an on-curve point last: the start is the last point.
    let o = FixedOutline {
        points: vec![fp(64, 64), fp(128, 0), fp(0, 0)],
        tags: vec![PointTag::Conic, PointTag::On, PointTag::On],
        contours: vec![2],
        ..FixedOutline::default()
    };
    let mut log = Log::default();
    assert!(o.decompose(&mut log));
    assert_eq!(log.0, ["M0,0", "Q64,64 128,0", "L0,0"]);
    // A cubic, and a contour starting on a cubic control (not an outline).
    let o = FixedOutline {
        points: vec![fp(0, 0), fp(10, 20), fp(30, 20), fp(40, 0)],
        tags: vec![PointTag::On, PointTag::Cubic, PointTag::Cubic, PointTag::On],
        contours: vec![3],
        ..FixedOutline::default()
    };
    let mut log = Log::default();
    assert!(o.decompose(&mut log));
    assert_eq!(log.0, ["M0,0", "C10,20 30,20 40,0", "L0,0"]);
    let bad = FixedOutline { tags: vec![PointTag::Cubic, PointTag::On, PointTag::On, PointTag::On], ..o };
    assert!(!bad.decompose(&mut Log::default()));
}

#[test]
fn cubics_and_quadratics_cover_alike() {
    // The same parabola as a quadratic and as its exact cubic (its controls rounded to 64ths): coverage within a few
    // levels, as FreeType cuts them differently (a quadratic into 2^n even pieces, a cubic in halves until its
    // controls are within half a pixel of its chord's thirds).
    let mut q = FixedOutline::new();
    q.move_to(p(0.0, 0.0));
    q.conic_to(p(4.0, 8.0), p(8.0, 0.0));
    q.close();
    let mut c = FixedOutline::new();
    c.move_to(p(0.0, 0.0));
    c.cubic_to(p(8.0 / 3.0, 16.0 / 3.0), p(16.0 / 3.0, 16.0 / 3.0), p(8.0, 0.0));
    c.close();
    let (a, b) = (q.render(), c.render());
    assert_eq!((a.left, a.top, a.width, a.height), (0, 8, 8, 8));
    assert_eq!((b.left, b.top, b.width, b.height), (0, 6, 8, 6));
    // By pixel, y up from the baseline.
    for y in 0..8 {
        for x in 0..8 {
            let d = a.at(x, a.top - 1 - y).abs_diff(b.at(x, b.top - 1 - y));
            assert!(d <= 4, "({x}, {y}): {d}");
        }
    }
}

#[test]
fn transforms_are_freetypes_fixed_point() {
    // FT_MulFix rounds half away from zero.
    assert_eq!(mul_fix(3, 0x8000), 2);
    assert_eq!(mul_fix(-3, 0x8000), -2);
    assert_eq!(mul_fix(64, 0x10000), 64);
    let mut o = polygon(&rect(0.0, 0.0, 2.0, 4.0, true));
    // Twice as wide, and slanted: a point 4 px up moves 1 px right (a quarter).
    o.transform(&FixedMatrix::from_f32(2.0, 0.25, 0.0, 1.0));
    let b = o.cbox().unwrap();
    assert_eq!((b.x_min, b.x_max, b.y_min, b.y_max), (0, 5 * 64, 0, 4 * 64));
    assert!(FixedMatrix::IDENTITY.is_identity());
    // (C's conversion: towards zero.)
    assert_eq!(FixedMatrix::from_f32(-0.25, 0.0, 0.0, 1.0).xx, -16384);
}

#[test]
fn what_lies_outside_is_left_out() {
    // A square reaching past a 4×4 bitmap each way: only its part inside is covered, cover from its left carried in.
    let o = polygon(&rect(-2.0, -2.0, 2.5, 6.0, true));
    let alpha = o.rasterize(4, 4);
    for row in alpha.chunks(4) {
        assert_eq!(row, [255, 255, 128, 0]);
    }
    assert!(o.rasterize(0, 3).is_empty());
}
