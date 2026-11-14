//! Vector path data: PathNode, PathParser (Compose's grammar, its quirks), PathBuilder, toPath and its arcs, and
//! morphing path data (the C++ Kompoz's tests about them; its parser's leniencies are Compose's quirks here).

use kompoz_graphics::*;

// ---- Helpers ------------------------------------------------------------------------------------------------------

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn mv(x: f32, y: f32) -> PathNode {
    PathNode::MoveTo { x, y }
}
fn rmv(dx: f32, dy: f32) -> PathNode {
    PathNode::RelativeMoveTo { dx, dy }
}
fn ln(x: f32, y: f32) -> PathNode {
    PathNode::LineTo { x, y }
}
fn rln(dx: f32, dy: f32) -> PathNode {
    PathNode::RelativeLineTo { dx, dy }
}
fn arc(a: f32, b: f32, theta: f32, large: bool, positive: bool, x: f32, y: f32) -> PathNode {
    PathNode::ArcTo {
        horizontal_ellipse_radius: a,
        vertical_ellipse_radius: b,
        theta,
        is_more_than_half: large,
        is_positive_arc: positive,
        arc_start_x: x,
        arc_start_y: y,
    }
}
fn rarc(a: f32, b: f32, theta: f32, large: bool, positive: bool, dx: f32, dy: f32) -> PathNode {
    PathNode::RelativeArcTo {
        horizontal_ellipse_radius: a,
        vertical_ellipse_radius: b,
        theta,
        is_more_than_half: large,
        is_positive_arc: positive,
        arc_start_dx: dx,
        arc_start_dy: dy,
    }
}

/// A command's end (a close's: none).
fn end(c: &PathCommand) -> Offset {
    c.end().unwrap_or(Offset::Unspecified)
}
/// A cubic's or quadratic's first control point.
fn c1(c: &PathCommand) -> Offset {
    match *c {
        PathCommand::Quadratic(c1, _) | PathCommand::Cubic(c1, _, _) => c1,
        _ => Offset::Unspecified,
    }
}
fn c2(c: &PathCommand) -> Offset {
    match *c {
        PathCommand::Cubic(_, c2, _) => c2,
        _ => Offset::Unspecified,
    }
}
fn is_cubic(c: &PathCommand) -> bool {
    matches!(c, PathCommand::Cubic(..))
}

fn commands(data: &str) -> Vec<PathCommand> {
    to_path(&add_path_nodes(data)).commands().to_vec()
}

// ---- Path data ----------------------------------------------------------------------------------------------------

#[test]
fn path_data_is_read_as_composes_path_parser_reads_it() {
    let add = add_path_nodes("M19,13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z");
    assert_eq!(add.len(), 14);
    assert_eq!(add[0], mv(19.0, 13.0));
    assert_eq!(add[1], PathNode::RelativeHorizontalTo { dx: -6.0 });
    assert_eq!(add[2], PathNode::RelativeVerticalTo { dy: 6.0 });
    assert_eq!(add[5], PathNode::HorizontalTo { x: 5.0 });
    assert_eq!(add[8], PathNode::VerticalTo { y: 5.0 });
    assert_eq!(add[13], PathNode::Close);

    // "1.41-1.41" is two numbers, a command's sets of numbers repeat it, a move's later pairs are lines.
    let p = add_path_nodes("M0,0L1.41-1.41 2,2c1,0 2,1 3,1s2,1 3,1zm1,1h2");
    assert_eq!(
        p,
        vec![
            mv(0.0, 0.0),
            ln(1.41, -1.41),
            ln(2.0, 2.0),
            PathNode::RelativeCurveTo { dx1: 1.0, dy1: 0.0, dx2: 2.0, dy2: 1.0, dx3: 3.0, dy3: 1.0 },
            PathNode::RelativeReflectiveCurveTo { dx1: 2.0, dy1: 1.0, dx2: 3.0, dy2: 1.0 },
            PathNode::Close,
            rmv(1.0, 1.0),
            PathNode::RelativeHorizontalTo { dx: 2.0 },
        ]
    );
    assert_eq!(add_path_nodes("M1 2 3 4m1 1 2 2"), vec![mv(1.0, 2.0), ln(3.0, 4.0), rmv(1.0, 1.0), rln(2.0, 2.0)]);

    // Numbers run together (".5.5" is two; exponents; a minus starts the next), what is left of a set dropped,
    // what can't be read skipped to the next command letter. A `+` is no sign (Compose's: the C++ read it): "l+1+2"
    // has no numbers.
    assert_eq!(add_path_nodes("  M.5.5L1e1-2.5E-1 3 # l+1+2 Z "), vec![mv(0.5, 0.5), ln(10.0, -0.25), PathNode::Close]);
    assert_eq!(
        add_path_nodes("Q1 2 3 4q1 2 3 4T5 6t5 6S1 2 3 4"),
        vec![
            PathNode::QuadTo { x1: 1.0, y1: 2.0, x2: 3.0, y2: 4.0 },
            PathNode::RelativeQuadTo { dx1: 1.0, dy1: 2.0, dx2: 3.0, dy2: 4.0 },
            PathNode::ReflectiveQuadTo { x: 5.0, y: 6.0 },
            PathNode::RelativeReflectiveQuadTo { dx: 5.0, dy: 6.0 },
            PathNode::ReflectiveCurveTo { x1: 1.0, y1: 2.0, x2: 3.0, y2: 4.0 },
        ]
    );
    assert_eq!(
        add_path_nodes("C1 2 3 4 5 6v1V2h3H4"),
        vec![
            PathNode::CurveTo { x1: 1.0, y1: 2.0, x2: 3.0, y2: 4.0, x3: 5.0, y3: 6.0 },
            PathNode::RelativeVerticalTo { dy: 1.0 },
            PathNode::VerticalTo { y: 2.0 },
            PathNode::RelativeHorizontalTo { dx: 3.0 },
            PathNode::HorizontalTo { x: 4.0 },
        ]
    );
    // An arc's flags: a character each in a command's first set ("01.5.5": 0, 1, .5, .5); a later set's read as
    // numbers (Compose's: the C++ read every set's as characters).
    assert_eq!(
        add_path_nodes("a1 1 0 01.5.5 2 2 30 1 0 2 2A3 4 5 1 1 6 7"),
        vec![
            rarc(1.0, 1.0, 0.0, false, true, 0.5, 0.5),
            rarc(2.0, 2.0, 30.0, true, false, 2.0, 2.0),
            arc(3.0, 4.0, 5.0, true, true, 6.0, 7.0),
        ]
    );
    // ("10" a later set's flag, then 2 the other: one number short of a second arc.)
    assert_eq!(add_path_nodes("a1 1 0 01.5.5 2 2 30 10 2 2"), vec![rarc(1.0, 1.0, 0.0, false, true, 0.5, 0.5)]);
    assert!(add_path_nodes("").is_empty() && add_path_nodes("   ").is_empty() && add_path_nodes("12 3").is_empty());

    // As a path: relative to the current point, s reflecting the last control point, m after z from the contour's
    // start.
    let c = to_path(&p).commands().to_vec();
    assert_eq!(c.len(), 8);
    assert!(matches!(c[2], PathCommand::Line(_)) && end(&c[2]) == Offset::new(2.0, 2.0));
    assert!(is_cubic(&c[3]) && c1(&c[3]) == Offset::new(3.0, 2.0) && c2(&c[3]) == Offset::new(4.0, 3.0));
    assert_eq!(end(&c[3]), Offset::new(5.0, 3.0));
    assert!(is_cubic(&c[4]) && c1(&c[4]) == Offset::new(6.0, 3.0) && c2(&c[4]) == Offset::new(7.0, 4.0));
    assert_eq!(end(&c[4]), Offset::new(8.0, 4.0));
    assert!(c[5] == PathCommand::Close && c[6] == PathCommand::Move(Offset::new(1.0, 1.0)));
    assert_eq!(end(&c[7]), Offset::new(3.0, 1.0));

    // A reflective curve after anything else starts at the current point; quadratics reflect their control point
    // (a reflected one too), H and V keep the other coordinate.
    let c = commands("M0,0L2,2S3,3 4,4M0,0Q1,2 2,0T4,0t2,0M1,1l0,2s1,0 2,0H5V2h1v1Z");
    assert_eq!(c.len(), 15);
    assert!(is_cubic(&c[2]) && c1(&c[2]) == Offset::new(2.0, 2.0) && c2(&c[2]) == Offset::new(3.0, 3.0));
    assert!(matches!(c[5], PathCommand::Quadratic(..)) && c1(&c[5]) == Offset::new(3.0, -2.0));
    assert_eq!(end(&c[5]).x, 4.0);
    assert!(c1(&c[6]) == Offset::new(5.0, 2.0) && end(&c[6]) == Offset::new(6.0, 0.0));
    assert!(is_cubic(&c[9]) && c1(&c[9]) == Offset::new(1.0, 3.0) && c2(&c[9]) == Offset::new(2.0, 3.0));
    assert_eq!(end(&c[9]), Offset::new(3.0, 3.0));
    assert!(end(&c[10]) == Offset::new(5.0, 3.0) && end(&c[11]) == Offset::new(5.0, 2.0));
    assert!(end(&c[12]) == Offset::new(6.0, 2.0) && end(&c[13]) == Offset::new(6.0, 3.0));

    // A relative reflective quadratic's control point what the next one reflects; after z, H and V from the
    // contour's start.
    let c = commands("M0,0Q1,2 2,0t2,0T6,0M2,2h4v4zV10");
    assert!(c.len() >= 9 && matches!(c[3], PathCommand::Quadratic(..)) && c1(&c[3]) == Offset::new(5.0, 2.0));
    assert_eq!(end(c.last().unwrap()), Offset::new(2.0, 10.0));

    // Into a path of one's own: emptied, its fill type kept.
    let mut into = Path::new();
    into.fill_type = PathFillType::EvenOdd;
    into.add_rect(Rect::new(0.0, 0.0, 5.0, 5.0), PathDirection::CounterClockwise);
    to_path_into(&add_path_nodes("M1,1h1"), &mut into);
    assert!(into.fill_type == PathFillType::EvenOdd && into.commands().len() == 2);

    // Compose's PathParser keeps its nodes; PathBuilder makes them by calls.
    let mut parser = PathParser::new();
    assert_eq!(parser.parse_path_string("M0,0h4").to_nodes().len(), 2);
    assert_eq!(parser.parse_path_string("M1,1").to_nodes().len(), 1);
    parser.add_path_nodes(&[rln(2.0, 0.0)]);
    assert_eq!(parser.to_nodes().len(), 2);
    assert_eq!(end(parser.to_path().commands().last().unwrap()).x, 3.0);
    let mut appended = vec![PathNode::Close];
    parser.path_string_to_nodes("M5,5", &mut appended);
    assert!(appended.len() == 2 && appended[1] == mv(5.0, 5.0) && parser.to_nodes().len() == 2);
    parser.clear();
    assert!(parser.to_nodes().is_empty());

    let built = PathData(|b| {
        b.move_to(1.0, 2.0)
            .move_to_relative(1.0, 1.0)
            .line_to(3.0, 4.0)
            .line_to_relative(1.0, 0.0)
            .horizontal_line_to(5.0)
            .horizontal_line_to_relative(1.0)
            .vertical_line_to(6.0)
            .vertical_line_to_relative(1.0)
            .curve_to(1.0, 2.0, 3.0, 4.0, 5.0, 6.0)
            .curve_to_relative(1.0, 2.0, 3.0, 4.0, 5.0, 6.0)
            .reflective_curve_to(1.0, 2.0, 3.0, 4.0)
            .reflective_curve_to_relative(1.0, 2.0, 3.0, 4.0)
            .quad_to(1.0, 2.0, 3.0, 4.0)
            .quad_to_relative(1.0, 2.0, 3.0, 4.0)
            .reflective_quad_to(1.0, 2.0)
            .reflective_quad_to_relative(3.0, 4.0)
            .arc_to(1.0, 2.0, 3.0, true, false, 4.0, 5.0)
            .arc_to_relative(1.0, 2.0, 3.0, false, true, 4.0, 5.0)
            .close();
    });
    assert_eq!(
        built,
        add_path_nodes(
            "M1,2m1,1L3,4l1,0H5h1V6v1C1,2 3,4 5,6c1,2 3,4 5,6S1,2 3,4s1,2 3,4Q1,2 3,4q1,2 3,4T1,2t3,4A1,2 3 1 0 4,5a1,2 3 0 1 4,5Z"
        )
    );

    let zero6 = PathNode::CurveTo { x1: 0.0, y1: 0.0, x2: 0.0, y2: 0.0, x3: 0.0, y3: 0.0 };
    let rzero6 = PathNode::RelativeCurveTo { dx1: 0.0, dy1: 0.0, dx2: 0.0, dy2: 0.0, dx3: 0.0, dy3: 0.0 };
    let quad = PathNode::QuadTo { x1: 0.0, y1: 0.0, x2: 0.0, y2: 0.0 };
    assert!(zero6.is_curve() && rzero6.is_curve());
    assert!(PathNode::ReflectiveCurveTo { x1: 0.0, y1: 0.0, x2: 0.0, y2: 0.0 }.is_curve());
    assert!(PathNode::RelativeReflectiveCurveTo { dx1: 0.0, dy1: 0.0, dx2: 0.0, dy2: 0.0 }.is_curve());
    assert!(!quad.is_curve() && !ln(0.0, 0.0).is_curve());
    assert!(quad.is_quad() && PathNode::RelativeQuadTo { dx1: 0.0, dy1: 0.0, dx2: 0.0, dy2: 0.0 }.is_quad());
    assert!(PathNode::ReflectiveQuadTo { x: 0.0, y: 0.0 }.is_quad());
    assert!(PathNode::RelativeReflectiveQuadTo { dx: 0.0, dy: 0.0 }.is_quad());
    assert!(!zero6.is_quad() && !PathNode::Close.is_quad());
    assert!(EmptyPath.is_empty());
}

#[test]
fn path_data_numbers_are_composes_next_float() {
    let line = |data: &str| match add_path_nodes(data).get(1) {
        Some(PathNode::LineTo { x, y }) => (*x, *y),
        other => panic!("{data}: {other:?}"),
    };
    // A point before the digits, a point after them; exponents either case and either sign.
    assert_eq!(line("M0 0L.25 5."), (0.25, 5.0));
    assert_eq!(line("M0 0L1E2 2.5e-1"), (100.0, 0.25));
    assert_eq!(line("M0 0L1e+2-3"), (100.0, -3.0));
    // A lone `e` is an exponent of 0 (and no command).
    assert_eq!(line("M0 0L2e 3"), (2.0, 3.0));
    // A second point starts the next number; a minus too, after a number or an exponent.
    assert_eq!(line("M0 0L1.5.5"), (1.5, 0.5));
    assert_eq!(line("M0 0L-1-.5"), (-1.0, -0.5));
    // A lone minus or point is no number: the set is one short, and nothing comes of it.
    assert_eq!(add_path_nodes("M0 0L1 -"), vec![mv(0.0, 0.0)]);
    assert_eq!(add_path_nodes("M0 0L1 ."), vec![mv(0.0, 0.0)]);
    // Commas and blanks between numbers, as many as there are; a trailing comma too.
    assert_eq!(line("M0 0L 1 ,, 2 ,"), (1.0, 2.0));
    // As the nearest float: the fast path (a significand of 24 bits, a power of ten up to 10), and beyond it.
    assert_eq!(line("M0 0L0.1 1.2345678"), (0.1, 1.2345678));
    assert_eq!(line("M0 0L123456789.123 1e-20"), (123456789.123, 1e-20));
    assert_eq!(line("M0 0L3.4028235e38 1e39"), (3.4028235e38, f32::INFINITY));
    assert_eq!(line("M0 0L-0 1e-50"), (-0.0, 0.0));
    assert!(line("M0 0L-0 0").0.is_sign_negative());
    // More than 19 digits: the first 19, the rest moving the exponent.
    assert_eq!(line("M0 0L12345678901234567890123 0.000000000000000000000012345"), (1.2345679e22, 1.2345e-23));
    // A letter of no command ends the reading (where Compose throws).
    assert_eq!(add_path_nodes("M1 2L3 4X5 6L7 8"), vec![mv(1.0, 2.0), ln(3.0, 4.0)]);
    // Not text at all: what reads, read.
    assert_eq!(add_path_nodes("M1 2é L3 4"), vec![mv(1.0, 2.0), ln(3.0, 4.0)]);
}

#[test]
fn path_data_arcs_become_cubics() {
    // Each cubic's middle and end `r` from `center` (the arc on the circle all along).
    let on_circle = |c: &[PathCommand], center: Offset, r: f32| {
        c.len() > 1
            && (1..c.len()).all(|i| {
                if !is_cubic(&c[i]) {
                    return false;
                }
                let (a, p1, p2, p3) = (end(&c[i - 1]), c1(&c[i]), c2(&c[i]), end(&c[i]));
                let middle = Offset::new(
                    (a.x + 3.0 * p1.x + 3.0 * p2.x + p3.x) / 8.0,
                    (a.y + 3.0 * p1.y + 3.0 * p2.y + p3.y) / 8.0,
                );
                [middle, p3].iter().all(|p| near((*p - center).get_distance(), r, 0.005))
            })
    };
    // A half circle over the top (y down): four eighths of a turn (as Compose's arcToBezier makes them), the second
    // ending at its top; the handles sin(θ)(√(4 + 3 tan²(θ/2)) - 1)/3 of the radius long, along the circle.
    let half = commands("M0,12A12,12 0 0,1 24,12");
    assert!(half.len() == 5 && on_circle(&half, Offset::new(12.0, 12.0), 12.0));
    assert!(near(end(&half[2]).x, 12.0, 0.001) && near(end(&half[2]).y, 0.0, 0.001));
    assert!(near(end(&half[4]).x, 24.0, 0.001) && near(end(&half[4]).y, 12.0, 0.001));
    assert!(near(c1(&half[1]).x, 0.0, 0.001) && near(c1(&half[1]).y, 8.81862, 0.001));
    // The other way round: under the bottom.
    let under = commands("M0,12A12,12 0 0,0 24,12");
    assert!(under.len() == 5 && on_circle(&under, Offset::new(12.0, 12.0), 12.0));
    assert!(near(end(&under[2]).x, 12.0, 0.001) && near(end(&under[2]).y, 24.0, 0.001));
    // Relative, its flags run together: the same half circle.
    let compact = commands("M0,12a12,12 0 0124,0");
    assert!(compact.len() == 5 && near(end(&compact[2]).x, 12.0, 0.001) && near(end(&compact[2]).y, 0.0, 0.001));
    assert!(near(end(&compact[4]).x, 24.0, 0.001));
    // Radii too small to reach: (a hair over) the smallest that does, 5 here.
    let small = commands("M0,0A1,1 0 0,1 10,0");
    assert!(small.len() == 5 && near(end(&small[2]).x, 5.0, 0.05) && near(end(&small[2]).y, -5.0, 0.05));
    assert!(near(end(&small[4]).x, 10.0, 0.001));
    // The large arc: three quarter turns around (12, 12), six eighths.
    let large = commands("M12,0A12,12 0 1,0 24,12");
    assert!(large.len() == 7 && on_circle(&large, Offset::new(12.0, 12.0), 12.0));
    assert!(near(end(&large[2]).x, 0.0, 0.001) && near(end(&large[2]).y, 12.0, 0.001));
    assert!(near(end(&large[6]).x, 24.0, 0.001) && near(end(&large[6]).y, 12.0, 0.001));
    // The other way: around (24, 0).
    let large_other = commands("M12,0A12,12 0 1,1 24,12");
    assert!(large_other.len() == 7 && on_circle(&large_other, Offset::new(24.0, 0.0), 12.0));
    assert!(near(end(&large_other[2]).x, 24.0, 0.001) && near(end(&large_other[2]).y, -12.0, 0.001));
    assert!(near(end(&large_other[4]).x, 36.0, 0.001) && near(end(&large_other[4]).y, 0.0, 0.001));
    // An ellipse turned a quarter: its radii swap places.
    let turned = commands("M0,10A10,5 90 0,1 0,30");
    assert!(turned.len() == 5 && near(end(&turned[2]).x, 5.0, 0.001) && near(end(&turned[2]).y, 20.0, 0.001));
    // No radius: a line (as SVG has it). After it the path goes on from its end. The same point: nothing.
    let flat = commands("M0,0A0,5 0 0,1 10,0h5");
    assert!(flat.len() == 3 && matches!(flat[1], PathCommand::Line(_)) && near(end(&flat[2]).x, 15.0, 0.5));
    assert_eq!(commands("M3,3A5,5 0 0,1 3,3").len(), 1);
    // A circle from two arcs: eight eighths.
    let circle = commands("M2,12a10,10 0 1,0 20,0a10,10 0 1,0 -20,0");
    assert!(circle.len() == 9 && on_circle(&circle, Offset::new(12.0, 12.0), 10.0));
}

#[test]
fn path_data_morphs_number_by_number() {
    let from = add_path_nodes("M0,0L10,0A5,5 0 0,1 20,0");
    let to = add_path_nodes("M10,10L20,20A15,25 90 1,0 40,0");
    assert!(can_morph(&from, &to) && !can_morph(&from, &to[..2]));
    let half = lerp_path_nodes(&from, &to, 0.5);
    assert_eq!(half[0], mv(5.0, 5.0));
    assert_eq!(half[1], ln(15.0, 10.0));
    // An arc's flags set while they are anywhere but 0 (as Android interpolates them).
    assert_eq!(half[2], arc(10.0, 15.0, 45.0, true, true, 30.0, 0.0));
    assert_eq!(lerp_path_nodes(&from, &to, 0.0), from);
    assert_eq!(lerp_path_nodes(&from, &to, 1.0)[2], arc(15.0, 25.0, 90.0, true, false, 40.0, 0.0));
    // Lists of other shapes go from one to the other halfway.
    let other = add_path_nodes("M0,0h10");
    assert_eq!(lerp_path_nodes(&from, &other, 0.49), from);
    assert_eq!(lerp_path_nodes(&from, &other, 0.5), other);
}
