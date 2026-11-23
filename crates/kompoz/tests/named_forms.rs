//! The C++ Kompoz's named-argument form tests (survey §3 Q) of stages 10 to 12, in both syntaxes: the Rust one's
//! `#[named]` calls in kompozable bodies and Kotlin's through `kompoz!`, each test running both side by side (the
//! Rust one's tags and texts prefixed `rust `), M15's API where the C++'s differs (said at each test).
//!
//! - C++ 8100 (`state_forms_take_compose_named_arguments`): its `PaddingValues` forms (all; horizontal, vertical;
//!   start, top, end, bottom; one by name), a ViewConfiguration's millis and Lifecycle's names (Kotlin's
//!   `Lifecycle.State.RESUMED`, `Lifecycle.Event.ON_STOP`: Rust's `LifecycleState::Resumed`, `LifecycleEvent::OnStop`;
//!   stage 13). (Its `kt::PaddingValues(top =, all =)` isn't a Compose overload; its Channel's ChannelResult,
//!   `produce`/`consumeEach` and list arithmetic are the coroutines' and kt shims' — stage 2 and 9's.)
//! - C++ 8136 (`scroll_forms_take_compose_named_arguments`): `overscrollEffect = null` (none) on a LazyRow; Material's
//!   `Modifier.pullToRefresh(isRefreshing =, state =, onRefresh =)` by name on a Box of `rememberPullToRefreshState()`
//!   with `PullToRefreshDefaults.Indicator(state =, isRefreshing =, modifier =)` and `PullToRefreshDefaults.IndicatorBox(
//!   state =, isRefreshing =, modifier =, shape =) { LoadingIndicator(progress =, modifier =) }` in it (stage 12 WP5,
//!   WP2). (Its TopAppBar and AppBarRow are in kotlin_scaffold.rs (WP4a); its AppBarWithSearch is in
//!   kotlin_search_carousel.rs (WP7); a VelocityTracker's copies sharing samples is Kotlin's reference semantics of a
//!   class KompozR has as a value — see stage10-followups.md.)
//! - C++ 8188 (`layout_forms_take_compose_named_arguments`): `Arrangement.spacedBy(space, alignment)`, `spring(…)` of
//!   a float with its damping and stiffness, `Modifier.onFirstVisible(minDurationMs =) { }`, a list's
//!   `getOrNull` with a nullable index, `String.repeat`, `Modifier.approachLayout(isMeasurementApproachInProgress =,
//!   approachMeasure =)` by name (its approach measure block Compose's: a `MeasureScope`-style one, its lookahead size
//!   in scope). (Its pane scaffolds are Material's adaptive ones, stage 12's.)
//! - C++ 11965 (`lists_pagers_and_carousels_take_compose_arguments`): a grid's `span = { GridItemSpan(maxLineSpan) }`,
//!   `items(n, span = { … })`, pager states with their count last and by name, `VerticalScrollbar(adapter =,
//!   modifier =)`, Material's `PullToRefreshBox(isRefreshing =, onRefresh =, modifier =) { }` (stage 12 WP5, WP2), a
//!   carousel state with its count last (`rememberCarouselState(itemCount =)`) and `HorizontalMultiBrowseCarousel(state
//!   =, preferredItemWidth =, modifier =, itemSpacing =) { index -> }` on its item scope and index (stage 12 WP7; the
//!   C++'s item scope carried the index too: Compose's gives it as the lambda's parameter). (AsyncImage is stage 13's.)
//! - C++ 7683 (`animations_take_compose_named_arguments`), its animated values (stage 11 WP1):
//!   `animateFloatAsState(targetValue =, animationSpec =, label =)`, `animateColorAsState(target, spec)` (C++'s duration
//!   in milliseconds is Compose's `tween(100)`), `updateTransition(targetState =, label =)` and its
//!   `animateFloat(label =) { }`, `animateDp(label =, transitionSpec = { … isTransitioningTo … }) { }`,
//!   `rememberInfiniteTransition(label =).animateFloat(initialValue =, targetValue =, animationSpec =)`. (Its
//!   AnimatedVisibility, AnimatedContent, Crossfade and SharedTransitionLayout parts are WP3's and WP4's.)
//! - C++ 7683's AnimatedVisibility, AnimatedContent and Crossfade parts and C++ 8240's (`graphics_forms_take_compose_
//!   named_arguments`) stage-11 parts (stage 11 WP3): `AnimatedVisibility(visible =, enter =, exit =, label =) { }`
//!   (in a Column: ColumnScope's), `AnimatedContent(targetState =, transitionSpec = { … togetherWith … }, label =)
//!   { n -> }`, `Crossfade(targetState =, label =) { }`, the slides' lambdas of the full size (`slideInVertically { it
//!   / 2 }`: half its height below half way), `AnimatedVisibility(visibleState =, enter =) { }`, `animateEnterExit(exit
//!   =)` by name, `fadeIn(animationSpec =)`, `fadeOut(targetAlpha =)`, `slideOutVertically(animationSpec =) { }`,
//!   `scaleOut(targetScale =)`, `spring(dampingRatio =, stiffness =)`.
//! - C++ 7683's SharedTransitionLayout part (stage 11 WP4): `SharedTransitionLayout(modifier =) { AnimatedContent(
//!   targetState =) { … Modifier.sharedElement(sharedContentState = rememberSharedContentState(key =),
//!   animatedVisibilityScope = this@AnimatedContent) } }`.
//!
//! Stage 12's (Material, M15), each whole unless said:
//! - C++ `material_components_take_compose_arguments`: chips, a toggleable row with a Checkbox, SnackbarHost and
//!   `showSnackbar(actionLabel =)`, `LinearProgressIndicator(progress = { })`, CircularProgressIndicator,
//!   `NavigationSuiteScaffold(navigationSuiteItems =, layoutType =)`.
//! - C++ `theme_pieces_take_compose_named_arguments`, `containers_take_compose_named_arguments`,
//!   `expressive_buttons_take_compose_named_arguments`, `segmented_buttons_take_compose_named_arguments`,
//!   `tabs_and_fab_menu_take_compose_named_arguments`, `text_styles_take_compose_named_arguments`,
//!   `sections_forms_take_compose_named_arguments`.
//! - C++ `app_shell_forms_take_compose_named_arguments`, its stage-12 part: Compose's key names, SnackbarData's
//!   visuals, `NavigationSuiteScaffold(navigationItems = { NavigationSuiteItem(…) }, navigationSuiteType =)` (its two
//!   items switching a state). Its NavController, NavHost, `navOptions { popUpTo<T> { saveState } }`,
//!   `kompozable<T>(deepLinks =)`, `navigation<T>(startDestination =)`, NavDestination's hierarchy and the deep link
//!   wait for stage 13; its desktop MenuBar for stage 15/16 (D7).
//! - C++ `gestures_take_compose_named_arguments` (foundation's, no Material; ported nowhere else): draggable,
//!   dragAndDropTarget (its target made in Rust: Kotlin's `object :` expressions are stage 14's), DraggableAnchors.
//! - C++ `graphics_forms_take_compose_named_arguments`, its Material part (its stage-11 parts above): the wavy
//!   indicators, ContainedLoadingIndicator, `MaterialShapes.Heart.toShape()`, `Morph(Heart, Pill)`.
//! - C++ `drawing_forms_take_compose_named_arguments` (ported nowhere else but its text part, crates/kompoz-ui/
//!   tests/text.rs): whole, its Material piece the drawn text's `typography.labelSmall` (Kotlin's `sign` of an Int
//!   is Float's there: kt has no `Int.sign`).
//! - Elsewhere: C++ `navigation_rail_items_take_their_options` is whole in crates/kompoz-material3/tests/
//!   navigation_rail.rs (Rust) and crates/kompoz/tests/kotlin_navigation.rs `RailItemOptions` (Kotlin); C++ 8188's
//!   pane scaffolds are in kotlin_adaptive.rs (`NamedPaneScaffolds`, `RustNamedPaneScaffolds`); C++ 8136's TopAppBar
//!   and AppBarRow in kotlin_scaffold.rs, its AppBarWithSearch in kotlin_search_carousel.rs.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::foundation::lazy::grid::LazyGridItemSpanScope;
use kompoz::foundation::lazy::staggeredgrid::{LazyStaggeredGridScope, StaggeredGridCells, StaggeredGridItemSpan};
use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use kompoz::ui::draganddrop::{DragAndDropEvent, DragAndDropTarget};
use kompoz::ui::draw::remember_graphics_layer;
use kompoz::ui::platform::ViewConfiguration;
use kompoz::ui::semantics::Role;
use kompoz::ui::text::remember_text_measurer;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

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
    fun lifecycleNames(): Pair<Lifecycle.State, Lifecycle.Event> = Pair(Lifecycle.State.RESUMED, Lifecycle.Event.ON_STOP)

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
    // Lifecycle's names: Kotlin's nested `Lifecycle.State` / `Lifecycle.Event` and their entries.
    let (state, event) = lifecycle_names();
    assert!(state == LifecycleState::Resumed && event == LifecycleEvent::OnStop);
    assert!(state.name() == "RESUMED" && event.to_string() == "ON_STOP");
}

// ---- C++ 8136: overscrollEffect = null on a LazyRow ---------------------------------------------------------------

kompoz! {
    @Composable
    fun ScrollForms(onRefreshed: () -> Unit) {
        val state = rememberPullToRefreshState()
        Column {
            Box(modifier = Modifier.size(200.dp, 120.dp).pullToRefresh(isRefreshing = false, state = state, onRefresh = { onRefreshed() }).testTag("çekilir")) {
                LazyColumn(modifier = Modifier.fillMaxSize()) { items(10) { Text("Satır $it", modifier = Modifier.height(50.dp)) } }
                PullToRefreshDefaults.Indicator(state = state, isRefreshing = false, modifier = Modifier.align(Alignment.TopCenter))
                PullToRefreshDefaults.IndicatorBox(state = state, isRefreshing = false, modifier = Modifier.align(Alignment.TopEnd), shape = CircleShape) {
                    LoadingIndicator(progress = { min(1f, state.distanceFraction) }, modifier = Modifier.size(24.dp))
                }
            }
            LazyRow(modifier = Modifier.height(40.dp).width(100.dp).testTag("satır"), overscrollEffect = null) {
                items(3) { Box(modifier = Modifier.size(40.dp)) }
            }
        }
    }
}

#[kompozable]
fn RustScrollForms(on_refreshed: impl Fn() + 'static) {
    let state = remember_pull_to_refresh_state();
    let on_refreshed = Rc::new(on_refreshed);
    Column(
        content = || {
            let on_refreshed = on_refreshed.clone();
            Box(
                modifier = Modifier
                    .size_wh(200.dp(), 120.dp())
                    .pull_to_refresh(false, state.clone(), on_refresh = move || on_refreshed())
                    .test_tag("rust çekilir"),
                content = || {
                    LazyColumn(
                        modifier = Modifier.fill_max_size(),
                        content = |this: &mut LazyListScope| {
                            this.items(10, |i| Text(format!("Satır {i}"), modifier = Modifier.height(50.dp())))
                        },
                    );
                    PullToRefreshDefaults.Indicator(
                        state = state.clone(),
                        is_refreshing = false,
                        modifier = Modifier.align(Alignment::TopCenter),
                    );
                    let progress_state = state.clone();
                    PullToRefreshDefaults.IndicatorBox(
                        state = state.clone(),
                        is_refreshing = false,
                        modifier = Modifier.align(Alignment::TopEnd),
                        shape = Arc::new(CircleShape) as Arc<dyn Shape>,
                        content = move || {
                            let s = progress_state.clone();
                            LoadingIndicator(
                                progress = move || s.distance_fraction().min(1.0),
                                modifier = Modifier.size(24.dp()),
                            );
                        },
                    );
                },
            );
            LazyRow(
                modifier = Modifier.height(40.dp()).width(100.dp()).test_tag("rust satır"),
                overscroll_effect = Some(None),
                content = |this: &mut LazyListScope| this.items(3, |_| Box(modifier = Modifier.size(40.dp()))),
            );
        },
    );
}

fn scroll_forms_take_compose_named_arguments() {
    let refreshed = Rc::new(Cell::new(0));
    let rule = {
        let refreshed = refreshed.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let (kotlin, rust) = (refreshed.clone(), refreshed.clone());
                Row(content = move || {
                    let (kotlin, rust) = (kotlin.clone(), rust.clone());
                    ScrollForms(move || kotlin.set(kotlin.get() + 1));
                    RustScrollForms(move || rust.set(rust.get() + 10));
                });
            }),
            IntSize::new(400, 800),
        )
    };
    assert!(near(bounds(&rule, "satır").width(), 100.0) && near(bounds(&rule, "rust satır").width(), 100.0));
    // (the C++'s check: the pulled box is there, nothing refreshed yet)
    assert!(rule.on_node_with_tag("çekilir").exists() && rule.on_node_with_tag("rust çekilir").exists());
    assert_eq!(refreshed.get(), 0);
    // (at rest the two loading indicators tell their progress, 0; the arrows nothing)
    assert_eq!(progresses(&rule), [0.0, 0.0]);
    // Pulled 300 down slowly on each (292 past the slop: 146, past the 80 dp threshold): each refreshes. Held there,
    // its arrow and its loading indicator (its progress at most 1) tell the pull.
    for tag in ["çekilir", "rust çekilir"] {
        let top = bounds(&rule, tag).top_center() + Offset::new(0.0, 10.0);
        let h = rule.host();
        h.pointer_down(top, -1, 0, kompoz::ui::input::PointerType::Touch);
        for i in 1..=30 {
            rule.advance_frame();
            h.pointer_move(top + Offset::new(0.0, 10.0 * i as f32), -1, 0);
        }
        rule.advance_frame();
        let held = progresses(&rule);
        assert!(held.len() == 3 && held.contains(&0.0) && held.contains(&1.0), "{held:?}");
        rule.advance_time_by(200);
        h.pointer_up(top + Offset::new(0.0, 300.0), -1, 0);
        rule.wait_for_idle();
    }
    assert_eq!(refreshed.get(), 11);
}

/// The determinate progresses told to accessibility (the pull indicators'), in order.
fn progresses(rule: &KompozTestRule) -> Vec<f32> {
    use kompoz::ui::semantics::SemanticsProperties;
    rule.all_nodes(true)
        .iter()
        .filter_map(|n| n.config().get(SemanticsProperties::ProgressBarRangeInfo))
        .map(|p| p.current)
        .collect()
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
    fun ListForms(pages: MutableState<Int>, byName: MutableState<Int>, carouselItem: (Int) -> Unit) {
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
            val carousel = rememberCarouselState(itemCount = { 3 })
            HorizontalMultiBrowseCarousel(state = carousel, preferredItemWidth = 100.dp, modifier = Modifier.width(300.dp).height(80.dp), itemSpacing = 8.dp) { index ->
                carouselItem(index)
            }
            val scroll = rememberScrollState()
            Box(modifier = Modifier.height(50.dp)) {
                Column(modifier = Modifier.verticalScroll(scroll)) { Box(modifier = Modifier.size(10.dp, 500.dp)) }
                VerticalScrollbar(adapter = rememberScrollbarAdapter(scroll), modifier = Modifier.fillMaxHeight().testTag("çubuk"))
            }
            PullToRefreshBox(isRefreshing = false, onRefresh = {}, modifier = Modifier.size(50.dp)) { Text("içerik") }
        }
    }
}

#[kompozable]
fn RustListForms(pages: MutableState<i32>, by_name: MutableState<i32>, item: Rc<dyn Fn(i32)>) {
    Column(
        content = move || {
            LazyVerticalGrid(
                GridCells::Fixed(3),
                modifier = Modifier.width(300.dp()).height(200.dp()),
                content = |this: &mut LazyGridScope| {
                    this.item(
                        span = Some(Rc::new(|s: &LazyGridItemSpanScope| GridItemSpan(s.max_line_span()))),
                        content = || Box(modifier = Modifier.fill_max_width().height(20.dp()).test_tag("rust başlık")),
                    );
                    this.items(
                        3,
                        span =
                            Some(Rc::new(|_: &LazyGridItemSpanScope, i: i32| GridItemSpan(if i == 0 { 2 } else { 1 }))),
                        item_content = |i| {
                            Box(modifier = Modifier.fill_max_width().height(20.dp()).test_tag(format!("rust hücre{i}")))
                        },
                    );
                },
            );
            pages.set(remember_pager_state(page_count = || 4).page_count());
            by_name.set(remember_pager_state(initial_page = 1, page_count = || 5).current_page());
            let carousel = remember_carousel_state(item_count = || 3);
            let item = item.clone();
            HorizontalMultiBrowseCarousel(
                state = carousel,
                preferred_item_width = 100.dp(),
                modifier = Modifier.width(300.dp()).height(80.dp()),
                item_spacing = 8.dp(),
                content = move |_: &CarouselItemScope, index: i32| item(index),
            );
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
            PullToRefreshBox(
                is_refreshing = false,
                on_refresh = || {},
                modifier = Modifier.size(50.dp()),
                content = || Text("rust içerik"),
            );
        },
    );
}

fn lists_pagers_and_scrollbars_take_compose_arguments() {
    let (pages, by_name) = (mutable_state_of(0), mutable_state_of(0));
    let (rust_pages, rust_by_name) = (mutable_state_of(0), mutable_state_of(0));
    let (p, b, rp, rb) = (pages.clone(), by_name.clone(), rust_pages.clone(), rust_by_name.clone());
    // (the carousel items composed, each syntax's: their indices)
    let (items, rust_items) = (Rc::new(RefCell::new(Vec::<i32>::new())), Rc::new(RefCell::new(Vec::<i32>::new())));
    let (it, rit) = (items.clone(), rust_items.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            Row(content = {
                let (p, b, rp, rb) = (p.clone(), b.clone(), rp.clone(), rb.clone());
                let (it, rit) = (it.clone(), rit.clone());
                move || {
                    let it = it.clone();
                    ListForms(p.clone(), b.clone(), move |i: i32| it.borrow_mut().push(i));
                    let rit = rit.clone();
                    RustListForms(rp.clone(), rb.clone(), Rc::new(move |i: i32| rit.borrow_mut().push(i)));
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
        assert!(rule.on_node_with_text(&format!("{prefix}içerik")).exists());
    }
    assert_eq!((pages.peek(), by_name.peek(), rust_pages.peek(), rust_by_name.peek()), (4, 1, 4, 1));
    for seen in [&items, &rust_items] {
        let seen = seen.borrow();
        assert!(seen.contains(&0) && seen.iter().all(|i| (0..3).contains(i)), "{seen:?}");
    }
}

// ---- C++ 7683: animated values by name -----------------------------------------------------------------------

kompoz! {
    @Composable
    fun AnimationForms(on: Boolean, log: (String) -> Unit) {
        val scale = animateFloatAsState(targetValue = if (on) 2f else 1f, animationSpec = tween(100, 0, LinearEasing), label = "scale")
        val tint = animateColorAsState(if (on) Color.White else Color.Black, tween(100))
        val transition = updateTransition(targetState = on, label = "on")
        val size = transition.animateFloat(label = "size") { if (it) 40f else 20f }
        val dp = transition.animateDp(label = "dp", transitionSpec = { if (false isTransitioningTo true) tween(50) else snap() }) {
            if (it) 10.dp else 0.dp
        }
        val pulse = rememberInfiniteTransition(label = "pulse").animateFloat(initialValue = 0f, targetValue = 1f, animationSpec = infiniteRepeatable(tween(100)))
        log("${scale.value.roundToInt()} ${tint.value.red.roundToInt()} ${size.value.roundToInt()} ${dp.value.value.roundToInt()} ${pulse.value >= 0}")
    }
}

#[kompozable]
fn RustAnimationForms(on: bool, log: Rc<dyn Fn(String)>) {
    let linear: Arc<dyn AnimationSpec<f32>> = Arc::new(TweenSpec::new(100, 0, LinearEasing));
    let scale = animate_float_as_state(
        target_value = if on { 2.0 } else { 1.0 },
        animation_spec = linear,
        label = "scale".into(),
    );
    let tint = animate_color_as_state(if on { Color::White } else { Color::Black }, Arc::new(named_args!(tween(100))));
    let transition = update_transition(target_state = on, label = Some("on".into()));
    let size = transition
        .animate_float(label = "size".into(), target_value_by_state = |on: &bool| if *on { 40.0 } else { 20.0 });
    let dp = transition.animate_dp(
        label = "dp".into(),
        transition_spec = |segment: &Segment<bool>| -> Arc<dyn FiniteAnimationSpec<Dp>> {
            if segment.is_transitioning_to(false, true) { Arc::new(named_args!(tween(50))) } else { Arc::new(snap()) }
        },
        target_value_by_state = |on: &bool| if *on { 10.dp() } else { 0.dp() },
    );
    let pulse = remember_infinite_transition(label = "pulse".into()).animate_float(
        initial_value = 0.0,
        target_value = 1.0,
        animation_spec = infinite_repeatable(TweenSpec::new(100, 0, FastOutSlowInEasing)),
    );
    log(format!(
        "{:.0} {:.0} {:.0} {:.0} {}",
        scale.value(),
        tint.value().red(),
        size.value(),
        dp.value().0,
        pulse.value() >= 0.0
    ));
}

fn animations_take_compose_named_arguments() {
    let on = mutable_state_of(false);
    let (kotlin, rust) = (Rc::new(RefCell::new(Vec::<String>::new())), Rc::new(RefCell::new(Vec::<String>::new())));
    let (o, k, r) = (on.clone(), kotlin.clone(), rust.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let k = k.clone();
        AnimationForms(o.get(), move |line| k.borrow_mut().push(line));
        let r = r.clone();
        RustAnimationForms(o.get(), Rc::new(move |line| r.borrow_mut().push(line)));
    }));
    assert_eq!(kotlin.borrow().last().unwrap(), "1 0 20 0 true");
    assert_eq!(rust.borrow().last().unwrap(), "1 0 20 0 true");
    on.set(true);
    rule.wait_for_idle();
    assert_eq!(kotlin.borrow().last().unwrap(), "2 1 40 10 true");
    assert_eq!(rust.borrow().last().unwrap(), "2 1 40 10 true");
}

// ---- C++ 7683's and 8240's AnimatedVisibility, AnimatedContent and Crossfade parts --------------------------------

kompoz! {
    @Composable
    fun AnimatedForms(on: Boolean, count: Int, shown: MutableTransitionState<Boolean>) {
        val bouncy = spring<Float>(dampingRatio = 0.2f, stiffness = 200f)
        Column {
            AnimatedVisibility(visible = on, enter = slideInVertically { it / 2 } + fadeIn(), exit = slideOutVertically { -it }, label = "details") {
                Text("ayrıntı")
            }
            AnimatedContent(
                targetState = count,
                transitionSpec = {
                    if (targetState > initialState) slideInVertically { it } togetherWith slideOutVertically { -it }
                    else slideInVertically { -it } togetherWith slideOutVertically { it }
                },
                label = "count",
            ) { n -> Text("sayı $n") }
            Crossfade(targetState = on, label = "page") { v -> Text(if (v) "açık" else "kapalı") }
            AnimatedVisibility(visibleState = shown, enter = fadeIn(animationSpec = tween(100)) + scaleIn(animationSpec = bouncy),
                               exit = fadeOut(targetAlpha = 0.5f) + slideOutVertically(animationSpec = tween(100)) { it }) {
                Box(modifier = Modifier.animateEnterExit(exit = scaleOut(targetScale = 0.5f)).size(10.dp).testTag("giren"))
            }
        }
    }

    @Composable
    fun Sliding(visible: Boolean) {
        AnimatedVisibility(visible, enter = slideInVertically(tween(100, easing = LinearEasing)) { it / 2 }, exit = ExitTransition.None) {
            Box(modifier = Modifier.size(100.dp).testTag("kayan"))
        }
    }
}

#[kompozable]
fn RustAnimatedForms(on: bool, count: i32, shown: MutableTransitionState<bool>) {
    let bouncy: Arc<dyn FiniteAnimationSpec<f32>> = Arc::new(SpringSpec::<f32>::new(0.2, 200.0, None));
    Column(move || {
        ColumnAnimatedVisibility(
            visible = on,
            enter = slide_in_vertically(initial_offset_y = |it| it / 2) + fade_in(),
            exit = slide_out_vertically(target_offset_y = |it| -it),
            label = "details".into(),
            content = |_| Text("ayrıntı (Rust)"),
        );
        AnimatedContent(
            target_state = count,
            transition_spec = |s: &AnimatedContentTransitionScope<i32>| {
                if s.target_state() > s.initial_state() {
                    slide_in_vertically(initial_offset_y = |it| it)
                        .together_with(slide_out_vertically(target_offset_y = |it| -it))
                } else {
                    slide_in_vertically(initial_offset_y = |it| -it)
                        .together_with(slide_out_vertically(target_offset_y = |it| it))
                }
            },
            label = "count".into(),
            content = |_, n: i32| Text(format!("sayı {n} (Rust)")),
        );
        Crossfade(
            target_state = on,
            label = "page".into(),
            content = |v: bool| Text(if v { "açık (Rust)" } else { "kapalı (Rust)" }),
        );
        let bouncy = bouncy.clone();
        ColumnAnimatedVisibilityWithState(
            visible_state = shown.clone(),
            enter = fade_in(animation_spec = Arc::new(tween(100))) + scale_in(animation_spec = bouncy),
            exit = fade_out(target_alpha = 0.5)
                + slide_out_vertically(animation_spec = Arc::new(tween(100)), target_offset_y = |it| it),
            content = |s: &AnimatedVisibilityScope| {
                let m = s.animate_enter_exit(Modifier, exit = scale_out(target_scale = 0.5));
                Box(modifier = m.size(10.dp()).test_tag("giren (Rust)"));
            },
        );
    });
}

#[kompozable]
fn RustSliding(visible: bool) {
    let linear: Arc<dyn FiniteAnimationSpec<IntOffset>> = Arc::new(TweenSpec::new(100, 0, LinearEasing));
    AnimatedVisibility(
        visible,
        enter = slide_in_vertically(linear, |h| h / 2),
        exit = ExitTransition::None,
        content = |_| Box(modifier = Modifier.size(100.dp()).test_tag("kayan")),
    );
}

fn animated_forms_take_compose_named_arguments() {
    let on = mutable_state_of(false);
    let count = mutable_state_of(0);
    let (shown, rust_shown) = (MutableTransitionState(false), MutableTransitionState(false));
    shown.set_target_state(true);
    rust_shown.set_target_state(true);
    let (o, c, s, rs) = (on.clone(), count.clone(), shown.clone(), rust_shown.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            AnimatedForms(o.get(), c.get(), s.clone());
            RustAnimatedForms(o.get(), c.get(), rs.clone());
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    for suffix in ["", " (Rust)"] {
        assert!(rule.on_node_with_text(&format!("sayı 0{suffix}")).exists());
        assert!(rule.on_node_with_text(&format!("kapalı{suffix}")).exists());
        assert!(!rule.on_node_with_text(&format!("ayrıntı{suffix}")).exists());
        assert!(rule.on_node_with_tag(&format!("giren{suffix}")).exists());
    }
    assert!(shown.current_state() && rust_shown.current_state());
    on.set(true);
    count.set(1);
    rule.wait_for_idle();
    for suffix in ["", " (Rust)"] {
        assert!(rule.on_node_with_text(&format!("sayı 1{suffix}")).exists());
        assert!(!rule.on_node_with_text(&format!("sayı 0{suffix}")).exists());
        assert!(rule.on_node_with_text(&format!("açık{suffix}")).exists());
        assert!(rule.on_node_with_text(&format!("ayrıntı{suffix}")).exists());
    }
    shown.set_target_state(false);
    rust_shown.set_target_state(false);
    rule.wait_for_idle();
    assert!(!rule.on_node_with_tag("giren").exists() && !rule.on_node_with_tag("giren (Rust)").exists());
    // The slides' lambdas: a content 100 tall hidden by slideInVertically { it / 2 } sits 25 below its place half way.
    for rust in [false, true] {
        let vis = mutable_state_of(false);
        let v = vis.clone();
        let slide = KompozTestRule::new(kompozable_lambda!(move || {
            if rust {
                RustSliding(v.get());
            } else {
                Sliding(v.get());
            }
        }));
        vis.set(true);
        slide.advance_frame(); // (composed; the slide starts at the next frame, as Compose's: C++'s a frame earlier)
        slide.advance_time_by(64);
        let y = bounds(&slide, "kayan").top;
        assert!(y > 20.0 && y < 30.0, "{rust}: {y}");
    }
}

// ---- C++ 7683's SharedTransitionLayout part ----------------------------------------------------------------------

kompoz! {
    @Composable
    fun SharedForms(on: Boolean) {
        SharedTransitionLayout(modifier = Modifier.testTag("paylaşım")) {
            AnimatedContent(targetState = on) { open ->
                Box(modifier = Modifier.size(if (open) 80.dp else 40.dp).testTag(if (open) "büyük" else "küçük")
                    .sharedElement(sharedContentState = rememberSharedContentState(key = "foto"), animatedVisibilityScope = this@AnimatedContent))
            }
        }
    }
}

#[kompozable]
fn RustSharedForms(on: bool) {
    SharedTransitionLayout(
        modifier = Modifier.test_tag("paylaşım (Rust)"),
        content = move |shared: &SharedTransitionScope| {
            let shared = shared.clone();
            AnimatedContent(
                target_state = on,
                content = move |scope: &AnimatedContentScope, open: bool| {
                    let size = if open { 80.dp() } else { 40.dp() };
                    let tag = if open { "büyük (Rust)" } else { "küçük (Rust)" };
                    let m = shared.shared_element(
                        Modifier.size(size).test_tag(tag),
                        shared_content_state = shared.remember_shared_content_state(key = SlotKey::new("foto")),
                        animated_visibility_scope = scope,
                    );
                    Box(modifier = m);
                },
            );
        },
    );
}

fn shared_forms_take_compose_named_arguments() {
    let on = mutable_state_of(false);
    let o = on.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let o = o.clone();
            Column(move || {
                SharedForms(o.get());
                RustSharedForms(o.get());
            });
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    for suffix in ["", " (Rust)"] {
        assert!(rule.on_node_with_tag(&format!("küçük{suffix}")).exists());
        assert!(rule.on_node_with_tag(&format!("paylaşım{suffix}")).exists());
        assert!(!rule.on_node_with_tag(&format!("büyük{suffix}")).exists());
    }
    on.set(true);
    rule.wait_for_idle();
    for suffix in ["", " (Rust)"] {
        assert!(rule.on_node_with_tag(&format!("büyük{suffix}")).exists());
        assert!(!rule.on_node_with_tag(&format!("küçük{suffix}")).exists());
        assert!(near(bounds(&rule, &format!("büyük{suffix}")).width(), 80.0));
    }
}

// ---- C++ material_components_take_compose_arguments ------------------------------------------------------------

kompoz! {
    @Composable
    fun MaterialComponents(on: MutableState<Boolean>, muted: MutableState<Boolean>, snackbars: SnackbarHostState, showing: Boolean) {
        var tab by remember { mutableStateOf(0) }
        // (the C++ showed its snackbar from the test; Compose's showSnackbar suspends: shown from an effect here)
        LaunchedEffect(showing) {
            if (showing) snackbars.showSnackbar("Silindi", actionLabel = "Geri al")
        }
        Column {
            FilterChip(selected = on.value, onClick = { on.value = !on.value }, label = { Text("Okunmamış") },
                       leadingIcon = { Icon(Icons.Filled.Check, contentDescription = "") }, modifier = Modifier.testTag("çip"))
            AssistChip(onClick = {}, label = { Text("Yardım") }, enabled = false)
            Row(modifier = Modifier.toggleable(value = muted.value, onValueChange = { muted.value = it }, role = Role.Checkbox).testTag("satır")) {
                Checkbox(muted.value, null)
                Text("Sessiz")
            }
            SnackbarHost(hostState = snackbars)
            LinearProgressIndicator(progress = { 0.4f }, modifier = Modifier.testTag("ilerleme"))
            CircularProgressIndicator(modifier = Modifier.testTag("dönen"), strokeWidth = 2.dp)
            NavigationSuiteScaffold(
                navigationSuiteItems = {
                    item(selected = tab == 0, onClick = { tab = 0 }, icon = { Icon(Icons.Filled.Home, contentDescription = "") }, label = { Text("Ev") })
                    item(selected = tab == 1, onClick = { tab = 1 }, icon = { Icon(Icons.Filled.Settings, contentDescription = "") }, label = { Text("Ayarlar") })
                },
                modifier = Modifier.height(300.dp),
                layoutType = NavigationSuiteType.NavigationBar,
            ) {
                Text("Sekme $tab")
            }
        }
    }
}

/// A composable slot given by name (Kotlin's `(@Composable () -> Unit)?`).
fn slot(f: impl Fn() + 'static) -> Option<Rc<dyn Fn()>> {
    Some(Rc::new(f))
}

#[kompozable]
fn RustMaterialComponents(
    on: MutableState<bool>,
    muted: MutableState<bool>,
    snackbars: SnackbarHostState,
    showing: bool,
) {
    let tab = remember(|| mutable_state_of(0));
    let shown = snackbars.clone();
    LaunchedEffect(showing, async move {
        if showing {
            named_args!(shown.show_snackbar("rust Silindi".into(), action_label = Some("rust Geri al".into()))).await;
        }
    });
    Column(
        content = move || {
            let o = on.clone();
            FilterChip(
                selected = on.get(),
                on_click = move || o.set(!o.get()),
                label = || Text("rust Okunmamış"),
                leading_icon = slot(|| VectorIcon(Icons::Filled::Check(), Some(String::new()))),
                modifier = Modifier.test_tag("rust çip"),
            );
            AssistChip(on_click = || {}, label = || Text("rust Yardım"), enabled = false);
            let (m, checked) = (muted.clone(), muted.get());
            Row(
                modifier = Modifier
                    .toggleable(value = checked, on_value_change = move |v| m.set(v), role = Some(Role::Checkbox))
                    .test_tag("rust satır"),
                content = move || {
                    Checkbox(checked, None);
                    Text("rust Sessiz");
                },
            );
            SnackbarHost(host_state = snackbars.clone());
            LinearProgressIndicator(progress = || 0.4, modifier = Modifier.test_tag("rust ilerleme"));
            // (Compose's CircularProgressIndicator without a progress: Rust's name for that overload)
            IndeterminateCircularProgressIndicator(modifier = Modifier.test_tag("rust dönen"), stroke_width = 2.dp());
            let t = tab.clone();
            NavigationSuiteScaffoldWithLayoutType(
                navigation_suite_items = move |nav: &mut NavigationSuiteScope| {
                    let (t0, t1) = (t.clone(), t.clone());
                    nav.item(
                        selected = t.get() == 0,
                        on_click = move || t0.set(0),
                        icon = || VectorIcon(Icons::Filled::Home(), Some(String::new())),
                        label = slot(|| Text("rust Ev")),
                    );
                    nav.item(
                        selected = t.get() == 1,
                        on_click = move || t1.set(1),
                        icon = || VectorIcon(Icons::Filled::Settings(), Some(String::new())),
                        label = slot(|| Text("rust Ayarlar")),
                    );
                },
                modifier = Modifier.height(300.dp()),
                layout_type = NavigationSuiteType::NavigationBar,
                content = {
                    let tab = tab.clone();
                    move || Text(format!("rust Sekme {}", tab.get()))
                },
            );
        },
    );
}

fn material_components_take_compose_arguments() {
    let (on, muted, rust_on, rust_muted) =
        (mutable_state_of(false), mutable_state_of(false), mutable_state_of(false), mutable_state_of(false));
    let showing = mutable_state_of(false);
    let (snackbars, rust_snackbars) = (SnackbarHostState::new(), SnackbarHostState::new());
    let (o, m, ro, rm, sh) = (on.clone(), muted.clone(), rust_on.clone(), rust_muted.clone(), showing.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (o, m, ro, rm, sh) = (o.clone(), m.clone(), ro.clone(), rm.clone(), sh.get());
            let (s, rs) = (snackbars.clone(), rust_snackbars.clone());
            Row(content = move || {
                let (o, m, s) = (o.clone(), m.clone(), s.clone());
                Box(
                    modifier = Modifier.width(400.dp()),
                    content = move || MaterialComponents(o.clone(), m.clone(), s.clone(), sh),
                );
                let (ro, rm, rs) = (ro.clone(), rm.clone(), rs.clone());
                Box(
                    modifier = Modifier.width(400.dp()),
                    content = move || RustMaterialComponents(ro.clone(), rm.clone(), rs.clone(), sh),
                );
            });
        }),
        IntSize::new(800, 800),
    );
    use kompoz::ui::semantics::SemanticsProperties;
    for prefix in ["", "rust "] {
        rule.on_node_with_tag(&format!("{prefix}çip")).perform_click();
        rule.on_node_with_tag(&format!("{prefix}satır")).perform_click();
        // (the row: a checkbox to accessibility, its text merged in)
        let row = rule.on_node_with_tag(&format!("{prefix}satır")).fetch_semantics_node();
        assert_eq!(row.config().get(SemanticsProperties::Role), Some(Role::Checkbox));
        assert!(rule.on_node_with_text(&format!("{prefix}Sessiz")).exists());
    }
    assert!(on.get() && muted.get() && rust_on.get() && rust_muted.get());
    showing.set(true);
    rule.wait_for_idle();
    for prefix in ["", "rust "] {
        assert!(rule.on_node_with_text(&format!("{prefix}Geri al")).exists());
        assert!(rule.on_node_with_text(&format!("{prefix}Silindi")).exists());
        assert!(rule.on_node_with_tag(&format!("{prefix}ilerleme")).exists());
        assert!(rule.on_node_with_tag(&format!("{prefix}dönen")).exists());
        assert!(rule.on_node_with_text(&format!("{prefix}Sekme 0")).exists());
        rule.on_node_with_text(&format!("{prefix}Ayarlar")).perform_click();
        assert!(rule.on_node_with_text(&format!("{prefix}Sekme 1")).exists());
    }
}

// ---- C++ theme_pieces_take_compose_named_arguments ---------------------------------------------------------------

kompoz! {
    fun brandScheme(): ColorScheme = lightColorScheme(primary = Color(0xFF0061A4), onPrimary = Color.White)

    fun changedScheme(brand: ColorScheme): ColorScheme = brand.copy(onPrimary = Color.Black)

    fun serifType(): Typography = Typography().copy(headlineSmall = Typography().headlineSmall.copy(fontFamily = FontFamily.Serif, fontSize = 30.sp))

    @Composable
    fun ThemedType(type: Typography, seen: (TextStyle, Color) -> Unit) {
        MaterialTheme(darkColorScheme()) {
            MaterialTheme(typography = type) {
                seen(MaterialTheme.typography.headlineSmall, MaterialTheme.colorScheme.primary)
            }
        }
    }

    fun roundedMedium(): Shapes = Shapes().copy(medium = RoundedCornerShape(20.dp))

    @Composable
    fun ShapedTheme(seen: (CornerBasedShape) -> Unit) {
        MaterialTheme(shapes = Shapes().copy(large = RoundedCornerShape(24.dp))) {
            seen(MaterialTheme.shapes.large)
        }
    }
}

fn theme_pieces_take_compose_named_arguments() {
    // lightColorScheme(primary = …), scheme.copy(…), Typography's and TextStyle's copy by name (the C++'s
    // `Font::copy`), and a MaterialTheme given a typography (its color scheme, left out, the one around it); Shapes,
    // their copy and MaterialTheme(shapes = …). Rust's named form, then Kotlin's.
    let brand = named_args!(light_color_scheme(primary = Color(0xFF0061A4_i64), on_primary = Color::White));
    for brand in [brand, brand_scheme()] {
        assert!(brand.primary == Color(0xFF0061A4_i64) && brand.on_primary == Color::White);
        assert_eq!(brand.secondary, light_color_scheme().secondary);
        // (the surface tint is the primary given)
        assert_eq!(brand.surface_tint, brand.primary);
    }
    for changed in [named_args!(brand.copy(on_primary = Color::Black)), changed_scheme(brand)] {
        assert!(changed.on_primary == Color::Black && changed.primary == brand.primary);
    }
    let base = Typography();
    let headline = named_args!(base.headline_small.copy(font_family = Some(FontFamily::Serif), font_size = 30.sp()));
    let rust_type = named_args!(base.copy(headline_small = headline));
    for ty in [rust_type.clone(), serif_type()] {
        assert!(ty.headline_small.font_family == Some(FontFamily::Serif) && ty.headline_small.font_size == 30.sp());
        assert_eq!(ty.body_medium, Typography().body_medium);
    }
    let (seen, kotlin_seen) = (Rc::new(RefCell::new(None)), Rc::new(RefCell::new(None)));
    let (s, ks) = (seen.clone(), kotlin_seen.clone());
    let t = rust_type.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, t) = (s.clone(), t.clone());
        MaterialTheme(
            color_scheme = dark_color_scheme(),
            content = move || {
                let s = s.clone();
                MaterialTheme(
                    typography = t.clone(),
                    content = move || {
                        *s.borrow_mut() =
                            Some((MaterialTheme::typography().headline_small, MaterialTheme::color_scheme().primary));
                    },
                );
            },
        );
        let ks = ks.clone();
        ThemedType(serif_type(), move |style: TextStyle, primary: Color| *ks.borrow_mut() = Some((style, primary)));
    }));
    for seen in [&seen, &kotlin_seen] {
        let (headline, primary) = seen.borrow().clone().unwrap();
        assert!(headline.font_family == Some(FontFamily::Serif) && headline.font_size == 30.sp());
        // (kept from the theme around it)
        assert_eq!(primary, dark_color_scheme().primary);
    }
    // Shapes: Material's scale by default, MaterialTheme(shapes = …) for what is inside.
    assert_eq!(Shapes().medium, CornerBasedShape::from(RoundedCornerShape(12.dp())));
    let shapes = named_args!(Shapes().copy(medium = RoundedCornerShape(20.dp()).into()));
    for shapes in [shapes, rounded_medium()] {
        assert_eq!(shapes.medium, CornerBasedShape::from(RoundedCornerShape(20.dp())));
    }
    let (corner, kotlin_corner) = (Rc::new(RefCell::new(None)), Rc::new(RefCell::new(None)));
    let (c, kc) = (corner.clone(), kotlin_corner.clone());
    let _shaped = KompozTestRule::new(kompozable_lambda!(move || {
        let c = c.clone();
        let shapes = named_args!(Shapes().copy(large = RoundedCornerShape(24.dp()).into()));
        MaterialTheme(shapes = shapes, content = move || *c.borrow_mut() = Some(MaterialTheme::shapes().large));
        let kc = kc.clone();
        ShapedTheme(move |large: CornerBasedShape| *kc.borrow_mut() = Some(large));
    }));
    for corner in [&corner, &kotlin_corner] {
        assert_eq!(corner.borrow().clone(), Some(CornerBasedShape::from(RoundedCornerShape(24.dp()))));
    }
    // (outside a composition: the default theme's)
    assert_eq!(MaterialTheme::shapes().large, CornerBasedShape::from(RoundedCornerShape(16.dp())));
    assert_eq!(Shapes().large, CornerBasedShape::from(RoundedCornerShape(16.dp())));
}

// ---- C++ containers_take_compose_named_arguments -----------------------------------------------------------------

kompoz! {
    @Composable
    fun ContainerForms(tasks: SnapshotStateList<Int>, plan: MutableState<Int>) {
        Column {
            LazyColumn(modifier = Modifier.height(200.dp)) { items(tasks, key = { it }) { Text("görev $it") } }
            BadgedBox(badge = { Badge { Text("8") } }) { Icon(Icons.Filled.Email, contentDescription = "Posta") }
            BadgedBox(badge = { Badge(modifier = Modifier.testTag("nokta")) }) { Icon(Icons.Filled.Notifications, contentDescription = "Bildirim") }
            for (i in 0 until 2) {
                // (the C++'s `headlineContent = { Text("plan $i") }`: M15's SegmentedListItem takes it as its content)
                SegmentedListItem(selected = plan.value == i, onClick = { plan.value = i }, shapes = ListItemDefaults.segmentedShapes(i, 2),
                                  supportingContent = { Text(if (plan.value == i) "seçili" else "") }) { Text("plan $i") }
            }
            LoadingIndicator(modifier = Modifier.size(48.dp).testTag("yükleniyor"))
            // (the C++'s `progress = 0.5f`: M15 takes a lambda)
            LoadingIndicator(progress = { 0.5f }, modifier = Modifier.testTag("yarım"))
            SmallExtendedFloatingActionButton(onClick = {}, icon = { Icon(Icons.Filled.Add, contentDescription = "") }, text = { Text("Yeni") },
                                              modifier = Modifier.testTag("fab"))
            // (the C++'s `WindowInsets{}`: Compose's none, `WindowInsets(0.dp)`)
            BottomAppBar(actions = { IconButton(onClick = {}) { Icon(Icons.Filled.Check, contentDescription = "Tamam") } },
                         windowInsets = WindowInsets(0.dp), modifier = Modifier.testTag("çubuk"))
        }
    }
}

#[kompozable]
fn RustContainerForms(tasks: SnapshotStateList<i32>, plan: MutableState<i32>) {
    Column(
        content = move || {
            let tasks = tasks.clone();
            LazyColumn(
                modifier = Modifier.height(200.dp()),
                content = move |this: &mut LazyListScope| {
                    this.items_list(
                        &tasks,
                        key = Some(Rc::new(|v: &i32| LazyKey::from(*v)) as Rc<dyn for<'a> Fn(&'a i32) -> LazyKey>),
                        item_content = |v: &i32| Text(format!("rust görev {v}")),
                    );
                },
            );
            BadgedBox(
                badge = || Badge(content = slot(|| Text("rust 8"))),
                content = || VectorIcon(Icons::Filled::Email(), Some("Posta".into())),
            );
            BadgedBox(
                badge = || Badge(modifier = Modifier.test_tag("rust nokta")),
                content = || VectorIcon(Icons::Filled::Notifications(), Some("Bildirim".into())),
            );
            for i in 0..2 {
                let (p, selected) = (plan.clone(), plan.get() == i);
                // (Compose's SegmentedListItem(selected =, onClick =, …): Rust's name for that overload)
                SelectableSegmentedListItem(
                    selected = selected,
                    on_click = move || p.set(i),
                    shapes = ListItemDefaults.segmented_shapes(i, 2),
                    supporting_content = slot(move || Text(if selected { "rust seçili" } else { "" })),
                    content = move || Text(format!("rust plan {i}")),
                );
            }
            IndeterminateLoadingIndicator(modifier = Modifier.size(48.dp()).test_tag("rust yükleniyor"));
            LoadingIndicator(progress = || 0.5, modifier = Modifier.test_tag("rust yarım"));
            TextIconSmallExtendedFloatingActionButton(
                on_click = || {},
                icon = || VectorIcon(Icons::Filled::Add(), Some(String::new())),
                text = || Text("rust Yeni"),
                modifier = Modifier.test_tag("rust fab"),
            );
            BottomAppBar(
                actions = || {
                    IconButton(on_click = || {}, content = || VectorIcon(Icons::Filled::Check(), Some("Tamam".into())))
                },
                window_insets = WindowInsets(),
                modifier = Modifier.test_tag("rust çubuk"),
            );
        },
    );
}

fn containers_take_compose_named_arguments() {
    let tasks = mutable_state_list_of([1, 2, 3]);
    let (plan, rust_plan) = (mutable_state_of(0), mutable_state_of(0));
    let (t, p, rp) = (tasks.clone(), plan.clone(), rust_plan.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (t, p, rp) = (t.clone(), p.clone(), rp.clone());
            Row(content = move || {
                let (t1, p) = (t.clone(), p.clone());
                Box(modifier = Modifier.width(400.dp()), content = move || ContainerForms(t1.clone(), p.clone()));
                let (t2, rp) = (t.clone(), rp.clone());
                Box(modifier = Modifier.width(400.dp()), content = move || RustContainerForms(t2.clone(), rp.clone()));
            });
        }),
        IntSize::new(800, 800),
    );
    for prefix in ["", "rust "] {
        assert!(rule.on_node_with_text(&format!("{prefix}görev 2")).exists());
    }
    tasks.add_at(0, 9);
    rule.wait_for_idle();
    for prefix in ["", "rust "] {
        assert!(rule.on_node_with_text(&format!("{prefix}görev 9")).exists());
        assert!(rule.on_node_with_text(&format!("{prefix}8")).exists());
        assert!(rule.on_node_with_tag(&format!("{prefix}nokta")).exists());
        rule.on_node_with_text(&format!("{prefix}plan 1")).perform_click();
        assert!(rule.on_node_with_text(&format!("{prefix}seçili")).exists());
        assert!(rule.on_node_with_tag(&format!("{prefix}yükleniyor")).exists());
        assert!(rule.on_node_with_tag(&format!("{prefix}yarım")).exists());
        // (M15's extended FAB clears its text's semantics (`clearAndSetSemantics {}`): in the unmerged tree only)
        assert!(named_args!(rule.on_node_with_text(&format!("{prefix}Yeni"), use_unmerged_tree = true)).exists());
        assert!(!rule.on_node_with_text(&format!("{prefix}Yeni")).exists());
        assert!(bounds(&rule, &format!("{prefix}çubuk")).height() <= 80.5);
        // (the loading indicators: one indeterminate, 48 dp; one at half)
        assert!(near(bounds(&rule, &format!("{prefix}yükleniyor")).width(), 48.0));
        let half = rule.on_node_with_tag(&format!("{prefix}yarım")).fetch_semantics_node();
        use kompoz::ui::semantics::SemanticsProperties;
        assert_eq!(half.config().get(SemanticsProperties::ProgressBarRangeInfo).map(|r| r.current), Some(0.5));
    }
    assert!(plan.get() == 1 && rust_plan.get() == 1);
}

// ---- C++ expressive_buttons_take_compose_named_arguments ---------------------------------------------------------
// M15 differences: the menu state's `isExpanded` is hidden in M15 (its `isShowing`); M15 has no
// `SplitButtonDefaults.TrailingButtonIcon` (its samples' turning arrow); M15's `SliderDefaults.Thumb(state)` is
// `Thumb(interactionSource, sliderState)` (Rust `StateThumb`), the slot overload of Slider Rust's `SlotSlider`.

kompoz! {
    @Composable
    fun ExpressiveForms(last: MutableState<String>, bold: MutableState<Boolean>) {
        var open by remember { mutableStateOf(false) }
        var volume by remember { mutableStateOf(40f) }
        var price by remember { mutableStateOf(200f..800f) }
        val source = remember { MutableInteractionSource() }
        Column {
            ButtonGroup(modifier = Modifier.testTag("grup")) {
                Button(onClick = { last.value = "oynat" }, modifier = Modifier.animateWidth(source).testTag("oynat"), interactionSource = source) { Text("Oynat") }
            }
            ButtonGroup(
                overflowIndicator = { menuState ->
                    IconButton(onClick = { if (menuState.isShowing) menuState.dismiss() else menuState.show() }, modifier = Modifier.testTag("daha")) {
                        Icon(Icons.Filled.MoreVert, contentDescription = "Daha")
                    }
                },
                modifier = Modifier.width(120.dp),
            ) {
                clickableItem(onClick = { last.value = "kopyala" }, label = "Kopyala")
                toggleableItem(checked = bold.value, onCheckedChange = { bold.value = it }, label = "Kalın")
                clickableItem(onClick = { last.value = "sil" }, label = "Sil", icon = { Icon(Icons.Filled.Delete, contentDescription = "") })
            }
            ToggleButton(checked = bold.value, onCheckedChange = { bold.value = it }, shapes = ButtonGroupDefaults.connectedLeadingButtonShapes()) { Text("Koyu") }
            SplitButtonLayout(
                leadingButton = { SplitButtonDefaults.LeadingButton(onClick = { last.value = "kaydet" }) { Text("Kaydet") } },
                trailingButton = {
                    SplitButtonDefaults.TrailingButton(checked = open, onCheckedChange = { open = it }) {
                        val rotation: Float by animateFloatAsState(targetValue = if (open) 180f else 0f)
                        Icon(
                            Icons.Filled.KeyboardArrowDown,
                            modifier = Modifier.size(SplitButtonDefaults.TrailingIconSize).graphicsLayer { rotationZ = rotation },
                            contentDescription = "Localized description",
                        )
                    }
                },
            )
            Slider(value = volume, onValueChange = { volume = it }, valueRange = 0f..100f, interactionSource = source,
                   thumb = { s -> Label(label = { PlainTooltip { Text("x") } }, interactionSource = source) { SliderDefaults.Thumb(interactionSource = source, sliderState = s) } })
            RangeSlider(value = price, onValueChange = { price = it }, valueRange = 0f..1000f, steps = 19)
            val stars = rememberSliderState(value = 3f, steps = 3, valueRange = 1f..5f)
            val hours = rememberRangeSliderState(activeRangeStart = 9f, activeRangeEnd = 17f, steps = 23, valueRange = 0f..24f)
            Text("son: ${last.value.ifEmpty { "yok" }} · ${stars.value.roundToInt()} · ${hours.activeRangeEnd.roundToInt()}")
        }
    }
}

#[kompozable]
fn RustExpressiveForms(last: MutableState<String>, bold: MutableState<bool>) {
    let open = remember(|| mutable_state_of(false));
    let volume = remember(|| mutable_state_of(40.0f32));
    let price = remember(|| mutable_state_of(200.0f32..=800.0));
    let source = remember(MutableInteractionSource::new);
    Column(move || {
        let (l1, s1) = (last.clone(), source.clone());
        ButtonGroupWithoutOverflowIndicator(
            modifier = Modifier.test_tag("rust grup"),
            content = move |group: &ButtonGroupScope| {
                let l = l1.clone();
                Button(
                    on_click = move || l.set("oynat".into()),
                    modifier = group.animate_width(Modifier, &s1).test_tag("rust oynat"),
                    interaction_source = Some(s1.clone()),
                    content = || Text("rust Oynat"),
                );
            },
        );
        let (l2, b2) = (last.clone(), bold.clone());
        ButtonGroup(
            overflow_indicator = |menu: &ButtonGroupMenuState| {
                let menu = menu.clone();
                IconButton(
                    on_click = move || if menu.is_showing() { menu.dismiss() } else { menu.show() },
                    modifier = Modifier.test_tag("rust daha"),
                    content = || VectorIcon(Icons::Filled::MoreVert(), Some("Daha".into())),
                );
            },
            modifier = Modifier.width(120.dp()),
            content = move |items: &ButtonGroupScope| {
                let (k, s, b) = (l2.clone(), l2.clone(), b2.clone());
                items.clickable_item(on_click = move || k.set("kopyala".into()), label = "rust Kopyala");
                items.toggleable_item(
                    checked = b2.get(),
                    on_checked_change = move |on| b.set(on),
                    label = "rust Kalın",
                );
                items.clickable_item(
                    on_click = move || s.set("sil".into()),
                    label = "rust Sil",
                    icon = slot(|| VectorIcon(Icons::Filled::Delete(), Some(String::new()))),
                );
            },
        );
        let b3 = bold.clone();
        ToggleButton(
            checked = bold.get(),
            on_checked_change = move |on| b3.set(on),
            shapes = ButtonGroupDefaults.connected_leading_button_shapes(),
            content = || Text("rust Koyu"),
        );
        let (l4, o4) = (last.clone(), open.clone());
        SplitButtonLayout(
            leading_button = move || {
                let l = l4.clone();
                SplitButtonDefaults
                    .LeadingButton(on_click = move || l.set("kaydet".into()), content = || Text("rust Kaydet"));
            },
            trailing_button = move || {
                let (o, checked) = (o4.clone(), o4.get());
                SplitButtonDefaults.ToggleableTrailingButton(
                    checked = checked,
                    on_checked_change = move |on| o.set(on),
                    content = move || {
                        let rotation = animate_float_as_state(target_value = if checked { 180.0 } else { 0.0 });
                        VectorIcon(
                            Icons::Filled::KeyboardArrowDown(),
                            Some("Localized description".into()),
                            modifier = Modifier
                                .size(SplitButtonDefaults::TrailingIconSize)
                                .graphics_layer_block(move |layer| layer.rotation_z = rotation.value()),
                        );
                    },
                );
            },
        );
        let (v5, s5) = (volume.clone(), source.clone());
        SlotSlider(
            value = volume.get(),
            on_value_change = move |v| v5.set(v),
            value_range = 0.0..=100.0,
            interaction_source = Some(source.clone()),
            thumb = Some(Rc::new(move |s: SliderState| {
                let s6 = s5.clone();
                Label(
                    label = |scope: &dyn TooltipScope| scope.PlainTooltip(content = || Text("x")),
                    interaction_source = Some(s5.clone()),
                    content = move || SliderDefaults.StateThumb(s6.clone(), s.clone()),
                );
            }) as Rc<dyn Fn(SliderState)>),
        );
        let p7 = price.clone();
        RangeSlider(value = price.get(), on_value_change = move |v| p7.set(v), value_range = 0.0..=1000.0, steps = 19);
        let stars = remember_slider_state(value = 3.0, steps = 3, value_range = 1.0..=5.0);
        let hours = remember_range_slider_state(
            active_range_start = 9.0,
            active_range_end = 17.0,
            steps = 23,
            value_range = 0.0..=24.0,
        );
        let last = last.get();
        Text(format!(
            "rust son: {} · {:.0} · {:.0}",
            if last.is_empty() { "yok".to_string() } else { last },
            stars.value(),
            hours.active_range_end()
        ));
    });
}

/// The bounds of the shown node of `text` (the last one: a group's menu's, over its row's, which are composed and
/// not placed).
fn shown_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    let all = named_args!(rule.on_all_nodes_with_text(text, use_unmerged_tree = true)).fetch_semantics_nodes();
    all.iter().map(|n| n.bounds_in_root()).rfind(|b| b.width() > 0.0).expect("shown")
}

fn tap(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_down(at, -1, 0, kompoz::ui::input::PointerType::Touch);
    rule.host().pointer_up(at, -1, 0);
    rule.wait_for_idle();
}

fn expressive_buttons_take_compose_named_arguments() {
    let (last, bold, rust_last, rust_bold) = (
        mutable_state_of(String::new()),
        mutable_state_of(false),
        mutable_state_of(String::new()),
        mutable_state_of(false),
    );
    let (l, b, rl, rb) = (last.clone(), bold.clone(), rust_last.clone(), rust_bold.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (l, b, rl, rb) = (l.clone(), b.clone(), rl.clone(), rb.clone());
            Row(content = move || {
                let (l, b) = (l.clone(), b.clone());
                Box(modifier = Modifier.width(400.dp()), content = move || ExpressiveForms(l.clone(), b.clone()));
                let (rl, rb) = (rl.clone(), rb.clone());
                Box(modifier = Modifier.width(400.dp()), content = move || RustExpressiveForms(rl.clone(), rb.clone()));
            });
        }),
        IntSize::new(800, 800),
    );
    use kompoz::ui::semantics::SemanticsActions;
    for prefix in ["", "rust "] {
        rule.on_node_with_text(&format!("{prefix}son: yok · 3 · 17")).assert_exists();
        rule.on_node_with_tag(&format!("{prefix}oynat")).perform_click();
        rule.on_node_with_text(&format!("{prefix}Kaydet")).perform_click();
        rule.on_node_with_text(&format!("{prefix}son: kaydet · 3 · 17")).assert_exists();
        rule.on_node_with_text(&format!("{prefix}Koyu")).perform_click();
        // (M15's group is as tall as the items it shows — none here —: the indicator, placed at its top, reaches over
        // the toggle button under it, which takes a touch there (Compose's later sibling first). Its click action,
        // then.)
        rule.on_node_with_tag(&format!("{prefix}daha")).perform_semantics_action(SemanticsActions::OnClick);
        rule.wait_for_idle();
        // the menu's (nothing of 120 dp fits beside the indicator: all three are in the menu)
        tap(&rule, shown_bounds(&rule, &format!("{prefix}Sil")).center());
        rule.on_node_with_text(&format!("{prefix}son: sil · 3 · 17")).assert_exists();
    }
    assert!(bold.get() && rust_bold.get());
    assert!(last.get() == "sil" && rust_last.get() == "sil");
}

// ---- C++ segmented_buttons_take_compose_named_arguments ----------------------------------------------------------
// M15: the shapes are given (no default; the multi-choice row's one segment the base shape), the base shape
// CircleShape (50% corners, the C++'s too).

kompoz! {
    @Composable
    fun SegmentedForms(pick: MutableState<Int>, bold: MutableState<Boolean>) {
        Column {
            SingleChoiceSegmentedButtonRow(modifier = Modifier.width(300.dp)) {
                for (i in 0 until 2) {
                    SegmentedButton(selected = pick.value == i, onClick = { pick.value = i }, shape = SegmentedButtonDefaults.itemShape(i, 2)) {
                        Text(if (i == 0) "Bir" else "İki")
                    }
                }
            }
            MultiChoiceSegmentedButtonRow {
                SegmentedButton(checked = bold.value, onCheckedChange = { bold.value = it }, shape = SegmentedButtonDefaults.itemShape(0, 1),
                                label = { Text("Kalın") })
            }
            Text("seçim ${pick.value} ${if (bold.value) "kalın" else "ince"}")
            LazyVerticalStaggeredGrid(StaggeredGridCells.Fixed(2), modifier = Modifier.size(200.dp)) {
                item(span = StaggeredGridItemSpan.FullLine) { Text("Başlık", modifier = Modifier.testTag("başlık")) }
                items(2) { i -> Text("$i", modifier = Modifier.height(40.dp).testTag("kart$i")) }
            }
        }
    }

    @Composable
    fun SegmentShapes(seen: (Shape, Shape, Shape) -> Unit) {
        seen(SegmentedButtonDefaults.itemShape(0, 3), SegmentedButtonDefaults.itemShape(1, 3), SegmentedButtonDefaults.itemShape(2, 3))
    }
}

#[kompozable]
fn RustSegmentedForms(pick: MutableState<i32>, bold: MutableState<bool>) {
    Column(move || {
        let p = pick.clone();
        SingleChoiceSegmentedButtonRow(
            modifier = Modifier.width(300.dp()),
            content = move |row| {
                for i in 0..2 {
                    let p = p.clone();
                    row.SegmentedButton(
                        selected = p.get() == i,
                        on_click = move || p.set(i),
                        shape = SegmentedButtonDefaults.item_shape(i, 2),
                        label = move || Text(if i == 0 { "rust Bir" } else { "rust İki" }),
                    );
                }
            },
        );
        let b = bold.clone();
        MultiChoiceSegmentedButtonRow(
            content = move |row| {
                let b = b.clone();
                row.SegmentedButton(
                    checked = b.get(),
                    on_checked_change = move |on| b.set(on),
                    shape = SegmentedButtonDefaults.item_shape(0, 1),
                    label = || Text("rust Kalın"),
                );
            },
        );
        Text(format!("rust seçim {} {}", pick.get(), if bold.get() { "kalın" } else { "ince" }));
        LazyVerticalStaggeredGrid(
            columns = StaggeredGridCells::Fixed(2),
            modifier = Modifier.size(200.dp()),
            content = move |this: &mut LazyStaggeredGridScope| {
                this.item(
                    span = Some(StaggeredGridItemSpan::FullLine),
                    content = || Text("rust Başlık", modifier = Modifier.test_tag("rust başlık")),
                );
                this.items(
                    2,
                    item_content =
                        |i| Text(i.to_string(), modifier = Modifier.height(40.dp()).test_tag(format!("rust kart{i}"))),
                );
            },
        );
    });
}

fn segmented_buttons_take_compose_named_arguments() {
    let (pick, bold, rust_pick, rust_bold) =
        (mutable_state_of(0), mutable_state_of(false), mutable_state_of(0), mutable_state_of(false));
    let kotlin_shapes: Rc<RefCell<Vec<Arc<dyn Shape>>>> = Rc::new(RefCell::new(Vec::new()));
    let (p, b, rp, rb, ks) = (pick.clone(), bold.clone(), rust_pick.clone(), rust_bold.clone(), kotlin_shapes.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (p, b, rp, rb) = (p.clone(), b.clone(), rp.clone(), rb.clone());
            Row(content = move || {
                let (p, b) = (p.clone(), b.clone());
                Box(modifier = Modifier.width(400.dp()), content = move || SegmentedForms(p.clone(), b.clone()));
                let (rp, rb) = (rp.clone(), rb.clone());
                Box(modifier = Modifier.width(400.dp()), content = move || RustSegmentedForms(rp.clone(), rb.clone()));
            });
            let ks = ks.clone();
            SegmentShapes(move |a: Arc<dyn Shape>, b: Arc<dyn Shape>, c: Arc<dyn Shape>| {
                *ks.borrow_mut() = vec![a, b, c]
            });
        }),
        IntSize::new(800, 800),
    );
    for prefix in ["", "rust "] {
        rule.on_node_with_text(&format!("{prefix}İki")).perform_click();
        rule.on_node_with_text(&format!("{prefix}Kalın")).perform_click();
        rule.on_node_with_text(&format!("{prefix}seçim 1 kalın")).assert_exists();
        let tag = |t: &str| bounds(&rule, &format!("{prefix}{t}"));
        let (header, a, b) = (tag("başlık"), tag("kart0"), tag("kart1"));
        assert!(a.top >= header.top + header.height() - 1.0 && (a.top - b.top).abs() < 1.0 && a.left < b.left);
    }
    assert!(pick.get() == 1 && bold.get() && rust_pick.get() == 1 && rust_bold.get());
    let corners = |s: &Arc<dyn Shape>| {
        let c = (**s).shape_as_any().downcast_ref::<CornerBasedShape>().expect("a corner based shape").clone();
        (c.top_start().clone(), c.top_end().clone(), c.bottom_end().clone(), c.bottom_start().clone())
    };
    let rust_shapes: Vec<Arc<dyn Shape>> =
        (0..3).map(|i| named_args!(SegmentedButtonDefaults.item_shape(i, 3))).collect();
    let (zero, full) = (CornerSize::dp(Dp(0.0)), CornerSize::percent(50));
    for shapes in [rust_shapes, kotlin_shapes.borrow().clone()] {
        let [first, middle, last] = &shapes[..] else { panic!("three shapes") };
        assert_eq!(corners(first), (full.clone(), zero.clone(), zero.clone(), full.clone()));
        assert!(**middle == *(&RectangleShape as &dyn Shape));
        assert_eq!(corners(last), (zero.clone(), full.clone(), full.clone(), zero.clone()));
    }
}

// ---- C++ tabs_and_fab_menu_take_compose_named_arguments ----------------------------------------------------------
// Its tabs, the FAB menu (its items made through the scope) and HorizontalFloatingToolbar's colors by name.

kompoz! {
    @Composable
    fun TabsAndFabMenu(tab: MutableState<Int>) {
        var open by remember { mutableStateOf(false) }
        var picked by remember { mutableStateOf("yok") }
        Column {
            PrimaryTabRow(selectedTabIndex = tab.value) {
                for (i in 0 until 2) {
                    Tab(selected = tab.value == i, onClick = { tab.value = i }, text = { Text("sekme $i") }, icon = { Icon(Icons.Filled.Star, contentDescription = "") })
                }
            }
            SecondaryScrollableTabRow(selectedTabIndex = tab.value, edgePadding = 0.dp) {
                for (i in 0 until 2) {
                    LeadingIconTab(selected = tab.value == i, onClick = { tab.value = i }, text = { Text("kayan $i") },
                                   icon = { Icon(Icons.Filled.Home, contentDescription = "") }, unselectedContentColor = Color.Red)
                }
            }
            Text("seçili ${tab.value} · $picked")
            Box(modifier = Modifier.size(300.dp), contentAlignment = Alignment.BottomEnd) {
                FloatingActionButtonMenu(
                    expanded = open,
                    button = {
                        ToggleFloatingActionButton(checked = open, onCheckedChange = { open = it }, modifier = Modifier.testTag("aç")) {
                            Icon(Icons.Filled.Add, contentDescription = "Menü")
                        }
                    },
                ) {
                    for (label in listOf("Not", "Konum")) {
                        FloatingActionButtonMenuItem(onClick = { picked = label; open = false }, text = { Text(label) },
                                                     icon = { Icon(Icons.Filled.Edit, contentDescription = "") })
                    }
                }
            }
            HorizontalFloatingToolbar(expanded = true, colors = FloatingToolbarDefaults.vibrantFloatingToolbarColors()) {
                IconButton(onClick = {}) { Icon(Icons.Filled.MoreVert, contentDescription = "Daha") }
            }
        }
    }
}

#[kompozable]
fn RustTabsAndFabMenu(tab: MutableState<i32>) {
    let open = remember(|| mutable_state_of(false));
    let picked = remember(|| mutable_state_of("yok".to_string()));
    Column(move || {
        let (t1, t2) = (tab.clone(), tab.clone());
        PrimaryTabRow(
            selected_tab_index = tab.get(),
            tabs = move || {
                for i in 0..2 {
                    let t = t1.clone();
                    // (Compose's Tab(selected, onClick, text =, icon =): Rust's name for that overload)
                    TextIconTab(
                        selected = t.get() == i,
                        on_click = move || t.set(i),
                        text = slot(move || Text(format!("rust sekme {i}"))),
                        icon = slot(|| VectorIcon(Icons::Filled::Star(), Some(String::new()))),
                    );
                }
            },
        );
        SecondaryScrollableTabRow(
            selected_tab_index = tab.get(),
            edge_padding = 0.dp(),
            tabs = move || {
                for i in 0..2 {
                    let t = t2.clone();
                    LeadingIconTab(
                        selected = t.get() == i,
                        on_click = move || t.set(i),
                        text = move || Text(format!("rust kayan {i}")),
                        icon = || VectorIcon(Icons::Filled::Home(), Some(String::new())),
                        unselected_content_color = Some(Color::Red),
                    );
                }
            },
        );
        Text(format!("rust seçili {} · {}", tab.get(), picked.get()));
        let (o, p) = (open.clone(), picked.clone());
        Box(
            modifier = Modifier.size(300.dp()),
            content_alignment = Alignment::BottomEnd,
            content = move || {
                let (button_open, items_open, p) = (o.clone(), o.clone(), p.clone());
                FloatingActionButtonMenu(
                    expanded = o.get(),
                    button = move || {
                        let set = button_open.clone();
                        ToggleFloatingActionButton(
                            checked = button_open.get(),
                            on_checked_change = move |on| set.set(on),
                            modifier = Modifier.test_tag("rust aç"),
                            content = |_: &ToggleFloatingActionButtonScope| {
                                VectorIcon(Icons::Filled::Add(), Some("rust Menü".into()))
                            },
                        );
                    },
                    content = move |menu: &FloatingActionButtonMenuScope| {
                        for label in ["rust Not", "rust Konum"] {
                            let (p, o) = (p.clone(), items_open.clone());
                            menu.FloatingActionButtonMenuItem(
                                on_click = move || {
                                    p.set(label.to_string());
                                    o.set(false);
                                },
                                text = move || Text(label),
                                icon = || VectorIcon(Icons::Filled::Edit(), Some(String::new())),
                            );
                        }
                    },
                );
            },
        );
        HorizontalFloatingToolbar(
            expanded = true,
            colors = FloatingToolbarDefaults.vibrant_floating_toolbar_colors(),
            content = || {
                IconButton(
                    on_click = || {},
                    content = || VectorIcon(Icons::Filled::MoreVert(), Some("rust Daha".into())),
                );
            },
        );
    });
}

fn tabs_and_fab_menu_take_compose_named_arguments() {
    let (tab, rust_tab) = (mutable_state_of(0), mutable_state_of(0));
    let (t, rt) = (tab.clone(), rust_tab.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (t, rt) = (t.clone(), rt.clone());
            Row(content = move || {
                let t = t.clone();
                Box(modifier = Modifier.width(400.dp()), content = move || TabsAndFabMenu(t.clone()));
                let rt = rt.clone();
                Box(modifier = Modifier.width(400.dp()), content = move || RustTabsAndFabMenu(rt.clone()));
            });
        }),
        IntSize::new(800, 800),
    );
    for prefix in ["", "rust "] {
        let text = |t: &str| format!("{prefix}{t}");
        rule.on_node_with_text(&text("sekme 1")).assert_exists();
        rule.on_node_with_text(&text("kayan 1")).assert_exists();
        rule.on_node_with_text(&text("kayan 1")).perform_click();
        rule.on_node_with_text(&text("seçili 1 · yok")).assert_exists();
        rule.on_node_with_tag(&text("aç")).perform_click();
        for _ in 0..91 {
            rule.advance_frame();
        }
        rule.on_node_with_text(&text("Konum")).assert_exists();
        rule.on_node_with_text(&text("Konum")).perform_click();
        for _ in 0..90 {
            rule.advance_frame();
        }
        rule.on_node_with_text(&format!("{prefix}seçili 1 · {prefix}Konum")).assert_exists();
        rule.on_node_with_content_description(&text("Daha")).assert_exists();
        // (closed by the item: a FAB menu item hidden tells nothing)
        assert!(!rule.on_node_with_text(&text("Konum")).exists());
    }
    assert!(tab.get() == 1 && rust_tab.get() == 1);
}

// ---- C++ app_shell_forms_take_compose_named_arguments: its stage-12 part ------------------------------------------
// Compose's key names, SnackbarData's visuals, NavigationSuiteItem by name and NavigationSuiteScaffold(navigationItems
// =, navigationSuiteType =, modifier =). Its NavController, NavHost, `navOptions { popUpTo<T> { saveState } }`,
// `kompozable<T>(deepLinks =)`, `navigation<T>(startDestination =)`, NavDestination's hierarchy / hasRoute<T> and the
// deep link are stage 13's (the two items switch a state here, the content showing the screen the NavHost would);
// its desktop MenuBar (Menu, Item, CheckboxItem, RadioButtonItem, Separator, a submenu) is stage 15/16's (D7).

kompoz! {
    fun composeKeys(): List<Key> = listOf(Key.DirectionLeft, Key.MoveEnd, Key.Spacebar)

    @Composable
    fun ShellSnackbars(snackbars: SnackbarHostState, log: (String) -> Unit) {
        LaunchedEffect(Unit) {
            snackbars.showSnackbar("m")
            snackbars.showSnackbar("m", actionLabel = "Geri al")
        }
        val visuals = snackbars.currentSnackbarData?.visuals
        log("${visuals?.message} ${visuals?.actionLabel}")
    }

    @Composable
    fun AppShell() {
        var inLogin by remember { mutableStateOf(false) }
        Column {
            NavigationSuiteScaffold(
                navigationItems = {
                    NavigationSuiteItem(selected = !inLogin, onClick = { inLogin = false }, icon = { Icon(Icons.Filled.Home, contentDescription = "") },
                                        label = { Text("Ev") })
                    NavigationSuiteItem(selected = inLogin, onClick = { inLogin = true }, icon = { Icon(Icons.Filled.Settings, contentDescription = "") },
                                        label = { Text("Giriş") }, badge = { Badge { Text("1") } })
                },
                navigationSuiteType = NavigationSuiteType.NavigationRail,
                modifier = Modifier.weight(1f),
            ) {
                // (stage 13: the NavHost of the home route and the login graph)
                Text(if (inLogin) "giriş ekranı" else "ev ekranı")
            }
            Text(if (inLogin) "sekme giriş" else "sekme ev")
        }
    }
}

#[kompozable]
fn RustShellSnackbars(snackbars: SnackbarHostState, log: Rc<dyn Fn(String)>) {
    let shown = snackbars.clone();
    LaunchedEffect((), async move {
        shown.show_snackbar("m".into()).await;
        named_args!(shown.show_snackbar("m".into(), action_label = Some("Geri al".into()))).await;
    });
    let visuals = snackbars.current_snackbar_data().map(|d| d.visuals());
    log(format!(
        "{} {}",
        visuals.as_ref().map_or("null".to_string(), |v| v.message()),
        visuals.as_ref().and_then(|v| v.action_label()).unwrap_or_else(|| "null".into())
    ));
}

#[kompozable]
fn RustAppShell() {
    let in_login = remember(|| mutable_state_of(false));
    Column(move || {
        let (home, login) = (in_login.clone(), in_login.clone());
        let on = in_login.get();
        NavigationSuiteScaffold(
            navigation_items = move || {
                let (home, login) = (home.clone(), login.clone());
                NavigationSuiteItem(
                    selected = !on,
                    on_click = move || home.set(false),
                    icon = || VectorIcon(Icons::Filled::Home(), Some(String::new())),
                    label = slot(|| Text("rust Ev")),
                );
                NavigationSuiteItem(
                    selected = on,
                    on_click = move || login.set(true),
                    icon = || VectorIcon(Icons::Filled::Settings(), Some(String::new())),
                    label = slot(|| Text("rust Giriş")),
                    badge = slot(|| Badge(content = slot(|| Text("rust 1")))),
                );
            },
            navigation_suite_type = NavigationSuiteType::NavigationRail,
            modifier = Modifier.weight(1.0),
            // (stage 13: the NavHost of the home route and the login graph)
            content = move || Text(if on { "rust giriş ekranı" } else { "rust ev ekranı" }),
        );
        Text(if on { "rust sekme giriş" } else { "rust sekme ev" });
    });
}

fn app_shell_forms_take_compose_named_arguments() {
    use kompoz::ui::input::Key;
    // Compose's key names (the C++'s aliases of its own `Key::Left`, `End`, `Space`: Compose has only these).
    let keys = [Key::DirectionLeft, Key::MoveEnd, Key::Spacebar];
    assert_eq!(compose_keys(), keys);
    assert!(keys[0] != keys[1] && keys[1] != keys[2] && keys[0] != keys[2]);
    // SnackbarData's visuals (the C++ made a SnackbarData; Compose's is an interface, its data the host state's
    // current one): its message and action label, none and then one.
    let (snackbars, rust_snackbars) = (SnackbarHostState::new(), SnackbarHostState::new());
    let (kotlin, rust) = (Rc::new(RefCell::new(Vec::<String>::new())), Rc::new(RefCell::new(Vec::<String>::new())));
    let (s, rs, k, r) = (snackbars.clone(), rust_snackbars.clone(), kotlin.clone(), rust.clone());
    let shell = KompozTestRule::new(kompozable_lambda!(move || {
        let k = k.clone();
        ShellSnackbars(s.clone(), move |line| k.borrow_mut().push(line));
        let r = r.clone();
        RustShellSnackbars(rs.clone(), Rc::new(move |line| r.borrow_mut().push(line)));
    }));
    shell.wait_for_idle();
    for log in [&kotlin, &rust] {
        assert_eq!(log.borrow().last().unwrap(), "m null");
    }
    for state in [&snackbars, &rust_snackbars] {
        state.current_snackbar_data().expect("shown").dismiss();
    }
    shell.wait_for_idle();
    for log in [&kotlin, &rust] {
        assert_eq!(log.borrow().last().unwrap(), "m Geri al");
    }
    // The navigation suite of its own items, a rail.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Row(content = || {
                Box(modifier = Modifier.width(600.dp()), content = || AppShell());
                Box(modifier = Modifier.width(600.dp()), content = || RustAppShell());
            });
        }),
        IntSize::new(1200, 800),
    );
    rule.wait_for_idle();
    for prefix in ["", "rust "] {
        let text = |t: &str| format!("{prefix}{t}");
        assert!(
            rule.on_node_with_text(&text("ev ekranı")).exists() && rule.on_node_with_text(&text("sekme ev")).exists()
        );
        assert!(rule.on_node_with_text(&text("1")).exists());
        // (a rail: the items down its start, the content beside it)
        let (home, login) = (
            named_args!(rule.on_node_with_text(&text("Ev"), use_unmerged_tree = true)).get_unclipped_bounds_in_root(),
            named_args!(rule.on_node_with_text(&text("Giriş"), use_unmerged_tree = true))
                .get_unclipped_bounds_in_root(),
        );
        let screen = rule.on_node_with_text(&text("ev ekranı")).get_unclipped_bounds_in_root();
        assert!(login.top > home.bottom && screen.left >= home.right, "{home:?} {login:?} {screen:?}");
        rule.on_node_with_text(&text("Giriş")).perform_click();
        rule.wait_for_idle();
        rule.advance_time_by(1000);
        rule.wait_for_idle();
        assert!(rule.on_node_with_text(&text("giriş ekranı")).exists());
        assert!(rule.on_node_with_text(&text("sekme giriş")).exists());
    }
}

// ---- C++ text_styles_take_compose_named_arguments ----------------------------------------------------------------
// A typography style's copy(shadow =, textIndent =) and copy(fontWeight =) are TextStyles (the C++'s second a
// `Font`), Text(style =), basicMarquee(iterations =, velocity =) (the C++'s `velocity = 40.f`: Compose's Dp), autoSize
// TextAutoSize.StepBased, the measurer's style a typography's.

kompoz! {
    @Composable
    fun TextStyles() {
        Column {
            val measurer = rememberTextMeasurer()
            val shaded = MaterialTheme.typography.titleLarge.copy(shadow = Shadow(Color.Black, Offset(2f, 3f), 4f), textIndent = TextIndent(24.sp))
            val bolder = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Bold)
            Text("gölge", style = shaded)
            Text("kalın", style = bolder)
            Text("kayan yazı kayan yazı kayan yazı", modifier = Modifier.width(100.dp).basicMarquee(iterations = 3, velocity = 40.dp).testTag("kayan"))
            Text("sığ", modifier = Modifier.width(60.dp), maxLines = 1, autoSize = TextAutoSize.StepBased(8.sp, 40.sp, 1.sp))
            Text(if (measurer.measure("Kompoz", style = MaterialTheme.typography.titleMedium).size.width > 10) "ölçü var" else "ölçü yok")
        }
    }
}

#[kompozable]
fn RustTextStyles() {
    Column(|| {
        let measurer = remember_text_measurer();
        let title = MaterialTheme::typography().title_large;
        let shaded = named_args!(title.copy(
            shadow = Some(named_args!(Shadow(Color::Black, Offset::new(2.0, 3.0), 4.0))),
            text_indent = Some(named_args!(TextIndent(24.sp())))
        ));
        let bolder = named_args!(title.copy(font_weight = Some(FontWeight::Bold)));
        Text("rust gölge", style = shaded);
        Text("rust kalın", style = bolder);
        Text(
            "rust kayan yazı kayan yazı kayan yazı",
            modifier =
                Modifier.width(100.dp()).basic_marquee(iterations = 3, velocity = 40.dp()).test_tag("rust kayan"),
        );
        Text(
            "rust sığ",
            modifier = Modifier.width(60.dp()),
            max_lines = 1,
            auto_size = Some(<dyn TextAutoSize>::step_based(8.sp(), 40.sp(), 1.sp())),
        );
        let measured = named_args!(measurer.measure("Kompoz", style = MaterialTheme::typography().title_medium));
        Text(if measured.size.width > 10 { "rust ölçü var" } else { "rust ölçü yok" });
    });
}

/// The style the Text of `text` was laid out in.
fn laid_out_style(rule: &KompozTestRule, text: &str) -> TextStyle {
    use kompoz::ui::semantics::SemanticsActions;
    let mut layouts: Vec<TextLayoutResult> = Vec::new();
    rule.on_node_with_text(text).perform_semantics_action_with(SemanticsActions::GetTextLayoutResult, |f| {
        assert!(f(&mut layouts));
    });
    layouts[0].layout_input.style.clone()
}

fn text_styles_take_compose_named_arguments() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Row(content = || {
                Box(modifier = Modifier.width(400.dp()), content = || TextStyles());
                Box(modifier = Modifier.width(400.dp()), content = || RustTextStyles());
            });
        }),
        IntSize::new(800, 800),
    );
    let title = MaterialTheme::typography().title_large;
    for prefix in ["", "rust "] {
        let text = |t: &str| format!("{prefix}{t}");
        assert!(rule.on_node_with_text(&text("gölge")).exists() && rule.on_node_with_text(&text("kalın")).exists());
        assert!(rule.on_node_with_tag(&text("kayan")).exists());
        assert!(rule.on_node_with_text(&text("sığ")).exists() && rule.on_node_with_text(&text("ölçü var")).exists());
        // The copies: the title style with the shadow and indent given, or bold.
        let shaded = laid_out_style(&rule, &text("gölge"));
        assert_eq!(shaded.shadow, Some(Shadow::new(Color::Black, Offset::new(2.0, 3.0), 4.0, 0.0)));
        assert_eq!(shaded.text_indent, Some(TextIndent::new(24.sp(), 0.sp())));
        assert!(shaded.font_size == title.font_size && shaded.font_weight == title.font_weight);
        let bolder = laid_out_style(&rule, &text("kalın"));
        assert!(bolder.font_weight == Some(FontWeight::Bold) && bolder.font_size == title.font_size);
        // (the text inside the marquee, its tag after it, is measured as wide as it is: past the 100 dp it scrolls in)
        assert!(bounds(&rule, &text("kayan")).width() > 100.0);
    }
}

// ---- C++ gestures_take_compose_named_arguments --------------------------------------------------------------------
// draggable(orientation =, state =), dragAndDropTarget(shouldStartDragAndDrop =, target =) and Compose's
// DraggableAnchors { } builder (foundation's, no Material). The C++'s `DragAndDropTarget{.onDrop = …}` is Compose's
// `object : DragAndDropTarget { override fun onDrop(…) }` — object expressions are stage 14's: the target is made in
// Rust and given to the Kotlin composable.

kompoz! {
    @Composable
    fun GestureForms(x: MutableState<Float>, target: DragAndDropTarget) {
        val drag = rememberDraggableState { delta -> x.value = x.value + delta }
        Box(modifier = Modifier.size(56.dp).testTag("tutamaç").draggable(orientation = Orientation.Horizontal, state = drag))
        Box(modifier = Modifier.size(100.dp).testTag("hedef").dragAndDropTarget(shouldStartDragAndDrop = { e -> e.hasMimeType("text/plain") }, target = target))
    }

    fun twoAnchors(): DraggableAnchors<Int> = DraggableAnchors {
        1 at 0f
        2 at 120f
    }
}

#[kompozable]
fn RustGestureForms(x: MutableState<f32>, target: Rc<dyn DragAndDropTarget>) {
    let drag = remember_draggable_state(move |delta| x.set(x.peek() + delta));
    Box(modifier =
        Modifier.size(56.dp()).test_tag("rust tutamaç").draggable(orientation = Orientation::Horizontal, state = drag));
    // (a method of no defaults: no named form, its arguments in place)
    Box(modifier = Modifier
        .size(100.dp())
        .test_tag("rust hedef")
        .drag_and_drop_target(|e: &DragAndDropEvent| e.has_mime_type("text/plain"), target.clone()));
}

/// The C++'s target: the text dropped on it noted.
struct Dropped(Rc<RefCell<String>>);

impl DragAndDropTarget for Dropped {
    fn on_drop(&self, event: &DragAndDropEvent) -> bool {
        *self.0.borrow_mut() = event.text().to_string();
        true
    }
}

fn gestures_take_compose_named_arguments() {
    let (x, rust_x) = (mutable_state_of(0.0f32), mutable_state_of(0.0f32));
    let dropped = Rc::new(RefCell::new(String::new()));
    let target: Rc<dyn DragAndDropTarget> = Rc::new(Dropped(dropped.clone()));
    let (xs, rxs) = (x.clone(), rust_x.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (xs, rxs, target) = (xs.clone(), rxs.clone(), target.clone());
        Column(
            content = move || {
                GestureForms(xs.clone(), target.clone());
                RustGestureForms(rxs.clone(), target.clone());
            },
        );
    }));
    // (the C++'s swipe from 10 to 40 across the handle: a move a frame)
    for tag in ["tutamaç", "rust tutamaç"] {
        let y = bounds(&rule, tag).center().y;
        let (from, h) = (Offset::new(10.0, y), rule.host());
        h.pointer_down(from, -1, 0, kompoz::ui::input::PointerType::Touch);
        for i in 1..=10 {
            rule.advance_frame();
            h.pointer_move(from + Offset::new(3.0 * i as f32, 0.0), -1, 0);
        }
        h.pointer_up(from + Offset::new(30.0, 0.0), -1, 0);
        rule.wait_for_idle();
    }
    assert!(x.get() > 20.0 && rust_x.get() > 20.0, "{} {}", x.get(), rust_x.get());
    assert!(rule.on_node_with_tag("hedef").exists() && rule.on_node_with_tag("rust hedef").exists());
    assert!(dropped.borrow().is_empty());
    // The builder: the state's anchors of its own value type.
    let rust_anchors = DraggableAnchors(|a| {
        a.at(1, 0.0);
        a.at(2, 120.0);
    });
    assert!(rust_anchors.position_of(&2) == 120.0 && rust_anchors.size() == 2);
    let anchors = two_anchors();
    assert!(anchors.position_of(&2) == 120.0 && anchors.size() == 2);
}

// ---- C++ graphics_forms_take_compose_named_arguments: its Material part -------------------------------------------
// The wavy indicators by name and by place (the C++'s `WavyProgressIndicatorOptions{.strokeWidth = 2}` is M15's
// `stroke` and `trackStroke`), CircularWavyProgressIndicator(trackColor =), ContainedLoadingIndicator by name and by
// place (the C++'s `progress = 0.3f`: M15 takes a lambda), `MaterialShapes.Heart.toShape()` and a Morph(Heart, Pill)'s
// copies (Kotlin's: the same object; Rust's clone the same outlines). (Its stage-11 parts are above; its keyframes,
// ColorMatrix, list arithmetic and Modifier shadow / paint / blur / magnifier are the lower crates'.)

kompoz! {
    @Composable
    fun GraphicsFormsMaterial() {
        Column {
            LinearWavyProgressIndicator(progress = { 0.5f }, modifier = Modifier.width(100.dp).testTag("dalga"), stopSize = 0.dp)
            LinearWavyProgressIndicator({ 0.5f }, Modifier.width(80.dp).testTag("yerinde"), stroke = Stroke(width = 2f, cap = StrokeCap.Round),
                                        trackStroke = Stroke(width = 2f, cap = StrokeCap.Round))
            CircularWavyProgressIndicator(trackColor = Color.Gray)
            ContainedLoadingIndicator(progress = { 0.3f }, containerColor = Color.Yellow, modifier = Modifier.testTag("kap"))
            ContainedLoadingIndicator(Modifier.testTag("dönen"))
        }
    }

    @Composable
    fun HeartShapes(seen: (Shape, Shape) -> Unit) {
        val heart: Shape = MaterialShapes.Heart.toShape()
        val pill: Shape = MaterialShapes.Pill.toShape()
        seen(heart, pill)
    }

    fun morphCopiesMorphAlike(): Boolean {
        val morph = Morph(MaterialShapes.Heart, MaterialShapes.Pill)
        val copy = morph
        return copy.asCubics(0.5f) == morph.asCubics(0.5f)
    }
}

#[kompozable]
fn RustGraphicsFormsMaterial() {
    Column(|| {
        LinearWavyProgressIndicator(
            progress = || 0.5,
            modifier = Modifier.width(100.dp()).test_tag("rust dalga"),
            stop_size = 0.dp(),
        );
        LinearWavyProgressIndicator(
            || 0.5,
            Modifier.width(80.dp()).test_tag("rust yerinde"),
            stroke = Stroke(width = 2.0, cap = StrokeCap::Round),
            track_stroke = Stroke(width = 2.0, cap = StrokeCap::Round),
        );
        // (Compose's overloads without a progress: Rust's names for them)
        IndeterminateCircularWavyProgressIndicator(track_color = Color::Gray);
        ContainedLoadingIndicator(
            progress = || 0.3,
            container_color = Color::Yellow,
            modifier = Modifier.test_tag("rust kap"),
        );
        IndeterminateContainedLoadingIndicator(Modifier.test_tag("rust dönen"));
    });
}

fn graphics_forms_take_compose_named_arguments() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Row(content = || {
                Box(modifier = Modifier.width(200.dp()), content = || GraphicsFormsMaterial());
                Box(modifier = Modifier.width(200.dp()), content = || RustGraphicsFormsMaterial());
            });
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    rule.advance_time_by(1000);
    rule.wait_for_idle();
    for prefix in ["", "rust "] {
        let width = |t: &str| bounds(&rule, &format!("{prefix}{t}")).width();
        assert!(near(width("dalga"), 100.0) && near(width("yerinde"), 80.0));
        assert!(near(width("kap"), 48.0) && near(width("dönen"), 48.0));
    }
    // (the strokes given: 2 px wide, the thin wavy ones' in their rows 10 to 20 down; the track gray; the container
    // yellow — each twice, a syntax's each)
    let canvas = rule.draw();
    let thin = (0..canvas.paths.len()).filter(|&i| (10.0..20.0).contains(&canvas.paths[i].center().y));
    assert!(thin.clone().count() >= 4 && thin.into_iter().all(|i| canvas.path_strokes[i] == 2.0));
    assert!(canvas.path_colors.iter().filter(|c| **c == Color::Gray).count() >= 2);
    assert!(canvas.rect_colors.iter().filter(|c| **c == Color::Yellow).count() >= 2);
    // MaterialShapes' heart as a Shape: equal to another of it (its polygon and start angle), not the pill's; a
    // morph's copy morphs as it does (Kotlin's: the same object; Rust's clone: the same outlines).
    let heart = MaterialShapes::Heart();
    assert!(heart.to_shape(0) == heart.to_shape(0) && heart.to_shape(0) != MaterialShapes::Pill().to_shape(0));
    let kotlin: Rc<RefCell<Vec<Arc<dyn Shape>>>> = Rc::new(RefCell::new(Vec::new()));
    let k = kotlin.clone();
    let _shapes = KompozTestRule::new(kompozable_lambda!(move || {
        let k = k.clone();
        HeartShapes(move |a: Arc<dyn Shape>, b: Arc<dyn Shape>| *k.borrow_mut() = vec![a, b]);
    }));
    let (rust_heart, rust_pill): (Arc<dyn Shape>, Arc<dyn Shape>) =
        (Arc::new(heart.to_shape(0)), Arc::new(MaterialShapes::Pill().to_shape(0)));
    assert!(*kotlin.borrow()[0] == *rust_heart && *kotlin.borrow()[1] == *rust_pill);
    assert!(*kotlin.borrow()[0] != *rust_pill);
    let morph = Morph::new(heart.clone(), MaterialShapes::Pill().clone());
    let copy = morph.clone();
    assert!(copy.as_cubics(0.5) == morph.as_cubics(0.5) && morph_copies_morph_alike());
}

// ---- C++ sections_forms_take_compose_named_arguments --------------------------------------------------------------
// SwipeToDismissBox(state =, backgroundContent =) of `rememberSwipeToDismissBoxState(confirmValueChange =)` (M15's
// deprecated overload) in `key(id)`, SearchBar(state =, inputField =) with SearchBarDefaults.InputField(textFieldState
// =, searchBarState =, …) (M15's takes its `onSearch`), TooltipBox(positionProvider =, tooltip =, state =) with
// PlainTooltip (the C++'s `caretSize = TooltipDefaults.caretSize`: M15's `caretShape = TooltipDefaults.caretShape()`;
// its `rememberTooltipPositionProvider()` deprecated for the anchor position form written here), RichTooltip(title =,
// action =) { } (Compose's is the tooltip scope's, in a TooltipBox shown persistently: the C++ composed it on its
// own), clickable(interactionSource =, indication = null) { }, focusProperties { canFocus = false },
// WindowInsets.statusBars' top. (The C++'s doc names date and time picker dialogs its body doesn't compose.)

kompoz! {
    @Composable
    fun SectionsForms(tasks: SnapshotStateList<Int>) {
        val search = rememberSearchBarState()
        Column {
            for (id in tasks.toList()) {
                key(id) {
                    val state = rememberSwipeToDismissBoxState(confirmValueChange = { tasks.remove(id); true })
                    SwipeToDismissBox(state = state, backgroundContent = { Box(modifier = Modifier.fillMaxSize().background(Color.Red)) }) {
                        Text("görev $id", modifier = Modifier.fillMaxWidth().testTag("görev$id"))
                    }
                }
            }
            val query = rememberTextFieldState()
            SearchBar(state = search, inputField = {
                SearchBarDefaults.InputField(textFieldState = query, searchBarState = search, onSearch = {}, placeholder = { Text("Ara") },
                                             leadingIcon = { Icon(Icons.Filled.Search, contentDescription = "Ara") })
            }, modifier = Modifier.testTag("arama"))
            TooltipBox(positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
                       tooltip = { PlainTooltip(caretShape = TooltipDefaults.caretShape()) { Text("İpucu") } }, state = rememberTooltipState()) {
                IconButton(onClick = {}) { Icon(Icons.Filled.Info, contentDescription = "Bilgi") }
            }
            TooltipBox(positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Below),
                       tooltip = { RichTooltip(title = { Text("Başlık") }, action = { TextButton(onClick = {}) { Text("Tamam") } }) { Text("Metin") } },
                       state = rememberTooltipState(initialIsVisible = true, isPersistent = true)) {
                Icon(Icons.Filled.Info, contentDescription = "Zengin")
            }
            val source = remember { MutableInteractionSource() }
            Box(modifier = Modifier.size(40.dp).testTag("kart").clickable(interactionSource = source, indication = null) {})
            Box(modifier = Modifier.size(40.dp).focusProperties { canFocus = false }.testTag("odaksız"))
            Text("tasks ${tasks.size} · ${WindowInsets.statusBars.getTop(LocalDensity.current)}", modifier = Modifier.testTag("özet"))
        }
    }
}

#[kompozable]
fn RustSectionsForms(tasks: SnapshotStateList<i32>) {
    let search = remember_search_bar_state();
    Column(move || {
        for id in tasks.to_vec() {
            let tasks = tasks.clone();
            key(&id, move || {
                let tasks = tasks.clone();
                // (Compose's rememberSwipeToDismissBoxState(confirmValueChange =): Rust's name for that overload)
                let state = remember_swipe_to_dismiss_box_state_confirming(
                    confirm_value_change = move |_| {
                        tasks.remove(&id);
                        true
                    },
                );
                SwipeToDismissBox(
                    state = state,
                    background_content = || Box(modifier = Modifier.fill_max_size().background(Color::Red)),
                    content = move || {
                        Text(
                            format!("rust görev {id}"),
                            modifier = Modifier.fill_max_width().test_tag(format!("rust görev{id}")),
                        )
                    },
                );
            });
        }
        let query = remember_text_field_state();
        let (s, q) = (search.clone(), query.clone());
        SearchBar(
            state = search.clone(),
            input_field = move || {
                SearchBarDefaults.InputField(
                    text_field_state = q.clone(),
                    search_bar_state = s.clone(),
                    on_search = |_: String| {},
                    placeholder = slot(|| Text("rust Ara")),
                    leading_icon = slot(|| VectorIcon(Icons::Filled::Search(), Some("rust Ara".into()))),
                );
            },
            modifier = Modifier.test_tag("rust arama"),
        );
        TooltipBox(
            position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
            tooltip = |s: &dyn TooltipScope| {
                s.PlainTooltip(caret_shape = Some(TooltipDefaults::caret_shape()), content = || Text("rust İpucu"));
            },
            state = remember_tooltip_state(),
            content = || {
                IconButton(on_click = || {}, content = || VectorIcon(Icons::Filled::Info(), Some("rust Bilgi".into())))
            },
        );
        TooltipBox(
            position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Below),
            tooltip = |s: &dyn TooltipScope| {
                s.RichTooltip(
                    title = slot(|| Text("rust Başlık")),
                    action = slot(|| TextButton(on_click = || {}, content = || Text("rust Tamam"))),
                    text = || Text("rust Metin"),
                );
            },
            state = remember_tooltip_state(initial_is_visible = true, is_persistent = true),
            content = || VectorIcon(Icons::Filled::Info(), Some("rust Zengin".into())),
        );
        let source = remember(MutableInteractionSource::new);
        // (Compose's clickable(interactionSource =, indication =): Rust's name for that overload)
        Box(modifier = Modifier.size(40.dp()).test_tag("rust kart").clickable_indication(
            interaction_source = Some(source),
            indication = None,
            on_click = || {},
        ));
        Box(modifier = Modifier
            .size(40.dp())
            .focus_properties(|p: &mut kompoz::ui::focus::FocusProperties| p.can_focus = false)
            .test_tag("rust odaksız"));
        let top = WindowInsets::status_bars().get_top(&LocalDensity.current());
        Text(format!("rust tasks {} · {top}", tasks.size()), modifier = Modifier.test_tag("rust özet"));
    });
}

fn sections_forms_take_compose_named_arguments() {
    let (tasks, rust_tasks) = (mutable_state_list_of([1, 2]), mutable_state_list_of([1, 2]));
    let (t, rt) = (tasks.clone(), rust_tasks.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (t, rt) = (t.clone(), rt.clone());
            Row(content = move || {
                let t = t.clone();
                Box(modifier = Modifier.width(400.dp()), content = move || SectionsForms(t.clone()));
                let rt = rt.clone();
                Box(modifier = Modifier.width(400.dp()), content = move || RustSectionsForms(rt.clone()));
            });
        }),
        IntSize::new(800, 800),
    );
    rule.wait_for_idle();
    for prefix in ["", "rust "] {
        let text = |t: &str| format!("{prefix}{t}");
        assert!(rule.on_node_with_tag(&text("görev1")).exists() && rule.on_node_with_tag(&text("arama")).exists());
        assert!(rule.on_node_with_text(&text("Başlık")).exists());
        assert!(rule.on_node_with_text(&text("tasks 2 · 0")).exists() && rule.on_node_with_tag(&text("kart")).exists());
        assert!(rule.on_node_with_tag(&text("odaksız")).exists());
    }
    // (beyond the C++'s checks: a box swiped away asks, and its task goes)
    for prefix in ["", "rust "] {
        let row = bounds(&rule, &format!("{prefix}görev1"));
        let (from, h) = (Offset::new(row.right - 20.0, row.center().y), rule.host());
        h.pointer_down(from, -1, 0, kompoz::ui::input::PointerType::Touch);
        for i in 1..=20 {
            rule.advance_frame();
            h.pointer_move(from - Offset::new(15.0 * i as f32, 0.0), -1, 0);
        }
        h.pointer_up(from - Offset::new(300.0, 0.0), -1, 0);
        rule.wait_for_idle();
        assert!(!rule.on_node_with_tag(&format!("{prefix}görev1")).exists());
        assert!(rule.on_node_with_text(&format!("{prefix}tasks 1 · 0")).exists());
    }
}

// ---- C++ drawing_forms_take_compose_named_arguments ---------------------------------------------------------------
// drawText(measurer, text, topLeft =, style =) — its style Material's `typography.labelSmall` — and record(layer, size
// =) { } by name draw what the positional forms draw; a layer's properties (Compose's `layer.alpha`, of
// `rememberGraphicsLayer()`: the C++ made one on its own); onVisibilityChanged(minFractionVisible =) { } and
// draggable2D(state, startDragImmediately =) by name; Compose's Rect(left, top, right, bottom) and Rect(offset,
// size), CornerSize(percent) and CornerSize(px), kotlin.math, a PathMeasure's length, the zero values. (Its
// `PointerIcon::Move`: KompozR's own cursor, Compose's common PointerIcon has none — desktop's is AWT's
// `PointerIcon(Cursor(Cursor.MOVE_CURSOR))` —, checked in Rust only. C++ 7983's text part is in
// crates/kompoz-ui/tests/text.rs too, in Rust.)

kompoz! {
    fun drawingValues(): List<Boolean> {
        val path = Path()
        path.moveTo(0f, 0f)
        path.lineTo(10f, 0f)
        val measure = PathMeasure()
        measure.setPath(path, false)
        val box = Size(100f, 100f)
        return listOf(
            Rect(0f, -3f, 18f, 3f) == Rect(Offset(0f, -3f), Size(18f, 6f)),
            Rect(Offset(1f, 2f), Size(3f, 4f)) == Rect(1f, 2f, 4f, 6f),
            CornerSize(50).toPx(box, Density(1f)) == 50f && CornerSize(6f).toPx(box, Density(1f)) == 6f,
            // (the C++'s `sign(-3)` of an Int is Kotlin's `(-3).sign`, which kompoz-runtime's kt shims lack for Int:
            // Float's `sign(-3f)`)
            max(0f, 2f) == 2f && min(3, 1) == 1 && abs(-2.5f) == 2.5f && round(2.5f) == 2f && sign(-3f) == -1f && (-3).toFloat() == -3f,
            PI.toFloat() > 3.14f && PI.toFloat() < 3.15f && sqrt(16f) == 4f && sin(0f) == 0f,
            Offset.Zero == Offset(0f, 0f) && IntOffset.Zero == IntOffset(0, 0) && Size.Zero == Size(0f, 0f),
            // (Compose's measure.length)
            measure.length == 10f,
        )
    }

    // A layer's properties (Compose's `layer.alpha`, `layer.topLeft`), read and written.
    @Composable
    fun LayerProperties(seen: (Float, IntOffset) -> Unit) {
        val layer = rememberGraphicsLayer()
        layer.alpha = 0.5f
        layer.topLeft = IntOffset(0, 20)
        seen(layer.alpha, layer.topLeft)
    }

    @Composable
    fun Drawn(byName: Boolean, text: Boolean) {
        val measurer = rememberTextMeasurer()
        val layer = rememberGraphicsLayer()
        val style = MaterialTheme.typography.labelSmall
        Box(modifier = Modifier.size(100.dp, 50.dp).drawWithContent {
            if (byName) {
                record(layer, size = IntSize(100, 25)) { drawContent() }
                drawLayer(layer)
                if (text) drawText(measurer, "ölçek", topLeft = Offset(2f, 30f), style = style)
            } else {
                record(layer, IntSize(100, 25)) { drawContent() }
                drawLayer(layer)
                if (text) drawText(measurer, "ölçek", Offset(2f, 30f), style)
            }
        }) {
            Box(modifier = Modifier.size(100.dp, 25.dp).background(Color.White))
        }
    }

    @Composable
    fun SeenAndPulled(seen: (Boolean) -> Unit, pulled: MutableState<Offset>) {
        val state = rememberDraggable2DState { d -> pulled.value = pulled.value + d }
        Box(modifier = Modifier.size(200.dp).onVisibilityChanged(minFractionVisible = 0f) { v -> seen(v) }.draggable2D(state, startDragImmediately = true))
    }
}

fn rust_drawing_values() -> Vec<bool> {
    let mut path = Path::new();
    path.move_to(0.0, 0.0);
    path.line_to(10.0, 0.0);
    let mut measure = PathMeasure::new();
    measure.set_path(&path, false);
    let (b, d) = (Size::new(100.0, 100.0), Density::new(1.0, 1.0));
    let pi = std::f32::consts::PI;
    vec![
        Rect((0.0, -3.0, 18.0, 3.0)) == Rect((Offset::new(0.0, -3.0), Size::new(18.0, 6.0))),
        Rect((Offset::new(1.0, 2.0), Size::new(3.0, 4.0))) == Rect((1.0, 2.0, 4.0, 6.0)),
        CornerSize::percent(50).to_px(b, &d) == 50.0 && CornerSize::px(6.0).to_px(b, &d) == 6.0,
        0.0f32.max(2.0) == 2.0
            && [3, 1].into_iter().min() == Some(1)
            && (-2.5f32).abs() == 2.5
            && (-3i32).signum() == -1,
        (pi * 100.0).floor() == 314.0 && 16.0f32.sqrt() == 4.0 && 0.0f32.sin() == 0.0,
        Offset::Zero == Offset::new(0.0, 0.0)
            && IntOffset::Zero == IntOffset::new(0, 0)
            && Size::Zero == Size::new(0.0, 0.0),
        measure.length() == 10.0,
    ]
}

#[kompozable]
fn RustLayerProperties(seen: Rc<dyn Fn(f32, IntOffset)>) {
    let layer = remember_graphics_layer();
    layer.set_alpha(0.5);
    layer.set_top_left(IntOffset::new(0, 20));
    seen(layer.alpha(), layer.top_left());
}

#[kompozable]
fn RustDrawn(by_name: bool, text: bool) {
    let measurer = remember_text_measurer();
    let layer = remember_graphics_layer();
    let style = MaterialTheme::typography().label_small;
    Box(
        modifier = Modifier.size_wh(100.dp(), 50.dp()).draw_with_content(move |d| {
            if by_name {
                d.record(&layer, size = IntSize::new(100, 25), block = |c| c.draw_content());
                d.draw_layer(&layer);
                if text {
                    d.draw_text_measurer(&measurer, "ölçek", top_left = Offset::new(2.0, 30.0), style = style.clone());
                }
            } else {
                d.record_with(&layer, IntSize::new(100, 25), |c| c.draw_content());
                d.draw_layer(&layer);
                if !text {
                    return;
                }
                d.draw_text_measurer_with(
                    &measurer,
                    "ölçek",
                    Offset::new(2.0, 30.0),
                    style.clone(),
                    TextOverflow::Clip,
                    true,
                    usize::MAX,
                    Vec::new(),
                    Size::Unspecified,
                    kompoz::ui::draw::DefaultBlendMode,
                );
            }
        }),
        content = || Box(modifier = Modifier.size_wh(100.dp(), 25.dp()).background(Color::White)),
    );
}

#[kompozable]
fn RustSeenAndPulled(seen: Rc<dyn Fn(bool)>, pulled: MutableState<Offset>) {
    let state = remember_draggable2d_state(move |d| pulled.set(pulled.peek() + d));
    Box(modifier = Modifier
        .size(200.dp())
        .on_visibility_changed(min_fraction_visible = 0.0, callback = move |v| seen(v))
        .draggable2d(state, start_drag_immediately = true));
}

fn drawing_forms_take_compose_named_arguments() {
    assert!(drawing_values().iter().all(|b| *b), "{:?}", drawing_values());
    assert!(rust_drawing_values().iter().all(|b| *b), "{:?}", rust_drawing_values());
    // (Kotlin's `round` rounds half to even: 2.5 to 2, as the C++'s)
    assert_eq!(kompoz::kt::round(2.5f32), 2.0);
    let _ = kompoz::ui::platform::PointerIcon::Move;
    // A layer's properties, each syntax.
    for rust in [false, true] {
        let seen = Rc::new(Cell::new((0.0, IntOffset::Zero)));
        let s = seen.clone();
        let _layers = KompozTestRule::new(kompozable_lambda!(move || {
            let s = s.clone();
            if rust {
                RustLayerProperties(Rc::new(move |a: f32, at: IntOffset| s.set((a, at))));
            } else {
                LayerProperties(move |a: f32, at: IntOffset| s.set((a, at)));
            }
        }));
        assert_eq!(seen.get(), (0.5, IntOffset::new(0, 20)));
    }
    // By name and by place, each syntax: the same drawing.
    let drawn = |which: u8| {
        KompozTestRule::new(kompozable_lambda!(move || match which {
            0 => Drawn(true, false),
            1 => Drawn(false, false),
            2 => RustDrawn(true, false),
            3 => RustDrawn(false, false),
            4 => RustDrawn(true, true),
            5 => RustDrawn(false, true),
            6 => Drawn(true, true),
            _ => Drawn(false, true),
        }))
        .draw()
    };
    let recorded = drawn(0);
    assert!(!recorded.rects.is_empty() && recorded.texts.is_empty());
    for b in [drawn(1), drawn(2), drawn(3)] {
        assert!(recorded.rects == b.rects && recorded.order == b.order && b.texts.is_empty());
    }
    let a = drawn(4);
    assert!(a.rects == recorded.rects && a.texts == ["ölçek"]);
    for b in [drawn(5), drawn(6), drawn(7)] {
        assert!(a.rects == b.rects && a.texts == b.texts && a.order == b.order && a.baselines == b.baselines);
        assert!(a.text_styles == b.text_styles);
    }
    // (the label's style Material's: labelSmall's size)
    let small = MaterialTheme::typography().label_small.font_size.value();
    assert!((a.text_styles[0].font_size - small).abs() < 0.01);
    // Visible at once (none of it need be), pulled 40 across and 30 down from the press.
    for rust in [false, true] {
        let seen = Rc::new(Cell::new(None));
        let pulled = mutable_state_of(Offset::Zero);
        let (s, p) = (seen.clone(), pulled.clone());
        let rule = KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let s = s.clone();
                if rust {
                    RustSeenAndPulled(Rc::new(move |v| s.set(Some(v))), p.clone());
                } else {
                    SeenAndPulled(move |v: bool| s.set(Some(v)), p.clone());
                }
            }),
            IntSize::new(400, 800),
        );
        rule.wait_for_idle();
        assert_eq!(seen.get(), Some(true));
        let (from, h) = (Offset::new(50.0, 50.0), rule.host());
        h.pointer_down(from, -1, 0, kompoz::ui::input::PointerType::Touch);
        for i in 1..=10 {
            rule.advance_frame();
            h.pointer_move(from + Offset::new(4.0 * i as f32, 3.0 * i as f32), -1, 0);
        }
        h.pointer_up(from + Offset::new(40.0, 30.0), -1, 0);
        rule.wait_for_idle();
        let p = pulled.peek();
        assert!(p.x > 30.0 && p.y > 20.0, "{rust}: {p:?}");
    }
}

tests![
    shared_forms_take_compose_named_arguments,
    animated_forms_take_compose_named_arguments,
    animations_take_compose_named_arguments,
    state_forms_take_compose_named_arguments,
    scroll_forms_take_compose_named_arguments,
    layout_forms_take_compose_named_arguments,
    lists_pagers_and_scrollbars_take_compose_arguments,
    material_components_take_compose_arguments,
    theme_pieces_take_compose_named_arguments,
    containers_take_compose_named_arguments,
    expressive_buttons_take_compose_named_arguments,
    segmented_buttons_take_compose_named_arguments,
    tabs_and_fab_menu_take_compose_named_arguments,
    app_shell_forms_take_compose_named_arguments,
    text_styles_take_compose_named_arguments,
    gestures_take_compose_named_arguments,
    graphics_forms_take_compose_named_arguments,
    sections_forms_take_compose_named_arguments,
    drawing_forms_take_compose_named_arguments,
];
