//! basicMarquee, as Compose's: content wider than its room scrolled across it with a copy following after a gap,
//! waiting between passes, a number of times or for ever, either way, at once or while focused (the C++ Kompoz's
//! marquee tests ported onto Compose's animation: an Animatable on the frame clock, whole-px spacing).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_coroutines::CoroutineScope;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::display_list::DisplayList;
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::input::PointerType;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::*;
use std::cell::Cell;
use std::rc::Rc;

const RED: Color = Color::new(1.0, 0.0, 0.0, 1.0);
const GREEN: Color = Color::new(0.0, 1.0, 0.0, 1.0);
const BLUE: Color = Color::new(0.0, 0.0, 1.0, 1.0);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// The rects drawn in `color`, left to right.
fn rects_of(canvas: &RecordingCanvas, color: Color) -> Vec<Rect> {
    let mut found: Vec<Rect> =
        canvas.rects.iter().zip(&canvas.rect_colors).filter(|(_, c)| **c == color).map(|(r, _)| *r).collect();
    found.sort_by(|a, b| a.left.total_cmp(&b.left));
    found
}

fn red_rects(rule: &KompozTestRule) -> Vec<Rect> {
    rects_of(&rule.draw(), RED)
}

fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(p, -1, 0);
    rule.advance_frame();
}

// ---- C++ 27071: basic_marquee_scrolls_what_doesnt_fit_and_its_copy_follows --------------------------------------

fn basic_marquee_scrolls_what_doesnt_fit_and_its_copy_follows() {
    // C++'s basic_marquee_scrolls_what_doesnt_fit_and_its_copy_follows on Compose's marquee: the defaults (3 passes,
    // 1200 ms waits, a third of the room behind, 30 dp/s). Compose's spacing is whole px (33: C++ had 33.33), and
    // its animation runs by frames through the waits (C++ slept on timers then). The tag goes before the marquee
    // to read its room: after it, a semantics node is on the content, as wide as it is (Compose's too).
    let clicks = Rc::new(Cell::new(0));
    let shown = mutable_state_of(false); // (composed later, at a known time)
    let (c, s) = (clicks.clone(), shown.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (c, s) = (c.clone(), s.clone());
        Column(
            modifier = Modifier.padding_hv(10.dp(), 0.dp()),
            content = move || {
                if s.get() {
                    let c = c.clone();
                    Box(
                        modifier = Modifier.width(100.dp()).test_tag("kayan").basic_marquee(),
                        content = move || {
                            let c = c.clone();
                            Box(Modifier
                                .size_wh(300.dp(), 20.dp())
                                .background(RED)
                                .clickable(move || c.set(c.get() + 1))
                                .test_tag("içerik"));
                        },
                    );
                }
                // (drawn after it, below: not in its clip)
                Box(Modifier.padding_hv(0.dp(), 20.dp()).size_wh(50.dp(), 20.dp()).background(GREEN));
            },
        );
    }));
    rule.main_clock().set_auto_advance(false);
    shown.set(true);
    rule.advance_frame();
    let marquee = rule.on_node_with_tag("kayan").get_unclipped_bounds_in_root();
    assert!(marquee.left == 10.0 && marquee.width() == 100.0 && marquee.height() == 20.0); // its room, the content wider
    assert_eq!(rule.on_node_with_tag("içerik").get_unclipped_bounds_in_root().width(), 300.0);
    let canvas = rule.draw();
    let red = rects_of(&canvas, RED);
    assert!(red.len() == 1 && red[0].left == 10.0, "{red:?}"); // waits first (1200 ms)
    assert!(canvas.clips.iter().any(|c| c.left == 10.0 && c.width() == 100.0), "{:?}", canvas.clips); // its room
    rule.advance_time_by(1100);
    assert_eq!(red_rects(&rule)[0].left, 10.0);
    rule.advance_time_by(1100); // about a second in, at 30 dp/s
    let red = red_rects(&rule);
    assert!(red.len() == 1 && near(red[0].left, 10.0 - 30.0, 3.0), "{red:?}");
    tap(&rule, Offset::new(20.0, 10.0)); // taps land where it is laid out
    assert_eq!(clicks.get(), 1);
    // The copy, a third of the room (33 px) behind, comes in once the end has crossed into view.
    rule.advance_time_by(7300);
    let red = red_rects(&rule);
    assert_eq!(red.len(), 2, "{red:?}");
    assert!(near(red[1].left - red[0].left, 333.0, 0.01) && red[1].left < 110.0 && red[0].right > 10.0);
    rule.advance_time_by(2200); // past 300: the content has left, only the copy is drawn
    let red = red_rects(&rule);
    assert!(red.len() == 1 && near(red[0].left, 10.0 - 315.0 + 333.0, 3.0), "{red:?}");
    // The pass ends where the copy began (the start again), then the wait before the next.
    rule.advance_time_by(1300);
    let red = red_rects(&rule);
    assert!(red.len() == 1 && red[0].left == 10.0, "{red:?}");
    // Recorded and replayed: what comes after it isn't clipped with it.
    let mut list = DisplayList::new();
    list.begin(Size::new(400.0, 800.0));
    rule.host().draw(&mut list);
    list.end();
    let mut replayed = RecordingCanvas::new();
    list.replay(&mut replayed, None);
    assert_eq!(replayed.rect_colors.iter().filter(|c| **c == GREEN).count(), 1);
    // Three passes, then it stays at the start, nothing more to do.
    rule.advance_time_by(2 * (1200 + 11100) + 500);
    assert_eq!(red_rects(&rule)[0].left, 10.0);
    assert!(!rule.host().has_pending_work() && rule.host().next_deadline().is_none());
}

// ---- C++ 27126: a_marquee_out_of_view_leaves_only_its_timers ----------------------------------------------------

fn a_marquee_out_of_view_goes_on_as_composes_does() {
    // C++'s a_marquee_out_of_view_leaves_only_its_timers, adapted: the C++ marquee moved as it was drawn (out of
    // view, only its timers were left); Compose's is an Animatable on the frame clock, so out of view it goes on
    // (frames asked for) and shows where it got to when scrolled back.
    let scroll = ScrollState::new(0);
    let on = mutable_state_of(false);
    let (sc, o) = (scroll.clone(), on.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (sc, o) = (sc.clone(), o.clone());
            Column(
                modifier = Modifier.height(200.dp()).vertical_scroll(&sc),
                content = move || {
                    Box(Modifier.size_wh(100.dp(), 300.dp()));
                    let iterations = if o.get() { i32::MAX } else { 0 };
                    Box(
                        modifier =
                            Modifier.width(100.dp()).basic_marquee(iterations = iterations, initial_delay_millis = 0),
                        content = || Box(Modifier.size_wh(200.dp(), 20.dp()).background(RED)),
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    on.set(true);
    rule.advance_time_by(100); // a pass under way, out of view
    assert!(rule.host().has_pending_work() && red_rects(&rule).iter().all(|r| r.top >= 200.0)); // (below the view)
    rule.advance_time_by(1000); // (still going: about 33 px in)
    let s = scroll.clone();
    CoroutineScope::new(rule.host().dispatcher()).launch(async move {
        s.scroll_to(150).await;
    });
    rule.advance_frame(); // (not waiting for idle: it scrolls for ever)
    let red = red_rects(&rule);
    assert_eq!(red.len(), 1);
    assert!(near(red[0].left, -33.0, 3.0), "{red:?}");
}

// ---- C++ 27156: basic_marquee_leaves_alone_what_fits_and_starts_again_when_it_grows -----------------------------

fn basic_marquee_leaves_alone_what_fits_and_starts_again_when_it_grows() {
    // C++'s basic_marquee_leaves_alone_what_fits_and_starts_again_when_it_grows (Compose's numbers are C++'s here).
    let width = mutable_state_of(80);
    let rtl = mutable_state_of(false);
    let (w, r) = (width.clone(), rtl.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let w = w.clone();
        let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], move || {
            let w = w.clone();
            Box(
                modifier = Modifier.width(100.dp()).basic_marquee(
                    iterations = i32::MAX,
                    spacing = MarqueeSpacing(20.dp()),
                    velocity = 50.dp(),
                ),
                content = move || Box(Modifier.size_wh(w.get().dp(), 20.dp()).background(RED)),
            );
        });
    }));
    rule.advance_time_by(5000);
    let red = red_rects(&rule);
    assert!(red.len() == 1 && red[0].left == 0.0);
    assert!(!rule.host().has_pending_work() && rule.host().next_deadline().is_none());
    rtl.set(true);
    rule.wait_for_idle();
    assert_eq!(red_rects(&rule)[0].left, 20.0); // at the start: the right
    width.set(200);
    rule.advance_frame();
    rule.advance_time_by(1200 + 1000); // right to left: anchored at the right, moving right
    let red = red_rects(&rule);
    assert!(red.len() == 1 && near(red[0].left, -100.0 + 50.0, 3.0), "{red:?}");
    rule.advance_time_by(2000); // 150 in: the copy (20 behind) comes in from the left
    let red = red_rects(&rule);
    assert_eq!(red.len(), 2, "{red:?}");
    assert!(near(red[1].left - red[0].left, 220.0, 0.01) && red[1].left > 0.0 && red[0].right > 0.0);
    rule.advance_time_by(2000); // past 220: a pass is done, the wait, and it goes on for ever
    assert_eq!(red_rects(&rule)[0].left, -100.0);
    assert!(rule.host().has_pending_work());
}

// ---- C++ 27185: basic_marquee_measures_probes_without_writing ---------------------------------------------------

fn basic_marquee_measures_probes_without_writing() {
    // C++'s basic_marquee_measures_probes_without_writing: an intrinsic probe doesn't set its widths (Compose's
    // intrinsics, b/278729564), so the layout settles at once.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            modifier = Modifier.width_in(0.dp(), 100.dp()).width_intrinsic(IntrinsicSize::Max),
            content = || {
                Box(
                    modifier = Modifier.fill_max_width().test_tag("kayan").basic_marquee(iterations = 0),
                    content = || Box(Modifier.size_wh(300.dp(), 20.dp()).background(RED)),
                );
            },
        );
    }));
    assert_eq!(rule.on_node_with_tag("kayan").get_unclipped_bounds_in_root().width(), 100.0);
    assert!(!rule.host().has_pending_work());
}

// ---- C++ 27198: basic_marquee_backwards_and_while_focused -------------------------------------------------------

fn basic_marquee_backwards_and_while_focused() {
    // C++'s basic_marquee_backwards_and_while_focused.
    let requester = FocusRequester::new();
    let shown = mutable_state_of(false); // (composed later: waiting for idle would run it to its end)
    let (rq, s) = (requester.clone(), shown.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (rq, s) = (rq.clone(), s.clone());
        Column(move || {
            if s.get() {
                Box(
                    modifier = Modifier.width(100.dp()).basic_marquee(
                        initial_delay_millis = 0,
                        spacing = MarqueeSpacing(50.dp()),
                        velocity = (-40).dp(),
                    ),
                    content = || Box(Modifier.size_wh(150.dp(), 20.dp()).background(RED)),
                );
            }
            Box(
                modifier = Modifier
                    .width(100.dp())
                    .basic_marquee(
                        animation_mode = MarqueeAnimationMode::WhileFocused,
                        spacing = MarqueeSpacing(10.dp()),
                    )
                    .focus_requester(&rq)
                    .focusable(),
                content = || Box(Modifier.size_wh(150.dp(), 20.dp()).background(BLUE).test_tag("mavi")),
            );
        });
    }));
    let blue = |rule: &KompozTestRule| -> Vec<f32> { rects_of(&rule.draw(), BLUE).iter().map(|r| r.left).collect() };
    shown.set(true);
    rule.advance_frame();
    rule.advance_time_by(1000); // a negative velocity: no wait asked, moving right
    let red = red_rects(&rule);
    assert!(red.len() == 1 && near(red[0].left, 40.0, 3.0), "{red:?}");
    rule.advance_time_by(1500); // 100 in: the copy's end comes in from the left past the 50 px gap
    let red = red_rects(&rule);
    assert_eq!(red.len(), 2, "{red:?}");
    assert!(near(red[1].left - red[0].left, 200.0, 0.01) && red[0].right > 0.0);
    assert_eq!(blue(&rule), vec![0.0]); // not focused: still
    requester.request_focus();
    rule.advance_time_by(500); // focused: at once (no wait), a few frames after the focus came
    let xs = blue(&rule);
    assert!(xs.len() == 1 && near(xs[0], -15.0, 3.0), "{xs:?}");
    rule.host().clear_focus(true);
    rule.advance_time_by(100);
    assert_eq!(blue(&rule), vec![0.0]); // back at the start
}

// ---- Compose's spacing and modes --------------------------------------------------------------------------------

fn marquee_spacing_and_modes_as_compose() {
    let density = Density::new(2.0, 1.0);
    assert_eq!(MarqueeSpacing(10.dp()).calculate_spacing(&density, 300, 100), 20);
    assert_eq!(MarqueeSpacing::fraction_of_container(1.0 / 3.0).calculate_spacing(&density, 300, 100), 33);
    assert_eq!(MarqueeDefaults::Spacing.calculate_spacing(&density, 0, 200), 67);
    let custom = MarqueeSpacing::new(|_, content, container| content - container);
    assert_eq!(custom.calculate_spacing(&density, 300, 100), 200);
    assert!(custom == custom.clone() && MarqueeSpacing(1.dp()) == MarqueeSpacing(1.dp()));
    assert_eq!(MarqueeAnimationMode::WhileFocused.to_string(), "WhileFocused");
    assert_eq!(format!("{:?}", MarqueeAnimationMode::Immediately), "Immediately");
}

tests![
    basic_marquee_scrolls_what_doesnt_fit_and_its_copy_follows,
    a_marquee_out_of_view_goes_on_as_composes_does,
    basic_marquee_leaves_alone_what_fits_and_starts_again_when_it_grows,
    basic_marquee_measures_probes_without_writing,
    basic_marquee_backwards_and_while_focused,
    marquee_spacing_and_modes_as_compose,
];
