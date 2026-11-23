//! Material Expressive's button groups as Compose's material3 (1.5.0-alpha17) has them: the C++ Kompoz's
//! button_group_grows_the_held_button_and_squeezes_its_neighbours, button_group_puts_what_does_not_fit_into_a_menu
//! and connected_button_group_rounds_the_outer_ends_and_the_checked_one (with M15's values where the C++ had its own,
//! said at each; layout_is_in_whole_pixels_as_composes, its ButtonGroup part with it, is whole in cross_area.rs;
//! expressive_buttons_take_compose_named_arguments is in both syntaxes in crates/kompoz/tests/named_forms.rs),
//! Compose's behaviour (weights, alignment, the defaults, the overflow indicator, the menu state) and the 6 samples of
//! M15's ButtonGroupSamples.kt. Density 1 unless said: dp are px.

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_extended::Icons;
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
    button_group_grows_the_held_button_and_squeezes_its_neighbours,
    button_group_puts_what_does_not_fit_into_a_menu,
    connected_button_group_rounds_the_outer_ends_and_the_checked_one,
    button_group_weights_share_what_is_left,
    button_group_items_align_vertically,
    button_group_defaults_follow_the_tokens,
    button_group_menu_state_shows_and_dismisses,
    button_group_overflow_indicator_opens_the_menu_and_tells_its_name,
    button_group_items_take_weights_and_icons,
    button_group_samples,
);

/// Waits for idle with the clock advancing on its own (Compose's `mainClock.autoAdvance = true; waitForIdle()`):
/// what a test stepping the clock by hand does to let the animations run their course; its `auto_advance` put
/// back after.
fn finish_animations(rule: &KompozTestRule) {
    let clock = rule.main_clock();
    let was = clock.auto_advance();
    clock.set_auto_advance(true);
    rule.wait_for_idle();
    clock.set_auto_advance(was);
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn same_rect(a: Rect, b: Rect) -> bool {
    near(a.left, b.left, 0.01)
        && near(a.top, b.top, 0.01)
        && near(a.right, b.right, 0.01)
        && near(a.bottom, b.bottom, 0.01)
}

/// A node's bounds, in the unmerged tree.
fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
}

fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    named_args!(rule.on_node_with_text(text, use_unmerged_tree = true)).get_unclipped_bounds_in_root()
}

fn press(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
}

fn release(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_up(at, -1, 0);
}

fn tap(rule: &KompozTestRule, at: Offset) {
    press(rule, at);
    release(rule, at);
    rule.wait_for_idle();
}

/// Whether `text` was drawn.
fn drawn(rule: &KompozTestRule, text: &str) -> bool {
    rule.drawn_texts().iter().any(|t| t == text)
}

/// Whether a node of `text` is on the screen (placed, of a size).
fn shown(rule: &KompozTestRule, text: &str) -> bool {
    let all = named_args!(rule.on_all_nodes_with_text(text, use_unmerged_tree = true)).fetch_semantics_nodes();
    all.iter().any(|n| n.bounds_in_root().width() > 0.0)
}

/// The bounds of the shown node of `text` (the last one: the menu's, over the row's).
fn shown_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    let all = named_args!(rule.on_all_nodes_with_text(text, use_unmerged_tree = true)).fetch_semantics_nodes();
    all.iter().map(|n| n.bounds_in_root()).rfind(|b| b.width() > 0.0).expect("shown")
}

/// The corners of the shape a node tells (its container's, as drawn: a connected button's corners of two sizes are
/// a path), left to right.
fn corners_of(rule: &KompozTestRule, tag: &str) -> CornerRadii {
    let node = rule.on_node_with_tag_unmerged(tag).fetch_semantics_node();
    let shape = node.config().get(SemanticsProperties::Shape).expect("a shape");
    let b = bounds(rule, tag);
    match shape.create_outline(Size::new(b.width(), b.height()), LayoutDirection::Ltr, &Density::new(1.0, 1.0)) {
        Outline::Rounded(r) => CornerRadii {
            top_left: r.top_left_corner_radius.x,
            top_right: r.top_right_corner_radius.x,
            bottom_right: r.bottom_right_corner_radius.x,
            bottom_left: r.bottom_left_corner_radius.x,
        },
        other => panic!("{other:?}"),
    }
}

fn same_corners(a: CornerRadii, b: [f32; 4]) -> bool {
    near(a.top_left, b[0], 0.05)
        && near(a.top_right, b[1], 0.05)
        && near(a.bottom_right, b[2], 0.05)
        && near(a.bottom_left, b[3], 0.05)
}

/// Long enough for the theme's fast spatial spring to settle, drawn in between.
fn settle(rule: &KompozTestRule) {
    rule.advance_time_by(1000);
    rule.draw();
    rule.advance_time_by(100);
}

// ---- The C++'s tests -------------------------------------------------------------------------------------------

fn button_group_grows_the_held_button_and_squeezes_its_neighbours() {
    // The C++'s test, on the deprecated content overload (the C++'s ButtonGroup composed its content as written).
    // Compose's semantics bounds of a button are its clickable container's (40 dp, 4 dp into its 48 of room).
    let (first, second, third) =
        (MutableInteractionSource::new(), MutableInteractionSource::new(), MutableInteractionSource::new());
    let rtl = mutable_state_of(false);
    let (f, s, t, r) = (first.clone(), second.clone(), third.clone(), rtl.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        let (f, s, t) = (f.clone(), s.clone(), t.clone());
        KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], move || {
            let (f, s, t) = (f.clone(), s.clone(), t.clone());
            Column(move || {
                let (f, s, t) = (f.clone(), s.clone(), t.clone());
                ButtonGroupWithoutOverflowIndicator(
                    content = move |group: &ButtonGroupScope| {
                        Button(
                            || {},
                            modifier = group.animate_width(Modifier, &f).test_tag("a"),
                            interaction_source = Some(f.clone()),
                            content = || Text("Önceki"),
                        );
                        Button(
                            || {},
                            modifier = group.animate_width(Modifier, &s).test_tag("b"),
                            interaction_source = Some(s.clone()),
                            content = || Text("Oynat"),
                        );
                        Button(
                            || {},
                            modifier = group.animate_width(Modifier, &t).test_tag("c"),
                            interaction_source = Some(t.clone()),
                            content = || Text("Sonraki"),
                        );
                    },
                );
                // narrower than its label and padding
                Button(|| {}, modifier = Modifier.width(50.dp()), content = || Text("Dar"));
            });
        });
    }));
    let at = |tag: &str| bounds(&rule, tag);
    let (a0, b0, c0) = (at("a"), at("b"), at("c"));
    assert!(near(b0.left - a0.right, 12.0, 0.01) && near(c0.left - b0.right, 12.0, 0.01) && near(a0.left, 0.0, 0.01)); // spaced apart
    assert!(b0.width() < a0.width() && a0.width() < c0.width(), "{a0:?} {b0:?} {c0:?}"); // as wide as their labels
    // (the clock by hand: the checks on the way see the animation running)
    rule.main_clock().set_auto_advance(false);

    press(&rule, b0.center());
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    let moving = at("b").width();
    assert!(moving > b0.width() + 0.5 && moving < b0.width() * 1.14, "on its way: {moving}");
    rule.advance_time_by(500);
    let extra = b0.width() * 0.15;
    // (each part rounded to whole px, as Compose's: a px either way)
    assert!(near(at("b").width(), b0.width() + extra, 1.0), "{:?}", at("b"));
    assert!(
        near(at("a").width(), a0.width() - extra / 2.0, 1.0) && near(at("c").width(), c0.width() - extra / 2.0, 1.0)
    );
    assert!(near(at("a").left, 0.0, 0.01) && near(at("c").right, c0.right, 1.01)); // the row keeps its width
    assert!(near(at("c").left - at("b").right, 12.0, 1.01));
    release(&rule, b0.center());
    rule.advance_time_by(500);
    assert!(same_rect(at("a"), a0) && same_rect(at("b"), b0) && same_rect(at("c"), c0));

    press(&rule, a0.center()); // the first one takes it all from the second
    rule.advance_time_by(500);
    let extra = a0.width() * 0.15;
    assert!(
        near(at("a").width(), a0.width() + extra, 1.0)
            && near(at("b").width(), b0.width() - extra, 1.0)
            && same_rect(at("c"), c0)
    );
    // Squeezed narrower than its label and padding: Compose's button keeps its padding, so the label wraps (the
    // C++'s gave the padding up, the label on one line) and the button grows taller with it (the C++'s kept its
    // height); the label stays centered.
    let line = text_bounds(&rule, "Önceki").height();
    let label = text_bounds(&rule, "Oynat");
    assert!(label.height() > line, "wrapped: {label:?}");
    assert!(near(label.left - at("b").left, at("b").right - label.right, 0.51), "centered: {label:?} {:?}", at("b"));
    // A fixed width too: the label wraps inside (the C++'s 22 wide on one line).
    let narrow = text_bounds(&rule, "Dar");
    assert!(narrow.height() > line && narrow.left > 10.0, "{narrow:?}");
    release(&rule, a0.center());
    rule.advance_time_by(500);

    rtl.set(true);
    finish_animations(&rule);
    assert!(near(at("a").right, c0.right, 0.01) && at("c").left < at("b").left); // from the right
    press(&rule, at("c").center()); // the last one, on the left now, takes from the middle
    rule.advance_time_by(500);
    let extra = c0.width() * 0.15;
    assert!(
        near(at("c").width(), c0.width() + extra, 1.0)
            && near(at("b").width(), b0.width() - extra, 1.0)
            && near(at("c").left, 0.0, 0.01)
    );

    let (only, big, small) =
        (MutableInteractionSource::new(), MutableInteractionSource::new(), MutableInteractionSource::new());
    let extreme = KompozTestRule::new(kompozable_lambda!(move || {
        let (only, big, small) = (only.clone(), big.clone(), small.clone());
        Column(move || {
            let (only, big, small) = (only.clone(), big.clone(), small.clone());
            ButtonGroupWithoutOverflowIndicator(
                content = move |group: &ButtonGroupScope| {
                    Button(
                        || {},
                        modifier = group.animate_width(Modifier, &only).test_tag("tek"),
                        interaction_source = Some(only.clone()),
                        content = || Text("Tek"),
                    );
                },
            );
            ButtonGroupWithoutOverflowIndicator(
                expanded_ratio = 0.6,
                horizontal_arrangement = Arrangement::spaced_by(4.dp()),
                content = move |group: &ButtonGroupScope| {
                    Button(
                        || {},
                        modifier = group.animate_width(Modifier, &big).test_tag("büyük"),
                        interaction_source = Some(big.clone()),
                        content = || Text("Önceki"),
                    );
                    Button(
                        || {},
                        modifier = group.animate_width(Modifier, &small).test_tag("küçük"),
                        interaction_source = Some(small.clone()),
                        content = || Text("Oynat"),
                    );
                    Box(modifier = Modifier.size(64.dp()).test_tag("kutu"));
                },
            );
        });
    }));
    let in_ = |tag: &str| bounds(&extreme, tag);
    let alone = in_("tek");
    press(&extreme, alone.center());
    extreme.advance_time_by(500);
    assert!(same_rect(in_("tek"), alone)); // no neighbour to take from
    release(&extreme, alone.center());
    extreme.advance_time_by(500);
    let (big0, small0, kutu) = (in_("büyük"), in_("küçük"), in_("kutu"));
    // M15's deprecated group places its children at the top (the C++'s centered them: 8 dp down in the 64 dp row);
    // the container 4 dp into its 48 dp of room. 4 dp apart.
    assert!(near(small0.top, kutu.top + 4.0, 0.01) && near(small0.left - big0.right, 4.0, 0.01), "{small0:?} {kutu:?}");
    press(&extreme, big0.center());
    extreme.advance_time_by(500);
    assert!(near(in_("küçük").width(), small0.width() - big0.width() * 0.6, 1.0) && same_rect(in_("kutu"), kutu));
    let wrapped = text_bounds(&extreme, "Oynat");
    // Past its label and its paddings: Compose's button keeps its 24 dp paddings, so nothing is left for the label
    // (measured 0 wide; the C++'s wrapped it inside), and the button grows taller with the letters' lines.
    assert!(wrapped.width() == 0.0 && in_("küçük").height() > small0.height(), "{wrapped:?} {:?}", in_("küçük"));
}

fn button_group_puts_what_does_not_fit_into_a_menu() {
    // The C++'s test on M15's ButtonGroup (items, the default overflow indicator). The C++'s menu showed a check mark
    // ("Selected") by a checked toggle; M15's menu item of a toggleable item is a plain item of its label (and icon).
    let log = Rc::new(RefCell::new(Vec::<&'static str>::new()));
    let cut = mutable_state_of(false);
    let width = mutable_state_of(400.0f32);
    let (l, c, w) = (log.clone(), cut.clone(), width.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (l, c) = (l.clone(), c.clone());
            Box(
                modifier = Modifier.width(w.get().dp()),
                content = move || {
                    let (l, c) = (l.clone(), c.clone());
                    ButtonGroup(
                        overflow_indicator =
                            |menu_state: &ButtonGroupMenuState| ButtonGroupDefaults.OverflowIndicator(menu_state),
                        content = move |items: &ButtonGroupScope| {
                            let (l1, l2, l3, l4) = (l.clone(), l.clone(), l.clone(), l.clone());
                            let c2 = c.clone();
                            items.clickable_item(move || l1.borrow_mut().push("kopyala"), "Kopyala");
                            items.clickable_item(move || l2.borrow_mut().push("yapıştır"), "Yapıştır");
                            items.toggleable_item(c.get(), "Kes", move |on| c2.set(on));
                            items.clickable_item(move || l3.borrow_mut().push("sil"), "Sil");
                            items.clickable_item(move || l4.borrow_mut().push("paylaş"), "Paylaş");
                        },
                    );
                },
            );
        }),
        IntSize::new(600, 800),
    );
    rule.wait_for_idle();
    // Kopyala, Yapıştır and Kes (with the 48 dp ⋮ button, 12 apart) fit in 400; Sil and Paylaş don't.
    assert!(drawn(&rule, "Kopyala") && drawn(&rule, "Yapıştır") && drawn(&rule, "Kes") && shown(&rule, "Kes"));
    assert!(!drawn(&rule, "Sil") && !drawn(&rule, "Paylaş"));
    assert!(rule.on_node_with_content_description("More options").exists());
    tap(&rule, shown_bounds(&rule, "Kopyala").center());
    tap(&rule, rule.on_node_with_content_description("More options").get_unclipped_bounds_in_root().center()); // the menu: the rest
    assert!(drawn(&rule, "Sil") && drawn(&rule, "Paylaş"));
    tap(&rule, shown_bounds(&rule, "Paylaş").center());
    assert_eq!(*log.borrow(), vec!["kopyala", "paylaş"]);
    assert!(!drawn(&rule, "Sil")); // picked, and the menu closed
    width.set(600.0); // room for all: no ⋮
    rule.wait_for_idle();
    // (the indicator is composed still, not placed: not displayed, as Compose's)
    assert!(drawn(&rule, "Sil") && drawn(&rule, "Paylaş"));
    rule.on_node_with_content_description("More options").assert_is_not_displayed();
    width.set(300.0); // the toggle goes into the menu
    cut.set(true);
    rule.wait_for_idle();
    assert!(!drawn(&rule, "Kes"));
    tap(&rule, rule.on_node_with_content_description("More options").get_unclipped_bounds_in_root().center());
    assert!(drawn(&rule, "Kes") && !rule.on_node_with_content_description("Selected").exists());
    tap(&rule, shown_bounds(&rule, "Kes").center()); // picked in the menu: it toggles
    assert!(!cut.get());
}

fn connected_button_group_rounds_the_outer_ends_and_the_checked_one() {
    // The C++'s test: a connected group of toggle buttons in a Row (as M15's connected samples).
    let r = Size::new(100.0, 40.0);
    let radii = |shape: &Arc<dyn Shape>, direction: LayoutDirection| -> [f32; 4] {
        match shape.create_outline(r, direction, &Density::new(1.0, 1.0)) {
            Outline::Rounded(rr) => [
                rr.top_left_corner_radius.x,
                rr.top_right_corner_radius.x,
                rr.bottom_right_corner_radius.x,
                rr.bottom_left_corner_radius.x,
            ],
            other => panic!("{other:?}"),
        }
    };
    let close = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| near(*x, y, 0.05));
    let leading = ButtonGroupDefaults.connected_leading_button_shapes();
    assert!(close(radii(&leading.shape, LayoutDirection::Ltr), [20.0, 8.0, 8.0, 20.0]));
    assert!(close(radii(&leading.shape, LayoutDirection::Rtl), [8.0, 20.0, 20.0, 8.0])); // the start on the right
    assert!(close(
        radii(&ButtonGroupDefaults.connected_trailing_button_shapes().pressed_shape, LayoutDirection::Ltr),
        [4.0, 20.0, 20.0, 4.0]
    ));

    let picked = mutable_state_of(-1);
    let p = picked.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let p = p.clone();
        Row(
            modifier = Modifier.width(320.dp()),
            horizontal_arrangement = Arrangement::spaced_by(ButtonGroupDefaults::ConnectedSpaceBetween),
            content = move || {
                let labels = ["Gün", "Hafta", "Ay"];
                for (i, label) in labels.iter().enumerate() {
                    let p2 = p.clone();
                    let shapes = match i {
                        0 => ButtonGroupDefaults.connected_leading_button_shapes(),
                        2 => ButtonGroupDefaults.connected_trailing_button_shapes(),
                        _ => ButtonGroupDefaults.connected_middle_button_shapes(),
                    };
                    let label = *label;
                    ToggleButton(
                        p.get() == i as i32,
                        move |_| p2.set(i as i32),
                        modifier = Modifier.weight(1.0).test_tag(label),
                        shapes = shapes,
                        content = move || Text(label),
                    );
                }
            },
        );
    }));
    let look = |tag: &str| {
        rule.draw();
        corners_of(&rule, tag)
    };
    let (day, week) = (bounds(&rule, "Gün"), bounds(&rule, "Hafta"));
    // (the clock by hand: the checks on the way see the animation running)
    rule.main_clock().set_auto_advance(false);
    assert!(near(week.left - day.right, 2.0, 0.01)); // connected: 2 dp apart
    assert!(
        same_corners(look("Gün"), [20.0, 8.0, 8.0, 20.0])
            && same_corners(look("Hafta"), [8.0; 4])
            && same_corners(look("Ay"), [8.0, 20.0, 20.0, 8.0])
    );
    press(&rule, week.center());
    settle(&rule);
    assert!(same_corners(look("Hafta"), [4.0; 4])); // squarer while held
    release(&rule, week.center());
    rule.advance_frame();
    rule.draw();
    rule.advance_frame();
    rule.advance_frame();
    let between = look("Hafta");
    assert!(between.top_left > 4.5 && between.top_left < 19.5, "on its way round: {between:?}");
    settle(&rule);
    assert!(picked.get() == 1 && same_corners(look("Hafta"), [20.0; 4])); // checked: fully round
    tap(&rule, day.center());
    settle(&rule);
    assert!(same_corners(look("Gün"), [20.0; 4]) && same_corners(look("Hafta"), [8.0; 4])); // corner by corner, back again
}

// (C++ expressive_buttons_take_compose_named_arguments: in both syntaxes in crates/kompoz/tests/named_forms.rs)

// ---- Compose's behaviour ---------------------------------------------------------------------------------------

fn button_group_weights_share_what_is_left() {
    // Compose's weights: a fixed 50 dp child and 12 dp after it, then the rest (less the 12 dp between the weighted)
    // shared 1:2; the group as wide as the room (Compose's weighted space, its spacing added per child, kept to what
    // is left).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Box(
                modifier = Modifier.width(320.dp()),
                content = || {
                    ButtonGroupWithoutOverflowIndicator(
                        modifier = Modifier.test_tag("grup"),
                        content = |group: &ButtonGroupScope| {
                            Box(modifier = Modifier.size(50.dp()).test_tag("sabit"));
                            Box(modifier = group.weight(Modifier, 1.0).height(10.dp()).test_tag("bir"));
                            Box(modifier = group.weight(Modifier, 2.0).height(10.dp()).test_tag("iki"));
                        },
                    );
                },
            );
            // the items' weights: the two items share the 200 dp less the 12 between them
            Box(
                modifier = Modifier.width(200.dp()),
                content = || {
                    ButtonGroup(
                        overflow_indicator = |m: &ButtonGroupMenuState| ButtonGroupDefaults.OverflowIndicator(m),
                        content = |items: &ButtonGroupScope| {
                            items.clickable_item(|| {}, "A", weight = 1.0);
                            items.clickable_item(|| {}, "B", weight = 1.0);
                        },
                    );
                },
            );
        });
    }));
    let at = |tag: &str| bounds(&rule, tag);
    assert!(near(at("grup").width(), 320.0, 0.01), "{:?}", at("grup"));
    assert!(
        near(at("sabit").left, 0.0, 0.01) && near(at("bir").left, 62.0, 0.01) && near(at("bir").width(), 82.0, 0.01)
    );
    assert!(near(at("iki").left, 156.0, 0.01) && near(at("iki").width(), 164.0, 0.01));
    let (a, b) = (shown_bounds(&rule, "A"), shown_bounds(&rule, "B"));
    assert!(b.left > a.right, "{a:?} {b:?}");
    let buttons = rule.on_all_nodes(has_role(Role::Button)).fetch_semantics_nodes();
    let widths: Vec<f32> = buttons.iter().map(|n| n.bounds_in_root().width()).collect();
    assert!(widths.iter().filter(|w| near(**w, 94.0, 0.01)).count() == 2, "{widths:?}");
}

fn button_group_items_align_vertically() {
    // `vertical_alignment` places the items in the row's height; an item's `align` wins.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        ButtonGroup(
            overflow_indicator = |m: &ButtonGroupMenuState| ButtonGroupDefaults.OverflowIndicator(m),
            vertical_alignment = Alignment::CenterVertically,
            content = |items: &ButtonGroupScope| {
                items.custom_item(|| Box(modifier = Modifier.size(64.dp()).test_tag("uzun")), |_| {});
                items.custom_item(|| Box(modifier = Modifier.size(30.dp()).test_tag("orta")), |_| {});
                let scope = items.clone();
                items.custom_item(
                    move || Box(modifier = scope.align(Modifier, Alignment::Bottom).size(20.dp()).test_tag("alt")),
                    |_| {},
                );
            },
        );
    }));
    assert!(near(bounds(&rule, "uzun").top, 0.0, 0.01));
    assert!(near(bounds(&rule, "orta").top, 17.0, 0.01));
    assert!(near(bounds(&rule, "alt").top, 44.0, 0.01));
    // the deprecated group: at the top whatever the heights
    let top = KompozTestRule::new(kompozable_lambda!(|| {
        ButtonGroupWithoutOverflowIndicator(
            content = |_: &ButtonGroupScope| {
                Box(modifier = Modifier.size(64.dp()));
                Box(modifier = Modifier.size(30.dp()).test_tag("üst"));
            },
        );
    }));
    assert!(near(bounds(&top, "üst").top, 0.0, 0.01));
}

fn button_group_defaults_follow_the_tokens() {
    assert_eq!(ButtonGroupDefaults::ExpandedRatio, 0.15);
    assert_eq!(ButtonGroupDefaults::ConnectedSpaceBetween, 2.dp());
    assert!(ButtonGroupDefaults::HorizontalArrangement() == Arrangement::spaced_by(12.dp()));
    assert_eq!(ButtonGroupDefaults::HorizontalArrangement().spacing(), 12.dp());
    let circle: Arc<dyn Shape> = Arc::new(CircleShape);
    assert!(ButtonGroupDefaults::connected_button_checked_shape() == CircleShape);
    let middle = ButtonGroupDefaults.connected_middle_button_shapes();
    let small: Arc<dyn Shape> = Arc::new(RoundedCornerShape(CornerSize::dp(8.dp())));
    let extra_small: Arc<dyn Shape> = Arc::new(RoundedCornerShape(CornerSize::dp(4.dp())));
    assert!(*middle.shape == *small && *middle.pressed_shape == *extra_small && *middle.checked_shape == *circle);
    let leading = ButtonGroupDefaults.connected_leading_button_shapes();
    let expected: Arc<dyn Shape> = Arc::new(RoundedCornerShape::new(
        CornerSize::percent(100),
        CornerSize::dp(8.dp()),
        CornerSize::dp(8.dp()),
        CornerSize::percent(100),
    ));
    assert!(*leading.shape == *expected && *leading.checked_shape == *circle);
    let trailing = ButtonGroupDefaults.connected_trailing_button_shapes();
    let expected: Arc<dyn Shape> = Arc::new(RoundedCornerShape::new(
        CornerSize::dp(4.dp()),
        CornerSize::percent(100),
        CornerSize::percent(100),
        CornerSize::dp(4.dp()),
    ));
    assert!(*trailing.pressed_shape == *expected);
    // given shapes kept
    let given = named_args!(ButtonGroupDefaults.connected_middle_button_shapes(shape = circle.clone()));
    assert!(*given.shape == *circle && *given.pressed_shape == *extra_small);
}

fn button_group_menu_state_shows_and_dismisses() {
    let state = ButtonGroupMenuState::new(false);
    assert!(!state.is_showing());
    state.show();
    assert!(state.is_showing());
    state.dismiss();
    assert!(!state.is_showing());
    let shown = named_args!(ButtonGroupMenuState(initial_is_showing = true));
    assert!(shown.is_showing() && shown == shown.clone() && shown != state);
    assert_eq!(format!("{shown:?}"), "ButtonGroupMenuState(isShowing=true)");
}

fn button_group_overflow_indicator_opens_the_menu_and_tells_its_name() {
    // The default indicator: a filled icon button (40 dp, primary) of three dots named "More options", toggling
    // the menu, a tooltip of its name above it while held long.
    let state = ButtonGroupMenuState::new(false);
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Box(
            modifier = Modifier.padding(100.dp()),
            content = move || {
                ButtonGroupDefaults.OverflowIndicator(&s, modifier = Modifier.test_tag("daha"));
            },
        );
    }));
    let at = bounds(&rule, "daha");
    assert!(near(at.width(), 40.0, 0.01) && near(at.height(), 40.0, 0.01), "{at:?}");
    let canvas = rule.draw();
    let cs = light_color_scheme();
    assert!(canvas.rect_colors.contains(&cs.primary), "{:?}", canvas.rect_colors);
    rule.on_node_with_content_description("More options").assert_exists();
    tap(&rule, at.center());
    assert!(state.is_showing());
    tap(&rule, at.center());
    assert!(!state.is_showing());
    // held: the tooltip
    press(&rule, at.center());
    rule.advance_time_by(1000);
    assert!(drawn(&rule, "More options"), "{:?}", rule.drawn_texts());
    release(&rule, at.center());
    rule.wait_for_idle();
    // disabled: no clicks
    let disabled = ButtonGroupMenuState::new(false);
    let d = disabled.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        ButtonGroupDefaults.OverflowIndicator(&d, modifier = Modifier.test_tag("kapalı"), enabled = false);
    }));
    tap(&rule, bounds(&rule, "kapalı").center());
    assert!(!disabled.is_showing());
}

fn button_group_items_take_weights_and_icons() {
    // Clickable and toggleable items: buttons of the label after the icon (8 dp between), a toggleable one a
    // checkbox, checked or not; a disabled item takes no clicks; each item grows as it is pressed.
    let clicks = Rc::new(Cell::new(0));
    let checked = mutable_state_of(true);
    let (c, k) = (clicks.clone(), checked.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (c, k) = (c.clone(), k.clone());
        ButtonGroup(
            overflow_indicator = |m: &ButtonGroupMenuState| ButtonGroupDefaults.OverflowIndicator(m),
            content = move |items: &ButtonGroupScope| {
                let (c1, c2, k2) = (c.clone(), c.clone(), k.clone());
                items.clickable_item(
                    move || c1.set(c1.get() + 1),
                    "Ara",
                    icon = Some(Rc::new(|| Box(modifier = Modifier.size(18.dp()).test_tag("simge"))) as Rc<dyn Fn()>),
                );
                items.toggleable_item(k.get(), "Seç", move |on| k2.set(on));
                items.clickable_item(move || c2.set(c2.get() + 10), "Kapalı", enabled = false);
            },
        );
    }));
    let (icon, label) = (bounds(&rule, "simge"), text_bounds(&rule, "Ara"));
    assert!(near(label.left - icon.right, 8.0, 0.01), "{icon:?} {label:?}");
    rule.on_node_with_text("Ara").perform_click();
    rule.on_node_with_text("Kapalı").perform_click();
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 1);
    rule.on_all_nodes(has_role(Role::Checkbox).and(is_on()).and(has_text("Seç", false, false))).assert_count_equals(1);
    rule.on_node_with_text("Seç").perform_click();
    rule.wait_for_idle();
    assert!(!checked.get());
    let button = rule.on_all_nodes(has_role(Role::Button)).fetch_semantics_nodes()[0].bounds_in_root();
    press(&rule, button.center());
    rule.advance_time_by(500);
    let pressed = rule.on_all_nodes(has_role(Role::Button)).fetch_semantics_nodes()[0].bounds_in_root();
    assert!(near(pressed.width(), button.width() * 1.15, 1.0), "{button:?} {pressed:?}");
    release(&rule, button.center());
    rule.advance_time_by(500);
}

// ---- M15's ButtonGroupSamples.kt -------------------------------------------------------------------------------

#[kompozable]
fn ButtonGroupSample() {
    let num_buttons = 10;
    ButtonGroup(
        overflow_indicator = |menu_state: &ButtonGroupMenuState| {
            ButtonGroupDefaults.OverflowIndicator(menu_state = menu_state);
        },
        vertical_alignment = Alignment::Top,
        content = move |scope: &ButtonGroupScope| {
            for i in 0..num_buttons {
                scope.clickable_item(on_click = || {}, label = format!("{i}"));
            }
        },
    );
}

/// The connected samples' icons (Work, Restaurant, Coffee, Search, Home: outlined unchecked, filled checked).
fn sample_icon(index: usize, checked: bool) -> ImageVector {
    match (index, checked) {
        (0, false) => Icons::Outlined::Work(),
        (1, false) => Icons::Outlined::Restaurant(),
        (2, false) => Icons::Outlined::Coffee(),
        (3, false) => Icons::Outlined::Search(),
        (4, false) => Icons::Outlined::Home(),
        (0, true) => Icons::Filled::Work(),
        (1, true) => Icons::Filled::Restaurant(),
        (2, true) => Icons::Filled::Coffee(),
        (3, true) => Icons::Filled::Search(),
        _ => Icons::Filled::Home(),
    }
}

fn connected_shapes(index: usize, last_index: usize) -> ToggleButtonShapes {
    if index == 0 {
        ButtonGroupDefaults.connected_leading_button_shapes()
    } else if index == last_index {
        ButtonGroupDefaults.connected_trailing_button_shapes()
    } else {
        ButtonGroupDefaults.connected_middle_button_shapes()
    }
}

/// A connected sample's button: its icon, the spacing, its label.
#[kompozable]
fn SampleToggle(
    index: usize,
    label: &'static str,
    checked: bool,
    on_checked_change: Rc<dyn Fn(bool)>,
    modifier: Modifier,
    last_index: usize,
) {
    ToggleButton(
        checked = checked,
        on_checked_change = move |c| on_checked_change(c),
        modifier = modifier,
        shapes = connected_shapes(index, last_index),
        content = move || {
            VectorIcon(sample_icon(index, checked), Some("Localized description".into()));
            Spacer(Modifier.size(ToggleButtonDefaults::IconSpacing));
            Text(label);
        },
    );
}

#[kompozable]
fn SingleSelectConnectedButtonGroupSample() {
    let options = ["Work", "Restaurant", "Coffee"];
    let selected_index = remember(|| mutable_int_state_of(0));
    let s = selected_index.clone();
    Row(
        modifier = Modifier.padding_hv(horizontal = 8.dp()),
        horizontal_arrangement = Arrangement::spaced_by(ButtonGroupDefaults::ConnectedSpaceBetween),
        content = move || {
            let modifiers = [Modifier.weight(1.0), Modifier.weight(1.5), Modifier.weight(1.0)];
            for (index, label) in options.iter().enumerate() {
                let s2 = s.clone();
                SampleToggle(
                    index,
                    label,
                    s.get() == index as i32,
                    Rc::new(move |_| s2.set(index as i32)),
                    modifiers[index].clone().semantics(|c| c.set_role(Role::RadioButton)),
                    options.len() - 1,
                );
            }
        },
    );
}

#[kompozable]
fn SingleSelectConnectedButtonGroupWithFlowLayoutSample() {
    let options = ["Work", "Restaurant", "Coffee", "Search", "Home"];
    let selected_index = remember(|| mutable_int_state_of(0));
    let s = selected_index.clone();
    FlowRow(
        modifier = Modifier.padding_hv(horizontal = 8.dp()).fill_max_width(),
        horizontal_arrangement = Arrangement::spaced_by(ButtonGroupDefaults::ConnectedSpaceBetween),
        vertical_arrangement = Arrangement::spaced_by(2.dp()),
        content = move || {
            for (index, label) in options.iter().enumerate() {
                let s2 = s.clone();
                SampleToggle(
                    index,
                    label,
                    s.get() == index as i32,
                    Rc::new(move |_| s2.set(index as i32)),
                    Modifier.semantics(|c| c.set_role(Role::RadioButton)),
                    options.len() - 1,
                );
            }
        },
    );
}

#[kompozable]
fn MultiSelectConnectedButtonGroupSample() {
    let options = ["Work", "Restaurant", "Coffee"];
    let checked = remember(|| mutable_state_list_of(vec![false, false, false]));
    let c = checked.clone();
    Row(
        modifier = Modifier.padding_hv(horizontal = 8.dp()),
        horizontal_arrangement = Arrangement::spaced_by(ButtonGroupDefaults::ConnectedSpaceBetween),
        content = move || {
            let modifiers = [Modifier.weight(1.0), Modifier.weight(1.5), Modifier.weight(1.0)];
            for (index, label) in options.iter().enumerate() {
                let c2 = c.clone();
                SampleToggle(
                    index,
                    label,
                    c.get(index).unwrap(),
                    Rc::new(move |it| c2.set(index, it)),
                    modifiers[index].clone(),
                    options.len() - 1,
                );
            }
        },
    );
}

#[kompozable]
fn MultiSelectConnectedButtonGroupWithFlowLayoutSample() {
    let options = ["Work", "Restaurant", "Coffee", "Search", "Home"];
    let checked = remember(|| mutable_state_list_of(vec![false, false, false, false, false]));
    let c = checked.clone();
    FlowRow(
        modifier = Modifier.padding_hv(horizontal = 8.dp()).fill_max_width(),
        horizontal_arrangement = Arrangement::spaced_by(ButtonGroupDefaults::ConnectedSpaceBetween),
        vertical_arrangement = Arrangement::spaced_by(2.dp()),
        content = move || {
            for (index, label) in options.iter().enumerate() {
                let c2 = c.clone();
                SampleToggle(
                    index,
                    label,
                    c.get(index).unwrap(),
                    Rc::new(move |it| c2.set(index, it)),
                    Modifier,
                    options.len() - 1,
                );
            }
        },
    );
}

#[kompozable]
fn VerticalButtonGroupSample() {
    let options = ["Button 1", "Button 2", "Button 3", "Button 4", "Button 5"];
    let selected_index = remember(|| mutable_int_state_of(0));
    let s = selected_index.clone();
    Column(
        vertical_arrangement = Arrangement::spaced_by((-6.0).dp()),
        content = move || {
            for (index, label) in options.iter().enumerate() {
                // (Kotlin's `shape as RoundedCornerShape`)
                let middle = ButtonGroupDefaults.connected_middle_button_shapes().shape;
                let as_rounded = || -> RoundedCornerShape {
                    match (*middle).shape_as_any().downcast_ref::<CornerBasedShape>() {
                        Some(CornerBasedShape::Rounded(r)) => r.clone(),
                        _ => (*middle)
                            .shape_as_any()
                            .downcast_ref::<RoundedCornerShape>()
                            .expect("a RoundedCornerShape")
                            .clone(),
                    }
                };
                let shape: Arc<dyn Shape> = if index == 0 {
                    Arc::new(named_args!(
                        as_rounded().copy(top_start = CornerSize::percent(100), top_end = CornerSize::percent(100))
                    ))
                } else if index == options.len() - 1 {
                    Arc::new(named_args!(
                        as_rounded()
                            .copy(bottom_start = CornerSize::percent(100), bottom_end = CornerSize::percent(100))
                    ))
                } else {
                    middle.clone()
                };
                let s2 = s.clone();
                let label = *label;
                ToggleButton(
                    checked = s.get() == index as i32,
                    on_checked_change = move |_| s2.set(index as i32),
                    shapes = ToggleButtonDefaults.shapes(
                        shape = shape,
                        checked_shape =
                            Arc::new(ButtonGroupDefaults::connected_button_checked_shape()) as Arc<dyn Shape>,
                    ),
                    modifier = Modifier.semantics(|c| c.set_role(Role::RadioButton)),
                    content = move || Text(label),
                );
            }
        },
    );
}

fn button_group_samples() {
    // ButtonGroupSample: ten items in 400 dp; those that don't fit are in the menu the indicator opens.
    let rule = KompozTestRule::new(ButtonGroupSample);
    rule.wait_for_idle();
    assert!(drawn(&rule, "0") && !drawn(&rule, "9"));
    tap(&rule, rule.on_node_with_content_description("More options").get_unclipped_bounds_in_root().center());
    assert!(drawn(&rule, "9"));

    // SingleSelectConnectedButtonGroupSample: three radio buttons sharing the row 1:1.5:1, the first checked.
    let rule = KompozTestRule::new(SingleSelectConnectedButtonGroupSample);
    let radios = rule.on_all_nodes(has_role(Role::RadioButton)).fetch_semantics_nodes();
    assert_eq!(radios.len(), 3);
    let widths: Vec<f32> = radios.iter().map(|n| n.bounds_in_root().width()).collect();
    assert!(near(widths[1], widths[0] * 1.5, 1.0) && near(widths[0], widths[2], 1.0), "{widths:?}");
    rule.on_node_with_text("Coffee").perform_click();
    rule.wait_for_idle();
    rule.on_all_nodes(is_on().and(has_text("Coffee", false, false))).assert_count_equals(1);
    rule.on_all_nodes(is_on()).assert_count_equals(1);

    // The flow row: five that don't fit in one row wrap, 2 dp below.
    let rule = KompozTestRule::new(SingleSelectConnectedButtonGroupWithFlowLayoutSample);
    let tops: Vec<f32> = ["Work", "Home"].iter().map(|t| text_bounds(&rule, t).top).collect();
    assert!(tops[1] > tops[0] + 40.0, "{tops:?}");
    rule.on_node_with_text("Home").perform_click();
    rule.wait_for_idle();
    rule.on_all_nodes(is_on().and(has_text("Home", false, false))).assert_count_equals(1);

    // Multi select: each toggles on its own.
    let rule = KompozTestRule::new(MultiSelectConnectedButtonGroupSample);
    rule.on_node_with_text("Work").perform_click();
    rule.on_node_with_text("Coffee").perform_click();
    rule.wait_for_idle();
    rule.on_all_nodes(is_on()).assert_count_equals(2);
    let rule = KompozTestRule::new(MultiSelectConnectedButtonGroupWithFlowLayoutSample);
    rule.on_node_with_text("Search").perform_click();
    rule.on_node_with_text("Home").perform_click();
    rule.on_node_with_text("Home").perform_click();
    rule.wait_for_idle();
    rule.on_all_nodes(is_on()).assert_count_equals(1);

    // Vertical: one under the other, overlapping by 6 dp; the first checked.
    let rule = KompozTestRule::new(VerticalButtonGroupSample);
    let (one, two) = (text_bounds(&rule, "Button 1"), text_bounds(&rule, "Button 2"));
    assert!(two.top > one.top, "{one:?} {two:?}");
    rule.on_node_with_text("Button 3").perform_click();
    rule.wait_for_idle();
    rule.on_all_nodes(is_on().and(has_text("Button 3", false, false))).assert_count_equals(1);
}
