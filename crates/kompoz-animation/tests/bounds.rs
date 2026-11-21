//! Lookahead animations of a layout's size and bounds: `Modifier.animate_content_size` (the C++ Kompoz's
//! animate_content_size_follows_the_content and double_measured_children_settle's non-Material part, then Compose's
//! own behaviour: the finished listener, the alignment, a lookahead scope), `Modifier.animate_bounds` (C++'s
//! animate_bounds_glides_to_a_new_place_and_size in Compose's form, its `modifier` and motion frame of reference, a
//! scroller in the scope), and the lookahead animation visual debugging.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::near;
use kompoz_animation::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn linear<T: 'static>(ms: i32) -> TweenSpec<T> {
    TweenSpec::<T>::new(ms, 0, LinearEasing)
}

const Red: Color = Color::from_argb(0xFFFF0000);

/// Where the red rect was drawn last.
fn red(rule: &KompozTestRule) -> Option<Rect> {
    let canvas = rule.draw();
    canvas.rects.iter().zip(&canvas.rect_colors).rev().find(|(_, c)| **c == Red).map(|(r, _)| *r)
}

// ---- animate_content_size --------------------------------------------------------------------------------------

fn animate_content_size_follows_the_content() {
    // C++'s animate_content_size_follows_the_content.
    let big = mutable_state_of(false);
    let shown = mutable_state_of(false);
    let (b, s) = (big.clone(), shown.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        if s.get() {
            let b = b.clone();
            let spec: Arc<dyn FiniteAnimationSpec<IntSize>> = Arc::new(linear::<IntSize>(200));
            Box(
                modifier = Modifier.test_tag("box").animate_content_size(animation_spec = spec),
                content = move || {
                    let side = if b.get() { 200 } else { 100 };
                    Box(modifier = Modifier.size(side.dp()));
                },
            );
        }
    }));
    shown.set(true);
    rule.advance_frame();
    // The first size isn't animated.
    assert!(near(bounds(&rule, "box").width(), 100.0, 0.5));
    big.set(true);
    rule.advance_time_by(112);
    let mid = bounds(&rule, "box").width();
    assert!(mid > 120.0 && mid < 180.0, "{mid}");
    // Turned back half way: from where it is. (Adapted: Compose's animation frame runs before the layout that
    // turns it back, so the first frame shows one more step on (8 px of the 200 ms tween's 100); C++ turned at once.)
    big.set(false);
    rule.advance_frame();
    let back = bounds(&rule, "box").width();
    assert!(back <= mid + 8.5 && back > 100.0, "{back} after {mid}");
    rule.advance_frame();
    let further = bounds(&rule, "box").width();
    assert!(further < back, "{further} after {back}");
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "box").width(), 100.0, 0.5));
    big.set(true);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "box").width(), 200.0, 0.5));
    // And back from its end.
    big.set(false);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "box").height(), 100.0, 0.5));
}

fn animate_content_size_tells_where_it_started_and_aligns_its_content() {
    // Compose's: the finished listener gets the size an animation started from and the one it reached (not an
    // interrupted one's: the next starts where it was); the content is placed by the alignment in the size along the
    // way, clipped to it.
    let big = mutable_state_of(false);
    let finished = Rc::new(RefCell::new(Vec::<(IntSize, IntSize)>::new()));
    let (b, f) = (big.clone(), finished.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let b = b.clone();
        let f = f.clone();
        let listener: SizeAnimationFinishedListener = Rc::new(move |from, to| f.borrow_mut().push((from, to)));
        let spec: Arc<dyn FiniteAnimationSpec<IntSize>> = Arc::new(linear::<IntSize>(160));
        Column(move || {
            let b = b.clone();
            Box(
                modifier = Modifier.test_tag("kutu").animate_content_size(
                    animation_spec = spec.clone(),
                    alignment = Alignment::BottomEnd,
                    finished_listener = Some(listener.clone()),
                ),
                content = move || {
                    let side = if b.get() { 100 } else { 50 };
                    Box(modifier = Modifier.size(side.dp()).background(Red).test_tag("içerik"));
                },
            );
        });
    }));
    assert!(finished.borrow().is_empty());
    big.set(true);
    rule.advance_time_by(80);
    let outer = bounds(&rule, "kutu");
    let inner = bounds(&rule, "içerik");
    assert!(outer.width() > 55.0 && outer.width() < 95.0, "{outer:?}");
    // (bottom end: the content's bottom right at the box's)
    assert!(near(inner.right, outer.right, 0.5) && near(inner.bottom, outer.bottom, 0.5), "{inner:?} in {outer:?}");
    // Drawn clipped to it.
    let canvas = rule.draw();
    assert!(canvas.clips.iter().any(|c| near(c.width(), outer.width(), 0.5)), "{:?}", canvas.clips);
    // Interrupted half way: not told; the next tells where it started.
    big.set(false);
    rule.advance_frame();
    let turned = bounds(&rule, "kutu").width();
    rule.wait_for_idle();
    assert_eq!(finished.borrow().len(), 1, "{:?}", finished.borrow());
    let (from, to) = finished.borrow()[0];
    assert!((from.width as f32 - turned).abs() <= 6.0 && to == IntSize::new(50, 50), "{from:?} → {to:?} ({turned})");
    big.set(true);
    rule.wait_for_idle();
    assert_eq!(finished.borrow().last().copied(), Some((IntSize::new(50, 50), IntSize::new(100, 100))));
    // The overload without an alignment: at the top start.
    let r2 = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            modifier = Modifier.size(80.dp()).animate_content_size_top_start(),
            content = || Box(modifier = Modifier.size(20.dp()).test_tag("köşe")),
        );
    }));
    assert_eq!(bounds(&r2, "köşe").top_left(), Offset::Zero);
}

fn animate_content_size_in_a_lookahead_scope_goes_to_the_lookahead_size() {
    // Compose 1.12's: in a lookahead scope its lookahead pass is the content's size at once; the approach animates to
    // it, the content measured with the lookahead constraints all along (not again on the way).
    let big = mutable_state_of(false);
    let measures = Rc::new(RefCell::new(Vec::<Constraints>::new()));
    let (b, m) = (big.clone(), measures.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (b, m) = (b.clone(), m.clone());
        LookaheadScope(move |_| {
            let (b, m) = (b.clone(), m.clone());
            let spec: Arc<dyn FiniteAnimationSpec<IntSize>> = Arc::new(linear::<IntSize>(160));
            Column(move || {
                let (b, m) = (b.clone(), m.clone());
                Box(
                    modifier = Modifier.test_tag("saran").animate_content_size(animation_spec = spec.clone()),
                    content = move || {
                        let side = if b.get() { 120 } else { 40 };
                        let m = m.clone();
                        Box(modifier = Modifier
                            .layout(move |s, measurable, constraints| {
                                if !s.is_looking_ahead() {
                                    m.borrow_mut().push(constraints);
                                }
                                let p = measurable.measure(constraints);
                                let (w, h) = (p.width(), p.height());
                                s.layout(w, h, move |pl| pl.place(&p, 0, 0))
                            })
                            .size(side.dp()));
                    },
                );
                Box(modifier = Modifier.size(10.dp()).test_tag("altta"));
            });
        });
    }));
    assert!(near(bounds(&rule, "saran").height(), 40.0, 0.5));
    big.set(true);
    measures.borrow_mut().clear();
    rule.advance_time_by(80);
    let mid = bounds(&rule, "saran").height();
    assert!(mid > 50.0 && mid < 110.0, "{mid}");
    // (what is below it moves with it: the approach of the layout's size)
    assert!(near(bounds(&rule, "altta").top, mid, 0.5));
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "saran").height(), 120.0, 0.5));
    // The content measured at the lookahead constraints throughout: the same constraints each time.
    let seen = measures.borrow();
    assert!(seen.windows(2).all(|w| w[0] == w[1]), "{seen:?}");
}

fn double_measured_children_settle() {
    // C++'s double_measured_children_settle, its non-Material part: an animated size in a row of IntrinsicSize.Min
    // (asked its intrinsic height, then measured). Ten frames later nothing composes again or waits; a change
    // settles. (Its Material half, a PrimaryTabRow measuring its tabs twice, is kompoz-material3's tests/tabs.rs
    // `double_measured_children_settle`.)
    let big = mutable_state_of(false);
    let b = big.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let b = b.clone();
        Column(move || {
            let b = b.clone();
            Row(
                modifier = Modifier.height_intrinsic(IntrinsicSize::Min),
                content = move || {
                    let b = b.clone();
                    let spec: Arc<dyn FiniteAnimationSpec<IntSize>> =
                        Arc::new(TweenSpec::<IntSize>::new(100, 0, FastOutSlowInEasing));
                    Box(
                        modifier = Modifier.animate_content_size(animation_spec = spec).test_tag("anim"),
                        content = move || {
                            let side = if b.get() { 80 } else { 40 };
                            Box(modifier = Modifier.size(side.dp()));
                        },
                    );
                    Box(modifier = Modifier.width(2.dp()).fill_max_height());
                },
            );
        });
    }));
    rule.wait_for_idle();
    let before = rule.host().stats().recompositions;
    for _ in 0..10 {
        rule.advance_frame();
    }
    assert_eq!(rule.host().stats().recompositions, before);
    assert!(!rule.host().has_pending_work());
    big.set(true);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "anim").height(), 80.0, 0.5));
    assert!(!rule.host().has_pending_work());
}

// ---- animate_bounds --------------------------------------------------------------------------------------------

fn animate_bounds_glides_to_a_new_place_and_size() {
    // C++'s animate_bounds_glides_to_a_new_place_and_size, in Compose's form (D9): `LookaheadScope { }` takes no
    // modifier (a sized Box around it), `animate_bounds(scope, bounds_transform = …)` with the scope its content gets,
    // its spec a BoundsTransform's. Compose's approach lays it out along the way (C++ laid it out at its target at
    // once, drawn moved): its bounds are where it is drawn.
    let right = mutable_state_of(false);
    let wide = mutable_state_of(false);
    let scroll = ScrollState::new(0);
    let widths = Rc::new(RefCell::new(Vec::<i32>::new()));
    let (r, w, s, ws) = (right.clone(), wide.clone(), scroll.clone(), widths.clone());
    let transform = BoundsTransform(|_, _| linear::<Rect>(400));
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (r, w, ws, transform) = (r.clone(), w.clone(), ws.clone(), transform.clone());
        Column(
            modifier = Modifier.vertical_scroll(&s),
            content = move || {
                let (r, w, ws, transform) = (r.clone(), w.clone(), ws.clone(), transform.clone());
                Box(
                    modifier = Modifier.size_wh(300.dp(), 200.dp()),
                    content = move || {
                        let (r, w, ws, transform) = (r.clone(), w.clone(), ws.clone(), transform.clone());
                        LookaheadScope(move |scope| {
                            let (w, ws, transform, scope) = (w.clone(), ws.clone(), transform.clone(), scope.clone());
                            let alignment = if r.get() { Alignment::BottomEnd } else { Alignment::TopStart };
                            Box(
                                modifier = Modifier.fill_max_size(),
                                content_alignment = alignment,
                                content = move || {
                                    let ws = ws.clone();
                                    let width = if w.get() { 100 } else { 40 };
                                    Box(
                                        modifier = Modifier
                                            .animate_bounds(scope.clone(), bounds_transform = transform.clone())
                                            .size_wh(width.dp(), 40.dp())
                                            .background(Red)
                                            .test_tag("kutu"),
                                        content = move || {
                                            let ws = ws.clone();
                                            Box(modifier =
                                                Modifier.fill_max_size().on_size_changed(move |size: IntSize| {
                                                    ws.borrow_mut().push(size.width)
                                                }));
                                        },
                                    );
                                },
                            );
                        });
                    },
                );
                Box(modifier = Modifier.size_wh(10.dp(), 1000.dp()));
            },
        );
    }));
    let frames = |n: i32| -> Rect {
        let mut last = None;
        for _ in 0..n {
            rule.advance_frame();
            last = red(&rule);
        }
        last.expect("the red box drawn")
    };
    let at = frames(1);
    assert_eq!(at.top_left(), Offset::Zero);
    // To the bottom right corner: its lookahead there at once, laid out and drawn on the way.
    right.set(true);
    let at = frames(1);
    assert_eq!(at.top_left(), Offset::Zero);
    assert_eq!(bounds(&rule, "kutu").top_left(), Offset::Zero);
    assert!(rule.host().has_pending_work());
    // About half way (208 of 400 ms).
    let at = frames(13);
    assert!(at.left > 100.0 && at.left < 160.0 && at.top > 60.0 && at.top < 100.0, "{at:?}");
    assert_eq!(bounds(&rule, "kutu").top_left(), at.top_left());
    let at = frames(20);
    assert_eq!(at.top_left(), Offset::new(260.0, 160.0));
    assert!(!rule.host().has_pending_work());
    // Wider: the size grows along the way, the content laid out at it — never told the size it goes to before it
    // gets there.
    wide.set(true);
    widths.borrow_mut().clear();
    let at = frames(13);
    assert!(!widths.borrow().is_empty() && widths.borrow().iter().all(|w| *w < 100), "{:?}", widths.borrow());
    assert!(at.width() > 55.0 && at.width() < 85.0, "{at:?}");
    assert_eq!(bounds(&rule, "kutu").width(), at.width());
    let at = frames(20);
    assert!(at.width() == 100.0 && at.left == 200.0, "{at:?}");
    assert!(!rule.host().has_pending_work());
    // The scope scrolls with it: nothing animates.
    let sc = scroll.clone();
    kompoz_coroutines::CoroutineScope::new(rule.host().dispatcher()).launch(async move {
        sc.scroll_to(100).await;
    });
    let at = frames(2);
    assert_eq!(at.top, 160.0 - 100.0);
    assert!(!rule.host().has_pending_work());
    // At 2.625 px a dp: placed at whole px along the way (its offset rounded, as Compose's).
    rule.host().set_density(Density::new(2.625, 1.0));
    rule.wait_for_idle();
    right.set(false);
    let at = frames(7);
    let scrolled = scroll.value() as f32;
    assert!(at.left > 1.0 && at.left < 199.0 * 2.625, "{at:?}");
    assert_eq!(at.left, at.left.round());
    assert_eq!(at.top + scrolled, (at.top + scrolled).round());
}

fn animate_bounds_takes_a_modifier_between_and_follows_a_scroller_only_when_asked() {
    // Compose's `modifier`: a layout modifier between the two nodes — its change reaches the parent (the outer node's
    // size) and the content (measured at the animated size) gradually. And a layout in a scroller inside the scope:
    // scrolling moves it at once (a motion frame of reference), unless `animate_motion_frame_of_reference` (it chases).
    let padded = mutable_state_of(false);
    let scroll = ScrollState::new(0);
    let chasing = ScrollState::new(0);
    let (p, s, c) = (padded.clone(), scroll.clone(), chasing.clone());
    let transform = BoundsTransform(|_, _| linear::<Rect>(160));
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (p, s, c, transform) = (p.clone(), s.clone(), c.clone(), transform.clone());
        LookaheadScope(move |scope| {
            let (p, s, c, transform, scope) = (p.clone(), s.clone(), c.clone(), transform.clone(), scope.clone());
            Column(move || {
                let pad = if p.get() { 20 } else { 0 };
                Box(
                    modifier = Modifier.test_tag("dolgulu").animate_bounds(
                        scope.clone(),
                        modifier = Modifier.padding(pad.dp()),
                        bounds_transform = transform.clone(),
                    ),
                    content = || Box(modifier = Modifier.size(40.dp()).test_tag("dolgu-içi")),
                );
                let (scope1, scope2, transform1, transform2) =
                    (scope.clone(), scope.clone(), transform.clone(), transform.clone());
                Column(
                    modifier = Modifier.height(100.dp()).vertical_scroll(&s),
                    content = move || {
                        Box(modifier = Modifier.size(50.dp()));
                        Box(modifier = Modifier
                            .animate_bounds(scope1.clone(), bounds_transform = transform1.clone())
                            .size(30.dp())
                            .test_tag("kaydırılan"));
                        Box(modifier = Modifier.size(300.dp()));
                    },
                );
                Column(
                    modifier = Modifier.height(100.dp()).vertical_scroll(&c),
                    content = move || {
                        Box(modifier = Modifier.size(50.dp()));
                        Box(modifier = Modifier
                            .animate_bounds(
                                scope2.clone(),
                                bounds_transform = transform2.clone(),
                                animate_motion_frame_of_reference = true,
                            )
                            .size(30.dp())
                            .test_tag("kovalayan"));
                        Box(modifier = Modifier.size(300.dp()));
                    },
                );
            });
        });
    }));
    assert_eq!(bounds(&rule, "dolgulu").size(), Size::new(40.0, 40.0));
    padded.set(true);
    rule.advance_time_by(80);
    // The parent sees the padded size on its way (the outer node's); the content, its own size all along, glides to
    // where the padding puts it (the inner node's place).
    let mid = bounds(&rule, "dolgulu").size();
    assert!(mid.width > 45.0 && mid.width < 75.0, "{mid:?}");
    let inner = bounds(&rule, "dolgu-içi");
    assert!(inner.width() == 40.0 && inner.left > 2.0 && inner.left < 18.0, "{inner:?}");
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "dolgulu").size(), Size::new(80.0, 80.0));
    assert_eq!(bounds(&rule, "dolgu-içi"), Rect::new(20.0, 20.0, 60.0, 60.0));
    // Scrolled: at once.
    let top = bounds(&rule, "kaydırılan").top;
    let chaser_top = bounds(&rule, "kovalayan").top;
    let (s2, c2) = (scroll.clone(), chasing.clone());
    kompoz_coroutines::CoroutineScope::new(rule.host().dispatcher()).launch(async move {
        s2.scroll_to(40).await;
        c2.scroll_to(40).await;
    });
    rule.advance_frame();
    assert_eq!(bounds(&rule, "kaydırılan").top, top - 40.0);
    // Chasing the scroll: still where it was, then gliding there.
    rule.advance_frame();
    let chased = bounds(&rule, "kovalayan").top;
    assert!(chased > chaser_top - 40.0, "{chased} from {chaser_top}");
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "kovalayan").top, chaser_top - 40.0);
    assert!(!rule.host().has_pending_work());
}

fn lookahead_animation_visual_debugging_outlines_animations() {
    // Compose's LookaheadAnimationVisualDebugging: an animate_bounds at rest outlined in the inactive color; on its
    // way, outlined in its own color with its target outlined at the target (CustomizedLookaheadAnimationVisualDebugging
    // gives that color), with the path between and where it is on it.
    let moved = mutable_state_of(false);
    let enabled = mutable_state_of(true);
    let (m, e) = (moved.clone(), enabled.clone());
    let inactive = Color::from_argb(0xFF00FF00);
    let own = Color::from_argb(0xFF0000FF);
    let transform = BoundsTransform(|_, _| linear::<Rect>(160));
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (m, transform) = (m.clone(), transform.clone());
        LookaheadAnimationVisualDebugging(
            is_enabled = e.get(),
            inactive_element_color = inactive,
            content = move || {
                let (m, transform) = (m.clone(), transform.clone());
                CustomizedLookaheadAnimationVisualDebugging(own, move || {
                    let (m, transform) = (m.clone(), transform.clone());
                    LookaheadScope(move |scope| {
                        let (transform, scope) = (transform.clone(), scope.clone());
                        let alignment = if m.get() { Alignment::TopEnd } else { Alignment::TopStart };
                        Box(
                            modifier = Modifier.size_wh(140.dp(), 40.dp()),
                            content_alignment = alignment,
                            content = move || {
                                Box(modifier = Modifier
                                    .animate_bounds(scope.clone(), bounds_transform = transform.clone())
                                    .size(40.dp())
                                    .test_tag("hata"));
                            },
                        );
                    });
                });
            },
        );
    }));
    rule.advance_frame();
    let canvas = rule.draw();
    // (a white highlight under the outline, 2.5 dp: 5 px wide, then the color, 2.5 px)
    let outlined = |canvas: &RecordingCanvas, color: Color| -> Vec<Rect> {
        canvas.strokes.iter().zip(&canvas.stroke_colors).filter(|(_, c)| **c == color).map(|(r, _)| *r).collect()
    };
    let rest = outlined(&canvas, inactive);
    assert_eq!(rest.len(), 1, "{:?} {:?}", canvas.strokes, canvas.stroke_colors);
    assert!(near(rest[0].width(), 40.0, 3.0));
    moved.set(true);
    rule.advance_time_by(48);
    let canvas = rule.draw();
    let mine = outlined(&canvas, own);
    // (where it is, and at its target 100 px right of where it started)
    assert_eq!(mine.len(), 2, "{:?} {:?}", canvas.strokes, canvas.stroke_colors);
    let now = bounds(&rule, "hata");
    assert!(now.left > 0.0 && now.left < 100.0, "{now:?}");
    assert!(mine.iter().any(|r| near(r.center().x, 120.0, 3.0)), "{mine:?}");
    assert!(outlined(&canvas, inactive).is_empty());
    // The dashed path and the diamond.
    assert!(canvas.paths.len() >= 2, "{:?}", canvas.paths);
    rule.wait_for_idle();
    // Turned off: nothing.
    enabled.set(false);
    rule.wait_for_idle();
    let canvas = rule.draw();
    assert!(outlined(&canvas, inactive).is_empty() && outlined(&canvas, own).is_empty());
}

tests!(
    animate_content_size_follows_the_content,
    animate_content_size_tells_where_it_started_and_aligns_its_content,
    animate_content_size_in_a_lookahead_scope_goes_to_the_lookahead_size,
    double_measured_children_settle,
    animate_bounds_glides_to_a_new_place_and_size,
    animate_bounds_takes_a_modifier_between_and_follows_a_scroller_only_when_asked,
    lookahead_animation_visual_debugging_outlines_animations,
);
