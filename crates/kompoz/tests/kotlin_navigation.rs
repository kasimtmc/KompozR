//! Material's navigation bars, rails and drawers in Kotlin's syntax, as a Compose user writes them: the C++ Kompoz's
//! navigation tests written in Kotlin through `kompoz!` (its kompozable_tests.cpp has no bar, rail or drawer source),
//! and material3's samples of those components from `.kt` files (kompoz_file!).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;

// ==== navigation bars ================================================================================================
// The C++ Kompoz's navigation bar tests (kompoz_tests.cpp: navigation_bar_switches_tabs,
// navigation_bar_items_as_compose_1_4, short_navigation_bar_is_64_dp_over_the_navigation_bar,
// short_navigation_bar_ripples_start_under_the_finger, short_navigation_bar_shares_the_width_or_centres_its_items, the
// whole of layouts_that_measure_twice_probe_first_so_sizes_settle, its ButtonGroup half WP1b's) written in Kotlin, checked as
// crates/kompoz-material3/tests/navigation_bar.rs checks them (M15's values; the differences from the C++'s said
// there), and material3's NavigationBarSamples + BadgeSamples' NavigationBarItemWithBadge from kt/navigation_bar.kt.

kompoz! {
    // (the C++'s: a Scaffold's bottom bar)
    @Composable
    fun BarSwitchingTabs() {
        var tab by remember { mutableStateOf(0) }
        Scaffold(Modifier.fillMaxSize(), bottomBar = {
            NavigationBar {
                NavigationBarItem(tab == 0, { tab = 0 }, { Icon(Icons.Filled.Home, null) }, label = { Text("Bir") })
                NavigationBarItem(tab == 1, { tab = 1 }, { Icon(Icons.AutoMirrored.Filled.List, null) }, label = { Text("İki") })
            }
        }) { padding -> Text(if (tab == 0) "sayfa 1" else "sayfa 2", Modifier.padding(padding)) }
    }

    @Composable
    fun BarItems() {
        var tab by remember { mutableStateOf(0) }
        NavigationBar {
            NavigationBarItem(selected = tab == 0, onClick = { tab = 0 }, icon = { Icon(Icons.Filled.Home, null) }, label = { Text("Bir") })
            NavigationBarItem(selected = tab == 1, onClick = { tab = 1 }, icon = { Icon(Icons.AutoMirrored.Filled.List, null) }, label = { Text("İki") })
        }
    }

    @Composable
    fun ShortBarOverTheSystemBar() {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
            ShortNavigationBar(Modifier.testTag("çubuk")) {
                ShortNavigationBarItem(true, {}, { Icon(Icons.Filled.Home, "") }, { Text("Ev") }, Modifier.testTag("öğe"))
            }
        }
    }

    // (Compose's items take their icon position: at the start with the Centered arrangement, as M15's sample)
    @Composable
    fun RippledShortBar(centered: Boolean) {
        val position = if (centered) NavigationItemIconPosition.Start else NavigationItemIconPosition.Top
        ShortNavigationBar(arrangement = if (centered) ShortNavigationBarArrangement.Centered else ShortNavigationBarArrangement.EqualWeight) {
            ShortNavigationBarItem(true, {}, { Box(Modifier.size(24.dp)) }, { Text("Ana") }, Modifier.testTag("ana"), iconPosition = position)
            ShortNavigationBarItem(false, {}, { Box(Modifier.size(24.dp)) }, { Text("Ara") }, iconPosition = position)
        }
    }

    @Composable
    fun SharedShortBar(arrangement: ShortNavigationBarArrangement = ShortNavigationBarDefaults.arrangement) {
        var selected by remember { mutableStateOf(0) }
        val position =
            if (arrangement == ShortNavigationBarArrangement.Centered) NavigationItemIconPosition.Start
            else NavigationItemIconPosition.Top
        ShortNavigationBar(Modifier.testTag("çubuk"), arrangement = arrangement, windowInsets = WindowInsets()) {
            listOf("Ana", "Ara", "Profil").forEachIndexed { i, label ->
                ShortNavigationBarItem(
                    selected = selected == i,
                    onClick = { selected = i },
                    icon = { Box(Modifier.size(24.dp)) },
                    label = { Text(label) },
                    modifier = Modifier.testTag(label),
                    iconPosition = position,
                )
            }
        }
    }

    // (WP1b: group) the C++'s ButtonGroup half too: a held button squeezing its neighbour (M15's deprecated content
    // overload, as the C++ composed it)
    @Composable
    fun MeasuredTwice() {
        var squeezed by remember { mutableStateOf(0) }
        var item by remember { mutableStateOf(0) }
        var label by remember { mutableStateOf(0) }
        val first = remember { MutableInteractionSource() }
        val second = remember { MutableInteractionSource() }
        Column {
            ButtonGroup {
                Button({}, Modifier.animateWidth(first).testTag("a"), interactionSource = first) { Text("Önceki") }
                Button({}, Modifier.animateWidth(second).onSizeChanged { squeezed = it.width }, interactionSource = second) { Text("Oynat") }
            }
            ShortNavigationBar(arrangement = ShortNavigationBarArrangement.Centered, windowInsets = WindowInsets()) {
                ShortNavigationBarItem(true, {}, {}, { Text("Ana") }, Modifier.onSizeChanged { item = it.width })
                ShortNavigationBarItem(false, {}, {}, { Text("Uzun sekme") })
            }
            Button({}, Modifier.width(16.dp)) {
                Text("Dar", Modifier.onSizeChanged { label = it.height })
            }
            Text("ölçüler $squeezed $item $label")
        }
    }
}

fn kt_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// A text's own bounds (an item merges its label).
fn kt_text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    rule.on_node_with(kompoz::test::has_text(text, false, false), true).get_unclipped_bounds_in_root()
}

fn kt_near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// Whether a ripple's translucent circle is drawn centered at `at` after pressing at `press`.
fn kt_ripple_at(rule: &KompozTestRule, press: Offset, at: Offset) -> bool {
    rule.host().pointer_down(press, -1, 0, kompoz::ui::input::PointerType::Touch);
    rule.advance_frame();
    rule.draw();
    rule.advance_frame();
    let canvas = rule.draw();
    let found = canvas.rects.iter().zip(&canvas.rect_colors).any(|(r, c)| {
        kt_near(r.width(), r.height(), 0.01)
            && c.alpha() > 0.0
            && c.alpha() < 0.2
            && kt_near(r.center().x, at.x, 3.0)
            && kt_near(r.center().y, at.y, 3.0)
    });
    rule.host().pointer_up(press, -1, 0);
    rule.wait_for_idle();
    found
}

fn navigation_bar_switches_tabs() {
    let rule = KompozTestRule::with_viewport(BarSwitchingTabs, IntSize::new(400, 800));
    let label = kt_text_bounds(&rule, "İki");
    assert!(label.top > 800.0 - 80.0 && label.bottom < 800.0, "{label:?}");
    rule.on_node_with_text("İki").perform_click();
    rule.on_node_with_text("sayfa 2").assert_exists();
}

fn navigation_bar_items_as_compose_1_4() {
    let rule = KompozTestRule::with_viewport(BarItems, IntSize::new(400, 300));
    let cs = light_color_scheme();
    let first = kt_text_bounds(&rule, "Bir");
    let pills = |canvas: &kompoz::test::RecordingCanvas| -> Vec<Rect> {
        canvas
            .rects
            .iter()
            .zip(&canvas.rect_colors)
            .filter(|(_, c)| **c == cs.secondary_container)
            .map(|(r, _)| *r)
            .collect()
    };
    // M15's 56 × 32 pill (the C++'s 64), 12 dp down, the label 4 dp under it.
    let shown = pills(&rule.draw());
    assert!(
        shown.len() == 1
            && kt_near(shown[0].width(), 56.0, 0.01)
            && kt_near(shown[0].top, first.top - 36.0, 0.01)
            && kt_near(shown[0].center().x, first.center().x, 1.0),
        "{shown:?}"
    );
    rule.on_node_with_text("İki").perform_click();
    rule.wait_for_idle();
    let second = kt_text_bounds(&rule, "İki");
    let moved = pills(&rule.draw());
    assert!(moved.len() == 1 && kt_near(moved[0].center().x, second.center().x, 1.0), "{moved:?}");
    let at = Offset::new(first.center().x + 20.0, first.top - 20.0);
    assert!(kt_ripple_at(&rule, at, at)); // (and the first picked again)
    let tabs =
        rule.host().accessibility_tree().into_iter().filter(|n| n.role == Some(kompoz::ui::semantics::Role::Tab));
    assert_eq!(tabs.map(|n| n.checked).collect::<Vec<_>>(), vec![Some(true), Some(false)]);
}

fn short_navigation_bar_is_64_dp_over_the_navigation_bar() {
    let rule = KompozTestRule::with_viewport(ShortBarOverTheSystemBar, IntSize::new(400, 800));
    rule.host().set_system_insets(kompoz::ui::platform::SystemInsets {
        navigation_bars: kompoz::ui::platform::Insets::new(0, 0, 0, 48),
        ..Default::default()
    });
    rule.wait_for_idle();
    let (bar, item) = (kt_bounds(&rule, "çubuk"), kt_bounds(&rule, "öğe"));
    assert!(
        kt_near(bar.height(), 112.0, 0.01) && kt_near(item.height(), 64.0, 0.01) && kt_near(item.top, bar.top, 0.01)
    );
}

fn short_navigation_bar_ripples_start_under_the_finger() {
    // Top icon: M15 moves the press by NavigationBar's 12 dp though this pill is 6 dp down (Compose's).
    let rule = KompozTestRule::with_viewport(kompozable_lambda!(|| RippledShortBar(false)), IntSize::new(400, 300));
    let label = kt_text_bounds(&rule, "Ana");
    let press = Offset::new(label.center().x + 12.0, label.top - 20.0);
    assert!(kt_ripple_at(&rule, press, Offset::new(press.x, press.y - 6.0)));
    // Start icon: the presses not moved into the pill (Compose's): the ripple as far off as the pill's corner.
    let rule = KompozTestRule::with_viewport(kompozable_lambda!(|| RippledShortBar(true)), IntSize::new(400, 300));
    let (label, item) = (kt_text_bounds(&rule, "Ana"), kt_bounds(&rule, "ana"));
    let press = Offset::new(label.center().x, label.center().y + 6.0);
    let pill_width = 16.0 + 24.0 + 4.0 + label.width() + 16.0;
    let pill = Offset::new(((item.width() - pill_width) / 2.0).trunc(), (item.height() - 40.0) / 2.0);
    assert!(kt_ripple_at(&rule, press, press + pill));
}

fn short_navigation_bar_shares_the_width_or_centres_its_items() {
    let shared = KompozTestRule::with_viewport(kompozable_lambda!(|| SharedShortBar()), IntSize::new(360, 640));
    assert_eq!(kt_bounds(&shared, "çubuk").size(), Size::new(360.0, 64.0));
    assert_eq!((kt_bounds(&shared, "Ana").width(), kt_bounds(&shared, "Profil").left), (120.0, 240.0));
    shared.on_node_with_tag("Ana").assert_is_selected();
    shared.on_node_with_tag("Ara").perform_click();
    shared.on_node_with_tag("Ara").assert_is_selected();
    let centered = KompozTestRule::with_viewport(
        kompozable_lambda!(|| SharedShortBar(ShortNavigationBarArrangement::Centered)),
        IntSize::new(360, 640),
    );
    // Each as wide as it needs (16 + 24 + 4 + its label + 16), together in the middle of the 20% paddings.
    let width = |label: &str| 16.0 + 24.0 + 4.0 + kt_text_bounds(&centered, label).width() + 16.0;
    let start = 72.0 - ["Ana", "Ara", "Profil"].iter().map(|l| ((width(l) - 72.0) / 2.0).trunc()).sum::<f32>();
    let ana = kt_bounds(&centered, "Ana");
    assert!(kt_near(ana.left, start, 0.01) && kt_near(ana.width(), width("Ana"), 0.01), "{ana:?}");
    // (the default arrangement whatever the window: sharing the width)
    let wide = KompozTestRule::with_viewport(kompozable_lambda!(|| SharedShortBar()), IntSize::new(700, 400));
    assert!(kt_near(kt_bounds(&wide, "Ana").width(), 233.0, 0.01));
}

#[allow(deprecated)] // (WP1b: group — ButtonGroup's deprecated content overload, as the C++ composed it)
fn layouts_that_measure_twice_probe_first_so_sizes_settle() {
    let rule = KompozTestRule::new(MeasuredTwice);
    rule.wait_for_idle();
    // (WP1b: group) held and settled: the second button squeezed, 84 less 15% of 91 in whole px (the C++'s 70.35)
    let a = rule.on_node_with_tag("a").get_unclipped_bounds_in_root();
    rule.host().pointer_down(a.center(), -1, 0, kompoz::ui::input::PointerType::Touch);
    rule.advance_time_by(600);
    // M15's top-icon items, centered: each the equal share between the 25% paddings (100); the label wrapped.
    let sizes = || named_args!(rule.on_node_with_text("ölçüler 70 100 ", substring = true));
    sizes().assert_exists();
    let told = format!("{:?}", sizes().fetch_semantics_node().config());
    rule.advance_time_by(300);
    assert_eq!(format!("{:?}", sizes().fetch_semantics_node().config()), told); // settled: told no other size
}

kompoz_file!("tests/kt/navigation_bar.kt");

/// A navigation bar sample: Songs picked, then Playlists once clicked, its icons swapped.
fn kt_bar_sample(content: impl Content + 'static, viewport: IntSize) -> KompozTestRule {
    let rule = KompozTestRule::with_viewport(content, viewport);
    let selection = |rule: &KompozTestRule| -> Vec<Option<bool>> {
        let tree = rule.host().accessibility_tree();
        tree.iter().filter(|n| n.role == Some(kompoz::ui::semantics::Role::Tab)).map(|n| n.checked).collect()
    };
    assert_eq!(selection(&rule), vec![Some(true), Some(false), Some(false)]);
    let before = rule.draw().vectors.clone();
    rule.on_node_with_text("Playlists").perform_click();
    rule.wait_for_idle();
    assert_eq!(selection(&rule), vec![Some(false), Some(false), Some(true)]);
    assert!(rule.draw().vectors.contains(&Icons::Filled::Star().id()) && rule.draw().vectors != before);
    rule
}

fn navigation_bar_samples() {
    let rule = kt_bar_sample(ShortNavigationBarSample, IntSize::new(360, 640));
    assert!(kt_near(rule.on_node_with_text("Songs").get_unclipped_bounds_in_root().width(), 120.0, 0.01));
    let rule = kt_bar_sample(ShortNavigationBarWithHorizontalItemsSample, IntSize::new(700, 400));
    let songs = rule.on_node_with_text("Songs").get_unclipped_bounds_in_root();
    assert!(songs.left > 0.0 && songs.width() < 700.0 / 3.0 && kt_near(songs.height(), 64.0, 0.01), "{songs:?}");
    let rule = kt_bar_sample(NavigationBarSample, IntSize::new(400, 300));
    let tree = rule.host().accessibility_tree();
    let labels: Vec<_> =
        tree.iter().filter(|n| n.role == Some(kompoz::ui::semantics::Role::Tab)).map(|n| n.label.as_str()).collect();
    assert_eq!(labels, vec!["Songs", "Artists", "Playlists"]);
}

fn navigation_bar_item_with_badge() {
    let rule = KompozTestRule::with_viewport(NavigationBarItemWithBadge, IntSize::new(600, 300));
    for description in ["New notification", "8 new notifications", "999+ new notifications"] {
        named_args!(rule.on_node_with_content_description(description, use_unmerged_tree = true)).assert_exists();
    }
    let dot = named_args!(rule.on_node_with_content_description("New notification", use_unmerged_tree = true))
        .get_unclipped_bounds_in_root();
    assert_eq!(dot.size(), Size::new(6.0, 6.0));
}

// ==== navigation rails ===============================================================================================
// The C++ Kompoz's rail tests (kompoz_tests.cpp navigation_rail_as_compose_1_4, window_size_local_and_navigation_rail,
// navigation_rail_items_take_their_options, wide_navigation_rail_widens_and_puts_the_labels_after_the_icons,
// modal_wide_navigation_rail_opens_over_the_content_with_a_scrim) written in Kotlin as a Compose user writes them
// (shorter than the Rust ones in kompoz-material3's tests/navigation_rail.rs; M15's values), and material3's
// NavigationRailSamples from kt/navigation_rail.kt.

kompoz! {
    @Composable
    fun RailAsCompose14() {
        var tab by remember { mutableIntStateOf(0) }
        NavigationRail(Modifier.testTag("ray"), header = { Box(Modifier.size(40.dp).testTag("baş")) }) {
            NavigationRailItem(
                selected = tab == 0,
                onClick = { tab = 0 },
                icon = { Icon(Icons.Filled.Home, contentDescription = null) },
                label = { Text("Bir") },
            )
            NavigationRailItem(
                selected = tab == 1,
                onClick = { tab = 1 },
                icon = { Icon(Icons.AutoMirrored.Filled.List, contentDescription = null) },
                label = { Text("İki") },
            )
        }
    }

    @Composable
    fun WindowSizeRail() {
        val window = LocalWindowInfo.current.containerSize
        if (window.width >= 600) {
            Row(Modifier.fillMaxSize()) {
                NavigationRail(Modifier.testTag("rail")) {
                    NavigationRailItem(true, {}, { Icon(Icons.Filled.Home, null) }, label = { Text("Ana") })
                    NavigationRailItem(false, {}, { Icon(Icons.Filled.Star, null) }, label = { Text("Yıldız") })
                }
                Text("geniş")
            }
        } else {
            Text("dar")
        }
    }

    @Composable
    fun RailItemOptions() {
        var clicks by remember { mutableIntStateOf(0) }
        val own = remember { MutableInteractionSource() }
        val pressed by own.collectIsPressedAsState()
        Column {
            NavigationRail {
                NavigationRailItem(
                    selected = false,
                    onClick = { clicks++ },
                    icon = { Icon(Icons.Filled.Home, "ev", Modifier.testTag("simge")) },
                    modifier = Modifier.testTag("kapalı"),
                    label = { Text("Kapalı") },
                    enabled = false,
                )
                NavigationRailItem(
                    selected = false,
                    onClick = {},
                    icon = { Icon(Icons.AutoMirrored.Filled.List, "liste") },
                    modifier = Modifier.testTag("kendi"),
                    label = { Text("Kendi") },
                    interactionSource = own,
                    colors = NavigationRailItemDefaults.colors(unselectedTextColor = Color.Red),
                )
            }
            Text("$clicks tık")
            Text(if (pressed) "basılı" else "serbest")
        }
    }

    @Composable
    fun WideRailWidens() {
        var selected by remember { mutableIntStateOf(0) }
        val state = rememberWideNavigationRailState()
        val scope = rememberCoroutineScope()
        Row {
            WideNavigationRail(
                modifier = Modifier.testTag("ray"),
                state = state,
                header = { Box(Modifier.size(40.dp).testTag("başlık")) },
                windowInsets = WindowInsets(0.dp),
            ) {
                listOf("Gelen", "Giden").forEachIndexed { i, label ->
                    WideNavigationRailItem(
                        selected = selected == i,
                        onClick = { selected = i },
                        icon = { Box(Modifier.size(24.dp)) },
                        label = { Text(label) },
                        railExpanded = state.targetValue == WideNavigationRailValue.Expanded,
                        modifier = Modifier.testTag(label),
                    )
                }
            }
            Text("aç / kapat", Modifier.clickable { scope.launch { state.toggle() } })
        }
    }

    @Composable
    fun ModalWideRail() {
        var picked by remember { mutableIntStateOf(0) }
        var hide by remember { mutableStateOf(false) }
        val state = rememberWideNavigationRailState()
        val scope = rememberCoroutineScope()
        Row(Modifier.fillMaxSize()) {
            ModalWideNavigationRail(state = state, hideOnCollapse = hide, windowInsets = WindowInsets(0.dp)) {
                listOf("Gelen", "Giden").forEachIndexed { i, label ->
                    WideNavigationRailItem(
                        selected = picked == i,
                        onClick = { picked = i },
                        icon = { Box(Modifier.size(24.dp)) },
                        label = { Text(label) },
                        railExpanded = state.targetValue == WideNavigationRailValue.Expanded,
                        modifier = Modifier.testTag(label),
                    )
                }
            }
            Column(Modifier.weight(1f).fillMaxHeight().testTag("içerik")) {
                Text("aç", Modifier.clickable { scope.launch { state.expand() } })
                Text("gizle", Modifier.clickable { hide = true })
            }
        }
    }

    // (navigation_items_animate_composing_nothing_around_them: the three components a tab change composes once)
    @Composable
    fun ThreeNavigations(onCompose: () -> Unit) {
        onCompose()
        var tab by remember { mutableIntStateOf(0) }
        Column {
            ShortNavigationBar {
                repeat(3) { i ->
                    ShortNavigationBarItem(
                        selected = tab == i,
                        onClick = { tab = i },
                        icon = { Icon(Icons.Filled.Home, contentDescription = "") },
                        label = { Text("Kısa $i") },
                    )
                }
            }
            NavigationBar {
                repeat(3) { i ->
                    NavigationBarItem(
                        selected = tab == i,
                        onClick = { tab = i },
                        icon = { Icon(Icons.Filled.Home, contentDescription = null) },
                        label = { Text("Sekme $i") },
                    )
                }
            }
            WideNavigationRail {
                repeat(3) { i ->
                    WideNavigationRailItem(
                        selected = tab == i,
                        onClick = { tab = i },
                        icon = { Icon(Icons.Filled.Home, contentDescription = "") },
                        label = { Text("Ray $i") },
                        railExpanded = false,
                    )
                }
            }
        }
    }
}

fn rail_as_compose_1_4() {
    let rule = KompozTestRule::with_viewport(RailAsCompose14, IntSize::new(400, 600));
    let rail = rule.on_node_with_tag("ray").get_unclipped_bounds_in_root();
    let header = rule.on_node_with_tag_unmerged("baş").get_unclipped_bounds_in_root();
    let bir = rule.on_node_with(kompoz::test::has_text("Bir", false, false), true).get_unclipped_bounds_in_root();
    assert!((rail.width() - 80.0).abs() < 0.01 && (header.top - 4.0).abs() < 0.01);
    assert!((bir.top - (4.0 + 40.0 + 4.0 + 8.0 + 4.0 + 36.0)).abs() < 0.01, "{bir:?}");
    let cs = light_color_scheme();
    let canvas = rule.draw();
    let label = canvas.texts.iter().position(|t| t == "Bir").map(|i| canvas.colors[i]);
    assert_eq!(label, Some(cs.secondary));
    rule.on_node_with_text("İki").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("İki").assert_is_selected();
    rule.on_node_with_text("Bir").assert_is_not_selected();
}

fn window_size_and_rail() {
    let rule = KompozTestRule::with_viewport(WindowSizeRail, IntSize::new(400, 800));
    rule.on_node_with_text("dar").assert_exists();
    rule.host().set_viewport_size(IntSize::new(800, 600));
    rule.wait_for_idle();
    rule.on_node_with_text("geniş").assert_exists();
    assert!((rule.on_node_with_tag("rail").get_unclipped_bounds_in_root().width() - 80.0).abs() < 0.01);
}

fn rail_items_take_their_options() {
    let rule = KompozTestRule::with_viewport(RailItemOptions, IntSize::new(400, 600));
    let off = rule.on_node_with_tag("kapalı").get_unclipped_bounds_in_root();
    let icon = rule.on_node_with_tag_unmerged("simge").get_unclipped_bounds_in_root();
    assert!((icon.top - (off.top + 4.0)).abs() < 0.01);
    let canvas = rule.draw();
    let color = |text: &str| canvas.texts.iter().position(|t| t == text).map(|i| canvas.colors[i]).expect(text);
    assert!((color("Kapalı").alpha() - 0.38).abs() < 0.01 && color("Kendi") == Color::Red);
    rule.on_node_with_tag("kapalı").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("0 tık").assert_exists();
    let at = rule.on_node_with_tag("kendi").get_unclipped_bounds_in_root().center();
    rule.host().pointer_down(at, -1, 0, kompoz::ui::input::PointerType::Touch);
    rule.advance_frame();
    rule.advance_frame();
    rule.on_node_with_text("basılı").assert_exists();
    rule.host().pointer_up(at, -1, 0);
    rule.wait_for_idle();
    rule.on_node_with_text("serbest").assert_exists();
}

fn wide_rail_widens() {
    let rule = KompozTestRule::with_viewport(WideRailWidens, IntSize::new(400, 600));
    let width = |rule: &KompozTestRule| rule.on_node_with_tag("ray").get_unclipped_bounds_in_root().width();
    assert!((width(&rule) - 96.0).abs() < 0.5);
    let gelen = rule.on_node_with_tag("Gelen").get_unclipped_bounds_in_root();
    assert!((gelen.top - 124.0).abs() < 0.5 && (gelen.height() - 64.0).abs() < 0.5, "{gelen:?}");
    rule.on_node_with_text("aç / kapat").perform_click();
    rule.wait_for_idle();
    assert!((width(&rule) - 220.0).abs() < 0.5);
    let giden = rule.on_node_with_tag("Giden").get_unclipped_bounds_in_root();
    assert!((giden.top - 180.0).abs() < 0.5 && (giden.height() - 56.0).abs() < 0.5, "{giden:?}");
    rule.on_node_with_tag("Giden").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_tag("Giden").assert_is_selected();
    rule.on_node_with_text("aç / kapat").perform_click();
    rule.wait_for_idle();
    assert!((width(&rule) - 96.0).abs() < 0.5);
}

fn modal_wide_rail_opens_over_the_content() {
    let rule = KompozTestRule::with_viewport(ModalWideRail, IntSize::new(400, 600));
    let items = |rule: &KompozTestRule| rule.on_all_nodes_with_tag("Gelen").fetch_semantics_nodes().len();
    let content_left = |rule: &KompozTestRule| rule.on_node_with_tag("içerik").get_unclipped_bounds_in_root().left;
    assert!(items(&rule) == 1 && (content_left(&rule) - 96.0).abs() < 0.5);
    rule.on_node_with_text("aç").perform_click();
    rule.wait_for_idle();
    // Over the content, the scrim drawn; the content doesn't move.
    assert!(rule.draw().rect_colors.iter().any(|c| (c.alpha() - 0.32).abs() < 0.01));
    assert!((content_left(&rule) - 96.0).abs() < 0.5);
    // Escape collapses it.
    assert!(rule.host().key_down(kompoz::ui::input::Key::Escape, kompoz::ui::input::KeyModifiers::default()));
    rule.wait_for_idle();
    assert!(!rule.draw().rect_colors.iter().any(|c| (c.alpha() - 0.32).abs() < 0.01));
    // Hidden on collapse: nothing in place.
    rule.on_node_with_text("gizle").perform_click();
    rule.wait_for_idle();
    assert!(items(&rule) == 0 && content_left(&rule).abs() < 0.5);
}

// (the rail samples' TooltipBox types its position provider with the window's trait)
use kompoz::ui::window::PopupPositionProvider;

fn three_navigations_animate_composing_nothing_around_them() {
    let composed = std::rc::Rc::new(std::cell::Cell::new(0));
    let c = composed.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let c = c.clone();
            ThreeNavigations(move || c.set(c.get() + 1));
        }),
        IntSize::new(400, 800),
    );
    let at = rule.on_node_with_text("Sekme 1").get_unclipped_bounds_in_root().center();
    rule.host().pointer_down(at, -1, 0, kompoz::ui::input::PointerType::Touch);
    rule.host().pointer_up(at, -1, 0);
    rule.advance_frame(); // the tab changes: composed again...
    rule.advance_frame();
    let before = composed.get();
    rule.wait_for_idle();
    assert_eq!(composed.get(), before); // ...and not as the items animate
    rule.on_node_with_text("Kısa 1").assert_is_selected();
    rule.on_node_with_text("Ray 1").assert_is_selected();
    rule.on_node_with_text("Sekme 0").assert_is_not_selected();
}

kompoz_file!("tests/kt/navigation_rail.kt");

/// The rail samples' items (Home, Search, Settings) and which is selected (by the selected item's icon: the filled
/// one, the others outlined — told by their content descriptions where the sample gives them).
fn rail_sample_selects(rule: &KompozTestRule, click: &str) {
    rule.on_node_with_text(click).perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text(click).assert_is_selected();
    rule.on_node_with_text("Home").assert_is_not_selected();
}

fn navigation_rail_samples() {
    // NavigationRailSample: 80 dp, its items from the top; selecting moves the selection.
    let rule = KompozTestRule::with_viewport(NavigationRailSample, IntSize::new(400, 600));
    rule.on_node_with_text("Home").assert_is_selected();
    let top = rule.on_node_with_text("Home").get_unclipped_bounds_in_root().top;
    assert!(top < 10.0, "{top}");
    rail_sample_selects(&rule, "Search");
    // NavigationRailBottomAlignSample: the spacer's weight pushes the items to the bottom.
    let rule = KompozTestRule::with_viewport(NavigationRailBottomAlignSample, IntSize::new(400, 600));
    let bottom = rule.on_node_with_text("Settings").get_unclipped_bounds_in_root().bottom;
    assert!((bottom - 596.0).abs() < 0.5, "{bottom}");
    rail_sample_selects(&rule, "Settings");
}

fn wide_navigation_rail_samples() {
    // Collapsed (96 dp) and expanded (220 dp) ones.
    let rule = KompozTestRule::with_viewport(WideNavigationRailCollapsedSample, IntSize::new(400, 600));
    rail_sample_selects(&rule, "Search");
    let collapsed = rule.on_node_with_text("Home").get_unclipped_bounds_in_root();
    assert!((collapsed.width() - 96.0).abs() < 0.5, "{collapsed:?}");
    let rule = KompozTestRule::with_viewport(WideNavigationRailExpandedSample, IntSize::new(400, 600));
    rail_sample_selects(&rule, "Settings");
    let expanded = rule.on_node_with_text("Home").get_unclipped_bounds_in_root();
    assert!(expanded.height() < 60.0, "{expanded:?}"); // a start icon's item: 56 dp
    // The responsive one: its header button expands and collapses it, telling its state.
    let rule = KompozTestRule::with_viewport(WideNavigationRailResponsiveSample, IntSize::new(800, 600));
    rule.on_node_with_text("The rail is Collapsed.").assert_exists();
    rule.on_node_with_content_description("Expand rail").assert_value_equals("Collapsed");
    rule.on_node_with_content_description("Expand rail").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("The rail is Expanded.").assert_exists();
    rule.on_node_with_text("Is animating: false").assert_exists();
    rule.on_node_with_content_description("Collapse rail").assert_value_equals("Expanded");
    rail_sample_selects(&rule, "Search");
}

fn modal_wide_navigation_rail_samples() {
    // ModalWideNavigationRailSample: collapsed in place, its header expanding it over the content.
    let rule = KompozTestRule::with_viewport(ModalWideNavigationRailSample, IntSize::new(800, 600));
    rule.on_node_with_text("The rail is Collapsed.").assert_exists();
    rule.on_node_with_content_description("Expand rail").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("The rail is Expanded.").assert_exists();
    assert!(rule.draw().rect_colors.iter().any(|c| (c.alpha() - 0.32).abs() < 0.01)); // the scrim
    rule.on_node_with_content_description("Search").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_content_description("Search").assert_is_selected();
    // (the collapsed rail in place keeps its header, under the dialog: the expanded one's is the last)
    rule.on_all_nodes_with(kompoz::test::has_content_description("Collapse rail", false, false), false)
        .on_last()
        .perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("The rail is Collapsed.").assert_exists();
    // DismissibleModalWideNavigationRailSample: hidden until its button opens it; an item closes it again.
    let rule = KompozTestRule::with_viewport(DismissibleModalWideNavigationRailSample, IntSize::new(800, 600));
    rule.on_node_with_text("Search").assert_does_not_exist();
    rule.on_node_with_text("Home Page\nOpen modal rail").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Search").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Search").assert_does_not_exist();
    rule.on_node_with_text("Search Page\nOpen modal rail").assert_exists();
}

// ==== navigation rails end ==========================================================================================

fn main() {
    common::run(&[
        // (navigation bars)
        ("navigation_bar_switches_tabs", navigation_bar_switches_tabs),
        ("navigation_bar_items_as_compose_1_4", navigation_bar_items_as_compose_1_4),
        (
            "short_navigation_bar_is_64_dp_over_the_navigation_bar",
            short_navigation_bar_is_64_dp_over_the_navigation_bar,
        ),
        ("short_navigation_bar_ripples_start_under_the_finger", short_navigation_bar_ripples_start_under_the_finger),
        (
            "short_navigation_bar_shares_the_width_or_centres_its_items",
            short_navigation_bar_shares_the_width_or_centres_its_items,
        ),
        (
            "layouts_that_measure_twice_probe_first_so_sizes_settle",
            layouts_that_measure_twice_probe_first_so_sizes_settle,
        ),
        ("navigation_bar_samples", navigation_bar_samples),
        ("navigation_bar_item_with_badge", navigation_bar_item_with_badge),
        // (navigation rails)
        ("rail_as_compose_1_4", rail_as_compose_1_4),
        ("window_size_and_rail", window_size_and_rail),
        ("rail_items_take_their_options", rail_items_take_their_options),
        ("wide_rail_widens", wide_rail_widens),
        ("modal_wide_rail_opens_over_the_content", modal_wide_rail_opens_over_the_content),
        (
            "three_navigations_animate_composing_nothing_around_them",
            three_navigations_animate_composing_nothing_around_them,
        ),
        ("navigation_rail_samples", navigation_rail_samples),
        ("wide_navigation_rail_samples", wide_navigation_rail_samples),
        ("modal_wide_navigation_rail_samples", modal_wide_navigation_rail_samples),
        // (navigation drawers)
        ("permanent_and_dismissible_drawers", permanent_and_dismissible_drawers),
        (
            "standard_drawer_sheets_pad_by_the_insets_left_to_them",
            standard_drawer_sheets_pad_by_the_insets_left_to_them,
        ),
        ("drawer_sheets_follow_the_back_gesture", drawer_sheets_follow_the_back_gesture),
        (
            "navigation_drawer_opens_closes_and_leaves_content_usable",
            navigation_drawer_opens_closes_and_leaves_content_usable,
        ),
        ("modal_drawer_closes_by_its_own_width", modal_drawer_closes_by_its_own_width),
        ("navigation_drawer_items_as_compose_1_4", navigation_drawer_items_as_compose_1_4),
        ("rtl_drawer_and_swipe_to_dismiss_follow_the_mirror", rtl_drawer_and_swipe_to_dismiss_follow_the_mirror),
        ("drawer_samples", drawer_samples),
    ]);
}

// ==== navigation drawers =============================================================================================
// The C++ Kompoz's drawer tests (kompoz_tests.cpp; no kompozable_tests.cpp source composes a drawer) written in Kotlin
// as a Compose user writes them, checked as the Rust tests of crates/kompoz-material3/tests/navigation_drawer.rs check
// them (shorter; M15's values where the C++ differed, said there), and material3's DrawerSamples from
// kt/navigation_drawer.kt.

kompoz! {
    @Kompozable
    fun PermanentAndDismissible() {
        Column {
            PermanentNavigationDrawer(
                drawerContent = { PermanentDrawerSheet(Modifier.width(250.dp)) { Text("Kalıcı menü") } },
                modifier = Modifier.height(200.dp),
            ) {
                Text("Kalıcı içerik", Modifier.testTag("permanent"))
            }
            val drawerState = rememberDrawerState(DrawerValue.Closed)
            val scope = rememberCoroutineScope()
            DismissibleNavigationDrawer(
                drawerContent = { DismissibleDrawerSheet(drawerState, Modifier.width(250.dp)) { Text("Açılır menü") } },
                modifier = Modifier.fillMaxSize(),
                drawerState = drawerState,
            ) {
                Column(Modifier.testTag("content")) {
                    Text("Çekmece: ${drawerState.currentValue}")
                    Button(onClick = { scope.launch { drawerState.open() } }) { Text("Aç") }
                }
            }
        }
    }

    @Kompozable
    fun StandardSheetInsets() {
        Row {
            PermanentDrawerSheet(Modifier.width(240.dp)) { Text("Kenarda", Modifier.testTag("edge")) }
            // A screen that already padded for the status bar.
            Box(Modifier.padding(top = 24.dp).consumeWindowInsets(PaddingValues(top = 24.dp))) {
                PermanentDrawerSheet(Modifier.width(240.dp)) { Text("İçeride", Modifier.testTag("inside")) }
            }
        }
    }

    @Kompozable
    fun BackGesture() {
        val drawerState = rememberDrawerState(DrawerValue.Open)
        ModalNavigationDrawer(
            drawerContent = { ModalDrawerSheet(drawerState) { Text("Gelen", Modifier.fillMaxWidth()) } },
            drawerState = drawerState,
        ) {
            Text("Çekmece: ${drawerState.currentValue}", Modifier.padding(top = 300.dp))
        }
    }

    @Kompozable
    fun OpensAndCloses() {
        val drawerState = remember { DrawerState(DrawerValue.Closed) }
        val scope = rememberCoroutineScope()
        var clicks by remember { mutableStateOf(0) }
        ModalNavigationDrawer(
            drawerContent = {
                ModalDrawerSheet(drawerState, Modifier.testTag("sheet")) {
                    NavigationDrawerItem(label = { Text("Gelen kutusu") }, selected = true, onClick = {})
                }
            },
            drawerState = drawerState,
        ) {
            Column(Modifier.padding(100.dp)) {
                Button(onClick = { clicks++ }) { Text("İçerik: $clicks") }
                Button(onClick = { scope.launch { drawerState.open() } }) { Text("Aç") }
                Text(if (drawerState.isOpen) "açık" else "kapalı")
            }
        }
    }

    @Kompozable
    fun ClosesByItsOwnWidth() {
        val drawerState = rememberDrawerState(DrawerValue.Open)
        ModalNavigationDrawer(
            drawerContent = { ModalDrawerSheet { Text("Gelen", Modifier.fillMaxWidth().testTag("gelen")) } },
            drawerState = drawerState,
        ) {
            Text("Çekmece: ${drawerState.currentValue}")
        }
    }

    @Kompozable
    fun DrawerItems() {
        var picked by remember { mutableStateOf(0) }
        Column(Modifier.width(300.dp)) {
            NavigationDrawerItem(
                label = { Text("Gelen") },
                selected = picked == 0,
                onClick = { picked = 0 },
                modifier = Modifier.testTag("gelen"),
            )
            NavigationDrawerItem(
                label = { Box(Modifier.size(10.dp, 70.dp)) },
                selected = picked == 1,
                onClick = { picked = 1 },
                modifier = Modifier.testTag("uzun"),
            )
            NavigationDrawerItem(
                label = { Text("Rozetli") },
                selected = picked == 2,
                onClick = { picked = 2 },
                modifier = Modifier.padding(NavigationDrawerItemDefaults.ItemPadding).testTag("rozetli"),
                icon = { Icon(Icons.Default.Email, contentDescription = null) },
                badge = { Text("24") },
                shape = RectangleShape,
                colors = NavigationDrawerItemDefaults.colors(selectedContainerColor = Color.Red, unselectedBadgeColor = Color.Gray),
            )
        }
    }

    @Kompozable
    fun RtlDrawer() {
        CompositionLocalProvider(LocalLayoutDirection provides LayoutDirection.Rtl) {
            val drawerState = remember { DrawerState(DrawerValue.Closed) }
            ModalNavigationDrawer(
                drawerContent = {
                    ModalDrawerSheet(Modifier.testTag("sheet"), drawerContainerColor = DrawerDefaults.standardContainerColor) {
                        Text("Gelen kutusu", Modifier.fillMaxWidth())
                    }
                },
                drawerState = drawerState,
                scrimColor = DrawerDefaults.scrimColor,
            ) {
                Text("Çekmece: ${drawerState.currentValue}", Modifier.padding(top = 300.dp))
            }
        }
    }
}

/// The C++ tests' `rule.swipe(from, to, frames)`: a move each frame, up at once (a fling).
fn drawer_swipe(rule: &KompozTestRule, from: (f32, f32), to: (f32, f32), frames: i32) {
    use kompoz::graphics::Offset;
    let (from, to) = (Offset::new(from.0, from.1), Offset::new(to.0, to.1));
    let h = rule.host();
    h.pointer_down(from, -1, 0, kompoz::ui::input::PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / frames as f32), -1, 0);
    }
    h.pointer_up(to, -1, 0);
    rule.advance_frame();
}

fn drawer_bounds(rule: &KompozTestRule, tag: &str) -> kompoz::graphics::Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn drawer_escape(rule: &KompozTestRule) -> bool {
    rule.host().key_down(kompoz::ui::input::Key::Escape, kompoz::ui::input::KeyModifiers::default())
}

fn permanent_and_dismissible_drawers() {
    let rule = KompozTestRule::with_viewport(PermanentAndDismissible, IntSize::new(600, 800));
    assert_eq!(drawer_bounds(&rule, "permanent").left, 250.0);
    assert_eq!(drawer_bounds(&rule, "content").left, 0.0);
    rule.on_node_with_text("Açılır menü").assert_is_not_displayed();
    rule.on_node_with_text("Aç").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Çekmece: Open").assert_exists();
    assert_eq!(drawer_bounds(&rule, "content").left, 250.0);
    drawer_swipe(&rule, (400.0, 500.0), (100.0, 500.0), 20); // dragged closed
    rule.wait_for_idle();
    rule.on_node_with_text("Çekmece: Closed").assert_exists();
    drawer_swipe(&rule, (50.0, 500.0), (400.0, 500.0), 20); // and open
    rule.wait_for_idle();
    rule.on_node_with_text("Çekmece: Open").assert_exists();
    assert!(drawer_escape(&rule)); // back closes it (the drawer state's sheet)
    rule.wait_for_idle();
    rule.on_node_with_text("Çekmece: Closed").assert_exists();
}

fn standard_drawer_sheets_pad_by_the_insets_left_to_them() {
    let rule = KompozTestRule::with_viewport(StandardSheetInsets, IntSize::new(600, 800));
    rule.host().set_system_insets(kompoz::ui::platform::SystemInsets {
        status_bars: kompoz::ui::platform::Insets::new(0, 24, 0, 0),
        ..Default::default()
    });
    rule.wait_for_idle();
    assert_eq!(drawer_bounds(&rule, "edge").top, 24.0);
    assert_eq!(drawer_bounds(&rule, "inside").top, 24.0);
}

fn drawer_sheets_follow_the_back_gesture() {
    use kompoz::ui::back::{BackEventCompat, SwipeEdge};
    let rule = KompozTestRule::with_viewport(BackGesture, IntSize::new(400, 800));
    rule.wait_for_idle();
    let event = |touch_x: f32, progress: f32| BackEventCompat {
        touch_x,
        touch_y: 400.0,
        progress,
        swipe_edge: SwipeEdge::Left,
    };
    // From the drawer's own edge: stretched across and lowered (scaled about the window's edge half way down).
    let host = rule.host();
    host.back_started(event(0.0, 0.0));
    host.back_progressed(event(40.0, 0.5));
    rule.advance_frame();
    let canvas = rule.draw();
    assert!(canvas.transforms.iter().any(|m| m.get(0, 0) > 1.0 && m.get(1, 1) < 1.0), "{:?}", canvas.transforms);
    host.back_cancelled(); // as it was, still open
    rule.wait_for_idle();
    assert!(rule.draw().transforms.iter().all(|m| m.get(0, 0) == 1.0 && m.get(1, 1) == 1.0));
    rule.on_node_with_text("Çekmece: Open").assert_exists();
    // Done: it closes.
    host.back_started(event(0.0, 0.0));
    host.back_progressed(event(40.0, 0.5));
    assert!(host.back_invoked());
    rule.wait_for_idle();
    rule.on_node_with_text("Çekmece: Closed").assert_exists();
    assert!(!host.wants_back());
}

fn navigation_drawer_opens_closes_and_leaves_content_usable() {
    let rule = KompozTestRule::with_viewport(OpensAndCloses, IntSize::new(400, 800));
    rule.on_node_with_text("İçerik: 0").perform_click(); // closed: the content works
    rule.on_node_with_text("İçerik: 1").assert_exists();
    rule.on_node_with_text("Gelen kutusu").assert_is_not_displayed();
    rule.on_node_with_text("Aç").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("açık").assert_exists();
    assert_eq!(drawer_bounds(&rule, "sheet").width(), 360.0);
    rule.host().pointer_down(kompoz::graphics::Offset::new(380.0, 400.0), -1, 0, kompoz::ui::input::PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(kompoz::graphics::Offset::new(380.0, 400.0), -1, 0); // the scrim closes it
    rule.wait_for_idle();
    rule.on_node_with_text("kapalı").assert_exists();
    rule.on_node_with_text("İçerik: 1").assert_exists();
    drawer_swipe(&rule, (20.0, 400.0), (300.0, 400.0), 12); // drag open from the content
    rule.wait_for_idle();
    rule.on_node_with_text("açık").assert_exists();
    assert!(drawer_escape(&rule)); // back closes it
    rule.wait_for_idle();
    rule.on_node_with_text("kapalı").assert_exists();
}

fn modal_drawer_closes_by_its_own_width() {
    let rule = KompozTestRule::with_viewport(ClosesByItsOwnWidth, IntSize::new(300, 600));
    // In a window narrower than the sheet's 360 dp: the window's width.
    assert_eq!(drawer_bounds(&rule, "gelen").width(), 300.0);
    rule.host().set_viewport_size(IntSize::new(250, 600));
    rule.wait_for_idle();
    assert_eq!(drawer_bounds(&rule, "gelen").width(), 250.0);
    drawer_swipe(&rule, (200.0, 300.0), (20.0, 300.0), 12); // shut by its own width
    rule.wait_for_idle();
    rule.on_node_with_text("Çekmece: Closed").assert_exists();
    rule.on_node_with_tag("gelen").assert_is_not_displayed();
}

fn navigation_drawer_items_as_compose_1_4() {
    let rule = KompozTestRule::new(DrawerItems);
    assert_eq!(drawer_bounds(&rule, "gelen").height(), 56.0);
    assert_eq!(drawer_bounds(&rule, "uzun").height(), 70.0);
    rule.on_node_with_tag("gelen").assert_is_selected();
    rule.on_node_with_tag("uzun").perform_click();
    rule.on_node_with_tag("uzun").assert_is_selected();
    rule.on_node_with_tag("gelen").assert_is_not_selected();
    // The item padded 12 dp in, its badge in the color given while unselected, its container red once selected.
    let item = drawer_bounds(&rule, "rozetli");
    assert!(item.left == 12.0 && item.width() == 300.0 - 24.0, "{item:?}");
    assert!(rule.draw().colors.contains(&Color::Gray));
    rule.on_node_with_tag("rozetli").perform_click();
    assert!(rule.draw().rect_colors.contains(&Color::Red));
}

fn rtl_drawer_and_swipe_to_dismiss_follow_the_mirror() {
    // (the drawer half; the swipe to dismiss half is WP5's, crates/kompoz-material3/tests/swipe_pull.rs's test of
    // the same name)
    let rule = KompozTestRule::with_viewport(RtlDrawer, IntSize::new(400, 800));
    drawer_swipe(&rule, (380.0, 100.0), (100.0, 100.0), 12); // the drawer is at the right: dragged open to the left
    rule.wait_for_idle();
    rule.on_node_with_text("Çekmece: Open").assert_exists();
    let sheet = drawer_bounds(&rule, "sheet");
    assert!(sheet.right == 400.0 && sheet.left == 40.0, "{sheet:?}");
    drawer_swipe(&rule, (100.0, 100.0), (380.0, 100.0), 12); // and shut to the right
    rule.wait_for_idle();
    rule.on_node_with_text("Çekmece: Closed").assert_exists();
}

// (the type the samples construct — Compose's androidx.compose.ui.focus.FocusRequester import)
use kompoz::ui::focus::FocusRequester;

kompoz_file!("tests/kt/navigation_drawer.kt");

fn drawer_samples() {
    // ModalNavigationDrawerSample: opened by its button, an item clicked selects it and closes the drawer.
    let rule = KompozTestRule::with_viewport(ModalNavigationDrawerSample, IntSize::new(400, 800));
    rule.on_node_with_text(">>> Swipe >>>").assert_exists();
    rule.on_node_with_text("Click to open").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("<<< Swipe <<<").assert_exists();
    rule.on_node_with_text("AccountCircle").assert_is_selected();
    rule.on_node_with_text("Email").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text(">>> Swipe >>>").assert_exists();
    rule.on_node_with_text("Email").assert_is_not_displayed();
    rule.on_node_with_text("Email").assert_is_selected();
    // PermanentNavigationDrawerSample: the content beside the sheet, an item clicked selected.
    let rule = KompozTestRule::with_viewport(PermanentNavigationDrawerSample, IntSize::new(800, 600));
    let content = rule.on_node_with_text("Application content").get_unclipped_bounds_in_root();
    // (centered in whole pixels: within one)
    assert!((content.center().x - (240.0 + 560.0 / 2.0)).abs() <= 1.0, "{content:?}");
    rule.on_node_with_text("Dashboard").perform_click();
    rule.on_node_with_text("Dashboard").assert_is_selected();
    rule.on_node_with_text("AccountCircle").assert_is_not_selected();
    // DismissibleNavigationDrawerSample: the content pushed aside by the sheet (360 dp), back when an item closes it.
    let rule = KompozTestRule::with_viewport(DismissibleNavigationDrawerSample, IntSize::new(800, 600));
    let button = rule.on_node_with_text("Click to open").get_unclipped_bounds_in_root();
    rule.on_node_with_text("Click to open").perform_click();
    rule.wait_for_idle();
    let pushed = rule.on_node_with_text("Click to open").get_unclipped_bounds_in_root();
    assert!((pushed.left - button.left - 360.0).abs() < 0.5, "{button:?} {pushed:?}");
    rule.on_node_with_text("Favorite").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text(">>> Swipe >>>").assert_exists();
    rule.on_node_with_text("Favorite").assert_is_selected();
}
