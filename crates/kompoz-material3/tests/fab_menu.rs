//! Material Expressive's FAB menu as Compose's material3 (1.5.0-alpha17) has it: the C++ Kompoz's
//! fab_menu_opens_from_the_bottom_and_its_button_turns_round and tabs_and_fab_menu_take_compose_named_arguments
//! (its tabs, FAB menu and floating toolbar; with M15's values where the C++ had its own), Compose's
//! behaviour (the toggleable FAB's defaults and icon, the menu's alignment, scrolling, keyboard focus and
//! accessibility) and M15's FloatingActionButtonMenuSample.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::lazy::{LazyListScope, remember_lazy_list_state};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_extended::Icons;
use kompoz_material3::Text;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::draw::remember_vector_painter;
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::input::{Key, KeyEvent, KeyEventType, KeyModifiers, PointerType};
use kompoz_ui::semantics::SemanticsActions;
use kompoz_ui::semantics::{CustomAccessibilityAction, SemanticsProperties, ToggleableState};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

tests!(
    fab_menu_opens_from_the_bottom_and_its_button_turns_round,
    tabs_and_fab_menu_take_compose_named_arguments,
    toggle_floating_action_button_defaults_go_from_the_fab_to_the_close_button,
    a_toggle_fabs_icon_is_sized_and_tinted_by_its_progress,
    fab_menu_items_line_up_as_the_menu_says,
    a_fab_menu_taller_than_its_room_scrolls,
    tab_from_the_fab_menus_button_goes_to_its_first_item,
    fab_menu_items_keep_no_48_dp_room_and_come_before_the_button,
    floating_action_button_menu_sample,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn same_rect(a: Rect, b: Rect) -> bool {
    near(a.left, b.left, 0.01)
        && near(a.top, b.top, 0.01)
        && near(a.width(), b.width(), 0.01)
        && near(a.height(), b.height(), 0.01)
}

/// C++'s `Rect{x, y, width, height}`.
fn xywh(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::new(x, y, x + width, y + height)
}

fn same_color(a: Color, b: Color) -> bool {
    near(a.red(), b.red(), 0.01)
        && near(a.green(), b.green(), 0.01)
        && near(a.blue(), b.blue(), 0.01)
        && near(a.alpha(), b.alpha(), 0.01)
}

fn tap(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.host().pointer_up(at, -1, 0);
}

/// The rounded rect filling `b` (its radius and color), as drawn.
fn fill(rule: &KompozTestRule, b: Rect) -> Option<(f32, Color)> {
    let c = rule.draw();
    c.rects
        .iter()
        .zip(c.rect_radii.iter().zip(&c.rect_colors))
        .find(|(r, _)| same_rect(**r, b))
        .map(|(_, (radius, color))| (*radius, *color))
}

/// Whether a text shows (its semantics aren't cleared: a FAB menu item hidden tells nothing).
fn shows(rule: &KompozTestRule, text: &str) -> bool {
    rule.on_node_with_text(text).exists()
}

// ---- C++ fab_menu_opens_from_the_bottom_and_its_button_turns_round ----------------------------------------------

fn fab_menu_opens_from_the_bottom_and_its_button_turns_round() {
    // M15: the menu keeps 16 dp at its sides and its button 16 dp above its bottom (the C++ put the button in the
    // corner); its items don't come in by an AnimatedVisibility each but are shown one after the other from the
    // button up by a stagger animation, each widening and fading in — hidden, an item isn't placed and tells
    // nothing. The FAB's semantics bounds are its toggleable container's (Compose's: the C++ took its child's).
    let open = mutable_state_of(false);
    let picked = Rc::new(RefCell::new(String::new()));
    let progress = Rc::new(Cell::new(-1.0f32));
    let cs = light_color_scheme();
    let (o, pk, pr) = (open.clone(), picked.clone(), progress.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, pk, pr) = (o.clone(), pk.clone(), pr.clone());
        Box(
            modifier = Modifier.size_wh(400.dp(), 600.dp()),
            content_alignment = Alignment::BottomEnd,
            content = move || {
                let (o, pk, pr) = (o.clone(), pk.clone(), pr.clone());
                let button_open = o.clone();
                FloatingActionButtonMenu(
                    o.get(),
                    move || {
                        let (set, pr) = (button_open.clone(), pr.clone());
                        ToggleFloatingActionButton(
                            button_open.get(),
                            move |on| set.set(on),
                            modifier = Modifier.test_tag("fab"),
                            content = move |s: &ToggleFloatingActionButtonScope| {
                                pr.set(s.checked_progress());
                                VectorIcon(Icons::Filled::Add(), Some("Menü".into()));
                            },
                        );
                    },
                    content = move |menu: &FloatingActionButtonMenuScope| {
                        for label in ["Belge", "Fotoğraf", "Kişi"] {
                            let pk = pk.clone();
                            menu.FloatingActionButtonMenuItem(
                                move || *pk.borrow_mut() = label.to_string(),
                                move || Text(label),
                                || VectorIcon(Icons::Filled::Add(), Some(String::new())),
                                modifier = Modifier.test_tag(label),
                            );
                        }
                    },
                );
            },
        );
    }));
    let fab = bounds(&rule, "fab");
    assert!(same_rect(fab, xywh(400.0 - 16.0 - 56.0, 600.0 - 16.0 - 56.0, 56.0, 56.0)), "{fab:?}");
    let button = |rule: &KompozTestRule| fill(rule, bounds(rule, "fab")).expect("the container drawn");
    assert!(near(button(&rule).0, 16.0, 0.01) && same_color(button(&rule).1, cs.primary_container));
    assert_eq!(progress.get(), 0.0);
    assert!(!shows(&rule, "Belge") && !shows(&rule, "Kişi")); // closed: no items

    tap(&rule, fab.center());
    // (frame by frame until the first item shows: Compose's stagger starts a frame after the change, its spring
    // counting the items in, each item's fade starting as the count reaches it)
    let mut frames = 0;
    while !shows(&rule, "Kişi") && frames < 30 {
        rule.advance_frame();
        frames += 1;
    }
    assert!(shows(&rule, "Kişi") && !shows(&rule, "Belge"), "the one next to the button first");
    assert!(bounds(&rule, "Kişi").width() < bounds(&rule, "Kişi").height() * 3.0, "widening");
    assert!(progress.get() > 0.0 && progress.get() < 1.0, "{}", progress.get());
    rule.wait_for_idle();
    let (radius, color) = button(&rule);
    assert!(near(radius, 28.0, 0.01) && same_color(color, cs.primary) && near(progress.get(), 1.0, 0.01)); // a primary circle
    let (top, middle, bottom) = (bounds(&rule, "Belge"), bounds(&rule, "Fotoğraf"), bounds(&rule, "Kişi"));
    assert!(
        near(top.height(), 56.0, 0.01)
            && near(middle.top - top.bottom, 4.0, 0.01)
            && near(bottom.top - middle.bottom, 4.0, 0.01)
    );
    // Above the button, at its end.
    assert!(
        near(fab.top - bottom.bottom, 8.0, 0.01)
            && near(top.right, fab.right, 0.01)
            && near(bottom.right, fab.right, 0.01)
    );
    let (pill_radius, pill_color) = fill(&rule, top).expect("the pill drawn"); // pills
    assert!(near(pill_radius, 28.0, 0.01) && pill_color == cs.primary_container);

    tap(&rule, middle.center());
    assert_eq!(*picked.borrow(), "Fotoğraf");
    open.set(false);
    let mut frames = 0;
    while shows(&rule, "Belge") && frames < 30 {
        rule.advance_frame();
        frames += 1;
    }
    assert!(!shows(&rule, "Belge") && shows(&rule, "Kişi"), "closing from the top");
    assert!(near(bounds(&rule, "Kişi").height(), 56.0, 0.01));
    rule.wait_for_idle();
    assert!(!shows(&rule, "Kişi") && near(button(&rule).0, 16.0, 0.01));
    let fab_node = rule.on_node_with_content_description("Menü").fetch_semantics_node();
    assert_eq!(fab_node.config().get(SemanticsProperties::ToggleableState), Some(ToggleableState::Off)); // told unchecked

    // A medium one (M15's `containerSize(80.dp)`: 80 dp, 56 once checked) shrinking towards its top end.
    let medium = mutable_state_of(false);
    let m = medium.clone();
    let sized = KompozTestRule::new(kompozable_lambda!(move || {
        let m = m.clone();
        let set = m.clone();
        Box(move || {
            ToggleFloatingActionButton(
                m.get(),
                move |on| set.set(on),
                modifier = Modifier.test_tag("orta"),
                container_size = ToggleFloatingActionButtonDefaults.container_size_of(80.dp()),
                content = |_: &ToggleFloatingActionButtonScope| {},
            );
        });
    }));
    let outer = sized.on_root().fetch_semantics_node().children()[0].bounds_in_root();
    assert!(
        same_rect(outer, xywh(0.0, 0.0, 80.0, 80.0)) && same_rect(bounds(&sized, "orta"), xywh(0.0, 0.0, 80.0, 80.0))
    );
    medium.set(true);
    sized.wait_for_idle();
    assert!(same_rect(bounds(&sized, "orta"), xywh(24.0, 0.0, 56.0, 56.0)), "{:?}", bounds(&sized, "orta"));
}

// ---- C++ tabs_and_fab_menu_take_compose_named_arguments -------------------------------------------------------

fn tabs_and_fab_menu_take_compose_named_arguments() {
    // The C++ test in Rust's named form (its Kotlin form, `Navigation`, in crates/kompoz/tests/
    // kotlin_expressive_buttons.rs): its tabs (WP5's), FAB menu — the items made through the scope — and
    // HorizontalFloatingToolbar (WP4a's).
    let tab = mutable_state_of(0);
    let open = mutable_state_of(false);
    let picked = mutable_state_of("yok".to_string());
    let (t, o, p) = (tab.clone(), open.clone(), picked.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (t, o, p) = (t.clone(), o.clone(), p.clone());
            Column(move || {
                let (t1, t2) = (t.clone(), t.clone());
                PrimaryTabRow(
                    selected_tab_index = t.get(),
                    tabs = move || {
                        for i in 0..2 {
                            let t = t1.clone();
                            TextIconTab(
                                selected = t.get() == i,
                                on_click = move || t.set(i),
                                text = Some(Rc::new(move || Text(format!("sekme {i}")))),
                                icon = Some(Rc::new(|| VectorIcon(Icons::Filled::Star(), Some(String::new())))),
                            );
                        }
                    },
                );
                SecondaryScrollableTabRow(
                    selected_tab_index = t.get(),
                    edge_padding = Dp(0.0),
                    tabs = move || {
                        for i in 0..2 {
                            let t = t2.clone();
                            LeadingIconTab(
                                selected = t.get() == i,
                                on_click = move || t.set(i),
                                text = move || Text(format!("kayan {i}")),
                                icon = || VectorIcon(Icons::Filled::Home(), Some(String::new())),
                                unselected_content_color = Some(Color::Red),
                            );
                        }
                    },
                );
                Text(format!("seçili {} · {}", t.get(), p.get()));
                let (o, p) = (o.clone(), p.clone());
                Box(
                    modifier = Modifier.size(300.dp()),
                    content_alignment = Alignment::BottomEnd,
                    content = move || {
                        let (button_open, items_open, p) = (o.clone(), o.clone(), p.clone());
                        FloatingActionButtonMenu(
                            expanded = o.get(),
                            button = move || {
                                let set = button_open.clone();
                                ToggleFloatingActionButton(
                                    checked = button_open.get(),
                                    on_checked_change = move |on| set.set(on),
                                    modifier = Modifier.test_tag("aç"),
                                    content = |_: &ToggleFloatingActionButtonScope| {
                                        VectorIcon(Icons::Filled::Add(), Some("Menü".into()))
                                    },
                                );
                            },
                            content = move |menu: &FloatingActionButtonMenuScope| {
                                for label in ["Not", "Konum"] {
                                    let (p, o) = (p.clone(), items_open.clone());
                                    menu.FloatingActionButtonMenuItem(
                                        on_click = move || {
                                            p.set(label.to_string());
                                            o.set(false);
                                        },
                                        text = move || Text(label),
                                        icon = || VectorIcon(Icons::Filled::Edit(), Some(String::new())),
                                    );
                                }
                            },
                        );
                    },
                );
                // (WP4a's toolbar, its colors by name)
                HorizontalFloatingToolbar(
                    expanded = true,
                    colors = FloatingToolbarDefaults.vibrant_floating_toolbar_colors(),
                    content = || {
                        IconButton(|| {}, content = || VectorIcon(Icons::Filled::MoreVert(), Some("Daha".into())));
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    rule.on_node_with_text("sekme 1").assert_exists();
    rule.on_node_with_text("kayan 1").assert_exists();
    rule.on_node_with_text("kayan 1").perform_click();
    rule.on_node_with_text("seçili 1 · yok").assert_exists();
    rule.on_node_with_tag("aç").perform_click();
    for _ in 0..91 {
        rule.advance_frame();
    }
    rule.on_node_with_text("Konum").assert_exists();
    rule.on_node_with_text("Konum").perform_click();
    for _ in 0..90 {
        rule.advance_frame();
    }
    rule.on_node_with_text("seçili 1 · Konum").assert_exists();
    rule.on_node_with_content_description("Daha").assert_exists();
    assert!(!shows(&rule, "Konum"), "closed by the item");
}

// ---- Compose's behaviour ----------------------------------------------------------------------------------------

fn toggle_floating_action_button_defaults_go_from_the_fab_to_the_close_button() {
    type Made = (Rc<dyn Fn(f32) -> Color>, Rc<dyn Fn(f32) -> Color>, Rc<dyn Fn(f32) -> Color>);
    let made: Rc<RefCell<Option<Made>>> = Rc::new(RefCell::new(None));
    let m = made.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        *m.borrow_mut() = Some((
            ToggleFloatingActionButtonDefaults.container_color(),
            ToggleFloatingActionButtonDefaults.icon_color(),
            ToggleFloatingActionButtonDefaults.container_color(initial_color = Color::Red, final_color = Color::Blue),
        ));
    }));
    let (container, icon, given) = made.borrow_mut().take().expect("composed");
    let cs = light_color_scheme();
    assert_eq!(container(0.0), cs.primary_container);
    assert_eq!(container(1.0), cs.primary);
    assert_eq!(icon(0.0), cs.on_primary_container);
    assert_eq!(icon(1.0), cs.on_primary);
    assert_eq!(given(0.5), lerp_color(Color::Red, Color::Blue, 0.5));
    let d = ToggleFloatingActionButtonDefaults;
    let ends = |f: Rc<dyn Fn(f32) -> Dp>| (f(0.0), f(1.0));
    assert_eq!(ends(d.container_size()), (Dp(56.0), Dp(56.0)));
    assert_eq!(ends(d.container_size_medium()), (Dp(80.0), Dp(56.0)));
    assert_eq!(ends(d.container_size_large()), (Dp(96.0), Dp(56.0)));
    assert_eq!(ends(named_args!(d.container_size_of(Dp(64.0), final_size = Dp(40.0)))), (Dp(64.0), Dp(40.0)));
    assert_eq!(ends(d.container_size_of(Dp(64.0))), (Dp(64.0), Dp(56.0)));
    assert_eq!(ends(d.container_corner_radius_of(Dp(8.0))), (Dp(8.0), Dp(28.0)));
    assert_eq!(ends(named_args!(d.icon_size_of(Dp(30.0), final_size = Dp(18.0)))), (Dp(30.0), Dp(18.0)));
    assert_eq!(ends(d.container_corner_radius()), (Dp(16.0), Dp(28.0)));
    assert_eq!(ends(d.container_corner_radius_medium()), (Dp(20.0), Dp(28.0)));
    assert_eq!(ends(d.container_corner_radius_large()), (Dp(28.0), Dp(28.0)));
    assert_eq!(ends(d.icon_size()), (Dp(24.0), Dp(20.0)));
    assert_eq!(ends(d.icon_size_medium()), (Dp(28.0), Dp(20.0)));
    // (M15's TODO: FabLargeTokens.IconSize is incorrect, 36 dp written)
    assert_eq!(ends(d.icon_size_large()), (Dp(36.0), Dp(20.0)));
    assert_eq!(d.container_size()(0.5), Dp(56.0));
    assert_eq!(d.container_size_medium()(0.5), Dp(68.0));
}

fn a_toggle_fabs_icon_is_sized_and_tinted_by_its_progress() {
    // animateIcon: measured at exactly its size (24 dp to 20), drawn through a layer tinted by its color.
    let checked = mutable_state_of(false);
    let c = checked.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let c = c.clone();
        let set = c.clone();
        ToggleFloatingActionButton(
            c.get(),
            move |on| set.set(on),
            content = |s: &ToggleFloatingActionButtonScope| {
                let s = s.clone();
                VectorIcon(
                    Icons::Filled::Add(),
                    Some("artı".into()),
                    modifier = Modifier.animate_icon(Rc::new(move || s.checked_progress())).test_tag("simge"),
                );
            },
        );
    }));
    let cs = light_color_scheme();
    let icon = |rule: &KompozTestRule| rule.on_node_with_tag_unmerged("simge").get_unclipped_bounds_in_root();
    assert!(same_rect(icon(&rule), xywh(16.0, 16.0, 24.0, 24.0)), "{:?}", icon(&rule));
    // (the icon's color as drawn through the layer's tint filter)
    let tint = |rule: &KompozTestRule| -> Option<Color> { rule.draw().vector_colors.last().copied() };
    assert!(same_color(tint(&rule).expect("tinted"), cs.on_primary_container));
    checked.set(true);
    rule.wait_for_idle();
    assert!(same_rect(icon(&rule), xywh(18.0, 18.0, 20.0, 20.0)), "{:?}", icon(&rule));
    assert!(same_color(tint(&rule).expect("tinted"), cs.on_primary));
}

fn fab_menu_items_line_up_as_the_menu_says() {
    // horizontalAlignment: the items and the button at the start of the menu (16 dp in), each item as wide as its own.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            modifier = Modifier.fill_max_size(),
            content = || {
                FloatingActionButtonMenu(
                    true,
                    || {
                        ToggleFloatingActionButton(
                            true,
                            |_| {},
                            modifier = Modifier.test_tag("düğme"),
                            content = |_: &ToggleFloatingActionButtonScope| {},
                        );
                    },
                    horizontal_alignment = Alignment::Start,
                    content = |menu: &FloatingActionButtonMenuScope| {
                        assert_eq!(menu.horizontal_alignment(), Alignment::Start);
                        for label in ["Kısa", "Çok daha uzun bir öğe"] {
                            menu.FloatingActionButtonMenuItem(
                                || {},
                                move || Text(label),
                                || {},
                                modifier = Modifier.test_tag(label),
                            );
                        }
                    },
                );
            },
        );
    }));
    rule.wait_for_idle();
    let (short, long, button) = (bounds(&rule, "Kısa"), bounds(&rule, "Çok daha uzun bir öğe"), bounds(&rule, "düğme"));
    assert!(near(short.left, 16.0, 0.01) && near(long.left, 16.0, 0.01) && near(button.left, 16.0, 0.01));
    assert!(short.width() < long.width());
    // At least as wide as tall.
    assert!(short.width() >= 56.0);
}

fn a_fab_menu_taller_than_its_room_scrolls() {
    // The column scrolls (enabled while expanded) once its items don't fit the room it is given.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            modifier = Modifier.size_wh(300.dp(), 400.dp()),
            content = || {
                FloatingActionButtonMenu(
                    true,
                    || ToggleFloatingActionButton(true, |_| {}, content = |_: &ToggleFloatingActionButtonScope| {}),
                    content = |menu: &FloatingActionButtonMenuScope| {
                        for i in 0..10 {
                            menu.FloatingActionButtonMenuItem(
                                || {},
                                move || Text(format!("öğe {i}")),
                                || {},
                                modifier = Modifier.test_tag(format!("öğe {i}")),
                            );
                        }
                    },
                );
            },
        );
    }));
    rule.wait_for_idle();
    rule.wait_for_idle();
    let scrolled =
        rule.all_nodes(true).into_iter().any(|n| n.config().contains(SemanticsProperties::VerticalScrollAxisRange));
    assert!(scrolled, "the column scrolls");
    let first = bounds(&rule, "öğe 0");
    assert!(near(first.top, 0.0, 0.01), "{first:?}");
}

fn tab_from_the_fab_menus_button_goes_to_its_first_item() {
    // For keyboard users the focus goes from the button (after the items in the order) to the first item: Tab (no
    // Shift) or Down while expanded.
    let focused = Rc::new(Cell::new(false));
    let f = focused.clone();
    let requester = FocusRequester::new();
    let r = requester.clone();
    let modes: Rc<RefCell<Option<kompoz_ui::platform::InputModeManager>>> = Rc::new(RefCell::new(None));
    let md = modes.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *md.borrow_mut() = Some(kompoz_ui::platform::LocalInputModeManager.current());
        let (f, r) = (f.clone(), r.clone());
        Box(
            modifier = Modifier.fill_max_size(),
            content = move || {
                let (f, r) = (f.clone(), r.clone());
                FloatingActionButtonMenu(
                    true,
                    move || {
                        ToggleFloatingActionButton(
                            true,
                            |_| {},
                            modifier = Modifier.focus_requester(&r),
                            content = |_: &ToggleFloatingActionButtonScope| {},
                        )
                    },
                    content = move |menu: &FloatingActionButtonMenuScope| {
                        let f = f.clone();
                        menu.FloatingActionButtonMenuItem(
                            || {},
                            || Text("ilk"),
                            || {},
                            modifier = Modifier.on_focus_changed(move |state| f.set(state.is_focused())),
                        );
                        menu.FloatingActionButtonMenuItem(|| {}, || Text("ikinci"), || {});
                    },
                );
            },
        );
    }));
    // (the keyboard's input mode, as a Compose test asks through LocalInputModeManager)
    let modes = modes.borrow().clone().expect("composed");
    rule.run_on_idle(|| modes.request_input_mode(kompoz_ui::platform::InputMode::Keyboard));
    rule.wait_for_idle();
    requester.request_focus();
    rule.wait_for_idle();
    assert!(!focused.get());
    rule.host().key_down(Key::Tab, KeyModifiers::None);
    rule.wait_for_idle();
    assert!(focused.get(), "Tab went to the first item");
}

fn fab_menu_items_keep_no_48_dp_room_and_come_before_the_button() {
    // An item provides no minimum interactive size to its content (a 24 dp icon button inside measures 40), and the
    // items' column is a traversal group before the button (traversal index -0.9).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            modifier = Modifier.fill_max_size(),
            content = || {
                FloatingActionButtonMenu(
                    true,
                    || ToggleFloatingActionButton(true, |_| {}, content = |_: &ToggleFloatingActionButtonScope| {}),
                    content = |menu: &FloatingActionButtonMenuScope| {
                        menu.FloatingActionButtonMenuItem(
                            || {},
                            || Text("öğe"),
                            || IconButton(|| {}, modifier = Modifier.test_tag("iç"), content = || {}),
                        );
                    },
                );
            },
        );
    }));
    rule.wait_for_idle();
    let inner = rule.on_node_with_tag_unmerged("iç").get_unclipped_bounds_in_root();
    assert!(near(inner.height(), 40.0, 0.01), "{inner:?}");
    let groups: Vec<f32> = rule
        .all_nodes(true)
        .into_iter()
        .filter(|n| n.config().get(SemanticsProperties::IsTraversalGroup) == Some(true))
        .filter_map(|n| n.config().get(SemanticsProperties::TraversalIndex))
        .collect();
    assert!(groups.contains(&-0.9), "{groups:?}");
}

// ---- M15's FloatingActionButtonMenuSamples.kt -------------------------------------------------------------------

#[kompozable]
fn FloatingActionButtonMenuSample() {
    let list_state = remember_lazy_list_state();
    let visible_state = list_state.clone();
    let fab_visible = remember(move || {
        let s = visible_state.clone();
        derived_state_of(move || s.first_visible_item_index() == 0 || !s.can_scroll_forward())
    });
    let focus_requester = remember(FocusRequester::new);

    Box(move || {
        LazyColumn(
            state = Some(list_state.clone()),
            modifier = Modifier.fill_max_size(),
            content = |this: &mut LazyListScope| {
                for index in 0..100 {
                    this.item(
                        content = move || {
                            Text(
                                format!("List item - {index}"),
                                modifier = Modifier.clickable(|| {}).fill_max_width().padding(24.dp()),
                            )
                        },
                    );
                }
            },
        );

        let items = vec![
            (Icons::AutoMirrored::Filled::Message(), "Reply"),
            (Icons::Filled::People(), "Reply all"),
            (Icons::Filled::Contacts(), "Forward"),
            (Icons::Filled::Snooze(), "Snooze"),
            (Icons::Filled::Archive(), "Archive"),
            (Icons::AutoMirrored::Filled::Label(), "Label"),
        ];

        let fab_menu_expanded = remember_saveable(|| mutable_state_of(false));

        let back = fab_menu_expanded.clone();
        kompoz_ui::back::BackHandler(fab_menu_expanded.get(), move || back.set(false));

        let (button_expanded, fab_visible, focus_requester) =
            (fab_menu_expanded.clone(), fab_visible.clone(), focus_requester.clone());
        let items_expanded = fab_menu_expanded.clone();
        let item_focus = focus_requester.clone();
        FloatingActionButtonMenu(
            modifier = Modifier.align(Alignment::BottomEnd),
            expanded = fab_menu_expanded.get(),
            button = move || {
                let (expanded, fab_visible, focus_requester) =
                    (button_expanded.clone(), fab_visible.clone(), focus_requester.clone());
                // A FAB should have a tooltip associated with it.
                TooltipBox(
                    position_provider = TooltipDefaults.remember_tooltip_position_provider(if expanded.get() {
                        TooltipAnchorPosition::Start
                    } else {
                        TooltipAnchorPosition::Above
                    }),
                    tooltip = |scope: &dyn TooltipScope| scope.PlainTooltip(content = || Text("Toggle menu")),
                    state = remember_tooltip_state(),
                    content = move || {
                        let described = expanded.clone();
                        let toggle = expanded.clone();
                        ToggleFloatingActionButton(
                            modifier = Modifier
                                .semantics(move |s| {
                                    s.set_traversal_index(-1.0);
                                    s.set_state_description(if described.get() { "Expanded" } else { "Collapsed" });
                                    s.set_content_description("Toggle menu");
                                })
                                .animate_floating_action_button(
                                    visible = fab_visible.get() || expanded.get(),
                                    alignment = Alignment::BottomEnd,
                                )
                                .focus_requester(&focus_requester),
                            checked = expanded.get(),
                            on_checked_change = move |_| toggle.set(!toggle.get()),
                            content = |this: &ToggleFloatingActionButtonScope| {
                                let scope = this.clone();
                                let image_vector = remember(move || {
                                    derived_state_of(move || {
                                        if scope.checked_progress() > 0.5 {
                                            Icons::Filled::Close()
                                        } else {
                                            Icons::Filled::Add()
                                        }
                                    })
                                });
                                let progress = this.clone();
                                Icon(
                                    painter = Rc::new(remember_vector_painter(image_vector.get())),
                                    content_description = None,
                                    modifier = Modifier.animate_icon(Rc::new(move || progress.checked_progress())),
                                );
                            },
                        );
                    },
                );
            },
            content = move |this: &FloatingActionButtonMenuScope| {
                let count = items.len();
                for (i, item) in items.iter().enumerate() {
                    let (closing, closing_click) = (items_expanded.clone(), items_expanded.clone());
                    let focus_requester = item_focus.clone();
                    let icon = item.0.clone();
                    let text = item.1;
                    this.FloatingActionButtonMenuItem(
                        modifier = Modifier
                            .semantics(move |s| {
                                s.set_is_traversal_group(true);
                                // Add a custom a11y action to allow closing the menu when focusing
                                // the last menu item, since the close button comes before the first
                                // menu item in the traversal order.
                                if i == count - 1 {
                                    let closing = closing.clone();
                                    s.set_custom_actions(vec![CustomAccessibilityAction::new(
                                        "Close menu",
                                        move || {
                                            closing.set(false);
                                            true
                                        },
                                    )]);
                                }
                            })
                            .then(if i == 0 {
                                Modifier.on_key_event(move |it: &KeyEvent| {
                                    // Navigating back from the first item should go back to the
                                    // FAB menu button.
                                    if it.r#type == KeyEventType::KeyDown
                                        && (it.key == Key::DirectionUp || (it.is_shift_pressed() && it.key == Key::Tab))
                                    {
                                        focus_requester.request_focus();
                                        return true;
                                    }
                                    false
                                })
                            } else {
                                Modifier
                            }),
                        on_click = move || closing_click.set(false),
                        icon = move || VectorIcon(icon.clone(), None),
                        text = move || Text(text),
                    );
                }
            },
        );
    });
}

fn floating_action_button_menu_sample() {
    let rule =
        KompozTestRule::with_viewport(kompozable_lambda!(|| FloatingActionButtonMenuSample()), IntSize::new(400, 800));
    let toggle = || rule.on_node_with_content_description("Toggle menu");
    toggle().assert_exists();
    assert_eq!(
        toggle().fetch_semantics_node().config().get(SemanticsProperties::StateDescription),
        Some("Collapsed".into())
    );
    assert!(!shows(&rule, "Reply"));
    toggle().perform_click();
    rule.wait_for_idle();
    assert_eq!(
        toggle().fetch_semantics_node().config().get(SemanticsProperties::StateDescription),
        Some("Expanded".into())
    );
    for label in ["Reply", "Reply all", "Forward", "Snooze", "Archive", "Label"] {
        assert!(shows(&rule, label), "{label}");
    }
    // The last item closes the menu by its custom action.
    let last = rule.on_node_with_text("Label").fetch_semantics_node();
    let actions = last.config().get(SemanticsActions::CustomActions).expect("the last item's action");
    assert_eq!(actions[0].label, "Close menu");
    rule.run_on_idle(|| (actions[0].action)());
    rule.wait_for_idle();
    assert!(!shows(&rule, "Reply"));
    // Back closes it too.
    toggle().perform_click();
    rule.wait_for_idle();
    assert!(shows(&rule, "Archive"));
    rule.host().back_invoked();
    rule.wait_for_idle();
    assert!(!shows(&rule, "Archive"));
    // An item's click closes it.
    toggle().perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Forward").perform_click();
    rule.wait_for_idle();
    assert!(!shows(&rule, "Forward"));
    // Scrolled down the list, the FAB hides (unless the menu is open).
    rule.on_node_with_text("List item - 0").assert_exists();
    let before = rule.draw().vectors.len();
    rule.on_root().perform_touch_input(|t| t.swipe_up());
    rule.wait_for_idle();
    rule.on_node_with_text("List item - 0").assert_does_not_exist();
    assert!(rule.draw().vectors.len() < before, "the FAB's icon hidden");
}
