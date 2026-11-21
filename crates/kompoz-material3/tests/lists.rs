//! Material's list items as Compose's material3 (1.5.0-alpha17) has them: the C++ Kompoz's tests by name
//! (kompoz_tests.cpp; they followed Compose 1.4, whose list tokens M15 renamed — `ListItem…` to `Item…` — keeping
//! every value the classic item reads), Compose's behaviour (the defaults from the tokens, the colors by state, the
//! interactive items' padding, vertical alignment, semantics, long clicks, and their shapes, colors and shadow as
//! they are hovered, focused, dragged), and the M15 samples of ListSamples.kt.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_animation::{ColumnAnimatedVisibility, expand_vertically, shrink_vertically};
use kompoz_foundation::interaction::{DragInteraction, FocusInteraction, HoverInteraction, MutableInteractionSource};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
// (the extended set's: the core icons and the rest, as Kotlin's one `Icons.Default` with material-icons-extended)
use kompoz_material_icons_extended::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas, has_role, has_text};
use kompoz_ui::input::PointerType;
use kompoz_ui::semantics::{Role, SemanticsProperties};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

tests!(
    list_items_as_compose_1_4,
    list_item_height_follows_its_lines,
    interactive_list_items_take_the_tap_and_morph,
    segmented_list_items_round_the_ends_of_their_card,
    list_item_defaults_follow_the_tokens,
    list_item_colors_pick_by_state_and_copy_what_is_given,
    list_item_slots_take_their_colors_and_styles,
    classic_list_items_measure_their_intrinsics,
    interactive_list_items_pad_and_align_their_parts,
    interactive_list_items_tell_accessibility_what_they_are,
    interactive_list_items_long_click_and_disable,
    interactive_list_items_follow_hover_focus_and_drag,
    list_item_samples,
    clickable_list_item_samples,
    segmented_list_item_with_expansion_sample,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// A node's bounds in the unmerged tree (a list item's parts: merged into the item).
fn part_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
}

/// A text's bounds in the unmerged tree.
fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    named_args!(rule.on_node_with_text(text, use_unmerged_tree = true)).get_unclipped_bounds_in_root()
}

/// A nullable composable slot (Kotlin's `(@Composable () -> Unit)?`).
fn slot(f: impl Fn() + 'static) -> Option<Rc<dyn Fn()>> {
    Some(Rc::new(f))
}

fn faded(color: Color, alpha: f32) -> Color {
    named_args!(color.copy(alpha = alpha))
}

/// The top left corner of the first fill of `color` (C++'s `cornerOf`), -1 where there is none.
fn corner_of(canvas: &RecordingCanvas, color: Color) -> f32 {
    canvas.rect_colors.iter().zip(&canvas.rect_corners).find(|(c, _)| **c == color).map_or(-1.0, |(_, r)| r.top_left)
}

/// The color `text` was drawn in (C++'s `textColor`).
fn text_color(canvas: &RecordingCanvas, text: &str) -> Color {
    canvas.texts.iter().zip(&canvas.colors).find(|(t, _)| *t == text).map_or(Color::Unspecified, |(_, c)| *c)
}

/// The style `text` was drawn in.
fn text_size(canvas: &RecordingCanvas, text: &str) -> f32 {
    canvas.texts.iter().zip(&canvas.text_styles).find(|(t, _)| *t == text).map_or(-1.0, |(_, s)| s.font_size)
}

/// The highest elevation shadow drawn around `r`'s center.
fn shadow_at(canvas: &RecordingCanvas, r: Rect) -> f32 {
    canvas
        .elevation_shadows
        .iter()
        .filter(|e| near(e.bounds.center().x, r.center().x, 2.0) && near(e.bounds.center().y, r.center().y, 2.0))
        .map(|e| e.elevation())
        .fold(0.0, f32::max)
}

/// `shape`'s radii in a 300 by 56 rect (C++'s `radiiFor`): top left, top right, bottom right, bottom left.
fn radii(shape: &Arc<dyn Shape>) -> [f32; 4] {
    match shape.create_outline(Size::new(300.0, 56.0), LayoutDirection::Ltr, &Density::new(1.0, 1.0)) {
        Outline::Rounded(r) => [
            r.top_left_corner_radius.x,
            r.top_right_corner_radius.x,
            r.bottom_right_corner_radius.x,
            r.bottom_left_corner_radius.x,
        ],
        Outline::Rectangle(_) => [0.0; 4],
        Outline::Generic(_) => panic!("a corner shape"),
    }
}

// ---- The C++ Kompoz's tests --------------------------------------------------------------------------------------

fn list_items_as_compose_1_4() {
    // (M15's ListTokens: the classic item's heights 56/72/88, its fonts and colors as Compose 1.4's under new names;
    // its paddings — 16 dp at the sides, 8 dp above and below, 12 dp in a three line item — are ListItem.kt's own)
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            // Supporting text over two lines: three lines' worth (12 dp, its sides at the top).
            ListItem(
                || Text("Başlık"),
                modifier = Modifier.test_tag("uzun"),
                supporting_content = slot(|| Text("bir\niki")),
                leading_content = slot(|| Box(modifier = Modifier.size(40.dp()).test_tag("baş"))),
            );
            // Two lines: its sides in the middle.
            ListItem(
                || Text("Başlık"),
                modifier = Modifier.test_tag("orta"),
                supporting_content = slot(|| Text("tek")),
                trailing_content = slot(|| Box(modifier = Modifier.size(24.dp()).test_tag("son"))),
            );
        });
    }));
    let (tall, lead) = (bounds(&rule, "uzun"), part_bounds(&rule, "baş"));
    assert!(near(tall.height(), 88.0, 0.01) && near(lead.top, tall.top + 12.0, 0.01), "{tall:?} {lead:?}");
    let (mid, trail) = (bounds(&rule, "orta"), part_bounds(&rule, "son"));
    assert!(near(mid.height(), 72.0, 0.01) && near(trail.center().y, mid.center().y, 0.01));
    assert!(near(trail.right, mid.right - 16.0, 0.01));
    // One node for accessibility: its texts together.
    let items =
        rule.on_all_nodes(has_text("Başlık", false, false).and(has_text("tek", false, false))).fetch_semantics_nodes();
    assert_eq!(items.len(), 1);
}

fn list_item_height_follows_its_lines() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            ListItem(|| Text("Tek"), modifier = Modifier.test_tag("one"));
            ListItem(
                || Text("İki"),
                modifier = Modifier.test_tag("two"),
                supporting_content = slot(|| Text("alt")),
                leading_content = slot(|| VectorIcon(Icons::Filled::Person(), Some("Kişi".to_string()))),
            );
            ListItem(
                || Text("Üç"),
                modifier = Modifier.test_tag("three"),
                overline_content = slot(|| Text("üst")),
                supporting_content = slot(|| Text("alt")),
                trailing_content = slot(|| Text("100+")),
            );
        });
    }));
    assert!(near(bounds(&rule, "one").height(), 56.0, 0.01));
    assert!(near(bounds(&rule, "two").height(), 72.0, 0.01));
    assert!(near(bounds(&rule, "three").height(), 88.0, 0.01));
    let person = named_args!(rule.on_node_with_content_description("Kişi", use_unmerged_tree = true))
        .get_unclipped_bounds_in_root();
    let headline = text_bounds(&rule, "İki");
    assert!(near(person.left, 16.0, 0.01) && near(headline.left, 16.0 + 24.0 + 16.0, 0.01));
    // 16 dp at the end, as Compose 1.4's (and 1.5's)
    assert!(near(text_bounds(&rule, "100+").right, 400.0 - 16.0, 0.01));
}

fn interactive_list_items_take_the_tap_and_morph() {
    let selected = mutable_state_of(false);
    let checked = Rc::new(Cell::new(false));
    let clicks = Rc::new(Cell::new(0));
    let (s, c, k) = (selected.clone(), checked.clone(), clicks.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, c, k) = (s.clone(), c.clone(), k.clone());
        MaterialTheme(
            color_scheme = light_color_scheme(),
            content = move || {
                let (s, c, k) = (s.clone(), c.clone(), k.clone());
                Column(move || {
                    let (k1, k2, s1, c1) = (k.clone(), k.clone(), s.clone(), c.clone());
                    ClickableListItem(
                        move || k1.set(k1.get() + 1),
                        modifier = Modifier.test_tag("tık"),
                        supporting_content = slot(|| Text("açıklama")),
                        content = || Text("Tıkla"),
                    );
                    SelectableListItem(
                        s.get(),
                        move || s1.set(!s1.peek()),
                        modifier = Modifier.test_tag("seç"),
                        content = || Text("Seç"),
                    );
                    ToggleableListItem(
                        c.get(),
                        move |v| c1.set(v),
                        modifier = Modifier.test_tag("işaret"),
                        content = || Text("İşaretle"),
                    );
                    ClickableListItem(
                        move || k2.set(k2.get() + 1),
                        modifier = Modifier.test_tag("kapalı"),
                        enabled = false,
                        content = || Text("Kapalı"),
                    );
                });
            },
        );
    }));
    let cs = light_color_scheme();
    // (a little rounded at rest: M15's extra small corners, 4 dp, as the C++'s)
    assert!(bounds(&rule, "tık").height() >= 56.0 && near(corner_of(&rule.draw(), cs.surface), 4.0, 0.01));
    rule.on_node_with_tag("tık").perform_click();
    rule.on_node_with_tag("kapalı").perform_click(); // disabled: nothing
    assert_eq!(clicks.get(), 1);
    let idle = rule.draw();
    assert_eq!(text_color(&idle, "Tıkla"), cs.on_surface);
    assert_eq!(text_color(&idle, "açıklama"), cs.on_surface_variant);
    assert_eq!(text_color(&idle, "Kapalı"), faded(cs.on_surface, 0.38));
    rule.on_node_with_tag("seç").perform_click(); // selected: secondaryContainer, rounder
    rule.advance_time_by(1000);
    rule.wait_for_idle();
    let chosen = rule.draw();
    assert!(selected.peek());
    assert!(
        near(corner_of(&chosen, cs.secondary_container), 16.0, 0.5),
        "{}",
        corner_of(&chosen, cs.secondary_container)
    );
    assert_eq!(text_color(&chosen, "Seç"), cs.on_secondary_container);
    rule.on_node_with_tag("işaret").perform_click();
    rule.wait_for_idle();
    assert!(checked.get());
    // Held: the pressed shape (16), back to 4 once let go.
    let at = bounds(&rule, "tık").center();
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.advance_time_by(1000);
    assert!(near(corner_of(&rule.draw(), cs.surface), 16.0, 0.5));
    rule.host().pointer_up(at, -1, 0);
    rule.advance_time_by(1000);
    rule.wait_for_idle();
    assert!(near(corner_of(&rule.draw(), cs.surface), 4.0, 0.5));
}

fn segmented_list_items_round_the_ends_of_their_card() {
    // (the shapes are a composable's: Compose's segmentedShapes remembers them and reads the theme)
    let shapes: Rc<RefCell<Vec<ListItemShapes>>> = Rc::new(RefCell::new(Vec::new()));
    let opened = Rc::new(Cell::new(-1));
    let (sh, o) = (shapes.clone(), opened.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (sh, o) = (sh.clone(), o.clone());
        *sh.borrow_mut() = vec![
            ListItemDefaults.segmented_shapes(0, 3),
            ListItemDefaults.segmented_shapes(1, 3),
            ListItemDefaults.segmented_shapes(2, 3),
            ListItemDefaults.segmented_shapes(0, 1),
        ];
        Column(
            vertical_arrangement = Arrangement::spaced_by(ListItemDefaults::SegmentedGap),
            content = move || {
                for i in 0..3 {
                    let o = o.clone();
                    SegmentedListItem(
                        move || o.set(i),
                        ListItemDefaults.segmented_shapes(i, 3),
                        modifier = Modifier.test_tag(format!("öğe{i}")),
                        content = move || Text(format!("Öğe {i}")),
                    );
                }
            },
        );
    }));
    {
        let shapes = shapes.borrow();
        let (f, m, l) = (radii(&shapes[0].shape), radii(&shapes[1].shape), radii(&shapes[2].shape));
        assert!(near(f[0], 16.0, 0.01) && near(f[1], 16.0, 0.01) && near(f[3], 4.0, 0.01), "{f:?}");
        assert!(near(m[0], 4.0, 0.01) && near(m[2], 4.0, 0.01), "{m:?}");
        assert!(near(l[0], 4.0, 0.01) && near(l[3], 16.0, 0.01) && near(l[2], 16.0, 0.01), "{l:?}");
        assert!(near(radii(&shapes[3].shape)[3], 4.0, 0.01)); // alone: as it is
        // (only the rest shape changes: the first item pressed takes the pressed shape as it is)
        assert!(*shapes[0].pressed_shape == *shapes[1].pressed_shape);
    }
    assert!(near(bounds(&rule, "öğe1").top, bounds(&rule, "öğe0").bottom + 2.0, 0.01));
    rule.on_node_with_tag("öğe2").perform_click();
    assert_eq!(opened.get(), 2);
    // (each item on the surface — the first and last as paths, their corners not all alike — and cut to its shape)
    let canvas = rule.draw();
    let surface = MaterialTheme_light_surface();
    let fills = canvas.rect_colors.iter().chain(&canvas.path_colors).filter(|c| **c == surface).count();
    assert_eq!(fills, 3);
    let corners: Vec<_> =
        canvas.clip_shapes.iter().filter(|c| near(c.bounds.height(), 56.0, 0.5)).map(|c| c.radii).collect();
    assert_eq!(corners.len(), 3, "{:?}", canvas.clip_shapes);
    assert!(near(corners[0].top_left, 16.0, 0.01) && near(corners[1].top_left, 4.0, 0.01));
    assert!(near(corners[2].bottom_left, 16.0, 0.01));
}

/// The baseline scheme's surface (what a segmented item is on outside a MaterialTheme too).
fn MaterialTheme_light_surface() -> Color {
    light_color_scheme().surface
}

// ---- Compose's behaviour -----------------------------------------------------------------------------------------

fn list_item_defaults_follow_the_tokens() {
    type Got = (ListItemColors, ListItemColors, ListItemShapes, Arc<dyn Shape>, Color, Color, VerticalAlignment);
    let got: Rc<RefCell<Option<Got>>> = Rc::new(RefCell::new(None));
    let g = got.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let g = g.clone();
        MaterialTheme(
            content = move || {
                *g.borrow_mut() = Some((
                    ListItemDefaults.colors(),
                    ListItemDefaults.segmented_colors(),
                    ListItemDefaults.shapes(),
                    ListItemDefaults::shape(),
                    ListItemDefaults::container_color(),
                    ListItemDefaults::content_color(),
                    ListItemDefaults.vertical_alignment(),
                ));
            },
        );
    }));
    let (colors, segmented, shapes, shape, container, content, alignment) = got.borrow_mut().take().unwrap();
    let cs = light_color_scheme();
    assert_eq!((container, content), (cs.surface, cs.on_surface));
    assert!((*shape).shape_as_any().is::<RectangleShape>());
    // At rest: on the surface; the slots on surface variant.
    assert_eq!(colors.container_color, cs.surface);
    assert_eq!(colors.content_color, cs.on_surface);
    for c in [
        colors.leading_content_color,
        colors.trailing_content_color,
        colors.overline_content_color,
        colors.supporting_content_color,
    ] {
        assert_eq!(c, cs.on_surface_variant);
    }
    // Selected: the secondary container, all on it.
    assert_eq!(colors.selected_container_color, cs.secondary_container);
    for c in [
        colors.selected_content_color,
        colors.selected_leading_content_color,
        colors.selected_trailing_content_color,
        colors.selected_overline_content_color,
        colors.selected_supporting_content_color,
    ] {
        assert_eq!(c, cs.on_secondary_container);
    }
    // Disabled: on surface at 38%, the container as it is.
    assert_eq!(colors.disabled_container_color, cs.surface);
    for c in [
        colors.disabled_content_color,
        colors.disabled_leading_content_color,
        colors.disabled_trailing_content_color,
        colors.disabled_overline_content_color,
        colors.disabled_supporting_content_color,
    ] {
        assert_eq!(c, faded(cs.on_surface, 0.38));
    }
    // Dragged (the reorder list's): the tertiary container, all on it.
    assert_eq!(colors.dragged_container_color, cs.tertiary_container);
    for c in [
        colors.dragged_content_color,
        colors.dragged_leading_content_color,
        colors.dragged_trailing_content_color,
        colors.dragged_overline_content_color,
        colors.dragged_supporting_content_color,
    ] {
        assert_eq!(c, cs.on_tertiary_container);
    }
    // The segmented ones: the same (their container token is the surface too in M15).
    assert_eq!(segmented, colors);
    // Shapes: extra small at rest, large selected, pressed, focused, dragged, medium hovered.
    let corner = |s: &Arc<dyn Shape>| radii(s)[0];
    assert_eq!(
        [
            corner(&shapes.shape),
            corner(&shapes.selected_shape),
            corner(&shapes.pressed_shape),
            corner(&shapes.focused_shape),
            corner(&shapes.hovered_shape),
            corner(&shapes.dragged_shape),
        ],
        [4.0, 16.0, 16.0, 16.0, 12.0, 16.0]
    );
    // Elevation, padding, gap.
    assert_eq!(ListItemDefaults::Elevation, Dp(0.0));
    assert_eq!(ListItemDefaults::SegmentedGap, Dp(2.0));
    let padding = ListItemDefaults::ContentPadding();
    assert_eq!(
        (
            padding.calculate_start_padding(LayoutDirection::Ltr),
            padding.calculate_top_padding(),
            padding.calculate_end_padding(LayoutDirection::Ltr),
            padding.calculate_bottom_padding()
        ),
        (Dp(16.0), Dp(10.0), Dp(16.0), Dp(10.0))
    );
    let elevation = named_args!(ListItemDefaults.elevation());
    assert_eq!((elevation.elevation, elevation.dragged_elevation), (Dp(0.0), Dp(8.0)));
    assert_eq!(named_args!(ListItemDefaults.elevation(elevation = Dp(1.0))), ListItemElevation::new(Dp(1.0), Dp(8.0)));
    // The vertical alignment: centered under (88 + 72) / 2 - 20 = 60 dp, at the top from there.
    assert_eq!(alignment.align(20, 59), 20);
    assert_eq!(alignment.align(20, 60), 0);
}

fn list_item_colors_pick_by_state_and_copy_what_is_given() {
    let c = |v: u32| Color::from_argb(0xFF00_0000 | v);
    let colors = ListItemColors::new(
        c(1),
        c(2),
        c(3),
        c(4),
        c(5),
        c(6),
        c(11),
        c(12),
        c(13),
        c(14),
        c(15),
        c(16),
        c(21),
        c(22),
        c(23),
        c(24),
        c(25),
        c(26),
        c(31),
        c(32),
        c(33),
        c(34),
        c(35),
        c(36),
    );
    // Disabled first, then dragged, then selected.
    assert_eq!(colors.container_color_for(true, false, false), c(1));
    assert_eq!(colors.container_color_for(false, true, true), c(11));
    assert_eq!(colors.content_color_for(true, true, true), c(32));
    assert_eq!(colors.leading_content_color_for(true, true, false), c(23));
    assert_eq!(colors.trailing_content_color_for(true, false, true), c(34));
    assert_eq!(colors.overline_content_color_for(false, false, false), c(15));
    assert_eq!(colors.supporting_content_color_for(true, true, false), c(26));
    // Copy: unspecified keeps the own.
    let copied = named_args!(colors.copy(content_color = c(99), selected_container_color = Color::Unspecified));
    assert_eq!((copied.content_color, copied.selected_container_color, copied.container_color), (c(99), c(21), c(1)));
    // The older names.
    let older = ListItemColors::new_with_headline_color(c(1), c(2), c(3), c(5), c(6), c(4), c(12), c(13), c(14));
    assert_eq!(
        (older.content_color, older.leading_content_color, older.trailing_content_color, older.overline_content_color),
        (c(2), c(3), c(4), c(5))
    );
    assert_eq!(older.disabled_content_color, c(12));
    assert_eq!(older.selected_container_color, Color::Unspecified);
    assert_eq!(older.disabled_overline_content_color, Color::Unspecified);
    #[allow(deprecated)]
    {
        assert_eq!(
            (colors.headline_color(), colors.leading_icon_color(), colors.trailing_icon_color()),
            (c(2), c(3), c(4))
        );
        assert_eq!((colors.overline_color(), colors.supporting_text_color()), (c(5), c(6)));
        assert_eq!(
            (
                colors.disabled_headline_color(),
                colors.disabled_leading_icon_color(),
                colors.disabled_trailing_icon_color()
            ),
            (c(12), c(13), c(14))
        );
        // (the deprecated copy makes colors of the older names: its selected and dragged ones unspecified)
        let copied = named_args!(colors.copy_with_headline_color(headline_color = c(77)));
        assert_eq!((copied.content_color, copied.container_color), (c(77), c(1)));
        assert_eq!(copied.dragged_container_color, Color::Unspecified);
    }
    // ListItemDefaults' colors over the theme's.
    let got: Rc<RefCell<Vec<ListItemColors>>> = Rc::new(RefCell::new(Vec::new()));
    let g = got.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let g = g.clone();
        MaterialTheme(
            content = move || {
                *g.borrow_mut() = vec![
                    named_args!(ListItemDefaults.colors(container_color = Color::Transparent)),
                    named_args!(
                        ListItemDefaults
                            .colors_with_headline_color(headline_color = Color::Red, leading_icon_color = Color::Blue)
                    ),
                    named_args!(ListItemDefaults.segmented_colors(selected_container_color = Color::Green)),
                ];
            },
        );
    }));
    let got = got.borrow();
    let cs = light_color_scheme();
    assert_eq!((got[0].container_color, got[0].content_color), (Color::Transparent, cs.on_surface));
    assert_eq!((got[1].content_color, got[1].leading_content_color), (Color::Red, Color::Blue));
    assert_eq!(got[1].selected_container_color, cs.secondary_container);
    assert_eq!((got[2].selected_container_color, got[2].container_color), (Color::Green, cs.surface));
}

fn list_item_slots_take_their_colors_and_styles() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(|| {
                    ListItem(
                        || Text("başlık"),
                        overline_content = slot(|| Text("üst")),
                        supporting_content = slot(|| Text("alt")),
                        leading_content = slot(|| Text("ön")),
                        trailing_content = slot(|| Text("son")),
                        tonal_elevation = Dp(3.0),
                        shadow_elevation = Dp(2.0),
                        modifier = Modifier.test_tag("klasik"),
                    );
                    ClickableListItem(
                        || {},
                        overline_content = slot(|| Text("e-üst")),
                        supporting_content = slot(|| Text("e-alt")),
                        leading_content = slot(|| Text("e-ön")),
                        trailing_content = slot(|| Text("e-son")),
                        content = || Text("e-başlık"),
                    );
                });
            },
        );
    }));
    let cs = light_color_scheme();
    let canvas = rule.draw();
    // The classic item: headline body large (16 sp) on surface, supporting body medium (14), overline and trailing
    // label small (11), the slots on surface variant.
    assert_eq!((text_color(&canvas, "başlık"), text_size(&canvas, "başlık")), (cs.on_surface, 16.0));
    assert_eq!((text_color(&canvas, "alt"), text_size(&canvas, "alt")), (cs.on_surface_variant, 14.0));
    assert_eq!((text_color(&canvas, "üst"), text_size(&canvas, "üst")), (cs.on_surface_variant, 11.0));
    assert_eq!((text_color(&canvas, "son"), text_size(&canvas, "son")), (cs.on_surface_variant, 11.0));
    // (its leading content takes the color only: the text style around, body large in the theme)
    assert_eq!((text_color(&canvas, "ön"), text_size(&canvas, "ön")), (cs.on_surface_variant, 16.0));
    // (tonal elevation tints the surface; the shadow is drawn)
    let item = bounds(&rule, "klasik");
    assert!(canvas.rect_colors.contains(&cs.surface_color_at_elevation(Dp(3.0))));
    assert!(shadow_at(&canvas, item) > 0.0);
    // The interactive one: its leading content in title medium (16 sp), the rest as the classic's.
    assert_eq!((text_color(&canvas, "e-başlık"), text_size(&canvas, "e-başlık")), (cs.on_surface, 16.0));
    assert_eq!(text_size(&canvas, "e-ön"), 16.0);
    assert_eq!(text_size(&canvas, "e-alt"), 14.0);
    assert_eq!((text_size(&canvas, "e-üst"), text_size(&canvas, "e-son")), (11.0, 11.0));
    assert_eq!(text_color(&canvas, "e-son"), cs.on_surface_variant);
}

fn classic_list_items_measure_their_intrinsics() {
    // An item asked its intrinsic height (in a Row of IntrinsicSize.Min): its lines' height as it would measure,
    // the supporting text's lines guessed from its height (more than 30 sp: three lines).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Row(
                modifier = Modifier.height_intrinsic(IntrinsicSize::Min),
                content = || {
                    Box(
                        modifier = Modifier.weight(1.0),
                        content = || {
                            ListItem(|| Text("Başlık"), supporting_content = slot(|| Text("tek")));
                        },
                    );
                    Box(modifier = Modifier.width(10.dp()).fill_max_height().test_tag("yan1"));
                },
            );
            Row(
                modifier = Modifier.height_intrinsic(IntrinsicSize::Min),
                content = || {
                    Box(
                        modifier = Modifier.weight(1.0),
                        content = || {
                            ListItem(|| Text("Başlık"), supporting_content = slot(|| Text("bir\niki\nüç")));
                        },
                    );
                    Box(modifier = Modifier.width(10.dp()).fill_max_height().test_tag("yan2"));
                },
            );
            Box(
                modifier = Modifier.width_intrinsic(IntrinsicSize::Max).test_tag("geniş"),
                content = || {
                    ListItem(
                        headline_content = || {
                            Box(modifier = Modifier.width(100.dp()).height(10.dp()));
                        },
                    );
                },
            );
        });
    }));
    assert!(near(bounds(&rule, "yan1").height(), 72.0, 0.01));
    let three = bounds(&rule, "yan2").height();
    assert!(three >= 88.0, "{three}");
    // (its intrinsic width: the headline's and the padding's)
    assert!(near(bounds(&rule, "geniş").width(), 100.0 + 32.0, 0.01));
}

fn interactive_list_items_pad_and_align_their_parts() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(|| {
                    ClickableListItem(
                        || {},
                        modifier = Modifier.test_tag("kısa"),
                        leading_content = slot(|| Box(modifier = Modifier.size(24.dp()).test_tag("k-ön"))),
                        trailing_content = slot(|| Box(modifier = Modifier.size(24.dp()).test_tag("k-son"))),
                        content = || Text("kısa"),
                    );
                    ClickableListItem(
                        || {},
                        modifier = Modifier.test_tag("uzun"),
                        leading_content = slot(|| Box(modifier = Modifier.size(24.dp()).test_tag("u-ön"))),
                        content = || Box(modifier = Modifier.size_wh(50.dp(), 80.dp()).test_tag("u-içerik")),
                    );
                });
            },
        );
    }));
    // A short item: at least 56 dp, its parts centered, 16 dp in at its sides, 12 dp between the slots and the
    // headline.
    let short = bounds(&rule, "kısa");
    let (lead, trail, headline) = (part_bounds(&rule, "k-ön"), part_bounds(&rule, "k-son"), text_bounds(&rule, "kısa"));
    assert!(near(short.height(), 56.0, 0.01), "{short:?}");
    assert!(near(lead.left, short.left + 16.0, 0.01) && near(lead.center().y, short.center().y, 0.01));
    assert!(near(trail.right, short.right - 16.0, 0.01) && near(trail.center().y, short.center().y, 0.01));
    assert!(near(headline.left, lead.right + 12.0, 0.01));
    // A tall one (its room past the 60 dp breakpoint): its parts at the top, 10 dp down.
    let tall = bounds(&rule, "uzun");
    let (lead, content) = (part_bounds(&rule, "u-ön"), part_bounds(&rule, "u-içerik"));
    assert!(near(tall.height(), 100.0, 0.01), "{tall:?}");
    assert!(near(lead.top, tall.top + 10.0, 0.01) && near(content.top, tall.top + 10.0, 0.01));
}

fn interactive_list_items_tell_accessibility_what_they_are() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(|| {
                    ClickableListItem(|| {}, content = || Text("tık"));
                    SelectableListItem(true, || {}, content = || Text("seç"));
                    ToggleableListItem(false, |_| {}, content = || Text("işaret"));
                    SelectableSegmentedListItem(
                        false,
                        || {},
                        ListItemDefaults.segmented_shapes(0, 2),
                        content = || Text("s-seç"),
                    );
                    ToggleableSegmentedListItem(
                        true,
                        |_| {},
                        ListItemDefaults.segmented_shapes(1, 2),
                        content = || Text("s-işaret"),
                    );
                });
            },
        );
    }));
    rule.on_node_with_text("tık").assert_has_click_action();
    let role = |text: &str| rule.on_node_with_text(text).fetch_semantics_node().config().get(SemanticsProperties::Role);
    assert_eq!(role("tık"), None);
    rule.on_node_with_text("seç").assert_is_selected();
    assert_eq!(role("seç"), Some(Role::RadioButton));
    rule.on_node_with_text("işaret").assert_is_off();
    assert_eq!(role("işaret"), Some(Role::Checkbox));
    rule.on_node_with_text("s-seç").assert_is_not_selected();
    rule.on_node_with_text("s-işaret").assert_is_on();
    rule.on_all_nodes(has_role(Role::Checkbox)).assert_count_equals(2);
}

fn interactive_list_items_long_click_and_disable() {
    let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
    let enabled = mutable_state_of(true);
    let (l, e) = (log.clone(), enabled.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l1, l2, e) = (l.clone(), l.clone(), e.clone());
        MaterialTheme(
            content = move || {
                let (l1, l2) = (l1.clone(), l2.clone());
                ClickableListItem(
                    move || l1.borrow_mut().push("tık"),
                    modifier = Modifier.test_tag("öğe"),
                    enabled = e.get(),
                    on_long_click = slot(move || l2.borrow_mut().push("uzun")),
                    on_long_click_label = Some("seçim kipi".to_string()),
                    content = || Text("öğe"),
                );
            },
        );
    }));
    rule.on_node_with_tag("öğe").perform_touch_input(|t| named_args!(t.long_click()));
    rule.on_node_with_tag("öğe").perform_click();
    assert_eq!(*log.borrow(), ["uzun", "tık"]);
    enabled.set(false);
    rule.wait_for_idle();
    rule.on_node_with_tag("öğe").perform_click().assert_is_not_enabled();
    assert_eq!(log.borrow().len(), 2);
    // (disabled: its headline in on surface at 38%, its container the surface)
    let canvas = rule.draw();
    let cs = light_color_scheme();
    assert_eq!(text_color(&canvas, "öğe"), faded(cs.on_surface, 0.38));
}

fn interactive_list_items_follow_hover_focus_and_drag() {
    let source = MutableInteractionSource::new();
    let s = source.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        MaterialTheme(
            content = move || {
                ClickableListItem(
                    || {},
                    modifier = Modifier.test_tag("öğe"),
                    interaction_source = Some(s.clone()),
                    content = || Text("öğe"),
                );
            },
        );
    }));
    let cs = light_color_scheme();
    let item = bounds(&rule, "öğe");
    assert!(near(corner_of(&rule.draw(), cs.surface), 4.0, 0.01));
    // Hovered: medium corners (12 dp).
    let enter = HoverInteraction::Enter::new();
    source.try_emit(enter);
    rule.advance_time_by(1000);
    assert!(near(corner_of(&rule.draw(), cs.surface), 12.0, 0.5));
    source.try_emit(HoverInteraction::Exit::new(enter));
    rule.advance_time_by(1000);
    assert!(near(corner_of(&rule.draw(), cs.surface), 4.0, 0.5));
    // Focused: large corners.
    let focus = FocusInteraction::Focus::new();
    source.try_emit(focus);
    rule.advance_time_by(1000);
    assert!(near(corner_of(&rule.draw(), cs.surface), 16.0, 0.5));
    source.try_emit(FocusInteraction::Unfocus::new(focus));
    rule.advance_time_by(1000);
    // Dragged: the tertiary container, its headline on it, large corners, 8 dp of shadow (drawn over the others).
    let start = DragInteraction::Start::new();
    source.try_emit(start);
    rule.advance_time_by(1000);
    rule.wait_for_idle();
    let dragged = rule.draw();
    assert!(near(corner_of(&dragged, cs.tertiary_container), 16.0, 0.5), "{:?}", dragged.rect_colors);
    assert_eq!(text_color(&dragged, "öğe"), cs.on_tertiary_container);
    assert!(near(shadow_at(&dragged, item), 8.0, 0.1), "{}", shadow_at(&dragged, item));
    source.try_emit(DragInteraction::Stop::new(start));
    rule.advance_time_by(1000);
    rule.wait_for_idle();
    let after = rule.draw();
    assert!(near(corner_of(&after, cs.surface), 4.0, 0.5));
    assert_eq!(shadow_at(&after, item), 0.0);
}

// ---- The M15 samples ---------------------------------------------------------------------------------------------

#[kompozable]
fn OneLineListItem() {
    Column(|| {
        HorizontalDivider();
        ListItem(
            || Text("One line list item with 24x24 icon"),
            leading_content = slot(|| VectorIcon(Icons::Filled::Favorite(), Some("Localized description".into()))),
        );
        HorizontalDivider();
    });
}

#[kompozable]
fn TwoLineListItem() {
    Column(|| {
        HorizontalDivider();
        ListItem(
            || Text("Two line list item with trailing"),
            supporting_content = slot(|| Text("Secondary text")),
            trailing_content = slot(|| Text("meta")),
            leading_content = slot(|| VectorIcon(Icons::Filled::Favorite(), Some("Localized description".into()))),
        );
        HorizontalDivider();
    });
}

#[kompozable]
fn ThreeLineListItemWithOverlineAndSupporting() {
    Column(|| {
        HorizontalDivider();
        ListItem(
            || Text("Three line list item"),
            overline_content = slot(|| Text("OVERLINE")),
            supporting_content = slot(|| Text("Secondary text")),
            leading_content = slot(|| VectorIcon(Icons::Filled::Favorite(), Some("Localized description".into()))),
            trailing_content = slot(|| Text("meta")),
        );
        HorizontalDivider();
    });
}

#[kompozable]
fn ThreeLineListItemWithExtendedSupporting() {
    Column(|| {
        HorizontalDivider();
        ListItem(
            || Text("Three line list item"),
            supporting_content = slot(|| Text("Secondary text that\nspans multiple lines")),
            leading_content = slot(|| VectorIcon(Icons::Filled::Favorite(), Some("Localized description".into()))),
            trailing_content = slot(|| Text("meta")),
        );
        HorizontalDivider();
    });
}

#[kompozable]
fn ClickableListItemSample() {
    Column(|| {
        HorizontalDivider();

        for idx in 0..3 {
            let count = remember_saveable(|| mutable_int_state_of(0));
            let (c1, c2) = (count.clone(), count.clone());
            ClickableListItem(
                move || c1.set(c1.peek() + 1),
                leading_content = slot(|| VectorIcon(Icons::Default::Home(), None)),
                trailing_content = slot(move || Text(format!("{}", c2.get()))),
                supporting_content = slot(|| Text("Additional info")),
                content = move || Text(format!("Item {}", idx + 1)),
            );

            HorizontalDivider();
        }
    });
}

#[kompozable]
fn ClickableListItemWithClickableChildSample(child_clicks: Rc<Cell<i32>>) {
    Column(move || {
        HorizontalDivider();

        for idx in 0..3 {
            let child_clicks = child_clicks.clone();
            ClickableListItem(
                || { /* ListItem onClick callback */ },
                leading_content = slot(|| VectorIcon(Icons::Default::Home(), None)),
                trailing_content = slot(move || {
                    let child_clicks = child_clicks.clone();
                    IconButton(
                        move || child_clicks.set(child_clicks.get() + 1),
                        content = || VectorIcon(Icons::Default::Favorite(), Some("Localized description".into())),
                    );
                }),
                supporting_content = slot(|| Text("The trailing icon has a separate click action")),
                content = move || Text(format!("Item {}", idx + 1)),
            );

            HorizontalDivider();
        }
    });
}

fn list_item_samples() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            MaterialTheme(
                content = || {
                    Column(|| {
                        OneLineListItem();
                        TwoLineListItem();
                        ThreeLineListItemWithOverlineAndSupporting();
                        ThreeLineListItemWithExtendedSupporting();
                    });
                },
            );
        }),
        // (wide: the test text backend's glyphs are wider than a real font's, and the two line item's headline
        // would wrap, making it taller as Compose's would)
        IntSize::new(800, 900),
    );
    // Each item one node, its height by its lines: 56, 72, 88 and 88 (the supporting text over two lines).
    let heights: Vec<f32> = rule
        .on_all_nodes(has_text("list item", true, false))
        .fetch_semantics_nodes()
        .iter()
        .map(|n| n.unclipped_bounds_in_root().height())
        .collect();
    assert_eq!(heights, [56.0, 72.0, 88.0, 88.0]);
    // (a three line item's trailing meta text at the top, 12 dp down; a two line item's centered)
    let items = rule.on_all_nodes(has_text("list item", true, false)).fetch_semantics_nodes();
    let metas = named_args!(rule.on_all_nodes_with_text("meta", use_unmerged_tree = true)).fetch_semantics_nodes();
    assert_eq!(metas.len(), 3);
    let (two, two_meta) = (items[1].unclipped_bounds_in_root(), metas[0].unclipped_bounds_in_root());
    assert!(near(two_meta.center().y, two.center().y, 0.01));
    let (three, three_meta) = (items[2].unclipped_bounds_in_root(), metas[1].unclipped_bounds_in_root());
    assert!(near(three_meta.top, three.top + 12.0, 0.01));
    // (the dividers around: one dp lines of the outline variant, stroked paths)
    let cs = light_color_scheme();
    let canvas = rule.draw();
    assert_eq!(canvas.path_colors.iter().filter(|c| **c == cs.outline_variant).count(), 8);
}

fn clickable_list_item_samples() {
    let child_clicks = Rc::new(Cell::new(0));
    let c = child_clicks.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let c = c.clone();
            MaterialTheme(
                content = move || {
                    let c = c.clone();
                    Column(move || {
                        ClickableListItemSample();
                        ClickableListItemWithClickableChildSample(c.clone());
                    });
                },
            );
        }),
        IntSize::new(400, 900),
    );
    // Each item counts its own clicks.
    rule.on_all_nodes_with_text("Item 2").on_first().perform_click();
    rule.on_all_nodes_with_text("Item 2").on_first().perform_click();
    rule.on_all_nodes_with_text("Item 3").on_first().perform_click();
    // (their counts 0, 2 and 1)
    for count in ["0", "1", "2"] {
        named_args!(rule.on_all_nodes_with_text(count, use_unmerged_tree = true)).assert_count_equals(1);
    }
    // The trailing icon button takes its own clicks, apart from the item's.
    named_args!(rule.on_all_nodes_with_content_description("Localized description", use_unmerged_tree = true))
        .on_first()
        .perform_click();
    assert_eq!(child_clicks.get(), 1);
    // (six items that take clicks, and three icon buttons in them: nodes of their own, a clickable merging what is
    // below it but not into what is above, as Compose's)
    rule.on_all_nodes(kompoz_test::has_click_action().and(has_text("Item", true, false))).assert_count_equals(6);
    rule.on_all_nodes(kompoz_test::has_click_action()).assert_count_equals(9);
}

#[kompozable]
fn SegmentedListItemWithExpansionSample() {
    let expanded = remember_saveable(|| mutable_state_of(false));
    let num_children = 3;
    let item_count = 1 + if expanded.get() { num_children } else { 0 };
    let children_checked = remember_saveable(|| mutable_state_list_of(vec![false; num_children as usize]));

    let colors =
        named_args!(ListItemDefaults.colors(container_color = MaterialTheme::color_scheme().surface_container));

    Column(
        modifier = Modifier.fill_max_size(),
        vertical_arrangement = Arrangement::spaced_by(ListItemDefaults::SegmentedGap),
        content = || {
            Spacer(Modifier.height(100.dp()));
            let (e, e2, e3) = (expanded.clone(), expanded.clone(), expanded.clone());
            SegmentedListItem(
                move || e.set(!e.get()),
                ListItemDefaults.segmented_shapes(0, item_count),
                modifier = Modifier
                    .semantics(move |c| c.set_state_description(if e2.get() { "Expanded" } else { "Collapsed" })),
                colors = colors,
                leading_content = slot(|| VectorIcon(Icons::Default::Favorite(), None)),
                trailing_content = slot(move || {
                    VectorIcon(
                        if e3.get() { Icons::Default::ExpandLess() } else { Icons::Default::ExpandMore() },
                        None,
                    );
                }),
                content = || Text("Click to expand/collapse"),
            );
            let children_checked = children_checked.clone();
            ColumnAnimatedVisibility(
                expanded.get(),
                enter = named_args!(expand_vertically(MaterialTheme::motion_scheme().fast_spatial_spec())),
                exit = named_args!(shrink_vertically(MaterialTheme::motion_scheme().fast_spatial_spec())),
                content = move |_| {
                    let children_checked = children_checked.clone();
                    Column(
                        vertical_arrangement = Arrangement::spaced_by(ListItemDefaults::SegmentedGap),
                        content = move || {
                            for idx in 0..num_children {
                                let checks = children_checked.clone();
                                // (Compose's sample's trailingContent, `Checkbox(checked = childrenChecked[idx],
                                // onCheckedChange = null)`, comes with WP2's Checkbox: paste-ready in the WP4b notes'
                                // icons-pending.rs)
                                ToggleableSegmentedListItem(
                                    children_checked.get(idx as usize).unwrap(),
                                    move |it| checks.set(idx as usize, it),
                                    ListItemDefaults.segmented_shapes(idx + 1, item_count),
                                    colors = colors,
                                    leading_content = slot(|| VectorIcon(Icons::Default::Favorite(), None)),
                                    content = move || Text(format!("Child {}", idx + 1)),
                                );
                            }
                        },
                    );
                },
            );
        },
    );
}

fn segmented_list_item_with_expansion_sample() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| MaterialTheme(content = || SegmentedListItemWithExpansionSample())),
        IntSize::new(400, 900),
    );
    let header = || rule.on_node_with_text("Click to expand/collapse");
    let state = || header().fetch_semantics_node().config().get(SemanticsProperties::StateDescription);
    let drawn = |icon: ImageVector| rule.draw().vectors.contains(&icon.id());
    // Collapsed: the header alone, under the 100 dp spacer and the 2 dp gap, ExpandMore after it, on the surface
    // container.
    assert_eq!(state().as_deref(), Some("Collapsed"));
    rule.on_node_with_text("Child 1").assert_does_not_exist();
    let top = header().get_unclipped_bounds_in_root().top;
    assert!(near(top, 102.0, 0.5), "{top}");
    assert!(drawn(Icons::Default::ExpandMore()) && !drawn(Icons::Default::ExpandLess()));
    let canvas = rule.draw();
    let container = light_color_scheme().surface_container;
    assert!(canvas.rect_colors.contains(&container) || canvas.path_colors.contains(&container));
    // Expanded: three children 2 dp apart under it, sliding in; ExpandLess.
    header().perform_click();
    rule.advance_time_by(1000);
    assert_eq!(state().as_deref(), Some("Expanded"));
    assert!(drawn(Icons::Default::ExpandLess()) && !drawn(Icons::Default::ExpandMore()));
    let items: Vec<Rect> = ["Click to expand/collapse", "Child 1", "Child 2", "Child 3"]
        .iter()
        .map(|t| rule.on_node_with_text(t).get_unclipped_bounds_in_root())
        .collect();
    for pair in items.windows(2) {
        assert!(near(pair[1].top, pair[0].bottom + 2.0, 0.5), "{items:?}");
    }
    // (each child checks on its own click; the saved list keeps it)
    rule.on_node_with_text("Child 2").assert_is_off();
    rule.on_node_with_text("Child 2").perform_click();
    rule.on_node_with_text("Child 2").assert_is_on();
    rule.on_node_with_text("Child 1").assert_is_off();
    // Collapsed again: the children gone once they have shrunk.
    header().perform_click();
    rule.advance_time_by(1000);
    rule.on_node_with_text("Child 2").assert_does_not_exist();
    assert_eq!(state().as_deref(), Some("Collapsed"));
    // (expanded again: the second child still checked)
    header().perform_click();
    rule.advance_time_by(1000);
    rule.on_node_with_text("Child 2").assert_is_on();
}
