//! What the platform's accessibility bridges read (C++'s flat `SemanticsNode` summary and `performSemanticsAction`,
//! what Compose's AndroidComposeViewAccessibilityDelegateCompat works out of the merged tree): the nodes
//! accessibility sees, parents first and each parent's in reading order, and the actions it performs on them.

use super::configuration::{SemanticsConfiguration, SemanticsPropertyKey};
use super::node::{SemanticsNode, SemanticsOwner};
use super::properties::{
    AccessibilityAction, ActionFn, LiveRegionMode, ProgressBarRangeInfo, Role, SemanticsActions, SemanticsProperties,
};
use crate::state::ToggleableState;
use kompoz_graphics::{LayoutDirection, Rect};
use std::collections::HashMap;
use std::rc::Rc;

/// A node as accessibility sees it (C++'s SemanticsNode): a node of the merged tree that tells accessibility
/// something, summed up. Bounds are in the host's window, px.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct AccessibilityNode {
    /// Its semantics node's id: the same while the node is there.
    pub id: u64,
    /// The id of the node above it in this list (none for a root).
    pub parent: Option<u64>,
    pub bounds: Rect,
    /// What it says: its content descriptions then its texts, and where it merges, those of what it merges, in
    /// the order they are below it (what hides from accessibility left out); space separated.
    pub label: String,
    /// A field's text.
    pub value: String,
    pub state_description: String,
    pub role: Option<Role>,
    /// It has a click action.
    pub clickable: bool,
    /// It isn't disabled.
    pub enabled: bool,
    /// It has the keyboard focus.
    pub focused: bool,
    /// It is a text field.
    pub editable: bool,
    /// It scrolls.
    pub scrollable: bool,
    pub heading: bool,
    /// The selectables in it are one group.
    pub selectable_group: bool,
    /// On or off (a toggleable's state, or a selectable's selection); none if neither, or indeterminate.
    pub checked: Option<bool>,
    pub range: Option<ProgressBarRangeInfo>,
    /// `SetProgress` sets its value.
    pub settable: bool,
    pub live_region: Option<LiveRegionMode>,
    /// The labels of its custom actions (`SemanticsAction::CustomAction`'s index is into these).
    pub custom_actions: Vec<String>,
    /// A field's autofill hints (its own content type, or the nearest one above it).
    pub content_type: Vec<&'static str>,
    /// It hides what is typed in it.
    pub password: bool,
    /// It is a popup's or a dialog's root content.
    pub is_popup: bool,
    pub is_dialog: bool,
    pub pane_title: Option<String>,
    pub test_tag: Option<String>,
}

/// What accessibility asks of a node (C++'s SemanticsAction; Android's accessibility actions), each done through
/// its semantics action.
#[derive(Clone, PartialEq, Debug)]
pub enum SemanticsAction {
    Click,
    LongClick,
    /// The keyboard focus put on it (its `RequestFocus` action, else its focus target).
    Focus,
    /// A page forward or back along the axis it scrolls (its `ScrollBy` action, by its viewport's length).
    ScrollForward,
    ScrollBackward,
    PageUp,
    PageDown,
    PageLeft,
    PageRight,
    SetProgress(f32),
    /// Its custom action at this index.
    CustomAction(usize),
    SetText(String),
    SetSelection {
        start: i32,
        end: i32,
        relative_to_original_text: bool,
    },
    InsertTextAtCursor(String),
    ImeAction,
    CopyText,
    CutText,
    PasteText,
    Expand,
    Collapse,
    Dismiss,
}

impl SemanticsOwner {
    /// The nodes accessibility sees (C++'s `semanticsTree`): the merged tree's nodes that tell it something, not
    /// hidden (nor what is below a hidden one), parents first and each parent's in reading order. While a
    /// focusable popup or a dialog is on top, only its window is seen (it is modal); else the host's window and
    /// its non-focusable popups.
    pub fn accessibility_tree(&self) -> Vec<AccessibilityNode> {
        let Some(owner) = self.owner.upgrade() else { return Vec::new() };
        let roots: Vec<SemanticsNode> = match owner.top_focusable_popup() {
            Some(popup) => vec![SemanticsNode::of(popup, true)],
            None => self.root_semantics_nodes(true),
        };
        let focused = owner.focused_layout_node().map(|n| n.id());
        let mut out = Vec::new();
        for root in roots {
            visit(&root, None, &mut Vec::new(), focused, &mut out);
        }
        out
    }

    /// Performs `action` on the node with `id` (C++'s `performSemanticsAction`); whether it was done.
    pub fn perform_semantics_action(&self, id: u64, action: SemanticsAction) -> bool {
        let Some(node) = self.find(id, true) else { return false };
        let config = node.config();
        let run0 = |key: SemanticsPropertyKey<AccessibilityAction<ActionFn>>| {
            config.get(key).and_then(|a| a.action).is_some_and(|f| f())
        };
        match action {
            SemanticsAction::Click => run0(SemanticsActions::OnClick),
            SemanticsAction::LongClick => run0(SemanticsActions::OnLongClick),
            SemanticsAction::Focus => {
                if let Some(f) = config.get(SemanticsActions::RequestFocus).and_then(|a| a.action) {
                    return f();
                }
                crate::focus::request_focus_in_layout(node.layout_node())
            }
            SemanticsAction::ScrollForward | SemanticsAction::ScrollBackward => {
                let Some(scroll) = config.get(SemanticsActions::ScrollBy).and_then(|a| a.action) else { return false };
                let forward = action == SemanticsAction::ScrollForward;
                let horizontal = config.contains(SemanticsProperties::HorizontalScrollAxisRange)
                    && !config.contains(SemanticsProperties::VerticalScrollAxisRange);
                let mut lengths = Vec::new();
                let viewport = config
                    .get(SemanticsActions::GetScrollViewportLength)
                    .and_then(|a| a.action)
                    .filter(|f| f(&mut lengths))
                    .and_then(|_| lengths.first().copied());
                let size = node.size();
                let page = viewport.unwrap_or(if horizontal { size.width as f32 } else { size.height as f32 });
                let sign = if forward { 1.0 } else { -1.0 };
                if horizontal {
                    let rtl = node.layout_node().layout_direction() == LayoutDirection::Rtl;
                    let reverse =
                        config.get(SemanticsProperties::HorizontalScrollAxisRange).is_some_and(|r| r.reverse_scrolling);
                    let flip = if rtl != reverse { -1.0 } else { 1.0 };
                    scroll(sign * flip * page, 0.0)
                } else {
                    let reverse =
                        config.get(SemanticsProperties::VerticalScrollAxisRange).is_some_and(|r| r.reverse_scrolling);
                    scroll(0.0, sign * if reverse { -page } else { page })
                }
            }
            SemanticsAction::PageUp => run0(SemanticsActions::PageUp),
            SemanticsAction::PageDown => run0(SemanticsActions::PageDown),
            SemanticsAction::PageLeft => run0(SemanticsActions::PageLeft),
            SemanticsAction::PageRight => run0(SemanticsActions::PageRight),
            SemanticsAction::SetProgress(v) => {
                config.get(SemanticsActions::SetProgress).and_then(|a| a.action).is_some_and(|f| f(v))
            }
            SemanticsAction::CustomAction(i) => config
                .get(SemanticsActions::CustomActions)
                .and_then(|actions| actions.get(i).map(|a| a.action.clone()))
                .is_some_and(|f| f()),
            SemanticsAction::SetText(text) => {
                config.get(SemanticsActions::SetText).and_then(|a| a.action).is_some_and(|f| f(text.into()))
            }
            SemanticsAction::SetSelection { start, end, relative_to_original_text } => config
                .get(SemanticsActions::SetSelection)
                .and_then(|a| a.action)
                .is_some_and(|f| f(start, end, relative_to_original_text)),
            SemanticsAction::InsertTextAtCursor(text) => {
                config.get(SemanticsActions::InsertTextAtCursor).and_then(|a| a.action).is_some_and(|f| f(text.into()))
            }
            SemanticsAction::ImeAction => run0(SemanticsActions::OnImeAction),
            SemanticsAction::CopyText => run0(SemanticsActions::CopyText),
            SemanticsAction::CutText => run0(SemanticsActions::CutText),
            SemanticsAction::PasteText => run0(SemanticsActions::PasteText),
            SemanticsAction::Expand => run0(SemanticsActions::Expand),
            SemanticsAction::Collapse => run0(SemanticsActions::Collapse),
            SemanticsAction::Dismiss => run0(SemanticsActions::Dismiss),
        }
    }
}

/// Adds `node` (if accessibility sees it) and what is below it; `content_types` are those of the nodes above.
fn visit(
    node: &SemanticsNode,
    parent: Option<u64>,
    content_types: &mut Vec<Vec<&'static str>>,
    focused: Option<u64>,
    out: &mut Vec<AccessibilityNode>,
) {
    if node.is_hidden() {
        return;
    }
    // Not placed (measured only) or gone: not shown, not told (Compose's getAllUncoveredSemanticsNodes).
    if !node.is_fake() && (!node.is_placed() || !node.layout_node().is_attached()) {
        return;
    }
    let config = node.config();
    let own_types = config.get(SemanticsProperties::ContentType).map(|c| c.hints);
    let include = node.is_root() || node.is_important_for_accessibility();
    let mut me = parent;
    if include {
        let summary = summarize(node, &config, parent, content_types.last().cloned(), focused);
        me = Some(summary.id);
        out.push(summary);
    }
    let pushed = own_types.is_some();
    if let Some(t) = own_types {
        content_types.push(t);
    }
    let children = node.children();
    let sorted = node.sort_by_geometry_groupings(children, &|_| false, &HashMap::new());
    for child in sorted {
        visit(&child, me, content_types, focused, out);
    }
    if pushed {
        content_types.pop();
    }
}

fn summarize(
    node: &SemanticsNode,
    config: &SemanticsConfiguration,
    parent: Option<u64>,
    around: Option<Vec<&'static str>>,
    focused: Option<u64>,
) -> AccessibilityNode {
    let mut words = Vec::new();
    label_of(node, &mut words);
    let checked = match config.get(SemanticsProperties::ToggleableState) {
        Some(ToggleableState::On) => Some(true),
        Some(ToggleableState::Off) => Some(false),
        Some(ToggleableState::Indeterminate) => None,
        None => config.get(SemanticsProperties::Selected),
    };
    let editable =
        config.contains(SemanticsProperties::EditableText) || config.get(SemanticsProperties::IsEditable) == Some(true);
    AccessibilityNode {
        id: node.id(),
        parent,
        bounds: node.bounds_in_window(),
        label: words.join(" "),
        value: config.get(SemanticsProperties::EditableText).map(|t| t.text().to_string()).unwrap_or_default(),
        state_description: config.get(SemanticsProperties::StateDescription).unwrap_or_default(),
        role: config.get(SemanticsProperties::Role),
        clickable: config.get(SemanticsActions::OnClick).is_some_and(|a| a.action.is_some()),
        enabled: !config.contains(SemanticsProperties::Disabled),
        focused: config.get(SemanticsProperties::Focused) == Some(true) || focused == Some(node.layout_node().id()),
        editable,
        scrollable: node.is_scroll_node() || config.contains(SemanticsActions::ScrollBy),
        heading: config.contains(SemanticsProperties::Heading),
        selectable_group: config.contains(SemanticsProperties::SelectableGroup),
        checked,
        range: config.get(SemanticsProperties::ProgressBarRangeInfo),
        settable: config.get(SemanticsActions::SetProgress).is_some_and(|a| a.action.is_some()),
        live_region: config.get(SemanticsProperties::LiveRegion),
        custom_actions: config
            .get(SemanticsActions::CustomActions)
            .map(|a| a.into_iter().map(|c| c.label).collect())
            .unwrap_or_default(),
        content_type: if editable {
            config.get(SemanticsProperties::ContentType).map(|c| c.hints).or(around).unwrap_or_default()
        } else {
            Vec::new()
        },
        password: config.contains(SemanticsProperties::Password),
        is_popup: config.contains(SemanticsProperties::IsPopup),
        is_dialog: config.contains(SemanticsProperties::IsDialog),
        pane_title: config.get(SemanticsProperties::PaneTitle),
        test_tag: config.get(SemanticsProperties::TestTag),
    }
}

/// A node's words: its own content descriptions and texts, then (where it merges) those of the unmerged nodes
/// below it that don't merge their own and aren't hidden, in the order they are below it.
fn label_of(node: &SemanticsNode, words: &mut Vec<String>) {
    let own = node.unmerged_config();
    words.extend(own.get(SemanticsProperties::ContentDescription).unwrap_or_default());
    words.extend(own.get(SemanticsProperties::Text).unwrap_or_default().iter().map(|t| t.text().to_string()));
    if !node.is_merging_semantics_of_descendants() {
        return;
    }
    fn below(node: &SemanticsNode, words: &mut Vec<String>) {
        if node.unmerged_config().is_clearing_semantics {
            return;
        }
        let unmerged = SemanticsNode::of(Rc::clone(node.layout_node()), false);
        for child in unmerged.children() {
            let merged = child.with_merging_enabled();
            if merged.is_merging_semantics_of_descendants() || child.is_hidden() {
                continue;
            }
            let c = child.unmerged_config();
            words.extend(c.get(SemanticsProperties::ContentDescription).unwrap_or_default());
            words.extend(c.get(SemanticsProperties::Text).unwrap_or_default().iter().map(|t| t.text().to_string()));
            below(&child, words);
        }
    }
    below(node, words);
}
