//! The C++ Kompoz's named-argument form tests that touch stage 10 (survey §3 Q), in both syntaxes: the Rust one's
//! `#[named]` calls in kompozable bodies and Kotlin's through `kompoz!`.
//!
//! - C++ 8100 (`state_forms_take_compose_named_arguments`): its `PaddingValues` forms (all; horizontal, vertical;
//!   start, top, end, bottom; one by name) and a ViewConfiguration's millis. (Its `kt::PaddingValues(top =, all =)`
//!   isn't a Compose overload; its Channel's ChannelResult, `produce`/`consumeEach` and list arithmetic are the
//!   coroutines' and kt shims' — stage 2 and 9's; Lifecycle's names are stage 13's.)
//! - C++ 8136 (`scroll_forms_take_compose_named_arguments`): `overscrollEffect = null` (none) on a LazyRow. (Its app
//!   bars and pull to refresh are Material's, stage 12's; a VelocityTracker's copies sharing samples is Kotlin's
//!   reference semantics of a class KompozR has as a value — see stage10-followups.md.)
//! - C++ 8188 (`layout_forms_take_compose_named_arguments`): `Arrangement.spacedBy(space, alignment)`, `spring(…)` of
//!   a float with its damping and stiffness, `Modifier.onFirstVisible(minDurationMs =) { }`, a list's
//!   `getOrNull` with a nullable index, `String.repeat`, `Modifier.approachLayout(isMeasurementApproachInProgress =,
//!   approachMeasure =)` by name (its approach measure block Compose's: a `MeasureScope`-style one, its lookahead size
//!   in scope). (Its pane scaffolds are Material's adaptive ones, stage 12's.)
//! - C++ 11965 (`lists_pagers_and_carousels_take_compose_arguments`): a grid's `span = { GridItemSpan(maxLineSpan) }`,
//!   `items(n, span = { … })`, pager states with their count last and by name, `VerticalScrollbar(adapter =,
//!   modifier =)`. (Its carousel and PullToRefreshBox are Material's, stage 12's; AsyncImage is stage 13's.)

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::foundation::lazy::grid::LazyGridItemSpanScope;
use kompoz::prelude::*;
use kompoz::test::{KompozTestRule, Text};
use kompoz::ui::platform::ViewConfiguration;
use std::cell::Cell;
use std::rc::Rc;

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

fn sides(p: &PaddingValues) -> [f32; 4] {
    let d = LayoutDirection::Ltr;
    [
        p.calculate_start_padding(d).0,
        p.calculate_top_padding().0,
        p.calculate_end_padding(d).0,
        p.calculate_bottom_padding().0,
    ]
}

// ---- C++ 8100: PaddingValues forms and a ViewConfiguration's millis ----------------------------------------------

kompoz! {
    fun paddingForms(): List<PaddingValues> = listOf(
        PaddingValues(4.dp),
        PaddingValues(12.dp, 8.dp),
        PaddingValues(1.dp, 2.dp, 3.dp, 4.dp),
        PaddingValues(horizontal = 6.dp),
        PaddingValues(top = 5.dp),
    )
}

fn state_forms_take_compose_named_arguments() {
    // Rust: the associated functions and the named form.
    let rust = [
        PaddingValues::all(4.dp()),
        PaddingValues::hv(12.dp(), 8.dp()),
        PaddingValues::new(1.dp(), 2.dp(), 3.dp(), 4.dp()),
        named_args!(PaddingValues(start = 6.dp(), end = 6.dp())),
        named_args!(PaddingValues(top = 5.dp())),
    ];
    let expected = [
        [4.0, 4.0, 4.0, 4.0],
        [12.0, 8.0, 12.0, 8.0],
        [1.0, 2.0, 3.0, 4.0],
        [6.0, 0.0, 6.0, 0.0],
        [0.0, 5.0, 0.0, 0.0],
    ];
    for (p, e) in rust.iter().zip(expected) {
        assert_eq!(sides(p), e);
    }
    // Kotlin: each overload chosen by its arguments.
    for (p, e) in padding_forms().iter().zip(expected) {
        assert_eq!(sides(p), e);
    }
    let vc = ViewConfiguration::default();
    assert!(vc.long_press_timeout_millis() == 400 && vc.double_tap_timeout_millis() == 300);
}

// ---- C++ 8136: overscrollEffect = null on a LazyRow ---------------------------------------------------------------

kompoz! {
    @Composable
    fun ScrollForms() {
        LazyRow(modifier = Modifier.height(40.dp).width(100.dp).testTag("satır"), overscrollEffect = null) {
            items(3) { Box(modifier = Modifier.size(40.dp)) }
        }
    }
}

#[kompozable]
fn RustScrollForms() {
    LazyRow(
        modifier = Modifier.height(40.dp()).width(100.dp()).test_tag("rust satır"),
        overscroll_effect = Some(None),
        content = |this: &mut LazyListScope| this.items(3, |_| Box(modifier = Modifier.size(40.dp()))),
    );
}

fn scroll_forms_take_compose_named_arguments() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            ScrollForms();
            RustScrollForms();
        }),
        IntSize::new(400, 800),
    );
    assert!(near(bounds(&rule, "satır").width(), 100.0) && near(bounds(&rule, "rust satır").width(), 100.0));
}

// ---- C++ 8188: spacedBy with an alignment, spring, onFirstVisible, getOrNull, repeat --------------------------

thread_local! {
    static SEEN: Cell<i32> = const { Cell::new(0) };
    static APPROACHED: Cell<i32> = const { Cell::new(0) };
}

fn see() {
    SEEN.with(|s| s.set(s.get() + 1));
}

fn approach() {
    APPROACHED.with(|a| a.set(a.get() + 1));
}

kompoz! {
    @Composable
    fun LayoutForms() {
        val names = listOf("a", "b")
        val picked: Int? = 1
        Row(modifier = Modifier.width(200.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally)) {
            Box(modifier = Modifier.size(20.dp).onFirstVisible(minDurationMs = 0) { see() }.testTag("nokta"))
            Box(modifier = Modifier.size(20.dp))
        }
        Text("${names.getOrNull(picked ?: 0)} ${"ab".repeat(3)}")
        Box(modifier = Modifier
                .approachLayout(
                    isMeasurementApproachInProgress = { approach(); false },
                    approachMeasure = { measurable, _ ->
                        val placeable = measurable.measure(Constraints.fixed(lookaheadSize.width, lookaheadSize.height))
                        layout(placeable.width, placeable.height) { placeable.place(0, 0) }
                    })
                .testTag("etiket")) { Text("etiket") }
    }

    fun bouncy(): SpringSpec<Float> = spring(dampingRatio = 0.2f, stiffness = 200f)
}

#[kompozable]
fn RustLayoutForms() {
    Row(
        modifier = Modifier.width(200.dp()).test_tag("rust"),
        horizontal_arrangement = Arrangement::spaced_by_aligned(8.dp(), Alignment::CenterHorizontally),
        content = || {
            Box(modifier =
                Modifier.size(20.dp()).on_first_visible(min_duration_ms = 0, callback = see).test_tag("rust nokta"));
            Box(modifier = Modifier.size(20.dp()));
        },
    );
    Box(
        modifier = Modifier
            .approach_layout(
                is_measurement_approach_in_progress = |_| {
                    approach();
                    false
                },
                approach_measure = |s: &ApproachMeasureScope, m: &dyn Measurable, _| {
                    let size = s.lookahead_size();
                    let p = m.measure(Constraints::fixed(size.width, size.height));
                    s.layout(p.width(), p.height(), move |pl| pl.place(&p, 0, 0))
                },
            )
            .test_tag("rust etiket"),
        content = || Text("rust etiket"),
    );
}

fn layout_forms_take_compose_named_arguments() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            LayoutForms();
            RustLayoutForms();
        }),
        IntSize::new(400, 800),
    );
    rule.advance_time_by(100);
    rule.wait_for_idle();
    // (200 wide, 48 of boxes and a gap centered: the first at 76)
    assert!(near(bounds(&rule, "nokta").left, 76.0) && near(bounds(&rule, "rust nokta").left, 76.0));
    assert_eq!(SEEN.with(Cell::get), 2);
    rule.on_node_with_text("b ababab").assert_exists();
    // The approach layouts: asked, done at once (their lookahead size, as it is).
    assert!(APPROACHED.with(Cell::get) > 0);
    assert!(bounds(&rule, "etiket").width() > 0.0);
    assert!(near(bounds(&rule, "etiket").height(), bounds(&rule, "rust etiket").height()));
    let spec = bouncy();
    assert!(spec.damping_ratio == 0.2 && spec.stiffness == 200.0);
    let rust: SpringSpec<f32> = named_args!(spring(damping_ratio = 0.2, stiffness = 200.0));
    assert!(rust.damping_ratio == spec.damping_ratio && rust.stiffness == spec.stiffness);
}

// ---- C++ 11965: grid spans, pager states, scrollbars by name -----------------------------------------------------

kompoz! {
    @Composable
    fun ListForms(pages: MutableState<Int>, byName: MutableState<Int>) {
        Column {
            LazyVerticalGrid(GridCells.Fixed(3), modifier = Modifier.width(300.dp).height(200.dp)) {
                item(span = { GridItemSpan(maxLineSpan) }) {
                    Box(modifier = Modifier.fillMaxWidth().height(20.dp).testTag("başlık"))
                }
                items(3, span = { i -> GridItemSpan(if (i == 0) 2 else 1) }) { i ->
                    Box(modifier = Modifier.fillMaxWidth().height(20.dp).testTag("hücre$i"))
                }
            }
            pages.value = rememberPagerState { 4 }.pageCount
            byName.value = rememberPagerState(initialPage = 1, pageCount = { 5 }).currentPage
            val scroll = rememberScrollState()
            Box(modifier = Modifier.height(50.dp)) {
                Column(modifier = Modifier.verticalScroll(scroll)) { Box(modifier = Modifier.size(10.dp, 500.dp)) }
                VerticalScrollbar(adapter = rememberScrollbarAdapter(scroll), modifier = Modifier.fillMaxHeight().testTag("çubuk"))
            }
        }
    }
}

#[kompozable]
fn RustListForms(pages: MutableState<i32>, by_name: MutableState<i32>) {
    Column(
        content = move || {
            LazyVerticalGrid(
                GridCells::Fixed(3),
                modifier = Modifier.width(300.dp()).height(200.dp()),
                content = |this: &mut LazyGridScope| {
                    this.item(
                        span = Some(Rc::new(|s: &LazyGridItemSpanScope| GridItemSpan(s.max_line_span()))
                            as Rc<dyn Fn(&LazyGridItemSpanScope) -> GridItemSpan>),
                        content = || Box(modifier = Modifier.fill_max_width().height(20.dp()).test_tag("rust başlık")),
                    );
                    this.items(
                        3,
                        span =
                            Some(Rc::new(|_: &LazyGridItemSpanScope, i: i32| GridItemSpan(if i == 0 { 2 } else { 1 }))
                                as Rc<dyn Fn(&LazyGridItemSpanScope, i32) -> GridItemSpan>),
                        item_content = |i| {
                            Box(modifier = Modifier.fill_max_width().height(20.dp()).test_tag(format!("rust hücre{i}")))
                        },
                    );
                },
            );
            pages.set(remember_pager_state(page_count = || 4).page_count());
            by_name.set(remember_pager_state(initial_page = 1, page_count = || 5).current_page());
            let scroll = remember_scroll_state();
            Box(
                modifier = Modifier.height(50.dp()),
                content = move || {
                    Column(
                        modifier = Modifier.vertical_scroll(&scroll),
                        content = || Box(modifier = Modifier.size_wh(10.dp(), 500.dp())),
                    );
                    VerticalScrollbar(
                        adapter = remember_scrollbar_adapter(scroll.clone()),
                        modifier = Modifier.fill_max_height().test_tag("rust çubuk"),
                    );
                },
            );
        },
    );
}

fn lists_pagers_and_scrollbars_take_compose_arguments() {
    let (pages, by_name) = (mutable_state_of(0), mutable_state_of(0));
    let (rust_pages, rust_by_name) = (mutable_state_of(0), mutable_state_of(0));
    let (p, b, rp, rb) = (pages.clone(), by_name.clone(), rust_pages.clone(), rust_by_name.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            Row(content = {
                let (p, b, rp, rb) = (p.clone(), b.clone(), rp.clone(), rb.clone());
                move || {
                    ListForms(p.clone(), b.clone());
                    RustListForms(rp.clone(), rb.clone());
                }
            });
        }),
        IntSize::new(800, 800),
    );
    rule.wait_for_idle();
    for prefix in ["", "rust "] {
        let (header, wide) = (bounds(&rule, &format!("{prefix}başlık")), bounds(&rule, &format!("{prefix}hücre0")));
        assert!(near(header.width(), 300.0) && near(wide.width(), 200.0), "{prefix}: {header:?} {wide:?}");
        assert!(rule.on_node_with_tag(&format!("{prefix}çubuk")).exists());
    }
    assert_eq!((pages.peek(), by_name.peek(), rust_pages.peek(), rust_by_name.peek()), (4, 1, 4, 1));
}

tests![
    state_forms_take_compose_named_arguments,
    scroll_forms_take_compose_named_arguments,
    layout_forms_take_compose_named_arguments,
    lists_pagers_and_scrollbars_take_compose_arguments,
];
