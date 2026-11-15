//! The test rule's tests: finders and matchers over the semantics trees, selections (children, parent, siblings,
//! ancestors, index), assertions and their Compose messages, actions (clicks, semantics actions, scrolling to a
//! node through the bring-into-view chain), printing. The C++ Kompoz's test_rule_assertions without Material (its
//! buttons, checkbox and icon as semantics boxes, its scrolling column as a scroll-like bring-into-view node).

#![allow(non_snake_case)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::focus::{BoundsProvider, BringIntoViewFuture, BringIntoViewModifierNode};
use kompoz_ui::layout::LayoutCoordinates;
use kompoz_ui::semantics::*;
use kompoz_ui::*;
use std::cell::Cell;
use std::rc::Rc;

/// The message of the panic `f` ends in, if it panics.
fn panic_message(f: impl FnOnce()) -> Option<String> {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    std::panic::set_hook(previous);
    match r {
        Ok(()) => None,
        Err(e) => Some(
            e.downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default(),
        ),
    }
}

#[track_caller]
fn fails(f: impl FnOnce(), message: &str) {
    match panic_message(f) {
        Some(m) => assert!(m.contains(message), "expected a failure with {message:?}, got:\n{m}"),
        None => panic!("expected a failure with {message:?}, it passed"),
    }
}

/// A box scrolling what it wraps up by its state (a stand-in for a scroll container: stage 10's): it brings a
/// rect of what is below it into view by scrolling just far enough (Compose's BringIntoViewModifierNode).
fn scroller(scroll: MutableState<i32>) -> Modifier {
    let s = scroll.clone();
    Modifier.then_element(Scroller { scroll }).layout(move |scope, m, c| {
        let p = m.measure(Constraints::new(c.min_width, c.max_width, 0, Constraints::Infinity));
        let s = s.clone();
        scope.layout(c.max_width, c.max_height, move |pl| pl.place(&p, 0, -s.get()))
    })
}

#[derive(PartialEq)]
struct Scroller {
    scroll: MutableState<i32>,
}

impl ModifierNodeElement for Scroller {
    type Node = ScrollerNode;
    fn create(&self) -> ScrollerNode {
        ScrollerNode { scroll: self.scroll.clone(), handle: None }
    }
    fn update(&self, node: &mut ScrollerNode) {
        node.scroll = self.scroll.clone();
    }
}

struct ScrollerNode {
    scroll: MutableState<i32>,
    handle: Option<NodeHandle>,
}

impl ModifierNode for ScrollerNode {
    node_capabilities!(bring_into_view);
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }
}

impl BringIntoViewModifierNode for ScrollerNode {
    fn bring_into_view(&mut self, child: LayoutCoordinates, bounds: BoundsProvider) -> BringIntoViewFuture {
        let (scroll, handle) = (self.scroll.clone(), self.handle.clone());
        Box::pin(async move {
            let Some(me) = handle.and_then(|h| h.coordinates()) else { return };
            let Some(rect) = bounds() else { return };
            let at = me.local_bounding_box_of(&child, false).top_left();
            let rect = rect.translate(at);
            let height = me.size().height as f32;
            let delta = if rect.bottom > height {
                rect.bottom - height
            } else if rect.top < 0.0 {
                rect.top
            } else {
                0.0
            };
            scroll.set(scroll.peek() + delta.round() as i32);
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Ported from C++

fn test_rule_assertions() {
    // C++'s test_rule_assertions: semantics boxes for its buttons, checkbox and icon; a scroll-like box for its
    // scrolling column. Compose's messages.
    let ticked = mutable_state_of(false);
    let scroll = mutable_state_of(0);
    let (t, sc) = (ticked.clone(), scroll.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (t, sc) = (t.clone(), sc.clone());
            Column(
                Modifier.fill_max_size().then(scroller(sc.clone())),
                content = move || {
                    Box(Modifier.semantics_with(true, |c| c.on_click(|| true)), content = || Text("Tamam"));
                    Box(
                        Modifier.semantics_with(true, |c| {
                            c.on_click(|| true);
                            c.disabled();
                        }),
                        content = || Text("Kapalı"),
                    );
                    let (on, t2) = (t.get(), t.clone());
                    Box(Modifier
                        .size(20.dp())
                        .test_tag("box")
                        .semantics_with(true, move |c| {
                            let t2 = t2.clone();
                            c.set_toggleable_state(ToggleableState::of(on));
                            c.on_click(move || {
                                t2.set(!t2.peek());
                                true
                            });
                        })
                        .pointer_input(on, {
                            let t3 = t.clone();
                            move |s: input::PointerInputScope| {
                                let t3 = t3.clone();
                                async move { s.detect_tap_gestures().on_tap(move |_| t3.set(!t3.peek())).await }
                            }
                        }));
                    Box(Modifier.size(24.dp()).semantics(|c| c.set_content_description("Bilgi")));
                    Text("Tekrar");
                    Text("Tekrar");
                    Box(Modifier.height(1500.dp()));
                    Text("Aşağıda");
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.on_node_with_text("Tamam")
        .assert_is_displayed()
        .assert_has_click_action()
        .assert_is_enabled()
        .assert_text_equals(&["Tamam"]);
    rule.on_node_with_text("Kapalı").assert_has_click_action().assert_is_not_enabled();
    rule.on_node_with_content_description("Bilgi").assert_has_no_click_action();
    rule.on_node_with_text("Yok").assert_does_not_exist();
    rule.on_node_with(has_text("Tam", true, false), false).assert_text_contains_with("ama", true, false);
    fails(
        || {
            rule.on_node_with_text("Yok").assert_exists();
        },
        "Failed: assertExists.\nReason: Expected exactly '1' node but could not find any node that satisfies: \
         (Text + InputText + EditableText contains 'Yok' (ignoreCase: false))",
    );
    fails(
        || {
            rule.on_node_with_text("Tamam").assert_text_equals(&["Vazgeç"]);
        },
        "Failed to assert the following: (Text + EditableText = [Vazgeç])",
    );
    fails(
        || {
            rule.on_node_with_text("Kapalı").assert_is_enabled();
        },
        "Failed to assert the following: (is enabled)",
    );
    rule.on_node_with_tag("box").assert_is_off();
    rule.on_node_with_tag("box").perform_click();
    rule.on_node_with_tag("box").assert_is_on();
    fails(
        || {
            rule.on_node_with_tag("box").assert_is_off();
        },
        "Failed to assert the following: (ToggleableState = 'Off')",
    );
    rule.on_all_nodes_with_text("Tekrar").assert_count_equals(2);
    rule.on_all_nodes_with_text("Tekrar").get(1).assert_is_displayed();
    fails(
        || {
            rule.on_all_nodes_with_text("Tekrar").assert_count_equals(3);
        },
        "Expected '3' nodes but found '2' nodes that satisfy",
    );
    // Below the window: not displayed until scrolled to.
    rule.on_node_with_text("Aşağıda").assert_exists().assert_is_not_displayed();
    fails(
        || {
            rule.on_node_with_text("Aşağıda").assert_is_displayed();
        },
        "Assert failed: The component with Text + InputText + EditableText contains 'Aşağıda' (ignoreCase: false) is \
         not displayed!",
    );
    rule.on_node_with_text("Aşağıda").perform_scroll_to();
    rule.on_node_with_text("Aşağıda").assert_is_displayed();
    assert!(scroll.peek() > 700, "{}", scroll.peek());
    // Nothing above it scrolls: Compose's message.
    let other = KompozTestRule::new(kompozable_lambda!(|| Text("yalnız")));
    fails(
        || {
            other.on_node_with_text("yalnız").perform_scroll_to();
        },
        "Semantic Node has no parent layout with a Scroll SemanticsAction",
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Compose's API

fn selections_follow_the_tree() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            Modifier.test_tag("sütun"),
            content = || {
                Text("bir", modifier = Modifier.test_tag("1"));
                Text("iki", modifier = Modifier.test_tag("2"));
                Text("üç", modifier = Modifier.test_tag("3"));
            },
        );
    }));
    let column = rule.on_node_with_tag("sütun");
    column.on_children().assert_count_equals(3);
    column.on_child_at(1).assert_text_equals(&["iki"]);
    column.on_children().on_first().assert_text_equals(&["bir"]);
    column.on_children().on_last().assert_text_equals(&["üç"]);
    column.on_children().filter(has_text("ki", true, false)).assert_count_equals(1);
    column.on_children().filter_to_one(has_test_tag("3")).assert_text_equals(&["üç"]);
    rule.on_node_with_tag("2").on_parent().assert(has_test_tag("sütun"));
    rule.on_node_with_tag("2").on_siblings().assert_count_equals(2).assert_all(!has_test_tag("2"));
    rule.on_node_with_tag("2").on_ancestors().assert_any(is_root());
    column.on_children().assert_any(has_text("üç", false, false));
    fails(
        || {
            column.on_children().assert_all(has_text("bir", false, false));
        },
        "Failed to assertAll(Text + InputText + EditableText contains 'bir' (ignoreCase: false))\nFound '2' nodes not matching:",
    );
    fails(
        || {
            column.on_child_at(5).assert_exists();
        },
        "Can't retrieve node at index '5' of '(TestTag = 'sütun').children'",
    );
    fails(
        || {
            column.on_child().assert_exists();
        },
        "Expected exactly '1' node but found '3' nodes that satisfy: ((TestTag = 'sütun').child)",
    );
    fails(
        || {
            rule.on_node_with(has_text("i", true, false), false).assert_exists();
        },
        "Expected exactly '1' node but found '2' nodes",
    );
    // Matchers combined.
    rule.on_node(has_text("bir", false, false).and(has_test_tag("1"))).assert_exists();
    rule.on_all_nodes(has_text("bir", false, false).or(has_test_tag("2"))).assert_count_equals(2);
    rule.on_node(has_any_sibling(has_test_tag("1")).and(has_test_tag("3"))).assert_exists();
    rule.on_node(has_any_child(has_test_tag("1"))).assert(has_test_tag("sütun"));
    rule.on_node(has_any_ancestor(has_test_tag("sütun")).and(has_text("üç", false, false))).assert_exists();
    assert_eq!(
        has_text("a", false, true).and(!is_root()).description,
        "(Text + InputText + EditableText contains 'a' (ignoreCase: true)) && (NOT (isRoot))"
    );
    // Ignoring case.
    // (as Kotlin's: characters equal if their upper or their lower cases are; İ's lower case is i)
    rule.on_node_with(has_text("BİR", false, true), false).assert_exists();
    rule.on_node_with(has_text("BİR", false, false), false).assert_does_not_exist();
}

fn printing_shows_the_tree_as_compose_does() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            Modifier.test_tag("kök").semantics_with(true, |c| c.on_click(|| true)),
            content = || {
                Text("bir");
                Box(Modifier.size(10.dp()).semantics(|c| c.heading()));
            },
        );
    }));
    let printed = rule.on_node_with_tag("kök").print_to_string();
    assert!(printed.starts_with("Printing with useUnmergedTree = 'false'\nNode #"), "{printed}");
    assert!(
        printed.contains(", Tag: 'kök'")
            && printed.contains("Actions = [OnClick]")
            && printed.contains("MergeDescendants = 'true'"),
        "{printed}"
    );
    let unmerged = rule.on_node_with_tag_unmerged("kök").print_to_string();
    assert!(unmerged.contains(" |-Node #") && unmerged.contains("[Heading]"), "{unmerged}");
    let message = panic_message(|| {
        rule.on_node_with_tag("kök").assert(has_state_description("yok"));
    })
    .unwrap();
    assert!(
        message.contains("Semantics of the node:\nNode #") && message.contains("Selector used: (TestTag = 'kök')"),
        "{message}"
    );
}

fn semantics_actions_run_through_the_rule() {
    let clicks = Rc::new(Cell::new(0));
    let progress = Rc::new(Cell::new(0.0f32));
    let (c, p) = (clicks.clone(), progress.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (c, p) = (c.clone(), p.clone());
        Box(Modifier.size(30.dp()).test_tag("x").semantics(move |cfg| {
            let (c, p) = (c.clone(), p.clone());
            cfg.on_click(move || {
                c.set(c.get() + 1);
                true
            });
            cfg.set_progress(move |v| {
                p.set(v);
                true
            });
            cfg.set_custom_actions(vec![CustomAccessibilityAction::new("özel", || true)]);
        }));
    }));
    rule.on_node_with_tag("x").perform_semantics_action(SemanticsActions::OnClick);
    assert_eq!(clicks.get(), 1);
    rule.on_node_with_tag("x").perform_semantics_action_with(SemanticsActions::SetProgress, |f| {
        f(0.5);
    });
    assert_eq!(progress.get(), 0.5);
    rule.on_node_with_tag("x").perform_custom_accessibility_action_with_label("özel");
    fails(
        || {
            rule.on_node_with_tag("x").perform_semantics_action(SemanticsActions::Dismiss);
        },
        "Failed to perform action Dismiss, the node is missing [Dismiss]",
    );
    fails(
        || {
            rule.on_node_with_tag("x").request_focus();
        },
        "the node is missing [RequestFocus]",
    );
    // Touch input and clicks land where the node is, a popup's too (its coordinates are the window's).
    rule.on_node_with_tag("x").perform_click();
}

tests!(
    test_rule_assertions,
    selections_follow_the_tree,
    printing_shows_the_tree_as_compose_does,
    semantics_actions_run_through_the_rule,
);
