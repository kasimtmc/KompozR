//! Shapes, corner sizes, their outlines and lerp; Material's shapes, rounded polygons, morphs, SVG path data and
//! serialized features (the C++ Kompoz's tests about their values, with Compose's behaviour where the C++
//! simplified it).

use kompoz_graphics::material_shapes::*;
use kompoz_graphics::*;
use kompoz_runtime::kompozable_block;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

// ---- Helpers ------------------------------------------------------------------------------------------------------

fn o(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() < tolerance
}

fn panics(f: impl FnOnce()) -> bool {
    catch_unwind(AssertUnwindSafe(f)).is_err()
}

const LTR: LayoutDirection = LayoutDirection::Ltr;
const RTL: LayoutDirection = LayoutDirection::Rtl;
const ONE: Density = Density::new(1.0, 1.0);

/// An outline's corners as drawn (top left, top right, bottom right, bottom left), a rect's none.
fn radii(outline: &Outline) -> [f32; 4] {
    match outline {
        Outline::Rectangle(_) => [0.0; 4],
        Outline::Rounded(r) => [
            r.top_left_corner_radius.x,
            r.top_right_corner_radius.x,
            r.bottom_right_corner_radius.x,
            r.bottom_left_corner_radius.x,
        ],
        Outline::Generic(_) => panic!("a path"),
    }
}

fn same_radii(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| near(*x, y, 1e-3))
}

fn outline_of(shape: &dyn Shape, size: Size, direction: LayoutDirection) -> Outline {
    shape.create_outline(size, direction, &ONE)
}

fn path_of(outline: Outline) -> Path {
    match outline {
        Outline::Generic(path) => path,
        other => panic!("not a path: {other:?}"),
    }
}

// ---- Shapes and their outlines ------------------------------------------------------------------------------

#[test]
fn shapes_make_outlines_as_compose() {
    let size = Size::new(100.0, 40.0);
    assert_eq!(outline_of(&RectangleShape, size, LTR), Outline::Rectangle(Rect::new(0.0, 0.0, 100.0, 40.0)));
    assert!(matches!(outline_of(&RoundedCornerShape(0), size, LTR), Outline::Rectangle(_)));
    let circle = outline_of(&CircleShape, size, LTR);
    assert!(matches!(circle, Outline::Rounded(_)) && same_radii(radii(&circle), [20.0; 4]));
    // Corners of their own, dp by the density; the start on the right right to left.
    let tab = RoundedCornerShape::new(8.dp(), 4.dp(), 0.dp(), 2.dp());
    let two = Density::new(2.0, 1.0);
    assert!(same_radii(radii(&tab.create_outline(size, LTR, &two)), [16.0, 8.0, 0.0, 4.0]));
    assert!(same_radii(radii(&tab.create_outline(size, RTL, &two)), [8.0, 16.0, 4.0, 0.0]));
    let absolute = AbsoluteRoundedCornerShape::new(16.0, 8.0, 0.0, 4.0);
    assert!(same_radii(radii(&outline_of(&absolute, size, RTL)), [16.0, 8.0, 0.0, 4.0]));
    // Cut corners: a path.
    let cut = path_of(outline_of(&CutCornerShape(10.0), size, LTR));
    assert!(!cut.contains(o(2.0, 2.0)) && cut.contains(o(12.0, 2.0)));
    // A generic shape builds its path in the size and direction asked for, then closes it.
    let seen: Arc<Mutex<Option<(Size, LayoutDirection)>>> = Arc::default();
    let notch = {
        let seen = seen.clone();
        GenericShape(move |path, size, direction| {
            *seen.lock().unwrap() = Some((size, direction));
            path.move_to(0.0, 0.0);
            path.line_to(size.width / 2.0, 0.0);
            path.line_to(size.width / 2.0, size.height);
            path.line_to(0.0, size.height);
        })
    };
    let half = outline_of(&notch, Size::new(80.0, 30.0), RTL);
    assert_eq!(*seen.lock().unwrap(), Some((Size::new(80.0, 30.0), RTL)));
    assert_eq!(half.bounds(), Rect::new(0.0, 0.0, 40.0, 30.0));
    assert!(matches!(half, Outline::Generic(ref p) if matches!(p.commands().last(), Some(PathCommand::Close))));
    let copy = notch.clone();
    assert!(copy == notch && GenericShape(|_, _, _| {}) != notch);
    // A shape of one's own outline: an elliptical rounded rect.
    let elliptical = OutlineShape(|size, _, _| {
        Outline::Rounded(RoundRect::from_rect_radius(size.to_rect(), CornerRadius::new(30.0, 10.0)))
    });
    let Outline::Rounded(r) = outline_of(&elliptical, size, LTR) else { panic!("rounded") };
    assert_eq!(r.top_left_corner_radius, CornerRadius::new(30.0, 10.0));
    assert!(elliptical.clone() == elliptical && OutlineShape(|s, _, _| Outline::Rectangle(s.to_rect())) != elliptical);
    // Equal by what they are, across types too.
    let shapes: [Arc<dyn Shape>; 4] = [
        Arc::new(RectangleShape),
        Arc::new(CircleShape),
        Arc::new(RoundedCornerShape(50)),
        Arc::new(CutCornerShape(50)),
    ];
    assert!(*shapes[1] == *shapes[2] && *shapes[0] != *shapes[1] && *shapes[2] != *shapes[3]);
    let rectangle: &dyn Shape = &RectangleShape;
    assert!(*shapes[0] == *rectangle && format!("{:?}", shapes[0]) == "RectangleShape");
}

#[test]
fn corners_of_their_own_fit_and_mirror_in_rtl() {
    let size = Size::new(100.0, 40.0);
    let tab = RoundedCornerShape::new(16.0, 8.0, 0.0, 4.0);
    assert!(same_radii(radii(&outline_of(&tab, size, LTR)), [16.0, 8.0, 0.0, 4.0]));
    assert!(same_radii(radii(&outline_of(&tab, size, RTL)), [8.0, 16.0, 4.0, 0.0])); // the start on the right
    // A side's two corners fit in the shorter side (both shrunk), as Compose's.
    let left = RoundedCornerShape::new(30.0, 0.0, 0.0, 30.0);
    assert!(same_radii(radii(&outline_of(&left, size, LTR)), [20.0, 0.0, 0.0, 20.0]));
    let right = RoundedCornerShape::new(0.0, 30.0, 30.0, 0.0);
    assert!(same_radii(radii(&outline_of(&right, size, LTR)), [0.0, 20.0, 20.0, 0.0]));
    let mixed = RoundedCornerShape::new(50, 8.0, 8.0, 50);
    assert!(same_radii(radii(&outline_of(&mixed, size, LTR)), [20.0, 8.0, 8.0, 20.0]));
    assert!(same_radii(radii(&outline_of(&RoundedCornerShape(12.0), size, RTL)), [12.0; 4]));
    // Corners that are none: a rect.
    assert!(matches!(outline_of(&CutCornerShape(0), size, LTR), Outline::Rectangle(_)));
    // A negative one: as Compose throws.
    assert!(panics(|| {
        outline_of(&RoundedCornerShape(-4.0), size, LTR);
    }));
}

#[test]
fn cut_corner_and_absolute_shapes() {
    let size = Size::new(100.0, 40.0);
    // The top start corner cut 10 along each side.
    let cut = CutCornerShape::new(10.0, 0.0, 0.0, 0.0);
    let ltr = path_of(outline_of(&cut, size, LTR));
    assert!(!ltr.contains(o(3.0, 3.0)) && ltr.contains(o(97.0, 3.0)) && ltr.contains(o(12.0, 3.0)));
    // Right to left the start is on the right; an absolute one stays on the left.
    let rtl = path_of(outline_of(&cut, size, RTL));
    assert!(rtl.contains(o(3.0, 3.0)) && !rtl.contains(o(97.0, 3.0)));
    let absolute = path_of(outline_of(&AbsoluteCutCornerShape::new(10.0, 0.0, 0.0, 0.0), size, RTL));
    assert!(!absolute.contains(o(3.0, 3.0)) && absolute.contains(o(97.0, 3.0)));
    // An absolute rounded one keeps its top left corner rounded.
    let rounded = outline_of(&AbsoluteRoundedCornerShape::new(10.0, 0.0, 0.0, 0.0), size, RTL);
    assert!(same_radii(radii(&rounded), [10.0, 0.0, 0.0, 0.0]));
    // Cuts of half the shorter side: diamond ends; too large ones shrink to fit.
    let diamond = path_of(outline_of(&CutCornerShape(50), size, LTR));
    assert!(diamond.contains(o(50.0, 20.0)) && diamond.contains(o(21.0, 20.0)) && !diamond.contains(o(10.0, 5.0)));
    let cut30 = path_of(outline_of(&CutCornerShape(30.0), Size::new(60.0, 40.0), LTR));
    assert!(cut30.contains(o(30.0, 1.0)) && !cut30.contains(o(2.0, 2.0)));
    assert!(cut30.contains(o(12.0, 12.0))); // the cuts fit the 40 side: 20 each
    // Compose's path: from the top start corner's left end, round clockwise.
    assert_eq!(cut30.commands()[0], PathCommand::Move(o(0.0, 20.0)));
}

#[test]
fn corner_sizes_are_composes() {
    let size = Size::new(100.0, 40.0);
    let density = Density::new(2.5, 1.0);
    assert_eq!(CornerSize(8.dp()).to_px(size, &density), 20.0);
    assert_eq!(CornerSize(6.0).to_px(size, &density), 6.0);
    assert_eq!(CornerSize(25).to_px(size, &density), 10.0); // a quarter of the shorter side
    assert_eq!(ZeroCornerSize.to_px(size, &density), 0.0);
    assert!(
        panics(|| {
            CornerSize(101);
        }) && panics(|| {
            CornerSize(-1);
        })
    );
    // Equal by what they are; printed as Compose's.
    assert!(CornerSize(50) == CornerSize::percent(50) && CornerSize(6.0) == CornerSize::px(6.0));
    assert!(CornerSize(6.0) != CornerSize(6.dp()) && CornerSize::Zero == ZeroCornerSize);
    assert_eq!(format!("{:?}", CornerSize(16.dp())), "CornerSize(size = 16.0.dp)");
    assert_eq!(format!("{:?}", CornerSize(4.0)), "CornerSize(size = 4.0.px)");
    assert_eq!(format!("{:?}", CornerSize(50)), "CornerSize(size = 50.0%)");
    assert_eq!(format!("{ZeroCornerSize:?}"), "ZeroCornerSize");
    // Between two: their px mixed when resolved; equal only to itself (and its copies).
    let between = lerp_corner_size(&CornerSize(8.dp()), &CornerSize(25), 0.5);
    assert_eq!(between.to_px(size, &density), 15.0);
    assert!(between.clone() == between && lerp_corner_size(&CornerSize(8.dp()), &CornerSize(25), 0.5) != between);
    assert_eq!(
        format!("{:?}", RoundedCornerShape::new(1.dp(), 2.0, 3, ZeroCornerSize)),
        "RoundedCornerShape(topStart = CornerSize(size = 1.0.dp), topEnd = CornerSize(size = 2.0.px), bottomEnd = \
         CornerSize(size = 3.0%), bottomStart = ZeroCornerSize)"
    );
}

#[test]
fn corner_based_shapes_copy_as_composes() {
    let shape = RoundedCornerShape(8.dp());
    let top = kompozable_block!(shape.copy(bottom_end = ZeroCornerSize, bottom_start = ZeroCornerSize));
    assert_eq!(top, RoundedCornerShape::new(8.dp(), 8.dp(), ZeroCornerSize, ZeroCornerSize));
    assert_eq!(shape.copy(), shape);
    assert_eq!(shape.copy_all(CornerSize(50)), RoundedCornerShape(50));
    // Any of the four as a theme holds them: copied as its kind.
    let any: CornerBasedShape = CutCornerShape(4.dp()).into();
    let copied = kompozable_block!(any.copy(top_start = CornerSize(0)));
    assert_eq!(copied, CornerBasedShape::Cut(CutCornerShape::new(0, 4.dp(), 4.dp(), 4.dp())));
    assert_eq!(any.top_end(), &CornerSize(4.dp()));
    assert!(matches!(any.copy_all(ZeroCornerSize), CornerBasedShape::Cut(_)));
    let size = Size::new(60.0, 40.0);
    assert_eq!(outline_of(&any, size, LTR), outline_of(&CutCornerShape(4.dp()), size, LTR));
    // Shared, it is the shape it holds (Kotlin's subclass equality: a theme's shape is a rounded one there).
    let shared = |s: Arc<dyn Shape>| s;
    assert!(*shared(Arc::new(any.clone())) == *shared(Arc::new(CutCornerShape(4.dp()))));
    assert!(*shared(Arc::new(CutCornerShape(4.dp()))) == *shared(Arc::new(any.clone())));
    assert!(*shared(Arc::new(any)) != *shared(Arc::new(RoundedCornerShape(4.dp()))));
    let rounded: CornerBasedShape = RoundedCornerShape(8.dp()).into();
    assert!(*shared(Arc::new(rounded.clone())) == *shared(Arc::new(rounded)));
    assert!(
        *shared(Arc::new(CornerBasedShape::from(RoundedCornerShape(8.dp()))))
            == *shared(Arc::new(RoundedCornerShape(8.dp())))
    );
}

#[test]
fn shapes_lerp_as_composes() {
    let size = Size::new(100.0, 40.0);
    let shape = |s: Arc<dyn Shape>| s;
    let circle = shape(Arc::new(CircleShape));
    // Corner by corner, each corner's px mixed.
    let to_tab = lerp_shape(&circle, &shape(Arc::new(RoundedCornerShape::new(0.0, 8.0, 8.0, 0.0))), 0.5);
    assert!(same_radii(radii(&outline_of(&*to_tab, size, LTR)), [10.0, 14.0, 14.0, 10.0]));
    let to_twelve = lerp_shape(&circle, &shape(Arc::new(RoundedCornerShape(12.dp()))), 0.5);
    assert!(same_radii(radii(&outline_of(&*to_twelve, size, LTR)), [16.0; 4])); // 6 px + 25 %
    // A rectangle as corners of none (either way round).
    let rect = shape(Arc::new(RectangleShape));
    assert!(same_radii(radii(&outline_of(&*lerp_shape(&rect, &circle, 1.0), size, LTR)), [20.0; 4]));
    assert!(same_radii(radii(&outline_of(&*lerp_shape(&rect, &circle, 0.25), size, LTR)), [5.0; 4]));
    assert!(same_radii(radii(&outline_of(&*lerp_shape(&circle, &rect, 0.25), size, LTR)), [15.0; 4]));
    // To nothing: as to no corners.
    let none = CircleShape.lerp(None, 0.5).unwrap();
    assert!(same_radii(radii(&outline_of(&*none, size, LTR)), [10.0; 4]));
    // Cut ones mix with cut ones; other kinds, and shapes of one's own, switch half way.
    let cut = shape(Arc::new(CutCornerShape(10.0)));
    let cut_more = lerp_shape(&cut, &shape(Arc::new(CutCornerShape(20.0))), 0.5);
    assert!((*cut_more).shape_as_any().is::<CutCornerShape>());
    let p = path_of(outline_of(&*cut_more, size, LTR));
    assert_eq!(p.commands()[0], PathCommand::Move(o(0.0, 15.0)));
    assert!(*lerp_shape(&cut, &circle, 0.49) == *cut && *lerp_shape(&cut, &circle, 0.5) == *circle);
    let own = shape(Arc::new(GenericShape(|_, _, _| {})));
    assert!(
        Arc::ptr_eq(&lerp_shape(&own, &circle, 0.2), &own) && Arc::ptr_eq(&lerp_shape(&own, &circle, 0.7), &circle)
    );
    let absolute = shape(Arc::new(AbsoluteRoundedCornerShape(10.0)));
    let mixed = lerp_shape(&absolute, &shape(Arc::new(AbsoluteRoundedCornerShape(20.0))), 0.5);
    assert!((*mixed).shape_as_any().is::<AbsoluteRoundedCornerShape>());
    assert!(same_radii(radii(&outline_of(&*mixed, size, RTL)), [15.0; 4]));
    // Equal ones switch half way.
    let same = shape(Arc::new(CircleShape));
    assert!(
        Arc::ptr_eq(&lerp_shape(&circle, &same, 0.3), &circle) && Arc::ptr_eq(&lerp_shape(&circle, &same, 0.6), &same)
    );
    // A theme's shape lerps as its kind.
    let theme = shape(Arc::new(CornerBasedShape::from(RoundedCornerShape(10.0))));
    let towards = lerp_shape(&theme, &shape(Arc::new(RoundedCornerShape(20.0))), 0.5);
    assert!(same_radii(radii(&outline_of(&*towards, size, LTR)), [15.0; 4]));
}

#[test]
fn shared_shapes_equal_shapes_of_a_kind_as_kotlin() {
    // Kotlin compares a `Shape` with a shape of a kind by `equals` (`shapes.pressedShape == CircleShape`): either way
    // round, a theme's corner-based shape as the kind it holds, a rectangle as a rectangle.
    let circle: Arc<dyn Shape> = Arc::new(CircleShape);
    assert!(circle == CircleShape && CircleShape == circle);
    assert!(circle != RoundedCornerShape(4.0) && circle != RectangleShape);
    let theme: Arc<dyn Shape> = Arc::new(CornerBasedShape::from(RoundedCornerShape(12.0)));
    let held: CornerBasedShape = RoundedCornerShape(12.0).into();
    assert!(theme == RoundedCornerShape(12.0) && theme == held);
    assert!(theme != CutCornerShape(12.0));
    let rectangle: Arc<dyn Shape> = Arc::new(RectangleShape);
    assert!(rectangle == RectangleShape && RectangleShape == rectangle);
}

// ---- Rounded polygons, Material's shapes and morphs -------------------------------------------------------------

/// Points along a polygon's cubics, in order round it.
fn samples(cubics: &[Cubic], per_cubic: usize) -> Vec<Offset> {
    cubics.iter().flat_map(|c| (0..per_cubic).map(move |i| c.point_on_curve(i as f32 / per_cubic as f32))).collect()
}

/// How many times its distance from the middle of the unit square rises past its mean, going round.
fn bumps(polygon: &RoundedPolygon) -> usize {
    let d: Vec<f32> = samples(polygon.cubics(), 40).iter().map(|p| (*p - o(0.5, 0.5)).get_distance()).collect();
    let mean = d.iter().sum::<f32>() / d.len() as f32;
    (0..d.len()).filter(|&i| d[(i + d.len() - 1) % d.len()] < mean && d[i] >= mean).count()
}

#[test]
fn material_shapes_fill_the_unit_square_and_morph() {
    // Every shape: normalized into the unit square, filling it one way at least, centered there; the same object each
    // time; its outline contiguous.
    let all = MaterialShapes::all();
    assert_eq!(all.len(), 35);
    assert_eq!(all[19].0, "Cookie9Sided");
    assert!(std::ptr::eq(all[19].1(), MaterialShapes::Cookie9Sided()));
    for (name, shape) in all {
        let polygon = shape();
        let [x0, y0, x1, y1] = polygon.calculate_bounds(true);
        let fits = x0 >= -1e-4 && y0 >= -1e-4 && x1 <= 1.0 + 1e-4 && y1 <= 1.0 + 1e-4;
        let fills = near(x1 - x0, 1.0, 1e-3) || near(y1 - y0, 1.0, 1e-3);
        let centered = near(x0, 1.0 - x1, 1e-3) && near(y0, 1.0 - y1, 1e-3);
        assert!(fits && fills && centered, "{name}: {x0} {y0} {x1} {y1}");
        assert!(std::ptr::eq(polygon, shape()), "{name}");
        assert!(polygon.cubics().len() >= 4 && polygon.features().len() >= 4, "{name}");
        // its exact bounds within the control points'
        let [ex0, ey0, ex1, ey1] = polygon.calculate_bounds(false);
        assert!(ex0 >= x0 - 1e-5 && ey0 >= y0 - 1e-5 && ex1 <= x1 + 1e-5 && ey1 <= y1 + 1e-5, "{name}");
    }
    // A circle is round; a square's corners rounded off; a triangle points up; a pill lies across the diagonal.
    for p in samples(MaterialShapes::Circle().cubics(), 8) {
        assert!(near((p - o(0.5, 0.5)).get_distance(), 0.5, 0.01));
    }
    let square = samples(MaterialShapes::Square().cubics(), 8);
    let furthest = square.iter().map(|p| (*p - o(0.5, 0.5)).get_distance()).fold(0.0f32, f32::max);
    assert!(near(furthest, 0.2 * 2f32.sqrt() + 0.3, 0.01)); // its corners' arcs 0.3 round
    let apex = samples(MaterialShapes::Triangle().cubics(), 8)
        .into_iter()
        .fold(o(0.0, 2.0), |a, p| if p.y < a.y { p } else { a });
    assert!(near(apex.x, 0.5, 0.02));
    let pill = samples(MaterialShapes::Pill().cubics(), 8);
    let tip = pill.iter().fold(o(0.5, 0.5), |a, p| {
        if (*p - o(0.5, 0.5)).get_distance() > (a - o(0.5, 0.5)).get_distance() { *p } else { a }
    });
    assert!(near((tip.x - 0.5).abs(), (tip.y - 0.5).abs(), 0.05), "{tip:?}");
    // A cookie with n sides has n bumps; so do the clovers and the burst.
    assert_eq!(bumps(MaterialShapes::Cookie4Sided()), 4);
    assert_eq!(bumps(MaterialShapes::Cookie6Sided()), 6);
    assert_eq!(bumps(MaterialShapes::Cookie7Sided()), 7);
    assert_eq!(bumps(MaterialShapes::Cookie9Sided()), 9);
    assert_eq!(bumps(MaterialShapes::Cookie12Sided()), 12);
    assert_eq!(bumps(MaterialShapes::Clover4Leaf()), 4);
    assert_eq!(bumps(MaterialShapes::Clover8Leaf()), 8);
    assert_eq!(bumps(MaterialShapes::Burst()), 12);
    // A morph: the start, the end, between; its last cubic ending where its first starts.
    let morph = Morph::new(MaterialShapes::Circle().clone(), MaterialShapes::Square().clone());
    let exact = |cubics: &[Cubic]| {
        cubics.iter().map(|c| c.calculate_bounds(false)).fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |a, b| {
            [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]
        })
    };
    let same_bounds = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| near(*x, y, 1e-3));
    assert!(same_bounds(exact(&morph.as_cubics(0.0)), MaterialShapes::Circle().calculate_bounds(false)));
    assert!(same_bounds(exact(&morph.as_cubics(1.0)), MaterialShapes::Square().calculate_bounds(false)));
    let [hx0, hy0, hx1, hy1] = exact(&morph.as_cubics(0.5));
    assert!(hx0 > -1e-3 && hy0 > -1e-3 && hx1 < 1.0 + 1e-3 && hy1 < 1.0 + 1e-3 && hx1 - hx0 > 0.9);
    assert_eq!(morph.to_path(0.5, 0).commands().len(), morph.morph_match().len() + 2); // a move, the cubics, a close
    let cubics = morph.as_cubics(0.5);
    let (first, last) = (cubics[0], cubics[cubics.len() - 1]);
    assert!(first.anchor0_x() == last.anchor1_x() && first.anchor0_y() == last.anchor1_y());
    let corner = |progress: f32| {
        samples(&morph.as_cubics(progress), 8).iter().map(|p| (*p - o(0.5, 0.5)).get_distance()).fold(0.0f32, f32::max)
    };
    assert!(corner(0.0) < corner(0.5) && corner(0.5) < corner(1.0));
    let mut each = 0;
    let mut mutable = MutableCubic::new();
    morph.for_each_cubic(0.5, &mut mutable, |c| {
        assert!(c.anchor0_x().is_finite());
        each += 1;
    });
    assert_eq!(each, morph.morph_match().len());
    let [bx0, _, bx1, _] = morph.calculate_bounds(true);
    assert!(near(bx0, 0.0, 1e-4) && near(bx1, 1.0, 1e-4));
    let [mx0, my0, mx1, my1] = morph.calculate_max_bounds();
    assert!(near(mx1 - mx0, my1 - my0, 1e-4) && mx1 - mx0 >= 1.0);
}

#[test]
fn polygon_shapes_fill_the_size_they_are_given() {
    let size = Size::new(80.0, 40.0);
    let heart = MaterialShapes::Heart().to_shape(0);
    let outline = outline_of(&heart, size, LTR);
    let bounds = outline.bounds();
    // its unit square stretched over it (the heart fills it across), centered there
    assert!(near(bounds.width(), 80.0, 1e-3) && bounds.height() > 30.0 && bounds.height() <= 40.0, "{bounds:?}");
    assert!(near(bounds.center().x, 40.0, 1e-3) && near(bounds.center().y, 20.0, 1e-3));
    assert!(path_of(outline).contains(o(40.0, 25.0)));
    assert!(heart == MaterialShapes::Heart().to_shape(0) && heart != MaterialShapes::Bun().to_shape(0));
    assert!(heart != MaterialShapes::Heart().to_shape(90));
    // Turned to start where asked, about the origin, then centered again.
    let turned = outline_of(&MaterialShapes::Square().to_shape(90), size, LTR).bounds();
    assert!(near(turned.center().x, 40.0, 1e-3) && near(turned.center().y, 20.0, 1e-3));
    // A morph as a shape: its path stretched from the unit square; equal by the same morph and progress.
    let morph = Arc::new(Morph::new(MaterialShapes::Cookie9Sided().clone(), MaterialShapes::Pill().clone()));
    let shape = morph.to_shape(0.3);
    let b = outline_of(&shape, Size::new(100.0, 100.0), LTR).bounds();
    assert!(b.left >= -0.1 && b.top >= -0.1 && b.right <= 100.1 && b.bottom <= 100.1 && b.width() > 50.0);
    assert!(shape == morph.to_shape(0.3) && shape != morph.to_shape(0.4));
    let other = Arc::new(Morph::new(MaterialShapes::Cookie9Sided().clone(), MaterialShapes::Pill().clone()));
    assert!(shape != other.to_shape(0.3));
    // Through a matrix (material3's transformed): turned a quarter round its origin.
    let mut quarter = Matrix::new();
    quarter.rotate_z(90.0);
    let turned = MaterialShapes::Square().transformed_by(&quarter);
    let [x0, y0, x1, y1] = turned.calculate_bounds(true);
    assert!(near(x0, -1.0, 1e-4) && near(x1, 0.0, 1e-4) && near(y0, 0.0, 1e-4) && near(y1, 1.0, 1e-4));
}

#[test]
fn rounded_polygons_are_composes() {
    // A regular polygon: its vertices on the circle, the first at 3 o'clock; a corner and an edge each.
    let hexagon = RoundedPolygon::new(6, 2.0, 1.0, 1.0, CornerRounding::Unrounded, None);
    assert_eq!(hexagon.features().len(), 12);
    assert!(hexagon.features()[0].is_convex_corner() && hexagon.features()[1].is_edge());
    let c = hexagon.features()[0].cubics()[0];
    assert!(near(c.anchor0_x(), 3.0, 1e-5) && near(c.anchor0_y(), 1.0, 1e-5));
    assert!(hexagon.center_x() == 1.0 && hexagon.center_y() == 1.0);
    // Rounded: each corner a flanking curve, an arc and another; the outline starts in the middle of the first.
    let rounded = RoundedPolygon::new(4, 1.0, 0.0, 0.0, CornerRounding::new(0.2, 0.0), None);
    assert_eq!(rounded.features()[0].cubics().len(), 3);
    let first = rounded.cubics()[0];
    assert!(near(first.anchor0_y(), 0.0, 1e-4) && first.anchor0_x() > 0.8);
    // A circle's points a radius from its center.
    let circle = RoundedPolygon::circle(8, 3.0, 0.0, 0.0);
    for p in samples(circle.cubics(), 6) {
        assert!(near(p.get_distance(), 3.0, 0.01));
    }
    // A rectangle, a star, a pill, a pill star.
    let rect = RoundedPolygon::rectangle(4.0, 2.0, CornerRounding::Unrounded, None, 0.0, 0.0);
    assert_eq!(rect.calculate_bounds(false), [-2.0, -1.0, 2.0, 1.0]);
    let star = RoundedPolygon::star(5, 1.0, 0.5, CornerRounding::Unrounded, None, None, 0.0, 0.0);
    assert_eq!(star.features().iter().filter(|f| f.is_concave_corner()).count(), 5);
    assert_eq!(star.features().iter().filter(|f| f.is_convex_corner()).count(), 5);
    let pill = RoundedPolygon::pill(2.0, 1.0, 0.0, 0.0, 0.0);
    let [x0, y0, x1, y1] = pill.calculate_bounds(false);
    assert!(near(x0, -1.0, 1e-4) && near(x1, 1.0, 1e-4) && near(y0, -0.5, 1e-4) && near(y1, 0.5, 1e-4));
    let notch = Some(CornerRounding::new(0.1, 0.0));
    let pill_star =
        RoundedPolygon::pill_star(2.0, 1.0, 8, 0.5, CornerRounding::Unrounded, notch, None, 0.5, 0.0, 0.0, 0.0);
    assert_eq!(pill_star.features().len(), 32);
    // As Compose throws.
    assert!(panics(|| {
        RoundedPolygon::new(2, 1.0, 0.0, 0.0, CornerRounding::Unrounded, None);
    }));
    assert!(panics(|| {
        RoundedPolygon::star(5, 1.0, 1.0, CornerRounding::Unrounded, None, None, 0.0, 0.0);
    }));
    assert!(panics(|| {
        RoundedPolygon::from_vertices(&[0.0, 0.0, 1.0, 0.0, 1.0, 1.0], CornerRounding::Unrounded, Some(&[]), None);
    }));
    assert!(panics(|| {
        RoundedPolygon::pill(0.0, 1.0, 0.0, 0.0, 0.0);
    }));
    // Normalized: into the unit square, its proportions kept, centered the way it has room.
    let vertices = [0.0, 0.0, 2.0, 0.0, 2.0, 1.0, 0.0, 1.0];
    let wide = RoundedPolygon::from_vertices(&vertices, CornerRounding::Unrounded, None, None);
    assert!(wide.center_x() == 1.0 && wide.center_y() == 0.5);
    assert_eq!(wide.normalized().calculate_bounds(true), [0.0, 0.25, 1.0, 0.75]);
    let moved = wide.transformed(|x: f32, y: f32| (x + 1.0, y * 2.0));
    assert!(moved.center_x() == 2.0 && moved.center_y() == 1.0);
    assert_eq!(moved.calculate_bounds(true), [1.0, 0.0, 3.0, 2.0]);
    let [mx0, _, mx1, _] = wide.calculate_max_bounds();
    assert!(near(mx1 - mx0, 5f32.sqrt(), 1e-4));
    // A copy is equal; one of other features isn't.
    assert!(wide.clone() == wide && wide != moved);
    assert!(format!("{wide:?}").starts_with("[RoundedPolygon. Cubics = anchor0: "));
}

#[test]
fn cubics_and_features_are_composes() {
    let line = Cubic::straight_line(0.0, 0.0, 3.0, 6.0);
    assert_eq!(line.points(), &[0.0, 0.0, 1.0, 2.0, 2.0, 4.0, 3.0, 6.0]);
    let curve = Cubic(0.0, 0.0, 0.0, 10.0, 10.0, 10.0, 10.0, 0.0);
    let (a, b) = curve.split(0.5);
    assert!(a.anchor1_x() == 5.0 && a.anchor1_y() == 7.5 && b.anchor0_x() == 5.0 && b.anchor1_x() == 10.0);
    assert_eq!(curve.reverse().reverse(), curve);
    assert_eq!(curve.calculate_bounds(false), [0.0, 0.0, 10.0, 7.5]);
    assert_eq!(curve.calculate_bounds(true), [0.0, 0.0, 10.0, 10.0]);
    assert_eq!((curve + curve).points()[3], 20.0);
    assert_eq!((curve * 2).points(), (curve / 0.5).points());
    // A quarter circle: its middle on the circle.
    let arc = Cubic::circular_arc(0.0, 0.0, 1.0, 0.0, 0.0, 1.0);
    assert!(near(arc.point_on_curve(0.5).get_distance(), 1.0, 1e-3));
    let moved = curve.transformed(&|x: f32, y: f32| (x + 1.0, -y));
    assert_eq!(moved.points(), &[1.0, 0.0, 1.0, -10.0, 11.0, -10.0, 11.0, 0.0]);
    assert_eq!(
        format!("{line:?}"),
        "anchor0: (0.0, 0.0) control0: (1.0, 2.0), control1: (2.0, 4.0), anchor1: (3.0, 6.0)"
    );
    let mut m = MutableCubic::new();
    m.interpolate(&line, &curve, 0.5);
    assert_eq!(m.anchor1_x(), 6.5);
    m.transform(&|x: f32, y: f32| (x * 2.0, y));
    assert_eq!(m.anchor1_x(), 13.0);
    // Features: built and checked as Compose's.
    let corner = Feature::build_convex_corner(vec![a, b]);
    assert!(corner.is_convex_corner() && !corner.is_edge() && !corner.is_ignorable_feature());
    assert!(corner.reversed().is_concave_corner() && corner.reversed().cubics()[0] == b.reverse());
    assert!(Feature::build_edge(line).is_edge() && Feature::build_ignorable_feature(vec![line]).is_ignorable_feature());
    assert!(Feature::build_concave_corner(vec![curve]).is_concave_corner());
    assert!(panics(|| {
        Feature::build_convex_corner(vec![]);
    }));
    assert!(panics(|| {
        Feature::build_convex_corner(vec![line, line]); // not continuous
    }));
    assert_eq!(format!("{:?}", Feature::build_edge(line)), "Edge");
    // A polygon of features of one's own.
    let point = |x: f32, y: f32| Feature::build_convex_corner(vec![Cubic::straight_line(x, y, x, y)]);
    let edge = |x0: f32, y0: f32, x1: f32, y1: f32| Feature::build_edge(Cubic::straight_line(x0, y0, x1, y1));
    let triangle = RoundedPolygon::from_features(
        vec![
            point(0.0, 0.0),
            edge(0.0, 0.0, 1.0, 0.0),
            point(1.0, 0.0),
            edge(1.0, 0.0, 0.0, 1.0),
            point(0.0, 1.0),
            edge(0.0, 1.0, 0.0, 0.0),
        ],
        None,
    );
    assert_eq!(triangle.cubics().len(), 3); // (the corners of no length left out)
    assert!(panics(|| {
        RoundedPolygon::from_features(vec![edge(0.0, 0.0, 3.0, 6.0), edge(9.0, 9.0, 8.0, 8.0)], None);
    }));
}

#[test]
fn svg_paths_and_serialized_features_are_composes() {
    // A triangle from SVG path data: its three corners, oriented clockwise.
    let features = SvgPathParser::parse_features("M0,0 0.5,1 1,0Z");
    assert_eq!(features.iter().filter(|f| f.is_convex_corner()).count(), 3);
    let triangle = RoundedPolygon::from_features(features.clone(), Some(o(0.5, 0.5)));
    assert_eq!(triangle.calculate_bounds(true), [0.0, 0.0, 1.0, 1.0]);
    // Relative commands, curves, arcs: the same square every way it is written.
    let square =
        |d: &str| RoundedPolygon::from_features(SvgPathParser::parse_features(d), None).calculate_bounds(false);
    let want = [0.0, 0.0, 10.0, 10.0];
    for d in ["M0 0 L10 0 L10 10 L0 10 Z", "m0,0 h10 v10 h-10 z", "M0,0 l10,0 0,10 -10,0z"] {
        let b = square(d);
        assert!(b.iter().zip(want).all(|(x, y)| near(*x, y, 1e-4)), "{d}: {b:?}");
    }
    let rounded = square("M0 5 C0 0 0 0 5 0 S10 0 10 5 Q10 10 5 10 T0 5 Z");
    assert!(near(rounded[0], 0.0, 1e-4) && near(rounded[2], 10.0, 1e-4) && rounded[3] > 7.0);
    // (two half circles: Android's arc conversion, in its floats, a little off where the ends are a diameter apart)
    let circle = square("M0 5 A5 5 0 0 1 10 5 A5 5 0 0 1 0 5 Z");
    assert!(circle.iter().zip(want).all(|(x, y)| near(*x, y, 0.02)), "{circle:?}");
    // Only the first shape; what isn't a number panics, as Compose throws.
    let first = SvgPathParser::parse_features("M0,0 1,0 1,1Z M5,5 6,5 6,6Z");
    assert!(RoundedPolygon::from_features(first, None).calculate_bounds(true)[2] <= 1.0);
    assert!(panics(|| {
        SvgPathParser::parse_features("M0,0 x,1 1,0Z");
    }));
    // Serialized and parsed back: the same features.
    let text = FeatureSerializer::serialize(&features);
    assert!(text.starts_with("V1x") || text.starts_with("V1n"), "{text}");
    assert_eq!(FeatureSerializer::parse(&text), features);
    let edge_and_corner = vec![
        Feature::build_edge(Cubic(0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0)),
        Feature::build_convex_corner(vec![
            Cubic(0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0),
            Cubic(3.0, 3.0, 4.0, 4.0, 5.0, 5.0, 6.5, 6.0),
        ]),
    ];
    let text = FeatureSerializer::serialize(&edge_and_corner);
    assert_eq!(text, "V1n0,0,1,1,2,2,3,3x0,0,1,1,2,2,3,3,4,4,5,5,6.5,6");
    assert_eq!(FeatureSerializer::parse(&text), edge_and_corner);
    // Any version read as the first; an unknown tag an edge; numbers missing panic.
    assert!(FeatureSerializer::parse("  V7 o1,1,2,2,3,3,4,4")[0].is_concave_corner());
    assert!(FeatureSerializer::parse("q1,1,2,2,3,3,4,4")[0].is_edge());
    assert!(panics(|| {
        FeatureSerializer::parse("V1 1,1,2,2");
    }));
    assert!(panics(|| {
        FeatureSerializer::parse("V1n1,1,2,2,3,3");
    }));
}
