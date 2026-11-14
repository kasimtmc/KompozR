//! Units, geometry and Matrix, against what Compose gives.

use kompoz_graphics::*;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn dp_and_sp_turn_into_pixels_by_density() {
    let d = Density::new(2.625, 1.3);
    assert_eq!(d.to_px(16.dp()), 42.0);
    assert_eq!(d.round_to_px(10.dp()), 26); // 26.25 rounds to 26
    assert_eq!(d.round_to_px(Dp(0.2)), 1); // 0.525: a half up
    assert_eq!(d.round_to_px(Dp::Infinity), i32::MAX);
    assert!(near(d.to_dp(42.0).0, 16.0));
    assert!(near(d.text_to_px(10.sp()), 10.0 * 1.3 * 2.625));
    assert!(near(d.to_sp(d.text_to_px(14.sp())).value(), 14.0));
    assert_eq!(d.size_to_px(DpSize::new(2.dp(), 4.dp())), Size::new(5.25, 10.5));
    assert!(Dp::Unspecified.is_unspecified() && 3.dp().is_specified());
    assert_eq!(Dp::Unspecified.take_or_else(|| 5.dp()), 5.dp());
    assert_eq!(16.dp() + 8.dp(), 24.dp());
    assert_eq!(16.dp() / 4.dp(), 4.0);
    assert_eq!(3.dp().coerce_in(4.dp(), 8.dp()), 4.dp());
    assert!(1.5.em().is_em() && 14.sp().is_sp() && TextUnit::Unspecified.is_unspecified());
}

#[test]
fn rects_are_by_their_edges() {
    let r = Rect::new(10.0, 20.0, 110.0, 70.0);
    assert_eq!((r.width(), r.height()), (100.0, 50.0));
    assert_eq!(r.center(), Offset::new(60.0, 45.0));
    assert!(r.contains(Offset::new(10.0, 20.0)) && !r.contains(Offset::new(110.0, 45.0)));
    assert_eq!(r.translate(Offset::new(-10.0, 5.0)), Rect::new(0.0, 25.0, 100.0, 75.0));
    assert_eq!(r.inflate(5.0), Rect::new(5.0, 15.0, 115.0, 75.0));
    assert_eq!(r.intersect(Rect::new(100.0, 0.0, 200.0, 30.0)), Rect::new(100.0, 20.0, 110.0, 30.0));
    assert!(r.overlaps(Rect::new(109.0, 69.0, 200.0, 200.0)) && !r.overlaps(Rect::new(110.0, 0.0, 200.0, 200.0)));
    assert!(Rect::new(5.0, 5.0, 5.0, 10.0).is_empty());
    assert_eq!(Rect::from_center(Offset::new(5.0, 5.0), 2.0), Rect::new(3.0, 3.0, 7.0, 7.0));
    assert_eq!(Rect::new(0.4, 0.6, 9.5, 9.4).round_out(), IntRect::new(0, 0, 10, 10));
    assert_eq!(lerp_rect(Rect::Zero, r, 0.5), Rect::new(5.0, 10.0, 55.0, 35.0));
    assert_eq!(Size::new(30.0, -10.0).min_dimension(), 10.0);
    assert!(Size::new(0.0, 10.0).is_empty() && Size::Unspecified.is_unspecified());
    assert!(near(Offset::new(3.0, 4.0).get_distance(), 5.0));
    assert!(Offset::Unspecified.is_unspecified() && !Offset::Unspecified.is_valid());
}

#[test]
fn round_rects_count_their_corners() {
    let rr = RoundRect::from_rect_radius(Rect::new(0.0, 0.0, 100.0, 50.0), CornerRadius::circular(20.0));
    assert!(rr.contains(Offset::new(50.0, 25.0)));
    assert!(!rr.contains(Offset::new(1.0, 1.0))); // outside the top left corner's curve
    assert!(rr.contains(Offset::new(6.0, 6.0))); // inside it
    assert!(!rr.is_rect() && rr.is_simple());
    // Radii bigger than the sides scaled down together.
    let big =
        RoundRect::from_rect_radius(Rect::new(0.0, 0.0, 100.0, 50.0), CornerRadius::circular(50.0)).scaled_radii();
    assert_eq!(big.top_left_corner_radius, CornerRadius::circular(25.0));
}

#[test]
fn matrices_transform_as_composes_do() {
    let mut m = Matrix::new();
    m.translate(10.0, 20.0, 0.0);
    assert_eq!(m.map(Offset::new(1.0, 2.0)), Offset::new(11.0, 22.0));
    m.scale(2.0, 3.0, 1.0); // before what it did
    assert_eq!(m.map(Offset::new(1.0, 1.0)), Offset::new(12.0, 23.0));
    let mut r = Matrix::new();
    r.rotate_z(90.0);
    let p = r.map(Offset::new(1.0, 0.0)); // clockwise on the screen: x goes to y
    assert!(near(p.x, 0.0) && near(p.y, 1.0), "{p:?}");
    let mut inv = m;
    inv.invert();
    let mut both = m;
    both.times_assign(&inv);
    for (a, b) in both.values.iter().zip(Matrix::IDENTITY.values) {
        assert!(near(*a, b));
    }
    assert_eq!(m.map_rect(Rect::new(0.0, 0.0, 1.0, 1.0)), Rect::new(10.0, 20.0, 12.0, 23.0));
    // graphicsLayer's: scaled 2x about (50, 50), then moved by (5, 0).
    let mut g = Matrix::new();
    g.reset_to_pivoted_transform(50.0, 50.0, 5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 2.0, 1.0);
    assert_eq!(g.map(Offset::new(50.0, 50.0)), Offset::new(55.0, 50.0));
    assert_eq!(g.map(Offset::new(60.0, 50.0)), Offset::new(75.0, 50.0));
    let mut singular = Matrix::new();
    singular.scale(0.0, 1.0, 1.0);
    let before = singular;
    singular.invert();
    assert_eq!(singular, before); // no inverse: left as it is
}
