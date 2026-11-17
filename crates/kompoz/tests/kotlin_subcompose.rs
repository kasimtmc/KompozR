//! SubcomposeLayout and BoxWithConstraints in Kotlin's syntax: the C++ Kompoz's `Underlined` (kompozable_tests.cpp's
//! SubkompozLayout, its spelling taken as Compose's `SubcomposeLayout`/`subcompose`) and the BoxWithConstraints part
//! of its `layouts_as_compose_writes_them`, through `kompoz!`; Compose's spelling, a slot reuse policy and a scope's
//! constraints through a `.kt` file (`kompoz_file!`). The same layouts the Rust syntax's tests check
//! (crates/kompoz-foundation/tests/subcompose.rs).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::{KompozTestRule, Text};
use kompoz::ui::media_query::media_query;

/// A node's bounds in the root: x, y, width, height (px; the test rule's density is 1).
fn bounds(rule: &KompozTestRule, tag: &str) -> [f32; 4] {
    let b = rule.on_node_with_tag(tag).get_unclipped_bounds_in_root();
    [b.left, b.top, b.width(), b.height()]
}

// ---- custom_layouts_as_compose_writes_them: Underlined (kompozable_tests.cpp 2161-2190) ----------------------------

kompoz! {
    // (a slot measured, then a line as wide as it composed from its width: composed as it is asked for, there on the
    // first pass; Compose's Int px, `width.toDp()`, where the C++ Kompoz's was in floats)
    @Kompozable
    fun Underlined(content: @Kompozable () -> Unit) {
        SubkompozLayout(modifier = Modifier.testTag("underlined")) { constraints ->
            val main = subkompoz("main", content).map { it.measure(constraints.copy(minWidth = 0, minHeight = 0)) }
            val width = main.maxOf { it.width }
            val height = main.sumOf { it.height }
            val line = subkompoz("line") { Box(modifier = Modifier.size(width.toDp(), 2.dp)) }.map { it.measure(constraints) }
            layout(width, height + 2.dp.roundToPx()) {
                main.forEach { it.placeRelative(0, 0) }
                line.forEach { it.placeRelative(0, height) }
            }
        }
    }

    @Kompozable
    fun UnderlinedBox() {
        Column {
            Underlined { Box(modifier = Modifier.size(50.dp, 10.dp)) }
        }
    }
}

fn custom_layouts_as_compose_writes_them() {
    let rule = KompozTestRule::new(UnderlinedBox);
    let under = bounds(&rule, "underlined");
    assert_eq!((under[2], under[3]), (50.0, 12.0));
}

// ---- layouts_as_compose_writes_them: BoxWithConstraints (kompozable_tests.cpp 2124-2145) ---------------------------

kompoz! {
    // (FlexBox, Grid and FlowRow, the rest of the C++ test, are WP3's; mediaQuery { } is the UI core's)
    @Composable
    fun ComposeLayouts() {
        Column {
            BoxWithConstraints(modifier = Modifier.width(150.dp)) {
                Text(if (maxWidth > 100.dp) "wide" else "narrow")
            }
            BoxWithConstraints(modifier = Modifier.width(80.dp).testTag("narrow box")) {
                Text(if (maxWidth > 100.dp) "wide" else "narrow")
            }
            if (mediaQuery { windowWidth > 0.dp }) Text("has a window")
        }
    }
}

fn layouts_as_compose_writes_them() {
    let rule = KompozTestRule::new(ComposeLayouts);
    rule.on_node_with_text("wide").assert_exists();
    rule.on_node_with_text("narrow").assert_exists();
    rule.on_node_with_text("has a window").assert_exists();
}

// ---- Compose's spelling, from a .kt file -------------------------------------------------------------------------

kompoz_file!("tests/kt/subcompose.kt");

fn compose_spelling_from_a_kt_file() {
    let rule = KompozTestRule::with_viewport(Adaptive, IntSize::new(400, 800));
    rule.on_node_with_text("compact 400x790 min 400x790").assert_exists();
    let side = bounds(&rule, "side");
    assert_eq!((side[0], side[2]), (100.0, 20.0)); // placed after the measured title, as wide as its slot asked
    rule.host().set_viewport_size(IntSize::new(700, 800));
    rule.wait_for_idle();
    rule.on_node_with_text("expanded 700x790 min 700x790").assert_exists();
    rule.on_node_with_text("compact 400x790 min 400x790").assert_does_not_exist();
}

tests![custom_layouts_as_compose_writes_them, layouts_as_compose_writes_them, compose_spelling_from_a_kt_file];
