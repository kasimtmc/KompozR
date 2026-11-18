//! Lazy grids and staggered grids: cells (fixed, adaptive), full-line spans, lines along either axis, composing
//! only what shows and the next line in a pause, items under the shortest lane, the states going by item and
//! telling rows, columns and lanes, snapping, overscroll — the C++ Kompoz's grid tests (survey F) and Compose's
//! behaviour where the two differ. Numbers in px (density 1).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_animation::*;
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_foundation::gestures::snapping::*;
use kompoz_foundation::lazy::grid::*;
use kompoz_foundation::lazy::staggeredgrid::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::input::PointerType;
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

/// Left, top, width, height within half a pixel (C++'s `sameRect`).
fn same_rect(r: Rect, x: f32, y: f32, w: f32, h: f32) -> bool {
    near(r.left, x) && near(r.top, y) && near(r.width(), w) && near(r.height(), h)
}

type Scope = Rc<RefCell<Option<CoroutineScope>>>;

fn new_scope() -> Scope {
    Rc::new(RefCell::new(None))
}

fn launch(scope: &Scope, f: impl std::future::Future<Output = ()> + 'static) {
    scope.borrow().as_ref().unwrap().launch(f);
}

/// Runs a scroll of `state` by `by` px in a coroutine of its own (C++'s synchronous scrolls); what it took.
fn scroll_by<S: ScrollableState + Clone + 'static>(rule: &KompozTestRule, scope: &Scope, state: &S, by: f32) -> f32 {
    let took = Rc::new(Cell::new(f32::NAN));
    let (s, t) = (state.clone(), took.clone());
    launch(scope, async move { t.set(s.scroll_by(by).await) });
    rule.wait_for_idle();
    took.get()
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

/// The span of a whole line (C++'s `LazyGridScope::FullLine`; Compose's `{ GridItemSpan(maxLineSpan) }`).
fn full_line() -> Option<Rc<dyn for<'s> Fn(&'s LazyGridItemSpanScope) -> GridItemSpan>> {
    Some(Rc::new(|s: &LazyGridItemSpanScope| GridItemSpan(s.max_line_span())))
}

fn lazy_vertical_grid_fixed_adaptive_and_full_line_spans() {
    // C++ 10132: 3 columns of (400 - 2 · 10) / 3 = 126.67 — whole pixels here, the remainder to the first ones (Compose's
    // calculateCellsCrossAxisSizeImpl: 127, 127, 126); a full-line header; adaptive cells of at least 100.
    let adaptive = mutable_state_of(false);
    let a = adaptive.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            LazyVerticalGrid(
                columns = if a.get() { GridCells::Adaptive(100.dp()) } else { GridCells::Fixed(3) },
                modifier = Modifier.fill_max_size(),
                vertical_arrangement = Some(Arrangement::spaced_by(8.dp())),
                horizontal_arrangement = Arrangement::spaced_by(10.dp()),
                content = |this: &mut LazyGridScope| {
                    this.item(
                        span = full_line(),
                        content = || {
                            Box(modifier = Modifier.fill_max_width().height(20.dp()).test_tag("header"));
                        },
                    );
                    this.items_list(
                        (0..300).collect::<Vec<i32>>(),
                        key = kompoz_foundation::lazy::item_key(|v: &i32| *v),
                        item_content = |v: &i32| {
                            Box(modifier = Modifier.fill_max_width().height(50.dp()).test_tag(format!("cell{v}")));
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    assert!(same_rect(bounds(&rule, "header"), 0.0, 0.0, 400.0, 20.0));
    assert!(same_rect(bounds(&rule, "cell0"), 0.0, 28.0, 127.0, 50.0), "{:?}", bounds(&rule, "cell0"));
    assert!(same_rect(bounds(&rule, "cell2"), 274.0, 28.0, 126.0, 50.0), "{:?}", bounds(&rule, "cell2"));
    assert!(same_rect(bounds(&rule, "cell3"), 0.0, 28.0 + 58.0, 127.0, 50.0));
    // (only the lines near the view)
    assert!(has(&rule, "cell29") && !has(&rule, "cell299"));
    // 100 minimum: (400 + 10) / 110 = 3 columns; at 450 wide, 4.
    adaptive.set(true);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "cell3").top, 28.0 + 58.0));
    rule.host().set_viewport_size(IntSize::new(450, 800));
    rule.wait_for_idle();
    // (450 - 3 · 10) / 4 = 105 each)
    assert!(near(bounds(&rule, "cell3").left, 3.0 * (105.0 + 10.0)) && near(bounds(&rule, "cell4").left, 0.0));
}

fn lazy_horizontal_grid_lays_out_columns() {
    // C++ 10156: rows of (320 - 2 · 10) / 3 = 100; the full-line item takes a whole column; scrolled sideways.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            LazyHorizontalGrid(
                rows = GridCells::Fixed(3),
                modifier = Modifier.fill_max_width().height(320.dp()).test_tag("grid"),
                vertical_arrangement = Arrangement::spaced_by(10.dp()),
                horizontal_arrangement = Some(Arrangement::spaced_by(8.dp())),
                content = |this: &mut LazyGridScope| {
                    this.item(
                        span = full_line(),
                        content = || {
                            Box(modifier = Modifier.width(40.dp()).fill_max_height().test_tag("side"));
                        },
                    );
                    this.items(
                        300,
                        item_content = |v| {
                            Box(modifier = Modifier.width(90.dp()).fill_max_height().test_tag(format!("cell{v}")));
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    assert!(same_rect(bounds(&rule, "side"), 0.0, 0.0, 40.0, 320.0));
    assert!(same_rect(bounds(&rule, "cell0"), 48.0, 0.0, 90.0, 100.0));
    assert!(same_rect(bounds(&rule, "cell2"), 48.0, 220.0, 90.0, 100.0));
    assert!(same_rect(bounds(&rule, "cell3"), 48.0 + 98.0, 0.0, 90.0, 100.0));
    assert!(has(&rule, "cell9") && !has(&rule, "cell299"));
    swipe(&rule, at(380.0, 150.0), at(20.0, 150.0), 20);
    rule.wait_for_idle();
    assert!(!has(&rule, "cell0") || bounds(&rule, "cell0").left < 0.0);
    assert!(has(&rule, "cell20"));
}

fn staggered_grid_composes_the_next_item_in_a_pause() {
    // C++ 10179: scrolled, each lane's next item is composed in a pause between frames (prefetched), not before;
    // (C++ composed the next one alone: Compose prefetches each lane's next, here items `past` and `past + 1`).
    let state = LazyStaggeredGridState::new(0, 0);
    let made = Rc::new(RefCell::new(Vec::<i32>::new()));
    let scope = new_scope();
    let (s, m, sc) = (state.clone(), made.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let m = m.clone();
            LazyVerticalStaggeredGrid(
                columns = StaggeredGridCells::Fixed(2),
                modifier = Modifier.fill_max_size(),
                state = Some(s.clone()),
                content =
                    move |this: &mut LazyStaggeredGridScope| {
                        let m = m.clone();
                        this.items(200, item_content = move |i| {
                        let m = m.clone();
                        remember(move || m.borrow_mut().push(i));
                        Box(modifier = Modifier.fill_max_width().height(300.dp()).test_tag(format!("cell {i}")));
                    });
                    },
            );
        }),
        IntSize::new(400, 800),
    );
    let has_cell = |i: i32| has(&rule, &format!("cell {i}"));
    let mut past = 0;
    while has_cell(past) {
        past += 1;
    }
    assert!(past > 2 && !rule.host().has_idle_work());
    scroll_by(&rule, &scope, &state, 10.0);
    assert!(!has_cell(past) && rule.host().has_idle_work());
    made.borrow_mut().clear();
    rule.host().idle(1_000_000_000);
    assert_eq!(*made.borrow(), vec![past, past + 1]);
    rule.wait_for_idle();
    assert!(has_cell(past - 1));
}

/// The heights of C++'s staggered items.
fn height_of(i: i32) -> f32 {
    (50 + (i * 37) % 100) as f32
}

fn staggered_grid_puts_items_under_the_shortest_lane() {
    // C++ 10206: lanes of (400 - 8) / 2 = 196; each item under the shorter lane (the first on a tie), a full-line
    // one under the longer, across both.
    let state = LazyStaggeredGridState::new(0, 0);
    let scope = new_scope();
    let (s, sc) = (state.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            LazyVerticalStaggeredGrid(
                columns = StaggeredGridCells::Fixed(2),
                modifier = Modifier.fill_max_size(),
                state = Some(s.clone()),
                vertical_item_spacing = 8.dp(),
                horizontal_arrangement = Arrangement::spaced_by(8.dp()),
                content = |this: &mut LazyStaggeredGridScope| {
                    this.item(
                        span = Some(StaggeredGridItemSpan::FullLine),
                        content = || {
                            Box(modifier = Modifier.fill_max_width().height(40.dp()).test_tag("header"));
                        },
                    );
                    this.items(
                        10,
                        item_content = |i| {
                            Box(modifier =
                                Modifier.fill_max_width().height(height_of(i).dp()).test_tag(format!("a{i}")));
                        },
                    );
                    this.item(
                        span = Some(StaggeredGridItemSpan::FullLine),
                        content = || {
                            Box(modifier = Modifier.fill_max_width().height(30.dp()).test_tag("middle"));
                        },
                    );
                    this.items(
                        500,
                        item_content = |i| {
                            Box(modifier =
                                Modifier.fill_max_width().height(height_of(i).dp()).test_tag(format!("b{i}")));
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let mut lanes = [48.0f32, 48.0];
    let mut expect = |tag: String, h: f32| {
        let lane = if lanes[0] <= lanes[1] { 0 } else { 1 };
        let r = bounds(&rule, &tag);
        let ok = same_rect(r, lane as f32 * 204.0, lanes[lane], 196.0, h);
        lanes[lane] += h + 8.0;
        ok
    };
    assert!(same_rect(bounds(&rule, "header"), 0.0, 0.0, 400.0, 40.0));
    for i in 0..10 {
        assert!(expect(format!("a{i}"), height_of(i)), "a{i}");
    }
    let below = lanes[0].max(lanes[1]);
    assert!(same_rect(bounds(&rule, "middle"), 0.0, below, 400.0, 30.0));
    lanes = [below + 38.0, below + 38.0];
    // (as far as the view goes: C++ composed half a view more; Compose composes what shows)
    for i in 0..6 {
        if !has(&rule, &format!("b{i}")) {
            assert!(i >= 3 && lanes[0].min(lanes[1]) >= 800.0 - 150.0, "b{i}");
            break;
        }
        let ok = {
            let lane = if lanes[0] <= lanes[1] { 0 } else { 1 };
            let r = bounds(&rule, &format!("b{i}"));
            let ok = same_rect(r, lane as f32 * 204.0, lanes[lane], 196.0, height_of(i));
            lanes[lane] += height_of(i) + 8.0;
            ok
        };
        assert!(ok, "b{i}");
    }
    assert!(!has(&rule, "b200"));
    // Far down: the items there composed, the first ones let go.
    scroll_by(&rule, &scope, &state, 20_000.0);
    let shown = (0..500)
        .filter(|i| {
            let tag = format!("b{i}");
            has(&rule, &tag) && {
                let b = bounds(&rule, &tag);
                b.top < 800.0 && b.bottom > 0.0
            }
        })
        .count();
    assert!(shown >= 6 && !has(&rule, "a0"), "{shown}");
    // And back: the same places (a1 then a3 on the right).
    scroll_by(&rule, &scope, &state, -40_000.0);
    assert!(same_rect(bounds(&rule, "a3"), 204.0, 48.0 + height_of(1) + 8.0, 196.0, height_of(3)));
}

fn horizontal_staggered_grid_puts_items_after_the_shortest_row() {
    // C++ 10253: rows of (208 - 8) / 2 = 100; each item after the shorter row, 6 on; the layout info along x; by
    // item (C++'s `info.lanes` isn't Compose's: the lanes are the items' `lane`s).
    let state = LazyStaggeredGridState::new(0, 0);
    let scope = new_scope();
    let (s, sc) = (state.clone(), scope.clone());
    let width_of = height_of;
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            LazyHorizontalStaggeredGrid(
                rows = StaggeredGridCells::Fixed(2),
                modifier = Modifier.fill_max_width().height(208.dp()),
                state = Some(s.clone()),
                horizontal_item_spacing = 6.dp(),
                vertical_arrangement = Arrangement::spaced_by(8.dp()),
                content = |this: &mut LazyStaggeredGridScope| {
                    this.item(
                        span = Some(StaggeredGridItemSpan::FullLine),
                        content = || {
                            Box(modifier = Modifier.width(40.dp()).fill_max_height().test_tag("header"));
                        },
                    );
                    this.items(
                        300,
                        item_content = |i| {
                            Box(modifier =
                                Modifier.width(height_of(i).dp()).fill_max_height().test_tag(format!("a{i}")));
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let mut rows = [46.0f32, 46.0];
    assert!(same_rect(bounds(&rule, "header"), 0.0, 0.0, 40.0, 208.0));
    for i in 0..6 {
        let row = if rows[0] <= rows[1] { 0 } else { 1 };
        assert!(same_rect(bounds(&rule, &format!("a{i}")), rows[row], row as f32 * 108.0, width_of(i), 100.0), "a{i}");
        rows[row] += width_of(i) + 6.0;
    }
    assert!(!has(&rule, "a200"));
    let info = state.layout_info();
    assert!(info.viewport_size() == IntSize::new(400, 208) && info.main_axis_item_spacing() == 6);
    assert_eq!(info.orientation(), Orientation::Horizontal);
    // By item: its start at the start.
    let s = state.clone();
    launch(&scope, async move { s.scroll_to_item(101).await });
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "a100").left, 0.0) && state.first_visible_item_index() <= 101);
    let info = state.layout_info();
    let items = info.visible_items_info();
    assert!(!items.is_empty() && items[0].index() == state.first_visible_item_index());
    for item in &items {
        let b = bounds(&rule, &format!("a{}", item.index() - 1));
        assert!(near(item.offset().x as f32, b.left) && near(item.offset().y as f32, b.top));
        assert!(near(item.size().width as f32, b.width()) && item.size().height == 100);
        assert!(item.lane() == if b.top > 50.0 { 1 } else { 0 } && b.left < 400.0 && b.right > 0.0);
    }
    assert!(!has(&rule, "a0"));
}

fn snapping_grid_settles_with_a_row_lined_up() {
    // C++ 14724: the same fling on both: the snapping one stops with a row at the top, the other anywhere.
    let grid = LazyGridState::new(0, 0);
    let free = LazyGridState::new(0, 0);
    let (g, f) = (grid.clone(), free.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (g, f) = (g.clone(), f.clone());
            Row(content = move || {
                let snapping =
                    named_args!(remember_snap_fling_behavior_for_grid(&g, snap_position = SnapPosition::Start));
                for (s, fling) in [(g.clone(), Some(snapping)), (f.clone(), None)] {
                    LazyVerticalGrid(
                        columns = GridCells::Fixed(2),
                        modifier = Modifier.size_wh(150.dp(), 450.dp()),
                        state = Some(s),
                        fling_behavior = fling,
                        content = |this: &mut LazyGridScope| {
                            this.items(
                                200,
                                item_content = |_| Box(modifier = Modifier.fill_max_width().height(100.dp())),
                            );
                        },
                    );
                }
            });
        }),
        IntSize::new(300, 800),
    );
    swipe(&rule, at(75.0, 400.0), at(75.0, 250.0), 4);
    swipe(&rule, at(225.0, 400.0), at(225.0, 250.0), 4);
    rule.wait_for_idle();
    assert!(
        grid.first_visible_item_index() > 0
            && grid.first_visible_item_scroll_offset() == 0
            && grid.first_visible_item_index() % 2 == 0,
        "{grid:?}"
    );
    assert!(free.first_visible_item_index() > 0 && free.first_visible_item_scroll_offset() > 0, "{free:?}");
}

fn lazy_grid_state_goes_by_item_and_tells_rows_and_columns() {
    // C++ 17750: opened at item 10's row; by item, its line's first item the first visible; rows and columns told;
    // animated; sideways the lines are columns.
    let state: Rc<RefCell<Option<LazyGridState>>> = Rc::new(RefCell::new(None));
    let sideways: Rc<RefCell<Option<LazyGridState>>> = Rc::new(RefCell::new(None));
    let scope = new_scope();
    let (st, sw, sc) = (state.clone(), sideways.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = remember_lazy_grid_state(initial_first_visible_item_index = 10);
            *st.borrow_mut() = Some(s.clone());
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let sw = sw.clone();
            Column(
                content = move || {
                    LazyVerticalGrid(
                        columns = GridCells::Fixed(3),
                        modifier = Modifier.size_wh(300.dp(), 400.dp()),
                        state = Some(s.clone()),
                        vertical_arrangement = Some(Arrangement::spaced_by(10.dp())),
                        horizontal_arrangement = Arrangement::spaced_by(6.dp()),
                        content = |this: &mut LazyGridScope| {
                            // (item 0: a row of its own)
                            this.item(
                                span = full_line(),
                                content = || Box(modifier = Modifier.fill_max_width().height(50.dp())),
                            );
                            this.items(
                                60,
                                item_content = |i| {
                                    Box(modifier =
                                        Modifier.fill_max_width().height(if i % 2 == 1 { 80.dp() } else { 60.dp() }));
                                },
                            );
                        },
                    );
                    let h = remember_lazy_grid_state();
                    *sw.borrow_mut() = Some(h.clone());
                    LazyHorizontalGrid(
                        rows = GridCells::Fixed(2),
                        modifier = Modifier.size_wh(300.dp(), 200.dp()),
                        state = Some(h),
                        content = |this: &mut LazyGridScope| {
                            this.items(
                                20,
                                item_content = |_| Box(modifier = Modifier.width(70.dp()).fill_max_height()),
                            );
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let state = state.borrow().clone().unwrap();
    let sideways = sideways.borrow().clone().unwrap();
    // Rows: [0], [1 2 3], [4 5 6] … item 10 starts the fifth.
    assert!(state.first_visible_item_index() == 10 && state.first_visible_item_scroll_offset() == 0);
    let opened = state.layout_info().visible_items_info();
    assert!(!opened.is_empty() && opened[0].index() == 10);
    let s = state.clone();
    launch(&scope, async move { s.scroll_to_item(20).await });
    rule.wait_for_idle();
    // (in the row [19 20 21]: its first the first visible)
    assert_eq!(state.first_visible_item_index(), 19);
    let info = state.layout_info();
    assert!(info.total_items_count() == 61 && info.max_span() == 3 && info.orientation() == Orientation::Vertical);
    assert!(info.main_axis_item_spacing() == 10);
    let items = info.visible_items_info();
    assert!(items.len() >= 6 && items[0].index() == 19);
    // Cells 96 wide (300 less two gaps of 6, by three); their own heights.
    let (a, b, c) = (&items[0], &items[1], &items[2]);
    assert!(a.row() == 7 && a.column() == 0 && b.column() == 1 && c.column() == 2 && b.row() == 7);
    assert!(a.offset() == IntOffset::new(0, 0) && b.offset() == IntOffset::new(102, 0) && c.offset().x == 204);
    assert!(a.size() == IntSize::new(96, 60) && b.size().height == 80);
    // (below the row's tallest, past the gap)
    assert!(items[3].row() == 8 && items[3].offset().y == 80 + 10);
    let s = state.clone();
    launch(&scope, async move { s.animate_scroll_to_item(0).await });
    rule.wait_for_idle();
    assert!(state.first_visible_item_index() == 0 && !state.can_scroll_backward() && state.can_scroll_forward());
    let info = state.layout_info();
    assert!(info.visible_items_info()[0].index() == 0 && info.visible_items_info()[0].size().width == 300);
    let s = state.clone();
    launch(&scope, async move { s.animate_scroll_to_item(30).await });
    rule.wait_for_idle();
    // (item 30's row: [28 29 30])
    assert_eq!(state.first_visible_item_index(), 28);
    // Sideways: the lines are columns ([0 1], [2 3], [4 5], [6 7]).
    let s = sideways.clone();
    launch(&scope, async move { s.scroll_to_item(7).await });
    rule.wait_for_idle();
    let side = sideways.layout_info();
    assert!(sideways.first_visible_item_index() == 6 && side.orientation() == Orientation::Horizontal);
    let items = side.visible_items_info();
    assert!(items[0].index() == 6 && items[0].column() == 3 && items[0].row() == 0);
    assert!(items[1].row() == 1 && items[1].offset().y == 100 && items[0].size().width == 70);
}

fn lazy_staggered_grid_state_goes_by_item_and_tells_lanes() {
    // C++ 18007: opened at item 30, its top where the content starts; the layout info as the nodes are (lanes of
    // 196, offsets from the content's start); by item at once and animated; partly scrolled out; a scroll right
    // after a scroll to an item wins; a tall item in one lane leaves the indices beside it out.
    let state: Rc<RefCell<Option<LazyStaggeredGridState>>> = Rc::new(RefCell::new(None));
    let scope = new_scope();
    let (st, sc) = (state.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = remember_lazy_staggered_grid_state(initial_first_visible_item_index = 30);
            *st.borrow_mut() = Some(s.clone());
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            Column(
                content = move || {
                    BasicText(format!("ilk {}", s.first_visible_item_index()), modifier = Modifier.test_tag("ilk"));
                    LazyVerticalStaggeredGrid(
                        columns = StaggeredGridCells::Fixed(2),
                        modifier = Modifier.size_wh(400.dp(), 600.dp()).test_tag("ızgara"),
                        state = Some(s.clone()),
                        content_padding = PaddingValues::new(0.dp(), 10.dp(), 0.dp(), 10.dp()),
                        vertical_item_spacing = 8.dp(),
                        horizontal_arrangement = Arrangement::spaced_by(8.dp()),
                        content = |this: &mut LazyStaggeredGridScope| {
                            this.items(
                                200,
                                item_content = |i| {
                                    Box(modifier =
                                        Modifier.fill_max_width().height(height_of(i).dp()).test_tag(format!("s{i}")));
                                },
                            );
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let state = state.borrow().clone().unwrap();
    let grid = bounds(&rule, "ızgara");
    assert!(near(bounds(&rule, "s30").top, grid.top + 10.0));
    let first = state.first_visible_item_index();
    assert!(first <= 30);
    rule.on_node_with_text(&format!("ilk {first}")).assert_exists();
    let check_info = || {
        let info = state.layout_info();
        let items = info.visible_items_info();
        assert!(info.total_items_count() == 200 && !items.is_empty());
        // (C++ made the first item in view the state's first; Compose's state's first is the lanes' first past the
        // content padding, items in the padding shown before it)
        assert!(items.iter().any(|i| i.index() == state.first_visible_item_index()));
        assert!(items.windows(2).all(|w| w[0].index() < w[1].index()));
        for item in &items {
            let b = bounds(&rule, &format!("s{}", item.index()));
            assert!(near(item.offset().y as f32, b.top - grid.top - 10.0), "{item:?} {b:?}");
            assert!(near(item.offset().x as f32, b.left - grid.left) && item.size().width == 196);
            assert!(near(item.size().height as f32, b.height()));
            assert_eq!(item.lane(), if b.left > grid.left + 100.0 { 1 } else { 0 });
            // (in view, all of them)
            assert!(b.top < grid.bottom && b.bottom > grid.top);
        }
    };
    check_info();
    let s = state.clone();
    launch(&scope, async move { s.scroll_to_item(100).await });
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "s100").top, grid.top + 10.0));
    check_info();
    rule.on_node_with_text(&format!("ilk {}", state.first_visible_item_index())).assert_exists();
    let s = state.clone();
    launch(&scope, async move { s.animate_scroll_to_item(0).await });
    rule.wait_for_idle();
    assert!(state.first_visible_item_index() == 0 && state.first_visible_item_scroll_offset() == 0);
    assert!(!state.can_scroll_backward() && state.can_scroll_forward());
    rule.on_node_with_text("ilk 0").assert_exists();
    // (partly scrolled out: the offset says how much)
    scroll_by(&rule, &scope, &state, 25.0);
    assert!(state.first_visible_item_index() == 0 && state.first_visible_item_scroll_offset() == 25);
    // A scroll right after a scroll to an item (a finger) wins: it cancels it.
    let (s1, s2) = (state.clone(), state.clone());
    launch(&scope, async move { s1.scroll_to_item(150).await });
    launch(&scope, async move {
        s2.scroll_by(-100_000.0).await;
    });
    rule.wait_for_idle();
    assert!(state.first_visible_item_index() == 0 && state.first_visible_item_scroll_offset() == 0, "{state:?}");

    // A tall item in one lane, short ones in the other: scrolled past the short ones, only the tall one and what
    // is in view count.
    let tall = LazyStaggeredGridState::new(0, 0);
    let other_scope = new_scope();
    let (t, osc) = (tall.clone(), other_scope.clone());
    let other = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *osc.borrow_mut() = Some(remember_coroutine_scope());
            LazyVerticalStaggeredGrid(
                columns = StaggeredGridCells::Fixed(2),
                modifier = Modifier.size_wh(400.dp(), 300.dp()),
                state = Some(t.clone()),
                content = |this: &mut LazyStaggeredGridScope| {
                    this.item(content = || Box(modifier = Modifier.fill_max_width().height(800.dp())));
                    this.items(40, item_content = |_| Box(modifier = Modifier.fill_max_width().height(50.dp())));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    scroll_by(&other, &other_scope, &tall, 400.0);
    let items = tall.layout_info().visible_items_info();
    assert!(!items.is_empty() && items[0].index() == 0 && (items.len() < 2 || items[1].index() >= 9), "{items:?}");
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

/// Down at `from`, moved by `by` over 10 frames, held; the stretches then; let go, until idle.
fn pull(rule: &KompozTestRule, from: Offset, by: Offset) -> Vec<(Rect, Offset)> {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=10 {
        rule.advance_frame();
        h.pointer_move(from + by * (i as f32 / 10.0), -1, 0);
    }
    let stretched = stretches(rule);
    h.pointer_up(from + by, -1, 0);
    rule.wait_for_idle();
    stretched
}

fn grids_stretch_unless_told_not_to() {
    // C++ 3885's and 4303's grid parts: by default a grid and a staggered grid stretch pulled past their start;
    // under no factory, or given no effect (`Some(None)`), they don't.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Row(content = || {
                LazyVerticalGrid(
                    columns = GridCells::Fixed(2),
                    modifier = Modifier.weight(1.0).fill_max_height(),
                    content = |this: &mut LazyGridScope| {
                        this.items(60, item_content = |_| Box(modifier = Modifier.fill_max_width().height(50.dp())));
                    },
                );
                LazyVerticalStaggeredGrid(
                    columns = StaggeredGridCells::Fixed(2),
                    modifier = Modifier.weight(1.0).fill_max_height(),
                    content = |this: &mut LazyStaggeredGridScope| {
                        this.items(60, item_content = |_| Box(modifier = Modifier.fill_max_width().height(50.dp())));
                    },
                );
                LazyVerticalGrid(
                    columns = GridCells::Fixed(2),
                    modifier = Modifier.weight(1.0).fill_max_height(),
                    overscroll_effect = Some(None),
                    content = |this: &mut LazyGridScope| {
                        this.items(60, item_content = |_| Box(modifier = Modifier.fill_max_width().height(50.dp())));
                    },
                );
                LazyVerticalStaggeredGrid(
                    columns = StaggeredGridCells::Fixed(2),
                    modifier = Modifier.weight(1.0).fill_max_height(),
                    overscroll_effect = Some(None),
                    content = |this: &mut LazyStaggeredGridScope| {
                        this.items(60, item_content = |_| Box(modifier = Modifier.fill_max_width().height(50.dp())));
                    },
                );
                let none: Option<Rc<dyn OverscrollFactory>> = None;
                KompozitionLocalProvider(&[LocalOverscrollFactory.provides(none)], || {
                    LazyVerticalGrid(
                        columns = GridCells::Fixed(2),
                        modifier = Modifier.weight(1.0).fill_max_height(),
                        content = |this: &mut LazyGridScope| {
                            this.items(
                                60,
                                item_content = |_| Box(modifier = Modifier.fill_max_width().height(50.dp())),
                            );
                        },
                    );
                    LazyVerticalStaggeredGrid(
                        columns = StaggeredGridCells::Fixed(2),
                        modifier = Modifier.weight(1.0).fill_max_height(),
                        content = |this: &mut LazyStaggeredGridScope| {
                            this.items(
                                60,
                                item_content = |_| Box(modifier = Modifier.fill_max_width().height(50.dp())),
                            );
                        },
                    );
                });
            });
        }),
        IntSize::new(600, 800),
    );
    for (column, stretch) in [(0, true), (1, true), (2, false), (3, false), (4, false), (5, false)] {
        let x = 50.0 + column as f32 * 100.0;
        let stretched = pull(&rule, at(x, 300.0), at(0.0, 200.0));
        let here = stretched.iter().any(|(b, d)| b.contains(at(x, 300.0)) && d.y > 0.0);
        assert_eq!(here, stretch, "column {column}: {stretched:?}");
    }
}

fn grid_scroll_state_tells_whether_it_scrolls() {
    // C++ 12899's grid part: an animated scroll to an item scrolls while it runs (read in a composition too).
    let grid = LazyGridState::new(0, 0);
    let scope = new_scope();
    let (g, sc) = (grid.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let g = g.clone();
            Column(
                content = move || {
                    BasicText(if g.is_scroll_in_progress() { "ızgara kayıyor" } else { "ızgara" });
                    LazyVerticalGrid(
                        columns = GridCells::Fixed(2),
                        modifier = Modifier.size_wh(200.dp(), 200.dp()),
                        state = Some(g.clone()),
                        content = |this: &mut LazyGridScope| {
                            this.items(
                                100,
                                item_content = |i| BasicText(format!("{i}"), modifier = Modifier.height(40.dp())),
                            );
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 1200),
    );
    rule.on_node_with_text("ızgara").assert_exists();
    let s = grid.clone();
    launch(&scope, async move { s.animate_scroll_to_item(30).await });
    rule.advance_frame();
    rule.advance_frame();
    assert!(grid.is_scroll_in_progress());
    rule.on_node_with_text("ızgara kayıyor").assert_exists();
    rule.wait_for_idle();
    assert!(!grid.is_scroll_in_progress() && grid.first_visible_item_index() == 30);
}

fn sticky_grid_headers_stay_at_the_top() {
    // Compose's grid sticky headers: a whole line, staying at the top until the next one pushes it out.
    let state = LazyGridState::new(0, 0);
    let scope = new_scope();
    let (s, sc) = (state.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            LazyVerticalGrid(
                columns = GridCells::Fixed(2),
                modifier = Modifier.size_wh(200.dp(), 300.dp()),
                state = Some(s.clone()),
                content = |this: &mut LazyGridScope| {
                    for section in 0..3 {
                        this.sticky_header(
                            content = move |_| {
                                Box(modifier =
                                    Modifier.fill_max_width().height(20.dp()).test_tag(format!("başlık {section}")));
                            },
                        );
                        this.items(
                            10,
                            item_content = move |i| {
                                Box(modifier =
                                    Modifier.fill_max_width().height(50.dp()).test_tag(format!("{section}.{i}")));
                            },
                        );
                    }
                },
            );
        }),
        IntSize::new(400, 800),
    );
    assert!(same_rect(bounds(&rule, "başlık 0"), 0.0, 0.0, 200.0, 20.0));
    scroll_by(&rule, &scope, &state, 100.0);
    // (the first section's header at the top still)
    assert!(near(bounds(&rule, "başlık 0").top, 0.0) && near(bounds(&rule, "0.2").top, 20.0 + 50.0 - 100.0));
    // The next one pushes it out: 20 + 5 · 50 = 270 down, the second header 10 from the top pushes the first up.
    scroll_by(&rule, &scope, &state, 160.0);
    assert!(near(bounds(&rule, "başlık 1").top, 10.0) && near(bounds(&rule, "başlık 0").top, -10.0));
}

fn grid_composes_the_next_line_in_a_pause() {
    // Compose's grid prefetch: scrolled, the items of the next line are composed in a pause between frames.
    let state = LazyGridState::new(0, 0);
    let made = Rc::new(RefCell::new(Vec::<i32>::new()));
    let scope = new_scope();
    let (s, m, sc) = (state.clone(), made.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let m = m.clone();
            LazyVerticalGrid(
                columns = GridCells::Fixed(3),
                modifier = Modifier.fill_max_size(),
                state = Some(s.clone()),
                content =
                    move |this: &mut LazyGridScope| {
                        let m = m.clone();
                        this.items(300, item_content = move |i| {
                        let m = m.clone();
                        remember(move || m.borrow_mut().push(i));
                        Box(modifier = Modifier.fill_max_width().height(300.dp()).test_tag(format!("cell {i}")));
                    });
                    },
            );
        }),
        IntSize::new(300, 800),
    );
    // (lines of 300 in 800: three lines, items 0..8)
    assert!(has(&rule, "cell 8") && !has(&rule, "cell 9") && !rule.host().has_idle_work());
    scroll_by(&rule, &scope, &state, 10.0);
    assert!(!has(&rule, "cell 9") && rule.host().has_idle_work());
    made.borrow_mut().clear();
    rule.host().idle(1_000_000_000);
    assert_eq!(*made.borrow(), vec![9, 10, 11]);
}

fn grid_items_glide_when_reordered() {
    // An item's `Modifier.animate_item` in a grid (Compose's LazyGridItemScope.animateItem): moved to another
    // cell, it glides there along its placement spec.
    let order = mutable_state_of(vec![0, 1, 2, 3]);
    let o = order.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let list = o.get();
            LazyVerticalGrid(
                columns = GridCells::Fixed(2),
                modifier = Modifier.size_wh(200.dp(), 400.dp()),
                content = move |this: &mut LazyGridScope| {
                    this.items_list(
                        list.clone(),
                        key = kompoz_foundation::lazy::item_key(|v: &i32| *v),
                        item_content = |v: &i32| {
                            let spec: TweenSpec<IntOffset> =
                                tween(duration_millis = 200, easing = Arc::new(LinearEasing));
                            let spec: Arc<dyn FiniteAnimationSpec<IntOffset>> = Arc::new(spec);
                            Box(modifier = named_args!(Modifier.animate_item(placement_spec = Some(spec)))
                                .fill_max_width()
                                .height(50.dp())
                                .test_tag(format!("hücre {v}")));
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    assert!(same_rect(bounds(&rule, "hücre 3"), 100.0, 50.0, 100.0, 50.0));
    // (3 to the first cell: from (100, 50) to (0, 0) in 200 ms)
    order.set(vec![3, 0, 1, 2]);
    rule.advance_time_by(100);
    let mid = bounds(&rule, "hücre 3");
    assert!(mid.left > 10.0 && mid.left < 90.0 && mid.top > 5.0 && mid.top < 45.0, "{mid:?}");
    rule.wait_for_idle();
    assert!(
        same_rect(bounds(&rule, "hücre 3"), 0.0, 0.0, 100.0, 50.0)
            && same_rect(bounds(&rule, "hücre 0"), 100.0, 0.0, 100.0, 50.0)
    );
}

tests!(
    lazy_vertical_grid_fixed_adaptive_and_full_line_spans,
    lazy_horizontal_grid_lays_out_columns,
    staggered_grid_composes_the_next_item_in_a_pause,
    staggered_grid_puts_items_under_the_shortest_lane,
    horizontal_staggered_grid_puts_items_after_the_shortest_row,
    snapping_grid_settles_with_a_row_lined_up,
    lazy_grid_state_goes_by_item_and_tells_rows_and_columns,
    lazy_staggered_grid_state_goes_by_item_and_tells_lanes,
    grids_stretch_unless_told_not_to,
    grid_scroll_state_tells_whether_it_scrolls,
    sticky_grid_headers_stay_at_the_top,
    grid_composes_the_next_line_in_a_pause,
    grid_items_glide_when_reordered,
);
