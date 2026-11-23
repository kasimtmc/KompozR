//! Scrolling: `ScrollState` with `vertical_scroll` and `horizontal_scroll`, `scrollable` and `scrollable_area`,
//! drags, flings along Android's spline, the mouse wheel and a trackpad, nested scrolling, Page Up and Down,
//! accessibility's scrolls, bringing into view (requesters, the focus, `BringIntoViewSpec`), clipping — the C++
//! Kompoz's scroll tests (survey B) and Compose's behaviour where the two differ. Numbers in px (density 1).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_animation::*;
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::input::*;
use kompoz_ui::layout::LayoutCoordinates;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::semantics::SemanticsActions;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// C++'s `rule.swipe`: a finger from `from` to `to`, one move a frame, let go, a frame.
fn swipe(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        let t = i as f32 / frames as f32;
        h.pointer_move(from + (to - from) * t, -1, 0);
    }
    h.pointer_up(to, -1, 0);
    rule.advance_frame();
}

/// C++'s `slowDrag`: the same, held 200 ms before letting go (no fling).
fn slow_drag(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        let t = i as f32 / frames as f32;
        h.pointer_move(from + (to - from) * t, -1, 0);
    }
    rule.advance_time_by(200);
    h.pointer_up(to, -1, 0);
    rule.advance_frame();
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).fetch_semantics_node().unclipped_bounds_in_root()
}

/// A column of `n` boxes `size` px tall, tagged `{prefix}{i}`.
#[kompozable]
fn Boxes(n: i32, size: f32, prefix: &'static str) {
    for i in 0..n {
        Box(modifier = Modifier.size(size.dp()).test_tag(format!("{prefix}{i}")));
    }
}

fn vertical_scroll_moves_content() {
    // C++ 1502: a notch of the wheel scrolls; the boxes move up by the value.
    let scroll = ScrollState::new(0);
    let s = scroll.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Column(
                modifier = Modifier.height(100.dp()).vertical_scroll(&s),
                content = move || {
                    Boxes(10, 50.0, "box");
                },
            );
        }),
        IntSize::new(200, 400),
    );
    assert_eq!(scroll.max_value(), 400);
    rule.host().scroll(at(10.0, 10.0), at(0.0, 1.0)); // one notch down
    rule.wait_for_idle();
    assert!(scroll.value() > 0);
    assert!(near(bounds(&rule, "box1").top, 50.0 - scroll.value() as f32, 0.01));
}

fn horizontal_scroll_modifier() {
    // C++ 2112: a swipe left scrolls a row.
    let scroll = ScrollState::new(0);
    let s = scroll.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Row(
            modifier = Modifier.fill_max_width().horizontal_scroll(&s),
            content = move || {
                for i in 0..20 {
                    Box(modifier = Modifier.size(50.dp()).test_tag(format!("b{i}")));
                }
            },
        );
    }));
    assert_eq!(scroll.max_value(), 600);
    swipe(&rule, at(300.0, 25.0), at(100.0, 25.0), 10);
    rule.wait_for_idle();
    assert!(scroll.value() > 150, "{}", scroll.value());
    assert!(near(bounds(&rule, "b0").left, -(scroll.value() as f32), 0.01));
}

fn scroll_state_animates_to_a_value() {
    // C++ 12864: animate_scroll_to animates, only as far as it goes; a newer one takes over; a finger stops it
    // and flings on from there.
    let scroll = ScrollState::new(0);
    let scope: Rc<RefCell<Option<CoroutineScope>>> = Rc::new(RefCell::new(None));
    let (s, sc) = (scroll.clone(), scope.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *sc.borrow_mut() = Some(remember_coroutine_scope());
        let s = s.clone();
        Column(
            modifier = Modifier.size_wh(200.dp(), 400.dp()).vertical_scroll(&s),
            content = || {
                Box(modifier = Modifier.fill_max_width().height(2000.dp()));
            },
        );
    }));
    let launch = |f: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>| {
        scope.borrow().as_ref().unwrap().launch(f);
    };
    let to = |v: i32| {
        let s = scroll.clone();
        launch(Box::pin(async move { s.animate_scroll_to(v).await }));
    };
    to(1000);
    for _ in 0..5 {
        rule.advance_frame();
    }
    assert!(scroll.value() > 0 && scroll.value() < 1000, "{}", scroll.value());
    rule.wait_for_idle();
    assert_eq!(scroll.value(), 1000);
    to(5000); // only as far as it goes
    rule.wait_for_idle();
    assert_eq!(scroll.value(), 1600);
    {
        let s = scroll.clone();
        launch(Box::pin(async move {
            let t: TweenSpec<f32> = named_args!(tween(200));
            let spec: std::sync::Arc<dyn AnimationSpec<f32>> = std::sync::Arc::new(t);
            named_args!(s.animate_scroll_by(-600.0, spec)).await;
        }));
    }
    rule.advance_time_by(100);
    assert!(scroll.value() < 1600 && scroll.value() > 1000, "{}", scroll.value());
    rule.wait_for_idle();
    assert_eq!(scroll.value(), 1000);
    to(0); // a newer one takes over
    rule.advance_frame();
    rule.advance_frame();
    to(800);
    rule.wait_for_idle();
    assert_eq!(scroll.value(), 800);
    to(0); // and a finger
    for _ in 0..4 {
        rule.advance_frame();
    }
    swipe(&rule, at(100.0, 300.0), at(100.0, 250.0), 4);
    let released = scroll.value(); // the animation stopped: the fling goes on from here, not back to 0
    rule.wait_for_idle();
    assert!(released > 100 && scroll.value() > released + 50, "{released} {}", scroll.value());
}

fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    rule.on_node_with_text(text).fetch_semantics_node().unclipped_bounds_in_root()
}

/// Whether the node tagged `tag` has the focus.
fn is_focused(rule: &KompozTestRule, tag: &str) -> bool {
    let node = rule.on_node_with_tag(tag).fetch_semantics_node();
    rule.host().focused_node().is_some_and(|f| Rc::ptr_eq(&f, node.layout_node()))
}

/// C++'s `state.scrollTo(value)`: scrolled from code, run until idle.
fn scroll_to(rule: &KompozTestRule, state: &ScrollState, value: i32) {
    let s = state.clone();
    CoroutineScope::new(rule.host().dispatcher()).launch(async move {
        s.scroll_to(value).await;
    });
    rule.wait_for_idle();
}

fn the_mouse_wheel_scrolls_smoothly_as_compose() {
    // C++ 1952: Compose's MouseWheelScrollingLogic: 64 dp a notch, the first 6 at once, the rest at 1 dp a
    // millisecond (at most 100 ms); notches meanwhile add up; a precise wheel scrolls at once; a press stops it.
    let scroll = ScrollState::new(0);
    let s = scroll.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll(&s),
            content = || {
                Box(modifier = Modifier.fill_max_width().height(5000.dp()));
            },
        );
    }));
    let h = rule.host();
    let notch = |n: f32| {
        h.scroll(at(100.0, 100.0), at(0.0, n));
    };
    notch(1.0);
    rule.host().dispatcher().run_ready();
    assert_eq!(scroll.value(), 6);
    rule.advance_frame(); // (the animation's first frame is its start, as Compose's animateTo has it)
    assert_eq!(scroll.value(), 6);
    rule.advance_frame();
    let early = scroll.value();
    assert!(early > 6 && early < 64, "{early}");
    rule.advance_time_by(100);
    assert_eq!(scroll.value(), 64);
    // Two notches: the second on top of what is left.
    rule.wait_for_idle();
    notch(1.0);
    rule.advance_frame();
    notch(1.0);
    rule.wait_for_idle();
    assert_eq!(scroll.value(), 64 * 3);
    // Back the other way: from where it is.
    notch(-1.0);
    rule.wait_for_idle();
    assert_eq!(scroll.value(), 128);
    // A precise wheel (half a notch): at once.
    notch(0.5);
    rule.host().dispatcher().run_ready();
    assert_eq!(scroll.value(), 160);
    rule.wait_for_idle();
    // A press stops it where it is.
    notch(1.0);
    rule.advance_frame();
    let stopped = scroll.value();
    h.pointer_down(at(100.0, 100.0), -1, 0, PointerType::Touch);
    rule.advance_time_by(100);
    h.pointer_up(at(100.0, 100.0), -1, 0);
    rule.wait_for_idle();
    assert!(scroll.value() == stopped && stopped < 160 + 64, "{stopped} {}", scroll.value());
}

fn scroll_container_removed_mid_drag_does_not_crash() {
    // C++ 12243.
    let shown = mutable_state_of(true);
    let scroll = ScrollState::new(0);
    let (sh, s) = (shown.clone(), scroll.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        if sh.get() {
            let s = s.clone();
            Column(
                modifier = Modifier.fill_max_size().vertical_scroll(&s),
                content = || {
                    Box(modifier = Modifier.height(3000.dp()));
                },
            );
        }
    }));
    let h = rule.host();
    h.pointer_down(at(100.0, 400.0), -1, 0, PointerType::Touch);
    h.pointer_move(at(100.0, 350.0), -1, 0); // past the slop: scrolling
    h.pointer_move(at(100.0, 300.0), -1, 0);
    shown.set(false);
    rule.wait_for_idle();
    h.pointer_move(at(100.0, 250.0), -1, 0); // the list is gone
    h.pointer_up(at(100.0, 250.0), -1, 0);
    rule.wait_for_idle();
    assert!(!shown.peek());
}

fn drag_gestures_move_an_offset_and_win_over_scrolling() {
    // C++ 8842: a drag detector in a scrolling column takes its drags; outside it the column scrolls.
    let dragged = mutable_state_of(Offset::Zero);
    let scroll = ScrollState::new(0);
    let events = Rc::new(RefCell::new(Vec::<&str>::new()));
    let (d, s, e) = (dragged.clone(), scroll.clone(), events.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (d, s, e) = (d.clone(), s.clone(), e.clone());
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll(&s),
            content = move || {
                let (d, d2, e) = (d.clone(), d.clone(), e.clone());
                Box(modifier = Modifier
                    .offset_px(move |_| {
                        let o = d2.get();
                        IntOffset::new(o.x.round() as i32, o.y.round() as i32)
                    })
                    .size(100.dp())
                    .test_tag("box")
                    .pointer_input(0, move |scope: PointerInputScope| {
                        let (d, e1, e2) = (d.clone(), e.clone(), e.clone());
                        async move {
                            scope
                                .detect_drag_gestures(move |change, amount| {
                                    change.consume();
                                    d.set(d.peek() + amount);
                                })
                                .on_drag_start(move |_| e1.borrow_mut().push("start"))
                                .on_drag_end(move || e2.borrow_mut().push("end"))
                                .await
                        }
                    }));
                Box(modifier = Modifier.fill_max_width().height(2000.dp()));
            },
        );
    }));
    swipe(&rule, at(50.0, 50.0), at(150.0, 110.0), 10);
    rule.wait_for_idle();
    // The whole movement less the touch slop along its way.
    let length = (100.0f32 * 100.0 + 60.0 * 60.0).sqrt();
    let k = (length - 8.0) / length;
    assert!(near(dragged.peek().x, 100.0 * k, 0.01) && near(dragged.peek().y, 60.0 * k, 0.01), "{:?}", dragged.peek());
    assert_eq!(scroll.value(), 0); // the drag consumed the gesture
    assert_eq!(*events.borrow(), ["start", "end"]);
    swipe(&rule, at(300.0, 500.0), at(300.0, 300.0), 10); // outside the box the list scrolls
    rule.wait_for_idle();
    assert!(scroll.value() > 100, "{}", scroll.value());
}

fn scrollable_reports_drags_in_the_finger_direction() {
    // C++ 10377: a scrollable's deltas are the finger's; what it doesn't take goes to the page; reversed, the other
    // way.
    let offset = Rc::new(Cell::new(0.0f32));
    let reversed = Rc::new(Cell::new(0.0f32));
    let page = ScrollState::new(0);
    let (o, r, p) = (offset.clone(), reversed.clone(), page.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, r) = (o.clone(), r.clone());
        // Takes deltas while the offset stays within [-200, 0]; the page gets the rest.
        let state = remember_scrollable_state(move |delta| {
            let next = (o.get() + delta).clamp(-200.0, 0.0);
            let used = next - o.get();
            o.set(next);
            used
        });
        let other = remember_scrollable_state(move |delta| {
            r.set(r.get() + delta);
            delta
        });
        let p = p.clone();
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll(&p),
            content = move || {
                Box(modifier = Modifier
                    .fill_max_width()
                    .height(300.dp())
                    .scrollable(state.clone(), Orientation::Vertical)
                    .test_tag("area"));
                Box(modifier = named_args!(Modifier.fill_max_width().height(100.dp()).scrollable(
                    other.clone(),
                    Orientation::Horizontal,
                    reverse_direction = true
                ))
                .test_tag("row"));
                Box(modifier = Modifier.height(2000.dp()));
            },
        );
    }));
    // The finger up 100: -100 less the touch slop (Compose's drag starts past it: C++ counted it), and a frame of
    // the fling.
    swipe(&rule, at(200.0, 250.0), at(200.0, 150.0), 20);
    rule.advance_frame();
    let o = offset.get();
    assert!((-100.0..=-91.5).contains(&o) && page.value() == 0, "{o} {}", page.value());
    swipe(&rule, at(200.0, 250.0), at(200.0, 50.0), 20); // only 100 more fit: the page scrolls with the rest
    rule.wait_for_idle();
    assert!(near(offset.get(), -200.0, 0.01) && page.value() > 50, "{} {}", offset.get(), page.value());
    scroll_to(&rule, &page, 0);
    let y = bounds(&rule, "row").center().y;
    swipe(&rule, at(300.0, y), at(200.0, y), 20); // reversed: a leftward finger reads as positive
    rule.advance_frame();
    let r = reversed.get();
    assert!((91.5..=100.0).contains(&r), "{r}");
}

fn nested_scrollables_hand_over_at_the_edge() {
    // C++ 14519: the inner one scrolls first, the outer one takes the rest; back, the inner one first; a fling on
    // the inner one goes on in the outer one at its end.
    let (outer, inner) = (ScrollState::new(0), ScrollState::new(0));
    let (o, i) = (outer.clone(), inner.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, i) = (o.clone(), i.clone());
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll(&o),
            content = move || {
                Box(modifier = Modifier.height(100.dp()));
                let i = i.clone();
                Column(
                    modifier = Modifier.fill_max_width().height(300.dp()).vertical_scroll(&i),
                    content = || {
                        Box(modifier = Modifier.height(600.dp()));
                    },
                );
                Box(modifier = Modifier.height(1000.dp()));
            },
        );
    }));
    assert!(inner.max_value() == 300 && outer.max_value() == 600);
    // Up 500: the touch slop (8) starts the drag, the inner one scrolls to its end, the outer one takes the rest.
    slow_drag(&rule, at(200.0, 350.0), at(200.0, -150.0), 20);
    assert!(inner.value() == 300 && outer.value() == 192, "{} {}", inner.value(), outer.value());
    // Down: the inner one (under the finger) goes back first.
    slow_drag(&rule, at(200.0, 150.0), at(200.0, 400.0), 20);
    assert!(inner.value() == 58 && outer.value() == 192, "{} {}", inner.value(), outer.value());
    slow_drag(&rule, at(200.0, 100.0), at(200.0, 400.0), 20);
    assert!(inner.value() == 0 && outer.value() == 0, "{} {}", inner.value(), outer.value());
    // A fling on the inner one carries on in the outer one once the inner one is at its end.
    swipe(&rule, at(200.0, 350.0), at(200.0, 150.0), 4);
    rule.wait_for_idle();
    assert!(inner.value() == 300 && outer.value() > 50, "{} {}", inner.value(), outer.value());
}

fn a_scrollable_takes_its_drags_from_an_outer_drag_handler() {
    // C++ 14571: a list inside a drag handler scrolls with drags along it; across, the handler gets them.
    let list = ScrollState::new(0);
    let outer_drags = Rc::new(Cell::new(0));
    let (l, od) = (list.clone(), outer_drags.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, od) = (l.clone(), od.clone());
        Box(
            modifier = Modifier.fill_max_size().pointer_input(0, move |s: PointerInputScope| {
                let od = od.clone();
                async move {
                    s.detect_drag_gestures(move |c, _| {
                        c.consume();
                        od.set(od.get() + 1);
                    })
                    .await
                }
            }),
            content = move || {
                let l = l.clone();
                Column(
                    modifier = Modifier.fill_max_width().height(400.dp()).vertical_scroll(&l),
                    content = || {
                        Box(modifier = Modifier.height(2000.dp()));
                    },
                );
            },
        );
    }));
    slow_drag(&rule, at(200.0, 300.0), at(200.0, 100.0), 20);
    assert!(list.value() > 150 && outer_drags.get() == 0, "{} {}", list.value(), outer_drags.get());
    let before = list.value();
    slow_drag(&rule, at(100.0, 200.0), at(300.0, 200.0), 20); // sideways: not along the list
    assert!(list.value() == before && outer_drags.get() > 0);
}

fn rtl_horizontal_scrolling_starts_at_the_right_and_follows_the_finger() {
    // C++ 17320 (its LazyRow part is the lazy lists'): right to left, a horizontal scroll starts at the right, a
    // finger to the right goes on, up and down don't mirror; keyboard focus brings the next one into view.
    let (scroll, down) = (ScrollState::new(0), ScrollState::new(0));
    let (s, d) = (scroll.clone(), down.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (s, d) = (s.clone(), d.clone());
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], move || {
                let (s, d) = (s.clone(), d.clone());
                Column(
                    content = move || {
                        let s = s.clone();
                        Row(
                            modifier =
                                Modifier.width(200.dp()).height(40.dp()).horizontal_scroll(&s).test_tag("kaydır"),
                            content = || {
                                for i in 0..10 {
                                    Box(modifier =
                                        Modifier.size_wh(50.dp(), 40.dp()).clickable(|| {}).test_tag(format!("k{i}")));
                                }
                            },
                        );
                        let d = d.clone();
                        Column(
                            modifier = Modifier.size(100.dp()).vertical_scroll(&d).test_tag("dikey"),
                            content = || {
                                for _ in 0..5 {
                                    Box(modifier = Modifier.size_wh(100.dp(), 50.dp()));
                                }
                            },
                        );
                    },
                );
            });
        }),
        IntSize::new(200, 400),
    );
    let area = bounds(&rule, "kaydır");
    assert!(
        near(bounds(&rule, "k0").right, area.right, 0.01) && near(bounds(&rule, "k1").right, area.right - 50.0, 0.01)
    );
    // The finger to the right goes on (the content follows it), to the left back.
    swipe(&rule, at(area.left + 30.0, area.top + 20.0), at(area.left + 130.0, area.top + 20.0), 10);
    rule.wait_for_idle();
    assert!(scroll.value() > 60 && bounds(&rule, "k0").left > area.right - 50.0, "{}", scroll.value());
    let column = bounds(&rule, "dikey"); // up and down don't mirror
    swipe(&rule, at(column.left + 50.0, column.top + 90.0), at(column.left + 50.0, column.top + 10.0), 10);
    rule.wait_for_idle();
    assert!(down.value() > 30, "{}", down.value());
    let went = scroll.value();
    swipe(&rule, at(area.left + 130.0, area.top + 20.0), at(area.left + 60.0, area.top + 20.0), 10);
    rule.wait_for_idle();
    assert!(scroll.value() < went);
    // Brought into view by keyboard focus: the sixth (off at the left) scrolls in.
    scroll_to(&rule, &scroll, 0);
    assert!(near(bounds(&rule, "k0").right, area.right, 0.01));
    for _ in 0..6 {
        rule.host().key_down(Key::Tab, KeyModifiers::None);
    }
    rule.wait_for_idle();
    let sixth = bounds(&rule, "k5");
    assert!(is_focused(&rule, "k5"));
    assert!(sixth.left >= area.left - 0.5 && sixth.right <= area.right + 0.5, "{sixth:?}");
}

fn bring_into_view_requesters_scroll_their_node_into_view() {
    // C++ 10940: only the box around the node scrolls when that is enough; animated, just into view; both ways
    // through a row in the page; a part of a long node.
    let (scroll, across, boxed_scroll) = (ScrollState::new(0), ScrollState::new(0), ScrollState::new(0));
    let (far, inner, part, boxed) = (
        BringIntoViewRequester::new(),
        BringIntoViewRequester::new(),
        BringIntoViewRequester::new(),
        BringIntoViewRequester::new(),
    );
    let (s, a, bs, f, i, p, b) =
        (scroll.clone(), across.clone(), boxed_scroll.clone(), far.clone(), inner.clone(), part.clone(), boxed.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, a, bs, f, i, p, b) = (s.clone(), a.clone(), bs.clone(), f.clone(), i.clone(), p.clone(), b.clone());
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll(&s),
            content = move || {
                // A scrollable box, in view, the item far down in it (below the page's view until the box brings it up).
                let b = b.clone();
                Column(
                    modifier = Modifier.height(200.dp()).vertical_scroll(&bs),
                    content = move || {
                        Box(modifier = Modifier.height(1500.dp()));
                        Box(modifier = Modifier.size(50.dp()).bring_into_view_requester(&b).test_tag("kutuda"));
                    },
                );
                Box(modifier = Modifier.height(1500.dp()));
                Box(modifier = Modifier.size(100.dp()).bring_into_view_requester(&f).test_tag("uzak"));
                Box(modifier = Modifier.height(1500.dp()));
                // In a scrollable across, in the one down.
                let i = i.clone();
                Row(
                    modifier = Modifier.fill_max_width().horizontal_scroll(&a),
                    content = move || {
                        Box(modifier = Modifier.size_wh(1000.dp(), 50.dp()));
                        Box(modifier = Modifier.size_wh(80.dp(), 50.dp()).bring_into_view_requester(&i).test_tag("iç"));
                    },
                );
                Box(modifier = Modifier.size_wh(100.dp(), 600.dp()).bring_into_view_requester(&p).test_tag("uzun"));
                Box(modifier = Modifier.height(1500.dp()));
            },
        );
    }));
    let scope = CoroutineScope::new(rule.host().dispatcher());
    {
        let b = boxed.clone();
        scope.launch(async move { b.bring_into_view_rect(None).await });
    }
    rule.advance_time_by(2000);
    rule.wait_for_idle();
    assert!(scroll.value() == 0 && near(bounds(&rule, "kutuda").bottom, 200.0, 0.5), "{}", scroll.value());
    let done = Rc::new(Cell::new(false));
    {
        let (f, d) = (far.clone(), done.clone());
        scope.launch(async move {
            f.bring_into_view_rect(None).await;
            d.set(true);
        });
    }
    rule.advance_time_by(48);
    let mid = scroll.value();
    rule.advance_time_by(2000);
    rule.wait_for_idle();
    let b = bounds(&rule, "uzak");
    // Animated, just into view at the bottom.
    assert!(done.get() && mid > 0 && mid < scroll.value() - 50 && near(b.bottom, 800.0, 0.5), "{mid} {b:?}");
    {
        let i = inner.clone();
        scope.launch(async move { i.bring_into_view_rect(None).await });
    }
    rule.advance_time_by(2000);
    rule.wait_for_idle();
    let i = bounds(&rule, "iç");
    assert!(near(i.right, 400.0, 0.5) && near(i.bottom, 800.0, 0.5) && across.value() > 600, "{i:?}"); // both ways
    // A part of a long node: its 100 px from 250 down, just into view.
    {
        let p = part.clone();
        scope.launch(async move { p.bring_into_view_rect(Some(Rect::new(0.0, 250.0, 100.0, 350.0))).await });
    }
    rule.advance_time_by(2000);
    rule.wait_for_idle();
    let u = bounds(&rule, "uzun");
    assert!(near(u.top + 350.0, 800.0, 0.5), "{u:?}");
}

fn a_focus_request_brings_the_node_into_view() {
    // C++ 11025 (its Button a focusable box: Material is stage 12's; its TextField a BasicTextField):
    // far below and no keyboard to make room for, scrolled to anyway; back up.
    let scroll = ScrollState::new(0);
    let (below, above) = (FocusRequester::new(), FocusRequester::new());
    let (s, b, a) = (scroll.clone(), below.clone(), above.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, b, a) = (s.clone(), b.clone(), a.clone());
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll(&s),
            content = move || {
                // (with a width: a zero-width layout has no bounds to bring in, Compose's localBoundingBoxOf giving Rect.Zero)
                Box(modifier =
                    Modifier.height(60.dp()).fill_max_width().focus_requester(&a).focusable().test_tag("üst"));
                Box(modifier = Modifier.height(1500.dp()));
                BasicTextField(
                    remember_text_field_state(String::new()),
                    modifier = Modifier.height(56.dp()).fill_max_width().focus_requester(&b).test_tag("alan"),
                );
                Box(modifier = Modifier.height(1500.dp()));
            },
        );
    }));
    below.request_focus();
    rule.wait_for_idle();
    let field = bounds(&rule, "alan");
    assert!(field.top >= 0.0 && near(field.bottom, 800.0, 1.0), "{field:?}"); // just in view, at the bottom
    above.request_focus(); // and back up
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "üst").top, 0.0, 0.01) && scroll.value() == 0, "{}", scroll.value());
}

fn scroll_states_tell_their_value_and_whether_they_scroll() {
    // C++ 12899's ScrollState part (its lazy list and grid parts are the lazy layouts'): read in composition,
    // recomposed as they change; scrolling while dragged and flung, not once done; a wheel stops a fling; held
    // still before letting go, no fling; while animated.
    let scroll = ScrollState::new(0);
    let s = scroll.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Column(
                content = move || {
                    Text(format!(
                        "değer {}/{} {}{}{}",
                        s.value(),
                        s.max_value(),
                        if s.can_scroll_backward() { "<" } else { "" },
                        if s.can_scroll_forward() { ">" } else { "" },
                        if s.is_scroll_in_progress() { " kayıyor" } else { "" }
                    ));
                    Column(
                        modifier = Modifier.size(200.dp()).vertical_scroll(&s).test_tag("sütun"),
                        content = || {
                            Box(modifier = Modifier.fill_max_width().height(1000.dp()));
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 1200),
    );
    let shows = |text: &str| rule.on_node_with_text(text).exists();
    assert!(shows("değer 0/800 >"));
    scroll_to(&rule, &scroll, 300);
    assert!(shows("değer 300/800 <>"));
    // Dragged: scrolling; let go with speed: still, as it flings; then not.
    let top = bounds(&rule, "sütun").top;
    let h = rule.host();
    h.pointer_down(at(100.0, top + 150.0), -1, 0, PointerType::Touch);
    for i in 1..=5 {
        rule.advance_frame();
        h.pointer_move(at(100.0, top + 150.0 - 20.0 * i as f32), -1, 0);
    }
    rule.advance_frame();
    assert!(
        scroll.is_scroll_in_progress() && named_args!(rule.on_node_with_text("kayıyor", substring = true)).exists(),
        "{}",
        scroll.value()
    );
    h.pointer_up(at(100.0, top + 50.0), -1, 0);
    rule.advance_frame();
    assert!(scroll.is_scroll_in_progress());
    rule.wait_for_idle();
    assert!(!scroll.is_scroll_in_progress() && scroll.value() > 400, "{}", scroll.value());
    // A fling a wheel stops: done once the wheel's scroll is (its 50 ms timeout: Compose's), the fling gone.
    swipe(&rule, at(100.0, top + 150.0), at(100.0, top + 50.0), 5);
    rule.advance_frame();
    assert!(scroll.is_scroll_in_progress());
    let flinging = scroll.value();
    h.scroll(at(100.0, top + 100.0), at(0.0, 0.05)); // a touchpad's tenth of a notch: at once
    rule.host().dispatcher().run_ready();
    let wheeled = scroll.value();
    assert!(wheeled - flinging <= 4, "{flinging} {wheeled}");
    rule.wait_for_idle();
    assert!(!scroll.is_scroll_in_progress() && scroll.value() == wheeled, "{wheeled} {}", scroll.value());
    // Held still before letting go: no fling, done at once.
    h.pointer_down(at(100.0, top + 150.0), -1, 0, PointerType::Touch);
    for i in 1..=5 {
        rule.advance_frame();
        h.pointer_move(at(100.0, top + 150.0 + 10.0 * i as f32), -1, 0);
    }
    rule.advance_time_by(200);
    h.pointer_up(at(100.0, top + 200.0), -1, 0);
    rule.host().dispatcher().run_ready();
    assert!(!scroll.is_scroll_in_progress());
    // Animated: while it runs.
    let scope = CoroutineScope::new(rule.host().dispatcher());
    let s = scroll.clone();
    scope.launch(async move { s.animate_scroll_to(0).await });
    rule.advance_frame();
    rule.advance_frame();
    assert!(scroll.is_scroll_in_progress());
    rule.wait_for_idle();
    assert!(!scroll.is_scroll_in_progress() && shows("değer 0/800 >"));
}

fn scrolling_turned_off_for_the_user_and_scrolling_from_the_end() {
    // C++ 12992's scroll container parts (its lazy lists the lazy layouts', the inner one a scrolling column here):
    // not by the user, a drag or the wheel goes to the page around it, what a list in it leaves too; accessibility
    // can't scroll it; code and bringing into view still do; from the end, its bottom shows first.
    let (page, fixed_scroll, from_end, inner) =
        (ScrollState::new(0), ScrollState::new(0), ScrollState::new(0), ScrollState::new(0));
    let deep = FocusRequester::new();
    let (p, f, fe, i, d) = (page.clone(), fixed_scroll.clone(), from_end.clone(), inner.clone(), deep.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (p, f, fe, i, d) = (p.clone(), f.clone(), fe.clone(), i.clone(), d.clone());
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll(&p),
            content = move || {
                let (i, d) = (i.clone(), d.clone());
                Column(
                    modifier =
                        named_args!(Modifier.size(200.dp()).vertical_scroll(&f, enabled = false)).test_tag("sabit"),
                    content = move || {
                        // What it leaves goes past the fixed one.
                        Column(
                            modifier = Modifier.size_wh(200.dp(), 100.dp()).vertical_scroll(&i),
                            content = || {
                                for k in 0..3 {
                                    Text(format!("iç {k}"), modifier = Modifier.height(40.dp()));
                                }
                            },
                        );
                        Box(modifier = Modifier.fill_max_width().height(400.dp()));
                        Box(modifier = Modifier.size(40.dp()).focus_requester(&d).focusable().test_tag("derin"));
                    },
                );
                // From the end: it shows its bottom first.
                Column(
                    modifier = named_args!(Modifier.size(200.dp()).vertical_scroll(&fe, reverse_scrolling = true))
                        .test_tag("sondan"),
                    content = || {
                        for k in 0..10 {
                            Text(format!("satır {k}"), modifier = Modifier.height(50.dp()));
                        }
                    },
                );
                Box(modifier = Modifier.height(2000.dp()));
            },
        );
    }));
    rule.host().scroll(at(100.0, 100.0), at(0.0, 3.0)); // nor the wheel, over the fixed one: the page takes it
    rule.wait_for_idle();
    assert!(fixed_scroll.value() == 0 && page.value() > 0, "{}", page.value());
    scroll_to(&rule, &page, 0);
    swipe(&rule, at(100.0, 90.0), at(100.0, 10.0), 20); // the list inside scrolls its 20, the rest to the page
    rule.wait_for_idle();
    assert!(fixed_scroll.value() == 0 && page.value() > 30 && !inner.can_scroll_forward(), "{}", page.value());
    scroll_to(&rule, &page, 0);
    // Accessibility can't scroll the fixed one: the page, the column in it and the one from the end only.
    let scrolls = rule.all_nodes(false).iter().filter(|n| n.config().contains(SemanticsActions::ScrollBy)).count();
    assert_eq!(scrolls, 3);
    // Bringing something into view still scrolls it.
    deep.request_focus();
    rule.wait_for_idle();
    assert!(fixed_scroll.value() > 300, "{}", fixed_scroll.value());
    // From the end: value 0 at the bottom, rows 6–9 in view; a finger down scrolls back up.
    scroll_to(&rule, &page, 0);
    let view = bounds(&rule, "sondan");
    assert!(from_end.max_value() == 300 && from_end.value() == 0);
    assert!(near(text_bounds(&rule, "satır 9").bottom, view.bottom, 0.01));
    swipe(&rule, at(100.0, view.top + 50.0), at(100.0, view.top + 150.0), 20);
    rule.wait_for_idle();
    assert!(from_end.value() > 50, "{}", from_end.value());
    assert!(near(text_bounds(&rule, "satır 9").bottom, view.bottom + from_end.value() as f32, 0.01));
}

fn focus_moved_by_keys_scrolls_into_view_and_so_does_the_keyboard() {
    // C++ 20849's scrolling (its Buttons focusable boxes: Material is stage 12's; its TextField a BasicTextField): Tab brings the focused one into view, Shift+Tab back up; the focused field a keyboard comes up
    // over (the scroll's viewport shrinking by its padding) has its cursor kept above it; a window that gets shorter too; not
    // once scrolled away from it.
    let scroll = ScrollState::new(0);
    let field = FocusRequester::new();
    let (s, f) = (scroll.clone(), field.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, f) = (s.clone(), f.clone());
        Column(
            modifier = Modifier.fill_max_size().ime_padding().vertical_scroll(&s),
            content = move || {
                for i in 0..20 {
                    Box(modifier = Modifier.fill_max_width().height(60.dp()).focusable().test_tag(format!("b{i}")));
                }
                BasicTextField(
                    remember_text_field_state(String::new()),
                    modifier = Modifier.fill_max_width().height(60.dp()).focus_requester(&f).test_tag("field"),
                    text_style = kompoz_text::TextStyle { font_size: 16.sp(), ..kompoz_text::TextStyle::Default },
                );
            },
        );
    }));
    for _ in 0..15 {
        rule.host().key_down(Key::Tab, KeyModifiers::None);
    }
    rule.wait_for_idle();
    let b14 = bounds(&rule, "b14");
    assert!(is_focused(&rule, "b14") && b14.top >= 0.0 && b14.bottom <= 800.0 && scroll.value() > 0, "{b14:?}");
    // Back up: as far as needed.
    for _ in 0..14 {
        rule.host().key_down(Key::Tab, KeyModifiers::Shift);
    }
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "b0").top, 0.0, 0.01));
    // The field near the bottom focused, then the keyboard comes up over it: it scrolls above it.
    scroll_to(&rule, &scroll, scroll.max_value());
    field.request_focus();
    rule.wait_for_idle();
    assert!(bounds(&rule, "field").bottom > 700.0);
    let insets = kompoz_ui::platform::SystemInsets {
        ime: kompoz_ui::platform::Insets { left: 0, top: 0, right: 0, bottom: 300 },
        ..Default::default()
    };
    rule.host().set_system_insets(insets);
    rule.wait_for_idle();
    // Compose keeps the field's focus rect in view — its cursor, its first line (20 px) — not all of it (the
    // C++ kept its text area).
    let editing = bounds(&rule, "field");
    assert!(near(editing.top + 20.0, 500.0, 0.5), "{editing:?}");
    // A window that isn't edge to edge gets shorter instead.
    rule.host().set_system_insets(kompoz_ui::platform::SystemInsets::default());
    rule.wait_for_idle();
    scroll_to(&rule, &scroll, scroll.max_value());
    rule.host().set_viewport_size(IntSize::new(400, 500));
    rule.wait_for_idle();
    let editing = bounds(&rule, "field");
    assert!(editing.top + 20.0 <= 500.5 && editing.top + 20.0 > 450.0, "{editing:?}");
    scroll_to(&rule, &scroll, 0); // scrolled away from it, then taller again: it stays where it is
    rule.host().set_viewport_size(IntSize::new(400, 800));
    rule.wait_for_idle();
    assert_eq!(scroll.value(), 0);
}

/// Logs what `source` emits while composed (C++'s LogInteractions).
#[kompozable]
fn LogInteractions(source: MutableInteractionSource, log: Rc<RefCell<Vec<&'static str>>>) {
    let flow = source.interactions();
    kompoz_coroutines::LaunchedEffect(source.clone(), async move {
        flow.collect(move |i| log.borrow_mut().push(interaction_name(&*i))).await;
    });
}

fn interaction_name(i: &dyn Interaction) -> &'static str {
    if i.is::<PressInteraction::Press>() {
        "Press"
    } else if i.is::<PressInteraction::Release>() {
        "Release"
    } else if i.is::<PressInteraction::Cancel>() {
        "Cancel"
    } else if i.is::<DragInteraction::Start>() {
        "DragStart"
    } else if i.is::<DragInteraction::Stop>() {
        "DragStop"
    } else if i.is::<DragInteraction::Cancel>() {
        "DragCancel"
    } else {
        "?"
    }
}

fn a_press_in_a_scrollable_shows_after_the_tap_timeout() {
    // C++ 23313: held in a scrollable, a press shows once the tap timeout (100 ms) is over; a quick tap at once;
    // outside a scrollable, or with a mouse, at once.
    let (in_list, outside) = (MutableInteractionSource::new(), MutableInteractionSource::new());
    let (list_log, outside_log) = (Rc::new(RefCell::new(Vec::new())), Rc::new(RefCell::new(Vec::new())));
    let clicks = Rc::new(Cell::new(0));
    let (il, ou, ll, ol, c) = (in_list.clone(), outside.clone(), list_log.clone(), outside_log.clone(), clicks.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        LogInteractions(il.clone(), ll.clone());
        LogInteractions(ou.clone(), ol.clone());
        let (il, ou, c) = (il.clone(), ou.clone(), c.clone());
        Column(
            content = move || {
                let (il, c) = (il.clone(), c.clone());
                let scroll = remember_scroll_state();
                Column(
                    modifier = Modifier.height(300.dp()).vertical_scroll(&scroll),
                    content = move || {
                        let c = c.clone();
                        Box(modifier =
                            named_args!(Modifier.size_wh(200.dp(), 60.dp()).test_tag("içeride").clickable_indication(
                                Some(il.clone()),
                                None,
                                on_click = move || c.set(c.get() + 1)
                            )));
                        Box(modifier = Modifier.height(1000.dp()));
                    },
                );
                Box(modifier =
                    named_args!(Modifier.size_wh(200.dp(), 60.dp()).test_tag("dışarıda").clickable_indication(
                        Some(ou.clone()),
                        None,
                        on_click = || {}
                    )));
            },
        );
    }));
    let h = rule.host();
    let inside = bounds(&rule, "içeride").center();
    h.pointer_down(inside, -1, 0, PointerType::Touch);
    rule.advance_time_by(50);
    assert!(list_log.borrow().is_empty());
    rule.advance_time_by(80);
    assert_eq!(*list_log.borrow(), ["Press"]);
    h.pointer_up(inside, -1, 0);
    rule.wait_for_idle();
    assert!(*list_log.borrow() == ["Press", "Release"] && clicks.get() == 1, "{:?}", list_log.borrow());
    // A quick tap: pressed and let go at once, and clicked.
    list_log.borrow_mut().clear();
    h.pointer_down(inside, -1, 0, PointerType::Touch);
    h.pointer_up(inside, -1, 0);
    rule.wait_for_idle();
    assert!(*list_log.borrow() == ["Press", "Release"] && clicks.get() == 2, "{:?}", list_log.borrow());
    // Waited for: the press's own deadline wakes it (the clock taken past the tap timeout: a delay, which waiting
    // for idle doesn't wait for, as Compose's doesn't).
    list_log.borrow_mut().clear();
    h.pointer_down(inside, -1, 0, PointerType::Touch);
    rule.main_clock().advance_time_by(130);
    assert_eq!(*list_log.borrow(), ["Press"]);
    h.pointer_up(inside, -1, 0);
    rule.wait_for_idle();
    // Outside a scrollable, or with a mouse: at once.
    let out = bounds(&rule, "dışarıda").center();
    h.pointer_down(out, -1, 0, PointerType::Touch);
    rule.advance_frame();
    assert_eq!(*outside_log.borrow(), ["Press"]);
    h.pointer_up(out, -1, 0);
    rule.wait_for_idle();
    list_log.borrow_mut().clear();
    h.pointer_down(inside, -1, 0, PointerType::Mouse);
    rule.advance_frame();
    assert_eq!(*list_log.borrow(), ["Press"]);
    h.pointer_up(inside, -1, 0);
    rule.wait_for_idle();
}

fn scrollable_states_hear_the_users_drags() {
    // C++ 23367's scroll container and scrollable parts (its lazy list and pager parts are the lazy layouts' and
    // the pager's; its Button a clickable box): a vertical scroll's own source hears the drag (not the press of an
    // item dragged before the tap timeout); a scrollable's given source.
    let item = MutableInteractionSource::new();
    let own = MutableInteractionSource::new();
    let list = ScrollState::new(0);
    let (list_log, item_log, own_log) =
        (Rc::new(RefCell::new(Vec::new())), Rc::new(RefCell::new(Vec::new())), Rc::new(RefCell::new(Vec::new())));
    let (it, ow, l, ll, il, ol) =
        (item.clone(), own.clone(), list.clone(), list_log.clone(), item_log.clone(), own_log.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let source = l.interaction_source();
        // (its own source, as a MutableInteractionSource can't be had of it: its interactions collected)
        let flow = source.interactions();
        let ll = ll.clone();
        kompoz_coroutines::LaunchedEffect((), async move {
            flow.collect(move |i| ll.borrow_mut().push(interaction_name(&*i))).await;
        });
        LogInteractions(it.clone(), il.clone());
        LogInteractions(ow.clone(), ol.clone());
        let dragged = source.collect_is_dragged_as_state().get();
        let (it, ow, l) = (it.clone(), ow.clone(), l.clone());
        Column(
            content = move || {
                Text(format!("liste {dragged}"));
                let it = it.clone();
                Column(
                    modifier = Modifier.fill_max_width().height(300.dp()).vertical_scroll(&l).test_tag("liste"),
                    content = move || {
                        Box(modifier = named_args!(
                            Modifier.fill_max_width().height(48.dp()).test_tag("öğe").clickable_indication(
                                Some(it.clone()),
                                None,
                                on_click = || {}
                            )
                        ));
                        for i in 0..50 {
                            Text(format!("satır {i}"), modifier = Modifier.height(40.dp()));
                        }
                    },
                );
                let state = remember_scrollable_state(|d| d);
                Box(modifier = named_args!(Modifier.fill_max_width().height(100.dp()).scrollable(
                    state,
                    Orientation::Vertical,
                    interaction_source = Some(ow.clone())
                ))
                .test_tag("kendi"));
            },
        );
    }));
    let h = rule.host();
    let start = bounds(&rule, "öğe").center();
    h.pointer_down(start, -1, 0, PointerType::Touch);
    for i in 1..=6 {
        h.pointer_move(at(start.x, start.y - 10.0 * i as f32), -1, 0);
    }
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("liste true").exists());
    // Dragged before the tap timeout: the press under it never shows.
    assert!(
        *list_log.borrow() == ["DragStart"] && item_log.borrow().is_empty(),
        "{:?} {:?}",
        list_log.borrow(),
        item_log.borrow()
    );
    h.pointer_up(at(start.x, start.y - 60.0), -1, 0);
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("liste false").exists() && *list_log.borrow() == ["DragStart", "DragStop"]);
    // A scrollable modifier's own source.
    let at_ = bounds(&rule, "kendi").center();
    slow_drag(&rule, at_, at(at_.x, at_.y - 40.0), 10);
    rule.wait_for_idle();
    assert_eq!(*own_log.borrow(), ["DragStart", "DragStop"]);
}

fn flings_go_along_androids_spline() {
    // C++ 24008: as far and as long as Android's spline says for the speed the finger was measured at.
    let scroll = ScrollState::new(0);
    let s = scroll.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Column(
            modifier = Modifier.size_wh(200.dp(), 400.dp()).vertical_scroll(&s),
            content = || {
                Box(modifier = Modifier.fill_max_width().height(20000.dp()));
            },
        );
    }));
    // A steady finger up, let go: how far the fling went, and for how long.
    let flick = |distance: f32, frames: i32| -> (f32, i32) {
        scroll_to(&rule, &scroll, 0);
        swipe(&rule, at(100.0, 380.0), at(100.0, 380.0 - distance), frames);
        let released = scroll.value();
        let (mut last, mut ms, mut still) = (released, 0, 0);
        while still < 3 && ms < 10000 {
            // till it stops moving
            rule.advance_frame();
            ms += 16;
            still = if scroll.value() == last { still + 1 } else { 0 };
            last = scroll.value();
        }
        ((scroll.value() - released) as f32, ms - 3 * 16)
    };
    let (slow, slow_ms) = flick(160.0, 16); // 625 px/s
    let (fast, fast_ms) = flick(320.0, 8); // 2500 px/s
    let calculator = FlingCalculator::new(PlatformFlingScrollFriction, Density::new(1.0, 1.0));
    let (slow_spline, fast_spline) = (calculator.fling_distance(625.0), calculator.fling_distance(2500.0));
    // Four times as fast, eleven times as far (an exponential decay: four).
    assert!(near(slow, slow_spline, 0.15 * slow_spline), "{slow} {slow_spline}");
    assert!(near(fast, fast_spline, 0.1 * fast_spline) && fast > 9.0 * slow, "{fast} {fast_spline}");
    assert!(
        (slow_ms as i64 - calculator.fling_duration(625.0)).abs() < 50,
        "{slow_ms} {}",
        calculator.fling_duration(625.0)
    );
    assert!(
        (fast_ms as i64 - calculator.fling_duration(2500.0)).abs() < 50,
        "{fast_ms} {}",
        calculator.fling_duration(2500.0)
    );
}

/// Whether the frame's clips include `want`.
fn has_clip(canvas: &RecordingCanvas, want: Rect) -> bool {
    canvas.clips.iter().any(|r| {
        near(r.left, want.left, 0.01)
            && near(r.top, want.top, 0.01)
            && near(r.right, want.right, 0.01)
            && near(r.bottom, want.bottom, 0.01)
    })
}

fn scroll_containers_clip_along_their_axis_and_leave_room_across() {
    // C++ 25032's scroll container part (its LazyRow is the lazy lists'): its height, 30 more each side across.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            modifier = Modifier.padding(40.dp()),
            content = || {
                let s = remember_scroll_state();
                Column(
                    modifier = Modifier.size_wh(200.dp(), 100.dp()).vertical_scroll(&s),
                    content = || {
                        Box(modifier = Modifier.size_wh(200.dp(), 300.dp()));
                    },
                );
                let r = remember_scroll_state();
                Row(
                    modifier = Modifier.size_wh(200.dp(), 100.dp()).horizontal_scroll(&r),
                    content = || {
                        Box(modifier = Modifier.size_wh(300.dp(), 100.dp()));
                    },
                );
            },
        );
    }));
    let canvas = rule.draw();
    assert!(has_clip(&canvas, Rect::new(10.0, 40.0, 270.0, 140.0)), "{:?}", canvas.clips); // the column
    assert!(has_clip(&canvas, Rect::new(40.0, 110.0, 240.0, 270.0)), "{:?}", canvas.clips); // the row
}

fn clip_scrollable_container_clips_along_its_axis() {
    // C++ 26171: its size along its axis, 30 more each side across; what hangs out across is clicked there, not
    // past it; the same modifier turned the other way clips the other way.
    let clicks = Rc::new(Cell::new(0));
    let c = clicks.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let c = c.clone();
        Column(
            modifier = Modifier.padding(40.dp()),
            content = move || {
                Box(
                    modifier = Modifier.size_wh(200.dp(), 100.dp()).clip_scrollable_container(Orientation::Vertical),
                    content = || {
                        Box(modifier = Modifier.size(300.dp()));
                    },
                );
                let c = c.clone();
                Box(
                    modifier = Modifier.size_wh(200.dp(), 100.dp()).clip_scrollable_container(Orientation::Horizontal),
                    content = move || {
                        let c = c.clone();
                        // 10 px in its 30 below, 30 past it.
                        Box(modifier = Modifier
                            .offset(0.dp(), 100.dp())
                            .size_wh(80.dp(), 40.dp())
                            .clickable(move || c.set(c.get() + 1)));
                    },
                );
            },
        );
    }));
    let canvas = rule.draw();
    assert!(has_clip(&canvas, Rect::new(10.0, 40.0, 270.0, 140.0)), "{:?}", canvas.clips);
    assert!(has_clip(&canvas, Rect::new(40.0, 110.0, 240.0, 270.0)), "{:?}", canvas.clips);
    let mouse_tap = |p: Offset| {
        rule.host().pointer_down(p, -1, 0, PointerType::Mouse);
        rule.advance_frame();
        rule.host().pointer_up(p, -1, 0);
        rule.wait_for_idle();
    };
    mouse_tap(at(80.0, 275.0)); // past it: cut off
    assert_eq!(clicks.get(), 0);
    mouse_tap(at(80.0, 245.0)); // 240..270 shows
    assert_eq!(clicks.get(), 1);
    // Moved there (as a mouse goes), past it again: cut off.
    rule.host().pointer_move(at(80.0, 275.0), -1, 0);
    rule.advance_frame();
    mouse_tap(at(80.0, 275.0));
    assert_eq!(clicks.get(), 1);
    // The same text turned the other way clips the other way (not skipped as if nothing changed).
    let vertical = mutable_state_of(true);
    let v = vertical.clone();
    let turned = KompozTestRule::new(kompozable_lambda!(move || {
        let orientation = if v.get() { Orientation::Vertical } else { Orientation::Horizontal };
        Text(
            "Kart",
            modifier = Modifier.padding(40.dp()).size_wh(200.dp(), 100.dp()).clip_scrollable_container(orientation),
        );
    }));
    vertical.set(false);
    turned.wait_for_idle();
    let after = turned.draw();
    assert!(has_clip(&after, Rect::new(40.0, 10.0, 240.0, 170.0)), "{:?}", after.clips);
}

fn what_shows_past_a_scroll_container_across_counts_as_shown() {
    // C++ 26208's scroll container part (its LazyRows are the lazy lists'): 20 px out of a column's side, in its
    // 30, all of it shows: visible (Compose's onVisibilityChanged goes by the window). Its bounds in the window
    // are cut at the column's side all the same: Compose's `rectInParent` clips a clipping layer's bounds to its
    // size, not its outline (C++ took the outline: 20..70).
    let visible = Rc::new(Cell::new(None::<bool>));
    let shown = Rc::new(Cell::new(Rect::Zero));
    let (v, s) = (visible.clone(), shown.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, s) = (v.clone(), s.clone());
        Column(
            modifier = Modifier.padding(40.dp()),
            content = move || {
                let (v, s) = (v.clone(), s.clone());
                let scroll = remember_scroll_state();
                Column(
                    modifier = Modifier.size_wh(200.dp(), 100.dp()).vertical_scroll(&scroll),
                    content = move || {
                        let (v, s) = (v.clone(), s.clone());
                        Box(modifier = named_args!(
                            Modifier
                                .size(50.dp())
                                .offset((-20).dp(), 0.dp())
                                .on_visibility_changed(callback = move |shown: bool| v.set(Some(shown)))
                        )
                        .on_globally_positioned(move |c| s.set(c.bounds_in_window())));
                    },
                );
            },
        );
    }));
    rule.wait_for_idle();
    assert_eq!(visible.get(), Some(true));
    assert!(near(shown.get().left, 40.0, 0.01) && near(shown.get().width(), 30.0, 0.01), "{:?}", shown.get());
}

fn on_globally_positioned_tells_where_a_node_ended_up() {
    // C++ 26326's scroll part: below the view nothing of it shows; scrolled into view, its place on the screen
    // changes (told again), not in its parent. Placed again all the same, where it was: the scroll is read in the
    // scroll node's placement, which lays the column out again, and Compose calls `onPlaced` on each placement
    // ("even if it was already placed": MeasurePassDelegate.onNodePlaced; C++ counted one).
    let (inner, placed) = (Rc::new(RefCell::new(Vec::<LayoutCoordinates>::new())), Rc::new(Cell::new(0)));
    let scroll = ScrollState::new(0);
    let (i, p, s) = (inner.clone(), placed.clone(), scroll.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (i, p, s) = (i.clone(), p.clone(), s.clone());
        Column(
            modifier = Modifier.padding(20.dp()),
            content = move || {
                let (i, p) = (i.clone(), p.clone());
                Column(
                    modifier = Modifier.height(200.dp()).fill_max_width().vertical_scroll(&s),
                    content = move || {
                        Box(modifier = Modifier.height(300.dp()));
                        let (i, p) = (i.clone(), p.clone());
                        Box(modifier = Modifier
                            .size_wh(80.dp(), 40.dp())
                            .padding(10.dp())
                            .on_globally_positioned(move |c| i.borrow_mut().push(c.clone()))
                            .on_placed(move |_| p.set(p.get() + 1)));
                    },
                );
            },
        );
    }));
    assert!(inner.borrow().len() == 1 && placed.get() == 1);
    {
        let c = &inner.borrow()[0];
        // Inside the padding, below the view: nothing of it shows.
        assert!(c.size() == IntSize::new(60, 20) && c.position_in_root() == at(30.0, 330.0));
        assert!(c.bounds_in_root().height() == 0.0 && c.position_in_parent().y == 310.0, "{:?}", c.bounds_in_root());
    }
    scroll_to(&rule, &scroll, 150); // into view: its place on the screen changes, not in its parent
    assert!(inner.borrow().len() == 2 && placed.get() == 2, "{} {}", inner.borrow().len(), placed.get());
    let c = &inner.borrow()[1];
    // As far as it goes: 340 - 200.
    assert!(
        c.position_in_root().y == 190.0 && c.bounds_in_root().height() == 20.0 && c.position_in_parent().y == 310.0
    );
}

fn offscreen_clipped_content_is_not_drawn() {
    // C++ 3853: far down a scroll container, what clips, fades or turns isn't drawn where the screen doesn't show
    // it (the canvas culls it, as a real screen's does); what has no clip of its own is drawn, left to the screen's
    // clip. Not culled, everything is drawn through its clip, layer and transform.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let scroll = remember_scroll_state();
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll(&scroll),
            content = || {
                Text("görünen");
                Box(modifier = Modifier.height(2000.dp()));
                Box(
                    modifier = Modifier.size(80.dp()).clip(RoundedCornerShape(12.dp())),
                    content = || Text("kırpılmış"),
                );
                Box(modifier = Modifier.size(80.dp()).alpha(0.5), content = || Text("soluk"));
                Box(
                    modifier = Modifier.size(80.dp()).graphics_layer_block(|l| l.rotation_z = 30.0),
                    content = || Text("dönen"),
                );
                Text("düz"); // no clip of its own: drawn and left to the screen's clip
            },
        );
    }));
    let mut canvas = RecordingCanvas::new();
    canvas.visible = Some(Rect::new(0.0, 0.0, 400.0, 800.0));
    rule.host().draw(&mut canvas);
    let drawn = |c: &RecordingCanvas, text: &str| c.texts.iter().any(|t| t == text);
    assert!(drawn(&canvas, "görünen") && drawn(&canvas, "düz"), "{:?}", canvas.texts);
    assert!(
        !drawn(&canvas, "kırpılmış") && !drawn(&canvas, "soluk") && !drawn(&canvas, "dönen") && canvas.layers == 0,
        "{:?} {}",
        canvas.texts,
        canvas.layers
    );
    let all = rule.draw();
    assert!(drawn(&all, "kırpılmış") && all.layers == 1 && all.clip_shapes.len() == 1, "{:?}", all.texts);
    assert_eq!(all.transforms.len(), 1); // the faded one's layer; the turned one's transform
}

/// A spec putting what it brings in `inset` px from the start (C++'s `BringIntoViewSpec{offset - inset}`).
#[derive(PartialEq)]
struct InsetSpec(f32);

impl BringIntoViewSpec for InsetSpec {
    fn calculate_scroll_distance(&self, offset: f32, _size: f32, _container_size: f32) -> f32 {
        offset - self.0
    }
}

fn bring_into_view_spec_puts_the_focused_item_where_it_says() {
    // C++ 25046's scroll container part (its LazyColumn and LazyRow are the lazy lists'): under a
    // LocalBringIntoViewSpec, a focused box goes where it says; not under it, the default: the end just shows; one
    // bigger than the view, its start. Two scroll modifiers of one state are the same, under a spec or not (Compose
    // reads the spec in the node: C++ took one under a spec as never the same).
    let scrolled: Vec<FocusRequester> = (0..20).map(|_| FocusRequester::new()).collect();
    let plain: Vec<FocusRequester> = (0..20).map(|_| FocusRequester::new()).collect();
    let big = FocusRequester::new();
    let same = Rc::new(Cell::new((false, false)));
    let (sc, pl, bg, sm) = (scrolled.clone(), plain.clone(), big.clone(), same.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (sc, pl, bg, sm) = (sc.clone(), pl.clone(), bg.clone(), sm.clone());
            Row(
                modifier = Modifier.fill_max_size(),
                content = move || {
                    let sc = sc.clone();
                    let sm2 = sm.clone();
                    let at100: Rc<dyn BringIntoViewSpec> = Rc::new(InsetSpec(100.0));
                    KompozitionLocalProvider(&[LocalBringIntoViewSpec.provides(at100)], move || {
                        let st = ScrollState::new(0);
                        let with = Modifier.vertical_scroll(&st) == Modifier.vertical_scroll(&st);
                        sm2.set((with, sm2.get().1));
                        let scroll = remember_scroll_state();
                        let sc = sc.clone();
                        Column(
                            modifier = Modifier.width(100.dp()).fill_max_height().vertical_scroll(&scroll),
                            content = move || {
                                for (i, r) in sc.iter().enumerate() {
                                    Box(modifier = Modifier
                                        .fill_max_width()
                                        .height(50.dp())
                                        .focus_requester(r)
                                        .focusable()
                                        .test_tag(format!("s{i}")));
                                }
                            },
                        );
                    });
                    let st = ScrollState::new(0);
                    let without = Modifier.vertical_scroll(&st) == Modifier.vertical_scroll(&st);
                    sm.set((sm.get().0, without));
                    // Not under it: just far enough.
                    let scroll = remember_scroll_state();
                    let (pl, bg) = (pl.clone(), bg.clone());
                    Column(
                        modifier = Modifier.width(100.dp()).fill_max_height().vertical_scroll(&scroll),
                        content = move || {
                            for (i, r) in pl.iter().enumerate() {
                                Box(modifier = Modifier
                                    .fill_max_width()
                                    .height(50.dp())
                                    .focus_requester(r)
                                    .focusable()
                                    .test_tag(format!("d{i}")));
                            }
                            // Taller than the column.
                            Box(modifier = Modifier
                                .fill_max_width()
                                .height(700.dp())
                                .focus_requester(&bg)
                                .focusable()
                                .test_tag("büyük"));
                        },
                    );
                },
            );
        }),
        IntSize::new(500, 500),
    );
    assert_eq!(same.get(), (true, true));
    scrolled[12].request_focus(); // a scrolling column under it: 100 in
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "s12").top, 100.0, 0.5), "{:?}", bounds(&rule, "s12"));
    // The default: the end just shows.
    plain[14].request_focus();
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "d14").bottom, 500.0, 0.5), "{:?}", bounds(&rule, "d14"));
    big.request_focus(); // bigger than the view: its start shows, not its end
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "büyük").top, 0.0, 0.5), "{:?}", bounds(&rule, "büyük"));
}

fn a_trackpad_pan_scrolls_with_the_fingers_then_flings() {
    // Compose's TrackpadScrollingLogic: a two-finger pan's moves scroll the content with the fingers at once (no
    // touch slop), across it nothing; let go, a fling of their speed; the mouse wheel's notches are another way.
    let scroll = ScrollState::new(0);
    let s = scroll.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll(&s),
            content = || {
                Box(modifier = Modifier.fill_max_width().height(5000.dp()));
            },
        );
    }));
    let h = rule.host();
    let p = at(100.0, 100.0);
    assert!(h.pan(p, PointerEventType::PanStart, Offset::Zero) || scroll.value() == 0);
    for _ in 1..=5 {
        rule.advance_frame();
        h.pan(p, PointerEventType::PanMove, at(0.0, 10.0)); // the fingers up (a pan offset is the scroll they make)
    }
    rule.advance_frame();
    assert_eq!(scroll.value(), 50);
    // Across a vertical list: nothing.
    h.pan(p, PointerEventType::PanMove, at(30.0, 0.0));
    rule.advance_frame();
    assert_eq!(scroll.value(), 50);
    h.pan(p, PointerEventType::PanEnd, Offset::Zero);
    let released = scroll.value();
    rule.wait_for_idle();
    assert!(scroll.value() > released + 10, "{released} {}", scroll.value());
}

fn swipe_with_velocity_ends_at_the_velocity_it_was_asked_for() {
    // Compose's swipeWithVelocity: whatever the distance and duration, the velocity a tracker measures at its end
    // is the one asked for (a draggable is told it as it is let go).
    let velocity = Rc::new(Cell::new(0.0f32));
    let v = velocity.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let v = v.clone();
        let stopped: OnDragStopped = Rc::new(move |_, released| {
            v.set(released);
            Box::pin(async {})
        });
        let state = remember_draggable_state(|_| {});
        Box(modifier = named_args!(Modifier.fill_max_size().draggable(
            state,
            Orientation::Vertical,
            on_drag_stopped = Some(stopped.clone())
        ))
        .test_tag("alan"));
    }));
    for (end_velocity, duration) in [(1500.0, None), (4000.0, Some(300)), (500.0, None)] {
        rule.on_node_with_tag("alan").perform_touch_input(|t| {
            named_args!(t.swipe_with_velocity(
                at(200.0, 600.0),
                at(200.0, 200.0),
                end_velocity,
                duration_millis = duration
            ))
        });
        rule.wait_for_idle();
        assert!(near(velocity.get(), -end_velocity, end_velocity * 0.02), "{end_velocity}: {}", velocity.get());
    }
}

tests!(
    vertical_scroll_moves_content,
    the_mouse_wheel_scrolls_smoothly_as_compose,
    horizontal_scroll_modifier,
    drag_gestures_move_an_offset_and_win_over_scrolling,
    scrollable_reports_drags_in_the_finger_direction,
    bring_into_view_requesters_scroll_their_node_into_view,
    a_focus_request_brings_the_node_into_view,
    scroll_container_removed_mid_drag_does_not_crash,
    scroll_state_animates_to_a_value,
    scroll_states_tell_their_value_and_whether_they_scroll,
    scrolling_turned_off_for_the_user_and_scrolling_from_the_end,
    nested_scrollables_hand_over_at_the_edge,
    a_scrollable_takes_its_drags_from_an_outer_drag_handler,
    rtl_horizontal_scrolling_starts_at_the_right_and_follows_the_finger,
    focus_moved_by_keys_scrolls_into_view_and_so_does_the_keyboard,
    a_press_in_a_scrollable_shows_after_the_tap_timeout,
    scrollable_states_hear_the_users_drags,
    flings_go_along_androids_spline,
    scroll_containers_clip_along_their_axis_and_leave_room_across,
    clip_scrollable_container_clips_along_its_axis,
    what_shows_past_a_scroll_container_across_counts_as_shown,
    on_globally_positioned_tells_where_a_node_ended_up,
    offscreen_clipped_content_is_not_drawn,
    bring_into_view_spec_puts_the_focused_item_where_it_says,
    a_trackpad_pan_scrolls_with_the_fingers_then_flings,
    swipe_with_velocity_ends_at_the_velocity_it_was_asked_for,
);
