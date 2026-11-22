//! Material's search bars as Compose's material3 has them: the C++ Kompoz's search tests by name (kompoz_tests.cpp;
//! M15's values and Compose for desktop's actuals where the C++ differed, said where), Compose's behaviour (the
//! states, their saver and animations, the input fields, the expanded forms, the scroll behavior, the colors and
//! defaults) and M15's SearchBarSamples.kt; the bars of the C++ tests and the samples that are app bars in a
//! Scaffold's top bar over a list.

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_animation::{AnimatedVisibility, AnimationSpec, snap};
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_extended::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas};
use kompoz_ui::input::{Key, KeyModifiers, PointerType};
use kompoz_ui::platform::{Insets, SystemInsets};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

tests!(
    // the C++ Kompoz's
    search_bar_expands_searches_and_closes,
    docked_search_bar_opens_in_place,
    search_bar_state_expands_full_screen_or_docked,
    full_screen_search_grows_out_of_its_bar_and_follows_the_back_gesture,
    top_search_bar_sits_alone_and_slides_away,
    app_bar_with_search_slides_away_with_the_content,
    // Compose's behaviour
    search_bar_states_animate_snap_and_save,
    input_fields_expand_their_state_as_compose_says,
    the_search_bar_with_content_follows_the_back_gesture,
    contained_full_screen_search_bars_fade_their_view_in,
    docked_search_bars_with_a_gap_drop_down_over_a_scrim,
    enter_always_scroll_behavior_settles_reverses_and_saves,
    app_bars_with_search_take_their_scrolled_colors,
    deprecated_search_bars_of_a_query,
    search_bar_defaults_are_the_tokens,
    // M15's samples
    simple_search_bar_sample,
    full_screen_search_bar_scaffold_sample,
    docked_search_bar_scaffold_sample,
);

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

fn near_within(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

type Slot<T> = Rc<RefCell<Option<T>>>;

fn slot<T>() -> Slot<T> {
    Rc::new(RefCell::new(None))
}

fn get<T: Clone>(s: &Slot<T>) -> T {
    s.borrow().clone().expect("set by the composition")
}

fn launch(scope: &Slot<CoroutineScope>, f: impl std::future::Future<Output = ()> + 'static) {
    named_args!(get(scope).launch(f));
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
}

fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    rule.on_node_with_text(text).get_unclipped_bounds_in_root()
}

fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(p, -1, 0);
    rule.advance_frame();
}

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

/// A drag held at its end (not let go): `release` lets it go.
fn drag_held(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        let t = i as f32 / frames as f32;
        h.pointer_move(from + (to - from) * t, -1, 0);
    }
    rule.advance_time_by(200);
}

fn release(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_up(at, -1, 0);
    rule.wait_for_idle();
}

/// Where a scroll behavior settles when let go at `offset` of `limit` (Compose's `settleSearchBar` without a
/// velocity): shown under half way, else gone.
fn settled(offset: f32, limit: f32) -> f32 {
    if offset / limit < 0.5 { 0.0 } else { limit }
}

fn escape(rule: &KompozTestRule) -> bool {
    rule.host().key_down(Key::Escape, KeyModifiers::default())
}

/// The touch slop the test host's gestures pass before a drag counts.
fn touch_slop() -> f32 {
    kompoz_ui::platform::ViewConfiguration::default().touch_slop
}

const Cities: [&str; 5] = ["Ankara", "Antalya", "Bursa", "İstanbul", "İzmir"];

/// The cities starting with `query`, each a 48 dp row; a click picks it.
#[kompozable]
fn CityRows(query: MutableState<String>, expanded: MutableState<bool>) {
    for city in Cities {
        if city.starts_with(&query.get()) {
            let (query, expanded) = (query.clone(), expanded.clone());
            Text(
                city,
                modifier = Modifier.fill_max_width().height(48.dp()).clickable(
                    on_click = move || {
                        query.set(city.to_string());
                        expanded.set(false);
                    },
                ),
            );
        }
    }
}

// ---- the C++ Kompoz's ------------------------------------------------------------------------------------------

/// C++ `search_bar_expands_searches_and_closes`, M15's `SearchBar(inputField, expanded, onExpandedChange, …,
/// content)` and `SearchBarDefaults.InputField(query, …)`. M15 vs the C++: the field's content description is the
/// library's "Search" (the placeholder is a text of its own); the bar grows in place to fill what it is given (the
/// Column's room: the page goes below it), no overlay.
fn search_bar_expands_searches_and_closes() {
    let query = mutable_state_of(String::new());
    let expanded = mutable_state_of(false);
    let searched = Rc::new(RefCell::new(Vec::<String>::new()));
    let rule = {
        let (query, expanded, searched) = (query.clone(), expanded.clone(), searched.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let (query, expanded, searched) = (query.clone(), expanded.clone(), searched.clone());
                Column(
                    content = move || {
                        let (q1, q2, e1, e2, e3, s) = (
                            query.clone(),
                            query.clone(),
                            expanded.clone(),
                            expanded.clone(),
                            expanded.clone(),
                            searched.clone(),
                        );
                        let (rows_query, rows_expanded) = (query.clone(), expanded.clone());
                        let expanded_now = expanded.get();
                        SearchBarWithContent(
                            move || {
                                let (q2, e1, e2, s) = (q2.clone(), e1.clone(), e2.clone(), s.clone());
                                SearchBarDefaults.StringInputField(
                                    q1.get(),
                                    move |v: String| q2.set(v),
                                    move |q: String| {
                                        s.borrow_mut().push(q);
                                        e1.set(false);
                                    },
                                    expanded_now,
                                    move |v: bool| e2.set(v),
                                    modifier = Modifier.test_tag("field"),
                                    placeholder = Some(Rc::new(|| Text("Şehir ara"))),
                                );
                            },
                            expanded_now,
                            move |v: bool| e3.set(v),
                            modifier = Modifier.fill_max_width().test_tag("bar"),
                            content = move || CityRows(rows_query.clone(), rows_expanded.clone()),
                        );
                        Text("Sayfa", modifier = Modifier.test_tag("page"));
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    rule.on_node_with_text("Şehir ara").assert_exists();
    rule.on_node_with_content_description("Search").assert_exists();
    assert!(!rule.on_node_with_text("Ankara").exists());
    // At rest: the field's 56 dp, 8 dp above it (M15's bottom padding comes in as it expands).
    assert!(
        near(bounds(&rule, "bar").height(), 64.0) && near(bounds(&rule, "page").top, 64.0),
        "{:?}",
        bounds(&rule, "bar")
    );
    rule.on_node_with_content_description("Search").perform_click(); // expands: suggestions, the field focused
    rule.wait_for_idle();
    assert!(expanded.peek() && rule.on_node_with_text("Bursa").exists() && rule.host().overlay_count() == 0);
    rule.on_node_with_tag("field").assert_is_focused();
    // (M15: the bar fills the Column's 800: the page below it)
    assert!(near(bounds(&rule, "bar").height(), 800.0), "{:?}", bounds(&rule, "bar"));
    rule.on_node_with_tag("page").assert_is_not_displayed();
    rule.host().text_input("An");
    rule.wait_for_idle();
    assert!(
        query.peek() == "An" && rule.on_node_with_text("Antalya").exists() && !rule.on_node_with_text("Bursa").exists()
    );
    rule.host().ime_action(kompoz_ui::platform::ImeAction::Search); // the keyboard's search key
    rule.wait_for_idle();
    assert_eq!(*searched.borrow(), vec!["An".to_string()]);
    assert!(!expanded.peek() && !rule.on_node_with_text("Antalya").exists());
    rule.on_node_with_text("An").assert_exists(); // collapsed, showing the query
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Antalya").perform_click(); // a suggestion fills the query and closes
    rule.wait_for_idle();
    assert!(query.peek() == "Antalya" && !expanded.peek());
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    assert!(expanded.peek());
    escape(&rule); // back closes it
    rule.wait_for_idle();
    assert!(!expanded.peek() && near(bounds(&rule, "bar").height(), 64.0));
}

/// C++ `docked_search_bar_opens_in_place`, M15's `DockedSearchBar` and `SearchBarDefaults.InputField(query, …)`.
/// M15 vs the C++: no overlay (its results expand inside its surface, drawn over what follows — its z index —
/// though the Column places what follows below its surface, as it grows); a tap outside doesn't collapse it (only
/// back does).
fn docked_search_bar_opens_in_place() {
    let query = mutable_state_of(String::new());
    let expanded = mutable_state_of(false);
    let rule = {
        let (query, expanded) = (query.clone(), expanded.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let (query, expanded) = (query.clone(), expanded.clone());
                Column(
                    modifier = Modifier.padding(20.dp()),
                    content = move || {
                        Text("Üstte", modifier = Modifier.height(100.dp()));
                        let (q1, q2, e2, e3) = (query.clone(), query.clone(), expanded.clone(), expanded.clone());
                        let (rows_query, rows_expanded) = (query.clone(), expanded.clone());
                        let expanded_now = expanded.get();
                        DockedSearchBar(
                            move || {
                                let (q2, e2) = (q2.clone(), e2.clone());
                                SearchBarDefaults.StringInputField(
                                    q1.get(),
                                    move |v: String| q2.set(v),
                                    |_: String| {},
                                    expanded_now,
                                    move |v: bool| e2.set(v),
                                    modifier = Modifier.test_tag("field"),
                                    placeholder = Some(Rc::new(|| Text("Şehir ara"))),
                                );
                            },
                            expanded_now,
                            move |v: bool| e3.set(v),
                            modifier = Modifier.fill_max_width().test_tag("bar"),
                            content = move || CityRows(rows_query.clone(), rows_expanded.clone()),
                        );
                        Text("Sayfa", modifier = Modifier.test_tag("page"));
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let bar = bounds(&rule, "bar");
    let page_y = bounds(&rule, "page").top;
    assert!(near(bar.width(), 360.0) && near(bar.height(), 56.0) && near(page_y, bar.bottom));
    assert!(!rule.on_node_with_text("Ankara").exists() && rule.host().overlay_count() == 0);
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    assert!(expanded.peek() && rule.host().overlay_count() == 0);
    rule.on_node_with_tag("field").assert_is_focused();
    // Under the field and its divider, at the bar's width.
    let ankara = text_bounds(&rule, "Ankara");
    assert!(near(ankara.left, bar.left) && near(ankara.width(), bar.width()) && near(ankara.top, bar.bottom + 1.0));
    // (M15: between 240 dp and two thirds of the window tall: the five rows and the divider take 241)
    assert!(near(bounds(&rule, "bar").height(), 56.0 + 241.0), "{:?}", bounds(&rule, "bar"));
    assert!(near(bounds(&rule, "page").top, page_y + 241.0));
    rule.host().text_input("An");
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("Antalya").exists() && !rule.on_node_with_text("Bursa").exists());
    rule.on_node_with_text("Antalya").perform_click();
    rule.wait_for_idle();
    assert!(query.peek() == "Antalya" && !expanded.peek());
    rule.on_node_with_content_description("Search").perform_click(); // open again
    rule.wait_for_idle();
    assert!(expanded.peek());
    tap(&rule, Offset::new(200.0, 790.0)); // (M15: a tap outside leaves it open)
    rule.wait_for_idle();
    assert!(expanded.peek());
    escape(&rule); // back closes it
    rule.wait_for_idle();
    assert!(!expanded.peek() && !rule.on_node_with_text("Ankara").exists());
}

/// C++ `search_bar_state_expands_full_screen_or_docked`: M15's `SearchBar(state, …)` with
/// `ExpandedFullScreenSearchBar` / `ExpandedDockedSearchBar` and `SearchBarDefaults.InputField(textFieldState,
/// searchBarState, …)`. M15 vs the C++: the field's description is the library's "Search"; the collapsed bar keeps
/// its field while the expanded view is open over it (the view's own field is the one focused).
fn search_bar_state_expands_full_screen_or_docked() {
    let (state, text, scope) = (slot::<SearchBarState>(), slot::<TextFieldState>(), slot::<CoroutineScope>());
    let searched = Rc::new(RefCell::new(Vec::<String>::new()));
    let docked = mutable_state_of(false);
    let rule = {
        let (state, text, scope, searched, docked) =
            (state.clone(), text.clone(), scope.clone(), searched.clone(), docked.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let s = remember_search_bar_state();
                let t = remember_text_field_state();
                let sc = remember_coroutine_scope();
                *state.borrow_mut() = Some(s.clone());
                *text.borrow_mut() = Some(t.clone());
                *scope.borrow_mut() = Some(sc.clone());
                let field = {
                    let (s, t, searched) = (s.clone(), t.clone(), searched.clone());
                    Rc::new(move |tag: &'static str| {
                        let (s2, sc, searched) = (s.clone(), sc.clone(), searched.clone());
                        SearchBarDefaults.InputField(
                            t.clone(),
                            s.clone(),
                            move |q: String| {
                                searched.borrow_mut().push(q);
                                let s2 = s2.clone();
                                named_args!(sc.launch(async move { s2.animate_to_collapsed().await }));
                            },
                            modifier = Modifier.test_tag(tag),
                            placeholder = Some(Rc::new(|| Text("Şehir ara"))),
                        );
                    })
                };
                let results = {
                    let t = t.clone();
                    Rc::new(move || {
                        for city in ["Ankara", "Antalya", "Bursa"] {
                            if city.starts_with(&t.text()) {
                                Text(city, modifier = Modifier.fill_max_width().height(48.dp()));
                            }
                        }
                    })
                };
                let (bar_state, bar_field) = (s.clone(), field.clone());
                Column(
                    modifier = Modifier.padding_sides(16.dp(), 100.dp(), 16.dp(), 0.dp()),
                    content = move || {
                        let bar_field = bar_field.clone();
                        SearchBar(
                            bar_state.clone(),
                            input_field = move || bar_field("bar-field"),
                            modifier = Modifier.fill_max_width().test_tag("bar"),
                        );
                        Text("Sayfa");
                    },
                );
                let (f, r) = (field.clone(), results.clone());
                if docked.get() {
                    ExpandedDockedSearchBar(s.clone(), input_field = move || f("view-field"), content = move || r());
                } else {
                    ExpandedFullScreenSearchBar(
                        s.clone(),
                        input_field = move || f("view-field"),
                        content = move || r(),
                    );
                }
            }),
            IntSize::new(400, 800),
        )
    };
    let host = rule.host();
    let s = get(&state);
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
    // A tap expands it: heading there at once, settled when the spring is done.
    let pill = bounds(&rule, "bar").center();
    tap(&rule, pill);
    rule.advance_time_by(100);
    assert!(s.target_value() == SearchBarValue::Expanded, "{s:?}");
    assert!(s.progress() > 0.1 && s.progress() < 0.9 && host.overlay_count() == 1, "{s:?}");
    {
        // the view fades in with it
        let canvas: RecordingCanvas = rule.draw();
        let p = s.progress();
        assert!(canvas.layer_alphas.iter().any(|a| near_within(*a, p, 0.05)), "{p} {:?}", canvas.layer_alphas);
    }
    rule.wait_for_idle();
    assert!(
        s.current_value() == SearchBarValue::Expanded
            && s.progress() == 1.0
            && rule.on_node_with_text("Bursa").exists()
    );
    rule.on_node_with_tag("view-field").assert_is_focused(); // the view's field, to type into
    rule.on_node_with_tag("bar-field").assert_exists(); // (M15: the collapsed bar keeps its field)
    host.text_input("An");
    rule.wait_for_idle();
    assert!(
        get(&text).text() == "An"
            && rule.on_node_with_text("Antalya").exists()
            && !rule.on_node_with_text("Bursa").exists()
    );
    // Searching collapses it (the app does).
    host.ime_action(kompoz_ui::platform::ImeAction::Search);
    rule.advance_time_by(64);
    assert_eq!(*searched.borrow(), vec!["An".to_string()]);
    assert!(s.target_value() == SearchBarValue::Collapsed && host.overlay_count() == 1);
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Collapsed && s.progress() == 0.0 && host.overlay_count() == 0);
    rule.on_node_with_tag("bar-field").assert_text_equals_with(&["An"], true); // the bar shows the query
    // Back collapses it too.
    rule.on_node_with_tag("bar-field").perform_click();
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Expanded);
    escape(&rule);
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Collapsed && host.overlay_count() == 0);
    // In code, without the animation.
    let ss = s.clone();
    launch(&scope, async move { ss.snap_to(1.0).await });
    rule.advance_frame();
    rule.advance_frame();
    assert!(s.current_value() == SearchBarValue::Expanded && s.progress() == 1.0 && host.overlay_count() == 1);
    let ss = s.clone();
    launch(&scope, async move { ss.snap_to(0.0).await });
    rule.wait_for_idle();
    assert!(host.overlay_count() == 0);
    // Docked: over the collapsed bar, at its width; a tap outside collapses it.
    docked.set(true);
    rule.wait_for_idle();
    let bar = bounds(&rule, "bar");
    rule.on_node_with_tag("bar-field").perform_click();
    rule.wait_for_idle();
    assert!(host.overlay_count() == 1 && s.current_value() == SearchBarValue::Expanded);
    // Under the field and its divider; (M15) as wide as the view's field, 360 dp (a text field's own width, within
    // the bar's: the C++'s took the bar's 368).
    let first = text_bounds(&rule, "Ankara");
    assert!(
        near(first.left, bar.left) && near(first.width(), 360.0) && near(first.top, bar.top + 56.0 + 1.0),
        "{bar:?} {first:?}"
    );
    tap(&rule, Offset::new(200.0, 750.0));
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Collapsed && host.overlay_count() == 0);
}

/// C++ `full_screen_search_grows_out_of_its_bar_and_follows_the_back_gesture`: M15's `ExpandedFullScreenSearchBar`
/// grows from the collapsed bar's place and size to the window's. M15 vs the C++: the view's field is
/// `lerp(collapsedWidth, width, progress)` wide where the view is `lerp(collapsedWidth, windowWidth, progress)`
/// (Compose's `paddedInputFieldWidth`), so part way it is narrower than the C++'s, centred on the bar's centre; and
/// the back gesture: Compose for desktop's dialog (the full-screen view's, `BasicEdgeToEdgeDialog`'s actual) gives
/// its content a predictive back state that never runs, so on the desktop the view doesn't follow the gesture — a
/// completed back dismisses it (the dialog's back press) and it collapses into the bar. (The layout's predictive back
/// math, Android's, is checked against the C++'s numbers in search_bar.rs' unit tests.)
fn full_screen_search_grows_out_of_its_bar_and_follows_the_back_gesture() {
    let (state, scope) = (slot::<SearchBarState>(), slot::<CoroutineScope>());
    let wide = mutable_state_of(false);
    let rule = {
        let (state, scope, wide) = (state.clone(), scope.clone(), wide.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let s = remember_search_bar_state();
                *state.borrow_mut() = Some(s.clone());
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                let t = remember_text_field_state();
                // The bar at 40, 100, 320 x 56; wide: at 16, 10, 368 x 56.
                let padding = if wide.get() {
                    Modifier.padding_sides(16.dp(), 10.dp(), 16.dp(), 0.dp())
                } else {
                    Modifier.padding_sides(40.dp(), 100.dp(), 40.dp(), 0.dp())
                };
                let (bar_state, bar_text) = (s.clone(), t.clone());
                Column(
                    modifier = padding,
                    content = move || {
                        let (bar_state2, bar_text) = (bar_state.clone(), bar_text.clone());
                        // (Compose's input field is 360 dp at least: given 320 here, the bar's own size)
                        SearchBar(
                            bar_state.clone(),
                            move || {
                                SearchBarDefaults.InputField(
                                    bar_text.clone(),
                                    bar_state2.clone(),
                                    |_: String| {},
                                    modifier = Modifier.required_width_in(min = 0.dp()),
                                    placeholder = Some(Rc::new(|| Text("Ara"))),
                                )
                            },
                            modifier = Modifier.fill_max_width(),
                        );
                    },
                );
                let (view_state, view_text) = (s.clone(), t.clone());
                ExpandedFullScreenSearchBar(
                    s.clone(),
                    move || {
                        SearchBarDefaults.InputField(
                            view_text.clone(),
                            view_state.clone(),
                            |_: String| {},
                            modifier = Modifier.test_tag("açık"),
                            placeholder = Some(Rc::new(|| Text("Ara"))),
                        )
                    },
                    content = || Text("Sonuç", modifier = Modifier.test_tag("sonuç")),
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let s = get(&state);
    let field = || bounds(&rule, "açık"); // the expanded view's
    // Part way out of the bar: between its bounds and the window's, the field 8 dp lower as it goes.
    let ss = s.clone();
    launch(&scope, async move { ss.animate_to_expanded().await });
    rule.advance_time_by(150);
    let p = s.progress();
    assert!(p > 0.2 && p < 0.95, "{p}");
    let a = field();
    let view_width = 320.0 + (400.0 - 320.0) * p;
    let field_width = 320.0 + (view_width - 320.0) * p;
    assert!(near(a.width(), field_width) && near(a.center().x, 200.0), "{a:?} {p}");
    assert!(near(a.top, 100.0 * (1.0 - p) + 8.0 * p), "{a:?} {p}");
    // the results under it, 8 dp more as it goes (and the divider's 1)
    assert!(near(bounds(&rule, "sonuç").top, a.bottom + 8.0 * p + 1.0), "{:?} {a:?}", bounds(&rule, "sonuç"));
    rule.wait_for_idle();
    let f = field();
    assert!(
        near(f.left, 0.0)
            && near(f.top, 8.0)
            && near(f.width(), 400.0)
            && near(bounds(&rule, "sonuç").top, 8.0 + 56.0 + 8.0 + 1.0)
    );
    // A back gesture: the view stays as it is on the desktop (see above); cancelled, nothing happened.
    let back = |x: f32, y: f32, progress: f32, edge| kompoz_ui::back::BackEventCompat {
        touch_x: x,
        touch_y: y,
        progress,
        swipe_edge: edge,
    };
    rule.host().back_started(back(10.0, 400.0, 0.0, kompoz_ui::back::SwipeEdge::Left));
    rule.host().back_progressed(back(60.0, 600.0, 0.5, kompoz_ui::back::SwipeEdge::Left));
    rule.advance_frame();
    assert!(near(field().left, 0.0) && near(field().width(), 400.0) && near(field().top, 8.0));
    rule.host().back_cancelled();
    rule.advance_frame();
    assert!(near(field().left, 0.0) && s.target_value() == SearchBarValue::Expanded);
    // Done: the dialog's back press collapses it into the bar.
    rule.host().back_started(back(390.0, 400.0, 0.0, kompoz_ui::back::SwipeEdge::Right));
    rule.host().back_progressed(back(340.0, 400.0, 0.5, kompoz_ui::back::SwipeEdge::Right));
    assert!(rule.host().back_invoked());
    rule.advance_time_by(100);
    let q = s.progress();
    assert!(s.target_value() == SearchBarValue::Collapsed && q > 0.05 && q < 0.95, "{q}");
    let h = field();
    let view_width = 320.0 + (400.0 - 320.0) * q;
    assert!(near(h.width(), 320.0 + (view_width - 320.0) * s.progress()) && near(h.center().x, 200.0), "{h:?}");
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Collapsed && rule.host().overlay_count() == 0);
    // Opened again: whole.
    let ss = s.clone();
    launch(&scope, async move { ss.animate_to_expanded().await });
    rule.wait_for_idle();
    assert!(near(field().left, 0.0) && near(field().width(), 400.0));
    // A bar wider and near the top: the view grows from it the same way.
    let ss = s.clone();
    launch(&scope, async move { ss.snap_to(0.0).await });
    wide.set(true);
    rule.wait_for_idle();
    let ss = s.clone();
    launch(&scope, async move { ss.animate_to_expanded().await });
    rule.advance_time_by(150);
    let p = s.progress();
    let a = field();
    let view_width = 368.0 + (400.0 - 368.0) * p;
    assert!(near(a.width(), 368.0 + (view_width - 368.0) * p) && near(a.center().x, 200.0), "{a:?}");
    assert!(near(a.top, 10.0 * (1.0 - p) + 8.0 * p), "{a:?}");
    rule.wait_for_idle();
}

/// The C++'s Scaffold of a top bar (and a bottom bar) over a list of 100 rows of 50 dp (`row{i}`), the scroll
/// connections given on the scaffold.
#[kompozable]
fn ScaffoldOfList(
    top_bar: Rc<dyn Fn()>,
    bottom_bar: Option<Rc<dyn Fn()>>,
    scrolls: Vec<Rc<dyn kompoz_ui::input::NestedScrollConnection>>,
    list: LazyListState,
    label: &'static str,
) {
    let modifier = scrolls.into_iter().fold(Modifier.fill_max_size(), |m, s| m.nested_scroll(s));
    Scaffold(
        modifier = modifier,
        top_bar = move || top_bar(),
        bottom_bar = move || {
            if let Some(b) = &bottom_bar {
                b();
            }
        },
        content = move |padding: PaddingValues| {
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                state = list.clone(),
                content_padding = padding,
                content = move |scope: &mut LazyListScope| {
                    scope.items(100, move |i: i32| {
                        Text(format!("{label} {i}"), modifier = Modifier.height(50.dp()).test_tag(format!("row{i}")));
                    });
                },
            );
        },
    );
}

/// C++ `top_search_bar_sits_alone_and_slides_away`: M15's deprecated `TopSearchBar` (an `AppBarWithSearch` of
/// a transparent container) with `SearchBarDefaults.enterAlwaysSearchBarScrollBehavior()`. M15 vs the C++: the
/// pill is 4 dp from the bar's top and bottom (the bar 64 dp; the C++'s 8, 72); the behavior is M15's own
/// `SearchBarScrollBehavior` (its `scrollOffset`, not a TopAppBarState's height offset). Its BottomAppBar leaves on
/// the caller's state (`exitAlwaysScrollBehavior`) once the content scrolls.
fn top_search_bar_sits_alone_and_slides_away() {
    let (behavior, search) = (slot::<Rc<dyn SearchBarScrollBehavior>>(), slot::<SearchBarState>());
    let bottom = slot::<Rc<dyn BottomAppBarState>>();
    let wide = mutable_state_of(false);
    let rule = {
        let (behavior, search, bottom, wide) = (behavior.clone(), search.clone(), bottom.clone(), wide.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let scroll = SearchBarDefaults.enter_always_search_bar_scroll_behavior();
                *behavior.borrow_mut() = Some(scroll.clone());
                let s = remember_search_bar_state();
                *search.borrow_mut() = Some(s.clone());
                let b = remember_bottom_app_bar_state();
                *bottom.borrow_mut() = Some(b.clone());
                let exit = BottomAppBarDefaults.exit_always_scroll_behavior(state = b); // the caller's state
                let text = remember_text_field_state();
                let list = remember_lazy_list_state();
                let (bar_state, bar_scroll) = (s.clone(), scroll.clone());
                let connections = vec![scroll.nested_scroll_connection(), exit.nested_scroll_connection()];
                Box(
                    modifier = Modifier.width(if wide.get() { 1000.dp() } else { 400.dp() }).fill_max_height(),
                    content = move || {
                        let (bar_state, bar_scroll, text, exit) =
                            (bar_state.clone(), bar_scroll.clone(), text.clone(), exit.clone());
                        ScaffoldOfList(
                            Rc::new(move || {
                                let (s2, text) = (bar_state.clone(), text.clone());
                                TopSearchBar(
                                    bar_state.clone(),
                                    move || {
                                        SearchBarDefaults.InputField(
                                            text.clone(),
                                            s2.clone(),
                                            |_: String| {},
                                            modifier = Modifier.test_tag("hap"),
                                            placeholder = Some(Rc::new(|| Text("Ara"))),
                                        )
                                    },
                                    scroll_behavior = Some(bar_scroll.clone()),
                                );
                            }),
                            Some(Rc::new(move || {
                                BottomAppBar(actions = || Text("Alt"), scroll_behavior = Some(exit.clone()));
                            })),
                            connections.clone(),
                            list.clone(),
                            "Satır",
                        );
                    },
                );
            }),
            IntSize::new(1000, 800),
        )
    };
    let pill = || bounds(&rule, "hap");
    let b = get(&behavior);
    // 4 dp from the top, 56 tall, centred (M15: the field's 360 dp, not the room's 384: a text field doesn't fill
    // what it is given); the content under its 64 dp.
    let p = pill();
    assert!(near(p.left, 20.0) && near(p.top, 4.0) && near(p.width(), 360.0) && near(p.height(), 56.0), "{p:?}");
    assert!(near(bounds(&rule, "row0").top, 64.0) && near(b.scroll_offset_limit(), -64.0));
    // Scrolled on (40 px, the slop's first): it slides up and gives the content its room.
    let moved = 40.0 - touch_slop();
    drag_held(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 360.0), 20);
    assert!(near(b.scroll_offset(), -moved) && near(pill().top, 4.0 - moved), "{} {:?}", b.scroll_offset(), pill());
    assert!(near(bounds(&rule, "row0").top, 64.0 - moved));
    let bottom_bar = get(&bottom);
    assert!(bottom_bar.height_offset() == 0.0); // the content hasn't moved yet
    // (M15: let go part way, it settles shown or gone, whichever is nearer)
    release(&rule, Offset::new(200.0, 360.0));
    assert!(near(b.scroll_offset(), settled(-moved, -64.0)), "{}", b.scroll_offset());
    // On until the content scrolls: the bottom bar, on the caller's state, leaves too.
    slow_drag(&rule, Offset::new(200.0, 600.0), Offset::new(200.0, 300.0), 20);
    rule.wait_for_idle();
    assert!(near(b.scroll_offset(), -64.0) && bottom_bar.height_offset() != 0.0, "{}", bottom_bar.height_offset());
    // Back, and a tap on it expands the search.
    slow_drag(&rule, Offset::new(200.0, 300.0), Offset::new(200.0, 700.0), 20);
    slow_drag(&rule, Offset::new(200.0, 300.0), Offset::new(200.0, 700.0), 20);
    rule.wait_for_idle();
    assert!(near(b.scroll_offset(), 0.0));
    rule.on_node_with_tag("hap").perform_click();
    rule.wait_for_idle();
    let s = get(&search);
    assert!(s.target_value() == SearchBarValue::Expanded);
    // In a wide window: in the middle (M15: still the field's 360; up to 720 for a field that fills its room).
    wide.set(true);
    rule.wait_for_idle();
    assert!(near(pill().width(), 360.0) && near(pill().left, (1000.0 - 360.0) / 2.0), "{:?}", pill());
    // Under the status bar.
    rule.host().set_system_insets(SystemInsets {
        status_bars: Insets { left: 0, top: 24, right: 0, bottom: 0 },
        ..SystemInsets::default()
    });
    rule.wait_for_idle();
    assert!(near(pill().top, 24.0 + 4.0), "{:?}", pill());
}

/// C++ `app_bar_with_search_slides_away_with_the_content`: M15's `AppBarWithSearch` with a navigation icon, actions
/// and `enterAlwaysSearchBarScrollBehavior()`. M15 vs the C++: the bar is 64 dp (the pill 4 dp from its top and
/// bottom), the pill 8 dp from the icons' room (the C++'s 4), the menu's centre at 32; the behavior is M15's own
/// `SearchBarScrollBehavior`.
fn app_bar_with_search_slides_away_with_the_content() {
    let (behavior, search) = (slot::<Rc<dyn SearchBarScrollBehavior>>(), slot::<SearchBarState>());
    let list_slot = slot::<LazyListState>();
    let menus = Rc::new(Cell::new(0));
    let pinned = mutable_state_of(false);
    let rule = {
        let (behavior, search, list_slot, menus, pinned) =
            (behavior.clone(), search.clone(), list_slot.clone(), menus.clone(), pinned.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let scroll = SearchBarDefaults.enter_always_search_bar_scroll_behavior();
                *behavior.borrow_mut() = Some(scroll.clone());
                let s = remember_search_bar_state();
                *search.borrow_mut() = Some(s.clone());
                let text = remember_text_field_state();
                let list = remember_lazy_list_state();
                *list_slot.borrow_mut() = Some(list.clone());
                let field = {
                    let (s, text) = (s.clone(), text.clone());
                    Rc::new(move || {
                        SearchBarDefaults.InputField(
                            text.clone(),
                            s.clone(),
                            |_: String| {},
                            placeholder = Some(Rc::new(|| Text("Postalarda ara"))),
                        )
                    })
                };
                let (bar_state, bar_field, menus, pinned_now) = (s.clone(), field.clone(), menus.clone(), pinned.get());
                let bar_scroll = scroll.clone();
                ScaffoldOfList(
                    Rc::new(move || {
                        let (bar_field, menus) = (bar_field.clone(), menus.clone());
                        AppBarWithSearch(
                            bar_state.clone(),
                            move || bar_field(),
                            modifier = Modifier.test_tag("çubuk"),
                            navigation_icon = Some(Rc::new(move || {
                                let menus = menus.clone();
                                IconButton(
                                    on_click = move || menus.set(menus.get() + 1),
                                    content = || VectorIcon(Icons::Filled::Menu(), Some("Menü".to_string())),
                                );
                            })),
                            actions = Some(Rc::new(|| VectorIcon(Icons::Filled::Person(), Some("Hesap".to_string())))),
                            scroll_behavior = if pinned_now { None } else { Some(bar_scroll.clone()) },
                        );
                    }),
                    None,
                    vec![scroll.nested_scroll_connection()],
                    list,
                    "Posta",
                );
                let f = field.clone();
                ExpandedFullScreenSearchBar(s.clone(), input_field = move || f(), content = || Text("Sonuçlar"));
            }),
            IntSize::new(400, 800),
        )
    };
    let b = get(&behavior);
    let list = get(&list_slot);
    // 64 dp: the pill (56) filling the room between the navigation icon's button (48 dp of room, 4 dp in) and the
    // actions (4 dp in), 8 dp from each.
    let bar = bounds(&rule, "çubuk");
    let menu = rule.on_node_with_content_description("Menü").get_unclipped_bounds_in_root();
    let account = rule.on_node_with_content_description("Hesap").get_unclipped_bounds_in_root();
    let pill = rule.on_node_with_content_description("Search").get_unclipped_bounds_in_root();
    assert!(
        near(bar.height(), 64.0) && near(b.scroll_offset_limit(), -64.0) && near(bounds(&rule, "row0").top, 64.0),
        "{bar:?}"
    );
    assert!(near(pill.height(), 56.0) && near(pill.top, 4.0), "{pill:?}");
    assert!(
        near(pill.left, 4.0 + 48.0 + 8.0) && near(menu.center().x, 4.0 + 24.0) && near(menu.center().y, 32.0),
        "{pill:?} {menu:?}"
    );
    assert!(near(pill.right, account.left - 8.0) && near(account.right, bar.right - 4.0), "{pill:?} {account:?}");
    // Scrolled on: it slides up (first, the list waiting) and gives the content its room.
    let moved = 40.0 - touch_slop();
    drag_held(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 360.0), 20);
    assert!(
        near(b.scroll_offset(), -moved)
            && list.first_visible_item_index() == 0
            && list.first_visible_item_scroll_offset() == 0,
        "{} {moved} {} {}",
        b.scroll_offset(),
        list.first_visible_item_index(),
        list.first_visible_item_scroll_offset()
    );
    assert!(near(bounds(&rule, "çubuk").height(), 64.0 - moved) && near(bounds(&rule, "row0").top, 64.0 - moved));
    let pill_now = rule.on_node_with_content_description("Search").get_unclipped_bounds_in_root();
    assert!(near(pill_now.top, 4.0 - moved), "{pill_now:?}"); // the pill slides with it
    release(&rule, Offset::new(200.0, 360.0)); // (M15: it settles)
    assert!(near(b.scroll_offset(), settled(-moved, -64.0)), "{}", b.scroll_offset());
    slow_drag(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 200.0), 20);
    rule.wait_for_idle();
    assert!(near(b.scroll_offset(), -64.0) && near(bounds(&rule, "çubuk").height(), 0.0));
    // Scrolled back: it comes back before the list moves.
    let index = list.first_visible_item_index();
    let offset = list.first_visible_item_scroll_offset();
    drag_held(&rule, Offset::new(200.0, 300.0), Offset::new(200.0, 340.0), 20);
    assert!(
        near(b.scroll_offset(), -64.0 + moved)
            && list.first_visible_item_index() == index
            && list.first_visible_item_scroll_offset() == offset
    );
    release(&rule, Offset::new(200.0, 340.0));
    // Its pill expands the search; the navigation icon is a button of its own.
    slow_drag(&rule, Offset::new(200.0, 300.0), Offset::new(200.0, 400.0), 20);
    rule.wait_for_idle();
    assert!(near(b.scroll_offset(), 0.0));
    rule.on_node_with_content_description("Menü").perform_click();
    let s = get(&search);
    assert!(menus.get() == 1 && s.target_value() == SearchBarValue::Collapsed);
    rule.on_node_with_text("Postalarda ara").perform_click();
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Expanded && rule.on_node_with_text("Sonuçlar").exists());
    // Without a scroll behavior it stays.
    escape(&rule);
    pinned.set(true);
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Collapsed);
    slow_drag(&rule, Offset::new(200.0, 600.0), Offset::new(200.0, 400.0), 20);
    assert!(near(bounds(&rule, "çubuk").height(), 64.0));
}

// ---- Compose's behaviour ---------------------------------------------------------------------------------------

/// Compose's `SearchBarState`: its value from its animation (expanded past 2 % of the way), its target, its
/// progress kept within 0..1, its saver (progress and content progress), its collapsed bar's coordinates.
fn search_bar_states_animate_snap_and_save() {
    let (state, scope) = (slot::<SearchBarState>(), slot::<CoroutineScope>());
    let rule = {
        let (state, scope) = (state.clone(), scope.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            let s = remember_search_bar_state();
            *state.borrow_mut() = Some(s.clone());
            *scope.borrow_mut() = Some(remember_coroutine_scope());
            let t = remember_text_field_state();
            let bs = s.clone();
            Box(
                modifier = Modifier.padding(30.dp()),
                content = move || {
                    let (bs2, t) = (bs.clone(), t.clone());
                    SearchBar(
                        bs.clone(),
                        input_field = move || SearchBarDefaults.InputField(t.clone(), bs2.clone(), |_: String| {}),
                    );
                },
            );
        }))
    };
    let s = get(&state);
    // The bar told it where it is.
    let coords = s.collapsed_coords().expect("set as the bar is placed");
    assert!(near(coords.position_in_window().x, 30.0) && coords.size().height == 56, "{:?}", coords.size());
    for (fraction, value) in
        [(0.01, SearchBarValue::Collapsed), (0.03, SearchBarValue::Expanded), (1.0, SearchBarValue::Expanded)]
    {
        let ss = s.clone();
        launch(&scope, async move { ss.snap_to(fraction).await });
        rule.advance_frame();
        assert_eq!(s.current_value(), value, "{fraction}");
        assert!(near_within(s.progress(), fraction, 1e-6) && !s.is_animating());
    }
    // Snapped past the ends, the progress stays within them.
    let ss = s.clone();
    launch(&scope, async move { ss.snap_to(1.5).await });
    rule.advance_frame();
    assert!(s.progress() == 1.0);
    let ss = s.clone();
    launch(&scope, async move { ss.animate_to_collapsed().await });
    rule.advance_frame();
    rule.advance_frame();
    assert!(
        s.target_value() == SearchBarValue::Collapsed
            && s.is_animating()
            && s.current_value() == SearchBarValue::Expanded
    );
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Collapsed && s.progress() == 0.0 && !s.is_animating());
    // Saved and restored: its progress (the content's too).
    let spec: Arc<dyn AnimationSpec<f32>> = Arc::new(snap::<f32>());
    let saver = SearchBarState::Saver(spec.clone(), spec.clone());
    let half = SearchBarState(SearchBarValue::Expanded, spec.clone(), spec.clone());
    let saved = saver.save(&half);
    assert_eq!(saved.as_list().map(|l| l.len()), Some(2));
    let restored = saver.restore(&saved).expect("restored");
    assert!(restored.progress() == 1.0 && restored.current_value() == SearchBarValue::Expanded && restored != half);
    let fading = SearchBarState::SaverWithContentFade(spec.clone(), spec.clone(), spec.clone(), spec.clone());
    assert!(fading.restore(&fading.save(&s)).expect("restored").progress() == 0.0);
}

/// Compose's `SearchBarDefaults.InputField(textFieldState, searchBarState, …)`: in touch mode focus expands the
/// state and a collapse gives the focus up a moment later; out of it, focus doesn't — the down key does, and moves
/// the focus on to the results once expanded; typing while focused expands it; a released press always does. Its
/// description says it searches, its state that suggestions are below while expanded.
fn input_fields_expand_their_state_as_compose_says() {
    let (state, scope) = (slot::<SearchBarState>(), slot::<CoroutineScope>());
    let rule = {
        let (state, scope) = (state.clone(), scope.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let s = remember_search_bar_state();
                *state.borrow_mut() = Some(s.clone());
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                let t = remember_text_field_state();
                let (bs, bt) = (s.clone(), t.clone());
                Column(
                    content = move || {
                        Box(modifier = Modifier.size(10.dp()).focusable().test_tag("önce"));
                        let (bs2, bt) = (bs.clone(), bt.clone());
                        SearchBar(
                            bs.clone(),
                            input_field = move || {
                                SearchBarDefaults.InputField(
                                    bt.clone(),
                                    bs2.clone(),
                                    |_: String| {},
                                    modifier = Modifier.test_tag("alan"),
                                )
                            },
                        );
                        if bs.current_value() == SearchBarValue::Expanded {
                            Box(modifier = Modifier.size(10.dp()).focusable().test_tag("sonuç"));
                        }
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let s = get(&state);
    let config = || rule.on_node_with_tag("alan").fetch_semantics_node().config().clone();
    assert_eq!(
        config().get(kompoz_ui::semantics::SemanticsProperties::ContentDescription),
        Some(vec!["Search".to_string()])
    );
    assert_eq!(config().get(kompoz_ui::semantics::SemanticsProperties::StateDescription), None);
    // Touch: focused by a tap, it expands.
    tap(&rule, bounds(&rule, "alan").center());
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Expanded);
    assert_eq!(
        config().get(kompoz_ui::semantics::SemanticsProperties::StateDescription),
        Some("Suggestions below".to_string())
    );
    rule.on_node_with_tag("alan").assert_is_focused();
    // Collapsed: the focus goes a moment (100 ms) after.
    let ss = s.clone();
    launch(&scope, async move { ss.snap_to(0.0).await });
    rule.advance_frame();
    rule.on_node_with_tag("alan").assert_is_focused();
    rule.wait_for_idle();
    assert!(rule.host().focused_node().is_none());
    // The keyboard: Tab focuses it without expanding; Down expands; Down again moves on to the results.
    // (the first key turns keyboard mode on; the field's focus callback, as Compose's, reads the mode it was composed
    // in: the second Tab reaches the field in keyboard mode)
    rule.host().key_down(Key::Tab, KeyModifiers::default());
    rule.wait_for_idle();
    rule.host().key_down(Key::Tab, KeyModifiers::default());
    rule.wait_for_idle();
    rule.on_node_with_tag("alan").assert_is_focused();
    assert!(s.current_value() == SearchBarValue::Collapsed);
    rule.host().key_down(Key::DirectionDown, KeyModifiers::default());
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Expanded);
    rule.host().key_down(Key::DirectionDown, KeyModifiers::default());
    rule.wait_for_idle();
    rule.on_node_with_tag("sonuç").assert_is_focused();
    // Typing into it while focused and collapsed expands it.
    let ss = s.clone();
    launch(&scope, async move { ss.snap_to(0.0).await });
    rule.wait_for_idle();
    rule.host().key_down(Key::Tab, KeyModifiers::Shift);
    rule.wait_for_idle();
    rule.on_node_with_tag("alan").assert_is_focused();
    assert!(s.current_value() == SearchBarValue::Collapsed);
    rule.on_node_with_tag("alan").perform_text_input("a");
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Expanded, "{s:?}");
}

/// The search bar that expands itself (`SearchBar(inputField, expanded, onExpandedChange, …)`) follows a back
/// gesture (Compose's `SearchBarLayout`): as it goes, the bar shrinks toward nine tenths of what it is given, moves
/// toward the gesture's side (a twentieth of the width less 8 dp at most) and with the finger (24 dp at most);
/// called off, it grows back; done, it tells `on_expanded_change` and collapses from where the gesture left it.
fn the_search_bar_with_content_follows_the_back_gesture() {
    let expanded = mutable_state_of(true);
    let rule = {
        let expanded = expanded.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let t = remember_text_field_state();
                let (e1, e2) = (expanded.clone(), expanded.clone());
                let now = expanded.get();
                SearchBarWithContent(
                    move || {
                        let e1 = e1.clone();
                        SearchBarDefaults.InputFieldWithExpanded(
                            t.clone(),
                            |_: String| {},
                            now,
                            move |v: bool| e1.set(v),
                            modifier = Modifier.test_tag("alan"),
                        )
                    },
                    now,
                    move |v: bool| e2.set(v),
                    modifier = Modifier.test_tag("bar"),
                    content = || Text("Sonuç", modifier = Modifier.test_tag("sonuç")),
                );
            }),
            IntSize::new(400, 800),
        )
    };
    rule.wait_for_idle();
    let f = bounds(&rule, "alan");
    assert!(
        near(f.left, 0.0) && near(f.width(), 400.0) && near(f.top, 8.0) && near(bounds(&rule, "bar").height(), 800.0)
    );
    let back = |x: f32, y: f32, progress: f32| kompoz_ui::back::BackEventCompat {
        touch_x: x,
        touch_y: y,
        progress,
        swipe_edge: kompoz_ui::back::SwipeEdge::Left,
    };
    // (the flow a predictive back handler collects starts with the first progress, not the start: its first event is
    // where the finger's moves are counted from — the C++ counted from the start)
    rule.host().back_started(back(10.0, 400.0, 0.0));
    rule.host().back_progressed(back(30.0, 400.0, 0.1));
    rule.host().back_progressed(back(60.0, 600.0, 0.5));
    rule.advance_frame();
    let b: f32 =
        kompoz_animation::Easing::transform(&kompoz_animation::CubicBezierEasing::constant(0.1, 0.1, 0.0, 1.0), 0.5);
    let p: f32 = 1.0 - b;
    // 360 (nine tenths) to 400 by the progress; 720 to 800 tall.
    let width = (360.0 + 40.0 * p).round();
    let height = (720.0 + 80.0 * p).round();
    let offset_x = (12.0 * (1.0 - p)).round();
    let space = ((800.0 - height) / 2.0 - 8.0).max(0.0);
    let offset_y = (space.min(24.0) * 0.25).round();
    let f = bounds(&rule, "alan");
    assert!(
        near(f.width(), width) && near(f.left, offset_x) && near(f.top, offset_y + 8.0),
        "{f:?} {width} {offset_x} {offset_y}"
    );
    assert!(near(bounds(&rule, "bar").height(), height), "{:?} {height}", bounds(&rule, "bar"));
    rule.host().back_cancelled();
    rule.wait_for_idle();
    assert!(expanded.peek() && near(bounds(&rule, "alan").width(), 400.0) && near(bounds(&rule, "alan").left, 0.0));
    rule.host().back_started(back(10.0, 400.0, 0.0));
    rule.host().back_progressed(back(60.0, 400.0, 0.5));
    assert!(rule.host().back_invoked());
    rule.advance_frame();
    assert!(!expanded.peek());
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "bar").height(), 64.0) && !rule.on_node_with_tag("sonuç").exists());
}

/// Compose's Expressive `ExpandedFullScreenContainedSearchBar` of `rememberContainedSearchBarState()`: its surface at
/// the window's size from the start, fading in with the state; the field 8 dp in from the sides and 4 from the top
/// (an app bar's padding); the content 8 dp under it (no divider), fading in by the state's content animation; its
/// colors the contained ones of the state (the full screen container while expanded).
fn contained_full_screen_search_bars_fade_their_view_in() {
    let (state, scope) = (slot::<SearchBarState>(), slot::<CoroutineScope>());
    let colors = slot::<(SearchBarColors, ColorScheme)>();
    let rule = {
        let (state, scope, colors) = (state.clone(), scope.clone(), colors.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let s = remember_contained_search_bar_state();
                *state.borrow_mut() = Some(s.clone());
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                *colors.borrow_mut() =
                    Some((SearchBarDefaults.contained_colors(s.clone()), MaterialTheme::color_scheme()));
                let t = remember_text_field_state();
                let (bs, bt) = (s.clone(), t.clone());
                Box(
                    modifier = Modifier.padding_sides(20.dp(), 100.dp(), 20.dp(), 0.dp()),
                    content = move || {
                        let (bs2, bt) = (bs.clone(), bt.clone());
                        SearchBar(
                            bs.clone(),
                            input_field = move || SearchBarDefaults.InputField(bt.clone(), bs2.clone(), |_: String| {}),
                        );
                    },
                );
                let (vs, vt) = (s.clone(), t.clone());
                ExpandedFullScreenContainedSearchBar(
                    s.clone(),
                    move || {
                        SearchBarDefaults.InputField(
                            vt.clone(),
                            vs.clone(),
                            |_: String| {},
                            modifier = Modifier.test_tag("alan"),
                        )
                    },
                    content = || Text("Sonuç", modifier = Modifier.test_tag("sonuç")),
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let s = get(&state);
    let (collapsed_colors, scheme) = get(&colors);
    assert!(collapsed_colors.container_color == scheme.surface_container_high);
    assert!(collapsed_colors.input_field_colors.focused_container_color == scheme.surface_container_high);
    let ss = s.clone();
    launch(&scope, async move { ss.animate_to_expanded().await });
    rule.advance_time_by(48);
    let p = s.progress();
    assert!(p > 0.05 && p < 0.95, "{p}");
    let canvas = rule.draw();
    assert!(canvas.layer_alphas.iter().any(|a| near_within(*a, p, 0.05)), "{p} {:?}", canvas.layer_alphas);
    rule.wait_for_idle();
    let f = bounds(&rule, "alan");
    assert!(near(f.left, 8.0) && near(f.width(), 384.0) && near(f.top, 4.0), "{f:?}");
    assert!(near(bounds(&rule, "sonuç").top, 4.0 + 56.0 + 8.0), "{:?}", bounds(&rule, "sonuç"));
    let (expanded_colors, scheme) = get(&colors);
    assert!(expanded_colors.container_color == scheme.surface_container_low);
    // The surface: the whole window in the expanded container color.
    let canvas = rule.draw();
    let i = canvas.path_colors.iter().position(|c| *c == scheme.surface_container_low).expect("the surface drawn");
    assert!(near(canvas.paths[i].width(), 400.0) && near(canvas.paths[i].height(), 800.0), "{:?}", canvas.paths[i]);
}

/// Compose's Expressive `ExpandedDockedSearchBarWithGap` of `rememberWithGapSearchBarState()`: a popup over the
/// window, its scrim (the scrim color at 32 %) fading in with the state, the field at the collapsed bar's place
/// and the results in a drop-down 2 dp under it (12 dp corners); a tap on the scrim collapses the state.
fn docked_search_bars_with_a_gap_drop_down_over_a_scrim() {
    let (state, scope) = (slot::<SearchBarState>(), slot::<CoroutineScope>());
    let rule = {
        let (state, scope) = (state.clone(), scope.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let s = remember_with_gap_search_bar_state();
                *state.borrow_mut() = Some(s.clone());
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                let t = remember_text_field_state();
                let (bs, bt) = (s.clone(), t.clone());
                Box(
                    modifier = Modifier.padding_sides(20.dp(), 100.dp(), 20.dp(), 0.dp()),
                    content = move || {
                        let (bs2, bt) = (bs.clone(), bt.clone());
                        SearchBar(
                            bs.clone(),
                            input_field = move || SearchBarDefaults.InputField(bt.clone(), bs2.clone(), |_: String| {}),
                            modifier = Modifier.test_tag("bar"),
                        );
                    },
                );
                let (vs, vt) = (s.clone(), t.clone());
                ExpandedDockedSearchBarWithGap(
                    s.clone(),
                    move || {
                        SearchBarDefaults.InputField(
                            vt.clone(),
                            vs.clone(),
                            |_: String| {},
                            modifier = Modifier.test_tag("alan"),
                        )
                    },
                    content = || Text("Sonuç", modifier = Modifier.fill_max_width().height(48.dp()).test_tag("sonuç")),
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let s = get(&state);
    let bar = bounds(&rule, "bar");
    let ss = s.clone();
    launch(&scope, async move { ss.animate_to_expanded().await });
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Expanded && rule.host().overlay_count() == 1);
    let f = bounds(&rule, "alan");
    assert!(near(f.left, bar.left) && near(f.top, bar.top), "{f:?} {bar:?}");
    let r = bounds(&rule, "sonuç");
    assert!(near(r.top, f.bottom + 2.0) && near(r.left, bar.left), "{r:?} {f:?}");
    let canvas = rule.draw();
    let scrim = light_color_scheme().scrim;
    let i = canvas
        .rect_colors
        .iter()
        .position(|c| c.red() == scrim.red() && near_within(c.alpha(), 0.32, 0.01))
        .expect("the scrim");
    assert!(near(canvas.rects[i].width(), 400.0) && near(canvas.rects[i].height(), 800.0));
    tap(&rule, Offset::new(200.0, 700.0)); // the scrim
    rule.wait_for_idle();
    assert!(s.current_value() == SearchBarValue::Collapsed && rule.host().overlay_count() == 0);
}

/// Compose's `enterAlwaysSearchBarScrollBehavior`: its connection slides the bar before the content scrolls (not
/// with a reversed layout, where only a pull down reveals it), counts what the content scrolled, settles a fling
/// shown or gone, moves only while `canScroll` says, and saves its offsets.
fn enter_always_scroll_behavior_settles_reverses_and_saves() {
    let (normal, reversed, frozen) = (
        slot::<Rc<dyn SearchBarScrollBehavior>>(),
        slot::<Rc<dyn SearchBarScrollBehavior>>(),
        slot::<Rc<dyn SearchBarScrollBehavior>>(),
    );
    let scope = slot::<CoroutineScope>();
    let rule = {
        let (normal, reversed, frozen, scope) = (normal.clone(), reversed.clone(), frozen.clone(), scope.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            *normal.borrow_mut() = Some(SearchBarDefaults.enter_always_search_bar_scroll_behavior());
            *reversed.borrow_mut() =
                Some(SearchBarDefaults.enter_always_search_bar_scroll_behavior(reverse_layout = true));
            *frozen.borrow_mut() =
                Some(SearchBarDefaults.enter_always_search_bar_scroll_behavior(can_scroll = || false));
            *scope.borrow_mut() = Some(remember_coroutine_scope());
        }))
    };
    let source = kompoz_ui::input::NestedScrollSource::UserInput;
    let b = get(&normal);
    assert!(b.scroll_offset_limit() == -f32::MAX && b.scroll_offset() == 0.0);
    b.set_scroll_offset_limit(-100.0);
    let c = b.nested_scroll_connection();
    assert_eq!(c.on_pre_scroll(Offset::new(3.0, -30.0), source), Offset::new(0.0, -30.0));
    assert!(b.scroll_offset() == -30.0);
    assert_eq!(c.on_pre_scroll(Offset::new(0.0, -100.0), source), Offset::new(0.0, -100.0));
    assert!(b.scroll_offset() == -100.0);
    assert_eq!(c.on_pre_scroll(Offset::new(0.0, -10.0), source), Offset::Zero); // gone: the content takes it
    assert_eq!(c.on_post_scroll(Offset::new(0.0, -10.0), Offset::Zero, source), Offset::Zero);
    assert!(b.content_offset() == -10.0);
    // Let go 60 % of the way: it settles gone; 40 %: shown.
    b.set_scroll_offset(-60.0);
    let cc = c.clone();
    launch(&scope, async move {
        cc.on_post_fling(kompoz_ui::unit::Velocity::Zero, kompoz_ui::unit::Velocity::Zero).await;
    });
    rule.wait_for_idle();
    assert!(b.scroll_offset() == -100.0, "{}", b.scroll_offset());
    b.set_scroll_offset(-40.0);
    let cc = c.clone();
    launch(&scope, async move {
        cc.on_post_fling(kompoz_ui::unit::Velocity::Zero, kompoz_ui::unit::Velocity::Zero).await;
    });
    rule.wait_for_idle();
    assert!(b.scroll_offset() == 0.0, "{}", b.scroll_offset());
    // A fling down carries it on to shown, what velocity is left given back.
    b.set_scroll_offset(-80.0);
    let cc = c.clone();
    let left = Rc::new(Cell::new(f32::NAN));
    let l = left.clone();
    launch(&scope, async move {
        let v = cc.on_post_fling(kompoz_ui::unit::Velocity::Zero, kompoz_ui::unit::Velocity::new(0.0, 3000.0)).await;
        l.set(v.y);
    });
    rule.wait_for_idle();
    assert!(
        b.scroll_offset() == 0.0 && left.get() > 0.0 && left.get() < 3000.0,
        "{} {}",
        b.scroll_offset(),
        left.get()
    );
    // Compose's quirk kept: a fling up is stopped on its first frame (its check `abs(delta - consumed)` of a delta
    // under 0 against what was consumed, a distance), then the bar settles from where that frame left it.
    b.set_scroll_offset(-20.0);
    let cc = c.clone();
    launch(&scope, async move {
        cc.on_post_fling(kompoz_ui::unit::Velocity::Zero, kompoz_ui::unit::Velocity::new(0.0, -3000.0)).await;
    });
    rule.wait_for_idle();
    assert!(b.scroll_offset() == 0.0, "{}", b.scroll_offset());
    // Reversed: up isn't taken before the content; a pull down after it is, and moves the content's count too.
    let r = get(&reversed);
    r.set_scroll_offset_limit(-100.0);
    r.set_scroll_offset(-50.0);
    let rc = r.nested_scroll_connection();
    assert_eq!(rc.on_pre_scroll(Offset::new(0.0, -10.0), source), Offset::Zero);
    assert!(r.scroll_offset() == -60.0);
    assert_eq!(rc.on_post_scroll(Offset::Zero, Offset::new(0.0, 20.0), source), Offset::new(0.0, 20.0));
    assert!(r.scroll_offset() == -40.0 && r.content_offset() == 20.0);
    // Frozen: nothing.
    let f = get(&frozen);
    f.set_scroll_offset_limit(-100.0);
    assert_eq!(f.nested_scroll_connection().on_pre_scroll(Offset::new(0.0, -10.0), source), Offset::Zero);
    assert!(f.scroll_offset() == 0.0);
}

/// Compose's `AppBarWithSearch` colors: while content is scrolled under the bar (its overlapped fraction past 1 %),
/// the app bar's container animates to its scrolled color and the search bar's to its own.
fn app_bars_with_search_take_their_scrolled_colors() {
    let behavior = slot::<Rc<dyn SearchBarScrollBehavior>>();
    let rule = {
        let behavior = behavior.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let scroll = SearchBarDefaults.enter_always_search_bar_scroll_behavior();
                *behavior.borrow_mut() = Some(scroll.clone());
                let s = remember_search_bar_state();
                let t = remember_text_field_state();
                let bs = s.clone();
                AppBarWithSearch(
                    s.clone(),
                    move || SearchBarDefaults.InputField(t.clone(), bs.clone(), |_: String| {}),
                    modifier = Modifier.test_tag("çubuk"),
                    scroll_behavior = Some(scroll),
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let scheme = light_color_scheme();
    let colors_drawn = |rule: &KompozTestRule| {
        let c = rule.draw();
        (c.rect_colors.clone(), c.path_colors.clone())
    };
    let (rects, paths) = colors_drawn(&rule);
    assert!(rects.contains(&scheme.surface) || paths.contains(&scheme.surface), "{rects:?} {paths:?}");
    assert!(paths.contains(&scheme.surface_container_high) || rects.contains(&scheme.surface_container_high));
    let b = get(&behavior);
    b.set_content_offset(-20.0); // content scrolled under it
    rule.wait_for_idle();
    let (rects, paths) = colors_drawn(&rule);
    assert!(
        rects.contains(&scheme.surface_container) || paths.contains(&scheme.surface_container),
        "{rects:?} {paths:?}"
    );
    assert!(paths.contains(&scheme.surface_container_highest) || rects.contains(&scheme.surface_container_highest));
}

/// Compose's deprecated `SearchBar(query: String, …)` and `DockedSearchBar(query: String, …)`: bars of a query
/// input field, active as told.
fn deprecated_search_bars_of_a_query() {
    let query = mutable_state_of(String::new());
    let active = mutable_state_of(false);
    let docked = mutable_state_of(false);
    let searched = Rc::new(RefCell::new(String::new()));
    let rule = {
        let (query, active, docked, searched) = (query.clone(), active.clone(), docked.clone(), searched.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let (q, a1, a2, s) = (query.clone(), active.clone(), active.clone(), searched.clone());
                if docked.get() {
                    StringDockedSearchBar(
                        query.get(),
                        move |v: String| q.set(v),
                        move |v: String| *s.borrow_mut() = v,
                        active.get(),
                        move |v: bool| a1.set(v),
                        modifier = Modifier.test_tag("bar"),
                        placeholder = Some(Rc::new(|| Text("Ara"))),
                        content = || Text("Öneri"),
                    );
                } else {
                    StringSearchBar(
                        query.get(),
                        move |v: String| q.set(v),
                        move |v: String| *s.borrow_mut() = v,
                        active.get(),
                        move |v: bool| a2.set(v),
                        modifier = Modifier.test_tag("bar"),
                        placeholder = Some(Rc::new(|| Text("Ara"))),
                        content = || Text("Öneri"),
                    );
                }
            }),
            IntSize::new(400, 800),
        )
    };
    for is_docked in [false, true] {
        docked.set(is_docked);
        active.set(false);
        query.set(String::new());
        rule.wait_for_idle();
        rule.on_node_with_text("Ara").assert_exists();
        rule.on_node_with_content_description("Search").perform_click();
        rule.wait_for_idle();
        assert!(active.peek() && rule.on_node_with_text("Öneri").exists(), "{is_docked}");
        rule.host().text_input("kedi");
        rule.wait_for_idle();
        assert_eq!(query.peek(), "kedi");
        rule.host().ime_action(kompoz_ui::platform::ImeAction::Search);
        assert_eq!(*searched.borrow(), query.peek());
        escape(&rule);
        rule.wait_for_idle();
        assert!(!active.peek());
    }
}

/// Compose's `SearchBarDefaults` and colors: the tokens' values.
fn search_bar_defaults_are_the_tokens() {
    let got =
        slot::<(SearchBarColors, AppBarWithSearchColors, TextFieldColors, ColorScheme, WindowInsets, WindowInsets)>();
    let rule = {
        let got = got.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            *got.borrow_mut() = Some((
                SearchBarDefaults.colors(),
                SearchBarDefaults.app_bar_with_search_colors(),
                SearchBarDefaults.input_field_colors(),
                MaterialTheme::color_scheme(),
                SearchBarDefaults::window_insets(),
                SearchBarDefaults::full_screen_window_insets(),
            ));
        }))
    };
    let _ = &rule;
    let (colors, app_bar, field, scheme, _insets, _full_screen) = get(&got);
    assert!(colors.container_color == scheme.surface_container_high && colors.divider_color == scheme.outline);
    assert!(colors.input_field_colors == field);
    assert!(field.focused_text_color == scheme.on_surface && field.unfocused_text_color == scheme.on_surface);
    assert!(
        field.focused_placeholder_color == scheme.on_surface_variant
            && field.focused_leading_icon_color == scheme.on_surface
    );
    assert!(
        field.focused_trailing_icon_color == scheme.on_surface_variant
            && field.focused_container_color == Color::Transparent
    );
    assert!(
        field.cursor_color == scheme.primary
            && field.disabled_text_color == named_args!(scheme.on_surface.copy(alpha = 0.38))
    );
    assert!(
        app_bar.search_bar_colors == colors
            && app_bar.scrolled_search_bar_container_color == scheme.surface_container_highest
    );
    assert!(
        app_bar.app_bar_container_color == scheme.surface
            && app_bar.scrolled_app_bar_container_color == scheme.surface_container
    );
    assert!(
        app_bar.app_bar_navigation_icon_color == scheme.on_surface
            && app_bar.app_bar_action_icon_color == scheme.on_surface_variant
    );
    assert!(SearchBarDefaults::TonalElevation == Dp(0.0) && SearchBarDefaults::ShadowElevation == Dp(0.0));
    assert!(SearchBarDefaults::InputFieldHeight == Dp(56.0) && SearchBarDefaults::DockedDropdownGapSize == Dp(2.0));
    assert!(SearchBarDefaults::AppBarContentPadding() == PaddingValues::all(0.dp()));
    assert!(SearchBarDefaults::docked_dropdown_shape() == RoundedCornerShape::new(12.dp(), 12.dp(), 12.dp(), 12.dp()));
    let swapped = named_args!(colors.copy(divider_color = Color::Red));
    assert!(swapped.divider_color == Color::Red && swapped.container_color == colors.container_color);
    let plain = AppBarWithSearchColors::without_scrolled_colors(colors, Color::Blue, Color::Red, Color::Green);
    assert!(
        plain.scrolled_search_bar_container_color == Color::Unspecified && plain.app_bar_container_color == Color::Blue
    );
    let old = SearchBarColors::without_input_field_colors(Color::Blue, Color::Red);
    assert!(old.input_field_colors.focused_text_color == Color::Unspecified);
}

// ---- M15's samples ---------------------------------------------------------------------------------------------

/// SearchBarSamples.kt's private `SampleSearchResults`.
#[kompozable]
fn SampleSearchResults(on_result_click: Rc<dyn Fn(String)>) {
    Column(
        modifier = Modifier.vertical_scroll(&remember_scroll_state()),
        content = move || {
            for idx in 0..10 {
                let result_text = format!("Suggestion {idx}");
                let on_result_click = on_result_click.clone();
                let clicked = result_text.clone();
                ListItem(
                    headline_content = move || Text(result_text.clone()),
                    supporting_content = Some(Rc::new(|| Text("Additional info"))),
                    leading_content = Some(Rc::new(|| VectorIcon(Icons::Filled::Star(), None))),
                    colors = ListItemDefaults.colors(container_color = Color::Transparent),
                    modifier = Modifier
                        .clickable(on_click = move || on_result_click(clicked.clone()))
                        .fill_max_width()
                        .padding_hv(horizontal = 16.dp(), vertical = 4.dp()),
                );
            }
        },
    );
}

/// SearchBarSamples.kt's private `SampleLeadingIcon`.
#[kompozable]
fn SampleLeadingIcon(search_bar_state: SearchBarState, scope: CoroutineScope) {
    if search_bar_state.current_value() == SearchBarValue::Expanded {
        TooltipBox(
            position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
            tooltip = |s: &dyn TooltipScope| s.PlainTooltip(content = || Text("Back")),
            state = remember_tooltip_state(),
            content = move || {
                let (s, scope) = (search_bar_state.clone(), scope.clone());
                IconButton(
                    on_click = move || {
                        let s = s.clone();
                        named_args!(scope.launch(async move { s.animate_to_collapsed().await }));
                    },
                    content = || VectorIcon(Icons::AutoMirrored::Default::ArrowBack(), Some("Back".to_string())),
                );
            },
        );
    } else {
        VectorIcon(Icons::Default::Search(), None);
    }
}

/// SearchBarSamples.kt's private `SampleTrailingIcon`.
#[kompozable]
fn SampleTrailingIcon() {
    TooltipBox(
        position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
        tooltip = |s: &dyn TooltipScope| s.PlainTooltip(content = || Text("Mic")),
        state = remember_tooltip_state(),
        content = || {
            IconButton(on_click = || {}, content = || VectorIcon(Icons::Default::Mic(), Some("Mic".to_string())));
        },
    );
}

/// SearchBarSamples.kt's private `SampleNavigationIcon` / `SampleActions`: a tooltip box of an icon button, sliding
/// out to its side while the state heads for expanded (when animated).
#[kompozable]
fn SampleSideIcon(
    state: SearchBarState,
    is_animated: bool,
    label: &'static str,
    icon: fn() -> ImageVector,
    from_end: bool,
) {
    AnimatedVisibility(
        visible = !is_animated || state.target_value() == SearchBarValue::Collapsed,
        enter = kompoz_animation::slide_in(
            MaterialTheme::motion_scheme().fast_spatial_spec::<IntOffset>(),
            move |it: IntSize| IntOffset::new(if from_end { it.width } else { -it.width }, 0),
        ),
        exit = kompoz_animation::slide_out(
            Arc::new(kompoz_animation::TweenSpec::<IntOffset>::new(150, 0, kompoz_animation::FastOutSlowInEasing)),
            move |it: IntSize| IntOffset::new(if from_end { it.width } else { -it.width }, 0),
        ),
        content = move |_: &kompoz_animation::AnimatedVisibilityScope| {
            TooltipBox(
                position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                tooltip = move |s: &dyn TooltipScope| s.PlainTooltip(content = move || Text(label)),
                state = remember_tooltip_state(),
                content = move || {
                    IconButton(on_click = || {}, content = move || VectorIcon(icon(), Some(label.to_string())));
                },
            );
        },
    );
}

/// SearchBarSamples.kt's `SimpleSearchBarSample`.
#[kompozable]
fn SimpleSearchBarSample() {
    let search_bar_state = remember_search_bar_state();
    let text_field_state = remember_text_field_state();
    let scope = remember_coroutine_scope();
    let input_field: Rc<dyn Fn()> = {
        let (s, t, scope) = (search_bar_state.clone(), text_field_state.clone(), scope.clone());
        Rc::new(move || {
            let (s1, s2, scope1, scope2) = (s.clone(), s.clone(), scope.clone(), scope.clone());
            SearchBarDefaults.InputField(
                t.clone(),
                s.clone(),
                move |_: String| {
                    let s = s1.clone();
                    named_args!(scope1.launch(async move { s.animate_to_collapsed().await }));
                },
                placeholder = Some(Rc::new(|| Text("Search", modifier = Modifier.clear_and_set_semantics(|_| {})))),
                leading_icon = Some(Rc::new(move || SampleLeadingIcon(s2.clone(), scope2.clone()))),
                trailing_icon = Some(Rc::new(|| SampleTrailingIcon())),
            );
        })
    };

    let f = input_field.clone();
    SearchBar(search_bar_state.clone(), input_field = move || f());
    let f = input_field.clone();
    let (s, t) = (search_bar_state.clone(), text_field_state.clone());
    ExpandedFullScreenSearchBar(
        search_bar_state.clone(),
        input_field = move || f(),
        content = move || {
            let (s, t, scope) = (s.clone(), t.clone(), scope.clone());
            SampleSearchResults(Rc::new(move |result: String| {
                t.set_text_and_place_cursor_at_end(&result);
                let s = s.clone();
                named_args!(scope.launch(async move { s.animate_to_collapsed().await }));
            }));
        },
    );
}

fn simple_search_bar_sample() {
    let rule = KompozTestRule::with_viewport(SimpleSearchBarSample, IntSize::new(400, 800));
    // (its placeholder's semantics cleared, as the sample has it: the field is found by its description)
    rule.on_node_with_content_description("Search").assert_exists();
    rule.on_node_with_content_description("Mic").assert_exists();
    assert!(!rule.on_node_with_text("Suggestion 3").exists());
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Suggestion 3").assert_exists();
    assert_eq!(rule.on_all_nodes_with_content_description("Back").fetch_semantics_nodes().len(), 2);
    rule.on_node_with_text("Suggestion 3").perform_click();
    rule.wait_for_idle();
    assert!(!rule.on_node_with_text("Suggestion 0").exists() && rule.host().overlay_count() == 0);
    rule.on_node_with_content_description("Search").assert_text_equals_with(&["Suggestion 3"], true);
    // Again, and the back button collapses it.
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    // (the bar's field and the view's both show the state's back button now: the view's is on top)
    rule.on_all_nodes_with_content_description("Back").on_last().perform_click();
    rule.wait_for_idle();
    assert!(rule.host().overlay_count() == 0);
}

/// SearchBarSamples.kt's `FullScreenSearchBarScaffoldSample` (its list of 100 texts: the rows of `ScaffoldOfList`).
#[kompozable]
fn FullScreenSearchBarScaffoldSample() {
    let text_field_state = remember_text_field_state();
    let search_bar_state = remember_contained_search_bar_state();
    let scope = remember_coroutine_scope();
    let scroll_behavior = SearchBarDefaults.enter_always_search_bar_scroll_behavior();
    let app_bar_with_search_colors = SearchBarDefaults
        .app_bar_with_search_colors(search_bar_colors = SearchBarDefaults.contained_colors(search_bar_state.clone()));
    let input_field = scaffold_sample_field(&text_field_state, &search_bar_state, &scope, app_bar_with_search_colors);
    let (s, f, sb) = (search_bar_state.clone(), input_field.clone(), scroll_behavior.clone());
    let (t, scope2) = (text_field_state.clone(), scope.clone());
    ScaffoldOfList(
        Rc::new(move || {
            let f1 = f.clone();
            AppBarWithSearch(
                s.clone(),
                move || f1(),
                navigation_icon = Some(sample_side_icon(&s, true, "Menu", Icons::Default::Menu, false)),
                actions = Some(sample_side_icon(&s, true, "Account", Icons::Default::AccountCircle, true)),
                colors = app_bar_with_search_colors,
                scroll_behavior = Some(sb.clone()),
            );
            let f2 = f.clone();
            let (s2, t, scope2) = (s.clone(), t.clone(), scope2.clone());
            ExpandedFullScreenContainedSearchBar(
                s.clone(),
                move || f2(),
                colors = Some(app_bar_with_search_colors.search_bar_colors),
                content = move || {
                    let (s2, t, scope2) = (s2.clone(), t.clone(), scope2.clone());
                    SampleSearchResults(Rc::new(move |result: String| {
                        t.set_text_and_place_cursor_at_end(&result);
                        let s = s2.clone();
                        named_args!(scope2.launch(async move { s.animate_to_collapsed().await }));
                    }));
                },
            );
        }),
        None,
        vec![scroll_behavior.nested_scroll_connection()],
        remember_lazy_list_state(),
        "Text",
    );
}

/// The samples' `inputField` of a scaffold sample: an input field of the app bar's colors.
fn scaffold_sample_field(
    text_field_state: &TextFieldState,
    search_bar_state: &SearchBarState,
    scope: &CoroutineScope,
    colors: AppBarWithSearchColors,
) -> Rc<dyn Fn()> {
    let (t, s, scope) = (text_field_state.clone(), search_bar_state.clone(), scope.clone());
    Rc::new(kompozable_lambda!(move || {
        let (s1, s2, scope1, scope2) = (s.clone(), s.clone(), scope.clone(), scope.clone());
        SearchBarDefaults.InputField(
            t.clone(),
            s.clone(),
            move |_: String| {
                let s = s1.clone();
                named_args!(scope1.launch(async move { s.animate_to_collapsed().await }));
            },
            colors = colors.search_bar_colors.input_field_colors,
            placeholder = Some(Rc::new(|| Text("Search", modifier = Modifier.clear_and_set_semantics(|_| {})))),
            leading_icon = Some(Rc::new(move || SampleLeadingIcon(s2.clone(), scope2.clone()))),
            trailing_icon = Some(Rc::new(|| SampleTrailingIcon())),
        );
    }))
}

fn sample_side_icon(
    state: &SearchBarState,
    is_animated: bool,
    label: &'static str,
    icon: fn() -> ImageVector,
    from_end: bool,
) -> Rc<dyn Fn()> {
    let state = state.clone();
    Rc::new(kompozable_lambda!(move || SampleSideIcon(state.clone(), is_animated, label, icon, from_end)))
}

fn full_screen_search_bar_scaffold_sample() {
    let rule = KompozTestRule::with_viewport(FullScreenSearchBarScaffoldSample, IntSize::new(400, 800));
    rule.on_node_with_content_description("Menu").assert_exists();
    rule.on_node_with_content_description("Account").assert_exists();
    rule.on_node_with_text("Text 0").assert_exists();
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Suggestion 0").assert_exists();
    // (animated: the side icons slid away)
    assert!(!rule.on_node_with_content_description("Menu").exists());
    rule.on_node_with_text("Suggestion 2").perform_click();
    rule.wait_for_idle();
    assert!(rule.host().overlay_count() == 0 && rule.on_node_with_content_description("Menu").exists());
}

/// SearchBarSamples.kt's `DockedSearchBarScaffoldSample` (its list of 100 texts: the rows of `ScaffoldOfList`).
#[kompozable]
fn DockedSearchBarScaffoldSample() {
    let text_field_state = remember_text_field_state();
    let search_bar_state = remember_with_gap_search_bar_state();
    let scope = remember_coroutine_scope();
    let scroll_behavior = SearchBarDefaults.enter_always_search_bar_scroll_behavior();
    let app_bar_with_search_colors = SearchBarDefaults.app_bar_with_search_colors();
    let input_field = scaffold_sample_field(&text_field_state, &search_bar_state, &scope, app_bar_with_search_colors);
    let (s, f, sb) = (search_bar_state.clone(), input_field.clone(), scroll_behavior.clone());
    let (t, scope2) = (text_field_state.clone(), scope.clone());
    ScaffoldOfList(
        Rc::new(move || {
            let f1 = f.clone();
            AppBarWithSearch(
                s.clone(),
                move || f1(),
                navigation_icon = Some(sample_side_icon(&s, false, "Menu", Icons::Default::Menu, false)),
                actions = Some(sample_side_icon(&s, false, "Account", Icons::Default::AccountCircle, true)),
                colors = app_bar_with_search_colors,
                scroll_behavior = Some(sb.clone()),
            );
            let f2 = f.clone();
            let (s2, t, scope2) = (s.clone(), t.clone(), scope2.clone());
            ExpandedDockedSearchBarWithGap(
                s.clone(),
                move || f2(),
                content = move || {
                    let (s2, t, scope2) = (s2.clone(), t.clone(), scope2.clone());
                    SampleSearchResults(Rc::new(move |result: String| {
                        t.set_text_and_place_cursor_at_end(&result);
                        let s = s2.clone();
                        named_args!(scope2.launch(async move { s.animate_to_collapsed().await }));
                    }));
                },
            );
        }),
        None,
        vec![scroll_behavior.nested_scroll_connection()],
        remember_lazy_list_state(),
        "Text",
    );
}

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
