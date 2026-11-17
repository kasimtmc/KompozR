//! Paths, their operations, iterators, measures, hit testers, effects and strokes, and Matrix (the C++ Kompoz's
//! tests about them; those that drew through a canvas check the paths they drew).

use kompoz_graphics::*;
use std::f32::consts::{FRAC_1_SQRT_2, PI};

// ---- Helpers ------------------------------------------------------------------------------------------------------

fn o(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}
fn near_by(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() < tolerance
}

/// A rect from its left, top, width and height (as the C++ wrote them).
fn xywh(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x, y, x + w, y + h)
}

/// Its left, top, width and height near these (within half a unit).
fn same_rect(r: Rect, x: f32, y: f32, w: f32, h: f32) -> bool {
    near(r.left, x) && near(r.top, y) && near(r.width(), w) && near(r.height(), h)
}

/// A command's control points and end.
fn c1(c: &PathCommand) -> Offset {
    match *c {
        PathCommand::Quadratic(c1, _) | PathCommand::Conic(c1, _, _) | PathCommand::Cubic(c1, _, _) => c1,
        _ => Offset::Zero,
    }
}
fn c2(c: &PathCommand) -> Offset {
    match *c {
        PathCommand::Cubic(_, c2, _) => c2,
        _ => Offset::Zero,
    }
}
fn end(c: &PathCommand) -> Offset {
    c.end().unwrap_or(Offset::Zero)
}
fn weight(c: &PathCommand) -> f32 {
    match *c {
        PathCommand::Conic(_, _, w) => w,
        _ => 1.0,
    }
}
fn is_move(c: &PathCommand) -> bool {
    matches!(c, PathCommand::Move(_))
}
fn is_line(c: &PathCommand) -> bool {
    matches!(c, PathCommand::Line(_))
}
fn is_quad(c: &PathCommand) -> bool {
    matches!(c, PathCommand::Quadratic(..))
}
fn is_conic(c: &PathCommand) -> bool {
    matches!(c, PathCommand::Conic(..))
}
fn is_cubic(c: &PathCommand) -> bool {
    matches!(c, PathCommand::Cubic(..))
}
fn last_end(p: &Path) -> Offset {
    end(p.commands().last().expect("a command"))
}

/// A path's commands as text: "M0,0 L10,0 Q… K… C… Z", each end to a hundredth.
fn path_text(path: &Path) -> String {
    let num = |v: f32| format!("{}", (v * 100.0).round() / 100.0);
    let mut out = String::new();
    for c in path.commands() {
        if !out.is_empty() {
            out.push(' ');
        }
        let (letter, p) = match *c {
            PathCommand::Move(p) => ("M", p),
            PathCommand::Line(p) => ("L", p),
            PathCommand::Quadratic(_, p) => ("Q", p),
            PathCommand::Conic(_, p, _) => ("K", p),
            PathCommand::Cubic(_, _, p) => ("C", p),
            PathCommand::Close => {
                out.push('Z');
                continue;
            }
        };
        out += &format!("{letter}{},{}", num(p.x), num(p.y));
    }
    out
}

/// What a path effect makes of a path drawn with `stroke` at a unit a pixel (the path itself if it leaves it as it
/// is).
fn effected_with(effect: &PathEffect, path: &Path, mut stroke: PathStroke) -> Path {
    let mut out = Path::new();
    if effect.filter_path(&mut out, path, &mut stroke, 1.0) { out } else { path.clone() }
}
fn effected(effect: &PathEffect, path: &Path) -> Path {
    effected_with(effect, path, PathStroke::new(1.0))
}

fn stroke_with(width: f32, cap: StrokeCap) -> PathStroke {
    PathStroke { width, cap, ..PathStroke::default() }
}

fn measure(path: &Path) -> PathMeasure {
    measure_with(path, false, 1.0)
}
fn measure_with(path: &Path, force_closed: bool, res_scale: f32) -> PathMeasure {
    let mut m = PathMeasure::new();
    m.set_path_at_res_scale(path, force_closed, res_scale);
    m
}
/// Every contour's length, added up.
fn total_length(path: &Path) -> f32 {
    let mut m = measure(path);
    let mut drawn = m.length();
    while m.next_contour() {
        drawn += m.length();
    }
    drawn
}

fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Path {
    let mut p = Path::new();
    p.add_rect(xywh(x, y, w, h), PathDirection::CounterClockwise);
    p
}

fn line(from: Offset, to: Offset) -> Path {
    let mut p = Path::new();
    p.move_to(from.x, from.y);
    p.line_to(to.x, to.y);
    p
}

/// The path Skia draws for an arc (SkPathPriv::CreateDrawArcPath, as Compose's DrawScope.drawArc makes it): a whole
/// oval for a filled sweep of a turn or more; else through the center first if `use_center` (and back to it), the
/// sweep in half turns while it is a turn or more (arc_to goes round only once).
fn arc_path(oval: Rect, mut start: f32, mut sweep: f32, use_center: bool, fill_no_effect: bool) -> Path {
    let mut p = Path::new();
    if fill_no_effect && sweep.abs() >= 360.0 {
        p.add_oval(oval, PathDirection::Clockwise);
        return p;
    }
    if use_center {
        p.move_to(oval.center().x, oval.center().y);
    }
    let mut force_move_to = !use_center;
    for half in [-180.0f32, 180.0] {
        while if half < 0.0 { sweep <= -360.0 } else { sweep >= 360.0 } {
            p.arc_to(oval, start, half, force_move_to);
            start += half;
            p.arc_to(oval, start, half, false);
            start += half;
            force_move_to = false;
            sweep -= 2.0 * half;
        }
    }
    p.arc_to(oval, start, sweep, force_move_to);
    if use_center {
        p.close();
    }
    p
}

// ---- Matrix -------------------------------------------------------------------------------------------------------

#[test]
fn matrix_follows_compose() {
    let at = |p: Offset, want: Offset| (p.x - want.x).abs() < 1e-4 && (p.y - want.y).abs() < 1e-4;
    let mut m = Matrix::new();
    assert!(m.is_identity());
    m.translate(10.0, 0.0, 0.0);
    m.rotate_z(90.0); // turned first (clockwise), then moved
    assert!(at(m.map(o(1.0, 0.0)), o(10.0, 1.0)));
    let mut s = Matrix::new();
    s.scale(2.0, 3.0, 1.0);
    s.translate(5.0, 5.0, 0.0); // moved first, then scaled
    assert!(at(s.map(o(1.0, 1.0)), o(12.0, 18.0)));
    let mut x = Matrix::new();
    x.scale(2.0, 2.0, 1.0);
    x.rotate_x(60.0); // after the scale: y' = y cos 60
    assert!(at(x.map(o(0.0, 1.0)), o(0.0, 1.0)));
    let mut p = Matrix::new();
    p.set(1, 3, 0.01); // Perspective1: w = 1 + y / 100
    assert!(at(p.map(o(0.0, 100.0)), o(0.0, 50.0)));
    p.set(3, 3, 0.0);
    assert!(at(p.map(o(0.0, 0.0)), o(0.0, 0.0))); // w = 0: the origin
    let mut r = Matrix::new();
    r.rotate_z(90.0);
    let b = r.map_rect(Rect::new(0.0, 0.0, 10.0, 20.0));
    assert!(
        (b.left + 20.0).abs() < 1e-4
            && b.top.abs() < 1e-4
            && (b.width() - 20.0).abs() < 1e-4
            && (b.height() - 10.0).abs() < 1e-4
    );
    let mut a = Matrix::new();
    let mut c = Matrix::new();
    a.translate(10.0, 0.0, 0.0);
    c.scale(2.0, 2.0, 1.0);
    a *= &c; // a, then c
    assert!(at(a.map(o(1.0, 0.0)), o(22.0, 0.0)));
    let mut inv = m;
    inv.invert();
    assert!(at(inv.map(m.map(o(3.0, 7.0))), o(3.0, 7.0)));
    let mut singular = Matrix::new();
    singular.scale(0.0, 1.0, 1.0);
    let kept = singular;
    singular.invert();
    assert_eq!(singular, kept); // no inverse: left as it is
    let mut g = Matrix::new();
    g.reset_to_pivoted_transform(10.0, 10.0, 5.0, 0.0, 0.0, 0.0, 0.0, 90.0, 2.0, 2.0, 1.0); // scaled, turned about (10, 10), moved
    assert!(at(g.map(o(20.0, 10.0)), o(15.0, 30.0)));
    let mut steps = Matrix::new(); // the same step by step
    steps.translate(15.0, 10.0, 0.0);
    steps.rotate_z(90.0);
    steps.scale(2.0, 2.0, 1.0);
    steps.translate(-10.0, -10.0, 0.0);
    assert!(at(steps.map(o(13.0, 4.0)), g.map(o(13.0, 4.0))));
    let mut tilt = Matrix::new();
    tilt.reset_to_pivoted_transform(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 60.0, 0.0, 1.0, 1.0, 1.0); // about y: x' = x cos 60
    assert!(at(tilt.map(o(4.0, 2.0)), o(2.0, 2.0)));
    let mut both = Matrix::new();
    both.reset_to_pivoted_transform(0.0, 0.0, 0.0, 0.0, 0.0, 60.0, 60.0, 0.0, 1.0, 1.0, 1.0); // y goes into x by sin 60 sin 60
    assert!(at(both.map(o(0.0, 1.0)), o(0.75, 0.5)));
    let mut yx = Matrix::new();
    yx.rotate_y(60.0);
    yx.rotate_x(60.0); // x went into z, which this turns into y: y' = -z sin 60
    assert!(at(yx.map(o(1.0, 0.0)), o(0.5, 0.75)));
    g.reset();
    assert!(g.is_identity());
}

// ---- Building paths -----------------------------------------------------------------------------------------------

#[test]
fn path_building_and_bounds() {
    let mut p = Path::new();
    p.move_to(10.0, 10.0);
    p.relative_line_to(20.0, 0.0);
    p.quadratic_to(40.0, 0.0, 50.0, 10.0);
    p.cubic_to(60.0, 20.0, 60.0, 30.0, 50.0, 40.0);
    p.close();
    assert!(same_rect(p.get_bounds(), 10.0, 0.0, 50.0, 40.0));
    assert!(p.commands().len() == 5 && end(&p.commands()[1]).x == 30.0);
    let mut oval = Path::new();
    oval.add_oval(xywh(0.0, 0.0, 100.0, 50.0), PathDirection::CounterClockwise);
    assert!(same_rect(oval.get_bounds(), 0.0, 0.0, 100.0, 50.0));
    assert_eq!(oval.commands().len(), 6);
    let mut copy = oval.clone();
    assert!(copy == oval && copy != p);
    copy.reset();
    assert!(copy.is_empty());
    let mut rounded = Path::new();
    // The radius clamped to half the height.
    rounded.add_round_rect(
        &RoundRect::from_rect_radius(xywh(0.0, 0.0, 40.0, 20.0), CornerRadius::circular(50.0)),
        PathDirection::CounterClockwise,
    );
    assert!(same_rect(rounded.get_bounds(), 0.0, 0.0, 40.0, 20.0));
}

#[test]
fn paths_take_arcs() {
    let at = |p: Offset, x: f32, y: f32| near_by(p.x, x, 0.01) && near_by(p.y, y, 0.01);
    let r100 = xywh(0.0, 0.0, 100.0, 100.0);
    // A quarter of the circle in 100×100: from the right (0°), clockwise, to the bottom.
    let mut quarter = Path::new();
    quarter.add_arc(r100, 0.0, 90.0);
    assert!(is_move(&quarter.commands()[0]) && at(end(&quarter.commands()[0]), 100.0, 50.0));
    assert!(at(last_end(&quarter), 50.0, 100.0) && near_by(measure(&quarter).length(), PI * 25.0, 0.3));
    // Joined by a line to where the arc starts, unless forced to move there.
    let mut joined = Path::new();
    joined.move_to(0.0, 0.0);
    joined.arc_to(r100, 180.0, 90.0, false);
    assert!(is_line(&joined.commands()[1]) && at(last_end(&joined), 50.0, 0.0));
    // (A move right after a move takes its place, as Skia's.)
    let mut moved = Path::new();
    moved.move_to(0.0, 0.0);
    moved.arc_to(r100, 180.0, 90.0, true);
    assert!(moved.commands().len() == 2 && is_move(&moved.commands()[0]) && at(last_end(&moved), 50.0, 0.0));
    let (mut back, mut wide, mut whole, mut lone) = (Path::new(), Path::new(), Path::new(), Path::new());
    back.add_arc(r100, 0.0, -90.0); // the other way: up to the top
    wide.add_arc(xywh(0.0, 0.0, 200.0, 100.0), 90.0, 270.0); // three quarters of a wide oval, in three conics
    whole.add_arc(r100, 90.0, 360.0); // a turn from a quarter: the oval from there, clockwise
    lone.add_arc(r100, 30.0, 360.0); // a turn from elsewhere: where it starts and ends (Skia's)
    assert!(at(last_end(&back), 50.0, 0.0) && at(last_end(&wide), 200.0, 50.0) && wide.commands().len() == 4);
    assert!(
        whole.commands().len() == 6 && end(&whole.commands()[0]) == o(50.0, 100.0) && is_conic(&whole.commands()[1])
    );
    assert!(lone.commands().len() == 1 && is_move(&lone.commands()[0]));
    // About 3/4 of a 200×100 oval.
    assert!(near_by(measure(&wide).length(), 3.0 * measure(&quarter).length() * 1.5, 15.0));
    // Relative curves, paths added and moved.
    let mut curves = Path::new();
    curves.move_to(10.0, 10.0);
    curves.relative_quadratic_to(10.0, 0.0, 10.0, 10.0);
    curves.relative_cubic_to(0.0, 10.0, 10.0, 10.0, 10.0, 0.0);
    assert!(at(end(&curves.commands()[1]), 20.0, 20.0) && at(last_end(&curves), 30.0, 20.0));
    assert!(at(c1(&curves.commands()[2]), 20.0, 30.0));
    let mut square = rect_path(0.0, 0.0, 10.0, 10.0);
    let mut two = Path::new();
    two.add_path(&square, o(5.0, 5.0));
    two.add_path(&square, o(20.0, 0.0));
    let b = two.get_bounds();
    assert!(near(b.left, 5.0) && near(b.top, 0.0) && near(b.right, 30.0) && near(b.bottom, 15.0));
    square.translate(o(1.0, 2.0));
    let b = square.get_bounds();
    assert!(near(b.left, 1.0) && near(b.top, 2.0) && near(b.width(), 10.0));
    curves.translate(o(5.0, -5.0)); // control points too
    assert!(at(c1(&curves.commands()[1]), 25.0, 5.0) && at(c2(&curves.commands()[2]), 35.0, 25.0));
    assert!(at(last_end(&curves), 35.0, 15.0));
}

#[test]
fn paths_are_skias() {
    let is = |p: Offset, x: f32, y: f32| near_by(p.x, x, 1e-4) && near_by(p.y, y, 1e-4);
    let ccw_dir = PathDirection::CounterClockwise;
    let cw_dir = PathDirection::Clockwise;
    // Shapes start where Skia's do and go round as asked (counterclockwise by default, as Compose's): a rect from its
    // top left, an oval from its right as four conics.
    let (mut ccw, mut cw) = (Path::new(), Path::new());
    ccw.add_rect(xywh(0.0, 0.0, 20.0, 10.0), ccw_dir);
    cw.add_rect(xywh(0.0, 0.0, 20.0, 10.0), cw_dir);
    assert_eq!(path_text(&ccw), "M0,0 L0,10 L20,10 L20,0 Z");
    assert_eq!(path_text(&cw), "M0,0 L20,0 L20,10 L0,10 Z");
    let (mut oval, mut oval_cw) = (Path::new(), Path::new());
    oval.add_oval(xywh(0.0, 0.0, 20.0, 10.0), ccw_dir);
    oval_cw.add_oval(xywh(0.0, 0.0, 20.0, 10.0), cw_dir);
    assert_eq!(path_text(&oval), "M20,5 K10,0 K0,5 K10,10 K20,5 Z");
    assert_eq!(path_text(&oval_cw), "M20,5 K10,10 K0,5 K10,0 K20,5 Z");
    assert!(weight(&oval.commands()[1]) == FRAC_1_SQRT_2 && is(c1(&oval.commands()[1]), 20.0, 0.0));
    // Round rects: from where the top left corner meets the left side counterclockwise (the bottom left clockwise),
    // lines and quarter ellipses; all corners round: an oval from its left; none: a rect; a corner with a radius of 0
    // either way: square.
    let c46 = CornerRadius::new(4.0, 6.0);
    let r40 = RoundRect::from_rect_radius(xywh(0.0, 0.0, 40.0, 20.0), c46);
    let (mut rounded, mut rounded_cw) = (Path::new(), Path::new());
    rounded.add_round_rect(&r40, ccw_dir);
    rounded_cw.add_round_rect(&r40, cw_dir);
    assert_eq!(path_text(&rounded), "M0,6 L0,14 K4,20 L36,20 K40,14 L40,6 K36,0 L4,0 K0,6 Z");
    assert_eq!(path_text(&rounded_cw), "M0,14 L0,6 K4,0 L36,0 K40,6 L40,14 K36,20 L4,20 K0,14 Z");
    let (mut circle, mut square, mut one_square) = (Path::new(), Path::new(), Path::new());
    let r20 = xywh(0.0, 0.0, 20.0, 20.0);
    circle.add_round_rect(&RoundRect::from_rect_radius(r20, CornerRadius::circular(10.0)), ccw_dir);
    square.add_round_rect(&RoundRect::from_rect_radius(r20, CornerRadius::Zero), ccw_dir);
    let four = CornerRadius::circular(4.0);
    one_square.add_round_rect(&RoundRect::from_rect(r20, CornerRadius::new(4.0, 0.0), four, four, four), ccw_dir);
    assert_eq!(path_text(&circle), "M0,10 K10,20 K20,10 K10,0 K0,10 Z");
    assert_eq!(path_text(&square), "M0,0 L0,20 L20,20 L20,0 Z");
    // (Its corner a conic of no size, as Skia's.)
    assert_eq!(path_text(&one_square), "M0,0 L0,16 K4,20 L16,20 K20,16 L20,4 K16,0 L0,0 K0,0 Z");
    // Arcs as conics: a quarter one, 200 degrees in three (the last its 20 degrees: w = cos 10°).
    let (mut quarter, mut wide) = (Path::new(), Path::new());
    quarter.arc_to(r20, 0.0, 90.0, true);
    wide.arc_to(r20, 30.0, 200.0, true);
    assert!(
        quarter.commands().len() == 2
            && is_conic(&quarter.commands()[1])
            && is(end(&quarter.commands()[1]), 10.0, 20.0)
    );
    assert!(wide.commands().len() == 4 && near_by(weight(&wide.commands()[3]), (10.0 * PI / 180.0).cos(), 1e-4));
    let a230 = 230.0 * PI / 180.0;
    assert!(is(end(&wide.commands()[3]), 10.0 + 10.0 * a230.cos(), 10.0 + 10.0 * a230.sin()));
    // A sweep of none from 0 is a lone point (moving into an oval); nearly a turn, nearly a circle; a turn exactly, a
    // point (arc_to goes round once at most: drawArc halves it).
    let (mut lone, mut there, mut almost, mut closest, mut turn) =
        (Path::new(), Path::new(), Path::new(), Path::new(), Path::new());
    lone.move_to(0.0, 0.0);
    lone.arc_to(r20, 0.0, 0.0, false);
    there.move_to(20.0, 10.0); // (a lone point's line even where the pen is)
    there.arc_to(r20, 0.0, 0.0, false);
    almost.arc_to(r20, 0.0, 359.5, true);
    closest.arc_to(r20, 0.0, 359.99, true); // its end the start's in floats: nudged back, nearly round
    turn.arc_to(r20, 0.0, 360.0, true);
    assert_eq!(path_text(&lone), "M0,0 L20,10");
    assert_eq!(path_text(&there), "M20,10 L20,10");
    assert!(almost.commands().len() == 5 && turn.commands().len() == 1);
    assert!(closest.commands().len() == 5 && end(&closest.commands()[4]).y < 10.0);
    // Joined by a line where the arc doesn't start at the pen; not when it does.
    let (mut joined, mut onward) = (Path::new(), Path::new());
    joined.move_to(0.0, 0.0);
    joined.arc_to(r20, 180.0, 90.0, false);
    onward.move_to(0.0, 10.0);
    onward.arc_to(r20, 180.0, 90.0, false);
    assert_eq!(path_text(&joined), "M0,0 L0,10 K10,0");
    assert_eq!(path_text(&onward), "M0,10 K10,0");
    // conic_to: a weight of 1 is a quadratic, none a line, an infinite one two lines.
    let mut conics = Path::new();
    conics.move_to(0.0, 0.0);
    conics.conic_to(5.0, 5.0, 10.0, 0.0, 1.0);
    conics.conic_to(15.0, 5.0, 20.0, 0.0, 0.0);
    conics.conic_to(25.0, 5.0, 30.0, 0.0, f32::INFINITY);
    assert_eq!(path_text(&conics), "M0,0 Q10,0 L20,0 L25,5 L30,0");
    // A move right after a move takes its place; drawing after close() starts from the contour's start with a move
    // of its own; a relative move after close() is from that start.
    let mut moves = Path::new();
    moves.move_to(1.0, 1.0);
    moves.move_to(2.0, 2.0);
    moves.line_to(10.0, 2.0);
    moves.close();
    moves.line_to(5.0, 9.0);
    moves.close();
    moves.relative_move_to(1.0, 1.0);
    moves.line_to(0.0, 0.0);
    assert_eq!(path_text(&moves), "M2,2 L10,2 Z M2,2 L5,9 Z M3,3 L0,0");
    assert_eq!(Path::from_commands(moves.commands().to_vec(), PathFillType::NonZero), moves);
    // Through a matrix: affinely every point; in perspective quadratics become conics (their weight through the map,
    // exactly), cubics four, what is behind the eye cut off.
    let mut turned = Matrix::new();
    turned.rotate_z(90.0);
    let mut t = ccw.clone();
    t.transform(&turned);
    assert!(is(end(&t.commands()[1]), -10.0, 0.0) && is(end(&t.commands()[2]), -10.0, 20.0));
    let mut seen = Matrix::new();
    seen.set(0, 3, 0.01); // w = 1 + x / 100
    let mut q = Path::new();
    q.move_to(0.0, 0.0);
    q.quadratic_to(50.0, 50.0, 100.0, 0.0);
    q.transform(&seen);
    let (w0, w1, w2) = (1.0f32, 1.5f32, 2.0f32); // the weight w1 / sqrt(w0 w2)
    let k = &q.commands()[1];
    assert!(is_conic(k) && near_by(weight(k), w1 / (w0 * w2).sqrt(), 1e-5));
    assert!(is(end(k), 50.0, 0.0) && is(c1(k), 50.0 / 1.5, 50.0 / 1.5));
    let mut cubic = Path::new();
    cubic.move_to(0.0, 0.0);
    cubic.cubic_to(10.0, 20.0, 30.0, 20.0, 40.0, 0.0);
    cubic.transform(&seen);
    assert!(cubic.commands().len() == 5 && is(end(&cubic.commands()[4]), 40.0 / 1.4, 0.0));
    let mut behind = Matrix::new();
    behind.set(0, 3, 0.02); // w below 0 left of x = -50
    let mut cut = Path::new();
    cut.add_rect(xywh(-100.0, 0.0, 140.0, 10.0), cw_dir);
    cut.transform(&behind);
    // (Its far left: where w is the plane's 1/16384, Skia's.)
    let (mut right, mut left) = (f32::NEG_INFINITY, f32::INFINITY);
    for c in cut.commands() {
        if let Some(p) = c.end() {
            right = right.max(p.x);
            left = left.min(p.x);
            assert!(p.x.is_finite() && p.y.is_finite());
        }
    }
    assert!(
        cut.commands().len() == 5 && near_by(right, 40.0 / 1.8, 1e-3) && near_by(left, -50.0 * 16384.0 + 50.0, 100.0)
    );
    // Convex: one contour turning one way (a line too); not two contours, nor a dent, nor back and forth thrice.
    let (mut triangle, mut dent, mut line, mut zigzag, mut lines) =
        (Path::new(), Path::new(), Path::new(), Path::new(), Path::new());
    let mut two = ccw.clone();
    lines.move_to(0.0, 0.0); // (two contours, one way all along)
    lines.line_to(10.0, 0.0);
    lines.move_to(20.0, 0.0);
    lines.line_to(30.0, 0.0);
    triangle.move_to(0.0, 0.0);
    triangle.line_to(10.0, 0.0);
    triangle.line_to(0.0, 10.0);
    triangle.close();
    dent.move_to(0.0, 0.0);
    dent.line_to(10.0, 0.0);
    dent.line_to(5.0, 5.0);
    dent.line_to(10.0, 10.0);
    dent.line_to(0.0, 10.0);
    dent.close();
    line.move_to(0.0, 0.0);
    line.line_to(10.0, 10.0);
    zigzag.move_to(0.0, 0.0);
    zigzag.line_to(10.0, 0.0);
    zigzag.line_to(0.0, 0.0);
    zigzag.line_to(10.0, 0.0);
    two.add_rect(xywh(30.0, 0.0, 5.0, 5.0), ccw_dir);
    assert!(
        triangle.is_convex() && oval.is_convex() && rounded.is_convex() && line.is_convex() && Path::new().is_convex()
    );
    assert!(!dent.is_convex() && !two.is_convex() && !zigzag.is_convex() && !lines.is_convex());
    // Inside, the curves exact: within a hundredth of a unit of a circle's edge, either way; on it, inside.
    let mut round = Path::new();
    round.add_oval(xywh(0.0, 0.0, 200.0, 200.0), ccw_dir);
    let (r, d45) = (100.0f32, FRAC_1_SQRT_2);
    assert!(round.contains(o(100.0 + (r - 0.01) * d45, 100.0 + (r - 0.01) * d45)));
    assert!(!round.contains(o(100.0 + (r + 0.01) * d45, 100.0 + (r + 0.01) * d45)));
    assert!(round.contains(o(200.0, 100.0)) && round.contains(o(100.0, 0.0)));
}

/// The paths Compose's DrawScope draws its arcs and points as (the C++ draw_scopes_take_composes_parameters and
/// draw_points_as_dots_lines_or_one_line, their paths).
#[test]
fn drawn_arcs_and_points_are_skias_paths() {
    let r20 = xywh(0.0, 0.0, 20.0, 20.0);
    // Arcs: a pie slice, filled across its ends, its outline through the center, an arc; an ellipse's; a turn and
    // more: a whole oval filled, outlined round and on.
    assert_eq!(path_text(&arc_path(r20, 0.0, 90.0, true, true)), "M10,10 L20,10 K10,20 Z"); // a quarter circle: one conic
    assert_eq!(path_text(&arc_path(r20, 0.0, 90.0, false, true)), "M20,10 K10,20");
    assert_eq!(path_text(&arc_path(r20, 0.0, 90.0, true, false)), "M10,10 L20,10 K10,20 Z");
    assert_eq!(path_text(&arc_path(xywh(0.0, 0.0, 20.0, 10.0), 0.0, 90.0, false, false)), "M20,5 K10,10");
    let mut oval = Path::new(); // (as Android's canvas makes it: clockwise, from its right)
    oval.add_oval(r20, PathDirection::Clockwise);
    assert_eq!(arc_path(r20, 0.0, 400.0, false, true), oval);
    let mut around = measure(&arc_path(r20, 0.0, 400.0, false, false)); // a turn and 40 degrees, in one contour
    assert!(near(around.length(), 2.0 * PI * 10.0 * 400.0 / 360.0) && !around.next_contour());
    // Points: dots as wide as the stroke, ovals for a round cap, squares (counterclockwise) otherwise.
    let points = [o(0.0, 0.0), o(10.0, 0.0), o(10.0, 10.0), o(0.0, 10.0), o(5.0, 5.0)];
    let (mut round, mut squares) = (Path::new(), Path::new());
    for p in points {
        let dot = Rect::new(p.x - 2.0, p.y - 2.0, p.x + 2.0, p.y + 2.0);
        round.add_oval(dot, PathDirection::CounterClockwise);
        squares.add_rect(dot, PathDirection::CounterClockwise);
    }
    let dots = path_text(&round);
    assert!(dots.matches('M').count() == 5 && dots.contains('K'));
    assert!(path_text(&squares).starts_with("M-2,-2 L-2,2 L2,2 L2,-2 Z"));
}

// ---- Path effects -------------------------------------------------------------------------------------------------

/// The C++ path_effects_dash_and_round_strokes: what the effects made of what DrawScope drew.
#[test]
fn path_effects_dash_and_round_strokes() {
    let dash = PathEffect::dash_path_effect(&[10.0, 5.0], 0.0);
    let drawn = |path: &Path, stroke: &mut PathStroke| path_effected(path, stroke, 1.0).into_owned();
    let with = |width: f32, cap: StrokeCap, effect: PathEffect| PathStroke {
        width,
        cap,
        path_effect: Some(effect),
        ..PathStroke::default()
    };
    // A line with butt ends: each dash its rectangle, filled (Skia's way with a line).
    let mut stroke = with(2.0, StrokeCap::Butt, dash.clone());
    let butt = drawn(&line(o(0.0, 0.0), o(40.0, 0.0)), &mut stroke);
    assert_eq!(path_text(&butt), "M0,-1 L10,-1 L10,1 L0,1 M15,-1 L25,-1 L25,1 L15,1 M30,-1 L40,-1 L40,1 L30,1");
    assert!(stroke.width == 0.0 && !stroke.strokes() && stroke.path_effect.is_none());
    // 5 into the pattern.
    let mut stroke = with(2.0, StrokeCap::Round, PathEffect::dash_path_effect(&[10.0, 5.0], 5.0));
    let round = drawn(&line(o(0.0, 10.0), o(40.0, 10.0)), &mut stroke);
    assert_eq!(path_text(&round), "M0,10 L5,10 M10,10 L20,10 M25,10 L35,10");
    assert!(stroke.width == 2.0 && stroke.cap == StrokeCap::Round);
    // Each corner cut back 4 and rounded through it, from the first side cut back (counterclockwise, as Compose's
    // add_rect goes: down the left side first).
    let mut square = Path::new();
    square.add_rect(xywh(0.0, 0.0, 20.0, 20.0), PathDirection::CounterClockwise);
    let mut stroke = with(1.0, StrokeCap::Butt, PathEffect::corner_path_effect(4.0));
    assert_eq!(path_text(&drawn(&square, &mut stroke)), "M0,4 L0,16 Q4,20 L16,20 Q20,16 L20,4 Q16,0 L4,0 Q0,4 Z");
    // As Android's canvas makes them, clockwise: the circle from its right, dashed from there downwards, its first
    // dash left for the end, where the last one goes on into it (a closed contour's dashes join round its start); the
    // rect from its top left (rounded, then dashed from its first side cut back: the first dash, 0 to 4, joined to
    // the end too).
    let mut circle = Path::new();
    circle.add_oval(xywh(40.0, 40.0, 20.0, 20.0), PathDirection::Clockwise);
    let dashes = drawn(&circle, &mut with(2.0, StrokeCap::Butt, dash.clone()));
    let d = dashes.commands();
    assert!(d.len() > 2 && is_move(&d[0]) && near_by(end(&d[0]).x, 50.58, 0.01) && near_by(end(&d[0]).y, 59.98, 0.01));
    assert!(is_conic(&d[1]) && near_by(last_end(&dashes).x, 55.33, 0.01) && near_by(last_end(&dashes).y, 58.46, 0.01));
    // The dashes around the circle: two thirds of it drawn.
    assert!(near_by(total_length(&dashes), 2.0 * PI * 10.0 * 2.0 / 3.0, 1.5));
    let mut rect = Path::new();
    rect.add_rect(xywh(0.0, 0.0, 30.0, 30.0), PathDirection::Clockwise);
    let chain = PathEffect::chain_path_effect(
        &PathEffect::dash_path_effect(&[4.0, 4.0], 0.0),
        &PathEffect::corner_path_effect(6.0),
    );
    let chained = path_text(&drawn(&rect, &mut with(1.0, StrokeCap::Butt, chain)));
    assert!(chained.starts_with("M14,0 L18,0") && chained.ends_with("M6,0 L10,0"));
    // Rounded, then dashed: its curves pieces of them.
    assert!(chained.contains('Q') && chained.find('M') != chained.rfind('M'));
    // A quarter circle (15.7) in 3 on, 3 off: three dashes.
    let arc = arc_path(xywh(0.0, 0.0, 20.0, 20.0), 0.0, 90.0, false, false);
    let arc_dashes = drawn(&arc, &mut with(1.0, StrokeCap::Butt, PathEffect::dash_path_effect(&[3.0, 3.0], 0.0)));
    assert!(near_by(total_length(&arc_dashes), 9.0, 0.3));
    // (The C++ display_list_path_effects_are_part_of_what_is_drawn) 5 into a pattern of 10 on, 10 off.
    let mut stroke = with(2.0, StrokeCap::Butt, PathEffect::dash_path_effect(&[10.0, 10.0], 5.0));
    assert!(
        path_text(&drawn(&line(o(10.0, 10.0), o(90.0, 10.0)), &mut stroke))
            .starts_with("M10,9 L15,9 L15,11 L10,11 M25,9")
    );
}

/// The C++ canvases_draw_as_composes, its paths: fills with a path effect are rounded where it rounds, never dashed
/// (Skia dashes only strokes).
#[test]
fn fills_are_rounded_never_dashed() {
    let mut rect = Path::new();
    rect.add_rect(xywh(0.0, 0.0, 20.0, 20.0), PathDirection::Clockwise);
    let fill = |effect: PathEffect| PathStroke { path_effect: Some(effect), ..PathStroke::default() };
    let mut corner = fill(PathEffect::corner_path_effect(4.0));
    assert!(path_text(&path_effected(&rect, &mut corner, 1.0)).contains('Q'));
    let mut dashed = fill(PathEffect::dash_path_effect(&[2.0, 2.0], 0.0));
    let same = path_effected(&rect, &mut dashed, 1.0);
    assert!(path_text(&same) == "M0,0 L20,0 L20,20 L0,20 Z" && !dashed.strokes());
    let mut stroked = PathStroke { width: 1.0, ..fill(PathEffect::dash_path_effect(&[2.0, 2.0], 0.0)) };
    assert!(path_effected(&rect, &mut stroked, 1.0).commands().len() > 10);
}

#[test]
fn stamped_path_effects_lay_a_shape_along_the_path() {
    let box_ = rect_path(0.0, -1.0, 4.0, 2.0); // 4 x 2, around the x axis
    let starts = |p: &Path| {
        // Where each contour starts.
        let mut out = Vec::new();
        let mut m = measure(p);
        loop {
            out.push(m.get_position(0.0));
            if !m.next_contour() {
                break;
            }
        }
        out
    };
    let near2 = |a: Offset, b: Offset| near_by(a.x, b.x, 0.01) && near_by(a.y, b.y, 0.01);
    let stamp =
        |shape: &Path, advance: f32, phase: f32, style| PathEffect::stamped_path_effect(shape, advance, phase, style);
    // Moved: every 20 along a 100 long line.
    let line = line(o(0.0, 0.0), o(100.0, 0.0));
    let at = starts(&effected(&stamp(&box_, 20.0, 0.0, StampedPathEffectStyle::Translate), &line));
    assert_eq!(at.len(), 5);
    for (i, &p) in at.iter().enumerate() {
        assert!(near2(p, o(20.0 * i as f32, -1.0)));
    }
    // A phase of 5 puts the first one at 15 (Skia's way).
    let at = starts(&effected(&stamp(&box_, 20.0, 5.0, StampedPathEffectStyle::Translate), &line));
    assert!(at.len() == 5 && near2(at[0], o(15.0, -1.0)) && near2(at[4], o(95.0, -1.0)));
    // Turned with a line going down: the box lies along it.
    let down = self::line(o(0.0, 0.0), o(0.0, 100.0));
    let turned = effected(&stamp(&box_, 50.0, 0.0, StampedPathEffectStyle::Rotate), &down).get_bounds();
    assert!(near_by(turned.left, -1.0, 0.01) && near_by(turned.width(), 2.0, 0.01));
    assert!(near_by(turned.top, 0.0, 0.01) && near_by(turned.bottom, 54.0, 0.01));
    // Bent along a circle (radius 50) as Skia bends it: each side a quadratic through its middle moved onto the path.
    // A 30 long box keeps to the circle instead of running off along its tangent (sqrt(50² + 30²) ≈ 58 at its end);
    // its inner side, 49 out, dips between its ends to the quadratic's middle, 49 (1 + cos 0.3) / 2 ≈ 47.9.
    let mut circle = Path::new();
    circle.add_oval(xywh(0.0, 0.0, 100.0, 100.0), PathDirection::CounterClockwise);
    let long_box = rect_path(0.0, -1.0, 30.0, 2.0);
    let bent = effected(&stamp(&long_box, 80.0, 0.0, StampedPathEffectStyle::Morph), &circle);
    let mut m = measure(&bent);
    let (mut farthest, mut closest, mut reach) = (0.0f32, 100.0f32, 100.0f32);
    loop {
        let first = m.get_position(0.0);
        let mut span = 0.0f32; // how far it reaches from its start: along the circle, nearly its 30
        let mut d = 0.0f32;
        while d <= m.length() {
            let p = m.get_position(d);
            let r = (p.x - 50.0).hypot(p.y - 50.0);
            farthest = farthest.max(r);
            closest = closest.min(r);
            span = span.max((p.x - first.x).hypot(p.y - first.y));
            d += 1.0;
        }
        reach = reach.min(span);
        if !m.next_contour() {
            break;
        }
    }
    assert!(farthest < 51.01 && closest > 47.8 && closest < 48.0 && reach > 28.0);
    // Round a closed contour, its closing side too: 6 stamps on a 160 long square, every 30.
    let mut square = Path::new();
    square.move_to(0.0, 0.0);
    square.line_to(40.0, 0.0);
    square.line_to(40.0, 40.0);
    square.line_to(0.0, 40.0);
    square.close();
    let at = starts(&effected(&stamp(&box_, 30.0, 0.0, StampedPathEffectStyle::Translate), &square));
    assert!(at.len() == 6 && near2(at[5], o(0.0, 10.0 - 1.0)));
    // Rounded corners first, then stamped; drawn filled, not stroked.
    let chained = PathEffect::chain_path_effect(
        &stamp(&box_, 20.0, 0.0, StampedPathEffectStyle::Translate),
        &PathEffect::corner_path_effect(4.0),
    );
    let mut stroke = PathStroke::new(3.0);
    let mut made = Path::new();
    assert!(chained.filter_path(&mut made, &line, &mut stroke, 1.0) && !stroke.strokes() && !made.is_empty());
    let mut drawn = PathStroke {
        path_effect: Some(stamp(&box_, 20.0, 0.0, StampedPathEffectStyle::Translate)),
        ..PathStroke::new(3.0)
    };
    path_effected(&line, &mut drawn, 1.0);
    assert_eq!(drawn.width, 0.0);
}

/// The C++ display_list_damage_reaches_what_effects_and_spreads_draw, its paths: how far stamps reach past a path.
#[test]
fn stamps_reach_past_the_path() {
    let line = line(o(20.0, 100.0), o(380.0, 100.0));
    let box_ = rect_path(-2.0, -30.0, 4.0, 60.0); // across the line, 30 up and down
    let drawn = |phase: f32| {
        // Where the stamps are at `phase`.
        let mut s = PathStroke {
            path_effect: Some(PathEffect::stamped_path_effect(&box_, 40.0, phase, StampedPathEffectStyle::Rotate)),
            ..PathStroke::new(1.0)
        };
        path_effected(&line, &mut s, 1.0).get_bounds()
    };
    assert!(PathEffect::stamped_path_effect(&box_, 40.0, 0.0, StampedPathEffectStyle::Rotate).reach() >= 30.0);
    assert_eq!(PathEffect::dash_path_effect(&[4.0, 4.0], 0.0).reach(), 0.0);
    assert!(drawn(15.0).top < 75.0 && drawn(0.0).top < 75.0);
}

/// Dashes, corners and stamps as Skia makes them; what Skia refuses is no effect; a dash on a curve is measured at
/// the pixels it is drawn at.
#[test]
fn path_effects_are_skias() {
    let line = line(o(0.0, 0.0), o(40.0, 0.0));
    let dash = PathEffect::dash_path_effect;
    // A zero-length dash: a zero-length line, for caps to show (round ones: dots).
    assert_eq!(
        path_text(&effected_with(&dash(&[0.0, 10.0], 0.0), &line, stroke_with(2.0, StrokeCap::Round))),
        "M0,0 L0,0 M10,0 L10,0 M20,0 L20,0 M30,0 L30,0"
    );
    // A negative phase from the pattern's end: -5 of 15 is 10 into it.
    assert!(
        dash(&[10.0, 5.0], -5.0) == dash(&[10.0, 5.0], 10.0) && dash(&[10.0, 5.0], 25.0) == dash(&[10.0, 5.0], 10.0)
    );
    assert!(dash(&[10.0, 5.0], 3.0) != dash(&[10.0, 5.0], 0.0) && dash(&[10.0, 5.0], 0.0) != dash(&[10.0, 6.0], 0.0));
    assert!(PathEffect::corner_path_effect(2.0) != PathEffect::corner_path_effect(3.0));
    assert_eq!(
        path_text(&effected_with(&dash(&[10.0, 5.0], -5.0), &line, stroke_with(1.0, StrokeCap::Round))),
        "M5,0 L15,0 M20,0 L30,0 M35,0 L40,0"
    );
    // What Skia refuses (crbug.com/348821 among them) is none: the path drawn as it is.
    for none in [
        dash(&[], 0.0),
        dash(&[1.0], 0.0),
        dash(&[-1.0, 2.0], 0.0),
        dash(&[0.0, 0.0], 0.0),
        dash(&[1.769_343_6e36, f32::from_bits(2)], f32::INFINITY),
        PathEffect::corner_path_effect(0.0),
        PathEffect::corner_path_effect(-1.0),
        PathEffect::corner_path_effect(f32::NAN),
        PathEffect::stamped_path_effect(&line, 0.0, 0.0, StampedPathEffectStyle::Translate),
        PathEffect::stamped_path_effect(&Path::new(), 10.0, 0.0, StampedPathEffectStyle::Rotate),
    ] {
        let mut out = Path::new();
        let mut stroke = PathStroke::new(1.0);
        assert!(none.is_none() && !none.filter_path(&mut out, &line, &mut stroke, 1.0) && none == PathEffect::none());
    }
    // Chained with none: the other one.
    let corner = PathEffect::corner_path_effect(3.0);
    assert!(PathEffect::chain_path_effect(&PathEffect::none(), &corner) == corner);
    assert!(PathEffect::chain_path_effect(&corner, &PathEffect::none()) == corner);
    // A fill isn't dashed; its corners are rounded.
    let mut square = Path::new();
    square.add_rect(xywh(0.0, 0.0, 20.0, 20.0), PathDirection::Clockwise);
    let mut out = Path::new();
    let mut fill = PathStroke::default();
    assert!(!dash(&[2.0, 2.0], 0.0).filter_path(&mut out, &square, &mut fill, 1.0));
    assert!(corner.filter_path(&mut out, &square, &mut fill, 1.0));
    // Curves go through a corner effect as they are, corners between lines rounded; an open contour's first line
    // begins with a quadratic at its start, and a curve goes on from where the line before it stopped (Skia's: "just
    // replicate the curve for now").
    let mut mixed = Path::new();
    mixed.move_to(0.0, 0.0);
    mixed.line_to(20.0, 0.0);
    mixed.quadratic_to(30.0, 0.0, 30.0, 10.0);
    mixed.line_to(30.0, 30.0);
    mixed.line_to(0.0, 30.0);
    assert_eq!(
        path_text(&effected(&PathEffect::corner_path_effect(4.0), &mixed)),
        "M0,0 Q4,0 L16,0 Q30,10 Q30,14 L30,26 Q26,30 L4,30 L0,30"
    );
    // Measured at the pixels: a dashed circle's (radius 50) dash 300 along it starts nearer its true place (6 radians
    // round) at 3 pixels a unit than at 1.
    let mut circle = Path::new();
    circle.add_oval(xywh(0.0, 0.0, 100.0, 100.0), PathDirection::Clockwise);
    let off = |res_scale: f32| {
        let mut dashes = Path::new();
        let mut stroke = PathStroke::new(1.0);
        dash(&[10.0, 10.0], 0.0).filter_path(&mut dashes, &circle, &mut stroke, res_scale);
        let mut m = measure(&dashes);
        for _ in 0..14 {
            m.next_contour(); // (the first dash, 0 to 10, waits for the end)
        }
        let p = m.get_position(0.0);
        let a = (p.y - 50.0).atan2(p.x - 50.0);
        (a + 2.0 * PI - 6.0).abs()
    };
    assert!(off(3.0) < off(1.0) && off(3.0) < 0.005 && off(1.0) > 0.005);
    // Far too many dashes: Skia gives up (nothing dashed); odd cases neither hang nor fail (DashPath_bug4871,
    // DashCrazy_crbug_875494).
    let long_line = self::line(o(1.0, 1.0), o(1.0, 5.0e10));
    let mut wide = stroke_with(2.0, StrokeCap::Round);
    assert!(!dash(&[1.0, 1.0], 0.0).filter_path(&mut out, &long_line, &mut wide, 1.0) && out.is_empty());
    let mut tiny = Path::new();
    tiny.move_to(30.0, 24.0);
    tiny.cubic_to(30.002, 24.0, 30.0, 24.0, 30.0, 24.0);
    tiny.close();
    let mut stroke = PathStroke::new(1.0);
    out.reset();
    dash(&[1.0, 1.0], 0.0).filter_path(&mut out, &tiny, &mut stroke, 1.0);
    let mut rect = Path::new();
    rect.add_rect(xywh(43.0, 236.0, 57.0, 149.0), PathDirection::Clockwise);
    out.reset();
    assert!(dash(&[98.0, 94.0, 2_888_458_849.0, 227.0, 0.0, 197.0], 222.0).filter_path(
        &mut out,
        &rect,
        &mut stroke,
        1.0
    ));
    // The scale strokes and effects are worked out at (Skia's ComputeResScaleForStroking).
    assert!(res_scale_of(2.0, 0.0, 0.0, 3.0) == 3.0 && res_scale_of(0.0, 1.0, -1.0, 0.0) == 1.0);
    assert!(res_scale_of(0.0, 0.0, 0.0, 0.0) == 1.0 && res_scale_of(f32::INFINITY, 0.0, 0.0, 1.0) == 1.0);
    assert!(res_scale_of(1e6, 0.0, 0.0, 1.0) == 1e5);
    assert!((res_scale_of(3.0, 4.0, 0.0, 1.0) - 5.0).abs() < 1e-6);
}

#[test]
fn path_effects_edge_cases() {
    let mut square = Path::new();
    square.add_rect(xywh(0.0, 0.0, 20.0, 20.0), PathDirection::Clockwise);
    // Dashes come in on/off pairs: an odd last interval is dropped (as Android drops it).
    let odd = PathEffect::dash_path_effect(&[10.0, 5.0, 3.0], 0.0);
    assert_eq!(odd, PathEffect::dash_path_effect(&[10.0, 5.0], 0.0));
    let line = line(o(0.0, 0.0), o(40.0, 0.0));
    assert_eq!(
        path_text(&effected_with(&odd, &line, stroke_with(1.0, StrokeCap::Round))),
        "M0,0 L10,0 M15,0 L25,0 M30,0 L40,0"
    );
    // A closed contour's dashes go round its closing side too.
    assert_eq!(
        path_text(&effected(&PathEffect::dash_path_effect(&[100.0, 1.0], 0.0), &square)),
        "M0,0 L20,0 L20,20 L0,20 L0,0"
    );
    // A corner gives at most half of either side to its curve.
    assert_eq!(
        path_text(&effected(&PathEffect::corner_path_effect(15.0), &square)),
        "M10,0 Q20,10 Q10,20 Q0,10 Q10,0 Z"
    );
    // Going back to the start before close() is the same square.
    let mut back = Path::new();
    back.move_to(0.0, 0.0);
    back.line_to(20.0, 0.0);
    back.line_to(20.0, 20.0);
    back.line_to(0.0, 20.0);
    back.line_to(0.0, 0.0);
    back.close();
    let corner4 = PathEffect::corner_path_effect(4.0);
    assert_eq!(path_text(&effected(&corner4, &back)), path_text(&effected(&corner4, &square)));
    // Chained: rounded corners, then dashed (4 on, 4 off): half of the shorter way round.
    let chain = |a: &PathEffect, b: &PathEffect| PathEffect::chain_path_effect(a, b);
    let (dash4, corner6) = (PathEffect::dash_path_effect(&[4.0, 4.0], 0.0), PathEffect::corner_path_effect(6.0));
    let chained = chain(&dash4, &corner6);
    assert!(chained == chain(&dash4, &corner6) && chained != chain(&corner6, &dash4));
    let rounded = measure(&effected(&corner6, &square));
    let drawn = total_length(&effected(&chained, &square));
    assert!(rounded.length() < 78.0 && near_by(drawn, rounded.length() / 2.0, 2.0));
    // A lone point measures nothing: the first contour with a length is the first one.
    let mut dot = Path::new();
    dot.move_to(5.0, 5.0);
    dot.add_rect(xywh(0.0, 0.0, 10.0, 10.0), PathDirection::CounterClockwise);
    assert_eq!(measure(&dot).length(), 40.0);
}

// ---- Operations ---------------------------------------------------------------------------------------------------

fn contours(p: &Path) -> usize {
    p.commands().iter().filter(|c| is_move(c)).count()
}

#[test]
fn path_op_combines_shapes() {
    let rect = rect_path;
    let a = rect(0.0, 0.0, 100.0, 100.0);
    let b = rect(50.0, 50.0, 100.0, 100.0);
    // Union: one outline round both; inside either, not in the corners outside them.
    let u = Path::combine(PathOperation::Union, &a, &b);
    assert!(contours(&u) == 1 && u.contains(o(25.0, 25.0)) && u.contains(o(125.0, 125.0)));
    assert!(u.contains(o(75.0, 75.0)) && !u.contains(o(125.0, 25.0)));
    assert!(near(u.get_bounds().width(), 150.0) && near(u.get_bounds().height(), 150.0));
    // Intersect: just the overlap.
    let i = &a & &b;
    assert!(contours(&i) == 1 && i.contains(o(75.0, 75.0)) && !i.contains(o(25.0, 25.0)));
    assert!(near(i.get_bounds().left, 50.0) && near(i.get_bounds().width(), 50.0));
    // Difference and the reverse.
    let d = &a - &b;
    let r = Path::combine(PathOperation::ReverseDifference, &a, &b);
    assert!(d.contains(o(25.0, 25.0)) && !d.contains(o(75.0, 75.0)) && !d.contains(o(125.0, 125.0)));
    assert!(r.contains(o(125.0, 125.0)) && !r.contains(o(75.0, 75.0)) && !r.contains(o(25.0, 25.0)));
    // Xor: two pieces (touching at two corners, each its own contour), the overlap left out.
    let x = &a ^ &b;
    assert!(
        contours(&x) == 2 && x.contains(o(25.0, 25.0)) && x.contains(o(125.0, 125.0)) && !x.contains(o(75.0, 75.0))
    );
    // A hole: a smaller square taken out of the middle.
    let holed = &a - &rect(25.0, 25.0, 50.0, 50.0);
    assert!(contours(&holed) == 2 && holed.contains(o(10.0, 10.0)) && !holed.contains(o(50.0, 50.0)));
    // Side by side, sharing a side: one rectangle, the shared side gone.
    let joined = rect(0.0, 0.0, 50.0, 50.0) | rect(50.0, 0.0, 50.0, 50.0);
    assert!(contours(&joined) == 1 && joined.contains(o(50.0, 25.0)) && near(joined.get_bounds().width(), 100.0));
    // Overlapping along their top and bottom sides: those split where the other's end, one outline.
    let along = rect(0.0, 0.0, 100.0, 50.0) | rect(50.0, 0.0, 100.0, 50.0);
    assert!(contours(&along) == 1 && along.contains(o(125.0, 25.0)) && along.contains(o(25.0, 25.0)));
    assert!(near(along.get_bounds().width(), 150.0));
    assert!(near_by(measure(&along).length(), 400.0, 0.1)); // its outline only: nothing drawn twice when stroked
    // The same shape twice: itself.
    let twice = Path::combine(PathOperation::Intersect, &a, &a);
    assert!(contours(&twice) == 1 && twice.contains(o(50.0, 50.0)) && near(twice.get_bounds().width(), 100.0));
    assert!(!Path::new().op(&a, &a, PathOperation::Difference)); // nothing left
    // Curves: a circle with a bite out of it.
    let mut circle = Path::new();
    circle.add_oval(xywh(0.0, 0.0, 100.0, 100.0), PathDirection::CounterClockwise);
    let bitten = &circle - &rect(50.0, 50.0, 100.0, 100.0);
    assert!(bitten.contains(o(25.0, 50.0)) && !bitten.contains(o(75.0, 75.0)));
    assert!(bitten.contains(o(75.0, 25.0)) && !bitten.contains(o(2.0, 2.0)));
    // Even-odd input: an inner square counts as a hole of the outer one.
    let mut even_odd = Path::new();
    even_odd.fill_type = PathFillType::EvenOdd;
    even_odd.add_rect(xywh(0.0, 0.0, 100.0, 100.0), PathDirection::CounterClockwise);
    even_odd.add_rect(xywh(25.0, 25.0, 50.0, 50.0), PathDirection::CounterClockwise);
    assert!(!even_odd.contains(o(50.0, 50.0)) && even_odd.contains(o(10.0, 10.0)));
    let filled = &even_odd | &rect(40.0, 40.0, 20.0, 20.0); // a smaller square back in the hole
    assert!(filled.contains(o(50.0, 50.0)) && !filled.contains(o(30.0, 30.0)) && filled.contains(o(10.0, 10.0)));
}

#[test]
fn path_ops_keep_curves() {
    let on_circle = |p: Offset, c: Offset, r: f32| ((p.x - c.x).hypot(p.y - c.y) - r).abs() < 0.02;
    let on_either = |p: Offset| on_circle(p, o(50.0, 50.0), 50.0) || on_circle(p, o(100.0, 50.0), 50.0);
    let conic_mid = |g: &PathSegment| {
        let (w, p) = (g.weight, &g.points);
        o(
            (p[0].x + 2.0 * w * p[1].x + p[2].x) / (2.0 + 2.0 * w),
            (p[0].y + 2.0 * w * p[1].y + p[2].y) / (2.0 + 2.0 * w),
        )
    };
    let (mut a, mut b) = (Path::new(), Path::new());
    a.add_oval(xywh(0.0, 0.0, 100.0, 100.0), PathDirection::CounterClockwise);
    b.add_oval(xywh(50.0, 0.0, 100.0, 100.0), PathDirection::CounterClockwise);
    // Union: conics only (no lines), every point on one circle or the other; inside as either is.
    let u = Path::combine(PathOperation::Union, &a, &b);
    let mut curves_only = true;
    let mut on_one = true;
    for c in u.commands() {
        if is_line(c) {
            curves_only = false;
        }
        if is_move(c) || is_conic(c) {
            on_one = on_one && on_either(end(c));
        }
    }
    // Each conic's middle on its circle too (its weight right, not just its ends).
    for g in u.iterator_with(ConicEvaluation::AsConic, 0.25) {
        if g.r#type == PathSegmentType::Conic {
            on_one = on_one && on_either(conic_mid(&g));
        }
    }
    assert!(curves_only && on_one && !u.is_empty());
    let mut x = -5.0f32;
    while x <= 155.0 {
        let mut y = -5.0f32;
        while y <= 105.0 {
            let p = o(x, y);
            if !on_either(p) {
                assert_eq!(u.contains(p), a.contains(p) || b.contains(p), "at {p:?}");
            }
            y += 6.1;
        }
        x += 7.3;
    }
    // Intersect and xor, against the operands; the lens is two arcs.
    let lens = Path::combine(PathOperation::Intersect, &a, &b);
    let xored = Path::combine(PathOperation::Xor, &a, &b);
    let arcs = lens.commands().iter().filter(|c| is_conic(c)).count();
    assert!((2..=4).contains(&arcs));
    let mut x = 1.0f32;
    while x < 150.0 {
        let mut y = 1.0f32;
        while y < 100.0 {
            let p = o(x, y);
            if !on_either(p) {
                assert_eq!(lens.contains(p), a.contains(p) && b.contains(p), "at {p:?}");
                assert_eq!(xored.contains(p), a.contains(p) != b.contains(p), "at {p:?}");
            }
            y += 8.3;
        }
        x += 9.7;
    }
    // The middle of an arc kept (a piece between two places inside one conic): still on its circle.
    let corner = rect_path(80.0, 2.0, 40.0, 36.0);
    let bite = Path::combine(PathOperation::Intersect, &a, &corner);
    let c50 = o(50.0, 50.0);
    let mut middle_on = false;
    for g in bite.iterator_with(ConicEvaluation::AsConic, 0.25) {
        if g.r#type == PathSegmentType::Conic {
            middle_on = on_circle(g.points[0], c50, 50.0)
                && on_circle(g.points[2], c50, 50.0)
                && on_circle(conic_mid(&g), c50, 50.0);
        }
    }
    assert!(middle_on);
    // Where the shapes only touch: a square less the oval inside it, touching its sides, is three pieces, each
    // bounded by lines and a quarter of the ellipse turned round.
    let square = rect_path(0.0, 0.0, 20.0, 20.0);
    let mut inner = Path::new();
    inner.add_oval(xywh(0.0, 0.0, 20.0, 10.0), PathDirection::CounterClockwise);
    let d = Path::combine(PathOperation::Difference, &square, &inner);
    let quarter_arcs = d.commands().iter().filter(|c| is_conic(c) && near_by(weight(c), FRAC_1_SQRT_2, 1e-4)).count();
    assert!(contours(&d) == 3 && quarter_arcs == 4);
    // Cubics keep theirs: a piece of each, still a cubic.
    let mut wave = Path::new();
    wave.move_to(0.0, 50.0);
    wave.cubic_to(30.0, -20.0, 70.0, 120.0, 100.0, 50.0);
    wave.line_to(100.0, 100.0);
    wave.line_to(0.0, 100.0);
    wave.close();
    let band = rect_path(0.0, 0.0, 100.0, 60.0);
    let both = Path::combine(PathOperation::Intersect, &wave, &band);
    assert!(both.commands().iter().any(is_cubic));
    let mut x = 2.0f32;
    while x < 100.0 {
        let mut y = 2.0f32;
        while y < 100.0 {
            let p = o(x, y);
            assert!(both.contains(p) == (wave.contains(p) && band.contains(p)) || (y - 60.0).abs() < 0.05, "at {p:?}");
            y += 5.9;
        }
        x += 6.7;
    }
}

// ---- Measures -----------------------------------------------------------------------------------------------------

#[test]
fn path_measure_lengths_positions_and_segments() {
    let mut square = Path::new(); // (clockwise: along the top first)
    square.add_rect(xywh(0.0, 0.0, 10.0, 10.0), PathDirection::Clockwise);
    square.add_rect(xywh(20.0, 0.0, 5.0, 5.0), PathDirection::Clockwise);
    let mut m = measure(&square);
    assert_eq!(m.length(), 40.0);
    assert!(m.get_position(15.0) == o(10.0, 5.0) && m.get_tangent(15.0) == o(0.0, 1.0));
    assert_eq!(m.get_position(100.0), o(0.0, 0.0)); // past the end: at the end (back at the start)
    // Before the start: at the start.
    assert!(m.get_position(-5.0) == o(0.0, 0.0) && m.get_tangent(-5.0) == o(1.0, 0.0));
    let mut piece = Path::new();
    assert!(m.get_segment(5.0, 25.0, &mut piece, true));
    assert_eq!(path_text(&piece), "M5,0 L10,0 L10,10 L5,10");
    // Not from a move: going on from where it was, no line to its start (a zero-length one where it starts on a
    // corner, as Skia's).
    assert!(!m.get_segment(20.0, 20.0, &mut piece, true) && m.get_segment(30.0, 35.0, &mut piece, false));
    assert!(path_text(&piece).ends_with("L5,10 L5,10 L0,5"));
    assert!(m.next_contour() && m.length() == 20.0 && !m.next_contour());
    let mut round = Path::new();
    round.add_oval(xywh(0.0, 0.0, 20.0, 20.0), PathDirection::CounterClockwise);
    // Its curves cut until each piece is within half a unit of a line: a circle of 10 a little short; at 4 pixels a
    // unit, within an eighth of one.
    assert!(near_by(measure(&round).length(), 62.427, 0.001));
    assert!(near_by(measure_with(&round, false, 4.0).length(), 62.730, 0.001));
    let mut corner = Path::new();
    corner.move_to(0.0, 0.0);
    corner.line_to(10.0, 0.0);
    corner.line_to(10.0, 10.0);
    assert!(
        measure(&corner).length() == 20.0 && near_by(measure_with(&corner, true, 1.0).length(), 20.0 + 14.142, 0.01)
    );
    assert!(PathMeasure::new().length() == 0.0 && measure(&Path::new()).get_position(3.0).x.is_nan());
    assert!(PathMeasure::new().get_tangent(0.0).y.is_nan());
}

/// Skia's PathMeasureTest, ported: lengths, positions and tangents, degenerate pieces, contours of no length skipped,
/// a close not followed by a move; and its contour_measure_verbs' distances.
#[test]
fn path_measure_is_skias() {
    let nearly = |a: f32, b: f32| (a - b).abs() <= 1.0 / 4096.0; // (SkScalarNearlyEqual)
    let nearly_by = |a: f32, b: f32, tolerance: f32| (a - b).abs() <= tolerance;
    let mut square = Path::new();
    square.move_to(0.0, 0.0);
    square.line_to(1.0, 0.0);
    square.line_to(1.0, 1.0);
    square.line_to(0.0, 1.0);
    assert!(measure_with(&square, true, 1.0).length() == 4.0 && measure(&square).length() == 3.0);
    assert!(measure_with(&square, true, 1.0).is_closed());
    let line = line(o(0.0, 0.0), o(3.0, 4.0));
    assert!(measure(&line).length() == 5.0 && !measure(&line).is_closed());
    // A close not followed by a move: drawing on starts from the contour's start.
    let mut closed = Path::new();
    closed.line_to(1.0, 0.0);
    closed.line_to(1.0, 1.0);
    closed.line_to(0.0, 1.0);
    closed.close();
    closed.line_to(-1.0, 0.0);
    let mut m = measure(&closed);
    assert!(m.length() == 4.0 && m.is_closed() && m.next_contour() && m.length() == 1.0 && !m.is_closed());
    let (p, t) = (m.get_position(0.5), m.get_tangent(0.5));
    assert!(nearly_by(p.x, -0.5, 0.0001) && p.y == 0.0 && t == o(-1.0, 0.0));
    // Degenerate pieces (a zero-length line, quadratic and cubic) measure nothing.
    let mut degenerate = Path::new();
    degenerate.move_to(0.0, 0.0);
    degenerate.line_to(0.0, 0.0);
    degenerate.line_to(1.0, 0.0);
    degenerate.quadratic_to(1.0, 0.0, 1.0, 0.0);
    degenerate.quadratic_to(1.0, 1.0, 1.0, 2.0);
    degenerate.cubic_to(1.0, 2.0, 1.0, 2.0, 1.0, 2.0);
    degenerate.cubic_to(2.0, 2.0, 3.0, 2.0, 4.0, 2.0);
    m.set_path(&degenerate, false);
    assert_eq!(m.length(), 6.0);
    let (p, t) = (m.get_position(0.5), m.get_tangent(0.5));
    assert!(nearly_by(p.x, 0.5, 0.0001) && p.y == 0.0 && t == o(1.0, 0.0));
    let (p, t) = (m.get_position(2.5), m.get_tangent(2.5));
    assert!(nearly_by(p.x, 1.0, 0.0001) && nearly(p.y, 1.5) && t == o(0.0, 1.0));
    let (p, t) = (m.get_position(4.5), m.get_tangent(4.5));
    assert!(nearly_by(p.x, 2.5, 0.0001) && nearly_by(p.y, 2.0, 0.0001) && t == o(1.0, 0.0));
    // Moves in between: the last one starts the next contour.
    let mut moves = Path::new();
    moves.move_to(0.0, 0.0);
    moves.line_to(1.0, 0.0);
    moves.move_to(1.0, 1.0);
    moves.move_to(2.0, 2.0);
    moves.line_to(1.0, 2.0);
    m.set_path(&moves, false);
    let (p, t) = (m.get_position(0.5), m.get_tangent(0.5));
    assert!(m.length() == 1.0 && nearly_by(p.x, 0.5, 0.0001) && p.y == 0.0 && t == o(1.0, 0.0));
    assert!(m.next_contour() && m.length() == 1.0);
    let (p, t) = (m.get_position(0.5), m.get_tangent(0.5));
    assert!(nearly_by(p.x, 1.5, 0.0001) && nearly_by(p.y, 2.0, 0.0001) && t == o(-1.0, 0.0) && !m.next_contour());
    // Huge and tiny segments neither hang nor go wrong (test_small_segment, 2 and 3).
    let mut small = Path::new();
    small.move_to(100000.0, 100000.0);
    small.line_to(1.0005, 0.9999);
    small.line_to(1.0, 1.0);
    let mut quads = Path::new();
    quads.quadratic_to(100_000_000_000.0, 100_000_000_000.0, 0.0, 0.0);
    quads.quadratic_to(10.0, 10.0, 0.0, 0.0);
    let mut cubics = Path::new();
    cubics.cubic_to(100_000_000_000.0, 100_000_000_000.0, 0.0, 0.0, 10.0, 10.0);
    cubics.cubic_to(10.0, 10.0, 0.0, 0.0, 10.0, 10.0);
    assert!(measure(&small).length() > 0.0 && measure(&quads).length().is_finite());
    assert!(measure_with(&cubics, false, 1000.0).length().is_finite());
    // A conic of weight 1 and of 10 along a line: a point 20 along is at 20.
    let mut conic = Path::new();
    conic.conic_to(100.0, 0.0, 100.0, 0.0, 1.0);
    let mut heavy = Path::new();
    heavy.conic_to(100.0, 0.0, 100.0, 0.0, 10.0);
    assert!((measure(&conic).get_position(20.0).x - 20.0).abs() < 0.5);
    assert!((measure(&heavy).get_position(20.0).x - 20.0).abs() < 0.5);
    assert!(!measure(&line).next_contour()); // (PathMeasure_nextctr)
    // Two circles, each a closed contour: a quarter round, at the bottom going left.
    let mut circles = Path::new();
    circles.add_oval(xywh(-100.0, -100.0, 200.0, 200.0), PathDirection::Clockwise);
    circles.add_oval(xywh(-10.0, -10.0, 20.0, 20.0), PathDirection::Clockwise);
    m.set_path(&circles, false);
    for r in [100.0f32, 10.0] {
        let (p, t) = (m.get_position(m.length() / 4.0), m.get_tangent(m.length() / 4.0));
        assert!(m.is_closed() && nearly_by(m.length(), 2.0 * r * PI, if r == 100.0 { 1.5 } else { 0.5 }));
        assert!(nearly(p.x, 0.0) && nearly(p.y, r) && nearly(t.x, -1.0) && nearly(t.y, 0.0));
        m.next_contour();
    }
    assert_eq!(m.length(), 0.0);
    // Contours of no length are skipped (test_empty_contours, test_MLM_contours).
    let mut empty = Path::new();
    empty.move_to(0.0, 0.0);
    empty.line_to(100.0, 100.0);
    empty.line_to(200.0, 100.0);
    empty.move_to(2.0, 2.0);
    empty.move_to(3.0, 3.0);
    empty.move_to(4.0, 4.0);
    empty.close();
    empty.close();
    empty.close();
    empty.move_to(5.0, 5.0);
    empty.line_to(5.0, 5.0);
    empty.move_to(5.0, 5.0);
    empty.line_to(5.0, 5.0);
    empty.close();
    empty.move_to(5.0, 5.0);
    empty.line_to(5.0, 5.0);
    empty.close();
    empty.close();
    empty.move_to(6.0, 6.0);
    empty.line_to(7.0, 7.0);
    empty.move_to(10.0, 10.0);
    m.set_path(&empty, false);
    assert!(m.length() > 0.0 && m.next_contour() && nearly(m.length(), 2f32.sqrt()) && !m.next_contour());
    let mut trailing = Path::new();
    trailing.move_to(10.0, 10.0);
    trailing.line_to(20.0, 20.0);
    trailing.move_to(30.0, 30.0);
    for force_closed in [false, true] {
        m.set_path(&trailing, force_closed);
        assert!(m.length() > 0.0 && !m.next_contour());
    }
    // contour_measure_verbs: the distance at the end of each piece, as Skia measures it.
    let mut verbs = Path::new();
    verbs.move_to(10.0, 10.0);
    verbs.line_to(10.0, 30.0);
    verbs.line_to(30.0, 30.0);
    verbs.quadratic_to(40.0, 30.0, 40.0, 40.0);
    assert!(nearly(measure(&verbs).length(), 56.127_525));
    verbs.cubic_to(50.0, 40.0, 50.0, 50.0, 40.0, 50.0);
    assert!(nearly(measure(&verbs).length(), 76.004_69));
    verbs.conic_to(50.0, 50.0, 50.0, 60.0, 1.2);
    assert!(nearly(measure(&verbs).length(), 92.428_185));
}

/// Pieces of curves are the curves cut there (Skia's segTo): a quadratic's piece a quadratic, a cubic's a cubic, a
/// conic's a conic (its weight what that piece needs), their ends where the measure puts those distances.
#[test]
fn path_measure_segments_cut_curves() {
    let close = |a: Offset, b: Offset| (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3;
    let mut quad = Path::new();
    quad.move_to(0.0, 0.0);
    quad.quadratic_to(50.0, 100.0, 100.0, 0.0);
    let mut m = measure(&quad);
    let len = m.length();
    let mut piece = Path::new();
    assert!(m.get_segment(len / 4.0, len / 2.0, &mut piece, true));
    let q = piece.commands();
    assert!(q.len() >= 2 && is_move(&q[0]) && close(end(&q[0]), m.get_position(len / 4.0)));
    assert!(is_quad(&q[q.len() - 1]) && close(end(&q[q.len() - 1]), m.get_position(len / 2.0)));
    // A middle piece of a quadratic: its control point where the curve's tangents at its ends meet.
    assert_eq!(q.len(), 2);
    let (a, c, b) = (end(&q[0]), c1(&q[1]), end(&q[1]));
    let (ta, tb) = (m.get_tangent(len / 4.0), m.get_tangent(len / 2.0));
    let cross1 = (c.x - a.x) * ta.y - (c.y - a.y) * ta.x;
    let cross2 = (c.x - b.x) * tb.y - (c.y - b.y) * tb.x;
    assert!(cross1.abs() < 0.01 && cross2.abs() < 0.01);
    let mut cubic = Path::new();
    cubic.move_to(0.0, 0.0);
    cubic.cubic_to(0.0, 100.0, 100.0, 100.0, 100.0, 0.0);
    m.set_path(&cubic, false);
    let len = m.length();
    piece.reset();
    assert!(m.get_segment(0.0, len / 3.0, &mut piece, true) && is_cubic(piece.commands().last().unwrap()));
    assert!(close(last_end(&piece), m.get_position(len / 3.0)));
    piece.reset();
    assert!(m.get_segment(len / 3.0, len, &mut piece, true) && is_cubic(piece.commands().last().unwrap()));
    assert_eq!(last_end(&piece), o(100.0, 0.0));
    // A cubic's tangent at an end whose control point is on it: towards the next control point (Skia's), at the other
    // end from the one before; with both on it, end to end.
    let tangent_at = |c1: Offset, c2: Offset, e: Offset, at_end: bool| {
        let mut p = Path::new();
        p.move_to(0.0, 0.0);
        p.cubic_to(c1.x, c1.y, c2.x, c2.y, e.x, e.y);
        let pm = measure(&p);
        pm.get_tangent(if at_end { pm.length() } else { 0.0 })
    };
    let t0 = tangent_at(o(0.0, 0.0), o(100.0, 100.0), o(100.0, 0.0), false);
    let t1 = tangent_at(o(0.0, 100.0), o(100.0, 0.0), o(100.0, 0.0), true);
    let both = tangent_at(o(0.0, 0.0), o(0.0, 0.0), o(0.0, 100.0), false);
    assert!(
        close(t0, o(FRAC_1_SQRT_2, FRAC_1_SQRT_2))
            && close(t1, o(FRAC_1_SQRT_2, -FRAC_1_SQRT_2))
            && close(both, o(0.0, 1.0))
    );
    // A circle's quarter (a conic of weight cos 45°): its piece from 15° to 75°, a conic of cos 30°.
    let mut round = Path::new();
    round.add_oval(xywh(-10.0, -10.0, 20.0, 20.0), PathDirection::Clockwise);
    m.set_path(&round, false);
    let len = m.length();
    piece.reset();
    assert!(m.get_segment(len / 24.0, len * 5.0 / 24.0, &mut piece, true));
    let k = piece.commands();
    assert!(k.len() == 2 && is_conic(&k[1]) && (weight(&k[1]) - (PI / 6.0).cos()).abs() < 0.01);
    // From a quarter's start, and to its end (a conic cut in two): 30° pieces, conics of cos 15°.
    for (from, to) in [(0.0, len / 12.0), (len / 6.0, len / 4.0)] {
        piece.reset();
        assert!(m.get_segment(from, to, &mut piece, true) && piece.commands().len() == 2);
        assert!(is_conic(&piece.commands()[1]) && (weight(&piece.commands()[1]) - (PI / 12.0).cos()).abs() < 0.002);
    }
    // Across pieces: each cut where it starts or stops, the ones between whole.
    piece.reset();
    assert!(m.get_segment(len / 8.0, len * 7.0 / 8.0, &mut piece, true) && piece.commands().len() == 5);
    assert_eq!(end(&piece.commands()[2]), o(-10.0, 0.0));
}

// ---- Iterators and hit testers ------------------------------------------------------------------------------------

#[test]
fn path_iterators_give_compose_segments() {
    let mut oval = Path::new();
    oval.add_oval(xywh(0.0, 0.0, 20.0, 10.0), PathDirection::CounterClockwise);
    oval.move_to(30.0, 0.0);
    oval.line_to(40.0, 0.0);
    oval.cubic_to(40.0, 5.0, 35.0, 10.0, 30.0, 10.0);
    // As conics: the move, four conics from where each starts, the close; then the line and the cubic.
    let mut conics = oval.iterator_with(ConicEvaluation::AsConic, 0.25);
    assert!(conics.calculate_size(true) == 9 && conics.conic_evaluation() == ConicEvaluation::AsConic);
    let mut got = Vec::new();
    while conics.has_next() {
        got.push(conics.next().unwrap());
    }
    assert!(got.len() == 9 && conics.next().is_none());
    assert_eq!(got[0], PathSegment::new(PathSegmentType::Move, vec![o(20.0, 5.0)], 0.0));
    assert!(got[1].r#type == PathSegmentType::Conic && got[1].points == vec![o(20.0, 5.0), o(20.0, 0.0), o(10.0, 0.0)]);
    assert_eq!(got[1].weight, FRAC_1_SQRT_2);
    assert!(
        got[5].r#type == PathSegmentType::Close && got[5].points.is_empty() && got[6].r#type == PathSegmentType::Move
    );
    assert_eq!(got[7], PathSegment::new(PathSegmentType::Line, vec![o(30.0, 0.0), o(40.0, 0.0)], 0.0));
    assert!(got[8].r#type == PathSegmentType::Cubic && got[8].points.len() == 4);
    // As quadratics (the default): each conic as Skia approximates it within the tolerance, more of them finer.
    let quads = oval.iterator();
    let coarse = quads.calculate_size(true);
    let fine = oval.iterator_with(ConicEvaluation::AsQuadratics, 0.001).calculate_size(true);
    assert!(quads.conic_evaluation() == ConicEvaluation::AsQuadratics && quads.tolerance() == 0.25);
    assert!(coarse > 9 && fine > coarse && quads.calculate_size(false) == 9);
    let mut n = 0;
    let mut worst = 0.0f32;
    let mut last = Offset::Zero;
    for s in quads {
        n += 1;
        assert_ne!(s.r#type, PathSegmentType::Conic);
        if s.r#type == PathSegmentType::Quadratic {
            // Each one starts where the last ended, ends on the ellipse.
            assert!(near_by(s.points[0].x, last.x, 1e-4) && near_by(s.points[0].y, last.y, 1e-4));
            let e = s.points[2];
            worst = worst.max((((e.x - 10.0) / 10.0).hypot((e.y - 5.0) / 5.0) - 1.0).abs());
        }
        if let Some(&p) = s.points.last() {
            last = p;
        }
    }
    assert!(n == coarse && worst < 1e-4);
}

#[test]
fn path_hit_testers_are_composes() {
    // An open contour isn't closed, a point on an edge isn't inside (Compose's), where contains() closes it and
    // counts the edge (Skia's); the fill type counts.
    let mut open = Path::new();
    open.move_to(0.0, 0.0);
    open.line_to(20.0, 0.0);
    open.line_to(20.0, 20.0);
    open.line_to(0.0, 20.0);
    let mut tester = PathHitTester::with_path(&open, PathHitTester::DefaultTolerance);
    assert!(open.contains(o(10.0, 10.0)) && !tester.contains(o(10.0, 10.0)));
    let mut box_ = rect_path(0.0, 0.0, 20.0, 20.0);
    box_.add_rect(xywh(5.0, 5.0, 10.0, 10.0), PathDirection::CounterClockwise);
    tester.update_path(&box_, 0.5);
    assert!(tester.contains(o(2.0, 2.0)) && tester.contains(o(10.0, 10.0)));
    assert!(!tester.contains(o(25.0, 10.0)) && !tester.contains(o(20.0, 10.0)));
    box_.fill_type = PathFillType::EvenOdd;
    tester.update_path(&box_, 0.5);
    assert!(tester.contains(o(2.0, 2.0)) && !tester.contains(o(10.0, 10.0)));
    assert!(box_.contains(o(0.0, 10.0)) && !PathHitTester::with_path(&box_, 0.5).contains(o(0.0, 10.0)));
    // Its conics as quadratics within the tolerance: a circle's edge a hair inside, inside.
    let mut round = Path::new();
    round.add_oval(xywh(0.0, 0.0, 200.0, 200.0), PathDirection::CounterClockwise);
    let fine = PathHitTester::with_path(&round, 0.001);
    let d45 = FRAC_1_SQRT_2;
    assert!(fine.contains(o(100.0 + 99.9 * d45, 100.0 + 99.9 * d45)));
    assert!(!fine.contains(o(100.0 + 100.1 * d45, 100.0 + 100.1 * d45)));
    assert!(!PathHitTester::new().contains(o(0.0, 0.0)));
}

// ---- Strokes ------------------------------------------------------------------------------------------------------

/// The C++ GPU tests' curvesAreFlattenedClosely: curves cut into lines stay within the tolerance of the curve, where
/// they are drawn.
#[test]
fn curves_are_flattened_closely() {
    let mut circle = Path::new();
    circle.add_oval(xywh(0.0, 0.0, 100.0, 100.0), PathDirection::CounterClockwise);
    for scale in [1.0f32, 2.625, 4.0] {
        let contours = flatten_path(&circle, DEFAULT_FLATTEN_TOLERANCE, |p| p * scale);
        assert_eq!(contours.len(), 1);
        let r = 50.0 * scale;
        let pts = &contours[0].points;
        let mut worst = 0.0f32;
        for w in pts.windows(2) {
            // Each line's middle, the farthest from the arc.
            let m = o((w[0].x + w[1].x) / 2.0 - r, (w[0].y + w[1].y) / 2.0 - r);
            worst = worst.max((m.x.hypot(m.y) - r).abs());
        }
        // (The oval is conics, its points on the circle.)
        assert!(worst <= DEFAULT_FLATTEN_TOLERANCE + 0.05, "{worst} at {scale}");
    }
    // Finer when asked (a transform enlarges it after): within that.
    let fine = flatten_path(&circle, 0.02, |p| p);
    let mut worst = 0.0f32;
    for w in fine[0].points.windows(2) {
        let m = o((w[0].x + w[1].x) / 2.0 - 50.0, (w[0].y + w[1].y) / 2.0 - 50.0);
        worst = worst.max((m.x.hypot(m.y) - 50.0).abs());
    }
    assert!(worst <= 0.02 + 0.02);
    assert!(fine[0].closed);
}

/// The C++ GPU tests' strokesCoverOnce, its outlines: narrow Vs of short legs, either way round, and a thin triangle
/// stroked either way round: there the inner offsets would meet past the segments, so the outline goes through the
/// vertices; every point of it lies in what the stroke covers (within half its width of a segment).
#[test]
fn stroke_outlines_stay_within_the_stroke() {
    let within = |mut pts: Vec<Offset>, closed: bool| {
        let outline = stroke_outline(
            &[Contour { points: pts.clone(), closed }],
            10.0,
            StrokeCap::Butt,
            StrokeJoin::Bevel,
            4.0,
            DEFAULT_FLATTEN_TOLERANCE,
        );
        if closed {
            pts.push(pts[0]);
        }
        let mut farthest = 0.0f32;
        for c in &outline {
            for &q in &c.points {
                let mut nearest = f32::INFINITY;
                for s in pts.windows(2) {
                    let (a, b) = (s[0], s[1]);
                    let t = (((q.x - a.x) * (b.x - a.x) + (q.y - a.y) * (b.y - a.y))
                        / ((b.x - a.x) * (b.x - a.x) + (b.y - a.y) * (b.y - a.y)))
                        .clamp(0.0, 1.0);
                    nearest = nearest.min((q.x - (a.x + (b.x - a.x) * t)).hypot(q.y - (a.y + (b.y - a.y) * t)));
                }
                farthest = farthest.max(nearest);
            }
        }
        !outline.is_empty() && farthest <= 5.001
    };
    assert!(within(vec![o(17.0, 30.0), o(20.0, 8.0), o(22.0, 30.0)], false));
    assert!(within(vec![o(22.0, 30.0), o(20.0, 8.0), o(17.0, 30.0)], false));
    assert!(within(vec![o(17.0, 40.0), o(20.0, 6.0), o(23.0, 40.0)], true));
    assert!(within(vec![o(23.0, 40.0), o(20.0, 6.0), o(17.0, 40.0)], true));
}

/// A stroke's fill path covers what the stroke does: half its width each side, its caps and joins; a hairline a
/// pixel wide; a fill the path itself; a dashed line its dashes.
#[test]
fn strokes_become_fills() {
    let l = line(o(0.0, 0.0), o(40.0, 0.0));
    let butt = PathStroke::new(4.0).fill_path(&l, 1.0);
    assert!(butt.contains(o(20.0, 1.9)) && !butt.contains(o(20.0, 2.1)) && !butt.contains(o(-1.0, 0.0)));
    let b = butt.get_bounds();
    assert!(near_by(b.left, 0.0, 1e-4) && near_by(b.right, 40.0, 1e-4) && near_by(b.top, -2.0, 1e-4));
    let square = PathStroke { cap: StrokeCap::Square, ..PathStroke::new(4.0) }.fill_path(&l, 1.0);
    assert!(near_by(square.get_bounds().left, -2.0, 1e-4) && near_by(square.get_bounds().right, 42.0, 1e-4));
    let round = PathStroke { cap: StrokeCap::Round, ..PathStroke::new(4.0) }.fill_path(&l, 1.0);
    assert!(round.contains(o(-1.9, 0.0)) && !round.contains(o(-1.5, 1.5)) && round.contains(o(41.0, 1.0)));
    // A square stroked: its corners mitered, its middle left out (two loops, opposite ways round).
    let mut rect = Path::new();
    rect.add_rect(xywh(0.0, 0.0, 20.0, 20.0), PathDirection::Clockwise);
    let mitered = PathStroke::new(2.0).fill_path(&rect, 1.0);
    assert!(mitered.contains(o(-0.9, -0.9)) && mitered.contains(o(10.0, 0.5)) && !mitered.contains(o(10.0, 10.0)));
    let beveled = PathStroke { join: StrokeJoin::Bevel, ..PathStroke::new(2.0) }.fill_path(&rect, 1.0);
    assert!(!beveled.contains(o(-0.9, -0.9)) && beveled.contains(o(-0.4, -0.4)));
    // A miter longer than the limit is beveled: a square's corner's miter is sqrt 2 times half the width.
    let limited = PathStroke { miter: 1.4, ..PathStroke::new(2.0) }.fill_path(&rect, 1.0);
    assert!(!limited.contains(o(-0.9, -0.9)));
    // A hairline: a pixel wide at 2 pixels a unit is half a unit.
    let hairline = PathStroke { hairline: true, ..PathStroke::default() }.fill_path(&l, 2.0);
    assert!(hairline.contains(o(20.0, 0.24)) && !hairline.contains(o(20.0, 0.26)));
    // A fill: the path itself; with an effect, what the effect made.
    assert_eq!(PathStroke::default().fill_path(&rect, 1.0), rect);
    let dashed =
        PathStroke { path_effect: Some(PathEffect::dash_path_effect(&[10.0, 10.0], 0.0)), ..PathStroke::new(2.0) };
    let dashes = dashed.fill_path(&l, 1.0);
    assert!(dashes.contains(o(5.0, 0.0)) && !dashes.contains(o(15.0, 0.0)) && dashes.contains(o(25.0, 0.0)));
    // A point with round caps: a dot; with butt ones: nothing.
    let dot = line(o(5.0, 5.0), o(5.0, 5.0));
    assert!(PathStroke { cap: StrokeCap::Round, ..PathStroke::new(4.0) }.fill_path(&dot, 1.0).contains(o(6.5, 5.0)));
    assert!(PathStroke::new(4.0).fill_path(&dot, 1.0).is_empty());
}

// ---- Outlines -----------------------------------------------------------------------------------------------------

#[test]
fn outlines_are_composes() {
    let rect = xywh(0.0, 0.0, 20.0, 10.0);
    let rounded = RoundRect::from_rect_radius(rect, CornerRadius::circular(4.0));
    let mut triangle = Path::new();
    triangle.move_to(10.0, 0.0);
    triangle.line_to(20.0, 10.0);
    triangle.line_to(0.0, 10.0);
    triangle.close();
    assert_eq!(Outline::Rectangle(rect).bounds(), rect);
    assert_eq!(Outline::Rounded(rounded).bounds(), rect);
    assert_eq!(Outline::Generic(triangle.clone()).bounds(), Rect::new(0.0, 0.0, 20.0, 10.0));
    // Added to a path: a rect's or a rounded rect's (counterclockwise), or the path.
    let mut p = Path::new();
    p.add_outline(&Outline::Rectangle(rect));
    assert_eq!(path_text(&p), "M0,0 L0,10 L20,10 L20,0 Z");
    let mut r = Path::new();
    r.add_outline(&Outline::Rounded(rounded));
    let mut want = Path::new();
    want.add_round_rect(&rounded, PathDirection::CounterClockwise);
    assert_eq!(r, want);
    let mut g = Path::new();
    g.add_outline(&Outline::Generic(triangle.clone()));
    assert_eq!(g, triangle);
}

// ---- Cubic Béziers along an axis ----------------------------------------------------------------------------------

/// The cubic Bézier of `p` at `t`, in doubles.
fn cubic(p: [f32; 4], t: f64) -> f64 {
    let u = 1.0 - t;
    u * u * u * p[0] as f64 + 3.0 * u * u * t * p[1] as f64 + 3.0 * u * t * t * p[2] as f64 + t * t * t * p[3] as f64
}

/// Compose's Bezier.kt: a cubic's first root within 0..1 (NaN if none), its value with its ends at 0 and 1, and its
/// bounds along the axis (what CubicBezierEasing solves its curves with).
#[test]
fn cubic_roots_and_bounds_are_composes() {
    // Roots: where the curve crosses 0, the first within 0..1.
    for p in [[-0.5, -0.1, 0.3, 0.5], [-1.0, 2.0, -2.0, 1.0], [0.25, -0.75, 0.75, -0.25], [-0.2, -0.2, 0.8, 0.8]] {
        let t = find_first_cubic_root(p[0], p[1], p[2], p[3]);
        assert!((0.0..=1.0).contains(&t) && cubic(p, t as f64).abs() < 1e-4, "{p:?}: {t}");
    }
    // A quadratic, a line, nothing; outside 0..1: none.
    let quadratic = find_first_cubic_root(-1.0, 0.0, 1.0, 2.0);
    assert!(cubic([-1.0, 0.0, 1.0, 2.0], quadratic as f64).abs() < 1e-5);
    assert!(near_by(find_first_cubic_root(-0.5, -0.5 + 1.0 / 3.0, -0.5 + 2.0 / 3.0, 0.5), 0.5, 1e-5));
    assert!(find_first_cubic_root(1.0, 1.0, 1.0, 1.0).is_nan());
    assert!(find_first_cubic_root(1.0, 2.0, 3.0, 4.0).is_nan());
    // Its ends at 0 and 1: FastOutSlowIn's y halfway along t.
    assert!(near_by(evaluate_cubic(0.0, 1.0, 0.5), cubic([0.0, 0.0, 1.0, 1.0], 0.5) as f32, 1e-6));
    assert!(evaluate_cubic(0.3, 0.9, 0.0) == 0.0 && near_by(evaluate_cubic(0.3, 0.9, 1.0), 1.0, 1e-6));
    // Bounds: its ends, or past them where it overshoots.
    assert_eq!(compute_cubic_vertical_bounds(0.0, 0.0, 1.0, 1.0), (0.0, 1.0));
    let (min, max) = compute_cubic_vertical_bounds(0.0, -0.5, 1.5, 1.0);
    let brute = (0..=10_000).map(|i| cubic([0.0, -0.5, 1.5, 1.0], i as f64 / 10_000.0) as f32);
    let (want_min, want_max) = brute.fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(y), b.max(y)));
    assert!(near_by(min, want_min, 1e-4) && near_by(max, want_max, 1e-4) && min < 0.0 && max > 1.0);
}

/// Compose's Bezier.kt over a path's segment: its y at t, where along it x is a value, its bounds along x (what
/// PathEasing solves with).
#[test]
fn segment_roots_and_bounds_are_composes() {
    let segment =
        |r#type, points: &[(f32, f32)]| PathSegment::new(r#type, points.iter().map(|&(x, y)| o(x, y)).collect(), 0.0);
    let line = segment(PathSegmentType::Line, &[(0.0, 0.0), (2.0, 4.0)]);
    assert!(near_by(find_first_root(&line, 1.0), 0.5, 1e-6) && near_by(evaluate_y(&line, 0.5), 2.0, 1e-6));
    assert!(find_first_root(&line, 3.0).is_nan());
    let quadratic = segment(PathSegmentType::Quadratic, &[(0.0, 0.0), (1.0, 2.0), (2.0, 0.0)]);
    assert!(near_by(find_first_root(&quadratic, 1.0), 0.5, 1e-6) && near_by(evaluate_y(&quadratic, 0.5), 1.0, 1e-6));
    let cubic_segment = segment(PathSegmentType::Cubic, &[(0.0, 0.0), (0.4, 0.0), (0.2, 1.0), (1.0, 1.0)]);
    let t = find_first_root(&cubic_segment, 0.5);
    assert!(near_by(cubic([0.0, 0.4, 0.2, 1.0], t as f64) as f32, 0.5, 1e-5));
    assert!(near_by(evaluate_y(&cubic_segment, t), cubic([0.0, 0.0, 1.0, 1.0], t as f64) as f32, 1e-6));
    // Bounds along x: past its ends where it bulges.
    let mut roots = [0.0f32; 5];
    let bulge = segment(PathSegmentType::Cubic, &[(0.0, 0.0), (-1.0, 0.0), (2.0, 0.0), (1.0, 0.0)]);
    let (min, max) = compute_horizontal_bounds(&bulge, &mut roots);
    let brute = (0..=10_000).map(|i| cubic([0.0, -1.0, 2.0, 1.0], i as f64 / 10_000.0) as f32);
    let (want_min, want_max) = brute.fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
    assert!(near_by(min, want_min, 1e-4) && near_by(max, want_max, 1e-4) && min < 0.0 && max > 1.0);
    assert_eq!(compute_horizontal_bounds(&line, &mut roots), (0.0, 2.0));
    // A move, a close: no root.
    assert!(find_first_root(&segment(PathSegmentType::Move, &[(0.0, 0.0)]), 0.0).is_nan());
}

/// Compose's IntervalTree: intervals found by what they overlap, kept balanced, in order by their starts.
#[test]
fn interval_trees_find_overlaps() {
    let mut tree = IntervalTree::new();
    assert!(!tree.contains(0.5) && tree.find_first_overlap(0.0, 1.0).is_none());
    for i in 0..100 {
        let start = ((i * 37) % 100) as f32;
        tree.add_interval(start, start + 1.5, Some(i));
    }
    // In order by their starts.
    let starts: Vec<f32> = tree.iter().map(|i| i.start).collect();
    assert!(starts.windows(2).all(|w| w[0] <= w[1]) && starts.len() == 100);
    // What overlaps 10.2: the intervals from 9 and 10.
    let mut found: Vec<f32> = tree.find_overlaps(10.2, 10.2).iter().map(|i| i.start).collect();
    found.sort_by(f32::total_cmp);
    assert_eq!(found, [9.0, 10.0]);
    let first = tree.find_first_overlap(10.2, 10.2).unwrap();
    assert!(first.contains(10.2) && first.data.is_some());
    assert!(tree.contains(100.5) && !tree.contains(101.0) && tree.contains_range(-5.0, 0.0));
    assert_eq!(tree.find_overlaps(20.0, 30.0).len(), 12);
    tree.clear();
    assert!(tree.iter().next().is_none() && !tree.contains(10.0));
    let i = Interval::new(0.0, 1.0, Some("a"));
    assert!(i.overlaps(&Interval::new(1.0, 2.0, None)) && !i.overlaps_range(1.5, 2.0));
}
