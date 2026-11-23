//! The test rule's clock as Compose's MainTestClock: auto advance (infinite animations cancelled by the rule's
//! InfiniteAnimationPolicy while it is on; run while it is off), advancing by a time (frames or exact), until a
//! condition, waiting until a condition, running on idle; finders waiting for idle (Compose's test owner's getRoots),
//! the delays idling doesn't wait for, synchronizing inside a frame refused; the composition's effect context the
//! rule gives (Compose's Recomposer.effectCoroutineContext).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz_animation::*;
use kompoz_coroutines::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::{InfiniteAnimationPolicy, Modifier, ModifierNode, ModifierNodeElement, NodeCx};
use std::cell::Cell;
use std::rc::Rc;

/// An infinite animation's loop: a frame counted each time it gets one.
async fn counting_forever(frames: Rc<Cell<u32>>) {
    loop {
        with_infinite_animation_frame_nanos(|_| frames.set(frames.get() + 1)).await;
    }
}

fn an_infinite_animation_is_cancelled_while_the_clock_advances_on_its_own() {
    let frames = Rc::new(Cell::new(0));
    let f = frames.clone();
    // (no "never idle": it is cancelled at its first frame)
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        LaunchedEffect((), counting_forever(f.clone()));
    }));
    assert!(rule.main_clock().auto_advance());
    rule.wait_for_idle();
    assert_eq!(frames.get(), 0);
    rule.main_clock().advance_time_by(160);
    assert_eq!(frames.get(), 0);
}

fn with_auto_advance_off_infinite_animations_run_as_the_test_advances() {
    let frames = Rc::new(Cell::new(0));
    let rule = KompozTestRule::empty();
    rule.main_clock().set_auto_advance(false);
    let f = frames.clone();
    rule.set_content(kompozable_lambda!(move || {
        LaunchedEffect((), counting_forever(f.clone()));
    }));
    // Composed and launched, at the time it was: no frame yet.
    assert_eq!(rule.main_clock().current_time(), 0);
    assert_eq!(frames.get(), 0);
    rule.main_clock().advance_time_by(160);
    assert_eq!(frames.get(), 10);
    // Waiting for idle runs what is due now, the clock still.
    rule.wait_for_idle();
    assert_eq!((rule.main_clock().current_time(), frames.get()), (160, 10));
    rule.main_clock().advance_time_by_frame();
    assert_eq!(frames.get(), 11);
    // Back on: its next frame cancels it.
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
    let n = frames.get();
    rule.main_clock().advance_time_by(100);
    assert_eq!(frames.get(), n);
}

fn the_clock_advances_until_a_condition_holds() {
    let frames = Rc::new(Cell::new(0));
    let f = frames.clone();
    let rule = KompozTestRule::empty();
    rule.main_clock().set_auto_advance(false);
    rule.set_content(kompozable_lambda!(move || {
        let f = f.clone();
        LaunchedEffect((), async move {
            loop {
                with_frame_nanos(|_| f.set(f.get() + 1)).await;
            }
        });
    }));
    let start = rule.main_clock().current_time();
    let c = frames.clone();
    named_args!(rule.main_clock().advance_time_until(condition = move || c.get() >= 5));
    assert_eq!(frames.get(), 5);
    assert_eq!(rule.main_clock().current_time() - start, 5 * 16);
    // Not reached in time: a ComposeTimeoutException.
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        named_args!(rule.main_clock().advance_time_until(timeout_millis = 100, condition = || false));
    }));
    let error = failed.expect_err("timed out");
    let error = error.downcast_ref::<ComposeTimeoutException>().expect("a ComposeTimeoutException");
    assert_eq!(error.0, "Condition still not satisfied after 100 ms");
}

fn time_advances_by_frames_or_exactly() {
    let fired = Rc::new(Cell::new(-1i64));
    let f = fired.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let f = f.clone();
        LaunchedEffect((), async move {
            delay_ms(40).await;
            f.set(Dispatcher::current().now() / 1_000_000);
        });
    }));
    rule.main_clock().set_auto_advance(false);
    let start = rule.main_clock().current_time();
    // Exactly 40 ms: the frames on the way (16, 32), then the delay due at 40, no frame.
    named_args!(rule.main_clock().advance_time_by(40, ignore_frame_duration = true));
    assert_eq!(rule.main_clock().current_time() - start, 40);
    assert_eq!(fired.get() - start, 40);
    // Rounded up to whole frames.
    rule.main_clock().advance_time_by(40);
    assert_eq!(rule.main_clock().current_time() - start, 40 + 48);
}

fn waiting_until_and_running_on_idle() {
    let count = mutable_state_of(0);
    let c = count.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let c = c.clone();
        LaunchedEffect((), async move {
            for _ in 0..3 {
                delay_ms(100).await;
                c.update(|v| *v += 1);
            }
        });
    }));
    let k = count.clone();
    named_args!(rule.wait_until(condition = move || k.get() == 2));
    assert_eq!(count.get(), 2);
    // (Compose's waitForIdle doesn't wait for a coroutine's delay: the third step comes when the clock gets to it —
    // the C++ rule's idling waited for delays due within 2 s)
    assert_eq!(rule.run_on_idle(|| count.get()), 2);
    rule.main_clock().advance_time_by(100);
    assert_eq!(rule.run_on_idle(|| count.get()), 3);
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        named_args!(rule.wait_until(condition_description = Some("hiç"), timeout_millis = 50, condition = || false));
    }));
    let error = failed.expect_err("timed out");
    let error = error.downcast_ref::<ComposeTimeoutException>().expect("a ComposeTimeoutException");
    assert_eq!(error.0, "Condition (hiç) still not satisfied after 50 ms");
}

/// A node noting whether its coroutine scope runs with an infinite animation policy.
struct PolicyNode(Rc<Cell<bool>>);

impl ModifierNode for PolicyNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        self.0.set(cx.coroutine_scope().coroutine_context().get::<InfiniteAnimationPolicy>().is_some());
    }
}

#[derive(PartialEq)]
struct PolicyElement;

thread_local! {
    static NODE_SEES: Rc<Cell<bool>> = Rc::new(Cell::new(false));
}

impl ModifierNodeElement for PolicyElement {
    type Node = PolicyNode;
    fn create(&self) -> PolicyNode {
        PolicyNode(NODE_SEES.with(|s| s.clone()))
    }
    fn update(&self, _: &mut PolicyNode) {}
}

fn effects_and_nodes_run_in_the_rules_effect_context() {
    let seen = Rc::new(Cell::new((false, false, false)));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        let has = || current_coroutine_context().get::<InfiniteAnimationPolicy>().is_some();
        let scope = remember_coroutine_scope();
        let in_scope = scope.coroutine_context().get::<InfiniteAnimationPolicy>().is_some();
        let s2 = s.clone();
        LaunchedEffect((), async move {
            let (_, _, produced) = s2.get();
            s2.set((has(), in_scope, produced));
        });
        let s3 = s.clone();
        let _ = produce_state(0, (), move |_state| async move {
            let (launched, in_scope, _) = s3.get();
            s3.set((launched, in_scope, has()));
        });
        Box(modifier = Modifier::element(PolicyElement));
    }));
    assert_eq!(seen.get(), (true, true, true));
    assert!(NODE_SEES.with(|s| s.get()));
    assert!(rule.host().effect_coroutine_context().get::<InfiniteAnimationPolicy>().is_some());
}

fn finders_wait_for_idle_as_composes_do() {
    // Compose's test owner's getRoots → waitForIdle: a finder sees the UI settled — with the clock's auto advance on,
    // an animation run to its end; off, only what is due now (composed and laid out, no frame).
    let wide = mutable_state_of(false);
    let w = wide.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let width = animate_dp_as_state(if w.get() { 200.dp() } else { 100.dp() });
        Box(modifier = Modifier.width(width.get()).height(10.dp()).test_tag("kutu"));
    }));
    let width = |rule: &KompozTestRule| rule.on_node_with_tag("kutu").get_unclipped_bounds_in_root().width();
    let start = rule.main_clock().current_time();
    wide.set(true);
    assert_eq!(width(&rule), 200.0);
    assert!(rule.main_clock().current_time() > start);
    // Waiting when idle advances no frame.
    let now = rule.main_clock().current_time();
    rule.wait_for_idle();
    assert_eq!(rule.main_clock().current_time(), now);
    // Off: what is due now, then the test's frames.
    rule.main_clock().set_auto_advance(false);
    wide.set(false);
    assert_eq!(width(&rule), 200.0);
    rule.main_clock().advance_time_by(48);
    let between = width(&rule);
    assert!(between > 100.0 && between < 200.0, "{between}");
    // Without the implicit wait (Compose's runWithoutImplicitWait): the tree as it is.
    rule.main_clock().set_auto_advance(true);
    assert_eq!(rule.run_without_implicit_wait(|| width(&rule)), between);
    assert_eq!(width(&rule), 100.0);
}

fn waiting_for_idle_doesnt_wait_for_delays() {
    // Compose's ComposeIdlingResource counts frame awaiters, recompositions and layouts, not a coroutine's delay:
    // the clock gets to it when the test advances it.
    let done = mutable_state_of(false);
    let d = done.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let d = d.clone();
        LaunchedEffect((), async move {
            delay_ms(5_000).await;
            d.set(true);
        });
    }));
    let start = rule.main_clock().current_time();
    rule.wait_for_idle();
    assert!(!done.get() && rule.main_clock().current_time() == start);
    rule.main_clock().advance_time_by(5_000);
    assert!(done.get());
}

fn synchronizing_inside_a_frame_panics_as_composes_does() {
    // A finder's wait called from a composition (a frame of the host): Compose's synchronization can't run on the UI
    // thread (its EspressoLink's message).
    let holder: Rc<std::cell::RefCell<Option<Rc<KompozTestRule>>>> = Rc::default();
    let message: Rc<std::cell::RefCell<Option<String>>> = Rc::default();
    let go = mutable_state_of(false);
    let (h, m, g) = (holder.clone(), message.clone(), go.clone());
    let rule = Rc::new(KompozTestRule::new(kompozable_lambda!(move || {
        if g.get()
            && let Some(rule) = h.borrow().clone()
        {
            let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rule.wait_for_idle()));
            let error = failed.expect_err("synchronized inside a frame");
            *m.borrow_mut() =
                error.downcast_ref::<String>().cloned().or_else(|| error.downcast_ref::<&str>().map(|s| s.to_string()));
        }
    })));
    *holder.borrow_mut() = Some(rule.clone());
    go.set(true);
    rule.main_clock().advance_time_by_frame();
    let message = message.borrow().clone().expect("the panic's message");
    assert!(message.starts_with("Functions that involve synchronization (Assertions, Actions, Synchronization;"));
    assert!(message.ends_with("Did you nest such a function inside runOnIdle {}, runOnUiThread {} or setContent {}?"));
    holder.borrow_mut().take();
}

tests!(
    an_infinite_animation_is_cancelled_while_the_clock_advances_on_its_own,
    with_auto_advance_off_infinite_animations_run_as_the_test_advances,
    the_clock_advances_until_a_condition_holds,
    time_advances_by_frames_or_exactly,
    waiting_until_and_running_on_idle,
    effects_and_nodes_run_in_the_rules_effect_context,
    finders_wait_for_idle_as_composes_do,
    waiting_for_idle_doesnt_wait_for_delays,
    synchronizing_inside_a_frame_panics_as_composes_does,
);
