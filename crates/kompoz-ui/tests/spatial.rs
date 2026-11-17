//! ui's spatial callbacks: `on_layout_rect_changed` with its throttle and debounce, `on_visibility_changed`,
//! `on_first_visible`, `layout_bounds`, a node's registration, occlusions; scrolled by foundation's
//! `vertical_scroll`.

#![allow(non_snake_case)]

mod common;

use common::*;
use kompoz_foundation::gestures::ScrollableState;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::layout::LayoutBoundsHolder;
use kompoz_ui::spatial::{RegistrationHandle, RelativeLayoutBounds};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Scrolls `state` to `value` px at once, as a frame's drag does (`dispatch_raw_delta`): laid out at the next
/// frame.
fn set_scroll(state: &ScrollState, value: i32) {
    state.dispatch_raw_delta((value - state.value()) as f32);
}

/// What a callback was told: who, where in the window (y), how much of it is in the window.
#[derive(Clone, Debug)]
struct Told {
    who: char,
    y: i32,
    visible: f32,
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

fn on_layout_rect_changed_debounces_and_throttles() {
    // C++'s on_layout_rect_changed_debounces_and_throttles.
    let told: Rc<RefCell<Vec<Told>>> = Rc::new(RefCell::new(Vec::new()));
    let shown = mutable_state_of(true);
    let scroll = ScrollState::new(0);
    let (t, sh, sc) = (told.clone(), shown.clone(), scroll.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let report = |who: char| {
                let t = t.clone();
                move |b: &RelativeLayoutBounds| {
                    t.borrow_mut().push(Told {
                        who,
                        y: b.position_in_window().y,
                        visible: b.fraction_visible_in_window(),
                    })
                }
            };
            let (a, b, c) = (report('a'), report('b'), report('c'));
            Column(Modifier.fill_max_size().vertical_scroll(&sc), || {
                Box(Modifier.fill_max_width().height(200.dp()));
                // At 200: debounced 64 ms.
                if sh.get() {
                    Box(Modifier.fill_max_width().height(100.dp()).on_layout_rect_changed(callback = a));
                }
                // At 300: throttled.
                Box(Modifier.fill_max_width().height(100.dp()).on_layout_rect_changed(100, 0, callback = b));
                // At 400: at once.
                Box(Modifier.fill_max_width().height(100.dp()).on_layout_rect_changed(0, 0, callback = c));
                Box(Modifier.fill_max_width().height(1000.dp()));
            });
        }),
        IntSize::new(400, 450),
    );
    let of = |who: char| told.borrow().iter().filter(|t| t.who == who).cloned().collect::<Vec<_>>();
    // Laid out (waited for until all is still): each told once, c half in the window.
    rule.wait_for_idle();
    assert_eq!((of('a').len(), of('b').len(), of('c').len()), (1, 1, 1));
    assert!(of('a')[0].y == 200 && near(of('a')[0].visible, 1.0));
    assert!(of('c')[0].y == 400 && near(of('c')[0].visible, 0.5));
    // Scrolled 10 a frame (16 ms) for 12 frames: c each frame; b at once, then not for 100 ms (the 8th frame,
    // 112 ms on); a not while it moves.
    rule.advance_time_by(200);
    told.borrow_mut().clear();
    for i in 1..=12 {
        set_scroll(&scroll, 10 * i);
        rule.advance_frame();
    }
    let c = of('c');
    assert!(c.len() == 12 && c[11].y == 400 - 120 && near(c[11].visible, 1.0));
    let b = of('b');
    assert!(b.len() == 2 && b[0].y == 300 - 10 && b[1].y == 300 - 80, "{b:?}");
    assert!(of('a').is_empty());
    // Stopped: a told where it stopped, b its last place once the throttle lets it.
    rule.wait_for_idle();
    let (a, b) = (of('a'), of('b'));
    assert!(a.len() == 1 && a[0].y == 200 - 120, "{a:?}");
    assert!(b.len() == 3 && b[2].y == 300 - 120, "{b:?}");
    assert_eq!(of('c').len(), 12);
    // Leaving the layout, even while its new place waits to be told: nothing told (b and c move up, and are
    // told).
    told.borrow_mut().clear();
    set_scroll(&scroll, 130);
    rule.advance_frame();
    shown.set(false);
    rule.wait_for_idle();
    let (a, b, c) = (of('a'), of('b'), of('c'));
    assert!(a.is_empty());
    assert!(c.len() == 2 && c[1].y == 300 - 130, "{c:?}");
    // Its throttle still running at the scroll: the latest place only.
    assert!(b.len() == 1 && b[0].y == 200 - 130, "{b:?}");
}

fn visibility_is_told_as_nodes_come_and_go() {
    // C++'s visibility_is_told_as_nodes_come_and_go.
    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let (shown, placed) = (mutable_state_of(true), mutable_state_of(true));
    let (scroll, inner) = (ScrollState::new(0), ScrollState::new(0));
    let holder = LayoutBoundsHolder::new();
    let (l, sh, pl, sc, inn, h) =
        (log.clone(), shown.clone(), placed.clone(), scroll.clone(), inner.clone(), holder.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let note = |name: &'static str| {
                let l = l.clone();
                move |v: bool| l.borrow_mut().push(format!("{name} {}", if v { "+" } else { "-" }))
            };
            let first = {
                let l = l.clone();
                move || l.borrow_mut().push("ilk".to_string())
            };
            let (top, going, settled, bottom, timed, boxed) =
                (note("üst"), note("gidecek"), note("yerleşik"), note("alt"), note("süreli"), note("kutuda"));
            let (pl, h, inn) = (pl.clone(), h.clone(), inn.clone());
            Column(Modifier.fill_max_size().vertical_scroll(&sc), || {
                // (taking the second one away moves nothing)
                Row(|| {
                    Box(Modifier.size(100.dp()).on_visibility_changed(callback = top));
                    if sh.get() {
                        Box(Modifier.size(50.dp()).on_visibility_changed(callback = going));
                    }
                    let pl = pl.clone();
                    Layout(
                        content = || Box(Modifier.size(50.dp()).on_visibility_changed(callback = settled)),
                        modifier = Modifier.size(50.dp()),
                        measure_policy = measure_policy(move |scope, ms, c| {
                            let p = ms[0].measure(c);
                            let pl = pl.clone();
                            // Measured but left out: not visible.
                            scope.layout(50, 50, move |s| {
                                if pl.get() {
                                    s.place(&p, 0, 0);
                                }
                            })
                        }),
                    );
                });
                Box(Modifier.height(900.dp()));
                // At 1000.
                Box(Modifier.size(100.dp()).on_visibility_changed(min_fraction_visible = 0.5, callback = bottom));
                // At 1100.
                Box(Modifier.size(100.dp()).on_first_visible(min_duration_ms = 500, callback = first));
                // At 1200.
                Box(Modifier.size(100.dp()).on_visibility_changed(min_duration_ms = 300, callback = timed));
                // At 1300, a box of its own.
                Column(
                    Modifier.fill_max_width().height(200.dp()).layout_bounds(h.clone()).vertical_scroll(&inn),
                    || {
                        Box(Modifier.height(250.dp()));
                        Box(Modifier
                            .size(50.dp())
                            .on_visibility_changed(viewport_bounds = Some(h.clone()), callback = boxed));
                        Box(Modifier.height(300.dp()));
                    },
                );
                Box(Modifier.height(2000.dp()));
            });
        }),
        IntSize::new(400, 800),
    );
    let logged = || log.borrow().clone();
    let count = |what: &str| log.borrow().iter().filter(|l| *l == what).count();
    rule.wait_for_idle();
    // What shows at first; the rest isn't told.
    assert_eq!(logged(), ["üst +", "gidecek +", "yerleşik +"]);
    // Gone while visible: not visible.
    shown.set(false);
    rule.wait_for_idle();
    assert_eq!(logged(), ["üst +", "gidecek +", "yerleşik +", "gidecek -"]);
    // Still there, but no longer placed.
    placed.set(false);
    rule.wait_for_idle();
    assert_eq!(logged().last().map(String::as_str), Some("yerleşik -"));
    log.borrow_mut().clear();
    // "alt" 750..850: half of it in, not more; "üst" out.
    set_scroll(&scroll, 250);
    rule.wait_for_idle();
    assert_eq!(logged(), ["üst -"]);
    // 60% of it in.
    set_scroll(&scroll, 260);
    rule.wait_for_idle();
    assert_eq!(logged(), ["üst -", "alt +"]);
    log.borrow_mut().clear();
    // The first-visible one all in (650..750): told after 500 ms of it (waiting for idle would run them out).
    set_scroll(&scroll, 450);
    rule.advance_time_by(300);
    assert!(logged().is_empty());
    rule.advance_time_by(300);
    assert_eq!(logged(), ["ilk"]);
    set_scroll(&scroll, 0);
    rule.wait_for_idle();
    set_scroll(&scroll, 450);
    rule.wait_for_idle();
    rule.advance_time_by(1000);
    // Only the first time.
    assert_eq!(count("ilk"), 1);
    set_scroll(&scroll, 0);
    rule.wait_for_idle();
    log.borrow_mut().clear();
    set_scroll(&scroll, 450);
    rule.wait_for_idle();
    assert_eq!(logged(), ["üst -", "alt +"]);
    // "süreli" all in: waiting for the app to settle runs its 300 ms out.
    set_scroll(&scroll, 550);
    rule.wait_for_idle();
    assert_eq!(logged().last().map(String::as_str), Some("süreli +"));
    // In its box's viewport: the box at 500..700 on screen, its item at 750..800: in the window, off the box.
    log.borrow_mut().clear();
    set_scroll(&scroll, 800);
    rule.wait_for_idle();
    assert_eq!(count("kutuda +"), 0);
    // The item at 550..600, in the box.
    set_scroll(&inner, 200);
    rule.wait_for_idle();
    assert_eq!(count("kutuda +"), 1);
}

fn layout_bounds_and_relative_bounds_as_compose() {
    let holder = LayoutBoundsHolder::new();
    let shown = mutable_state_of(true);
    let read: Rc<RefCell<Vec<Option<IntRect>>>> = Rc::new(RefCell::new(Vec::new()));
    let (h, sh, r) = (holder.clone(), shown.clone(), read.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            // Read in a composition: composed again when they change.
            r.borrow_mut().push(h.bounds().map(|b| b.bounds_in_root()));
            let h = h.clone();
            Column(Modifier.padding(10.dp()), || {
                Box(Modifier.size_wh(100.dp(), 40.dp()));
                if sh.get() {
                    Box(Modifier
                        .offset(x = 7.dp())
                        .size_wh(50.dp(), 30.dp())
                        .layout_bounds(h.clone())
                        .test_tag("held"));
                }
            });
        }),
        IntSize::new(200, 300),
    );
    rule.wait_for_idle();
    let bounds = holder.bounds().expect("laid out: its bounds");
    // The layout's own bounds (its outer coordinates: the offset moves only what it wraps).
    assert_eq!(bounds.bounds_in_root(), IntRect::new(10, 50, 60, 80));
    assert_eq!((bounds.width(), bounds.height()), (50, 30));
    assert_eq!(bounds.position_in_root(), IntOffset::new(10, 50));
    assert_eq!(bounds.position_in_window(), IntOffset::new(10, 50));
    assert_eq!(bounds.bounds_in_screen(), bounds.bounds_in_window());
    assert!(near(bounds.fraction_visible_in_window(), 1.0));
    // Less the window's top 70: 10 of its 30 rows left.
    assert!(near(bounds.fraction_visible_in_window_with_insets(IntOffset::new(0, 70), IntOffset::Zero), 1.0 / 3.0));
    // In a smaller rectangle: over the smaller of the two areas.
    assert!(near(bounds.fraction_visible_in_rect(0, 0, 35, 300), 0.5));
    assert!(near(bounds.fraction_visible_in_rect(20, 60, 30, 70), 1.0));
    assert_eq!(read.borrow().last().copied().flatten(), Some(IntRect::new(10, 50, 60, 80)));
    // The window moved: its window bounds with it.
    rule.host().set_window_offset(Offset::new(5.0, 6.0));
    rule.wait_for_idle();
    assert_eq!(holder.bounds().unwrap().position_in_window(), IntOffset::new(15, 56));
    // Gone: none.
    shown.set(false);
    rule.wait_for_idle();
    assert!(holder.bounds().is_none());
    assert_eq!(read.borrow().last().copied().flatten(), None);
    assert_eq!(holder, holder.clone());
    assert_ne!(holder, LayoutBoundsHolder::new());
}

fn occlusions_are_what_is_drawn_over_it() {
    let holder = LayoutBoundsHolder::new();
    let h = holder.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let h = h.clone();
        Box(|| {
            // Drawn first: what comes after it overlaps it.
            Box(Modifier.size(100.dp()).layout_bounds(h.clone()), content = || Box(Modifier.size(50.dp())));
            Box(Modifier.offset_px(|_| IntOffset::new(0, 0)).padding(60.dp()).size(30.dp()));
            Box(Modifier.size(20.dp()));
            // Placed apart (at the parent's bottom end, 140 by 140): no overlap.
            Box(Modifier.align(Alignment::BottomEnd).size(10.dp()));
            // Below in z order, though placed later.
            Box(Modifier.z_index(-1.0).size(40.dp()));
        });
    }));
    rule.wait_for_idle();
    let bounds = holder.bounds().unwrap();
    // Not its own child (a descendant), not the one placed apart, not the one under it.
    let mut occlusions = bounds.calculate_occlusions();
    occlusions.sort_by_key(|r| (r.width(), r.left));
    assert_eq!(occlusions, [IntRect::new(0, 0, 20, 20), IntRect::new(0, 0, 150, 150)]);
}

/// A node that registers a rect callback while attached, through `register_on_layout_rect_changed`.
struct Registering {
    seen: Rc<RefCell<Vec<IntOffset>>>,
    handle: Option<RegistrationHandle>,
    /// Unregister from inside the callback, the first time.
    once: bool,
}

impl ModifierNode for Registering {
    fn on_attach(&mut self, cx: &NodeCx) {
        let seen = self.seen.clone();
        let me = cx.handle();
        let once = self.once;
        self.handle = Some(cx.register_on_layout_rect_changed(0, 0, move |b| {
            seen.borrow_mut().push(b.position_in_root());
            if once && let Some(cx) = me.cx() {
                cx.with::<Registering, _>(|n| n.handle.take().map(RegistrationHandle::unregister));
            }
        }));
    }
    fn on_detach(&mut self, _: &NodeCx) {
        if let Some(handle) = self.handle.take() {
            handle.unregister();
        }
    }
}

struct RegisteringElement {
    seen: Rc<RefCell<Vec<IntOffset>>>,
    once: bool,
}

impl PartialEq for RegisteringElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.seen, &other.seen) && self.once == other.once
    }
}

impl ModifierNodeElement for RegisteringElement {
    type Node = Registering;
    fn create(&self) -> Registering {
        Registering { seen: self.seen.clone(), handle: None, once: self.once }
    }
    fn update(&self, node: &mut Registering) {
        node.seen = self.seen.clone();
    }
}

fn a_node_registers_rect_callbacks_and_unregisters_them() {
    let (seen, seen_once) = (Rc::new(RefCell::new(Vec::new())), Rc::new(RefCell::new(Vec::new())));
    let down = mutable_state_of(0);
    let (s, so, d) = (seen.clone(), seen_once.clone(), down.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, so, d) = (s.clone(), so.clone(), d.clone());
        Column(Modifier.offset_px(move |_| IntOffset::new(0, d.get())), || {
            Box(Modifier.then_element(RegisteringElement { seen: s.clone(), once: false }).size(10.dp()));
            Box(Modifier.then_element(RegisteringElement { seen: so.clone(), once: true }).size(10.dp()));
        });
    }));
    rule.wait_for_idle();
    assert_eq!(*seen.borrow(), [IntOffset::new(0, 0)]);
    assert_eq!(*seen_once.borrow(), [IntOffset::new(0, 10)]);
    down.set(5);
    rule.wait_for_idle();
    assert_eq!(*seen.borrow(), [IntOffset::new(0, 0), IntOffset::new(0, 5)]);
    // Unregistered from inside its own callback: told nothing more.
    assert_eq!(*seen_once.borrow(), [IntOffset::new(0, 10)]);
    down.set(9);
    rule.wait_for_idle();
    assert_eq!(seen.borrow().len(), 3);
    assert_eq!(seen_once.borrow().len(), 1);
}

fn visibility_follows_the_window_and_the_plain_forms() {
    let visible = Rc::new(Cell::new(None::<bool>));
    let firsts = Rc::new(Cell::new(0));
    let rects = Rc::new(Cell::new(0));
    let (v, f, r) = (visible.clone(), firsts.clone(), rects.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (v, f, r) = (v.clone(), f.clone(), r.clone());
            // (as long as its content, past the window)
            Column(Modifier.wrap_content_height_with(Alignment::Top, true), || {
                Box(Modifier.height(300.dp()));
                Box(Modifier
                    .size(100.dp())
                    .on_visibility_changed_with(0, 0.2, None, move |b| v.set(Some(b)))
                    .on_first_visible_with(0, 1.0, None, move || f.set(f.get() + 1))
                    .on_layout_rect_changed_with(0, 0, move |_| r.set(r.get() + 1)));
            });
        }),
        IntSize::new(400, 310),
    );
    rule.wait_for_idle();
    // 10 of its 100 rows in the window: not more than a fifth.
    assert_eq!(visible.get(), None);
    assert_eq!(firsts.get(), 0);
    // The window grows: 50 rows in, visible; then all of it, seen for the first time.
    rule.host().set_viewport_size(IntSize::new(400, 350));
    rule.wait_for_idle();
    assert_eq!(visible.get(), Some(true));
    assert_eq!(firsts.get(), 0);
    let told = rects.get();
    rule.host().set_viewport_size(IntSize::new(400, 400));
    rule.wait_for_idle();
    assert_eq!(firsts.get(), 1);
    // Told of the window's new size too.
    assert!(rects.get() > told);
    rule.host().set_viewport_size(IntSize::new(400, 300));
    rule.wait_for_idle();
    assert_eq!(visible.get(), Some(false));
    assert_eq!(firsts.get(), 1);
}

tests!(
    on_layout_rect_changed_debounces_and_throttles,
    visibility_is_told_as_nodes_come_and_go,
    layout_bounds_and_relative_bounds_as_compose,
    occlusions_are_what_is_drawn_over_it,
    a_node_registers_rect_callbacks_and_unregisters_them,
    visibility_follows_the_window_and_the_plain_forms,
);
