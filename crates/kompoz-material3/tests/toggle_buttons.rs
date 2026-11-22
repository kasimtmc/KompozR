//! Material Expressive's toggle buttons as Compose's material3 (1.5.0-alpha17) has them: the C++ Kompoz's
//! toggle_buttons_morph_from_round_to_square_and_tell_they_are_checked (with M15's values where the C++ had its
//! own), Compose's behaviour (colors per kind and state, the shapes of each Expressive size, the morphing between
//! the rest, pressed and checked shapes, the elevation, the outlined one's border, a checkbox to accessibility) and
//! the 10 samples of M15's ToggleButtonSamples.kt.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::Text;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::input::PointerType;
use kompoz_ui::semantics::{Role, SemanticsProperties};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

tests!(
    toggle_buttons_morph_from_round_to_square_and_tell_they_are_checked,
    toggle_button_colors_follow_the_scheme_and_the_state,
    toggle_button_colors_copy_keeps_what_is_unspecified,
    toggle_button_defaults_follow_the_tokens,
    toggle_button_shapes_for_each_size,
    toggle_buttons_draw_their_colors_per_state,
    toggle_button_shapes_that_are_not_corner_based_switch,
    toggle_button_elevations_follow_their_kind,
    toggle_buttons_are_checkboxes_in_label_large,
    toggle_button_samples,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// A shared shape equal to `shape`.
fn same(a: &Arc<dyn Shape>, shape: impl Shape) -> bool {
    **a == *(&shape as &dyn Shape)
}

/// A node's bounds, in the unmerged tree.
fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
}

/// The container drawn under `b` (a fill centered on it): its top start radius and its color.
fn look(canvas: &RecordingCanvas, b: Rect) -> Option<(f32, Color)> {
    canvas
        .rects
        .iter()
        .zip(canvas.rect_radii.iter().zip(&canvas.rect_colors))
        .find(|(r, _)| near(r.center().y, b.center().y, 0.1) && near(r.center().x, b.center().x, 0.1))
        .map(|(_, (radius, color))| (*radius, *color))
}

/// The color of the line drawn around `b`, if any.
fn stroke_around(canvas: &RecordingCanvas, b: Rect) -> Option<Color> {
    canvas
        .strokes
        .iter()
        .zip(&canvas.stroke_colors)
        .find(|(s, _)| near(s.center().y, b.center().y, 0.5) && near(s.center().x, b.center().x, 0.5))
        .map(|(_, c)| *c)
}

/// The highest shadow drawn under `b` and its outline's top left radius.
fn shadow_under(canvas: &RecordingCanvas, b: Rect) -> Option<(f32, f32)> {
    canvas
        .elevation_shadows
        .iter()
        .filter(|e| near(e.bounds.center().x, b.center().x, 2.0) && near(e.bounds.center().y, b.center().y, 2.0))
        .map(|e| {
            let radius = match &e.outline {
                Outline::Rounded(r) => r.top_left_corner_radius.x,
                Outline::Rectangle(_) => 0.0,
                Outline::Generic(_) => -1.0,
            };
            (e.elevation(), radius)
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
}

fn press(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
}

fn release(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_up(at, -1, 0);
}

/// Long enough for the theme's fast spatial spring to settle, drawn in between (a shape's size is known once drawn).
fn settle(rule: &KompozTestRule) {
    rule.advance_time_by(1000);
    rule.draw();
    rule.advance_time_by(100);
}

fn toggle_buttons_morph_from_round_to_square_and_tell_they_are_checked() {
    // The C++'s test. M15's values: pressed, the default toggle button shapes' 6 dp corners (the C++ had 8, its
    // button's); checked, the theme's medium corners (12). The C++'s first checks (lerping a CircleShape and a
    // RectangleShape) are its shapes' own, not a toggle button's. Compose's semantics bounds of a toggle button are
    // its toggleable's (the 40 dp container; the C++ measured 48 of room and took 4 off each side).
    let bold = mutable_state_of(false);
    let italic = mutable_state_of(true);
    let clicks = Rc::new(Cell::new(0));
    let (b, i, c) = (bold.clone(), italic.clone(), clicks.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (b, i, c) = (b.clone(), i.clone(), c.clone());
        Column(move || {
            let (b, i) = (b.clone(), i.clone());
            let (c1, c2, c3) = (c.clone(), c.clone(), c.clone());
            ToggleButton(
                b.get(),
                move |on| b.set(on),
                modifier = Modifier.test_tag("kalın"),
                content = || Text("Kalın"),
            );
            OutlinedToggleButton(
                i.get(),
                move |on| i.set(on),
                modifier = Modifier.test_tag("italik"),
                content = || Text("İtalik"),
            );
            ButtonWithShapes(
                move || c1.set(c1.get() + 1),
                ButtonDefaults.shapes(),
                modifier = Modifier.test_tag("düğme"),
                content = || Text("Bas"),
            );
            TonalToggleButton(
                true,
                move |_| c2.set(c2.get() + 1),
                modifier = Modifier.test_tag("ton"),
                content = || Text("Ton"),
            );
            ToggleButton(
                true,
                move |_| c3.set(c3.get() + 1),
                modifier = Modifier.test_tag("kapalı"),
                enabled = false,
                content = || Text("Kapalı"),
            );
            ElevatedToggleButton(true, |_| {}, modifier = Modifier.test_tag("kabarık"), content = || Text("Kabarık"));
        });
    }));
    let cs = light_color_scheme();
    let at = bounds(&rule, "kalın");
    assert_eq!(look(&rule.draw(), at), Some((20.0, cs.surface_container)));
    rule.on_node_with_tag("kalın").assert_is_off();
    press(&rule, at.center());
    rule.advance_frame();
    rule.draw();
    rule.advance_frame();
    rule.advance_frame();
    let (morphing, _) = look(&rule.draw(), at).expect("the container");
    assert!(morphing < 19.5 && morphing > 6.5, "on its way: {morphing}");
    settle(&rule);
    assert!(near(look(&rule.draw(), at).unwrap().0, 6.0, 0.01), "squarer while held"); // (M15: 6, the C++ 8)
    release(&rule, at.center());
    settle(&rule);
    let (square, filled) = look(&rule.draw(), at).unwrap();
    assert!(near(square, 12.0, 0.01) && filled == cs.primary && bold.get(), "square once checked: {square}");
    rule.on_node_with_tag("kalın").assert_is_on();
    rule.on_node_with_tag("kalın").perform_click();
    settle(&rule);
    let (round, plain) = look(&rule.draw(), at).unwrap();
    assert!(!bold.get() && near(round, 20.0, 0.01) && plain == cs.surface_container);

    // The outlined one: checked, filled in the inverse surface without its line; unchecked, outlined.
    let it = bounds(&rule, "italik");
    let canvas = rule.draw();
    assert!(look(&canvas, it).unwrap().1 == cs.inverse_surface && stroke_around(&canvas, it).is_none());
    rule.on_node_with_tag("italik").perform_click();
    rule.wait_for_idle();
    assert!(!italic.get() && stroke_around(&rule.draw(), it) == Some(cs.outline_variant));

    // A plain button with shapes morphs while held too (the theme's small corners, 8 dp).
    let d = bounds(&rule, "düğme");
    assert!(near(look(&rule.draw(), d).unwrap().0, 20.0, 0.01));
    press(&rule, d.center());
    settle(&rule);
    assert!(near(look(&rule.draw(), d).unwrap().0, 8.0, 0.01));
    release(&rule, d.center());
    settle(&rule);
    assert!(near(look(&rule.draw(), d).unwrap().0, 20.0, 0.01) && clicks.get() == 1);

    // Tonal, checked: secondary, square. Disabled: the surface's text at 10%, takes no clicks.
    let canvas = rule.draw();
    let (ton_radius, ton_color) = look(&canvas, bounds(&rule, "ton")).unwrap();
    assert!(ton_color == cs.secondary && near(ton_radius, 12.0, 0.01));
    assert_eq!(look(&canvas, bounds(&rule, "kapalı")).unwrap().1, named_args!(cs.on_surface.copy(alpha = 0.1)));
    rule.on_node_with_tag("kapalı").perform_click();
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 1, "disabled");
    rule.on_node_with_tag("kapalı").assert_is_not_enabled().assert_is_on();
    // A checkbox to accessibility services, named by its label, checked.
    let told = rule
        .on_all_nodes(has_role(Role::Checkbox).and(is_on()).and(has_text("Kabarık", false, false)))
        .fetch_semantics_nodes();
    assert_eq!(told.len(), 1);
    // The shadow under the elevated one is as square as it is (12 dp corners).
    let e = bounds(&rule, "kabarık");
    let (elevation, radius) = shadow_under(&rule.draw(), e).expect("its shadow");
    assert!(near(elevation, 1.0, 0.01) && near(radius, 12.0, 0.01), "{elevation} {radius}");
}

fn toggle_button_colors_follow_the_scheme_and_the_state() {
    // Compose's ToggleButtonDefaults colors: unchecked / disabled / checked container and content of each kind.
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        MaterialTheme(
            content = move || {
                s.borrow_mut().extend([
                    ToggleButtonDefaults.toggle_button_colors(),
                    ToggleButtonDefaults.elevated_toggle_button_colors(),
                    ToggleButtonDefaults.tonal_toggle_button_colors(),
                    ToggleButtonDefaults.outlined_toggle_button_colors(),
                ]);
            },
        );
    }));
    rule.wait_for_idle();
    let cs = light_color_scheme();
    let disabled_container = named_args!(cs.on_surface.copy(alpha = 0.1));
    // (M15's Expressive button tokens: the disabled label is the surface variant's text at 38%)
    let disabled_content = named_args!(cs.on_surface_variant.copy(alpha = 0.38));
    let seen = seen.borrow();
    assert_eq!(
        seen[0],
        ToggleButtonColors::new(
            cs.surface_container,
            cs.on_surface_variant,
            disabled_container,
            disabled_content,
            cs.primary,
            cs.on_primary,
        )
    );
    assert_eq!(
        seen[1],
        ToggleButtonColors::new(
            cs.surface_container_low,
            cs.primary,
            disabled_container,
            disabled_content,
            cs.primary,
            cs.on_primary,
        )
    );
    assert_eq!(
        seen[2],
        ToggleButtonColors::new(
            cs.secondary_container,
            cs.on_secondary_container,
            disabled_container,
            disabled_content,
            cs.secondary,
            cs.on_secondary,
        )
    );
    // (the outlined one's disabled container is the outline variant at 10%)
    assert_eq!(
        seen[3],
        ToggleButtonColors::new(
            Color::Transparent,
            cs.on_surface_variant,
            named_args!(cs.outline_variant.copy(alpha = 0.1)),
            disabled_content,
            cs.inverse_surface,
            cs.inverse_on_surface,
        )
    );
}

fn toggle_button_colors_copy_keeps_what_is_unspecified() {
    let colors =
        ToggleButtonColors::new(Color::Red, Color::Green, Color::Blue, Color::Yellow, Color::Cyan, Color::Black);
    let copy = named_args!(colors.copy(content_color = Color::White, checked_container_color = Color::Magenta));
    assert_eq!(
        copy,
        ToggleButtonColors::new(Color::Red, Color::White, Color::Blue, Color::Yellow, Color::Magenta, Color::Black)
    );
    assert_eq!(colors.copy(), colors);
    // (given ones in the theme's place)
    let seen = Rc::new(Cell::new(None));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        s.set(Some(ToggleButtonDefaults.tonal_toggle_button_colors(checked_content_color = Color::Red)));
    }));
    rule.wait_for_idle();
    let c = seen.get().unwrap();
    assert!(c.checked_content_color == Color::Red && c.checked_container_color == light_color_scheme().secondary);
}

fn toggle_button_defaults_follow_the_tokens() {
    assert_eq!(ToggleButtonDefaults::MinHeight, Dp(40.0));
    assert_eq!(ToggleButtonDefaults::IconSize, Dp(20.0));
    assert_eq!(ToggleButtonDefaults::IconSpacing, Dp(8.0));
    assert_eq!(ToggleButtonDefaults::ContentPadding(), PaddingValues::new(Dp(16.0), Dp(8.0), Dp(16.0), Dp(8.0)));
    let full = RoundedCornerShape(CornerSize::percent(50));
    assert!(same(&ToggleButtonDefaults::round_shape(), full.clone()));
    assert!(same(&ToggleButtonDefaults::shape(), full));
    assert!(same(&ToggleButtonDefaults::square_shape(), RoundedCornerShape(12.dp())));
    assert!(same(&ToggleButtonDefaults::pressed_shape(), RoundedCornerShape(6.dp())));
    assert!(same(&ToggleButtonDefaults::checked_shape(), RoundedCornerShape(12.dp())));
    // Each Expressive size's square, pressed and checked corners (the theme's medium, large, extra large…).
    assert!(same(&ToggleButtonDefaults::extra_small_square_shape(), RoundedCornerShape(12.dp())));
    assert!(same(&ToggleButtonDefaults::medium_square_shape(), RoundedCornerShape(16.dp())));
    assert!(same(&ToggleButtonDefaults::large_square_shape(), RoundedCornerShape(28.dp())));
    assert!(same(&ToggleButtonDefaults::extra_large_square_shape(), RoundedCornerShape(28.dp())));
    assert!(same(&ToggleButtonDefaults::extra_small_pressed_shape(), RoundedCornerShape(8.dp())));
    assert!(same(&ToggleButtonDefaults::medium_pressed_shape(), RoundedCornerShape(12.dp())));
    assert!(same(&ToggleButtonDefaults::large_pressed_shape(), RoundedCornerShape(16.dp())));
    assert!(same(&ToggleButtonDefaults::extra_large_pressed_shape(), RoundedCornerShape(16.dp())));
    assert!(same(&ToggleButtonDefaults::extra_small_checked_square_shape(), RoundedCornerShape(12.dp())));
    assert!(same(&ToggleButtonDefaults::medium_checked_square_shape(), RoundedCornerShape(16.dp())));
    assert!(same(&ToggleButtonDefaults::large_checked_square_shape(), RoundedCornerShape(28.dp())));
    assert!(same(&ToggleButtonDefaults::extra_large_checked_square_shape(), RoundedCornerShape(28.dp())));
}

fn toggle_button_shapes_for_each_size() {
    // Compose's shapesFor: round at rest; pressed and checked in the corners of the nearest size (32, 40, 56, 96,
    // 136 dp: halfway between two goes to the smaller); shapes() the default ones, given ones replacing them.
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let mut s = s.borrow_mut();
        for h in [32.0, 36.0, 40.0, 48.0, 56.0, 76.0, 96.0, 116.0, 136.0] {
            s.push(ToggleButtonDefaults.shapes_for(Dp(h)));
        }
        s.push(ToggleButtonDefaults.shapes());
        s.push(named_args!(ToggleButtonDefaults.shapes(checked_shape = Some(Arc::new(CircleShape)))));
    }));
    rule.wait_for_idle();
    let seen = seen.borrow();
    let corners = |s: &ToggleButtonShapes| {
        let r = |shape: &Arc<dyn Shape>| match shape.create_outline(
            Size::new(200.0, 200.0),
            LayoutDirection::Ltr,
            &Density::new(1.0, 1.0),
        ) {
            Outline::Rounded(r) => r.top_left_corner_radius.x,
            Outline::Rectangle(_) => 0.0,
            Outline::Generic(_) => -1.0,
        };
        (r(&s.shape), r(&s.pressed_shape), r(&s.checked_shape))
    };
    let expected = [
        (100.0, 8.0, 12.0),
        (100.0, 8.0, 12.0),
        (100.0, 6.0, 12.0),
        (100.0, 6.0, 12.0),
        (100.0, 12.0, 16.0),
        (100.0, 12.0, 16.0),
        (100.0, 16.0, 28.0),
        (100.0, 16.0, 28.0),
        (100.0, 16.0, 28.0),
    ];
    for (i, e) in expected.iter().enumerate() {
        assert_eq!(corners(&seen[i]), *e, "shapes_for #{i}");
    }
    assert_eq!(seen[9], seen[2]);
    assert_eq!(corners(&seen[10]), (100.0, 6.0, 100.0));
    // Equal by their shapes; copy keeps what isn't given.
    let shapes =
        ToggleButtonShapes::new(Arc::new(RectangleShape), Arc::new(RoundedCornerShape(4.dp())), Arc::new(CircleShape));
    assert_eq!(shapes.copy(), shapes);
    let copied = named_args!(shapes.copy(pressed_shape = Some(Arc::new(CircleShape))));
    assert!(same(&copied.pressed_shape, CircleShape) && same(&copied.shape, RectangleShape));
    assert_ne!(copied, shapes);
}

fn toggle_buttons_draw_their_colors_per_state() {
    // Unchecked, checked and disabled, each kind in its own colors; the content (a vector icon) in the content
    // color.
    let on = mutable_state_of(false);
    let o = on.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let o = o.clone();
        Column(move || {
            let checked = o.get();
            for (i, enabled) in [true, false].into_iter().enumerate() {
                let tag = |k: &str| format!("{k}{i}");
                ToggleButton(
                    checked,
                    |_| {},
                    modifier = Modifier.test_tag(tag("dolu")),
                    enabled = enabled,
                    content = || VectorIcon(Icons::Filled::Edit(), None),
                );
                ElevatedToggleButton(
                    checked,
                    |_| {},
                    modifier = Modifier.test_tag(tag("kabarık")),
                    enabled = enabled,
                    content = || VectorIcon(Icons::Filled::Edit(), None),
                );
                TonalToggleButton(
                    checked,
                    |_| {},
                    modifier = Modifier.test_tag(tag("ton")),
                    enabled = enabled,
                    content = || VectorIcon(Icons::Filled::Edit(), None),
                );
                OutlinedToggleButton(
                    checked,
                    |_| {},
                    modifier = Modifier.test_tag(tag("çizgili")),
                    enabled = enabled,
                    content = || VectorIcon(Icons::Filled::Edit(), None),
                );
            }
        });
    }));
    let cs = light_color_scheme();
    let check = |checked: bool| {
        on.set(checked);
        rule.wait_for_idle();
        let canvas = rule.draw();
        let kinds = [
            (
                "dolu",
                ToggleButtonColors::new(
                    cs.surface_container,
                    cs.on_surface_variant,
                    Color::Unspecified,
                    Color::Unspecified,
                    cs.primary,
                    cs.on_primary,
                ),
            ),
            (
                "kabarık",
                ToggleButtonColors::new(
                    cs.surface_container_low,
                    cs.primary,
                    Color::Unspecified,
                    Color::Unspecified,
                    cs.primary,
                    cs.on_primary,
                ),
            ),
            (
                "ton",
                ToggleButtonColors::new(
                    cs.secondary_container,
                    cs.on_secondary_container,
                    Color::Unspecified,
                    Color::Unspecified,
                    cs.secondary,
                    cs.on_secondary,
                ),
            ),
        ];
        for (k, colors) in kinds {
            let b = bounds(&rule, &format!("{k}0"));
            let want = if checked { colors.checked_container_color } else { colors.container_color };
            assert_eq!(look(&canvas, b).map(|l| l.1), Some(want), "{k} {checked}");
            let d = bounds(&rule, &format!("{k}1"));
            assert_eq!(
                look(&canvas, d).map(|l| l.1),
                Some(named_args!(cs.on_surface.copy(alpha = 0.1))),
                "{k} disabled"
            );
        }
        // (the outlined one: transparent and outlined unchecked, the inverse surface checked)
        let o = bounds(&rule, "çizgili0");
        if checked {
            assert_eq!(look(&canvas, o).map(|l| l.1), Some(cs.inverse_surface));
            assert!(stroke_around(&canvas, o).is_none());
        } else {
            assert_eq!(stroke_around(&canvas, o), Some(cs.outline_variant));
            let d = bounds(&rule, "çizgili1");
            assert_eq!(stroke_around(&canvas, d), Some(named_args!(cs.outline_variant.copy(alpha = 0.1))));
        }
        canvas.vector_colors.clone()
    };
    let disabled = named_args!(cs.on_surface_variant.copy(alpha = 0.38));
    assert_eq!(
        check(false),
        vec![
            cs.on_surface_variant,
            cs.primary,
            cs.on_secondary_container,
            cs.on_surface_variant,
            disabled,
            disabled,
            disabled,
            disabled
        ]
    );
    assert_eq!(
        check(true),
        vec![
            cs.on_primary,
            cs.on_primary,
            cs.on_secondary,
            cs.inverse_on_surface,
            disabled,
            disabled,
            disabled,
            disabled
        ]
    );
}

fn toggle_button_shapes_that_are_not_corner_based_switch() {
    // Shapes that aren't all corner based aren't morphed: the one of the state at once (a generic shape's path
    // pressed); checked after it is let go.
    let on = mutable_state_of(false);
    let o = on.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let o = o.clone();
        ToggleButton(
            o.get(),
            move |c| o.set(c),
            modifier = Modifier.test_tag("t"),
            shapes = ToggleButtonShapes::new(
                Arc::new(RectangleShape),
                Arc::new(GenericShape(|path: &mut Path, size: Size, _: LayoutDirection| {
                    path.add_oval(size.to_rect(), PathDirection::CounterClockwise)
                })),
                Arc::new(CutCornerShape(8.dp())),
            ),
            content = || Text("T"),
        );
    }));
    let b = bounds(&rule, "t");
    assert_eq!(look(&rule.draw(), b).map(|l| l.0), Some(0.0));
    press(&rule, b.center());
    rule.advance_frame();
    rule.advance_frame();
    let canvas = rule.draw();
    // (the ripple's faint circle aside)
    let opaque = |canvas: &RecordingCanvas| canvas.rect_colors.iter().any(|c| c.alpha() > 0.5);
    assert!(!opaque(&canvas) && !canvas.paths.is_empty(), "the oval at once");
    release(&rule, b.center());
    rule.wait_for_idle();
    assert!(on.get());
    let canvas = rule.draw();
    assert!(!opaque(&canvas) && !canvas.paths.is_empty(), "the cut corners, a path");
}

fn toggle_button_elevations_follow_their_kind() {
    // The filled toggle button's elevation: none, 1 dp hovered; the elevated one's 1 dp, 3 hovered; the tonal
    // one's none; the outlined one none (no elevation).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            ToggleButton(false, |_| {}, modifier = Modifier.test_tag("dolu"), content = || Text("A"));
            ElevatedToggleButton(false, |_| {}, modifier = Modifier.test_tag("kabarık"), content = || Text("B"));
            TonalToggleButton(false, |_| {}, modifier = Modifier.test_tag("ton"), content = || Text("C"));
            OutlinedToggleButton(false, |_| {}, modifier = Modifier.test_tag("çizgili"), content = || Text("D"));
        });
    }));
    let height = |tag: &str| shadow_under(&rule.draw(), bounds(&rule, tag)).map_or(0.0, |s| s.0);
    assert!(near(height("dolu"), 0.0, 0.01) && near(height("kabarık"), 1.0, 0.01));
    assert!(near(height("ton"), 0.0, 0.01) && near(height("çizgili"), 0.0, 0.01));
    rule.host().pointer_move(bounds(&rule, "kabarık").center(), -1, 0);
    rule.advance_time_by(300);
    assert!(near(height("kabarık"), 3.0, 0.01), "{}", height("kabarık"));
    rule.host().pointer_move(bounds(&rule, "dolu").center(), -1, 0);
    rule.advance_time_by(300);
    assert!(near(height("dolu"), 1.0, 0.01) && near(height("kabarık"), 1.0, 0.01));
    rule.host().pointer_move(bounds(&rule, "çizgili").center(), -1, 0);
    rule.advance_time_by(300);
    assert!(near(height("çizgili"), 0.0, 0.01));
}

fn toggle_buttons_are_checkboxes_in_label_large() {
    // Compose: role Checkbox, its toggle state; the label in the theme's label large; at least 40 dp tall (no
    // least width, unlike a Button's 58 dp) and the content padding of a 40 dp Expressive button (16 dp each side)
    // by default; a click calls back with the other state.
    let heard = Rc::new(RefCell::new(Vec::new()));
    let h = heard.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let h = h.clone();
        MaterialTheme(
            content = move || {
                let h = h.clone();
                ToggleButton(
                    true,
                    move |c| h.borrow_mut().push(c),
                    modifier = Modifier.test_tag("t"),
                    content = || Box(modifier = Modifier.size_wh(10.dp(), 10.dp()).test_tag("içi")),
                );
            },
        );
    }));
    let node = rule.on_node_with_tag("t").fetch_semantics_node();
    assert_eq!(node.config().get(SemanticsProperties::Role), Some(Role::Checkbox));
    rule.on_node_with_tag("t").assert_is_on().perform_click();
    assert_eq!(*heard.borrow(), vec![false]);
    let (t, inner) = (bounds(&rule, "t"), bounds(&rule, "içi"));
    assert!(near(t.height(), 40.0, 0.01) && near(inner.left - t.left, 16.0, 0.01), "{t:?} {inner:?}");
    assert!(near(t.width(), 42.0, 0.01), "{t:?}");
    let style = Rc::new(RefCell::new(None));
    let s = style.clone();
    let typography = Rc::new(RefCell::new(None));
    let ty = typography.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, ty) = (s.clone(), ty.clone());
        MaterialTheme(
            content = move || {
                *ty.borrow_mut() = Some(MaterialTheme::typography().label_large);
                let s = s.clone();
                ToggleButton(
                    false,
                    |_| {},
                    content = move || {
                        *s.borrow_mut() = Some(LocalTextStyle.current());
                    },
                );
            },
        );
    }));
    rule.wait_for_idle();
    assert_eq!(style.borrow().clone().map(|t| t.font_size), typography.borrow().clone().map(|t| t.font_size));
}

fn toggle_button_samples() {
    // M15's ToggleButtonSamples.kt, each toggled by a click; the sized ones as tall as their size, their icon
    // filled once checked.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            MaterialTheme(
                content = || {
                    Column(|| {
                        // ToggleButtonSample
                        {
                            let checked = remember(|| mutable_state_of(false));
                            let c = checked.clone();
                            ToggleButton(
                                checked.get(),
                                move |it| c.set(it),
                                modifier = Modifier.test_tag("düz"),
                                content = || Text("Button"),
                            );
                        }
                        // SquareToggleButtonSample
                        {
                            let checked = remember(|| mutable_state_of(false));
                            let c = checked.clone();
                            let shapes = ToggleButtonShapes::new(
                                ToggleButtonDefaults::square_shape(),
                                ToggleButtonDefaults::pressed_shape(),
                                ToggleButtonDefaults::round_shape(),
                            );
                            ToggleButton(
                                checked.get(),
                                move |it| c.set(it),
                                shapes = shapes,
                                modifier = Modifier.test_tag("kare"),
                                content = || Text("Button"),
                            );
                        }
                        // ElevatedToggleButtonSample
                        {
                            let checked = remember(|| mutable_state_of(false));
                            let c = checked.clone();
                            ElevatedToggleButton(
                                checked.get(),
                                move |it| c.set(it),
                                content = || Text("Elevated Button"),
                            );
                        }
                        // TonalToggleButtonSample
                        {
                            let checked = remember(|| mutable_state_of(false));
                            let c = checked.clone();
                            TonalToggleButton(checked.get(), move |it| c.set(it), content = || Text("Tonal Button"));
                        }
                        // OutlinedToggleButtonSample
                        {
                            let checked = remember(|| mutable_state_of(false));
                            let c = checked.clone();
                            OutlinedToggleButton(
                                checked.get(),
                                move |it| c.set(it),
                                content = || Text("Outlined Button"),
                            );
                        }
                        // ToggleButtonWithIconSample
                        {
                            let checked = remember(|| mutable_state_of(false));
                            let c = checked.clone();
                            let on = checked.get();
                            ElevatedToggleButton(
                                checked.get(),
                                move |it| c.set(it),
                                content = move || {
                                    VectorIcon(
                                        if on { Icons::Filled::Edit() } else { Icons::Outlined::Edit() },
                                        Some("Localized description".into()),
                                        modifier = Modifier.size(ButtonDefaults::IconSize),
                                    );
                                    Spacer(Modifier.size(ButtonDefaults::IconSpacing));
                                    Text("Edit");
                                },
                            );
                        }
                        // XSmall / Medium / Large / XLargeToggleButtonWithIconSample
                        for (name, size) in [
                            ("xs", ButtonDefaults::ExtraSmallContainerHeight),
                            ("m", ButtonDefaults::MediumContainerHeight()),
                            ("l", ButtonDefaults::LargeContainerHeight()),
                            ("xl", ButtonDefaults::ExtraLargeContainerHeight),
                        ] {
                            key(&name, || {
                                let checked = remember(|| mutable_state_of(false));
                                let c = checked.clone();
                                let on = checked.get();
                                let small = name == "xs";
                                ToggleButton(
                                    checked.get(),
                                    move |it| c.set(it),
                                    modifier = Modifier.height_in(size).test_tag(name),
                                    shapes = ToggleButtonDefaults.shapes_for(size),
                                    content_padding = ButtonDefaults.content_padding_for(size),
                                    content = move || {
                                        VectorIcon(
                                            if on { Icons::Filled::Edit() } else { Icons::Outlined::Edit() },
                                            Some(name.into()),
                                            modifier = Modifier.size(ButtonDefaults.icon_size_for(size)),
                                        );
                                        Spacer(Modifier.size(ButtonDefaults.icon_spacing_for(size)));
                                        if small {
                                            Text("Label");
                                        } else {
                                            Text("Label", style = ButtonDefaults.text_style_for(size));
                                        }
                                    },
                                );
                            });
                        }
                    });
                },
            );
        }),
        IntSize::new(600, 1200),
    );
    rule.on_node_with_tag("düz").assert_is_off().perform_click().assert_is_on();
    for text in ["Elevated Button", "Tonal Button", "Outlined Button", "Edit"] {
        rule.on_node_with_text(text).assert_is_off().perform_click();
        rule.on_node_with_text(text).assert_is_on();
    }
    rule.on_node_with_tag("kare").assert_is_off().perform_click().assert_is_on();
    // (the square sample turns round once checked)
    settle(&rule);
    let k = bounds(&rule, "kare");
    assert!(near(look(&rule.draw(), k).unwrap().0, 20.0, 0.01));
    for (tag, height) in [("xs", 32.0), ("m", 56.0), ("l", 96.0), ("xl", 136.0)] {
        assert!(near(bounds(&rule, tag).height(), height, 0.01), "{tag}");
        let before = rule.draw().vectors.clone();
        rule.on_node_with_tag(tag).assert_is_off().perform_click().assert_is_on();
        rule.wait_for_idle();
        assert_ne!(rule.draw().vectors, before, "{tag}: the filled icon once checked");
    }
}
