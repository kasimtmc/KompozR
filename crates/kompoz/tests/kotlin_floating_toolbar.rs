//! Material's floating toolbars in Kotlin's syntax: material3 1.5's FloatingToolbarSamples from a `.kt` file
//! (kompoz_file!) and the C++ Kompoz's floating toolbar test written as a Compose user writes it, with the floating
//! toolbar of its kompozable_tests.cpp's Navigation source, each composed and used as a user does. Checked as
//! crates/kompoz-material3/tests/floating_toolbar.rs checks them (M15's values; the differences from the C++'s said
//! there). Density 1: dp are px; the viewport is 400 × 800 unless said.

#![allow(non_snake_case, non_upper_case_globals, unused_variables)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
// (the samples' imports of what the generated code names: TooltipBox's position provider, nestedScroll's connection)
use kompoz::ui::input::{NestedScrollConnection, PointerType};
use kompoz::ui::window::PopupPositionProvider;

kompoz_file!("tests/kt/floating_toolbar.kt");

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

fn same_rect(a: Rect, b: Rect) -> bool {
    near(a.left, b.left) && near(a.top, b.top) && near(a.right, b.right) && near(a.bottom, b.bottom)
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    named_args!(rule.on_node_with_tag(tag, use_unmerged_tree = true)).get_unclipped_bounds_in_root()
}

fn exists(rule: &KompozTestRule, tag: &str) -> bool {
    named_args!(rule.on_all_nodes_with_tag(tag, use_unmerged_tree = true)).fetch_semantics_nodes().len() == 1
}

/// The color of the rect drawn at `b`.
fn fill(rule: &KompozTestRule, b: Rect) -> Option<Color> {
    let canvas = rule.draw();
    canvas.rects.iter().zip(&canvas.rect_colors).find(|(r, _)| same_rect(**r, b)).map(|(_, color)| *color)
}

/// A finger's swipe from `from` to `to` over `frames` frames, let go at once (a fling).
fn swipe(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / frames as f32), -1, 0);
    }
    h.pointer_up(to, -1, 0);
    rule.advance_frame();
}

/// Where what is in the node tagged `tag` is (its first child's bounds): a toolbar's tag sits outside the layout its
/// scroll behavior moves.
fn moved(rule: &KompozTestRule, tag: &str) -> Rect {
    named_args!(rule.on_node_with_tag(tag, use_unmerged_tree = true))
        .on_children()
        .on_first()
        .get_unclipped_bounds_in_root()
}

// ==== the C++ Kompoz's ===============================================================================================
// kompoz_tests.cpp's floating_toolbar_shows_its_side_actions_when_expanded_and_leaves_on_scroll (kompozable_tests.cpp
// has no source for it) and the HorizontalFloatingToolbar of kompozable_tests.cpp's Navigation (2849-2899; its tabs are
// in kotlin_lists_sheets.rs, its FloatingActionButtonMenu is WP1b's).

kompoz! {
    // (the C++'s `action`: a 48 dp action)
    @Composable
    fun Action(tag: String) {
        Box(Modifier.size(48.dp).testTag(tag))
    }

    @Composable
    fun SideActions() {
        var expanded by remember { mutableStateOf(false) }
        Column {
            Button(onClick = { expanded = !expanded }, modifier = Modifier.testTag("aç")) { Text("Aç") }
            HorizontalFloatingToolbar(expanded, Modifier.testTag("yatay"), leadingContent = { Action("baş") }, trailingContent = { Action("son") }) {
                Action("orta")
            }
            VerticalFloatingToolbar(expanded, Modifier.testTag("dikey"), colors = FloatingToolbarDefaults.vibrantFloatingToolbarColors(),
                                    trailingContent = { Action("alt") }) {
                Action("dikey-orta")
            }
            HorizontalFloatingToolbar(
                expanded = expanded,
                floatingActionButton = { FloatingToolbarDefaults.StandardFloatingActionButton(onClick = { }, modifier = Modifier.testTag("fab")) { } },
                modifier = Modifier.testTag("fablı"),
            ) {
                Action("araç")
            }
        }
    }

    // (the C++'s second page: toolbars leaving as the list scrolls)
    @Composable
    fun LeavingToolbars() {
        val kept = FloatingToolbarDefaults.exitAlwaysScrollBehavior(exitDirection = FloatingToolbarExitDirection.Bottom)
        val side = FloatingToolbarDefaults.exitAlwaysScrollBehavior(exitDirection = FloatingToolbarExitDirection.End)
        Box(Modifier.size(400.dp, 600.dp).nestedScroll(kept).nestedScroll(side)) {
            LazyColumn(Modifier.fillMaxSize()) {
                items(100) { Box(Modifier.fillMaxWidth().height(60.dp)) }
            }
            HorizontalFloatingToolbar(true, Modifier.align(Alignment.BottomCenter).offset(y = -16.dp), scrollBehavior = kept) {
                Action("sayfa")
            }
            VerticalFloatingToolbar(true, Modifier.align(Alignment.CenterEnd).offset(x = -16.dp), scrollBehavior = side) {
                Action("yan")
            }
        }
    }

    // (Kotlin's layout scopes of a toolbar's content: `RowScope.AnimatedVisibility` in a horizontal one's, leading
    // content too, `ColumnScope.AnimatedVisibility` in a vertical one's)
    @Composable
    fun ToolbarScopes() {
        var shown by remember { mutableStateOf(true) }
        Box {
            Button(onClick = { shown = !shown }, modifier = Modifier.testTag("gizle")) { Text("Gizle") }
            HorizontalFloatingToolbar(true, Modifier.offset(y = 100.dp).testTag("yatay"), leadingContent = { AnimatedVisibility(shown) { Action("sol") } }) {
                AnimatedVisibility(shown) { Action("satır") }
            }
            VerticalFloatingToolbar(true, Modifier.offset(y = 200.dp).testTag("dikey")) {
                AnimatedVisibility(shown) { Action("sütun") }
            }
        }
    }

    // Navigation's floating toolbar as the C++ wrote it (its text without the tab and the FAB menu's pick, which are
    // the parts kotlin_lists_sheets.rs and WP1b have).
    @Kompozable
    fun NavigationToolbar() {
        var more by remember { mutableStateOf(false) }
        Column {
            Text("${if (more) "fazla" else "az"}")
            HorizontalFloatingToolbar(expanded = more, colors = FloatingToolbarDefaults.vibrantFloatingToolbarColors(),
                                      leadingContent = { IconButton(onClick = { }) { Icon(Icons.Filled.Favorite, contentDescription = "Beğen") } }) {
                IconButton(onClick = { more = !more }, modifier = Modifier.testTag("daha")) { Icon(Icons.Filled.MoreVert, contentDescription = "Daha") }
            }
        }
    }
}

/// The C++ test (M15's values: the FAB toolbar keeps its toolbar's room, its FAB 80 dp collapsed and 56 dp expanded,
/// in the primary container).
fn floating_toolbar_shows_its_side_actions_when_expanded_and_leaves_on_scroll() {
    let rule = KompozTestRule::new(SideActions);
    let cs = light_color_scheme();
    let bar = bounds(&rule, "yatay");
    // Collapsed: the content, 8 dp around; no side content.
    assert!(near(bar.width(), 64.0) && near(bar.height(), 64.0), "{bar:?}");
    assert_eq!(fill(&rule, bar), Some(cs.surface_container));
    assert!(!exists(&rule, "baş") && !exists(&rule, "son") && !exists(&rule, "alt"));
    assert_eq!(fill(&rule, bounds(&rule, "dikey")), Some(cs.primary_container)); // vibrant
    let (with_fab, fab) = (bounds(&rule, "fablı"), bounds(&rule, "fab"));
    assert!(near(with_fab.width(), 64.0 + 8.0 + 56.0) && near(with_fab.height(), 80.0), "{with_fab:?}");
    assert!(near(fab.width(), 80.0) && near(fab.right, with_fab.right), "{fab:?}");
    assert_eq!(fill(&rule, fab), Some(cs.primary_container));

    rule.on_node_with_tag("aç").perform_click();
    rule.wait_for_idle();
    let bar = bounds(&rule, "yatay");
    assert!(near(bar.width(), 8.0 + 3.0 * 48.0 + 8.0), "{bar:?}");
    assert!(near(bounds(&rule, "baş").left, 8.0) && near(bounds(&rule, "orta").left, 56.0));
    assert!(near(bounds(&rule, "son").left, 104.0));
    let column = bounds(&rule, "dikey");
    assert!(near(column.height(), 8.0 + 2.0 * 48.0 + 8.0) && near(bounds(&rule, "alt").top, column.top + 56.0));
    let (tools, fab) = (bounds(&rule, "araç"), bounds(&rule, "fab"));
    assert!(near(fab.left - tools.right, 8.0 + 8.0) && near(fab.width(), 56.0), "{tools:?} {fab:?}");

    // Out of view as the content scrolls forwards (the horizontal one below, the vertical one off the end), back as
    // it scrolls back.
    let page = KompozTestRule::with_viewport(LeavingToolbars, IntSize::new(400, 600));
    let (shown, beside) = (bounds(&page, "sayfa"), bounds(&page, "yan"));
    assert!(near(shown.bottom, 600.0 - 16.0 - 8.0), "{shown:?}");
    swipe(&page, Offset::new(200.0, 400.0), Offset::new(200.0, 300.0), 10);
    page.wait_for_idle();
    assert!(bounds(&page, "sayfa").top >= 600.0 - 0.5, "{:?}", bounds(&page, "sayfa"));
    let side = bounds(&page, "yan");
    assert!(near(side.left, beside.left + 80.0) && near(side.top, beside.top), "{side:?} {beside:?}");
    swipe(&page, Offset::new(200.0, 300.0), Offset::new(200.0, 400.0), 10);
    page.wait_for_idle();
    assert!(same_rect(bounds(&page, "sayfa"), shown) && same_rect(bounds(&page, "yan"), beside));
}

/// The C++'s `tabs_and_fab_menu_as_compose_writes_them`, its toolbar part: "Daha" expands the toolbar, showing its
/// leading "Beğen".
fn navigation_toolbar_as_compose_writes_it() {
    let rule = KompozTestRule::new(NavigationToolbar);
    rule.on_node_with_text("az").assert_exists();
    rule.on_node_with_content_description("Beğen").assert_does_not_exist();
    rule.on_node_with_tag("daha").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("fazla").assert_exists();
    rule.on_node_with_content_description("Beğen").assert_exists();
}

/// A toolbar's content in its layout scope: `AnimatedVisibility` there is the row's (it shrinks horizontally) or the
/// column's (vertically); hidden, the toolbars are their padding alone.
fn toolbar_content_is_in_its_layout_scope() {
    let rule = KompozTestRule::new(ToolbarScopes);
    assert!(exists(&rule, "sol") && exists(&rule, "satır") && exists(&rule, "sütun"));
    assert!(near(bounds(&rule, "yatay").width(), 8.0 + 2.0 * 48.0 + 8.0), "{:?}", bounds(&rule, "yatay"));
    rule.on_node_with_tag("gizle").perform_click();
    rule.wait_for_idle();
    assert!(!exists(&rule, "sol") && !exists(&rule, "satır") && !exists(&rule, "sütun"));
    assert!(near(bounds(&rule, "yatay").width(), 16.0) && near(bounds(&rule, "dikey").height(), 16.0));
}

// ==== M15's samples (kt/floating_toolbar.kt) =========================================================================

/// The descriptions of the icons shown (every icon of the samples has the same one).
fn shown_icons(rule: &KompozTestRule) -> usize {
    named_args!(rule.on_all_nodes_with_content_description("Localized description", use_unmerged_tree = true))
        .fetch_semantics_nodes()
        .len()
}

fn expandable_horizontal_floating_toolbar_sample() {
    let rule = KompozTestRule::new(ExpandableHorizontalFloatingToolbarSample);
    assert_eq!(shown_icons(&rule), 5); // leading 2, the content's, trailing 2
    swipe(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 300.0), 10); // forwards: collapses
    rule.wait_for_idle();
    assert_eq!(shown_icons(&rule), 1);
    swipe(&rule, Offset::new(200.0, 300.0), Offset::new(200.0, 400.0), 10); // back: expands
    rule.wait_for_idle();
    assert_eq!(shown_icons(&rule), 5);
}

fn overflowing_horizontal_floating_toolbar_sample() {
    // The AppBarRow gets less room than its five 48 dp items: some go in its overflow menu.
    let rule = KompozTestRule::new(OverflowingHorizontalFloatingToolbarSample);
    rule.on_node_with_content_description("More options").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("ArrowUpward").assert_exists();
}

fn scrollable_horizontal_floating_toolbar_sample() {
    let rule = KompozTestRule::new(ScrollableHorizontalFloatingToolbarSample);
    assert!(near(bounds(&rule, "araçlar").bottom, 800.0 - 16.0), "{:?}", bounds(&rule, "araçlar"));
    let inner = moved(&rule, "araçlar");
    swipe(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 200.0), 10);
    rule.wait_for_idle();
    assert!(moved(&rule, "araçlar").top >= 800.0 - 0.5, "out of view: {:?}", moved(&rule, "araçlar"));
    swipe(&rule, Offset::new(200.0, 200.0), Offset::new(200.0, 400.0), 10);
    rule.wait_for_idle();
    assert!(same_rect(moved(&rule, "araçlar"), inner), "{:?} {inner:?}", moved(&rule, "araçlar"));
}

fn expandable_vertical_floating_toolbar_sample() {
    let rule = KompozTestRule::new(ExpandableVerticalFloatingToolbarSample);
    assert_eq!(shown_icons(&rule), 5);
    swipe(&rule, Offset::new(150.0, 400.0), Offset::new(150.0, 300.0), 10);
    rule.wait_for_idle();
    assert_eq!(shown_icons(&rule), 1);
    swipe(&rule, Offset::new(150.0, 300.0), Offset::new(150.0, 400.0), 10);
    rule.wait_for_idle();
    assert_eq!(shown_icons(&rule), 5);
}

fn overflowing_vertical_floating_toolbar_sample() {
    // 400 dp tall: the AppBarColumn's five 48 dp items don't all fit.
    let rule = KompozTestRule::with_viewport(OverflowingVerticalFloatingToolbarSample, IntSize::new(400, 400));
    rule.on_node_with_content_description("More options").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("ArrowUpward").assert_exists();
}

fn scrollable_vertical_floating_toolbar_sample() {
    let rule = KompozTestRule::new(ScrollableVerticalFloatingToolbarSample);
    assert!(near(bounds(&rule, "araçlar").right, 400.0 - 16.0), "{:?}", bounds(&rule, "araçlar"));
    let inner = moved(&rule, "araçlar");
    swipe(&rule, Offset::new(150.0, 400.0), Offset::new(150.0, 200.0), 10);
    rule.wait_for_idle();
    assert!(moved(&rule, "araçlar").left >= 400.0 - 0.5, "out of the end: {:?}", moved(&rule, "araçlar"));
    swipe(&rule, Offset::new(150.0, 200.0), Offset::new(150.0, 400.0), 10);
    rule.wait_for_idle();
    assert!(same_rect(moved(&rule, "araçlar"), inner), "{:?} {inner:?}", moved(&rule, "araçlar"));
}

fn horizontal_floating_toolbar_with_fab_sample() {
    let rule = KompozTestRule::new(HorizontalFloatingToolbarWithFabSample);
    let bar = bounds(&rule, "araçlar");
    // Bottom end, 16 dp in: the toolbar (8 + 4 × 48 + 8), 8 dp, the 56 dp FAB.
    assert!(near(bar.right, 400.0 - 16.0) && near(bar.bottom, 800.0 - 16.0) && near(bar.width(), 208.0 + 8.0 + 56.0));
    assert_eq!(fill(&rule, bounds(&rule, "fab")), Some(light_color_scheme().tertiary_container));
    rule.on_node_with_tag("fab").perform_click();
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "fab").width(), 80.0)); // collapsed
    rule.on_node_with_tag("fab").perform_click();
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "fab").width(), 56.0));
    // The Lorem Ipsum scrolled forwards collapses it.
    swipe(&rule, Offset::new(100.0, 400.0), Offset::new(100.0, 300.0), 10);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "fab").width(), 80.0));
}

fn centered_horizontal_floating_toolbar_with_fab_sample() {
    let rule = KompozTestRule::new(CenteredHorizontalFloatingToolbarWithFabSample);
    let shown = bounds(&rule, "araçlar");
    assert!(near(shown.center().x, 200.0) && near(shown.bottom, 800.0 - 16.0), "{shown:?}");
    swipe(&rule, Offset::new(100.0, 400.0), Offset::new(100.0, 200.0), 10);
    rule.wait_for_idle();
    assert!(moved(&rule, "araçlar").top >= 800.0 - 0.5, "{:?}", moved(&rule, "araçlar"));
}

fn vertical_floating_toolbar_with_fab_sample() {
    let rule = KompozTestRule::new(VerticalFloatingToolbarWithFabSample);
    let bar = bounds(&rule, "araçlar");
    assert!(near(bar.bottom, 800.0 - 16.0) && near(bar.height(), 208.0 + 8.0 + 56.0), "{bar:?}");
    rule.on_node_with_tag("fab").perform_click();
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "fab").height(), 80.0));
    swipe(&rule, Offset::new(100.0, 400.0), Offset::new(100.0, 300.0), 10);
    swipe(&rule, Offset::new(100.0, 300.0), Offset::new(100.0, 400.0), 10); // back: expands
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "fab").height(), 56.0), "{:?}", bounds(&rule, "fab"));
}

fn centered_vertical_floating_toolbar_with_fab_sample() {
    let rule = KompozTestRule::new(CenteredVerticalFloatingToolbarWithFabSample);
    let shown = bounds(&rule, "araçlar");
    assert!(near(shown.center().y, 400.0) && near(shown.right, 400.0 - 16.0), "{shown:?}");
    swipe(&rule, Offset::new(100.0, 400.0), Offset::new(100.0, 200.0), 10);
    rule.wait_for_idle();
    assert!(moved(&rule, "araçlar").left >= 400.0 - 0.5, "{:?}", moved(&rule, "araçlar"));
}

fn horizontal_floating_toolbar_as_scaffold_fab_sample() {
    let rule = KompozTestRule::new(HorizontalFloatingToolbarAsScaffoldFabSample);
    // Where the Scaffold puts its FAB: 16 dp from the bottom end.
    let bar = bounds(&rule, "araçlar");
    assert!(near(bar.right, 400.0 - 16.0) && near(bar.bottom, 800.0 - 16.0), "{bar:?}");
    rule.on_node_with_tag("fab").perform_click();
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "fab").width(), 80.0));
    assert!(same_rect(bounds(&rule, "araçlar"), bar), "the layout keeps its room");
}

fn main() {
    common::run(&[
        // ==== floating toolbar ====
        // (the C++ Kompoz's)
        (
            "floating_toolbar_shows_its_side_actions_when_expanded_and_leaves_on_scroll",
            floating_toolbar_shows_its_side_actions_when_expanded_and_leaves_on_scroll,
        ),
        ("navigation_toolbar_as_compose_writes_it", navigation_toolbar_as_compose_writes_it),
        // (the generator's)
        ("toolbar_content_is_in_its_layout_scope", toolbar_content_is_in_its_layout_scope),
        // (M15's samples)
        ("expandable_horizontal_floating_toolbar_sample", expandable_horizontal_floating_toolbar_sample),
        ("overflowing_horizontal_floating_toolbar_sample", overflowing_horizontal_floating_toolbar_sample),
        ("scrollable_horizontal_floating_toolbar_sample", scrollable_horizontal_floating_toolbar_sample),
        ("expandable_vertical_floating_toolbar_sample", expandable_vertical_floating_toolbar_sample),
        ("overflowing_vertical_floating_toolbar_sample", overflowing_vertical_floating_toolbar_sample),
        ("scrollable_vertical_floating_toolbar_sample", scrollable_vertical_floating_toolbar_sample),
        ("horizontal_floating_toolbar_with_fab_sample", horizontal_floating_toolbar_with_fab_sample),
        ("centered_horizontal_floating_toolbar_with_fab_sample", centered_horizontal_floating_toolbar_with_fab_sample),
        ("vertical_floating_toolbar_with_fab_sample", vertical_floating_toolbar_with_fab_sample),
        ("centered_vertical_floating_toolbar_with_fab_sample", centered_vertical_floating_toolbar_with_fab_sample),
        ("horizontal_floating_toolbar_as_scaffold_fab_sample", horizontal_floating_toolbar_as_scaffold_fab_sample),
    ]);
}
