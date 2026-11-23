//! Material's menus as Compose's material3 has them (Menu.kt, MenuDefaults.kt, ExposedDropdownMenu.kt,
//! internal/MenuPosition.kt, Compose for desktop's SkikoMenu/Menu/ExposedDropdownMenu actuals): the C++ Kompoz's
//! menu tests by name (kompoz_tests.cpp, Compose's numbers where the two differ — said in each test), and Compose's
//! behaviour where the C++ tests don't reach.
//!
//! In a MaterialTheme (Compose's menus take the theme's label large style, 14 sp with 0.1 sp letter spacing: 7.1 px a
//! character on the fake text backend). Density 1: dp are px.

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas, has_role, has_test_tag};
use kompoz_ui::graphics_context::CornerRadii;
use kompoz_ui::input::{Key, KeyModifiers, PointerType};
use kompoz_ui::semantics::{Role, SemanticsProperties};
use kompoz_ui::strings::{LocalStrings, Strings, is_rtl_language};
use kompoz_ui::window::PopupProperties;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

tests!(
    dropdown_menu_opens_below_its_anchor_and_closes_outside,
    rtl_from_the_host_and_menus_under_the_anchors_end,
    expressive_menu_groups_and_checked_and_selected_items,
    exposed_dropdown_menu_picks_an_option,
    exposed_dropdown_menu_filters_as_you_type,
    library_strings_follow_the_hosts_language_and_local_overrides,
    menus_compose_around_them_only_as_they_open_and_check,
    arrow_keys_move_the_focus_through_a_menus_items,
    menus_scale_and_fade_in_from_where_they_meet_their_anchor,
    menu_item_colors_follow_enabled_and_selected,
    groups_take_their_inactive_shape_once_hovered_and_left,
    exposed_menus_go_above_where_there_is_no_room_below,
    anchor_types_decide_whether_the_menu_takes_the_focus,
    back_closes_menus,
    a_long_menu_scrolls_by_its_scroll_state,
    menu_defaults_and_shapes_as_composes,
    items_lay_out_their_icons_and_text,
    menus_take_an_offset_and_keep_their_margins_at_the_windows_edges,
    menus_take_their_properties_shapes_colors_and_paddings,
);

// ---- Helpers --------------------------------------------------------------------------------------------------

type Log = Rc<RefCell<Vec<String>>>;

fn new_log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn push(log: &Log, s: &str) {
    log.borrow_mut().push(s.to_string());
}

fn taken(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

fn slot(f: impl Fn() + 'static) -> Option<Rc<dyn Fn()>> {
    Some(Rc::new(f))
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// The bounds of the first node of `tag` in the unmerged tree (an Expressive item's leading icon is there twice: the
/// one shown and the ghost measuring its room, not placed — at the same place).
fn unmerged(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_all_nodes_with(has_test_tag(tag), true).on_first().get_unclipped_bounds_in_root()
}

fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    named_args!(rule.on_node_with_text(text, use_unmerged_tree = true)).get_unclipped_bounds_in_root()
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.host().pointer_up(p, -1, 0);
    rule.wait_for_idle();
}

fn key(rule: &KompozTestRule, key: Key) {
    rule.host().key_down(key, KeyModifiers::None);
    rule.host().key_up(key, KeyModifiers::None);
    rule.wait_for_idle();
}

/// The first filled rect of `color` whose middle is inside `inside`.
fn fill(c: &RecordingCanvas, inside: Rect, color: Color) -> Option<usize> {
    (0..c.rects.len()).find(|&i| c.rect_colors[i] == color && inside.contains(c.rects[i].center()))
}

/// The first filled path of `color` whose middle is inside `inside`.
fn fill_path(c: &RecordingCanvas, inside: Rect, color: Color) -> Option<usize> {
    (0..c.paths.len()).find(|&i| c.path_colors[i] == color && inside.contains(c.paths[i].center()))
}

/// Whether the point `d` px in from a drawn path's top left corner (`bottom`: bottom left) is in it.
fn corner_in(path: &Path, d: f32, bottom: bool) -> bool {
    let b = path.get_bounds();
    path.contains(if bottom { Offset::new(b.left + d, b.bottom - d) } else { Offset::new(b.left + d, b.top + d) })
}

fn corners(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> CornerRadii {
    CornerRadii { top_left, top_right, bottom_right, bottom_left }
}

fn same_rect(a: Rect, b: Rect) -> bool {
    near(a.left, b.left, 0.5) && near(a.top, b.top, 0.5) && near(a.right, b.right, 0.5) && near(a.bottom, b.bottom, 0.5)
}

/// The colors `text` was drawn in.
fn text_colors(c: &RecordingCanvas, text: &str) -> Vec<Color> {
    (0..c.texts.len()).filter(|&i| c.texts[i] == text).map(|i| c.colors[i]).collect()
}

/// A shape's corners as `RoundedCornerShape(…)` writes them (top start, top end, bottom end, bottom start), in dp.
fn rounded(ts: f32, te: f32, be: f32, bs: f32) -> Arc<dyn Shape> {
    Arc::new(RoundedCornerShape::new(ts.dp(), te.dp(), be.dp(), bs.dp()))
}

fn scheme() -> ColorScheme {
    light_color_scheme()
}

// ---- The C++ tests by name ------------------------------------------------------------------------------------

/// C++'s dropdown_menu_opens_below_its_anchor_and_closes_outside: under the box it is composed in, its first item's
/// text 12 dp in (the item's padding); a tap outside closes it without reaching the button beneath (a focusable
/// popup), a tap on empty space too; an item's click runs it and closes the menu.
fn dropdown_menu_opens_below_its_anchor_and_closes_outside() {
    let expanded = mutable_state_of(false);
    let log = new_log();
    let (e, l) = (expanded.clone(), log.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (e, l) = (e.clone(), l.clone());
        MaterialTheme(
            content = move || {
                let (e, l) = (e.clone(), l.clone());
                Column(
                    modifier = Modifier.padding(20.dp()),
                    content = move || {
                        let (e1, l1) = (e.clone(), l.clone());
                        Box(
                            modifier = Modifier.test_tag("anchor"),
                            content = move || {
                                let (e2, e3, e4, l2) = (e1.clone(), e1.clone(), e1.clone(), l1.clone());
                                Button(move || e2.set(true), content = || Text("Menü"));
                                DropdownMenu(
                                    e1.get(),
                                    move || e3.set(false),
                                    content = move || {
                                        let (e5, l3, l4) = (e4.clone(), l2.clone(), l2.clone());
                                        DropdownMenuItem(
                                            || Text("Kopyala"),
                                            on_click = move || {
                                                push(&l3, "copy");
                                                e5.set(false);
                                            },
                                        );
                                        DropdownMenuItem(|| Text("Yapıştır"), on_click = move || push(&l4, "paste"));
                                    },
                                );
                            },
                        );
                        let l5 = l.clone();
                        Button(
                            move || push(&l5, "other"),
                            modifier = Modifier.padding_sides(top = 300.dp()).test_tag("other"),
                            content = || Text("Diğer"),
                        );
                    },
                );
            },
        );
    }));
    rule.on_node_with_text("Kopyala").assert_does_not_exist();
    rule.on_node_with_text("Menü").perform_click();
    let anchor = bounds(&rule, "anchor");
    let copy = text_bounds(&rule, "Kopyala");
    assert!(near(copy.left, anchor.left + 12.0, 0.5) && copy.top > anchor.bottom, "{anchor:?} {copy:?}");
    rule.on_node_with_text("Yapıştır").assert_exists();
    tap(&rule, text_bounds(&rule, "Diğer").center()); // outside: closes, the button doesn't fire
    rule.wait_for_idle();
    assert!(taken(&log).is_empty());
    assert!(!expanded.peek());
    rule.on_node_with_text("Diğer").perform_click(); // (it does fire once the menu is closed)
    assert_eq!(taken(&log), ["other"]);
    rule.on_node_with_text("Menü").perform_click();
    tap(&rule, Offset::new(380.0, 780.0)); // empty space closes it too
    rule.wait_for_idle();
    assert!(taken(&log).is_empty() && !expanded.peek());
    rule.on_node_with_text("Kopyala").assert_does_not_exist();
    rule.on_node_with_text("Menü").perform_click();
    rule.on_node_with_text("Kopyala").perform_click();
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["copy"]);
    rule.on_node_with_text("Kopyala").assert_does_not_exist();
}

/// C++'s rtl_from_the_host_and_menus_under_the_anchors_end: the host's direction right to left mirrors the layout;
/// a menu opened then has its end (its right) under its anchor's. Compose's numbers: the menu is its item's least
/// width, 112 dp (the C++ checked only "wider than its 100 dp anchor").
fn rtl_from_the_host_and_menus_under_the_anchors_end() {
    let open = mutable_state_of(false);
    let o = open.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let o = o.clone();
        MaterialTheme(
            content = move || {
                let o = o.clone();
                Column(
                    modifier = Modifier.fill_max_size(),
                    content = move || {
                        Row(content = || {
                            Box(modifier = Modifier.size(20.dp()).test_tag("ilk"));
                            Box(modifier = Modifier.size(20.dp()));
                        });
                        let o = o.clone();
                        Box(
                            modifier = Modifier.fill_max_width(),
                            content_alignment = Alignment::Center,
                            content = move || {
                                let o = o.clone();
                                Box(
                                    modifier = Modifier.size_wh(100.dp(), 40.dp()).test_tag("çapa"),
                                    content = move || {
                                        let o2 = o.clone();
                                        DropdownMenu(
                                            o.get(),
                                            move || o2.set(false),
                                            modifier = Modifier.test_tag("menü"),
                                            content = || {
                                                DropdownMenuItem(|| Text("Kısa"), on_click = || {});
                                            },
                                        );
                                    },
                                );
                            },
                        );
                    },
                );
            },
        );
    }));
    assert!(near(bounds(&rule, "ilk").left, 0.0, 0.5));
    rule.host().set_layout_direction(LayoutDirection::Rtl); // the system's language is written right to left
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "ilk").right, 400.0, 0.5));
    open.set(true);
    rule.wait_for_idle();
    let (anchor, menu) = (bounds(&rule, "çapa"), bounds(&rule, "menü"));
    assert!(near(anchor.right, 250.0, 0.5), "{anchor:?}");
    assert!(near(menu.width(), 112.0, 0.5) && near(menu.right, anchor.right, 1.0), "{anchor:?} {menu:?}");
    assert!(menu.top >= anchor.bottom, "{anchor:?} {menu:?}");
    assert!(
        is_rtl_language("ar")
            && is_rtl_language("ar-EG")
            && is_rtl_language("he_IL")
            && is_rtl_language("FA")
            && is_rtl_language("ur-PK")
            && is_rtl_language("iw")
    );
    assert!(!is_rtl_language("tr-TR") && !is_rtl_language("en") && !is_rtl_language("") && !is_rtl_language("arn"));
}

/// C++'s expressive_menu_groups_and_checked_and_selected_items, with Compose's numbers: the groups' shapes are
/// round (16 dp) at the menu's ends and small (8 dp, the theme's small corners; the C++ had 4) where they meet,
/// the items' 12 and 4; the groups are the menu's cards (surface container low), as wide as the menu, 2 dp apart,
/// the first under the box the popup is composed in (its content's bottom: Compose's popup anchor is the parent's
/// inner bounds, so 20 dp in each way — the C++ placed it under the padding, 8 dp off the window's edge). A checked
/// item sits inset 4 dp in its group on tertiary container in its 12 dp selected shape, its text on tertiary
/// container; its check shows only where a checked leading icon is given (Compose's; the C++ always drew one). The
/// supporting text is in the item's text color (Compose's content color; the C++ had on surface variant) and body
/// medium. Checkable items are checkboxes, selectable ones radio buttons.
fn expressive_menu_groups_and_checked_and_selected_items() {
    let open = mutable_state_of(true);
    let bold = mutable_state_of(true);
    let italic = mutable_state_of(false);
    let align = mutable_state_of(0);
    let settings = Rc::new(Cell::new(0));
    let cs = scheme();
    let (o, b, it, a, s) = (open.clone(), bold.clone(), italic.clone(), align.clone(), settings.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, b, it, a, s) = (o.clone(), b.clone(), it.clone(), a.clone(), s.clone());
        MaterialTheme(
            content = move || {
                let (o, b, it, a, s) = (o.clone(), b.clone(), it.clone(), a.clone(), s.clone());
                Box(
                    modifier = Modifier.padding(20.dp()),
                    content = move || {
                        Box(modifier = Modifier.size(40.dp()));
                        let (o2, b, it, a, s) = (o.clone(), b.clone(), it.clone(), a.clone(), s.clone());
                        DropdownMenuPopup(
                            o.get(),
                            move || o2.set(false),
                            content = move || {
                                let (b, it, a, s) = (b.clone(), it.clone(), a.clone(), s.clone());
                                DropdownMenuGroup(
                                    MenuDefaults::group_shape(0, 3),
                                    modifier = Modifier.test_tag("g0"),
                                    content = move || {
                                        let (b2, it2) = (b.clone(), it.clone());
                                        CheckableDropdownMenuItem(
                                            b.get(),
                                            move |v| b2.set(v),
                                            || Text("Kalın"),
                                            MenuDefaults::item_shape(0, 2),
                                            checked_leading_icon = slot(|| VectorIcon(Icons::Filled::Check(), None)),
                                        );
                                        CheckableDropdownMenuItem(
                                            it.get(),
                                            move |v| it2.set(v),
                                            || Text("İtalik"),
                                            MenuDefaults::item_shape(1, 2),
                                        );
                                    },
                                );
                                Spacer(Modifier.height(MenuDefaults::GroupSpacing));
                                DropdownMenuGroup(
                                    MenuDefaults::group_shape(1, 3),
                                    modifier = Modifier.test_tag("g1"),
                                    content = move || {
                                        for (i, name) in ["Sola", "Ortaya", "Sağa"].into_iter().enumerate() {
                                            let a2 = a.clone();
                                            SelectableDropdownMenuItem(
                                                a.get() == i,
                                                move || a2.set(i),
                                                move || Text(name),
                                                MenuDefaults::item_shape(i as i32, 3),
                                                selected_leading_icon =
                                                    slot(|| VectorIcon(Icons::Filled::Check(), None)),
                                            );
                                        }
                                    },
                                );
                                Spacer(Modifier.height(MenuDefaults::GroupSpacing));
                                DropdownMenuGroup(
                                    MenuDefaults::group_shape(2, 3),
                                    modifier = Modifier.test_tag("g2"),
                                    content = move || {
                                        let s = s.clone();
                                        ShapedDropdownMenuItem(
                                            move || s.set(s.get() + 1),
                                            || Text("Ayarlar"),
                                            MenuDefaults::item_shape(0, 1).shape,
                                            supporting_text = slot(|| Text("Görünüm ve yazı")),
                                        );
                                    },
                                );
                            },
                        );
                    },
                );
            },
        );
    }));
    // Shapes: round at the menu's (and a group's) ends, small where they meet.
    let shapes = Rc::new(RefCell::new(Vec::new()));
    let sh = shapes.clone();
    let probe = KompozTestRule::new(kompozable_lambda!(move || {
        let sh = sh.clone();
        MaterialTheme(
            content = move || {
                *sh.borrow_mut() = vec![
                    MenuDefaults::group_shape(0, 3).shape,
                    MenuDefaults::group_shape(1, 3).shape,
                    MenuDefaults::group_shape(2, 3).shape,
                    MenuDefaults::group_shape(0, 1).shape,
                    MenuDefaults::item_shape(0, 2).shape,
                    MenuDefaults::item_shape(1, 3).shape,
                ];
            },
        );
    }));
    drop(probe);
    let shapes = shapes.borrow();
    assert!(*shapes[0] == *rounded(16.0, 16.0, 8.0, 8.0) && *shapes[1] == *rounded(8.0, 8.0, 8.0, 8.0), "{shapes:?}");
    assert!(*shapes[2] == *rounded(8.0, 8.0, 16.0, 16.0) && *shapes[3] == *rounded(16.0, 16.0, 16.0, 16.0));
    assert!(*shapes[4] == *rounded(12.0, 12.0, 4.0, 4.0) && *shapes[5] == *rounded(4.0, 4.0, 4.0, 4.0));
    // The groups are the cards, as wide as the menu and 2 dp apart; no card behind them.
    let (g0, g1, g2) = (bounds(&rule, "g0"), bounds(&rule, "g1"), bounds(&rule, "g2"));
    assert!(near(g1.top, g0.bottom + 2.0, 0.5) && near(g2.top, g1.bottom + 2.0, 0.5), "{g0:?} {g1:?} {g2:?}");
    assert!(near(g0.width(), g1.width(), 0.5) && near(g1.width(), g2.width(), 0.5));
    assert!(near(g0.top, 60.0, 0.5) && near(g0.left, 20.0, 0.5), "{g0:?}");
    let c = rule.draw();
    // (corners of two sizes: drawn as a path, a point d px in from a corner being in it for a radius under 3.41 d)
    let card = fill_path(&c, g0, cs.surface_container_low).expect("the first group's card");
    let path = &c.path_shapes[card];
    assert!(same_rect(c.paths[card], g0), "{:?}", c.paths[card]);
    assert!(!corner_in(path, 4.0, false) && corner_in(path, 4.0, true) && !corner_in(path, 2.0, true)); // 16, 8
    let card = fill_path(&c, g2, cs.surface_container_low).expect("the last group's card");
    let path = &c.path_shapes[card];
    assert!(corner_in(path, 4.0, false) && !corner_in(path, 2.0, false) && !corner_in(path, 4.0, true)); // 8, 16
    let card = fill(&c, g1, cs.surface_container_low).expect("the middle group's card");
    assert!(same_rect(c.rects[card], g1) && c.rect_corners[card] == corners(8.0, 8.0, 8.0, 8.0));
    assert!(fill(&c, g0, cs.surface_container).is_none());
    // Checked: on tertiary container, 12 dp round, inset 4 dp in its group, a check before it.
    let (bold_row, italic_row) = (text_bounds(&rule, "Kalın"), text_bounds(&rule, "İtalik"));
    let around = |row: Rect, group: Rect| Rect::new(group.left, row.top - 10.0, group.right, row.bottom + 10.0);
    let on = fill(&c, around(bold_row, g0), cs.tertiary_container).expect("the checked item's container");
    assert!(near(c.rects[on].left, g0.left + 4.0, 0.5) && near(c.rects[on].width(), g0.width() - 8.0, 0.5));
    assert_eq!(c.rect_corners[on], corners(12.0, 12.0, 12.0, 12.0));
    // (its 24 dp check makes it 48 tall; italic, without icons, is the items' least height, 44 dp: its container
    // the group's color, in its trailing shape — a path)
    assert!(near(c.rects[on].height(), 48.0, 0.5), "{:?} {bold_row:?}", c.rects[on]);
    let off = fill_path(&c, around(italic_row, g0), cs.surface_container_low).expect("the unchecked container");
    assert!(near(c.paths[off].height(), 44.0, 0.5) && near(c.paths[off].left, g0.left + 4.0, 0.5));
    assert!(fill(&c, around(italic_row, g0), cs.tertiary_container).is_none());
    assert_eq!(text_colors(&c, "Kalın"), [cs.on_tertiary_container]);
    assert_eq!(text_colors(&c, "İtalik"), [cs.on_surface]);
    // (bold's check and the left alignment's; italic gave none)
    assert_eq!(c.vector_colors.iter().filter(|&&v| v == cs.on_tertiary_container).count(), 2);
    // (a selectable surface: checked is selected to accessibility, as Compose's — the C++ told on and off)
    rule.on_node_with_text("Kalın").assert_is_selected();
    rule.on_node_with_text("İtalik").assert_is_not_selected().perform_click();
    rule.on_node_with_text("Kalın").perform_click();
    rule.wait_for_idle();
    assert!(italic.peek() && !bold.peek() && open.peek());
    rule.on_node_with_text("Kalın").assert(has_role(Role::Checkbox));
    // Selected: one of several.
    rule.on_node_with_text("Sola").assert_is_selected().assert(has_role(Role::RadioButton));
    rule.on_node_with_text("Sağa").perform_click();
    rule.wait_for_idle();
    assert_eq!(align.peek(), 2);
    rule.on_node_with_text("Sağa").assert_is_selected();
    rule.on_node_with_text("Sola").assert_is_not_selected();
    let c = rule.draw();
    let right = text_bounds(&rule, "Sağa");
    assert!(fill(&c, around(right, g1), cs.tertiary_container).is_some());
    // A supporting line under the text: the text label large, the line body medium, both in the item's text color.
    let (title, line) = (text_bounds(&rule, "Ayarlar"), text_bounds(&rule, "Görünüm ve yazı"));
    assert!(line.top >= title.bottom - 0.5 && near(line.left, title.left, 0.5), "{title:?} {line:?}");
    let t = Typography::default();
    for i in 0..c.texts.len() {
        if c.texts[i] == "Görünüm ve yazı" {
            assert!(c.colors[i] == cs.on_surface && c.text_styles[i].font_size == t.body_medium.font_size.value());
        }
        if c.texts[i] == "Ayarlar" {
            assert_eq!(c.text_styles[i].font_size, t.label_large.font_size.value());
        }
    }
    rule.on_node_with_text("Ayarlar").assert(has_role(Role::Button)).perform_click();
    assert_eq!(settings.get(), 1);
}

/// C++'s exposed_dropdown_menu_picks_an_option, with Compose's numbers: a read-only field anchoring the menu
/// (`menu_anchor(PrimaryNotEditable)`) is a dropdown list to accessibility; a click opens the menu (one overlay),
/// the trailing arrow turned up (Compose's has no description: the C++ named it "Close"); the menu is under the
/// field at its width, and no taller than the window leaves below it (800 − 48 − 76: 676 dp, Compose's
/// `calculateMaxHeight`; the C++ had half the window); it scrolls. The field is focused, typing changes nothing; an
/// item's click picks it and closes the menu; the field's click opens and closes it.
fn exposed_dropdown_menu_picks_an_option() {
    let open = mutable_state_of(false);
    let choice = mutable_state_of("Ankara".to_string());
    let mut cities: Vec<String> = (0..30).map(|i| format!("Şehir {i}")).collect();
    cities[1] = "Bursa".to_string();
    let cities = Rc::new(cities);
    let (o, ch, ci) = (open.clone(), choice.clone(), cities.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, ch, ci) = (o.clone(), ch.clone(), ci.clone());
        MaterialTheme(
            content = move || {
                let (o, ch, ci) = (o.clone(), ch.clone(), ci.clone());
                Column(
                    modifier = Modifier.padding(20.dp()),
                    content = move || {
                        let (o, o2, ch, ci) = (o.clone(), o.clone(), ch.clone(), ci.clone());
                        ExposedDropdownMenuBox(
                            o.get(),
                            move |v| o2.set(v),
                            modifier = Modifier.test_tag("box"),
                            content = move |scope: &ExposedDropdownMenuBoxScope| {
                                let (ch2, o3, o4) = (ch.clone(), o.clone(), o.clone());
                                StringTextField(
                                    ch.get(),
                                    move |v| ch2.set(v),
                                    modifier = scope
                                        .menu_anchor(Modifier, ExposedDropdownMenuAnchorType::PrimaryNotEditable)
                                        .fill_max_width()
                                        .test_tag("alan"),
                                    read_only = true,
                                    label = slot(|| Text("Şehir")),
                                    trailing_icon = slot(move || ExposedDropdownMenuDefaults.TrailingIcon(o3.get())),
                                );
                                let (o5, ch, ci) = (o.clone(), ch.clone(), ci.clone());
                                scope.ExposedDropdownMenu(
                                    o.get(),
                                    move || o4.set(false),
                                    modifier = Modifier.test_tag("menü"),
                                    content = move || {
                                        for city in ci.iter() {
                                            let (city, ch, o) = (city.clone(), ch.clone(), o5.clone());
                                            let text = city.clone();
                                            DropdownMenuItem(
                                                move || Text(text.clone()),
                                                on_click = move || {
                                                    ch.set(city.clone());
                                                    o.set(false);
                                                },
                                            );
                                        }
                                    },
                                );
                            },
                        );
                    },
                );
            },
        );
    }));
    let field = bounds(&rule, "alan");
    rule.on_node_with_tag("alan").assert(has_role(Role::DropdownList));
    tap(&rule, field.center()); // the field
    assert!(open.peek() && rule.host().overlay_count() == 1, "{}", rule.host().overlay_count());
    let c = rule.draw();
    // (the arrow drawn turned half round: its layer's transform)
    assert!(
        c.transforms.iter().any(|m| near(m.values[0], -1.0, 1e-4) && near(m.values[5], -1.0, 1e-4)),
        "the arrow turned up {:?}",
        c.transforms
    );
    // Under the field, at its width; it scrolls rather than run off the window.
    let menu = bounds(&rule, "menü");
    let item = text_bounds(&rule, "Bursa");
    let box_ = bounds(&rule, "box");
    assert!(near(menu.width(), box_.width(), 0.5) && near(menu.left, box_.left, 0.5), "{menu:?} {box_:?}");
    assert!(near(item.left, box_.left + 12.0, 0.5), "{item:?}");
    assert!(menu.top >= box_.bottom - 1.0, "{menu:?} {box_:?}");
    assert!(near(box_.bottom, 76.0, 0.5), "{box_:?}");
    // (the surface around the column: the column's 676 tall, 8 dp padded inside)
    assert!(near(menu.height(), 676.0, 0.5), "{menu:?}");
    // Read-only: focused, but typing changes nothing.
    rule.host().text_input("x");
    rule.wait_for_idle();
    assert!(choice.peek() == "Ankara" && rule.host().focused_node().is_some());
    rule.on_node_with_text("Bursa").perform_click(); // the menu's item
    rule.wait_for_idle();
    assert!(choice.peek() == "Bursa" && !open.peek() && rule.host().overlay_count() == 0);
    tap(&rule, field.center()); // now the field: opens...
    assert!(open.peek());
    tap(&rule, field.center()); // ...and closes (the field is where the press goes: the popup is under it)
    assert!(!open.peek(), "closed");
    rule.wait_for_idle();
    assert!(rule.host().overlay_count() == 0 && choice.peek() == "Bursa");
}

/// C++'s exposed_dropdown_menu_filters_as_you_type: an edited field anchoring the menu
/// (`menu_anchor(PrimaryEditable)`) shows the matches as one types; the menu doesn't take the focus (Compose's
/// popup isn't focusable for an editable anchor), so the field keeps typing; a pick closes it.
fn exposed_dropdown_menu_filters_as_you_type() {
    let open = mutable_state_of(false);
    let query = mutable_state_of(String::new());
    let cities = ["Adana", "Ankara", "Antalya", "Bursa", "İzmir"];
    let (o, q) = (open.clone(), query.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, q) = (o.clone(), q.clone());
        MaterialTheme(
            content = move || {
                let (o, o2, q) = (o.clone(), o.clone(), q.clone());
                ExposedDropdownMenuBox(
                    o.get(),
                    move |v| o2.set(v),
                    modifier = Modifier.padding(20.dp()),
                    content = move |scope: &ExposedDropdownMenuBoxScope| {
                        let (q2, o3, o4) = (q.clone(), o.clone(), o.clone());
                        StringTextField(
                            q.get(),
                            move |v| {
                                q2.set(v);
                                o3.set(true); // typing shows the matches
                            },
                            modifier = scope
                                .menu_anchor(Modifier, ExposedDropdownMenuAnchorType::PrimaryEditable)
                                .fill_max_width()
                                .test_tag("field"),
                            label = slot(|| Text("Şehir")),
                        );
                        let (q, o5) = (q.clone(), o.clone());
                        scope.ExposedDropdownMenu(
                            o.get(),
                            move || o4.set(false),
                            content = move || {
                                for city in cities {
                                    if city.starts_with(q.get().as_str()) {
                                        let (q, o) = (q.clone(), o5.clone());
                                        DropdownMenuItem(
                                            move || Text(city),
                                            on_click = move || {
                                                q.set(city.to_string());
                                                o.set(false);
                                            },
                                        );
                                    }
                                }
                            },
                        );
                    },
                );
            },
        );
    }));
    rule.on_node_with_tag("field").perform_text_input("A");
    rule.wait_for_idle();
    assert!(open.peek());
    rule.on_node_with_text("Adana").assert_exists();
    rule.on_node_with_text("Bursa").assert_does_not_exist();
    rule.host().text_input("n"); // the field keeps the keyboard while the menu is open
    rule.wait_for_idle();
    assert_eq!(query.peek(), "An");
    rule.on_node_with_text("Antalya").assert_exists();
    rule.on_node_with_text("Adana").assert_does_not_exist();
    rule.on_node_with_text("Antalya").perform_click();
    rule.wait_for_idle();
    assert!(query.peek() == "Antalya" && !open.peek());
}

/// C++'s library_strings_follow_the_hosts_language_and_local_overrides, its Material part with Compose's texts: the
/// trailing icon has no description in Compose, so the texts checked are a secondary editable anchor's (Compose's
/// `MenuExpanded`/`MenuCollapsed` state and `ToggleDropdownMenu` description): English by default, Turkish once the
/// host's strings are, and a local override's language (German) for what is in it. (KompozR's own Strings — the
/// C++'s `forLanguage`, `turkish()` — are kompoz-ui's, tested there.)
fn library_strings_follow_the_hosts_language_and_local_overrides() {
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        MaterialTheme(
            content = move || {
                Column(
                    content = || {
                        Anchored("bir");
                        let german = Strings::for_language("de_DE.UTF-8");
                        KompozitionLocalProvider(&[LocalStrings.provides(german)], || Anchored("iki"));
                    },
                );
            },
        );
    }));
    let description = |tag: &str| {
        rule.on_node_with_tag(tag).fetch_semantics_node().config().get(SemanticsProperties::ContentDescription)
    };
    let state = |tag: &str| {
        rule.on_node_with_tag(tag).fetch_semantics_node().config().get(SemanticsProperties::StateDescription)
    };
    assert_eq!(description("bir"), Some(vec!["Toggle dropdown menu".to_string()]));
    assert_eq!(state("bir").as_deref(), Some("Collapsed"));
    assert_eq!(description("iki"), Some(vec!["Drop-down-Menü maximieren/minimieren".to_string()]));
    assert_eq!(state("iki").as_deref(), Some("Minimiert"));
    rule.host().set_strings(Strings::turkish()); // a Turkish system
    rule.wait_for_idle();
    assert_eq!(description("bir"), Some(vec!["Açılır menüyü açın/kapatın".to_string()]));
    assert_eq!(state("bir").as_deref(), Some("Daraltıldı"));
    assert_eq!(state("iki").as_deref(), Some("Minimiert"));
    rule.on_node_with_tag("bir").perform_click();
    assert_eq!(state("bir").as_deref(), Some("Genişletildi"));
    rule.host().set_strings(Strings::for_language("en-US"));
    rule.wait_for_idle();
    assert_eq!(state("bir").as_deref(), Some("Expanded"));
}

/// A box with a secondary editable anchor (an icon of an edited field), its menu empty.
#[kompozable]
fn Anchored(tag: &'static str) {
    let open = remember(|| mutable_state_of(false));
    let o = open.clone();
    ExposedDropdownMenuBox(
        open.get(),
        move |v| o.set(v),
        content = move |scope: &ExposedDropdownMenuBoxScope| {
            Box(modifier = scope
                .menu_anchor(Modifier, ExposedDropdownMenuAnchorType::SecondaryEditable)
                .size(24.dp())
                .test_tag(tag));
        },
    );
}

/// The menu part of C++'s overlays_fields_and_indicators_animate_composing_around_them_only_as_they_turn (whole in
/// cross_area.rs): a DropdownMenu with a
/// checkable item and the exposed menu's trailing icon are composed as they turn, not while they scale in, nor
/// while an item checked fades its container's color.
fn menus_compose_around_them_only_as_they_open_and_check() {
    let open = mutable_state_of(false);
    let mark = mutable_state_of(false);
    let composed = Rc::new(Cell::new(0));
    let (o, m, c) = (open.clone(), mark.clone(), composed.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        c.set(c.get() + 1);
        let o = o.get();
        let m = m.clone();
        MaterialTheme(
            content = move || {
                let m = m.clone();
                Column(
                    content = move || {
                        let m = m.clone();
                        Box(content = move || {
                            let m = m.clone();
                            DropdownMenu(
                                o,
                                || {},
                                content = move || {
                                    Text("Seçenek");
                                    let m2 = m.clone();
                                    CheckableDropdownMenuItem(
                                        m.get(),
                                        move |v| m2.set(v),
                                        || Text("İşaretli"),
                                        MenuDefaults.item_shapes(),
                                    );
                                },
                            );
                        });
                        Row(content = move || {
                            ExposedDropdownMenuDefaults.TrailingIcon(o);
                        });
                    },
                );
            },
        );
    }));
    rule.on_node_with_text("Seçenek").assert_does_not_exist();
    open.set(true);
    rule.advance_frame();
    rule.advance_frame(); // composed as it turns...
    let before = composed.get();
    rule.advance_time_by(48);
    assert_eq!(composed.get(), before); // ...and not on the way
    rule.advance_time_by(700);
    rule.on_node_with_text("Seçenek").assert_exists();
    // An item checked: composed as it turns, not as its color goes over.
    mark.set(true);
    rule.advance_frame();
    rule.advance_frame();
    let before = composed.get();
    rule.advance_time_by(48);
    assert_eq!(composed.get(), before);
    // Closed: gone once it is out.
    open.set(false);
    rule.wait_for_idle();
    rule.on_node_with_text("Seçenek").assert_does_not_exist();
}

// ---- Compose's behaviour ---------------------------------------------------------------------------------------

/// A menu of three standard items under a 100 by 40 anchor 20 dp in, open as `open` says.
#[kompozable]
fn ThreeItemMenu(open: MutableState<bool>, modifier: Modifier) {
    let o = open.clone();
    Box(
        modifier = Modifier.padding(20.dp()).size_wh(100.dp(), 40.dp()).test_tag("çapa"),
        content = move || {
            let o2 = o.clone();
            DropdownMenu(
                o.get(),
                move || o2.set(false),
                modifier = modifier.clone(),
                content = || {
                    DropdownMenuItem(|| Text("Bir"), on_click = || {});
                    DropdownMenuItem(|| Text("İki"), on_click = || {});
                    DropdownMenuItem(|| Text("Üç"), on_click = || {});
                },
            );
        },
    );
}

/// Compose for desktop's `handleDropdownOnKeyEvent`: in an open menu, Down moves the focus to the next item and Up
/// to the previous one, the input mode turned to the keyboard's; the Expressive popup's the same.
fn arrow_keys_move_the_focus_through_a_menus_items() {
    let open = mutable_state_of(true);
    let mode = Rc::new(Cell::new(kompoz_ui::platform::InputMode::Touch));
    let (o, m) = (open.clone(), mode.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, m) = (o.clone(), m.clone());
        MaterialTheme(
            content = move || {
                m.set(kompoz_ui::platform::LocalInputModeManager.current().input_mode());
                ThreeItemMenu(o.clone(), Modifier);
            },
        );
    }));
    rule.wait_for_idle();
    rule.on_node_with_text("Bir").assert_is_not_focused();
    key(&rule, Key::DirectionDown);
    rule.on_node_with_text("Bir").assert_is_focused();
    assert_eq!(mode.get(), kompoz_ui::platform::InputMode::Keyboard);
    key(&rule, Key::DirectionDown);
    rule.on_node_with_text("İki").assert_is_focused();
    rule.on_node_with_text("Bir").assert_is_not_focused();
    key(&rule, Key::DirectionDown);
    key(&rule, Key::DirectionUp);
    rule.on_node_with_text("İki").assert_is_focused();
    // (other keys aren't the menu's: Enter on the focused item clicks it, the clickable's)
    assert!(open.peek());

    // The Expressive popup.
    let checked = mutable_state_of(false);
    let ch = checked.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let ch = ch.clone();
        MaterialTheme(
            content = move || {
                let ch = ch.clone();
                Box(content = move || {
                    let ch = ch.clone();
                    DropdownMenuPopup(
                        true,
                        || {},
                        content = move || {
                            let ch = ch.clone();
                            DropdownMenuGroup(
                                MenuDefaults.group_shapes(),
                                content = move || {
                                    let ch2 = ch.clone();
                                    CheckableDropdownMenuItem(
                                        ch.get(),
                                        move |v| ch2.set(v),
                                        || Text("Bir"),
                                        MenuDefaults::item_shape(0, 2),
                                    );
                                    SelectableDropdownMenuItem(
                                        false,
                                        || {},
                                        || Text("İki"),
                                        MenuDefaults::item_shape(1, 2),
                                    );
                                },
                            );
                        },
                    );
                });
            },
        );
    }));
    key(&rule, Key::DirectionDown);
    rule.on_node_with_text("Bir").assert_is_focused();
    key(&rule, Key::DirectionDown);
    rule.on_node_with_text("İki").assert_is_focused();
    key(&rule, Key::DirectionUp);
    key(&rule, Key::Enter);
    assert!(checked.peek());
}

/// Compose's `DropdownMenuContent` and `calculateTransformOrigin`: a menu opening scales from 0.8 and fades in from
/// nothing, about where it meets its anchor — its top edge where it is below the anchor (the middle of their
/// overlap across), its bottom edge where it is above; closing goes the other way, then the popup goes.
fn menus_scale_and_fade_in_from_where_they_meet_their_anchor() {
    let open = mutable_state_of(false);
    let at_bottom = mutable_state_of(false);
    let (o, b) = (open.clone(), at_bottom.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, b) = (o.clone(), b.clone());
        MaterialTheme(
            content = move || {
                let o = o.clone();
                Column(
                    content = move || {
                        if b.get() {
                            Spacer(Modifier.height(700.dp()));
                        }
                        ThreeItemMenu(o.clone(), Modifier.test_tag("menü"));
                    },
                );
            },
        );
    }));
    // The scale and its pivot: the menu's layer transform, x' = s x + (1 − s) pivot.
    let layer = |rule: &KompozTestRule| -> (f32, Offset, f32) {
        let c = rule.draw();
        let m = c.transforms.iter().find(|m| m.values[0] < 0.999 && m.values[0] > 0.0).copied();
        let alpha = c.layer_alphas.iter().copied().find(|a| *a < 0.999).unwrap_or(1.0);
        match m {
            Some(m) => {
                let s = m.values[0];
                (s, Offset::new(m.values[12] / (1.0 - s), m.values[13] / (1.0 - s)), alpha)
            }
            None => (1.0, Offset::Zero, alpha),
        }
    };
    open.set(true);
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_time_by(16);
    let (scale, pivot, alpha) = layer(&rule);
    assert!(scale > 0.79 && scale < 1.0 && alpha < 1.0, "{scale} {alpha}");
    rule.wait_for_idle();
    let (menu, anchor) = (bounds(&rule, "menü"), bounds(&rule, "çapa"));
    // Below its anchor: the top edge; across, the middle of the overlap (20 to 120 with 20 to 132).
    let surface_width = menu.width();
    assert!(near(pivot.y, 0.0, 0.5), "{pivot:?}");
    assert!(near(pivot.x, (70.0 - anchor.left).max(0.0) / surface_width * surface_width, 1.0), "{pivot:?} {menu:?}");
    assert_eq!(layer(&rule).0, 1.0); // there
    open.set(false);
    rule.advance_frame();
    rule.advance_frame();
    // (one frame more than opening: the menu's popup content, the same lambda instance — Compose's memoized one —, is
    // skipped and composed again by the transition state its menu set while composing, which reaches the popup's
    // composition at the next frame here; see stage12-followups "Closing — core")
    rule.advance_frame();
    rule.advance_time_by(16);
    let (scale, _, alpha) = layer(&rule);
    assert!(scale < 1.0 && alpha < 1.0, "{scale} {alpha}");
    rule.on_node_with_text("Bir").assert_exists(); // (on its way out)
    rule.wait_for_idle();
    rule.on_node_with_text("Bir").assert_does_not_exist();
    assert_eq!(rule.host().overlay_count(), 0);
    // Above its anchor (no room below): from its bottom edge.
    at_bottom.set(true);
    rule.wait_for_idle();
    open.set(true);
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_time_by(16);
    let (scale, pivot, _) = layer(&rule);
    rule.wait_for_idle();
    let (menu, anchor) = (bounds(&rule, "menü"), bounds(&rule, "çapa"));
    assert!(near(menu.bottom, anchor.top, 0.5), "{menu:?} {anchor:?}");
    assert!(scale < 1.0 && near(pivot.y, menu.height(), 1.0), "{scale} {pivot:?} {menu:?}");
}

/// Compose's `MenuItemColors`: the six-color constructor leaves the container and selected colors unspecified,
/// `copy` keeps what is unspecified; the defaults are the tokens' (a standard item's list colors, the selectable
/// standard and vibrant ones); an item draws its text and icons in its colors for enabled and selected (a disabled
/// item in its disabled ones, a click doing nothing), its container in its container color.
fn menu_item_colors_follow_enabled_and_selected() {
    let cs = scheme();
    let red = Color::from_argb(0xFFFF0000);
    let six = MenuItemColors(red, red, red, red, red, red);
    assert!(six.container_color.is_unspecified() && six.selected_text_color.is_unspecified());
    let all = named_args!(MenuItemColors(
        red,
        red,
        red,
        red,
        red,
        red,
        container_color = cs.surface,
        selected_container_color = cs.primary
    ));
    assert_eq!(all.selected_container_color, cs.primary);
    let copied = named_args!(all.copy(text_color = cs.secondary, selected_container_color = Color::Unspecified));
    assert!(copied.text_color == cs.secondary && copied.selected_container_color == cs.primary && copied != all);
    assert_eq!(all.copy(), all);

    let defaults = Rc::new(RefCell::new(Vec::new()));
    let clicks = Rc::new(Cell::new(0));
    let (d, k) = (defaults.clone(), clicks.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (d, k) = (d.clone(), k.clone());
        MaterialTheme(
            content = move || {
                *d.borrow_mut() = vec![
                    MenuDefaults.item_colors(),
                    MenuDefaults.selectable_item_colors(),
                    MenuDefaults.selectable_item_vibrant_colors(),
                    MenuDefaults.item_colors(text_color = Color::from_argb(0xFF00FF00)),
                ];
                let (d, k) = (d.clone(), k.clone());
                Box(content = move || {
                    let (d, k) = (d.clone(), k.clone());
                    DropdownMenu(
                        true,
                        || {},
                        content = move || {
                            let k = k.clone();
                            DropdownMenuItem(
                                || Text("Kapalı"),
                                on_click = move || k.set(k.get() + 1),
                                leading_icon = slot(|| VectorIcon(Icons::Filled::Add(), None)),
                                enabled = false,
                            );
                            DropdownMenuItem(
                                || Text("Açık"),
                                on_click = || {},
                                trailing_icon = slot(|| VectorIcon(Icons::Filled::Delete(), None)),
                                colors = d.borrow()[3],
                            );
                            CheckableDropdownMenuItem(
                                true,
                                |_| {},
                                || Text("Seçili kapalı"),
                                MenuDefaults.item_shapes(),
                                enabled = false,
                            );
                            CheckableDropdownMenuItem(
                                true,
                                |_| {},
                                || Text("Canlı"),
                                MenuDefaults.item_shapes(),
                                colors = d.borrow()[2],
                            );
                        },
                    );
                });
            },
        );
    }));
    let d = defaults.borrow().clone();
    let (standard, selectable, vibrant) = (d[0], d[1], d[2]);
    assert!(standard.text_color == cs.on_surface && standard.leading_icon_color == cs.on_surface_variant);
    assert_eq!(standard.disabled_text_color, named_args!(cs.on_surface.copy(alpha = 0.38)));
    assert!(standard.container_color.is_unspecified());
    assert!(
        selectable.container_color == cs.surface_container_low
            && selectable.selected_container_color == cs.tertiary_container
    );
    assert!(
        selectable.selected_text_color == cs.on_tertiary_container
            && selectable.disabled_container_color == cs.surface_container_low
    );
    assert!(vibrant.container_color == cs.tertiary_container && vibrant.selected_container_color == cs.tertiary);
    assert!(vibrant.text_color == cs.on_tertiary_container && vibrant.selected_text_color == cs.on_tertiary);
    assert!(d[3].text_color == Color::from_argb(0xFF00FF00) && d[3].leading_icon_color == standard.leading_icon_color);
    let c = rule.draw();
    let disabled = named_args!(cs.on_surface.copy(alpha = 0.38));
    assert_eq!(text_colors(&c, "Kapalı"), [disabled]);
    assert!(c.vector_colors.contains(&disabled), "the disabled leading icon {:?}", c.vector_colors);
    assert_eq!(text_colors(&c, "Açık"), [Color::from_argb(0xFF00FF00)]);
    assert!(c.vector_colors.contains(&cs.on_surface_variant), "the trailing icon");
    // Checked but disabled: the disabled colors, not the selected ones.
    assert_eq!(text_colors(&c, "Seçili kapalı"), [disabled]);
    let row = text_bounds(&rule, "Seçili kapalı");
    assert!(fill(&c, Rect::new(0.0, row.top - 10.0, 400.0, row.bottom + 10.0), cs.tertiary_container).is_none());
    // Vibrant and checked: on tertiary, its text on tertiary.
    assert_eq!(text_colors(&c, "Canlı"), [cs.on_tertiary]);
    let row = text_bounds(&rule, "Canlı");
    assert!(fill(&c, Rect::new(0.0, row.top - 10.0, 400.0, row.bottom + 10.0), cs.tertiary).is_some());
    rule.on_node_with_text("Kapalı").assert_is_not_enabled().perform_click();
    assert_eq!(clicks.get(), 0);
}

/// Compose's `DropdownMenuGroup` and `shapeByInteraction`: a group keeps its shape while not yet hovered and while
/// hovered; once the pointer leaves it, it morphs to its inactive shape (the theme's small corners, 8 dp) along the
/// fast spatial spec. Shapes given override the defaults.
fn groups_take_their_inactive_shape_once_hovered_and_left() {
    let cs = scheme();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        MaterialTheme(
            content = move || {
                Box(content = move || {
                    DropdownMenuPopup(
                        true,
                        || {},
                        content = move || {
                            DropdownMenuGroup(
                                MenuDefaults.group_shapes(),
                                modifier = Modifier.test_tag("grup"),
                                content = || {
                                    Box(modifier = Modifier.size_wh(150.dp(), 60.dp()));
                                },
                            );
                            Spacer(Modifier.height(MenuDefaults::GroupSpacing));
                            DropdownMenuGroup(
                                named_args!(MenuDefaults.group_shapes(shape = Some(rounded(2.0, 2.0, 2.0, 2.0)))),
                                modifier = Modifier.test_tag("kare"),
                                container_color = MenuDefaults::group_vibrant_container_color(),
                                content = || {
                                    Box(modifier = Modifier.size_wh(150.dp(), 60.dp()));
                                },
                            );
                        },
                    );
                });
            },
        );
    }));
    let group = bounds(&rule, "grup");
    let radius = |rule: &KompozTestRule, color: Color, at: Rect| -> f32 {
        let c = rule.draw();
        let i = fill(&c, at, color).expect("the group's card");
        c.rect_corners[i].top_left
    };
    assert_eq!(radius(&rule, cs.surface_container_low, group), 16.0);
    assert_eq!(radius(&rule, cs.tertiary_container, bounds(&rule, "kare")), 2.0);
    rule.host().pointer_move(group.center(), -1, 0); // hovered: the same
    rule.wait_for_idle();
    assert_eq!(radius(&rule, cs.surface_container_low, group), 16.0);
    rule.host().pointer_move(Offset::new(390.0, 790.0), -1, 0); // left: inactive
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_time_by(32);
    let on_the_way = radius(&rule, cs.surface_container_low, group);
    assert!(on_the_way < 16.0 && on_the_way > 8.0, "{on_the_way}");
    rule.wait_for_idle();
    assert_eq!(radius(&rule, cs.surface_container_low, group), 8.0);
    rule.host().pointer_move(group.center(), -1, 0); // hovered again: its shape again
    rule.wait_for_idle();
    assert_eq!(radius(&rule, cs.surface_container_low, group), 16.0);
}

/// Compose's `calculateMaxHeight` and `ExposedDropdownMenuPositionProvider`: a field low in the window opens its menu
/// above it, its bottom on the field's top, no taller than the room above it (650 − 48: 602 dp); the window made
/// smaller while it is open gives it less (`OnPlatformWindowBoundsChange`). `match_anchor_width = false` keeps the
/// menu as wide as its items; the deprecated `ExposedDropdownMenu(…, focusable, matchTextFieldWidth, …)` is the
/// same menu; `exposed_dropdown_size` sizes a menu of one's own.
fn exposed_menus_go_above_where_there_is_no_room_below() {
    let open = mutable_state_of(false);
    let items = mutable_state_of(5);
    let variant = mutable_state_of(0);
    let (o, n, v) = (open.clone(), items.clone(), variant.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, n, v) = (o.clone(), n.clone(), v.clone());
        MaterialTheme(
            content = move || {
                let (o, n, v) = (o.clone(), n.clone(), v.clone());
                Column(
                    content = move || {
                        Spacer(Modifier.height(650.dp()));
                        let (o, o2, n, v) = (o.clone(), o.clone(), n.clone(), v.clone());
                        ExposedDropdownMenuBox(
                            o.get(),
                            move |x| o2.set(x),
                            modifier = Modifier.width(300.dp()).test_tag("kutu"),
                            content = move |scope: &ExposedDropdownMenuBoxScope| {
                                Box(modifier = scope
                                    .menu_anchor(Modifier, ExposedDropdownMenuAnchorType::PrimaryNotEditable)
                                    .fill_max_width()
                                    .height(56.dp())
                                    .test_tag("alan"));
                                let (o3, count) = (o.clone(), n.get());
                                let items = move || {
                                    for i in 0..count {
                                        DropdownMenuItem(move || Text(format!("Öğe {i}")), on_click = || {});
                                    }
                                };
                                match v.get() {
                                    0 => scope.ExposedDropdownMenu(
                                        o.get(),
                                        move || o3.set(false),
                                        modifier = Modifier.test_tag("menü"),
                                        shape = rounded(10.0, 10.0, 10.0, 10.0),
                                        container_color = Color::from_argb(0xFF00FF00),
                                        content = items,
                                    ),
                                    1 => scope.ExposedDropdownMenu(
                                        o.get(),
                                        move || o3.set(false),
                                        modifier = Modifier.test_tag("menü"),
                                        match_anchor_width = false,
                                        content = items,
                                    ),
                                    2 => scope.FocusableExposedDropdownMenu(
                                        o.get(),
                                        move || o3.set(false),
                                        modifier = Modifier.test_tag("menü"),
                                        focusable = false,
                                        match_text_field_width = true,
                                        content = items,
                                    ),
                                    _ => {
                                        if o.get() {
                                            DropdownMenuPopup(
                                                true,
                                                move || o3.set(false),
                                                modifier = scope.exposed_dropdown_size(Modifier.test_tag("menü")),
                                                content = items,
                                            );
                                        }
                                    }
                                }
                            },
                        );
                    },
                );
            },
        );
    }));
    let field = bounds(&rule, "alan");
    assert!(near(field.top, 650.0, 0.5) && near(field.bottom, 706.0, 0.5));
    tap(&rule, field.center());
    let menu = bounds(&rule, "menü");
    assert!(near(menu.bottom, field.top, 0.5) && near(menu.width(), 300.0, 0.5), "{menu:?} {field:?}");
    let c = rule.draw();
    let card = fill(&c, menu, Color::from_argb(0xFF00FF00)).expect("the menu's container");
    assert_eq!(c.rect_corners[card], corners(10.0, 10.0, 10.0, 10.0)); // (its shape and color)
    items.set(30);
    rule.wait_for_idle();
    let menu = bounds(&rule, "menü");
    assert!(near(menu.height(), 602.0, 0.5) && near(menu.top, 48.0, 0.5), "{menu:?}");
    rule.host().set_viewport_size(IntSize::new(400, 760)); // a smaller window: less room above
    rule.wait_for_idle();
    let menu = bounds(&rule, "menü");
    assert!(near(menu.height(), 602.0, 0.5), "{menu:?}"); // (above it: 650 − 48 still the most)
    rule.host().set_viewport_size(IntSize::new(400, 800));
    rule.wait_for_idle();
    variant.set(1);
    items.set(3);
    rule.wait_for_idle();
    let menu = bounds(&rule, "menü");
    assert!(near(menu.width(), 112.0, 0.5), "{menu:?}"); // as wide as its items
    variant.set(2);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "menü").width(), 300.0, 0.5));
    variant.set(3);
    rule.wait_for_idle();
    let menu = bounds(&rule, "menü");
    assert!(near(menu.width(), 300.0, 0.5), "{menu:?}"); // a popup of one's own, sized as the box's menu
}

/// Compose's `menuAnchor`, `expandable` and Compose for desktop's `popupPropertiesForAnchorType`: a primary anchor
/// that isn't edited (the deprecated `menuAnchor()` too) opens a focusable menu — a tap outside it reaches nothing
/// beneath; an edited one a menu that isn't — the tap reaches what is beneath — until the keyboard goes for the
/// menu (Down), which makes it focusable. A disabled anchor neither opens the menu nor tells it is a dropdown list.
/// Enter and Space click an anchor that isn't edited; an edited one takes Enter only.
fn anchor_types_decide_whether_the_menu_takes_the_focus() {
    let open = mutable_state_of(false);
    let anchor = mutable_state_of(0);
    let log = new_log();
    let (o, a, l) = (open.clone(), anchor.clone(), log.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, a, l) = (o.clone(), a.clone(), l.clone());
        MaterialTheme(
            content = move || {
                let (o, a, l) = (o.clone(), a.clone(), l.clone());
                Column(
                    content = move || {
                        let (o, o2, a) = (o.clone(), o.clone(), a.clone());
                        ExposedDropdownMenuBox(
                            o.get(),
                            move |x| o2.set(x),
                            content = move |scope: &ExposedDropdownMenuBoxScope| {
                                let m = Modifier.size_wh(200.dp(), 56.dp());
                                let m = match a.get() {
                                    0 => scope.menu_anchor(m, ExposedDropdownMenuAnchorType::PrimaryNotEditable),
                                    1 => scope.menu_anchor(m, ExposedDropdownMenuAnchorType::PrimaryEditable),
                                    2 => scope.menu_anchor_untyped(m),
                                    _ => named_args!(scope.menu_anchor(
                                        m,
                                        ExposedDropdownMenuAnchorType::PrimaryNotEditable,
                                        enabled = false
                                    )),
                                };
                                Box(modifier = m.focusable().test_tag("çapa"));
                                let o3 = o.clone();
                                scope.ExposedDropdownMenu(
                                    o.get(),
                                    move || o3.set(false),
                                    content = || {
                                        DropdownMenuItem(|| Text("Seçenek"), on_click = || {});
                                    },
                                );
                            },
                        );
                        let l = l.clone();
                        Button(
                            move || push(&l, "altta"),
                            modifier = Modifier.padding_sides(top = 300.dp()),
                            content = || Text("Altta"),
                        );
                    },
                );
            },
        );
    }));
    let below = text_bounds(&rule, "Altta").center();
    let anchor_center = bounds(&rule, "çapa").center();
    rule.on_node_with_tag("çapa").assert(has_role(Role::DropdownList));
    // Not edited: focusable.
    tap(&rule, anchor_center);
    assert!(open.peek());
    tap(&rule, below);
    assert!(!open.peek() && taken(&log).is_empty());
    // Edited: not focusable, the tap goes on.
    anchor.set(1);
    rule.wait_for_idle();
    tap(&rule, anchor_center);
    assert!(open.peek());
    tap(&rule, below);
    assert!(!open.peek());
    assert_eq!(taken(&log), ["altta"]);
    // ...until the keyboard goes for the menu.
    tap(&rule, anchor_center);
    rule.on_node_with_tag("çapa").request_focus();
    rule.wait_for_idle();
    key(&rule, Key::DirectionDown);
    tap(&rule, below);
    assert!(!open.peek() && taken(&log).is_empty());
    // Keys: Enter opens an edited anchor's menu, Space doesn't.
    rule.on_node_with_tag("çapa").request_focus();
    key(&rule, Key::Spacebar);
    assert!(!open.peek());
    key(&rule, Key::Enter);
    assert!(open.peek());
    open.set(false);
    rule.wait_for_idle();
    // The deprecated `menuAnchor()`: not edited — Space opens it, the menu is focusable.
    anchor.set(2);
    rule.wait_for_idle();
    rule.on_node_with_tag("çapa").request_focus();
    key(&rule, Key::Spacebar);
    assert!(open.peek());
    tap(&rule, below);
    assert!(!open.peek() && taken(&log).is_empty());
    // Disabled: no dropdown list, a tap opens nothing.
    anchor.set(3);
    rule.wait_for_idle();
    rule.on_node_with_tag("çapa").assert(!has_role(Role::DropdownList));
    tap(&rule, anchor_center);
    assert!(!open.peek());
    assert_eq!(format!("{}", ExposedDropdownMenuAnchorType::SecondaryEditable), "SecondaryEditable");
    assert_eq!(format!("{:?}", ExposedDropdownMenuAnchorType::PrimaryEditable), "PrimaryEditable");
    let renamed: MenuAnchorType = ExposedDropdownMenuAnchorType::PrimaryNotEditable;
    assert_eq!(renamed, ExposedDropdownMenuAnchorType::PrimaryNotEditable);
}

/// Back (Escape on the desktop) closes a dropdown menu (its focusable popup's dismissal) and an exposed menu: one
/// that takes the focus by its popup, an edited anchor's (not focusable) by the box's `BackHandler`.
fn back_closes_menus() {
    let open = mutable_state_of(true);
    let o = open.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let o = o.clone();
        MaterialTheme(content = move || ThreeItemMenu(o.clone(), Modifier));
    }));
    rule.on_node_with_text("Bir").assert_exists();
    key(&rule, Key::Escape);
    assert!(!open.peek());

    for edited in [false, true] {
        let open = mutable_state_of(true);
        let dismissed = new_log();
        let (o, d) = (open.clone(), dismissed.clone());
        let rule = KompozTestRule::new(kompozable_lambda!(move || {
            let (o, d) = (o.clone(), d.clone());
            MaterialTheme(
                content = move || {
                    let (o, o2, d) = (o.clone(), o.clone(), d.clone());
                    ExposedDropdownMenuBox(
                        o.get(),
                        move |x| o2.set(x),
                        content = move |scope: &ExposedDropdownMenuBoxScope| {
                            let t = if edited {
                                ExposedDropdownMenuAnchorType::PrimaryEditable
                            } else {
                                ExposedDropdownMenuAnchorType::PrimaryNotEditable
                            };
                            Box(modifier = scope.menu_anchor(Modifier.size(56.dp()), t).focusable());
                            let (o3, d) = (o.clone(), d.clone());
                            scope.ExposedDropdownMenu(
                                o.get(),
                                move || {
                                    push(&d, "dismiss");
                                    o3.set(false);
                                },
                                content = || {
                                    DropdownMenuItem(|| Text("Seçenek"), on_click = || {});
                                },
                            );
                        },
                    );
                },
            );
        }));
        rule.on_node_with_text("Seçenek").assert_exists();
        key(&rule, Key::Escape);
        assert!(!open.peek(), "edited: {edited}");
        // (the focusable popup's dismissal; the box's back handler for the edited anchor's)
        assert_eq!(taken(&dismissed), if edited { Vec::<String>::new() } else { vec!["dismiss".to_string()] });
    }
}

/// Compose's `DropdownMenu(…, scrollState, …)` (its MenuWithScrollStateSample): a menu taller than the window is
/// kept in it and scrolls by the scroll state given, which can take it to its end.
fn a_long_menu_scrolls_by_its_scroll_state() {
    let open = mutable_state_of(false);
    let to_end = mutable_state_of(false);
    let state: Rc<RefCell<Option<ScrollState>>> = Rc::new(RefCell::new(None));
    let (o, st, te) = (open.clone(), state.clone(), to_end.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, st, te) = (o.clone(), st.clone(), te.clone());
        MaterialTheme(
            content = move || {
                let (o, st) = (o.clone(), st.clone());
                let scroll_state = remember_scroll_state();
                *st.borrow_mut() = Some(scroll_state.clone());
                let (s, end) = (scroll_state.clone(), te.get());
                kompoz_coroutines::LaunchedEffect(end, async move {
                    if end {
                        // Scroll to show the bottom menu items.
                        s.scroll_to(s.max_value()).await;
                    }
                });
                Box(
                    modifier = Modifier.padding(20.dp()).size(40.dp()),
                    content = move || {
                        let o2 = o.clone();
                        DropdownMenu(
                            o.get(),
                            move || o2.set(false),
                            modifier = Modifier.test_tag("menü"),
                            scroll_state = scroll_state.clone(),
                            content = || {
                                for i in 0..40 {
                                    DropdownMenuItem(move || Text(format!("Öğe {i}")), on_click = || {});
                                }
                            },
                        );
                    },
                );
            },
        );
    }));
    open.set(true);
    rule.wait_for_idle();
    let menu = bounds(&rule, "menü");
    // (kept in the window: its column 800 − 16 tall)
    assert!(menu.top >= -0.5 && menu.bottom <= 800.5, "{menu:?}");
    let state = state.borrow().clone().unwrap();
    assert!(state.max_value() > 0 && state.value() == 0, "{}", state.max_value());
    let last = text_bounds(&rule, "Öğe 39");
    assert!(last.top > 800.0, "{last:?}");
    to_end.set(true); // (as the sample: an effect scrolling to the end)
    rule.wait_for_idle();
    assert_eq!(state.value(), state.max_value());
    let last = text_bounds(&rule, "Öğe 39");
    assert!(last.bottom <= 800.0 && last.top > 0.0, "{last:?}");
}

/// Compose's `MenuDefaults` and `ExposedDropdownMenuDefaults`: the elevations, icon sizes, paddings, the menu's,
/// groups' and items' shapes and colors in the theme, `groupShape`/`itemShape` by where a group or item is, the
/// shapes' overrides and `copy`; the group label and the trailing label in the label large style; the exposed
/// field's colors (the filled and outlined autocomplete fields').
fn menu_defaults_and_shapes_as_composes() {
    assert_eq!(MenuDefaults::TonalElevation, 0.dp());
    assert_eq!(MenuDefaults::ShadowElevation, 3.dp());
    assert_eq!(MenuDefaults::LeadingIconSize, 20.dp());
    assert_eq!(MenuDefaults::TrailingIconSize(), 20.dp());
    assert_eq!(MenuDefaults::GroupSpacing, 2.dp());
    assert_eq!(MenuDefaults::DropdownMenuItemContentPadding(), PaddingValues::hv(12.dp(), 0.dp()));
    assert_eq!(MenuDefaults::DropdownMenuSelectableItemContentPadding(), PaddingValues::all(12.dp()));
    assert_eq!(MenuDefaults::DropdownMenuGroupContentPadding(), PaddingValues::hv(0.dp(), 2.dp()));
    assert_eq!(MenuDefaults::HorizontalDividerPadding(), PaddingValues::hv(12.dp(), 2.dp()));
    assert_eq!(
        MenuDefaults::DropdownMenuGroupLabelHorizontalPadding(),
        PaddingValues::new(12.dp(), 0.dp(), 4.dp(), 0.dp())
    );
    assert_eq!(MenuDefaults::DropdownMenuItemTrailingLabelHorizontalPadding(), PaddingValues::all(0.dp()));
    assert_eq!(ExposedDropdownMenuDefaults::ItemContentPadding(), PaddingValues::hv(16.dp(), 0.dp()));

    let cs = scheme();
    let seen = Rc::new(RefCell::new(None));
    let sn = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let sn = sn.clone();
        MaterialTheme(
            content = move || {
                *sn.borrow_mut() = Some((
                    vec![
                        MenuDefaults::shape(),
                        MenuDefaults::leading_group_shape(),
                        MenuDefaults::middle_group_shape(),
                        MenuDefaults::trailing_group_shape(),
                        MenuDefaults::leading_item_shape(),
                        MenuDefaults::middle_item_shape(),
                        MenuDefaults::trailing_item_shape(),
                        MenuDefaults::standalone_item_shape(),
                        MenuDefaults::selected_item_shape(),
                        MenuDefaults::standalone_group_shape(),
                        MenuDefaults::inactive_group_shape(),
                    ],
                    vec![
                        MenuDefaults::container_color(),
                        MenuDefaults::group_standard_container_color(),
                        MenuDefaults::group_vibrant_container_color(),
                    ],
                    (
                        MenuDefaults.item_shapes(),
                        named_args!(MenuDefaults.item_shapes(selected_shape = Some(rounded(1.0, 1.0, 1.0, 1.0)))),
                        MenuDefaults.group_shapes(),
                        named_args!(MenuDefaults.group_shapes(inactive_shape = Some(rounded(3.0, 3.0, 3.0, 3.0)))),
                    ),
                    (0..3).map(|i| MenuDefaults::item_shape(i, 3)).collect::<Vec<_>>(),
                    MenuDefaults::item_shape(0, 1),
                    MenuDefaults::group_shape(0, 2),
                    (
                        ExposedDropdownMenuDefaults.text_field_colors(),
                        ExposedDropdownMenuDefaults.outlined_text_field_colors(),
                        named_args!(ExposedDropdownMenuDefaults.text_field_colors(focused_text_color = cs.error)),
                    ),
                ));
                Column(
                    content = || {
                        MenuDefaults.Label(content = || Text("Etiket"));
                        Row(content = || {
                            MenuDefaults.DropdownMenuItemTrailingLabel(content = || Text("⌘C"));
                        });
                        MenuDefaults.LabelWithSupportingText(|| Text("Destek"), || Text("Başlık"));
                    },
                );
            },
        );
    }));
    let (shapes, colors, overrides, items, standalone, two_groups, fields) = seen.borrow().clone().unwrap();
    let expected = [
        rounded(4.0, 4.0, 4.0, 4.0),
        rounded(16.0, 16.0, 8.0, 8.0),
        rounded(8.0, 8.0, 8.0, 8.0),
        rounded(8.0, 8.0, 16.0, 16.0),
        rounded(12.0, 12.0, 4.0, 4.0),
        rounded(4.0, 4.0, 4.0, 4.0),
        rounded(4.0, 4.0, 12.0, 12.0),
        rounded(4.0, 4.0, 4.0, 4.0),
        rounded(12.0, 12.0, 12.0, 12.0),
        rounded(16.0, 16.0, 16.0, 16.0),
        rounded(8.0, 8.0, 8.0, 8.0),
    ];
    for (i, (shape, expected)) in shapes.iter().zip(expected.iter()).enumerate() {
        assert!(**shape == **expected, "{i}: {shape:?}");
    }
    assert_eq!(colors, [cs.surface_container, cs.surface_container_low, cs.tertiary_container]);
    let (item_shapes, item_override, group_shapes, group_override) = overrides;
    assert_eq!(item_shapes, MenuItemShapes(rounded(4.0, 4.0, 4.0, 4.0), rounded(12.0, 12.0, 12.0, 12.0)));
    assert!(
        *item_override.selected_shape == *rounded(1.0, 1.0, 1.0, 1.0) && *item_override.shape == *item_shapes.shape
    );
    assert_eq!(group_shapes, MenuGroupShapes(rounded(16.0, 16.0, 16.0, 16.0), rounded(8.0, 8.0, 8.0, 8.0)));
    assert!(*group_override.inactive_shape == *rounded(3.0, 3.0, 3.0, 3.0));
    assert_eq!(group_override, named_args!(group_shapes.copy(inactive_shape = Some(rounded(3.0, 3.0, 3.0, 3.0)))));
    assert_eq!(item_shapes.copy(), item_shapes);
    assert_ne!(item_override, item_shapes);
    assert!(*items[0].shape == *rounded(12.0, 12.0, 4.0, 4.0) && *items[1].shape == *rounded(4.0, 4.0, 4.0, 4.0));
    assert!(*items[2].shape == *rounded(4.0, 4.0, 12.0, 12.0));
    assert!(items.iter().all(|i| *i.selected_shape == *rounded(12.0, 12.0, 12.0, 12.0)));
    assert_eq!(standalone, item_shapes);
    assert!(
        *two_groups.shape == *rounded(16.0, 16.0, 8.0, 8.0)
            && *two_groups.inactive_shape == *rounded(8.0, 8.0, 8.0, 8.0)
    );
    let (filled, outlined, red) = fields;
    assert!(
        filled.focused_container_color == cs.surface_container_highest && filled.focused_text_color == cs.on_surface
    );
    assert_eq!(filled.focused_indicator_color, cs.primary);
    assert_eq!(filled.disabled_label_color, cs.on_surface); // (Compose's: no alpha for the filled one's label)
    assert!(outlined.focused_container_color == Color::Transparent && outlined.focused_indicator_color == cs.primary);
    assert_eq!(outlined.disabled_label_color, named_args!(cs.on_surface.copy(alpha = 0.38)));
    assert!(red.focused_text_color == cs.error && red.unfocused_text_color == filled.unfocused_text_color);
    // The labels: label large; the group label at least 112 by 32, 12 dp in.
    let c = rule.draw();
    let t = Typography::default();
    let style_of = |text: &str| (0..c.texts.len()).find(|&i| c.texts[i] == text).map(|i| c.text_styles[i].font_size);
    assert_eq!(style_of("Etiket"), Some(t.label_large.font_size.value()));
    assert_eq!(style_of("⌘C"), Some(t.label_large.font_size.value()));
    assert_eq!(style_of("Başlık"), Some(t.label_large.font_size.value()));
    assert_eq!(style_of("Destek"), Some(t.body_medium.font_size.value()));
    let label = text_bounds(&rule, "Etiket");
    assert!(near(label.left, 12.0, 0.5) && near(label.center().y, 16.0, 0.5), "{label:?}");
    let trailing = text_bounds(&rule, "⌘C");
    assert!(near(trailing.top, 32.0, 0.5) && near(trailing.left, 0.0, 0.5), "{trailing:?}");
}

/// Compose's items' layouts: the standard item's row (the leading icon 12 dp in, the text 12 dp after it taking the
/// rest, the trailing icon 12 dp from the end; 48 dp tall, as wide as the menu); the Expressive item's
/// `DropdownMenuItemMeasurePolicy` (the leading icon, the text after the ghost icon's width, the trailing icon at
/// the end) and its checked icon coming in across as it is checked where it has no unchecked one.
fn items_lay_out_their_icons_and_text() {
    let checked = mutable_state_of(false);
    let ch = checked.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let ch = ch.clone();
        MaterialTheme(
            content = move || {
                let ch = ch.clone();
                Box(content = move || {
                    let ch = ch.clone();
                    DropdownMenu(
                        true,
                        || {},
                        content = move || {
                            DropdownMenuItem(
                                || Text("Kes"),
                                on_click = || {},
                                modifier = Modifier.test_tag("düz"),
                                leading_icon = slot(|| VectorIcon(Icons::Filled::Add(), None, Modifier.test_tag("ön"))),
                                trailing_icon = slot(|| Text("Ctrl+X", modifier = Modifier.test_tag("arka"))),
                            );
                            let ch2 = ch.clone();
                            CheckableDropdownMenuItem(
                                ch.get(),
                                move |v| ch2.set(v),
                                || Text("İşaret"),
                                MenuDefaults.item_shapes(),
                                modifier = Modifier.test_tag("işaret"),
                                checked_leading_icon =
                                    slot(|| VectorIcon(Icons::Filled::Check(), None, Modifier.test_tag("tik"))),
                            );
                            SelectableDropdownMenuItem(
                                false,
                                || {},
                                || Text("Her ikisi"),
                                MenuDefaults.item_shapes(),
                                leading_icon =
                                    slot(|| VectorIcon(Icons::Filled::Star(), None, Modifier.test_tag("yıldız"))),
                                trailing_icon = slot(|| {
                                    MenuDefaults.DropdownMenuItemTrailingLabel(content = || Text("⌘S"));
                                }),
                            );
                            ShapedDropdownMenuItem(
                                || {},
                                || Text("Sonda"),
                                MenuDefaults::trailing_item_shape(),
                                trailing_icon =
                                    slot(|| VectorIcon(Icons::Filled::Close(), None, Modifier.test_tag("x"))),
                            );
                        },
                    );
                });
            },
        );
    }));
    let row = bounds(&rule, "düz");
    assert!(near(row.height(), 48.0, 0.5), "{row:?}");
    let (lead, text, trail) = (unmerged(&rule, "ön"), text_bounds(&rule, "Kes"), unmerged(&rule, "arka"));
    assert!(near(lead.left, row.left + 12.0, 0.5) && near(text.left, lead.right + 12.0, 0.5), "{lead:?} {text:?}");
    assert!(near(trail.right, row.right - 12.0, 0.5), "{trail:?} {row:?}");
    // Expressive: the checked icon comes in across.
    // (the item's merged node: its selectable surface, inset 4 dp in the menu; its content 12 dp in)
    let item = bounds(&rule, "işaret");
    assert!(near(item.left, 4.0, 0.5), "{item:?}");
    let unchecked = text_bounds(&rule, "İşaret");
    assert!(near(unchecked.left, item.left + 12.0, 0.5), "{unchecked:?} {item:?}");
    // (no check drawn: only the ghost icon measuring the room it takes, not placed)
    let drawn_in =
        |rule: &KompozTestRule, at: Rect| rule.draw().vector_bounds.iter().filter(|b| at.contains(b.center())).count();
    assert_eq!(drawn_in(&rule, item), 0);
    checked.set(true);
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_time_by(32);
    let moving = text_bounds(&rule, "İşaret");
    assert!(moving.left > unchecked.left && moving.left < unchecked.left + 32.0, "{moving:?}");
    rule.wait_for_idle();
    assert_eq!(drawn_in(&rule, item), 1);
    let tick = *rule.draw().vector_bounds.iter().find(|b| item.contains(b.center())).unwrap();
    let checked_text = text_bounds(&rule, "İşaret");
    // (the icon 24 dp, 8 dp before the text)
    assert!(
        near(tick.left, unchecked.left, 0.5) && near(checked_text.left, tick.right + 8.0, 0.5),
        "{tick:?} {checked_text:?}"
    );
    // Both icons: the leading one, the text after it, the trailing label at the end (centered in the trailing box,
    // at least 20 dp wide).
    let star = unmerged(&rule, "yıldız");
    let both = text_bounds(&rule, "Her ikisi");
    let label = text_bounds(&rule, "⌘S");
    assert!(near(both.left, star.right + 8.0, 0.5), "{star:?} {both:?}");
    assert!(near(label.center().x, item.right - 12.0 - 10.0, 0.5), "{label:?} {item:?}");
    // The trailing icon only: the text at the start, the icon at the end.
    let last = text_bounds(&rule, "Sonda");
    let x = unmerged(&rule, "x");
    assert!(near(last.left, item.left + 12.0, 0.5) && near(x.right, item.right - 12.0, 0.5), "{last:?} {x:?}");
}

/// Compose's `DropdownMenuPositionProvider` as the two menus make it: an offset moves a menu from its anchor (its x
/// mirrored right to left); at the window's right edge a dropdown menu lines its end up with its anchor's (no side
/// margin on the desktop), the Expressive popup keeps 8 dp from the window's side.
fn menus_take_an_offset_and_keep_their_margins_at_the_windows_edges() {
    let rtl = mutable_state_of(false);
    let r = rtl.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let r = r.clone();
        MaterialTheme(
            content = move || {
                let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
                KompozitionLocalProvider(&[kompoz_ui::platform::LocalLayoutDirection.provides(direction)], || {
                    Column(
                        content = || {
                            Box(
                                modifier = Modifier.padding(100.dp()).size(40.dp()).test_tag("orta"),
                                content = || {
                                    DropdownMenu(
                                        true,
                                        || {},
                                        modifier = Modifier.test_tag("kaymış"),
                                        offset = DpOffset::new(10.dp(), 5.dp()),
                                        content = || {
                                            DropdownMenuItem(|| Text("Kaymış"), on_click = || {});
                                        },
                                    );
                                },
                            );
                            Box(
                                modifier = Modifier.fill_max_width(),
                                content_alignment = Alignment::TopEnd,
                                content = || {
                                    Box(
                                        modifier = Modifier.size(40.dp()).test_tag("kenar"),
                                        content = || {
                                            DropdownMenu(
                                                true,
                                                || {},
                                                modifier = Modifier.test_tag("menü"),
                                                content = || {
                                                    DropdownMenuItem(|| Text("Menü"), on_click = || {});
                                                },
                                            );
                                            DropdownMenuPopup(
                                                true,
                                                || {},
                                                modifier = Modifier.test_tag("açılır"),
                                                content = || {
                                                    Box(modifier = Modifier.size_wh(112.dp(), 40.dp()));
                                                },
                                            );
                                        },
                                    );
                                },
                            );
                        },
                    );
                });
            },
        );
    }));
    let (anchor, menu) = (bounds(&rule, "orta"), bounds(&rule, "kaymış"));
    assert!(
        near(menu.left, anchor.left + 10.0, 0.5) && near(menu.top, anchor.bottom + 5.0, 0.5),
        "{anchor:?} {menu:?}"
    );
    let edge = bounds(&rule, "kenar");
    assert!(near(edge.right, 400.0, 0.5));
    let (menu, popup) = (bounds(&rule, "menü"), bounds(&rule, "açılır"));
    assert!(near(menu.right, 400.0, 0.5) && near(menu.width(), 112.0, 0.5), "{menu:?}");
    assert!(near(popup.right, 392.0, 0.5), "{popup:?}");
    rtl.set(true);
    rule.wait_for_idle();
    let (anchor, menu) = (bounds(&rule, "orta"), bounds(&rule, "kaymış"));
    assert!(near(menu.right, anchor.right - 10.0, 0.5), "{anchor:?} {menu:?}");
}

/// The menus' parameters as Compose takes them: a dropdown menu's popup properties (one that isn't focusable lets
/// a tap outside reach what is beneath, dismissing it as well), its shape, container color, border and shadow; a
/// group's border, content padding and the interaction source its hovers go to; an item's content padding and
/// interaction source; an exposed menu's shape and container color; the Expressive popup's offset.
fn menus_take_their_properties_shapes_colors_and_paddings() {
    let cs = scheme();
    let red = Color::from_argb(0xFFFF0000);
    let open = mutable_state_of(true);
    let log = new_log();
    let group_source = MutableInteractionSource::new();
    let item_source = MutableInteractionSource::new();
    let (o, l, gs, is) = (open.clone(), log.clone(), group_source.clone(), item_source.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, l, gs, is) = (o.clone(), l.clone(), gs.clone(), is.clone());
        MaterialTheme(
            content = move || {
                let (o, l, gs, is) = (o.clone(), l.clone(), gs.clone(), is.clone());
                Column(
                    content = move || {
                        let (o, is) = (o.clone(), is.clone());
                        Box(
                            modifier = Modifier.padding(20.dp()).size(40.dp()),
                            content = move || {
                                let (o2, is) = (o.clone(), is.clone());
                                DropdownMenu(
                                    o.get(),
                                    move || o2.set(false),
                                    modifier = Modifier.test_tag("menü"),
                                    properties = PopupProperties { focusable: false, ..PopupProperties::default() },
                                    shape = rounded(20.0, 20.0, 20.0, 20.0),
                                    container_color = red,
                                    shadow_elevation = 6.dp(),
                                    border = Some(BorderStroke(2.dp(), cs.outline)),
                                    content = move || {
                                        DropdownMenuItem(
                                            || Text("Geniş"),
                                            on_click = || {},
                                            content_padding = PaddingValues::hv(30.dp(), 0.dp()),
                                            interaction_source = Some(is.clone()),
                                            modifier = Modifier.test_tag("öğe"),
                                        );
                                    },
                                );
                            },
                        );
                        let l = l.clone();
                        Button(
                            move || push(&l, "altta"),
                            modifier = Modifier.padding_sides(top = 400.dp()),
                            content = || Text("Altta"),
                        );
                        let gs = gs.clone();
                        Box(
                            modifier = Modifier.padding_sides(start = 200.dp()).size(40.dp()).test_tag("ikinci"),
                            content = move || {
                                let gs = gs.clone();
                                DropdownMenuPopup(
                                    true,
                                    || {},
                                    offset = DpOffset::new(0.dp(), 10.dp()),
                                    properties = PopupProperties { focusable: false, ..PopupProperties::default() },
                                    content = move || {
                                        DropdownMenuGroup(
                                            MenuDefaults.group_shapes(),
                                            modifier = Modifier.test_tag("grup"),
                                            border = Some(BorderStroke(1.dp(), red)),
                                            content_padding = PaddingValues::all(10.dp()),
                                            interaction_source = Some(gs.clone()),
                                            content = || {
                                                Box(modifier = Modifier.size_wh(120.dp(), 30.dp()).test_tag("içerik"));
                                            },
                                        );
                                    },
                                );
                            },
                        );
                    },
                );
            },
        );
    }));
    let c = rule.draw();
    let menu = bounds(&rule, "menü");
    let card = fill(&c, menu, red).expect("the menu's container");
    assert!(same_rect(c.rects[card], menu) && c.rect_corners[card] == corners(20.0, 20.0, 20.0, 20.0));
    assert!(c.strokes.iter().zip(&c.stroke_colors).any(|(r, &k)| k == cs.outline && same_rect(*r, menu)), "its border");
    assert!(!c.elevation_shadows.is_empty(), "its shadow");
    let (item, text) = (bounds(&rule, "öğe"), text_bounds(&rule, "Geniş"));
    assert!(near(text.left, item.left + 30.0, 0.5), "{item:?} {text:?}");
    // Pressed: told to the item's source.
    let pressed = Rc::new(Cell::new(false));
    let p = pressed.clone();
    let source = item_source.as_interaction_source();
    let probe = KompozTestRule::new(kompozable_lambda!(move || {
        p.set(source.collect_is_pressed_as_state().get());
    }));
    rule.host().pointer_down(item.center(), -1, 0, PointerType::Touch);
    rule.wait_for_idle();
    probe.wait_for_idle();
    assert!(pressed.get(), "the press told to its source");
    rule.host().pointer_up(item.center(), -1, 0);
    rule.wait_for_idle();
    // The group: its border, its padding, its offset popup; its hovers told to its source.
    let (anchor, group, inner) = (bounds(&rule, "ikinci"), bounds(&rule, "grup"), bounds(&rule, "içerik"));
    assert!(near(group.top, anchor.bottom + 10.0, 0.5), "{anchor:?} {group:?}");
    assert!(near(inner.left, group.left + 10.0, 0.5) && near(inner.top, group.top + 10.0, 0.5), "{inner:?}");
    assert!(c.strokes.iter().zip(&c.stroke_colors).any(|(r, &k)| k == red && same_rect(*r, group)), "its border");
    let hovered = Rc::new(Cell::new(false));
    let h = hovered.clone();
    let source = group_source.as_interaction_source();
    let probe = KompozTestRule::new(kompozable_lambda!(move || {
        h.set(source.collect_is_hovered_as_state().get());
    }));
    rule.host().pointer_move(group.center(), -1, 0);
    rule.wait_for_idle();
    probe.wait_for_idle();
    assert!(hovered.get(), "the hover told to its source");
    // Not focusable: a tap outside reaches the button beneath and dismisses the menu.
    tap(&rule, text_bounds(&rule, "Altta").center());
    assert_eq!(taken(&log), ["altta"]);
    assert!(!open.peek());
}
