//! foundation.gestures: the tap, drag and transform detectors and the blocks they are made of, the multi-touch
//! calculations, a custom drag through nested scrolling, gestures injected with `perform_touch_input`, and
//! `Modifier.draggable` (the C++ Kompoz's gesture tests that need no clickable or Material; Compose's behaviour where
//! they differ). Numbers in px.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_animation::{FlingCalculator, PlatformFlingScrollFriction};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::input::*;
use kompoz_ui::platform::{LocalViewConfiguration, ViewConfiguration};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

type Log = Rc<RefCell<Vec<String>>>;

fn new_log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn push(log: &Log, s: impl Into<String>) {
    log.borrow_mut().push(s.into());
}

fn taken(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn down_id(rule: &KompozTestRule, p: Offset, id: i64) {
    rule.host().pointer_down(p, -1, id, PointerType::Touch);
}

fn move_id(rule: &KompozTestRule, p: Offset, id: i64) {
    rule.host().pointer_move(p, -1, id);
}

fn up_id(rule: &KompozTestRule, p: Offset, id: i64) {
    rule.host().pointer_up(p, -1, id);
}

/// C++'s `tap`: down, a frame, up, a frame.
fn tap(rule: &KompozTestRule, p: Offset) {
    down_id(rule, p, 0);
    rule.advance_frame();
    up_id(rule, p, 0);
    rule.advance_frame();
}

/// C++'s `slowDrag`: a finger drag in `frames` moves that holds still before lifting (no fling).
fn slow_drag(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    down_id(rule, from, 0);
    for i in 1..=frames {
        rule.advance_frame();
        let t = i as f32 / frames as f32;
        move_id(rule, lerp_offset(from, to, t), 0);
    }
    rule.main_clock().advance_time_by(200);
    up_id(rule, to, 0);
    rule.advance_frame();
}

/// C++'s `pinch`: fingers 1 and 2 from their start to their end in `steps` frames, each move an event of its own.
fn pinch(rule: &KompozTestRule, a0: Offset, a1: Offset, b0: Offset, b1: Offset, steps: i32, lift: bool) {
    down_id(rule, a0, 1);
    down_id(rule, b0, 2);
    for i in 1..=steps {
        rule.advance_frame();
        let t = i as f32 / steps as f32;
        move_id(rule, lerp_offset(a0, a1, t), 1);
        move_id(rule, lerp_offset(b0, b1, t), 2);
    }
    if !lift {
        return;
    }
    up_id(rule, a1, 1);
    up_id(rule, b1, 2);
    rule.advance_frame();
}

/// A tap detector logging its taps (`tap x` local), double taps and long presses.
fn tapper(
    log: &Log,
) -> impl Fn(PointerInputScope) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> + 'static {
    let log = log.clone();
    move |s: PointerInputScope| {
        let (a, b, c) = (log.clone(), log.clone(), log.clone());
        Box::pin(async move {
            s.detect_tap_gestures()
                .on_tap(move |p| push(&a, format!("tap {}", p.x as i32)))
                .on_double_tap(move |_| push(&b, "double"))
                .on_long_press(move |_| push(&c, "long"))
                .await
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Taps

fn tap_gestures_tap_double_tap_and_long_press() {
    // C++'s tap_gestures_tap_double_tap_and_long_press.
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Box(modifier = Modifier.padding(10.dp()).size(100.dp()).pointer_input(0, tapper(&l)));
    }));
    tap(&rule, at(30.0, 30.0));
    assert!(log.borrow().is_empty()); // could still become a double tap
    rule.main_clock().advance_time_by(300 + 100); // (the double tap timeout: a delay, not waited for by idling)
    assert_eq!(taken(&log), ["tap 20"]); // local to the element

    tap(&rule, at(30.0, 30.0));
    rule.main_clock().advance_time_by(48);
    tap(&rule, at(32.0, 30.0));
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["double"]);

    // Its composable running again between the taps, its element replaced while the block waits for the
    // second: the block goes on, and the second tap still doubles it.
    let shift = mutable_state_of(0);
    let composed = Rc::new(Cell::new(0));
    let (sh, c, l) = (shift.clone(), composed.clone(), log.clone());
    let again = KompozTestRule::new(kompozable_lambda!(move || {
        c.set(c.get() + 1);
        Box(modifier = Modifier.size(100.dp()).padding((sh.get() as f32).dp()).pointer_input(0, tapper(&l)));
    }));
    tap(&again, at(30.0, 30.0));
    shift.set(1);
    again.main_clock().advance_time_by(48);
    assert_eq!(composed.get(), 2); // composed again since the first tap
    tap(&again, at(32.0, 30.0));
    again.wait_for_idle();
    assert_eq!(taken(&log), ["double"]);

    // A second tap sooner than the double-tap minimum (40 ms) is none.
    tap(&rule, at(30.0, 30.0));
    tap(&rule, at(32.0, 30.0));
    rule.main_clock().advance_time_by(300 + 100); // (the double tap timeout: a delay, not waited for by idling)
    assert_eq!(taken(&log), ["tap 20"]);

    down_id(&rule, at(30.0, 30.0), 0);
    rule.main_clock().advance_time_by(500);
    assert_eq!(taken(&log), ["long"]);
    up_id(&rule, at(30.0, 30.0), 0);
    rule.wait_for_idle();
    assert!(taken(&log).is_empty());

    // Moving out of the element cancels.
    down_id(&rule, at(30.0, 30.0), 0);
    rule.advance_frame();
    move_id(&rule, at(300.0, 30.0), 0);
    up_id(&rule, at(300.0, 30.0), 0);
    rule.wait_for_idle();
    assert!(taken(&log).is_empty());
}

fn a_tap_after_idling_is_not_a_long_press() {
    // C++'s a_tap_after_idling_is_not_a_long_press: driven by hand, the clock jumping ahead without frames.
    let log = new_log();
    let l = log.clone();
    let host = KompozHost::new(kompozable_lambda!(move || {
        let (a, b) = (l.clone(), l.clone());
        Box(modifier = Modifier.size(100.dp()).pointer_input(0, move |s: PointerInputScope| {
            let (a, b) = (a.clone(), b.clone());
            async move {
                s.detect_tap_gestures().on_tap(move |_| push(&a, "tap")).on_long_press(move |_| push(&b, "long")).await
            }
        }));
    }));
    host.set_viewport_size(IntSize::new(400, 800));
    host.frame(16_000_000);
    let t: i64 = 60_000_000_000; // a minute later
    host.pointer_down(at(50.0, 50.0), t, 0, PointerType::Touch);
    host.frame(t + 16_000_000);
    host.pointer_up(at(50.0, 50.0), t + 80_000_000, 0);
    host.frame(t + 96_000_000);
    assert_eq!(taken(&log), ["tap"]);
}

fn press_gestures_await_their_release_or_cancellation() {
    // `on_press`'s scope: a release, or a cancel when the finger leaves the element.
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Box(modifier = Modifier.size(100.dp()).pointer_input(0, move |s: PointerInputScope| {
            let (l, t) = (l.clone(), l.clone());
            async move {
                s.detect_tap_gestures()
                    .on_press(move |press: PressGestureScope, p: Offset| {
                        let l = l.clone();
                        async move {
                            push(&l, format!("bas {}", p.x));
                            let released = press.try_await_release().await;
                            push(&l, format!("bırakıldı {released}"));
                        }
                    })
                    .on_tap(move |_| push(&t, "tap"))
                    .await
            }
        }));
    }));
    tap(&rule, at(20.0, 20.0));
    rule.wait_for_idle();
    // (the press's coroutine resumes after the tap is told, as Compose's dispatched resumption)
    assert_eq!(taken(&log), ["bas 20", "tap", "bırakıldı true"]);
    down_id(&rule, at(20.0, 20.0), 0);
    rule.advance_frame();
    move_id(&rule, at(200.0, 20.0), 0);
    up_id(&rule, at(200.0, 20.0), 0);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["bas 20", "bırakıldı false"]);
    // Each press after the last one was let go.
    tap(&rule, at(30.0, 30.0));
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["bas 30", "tap", "bırakıldı true"]);
}

fn small_tap_targets_take_touches_near_them_only_for_a_finger() {
    // The tap detector part of C++'s small_touch_targets_take_touches_near_them: a 20 px one takes a tap 5 px off
    // it, and a finger moving a little within its minimum touch target doesn't cancel; a mouse's is its bounds.
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Box(
            modifier = Modifier.fill_max_size(),
            content = move || {
                let moved = Modifier.layout(|scope, m, c| {
                    let p = m.measure(c);
                    scope.layout(p.width(), p.height(), move |pl| pl.place(&p, 300, 100))
                });
                let l = l.clone();
                Box(modifier = moved.size(20.dp()).pointer_input(0, move |s: PointerInputScope| {
                    let l = l.clone();
                    async move { s.detect_tap_gestures().on_tap(move |_| push(&l, "dokunuş")).await }
                }));
            },
        );
    }));
    tap(&rule, at(325.0, 110.0));
    assert_eq!(taken(&log), ["dokunuş"]);
    down_id(&rule, at(325.0, 110.0), 0);
    rule.advance_frame();
    move_id(&rule, at(327.0, 112.0), 0);
    rule.advance_frame();
    up_id(&rule, at(327.0, 112.0), 0);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["dokunuş"]);
    // A mouse let go a little off it: no tap.
    let h = rule.host();
    h.pointer_down(at(310.0, 110.0), -1, 0, PointerType::Mouse);
    rule.advance_frame();
    h.pointer_move(at(325.0, 110.0), -1, 0);
    rule.advance_frame();
    h.pointer_up(at(325.0, 110.0), -1, 0);
    rule.wait_for_idle();
    assert!(taken(&log).is_empty());
}

fn view_configuration_sets_the_gesture_thresholds() {
    // C++ 8642 (its scrollable2D is the 2D scrolling's, its tooltip stage 12's, its long click the clickable
    // tests'): the host's long press and double tap timeouts, a touch slop a part of the screen provides for its
    // own, and the host's slop and fastest fling in a scroll container.
    let log = new_log();
    let seen = Rc::new(RefCell::new(None::<ViewConfiguration>));
    let inner = Rc::new(RefCell::new(None::<ViewConfiguration>));
    let scroll = ScrollState::new(0);
    let (l, se, inn, sc) = (log.clone(), seen.clone(), inner.clone(), scroll.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *se.borrow_mut() = Some(LocalViewConfiguration.current());
        let (l, inn, sc) = (l.clone(), inn.clone(), sc.clone());
        Column(move || {
            let l2 = l.clone();
            Box(modifier = Modifier.size(100.dp()).pointer_input(0, move |s: PointerInputScope| {
                let (a, b, c) = (l2.clone(), l2.clone(), l2.clone());
                async move {
                    s.detect_tap_gestures()
                        .on_tap(move |_| push(&a, "tap"))
                        .on_double_tap(move |_| push(&b, "double"))
                        .on_long_press(move |_| push(&c, "long"))
                        .await
                }
            }));
            let (l3, inn) = (l.clone(), inn.clone());
            let own = ViewConfiguration { touch_slop: 40.0, ..ViewConfiguration::default() };
            KompozitionLocalProvider(&[LocalViewConfiguration.provides(own)], move || {
                *inn.borrow_mut() = Some(LocalViewConfiguration.current());
                let l3 = l3.clone();
                Box(modifier = Modifier.size(100.dp()).pointer_input(0, move |s: PointerInputScope| {
                    let l = l3.clone();
                    async move { s.detect_drag_gestures(|_, _| {}).on_drag_start(move |_| push(&l, "drag")).await }
                }));
            });
            // 200..400
            Column(
                modifier = Modifier.fill_max_width().height(200.dp()).vertical_scroll(&sc),
                content = || {
                    for _ in 0..100 {
                        Box(modifier = Modifier.fill_max_width().height(50.dp()));
                    }
                },
            );
        });
    }));
    let h = rule.host();
    h.set_view_configuration(ViewConfiguration {
        long_press_timeout: Duration::from_millis(600),
        double_tap_timeout: Duration::from_millis(200),
        touch_slop: 20.0,
        maximum_fling_velocity: 1000.0,
        ..ViewConfiguration::default()
    });
    rule.wait_for_idle();
    let (s, i) = (seen.borrow().unwrap(), inner.borrow().unwrap());
    assert!(s.long_press_timeout_millis() == 600 && s.touch_slop == 20.0);
    assert!(i.touch_slop == 40.0 && i.long_press_timeout_millis() == 400);
    // A long press takes 600 ms.
    down_id(&rule, at(50.0, 50.0), 0);
    rule.main_clock().advance_time_by(500);
    assert!(log.borrow().is_empty());
    rule.main_clock().advance_time_by(150);
    assert_eq!(taken(&log), ["long"]);
    up_id(&rule, at(50.0, 50.0), 0);
    rule.wait_for_idle();
    // A second tap 250 ms on is a tap of its own; 100 ms on, a double tap.
    tap(&rule, at(50.0, 50.0));
    rule.main_clock().advance_time_by(250);
    tap(&rule, at(50.0, 50.0));
    rule.main_clock().advance_time_by(200 + 100); // (the double tap timeout: a delay, not waited for by idling)
    assert_eq!(taken(&log), ["tap", "tap"]);
    tap(&rule, at(50.0, 50.0));
    rule.main_clock().advance_time_by(100);
    tap(&rule, at(50.0, 50.0));
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["double"]);
    // The drag with its own slop: not at 30 px, at 50.
    down_id(&rule, at(10.0, 150.0), 0);
    for x in (15..=40).step_by(5) {
        rule.advance_frame();
        move_id(&rule, at(x as f32, 150.0), 0);
    }
    assert!(log.borrow().is_empty());
    for x in (45..=60).step_by(5) {
        rule.advance_frame();
        move_id(&rule, at(x as f32, 150.0), 0);
    }
    assert_eq!(taken(&log), ["drag"]);
    up_id(&rule, at(60.0, 150.0), 0);
    rule.wait_for_idle();
    // The host's scrolling starts past its 20 px of slop: 5 px at a time, 60 in all, the slop left out (Compose's
    // drag starts with what is past it: C++ counted from the move past it on, 35).
    down_id(&rule, at(200.0, 390.0), 0);
    for i in 1..=12 {
        rule.advance_frame();
        move_id(&rule, at(200.0, 390.0 - 5.0 * i as f32), 0);
    }
    rule.advance_frame(); // (the last delta reaches the scroll in its coroutine)
    assert_eq!(scroll.value(), 60 - 20);
    up_id(&rule, at(200.0, 330.0), 0);
    rule.wait_for_idle();
    let s = scroll.clone();
    kompoz_coroutines::CoroutineScope::new(h.dispatcher()).launch(async move {
        s.scroll_to(0).await;
    });
    rule.wait_for_idle();
    // The host's flings go no faster than 1000 px/s.
    down_id(&rule, at(200.0, 390.0), 0);
    for i in 1..=4 {
        rule.advance_frame();
        move_id(&rule, at(200.0, 390.0 - 40.0 * i as f32), 0);
    }
    up_id(&rule, at(200.0, 230.0), 0);
    h.dispatcher().run_ready(); // (the last delta in: the fling not yet on its way)
    let dragged = scroll.value();
    rule.wait_for_idle();
    let spline = FlingCalculator::new(PlatformFlingScrollFriction, Density::new(1.0, 1.0)).fling_distance(1000.0);
    assert!(
        dragged == 160 - 20 && near((scroll.value() - dragged) as f32, spline, 1.0),
        "{dragged} {} {spline}",
        scroll.value()
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Blocks

fn gestures_built_from_compose_blocks() {
    // C++'s gestures_built_from_compose_blocks.
    let taps = new_log();
    let events = Rc::new(Cell::new(0));
    let downs = new_log();
    let (t, e, d) = (taps.clone(), events.clone(), downs.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (t, e, d) = (t.clone(), e.clone(), d.clone());
        Column(move || {
            // A tap of its own: a press, then its release unless the gesture was taken first.
            let t = t.clone();
            Box(modifier = Modifier.size(100.dp()).pointer_input(0, move |scope: PointerInputScope| {
                let t = t.clone();
                async move {
                    scope
                        .await_each_gesture(move |s| {
                            let t = t.clone();
                            async move {
                                let down = s.await_first_down().await;
                                let up = s.wait_for_up_or_cancellation().await;
                                push(
                                    &t,
                                    match up {
                                        Some(u) => format!(
                                            "dokundu {},{}→{},{}",
                                            down.position.x, down.position.y, u.position.x, u.position.y
                                        ),
                                        None => "iptal".to_string(),
                                    },
                                );
                            }
                        })
                        .await
                }
            }));
            // A gesture done after its first event: the next one waits for the pointers to be up.
            let e = e.clone();
            Box(modifier = Modifier.size(100.dp()).pointer_input(0, move |scope: PointerInputScope| {
                let e = e.clone();
                async move {
                    scope
                        .await_each_gesture(move |s| {
                            let e = e.clone();
                            async move {
                                s.await_pointer_event().await;
                                e.set(e.get() + 1);
                            }
                        })
                        .await
                }
            }));
            // A press taken inside (consumed in the Main pass, innermost first) isn't a first down unless asked.
            let counting = move |label: &'static str, require_unconsumed: bool| {
                let d = d.clone();
                move |scope: PointerInputScope| {
                    let d = d.clone();
                    async move {
                        scope
                            .await_each_gesture(move |s| {
                                let d = d.clone();
                                async move {
                                    s.await_first_down_with(require_unconsumed, PointerEventPass::Main).await;
                                    push(&d, label);
                                    // Its release taken inside too: no up.
                                    let up = s.wait_for_up_or_cancellation().await;
                                    push(&d, if up.is_some() { "bırakıldı" } else { "bırakış alınmış" });
                                }
                            })
                            .await
                    }
                }
            };
            Box(
                modifier = Modifier
                    .size(100.dp())
                    .pointer_input(0, counting("dış", true))
                    .pointer_input(1, counting("dış, alınmış da", false)),
                content = || {
                    Box(modifier = Modifier.size(100.dp()).pointer_input(0, |scope: PointerInputScope| async move {
                        scope
                            .await_pointer_event_scope(|s| async move {
                                loop {
                                    s.await_pointer_event().await.changes.iter().for_each(|c| c.consume());
                                }
                            })
                            .await
                    }));
                },
            );
        });
    }));
    down_id(&rule, at(30.0, 40.0), 0);
    up_id(&rule, at(35.0, 45.0), 0);
    rule.wait_for_idle();
    down_id(&rule, at(50.0, 50.0), 0);
    move_id(&rule, at(50.0, 150.0), 0); // out of the element: taken
    up_id(&rule, at(50.0, 150.0), 0);
    rule.wait_for_idle();
    down_id(&rule, at(60.0, 60.0), 0);
    up_id(&rule, at(60.0, 60.0), 0);
    rule.wait_for_idle();
    assert_eq!(taken(&taps), ["dokundu 30,40→35,45", "iptal", "dokundu 60,60→60,60"]);
    // Pressed, moved three times, let go, pressed again: two gestures, not five.
    down_id(&rule, at(50.0, 150.0), 0);
    for i in 1..=3 {
        move_id(&rule, at(50.0 + i as f32, 150.0), 0);
    }
    up_id(&rule, at(53.0, 150.0), 0);
    down_id(&rule, at(50.0, 150.0), 0);
    up_id(&rule, at(50.0, 150.0), 0);
    rule.wait_for_idle();
    assert_eq!(events.get(), 2);
    down_id(&rule, at(50.0, 250.0), 0);
    up_id(&rule, at(50.0, 250.0), 0);
    rule.wait_for_idle();
    assert_eq!(taken(&downs), ["dış, alınmış da", "bırakış alınmış"]);
    // Two fingers: up once the last one is.
    down_id(&rule, at(20.0, 20.0), 1);
    down_id(&rule, at(80.0, 80.0), 2);
    up_id(&rule, at(20.0, 20.0), 1);
    rule.wait_for_idle();
    assert!(taps.borrow().is_empty());
    up_id(&rule, at(80.0, 80.0), 2);
    rule.wait_for_idle();
    let t = taken(&taps);
    assert!(t.len() == 1 && t[0].starts_with("dokundu 20,20"), "{t:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// Drags

fn a_drag_follows_its_own_finger_and_ends_cancelled_when_taken() {
    // C++'s a_drag_follows_its_own_finger_and_ends_cancelled_when_taken: detect_drag_gestures and
    // detect_drag_gestures_after_long_press; another finger's moves aren't the drag's; a move something takes first
    // (an outer handler, in the Initial pass) calls it off.
    let log = new_log();
    let dragged = Rc::new(Cell::new(0.0f32));
    let takes = Rc::new(Cell::new(false));
    let long_press = mutable_state_of(false);
    let (l, dr, tk, lp) = (log.clone(), dragged.clone(), takes.clone(), long_press.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, dr, tk, lp) = (l.clone(), dr.clone(), tk.clone(), lp.get());
        let outer = move |s: PointerInputScope| {
            let tk = tk.clone();
            async move {
                s.await_pointer_event_scope(|s| async move {
                    loop {
                        let e = s.await_pointer_event_with(PointerEventPass::Initial).await;
                        if tk.get() && e.r#type == PointerEventType::Move {
                            e.changes.iter().for_each(|c| c.consume());
                        }
                    }
                })
                .await
            }
        };
        Box(
            modifier = Modifier.size_wh(300.dp(), 300.dp()).pointer_input(0, outer),
            content = move || {
                let (l, dr) = (l.clone(), dr.clone());
                Box(modifier = Modifier.fill_max_size().pointer_input(lp, move |s: PointerInputScope| {
                    let (a, b, c, dr) = (l.clone(), l.clone(), l.clone(), dr.clone());
                    async move {
                        let on_drag = move |_: &PointerInputChange, d: Offset| dr.set(dr.get() + d.y);
                        if lp {
                            s.detect_drag_gestures_after_long_press(on_drag)
                                .on_drag_start(move |_| push(&a, "start"))
                                .on_drag_end(move || push(&b, "end"))
                                .on_drag_cancel(move || push(&c, "cancel"))
                                .await
                        } else {
                            s.detect_drag_gestures(on_drag)
                                .on_drag_start(move |_| push(&a, "start"))
                                .on_drag_end(move || push(&b, "end"))
                                .on_drag_cancel(move || push(&c, "cancel"))
                                .await
                        }
                    }
                }));
            },
        );
    }));
    down_id(&rule, at(100.0, 200.0), 0);
    for i in 1..=5 {
        rule.advance_frame();
        move_id(&rule, at(100.0, 200.0 - 10.0 * i as f32), 0);
    }
    let before = dragged.get();
    down_id(&rule, at(200.0, 200.0), 1); // a second finger, moving a long way
    for i in 1..=5 {
        rule.advance_frame();
        move_id(&rule, at(200.0, 200.0 + 30.0 * i as f32), 1);
    }
    assert!(*log.borrow() == ["start"] && near(dragged.get(), before, 0.01));
    up_id(&rule, at(200.0, 350.0), 1);
    up_id(&rule, at(100.0, 150.0), 0);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["start", "end"]);
    // Taken half way: cancelled.
    for lp in [false, true] {
        long_press.set(lp);
        takes.set(false);
        rule.wait_for_idle();
        taken(&log);
        down_id(&rule, at(100.0, 200.0), 0);
        if lp {
            rule.main_clock().advance_time_by(600);
        }
        for i in 1..=5 {
            rule.advance_frame();
            move_id(&rule, at(100.0, 200.0 - 10.0 * i as f32), 0);
        }
        takes.set(true);
        move_id(&rule, at(100.0, 140.0), 0);
        rule.advance_frame();
        assert_eq!(taken(&log), ["start", "cancel"], "after a long press: {lp}");
        up_id(&rule, at(100.0, 140.0), 0);
        rule.wait_for_idle();
    }
}

fn drags_after_a_long_press_pick_up_and_follow() {
    // C++ 8880: in a scrolling page.
    let log = new_log();
    let dragged = Rc::new(Cell::new(Offset::Zero));
    let scroll = ScrollState::new(0);
    let (l, dr, sc) = (log.clone(), dragged.clone(), scroll.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, dr) = (l.clone(), dr.clone());
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll(&sc),
            content = move || {
                let (l, dr) = (l.clone(), dr.clone());
                Box(modifier = Modifier.size(100.dp()).pointer_input(0, move |s: PointerInputScope| {
                    let (a, b, c, dr) = (l.clone(), l.clone(), l.clone(), dr.clone());
                    async move {
                        s.detect_drag_gestures_after_long_press(move |_, d| dr.set(dr.get() + d))
                            .on_drag_start(move |p| push(&a, format!("start {},{}", p.x, p.y)))
                            .on_drag_end(move || push(&b, "end"))
                            .on_drag_cancel(move || push(&c, "cancel"))
                            .await
                    }
                }));
                Box(modifier = Modifier.fill_max_width().height(2000.dp()));
            },
        );
    }));
    // Held still, then moved: the whole way (no slop taken off), the page left alone.
    down_id(&rule, at(50.0, 50.0), 0);
    rule.main_clock().advance_time_by(450);
    for i in 1..=5 {
        move_id(&rule, at(50.0, 50.0 + 10.0 * i as f32), 0);
        rule.advance_frame();
    }
    up_id(&rule, at(50.0, 100.0), 0);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["start 50,50", "end"]);
    assert!(near(dragged.get().y, 50.0, 0.01) && scroll.value() == 0);
    // Moved before the long press: no drag; the page scrolls.
    down_id(&rule, at(50.0, 80.0), 0);
    for i in 1..=5 {
        rule.advance_frame();
        move_id(&rule, at(50.0, 80.0 - 12.0 * i as f32), 0);
    }
    up_id(&rule, at(50.0, 20.0), 0);
    rule.wait_for_idle();
    assert!(taken(&log).is_empty() && near(dragged.get().y, 50.0, 0.01) && scroll.value() > 30, "{}", scroll.value());
    let s = scroll.clone();
    kompoz_coroutines::CoroutineScope::new(rule.host().dispatcher()).launch(async move {
        s.scroll_to(0).await;
    });
    rule.wait_for_idle(); // the item back under the finger
    // Let go before the long press: nothing.
    down_id(&rule, at(50.0, 50.0), 0);
    rule.main_clock().advance_time_by(200);
    up_id(&rule, at(50.0, 50.0), 0);
    rule.wait_for_idle();
    assert!(taken(&log).is_empty());
    // Moved sideways past the slop, then held: Compose's long press isn't given up for moving within the element
    // (C++'s was), so the drag starts where the finger is.
    down_id(&rule, at(20.0, 50.0), 0);
    for i in 1..=3 {
        move_id(&rule, at(20.0 + 10.0 * i as f32, 50.0), 0);
        rule.advance_frame();
    }
    rule.main_clock().advance_time_by(500);
    up_id(&rule, at(50.0, 50.0), 0);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["start 50,50", "end"]);
    // Out of the element before the long press: no drag.
    down_id(&rule, at(50.0, 50.0), 0);
    rule.advance_frame();
    move_id(&rule, at(50.0, 150.0), 0);
    rule.main_clock().advance_time_by(500);
    up_id(&rule, at(50.0, 150.0), 0);
    rule.wait_for_idle();
    assert!(taken(&log).is_empty());
}

fn drags_along_an_axis_take_only_their_axis() {
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Row(move || {
            let (lh, lv) = (l.clone(), l.clone());
            Box(modifier = Modifier.size(100.dp()).test_tag("yatay").pointer_input(0, move |s: PointerInputScope| {
                let (a, b) = (lh.clone(), lh.clone());
                async move {
                    s.detect_horizontal_drag_gestures(move |_, dx| push(&a, format!("yatay {dx:.0}")))
                        .on_drag_end(move || push(&b, "yatay bitti"))
                        .await
                }
            }));
            Box(modifier = Modifier.size(100.dp()).test_tag("dikey").pointer_input(0, move |s: PointerInputScope| {
                let (a, b) = (lv.clone(), lv.clone());
                async move {
                    s.detect_vertical_drag_gestures(move |_, dy| push(&a, format!("dikey {dy}")))
                        .on_drag_end(move || push(&b, "dikey bitti"))
                        .await
                }
            }));
        });
    }));
    // Down the horizontal one: no drag.
    rule.on_node_with_tag("yatay").perform_touch_input(|t| t.swipe_with(at(50.0, 10.0), at(50.0, 90.0), 80));
    rule.wait_for_idle();
    assert!(taken(&log).is_empty());
    // Across it: 16 px moves, the first less the 8 px slop.
    rule.on_node_with_tag("yatay").perform_touch_input(|t| t.swipe_with(at(10.0, 50.0), at(90.0, 50.0), 80));
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["yatay 8", "yatay 16", "yatay 16", "yatay 16", "yatay 16", "yatay bitti"]);
    rule.on_node_with_tag("dikey").perform_touch_input(|t| t.swipe_up());
    rule.wait_for_idle();
    let l = taken(&log);
    assert!(l.first().is_some_and(|s| s.starts_with("dikey -")) && l.last().unwrap() == "dikey bitti", "{l:?}");
    let total: f32 = l.iter().filter_map(|s| s.strip_prefix("dikey ")?.parse::<f32>().ok()).sum();
    assert!(near(total, -(99.0 - 8.0), 0.01), "{total}");
}

// ---------------------------------------------------------------------------------------------------------------
// Transforms

fn two_fingers(a0: Offset, a1: Offset, b0: Offset, b1: Offset) -> PointerEvent {
    let change =
        |id: i64, from: Offset, to: Offset| PointerInputChange::new(PointerId(id), 16, to, true, 0, from, true, false);
    PointerEvent::new(vec![change(1, a0, a1), change(2, b0, b1)])
}

fn multi_touch_helpers_measure_zoom_rotation_and_pan() {
    // C++'s multi_touch_helpers_measure_zoom_rotation_and_pan.
    let spread = two_fingers(at(50.0, 0.0), at(0.0, 0.0), at(150.0, 0.0), at(200.0, 0.0)); // 100 apart -> 200
    assert!(near(spread.calculate_zoom() * 100.0, 200.0, 0.01) && near(spread.calculate_rotation(), 0.0, 0.01));
    assert!(near(spread.calculate_pan().x, 0.0, 0.01) && near(spread.calculate_centroid(true).x, 100.0, 0.01));
    let turn = two_fingers(at(-50.0, 0.0), at(0.0, -50.0), at(50.0, 0.0), at(0.0, 50.0)); // a quarter turn clockwise
    assert!(near(turn.calculate_rotation(), 90.0, 0.01) && near(turn.calculate_zoom() * 100.0, 100.0, 0.01));
    let moved = two_fingers(at(0.0, 0.0), at(10.0, 20.0), at(100.0, 0.0), at(110.0, 20.0));
    assert!(near(moved.calculate_pan().x, 10.0, 0.01) && near(moved.calculate_pan().y, 20.0, 0.01));
    assert!(near(moved.calculate_zoom() * 100.0, 100.0, 0.01));
    let mut one = two_fingers(at(0.0, 0.0), at(10.0, 0.0), at(0.0, 0.0), at(0.0, 0.0));
    one.changes.pop();
    assert!(one.calculate_zoom() == 1.0 && one.calculate_rotation() == 0.0 && near(one.calculate_pan().x, 10.0, 0.01));
    assert_eq!(one.calculate_centroid_size(true), 0.0);
    // None pressed both times: no centroid.
    let mut lifted = two_fingers(at(0.0, 0.0), at(10.0, 0.0), at(5.0, 0.0), at(5.0, 0.0));
    lifted.changes.iter_mut().for_each(|c| c.pressed = false);
    assert!(lifted.calculate_centroid(true).is_unspecified() && lifted.calculate_pan() == Offset::Zero);
}

fn detect_transform_gestures_reports_pinch_and_twist() {
    // C++'s detect_transform_gestures_reports_pinch_and_twist.
    let zoom = Rc::new(Cell::new(1.0f32));
    let rotation = Rc::new(Cell::new(0.0f32));
    let (z, r) = (zoom.clone(), rotation.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (z, r) = (z.clone(), r.clone());
        Box(modifier = Modifier.size(400.dp()).pointer_input(0, move |s: PointerInputScope| {
            let (z, r) = (z.clone(), r.clone());
            async move {
                s.detect_transform_gestures(move |_, _, zoom, rotation| {
                    z.set(z.get() * zoom);
                    r.set(r.get() + rotation);
                })
                .await
            }
        }));
    }));
    pinch(&rule, at(150.0, 200.0), at(50.0, 200.0), at(250.0, 200.0), at(350.0, 200.0), 10, true); // 100 apart -> 300
    assert!(zoom.get() > 2.2 && zoom.get() < 3.01 && rotation.get().abs() < 1.0, "{} {}", zoom.get(), rotation.get());
    // A quarter turn clockwise (on screen, y down) around (200, 200), the fingers on a circle.
    zoom.set(1.0);
    let on_circle = |degrees: f32| {
        let a = degrees.to_radians();
        at(200.0 + 100.0 * a.cos(), 200.0 + 100.0 * a.sin())
    };
    down_id(&rule, on_circle(-90.0), 1);
    down_id(&rule, on_circle(90.0), 2);
    for i in 1..=18 {
        rule.advance_frame();
        move_id(&rule, on_circle(-90.0 + 5.0 * i as f32), 1);
        move_id(&rule, on_circle(90.0 + 5.0 * i as f32), 2);
    }
    up_id(&rule, on_circle(0.0), 1);
    up_id(&rule, on_circle(180.0), 2);
    // (less what went into the slop)
    assert!(
        rotation.get() > 80.0 && rotation.get() < 91.0 && (zoom.get() - 1.0).abs() < 0.02,
        "{} {}",
        rotation.get(),
        zoom.get()
    );
}

fn a_pinch_goes_on_as_a_pan_with_the_finger_left() {
    // The transform part of C++'s two_finger_tap_is_no_click_and_the_next_finger_leads (its click part is the
    // clickable tests'):
    // pinched, the first finger lifted, the second one goes on panning.
    let pans = Rc::new(RefCell::new(Vec::<Offset>::new()));
    let p = pans.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let p = p.clone();
        Column(move || {
            Box(modifier = Modifier.size(200.dp()));
            let p = p.clone();
            Box(modifier = Modifier.size(300.dp()).pointer_input(0, move |s: PointerInputScope| {
                let p = p.clone();
                async move { s.detect_transform_gestures(move |_, pan, _, _| p.borrow_mut().push(pan)).await }
            }));
        });
    }));
    pinch(&rule, at(100.0, 350.0), at(80.0, 350.0), at(200.0, 350.0), at(220.0, 350.0), 5, false);
    up_id(&rule, at(80.0, 350.0), 1);
    let before = pans.borrow().len();
    for i in 1..=5 {
        rule.advance_frame();
        move_id(&rule, at(220.0, 350.0 + 10.0 * i as f32), 2);
    }
    up_id(&rule, at(220.0, 400.0), 2);
    rule.advance_frame();
    let down: f32 = pans.borrow()[before..].iter().map(|p| p.y).sum();
    assert!(pans.borrow().len() > before && near(down, 50.0, 0.01), "{down}");
}

fn pan_zoom_lock_keeps_a_pinch_from_turning() {
    let rotation = Rc::new(Cell::new(0.0f32));
    let zoom = Rc::new(Cell::new(1.0f32));
    let (r, z) = (rotation.clone(), zoom.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (r, z) = (r.clone(), z.clone());
        Box(modifier = Modifier.size(400.dp()).test_tag("alan").pointer_input(0, move |s: PointerInputScope| {
            let (r, z) = (r.clone(), z.clone());
            async move {
                s.detect_transform_gestures(move |_, _, zoom, rotation| {
                    z.set(z.get() * zoom);
                    r.set(r.get() + rotation);
                })
                .pan_zoom_lock(true)
                .await
            }
        }));
    }));
    // Spread first (zoom), turning a little as it goes: locked to pan and zoom, no rotation.
    rule.on_node_with_tag("alan").perform_touch_input(|t| {
        t.pinch_with(at(150.0, 200.0), at(40.0, 180.0), at(250.0, 200.0), at(360.0, 220.0), 160);
    });
    assert!(zoom.get() > 2.0 && rotation.get() == 0.0, "{} {}", zoom.get(), rotation.get());
}

// ---------------------------------------------------------------------------------------------------------------
// Nested scrolling with a custom drag

/// Records the pre-scrolls it gets and takes half of each.
struct HalfTaker {
    log: Log,
    used: Rc<Cell<f32>>,
}

impl NestedScrollConnection for HalfTaker {
    fn on_pre_scroll(&self, available: Offset, _: NestedScrollSource) -> Offset {
        push(&self.log, "P.pre");
        self.used.set(self.used.get() + available.y / 2.0);
        Offset::new(0.0, available.y / 2.0)
    }
}

fn nested_scroll_dispatcher_offers_a_custom_drag_to_the_parents() {
    // C++'s nested_scroll_dispatcher_offers_a_custom_drag_to_the_parents.
    let log = new_log();
    let used = Rc::new(Cell::new(0.0f32));
    let moved = Rc::new(Cell::new(0.0f32));
    let parent: Rc<dyn NestedScrollConnection> = Rc::new(HalfTaker { log: log.clone(), used: used.clone() });
    let (pa, mv) = (parent.clone(), moved.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let dispatcher = remember(NestedScrollDispatcher::new);
        let mv = mv.clone();
        Box(
            modifier = Modifier.fill_max_size().nested_scroll(pa.clone()),
            content = move || {
                struct Nothing;
                impl NestedScrollConnection for Nothing {}
                let none: Rc<dyn NestedScrollConnection> = Rc::new(Nothing);
                let (d, mv) = (dispatcher.clone(), mv.clone());
                Box(modifier = Modifier
                    .size(200.dp())
                    .nested_scroll(none, dispatcher = Some(dispatcher.clone()))
                    .pointer_input(0, move |s: PointerInputScope| {
                        let (d, mv) = (d.clone(), mv.clone());
                        async move {
                            s.detect_vertical_drag_gestures(move |c, dy| {
                                c.consume();
                                let used = d.dispatch_pre_scroll(Offset::new(0.0, dy), NestedScrollSource::UserInput);
                                mv.set(mv.get() + dy - used.y);
                            })
                            .await
                        }
                    }));
            },
        );
    }));
    slow_drag(&rule, at(100.0, 50.0), at(100.0, 150.0), 10);
    let l = taken(&log);
    assert!(!l.is_empty() && l[0] == "P.pre" && used.get() > 0.0 && near(moved.get(), used.get(), 0.01));
}

// ---------------------------------------------------------------------------------------------------------------
// Injected touch input

fn touch_input_injected_on_a_node_clicks_double_clicks_long_clicks_and_swipes() {
    let log = new_log();
    let drags = Rc::new(Cell::new(Offset::Zero));
    let (l, dr) = (log.clone(), drags.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, dr) = (l.clone(), dr.clone());
        Column(move || {
            Box(modifier = Modifier.size(100.dp()).test_tag("dokun").pointer_input(0, tapper(&l)));
            let (l, dr) = (l.clone(), dr.clone());
            Box(modifier =
                Modifier.size(100.dp()).test_tag("sürükle").pointer_input(0, move |s: PointerInputScope| {
                    let (a, b, dr) = (l.clone(), l.clone(), dr.clone());
                    async move {
                        s.detect_drag_gestures(move |c, d| {
                            c.consume();
                            dr.set(dr.get() + d);
                        })
                        .on_drag_start(move |p| push(&a, format!("başla {},{}", p.x, p.y)))
                        .on_drag_end(move || push(&b, "bitti"))
                        .await
                    }
                }));
        });
    }));
    let target = rule.on_node_with_tag("dokun");
    target.perform_touch_input(|t| t.click());
    rule.main_clock().advance_time_by(300 + 100); // (the double tap timeout: a delay, not waited for by idling)
    assert_eq!(taken(&log), ["tap 50"]);
    target.perform_touch_input(|t| t.click_with(at(10.0, 10.0)));
    rule.main_clock().advance_time_by(300 + 100); // (the double tap timeout: a delay, not waited for by idling)
    assert_eq!(taken(&log), ["tap 10"]);
    target.perform_touch_input(|t| t.double_click());
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["double"]);
    target.perform_touch_input(|t| t.long_click());
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["long"]);
    // A gesture across two calls: the finger stays down between them.
    target.perform_touch_input(|t| t.down(at(20.0, 20.0)));
    assert!(log.borrow().is_empty());
    target.perform_touch_input(|t| {
        assert_eq!(t.current_position(0), Some(at(20.0, 20.0)));
        t.up();
    });
    rule.main_clock().advance_time_by(300 + 100); // (the double tap timeout: a delay, not waited for by idling)
    assert_eq!(taken(&log), ["tap 20"]);
    // A swipe right across the second one: the drag, less its slop, from where it passed the slop.
    rule.on_node_with_tag("sürükle").perform_touch_input(|t| t.swipe_right());
    rule.wait_for_idle();
    let l = taken(&log);
    assert_eq!(l.len(), 2, "{l:?}");
    assert!(l[0].starts_with("başla") && l[1] == "bitti");
    assert!(near(drags.get().x, 99.0 - 8.0, 0.01) && near(drags.get().y, 0.0, 0.01), "{:?}", drags.get());
    // Cancelled half way: no end.
    rule.on_node_with_tag("sürükle").perform_touch_input(|t| {
        t.down(at(10.0, 50.0));
        t.move_by(at(20.0, 0.0));
        t.move_by(at(20.0, 0.0));
        t.cancel();
    });
    rule.wait_for_idle();
    assert_eq!(taken(&log).len(), 1); // the start only
}

// ---------------------------------------------------------------------------------------------------------------
// Draggable

/// C++'s `rule.swipe`: a finger from `from` to `to`, one move a frame, let go, a frame.
fn swipe(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    down_id(rule, from, 0);
    for i in 1..=frames {
        rule.advance_frame();
        move_id(rule, from + (to - from) * (i as f32 / frames as f32), 0);
    }
    up_id(rule, to, 0);
    rule.advance_frame();
}

/// What `on_drag_stopped` is given: the velocity, set into `velocity`.
fn stopped_into(velocity: Rc<Cell<f32>>) -> OnDragStopped {
    Rc::new(move |_, v| {
        velocity.set(v);
        Box::pin(async {})
    })
}

fn draggable_reports_deltas_with_the_latest_callback() {
    // C++ 14215: a horizontal swipe of 100 px reports 100 less the touch slop (Compose's drag starts past it),
    // stopped once; recomposed with another callback, the next drag goes to that one.
    let total = Rc::new(Cell::new(0.0f32));
    let stops = Rc::new(Cell::new(0));
    let factor = mutable_state_of(1.0f32);
    let (t, st, f) = (total.clone(), stops.clone(), factor.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (t, st) = (t.clone(), st.clone());
        let by = f.get();
        let drag = remember_draggable_state(move |d| t.set(t.get() + d * by));
        let stopped: OnDragStopped = Rc::new(move |_, _| {
            st.set(st.get() + 1);
            Box::pin(async {})
        });
        Box(modifier = named_args!(Modifier.size_wh(200.dp(), 100.dp()).draggable(
            drag,
            Orientation::Horizontal,
            on_drag_stopped = Some(stopped.clone())
        )));
    }));
    let slop = ViewConfiguration::default().touch_slop;
    swipe(&rule, at(20.0, 50.0), at(120.0, 50.0), 10);
    rule.wait_for_idle();
    assert!(near(total.get(), 100.0 - slop, 0.01) && stops.get() == 1, "{} {}", total.get(), stops.get());
    factor.set(2.0);
    rule.wait_for_idle();
    swipe(&rule, at(120.0, 50.0), at(20.0, 50.0), 10);
    rule.wait_for_idle();
    assert!(near(total.get(), (100.0 - slop) - 2.0 * (100.0 - slop), 0.01) && stops.get() == 2, "{}", total.get());
}

fn draggable_flings_a_node_that_moves_with_the_finger() {
    // C++ 14227: the node follows the finger (a frame late), so the pointer hardly moves over it: the drag's
    // travel counts, and the velocity it is let go with. A move that leaves the pointer where the last one left it
    // over the node isn't dispatched to it (Compose's HitPathTracker with its default flags:
    // isTriggerMoveEventsWhenLocationHasNotChangedEnabled off), so of three moves of 20 px the third, the node
    // having caught up, is missed: 20 less the slop, then 20 (C++ counted all 60 less the slop).
    let x = mutable_state_of(0.0f32);
    let velocity = Rc::new(Cell::new(0.0f32));
    let (xs, v) = (x.clone(), velocity.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let xd = xs.clone();
            let drag = remember_draggable_state(move |d| xd.set(xd.peek() + d));
            let xo = xs.clone();
            Box(modifier = named_args!(
                Modifier.offset_px(move |_| IntOffset::new(xo.get().round() as i32, 0)).size(40.dp()).draggable(
                    drag,
                    Orientation::Horizontal,
                    on_drag_stopped = Some(stopped_into(v.clone()))
                )
            ));
        }),
        IntSize::new(400, 400),
    );
    let slop = ViewConfiguration::default().touch_slop;
    swipe(&rule, at(20.0, 20.0), at(80.0, 20.0), 3);
    rule.wait_for_idle();
    assert!(near(x.peek(), 20.0 - slop + 20.0, 0.01) && velocity.get() > 600.0, "{} {}", x.peek(), velocity.get());
    let from = x.peek() + 20.0;
    swipe(&rule, at(from, 20.0), at(from - 60.0, 20.0), 3);
    rule.wait_for_idle();
    assert!(velocity.get() < -600.0, "{}", velocity.get());
}

fn draggable_tells_the_velocity_it_was_let_go_with() {
    // C++ 24105: 200 px in 80 ms is let go fast; held still before letting go, with none; a draggable without a
    // stop callback of its own is stopped too (its drag ends).
    let velocity = Rc::new(Cell::new(-1.0f32));
    let ended = Rc::new(Cell::new(0));
    let (v, e) = (velocity.clone(), ended.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, e) = (v.clone(), e.clone());
        Column(move || {
            let fast = remember_draggable_state(|_| {});
            Box(modifier = named_args!(Modifier.size_wh(300.dp(), 60.dp()).draggable(
                fast,
                Orientation::Horizontal,
                on_drag_stopped = Some(stopped_into(v.clone()))
            ))
            .test_tag("hızlı"));
            let plain = remember_draggable_state(|_| {});
            let e = e.clone();
            let stopped: OnDragStopped = Rc::new(move |_, _| {
                e.set(e.get() + 1);
                Box::pin(async {})
            });
            Box(modifier = named_args!(Modifier.size_wh(300.dp(), 60.dp()).draggable(
                plain,
                Orientation::Vertical,
                on_drag_stopped = Some(stopped.clone())
            ))
            .test_tag("düz"));
        });
    }));
    let b = rule.on_node_with_tag("hızlı").fetch_semantics_node().unclipped_bounds_in_root();
    swipe(&rule, at(b.left + 20.0, b.top + 30.0), at(b.left + 220.0, b.top + 30.0), 5); // 200 px in 80 ms
    rule.wait_for_idle();
    assert!(velocity.get() > 1500.0 && velocity.get() < 4000.0, "{}", velocity.get());
    velocity.set(-1.0);
    down_id(&rule, at(b.left + 200.0, b.top + 30.0), 0); // then held still: none
    for d in [-20.0, -40.0, -60.0] {
        move_id(&rule, at(b.left + 200.0 + d, b.top + 30.0), 0);
    }
    rule.main_clock().advance_time_by(200);
    up_id(&rule, at(b.left + 140.0, b.top + 30.0), 0);
    rule.wait_for_idle();
    assert_eq!(velocity.get(), 0.0);
    let p = rule.on_node_with_tag("düz").fetch_semantics_node().unclipped_bounds_in_root();
    swipe(&rule, at(p.left + 20.0, p.top + 5.0), at(p.left + 20.0, p.top + 55.0), 5);
    rule.wait_for_idle();
    assert_eq!(ended.get(), 1);
}

fn draggable_tells_its_drags() {
    // The draggable part of C++ 23625 (its clickable part is the clickable tests'): a drag is told to the
    // interaction source as it starts and as it stops, and shows as dragged meanwhile.
    let source = MutableInteractionSource::new();
    let log = new_log();
    let dragged = Rc::new(Cell::new(false));
    let moved = Rc::new(Cell::new(0.0f32));
    let (s, l, dr, m) = (source.clone(), log.clone(), dragged.clone(), moved.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let flow = s.interactions();
        let l = l.clone();
        kompoz_coroutines::LaunchedEffect(s.clone(), async move {
            flow.collect(move |i| {
                let name = if i.is::<DragInteraction::Start>() {
                    "DragStart"
                } else if i.is::<DragInteraction::Stop>() {
                    "DragStop"
                } else if i.is::<DragInteraction::Cancel>() {
                    "DragCancel"
                } else {
                    "other"
                };
                push(&l, name);
            })
            .await;
        });
        dr.set(s.collect_is_dragged_as_state().get());
        let m = m.clone();
        let drag = remember_draggable_state(move |d| m.set(m.get() + d));
        Box(modifier = named_args!(Modifier.size(100.dp()).draggable(
            drag,
            Orientation::Horizontal,
            interaction_source = Some(s.clone())
        ))
        .test_tag("sürgü"));
    }));
    let c = at(50.0, 50.0);
    down_id(&rule, c, 0);
    for i in 1..=5 {
        move_id(&rule, at(c.x + 10.0 * i as f32, c.y), 0);
    }
    rule.wait_for_idle();
    assert!(dragged.get() && *log.borrow() == ["DragStart"] && moved.get() > 0.0, "{:?}", log.borrow());
    up_id(&rule, at(c.x + 50.0, c.y), 0);
    rule.wait_for_idle();
    assert!(!dragged.get() && *log.borrow() == ["DragStart", "DragStop"], "{:?}", log.borrow());
}

tests!(
    tap_gestures_tap_double_tap_and_long_press,
    a_tap_after_idling_is_not_a_long_press,
    press_gestures_await_their_release_or_cancellation,
    small_tap_targets_take_touches_near_them_only_for_a_finger,
    view_configuration_sets_the_gesture_thresholds,
    gestures_built_from_compose_blocks,
    a_drag_follows_its_own_finger_and_ends_cancelled_when_taken,
    drags_after_a_long_press_pick_up_and_follow,
    drags_along_an_axis_take_only_their_axis,
    multi_touch_helpers_measure_zoom_rotation_and_pan,
    detect_transform_gestures_reports_pinch_and_twist,
    a_pinch_goes_on_as_a_pan_with_the_finger_left,
    pan_zoom_lock_keeps_a_pinch_from_turning,
    nested_scroll_dispatcher_offers_a_custom_drag_to_the_parents,
    touch_input_injected_on_a_node_clicks_double_clicks_long_clicks_and_swipes,
    draggable_reports_deltas_with_the_latest_callback,
    draggable_flings_a_node_that_moves_with_the_finger,
    draggable_tells_the_velocity_it_was_let_go_with,
    draggable_tells_its_drags,
);
