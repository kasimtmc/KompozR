//! Interactions with the nodes finders find (Compose's SemanticsNodeInteraction, SemanticsNodeInteractionCollection,
//! SemanticsSelector, Selectors.kt, Assertions.kt, Actions.kt and Output.kt): selections chained from a matcher,
//! assertions panicking with Compose's messages, actions, and the tree printed for failures.

use crate::KompozTestRule;
use crate::matcher::{self, SemanticsMatcher, ancestors};
use kompoz_graphics::{Dp, Offset, Rect};
use kompoz_ui::semantics::{
    AccessibilityAction, ActionFn, ProgressBarRangeInfo, SemanticsActions, SemanticsConfiguration, SemanticsNode,
    SemanticsProperties, SemanticsPropertyKey,
};
use std::fmt;
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// Selectors

/// What a selection came to (Compose's SelectionResult): its nodes, or why none (an index out of range).
pub struct SelectionResult {
    pub selected_nodes: Vec<SemanticsNode>,
    pub custom_error_on_no_match: Option<String>,
}

type SelectFn = Rc<dyn Fn(Vec<SemanticsNode>) -> SelectionResult>;

/// A chain of selections from all the nodes (Compose's SemanticsSelector): a matcher's, then children, parent,
/// index… of what the one before selected.
#[derive(Clone)]
pub struct SemanticsSelector {
    pub description: String,
    requires_exactly_one_node: bool,
    chained: Option<Rc<SemanticsSelector>>,
    selector: SelectFn,
}

impl SemanticsSelector {
    /// The nodes `matcher` matches.
    pub fn of(matcher: SemanticsMatcher) -> SemanticsSelector {
        SemanticsSelector {
            description: matcher.description.clone(),
            requires_exactly_one_node: false,
            chained: None,
            selector: Rc::new(move |nodes| SelectionResult {
                selected_nodes: nodes.into_iter().filter(|n| matcher.matches(n)).collect(),
                custom_error_on_no_match: None,
            }),
        }
    }

    /// Selects from `nodes` (Compose's `map`); panics if a step needing one node got another count.
    #[track_caller]
    pub fn map(&self, nodes: Vec<SemanticsNode>, error_on_fail: &str) -> SelectionResult {
        match self.try_map(nodes, error_on_fail) {
            Ok(r) => r,
            Err(message) => panic!("{message}"),
        }
    }

    /// The same, the failure given back.
    pub fn try_map(&self, nodes: Vec<SemanticsNode>, error_on_fail: &str) -> Result<SelectionResult, String> {
        let chained = match &self.chained {
            Some(c) => Some(c.try_map(nodes.clone(), error_on_fail)?),
            None => None,
        };
        let input = match &chained {
            Some(r) => r.selected_nodes.clone(),
            None => nodes,
        };
        if self.requires_exactly_one_node && input.len() != 1 {
            let message = chained.and_then(|r| r.custom_error_on_no_match).unwrap_or_else(|| {
                let selector = self.chained.as_deref().unwrap_or(self);
                count_mismatch(error_on_fail, Some(selector), &input, 1, &[])
            });
            return Err(message);
        }
        Ok((self.selector)(input))
    }

    fn chain(&self, description: String, exactly_one: bool, selector: SelectFn) -> SemanticsSelector {
        SemanticsSelector {
            description,
            requires_exactly_one_node: exactly_one,
            chained: Some(Rc::new(self.clone())),
            selector,
        }
    }

    fn select_from_one(&self, what: &str, f: impl Fn(&SemanticsNode) -> Vec<SemanticsNode> + 'static) -> Self {
        self.chain(
            format!("({}).{what}", self.description),
            true,
            Rc::new(move |nodes| SelectionResult { selected_nodes: f(&nodes[0]), custom_error_on_no_match: None }),
        )
    }

    fn index(&self, index: usize) -> SemanticsSelector {
        let me = self.clone();
        self.chain(
            format!("({})[{index}]", self.description),
            false,
            Rc::new(move |nodes| {
                if index < nodes.len() {
                    SelectionResult { selected_nodes: vec![nodes[index].clone()], custom_error_on_no_match: None }
                } else {
                    let message = index_error(index, &me, &nodes);
                    SelectionResult { selected_nodes: Vec::new(), custom_error_on_no_match: Some(message) }
                }
            }),
        )
    }

    fn last(&self) -> SemanticsSelector {
        self.chain(
            format!("({}).last", self.description),
            false,
            Rc::new(|nodes| SelectionResult {
                selected_nodes: nodes.last().cloned().into_iter().collect(),
                custom_error_on_no_match: None,
            }),
        )
    }

    fn via_matcher(&self, name: &str, matcher: SemanticsMatcher) -> SemanticsSelector {
        self.chain(
            format!("({}).{name}({})", self.description, matcher.description),
            false,
            Rc::new(move |nodes| SelectionResult {
                selected_nodes: nodes.into_iter().filter(|n| matcher.matches(n)).collect(),
                custom_error_on_no_match: None,
            }),
        )
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Error messages (Compose's ErrorMessages.kt)

fn count_mismatch(
    error_message: &str,
    selector: Option<&SemanticsSelector>,
    found: &[SemanticsNode],
    expected: usize,
    found_unmerged: &[SemanticsNode],
) -> String {
    let mut sb = String::new();
    sb.push_str(error_message);
    sb.push_str("\nReason: ");
    match expected {
        0 => sb.push_str("Did not expect any node"),
        1 => sb.push_str("Expected exactly '1' node"),
        n => sb.push_str(&format!("Expected '{n}' nodes")),
    }
    if found.is_empty() {
        sb.push_str(" but could not find any");
    } else {
        sb.push_str(&format!(" but found '{}'", found.len()));
    }
    match selector {
        Some(s) if found.len() <= 1 => sb.push_str(&format!(" node that satisfies: ({})", s.description)),
        Some(s) => sb.push_str(&format!(" nodes that satisfy: ({})", s.description)),
        None => sb.push('.'),
    }
    if found.is_empty() && !found_unmerged.is_empty() {
        sb.push_str("\nHowever, the unmerged tree contains ");
        if found_unmerged.len() == 1 {
            sb.push_str("'1' node that matches. ");
        } else {
            sb.push_str(&format!("'{}' nodes that match. ", found_unmerged.len()));
        }
        sb.push_str("Are you missing `useUnmergedNode = true` in your finder?");
    }
    sb.push('\n');
    if !found.is_empty() {
        sb.push_str(if found.len() == 1 { "Node found:\n" } else { "Nodes found:\n" });
        sb.push_str(&print_nodes(found, 0));
        sb.push('\n');
    }
    sb
}

fn general_error(error_message: &str, selector: &SemanticsSelector, node: &SemanticsNode) -> String {
    format!(
        "{error_message}\nSemantics of the node:\n{}\nSelector used: ({})\n",
        print_node(node, 0),
        selector.description
    )
}

fn index_error(index: usize, selector: &SemanticsSelector, nodes: &[SemanticsNode]) -> String {
    let mut sb = format!("Can't retrieve node at index '{index}' of '{}'\n", selector.description);
    match nodes.len() {
        0 => sb.push_str("There are no existing nodes for that selector.\n"),
        1 => sb.push_str(&format!("There is 1 node only:\n{}\n", print_nodes(nodes, 0))),
        n => sb.push_str(&format!("There are '{n}' nodes only:\n{}\n", print_nodes(nodes, 0))),
    }
    sb
}

// ---------------------------------------------------------------------------------------------------------------
// Printing (Compose's Output.kt)

/// Nodes printed, numbered if several (Compose's `Collection<SemanticsNode>.printToString`).
pub fn print_nodes(nodes: &[SemanticsNode], max_depth: usize) -> String {
    let mut sb = String::new();
    for (i, n) in nodes.iter().enumerate() {
        if nodes.len() > 1 {
            sb.push_str(&format!("{}) ", i + 1));
        }
        sb.push_str(&print_node(n, max_depth));
        if i + 1 < nodes.len() {
            sb.push('\n');
        }
    }
    sb
}

/// A node and its subtree to `max_depth` printed (Compose's `SemanticsNode.printToString`).
pub fn print_node(node: &SemanticsNode, max_depth: usize) -> String {
    let mut sb = String::new();
    print_inner(node, &mut sb, max_depth, 0, "", false);
    sb
}

fn print_inner(node: &SemanticsNode, sb: &mut String, max_depth: usize, level: usize, indent: &str, followed: bool) {
    let new_indent = if level == 0 {
        String::new()
    } else if followed {
        format!("{indent} | ")
    } else {
        format!("{indent}   ")
    };
    if level > 0 {
        sb.push_str(&format!("{indent} |-"));
    }
    let at = node.position_in_window();
    let size = node.size();
    let bounds = Rect::new(at.x, at.y, at.x + size.width as f32, at.y + size.height as f32);
    sb.push_str(&format!(
        "Node #{} at (l={}, t={}, r={}, b={})px",
        node.id(),
        bounds.left,
        bounds.top,
        bounds.right,
        bounds.bottom
    ));
    let config = node.config();
    if let Some(tag) = config.get(SemanticsProperties::TestTag) {
        sb.push_str(&format!(", Tag: '{tag}'"));
    }
    append_config(sb, &config, &new_indent);
    if level == max_depth {
        let children = node.children().len();
        let siblings = node.parent().map_or(1, |p| p.children().len()) - 1;
        if children > 0 || (siblings > 0 && level == 0) {
            sb.push('\n');
            sb.push_str(&new_indent);
            sb.push_str("Has ");
            if children > 1 {
                sb.push_str(&format!("{children} children"));
            } else if children == 1 {
                sb.push_str("1 child");
            }
            if siblings > 0 && level == 0 {
                if children > 0 {
                    sb.push_str(", ");
                }
                if siblings > 1 {
                    sb.push_str(&format!("{siblings} siblings"));
                } else {
                    sb.push_str("1 sibling");
                }
            }
        }
        return;
    }
    let children = node.children();
    for (i, child) in children.iter().enumerate() {
        sb.push('\n');
        print_inner(child, sb, max_depth, level + 1, &new_indent, i + 1 < children.len());
    }
}

fn append_config(sb: &mut String, config: &SemanticsConfiguration, indent: &str) {
    let mut entries = Vec::new();
    config.for_each_printed(|name, value| entries.push((name, value)));
    entries.sort_by_key(|(name, _)| *name);
    let mut actions = Vec::new();
    let mut units = Vec::new();
    for (name, value) in entries {
        if name == "TestTag" {
            continue;
        }
        if value.starts_with("AccessibilityAction(") || value == "Function" {
            actions.push(name);
            continue;
        }
        if value == "()" {
            units.push(name);
            continue;
        }
        sb.push('\n');
        sb.push_str(indent);
        sb.push_str(&format!("{name} = '{value}'"));
    }
    if !units.is_empty() {
        sb.push('\n');
        sb.push_str(indent);
        sb.push_str(&format!("[{}]", units.join(", ")));
    }
    if !actions.is_empty() {
        sb.push('\n');
        sb.push_str(indent);
        sb.push_str(&format!("Actions = [{}]", actions.join(", ")));
    }
    if config.is_merging_semantics_of_descendants {
        sb.push('\n');
        sb.push_str(indent);
        sb.push_str("MergeDescendants = 'true'");
    }
    if config.is_clearing_semantics {
        sb.push('\n');
        sb.push_str(indent);
        sb.push_str("ClearAndSetSemantics = 'true'");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// One node

/// A node a finder found, to assert about and act on (Compose's SemanticsNodeInteraction): looked up anew each
/// time, so it follows the tree as it changes.
#[derive(Clone)]
pub struct SemanticsNodeInteraction<'a> {
    pub(crate) rule: &'a KompozTestRule,
    pub(crate) selector: SemanticsSelector,
    pub(crate) use_unmerged_tree: bool,
}

impl fmt::Debug for SemanticsNodeInteraction<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SemanticsNodeInteraction({})", self.selector.description)
    }
}

impl<'a> SemanticsNodeInteraction<'a> {
    pub(crate) fn new(rule: &'a KompozTestRule, selector: SemanticsSelector, use_unmerged_tree: bool) -> Self {
        SemanticsNodeInteraction { rule, selector, use_unmerged_tree }
    }

    /// Its selector's description.
    pub fn description(&self) -> &str {
        &self.selector.description
    }

    #[track_caller]
    fn select(&self, error_message: &str) -> SelectionResult {
        self.selector.map(self.rule.all_nodes(self.use_unmerged_tree), error_message)
    }

    /// The node, or why there isn't exactly one (Compose's `fetchSemanticsNode`).
    #[track_caller]
    pub fn fetch_semantics_node(&self) -> SemanticsNode {
        self.fetch_with("Failed: assertExists.")
    }

    #[track_caller]
    fn fetch_with(&self, error_message: &str) -> SemanticsNode {
        let result = self.select(error_message);
        if result.selected_nodes.len() == 1 {
            return result.selected_nodes.into_iter().next().unwrap();
        }
        if let Some(custom) = result.custom_error_on_no_match {
            panic!("{error_message}\n{custom}");
        }
        let unmerged = if self.use_unmerged_tree {
            Vec::new()
        } else {
            self.selector.map(self.rule.all_nodes(true), error_message).selected_nodes
        };
        panic!("{}", count_mismatch(error_message, Some(&self.selector), &result.selected_nodes, 1, &unmerged));
    }

    /// Whether exactly one node matches.
    pub fn exists(&self) -> bool {
        self.selector
            .try_map(self.rule.all_nodes(self.use_unmerged_tree), "")
            .is_ok_and(|r| r.selected_nodes.len() == 1)
    }

    #[track_caller]
    pub fn assert_exists(&self) -> &Self {
        self.fetch_semantics_node();
        self
    }

    #[track_caller]
    pub fn assert_does_not_exist(&self) -> &Self {
        let message = "Failed: assertDoesNotExist.";
        let found = self.select(message).selected_nodes;
        if !found.is_empty() {
            panic!("{}", count_mismatch(message, Some(&self.selector), &found, 0, &[]));
        }
        self
    }

    /// Asserts `matcher` holds for the node (Compose's `assert`).
    #[track_caller]
    pub fn assert(&self, matcher: SemanticsMatcher) -> &Self {
        let message = format!("Failed to assert the following: ({})", matcher.description);
        let node = self.fetch_with(&message);
        if !matcher.matches(&node) {
            panic!("{}", general_error(&message, &self.selector, &node));
        }
        self
    }

    /// Whether it is displayed (Compose's `isDisplayed`): its layout and those above it placed, its bounds in the
    /// window (cut by clips and the window) not empty.
    pub fn is_displayed(&self) -> bool {
        self.check_is_displayed(false)
    }

    /// Whether it is not displayed fully (Compose's `isNotDisplayed`).
    pub fn is_not_displayed(&self) -> bool {
        !self.check_is_displayed(true)
    }

    #[track_caller]
    fn check_is_displayed(&self, fully: bool) -> bool {
        let nodes = self.select("").selected_nodes;
        if nodes.is_empty() {
            return false;
        }
        if nodes.len() > 1 {
            panic!(
                "Failed to perform checkIsDisplayed check: Expected at most 1 node but found {} nodes that \
                 satisfy ({})",
                nodes.len(),
                self.selector.description
            );
        }
        let node = &nodes[0];
        let mut layout = Some(node.layout_node().clone());
        while let Some(l) = layout {
            if !l.is_placed() {
                return false;
            }
            layout = l.parent();
        }
        let clipped = node.bounds_in_window();
        if clipped.width() <= 0.0 || clipped.height() <= 0.0 {
            return false;
        }
        if fully {
            let window = self.rule.host().viewport_size();
            let unclipped = node.unclipped_bounds_in_root();
            return clipped == unclipped
                && unclipped.left >= 0.0
                && unclipped.top >= 0.0
                && unclipped.right <= window.width as f32
                && unclipped.bottom <= window.height as f32;
        }
        true
    }

    /// Asserts it is displayed (Compose's `assertIsDisplayed`): in the window and not clipped away.
    #[track_caller]
    pub fn assert_is_displayed(&self) -> &Self {
        if !self.is_displayed() {
            panic!("Assert failed: The component with {} is not displayed!", self.selector.description);
        }
        self
    }

    /// Asserts it is not (fully) displayed (Compose's `assertIsNotDisplayed`).
    #[track_caller]
    pub fn assert_is_not_displayed(&self) -> &Self {
        if !self.is_not_displayed() {
            panic!("Assert failed: The component with {} is displayed!", self.selector.description);
        }
        self
    }

    #[track_caller]
    pub fn assert_is_enabled(&self) -> &Self {
        self.assert(matcher::is_enabled())
    }
    #[track_caller]
    pub fn assert_is_not_enabled(&self) -> &Self {
        self.assert(matcher::is_not_enabled())
    }
    #[track_caller]
    pub fn assert_is_on(&self) -> &Self {
        self.assert(matcher::is_on())
    }
    #[track_caller]
    pub fn assert_is_off(&self) -> &Self {
        self.assert(matcher::is_off())
    }
    #[track_caller]
    pub fn assert_is_selected(&self) -> &Self {
        self.assert(matcher::is_selected())
    }
    #[track_caller]
    pub fn assert_is_not_selected(&self) -> &Self {
        self.assert(matcher::is_not_selected())
    }
    #[track_caller]
    pub fn assert_is_toggleable(&self) -> &Self {
        self.assert(matcher::is_toggleable())
    }
    #[track_caller]
    pub fn assert_is_selectable(&self) -> &Self {
        self.assert(matcher::is_selectable())
    }
    #[track_caller]
    pub fn assert_is_focused(&self) -> &Self {
        self.assert(matcher::is_focused())
    }
    #[track_caller]
    pub fn assert_is_not_focused(&self) -> &Self {
        self.assert(matcher::is_not_focused())
    }
    #[track_caller]
    pub fn assert_has_click_action(&self) -> &Self {
        self.assert(matcher::has_click_action())
    }
    #[track_caller]
    pub fn assert_has_no_click_action(&self) -> &Self {
        self.assert(matcher::has_no_click_action())
    }
    /// Its state description is `value` (Compose's `assertValueEquals`).
    #[track_caller]
    pub fn assert_value_equals(&self, value: &str) -> &Self {
        self.assert(matcher::has_state_description(value))
    }
    #[track_caller]
    pub fn assert_range_info_equals(&self, value: ProgressBarRangeInfo) -> &Self {
        self.assert(matcher::has_progress_bar_range_info(value))
    }
    /// Its content descriptions are exactly `values` (Compose's `assertContentDescriptionEquals`).
    #[track_caller]
    pub fn assert_content_description_equals(&self, values: &[&str]) -> &Self {
        self.assert(matcher::has_content_description_exactly(values))
    }
    /// Its texts (and its field's shown text) are exactly `values` (Compose's `assertTextEquals`).
    #[track_caller]
    pub fn assert_text_equals_with(&self, values: &[&str], include_editable_text: bool) -> &Self {
        self.assert(matcher::has_text_exactly(values, include_editable_text))
    }
    /// One of its texts is `value` (or contains it, with `substring`) (Compose's `assertTextContains`).
    #[track_caller]
    pub fn assert_text_contains_with(&self, value: &str, substring: bool, ignore_case: bool) -> &Self {
        self.assert(matcher::has_text(value, substring, ignore_case))
    }
    /// One of its content descriptions is `value` (or contains it) (Compose's `assertContentDescriptionContains`).
    #[track_caller]
    pub fn assert_content_description_contains_with(&self, value: &str, substring: bool, ignore_case: bool) -> &Self {
        self.assert(matcher::has_content_description(value, substring, ignore_case))
    }

    // ---- Bounds (Compose's BoundsAssertions) ----

    /// Its bounds in the root, not cut by clipping layers (Compose's `getUnclippedBoundsInRoot`), px.
    #[track_caller]
    pub fn get_unclipped_bounds_in_root(&self) -> Rect {
        self.fetch_semantics_node().unclipped_bounds_in_root()
    }

    /// Its bounds in the root, cut by clipping layers (Compose's `getBoundsInRoot`), px.
    #[track_caller]
    pub fn get_bounds_in_root(&self) -> Rect {
        self.fetch_semantics_node().bounds_in_root()
    }

    /// The layout node it is.
    #[track_caller]
    pub fn layout_node(&self) -> Rc<kompoz_ui::LayoutNode> {
        self.fetch_semantics_node().layout_node().clone()
    }

    fn density(&self) -> f32 {
        self.rule.density().density
    }

    #[track_caller]
    fn assert_close(&self, what: &str, actual_px: f32, expected: Dp) {
        let actual = actual_px / self.density();
        assert!(
            (actual - expected.0).abs() <= 0.5,
            "Actual {what} is {actual}.dp, expected {}.dp (tolerance: 0.5.dp) for ({})",
            expected.0,
            self.selector.description
        );
    }

    /// Its width is `expected` (dp), within half a dp (Compose's `assertWidthIsEqualTo`).
    #[track_caller]
    pub fn assert_width_is_equal_to(&self, expected: Dp) -> &Self {
        let b = self.get_unclipped_bounds_in_root();
        self.assert_close("width", b.width(), expected);
        self
    }

    #[track_caller]
    pub fn assert_height_is_equal_to(&self, expected: Dp) -> &Self {
        let b = self.get_unclipped_bounds_in_root();
        self.assert_close("height", b.height(), expected);
        self
    }

    /// Its top left in the root is (`left`, `top`) dp (Compose's `assertPositionInRootIsEqualTo`).
    #[track_caller]
    pub fn assert_position_in_root_is_equal_to(&self, left: Dp, top: Dp) -> &Self {
        let b = self.get_unclipped_bounds_in_root();
        self.assert_close("left", b.left, left);
        self.assert_close("top", b.top, top);
        self
    }

    #[track_caller]
    pub fn assert_left_position_in_root_is_equal_to(&self, left: Dp) -> &Self {
        self.assert_close("left", self.get_unclipped_bounds_in_root().left, left);
        self
    }

    #[track_caller]
    pub fn assert_top_position_in_root_is_equal_to(&self, top: Dp) -> &Self {
        self.assert_close("top", self.get_unclipped_bounds_in_root().top, top);
        self
    }

    /// Its position in the root, px.
    #[track_caller]
    pub fn position_in_root(&self) -> Offset {
        self.fetch_semantics_node().position_in_root()
    }

    // ---- Selections (Compose's Selectors.kt) ----

    /// Its parent (Compose's `onParent`).
    pub fn on_parent(&self) -> SemanticsNodeInteraction<'a> {
        let s = self.selector.select_from_one("parent", |n| n.parent().into_iter().collect());
        SemanticsNodeInteraction::new(self.rule, s, self.use_unmerged_tree)
    }

    /// Its children (Compose's `onChildren`).
    pub fn on_children(&self) -> SemanticsNodeInteractionCollection<'a> {
        let s = self.selector.select_from_one("children", |n| n.children());
        SemanticsNodeInteractionCollection::new(self.rule, s, self.use_unmerged_tree)
    }

    /// Its only child (Compose's `onChild`).
    pub fn on_child(&self) -> SemanticsNodeInteraction<'a> {
        let s = self.selector.select_from_one("child", |n| n.children());
        SemanticsNodeInteraction::new(self.rule, s, self.use_unmerged_tree)
    }

    /// Its child at `index` (Compose's `onChildAt`).
    pub fn on_child_at(&self, index: usize) -> SemanticsNodeInteraction<'a> {
        self.on_children().get(index)
    }

    /// Its siblings (Compose's `onSiblings`).
    pub fn on_siblings(&self) -> SemanticsNodeInteractionCollection<'a> {
        let s = self.selector.select_from_one("siblings", siblings);
        SemanticsNodeInteractionCollection::new(self.rule, s, self.use_unmerged_tree)
    }

    /// Its only sibling (Compose's `onSibling`).
    pub fn on_sibling(&self) -> SemanticsNodeInteraction<'a> {
        let s = self.selector.select_from_one("sibling", siblings);
        SemanticsNodeInteraction::new(self.rule, s, self.use_unmerged_tree)
    }

    /// The nodes above it, nearest first (Compose's `onAncestors`).
    pub fn on_ancestors(&self) -> SemanticsNodeInteractionCollection<'a> {
        let s = self.selector.select_from_one("ancestors", ancestors);
        SemanticsNodeInteractionCollection::new(self.rule, s, self.use_unmerged_tree)
    }

    // ---- Actions (Compose's Actions.kt) ----

    /// Clicks it: a finger tap at its center (Compose's `performClick` on Android: `performTouchInput { click() }`),
    /// then the rule run until idle.
    #[track_caller]
    pub fn perform_click(&self) -> &Self {
        self.perform_touch_input(|s| s.click());
        self.rule.wait_for_idle();
        self
    }

    /// Brings it into view through the bring-into-view nodes above it (Compose's `performScrollTo`; here by the
    /// scroll containers' bring-into-view chain), the rule run until idle. Panics if nothing above it scrolls.
    #[track_caller]
    pub fn perform_scroll_to(&self) -> &Self {
        let node = self.fetch_with("Failed to perform performScrollTo.");
        let Some(future) = kompoz_ui::focus::bring_layout_into_view(node.layout_node(), None) else {
            panic!(
                "{}",
                general_error(
                    "Semantic Node has no parent layout with a Scroll SemanticsAction",
                    &self.selector,
                    &node
                )
            );
        };
        kompoz_coroutines::CoroutineScope::new(self.rule.host().dispatcher()).launch(future);
        self.rule.wait_for_idle();
        self
    }

    /// Performs its action `key` (one taking nothing) (Compose's `performSemanticsAction(key)`), the rule run until
    /// idle. Panics if it has none.
    #[track_caller]
    pub fn perform_semantics_action(&self, key: SemanticsPropertyKey<AccessibilityAction<ActionFn>>) -> &Self {
        self.perform_semantics_action_with(key, |f| {
            f();
        })
    }

    /// Performs its action `key` through `invocation`, which calls it with what it takes (Compose's
    /// `performSemanticsAction(key, invocation)`), the rule run until idle.
    #[track_caller]
    pub fn perform_semantics_action_with<F: ?Sized + 'static>(
        &self,
        key: SemanticsPropertyKey<AccessibilityAction<F>>,
        invocation: impl FnOnce(&F),
    ) -> &Self {
        let node = self.fetch_with(&format!("Failed to perform {} action.", key.name()));
        let config = node.config();
        let Some(action) = config.get(key) else {
            let message = format!("Failed to perform action {}, the node is missing [{}]", key.name(), key.name());
            panic!("{}", general_error(&message, &self.selector, &node));
        };
        if let Some(f) = action.action {
            invocation(&f);
        }
        self.rule.wait_for_idle();
        self
    }

    /// Performs its custom accessibility action labelled `label` (Compose's
    /// `performCustomAccessibilityActionWithLabel`).
    #[track_caller]
    pub fn perform_custom_accessibility_action_with_label(&self, label: &str) -> &Self {
        let node = self.fetch_with("Failed to perform custom accessibility action.");
        let actions = node.config().get(SemanticsActions::CustomActions).unwrap_or_default();
        let matching: Vec<_> = actions.iter().filter(|a| a.label == label).collect();
        match matching.len() {
            0 => panic!(
                "{}",
                general_error(
                    &format!("No custom accessibility actions matched [label is \"{label}\"]."),
                    &self.selector,
                    &node
                )
            ),
            1 => {
                (matching[0].action)();
            }
            _ => panic!(
                "{}",
                general_error(
                    &format!(
                        "Expected exactly one custom accessibility action to match [label is \"{label}\"], but found {}.",
                        matching.len()
                    ),
                    &self.selector,
                    &node
                )
            ),
        }
        self.rule.wait_for_idle();
        self
    }

    /// Asks for the focus through its `RequestFocus` action (Compose's `requestFocus`).
    #[track_caller]
    pub fn request_focus(&self) -> &Self {
        self.perform_semantics_action(SemanticsActions::RequestFocus)
    }

    // ---- Output ----

    /// It and its subtree to `max_depth` printed (Compose's `printToString`).
    #[track_caller]
    pub fn print_to_string_depth(&self, max_depth: usize) -> String {
        let node = self.fetch_semantics_node();
        format!("Printing with useUnmergedTree = '{}'\n{}", self.use_unmerged_tree, print_node(&node, max_depth))
    }

    /// The same, printed to the log (stderr) under `tag` (Compose's `printToLog`).
    #[track_caller]
    pub fn print_to_log_depth(&self, tag: &str, max_depth: usize) {
        eprintln!("{tag}: printToLog:\n{}", self.print_to_string_depth(max_depth));
    }
}

impl SemanticsNodeInteraction<'_> {
    /// Its texts (and its field's shown text) are exactly `values` (Compose's `assertTextEquals(vararg values)`).
    #[track_caller]
    pub fn assert_text_equals(&self, values: &[&str]) -> &Self {
        self.assert_text_equals_with(values, true)
    }

    /// One of its texts is `value` (Compose's `assertTextContains(value)`).
    #[track_caller]
    pub fn assert_text_contains(&self, value: &str) -> &Self {
        self.assert_text_contains_with(value, false, false)
    }

    /// One of its content descriptions is `value` (Compose's `assertContentDescriptionContains(value)`).
    #[track_caller]
    pub fn assert_content_description_contains(&self, value: &str) -> &Self {
        self.assert_content_description_contains_with(value, false, false)
    }

    /// It and its whole subtree printed (Compose's `printToString()`).
    #[track_caller]
    pub fn print_to_string(&self) -> String {
        self.print_to_string_depth(usize::MAX)
    }

    /// The same to the log (Compose's `printToLog(tag)`).
    #[track_caller]
    pub fn print_to_log(&self, tag: &str) {
        self.print_to_log_depth(tag, usize::MAX)
    }
}

fn siblings(node: &SemanticsNode) -> Vec<SemanticsNode> {
    let id = node.id();
    node.parent().map(|p| p.children().into_iter().filter(|c| c.id() != id).collect()).unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------------------------
// Several nodes

/// The nodes a finder found (Compose's SemanticsNodeInteractionCollection): looked up anew each time.
#[derive(Clone)]
pub struct SemanticsNodeInteractionCollection<'a> {
    pub(crate) rule: &'a KompozTestRule,
    pub(crate) selector: SemanticsSelector,
    pub(crate) use_unmerged_tree: bool,
}

impl fmt::Debug for SemanticsNodeInteractionCollection<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SemanticsNodeInteractionCollection({})", self.selector.description)
    }
}

impl<'a> SemanticsNodeInteractionCollection<'a> {
    pub(crate) fn new(rule: &'a KompozTestRule, selector: SemanticsSelector, use_unmerged_tree: bool) -> Self {
        SemanticsNodeInteractionCollection { rule, selector, use_unmerged_tree }
    }

    /// The nodes (Compose's `fetchSemanticsNodes`).
    #[track_caller]
    pub fn fetch_semantics_nodes(&self) -> Vec<SemanticsNode> {
        self.selector.map(self.rule.all_nodes(self.use_unmerged_tree), "").selected_nodes
    }

    /// How many there are.
    #[track_caller]
    pub fn len(&self) -> usize {
        self.fetch_semantics_nodes().len()
    }

    #[track_caller]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The one at `index` (Compose's `get`, its `[index]`).
    pub fn get(&self, index: usize) -> SemanticsNodeInteraction<'a> {
        SemanticsNodeInteraction::new(self.rule, self.selector.index(index), self.use_unmerged_tree)
    }

    /// The first (Compose's `onFirst`).
    pub fn on_first(&self) -> SemanticsNodeInteraction<'a> {
        self.get(0)
    }

    /// The last (Compose's `onLast`).
    pub fn on_last(&self) -> SemanticsNodeInteraction<'a> {
        SemanticsNodeInteraction::new(self.rule, self.selector.last(), self.use_unmerged_tree)
    }

    /// Those `matcher` matches (Compose's `filter`).
    pub fn filter(&self, matcher: SemanticsMatcher) -> SemanticsNodeInteractionCollection<'a> {
        SemanticsNodeInteractionCollection::new(
            self.rule,
            self.selector.via_matcher("filter", matcher),
            self.use_unmerged_tree,
        )
    }

    /// The one `matcher` matches (Compose's `filterToOne`).
    pub fn filter_to_one(&self, matcher: SemanticsMatcher) -> SemanticsNodeInteraction<'a> {
        SemanticsNodeInteraction::new(
            self.rule,
            self.selector.via_matcher("filterToOne", matcher),
            self.use_unmerged_tree,
        )
    }

    /// Asserts there are `expected` (Compose's `assertCountEquals`).
    #[track_caller]
    pub fn assert_count_equals(&self, expected: usize) -> &Self {
        let message = "Failed to assert count of nodes.";
        let found = self.selector.map(self.rule.all_nodes(self.use_unmerged_tree), message).selected_nodes;
        if found.len() != expected {
            panic!("{}", count_mismatch(message, Some(&self.selector), &found, expected, &[]));
        }
        self
    }

    /// Asserts `matcher` holds for one of them at least (Compose's `assertAny`).
    #[track_caller]
    pub fn assert_any(&self, matcher: SemanticsMatcher) -> &Self {
        let message = format!("Failed to assertAny({})", matcher.description);
        let nodes = self.selector.map(self.rule.all_nodes(self.use_unmerged_tree), &message).selected_nodes;
        if nodes.is_empty() {
            panic!(
                "{message}\nAssert needs to receive at least 1 node but 0 nodes were found for selector: '{}'\n",
                self.selector.description
            );
        }
        if !matcher.matches_any(&nodes) {
            panic!(
                "Failed to assertAny({})\nNone of the following nodes match:\n{}\nSelector used: '{}'\n",
                matcher.description,
                print_nodes(&nodes, 0),
                self.selector.description
            );
        }
        self
    }

    /// Asserts `matcher` holds for each of them (Compose's `assertAll`).
    #[track_caller]
    pub fn assert_all(&self, matcher: SemanticsMatcher) -> &Self {
        let message = format!("Failed to assertAll({})", matcher.description);
        let nodes = self.selector.map(self.rule.all_nodes(self.use_unmerged_tree), &message).selected_nodes;
        let violations: Vec<SemanticsNode> = nodes.into_iter().filter(|n| !matcher.matches(n)).collect();
        if !violations.is_empty() {
            panic!(
                "Failed to assertAll({})\nFound '{}' {} not matching:\n{}\nSelector used: '{}'\n",
                matcher.description,
                violations.len(),
                if violations.len() == 1 { "node" } else { "nodes" },
                print_nodes(&violations, 0),
                self.selector.description
            );
        }
        self
    }

    /// Them printed to `max_depth` (Compose's `printToString(maxDepth = 0)`).
    #[track_caller]
    pub fn print_to_string(&self, max_depth: usize) -> String {
        let nodes = self.fetch_semantics_nodes();
        let body =
            if nodes.is_empty() { "There were 0 nodes found!".to_string() } else { print_nodes(&nodes, max_depth) };
        format!("Printing with useUnmergedTree = '{}'\n{body}", self.use_unmerged_tree)
    }

    /// The same to the log (stderr) under `tag`.
    #[track_caller]
    pub fn print_to_log(&self, tag: &str, max_depth: usize) {
        eprintln!("{tag}: printToLog:\n{}", self.print_to_string(max_depth));
    }
}
