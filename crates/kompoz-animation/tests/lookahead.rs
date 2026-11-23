//! Approach layouts animated to their lookahead size with a DeferredTargetAnimation: the C++ Kompoz's
//! approach_layout_approaches_the_lookahead_size, and DeferredTargetAnimation as Compose has it.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::settle;

use kompoz_animation::*;
use kompoz_coroutines::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

fn width(rule: &KompozTestRule, tag: &str) -> f32 {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root().width()
}

fn linear(ms: i32) -> Arc<dyn FiniteAnimationSpec<Size>> {
    Arc::new(TweenSpec::<Size>::new(ms, 0, LinearEasing))
}

/// The approach: what it wraps measured at `size` (fixed), placed at its top left.
fn measured_at(s: &ApproachMeasureScope, m: &dyn Measurable, size: IntSize) -> MeasureResult {
    let p = m.measure(Constraints::fixed(size.width, size.height));
    s.layout(p.width(), p.height(), move |pl| pl.place(&p, 0, 0))
}

fn approach_layout_approaches_the_lookahead_size() {
    // C++'s approach_layout_approaches_the_lookahead_size. Adapted: approach layouts outside a LookaheadScope are
    // lookahead roots of their own, as Compose's; DeferredTargetAnimation as Compose's (its converter given, its spec
    // at `update_target`; `pending_target` the last target given, not none once idle); the tag of the one bigger
    // than it may be goes before its size (its node's bounds: Compose's semantics bounds are its coordinator's).
    let wide = mutable_state_of(false);
    let approaching = mutable_state_of(true);
    let lookaheads = Rc::new(RefCell::new(Vec::<IntSize>::new()));
    let seen = Rc::new(RefCell::new(Vec::<f32>::new()));
    let size = DeferredTargetAnimation::new(Size::VectorConverter);
    let outer: Rc<RefCell<Option<CoroutineScope>>> = Rc::default();
    let (w, a, l, s, o, d) =
        (wide.clone(), approaching.clone(), lookaheads.clone(), seen.clone(), outer.clone(), size.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let scope = remember_coroutine_scope();
        *o.borrow_mut() = Some(scope.clone());
        let (w, a, l, s, d) = (w.clone(), a.clone(), l.clone(), s.clone(), d.clone());
        Column(move || {
            let (w, a, l, s, d, scope) = (w.clone(), a.clone(), l.clone(), s.clone(), d.clone(), scope.clone());
            // By hand: half as wide as it is going to be, while approaching (inside a box as wide as it is going
            // to be: asked from above it, a lookahead root answers with its lookahead intrinsics).
            let (w1, a1, l1) = (w.clone(), a.clone(), l.clone());
            Box(
                modifier = Modifier.width_intrinsic(IntrinsicSize::Max).test_tag("sarmal"),
                content = move || {
                    let (w1, a1, l1) = (w1.clone(), a1.clone(), l1.clone());
                    Box(
                        modifier = Modifier.approach_layout(
                            is_measurement_approach_in_progress = move |target| {
                                l1.borrow_mut().push(target);
                                a1.get()
                            },
                            approach_measure = |s: &ApproachMeasureScope, m: &dyn Measurable, _| {
                                let target = s.lookahead_size();
                                measured_at(s, m, IntSize::new(target.width / 2, target.height))
                            },
                        ),
                        content = move || {
                            let width = if w1.get() { 200 } else { 100 };
                            Box(modifier = Modifier.size_wh(width.dp(), 50.dp()).test_tag("yarım"));
                        },
                    );
                },
            );
            // Bigger than it may be: kept to its constraints.
            Box(modifier = Modifier.test_tag("taşan").size_wh(80.dp(), 50.dp()).approach_layout(
                is_measurement_approach_in_progress = |_| true,
                approach_measure =
                    |s: &ApproachMeasureScope, m: &dyn Measurable, _| measured_at(s, m, IntSize::new(300, 50)),
            ));
            // Animated to it.
            let (d1, d2, scope1, scope2) = (d.clone(), d.clone(), scope.clone(), scope.clone());
            Box(
                modifier = Modifier.approach_layout(
                    is_measurement_approach_in_progress = move |target| {
                        named_args!(d1.update_target(target.to_size(), &scope1, animation_spec = linear(200)));
                        !d1.is_idle()
                    },
                    approach_measure = move |sc: &ApproachMeasureScope, m: &dyn Measurable, _| {
                        let target = sc.lookahead_size().to_size();
                        let now = named_args!(d2.update_target(target, &scope2, animation_spec = linear(200)));
                        s.borrow_mut().push(now.width);
                        measured_at(sc, m, IntSize::new(now.width.round() as i32, now.height.round() as i32))
                    },
                ),
                content = move || {
                    let width = if w.get() { 200 } else { 100 };
                    Box(modifier = Modifier.size_wh(width.dp(), 50.dp()).test_tag("canlı"));
                },
            );
        });
    }));
    rule.main_clock().set_auto_advance(false);
    assert!(near(width(&rule, "yarım"), 50.0) && lookaheads.borrow().last().is_some_and(|l| l.width == 100));
    // Its first target as it is.
    assert!(near(width(&rule, "canlı"), 100.0) && size.is_idle());
    assert!(seen.borrow().iter().all(|w| near(*w, 100.0)));
    assert_eq!(size.pending_target(), Some(Size::new(100.0, 50.0)));
    let fresh = DeferredTargetAnimation::new(f32::VectorConverter);
    let scope = outer.borrow().clone().unwrap();
    assert!(near(fresh.update_target(5.0, &scope), 5.0) && fresh.is_idle());
    assert!(near(width(&rule, "sarmal"), 100.0) && near(width(&rule, "taşan"), 80.0));
    approaching.set(false);
    settle(&rule);
    // There: as the lookahead.
    assert!(near(width(&rule, "yarım"), 100.0));
    wide.set(true);
    rule.advance_time_by(100);
    let mid = width(&rule, "canlı");
    assert!(mid > 110.0 && mid < 190.0 && !size.is_idle(), "{mid}");
    assert!(size.pending_target().is_some_and(|t| near(t.width, 200.0)));
    settle(&rule);
    assert!(near(width(&rule, "canlı"), 200.0) && size.is_idle());
    assert!(size.pending_target().is_some_and(|t| near(t.width, 200.0)));
    assert!(lookaheads.borrow().last().is_some_and(|l| l.width == 200));
}

fn a_deferred_target_animation_animates_to_each_new_target() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {}));
    let scope = CoroutineScope::new(rule.host().dispatcher());
    let animation = DeferredTargetAnimation::new(f32::VectorConverter);
    assert_eq!(animation.pending_target(), None);
    // Not set up: idle (no target, nothing running).
    assert!(animation.is_idle());
    // The first target: set up there, as it is.
    assert_eq!(animation.update_target(10.0, &scope), 10.0);
    assert!(animation.is_idle() && animation.pending_target() == Some(10.0));
    // A new one: animated to (a spring by default), its value now the one it had.
    assert_eq!(animation.update_target(20.0, &scope), 10.0);
    assert!(!animation.is_idle() && animation.pending_target() == Some(20.0));
    rule.advance_time_by(48);
    let between = animation.update_target(20.0, &scope);
    assert!(between > 10.0 && between < 20.0, "{between}");
    rule.wait_for_idle();
    assert_eq!(animation.update_target(20.0, &scope), 20.0);
    assert!(animation.is_idle());
    // Given another spec.
    let tween: Arc<dyn FiniteAnimationSpec<f32>> = Arc::new(TweenSpec::<f32>::new(160, 0, LinearEasing));
    named_args!(animation.update_target(0.0, &scope, animation_spec = tween.clone()));
    rule.advance_time_by(80);
    // (its first frame starts it: 64 ms of its 160 at the fifth)
    let part = named_args!(animation.update_target(0.0, &scope, animation_spec = tween));
    assert!((part - 12.0).abs() < 0.01, "{part}");
}

tests!(approach_layout_approaches_the_lookahead_size, a_deferred_target_animation_animates_to_each_new_target);
