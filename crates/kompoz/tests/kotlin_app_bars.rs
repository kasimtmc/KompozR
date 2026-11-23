//! Material's app bars in Kotlin's syntax: material3 1.5's AppBarSamples from `.kt` files (kompoz_file!) — the top app
//! bars and their scroll behaviors, the bottom app bars, AppBarRow and AppBarColumn — and the C++ Kompoz's app bar tests
//! written as a Compose user writes them, each composed and used as a user does. Density 1: dp are px.

#![allow(non_snake_case, non_upper_case_globals, unused_variables)]

mod common;

#[allow(unused_imports)]
use kompoz::prelude::*;
#[allow(unused_imports)]
use kompoz::test::KompozTestRule;
// (top app bars: the drags' pointers, the samples' imports of what the generated code names — TooltipBox's position
// provider, nestedScroll's connection —, a counter of compositions)
use kompoz::ui::input::PointerType;
use std::cell::Cell;
use std::rc::Rc;

// ==== top app bars ===============================================================================================
// The C++ Kompoz's top app bar tests (kompoz_tests.cpp; kompozable_tests.cpp has no source for them) written as a
// Compose user writes them, checked as crates/kompoz-material3/tests/top_app_bars.rs checks them (M15's values; the
// differences from the C++'s said there), and M15's top-bar AppBarSamples (kt/top_app_bars.kt).

fn top_near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

/// A text's bounds (the merged tree's: a two-rows bar's row hidden from accessibility isn't found).
fn top_text(rule: &KompozTestRule, text: &str) -> Rect {
    rule.on_node_with_text(text).get_unclipped_bounds_in_root()
}

fn top_tag(rule: &KompozTestRule, tag: &str) -> Rect {
    named_args!(rule.on_node_with_tag(tag, use_unmerged_tree = true)).get_unclipped_bounds_in_root()
}

/// The C++ tests' `slowDrag`: down, moved over `frames` frames, held 200 ms, up.
fn top_slow_drag(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / frames as f32), -1, 0);
    }
    rule.advance_time_by(200);
    h.pointer_up(to, -1, 0);
    rule.advance_frame();
}

/// A fling: a move each frame, let go at once.
fn top_swipe(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / frames as f32), -1, 0);
    }
    h.pointer_up(to, -1, 0);
    rule.advance_frame();
}

/// The bar drawn: its container's rect (at the top, as wide as the window) and color.
fn top_bar(rule: &KompozTestRule) -> (Rect, Color) {
    let c = rule.draw();
    c.rects
        .iter()
        .zip(&c.rect_colors)
        .find(|(r, _)| {
            top_near(r.left, 0.0) && top_near(r.top, 0.0) && top_near(r.width(), 400.0) && r.height() < 400.0
        })
        .map(|(r, c)| (*r, *c))
        .unwrap_or((Rect::Zero, Color::Unspecified))
}

/// The color `text` was drawn in.
fn top_color_of(rule: &KompozTestRule, text: &str) -> Option<Color> {
    let c = rule.draw();
    c.texts.iter().position(|t| t == text).map(|i| c.colors[i])
}

kompoz! {
    // (the C++ tests' list: 100 rows 50 dp tall under the scaffold's padding)
    @Composable
    fun Rows(padding: PaddingValues) {
        LazyColumn(Modifier.fillMaxSize(), contentPadding = padding) {
            items(100) { i -> Text("Satır $i", Modifier.height(50.dp).testTag("row$i")) }
        }
    }

    @Composable
    fun EnterAlwaysBar() {
        val scrollBehavior = TopAppBarDefaults.enterAlwaysScrollBehavior()
        Scaffold(
            modifier = Modifier.fillMaxSize().nestedScroll(scrollBehavior.nestedScrollConnection),
            topBar = { TopAppBar(title = { Text("Başlık") }, scrollBehavior = scrollBehavior) },
        ) { padding ->
            Rows(padding)
        }
    }

    // (what lays out by the scaffold's padding: the list's content padding, Modifier.padding, consumed insets; told
    // each time the content composes)
    @Composable
    fun CollapsingBar(onCompose: () -> Unit) {
        val scrollBehavior = TopAppBarDefaults.enterAlwaysScrollBehavior()
        Scaffold(
            modifier = Modifier.fillMaxSize().nestedScroll(scrollBehavior.nestedScrollConnection),
            topBar = { TopAppBar(title = { Text("Başlık") }, scrollBehavior = scrollBehavior) },
        ) { padding ->
            onCompose()
            Box(Modifier.fillMaxSize()) {
                Rows(padding)
                Box(
                    Modifier.fillMaxWidth()
                        .padding(padding)
                        .consumeWindowInsets(padding)
                        .windowInsetsTopHeight(WindowInsets(top = 120.dp))
                        .testTag("kenar")
                )
            }
        }
    }

    @Composable
    fun LargeBar() {
        val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
        Scaffold(
            modifier = Modifier.fillMaxSize().nestedScroll(scrollBehavior.nestedScrollConnection),
            topBar = { LargeTopAppBar(title = { Text("Büyük başlık") }, scrollBehavior = scrollBehavior) },
        ) { padding ->
            Rows(padding)
        }
    }

    @Composable
    fun OwnTitlesBar() {
        val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
        // the style around the bar: 10 dp a letter
        CompositionLocalProvider(LocalTextStyle provides TextStyle(fontSize = 20.sp)) {
            Scaffold(
                modifier = Modifier.fillMaxSize().nestedScroll(scrollBehavior.nestedScrollConnection),
                topBar = {
                    TwoRowsTopAppBar(
                        title = { expanded ->
                            Text(if (expanded) "Büyük başlık" else "Küçük", Modifier.testTag(if (expanded) "büyük" else "küçük"))
                        },
                        modifier = Modifier.testTag("çubuk"),
                        subtitle = { expanded ->
                            Text(if (expanded) "uzun alt başlık" else "alt", Modifier.testTag(if (expanded) "alt büyük" else "alt küçük"))
                        },
                        scrollBehavior = scrollBehavior,
                    )
                },
            ) { padding ->
                Rows(padding)
            }
        }
    }

    @Composable
    fun PinnedBar() {
        val scrollBehavior = TopAppBarDefaults.pinnedScrollBehavior()
        Scaffold(
            modifier = Modifier.fillMaxSize().nestedScroll(scrollBehavior.nestedScrollConnection),
            topBar = { TopAppBar(title = { Text("Başlık") }, scrollBehavior = scrollBehavior) },
        ) { padding ->
            Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(padding)) {
                Box(Modifier.height(2000.dp))
            }
        }
    }

    @Composable
    fun FlexibleBars() {
        Column {
            MediumFlexibleTopAppBar(title = { Text("Notlar") }, windowInsets = WindowInsets(), modifier = Modifier.testTag("orta-çubuk"))
            MediumFlexibleTopAppBar(
                title = { Text("Notlar") },
                subtitle = { Text("12 not") },
                windowInsets = WindowInsets(),
                modifier = Modifier.testTag("alt-yazılı"),
            )
            LargeFlexibleTopAppBar(
                title = { Text("Gelen", Modifier.testTag("gelen")) },
                windowInsets = WindowInsets(),
                titleHorizontalAlignment = Alignment.CenterHorizontally,
                modifier = Modifier.testTag("büyük-çubuk"),
            )
            LargeFlexibleTopAppBar(
                title = { Text("Giden", Modifier.testTag("giden")) },
                subtitle = { Text("2 ileti") },
                windowInsets = WindowInsets(),
                titleHorizontalAlignment = Alignment.End,
                modifier = Modifier.testTag("son-çubuk"),
            )
        }
    }

    @Composable
    fun SubtitledBars() {
        Column {
            TopAppBar(title = { Text("Gelen kutusu", Modifier.testTag("başlık")) }, subtitle = { Text("3 okunmamış", Modifier.testTag("alt")) })
            TopAppBar(
                title = { Text("Takvim", Modifier.testTag("orta-başlık")) },
                subtitle = { Text("Ekim", Modifier.testTag("orta-alt")) },
                titleHorizontalAlignment = Alignment.CenterHorizontally,
            )
            MediumFlexibleTopAppBar(title = { Text("Notlar") }, subtitle = { Text("12 not") }, modifier = Modifier.testTag("orta-çubuk"))
        }
    }

    @Composable
    fun OwnColorsBars() {
        val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
        Column {
            TopAppBar(
                title = { Text("Başlık") },
                subtitle = { Text("Alt") },
                modifier = Modifier.testTag("tek"),
                navigationIcon = { Icon(Icons.Filled.Menu, contentDescription = "Menü") },
                actions = { Icon(Icons.Filled.Search, contentDescription = "Ara") },
                titleHorizontalAlignment = Alignment.CenterHorizontally,
                expandedHeight = 80.dp,
                windowInsets = WindowInsets(top = 10.dp),
                colors = TopAppBarDefaults.topAppBarColors(
                    navigationIconContentColor = Color.Red,
                    titleContentColor = Color.Green,
                    actionIconContentColor = Color.Blue,
                    subtitleContentColor = Color.Magenta,
                ),
            )
            MediumTopAppBar(
                title = { Text("Orta") },
                modifier = Modifier.testTag("orta"),
                collapsedHeight = 56.dp,
                expandedHeight = 150.dp,
                scrollBehavior = scrollBehavior,
            )
        }
    }

    @Composable
    fun RightToLeftBar() {
        CompositionLocalProvider(LocalLayoutDirection provides LayoutDirection.Rtl) {
            CenterAlignedTopAppBar(
                title = { Text("Başlık") },
                navigationIcon = { Icon(Icons.Filled.Menu, contentDescription = "Menü") },
                actions = { Icon(Icons.Filled.Search, contentDescription = "Ara") },
                windowInsets = WindowInsets(),
            )
        }
    }

    // (Kotlin's RowScope of a top app bar's actions: `RowScope.AnimatedVisibility` there)
    @Composable
    fun ActionsInARow() {
        var shown by remember { mutableStateOf(true) }
        Column {
            Button(onClick = { shown = !shown }, modifier = Modifier.testTag("gizle")) { Text("Gizle") }
            TopAppBar(title = { Text("Başlık") }, windowInsets = WindowInsets(), actions = {
                AnimatedVisibility(shown, Modifier.testTag("kap")) { Box(Modifier.size(48.dp).testTag("eylem")) }
                Box(Modifier.size(48.dp).testTag("son"))
            })
            MediumTopAppBar(title = { Text("Orta") }, windowInsets = WindowInsets(), actions = {
                AnimatedVisibility(shown) { Box(Modifier.size(48.dp).testTag("orta-eylem")) }
            })
        }
    }
}

/// The C++ test: the bar collapses before the list scrolls up, comes back before it scrolls down.
fn top_app_bar_enter_always_hides_before_the_list_scrolls() {
    let rule = KompozTestRule::new(EnterAlwaysBar);
    // (the clock by hand: the checks after a gesture see the bar before it settles)
    rule.main_clock().set_auto_advance(false);
    assert!(top_near(top_tag(&rule, "row0").top, 64.0));
    // up 40 less Compose's 8 dp touch slop: the bar 32 up, the content with it, the list where it was
    top_slow_drag(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 360.0), 20);
    assert!(top_near(top_tag(&rule, "row0").top, 64.0 - 32.0));
    assert!((top_text(&rule, "Başlık").center().y - 16.0).abs() <= 1.0);
    top_slow_drag(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 200.0), 20);
    common::settle(&rule);
    rule.on_node_with_tag("row0").assert_does_not_exist();
    let row = top_tag(&rule, "row5");
    // down: the bar comes back first, the list not scrolling (its rows only 32 lower with the scaffold's padding)
    top_slow_drag(&rule, Offset::new(200.0, 300.0), Offset::new(200.0, 340.0), 20);
    assert!(top_near(top_tag(&rule, "row5").top, row.top + 32.0), "{:?} {row:?}", top_tag(&rule, "row5"));
    assert!((top_text(&rule, "Başlık").center().y - 16.0).abs() <= 1.0);
}

/// The C++ test: what lays out by the scaffold's padding follows the bar, the content not composed again.
fn a_collapsing_top_bar_moves_the_content_without_recomposing_it() {
    let composed = Rc::new(Cell::new(0));
    let rule = {
        let composed = composed.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let composed = composed.clone();
            CollapsingBar(move || composed.set(composed.get() + 1));
        }))
    };
    // (the clock by hand: the checks after a gesture see the bar before it settles)
    rule.main_clock().set_auto_advance(false);
    let edge = || {
        let inner = rule.on_node_with_tag("kenar").layout_node().inner_coordinates();
        inner.find_root_coordinates().local_bounding_box_of(&inner, false)
    };
    let before = composed.get();
    assert!(top_near(edge().top, 64.0) && top_near(edge().height(), 120.0 - 64.0));
    top_slow_drag(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 360.0), 20);
    assert!(top_near(top_tag(&rule, "row0").top, 32.0));
    assert!(top_near(edge().top, 32.0) && top_near(edge().height(), 120.0 - 32.0));
    assert_eq!(composed.get(), before);
}

/// The C++ test: a large bar collapses before the list scrolls up, and expands once the list is back at its top.
fn large_top_app_bar_exit_until_collapsed() {
    let rule = KompozTestRule::new(LargeBar);
    // (the clock by hand: the checks after a gesture see the bar before it settles)
    rule.main_clock().set_auto_advance(false);
    assert!(top_near(top_tag(&rule, "row0").top, 152.0));
    top_slow_drag(&rule, Offset::new(200.0, 500.0), Offset::new(200.0, 440.0), 20);
    assert!(top_near(top_tag(&rule, "row0").top, 152.0 - 52.0)); // the bar 52 up, the list where it was
    common::settle(&rule);
    top_slow_drag(&rule, Offset::new(200.0, 500.0), Offset::new(200.0, 200.0), 20);
    common::settle(&rule);
    assert!(top_near(top_bar(&rule).0.height(), 64.0));
    rule.on_node_with_tag("row0").assert_does_not_exist();
    // Down: the list first, then the bar.
    top_slow_drag(&rule, Offset::new(200.0, 300.0), Offset::new(200.0, 400.0), 20);
    common::settle(&rule);
    assert!(top_near(top_bar(&rule).0.height(), 64.0));
    top_slow_drag(&rule, Offset::new(200.0, 200.0), Offset::new(200.0, 700.0), 20);
    common::settle(&rule);
    assert!(top_near(top_tag(&rule, "row0").top, 152.0) && top_near(top_bar(&rule).0.height(), 152.0));
}

/// The C++ test: each row asks for its own title and subtitle, in the bar's styles merged into the one around.
fn two_rows_top_app_bars_let_their_titles_style_themselves() {
    let rule = KompozTestRule::new(OwnTitlesBar);
    assert!(top_near(top_tag(&rule, "çubuk").height(), 136.0));
    assert!(top_tag(&rule, "büyük").top > 63.0 && top_tag(&rule, "küçük").top < 40.0);
    assert!(top_tag(&rule, "alt büyük").top > top_tag(&rule, "büyük").top);
    // headline medium 36 dp a line, title large 28, label large 20, label medium 16
    assert!(top_near(top_tag(&rule, "büyük").height(), 36.0) && top_near(top_tag(&rule, "küçük").height(), 28.0));
    assert!(
        top_near(top_tag(&rule, "alt büyük").height(), 20.0) && top_near(top_tag(&rule, "alt küçük").height(), 16.0)
    );
    top_slow_drag(&rule, Offset::new(200.0, 500.0), Offset::new(200.0, 200.0), 20);
    assert!(top_near(top_tag(&rule, "çubuk").height(), 64.0));
}

/// The C++ test: a pinned bar stays as the content scrolls under it, and knows once it is back at its top (its
/// color says).
fn pinned_top_app_bar_knows_when_content_is_under_it() {
    let rule = KompozTestRule::new(PinnedBar);
    let cs = light_color_scheme();
    top_slow_drag(&rule, Offset::new(200.0, 500.0), Offset::new(200.0, 300.0), 20);
    rule.wait_for_idle();
    assert_eq!(top_bar(&rule), (Rect::new(0.0, 0.0, 400.0, 64.0), cs.surface_container));
    top_slow_drag(&rule, Offset::new(200.0, 200.0), Offset::new(200.0, 700.0), 20);
    rule.wait_for_idle();
    assert_eq!(top_bar(&rule).1, cs.surface);
}

/// The C++ test: the pinned bar's container turns to its scrolled color as content comes under it.
fn a_top_app_bar_takes_its_scrolled_color() {
    let rule = KompozTestRule::new(PinnedBar);
    let cs = light_color_scheme();
    assert_eq!(top_bar(&rule).1, cs.surface);
    top_slow_drag(&rule, Offset::new(200.0, 500.0), Offset::new(200.0, 300.0), 20);
    rule.wait_for_idle();
    assert_eq!(top_bar(&rule).1, cs.surface_container);
}

/// The C++ test: the flexible bars are taller (more with a subtitle), their titles placed across.
fn flexible_top_app_bars_are_bigger_and_place_their_title_across() {
    let rule = KompozTestRule::with_viewport(FlexibleBars, IntSize::new(400, 1000));
    assert!(
        top_near(top_tag(&rule, "orta-çubuk").height(), 112.0)
            && top_near(top_tag(&rule, "alt-yazılı").height(), 136.0)
    );
    assert!(
        top_near(top_tag(&rule, "büyük-çubuk").height(), 120.0)
            && top_near(top_tag(&rule, "son-çubuk").height(), 152.0)
    );
    // the large titles (display small, 36 sp: 18 dp a letter) centered and at the end, 8 dp from it
    let (gelen, giden) = (top_text(&rule, "Gelen"), top_text(&rule, "Giden"));
    assert!(top_near(gelen.center().x, 200.0) && top_near(gelen.width(), 90.0), "{gelen:?}");
    assert!(top_near(giden.right, 400.0 - 8.0), "{giden:?}");
}

/// The C++ test: the subtitle sits under the title; centered, both are in the middle; a medium bar with one is
/// taller.
fn top_app_bars_show_a_subtitle_under_the_title() {
    let rule = KompozTestRule::new(SubtitledBars);
    let (title, sub) = (top_tag(&rule, "başlık"), top_tag(&rule, "alt"));
    assert!(top_near(sub.top, title.bottom) && top_near(sub.left, 16.0) && top_near(title.left, 16.0));
    assert_eq!(top_color_of(&rule, "3 okunmamış"), Some(light_color_scheme().on_surface_variant));
    assert!(
        top_near(top_tag(&rule, "orta-başlık").center().x, 200.0)
            && top_near(top_tag(&rule, "orta-alt").center().x, 200.0)
    );
    assert!(top_near(top_tag(&rule, "orta-çubuk").height(), 112.0 + 24.0));
}

/// The C++ test: each part's color, the heights and the insets are the bar's arguments; the medium bar collapses
/// from its expanded height to its collapsed one.
fn top_app_bar_colors_heights_and_insets_can_be_set() {
    let rule = KompozTestRule::new(OwnColorsBars);
    assert_eq!(top_color_of(&rule, "Başlık"), Some(Color::Green));
    assert_eq!(top_color_of(&rule, "Alt"), Some(Color::Magenta));
    assert!(top_near(top_tag(&rule, "tek").height(), 80.0 + 10.0) && top_near(top_tag(&rule, "orta").height(), 150.0));
    top_slow_drag(&rule, Offset::new(200.0, 150.0), Offset::new(200.0, 30.0), 20);
    rule.wait_for_idle();
    assert!(top_near(top_tag(&rule, "orta").height(), 56.0), "{:?}", top_tag(&rule, "orta"));
}

/// The C++ test: right to left, the navigation icon at the start (the right), the actions at the end (the left).
fn center_aligned_top_app_bar_mirrors_right_to_left() {
    let rule = KompozTestRule::new(RightToLeftBar);
    let menu = rule.on_node_with_content_description("Menü").get_unclipped_bounds_in_root();
    let search = rule.on_node_with_content_description("Ara").get_unclipped_bounds_in_root();
    assert!(top_near(menu.right, 400.0 - 4.0) && top_near(search.left, 4.0), "{menu:?} {search:?}");
    assert!((top_text(&rule, "Başlık").center().x - 200.0).abs() <= 1.0);
}

/// A bar's actions in Kotlin's RowScope: `AnimatedVisibility` there is the row's (it shrinks horizontally).
fn top_app_bar_actions_are_in_a_row_scope() {
    let rule = KompozTestRule::new(ActionsInARow);
    let last = top_tag(&rule, "son");
    assert!(top_near(last.right, 400.0 - 4.0) && top_near(top_tag(&rule, "eylem").right, last.left));
    rule.main_clock().set_auto_advance(false);
    rule.on_node_with_tag("gizle").perform_click();
    // half way: narrower as the row's AnimatedVisibility makes it, as tall
    rule.advance_time_by(150);
    let leaving = top_tag(&rule, "kap");
    assert!(top_near(leaving.height(), 48.0) && leaving.width() < 47.0, "{leaving:?}");
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
    named_args!(rule.on_node_with_tag("eylem", use_unmerged_tree = true)).assert_does_not_exist();
    named_args!(rule.on_node_with_tag("orta-eylem", use_unmerged_tree = true)).assert_does_not_exist();
    assert!(top_near(top_tag(&rule, "son").right, 400.0 - 4.0));
}

kompoz_file!("tests/kt/top_app_bars.kt");

/// A small bar's sample: 64 dp in the surface's color, the navigation icon at the start, `actions` at the end, the
/// title after the icon (or centered).
fn top_small_sample(rule: &KompozTestRule, title: &str, centered: bool, actions: &[&str]) {
    let (rect, color) = top_bar(rule);
    assert!(top_near(rect.height(), 64.0) && color == light_color_scheme().surface, "{rect:?}");
    let button = |d: &str| rule.on_node_with_content_description(d).get_unclipped_bounds_in_root();
    assert!(button("Menu").center().x < 52.0);
    if let Some(last) = actions.last() {
        assert!((button(last).center().x - (400.0 - 4.0 - 24.0)).abs() <= 1.0);
    }
    for pair in actions.windows(2) {
        assert!(button(pair[0]).right <= button(pair[1]).left + 0.5);
    }
    let t = top_text(rule, title);
    if centered {
        assert!((t.center().x - 200.0).abs() <= 1.0, "{t:?}");
    } else {
        assert!(t.left >= button("Menu").right - 0.5 && t.left < 80.0, "{t:?}");
    }
}

/// Scrolls a sample's content up by `by` and lets it settle.
fn top_scroll_up(rule: &KompozTestRule, by: f32) {
    top_slow_drag(rule, Offset::new(200.0, 600.0), Offset::new(200.0, 600.0 - by), 20);
    rule.wait_for_idle();
}

fn simple_top_app_bar() {
    let rule = KompozTestRule::new(SimpleTopAppBar);
    top_small_sample(&rule, "Simple TopAppBar", false, &["Add to favorites"]);
    assert!(top_near(top_text(&rule, "0").top, 64.0));
    top_scroll_up(&rule, 300.0);
    assert_eq!(top_bar(&rule), (Rect::new(0.0, 0.0, 400.0, 64.0), light_color_scheme().surface));
}

/// Whether a node of `description` is placed (AppBarRow composes every item and its overflow indicator, placing
/// only those that show).
fn kt_placed_with_description(rule: &KompozTestRule, description: &str) -> bool {
    rule.on_all_nodes_with_content_description(description).fetch_semantics_nodes().iter().any(|n| n.is_placed())
}

/// As crates/kompoz-material3/tests/top_app_bars.rs checks it: compact, Attachment, Edit and the overflow indicator
/// (M15's AppBarRow keeps the last of `maxItemCount` for it), Star, Snooze and Mark unread in its menu; medium, all
/// five.
fn simple_top_app_bar_with_adaptive_actions() {
    let rule = KompozTestRule::new(SimpleTopAppBarWithAdaptiveActions);
    rule.on_node_with_text("Simple TopAppBar").assert_exists();
    for item in ["Attachment", "Edit", "Overflow"] {
        assert!(kt_placed_with_description(&rule, item), "{item}");
    }
    for item in ["Star", "Snooze", "Mark unread"] {
        assert!(!kt_placed_with_description(&rule, item), "{item}");
    }
    rule.on_node_with_content_description("Overflow").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Snooze").assert_exists();
    rule.on_node_with_text("Mark unread").assert_exists();
    let rule = KompozTestRule::with_viewport(SimpleTopAppBarWithAdaptiveActions, IntSize::new(700, 800));
    for item in ["Attachment", "Edit", "Star", "Snooze", "Mark unread"] {
        assert!(kt_placed_with_description(&rule, item), "{item}");
    }
    assert!(!kt_placed_with_description(&rule, "Overflow"));
}

fn simple_top_app_bar_with_subtitle() {
    let rule = KompozTestRule::new(SimpleTopAppBarWithSubtitle);
    top_small_sample(&rule, "Simple TopAppBar", false, &["Add to favorites"]);
    assert!(top_near(top_text(&rule, "Subtitle").top, top_text(&rule, "Simple TopAppBar").bottom));
    top_scroll_up(&rule, 300.0);
    assert_eq!(top_bar(&rule), (Rect::new(0.0, 0.0, 400.0, 64.0), light_color_scheme().surface_container));
}

fn simple_center_aligned_top_app_bar() {
    let rule = KompozTestRule::new(SimpleCenterAlignedTopAppBar);
    top_small_sample(&rule, "Centered TopAppBar", true, &["Add to favorites"]);
}

fn simple_center_aligned_top_app_bar_with_subtitle() {
    let rule = KompozTestRule::new(SimpleCenterAlignedTopAppBarWithSubtitle);
    top_small_sample(&rule, "Simple TopAppBar", true, &["Add to favorites"]);
    assert!((top_text(&rule, "Subtitle").center().x - 200.0).abs() <= 1.0);
    top_scroll_up(&rule, 300.0);
    assert_eq!(top_bar(&rule).1, light_color_scheme().surface_container);
}

fn pinned_top_app_bar() {
    let rule = KompozTestRule::new(PinnedTopAppBar);
    top_small_sample(&rule, "TopAppBar", false, &["Add to favorites", "Star"]);
    top_scroll_up(&rule, 300.0);
    assert_eq!(top_bar(&rule), (Rect::new(0.0, 0.0, 400.0, 64.0), light_color_scheme().surface_container));
}

fn pinned_top_app_bar_with_pre_scrolled_lazy_column() {
    let rule = KompozTestRule::new(PinnedTopAppBarWithPreScrolledLazyColumn);
    rule.on_node_with_text("30").assert_exists();
    assert_eq!(top_bar(&rule).1, light_color_scheme().surface_container);
}

fn pinned_top_app_bar_with_reversed_lazy_grid() {
    let rule = KompozTestRule::new(PinnedTopAppBarWithReversedLazyGrid);
    assert!(top_near(top_text(&rule, "0").top, 800.0 - 50.0));
    assert_eq!(top_bar(&rule).1, light_color_scheme().surface_container);
    top_swipe(&rule, Offset::new(200.0, 200.0), Offset::new(200.0, 700.0), 5);
    rule.wait_for_idle();
    assert!(top_near(top_text(&rule, "75").top, 64.0));
    assert_eq!(top_bar(&rule).1, light_color_scheme().surface);
}

fn enter_always_top_app_bar() {
    let rule = KompozTestRule::new(EnterAlwaysTopAppBar);
    top_small_sample(&rule, "TopAppBar", false, &["Add to favorites"]);
    top_scroll_up(&rule, 300.0);
    assert!(top_near(top_bar(&rule).0.height(), 0.0));
    top_slow_drag(&rule, Offset::new(200.0, 300.0), Offset::new(200.0, 340.0), 20);
    assert!(top_near(top_bar(&rule).0.height(), 32.0), "{:?}", top_bar(&rule));
}

fn enter_always_top_app_bar_with_reverse_scrolling() {
    let rule = KompozTestRule::new(EnterAlwaysTopAppBarWithReverseScrolling);
    assert!(top_text(&rule, "75").bottom > 700.0 && top_text(&rule, "0").bottom < 0.0);
    assert_eq!(top_bar(&rule).1, light_color_scheme().surface_container);
    top_scroll_up(&rule, 300.0);
    assert!(top_near(top_bar(&rule).0.height(), 0.0));
}

/// A two-rows sample: `expanded` tall, its large title (`centered`), collapsing to 64 dp in its scrolled color.
fn top_two_rows_sample(rule: &KompozTestRule, title: &str, expanded: f32, centered: bool) {
    assert!(top_near(top_bar(rule).0.height(), expanded), "{:?}", top_bar(rule));
    let t = top_text(rule, title);
    assert!(t.top >= 63.5 && (!centered || (t.center().x - 200.0).abs() <= 1.0), "{t:?}");
    top_scroll_up(rule, 400.0);
    assert_eq!(top_bar(rule), (Rect::new(0.0, 0.0, 400.0, 64.0), light_color_scheme().surface_container));
    assert!(top_text(rule, title).bottom <= 64.5);
}

fn exit_until_collapsed_medium_top_app_bar() {
    top_two_rows_sample(&KompozTestRule::new(ExitUntilCollapsedMediumTopAppBar), "Medium TopAppBar", 112.0, false);
}

fn exit_until_collapsed_center_aligned_medium_flexible_top_app_bar() {
    let rule = KompozTestRule::new(ExitUntilCollapsedCenterAlignedMediumFlexibleTopAppBar);
    assert!((top_text(&rule, "Subtitle").center().x - 200.0).abs() <= 1.0);
    top_two_rows_sample(&rule, "Medium TopAppBar", 136.0, true);
}

fn exit_until_collapsed_large_top_app_bar() {
    top_two_rows_sample(&KompozTestRule::new(ExitUntilCollapsedLargeTopAppBar), "Large TopAppBar", 152.0, false);
}

fn exit_until_collapsed_center_aligned_large_flexible_top_app_bar() {
    let rule = KompozTestRule::new(ExitUntilCollapsedCenterAlignedLargeFlexibleTopAppBar);
    assert!((top_text(&rule, "Subtitle").center().x - 200.0).abs() <= 1.0);
    top_two_rows_sample(&rule, "Large TopAppBar", 152.0, true);
}

fn custom_two_rows_top_app_bar() {
    let rule = KompozTestRule::new(CustomTwoRowsTopAppBar);
    assert!(top_near(top_bar(&rule).0.height(), 156.0));
    rule.on_node_with_text("Expanded Subtitle").assert_exists();
    rule.on_node_with_text("Collapsed TopAppBar").assert_does_not_exist();
    named_args!(rule.on_node_with_text("Lorem ipsum", substring = true)).assert_exists();
    top_scroll_up(&rule, 300.0);
    assert!(top_near(top_bar(&rule).0.height(), 64.0));
    rule.on_node_with_text("Collapsed Subtitle").assert_exists();
    rule.on_node_with_text("Expanded TopAppBar").assert_does_not_exist();
}

// ==== bottom app bars, AppBarRow, AppBarColumn ===================================================================
// The C++ Kompoz's bottom app bar, AppBarRow and AppBarColumn tests (kompoz_tests.cpp; kompozable_tests.cpp has no
// source for them) written as a Compose user writes them, checked as crates/kompoz-material3/tests/bottom_app_bars.rs
// checks them (M15's values; the differences from the C++'s said there), and M15's bottom-bar AppBarSamples
// (kt/bottom_app_bars.kt).

fn bottom_near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

fn bottom_same_rect(a: Rect, b: Rect) -> bool {
    bottom_near(a.left, b.left)
        && bottom_near(a.top, b.top)
        && bottom_near(a.right, b.right)
        && bottom_near(a.bottom, b.bottom)
}

fn bottom_tag(rule: &KompozTestRule, tag: &str) -> Rect {
    named_args!(rule.on_node_with_tag(tag, use_unmerged_tree = true)).get_unclipped_bounds_in_root()
}

fn bottom_described(rule: &KompozTestRule, description: &str) -> Rect {
    rule.on_node_with_content_description(description).get_unclipped_bounds_in_root()
}

/// Whether `icon` was drawn (an item left out of a row or column is composed, not placed: not drawn).
fn bottom_drawn(rule: &KompozTestRule, icon: ImageVector) -> bool {
    rule.draw().vectors.contains(&icon.id())
}

/// Whether a vector other than `icons` was drawn: the overflow indicator's ⋮ (material3's own copy of the MoreVert
/// icon; the indicator, not measured when nothing is left out, is composed, not placed).
fn bottom_drawn_other_than(rule: &KompozTestRule, icons: &[ImageVector]) -> bool {
    rule.draw().vectors.iter().any(|v| icons.iter().all(|i| i.id() != *v))
}

/// The C++ tests' `rule.swipe(from, to, frames)`: a move each frame, up at once (a fling).
fn bottom_swipe(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / frames as f32), -1, 0);
    }
    h.pointer_up(to, -1, 0);
    rule.advance_frame();
}

kompoz! {
    // (bottom_app_bar_as_compose_1_4: the classic bar at the window's bottom, an action and a FAB)
    @Composable
    fun ClassicBottomBar() {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomStart) {
            BottomAppBar(
                actions = { Box(Modifier.size(48.dp).testTag("eylem")) },
                floatingActionButton = { Box(Modifier.size(56.dp).testTag("fab")) },
                windowInsets = WindowInsets(0.dp),
                modifier = Modifier.testTag("çubuk"),
            )
        }
    }

    // (app_bar_rows_put_what_does_not_fit_in_a_menu: four clickable items and a toggleable one, `width` wide, at most
    // `maxItems` slots)
    @Composable
    fun OverflowingRow(width: Dp, maxItems: Int) {
        var starred by remember { mutableStateOf(false) }
        var clicked by remember { mutableStateOf("yok") }
        val names = listOf("Bir", "İki", "Üç", "Dört")
        val icons = listOf(Icons.Filled.Add, Icons.Filled.Edit, Icons.Filled.Share, Icons.Filled.Delete)
        Column {
            Box(Modifier.width(width)) {
                AppBarRow(modifier = Modifier.testTag("satır"), maxItemCount = maxItems) {
                    names.forEachIndexed { i, name ->
                        clickableItem(
                            onClick = { clicked = name },
                            icon = { Icon(icons[i], contentDescription = name) },
                            label = "$name öğesi",
                        )
                    }
                    toggleableItem(
                        checked = starred,
                        onCheckedChange = { starred = it },
                        icon = { Icon(Icons.Filled.Star, contentDescription = "Beş") },
                        label = "Beş öğesi",
                    )
                }
            }
            Text("tıklanan: $clicked, ${if (starred) "yıldızlı" else "yıldızsız"}")
        }
    }

    // (app_bar_columns_put_what_does_not_fit_in_a_menu_below: four items in 150 dp)
    @Composable
    fun OverflowingColumn() {
        var picked by remember { mutableStateOf("yok") }
        Column {
            Box(Modifier.height(150.dp)) {
                AppBarColumn(modifier = Modifier.testTag("sütun")) {
                    clickableItem(onClick = { picked = "Ekle" }, icon = { Icon(Icons.Filled.Add, contentDescription = "Ekle") }, label = "Ekle öğesi")
                    clickableItem(onClick = { picked = "Düzenle" }, icon = { Icon(Icons.Filled.Edit, contentDescription = "Düzenle") }, label = "Düzenle öğesi")
                    clickableItem(onClick = { picked = "Paylaş" }, icon = { Icon(Icons.Filled.Share, contentDescription = "Paylaş") }, label = "Paylaş öğesi")
                    clickableItem(onClick = { picked = "Sil" }, icon = { Icon(Icons.Filled.Delete, contentDescription = "Sil") }, label = "Sil öğesi")
                }
            }
            Text("seçilen: $picked")
        }
    }

    // (bottom_app_bars_spread_out_and_leave_as_the_content_scrolls_on: the flexible bar or the classic one, an
    // exit-always behavior, over a list of 60 rows 60 dp tall)
    @Composable
    fun LeavingBottomBar(flexible: Boolean) {
        val scrollBehavior = BottomAppBarDefaults.exitAlwaysScrollBehavior()
        Scaffold(
            modifier = Modifier.fillMaxSize().nestedScroll(scrollBehavior.nestedScrollConnection),
            bottomBar = {
                if (flexible) {
                    FlexibleBottomAppBar(
                        modifier = Modifier.testTag("çubuk"),
                        scrollBehavior = scrollBehavior,
                        windowInsets = WindowInsets(0.dp),
                    ) {
                        Box(Modifier.size(48.dp).testTag("a"))
                        Box(Modifier.size(48.dp).testTag("b"))
                        Box(Modifier.size(48.dp).testTag("c"))
                    }
                } else {
                    BottomAppBar(
                        actions = { Box(Modifier.size(48.dp).testTag("a")) },
                        windowInsets = WindowInsets(0.dp),
                        scrollBehavior = scrollBehavior,
                        modifier = Modifier.testTag("çubuk"),
                    )
                }
            },
        ) { padding ->
            LazyColumn(Modifier.fillMaxSize(), contentPadding = padding) {
                items(60) { Box(Modifier.fillMaxWidth().height(60.dp)) }
            }
        }
    }
}

/// The C++ test: 80 dp, the actions 4 dp in at the start and centered below the 4 dp at the top, the FAB 12 dp down
/// and 16 dp in from the end.
fn bottom_app_bar_as_compose_1_4() {
    let rule = KompozTestRule::new(ClassicBottomBar);
    let (bar, action, fab) = (bottom_tag(&rule, "çubuk"), bottom_tag(&rule, "eylem"), bottom_tag(&rule, "fab"));
    assert!(bottom_same_rect(bar, Rect::new(0.0, 720.0, 400.0, 800.0)), "{bar:?}");
    assert!(bottom_near(action.left, 4.0) && bottom_near(action.center().y, 720.0 + 4.0 + 38.0), "{action:?}");
    assert!(bottom_near(fab.top, 720.0 + 12.0) && bottom_near(fab.right, 400.0 - 16.0), "{fab:?}");
}

/// The C++ test: what fits in the row, room kept for the ⋮ (which the last item may take), the rest in its menu,
/// whose items run their actions and close it; `maxItemCount` slots at most.
fn app_bar_rows_put_what_does_not_fit_in_a_menu() {
    let width = mutable_state_of(200.0f32);
    let max_items = mutable_state_of(i32::MAX);
    let rule = {
        let (width, max_items) = (width.clone(), max_items.clone());
        KompozTestRule::new(kompozable_lambda!(move || OverflowingRow(width.get().dp(), max_items.get())))
    };
    let (add, edit, share, delete, star) = (
        Icons::Filled::Add(),
        Icons::Filled::Edit(),
        Icons::Filled::Share(),
        Icons::Filled::Delete(),
        Icons::Filled::Star(),
    );
    // 200 dp: three, and the ⋮.
    assert!(
        bottom_drawn(&rule, add.clone()) && bottom_drawn(&rule, share.clone()) && !bottom_drawn(&rule, delete.clone())
    );
    assert!(!bottom_drawn(&rule, star.clone()) && bottom_near(bottom_tag(&rule, "satır").width(), 4.0 * 48.0));
    rule.on_node_with_content_description("More options").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Dört öğesi").assert_exists();
    rule.on_node_with_text("Üç öğesi").assert_does_not_exist();
    rule.on_node_with_text("Beş öğesi").perform_click(); // a toggle from the menu: flipped, the menu gone
    rule.wait_for_idle();
    rule.on_node_with_text("tıklanan: yok, yıldızlı").assert_exists();
    rule.on_node_with_text("Dört öğesi").assert_does_not_exist();
    rule.on_node_with_content_description("More options").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Dört öğesi").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("tıklanan: Dört, yıldızlı").assert_exists();
    rule.on_node_with_text("Beş öğesi").assert_does_not_exist();
    width.set(240.0); // four fit, and the last one takes the ⋮'s room: no menu
    rule.wait_for_idle();
    let all = [add, edit.clone(), share.clone(), delete, star.clone()];
    assert!(bottom_drawn(&rule, star) && bottom_near(bottom_tag(&rule, "satır").width(), 5.0 * 48.0));
    assert!(!bottom_drawn_other_than(&rule, &all));
    max_items.set(3); // three slots: two, and the ⋮
    rule.wait_for_idle();
    assert!(bottom_drawn(&rule, edit) && !bottom_drawn(&rule, share) && bottom_drawn_other_than(&rule, &all));
}

/// The C++ test: the same downwards, the ⋮ at the bottom.
fn app_bar_columns_put_what_does_not_fit_in_a_menu_below() {
    let rule = KompozTestRule::new(OverflowingColumn);
    let column = bottom_tag(&rule, "sütun");
    assert!(bottom_near(column.height(), 3.0 * 48.0) && bottom_near(column.width(), 48.0), "{column:?}");
    assert!(bottom_drawn(&rule, Icons::Filled::Edit()) && !bottom_drawn(&rule, Icons::Filled::Share()));
    // (the node described is the icon button's, 40 dp in its 48 dp room: 4 dp down)
    assert!(bottom_near(bottom_described(&rule, "More options").top, column.top + 2.0 * 48.0 + 4.0));
    rule.on_node_with_content_description("More options").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Paylaş öğesi").assert_exists();
    rule.on_node_with_text("Düzenle öğesi").assert_does_not_exist();
    rule.on_node_with_text("Sil öğesi").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("seçilen: Sil").assert_exists();
}

/// The C++ test: the flexible bar spread out across its 64 dp, 16 dp in; it leaves as far as the content scrolls
/// on, all of it past its height; back a little over half way and let go, all the way back; the classic one leaves
/// the same way, 80 dp of it.
fn bottom_app_bars_spread_out_and_leave_as_the_content_scrolls_on() {
    let flexible = mutable_state_of(true);
    let rule = {
        let flexible = flexible.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || LeavingBottomBar(flexible.get())),
            IntSize::new(400, 600),
        )
    };
    // (the clock by hand: the checks after a gesture see the bar before it settles)
    rule.main_clock().set_auto_advance(false);
    let bar = || bottom_tag(&rule, "çubuk");
    assert!(bottom_same_rect(bar(), Rect::new(0.0, 536.0, 400.0, 600.0)), "{:?}", bar());
    assert!(bottom_near(bottom_tag(&rule, "a").left, 16.0) && bottom_near(bottom_tag(&rule, "b").left, 176.0));
    assert!(bottom_near(bottom_tag(&rule, "c").left, 336.0));
    bottom_swipe(&rule, Offset::new(200.0, 300.0), Offset::new(200.0, 270.0), 10); // forwards: down as far
    rule.advance_frame();
    let went = 64.0 - bar().height();
    assert!(went > 20.0 && went < 30.5, "{went}");
    bottom_swipe(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 250.0), 10); // past it: gone
    common::settle(&rule);
    assert!(bottom_near(bar().height(), 0.0), "{:?}", bar());
    let h = rule.host(); // back a little over half way, held, let go: all the way back
    h.pointer_down(Offset::new(200.0, 300.0), -1, 0, PointerType::Touch);
    for i in 1..=8 {
        rule.advance_frame();
        h.pointer_move(Offset::new(200.0, 300.0 + 6.5 * i as f32), -1, 0);
    }
    for _ in 0..10 {
        rule.advance_frame();
        h.pointer_move(Offset::new(200.0, 352.0), -1, 0);
    }
    let halfway = 64.0 - bar().height(); // (how far it still is from all the way back)
    h.pointer_up(Offset::new(200.0, 352.0), -1, 0);
    common::settle(&rule);
    assert!(halfway > 5.0 && halfway < 32.0, "{halfway}");
    assert!(bottom_near(bar().height(), 64.0), "{:?}", bar());
    flexible.set(false); // the classic one, 80 dp of it
    common::settle(&rule);
    assert!(bottom_near(bar().height(), 80.0));
    bottom_swipe(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 250.0), 10);
    common::settle(&rule);
    assert!(bottom_near(bar().height(), 0.0), "{:?}", bar());
}

kompoz_file!("tests/kt/bottom_app_bars.kt");

/// The bar a sample drew: the lowest rect as wide as the window and less than half its height, and its color.
fn bottom_bar(rule: &KompozTestRule) -> (Rect, Color) {
    let c = rule.draw();
    c.rects
        .iter()
        .zip(&c.rect_colors)
        .filter(|(r, _)| bottom_near(r.left, 0.0) && bottom_near(r.width(), 400.0) && r.height() < 400.0)
        .max_by(|(a, _), (b, _)| a.top.total_cmp(&b.top))
        .map(|(r, c)| (*r, *c))
        .unwrap_or((Rect::Zero, Color::Unspecified))
}

/// Scrolls a sample's content up by `by` (a slow drag) and lets it settle.
fn bottom_scroll_up(rule: &KompozTestRule, by: f32) {
    top_slow_drag(rule, Offset::new(200.0, 500.0), Offset::new(200.0, 500.0 - by), 20);
    rule.wait_for_idle();
}

fn simple_bottom_app_bar() {
    let rule = KompozTestRule::new(SimpleBottomAppBar);
    assert_eq!(bottom_bar(&rule), (Rect::new(0.0, 0.0, 400.0, 80.0), light_color_scheme().surface_container));
    let menu = bottom_described(&rule, "Menu button").center();
    assert!(bottom_near(menu.x, 28.0) && bottom_near(menu.y, 42.0), "{menu:?}");
}

fn bottom_app_bar_with_fab() {
    let rule = KompozTestRule::new(BottomAppBarWithFAB);
    assert_eq!(bottom_bar(&rule).0, Rect::new(0.0, 0.0, 400.0, 80.0));
    assert!(bottom_near(bottom_described(&rule, "Edit").center().x, 28.0 + 48.0));
    let fab = bottom_described(&rule, "Add");
    assert!(bottom_same_rect(fab, Rect::new(328.0, 12.0, 384.0, 68.0)), "{fab:?}");
    let c = rule.draw();
    let color = c.rects.iter().zip(&c.rect_colors).find(|(r, _)| bottom_same_rect(**r, fab)).map(|(_, c)| *c);
    assert_eq!(color, Some(light_color_scheme().secondary_container));
}

fn exit_always_bottom_app_bar() {
    let rule = KompozTestRule::new(ExitAlwaysBottomAppBar);
    assert_eq!(bottom_bar(&rule).0, Rect::new(0.0, 720.0, 400.0, 800.0));
    let fab = bottom_described(&rule, "Add"); // over the bar, 16 dp in, 4 dp lower
    assert!(bottom_same_rect(fab, Rect::new(328.0, 732.0, 384.0, 788.0)), "{fab:?}");
    bottom_scroll_up(&rule, 300.0);
    assert!(bottom_bar(&rule).0.top >= 799.5, "{:?}", bottom_bar(&rule)); // gone below the window
    assert!(bottom_same_rect(bottom_described(&rule, "Add"), fab));
}

/// A flexible sample: 64 dp at the bottom in `color`, its five buttons' centers at `centers`; scrolled on, it leaves.
fn bottom_flexible_sample(rule: &KompozTestRule, color: Color, centers: [f32; 5]) {
    assert_eq!(bottom_bar(rule), (Rect::new(0.0, 736.0, 400.0, 800.0), color));
    for (name, x) in ["Back", "Forward", "Add", "Check", "Edit"].into_iter().zip(centers) {
        let c = bottom_described(rule, name).center();
        assert!(bottom_near(c.x, x) && bottom_near(c.y, 768.0), "{name}: {c:?}, not {x}");
    }
    bottom_scroll_up(rule, 300.0);
    assert!(bottom_bar(rule).0.top >= 799.5, "{:?}", bottom_bar(rule));
}

fn exit_always_bottom_app_bar_spaced_around() {
    let share = (400.0 - 4.0 * 48.0 - 56.0) / 5.0;
    let first = share / 2.0 + 24.0;
    let centers = [first, first + 48.0 + share, 200.0, 400.0 - first - 48.0 - share, 400.0 - first];
    bottom_flexible_sample(
        &KompozTestRule::new(ExitAlwaysBottomAppBarSpacedAround),
        light_color_scheme().surface_container,
        centers,
    );
}

fn exit_always_bottom_app_bar_spaced_between() {
    let gap = (400.0 - 32.0 - 4.0 * 48.0 - 56.0) / 4.0;
    let centers = [40.0, 40.0 + 48.0 + gap, 200.0, 360.0 - 48.0 - gap, 360.0];
    bottom_flexible_sample(
        &KompozTestRule::new(ExitAlwaysBottomAppBarSpacedBetween),
        light_color_scheme().surface_container,
        centers,
    );
}

fn exit_always_bottom_app_bar_spaced_evenly() {
    let gap = (400.0 - 4.0 * 48.0 - 56.0) / 6.0;
    let centers = [gap + 24.0, 2.0 * gap + 72.0, 200.0, 400.0 - 2.0 * gap - 72.0, 400.0 - gap - 24.0];
    bottom_flexible_sample(
        &KompozTestRule::new(ExitAlwaysBottomAppBarSpacedEvenly),
        light_color_scheme().surface_container,
        centers,
    );
}

fn exit_always_bottom_app_bar_fixed() {
    // 32 dp apart in the 368 dp inside the padding; the last one given the 40 dp left
    let centers = [40.0, 120.0, 204.0, 288.0, 364.0];
    bottom_flexible_sample(
        &KompozTestRule::new(ExitAlwaysBottomAppBarFixed),
        light_color_scheme().surface_container,
        centers,
    );
}

fn exit_always_bottom_app_bar_fixed_vibrant() {
    let centers = [40.0, 120.0, 204.0, 288.0, 364.0];
    bottom_flexible_sample(
        &KompozTestRule::new(ExitAlwaysBottomAppBarFixedVibrant),
        light_color_scheme().primary_container,
        centers,
    );
}

fn bottom_app_bar_with_overflow() {
    let rule = KompozTestRule::new(BottomAppBarWithOverflow);
    // 208 dp inside the padding, 48 kept for the indicator: three items and the indicator, centered.
    assert!(bottom_near(bottom_described(&rule, "Back").center().x, 104.0 + 24.0));
    assert!(bottom_near(bottom_described(&rule, "Overflow").center().x, 104.0 + 3.0 * 48.0 + 24.0));
    rule.on_node_with_content_description("Overflow").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Favorite").assert_exists();
    rule.on_node_with_text("Back").assert_does_not_exist();
    rule.on_node_with_text("Edit").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Favorite").assert_does_not_exist();
}

fn main() {
    common::run(&[
        // ==== top app bars ====
        // (the C++ Kompoz's)
        (
            "top_app_bar_enter_always_hides_before_the_list_scrolls",
            top_app_bar_enter_always_hides_before_the_list_scrolls,
        ),
        (
            "a_collapsing_top_bar_moves_the_content_without_recomposing_it",
            a_collapsing_top_bar_moves_the_content_without_recomposing_it,
        ),
        ("large_top_app_bar_exit_until_collapsed", large_top_app_bar_exit_until_collapsed),
        (
            "two_rows_top_app_bars_let_their_titles_style_themselves",
            two_rows_top_app_bars_let_their_titles_style_themselves,
        ),
        ("pinned_top_app_bar_knows_when_content_is_under_it", pinned_top_app_bar_knows_when_content_is_under_it),
        ("a_top_app_bar_takes_its_scrolled_color", a_top_app_bar_takes_its_scrolled_color),
        (
            "flexible_top_app_bars_are_bigger_and_place_their_title_across",
            flexible_top_app_bars_are_bigger_and_place_their_title_across,
        ),
        ("top_app_bars_show_a_subtitle_under_the_title", top_app_bars_show_a_subtitle_under_the_title),
        ("top_app_bar_colors_heights_and_insets_can_be_set", top_app_bar_colors_heights_and_insets_can_be_set),
        ("center_aligned_top_app_bar_mirrors_right_to_left", center_aligned_top_app_bar_mirrors_right_to_left),
        // (the generator's)
        ("top_app_bar_actions_are_in_a_row_scope", top_app_bar_actions_are_in_a_row_scope),
        // (M15's samples)
        ("simple_top_app_bar", simple_top_app_bar),
        ("simple_top_app_bar_with_adaptive_actions", simple_top_app_bar_with_adaptive_actions),
        ("simple_top_app_bar_with_subtitle", simple_top_app_bar_with_subtitle),
        ("simple_center_aligned_top_app_bar", simple_center_aligned_top_app_bar),
        ("simple_center_aligned_top_app_bar_with_subtitle", simple_center_aligned_top_app_bar_with_subtitle),
        ("pinned_top_app_bar", pinned_top_app_bar),
        ("pinned_top_app_bar_with_pre_scrolled_lazy_column", pinned_top_app_bar_with_pre_scrolled_lazy_column),
        ("pinned_top_app_bar_with_reversed_lazy_grid", pinned_top_app_bar_with_reversed_lazy_grid),
        ("enter_always_top_app_bar", enter_always_top_app_bar),
        ("enter_always_top_app_bar_with_reverse_scrolling", enter_always_top_app_bar_with_reverse_scrolling),
        ("exit_until_collapsed_medium_top_app_bar", exit_until_collapsed_medium_top_app_bar),
        (
            "exit_until_collapsed_center_aligned_medium_flexible_top_app_bar",
            exit_until_collapsed_center_aligned_medium_flexible_top_app_bar,
        ),
        ("exit_until_collapsed_large_top_app_bar", exit_until_collapsed_large_top_app_bar),
        (
            "exit_until_collapsed_center_aligned_large_flexible_top_app_bar",
            exit_until_collapsed_center_aligned_large_flexible_top_app_bar,
        ),
        ("custom_two_rows_top_app_bar", custom_two_rows_top_app_bar),
        // ==== bottom app bars ====
        // (the C++ Kompoz's)
        ("bottom_app_bar_as_compose_1_4", bottom_app_bar_as_compose_1_4),
        ("app_bar_rows_put_what_does_not_fit_in_a_menu", app_bar_rows_put_what_does_not_fit_in_a_menu),
        (
            "app_bar_columns_put_what_does_not_fit_in_a_menu_below",
            app_bar_columns_put_what_does_not_fit_in_a_menu_below,
        ),
        (
            "bottom_app_bars_spread_out_and_leave_as_the_content_scrolls_on",
            bottom_app_bars_spread_out_and_leave_as_the_content_scrolls_on,
        ),
        // (M15's samples)
        ("simple_bottom_app_bar", simple_bottom_app_bar),
        ("bottom_app_bar_with_fab", bottom_app_bar_with_fab),
        ("exit_always_bottom_app_bar", exit_always_bottom_app_bar),
        ("exit_always_bottom_app_bar_spaced_around", exit_always_bottom_app_bar_spaced_around),
        ("exit_always_bottom_app_bar_spaced_between", exit_always_bottom_app_bar_spaced_between),
        ("exit_always_bottom_app_bar_spaced_evenly", exit_always_bottom_app_bar_spaced_evenly),
        ("exit_always_bottom_app_bar_fixed", exit_always_bottom_app_bar_fixed),
        ("exit_always_bottom_app_bar_fixed_vibrant", exit_always_bottom_app_bar_fixed_vibrant),
        ("bottom_app_bar_with_overflow", bottom_app_bar_with_overflow),
    ]);
}
