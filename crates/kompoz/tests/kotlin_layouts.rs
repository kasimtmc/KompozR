//! The non-lazy layouts, the marquee, the tooltip box and the ripple in Kotlin's syntax, as a Compose user writes
//! them: the FlexBox, Grid and FlowRow parts of the C++ Kompoz's `layouts_as_compose_writes_them`
//! (kompozable_tests.cpp 2124-2145: `config = { … }` on their scopes, `flex { grow(1f) }`, `1.fr`, `gridItem(row =,
//! column =)`, FlowRow's arrangement), its marquee by name (2947-2960), a ripple through `LocalIndication provides
//! ripple()`, through `kompoz!`; FlowColumn with its overflow indicator, ContextualFlowRow, BasicTooltipBox and a
//! FlexBox config by the constraints from a `.kt` file (kompoz_file!). The same layouts the Rust syntax's tests check
//! (crates/kompoz-foundation/tests/{flow_layout,flex_box,grid,marquee,tooltip}.rs,
//! crates/kompoz-material-ripple/tests/ripple.rs).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::{KompozTestRule, RecordingCanvas};
use kompoz::ui::input::PointerType;
use kompoz::ui::window::PopupPositionProvider;
use std::rc::Rc;

/// A node's bounds in the root, unclipped (px; the rule's density is 1).
fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

// ---- layouts_as_compose_writes_them (kompozable_tests.cpp 2124-2145): FlexBox, Grid, FlowRow ------------------

kompoz! {
    @Composable
    fun ComposeLayouts() {
        Column {
            FlexBox(modifier = Modifier.width(200.dp).testTag("flex"), config = { columnGap(10.dp) }) {
                Box(modifier = Modifier.size(40.dp))
                Box(modifier = Modifier.height(40.dp).flex { grow(1f) }.testTag("grows"))
            }
            Grid(config = {
                column(50.dp)
                column(1.fr)
                gap(row = 4.dp, column = 10.dp)
            }, modifier = Modifier.width(200.dp)) {
                Box(modifier = Modifier.height(20.dp).fillMaxWidth().gridItem(row = 1, column = 2).testTag("cell"))
            }
            FlowRow(modifier = Modifier.width(100.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                for (i in 0 until 3) Box(modifier = Modifier.size(40.dp).testTag("flow$i"))
            }
        }
    }
}

fn layouts_as_compose_writes_them() {
    let rule = KompozTestRule::new(ComposeLayouts);
    let (flex, grown) = (bounds(&rule, "flex"), bounds(&rule, "grows"));
    assert!(near(grown.left - flex.left, 50.0) && near(grown.width(), 150.0));
    let cell = bounds(&rule, "cell");
    assert!(near(cell.left, 60.0) && near(cell.width(), 140.0));
    assert!(near(bounds(&rule, "flow1").left, 50.0) && near(bounds(&rule, "flow2").left, 0.0));
}

// ---- text_styles_as_compose_writes_them (2947-2960): its marquee by name ---------------------------------------

kompoz! {
    // (scrolling for ever once `on`)
    @Composable
    fun Marquee(on: MutableState<Boolean>) {
        Column {
            Text(
                "kayan yazı kayan yazı kayan yazı kayan yazı kayan yazı",
                modifier = Modifier.testTag("kayan").width(120.dp)
                    .basicMarquee(iterations = if (on.value) Int.MAX_VALUE else 0, velocity = 60.dp),
            )
        }
    }
}

fn marquee_as_compose_writes_it() {
    let on = mutable_state_of(true);
    let o = on.clone();
    // (Compose's way: the clock's auto advance off before composing — while it is on, the rule's
    // InfiniteAnimationPolicy cancels the marquee's infinite animation)
    let rule = KompozTestRule::empty();
    rule.main_clock().set_auto_advance(false);
    rule.set_content(kompozable_lambda!(move || Marquee(o.clone())));
    rule.advance_frame();
    assert_eq!(bounds(&rule, "kayan").width(), 120.0);
    // (scrolling for ever, at 60 dp a second after its 1.2 s wait)
    let text_x = |rule: &KompozTestRule| rule.draw().baselines.first().map(|b| b.x).unwrap_or(f32::NAN);
    let start = text_x(&rule);
    rule.advance_time_by(2200);
    assert!((start - text_x(&rule) - 60.0).abs() < 4.0, "{start} {}", text_x(&rule));
}

// ---- A ripple through LocalIndication ----------------------------------------------------------------------------

kompoz! {
    @Composable
    fun Rippling() {
        CompositionLocalProvider(LocalIndication provides ripple()) {
            Box(modifier = Modifier.size(100.dp).clickable { }.testTag("dalga"))
        }
    }
}

/// The circles drawn (round rects rounded all the way).
fn circles(canvas: &RecordingCanvas) -> Vec<Rect> {
    canvas
        .rects
        .iter()
        .zip(&canvas.rect_radii)
        .filter(|(r, radius)| (r.width() - r.height()).abs() < 0.01 && (**radius - r.width() / 2.0).abs() < 0.01)
        .map(|(r, _)| *r)
        .collect()
}

fn a_ripple_through_local_indication() {
    let rule = KompozTestRule::new(Rippling);
    let b = bounds(&rule, "dalga");
    rule.host().pointer_down(b.center(), -1, 0, PointerType::Touch);
    rule.advance_time_by(300);
    let spread = circles(&rule.draw());
    // (to the corners and 10 dp further, from the middle)
    assert!(spread.len() == 1 && near(spread[0].width() / 2.0, 100f32.hypot(100.0) / 2.0 + 10.0), "{spread:?}");
    rule.host().pointer_up(b.center(), -1, 0);
    rule.wait_for_idle();
    assert!(circles(&rule.draw()).is_empty());
}

// ---- From a .kt file ---------------------------------------------------------------------------------------------

/// Under the anchor, its left edge lined up with it.
struct Below;

impl PopupPositionProvider for Below {
    fn calculate_position(&self, anchor: IntRect, _: IntSize, _: LayoutDirection, _: IntSize) -> IntOffset {
        IntOffset::new(anchor.left, anchor.bottom)
    }
}

fn below() -> Rc<dyn PopupPositionProvider> {
    Rc::new(Below)
}

kompoz_file!("tests/kt/layouts.kt");

fn flow_column_contextual_flow_row_and_a_tooltip_from_a_kt_file() {
    let rule = KompozTestRule::with_viewport(MoreLayouts, IntSize::new(400, 800));
    // FlowColumn: two 30s to a 70 tall column; at most one column, the rest counted by the indicator.
    assert!(rule.on_node_with_tag("c1").is_displayed() && !rule.on_node_with_tag("c2").is_displayed());
    assert!(near(bounds(&rule, "daha").top - bounds(&rule, "sütun").top, 60.0));
    // ContextualFlowRow: each item told its line as it is composed (the third, composed before it is found not to
    // fit, line 0: Compose's numbers), placed on the next.
    rule.on_node_with_text("0:0").assert_exists();
    rule.on_node_with_text("2:0").assert_exists();
    let (first, third) = (bounds(&rule, "ö0"), bounds(&rule, "ö2"));
    assert!(near(third.left, first.left) && near(third.top - first.top, first.height()), "{first:?} {third:?}");
    // ContextualFlowColumn: one 20 to a 30 tall column, the second in the next column.
    assert!(near(bounds(&rule, "k1:1").left - bounds(&rule, "k0:0").left, 20.0));
    // A FlexBox wrapping when narrow: its second child under the first.
    assert!(near(bounds(&rule, "ikinci").top - bounds(&rule, "dar").top, 20.0));
    // BasicTooltipBox: a long press shows the tooltip under its anchor.
    let anchor = bounds(&rule, "çapa");
    rule.host().pointer_down(anchor.center(), -1, 0, PointerType::Touch);
    rule.advance_time_by(600);
    rule.host().pointer_up(anchor.center(), -1, 0);
    rule.advance_frame();
    let tip = bounds(&rule, "ipucu");
    assert!(near(tip.left, anchor.left) && near(tip.top, anchor.bottom), "{tip:?} {anchor:?}");
}

tests![
    layouts_as_compose_writes_them,
    marquee_as_compose_writes_it,
    a_ripple_through_local_indication,
    flow_column_contextual_flow_row_and_a_tooltip_from_a_kt_file,
];
