//! Pagers: dragging, flinging and snapping to pages, page sizes and snap positions, reversed and keyed pages, the
//! current, settled and target pages, animated scrolls, scrolls inside a page handing over to the pager and back,
//! right to left, overscroll, interactions — the C++ Kompoz's pager tests (survey G) and Compose's behaviour where
//! the two differ (the pages' nested scroll connection settling the pager with what an inner fling leaves is the C++
//! Kompoz's, kept). Numbers in px (density 1).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_animation::*;
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_foundation::gestures::LocalFuture;
use kompoz_foundation::pager::PageSizeCalculator;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::input::*;
use kompoz_ui::modifier::DelegatableNode;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::unit::Velocity;
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

fn near_by(a: f32, b: f32, by: f32) -> bool {
    (a - b).abs() <= by
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).fetch_semantics_node().unclipped_bounds_in_root()
}

fn x(rule: &KompozTestRule, tag: &str) -> f32 {
    bounds(rule, tag).left
}

fn has(rule: &KompozTestRule, tag: &str) -> bool {
    rule.on_node_with_tag(tag).exists()
}

fn shows(rule: &KompozTestRule, text: &str) -> bool {
    rule.on_node_with_text(text).exists()
}

type Scope = Rc<RefCell<Option<CoroutineScope>>>;

fn new_scope() -> Scope {
    Rc::new(RefCell::new(None))
}

fn launch(scope: &Scope, f: impl std::future::Future<Output = ()> + 'static) {
    scope.borrow().as_ref().unwrap().launch(f);
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

/// A page box filling its page, tagged `name` and its index.
fn page(name: &'static str) -> impl Fn(i32) + 'static {
    move |i| kompozable_block!({ Box(modifier = Modifier.fill_max_size().test_tag(format!("{name}{i}"))) })
}

/// Runs `f`, which is expected to panic, without the panic printed; its message.
fn panic_message(f: impl FnOnce()) -> String {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    std::panic::set_hook(hook);
    let payload = r.expect_err("it panics");
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default()
}

fn horizontal_pager_drags_snaps_and_animates() {
    // C++ 10494: a slow drag past half a page snaps to the next page, a short fast flick turns it too, a short slow
    // drag springs back; animated to the last page; it can't go past it.
    let pager: Rc<RefCell<Option<PagerState>>> = Rc::new(RefCell::new(None));
    let scope = new_scope();
    let (p, sc) = (pager.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = remember_pager_state(page_count = || 5);
            *p.borrow_mut() = Some(state.clone());
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            Column(
                content = move || {
                    HorizontalPager(
                        state = state.clone(),
                        modifier = Modifier.fill_max_width().height(200.dp()).test_tag("pager"),
                        page_content =
                            |page| Text(format!("Sayfa {page}"), modifier = Modifier.test_tag(format!("page{page}"))),
                    );
                    Text(format!("Şu an: {}", state.current_page()));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let pager = pager.borrow().clone().unwrap();
    assert!(shows(&rule, "Şu an: 0"));
    assert!(near(x(&rule, "page0"), 0.0) && !has(&rule, "page1") && !has(&rule, "page3"));
    // (Compose composes the pages in view only: C++ composed the next one too)
    slow_drag(&rule, at(300.0, 100.0), at(60.0, 100.0), 40);
    rule.wait_for_idle();
    assert!(shows(&rule, "Şu an: 1") && near(x(&rule, "page1"), 0.0));
    swipe(&rule, at(300.0, 100.0), at(220.0, 100.0), 3);
    rule.wait_for_idle();
    assert!(shows(&rule, "Şu an: 2"), "{pager:?}");
    slow_drag(&rule, at(300.0, 100.0), at(240.0, 100.0), 40);
    rule.wait_for_idle();
    assert!(shows(&rule, "Şu an: 2") && near(x(&rule, "page2"), 0.0));
    let s = pager.clone();
    launch(&scope, async move { s.animate_scroll_to_page(4).await });
    rule.wait_for_idle();
    assert!(shows(&rule, "Şu an: 4") && near(x(&rule, "page4"), 0.0));
    // (it can't go past the last page either way; back to 3)
    swipe(&rule, at(100.0, 100.0), at(380.0, 100.0), 3);
    rule.wait_for_idle();
    assert!(shows(&rule, "Şu an: 3"));
}

/// Two pages a view: (the room less the spacing) / 2.
struct HalfPage;

impl PageSizeCalculator for HalfPage {
    fn calculate_main_axis_page_size(&self, _density: &Density, available_space: i32, page_spacing: i32) -> i32 {
        (available_space - page_spacing) / 2
    }
}

fn pager_pages_of_a_fixed_size_snap_where_asked() {
    // C++ 10531: fixed-size pages at the start, the end or the middle; the first and last pages don't scroll past
    // their ends; scrolled to a page and a fraction of one; a page size of one's own.
    type States = Rc<RefCell<Vec<PagerState>>>;
    let states: States = Rc::new(RefCell::new(Vec::new()));
    let scope = new_scope();
    let (st, sc) = (states.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let a = remember_pager_state(page_count = || 6);
            let b = remember_pager_state(page_count = || 10);
            let c = remember_pager_state(initial_page = 9, page_count = || 10);
            *st.borrow_mut() = vec![a.clone(), b.clone(), c.clone()];
            let half: Rc<dyn PageSizeCalculator> = Rc::new(HalfPage);
            Column(
                content = move || {
                    // 200 pages in the middle of 300 of room (50 of padding each side), 10 apart.
                    HorizontalPager(
                        state = a.clone(),
                        modifier = Modifier.fill_max_width().height(100.dp()),
                        content_padding = PaddingValues::new(50.dp(), 0.dp(), 50.dp(), 0.dp()),
                        page_size = PageSize::Fixed(200.dp()),
                        page_spacing = 10.dp(),
                        snap_position = SnapPosition::Center,
                        page_content = page("orta"),
                    );
                    // 100 pages from the start: four in view.
                    HorizontalPager(
                        state = b.clone(),
                        modifier = Modifier.fill_max_width().height(100.dp()),
                        page_size = PageSize::Fixed(100.dp()),
                        page_content = page("baş"),
                    );
                    // From page 9, which can't come to the start: as far as the pages go, settled there.
                    HorizontalPager(
                        state = c.clone(),
                        modifier = Modifier.fill_max_width().height(100.dp()),
                        page_size = PageSize::Fixed(100.dp()),
                        page_content = page("son"),
                    );
                    // 100 pages the padding lets come to the middle (from 150).
                    HorizontalPager(
                        state = remember_pager_state(initial_page = 5, page_count = || 10),
                        modifier = Modifier.fill_max_width().height(100.dp()),
                        content_padding = PaddingValues::new(150.dp(), 0.dp(), 150.dp(), 0.dp()),
                        page_size = PageSize::Fixed(100.dp()),
                        page_content = page("dar"),
                    );
                    // Two pages a view, as a page size of its own says.
                    HorizontalPager(
                        state = remember_pager_state(page_count = || 4),
                        modifier = Modifier.fill_max_width().height(100.dp()),
                        page_size = PageSize::Custom(half.clone()),
                        page_spacing = 20.dp(),
                        page_content = page("yarım"),
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let (centered, started, last) = {
        let s = states.borrow();
        (s[0].clone(), s[1].clone(), s[2].clone())
    };
    // The pages in view composed (C++ composed the next one too: Compose prefetches it as it scrolls).
    assert!(near(x(&rule, "baş3"), 300.0) && !has(&rule, "baş4"));
    assert!(near(x(&rule, "dar5"), 150.0) && near(x(&rule, "dar3"), -50.0) && !has(&rule, "dar1"));
    // The first page can't come to the middle: the pages begin at the start padding.
    assert!(
        near(x(&rule, "orta0"), 50.0) && near(x(&rule, "orta1"), 260.0) && near(bounds(&rule, "orta0").width(), 200.0)
    );
    assert!(centered.current_page() == 0 && !centered.can_scroll_backward() && centered.can_scroll_forward());
    assert!(started.settled_page() == 0 && last.current_page() == 6 && last.settled_page() == 6);
    assert!(near(x(&rule, "son9"), 300.0));
    let s = centered.clone();
    launch(&scope, async move { s.scroll_to_page(2).await });
    rule.wait_for_idle();
    // (in the middle, its neighbours peeking in)
    assert!(near(x(&rule, "orta2"), 100.0) && near(x(&rule, "orta1"), -110.0) && near(x(&rule, "orta3"), 310.0));
    assert!(centered.current_page() == 2 && centered.settled_page() == 2);
    assert!(near_by(centered.get_offset_distance_in_pages(3), 1.0, 0.01));
    // (the last one's end at the end padding, no further)
    let s = centered.clone();
    launch(&scope, async move { s.scroll_to_page(5).await });
    rule.wait_for_idle();
    assert!(near(x(&rule, "orta5"), 150.0) && centered.current_page() == 5);
    assert!(!centered.can_scroll_forward() && centered.can_scroll_backward());
    assert!(near_by(centered.get_offset_distance_in_pages(5), 50.0 / 210.0, 0.01));
    // Half a page on from 1: Compose's fraction +0.5 (C++ told it the other sign).
    let s = centered.clone();
    launch(&scope, async move { named_args!(s.scroll_to_page(1, page_offset_fraction = 0.5)).await });
    rule.wait_for_idle();
    assert!(near(x(&rule, "orta1"), 100.0 - 105.0) && near_by(centered.current_page_offset_fraction(), 0.5, 0.01));
    let s = started.clone();
    launch(&scope, async move { s.scroll_to_page(3).await });
    rule.wait_for_idle();
    assert!(near(x(&rule, "baş6"), 300.0) && !has(&rule, "baş0"));
    // (the last four pages fill the view: page 6 is as far as it goes)
    let s = started.clone();
    launch(&scope, async move { s.scroll_to_page(9).await });
    rule.wait_for_idle();
    assert!(near(x(&rule, "baş9"), 300.0) && started.current_page() == 6 && !started.can_scroll_forward());
    assert!(near(bounds(&rule, "yarım0").width(), 190.0) && near(x(&rule, "yarım1"), 210.0));
}

fn pager_measured_without_end_shows_its_first_pages() {
    // C++ 10587 (adapted): measured unbounded along it — in a sideways scrolling row, in a scrolling column —
    // Compose's pager, a scrollable container, refuses (C++ laid its first pages out).
    let horizontal = panic_message(|| {
        KompozTestRule::with_viewport(
            kompozable_lambda!(|| {
                Row(
                    modifier = Modifier.horizontal_scroll(&remember_scroll_state()),
                    content = || {
                        HorizontalPager(
                            state = remember_pager_state(page_count = || 5),
                            modifier = Modifier.height(100.dp()),
                            page_size = PageSize::Fixed(120.dp()),
                            page_content = page("sayfa"),
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        );
    });
    assert!(horizontal.starts_with("Horizontally scrollable component was measured with an infinity"), "{horizontal}");
    let vertical = panic_message(|| {
        KompozTestRule::with_viewport(
            kompozable_lambda!(|| {
                Column(
                    modifier = Modifier.vertical_scroll(&remember_scroll_state()),
                    content = || {
                        VerticalPager(
                            state = remember_pager_state(page_count = || 5),
                            modifier = Modifier.fill_max_width(),
                            page_content = |i| {
                                Box(modifier = Modifier.fill_max_width().height(50.dp()).test_tag(format!("dikey{i}")))
                            },
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        );
    });
    assert!(vertical.starts_with("Vertically scrollable component was measured with an infinity"), "{vertical}");
    // A content padding wider than the pager: no page in view, the first one composed (at least one is).
    let padded = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            HorizontalPager(
                state = remember_pager_state(page_count = || 5),
                modifier = Modifier.fill_max_width().height(100.dp()),
                content_padding = PaddingValues::new(300.dp(), 0.dp(), 300.dp(), 0.dp()),
                page_size = PageSize::Fixed(120.dp()),
                snap_position = SnapPosition::Center,
                page_content = page("dolgu"),
            );
        }),
        IntSize::new(400, 800),
    );
    assert!(has(&padded, "dolgu0"));
}

fn pager_composes_the_pages_a_bigger_view_shows() {
    // C++ 10618: the same current page, four more in view when the view grows.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            HorizontalPager(
                state = remember_pager_state(page_count = || 10),
                modifier = Modifier.fill_max_width().height(100.dp()),
                page_size = PageSize::Fixed(100.dp()),
                page_content = page("sayfa"),
            );
        }),
        IntSize::new(400, 800),
    );
    assert!(has(&rule, "sayfa3") && !has(&rule, "sayfa6"));
    rule.host().set_viewport_size(IntSize::new(800, 800));
    rule.wait_for_idle();
    assert!(near(x(&rule, "sayfa7"), 700.0));
}

/// A page keeping a count of its taps (its own state, kept by its key).
#[kompozable]
fn Keyed(name: String) {
    let taps = remember(|| mutable_state_of(0));
    let t = taps.clone();
    Box(
        modifier = Modifier.fill_max_size().test_tag(format!("sayfa {name}")).clickable(move || t.set(t.get() + 1)),
        content = move || Text(format!("{name} {}", taps.get())),
    );
}

fn pagers_laid_out_from_the_end_and_keyed_pages() {
    // C++ 10629: from the end, the first page at the right (inside the end padding), the next ones to its left; a
    // vertical one's at the bottom; the finger the other way; keyed pages keep the current page and its state as
    // pages come or go before it.
    let names = mutable_state_of(vec!["A".to_string(), "B".into(), "C".into(), "D".into()]);
    let states: Rc<RefCell<Vec<PagerState>>> = Rc::new(RefCell::new(Vec::new()));
    let (n, st) = (names.clone(), states.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let a = remember_pager_state(page_count = || 10);
            let list = n.get();
            let count = list.len() as i32;
            let b = remember_pager_state(initial_page = 2, page_count = move || count);
            *st.borrow_mut() = vec![a.clone(), b.clone()];
            Column(
                content = move || {
                    HorizontalPager(
                        state = a.clone(),
                        modifier = Modifier.fill_max_width().height(100.dp()),
                        content_padding = PaddingValues::new(20.dp(), 0.dp(), 40.dp(), 0.dp()),
                        page_size = PageSize::Fixed(100.dp()),
                        reverse_layout = true,
                        page_content = page("ters"),
                    );
                    VerticalPager(
                        state = remember_pager_state(page_count = || 3),
                        modifier = Modifier.fill_max_width().height(100.dp()),
                        reverse_layout = true,
                        page_content = page("dikey"),
                    );
                    let keys = list.clone();
                    let key: Rc<dyn Fn(i32) -> LazyKey> = Rc::new(move |i| LazyKey::new(keys[i as usize].clone()));
                    let contents = list.clone();
                    HorizontalPager(
                        state = b.clone(),
                        modifier = Modifier.fill_max_width().height(100.dp()),
                        key = Some(key),
                        page_content = move |i| Keyed(contents[i as usize].clone()),
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let (reversed, keyed) = {
        let s = states.borrow();
        (s[0].clone(), s[1].clone())
    };
    assert!(near(x(&rule, "ters0"), 260.0) && near(x(&rule, "ters1"), 160.0) && near(x(&rule, "ters2"), 60.0));
    // (the next page above, out of the pager: Compose composes the pages in view only)
    assert!(near(bounds(&rule, "dikey0").top, 100.0) && !has(&rule, "dikey1"));
    // (dragged right, slowly, 3 pages and a bit: it goes on)
    slow_drag(&rule, at(50.0, 50.0), at(380.0, 50.0), 100);
    rule.wait_for_idle();
    assert!(
        reversed.current_page() == 3 && near(x(&rule, "ters3"), 260.0) && near(x(&rule, "ters4"), 160.0),
        "{reversed:?}"
    );
    // (flicked right: on to the next one)
    swipe(&rule, at(50.0, 50.0), at(150.0, 50.0), 3);
    rule.wait_for_idle();
    assert_eq!(reversed.current_page(), 4);
    let p = bounds(&rule, "sayfa C");
    tap(&rule, at(p.left + 50.0, p.top + 50.0));
    rule.wait_for_idle();
    assert!(shows(&rule, "C 1") && keyed.current_page() == 2);
    // (a page put in before: the current page one further on, with its state)
    names.set(vec!["Z".into(), "A".into(), "B".into(), "C".into(), "D".into()]);
    rule.wait_for_idle();
    assert!(keyed.current_page() == 3 && keyed.settled_page() == 3, "{keyed:?}");
    assert!(near(x(&rule, "sayfa C"), 0.0) && shows(&rule, "C 1"));
    // (pages taken away before it)
    names.set(vec!["C".into(), "D".into()]);
    rule.wait_for_idle();
    assert!(
        keyed.current_page() == 0 && near(x(&rule, "sayfa C"), 0.0) && shows(&rule, "C 1"),
        "{keyed:?} {} {}",
        x(&rule, "sayfa C"),
        shows(&rule, "C 1")
    );
}

fn pager_flings_turn_as_many_pages_as_allowed() {
    // C++ 10677: a hard fling turns as many pages as the snap distance allows, heading there before it gets there;
    // a slow drag past a quarter's threshold turns the page either way; a short one goes back as slowly as its
    // animation says; animated scrolls tell their target, one taking over another's; a finger stops it.
    let states: Rc<RefCell<Vec<PagerState>>> = Rc::new(RefCell::new(Vec::new()));
    let scope = new_scope();
    let (st, sc) = (states.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let a = remember_pager_state(page_count = || 10);
            let b = remember_pager_state(initial_page = 1, page_count = || 10);
            *st.borrow_mut() = vec![a.clone(), b.clone()];
            Column(
                content = move || {
                    let far_fling = named_args!(
                        PagerDefaults.fling_behavior(&a, pager_snap_distance = PagerSnapDistance::at_most(3))
                    );
                    HorizontalPager(
                        state = a.clone(),
                        modifier = Modifier.fill_max_width().height(100.dp()),
                        fling_behavior = Some(far_fling),
                        page_content = |_| Box(modifier = Modifier.fill_max_size()),
                    );
                    let near_fling = named_args!(PagerDefaults.fling_behavior(
                        &b,
                        snap_positional_threshold = 0.25,
                        snap_animation_spec = {
                            let spec: TweenSpec<f32> = tween(duration_millis = 1500, easing = Arc::new(LinearEasing));
                            Arc::new(spec)
                        }
                    ));
                    HorizontalPager(
                        state = b.clone(),
                        modifier = Modifier.fill_max_width().height(100.dp()),
                        fling_behavior = Some(near_fling),
                        page_content = |_| Box(modifier = Modifier.fill_max_size()),
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let (far, near_) = {
        let s = states.borrow();
        (s[0].clone(), s[1].clone())
    };
    // A hard fling: three pages, no more; heading there before it gets there.
    swipe(&rule, at(390.0, 50.0), at(10.0, 50.0), 3);
    rule.advance_frame();
    // (Compose's drag and fling are two scrolls: the fling's start settles the pager on the page current when let go
    // — past half a page, 1; C++ kept 0)
    assert!(far.target_page() == 3 && far.settled_page() == 1 && far.is_scroll_in_progress(), "{far:?}");
    rule.wait_for_idle();
    assert!(
        far.current_page() == 3 && far.settled_page() == 3 && far.target_page() == 3 && !far.is_scroll_in_progress()
    );
    // A gentle one: one page.
    swipe(&rule, at(300.0, 50.0), at(240.0, 50.0), 3);
    rule.wait_for_idle();
    assert_eq!(far.current_page(), 4);
    // A slow drag a third of a page: past a threshold of a quarter, the page turns, either way.
    slow_drag(&rule, at(300.0, 150.0), at(170.0, 150.0), 40);
    rule.wait_for_idle();
    assert_eq!(near_.current_page(), 2);
    slow_drag(&rule, at(100.0, 150.0), at(230.0, 150.0), 40);
    rule.wait_for_idle();
    assert_eq!(near_.current_page(), 1);
    // A fifth: back, as slowly as its animation says.
    slow_drag(&rule, at(300.0, 150.0), at(220.0, 150.0), 40);
    rule.advance_time_by(800);
    assert!(near_.is_scroll_in_progress());
    rule.wait_for_idle();
    assert!(near_.current_page() == 1 && near_.settled_page() == 1);
    // animate_scroll_to_page: its target known on the way.
    let s = far.clone();
    launch(&scope, async move { s.animate_scroll_to_page(7).await });
    rule.advance_frame();
    rule.advance_frame();
    assert!(far.target_page() == 7 && far.settled_page() == 4);
    rule.wait_for_idle();
    assert!(far.current_page() == 7 && far.settled_page() == 7);
    // (as the animation ends)
    let seen = Rc::new(Cell::new(-1));
    let (s, se) = (far.clone(), seen.clone());
    launch(&scope, async move {
        named_args!(s.animate_scroll_to_page(5, page_offset_fraction = 0.25)).await;
        se.set(s.settled_page());
    });
    rule.wait_for_idle();
    assert!(
        near_by(far.get_offset_distance_in_pages(5), -0.25, 0.01) && far.settled_page() == 5 && seen.get() == 5,
        "{far:?} {} {}",
        far.settled_page(),
        seen.get()
    );
    // Caught on the way by a finger: neither settled where it was going nor heading there.
    let s = far.clone();
    launch(&scope, async move { s.animate_scroll_to_page(8).await });
    for _ in 0..3 {
        rule.advance_frame();
    }
    // (another animation on the way takes over its target, and doesn't let it settle)
    let s = far.clone();
    launch(&scope, async move { s.animate_scroll_to_page(9).await });
    rule.advance_frame();
    assert!(far.target_page() == 9 && far.settled_page() == 5, "{} {}", far.target_page(), far.settled_page());
    for _ in 0..7 {
        rule.advance_frame();
    }
    let h = rule.host();
    h.pointer_down(at(100.0, 50.0), -1, 0, PointerType::Touch);
    rule.advance_frame();
    for x in [200.0, 300.0] {
        h.pointer_move(at(x, 50.0), -1, 0);
        rule.advance_frame();
    }
    // (not settled where it was going; Compose keeps a cancelled animation's target until the scroll after it ends —
    // its `scroll` resets it only when it returns —, where C++ told the current page at once)
    assert!(far.settled_page() == 5 && far.target_page() == 9 && far.current_page() < 9, "{far:?}");
    h.pointer_up(at(300.0, 50.0), -1, 0);
    rule.wait_for_idle();
    assert!(far.settled_page() == far.current_page() && far.target_page() == far.current_page(), "{far:?}");
}

fn pager_pages_line_up_across_it() {
    // C++ 10749: pages as big as they want (at the start of their room along the pager), in the middle across by
    // default, or where the alignment says.
    let small = |name: &'static str| {
        move |i: i32| kompozable_block!({ Box(modifier = Modifier.size(50.dp()).test_tag(format!("{name}{i}"))) })
    };
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            Column(
                content = move || {
                    HorizontalPager(
                        state = remember_pager_state(page_count = || 3),
                        modifier = Modifier.fill_max_width().height(200.dp()),
                        page_content = small("orta"),
                    );
                    HorizontalPager(
                        state = remember_pager_state(page_count = || 3),
                        modifier = Modifier.fill_max_width().height(200.dp()),
                        vertical_alignment = Alignment::Top,
                        page_content = small("üst"),
                    );
                    VerticalPager(
                        state = remember_pager_state(page_count = || 3),
                        modifier = Modifier.fill_max_width().height(200.dp()),
                        horizontal_alignment = Alignment::End,
                        page_content = small("sağ"),
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    assert!(
        near(x(&rule, "orta0"), 0.0)
            && near(bounds(&rule, "orta0").top, 75.0)
            && near(bounds(&rule, "orta0").width(), 50.0)
    );
    assert!(
        near(bounds(&rule, "üst0").top, 200.0)
            && near(x(&rule, "sağ0"), 350.0)
            && near(bounds(&rule, "sağ0").top, 400.0)
    );
    // (the next pages, scrolled to: in the next room)
    let r = bounds(&rule, "orta0");
    slow_drag(&rule, at(300.0, r.top), at(50.0, r.top), 30);
    rule.wait_for_idle();
    assert!(near(x(&rule, "orta1"), 0.0));
}

fn tap_inside_a_dragged_pager_does_not_fire() {
    // C++ 12303: a drag on the pager isn't a tap on its page; a tap is.
    let taps = Rc::new(Cell::new(0));
    let t = taps.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let t = t.clone();
            HorizontalPager(
                state = remember_pager_state(page_count = || 3),
                modifier = Modifier.fill_max_width().height(200.dp()),
                page_content = move |_| {
                    let t = t.clone();
                    Box(modifier = Modifier.fill_max_size().pointer_input((), move |s| {
                        let t = t.clone();
                        async move { s.detect_tap_gestures().on_tap(move |_| t.set(t.get() + 1)).await }
                    }));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    swipe(&rule, at(300.0, 100.0), at(100.0, 100.0), 20);
    rule.wait_for_idle();
    assert_eq!(taps.get(), 0);
    tap(&rule, at(200.0, 100.0));
    rule.wait_for_idle();
    assert_eq!(taps.get(), 1);
}

/// A pager of 3 pages, each a row of 800 scrolling sideways (`rows[page]`), `reverse_layout`.
fn rows_pager(rows: &[ScrollState], reverse_layout: bool) -> (KompozTestRule, PagerState) {
    let pager: Rc<RefCell<Option<PagerState>>> = Rc::new(RefCell::new(None));
    let (p, r) = (pager.clone(), rows.to_vec());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = remember_pager_state(page_count = || 3);
            *p.borrow_mut() = Some(state.clone());
            let r = r.clone();
            HorizontalPager(
                state = state,
                modifier = Modifier.fill_max_width().height(200.dp()),
                reverse_layout = reverse_layout,
                page_content = move |page| {
                    Row(
                        modifier = Modifier.fill_max_size().horizontal_scroll(&r[page as usize]),
                        content = || Box(modifier = Modifier.width(800.dp()).height(10.dp())),
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let state = pager.borrow().clone().unwrap();
    (rule, state)
}

fn pager_takes_over_from_a_row_inside_its_page() {
    // C++ 14590: dragged left 700: past the slop the row scrolls its 400, the pager the rest; going back moves the
    // pager back to its page before the row scrolls again (the pages' nested scroll connection); a drag the row
    // can't take all of turns the page past its threshold. (Compose takes the drag after the touch slop, not with
    // it: the numbers are Compose's.)
    let rows = [ScrollState::new(0), ScrollState::new(0), ScrollState::new(0)];
    let (rule, pager) = rows_pager(&rows, false);
    let h = rule.host();
    h.pointer_down(at(390.0, 100.0), -1, 0, PointerType::Touch);
    for i in 1..=20 {
        rule.advance_frame();
        h.pointer_move(at(390.0 - 35.0 * i as f32, 100.0), -1, 0);
    }
    // (the last move's scroll applied in the next frame)
    rule.advance_frame();
    let pager_part = 400.0 * pager.current_page() as f32 + pager.current_page_offset_fraction() * 400.0;
    assert!(rows[0].value() == 400 && pager.current_page() == 1, "{} {pager:?}", rows[0].value());
    // (all but the touch slop, 8)
    assert!(near_by(700.0 - 400.0 - pager_part, 8.0, 1.0), "{pager_part}");
    for i in 1..=10 {
        rule.advance_frame();
        h.pointer_move(at(-310.0 + 30.0 * i as f32, 100.0), -1, 0);
    }
    rule.advance_frame();
    // (300 back: the pager's part first, back to page 0, then the row)
    assert!(near_by(pager.current_page_offset_fraction(), 0.0, 0.001) && pager.current_page() == 0, "{pager:?}");
    assert!(rows[0].value() < 400 && rows[0].value() > 300, "{}", rows[0].value());
    rule.advance_time_by(200);
    h.pointer_up(at(-10.0, 100.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(pager.current_page(), 0);
    // The row scrolls what is left of it, then the pager, pulled 0.7 of a page: it snaps to the next page.
    let left = 400 - rows[0].value();
    slow_drag(&rule, at(390.0, 100.0), at(390.0 - 300.0 - left as f32 - 40.0, 100.0), 30);
    rule.wait_for_idle();
    assert!(
        pager.current_page() == 1 && pager.current_page_offset_fraction() == 0.0 && rows[0].value() == 400,
        "{pager:?}"
    );
    // (the neighbour pages don't show outside the pager: clipped along it, 30 more across for shadows)
    let canvas = rule.draw();
    assert!(
        canvas
            .clips
            .iter()
            .any(|c| near(c.left, 0.0) && near(c.top, -30.0) && near(c.right, 400.0) && near(c.bottom, 230.0))
    );
}

fn reversed_pager_goes_back_to_its_page_before_a_row_in_it_scrolls() {
    // C++ 14626: reversed, the finger the other way: right, the row at its start, the pager goes on; back left, the
    // pager back to its page first, then the row scrolls.
    let rows = [ScrollState::new(0), ScrollState::new(0), ScrollState::new(0)];
    let (rule, pager) = rows_pager(&rows, true);
    let h = rule.host();
    h.pointer_down(at(10.0, 100.0), -1, 0, PointerType::Touch);
    for i in 1..=10 {
        rule.advance_frame();
        h.pointer_move(at(10.0 + 20.0 * i as f32, 100.0), -1, 0);
    }
    assert!(pager.current_page_offset_fraction() > 0.3 && rows[0].value() == 0, "{pager:?}");
    for i in 1..=10 {
        rule.advance_frame();
        h.pointer_move(at(210.0 - 30.0 * i as f32, 100.0), -1, 0);
    }
    assert!(near_by(pager.current_page_offset_fraction(), 0.0, 0.01) && pager.current_page() == 0, "{pager:?}");
    assert!(rows[0].value() > 50, "{}", rows[0].value());
    h.pointer_up(at(-90.0, 100.0), -1, 0);
    rule.wait_for_idle();
}

/// A nested scroll connection telling the post flings it hears.
struct OuterFling(Rc<Cell<f32>>);

impl NestedScrollConnection for OuterFling {
    fn on_post_fling(&self, _consumed: Velocity, available: Velocity) -> FlingFuture<'_> {
        let seen = self.0.clone();
        Box::pin(async move {
            seen.set(seen.get().max(available.x.abs()));
            Velocity::Zero
        })
    }
}

fn pager_turns_the_page_with_a_flick_past_a_row_on_it() {
    // C++ 14653 (the C++ Kompoz's behaviour, kept): a quick flick towards the next page past a row at its end: the
    // pager takes the drag past the slop, and what the row's fling leaves turns the page; none of it goes on to the
    // pager's parents, also when the pager doesn't scroll by itself (it stays then). Right to left too.
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        for user_scroll in [true, false] {
            let pages = PagerState(0, 0.0, || 3);
            let row = LazyListState::new(0, 0);
            let outer_fling = Rc::new(Cell::new(0.0f32));
            let outer: Rc<dyn NestedScrollConnection> = Rc::new(OuterFling(outer_fling.clone()));
            let scope = new_scope();
            let (p, r, o, sc) = (pages.clone(), row.clone(), outer.clone(), scope.clone());
            let rule = KompozTestRule::with_viewport(
                kompozable_lambda!(move || {
                    *sc.borrow_mut() = Some(remember_coroutine_scope());
                    let (p, r, o) = (p.clone(), r.clone(), o.clone());
                    KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], move || {
                        let (p, r) = (p.clone(), r.clone());
                        Box(
                            modifier = Modifier.nested_scroll(o.clone(), None),
                            content = move || {
                                let r = r.clone();
                                HorizontalPager(
                                    state = p.clone(),
                                    modifier = Modifier.width(200.dp()).height(60.dp()),
                                    user_scroll_enabled = user_scroll,
                                    page_content = move |page| {
                                        if page == 0 {
                                            LazyRow(
                                                modifier = Modifier.fill_max_size(),
                                                state = Some(r.clone()),
                                                content = |this: &mut LazyListScope| {
                                                    this.items(3, |_| {
                                                        Box(modifier = Modifier.size_wh(100.dp(), 60.dp()))
                                                    });
                                                },
                                            );
                                        }
                                    },
                                );
                            },
                        );
                    });
                }),
                IntSize::new(200, 400),
            );
            // At its end (under a pager that doesn't scroll by itself the row flings into its end from its start).
            if user_scroll {
                let s = row.clone();
                launch(&scope, async move { s.scroll_to_item(2).await });
            }
            rule.wait_for_idle();
            let rtl = direction == LayoutDirection::Rtl;
            let (x0, to_next) = if rtl { (50.0, 25.0) } else { (150.0, -25.0) };
            let h = rule.host();
            h.pointer_down(at(x0, 30.0), -1, 0, PointerType::Touch);
            for i in 1..=4 {
                rule.advance_frame();
                h.pointer_move(at(x0 + to_next * i as f32, 30.0), -1, 0);
            }
            let fraction = pages.current_page_offset_fraction();
            if user_scroll {
                // (past the slop: less than half a page)
                assert!(fraction > 0.2 && fraction < 0.5, "{direction:?}: {fraction}");
            } else {
                assert_eq!(fraction, 0.0);
            }
            h.pointer_up(at(x0 + to_next * 4.0, 30.0), -1, 0);
            rule.wait_for_idle();
            assert!(
                pages.current_page() == if user_scroll { 1 } else { 0 }
                    && near_by(pages.current_page_offset_fraction(), 0.0, 0.01),
                "{direction:?} {user_scroll}: {pages:?}"
            );
            assert!(
                row.first_visible_item_index() == 1 && outer_fling.get() == 0.0,
                "{direction:?} {user_scroll}: {row:?} {}",
                outer_fling.get()
            );
        }
    }
}

fn vertical_pager_snaps_to_pages() {
    // C++ 14742: a slow drag past half a page snaps to the next page; a flick turns the page.
    let pager: Rc<RefCell<Option<PagerState>>> = Rc::new(RefCell::new(None));
    let p = pager.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = remember_pager_state(page_count = || 4);
            *p.borrow_mut() = Some(state.clone());
            VerticalPager(
                state = state,
                modifier = Modifier.fill_max_width().height(300.dp()),
                page_content =
                    |page| Text(format!("Sayfa {page}"), modifier = Modifier.test_tag(format!("page{page}"))),
            );
        }),
        IntSize::new(400, 800),
    );
    let pager = pager.borrow().clone().unwrap();
    slow_drag(&rule, at(200.0, 280.0), at(200.0, 80.0), 20);
    rule.wait_for_idle();
    assert!(pager.current_page() == 1 && near(bounds(&rule, "page1").top, 0.0));
    swipe(&rule, at(200.0, 250.0), at(200.0, 200.0), 3);
    rule.wait_for_idle();
    assert_eq!(pager.current_page(), 2);
}

fn rtl_pager_turns_with_the_finger_the_other_way() {
    // C++ 17366: right to left, a quick swipe to the right turns to the next page; to the left, back.
    let state = PagerState(0, 0.0, || 5);
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], move || {
                HorizontalPager(
                    state = s.clone(),
                    modifier = Modifier.fill_max_width().height(200.dp()).test_tag("sayfalar"),
                    page_content = page("s"),
                );
            });
        }),
        IntSize::new(300, 400),
    );
    let area = bounds(&rule, "sayfalar");
    swipe(&rule, at(area.left + 40.0, 100.0), at(area.left + 200.0, 100.0), 6);
    rule.wait_for_idle();
    assert!(state.current_page() == 1 && near(x(&rule, "s1"), area.left), "{state:?}");
    swipe(&rule, at(area.left + 260.0, 100.0), at(area.left + 100.0, 100.0), 6);
    rule.wait_for_idle();
    assert_eq!(state.current_page(), 0);
}

// ---- An effect of one's own ----------------------------------------------------------------------------------

/// What a [`RecordingOverscroll`] heard: what the scrolls left (from the user), each fling's velocity.
#[derive(Default)]
struct Recorded {
    pulled: Cell<f32>,
    flings: RefCell<Vec<f32>>,
}

/// C++'s RecordingOverscroll: adds up what the scrolls left, hears of the flings.
struct RecordingOverscroll {
    recorded: Rc<Recorded>,
    node: DelegatableNode,
}

impl PartialEq for RecordingOverscroll {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.recorded, &other.recorded)
    }
}

/// Its node: nothing of its own.
struct NoNode;

impl ModifierNode for NoNode {}

impl OverscrollEffect for RecordingOverscroll {
    fn apply_to_scroll(
        &self,
        delta: Offset,
        source: NestedScrollSource,
        perform_scroll: &dyn Fn(Offset) -> Offset,
    ) -> Offset {
        let used = perform_scroll(delta);
        if source == NestedScrollSource::UserInput {
            let r = &self.recorded;
            r.pulled.set(r.pulled.get() + delta.x - used.x + delta.y - used.y);
        }
        used
    }

    fn apply_to_fling(&self, velocity: Velocity, perform_fling: PerformFling) -> LocalFuture<'_, ()> {
        let r = self.recorded.clone();
        Box::pin(async move {
            r.flings.borrow_mut().push(velocity.x + velocity.y);
            perform_fling(velocity).await;
            r.pulled.set(0.0);
        })
    }

    fn is_in_progress(&self) -> bool {
        self.recorded.pulled.get() != 0.0
    }

    fn node(&self) -> DelegatableNode {
        self.node.clone()
    }
}

fn pager_overscroll_effect_hears_what_it_leaves() {
    // C++ 4303's pager part: a pager's own effect hears the drag past its first page, and the release's fling.
    let recorded = Rc::new(Recorded::default());
    let effect: Rc<dyn OverscrollEffect> =
        Rc::new(RecordingOverscroll { recorded: recorded.clone(), node: DelegatableNode::new(NoNode) });
    let pager: Rc<RefCell<Option<PagerState>>> = Rc::new(RefCell::new(None));
    let (p, e) = (pager.clone(), effect.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = remember_pager_state(page_count = || 3);
            *p.borrow_mut() = Some(state.clone());
            HorizontalPager(
                state = state,
                modifier = Modifier.fill_max_width().height(200.dp()).test_tag("sayfa"),
                overscroll_effect = Some(Some(e.clone())),
                page_content = |_| Box(modifier = Modifier.fill_max_size()),
            );
        }),
        IntSize::new(800, 800),
    );
    let pager = pager.borrow().clone().unwrap();
    let center = bounds(&rule, "sayfa").center();
    let h = rule.host();
    h.pointer_down(center, -1, 0, PointerType::Touch);
    for i in 1..=10 {
        rule.advance_frame();
        h.pointer_move(center + at(200.0 * i as f32 / 10.0, 0.0), -1, 0);
    }
    assert!(recorded.pulled.get() > 150.0 && pager.current_page() == 0, "{}", recorded.pulled.get());
    h.pointer_up(center + at(200.0, 0.0), -1, 0);
    rule.wait_for_idle();
    assert!(recorded.flings.borrow().len() == 1 && recorded.pulled.get() == 0.0 && pager.current_page() == 0);
}

#[kompozable]
fn LogInteractions(source: InteractionSource, log: Rc<RefCell<Vec<&'static str>>>) {
    let flow = source.interactions();
    kompoz_coroutines::LaunchedEffect((), async move {
        flow.collect(move |i| log.borrow_mut().push(interaction_name(&*i))).await;
    });
}

fn interaction_name(i: &dyn Interaction) -> &'static str {
    if i.is::<DragInteraction::Start>() {
        "DragStart"
    } else if i.is::<DragInteraction::Stop>() {
        "DragStop"
    } else if i.is::<DragInteraction::Cancel>() {
        "DragCancel"
    } else {
        "?"
    }
}

fn lazy_list_and_pager_states_hear_the_users_drags() {
    // C++ 23367's lazy list and pager parts (its Button a clickable box): a lazy list's and a pager's sources hear
    // their drags, read as a state where composed.
    let list = LazyListState::new(0, 0);
    let pager: Rc<RefCell<Option<PagerState>>> = Rc::new(RefCell::new(None));
    let list_log = Rc::new(RefCell::new(Vec::new()));
    let (l, p, ll) = (list.clone(), pager.clone(), list_log.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let state = remember_pager_state(page_count = || 3);
        *p.borrow_mut() = Some(state.clone());
        LogInteractions(l.interaction_source(), ll.clone());
        let list_dragged = l.interaction_source().collect_is_dragged_as_state().get();
        let pager_dragged = state.interaction_source().collect_is_dragged_as_state().get();
        let l = l.clone();
        Column(
            content = move || {
                Text(format!("liste {list_dragged} sayfa {pager_dragged}"));
                LazyColumn(
                    modifier = Modifier.fill_max_width().height(300.dp()).test_tag("liste"),
                    state = Some(l.clone()),
                    content = |this: &mut LazyListScope| {
                        this.item(
                            content = || {
                                Box(modifier =
                                    Modifier.fill_max_width().height(48.dp()).test_tag("öğe").clickable(|| {}));
                            },
                        );
                        this.items(50, |i| Text(format!("satır {i}"), modifier = Modifier.height(40.dp())));
                    },
                );
                HorizontalPager(
                    state = state.clone(),
                    modifier = Modifier.fill_max_width().height(200.dp()).test_tag("sayfalar"),
                    page_content = |page| Text(format!("sayfa {page}")),
                );
            },
        );
    }));
    let pager = pager.borrow().clone().unwrap();
    let h = rule.host();
    let start = bounds(&rule, "öğe").center();
    h.pointer_down(start, -1, 0, PointerType::Touch);
    for i in 1..=6 {
        h.pointer_move(at(start.x, start.y - 10.0 * i as f32), -1, 0);
    }
    rule.wait_for_idle_within(0);
    assert!(shows(&rule, "liste true sayfa false"));
    assert_eq!(*list_log.borrow(), ["DragStart"]);
    h.pointer_up(at(start.x, start.y - 60.0), -1, 0);
    rule.wait_for_idle();
    assert!(shows(&rule, "liste false sayfa false") && *list_log.borrow() == ["DragStart", "DragStop"]);
    let page = bounds(&rule, "sayfalar").center();
    h.pointer_down(page, -1, 0, PointerType::Touch);
    for i in 1..=6 {
        h.pointer_move(at(page.x - 15.0 * i as f32, page.y), -1, 0);
    }
    rule.wait_for_idle_within(0);
    assert!(shows(&rule, "liste false sayfa true"));
    h.pointer_up(at(page.x - 90.0, page.y), -1, 0);
    rule.wait_for_idle();
    // (a quarter of a page without a fling's speed: Compose's pager goes back to its page; C++ turned it)
    assert!(shows(&rule, "liste false sayfa false") && pager.current_page() == 0, "{pager:?}");
}

/// A pager remembering its state, shown while `shown` (the same content for both hosts).
fn saved_pager(shown: &MutableState<bool>, state: &Rc<RefCell<Option<PagerState>>>) -> impl Fn() + 'static {
    let (sh, st) = (shown.clone(), state.clone());
    kompozable_lambda!(move || {
        if sh.get() {
            let s = remember_pager_state(page_count = || 6);
            *st.borrow_mut() = Some(s.clone());
            HorizontalPager(state = s, modifier = Modifier.fill_max_width().height(100.dp()), page_content = page("p"));
        }
    })
}

fn pager_state_is_saved_with_the_host() {
    // Compose's DefaultPagerState.Saver: the current page and its fraction saved with the host, restored.
    let bytes = {
        let (shown, state) = (mutable_state_of(true), Rc::new(RefCell::new(None)));
        let rule = KompozTestRule::with_viewport(saved_pager(&shown, &state), IntSize::new(400, 400));
        let scope = CoroutineScope::new(rule.host().dispatcher());
        let s: PagerState = state.borrow().clone().unwrap();
        scope.launch(async move { named_args!(s.scroll_to_page(4, page_offset_fraction = 0.25)).await });
        rule.wait_for_idle();
        rule.host().save_state()
    };
    let (shown, state) = (mutable_state_of(false), Rc::new(RefCell::new(None)));
    let rule = KompozTestRule::with_viewport(saved_pager(&shown, &state), IntSize::new(400, 400));
    rule.host().restore_state(&bytes);
    shown.set(true);
    rule.wait_for_idle();
    let s: PagerState = state.borrow().clone().unwrap();
    assert!(s.current_page() == 4 && near_by(s.current_page_offset_fraction(), 0.25, 0.01), "{s:?}");
    assert!(near(x(&rule, "p4"), -100.0));
}

/// Where the knob of C++ 17447 rests.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Spot {
    Start,
    End,
}

fn rtl_nested_horizontal_scrolls_hand_over_the_right_way() {
    // C++ 17447: right to left, a lazy row inside a horizontal scroll hands what it can't take on to the scroll; a
    // lazy row on a page, past its end, moves the page (a drag past half of it turns it); anchoredDraggable, as
    // Compose's, mirrors right to left (its deltas reversed, its content offset relative: the finger to the left is
    // towards the end; C++ kept it to the screen).
    let outer = ScrollState::new(0);
    let inner = LazyListState::new(0, 0);
    let pages = PagerState(0, 0.0, || 3);
    let on_page = LazyListState::new(0, 0);
    let knob: Rc<RefCell<Option<AnchoredDraggableState<Spot>>>> = Rc::new(RefCell::new(None));
    let scope = new_scope();
    let (o, i, p, op, k, sc) =
        (outer.clone(), inner.clone(), pages.clone(), on_page.clone(), knob.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let (o, i, p, op, k) = (o.clone(), i.clone(), p.clone(), op.clone(), k.clone());
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], move || {
                let (o, i, p, op, k) = (o.clone(), i.clone(), p.clone(), op.clone(), k.clone());
                Column(
                    content = move || {
                        let i = i.clone();
                        // (a lazy row inside a horizontal scroll: what the row can't take goes on to the scroll)
                        Row(
                            modifier = Modifier.width(200.dp()).height(40.dp()).horizontal_scroll(&o),
                            content = move || {
                                LazyRow(
                                    modifier = Modifier.width(200.dp()).height(40.dp()).test_tag("iç"),
                                    state = Some(i.clone()),
                                    content = |this: &mut LazyListScope| {
                                        this.items(3, |_| Box(modifier = Modifier.size_wh(100.dp(), 40.dp())));
                                    },
                                );
                                Box(modifier = Modifier.size_wh(300.dp(), 40.dp()));
                            },
                        );
                        // (a lazy row on a page: past its end, the drag moves the page)
                        let op = op.clone();
                        HorizontalPager(
                            state = p.clone(),
                            modifier = Modifier.width(200.dp()).height(60.dp()),
                            page_content = move |page| {
                                if page == 0 {
                                    LazyRow(
                                        modifier = Modifier.fill_max_size().test_tag("sayfadaki"),
                                        state = Some(op.clone()),
                                        content = |this: &mut LazyListScope| {
                                            this.items(3, |_| Box(modifier = Modifier.size_wh(100.dp(), 60.dp())));
                                        },
                                    );
                                }
                            },
                        );
                        // (anchoredDraggable, mirrored)
                        let s = remember(|| AnchoredDraggableState(Spot::Start));
                        *k.borrow_mut() = Some(s.clone());
                        let (s1, s2) = (s.clone(), s.clone());
                        Box(
                            modifier = Modifier.width(256.dp()).height(56.dp()).on_size_changed(move |_| {
                                named_args!(s1.update_anchors(DraggableAnchors(|a| {
                                    a.at(Spot::Start, 0.0);
                                    a.at(Spot::End, 200.0);
                                })));
                            }),
                            content = move || {
                                let o = s2.clone();
                                Box(
                                    modifier = Modifier
                                        .offset_px(move |_| IntOffset::new(o.require_offset().round() as i32, 0))
                                        .anchored_draggable(s2.clone(), Orientation::Horizontal)
                                        .size(56.dp()),
                                    content = || Box(modifier = Modifier.fill_max_size().test_tag("tutamak")),
                                );
                            },
                        );
                    },
                );
            });
        }),
        IntSize::new(200, 600),
    );
    let r = bounds(&rule, "iç");
    // (to the right: on, past the row's end into the scroll)
    swipe(&rule, at(r.left + 10.0, r.top + 20.0), at(r.left + 190.0, r.top + 20.0), 20);
    rule.wait_for_idle();
    assert!(inner.first_visible_item_index() == 1 && outer.value() > 30, "{inner:?} {}", outer.value());
    let s = on_page.clone();
    launch(&scope, async move { s.scroll_to_item(2).await });
    rule.wait_for_idle();
    let p = bounds(&rule, "sayfadaki");
    // (on, past half a page: the page turns)
    swipe(&rule, at(p.left + 10.0, p.top + 30.0), at(p.left + 190.0, p.top + 30.0), 9);
    rule.wait_for_idle();
    assert_eq!(pages.current_page(), 1, "{pages:?}");
    let kb = bounds(&rule, "tutamak");
    swipe(&rule, at(kb.left + 28.0, kb.top + 28.0), at(kb.left - 122.0, kb.top + 28.0), 12);
    rule.wait_for_idle();
    assert_eq!(knob.borrow().as_ref().unwrap().current_value(), Spot::End);
    // (the knob at the end, the left one right to left)
    assert!((bounds(&rule, "tutamak").left - (kb.left - 200.0)).abs() < 0.5, "{:?} {kb:?}", bounds(&rule, "tutamak"));
}

tests!(
    horizontal_pager_drags_snaps_and_animates,
    pager_pages_of_a_fixed_size_snap_where_asked,
    pager_measured_without_end_shows_its_first_pages,
    pager_composes_the_pages_a_bigger_view_shows,
    pagers_laid_out_from_the_end_and_keyed_pages,
    pager_flings_turn_as_many_pages_as_allowed,
    pager_pages_line_up_across_it,
    tap_inside_a_dragged_pager_does_not_fire,
    pager_takes_over_from_a_row_inside_its_page,
    reversed_pager_goes_back_to_its_page_before_a_row_in_it_scrolls,
    pager_turns_the_page_with_a_flick_past_a_row_on_it,
    vertical_pager_snaps_to_pages,
    rtl_pager_turns_with_the_finger_the_other_way,
    pager_overscroll_effect_hears_what_it_leaves,
    lazy_list_and_pager_states_hear_the_users_drags,
    pager_state_is_saved_with_the_host,
    rtl_nested_horizontal_scrolls_hand_over_the_right_way,
);
