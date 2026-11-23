//! Material's navigation bars and short navigation bars as Compose's material3 (1.5.0-alpha17) has them: the C++
//! Kompoz's tests by name (kompoz_tests.cpp; where M15's values differ from the C++'s, M15's are asserted and the
//! difference said), Compose's behaviour (labels shown only when selected, items without labels, disabled items,
//! the insets, the overrides, the colors, accessibility, the shared item's start icon), and the M15 samples of
//! NavigationBarSamples.kt and BadgeSamples.kt's NavigationBarItemWithBadge.

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::layout::WindowInsets;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material_icons_extended::Icons as ExtendedIcons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas, has_text};
use kompoz_ui::input::PointerType;
use kompoz_ui::platform::{Insets, SystemInsets};
use kompoz_ui::semantics::Role;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

tests!(
    navigation_bar_switches_tabs,
    navigation_bar_items_as_compose_1_4,
    short_navigation_bar_is_64_dp_over_the_navigation_bar,
    short_navigation_bar_ripples_start_under_the_finger,
    short_navigation_bar_shares_the_width_or_centres_its_items,
    layouts_that_measure_twice_probe_first_so_sizes_settle,
    labels_shown_only_when_selected_come_up_with_the_icon,
    navigation_bar_items_without_labels_center_their_icons,
    navigation_bar_keeps_clear_of_its_insets_and_takes_its_colors,
    disabled_items_take_no_clicks_and_fade,
    navigation_bar_override_makes_the_bars_below,
    short_navigation_bar_override_makes_the_bars_below,
    item_colors_follow_the_tokens_and_copy,
    short_navigation_bar_items_put_their_icons_at_the_start,
    navigation_bar_items_tell_accessibility_their_tab_role_and_selection,
    badged_navigation_bar_items,
    navigation_bar_samples,
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

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// A text's own bounds (in the unmerged tree: an item merges its label).
fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    rule.on_node_with(has_text(text, false, false), true).get_unclipped_bounds_in_root()
}

/// A nullable composable slot (Kotlin's `(@Composable () -> Unit)?`).
fn slot(f: impl Fn() + 'static) -> Option<Rc<dyn Fn()>> {
    Some(Rc::new(f))
}

/// The color `text` was drawn in.
fn color_of(canvas: &RecordingCanvas, text: &str) -> Option<Color> {
    canvas.texts.iter().position(|t| t == text).map(|i| canvas.colors[i])
}

/// The rects drawn in `color`, with their corner radii.
fn rects_of(canvas: &RecordingCanvas, color: Color) -> Vec<(Rect, f32)> {
    canvas
        .rects
        .iter()
        .zip(&canvas.rect_colors)
        .zip(&canvas.rect_radii)
        .filter(|((_, c), _)| **c == color)
        .map(|((r, _), radius)| (*r, *radius))
        .collect()
}

fn same_rect(r: Rect, x: f32, y: f32, width: f32, height: f32) -> bool {
    near(r.left, x, 0.01) && near(r.top, y, 0.01) && near(r.width(), width, 0.01) && near(r.height(), height, 0.01)
}

fn tap(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(at, -1, 0);
    rule.advance_frame();
}

/// Whether a ripple (a translucent circle's square) is drawn centered at `at` after pressing there (C++'s
/// `rippleAt`, the press then let go).
fn ripple_at(rule: &KompozTestRule, press: Offset, at: Offset) -> bool {
    rule.host().pointer_down(press, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.draw(); // (a ripple starts when it is first drawn)
    rule.advance_frame();
    let canvas = rule.draw();
    let found = canvas.rects.iter().zip(&canvas.rect_colors).any(|(r, c)| {
        near(r.width(), r.height(), 0.01)
            && c.alpha() > 0.0
            && c.alpha() < 0.2
            && near(r.center().x, at.x, 3.0)
            && near(r.center().y, at.y, 3.0)
    });
    rule.host().pointer_up(press, -1, 0);
    rule.wait_for_idle();
    found
}

// ---- The C++ Kompoz's tests ------------------------------------------------------------------------------------

fn navigation_bar_switches_tabs() {
    let tab = mutable_state_of(0);
    let t = tab.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (t, page) = (t.clone(), t.clone());
            Scaffold(
                modifier = Modifier.fill_max_size(),
                bottom_bar = move || {
                    let t = t.clone();
                    NavigationBar(
                        content = move || {
                            let (t0, t1) = (t.clone(), t.clone());
                            NavigationBarItem(
                                t.get() == 0,
                                move || t0.set(0),
                                || VectorIcon(Icons::Filled::Home(), None),
                                label = slot(|| Text("Bir")),
                            );
                            NavigationBarItem(
                                t.get() == 1,
                                move || t1.set(1),
                                || VectorIcon(Icons::AutoMirrored::Filled::List(), None),
                                label = slot(|| Text("İki")),
                            );
                        },
                    );
                },
                content = move |padding: PaddingValues| {
                    Text(
                        if page.get() == 0 { "sayfa 1" } else { "sayfa 2" },
                        modifier = Modifier.padding_values(padding),
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let label = text_bounds(&rule, "İki");
    assert!(label.top > 800.0 - 80.0 && label.bottom < 800.0, "{label:?}"); // inside the 80dp bar at the bottom
    rule.on_node_with_text("İki").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("sayfa 2").assert_exists();
}

fn navigation_bar_items_as_compose_1_4() {
    // M15's pill is 56 × 32 dp (NavigationBarVerticalItemTokens.ActiveIndicatorWidth; the C++'s, as 1.4, 64).
    let tab = mutable_state_of(0);
    let scheme = Rc::new(Cell::new(light_color_scheme()));
    let (t, sc) = (tab.clone(), scheme.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            sc.set(MaterialTheme::color_scheme());
            let t = t.clone();
            NavigationBar(
                content = move || {
                    let (t0, t1) = (t.clone(), t.clone());
                    NavigationBarItem(
                        t.get() == 0,
                        move || t0.set(0),
                        || VectorIcon(Icons::Filled::Home(), None),
                        label = slot(|| Text("Bir")),
                    );
                    NavigationBarItem(
                        t.get() == 1,
                        move || t1.set(1),
                        || VectorIcon(Icons::AutoMirrored::Filled::List(), None),
                        label = slot(|| Text("İki")),
                    );
                },
            );
        }),
        IntSize::new(400, 300),
    );
    rule.wait_for_idle();
    let cs = scheme.get();
    let (first, second) = (text_bounds(&rule, "Bir"), text_bounds(&rule, "İki"));
    let indicator_under = |canvas: &RecordingCanvas, label: Rect| {
        rects_of(canvas, cs.secondary_container).into_iter().find(|(r, _)| near(r.center().x, label.center().x, 1.0))
    };
    // The label 4 dp under the 56 × 32 indicator 12 dp down (the bar 80 tall): 48 dp down.
    let canvas = rule.draw();
    let shown = indicator_under(&canvas, first).map(|(r, _)| r);
    assert!(
        shown.is_some_and(|r| near(r.width(), 56.0, 0.01)
            && near(r.height(), 32.0, 0.01)
            && near(r.top, first.top - 36.0, 0.01)),
        "{shown:?} {first:?}"
    );
    assert!(indicator_under(&canvas, second).is_none());
    assert_eq!((color_of(&canvas, "Bir"), color_of(&canvas, "İki")), (Some(cs.secondary), Some(cs.on_surface_variant)));
    // Picked: the indicator grows out of the middle, fading in; the colors eased.
    tab.set(1);
    for _ in 0..3 {
        rule.advance_frame();
    }
    let canvas = rule.draw();
    let growing = indicator_under(&canvas, second).map(|(r, _)| r);
    let fading = canvas.layer_alphas.iter().any(|a| *a > 0.01 && *a < 0.99);
    assert!(
        growing.is_some_and(|r| r.width() > 4.0 && r.width() < 52.0) && fading,
        "{growing:?} {:?}",
        canvas.layer_alphas
    );
    assert_ne!(color_of(&canvas, "İki"), Some(cs.secondary));
    rule.advance_time_by(1000);
    let canvas = rule.draw();
    let grown = indicator_under(&canvas, second).map(|(r, _)| r);
    assert!(grown.is_some_and(|r| near(r.width(), 56.0, 0.01)), "{grown:?}");
    assert_eq!(color_of(&canvas, "İki"), Some(cs.secondary));
    assert!(indicator_under(&canvas, first).is_none());
    // The ripple on the indicator starts where the finger went down.
    let at = Offset::new(first.center().x + 20.0, first.top - 20.0);
    assert!(ripple_at(&rule, at, at));
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(at, -1, 0);
    rule.wait_for_idle();
    assert_eq!(tab.get(), 0);
    let tree = rule.host().accessibility_tree();
    let tabs: Vec<_> = tree.iter().filter(|n| n.role == Some(Role::Tab)).collect();
    assert_eq!((tabs.len(), tabs.iter().filter(|n| n.checked == Some(true)).count()), (2, 1));
}

fn short_navigation_bar_is_64_dp_over_the_navigation_bar() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Box(
                modifier = Modifier.fill_max_size(),
                content_alignment = Alignment::BottomCenter,
                content = || {
                    ShortNavigationBar(
                        modifier = Modifier.test_tag("çubuk"),
                        content = || {
                            ShortNavigationBarItem(
                                true,
                                || {},
                                || VectorIcon(Icons::Filled::Home(), Some(String::new())),
                                slot(|| Text("Ev")),
                                modifier = Modifier.test_tag("öğe"),
                            );
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.host().set_system_insets(SystemInsets { navigation_bars: Insets::new(0, 0, 0, 48), ..Default::default() });
    rule.wait_for_idle();
    let (bar, item) = (bounds(&rule, "çubuk"), bounds(&rule, "öğe"));
    // Compose's: the inset below its 64 dp
    assert!(
        near(bar.height(), 64.0 + 48.0, 0.01) && near(item.height(), 64.0, 0.01) && near(item.top, bar.top, 0.01),
        "{bar:?} {item:?}"
    );
}

fn short_navigation_bar_ripples_start_under_the_finger() {
    // The C++'s Centered arrangement put the icons at the start; Compose's items take their icon position (the
    // sample's NavigationItemIconPosition.Start with the Centered arrangement), so the test gives it with it.
    let arrangement = mutable_state_of(ShortNavigationBarArrangement::EqualWeight);
    let a = arrangement.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let arrangement = a.get();
            let icon_position = if arrangement == ShortNavigationBarArrangement::Centered {
                NavigationItemIconPosition::Start
            } else {
                NavigationItemIconPosition::Top
            };
            ShortNavigationBar(
                arrangement = arrangement,
                content = move || {
                    ShortNavigationBarItem(
                        true,
                        || {},
                        || Box(modifier = Modifier.size(24.dp())),
                        slot(|| Text("Ana")),
                        modifier = Modifier.test_tag("ana"),
                        icon_position = icon_position,
                    );
                    ShortNavigationBarItem(
                        false,
                        || {},
                        || Box(modifier = Modifier.size(24.dp())),
                        slot(|| Text("Ara")),
                        icon_position = icon_position,
                    );
                },
            );
        }),
        IntSize::new(400, 300),
    );
    // Its icon on top: the 56 × 32 pill above the label. M15 moves the press by NavigationBar's 12 dp (its
    // IndicatorVerticalOffset) though this pill is 6 dp down: the ripple starts 6 dp above the finger (Compose's).
    let label = text_bounds(&rule, "Ana");
    let press = Offset::new(label.center().x + 12.0, label.top - 20.0);
    assert!(ripple_at(&rule, press, Offset::new(press.x, press.y - 6.0)));
    // Its icon at the start: the pill around both. M15 doesn't move the presses of a start icon's item into the
    // pill (no MappedInteractionSource there): the ripple is as far from the finger as the pill is from the item's
    // corner (Compose's).
    arrangement.set(ShortNavigationBarArrangement::Centered);
    rule.wait_for_idle();
    let (label, item) = (text_bounds(&rule, "Ana"), bounds(&rule, "ana"));
    let press = Offset::new(label.center().x, label.center().y + 6.0);
    // (the pill 16 + 24 + 4 + "Ana" + 16 wide and 40 tall, in the middle of the item)
    let pill_width = 16.0 + 24.0 + 4.0 + label.width() + 16.0;
    let pill = Offset::new(((item.width() - pill_width) / 2.0).trunc(), (item.height() - 40.0) / 2.0);
    assert!(ripple_at(&rule, press, press + pill), "{item:?} {label:?}");
}

fn short_navigation_bar_shares_the_width_or_centres_its_items() {
    let selected = mutable_state_of(0);
    let arrangement = mutable_state_of(Some(ShortNavigationBarArrangement::EqualWeight));
    let cs = light_color_scheme();
    let bar = {
        let (selected, arrangement) = (selected.clone(), arrangement.clone());
        move || {
            let (selected, arrangement) = (selected.clone(), arrangement.clone());
            kompozable_lambda!(move || {
                // (None: Kotlin's default arrangement; the C++'s Centered put the icons at the start, Compose's
                // items are given it — see short_navigation_bar_ripples_start_under_the_finger)
                let arrangement = arrangement.get();
                let icon_position = if arrangement == Some(ShortNavigationBarArrangement::Centered) {
                    NavigationItemIconPosition::Start
                } else {
                    NavigationItemIconPosition::Top
                };
                let selected = selected.clone();
                let items = move || {
                    for (i, label) in ["Ana", "Ara", "Profil"].into_iter().enumerate() {
                        let s = selected.clone();
                        ShortNavigationBarItem(
                            selected.get() == i as i32,
                            move || s.set(i as i32),
                            || Box(modifier = Modifier.size(24.dp())),
                            slot(move || Text(label)),
                            modifier = Modifier.test_tag(label),
                            icon_position = icon_position,
                        );
                    }
                };
                match arrangement {
                    Some(arrangement) => ShortNavigationBar(
                        modifier = Modifier.test_tag("çubuk"),
                        arrangement = arrangement,
                        window_insets = WindowInsets::Zero,
                        content = items,
                    ),
                    None => ShortNavigationBar(
                        modifier = Modifier.test_tag("çubuk"),
                        window_insets = WindowInsets::Zero,
                        content = items,
                    ),
                }
            })
        }
    };
    let rule = KompozTestRule::with_viewport(bar(), IntSize::new(360, 640));
    let at = |tag: &str| bounds(&rule, tag);
    struct Look {
        pills: Vec<(Rect, f32)>, // the indicators drawn
        labels: Vec<(String, Color)>,
    }
    let look = || {
        let c = rule.draw();
        Look {
            pills: rects_of(&c, cs.secondary_container),
            labels: c.texts.iter().cloned().zip(c.colors.iter().copied()).collect(),
        }
    };
    assert!(same_rect(at("çubuk"), 0.0, 0.0, 360.0, 64.0) && same_rect(at("Ana"), 0.0, 0.0, 120.0, 64.0));
    assert!(same_rect(at("Profil"), 240.0, 0.0, 120.0, 64.0), "{:?}", at("Profil"));
    let first = look();
    // Behind the icon, above the label: 32 + 4 + 16 (labelMedium's line) centered in 64.
    assert!(
        first.pills.len() == 1
            && same_rect(first.pills[0].0, 32.0, 6.0, 56.0, 32.0)
            && near(first.pills[0].1, 16.0, 0.01),
        "{:?}",
        first.pills
    );
    assert!(first.labels.len() == 3 && first.labels[0].1 == cs.secondary && first.labels[1].1 == cs.on_surface_variant);
    rule.on_node_with_tag("Ana").assert_is_selected();
    rule.on_node_with_tag("Ara").assert_is_not_selected();
    let tabs = rule.host().accessibility_tree().iter().filter(|n| n.role == Some(Role::Tab)).count();
    assert_eq!(tabs, 3); // tabs to accessibility services

    tap(&rule, at("Ara").center());
    // (KompozR's animations move a frame after the C++'s: the change composes, the animation starts the frame
    // after — as Compose's)
    rule.advance_frame();
    rule.advance_frame();
    let growing = rule.draw();
    assert_eq!(selected.get(), 1);
    // (the pill fades in by ModulateAlpha: its color drawn at its alpha, no layer)
    let same_rgb = |c: &Color| {
        let s = cs.secondary_container;
        c.alpha() < 1.0 && c.red() == s.red() && c.green() == s.green() && c.blue() == s.blue()
    };
    let p = growing
        .rects
        .iter()
        .zip(&growing.rect_colors)
        .find(|(r, c)| same_rgb(c) && near(r.center().x, 180.0, 0.5))
        .map(|(r, _)| *r)
        .expect("a pill growing");
    assert!(p.width() > 1.0 && p.width() < 55.0 && near(p.center().x, 180.0, 0.5), "{p:?}"); // from the middle out
    rule.wait_for_idle();
    let second = look();
    assert!(second.pills.len() == 1 && same_rect(second.pills[0].0, 152.0, 6.0, 56.0, 32.0), "{:?}", second.pills);

    arrangement.set(Some(ShortNavigationBarArrangement::Centered));
    rule.wait_for_idle();
    // Each as wide as it needs (16 + 24 icon + 4 + its label + 16; the C++ made them all as wide as the widest),
    // together in the middle: M15 takes half of each widening past the equal share (360 − 2 × 72) / 3 = 72 off the
    // 20% padding (72), at whole px.
    let item_width = |label: &str| 16.0 + 24.0 + 4.0 + text_bounds(&rule, label).width() + 16.0;
    let (ana, ara, profil) = (item_width("Ana"), item_width("Ara"), item_width("Profil"));
    let start = 72.0 - [ana, ara, profil].iter().map(|w| ((w - 72.0) / 2.0).trunc()).sum::<f32>();
    assert!(same_rect(at("Ana"), start, 0.0, ana, 64.0), "{:?} {start} {ana}", at("Ana"));
    assert!(same_rect(at("Ara"), start + ana, 0.0, ara, 64.0), "{:?}", at("Ara"));
    assert!(same_rect(at("Profil"), start + ana + ara, 0.0, profil, 64.0), "{:?}", at("Profil"));
    let centered = look(); // round icon and label, filling its item
    assert!(
        centered.pills.len() == 1
            && same_rect(centered.pills[0].0, start + ana, 12.0, ara, 40.0)
            && near(centered.pills[0].1, 20.0, 0.01),
        "{:?}",
        centered.pills
    );

    // Kotlin's default arrangement: sharing the width. (The C++ picked it by the window — compact sharing,
    // medium and up centered; Compose's default is EqualWeight whatever the window: the navigation suite picks the
    // arrangement by the window size.)
    arrangement.set(None);
    rule.wait_for_idle();
    assert!(same_rect(at("Ana"), 0.0, 0.0, 120.0, 64.0));
    let wide = KompozTestRule::with_viewport(bar(), IntSize::new(700, 400));
    let ana = wide.on_node_with_tag("Ana").get_unclipped_bounds_in_root();
    assert!(near(ana.width(), 233.0, 0.01) && near(ana.left, 0.0, 0.01), "{ana:?}");
}

#[allow(deprecated)] // (WP1b: group — ButtonGroup's deprecated content overload, as the C++ composed it)
fn layouts_that_measure_twice_probe_first_so_sizes_settle() {
    // The C++'s test, whole: a held button in a group squeezing its neighbour (WP1b: group), the short bar's centered
    // items (measured by their intrinsic sizes first) and a button narrower than its label — each told once of the
    // size it ends up with.
    let squeezed = mutable_state_of(0.0f32); // (WP1b: group)
    let item = mutable_state_of(0.0f32);
    let label = mutable_state_of(0.0f32);
    let reports = Rc::new(Cell::new(0));
    let (first, second) = (MutableInteractionSource::new(), MutableInteractionSource::new()); // (WP1b: group)
    let (sq, it, la, re, f, sc) =
        (squeezed.clone(), item.clone(), label.clone(), reports.clone(), first.clone(), second.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (sq, it, la, re, f, sc) = (sq.clone(), it.clone(), la.clone(), re.clone(), f.clone(), sc.clone());
        Column(move || {
            // (WP1b: group)
            let (sq2, re2, f, sc) = (sq.clone(), re.clone(), f.clone(), sc.clone());
            ButtonGroupWithoutOverflowIndicator(
                content = move |group: &ButtonGroupScope| {
                    Button(
                        || {},
                        modifier = group.animate_width(Modifier, &f).test_tag("a"),
                        interaction_source = Some(f.clone()),
                        content = || Text("Önceki"),
                    );
                    let (sq, re) = (sq2.clone(), re2.clone());
                    Button(
                        || {},
                        modifier = group.animate_width(Modifier, &sc).on_size_changed(move |s| {
                            re.set(re.get() + 1);
                            sq.set(s.width as f32);
                        }),
                        interaction_source = Some(sc.clone()),
                        content = || Text("Oynat"),
                    );
                },
            );
            let (it2, re2) = (it.clone(), re.clone());
            ShortNavigationBar(
                arrangement = ShortNavigationBarArrangement::Centered,
                window_insets = WindowInsets::Zero,
                content = move || {
                    let (it, re) = (it2.clone(), re2.clone());
                    ShortNavigationBarItem(
                        true,
                        || {},
                        || {},
                        slot(|| Text("Ana")),
                        modifier = Modifier.on_size_changed(move |s| {
                            re.set(re.get() + 1);
                            it.set(s.width as f32);
                        }),
                    );
                    ShortNavigationBarItem(false, || {}, || {}, slot(|| Text("Uzun sekme")));
                },
            );
            let (la2, re2) = (la.clone(), re.clone());
            // narrower than its label: the padding gone, the label wraps
            Button(
                || {},
                modifier = Modifier.width(16.dp()),
                content = move || {
                    let (la, re) = (la2.clone(), re2.clone());
                    Text(
                        "Dar",
                        modifier = Modifier.on_size_changed(move |s| {
                            re.set(re.get() + 1);
                            la.set(s.height as f32);
                        }),
                    );
                },
            );
            Text(format!("{} {} {}", sq.get(), it.get(), la.get())); // reads them: a change recomposes
        });
    }));
    rule.wait_for_idle();
    // (WP1b: group) the natural widths: Önceki 43 + 48 of padding, Oynat 36 + 48 (labelLarge's letters 7.1 wide)
    let a = rule.on_node_with_tag_unmerged("a").get_unclipped_bounds_in_root();
    let natural = squeezed.get();
    assert!(near(a.width(), 91.0, 0.01) && near(natural, 84.0, 0.01), "{a:?} {natural}");
    rule.host().pointer_down(a.center(), -1, 0, PointerType::Touch);
    rule.advance_time_by(600); // held and settled: the second button squeezed
    let settled = reports.get();
    rule.advance_time_by(300);
    assert_eq!(reports.get(), settled); // told once of each size, not of a size measured on the way
    // (WP1b: group) 84 less 15% of 91 rounded to a whole px, as M15's group gives up whole px (the C++'s 70.35)
    assert!(near(squeezed.get(), 70.0, 0.01), "{}", squeezed.get());
    // M15's centered top-icon items (the icon position Kotlin's default): "Uzun sekme" 65 wide is less than the
    // equal share between the 25% paddings (400 / 4 = 100), so each item is 100 (the C++'s, its icons at the
    // start, the widest's 16 + 65 + 16 = 97).
    assert!(near(item.get(), 100.0, 0.01), "{}", item.get());
    assert!(label.get() > 16.0, "{}", label.get()); // wrapped
}

// ---- Compose's behaviour ---------------------------------------------------------------------------------------

fn labels_shown_only_when_selected_come_up_with_the_icon() {
    let tab = mutable_state_of(0);
    let t = tab.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let t = t.clone();
            NavigationBar(
                content = move || {
                    for (i, name) in ["Bir", "İki"].into_iter().enumerate() {
                        let s = t.clone();
                        NavigationBarItem(
                            t.get() == i as i32,
                            move || s.set(i as i32),
                            move || Box(modifier = Modifier.size(24.dp()).test_tag(format!("simge{i}"))),
                            label = slot(move || Text(name)),
                            always_show_label = false,
                        );
                    }
                },
            );
        }),
        IntSize::new(400, 300),
    );
    let icon = |i: usize| rule.on_node_with_tag_unmerged(&format!("simge{i}")).get_unclipped_bounds_in_root();
    // Selected: the icon 16 dp down, its label under it; unselected: the icon in the middle of the 80 dp, no label.
    assert!(near(icon(0).top, 16.0, 0.01) && near(icon(1).top, 28.0, 0.01), "{:?} {:?}", icon(0), icon(1));
    let canvas = rule.draw();
    assert!(color_of(&canvas, "Bir").is_some() && color_of(&canvas, "İki").is_none(), "{:?}", canvas.texts);
    // (the clock by hand: the check on the way sees the icon moving up)
    rule.main_clock().set_auto_advance(false);
    tab.set(1);
    for _ in 0..4 {
        rule.advance_frame();
    }
    let moving = icon(1).top;
    assert!(moving > 16.0 && moving < 28.0, "{moving}"); // on its way up
    finish_animations(&rule);
    assert!(near(icon(1).top, 16.0, 0.01) && near(icon(0).top, 28.0, 0.01));
    assert!(color_of(&rule.draw(), "İki").is_some());
}

fn navigation_bar_items_without_labels_center_their_icons() {
    let scheme = Rc::new(Cell::new(light_color_scheme()));
    let sc = scheme.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            sc.set(MaterialTheme::color_scheme());
            NavigationBar(
                modifier = Modifier.test_tag("çubuk"),
                window_insets = WindowInsets::Zero,
                content = || {
                    NavigationBarItem(true, || {}, icon = || Box(modifier = Modifier.size(24.dp()).test_tag("simge")));
                },
            );
        }),
        IntSize::new(400, 300),
    );
    let icon = rule.on_node_with_tag_unmerged("simge").get_unclipped_bounds_in_root();
    assert!(same_rect(bounds(&rule, "çubuk"), 0.0, 0.0, 400.0, 80.0));
    assert!(same_rect(icon, 188.0, 28.0, 24.0, 24.0), "{icon:?}");
    let pills = rects_of(&rule.draw(), scheme.get().secondary_container);
    assert!(pills.len() == 1 && same_rect(pills[0].0, 172.0, 24.0, 56.0, 32.0), "{pills:?}");
}

fn navigation_bar_keeps_clear_of_its_insets_and_takes_its_colors() {
    let scheme = Rc::new(Cell::new(light_color_scheme()));
    let sc = scheme.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            sc.set(MaterialTheme::color_scheme());
            Column(|| {
                NavigationBar(
                    modifier = Modifier.test_tag("bir"),
                    content = || {
                        NavigationBarItem(true, || {}, || {}, modifier = Modifier.test_tag("öğe"));
                    },
                );
                let cs = MaterialTheme::color_scheme();
                NavigationBar(
                    modifier = Modifier.test_tag("iki"),
                    container_color = cs.primary,
                    window_insets = WindowInsets(left = 10.dp(), bottom = 6.dp()),
                    content = || {
                        let color = LocalContentColor.current();
                        Text(format!("{:?}", color), modifier = Modifier.test_tag("renk"));
                    },
                );
            });
        }),
        IntSize::new(400, 600),
    );
    rule.host().set_system_insets(SystemInsets {
        navigation_bars: Insets::new(12, 0, 8, 20),
        status_bars: Insets::new(0, 24, 0, 0),
        ..Default::default()
    });
    rule.wait_for_idle();
    let cs = scheme.get();
    // The system bars at the bottom and the sides, not the top.
    let (bar, item) = (bounds(&rule, "bir"), bounds(&rule, "öğe"));
    assert!(near(bar.height(), 80.0 + 20.0, 0.01) && near(item.left, 12.0, 0.01) && near(item.right, 392.0, 0.01));
    assert!(near(item.top, bar.top, 0.01), "{bar:?} {item:?}");
    // Its own insets; the content in the scheme's on-primary over a primary bar.
    let second = bounds(&rule, "iki");
    assert!(near(second.height(), 86.0, 0.01), "{second:?}");
    rule.on_node_with_tag("renk").assert_text_equals_with(&[&format!("{:?}", cs.on_primary)], true);
    let canvas = rule.draw();
    assert!(
        rects_of(&canvas, cs.surface_container).iter().any(|(r, _)| same_rect(*r, bar.left, bar.top, 400.0, 100.0))
    );
    assert!(rects_of(&canvas, cs.primary).iter().any(|(r, _)| near(r.height(), 86.0, 0.01)));
}

fn disabled_items_take_no_clicks_and_fade() {
    let clicked = Rc::new(Cell::new(0));
    let scheme = Rc::new(Cell::new(light_color_scheme()));
    let (c, sc) = (clicked.clone(), scheme.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            sc.set(MaterialTheme::color_scheme());
            let (c1, c2) = (c.clone(), c.clone());
            Column(move || {
                let (c1, c2) = (c1.clone(), c2.clone());
                NavigationBar(
                    content = move || {
                        let c1 = c1.clone();
                        NavigationBarItem(
                            false,
                            move || c1.set(c1.get() + 1),
                            || {},
                            enabled = false,
                            label = slot(|| Text("Kapalı")),
                        );
                    },
                );
                ShortNavigationBar(
                    content = move || {
                        let c2 = c2.clone();
                        ShortNavigationBarItem(
                            false,
                            move || c2.set(c2.get() + 1),
                            || {},
                            slot(|| Text("Kısa")),
                            enabled = false,
                        );
                    },
                );
            });
        }),
        IntSize::new(400, 600),
    );
    let cs = scheme.get();
    rule.on_node_with_text("Kapalı").perform_click();
    rule.on_node_with_text("Kısa").perform_click();
    rule.wait_for_idle();
    assert_eq!(clicked.get(), 0);
    rule.on_node_with_text("Kapalı").assert_is_not_enabled();
    let faded = named_args!(cs.on_surface_variant.copy(alpha = 0.38));
    let canvas = rule.draw();
    assert_eq!((color_of(&canvas, "Kapalı"), color_of(&canvas, "Kısa")), (Some(faded), Some(faded)));
}

struct Plain;

impl NavigationBarOverride for Plain {
    #[track_caller]
    fn NavigationBar(&self, scope: &NavigationBarOverrideScope) {
        let (content, color) = (scope.content(), scope.container_color());
        kompozable_block!({
            Row(modifier = scope.modifier().test_tag("düz").background(color), content = move || content());
        })
    }
}

fn navigation_bar_override_makes_the_bars_below() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            KompozitionLocalProvider(
                &[LocalNavigationBarOverride.provides(Rc::new(Plain) as Rc<dyn NavigationBarOverride>)],
                || {
                    NavigationBar(
                        modifier = Modifier.width(100.dp()),
                        container_color = Color::Red,
                        content = || {
                            Text("içinde");
                        },
                    );
                },
            );
            NavigationBar(modifier = Modifier.test_tag("olağan"), content = || Text("dışında"));
        });
    }));
    let plain = bounds(&rule, "düz");
    assert!(near(plain.width(), 100.0, 0.01) && plain.height() < 80.0, "{plain:?}");
    rule.on_node_with_text("içinde").assert_exists();
    assert!(near(bounds(&rule, "olağan").height(), 80.0, 0.01));
    assert!(rects_of(&rule.draw(), Color::Red).iter().any(|(r, _)| near(r.width(), 100.0, 0.01)));
}

struct Bare(Rc<RefCell<Vec<String>>>);

impl ShortNavigationBarOverride for Bare {
    #[track_caller]
    fn ShortNavigationBar(&self, scope: &ShortNavigationBarOverrideScope) {
        self.0.borrow_mut().push(format!(
            "{:?} {:?}",
            scope.arrangement(),
            scope.window_insets() == WindowInsets::Zero
        ));
        let content = scope.content();
        kompozable_block!({
            Column(modifier = scope.modifier(), content = move || content());
        })
    }
}

fn short_navigation_bar_override_makes_the_bars_below() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        KompozitionLocalProvider(
            &[LocalShortNavigationBarOverride.provides(Rc::new(Bare(s)) as Rc<dyn ShortNavigationBarOverride>)],
            || {
                ShortNavigationBar(
                    modifier = Modifier.test_tag("çıplak"),
                    arrangement = ShortNavigationBarArrangement::Centered,
                    window_insets = WindowInsets::Zero,
                    content = || {
                        Text("bir");
                        Text("iki");
                    },
                );
            },
        );
    }));
    assert_eq!(seen.borrow().last().map(String::as_str), Some("Centered true"));
    // (a column: the second under the first)
    assert!(text_bounds(&rule, "iki").top >= text_bounds(&rule, "bir").bottom);
}

fn item_colors_follow_the_tokens_and_copy() {
    let seen = Rc::new(RefCell::new(None));
    let s = seen.clone();
    KompozTestRule::new(kompozable_lambda!(move || {
        *s.borrow_mut() = Some((
            MaterialTheme::color_scheme(),
            NavigationBarItemDefaults.colors(),
            named_args!(
                NavigationBarItemDefaults.colors(indicator_color = Color::Red, disabled_text_color = Color::Blue)
            ),
            ShortNavigationBarItemDefaults.colors(),
            named_args!(ShortNavigationBarItemDefaults.colors(unselected_icon_color = Color::Green)),
            NavigationBarDefaults::container_color(),
            ShortNavigationBarDefaults::content_color(),
        ));
    }));
    let (cs, bar, given, short, short_given, container, content) = (*seen.borrow()).expect("composed");
    let faded = |c: Color| named_args!(c.copy(alpha = 0.38));
    let expected = NavigationBarItemColors::new(
        cs.on_secondary_container,
        cs.secondary,
        cs.secondary_container,
        cs.on_surface_variant,
        cs.on_surface_variant,
        faded(cs.on_surface_variant),
        faded(cs.on_surface_variant),
    );
    assert_eq!(bar, expected);
    assert_eq!(
        given,
        named_args!(expected.copy(selected_indicator_color = Color::Red, disabled_text_color = Color::Blue))
    );
    assert_eq!(given.copy(), given); // (unspecified keeps)
    let item = NavigationItemColors::new(
        expected.selected_icon_color,
        expected.selected_text_color,
        expected.selected_indicator_color,
        expected.unselected_icon_color,
        expected.unselected_text_color,
        expected.disabled_icon_color,
        expected.disabled_text_color,
    );
    assert_eq!(short, item);
    // (Kotlin's default for a disabled color: the unselected one given, at 38%)
    assert_eq!(
        short_given,
        named_args!(item.copy(unselected_icon_color = Color::Green, disabled_icon_color = faded(Color::Green)))
    );
    assert_eq!(
        (item.icon_color(true, true), item.icon_color(false, true), item.icon_color(true, false)),
        (cs.on_secondary_container, cs.on_surface_variant, faded(cs.on_surface_variant))
    );
    assert_eq!(
        (item.text_color(true, true), item.text_color(false, false)),
        (cs.secondary, faded(cs.on_surface_variant))
    );
    assert_eq!((container, content), (cs.surface_container, cs.on_surface));
    assert_eq!(NavigationBarDefaults::Elevation, 0.dp());
    assert_eq!(ShortNavigationBarDefaults::arrangement(), ShortNavigationBarArrangement::EqualWeight);
}

fn short_navigation_bar_items_put_their_icons_at_the_start() {
    let scheme = Rc::new(Cell::new(light_color_scheme()));
    let sc = scheme.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            sc.set(MaterialTheme::color_scheme());
            ShortNavigationBar(
                window_insets = WindowInsets::Zero,
                content = || {
                    ShortNavigationBarItem(
                        true,
                        || {},
                        || Box(modifier = Modifier.size(24.dp()).test_tag("simge")),
                        slot(|| Text("Uzun")),
                        modifier = Modifier.test_tag("öğe"),
                        icon_position = NavigationItemIconPosition::Start,
                    );
                    // (no label: the icon alone in its pill, whatever the position)
                    ShortNavigationBarItem(
                        false,
                        || {},
                        || Box(modifier = Modifier.size(24.dp()).test_tag("yalnız")),
                        None,
                        icon_position = NavigationItemIconPosition::Start,
                    );
                },
            );
        }),
        IntSize::new(400, 300),
    );
    // A 200 × 64 item: icon, 4 dp, "Uzun" (26), together in the middle; the pill 16 dp round them, 40 tall.
    let (item, icon, label) = (
        bounds(&rule, "öğe"),
        rule.on_node_with_tag_unmerged("simge").get_unclipped_bounds_in_root(),
        text_bounds(&rule, "Uzun"),
    );
    assert!(same_rect(item, 0.0, 0.0, 200.0, 64.0), "{item:?}");
    let content_width = 24.0 + 4.0 + label.width();
    assert!(near(icon.left, ((200.0 - content_width) / 2.0).trunc(), 0.01) && near(icon.top, 20.0, 0.01), "{icon:?}");
    assert!(near(label.left, icon.right + 4.0, 0.01) && near(label.center().y, 32.0, 0.01), "{label:?}");
    let pills = rects_of(&rule.draw(), scheme.get().secondary_container);
    let pill_width = 16.0 + content_width + 16.0;
    assert!(
        pills.len() == 1
            && same_rect(pills[0].0, ((200.0 - pill_width) / 2.0).trunc(), 12.0, pill_width, 40.0)
            && near(pills[0].1, 20.0, 0.01),
        "{pills:?}"
    );
    let lone = rule.on_node_with_tag_unmerged("yalnız").get_unclipped_bounds_in_root();
    assert!(same_rect(lone, 288.0, 20.0, 24.0, 24.0), "{lone:?}");
}

fn navigation_bar_items_tell_accessibility_their_tab_role_and_selection() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            NavigationBar(
                content = || {
                    NavigationBarItem(
                        true,
                        || {},
                        || VectorIcon(Icons::Filled::Home(), Some("Ev simgesi".to_string())),
                        label = slot(|| Text("Ev")),
                    );
                    NavigationBarItem(
                        false,
                        || {},
                        || VectorIcon(Icons::Filled::Star(), Some("Yıldız".to_string())),
                        label = slot(|| Text("Gizli")),
                        always_show_label = false,
                    );
                    NavigationBarItem(
                        false,
                        || {},
                        icon = || VectorIcon(Icons::Filled::Favorite(), Some("Beğeni".to_string())),
                    );
                },
            );
        });
    }));
    let tree = rule.host().accessibility_tree();
    let tabs: Vec<_> = tree.iter().filter(|n| n.role == Some(Role::Tab)).collect();
    assert_eq!(tabs.len(), 3);
    assert_eq!(tabs.iter().map(|t| t.checked).collect::<Vec<_>>(), vec![Some(true), Some(false), Some(false)]);
    // A shown label silences the icon's description; a hidden one (unselected, not always shown) doesn't, nor no
    // label.
    assert_eq!(tabs[0].label, "Ev");
    assert!(tabs[1].label.contains("Yıldız"), "{:?}", tabs[1].label);
    assert_eq!(tabs[2].label, "Beğeni");
    assert!(tree.iter().any(|n| n.selectable_group));
}

// ---- The M15 samples -------------------------------------------------------------------------------------------

/// BadgeSamples.kt's `NavigationBarItemWithBadge` (left by WP1a for the navigation bar).
#[kompozable]
fn NavigationBarItemWithBadge() {
    NavigationBar(
        content = || {
            NavigationBarItem(
                icon = || {
                    BadgedBox(
                        badge = || {
                            Badge(modifier = Modifier.semantics(|c| c.set_content_description("New notification")));
                        },
                        content = || VectorIcon(Icons::Filled::Star(), Some("Favorite".to_string())),
                    );
                },
                selected = false,
                on_click = || {},
            );
            NavigationBarItem(
                icon = || {
                    BadgedBox(
                        badge = || {
                            Badge(
                                content = slot(|| {
                                    let badge_number = "8";
                                    Text(
                                        badge_number,
                                        modifier = Modifier.semantics(move |c| {
                                            c.set_content_description(format!("{badge_number} new notifications"))
                                        }),
                                    );
                                }),
                            );
                        },
                        content = || VectorIcon(Icons::Filled::Star(), Some("Favorite".to_string())),
                    );
                },
                selected = false,
                on_click = || {},
            );
            NavigationBarItem(
                icon = || {
                    BadgedBox(
                        badge = || {
                            Badge(
                                content = slot(|| {
                                    let badge_number = "999+";
                                    Text(
                                        badge_number,
                                        modifier = Modifier.semantics(move |c| {
                                            c.set_content_description(format!("{badge_number} new notifications"))
                                        }),
                                    );
                                }),
                            );
                        },
                        content = || VectorIcon(Icons::Filled::Star(), Some("Favorite".to_string())),
                    );
                },
                selected = false,
                on_click = || {},
            );
        },
    );
}

fn badged_navigation_bar_items() {
    let rule =
        KompozTestRule::with_viewport(kompozable_lambda!(|| NavigationBarItemWithBadge()), IntSize::new(600, 300));
    let tabs: Vec<_> = rule.host().accessibility_tree().into_iter().filter(|n| n.role == Some(Role::Tab)).collect();
    assert_eq!(tabs.len(), 3);
    // Each badge at the icon's top end, within the item's badge bounds; the texts told.
    let dot = named_args!(rule.on_node_with_content_description("New notification", use_unmerged_tree = true))
        .get_unclipped_bounds_in_root();
    let eight = named_args!(rule.on_node_with_text("8", use_unmerged_tree = true)).get_unclipped_bounds_in_root();
    let many = named_args!(rule.on_node_with_text("999+", use_unmerged_tree = true)).get_unclipped_bounds_in_root();
    assert!(near(dot.width(), 6.0, 0.01) && near(dot.height(), 6.0, 0.01), "{dot:?}");
    assert!(eight.height() > 6.0 && many.width() > eight.width(), "{eight:?} {many:?}");
    named_args!(rule.on_node_with_content_description("8 new notifications", use_unmerged_tree = true)).assert_exists();
    named_args!(rule.on_node_with_content_description("999+ new notifications", use_unmerged_tree = true))
        .assert_exists();
    let item_width = 600.0 / 3.0 - 8.0 * 2.0 / 3.0;
    // (each item's icon centered in its item, the badge past its end)
    assert!(dot.left > item_width / 2.0 && dot.left < item_width / 2.0 + 12.0 + 6.0, "{dot:?}");
    // The long count's badge: its start 12 dp in from the icon's end, its bottom 14 dp below the icon's top.
    let icons = named_args!(rule.on_all_nodes_with_content_description("Favorite", use_unmerged_tree = true))
        .fetch_semantics_nodes();
    assert_eq!(icons.len(), 3);
    let third = icons
        .iter()
        .map(|n| n.bounds_in_root())
        .fold(Rect::new(0.0, 0.0, 0.0, 0.0), |a, b| if b.left > a.left { b } else { a });
    let canvas = rule.draw();
    assert!(
        canvas.rects.iter().any(|r| near(r.left, third.right - 12.0, 0.01) && near(r.bottom, third.top + 14.0, 0.01)),
        "{third:?}"
    );
}

/// NavigationBarSamples.kt's `ShortNavigationBarSample`.
#[kompozable]
fn ShortNavigationBarSample() {
    let selected_item = remember(|| mutable_int_state_of(0));
    let items = ["Songs", "Artists", "Playlists"];
    let selected_icons = [Icons::Filled::Home(), Icons::Filled::Favorite(), Icons::Filled::Star()];
    let unselected_icons =
        [Icons::Outlined::Home(), Icons::Outlined::FavoriteBorder(), ExtendedIcons::Outlined::StarBorder()];

    ShortNavigationBar(
        content = move || {
            for (index, item) in items.into_iter().enumerate() {
                let index = index as i32;
                let (selected_icons, unselected_icons) = (selected_icons.clone(), unselected_icons.clone());
                let (s, s2) = (selected_item.clone(), selected_item.clone());
                ShortNavigationBarItem(
                    icon = move || {
                        VectorIcon(
                            if s.get() == index {
                                selected_icons[index as usize].clone()
                            } else {
                                unselected_icons[index as usize].clone()
                            },
                            None,
                        )
                    },
                    label = slot(move || Text(item)),
                    selected = selected_item.get() == index,
                    on_click = move || s2.set(index),
                );
            }
        },
    );
}

/// NavigationBarSamples.kt's `ShortNavigationBarWithHorizontalItemsSample`.
#[kompozable]
fn ShortNavigationBarWithHorizontalItemsSample() {
    let selected_item = remember(|| mutable_int_state_of(0));
    let items = ["Songs", "Artists", "Playlists"];
    let selected_icons = [Icons::Filled::Home(), Icons::Filled::Favorite(), Icons::Filled::Star()];
    let unselected_icons =
        [Icons::Outlined::Home(), Icons::Outlined::FavoriteBorder(), ExtendedIcons::Outlined::StarBorder()];

    Column(move || {
        Text(
            "Note: this is configuration is better displayed in medium screen sizes.",
            modifier = Modifier.padding(16.dp()),
        );

        Spacer(Modifier.height(32.dp()));

        let (selected_item, selected_icons, unselected_icons) =
            (selected_item.clone(), selected_icons.clone(), unselected_icons.clone());
        ShortNavigationBar(
            arrangement = ShortNavigationBarArrangement::Centered,
            content = move || {
                for (index, item) in items.into_iter().enumerate() {
                    let index = index as i32;
                    let (selected_icons, unselected_icons) = (selected_icons.clone(), unselected_icons.clone());
                    let (s, s2) = (selected_item.clone(), selected_item.clone());
                    ShortNavigationBarItem(
                        icon_position = NavigationItemIconPosition::Start,
                        icon = move || {
                            VectorIcon(
                                if s.get() == index {
                                    selected_icons[index as usize].clone()
                                } else {
                                    unselected_icons[index as usize].clone()
                                },
                                None,
                            )
                        },
                        label = slot(move || Text(item)),
                        selected = selected_item.get() == index,
                        on_click = move || s2.set(index),
                    );
                }
            },
        );
    });
}

/// NavigationBarSamples.kt's `NavigationBarSample`.
#[kompozable]
fn NavigationBarSample() {
    let selected_item = remember(|| mutable_int_state_of(0));
    let items = ["Songs", "Artists", "Playlists"];
    let selected_icons = [Icons::Filled::Home(), Icons::Filled::Favorite(), Icons::Filled::Star()];
    let unselected_icons =
        [Icons::Outlined::Home(), Icons::Outlined::FavoriteBorder(), ExtendedIcons::Outlined::StarBorder()];

    NavigationBar(
        content = move || {
            for (index, item) in items.into_iter().enumerate() {
                let index = index as i32;
                let (selected_icons, unselected_icons) = (selected_icons.clone(), unselected_icons.clone());
                let (s, s2) = (selected_item.clone(), selected_item.clone());
                NavigationBarItem(
                    icon = move || {
                        VectorIcon(
                            if s.get() == index {
                                selected_icons[index as usize].clone()
                            } else {
                                unselected_icons[index as usize].clone()
                            },
                            Some(item.to_string()),
                        )
                    },
                    label = slot(move || Text(item)),
                    selected = selected_item.get() == index,
                    on_click = move || s2.set(index),
                );
            }
        },
    );
}

/// A sample's three items: Songs picked first, Playlists once clicked (the icons swapped: its outlined star for
/// the filled one), one picked at a time.
fn check_sample(name: &str, sample: impl Content + 'static, viewport: IntSize) -> KompozTestRule {
    let rule = KompozTestRule::with_viewport(sample, viewport);
    let selection = |rule: &KompozTestRule| -> Vec<Option<bool>> {
        rule.host().accessibility_tree().iter().filter(|n| n.role == Some(Role::Tab)).map(|n| n.checked).collect()
    };
    assert_eq!(selection(&rule), vec![Some(true), Some(false), Some(false)], "{name}");
    let icons = |rule: &KompozTestRule| rule.draw().vectors.clone();
    let before = icons(&rule);
    rule.on_node_with_text("Playlists").perform_click();
    rule.wait_for_idle();
    assert_eq!(selection(&rule), vec![Some(false), Some(false), Some(true)], "{name}");
    assert_ne!(icons(&rule), before, "{name}: the icons swapped");
    rule
}

fn navigation_bar_samples() {
    let rule = check_sample(
        "ShortNavigationBarSample",
        kompozable_lambda!(|| ShortNavigationBarSample()),
        IntSize::new(360, 640),
    );
    assert!(near(rule.on_node_with_text("Songs").get_unclipped_bounds_in_root().width(), 120.0, 0.01));
    let rule = check_sample(
        "ShortNavigationBarWithHorizontalItemsSample",
        kompozable_lambda!(|| ShortNavigationBarWithHorizontalItemsSample()),
        IntSize::new(700, 400),
    );
    // (centered, the icons at the start of their labels: items 64 dp tall, narrower than a third)
    let songs = rule.on_node_with_text("Songs").get_unclipped_bounds_in_root();
    assert!(songs.left > 0.0 && songs.width() < 700.0 / 3.0 && near(songs.height(), 64.0, 0.01), "{songs:?}");
    let rule =
        check_sample("NavigationBarSample", kompozable_lambda!(|| NavigationBarSample()), IntSize::new(400, 300));
    // (the labels shown: the icons' descriptions silent)
    let tabs: Vec<_> = rule.host().accessibility_tree().into_iter().filter(|n| n.role == Some(Role::Tab)).collect();
    assert_eq!(tabs.iter().map(|t| t.label.as_str()).collect::<Vec<_>>(), vec!["Songs", "Artists", "Playlists"]);
}
