//! Material's navigation suite as Compose's material3-adaptive-navigation-suite (1.5.0-alpha17) has it: the C++
//! Kompoz's tests by name (kompoz_tests.cpp), Compose's behaviour (the types each window suits, the layouts of each
//! type, the scope's items, the insets, colors and defaults, the override API, the state and its saver, movable
//! items), and the samples of NavigationSuiteScaffoldSamples.kt.

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_foundation::layout::{Arrangement, PaddingValues, VerticalArrangement};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_extended::Icons;
use kompoz_material3::prelude::*;
use kompoz_material3_adaptive::prelude::*;
use kompoz_material3_adaptive_navigation_suite::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, has_text};
use kompoz_text::TextAlign;
use kompoz_ui::media_query::{self, MediaInfo};
use kompoz_ui::platform::{Insets, SystemInsets};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

tests!(
    window_size_classes_and_the_navigation_they_suit,
    navigation_suite_scaffold_moves_its_items_and_keeps_the_screen,
    navigation_suite_types_as_compose_1_4,
    navigation_suite_scaffold_with_its_own_items_a_primary_action_and_hiding,
    navigation_suite_items_suit_each_type,
    navigation_suite_layouts_leave_the_body_its_own_size,
    navigation_suite_types_by_window_and_their_names,
    navigation_suite_scaffold_layout_places_each_type_and_the_primary_action,
    navigation_suite_scaffold_consumes_the_insets_of_its_navigation_component,
    navigation_suite_of_scope_items_by_layout_type,
    navigation_suite_items_take_their_colors,
    navigation_suite_colors_and_defaults,
    navigation_suite_scaffold_overrides_make_the_scaffolds_below,
    navigation_suite_scaffold_state_animates_and_is_saved,
    navigation_suite_items_keep_their_state_as_the_type_changes,
    navigation_suite_items_follow_the_arguments_they_capture,
    navigation_suite_scaffold_sample,
    navigation_suite_scaffold_custom_config_sample,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// The bounds of the one placed node tagged `tag` (others may be composed but not placed).
fn placed_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    let nodes: Vec<_> =
        rule.on_all_nodes_with_tag(tag).fetch_semantics_nodes().into_iter().filter(|n| n.is_placed()).collect();
    assert_eq!(nodes.len(), 1, "{tag}");
    nodes[0].unclipped_bounds_in_root()
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// A text's own bounds (in the unmerged tree: an item merges its label).
fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    rule.on_node_with(has_text(text, false, false), true).get_unclipped_bounds_in_root()
}

/// A nullable composable slot (Kotlin's `(@Composable () -> Unit)?`).
fn slot(f: impl Fn() + 'static) -> Option<Rc<dyn Fn()>> {
    Some(Rc::new(f))
}

type Slot<T> = Rc<RefCell<Option<T>>>;

fn new_slot<T>() -> Slot<T> {
    Rc::new(RefCell::new(None))
}

fn get<T: Clone>(s: &Slot<T>) -> T {
    s.borrow().clone().expect("set by the composition")
}

/// Runs `f` in the composition's coroutine scope (a suspending call of the scaffold's state).
fn launch(scope: &Slot<CoroutineScope>, f: impl std::future::Future<Output = ()> + 'static) {
    named_args!(get(scope).launch(f));
}

fn info(width: f32, height: f32, tabletop: bool) -> WindowAdaptiveInfo {
    WindowAdaptiveInfo(WindowSizeClass::compute(width, height), Posture::new_tabletop(tabletop))
}

/// A posture, tabletop or not (Kotlin's `Posture(isTabletop = …)`).
trait PostureOf {
    fn new_tabletop(tabletop: bool) -> Posture;
}

impl PostureOf for Posture {
    fn new_tabletop(tabletop: bool) -> Posture {
        Posture { is_tabletop: tabletop, hinge_list: Vec::new() }
    }
}

// ---- The C++ Kompoz's tests ------------------------------------------------------------------------------------

fn window_size_classes_and_the_navigation_they_suit() {
    let phone = WindowSizeClass::compute(400.0, 800.0);
    assert!(
        phone.window_width_size_class() == WindowWidthSizeClass::COMPACT
            && phone.window_height_size_class() == WindowHeightSizeClass::MEDIUM
    );
    assert!(
        phone.is_height_at_least_breakpoint(WindowSizeClass::HEIGHT_DP_MEDIUM_LOWER_BOUND)
            && !phone.is_width_at_least_breakpoint(WindowSizeClass::WIDTH_DP_MEDIUM_LOWER_BOUND)
    );
    let medium = WindowSizeClass::compute(700.0, 1000.0);
    assert!(
        medium.window_width_size_class() == WindowWidthSizeClass::MEDIUM
            && medium.window_height_size_class() == WindowHeightSizeClass::EXPANDED
    );
    assert!(
        WindowSizeClass::compute(600.0, 480.0).window_width_size_class() == WindowWidthSizeClass::MEDIUM
            && WindowSizeClass::compute(599.5, 479.0).window_height_size_class() == WindowHeightSizeClass::COMPACT
    );
    let expanded = WindowSizeClass::compute(840.0, 900.0);
    assert!(expanded.window_width_size_class() == WindowWidthSizeClass::EXPANDED && expanded.min_height_dp() == 900);
    let suits = |w: f32, h: f32| NavigationSuiteScaffoldDefaults::calculate_from_adaptive_info(info(w, h, false));
    assert!(
        suits(400.0, 800.0) == NavigationSuiteType::NavigationBar
            && suits(700.0, 1000.0) == NavigationSuiteType::NavigationRail
    );
    // a phone on its side
    assert!(
        suits(1280.0, 800.0) == NavigationSuiteType::NavigationRail
            && suits(900.0, 400.0) == NavigationSuiteType::NavigationBar
    );
}

fn navigation_suite_scaffold_moves_its_items_and_keeps_the_screen() {
    // The C++'s scaffold of scope items is Compose's NavigationSuiteScaffold(navigationSuiteItems, layoutType): a
    // bar, a rail or a drawer (M15's NavigationBar 80 dp over the navigation bar's inset).
    let tab = mutable_state_of(0);
    let forced: MutableState<Option<NavigationSuiteType>> = mutable_state_of(None);
    let seen: Rc<Cell<(Dp, Dp)>> = Rc::new(Cell::new((Dp(-1.0), Dp(-1.0))));
    let (t, f, s) = (tab.clone(), forced.clone(), seen.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (t, s) = (t.clone(), s.clone());
            let layout_type = f.get().unwrap_or_else(|| {
                NavigationSuiteScaffoldDefaults::calculate_from_adaptive_info(current_window_adaptive_info())
            });
            NavigationSuiteScaffoldWithLayoutType(
                navigation_suite_items = move |nav: &mut NavigationSuiteScope| {
                    let (t0, t1) = (t.clone(), t.clone());
                    nav.item(
                        t.get() == 0,
                        move || t0.set(0),
                        || VectorIcon(Icons::Filled::Home(), None),
                        label = slot(|| Text("Ev")),
                    );
                    nav.item(
                        t.get() == 1,
                        move || t1.set(1),
                        || VectorIcon(Icons::Filled::Settings(), None),
                        label = slot(|| Text("Ayarlar")),
                    );
                },
                modifier = Modifier.fill_max_size(),
                layout_type = layout_type,
                content = move || {
                    let s = s.clone();
                    Scaffold(
                        modifier = Modifier.fill_max_size(),
                        content = move |p: PaddingValues| {
                            s.set((p.calculate_top_padding(), p.calculate_bottom_padding()));
                            let count = remember(|| mutable_state_of(0));
                            let p = p.clone();
                            Box(
                                modifier = Modifier
                                    .fill_max_size()
                                    .padding_values(p.clone())
                                    .consume_window_insets_padding_values(p)
                                    .ime_padding(),
                                content = move || {
                                    Box(modifier = Modifier.fill_max_size().test_tag("ekran"));
                                    let c = count.clone();
                                    TextButton(
                                        move || c.set(c.peek() + 1),
                                        modifier = Modifier.test_tag("sayac"),
                                        content = move || Text(format!("sayaç {}", count.get())),
                                    );
                                },
                            );
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let insets = |ime: i32| SystemInsets {
        status_bars: Insets::new(0, 24, 0, 0),
        navigation_bars: Insets::new(0, 0, 0, 48),
        ime: Insets::new(0, 0, 0, ime),
        ..Default::default()
    };
    rule.host().set_system_insets(insets(0));
    rule.wait_for_idle();
    // Compact: a bar at the bottom; the screen's Scaffold pads by the status bar only (the bar already keeps clear
    // of the navigation bar).
    assert!(text_bounds(&rule, "Ev").top > 650.0);
    assert_eq!(seen.get(), (Dp(24.0), Dp(0.0)));
    rule.on_node_with_text("Ayarlar").perform_click();
    rule.wait_for_idle();
    assert_eq!(tab.peek(), 1);
    rule.on_node_with_tag("sayac").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("sayaç 1").assert_exists();
    assert!(near(bounds(&rule, "ekran").bottom, 800.0 - 80.0 - 48.0, 0.5));
    // The keyboard up. M15 vs C++: the screen consumed only the bar's inset (48), so its ime padding is the
    // keyboard less that, from its own bottom (above the bar's 80 dp): Compose's 672 - 252, where the C++ ended
    // the screen at the keyboard's top (500). A keyboard lower than the bar still pads by what is left of it
    // (Compose's 672 - 52; the C++'s nothing).
    rule.host().set_system_insets(insets(300));
    rule.wait_for_idle();
    assert!(
        near(bounds(&rule, "ekran").bottom, 800.0 - 80.0 - 48.0 - (300.0 - 48.0), 0.5),
        "{:?}",
        bounds(&rule, "ekran")
    );
    rule.host().set_system_insets(insets(100));
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "ekran").bottom, 800.0 - 80.0 - 48.0 - (100.0 - 48.0), 0.5));
    rule.host().set_system_insets(insets(0));
    // Wider: a rail at the start, the screen beside it, still where it was.
    rule.host().set_viewport_size(IntSize::new(1000, 800));
    rule.wait_for_idle();
    let home = text_bounds(&rule, "Ev");
    assert!(home.left < 80.0 && home.top < 300.0, "{home:?}");
    rule.on_node_with_text("sayaç 1").assert_exists();
    // the rail doesn't keep clear of the bottom
    assert!(bounds(&rule, "sayac").left >= 80.0);
    assert_eq!(seen.get().1, Dp(48.0));
    forced.set(Some(NavigationSuiteType::NavigationDrawer)); // asked for: a drawer
    rule.wait_for_idle();
    assert!(bounds(&rule, "sayac").left > 250.0);
    rule.on_node_with_text("sayaç 1").assert_exists();
    rule.on_node_with_text("Ayarlar").assert_exists();
    forced.set(Some(NavigationSuiteType::None));
    rule.wait_for_idle();
    rule.on_node_with_text("Ayarlar").assert_does_not_exist();
    assert!(near(bounds(&rule, "sayac").left, 0.0, 0.5));
    rule.on_node_with_text("sayaç 1").assert_exists();
}

fn navigation_suite_types_as_compose_1_4() {
    type T = NavigationSuiteType;
    type D = NavigationSuiteScaffoldDefaults;
    assert_eq!(D::navigation_suite_type(info(400.0, 800.0, false)), T::ShortNavigationBarCompact);
    assert_eq!(D::navigation_suite_type(info(900.0, 400.0, false)), T::ShortNavigationBarMedium); // a phone on its side
    assert_eq!(D::navigation_suite_type(info(700.0, 1000.0, true)), T::ShortNavigationBarMedium);
    assert_eq!(D::navigation_suite_type(info(1000.0, 800.0, false)), T::WideNavigationRailCollapsed);
    assert_eq!(D::navigation_suite_type(info(700.0, 1000.0, false)), T::WideNavigationRailCollapsed);
    assert_eq!(D::calculate_from_adaptive_info(info(700.0, 1000.0, true)), T::NavigationBar);
    assert_eq!(D::calculate_from_adaptive_info(info(700.0, 1000.0, false)), T::NavigationRail);
    // The window's posture: the C++ took its host's media posture; Compose for desktop's is always the default one
    // (postures come from Android's folding features), so the host's Tabletop leaves it flat — the tabletop types
    // are checked above with a WindowAdaptiveInfo made so.
    let seen: Slot<WindowAdaptiveInfo> = new_slot();
    let s = seen.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *s.borrow_mut() = Some(current_window_adaptive_info());
        }),
        IntSize::new(700, 1000),
    );
    assert_eq!(get(&seen).window_posture, Posture());
    rule.host().set_media_info(MediaInfo { window_posture: media_query::Posture::Tabletop, ..MediaInfo::default() });
    rule.wait_for_idle();
    assert_eq!(get(&seen).window_posture, Posture());
}

fn navigation_suite_scaffold_with_its_own_items_a_primary_action_and_hiding() {
    let tab = mutable_state_of(0);
    let state: Slot<Rc<dyn NavigationSuiteScaffoldState>> = new_slot();
    let scope: Slot<CoroutineScope> = new_slot();
    let (t, st, sc) = (tab.clone(), state.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = remember_navigation_suite_scaffold_state();
            *st.borrow_mut() = Some(s.clone());
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let t = t.clone();
            NavigationSuiteScaffold(
                navigation_items = move || {
                    let (t0, t1) = (t.clone(), t.clone());
                    NavigationSuiteItem(
                        t.get() == 0,
                        move || t0.set(0),
                        || VectorIcon(Icons::Filled::Home(), Some(String::new())),
                        slot(|| Text("Ev")),
                    );
                    NavigationSuiteItem(
                        t.get() == 1,
                        move || t1.set(1),
                        || VectorIcon(Icons::Filled::Settings(), Some(String::new())),
                        slot(|| Text("Ayarlar")),
                        badge = slot(|| Badge(content = slot(|| Text("3")))),
                    );
                },
                modifier = Modifier.fill_max_size(),
                state = s,
                primary_action_content = || Box(modifier = Modifier.size(56.dp()).test_tag("eylem")),
                content = || Box(modifier = Modifier.fill_max_size().test_tag("ekran")),
            );
        }),
        IntSize::new(400, 800),
    );
    // Compact: a short bar (64 dp) at the bottom, its icons above the labels; the action above its end, 16 dp in;
    // the screen above the bar.
    let (home, screen, action) = (text_bounds(&rule, "Ev"), bounds(&rule, "ekran"), bounds(&rule, "eylem"));
    assert!(near(screen.height(), 800.0 - 64.0, 0.5) && home.top > 736.0, "{screen:?} {home:?}");
    rule.on_node_with_text("3").assert_exists();
    assert!(near(action.left, 400.0 - 16.0 - 56.0, 0.5) && near(action.top, 800.0 - 64.0 - 16.0 - 56.0, 0.5));
    rule.on_node_with_text("Ayarlar").perform_click();
    rule.wait_for_idle();
    assert_eq!(tab.peek(), 1);
    // Hidden: the bar slides down, the screen takes its room (the action comes down with it).
    let s = get(&state);
    assert_eq!(s.current_value(), NavigationSuiteScaffoldValue::Visible);
    let st = s.clone();
    launch(&scope, async move { st.hide().await });
    rule.advance_frame();
    rule.advance_frame();
    assert_eq!(s.target_value(), NavigationSuiteScaffoldValue::Hidden);
    assert_eq!(s.current_value(), NavigationSuiteScaffoldValue::Hidden);
    assert!(s.is_animating());
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "ekran").height(), 800.0, 0.5));
    assert!(text_bounds(&rule, "Ev").top >= 800.0);
    assert!(near(bounds(&rule, "eylem").top, 800.0 - 16.0 - 56.0, 0.5));
    let st = s.clone();
    launch(&scope, async move { st.show().await });
    rule.advance_frame();
    rule.advance_frame(); // on its way in: still Hidden until all the way
    assert_eq!(s.target_value(), NavigationSuiteScaffoldValue::Visible);
    assert_eq!(s.current_value(), NavigationSuiteScaffoldValue::Hidden);
    let st = s.clone();
    launch(&scope, async move { st.snap_to(NavigationSuiteScaffoldValue::Visible).await });
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "ekran").height(), 800.0 - 64.0, 0.5));
    assert_eq!(s.current_value(), NavigationSuiteScaffoldValue::Visible);
    // Wider: a collapsed wide rail (96 dp) at the start, the action at its top, the screen beside it.
    rule.host().set_viewport_size(IntSize::new(1000, 800));
    rule.wait_for_idle();
    let (home, screen, action) = (text_bounds(&rule, "Ev"), bounds(&rule, "ekran"), placed_bounds(&rule, "eylem"));
    assert!(near(screen.left, 96.0, 0.5) && near(screen.width(), 1000.0 - 96.0, 0.5), "{screen:?}");
    assert!(home.left < 96.0 && action.left < 96.0 && action.top < home.top, "{home:?} {action:?}");
    // M15 vs C++ (one action node, in the rail): Compose's scaffold also composes its layout's action box (placed
    // for the bars only), not placed beside a rail — its node stays in the tree, unplaced.
    let actions = rule.on_all_nodes_with_tag("eylem").fetch_semantics_nodes();
    assert_eq!(actions.len(), 2);
    assert_eq!(actions.iter().filter(|n| n.is_placed()).count(), 1);
    let st = s.clone();
    launch(&scope, async move { st.toggle().await }); // hidden: out to the start
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "ekran").left, 0.0, 0.5));
    assert!(text_bounds(&rule, "Ev").right <= 0.5);
}

fn navigation_suite_items_suit_each_type() {
    let ty = mutable_state_of(NavigationSuiteType::NavigationRail);
    let clicked = Rc::new(Cell::new(false));
    let (tt, c) = (ty.clone(), clicked.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let t = tt.get();
            let c = c.clone();
            NavigationSuite(
                t,
                modifier = Modifier.test_tag("suite"),
                vertical_arrangement = Arrangement::Bottom,
                primary_action_content = || Box(modifier = Modifier.size(40.dp()).test_tag("başlık")),
                content = move || {
                    let c = c.clone();
                    NavigationSuiteItem(
                        true,
                        || {},
                        || VectorIcon(Icons::Filled::Home(), Some(String::new())),
                        slot(|| Text("Ev")),
                        navigation_suite_type = t,
                    );
                    NavigationSuiteItem(
                        false,
                        move || c.set(true),
                        || VectorIcon(Icons::Filled::Settings(), Some(String::new())),
                        slot(|| Text("Kapalı")),
                        navigation_suite_type = t,
                        enabled = false,
                    );
                },
            );
        }),
        IntSize::new(1000, 800),
    );
    // A rail: the header at the top, the items at the bottom.
    assert!(bounds(&rule, "başlık").top < 100.0 && text_bounds(&rule, "Kapalı").top > 650.0);
    ty.set(NavigationSuiteType::NavigationDrawer);
    rule.wait_for_idle();
    assert!(bounds(&rule, "suite").width() > 200.0 && text_bounds(&rule, "Kapalı").top > 650.0);
    ty.set(NavigationSuiteType::ShortNavigationBarMedium); // labels after the icons
    rule.wait_for_idle();
    let label = text_bounds(&rule, "Ev");
    assert!(bounds(&rule, "suite").height() < 100.0 && label.height() < 30.0);
    rule.on_node_with_tag("başlık").assert_does_not_exist();
    rule.on_node_with_text("Kapalı").perform_click(); // disabled, in each of them
    rule.wait_for_idle();
    assert!(!clicked.get());
    ty.set(NavigationSuiteType::WideNavigationRailExpanded); // labels after the icons, the rail wide
    rule.wait_for_idle();
    assert!(bounds(&rule, "suite").width() > 150.0 && text_bounds(&rule, "Kapalı").top > 650.0);
}

fn navigation_suite_layouts_leave_the_body_its_own_size() {
    // The NavigationSuiteScaffoldLayout cases (2, 3, 4) of C++ scaffold_bodies_and_carousel_items_leave_content_its_own_size_as_compose
    // (whole in kompoz-material3's scaffold.rs) and the older overload's: the body measured in a Box with no minimum
    // size keeps its own size beside a bar, a rail, or nothing.
    let which = mutable_state_of(2);
    let w = which.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let body = || Box(modifier = Modifier.size_wh(60.dp(), 40.dp()).test_tag("icerik"));
            let bar = || Box(modifier = Modifier.size_wh(80.dp(), 80.dp())); // a bar or a rail
            match w.get() {
                2 => NavigationSuiteScaffoldLayout(bar, NavigationSuiteType::NavigationBar, content = body),
                3 => NavigationSuiteScaffoldLayout(bar, NavigationSuiteType::NavigationRail, content = body),
                4 => NavigationSuiteScaffoldLayout(bar, NavigationSuiteType::None, content = body),
                // (the older overload: the same layout)
                _ => NavigationSuiteScaffoldLayoutWithLayoutType(
                    bar,
                    layout_type = NavigationSuiteType::NavigationBar,
                    content = body,
                ),
            }
        }),
        IntSize::new(400, 800),
    );
    for i in 2..=5 {
        which.set(i);
        rule.wait_for_idle();
        let b = bounds(&rule, "icerik");
        assert!(near(b.width(), 60.0, 0.01) && near(b.height(), 40.0, 0.01), "{i}: {b:?}");
    }
}

// ---- Compose's behaviour -----------------------------------------------------------------------------------------

fn navigation_suite_types_by_window_and_their_names() {
    type T = NavigationSuiteType;
    type D = NavigationSuiteScaffoldDefaults;
    // Width first: a compact width is a short bar of top icons whatever the height or posture.
    for (w, h, tabletop, suited, standard) in [
        (599.0, 1000.0, false, T::ShortNavigationBarCompact, T::NavigationBar),
        (599.0, 300.0, true, T::ShortNavigationBarCompact, T::NavigationBar),
        (600.0, 479.0, false, T::ShortNavigationBarMedium, T::NavigationBar),
        (600.0, 480.0, false, T::WideNavigationRailCollapsed, T::NavigationRail),
        (1600.0, 1000.0, true, T::ShortNavigationBarMedium, T::NavigationBar),
        (1600.0, 1000.0, false, T::WideNavigationRailCollapsed, T::NavigationRail),
    ] {
        assert_eq!(D::navigation_suite_type(info(w, h, tabletop)), suited, "{w}×{h}");
        assert_eq!(D::calculate_from_adaptive_info(info(w, h, tabletop)), standard, "{w}×{h}");
    }
    // The large classes (V2) are still not compact.
    let large = WindowAdaptiveInfo(WindowSizeClass(1200, 900), Posture());
    assert_eq!(D::navigation_suite_type(large), T::WideNavigationRailCollapsed);
    // Kotlin's toString: the description; the values' names.
    let names: Vec<String> = [
        T::ShortNavigationBarCompact,
        T::ShortNavigationBarMedium,
        T::WideNavigationRailCollapsed,
        T::WideNavigationRailExpanded,
        T::NavigationBar,
        T::NavigationRail,
        T::NavigationDrawer,
        T::None,
    ]
    .iter()
    .map(|t| t.to_string())
    .collect();
    assert_eq!(
        names,
        [
            "ShortNavigationBarCompact",
            "ShortNavigationBarMedium",
            "WideNavigationRailCollapsed",
            "WideNavigationRailExpanded",
            "NavigationBar",
            "NavigationRail",
            "NavigationDrawer",
            "None"
        ]
    );
    assert_ne!(T::NavigationBar, T::ShortNavigationBarCompact);
    assert_eq!(kompoz_runtime::kt::to_string(&T::None), "None");
    assert_eq!(
        NavigationSuiteScaffoldValue::ENTRIES.map(|v| v.to_string()),
        ["Visible".to_string(), "Hidden".to_string()]
    );
    // The default type follows the window: (WindowAdaptiveInfoDefault is currentWindowAdaptiveInfo(), without the
    // large widths).
    let seen: Slot<NavigationSuiteType> = new_slot();
    let s = seen.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            NavigationSuiteScaffold(
                navigation_items = || {},
                content = move || {
                    let s = s.clone();
                    *s.borrow_mut() =
                        Some(NavigationSuiteScaffoldDefaults::navigation_suite_type(current_window_adaptive_info()));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    assert_eq!(get(&seen), T::ShortNavigationBarCompact);
    rule.host().set_viewport_size(IntSize::new(900, 400));
    rule.wait_for_idle();
    assert_eq!(get(&seen), T::ShortNavigationBarMedium);
}

fn navigation_suite_scaffold_layout_places_each_type_and_the_primary_action() {
    let ty = mutable_state_of(NavigationSuiteType::ShortNavigationBarCompact);
    let alignment = mutable_state_of(0);
    let (t, a) = (ty.clone(), alignment.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let alignment = match a.get() {
                0 => Alignment::End,
                1 => Alignment::Start,
                _ => Alignment::CenterHorizontally,
            };
            NavigationSuiteScaffoldLayout(
                navigation_suite = || Box(modifier = Modifier.size_wh(70.dp(), 50.dp()).test_tag("nav")),
                navigation_suite_type = t.get(),
                primary_action_content = || Box(modifier = Modifier.size(40.dp()).test_tag("eylem")),
                primary_action_content_horizontal_alignment = alignment,
                content = || Box(modifier = Modifier.fill_max_size().test_tag("içerik")),
            );
        }),
        IntSize::new(400, 600),
    );
    let same = |r: Rect, l: f32, t: f32, w: f32, h: f32| {
        near(r.left, l, 0.5) && near(r.top, t, 0.5) && near(r.width(), w, 0.5) && near(r.height(), h, 0.5)
    };
    // The bar types: the component at the bottom start, the content above it exactly, the action 16 dp above it.
    for bar in [
        NavigationSuiteType::ShortNavigationBarCompact,
        NavigationSuiteType::ShortNavigationBarMedium,
        NavigationSuiteType::NavigationBar,
    ] {
        ty.set(bar);
        alignment.set(0);
        rule.wait_for_idle();
        assert!(same(bounds(&rule, "nav"), 0.0, 550.0, 70.0, 50.0), "{bar}");
        assert!(same(bounds(&rule, "içerik"), 0.0, 0.0, 400.0, 550.0), "{bar}");
        assert!(same(bounds(&rule, "eylem"), 400.0 - 16.0 - 40.0, 550.0 - 16.0 - 40.0, 40.0, 40.0), "{bar}");
    }
    alignment.set(1);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "eylem").left, 16.0, 0.5));
    alignment.set(2);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "eylem").left, (400.0 - 40.0) / 2.0, 0.5));
    // The others: the component at the start, the content beside it exactly; the action isn't placed.
    for side in [
        NavigationSuiteType::WideNavigationRailCollapsed,
        NavigationSuiteType::WideNavigationRailExpanded,
        NavigationSuiteType::NavigationRail,
        NavigationSuiteType::NavigationDrawer,
        NavigationSuiteType::None,
    ] {
        ty.set(side);
        rule.wait_for_idle();
        assert!(same(bounds(&rule, "nav"), 0.0, 0.0, 70.0, 50.0), "{side}");
        assert!(same(bounds(&rule, "içerik"), 70.0, 0.0, 330.0, 600.0), "{side}");
        assert!(!rule.on_node_with_tag("eylem").fetch_semantics_node().is_placed(), "{side}");
    }
    // Right to left: the start is the right.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            KompozitionLocalProvider(
                &[kompoz_ui::platform::LocalLayoutDirection.provides(LayoutDirection::Rtl)],
                || {
                    NavigationSuiteScaffoldLayout(
                        navigation_suite = || Box(modifier = Modifier.size_wh(70.dp(), 50.dp()).test_tag("nav")),
                        navigation_suite_type = NavigationSuiteType::NavigationRail,
                        content = || Box(modifier = Modifier.fill_max_size().test_tag("içerik")),
                    );
                },
            );
        }),
        IntSize::new(400, 600),
    );
    assert!(same(bounds(&rule, "nav"), 330.0, 0.0, 70.0, 50.0));
    assert!(same(bounds(&rule, "içerik"), 0.0, 0.0, 330.0, 600.0));
}

fn navigation_suite_scaffold_consumes_the_insets_of_its_navigation_component() {
    let ty = mutable_state_of(NavigationSuiteType::ShortNavigationBarCompact);
    let scope_items = mutable_state_of(false);
    let consumed: Rc<RefCell<Option<PaddingValues>>> = Rc::default();
    let state: Slot<Rc<dyn NavigationSuiteScaffoldState>> = new_slot();
    let scope: Slot<CoroutineScope> = new_slot();
    let (t, si, c, st, sc) = (ty.clone(), scope_items.clone(), consumed.clone(), state.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = remember_navigation_suite_scaffold_state();
            *st.borrow_mut() = Some(s.clone());
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let c = c.clone();
            let content = move || {
                let c = c.clone();
                Box(modifier = Modifier
                    .fill_max_size()
                    .on_consumed_window_insets_changed(move |i| *c.borrow_mut() = Some(i.as_padding_values())));
            };
            if si.get() {
                NavigationSuiteScaffoldWithLayoutType(
                    navigation_suite_items = |_: &mut NavigationSuiteScope| {},
                    layout_type = t.get(),
                    state = s,
                    content = content,
                );
            } else {
                NavigationSuiteScaffold(
                    navigation_items = || {},
                    navigation_suite_type = t.get(),
                    state = s,
                    content = content,
                );
            }
        }),
        IntSize::new(1000, 800),
    );
    rule.host().set_system_insets(SystemInsets {
        status_bars: Insets::new(0, 24, 0, 0),
        navigation_bars: Insets::new(10, 0, 0, 48),
        ..Default::default()
    });
    rule.wait_for_idle();
    let sides = || {
        let p = consumed.borrow().clone().expect("told");
        (
            p.calculate_left_padding(LayoutDirection::Ltr),
            p.calculate_top_padding(),
            p.calculate_right_padding(LayoutDirection::Ltr),
            p.calculate_bottom_padding(),
        )
    };
    let none = (Dp(0.0), Dp(0.0), Dp(0.0), Dp(0.0));
    let bottom = (Dp(0.0), Dp(0.0), Dp(0.0), Dp(48.0));
    let start = (Dp(10.0), Dp(0.0), Dp(0.0), Dp(0.0));
    // A bar consumes the bottom of its insets, a rail or a drawer their start, nothing else.
    for (t, expected) in [
        (NavigationSuiteType::ShortNavigationBarCompact, bottom),
        (NavigationSuiteType::ShortNavigationBarMedium, bottom),
        (NavigationSuiteType::NavigationBar, bottom),
        (NavigationSuiteType::WideNavigationRailCollapsed, start),
        (NavigationSuiteType::WideNavigationRailExpanded, start),
        (NavigationSuiteType::NavigationRail, start),
        (NavigationSuiteType::NavigationDrawer, start),
        (NavigationSuiteType::None, none),
    ] {
        ty.set(t);
        rule.wait_for_idle();
        assert_eq!(sides(), expected, "{t}");
    }
    // Hidden (and still): nothing.
    ty.set(NavigationSuiteType::ShortNavigationBarCompact);
    rule.wait_for_idle();
    let s = get(&state);
    let st = s.clone();
    launch(&scope, async move { st.hide().await });
    rule.advance_frame();
    rule.advance_frame();
    assert!(s.is_animating());
    assert_eq!(sides(), bottom); // while it moves
    rule.wait_for_idle();
    assert_eq!(sides(), none);
    let st = s.clone();
    launch(&scope, async move { st.snap_to(NavigationSuiteScaffoldValue::Visible).await });
    rule.wait_for_idle();
    // The scaffold of scope items consumes only a bar's, a rail's or a drawer's (Compose's override: none for the
    // short bars and the wide rails).
    scope_items.set(true);
    for (t, expected) in [
        (NavigationSuiteType::ShortNavigationBarCompact, none),
        (NavigationSuiteType::WideNavigationRailCollapsed, none),
        (NavigationSuiteType::NavigationBar, bottom),
        (NavigationSuiteType::NavigationRail, start),
        (NavigationSuiteType::NavigationDrawer, start),
    ] {
        ty.set(t);
        rule.wait_for_idle();
        assert_eq!(sides(), expected, "{t} (scope items)");
    }
}

fn navigation_suite_of_scope_items_by_layout_type() {
    let ty = mutable_state_of(NavigationSuiteType::NavigationBar);
    let tab = mutable_state_of(0);
    let (t, tb) = (ty.clone(), tab.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let tb = tb.clone();
            NavigationSuiteWithLayoutType(
                modifier = Modifier.test_tag("suite"),
                layout_type = t.get(),
                content = move |nav: &mut NavigationSuiteScope| {
                    let (t0, t1, t2) = (tb.clone(), tb.clone(), tb.clone());
                    nav.item(
                        tb.get() == 0,
                        move || t0.set(0),
                        || VectorIcon(Icons::Filled::Home(), Some("ev".into())),
                        label = slot(|| Text("Ev")),
                        modifier = Modifier.test_tag("ev"),
                    );
                    nav.item(
                        tb.get() == 1,
                        move || t1.set(1),
                        || VectorIcon(Icons::Filled::Star(), Some("yıldız".into())),
                        label = slot(|| Text("Yıldız")),
                        always_show_label = false,
                        badge = slot(|| Badge()),
                    );
                    // (no label: an empty text in a drawer)
                    nav.item(
                        tb.get() == 2,
                        move || t2.set(2),
                        icon = || VectorIcon(Icons::Filled::Settings(), Some("ayar".into())),
                    );
                },
            );
        }),
        IntSize::new(1000, 800),
    );
    // A NavigationBar (80 dp) of NavigationBarItems: the unselected item's label hidden while it doesn't always show.
    assert!(near(bounds(&rule, "suite").height(), 80.0, 0.5));
    rule.on_node_with_text("Ev").assert_exists();
    assert!(!rule.drawn_texts().contains(&"Yıldız".to_string()));
    rule.on_node_with_content_description("yıldız").perform_click();
    rule.wait_for_idle();
    assert_eq!(tab.peek(), 1);
    assert!(rule.drawn_texts().contains(&"Yıldız".to_string()));
    // A NavigationRail (80 dp wide) of NavigationRailItems.
    ty.set(NavigationSuiteType::NavigationRail);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "suite").width(), 80.0, 0.5) && near(bounds(&rule, "suite").height(), 800.0, 0.5));
    assert!(bounds(&rule, "ev").top < 100.0);
    // A PermanentDrawerSheet of NavigationDrawerItems (labels always shown, 56 dp pills).
    ty.set(NavigationSuiteType::NavigationDrawer);
    rule.wait_for_idle();
    let ev = bounds(&rule, "ev");
    assert!(bounds(&rule, "suite").width() >= 240.0 && near(ev.height(), 56.0, 0.5), "{ev:?}");
    rule.on_node_with_content_description("ayar").perform_click();
    rule.wait_for_idle();
    assert_eq!(tab.peek(), 2);
    // Any other type: that NavigationSuite of NavigationSuiteItems (a short bar of 64 dp).
    ty.set(NavigationSuiteType::ShortNavigationBarCompact);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "suite").height(), 64.0, 0.5));
    ty.set(NavigationSuiteType::WideNavigationRailCollapsed);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "suite").width(), 96.0, 0.5));
    // None: nothing.
    ty.set(NavigationSuiteType::None);
    rule.wait_for_idle();
    rule.on_node_with_tag("suite").assert_does_not_exist();
    rule.on_node_with_text("Ev").assert_does_not_exist();
    // The items are collected again as what they read changes.
    ty.set(NavigationSuiteType::NavigationBar);
    rule.wait_for_idle();
    tab.set(0);
    rule.wait_for_idle();
    rule.on_node_with_tag("ev").assert(kompoz_test::is_selected());
}

fn navigation_suite_items_take_their_colors() {
    // A NavigationBar type item is a short bar item 8 dp down in the bar item colors; a rail's and a drawer's of
    // NavigationItemColors are made of them; the scope's items take theirs or NavigationSuiteDefaults.itemColors().
    let ty = mutable_state_of(NavigationSuiteType::NavigationBar);
    let own = mutable_state_of(false);
    let picked = Color::from_argb(0xFF11_2233);
    let unpicked = Color::from_argb(0xFF44_5566);
    let (t, o) = (ty.clone(), own.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let t = t.get();
            let colors = if o.get() {
                Some(named_args!(NavigationItemColors::new(
                    picked,
                    picked,
                    Color::Transparent,
                    unpicked,
                    unpicked,
                    unpicked,
                    unpicked
                )))
            } else {
                None
            };
            Column(move || {
                NavigationSuite(
                    t,
                    content = move || {
                        NavigationSuiteItem(
                            true,
                            || {},
                            || Box(modifier = Modifier.size(24.dp()).test_tag("simge")),
                            slot(|| Text("Seçili")),
                            navigation_suite_type = t,
                            colors = colors,
                        );
                        NavigationSuiteItem(
                            false,
                            || {},
                            || Box(modifier = Modifier.size(24.dp())),
                            slot(|| Text("Değil")),
                            navigation_suite_type = t,
                            colors = colors,
                        );
                    },
                );
            });
        }),
        IntSize::new(1000, 800),
    );
    let color_of = |text: &str| {
        let canvas = rule.draw();
        canvas.texts.iter().position(|t| t == text).map(|i| canvas.colors[i])
    };
    let scheme = light_color_scheme();
    // The bar item colors by default: the label of a selected item in secondary, an unselected one's on surface
    // variant; the item 8 dp down in a bar at least 80 dp tall.
    assert_eq!(color_of("Seçili"), Some(scheme.secondary));
    assert_eq!(color_of("Değil"), Some(scheme.on_surface_variant));
    assert!(rule.on_node_with_tag_unmerged("simge").get_unclipped_bounds_in_root().top >= 8.0);
    own.set(true);
    rule.wait_for_idle();
    assert_eq!((color_of("Seçili"), color_of("Değil")), (Some(picked), Some(unpicked)));
    for t in [NavigationSuiteType::NavigationRail, NavigationSuiteType::NavigationDrawer] {
        ty.set(t);
        own.set(true);
        rule.wait_for_idle();
        assert_eq!((color_of("Seçili"), color_of("Değil")), (Some(picked), Some(unpicked)), "{t}");
        own.set(false);
        rule.wait_for_idle();
        let selected =
            if t == NavigationSuiteType::NavigationDrawer { scheme.on_secondary_container } else { scheme.secondary };
        assert_eq!(color_of("Seçili"), Some(selected), "{t}");
    }
    // The scope's items in their own colors.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let colors = NavigationSuiteDefaults.item_colors(
                navigation_bar_item_colors = NavigationBarItemDefaults.colors(selected_text_color = picked),
            );
            NavigationSuiteWithLayoutType(
                layout_type = NavigationSuiteType::NavigationBar,
                content = move |nav: &mut NavigationSuiteScope| {
                    nav.item(true, || {}, || {}, label = slot(|| Text("Kendi")), colors = Some(colors.clone()));
                    nav.item(false, || {}, || {}, label = slot(|| Text("Varsayılan")));
                },
            );
        }),
        IntSize::new(1000, 800),
    );
    let canvas = rule.draw();
    let color = |text: &str| canvas.texts.iter().position(|t| t == text).map(|i| canvas.colors[i]);
    assert_eq!(color("Kendi"), Some(picked));
    assert_eq!(color("Varsayılan"), Some(scheme.on_surface_variant));
}

fn navigation_suite_colors_and_defaults() {
    let seen: Slot<(NavigationSuiteColors, NavigationSuiteColors, NavigationSuiteItemColors, Color, Color)> =
        new_slot();
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let custom = NavigationSuiteDefaults
            .colors(navigation_bar_container_color = Color::Red, navigation_rail_container_color = Color::Transparent);
        *s.borrow_mut() = Some((
            NavigationSuiteDefaults.colors(),
            custom,
            NavigationSuiteDefaults.item_colors(),
            NavigationSuiteScaffoldDefaults::container_color(),
            LocalContentColor.current(),
        ));
    }));
    let (colors, custom, items, container, local_content) = get(&seen);
    let scheme = light_color_scheme();
    assert_eq!(colors.short_navigation_bar_container_color(), scheme.surface_container);
    assert_eq!(colors.short_navigation_bar_content_color(), scheme.on_surface);
    assert_eq!(
        colors.wide_navigation_rail_colors(),
        WideNavigationRailColors::new(
            scheme.surface,
            scheme.on_surface,
            scheme.surface_container,
            named_args!(scheme.scrim.copy(alpha = 0.32)),
            scheme.on_surface,
        )
    );
    assert_eq!(colors.navigation_bar_container_color(), scheme.surface_container);
    assert_eq!(colors.navigation_bar_content_color(), scheme.on_surface);
    assert_eq!(colors.navigation_rail_container_color(), scheme.surface);
    assert_eq!(colors.navigation_rail_content_color(), scheme.on_surface);
    assert_eq!(colors.navigation_drawer_container_color(), scheme.surface);
    assert_eq!(colors.navigation_drawer_content_color(), scheme.on_surface);
    // A content color defaults to its container's (the current content color for a color not the theme's).
    assert_eq!(custom.navigation_bar_container_color(), Color::Red);
    assert_eq!(custom.navigation_bar_content_color(), local_content);
    assert_eq!(custom.navigation_rail_content_color(), local_content);
    assert_eq!(items.navigation_bar_item_colors.selected_text_color, scheme.secondary);
    assert_eq!(items.navigation_rail_item_colors.selected_indicator_color, scheme.secondary_container);
    assert_eq!(container, scheme.background);
    assert_eq!(NavigationSuiteDefaults::vertical_arrangement(), VerticalArrangement::Top);
    assert_eq!(NavigationSuiteScaffoldDefaults::primary_action_content_alignment(), Alignment::End);
    let items = NavigationSuiteItemColors::new(
        items.navigation_bar_item_colors,
        items.navigation_rail_item_colors,
        items.navigation_drawer_item_colors.clone(),
    );
    assert_eq!(items.navigation_bar_item_colors().selected_text_color, scheme.secondary);
    // The scaffold: the scheme's background, its content in on background.
    let content_color = Rc::new(Cell::new(Color::Unspecified));
    let c = content_color.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let c = c.clone();
            NavigationSuiteScaffold(
                navigation_items = || {},
                modifier = Modifier.test_tag("iskele"),
                navigation_suite_type = NavigationSuiteType::None,
                content = move || c.set(LocalContentColor.current()),
            );
        }),
        IntSize::new(400, 800),
    );
    assert_eq!(content_color.get(), scheme.on_background);
    let canvas = rule.draw();
    assert!(canvas.rect_colors.contains(&scheme.background));
}

/// An override of the scaffold with a primary action: its arguments logged, its content and items in a column.
struct LoggingOverride(Rc<RefCell<Vec<String>>>);

impl NavigationSuiteScaffoldWithPrimaryActionOverride for LoggingOverride {
    fn NavigationSuiteScaffoldWithPrimaryAction(&self, scope: &NavigationSuiteScaffoldWithPrimaryActionOverrideScope) {
        self.0.borrow_mut().push(format!(
            "{} {:?} {}",
            scope.navigation_suite_type(),
            scope.state().current_value(),
            scope.primary_action_content_horizontal_alignment() == Alignment::Start
        ));
        let (items, action, content) = (scope.navigation_items(), scope.primary_action_content(), scope.content());
        kompozable_block!({
            Column(move || {
                items();
                action();
                content();
            });
        })
    }
}

/// An override of the scaffold of scope items: its items collected into texts.
struct ScopeOverride;

impl NavigationSuiteScaffoldOverride for ScopeOverride {
    fn NavigationSuiteScaffold(&self, scope: &NavigationSuiteScaffoldOverrideScope) {
        let items = scope.navigation_suite_items();
        let layout_type = scope.layout_type();
        let content = scope.content();
        kompozable_block!({
            Column(move || {
                Text(format!("tür {layout_type}"));
                let items = items.clone();
                NavigationSuiteWithLayoutType(
                    layout_type = NavigationSuiteType::NavigationRail,
                    content = move |s: &mut NavigationSuiteScope| items(s),
                );
                content();
            });
        })
    }
}

fn navigation_suite_scaffold_overrides_make_the_scaffolds_below() {
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let l = log.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let over: Rc<dyn NavigationSuiteScaffoldWithPrimaryActionOverride> = Rc::new(LoggingOverride(l.clone()));
            let scope_over: Rc<dyn NavigationSuiteScaffoldOverride> = Rc::new(ScopeOverride);
            KompozitionLocalProvider(
                &[
                    LocalNavigationSuiteScaffoldWithPrimaryActionOverride.provides(over),
                    LocalNavigationSuiteScaffoldOverride.provides(scope_over),
                ],
                || {
                    Column(|| {
                        NavigationSuiteScaffold(
                            navigation_items = || Text("öğe"),
                            navigation_suite_type = NavigationSuiteType::NavigationDrawer,
                            primary_action_content = || Text("eylem"),
                            primary_action_content_horizontal_alignment = Alignment::Start,
                            content = || Text("ekran"),
                        );
                        NavigationSuiteScaffoldWithLayoutType(
                            navigation_suite_items = |nav: &mut NavigationSuiteScope| {
                                nav.item(true, || {}, || {}, label = slot(|| Text("kapsam")));
                            },
                            layout_type = NavigationSuiteType::NavigationBar,
                            content = || Text("ikinci"),
                        );
                    });
                },
            );
        }),
        IntSize::new(1000, 800),
    );
    assert_eq!(log.borrow().first().map(String::as_str), Some("NavigationDrawer Visible true"));
    for text in ["öğe", "eylem", "ekran", "tür NavigationBar", "kapsam", "ikinci"] {
        rule.on_node_with_text(text).assert_exists();
    }
    // Overrides are equal by identity (Kotlin's objects).
    let a: Rc<dyn NavigationSuiteScaffoldOverride> = Rc::new(DefaultNavigationSuiteScaffoldOverride);
    let b: Rc<dyn NavigationSuiteScaffoldOverride> = Rc::new(ScopeOverride);
    let a2 = a.clone();
    assert!(*a == *a2 && *a != *b);
}

fn navigation_suite_scaffold_state_animates_and_is_saved() {
    // Starting hidden: the navigation component out of view, the content all of it.
    let state: Slot<Rc<dyn NavigationSuiteScaffoldState>> = new_slot();
    let scope: Slot<CoroutineScope> = new_slot();
    let (st, sc) = (state.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = remember_navigation_suite_scaffold_state(NavigationSuiteScaffoldValue::Hidden);
            *st.borrow_mut() = Some(s.clone());
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            NavigationSuiteScaffoldLayout(
                navigation_suite = || Box(modifier = Modifier.size_wh(400.dp(), 100.dp()).test_tag("nav")),
                navigation_suite_type = NavigationSuiteType::ShortNavigationBarCompact,
                state = s,
                content = || Box(modifier = Modifier.fill_max_size().test_tag("içerik")),
            );
        }),
        IntSize::new(400, 800),
    );
    let s = get(&state);
    assert_eq!(
        (s.current_value(), s.target_value(), s.is_animating()),
        (NavigationSuiteScaffoldValue::Hidden, NavigationSuiteScaffoldValue::Hidden, false)
    );
    assert!(near(bounds(&rule, "nav").top, 800.0, 0.5) && near(bounds(&rule, "içerik").height(), 800.0, 0.5));
    // Shown: the state's value moves on its spring, still Hidden until all the way; the layout follows its current
    // value, so the component comes in (on the same spring, the content following it) once the state is Visible.
    let st = s.clone();
    launch(&scope, async move { st.toggle().await });
    for _ in 0..4 {
        rule.advance_frame();
    }
    assert!(s.is_animating() && s.current_value() == NavigationSuiteScaffoldValue::Hidden);
    assert!(near(bounds(&rule, "nav").top, 800.0, 0.5));
    let mut frames = 0;
    while s.is_animating() {
        rule.advance_frame();
        frames += 1;
        assert!(frames < 120);
    }
    assert_eq!(s.current_value(), NavigationSuiteScaffoldValue::Visible);
    rule.advance_frame();
    rule.advance_frame();
    let top = bounds(&rule, "nav").top;
    assert!(top < 800.0 && top > 700.0, "{top}");
    assert!(near(bounds(&rule, "içerik").height(), top, 1.0));
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "nav").top, 700.0, 0.5));
    assert_eq!(s.current_value(), NavigationSuiteScaffoldValue::Visible);
    drop(rule);
    // Saved as its target value, restored as a state at it.
    type Shown = Slot<(Rc<dyn NavigationSuiteScaffoldState>, CoroutineScope)>;
    fn saved(registry: &SaveableStateRegistry, shown: &Shown) -> KompozTestRule {
        let (r, s) = (registry.clone(), shown.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            let s = s.clone();
            ProvideSaveableStateRegistry(&r, move || {
                *s.borrow_mut() = Some((remember_navigation_suite_scaffold_state(), remember_coroutine_scope()));
            });
        }))
    }
    let registry = SaveableStateRegistry::new(None);
    let shown: Shown = new_slot();
    let rule = saved(&registry, &shown);
    let (first, scope) = get(&shown);
    let f = first.clone();
    named_args!(scope.launch(async move { f.snap_to(NavigationSuiteScaffoldValue::Hidden).await }));
    rule.wait_for_idle();
    assert_eq!(first.current_value(), NavigationSuiteScaffoldValue::Hidden);
    let saved_values = registry.perform_save();
    drop(rule);
    let _rule = saved(&SaveableStateRegistry::new(Some(&saved_values)), &shown);
    let restored = get(&shown).0;
    assert!(!Rc::ptr_eq(&first, &restored));
    assert_eq!(restored.current_value(), NavigationSuiteScaffoldValue::Hidden);
    // The values saved by their ordinals.
    assert_eq!(NavigationSuiteScaffoldValue::Hidden.save(), SavedValue::Int(1));
    assert_eq!(NavigationSuiteScaffoldValue::restore(&SavedValue::Int(0)), Some(NavigationSuiteScaffoldValue::Visible));
    assert_eq!(NavigationSuiteScaffoldValue::restore(&SavedValue::Int(2)), None);
}

fn navigation_suite_items_keep_their_state_as_the_type_changes() {
    // NavigationSuite's content is movable (Compose's `remember(content) { movableContentOf(content) }`): what the
    // content remembers goes with it from a short bar to a rail and back as the window changes the type (inside an
    // item, only between types of the same item composable: a short bar item isn't a rail item) — while the
    // content is the same lambda (one capturing only what stays the same; the items take their type from the
    // window, as the sample's do), also when the caller composes again. A content capturing the type is another
    // lambda for each type in Compose: its items start over.
    let made = Rc::new(Cell::new(0));
    let other = mutable_state_of(0);
    let (m, o) = (made.clone(), other.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            Text(format!("çağıran {}", o.get()));
            let m = m.clone();
            NavigationSuite(
                NavigationSuiteScaffoldDefaults::navigation_suite_type(current_window_adaptive_info()),
                content = move || {
                    let m = m.clone();
                    let id = remember(move || {
                        m.set(m.get() + 1);
                        m.get()
                    });
                    NavigationSuiteItem(true, || {}, || {}, slot(move || Text(format!("kimlik {id}"))));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.on_node_with_text("kimlik 1").assert_exists();
    for (w, h, t) in [
        (1000, 800, NavigationSuiteType::WideNavigationRailCollapsed),
        (900, 400, NavigationSuiteType::ShortNavigationBarMedium),
        (400, 800, NavigationSuiteType::ShortNavigationBarCompact),
    ] {
        rule.host().set_viewport_size(IntSize::new(w, h));
        rule.wait_for_idle();
        let suited = NavigationSuiteScaffoldDefaults::navigation_suite_type(info(w as f32, h as f32, false));
        assert_eq!(suited, t);
        rule.on_node_with_text("kimlik 1").assert_exists();
        other.set(other.peek() + 1);
        rule.wait_for_idle();
        rule.on_node_with_text("kimlik 1").assert_exists();
    }
    assert_eq!(made.get(), 1);
}

#[kompozable]
fn ByValueItems(tab: i32, on_tab: impl Fn(i32) + 'static) {
    NavigationSuiteScaffold(
        navigation_items = move || {
            NavigationSuiteItem(tab == 0, move || on_tab(0), || {}, slot(|| Text("Ev")));
            NavigationSuiteItem(tab == 1, move || on_tab(1), || {}, slot(|| Text("Ayarlar")));
        },
        content = move || Text(format!("sekme {tab}")),
    );
}

/// Items given by value (a lambda capturing the tab, not a state): a new lambda when it changes, its items composed
/// again through the scaffold's override (whose lambdas pass the identity on: the movable content made anew).
fn navigation_suite_items_follow_the_arguments_they_capture() {
    let tab = mutable_state_of(0);
    let t = tab.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let t2 = t.clone();
            ByValueItems(t.get(), move |i| t2.set(i))
        }),
        IntSize::new(400, 800),
    );
    rule.on_node_with_text("Ayarlar").perform_click();
    rule.wait_for_idle();
    assert_eq!(tab.peek(), 1);
    rule.on_node_with_text("sekme 1").assert_exists();
    rule.on_node_with_text("Ayarlar").assert(kompoz_test::is_selected());
    rule.on_node_with_text("Ev").assert(kompoz_test::is_not_selected());
}

// ---- NavigationSuiteScaffoldSamples.kt -------------------------------------------------------------------------

/// The samples' screen: the type and the visibility in a centered text, a button hiding and showing the navigation
/// component.
#[kompozable(inline)]
fn SampleScreen(
    nav_suite_type: NavigationSuiteType,
    state: Rc<dyn NavigationSuiteScaffoldState>,
    scope: CoroutineScope,
) {
    // Screen content.
    Column(
        modifier = Modifier.fill_max_width(),
        horizontal_alignment = Alignment::CenterHorizontally,
        content = move || {
            Text(
                modifier = Modifier.padding(16.dp()),
                text = format!("Current NavigationSuiteType: {nav_suite_type}\nVisibility: {}", state.current_value()),
                text_align = Some(TextAlign::Center),
            );
            let (state, scope) = (state.clone(), scope.clone());
            Button(
                move || {
                    let state = state.clone();
                    named_args!(scope.launch(async move { state.toggle().await }));
                },
                content = || Text("Hide/show navigation component"),
            );
        },
    );
}

/// The samples' items: Songs, Artists and Playlists, the selected one's heart filled.
#[kompozable(inline)]
fn SampleItems(selected_item: MutableState<usize>, nav_suite_type: Option<NavigationSuiteType>) {
    let nav_items = ["Songs", "Artists", "Playlists"];
    for (index, nav_item) in nav_items.into_iter().enumerate() {
        let (selected, s) = (selected_item.clone(), selected_item.clone());
        let icon = move || {
            VectorIcon(
                if selected.get() == index { Icons::Filled::Favorite() } else { Icons::Outlined::FavoriteBorder() },
                None,
            )
        };
        match nav_suite_type {
            Some(t) => NavigationSuiteItem(
                selected_item.get() == index,
                move || s.set(index),
                icon,
                slot(move || Text(nav_item)),
                navigation_suite_type = t,
            ),
            None => NavigationSuiteItem(
                selected_item.get() == index,
                move || s.set(index),
                icon,
                slot(move || Text(nav_item)),
            ),
        }
    }
}

fn navigation_suite_scaffold_sample() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            let selected_item = remember(|| mutable_state_of(0usize));
            let nav_suite_type =
                NavigationSuiteScaffoldDefaults::navigation_suite_type(current_window_adaptive_info_v2());
            let state = remember_navigation_suite_scaffold_state();
            let scope = remember_coroutine_scope();
            let screen_state = state.clone();
            NavigationSuiteScaffold(
                state = state,
                navigation_items = move || SampleItems(selected_item.clone(), None),
                content = move || SampleScreen(nav_suite_type, screen_state.clone(), scope.clone()),
            );
        }),
        IntSize::new(400, 800),
    );
    rule.on_node_with_text("Current NavigationSuiteType: ShortNavigationBarCompact\nVisibility: Visible")
        .assert_exists();
    rule.on_node_with_text("Artists").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Artists").assert(kompoz_test::is_selected());
    assert!(text_bounds(&rule, "Artists").top > 700.0);
    rule.on_node_with_text("Hide/show navigation component").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Current NavigationSuiteType: ShortNavigationBarCompact\nVisibility: Hidden")
        .assert_exists();
    assert!(text_bounds(&rule, "Artists").top >= 800.0);
    rule.on_node_with_text("Hide/show navigation component").perform_click();
    rule.wait_for_idle();
    // A wide window: a collapsed wide rail (V2's large width class is still a rail), a short bar of start icons in a
    // short one.
    rule.host().set_viewport_size(IntSize::new(1300, 900));
    rule.wait_for_idle();
    rule.on_node_with_text("Current NavigationSuiteType: WideNavigationRailCollapsed\nVisibility: Visible")
        .assert_exists();
    assert!(text_bounds(&rule, "Songs").left < 96.0);
    rule.host().set_viewport_size(IntSize::new(900, 400));
    rule.wait_for_idle();
    rule.on_node_with_text("Current NavigationSuiteType: ShortNavigationBarMedium\nVisibility: Visible")
        .assert_exists();
}

fn navigation_suite_scaffold_custom_config_sample() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            let selected_item = remember(|| mutable_state_of(0usize));
            // Custom configuration that shows a wide navigation rail in small/medium width screens, an expanded wide
            // navigation rail in expanded width screens, and a short navigation bar in small height screens.
            let nav_suite_type = {
                let WindowAdaptiveInfo { window_size_class, .. } = current_window_adaptive_info_v2();
                if window_size_class.window_width_size_class() == WindowWidthSizeClass::COMPACT
                    || window_size_class.window_width_size_class() == WindowWidthSizeClass::MEDIUM
                {
                    NavigationSuiteType::WideNavigationRailCollapsed
                } else if window_size_class.window_height_size_class() == WindowHeightSizeClass::COMPACT {
                    NavigationSuiteType::ShortNavigationBarMedium
                } else if window_size_class.window_width_size_class() == WindowWidthSizeClass::EXPANDED {
                    NavigationSuiteType::WideNavigationRailExpanded
                } else {
                    NavigationSuiteScaffoldDefaults::navigation_suite_type(current_window_adaptive_info_v2())
                }
            };
            let state = remember_navigation_suite_scaffold_state();
            let scope = remember_coroutine_scope();
            let screen_state = state.clone();
            NavigationSuiteScaffold(
                navigation_suite_type = nav_suite_type,
                state = state,
                navigation_item_vertical_arrangement = Arrangement::Center,
                navigation_items = move || SampleItems(selected_item.clone(), Some(nav_suite_type)),
                content = move || SampleScreen(nav_suite_type, screen_state.clone(), scope.clone()),
            );
        }),
        IntSize::new(400, 800),
    );
    // Compact: a collapsed wide rail, its items in its middle.
    rule.on_node_with_text("Current NavigationSuiteType: WideNavigationRailCollapsed\nVisibility: Visible")
        .assert_exists();
    let songs = text_bounds(&rule, "Songs");
    assert!(songs.left < 96.0 && songs.top > 250.0 && songs.top < 450.0, "{songs:?}");
    rule.on_node_with_text("Playlists").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Playlists").assert(kompoz_test::is_selected());
    // Expanded: an expanded wide rail (its labels after its icons); a short height: a short bar.
    rule.host().set_viewport_size(IntSize::new(1000, 800));
    rule.wait_for_idle();
    rule.on_node_with_text("Current NavigationSuiteType: WideNavigationRailExpanded\nVisibility: Visible")
        .assert_exists();
    rule.on_node_with_text("Playlists").assert(kompoz_test::is_selected());
    rule.host().set_viewport_size(IntSize::new(1000, 400));
    rule.wait_for_idle();
    rule.on_node_with_text("Current NavigationSuiteType: ShortNavigationBarMedium\nVisibility: Visible")
        .assert_exists();
    assert!(text_bounds(&rule, "Songs").top > 300.0);
}
