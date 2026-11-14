//! Vector path data: PathNode, PathParser (Compose's grammar, its quirks), PathBuilder, toPath and its arcs, and
//! morphing path data (the C++ Kompoz's tests about them; its parser's leniencies are Compose's quirks here); vector
//! images: ImageVector and its builder, vector drawables read from XML, animated ones played (the C++ Kompoz's
//! tests' values; what painters draw of them is stage 5's).

use kompoz_graphics::*;
use kompoz_runtime::kompozable_block;

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

// ---- Vector images ------------------------------------------------------------------------------------------------

fn o(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

/// Each channel within a hundredth.
fn same(a: Color, b: Color) -> bool {
    near(a.red(), b.red(), 0.01)
        && near(a.green(), b.green(), 0.01)
        && near(a.blue(), b.blue(), 0.01)
        && near(a.alpha(), b.alpha(), 0.01)
}
fn same_rgb(a: Color, b: Color) -> bool {
    near(a.red(), b.red(), 0.01) && near(a.green(), b.green(), 0.01) && near(a.blue(), b.blue(), 0.01)
}

fn with_alpha(c: Color, alpha: f32) -> Color {
    Color::new(c.red(), c.green(), c.blue(), alpha)
}

fn solid(c: Color) -> Option<Brush> {
    Some(SolidColor(c))
}

/// A builder of Compose's defaults (as the C++ Kompoz's `ImageVector::Builder(w, h, vw, vh)`).
fn builder(width: f32, height: f32, viewport_width: f32, viewport_height: f32) -> ImageVectorBuilder {
    ImageVectorBuilder::new(Dp(width), Dp(height), viewport_width, viewport_height)
}

fn path_of(data: &str) -> VectorPath {
    VectorPath { path_data: add_path_nodes(data), ..VectorPath::default() }
}

/// A vector's path (in its groups too), with where its groups put it (in the viewport, by Compose's group matrices)
/// and the clips it is drawn inside (in viewport units, outermost first).
struct Placed {
    path: VectorPath,
    m: Matrix,
    clips: Vec<Path>,
}

impl Placed {
    /// Its path's points (the ends of its segments), placed.
    fn points(&self) -> Vec<Offset> {
        to_path(&self.path.path_data).commands().iter().filter_map(|c| c.end()).map(|p| self.m.map(p)).collect()
    }
    fn bbox(&self) -> Rect {
        let (mut x0, mut y0, mut x1, mut y1) = (1e9f32, 1e9f32, -1e9f32, -1e9f32);
        for p in self.points() {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
        Rect::new(x0, y0, x1, y1)
    }
}

fn placed_in(g: &VectorGroup, outer: &Matrix, clips: &[Path], out: &mut Vec<Placed>) {
    let mut m = g.group_matrix();
    m.times_assign(outer); // (its own, then its parents')
    let mut inside = clips.to_vec();
    if !g.clip_path_data.is_empty() {
        let mut clip = to_path(&g.clip_path_data);
        clip.transform(&m);
        inside.push(clip);
    }
    for n in g {
        match n {
            VectorNode::Path(p) => out.push(Placed { path: p.clone(), m, clips: inside.clone() }),
            VectorNode::Group(child) => placed_in(child, &m, &inside, out),
        }
    }
}

fn placed(v: &ImageVector) -> Vec<Placed> {
    let mut out = Vec::new();
    placed_in(v.root(), &Matrix::new(), &[], &mut out);
    out
}

const TEST_DRAWABLE: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<!-- a test drawable -->
<vector xmlns:android="http://schemas.android.com/apk/res/android" xmlns:aapt="http://schemas.android.com/aapt"
    android:width="48dp" android:height="40dp" android:viewportWidth="24" android:viewportHeight="20" android:autoMirrored="true">
    <path android:fillColor="#FF0000" android:pathData="M0,0h12v12h-12z"/>
    <group android:translateX="12" android:translateY="12">
        <path android:strokeColor="#800000FF" android:strokeWidth="2" android:strokeLineCap="round" android:strokeLineJoin="bevel"
              android:strokeAlpha="0.5" android:pathData="M0,0h6"/>
        <group android:rotation="90" android:pivotX="6" android:pivotY="6" android:scaleX="2">
            <path android:fillColor="#0F0" android:fillType="evenOdd" android:pathData="M6,0L6,6L8,6"/>
        </group>
    </group>
    <clip-path android:pathData="M0,0h24v24h-24z"/>
    <path android:pathData="M0,16h4" android:fillAlpha="0.25">
        <aapt:attr name="android:fillColor">
            <gradient android:startColor="#8FF0" android:endColor="#000000" android:type="linear"/>
        </aapt:attr>
    </path>
</vector>"##;

#[test]
fn vector_drawable_xml_becomes_an_image_vector() {
    let v = ImageVector::from_vector_xml(TEST_DRAWABLE).unwrap();
    assert!(v.default_width() == Dp(48.0) && v.default_height() == Dp(40.0));
    assert!(v.viewport_width() == 24.0 && v.viewport_height() == 20.0 && v.auto_mirror());
    assert!(v.tint_color().is_unspecified() && v.tint_blend_mode() == BlendMode::SrcIn && v.name().is_empty());
    // Its tree as the file has it: a path, a group (a path, a group in it), and the path after the clip-path in a
    // group of its own that clips it.
    let root = v.root();
    assert!(root.size() == 3 && root[0].path().is_some() && root[1].group().is_some() && root[2].group().is_some());
    let moved = root[1].group().unwrap();
    assert!(moved.translation_x == 12.0 && moved.translation_y == 12.0 && moved.size() == 2);
    assert!(moved[0].path().is_some() && moved[1].group().is_some());
    let turn = moved[1].group().unwrap();
    assert!(turn.rotation == 90.0 && turn.pivot_x == 6.0 && turn.pivot_y == 6.0);
    assert!(turn.scale_x == 2.0 && turn.scale_y == 1.0);
    let clipping = root[2].group().unwrap();
    assert!(clipping.clip_path_data == add_path_nodes("M0,0h24v24h-24z") && clipping.size() == 1);
    let paths = placed(&v);
    assert_eq!(paths.len(), 4);
    // (a stroke's width 1 where it isn't given, as Compose reads it)
    let square = &paths[0].path;
    assert!(square.fill == solid(Color::hex(0xFF0000)) && square.stroke.is_none());
    assert!(square.path_data.len() == 5 && square.stroke_line_width == 1.0);
    // In its group; its stroke's color half transparent, then half again.
    let line = &paths[1].path;
    assert!(
        line.fill.is_none() && near(line.stroke.as_ref().unwrap().solid_color().unwrap().alpha(), 128.0 / 255.0, 0.01)
    );
    assert!(line.stroke_alpha == 0.5 && line.stroke_line_width == 2.0);
    assert!(line.stroke_line_cap == StrokeCap::Round && line.stroke_line_join == StrokeJoin::Bevel);
    let at = paths[1].points();
    assert!(
        near(at[0].x, 12.0, 1e-4)
            && near(at[0].y, 12.0, 1e-4)
            && near(at[1].x, 18.0, 1e-4)
            && near(at[1].y, 12.0, 1e-4)
    );
    // Turned a quarter clockwise about its pivot (and stretched along x first), inside the moved group.
    let turned = &paths[2].path;
    assert!(turned.fill == solid(Color::hex(0x00FF00)) && turned.path_fill_type == PathFillType::EvenOdd);
    let t = paths[2].points();
    assert!(near(t[0].x, 24.0, 1e-4) && near(t[0].y, 18.0, 1e-4) && near(t[1].x, 18.0, 1e-4));
    assert!(near(t[1].y, 18.0, 1e-4) && near(t[2].x, 18.0, 1e-4) && near(t[2].y, 22.0, 1e-4));
    // A gradient no longer than a point: its start color (#ARGB), with its own alpha; clipped.
    let faded = &paths[3].path;
    let gradient = faded.fill.as_ref().unwrap();
    assert!(matches!(gradient.kind(), BrushKind::LinearGradient(_)));
    assert!(gradient.color_at(Offset::Zero, Rect::Zero) == Color::from_argb(0x88FFFF00) && faded.fill_alpha == 0.25);
    let b = paths[3].bbox();
    assert!(
        paths[3].clips.len() == 1 && near(b.left, 0.0, 1e-4) && near(b.top, 16.0, 1e-4) && near(b.width(), 4.0, 1e-4)
    );
    assert!(ImageVector::from_vector_xml("<shape/>").is_none() && ImageVector::from_vector_xml("not xml").is_none());
    // Its tint and how (android:tint, android:tintMode), as Compose reads them.
    let tinted = |attributes: &str| {
        ImageVector::from_vector_xml(&format!(
            r##"<vector xmlns:android="http://schemas.android.com/apk/res/android" android:width="24dp"
            android:height="24dp" android:viewportWidth="24" android:viewportHeight="24" {attributes}><path
            android:fillColor="#000" android:pathData="M0,0h4v4z"/></vector>"##
        ))
        .unwrap()
    };
    let atop = tinted(r##"android:tint="#80FF0000" android:tintMode="src_atop""##);
    assert!(atop.tint_color() == Color::from_argb(0x80FF0000) && atop.tint_blend_mode() == BlendMode::SrcAtop);
    let blue = tinted(r##"android:tint="#00F""##);
    assert!(blue.tint_blend_mode() == BlendMode::SrcIn && blue.tint_color() == Color::hex(0x0000FF));
    let over = tinted(r#"android:tintMode="src_over""#);
    assert!(over.tint_blend_mode() == BlendMode::SrcOver && over.tint_color().is_unspecified());
    assert_eq!(tinted(r#"android:tintMode="multiply""#).tint_blend_mode(), BlendMode::Modulate);
    assert_eq!(tinted(r#"android:tintMode="screen""#).tint_blend_mode(), BlendMode::Screen);
    assert_eq!(tinted(r#"android:tintMode="add""#).tint_blend_mode(), BlendMode::Plus);
    assert_eq!(tinted(r#"android:tintMode="src_in""#).tint_blend_mode(), BlendMode::SrcIn);
}

#[test]
fn vector_drawable_xml_that_cant_be_read_is_skipped() {
    // Whatever can't be read: no vector, or what can be read of it; never a panic.
    for xml in [
        "",
        "<",
        "<vector",
        "<vector android:width=>",
        "<vector android:width=\"24\"",
        "<!-- <vector/>",
        "<?xml version=\"1.0\"",
        "<<<>>>",
        "<vector>ğüşiöç</vector",
        "<animated-vector><target android:name=\"x\"/></animated-vector>",
    ] {
        assert!(ImageVector::from_vector_xml(xml).is_none(), "{xml}");
        assert!(AnimatedImageVector::from_animated_vector_xml(xml).is_none(), "{xml}");
    }
    // Unclosed inside, numbers and colors that aren't, path data that isn't: read as far as they go.
    let v = ImageVector::from_vector_xml(
        r##"<vector android:width="abc" android:viewportWidth="12dp"><group android:rotation="x">
        <path android:fillColor="#GG0000" android:strokeColor="@color/ink" android:pathData="M0,0Lq!"/>
        <path android:fillColor="#12345" android:pathData="M1,1h2"/></group></vector>"##,
    )
    .unwrap();
    assert!(v.viewport_width() == 12.0 && v.viewport_height() == 24.0 && v.default_width() == Dp(12.0));
    let g = v.root()[0].group().unwrap();
    assert!(g.rotation == 0.0 && g.size() == 2);
    let (bad, odd) = (g[0].path().unwrap(), g[1].path().unwrap());
    assert!(bad.fill.is_none() && bad.stroke == solid(Color::Black) && bad.path_data == add_path_nodes("M0,0"));
    assert!(odd.fill.is_none() && odd.path_data.len() == 2);
    // An unclosed vector: nothing whole.
    assert!(ImageVector::from_vector_xml("<vector><path android:pathData=\"M0,0h1\"/>").is_none());
}

#[test]
fn vector_drawable_gradients_become_brushes() {
    let xml = r##"<vector xmlns:android="http://schemas.android.com/apk/res/android" xmlns:aapt="http://schemas.android.com/aapt"
    android:width="48dp" android:height="48dp" android:viewportWidth="24" android:viewportHeight="24">
    <path android:pathData="M0,0h24v24h-24z">
        <aapt:attr name="android:fillColor">
            <gradient android:type="linear" android:startX="0" android:startY="0" android:endX="24" android:endY="0"
                      android:startColor="#FF0000" android:centerColor="#00FF00" android:endColor="#0000FF"/>
        </aapt:attr>
    </path>
    <group android:translateX="4" android:translateY="2">
        <path android:pathData="M0,0h8v8h-8z" android:strokeWidth="1">
            <aapt:attr name="android:strokeColor">
                <gradient android:type="radial" android:centerX="4" android:centerY="4" android:gradientRadius="4" android:tileMode="mirror">
                    <item android:offset="0" android:color="#FFFFFF"/>
                    <item android:offset="0.25" android:color="#FF0000"/>
                    <item android:offset="1" android:color="#000000"/>
                </gradient>
            </aapt:attr>
        </path>
    </group>
    <group android:scaleX="2" android:scaleY="2">
        <path android:pathData="M0,0h4v4h-4z">
            <aapt:attr name="android:fillColor">
                <gradient android:type="sweep" android:centerX="2" android:centerY="2" android:startColor="#FFFF00" android:endColor="#00FFFF"/>
            </aapt:attr>
        </path>
        <path android:pathData="M0,0h4v4h-4z">
            <aapt:attr name="android:fillColor">
                <gradient android:type="radial" android:centerX="2" android:centerY="2" android:gradientRadius="1" android:tileMode="repeat"
                          android:startColor="#000000" android:endColor="#FFFFFF"/>
            </aapt:attr>
        </path>
    </group>
    <path android:pathData="M0,0h4v4h-4z">
        <aapt:attr name="android:fillColor">
            <gradient android:type="radial" android:startColor="#FF0000" android:endColor="#FFFFFF"/>
        </aapt:attr>
    </path>
</vector>"##;
    let v = ImageVector::from_vector_xml(xml).unwrap();
    let paths = placed(&v);
    assert_eq!(paths.len(), 5);
    let viewport = Rect::new(0.0, 0.0, 24.0, 24.0);
    // Linear: start, center and end colors from its start point to its end.
    let linear = paths[0].path.fill.as_ref().unwrap();
    assert!(matches!(linear.kind(), BrushKind::LinearGradient(_)));
    assert!(same(linear.color_at(o(0.0, 5.0), viewport), Color::hex(0xFF0000)));
    assert!(same(linear.color_at(o(12.0, 5.0), viewport), Color::hex(0x00FF00)));
    assert!(same(linear.color_at(o(24.0, 5.0), viewport), Color::hex(0x0000FF)));
    // Radial, stroking, of its items, back and forth past its radius; in its path's units (its group moves it when
    // it is drawn).
    let ring = &paths[1].path;
    let radial = ring.stroke.as_ref().unwrap();
    assert!(ring.fill.is_none() && matches!(radial.kind(), BrushKind::RadialGradient(_)));
    assert_eq!(radial.tile_mode(), TileMode::Mirror);
    assert!(same(radial.color_at(o(4.0, 4.0), viewport), Color::White));
    assert!(same(radial.color_at(o(5.0, 4.0), viewport), Color::hex(0xFF0000)));
    assert!(same(radial.color_at(o(4.0, 8.0), viewport), Color::Black));
    assert!(same(radial.color_at(o(11.0, 4.0), viewport), Color::hex(0xFF0000)));
    assert!(near(paths[1].bbox().left, 4.0, 1e-4) && near(paths[1].bbox().top, 2.0, 1e-4));
    // Sweep, clockwise from 3 o'clock round its center: left of it, halfway.
    let sweep = paths[2].path.fill.as_ref().unwrap();
    assert!(matches!(sweep.kind(), BrushKind::SweepGradient(_)));
    assert!(same(sweep.color_at(o(4.0, 2.0), viewport), Color::hex(0xFFFF00)));
    assert!(same(sweep.color_at(o(0.0, 2.0), viewport), Color::new(0.5, 1.0, 0.5, 1.0)));
    // Repeated past its radius.
    let rings = paths[3].path.fill.as_ref().unwrap();
    assert!(
        rings.tile_mode() == TileMode::Repeated
            && same(rings.color_at(o(2.5, 2.0), viewport), Color::new(0.5, 0.5, 0.5, 1.0))
    );
    assert!(near(paths[3].bbox().width(), 8.0, 1e-4)); // (its group scales it twice)
    // A radial one without a radius (Android refuses it): its start color.
    assert_eq!(paths[4].path.fill, solid(Color::hex(0xFF0000)));
    // Not one color: a painter keeps it in an Argb8888 bitmap.
    assert!(!v.is_tintable() && v.cache_bitmap_config(None) == ImageBitmapConfig::Argb8888);
    // The same with stops of one's own.
    let own_radial =
        Brush::radial_gradient_stops(&[(0.0, Color::Black), (0.8, Color::White)], o(10.0, 10.0), 10.0, TileMode::Clamp);
    assert!(same(own_radial.color_at(o(14.0, 10.0), viewport), Color::new(0.5, 0.5, 0.5, 1.0)));
    assert!(same(own_radial.color_at(o(19.0, 10.0), viewport), Color::White));
    let own_sweep = Brush::sweep_gradient_stops(&[(0.0, Color::Black), (0.5, Color::White)], o(10.0, 10.0));
    assert!(same(own_sweep.color_at(o(10.0, 20.0), viewport), Color::new(0.5, 0.5, 0.5, 1.0)));
    assert!(same(own_sweep.color_at(o(0.0, 10.0), viewport), Color::White));
}

#[test]
fn image_vector_groups_move_and_clip_their_paths() {
    let (red, blue) = (Color::hex(0xC03030), Color::hex(0x3050C0));
    let turning = VectorGroup {
        name: "dönen".into(),
        rotation: 90.0,
        pivot_x: 12.0,
        pivot_y: 12.0,
        clip_path_data: add_path_nodes("M0,0h12v24h-12z"),
        ..VectorGroup::default()
    };
    let v = builder(48.0, 48.0, 24.0, 24.0)
        .group_of(turning, |b| {
            b.add_path_of(VectorPath { stroke: solid(red), stroke_line_width: 2.0, ..path_of("M12,2L12,6") });
            b.add_group_of(VectorGroup { scale_x: 2.0, scale_y: 2.0, translation_x: 1.0, ..VectorGroup::default() })
                .add_path_of(VectorPath {
                    fill: Some(Brush::linear_gradient(
                        &[Color::Black, Color::White],
                        o(0.0, 0.0),
                        o(2.0, 0.0),
                        TileMode::Clamp,
                    )),
                    stroke: solid(red),
                    stroke_line_width: 1.0,
                    ..path_of("M0,0h2v2h-2z")
                })
                .clear_group();
        })
        .add_path_of(VectorPath { fill: solid(blue), ..path_of("M0,0h4v4h-4z") })
        .build();
    // A tree: the group (its path, its group), then the path.
    let root = v.root();
    assert!(root.size() == 2 && root[1].path().is_some());
    let group = root[0].group().unwrap();
    assert!(group.name == "dönen" && group.size() == 2);
    let paths = placed(&v);
    assert_eq!(paths.len(), 3);
    // Turned a quarter clockwise about the middle: its line from the top to the right; its clip (the left half) the
    // top half.
    let line = paths[0].points();
    assert!(near(line[0].x, 22.0, 1e-4) && near(line[0].y, 12.0, 1e-4));
    assert!(near(line[1].x, 18.0, 1e-4) && near(line[1].y, 12.0, 1e-4));
    assert!(paths[0].path.stroke_line_width == 2.0 && paths[0].clips.len() == 1);
    let clip = paths[0].clips[0].get_bounds();
    assert!(near(clip.left, 0.0, 1e-4) && near(clip.top, 0.0, 1e-4));
    assert!(near(clip.width(), 24.0, 1e-4) && near(clip.height(), 12.0, 1e-4));
    // Inside a group of its own too: scaled twice, moved, then turned; clipped by the outer group still; its gradient
    // and stroke in its own units (drawn through its groups' transforms).
    let at = paths[1].bbox();
    assert!(near(at.left, 20.0, 1e-4) && near(at.top, 1.0, 1e-4) && near(at.width(), 4.0, 1e-4));
    assert!(near(at.height(), 4.0, 1e-4) && paths[1].clips.len() == 1);
    let viewport = Rect::new(0.0, 0.0, 24.0, 24.0);
    let fill = paths[1].path.fill.as_ref().unwrap();
    assert!(same(fill.color_at(o(1.0, 0.0), viewport), Color::new(0.5, 0.5, 0.5, 1.0)));
    assert_eq!(paths[1].path.stroke_line_width, 1.0);
    // Out of the groups: as it is.
    let out = paths[2].bbox();
    assert!(near(out.left, 0.0, 1e-4) && near(out.width(), 4.0, 1e-4) && paths[2].clips.is_empty());
    // Each group's matrix as Compose's: the group's takes (12, 2) to (22, 12); the inner one's is scaled about 0,
    // moved 1.
    let top = group.group_matrix().map(o(12.0, 2.0));
    let inner = group[1].group().unwrap().group_matrix().map(o(2.0, 2.0));
    assert!(
        near(top.x, 22.0, 0.001) && near(top.y, 12.0, 0.001) && near(inner.x, 5.0, 0.001) && near(inner.y, 4.0, 0.001)
    );
    assert!(VectorGroup::default().group_matrix().is_identity());
    // From a vector drawable: a <clip-path> clips what comes after it in its group.
    let xml = ImageVector::from_vector_xml(
        r##"<vector xmlns:android="http://schemas.android.com/apk/res/android"
        android:width="24dp" android:height="24dp" android:viewportWidth="24" android:viewportHeight="24">
        <group android:name="büyük" android:scaleX="2" android:scaleY="2">
            <clip-path android:name="kırp" android:pathData="M0,0h6v6h-6z"/>
            <path android:strokeColor="#000" android:strokeWidth="1" android:pathData="M1,1h2"/>
            <group android:translateX="1">
                <clip-path android:pathData="M0,0h2v2h-2z"/>
                <path android:fillColor="#000" android:pathData="M0,0h1v1h-1z"/>
            </group>
            <path android:fillColor="#000" android:pathData="M0,0h1v1h-1z"/>
        </group>
        <path android:fillColor="#000" android:pathData="M0,0h1v1h-1z"/>
    </vector>"##,
    )
    .unwrap();
    let xp = placed(&xml);
    assert_eq!(xp.len(), 4);
    let big = xml.root()[0].group().unwrap();
    assert!(big.name == "büyük" && big.size() == 1 && big[0].group().is_some_and(|g| g.name == "kırp"));
    assert!(xp[0].clips.len() == 1 && near(xp[0].clips[0].get_bounds().width(), 12.0, 1e-4));
    assert_eq!(xp[0].path.stroke_line_width, 1.0);
    assert!(xp[1].clips.len() == 2 && near(xp[1].clips[1].get_bounds().left, 2.0, 1e-4));
    assert!(near(xp[1].clips[1].get_bounds().width(), 4.0, 1e-4));
    assert!(xp[2].clips.len() == 1 && xp[3].clips.is_empty() && near(xp[3].bbox().width(), 1.0, 1e-4));
}

#[test]
fn vector_paths_trim() {
    // Trimmed: only a piece of a line drawn (in its viewport's units).
    let ink = Color::hex(0x2040A0);
    let drawn = |start: f32, end: f32, offset: f32, data: &str| -> Option<Rect> {
        let v = builder(48.0, 48.0, 24.0, 24.0)
            .add_path_of(VectorPath {
                stroke: solid(ink),
                stroke_line_width: 1.0,
                trim_path_start: start,
                trim_path_end: end,
                trim_path_offset: offset,
                ..path_of(data)
            })
            .build();
        let path = v.root()[0].path().unwrap().render_path();
        (!path.is_empty()).then(|| path.get_bounds())
    };
    let line = |start: f32, end: f32, offset: f32| drawn(start, end, offset, "M0,12h24");
    let spans =
        |r: Option<Rect>, x0: f32, x1: f32| r.is_some_and(|r| near(r.left, x0, 0.01) && near(r.right, x1, 0.01));
    assert!(spans(line(0.0, 1.0, 0.0), 0.0, 24.0)); // untrimmed
    assert!(spans(line(0.0, 1.0, 0.5), 0.0, 24.0)); // (from its start to its end however far moved on)
    assert!(spans(line(0.0, 0.5, 0.0), 0.0, 12.0)); // its first half
    assert!(spans(line(0.25, 0.75, 0.0), 6.0, 18.0));
    assert!(spans(line(0.25, 1.0, 0.0), 6.0, 24.0)); // (an end of 1 comes round to 0: from the start to the end)
    assert!(spans(line(0.0, 0.5, 0.5), 12.0, 24.0)); // moved on: its second half
    assert!(spans(line(0.5, 1.0, 0.25), 0.0, 24.0)); // 0.75 to 1.25: round its end, both ends drawn
    assert!(line(0.0, 0.0, 0.0).is_none()); // nothing
    // As Android measures it, its first contour alone: three quarters of the first line (12 long).
    let two = drawn(0.0, 0.75, 0.0, "M0,0h12M0,12h24").unwrap();
    assert!(near(two.width(), 9.0, 0.01) && near(two.height(), 0.0, 0.01));
    // Untrimmed: its own path, its fill type; trimmed, filled non-zero (as Compose's render path is).
    let odd = VectorPath { path_fill_type: PathFillType::EvenOdd, ..path_of("M0,0h4v4z") };
    assert!(
        odd.render_path() == {
            let mut p = to_path(&odd.path_data);
            p.fill_type = PathFillType::EvenOdd;
            p
        }
    );
    assert_eq!(VectorPath { trim_path_end: 0.5, ..odd }.render_path().fill_type, PathFillType::NonZero);
    // From a vector drawable.
    let xml = ImageVector::from_vector_xml(
        r##"<vector xmlns:android="http://schemas.android.com/apk/res/android"
        android:width="24dp" android:height="24dp" android:viewportWidth="24" android:viewportHeight="24">
        <path android:name="kısa" android:strokeColor="#000" android:strokeWidth="1" android:pathData="M0,0h4" android:trimPathStart="0.1"
              android:trimPathEnd="0.6" android:trimPathOffset="0.2"/>
    </vector>"##,
    )
    .unwrap();
    let p = xml.root()[0].path().unwrap();
    assert!(p.name == "kısa" && near(p.trim_path_start, 0.1, 0.01) && near(p.trim_path_end, 0.6, 0.01));
    assert!(near(p.trim_path_offset, 0.2, 0.01));
    let r = p.render_path().get_bounds();
    assert!(near(r.left, 1.2, 0.01) && near(r.right, 3.2, 0.01));
}

const ANIMATED: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<animated-vector xmlns:android="http://schemas.android.com/apk/res/android" xmlns:aapt="http://schemas.android.com/aapt">
    <aapt:attr name="android:drawable">
        <vector android:name="root" android:width="24dp" android:height="24dp" android:viewportWidth="24" android:viewportHeight="24">
            <group android:name="spin" android:pivotX="12" android:pivotY="12">
                <path android:name="dot" android:strokeColor="#000000" android:strokeWidth="1" android:pathData="M12,2L12,6"/>
            </group>
            <path android:name="bar" android:fillColor="#FF0000" android:pathData="M0,0h4v4h-4z"/>
            <path android:name="line" android:strokeColor="#000000" android:strokeWidth="1" android:pathData="M0,20h24"/>
        </vector>
    </aapt:attr>
    <target android:name="spin">
        <aapt:attr name="android:animation">
            <set>
                <objectAnimator android:propertyName="rotation" android:valueFrom="90" android:valueTo="180" android:startOffset="100"
                    android:duration="200" android:interpolator="@android:interpolator/linear"/>
                <objectAnimator android:propertyName="rotation" android:valueFrom="0" android:valueTo="90" android:duration="100"
                    android:interpolator="@android:interpolator/linear"/>
            </set>
        </aapt:attr>
    </target>
    <target android:name="bar">
        <aapt:attr name="android:animation">
            <set android:ordering="sequentially">
                <objectAnimator android:propertyName="fillColor" android:valueFrom="#FF0000" android:valueTo="#0000FF" android:duration="100"
                    android:interpolator="@android:interpolator/linear"/>
                <objectAnimator android:propertyName="pathData" android:valueType="pathType" android:valueFrom="M0,0h4v4h-4z"
                    android:valueTo="M0,0h8v8h-8z" android:duration="100" android:interpolator="@android:anim/linear_interpolator"/>
            </set>
        </aapt:attr>
    </target>
    <target android:name="line">
        <aapt:attr name="android:animation">
            <objectAnimator android:duration="60" android:startOffset="100" android:repeatCount="1" android:repeatMode="reverse"
                android:interpolator="@android:interpolator/linear">
                <propertyValuesHolder android:propertyName="trimPathEnd" android:valueFrom="0.8" android:valueTo="0"/>
                <propertyValuesHolder android:propertyName="strokeWidth" android:valueTo="3"/>
            </objectAnimator>
        </aapt:attr>
    </target>
    <target android:name="root">
        <aapt:attr name="android:animation">
            <objectAnimator android:propertyName="alpha" android:valueFrom="1" android:valueTo="0.5" android:duration="200"
                android:interpolator="@android:interpolator/accelerate_quad"/>
        </aapt:attr>
    </target>
</animated-vector>"##;

#[test]
fn animated_vectors_play_their_animations() {
    let avd = AnimatedImageVector::from_animated_vector_xml(ANIMATED).unwrap();
    // The group's: its longer animator (together), 100 in and 200 long.
    assert_eq!(avd.total_duration_millis(), 300);
    let at = |t: f32| placed(&avd.vector_at(t));
    let fill = |p: &Placed| p.path.fill.as_ref().unwrap().solid_color().unwrap();
    // As it starts: the drawable as it is.
    assert!(avd.vector_at(0.0) == avd.image_vector() && at(0.0).len() == 3);
    assert!(same_rgb(fill(&at(0.0)[1]), Color::hex(0xFF0000)) && near(at(0.0)[1].bbox().width(), 4.0, 1e-4));
    // Its group turning (linear): 45° at 50; from 100 its second animator (started later, over the first): 112.5° at
    // 150.
    let (dot, later) = (at(50.0)[0].points()[0], at(150.0)[0].points()[0]);
    assert!(
        near(dot.x, 19.07, 0.05) && near(dot.y, 4.93, 0.05) && near(later.x, 21.24, 0.05) && near(later.y, 15.83, 0.05)
    );
    // One after the other: the color first (halfway at 50, in Oklab as Compose animates it: ColorTest's red to blue
    // at 0.5; done at 100), then the shape (halfway at 150).
    assert!(same_rgb(fill(&at(50.0)[1]), Color::hex(0x8C53A2)) && same_rgb(fill(&at(100.0)[1]), Color::hex(0x0000FF)));
    assert!(near(at(100.0)[1].bbox().width(), 4.0, 0.01) && near(at(150.0)[1].bbox().width(), 6.0, 0.01));
    assert!(near(at(220.0)[1].bbox().width(), 8.0, 0.01));
    // From 100, there and back (repeatMode reverse), holding where it ends; before it starts, its own value. A
    // property without valueFrom starts at its own value.
    let line = |t: f32| at(t)[2].path.clone();
    assert!(near(line(99.0).trim_path_end, 1.0, 0.001) && near(line(115.0).trim_path_end, 0.6, 0.001));
    assert!(near(line(130.0).trim_path_end, 0.4, 0.001) && near(line(130.0).stroke_line_width, 2.0, 0.001));
    assert!(near(line(175.0).trim_path_end, 0.2, 0.001) && near(line(230.0).trim_path_end, 0.8, 0.001));
    assert!(near(line(300.0).trim_path_end, 0.8, 0.001) && near(line(300.0).stroke_line_width, 1.0, 0.001));
    // The vector's alpha, accelerating (t²): a quarter of the way at the halfway mark.
    assert!(near(at(100.0)[1].path.fill_alpha, 0.875, 0.005) && near(at(200.0)[1].path.fill_alpha, 0.5, 0.005));
    assert!(AnimatedImageVector::from_animated_vector_xml("<vector/>").is_none());
    assert!(AnimatedImageVector::from_animated_vector_xml("not xml").is_none());
    // An arc's flags in between: set while anywhere but 0 (Android interpolates them as numbers).
    let arcs = AnimatedImageVector::from_animated_vector_xml(
        r##"<animated-vector xmlns:android="http://schemas.android.com/apk/res/android"
        xmlns:aapt="http://schemas.android.com/aapt"><aapt:attr name="android:drawable"><vector android:width="24dp" android:height="24dp"
        android:viewportWidth="24" android:viewportHeight="24"><path android:name="yay" android:strokeColor="#000" android:pathData="M0,0A5,5 0 0,1 10,0"/>
        </vector></aapt:attr><target android:name="yay"><aapt:attr name="android:animation"><objectAnimator android:propertyName="pathData"
        android:valueType="pathType" android:valueFrom="M0,0A5,5 0 0,1 10,0" android:valueTo="M0,0A5,8 0 1,0 10,0" android:duration="100"
        android:interpolator="@android:interpolator/linear"/></aapt:attr></target></animated-vector>"##,
    )
    .unwrap();
    let arc_at = |t: f32| arcs.vector_at(t).root()[0].path().unwrap().path_data[1];
    assert_eq!(arc_at(0.0), arc(5.0, 5.0, 0.0, false, true, 10.0, 0.0));
    assert_eq!(arc_at(50.0), arc(5.0, 6.5, 0.0, true, true, 10.0, 0.0));
    assert_eq!(arc_at(100.0), arc(5.0, 8.0, 0.0, true, false, 10.0, 0.0));
    // A vector drawable read the same way, whether in an animated one or not.
    assert_eq!(ImageVector::from_vector_xml(ANIMATED).unwrap(), avd.image_vector());
    // The same one; another read of it: another.
    assert!(avd.clone() == avd && AnimatedImageVector::from_animated_vector_xml(ANIMATED).unwrap() != avd);
}

/// One `<objectAnimator>` of `attributes` on a path's fillAlpha (0 to 1, 100 long unless they say).
fn fill_alpha_animated(attributes: &str) -> AnimatedImageVector {
    AnimatedImageVector::from_animated_vector_xml(&format!(
        r##"<animated-vector xmlns:android="http://schemas.android.com/apk/res/android"
        xmlns:aapt="http://schemas.android.com/aapt"><aapt:attr name="android:drawable"><vector android:width="24dp"
        android:height="24dp" android:viewportWidth="24" android:viewportHeight="24"><path android:name="p" android:fillColor="#000"
        android:fillAlpha="0.5" android:pathData="M0,0h1v1z"/></vector></aapt:attr><target android:name="p"><aapt:attr
        name="android:animation"><objectAnimator android:propertyName="fillAlpha" android:valueFrom="0" android:valueTo="1"
        {attributes}/></aapt:attr></target></animated-vector>"##
    ))
    .unwrap()
}

/// The fill alpha `avd` has `t` in.
fn fill_alpha_at(avd: &AnimatedImageVector, t: f32) -> f32 {
    avd.vector_at(t).root()[0].path().unwrap().fill_alpha
}

/// x of Compose's CubicBezierEasing's curve solved by halving (to check its roots against).
fn bezier_at(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let curve =
        |p1: f64, p2: f64, t: f64| 3.0 * (1.0 - t) * (1.0 - t) * t * p1 + 3.0 * (1.0 - t) * t * t * p2 + t * t * t;
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if curve(x1 as f64, x2 as f64, mid) < x as f64 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    curve(y1 as f64, y2 as f64, (lo + hi) / 2.0) as f32
}

#[test]
fn animated_vectors_play_androids_interpolators() {
    // Android's default (no interpolator): accelerate_decelerate, cos((t + 1)π) / 2 + 0.5; 300 long.
    let default = fill_alpha_animated("");
    assert_eq!(default.total_duration_millis(), 300);
    assert!(near(fill_alpha_at(&default, 75.0), 0.146_446_6, 1e-5) && near(fill_alpha_at(&default, 150.0), 0.5, 1e-5));
    let named = |name: &str, t: f32| {
        fill_alpha_at(&fill_alpha_animated(&format!(r#"android:duration="100" android:interpolator="{name}""#)), t)
    };
    assert!(near(named("@android:anim/accelerate_decelerate_interpolator", 25.0), 0.146_446_6, 1e-5));
    assert!(near(named("@android:anim/linear_interpolator", 30.0), 0.3, 1e-6));
    assert!(near(named("@android:anim/accelerate_interpolator", 30.0), 0.09, 1e-6));
    assert!(near(named("@android:interpolator/decelerate_quad", 30.0), 0.51, 1e-6));
    assert!(near(named("@android:interpolator/unknown", 25.0), 0.146_446_6, 1e-5));
    // Compose's cubic easings: the curve's y where its x is the fraction.
    for (name, (x1, y1, x2, y2)) in [
        ("@android:interpolator/fast_out_slow_in", (0.4, 0.0, 0.2, 1.0)),
        ("@android:interpolator/fast_out_linear_in", (0.4, 0.0, 1.0, 1.0)),
        ("@android:interpolator/linear_out_slow_in", (0.0, 0.0, 0.2, 1.0)),
    ] {
        for t in [10.0, 25.0, 50.0, 75.0, 90.0] {
            assert!(near(named(name, t), bezier_at(x1, y1, x2, y2, t / 100.0), 1e-4), "{name} at {t}");
        }
        assert!(named(name, 0.0) == 0.0 && named(name, 100.0) == 1.0);
    }
    // Before it starts, its own value; repeated (restarting) as many times again as it says; an infinite repeat
    // played once; a start offset.
    let repeated = fill_alpha_animated(
        r#"android:duration="100" android:startOffset="50" android:repeatCount="2" android:interpolator="@android:interpolator/linear""#,
    );
    assert_eq!(repeated.total_duration_millis(), 350);
    assert!(near(fill_alpha_at(&repeated, 0.0), 0.5, 1e-6) && near(fill_alpha_at(&repeated, 75.0), 0.25, 1e-6));
    assert!(near(fill_alpha_at(&repeated, 175.0), 0.25, 1e-6) && near(fill_alpha_at(&repeated, 400.0), 1.0, 1e-6));
    let infinite = fill_alpha_animated(r#"android:duration="100" android:repeatCount="infinite""#);
    assert_eq!(infinite.total_duration_millis(), 100);
    assert_eq!(fill_alpha_animated(r#"android:duration="100" android:repeatCount="-1""#).total_duration_millis(), 100);
    // No duration: at its end at once.
    let at_once = fill_alpha_animated(r#"android:duration="0""#);
    assert!(at_once.total_duration_millis() == 0 && near(fill_alpha_at(&at_once, 0.0), 1.0, 1e-6));
}

#[test]
fn image_vector_builder_makes_vectors_equal_by_what_they_are() {
    let (red, blue, green) = (Color::hex(0xC03030), Color::hex(0x3050C0), Color::hex(0x30A030));
    let square = |data: &str, fill: Color| VectorPath { fill: solid(fill), ..path_of(data) };
    let make_two = || {
        ImageVector::Builder("iki", Dp(48.0), Dp(48.0), 24.0, 24.0, Color::Unspecified, BlendMode::SrcIn, false)
            .add_path_of(square("M0,0h12v12h-12z", red))
            .add_path_of(VectorPath { stroke: solid(blue), stroke_line_width: 2.0, ..path_of("M12,12h12v12h-12z") })
            .build()
    };
    let two = make_two();
    let fills = builder(24.0, 24.0, 24.0, 24.0)
        .add_path_of(square("M0,0h12v12h-12z", red))
        .add_path_of(VectorPath { fill_alpha: 0.5, ..square("M0,12h12v12h-12z", red) })
        .build();
    let arrow = ImageVector::Builder("", Dp(24.0), Dp(24.0), 24.0, 24.0, Color::Unspecified, BlendMode::SrcIn, true)
        .add_path_of(square("M0,0h12v12h-12z", red))
        .build();
    assert!(
        two.name() == "iki" && two.root().size() == 2 && !two.auto_mirror() && arrow.auto_mirror() && arrow != fills
    );
    // Equal by what it is, not by its id: the same built twice.
    let again = make_two();
    assert!(again == two && again.id() != two.id());
    let empty = |name: &str, w: f32, h: f32, vw: f32, vh: f32, tint: Color, mode: BlendMode| {
        ImageVector::Builder(name, Dp(w), Dp(h), vw, vh, tint, mode, false).build()
    };
    let (none, src_in) = (Color::Unspecified, BlendMode::SrcIn);
    assert_ne!(empty("üç", 48.0, 48.0, 24.0, 24.0, none, src_in), empty("iki", 48.0, 48.0, 24.0, 24.0, none, src_in));
    assert_ne!(
        empty("", 48.0, 48.0, 24.0, 24.0, Color::Black, src_in),
        empty("", 48.0, 48.0, 24.0, 24.0, none, src_in)
    );
    assert_ne!(
        empty("", 48.0, 48.0, 24.0, 24.0, Color::Black, BlendMode::SrcOver),
        empty("", 48.0, 48.0, 24.0, 24.0, Color::Black, src_in)
    );
    assert_ne!(empty("", 48.0, 40.0, 24.0, 24.0, none, src_in), empty("", 48.0, 48.0, 24.0, 24.0, none, src_in));
    assert_ne!(empty("", 48.0, 48.0, 24.0, 20.0, none, src_in), empty("", 48.0, 48.0, 24.0, 24.0, none, src_in));
    // Its own colors: kept in an Argb8888 bitmap, drawn through no filter but a painter's.
    assert!(!two.is_tintable() && two.root_tint_color().is_unspecified() && !two.is_one_color());
    assert_eq!(two.cache_bitmap_config(None), ImageBitmapConfig::Argb8888);
    assert_eq!(two.draw_color_filter(None), None);
    let tint_blue = ColorFilter::tint(blue, BlendMode::SrcIn);
    assert_eq!(two.draw_color_filter(Some(&tint_blue)), Some(tint_blue));
    // One color however opaque its paths are (the mask holds how much): an Alpha8 bitmap, drawn in that color
    // opaque (or a painter's tint).
    assert!(fills.is_one_color() && fills.root_tint_color() == red);
    assert_eq!(fills.cache_bitmap_config(None), ImageBitmapConfig::Alpha8);
    assert_eq!(fills.draw_color_filter(None), Some(ColorFilter::tint(red, BlendMode::SrcIn)));
    let tint_green = ColorFilter::tint(green, BlendMode::SrcIn);
    assert_eq!(fills.cache_bitmap_config(Some(&tint_green)), ImageBitmapConfig::Alpha8);
    assert_eq!(fills.draw_color_filter(Some(&tint_green)), Some(tint_green));
    // (a filter a mask can't take)
    let modulated = ColorFilter::tint(green, BlendMode::Modulate);
    assert_eq!(fills.cache_bitmap_config(Some(&modulated)), ImageBitmapConfig::Argb8888);
    let faint = builder(24.0, 24.0, 24.0, 24.0).add_path_of(square("M0,0h12v12h-12z", with_alpha(red, 0.5))).build();
    assert_eq!(faint.cache_bitmap_config(None), ImageBitmapConfig::Alpha8);
    assert_eq!(faint.draw_color_filter(None), Some(ColorFilter::tint(red, BlendMode::SrcIn)));
    // A gradient in a group of its own makes it not one color; a color in one, the same color.
    let mixed = builder(24.0, 24.0, 24.0, 24.0)
        .add_path_of(square("M0,0h12v12h-12z", red))
        .group_of(VectorGroup::default(), |b| {
            b.add_path_of(VectorPath {
                fill: Some(Brush::horizontal_gradient(
                    &[Color::Black, Color::White],
                    0.0,
                    f32::INFINITY,
                    TileMode::Clamp,
                )),
                ..path_of("M12,12h4v4z")
            });
        })
        .build();
    assert!(!mixed.is_tintable() && mixed.cache_bitmap_config(None) == ImageBitmapConfig::Argb8888);
    let grouped = builder(24.0, 24.0, 24.0, 24.0)
        .add_path_of(square("M0,0h12v12h-12z", red))
        .group_of(VectorGroup::default(), |b| {
            b.add_path_of(VectorPath { stroke: solid(with_alpha(red, 0.25)), ..path_of("M12,12h4v4z") });
        })
        .build();
    assert!(grouped.is_one_color() && grouped.root_tint_color() == red);
    let other = builder(24.0, 24.0, 24.0, 24.0)
        .add_path_of(square("M0,0h12v12h-12z", red))
        .group_of(VectorGroup::default(), |b| {
            b.add_path_of(square("M12,12h4v4z", blue));
        })
        .build();
    assert!(!other.is_tintable() && !other.is_one_color());
    // Nothing painted: tintable, but no color.
    let bare = builder(24.0, 24.0, 24.0, 24.0).add_path_of(path_of("M0,0h1v1z")).build();
    assert!(
        bare.is_tintable() && !bare.is_one_color() && bare.cache_bitmap_config(None) == ImageBitmapConfig::Argb8888
    );
    // Through its own tint (a mask still when it is SrcIn or SrcOver), else its color.
    let tinted = ImageVector::Builder("", Dp(24.0), Dp(24.0), 24.0, 24.0, blue, BlendMode::SrcOver, false)
        .add_path_of(square("M0,0h12v12h-12z", red))
        .build();
    let multiplied = ImageVector::Builder("", Dp(24.0), Dp(24.0), 24.0, 24.0, blue, BlendMode::Modulate, false)
        .add_path_of(square("M0,0h12v12h-12z", red))
        .build();
    assert_eq!(tinted.intrinsic_color_filter(), Some(ColorFilter::tint(blue, BlendMode::SrcOver)));
    assert_eq!(tinted.cache_bitmap_config(None), ImageBitmapConfig::Alpha8);
    assert_eq!(tinted.draw_color_filter(None), Some(ColorFilter::tint(blue, BlendMode::SrcOver)));
    assert_eq!(tinted.draw_color_filter(Some(&tint_green)), Some(tint_green));
    assert_eq!(multiplied.cache_bitmap_config(None), ImageBitmapConfig::Argb8888);
    assert_eq!(multiplied.draw_color_filter(None), Some(ColorFilter::tint(blue, BlendMode::Modulate)));
    assert!(fills.intrinsic_color_filter().is_none());
    assert!(tintable_with_alpha_mask(None) && tintable_with_alpha_mask(Some(&tint_green)));
    assert!(!tintable_with_alpha_mask(Some(&modulated)));
    assert!(!tintable_with_alpha_mask(Some(&ColorFilter::lighting(Color::Black, Color::White))));
}

#[test]
fn image_vector_builder_takes_composes_named_calls() {
    let red = Color::hex(0xC03030);
    // Compose's calls, their parameters named, the rest Compose's defaults.
    let mut b = builder(24.0, 24.0, 24.0, 24.0);
    kompozable_block!({
        b.add_path(add_path_nodes("M0,0h4v4z"), fill = solid(red), name = "kare");
        b.add_group(name = "g", rotate = 45.0, pivot_x = 12.0, pivot_y = 12.0);
        b.path(
            stroke = solid(red),
            stroke_line_width = 2.0,
            path_builder = |p: &mut PathBuilder| {
                p.move_to(7.0, 12.0).line_to(17.0, 12.0);
            },
        );
        b.clear_group();
        b.group(
            scale_x = 2.0,
            clip_path_data = add_path_nodes("M0,0h4v4z"),
            block = |b: &mut ImageVectorBuilder| {
                b.add_path(add_path_nodes("M0,0h1v1z"), path_fill_type = PathFillType::EvenOdd, trim_path_end = 0.5);
            },
        );
    });
    let named = b.build();
    // The same through the whole ones.
    let whole = builder(24.0, 24.0, 24.0, 24.0)
        .add_path_of(VectorPath { name: "kare".into(), fill: solid(red), ..path_of("M0,0h4v4z") })
        .group_of(
            VectorGroup { name: "g".into(), rotation: 45.0, pivot_x: 12.0, pivot_y: 12.0, ..VectorGroup::default() },
            |b| {
                b.path_of(VectorPath { stroke: solid(red), stroke_line_width: 2.0, ..VectorPath::default() }, |p| {
                    p.move_to(7.0, 12.0).line_to(17.0, 12.0);
                });
            },
        )
        .group_of(
            VectorGroup { scale_x: 2.0, clip_path_data: add_path_nodes("M0,0h4v4z"), ..VectorGroup::default() },
            |b| {
                b.add_path_of(VectorPath {
                    path_fill_type: PathFillType::EvenOdd,
                    trim_path_end: 0.5,
                    ..path_of("M0,0h1v1z")
                });
            },
        )
        .build();
    assert_eq!(named, whole);
    // From plain code: what has no default.
    let mut plain = builder(24.0, 24.0, 24.0, 24.0);
    plain.add_group();
    plain.add_path(add_path_nodes("M0,0h1v1z"));
    plain.clear_group();
    plain.group(|b| {
        b.path(|p| {
            p.move_to(1.0, 1.0);
        });
    });
    let plain = plain.build();
    assert_eq!(plain.root().size(), 2);
    assert_eq!(plain.root()[0].group().unwrap()[0].path().unwrap(), &path_of("M0,0h1v1z"));
    assert_eq!(plain.root()[1].group().unwrap()[0].path().unwrap().path_data, add_path_nodes("M1,1"));
    // The groups left open closed by build; a clear with none open, nothing.
    let mut open = builder(24.0, 24.0, 24.0, 24.0);
    open.clear_group().add_group_of(VectorGroup::default()).add_group_of(VectorGroup::default());
    open.add_path_of(path_of("M0,0h1"));
    let open = open.build();
    assert!(open.root().size() == 1 && open.root()[0].group().unwrap()[0].group().unwrap()[0].path().is_some());
    // Single use, as Compose's.
    let mut used = builder(24.0, 24.0, 24.0, 24.0);
    used.build();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| used.build())).is_err());
    // Compose's defaults.
    let p = VectorPath::default();
    assert!(
        p.name == DefaultPathName && p.path_fill_type == DefaultFillType && p.fill.is_none() && p.fill_alpha == 1.0
    );
    assert!(p.stroke_line_width == DefaultStrokeLineWidth && p.stroke_line_miter == DefaultStrokeLineMiter);
    assert!(p.stroke_line_cap == DefaultStrokeLineCap && p.stroke_line_join == DefaultStrokeLineJoin);
    assert!(p.trim_path_start == DefaultTrimPathStart && p.trim_path_end == DefaultTrimPathEnd);
    assert_eq!(p.trim_path_offset, DefaultTrimPathOffset);
    let g = VectorGroup::default();
    assert!(g.name == DefaultGroupName && g.rotation == DefaultRotation && g.pivot_x == DefaultPivotX);
    assert!(g.pivot_y == DefaultPivotY && g.scale_x == DefaultScaleX && g.scale_y == DefaultScaleY);
    assert!(g.translation_x == DefaultTranslationX && g.translation_y == DefaultTranslationY);
    assert!(g.clip_path_data == EmptyPath && g.children.is_empty());
    assert!(DefaultTintColor == Color::Transparent && DefaultTintBlendMode == BlendMode::SrcIn);
    assert_eq!((DefaultStrokeLineWidth, DefaultStrokeLineMiter, DefaultTrimPathEnd), (0.0, 4.0, 1.0));
}

#[test]
fn image_vectors_come_from_svg_path_data() {
    let add = ImageVector::from_svg_path("M19,13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z", 24.0, 24.0);
    assert!(add.default_width() == Dp(24.0) && add.default_height() == Dp(24.0) && add.viewport_width() == 24.0);
    assert!(add.root().size() == 1 && add.tint_color().is_unspecified() && !add.auto_mirror());
    let p = add.root()[0].path().unwrap();
    assert!(p.fill == solid(Color::Black) && p.stroke.is_none() && p.path_data.len() == 14);
    assert!(add.is_one_color() && add.root_tint_color() == Color::Black);
    let wide = ImageVector::from_svg_path("M0,0h48v12z", 48.0, 12.0);
    assert!(wide.default_width() == Dp(48.0) && wide.default_height() == Dp(12.0) && wide.viewport_height() == 12.0);
    // Its id its own; equal to another made the same.
    assert!(ImageVector::from_svg_path("M0,0h1v1z", 24.0, 24.0) == ImageVector::from_svg_path("M0,0h1v1z", 24.0, 24.0));
    assert_ne!(add.id(), ImageVector::from_svg_path("M19,13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z", 24.0, 24.0).id());
}
