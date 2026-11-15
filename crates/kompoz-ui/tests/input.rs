//! The pointer input tests: the passes and consumption, hit testing (layers, clips, minimum touch targets,
//! siblings sharing, parents intercepting), `pointer_input` coroutines (started by the first event, restarted on a
//! key change, cancelled with their node, resumed in place), timeouts, several pointers, hover, cancellation,
//! `on_pointer_event`, `pointer_hover_icon`, the velocity tracker and nested scrolling (the C++ Kompoz's tests of
//! them that need no clickable, scrollable or Material, and Compose's behaviour where they differ). Numbers in px.

#![allow(non_snake_case)]

mod common;

use common::*;
use kompoz_coroutines::{CoroutineScope, coroutine_scope, delay_ms};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::input::*;
use kompoz_ui::layout::GraphicsLayerScope;
use kompoz_ui::platform::{PlatformCallbacks, PointerIcon};
use kompoz_ui::unit::Velocity;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

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

fn down(rule: &KompozTestRule, x: f32, y: f32) {
    rule.host().pointer_down(at(x, y), -1, 0, PointerType::Touch);
}

fn move_to(rule: &KompozTestRule, x: f32, y: f32) {
    rule.host().pointer_move(at(x, y), -1, 0);
}

fn up(rule: &KompozTestRule, x: f32, y: f32) {
    rule.host().pointer_up(at(x, y), -1, 0);
}

/// C++'s `tap`: down, a frame, up, a frame.
fn tap(rule: &KompozTestRule, x: f32, y: f32) {
    down(rule, x, y);
    rule.advance_frame();
    up(rule, x, y);
    rule.advance_frame();
}

fn name(t: PointerEventType) -> &'static str {
    match t {
        PointerEventType::Press => "basış",
        PointerEventType::Release => "bırakış",
        PointerEventType::Move => "hareket",
        PointerEventType::Enter => "giriş",
        PointerEventType::Exit => "çıkış",
        PointerEventType::Scroll => "kaydırma",
        _ => "diğer",
    }
}

type Block = Pin<Box<dyn Future<Output = ()>>>;

/// A pointer input block logging each event of `pass` as `label type x,y`.
fn recording(log: Log, label: &'static str, pass: PointerEventPass) -> impl Fn(PointerInputScope) -> Block + 'static {
    move |scope| {
        let log = log.clone();
        Box::pin(async move {
            scope
                .await_pointer_event_scope(|s| async move {
                    loop {
                        let e = s.await_pointer_event_with(pass).await;
                        let p = e.changes[0].position;
                        push(&log, format!("{label} {} {},{}", name(e.r#type), p.x, p.y));
                    }
                })
                .await
        })
    }
}

/// Places what it wraps at (x, y), its own size what that measured (C++'s absolute offset).
fn offset_by(x: i32, y: i32) -> Modifier {
    Modifier.layout(move |scope, m, c| {
        let p = m.measure(c);
        scope.layout(p.width(), p.height(), move |pl| pl.place(&p, x, y))
    })
}

/// Places what it wraps through a layer that `block` sets up.
fn layered(block: impl Fn(&mut GraphicsLayerScope) + 'static) -> Modifier {
    let block: Rc<dyn Fn(&mut GraphicsLayerScope)> = Rc::new(block);
    Modifier.layout(move |scope, m, c| {
        let p = m.measure(c);
        let block = block.clone();
        scope
            .layout(p.width(), p.height(), move |pl| pl.place_with_layer_block(&p, IntOffset::Zero, block.clone(), 0.0))
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Passes, consumption, the coroutines

fn pointer_event_passes_reach_parents_before_and_after_their_children() {
    // C++'s pointer_event_passes_reach_parents_before_and_after_their_children: Initial from the parent in, Main
    // from the child out, Final from the parent in again; a parent consuming in the Initial pass is seen consumed by
    // the child (and its clickable doesn't click); the handler inside a clickable sees each event before it does.
    let log = new_log();
    let block = mutable_state_of(false);
    let child_takes = mutable_state_of(true);
    let parent_takes = mutable_state_of(false);
    let taps = Rc::new(Cell::new(0));
    let clicks = Rc::new(Cell::new(0));
    let (l, b, t, c, cht, pat) =
        (log.clone(), block.clone(), taps.clone(), clicks.clone(), child_takes.clone(), parent_takes.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (pl, pb, cl, ct, cc, cht, pat) =
            (l.clone(), b.clone(), l.clone(), t.clone(), c.clone(), cht.clone(), pat.clone());
        let parent = move |s: PointerInputScope| {
            let (log, block, parent_takes) = (pl.clone(), pb.clone(), pat.clone());
            async move {
                s.await_pointer_event_scope(|s| async move {
                    loop {
                        let e = s.await_pointer_event_with(PointerEventPass::Initial).await;
                        push(&log, format!("ebeveyn önce {}", name(e.r#type)));
                        if block.peek() && e.r#type == PointerEventType::Press {
                            e.changes[0].consume();
                        }
                        let e = s.await_pointer_event().await;
                        push(&log, format!("ebeveyn {}", name(e.r#type)));
                        if parent_takes.peek() && e.r#type == PointerEventType::Release {
                            e.changes[0].consume();
                        }
                        let e = s.await_pointer_event_with(PointerEventPass::Final).await;
                        let taken = if e.changes[0].is_consumed() { " (tüketilmiş)" } else { "" };
                        push(&log, format!("ebeveyn sonra {}{taken}", name(e.r#type)));
                    }
                })
                .await
            }
        };
        let child = move |s: PointerInputScope| {
            let (log, taps, child_takes) = (cl.clone(), ct.clone(), cht.clone());
            async move {
                s.await_pointer_event_scope(|s| async move {
                    loop {
                        let e = s.await_pointer_event().await;
                        let taken = if e.changes[0].is_consumed() { " (tüketilmiş)" } else { "" };
                        push(&log, format!("çocuk {}{taken}", name(e.r#type)));
                        if child_takes.peek() && e.r#type == PointerEventType::Release && !e.changes[0].is_consumed() {
                            taps.set(taps.get() + 1);
                            e.changes[0].consume();
                        }
                        let e = s.await_pointer_event_with(PointerEventPass::Final).await;
                        push(&log, format!("çocuk sonra {}", name(e.r#type)));
                    }
                })
                .await
            }
        };
        Box(
            modifier = Modifier.size(200.dp()).pointer_input(0, parent),
            content = move || {
                let cc = cc.clone();
                let clicked = move || cc.set(cc.get() + 1);
                Box(modifier = Modifier.size(100.dp()).clickable(clicked).pointer_input(0, child.clone()));
            },
        );
    }));
    down(&rule, 50.0, 50.0);
    rule.advance_frame();
    move_to(&rule, 52.0, 50.0);
    rule.advance_frame();
    up(&rule, 52.0, 50.0);
    rule.advance_frame();
    assert_eq!(
        taken(&log),
        [
            "ebeveyn önce basış",
            "çocuk basış",
            "ebeveyn basış",
            "ebeveyn sonra basış (tüketilmiş)",
            "çocuk sonra basış",
            "ebeveyn önce hareket",
            "çocuk hareket",
            "ebeveyn hareket",
            "ebeveyn sonra hareket",
            "çocuk sonra hareket",
            "ebeveyn önce bırakış",
            "çocuk bırakış",
            "ebeveyn bırakış",
            "ebeveyn sonra bırakış (tüketilmiş)",
            "çocuk sonra bırakış",
        ]
    );
    assert!(taps.get() == 1 && clicks.get() == 0); // the child's handler took the release before its clickable
    // The parent takes the press in the Initial pass: the child sees it taken, its clickable doesn't click.
    block.set(true);
    rule.wait_for_idle();
    tap(&rule, 50.0, 50.0);
    rule.wait_for_idle();
    let l = taken(&log);
    assert!(l.len() >= 2 && l[0] == "ebeveyn önce basış" && l[1] == "çocuk basış (tüketilmiş)", "{l:?}");
    assert_eq!(clicks.get(), 0);
    // Nobody inside takes the release: the click comes before the parent's handler does.
    block.set(false);
    child_takes.set(false);
    parent_takes.set(true);
    rule.wait_for_idle();
    tap(&rule, 50.0, 50.0);
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 1);
}

fn pointer_input_restarts_on_key_change_and_stops_when_removed() {
    // C++'s pointer_input_restarts_on_key_change_and_stops_when_removed.
    let key = mutable_state_of(1);
    let shown = mutable_state_of(true);
    let starts = Rc::new(Cell::new(0));
    let alive = Rc::new(Cell::new(0));
    struct Alive(Rc<Cell<i32>>);
    impl Drop for Alive {
        fn drop(&mut self) {
            self.0.set(self.0.get() - 1);
        }
    }
    let (k, sh, st, al) = (key.clone(), shown.clone(), starts.clone(), alive.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        if sh.get() {
            let (st, al) = (st.clone(), al.clone());
            Box(modifier = Modifier.size(100.dp()).pointer_input(k.get(), move |s: PointerInputScope| {
                let (st, al) = (st.clone(), al.clone());
                async move {
                    st.set(st.get() + 1);
                    al.set(al.get() + 1);
                    let _a = Alive(al);
                    s.await_pointer_event_scope(|s| async move {
                        loop {
                            s.await_pointer_event().await;
                        }
                    })
                    .await
                }
            }));
        }
    }));
    assert_eq!(starts.get(), 0); // started by the first event
    tap(&rule, 50.0, 50.0);
    assert_eq!((starts.get(), alive.get()), (1, 1));
    tap(&rule, 50.0, 50.0);
    assert_eq!(starts.get(), 1);
    key.set(2);
    rule.wait_for_idle();
    assert_eq!(alive.get(), 0); // the old block was cancelled
    tap(&rule, 50.0, 50.0);
    assert_eq!((starts.get(), alive.get()), (2, 1));
    shown.set(false);
    rule.wait_for_idle();
    assert_eq!(alive.get(), 0);
}

fn a_new_block_with_the_same_keys_goes_on_until_restarted() {
    // Compose's: a block recomposed with equal keys replaces the running one only for its next start (here the
    // density changing restarts it, as Compose's `onDensityChange`).
    let label = mutable_state_of(1);
    let log = new_log();
    let (lb, l) = (label.clone(), log.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (n, l) = (lb.get(), l.clone());
        Box(modifier = Modifier.size(100.dp()).pointer_input((), move |s: PointerInputScope| {
            let l = l.clone();
            async move {
                s.await_pointer_event_scope(|s| async move {
                    loop {
                        s.await_pointer_event().await;
                        push(&l, format!("blok {n}"));
                    }
                })
                .await
            }
        }));
    }));
    down(&rule, 10.0, 10.0);
    assert_eq!(taken(&log), ["blok 1"]);
    up(&rule, 10.0, 10.0);
    label.set(2);
    rule.wait_for_idle();
    tap(&rule, 10.0, 10.0);
    assert_eq!(taken(&log), ["blok 1", "blok 1", "blok 1"]);
    rule.host().set_density(Density::new(2.0, 1.0));
    rule.wait_for_idle();
    down(&rule, 10.0, 10.0);
    assert_eq!(taken(&log), ["blok 2"]);
    up(&rule, 10.0, 10.0);
}

fn awaiting_blocks_resume_in_place_so_the_next_handler_sees_what_they_consumed() {
    // The child's block runs as its Main pass is dispatched: the parent, after it, sees the change taken; two
    // blocks of one node get the Initial pass oldest first and the Main pass newest first (Compose's).
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (pl, cl) = (l.clone(), l.clone());
        let parent = move |s: PointerInputScope| {
            let log = pl.clone();
            async move {
                s.await_pointer_event_scope(|s| async move {
                    loop {
                        let e = s.await_pointer_event().await;
                        push(&log, format!("ebeveyn {}", e.changes[0].is_consumed()));
                    }
                })
                .await
            }
        };
        let child = move |s: PointerInputScope| {
            let log = cl.clone();
            async move {
                coroutine_scope(move |cs: CoroutineScope| async move {
                    for label in ["eski", "yeni"] {
                        let (s, log) = (s.clone(), log.clone());
                        cs.launch(async move {
                            s.await_pointer_event_scope(|s| async move {
                                loop {
                                    let e = s.await_pointer_event_with(PointerEventPass::Initial).await;
                                    push(&log, format!("{label} önce"));
                                    let e2 = s.await_pointer_event().await;
                                    push(&log, format!("{label} {}", e2.changes[0].is_consumed()));
                                    if label == "eski" {
                                        e.changes[0].consume();
                                    }
                                }
                            })
                            .await
                        });
                    }
                })
                .await
            }
        };
        Box(
            modifier = Modifier.size(200.dp()).pointer_input(0, parent),
            content = move || {
                Box(modifier = Modifier.size(100.dp()).pointer_input(0, child.clone()));
            },
        );
    }));
    down(&rule, 10.0, 10.0);
    assert_eq!(taken(&log), ["eski önce", "yeni önce", "yeni false", "eski false", "ebeveyn true"]);
    up(&rule, 10.0, 10.0);
}

fn with_timeout_gives_up_after_its_time_and_an_event_in_time_comes_through() {
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Box(modifier = Modifier.size(100.dp()).pointer_input(0, move |s: PointerInputScope| {
            let l = l.clone();
            async move {
                s.await_pointer_event_scope(|s| async move {
                    loop {
                        let got = s.with_timeout_or_null(100, |s| async move { s.await_pointer_event().await }).await;
                        push(&l, if got.is_some() { "olay" } else { "süre doldu" });
                        let r = s.with_timeout(0, |s| async move { s.await_pointer_event().await }).await;
                        push(&l, format!("{:?}", r.map(|e| e.r#type)));
                    }
                })
                .await
            }
        }));
    }));
    down(&rule, 10.0, 10.0);
    assert_eq!(taken(&log), ["olay"]);
    move_to(&rule, 20.0, 10.0); // a timeout of 0 still waits its last 8 ms (Compose's two delays): an event
    assert_eq!(taken(&log), ["Ok(Move)"]);
    // 100 ms (92 then 8, each run at the frame due): a timeout; then the one of 0, a frame later.
    rule.main_clock().advance_time_by(150);
    assert_eq!(taken(&log), ["süre doldu", "Err(PointerEventTimeoutCancellation { time_millis: 0 })"]);
    rule.main_clock().advance_time_by(48);
    assert!(taken(&log).is_empty());
    up(&rule, 20.0, 10.0);
    assert_eq!(taken(&log), ["olay"]);
}

fn every_pointer_down_is_in_each_event_and_batched_moves_are_one() {
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Box(modifier = Modifier.size(200.dp()).pointer_input(0, move |s: PointerInputScope| {
            let l = l.clone();
            async move {
                s.await_pointer_event_scope(|s| async move {
                    loop {
                        let e = s.await_pointer_event().await;
                        let changes: Vec<String> = e
                            .changes
                            .iter()
                            .map(|c| {
                                let d = c.position_change();
                                format!("{}{} {},{}", if c.pressed { "+" } else { "-" }, c.position.x, d.x, d.y)
                            })
                            .collect();
                        push(&l, format!("{} [{}]", name(e.r#type), changes.join(" ")));
                    }
                })
                .await
            }
        }));
    }));
    let h = rule.host();
    h.pointer_down(at(10.0, 10.0), -1, 1, PointerType::Touch);
    h.pointer_down(at(50.0, 50.0), -1, 2, PointerType::Touch);
    h.pointer_moves(&[(1, at(20.0, 20.0)), (2, at(60.0, 70.0))], -1);
    h.pointer_up(at(20.0, 20.0), -1, 1);
    h.pointer_up(at(60.0, 70.0), -1, 2);
    assert_eq!(
        taken(&log),
        [
            "basış [+10 0,0]",
            "basış [+10 0,0 +50 0,0]",
            "hareket [+20 10,10 +60 10,20]",
            "bırakış [-20 0,0 +60 0,0]",
            "bırakış [-60 0,0]",
        ]
    );
    // The pointers down, by the host's ids.
    h.pointer_down(at(5.0, 5.0), -1, 7, PointerType::Touch);
    assert_eq!(h.input.borrow().pointers_down(), [(7, at(5.0, 5.0))]);
    h.pointer_up(at(5.0, 5.0), -1, 7);
}

fn a_cancelled_gesture_ends_in_a_release_already_consumed() {
    // Compose's `onCancelPointerInput`: the blocks of the nodes on the hit path get the pointers still down let
    // go, consumed, in the three passes.
    let log = new_log();
    let rule = {
        let l = log.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            Box(modifier = Modifier.size(100.dp()).pointer_input(0, recording(l.clone(), "a", PointerEventPass::Final)));
        }))
    };
    down(&rule, 10.0, 10.0);
    rule.host().pointer_cancel();
    assert_eq!(taken(&log), ["a basış 10,10", "a bırakış 10,10"]);
    // The next press is a gesture of its own.
    tap(&rule, 20.0, 20.0);
    assert_eq!(taken(&log), ["a basış 20,20", "a bırakış 20,20"]);
}

// ---------------------------------------------------------------------------------------------------------------
// Hit testing

/// A node of its own taking pointer events: it logs each pass, shares with its siblings or intercepts its
/// children's out of bounds events if told.
struct RecordingNode {
    log: Log,
    label: &'static str,
    share: bool,
    intercept: bool,
}

impl ModifierNode for RecordingNode {
    node_capabilities!(pointer_input);
}

impl PointerInputModifierNode for RecordingNode {
    fn on_pointer_event(&mut self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        if pass == PointerEventPass::Main {
            let p = event.changes[0].position;
            push(
                &self.log,
                format!("{} {} {},{} {}x{}", self.label, name(event.r#type), p.x, p.y, bounds.width, bounds.height),
            );
        }
    }
    fn on_cancel_pointer_input(&mut self) {
        push(&self.log, format!("{} iptal", self.label));
    }
    fn intercept_out_of_bounds_child_events(&self) -> bool {
        self.intercept
    }
    fn share_pointer_input_with_siblings(&self) -> bool {
        self.share
    }
}

struct RecordingElement {
    log: Log,
    label: &'static str,
    share: bool,
    intercept: bool,
}

impl PartialEq for RecordingElement {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label && self.share == other.share && self.intercept == other.intercept
    }
}

impl ModifierNodeElement for RecordingElement {
    type Node = RecordingNode;
    fn create(&self) -> RecordingNode {
        RecordingNode { log: self.log.clone(), label: self.label, share: self.share, intercept: self.intercept }
    }
    fn update(&self, node: &mut RecordingNode) {
        node.share = self.share;
        node.intercept = self.intercept;
    }
}

fn recorded(log: &Log, label: &'static str) -> Modifier {
    Modifier::element(RecordingElement { log: log.clone(), label, share: false, intercept: false })
}

fn hit_testing_goes_through_layers_and_their_clips() {
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Column(|| {
            let l2 = l.clone();
            // 50 by 50, drawn twice the size about its center and 100 to the right: from 75 to 175 across.
            Box(
                modifier = Modifier.size(200.dp()),
                content = move || {
                    Box(modifier = Modifier
                        .then(layered(|g| {
                            g.scale_x = 2.0;
                            g.scale_y = 2.0;
                            g.translation_x = 100.0;
                        }))
                        .then(recorded(&l2, "dönüşen"))
                        .size(50.dp()));
                },
            );
            let l3 = l.clone();
            // A box clipping its content, a child sticking out of it from (80, 80).
            Box(
                modifier = Modifier.then(layered(|g| g.clip = true)).size(100.dp()),
                content = move || {
                    Box(modifier = offset_by(80, 80).then(recorded(&l3, "taşan")).size(50.dp()));
                },
            );
            let l4 = l.clone();
            // A circle clipping it: its corners aren't it.
            Box(modifier = Modifier
                .then(layered(|g| {
                    g.clip = true;
                    g.shape = std::sync::Arc::new(CircleShape);
                }))
                .then(recorded(&l4, "daire"))
                .size(100.dp()));
        });
    }));
    let h = rule.host();
    let click = |x: f32, y: f32| {
        h.pointer_down(at(x, y), -1, 0, PointerType::Mouse);
        h.pointer_up(at(x, y), -1, 0);
        h.pointer_exit();
        log.borrow_mut().retain(|s| !s.contains("çıkış")); // (the mouse leaving: its hover ends)
    };
    click(150.0, 50.0); // local: ((150 - 125) / 2 + 25, (50 - 25) / 2 + 25)
    assert_eq!(taken(&log), ["dönüşen basış 37.5,37.5 50x50", "dönüşen bırakış 37.5,37.5 50x50"]);
    click(60.0, 10.0); // in its bounds before the layer, not after
    assert!(taken(&log).is_empty());
    click(90.0, 290.0); // in the clipping box (from 200 down): the child sticking out
    assert_eq!(taken(&log)[0], "taşan basış 10,10 50x50");
    click(120.0, 320.0); // the child, but past the clip
    assert!(taken(&log).is_empty());
    click(50.0, 350.0); // the circle's center (from 300 down)
    assert_eq!(taken(&log)[0], "daire basış 50,50 100x100");
    click(5.0, 305.0); // its corner: clipped away
    assert!(taken(&log).is_empty());
}

fn small_targets_take_touches_near_them_the_nearest_first() {
    // C++'s small_touch_targets_take_touches_near_them with pointer input nodes (with its clickables: foundation's
    // clickable tests): the
    // 48 px minimum touch target, the nearer of two, not past it, not for a mouse.
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Box(
            modifier = Modifier.fill_max_size(),
            content = move || {
                Box(modifier = offset_by(100, 100).then(recorded(&l, "a")).size(20.dp())); // its target: 86..134
                Box(modifier = offset_by(140, 100).then(recorded(&l, "b")).size(20.dp())); // 126..174
                Box(modifier = offset_by(0, 300).then(recorded(&l, "geniş")).size_wh(300.dp(), 20.dp()));
                Box(
                    modifier = offset_by(250, 400).then(layered(|g| g.clip = true)).size(100.dp()),
                    content = {
                        let l = l.clone();
                        move || {
                            Box(modifier = recorded(&l, "köşe").size(20.dp()));
                        }
                    },
                );
            },
        );
    }));
    let touch = |x: f32, y: f32, t: PointerType| {
        let h = rule.host();
        h.pointer_down(at(x, y), -1, 0, t);
        h.pointer_up(at(x, y), -1, 0);
        h.pointer_exit();
        taken(&log).first().map(|s| s.split(' ').next().unwrap().to_string())
    };
    let finger = |x, y| touch(x, y, PointerType::Touch);
    assert_eq!(finger(95.0, 110.0).as_deref(), Some("a")); // 5 px off it
    assert_eq!(finger(80.0, 110.0), None); // past its minimum touch target
    assert_eq!(finger(128.0, 110.0).as_deref(), Some("a")); // in both: the nearer
    assert_eq!(finger(132.0, 110.0).as_deref(), Some("b"));
    assert_eq!(finger(150.0, 330.0).as_deref(), Some("geniş")); // 10 px under it
    assert_eq!(finger(310.0, 310.0), None); // past its end: not grown that way
    assert_eq!(finger(255.0, 425.0).as_deref(), Some("köşe"));
    assert_eq!(finger(255.0, 395.0), None); // not from outside the box clipping it
    assert_eq!(touch(95.0, 110.0, PointerType::Mouse), None); // a mouse's is just its bounds
    // The position is local to the target, outside it.
    rule.host().pointer_down(at(95.0, 110.0), -1, 0, PointerType::Touch);
    assert_eq!(taken(&log), ["a basış -5,10 20x20"]);
    rule.host().pointer_up(at(95.0, 110.0), -1, 0);
}

fn siblings_share_only_when_told_and_parents_intercept_out_of_bounds_children() {
    let log = new_log();
    let share = mutable_state_of(false);
    let intercept = mutable_state_of(false);
    let (l, sh, ic) = (log.clone(), share.clone(), intercept.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, share, intercept) = (l.clone(), sh.get(), ic.get());
        Column(|| {
            let l2 = l.clone();
            Box(
                modifier = Modifier.size(100.dp()),
                content = move || {
                    Box(modifier = Modifier.then(recorded(&l2, "alttaki")).fill_max_size());
                    Box(modifier = Modifier
                        .then(Modifier::element(RecordingElement {
                            log: l2.clone(),
                            label: "üstteki",
                            share,
                            intercept: false,
                        }))
                        .fill_max_size());
                },
            );
            let l3 = l.clone();
            Box(
                modifier = Modifier
                    .then(Modifier::element(RecordingElement {
                        log: l3.clone(),
                        label: "ebeveyn",
                        share: false,
                        intercept,
                    }))
                    .size(50.dp()),
                content = move || {
                    Box(modifier = offset_by(60, 0).then(recorded(&l3, "dışarıdaki")).size(50.dp()));
                },
            );
        });
    }));
    let click = |x: f32, y: f32| {
        let h = rule.host();
        h.pointer_down(at(x, y), -1, 0, PointerType::Mouse);
        h.pointer_up(at(x, y), -1, 0);
        h.pointer_exit();
        taken(&log)
            .into_iter()
            .filter(|s| s.contains("basış"))
            .map(|s| s.split(' ').next().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(click(50.0, 50.0), ["üstteki"]);
    assert_eq!(click(80.0, 120.0), ["dışarıdaki"]);
    share.set(true);
    intercept.set(true);
    rule.wait_for_idle();
    // (a sibling shared with is hit as if under the one sharing: Compose's hit path, its Main pass first)
    assert_eq!(click(50.0, 50.0), ["alttaki", "üstteki"]);
    assert_eq!(click(80.0, 120.0), ["dışarıdaki", "ebeveyn"]);
}

fn a_node_leaving_while_a_pointer_is_down_on_it_is_cancelled() {
    // Compose's `detachedListener`: the hit path drops it, and it (and what is under it) gets a cancel.
    let log = new_log();
    let shown = mutable_state_of(true);
    let (l, sh) = (log.clone(), shown.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        if sh.get() {
            Box(
                modifier = recorded(&l, "dış").size(100.dp()),
                content = move || {
                    Box(modifier = recorded(&l, "iç").size(50.dp()));
                },
            );
        }
    }));
    down(&rule, 10.0, 10.0);
    assert_eq!(taken(&log), ["iç basış 10,10 50x50", "dış basış 10,10 100x100"]);
    shown.set(false);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["iç iptal", "dış iptal"]);
    up(&rule, 10.0, 10.0);
    assert!(taken(&log).is_empty());
}

// ---------------------------------------------------------------------------------------------------------------
// Hover, on_pointer_event, the cursor

fn pointer_events_enter_move_exit_press_release_and_scroll() {
    // C++'s pointer_events_enter_move_exit_press_release_and_scroll with Compose's events: the first move over a
    // node is its Enter (no Move after it); the wheel's scroll is told the handler (no list here: scrolling is
    // stage 10's), and taken when it consumes it.
    let log = new_log();
    let take_wheel = mutable_state_of(false);
    let (l, tw) = (log.clone(), take_wheel.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, tw) = (l.clone(), tw.clone());
        let lr = l.clone();
        let record = move |label: &'static str| {
            let l = lr.clone();
            move |_: &AwaitPointerEventScope, e: &PointerEvent| {
                let p = e.changes[0].position;
                let mut entry = format!("{label} {},{}", p.x, p.y);
                if let Some(b) = e.button {
                    entry += &format!(" b{}", b.index());
                }
                if e.keyboard_modifiers.is_ctrl_pressed() {
                    entry += " ctrl";
                }
                push(&l, entry);
            }
        };
        let l2 = l.clone();
        Column(move || {
            Box(modifier = Modifier
                .padding(10.dp())
                .size_wh(100.dp(), 50.dp())
                .on_pointer_event(PointerEventType::Enter, on_event = record("enter"))
                .on_pointer_event(PointerEventType::Exit, on_event = record("exit"))
                .on_pointer_event(PointerEventType::Move, on_event = record("move"))
                .on_pointer_event(PointerEventType::Press, on_event = record("press"))
                .on_pointer_event(PointerEventType::Release, on_event = record("release")));
            let (l2, tw) = (l2.clone(), tw.clone());
            Box(modifier = Modifier.size_wh(100.dp(), 400.dp()).on_pointer_event(
                PointerEventType::Scroll,
                on_event = move |_: &AwaitPointerEventScope, e: &PointerEvent| {
                    push(&l2, format!("scroll d{}", e.changes[0].scroll_delta.y));
                    if tw.peek() {
                        e.changes[0].consume();
                    }
                },
            ));
        });
    }));
    let h = rule.host();
    h.pointer_move(at(20.0, 20.0), -1, 0); // over it: in (local to the box)
    h.pointer_move(at(30.0, 25.0), -1, 0);
    h.key_down(Key(0), KeyModifiers::Ctrl);
    h.pointer_down(at(30.0, 25.0), -1, 0, PointerType::Mouse);
    h.pointer_up(at(30.0, 25.0), -1, 0);
    h.key_up(Key(0), KeyModifiers::None);
    h.mouse_button(at(40.0, 30.0), -1, PointerButton::Tertiary, true);
    h.mouse_button(at(40.0, 30.0), -1, PointerButton::Tertiary, false);
    h.pointer_move(at(300.0, 20.0), -1, 0); // away: out
    h.pointer_move(at(300.0, 30.0), -1, 0);
    assert_eq!(
        taken(&log),
        [
            "enter 10,10",
            "move 20,15",
            "press 20,15 b0 ctrl",
            "release 20,15 b0 ctrl",
            "press 30,20 b2",
            "release 30,20 b2",
            "exit 290,10",
        ]
    );
    h.pointer_move(at(20.0, 20.0), -1, 0);
    h.pointer_exit(); // the mouse left the window
    assert_eq!(taken(&log), ["enter 10,10", "exit 10,10"]);
    // The wheel: the handler hears it (down: positive), and it is taken only when consumed.
    assert!(!h.scroll(at(50.0, 120.0), at(0.0, 1.0)));
    take_wheel.set(true);
    rule.wait_for_idle();
    assert!(h.scroll(at(50.0, 120.0), at(0.0, 1.0)));
    assert_eq!(taken(&log), ["scroll d1", "scroll d1"]);
}

fn mouse_input_injected_on_a_node_hovers_presses_and_scrolls_it() {
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Column(move || {
            Box(modifier = Modifier.size(50.dp()));
            Box(modifier = Modifier
                .size(100.dp())
                .test_tag("hedef")
                .pointer_input(0, recording(l.clone(), "fare", PointerEventPass::Main)));
        });
    }));
    rule.on_node_with_tag("hedef").perform_mouse_input(|m| {
        m.enter_with(Offset::new(10.0, 10.0), 0);
        m.move_to(Offset::new(20.0, 10.0));
        m.click();
        m.scroll(3.0);
        m.exit();
    });
    assert_eq!(
        taken(&log),
        [
            "fare giriş 10,10",
            "fare hareket 20,10",
            "fare basış 50,50",
            "fare bırakış 50,50",
            "fare kaydırma 50,50",
            "fare çıkış 50,50",
        ]
    );
}

/// A platform that writes down the cursors it is told.
struct Cursors(Rc<RefCell<Vec<PointerIcon>>>);

impl PlatformCallbacks for Cursors {
    fn cursor(&self, icon: PointerIcon) {
        self.0.borrow_mut().push(icon);
    }
}

fn pointer_hover_icon_picks_the_innermost_say_unless_overridden() {
    // C++'s pointer_hover_icon_picks_the_innermost_say with Compose's hover icon nodes (Compose's clickables set
    // no hand, unlike C++'s; its text fields' cursors are stage 6's).
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Column(|| {
            Box(
                modifier = Modifier.size_wh(200.dp(), 60.dp()).pointer_hover_icon(PointerIcon::Crosshair),
                content = || {
                    Box(modifier = Modifier.size(40.dp()).pointer_hover_icon(PointerIcon::Hand));
                },
            );
            Box(
                modifier = Modifier
                    .size_wh(200.dp(), 60.dp())
                    .pointer_hover_icon(PointerIcon::Move, override_descendants = true),
                content = || {
                    Box(modifier = Modifier.size(40.dp()).pointer_hover_icon(PointerIcon::Hand));
                },
            );
            Box(modifier = Modifier.size(40.dp()));
        });
    }));
    let cursors = Rc::new(RefCell::new(Vec::new()));
    rule.host().set_callbacks(Rc::new(Cursors(cursors.clone())));
    let over = |x: f32, y: f32| {
        rule.host().pointer_move(at(x, y), -1, 0);
        rule.wait_for_idle();
        *cursors.borrow().last().unwrap()
    };
    assert_eq!(over(150.0, 30.0), PointerIcon::Crosshair);
    assert_eq!(over(20.0, 20.0), PointerIcon::Hand); // a hand of its own inside
    assert_eq!(over(150.0, 30.0), PointerIcon::Crosshair); // back out of it
    assert_eq!(over(20.0, 80.0), PointerIcon::Move); // overridden
    assert_eq!(over(20.0, 140.0), PointerIcon::Default);
}

// ---------------------------------------------------------------------------------------------------------------
// The velocity tracker

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn velocity_tracker_measures_the_last_stretch() {
    // C++'s velocity_tracker_measures_the_last_stretch, with Compose's Lsq2 fit.
    let mut t = VelocityTracker::new();
    for i in 0..=5 {
        t.add_position(1000 + i * 10, at(i as f32 * 10.0, i as f32 * -5.0));
    }
    let v = t.calculate_velocity();
    assert!(near(v.x, 1000.0, 1.0) && near(v.y, -500.0, 1.0), "{v:?}");
    t.add_position(1200, at(50.0, -25.0)); // stood still for a while: nothing
    assert_eq!(t.calculate_velocity(), Velocity::Zero);
    for i in 1..=20 {
        // only the last 100 ms count
        t.add_position(1200 + i * 10, at(50.0 + (i - 10).max(0) as f32 * 20.0, -25.0));
    }
    assert!(near(t.calculate_velocity().x, 2000.0, 5.0));
    t.reset_tracking();
    t.add_position(5, at(1.0, 1.0));
    assert_eq!(t.calculate_velocity().x, 0.0);
    let mut paused = VelocityTracker::new(); // moved, then held 45 ms: the pause ends it
    for i in 0..=5 {
        paused.add_position(i * 10, at(i as f32 * 10.0, 0.0));
    }
    paused.add_position(95, at(50.0, 0.0));
    assert_eq!(paused.calculate_velocity().x, 0.0);
    // At most the maximum, either way.
    let mut fast = VelocityTracker::new();
    for i in 0..=5 {
        fast.add_position(i * 10, at(i as f32 * -100.0, 0.0));
    }
    assert_eq!(fast.calculate_velocity_max(Velocity::new(500.0, 500.0)).x, -500.0);
}

fn velocity_trackers_follow_changes_and_one_dimension_by_impulse() {
    // From a gesture's changes: a press starts anew, a release after a 40 ms pause forgets the motion.
    let change = |time: i64, x: f32, pressed: bool, previous_pressed: bool| {
        PointerInputChange::new(PointerId(0), time, at(x, 0.0), pressed, time - 10, at(x, 0.0), previous_pressed, false)
    };
    let mut t = VelocityTracker::new();
    t.add_position(0, at(500.0, 0.0)); // an earlier gesture's
    t.add_pointer_input_change(&change(100, 0.0, true, false));
    for i in 1..=4 {
        t.add_pointer_input_change(&change(100 + i * 10, i as f32 * 10.0, true, true));
    }
    t.add_pointer_input_change(&change(150, 40.0, false, true)); // let go 10 ms after the last move
    assert!(near(t.calculate_velocity().x, 1000.0, 1.0));
    t.add_pointer_input_change(&change(200, 0.0, true, false));
    t.add_pointer_input_change(&change(210, 10.0, true, true));
    t.add_pointer_input_change(&change(220, 20.0, true, true));
    t.add_pointer_input_change(&change(300, 20.0, false, true)); // 80 ms on
    assert_eq!(t.calculate_velocity().x, 0.0);
    // One dimension: positions or their differences, by the impulse strategy.
    let mut positions = VelocityTracker1D::new(false);
    let mut deltas = VelocityTracker1D::new(true);
    for i in 0..=5 {
        positions.add_data_point(i * 10, i as f32 * 5.0);
        deltas.add_data_point(i * 10, if i == 0 { 0.0 } else { 5.0 });
    }
    assert!(near(positions.calculate_velocity(), 500.0, 1.0), "{}", positions.calculate_velocity());
    assert!(near(deltas.calculate_velocity(), 500.0, 1.0), "{}", deltas.calculate_velocity());
    assert!(near(positions.calculate_velocity_max(100.0), 100.0, 0.001));
}

// ---------------------------------------------------------------------------------------------------------------
// Nested scrolling

/// Writes down the calls it gets; takes half of every pre-scroll, and its pre-fling a frame late, if asked to.
struct RecordingConnection {
    log: Log,
    name: &'static str,
    take_half: bool,
}

impl NestedScrollConnection for RecordingConnection {
    fn on_pre_scroll(&self, available: Offset, source: NestedScrollSource) -> Offset {
        push(&self.log, format!("{}.pre {} {source:?}", self.name, available.y));
        if self.take_half { Offset::new(0.0, available.y / 2.0) } else { Offset::Zero }
    }
    fn on_post_scroll(&self, consumed: Offset, available: Offset, _: NestedScrollSource) -> Offset {
        push(&self.log, format!("{}.post {} {}", self.name, consumed.y, available.y));
        Offset::Zero
    }
    fn on_pre_fling(&self, available: Velocity) -> FlingFuture<'_> {
        Box::pin(async move {
            if self.take_half {
                delay_ms(100).await; // suspends: the fling waits for it
            }
            push(&self.log, format!("{}.preFling {}", self.name, available.y));
            if self.take_half { Velocity::new(0.0, available.y / 2.0) } else { Velocity::Zero }
        })
    }
    fn on_post_fling(&self, consumed: Velocity, available: Velocity) -> FlingFuture<'_> {
        Box::pin(async move {
            push(&self.log, format!("{}.postFling {} {}", self.name, consumed.y, available.y));
            Velocity::Zero
        })
    }
}

fn nested_scroll_connections_pre_outer_first_post_inner_first_and_flings_suspend() {
    // C++'s nested_scroll_connections_pre_outer_first_post_inner_first, the scrolls sent by a dispatcher (the
    // scrollables are stage 10's), and its fling callbacks suspending (the C++ debt fixed).
    let log = new_log();
    let outer: Rc<dyn NestedScrollConnection> =
        Rc::new(RecordingConnection { log: log.clone(), name: "A", take_half: true });
    let inner: Rc<dyn NestedScrollConnection> =
        Rc::new(RecordingConnection { log: log.clone(), name: "B", take_half: false });
    let dispatcher = NestedScrollDispatcher::new();
    let shown = mutable_state_of(true);
    struct Nothing;
    impl NestedScrollConnection for Nothing {}
    let (o, i, d, sh) = (outer.clone(), inner.clone(), dispatcher.clone(), shown.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (i, d, sh) = (i.clone(), d.clone(), sh.clone());
        Box(
            modifier = Modifier.fill_max_size().nested_scroll(o.clone()),
            content = move || {
                let (d, sh) = (d.clone(), sh.clone());
                Box(
                    modifier = Modifier.fill_max_size().nested_scroll(i.clone()),
                    content = move || {
                        if sh.get() {
                            let n: Rc<dyn NestedScrollConnection> = Rc::new(Nothing);
                            Box(modifier = Modifier.size(100.dp()).nested_scroll(n, dispatcher = Some(d.clone())));
                        }
                    },
                );
            },
        );
    }));
    let used = dispatcher.dispatch_pre_scroll(at(0.0, -10.0), NestedScrollSource::UserInput);
    assert_eq!(used, at(0.0, -5.0));
    let left = dispatcher.dispatch_post_scroll(at(0.0, -5.0), at(0.0, -2.0), NestedScrollSource::UserInput);
    assert_eq!(left, Offset::Zero);
    assert_eq!(taken(&log), ["A.pre -10 UserInput", "B.pre -5 UserInput", "B.post -5 -2", "A.post -5 -2"]);
    // A fling: its pre-fling waits for the outer one's 100 ms.
    let result = Rc::new(Cell::new(None::<Velocity>));
    let (r, d) = (result.clone(), dispatcher.clone());
    dispatcher.coroutine_scope().launch(async move {
        let used = d.dispatch_pre_fling(Velocity::new(0.0, -1000.0)).await;
        r.set(Some(used));
        d.dispatch_post_fling(Velocity::new(0.0, -300.0), Velocity::new(0.0, -200.0)).await;
    });
    rule.main_clock().advance_time_by(48);
    assert!(result.get().is_none() && log.borrow().is_empty());
    rule.main_clock().advance_time_by(100);
    assert_eq!(result.get(), Some(Velocity::new(0.0, -500.0)));
    assert_eq!(taken(&log), ["A.preFling -1000", "B.preFling -500", "B.postFling -300 -200", "A.postFling -300 -200"]);
    // Its node gone, a post-fling still goes to the parent it had.
    shown.set(false);
    rule.wait_for_idle();
    assert_eq!(dispatcher.dispatch_pre_scroll(at(0.0, -10.0), NestedScrollSource::UserInput), Offset::Zero);
    let d = dispatcher.clone();
    CoroutineScope::new(rule.host().dispatcher()).launch(async move {
        d.dispatch_post_fling(Velocity::Zero, Velocity::new(0.0, 50.0)).await;
    });
    assert_eq!(taken(&log), ["B.postFling 0 50", "A.postFling 0 50"]);
    assert!(!dispatcher.coroutine_scope().is_active());
}

/// An indirect pointer node logging what reaches it, consuming in the Main pass if it takes them.
struct IndirectNode {
    name: &'static str,
    log: Log,
    takes: bool,
}

impl ModifierNode for IndirectNode {
    node_capabilities!(indirect_pointer_input);
}

impl IndirectPointerInputModifierNode for IndirectNode {
    fn on_indirect_pointer_event(&mut self, event: &IndirectPointerEvent, pass: PointerEventPass) {
        let c = &event.changes[0];
        push(
            &self.log,
            format!("{} {pass:?} {:?} {}>{}", self.name, event.r#type, c.previous_position.x, c.position.x),
        );
        if self.takes && pass == PointerEventPass::Main {
            c.consume();
        }
    }
    fn on_cancel_indirect_pointer_input(&mut self) {
        push(&self.log, format!("{} cancel", self.name));
    }
}

struct IndirectElement {
    name: &'static str,
    log: Log,
    takes: bool,
}

impl PartialEq for IndirectElement {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && Rc::ptr_eq(&self.log, &other.log) && self.takes == other.takes
    }
}

impl ModifierNodeElement for IndirectElement {
    type Node = IndirectNode;
    fn create(&self) -> IndirectNode {
        IndirectNode { name: self.name, log: self.log.clone(), takes: self.takes }
    }
    fn update(&self, _: &mut IndirectNode) {}
}

fn indirect_pointer_events_go_along_the_focused_path() {
    // Compose's dispatchIndirectPointerEvent: the nearest indirect pointer node at or above the focused target and
    // those above it, the Initial pass down, the Main pass up, the Final pass down; consumption is the answer. A
    // cancel goes from it up; the focus leaving cancels the stream of what it left.
    let log = new_log();
    let focus = kompoz_ui::focus::FocusRequester::new();
    let (l, f) = (log.clone(), focus.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, f) = (l.clone(), f.clone());
        Box(
            Modifier.then_element(IndirectElement { name: "dış", log: l.clone(), takes: false }),
            content = move || {
                Box(Modifier
                    .then_element(IndirectElement { name: "iç", log: l.clone(), takes: true })
                    .focus_requester(&f)
                    .focus_target()
                    .size(20.dp()));
                Box(Modifier.focus_target().size(20.dp()).test_tag("öbür"));
            },
        );
    }));
    use IndirectPointerEventPrimaryDirectionalMotionAxis::X;
    let send = |t, x: f32| rule.host().indirect_pointer(t, 0, Offset::new(x, 0.0), 1.0, -1, X);
    // Nothing focused: nowhere to go.
    assert!(!send(IndirectPointerEventType::Press, 5.0));
    assert!(taken(&log).is_empty());
    focus.request_focus();
    rule.wait_for_idle();
    assert!(send(IndirectPointerEventType::Press, 10.0));
    assert_eq!(
        taken(&log),
        [
            "dış Initial Press 10>10",
            "iç Initial Press 10>10",
            "iç Main Press 10>10",
            "dış Main Press 10>10",
            "dış Final Press 10>10",
            "iç Final Press 10>10"
        ]
    );
    // A move comes from where the press was.
    assert!(send(IndirectPointerEventType::Move, 30.0));
    assert!(taken(&log).iter().all(|l| l.ends_with("Move 10>30")));
    rule.host().indirect_pointer_cancel();
    assert_eq!(taken(&log), ["iç cancel", "dış cancel"]);
    // The focus moves elsewhere: the stream of what it left cancelled.
    send(IndirectPointerEventType::Press, 0.0);
    taken(&log);
    rule.host().move_focus(kompoz_ui::focus::FocusDirection::Next);
    rule.wait_for_idle();
    assert!(taken(&log).contains(&"iç cancel".to_string()));
}

tests!(
    indirect_pointer_events_go_along_the_focused_path,
    pointer_event_passes_reach_parents_before_and_after_their_children,
    pointer_input_restarts_on_key_change_and_stops_when_removed,
    a_new_block_with_the_same_keys_goes_on_until_restarted,
    awaiting_blocks_resume_in_place_so_the_next_handler_sees_what_they_consumed,
    with_timeout_gives_up_after_its_time_and_an_event_in_time_comes_through,
    every_pointer_down_is_in_each_event_and_batched_moves_are_one,
    a_cancelled_gesture_ends_in_a_release_already_consumed,
    hit_testing_goes_through_layers_and_their_clips,
    small_targets_take_touches_near_them_the_nearest_first,
    siblings_share_only_when_told_and_parents_intercept_out_of_bounds_children,
    a_node_leaving_while_a_pointer_is_down_on_it_is_cancelled,
    pointer_events_enter_move_exit_press_release_and_scroll,
    mouse_input_injected_on_a_node_hovers_presses_and_scrolls_it,
    pointer_hover_icon_picks_the_innermost_say_unless_overridden,
    velocity_tracker_measures_the_last_stretch,
    velocity_trackers_follow_changes_and_one_dimension_by_impulse,
    nested_scroll_connections_pre_outer_first_post_inner_first_and_flings_suspend,
);
