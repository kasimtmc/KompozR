//! Unit tests of what SharedTransitionScope (stage 11 WP4) is built on and no public API reaches yet: the
//! SkipToLookaheadSize node (its `ScaleToBounds` too) and the debug helper's shared transition visualizations. One
//! test, its parts in turn, on the one thread made the UI thread.

use crate::lookahead_debug::LookaheadAnimationVisualDebugHelper;
use crate::skip_to_lookahead_size::{
    ScaleToBounds, SkipToLookaheadSizeElement, content_scale_modifier, default_skip_enabled,
};
use kompoz_coroutines::{CoroutineScope, Dispatcher};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_text::TextMeasurer;
use kompoz_ui::draw::DrawScope;
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// An approach layout measuring what it wraps half as wide as its lookahead size while `approaching`.
fn half_wide_while(approaching: MutableState<bool>) -> Modifier {
    named_args!(Modifier.approach_layout(
        is_measurement_approach_in_progress = move |_| approaching.get(),
        approach_measure = |s: &ApproachMeasureScope, m: &dyn Measurable, _| {
            let target = s.lookahead_size();
            let p = m.measure(Constraints::fixed(target.width / 2, target.height));
            s.layout(p.width(), p.height(), move |pl| pl.place(&p, 0, 0))
        },
    ))
}

/// What the content inside measured with, outside the lookahead pass.
fn recording(seen: Rc<RefCell<Vec<Constraints>>>) -> Modifier {
    Modifier.layout(move |s, m, c| {
        if !s.is_looking_ahead() {
            seen.borrow_mut().push(c);
        }
        let p = m.measure(c);
        let (w, h) = (p.width(), p.height());
        s.layout(w, h, move |pl| pl.place(&p, 0, 0))
    })
}

fn skip_to_lookahead_size_measures_at_the_lookahead_constraints() {
    // Compose's SkipToLookaheadSizeNode: its content measured with the lookahead constraints all along the approach
    // (not the half width its parent approaches with), itself its lookahead size within its constraints; disabled,
    // what it is given.
    let approaching = mutable_state_of(true);
    let enabled = mutable_state_of(true);
    let seen = Rc::new(RefCell::new(Vec::<Constraints>::new()));
    let (a, e, s) = (approaching.clone(), enabled.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (a, s) = (a.clone(), s.clone());
        let skip_enabled = if e.get() { default_skip_enabled() } else { Rc::new(|| false) };
        LookaheadScope(move |_| {
            let (a, s, skip_enabled) = (a.clone(), s.clone(), skip_enabled.clone());
            Box(
                modifier = half_wide_while(a.clone()),
                content = move || {
                    let s = s.clone();
                    let skip = Modifier.then_element(SkipToLookaheadSizeElement {
                        scale_to_bounds: None,
                        is_enabled: skip_enabled.clone(),
                    });
                    Box(modifier =
                        Modifier.test_tag("atlayan").then(skip).then(recording(s)).size_wh(100.dp(), 40.dp()));
                },
            );
        });
    }));
    // Half as wide in the approach: the node takes 50 of its lookahead 100; its content measured as in lookahead.
    assert_eq!(bounds(&rule, "atlayan").width(), 50.0);
    let constraints = *seen.borrow().last().unwrap();
    assert!(constraints.max_width > 50 && !constraints.has_fixed_width(), "{constraints:?}");
    // Disabled: measured as given (the box around it loosens its fixed 50 by 40).
    enabled.set(false);
    rule.wait_for_idle();
    assert_eq!(*seen.borrow().last().unwrap(), Constraints::new(0, 50, 0, 40));
    approaching.set(false);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "atlayan").width(), 100.0);
}

fn scale_to_bounds_scales_the_content_into_the_size_on_the_way() {
    // Compose's createContentScaleModifier: the content at its lookahead size, scaled into the size along the way by
    // the content scale (FillBounds: half as wide), from its top left, placed by the alignment; cropping clips.
    let approaching = mutable_state_of(true);
    let a = approaching.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let a = a.clone();
        LookaheadScope(move |_| {
            Box(
                modifier = half_wide_while(a.clone()),
                content = || {
                    let scaled = content_scale_modifier(
                        Modifier,
                        ScaleToBounds { content_scale: ContentScale::FillBounds, alignment: Alignment::Center },
                        default_skip_enabled(),
                    );
                    Box(modifier = scaled.size_wh(100.dp(), 40.dp()).background(Color::Red));
                    let cropped = content_scale_modifier(
                        Modifier,
                        ScaleToBounds { content_scale: ContentScale::Crop, alignment: Alignment::Center },
                        default_skip_enabled(),
                    );
                    Box(modifier = cropped.size_wh(100.dp(), 40.dp()));
                },
            );
        });
    }));
    // (a layer scaling from its top left, drawn through its matrix)
    let half = |m: &Matrix| (m.values[0] - 0.5).abs() < 1e-4 && (m.values[5] - 1.0).abs() < 1e-4;
    let canvas = rule.draw();
    assert!(canvas.transforms.iter().any(half), "{:?}", canvas.transforms);
    // (Crop: the bigger of the two scales, 1 — its content cut to the 50 px it has by a clipping layer)
    assert!(canvas.clips.iter().any(|c| c.width() == 50.0), "{:?}", canvas.clips);
    approaching.set(false);
    rule.wait_for_idle();
    let canvas = rule.draw();
    assert!(!canvas.transforms.iter().any(half), "{:?}", canvas.transforms);
}

fn the_debug_helper_draws_a_shared_transition() {
    // Compose's LookaheadAnimationVisualDebugHelper, its shared transition parts: the overlay, the perimeter (its
    // progress animations started and stopped), an element with several matches, one without a match.
    let dispatcher = Dispatcher::new();
    let scope = CoroutineScope::new(&dispatcher);
    let helper = LookaheadAnimationVisualDebugHelper::new();
    helper.on_attach(&scope);
    helper.on_attach(&scope);
    helper.update_drawing_coordinates(Offset::new(10.0, 20.0), IntSize::new(200, 100));
    let measurer = TextMeasurer::new(kompoz_ui::platform::fake_resolver(), Density::default(), LayoutDirection::Ltr, 0);
    let mut canvas = RecordingCanvas::new();
    {
        let mut draw = DrawScope::new(&mut canvas, Size::new(50.0, 30.0), LayoutDirection::Ltr, Density::default());
        helper.draw_overlay(&mut draw, Color::from_argb(0x8034A853));
        helper.draw_global_visualizations(&mut draw);
        helper.draw_multiple_matches_element(&mut draw, Color::Red, true, "anahtar", 3, &measurer, 2.5);
        helper.draw_unmatched_element(&mut draw, Color::Blue, true, "yalnız", &measurer, 2.5);
    }
    // The overlay fills the area.
    assert!(
        canvas
            .rects
            .iter()
            .zip(&canvas.rect_colors)
            .any(|(r, c)| *c == Color::from_argb(0x8034A853) && r.width() == 50.0)
    );
    // The perimeter: the scope's rect stroked with the gradient going round, from the scope's origin (the canvas
    // moved there: recorded as its matrix).
    assert!(canvas.paths.iter().any(|r| r.width() == 200.0 && r.height() == 100.0), "{:?}", canvas.paths);
    assert!(canvas.transforms.iter().any(|m| m.values.contains(&-10.0) && m.values.contains(&-20.0)));
    // Several matches: outlined in their color, the count written; none: hatched, written too.
    assert!(canvas.stroke_colors.contains(&Color::Red) && canvas.stroke_colors.contains(&Color::Blue));
    assert!(canvas.texts.iter().any(|t| t.contains("anahtar: 3")), "{:?}", canvas.texts);
    assert!(canvas.texts.iter().any(|t| t.contains("yalnız: 0")), "{:?}", canvas.texts);
    // (the hatching: lines up and right across its 30 px height, 10 px apart, stroked as paths)
    assert!(canvas.paths.iter().filter(|r| r.height() == 30.0 && r.width() == 30.0).count() >= 5, "{:?}", canvas.paths);
    // Each key keeps its color; the next one is the next of the colors.
    let first = helper.choose_color("bir");
    assert_eq!(helper.choose_color("bir"), first);
    assert_ne!(helper.choose_color("iki"), first);
    helper.on_detach(&scope);
    helper.on_detach(&scope);
    scope.cancel();
}

#[test]
fn what_shared_transitions_are_built_on() {
    set_ui_thread();
    skip_to_lookahead_size_measures_at_the_lookahead_constraints();
    scale_to_bounds_scales_the_content_into_the_size_on_the_way();
    the_debug_helper_draws_a_shared_transition();
}
