//! The ripple node, as Compose's material-ripple: ripples spreading from presses and fading once let go, unbounded
//! ones from the middle, the state layer for hovers and focuses, a source's ripples on another node, inside the
//! shapes they are drawn in (the C++ Kompoz's ripple tests on Compose's ripple: given through `LocalIndication`, as
//! the default stays Compose's DefaultDebugIndication; material3's `ripple()`, with its alphas).

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_ripple::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::input::{Key, KeyModifiers, PointerType};
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::*;
use std::rc::Rc;
use std::sync::Arc;

/// material3's `ripple(bounded, radius, color)`: the ripple node in `color` (the content color, black, when
/// unspecified) at material3's alphas (RippleDefaults.RippleAlpha).
fn material_ripple(bounded: bool, radius: Dp, color: Color) -> Rc<dyn Indication> {
    named_args!(kompoz_material3::ripple(bounded = bounded, radius = radius, color = color))
}

/// An indication that isn't a node factory: nothing shown (the C++ `indication = false` given as a local).
#[derive(PartialEq)]
struct NoIndication;
impl Indication for NoIndication {}

/// How near a drawn alpha is to the one asked for (a color's alpha is kept in 8 bits).
const ALPHA: f32 = 0.5 / 255.0;

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// The circles drawn (round rects as wide as tall, rounded all the way) and their alphas.
fn circles(canvas: &RecordingCanvas) -> Vec<(Rect, f32)> {
    canvas
        .rects
        .iter()
        .zip(&canvas.rect_colors)
        .zip(&canvas.rect_radii)
        .filter(|((r, _), radius)| {
            near(r.width(), r.height(), 0.01) && r.width() > 0.0 && near(**radius, r.width() / 2.0, 0.01)
        })
        .map(|((r, c), _)| (*r, c.alpha()))
        .collect()
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_bounds_in_root()
}

fn down(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
}

fn up(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_up(p, -1, 0);
}

fn mouse_to(rule: &KompozTestRule, p: Offset) {
    rule.on_root().perform_mouse_input(|m| m.move_to(p));
}

// ---- C++ 23781: ripple_spreads_from_the_press_and_fades_once_let_go -------------------------------------------

fn ripple_spreads_from_the_press_and_fades_once_let_go() {
    // C++'s ripple_spreads_from_the_press_and_fades_once_let_go with LocalIndication providing the ripple. Compose's
    // numbers where they differ: a bounded ripple spreads 10 dp past the corners (C++: to them); a keyboard focus
    // shows the state layer's circle too (C++ drew a focus ring); C++'s flat StateLayer kind has no Compose
    // counterpart (left out).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        KompozitionLocalProvider(
            &[LocalIndication.provides(material_ripple(true, Dp::Unspecified, Color::Unspecified))],
            || {
                Column(|| {
                    Box(Modifier.size_wh(200.dp(), 100.dp()).clickable(|| {}).test_tag("kutu"));
                    Box(Modifier
                        .size(100.dp())
                        .clickable_indication(
                            None,
                            Some(material_ripple(false, 30.dp(), Color::Unspecified)),
                            on_click = || {},
                        )
                        .test_tag("sınırsız"));
                    // (C++'s LocalIndication provides false: Compose's clickable throws for a LocalIndication that isn't
                    // a node factory, so none given to it)
                    Box(Modifier.size(100.dp()).clickable_indication(None, None, on_click = || {}).test_tag("yok"));
                });
            },
        );
    }));
    let b = bounds(&rule, "kutu");
    down(&rule, Offset::new(b.left + 20.0, b.top + 20.0));
    rule.advance_frame();
    rule.advance_frame();
    let early = circles(&rule.draw());
    assert_eq!(early.len(), 1, "{early:?}");
    let c = early[0].0.center(); // small, near the press
    assert!(
        early[0].0.width() / 2.0 < 80.0
            && early[0].0.width() / 2.0 >= 60.0
            && c.x < b.left + 60.0
            && c.y < b.top + 45.0
    );
    rule.advance_time_by(300); // spread: past the farthest corner, around the middle; held, nothing moves
    let canvas = rule.draw();
    let spread = circles(&canvas);
    assert!(spread.len() == 1 && !rule.host().has_pending_work());
    let (r, held) = spread[0];
    assert!(near(r.width() / 2.0, (200f32.hypot(100.0)) / 2.0 + 10.0, 0.5) && near(r.center().x, b.left + 100.0, 0.5));
    assert!(near(r.center().y, b.top + 50.0, 0.5));
    assert!(canvas.clips.iter().any(|k| near(k.left, b.left, 0.5) && near(k.width(), b.width(), 0.5))); // clipped to it
    up(&rule, Offset::new(b.left + 20.0, b.top + 20.0));
    rule.advance_time_by(80);
    let fading = circles(&rule.draw());
    assert!(fading.len() == 1 && fading[0].1 < held * 0.8 && fading[0].1 > 0.0, "{fading:?}");
    rule.wait_for_idle();
    assert!(circles(&rule.draw()).is_empty() && !rule.host().has_pending_work());
    // The keyboard: Enter's ripple comes from the middle (the focus's state layer there too).
    rule.host().key_down(Key::Tab, KeyModifiers::None);
    rule.host().key_down(Key::Enter, KeyModifiers::None);
    rule.advance_frame();
    rule.advance_frame();
    let keyed = circles(&rule.draw());
    assert!(!keyed.is_empty() && keyed.iter().all(|(r, _)| near(r.center().x, b.left + 100.0, 2.0)), "{keyed:?}");
    rule.host().key_up(Key::Enter, KeyModifiers::None);
    rule.wait_for_idle();
    let focused = circles(&rule.draw());
    assert!(focused.len() == 1 && near(focused[0].1, 0.1, ALPHA), "{focused:?}"); // the focus's layer stays
    down(&rule, Offset::new(b.left + 20.0, b.top + 90.0)); // ends the keyboard focus
    up(&rule, Offset::new(b.left + 20.0, b.top + 90.0));
    rule.wait_for_idle();
    // Unbounded: from the middle, its own radius, not clipped.
    let u = bounds(&rule, "sınırsız");
    down(&rule, Offset::new(u.left + 10.0, u.top + 10.0));
    rule.advance_frame();
    rule.advance_frame();
    let start = circles(&rule.draw());
    let unbounded: Vec<_> = start.iter().filter(|(r, _)| r.center().y > b.bottom).collect();
    assert!(unbounded.len() == 1 && near(unbounded[0].0.center().x, u.left + 50.0, 1.0), "{start:?}");
    rule.advance_time_by(300);
    let canvas = rule.draw();
    let wide: Vec<_> = circles(&canvas).into_iter().filter(|(r, _)| r.center().y > b.bottom).collect();
    assert!(wide.len() == 1 && near(wide[0].0.width() / 2.0, 30.0, 0.5), "{wide:?}");
    assert!(near(wide[0].0.center().x, u.left + 50.0, 0.5) && near(wide[0].0.center().y, u.top + 50.0, 0.5));
    assert!(!canvas.clips.iter().any(|k| near(k.top, u.top, 0.5) && near(k.height(), u.height(), 0.5)));
    up(&rule, Offset::new(u.left + 10.0, u.top + 10.0));
    rule.wait_for_idle();
    // No indication: nothing more drawn.
    let n = bounds(&rule, "yok");
    let before = rule.draw();
    down(&rule, n.center());
    rule.advance_time_by(100);
    let after = rule.draw();
    assert_eq!(after.rects.len(), before.rects.len());
    up(&rule, n.center());
    rule.wait_for_idle();
}

// ---- C++ 31574: unbounded_indications_hover_in_a_circle_of_their_radius ----------------------------------------

fn unbounded_indications_hover_in_a_circle_of_their_radius() {
    // C++'s unbounded_indications_hover_in_a_circle_of_their_radius. Compose's: an unbounded one with no radius
    // hovers in a circle reaching its corners (C++: one over its bounds).
    let source = MutableInteractionSource::new();
    let s = source.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Column(move || {
            // Shown by `indication` on another node, as wide as its radius.
            Box(Modifier
                .size(24.dp())
                .hoverable(s.clone())
                .indication(&s, Some(material_ripple(false, 20.dp(), Color::Unspecified)))
                .test_tag("simge"));
            // Unbounded with no radius: to its corners.
            Box(Modifier
                .size(30.dp())
                .clickable_indication(
                    None,
                    Some(material_ripple(false, Dp::Unspecified, Color::Unspecified)),
                    on_click = || {},
                )
                .test_tag("geniş"));
        });
    }));
    let hovers = |rule: &KompozTestRule| -> Vec<(Rect, f32)> {
        circles(&rule.draw()).into_iter().filter(|(_, a)| near(*a, 0.08, ALPHA)).collect()
    };
    let (icon, wide) = (bounds(&rule, "simge"), bounds(&rule, "geniş"));
    mouse_to(&rule, icon.center());
    rule.wait_for_idle();
    let over = hovers(&rule);
    assert!(over.len() == 1 && near(over[0].0.width(), 40.0, 0.01), "{over:?}");
    assert!(near(over[0].0.center().x, icon.center().x, 0.01) && near(over[0].0.center().y, icon.center().y, 0.01));
    mouse_to(&rule, wide.center());
    rule.wait_for_idle();
    let over = hovers(&rule);
    assert!(over.len() == 1 && near(over[0].0.width(), 30f32.hypot(30.0), 0.01), "{over:?}");
    assert!(near(over[0].0.center().x, wide.center().x, 0.01) && near(over[0].0.center().y, wide.center().y, 0.01));
}

// ---- C++ 27018: indication_shows_a_sources_ripple_on_another_node ----------------------------------------------

fn indication_shows_a_sources_ripple_on_another_node() {
    // C++'s indication_shows_a_sources_ripple_on_another_node (its debug-indication half is kompoz-foundation's
    // clickable.rs): a press on the area shows the ripple on the icon, in its color. Compose's: the hover layer is
    // the bounded ripple's circle, to the corners and 10 dp further (clipped to the icon; C++: the icon's width).
    let source = MutableInteractionSource::new();
    let s = source.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Box(
            modifier = Modifier
                .size_wh(200.dp(), 100.dp())
                .clickable_indication(Some(s.clone()), None, on_click = || {})
                .test_tag("alan"),
            content_alignment = Alignment::Center,
            content = move || {
                let red = material_ripple(true, Dp::Unspecified, Color::new(1.0, 0.0, 0.0, 1.0));
                Box(Modifier.size(48.dp()).clip(CircleShape).indication(&s, Some(red)).test_tag("simge"));
            },
        );
    }));
    let (area, icon) = (bounds(&rule, "alan"), bounds(&rule, "simge"));
    let mut last: Vec<(Rect, f32)> = Vec::new();
    let mut last_color = Color::Transparent;
    let mut frames = |rule: &KompozTestRule, n: usize| -> Vec<(Rect, f32)> {
        for _ in 0..n {
            rule.advance_frame();
            let canvas = rule.draw();
            last = circles(&canvas);
            if let Some(i) = canvas.rect_colors.iter().position(|c| c.alpha() > 0.0 && c.alpha() < 0.2) {
                last_color = canvas.rect_colors[i];
            }
        }
        last.clone()
    };
    down(&rule, Offset::new(area.left + 10.0, area.top + 10.0)); // on the area, away from the icon
    let pressed = frames(&rule, 3);
    assert_eq!(pressed.len(), 1, "{pressed:?}");
    assert!(near(pressed[0].0.center().x, icon.left + 10.0, 30.0) && pressed[0].1 > 0.0); // on the icon
    let held = frames(&rule, 20); // spread and held: nothing moves
    assert!(held.len() == 1 && !rule.host().has_pending_work());
    let held = held[0].1;
    up(&rule, Offset::new(area.left + 10.0, area.top + 10.0));
    frames(&rule, 1);
    assert!(rule.host().has_pending_work()); // fading: it asks for frames
    let fading = frames(&rule, 4);
    assert!(fading.len() == 1 && fading[0].1 < held);
    assert!(frames(&rule, 20).is_empty() && !rule.host().has_pending_work());
    mouse_to(&rule, area.center()); // the mouse over the area: the icon shows the hover layer
    let hover = frames(&rule, 2);
    assert!(hover.len() == 1 && near(hover[0].1, 0.08, ALPHA), "{hover:?}");
    assert!(
        near(hover[0].0.width(), 48f32.hypot(48.0) + 20.0, 0.01) && near(hover[0].0.center().x, icon.center().x, 0.01)
    );
    assert!(near(last_color.red(), 1.0, 0.001) && last_color.green() == 0.0 && last_color.blue() == 0.0);
}

// ---- C++ 18455 and 20364: their ripple halves ------------------------------------------------------------------

fn ripples_keep_to_the_clip_they_are_drawn_in() {
    // C++'s shapes_with_corners_of_their_own_reach_the_canvas_mirrored_in_rtl and
    // outline_shapes_fill_stroke_clip_shadow_and_ripple, their ripple halves (the canvas halves are
    // kompoz-foundation's draw.rs): a ripple under a shape's clip is drawn inside it (and its own bounds' clip); a
    // ripple shown by `indication` under a graphics layer's clip, right to left, is inside the layer's. As Compose's,
    // a ripple isn't clipped to a background's shape (C++ clipped it to the outline it was drawn on): only to its
    // bounds.
    let tab = RoundedCornerShape::new(16.0, 8.0, 0.0, 4.0);
    let triangle = GenericShape(|p, s, _| {
        p.move_to(s.width / 2.0, 0.0);
        p.line_to(s.width, s.height);
        p.line_to(0.0, s.height);
    });
    let source = MutableInteractionSource::new();
    let (t, tri, s) = (tab.clone(), triangle.clone(), source.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (t, tri, s) = (t.clone(), tri.clone(), s.clone());
        KompozitionLocalProvider(
            &[LocalIndication.provides(material_ripple(true, Dp::Unspecified, Color::Unspecified))],
            move || {
                let (t, tri, s) = (t.clone(), tri.clone(), s.clone());
                KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], move || {
                    let (t, tri, s) = (t.clone(), tri.clone(), s.clone());
                    Column(move || {
                        Box(Modifier.size_wh(100.dp(), 40.dp()).clip(t.clone()).clickable(|| {}).test_tag("sekme"));
                        Box(Modifier
                            .size(70.dp())
                            .background(Color::new(1.0, 1.0, 0.0, 1.0), shape = Arc::new(tri.clone()))
                            .clickable(|| {})
                            .test_tag("üçgen"));
                        Box(Modifier
                            .size_wh(100.dp(), 40.dp())
                            .graphics_layer_block({
                                let t = t.clone();
                                move |l| {
                                    l.shape = Arc::new(t.clone());
                                    l.clip = true;
                                }
                            })
                            .clickable_indication(
                                Some(s.clone()),
                                Some(Rc::new(NoIndication) as Rc<dyn Indication>),
                                on_click = || {},
                            )
                            .indication(&s, Some(material_ripple(true, Dp::Unspecified, Color::Unspecified)))
                            .test_tag("katman"));
                    });
                });
            },
        );
    }));
    let unpressed = rule.draw();
    let press = |tag: &str| {
        let c = bounds(&rule, tag).center();
        down(&rule, c);
        for _ in 0..3 {
            rule.advance_frame();
        }
        let canvas = rule.draw();
        up(&rule, c);
        rule.wait_for_idle();
        canvas
    };
    // Under the shape's clip (its corners mirrored), with its bounds' clip too.
    let sekme = bounds(&rule, "sekme");
    let pressed = press("sekme");
    assert_eq!(circles(&pressed).len(), 1);
    assert_eq!(pressed.clip_shapes.len(), unpressed.clip_shapes.len());
    assert!(
        pressed.clip_shapes.iter().any(|k| k.radii.top_left == 8.0 && k.radii.top_right == 16.0),
        "{:?}",
        pressed.clip_shapes
    );
    assert!(pressed.clips.iter().any(|k| near(k.width(), sekme.width(), 0.5) && near(k.top, sekme.top, 0.5)));
    // Over a background's outline: clipped to its bounds only.
    let tri_bounds = bounds(&rule, "üçgen");
    let pressed = press("üçgen");
    assert_eq!(circles(&pressed).len(), 1);
    assert!(pressed.clips.iter().any(|k| near(k.width(), 70.0, 0.5) && near(k.top, tri_bounds.top, 0.5)));
    // In a graphics layer's clip, shown by `indication`.
    let pressed = press("katman");
    assert_eq!(circles(&pressed).len(), 1);
    assert!(pressed.clip_shapes.iter().filter(|k| k.radii.top_left == 8.0 && k.radii.top_right == 16.0).count() == 2);
}

fn ripple_alpha_as_compose() {
    let alpha = RippleAlpha(0.16, 0.1, 0.08, 0.1);
    assert_eq!(alpha, RippleAlpha::new(0.16, 0.1, 0.08, 0.1));
    assert_eq!(
        alpha.to_string(),
        "RippleAlpha(draggedAlpha=0.16, focusedAlpha=0.1, hoveredAlpha=0.08, pressedAlpha=0.1)"
    );
    assert_eq!(
        RippleAlpha(1.0, 0.0, 0.0, 0.0).to_string(),
        "RippleAlpha(draggedAlpha=1.0, focusedAlpha=0.0, hoveredAlpha=0.0, pressedAlpha=0.0)"
    );
}

tests![
    ripple_spreads_from_the_press_and_fades_once_let_go,
    unbounded_indications_hover_in_a_circle_of_their_radius,
    indication_shows_a_sources_ripple_on_another_node,
    ripples_keep_to_the_clip_they_are_drawn_in,
    ripple_alpha_as_compose,
];
