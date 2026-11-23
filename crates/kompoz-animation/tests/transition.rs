//! Transitions (Compose's Transition.kt): the C++ Kompoz's update_transition_runs_values_together (12204) and the
//! transition part of mutable_transition_states_animate_from_where_they_start (10864; its AnimatedVisibility part
//! is stage 11 WP3's); Compose's behaviour of segments and their specs, every animated type, an interrupted
//! transition, child transitions, deferred animations, SeekableTransitionState (seeking, animating, snapping,
//! interrupting), DeferredTransitionState and DeferredTransition, and seeking by tools.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_animation::*;
use kompoz_coroutines::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::Modifier;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

fn linear<T: 'static>(ms: i32) -> Arc<dyn FiniteAnimationSpec<T>> {
    Arc::new(TweenSpec::<T>::new(ms, 0, LinearEasing))
}

fn width(rule: &KompozTestRule, tag: &str) -> f32 {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root().width()
}

/// A slot a composition hands something out through.
type Slot<T> = Rc<RefCell<Option<T>>>;

fn slot<T>() -> Slot<T> {
    Rc::new(RefCell::new(None))
}

fn take<T: Clone>(s: &Slot<T>) -> T {
    s.borrow().clone().expect("set by the composition")
}

// ---- C++ 12204 ----------------------------------------------------------------------------------------------------

fn update_transition_runs_values_together() {
    let on = mutable_state_of(false);
    let states = Rc::new(RefCell::new(Vec::<String>::new()));
    let (o, s) = (on.clone(), states.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let t = update_transition(o.get());
        let size = t.animate_float(
            transition_spec = |_| linear(100),
            target_value_by_state = |s: &bool| {
                if *s { 100.0 } else { 50.0 }
            },
        );
        let color = t.animate_color(
            transition_spec = |_| linear(200),
            target_value_by_state = |s: &bool| {
                if *s { Color::White } else { Color::Black }
            },
        );
        s.borrow_mut().push(format!(
            "{} {} {:.0} {:.1}",
            t.current_state(),
            t.is_running(),
            size.value(),
            color.value().red()
        ));
    }));
    assert_eq!(states.borrow().last().unwrap(), "false false 50 0.0");
    on.set(true);
    rule.advance_time_by(150); // the size is done, the color isn't: still running
    assert!(states.borrow().last().unwrap().starts_with("false true 100"), "{:?}", states.borrow().last());
    rule.wait_for_idle();
    assert_eq!(states.borrow().last().unwrap(), "true false 100 1.0");
}

// ---- C++ 10864, its transition part ------------------------------------------------------------------------------

fn mutable_transition_states_animate_from_where_they_start() {
    // (its AnimatedVisibility(visibleState) part is WP3's)
    let open = mutable_state_of(false);
    let step = MutableTransitionState(0);
    step.set_target_state(1); // on its way as it first composes
    let (o, st) = (open.clone(), step.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, st) = (o.clone(), st.clone());
        Column(move || {
            if !o.get() {
                return;
            }
            let t = remember_transition(st.clone());
            let w =
                t.animate_float(transition_spec = |_| linear(320), target_value_by_state = |s: &i32| *s as f32 * 100.0);
            Box(modifier = Modifier.size_wh((1.0 + w.value()).dp(), 10.dp()).test_tag("geçiş"));
        });
    }));
    rule.main_clock().set_auto_advance(false);
    assert!(step.current_state() == 0 && !step.is_idle()); // not there yet
    open.set(true);
    rule.advance_time_by(160);
    // On its way from its current state.
    let w = width(&rule, "geçiş");
    assert!(w > 30.0 && w < 71.0, "{w}");
    assert!(step.current_state() == 0 && !step.is_idle());
    settle(&rule);
    assert!(near(width(&rule, "geçiş"), 101.0, 0.5) && step.current_state() == 1 && step.is_idle());
    // Turned back on the way: not idle though back where it is going. Adapted: Compose's transition takes the target it
    // was going to (2) as its current state when its target changes (C++ kept 1).
    step.set_target_state(2);
    rule.advance_time_by(96);
    step.set_target_state(1);
    rule.advance_frame();
    assert!(step.current_state() == 2 && step.target_state() == 1 && !step.is_idle());
    settle(&rule);
    assert!(step.is_idle() && step.current_state() == 1 && near(width(&rule, "geçiş"), 101.0, 0.5));
}

// ---- Compose's behaviour -------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Panel {
    Closed,
    Open,
}

fn segments_pick_the_specs() {
    // `transition_spec` gets the segment: `Closed isTransitioningTo Open` a tween, back a snap.
    let open = mutable_state_of(Panel::Closed);
    let seen = Rc::new(RefCell::new(Vec::<(f32, Segment<Panel>)>::new()));
    let transition: Slot<Transition<Panel>> = slot();
    let (o, s, tr) = (open.clone(), seen.clone(), transition.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let t = update_transition(o.get(), Some("panel".to_string()));
        let v = t.animate_float(
            transition_spec = |segment: &Segment<Panel>| -> Arc<dyn FiniteAnimationSpec<f32>> {
                if segment.is_transitioning_to(Panel::Closed, Panel::Open) { linear(160) } else { Arc::new(snap()) }
            },
            label = "genişlik".to_string(),
            target_value_by_state = |p: &Panel| if *p == Panel::Open { 100.0 } else { 0.0 },
        );
        s.borrow_mut().push((v.value(), t.segment()));
        *tr.borrow_mut() = Some(t);
    }));
    let t = take(&transition);
    assert_eq!(t.label().as_deref(), Some("panel"));
    assert_eq!((t.animations().len(), t.animations()[0].label()), (1, "genişlik".to_string()));
    assert_eq!(t.segment(), Segment::new(Panel::Closed, Panel::Closed));
    open.set(Panel::Open);
    rule.advance_frame();
    assert_eq!(t.segment(), Segment::new(Panel::Closed, Panel::Open));
    assert_eq!(t.total_duration_nanos(), 160_000_000);
    rule.advance_time_by(80);
    let mid = seen.borrow().last().unwrap().0;
    assert!(mid > 30.0 && mid < 70.0, "{mid}");
    rule.wait_for_idle();
    assert_eq!(seen.borrow().last().unwrap().0, 100.0);
    assert_eq!(t.current_state(), Panel::Open);
    // Back: a snap (no duration), there at its first frame.
    open.set(Panel::Closed);
    rule.advance_frame();
    assert_eq!(t.segment(), Segment::new(Panel::Open, Panel::Closed));
    rule.advance_frame();
    rule.advance_frame();
    assert_eq!(seen.borrow().last().unwrap().0, 0.0);
    rule.wait_for_idle();
    assert!(!t.is_running() && t.current_state() == Panel::Closed);
}

fn every_type_animates_in_a_transition() {
    let on = mutable_state_of(false);
    let seen = Rc::new(RefCell::new(String::new()));
    let (o, s) = (on.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let t = update_transition(o.get());
        let dp = t.animate_dp(target_value_by_state = |v: &bool| if *v { 10.dp() } else { 0.dp() });
        let offset =
            t.animate_offset(target_value_by_state = |v: &bool| if *v { Offset::new(3.0, 4.0) } else { Offset::Zero });
        let size = t.animate_size(target_value_by_state = |v: &bool| if *v { Size::new(5.0, 6.0) } else { Size::Zero });
        let int = t.animate_int(target_value_by_state = |v: &bool| if *v { 7 } else { 0 });
        let int_offset = t.animate_int_offset(
            target_value_by_state = |v: &bool| if *v { IntOffset::new(1, 2) } else { IntOffset::Zero },
        );
        let int_size =
            t.animate_int_size(target_value_by_state = |v: &bool| if *v { IntSize::new(3, 4) } else { IntSize::Zero });
        let rect = t.animate_rect(
            target_value_by_state = |v: &bool| {
                if *v { Rect::new(1.0, 1.0, 9.0, 9.0) } else { Rect::Zero }
            },
        );
        let value = t.animate_value(
            DpSize::VectorConverter,
            target_value_by_state = |v: &bool| {
                if *v { DpSize::new(2.dp(), 3.dp()) } else { DpSize::Zero }
            },
        );
        *s.borrow_mut() = format!(
            "{:?} {:?} {:?} {} {:?} {:?} {:?} {:?}",
            dp.value(),
            offset.value(),
            size.value(),
            int.value(),
            int_offset.value(),
            int_size.value(),
            rect.value(),
            value.value()
        );
    }));
    on.set(true);
    rule.wait_for_idle();
    let expected = format!(
        "{:?} {:?} {:?} 7 {:?} {:?} {:?} {:?}",
        10.dp(),
        Offset::new(3.0, 4.0),
        Size::new(5.0, 6.0),
        IntOffset::new(1, 2),
        IntSize::new(3, 4),
        Rect::new(1.0, 1.0, 9.0, 9.0),
        DpSize::new(2.dp(), 3.dp())
    );
    assert_eq!(*seen.borrow(), expected);
}

fn an_interrupted_transition_goes_on_from_where_it_is() {
    // A new target on the way: the current state becomes the former target, every value sets off from where it is
    // (a tween interrupted goes on along the default spring).
    let to = mutable_state_of(0);
    let seen = Rc::new(RefCell::new(Vec::<f32>::new()));
    let transition: Slot<Transition<i32>> = slot();
    let (t, s, tr) = (to.clone(), seen.clone(), transition.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let tn = update_transition(t.get());
        let v =
            tn.animate_float(transition_spec = |_| linear(320), target_value_by_state = |s: &i32| *s as f32 * 100.0);
        s.borrow_mut().push(v.value());
        *tr.borrow_mut() = Some(tn);
    }));
    let tn = take(&transition);
    to.set(1);
    rule.advance_time_by(160);
    let mid = *seen.borrow().last().unwrap();
    assert!(mid > 30.0 && mid < 60.0, "{mid}");
    to.set(2);
    rule.advance_frame();
    assert_eq!((tn.current_state(), tn.target_state()), (1, 2));
    // (no jump: from where it was)
    let after = *seen.borrow().last().unwrap();
    assert!((after - mid).abs() < 10.0, "{mid} → {after}");
    rule.wait_for_idle();
    assert_eq!(*seen.borrow().last().unwrap(), 200.0);
    assert_eq!(tn.current_state(), 2);
    // Its animations made by the interruption spring: their spec is no longer the tween's duration.
    let animation =
        TransitionAnimationState::<i32, f32, AnimationVector1D>::from_any(tn.animations()[0].clone()).unwrap();
    assert_eq!(animation.value(), 200.0);
}

fn a_child_transition_runs_with_its_parent() {
    // The child's state is the parent's mapped; the parent runs until the child's animations are done too, and counts
    // their duration.
    let to = mutable_state_of(0);
    let (parent, child): (Slot<Transition<i32>>, Slot<Transition<bool>>) = (slot(), slot());
    let seen = Rc::new(Cell::new(0.0f32));
    let (t, p, c, s) = (to.clone(), parent.clone(), child.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let tn = update_transition(t.get(), Some("üst".to_string()));
        let short = tn.animate_float(transition_spec = |_| linear(100), target_value_by_state = |s: &i32| *s as f32);
        let _ = short.value();
        let ch = tn.create_child_transition(label = "çocuk".to_string(), transform_to_child_state = |s: &i32| *s > 1);
        let long = ch.animate_float(
            transition_spec = |_| linear(300),
            target_value_by_state = |b: &bool| {
                if *b { 1.0 } else { 0.0 }
            },
        );
        s.set(long.value());
        *p.borrow_mut() = Some(tn);
        *c.borrow_mut() = Some(ch);
    }));
    let (p, c) = (take(&parent), take(&child));
    assert_eq!(c.label().as_deref(), Some("üst > çocuk"));
    assert_eq!(p.transitions().len(), 1);
    assert!(Transition::<bool>::from_any(p.transitions()[0].clone()).is_some_and(|t| t == c));
    // A parent change that doesn't change the child: the child stays.
    to.set(1);
    rule.wait_for_idle();
    assert!(!c.current_state() && !c.target_state() && p.current_state() == 1);
    to.set(2);
    rule.advance_frame();
    assert!(c.target_state() && !c.current_state());
    assert_eq!(p.total_duration_nanos(), 300_000_000);
    rule.advance_time_by(160);
    // The parent's own animation is done; it still runs with its child.
    assert!(p.is_running() && p.current_state() == 1 && seen.get() > 0.2 && seen.get() < 0.8);
    rule.wait_for_idle();
    assert!(!p.is_running() && p.current_state() == 2 && c.current_state() && seen.get() == 1.0);
}

fn a_deferred_animation_animates_once_set_up() {
    // Set up later (a measure's size): it animates from the current state to the target's values from then on.
    let big = mutable_state_of(false);
    let seen = Rc::new(RefCell::new(Vec::<f32>::new()));
    let (b, s) = (big.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let t = update_transition(b.get());
        let deferred = t.create_deferred_animation(f32::VectorConverter, label = "boy".to_string());
        assert!(deferred.animation().is_none() || deferred.label() == "boy");
        let s = s.clone();
        Box(modifier = Modifier.size(10.dp()).layout(move |scope, measurable, constraints| {
            let size = deferred.animate(|_| linear(160), |b: &bool| if *b { 100.0 } else { 10.0 });
            s.borrow_mut().push(size.value());
            let p = measurable.measure(constraints);
            scope.layout(p.width(), p.height(), move |pl| pl.place(&p, 0, 0))
        }));
    }));
    assert_eq!(*seen.borrow().last().unwrap(), 10.0);
    big.set(true);
    rule.advance_time_by(96);
    let mid = *seen.borrow().last().unwrap();
    assert!(mid > 20.0 && mid < 90.0, "{mid}");
    rule.wait_for_idle();
    assert_eq!(*seen.borrow().last().unwrap(), 100.0);
}

fn tools_seek_a_transition() {
    // Compose's tools-only seek: the transition put between two states at a play time, its children too, its
    // animations there; it doesn't run on its own from then on.
    let transition: Slot<Transition<bool>> = slot();
    let child: Slot<Transition<i32>> = slot();
    let seen = Rc::new(RefCell::new((0.0f32, 0.0f32)));
    let (tr, ch, s) = (transition.clone(), child.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let t = update_transition(false);
        let v = t.animate_float(
            transition_spec = |_| linear(100),
            target_value_by_state = |b: &bool| if *b { 10.0 } else { 0.0 },
        );
        let c = t.create_child_transition(transform_to_child_state = |b: &bool| if *b { 4 } else { 2 });
        let w = c.animate_float(transition_spec = |_| linear(100), target_value_by_state = |i: &i32| *i as f32);
        *s.borrow_mut() = (v.value(), w.value());
        *tr.borrow_mut() = Some(t);
        *ch.borrow_mut() = Some(c);
    }));
    let t = take(&transition);
    t.set_playtime_after_initial_and_target_state_established(false, true, 50_000_000);
    rule.wait_for_idle();
    let c = take(&child);
    assert!(t.is_seeking() && c.is_seeking());
    assert_eq!((c.current_state(), c.target_state()), (2, 4));
    t.set_playtime_after_initial_and_target_state_established(false, true, 50_000_000);
    rule.wait_for_idle();
    let (v, w) = *seen.borrow();
    assert!(near(v, 5.0, 0.01) && near(w, 3.0, 0.01), "{v} {w}");
    assert!(!t.is_running());
}

// ---- SeekableTransitionState ---------------------------------------------------------------------------------------

/// A seekable transition from `Closed`: the value it animates (0 closed, 100 open, linearly over 320 ms), its scope.
fn seekable() -> (KompozTestRule, SeekableTransitionState<Panel>, Rc<Cell<f32>>, CoroutineScope, Slot<Transition<Panel>>)
{
    let state = SeekableTransitionState(Panel::Closed);
    let seen = Rc::new(Cell::new(-1.0f32));
    let scope: Slot<CoroutineScope> = slot();
    let transition: Slot<Transition<Panel>> = slot();
    let (st, s, sc, tr) = (state.clone(), seen.clone(), scope.clone(), transition.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *sc.borrow_mut() = Some(remember_coroutine_scope());
        let t = remember_transition(st.clone());
        let v = t.animate_float(
            transition_spec = |_| linear(320),
            target_value_by_state = |p: &Panel| {
                if *p == Panel::Open { 100.0 } else { 0.0 }
            },
        );
        s.set(v.value());
        *tr.borrow_mut() = Some(t);
    }));
    let scope = take(&scope);
    (rule, state, seen, scope, transition)
}

fn a_seekable_transition_seeks_and_animates() {
    let (rule, state, seen, scope, transition) = seekable();
    let t = take(&transition);
    assert_eq!((state.current_state(), state.target_state(), state.fraction()), (Panel::Closed, Panel::Closed, 0.0));
    // Seeking toward Open: the values a fraction of the way, the state between the two.
    scope.launch(named_args!(state.seek_to(0.25, target_state = Panel::Open)));
    rule.wait_for_idle();
    assert_eq!((state.current_state(), state.target_state()), (Panel::Closed, Panel::Open));
    assert_eq!(state.fraction(), 0.25);
    assert!(near(seen.get(), 25.0, 0.01), "{}", seen.get());
    assert_eq!(t.total_duration_nanos(), 320_000_000);
    scope.launch(state.seek_to(0.5));
    rule.wait_for_idle();
    assert!(near(seen.get(), 50.0, 0.01));
    // Animated the rest of the way: linear over the rest of the duration, then there.
    scope.launch(state.animate_to());
    rule.advance_frame(); // (its first frame: the start time)
    rule.advance_time_by(80);
    let mid = seen.get();
    assert!(mid > 55.0 && mid < 85.0, "{mid}");
    assert!(state.fraction() > 0.5 && state.fraction() < 1.0);
    rule.wait_for_idle();
    assert_eq!((state.current_state(), state.target_state(), state.fraction()), (Panel::Open, Panel::Open, 0.0));
    assert_eq!(seen.get(), 100.0);
    // Snapped back: there at once.
    scope.launch(state.snap_to(Panel::Closed));
    rule.wait_for_idle();
    assert_eq!((state.current_state(), state.target_state()), (Panel::Closed, Panel::Closed));
    assert_eq!(seen.get(), 0.0);
    assert!(!t.is_running());
}

fn a_seekable_transition_animates_along_a_spec() {
    let (rule, state, seen, scope, _) = seekable();
    // Along a spec of the fraction: a 64 ms tween, whatever the transition's own duration.
    let spec: Arc<dyn FiniteAnimationSpec<f32>> = linear(64);
    scope.launch(named_args!(state.animate_to(Panel::Open, animation_spec = Some(spec))));
    rule.advance_frame(); // composed with the new target, the start time taken while waiting for it
    rule.advance_time_by(32);
    assert_near(seen.get(), 50.0, 0.01);
    rule.advance_time_by(32);
    assert_eq!((seen.get(), state.current_state()), (100.0, Panel::Open));
}

fn a_seekable_transition_redirected_animates_its_start_values() {
    // Seeked half way to Open, then to Closed again: the values it had animate on toward Open while the new seek starts
    // from Open (its current state then).
    let (rule, state, seen, scope, transition) = seekable();
    let t = take(&transition);
    scope.launch(named_args!(state.seek_to(0.5, target_state = Panel::Open)));
    rule.wait_for_idle();
    assert!(near(seen.get(), 50.0, 0.01));
    let job = scope.launch(named_args!(state.seek_to(0.0, target_state = Panel::Closed)));
    rule.advance_frame();
    assert_eq!((state.current_state(), state.target_state()), (Panel::Open, Panel::Closed));
    assert!(t.has_initial_value_animations());
    rule.advance_frame();
    rule.advance_frame();
    let moving = seen.get();
    assert!(moving > 50.0 && moving < 100.0, "{moving}");
    rule.wait_for_idle();
    assert!(job.is_completed() && !t.has_initial_value_animations());
    // At fraction 0 of Open → Closed: Open's value.
    assert!(near(seen.get(), 100.0, 0.01), "{}", seen.get());
}

fn a_seekable_transition_is_one_transitions() {
    let state = SeekableTransitionState(0);
    let (a, b) = (state.clone(), state.clone());
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        remember_transition(a.clone());
    }));
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _second = KompozTestRule::new(kompozable_lambda!(move || {
            remember_transition(b.clone());
        }));
    }));
    let message = failed.expect_err("used twice");
    let message = message.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(message.contains("An instance of SeekableTransitionState has been used in different Transitions"));
    let fraction_out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        drop(state.seek_to(1.5));
    }));
    let message = fraction_out.expect_err("out of range");
    let message = message.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(message.contains("Expecting fraction between 0 and 1. Got 1.5"), "{message}");
}

// ---- DeferredTransitionState ---------------------------------------------------------------------------------------

fn a_deferred_transition_holds_its_target_while_deferring() {
    let state = DeferredTransitionState(Panel::Closed);
    let transition: Slot<DeferredTransition<Panel>> = slot();
    let child: Slot<Transition<bool>> = slot();
    let seen = Rc::new(Cell::new(-1.0f32));
    let (st, tr, ch, s) = (state.clone(), transition.clone(), child.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let t = remember_deferred_transition(st.clone(), Some("ertelenen".to_string()));
        let v = t.animate_float(
            transition_spec = |_| linear(160),
            target_value_by_state = |p: &Panel| {
                if *p == Panel::Open { 100.0 } else { 0.0 }
            },
        );
        let c = t.create_child_transition(transform_to_child_state = |p: &Panel| *p == Panel::Open);
        s.set(v.value());
        *tr.borrow_mut() = Some(t);
        *ch.borrow_mut() = Some(c);
    }));
    let (t, c) = (take(&transition), take(&child));
    // Deferred: the target stays, the pending state is told (to the children too, mapped).
    state.defer(Panel::Open);
    rule.wait_for_idle();
    assert_eq!(state.pending_target_state(), Some(Panel::Open));
    assert_eq!((t.pending_target_state(), t.target_state()), (Some(Panel::Open), Panel::Closed));
    assert_eq!(c.pending_target_state(), Some(true));
    assert!(seen.get() == 0.0 && !t.is_running());
    // Its phase ended: the transition runs to it.
    state.animate_to(Panel::Open);
    rule.advance_time_by(96);
    assert_eq!((t.pending_target_state(), t.target_state()), (None, Panel::Open));
    assert!(seen.get() > 0.0 && seen.get() < 100.0);
    rule.wait_for_idle();
    assert!(seen.get() == 100.0 && state.current_state() == Panel::Open);
    // Deferred to where it is: nothing pending.
    state.defer(Panel::Open);
    assert_eq!(state.pending_target_state(), None);
    // Deferred then back to where it was: from the pending state back (it settles at once without hand-set values).
    state.defer(Panel::Closed);
    rule.wait_for_idle();
    state.animate_to(Panel::Open);
    rule.wait_for_idle();
    assert_eq!(t.pending_target_state(), None);
    assert!(seen.get() == 100.0 && t.current_state() == Panel::Open && !t.is_running());
}

fn a_transition_of_a_new_state_snaps_to_it() {
    // Compose's double tap to like: a new MutableTransitionState makes a new transition, at its state at once.
    let state = mutable_state_of(MutableTransitionState(false));
    let seen = Rc::new(Cell::new(-1.0f32));
    let (st, s) = (state.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        #[allow(deprecated)]
        let t = update_transition_of_state(st.get());
        let v = t.animate_float(
            transition_spec = |_| linear(320),
            target_value_by_state = |b: &bool| if *b { 1.0 } else { 0.0 },
        );
        s.set(v.value());
    }));
    assert_eq!(seen.get(), 0.0);
    state.set(MutableTransitionState(true));
    rule.advance_frame();
    assert_eq!(seen.get(), 1.0);
}

/// A scene holding an `Rc` (a name shared, a lambda): not `Send`, as Compose's states and animated values needn't
/// be.
#[derive(Clone, PartialEq, Debug)]
struct Scene {
    name: Rc<str>,
    width: f32,
}

/// Transitions, animatables and animate*AsState animate any value (Compose's: `Clone + PartialEq` here): a
/// transition's state and an animated value holding an `Rc`, kept in states confined to the UI thread.
fn transitions_and_animations_take_values_that_arent_send() {
    let on = mutable_state_of(false);
    let seen = Rc::new(RefCell::new(String::new()));
    let (o, s) = (on.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let scene = if o.get() {
            Scene { name: Rc::from("açık"), width: 100.0 }
        } else {
            Scene { name: Rc::from("kapalı"), width: 0.0 }
        };
        let t = update_transition(scene.clone());
        let width = t.animate_float(transition_spec = |_| linear(100), target_value_by_state = |s: &Scene| s.width);
        let converter = TwoWayConverter(
            |s: Scene| AnimationVector1D(s.width),
            |v: AnimationVector1D| Scene { name: Rc::from("yolda"), width: v.value },
        );
        let spec: Arc<dyn AnimationSpec<Scene>> = Arc::new(TweenSpec::<Scene>::new(200, 0, LinearEasing));
        let value = animate_value_as_state(scene, converter, animation_spec = spec);
        *s.borrow_mut() = format!(
            "{} {} {:.0} {} {:.0}",
            t.current_state().name,
            t.target_state().name,
            width.value(),
            value.value().name,
            value.value().width
        );
    }));
    assert_eq!(*seen.borrow(), "kapalı kapalı 0 kapalı 0");
    on.set(true);
    rule.advance_frame();
    // (the virtual clock's 16 ms frames: the change composed at the first, the animations started at the next;
    // 100 ms rounded up to 7 frames: 6 frames, 96 ms, after the start)
    rule.advance_time_by(100);
    assert_eq!(*seen.borrow(), "kapalı açık 96 yolda 48", "{}", seen.borrow());
    rule.wait_for_idle();
    assert_eq!(*seen.borrow(), "açık açık 100 açık 100");
}

tests!(
    transitions_and_animations_take_values_that_arent_send,
    update_transition_runs_values_together,
    mutable_transition_states_animate_from_where_they_start,
    segments_pick_the_specs,
    every_type_animates_in_a_transition,
    an_interrupted_transition_goes_on_from_where_it_is,
    a_child_transition_runs_with_its_parent,
    a_deferred_animation_animates_once_set_up,
    tools_seek_a_transition,
    a_seekable_transition_seeks_and_animates,
    a_seekable_transition_animates_along_a_spec,
    a_seekable_transition_redirected_animates_its_start_values,
    a_seekable_transition_is_one_transitions,
    a_deferred_transition_holds_its_target_while_deferring,
    a_transition_of_a_new_state_snaps_to_it,
);
