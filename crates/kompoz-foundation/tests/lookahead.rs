//! Lazy layouts, pagers and scrollers in a LookaheadScope, as Compose's lookahead has them: the lazy list, grid,
//! staggered grid and pager measured and placed in lookahead first — where their items are going — then in the
//! approach (items still approaching their lookahead sizes laid out by those, items only the approach shows composed
//! for it alone); `animate_item` gliding to its lookahead position; the approach kept where it was while the
//! lookahead scrolls back (the scroll delta between the passes); scrolling placed as a motion frame of reference, so
//! `animate_bounds` in an item or a page doesn't chase it. Numbers in px (density 1).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::run as run_tests;
use kompoz_animation::*;
use kompoz_coroutines::CoroutineScope;
use kompoz_foundation::lazy::item_key;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// Where each tagged layout was placed last: in the main pass and in lookahead, in the root.
#[derive(Clone, Default)]
struct Places(Rc<RefCell<HashMap<String, (Offset, Offset)>>>);

impl Places {
    /// A modifier telling where its layout is placed, as `tag`.
    fn of(&self, tag: impl Into<String>) -> Modifier {
        let (places, tag) = (self.0.clone(), tag.into());
        Modifier.on_placed(move |c: &LayoutCoordinates| {
            let lookahead = c.to_lookahead_coordinates().position_in_root();
            places.borrow_mut().insert(tag.clone(), (c.position_in_root(), lookahead));
        })
    }
    fn main(&self, tag: &str) -> Offset {
        self.0.borrow().get(tag).unwrap_or_else(|| panic!("{tag} not placed")).0
    }
    fn lookahead(&self, tag: &str) -> Offset {
        self.0.borrow().get(tag).unwrap_or_else(|| panic!("{tag} not placed")).1
    }
}

fn exists(rule: &KompozTestRule, tag: &str) -> bool {
    rule.on_node_with_tag(tag).exists()
}

/// Runs `block` in a coroutine of its own and waits until all is idle.
fn run_until_idle(rule: &KompozTestRule, block: impl std::future::Future<Output = ()> + 'static) {
    CoroutineScope::new(rule.host().dispatcher()).launch(block);
    rule.wait_for_idle();
}

/// An approach layout half as tall as it is going to be while `approaching`.
fn half_while(approaching: MutableState<bool>) -> Modifier {
    named_args!(Modifier.approach_layout(
        is_measurement_approach_in_progress = move |_| approaching.get(),
        approach_measure = |s: &ApproachMeasureScope, m: &dyn Measurable, _| {
            let target = s.lookahead_size();
            let p = m.measure(Constraints::fixed(target.width, target.height / 2));
            s.layout(p.width(), p.height(), move |pl| pl.place(&p, 0, 0))
        },
    ))
}

fn a_lazy_column_in_a_lookahead_scope_lays_its_items_out_in_both_passes() {
    // Its measure sees the lookahead pass (`is_looking_ahead`) and the approach; items at their lookahead sizes are
    // where they go in both; one still approaching (half its height) lays those after it out by its approach size in
    // the main pass, by its lookahead one in lookahead — and an item only the approach shows (the fifth) is composed
    // for it. Scrolling moves both passes alike, an item's animate_bounds not chasing it.
    let approaching = mutable_state_of(true);
    let looked_ahead = Rc::new(Cell::new(false));
    let places = Places::default();
    let state = LazyListState::new(0, 0);
    let (a, l, p, s) = (approaching.clone(), looked_ahead.clone(), places.clone(), state.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (a, l, p, s) = (a.clone(), l.clone(), p.clone(), s.clone());
        LookaheadScope(move |scope| {
            let (a, l, p, scope) = (a.clone(), l.clone(), p.clone(), scope.clone());
            LazyColumn(
                modifier = Modifier.size_wh(100.dp(), 200.dp()),
                state = Some(s.clone()),
                content = move |this: &mut LazyListScope| {
                    let (a, l, p, scope) = (a.clone(), l.clone(), p.clone(), scope.clone());
                    this.items(
                        20,
                        item_content = move |i| {
                            let l = l.clone();
                            let seen = Modifier.layout(move |sc, m, c| {
                                if sc.is_looking_ahead() {
                                    l.set(true);
                                }
                                let pl = m.measure(c);
                                let (w, h) = (pl.width(), pl.height());
                                sc.layout(w, h, move |x| x.place(&pl, 0, 0))
                            });
                            let approach = if i == 0 { half_while(a.clone()) } else { Modifier };
                            let bounds = if i == 2 { Modifier.animate_bounds(scope.clone()) } else { Modifier };
                            Box(modifier = seen
                                .then(approach)
                                .then(bounds)
                                .fill_max_width()
                                .height(50.dp())
                                .test_tag(format!("item{i}"))
                                .then(p.of(format!("item{i}"))));
                        },
                    );
                },
            );
        });
    }));
    assert!(looked_ahead.get());
    // Item 0 half its height in the approach: what follows it moves up there, not in lookahead.
    assert_eq!(places.main("item1").y, 25.0);
    assert_eq!(places.lookahead("item1").y, 50.0);
    assert_eq!(places.main("item3").y, 125.0);
    // (shown by the approach alone: composed for it)
    assert!(exists(&rule, "item4"));
    assert_eq!(places.main("item4").y, 175.0);
    assert_eq!(state.first_visible_item_index(), 0);
    approaching.set(false);
    rule.wait_for_idle();
    for i in 0..4 {
        let tag = format!("item{i}");
        assert_eq!(places.main(&tag), places.lookahead(&tag), "{tag}");
        assert_eq!(places.main(&tag).y, 50.0 * i as f32);
    }
    // Scrolled by 75: both passes at once, the item animating its bounds not chasing the scroll.
    let s = state.clone();
    run_until_idle(&rule, async move {
        s.scroll_by(75.0).await;
    });
    assert_eq!(state.first_visible_item_index(), 1);
    assert_eq!(state.first_visible_item_scroll_offset(), 25);
    rule.advance_frame();
    assert_eq!(places.main("item1").y, -25.0);
    assert_eq!(places.lookahead("item1").y, -25.0);
    assert_eq!(places.main("item2").y, 25.0);
    assert!(!rule.host().has_pending_work());
}

fn animate_item_in_a_lookahead_scope_glides_to_its_lookahead_position() {
    // Compose's: an item's animation is set up in the lookahead pass (where it goes: its lookahead offset at once)
    // and runs in the approach, which places it on the way there.
    let items = mutable_state_list_of([1, 2, 3, 4]);
    let places = Places::default();
    let (it, p) = (items.clone(), places.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (it, p) = (it.clone(), p.clone());
        LookaheadScope(move |_| {
            let (it, p) = (it.clone(), p.clone());
            LazyColumn(
                modifier = Modifier.size_wh(100.dp(), 400.dp()),
                content = move |this: &mut LazyListScope| {
                    let p = p.clone();
                    this.items_list(
                        &it,
                        key = item_key(|v: &i32| *v),
                        item_content = move |v: &i32| {
                            let spec: Arc<dyn FiniteAnimationSpec<IntOffset>> =
                                Arc::new(tween(200, 0, Arc::new(LinearEasing)));
                            Box(modifier = Modifier
                                .animate_item(placement_spec = Some(spec))
                                .fill_max_width()
                                .height(50.dp())
                                .then(p.of(format!("item{v}"))));
                        },
                    );
                },
            );
        });
    }));
    assert_eq!(places.main("item3").y, 100.0);
    items.remove_at(0);
    rule.advance_time_by(112);
    // There in lookahead; on the way in the approach.
    assert_eq!(places.lookahead("item3").y, 50.0);
    let mid = places.main("item3").y;
    assert!(mid > 55.0 && mid < 95.0, "{mid}");
    rule.wait_for_idle();
    assert_eq!(places.main("item3").y, 50.0);
}

fn the_approach_stays_where_it_was_while_the_lookahead_scrolls_back() {
    // Compose's LazyLayoutScrollDeltaBetweenPasses: the last item shrinks (animate_content_size: at once in
    // lookahead, on its way in the approach), the lookahead's end comes into the viewport and it scrolls back 40 px;
    // the approach, its item not small yet, would jump: it takes that scroll back as a delta animated away, so it
    // stays where it was and glides there.
    let small = mutable_state_of(false);
    let places = Places::default();
    let state = LazyListState::new(6, 0);
    let (sm, p, s) = (small.clone(), places.clone(), state.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (sm, p, s) = (sm.clone(), p.clone(), s.clone());
        LookaheadScope(move |_| {
            let (sm, p) = (sm.clone(), p.clone());
            LazyColumn(
                modifier = Modifier.size_wh(100.dp(), 200.dp()),
                state = Some(s.clone()),
                content = move |this: &mut LazyListScope| {
                    let (sm, p) = (sm.clone(), p.clone());
                    this.items(10, item_content = move |i| {
                        if i == 9 {
                            let sm = sm.clone();
                            let spec: Arc<dyn FiniteAnimationSpec<IntSize>> =
                                Arc::new(TweenSpec::<IntSize>::new(200, 0, LinearEasing));
                            Box(
                                modifier = Modifier.animate_content_size(animation_spec = spec).then(p.of("son")),
                                content = move || {
                                    let h = if sm.get() { 10 } else { 50 };
                                    Box(modifier = Modifier.fill_max_width().height(h.dp()));
                                },
                            );
                        } else {
                            Box(modifier = Modifier.fill_max_width().height(50.dp()).then(p.of(format!("item{i}"))));
                        }
                    });
                },
            );
        });
    }));
    assert_eq!(places.main("item6").y, 0.0);
    small.set(true);
    rule.advance_frame();
    // The lookahead at its end, scrolled back: item 5 10 px out of view, item 6 at 40.
    assert_eq!(state.first_visible_item_index(), 5);
    assert_eq!(state.first_visible_item_scroll_offset(), 10);
    assert_eq!(places.lookahead("item6").y, 40.0);
    // The approach where it was.
    let at = places.main("item6").y;
    assert!(at < 5.0, "{at}");
    rule.advance_time_by(96);
    let mid = places.main("item6").y;
    assert!(mid > at && mid < 40.0, "{mid}");
    rule.wait_for_idle();
    assert_eq!(places.main("item6").y, 40.0);
    assert_eq!(places.main("son").y, 190.0);
}

fn a_lazy_grid_in_a_lookahead_scope_lays_out_and_animates_by_lookahead() {
    // The grid's lines: approaching cells (half their height) lay their line out shorter in the approach — a cell
    // after it placed there, one with animate_item at its lookahead position (Compose's: an item animation's target
    // in the approach is its lookahead offset); a line only the approach shows is kept composed by the next
    // lookahead (Compose's linesRetainedForLookahead), not composed again; animate_item glides to the lookahead
    // position; scrolling moves both passes alike.
    let approaching = mutable_state_of(true);
    let tall = mutable_state_of(false);
    let items = mutable_state_list_of((0..20).collect::<Vec<i32>>());
    let places = Places::default();
    let composed = Rc::new(RefCell::new(Vec::<i32>::new()));
    let state = LazyGridState::new(0, 0);
    let (a, t, it, p, c, s) =
        (approaching.clone(), tall.clone(), items.clone(), places.clone(), composed.clone(), state.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (a, t, it, p, c, s) = (a.clone(), t.clone(), it.clone(), p.clone(), c.clone(), s.clone());
        LookaheadScope(move |_| {
            let (a, t, it, p, c) = (a.clone(), t.clone(), it.clone(), p.clone(), c.clone());
            LazyVerticalGrid(
                columns = GridCells::Fixed(2),
                modifier = Modifier.size_wh(100.dp(), 200.dp()),
                state = Some(s.clone()),
                content = move |this: &mut LazyGridScope| {
                    let (a, t, p, c) = (a.clone(), t.clone(), p.clone(), c.clone());
                    this.items_list(
                        &it,
                        key = kompoz_foundation::lazy::item_key(|v: &i32| *v),
                        item_content = move |v: &i32| {
                            let (c, value) = (c.clone(), *v);
                            remember(move || c.borrow_mut().push(value));
                            let height = if *v == 2 && t.get() { 60 } else { 50 };
                            let approach = if *v < 2 { half_while(a.clone()) } else { Modifier };
                            let spec: Arc<dyn FiniteAnimationSpec<IntOffset>> =
                                Arc::new(tween(200, 0, Arc::new(LinearEasing)));
                            let animated = if *v < 2 || *v == 3 {
                                Modifier
                            } else {
                                Modifier.animate_item(placement_spec = Some(spec))
                            };
                            Box(modifier = animated
                                .then(approach)
                                .height(height.dp())
                                .test_tag(format!("cell{v}"))
                                .then(p.of(format!("cell{v}"))));
                        },
                    );
                },
            );
        });
    }));
    assert_eq!(places.main("cell3"), Offset::new(50.0, 25.0));
    assert_eq!(places.lookahead("cell3"), Offset::new(50.0, 50.0));
    assert_eq!(places.main("cell2"), Offset::new(0.0, 50.0));
    // Line 1 taller: the lookahead's four lines fill the 200 px, the approach's (line 0 half as tall) show a fifth;
    // that line's cells are composed once, kept by the lookahead passes after.
    tall.set(true);
    rule.wait_for_idle();
    tall.set(false);
    rule.wait_for_idle();
    tall.set(true);
    rule.wait_for_idle();
    // (laid out by the lookahead below its four lines: an animate_item's place in the approach is its lookahead one)
    assert_eq!(places.lookahead("cell8").y, 50.0 + 60.0 + 50.0 + 50.0);
    assert_eq!(places.main("cell8").y, 50.0 + 60.0 + 50.0 + 50.0);
    assert_eq!(composed.borrow().iter().filter(|v| **v == 8).count(), 1, "{:?}", composed.borrow());
    tall.set(false);
    rule.wait_for_idle();
    // (the approach done — a change of the approach alone — the lookahead keeps the line it retained placed until
    // its next pass, as Compose's does: path dependent by design, which the layout check — everything laid out again
    // from scratch, the retention then from the latest approach — can't agree with until a lookahead pass)
    rule.set_layout_check(false);
    approaching.set(false);
    rule.wait_for_idle();
    assert_eq!(places.main("cell3").y, 50.0);
    // Cell 0 removed: cell 2 goes to the first line's second cell, from where it was. (A lookahead pass again: what
    // it retained let go.)
    rule.set_layout_check(true);
    items.remove_at(0);
    rule.advance_time_by(112);
    assert_eq!(places.lookahead("cell2"), Offset::new(50.0, 0.0));
    let mid = places.main("cell2");
    assert!(mid.y > 5.0 && mid.y < 45.0 && mid.x > 5.0 && mid.x < 45.0, "{mid:?}");
    rule.wait_for_idle();
    assert_eq!(places.main("cell2"), Offset::new(50.0, 0.0));
    let s = state.clone();
    run_until_idle(&rule, async move {
        s.scroll_by(60.0).await;
    });
    rule.advance_frame();
    assert_eq!(state.first_visible_item_index(), 2);
    assert_eq!(places.main("cell5").y, places.lookahead("cell5").y);
    assert_eq!(places.main("cell5").y, 50.0 - 10.0);
}

fn a_staggered_grid_in_a_lookahead_scope_lays_its_lanes_out_in_both_passes() {
    // A lane's approaching item lays the lane out shorter in the approach; the other lane is as in lookahead.
    let approaching = mutable_state_of(true);
    let places = Places::default();
    let (a, p) = (approaching.clone(), places.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (a, p) = (a.clone(), p.clone());
        LookaheadScope(move |_| {
            let (a, p) = (a.clone(), p.clone());
            LazyVerticalStaggeredGrid(
                columns = StaggeredGridCells::Fixed(2),
                modifier = Modifier.size_wh(100.dp(), 300.dp()),
                content =
                    move |this: &mut LazyStaggeredGridScope| {
                        let (a, p) = (a.clone(), p.clone());
                        this.items(10, item_content = move |i| {
                        let approach = if i == 0 { half_while(a.clone()) } else { Modifier };
                        let h = if i % 2 == 0 { 60 } else { 40 };
                        Box(modifier = approach.fill_max_width().height(h.dp()).then(p.of(format!("hücre{i}"))));
                    });
                    },
            );
        });
    }));
    // Lanes: 0 (60) | 1 (40); 2 under 1 (at 40), 3 under 0 (at 60)… in lookahead; in the approach item 0 is 30 tall:
    // item 2 goes under it (at 30) and item 3 under item 1 (at 40).
    assert_eq!(places.lookahead("hücre2"), Offset::new(50.0, 40.0));
    assert_eq!(places.lookahead("hücre3"), Offset::new(0.0, 60.0));
    assert_eq!(places.main("hücre2"), Offset::new(0.0, 30.0));
    assert_eq!(places.main("hücre3"), Offset::new(50.0, 40.0));
    approaching.set(false);
    rule.wait_for_idle();
    for i in 0..6 {
        let tag = format!("hücre{i}");
        assert_eq!(places.main(&tag), places.lookahead(&tag), "{tag}");
    }
}

fn a_pager_in_a_lookahead_scope_scrolls_without_chasing() {
    // The pager measured in both passes; its pages placed as a motion frame of reference: a page's animate_bounds
    // doesn't animate the pager's scroll.
    let pager: Rc<RefCell<Option<PagerState>>> = Rc::default();
    let places = Places::default();
    let (pg, p) = (pager.clone(), places.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (pg, p) = (pg.clone(), p.clone());
        LookaheadScope(move |scope| {
            let state = remember_pager_state(page_count = || 4);
            *pg.borrow_mut() = Some(state.clone());
            let (p, scope) = (p.clone(), scope.clone());
            HorizontalPager(
                state = state,
                modifier = Modifier.size_wh(200.dp(), 100.dp()),
                page_content = move |page| {
                    Box(modifier =
                        Modifier.animate_bounds(scope.clone()).size(40.dp()).then(p.of(format!("sayfa{page}"))));
                },
            );
        });
    }));
    let state = pager.borrow().clone().unwrap();
    // (a page's content centered across: 30 down)
    assert_eq!(places.main("sayfa0"), Offset::new(0.0, 30.0));
    let s = state.clone();
    run_until_idle(&rule, async move {
        s.scroll_to_page(2).await;
    });
    rule.advance_frame();
    assert_eq!(state.current_page(), 2);
    assert_eq!(places.main("sayfa2"), Offset::new(0.0, 30.0));
    assert_eq!(places.lookahead("sayfa2"), Offset::new(0.0, 30.0));
    assert!(!rule.host().has_pending_work());
    // Part of a page scrolled: at once too.
    let s = state.clone();
    run_until_idle(&rule, async move {
        s.scroll_by(-50.0).await;
    });
    rule.advance_frame();
    assert_eq!(places.main("sayfa2").x, 50.0);
    assert_eq!(places.main("sayfa1").x, 50.0 - 200.0);
    assert!(!rule.host().has_pending_work());
}

fn main() {
    run_tests(&[
        (
            "a_lazy_column_in_a_lookahead_scope_lays_its_items_out_in_both_passes",
            a_lazy_column_in_a_lookahead_scope_lays_its_items_out_in_both_passes,
        ),
        (
            "animate_item_in_a_lookahead_scope_glides_to_its_lookahead_position",
            animate_item_in_a_lookahead_scope_glides_to_its_lookahead_position,
        ),
        (
            "the_approach_stays_where_it_was_while_the_lookahead_scrolls_back",
            the_approach_stays_where_it_was_while_the_lookahead_scrolls_back,
        ),
        (
            "a_lazy_grid_in_a_lookahead_scope_lays_out_and_animates_by_lookahead",
            a_lazy_grid_in_a_lookahead_scope_lays_out_and_animates_by_lookahead,
        ),
        (
            "a_staggered_grid_in_a_lookahead_scope_lays_its_lanes_out_in_both_passes",
            a_staggered_grid_in_a_lookahead_scope_lays_its_lanes_out_in_both_passes,
        ),
        ("a_pager_in_a_lookahead_scope_scrolls_without_chasing", a_pager_in_a_lookahead_scope_scrolls_without_chasing),
    ]);
}
