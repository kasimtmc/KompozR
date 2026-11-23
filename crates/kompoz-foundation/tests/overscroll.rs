//! Overscroll: Android's stretch (EdgeEffect's physics, drawn as a layer's stretch) at a scroll container's ends,
//! `LocalOverscrollFactory`, effects of one's own (`Modifier.overscroll`, `scrollable_area`'s, shared, halved) — the
//! C++ Kompoz's overscroll tests (survey C) and Compose's behaviour where the two differ. Numbers in px (density 1).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_foundation::gestures::LocalFuture;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode};
use kompoz_ui::input::*;
use kompoz_ui::layout::LayoutAwareModifierNode;
use kompoz_ui::modifier::DelegatableNode;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::unit::Velocity;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).fetch_semantics_node().unclipped_bounds_in_root()
}

fn same_rect(a: Rect, b: Rect) -> bool {
    near(a.left, b.left, 0.01)
        && near(a.top, b.top, 0.01)
        && near(a.right, b.right, 0.01)
        && near(a.bottom, b.bottom, 0.01)
}

/// The stretched layers drawn now: each one's bounds and its stretch's direction (a fraction of its size each way:
/// positive from the top or left, negative from the bottom or right).
fn stretches(rule: &KompozTestRule) -> Vec<(Rect, Offset)> {
    let canvas = rule.draw();
    canvas
        .layer_stretches
        .iter()
        .zip(&canvas.layer_bounds)
        .filter_map(|(s, b)| s.filter(|s| !s.is_empty()).map(|s| (*b, s.stretch_direction())))
        .collect()
}

/// Waits for idle, then lets frames run until no stretch is drawn: a stretch springs back by its draws asking for
/// the next frame (Android's EdgeEffect animating as it is drawn), which isn't work idling waits for (Compose's
/// idling resource counts recompositions, frame awaiters and layouts, not a redraw), so the clock is advanced to
/// it (Compose's `advanceTimeUntil`).
fn spring_back(rule: &KompozTestRule) {
    rule.wait_for_idle();
    named_args!(
        rule.main_clock().advance_time_until(timeout_millis = 3_000, condition = || stretches(rule).is_empty())
    );
}

/// How far the node tagged `tag` is stretched now (C++'s `stretchOf`): the direction of the stretched layer drawn
/// over it (hwui's stretch vector: EdgeEffect's damped one, about a sixtieth for a pull of a quarter of the
/// height); none: zero.
fn stretch_of(rule: &KompozTestRule, tag: &str) -> Offset {
    let center = bounds(rule, tag).center();
    stretches(rule).into_iter().find(|(b, _)| b.contains(center)).map_or(Offset::Zero, |(_, d)| d)
}

/// A finger (or `type`) down at `from`, moved by `by` over `steps` frames, held there.
fn drag(rule: &KompozTestRule, from: Offset, by: Offset, r#type: PointerType, steps: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, r#type);
    for i in 1..=steps {
        rule.advance_frame();
        h.pointer_move(from + by * (i as f32 / steps as f32), -1, 0);
    }
}

/// C++'s `scrollTo`, run until idle.
fn scroll_to(rule: &KompozTestRule, state: &ScrollState, value: i32) {
    let s = state.clone();
    kompoz_coroutines::CoroutineScope::new(rule.host().dispatcher()).launch(async move {
        s.scroll_to(value).await;
    });
    rule.wait_for_idle();
}

/// A column of 30 boxes 50 px tall, as wide as it is.
#[kompozable]
fn Items() {
    for _ in 0..30 {
        Box(modifier = Modifier.fill_max_width().height(50.dp()));
    }
}

// ---- An effect of one's own ----------------------------------------------------------------------------------

/// What a [`RecordingOverscroll`] heard: what the scrolls left (from the user), each fling's velocity and what
/// flinging left of it, its draws (where, how big it was measured).
#[derive(Default)]
struct Recorded {
    pulled: Cell<f32>,
    flings: RefCell<Vec<f32>>,
    lefts: RefCell<Vec<f32>>,
    draws: Cell<i32>,
    drawn_at: Cell<Rect>,
    size: Cell<IntSize>,
}

/// C++'s RecordingOverscroll: adds up what the scrolls left, hears of the flings, draws the content as it is.
struct RecordingOverscroll {
    recorded: Rc<Recorded>,
    node: DelegatableNode,
}

impl RecordingOverscroll {
    fn made() -> (Rc<dyn OverscrollEffect>, Rc<Recorded>) {
        let recorded = Rc::new(Recorded::default());
        let node = DelegatableNode::new(RecordingNode(recorded.clone()));
        (Rc::new(RecordingOverscroll { recorded: recorded.clone(), node }), recorded)
    }
}

impl PartialEq for RecordingOverscroll {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.recorded, &other.recorded)
    }
}

impl OverscrollEffect for RecordingOverscroll {
    fn apply_to_scroll(
        &self,
        delta: Offset,
        source: NestedScrollSource,
        perform_scroll: &dyn Fn(Offset) -> Offset,
    ) -> Offset {
        let used = perform_scroll(delta);
        if source == NestedScrollSource::UserInput {
            let r = &self.recorded;
            r.pulled.set(r.pulled.get() + delta.x - used.x + delta.y - used.y);
        }
        used
    }

    fn apply_to_fling(&self, velocity: Velocity, perform_fling: PerformFling) -> LocalFuture<'_, ()> {
        let r = self.recorded.clone();
        Box::pin(async move {
            r.flings.borrow_mut().push(velocity.x + velocity.y);
            let used = perform_fling(velocity).await;
            r.lefts.borrow_mut().push(velocity.x - used.x + velocity.y - used.y);
            r.pulled.set(0.0);
        })
    }

    fn is_in_progress(&self) -> bool {
        self.recorded.pulled.get() != 0.0
    }

    fn node(&self) -> DelegatableNode {
        self.node.clone()
    }
}

/// Its node: counts its draws, knows its size.
struct RecordingNode(Rc<Recorded>);

impl ModifierNode for RecordingNode {
    node_capabilities!(draw, layout_aware);
}

impl DrawModifierNode for RecordingNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        self.0.draws.set(self.0.draws.get() + 1);
        self.0.drawn_at.set(Rect::from_offset_size(Offset::Zero, scope.size()));
        scope.draw_content();
    }
}

impl LayoutAwareModifierNode for RecordingNode {
    fn on_remeasured(&mut self, size: IntSize) {
        self.0.size.set(size);
    }
}

// ---- The tests -----------------------------------------------------------------------------------------------

fn local_overscroll_factory_turns_the_stretch_off() {
    // C++ 3885's scroll container part (its LazyColumn is the lazy lists'): no factory, no stretch.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Row(|| {
                let plain = remember_scroll_state();
                Column(
                    modifier = Modifier.weight(1.0).fill_max_height().vertical_scroll(&plain).test_tag("açık"),
                    content = || Items(),
                );
                let none: Option<Rc<dyn OverscrollFactory>> = None;
                KompozitionLocalProvider(&[LocalOverscrollFactory.provides(none)], || {
                    let column = remember_scroll_state();
                    Column(
                        modifier = Modifier.weight(1.0).fill_max_height().vertical_scroll(&column).test_tag("kapalı"),
                        content = || Items(),
                    );
                });
            });
        }),
        IntSize::new(600, 800),
    );
    // Down at the top, held.
    let pull = |tag: &str| {
        let x = bounds(&rule, tag).center().x;
        drag(&rule, at(x, 300.0), at(0.0, 200.0), PointerType::Touch, 10);
        let stretched = stretch_of(&rule, tag);
        rule.host().pointer_up(at(x, 500.0), -1, 0);
        rule.wait_for_idle();
        stretched
    };
    let on = pull("açık");
    assert!(on.y > 0.005 && on.x == 0.0, "{on:?}"); // the usual stretch, from the top
    assert_eq!(pull("kapalı"), Offset::Zero); // none where it is turned off
}

fn scrollable_area_scrolls_clips_and_draws_its_overscroll() {
    // C++ 3916: a layout placing its content by the state itself, 500 to scroll; past its start it stretches (a
    // bare scrollable doesn't); clipped along it with 30 more each side across (the bare one isn't; the disabled
    // one still is); an effect of its own drawn over its content; disabled, not dragged; right to left, a drag
    // right scrolls on. (Compose's scrollableArea gives the state scroll deltas, positive as the content goes on:
    // C++ gave the finger's.)
    let pos = mutable_state_of(0.0f32);
    let across = mutable_state_of(0.0f32);
    let still_pos = mutable_state_of(0.0f32);
    let own_pos = mutable_state_of(0.0f32);
    let consume = |p: MutableState<f32>| {
        move |delta: f32| {
            let next = (p.peek() + delta).clamp(0.0, 500.0);
            let used = next - p.peek();
            p.set(next);
            used
        }
    };
    let state = ScrollableState(consume(pos.clone()));
    let row = ScrollableState(consume(across.clone()));
    let still = ScrollableState(consume(still_pos.clone()));
    let own = ScrollableState(consume(own_pos.clone()));
    let (counting, recorded) = RecordingOverscroll::made();
    let rtl = mutable_state_of(false);
    let (p, r) = (pos.clone(), rtl.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (p, r) = (p.clone(), r.clone());
        let (state, row, still, own, counting) =
            (state.clone(), row.clone(), still.clone(), own.clone(), counting.clone());
        Column(move || {
            let p = p.clone();
            Box(
                modifier =
                    Modifier.size(200.dp()).scrollable_area(state.clone(), Orientation::Vertical).test_tag("alan"),
                content = move || {
                    let y = -p.get();
                    Box(modifier = Modifier.offset(0.dp(), y.dp()).size_wh(200.dp(), 700.dp()));
                },
            );
            Box(modifier = named_args!(Modifier.size_wh(200.dp(), 50.dp()).scrollable_area(
                still.clone(),
                Orientation::Vertical,
                enabled = false
            ))
            .test_tag("kapalı"));
            Box(modifier =
                Modifier.size_wh(200.dp(), 50.dp()).scrollable(own.clone(), Orientation::Vertical).test_tag("çıplak"));
            Box(
                modifier = Modifier
                    .size_wh(200.dp(), 50.dp())
                    .scrollable_area_with_overscroll(own.clone(), Orientation::Vertical, Some(counting.clone()))
                    .test_tag("efekt"),
                content = || Text("e"),
            );
            let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
            let row = row.clone();
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], move || {
                Box(modifier = Modifier
                    .size_wh(200.dp(), 50.dp())
                    .scrollable_area(row.clone(), Orientation::Horizontal)
                    .test_tag("satır"));
            });
        });
    }));
    let pull = |tag: &str, by: Offset| {
        let c = bounds(&rule, tag).center();
        drag(&rule, c, by, PointerType::Touch, 10);
    };
    let release = |tag: &str, by: Offset| {
        let c = bounds(&rule, tag).center();
        rule.host().pointer_up(c + by, -1, 0);
        rule.wait_for_idle();
    };
    // Dragged up: on through the content.
    pull("alan", at(0.0, -100.0));
    release("alan", at(0.0, -100.0));
    assert!(pos.peek() > 60.0, "{}", pos.peek());
    // Pulled down past its start: its stretch (a bare scrollable has none).
    pos.set(0.0);
    rule.wait_for_idle();
    pull("alan", at(0.0, 80.0));
    let stretched = stretch_of(&rule, "alan");
    release("alan", at(0.0, 80.0));
    pull("çıplak", at(0.0, 80.0));
    let bare = stretch_of(&rule, "çıplak");
    release("çıplak", at(0.0, 80.0));
    assert!(stretched.y > 0.005 && bare == Offset::Zero, "{stretched:?} {bare:?}");
    // Clipped along it, 30 more each side across.
    let canvas = rule.draw();
    let clipped = |tag: &str| {
        let r = bounds(&rule, tag);
        let want = Rect::new(r.left - 30.0, r.top, r.right + 30.0, r.bottom);
        canvas.clips.iter().any(|c| same_rect(*c, want))
    };
    assert!(clipped("alan") && clipped("kapalı") && !clipped("çıplak"), "{:?}", canvas.clips);
    // Its own effect drawn over its content.
    assert!(recorded.draws.get() > 0);
    // Disabled: not dragged.
    pull("kapalı", at(0.0, -40.0));
    release("kapalı", at(0.0, -40.0));
    assert_eq!(still_pos.peek(), 0.0);
    // Horizontal, right to left: dragged right, on through it.
    rtl.set(true);
    rule.wait_for_idle();
    pull("satır", at(100.0, 0.0));
    release("satır", at(100.0, 0.0));
    assert!(across.peek() > 60.0, "{}", across.peek());
}

fn overscroll_stretches_past_the_ends_and_springs_back() {
    // C++ 4033: a finger pulling down at the top stretches the content from the top edge; back up a little, the
    // stretch goes first; let go, it springs back. The mouse doesn't stretch it (it doesn't drag a scroll
    // container at all, as Compose's). At the end a pull up stretches it from the bottom; a fling that hits the
    // end bumps it.
    let scroll = ScrollState::new(0);
    let s = scroll.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Column(modifier = Modifier.fill_max_size().vertical_scroll(&s).test_tag("list"), content = || Items());
    }));
    let h = rule.host();
    drag(&rule, at(200.0, 300.0), at(0.0, 200.0), PointerType::Touch, 10);
    let pulled = stretch_of(&rule, "list");
    assert!(pulled.y > 0.005 && pulled.x == 0.0 && scroll.value() == 0, "{pulled:?} {}", scroll.value());
    // Back up a little: the stretch goes first, the content stays.
    for i in 1..=4 {
        rule.advance_frame();
        h.pointer_move(at(200.0, 500.0 - 10.0 * i as f32), -1, 0);
    }
    let back = stretch_of(&rule, "list");
    assert!(back.y < pulled.y && back.y > 0.0 && scroll.value() == 0, "{back:?} {}", scroll.value());
    // Held there, let go: it springs back. (Let go moving up, a fling going further than the stretch would take it
    // back and go on through the content: Compose's absorbToRelaxIfNeeded; C++ always sprang back.)
    for _ in 0..10 {
        rule.advance_frame();
    }
    h.pointer_up(at(200.0, 460.0), -1, 0);
    spring_back(&rule);
    assert!(stretches(&rule).is_empty() && scroll.value() == 0);
    // The mouse doesn't stretch it.
    drag(&rule, at(200.0, 300.0), at(0.0, 200.0), PointerType::Mouse, 10);
    assert!(stretches(&rule).is_empty());
    h.pointer_up(at(200.0, 500.0), -1, 0);
    rule.wait_for_idle();
    // At the end, a pull up stretches it from the bottom.
    scroll_to(&rule, &scroll, scroll.max_value());
    drag(&rule, at(200.0, 600.0), at(0.0, -200.0), PointerType::Touch, 10);
    let end = stretch_of(&rule, "list");
    assert!(end.y < -0.005, "{end:?}");
    h.pointer_up(at(200.0, 400.0), -1, 0);
    spring_back(&rule);
    assert!(stretches(&rule).is_empty());
    // A fling that hits the end bumps it.
    scroll_to(&rule, &scroll, scroll.max_value() - 300);
    h.pointer_down(at(200.0, 600.0), -1, 0, PointerType::Touch);
    for i in 1..=3 {
        rule.advance_frame();
        h.pointer_move(at(200.0, 600.0 - 60.0 * i as f32), -1, 0);
    }
    h.pointer_up(at(200.0, 420.0), -1, 0);
    let mut deepest = 0.0f32;
    for _ in 0..40 {
        rule.advance_frame();
        deepest = deepest.min(stretch_of(&rule, "list").y);
    }
    assert!(scroll.value() == scroll.max_value() && deepest < -0.005, "{} {deepest}", scroll.value());
    spring_back(&rule);
    assert!(stretches(&rule).is_empty() && !h.has_pending_work());
}

fn overscroll_effect_takes_the_touch_drags_and_flings_past_the_ends() {
    // C++ 4130: an effect of one's own gets what the touch drags and flings leave at the ends (sized and drawn where
    // the column is; no stretch of the container's own); the mouse goes past it.
    let scroll = ScrollState::new(0);
    let (effect, recorded) = RecordingOverscroll::made();
    let (s, e) = (scroll.clone(), effect.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll_with_overscroll(&s, Some(e.clone())).test_tag("list"),
            content = || Items(),
        );
    }));
    let h = rule.host();
    assert_eq!(recorded.size.get(), IntSize::new(400, 800)); // its size, where it is drawn
    // Down at the top: nothing scrolls, the effect gets it (past the touch slop).
    drag(&rule, at(200.0, 300.0), at(0.0, 200.0), PointerType::Touch, 10);
    assert!(recorded.pulled.get() > 150.0 && scroll.value() == 0, "{}", recorded.pulled.get());
    assert!(stretches(&rule).is_empty()); // no stretch of its own
    let draws = recorded.draws.get();
    rule.draw();
    assert!(recorded.draws.get() == draws + 1 && same_rect(recorded.drawn_at.get(), Rect::new(0.0, 0.0, 400.0, 800.0)));
    // Let go moving down: the fling has nowhere to go, all of it is left for the effect.
    h.pointer_up(at(200.0, 500.0), -1, 0);
    rule.wait_for_idle();
    {
        let (flings, lefts) = (recorded.flings.borrow(), recorded.lefts.borrow());
        assert!(flings.len() == 1 && flings[0] > 1000.0 && lefts.len() == 1, "{flings:?} {lefts:?}");
        assert!(lefts[0] > 0.8 * flings[0] && recorded.pulled.get() == 0.0, "{flings:?} {lefts:?}");
    }
    // A fling up from the middle goes to the end, and what it had left there comes back.
    scroll_to(&rule, &scroll, scroll.max_value() - 300);
    drag(&rule, at(200.0, 600.0), at(0.0, -180.0), PointerType::Touch, 3);
    h.pointer_up(at(200.0, 420.0), -1, 0);
    rule.wait_for_idle();
    {
        let (flings, lefts) = (recorded.flings.borrow(), recorded.lefts.borrow());
        assert!(scroll.value() == scroll.max_value() && flings.len() == 2 && flings[1] < -1000.0, "{flings:?}");
        assert!(lefts.len() == 2 && lefts[1] < 0.0 && lefts[1] > flings[1], "{flings:?} {lefts:?}");
    }
    // Held still in the middle: let go with nothing to fling, nothing left.
    scroll_to(&rule, &scroll, 300);
    drag(&rule, at(200.0, 400.0), at(0.0, 100.0), PointerType::Touch, 10);
    for _ in 0..10 {
        rule.advance_frame();
    }
    h.pointer_up(at(200.0, 500.0), -1, 0);
    rule.wait_for_idle();
    {
        let (flings, lefts) = (recorded.flings.borrow(), recorded.lefts.borrow());
        assert!(
            flings.len() == 3 && flings[2] == 0.0 && lefts[2] == 0.0 && recorded.pulled.get() == 0.0,
            "{flings:?} {lefts:?}"
        );
    }
    // Let go slowly in the middle (too slow to fling): nothing hit an end, nothing left.
    h.pointer_down(at(200.0, 400.0), -1, 0, PointerType::Touch);
    for i in 1..=40 {
        rule.advance_frame();
        h.pointer_move(at(200.0, 400.0 + 1.5 * i as f32), -1, 0); // ~94 px/s
    }
    h.pointer_up(at(200.0, 460.0), -1, 0);
    rule.wait_for_idle();
    {
        let (flings, lefts) = (recorded.flings.borrow(), recorded.lefts.borrow());
        assert!(flings.len() == 4 && flings[3].abs() > 50.0 && lefts[3] == 0.0, "{flings:?} {lefts:?}");
    }
    // A finger on a fling stops it: the fling is cancelled with what applied it (the drag's scroll takes the
    // MutatorMutex from it, as Compose's), so the effect hears nothing more of it (C++ told it the fling ended
    // with nothing left).
    scroll_to(&rule, &scroll, 300);
    drag(&rule, at(200.0, 600.0), at(0.0, -180.0), PointerType::Touch, 3);
    h.pointer_up(at(200.0, 420.0), -1, 0);
    rule.advance_frame();
    rule.advance_frame();
    assert_eq!(recorded.lefts.borrow().len(), 4); // still flinging
    h.pointer_down(at(200.0, 400.0), -1, 0, PointerType::Touch);
    rule.advance_frame();
    let stopped = scroll.value();
    for _ in 0..5 {
        rule.advance_frame();
    }
    assert!(scroll.value() == stopped && recorded.lefts.borrow().len() == 4, "{} {stopped}", scroll.value());
    h.pointer_up(at(200.0, 400.0), -1, 0);
    rule.wait_for_idle();
    // The mouse goes past it.
    scroll_to(&rule, &scroll, 0);
    let flings = recorded.flings.borrow().len();
    drag(&rule, at(200.0, 300.0), at(0.0, 200.0), PointerType::Mouse, 10);
    h.pointer_up(at(200.0, 500.0), -1, 0);
    rule.wait_for_idle();
    assert!(recorded.pulled.get() == 0.0 && recorded.flings.borrow().len() == flings);
}

/// A nested scroll parent taking every fling left to it.
struct EatsFlings;

impl NestedScrollConnection for EatsFlings {
    fn on_post_fling(&self, _consumed: Velocity, available: Velocity) -> FlingFuture<'_> {
        Box::pin(async move { available })
    }
}

fn overscroll_effect_gets_what_the_nested_scroll_parents_leave() {
    // C++ 4206: the parent took the whole fling: nothing left for the effect.
    let (effect, recorded) = RecordingOverscroll::made();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let e = effect.clone();
        Box(
            modifier = Modifier.fill_max_size().nested_scroll(Rc::new(EatsFlings)),
            content = move || {
                let scroll = remember_scroll_state();
                Column(
                    modifier = Modifier.fill_max_size().vertical_scroll_with_overscroll(&scroll, Some(e.clone())),
                    content = || Items(),
                );
            },
        );
    }));
    drag(&rule, at(200.0, 300.0), at(0.0, 200.0), PointerType::Touch, 10);
    rule.host().pointer_up(at(200.0, 500.0), -1, 0);
    rule.wait_for_idle();
    let (flings, lefts) = (recorded.flings.borrow(), recorded.lefts.borrow());
    assert!(flings.len() == 1 && flings[0] > 1000.0 && lefts.len() == 1 && lefts[0] == 0.0, "{flings:?} {lefts:?}");
}

fn stretch_holds_while_touched_and_lets_a_fling_go_by() {
    // C++ 4228 (its LazyRow a scrolling Row here: the lazy row's is the lazy lists'): pulled, let go, caught again
    // on the way back and held, it stays where the finger keeps it; let go moving up while still stretched, it
    // springs back instead of flinging; caught and let go again at once, it springs back once, not twice as fast;
    // a row pulled right at its start stretches from its left edge.
    let scroll = ScrollState::new(0);
    let s = scroll.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Column(
            modifier = Modifier.fill_max_size(),
            content = move || {
                Column(
                    modifier = Modifier.fill_max_width().height(600.dp()).vertical_scroll(&s).test_tag("list"),
                    content = || Items(),
                );
                let row = remember_scroll_state();
                Row(
                    modifier = Modifier.fill_max_width().height(200.dp()).horizontal_scroll(&row).test_tag("row"),
                    content = || {
                        for _ in 0..20 {
                            Box(modifier = Modifier.size_wh(100.dp(), 200.dp()));
                        }
                    },
                );
            },
        );
    }));
    let h = rule.host();
    let moves = |from: Offset, by: Offset, steps: i32| {
        for i in 1..=steps {
            rule.advance_frame();
            h.pointer_move(from + by * (i as f32 / steps as f32), -1, 0);
        }
    };
    let list = || stretch_of(&rule, "list").y;
    // Pulled, let go, caught again on the way back and held: it stays where the finger keeps it.
    h.pointer_down(at(200.0, 200.0), -1, 0, PointerType::Touch);
    moves(at(200.0, 200.0), at(0.0, 200.0), 10);
    h.pointer_up(at(200.0, 400.0), -1, 0);
    rule.advance_frame();
    rule.advance_frame();
    let caught = list();
    assert!(caught > 0.005, "{caught}");
    h.pointer_down(at(200.0, 400.0), -1, 0, PointerType::Touch);
    moves(at(200.0, 400.0), at(0.0, 40.0), 4);
    let held = list();
    for _ in 0..30 {
        rule.advance_frame();
    }
    assert!(held > caught && near(list(), held, 0.001), "{caught} {held} {}", list());
    // Back up a little but still stretched, and let go slowly moving up (a fling going no further than the
    // stretch): it springs back instead of flinging.
    moves(at(200.0, 440.0), at(0.0, -15.0), 3);
    assert!(list() > 0.0);
    h.pointer_up(at(200.0, 425.0), -1, 0);
    spring_back(&rule);
    assert!(list() == 0.0 && scroll.value() == 0, "{} {}", list(), scroll.value());
    // Let go fast moving up (a fling going further than the stretch, Compose's absorbToRelaxIfNeeded not
    // absorbing it): the fling takes the stretch back, then goes on through the content (C++ sprang back).
    h.pointer_down(at(200.0, 200.0), -1, 0, PointerType::Touch);
    moves(at(200.0, 200.0), at(0.0, 200.0), 10);
    moves(at(200.0, 400.0), at(0.0, -60.0), 3);
    assert!(list() > 0.0);
    h.pointer_up(at(200.0, 340.0), -1, 0);
    rule.wait_for_idle();
    assert!(list() == 0.0 && scroll.value() > 0, "{} {}", list(), scroll.value());
    scroll_to(&rule, &scroll, 0);
    // Let go, caught and let go again at once: it springs back once, not twice as fast.
    let settle = || {
        let mut frames = 0;
        while list() != 0.0 && frames < 200 {
            rule.advance_frame();
            frames += 1;
        }
        frames
    };
    h.pointer_down(at(200.0, 200.0), -1, 0, PointerType::Touch);
    moves(at(200.0, 200.0), at(0.0, 200.0), 10);
    h.pointer_up(at(200.0, 400.0), -1, 0);
    let once = settle();
    h.pointer_down(at(200.0, 200.0), -1, 0, PointerType::Touch);
    moves(at(200.0, 200.0), at(0.0, 200.0), 10);
    h.pointer_up(at(200.0, 400.0), -1, 0);
    rule.advance_frame();
    h.pointer_down(at(200.0, 400.0), -1, 0, PointerType::Touch);
    h.pointer_move(at(200.0, 430.0), -1, 0); // past the touch slop: caught
    h.pointer_up(at(200.0, 430.0), -1, 0);
    let again = settle();
    assert!(once > 10 && again > once * 3 / 4, "{once} {again}");
    rule.wait_for_idle();
    // A row pulled right at its start stretches from its left edge.
    let c = bounds(&rule, "row").center();
    h.pointer_down(c, -1, 0, PointerType::Touch);
    moves(c, at(150.0, 0.0), 10);
    let row = stretch_of(&rule, "row");
    assert!(row.x > 0.005 && row.y == 0.0, "{row:?}");
    h.pointer_up(c + at(150.0, 0.0), -1, 0);
    rule.wait_for_idle();
}

fn overscroll_effect_none_shared_or_halved() {
    // C++ 4303's scroll container part (its LazyColumn and grids are the lazy layouts', its pager the pager's):
    // none, nothing stretches; a bare scrollable taking the drags into the effect the box around it draws; one
    // effect drawn on one column and pulled on another; turned off later; equal only with the same effect.
    let (none, events, drawn, later) =
        (ScrollState::new(0), ScrollState::new(0), ScrollState::new(0), ScrollState::new(0));
    let stretches_on = mutable_state_of(true);
    type Effects =
        (Option<Rc<dyn OverscrollEffect>>, Option<Rc<dyn OverscrollEffect>>, Option<Rc<dyn OverscrollEffect>>);
    let effects: Rc<RefCell<Effects>> = Rc::new(RefCell::new((None, None, None)));
    let (n, ev, dr, la, on, ef) =
        (none.clone(), events.clone(), drawn.clone(), later.clone(), stretches_on.clone(), effects.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let effect = remember_overscroll_effect();
            let shared = remember_overscroll_effect();
            let off = Rc::new(RefCell::new(None));
            {
                let off = off.clone();
                let none_factory: Option<Rc<dyn OverscrollFactory>> = None;
                KompozitionLocalProvider(&[LocalOverscrollFactory.provides(none_factory)], move || {
                    *off.borrow_mut() = remember_overscroll_effect();
                });
            }
            *ef.borrow_mut() = (effect.clone(), shared.clone(), off.borrow().clone());
            let (n, ev, dr, la, on) = (n.clone(), ev.clone(), dr.clone(), la.clone(), on.clone());
            Row(
                modifier = Modifier.fill_max_width().height(600.dp()),
                content = move || {
                    Column(
                        modifier = Modifier
                            .weight(1.0)
                            .fill_max_height()
                            .vertical_scroll_with_overscroll(&n, None)
                            .test_tag("yok"),
                        content = || Items(),
                    );
                    // A bare scrollable (half its height) taking the drags into the effect the box around it draws.
                    let e = effect.clone();
                    Box(
                        modifier = Modifier.weight(1.0).fill_max_height().overscroll(effect.clone()).test_tag("kutu"),
                        content = move || {
                            let state = remember_scrollable_state(|_| 0.0);
                            Box(modifier = named_args!(Modifier.fill_max_width().height(300.dp()).scrollable(
                                state,
                                Orientation::Vertical,
                                overscroll_effect = e.clone()
                            ))
                            .test_tag("iç"));
                        },
                    );
                    // One stretch: drawn here, its drags there (sized only where drawn).
                    let sh = shared.clone().expect("a factory");
                    Column(
                        modifier = Modifier
                            .weight(1.0)
                            .fill_max_height()
                            .vertical_scroll_with_overscroll(&dr, Some(without_event_handling(sh.clone())))
                            .test_tag("çizim"),
                        content = || Items(),
                    );
                    Column(
                        modifier = Modifier
                            .weight(1.0)
                            .height(300.dp())
                            .vertical_scroll_with_overscroll(&ev, Some(without_visual_effect(sh.clone())))
                            .test_tag("olay"),
                        content = || Items(),
                    );
                    // The stretch turned off later.
                    let m = if on.get() {
                        Modifier.vertical_scroll(&la)
                    } else {
                        Modifier.vertical_scroll_with_overscroll(&la, None)
                    };
                    Column(
                        modifier = Modifier.weight(1.0).fill_max_height().then(m).test_tag("sonra"),
                        content = || Items(),
                    );
                },
            );
        }),
        IntSize::new(800, 800),
    );
    let h = rule.host();
    let (effect, shared) = {
        let e = effects.borrow();
        assert!(e.0.is_some() && e.1.is_some() && e.2.is_none()); // none where the factory is turned off
        (e.0.clone().unwrap(), e.1.clone().unwrap())
    };
    let held = Rc::new(Cell::new(Offset::Zero));
    // Down from its middle, held.
    let pull = |tag: &str| {
        let c = bounds(&rule, tag).center();
        drag(&rule, c, at(0.0, 200.0), PointerType::Touch, 10);
        held.set(c + at(0.0, 200.0));
    };
    let let_go = || {
        h.pointer_up(held.get(), -1, 0);
        spring_back(&rule);
    };
    // The one stretched layer's bounds (none if more than one).
    let stretched = || {
        let s = stretches(&rule);
        if s.len() == 1 { Some(s[0].0) } else { None }
    };
    let amount = |e: &Rc<dyn OverscrollEffect>| {
        let android = (&**e as &dyn std::any::Any)
            .downcast_ref::<kompoz_foundation::overscroll_android::AndroidEdgeEffectOverscrollEffect>()
            .expect("Android's stretch");
        android.distances()[0] - android.distances()[1]
    };
    // None: nothing stretches.
    pull("yok");
    assert!(stretches(&rule).is_empty());
    let_go();
    // The bare scrollable stretches the box around it, drawn over the box.
    pull("iç");
    let boxed = bounds(&rule, "kutu");
    let box_pulled = amount(&effect);
    let s = stretched();
    assert!(
        box_pulled > 0.05 && s.is_some_and(|s| s.contains(boxed.center()) && near(s.height(), boxed.height(), 0.01)),
        "{box_pulled} {s:?}"
    );
    let_go();
    assert!(amount(&effect) == 0.0 && stretched().is_none());
    // The shared one: pulled on one column, drawn on the other.
    pull("olay");
    let drawn_on = bounds(&rule, "çizim");
    let s = stretched();
    assert!(
        near(amount(&shared), box_pulled, 0.005) && s.is_some_and(|s| s.contains(drawn_on.center())),
        "{} {s:?}",
        amount(&shared)
    );
    let_go();
    pull("çizim"); // its drags (and its fling at the top) don't reach it
    assert!(amount(&shared) == 0.0 && stretched().is_none());
    h.pointer_up(held.get(), -1, 0);
    for _ in 0..5 {
        rule.advance_frame();
    }
    assert_eq!(amount(&shared), 0.0);
    rule.wait_for_idle();
    // Turned off on a column that stretched: its old stretch stays put.
    pull("sonra");
    let_go();
    stretches_on.set(false);
    rule.wait_for_idle();
    pull("sonra");
    assert!(stretch_of(&rule, "sonra") == Offset::Zero && stretched().is_none());
    let_go();
    // Turned off while stretched and held: on again, it isn't stretched still.
    stretches_on.set(true);
    rule.wait_for_idle();
    pull("sonra");
    stretches_on.set(false);
    rule.wait_for_idle();
    assert!(stretched().is_none());
    let_go();
    stretches_on.set(true);
    rule.wait_for_idle();
    assert!(stretch_of(&rule, "sonra") == Offset::Zero && stretched().is_none());
    // Equal only with the same effect: a skipped composable keeps its own.
    let s = ScrollState::new(0);
    assert!(
        Modifier.vertical_scroll_with_overscroll(&s, Some(effect.clone()))
            == Modifier.vertical_scroll_with_overscroll(&s, Some(effect.clone()))
    );
    assert!(
        Modifier.vertical_scroll_with_overscroll(&s, Some(effect.clone()))
            != Modifier.vertical_scroll_with_overscroll(&s, Some(shared.clone()))
    );
    assert!(Modifier.overscroll(Some(effect.clone())) != Modifier.overscroll(Some(shared.clone())));
    assert!(Modifier.vertical_scroll(&s) != Modifier.vertical_scroll_with_overscroll(&s, None));
}

fn a_node_that_stops_scrolling_lets_its_stretch_go() {
    // C++ 15666: stretched and held, its scrolling taken away: no stretch drawn any more; back, none either (a new
    // effect: the old one went with its node).
    let scrolls = mutable_state_of(true);
    let s = scrolls.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let scroll = remember_scroll_state();
        let m = if s.get() { Modifier.vertical_scroll(&scroll) } else { Modifier };
        Column(
            modifier = Modifier.size(100.dp()).test_tag("kutu").then(m),
            content = || {
                Box(modifier = Modifier.size_wh(100.dp(), 300.dp()));
            },
        );
    }));
    drag(&rule, at(50.0, 20.0), at(0.0, 60.0), PointerType::Touch, 10);
    assert!(stretch_of(&rule, "kutu").y > 0.0);
    scrolls.set(false);
    rule.wait_for_idle();
    assert!(stretches(&rule).is_empty());
    rule.host().pointer_up(at(50.0, 80.0), -1, 0);
    scrolls.set(true);
    rule.wait_for_idle();
    assert!(stretches(&rule).is_empty());
}

/// A lazy list held stretched at its top settles: what is due at the current time runs (`run_current`), nothing
/// left to do without a frame (stage 12 WP5 saw "never idle at the current time" there).
fn a_lazy_list_held_stretched_at_its_top_is_idle_at_the_current_time() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        LazyColumn(
            modifier = Modifier.fill_max_size().test_tag("list"),
            content = |this: &mut LazyListScope| {
                this.items_list((0..30).collect::<Vec<i32>>(), |v: &i32| {
                    Box(modifier = Modifier.fill_max_width().height(50.dp()).test_tag(format!("row {v}")));
                });
            },
        );
    }));
    let h = rule.host();
    h.pointer_down(at(200.0, 200.0), -1, 0, PointerType::Touch);
    for i in 1..=10 {
        rule.advance_frame();
        h.pointer_move(at(200.0, 200.0 + 20.0 * i as f32), -1, 0);
    }
    // Held: what the moves set going is done at the current time.
    rule.main_clock().run_current();
    let stretched = stretch_of(&rule, "list").y;
    assert!(stretched > 0.005, "{stretched}");
    rule.main_clock().run_current();
    h.pointer_up(at(200.0, 400.0), -1, 0);
    rule.wait_for_idle();
}

tests!(
    local_overscroll_factory_turns_the_stretch_off,
    scrollable_area_scrolls_clips_and_draws_its_overscroll,
    overscroll_stretches_past_the_ends_and_springs_back,
    overscroll_effect_takes_the_touch_drags_and_flings_past_the_ends,
    overscroll_effect_gets_what_the_nested_scroll_parents_leave,
    stretch_holds_while_touched_and_lets_a_fling_go_by,
    overscroll_effect_none_shared_or_halved,
    a_node_that_stops_scrolling_lets_its_stretch_go,
    a_lazy_list_held_stretched_at_its_top_is_idle_at_the_current_time,
);
