//! Material's cards, chips and badges as Compose's material3 (1.5.0-alpha17) has them: the C++ Kompoz's tests by
//! name (kompoz_tests.cpp, with M15's values where the C++ followed 1.4), Compose's behaviour (each chip kind's
//! paddings, heights and colors per state, filter chips toggling, input chips' avatar, cards clicked and raised as
//! they are pressed, badges' sizes and text), and the M15 samples of CardSamples.kt, ChipSamples.kt and
//! BadgeSamples.kt.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::interaction::{DragInteraction, MutableInteractionSource, PressInteraction};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas, has_content_description, has_role, has_text, is_selected};
use kompoz_text::TextStyle;
use kompoz_ui::input::{Key, KeyModifiers, PointerType};
use kompoz_ui::layout::FirstBaseline;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::semantics::{Role, SemanticsProperties};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

tests!(
    cards_as_compose_1_4,
    chips_as_compose_1_4,
    elevated_chips_cast_a_shadow_and_leading_icon_tabs_sit_side_by_side,
    badges_sit_on_the_top_end_corner,
    clickable_cards_click_and_rise_as_they_are_pressed,
    card_defaults_follow_the_tokens,
    card_colors_copy_keeps_what_is_unspecified,
    chip_defaults_follow_the_tokens,
    chip_borders_follow_enabled_and_selected,
    chip_kinds_take_their_paddings_and_heights,
    chip_colors_follow_the_state_and_selection,
    filter_chips_toggle_and_are_checkboxes,
    input_chip_avatars_are_round_and_fade_when_disabled,
    chip_elevations_follow_interactions,
    chip_arrangements_place_the_parts,
    deprecated_chip_overloads_still_compose,
    badge_sizes_padding_and_text_style,
    badged_box_keeps_the_anchors_size_and_baseline,
    card_samples,
    chip_samples,
    badge_samples,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// A node's bounds in the unmerged tree (a chip's parts: merged into the chip).
fn part_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
}

fn same_rect(a: Rect, b: Rect) -> bool {
    near(a.left, b.left, 0.01)
        && near(a.top, b.top, 0.01)
        && near(a.width(), b.width(), 0.01)
        && near(a.height(), b.height(), 0.01)
}

/// A nullable composable slot (Kotlin's `(@Composable () -> Unit)?`).
fn slot(f: impl Fn() + 'static) -> Option<Rc<dyn Fn()>> {
    Some(Rc::new(f))
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

/// The fill drawn exactly over `r`: its color and corner radius.
fn fill_of(canvas: &RecordingCanvas, r: Rect) -> Option<(Color, f32)> {
    canvas
        .rects
        .iter()
        .zip(canvas.rect_colors.iter().zip(&canvas.rect_radii))
        .find(|(x, _)| same_rect(**x, r))
        .map(|(_, (c, radius))| (*c, *radius))
}

/// The line drawn exactly around `r`: its color.
fn line_of(canvas: &RecordingCanvas, r: Rect) -> Option<Color> {
    canvas.strokes.iter().zip(&canvas.stroke_colors).find(|(x, _)| same_rect(**x, r)).map(|(_, c)| *c)
}

/// A chip's 32 dp container in its (48 dp tall) touch area.
fn chip_container(b: Rect) -> Rect {
    Rect::new(b.left, b.center().y - 16.0, b.right, b.center().y + 16.0)
}

type Seen = Rc<RefCell<HashMap<&'static str, Color>>>;

/// A part of a chip `width` by 18 dp tagged `name`, noting its content color.
fn part(seen: &Seen, name: &'static str, width: f32) -> Rc<dyn Fn()> {
    let seen = seen.clone();
    Rc::new(move || {
        seen.borrow_mut().insert(name, LocalContentColor.current());
        named_args!(Box(modifier = Modifier.size_wh(Dp(width), Dp(18.0)).test_tag(name)));
    })
}

fn cards_as_compose_1_4() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Card(
                modifier = Modifier.size_wh(100.dp(), 40.dp()).test_tag("dolu"),
                content = || {
                    Box(modifier = Modifier.size_wh(200.dp(), 40.dp()));
                },
            );
            ElevatedCard(modifier = Modifier.size_wh(100.dp(), 40.dp()).test_tag("yüksek"), content = || {});
            OutlinedCard(modifier = Modifier.size_wh(100.dp(), 40.dp()).test_tag("çizgili"), content = || {});
            ClickableCard(|| {}, modifier = Modifier.size_wh(100.dp(), 40.dp()).test_tag("tıklanır"), content = || {});
        });
    }));
    let cs = light_color_scheme();
    let canvas = rule.draw();
    let fill = |tag: &str| fill_of(&canvas, bounds(&rule, tag));
    let (filled, elevated, outlined, clickable) = (fill("dolu"), fill("yüksek"), fill("çizgili"), fill("tıklanır"));
    assert!(filled.is_some_and(|(c, r)| c == cs.surface_container_highest && near(r, 12.0, 0.01)), "{filled:?}");
    assert!(clickable.is_some_and(|(c, r)| c == cs.surface_container_highest && near(r, 12.0, 0.01)));
    assert!(elevated.is_some_and(|(c, r)| c == cs.surface_container_low && near(r, 12.0, 0.01)), "{elevated:?}");
    assert!(outlined.is_some_and(|(c, r)| c == cs.surface && near(r, 12.0, 0.01)), "{outlined:?}");
    // Their content clipped to their corners.
    let card = bounds(&rule, "dolu");
    assert!(
        canvas.clip_shapes.iter().any(|k| same_rect(k.bounds, card) && near(k.radii.top_left, 12.0, 0.01)),
        "{:?}",
        canvas.clip_shapes
    );
    // The elevated one's shadow the one of 1 dp; none for the others.
    assert!(near(shadow_at(&canvas, bounds(&rule, "yüksek")), 1.0, 0.01));
    assert!(near(shadow_at(&canvas, card), 0.0, 0.01));
}

fn chips_as_compose_1_4() {
    let picked = mutable_state_of(false);
    let seen: Seen = Rc::default();
    let (p, s) = (picked.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (p, s) = (p.clone(), s.clone());
        Column(move || {
            let s = s.clone();
            let label = |name: &'static str| {
                let part = part(&s, name, 40.0);
                move || part()
            };
            AssistChip(
                || {},
                label("yardım"),
                trailing_icon = Some(part(&s, "yardım-son", 18.0)),
                modifier = Modifier.test_tag("yardım-çip"),
            );
            SuggestionChip(|| {}, label("öneri"), icon = Some(part(&s, "öneri-baş", 18.0)));
            InputChip(false, || {}, label("giriş"), modifier = Modifier.test_tag("giriş-çip"));
            InputChip(
                true,
                || {},
                label("seçili-giriş"),
                leading_icon = Some(part(&s, "seçili-giriş-baş", 18.0)),
                trailing_icon = Some(part(&s, "seçili-giriş-son", 18.0)),
                modifier = Modifier.test_tag("seçili-giriş-çip"),
            );
            // The avatar before an icon, in the label's color.
            InputChip(
                true,
                || {},
                label("avatarlı"),
                leading_icon = Some(part(&s, "avatarsız-simge", 18.0)),
                avatar = Some(part(&s, "avatar", 24.0)),
                modifier = Modifier.test_tag("avatarlı-çip"),
            );
            FilterChip(
                p.get(),
                || {},
                label("filtre"),
                leading_icon = if p.get() { Some(part(&s, "tik", 18.0)) } else { None },
                modifier = Modifier.test_tag("filtre-çip"),
            );
            FilterChip(true, || {}, label("kapalı"), enabled = false, modifier = Modifier.test_tag("kapalı-çip"));
            FilterChip(
                false,
                || {},
                label("kapalı-boş"),
                enabled = false,
                modifier = Modifier.test_tag("kapalı-boş-çip"),
            );
        });
    }));
    let cs = light_color_scheme();
    // 8 dp around and between the parts: 16 at an end without an icon (an input chip's 12).
    let (assist, assist_end) = (part_bounds(&rule, "yardım"), part_bounds(&rule, "yardım-son"));
    assert!(near(assist_end.left - assist.right, 8.0, 0.01), "{assist:?} {assist_end:?}");
    let (input, input_chip) = (part_bounds(&rule, "giriş"), bounds(&rule, "giriş-çip"));
    assert!(near(input_chip.width(), 64.0, 0.01) && near(input.left, input_chip.left + 12.0, 0.01), "{input_chip:?}");
    let (with_avatar, avatar) = (bounds(&rule, "avatarlı-çip"), part_bounds(&rule, "avatar"));
    assert!(
        near(with_avatar.width(), 4.0 + 24.0 + 56.0 + 4.0, 0.01) && near(avatar.left, with_avatar.left + 4.0, 0.01)
    );
    let both = bounds(&rule, "seçili-giriş-çip");
    assert!(near(both.width(), 8.0 + 18.0 + 56.0 + 18.0 + 8.0, 0.01), "{both:?}");
    // Colors: an assist chip's label the surface's text, its icons primary; a suggestion chip's label the variant;
    // a selected input chip's leading icon primary, its trailing one onSecondaryContainer.
    {
        let seen = seen.borrow();
        assert!(seen["yardım"] == cs.on_surface && seen["yardım-son"] == cs.primary);
        assert!(seen["öneri"] == cs.on_surface_variant && seen["öneri-baş"] == cs.primary);
        assert!(seen["giriş"] == cs.on_surface_variant && seen["avatar"] == cs.on_secondary_container);
        assert!(!seen.contains_key("avatarsız-simge"));
        assert!(seen["seçili-giriş"] == cs.on_secondary_container && seen["seçili-giriş-baş"] == cs.primary);
        assert!(seen["seçili-giriş-son"] == cs.on_secondary_container);
    }
    // Disabled: selected on the surface's text at 12%, unselected in a line of it.
    let canvas = rule.draw();
    let on_surface_12 = named_args!(cs.on_surface.copy(alpha = 0.12));
    assert_eq!(fill_of(&canvas, chip_container(bounds(&rule, "kapalı-çip"))).map(|f| f.0), Some(on_surface_12));
    assert_eq!(line_of(&canvas, chip_container(bounds(&rule, "kapalı-boş-çip"))), Some(on_surface_12));
    assert_eq!(line_of(&canvas, chip_container(bounds(&rule, "filtre-çip"))), Some(cs.outline_variant));
    assert_eq!(seen.borrow()["kapalı"], named_args!(cs.on_surface.copy(alpha = 0.38)));
    // A filter chip's check grows in and its room shrinks away; it says it's a checkbox.
    assert!(near(bounds(&rule, "filtre-çip").width(), 72.0, 0.01));
    picked.set(true);
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    let growing = bounds(&rule, "filtre-çip").width();
    assert!(growing > 73.0 && growing < 89.0, "{growing}");
    rule.advance_time_by(1000);
    assert!(near(bounds(&rule, "filtre-çip").width(), 8.0 + 18.0 + 56.0 + 8.0, 0.01));
    assert_eq!(seen.borrow()["tik"], cs.on_secondary_container);
    picked.set(false);
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    let shrinking = bounds(&rule, "filtre-çip").width();
    assert!(shrinking > 73.0 && shrinking < 89.0, "{shrinking}");
    rule.advance_time_by(1000);
    assert!(near(bounds(&rule, "filtre-çip").width(), 72.0, 0.01));
    // The filter and input chips; the selected three (one disabled).
    let checkboxes = rule.on_all_nodes(has_role(Role::Checkbox)).fetch_semantics_nodes();
    assert_eq!(checkboxes.len(), 6);
    assert_eq!(checkboxes.iter().filter(|n| is_selected().matches(n)).count(), 3);
}

fn elevated_chips_cast_a_shadow_and_leading_icon_tabs_sit_side_by_side() {
    // The chips and (WP5) the leading icon tabs of a PrimaryTabRow.
    let on = mutable_state_of(false);
    let enabled = mutable_state_of(true);
    let assists = Rc::new(Cell::new(0));
    let tab = mutable_state_of(0);
    let (o, e, a, t) = (on.clone(), enabled.clone(), assists.clone(), tab.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (o, e, a, t) = (o.clone(), e.clone(), a.clone(), t.clone());
            Column(move || {
                let t = t.clone();
                let a = a.clone();
                ElevatedAssistChip(
                    move || a.set(a.get() + 1),
                    || Text("Yol tarifi"),
                    enabled = e.get(),
                    modifier = Modifier.test_tag("assist"),
                );
                let o2 = o.clone();
                ElevatedFilterChip(
                    o.get(),
                    move || o2.set(!o2.get()),
                    || Text("Açık"),
                    modifier = Modifier.test_tag("filter"),
                );
                ElevatedSuggestionChip(|| {}, || Text("Öneri"), modifier = Modifier.test_tag("suggestion"));
                AssistChip(|| {}, || Text("Düz"), modifier = Modifier.test_tag("plain"));
                PrimaryTabRow(
                    t.get(),
                    modifier = Modifier.test_tag("tabs"),
                    tabs = move || {
                        for i in 0..2 {
                            let t2 = t.clone();
                            LeadingIconTab(
                                t.get() == i,
                                move || t2.set(i),
                                text = move || Text(if i == 0 { "Uçuş" } else { "Otel" }),
                                icon = move || {
                                    let (icon, description) = if i == 0 {
                                        (Icons::Filled::Star(), "uçak")
                                    } else {
                                        (Icons::Filled::Home(), "ev")
                                    };
                                    VectorIcon(icon, Some(description.into()));
                                },
                                modifier = Modifier.test_tag(format!("tab{i}")),
                                unselected_content_color = Some(MaterialTheme::color_scheme().on_surface_variant),
                            );
                        }
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    let cs = light_color_scheme();
    struct Look {
        shadows: usize,
        borders: usize,
        container: Option<Color>,
    }
    let look = |tag: &str| {
        let canvas = rule.draw();
        let b = bounds(&rule, tag);
        let shadows =
            canvas.elevation_shadows.iter().filter(|e| b.contains(e.bounds.center()) && e.elevation() > 0.0).count();
        let container = canvas
            .rects
            .iter()
            .zip(canvas.rect_colors.iter().zip(&canvas.rect_radii))
            .find(|(r, (_, radius))| {
                b.contains(r.center())
                    && near(r.width(), b.width(), 0.01)
                    && near(r.height(), 32.0, 0.01)
                    && near(**radius, 8.0, 0.01)
            })
            .map(|(_, (c, _))| *c);
        let borders = canvas
            .strokes
            .iter()
            .zip(&canvas.stroke_colors)
            .filter(|(r, c)| b.contains(r.center()) && c.alpha() > 0.0)
            .count();
        Look { shadows, borders, container }
    };
    // On surfaceContainerLow, a shadow and no border; the plain chip keeps its border.
    for tag in ["assist", "filter", "suggestion"] {
        let l = look(tag);
        assert!(l.shadows > 0 && l.borders == 0 && l.container == Some(cs.surface_container_low), "{tag}");
        // 32 dp tall in 48 dp of room (the node's bounds the clickable's, inside the room, as Compose's)
        assert!(near(bounds(&rule, tag).height(), 32.0, 0.01), "{:?}", bounds(&rule, tag));
    }
    for (above, below) in [("assist", "filter"), ("filter", "suggestion"), ("suggestion", "plain")] {
        assert!(near(bounds(&rule, below).top - bounds(&rule, above).top, 48.0, 0.01));
    }
    let plain = look("plain");
    assert!(plain.shadows == 0 && plain.borders == 1 && plain.container != Some(cs.surface_container_low));
    // Selected: secondaryContainer.
    rule.on_node_with_text("Açık").perform_click();
    rule.wait_for_idle();
    assert!(on.get() && look("filter").container == Some(cs.secondary_container) && look("filter").shadows > 0);
    // Disabled: a faint container, no shadow, no clicks.
    rule.on_node_with_text("Yol tarifi").perform_click();
    assert_eq!(assists.get(), 1);
    enabled.set(false);
    rule.wait_for_idle();
    let off = look("assist");
    assert!(off.shadows == 0 && off.container == Some(named_args!(cs.on_surface.copy(alpha = 0.12))));
    rule.on_node_with_text("Yol tarifi").perform_click();
    assert_eq!(assists.get(), 1);
    // Leading icon tabs: 48 tall, the icon 8 dp before the text, both centered in the tab.
    let t0 = bounds(&rule, "tab0");
    let icon = named_args!(rule.on_node_with_content_description("uçak", use_unmerged_tree = true))
        .get_unclipped_bounds_in_root();
    let text = named_args!(rule.on_node_with_text("Uçuş", use_unmerged_tree = true)).get_unclipped_bounds_in_root();
    assert!(near(t0.height(), 48.0, 0.01) && near(bounds(&rule, "tabs").height(), 48.0, 0.01), "{t0:?}");
    assert!(near(text.left, icon.right + 8.0, 0.01), "{icon:?} {text:?}");
    assert!(near(icon.center().y, t0.center().y, 0.5) && near(text.center().y, t0.center().y, 0.5));
    // centered at whole px (the half px to the right)
    assert_eq!(icon.left - t0.left, ((t0.width() - (text.right - icon.left)) / 2.0 + 0.5).floor(), "{t0:?} {icon:?}");
    // The selected one in primary (the other as asked), its indicator under both.
    let canvas = rule.draw();
    for (t, c) in canvas.texts.iter().zip(&canvas.colors) {
        if t == "Uçuş" {
            assert_eq!(*c, cs.primary);
        }
        if t == "Otel" {
            assert_eq!(*c, cs.on_surface_variant);
        }
    }
    assert_eq!(canvas.vector_colors.iter().filter(|c| **c == cs.primary).count(), 1);
    let indicator = canvas.rects.iter().rev().find(|r| near(r.height(), 3.0, 0.01)).copied().expect("the indicator");
    assert!(
        // (within a px: the indicator centered at whole px in the tab, the row centered by its arrangement)
        near(indicator.left, icon.left, 1.0) && near(indicator.width(), text.right - icon.left, 1.0),
        "{indicator:?} {icon:?} {text:?}"
    );
    rule.on_node_with_text("Otel").perform_click();
    assert_eq!(tab.get(), 1);
}

fn badges_sit_on_the_top_end_corner() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Row(
            modifier = Modifier.padding(20.dp()),
            horizontal_arrangement = Arrangement::spaced_by(20.dp()),
            content = || {
                BadgedBox(
                    || Badge(),
                    modifier = Modifier.test_tag("dot"),
                    content = || {
                        VectorIcon(Icons::Filled::Email(), Some("Posta".into()));
                    },
                );
                BadgedBox(
                    || Badge(content = slot(|| Text("8"))),
                    modifier = Modifier.test_tag("count"),
                    content = || VectorIcon(Icons::Filled::Notifications(), Some("Bildirim".into())),
                );
            },
        );
    }));
    // the badge doesn't grow the box
    assert!(same_rect(bounds(&rule, "dot"), Rect::new(20.0, 20.0, 44.0, 44.0)), "{:?}", bounds(&rule, "dot"));
    let count = rule.on_node_with_text("8").get_unclipped_bounds_in_root();
    assert!(count.left > 64.0 + 24.0 - 13.0 && count.top < 20.0, "{count:?}");
    // As Compose's: the dot 6 dp in from the end, at the top; the count's 16 dp badge 12 dp in from the end, its
    // bottom 14 dp below the top.
    let canvas = rule.draw();
    let dot = canvas.rects.iter().any(|r| same_rect(*r, Rect::new(38.0, 20.0, 44.0, 26.0)));
    let badge = canvas.rects.iter().any(|r| {
        near(r.height(), 16.0, 0.01) && near(r.left, 64.0 + 24.0 - 12.0, 0.01) && near(r.bottom, 20.0 + 14.0, 0.01)
    });
    assert!(dot && badge, "{:?}", canvas.rects);
}

fn clickable_cards_click_and_rise_as_they_are_pressed() {
    // Compose's Card(onClick): clicked unless disabled, its disabled colors then; its shadow animating between its
    // elevations as it is hovered (3 dp), pressed (1 dp), dragged (8 dp), snapping to the disabled one.
    let clicks = Rc::new(Cell::new(0));
    let source = MutableInteractionSource::new();
    let enabled = mutable_state_of(true);
    let (c, src, e) = (clicks.clone(), source.clone(), enabled.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (c, src, e) = (c.clone(), src.clone(), e.clone());
        Column(move || {
            let (c1, c2) = (c.clone(), c.clone());
            ClickableElevatedCard(
                move || c1.set(c1.get() + 1),
                modifier = Modifier.size_wh(100.dp(), 40.dp()).test_tag("yüksek"),
                enabled = e.get(),
                interaction_source = Some(src.clone()),
                content = || Text("kart"),
            );
            ClickableCard(|| {}, modifier = Modifier.size_wh(100.dp(), 40.dp()).test_tag("dolu"), content = || {});
            ClickableCard(
                move || c2.set(c2.get() + 100),
                enabled = false,
                modifier = Modifier.size_wh(100.dp(), 40.dp()).test_tag("kapalı"),
                content = || {},
            );
            ClickableOutlinedCard(
                || {},
                enabled = false,
                modifier = Modifier.size_wh(100.dp(), 40.dp()).test_tag("çizgili-kapalı"),
                content = || {},
            );
        });
    }));
    let cs = light_color_scheme();
    let shadow = |tag: &str| shadow_at(&rule.draw(), bounds(&rule, tag));
    assert!(near(shadow("yüksek"), 1.0, 0.01));
    rule.host().pointer_move(bounds(&rule, "yüksek").center(), -1, 0);
    rule.advance_time_by(300);
    assert!(near(shadow("yüksek"), 3.0, 0.01), "{}", shadow("yüksek"));
    rule.host().pointer_move(bounds(&rule, "dolu").center(), -1, 0);
    rule.advance_time_by(300);
    assert!(near(shadow("yüksek"), 1.0, 0.01) && near(shadow("dolu"), 1.0, 0.01));
    // Pressed: back to 1 dp, though hovered; a click on release.
    let at = bounds(&rule, "yüksek").center();
    rule.host().pointer_move(at, -1, 0);
    rule.host().pointer_down(at, -1, 0, PointerType::Mouse);
    rule.advance_time_by(300);
    assert!(near(shadow("yüksek"), 1.0, 0.01));
    rule.host().pointer_up(at, -1, 0);
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 1);
    rule.advance_time_by(300);
    assert!(near(shadow("yüksek"), 3.0, 0.01));
    // Dragged: 8 dp, then back to the hover's.
    let start = DragInteraction::Start::new();
    assert!(source.try_emit(start));
    rule.advance_time_by(300);
    assert!(near(shadow("yüksek"), 8.0, 0.01), "{}", shadow("yüksek"));
    assert!(source.try_emit(DragInteraction::Stop::new(start)));
    rule.advance_time_by(300);
    assert!(near(shadow("yüksek"), 3.0, 0.01));
    // Disabled: no clicks, the disabled colors (the surface variant at 38% over the container; an outlined card's
    // line the outline at 12% over the low surface container).
    rule.on_node_with_tag("kapalı").perform_click();
    assert_eq!(clicks.get(), 1);
    let canvas = rule.draw();
    let disabled = named_args!(cs.surface_variant.copy(alpha = 0.38)).composite_over(cs.surface_container_highest);
    assert_eq!(fill_of(&canvas, bounds(&rule, "kapalı")).map(|f| f.0), Some(disabled));
    let outline = named_args!(cs.outline.copy(alpha = 0.12)).composite_over(cs.surface_container_low);
    assert_eq!(line_of(&canvas, bounds(&rule, "çizgili-kapalı")), Some(outline));
    assert_eq!(fill_of(&canvas, bounds(&rule, "çizgili-kapalı")).map(|f| f.0), Some(cs.surface));
    rule.on_node_with_tag("kapalı").assert_is_not_enabled();
    // Disabled while hovered: the disabled elevation (1 dp) at once, the surface at 38% over itself.
    enabled.set(false);
    rule.advance_frame();
    rule.advance_frame();
    let canvas = rule.draw();
    assert!(near(shadow_at(&canvas, bounds(&rule, "yüksek")), 1.0, 0.01));
    let elevated_disabled = named_args!(cs.surface.copy(alpha = 0.38)).composite_over(cs.surface);
    assert_eq!(fill_of(&canvas, bounds(&rule, "yüksek")).map(|f| f.0), Some(elevated_disabled));
    rule.on_node_with_tag("yüksek").perform_click();
    assert_eq!(clicks.get(), 1);
}

fn card_defaults_follow_the_tokens() {
    let cs = light_color_scheme();
    let got: Rc<RefCell<Vec<CardColors>>> = Rc::default();
    let borders: Rc<RefCell<Vec<BorderStroke>>> = Rc::default();
    let elevations: Rc<RefCell<Vec<CardElevation>>> = Rc::default();
    let (g, b, el) = (got.clone(), borders.clone(), elevations.clone());
    let local = Color::hex(0x123456);
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (g, b, el) = (g.clone(), b.clone(), el.clone());
        KompozitionLocalProvider(&[LocalContentColor.provides(local)], move || {
            let cs = MaterialTheme::color_scheme();
            let mut g = g.borrow_mut();
            g.clear();
            g.push(CardDefaults.card_colors());
            g.push(CardDefaults.elevated_card_colors());
            g.push(CardDefaults.outlined_card_colors());
            g.push(CardDefaults.card_colors(container_color = Some(cs.primary)));
            g.push(CardDefaults.card_colors(container_color = Some(Color::hex(0xABCDEF))));
            g.push(CardDefaults.card_colors(disabled_container_color = Some(Color::Red)));
            g.push(
                CardDefaults.outlined_card_colors(container_color = Some(cs.primary), content_color = Some(Color::Red)),
            );
            g.push(CardDefaults.elevated_card_colors(content_color = Some(Color::Red)));
            let mut b = b.borrow_mut();
            b.clear();
            b.push(CardDefaults.outlined_card_border());
            b.push(CardDefaults.outlined_card_border(enabled = false));
            let mut el = el.borrow_mut();
            el.clear();
            el.push(CardDefaults.card_elevation());
            el.push(CardDefaults.card_elevation(dragged_elevation = Dp(99.0)));
            el.push(CardDefaults.card_elevation(pressed_elevation = Dp(99.0)));
            el.push(CardDefaults.outlined_card_elevation(default_elevation = Dp(2.0)));
            el.push(CardDefaults.card_elevation(
                default_elevation = Dp(2.0),
                pressed_elevation = Dp(2.0),
                focused_elevation = Dp(2.0),
                hovered_elevation = Dp(2.0),
                disabled_elevation = Dp(0.0),
            ));
            el.push(CardDefaults.elevated_card_elevation());
            el.push(CardDefaults.card_elevation(
                default_elevation = Dp(1.0),
                pressed_elevation = Dp(1.0),
                focused_elevation = Dp(1.0),
                hovered_elevation = Dp(3.0),
                disabled_elevation = Dp(1.0),
            ));
        });
    }));
    let alpha = |c: Color, a: f32| named_args!(c.copy(alpha = a));
    let g = got.borrow();
    assert_eq!(
        g[0],
        CardColors::new(
            cs.surface_container_highest,
            cs.on_surface,
            alpha(cs.surface_variant, 0.38).composite_over(cs.surface_container_highest),
            alpha(cs.on_surface, 0.38),
        )
    );
    assert_eq!(
        g[1],
        CardColors::new(
            cs.surface_container_low,
            cs.on_surface,
            alpha(cs.surface, 0.38).composite_over(cs.surface),
            alpha(cs.on_surface, 0.38)
        )
    );
    assert_eq!(g[2], CardColors::new(cs.surface, cs.on_surface, cs.surface, alpha(cs.on_surface, 0.38)));
    // A container given: its content color, the disabled one of it at 38%.
    assert_eq!(g[3].container_color, cs.primary);
    assert!(g[3].content_color == cs.on_primary && g[3].disabled_content_color == alpha(cs.on_primary, 0.38));
    // A container not of the scheme, or none (only another color given): the content color around (Kotlin's
    // `contentColorFor(Color.Unspecified)`).
    assert_eq!(g[4].content_color, local);
    assert!(g[5].content_color == local && g[5].disabled_container_color == Color::Red);
    assert_eq!(g[5].container_color, cs.surface_container_highest);
    // An outlined card's disabled content: the container's content color at 38%, not the content color given.
    assert!(g[6].content_color == Color::Red && g[6].disabled_content_color == alpha(cs.on_primary, 0.38));
    assert!(g[7].content_color == Color::Red && g[7].disabled_content_color == alpha(Color::Red, 0.38));
    let b = borders.borrow();
    assert_eq!(b[0], BorderStroke(Dp(1.0), cs.outline_variant));
    assert_eq!(b[1], BorderStroke(Dp(1.0), alpha(cs.outline, 0.12).composite_over(cs.surface_container_low)));
    // Equal but for the dragged elevation (as Compose's equals).
    let e = elevations.borrow();
    assert!(e[0] == e[1] && e[0] != e[2]);
    // An outlined card's pressed, focused and hovered elevations: its default one.
    assert_eq!(e[3], e[4]);
    assert_eq!(e[5], e[6]);
}

fn card_colors_copy_keeps_what_is_unspecified() {
    let colors = CardColors::new(Color::Red, Color::Green, Color::Blue, Color::Black);
    assert_eq!(
        named_args!(colors.copy(content_color = Color::White)),
        CardColors::new(Color::Red, Color::White, Color::Blue, Color::Black)
    );
    assert_eq!(named_args!(colors.copy()), colors);
    assert_eq!(named_args!(colors.copy(container_color = Color::Unspecified)), colors);
    let chip = ChipColors::new(
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Black,
        Color::White,
        Color::Gray,
        Color::Cyan,
        Color::Magenta,
    );
    let copied = named_args!(chip.copy(label_color = Color::Yellow, disabled_trailing_icon_content_color = Color::Red));
    assert!(copied.label_color == Color::Yellow && copied.disabled_trailing_icon_content_color == Color::Red);
    assert!(copied.container_color == Color::Red && copied.leading_icon_content_color == Color::Blue);
    assert_eq!(named_args!(chip.copy()), chip);
    let colors13 = [
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
        Color::hex(0x010203),
        Color::hex(0x040506),
    ];
    let [a, b, c, d, e, f, g, h, i, j, k, l, m] = colors13;
    let selectable = SelectableChipColors::new(a, b, c, d, e, f, g, h, i, j, k, l, m);
    assert_eq!(named_args!(selectable.copy()), selectable);
    assert_eq!(
        named_args!(selectable.copy(selected_container_color = Color::Transparent)),
        SelectableChipColors::new(a, b, c, d, e, f, g, h, Color::Transparent, j, k, l, m)
    );
}

fn chip_defaults_follow_the_tokens() {
    assert_eq!(
        [
            AssistChipDefaults::Height,
            FilterChipDefaults::Height,
            InputChipDefaults::Height,
            SuggestionChipDefaults::Height
        ],
        [Dp(32.0); 4]
    );
    assert_eq!(
        [
            AssistChipDefaults::IconSize,
            FilterChipDefaults::IconSize,
            InputChipDefaults::IconSize,
            SuggestionChipDefaults::IconSize
        ],
        [Dp(18.0); 4]
    );
    assert_eq!(InputChipDefaults::AvatarSize, Dp(24.0));
    assert_eq!(
        [
            AssistChipDefaults::HorizontalSpacing,
            FilterChipDefaults::HorizontalSpacing,
            InputChipDefaults::HorizontalSpacing,
            SuggestionChipDefaults::HorizontalSpacing
        ],
        [Dp(8.0); 4]
    );
    let eight = PaddingValues::new(Dp(8.0), Dp(0.0), Dp(8.0), Dp(0.0));
    assert!(AssistChipDefaults::ContentPadding() == eight && FilterChipDefaults::ContentPadding() == eight);
    assert_eq!(SuggestionChipDefaults::ContentPadding(), eight);
    let p = |s: f32, e: f32| PaddingValues::new(Dp(s), Dp(0.0), Dp(e), Dp(0.0));
    assert_eq!(InputChipDefaults::content_padding(false, false, false), p(4.0, 4.0));
    assert_eq!(InputChipDefaults::content_padding(false, true, false), p(8.0, 4.0));
    assert_eq!(InputChipDefaults::content_padding(true, true, true), p(4.0, 8.0));
    assert_eq!(InputChipDefaults::content_padding(false, true, true), p(8.0, 8.0));

    let chip_colors: Rc<RefCell<Vec<ChipColors>>> = Rc::default();
    let selectable: Rc<RefCell<Vec<SelectableChipColors>>> = Rc::default();
    let shapes_equal = Rc::new(Cell::new(false));
    let (cc, sc, se) = (chip_colors.clone(), selectable.clone(), shapes_equal.clone());
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let mut cc = cc.borrow_mut();
        cc.clear();
        cc.push(AssistChipDefaults.assist_chip_colors());
        cc.push(AssistChipDefaults.elevated_assist_chip_colors());
        cc.push(AssistChipDefaults.elevated_assist_chip_colors(label_color = Some(Color::Red)));
        cc.push(SuggestionChipDefaults.suggestion_chip_colors());
        cc.push(SuggestionChipDefaults.elevated_suggestion_chip_colors());
        cc.push(SuggestionChipDefaults.suggestion_chip_colors(icon_content_color = Color::Red));
        let mut sc = sc.borrow_mut();
        sc.clear();
        sc.push(FilterChipDefaults.filter_chip_colors());
        sc.push(FilterChipDefaults.filter_chip_colors(icon_color = Color::Red));
        sc.push(FilterChipDefaults.elevated_filter_chip_colors());
        sc.push(InputChipDefaults.input_chip_colors());
        se.set(
            *AssistChipDefaults::shape() == *FilterChipDefaults::shape()
                && *InputChipDefaults::shape() == *SuggestionChipDefaults::shape()
                && *AssistChipDefaults::shape() == *SuggestionChipDefaults::shape(),
        );
    }));
    let cs = light_color_scheme();
    let a = |c: Color, x: f32| named_args!(c.copy(alpha = x));
    let t = Color::Transparent;
    let c = chip_colors.borrow();
    let d38 = a(cs.on_surface, 0.38);
    assert_eq!(c[0], ChipColors::new(t, cs.on_surface, cs.primary, cs.primary, t, d38, d38, d38));
    assert_eq!(
        c[1],
        ChipColors::new(
            cs.surface_container_low,
            cs.on_surface,
            cs.primary,
            cs.primary,
            a(cs.on_surface, 0.12),
            d38,
            d38,
            d38
        )
    );
    // Colors given to the elevated assist chip's: over the elevated suggestion chip's (as Compose's), no trailing
    // icon color.
    assert!(c[2].label_color == Color::Red && c[2].trailing_icon_content_color == Color::Unspecified);
    assert_eq!(
        c[3],
        ChipColors::new(t, cs.on_surface_variant, cs.primary, Color::Unspecified, t, d38, d38, Color::Unspecified)
    );
    assert_eq!(c[4].container_color, cs.surface_container_low);
    assert_eq!(c[4].disabled_container_color, a(cs.on_surface, 0.12));
    assert!(c[5].leading_icon_content_color == Color::Red && c[5].trailing_icon_content_color == Color::Unspecified);
    let s = selectable.borrow();
    let osv = cs.on_surface_variant;
    let osc = cs.on_secondary_container;
    assert_eq!(
        s[0],
        SelectableChipColors::new(
            t,
            osv,
            cs.primary,
            osv,
            t,
            d38,
            d38,
            d38,
            cs.secondary_container,
            a(cs.on_surface, 0.12),
            osc,
            osc,
            osc
        )
    );
    assert_eq!(s[1], named_args!(s[0].copy(leading_icon_color = Color::Red, trailing_icon_color = Color::Red)));
    assert_eq!(
        s[2],
        SelectableChipColors::new(
            cs.surface_container_low,
            osv,
            cs.primary,
            osv,
            a(cs.on_surface, 0.12),
            d38,
            d38,
            d38,
            cs.secondary_container,
            a(cs.on_surface, 0.12),
            osc,
            osc,
            osc
        )
    );
    assert_eq!(
        s[3],
        SelectableChipColors::new(
            t,
            osv,
            osv,
            osv,
            t,
            d38,
            d38,
            d38,
            cs.secondary_container,
            a(cs.on_surface, 0.12),
            osc,
            cs.primary,
            osc
        )
    );
    assert!(shapes_equal.get());

    // Elevations.
    let assist = AssistChipDefaults.assist_chip_elevation();
    assert!(assist.elevation == Dp(0.0) && assist.dragged_elevation == Dp(8.0) && assist.hovered_elevation == Dp(0.0));
    let raised = named_args!(AssistChipDefaults.assist_chip_elevation(elevation = Dp(2.0)));
    assert!(
        [raised.pressed_elevation, raised.focused_elevation, raised.hovered_elevation, raised.disabled_elevation]
            == [Dp(2.0); 4]
            && raised.dragged_elevation == Dp(8.0)
    );
    let elevated = AssistChipDefaults.elevated_assist_chip_elevation();
    assert_eq!(elevated, ChipElevation::new(Dp(1.0), Dp(1.0), Dp(1.0), Dp(3.0), Dp(8.0), Dp(0.0)));
    assert_eq!(SuggestionChipDefaults.elevated_suggestion_chip_elevation(), elevated);
    assert_eq!(SuggestionChipDefaults.suggestion_chip_elevation(), assist);
    let filter = FilterChipDefaults.filter_chip_elevation();
    assert_eq!(filter, SelectableChipElevation::new(Dp(0.0), Dp(0.0), Dp(0.0), Dp(1.0), Dp(8.0), Dp(0.0)));
    assert_eq!(named_args!(FilterChipDefaults.filter_chip_elevation(elevation = Dp(5.0))).disabled_elevation, Dp(5.0));
    assert_eq!(
        FilterChipDefaults.elevated_filter_chip_elevation(),
        SelectableChipElevation::new(Dp(1.0), Dp(1.0), Dp(1.0), Dp(3.0), Dp(8.0), Dp(0.0))
    );
    assert_eq!(
        named_args!(InputChipDefaults.input_chip_elevation(elevation = Dp(2.0))),
        SelectableChipElevation::new(Dp(2.0), Dp(2.0), Dp(2.0), Dp(2.0), Dp(8.0), Dp(2.0))
    );
    // Equal but for the dragged elevation.
    assert_eq!(
        ChipElevation::new(Dp(1.0), Dp(1.0), Dp(1.0), Dp(1.0), Dp(1.0), Dp(1.0)),
        ChipElevation::new(Dp(1.0), Dp(1.0), Dp(1.0), Dp(1.0), Dp(9.0), Dp(1.0))
    );
    assert_ne!(
        ChipElevation::new(Dp(1.0), Dp(1.0), Dp(1.0), Dp(1.0), Dp(1.0), Dp(1.0)),
        ChipElevation::new(Dp(1.0), Dp(2.0), Dp(1.0), Dp(1.0), Dp(1.0), Dp(1.0))
    );
}

fn chip_borders_follow_enabled_and_selected() {
    let got: Rc<RefCell<Vec<BorderStroke>>> = Rc::default();
    let g = got.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let mut g = g.borrow_mut();
        g.clear();
        g.push(AssistChipDefaults.assist_chip_border(true));
        g.push(AssistChipDefaults.assist_chip_border(false));
        g.push(FilterChipDefaults.filter_chip_border(true, false));
        g.push(FilterChipDefaults.filter_chip_border(true, true));
        g.push(FilterChipDefaults.filter_chip_border(false, false));
        g.push(FilterChipDefaults.filter_chip_border(false, true));
        g.push(InputChipDefaults.input_chip_border(true, false));
        g.push(InputChipDefaults.input_chip_border(true, true));
        g.push(InputChipDefaults.input_chip_border(false, false));
        g.push(SuggestionChipDefaults.suggestion_chip_border(true));
        g.push(SuggestionChipDefaults.suggestion_chip_border(false));
        g.push(FilterChipDefaults.filter_chip_border(
            true,
            true,
            selected_border_color = Color::Red,
            selected_border_width = Dp(2.0),
        ));
        g.push(AssistChipDefaults.assist_chip_border(true, border_width = Dp(3.0), border_color = Color::Blue));
    }));
    let cs = light_color_scheme();
    let line = BorderStroke(Dp(1.0), cs.outline_variant);
    let faint = BorderStroke(Dp(1.0), named_args!(cs.on_surface.copy(alpha = 0.12)));
    let none = BorderStroke(Dp(0.0), Color::Transparent);
    let g = got.borrow();
    assert_eq!(
        g[..11],
        [
            line.clone(),
            faint.clone(),
            line.clone(),
            none.clone(),
            faint.clone(),
            none.clone(),
            line.clone(),
            none,
            faint.clone(),
            line,
            faint
        ]
    );
    assert_eq!(g[11], BorderStroke(Dp(2.0), Color::Red));
    assert_eq!(g[12], BorderStroke(Dp(3.0), Color::Blue));
}

fn chip_kinds_take_their_paddings_and_heights() {
    let seen: Seen = Rc::default();
    let style: Rc<RefCell<Vec<TextStyle>>> = Rc::default();
    let (s, st) = (seen.clone(), style.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (s, st) = (s.clone(), st.clone());
            Column(move || {
                let s = s.clone();
                let label = |name: &'static str| {
                    let part = part(&s, name, 40.0);
                    move || part()
                };
                let st1 = st.clone();
                AssistChip(
                    || {},
                    move || {
                        st1.borrow_mut().push(LocalTextStyle.current());
                        named_args!(Box(modifier = Modifier.size_wh(Dp(40.0), Dp(18.0))));
                    },
                    modifier = Modifier.test_tag("yalın"),
                );
                AssistChip(
                    || {},
                    label("a-etiket"),
                    leading_icon = Some(part(&s, "a-baş", 18.0)),
                    modifier = Modifier.test_tag("simgeli"),
                );
                SuggestionChip(
                    || {},
                    label("ö-etiket"),
                    icon = Some(part(&s, "ö-baş", 18.0)),
                    modifier = Modifier.test_tag("öneri"),
                );
                ElevatedSuggestionChip(|| {}, label("yö-etiket"), modifier = Modifier.test_tag("yüksek-öneri"));
                InputChip(
                    false,
                    || {},
                    label("g-etiket"),
                    leading_icon = Some(part(&s, "g-baş", 18.0)),
                    modifier = Modifier.test_tag("giriş-baş"),
                );
                InputChip(
                    false,
                    || {},
                    label("gs-etiket"),
                    trailing_icon = Some(part(&s, "gs-son", 18.0)),
                    modifier = Modifier.test_tag("giriş-son"),
                );
                let st2 = st.clone();
                FilterChip(
                    true,
                    || {},
                    move || {
                        st2.borrow_mut().push(LocalTextStyle.current());
                        named_args!(Box(modifier = Modifier.size_wh(Dp(40.0), Dp(18.0)).test_tag("f-etiket")));
                    },
                    leading_icon = Some(part(&s, "f-baş", 18.0)),
                    horizontal_arrangement = FilterChipDefaults::horizontal_arrangement_with_spacing(4.dp()),
                    modifier = Modifier.test_tag("sık"),
                );
                AssistChip(
                    || {},
                    || Box(modifier = Modifier.size_wh(Dp(40.0), Dp(40.0))),
                    content_padding = PaddingValues::hv(Dp(20.0), Dp(0.0)),
                    modifier = Modifier.test_tag("geniş"),
                );
                FilterChip(
                    false,
                    || {},
                    || Box(modifier = Modifier.size_wh(Dp(1200.0), Dp(18.0))),
                    modifier = Modifier.test_tag("uzun"),
                );
            });
        }),
        IntSize::new(1400, 900),
    );
    let w = |tag: &str| bounds(&rule, tag).width();
    // 8 dp at the sides and between the parts (an empty part none).
    assert!(near(w("yalın"), 72.0, 0.01), "{}", w("yalın"));
    assert!(near(w("simgeli"), 8.0 + 18.0 + 8.0 + 40.0 + 8.0 + 8.0, 0.01));
    assert!(near(part_bounds(&rule, "a-etiket").left, bounds(&rule, "simgeli").left + 8.0 + 18.0 + 8.0, 0.01));
    assert!(near(w("öneri"), 90.0, 0.01) && near(w("yüksek-öneri"), 72.0, 0.01));
    // An input chip: 8 dp before a leading icon, 4 without; 8 after a trailing icon, 4 without.
    assert!(near(w("giriş-baş"), 8.0 + 18.0 + 8.0 + 40.0 + 8.0 + 4.0, 0.01), "{}", w("giriş-baş"));
    assert!(near(part_bounds(&rule, "g-baş").left, bounds(&rule, "giriş-baş").left + 8.0, 0.01));
    assert!(near(w("giriş-son"), 4.0 + 8.0 + 40.0 + 8.0 + 18.0 + 8.0, 0.01));
    assert!(near(part_bounds(&rule, "gs-son").right, bounds(&rule, "giriş-son").right - 8.0, 0.01));
    // An arrangement of 4 dp.
    assert!(near(w("sık"), 8.0 + 18.0 + 4.0 + 40.0 + 4.0 + 8.0, 0.01), "{}", w("sık"));
    assert!(near(part_bounds(&rule, "f-etiket").left, part_bounds(&rule, "f-baş").right + 4.0, 0.01));
    // A padding of one's own; a label taller than the chip makes it taller.
    assert!(near(w("geniş"), 20.0 + 8.0 + 40.0 + 8.0 + 20.0, 0.01));
    assert!(near(bounds(&rule, "geniş").height(), 40.0, 0.01) && near(bounds(&rule, "yalın").height(), 32.0, 0.01));
    // A selectable chip is at most 1000 dp wide.
    assert!(near(w("uzun"), 1000.0, 0.01), "{}", w("uzun"));
    // Their labels in the type scale's label large.
    let label_large = Typography::default().label_large;
    assert!(style.borrow().iter().all(|s| *s == label_large) && style.borrow().len() >= 2);
}

fn chip_colors_follow_the_state_and_selection() {
    let seen: Seen = Rc::default();
    let s = seen.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Column(move || {
                let s = s.clone();
                let chip = |selected: bool, enabled: bool, prefix: &'static str, names: [&'static str; 3]| {
                    let _ = prefix;
                    let label = part(&s, names[0], 40.0);
                    FilterChip(
                        selected,
                        || {},
                        move || label(),
                        enabled = enabled,
                        leading_icon = Some(part(&s, names[1], 18.0)),
                        trailing_icon = Some(part(&s, names[2], 18.0)),
                        modifier = Modifier.test_tag(prefix),
                    );
                };
                chip(false, true, "f", ["f-l", "f-b", "f-s"]);
                chip(true, true, "fs", ["fs-l", "fs-b", "fs-s"]);
                chip(false, false, "fk", ["fk-l", "fk-b", "fk-s"]);
                let label = part(&s, "i-l", 40.0);
                InputChip(
                    false,
                    || {},
                    move || label(),
                    leading_icon = Some(part(&s, "i-b", 18.0)),
                    trailing_icon = Some(part(&s, "i-s", 18.0)),
                );
                let label = part(&s, "ik-l", 40.0);
                InputChip(true, || {}, move || label(), enabled = false, trailing_icon = Some(part(&s, "ik-s", 18.0)));
                let label = part(&s, "ö-l", 40.0);
                SuggestionChip(|| {}, move || label(), enabled = false, icon = Some(part(&s, "ö-b", 18.0)));
                let label = part(&s, "a-l", 40.0);
                AssistChip(|| {}, move || label(), enabled = false, trailing_icon = Some(part(&s, "a-s", 18.0)));
                let label = part(&s, "ar-l", 40.0);
                AssistChip(
                    || {},
                    move || label(),
                    colors = AssistChipDefaults.assist_chip_colors(label_color = Color::Red),
                );
                ElevatedFilterChip(true, || {}, || {}, enabled = false, modifier = Modifier.test_tag("yfk"));
                ElevatedFilterChip(false, || {}, || {}, enabled = false, modifier = Modifier.test_tag("yfbk"));
                ElevatedSuggestionChip(|| {}, || {}, enabled = false, modifier = Modifier.test_tag("yök"));
            });
        }),
        IntSize::new(400, 900),
    );
    let cs = light_color_scheme();
    let d38 = named_args!(cs.on_surface.copy(alpha = 0.38));
    let d12 = named_args!(cs.on_surface.copy(alpha = 0.12));
    let seen = seen.borrow();
    let colors = |names: [&str; 3]| [seen[names[0]], seen[names[1]], seen[names[2]]];
    assert_eq!(colors(["f-l", "f-b", "f-s"]), [cs.on_surface_variant, cs.primary, cs.on_surface_variant]);
    assert_eq!(colors(["fs-l", "fs-b", "fs-s"]), [cs.on_secondary_container; 3]);
    assert_eq!(colors(["fk-l", "fk-b", "fk-s"]), [d38; 3]);
    assert_eq!(colors(["i-l", "i-b", "i-s"]), [cs.on_surface_variant; 3]);
    assert!(seen["ik-l"] == d38 && seen["ik-s"] == d38);
    assert!(seen["ö-l"] == d38 && seen["ö-b"] == d38 && seen["a-l"] == d38 && seen["a-s"] == d38);
    assert_eq!(seen["ar-l"], Color::Red);
    let canvas = rule.draw();
    assert_eq!(fill_of(&canvas, chip_container(bounds(&rule, "fs"))).map(|f| f.0), Some(cs.secondary_container));
    assert_eq!(fill_of(&canvas, chip_container(bounds(&rule, "yfk"))).map(|f| f.0), Some(d12));
    assert_eq!(fill_of(&canvas, chip_container(bounds(&rule, "yfbk"))).map(|f| f.0), Some(d12));
    assert_eq!(fill_of(&canvas, chip_container(bounds(&rule, "yök"))).map(|f| f.0), Some(d12));
}

fn filter_chips_toggle_and_are_checkboxes() {
    let selected = mutable_state_of(false);
    let input = mutable_state_of(false);
    let suggestions = Rc::new(Cell::new(0));
    let (sel, inp, sug) = (selected.clone(), input.clone(), suggestions.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (sel, inp, sug) = (sel.clone(), inp.clone(), sug.clone());
        Column(move || {
            let (s1, i1, g1) = (sel.clone(), inp.clone(), sug.clone());
            FilterChip(sel.get(), move || s1.set(!s1.get()), || Text("Filtre"), modifier = Modifier.test_tag("filtre"));
            FilterChip(
                false,
                || panic!("a disabled chip isn't clicked"),
                || Text("Kapalı"),
                enabled = false,
                modifier = Modifier.test_tag("kapalı"),
            );
            InputChip(inp.get(), move || i1.set(!i1.get()), || Text("Giriş"), modifier = Modifier.test_tag("giriş"));
            SuggestionChip(move || g1.set(g1.get() + 1), || Text("Öneri"), modifier = Modifier.test_tag("öneri"));
            AssistChip(|| {}, || Text("Yardım"), modifier = Modifier.test_tag("yardım"));
        });
    }));
    let role = |tag: &str| rule.on_node_with_tag(tag).fetch_semantics_node().config().get(SemanticsProperties::Role);
    assert_eq!(role("filtre"), Some(Role::Checkbox));
    assert_eq!(role("giriş"), Some(Role::Checkbox));
    assert_eq!(role("öneri"), Some(Role::Button));
    assert_eq!(role("yardım"), Some(Role::Button));
    rule.on_node_with_tag("filtre").assert_is_not_selected().assert_text_equals(&["Filtre"]).perform_click();
    rule.wait_for_idle();
    assert!(selected.get());
    rule.on_node_with_tag("filtre").assert_is_selected().perform_click();
    rule.wait_for_idle();
    assert!(!selected.get());
    rule.on_node_with_tag("kapalı").assert_is_not_enabled().perform_click();
    rule.on_node_with_tag("giriş").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_tag("giriş").assert_is_selected();
    rule.on_node_with_tag("öneri").perform_click();
    assert_eq!(suggestions.get(), 1);
}

fn input_chip_avatars_are_round_and_fade_when_disabled() {
    let enabled = mutable_state_of(false);
    let e = enabled.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        InputChip(
            true,
            || {},
            || Text("Kişi"),
            enabled = e.get(),
            avatar = slot(|| Box(modifier = Modifier.size(24.dp()).background(Color::Red).test_tag("avatar"))),
        );
    }));
    let avatar = part_bounds(&rule, "avatar");
    let canvas = rule.draw();
    // In a circle (CornerFull), at 38% when disabled.
    assert!(canvas.layer_corners.iter().any(|c| near(c.top_left, 12.0, 0.01)), "{:?}", canvas.layer_corners);
    assert!(canvas.layer_alphas.iter().any(|a| near(*a, 0.38, 0.001)), "{:?}", canvas.layer_alphas);
    assert!(near(avatar.width(), 24.0, 0.01));
    enabled.set(true);
    rule.wait_for_idle();
    let canvas = rule.draw();
    assert!(!canvas.layer_alphas.iter().any(|a| near(*a, 0.38, 0.001)));
}

fn chip_elevations_follow_interactions() {
    let source = MutableInteractionSource::new();
    let enabled = mutable_state_of(true);
    let (src, e) = (source.clone(), enabled.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (src, e) = (src.clone(), e.clone());
        Column(move || {
            ElevatedAssistChip(
                || {},
                || Text("Yüksek"),
                modifier = Modifier.test_tag("yüksek"),
                interaction_source = Some(src.clone()),
            );
            AssistChip(|| {}, || Text("Düz"), elevation = None, modifier = Modifier.test_tag("düz"));
            FilterChip(true, || {}, || Text("Seçili"), modifier = Modifier.test_tag("seçili"));
            ElevatedFilterChip(
                false,
                || {},
                || Text("Kapanan"),
                enabled = e.get(),
                modifier = Modifier.test_tag("kapanan"),
            );
        });
    }));
    let shadow = |tag: &str| shadow_at(&rule.draw(), bounds(&rule, tag));
    assert!(near(shadow("yüksek"), 1.0, 0.01) && near(shadow("kapanan"), 1.0, 0.01));
    let at = bounds(&rule, "yüksek").center();
    rule.host().pointer_move(at, -1, 0);
    rule.advance_time_by(300);
    assert!(near(shadow("yüksek"), 3.0, 0.01), "{}", shadow("yüksek"));
    rule.host().pointer_down(at, -1, 0, PointerType::Mouse);
    rule.advance_time_by(300);
    assert!(near(shadow("yüksek"), 1.0, 0.01));
    rule.host().pointer_up(at, -1, 0);
    rule.advance_time_by(300);
    assert!(near(shadow("yüksek"), 3.0, 0.01));
    let press = PressInteraction::Press::new(Offset::Zero);
    let start = DragInteraction::Start::new();
    assert!(source.try_emit(start));
    rule.advance_time_by(300);
    assert!(near(shadow("yüksek"), 8.0, 0.01));
    assert!(source.try_emit(DragInteraction::Cancel::new(start)));
    assert!(source.try_emit(press));
    rule.advance_time_by(300);
    assert!(near(shadow("yüksek"), 1.0, 0.01));
    assert!(source.try_emit(PressInteraction::Release::new(press)));
    rule.advance_time_by(300);
    assert!(near(shadow("yüksek"), 3.0, 0.01));
    // No elevation: no shadow, even hovered; a flat selected filter chip 1 dp hovered.
    rule.host().pointer_move(bounds(&rule, "düz").center(), -1, 0);
    rule.advance_time_by(300);
    assert!(near(shadow("düz"), 0.0, 0.01));
    rule.host().pointer_move(bounds(&rule, "seçili").center(), -1, 0);
    rule.advance_time_by(300);
    assert!(near(shadow("seçili"), 1.0, 0.01), "{}", shadow("seçili"));
    // Disabled: none, at once.
    enabled.set(false);
    rule.advance_frame();
    rule.advance_frame();
    assert!(near(shadow("kapanan"), 0.0, 0.01));
}

fn chip_arrangements_place_the_parts() {
    // Compose's ChipArrangement: the first part at the start, the second `spacing` after it, the third at the end;
    // mirrored right to left. The defaults share one; 8 dp is it.
    let default = AssistChipDefaults::horizontal_arrangement();
    assert!(
        default == FilterChipDefaults::horizontal_arrangement()
            && default == InputChipDefaults::horizontal_arrangement()
    );
    assert!(default == SuggestionChipDefaults::horizontal_arrangement());
    assert!(AssistChipDefaults::horizontal_arrangement_with_spacing(8.dp()) == default);
    let four = FilterChipDefaults::horizontal_arrangement_with_spacing(4.dp());
    assert!(four != default && four.spacing() == Dp(4.0) && default.spacing() == Dp(8.0));
    let density = Density::new(2.0, 1.0);
    let mut out = [0; 3];
    default.arrange(&density, 200, &[20, 40, 20], LayoutDirection::Ltr, &mut out);
    assert_eq!(out, [0, 36, 180]);
    default.arrange(&density, 200, &[20, 40, 20], LayoutDirection::Rtl, &mut out);
    assert_eq!(out, [180, 124, 0]);
    let mut none: [i32; 0] = [];
    default.arrange(&density, 200, &[], LayoutDirection::Ltr, &mut none);
    // Right to left, an input chip's parts mirrored.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
            InputChip(
                false,
                || {},
                || Box(modifier = Modifier.size_wh(40.dp(), 18.dp()).test_tag("etiket")),
                leading_icon = slot(|| Box(modifier = Modifier.size(18.dp()).test_tag("baş"))),
                modifier = Modifier.test_tag("çip"),
            );
        });
    }));
    let (chip, lead, label) = (bounds(&rule, "çip"), part_bounds(&rule, "baş"), part_bounds(&rule, "etiket"));
    assert!(
        near(lead.right, chip.right - 8.0, 0.01) && near(label.right, lead.left - 8.0, 0.01),
        "{chip:?} {lead:?} {label:?}"
    );
}

#[allow(deprecated)]
fn deprecated_chip_overloads_still_compose() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Column(|| {
                let label = || Box(modifier = Modifier.size_wh(40.dp(), 18.dp()));
                AssistChipWithoutArrangement(|| {}, label, modifier = Modifier.test_tag("a1"));
                AssistChipWithChipBorder(
                    || {},
                    label,
                    leading_icon = slot(|| Box(modifier = Modifier.size(18.dp()).test_tag("a2-baş"))),
                    modifier = Modifier.test_tag("a2"),
                );
                ElevatedAssistChipWithoutArrangement(|| {}, label, modifier = Modifier.test_tag("a3"));
                ElevatedAssistChipWithChipBorder(
                    || {},
                    label,
                    border = Some(AssistChipDefaults.assist_chip_chip_border(border_color = Color::Red)),
                    modifier = Modifier.test_tag("a4"),
                );
                FilterChipWithoutArrangement(false, || {}, label, modifier = Modifier.test_tag("f1"));
                ElevatedFilterChipWithoutArrangement(true, || {}, label, modifier = Modifier.test_tag("f2"));
                InputChipWithoutArrangement(false, || {}, label, modifier = Modifier.test_tag("i1"));
                SuggestionChipWithoutArrangement(|| {}, label, modifier = Modifier.test_tag("s1"));
                SuggestionChipWithChipBorder(|| {}, label, enabled = false, modifier = Modifier.test_tag("s2"));
                ElevatedSuggestionChipWithoutArrangement(|| {}, label, modifier = Modifier.test_tag("s3"));
                ElevatedSuggestionChipWithChipBorder(|| {}, label, modifier = Modifier.test_tag("s4"));
            });
        }),
        IntSize::new(400, 900),
    );
    for tag in ["a1", "a3", "a4", "f1", "f2", "s1", "s2", "s3", "s4"] {
        assert!(near(bounds(&rule, tag).width(), 72.0, 0.01), "{tag}: {:?}", bounds(&rule, tag));
    }
    assert!(near(bounds(&rule, "i1").width(), 64.0, 0.01));
    // The ChipBorder ones: the parts 8 dp apart (spaced by 8 dp, centered).
    assert!(near(bounds(&rule, "a2").width(), 90.0, 0.01));
    assert!(near(part_bounds(&rule, "a2-baş").left, bounds(&rule, "a2").left + 8.0, 0.01));
    let cs = light_color_scheme();
    let canvas = rule.draw();
    assert_eq!(line_of(&canvas, chip_container(bounds(&rule, "a2"))), Some(cs.outline_variant));
    assert_eq!(line_of(&canvas, chip_container(bounds(&rule, "a4"))), Some(Color::Red));
    assert_eq!(
        line_of(&canvas, chip_container(bounds(&rule, "s2"))),
        Some(named_args!(cs.on_surface.copy(alpha = 0.12)))
    );
    assert_eq!(line_of(&canvas, chip_container(bounds(&rule, "s4"))), None);
    assert_ne!(
        AssistChipDefaults.assist_chip_chip_border(),
        named_args!(AssistChipDefaults.assist_chip_chip_border(border_width = Dp(2.0)))
    );
}

fn badge_sizes_padding_and_text_style() {
    let style: Rc<RefCell<Option<(TextStyle, Color)>>> = Rc::default();
    let behind = Rc::new(Cell::new(0));
    let (st, b) = (style.clone(), behind.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (st, b) = (st.clone(), b.clone());
        Column(move || {
            Badge(modifier = Modifier.test_tag("nokta"));
            let st = st.clone();
            Badge(
                modifier = Modifier.test_tag("dar"),
                content = slot(move || {
                    *st.borrow_mut() = Some((LocalTextStyle.current(), LocalContentColor.current()));
                    named_args!(Box(modifier = Modifier.size_wh(Dp(4.0), Dp(10.0))));
                }),
            );
            Badge(
                modifier = Modifier.test_tag("geniş"),
                content = slot(|| Box(modifier = Modifier.size_wh(30.dp(), 10.dp()).test_tag("içerik"))),
            );
            Badge(
                modifier = Modifier.test_tag("renkli"),
                container_color = MaterialTheme::color_scheme().primary,
                content = slot(|| Text("1")),
            );
            let b = b.clone();
            Box(move || {
                let b = b.clone();
                Box(modifier = Modifier.size(40.dp()).test_tag("arka").clickable(move || b.set(b.get() + 1)));
                Badge(content = slot(|| Text("9")));
            });
        });
    }));
    let cs = light_color_scheme();
    assert!(same_rect(bounds(&rule, "nokta"), Rect::new(0.0, 0.0, 6.0, 6.0)), "{:?}", bounds(&rule, "nokta"));
    let narrow = bounds(&rule, "dar");
    assert!(near(narrow.width(), 16.0, 0.01) && near(narrow.height(), 16.0, 0.01));
    // 4 dp at each side of a wider content.
    let (wide, inside) = (bounds(&rule, "geniş"), bounds(&rule, "içerik"));
    assert!(near(wide.width(), 38.0, 0.01) && near(inside.left, wide.left + 4.0, 0.01));
    // The content in the label small, on the error color's content color.
    let (text_style, color) = style.borrow().clone().expect("the badge's content");
    assert_eq!(text_style.font_size, Typography::default().label_small.font_size);
    assert_eq!(color, cs.on_error);
    assert_eq!(BadgeDefaults::container_color(), MaterialTheme::color_scheme().error);
    let canvas = rule.draw();
    assert_eq!(fill_of(&canvas, bounds(&rule, "nokta")), Some((cs.error, 3.0)));
    assert_eq!(fill_of(&canvas, narrow), Some((cs.error, 8.0)));
    assert_eq!(fill_of(&canvas, bounds(&rule, "renkli")).map(|f| f.0), Some(cs.primary));
    // It doesn't take the pointer from what is behind it.
    let at = rule.on_node_with_text("9").get_unclipped_bounds_in_root().center();
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.host().pointer_up(at, -1, 0);
    rule.wait_for_idle();
    assert_eq!(behind.get(), 1);
}

fn badged_box_keeps_the_anchors_size_and_baseline() {
    let baselines = Rc::new(RefCell::new(Vec::new()));
    let rtl = mutable_state_of(false);
    let (bl, r) = (baselines.clone(), rtl.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (bl, r) = (bl.clone(), r.clone());
        Column(move || {
            let bl = bl.clone();
            let measured = move |content: Rc<dyn Fn()>| {
                let bl = bl.clone();
                Layout(
                    content = move || content(),
                    measure_policy = kompoz_ui::layout::measure_policy(move |scope, ms, c| {
                        let p = ms[0].measure(c);
                        bl.borrow_mut().push(p.get(FirstBaseline));
                        scope.layout(p.width(), p.height(), move |s| s.place(&p, 0, 0))
                    }),
                );
            };
            measured(Rc::new(|| Text("Gelen")));
            measured(Rc::new(|| {
                BadgedBox(
                    || Badge(content = slot(|| Text("3"))),
                    modifier = Modifier.test_tag("kutu"),
                    content = || Text("Gelen"),
                );
            }));
            let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], || {
                Box(
                    modifier = Modifier.padding(20.dp()),
                    content = || {
                        BadgedBox(
                            || Badge(modifier = Modifier.test_tag("nokta")),
                            modifier = Modifier.test_tag("çapa"),
                            content = || {
                                Box(modifier = Modifier.size(24.dp()));
                            },
                        );
                    },
                );
            });
        });
    }));
    let b = baselines.borrow().clone();
    assert!(b.len() >= 2 && b[0] == b[b.len() - 1] && b[0] > 0, "{b:?}");
    // Its size the anchor's.
    let anchor = bounds(&rule, "çapa");
    assert!(near(anchor.width(), 24.0, 0.01) && near(anchor.height(), 24.0, 0.01));
    let dot = bounds(&rule, "nokta");
    assert!(near(dot.left, anchor.right - 6.0, 0.01) && near(dot.top, anchor.top, 0.01));
    // Right to left: on the top start corner (the end of the layout direction).
    rtl.set(true);
    rule.wait_for_idle();
    let (anchor, dot) = (bounds(&rule, "çapa"), bounds(&rule, "nokta"));
    assert!(near(dot.right, anchor.left + 6.0, 0.01) && near(dot.top, anchor.top, 0.01), "{anchor:?} {dot:?}");
}

// ---- The M15 samples ---------------------------------------------------------------------------------------------

#[kompozable]
fn CardSample() {
    Card(
        modifier = Modifier.size_wh(180.dp(), 100.dp()).test_tag("CardSample"),
        content = || {
            Box(
                modifier = Modifier.fill_max_size(),
                content = || Text("Card content", modifier = Modifier.align(Alignment::Center)),
            );
        },
    );
}

#[kompozable]
fn ClickableCardSample(clicks: Rc<Cell<i32>>) {
    ClickableCard(
        move || clicks.set(clicks.get() + 1),
        modifier = Modifier.size_wh(180.dp(), 100.dp()).test_tag("ClickableCardSample"),
        content = || {
            Box(
                modifier = Modifier.fill_max_size(),
                content = || Text("Clickable", modifier = Modifier.align(Alignment::Center)),
            )
        },
    );
}

#[kompozable]
fn ElevatedCardSample() {
    ElevatedCard(
        modifier = Modifier.size_wh(180.dp(), 100.dp()).test_tag("ElevatedCardSample"),
        content = || {
            Box(
                modifier = Modifier.fill_max_size(),
                content = || Text("Card content", modifier = Modifier.align(Alignment::Center)),
            );
        },
    );
}

#[kompozable]
fn ClickableElevatedCardSample() {
    ClickableElevatedCard(
        || { /* Do something */ },
        modifier = Modifier.size_wh(180.dp(), 100.dp()).test_tag("ClickableElevatedCardSample"),
        content = || {
            Box(
                modifier = Modifier.fill_max_size(),
                content = || Text("Clickable", modifier = Modifier.align(Alignment::Center)),
            )
        },
    );
}

#[kompozable]
fn OutlinedCardSample() {
    OutlinedCard(
        modifier = Modifier.size_wh(180.dp(), 100.dp()).test_tag("OutlinedCardSample"),
        content = || {
            Box(
                modifier = Modifier.fill_max_size(),
                content = || Text("Card content", modifier = Modifier.align(Alignment::Center)),
            );
        },
    );
}

#[kompozable]
fn ClickableOutlinedCardSample() {
    ClickableOutlinedCard(
        || { /* Do something */ },
        modifier = Modifier.size_wh(180.dp(), 100.dp()).test_tag("ClickableOutlinedCardSample"),
        content = || {
            Box(
                modifier = Modifier.fill_max_size(),
                content = || Text("Clickable", modifier = Modifier.align(Alignment::Center)),
            )
        },
    );
}

fn card_samples() {
    let clicks = Rc::new(Cell::new(0));
    let c = clicks.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let c = c.clone();
            Column(move || {
                CardSample();
                ClickableCardSample(c.clone());
                ElevatedCardSample();
                ClickableElevatedCardSample();
                OutlinedCardSample();
                ClickableOutlinedCardSample();
            });
        }),
        IntSize::new(400, 900),
    );
    for tag in [
        "CardSample",
        "ClickableCardSample",
        "ElevatedCardSample",
        "ClickableElevatedCardSample",
        "OutlinedCardSample",
        "ClickableOutlinedCardSample",
    ] {
        let b = bounds(&rule, tag);
        assert!(near(b.width(), 180.0, 0.01) && near(b.height(), 100.0, 0.01), "{tag}: {b:?}");
    }
    // The text in the middle of the card.
    let card = bounds(&rule, "CardSample");
    let text = rule.on_all_nodes_with_text("Card content").on_first().get_unclipped_bounds_in_root();
    assert!(near(text.center().x, card.center().x, 0.6) && near(text.center().y, card.center().y, 0.6));
    rule.on_node_with_tag("ClickableCardSample").assert_has_click_action().perform_click();
    assert_eq!(clicks.get(), 1);
    rule.on_node_with_tag("ClickableOutlinedCardSample").assert_has_click_action();
    rule.on_node_with_tag("CardSample").assert_has_no_click_action();
}

#[kompozable]
fn AssistChipSample() {
    AssistChip(
        || { /* Do something! */ },
        || Text("Assist Chip"),
        leading_icon = slot(|| {
            VectorIcon(
                Icons::Filled::Settings(),
                Some("Localized description".into()),
                modifier = Modifier.size(AssistChipDefaults::IconSize),
            );
        }),
    );
}

#[kompozable]
fn ElevatedAssistChipSample() {
    ElevatedAssistChip(
        || { /* Do something! */ },
        || Text("Elevated Assist Chip"),
        leading_icon = slot(|| {
            VectorIcon(
                Icons::Filled::Settings(),
                Some("Elevated description".into()),
                modifier = Modifier.size(AssistChipDefaults::IconSize),
            );
        }),
    );
}

/// The samples' leading icon of a selected filter chip.
fn done_icon() -> Option<Rc<dyn Fn()>> {
    slot(|| {
        named_args!(VectorIcon(
            Icons::Filled::Done(),
            Some("Localized Description".into()),
            modifier = Modifier.size(FilterChipDefaults::IconSize),
        ));
    })
}

#[kompozable]
fn FilterChipSample() {
    let selected = remember(|| mutable_state_of(false));
    let s = selected.clone();
    FilterChip(
        selected.get(),
        move || s.set(!s.get()),
        || Text("Filter chip"),
        leading_icon = if selected.get() { done_icon() } else { None },
        modifier = Modifier.test_tag("FilterChipSample"),
    );
}

#[kompozable]
fn ElevatedFilterChipSample() {
    let selected = remember(|| mutable_state_of(false));
    let s = selected.clone();
    ElevatedFilterChip(
        selected.get(),
        move || s.set(!s.get()),
        || Text("Elevated filter chip"),
        leading_icon = if selected.get() { done_icon() } else { None },
        modifier = Modifier.test_tag("ElevatedFilterChipSample"),
    );
}

#[kompozable]
fn FilterChipWithLeadingIconSample() {
    let selected = remember(|| mutable_state_of(false));
    let s = selected.clone();
    FilterChip(
        selected.get(),
        move || s.set(!s.get()),
        || Text("Filter chip with leading icon"),
        leading_icon = if selected.get() {
            done_icon()
        } else {
            slot(|| {
                VectorIcon(
                    Icons::Filled::Home(),
                    Some("Home description".into()),
                    modifier = Modifier.size(FilterChipDefaults::IconSize),
                );
            })
        },
        modifier = Modifier.test_tag("FilterChipWithLeadingIconSample"),
    );
}

#[kompozable]
fn FilterChipWithTrailingIconSample() {
    let selected = remember(|| mutable_state_of(false));
    let s = selected.clone();
    FilterChip(
        selected.get(),
        move || s.set(!s.get()),
        || Text("Filter chip with trailing icon"),
        leading_icon = if selected.get() { done_icon() } else { None },
        trailing_icon = slot(|| {
            VectorIcon(
                Icons::Filled::ArrowDropDown(),
                Some("Drop down description".into()),
                modifier = Modifier.size(FilterChipDefaults::IconSize),
            );
        }),
        modifier = Modifier.test_tag("FilterChipWithTrailingIconSample"),
    );
}

#[kompozable]
fn FilterChipWithCustomSpacingSample() {
    let selected = remember(|| mutable_state_of(false));
    let s = selected.clone();
    FilterChip(
        selected.get(),
        move || s.set(!s.get()),
        || Text("Spaced filter chip"),
        leading_icon = if selected.get() { done_icon() } else { None },
        horizontal_arrangement = FilterChipDefaults::horizontal_arrangement_with_spacing(4.dp()),
        modifier = Modifier.test_tag("FilterChipWithCustomSpacingSample"),
    );
}

#[kompozable]
fn InputChipSample() {
    let selected = remember(|| mutable_state_of(false));
    let s = selected.clone();
    InputChip(
        selected.get(),
        move || s.set(!s.get()),
        || Text("Input Chip"),
        modifier = Modifier.test_tag("InputChipSample"),
    );
}

#[kompozable]
fn InputChipWithAvatarSample() {
    let selected = remember(|| mutable_state_of(false));
    let s = selected.clone();
    InputChip(
        selected.get(),
        move || s.set(!s.get()),
        || Text("Input Chip with avatar"),
        avatar = slot(|| {
            VectorIcon(
                Icons::Filled::Person(),
                Some("Avatar description".into()),
                modifier = Modifier.size(InputChipDefaults::AvatarSize),
            );
        }),
        modifier = Modifier.test_tag("InputChipWithAvatarSample"),
    );
}

#[kompozable]
fn SuggestionChipSample() {
    SuggestionChip(|| { /* Do something! */ }, label = || Text("Suggestion Chip"));
}

#[kompozable]
fn ElevatedSuggestionChipSample() {
    ElevatedSuggestionChip(|| { /* Do something! */ }, label = || Text("Elevated Suggestion Chip"));
}

/// The chips' samples' Show All chips' icon: Compose's `Icons.Filled.Tune` is of the extended icons (WP9's), the core
/// set's Settings in its place.
fn tune_icon() -> Option<Rc<dyn Fn()>> {
    slot(|| {
        named_args!(VectorIcon(
            Icons::Filled::Settings(),
            Some("Show all description".into()),
            modifier = Modifier.size(FilterChipDefaults::IconSize),
        ));
    })
}

#[kompozable]
fn ChipGroupSingleLineSample(expanded_seen: MutableState<bool>) {
    let expanded = expanded_seen;
    let list_size = 9;
    let chip_data: Vec<String> = (0..list_size).map(|index| format!("Chip {index}")).collect();
    Column(
        horizontal_alignment = Alignment::CenterHorizontally,
        content = || {
            Row(
                modifier = Modifier.horizontal_scroll(&remember_scroll_state()).test_tag("ChipGroupSingleLineSample"),
                content = || {
                    let e = expanded.clone();
                    AssistChip(move || e.set(!e.get()), || Text("Show All"), leading_icon = tune_icon());
                    // When chip lists exceed the available horizontal screen space, one option is to provide a chip button
                    // that opens a menu displaying all chip options. This ensures all options are accessible while
                    // maintaining the position of the content below the chip list.
                    let e = expanded.clone();
                    DropdownMenu(
                        expanded = expanded.get(),
                        on_dismiss_request = move || e.set(false),
                        content = || {
                            for data in chip_data.clone() {
                                DropdownMenuItem(
                                    text = move || Text(data.clone()),
                                    on_click = || {},
                                    // (Compose's AutoMirrored ArrowRight is of the extended icons, WP9's: the core
                                    // set's KeyboardArrowRight in its place)
                                    trailing_icon =
                                        slot(|| VectorIcon(Icons::AutoMirrored::Filled::KeyboardArrowRight(), None)),
                                );
                            }
                        },
                    );
                    for data in chip_data.clone() {
                        AssistChip(
                            || { /* do something*/ },
                            move || Text(data.clone()),
                            modifier = named_args!(Modifier.padding_hv(horizontal = 4.dp())),
                            trailing_icon = slot(|| VectorIcon(Icons::Filled::ArrowDropDown(), None)),
                        );
                    }
                },
            );
        },
    );
}

#[kompozable]
fn ChipGroupReflowSample() {
    let selected = remember(|| mutable_state_of(false));
    let color_names = ["Blue", "Yellow", "Red", "Orange", "Black", "Green", "White", "Magenta", "Gray", "Transparent"];
    Column(|| {
        let scroll = remember_scroll_state();
        let scrolled =
            if selected.get() { Modifier.vertical_scroll(&scroll) } else { Modifier.horizontal_scroll(&scroll) };
        FlowRow(
            modifier = Modifier
                .fill_max_width()
                .wrap_content_height_with(Alignment::Top, false)
                .then(scrolled)
                .test_tag("ChipGroupReflowSample"),
            horizontal_arrangement = Arrangement::Start,
            max_lines = if !selected.get() { 1 } else { i32::MAX },
            content = || {
                // When chip lists exceed the available horizontal screen space, one option is to provide a leading
                // chip that expands the list into a vertical scrolling list.
                let s = selected.clone();
                FilterChip(
                    selected.get(),
                    move || s.set(!s.get()),
                    || Text("Show All chips"),
                    modifier = named_args!(Modifier.padding_hv(horizontal = 4.dp())).align(Alignment::CenterVertically),
                    leading_icon = tune_icon(),
                );
                Box(
                    modifier = Modifier.height(FilterChipDefaults::Height).align(Alignment::CenterVertically),
                    content = || VerticalDivider(),
                );
                for (index, element) in color_names.iter().copied().enumerate() {
                    AssistChip(
                        || { /* do something*/ },
                        move || Text(format!("{element} {index}")),
                        modifier =
                            named_args!(Modifier.padding_hv(horizontal = 4.dp())).align(Alignment::CenterVertically),
                    );
                }
            },
        );
    });
}

fn chip_samples() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Column(|| {
                AssistChipSample();
                ElevatedAssistChipSample();
                FilterChipSample();
                ElevatedFilterChipSample();
                FilterChipWithLeadingIconSample();
                FilterChipWithTrailingIconSample();
                FilterChipWithCustomSpacingSample();
                InputChipSample();
                InputChipWithAvatarSample();
                SuggestionChipSample();
                ElevatedSuggestionChipSample();
            });
        }),
        IntSize::new(400, 900),
    );
    // (an icon's description is merged into its chip's: the unmerged tree's node)
    let icon = |description: &str| {
        rule.on_node_with(has_content_description(description, false, false), true).get_unclipped_bounds_in_root()
    };
    assert!(near(icon("Localized description").width(), 18.0, 0.01));
    assert!(near(icon("Elevated description").width(), 18.0, 0.01));
    rule.on_node_with_text("Suggestion Chip").assert_exists();
    rule.on_node_with_text("Elevated Suggestion Chip").assert_exists();
    // Filter chips: the check comes with the selection.
    for tag in [
        "FilterChipSample",
        "ElevatedFilterChipSample",
        "FilterChipWithTrailingIconSample",
        "FilterChipWithCustomSpacingSample",
    ] {
        rule.on_node_with_tag(tag).assert_is_not_selected();
    }
    rule.on_node_with_content_description("Localized Description").assert_does_not_exist();
    rule.on_node_with_tag("FilterChipSample").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_tag("FilterChipSample").assert_is_selected();
    rule.on_node_with_content_description("Localized Description").assert_exists();
    // The leading icon one: Home, then the check.
    rule.on_node_with_content_description("Home description").assert_exists();
    rule.on_node_with_tag("FilterChipWithLeadingIconSample").perform_click();
    rule.advance_time_by(1000);
    rule.on_node_with_content_description("Home description").assert_does_not_exist();
    rule.on_node_with_content_description("Drop down description").assert_exists();
    // The custom spacing one: the label 4 dp after the check.
    rule.on_node_with_tag("FilterChipWithCustomSpacingSample").perform_click();
    rule.advance_time_by(1000);
    let checks = named_args!(
        rule.on_all_nodes(has_content_description("Localized Description", false, false), use_unmerged_tree = true)
    )
    .fetch_semantics_nodes();
    assert_eq!(checks.len(), 3);
    let spaced = bounds(&rule, "FilterChipWithCustomSpacingSample");
    let check = checks.iter().map(|n| n.bounds_in_root()).find(|b| spaced.contains(b.center())).expect("its check");
    let label = rule.on_node_with(has_text("Spaced filter chip", false, false), true).get_unclipped_bounds_in_root();
    assert!(near(label.left, check.right + 4.0, 0.01), "{check:?} {label:?}");
    // Input chips: selected on a click; the avatar 24 dp.
    rule.on_node_with_tag("InputChipSample").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_tag("InputChipSample").assert_is_selected();
    assert!(near(icon("Avatar description").width(), 24.0, 0.01));
    rule.on_node_with_tag("InputChipWithAvatarSample").assert_is_not_selected();

    // The chip groups.
    let expanded = mutable_state_of(false);
    let e = expanded.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let e = e.clone();
        Column(move || {
            ChipGroupSingleLineSample(e.clone());
            ChipGroupReflowSample();
        });
    }));
    // One line, scrolled sideways: the chips past the screen's edge are there.
    let row = bounds(&rule, "ChipGroupSingleLineSample");
    let last = rule.on_node_with_text("Chip 8").get_unclipped_bounds_in_root();
    assert!(last.left > row.right && near(last.center().y, row.center().y, 0.6));
    assert_eq!(rule.on_all_nodes_with_text("Chip 0").fetch_semantics_nodes().len(), 1);
    rule.on_node_with_text("Show All").perform_click();
    rule.wait_for_idle();
    assert!(expanded.get());
    // "Show All" opens the menu of all the chips under the row, each with its arrow; Back (Escape) closes it.
    for i in 0..9 {
        let items = rule.on_all_nodes_with_text(&format!("Chip {i}")).fetch_semantics_nodes();
        assert_eq!(items.len(), 2, "Chip {i}");
    }
    let item = |i: usize| rule.on_all_nodes_with_text("Chip 0").fetch_semantics_nodes()[i].unclipped_bounds_in_root();
    let in_menu = if item(0).top > row.bottom { item(0) } else { item(1) };
    assert!(in_menu.top >= row.bottom && in_menu.left < row.right, "{in_menu:?} {row:?}");
    rule.host().key_down(Key::Escape, KeyModifiers::None);
    rule.host().key_up(Key::Escape, KeyModifiers::None);
    rule.wait_for_idle();
    assert!(!expanded.get());
    assert_eq!(rule.on_all_nodes_with_text("Chip 0").fetch_semantics_nodes().len(), 1);
    // The reflowing one: one line, all of them below each other once "Show All chips" is selected.
    let line_tops = |rule: &KompozTestRule| {
        let mut tops: Vec<i32> = ["Blue 0", "Gray 8", "Transparent 9"]
            .iter()
            .filter(|t| {
                rule.on_node_with_text(t).is_displayed()
                    || rule.on_all_nodes_with_text(t).fetch_semantics_nodes().len() == 1
            })
            .map(|t| rule.on_node_with_text(t).get_unclipped_bounds_in_root().top as i32)
            .collect();
        tops.dedup();
        tops.len()
    };
    let flow = bounds(&rule, "ChipGroupReflowSample");
    assert!(flow.height() < 60.0, "{flow:?}");
    rule.on_node_with_text("Show All chips").perform_click();
    rule.wait_for_idle();
    let flow = bounds(&rule, "ChipGroupReflowSample");
    assert!(flow.height() > 60.0 && line_tops(&rule) > 1, "{flow:?}");
}

#[kompozable]
fn NavigationBarItemWithBadge() {
    // Compose's sample puts each BadgedBox in a NavigationBarItem of a NavigationBar: they wait for WP4b's
    // navigation bar; the badged boxes in a Row here.
    Row(
        horizontal_arrangement = Arrangement::spaced_by(40.dp()),
        modifier = Modifier.padding(20.dp()),
        content = || {
            BadgedBox(
                || Badge(modifier = Modifier.semantics(|c| c.set_content_description("New notification"))),
                content = || VectorIcon(Icons::Filled::Star(), Some("Favorite".into())),
            );
            for badge_number in ["8", "999+"] {
                BadgedBox(
                    move || {
                        Badge(
                            content = slot(move || {
                                named_args!(Text(
                                    badge_number,
                                    modifier = Modifier
                                        .semantics(move |c| c
                                            .set_content_description(format!("{badge_number} new notifications"))),
                                ));
                            }),
                        );
                    },
                    content = || VectorIcon(Icons::Filled::Star(), Some("Favorite".into())),
                );
            }
        },
    );
}

fn badge_samples() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| NavigationBarItemWithBadge()));
    let dot = rule.on_node_with_content_description("New notification").get_unclipped_bounds_in_root();
    assert!(near(dot.width(), 6.0, 0.01) && near(dot.height(), 6.0, 0.01));
    rule.on_node_with_content_description("8 new notifications").assert_exists();
    let many = rule.on_node_with_content_description("999+ new notifications").get_unclipped_bounds_in_root();
    let eight = rule.on_node_with_content_description("8 new notifications").get_unclipped_bounds_in_root();
    assert!(many.width() > eight.width());
    assert_eq!(rule.on_all_nodes_with_content_description("Favorite").fetch_semantics_nodes().len(), 3);
    // The long count's badge: its start 12 dp in from the icon's end, its bottom 14 dp below the icon's top.
    let icons = rule.on_all_nodes_with_content_description("Favorite").fetch_semantics_nodes();
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
