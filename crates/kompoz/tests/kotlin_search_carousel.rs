//! Material's search bars, rich tooltips and carousels in Kotlin's syntax, as a Compose user writes them: the C++
//! Kompoz's sources (kompozable_tests.cpp SectionsForms' rich tooltip; SectionsForms and ScrollForms are whole, their
//! search bars too, in kotlin_material_sources.rs) through
//! `kompoz!`, and material3's samples of those components from `.kt` files (kompoz_file!) — their `CarouselItemScope`
//! and `TooltipScope` receivers as Compose's.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;

// ==== search bars ===================================================================================================
// The C++ Kompoz's search tests (kompoz_tests.cpp: search_bar_expands_searches_and_closes,
// docked_search_bar_opens_in_place, search_bar_state_expands_full_screen_or_docked,
// full_screen_search_grows_out_of_its_bar_and_follows_the_back_gesture, top_search_bar_sits_alone_and_slides_away,
// app_bar_with_search_slides_away_with_the_content) written in Kotlin (kompozable_tests.cpp has no source for them),
// checked as crates/kompoz-material3/tests/search_bar.rs checks them (M15's values; the differences from the C++'s
// said there); material3's
// SearchBarSamples and each public search API by Kotlin's named arguments from kt/search_bar.kt.

// (what Kotlin's imports name, for the Rust the Kotlin becomes: the module's own `use`s)
use kompoz::ui::input::NestedScrollConnection;
use kompoz::ui::window::{DialogProperties, PopupPositionProvider, PopupProperties};

kompoz! {
    val Cities = listOf("Ankara", "Antalya", "Bursa", "İstanbul", "İzmir")

    // The cities starting with `query`, each a 48 dp row; a click picks it.
    @Composable
    fun CityRows(query: String, onPick: (String) -> Unit) {
        for (city in Cities) {
            if (city.startsWith(query)) {
                Text(city, Modifier.fillMaxWidth().height(48.dp).clickable { onPick(city) })
            }
        }
    }

    // (the C++'s query and expanded states the test's: hoisted, as Compose's state hoisting has them)
    @Composable
    fun SearchingCities(query: MutableState<String>, expanded: MutableState<Boolean>, onSearched: (String) -> Unit) {
        Column {
            SearchBar(
                inputField = {
                    SearchBarDefaults.InputField(
                        query = query.value,
                        onQueryChange = { query.value = it },
                        onSearch = {
                            onSearched(it)
                            expanded.value = false
                        },
                        expanded = expanded.value,
                        onExpandedChange = { expanded.value = it },
                        modifier = Modifier.testTag("field"),
                        placeholder = { Text("Şehir ara") },
                    )
                },
                expanded = expanded.value,
                onExpandedChange = { expanded.value = it },
                modifier = Modifier.fillMaxWidth().testTag("bar"),
            ) {
                CityRows(query.value) {
                    query.value = it
                    expanded.value = false
                }
            }
            Text("Sayfa", Modifier.testTag("page"))
        }
    }

    @Composable
    fun DockedCities(query: MutableState<String>, expanded: MutableState<Boolean>) {
        Column(Modifier.padding(20.dp)) {
            Text("Üstte", Modifier.height(100.dp))
            DockedSearchBar(
                inputField = {
                    SearchBarDefaults.InputField(
                        query = query.value,
                        onQueryChange = { query.value = it },
                        onSearch = {},
                        expanded = expanded.value,
                        onExpandedChange = { expanded.value = it },
                        modifier = Modifier.testTag("field"),
                        placeholder = { Text("Şehir ara") },
                    )
                },
                expanded = expanded.value,
                onExpandedChange = { expanded.value = it },
                modifier = Modifier.fillMaxWidth().testTag("bar"),
            ) {
                CityRows(query.value) {
                    query.value = it
                    expanded.value = false
                }
            }
            Text("Sayfa", Modifier.testTag("page"))
        }
    }

    // (the state and the text field's handed to the test as Compose tests keep a `lateinit var` of them)
    @Composable
    fun StateSearch(docked: Boolean, onState: (SearchBarState) -> Unit, onText: (TextFieldState) -> Unit, onSearched: (String) -> Unit) {
        val searchBarState = rememberSearchBarState()
        val textFieldState = rememberTextFieldState()
        val scope = rememberCoroutineScope()
        onState(searchBarState)
        onText(textFieldState)
        val inputField = @Composable { tag: String ->
            SearchBarDefaults.InputField(
                textFieldState = textFieldState,
                searchBarState = searchBarState,
                onSearch = {
                    onSearched(it)
                    scope.launch { searchBarState.animateToCollapsed() }
                },
                modifier = Modifier.testTag(tag),
                placeholder = { Text("Şehir ara") },
            )
        }
        Column(Modifier.padding(start = 16.dp, top = 100.dp, end = 16.dp)) {
            SearchBar(state = searchBarState, inputField = { inputField("bar-field") }, modifier = Modifier.fillMaxWidth().testTag("bar"))
            Text("Sayfa")
        }
        if (docked) {
            ExpandedDockedSearchBar(state = searchBarState, inputField = { inputField("view-field") }) {
                for (city in listOf("Ankara", "Antalya", "Bursa")) {
                    if (city.startsWith(textFieldState.text)) Text(city, Modifier.fillMaxWidth().height(48.dp))
                }
            }
        } else {
            ExpandedFullScreenSearchBar(state = searchBarState, inputField = { inputField("view-field") }) {
                for (city in listOf("Ankara", "Antalya", "Bursa")) {
                    if (city.startsWith(textFieldState.text)) Text(city, Modifier.fillMaxWidth().height(48.dp))
                }
            }
        }
    }

    // The bar at 40, 100, 320 x 56; wide: at 16, 10, 368 x 56.
    @Composable
    fun GrowingSearch(wide: Boolean, onState: (SearchBarState) -> Unit) {
        val searchBarState = rememberSearchBarState()
        val textFieldState = rememberTextFieldState()
        onState(searchBarState)
        Column(if (wide) Modifier.padding(start = 16.dp, top = 10.dp, end = 16.dp) else Modifier.padding(start = 40.dp, top = 100.dp, end = 40.dp)) {
            // (Compose's input field is 360 dp at least: given 320 here, the bar's own size)
            SearchBar(
                state = searchBarState,
                inputField = {
                    SearchBarDefaults.InputField(
                        textFieldState = textFieldState,
                        searchBarState = searchBarState,
                        onSearch = {},
                        modifier = Modifier.requiredWidthIn(min = 0.dp),
                        placeholder = { Text("Ara") },
                    )
                },
                modifier = Modifier.fillMaxWidth(),
            )
        }
        ExpandedFullScreenSearchBar(
            state = searchBarState,
            inputField = {
                SearchBarDefaults.InputField(
                    textFieldState = textFieldState,
                    searchBarState = searchBarState,
                    onSearch = {},
                    modifier = Modifier.testTag("açık"),
                    placeholder = { Text("Ara") },
                )
            },
        ) {
            Text("Sonuç", Modifier.testTag("sonuç"))
        }
    }

    // (the C++'s top search bar alone in a Scaffold's top bar over a list, a bottom app bar on the caller's state)
    @Suppress("DEPRECATION")
    @Composable
    fun TopSearch(
        wide: Boolean,
        onBehavior: (SearchBarScrollBehavior) -> Unit,
        onState: (SearchBarState) -> Unit,
        onBottom: (BottomAppBarState) -> Unit,
    ) {
        val scrollBehavior = SearchBarDefaults.enterAlwaysSearchBarScrollBehavior()
        val searchBarState = rememberSearchBarState()
        val textFieldState = rememberTextFieldState()
        val bottomState = rememberBottomAppBarState()
        val exit = BottomAppBarDefaults.exitAlwaysScrollBehavior(state = bottomState)
        onBehavior(scrollBehavior)
        onState(searchBarState)
        onBottom(bottomState)
        Box(Modifier.width(if (wide) 1000.dp else 400.dp).fillMaxHeight()) {
            Scaffold(
                modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection).nestedScroll(exit.nestedScrollConnection),
                topBar = {
                    TopSearchBar(
                        state = searchBarState,
                        inputField = {
                            SearchBarDefaults.InputField(
                                textFieldState = textFieldState,
                                searchBarState = searchBarState,
                                onSearch = {},
                                modifier = Modifier.testTag("hap"),
                                placeholder = { Text("Ara") },
                            )
                        },
                        scrollBehavior = scrollBehavior,
                    )
                },
                bottomBar = { BottomAppBar(actions = { Text("Alt") }, scrollBehavior = exit) },
            ) { padding ->
                LazyColumn(Modifier.fillMaxSize(), contentPadding = padding) {
                    items(100) { i -> Text("Satır $i", Modifier.height(50.dp).testTag("row$i")) }
                }
            }
        }
    }

    @Composable
    fun MailSearch(
        pinned: Boolean,
        onMenu: () -> Unit,
        onBehavior: (SearchBarScrollBehavior) -> Unit,
        onState: (SearchBarState) -> Unit,
        onList: (LazyListState) -> Unit,
    ) {
        val scrollBehavior = SearchBarDefaults.enterAlwaysSearchBarScrollBehavior()
        val searchBarState = rememberSearchBarState()
        val textFieldState = rememberTextFieldState()
        val listState = rememberLazyListState()
        onBehavior(scrollBehavior)
        onState(searchBarState)
        onList(listState)
        val inputField = @Composable {
            SearchBarDefaults.InputField(
                textFieldState = textFieldState,
                searchBarState = searchBarState,
                onSearch = {},
                placeholder = { Text("Postalarda ara") },
            )
        }
        Scaffold(
            modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
            topBar = {
                AppBarWithSearch(
                    state = searchBarState,
                    inputField = inputField,
                    modifier = Modifier.testTag("çubuk"),
                    navigationIcon = {
                        IconButton(onClick = onMenu) { Icon(Icons.Filled.Menu, contentDescription = "Menü") }
                    },
                    actions = { Icon(Icons.Filled.Person, contentDescription = "Hesap") },
                    scrollBehavior = if (pinned) null else scrollBehavior,
                )
            },
        ) { padding ->
            LazyColumn(Modifier.fillMaxSize(), state = listState, contentPadding = padding) {
                items(100) { i -> Text("Posta $i", Modifier.height(50.dp).testTag("row$i")) }
            }
        }
        ExpandedFullScreenSearchBar(state = searchBarState, inputField = inputField) { Text("Sonuçlar") }
    }
}

kompoz_file!("tests/kt/search_bar.kt");

fn sb_near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn sb_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
}

fn sb_text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    rule.on_node_with_text(text).get_unclipped_bounds_in_root()
}

fn sb_tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, kompoz::ui::input::PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(p, -1, 0);
    rule.advance_frame();
}

fn sb_slow_drag(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    sb_drag_held(rule, from, to, frames);
    rule.host().pointer_up(to, -1, 0);
    rule.advance_frame();
}

/// A drag held at its end (not let go): `sb_release` lets it go.
fn sb_drag_held(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, kompoz::ui::input::PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        let t = i as f32 / frames as f32;
        h.pointer_move(from + (to - from) * t, -1, 0);
    }
    rule.advance_time_by(200);
}

fn sb_release(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_up(at, -1, 0);
    rule.wait_for_idle();
}

/// Where a scroll behavior settles when let go at `offset` of `limit` (Compose's `settleSearchBar` without a
/// velocity): shown under half way, else gone.
fn sb_settled(offset: f32, limit: f32) -> f32 {
    if offset / limit < 0.5 { 0.0 } else { limit }
}

fn sb_escape(rule: &KompozTestRule) -> bool {
    rule.host().key_down(kompoz::ui::input::Key::Escape, kompoz::ui::input::KeyModifiers::default())
}

fn sb_touch_slop() -> f32 {
    kompoz::ui::platform::ViewConfiguration::default().touch_slop
}

/// What a composable handed out as it composed (a Compose test's `lateinit var`).
type SbSlot<T> = std::rc::Rc<std::cell::RefCell<Option<T>>>;

fn sb_get<T: Clone>(s: &SbSlot<T>) -> T {
    s.borrow().clone().expect("set by the composition")
}

fn sb_launch(rule: &KompozTestRule, f: impl std::future::Future<Output = ()> + 'static) {
    kompoz::coroutines::CoroutineScope::new(rule.host().dispatcher()).launch(f);
}

/// C++ `search_bar_expands_searches_and_closes` in Kotlin: M15's `SearchBar(inputField, expanded,
/// onExpandedChange, …) { … }` and `SearchBarDefaults.InputField(query, …)`.
fn search_bar_expands_searches_and_closes() {
    let query = mutable_state_of(String::new());
    let expanded = mutable_state_of(false);
    let searched = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let rule = {
        let (query, expanded, searched) = (query.clone(), expanded.clone(), searched.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let searched = searched.clone();
                SearchingCities(query.clone(), expanded.clone(), move |q: String| searched.borrow_mut().push(q));
            }),
            IntSize::new(400, 800),
        )
    };
    rule.on_node_with_text("Şehir ara").assert_exists();
    rule.on_node_with_content_description("Search").assert_exists();
    assert!(!rule.on_node_with_text("Ankara").exists());
    assert!(
        sb_near(sb_bounds(&rule, "bar").height(), 64.0, 0.5) && sb_near(sb_bounds(&rule, "page").top, 64.0, 0.5),
        "{:?}",
        sb_bounds(&rule, "bar")
    );
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    assert!(expanded.peek() && rule.on_node_with_text("Bursa").exists() && rule.host().overlay_count() == 0);
    rule.on_node_with_tag("field").assert_is_focused();
    assert!(sb_near(sb_bounds(&rule, "bar").height(), 800.0, 0.5), "{:?}", sb_bounds(&rule, "bar"));
    rule.on_node_with_tag("page").assert_is_not_displayed();
    rule.host().text_input("An");
    rule.wait_for_idle();
    assert!(
        query.peek() == "An" && rule.on_node_with_text("Antalya").exists() && !rule.on_node_with_text("Bursa").exists()
    );
    rule.host().ime_action(kompoz::ui::platform::ImeAction::Search);
    rule.wait_for_idle();
    assert_eq!(*searched.borrow(), vec!["An".to_string()]);
    assert!(!expanded.peek() && !rule.on_node_with_text("Antalya").exists());
    rule.on_node_with_text("An").assert_exists();
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Antalya").perform_click();
    rule.wait_for_idle();
    assert!(query.peek() == "Antalya" && !expanded.peek());
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    assert!(expanded.peek());
    sb_escape(&rule);
    rule.wait_for_idle();
    assert!(!expanded.peek() && sb_near(sb_bounds(&rule, "bar").height(), 64.0, 0.5));
}

/// C++ `docked_search_bar_opens_in_place` in Kotlin: M15's `DockedSearchBar(inputField, expanded, …) { … }`.
fn docked_search_bar_opens_in_place() {
    let query = mutable_state_of(String::new());
    let expanded = mutable_state_of(false);
    let rule = {
        let (query, expanded) = (query.clone(), expanded.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || DockedCities(query.clone(), expanded.clone())),
            IntSize::new(400, 800),
        )
    };
    let bar = sb_bounds(&rule, "bar");
    let page_y = sb_bounds(&rule, "page").top;
    assert!(sb_near(bar.width(), 360.0, 0.5) && sb_near(bar.height(), 56.0, 0.5) && sb_near(page_y, bar.bottom, 0.5));
    assert!(!rule.on_node_with_text("Ankara").exists() && rule.host().overlay_count() == 0);
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    assert!(expanded.peek() && rule.host().overlay_count() == 0);
    rule.on_node_with_tag("field").assert_is_focused();
    let ankara = sb_text_bounds(&rule, "Ankara");
    assert!(
        sb_near(ankara.left, bar.left, 0.5)
            && sb_near(ankara.width(), bar.width(), 0.5)
            && sb_near(ankara.top, bar.bottom + 1.0, 0.5)
    );
    assert!(sb_near(sb_bounds(&rule, "bar").height(), 56.0 + 241.0, 0.5), "{:?}", sb_bounds(&rule, "bar"));
    assert!(sb_near(sb_bounds(&rule, "page").top, page_y + 241.0, 0.5));
    rule.host().text_input("An");
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("Antalya").exists() && !rule.on_node_with_text("Bursa").exists());
    rule.on_node_with_text("Antalya").perform_click();
    rule.wait_for_idle();
    assert!(query.peek() == "Antalya" && !expanded.peek());
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    assert!(expanded.peek());
    sb_tap(&rule, Offset::new(200.0, 790.0)); // (M15: a tap outside leaves it open)
    rule.wait_for_idle();
    assert!(expanded.peek());
    sb_escape(&rule);
    rule.wait_for_idle();
    assert!(!expanded.peek() && !rule.on_node_with_text("Ankara").exists());
}

/// C++ `search_bar_state_expands_full_screen_or_docked` in Kotlin: M15's `SearchBar(state, …)` with
/// `ExpandedFullScreenSearchBar` / `ExpandedDockedSearchBar`.
fn search_bar_state_expands_full_screen_or_docked() {
    let (state, text): (SbSlot<SearchBarState>, SbSlot<TextFieldState>) = Default::default();
    let searched = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let docked = mutable_state_of(false);
    let rule = {
        let (state, text, searched, docked) = (state.clone(), text.clone(), searched.clone(), docked.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let (state, text, searched) = (state.clone(), text.clone(), searched.clone());
                StateSearch(
                    docked.get(),
                    move |s: SearchBarState| *state.borrow_mut() = Some(s),
                    move |t: TextFieldState| *text.borrow_mut() = Some(t),
                    move |q: String| searched.borrow_mut().push(q),
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let host = rule.host();
    let s = sb_get(&state);
    assert!(
        s.current_value() == SearchBarValue::Collapsed
            && s.target_value() == SearchBarValue::Collapsed
            && s.progress() == 0.0
    );
    assert!(
        host.overlay_count() == 0
            && rule.on_node_with_text("Şehir ara").exists()
            && !rule.on_node_with_text("Bursa").exists()
    );
    let pill = sb_bounds(&rule, "bar").center();
    sb_tap(&rule, pill);
    rule.advance_time_by(100);
    assert!(s.target_value() == SearchBarValue::Expanded, "{s:?}");
    assert!(s.progress() > 0.1 && s.progress() < 0.9 && host.overlay_count() == 1, "{s:?}");
    {
        let canvas = rule.draw();
        let p = s.progress();
        assert!(canvas.layer_alphas.iter().any(|a| sb_near(*a, p, 0.05)), "{p} {:?}", canvas.layer_alphas);
    }
    rule.wait_for_idle();
    assert!(
        s.current_value() == SearchBarValue::Expanded
            && s.progress() == 1.0
            && rule.on_node_with_text("Bursa").exists()
    );
    rule.on_node_with_tag("view-field").assert_is_focused();
    rule.on_node_with_tag("bar-field").assert_exists();
    host.text_input("An");
    rule.wait_for_idle();
    assert!(
        sb_get(&text).text() == "An"
            && rule.on_node_with_text("Antalya").exists()
            && !rule.on_node_with_text("Bursa").exists()
    );
    host.ime_action(kompoz::ui::platform::ImeAction::Search);
    rule.advance_time_by(64);
    assert_eq!(*searched.borrow(), vec!["An".to_string()]);
    assert!(s.target_value() == SearchBarValue::Collapsed && host.overlay_count() == 1);
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Collapsed && s.progress() == 0.0 && host.overlay_count() == 0);
    rule.on_node_with_tag("bar-field").assert_text_equals_with(&["An"], true);
    rule.on_node_with_tag("bar-field").perform_click();
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Expanded);
    sb_escape(&rule);
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Collapsed && host.overlay_count() == 0);
    let ss = s.clone();
    sb_launch(&rule, async move { ss.snap_to(1.0).await });
    rule.advance_frame();
    rule.advance_frame();
    assert!(s.current_value() == SearchBarValue::Expanded && s.progress() == 1.0 && host.overlay_count() == 1);
    let ss = s.clone();
    sb_launch(&rule, async move { ss.snap_to(0.0).await });
    rule.wait_for_idle();
    assert!(host.overlay_count() == 0);
    // Docked: over the collapsed bar; a tap outside collapses it.
    docked.set(true);
    rule.wait_for_idle();
    let bar = sb_bounds(&rule, "bar");
    rule.on_node_with_tag("bar-field").perform_click();
    rule.wait_for_idle();
    assert!(host.overlay_count() == 1 && s.current_value() == SearchBarValue::Expanded);
    let first = sb_text_bounds(&rule, "Ankara");
    assert!(
        sb_near(first.left, bar.left, 0.5)
            && sb_near(first.width(), 360.0, 0.5)
            && sb_near(first.top, bar.top + 56.0 + 1.0, 0.5),
        "{bar:?} {first:?}"
    );
    sb_tap(&rule, Offset::new(200.0, 750.0));
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Collapsed && host.overlay_count() == 0);
}

/// C++ `full_screen_search_grows_out_of_its_bar_and_follows_the_back_gesture` in Kotlin: M15's
/// `ExpandedFullScreenSearchBar` grows from the collapsed bar to the window (on the desktop a completed back
/// dismisses it: see search_bar.rs).
fn full_screen_search_grows_out_of_its_bar_and_follows_the_back_gesture() {
    let state: SbSlot<SearchBarState> = Default::default();
    let wide = mutable_state_of(false);
    let rule = {
        let (state, wide) = (state.clone(), wide.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let state = state.clone();
                GrowingSearch(wide.get(), move |s: SearchBarState| *state.borrow_mut() = Some(s));
            }),
            IntSize::new(400, 800),
        )
    };
    let s = sb_get(&state);
    // (the clock by hand: the checks on the way see the view growing and shrinking)
    rule.main_clock().set_auto_advance(false);
    let field = || sb_bounds(&rule, "açık");
    let ss = s.clone();
    sb_launch(&rule, async move { ss.animate_to_expanded().await });
    rule.advance_time_by(150);
    let p = s.progress();
    assert!(p > 0.2 && p < 0.95, "{p}");
    let a = field();
    let view_width = 320.0 + (400.0 - 320.0) * p;
    let field_width = 320.0 + (view_width - 320.0) * p;
    assert!(sb_near(a.width(), field_width, 0.5) && sb_near(a.center().x, 200.0, 0.5), "{a:?} {p}");
    assert!(sb_near(a.top, 100.0 * (1.0 - p) + 8.0 * p, 0.5), "{a:?} {p}");
    assert!(sb_near(sb_bounds(&rule, "sonuç").top, a.bottom + 8.0 * p + 1.0, 0.5));
    common::settle(&rule);
    let f = field();
    assert!(
        sb_near(f.left, 0.0, 0.5)
            && sb_near(f.top, 8.0, 0.5)
            && sb_near(f.width(), 400.0, 0.5)
            && sb_near(sb_bounds(&rule, "sonuç").top, 8.0 + 56.0 + 8.0 + 1.0, 0.5)
    );
    let back = |x: f32, y: f32, progress: f32, edge| kompoz::ui::back::BackEventCompat {
        touch_x: x,
        touch_y: y,
        progress,
        swipe_edge: edge,
        frame_time_millis: 0,
    };
    rule.host().back_started(back(10.0, 400.0, 0.0, kompoz::ui::back::SwipeEdge::Left));
    rule.host().back_progressed(back(60.0, 600.0, 0.5, kompoz::ui::back::SwipeEdge::Left));
    rule.advance_frame();
    assert!(sb_near(field().left, 0.0, 0.5) && sb_near(field().width(), 400.0, 0.5));
    rule.host().back_cancelled();
    rule.advance_frame();
    assert!(sb_near(field().left, 0.0, 0.5) && s.target_value() == SearchBarValue::Expanded);
    rule.host().back_started(back(390.0, 400.0, 0.0, kompoz::ui::back::SwipeEdge::Right));
    rule.host().back_progressed(back(340.0, 400.0, 0.5, kompoz::ui::back::SwipeEdge::Right));
    assert!(rule.host().back_invoked());
    rule.advance_time_by(100);
    let q = s.progress();
    assert!(s.target_value() == SearchBarValue::Collapsed && q > 0.05 && q < 0.95, "{q}");
    let h = field();
    let view_width = 320.0 + (400.0 - 320.0) * q;
    assert!(sb_near(h.width(), 320.0 + (view_width - 320.0) * q, 0.5) && sb_near(h.center().x, 200.0, 0.5));
    common::settle(&rule);
    assert!(s.current_value() == SearchBarValue::Collapsed && rule.host().overlay_count() == 0);
    let ss = s.clone();
    sb_launch(&rule, async move { ss.animate_to_expanded().await });
    common::settle(&rule);
    assert!(sb_near(field().left, 0.0, 0.5) && sb_near(field().width(), 400.0, 0.5));
    // A bar wider and near the top: the view grows from it the same way.
    let ss = s.clone();
    sb_launch(&rule, async move { ss.snap_to(0.0).await });
    wide.set(true);
    common::settle(&rule);
    let ss = s.clone();
    sb_launch(&rule, async move { ss.animate_to_expanded().await });
    rule.advance_time_by(150);
    let p = s.progress();
    let a = field();
    let view_width = 368.0 + (400.0 - 368.0) * p;
    assert!(sb_near(a.width(), 368.0 + (view_width - 368.0) * p, 0.5) && sb_near(a.center().x, 200.0, 0.5));
    assert!(sb_near(a.top, 10.0 * (1.0 - p) + 8.0 * p, 0.5), "{a:?}");
    common::settle(&rule);
}

/// C++ `top_search_bar_sits_alone_and_slides_away` in Kotlin: M15's deprecated `TopSearchBar` with
/// `SearchBarDefaults.enterAlwaysSearchBarScrollBehavior()` in a Scaffold over a list, a BottomAppBar on the
/// caller's state leaving once the content scrolls.
fn top_search_bar_sits_alone_and_slides_away() {
    let (behavior, search): (SbSlot<std::rc::Rc<dyn SearchBarScrollBehavior>>, SbSlot<SearchBarState>) =
        Default::default();
    let bottom: SbSlot<std::rc::Rc<dyn BottomAppBarState>> = Default::default();
    let wide = mutable_state_of(false);
    let rule = {
        let (behavior, search, bottom, wide) = (behavior.clone(), search.clone(), bottom.clone(), wide.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let (behavior, search, bottom) = (behavior.clone(), search.clone(), bottom.clone());
                TopSearch(
                    wide.get(),
                    move |b: std::rc::Rc<dyn SearchBarScrollBehavior>| *behavior.borrow_mut() = Some(b),
                    move |s: SearchBarState| *search.borrow_mut() = Some(s),
                    move |b: std::rc::Rc<dyn BottomAppBarState>| *bottom.borrow_mut() = Some(b),
                );
            }),
            IntSize::new(1000, 800),
        )
    };
    let pill = || sb_bounds(&rule, "hap");
    let b = sb_get(&behavior);
    let p = pill();
    assert!(
        sb_near(p.left, 20.0, 0.5)
            && sb_near(p.top, 4.0, 0.5)
            && sb_near(p.width(), 360.0, 0.5)
            && sb_near(p.height(), 56.0, 0.5),
        "{p:?}"
    );
    assert!(sb_near(sb_bounds(&rule, "row0").top, 64.0, 0.5) && sb_near(b.scroll_offset_limit(), -64.0, 0.5));
    let moved = 40.0 - sb_touch_slop();
    sb_drag_held(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 360.0), 20);
    assert!(sb_near(b.scroll_offset(), -moved, 0.5) && sb_near(pill().top, 4.0 - moved, 0.5));
    assert!(sb_near(sb_bounds(&rule, "row0").top, 64.0 - moved, 0.5));
    let bottom_bar = sb_get(&bottom);
    assert!(bottom_bar.height_offset() == 0.0); // the content hasn't moved yet
    sb_release(&rule, Offset::new(200.0, 360.0));
    assert!(sb_near(b.scroll_offset(), sb_settled(-moved, -64.0), 0.5), "{}", b.scroll_offset());
    // On until the content scrolls: the bottom bar, on the caller's state, leaves too.
    sb_slow_drag(&rule, Offset::new(200.0, 600.0), Offset::new(200.0, 300.0), 20);
    rule.wait_for_idle();
    assert!(sb_near(b.scroll_offset(), -64.0, 0.5) && bottom_bar.height_offset() != 0.0);
    sb_slow_drag(&rule, Offset::new(200.0, 300.0), Offset::new(200.0, 700.0), 20);
    sb_slow_drag(&rule, Offset::new(200.0, 300.0), Offset::new(200.0, 700.0), 20);
    rule.wait_for_idle();
    assert!(sb_near(b.scroll_offset(), 0.0, 0.5));
    rule.on_node_with_tag("hap").perform_click();
    rule.wait_for_idle();
    assert!(sb_get(&search).target_value() == SearchBarValue::Expanded);
    wide.set(true);
    rule.wait_for_idle();
    assert!(sb_near(pill().width(), 360.0, 0.5) && sb_near(pill().left, (1000.0 - 360.0) / 2.0, 0.5));
    rule.host().set_system_insets(kompoz::ui::platform::SystemInsets {
        status_bars: kompoz::ui::platform::Insets { left: 0, top: 24, right: 0, bottom: 0 },
        ..Default::default()
    });
    rule.wait_for_idle();
    assert!(sb_near(pill().top, 24.0 + 4.0, 0.5), "{:?}", pill());
}

/// C++ `app_bar_with_search_slides_away_with_the_content` in Kotlin: M15's `AppBarWithSearch` with a navigation
/// icon, actions and `enterAlwaysSearchBarScrollBehavior()`.
fn app_bar_with_search_slides_away_with_the_content() {
    let (behavior, search, list_slot): (
        SbSlot<std::rc::Rc<dyn SearchBarScrollBehavior>>,
        SbSlot<SearchBarState>,
        SbSlot<LazyListState>,
    ) = Default::default();
    let menus = std::rc::Rc::new(std::cell::Cell::new(0));
    let pinned = mutable_state_of(false);
    let rule = {
        let (behavior, search, list_slot, menus, pinned) =
            (behavior.clone(), search.clone(), list_slot.clone(), menus.clone(), pinned.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let (behavior, search, list_slot, menus) =
                    (behavior.clone(), search.clone(), list_slot.clone(), menus.clone());
                MailSearch(
                    pinned.get(),
                    move || menus.set(menus.get() + 1),
                    move |b: std::rc::Rc<dyn SearchBarScrollBehavior>| *behavior.borrow_mut() = Some(b),
                    move |s: SearchBarState| *search.borrow_mut() = Some(s),
                    move |l: LazyListState| *list_slot.borrow_mut() = Some(l),
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let b = sb_get(&behavior);
    let list = sb_get(&list_slot);
    let bar = sb_bounds(&rule, "çubuk");
    let menu = rule.on_node_with_content_description("Menü").get_unclipped_bounds_in_root();
    let account = rule.on_node_with_content_description("Hesap").get_unclipped_bounds_in_root();
    let pill = rule.on_node_with_content_description("Search").get_unclipped_bounds_in_root();
    assert!(
        sb_near(bar.height(), 64.0, 0.5)
            && sb_near(b.scroll_offset_limit(), -64.0, 0.5)
            && sb_near(sb_bounds(&rule, "row0").top, 64.0, 0.5),
        "{bar:?}"
    );
    assert!(sb_near(pill.height(), 56.0, 0.5) && sb_near(pill.top, 4.0, 0.5), "{pill:?}");
    assert!(
        sb_near(pill.left, 4.0 + 48.0 + 8.0, 0.5)
            && sb_near(menu.center().x, 4.0 + 24.0, 0.5)
            && sb_near(menu.center().y, 32.0, 0.5),
        "{pill:?} {menu:?}"
    );
    assert!(sb_near(pill.right, account.left - 8.0, 0.5) && sb_near(account.right, bar.right - 4.0, 0.5));
    let moved = 40.0 - sb_touch_slop();
    sb_drag_held(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 360.0), 20);
    assert!(
        sb_near(b.scroll_offset(), -moved, 0.5)
            && list.first_visible_item_index() == 0
            && list.first_visible_item_scroll_offset() == 0
    );
    assert!(
        sb_near(sb_bounds(&rule, "çubuk").height(), 64.0 - moved, 0.5)
            && sb_near(sb_bounds(&rule, "row0").top, 64.0 - moved, 0.5)
    );
    let pill_now = rule.on_node_with_content_description("Search").get_unclipped_bounds_in_root();
    assert!(sb_near(pill_now.top, 4.0 - moved, 0.5), "{pill_now:?}");
    sb_release(&rule, Offset::new(200.0, 360.0));
    assert!(sb_near(b.scroll_offset(), sb_settled(-moved, -64.0), 0.5), "{}", b.scroll_offset());
    sb_slow_drag(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 200.0), 20);
    rule.wait_for_idle();
    assert!(sb_near(b.scroll_offset(), -64.0, 0.5) && sb_near(sb_bounds(&rule, "çubuk").height(), 0.0, 0.5));
    let index = list.first_visible_item_index();
    let offset = list.first_visible_item_scroll_offset();
    sb_drag_held(&rule, Offset::new(200.0, 300.0), Offset::new(200.0, 340.0), 20);
    assert!(
        sb_near(b.scroll_offset(), -64.0 + moved, 0.5)
            && list.first_visible_item_index() == index
            && list.first_visible_item_scroll_offset() == offset
    );
    sb_release(&rule, Offset::new(200.0, 340.0));
    sb_slow_drag(&rule, Offset::new(200.0, 300.0), Offset::new(200.0, 400.0), 20);
    rule.wait_for_idle();
    assert!(sb_near(b.scroll_offset(), 0.0, 0.5));
    rule.on_node_with_content_description("Menü").perform_click();
    let s = sb_get(&search);
    assert!(menus.get() == 1 && s.target_value() == SearchBarValue::Collapsed);
    rule.on_node_with_text("Postalarda ara").perform_click();
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Expanded && rule.on_node_with_text("Sonuçlar").exists());
    // Without a scroll behavior it stays.
    sb_escape(&rule);
    pinned.set(true);
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Collapsed);
    sb_slow_drag(&rule, Offset::new(200.0, 600.0), Offset::new(200.0, 400.0), 20);
    assert!(sb_near(sb_bounds(&rule, "çubuk").height(), 64.0, 0.5));
}

/// SearchBarSamples.kt's `SimpleSearchBarSample` from kt/search_bar.kt, checked as search_bar.rs checks its Rust
/// one; its icons' tooltip boxes show their tooltips on a long press.
fn simple_search_bar_sample() {
    let rule = KompozTestRule::with_viewport(SimpleSearchBarSample, IntSize::new(400, 800));
    rule.on_node_with_content_description("Search").assert_exists();
    rule.on_node_with_content_description("Mic").assert_exists();
    assert!(!rule.on_node_with_text("Suggestion 3").exists());
    // (the mic's tooltip box: its tooltip on a long press)
    let mic = rule.on_node_with_content_description("Mic").get_unclipped_bounds_in_root().center();
    rule.host().pointer_down(mic, -1, 0, kompoz::ui::input::PointerType::Touch);
    rule.advance_time_by(600);
    rule.host().pointer_up(mic, -1, 0);
    rule.advance_frame();
    rule.on_node_with_text("Mic").assert_exists();
    rule.advance_time_by(1600);
    rule.wait_for_idle();
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Suggestion 3").assert_exists();
    assert_eq!(rule.on_all_nodes_with_content_description("Back").fetch_semantics_nodes().len(), 2);
    rule.on_node_with_text("Suggestion 3").perform_click();
    rule.wait_for_idle();
    assert!(!rule.on_node_with_text("Suggestion 0").exists() && rule.host().overlay_count() == 0);
    rule.on_node_with_content_description("Search").assert_text_equals_with(&["Suggestion 3"], true);
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    rule.on_all_nodes_with_content_description("Back").on_last().perform_click();
    rule.wait_for_idle();
    assert!(rule.host().overlay_count() == 0);
}

/// SearchBarSamples.kt's `FullScreenSearchBarScaffoldSample` (its Scaffold stood in for).
fn full_screen_search_bar_scaffold_sample() {
    let rule = KompozTestRule::with_viewport(FullScreenSearchBarScaffoldSample, IntSize::new(400, 800));
    rule.on_node_with_content_description("Menu").assert_exists();
    rule.on_node_with_content_description("Account").assert_exists();
    rule.on_node_with_text("Text 0").assert_exists();
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Suggestion 0").assert_exists();
    assert!(!rule.on_node_with_content_description("Menu").exists()); // (animated: the side icons slid away)
    rule.on_node_with_text("Suggestion 2").perform_click();
    rule.wait_for_idle();
    assert!(rule.host().overlay_count() == 0 && rule.on_node_with_content_description("Menu").exists());
}

/// SearchBarSamples.kt's `DockedSearchBarScaffoldSample` (its Scaffold stood in for).
fn docked_search_bar_scaffold_sample() {
    let rule = KompozTestRule::with_viewport(DockedSearchBarScaffoldSample, IntSize::new(400, 800));
    rule.on_node_with_text("Text 0").assert_exists();
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Suggestion 0").assert_exists();
    assert!(rule.on_node_with_content_description("Menu").exists() && rule.host().overlay_count() == 1);
    rule.on_node_with_text("Suggestion 1").perform_click();
    rule.wait_for_idle();
    assert!(rule.host().overlay_count() == 0 && !rule.on_node_with_text("Suggestion 0").exists());
}

/// Each public search API by Kotlin's named arguments (kt/search_bar.kt `SearchBarsByName`): the colors and
/// defaults as given and as the tokens are, the states, the saver, the scroll behavior; each bar form composed, its
/// field searching.
fn search_bars_take_compose_named_arguments() {
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let form = mutable_state_of(0);
    let rule = {
        let (seen, form) = (seen.clone(), form.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let s = seen.clone();
                SearchBarsByName(form.get(), move |line: String| s.borrow_mut().push(line));
            }),
            IntSize::new(400, 800),
        )
    };
    let has = |line: &str| seen.borrow().iter().any(|l| l == line);
    for line in [
        "colors true true true",
        "app bar true true",
        "contained true",
        "copy true",
        "made true true",
        "app made true true",
        "defaults 0.0.dp 0.0.dp 0.0.dp 56.0.dp 2.0.dp true true true true true true true",
        "state Collapsed Collapsed 0.0 false Collapsed",
        "behavior 0.0 0.0",
        "made state Expanded 1.0",
    ] {
        assert!(has(line), "{line}: {:?}", seen.borrow());
    }
    assert!(seen.borrow().iter().any(|l| l.starts_with("insets ")));
    rule.on_node_with_tag("durum").assert_exists();
    rule.on_node_with_tag("alan").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("tam ekran").assert_exists();
    rule.host().text_input("kedi");
    rule.host().ime_action(kompoz::ui::platform::ImeAction::Search);
    rule.wait_for_idle();
    assert!(has("searched kedi"), "{:?}", seen.borrow());
    sb_escape(&rule);
    rule.wait_for_idle();
    form.set(1);
    rule.wait_for_idle();
    for tag in ["çubuk", "üst"] {
        rule.on_node_with_tag(tag).assert_exists();
    }
    rule.on_node_with_content_description("Hesap").assert_exists();
    form.set(2);
    rule.wait_for_idle();
    rule.on_node_with_tag("aralıklı").assert_exists();
    for (form_now, tag) in [(3, "genişleyen"), (4, "yerleşik"), (5, "eski"), (6, "eski yerleşik")] {
        form.set(form_now);
        rule.wait_for_idle();
        rule.on_node_with_tag(tag).assert_exists();
        rule.on_node_with_content_description("Search").perform_click();
        rule.wait_for_idle();
        rule.on_node_with_text("öneri").assert_exists();
        sb_escape(&rule);
        rule.wait_for_idle();
        assert!(!rule.on_node_with_text("öneri").exists(), "{tag}");
    }
}

// ==== search bars end ===============================================================================================

// ==== tooltips ======================================================================================================
// The C++ Kompoz's rich tooltip tests (kompoz_tests.cpp: rich_tooltip_stays_until_a_tap_outside,
// tooltips_as_compose_1_4) written in Kotlin (kompozable_tests.cpp has no source for them), checked as
// crates/kompoz-material3/tests/snackbars_tooltips.rs' "(WP7) rich tooltips" section checks them (M15's values);
// kompozable_tests.cpp SectionsForms' RichTooltip line; material3's four rich TooltipSamples and the rich tooltip's
// API by Kotlin's named arguments from kt/tooltips.kt. (SectionsForms' TooltipBox/PlainTooltip part is in
// kotlin_lists_sheets.rs, WP5's.)

kompoz! {
    // A rich tooltip's box: an info icon its anchor, the tooltip a title, a text and an action that dismisses it
    // (`tag` on the tooltip); its state handed to the test (a Compose test's `lateinit var`).
    @Composable
    fun RichTip(text: String, tag: String, onState: (TooltipState) -> Unit) {
        val state = rememberTooltipState(isPersistent = true)
        onState(state)
        TooltipBox(
            positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
            tooltip = {
                RichTooltip(
                    modifier = Modifier.testTag(tag),
                    title = { Text("Yedekleme") },
                    action = { TextButton(onClick = { state.dismiss() }) { Text("Anladım") } },
                ) {
                    Text(text)
                }
            },
            state = state,
            hasAction = true,
        ) {
            Icon(Icons.Filled.Info, contentDescription = "Bilgi")
        }
    }

    @Composable
    fun RichTipAt(top: Dp, text: String, onState: (TooltipState) -> Unit) {
        Column(Modifier.padding(top = top)) {
            RichTip(text, "kart", onState)
        }
    }

    @Composable
    fun Centered(content: @Composable () -> Unit) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { content() }
    }

    // kompozable_tests.cpp SectionsForms' `RichTooltip(title =, action =) { … }`: the C++ calls it bare; M15's is a
    // TooltipScope extension, so it is written in a tooltip box's `tooltip` lambda, shown from the start.
    @Kompozable
    fun SectionsFormsRichTooltip() {
        TooltipBox(
            positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Below),
            tooltip = {
                RichTooltip(title = { Text("Başlık") }, action = { TextButton(onClick = {}) { Text("Tamam") } }) { Text("Metin") }
            },
            state = rememberTooltipState(initialIsVisible = true, isPersistent = true),
        ) {
            Icon(Icons.Filled.Info, contentDescription = "Bilgi")
        }
    }
}

kompoz_file!("tests/kt/tooltips.kt");

/// What a composable handed out of its tooltip's state.
type TipSlot = std::rc::Rc<std::cell::RefCell<Option<std::rc::Rc<dyn TooltipState>>>>;

fn tip_state(h: &TipSlot) -> std::rc::Rc<dyn TooltipState> {
    h.borrow().clone().expect("a tooltip state")
}

fn rich_tip_rule(top: f32, text: &'static str, h: &TipSlot) -> KompozTestRule {
    let hh = h.clone();
    KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let hh = hh.clone();
            RichTipAt(top.dp(), text, move |s: std::rc::Rc<dyn TooltipState>| *hh.borrow_mut() = Some(s));
        }),
        IntSize::new(400, 800),
    )
}

/// Where the first line of `text` was drawn: its start and baseline.
fn tip_baseline_of(canvas: &kompoz::test::RecordingCanvas, text: &str) -> Offset {
    let i = canvas.texts.iter().position(|t| text.starts_with(t.as_str()) && !t.is_empty());
    canvas.baselines[i.unwrap_or_else(|| panic!("{text:?} not drawn: {:?}", canvas.texts))]
}

/// C++ `rich_tooltip_stays_until_a_tap_outside` in Kotlin: a persistent rich tooltip shown by a long press stays (no
/// room above: below the anchor) until its action dismisses it or a tap lands outside it.
fn rich_tooltip_stays_until_a_tap_outside() {
    let h: TipSlot = Default::default();
    let rule = rich_tip_rule(20.0, "Fotoğraflar her gece yedeklenir.", &h);
    let anchor = rule.on_node_with_content_description("Bilgi").get_unclipped_bounds_in_root();
    rule.host().pointer_down(anchor.center(), -1, 0, kompoz::ui::input::PointerType::Touch);
    rule.advance_time_by(500);
    rule.host().pointer_up(anchor.center(), -1, 0);
    rule.advance_time_by(3000);
    assert!(tip_state(&h).is_visible() && sb_text_bounds(&rule, "Yedekleme").top > anchor.bottom);
    rule.on_node_with_text("Anladım").perform_click();
    assert!(!tip_state(&h).is_visible());
    rule.wait_for_idle();
    let s = tip_state(&h);
    sb_launch(&rule, async move { s.show(MutatePriority::Default).await });
    rule.wait_for_idle();
    assert!(tip_state(&h).is_visible());
    rule.host().pointer_down(Offset::new(380.0, 780.0), -1, 0, kompoz::ui::input::PointerType::Touch);
    rule.host().pointer_up(Offset::new(380.0, 780.0), -1, 0);
    rule.wait_for_idle();
    assert!(!tip_state(&h).is_visible());
}

/// C++ `tooltips_as_compose_1_4` in Kotlin, the rich tooltip (M15's values, 1.4's): growing and fading in; the
/// title's first baseline 28 dp from the card's top, 16 dp in; the text's 24 dp under the title; 3 dp of elevation;
/// still there while it fades out, then gone.
fn tooltips_as_compose_1_4() {
    let h: TipSlot = Default::default();
    let rule = rich_tip_rule(300.0, "Her gece", &h);
    let s = tip_state(&h);
    // (the clock by hand: the checks on the way see the tooltip coming and going)
    rule.main_clock().set_auto_advance(false);
    sb_launch(&rule, async move { s.show(MutatePriority::Default).await });
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    let coming = rule.draw();
    let fading = coming.layer_alphas.iter().any(|a| *a > 0.01 && *a < 0.99);
    let growing = coming.transforms.iter().any(|m| m.values[0] > 0.8 && m.values[0] < 0.99);
    assert!(fading && growing, "{:?} {:?}", coming.layer_alphas, coming.transforms);
    common::settle(&rule);
    let card = sb_bounds(&rule, "kart");
    let shown = rule.draw();
    let title_baseline = tip_baseline_of(&shown, "Yedekleme");
    let title_bounds = sb_text_bounds(&rule, "Yedekleme");
    assert!(
        sb_near(title_baseline.y, card.top + 28.0, 0.5) && sb_near(title_baseline.x, card.left + 16.0, 0.5),
        "{title_baseline:?} {card:?}"
    );
    let text_baseline = tip_baseline_of(&shown, "Her gece");
    assert!(sb_near(text_baseline.y, title_bounds.bottom + 24.0, 0.5), "{text_baseline:?} {title_bounds:?}");
    let up = shown
        .elevation_shadows
        .iter()
        .filter(|e| sb_near(e.bounds.center().x, card.center().x, 2.0))
        .map(|e| e.elevation())
        .fold(0.0f32, f32::max);
    assert!(sb_near(up, 3.0, 1e-4), "{up}");
    tip_state(&h).dismiss();
    rule.advance_frame();
    rule.advance_frame();
    rule.on_node_with_tag_unmerged("kart").assert_exists();
    common::settle(&rule);
    assert!(!rule.on_node_with_tag_unmerged("kart").exists());
}

/// kompozable_tests.cpp SectionsForms' rich tooltip (C++ `sections_forms_take_compose_named_arguments`): its title,
/// text and action shown.
fn sections_forms_rich_tooltip() {
    let rule = KompozTestRule::with_viewport(SectionsFormsRichTooltip, IntSize::new(400, 800));
    rule.wait_for_idle();
    rule.on_node_with_text("Başlık").assert_exists();
    rule.on_node_with_text("Metin").assert_exists();
    rule.on_node_with_text("Tamam").assert_exists();
}

/// A sample at the window's middle (room above its anchor), as snackbars_tooltips.rs shows its Rust ones.
fn centered(sample: usize) -> KompozTestRule {
    KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            Centered(move || match sample {
                0 => RichTooltipSample(),
                1 => RichTooltipWithCaretSample(),
                2 => RichTooltipWithCustomCaretSample(),
                _ => RichTooltipWithManualInvocationSample(),
            })
        }),
        IntSize::new(400, 800),
    )
}

/// TooltipSamples.kt's rich tooltips from kt/tooltips.kt, checked as snackbars_tooltips.rs checks its Rust ones:
/// shown by a long press above the anchor, staying until the action; the carets' card one path, the caret over the
/// 4 dp gap; the manual one shown by its button.
fn rich_tooltip_samples() {
    for (i, caret) in [(0, 0.0), (1, 8.0), (2, 16.0)] {
        let rule = centered(i);
        let anchor = rule.on_node_with_content_description("Localized Description").get_unclipped_bounds_in_root();
        rule.host().pointer_down(anchor.center(), -1, 0, kompoz::ui::input::PointerType::Touch);
        rule.advance_time_by(600);
        rule.host().pointer_up(anchor.center(), -1, 0);
        rule.advance_time_by(3000);
        let title = sb_text_bounds(&rule, rich_tooltip_subhead_text);
        rule.on_node_with_text(rich_tooltip_text).assert_exists();
        assert!(title.bottom < anchor.top, "{i}");
        if caret > 0.0 {
            let canvas = rule.draw();
            let container = light_color_scheme().surface_container;
            let at = (0..canvas.paths.len())
                .find(|k| canvas.path_colors[*k] == container && canvas.path_strokes[*k] == 0.0)
                .expect("the card");
            let path = canvas.paths[at];
            let room_top = anchor.top - 4.0;
            assert!(
                path.width() <= 320.0 + 0.5 && sb_near(path.bottom, room_top - 4.0 + caret, 1.0),
                "{i}: {path:?} {anchor:?}"
            );
        }
        rule.on_node_with_text(rich_tooltip_action_text).perform_click();
        rule.wait_for_idle();
        assert!(!rule.on_node_with_text(rich_tooltip_subhead_text).exists(), "{i}");
    }
    let rule = centered(3);
    rule.on_node_with_text("Display tooltip").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text(rich_tooltip_text).assert_exists();
    rule.on_node_with_text(rich_tooltip_action_text).perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text(rich_tooltip_text).assert_does_not_exist();
}

/// The rich tooltip's API by Kotlin's named arguments (kt/tooltips.kt `RichTooltipsByName`): the colors as given,
/// copied and as the theme's, the defaults, the tooltip shown from the start by the deprecated rich position
/// provider (start aligned above its anchor), its action dismissing it.
fn rich_tooltips_take_compose_named_arguments() {
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let rule = {
        let seen = seen.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let s = seen.clone();
                RichTooltipsByName(move |line: String| s.borrow_mut().push(line));
            }),
            IntSize::new(400, 800),
        )
    };
    rule.wait_for_idle();
    for line in ["colors true true true", "themed true true", "defaults 320.0.dp true"] {
        assert!(seen.borrow().iter().any(|l| l == line), "{line}: {:?}", seen.borrow());
    }
    let anchor = rule.on_node_with_content_description("Bilgi").get_unclipped_bounds_in_root();
    let tip = sb_bounds(&rule, "zengin");
    assert!(sb_near(tip.left, anchor.left, 0.5) && sb_near(tip.bottom, anchor.top - 4.0, 0.5), "{tip:?} {anchor:?}");
    rule.on_node_with_text("Metin").assert_exists();
    rule.on_node_with_text("Tamam").perform_click();
    rule.wait_for_idle();
    assert!(!rule.on_node_with_text("Başlık").exists());
}

// ==== tooltips end ==================================================================================================

// ==== carousels =====================================================================================================

// The C++ Kompoz's kompozable_tests.cpp has no carousel source: material3's CarouselSamples from kt/carousel.kt (their
// `Modifier.maskClip(…)` / `maskBorder(…)` and `carouselItemDrawInfo` through the item lambda's CarouselItemScope
// receiver, the multi-aspect sample's through MultiAspectCarouselScope's), checked as
// crates/kompoz-material3/tests/carousel.rs checks its Rust ones, and each public API by Kotlin's named arguments.

mod carousel_kt {
    use kompoz::prelude::*;
    use kompoz::ui::back::BackHandler;

    // The samples' Android resources (`R.drawable.carousel_image_N`, `R.string.carousel_image_N_description`) as
    // KompozR's (Compose Multiplatform's): tests/composeResources' `Res`, which the Kotlin syntax maps `R.*` to.
    compose_resources!(dir = "tests/composeResources");

    /// The samples' local `data class CarouselItem(val id: Int, @DrawableRes val imageResId: Int, @StringRes val
    /// contentDescriptionResId: Int)` (and the multi-aspect sample's, with `val mainAxisSize: Dp`), its resource
    /// ids CMP's resources: Kotlin's data classes come with stage 14. Its properties are getters, as Kotlin's.
    #[derive(Clone, PartialEq, Debug)]
    pub struct CarouselItem {
        id: i32,
        image_res_id: DrawableResource,
        content_description_res_id: StringResource,
        main_axis_size: Dp,
    }

    #[named(type)]
    pub fn CarouselItem(
        id: i32,
        image_res_id: DrawableResource,
        content_description_res_id: StringResource,
        #[default(Dp(0.0))] main_axis_size: Dp,
    ) -> CarouselItem {
        CarouselItem { id, image_res_id, content_description_res_id, main_axis_size }
    }

    impl CarouselItem {
        // (Kotlin's property, which the samples don't read)
        #[allow(dead_code)]
        pub fn id(&self) -> i32 {
            self.id
        }
        pub fn image_res_id(&self) -> DrawableResource {
            self.image_res_id.clone()
        }
        pub fn content_description_res_id(&self) -> StringResource {
            self.content_description_res_id.clone()
        }
        pub fn main_axis_size(&self) -> Dp {
            self.main_axis_size
        }
    }

    kompoz_file!("tests/kt/carousel.kt");
}

use carousel_kt::*;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

fn image_bounds(rule: &KompozTestRule, i: i32) -> Rect {
    rule.on_node_with_content_description(&format!("Carousel image {i}")).get_unclipped_bounds_in_root()
}

fn tap(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_down(at, -1, 0, kompoz::ui::input::PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(at, -1, 0);
    rule.wait_for_idle();
}

/// HorizontalMultiBrowseCarouselSample in Kotlin: tapping the third image glides it into focus.
fn carousel_samples_multi_browse() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| HorizontalMultiBrowseCarouselSample()),
        IntSize::new(412, 800),
    );
    rule.wait_for_idle();
    let before = image_bounds(&rule, 3);
    tap(&rule, Offset::new(before.center().x, 100.0));
    assert!(image_bounds(&rule, 3).left < before.left - 100.0, "{before:?}");
}

/// HorizontalUncontainedCarouselSample in Kotlin: items of the item width and spacing (194).
fn carousel_samples_uncontained() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| HorizontalUncontainedCarouselSample()),
        IntSize::new(500, 800),
    );
    rule.wait_for_idle();
    let first = image_bounds(&rule, 1);
    assert!(near(first.width(), 194.0) && near(first.height(), 205.0), "{first:?}");
}

/// HorizontalCenteredHeroCarouselSample in Kotlin: tapping the third image centers it.
fn carousel_samples_centered_hero() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| HorizontalCenteredHeroCarouselSample()),
        IntSize::new(412, 800),
    );
    rule.wait_for_idle();
    let before = image_bounds(&rule, 3);
    tap(&rule, Offset::new(before.center().x, 100.0));
    let after = image_bounds(&rule, 3);
    assert!((after.center().x - 206.0).abs() <= 1.0, "{before:?} {after:?}");
}

/// FadingHorizontalMultiBrowseCarouselSample in Kotlin: stacked items of two bordered images, the others a chip.
fn carousel_samples_fading() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| FadingHorizontalMultiBrowseCarouselSample()),
        IntSize::new(412, 800),
    );
    rule.wait_for_idle();
    assert_eq!(rule.on_all_nodes_with_content_description("Carousel image 2").fetch_semantics_nodes().len(), 2);
    rule.on_node_with_text("Image 0").assert_exists();
    let canvas = rule.draw();
    for color in [Color::Red, Color::Magenta, Color::Green] {
        assert!(canvas.mask_colors.contains(&color), "{:?}", canvas.mask_colors);
    }
}

/// CarouselWithShowAllButtonSample in Kotlin: "Show all" shows the grid; back, the carousel.
fn carousel_samples_show_all() {
    let rule =
        KompozTestRule::with_viewport(kompozable_lambda!(|| CarouselWithShowAllButtonSample()), IntSize::new(412, 800));
    rule.wait_for_idle();
    rule.on_node_with_text("Show all").perform_click();
    rule.wait_for_idle();
    assert!(!rule.on_node_with_text("Show all").exists());
    for i in 1..=5 {
        rule.on_node_with_content_description(&format!("Carousel image {i}")).assert_exists();
    }
    assert!(rule.host().back_invoked());
    rule.wait_for_idle();
    rule.on_node_with_text("Show all").assert_exists();
}

/// MultiAspectCarouselLazyRowSample in Kotlin: images of their own widths, the one entering at the end masked.
fn carousel_samples_multi_aspect() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| MultiAspectCarouselLazyRowSample()),
        IntSize::new(412, 800),
    );
    rule.wait_for_idle();
    let (first, second) = (image_bounds(&rule, 1), image_bounds(&rule, 2));
    assert!(near(first.width(), 305.0) && near(second.width(), 205.0) && near(second.left, 16.0 + 305.0 + 8.0));
    let canvas = rule.draw();
    assert!(canvas.clip_shapes.iter().any(|k| k.bounds.width() < 204.0 && k.bounds.width() > 50.0));
}

kompoz! {
    // Each public carousel API by Kotlin's named arguments.
    @Composable
    fun CarouselsByName(seen: (String) -> Unit) {
        val state = rememberCarouselState(initialItem = 1, itemCount = { 6 })
        Column {
            HorizontalMultiBrowseCarousel(
                state = state,
                preferredItemWidth = 120.dp,
                modifier = Modifier.width(400.dp).height(80.dp).testTag("browse"),
                itemSpacing = 4.dp,
                flingBehavior = CarouselDefaults.multiBrowseFlingBehavior(state = state),
                userScrollEnabled = true,
                minSmallItemWidth = CarouselDefaults.MinSmallItemSize,
                maxSmallItemWidth = CarouselDefaults.MaxSmallItemSize,
                contentPadding = PaddingValues(horizontal = 8.dp),
            ) { index ->
                val shape = rememberMaskShape(shape = RoundedCornerShape(4.dp))
                Box(Modifier.fillMaxSize().background(Color.Blue, shape).testTag("b$index"))
            }
            HorizontalUncontainedCarousel(
                state = rememberCarouselState(itemCount = { 3 }),
                itemWidth = 100.dp,
                modifier = Modifier.width(400.dp).height(80.dp),
                itemSpacing = 4.dp,
                flingBehavior = CarouselDefaults.noSnapFlingBehavior(),
                userScrollEnabled = false,
                contentPadding = PaddingValues(0.dp),
            ) { index ->
                Box(Modifier.fillMaxSize().maskClip(shape = RectangleShape).maskBorder(border = BorderStroke(1.dp, Color.Red), shape = RectangleShape).testTag("u$index"))
            }
            HorizontalCenteredHeroCarousel(
                state = rememberCarouselState(initialItem = 0) { 5 },
                modifier = Modifier.width(400.dp).height(80.dp),
                maxItemWidth = 160.dp,
                itemSpacing = 4.dp,
                flingBehavior = CarouselDefaults.singleAdvanceFlingBehavior(state = state, snapAnimationSpec = spring(stiffness = Spring.StiffnessLow)),
                minSmallItemWidth = 40.dp,
                maxSmallItemWidth = 56.dp,
            ) { index ->
                seen("hero $index ${carouselItemDrawInfo.maxSize > 0f || carouselItemDrawInfo.minSize >= 0f}")
                Box(Modifier.fillMaxSize().testTag("h$index"))
            }
        }
        seen("current ${state.currentItem} ${state.isScrollInProgress}")
    }
}

/// Each public carousel API by Kotlin's named arguments: the states at their items, the items composed in their
/// scopes.
fn carousels_take_compose_named_arguments() {
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let s = seen.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            CarouselsByName(move |line: String| s.borrow_mut().push(line));
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    assert!(seen.borrow().iter().any(|l| l == "current 1 false"), "{:?}", seen.borrow());
    assert!(seen.borrow().iter().any(|l| l.starts_with("hero 0")));
    for tag in ["b1", "u0", "h0"] {
        rule.on_node_with_tag(tag).assert_exists();
    }
}

// ==== carousels end =================================================================================================

fn main() {
    common::run(&[
        // (search bars)
        ("search_bar_expands_searches_and_closes", search_bar_expands_searches_and_closes),
        ("docked_search_bar_opens_in_place", docked_search_bar_opens_in_place),
        ("search_bar_state_expands_full_screen_or_docked", search_bar_state_expands_full_screen_or_docked),
        (
            "full_screen_search_grows_out_of_its_bar_and_follows_the_back_gesture",
            full_screen_search_grows_out_of_its_bar_and_follows_the_back_gesture,
        ),
        ("top_search_bar_sits_alone_and_slides_away", top_search_bar_sits_alone_and_slides_away),
        ("app_bar_with_search_slides_away_with_the_content", app_bar_with_search_slides_away_with_the_content),
        ("simple_search_bar_sample", simple_search_bar_sample),
        ("full_screen_search_bar_scaffold_sample", full_screen_search_bar_scaffold_sample),
        ("docked_search_bar_scaffold_sample", docked_search_bar_scaffold_sample),
        ("search_bars_take_compose_named_arguments", search_bars_take_compose_named_arguments),
        // (tooltips)
        ("rich_tooltip_stays_until_a_tap_outside", rich_tooltip_stays_until_a_tap_outside),
        ("tooltips_as_compose_1_4", tooltips_as_compose_1_4),
        ("sections_forms_rich_tooltip", sections_forms_rich_tooltip),
        ("rich_tooltip_samples", rich_tooltip_samples),
        ("rich_tooltips_take_compose_named_arguments", rich_tooltips_take_compose_named_arguments),
        // (carousels)
        ("carousel_samples_multi_browse", carousel_samples_multi_browse),
        ("carousel_samples_uncontained", carousel_samples_uncontained),
        ("carousel_samples_centered_hero", carousel_samples_centered_hero),
        ("carousel_samples_fading", carousel_samples_fading),
        ("carousel_samples_show_all", carousel_samples_show_all),
        ("carousel_samples_multi_aspect", carousel_samples_multi_aspect),
        ("carousels_take_compose_named_arguments", carousels_take_compose_named_arguments),
    ]);
}
