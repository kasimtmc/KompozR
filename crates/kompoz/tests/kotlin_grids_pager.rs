//! Lazy grids, staggered grids and pagers in Kotlin's syntax, as a Compose user writes them: a grid with a
//! full-line header and spans of a list's items (`item(span = { GridItemSpan(maxLineSpan) })`,
//! `items(list, span = { … })`), a staggered grid with a full-line item and single-lane ones, a pager keyed by its
//! pages (kompozable_tests.cpp 2230, 2683, 3264), and `rememberPagerState { 5 }` turned by `animateScrollToPage` in
//! a coroutine, through `kompoz!`; and in a `.kt` file (kompoz_file!) a grid's state read by its layout info, a
//! horizontal grid, a vertical pager with its current, settled and target pages, a pager of a fixed page size
//! snapping its pages to the middle. The same layouts crates/kompoz-foundation/tests/lazy_grid.rs and pager.rs
//! check in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::{KompozTestRule, Text};
use kompoz::ui::input::PointerType;

/// A node's bounds in the root, unclipped (px; the rule's density is 1).
fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

kompoz! {
    // kompozable_tests.cpp 2230: a full-line header, a list's items spanning cells by their names.
    @Kompozable
    fun SpannedGrid() {
        LazyVerticalGrid(columns = GridCells.Fixed(3), modifier = Modifier.width(300.dp).height(100.dp)) {
            item(span = { GridItemSpan(maxLineSpan) }) { Box(modifier = Modifier.fillMaxWidth().height(20.dp).testTag("header")) }
            items(listOf("a", "b"), span = { name -> GridItemSpan(if (name == "a") 2 else 1) }) { name ->
                Box(modifier = Modifier.fillMaxWidth().height(20.dp).testTag(name))
            }
        }
    }

    // kompozable_tests.cpp 2683: a staggered grid's full-line header and single-lane cards.
    @Kompozable
    fun Cards() {
        LazyVerticalStaggeredGrid(columns = StaggeredGridCells.Fixed(2), modifier = Modifier.size(200.dp)) {
            item(span = StaggeredGridItemSpan.FullLine) { Text("Başlık", modifier = Modifier.testTag("başlık")) }
            items(listOf("a", "b"), span = { StaggeredGridItemSpan.SingleLane }) { Text(it, modifier = Modifier.height(40.dp).testTag("kart$it")) }
        }
    }

    // kompozable_tests.cpp 3264: a pager keyed by its pages.
    @Kompozable
    fun KeyedPages(pages: List<String>) {
        val pager = rememberPagerState(pageCount = { pages.size })
        HorizontalPager(state = pager, modifier = Modifier.size(200.dp, 40.dp), key = { pages[it] }) { i -> Text("sayfa ${pages[i]}") }
    }

    // rememberPagerState { 5 }, turned from a coroutine by animateScrollToPage.
    @Kompozable
    fun Turner() {
        val state = rememberPagerState { 5 }
        val scope = rememberCoroutineScope()
        Column {
            Text("sayfa ${state.currentPage}", modifier = Modifier.clickable {
                scope.launch { state.animateScrollToPage(state.currentPage + 2) }
            })
            HorizontalPager(state = state, modifier = Modifier.fillMaxWidth().height(100.dp)) { page ->
                Box(modifier = Modifier.fillMaxSize().testTag("p$page"))
            }
        }
    }
}

fn a_grid_of_a_full_line_header_and_spans() {
    let rule = KompozTestRule::new(SpannedGrid);
    assert!((bounds(&rule, "header").width() - 300.0).abs() < 0.5);
    assert!((bounds(&rule, "a").width() - 200.0).abs() < 0.5 && (bounds(&rule, "b").width() - 100.0).abs() < 0.5);
}

fn a_staggered_grid_of_a_header_across_both_lanes() {
    let rule = KompozTestRule::new(Cards);
    let (header, a, b) = (bounds(&rule, "başlık"), bounds(&rule, "karta"), bounds(&rule, "kartb"));
    assert!(a.top >= header.bottom - 1.0 && (a.top - b.top).abs() < 1.0 && a.left < b.left, "{header:?} {a:?} {b:?}");
}

fn a_keyed_pager_keeps_its_page() {
    let pages = mutable_state_of(vec!["bir".to_string(), "iki".into()]);
    let p = pages.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || KeyedPages(p.value())));
    rule.on_node_with_text("sayfa bir").assert_exists();
    pages.set(vec!["sıfır".into(), "bir".into(), "iki".into()]);
    rule.wait_for_idle();
    // (the page shown is "bir" still, now the second)
    rule.on_node_with_text("sayfa bir").assert_exists();
    assert!(rule.on_node_with_text("sayfa bir").get_unclipped_bounds_in_root().left.abs() < 0.5);
}

fn a_pager_turned_from_a_coroutine() {
    let rule = KompozTestRule::with_viewport(Turner, IntSize::new(400, 400));
    rule.on_node_with_text("sayfa 0").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("sayfa 2").assert_exists();
    assert!(bounds(&rule, "p2").left.abs() < 0.5);
}

// ---- kompoz_file! -------------------------------------------------------------------------------------------------

kompoz_file!("tests/kt/grids_pager.kt");

/// A finger from `from` to `to`, a move a frame, held `hold` ms, let go.
fn drag(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32, hold: i64) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / frames as f32), -1, 0);
    }
    rule.advance_time_by(hold);
    h.pointer_up(to, -1, 0);
    rule.advance_frame();
}

fn a_grid_state_read_by_its_layout_info_from_a_kt_file() {
    let rule = KompozTestRule::with_viewport(Gallery, IntSize::new(400, 800));
    rule.on_node_with_text("ilk 0 · 3 sütun").assert_exists();
    rule.on_node_with_text("ilk 0 · 3 sütun").perform_click();
    rule.wait_for_idle();
    // (item 31's row: [30 31 32])
    rule.on_node_with_text("ilk 30 · 3 sütun").assert_exists();
    assert!(bounds(&rule, "resim 30").top.abs() < 41.0);
}

fn a_horizontal_grid_from_a_kt_file() {
    let rule = KompozTestRule::with_viewport(Shelves, IntSize::new(400, 800));
    let (side, first, third) = (bounds(&rule, "yan"), bounds(&rule, "kutu 0"), bounds(&rule, "kutu 2"));
    assert!((side.height() - 200.0).abs() < 0.5 && (first.left - 40.0).abs() < 0.5, "{side:?} {first:?}");
    assert!((first.height() - 100.0).abs() < 0.5 && (third.left - 100.0).abs() < 0.5, "{third:?}");
}

fn a_vertical_pager_tells_its_pages_from_a_kt_file() {
    let rule = KompozTestRule::with_viewport(Story, IntSize::new(400, 800));
    rule.on_node_with_text("0/0/0").assert_exists();
    drag(&rule, Offset::new(200.0, 280.0), Offset::new(200.0, 80.0), 20, 200);
    rule.wait_for_idle();
    rule.on_node_with_text("1/1/1").assert_exists();
}

fn a_pager_of_fixed_pages_in_the_middle_from_a_kt_file() {
    let rule = KompozTestRule::with_viewport(Carousel, IntSize::new(400, 800));
    // (200 pages in 300 of room, the first at the start padding; the second the next one 10 on)
    assert!((bounds(&rule, "orta 0").left - 50.0).abs() < 0.5 && (bounds(&rule, "orta 1").left - 260.0).abs() < 0.5);
    rule.on_node_with_text("ileri").perform_click();
    rule.wait_for_idle();
    assert!((bounds(&rule, "orta 2").left - 100.0).abs() < 0.5, "{:?}", bounds(&rule, "orta 2"));
}

tests!(
    a_grid_of_a_full_line_header_and_spans,
    a_staggered_grid_of_a_header_across_both_lanes,
    a_keyed_pager_keeps_its_page,
    a_pager_turned_from_a_coroutine,
    a_grid_state_read_by_its_layout_info_from_a_kt_file,
    a_horizontal_grid_from_a_kt_file,
    a_vertical_pager_tells_its_pages_from_a_kt_file,
    a_pager_of_fixed_pages_in_the_middle_from_a_kt_file,
);
