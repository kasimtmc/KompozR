//! Lazy lists: LazyColumn and LazyRow composing only what shows, scrolling, keys, content padding and
//! arrangements, reverse layout, sticky headers, layout info, prefetching between frames, item animations,
//! scrolling to an item (at once, animated), snapping — the C++ Kompoz's lazy list tests (survey E) and Compose's
//! behaviour where the two differ. Numbers in px (density 1).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_animation::*;
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_foundation::gestures::snapping::*;
use kompoz_foundation::lazy::layout::{LazyKey, lazy_layout_cache_window};
use kompoz_foundation::lazy::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::input::PointerType;
use kompoz_ui::layout::SlotKey;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).fetch_semantics_node().unclipped_bounds_in_root()
}

fn has(rule: &KompozTestRule, tag: &str) -> bool {
    rule.on_node_with_tag(tag).exists()
}

/// Runs a scroll of `state` by `by` px in a coroutine of its own, as C++'s synchronous `scrollBy` did; what it took.
fn scroll_by(
    rule: &KompozTestRule,
    scope: &Rc<RefCell<Option<CoroutineScope>>>,
    state: &LazyListState,
    by: f32,
) -> f32 {
    let took = Rc::new(Cell::new(f32::NAN));
    let (s, t) = (state.clone(), took.clone());
    scope.borrow().as_ref().unwrap().launch(async move {
        t.set(s.scroll_by(by).await);
    });
    rule.wait_for_idle();
    took.get()
}

fn lazy_column_composes_only_what_is_visible() {
    // C++ 1814: 10 000 items of 20 px in 800: only those in view composed.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            LazyColumn(
                modifier = Modifier.fill_max_size().test_tag("list"),
                content = |this: &mut LazyListScope| {
                    this.items_list((0..10_000).collect::<Vec<i32>>(), |v: &i32| {
                        Box(modifier = Modifier.fill_max_width().height(20.dp()).test_tag(format!("row {v}")));
                    });
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let composed = rule.on_node_with_tag("list").fetch_semantics_node().children().len();
    assert!((40..80).contains(&composed), "{composed}");
    assert!(has(&rule, "row 0") && has(&rule, "row 39") && !has(&rule, "row 500"));
    // Items much taller than a guess: as many as fill the view (Compose composes what shows; C++ composed half a
    // view more).
    let composed = Rc::new(Cell::new(0));
    let c = composed.clone();
    let tall = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let c = c.clone();
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                content = move |this: &mut LazyListScope| {
                    let c = c.clone();
                    this.items(100, move |i| {
                        c.set(c.get() + 1);
                        Box(modifier = Modifier.fill_max_width().height(500.dp()).test_tag(format!("page {i}")));
                    });
                },
            );
        }),
        IntSize::new(400, 800),
    );
    assert!(has(&tall, "page 0") && has(&tall, "page 1") && !has(&tall, "page 2"));
    assert!(composed.get() <= 4, "{}", composed.get());
}

fn lazy_column_scrolls_and_disposes_items() {
    // C++ 1922: the wheel and scroll_by scroll; items that left are disposed of.
    let alive = Rc::new(Cell::new(0));
    let state = LazyListState::new(0, 0);
    let scope: Rc<RefCell<Option<CoroutineScope>>> = Rc::new(RefCell::new(None));
    let (a, s, sc) = (alive.clone(), state.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let a = a.clone();
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                state = Some(s.clone()),
                content = move |this: &mut LazyListScope| {
                    let a = a.clone();
                    this.items_list(
                        (0..1000).collect::<Vec<i32>>(),
                        key = item_key(|v: &i32| *v),
                        item_content = move |v: &i32| {
                            let a = a.clone();
                            DisposableEffect((), move || {
                                a.set(a.get() + 1);
                                on_dispose(move || a.set(a.get() - 1))
                            });
                            Box(modifier = Modifier.fill_max_width().height(20.dp()).test_tag(format!("row {v}")));
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let alive_at_top = alive.get();
    rule.host().scroll(at(10.0, 10.0), at(0.0, 10.0)); // 10 notches: 640 px
    rule.wait_for_idle();
    assert_eq!(state.first_visible_item_index(), 32);
    assert!(near(bounds(&rule, "row 32").top, 0.0));
    scroll_by(&rule, &scope, &state, 10_000.0);
    assert_eq!(state.first_visible_item_index(), 532);
    assert!(has(&rule, "row 538") && !has(&rule, "row 0"));
    assert!(alive.get() > 0 && alive.get() < alive_at_top + 25, "{} {}", alive.get(), alive_at_top);
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

fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(p, -1, 0);
    rule.advance_frame();
}

type Scope = Rc<RefCell<Option<CoroutineScope>>>;

fn new_scope() -> Scope {
    Rc::new(RefCell::new(None))
}

fn launch(scope: &Scope, f: impl std::future::Future<Output = ()> + 'static) {
    scope.borrow().as_ref().unwrap().launch(f);
}

/// A box `size` px tall, as wide as it can be, tagged.
#[kompozable]
fn Row20(tag: String, size: f32) {
    Box(modifier = Modifier.fill_max_width().height(size.dp()).test_tag(tag));
}

fn lazy_list_composes_the_next_item_in_a_pause() {
    // C++ 1849: scrolled, the item past the composed ones that way is composed in a pause between frames (the
    // host's idle), before the frame that shows it; one whose kind took longer than the pause before waits.
    // (`made`: items composed the first time, as remember runs then.)
    let state = LazyListState::new(0, 0);
    let made = Rc::new(RefCell::new(Vec::<i32>::new()));
    let scope = new_scope();
    let (s, m, sc) = (state.clone(), made.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let m = m.clone();
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                state = Some(s.clone()),
                content = move |this: &mut LazyListScope| {
                    let m = m.clone();
                    this.items(100, move |i| {
                        let m = m.clone();
                        remember(move || m.borrow_mut().push(i));
                        Box(modifier = Modifier.fill_max_width().height(500.dp()).test_tag(format!("page {i}")));
                    });
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let has_page = |i: i32| has(&rule, &format!("page {i}"));
    let mut past = 0;
    while has_page(past) {
        past += 1;
    }
    // (not scrolled: nothing to compose ahead)
    assert!(past > 1 && !rule.host().has_idle_work());
    scroll_by(&rule, &scope, &state, 10.0);
    assert!(has_page(past - 1) && !has_page(past) && rule.host().has_idle_work());
    made.borrow_mut().clear();
    rule.host().idle(1_000_000_000);
    // (composed now, that one alone; in the layout once the next frame measures)
    assert_eq!(*made.borrow(), vec![past]);
    rule.wait_for_idle();
    // One ahead only; the frames that scroll down to it don't compose it anew.
    assert!(!rule.host().has_idle_work());
    made.borrow_mut().clear();
    scroll_by(&rule, &scope, &state, 500.0);
    assert!(!made.borrow().contains(&past) && has_page(past));
    // Back the other way: the one before the composed ones.
    let s = state.clone();
    launch(&scope, async move { s.scroll_to_item(50).await });
    rule.wait_for_idle();
    scroll_by(&rule, &scope, &state, -10.0);
    let mut before = 50;
    while has_page(before - 1) {
        before -= 1;
    }
    made.borrow_mut().clear();
    rule.host().idle(1_000_000_000);
    assert_eq!(*made.borrow(), vec![before - 1]);
    // A pause shorter than what composing a page took: not started then.
    let mut tries = 0;
    while !rule.host().has_idle_work() && tries < 100 {
        scroll_by(&rule, &scope, &state, -25.0);
        tries += 1;
    }
    made.borrow_mut().clear();
    rule.host().idle(1);
    assert!(made.borrow().is_empty() && rule.host().has_idle_work());
}

fn lazy_list_state_opens_at_an_item() {
    // C++ 1907: remembered at item 500 before the list knows its count.
    let first = Rc::new(Cell::new(-1));
    let f = first.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = remember_lazy_list_state(initial_first_visible_item_index = 500);
            f.set(without_read_observation(|| state.first_visible_item_index()));
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                state = Some(state),
                content = |this: &mut LazyListScope| {
                    this.items(1000, |i| Row20(format!("row {i}"), 20.0));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    assert!(has(&rule, "row 500") && !has(&rule, "row 0"));
    assert!(near(bounds(&rule, "row 500").top, 0.0));
}

fn lazy_column_stops_at_the_end() {
    // C++ 2004: 50 rows of 20 px in 800: scrolled to the end, row 49 ends at the bottom; no more forward.
    let state = LazyListState::new(0, 0);
    let scope = new_scope();
    let (s, sc) = (state.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                state = Some(s.clone()),
                content = |this: &mut LazyListScope| {
                    this.items(50, |i| Row20(format!("row {i}"), 20.0));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    scroll_by(&rule, &scope, &state, 5000.0);
    assert!(near(bounds(&rule, "row 49").bottom, 800.0));
    assert!(!state.can_scroll_forward());
    assert_eq!(scroll_by(&rule, &scope, &state, 100.0), 0.0);
    assert_eq!(scroll_by(&rule, &scope, &state, -100.0), -100.0);
}

#[kompozable]
fn KeyedItem(v: i32) {
    let count = remember(|| mutable_state_of(0));
    let c = count.clone();
    BasicText(format!("item{v}:{}", count.get()), modifier = Modifier.clickable(move || c.set(c.get() + 1)));
}

fn lazy_column_keys_keep_item_state() {
    // C++ 2022: an item's state follows its key when an item is inserted before it.
    let data = mutable_state_of(vec![1, 2, 3]);
    let d = data.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let d = d.clone();
        LazyColumn(
            modifier = Modifier.fill_max_size(),
            content = move |this: &mut LazyListScope| {
                this.items_list(d.value(), key = item_key(|v: &i32| *v), item_content = |v: &i32| KeyedItem(*v));
            },
        );
    }));
    rule.on_node_with_text("item2:0").perform_click();
    data.set(vec![0, 1, 2, 3]);
    rule.wait_for_idle();
    rule.on_node_with_text("item2:1").assert_exists();
    rule.on_node_with_text("item0:0").assert_exists();
}

fn lazy_column_content_padding_and_spacing() {
    // C++ 2036: the padding before the first item, the spacing between them.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        LazyColumn(
            modifier = Modifier.fill_max_size(),
            content_padding = PaddingValues(start = 8.dp(), top = 16.dp(), end = 8.dp(), bottom = 16.dp()),
            vertical_arrangement = Some(Arrangement::spaced_by(10.dp())),
            content = |this: &mut LazyListScope| {
                this.item(|| Row20("header".into(), 20.0));
                this.items(5, |i| Row20(format!("row {i}"), 20.0));
            },
        );
    }));
    assert!(near(bounds(&rule, "header").top, 16.0));
    assert!(near(bounds(&rule, "header").left, 8.0));
    assert!(near(bounds(&rule, "row 0").top, 16.0 + 20.0 + 10.0));
}

fn lazy_column_scroll_to_item_and_observable_index() {
    // C++ 2050: the first visible index read in composition recomposes as it changes.
    let state = LazyListState::new(0, 0);
    let scope = new_scope();
    let (s, sc) = (state.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let s = s.clone();
            Column(
                content = move || {
                    BasicText(format!("first={}", s.first_visible_item_index()));
                    LazyColumn(
                        modifier = Modifier.fill_max_size(),
                        state = Some(s.clone()),
                        content = |this: &mut LazyListScope| {
                            this.items(1000, |i| Row20(format!("row {i}"), 20.0));
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let s = state.clone();
    launch(&scope, async move { s.scroll_to_item(500).await });
    rule.wait_for_idle();
    rule.on_node_with_text("first=500").assert_exists();
    assert!(has(&rule, "row 500"));
}

fn lazy_row_inside_lazy_column_scrolls_on_its_own_axis() {
    // C++ 2069: a horizontal swipe on the row scrolls the row; a vertical one below it, the column.
    let (column, row) = (LazyListState::new(0, 0), LazyListState::new(0, 0));
    let (c, r) = (column.clone(), row.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let r = r.clone();
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                state = Some(c.clone()),
                content = move |this: &mut LazyListScope| {
                    let r = r.clone();
                    this.item(move || {
                        LazyRow(
                            modifier = Modifier.fill_max_width().height(60.dp()),
                            state = Some(r.clone()),
                            content = |this: &mut LazyListScope| {
                                this.items(50, |v| {
                                    Box(modifier = Modifier.size(60.dp()).test_tag(format!("cell{v}")));
                                });
                            },
                        );
                    });
                    this.items(100, |i| Row20(format!("row {i}"), 20.0));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    swipe(&rule, at(300.0, 30.0), at(60.0, 30.0), 10);
    rule.wait_for_idle();
    assert!(row.first_visible_item_index() >= 4, "{}", row.first_visible_item_index());
    assert_eq!(column.first_visible_item_index(), 0);
    swipe(&rule, at(200.0, 700.0), at(200.0, 300.0), 10);
    rule.wait_for_idle();
    assert!(column.first_visible_item_index() > 0);
}

fn fling_keeps_scrolling_after_release() {
    // C++ 2093: 400 px in 80 ms: the fling goes on after the finger leaves, and ends by itself.
    let state = LazyListState::new(0, 0);
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                state = Some(s.clone()),
                content = |this: &mut LazyListScope| {
                    this.items(5000, |i| Row20(format!("row {i}"), 20.0));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    swipe(&rule, at(200.0, 700.0), at(200.0, 300.0), 5);
    let at_release = state.first_visible_item_index();
    rule.wait_for_idle();
    let after_fling = state.first_visible_item_index();
    assert!(at_release >= 15, "{at_release}");
    assert!(after_fling > at_release + 20, "{after_fling} {at_release}");
    assert!(!rule.host().has_pending_work());
}

#[kompozable]
fn LikeRow(i: i32) {
    let liked = remember_saveable(|| mutable_state_of(false));
    let plain = remember(|| mutable_state_of(false));
    let (l, p) = (liked.clone(), plain.clone());
    Row(
        modifier = Modifier.fill_max_width().height(50.dp()),
        content = move || {
            BasicText(
                format!(
                    "satır {i} {} {}",
                    if l.get() { "beğenildi" } else { "-" },
                    if p.get() { "işaretli" } else { "-" }
                ),
                modifier = Modifier.weight(1.0),
            );
            let (l, p) = (l.clone(), p.clone());
            // (a clickable box: Material's Button is stage 12's)
            Box(modifier = Modifier.size(40.dp()).test_tag(format!("like{i}")).clickable(move || {
                l.set(true);
                p.set(true);
            }));
        },
    );
}

fn lazy_column_items_keep_saveable_state_when_scrolled_away() {
    // C++ 6203: remember_saveable comes back with its item, remember doesn't.
    let list = LazyListState::new(0, 0);
    let scope = new_scope();
    let (l, sc) = (list.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                state = Some(l.clone()),
                content = |this: &mut LazyListScope| {
                    this.items(200, key = index_key(|i| i), item_content = |i| LikeRow(i));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.on_node_with_tag("like3").perform_click();
    rule.on_node_with_text("satır 3 beğenildi işaretli").assert_exists();
    let s = list.clone();
    launch(&scope, async move { s.scroll_to_item(150).await });
    rule.wait_for_idle();
    assert!(!has(&rule, "like3"));
    let s = list.clone();
    launch(&scope, async move { s.scroll_to_item(0).await });
    rule.wait_for_idle();
    rule.on_node_with_text("satır 3 beğenildi -").assert_exists();
    // One whose composable leaves while its item stays goes with it: it starts over when it comes back.
    let open = mutable_state_of(true);
    let o = open.clone();
    let inner = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let o = o.clone();
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                content = move |this: &mut LazyListScope| {
                    let o = o.clone();
                    this.items(
                        3,
                        key = index_key(|i| i),
                        item_content = move |i| {
                            if i == 0 && o.get() {
                                let count = remember_saveable(|| mutable_state_of(0));
                                let c = count.clone();
                                Box(
                                    modifier = Modifier.test_tag("say").clickable(move || c.set(c.get() + 1)),
                                    content = move || BasicText(format!("sayı {}", count.get())),
                                );
                            } else {
                                Box(modifier = Modifier.size(10.dp()));
                            }
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    inner.on_node_with_tag("say").perform_click();
    inner.on_node_with_text("sayı 1").assert_exists();
    open.set(false);
    inner.wait_for_idle();
    assert!(!has(&inner, "say"));
    open.set(true);
    inner.wait_for_idle();
    inner.on_node_with_text("sayı 0").assert_exists();
}

fn lazy_items_keep_a_temporary_list() {
    // C++ 12001: lists made for the call are kept by the items.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        LazyColumn(
            modifier = Modifier.size_wh(100.dp(), 200.dp()),
            content = |this: &mut LazyListScope| {
                this.items_list(vec!["birinci".to_string(), "ikinci".to_string()], |s: &String| BasicText(s.clone()));
                this.items_indexed(vec!["üçüncü".to_string()], |i, s: &String| BasicText(format!("{s}{i}")));
            },
        );
    }));
    rule.on_node_with_text("birinci").assert_exists();
    rule.on_node_with_text("ikinci").assert_exists();
    rule.on_node_with_text("üçüncü0").assert_exists();
}

fn animate_item_glides_on_removal_and_fades_in_on_insert() {
    // C++ 12439: an item removed, those after glide up (the placement spec a linear tween here: C++'s one argument
    // was the placement's); one inserted fades in through a layer while the others glide down.
    let items = mutable_state_list_of([1, 2, 3, 4]);
    let it = items.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let it = it.clone();
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                content = move |this: &mut LazyListScope| {
                    this.items_list(
                        &it,
                        key = item_key(|v: &i32| *v),
                        item_content = |v: &i32| {
                            let spec: Arc<dyn FiniteAnimationSpec<IntOffset>> =
                                Arc::new(tween(200, 0, Arc::new(LinearEasing)));
                            Box(modifier = Modifier
                                .animate_item(placement_spec = Some(spec))
                                .fill_max_width()
                                .height(50.dp())
                                .test_tag(format!("item{v}")));
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.main_clock().set_auto_advance(false);
    assert!(near(bounds(&rule, "item3").top, 100.0));
    items.remove_at(0); // 2, 3, 4 move up by 50
    rule.advance_time_by(112);
    let mid = bounds(&rule, "item3").top;
    assert!(mid > 55.0 && mid < 95.0, "{mid}");
    settle(&rule);
    assert!(near(bounds(&rule, "item3").top, 50.0));
    items.add_at(0, 9); // inserted at the top: fades in while the others glide down
    rule.advance_time_by(48);
    assert!(rule.draw().layers >= 1, "the new item draws through an alpha layer");
    settle(&rule);
    let after = rule.draw();
    assert!(after.layers == 0 && near(bounds(&rule, "item3").top, 100.0), "{}", after.layers);
}

fn animate_item_fades_a_removed_item_out_where_it_was() {
    // C++ 12466: a removed item fades out where it was, as it last was (its composition gone: the list draws it
    // through the layer it was placed with, as Compose does — no node of it is left, so it takes no input); one
    // that comes back before it is gone is an item again.
    let items = mutable_state_list_of([1, 2, 3]);
    let tick = mutable_state_of(0);
    let composed = Rc::new(RefCell::new(std::collections::HashMap::<i32, i32>::new()));
    let clicked = Rc::new(RefCell::new(Vec::<i32>::new()));
    let (it, tk, co, cl) = (items.clone(), tick.clone(), composed.clone(), clicked.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let t = tk.get();
            let (it, co, cl) = (it.clone(), co.clone(), cl.clone());
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                content = move |this: &mut LazyListScope| {
                    let (co, cl) = (co.clone(), cl.clone());
                    this.items_list(
                        &it,
                        key = item_key(|v: &i32| *v),
                        item_content = move |v: &i32| {
                            let v = *v;
                            *co.borrow_mut().entry(v).or_insert(0) += 1;
                            let cl = cl.clone();
                            let fade_in: Arc<dyn FiniteAnimationSpec<f32>> =
                                Arc::new(tween(100, 0, Arc::new(FastOutSlowInEasing)));
                            let placement: Arc<dyn FiniteAnimationSpec<IntOffset>> = Arc::new(SpringSpec::new(
                                Spring::DampingRatioNoBouncy,
                                Spring::StiffnessMediumLow,
                                None,
                            ));
                            let fade_out: Arc<dyn FiniteAnimationSpec<f32>> =
                                Arc::new(tween(200, 0, Arc::new(LinearEasing)));
                            BasicText(
                                format!("öğe {v} · {t}"),
                                modifier = Modifier
                                    .animate_item(
                                        fade_in_spec = Some(fade_in),
                                        placement_spec = Some(placement),
                                        fade_out_spec = Some(fade_out),
                                    )
                                    .fill_max_width()
                                    .height(50.dp())
                                    .clickable(move || cl.borrow_mut().push(v)),
                            );
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let drawn = |canvas: &RecordingCanvas, text: &str| canvas.texts.iter().any(|t| t == text);
    let (composed1, composed2) = (composed.borrow()[&1], composed.borrow()[&2]);
    items.remove_at(0);
    tick.set(1); // the list composes again; the leaving item doesn't (its data is gone)
    rule.advance_time_by(96);
    let mid = rule.draw();
    assert!(drawn(&mid, "öğe 1 · 0") && drawn(&mid, "öğe 2 · 1"), "{:?}", mid.texts);
    assert!(mid.layer_alphas.iter().any(|a| *a > 0.3 && *a < 0.7), "{:?}", mid.layer_alphas);
    assert!(composed.borrow()[&1] == composed1 && composed.borrow()[&2] > composed2);
    // Where it was: drawn in the first 50 px.
    let drawn_at = mid.texts.iter().position(|t| t == "öğe 1 · 0").unwrap();
    assert!(mid.baselines[drawn_at].y > 0.0 && mid.baselines[drawn_at].y < 50.0, "{:?}", mid.baselines[drawn_at]);
    // On it: it takes no input (no node of it is left).
    assert!(rule.on_all_nodes_with_text("öğe 1 · 0").fetch_semantics_nodes().is_empty());
    tap(&rule, at(200.0, 5.0));
    assert!(!clicked.borrow().contains(&1));
    rule.wait_for_idle();
    let after = rule.draw();
    assert!(!drawn(&after, "öğe 1 · 0") && after.layers == 0, "{:?}", after.texts);
    assert!(near(rule.on_node_with_text("öğe 2 · 1").fetch_semantics_node().unclipped_bounds_in_root().top, 0.0));
    items.remove_at(0); // 2 leaves, and comes back before it is gone: an item again
    rule.advance_time_by(48);
    items.add_at(0, 2);
    tick.set(2);
    rule.wait_for_idle();
    let back = rule.draw();
    assert!(drawn(&back, "öğe 2 · 2") && !drawn(&back, "öğe 2 · 1") && back.layers == 0, "{:?}", back.texts);
    let top = |text: &str| rule.on_node_with_text(text).fetch_semantics_node().unclipped_bounds_in_root().top;
    assert!(near(top("öğe 2 · 2"), 0.0) && near(top("öğe 3 · 2"), 50.0));
}

fn reverse_layout_puts_the_first_item_at_the_bottom() {
    // C++ 12736: reversed, item 0 at the bottom above the bottom padding; a finger pulling down brings the later
    // items down; at the far end the last item stops under the top padding.
    let state = LazyListState::new(0, 0);
    let count = mutable_state_of(3);
    let scope = new_scope();
    let (s, c, sc) = (state.clone(), count.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let (s, c) = (s.clone(), c.clone());
            Column(
                content = move || {
                    let n = c.get();
                    LazyColumn(
                        modifier = Modifier.size_wh(200.dp(), 400.dp()),
                        state = Some(s.clone()),
                        content_padding = PaddingValues(top = 10.dp(), bottom = 20.dp()),
                        reverse_layout = true,
                        content = move |this: &mut LazyListScope| {
                            this.items(n, |i| Row20(format!("i{i}"), 40.0));
                        },
                    );
                    LazyRow(
                        modifier = Modifier.size_wh(400.dp(), 50.dp()),
                        reverse_layout = true,
                        content = |this: &mut LazyListScope| {
                            this.items(3, |i| {
                                Box(modifier = Modifier.width(60.dp()).fill_max_height().test_tag(format!("r{i}")));
                            });
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let same = |r: Rect, x: f32, y: f32, w: f32, h: f32| {
        near(r.left, x) && near(r.top, y) && near(r.width(), w) && near(r.height(), h)
    };
    assert!(same(bounds(&rule, "i0"), 0.0, 340.0, 200.0, 40.0), "{:?}", bounds(&rule, "i0"));
    assert!(same(bounds(&rule, "i2"), 0.0, 260.0, 200.0, 40.0));
    assert!(same(bounds(&rule, "r0"), 340.0, 400.0, 60.0, 50.0) && same(bounds(&rule, "r1"), 280.0, 400.0, 60.0, 50.0));
    count.set(50);
    rule.wait_for_idle();
    assert!(same(bounds(&rule, "i0"), 0.0, 340.0, 200.0, 40.0) && state.first_visible_item_index() == 0);
    assert!(!state.can_scroll_backward() && state.can_scroll_forward());
    // A quick pull down: well past the first ones.
    swipe(&rule, at(100.0, 100.0), at(100.0, 260.0), 5);
    rule.wait_for_idle();
    let first = state.first_visible_item_index();
    assert!(first >= 4 && !has(&rule, "i0"), "{first}");
    // Item `first` is the one at the bottom, scrolled down past it by the offset.
    let bottom = bounds(&rule, &format!("i{first}"));
    assert!(near(bottom.top, 340.0 + state.first_visible_item_scroll_offset() as f32));
    // At the start, pulling up does nothing.
    let s = state.clone();
    launch(&scope, async move { s.scroll_to_item(0).await });
    rule.wait_for_idle();
    swipe(&rule, at(100.0, 300.0), at(100.0, 150.0), 10);
    rule.wait_for_idle();
    assert!(state.first_visible_item_index() == 0 && same(bounds(&rule, "i0"), 0.0, 340.0, 200.0, 40.0));
    // At the far end the last item stops under the top padding.
    let s = state.clone();
    launch(&scope, async move { s.scroll_to_item(49).await });
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "i49").top, 10.0) && !state.can_scroll_forward());
}

fn animate_scroll_to_item_glides_there() {
    // C++ 12779: a glide, not a jump; far: a jump to close by, then the glide; the last one stops where the list
    // ends; a finger stops it, and so does a newer one.
    let state = LazyListState::new(0, 0);
    let scope = new_scope();
    let (s, sc) = (state.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            LazyColumn(
                modifier = Modifier.size_wh(200.dp(), 400.dp()),
                state = Some(s.clone()),
                content = |this: &mut LazyListScope| {
                    // (estimates are off)
                    this.items(1000, |i| Row20(format!("i{i}"), if i % 3 == 0 { 60.0 } else { 40.0 }));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let go = |index: i32, offset: i32| {
        let s = state.clone();
        launch(&scope, async move { named_args!(s.animate_scroll_to_item(index, scroll_offset = offset)).await });
    };
    go(10, 0);
    for _ in 0..6 {
        rule.advance_frame();
    }
    let mid = state.first_visible_item_index();
    assert!(mid > 0 && mid < 10, "on its way, not a jump: {mid}");
    rule.wait_for_idle();
    assert!(state.first_visible_item_index() == 10 && state.first_visible_item_scroll_offset() == 0);
    // Far: a jump to close by (Compose's: a hundred items before it, after a first stretch of the glide), then
    // the glide; exactly there in the end.
    go(600, 15);
    for _ in 0..12 {
        rule.advance_frame();
    }
    let between = state.first_visible_item_index();
    assert!((500..600).contains(&between), "{between}");
    rule.wait_for_idle();
    assert!(state.first_visible_item_index() == 600 && state.first_visible_item_scroll_offset() == 15);
    go(0, 0);
    rule.wait_for_idle();
    assert!(state.first_visible_item_index() == 0 && state.first_visible_item_scroll_offset() == 0);
    // The last one can't be first: it stops where the list ends.
    go(999, 0);
    rule.wait_for_idle();
    let end = state.first_visible_item_index();
    assert!(!state.can_scroll_forward() && end < 999 && end > 980, "{end}");
    // A finger stops it, and so does a newer one.
    go(500, 0);
    for _ in 0..6 {
        rule.advance_frame();
    }
    let before = state.first_visible_item_index();
    assert!(before > 500, "{before}");
    swipe(&rule, at(100.0, 220.0), at(100.0, 160.0), 4); // a finger pushes it on
    rule.wait_for_idle();
    assert!(state.first_visible_item_index() > before);
    go(0, 0);
    rule.advance_frame();
    rule.advance_frame();
    go(100, 0);
    rule.wait_for_idle();
    assert_eq!(state.first_visible_item_index(), 100);
    // 40 + 40 + 60 - 10: into item 97.
    let s = state.clone();
    launch(&scope, async move {
        s.animate_scroll_by(-130.0).await;
    });
    rule.wait_for_idle();
    assert!(state.first_visible_item_index() == 97 && state.first_visible_item_scroll_offset() == 10);
}

fn animate_scroll_to_item_finds_items_bigger_than_guessed() {
    // C++ 12827: items 117..129 ten times taller than those around 100: one glide heading further as they turn out
    // taller; to the end it is over once the list can't go further.
    let state = LazyListState::new(0, 0);
    let scope = new_scope();
    let (s, sc) = (state.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            LazyColumn(
                modifier = Modifier.size_wh(200.dp(), 400.dp()),
                state = Some(s.clone()),
                content = |this: &mut LazyListScope| {
                    this.items(200, |i| Row20(format!("i{i}"), if (117..130).contains(&i) { 400.0 } else { 40.0 }));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let s = state.clone();
    launch(&scope, async move { s.scroll_to_item(100).await });
    rule.wait_for_idle();
    let arrived = Rc::new(Cell::new(false));
    let (s, a) = (state.clone(), arrived.clone());
    launch(&scope, async move {
        s.animate_scroll_to_item(130).await;
        a.set(true);
    });
    let mut last = 100;
    let mut forward = true;
    let mut t = 0;
    // (Compose's springs get there in about 0.9 s here; C++'s glide took under 0.7)
    while t < 3000 && !arrived.get() {
        rule.advance_frame();
        forward = forward && state.first_visible_item_index() >= last;
        last = state.first_visible_item_index();
        t += 16;
    }
    assert!(
        arrived.get() && forward,
        "{} {forward} {state:?} {:?}",
        arrived.get(),
        state.layout_info().visible_items_info().iter().map(|i| (i.index(), i.offset(), i.size())).collect::<Vec<_>>()
    );
    assert!(state.first_visible_item_index() == 130 && state.first_visible_item_scroll_offset() == 0);
    // To the end: over once it can't go further.
    arrived.set(false);
    let (s, a) = (state.clone(), arrived.clone());
    launch(&scope, async move {
        s.animate_scroll_to_item(199).await;
        a.set(true);
    });
    rule.advance_time_by(700);
    assert!(arrived.get() && !state.can_scroll_forward());
}

fn lazy_lists_arrange_items_that_all_fit() {
    // C++ 13052: items that all fit are arranged (bottom, centered with spacing, spread out; reversed at the top
    // with item 0 lowest); more than fit scroll as usual from the top.
    let many = mutable_state_of(false);
    let bottom = LazyListState::new(0, 0);
    let scope = new_scope();
    let (m, b, sc) = (many.clone(), bottom.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let n = if m.get() { 11 } else { 3 };
            let b = b.clone();
            Row(content = move || {
                let list = |name: &'static str,
                            arrangement: Option<VerticalArrangement>,
                            reverse: bool,
                            state: Option<LazyListState>| {
                    kompozable_block!({
                        LazyColumn(
                            modifier = Modifier.size_wh(100.dp(), 400.dp()),
                            reverse_layout = reverse,
                            vertical_arrangement = arrangement,
                            state = state,
                            content = move |this: &mut LazyListScope| {
                                this.items(n, move |i| Row20(format!("{name}{i}"), 40.0));
                            },
                        );
                    })
                };
                list("alt", Some(Arrangement::Bottom), false, Some(b.clone()));
                list("orta", Some(Arrangement::spaced_by_aligned(10.dp(), Alignment::CenterVertically)), false, None);
                list("yayık", Some(Arrangement::SpaceBetween), false, None);
                list("ters", Some(Arrangement::Top), true, None);
                list("düz", None, false, None);
            });
            LazyRow(
                modifier = Modifier.size_wh(400.dp(), 40.dp()),
                horizontal_arrangement = Some(Arrangement::End),
                content = |this: &mut LazyListScope| {
                    this.items(2, |i| {
                        Box(modifier = Modifier.size_wh(50.dp(), 40.dp()).test_tag(format!("satır{i}")));
                    });
                },
            );
        }),
        IntSize::new(600, 800),
    );
    let y = |tag: &str| bounds(&rule, tag).top;
    assert!(near(y("alt0"), 280.0) && near(y("alt2"), 360.0), "{} {}", y("alt0"), y("alt2"));
    assert!(near(y("orta0"), 130.0) && near(y("orta1"), 180.0) && near(y("orta2"), 230.0));
    assert!(near(y("yayık0"), 0.0) && near(y("yayık1"), 180.0) && near(y("yayık2"), 360.0));
    assert!(near(y("ters2"), 0.0) && near(y("ters0"), 80.0) && near(y("düz0"), 0.0));
    assert!(near(bounds(&rule, "satır0").left, 300.0) && near(bounds(&rule, "satır1").left, 350.0));
    // More than fit (440 in 400): from the top, the spacing still between them.
    many.set(true);
    rule.wait_for_idle();
    assert!(near(y("alt0"), 0.0) && near(y("orta0"), 0.0) && near(y("orta1"), 50.0) && near(y("yayık1"), 40.0));
    scroll_by(&rule, &scope, &bottom, 20.0);
    assert!(near(y("alt0"), -20.0));
}

fn rtl_snapping_row_settles_on_with_what_an_inner_fling_left() {
    // C++ 14698: right to left, at its end: the inner row flings into its end and the list, which can't go on
    // either, settles towards its end with the rest (it stays there).
    let list = LazyListState::new(0, 0);
    let inner = ScrollState::new(0);
    let scope = new_scope();
    let (l, i, sc) = (list.clone(), inner.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let (l, i) = (l.clone(), i.clone());
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], move || {
                let fling = named_args!(remember_snap_fling_behavior_for_list(&l, snap_position = SnapPosition::Start));
                let i = i.clone();
                LazyRow(
                    modifier = Modifier.width(200.dp()).height(60.dp()),
                    state = Some(l.clone()),
                    fling_behavior = Some(fling as Rc<dyn FlingBehavior>),
                    content = move |this: &mut LazyListScope| {
                        let i = i.clone();
                        this.items(4, move |index| {
                            if index == 3 {
                                Row(
                                    modifier = Modifier.size_wh(100.dp(), 60.dp()).horizontal_scroll(&i).test_tag("iç"),
                                    content = || Box(modifier = Modifier.size_wh(200.dp(), 60.dp())),
                                );
                            } else {
                                Box(modifier = Modifier.size_wh(100.dp(), 60.dp()));
                            }
                        });
                    },
                );
            });
        }),
        IntSize::new(200, 400),
    );
    let s = list.clone();
    launch(&scope, async move { s.scroll_to_item(3).await });
    rule.wait_for_idle();
    let r = bounds(&rule, "iç");
    swipe(&rule, at(r.left + 10.0, r.top + 30.0), at(r.left + 90.0, r.top + 30.0), 4);
    rule.wait_for_idle();
    assert!(
        inner.value() == inner.max_value() && list.first_visible_item_index() == 2,
        "{} {} {}",
        inner.value(),
        inner.max_value(),
        list.first_visible_item_index()
    );
}

fn sticky_headers_stay_at_the_top_until_the_next_one_pushes() {
    // C++ 21837: a header stays at the top while its section scrolls under it, until the next one pushes it up;
    // deep in a section its header is composed although far above; the pinned header is on top of the items.
    let list = LazyListState::new(0, 0);
    let taps = Rc::new(RefCell::new(Vec::<i32>::new()));
    let scope = new_scope();
    let (l, t, sc) = (list.clone(), taps.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let t = t.clone();
            LazyColumn(
                modifier = Modifier.fill_max_width().height(400.dp()),
                state = Some(l.clone()),
                content = move |this: &mut LazyListScope| {
                    // (40 + 10 × 50 = 540 each)
                    for section in 0..6 {
                        let t = t.clone();
                        this.sticky_header(
                            key = Some(LazyKey::new(section)),
                            content = move |_| {
                                let t = t.clone();
                                Box(modifier = Modifier
                                    .fill_max_width()
                                    .height(40.dp())
                                    .clickable(move || t.borrow_mut().push(section))
                                    .test_tag(format!("h{section}")));
                            },
                        );
                        this.items(10, move |i| Row20(format!("{section}.{i}"), 50.0));
                    }
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let top = |header: i32| bounds(&rule, &format!("h{header}")).top;
    assert!(near(top(0), 0.0));
    let s = list.clone();
    launch(&scope, async move { s.scroll_to_item(3).await }); // inside section 0: its header stays
    rule.wait_for_idle();
    assert!(near(top(0), 0.0));
    scroll_by(&rule, &scope, &list, 380.0); // section 1's header comes to 20: it pushes section 0's up
    assert!(near(top(1), 20.0) && near(top(0), -20.0), "{} {}", top(1), top(0));
    scroll_by(&rule, &scope, &list, 30.0); // past the top: section 1's takes over
    assert!(near(top(1), 0.0));
    let s = list.clone();
    launch(&scope, async move { s.scroll_to_item(4 * 11 + 8).await }); // deep in section 4
    rule.wait_for_idle();
    assert!(has(&rule, "h4") && near(top(4), 0.0));
    tap(&rule, at(200.0, 20.0)); // the pinned header is on top of the items under it
    assert_eq!(*taps.borrow(), vec![4]);
}

fn lazy_list_layout_info_tells_what_is_in_view() {
    // C++ 23886: what the last layout showed: the items in view (index, key, content type, offset, size), the
    // viewport, the padding and spacing; read in composition it recomposes as it changes (nothing moved: nothing
    // told); a pinned header comes first.
    let (state, headed) = (LazyListState::new(0, 0), LazyListState::new(0, 0));
    let compositions = Rc::new(Cell::new(0));
    let scope = new_scope();
    let (s, h, c, sc) = (state.clone(), headed.clone(), compositions.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let (s, h, c) = (s.clone(), h.clone(), c.clone());
            Column(
                content = move || {
                    let info = s.layout_info();
                    c.set(c.get() + 1);
                    let last = info.visible_items_info().last().map_or(-1, |i| i.index());
                    BasicText(format!("son {last} / {}", info.total_items_count()));
                    LazyColumn(
                        modifier = Modifier.fill_max_width().height(200.dp()),
                        state = Some(s.clone()),
                        content_padding = PaddingValues(top = 10.dp(), bottom = 10.dp()),
                        vertical_arrangement = Some(Arrangement::spaced_by(4.dp())),
                        content = |this: &mut LazyListScope| {
                            this.item(
                                key = Some(LazyKey::new("başlık")),
                                content_type = Some(SlotKey::new("header")),
                                content = || Box(modifier = Modifier.height(40.dp())),
                            );
                            this.items(
                                30,
                                key = index_key(|i| i),
                                content_type =
                                    Some(Rc::new(|_| Some(SlotKey::new(7))) as Rc<dyn Fn(i32) -> Option<SlotKey>>),
                                item_content = |_| Box(modifier = Modifier.height(36.dp())),
                            );
                        },
                    );
                    LazyColumn(
                        modifier = Modifier.fill_max_width().height(100.dp()),
                        state = Some(h.clone()),
                        content = |this: &mut LazyListScope| {
                            this.sticky_header(|_| Box(modifier = Modifier.height(20.dp())));
                            this.items(20, |_| Box(modifier = Modifier.height(30.dp())));
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let info = state.layout_info();
    assert!(
        info.total_items_count() == 31
            && info.viewport_start_offset() == -10
            && info.viewport_end_offset() == 190
            && info.before_content_padding() == 10
            && info.after_content_padding() == 10
            && info.main_axis_item_spacing() == 4
            && info.orientation() == Orientation::Vertical
            && info.viewport_size().height == 200,
        "{info:?}"
    );
    // (164..200 shows in the bottom padding)
    let v = info.visible_items_info();
    assert_eq!(v.len(), 5, "{v:?}");
    rule.on_node_with_text("son 4 / 31").assert_exists();
    assert!(
        v[0].index() == 0
            && v[0].offset() == 0
            && v[0].size() == 40
            && v[0].content_type() == Some(SlotKey::new("header"))
    );
    assert!(
        v[1].index() == 1 && v[1].offset() == 44 && v[1].size() == 36 && v[1].content_type() == Some(SlotKey::new(7))
    );
    assert!(v[4].offset() == 164 && v[1].key() != v[2].key() && v[0].key() == LazyKey::new("başlık"));
    let s = state.clone();
    launch(&scope, async move { named_args!(s.scroll_to_item(10, scroll_offset = 6)).await });
    rule.wait_for_idle();
    let info = state.layout_info();
    // Partly scrolled out at the top: a negative offset. (Item 9, composed for the padding before the content, ends
    // where the viewport starts: Compose tells it too; C++ began with item 10.)
    let v = info.visible_items_info();
    assert!(state.first_visible_item_index() == 10 && state.first_visible_item_scroll_offset() == 6);
    assert!(
        v[0].index() == 9 && v[0].offset() == -46 && v[1].index() == 10 && v[1].offset() == -6,
        "{:?}",
        v.iter().map(|i| (i.index(), i.offset(), i.size())).collect::<Vec<_>>()
    );
    rule.on_node_with_text("son 14 / 31").assert_exists();
    let before = compositions.get();
    rule.wait_for_idle(); // nothing moved: nothing told
    assert_eq!(compositions.get(), before);
    let s = headed.clone();
    launch(&scope, async move { s.scroll_to_item(5).await }); // the pinned header first, at the top
    rule.wait_for_idle();
    let h = headed.layout_info().visible_items_info();
    assert!(h.len() >= 2 && h[0].index() == 0 && h[0].offset() == 0 && h[1].index() == 5, "{h:?}");
}

fn match_parent_size_and_fill_parent_max() {
    // C++ 26421's lazy part: an item a fraction of the list's viewport past its padding; outside a list, a fraction
    // of its constraints (as fill_max_width).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                LazyColumn(
                    modifier = Modifier.size_wh(200.dp(), 300.dp()),
                    content_padding = PaddingValues(top = 20.dp(), bottom = 20.dp()),
                    content = |this: &mut LazyListScope| {
                        this.item(|| {
                            Box(modifier =
                                Modifier.fill_parent_max_height(0.5).fill_parent_max_width(0.5).test_tag("yarı"))
                        });
                        this.item(|| Box(modifier = Modifier.fill_parent_max_size().test_tag("tam")));
                    },
                );
                Box(
                    modifier = Modifier.size(200.dp()),
                    content =
                        || Box(modifier = Modifier.fill_parent_max_width(0.5).height(10.dp()).test_tag("dışarıda")),
                );
            },
        );
    }));
    let (half, full) = (bounds(&rule, "yarı"), bounds(&rule, "tam"));
    assert!(near(half.width(), 100.0) && near(half.height(), 130.0), "{half:?}");
    assert!(near(full.width(), 200.0) && near(full.height(), 260.0), "{full:?}");
    assert!(near(bounds(&rule, "dışarıda").width(), 100.0));
}

fn snap_fling_behavior_settles_on_an_item() {
    // C++ 26967: dragged 130 and held still, the nearest item lines up with the start; flung, on by several items,
    // still lined up; back the other way; centered: an item's middle settles in the list's middle.
    let (start, center) = (LazyListState::new(0, 0), LazyListState::new(0, 0));
    let (st, ce) = (start.clone(), center.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (st, ce) = (st.clone(), ce.clone());
        Column(
            content = move || {
                let start_fling =
                    named_args!(remember_snap_fling_behavior_for_list(&st, snap_position = SnapPosition::Start));
                LazyRow(
                    modifier = Modifier.width(300.dp()).height(60.dp()).test_tag("baş"),
                    state = Some(st.clone()),
                    horizontal_arrangement = Some(Arrangement::spaced_by(10.dp())),
                    fling_behavior = Some(start_fling as Rc<dyn FlingBehavior>),
                    content = |this: &mut LazyListScope| {
                        this.items(30, |i| {
                            Box(modifier = Modifier.size_wh(100.dp(), 60.dp()).test_tag(format!("b{i}")))
                        });
                    },
                );
                let center_fling = remember_snap_fling_behavior_for_list(&ce);
                LazyRow(
                    modifier = Modifier.width(300.dp()).height(60.dp()),
                    state = Some(ce.clone()),
                    horizontal_arrangement = Some(Arrangement::spaced_by(10.dp())),
                    fling_behavior = Some(center_fling as Rc<dyn FlingBehavior>),
                    content = |this: &mut LazyListScope| {
                        this.items(30, |i| {
                            Box(modifier = Modifier.size_wh(100.dp(), 60.dp()).test_tag(format!("o{i}")))
                        });
                    },
                );
            },
        );
    }));
    let near_half = |a: f32, b: f32| (a - b).abs() <= 0.5;
    slow_drag(&rule, at(250.0, 30.0), at(120.0, 30.0), 10);
    rule.advance_time_by(200);
    rule.wait_for_idle();
    assert!(
        start.first_visible_item_index() == 1 && near_half(start.first_visible_item_scroll_offset() as f32, 0.0),
        "{:?}",
        start
    );
    swipe(&rule, at(280.0, 30.0), at(180.0, 30.0), 3); // flung: on by several items, still lined up
    rule.wait_for_idle();
    let far = start.first_visible_item_index();
    assert!(far >= 4 && start.first_visible_item_scroll_offset() == 0, "{start:?}");
    swipe(&rule, at(180.0, 30.0), at(280.0, 30.0), 3); // back the other way
    rule.wait_for_idle();
    assert!(start.first_visible_item_index() < far && start.first_visible_item_scroll_offset() == 0, "{start:?}");
    // Centered: an item's middle in the list's middle (150).
    slow_drag(&rule, at(250.0, 90.0), at(100.0, 90.0), 10);
    rule.advance_time_by(200);
    rule.wait_for_idle();
    let centered = (0..30).any(|i| {
        let n = rule.on_node_with_tag(&format!("o{i}"));
        n.exists() && near_half(n.fetch_semantics_node().unclipped_bounds_in_root().left + 50.0, 150.0)
    });
    assert!(centered, "{center:?}");
}

fn a_hair_short_of_an_item_counts_as_there() {
    // C++ 27001: a float error short of item 1: it is first; a hair back stays there.
    let state = LazyListState::new(0, 0);
    let scope = new_scope();
    let (s, sc) = (state.clone(), scope.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *sc.borrow_mut() = Some(remember_coroutine_scope());
        LazyRow(
            modifier = Modifier.width(300.dp()).height(40.dp()),
            state = Some(s.clone()),
            horizontal_arrangement = Some(Arrangement::spaced_by(10.dp())),
            content = |this: &mut LazyListScope| {
                this.items(20, |_| Box(modifier = Modifier.size_wh(100.dp(), 40.dp())));
            },
        );
    }));
    scroll_by(&rule, &scope, &state, 109.99999);
    assert!(state.first_visible_item_index() == 1 && state.first_visible_item_scroll_offset() == 0, "{state:?}");
    scroll_by(&rule, &scope, &state, -0.000005);
    assert!(state.first_visible_item_index() == 1 && state.first_visible_item_scroll_offset() == 0, "{state:?}");
    scroll_by(&rule, &scope, &state, 5.0);
    assert!(state.first_visible_item_index() == 1 && state.first_visible_item_scroll_offset() == 5, "{state:?}");
}

/// The stretched layers drawn now: each one's bounds and its stretch's direction.
fn stretches(rule: &KompozTestRule) -> Vec<(Rect, Offset)> {
    let canvas = rule.draw();
    canvas
        .layer_stretches
        .iter()
        .zip(&canvas.layer_bounds)
        .filter_map(|(s, b)| s.filter(|s| !s.is_empty()).map(|s| (*b, s.stretch_direction())))
        .collect()
}

/// How far the node tagged `tag` is stretched now (C++'s `stretchOf`): the stretched layer drawn over it.
fn stretch_of(rule: &KompozTestRule, tag: &str) -> Offset {
    let center = bounds(rule, tag).center();
    stretches(rule).into_iter().find(|(b, _)| b.contains(center)).map_or(Offset::Zero, |(_, d)| d)
}

/// Down at `from`, moved by `by` over 10 frames, held; how far `tag` stretched; let go, until idle.
fn pull(rule: &KompozTestRule, tag: &str, from: Offset, by: Offset) -> Offset {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=10 {
        rule.advance_frame();
        h.pointer_move(from + by * (i as f32 / 10.0), -1, 0);
    }
    let stretched = stretch_of(rule, tag);
    h.pointer_up(from + by, -1, 0);
    rule.wait_for_idle();
    stretched
}

fn local_overscroll_factory_turns_the_stretch_off() {
    // C++ 3885's LazyColumn part: under no factory a lazy list doesn't stretch either; by default it does.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Row(content = || {
                LazyColumn(
                    modifier = Modifier.weight(1.0).fill_max_height().test_tag("açık"),
                    content = |this: &mut LazyListScope| {
                        this.items(30, |_| Box(modifier = Modifier.fill_max_width().height(50.dp())));
                    },
                );
                let none: Option<Rc<dyn OverscrollFactory>> = None;
                KompozitionLocalProvider(&[LocalOverscrollFactory.provides(none)], || {
                    LazyColumn(
                        modifier = Modifier.weight(1.0).fill_max_height().test_tag("tembel"),
                        content = |this: &mut LazyListScope| {
                            this.items(30, |_| Box(modifier = Modifier.fill_max_width().height(50.dp())));
                        },
                    );
                });
            });
        }),
        IntSize::new(600, 800),
    );
    let x = |tag: &str| bounds(&rule, tag).center().x;
    assert!(pull(&rule, "açık", at(x("açık"), 300.0), at(0.0, 200.0)).y > 0.0);
    assert_eq!(pull(&rule, "tembel", at(x("tembel"), 300.0), at(0.0, 200.0)), Offset::Zero);
}

fn overscroll_effect_none_shared_or_halved() {
    // C++ 4303's LazyColumn part: given no effect (`Some(None)`), nothing stretches.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            LazyColumn(
                modifier = Modifier.fill_max_width().height(600.dp()).test_tag("tembel"),
                overscroll_effect = Some(None),
                content = |this: &mut LazyListScope| {
                    this.items(30, |_| Box(modifier = Modifier.fill_max_width().height(50.dp())));
                },
            );
        }),
        IntSize::new(800, 800),
    );
    let center = bounds(&rule, "tembel").center();
    assert_eq!(pull(&rule, "tembel", center, at(0.0, 200.0)), Offset::Zero);
    assert!(stretches(&rule).is_empty());
}

/// A spec putting what it brings in `inset` px from the start.
#[derive(PartialEq)]
struct InsetSpec(f32);

impl BringIntoViewSpec for InsetSpec {
    fn calculate_scroll_distance(&self, offset: f32, _size: f32, _container_size: f32) -> f32 {
        offset - self.0
    }
}

fn bring_into_view_spec_puts_the_focused_item_where_it_says() {
    // C++ 25046's LazyColumn and LazyRow parts: under a LocalBringIntoViewSpec, a focused item goes where it says
    // (back too; as far as the list goes); along a row, right to left too.
    let items: Vec<FocusRequester> = (0..30).map(|_| FocusRequester::new()).collect();
    let row: Vec<FocusRequester> = (0..20).map(|_| FocusRequester::new()).collect();
    let rtl = mutable_state_of(false);
    let (it, rw, rt) = (items.clone(), row.clone(), rtl.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (it, rw, rt) = (it.clone(), rw.clone(), rt.clone());
            Row(
                modifier = Modifier.fill_max_size(),
                content = move || {
                    let it = it.clone();
                    let at100: Rc<dyn BringIntoViewSpec> = Rc::new(InsetSpec(100.0));
                    KompozitionLocalProvider(&[LocalBringIntoViewSpec.provides(at100)], move || {
                        let it = it.clone();
                        LazyColumn(
                            modifier = Modifier.width(100.dp()).fill_max_height().test_tag("liste"),
                            content = move |this: &mut LazyListScope| {
                                let it = it.clone();
                                this.items(30, move |i| {
                                    Box(modifier = Modifier
                                        .fill_max_width()
                                        .height(50.dp())
                                        .focus_requester(&it[i as usize])
                                        .focusable()
                                        .test_tag(format!("ö{i}")));
                                });
                            },
                        );
                    });
                    Box(modifier = Modifier.width(200.dp()));
                    let direction = if rt.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
                    let at30: Rc<dyn BringIntoViewSpec> = Rc::new(InsetSpec(30.0));
                    let rw = rw.clone();
                    KompozitionLocalProvider(
                        &[LocalLayoutDirection.provides(direction), LocalBringIntoViewSpec.provides(at30)],
                        move || {
                            let rw = rw.clone();
                            LazyRow(
                                modifier = Modifier.width(200.dp()).height(50.dp()),
                                content = move |this: &mut LazyListScope| {
                                    let rw = rw.clone();
                                    this.items(20, move |i| {
                                        Box(modifier = Modifier
                                            .width(50.dp())
                                            .fill_max_height()
                                            .focus_requester(&rw[i as usize])
                                            .focusable()
                                            .test_tag(format!("r{i}")));
                                    });
                                },
                            );
                        },
                    );
                },
            );
        }),
        IntSize::new(500, 500),
    );
    let top = |tag: &str| bounds(&rule, tag).top;
    // (only items composed can take the focus: those in view, as Compose composes; C++ composed half a view more)
    items[8].request_focus(); // forwards: 100 in
    rule.wait_for_idle();
    assert!(near(top("ö8"), 100.0), "{}", top("ö8"));
    items[12].request_focus(); // further
    rule.wait_for_idle();
    assert!(near(top("ö12"), 100.0), "{}", top("ö12"));
    items[10].request_focus(); // back: 100 in too
    rule.wait_for_idle();
    assert!(near(top("ö10"), 100.0), "{}", top("ö10"));
    for i in [8, 6, 4, 2] {
        items[i].request_focus();
        rule.wait_for_idle();
        assert!(near(top(&format!("ö{i}")), 100.0), "{i}: {}", top(&format!("ö{i}")));
    }
    items[0].request_focus(); // as far as the list goes
    rule.wait_for_idle();
    assert!(near(top("ö0"), 0.0));
    // Along a row (it starts at x = 300).
    row[3].request_focus();
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "r3").left, 300.0 + 30.0), "{:?}", bounds(&rule, "r3"));
    // Right to left the spec's offset is still from the row's left (Compose's ContentInViewNode gives it the
    // child's left edge; C++ measured from the right): r5, 20 from the left, goes to 30.
    rtl.set(true);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "r5").left, 320.0), "{:?}", bounds(&rule, "r5"));
    row[5].request_focus();
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "r5").left, 300.0 + 30.0), "{:?}", bounds(&rule, "r5"));
}

fn what_shows_past_a_lazy_row_across_counts_as_shown() {
    // C++ 26208's LazyRow parts: a node hanging 10 under a row, within its 30 (MaxSupportedElevation), is still
    // clicked there; a box cutting off a row 10 under it draws the row (its 30 across reach into the box).
    let clicks = Rc::new(Cell::new(0));
    let c = clicks.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let c = c.clone();
            Column(
                modifier = Modifier.padding(40.dp()),
                content = move || {
                    Box(modifier = Modifier.size_wh(200.dp(), 100.dp()));
                    let c = c.clone();
                    LazyRow(
                        modifier = Modifier.size_wh(200.dp(), 100.dp()),
                        content = move |this: &mut LazyListScope| {
                            let c = c.clone();
                            this.item(move || {
                                let c = c.clone();
                                Box(
                                    modifier = Modifier.size_wh(80.dp(), 100.dp()),
                                    content = move || {
                                        let c = c.clone();
                                        Box(modifier = Modifier
                                            .offset(0.dp(), 100.dp())
                                            .size_wh(80.dp(), 20.dp())
                                            .clickable(move || c.set(c.get() + 1)));
                                    },
                                );
                            });
                        },
                    );
                    Spacer(modifier = Modifier.height(40.dp()));
                    Box(
                        modifier = Modifier.size_wh(200.dp(), 100.dp()).clip_to_bounds(),
                        content = || {
                            LazyRow(
                                modifier = Modifier.offset(0.dp(), 110.dp()).size_wh(200.dp(), 100.dp()),
                                content = |this: &mut LazyListScope| {
                                    this.items(3, |_| Box(modifier = Modifier.size_wh(80.dp(), 100.dp())));
                                },
                            );
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    // (the row 140..240, the part 240..260)
    rule.host().pointer_down(at(80.0, 245.0), -1, 0, PointerType::Mouse);
    rule.advance_frame();
    rule.host().pointer_up(at(80.0, 245.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 1);
    let canvas = rule.draw();
    // (the inner row at 390: its clip reaches 30 up and down across)
    let reach = Rect::new(40.0, 390.0 - 30.0, 240.0, 390.0 - 30.0 + 160.0);
    assert!(
        canvas.clips.iter().any(|r| near(r.left, reach.left)
            && near(r.top, reach.top)
            && near(r.right, reach.right)
            && near(r.bottom, reach.bottom)),
        "{:?}",
        canvas.clips
    );
}

fn scroll_states_tell_whether_they_scroll() {
    // C++ 12899's lazy list part: flung, it scrolls until the fling ends; an animated scroll to an item scrolls
    // until a touch stops it; an animated scroll by scrolls while it runs.
    let list = LazyListState::new(0, 0);
    let scope = new_scope();
    let (l, sc) = (list.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let l = l.clone();
            Column(
                content = move || {
                    BasicText(format!("liste{}", if l.is_scroll_in_progress() { " kayıyor" } else { "" }));
                    LazyColumn(
                        modifier = Modifier.size_wh(200.dp(), 200.dp()).test_tag("liste"),
                        state = Some(l.clone()),
                        content = |this: &mut LazyListScope| {
                            this.items(100, |i| Row20(format!("{i}"), 40.0));
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 1200),
    );
    rule.main_clock().set_auto_advance(false);
    let list_top = bounds(&rule, "liste").top;
    swipe(&rule, at(100.0, list_top + 180.0), at(100.0, list_top + 40.0), 5);
    rule.advance_frame();
    assert!(list.is_scroll_in_progress());
    rule.on_node_with_text("liste kayıyor").assert_exists();
    settle(&rule);
    assert!(!list.is_scroll_in_progress() && list.first_visible_item_index() > 3);
    let s = list.clone();
    launch(&scope, async move { s.animate_scroll_to_item(0).await });
    rule.advance_frame();
    rule.advance_frame();
    assert!(list.is_scroll_in_progress());
    rule.host().pointer_down(at(100.0, list_top + 100.0), -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(at(100.0, list_top + 100.0), -1, 0);
    settle(&rule);
    assert!(!list.is_scroll_in_progress());
    let s = list.clone();
    launch(&scope, async move {
        s.animate_scroll_by(120.0).await;
    });
    rule.advance_frame();
    rule.advance_frame();
    assert!(list.is_scroll_in_progress());
    settle(&rule);
    assert!(!list.is_scroll_in_progress());
}

fn accessibility_scrolls_to_an_index_and_a_key() {
    // Compose's lazy layout semantics: a collection of its items' count, the index of a key, scrolling to an index;
    // kompoz-test's perform_scroll_to_index/key/node through them.
    let state = LazyListState::new(0, 0);
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            LazyColumn(
                modifier = Modifier.fill_max_size().test_tag("liste"),
                state = Some(s.clone()),
                content = |this: &mut LazyListScope| {
                    this.items_list(
                        (0..100).map(|i| i * 10).collect::<Vec<i32>>(),
                        key = item_key(|v: &i32| *v),
                        item_content = |v: &i32| {
                            Row20(format!("row {v}"), 50.0);
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 400),
    );
    let node = rule.on_node_with_tag("liste").fetch_semantics_node();
    assert_eq!(
        node.config().get(kompoz_ui::semantics::SemanticsProperties::CollectionInfo),
        Some(kompoz_ui::semantics::CollectionInfo { row_count: 100, column_count: 1 })
    );
    rule.on_node_with_tag("liste").perform_scroll_to_index(40);
    assert_eq!(state.first_visible_item_index(), 40);
    rule.on_node_with_tag("liste").perform_scroll_to_key(&700i32);
    assert_eq!(state.first_visible_item_index(), 70);
    rule.on_node_with_tag("liste").perform_scroll_to_key(&LazyKey::new(100i32));
    assert_eq!(state.first_visible_item_index(), 10);
    rule.on_node_with_tag("liste").perform_scroll_to_node(has_test_tag("row 950"));
    assert!(has(&rule, "row 950"));
}

fn a_pinned_item_stays_composed_out_of_view() {
    // Compose's LazyLayoutPinnableItem: an item its content pins stays composed (and placed) while it is
    // scrolled out of view, until the pin is released.
    let handle: Rc<RefCell<Option<kompoz_ui::layout::PinnedHandle>>> = Rc::new(RefCell::new(None));
    let state = LazyListState::new(0, 0);
    let scope = new_scope();
    let (h, s, sc) = (handle.clone(), state.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let h = h.clone();
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                state = Some(s.clone()),
                content = move |this: &mut LazyListScope| {
                    let h = h.clone();
                    this.items(100, move |i| {
                        if i == 2 {
                            let container = kompoz_ui::layout::LocalPinnableContainer.current();
                            let h = h.clone();
                            remember(move || {
                                *h.borrow_mut() = container.map(|c| c.pin());
                            });
                        }
                        Row20(format!("row {i}"), 50.0);
                    });
                },
            );
        }),
        IntSize::new(400, 400),
    );
    assert!(handle.borrow().is_some());
    let s = state.clone();
    launch(&scope, async move { s.scroll_to_item(50).await });
    rule.wait_for_idle();
    assert!(has(&rule, "row 2") && !has(&rule, "row 3"));
    handle.borrow_mut().take().unwrap().release();
    rule.wait_for_idle();
    assert!(!has(&rule, "row 2"));
}

fn focus_moves_beyond_the_bounds() {
    // Compose's beyond-bounds layout: focus moving down past the last item in view finds the next item, laid out
    // beyond the bounds for the search, and brings it into view.
    let first = FocusRequester::new();
    let f = first.clone();
    let state = LazyListState::new(0, 0);
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let f = f.clone();
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                state = Some(s.clone()),
                content = move |this: &mut LazyListScope| {
                    let f = f.clone();
                    this.items(50, move |i| {
                        let m = Modifier.fill_max_width().height(100.dp()).test_tag(format!("row {i}"));
                        let m = if i == 0 { m.focus_requester(&f) } else { m };
                        Box(modifier = m.focusable());
                    });
                },
            );
        }),
        IntSize::new(400, 300),
    );
    first.request_focus();
    rule.wait_for_idle();
    for _ in 0..4 {
        rule.host().focus.move_focus(kompoz_ui::focus::FocusDirection::Down);
        rule.wait_for_idle();
    }
    // (row 4 focused: brought into view)
    assert!(
        rule.on_node_with_tag("row 4")
            .fetch_semantics_node()
            .config()
            .get(kompoz_ui::semantics::SemanticsProperties::Focused)
            == Some(true)
    );
    assert!(state.first_visible_item_index() >= 2, "{state:?}");
    let b = bounds(&rule, "row 4");
    assert!(b.top >= 0.0 && b.bottom <= 300.0 + 0.5, "{b:?}");
}

fn a_cache_window_keeps_items_composed_ahead() {
    // Compose's cache window: with 300 ahead, scrolling forward prefetches (between frames) as many items as fill
    // 300 past the viewport.
    let state = LazyListState::with_cache_window(named_args!(lazy_layout_cache_window(ahead = 300.dp())), 0, 0);
    let scope = new_scope();
    let (s, sc) = (state.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                state = Some(s.clone()),
                content = |this: &mut LazyListScope| {
                    this.items(100, |i| Row20(format!("row {i}"), 100.0));
                },
            );
        }),
        IntSize::new(400, 400),
    );
    assert!(has(&rule, "row 3") && !has(&rule, "row 4"));
    scroll_by(&rule, &scope, &state, 10.0);
    // (the first frame's window; then one item at a time as each finishes)
    for _ in 0..5 {
        rule.host().idle(1_000_000_000);
        rule.wait_for_idle();
    }
    assert!(has(&rule, "row 6") && !has(&rule, "row 9"), "{} {}", has(&rule, "row 6"), has(&rule, "row 9"));
}

/// A list remembering its state, with items keeping a saveable count (the same content for both hosts).
fn saved_list(
    shown: &MutableState<bool>,
    state: &Rc<RefCell<Option<LazyListState>>>,
    counts: &Rc<RefCell<Vec<MutableState<i32>>>>,
) -> impl Content + 'static {
    let (sh, st, co) = (shown.clone(), state.clone(), counts.clone());
    kompozable_lambda!(move || {
        if sh.get() {
            let list = remember_lazy_list_state();
            *st.borrow_mut() = Some(list.clone());
            let co = co.clone();
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                state = Some(list),
                content = move |this: &mut LazyListScope| {
                    let co = co.clone();
                    this.items(
                        100,
                        key = index_key(|i| i),
                        item_content = move |i| {
                            let count = remember_saveable(|| mutable_state_of(0));
                            if i == 31 {
                                co.borrow_mut().push(count.clone());
                            }
                            BasicText(format!("satır {i}: {}", count.get()), modifier = Modifier.height(50.dp()));
                        },
                    );
                },
            );
        }
    })
}

fn lazy_list_state_is_saved_with_the_host() {
    // Compose's rememberLazyListState (LazyListState.Saver) and LazySaveableStateHolder: the first visible item
    // and its offset, and the saveable state of the items composed, come back with the host's state.
    let bytes = {
        let (shown, state, counts) =
            (mutable_state_of(true), Rc::new(RefCell::new(None)), Rc::new(RefCell::new(Vec::new())));
        let rule = KompozTestRule::with_viewport(saved_list(&shown, &state, &counts), IntSize::new(400, 400));
        let scope = kompoz_coroutines::CoroutineScope::new(rule.host().dispatcher());
        let s = state.borrow().clone().unwrap();
        scope.launch(async move { named_args!(s.scroll_to_item(30, scroll_offset = 7)).await });
        rule.wait_for_idle();
        counts.borrow().last().unwrap().set(3);
        rule.wait_for_idle();
        rule.on_node_with_text("satır 31: 3").assert_exists();
        rule.host().save_state()
    };
    let (shown, state, counts) =
        (mutable_state_of(false), Rc::new(RefCell::new(None)), Rc::new(RefCell::new(Vec::new())));
    let rule = KompozTestRule::with_viewport(saved_list(&shown, &state, &counts), IntSize::new(400, 400));
    rule.host().restore_state(&bytes);
    shown.set(true);
    rule.wait_for_idle();
    let s = state.borrow().clone().unwrap();
    assert_eq!((s.first_visible_item_index(), s.first_visible_item_scroll_offset()), (30, 7));
    rule.on_node_with_text("satır 31: 3").assert_exists();
}

tests!(
    lazy_column_composes_only_what_is_visible,
    lazy_column_scrolls_and_disposes_items,
    lazy_list_composes_the_next_item_in_a_pause,
    lazy_list_state_opens_at_an_item,
    lazy_column_stops_at_the_end,
    lazy_column_keys_keep_item_state,
    lazy_column_content_padding_and_spacing,
    lazy_column_scroll_to_item_and_observable_index,
    lazy_row_inside_lazy_column_scrolls_on_its_own_axis,
    fling_keeps_scrolling_after_release,
    lazy_column_items_keep_saveable_state_when_scrolled_away,
    lazy_items_keep_a_temporary_list,
    animate_item_glides_on_removal_and_fades_in_on_insert,
    animate_item_fades_a_removed_item_out_where_it_was,
    reverse_layout_puts_the_first_item_at_the_bottom,
    animate_scroll_to_item_glides_there,
    animate_scroll_to_item_finds_items_bigger_than_guessed,
    lazy_lists_arrange_items_that_all_fit,
    rtl_snapping_row_settles_on_with_what_an_inner_fling_left,
    sticky_headers_stay_at_the_top_until_the_next_one_pushes,
    lazy_list_layout_info_tells_what_is_in_view,
    match_parent_size_and_fill_parent_max,
    snap_fling_behavior_settles_on_an_item,
    a_hair_short_of_an_item_counts_as_there,
    local_overscroll_factory_turns_the_stretch_off,
    overscroll_effect_none_shared_or_halved,
    bring_into_view_spec_puts_the_focused_item_where_it_says,
    what_shows_past_a_lazy_row_across_counts_as_shown,
    scroll_states_tell_whether_they_scroll,
    accessibility_scrolls_to_an_index_and_a_key,
    a_pinned_item_stays_composed_out_of_view,
    focus_moves_beyond_the_bounds,
    a_cache_window_keeps_items_composed_ahead,
    lazy_list_state_is_saved_with_the_host,
);
