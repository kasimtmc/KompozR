//! Android's stretch effect as hwui has it (StretchEffect.h/.cpp): none within its epsilon, merged as
//! RenderNode.stretch merges, the shader's uniforms as getShader works them out, the shader's warp and its reverse,
//! the uniform scale.

use kompoz_graphics::*;

fn o(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

#[test]
fn a_stretch_is_none_within_its_epsilon() {
    assert!(StretchEffect::Empty.is_empty() && StretchEffect::default() == StretchEffect::Empty);
    // (1/25,000 of a screen.)
    assert!(StretchEffect::new(o(0.00004, -0.00004), 10.0, 10.0).is_empty());
    assert!(!StretchEffect::new(o(0.0, 0.00005), 10.0, 10.0).is_empty());
    assert!(StretchEffect::new(o(-0.00005, 0.0), 10.0, 10.0).requires_layer());
    assert!(!StretchEffect::new(o(0.00001, 0.0), 1.0, 1.0).requires_layer());
    let mut s = StretchEffect::new(o(0.2, 0.1), 30.0, 40.0);
    s.set_empty();
    assert_eq!(s, StretchEffect::Empty);
}

#[test]
fn stretches_merge_as_render_nodes_do() {
    let top = StretchEffect::new(o(0.0, 0.03), 100.0, 200.0);
    let left = StretchEffect::new(o(0.02, 0.0), 300.0, 50.0);
    // Onto none: the other as it is; none onto one: nothing changes.
    let mut s = StretchEffect::Empty;
    s.merge_with(&top);
    assert_eq!(s, top);
    s.merge_with(&StretchEffect::Empty);
    assert_eq!(s, top);
    // Two: the directions add up, each most the larger.
    s.merge_with(&left);
    assert_eq!(s, StretchEffect::new(o(0.02, 0.03), 300.0, 200.0));
    assert_eq!(s.stretch_direction(), o(0.02, 0.03));
    // Pulled back as far the other way (EdgeEffect's negation): none at all, its mosts gone too.
    s.merge_with(&StretchEffect::new(o(-0.02, -0.03), 500.0, 500.0));
    assert_eq!(s, StretchEffect::Empty);
    // Equal when all three are.
    assert_ne!(StretchEffect::new(o(0.1, 0.0), 1.0, 2.0), StretchEffect::new(o(0.1, 0.0), 1.0, 3.0));
}

#[test]
fn the_shaders_uniforms_are_get_shaders() {
    assert_eq!(StretchEffect::Empty.uniforms(100.0, 50.0), None);
    let u = StretchEffect::new(o(0.25, -0.5), 9.0, 9.0).uniforms(200.0, 100.0).expect("stretched");
    assert_eq!(u.stretch_affected_dist_x, 200.0);
    assert_eq!(u.stretch_affected_dist_y, 100.0);
    assert_eq!(u.distance_stretched_x, 160.0); // 200 / 1.25
    assert!(near(u.distance_stretched_y, 66.666_67, 1e-3)); // 100 / 1.5
    assert_eq!([u.inverse_distance_stretched_x, u.inverse_distance_stretched_y], [0.005, 0.01]);
    assert_eq!(u.dist_diff_x, -40.0);
    assert!(near(u.dist_diff_y, -33.333_33, 1e-3));
    assert_eq!([u.overscroll_x, u.overscroll_y], [0.25, -0.5]);
    assert_eq!([u.viewport_width, u.viewport_height], [200.0, 100.0]);
    assert_eq!([u.scroll_x, u.scroll_y, u.max_stretch_intensity], [0.0; 3]);
    assert_eq!(u.interpolation_strength, 0.7);
    let v = u.values();
    assert!(v[0] == 0.0 && v[1] == 200.0 && v[11] == 0.25 && v[12] == -0.5 && v[15] == 0.7);
}

#[test]
fn the_warp_stretches_from_the_edge_pulled() {
    let (w, h) = (200.0f32, 100.0f32);
    // Pulled from the left: its left edge stays, the layer shows the content up to w / (1 + o), the content near
    // the left stretched most; past the layer, the content moves by the difference.
    let s = StretchEffect::new(o(0.3, 0.0), w, h).uniforms(w, h).expect("stretched");
    let at = |x: f32| s.content_position(o(x, 40.0));
    assert_eq!(at(0.0), o(0.0, 40.0)); // (down: none)
    assert!(near(at(w).x, w / 1.3, 1e-3));
    assert!(near(at(250.0).x, 250.0 + (w / 1.3 - w), 1e-3));
    let mut last = -1.0;
    for i in 0..=200 {
        let x = at(i as f32).x;
        assert!(x > last, "it keeps the content in order");
        last = x;
    }
    let (near_edge, far_edge) = (at(10.0).x - at(0.0).x, at(200.0).x - at(190.0).x);
    assert!(near_edge < far_edge, "the content near the edge pulled stretched most: {near_edge} {far_edge}");
    // Pulled from the bottom: its bottom stays, the content from h - h / (1 + o) shows at its top.
    let s = StretchEffect::new(o(0.0, -0.3), w, h).uniforms(w, h).expect("stretched");
    assert!(near(s.content_position(o(5.0, h)).y, h, 1e-3) && s.content_position(o(5.0, h)).x == 5.0);
    assert!(near(s.content_position(o(5.0, 0.0)).y, h - h / 1.3, 1e-3));
    assert!(near(s.content_position(o(5.0, -20.0)).y, -20.0 + (h - h / 1.3), 1e-3));
}

#[test]
fn stretched_positions_are_the_warps_reverse() {
    let (w, h) = (360.0f32, 240.0f32);
    for (dx, dy) in [(0.3, 0.0), (0.0, 0.3), (-0.3, 0.0), (0.0, -0.3), (0.032, -0.016), (-0.5, 0.5)] {
        let e = StretchEffect::new(o(dx, dy), w, h);
        let u = e.uniforms(w, h).expect("stretched");
        // (Within the layer: at the edge away from the one pulled hwui's reverse steps off, as written there, which
        // a float a hair past the edge takes.)
        for i in 1..20 {
            let (x, y) = (i as f32 / 20.0 * w, i as f32 / 20.0 * h);
            let content = u.content_position(o(x, y));
            // Where the content shown at x lies shows at x again.
            let back_x = e.compute_stretched_position_x(content.x / w) * w;
            let back_y = e.compute_stretched_position_y(content.y / h) * h;
            assert!(near(back_x, x, 2e-3 * w) && near(back_y, y, 2e-3 * h), "{dx} {dy}: {x} {y} -> {back_x} {back_y}");
        }
    }
    // Along an axis it doesn't stretch: where it is.
    assert_eq!(StretchEffect::new(o(0.3, 0.0), w, h).compute_stretched_position_y(0.37), 0.37);
}

#[test]
fn the_linear_stretch_scales_from_the_edge_pulled() {
    let (w, h) = (200.0, 100.0);
    let m = StretchEffect::new(o(0.25, -0.5), 1.0, 1.0).make_linear_stretch(w, h);
    // About (0, h): the left and the bottom stay.
    assert_eq!(m.map(o(0.0, h)), o(0.0, h));
    let p = m.map(o(100.0, 50.0));
    assert!(near(p.x, 125.0, 1e-4) && near(p.y, 25.0, 1e-4), "{p:?}");
}
