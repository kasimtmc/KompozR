//! The C++ Kompoz's named-argument form tests that touch stage 10 (survey §3 Q), in both syntaxes: the Rust one's
//! `#[named]` calls in kompozable bodies and Kotlin's through `kompoz!`.
//!
//! - C++ 8100 (`state_forms_take_compose_named_arguments`): its `PaddingValues` forms (all; horizontal, vertical;
//!   start, top, end, bottom; one by name) and a ViewConfiguration's millis. (Its `kt::PaddingValues(top =, all =)`
//!   isn't a Compose overload; its Channel's ChannelResult, `produce`/`consumeEach` and list arithmetic are the
//!   coroutines' and kt shims' — stage 2 and 9's; Lifecycle's names are stage 13's.)
//! - C++ 8136 (`scroll_forms_take_compose_named_arguments`): `overscrollEffect = null` (none) on a LazyRow; Material's
//!   `Modifier.pullToRefresh(isRefreshing =, state =, onRefresh =)` by name on a Box of `rememberPullToRefreshState()`
//!   with `PullToRefreshDefaults.Indicator(state =, isRefreshing =, modifier =)` and `PullToRefreshDefaults.IndicatorBox(
//!   state =, isRefreshing =, modifier =, shape =) { LoadingIndicator(progress =, modifier =) }` in it (stage 12 WP5,
//!   WP2). (Its app bars are WP4a's;
//!   a VelocityTracker's copies sharing samples is Kotlin's reference semantics of a class KompozR has as a value — see
//!   stage10-followups.md.)
//! - C++ 8188 (`layout_forms_take_compose_named_arguments`): `Arrangement.spacedBy(space, alignment)`, `spring(…)` of
//!   a float with its damping and stiffness, `Modifier.onFirstVisible(minDurationMs =) { }`, a list's
//!   `getOrNull` with a nullable index, `String.repeat`, `Modifier.approachLayout(isMeasurementApproachInProgress =,
//!   approachMeasure =)` by name (its approach measure block Compose's: a `MeasureScope`-style one, its lookahead size
//!   in scope). (Its pane scaffolds are Material's adaptive ones, stage 12's.)
//! - C++ 11965 (`lists_pagers_and_carousels_take_compose_arguments`): a grid's `span = { GridItemSpan(maxLineSpan) }`,
//!   `items(n, span = { … })`, pager states with their count last and by name, `VerticalScrollbar(adapter =,
//!   modifier =)`, Material's `PullToRefreshBox(isRefreshing =, onRefresh =, modifier =) { }` (stage 12 WP5, WP2). (Its
//!   carousel is Material's, stage 12's; AsyncImage is stage 13's.)
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

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::foundation::lazy::grid::LazyGridItemSpanScope;
use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use kompoz::ui::platform::ViewConfiguration;
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
            PullToRefreshBox(isRefreshing = false, onRefresh = {}, modifier = Modifier.size(50.dp)) { Text("içerik") }
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
        assert!(rule.on_node_with_text(&format!("{prefix}içerik")).exists());
    }
    assert_eq!((pages.peek(), by_name.peek(), rust_pages.peek(), rust_by_name.peek()), (4, 1, 4, 1));
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

tests![
    shared_forms_take_compose_named_arguments,
    animated_forms_take_compose_named_arguments,
    animations_take_compose_named_arguments,
    state_forms_take_compose_named_arguments,
    scroll_forms_take_compose_named_arguments,
    layout_forms_take_compose_named_arguments,
    lists_pagers_and_scrollbars_take_compose_arguments,
];
