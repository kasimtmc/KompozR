//! foundation.gestures beyond one axis: `draggable2d`, `scrollable2d`, `transformable` and `anchored_draggable` with
//! their states, `DraggableAnchors` (the C++ Kompoz's tests, Compose's behaviour where they differ: each says so).
//! Numbers in px (density 1).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_coroutines::CoroutineScope;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::input::*;
use kompoz_ui::platform::{LocalViewConfiguration, ViewConfiguration};
use kompoz_ui::unit::Velocity;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::Arc;

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn slop() -> f32 {
    ViewConfiguration::default().touch_slop
}

fn down(rule: &KompozTestRule, p: Offset, id: i64) {
    rule.host().pointer_down(p, -1, id, PointerType::Touch);
}

fn mv(rule: &KompozTestRule, p: Offset, id: i64) {
    rule.host().pointer_move(p, -1, id);
}

fn up(rule: &KompozTestRule, p: Offset, id: i64) {
    rule.host().pointer_up(p, -1, id);
}

/// C++'s `rule.swipe`: a finger from `from` to `to`, one move a frame, let go, a frame.
fn swipe(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    down(rule, from, 0);
    for i in 1..=frames {
        rule.advance_frame();
        mv(rule, from + (to - from) * (i as f32 / frames as f32), 0);
    }
    up(rule, to, 0);
    rule.advance_frame();
}

/// C++'s `slowDrag`: a finger drag in `frames` moves that holds still before lifting (no fling).
fn slow_drag(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    down(rule, from, 0);
    for i in 1..=frames {
        rule.advance_frame();
        mv(rule, from + (to - from) * (i as f32 / frames as f32), 0);
    }
    rule.main_clock().advance_time_by(200);
    up(rule, to, 0);
    rule.advance_frame();
    rule.wait_for_idle();
}

/// C++'s `pinch`: fingers 1 and 2 from their start to their end in `steps` frames, then lifted.
fn pinch(rule: &KompozTestRule, a0: Offset, a1: Offset, b0: Offset, b1: Offset) {
    down(rule, a0, 1);
    down(rule, b0, 2);
    for i in 1..=8 {
        rule.advance_frame();
        let t = i as f32 / 8.0;
        mv(rule, a0 + (a1 - a0) * t, 1);
        mv(rule, b0 + (b1 - b0) * t, 2);
    }
    up(rule, a1, 1);
    up(rule, b1, 2);
    rule.advance_frame();
    rule.wait_for_idle();
}

fn add_into(s: MutableState<Offset>) -> impl Fn(Offset) + 'static {
    move |d| s.set(s.peek() + d)
}

/// Runs `f` in a coroutine of the host's, until idle.
fn run_until_idle(rule: &KompozTestRule, f: impl std::future::Future<Output = ()> + 'static) {
    CoroutineScope::new(rule.host().dispatcher()).launch(f);
    rule.wait_for_idle();
}

// ---------------------------------------------------------------------------------------------------------------
// draggable2d

fn draggable2d_drags_any_way_from_the_slop_or_at_once() {
    // C++ 14242. Compose's: a drag's start is where it crossed the slop (DragGestureNode's `startedPosition`: the
    // move that crossed it less what went past), not where the finger went down.
    let (pos, back, now) =
        (mutable_state_of(Offset::Zero), mutable_state_of(Offset::Zero), mutable_state_of(Offset::Zero));
    let started = Rc::new(Cell::new(at(-1.0, -1.0)));
    let stopped = Rc::new(Cell::new(None::<Velocity>));
    let code = Rc::new(RefCell::new(None::<DefaultDraggable2DState>));
    let source = MutableInteractionSource::new();
    let dragged = Rc::new(Cell::new(false));
    let (p, b, n, st, sp, c, s, dr) = (
        pos.clone(),
        back.clone(),
        now.clone(),
        started.clone(),
        stopped.clone(),
        code.clone(),
        source.clone(),
        dragged.clone(),
    );
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = remember_draggable2d_state(add_into(p.clone()));
            *c.borrow_mut() = Some(state.clone());
            let reversed = remember_draggable2d_state(add_into(b.clone()));
            let immediate = remember_draggable2d_state(add_into(n.clone()));
            dr.set(s.collect_is_dragged_as_state().get());
            let (st, sp, s) = (st.clone(), sp.clone(), s.clone());
            Column(move || {
                let (st, sp) = (st.clone(), sp.clone());
                Box(modifier = named_args!(Modifier.size(200.dp()).draggable2d(
                    state.clone(),
                    interaction_source = Some(s.clone()),
                    on_drag_started = Some(Rc::new(move |p| st.set(p)) as Rc<dyn Fn(Offset)>),
                    on_drag_stopped = Some(Rc::new(move |v| sp.set(Some(v))) as Rc<dyn Fn(Velocity)>)
                )));
                Box(modifier =
                    named_args!(Modifier.size(200.dp()).draggable2d(reversed.clone(), reverse_direction = true)));
                Box(modifier =
                    named_args!(Modifier.size(100.dp()).draggable2d(immediate.clone(), start_drag_immediately = true)));
            });
        }),
        IntSize::new(400, 800),
    );
    // Diagonally, 50 px (40 across, 30 down): what is past the touch slop, along the drag.
    swipe(&rule, at(50.0, 50.0), at(90.0, 80.0), 10);
    rule.wait_for_idle();
    let keep = (50.0 - slop()) / 50.0;
    let p = pos.peek();
    assert!(near(p.x, 40.0 * keep, 0.01) && near(p.y, 30.0 * keep, 0.01), "{p:?}");
    let crossed = at(50.0 + 40.0 * slop() / 50.0, 50.0 + 30.0 * slop() / 50.0);
    assert!(near(started.get().x, crossed.x, 0.5) && near(started.get().y, crossed.y, 0.5), "{:?}", started.get());
    let v = stopped.get().expect("stopped");
    assert!(v.x > 100.0 && near(v.y, v.x * 0.75, 1.0), "{v:?}");
    // Reversed: the other way.
    swipe(&rule, at(50.0, 250.0), at(90.0, 280.0), 10);
    rule.wait_for_idle();
    let b = back.peek();
    assert!(near(b.x, -40.0 * keep, 0.01) && near(b.y, -30.0 * keep, 0.01), "{b:?}");
    // Told to the interaction source while dragging.
    down(&rule, at(100.0, 100.0), 0);
    rule.advance_frame();
    mv(&rule, at(130.0, 130.0), 0);
    rule.wait_for_idle();
    assert!(dragged.get());
    up(&rule, at(130.0, 130.0), 0);
    rule.wait_for_idle();
    assert!(!dragged.get());
    // At once: a move under the touch slop already counts.
    down(&rule, at(50.0, 450.0), 0);
    rule.advance_frame();
    mv(&rule, at(53.0, 452.0), 0);
    rule.wait_for_idle();
    let n = now.peek();
    assert!(near(n.x, 3.0, 0.01) && near(n.y, 2.0, 0.01), "{n:?}");
    up(&rule, at(53.0, 452.0), 0);
    rule.wait_for_idle();
    // From code, as a drag would.
    let before = pos.peek();
    code.borrow().as_ref().unwrap().dispatch_raw_delta(at(5.0, -5.0));
    assert_eq!(pos.peek(), before + at(5.0, -5.0));
}

fn draggable2d_keeps_its_drag_from_the_scroll_around_it() {
    // C++ 14296.
    let (pos, now) = (mutable_state_of(Offset::Zero), mutable_state_of(Offset::Zero));
    let scroll = ScrollState::new(0);
    let (p, n, sc) = (pos.clone(), now.clone(), scroll.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = remember_draggable2d_state(add_into(p.clone()));
            let immediate = remember_draggable2d_state(add_into(n.clone()));
            Column(
                modifier = Modifier.fill_max_size().vertical_scroll(&sc),
                content = move || {
                    Box(modifier = Modifier.size(200.dp()).draggable2d(state.clone()));
                    Box(modifier = named_args!(
                        Modifier.size(200.dp()).draggable2d(immediate.clone(), start_drag_immediately = true)
                    ));
                    Box(modifier = Modifier.fill_max_width().height(2000.dp()));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    // Up, on the draggable: it is the drag's, the page stays.
    swipe(&rule, at(100.0, 150.0), at(100.0, 50.0), 10);
    rule.wait_for_idle();
    assert!(near(pos.peek().y, -(100.0 - slop()), 0.01) && scroll.value() == 0, "{:?} {}", pos.peek(), scroll.value());
    // The same at once.
    swipe(&rule, at(100.0, 350.0), at(100.0, 250.0), 10);
    rule.wait_for_idle();
    assert!(near(now.peek().y, -100.0, 0.01) && scroll.value() == 0, "{:?}", now.peek());
    // Off it the page scrolls.
    swipe(&rule, at(100.0, 600.0), at(100.0, 400.0), 10);
    rule.wait_for_idle();
    assert!(scroll.value() > 100, "{}", scroll.value());
}

// ---------------------------------------------------------------------------------------------------------------
// scrollable2d

fn scrollable2d_drags_and_flings_both_ways() {
    // C++ 14319, as Compose does it: the content moves with the finger within [-1000, 0] across and [-300, 0] down,
    // in a page that scrolls. Compose's scrollable2D differs from the C++ where its nested scrolling does: what the
    // box doesn't take goes on to the page (its nested scroll parent: C++'s page stayed); and it takes no mouse
    // wheel (Compose's Scrollable2DNode makes no wheel logic): the wheel scrolls the page.
    let pan = mutable_state_of(Offset::Zero);
    let scrolled = Rc::new(RefCell::new(None::<DefaultScrollable2DState>));
    let page = ScrollState::new(0);
    let (pn, sc, pg) = (pan.clone(), scrolled.clone(), page.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let pn = pn.clone();
            let state = remember_scrollable2d_state(move |d| {
                let p = pn.peek();
                let to = at((p.x + d.x).clamp(-1000.0, 0.0), (p.y + d.y).clamp(-300.0, 0.0));
                pn.set(to);
                to - p
            });
            *sc.borrow_mut() = Some(state.clone());
            Column(
                modifier = Modifier.fill_max_size().vertical_scroll(&pg),
                content = move || {
                    Box(modifier = Modifier.size(300.dp()).scrollable2d(state.clone()));
                    Box(modifier = Modifier.fill_max_width().height(2000.dp()));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let state = scrolled.borrow().clone().unwrap();
    // Up and left, 128 px: what is past the touch slop, along the drag.
    down(&rule, at(250.0, 250.0), 0);
    for i in 1..=10 {
        rule.advance_frame();
        mv(&rule, at(250.0 - 10.0 * i as f32, 250.0 - 8.0 * i as f32), 0);
    }
    rule.advance_frame(); // (the last delta taken by the drag's coroutine)
    let length = (100.0f32 * 100.0 + 80.0 * 80.0).sqrt();
    let keep = (length - slop()) / length;
    let p = pan.peek();
    assert!(near(p.x, -100.0 * keep, 0.01) && near(p.y, -80.0 * keep, 0.01), "{p:?}");
    assert!(state.is_scroll_in_progress());
    up(&rule, at(150.0, 170.0), 0);
    rule.advance_frame();
    assert!(state.is_scroll_in_progress(), "flinging on");
    rule.advance_time_by(3000);
    rule.wait_for_idle();
    let flung = pan.peek();
    assert!(
        flung.x < -100.0 * keep - 20.0 && flung.y < -80.0 * keep - 20.0 && near(flung.y / flung.x, 0.8, 0.05),
        "{flung:?}"
    );
    assert!(!state.is_scroll_in_progress() && page.value() == 0, "the gesture was the box's");
    // Mostly up a lot: the top end (-300) is reached and the fling goes on across, what the box can't take of it
    // going on to the page (its nested scroll parent: the fling's steps taken whole, it isn't cut short).
    swipe(&rule, at(200.0, 290.0), at(170.0, 10.0), 6);
    let released = pan.peek();
    rule.advance_time_by(3000);
    rule.wait_for_idle();
    assert!(near(pan.peek().y, -300.0, 0.01) && pan.peek().x < released.x - 40.0, "{:?} {released:?}", pan.peek());
    assert!(page.value() > 0, "the rest went to the page: {}", page.value());
    // Into the corner: both ends reached, its fling is over at once.
    state.dispatch_raw_delta(at(-1000.0, -300.0));
    swipe(&rule, at(100.0, 100.0), at(60.0, 60.0), 3);
    rule.advance_time_by(100);
    assert!(!state.is_scroll_in_progress());
    rule.wait_for_idle();
    // A press stops a fling.
    let s = page.clone();
    run_until_idle(&rule, async move {
        s.scroll_to(0).await;
    });
    state.dispatch_raw_delta(at(1000.0, 300.0)); // (back to the start)
    swipe(&rule, at(100.0, 200.0), at(50.0, 200.0), 4);
    let released = pan.peek();
    down(&rule, at(150.0, 150.0), 0);
    rule.advance_time_by(1000);
    assert!(pan.peek().x < 0.0 && near(pan.peek().x, released.x, 30.0), "{:?} {released:?}", pan.peek());
    up(&rule, at(150.0, 150.0), 0);
    rule.wait_for_idle();
    // The wheel goes past it: the page scrolls a notch (64 px), the box stays.
    let before = (pan.peek(), page.value());
    rule.host().scroll(at(150.0, 150.0), at(0.0, 1.0));
    rule.host().scroll(at(150.0, 150.0), at(1.0, 0.0));
    rule.wait_for_idle();
    assert!(pan.peek() == before.0 && page.value() == before.1 + 64, "{:?} {}", pan.peek(), page.value());
}

fn scrollable2d_waits_for_the_slop_of_its_place() {
    // The scrollable2D part of C++ 8642: under a view configuration of a 40 px touch slop, nothing scrolls at 30
    // px, then from the move past it on.
    let panned = mutable_state_of(Offset::Zero);
    let pn = panned.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let pn = pn.clone();
            let vc = ViewConfiguration { touch_slop: 40.0, ..ViewConfiguration::default() };
            KompozitionLocalProvider(&[LocalViewConfiguration.provides(vc)], move || {
                let pn = pn.clone();
                let state = remember_scrollable2d_state(move |d| {
                    pn.set(pn.peek() + d);
                    d
                });
                Box(modifier = Modifier.size(100.dp()).scrollable2d(state));
            });
        }),
        IntSize::new(400, 800),
    );
    down(&rule, at(10.0, 50.0), 0);
    for x in (15..=40).step_by(5) {
        rule.advance_frame();
        mv(&rule, at(x as f32, 50.0), 0);
    }
    assert_eq!(panned.peek().x, 0.0);
    for x in (45..=60).step_by(5) {
        rule.advance_frame();
        mv(&rule, at(x as f32, 50.0), 0);
    }
    assert!(panned.peek().x > 0.0);
    up(&rule, at(60.0, 50.0), 0);
    rule.wait_for_idle();
}

fn scrollable2d_state_scrolls_from_code() {
    // Compose's ScrollExtensions of a 2D state: scrollBy, animateScrollBy, stopScroll.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {}));
    let pan = Rc::new(Cell::new(Offset::Zero));
    let p = pan.clone();
    let state = Scrollable2DState(move |d| {
        p.set(p.get() + d);
        d
    });
    let (s, got) = (state.clone(), Rc::new(Cell::new(Offset::Zero)));
    let (g, g2) = (got.clone(), got.clone());
    run_until_idle(&rule, async move {
        g.set(s.scroll_by(at(10.0, -20.0)).await);
    });
    assert_eq!((pan.get(), got.get()), (at(10.0, -20.0), at(10.0, -20.0)));
    let s = state.clone();
    run_until_idle(&rule, async move {
        g2.set(s.animate_scroll_by(at(100.0, 50.0)).await);
    });
    assert!(near(got.get().x, 100.0, 0.01) && near(pan.get().y, 30.0, 0.01), "{:?} {:?}", got.get(), pan.get());
    assert!(!state.is_scroll_in_progress() && state.can_scroll(at(1.0, 1.0)));
}

// ---------------------------------------------------------------------------------------------------------------
// transformable

fn transformable_pinches_but_leaves_one_finger_to_the_scroll() {
    // C++ 21428, as Compose does it: one finger it doesn't pan (`can_pan` says no while not zoomed) goes to the
    // page around it; two fingers zoom; zoomed in, one finger pans. Compose's differ: once the page took a finger's
    // drag, the gesture is the page's until all fingers lift (the transformable's detection ends when what it
    // waited on is consumed: a second finger landing doesn't zoom — C++'s pinch took over); its mouse zooms with
    // Ctrl+wheel and a trackpad's pinch (no turn: Compose's has none of a trackpad).
    let scroll = ScrollState::new(0);
    let scale = mutable_state_of(1.0f32);
    let offset = mutable_state_of(Offset::Zero);
    let (sc, s, o) = (scroll.clone(), scale.clone(), offset.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (s2, o) = (s.clone(), o.clone());
            let state = remember_transformable_state(move |_, zoom, pan, _| {
                s2.set((s2.peek() * zoom).clamp(1.0, 5.0));
                o.set(o.peek() + pan);
            });
            let zoomed = s.clone();
            Column(
                modifier = Modifier.fill_max_size().vertical_scroll(&sc).test_tag("sayfa"),
                content = move || {
                    let zoomed = zoomed.clone();
                    Box(modifier = Modifier
                        .fill_max_width()
                        .height(400.dp())
                        .transformable_with_can_pan(state.clone(), move |_| zoomed.peek() > 1.0));
                    Box(modifier = Modifier.height(2000.dp()));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let scroll_to_top = |rule: &KompozTestRule| {
        let s = scroll.clone();
        run_until_idle(rule, async move {
            s.scroll_to(0).await;
        });
    };
    // One finger, not zoomed: the page scrolls.
    slow_drag(&rule, at(200.0, 300.0), at(200.0, 100.0), 10);
    assert!(scroll.value() > 150 && offset.peek().y == 0.0, "{} {:?}", scroll.value(), offset.peek());
    scroll_to_top(&rule);
    // Two fingers: zoom, the page stays.
    pinch(&rule, at(150.0, 200.0), at(100.0, 200.0), at(250.0, 200.0), at(300.0, 200.0));
    assert!(scale.peek() > 1.5 && scroll.value() == 0, "{} {}", scale.peek(), scroll.value());
    // Zoomed in: one finger pans.
    slow_drag(&rule, at(200.0, 300.0), at(200.0, 200.0), 10);
    assert!(offset.peek().y < -80.0 && scroll.value() == 0, "{:?} {}", offset.peek(), scroll.value());
    // Not zoomed, the first finger drifts past the slop (the page scrolls) and a second lands: the gesture stays
    // the page's (Compose's).
    scale.set(1.0);
    rule.wait_for_idle();
    down(&rule, at(150.0, 200.0), 1);
    for i in 1..=3 {
        rule.advance_frame();
        mv(&rule, at(150.0, 200.0 - 5.0 * i as f32), 1);
    }
    assert!(scroll.value() > 0, "it was scrolling");
    pinch(&rule, at(150.0, 185.0), at(100.0, 185.0), at(250.0, 185.0), at(300.0, 185.0));
    assert_eq!(scale.peek(), 1.0);
    scroll_to_top(&rule);
    // The mouse and the trackpad: a pinch zooms what is under the pointer.
    let h = rule.host();
    assert!(h.zoom(at(200.0, 150.0), 1.5, 0.0) && near(scale.peek(), 1.5, 0.001), "{}", scale.peek());
    assert!(!h.zoom(at(200.0, 700.0), 1.5, 0.0) && near(scale.peek(), 1.5, 0.001)); // (below the box: nothing)
    assert!(!h.zoom(at(200.0, 150.0), 1.0, 30.0) && near(scale.peek(), 1.5, 0.001)); // (a turn: nothing)
    assert!(!h.zoom(at(200.0, 150.0), 1.0, 0.0));
    // Ctrl+wheel: a notch down, 64 px of scroll, zooms out by 2^(-64/545); the page stays.
    h.set_keyboard_modifiers(KeyModifiers::Ctrl);
    h.scroll(at(200.0, 150.0), at(0.0, 1.0));
    rule.wait_for_idle();
    h.set_keyboard_modifiers(KeyModifiers::None);
    let expected = 1.5 * 2.0f32.powf(-64.0 / 545.0);
    assert!(near(scale.peek(), expected, 0.001) && scroll.value() == 0, "{} {}", scale.peek(), scroll.value());
    // A trackpad's pan pans.
    let before = offset.peek();
    h.pan(at(200.0, 150.0), PointerEventType::PanStart, Offset::Zero);
    h.pan(at(200.0, 150.0), PointerEventType::PanMove, at(0.0, 30.0));
    h.pan(at(200.0, 150.0), PointerEventType::PanEnd, Offset::Zero);
    rule.wait_for_idle();
    assert_eq!(offset.peek(), before + at(0.0, -30.0));
}

fn transformable_state_transforms_from_code() {
    // TransformableState.kt's calls: zoomBy, panBy, rotateBy, animateZoomBy/PanBy/RotateBy/animateBy, and the
    // centroid they are given; a transformation stops another.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {}));
    let seen = Rc::new(RefCell::new((1.0f32, Offset::Zero, 0.0f32, Offset::Zero)));
    let sn = seen.clone();
    let state = TransformableState(move |c, z, p, r| {
        let mut s = sn.borrow_mut();
        *s = (s.0 * z, s.1 + p, s.2 + r, c);
    });
    let st = state.clone();
    run_until_idle(&rule, async move {
        named_args!(st.zoom_by(2.0, centroid = at(5.0, 6.0))).await;
        st.pan_by(at(3.0, 4.0)).await;
        st.rotate_by(30.0).await;
    });
    let s = *seen.borrow();
    assert!(s.0 == 2.0 && s.1 == at(3.0, 4.0) && s.2 == 30.0 && s.3.is_unspecified(), "{s:?}");
    let st = state.clone();
    run_until_idle(&rule, async move {
        st.animate_zoom_by(1.5).await;
        st.animate_pan_by(at(10.0, 0.0)).await;
        st.animate_rotate_by(-30.0).await;
        named_args!(st.animate_by(2.0, at(0.0, 10.0), 90.0, centroid = at(1.0, 1.0))).await;
    });
    let s = *seen.borrow();
    assert!(
        near(s.0, 6.0, 0.01) && near(s.1.x, 13.0, 0.01) && near(s.1.y, 14.0, 0.01) && near(s.2, 90.0, 0.01),
        "{s:?}"
    );
    assert_eq!(s.3, at(1.0, 1.0));
    assert!(!state.is_transform_in_progress());
    // A transformation in progress is stopped by another.
    let (st, ended) = (state.clone(), Rc::new(Cell::new(false)));
    let e = ended.clone();
    CoroutineScope::new(rule.host().dispatcher()).launch(async move {
        st.animate_pan_by(at(1000.0, 0.0)).await;
        e.set(true);
    });
    rule.advance_frame();
    rule.advance_frame();
    assert!(state.is_transform_in_progress());
    let st = state.clone();
    run_until_idle(&rule, async move {
        st.stop_transformation().await;
    });
    assert!(!state.is_transform_in_progress() && !ended.get() && seen.borrow().1.x < 1013.0);
}

// ---------------------------------------------------------------------------------------------------------------
// anchored_draggable

#[derive(Clone, Copy, PartialEq, Debug)]
enum Spot {
    Start,
    Middle,
    End,
}

fn draggable_anchors_and_state_without_a_screen() {
    // C++ 23661, as Compose's anchors and state are: a key given twice is there twice (the first one's position is
    // its: C++ replaced it); a tie goes to the later anchor; the target is the anchor nearest the offset; the
    // current value is the next anchor once past half way to it; an offset read before anchors (and a raw delta)
    // throws.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {}));
    let anchors = DraggableAnchors(|a| {
        a.at(0, 0.0);
        a.at(1, 100.0);
        a.at(2, 250.0);
    });
    assert!(anchors.size() == 3 && anchors.position_of(&1) == 100.0 && anchors.position_of(&5).is_nan());
    assert!(anchors.min_position() == 0.0 && anchors.max_position() == 250.0 && anchors.has_position_for(&2));
    assert_eq!(anchors.closest_anchor(70.0), Some(1));
    assert_eq!(anchors.closest_anchor(50.0), Some(1)); // (a tie: the later)
    assert_eq!(anchors.closest_anchor_in_direction(70.0, false), Some(0));
    assert_eq!(anchors.closest_anchor_in_direction(130.0, true), Some(2));
    assert_eq!(anchors.closest_anchor_in_direction(260.0, true), Some(2)); // (none that way: still the last)
    assert_eq!(anchors.closest_anchor_in_direction(260.0, false), Some(2));
    let twice = DraggableAnchors(|a| {
        a.at(1, 100.0);
        a.at(1, 120.0);
    });
    assert!(twice.size() == 2 && twice.position_of(&1) == 100.0);
    let mut seen = Vec::new();
    anchors.for_each(&mut |k, p| seen.push((k, p)));
    assert_eq!(seen, [(0, 0.0), (1, 100.0), (2, 250.0)]);

    let two = || {
        DraggableAnchors(|a| {
            a.at(0, 0.0);
            a.at(1, 100.0);
        })
    };
    let state = anchored_draggable_state_with_anchors(0, two());
    assert!(state.offset() == 0.0 && state.current_value() == 0 && state.target_value() == 0);
    assert_eq!(state.dispatch_raw_delta(25.0), 25.0);
    assert!(near(state.progress(&0, &1), 0.25, 0.0001) && state.target_value() == 0 && state.current_value() == 0);
    assert_eq!(state.dispatch_raw_delta(1000.0), 75.0); // (kept within)
    assert!(state.offset() == 100.0 && state.current_value() == 1);
    assert_eq!(state.dispatch_raw_delta(-30.0), -30.0);
    assert!(state.target_value() == 1 && state.current_value() == 1 && state.settled_value() == 0);
    let s = state.clone();
    run_until_idle(&rule, async move { s.snap_to(1).await });
    assert!(state.offset() == 100.0 && state.settled_value() == 1 && state.target_value() == 1);

    let three = anchored_draggable_state_with_anchors(0, anchors.clone());
    three.dispatch_raw_delta(90.0); // (past half way to the next: its value)
    assert!(three.current_value() == 1 && three.target_value() == 1);
    three.dispatch_raw_delta(85.0); // (175: past `to`, the progress stays 1)
    assert!(near(three.progress(&0, &1), 1.0, 0.001) && near(three.progress(&1, &2), 0.5, 0.001));

    let later = AnchoredDraggableState(1); // (no anchors yet: no offset; its value waits)
    assert!(later.offset().is_nan());
    assert!(catch_unwind(AssertUnwindSafe(|| later.require_offset())).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| later.dispatch_raw_delta(10.0))).is_err());
    named_args!(later.update_anchors(DraggableAnchors(|a| {
        a.at(0, 0.0);
        a.at(1, 80.0);
    })));
    assert!(later.offset() == 80.0 && later.current_value() == 1);
    let saver = AnchoredDraggableState::<i32>::Saver();
    let restored = saver.restore(&saver.save(&later)).expect("restored");
    assert_eq!(restored.current_value(), 1);
}

fn anchored_draggable_follows_the_finger_and_settles_on_an_anchor() {
    // C++ 23687, as Compose does it: the drag counts from the slop (60 less it: C++ dropped the first step); the
    // target is the anchor nearest the offset; a fling past the velocity threshold goes to the next anchor its way,
    // the fling's offsets dragged without a velocity (the last velocity 0); an animation from code is a drag that
    // asks to confirm its target too (End refused: it lands there, its value the last one confirmed); new anchors
    // put it at the one nearest its offset (C++ kept its value).
    let width = mutable_state_of(356.0f32);
    let holder = Rc::new(RefCell::new(None::<(AnchoredDraggableState<Spot>, CoroutineScope)>));
    let (w, hd) = (width.clone(), holder.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = remember(|| anchored_draggable_state_confirming(Spot::Start, |v: &Spot| *v != Spot::End));
            let scope = kompoz_coroutines::remember_coroutine_scope();
            *hd.borrow_mut() = Some((s.clone(), scope));
            let width = w.get();
            let s2 = s.clone();
            Column(move || {
                let (s, s2) = (s2.clone(), s2.clone());
                Box(
                    modifier = Modifier.width(width.dp()).height(56.dp()).on_size_changed(move |size| {
                        let w = size.width as f32;
                        named_args!(s.update_anchors(DraggableAnchors(|a| {
                            a.at(Spot::Start, 0.0);
                            a.at(Spot::Middle, (w - 56.0) / 2.0);
                            a.at(Spot::End, w - 56.0);
                        })));
                    }),
                    content = move || {
                        let s = s2.clone();
                        let o = s.clone();
                        Box(
                            modifier = Modifier
                                .offset_px(move |_| IntOffset::new(o.require_offset().round() as i32, 0))
                                .anchored_draggable(s.clone(), Orientation::Horizontal)
                                .size(56.dp()),
                            content = || Box(modifier = Modifier.fill_max_size().test_tag("tutamak")),
                        );
                    },
                );
                let st = s2.clone();
                Text(format!("şimdi {} hedef {}", st.current_value() as i32, st.target_value() as i32));
            });
        }),
        IntSize::new(400, 800),
    );
    let (s, scope) = holder.borrow().clone().unwrap();
    let x = || rule.on_node_with_tag("tutamak").fetch_semantics_node().unclipped_bounds_in_root().left;
    // Moves without frames between, then held still: let go without speed.
    let drag = |steps: &[f32]| {
        let c = at(x() + 28.0, 28.0);
        down(&rule, c, 0);
        for d in steps {
            mv(&rule, at(c.x + d, c.y), 0);
        }
        rule.advance_time_by(150);
        at(c.x + steps[steps.len() - 1], c.y)
    };
    assert!(x() == 0.0 && s.anchors().position_of(&Spot::End) == 300.0);
    let p = drag(&[20.0, 40.0, 60.0]);
    assert!(near(x(), 60.0 - slop(), 0.5), "{}", x());
    rule.on_node_with_text("şimdi 0 hedef 0").assert_exists();
    up(&rule, p, 0); // (not half way: back)
    rule.wait_for_idle();
    assert!(x() == 0.0 && s.current_value() == Spot::Start);
    let p = drag(&[20.0, 60.0, 120.0]); // (112 of 150: on to the middle)
    up(&rule, p, 0);
    rule.wait_for_idle();
    assert!(x() == 150.0 && s.current_value() == Spot::Middle && s.settled_value() == Spot::Middle, "{}", x());
    let p = drag(&[20.0, 60.0, 120.0]); // (past half way to the end, which isn't allowed: back)
    up(&rule, p, 0);
    rule.wait_for_idle();
    assert!(x() == 150.0 && s.current_value() == Spot::Middle, "{}", x());
    swipe(&rule, at(x() + 28.0, 28.0), at(x() - 2.0, 28.0), 3); // (flung: 22 px past the slop is enough)
    rule.wait_for_idle();
    assert!(x() == 0.0 && s.current_value() == Spot::Start && s.last_velocity() == 0.0, "{}", x());
    rule.host().scroll(at(x() + 28.0, 28.0), at(0.0, -5.0)); // (the wheel doesn't move it)
    rule.wait_for_idle();
    assert_eq!(s.offset(), 0.0);
    let st = s.clone();
    scope.launch(async move { named_args!(st.animate_to(Spot::Middle)).await });
    rule.advance_frame();
    rule.advance_frame();
    assert!(s.is_animation_running() && s.offset() > 0.0);
    drag(&[-20.0, -30.0]); // (the user takes over: at once, the animation stopped)
    assert!(!s.is_animation_running());
    up(&rule, at(x() + 28.0, 28.0), 0);
    rule.wait_for_idle();
    assert!(x() == 0.0 && s.current_value() == Spot::Start, "{}", x());
    let st = s.clone();
    scope.launch(async move { named_args!(st.animate_to(Spot::End)).await });
    rule.advance_frame();
    assert!(s.is_animation_running() && s.target_value() == Spot::End && s.current_value() == Spot::Start);
    rule.wait_for_idle();
    assert!(x() == 300.0 && s.current_value() == Spot::Middle && !s.is_animation_running(), "{}", x());
    let st = s.clone();
    scope.launch(async move { st.snap_to(Spot::Middle).await });
    rule.wait_for_idle();
    assert_eq!(x(), 150.0);
    rule.on_node_with_text("şimdi 1 hedef 1").assert_exists();
    width.set(200.0); // (new anchors: at the one nearest its offset, 150: the end's 144)
    rule.wait_for_idle();
    assert!(x() == 144.0 && s.current_value() == Spot::End, "{}", x());
}

/// An overscroll effect noting what it is given beyond what scrolls.
#[derive(Default)]
struct NotedOverscroll {
    over: Cell<f32>,
    flings: Cell<i32>,
}

impl PartialEq for NotedOverscroll {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl OverscrollEffect for NotedOverscroll {
    fn apply_to_scroll(
        &self,
        delta: Offset,
        _: NestedScrollSource,
        perform_scroll: &dyn Fn(Offset) -> Offset,
    ) -> Offset {
        let used = perform_scroll(delta);
        self.over.set(self.over.get() + (delta - used).x);
        delta
    }
    fn apply_to_fling(&self, velocity: Velocity, perform_fling: PerformFling) -> LocalFuture<'_, ()> {
        Box::pin(async move {
            self.flings.set(self.flings.get() + 1);
            perform_fling(velocity).await;
        })
    }
    fn is_in_progress(&self) -> bool {
        false
    }
}

fn anchored_draggable_keeps_its_drag_from_the_scrollables_around_it() {
    // C++ 23756: past its ends nothing goes to the row (the drag is its own); an overscroll effect given gets
    // what goes past (Compose's), and the fling through it.
    let scroll = ScrollState::new(0);
    let holder = Rc::new(RefCell::new(None::<AnchoredDraggableState<i32>>));
    let effect = Rc::new(NotedOverscroll::default());
    let (sc, hd, ef) = (scroll.clone(), holder.clone(), effect.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = remember(|| AnchoredDraggableState(0));
            *hd.borrow_mut() = Some(s.clone());
            let (s2, ef) = (s.clone(), ef.clone());
            Row(
                modifier = Modifier.fill_max_width().horizontal_scroll(&sc),
                content = move || {
                    let (s, ef) = (s2.clone(), ef.clone());
                    Box(
                        modifier = Modifier.width(200.dp()).height(56.dp()),
                        content = move || {
                            let o = s.clone();
                            let effect: Rc<dyn OverscrollEffect> = ef.clone();
                            Box(
                                modifier = named_args!(
                                    Modifier
                                        .offset_px(move |_| IntOffset::new(o.require_offset().round() as i32, 0))
                                        .anchored_draggable(
                                            s.clone(),
                                            Orientation::Horizontal,
                                            overscroll_effect = Some(effect)
                                        )
                                )
                                .size(56.dp()),
                                content = || Box(modifier = Modifier.fill_max_size().test_tag("tutamak")),
                            );
                        },
                    );
                    Box(modifier = Modifier.width(800.dp()).height(56.dp()));
                },
            );
            named_args!(s.update_anchors(DraggableAnchors(|a| {
                a.at(0, 0.0);
                a.at(1, 144.0);
            })));
        }),
        IntSize::new(400, 800),
    );
    let s = holder.borrow().clone().unwrap();
    down(&rule, at(28.0, 28.0), 0);
    for d in [20.0, -20.0, -60.0, -100.0] {
        mv(&rule, at(28.0 + d, 28.0), 0); // (past its start: nothing left for the row)
    }
    rule.advance_frame(); // (the deltas taken by the drag's coroutine)
    assert!(s.offset() == 0.0 && scroll.value() == 0, "{} {}", s.offset(), scroll.value());
    assert!(effect.over.get() < -100.0, "{}", effect.over.get());
    for d in [-60.0, 40.0, 120.0, 400.0] {
        mv(&rule, at(28.0 + d, 28.0), 0); // (past its end too)
    }
    rule.advance_frame();
    assert!(s.offset() == 144.0 && scroll.value() == 0, "{} {}", s.offset(), scroll.value());
    up(&rule, at(428.0, 28.0), 0);
    rule.wait_for_idle();
    assert!(s.current_value() == 1 && scroll.value() == 0 && effect.flings.get() == 1);
}

fn anchored_draggable_settles_with_the_flings_speed_within_its_ends() {
    // C++ 24130, through Compose's `animateToWithDecay` (its `settle(velocity)` is the deprecated constructors'):
    // flung at 6000 px/s it decays on to the anchor (stopped there), still it animates; the velocity left is the
    // decay's at the anchor.
    let holder = Rc::new(RefCell::new(None::<CoroutineScope>));
    let hd = holder.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *hd.borrow_mut() = Some(kompoz_coroutines::remember_coroutine_scope());
    }));
    let scope = holder.borrow().clone().unwrap();
    let make = || {
        anchored_draggable_state_with_anchors(
            0,
            DraggableAnchors(|a| {
                a.at(0, 0.0);
                a.at(1, 100.0);
            }),
        )
    };
    let (slow, fast) = (make(), make());
    slow.dispatch_raw_delta(60.0);
    fast.dispatch_raw_delta(60.0);
    let used = Rc::new(Cell::new(0.0f32));
    let (s, f, u) = (slow.clone(), fast.clone(), used.clone());
    scope.launch(async move {
        named_args!(s.animate_to_with_decay(1, 0.0)).await;
    });
    scope.launch(async move {
        u.set(named_args!(f.animate_to_with_decay(1, 6000.0)).await);
    });
    rule.advance_frame();
    rule.advance_frame();
    assert!(fast.offset() > slow.offset(), "{} {}", fast.offset(), slow.offset());
    let mut most = 0.0f32;
    for _ in 0..60 {
        rule.advance_frame();
        most = most.max(fast.offset());
    }
    assert!(most <= 100.0 && fast.offset() == 100.0 && slow.offset() == 100.0 && fast.current_value() == 1);
    assert!(
        used.get() > 0.0 && used.get() < 6000.0 && fast.last_velocity() > 0.0,
        "{} {}",
        used.get(),
        fast.last_velocity()
    );
    // A state of the deprecated constructor settles by its own thresholds: fast enough, the next anchor.
    let configured = anchored_draggable_state_configured_with_anchors(
        0,
        DraggableAnchors(|a| {
            a.at(0, 0.0);
            a.at(1, 100.0);
        }),
        |d| d / 2.0,
        || 125.0,
        Arc::new(named_args!(kompoz_animation::tween::<f32>())),
        Arc::new(named_args!(kompoz_animation::exponential_decay::<f32>())),
        |_: &i32| true,
    );
    configured.dispatch_raw_delta(20.0);
    let c = configured.clone();
    scope.launch(async move {
        c.settle_with_velocity(500.0).await;
    });
    rule.wait_for_idle();
    assert!(configured.offset() == 100.0 && configured.settled_value() == 1);
}

fn anchored_draggable_default_fling_behavior_snaps_by_the_threshold() {
    // AnchoredDraggableDefaults.flingBehavior with a threshold of its own: a slow release a quarter of the way
    // settles on the next anchor.
    let holder = Rc::new(RefCell::new(None::<AnchoredDraggableState<i32>>));
    let hd = holder.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s: AnchoredDraggableState<i32> = remember(|| {
                anchored_draggable_state_with_anchors(
                    0,
                    DraggableAnchors(|a: &mut DraggableAnchorsConfig<i32>| {
                        a.at(0, 0.0);
                        a.at(1, 200.0);
                    }),
                )
            });
            *hd.borrow_mut() = Some(s.clone());
            let fling = AnchoredDraggableDefaults::fling_behavior(
                s.clone(),
                Some(Rc::new(|d: f32| d / 4.0) as Rc<dyn Fn(f32) -> f32>),
                None,
            );
            let fling: Rc<dyn FlingBehavior> = fling;
            Box(modifier = named_args!(Modifier.size(300.dp()).anchored_draggable(
                s.clone(),
                Orientation::Horizontal,
                fling_behavior = Some(fling)
            )));
        }),
        IntSize::new(400, 800),
    );
    let s = holder.borrow().clone().unwrap();
    // 60 px past the slop at a walking pace (a quarter of the way: 50), let go moving.
    down(&rule, at(10.0, 10.0), 0);
    for i in 1..=17 {
        rule.advance_time_by(16);
        mv(&rule, at(10.0 + 4.0 * i as f32, 10.0), 0);
    }
    up(&rule, at(78.0, 10.0), 0);
    rule.wait_for_idle();
    assert!(s.offset() == 200.0 && s.current_value() == 1, "{}", s.offset());
}

tests!(
    draggable2d_drags_any_way_from_the_slop_or_at_once,
    draggable2d_keeps_its_drag_from_the_scroll_around_it,
    scrollable2d_drags_and_flings_both_ways,
    scrollable2d_waits_for_the_slop_of_its_place,
    scrollable2d_state_scrolls_from_code,
    transformable_pinches_but_leaves_one_finger_to_the_scroll,
    transformable_state_transforms_from_code,
    draggable_anchors_and_state_without_a_screen,
    anchored_draggable_follows_the_finger_and_settles_on_an_anchor,
    anchored_draggable_keeps_its_drag_from_the_scrollables_around_it,
    anchored_draggable_settles_with_the_flings_speed_within_its_ends,
    anchored_draggable_default_fling_behavior_snaps_by_the_threshold,
);
