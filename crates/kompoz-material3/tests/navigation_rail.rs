//! Material's navigation rails as Compose's material3 (1.5.0-alpha17) has them: the C++ Kompoz's tests by name
//! (kompoz_tests.cpp), Compose's behaviour (the item's layouts with and without labels, the header, the insets,
//! the override, the colors, accessibility), and the M15 samples of NavigationRailSamples.kt.

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_coroutines::{CoroutineScope, LaunchedEffect, remember_coroutine_scope};
use kompoz_foundation::interaction::{Interaction, MutableInteractionSource, PressInteraction};
use kompoz_foundation::layout::WindowInsets;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas, has_text};
use kompoz_ui::input::{Key, KeyModifiers, PointerType};
use kompoz_ui::platform::LocalWindowInfo;
use kompoz_ui::semantics::{Role, SemanticsProperties};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

tests!(
    navigation_rail_as_compose_1_4,
    window_size_local_and_navigation_rail,
    navigation_rail_items_take_their_options,
    navigation_rail_items_without_labels_center_their_icons_in_a_round_indicator,
    labels_shown_only_when_selected_come_in_with_the_indicator,
    navigation_rail_keeps_clear_of_its_insets_and_fills_the_height,
    navigation_rail_items_tell_accessibility_their_tab_role_and_selection,
    navigation_rail_override_makes_the_rails_below,
    navigation_rail_item_colors_follow_the_tokens_and_copy,
    wide_navigation_rail_widens_and_puts_the_labels_after_the_icons,
    modal_wide_navigation_rail_opens_over_the_content_with_a_scrim,
    wide_navigation_rail_defaults_colors_and_override,
    navigation_rail_samples,
    wide_navigation_rail_samples,
    modal_wide_navigation_rail_samples,
    navigation_items_animate_composing_nothing_around_them,
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

/// A node's bounds in the unmerged tree (an item's parts: merged into the item).
fn part_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
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

/// The rects drawn in `color`.
fn rects_of(canvas: &RecordingCanvas, color: Color) -> Vec<Rect> {
    canvas.rects.iter().zip(&canvas.rect_colors).filter(|(_, c)| **c == color).map(|(r, _)| *r).collect()
}

fn tap(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(at, -1, 0);
    rule.advance_frame();
}

type Log = Rc<RefCell<Vec<String>>>;

/// C++'s LogInteractions: what `source` emits (presses and releases by name), logged while it is composed.
#[kompozable(inline)]
fn LogInteractions(source: MutableInteractionSource, log: Log) {
    let flow = source.interactions();
    LaunchedEffect(source.clone(), async move {
        flow.collect(move |i: std::sync::Arc<dyn Interaction>| {
            let name = if i.is::<PressInteraction::Press>() {
                "Press"
            } else if i.is::<PressInteraction::Release>() {
                "Release"
            } else if i.is::<PressInteraction::Cancel>() {
                "Cancel"
            } else {
                "?"
            };
            log.borrow_mut().push(name.to_string());
        })
        .await;
    });
}

// ---- The C++ Kompoz's tests ------------------------------------------------------------------------------------

fn navigation_rail_as_compose_1_4() {
    // (M15's values are the C++'s here: 80 dp wide, 56 dp items 4 dp apart, the 56 × 32 pill, the label 36 dp
    // down.)
    let tab = mutable_state_of(0);
    let scheme = Rc::new(Cell::new(light_color_scheme()));
    let (t, sc) = (tab.clone(), scheme.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            sc.set(MaterialTheme::color_scheme());
            let t = t.clone();
            NavigationRail(
                modifier = Modifier.test_tag("ray"),
                header = slot(|| Box(modifier = Modifier.size(40.dp()).test_tag("baş"))),
                content = move || {
                    let (t0, t1) = (t.clone(), t.clone());
                    NavigationRailItem(
                        t.get() == 0,
                        move || t0.set(0),
                        || VectorIcon(Icons::Filled::Home(), None),
                        label = slot(|| Text("Bir")),
                    );
                    NavigationRailItem(
                        t.get() == 1,
                        move || t1.set(1),
                        || VectorIcon(Icons::AutoMirrored::Filled::List(), None),
                        label = slot(|| Text("İki")),
                    );
                },
            );
        }),
        IntSize::new(400, 600),
    );
    let cs = scheme.get();
    // 4 dp above, the header, 8 under it; 80 × 56 items 4 dp apart, the label 36 dp down.
    let (rail, header) = (bounds(&rule, "ray"), part_bounds(&rule, "baş"));
    let (first, second) = (text_bounds(&rule, "Bir"), text_bounds(&rule, "İki"));
    assert!(near(rail.width(), 80.0, 0.01) && near(header.top, rail.top + 4.0, 0.01), "{rail:?} {header:?}");
    let first_top = header.top + 40.0 + 4.0 + 8.0 + 4.0;
    assert!(near(first.top, first_top + 36.0, 0.01), "{first:?}");
    assert!(near(second.top, first_top + 56.0 + 4.0 + 36.0, 0.01), "{second:?}");
    let canvas = rule.draw();
    let indicator = rects_of(&canvas, cs.secondary_container).into_iter().any(|r| {
        near(r.width(), 56.0, 0.01)
            && near(r.height(), 32.0, 0.01)
            && near(r.top, first_top, 0.01)
            && near(r.center().x, rail.center().x, 0.01)
    });
    assert!(indicator, "{:?}", canvas.rects);
    assert_eq!(color_of(&canvas, "Bir"), Some(cs.secondary));

    // Picked: it grows in; the ripple on its indicator starts under the finger.
    tab.set(1);
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    let growing = rule.draw();
    let part = rects_of(&growing, cs.secondary_container).into_iter().any(|r| r.width() > 4.0 && r.width() < 52.0);
    assert!(part, "{:?}", growing.rects);
    rule.wait_for_idle();
    let at = Offset::new(rail.center().x + 15.0, first_top + 20.0);
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.advance_frame();
    let pressed = rule.draw();
    let from_finger = pressed.rects.iter().zip(&pressed.rect_colors).any(|(r, c)| {
        near(r.width(), r.height(), 0.01)
            && c.alpha() > 0.0
            && c.alpha() < 0.2
            && near(r.center().x, at.x, 3.0)
            && near(r.center().y, at.y, 3.0)
    });
    assert!(from_finger, "{:?} {:?}", pressed.rects, pressed.rect_colors);
    rule.host().pointer_up(at, -1, 0);
    rule.wait_for_idle();
    assert_eq!(tab.get(), 0);
}

fn window_size_local_and_navigation_rail() {
    // C++'s LocalWindowSize is Compose's LocalWindowInfo.containerSize (px; density 1 here). The window-size half
    // (the content following the window's size) was ported nowhere else: the whole test is here.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            let window = LocalWindowInfo.current().container_size();
            if window.width >= 600 {
                Row(
                    modifier = Modifier.fill_max_size(),
                    content = || {
                        NavigationRail(
                            modifier = Modifier.test_tag("rail"),
                            content = || {
                                NavigationRailItem(
                                    true,
                                    || {},
                                    || VectorIcon(Icons::Filled::Home(), None),
                                    label = slot(|| Text("Ana")),
                                );
                                NavigationRailItem(
                                    false,
                                    || {},
                                    || VectorIcon(Icons::Filled::Star(), None),
                                    label = slot(|| Text("Yıldız")),
                                );
                            },
                        );
                        Text("geniş");
                    },
                );
            } else {
                Text("dar");
            }
        }),
        IntSize::new(400, 800),
    );
    rule.on_node_with_text("dar").assert_exists();
    rule.host().set_viewport_size(IntSize::new(800, 600));
    rule.wait_for_idle();
    rule.on_node_with_text("geniş").assert_exists();
    assert!(near(bounds(&rule, "rail").width(), 80.0, 0.01));
    assert!(text_bounds(&rule, "Yıldız").top > text_bounds(&rule, "Ana").top);
}

fn navigation_rail_items_take_their_options() {
    let own = MutableInteractionSource::new();
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let clicks = Rc::new(Cell::new(0));
    let scheme = Rc::new(Cell::new(light_color_scheme()));
    let (o, l, c, sc) = (own.clone(), log.clone(), clicks.clone(), scheme.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            sc.set(MaterialTheme::color_scheme());
            LogInteractions(o.clone(), l.clone());
            let (o, c) = (o.clone(), c.clone());
            NavigationRail(
                content = move || {
                    let c = c.clone();
                    NavigationRailItem(
                        false,
                        move || c.set(c.get() + 1),
                        || VectorIcon(Icons::Filled::Home(), Some("ev".into()), modifier = Modifier.test_tag("simge")),
                        modifier = Modifier.test_tag("kapalı"),
                        label = slot(|| Text("Kapalı")),
                        enabled = false,
                    );
                    NavigationRailItem(
                        false,
                        || {},
                        || VectorIcon(Icons::AutoMirrored::Filled::List(), Some("liste".into())),
                        modifier = Modifier.test_tag("kendi"),
                        label = slot(|| Text("Kendi")),
                        interaction_source = Some(o.clone()),
                    );
                },
            );
        }),
        IntSize::new(400, 600),
    );
    let cs = scheme.get();
    // The icon 4 dp down the item; disabled: on-surface-variant at 38% (M15; the C++ said on-surface: the same
    // alpha), and a tap does nothing.
    let off = bounds(&rule, "kapalı");
    assert!(near(part_bounds(&rule, "simge").top, off.top + 4.0, 0.01));
    let label = color_of(&rule.draw(), "Kapalı").expect("the label");
    assert!(near(label.alpha(), cs.on_surface_variant.alpha() * 0.38, 0.01), "{label:?}");
    assert_eq!(named_args!(label.copy(alpha = 1.0)), cs.on_surface_variant);
    tap(&rule, off.center());
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 0);
    // Its own interaction source hears its presses.
    tap(&rule, bounds(&rule, "kendi").center());
    rule.wait_for_idle();
    assert_eq!(log.borrow().first().map(String::as_str), Some("Press"), "{:?}", log.borrow());
}

// ---- Compose's behaviour ----------------------------------------------------------------------------------------

fn navigation_rail_items_without_labels_center_their_icons_in_a_round_indicator() {
    // Without a label the item is 56 dp tall, the icon in its middle, the indicator 56 dp round (16 dp round the
    // 24 dp icon each way) behind it.
    let scheme = Rc::new(Cell::new(light_color_scheme()));
    let sc = scheme.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        sc.set(MaterialTheme::color_scheme());
        NavigationRail(
            content = || {
                NavigationRailItem(
                    true,
                    || {},
                    || VectorIcon(Icons::Filled::Home(), Some("ev".into()), modifier = Modifier.test_tag("simge")),
                    modifier = Modifier.test_tag("öğe"),
                );
            },
        );
    }));
    let (item, icon) = (bounds(&rule, "öğe"), part_bounds(&rule, "simge"));
    assert!(near(item.width(), 80.0, 0.01) && near(item.height(), 56.0, 0.01), "{item:?}");
    assert!(near(icon.center().x, item.center().x, 0.01) && near(icon.center().y, item.center().y, 0.01));
    let canvas = rule.draw();
    let pill = rects_of(&canvas, scheme.get().secondary_container);
    assert!(
        pill.iter().any(|r| near(r.width(), 56.0, 0.01)
            && near(r.height(), 56.0, 0.01)
            && near(r.center().y, item.center().y, 0.01)),
        "{pill:?}"
    );
    // The icon is what accessibility hears (no label stands for it).
    rule.on_node_with_tag("öğe").assert(kompoz_test::has_content_description("ev", false, false));
}

fn labels_shown_only_when_selected_come_in_with_the_indicator() {
    // `always_show_label = false`: unselected, the icon alone in the middle and the label not placed; selected, the
    // icon at the top and the label under it, as with labels always shown.
    let selected = mutable_state_of(false);
    let s = selected.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        NavigationRail(
            content = move || {
                NavigationRailItem(
                    s.get(),
                    || {},
                    || VectorIcon(Icons::Filled::Home(), None, modifier = Modifier.test_tag("simge")),
                    modifier = Modifier.test_tag("öğe"),
                    label = slot(|| Text("Ev")),
                    always_show_label = false,
                );
            },
        );
    }));
    let item = bounds(&rule, "öğe");
    let icon = part_bounds(&rule, "simge");
    assert!(near(item.height(), 56.0, 0.01) && near(icon.center().y, item.center().y, 0.01), "{item:?} {icon:?}");
    assert!(!rule.drawn_texts().contains(&"Ev".to_string()));
    selected.set(true);
    rule.wait_for_idle();
    assert!(near(part_bounds(&rule, "simge").top, item.top + 4.0, 0.01));
    assert!(near(text_bounds(&rule, "Ev").top, item.top + 36.0, 0.01));
    assert!(rule.drawn_texts().contains(&"Ev".to_string()));
}

fn navigation_rail_keeps_clear_of_its_insets_and_fills_the_height() {
    // The rail is as tall as it may be; its column padded by the insets given (and 4 dp), at least 80 dp wide.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            NavigationRail(
                modifier = Modifier.test_tag("ray"),
                window_insets = WindowInsets(left = 10.dp(), top = 20.dp(), bottom = 5.dp()),
                content = || {
                    NavigationRailItem(
                        false,
                        || {},
                        || VectorIcon(Icons::Filled::Home(), None),
                        modifier = Modifier.test_tag("öğe"),
                    );
                },
            );
        }),
        IntSize::new(400, 600),
    );
    let (rail, item) = (bounds(&rule, "ray"), bounds(&rule, "öğe"));
    assert!(near(rail.height(), 600.0, 0.01) && near(rail.width(), 90.0, 0.01), "{rail:?}");
    assert!(near(item.left, 10.0, 0.01) && near(item.top, 24.0, 0.01), "{item:?}");
    // By default the system bars' (none in a test).
    let top = Rc::new(Cell::new(-1));
    let t = top.clone();
    let _defaults = KompozTestRule::new(kompozable_lambda!(move || {
        t.set(NavigationRailDefaults::window_insets().get_top(&Density(1.0)));
    }));
    assert_eq!(top.get(), 0);
}

fn navigation_rail_items_tell_accessibility_their_tab_role_and_selection() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        NavigationRail(
            modifier = Modifier.test_tag("ray"),
            content = || {
                NavigationRailItem(
                    true,
                    || {},
                    || VectorIcon(Icons::Filled::Home(), Some("ev".into())),
                    modifier = Modifier.test_tag("bir"),
                    label = slot(|| Text("Bir")),
                );
                NavigationRailItem(
                    false,
                    || {},
                    || VectorIcon(Icons::Filled::Star(), Some("yıldız".into())),
                    modifier = Modifier.test_tag("iki"),
                    label = slot(|| Text("İki")),
                );
            },
        );
    }));
    let tabs: Vec<_> =
        rule.all_nodes(false).into_iter().filter(|n| kompoz_test::has_role(Role::Tab).matches(n)).collect();
    assert_eq!(tabs.len(), 2);
    rule.on_node_with_tag("bir").assert(kompoz_test::is_selected());
    rule.on_node_with_tag("iki").assert(kompoz_test::is_not_selected());
    // The label stands for the icon: its description isn't heard.
    rule.on_node_with_tag("bir").assert(kompoz_test::has_text("Bir", false, false));
    rule.on_node_with_tag("bir").assert(!kompoz_test::has_content_description("ev", false, false));
    // The column is a selectable group and a traversal group.
    let rail = rule.on_node_with_tag("ray").on_child().fetch_semantics_node();
    assert_eq!(rail.config().get(SemanticsProperties::IsTraversalGroup), Some(true));
}

/// An override drawing the rail as a plain column of its items under its header, tagged.
struct TaggedRail;

impl NavigationRailOverride for TaggedRail {
    #[track_caller]
    fn NavigationRail(&self, scope: &NavigationRailOverrideScope) {
        let (header, content) = (scope.header(), scope.content());
        let red = scope.container_color() == Color::Red;
        kompozable_block!({
            Column(
                modifier = Modifier.test_tag("kendi"),
                content = move || {
                    Text(format!("{red}"));
                    if let Some(header) = header.clone() {
                        header();
                    }
                    content();
                },
            );
        })
    }
}

fn navigation_rail_override_makes_the_rails_below() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        KompozitionLocalProvider(&[LocalNavigationRailOverride.provides(Rc::new(TaggedRail))], || {
            NavigationRail(container_color = Color::Red, header = slot(|| Text("baş")), content = || Text("öğeler"));
        });
    }));
    rule.on_node_with_tag("kendi").assert_exists();
    rule.on_node_with_text("true").assert_exists();
    assert!(text_bounds(&rule, "öğeler").top > text_bounds(&rule, "baş").top);
}

fn navigation_rail_item_colors_follow_the_tokens_and_copy() {
    let colors = Rc::new(RefCell::new(Vec::new()));
    let scheme = Rc::new(Cell::new(light_color_scheme()));
    let (c, sc) = (colors.clone(), scheme.clone());
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        sc.set(MaterialTheme::color_scheme());
        c.borrow_mut().push(NavigationRailItemDefaults.colors());
        c.borrow_mut().push(NavigationRailItemDefaults.colors(unselected_icon_color = Color::Red));
        c.borrow_mut().push(
            NavigationRailItemDefaults
                .colors(unselected_text_color = Color::Blue, disabled_text_color = Some(Color::Green)),
        );
        assert_eq!(NavigationRailDefaults::ContainerColor(), MaterialTheme::color_scheme().surface);
    }));
    let cs = scheme.get();
    let colors = colors.borrow();
    let defaults = colors[0];
    assert_eq!(
        defaults,
        NavigationRailItemColors::new(
            cs.on_secondary_container,
            cs.secondary,
            cs.secondary_container,
            cs.on_surface_variant,
            cs.on_surface_variant,
            named_args!(cs.on_surface_variant.copy(alpha = 0.38)),
            named_args!(cs.on_surface_variant.copy(alpha = 0.38)),
        )
    );
    // A disabled color not given follows the unselected one given.
    assert_eq!(colors[1].disabled_icon_color, named_args!(Color::Red.copy(alpha = 0.38)));
    assert_eq!(colors[2].disabled_text_color, Color::Green);
    assert_eq!(colors[2].disabled_icon_color, defaults.disabled_icon_color);
    // copy: an unspecified color keeps its own.
    let copied = named_args!(defaults.copy(selected_icon_color = Color::Red, selected_text_color = Color::Unspecified));
    assert_eq!(copied.selected_icon_color, Color::Red);
    assert_eq!(copied.selected_text_color, cs.secondary);
    assert_eq!(copied.unselected_icon_color, cs.on_surface_variant);
}

// ---- Wide navigation rails --------------------------------------------------------------------------------------

type Slot<T> = Rc<RefCell<Option<T>>>;

fn new_slot<T>() -> Slot<T> {
    Rc::new(RefCell::new(None))
}

fn get<T: Clone>(s: &Slot<T>) -> T {
    s.borrow().clone().expect("set by the composition")
}

/// Runs `f` in the composition's coroutine scope (a suspending call of the rail's state).
fn launch(scope: &Slot<CoroutineScope>, f: impl std::future::Future<Output = ()> + 'static) {
    named_args!(get(scope).launch(f));
}

/// The pills drawn in `color`, with their top left corner radii.
fn pills(canvas: &RecordingCanvas, color: Color) -> Vec<(Rect, f32)> {
    (0..canvas.rects.len())
        .filter(|&i| canvas.rect_colors[i] == color)
        .map(|i| (canvas.rects[i], canvas.rect_radii[i]))
        .collect()
}

fn same_rect(r: Rect, left: f32, top: f32, width: f32, height: f32) -> bool {
    near(r.left, left, 0.5) && near(r.top, top, 0.5) && near(r.width(), width, 0.5) && near(r.height(), height, 0.5)
}

fn wide_navigation_rail_widens_and_puts_the_labels_after_the_icons() {
    let rail: Slot<Rc<dyn WideNavigationRailState>> = new_slot();
    let scope: Slot<CoroutineScope> = new_slot();
    let selected = mutable_state_of(0);
    let (r, sc, s) = (rail.clone(), scope.clone(), selected.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = remember_wide_navigation_rail_state();
            *r.borrow_mut() = Some(state.clone());
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let expanded = state.target_value() == WideNavigationRailValue::Expanded;
            let s = s.clone();
            WideNavigationRail(
                modifier = Modifier.test_tag("ray"),
                state = Some(state),
                header = slot(|| Box(modifier = Modifier.size(40.dp()).test_tag("başlık"))),
                window_insets = WindowInsets(),
                content = move || {
                    for (i, label) in ["Gelen", "Giden"].into_iter().enumerate() {
                        let s2 = s.clone();
                        WideNavigationRailItem(
                            s.get() == i,
                            move || s2.set(i),
                            || Box(modifier = Modifier.size(24.dp())),
                            slot(move || Text(label)),
                            expanded,
                            modifier = Modifier.test_tag(label),
                        );
                    }
                },
            );
        }),
        IntSize::new(400, 600),
    );
    let cs = light_color_scheme();
    let at = |tag: &str| bounds(&rule, tag);
    // Collapsed: 96 dp, 44 dp down the header, each icon above its label. (M15 places the header at the rail's
    // start, x 0: the C++ centered it, at 28.)
    assert!(same_rect(at("ray"), 0.0, 0.0, 96.0, 600.0), "{:?}", at("ray"));
    assert!(same_rect(part_bounds(&rule, "başlık"), 0.0, 44.0, 40.0, 40.0), "{:?}", part_bounds(&rule, "başlık"));
    assert!(same_rect(at("Gelen"), 0.0, 124.0, 96.0, 64.0), "{:?}", at("Gelen"));
    assert!(same_rect(at("Giden"), 0.0, 192.0, 96.0, 64.0), "{:?}", at("Giden"));
    let collapsed = pills(&rule.draw(), cs.secondary_container);
    assert!(
        collapsed.len() == 1 && same_rect(collapsed[0].0, 20.0, 130.0, 56.0, 32.0) && near(collapsed[0].1, 16.0, 0.5),
        "{collapsed:?}"
    ); // 6 dp down
    rule.on_node_with_tag("Gelen").assert(kompoz_test::is_selected());

    // (the clock by hand: the check on the way sees the rail widening)
    rule.main_clock().set_auto_advance(false);
    let state = get(&rail);
    let st = state.clone();
    launch(&scope, async move { st.expand().await });
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    let widening = at("ray").width();
    assert!(widening > 97.0 && widening < 219.0, "{widening}"); // on its way
    assert_eq!(state.target_value(), WideNavigationRailValue::Expanded);
    finish_animations(&rule);
    // Expanded: 220 dp, items 56 dp tall from 20 dp in, the label after the icon and the indicator round both.
    // (M15's expanded item is as wide as its indicator and 20 dp, 120 here — `widthStartIcon`; the C++'s items
    // took the rail's whole width.)
    assert!(same_rect(at("ray"), 0.0, 0.0, 220.0, 600.0), "{:?}", at("ray"));
    assert!(near(part_bounds(&rule, "başlık").left, 0.0, 0.5));
    assert!(same_rect(at("Gelen"), 0.0, 124.0, 120.0, 56.0), "{:?}", at("Gelen"));
    assert!(same_rect(at("Giden"), 0.0, 180.0, 120.0, 56.0), "{:?}", at("Giden"));
    let open = pills(&rule.draw(), cs.secondary_container);
    // (16 + 24 + 8 + the label's 36 — 35.5 rounded up — + 16)
    assert!(
        open.len() == 1
            && same_rect(open[0].0, 20.0, 124.0, 16.0 + 24.0 + 8.0 + 36.0 + 16.0, 56.0)
            && near(open[0].1, 28.0, 0.5),
        "{open:?}"
    );

    tap(&rule, at("Giden").center());
    finish_animations(&rule);
    let after = pills(&rule.draw(), cs.secondary_container);
    assert!(selected.get() == 1 && after.len() == 1 && near(after[0].0.top, 180.0, 0.5), "{after:?}");
    let st = state.clone();
    launch(&scope, async move { st.toggle().await });
    finish_animations(&rule);
    assert_eq!(state.current_value(), WideNavigationRailValue::Collapsed);
    assert!(same_rect(at("ray"), 0.0, 0.0, 96.0, 600.0) && same_rect(at("Giden"), 0.0, 192.0, 96.0, 64.0));
}

fn modal_wide_navigation_rail_opens_over_the_content_with_a_scrim() {
    let rail: Slot<Rc<dyn WideNavigationRailState>> = new_slot();
    let scope: Slot<CoroutineScope> = new_slot();
    let picked = mutable_state_of(0);
    let hide = mutable_state_of(false);
    let (r, sc, p, h) = (rail.clone(), scope.clone(), picked.clone(), hide.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = remember_wide_navigation_rail_state();
            *r.borrow_mut() = Some(state.clone());
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let expanded = state.target_value() == WideNavigationRailValue::Expanded;
            let (p, h) = (p.clone(), h.clone());
            Row(
                modifier = Modifier.fill_max_size(),
                content = move || {
                    let p = p.clone();
                    ModalWideNavigationRail(
                        state = Some(state.clone()),
                        hide_on_collapse = h.get(),
                        window_insets = WindowInsets(),
                        content = move || {
                            for (i, (label, tag)) in [("Gelen", "gelen"), ("Giden", "giden")].into_iter().enumerate() {
                                let p2 = p.clone();
                                WideNavigationRailItem(
                                    p.get() == i,
                                    move || p2.set(i),
                                    || Box(modifier = Modifier.size(24.dp())),
                                    slot(move || Text(label)),
                                    expanded,
                                    modifier = Modifier.test_tag(tag),
                                );
                            }
                        },
                    );
                    Box(modifier = Modifier.weight(1.0).fill_max_height().test_tag("içerik"));
                },
            );
        }),
        IntSize::new(400, 600),
    );
    // (M15's expanded modal rail is rounded all round, the theme's large corners: the C++ rounded its end only.)
    let cs = light_color_scheme();
    let items = || -> Vec<Rect> {
        rule.on_all_nodes_with_tag("gelen")
            .fetch_semantics_nodes()
            .iter()
            .map(|n| n.unclipped_bounds_in_root())
            .collect()
    };
    // Collapsed in place.
    let collapsed = items();
    assert!(collapsed.len() == 1 && near(collapsed[0].width(), 96.0, 0.5), "{collapsed:?}");
    assert!(near(bounds(&rule, "içerik").left, 96.0, 0.5));

    let state = get(&rail);
    let st = state.clone();
    launch(&scope, async move { st.expand().await });
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    let opening = items();
    assert!(!opening.is_empty(), "{opening:?}");
    rule.wait_for_idle();
    let canvas = rule.draw();
    let scrim = canvas
        .rects
        .iter()
        .zip(&canvas.rect_colors)
        .any(|(r, c)| same_rect(*r, 0.0, 0.0, 400.0, 600.0) && near(c.alpha(), 0.32, 0.01));
    let sheet = (0..canvas.rects.len()).any(|i| {
        same_rect(canvas.rects[i], 0.0, 0.0, 220.0, 600.0)
            && canvas.rect_colors[i] == cs.surface_container
            && near(canvas.rect_corners[i].top_right, 16.0, 0.5)
            && near(canvas.rect_corners[i].top_left, 16.0, 0.5)
    });
    assert!(scrim && sheet, "{:?} {:?}", canvas.rects, canvas.rect_colors);
    let open = items();
    assert!(open.len() == 1 && near(open[0].width(), 120.0, 0.5), "{open:?}"); // (M15: the item as wide as its indicator and 20 dp)
    assert!(near(bounds(&rule, "içerik").left, 96.0, 0.5)); // the content doesn't move

    tap(&rule, Offset::new(350.0, 300.0)); // the scrim: collapsed
    rule.wait_for_idle();
    assert_eq!(state.current_value(), WideNavigationRailValue::Collapsed);
    assert_eq!(items().len(), 1);
    let st = state.clone();
    launch(&scope, async move { st.expand().await });
    rule.wait_for_idle();
    assert!(rule.host().key_down(Key::Escape, KeyModifiers::default())); // back too
    rule.wait_for_idle();
    assert_eq!(state.current_value(), WideNavigationRailValue::Collapsed);
    assert_eq!(items().len(), 1);

    hide.set(true); // nothing in place while collapsed
    rule.wait_for_idle();
    assert!(items().is_empty() && near(bounds(&rule, "içerik").left, 0.0, 0.5));
    let st = state.clone();
    launch(&scope, async move { st.expand().await });
    rule.wait_for_idle();
    let shown = items();
    assert!(shown.len() == 1 && near(shown[0].width(), 120.0, 0.5), "{shown:?}");
    tap(&rule, Offset::new(shown[0].center().x, 500.0)); // on the rail but not an item: stays open
    rule.wait_for_idle();
    assert_eq!(state.current_value(), WideNavigationRailValue::Expanded);
}

/// An override drawing the wide rail as a plain column, tagged, telling whether its state is expanded.
struct TaggedWideRail;

impl WideNavigationRailOverride for TaggedWideRail {
    #[track_caller]
    fn WideNavigationRail(&self, scope: &WideNavigationRailOverrideScope) {
        let (content, expanded) = (scope.content(), scope.state().target_value() == WideNavigationRailValue::Expanded);
        kompozable_block!({
            Column(
                modifier = Modifier.test_tag("kendi"),
                content = move || {
                    Text(format!("açık: {expanded}"));
                    content();
                },
            );
        })
    }
}

fn wide_navigation_rail_defaults_colors_and_override() {
    let colors = Rc::new(RefCell::new(Vec::new()));
    let item_colors = Rc::new(RefCell::new(Vec::new()));
    let paddings = Rc::new(RefCell::new(Vec::new()));
    let (c, ic, p) = (colors.clone(), item_colors.clone(), paddings.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        c.borrow_mut().push(WideNavigationRailDefaults.colors());
        c.borrow_mut().push(WideNavigationRailDefaults.colors(container_color = Color::Black));
        ic.borrow_mut().push(WideNavigationRailItemDefaults.colors());
        ic.borrow_mut().push(WideNavigationRailItemDefaults.colors(unselected_text_color = Color::Red));
        p.borrow_mut().push(WideNavigationRailItemDefaults.indicator_padding(false));
        p.borrow_mut().push(WideNavigationRailItemDefaults.indicator_padding(true));
        KompozitionLocalProvider(&[LocalWideNavigationRailOverride.provides(Rc::new(TaggedWideRail))], || {
            let state = remember_wide_navigation_rail_state(WideNavigationRailValue::Expanded);
            WideNavigationRail(state = Some(state), content = || Text("öğeler"));
        });
    }));
    let cs = light_color_scheme();
    let colors = colors.borrow();
    // The surface and its content, the modal rail on the surface container, the scrim at 32%.
    assert_eq!(
        colors[0],
        WideNavigationRailColors::new(
            cs.surface,
            cs.on_surface,
            cs.surface_container,
            named_args!(cs.scrim.copy(alpha = 0.32)),
            cs.on_surface,
        )
    );
    // A container given: its content color follows (white on black: none of the scheme's — the content color
    // around).
    assert_eq!(colors[1].container_color, Color::Black);
    assert_eq!(colors[1].modal_container_color, cs.surface_container);
    // Compose's equals leaves the modal content color out.
    assert_eq!(colors[0], named_args!(colors[0].copy(modal_content_color = Color::Red)));
    assert_eq!(
        WideNavigationRailColors::without_modal_content_color(Color::Red, Color::Blue, Color::Green, Color::Black)
            .modal_content_color,
        Color::Blue
    );
    let item_colors = item_colors.borrow();
    assert_eq!(item_colors[0].selected_indicator_color, cs.secondary_container);
    assert_eq!(item_colors[1].disabled_text_color, named_args!(Color::Red.copy(alpha = 0.38)));
    // The indicator paddings: 16 × 4 dp round a top icon, 16 dp round a start icon and its label.
    let paddings = paddings.borrow();
    assert_eq!(paddings[0].calculate_top_padding(), Dp(4.0));
    assert_eq!(paddings[1].calculate_top_padding(), Dp(16.0));
    assert_eq!(paddings[1].calculate_left_padding(LayoutDirection::Ltr), Dp(16.0));
    assert_eq!(WideNavigationRailItemDefaults::icon_position_for(true), NavigationItemIconPosition::Start);
    assert_eq!(WideNavigationRailItemDefaults::icon_position_for(false), NavigationItemIconPosition::Top);
    assert_eq!(WideNavigationRailDefaults::ModalExpandedProperties, ModalWideNavigationRailProperties::default());
    assert_eq!(WideNavigationRailDefaults::ContentPadding().calculate_top_padding(), Dp(44.0));
    // The override makes the rail.
    rule.on_node_with_tag("kendi").assert_exists();
    rule.on_node_with_text("açık: true").assert_exists();
}

// ---- M15's NavigationRailSamples ------------------------------------------------------------------------------

use kompoz_material_icons_extended::Icons as ExtendedIcons;

const SampleItems: [&str; 3] = ["Home", "Search", "Settings"];

/// The samples' icons: the selected one filled, the others outlined (`selectedIcons` / `unselectedIcons`).
fn sample_icon(index: usize, selected: bool) -> ImageVector {
    match (index, selected) {
        (0, true) => Icons::Filled::Home(),
        (1, true) => Icons::Filled::Favorite(),
        (_, true) => Icons::Filled::Star(),
        (0, false) => Icons::Outlined::Home(),
        (1, false) => Icons::Outlined::FavoriteBorder(),
        (_, false) => ExtendedIcons::Outlined::StarBorder(),
    }
}

/// The header the responsive, modal and arrangements samples write: an icon button (in a tooltip box) expanding or
/// collapsing the rail, telling the rail's state, its icon MenuOpen or Menu.
#[kompozable]
fn SampleRailHeader(state: Rc<dyn WideNavigationRailState>) {
    let scope = remember_coroutine_scope();
    let expanded = state.target_value() == WideNavigationRailValue::Expanded;
    let header_description = if expanded { "Collapse rail" } else { "Expand rail" };
    // Header icon button should have a tooltip.
    TooltipBox(
        position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
        tooltip = move |tooltip: &dyn TooltipScope| tooltip.PlainTooltip(content = move || Text(header_description)),
        state = remember_tooltip_state(),
        content = move || {
            let (state, description_state, scope) = (state.clone(), state.clone(), scope.clone());
            IconButton(
                modifier = named_args!(Modifier.padding_sides(start = 24.dp())).semantics_with(false, move |c| {
                    // The button must announce the expanded or collapsed state of the
                    // rail for accessibility.
                    c.set_state_description(
                        if description_state.current_value() == WideNavigationRailValue::Expanded {
                            "Expanded"
                        } else {
                            "Collapsed"
                        }
                        .to_string(),
                    )
                }),
                on_click = move || {
                    let state = state.clone();
                    named_args!(scope.launch(async move {
                        if state.target_value() == WideNavigationRailValue::Expanded {
                            state.collapse().await
                        } else {
                            state.expand().await
                        }
                    }));
                },
                content = move || {
                    if expanded {
                        VectorIcon(ExtendedIcons::AutoMirrored::Filled::MenuOpen(), Some(header_description.into()));
                    } else {
                        VectorIcon(Icons::Filled::Menu(), Some(header_description.into()));
                    }
                },
            );
        },
    );
}

/// The samples' items of a wide rail, `rail_expanded` or not; `described`, each icon described by its item.
#[kompozable]
fn SampleWideItems(selected_item: MutableState<usize>, rail_expanded: bool, described: bool) {
    for (index, item) in SampleItems.into_iter().enumerate() {
        let s = selected_item.clone();
        let selected = selected_item.get() == index;
        WideNavigationRailItem(
            selected,
            move || s.set(index),
            move || VectorIcon(sample_icon(index, selected), if described { Some(item.into()) } else { None }),
            slot(move || Text(item)),
            rail_expanded,
        );
    }
}

/// An item's bounds, found by its label.
fn item_bounds(rule: &KompozTestRule, label: &str) -> Rect {
    rule.on_node_with_text(label).get_unclipped_bounds_in_root()
}

fn navigation_rail_samples() {
    // NavigationRailSample: its items from the top, each icon described by its item; a click selects one.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            let selected_item = remember(|| mutable_state_of(0usize));
            NavigationRail(
                content = move || {
                    for (index, item) in SampleItems.into_iter().enumerate() {
                        let s = selected_item.clone();
                        let selected = selected_item.get() == index;
                        NavigationRailItem(
                            selected,
                            move || s.set(index),
                            move || VectorIcon(sample_icon(index, selected), Some(item.into())),
                            label = slot(move || Text(item)),
                        );
                    }
                },
            );
        }),
        IntSize::new(400, 600),
    );
    rule.on_node_with_text("Home").assert(kompoz_test::is_selected());
    assert!(item_bounds(&rule, "Home").top < 10.0);
    rule.on_node_with_text("Search").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Search").assert(kompoz_test::is_selected());
    rule.on_node_with_text("Home").assert(kompoz_test::is_not_selected());

    // NavigationRailBottomAlignSample: a spacer of weight 1 pushes the items to the bottom.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            let selected_item = remember(|| mutable_state_of(0usize));
            NavigationRail(
                content = move || {
                    // A Spacer that pushes the NavigationRail items to the bottom of the NavigationRail.
                    Spacer(Modifier.weight(1.0));
                    for (index, item) in SampleItems.into_iter().enumerate() {
                        let s = selected_item.clone();
                        let selected = selected_item.get() == index;
                        NavigationRailItem(
                            selected,
                            move || s.set(index),
                            move || VectorIcon(sample_icon(index, selected), Some(item.into())),
                            label = slot(move || Text(item)),
                        );
                    }
                },
            );
        }),
        IntSize::new(400, 600),
    );
    assert!(near(item_bounds(&rule, "Settings").bottom, 596.0, 0.5), "{:?}", item_bounds(&rule, "Settings"));
    rule.on_node_with_text("Settings").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Settings").assert(kompoz_test::is_selected());
}

fn wide_navigation_rail_samples() {
    // WideNavigationRailCollapsedSample and WideNavigationRailExpandedSample: 96 dp items of top icons, 56 dp tall
    // items of start icons.
    for expanded in [false, true] {
        let rule = KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let selected_item = remember(|| mutable_state_of(0usize));
                let state = remember_wide_navigation_rail_state(if expanded {
                    WideNavigationRailValue::Expanded
                } else {
                    WideNavigationRailValue::Collapsed
                });
                WideNavigationRail(
                    state = Some(state),
                    content = move || SampleWideItems(selected_item.clone(), expanded, false),
                );
            }),
            IntSize::new(400, 600),
        );
        let home = item_bounds(&rule, "Home");
        if expanded {
            assert!(near(home.height(), 56.0, 0.5), "{home:?}");
        } else {
            assert!(near(home.width(), 96.0, 0.5) && near(home.height(), 64.0, 0.5), "{home:?}");
        }
        rule.on_node_with_text("Settings").perform_click();
        rule.wait_for_idle();
        rule.on_node_with_text("Settings").assert(kompoz_test::is_selected());
    }

    // WideNavigationRailResponsiveSample: the header's button expands and collapses it, telling its state.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            let selected_item = remember(|| mutable_state_of(0usize));
            let state = remember_wide_navigation_rail_state();
            Row(
                modifier = Modifier.fill_max_width(),
                content = move || {
                    let expanded = state.target_value() == WideNavigationRailValue::Expanded;
                    let (header_state, selected_item) = (state.clone(), selected_item.clone());
                    WideNavigationRail(
                        state = Some(state.clone()),
                        header = slot(move || SampleRailHeader(header_state.clone())),
                        content = move || SampleWideItems(selected_item.clone(), expanded, false),
                    );
                    let text_string = if state.current_value() == WideNavigationRailValue::Expanded {
                        "Expanded"
                    } else {
                        "Collapsed"
                    };
                    let is_animating = state.is_animating();
                    Column(move || {
                        Text(format!("Is animating: {is_animating}"), modifier = Modifier.padding(16.dp()));
                        Text(format!("The rail is {text_string}."), modifier = Modifier.padding(16.dp()));
                    });
                },
            );
        }),
        IntSize::new(800, 600),
    );
    rule.on_node_with_text("The rail is Collapsed.").assert_exists();
    rule.on_node_with_content_description("Expand rail").assert(kompoz_test::has_state_description("Collapsed"));
    rule.on_node_with_content_description("Expand rail").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("The rail is Expanded.").assert_exists();
    rule.on_node_with_text("Is animating: false").assert_exists();
    rule.on_node_with_content_description("Collapse rail").assert(kompoz_test::has_state_description("Expanded"));
    assert!(near(item_bounds(&rule, "Home").height(), 56.0, 0.5));

    // WideNavigationRailArrangementsSample (its arrangement in a state, as M15 keeps it): centered in the whole
    // height, then at the bottom.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            let selected_item = remember(|| mutable_state_of(0usize));
            let state = remember_wide_navigation_rail_state();
            let arrangement = remember(|| mutable_state_of!(Arrangement::Center));
            Row(
                modifier = Modifier.fill_max_width(),
                content = move || {
                    let expanded = state.target_value() == WideNavigationRailValue::Expanded;
                    let (header_state, selected_item) = (state.clone(), selected_item.clone());
                    WideNavigationRail(
                        state = Some(state.clone()),
                        arrangement = arrangement.get(),
                        header = slot(move || SampleRailHeader(header_state.clone())),
                        content = move || SampleWideItems(selected_item.clone(), expanded, false),
                    );
                    let is_arrangement_center = arrangement.get() == Arrangement::Center;
                    let change_to_string = if is_arrangement_center { "Bottom" } else { "Center" };
                    let arrangement = arrangement.clone();
                    Column(
                        modifier = Modifier.weight(1.0),
                        horizontal_alignment = Alignment::CenterHorizontally,
                        content = move || {
                            Text("Change arrangement to:", modifier = Modifier.padding(16.dp()));
                            let arrangement = arrangement.clone();
                            Button(
                                move || {
                                    if is_arrangement_center {
                                        arrangement.set(Arrangement::Bottom)
                                    } else {
                                        arrangement.set(Arrangement::Center)
                                    }
                                },
                                modifier = Modifier.padding(4.dp()),
                                content = move || Text(change_to_string),
                            );
                        },
                    );
                },
            );
        }),
        IntSize::new(800, 600),
    );
    // Centered in the whole height (600: the three 64 dp items and their 4 dp gaps, 200, from 200).
    let centered = item_bounds(&rule, "Home").top;
    assert!(near(centered, 200.0, 0.5), "{centered}");
    rule.on_node_with_text("Bottom").perform_click();
    rule.wait_for_idle();
    // At the bottom of the room left (inside the 44 dp padding).
    assert!(near(item_bounds(&rule, "Settings").bottom, 556.0, 0.5), "{:?}", item_bounds(&rule, "Settings"));
    rule.on_node_with_text("Center").assert_exists();
}

fn modal_wide_navigation_rail_samples() {
    // ModalWideNavigationRailSample: collapsed in place; its header expands it over the content (its header 64 dp
    // further down), a scrim behind.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            let selected_item = remember(|| mutable_state_of(0usize));
            let state = remember_wide_navigation_rail_state();
            Row(
                modifier = Modifier.fill_max_width(),
                content = move || {
                    let expanded = state.target_value() == WideNavigationRailValue::Expanded;
                    let (header_state, selected_item) = (state.clone(), selected_item.clone());
                    ModalWideNavigationRail(
                        state = Some(state.clone()),
                        // Note: the value of expandedHeaderTopPadding depends on the layout of your screen in
                        // order to achieve the best alignment.
                        expanded_header_top_padding = 64.dp(),
                        header = slot(move || SampleRailHeader(header_state.clone())),
                        content = move || SampleWideItems(selected_item.clone(), expanded, true),
                    );
                    let text_string = if state.current_value() == WideNavigationRailValue::Expanded {
                        "Expanded"
                    } else {
                        "Collapsed"
                    };
                    Column(move || Text(format!("The rail is {text_string}."), modifier = Modifier.padding(16.dp())));
                },
            );
        }),
        IntSize::new(800, 600),
    );
    rule.on_node_with_text("The rail is Collapsed.").assert_exists();
    rule.on_node_with_content_description("Expand rail").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("The rail is Expanded.").assert_exists();
    assert!(rule.draw().rect_colors.iter().any(|c| near(c.alpha(), 0.32, 0.01)));
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_content_description("Search").assert(kompoz_test::is_selected());
    // (the collapsed rail in place keeps its header, under the dialog: the expanded one's is the last)
    rule.on_all_nodes_with(kompoz_test::has_content_description("Collapse rail", false, false), false)
        .on_last()
        .perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("The rail is Collapsed.").assert_exists();

    // DismissibleModalWideNavigationRailSample: hidden until its button opens it; picking an item closes it.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            let selected_item = remember(|| mutable_state_of(0usize));
            let state = remember_wide_navigation_rail_state();
            let scope = remember_coroutine_scope();
            Row(
                modifier = Modifier.fill_max_size(),
                content = move || {
                    let (selected_item, item_state, item_scope) = (selected_item.clone(), state.clone(), scope.clone());
                    ModalWideNavigationRail(
                        state = Some(state.clone()),
                        hide_on_collapse = true,
                        content = move || {
                            for (index, item) in SampleItems.into_iter().enumerate() {
                                let (s, state, scope) = (selected_item.clone(), item_state.clone(), item_scope.clone());
                                let selected = selected_item.get() == index;
                                WideNavigationRailItem(
                                    selected,
                                    move || {
                                        s.set(index);
                                        let state = state.clone();
                                        named_args!(scope.launch(async move { state.collapse().await }));
                                    },
                                    move || VectorIcon(sample_icon(index, selected), None),
                                    slot(move || Text(item)),
                                    true,
                                );
                            }
                        },
                    );
                    let current_page = SampleItems[selected_item.get()];
                    let (state, scope) = (state.clone(), scope.clone());
                    Column(
                        modifier = Modifier.fill_max_width(),
                        horizontal_alignment = Alignment::CenterHorizontally,
                        content = move || {
                            let (state, scope) = (state.clone(), scope.clone());
                            Button(
                                move || {
                                    let state = state.clone();
                                    named_args!(scope.launch(async move { state.expand().await }));
                                },
                                modifier = Modifier.padding(32.dp()),
                                content = move || {
                                    Text(
                                        format!("{current_page} Page\nOpen modal rail"),
                                        text_align = kompoz_text::TextAlign::Center,
                                    )
                                },
                            );
                        },
                    );
                },
            );
        }),
        IntSize::new(800, 600),
    );
    rule.on_node_with_text("Search").assert_does_not_exist();
    rule.on_node_with_text("Home Page\nOpen modal rail").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Search").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Search").assert_does_not_exist();
    rule.on_node_with_text("Search Page\nOpen modal rail").assert_exists();
}

// ---- Composing the three navigation components ------------------------------------------------------------------

fn navigation_items_animate_composing_nothing_around_them() {
    // The C++ test with M15's components: a short navigation bar, a navigation bar and a wide navigation rail of
    // three items each, a tab change composing their caller once, not again as the items animate. (M15's short
    // navigation bar item takes its new label color at once — NavigationItem's StyledLabel doesn't animate it, as
    // the C++'s did —; the navigation bar item's label fades, as the C++'s short bar's did: its colors are checked
    // on their way.)
    let tab = mutable_state_of(0usize);
    let composed = Rc::new(Cell::new(0));
    let short_labels = Rc::new(RefCell::new([Color::Unspecified; 3]));
    let bar_labels = Rc::new(RefCell::new([Color::Unspecified; 3]));
    let (t, c, sl, bl) = (tab.clone(), composed.clone(), short_labels.clone(), bar_labels.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            c.set(c.get() + 1);
            let selected = t.get();
            let (t, sl, bl) = (t.clone(), sl.clone(), bl.clone());
            Column(move || {
                let (t1, t2, t3, sl, bl) = (t.clone(), t.clone(), t.clone(), sl.clone(), bl.clone());
                ShortNavigationBar(
                    content = move || {
                        for i in 0..3 {
                            let (t, sl) = (t1.clone(), sl.clone());
                            ShortNavigationBarItem(
                                selected == i,
                                move || t.set(i),
                                || VectorIcon(Icons::Filled::Home(), Some(String::new())),
                                slot(move || sl.borrow_mut()[i] = LocalContentColor.current()),
                            );
                        }
                    },
                );
                NavigationBar(
                    content = move || {
                        for i in 0..3 {
                            let (t, bl) = (t2.clone(), bl.clone());
                            NavigationBarItem(
                                selected == i,
                                move || t.set(i),
                                || VectorIcon(Icons::Filled::Home(), None),
                                label = slot(move || {
                                    bl.borrow_mut()[i] = LocalContentColor.current();
                                    Text("Sekme");
                                }),
                            );
                        }
                    },
                );
                WideNavigationRail(
                    content = move || {
                        for i in 0..3 {
                            let t = t3.clone();
                            WideNavigationRailItem(
                                selected == i,
                                move || t.set(i),
                                || VectorIcon(Icons::Filled::Home(), Some(String::new())),
                                slot(|| Text("Ray")),
                                false,
                            );
                        }
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    let cs = light_color_scheme();
    assert!(short_labels.borrow()[0] == cs.secondary && short_labels.borrow()[1] == cs.on_surface_variant);
    assert!(bar_labels.borrow()[0] == cs.secondary && bar_labels.borrow()[1] == cs.on_surface_variant);
    tab.set(1);
    rule.advance_frame(); // the caller composed with the new tab...
    let before = composed.get();
    assert_eq!(short_labels.borrow()[1], cs.secondary); // (M15: at once)
    rule.advance_frame();
    rule.advance_frame();
    let fading = bar_labels.borrow()[1];
    assert!(fading != cs.on_surface_variant && fading != cs.secondary, "{fading:?}"); // (on its way)
    rule.wait_for_idle();
    assert_eq!(composed.get(), before); // ...and not again as the items animated
    assert!(short_labels.borrow()[1] == cs.secondary && short_labels.borrow()[0] == cs.on_surface_variant);
    assert!(bar_labels.borrow()[1] == cs.secondary && bar_labels.borrow()[0] == cs.on_surface_variant);
}
