//! Material's segmented buttons as Compose's material3 (1.5.0-alpha17) has them: the C++ Kompoz's
//! segmented_buttons_as_compose_1_4, segmented_buttons_keep_48_dp_of_room_and_draw_40,
//! multi_choice_segmented_buttons_toggle_each_and_tell_their_roles and the segments of
//! chips_radio_buttons_and_segments_select (with M15's values where the C++ had its own; its
//! segmented_buttons_take_compose_named_arguments is in both syntaxes in crates/kompoz/tests/named_forms.rs),
//! Compose's behaviour (the colors and their copy, the segments' shapes, the padding, the icon's crossfade, a pressed
//! segment drawn over its neighbours) and the 2 samples of M15's SegmentedButtonSamples.kt.
//!
//! M15's `SegmentedButton` takes its `shape` (no default): the C++ tests' segments without one are given
//! `SegmentedButtonDefaults.item_shape(index, count)` here.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::interaction::{MutableInteractionSource, PressInteraction};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_extended::Icons;
use kompoz_material3::Text;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::input::PointerType;
use kompoz_ui::semantics::{Role, SemanticsProperties, ToggleableState};
use kompoz_ui::*;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

tests!(
    segmented_buttons_as_compose_1_4,
    segmented_buttons_keep_48_dp_of_room_and_draw_40,
    multi_choice_segmented_buttons_toggle_each_and_tell_their_roles,
    chips_radio_buttons_and_segments_select,
    segmented_button_colors_follow_the_scheme_and_copy_what_is_given,
    segmented_button_defaults_follow_the_tokens,
    segmented_button_item_shapes_round_the_ends_of_a_row,
    a_segmented_buttons_icon_crossfades_with_an_inactive_one,
    a_pressed_or_checked_segment_is_drawn_over_its_neighbours,
    segmented_button_samples,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    named_args!(rule.on_node_with_text(text, use_unmerged_tree = true)).get_unclipped_bounds_in_root()
}

fn same_rect(a: Rect, b: Rect) -> bool {
    near(a.left, b.left, 0.01)
        && near(a.top, b.top, 0.01)
        && near(a.width(), b.width(), 0.01)
        && near(a.height(), b.height(), 0.01)
}

fn faded(c: Color, alpha: f32) -> Color {
    named_args!(c.copy(alpha = alpha))
}

fn tap(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.host().pointer_up(at, -1, 0);
}

/// The paths filling their bounds' middle (their bounds and colors): an end segment's container (its shape rounds
/// two corners only, so it is drawn as a path).
fn fills(c: &RecordingCanvas) -> Vec<(Rect, Color)> {
    (0..c.paths.len()).filter(|&i| filled_middle(&c.path_shapes[i])).map(|i| (c.paths[i], c.path_colors[i])).collect()
}

/// The paths around their bounds' middle (their shapes and colors): an end segment's border (Compose's border of
/// such a shape fills the outline less the inside).
fn outlines(c: &RecordingCanvas) -> Vec<(Rect, Path, Color)> {
    (0..c.paths.len())
        .filter(|&i| !filled_middle(&c.path_shapes[i]))
        .map(|i| (c.paths[i], c.path_shapes[i].clone(), c.path_colors[i]))
        .collect()
}

/// Whether `path` (in its own coordinates) fills its middle.
fn filled_middle(path: &Path) -> bool {
    path.contains(path.get_bounds().center())
}

/// The shape of the segment at `index` of `count` (M15's `SegmentedButton` takes one; the C++'s had a default).
fn item_shape(index: i32, count: i32) -> Arc<dyn Shape> {
    named_args!(SegmentedButtonDefaults.item_shape(index, count))
}

// ---- C++ segmented_buttons_as_compose_1_4 ----------------------------------------------------------------------

fn segmented_buttons_as_compose_1_4() {
    // M15 as the C++'s 1.4: the label sits over half of the icon's room without one, slides aside as the check
    // mark grows in. Compose's animations move one frame later than the C++'s (the change's frame composes, the next
    // starts the animation): three frames where the C++ advanced two.
    let picked = mutable_state_of(0);
    let scheme = Rc::new(Cell::new(light_color_scheme()));
    let (p, sc) = (picked.clone(), scheme.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        sc.set(MaterialTheme::color_scheme());
        let p = p.clone();
        SingleChoiceSegmentedButtonRow(
            modifier = Modifier.width(300.dp()),
            content = move |row| {
                for i in 0..2 {
                    let p = p.clone();
                    row.SegmentedButton(
                        p.get() == i,
                        move || p.set(i),
                        item_shape(i, 2),
                        label = move || Text(if i == 0 { "Bir" } else { "Iki" }),
                    );
                }
            },
        );
    }));
    let cs = scheme.get();
    // Without an icon the label sits over half of the icon's room: centered in the segment.
    let (two, one) = (text_bounds(&rule, "Iki"), text_bounds(&rule, "Bir"));
    assert!(near(two.center().x, 225.0, 1.0), "{two:?}");
    // Selected: the container at once, the label sliding right, the check growing in.
    picked.set(1);
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    let c = rule.draw();
    // (a segment's shape rounds two corners only: its container is drawn as a path)
    assert!(fills(&c).iter().any(|(r, color)| *color == cs.secondary_container && r.left > 140.0));
    let sliding = text_bounds(&rule, "Iki").center().x;
    assert!(sliding > 225.5 && sliding < 237.5, "{sliding}");
    // (the scale layer about its bottom start, scaleIn's TransformOrigin(0, 1))
    assert!(c.transforms.iter().any(|m| m.values[0] > 0.01 && m.values[0] < 0.99), "the check grows in");
    rule.wait_for_idle();
    assert!(near(text_bounds(&rule, "Iki").center().x, 238.0, 1.0));
    // The other loses its check at once.
    assert!(near(text_bounds(&rule, "Bir").center().x, one.center().x, 13.5));
}

// ---- C++ segmented_buttons_keep_48_dp_of_room_and_draw_40 -------------------------------------------------------

fn segmented_buttons_keep_48_dp_of_room_and_draw_40() {
    // M15: each segment is a Surface keeping 48 dp of room to touch it (the row 48 tall), drawn 40 tall in the middle.
    // Compose's semantics bounds of a segment are its selectable's (the 40 dp container; the C++ said 48). Each
    // segment draws its own outline (the C++ drew one around the row and 1 dp separators): the first rounded at its
    // start, the last at its end, overlapping their neighbours by the outline's width (spacedBy(-1 dp)).
    let picked = mutable_state_of(0);
    let room = mutable_state_of(48.0f32);
    let cs = light_color_scheme();
    let (p, r) = (picked.clone(), room.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let p = p.clone();
        KompozitionLocalProvider(&[LocalMinimumInteractiveComponentSize.provides(Dp(r.get()))], move || {
            let p = p.clone();
            SingleChoiceSegmentedButtonRow(
                modifier = Modifier.width(300.dp()).test_tag("satır"),
                content = move |row| {
                    for i in 0..3 {
                        let p = p.clone();
                        row.SegmentedButton(
                            p.get() == i,
                            move || p.set(i),
                            item_shape(i, 3),
                            modifier = Modifier.test_tag(format!("seg{i}")),
                            label = move || Text(format!("S{i}")),
                        );
                    }
                },
            );
        });
    }));
    let (row, seg) = (bounds(&rule, "satır"), bounds(&rule, "seg1"));
    assert!(near(row.height(), 48.0, 0.01) && near(seg.height(), 40.0, 0.01), "{row:?} {seg:?}");
    assert!(near(seg.top, row.top + 4.0, 0.01));
    // (300 dp and two overlaps of 1 shared by three: 100.67 each, in whole pixels)
    assert!(near(seg.width(), 101.0, 1.0), "{seg:?}");
    let canvas = rule.draw();
    let first = bounds(&rule, "seg0");
    // The first segment's outline: round at its start (a 20 dp corner: the top left corner's point outside it), square
    // at its end; 1 dp of the outline color.
    let outline = outlines(&canvas).into_iter().any(|(at, path, color)| {
        let b = path.get_bounds();
        same_rect(at, first)
            && !path.contains(Offset::new(b.left + 1.0, b.top + 1.0))
            && path.contains(Offset::new(b.right - 0.5, b.top + 0.5))
            && path.contains(Offset::new(b.center().x, b.top + 0.5))
            && !path.contains(Offset::new(b.center().x, b.top + 1.5))
            && color == cs.outline
    });
    let fill = fills(&canvas).iter().any(|(r, color)| *color == cs.secondary_container && same_rect(*r, first));
    assert!(outline && fill, "{:?} {:?} {:?}", canvas.paths, canvas.path_strokes, canvas.path_colors);
    // A tap 2 dp from the top, above the band, picks the segment.
    tap(&rule, Offset::new(seg.center().x, row.top + 2.0));
    rule.wait_for_idle();
    assert_eq!(picked.peek(), 1);
    room.set(0.0);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "satır").height(), 40.0, 0.01));
}

// ---- C++ multi_choice_segmented_buttons_toggle_each_and_tell_their_roles ---------------------------------------

fn multi_choice_segmented_buttons_toggle_each_and_tell_their_roles() {
    // M15: a toggleable segment is the Surface's toggleable, which sets no role (the C++ told a checkbox) — its
    // state is its toggleable state; a selectable one a radio button, in a selectable group. M15's check mark has no
    // description.
    let walk = mutable_state_of(true);
    let bike = mutable_state_of(false);
    let period = mutable_state_of(0);
    let car_taps = Rc::new(Cell::new(0));
    let (w, b, pe, ct) = (walk.clone(), bike.clone(), period.clone(), car_taps.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (w, b, pe, ct) = (w.clone(), b.clone(), pe.clone(), ct.clone());
        Column(move || {
            let (w, b, ct) = (w.clone(), b.clone(), ct.clone());
            MultiChoiceSegmentedButtonRow(
                modifier = Modifier.width(300.dp()),
                content = move |row| {
                    let (w, b, ct) = (w.clone(), b.clone(), ct.clone());
                    row.SegmentedButton(w.get(), move |on| w.set(on), item_shape(0, 3), label = || Text("Yürüyüş"));
                    row.SegmentedButton(b.get(), move |on| b.set(on), item_shape(1, 3), label = || Text("Bisiklet"));
                    row.SegmentedButton(
                        false,
                        move |_| ct.set(ct.get() + 1),
                        item_shape(2, 3),
                        enabled = false,
                        label = || Text("Araba"),
                    );
                },
            );
            let pe = pe.clone();
            SingleChoiceSegmentedButtonRow(
                modifier = Modifier.width(300.dp()).test_tag("tekli"),
                content = move |row| {
                    let names = ["Gün", "Hafta", "Ay"];
                    for i in 0..3 {
                        let pe = pe.clone();
                        let icon: Option<Rc<dyn Fn()>> = if i == 2 {
                            Some(Rc::new(|| VectorIcon(Icons::Filled::Star(), Some("Yıldız".to_string()))))
                        } else {
                            None
                        };
                        row.SegmentedButton(
                            pe.get() == i,
                            move || pe.set(i),
                            item_shape(i, 3),
                            icon = icon,
                            label = move || Text(names[i as usize]),
                        );
                    }
                },
            );
        });
    }));
    rule.on_node_with_text("Bisiklet").perform_click(); // each on its own: walking stays on
    assert!(bike.peek() && walk.peek());
    rule.on_node_with_text("Yürüyüş").perform_click();
    assert!(!walk.peek() && bike.peek());
    rule.on_node_with_text("Araba").perform_click();
    assert_eq!(car_taps.get(), 0);
    rule.on_node_with_text("Ay").perform_click();
    assert_eq!(period.peek(), 2);
    rule.wait_for_idle();

    let node = |word: &str| rule.on_node_with_text(word).fetch_semantics_node();
    let (bike_node, walk_node, car_node, month) = (node("Bisiklet"), node("Yürüyüş"), node("Araba"), node("Ay"));
    assert_eq!(bike_node.config().get(SemanticsProperties::ToggleableState), Some(ToggleableState::On));
    assert_eq!(bike_node.config().get(SemanticsProperties::Role), None);
    assert_eq!(walk_node.config().get(SemanticsProperties::ToggleableState), Some(ToggleableState::Off));
    assert!(car_node.config().contains(SemanticsProperties::Disabled));
    assert_eq!(month.config().get(SemanticsProperties::Role), Some(Role::RadioButton));
    assert_eq!(month.config().get(SemanticsProperties::Selected), Some(true));
    assert!(
        rule.on_node_with_tag_unmerged("tekli")
            .fetch_semantics_node()
            .config()
            .contains(SemanticsProperties::SelectableGroup)
    );
    // A selected one shows a check mark, unless it has an icon of its own; a disabled one is dim.
    let canvas = rule.draw();
    let star = Icons::Filled::Star().id();
    assert_eq!(canvas.vectors.iter().filter(|v| **v == star).count(), 1);
    assert_eq!(canvas.vectors.iter().filter(|v| **v != star).count(), 1, "the check of \"Bisiklet\" only");
    let araba = canvas.texts.iter().position(|t| t == "Araba").expect("drawn");
    assert_eq!(canvas.colors[araba], faded(light_color_scheme().on_surface, 0.38));
}

// (C++ segmented_buttons_take_compose_named_arguments: in both syntaxes in crates/kompoz/tests/named_forms.rs)

// ---- C++ chips_radio_buttons_and_segments_select: its segments --------------------------------------------------

fn chips_radio_buttons_and_segments_select() {
    // The C++ test's SingleChoiceSegmentedButtonRow (its radio buttons are WP2's: tests/controls.rs, same name; its
    // FilterChip the chips', kompoz-material3/tests/containers.rs's FilterChip tests). M15's check mark has no
    // description (the C++'s said "Selected"): it is told by being drawn. Compose's segment bounds are its
    // container's, as wide as a third of 300 dp with the outlines overlapping.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let segment = remember(|| mutable_state_of(0));
        Column(move || {
            let s = segment.clone();
            SingleChoiceSegmentedButtonRow(
                modifier = Modifier.width(300.dp()),
                content = move |row| {
                    for i in 0..3 {
                        let s = s.clone();
                        row.SegmentedButton(
                            s.get() == i,
                            move || s.set(i),
                            item_shape(i, 3),
                            modifier = Modifier.test_tag(format!("seg{i}")),
                            label = move || Text(format!("S{i}")),
                        );
                    }
                },
            );
            Text(format!("seçim: {}", segment.get()));
        });
    }));
    rule.on_node_with_text("seçim: 0").assert_exists();
    rule.on_node_with_text("S2").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("seçim: 2").assert_exists();
    assert_eq!(rule.draw().vectors.len(), 1, "the selected segment's check");
    // Segments share the row equally.
    assert!(near(bounds(&rule, "seg0").width(), 100.0, 1.0) && near(bounds(&rule, "seg2").left, 200.0, 1.0));
}

// ---- Compose's behaviour ----------------------------------------------------------------------------------------

fn segmented_button_colors_follow_the_scheme_and_copy_what_is_given() {
    let made = Rc::new(Cell::new(None));
    let m = made.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        m.set(Some((
            SegmentedButtonDefaults.colors(),
            SegmentedButtonDefaults
                .colors(active_container_color = Color::Red, disabled_inactive_border_color = Color::Blue),
        )));
    }));
    let (colors, given) = made.get().expect("composed");
    let cs = light_color_scheme();
    assert_eq!(colors.active_container_color, cs.secondary_container);
    assert_eq!(colors.active_content_color, cs.on_secondary_container);
    assert_eq!(colors.active_border_color, cs.outline);
    assert_eq!(colors.inactive_container_color, Color::Transparent);
    assert_eq!(colors.inactive_content_color, cs.on_surface);
    assert_eq!(colors.inactive_border_color, cs.outline);
    assert_eq!(colors.disabled_active_container_color, cs.secondary_container);
    assert_eq!(colors.disabled_active_content_color, faded(cs.on_surface, 0.38));
    assert_eq!(colors.disabled_active_border_color, faded(cs.outline, 0.12));
    assert_eq!(colors.disabled_inactive_container_color, Color::Transparent);
    assert_eq!(colors.disabled_inactive_content_color, faded(cs.on_surface, 0.38));
    assert_eq!(colors.disabled_inactive_border_color, faded(cs.outline, 0.12));
    assert_eq!(
        given,
        named_args!(colors.copy(active_container_color = Color::Red, disabled_inactive_border_color = Color::Blue))
    );
    assert_ne!(given, colors);
    // Unspecified keeps the color.
    assert_eq!(named_args!(colors.copy(active_content_color = Color::Unspecified)), colors);
    let all = SegmentedButtonColors::new(
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Black,
        Color::White,
        Color::Gray,
        Color::Cyan,
        Color::Magenta,
        Color::Yellow,
        Color::DarkGray,
        Color::LightGray,
        Color::Transparent,
    );
    assert_eq!(all.disabled_inactive_content_color, Color::LightGray);
}

fn segmented_button_defaults_follow_the_tokens() {
    assert_eq!(SegmentedButtonDefaults::BorderWidth, Dp(1.0));
    assert_eq!(SegmentedButtonDefaults::IconSize, Dp(18.0));
    let padding = SegmentedButtonDefaults::ContentPadding();
    let button = ButtonDefaults::ContentPadding();
    assert_eq!(padding.calculate_left_padding(LayoutDirection::Ltr), Dp(12.0));
    assert_eq!(padding.calculate_right_padding(LayoutDirection::Ltr), Dp(12.0));
    assert_eq!(padding.calculate_top_padding(), button.calculate_top_padding());
    assert_eq!(padding.calculate_bottom_padding(), button.calculate_bottom_padding());
    assert_eq!(named_args!(SegmentedButtonDefaults.border_stroke(Color::Red)), BorderStroke(Dp(1.0), Color::Red));
    assert_eq!(
        named_args!(SegmentedButtonDefaults.border_stroke(Color::Red, width = Dp(3.0))),
        BorderStroke(Dp(3.0), Color::Red)
    );
}

fn segmented_button_item_shapes_round_the_ends_of_a_row() {
    let made = Rc::new(std::cell::RefCell::new(Vec::<Arc<dyn Shape>>::new()));
    let m = made.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let base = SegmentedButtonDefaults::base_shape();
        assert!(base == CornerBasedShape::Rounded(CircleShape));
        let cut = CornerBasedShape::Cut(CutCornerShape(CornerSize::dp(Dp(4.0))));
        *m.borrow_mut() = vec![
            SegmentedButtonDefaults.item_shape(0, 1),
            SegmentedButtonDefaults.item_shape(0, 2, base_shape = cut.clone()),
            SegmentedButtonDefaults.item_shape(1, 2, base_shape = cut),
        ];
    }));
    let shapes = made.borrow();
    // Alone: the base shape itself.
    assert!(*shapes[0] == *(&CornerBasedShape::Rounded(CircleShape) as &dyn Shape));
    // Of another base shape: its kind kept, its start or end corners.
    let cut = |s: &Arc<dyn Shape>| (**s).shape_as_any().downcast_ref::<CornerBasedShape>().cloned();
    assert!(matches!(cut(&shapes[1]), Some(CornerBasedShape::Cut(_))));
    let (first, last) = (cut(&shapes[1]).unwrap(), cut(&shapes[2]).unwrap());
    assert_eq!(*first.top_start(), CornerSize::dp(Dp(4.0)));
    assert_eq!(*first.top_end(), CornerSize::dp(Dp(0.0)));
    assert_eq!(*last.bottom_end(), CornerSize::dp(Dp(4.0)));
    assert_eq!(*last.bottom_start(), CornerSize::dp(Dp(0.0)));
}

fn a_segmented_buttons_icon_crossfades_with_an_inactive_one() {
    // SegmentedButtonDefaults.Icon with an inactive content crossfades the two (the default effects spec); without,
    // the active one comes in faded and scaled from its bottom start and goes at once.
    let active = mutable_state_of(false);
    let with_inactive = mutable_state_of(true);
    let (a, w) = (active.clone(), with_inactive.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (a, w) = (a.clone(), w.clone());
        Column(move || {
            let inactive: Option<Rc<dyn Fn()>> =
                if w.get() { Some(Rc::new(|| Box(modifier = Modifier.size(18.dp()).test_tag("pasif")))) } else { None };
            SegmentedButtonDefaults.Icon(
                a.get(),
                active_content = Rc::new(|| Box(modifier = Modifier.size(18.dp()).test_tag("etkin"))),
                inactive_content = inactive,
            );
        });
    }));
    rule.on_node_with_tag("pasif").assert_exists();
    rule.on_node_with_tag("etkin").assert_does_not_exist();
    active.set(true);
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    // Both while it crossfades.
    rule.on_node_with_tag("pasif").assert_exists();
    rule.on_node_with_tag("etkin").assert_exists();
    rule.wait_for_idle();
    rule.on_node_with_tag("pasif").assert_does_not_exist();
    // Without an inactive content: gone at once when it stops being active.
    with_inactive.set(false);
    rule.wait_for_idle();
    rule.on_node_with_tag("etkin").assert_exists();
    active.set(false);
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    rule.on_node_with_tag("etkin").assert_does_not_exist();
}

fn a_pressed_or_checked_segment_is_drawn_over_its_neighbours() {
    // Compose's interactionZIndex: a segment is placed above the others by its presses and focuses (and by 5 more
    // while checked), so its outline is drawn whole over the neighbours' it overlaps.
    let sources: Vec<MutableInteractionSource> = (0..3).map(|_| MutableInteractionSource::new()).collect();
    let checked = mutable_state_of(false);
    let (s, c) = (sources.clone(), checked.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, c) = (s.clone(), c.clone());
        MultiChoiceSegmentedButtonRow(
            modifier = Modifier.width(300.dp()),
            content = move |row| {
                for i in 0..3 {
                    let c = c.clone();
                    let colors = SegmentedButtonDefaults.colors(
                        active_border_color = Color::Red,
                        inactive_border_color = [Color::Blue, Color::Green, Color::Yellow][i as usize],
                    );
                    row.SegmentedButton(
                        i == 2 && c.get(),
                        move |on| c.set(on),
                        SegmentedButtonDefaults.item_shape(0, 1),
                        colors = colors,
                        interaction_source = Some(s[i as usize].clone()),
                        label = move || Text(format!("K{i}")),
                    );
                }
            },
        );
    }));
    // (round segments: their outlines drawn as rounded strokes)
    let stroke_order = |rule: &KompozTestRule| -> Vec<Color> { rule.draw().stroke_colors };
    // In their order at first.
    assert_eq!(stroke_order(&rule), vec![Color::Blue, Color::Green, Color::Yellow]);
    // The first one pressed: drawn last.
    let press = PressInteraction::Press::new(Offset::Zero);
    let source = sources[0].clone();
    let p = press;
    rule.run_on_idle(move || source.try_emit(p));
    rule.wait_for_idle();
    assert_eq!(stroke_order(&rule), vec![Color::Green, Color::Yellow, Color::Blue]);
    let source = sources[0].clone();
    rule.run_on_idle(move || source.try_emit(PressInteraction::Release::new(press)));
    rule.wait_for_idle();
    assert_eq!(stroke_order(&rule), vec![Color::Blue, Color::Green, Color::Yellow]);
    // The last one checked (its outline the active one) stays over the middle one even when it is pressed.
    checked.set(true);
    rule.wait_for_idle();
    let source = sources[1].clone();
    rule.run_on_idle(move || source.try_emit(PressInteraction::Press::new(Offset::Zero)));
    rule.wait_for_idle();
    assert_eq!(stroke_order(&rule), vec![Color::Blue, Color::Green, Color::Red]);
}

// ---- M15's SegmentedButtonSamples.kt ----------------------------------------------------------------------------

#[kompozable]
fn SegmentedButtonSingleSelectSample() {
    let selected_index = remember(|| mutable_state_of(0usize));
    let options = vec!["Day", "Month", "Week"];
    SingleChoiceSegmentedButtonRow(
        content = move |this| {
            for (index, label) in options.iter().enumerate() {
                let s = selected_index.clone();
                let label = *label;
                this.SegmentedButton(
                    shape = SegmentedButtonDefaults.item_shape(index = index as i32, count = options.len() as i32),
                    on_click = move || s.set(index),
                    selected = index == selected_index.get(),
                    label = move || Text(label),
                );
            }
        },
    );
}

#[kompozable]
fn SegmentedButtonMultiSelectSample() {
    let checked_list = remember(|| mutable_state_list_of(Vec::<usize>::new()));
    let options = vec!["Favorites", "Trending", "Saved"];
    let icons =
        vec![Icons::Filled::StarBorder(), Icons::AutoMirrored::Filled::TrendingUp(), Icons::Filled::BookmarkBorder()];
    MultiChoiceSegmentedButtonRow(
        content = move |this| {
            for (index, label) in options.iter().enumerate() {
                let label = *label;
                let (icon_list, icon_checked) = (icons.clone(), checked_list.clone());
                let toggled = checked_list.clone();
                this.SegmentedButton(
                    shape = SegmentedButtonDefaults.item_shape(index = index as i32, count = options.len() as i32),
                    icon = Some(Rc::new(move || {
                        let icon = icon_list[index].clone();
                        SegmentedButtonDefaults.Icon(
                            active = icon_checked.with(|l| l.contains(&index)),
                            inactive_content = Some(Rc::new(move || {
                                VectorIcon(
                                    image_vector = icon.clone(),
                                    content_description = None,
                                    modifier = Modifier.size(SegmentedButtonDefaults::IconSize),
                                )
                            })),
                        );
                    })),
                    on_checked_change = move |_| {
                        if toggled.with(|l| l.contains(&index)) {
                            toggled.retain(|i| *i != index);
                        } else {
                            toggled.add(index);
                        }
                    },
                    checked = checked_list.with(|l| l.contains(&index)),
                    label = move || Text(label),
                );
            }
        },
    );
}

fn segmented_button_samples() {
    // SegmentedButtonSingleSelectSample: one picked at a time, Day first.
    let rule = KompozTestRule::new(kompozable_lambda!(|| SegmentedButtonSingleSelectSample()));
    let selected = |rule: &KompozTestRule, t: &str| {
        rule.on_node_with_text(t).fetch_semantics_node().config().get(SemanticsProperties::Selected)
    };
    assert_eq!(selected(&rule, "Day"), Some(true));
    rule.on_node_with_text("Week").perform_click();
    rule.wait_for_idle();
    assert_eq!((selected(&rule, "Day"), selected(&rule, "Week")), (Some(false), Some(true)));
    // Three segments in a row as wide as the widest's share, each as wide.
    let widths: Vec<f32> = ["Day", "Month", "Week"]
        .iter()
        .map(|t| rule.on_node_with_text(t).get_unclipped_bounds_in_root().width())
        .collect();
    assert!(near(widths[0], widths[1], 1.0) && near(widths[1], widths[2], 1.0), "{widths:?}");

    // SegmentedButtonMultiSelectSample: each toggled on its own, its icon crossfading to the check mark.
    let rule = KompozTestRule::new(kompozable_lambda!(|| SegmentedButtonMultiSelectSample()));
    let state = |rule: &KompozTestRule, t: &str| {
        rule.on_node_with_text(t).fetch_semantics_node().config().get(SemanticsProperties::ToggleableState)
    };
    let star = Icons::Filled::StarBorder().id();
    assert!(rule.draw().vectors.contains(&star));
    rule.on_node_with_text("Favorites").perform_click();
    rule.on_node_with_text("Saved").perform_click();
    rule.wait_for_idle();
    assert_eq!(state(&rule, "Favorites"), Some(ToggleableState::On));
    assert_eq!(state(&rule, "Trending"), Some(ToggleableState::Off));
    assert_eq!(state(&rule, "Saved"), Some(ToggleableState::On));
    assert!(!rule.draw().vectors.contains(&star), "crossfaded to the check");
    rule.on_node_with_text("Favorites").perform_click();
    rule.wait_for_idle();
    assert_eq!(state(&rule, "Favorites"), Some(ToggleableState::Off));
    assert!(rule.draw().vectors.contains(&star));
}
