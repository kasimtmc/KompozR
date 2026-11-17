//! Animations running on a host's frame clock: Animatable (animate_to, animate_decay, snap_to, stop, bounds, one
//! at a time), the suspend `animate` functions and AnimationState's, the MutatorMutex, a MotionDurationScale and an
//! InfiniteAnimationPolicy in the coroutine's context, Android's fling at the host's density. The C++ Kompoz's
//! animatable_animates_and_can_be_interrupted (3070), animatable_carries_a_velocity_into_springs_and_decays (24036)
//! and keyframes_go_through_their_frames' Animatable part (24150; its animateFloatAsState part is stage 11's).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_animation::*;
use kompoz_coroutines::*;
use kompoz_graphics::{Color, Density, Offset};
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::platform::LocalDensity;
use kompoz_ui::{InfiniteAnimationPolicy, MotionDurationScale};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

/// A rule composing nothing, and a scope of its host's coroutines.
fn rule_and_scope() -> (KompozTestRule, CoroutineScope) {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {}));
    let scope = CoroutineScope::new(rule.host().dispatcher());
    (rule, scope)
}

fn linear_tween(ms: i32) -> Arc<dyn AnimationSpec<f32>> {
    Arc::new(TweenSpec::<f32>::new(ms, 0, LinearEasing))
}

// ---- C++ 3070 -----------------------------------------------------------------------------------------------------

fn animatable_animates_and_can_be_interrupted() {
    let anim = Animatable(0.0);
    let seen: Rc<RefCell<Vec<f32>>> = Rc::default();
    let first_done = Rc::new(Cell::new(false));
    let scope_cell: Rc<RefCell<Option<CoroutineScope>>> = Rc::default();
    let (a, s, sc) = (anim.clone(), seen.clone(), scope_cell.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *sc.borrow_mut() = Some(remember_coroutine_scope());
        s.borrow_mut().push(a.value());
    }));
    let scope = scope_cell.borrow().clone().unwrap();
    let (a, done) = (anim.clone(), first_done.clone());
    scope.launch(async move {
        named_args!(a.animate_to(100.0, linear_tween(1000))).await;
        done.set(true);
    });
    rule.advance_time_by(200);
    let mid = anim.peek();
    assert!(mid > 5.0 && mid < 40.0, "{mid}");
    assert!(anim.is_running());
    let a = anim.clone();
    scope.launch(async move {
        named_args!(a.animate_to(0.0, Arc::new(tween::<f32>()) as Arc<dyn AnimationSpec<f32>>)).await;
    });
    rule.wait_for_idle();
    // Compose: the interrupted animateTo throws its CancellationException where it is awaited, so what follows
    // it doesn't run (C++'s returned there and went on).
    assert!(!first_done.get());
    assert_eq!(anim.peek(), 0.0);
    assert!(!anim.is_running());
    assert!(seen.borrow().len() > 10, "the reader recomposed on animation frames: {}", seen.borrow().len());
}

fn an_interrupting_animation_starts_from_the_value_and_velocity() {
    let (rule, scope) = rule_and_scope();
    let anim = Animatable(0.0);
    let a = anim.clone();
    scope.launch(async move {
        named_args!(a.animate_to(100.0, linear_tween(1000))).await;
    });
    rule.advance_time_by(320);
    let (value, velocity) = (anim.peek(), anim.velocity());
    assert_near(velocity, 100.0, 1.0); // 100 per second
    let a = anim.clone();
    let started: Rc<RefCell<Vec<(f32, f32)>>> = Rc::default();
    let st = started.clone();
    scope.launch(async move {
        let st = st.clone();
        let block: Box<dyn for<'x> FnMut(&'x Animatable<f32, AnimationVector1D>)> =
            Box::new(move |a| st.borrow_mut().push((a.peek(), a.velocity())));
        named_args!(a.animate_to(0.0, a.default_spring_spec(), a.velocity(), Some(block))).await;
    });
    // (the new one's first frame is the old one's last frame: at once, where it was, as fast)
    let first = started.borrow()[0];
    assert_eq!(first.0, value);
    assert_near(first.1, velocity, 1.0);
    rule.wait_for_idle();
    assert_eq!(anim.peek(), 0.0);
    assert_eq!(anim.target_value(), 0.0);
}

// ---- C++ 24036 -----------------------------------------------------------------------------------------------------

fn animatable_carries_a_velocity_into_springs_and_decays() {
    let (rule, scope) = rule_and_scope();
    let a = Animatable(0.0);
    let b = a.clone();
    scope.launch(async move {
        b.animate_decay(1000.0, exponential_decay::<f32>()).await;
    });
    rule.advance_frame();
    rule.advance_frame();
    let (v1, p1) = (a.velocity(), a.peek());
    rule.advance_time_by(200);
    assert!(p1 > 0.0 && v1 > 0.0 && a.velocity() < v1 && a.peek() > p1 && a.is_running());
    rule.advance_time_by(3000);
    assert!(!a.is_running() && a.velocity() == 0.0);
    assert_near(a.peek(), 1000.0 / 4.2, 0.5);
    assert_near(a.target_value(), 1000.0 / 4.2, 0.5);

    // A bound ends it there.
    let b = a.clone();
    let result: Rc<RefCell<Option<AnimationResult<f32, AnimationVector1D>>>> = Rc::default();
    let r = result.clone();
    scope.launch(async move {
        b.snap_to(0.0).await;
        named_args!(b.update_bounds(Some(0.0), Some(100.0)));
        *r.borrow_mut() = Some(b.animate_decay(1000.0, exponential_decay::<f32>()).await);
    });
    rule.advance_time_by(500);
    assert!(!a.is_running() && a.peek() == 100.0 && a.velocity() == 0.0);
    // (Compose keeps the decay's target as the target value; C++ took the bound)
    assert_near(a.target_value(), 1000.0 / 4.2, 0.5);
    let result = result.borrow_mut().take().unwrap();
    assert_eq!(result.end_reason, AnimationEndReason::BoundReached);
    assert_eq!(result.end_state.peek(), 100.0);
    assert!(result.end_state.velocity() > 0.0); // as fast as it went when it hit the bound
    let b = a.clone();
    scope.launch(async move { b.snap_to(250.0).await });
    assert_eq!(a.peek(), 100.0); // kept within
    assert_eq!((a.lower_bound(), a.upper_bound()), (Some(0.0), Some(100.0)));
    named_args!(a.update_bounds(None, None));
    let b = a.clone();
    scope.launch(async move { b.snap_to(0.0).await });

    // Thrown from the target: a spring carries it out and back.
    let b = a.clone();
    scope.launch(async move {
        named_args!(b.animate_to(0.0, Arc::new(spring::<f32>()) as Arc<dyn AnimationSpec<f32>>, 2000.0)).await;
    });
    let mut most = 0.0f32;
    for _ in 0..60 {
        rule.advance_frame();
        most = most.max(a.peek());
    }
    assert!(most > 15.0 && a.peek() == 0.0 && !a.is_running(), "{most}"); // v0 / (e · sqrt(stiffness)): about 19

    // A tween's speed, measured.
    let b = a.clone();
    scope.launch(async move {
        let spec: Arc<dyn AnimationSpec<f32>> = Arc::new(TweenSpec::<f32>::new(160, 0, FastOutSlowInEasing));
        named_args!(b.animate_to(100.0, spec)).await;
    });
    rule.advance_frame();
    rule.advance_frame();
    assert!(a.velocity() > 100.0);
    rule.wait_for_idle();
    assert!(a.peek() == 100.0 && a.velocity() == 0.0);
}

fn animatable_bounds_must_be_in_order() {
    let a = Animatable(5.0);
    named_args!(a.update_bounds(Some(10.0), None));
    // Not running: its value moved within.
    assert_eq!(a.peek(), 10.0);
    let wrong =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| named_args!(a.update_bounds(Some(20.0), Some(10.0)))));
    assert!(wrong.is_err());
    // Only the upper one changed: the lower stays.
    named_args!(a.update_bounds(upper_bound = Some(50.0)));
    assert_eq!((a.lower_bound(), a.upper_bound()), (Some(10.0), Some(50.0)));
}

// ---- C++ 24150's Animatable part ------------------------------------------------------------------------------------

fn keyframes_animate_an_animatable() {
    let (rule, scope) = rule_and_scope();
    let scale = Animatable(1.0);
    let s = scale.clone();
    scope.launch(async move {
        let spec: Arc<dyn AnimationSpec<f32>> = Arc::new(keyframes(|k: &mut KeyframesSpecConfig<f32>| {
            k.duration_millis = 400;
            k.at(1.3, 100);
            k.at(0.9, 250);
        }));
        named_args!(s.animate_to(1.0, spec)).await;
    });
    let mut most = 1.0f32;
    for _ in 0..30 {
        rule.advance_frame();
        most = most.max(scale.peek());
    }
    assert!(most > 1.25 && scale.peek() == 1.0 && !scale.is_running(), "{most}");
}

// ---- stop, snap_to -----------------------------------------------------------------------------------------------------

fn stop_and_snap_to_cancel_the_running_animation() {
    let (rule, scope) = rule_and_scope();
    let a = Animatable(0.0);
    let cancelled = Rc::new(Cell::new(false));
    let (b, c) = (a.clone(), cancelled.clone());
    scope.launch(async move {
        /// Notes being dropped where it waits (Kotlin's `catch (e: CancellationException)`).
        struct Cancelled(Rc<Cell<bool>>, bool);
        impl Drop for Cancelled {
            fn drop(&mut self) {
                if self.1 {
                    self.0.set(true);
                }
            }
        }
        let mut guard = Cancelled(c, true);
        named_args!(b.animate_to(100.0, linear_tween(1000))).await;
        guard.1 = false;
    });
    rule.advance_time_by(160);
    let at = a.peek();
    let b = a.clone();
    scope.launch(async move { b.stop().await });
    assert!(!a.is_running() && a.velocity() == 0.0 && a.peek() == at);
    // (its coroutine goes where it awaits once it runs again, as Kotlin's resumes with the exception)
    rule.advance_frame();
    assert!(cancelled.get());
    rule.advance_time_by(100);
    assert_eq!(a.peek(), at);
    // snap_to: at once.
    let b = a.clone();
    scope.launch(async move { b.snap_to(42.0).await });
    assert_eq!((a.peek(), a.target_value()), (42.0, 42.0));
}

// ---- MutatorMutex ---------------------------------------------------------------------------------------------------

fn a_mutator_mutex_runs_one_mutation_at_a_time() {
    let (rule, scope) = rule_and_scope();
    let mutex = MutatorMutex::new();
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let push = |log: &Rc<RefCell<Vec<String>>>, s: &str| log.borrow_mut().push(s.to_string());
    // A user's drag, then a program's animation: the animation can't interrupt the drag, it is cancelled.
    let (m, l) = (mutex.clone(), log.clone());
    scope.launch(async move {
        let l2 = l.clone();
        named_args!(m.mutate(MutatePriority::UserInput, async move {
            push(&l2, "drag starts");
            delay_ms(100).await;
            push(&l2, "drag ends");
        }))
        .await;
        push(&l, "after the drag");
    });
    let (m, l) = (mutex.clone(), log.clone());
    scope.launch(async move {
        let l2 = l.clone();
        m.mutate(async move { push(&l2, "animation runs") }).await;
        push(&l, "after the animation");
    });
    rule.advance_time_by(200);
    assert_eq!(*log.borrow(), ["drag starts", "drag ends", "after the drag"]);
    // A drag interrupts an animation: the animation's coroutine is cancelled, the drag starts once it let go.
    log.borrow_mut().clear();
    let (m, l) = (mutex.clone(), log.clone());
    scope.launch(async move {
        let l2 = l.clone();
        m.mutate(async move {
            push(&l2, "animation starts");
            delay_ms(1000).await;
            push(&l2, "animation ends");
        })
        .await;
        push(&l, "after the animation");
    });
    rule.advance_time_by(50);
    let (m, l) = (mutex.clone(), log.clone());
    scope.launch(async move {
        let l2 = l.clone();
        named_args!(m.mutate(MutatePriority::UserInput, async move { push(&l2, "drag") })).await;
        push(&l, "after the drag");
    });
    rule.advance_time_by(2000);
    assert_eq!(*log.borrow(), ["animation starts", "drag", "after the drag"]);
    // try_mutate: only while nothing holds the lock.
    assert!(mutex.try_mutate(|| push(&log, "tried")));
    let (m, l) = (mutex.clone(), log.clone());
    scope.launch(async move {
        m.mutate(async move {
            delay_ms(100).await;
            let _ = l;
        })
        .await;
    });
    assert!(!mutex.try_mutate(|| panic!("ran while locked")));
    rule.advance_time_by(200);
    assert!(mutex.try_mutate(|| {}));
    // mutate_with: the receiver given to the block; the value of the mutation.
    let (m, out) = (mutex.clone(), Rc::new(Cell::new(0)));
    let o = out.clone();
    scope.launch(async move {
        let v = m.mutate_with(20, |x: i32| async move { x + 1 }).await;
        o.set(v);
    });
    assert_eq!(out.get(), 21);
}

// ---- animate, animate_decay, AnimationState ------------------------------------------------------------------------

fn animate_gives_each_frames_value_and_velocity() {
    let (rule, scope) = rule_and_scope();
    let frames: Rc<RefCell<Vec<(f32, f32)>>> = Rc::default();
    let done = Rc::new(Cell::new(false));
    let (f, d) = (frames.clone(), done.clone());
    scope.launch(async move {
        let f = f.clone();
        named_args!(animate(0.0, 10.0, 0.0, linear_tween(100), move |v, vel| f.borrow_mut().push((v, vel)))).await;
        d.set(true);
    });
    rule.wait_for_idle();
    assert!(done.get());
    let frames = frames.borrow();
    assert_eq!(frames[0], (0.0, 0.0)); // its first frame: the start
    assert_near(frames[1].0, 1.6, 1e-3); // 16 ms of 100
    assert_near(frames[1].1, 100.0, 1.0);
    assert_eq!(frames.last().unwrap().0, 10.0);
    // A decay: from 0 at 1000 a second, to rest at about 238.
    let last = Rc::new(Cell::new(0.0f32));
    let l = last.clone();
    scope.launch(async move {
        animate_decay(0.0, 1000.0, FloatExponentialDecaySpec::default(), move |v, _| l.set(v)).await
    });
    rule.wait_for_idle();
    assert_near(last.get(), 1000.0 / 4.2, 0.5);
    // Any type through its converter.
    let offsets: Rc<RefCell<Vec<Offset>>> = Rc::default();
    let o = offsets.clone();
    scope.launch(async move {
        animate_value(Offset::VectorConverter, Offset::Zero, Offset::new(10.0, -10.0), move |v, _| {
            o.borrow_mut().push(v)
        })
        .await;
    });
    rule.wait_for_idle();
    assert_eq!(*offsets.borrow().last().unwrap(), Offset::new(10.0, -10.0));
}

fn animation_states_animate_one_after_another() {
    let (rule, scope) = rule_and_scope();
    let state = float_animation_state(0.0);
    let s = state.clone();
    let scopes: Rc<RefCell<Vec<(i64, f32)>>> = Rc::default();
    let sc = scopes.clone();
    scope.launch(async move {
        named_args!(s.animate_to(10.0, linear_tween(100))).await;
        let sc = sc.clone();
        let block: Box<dyn for<'x> FnMut(&'x AnimationScope<f32, AnimationVector1D>)> =
            Box::new(move |scope| sc.borrow_mut().push((scope.start_time_nanos(), scope.value())));
        // Sequential: from the last one's last frame, not the next frame.
        named_args!(s.animate_to(20.0, linear_tween(100), true, block)).await;
    });
    rule.wait_for_idle();
    assert_eq!(state.peek(), 20.0);
    assert!(state.is_finished() && !state.is_running());
    let scopes = scopes.borrow();
    assert_eq!(scopes[0].1, 10.0); // started where the first ended, at its last frame
    assert!(scopes.windows(2).all(|w| w[0].0 == w[1].0));
    // The block can end it early.
    let s = state.clone();
    scope.launch(async move {
        let block: Box<dyn for<'x> FnMut(&'x AnimationScope<f32, AnimationVector1D>)> = Box::new(|scope| {
            if scope.value() < 15.0 {
                scope.cancel_animation();
            }
        });
        named_args!(s.animate_to(0.0, linear_tween(100), false, block)).await;
    });
    rule.wait_for_idle();
    assert!(state.peek() < 15.0 && state.peek() > 0.0 && !state.is_running());
    // A decay of the state: from its value at its velocity.
    let flung = AnimationState::new(
        f32::VectorConverter,
        0.0,
        Some(AnimationVector1D(420.0)),
        AnimationConstants::UnspecifiedTime,
        AnimationConstants::UnspecifiedTime,
        false,
    );
    let f = flung.clone();
    scope.launch(async move { f.animate_decay(exponential_decay::<f32>()).await });
    rule.wait_for_idle();
    assert_near(flung.peek(), 100.0, 0.05);
    // Its copy: these values, the rest its own.
    let copy = named_args!(flung.copy(value = 3.0));
    assert_eq!((copy.peek(), copy.last_frame_time_nanos()), (3.0, flung.last_frame_time_nanos()));
}

// ---- The coroutine's context -------------------------------------------------------------------------------------

fn a_motion_duration_scale_stretches_animations() {
    let (rule, scope) = rule_and_scope();
    let a = Animatable(0.0);
    let b = a.clone();
    scope.launch(with_context(MotionDurationScale::new(2.0), async move {
        named_args!(b.animate_to(100.0, linear_tween(160))).await;
    }));
    rule.advance_time_by(160);
    assert!(a.is_running());
    assert_near(a.peek(), 50.0, 6.0); // half way at the spec's end
    rule.advance_time_by(200);
    assert_eq!(a.peek(), 100.0);
    // 0: done at its first frame.
    let b = a.clone();
    scope.launch(with_context(MotionDurationScale::new(0.0), async move {
        named_args!(b.animate_to(0.0, linear_tween(1000))).await;
    }));
    rule.advance_frame();
    assert!(a.peek() == 0.0 && !a.is_running());
    // Children of a scope made inside see it too.
    let seen = Rc::new(Cell::new(0.0f32));
    let s = seen.clone();
    scope.launch(with_context(MotionDurationScale::new(3.0), async move {
        coroutine_scope(|inner| async move {
            inner.launch(async move {
                s.set(current_coroutine_context().get::<MotionDurationScale>().unwrap().scale_factor());
            });
        })
        .await;
    }));
    assert_eq!(seen.get(), 3.0);
}

fn an_infinite_animation_policy_sees_infinite_animations() {
    let (rule, scope) = rule_and_scope();
    let a = Animatable(0.0);
    // A policy counting the infinite animation's frames.
    let frames = Rc::new(Cell::new(0));
    let f = frames.clone();
    let counting = InfiniteAnimationPolicy::new(move |block| {
        f.set(f.get() + 1);
        block
    });
    let forever: Arc<dyn AnimationSpec<f32>> = Arc::new(InfiniteRepeatableSpec::new(
        TweenSpec::<f32>::new(100, 0, LinearEasing),
        RepeatMode::Reverse,
        StartOffset::default(),
    ));
    let b = a.clone();
    let spec = forever.clone();
    let job = scope.launch(with_context(counting, async move {
        named_args!(b.animate_to(10.0, spec)).await;
    }));
    rule.advance_time_by(160);
    assert!(frames.get() >= 9 && a.is_running());
    // A finite one doesn't go through it.
    job.cancel();
    let counted = frames.get();
    let b = a.clone();
    let f = frames.clone();
    scope.launch(with_context(
        InfiniteAnimationPolicy::new(move |block| {
            f.set(f.get() + 1000);
            block
        }),
        async move {
            named_args!(b.animate_to(0.0, linear_tween(50))).await;
        },
    ));
    rule.advance_time_by(100);
    assert_eq!(frames.get(), counted);
    // A test's policy: infinite animations cancelled (Compose's ui-test, while its clock advances on its own).
    let cancelled = Rc::new(Cell::new(false));
    let c = cancelled.clone();
    let cancelling = InfiniteAnimationPolicy::new(|_block| Box::pin(cancel_current::<()>()));
    let b = a.clone();
    scope.launch(with_context(cancelling, async move {
        named_args!(b.animate_to(10.0, forever)).await;
        c.set(true);
    }));
    rule.advance_time_by(100);
    assert!(!cancelled.get() && !a.is_running());
}

// ---- Other types -------------------------------------------------------------------------------------------------------

fn animatables_of_offsets_and_colors() {
    let (rule, scope) = rule_and_scope();
    let offset = Animatable((Offset::Zero, Offset::VectorConverter));
    let o = offset.clone();
    scope.launch(async move {
        let spec: Arc<dyn AnimationSpec<Offset>> = Arc::new(TweenSpec::<Offset>::new(100, 0, LinearEasing));
        named_args!(o.animate_to(Offset::new(100.0, 50.0), spec)).await;
    });
    rule.advance_time_by(48);
    let mid = offset.peek();
    assert!(mid.x > 30.0 && mid.x < 70.0 && near(mid.y, mid.x / 2.0, 0.5));
    rule.wait_for_idle();
    assert_eq!(offset.peek(), Offset::new(100.0, 50.0));
    // A color, in Oklab (alpha first in its vector).
    let color = Animatable(Color::Black);
    let c = color.clone();
    scope.launch(async move {
        let spec: Arc<dyn AnimationSpec<Color>> = Arc::new(TweenSpec::<Color>::new(100, 0, LinearEasing));
        named_args!(c.animate_to(Color::White, spec)).await;
    });
    rule.advance_time_by(48);
    let gray = color.peek();
    // (32 ms of 100 along Oklab's lightness: about a fifth of sRGB's)
    assert!(gray.red() > 0.1 && gray.red() < 0.8 && near(gray.red(), gray.green(), 1e-3), "{gray:?}");
    rule.wait_for_idle();
    assert_eq!(color.peek().to_argb(), Color::White.to_argb());
    // A float with a visibility threshold of its own: its spring ends sooner.
    let coarse = Animatable((0.0, 5.0));
    let fine = Animatable(0.0);
    let ends: Rc<RefCell<Vec<i64>>> = Rc::default();
    for a in [coarse.clone(), fine.clone()] {
        let e = ends.clone();
        scope.launch(async move {
            let result = a.animate_to(100.0).await;
            e.borrow_mut().push(result.end_state.finished_time_nanos());
        });
    }
    rule.wait_for_idle();
    let ends = ends.borrow();
    assert!(ends[0] < ends[1]);
    assert_eq!((coarse.peek(), fine.peek()), (100.0, 100.0));
}

fn androids_fling_takes_the_hosts_density() {
    let density = mutable_state_of(1.0f32);
    let decays: Rc<RefCell<Vec<FloatDecaySpecOf<f32>>>> = Rc::default();
    let (d, s) = (density.clone(), decays.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        KompozitionLocalProvider(&[LocalDensity.provides(Density::new(d.get(), 1.0))], move || {
            s.borrow_mut().push(remember_spline_based_decay::<f32>());
        });
    }));
    let at_one = decays.borrow().last().unwrap().calculate_float_target_value(0.0, 3200.0);
    assert_near(at_one, 1464.1, 1.0);
    density.set(2.0);
    rule.wait_for_idle();
    let at_two = decays.borrow().last().unwrap().calculate_float_target_value(0.0, 6400.0);
    assert_near(at_two, 2.0 * 1464.1, 2.0);
    // An Animatable flung along it.
    let (rule, scope) = rule_and_scope();
    let a = Animatable(0.0);
    let b = a.clone();
    scope.launch(async move {
        b.animate_decay(3200.0, spline_based_decay::<f32>(Density::new(1.0, 1.0))).await;
    });
    rule.advance_frame();
    rule.advance_frame();
    assert_near(a.velocity(), 3200.0, 300.0);
    rule.advance_time_by(1500);
    assert_near(a.peek(), 1464.1, 1.0);
    assert!(!a.is_running());
}

tests!(
    animatable_animates_and_can_be_interrupted,
    an_interrupting_animation_starts_from_the_value_and_velocity,
    animatable_carries_a_velocity_into_springs_and_decays,
    animatable_bounds_must_be_in_order,
    keyframes_animate_an_animatable,
    stop_and_snap_to_cancel_the_running_animation,
    a_mutator_mutex_runs_one_mutation_at_a_time,
    animate_gives_each_frames_value_and_velocity,
    animation_states_animate_one_after_another,
    a_motion_duration_scale_stretches_animations,
    an_infinite_animation_policy_sees_infinite_animations,
    animatables_of_offsets_and_colors,
    androids_fling_takes_the_hosts_density,
);
