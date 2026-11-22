//! Material Expressive's split buttons as Compose's material3 (1.5.0-alpha17) has them: the C++ Kompoz's
//! split_button_rounds_its_menu_button_when_checked_and_turns_the_arrow (with M15's values where the C++ had its
//! own), Compose's behaviour (the layout, each kind's colors, the inner corners of each Expressive size, the
//! morphing as pressed and checked, the checked state layer, the paddings and icon sizes) and the 12 samples of M15's
//! SplitButtonSamples.kt.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_animation::animate_float_as_state;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::Text;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::graphics_context::CornerRadii;
use kompoz_ui::input::PointerType;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::semantics::{Role, SemanticsProperties};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

tests!(
    split_button_rounds_its_menu_button_when_checked_and_turns_the_arrow,
    split_button_layout_measures_the_leading_button_first,
    split_button_kinds_take_their_buttons_colors,
    split_button_defaults_follow_the_tokens,
    split_button_shapes_paddings_and_icons_for_each_size,
    split_button_shapes_are_equal_by_their_shapes,
    split_button_trailing_button_marks_itself_checked,
    split_button_samples,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn corners(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> CornerRadii {
    CornerRadii { top_left, top_right, bottom_right, bottom_left }
}

fn same_corners(a: CornerRadii, b: CornerRadii) -> bool {
    near(a.top_left, b.top_left, 0.05)
        && near(a.top_right, b.top_right, 0.05)
        && near(a.bottom_right, b.bottom_right, 0.05)
        && near(a.bottom_left, b.bottom_left, 0.05)
}

/// A node's bounds, in the unmerged tree.
fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
}

/// The fills drawn as `b` (a split button's halves have corners of two sizes: a path), their colors.
fn drawn(canvas: &RecordingCanvas, b: Rect) -> Vec<Color> {
    let at = |r: &Rect| near(r.left, b.left, 0.1) && near(r.top, b.top, 0.1) && near(r.right, b.right, 0.1);
    let rects = canvas.rects.iter().zip(&canvas.rect_colors).filter(|(r, _)| at(r)).map(|(_, c)| *c);
    let paths = (canvas.paths.iter().zip(&canvas.path_colors).zip(&canvas.path_strokes))
        .filter(|((r, _), w)| at(r) && **w <= 0.0)
        .map(|((_, c), _)| *c);
    rects.chain(paths).collect()
}

/// The corners of the button tagged `tag` now: its shape (what it tells accessibility, the animated one) made an
/// outline of its size, laid out `direction`.
fn corners_of(rule: &KompozTestRule, tag: &str, direction: LayoutDirection) -> CornerRadii {
    let node = rule.on_node_with_tag_unmerged(tag).fetch_semantics_node();
    let shape = node.config().get(SemanticsProperties::Shape).expect("a shape");
    let b = bounds(rule, tag);
    match shape.create_outline(Size::new(b.width(), b.height()), direction, &Density::new(1.0, 1.0)) {
        Outline::Rounded(r) => corners(
            r.top_left_corner_radius.x,
            r.top_right_corner_radius.x,
            r.bottom_right_corner_radius.x,
            r.bottom_left_corner_radius.x,
        ),
        Outline::Rectangle(_) => corners(0.0, 0.0, 0.0, 0.0),
        Outline::Generic(_) => corners(-1.0, -1.0, -1.0, -1.0),
    }
}

/// The container's corners (left to right) and color.
fn look(rule: &KompozTestRule, canvas: &RecordingCanvas, tag: &str) -> Option<(CornerRadii, Color)> {
    let b = bounds(rule, tag);
    drawn(canvas, b).first().map(|c| (corners_of(rule, tag, LayoutDirection::Ltr), *c))
}

fn settle(rule: &KompozTestRule) {
    rule.advance_time_by(1000);
    rule.draw();
    rule.advance_time_by(100);
}

/// The samples' trailing icon (M15's `KeyboardArrowDown` turning over as `checked` animates `rotationZ`), its turn
/// told to `turn` as it is drawn.
#[kompozable]
fn TrailingArrow(checked: bool, size: Dp, turn: Rc<Cell<f32>>) {
    let rotation =
        animate_float_as_state(if checked { 180.0 } else { 0.0 }, label = "Trailing Icon Rotation".to_string());
    VectorIcon(
        Icons::Filled::KeyboardArrowDown(),
        Some("Localized description".into()),
        modifier = Modifier.size(size).graphics_layer_block(move |layer| {
            layer.rotation_z = rotation.value();
            turn.set(rotation.value());
        }),
    );
}

fn split_button_rounds_its_menu_button_when_checked_and_turns_the_arrow() {
    // The C++'s test. M15's values: 2 dp apart, the trailing button 48 wide (13 + 22 + 13), the inner corners 4 dp,
    // 12 pressed (the theme's medium corners; the C++ had 8), the checked trailing button a circle; the colors stay
    // (a 10% state layer of the content color over it, Compose's). The C++'s `TrailingButtonIcon` isn't in M15: its
    // samples' arrow (KeyboardArrowDown turned by `rotationZ`) stands in. Compose's semantics bounds are each
    // button's 40 dp container (the C++ measured the 48 dp room).
    let open = mutable_state_of(false);
    let rtl = mutable_state_of(false);
    let saves = Rc::new(Cell::new(0));
    let turn = Rc::new(Cell::new(0.0f32));
    let (o, r, s, t) = (open.clone(), rtl.clone(), saves.clone(), turn.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, s, t) = (o.clone(), s.clone(), t.clone());
        let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], move || {
            let (o, s, t) = (o.clone(), s.clone(), t.clone());
            Column(move || {
                let (o, s, t) = (o.clone(), s.clone(), t.clone());
                SplitButtonLayout(
                    leading_button = move || {
                        let s = s.clone();
                        SplitButtonDefaults.LeadingButton(
                            move || s.set(s.get() + 1),
                            modifier = Modifier.test_tag("kaydet"),
                            content = || Text("Kaydet"),
                        );
                    },
                    trailing_button = move || {
                        let (o2, t) = (o.clone(), t.clone());
                        let checked = o.get();
                        SplitButtonDefaults.ToggleableTrailingButton(
                            checked,
                            move |on| o2.set(on),
                            modifier = Modifier.test_tag("menü"),
                            content = move || TrailingArrow(checked, SplitButtonDefaults::TrailingIconSize, t.clone()),
                        );
                    },
                );
                SplitButtonLayout(
                    leading_button = || {
                        SplitButtonDefaults.TonalLeadingButton(
                            || {},
                            modifier = Modifier.test_tag("ton"),
                            content = || Text("Ton"),
                        );
                    },
                    trailing_button = || SplitButtonDefaults.TonalTrailingButton(false, |_| {}, content = || {}),
                );
            });
        });
    }));
    let cs = light_color_scheme();
    let (lead, trail) = (bounds(&rule, "kaydet"), bounds(&rule, "menü"));
    assert!(near(trail.left - lead.right, 2.0, 0.01) && near(trail.width(), 48.0, 0.01), "{lead:?} {trail:?}");
    assert!(near(trail.height(), lead.height(), 0.01) && near(lead.height(), 40.0, 0.01));
    let canvas = rule.draw();
    let (c, k) = look(&rule, &canvas, "kaydet").expect("the leading container");
    assert!(same_corners(c, corners(20.0, 4.0, 4.0, 20.0)) && k == cs.primary, "{c:?}");
    let (c, k) = look(&rule, &canvas, "menü").expect("the trailing container");
    assert!(same_corners(c, corners(4.0, 20.0, 20.0, 4.0)) && k == cs.primary && turn.get() == 0.0, "{c:?}");
    rule.on_node_with_tag("menü").assert_is_off();
    assert_eq!(look(&rule, &canvas, "ton").unwrap().1, cs.secondary_container);

    rule.on_node_with_tag("menü").perform_click();
    settle(&rule);
    let canvas = rule.draw();
    let (c, k) = look(&rule, &canvas, "menü").unwrap();
    assert!(
        open.get() && same_corners(c, corners(20.0, 20.0, 20.0, 20.0)) && k == cs.primary,
        "round, same colors: {c:?}"
    );
    assert!(near(turn.get(), 180.0, 0.5), "the arrow turned over: {}", turn.get());
    rule.on_node_with_tag("menü").assert_is_on();

    rule.host().pointer_down(lead.center(), -1, 0, PointerType::Touch);
    settle(&rule);
    let (c, _) = look(&rule, &rule.draw(), "kaydet").unwrap();
    assert!(same_corners(c, corners(20.0, 12.0, 12.0, 20.0)), "rounder inside while held: {c:?}"); // (C++: 8)
    rule.host().pointer_up(lead.center(), -1, 0);
    settle(&rule);
    let (c, _) = look(&rule, &rule.draw(), "kaydet").unwrap();
    assert!(saves.get() == 1 && same_corners(c, corners(20.0, 4.0, 4.0, 20.0)), "{c:?}");

    rule.on_node_with_tag("menü").perform_click();
    rtl.set(true);
    settle(&rule);
    let (lead, trail) = (bounds(&rule, "kaydet"), bounds(&rule, "menü"));
    let c = corners_of(&rule, "kaydet", LayoutDirection::Rtl); // (its start on the right)
    assert!(!open.get() && same_corners(c, corners(4.0, 20.0, 20.0, 4.0)) && trail.right < lead.left, "{c:?}");
}

fn split_button_layout_measures_the_leading_button_first() {
    // Compose's measure policy: the leading button unconstrained below, the trailing one what is left of the width
    // and exactly the leading one's height, `spacing` apart; the layout keeps 48 dp of room to touch (its buttons
    // none of their own).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            SplitButtonLayout(
                leading_button = || Box(modifier = Modifier.size_wh(60.dp(), 30.dp()).test_tag("a")),
                trailing_button = || Box(modifier = Modifier.width(20.dp()).fill_max_height().test_tag("b")),
                modifier = Modifier.test_tag("düzen"),
                spacing = 6.dp(),
            );
            Box(
                modifier = Modifier.width(100.dp()),
                content = || {
                    SplitButtonLayout(
                        leading_button = || Box(modifier = Modifier.size_wh(70.dp(), 50.dp()).test_tag("c")),
                        trailing_button = || Box(modifier = Modifier.fill_max_width().fill_max_height().test_tag("d")),
                        modifier = Modifier.test_tag("dar"),
                    );
                },
            );
            SplitButtonLayout(
                leading_button = || {
                    SplitButtonDefaults.LeadingButton(
                        || {},
                        modifier = Modifier.test_tag("ön"),
                        content = || Box(modifier = Modifier.size(10.dp())),
                    )
                },
                trailing_button = || {
                    SplitButtonDefaults.TrailingButton(
                        || {},
                        modifier = Modifier.test_tag("arka"),
                        content = || Box(modifier = Modifier.size(10.dp())),
                    )
                },
                modifier = Modifier.test_tag("düğmeler"),
            );
        });
    }));
    let (a, b, l) = (bounds(&rule, "a"), bounds(&rule, "b"), bounds(&rule, "düzen"));
    assert!(near(b.left - a.right, 6.0, 0.01) && near(b.height(), 30.0, 0.01) && near(b.width(), 20.0, 0.01));
    // (86 wide, 48 of room to touch: the pair centered in it)
    assert!(near(l.width(), 86.0, 0.01) && near(l.height(), 48.0, 0.01), "{l:?}");
    assert!(near(a.top - l.top, 9.0, 0.01), "{a:?} {l:?}");
    // (squeezed: what is left after 70 + 2)
    let (c, d) = (bounds(&rule, "c"), bounds(&rule, "d"));
    assert!(near(d.width(), 28.0, 0.01) && near(d.height(), 50.0, 0.01) && near(d.left - c.right, 2.0, 0.01));
    // The buttons: the leading one at least 48 wide (16 + 10 + 12 = 38 otherwise), the trailing one 48 (13 + 10 +
    // 13 = 36 otherwise), both 40 tall.
    let (front, back) = (bounds(&rule, "ön"), bounds(&rule, "arka"));
    assert!(near(front.width(), 48.0, 0.01) && near(back.width(), 48.0, 0.01), "{front:?} {back:?}");
    assert!(near(front.height(), 40.0, 0.01) && near(back.height(), 40.0, 0.01));
    rule.on_node_with_tag("ön").fetch_semantics_node();
    for tag in ["ön", "arka"] {
        let node = rule.on_node_with_tag(tag).fetch_semantics_node();
        assert_eq!(node.config().get(SemanticsProperties::Role), Some(Role::Button), "{tag}");
    }
}

fn split_button_kinds_take_their_buttons_colors() {
    // Each kind's buttons in the button's colors (filled, tonal, outlined with its border, elevated with its
    // shadow); disabled, the disabled ones.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            for (i, enabled) in [true, false].into_iter().enumerate() {
                let t = move |k: &str| format!("{k}{i}");
                SplitButtonLayout(
                    leading_button = move || {
                        SplitButtonDefaults.LeadingButton(
                            || {},
                            modifier = Modifier.test_tag(t("dolu")),
                            enabled = enabled,
                            content = || Text("A"),
                        )
                    },
                    trailing_button = move || {
                        SplitButtonDefaults.ToggleableTrailingButton(
                            false,
                            |_| {},
                            modifier = Modifier.test_tag(t("dolu-")),
                            enabled = enabled,
                            content = || Text("v"),
                        )
                    },
                );
                SplitButtonLayout(
                    leading_button = move || {
                        SplitButtonDefaults.TonalLeadingButton(
                            || {},
                            modifier = Modifier.test_tag(t("ton")),
                            enabled = enabled,
                            content = || Text("A"),
                        )
                    },
                    trailing_button = move || {
                        SplitButtonDefaults.TonalTrailingButton(
                            false,
                            |_| {},
                            modifier = Modifier.test_tag(t("ton-")),
                            enabled = enabled,
                            content = || Text("v"),
                        )
                    },
                );
                SplitButtonLayout(
                    leading_button = move || {
                        SplitButtonDefaults.OutlinedLeadingButton(
                            || {},
                            modifier = Modifier.test_tag(t("çizgili")),
                            enabled = enabled,
                            content = || Text("A"),
                        )
                    },
                    trailing_button = move || {
                        SplitButtonDefaults.OutlinedTrailingButton(
                            false,
                            |_| {},
                            modifier = Modifier.test_tag(t("çizgili-")),
                            enabled = enabled,
                            content = || Text("v"),
                        )
                    },
                );
                SplitButtonLayout(
                    leading_button = move || {
                        SplitButtonDefaults.ElevatedLeadingButton(
                            || {},
                            modifier = Modifier.test_tag(t("kabarık")),
                            enabled = enabled,
                            content = || Text("A"),
                        )
                    },
                    trailing_button = move || {
                        SplitButtonDefaults.ElevatedTrailingButton(
                            false,
                            |_| {},
                            modifier = Modifier.test_tag(t("kabarık-")),
                            enabled = enabled,
                            content = || Text("v"),
                        )
                    },
                );
            }
        });
    }));
    let cs = light_color_scheme();
    let canvas = rule.draw();
    let color = |tag: &str| drawn(&canvas, bounds(&rule, tag)).first().copied();
    for (k, want) in [("dolu", cs.primary), ("ton", cs.secondary_container), ("kabarık", cs.surface_container_low)] {
        assert_eq!(color(&format!("{k}0")), Some(want), "{k}");
        assert_eq!(color(&format!("{k}-0")), Some(want), "{k} trailing");
        // (the kinds' button tokens: the filled and elevated ones' 10%, the tonal one's 12%)
        let alpha = if k == "ton" { 0.12 } else { 0.1 };
        assert_eq!(color(&format!("{k}1")), Some(named_args!(cs.on_surface.copy(alpha = alpha))), "{k} disabled");
    }
    // (the outlined ones: transparent, outlined — the border's ring filled over it, corners of two sizes making
    // both paths —; a tenth of the line disabled)
    let line = |tag: &str| {
        let fills = drawn(&canvas, bounds(&rule, tag));
        assert_eq!(fills.first(), Some(&Color::Transparent), "{tag}");
        fills.get(1).copied()
    };
    assert_eq!(line("çizgili0"), Some(cs.outline_variant));
    assert_eq!(line("çizgili-0"), Some(cs.outline_variant));
    assert_eq!(line("çizgili1"), Some(named_args!(cs.outline_variant.copy(alpha = 0.1))));
    // (shadows: the elevated ones 1 dp, the others none)
    let shadow = |tag: &str| {
        let b = bounds(&rule, tag);
        canvas
            .elevation_shadows
            .iter()
            .filter(|e| near(e.bounds.center().x, b.center().x, 2.0) && near(e.bounds.center().y, b.center().y, 2.0))
            .map(|e| e.elevation())
            .fold(0.0, f32::max)
    };
    assert!(near(shadow("kabarık0"), 1.0, 0.01) && near(shadow("kabarık-0"), 1.0, 0.01));
    assert!(near(shadow("dolu0"), 0.0, 0.01) && near(shadow("ton0"), 0.0, 0.01) && near(shadow("çizgili0"), 0.0, 0.01));
}

fn split_button_defaults_follow_the_tokens() {
    assert_eq!(SplitButtonDefaults::LeadingIconSize, Dp(20.0));
    assert_eq!(SplitButtonDefaults::TrailingIconSize, Dp(22.0));
    assert_eq!(SplitButtonDefaults::Spacing, Dp(2.0));
    let dp = |c: CornerSize| c.to_px(Size::new(100.0, 100.0), &Density::new(1.0, 1.0));
    assert_eq!(
        [
            SplitButtonDefaults::ExtraSmallInnerCornerSize,
            SplitButtonDefaults::SmallInnerCornerSize,
            SplitButtonDefaults::MediumInnerCornerSize,
            SplitButtonDefaults::LargeInnerCornerSize,
            SplitButtonDefaults::ExtraLargeInnerCornerSize,
        ]
        .map(dp),
        [4.0, 4.0, 4.0, 8.0, 12.0]
    );
    assert_eq!(
        [
            SplitButtonDefaults::ExtraSmallInnerCornerSizePressed,
            SplitButtonDefaults::SmallInnerCornerSizePressed,
            SplitButtonDefaults::MediumInnerCornerSizePressed,
            SplitButtonDefaults::LargeInnerCornerSizePressed,
            SplitButtonDefaults::ExtraLargeInnerCornerSizePressed,
        ]
        .map(dp),
        [8.0, 12.0, 12.0, 20.0, 20.0]
    );
    // (full: 100% of the smaller side, clamped to half of it when an outline is made)
    assert_eq!(dp(SplitButtonDefaults::OuterCornerSize), 100.0);
    assert_eq!(
        [
            SplitButtonDefaults::ExtraSmallContainerHeight,
            SplitButtonDefaults::SmallContainerHeight,
            SplitButtonDefaults::MediumContainerHeight,
            SplitButtonDefaults::LargeContainerHeight,
            SplitButtonDefaults::ExtraLargeContainerHeight,
        ],
        [Dp(32.0), Dp(40.0), Dp(56.0), Dp(96.0), Dp(136.0)]
    );
    assert_eq!(
        [
            SplitButtonDefaults::ExtraSmallTrailingButtonIconSize,
            SplitButtonDefaults::SmallTrailingButtonIconSize,
            SplitButtonDefaults::MediumTrailingButtonIconSize,
            SplitButtonDefaults::LargeTrailingButtonIconSize,
            SplitButtonDefaults::ExtraLargeTrailingButtonIconSize,
        ],
        [Dp(22.0), Dp(22.0), Dp(26.0), Dp(38.0), Dp(50.0)]
    );
    let sides = |p: PaddingValues| {
        (p.calculate_start_padding(LayoutDirection::Ltr).0, p.calculate_end_padding(LayoutDirection::Ltr).0)
    };
    assert_eq!(
        [
            SplitButtonDefaults::ExtraSmallLeadingButtonContentPadding(),
            SplitButtonDefaults::SmallLeadingButtonContentPadding(),
            SplitButtonDefaults::MediumLeadingButtonContentPadding(),
            SplitButtonDefaults::LargeLeadingButtonContentPadding(),
            SplitButtonDefaults::ExtraLargeLeadingButtonContentPadding(),
        ]
        .map(sides),
        [(12.0, 10.0), (16.0, 12.0), (24.0, 24.0), (48.0, 48.0), (64.0, 64.0)]
    );
    assert_eq!(
        [
            SplitButtonDefaults::ExtraSmallTrailingButtonContentPadding(),
            SplitButtonDefaults::SmallTrailingButtonContentPadding(),
            SplitButtonDefaults::MediumTrailingButtonContentPadding(),
            SplitButtonDefaults::LargeTrailingButtonContentPadding(),
            SplitButtonDefaults::ExtraLargeTrailingButtonContentPadding(),
        ]
        .map(sides),
        [(13.0, 13.0), (13.0, 13.0), (15.0, 15.0), (29.0, 29.0), (43.0, 43.0)]
    );
}

fn split_button_shapes_paddings_and_icons_for_each_size() {
    // Compose's *For(buttonHeight): the leading button's outer corners full, the inner (and pressed) ones of the
    // nearest size; the trailing one's mirrored, a circle checked; paddings and icon sizes of the size it is at
    // least.
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let mut s = s.borrow_mut();
        for h in [32.0, 40.0, 56.0, 96.0, 136.0] {
            s.push((
                SplitButtonDefaults.leading_button_shapes_for(Dp(h)),
                SplitButtonDefaults.trailing_button_shapes_for(Dp(h)),
            ));
        }
    }));
    rule.wait_for_idle();
    let outline = |shape: &Arc<dyn Shape>| match shape.create_outline(
        Size::new(200.0, 100.0),
        LayoutDirection::Ltr,
        &Density::new(1.0, 1.0),
    ) {
        Outline::Rounded(r) => (r.top_left_corner_radius.x, r.top_right_corner_radius.x),
        _ => (-1.0, -1.0),
    };
    let inner = [(4.0, 8.0), (4.0, 12.0), (4.0, 12.0), (8.0, 20.0), (12.0, 20.0)];
    for ((leading, trailing), (rest, pressed)) in seen.borrow().iter().zip(inner) {
        assert_eq!(outline(&leading.shape), (50.0, rest));
        assert_eq!(outline(leading.pressed_shape.as_ref().unwrap()), (50.0, pressed));
        assert!(leading.checked_shape.is_none());
        assert_eq!(outline(&trailing.shape), (rest, 50.0));
        assert_eq!(outline(trailing.pressed_shape.as_ref().unwrap()), (pressed, 50.0));
        assert!(**trailing.checked_shape.as_ref().unwrap() == *(&CircleShape as &dyn Shape));
    }
    let start = |p: PaddingValues| p.calculate_start_padding(LayoutDirection::Ltr).0;
    let heights = [30.0, 32.0, 40.0, 55.0, 56.0, 95.0, 96.0, 135.0, 136.0];
    assert_eq!(
        heights.map(|h| start(SplitButtonDefaults.leading_button_content_padding_for(Dp(h)))),
        [12.0, 12.0, 16.0, 16.0, 24.0, 24.0, 48.0, 48.0, 64.0]
    );
    assert_eq!(
        heights.map(|h| start(SplitButtonDefaults.trailing_button_content_padding_for(Dp(h)))),
        [13.0, 13.0, 13.0, 13.0, 15.0, 15.0, 29.0, 29.0, 43.0]
    );
    assert_eq!(
        heights.map(|h| SplitButtonDefaults.trailing_button_icon_size_for(Dp(h)).0),
        [22.0, 22.0, 22.0, 22.0, 26.0, 26.0, 38.0, 38.0, 50.0]
    );
    // (the leading icon: a button's of that height)
    for h in heights {
        assert_eq!(SplitButtonDefaults.leading_button_icon_size_for(Dp(h)), ButtonDefaults.icon_size_for(Dp(h)));
    }
}

fn split_button_shapes_are_equal_by_their_shapes() {
    let a = SplitButtonShapes::new(Arc::new(CircleShape), Some(Arc::new(RoundedCornerShape(4.dp()))), None);
    let b = SplitButtonShapes::new(Arc::new(CircleShape), Some(Arc::new(RoundedCornerShape(4.dp()))), None);
    let c = SplitButtonShapes::new(Arc::new(CircleShape), None, None);
    let d = SplitButtonShapes::new(
        Arc::new(CircleShape),
        Some(Arc::new(RoundedCornerShape(4.dp()))),
        Some(Arc::new(RectangleShape)),
    );
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_ne!(a, d);
    use std::hash::{BuildHasher, RandomState};
    let hasher = RandomState::new();
    assert_eq!(hasher.hash_one(&a), hasher.hash_one(&b));
}

fn split_button_trailing_button_marks_itself_checked() {
    // While checked, the toggleable trailing button draws its content color at 10% over itself (Compose's
    // `drawWithContent` state layer), a circle; its shapes without a checked one stay as they are; shapes not
    // corner based switch at once. (Inside a SplitButtonLayout, as Compose means them: outside one the state layer
    // is drawn on the 48 dp room around the button, and an animated shape outlined in two sizes — the room's when
    // drawn, the button's when clipped — writes its clamped range over and over, Compose's as well.)
    let on = mutable_state_of(false);
    let o = on.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let o = o.clone();
        Column(move || {
            let (o, o2) = (o.clone(), o.clone());
            let checked = o.get();
            SplitButtonLayout(
                leading_button = || SplitButtonDefaults.LeadingButton(|| {}, content = || Text("a")),
                trailing_button = move || {
                    let o2 = o2.clone();
                    SplitButtonDefaults.ToggleableTrailingButton(
                        checked,
                        move |c| o2.set(c),
                        modifier = Modifier.test_tag("t"),
                        content = || Text("v"),
                    )
                },
            );
            SplitButtonLayout(
                leading_button = || SplitButtonDefaults.LeadingButton(|| {}, content = || Text("b")),
                trailing_button = move || {
                    SplitButtonDefaults.ToggleableTrailingButton(
                        checked,
                        |_| {},
                        modifier = Modifier.test_tag("u"),
                        shapes = SplitButtonShapes::new(Arc::new(RoundedCornerShape(4.dp())), None, None),
                        content = || Text("w"),
                    )
                },
            );
            SplitButtonLayout(
                leading_button = || SplitButtonDefaults.LeadingButton(|| {}, content = || Text("c")),
                trailing_button = move || {
                    SplitButtonDefaults.ToggleableTrailingButton(
                        checked,
                        |_| {},
                        modifier = Modifier.test_tag("r"),
                        shapes = SplitButtonShapes::new(
                            Arc::new(RectangleShape),
                            None,
                            Some(Arc::new(CutCornerShape(6.dp()))),
                        ),
                        content = || Text("x"),
                    )
                },
            );
        });
    }));
    let cs = light_color_scheme();
    let (t, r) = (bounds(&rule, "t"), bounds(&rule, "r"));
    assert_eq!(drawn(&rule.draw(), t), vec![cs.primary]);
    assert_eq!(corners_of(&rule, "r", LayoutDirection::Ltr), corners(0.0, 0.0, 0.0, 0.0));
    rule.on_node_with_tag("t").perform_click();
    settle(&rule);
    let canvas = rule.draw();
    let layers = drawn(&canvas, t);
    assert!(on.get() && layers.len() == 2 && layers[0] == cs.primary, "{layers:?}");
    // (the layer: the content color at 10%)
    let layer = layers[1];
    assert!(near(layer.red(), cs.on_primary.red(), 0.01) && near(layer.alpha(), 0.1, 0.01), "{layer:?}");
    assert!(same_corners(corners_of(&rule, "t", LayoutDirection::Ltr), corners(20.0, 20.0, 20.0, 20.0)));
    let u = corners_of(&rule, "u", LayoutDirection::Ltr);
    assert!(same_corners(u, corners(4.0, 4.0, 4.0, 4.0)), "no checked shape: its own: {u:?}");
    // (a rectangle unchecked; checked, the cut corners at once: a path)
    assert!(
        rule.on_node_with_tag_unmerged("r")
            .fetch_semantics_node()
            .config()
            .get(SemanticsProperties::Shape)
            .is_some_and(|s| *s == *(&CutCornerShape(6.dp()) as &dyn Shape))
    );
    assert!(canvas.paths.iter().any(|p| near(p.left, r.left, 0.1) && near(p.right, r.right, 0.1)));
}

fn split_button_samples() {
    // M15's SplitButtonSamples.kt (12), as written (their TooltipBoxes and the DropdownMenu are on main): each
    // trailing button toggles, says Expanded/Collapsed and turns its arrow; the menu sample's menu opens with it;
    // the sized ones as tall as their size.
    let turn = Rc::new(Cell::new(0.0f32));
    let t = turn.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let t = t.clone();
            MaterialTheme(
                content = move || {
                    let t = t.clone();
                    Column(
                        modifier = Modifier.vertical_scroll(&remember_scroll_state()),
                        content = move || {
                            let t = t.clone();
                            // FilledSplitButtonSample, TonalSplitButtonSample, ElevatedSplitButtonSample,
                            // OutlinedSplitButtonSample, SplitButtonWithTextSample, SplitButtonWithIconSample
                            for kind in ["filled", "tonal", "elevated", "outlined", "text", "icon"] {
                                let t = t.clone();
                                key(&kind, move || CheckableSample(kind, t.clone()));
                            }
                            // SplitButtonWithUnCheckableTrailingButtonSample
                            SplitButtonLayout(
                                leading_button = || {
                                    SplitButtonDefaults.LeadingButton(
                                        on_click = || { /* Do Nothing */ },
                                        content = || {
                                            VectorIcon(
                                                Icons::Filled::Edit(),
                                                modifier = Modifier.size(SplitButtonDefaults::LeadingIconSize),
                                                content_description = Some("Localized description".into()),
                                            );
                                            Spacer(Modifier.size(ButtonDefaults::IconSpacing));
                                            Text("My Button");
                                        },
                                    );
                                },
                                trailing_button = || {
                                    let description = "Toggle Button";
                                    // Icon-only trailing button should have a tooltip for a11y.
                                    TooltipBox(
                                        position_provider = TooltipDefaults
                                            .remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                                        tooltip = move |scope: &dyn TooltipScope| {
                                            scope.PlainTooltip(content = move || Text(description))
                                        },
                                        state = remember_tooltip_state(),
                                        content = move || {
                                            SplitButtonDefaults.TrailingButton(
                                                on_click = || { /* Do Nothing */ },
                                                modifier = Modifier.test_tag("işaretsiz"),
                                                content = move || {
                                                    VectorIcon(
                                                        Icons::Filled::KeyboardArrowDown(),
                                                        modifier = Modifier.size(SplitButtonDefaults::TrailingIconSize),
                                                        content_description = Some(description.into()),
                                                    );
                                                },
                                            );
                                        },
                                    );
                                },
                            );
                            // SplitButtonWithDropdownMenuSample
                            MenuSample(t.clone());
                            // XSmall / Medium / Large / ExtraLargeFilledSplitButtonSample
                            for (name, size) in [
                                ("xs", SplitButtonDefaults::ExtraSmallContainerHeight),
                                ("m", SplitButtonDefaults::MediumContainerHeight),
                                ("l", SplitButtonDefaults::LargeContainerHeight),
                                ("xl", SplitButtonDefaults::ExtraLargeContainerHeight),
                            ] {
                                let t = t.clone();
                                key(&name, move || SizedSample(name, size, t.clone()));
                            }
                        },
                    );
                },
            );
        }),
        IntSize::new(600, 2000),
    );
    for kind in ["filled", "tonal", "elevated", "outlined", "text", "icon"] {
        rule.on_node_with_tag(kind).assert_is_off();
        rule.on_node(has_state_description("Collapsed").and(has_test_tag(kind))).assert_exists();
        rule.on_node_with_tag(kind).perform_click().assert_is_on();
        rule.on_node(has_state_description("Expanded").and(has_test_tag(kind))).assert_exists();
        settle(&rule);
        assert!(near(turn.get(), 180.0, 0.5), "{kind}: {}", turn.get());
        rule.on_node_with_tag(kind).perform_click().assert_is_off();
        settle(&rule);
    }
    rule.on_node_with_text("Ton").assert_exists();
    // (the un-checkable trailing button: a button, no toggle state)
    let node = rule.on_node_with_tag("işaretsiz").fetch_semantics_node();
    assert_eq!(node.config().get(SemanticsProperties::Role), Some(Role::Button));
    assert!(node.config().get(SemanticsProperties::ToggleableState).is_none());
    // (the menu opens with the trailing button and its items are there)
    rule.on_node_with_text("Edit").assert_does_not_exist();
    rule.on_node_with_tag("menülü").perform_click();
    rule.wait_for_idle();
    for item in ["Edit", "Settings", "Send Feedback", "F11"] {
        rule.on_node_with_text(item).assert_exists();
    }
    rule.on_node_with_tag("menülü").assert_is_on();
    for (tag, height) in [("xs", 32.0), ("m", 56.0), ("l", 96.0), ("xl", 136.0)] {
        let lead = bounds(&rule, &format!("{tag}-ön"));
        let trail = bounds(&rule, tag);
        assert!(near(lead.height(), height, 0.01) && near(trail.height(), height, 0.01), "{tag}: {lead:?} {trail:?}");
        rule.on_node_with_tag(tag).assert_is_off();
    }
}

/// The samples whose trailing button toggles (Filled, Tonal, Elevated, Outlined, WithText, WithIcon), as M15
/// writes them, `kind` picking which.
#[kompozable]
fn CheckableSample(kind: &'static str, turn: Rc<Cell<f32>>) {
    let checked = remember(|| mutable_state_of(false));

    let leading_icon = move || {
        VectorIcon(
            Icons::Filled::Edit(),
            modifier = Modifier.size(SplitButtonDefaults::LeadingIconSize),
            content_description = Some("Localized description".into()),
        );
        Spacer(Modifier.size(ButtonDefaults::IconSpacing));
        Text(if kind == "tonal" { "Ton" } else { "My Button" });
    };
    let (c, c2) = (checked.clone(), checked.clone());
    SplitButtonLayout(
        leading_button = move || match kind {
            "tonal" => {
                SplitButtonDefaults.TonalLeadingButton(on_click = || { /* Do Nothing */ }, content = leading_icon)
            }
            "elevated" => {
                SplitButtonDefaults.ElevatedLeadingButton(on_click = || { /* Do Nothing */ }, content = leading_icon)
            }
            "outlined" => {
                SplitButtonDefaults.OutlinedLeadingButton(on_click = || { /* Do Nothing */ }, content = leading_icon)
            }
            "text" => {
                SplitButtonDefaults.LeadingButton(on_click = || { /* Do Nothing */ }, content = || Text("My Button"))
            }
            "icon" => {
                let description = "Button";
                // Icon-only leading button should have a tooltip for a11y.
                TooltipBox(
                    position_provider =
                        TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                    tooltip = move |scope: &dyn TooltipScope| scope.PlainTooltip(content = move || Text(description)),
                    state = remember_tooltip_state(),
                    content = move || {
                        SplitButtonDefaults.LeadingButton(
                            on_click = || { /* Do Nothing */ },
                            content = move || {
                                VectorIcon(
                                    Icons::Filled::Edit(),
                                    Some(description.into()),
                                    Modifier.size(SplitButtonDefaults::LeadingIconSize),
                                );
                            },
                        );
                    },
                );
            }
            _ => SplitButtonDefaults.LeadingButton(on_click = || { /* Do Nothing */ }, content = leading_icon),
        },
        trailing_button = move || {
            let description = "Toggle Button";
            let (c, c2, turn) = (c.clone(), c2.clone(), turn.clone());
            // Icon-only trailing button should have a tooltip for a11y.
            TooltipBox(
                position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                tooltip = move |scope: &dyn TooltipScope| scope.PlainTooltip(content = move || Text(description)),
                state = remember_tooltip_state(),
                content = move || {
                    let checked = c.get();
                    let set = c2.clone();
                    let on_checked_change = move |it: bool| set.set(it);
                    let modifier = Modifier.test_tag(kind).semantics(move |s| {
                        s.set_state_description(if checked { "Expanded" } else { "Collapsed" });
                        s.set_content_description(description);
                    });
                    let turn = turn.clone();
                    let content = move || TrailingArrow(checked, SplitButtonDefaults::TrailingIconSize, turn.clone());
                    match kind {
                        "tonal" => SplitButtonDefaults.TonalTrailingButton(
                            checked = checked,
                            on_checked_change = on_checked_change,
                            modifier = modifier,
                            content = content,
                        ),
                        "elevated" => SplitButtonDefaults.ElevatedTrailingButton(
                            checked = checked,
                            on_checked_change = on_checked_change,
                            modifier = modifier,
                            content = content,
                        ),
                        "outlined" => SplitButtonDefaults.OutlinedTrailingButton(
                            checked = checked,
                            on_checked_change = on_checked_change,
                            modifier = modifier,
                            content = content,
                        ),
                        _ => SplitButtonDefaults.ToggleableTrailingButton(
                            checked = checked,
                            on_checked_change = on_checked_change,
                            modifier = modifier,
                            content = content,
                        ),
                    }
                },
            );
        },
    );
}

/// M15's SplitButtonWithDropdownMenuSample.
#[kompozable]
fn MenuSample(turn: Rc<Cell<f32>>) {
    let checked = remember(|| mutable_state_of(false));

    let (c, c2, c3) = (checked.clone(), checked.clone(), checked.clone());
    Box(
        modifier = Modifier.fill_max_width().wrap_content_size(),
        content = move || {
            let (c, turn) = (c.clone(), turn.clone());
            SplitButtonLayout(
                leading_button = || {
                    SplitButtonDefaults.LeadingButton(
                        on_click = || { /* Do Nothing */ },
                        content = || {
                            VectorIcon(
                                Icons::Filled::Edit(),
                                modifier = Modifier.size(SplitButtonDefaults::LeadingIconSize),
                                content_description = Some("Localized description".into()),
                            );
                            Spacer(Modifier.size(ButtonDefaults::IconSpacing));
                            Text("My Button");
                        },
                    );
                },
                trailing_button = move || {
                    let description = "Toggle Button";
                    let (c, turn) = (c.clone(), turn.clone());
                    // Icon-only trailing button should have a tooltip for a11y.
                    TooltipBox(
                        position_provider =
                            TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                        tooltip =
                            move |scope: &dyn TooltipScope| scope.PlainTooltip(content = move || Text(description)),
                        state = remember_tooltip_state(),
                        content = move || {
                            let checked = c.get();
                            let (set, turn) = (c.clone(), turn.clone());
                            SplitButtonDefaults.ToggleableTrailingButton(
                                checked = checked,
                                on_checked_change = move |it| set.set(it),
                                modifier = Modifier.test_tag("menülü").semantics(move |s| {
                                    s.set_state_description(if checked { "Expanded" } else { "Collapsed" });
                                    s.set_content_description(description);
                                }),
                                content =
                                    move || TrailingArrow(checked, SplitButtonDefaults::TrailingIconSize, turn.clone()),
                            );
                        },
                    );
                },
            );

            let c3 = c3.clone();
            DropdownMenu(
                expanded = c2.get(),
                on_dismiss_request = move || c3.set(false),
                content = || {
                    DropdownMenuItem(
                        text = || Text("Edit"),
                        on_click = || { /* Handle edit! */ },
                        leading_icon =
                            Some(Rc::new(|| VectorIcon(Icons::Outlined::Edit(), content_description = None))),
                    );
                    DropdownMenuItem(
                        text = || Text("Settings"),
                        on_click = || { /* Handle settings! */ },
                        leading_icon =
                            Some(Rc::new(|| VectorIcon(Icons::Outlined::Settings(), content_description = None))),
                    );
                    HorizontalDivider();
                    DropdownMenuItem(
                        text = || Text("Send Feedback"),
                        on_click = || { /* Handle send feedback! */ },
                        leading_icon =
                            Some(Rc::new(|| VectorIcon(Icons::Outlined::Email(), content_description = None))),
                        trailing_icon = Some(Rc::new(|| Text("F11", text_align = kompoz_text::TextAlign::Center))),
                    );
                },
            );
        },
    );
}

/// M15's XSmall/Medium/Large/ExtraLargeFilledSplitButtonSample (`name` tags them).
#[kompozable]
fn SizedSample(name: &'static str, size: Dp, turn: Rc<Cell<f32>>) {
    let checked = remember(|| mutable_state_of(false));

    let c = checked.clone();
    SplitButtonLayout(
        leading_button = move || {
            SplitButtonDefaults.LeadingButton(
                on_click = || { /* Do Nothing */ },
                modifier = Modifier.height_in(size).test_tag(format!("{name}-ön")),
                shapes = SplitButtonDefaults.leading_button_shapes_for(size),
                content_padding = SplitButtonDefaults.leading_button_content_padding_for(size),
                content = move || {
                    VectorIcon(
                        Icons::Filled::Edit(),
                        modifier = Modifier.size(SplitButtonDefaults.leading_button_icon_size_for(size)),
                        content_description = Some("Localized description".into()),
                    );
                    Spacer(Modifier.size(ButtonDefaults.icon_spacing_for(size)));
                    Text("My Button", style = ButtonDefaults.text_style_for(size));
                },
            );
        },
        trailing_button = move || {
            let description = "Toggle Button";
            let (c, turn) = (c.clone(), turn.clone());
            // Icon-only trailing button should have a tooltip for a11y.
            TooltipBox(
                position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                tooltip = move |scope: &dyn TooltipScope| scope.PlainTooltip(content = move || Text(description)),
                state = remember_tooltip_state(),
                content = move || {
                    let checked = c.get();
                    let (set, turn) = (c.clone(), turn.clone());
                    SplitButtonDefaults.ToggleableTrailingButton(
                        checked = checked,
                        on_checked_change = move |it| set.set(it),
                        modifier = Modifier.height_in(size).test_tag(name).semantics(move |s| {
                            s.set_state_description(if checked { "Expanded" } else { "Collapsed" });
                            s.set_content_description(description);
                        }),
                        shapes = SplitButtonDefaults.trailing_button_shapes_for(size),
                        content_padding = SplitButtonDefaults.trailing_button_content_padding_for(size),
                        content = move || {
                            TrailingArrow(
                                checked,
                                SplitButtonDefaults.trailing_button_icon_size_for(size),
                                turn.clone(),
                            )
                        },
                    );
                },
            );
        },
    );
}
