//! What the platform's accessibility bridges read (C++'s flat `SemanticsNode` summary and `performSemanticsAction`,
//! what Compose's AndroidComposeViewAccessibilityDelegateCompat works out of the merged tree): the nodes
//! accessibility sees, parents first and each parent's in reading order, and the actions it performs on them.

use super::configuration::{SemanticsConfiguration, SemanticsPropertyKey};
use super::node::{SemanticsNode, SemanticsOwner};
use super::properties::{
    AccessibilityAction, ActionFn, CollectionInfo, CollectionItemInfo, LiveRegionMode, ProgressBarRangeInfo, Role,
    ScrollAxisRange, SemanticsActions, SemanticsProperties,
};
use crate::platform_text_layout::PlatformTextLayout;
use crate::state::ToggleableState;
use kompoz_graphics::{LayoutDirection, Rect};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

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
    /// A toggleable's state (on, off, indeterminate), apart from a selection (the web's `aria-checked`, `mixed`
    /// for the indeterminate).
    pub toggleable_state: Option<ToggleableState>,
    /// A selectable's selection (a tab's, an option's: the web's `aria-selected`), apart from a toggle.
    pub selected: Option<bool>,
    /// Expanded (it offers to collapse) or collapsed (it offers to expand): Compose's Expand and Collapse actions,
    /// as Android's delegate offers them; none if it offers neither.
    pub expanded: Option<bool>,
    /// What is wrong with its input (Compose's `error`), if something is.
    pub error: Option<String>,
    /// It is a link in a text (Compose's LinkTestMarker: a `LinkAnnotation`'s node).
    pub link: bool,
    /// It is a collection: its rows and columns (-1 where unknown: a lazy one's).
    pub collection_info: Option<CollectionInfo>,
    /// Its place in the collection above it: its own (Compose's `collectionItemInfo`), else, among the selectables
    /// of a selectable group that isn't lazy, its index among them (Compose's `setCollectionItemInfo`).
    pub collection_item_info: Option<CollectionItemInfo>,
    pub range: Option<ProgressBarRangeInfo>,
    /// `SetProgress` sets its value (offered only while it is enabled, as Compose does).
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
    /// A field's selection (its caret where it is collapsed), start then end, in code points of its text as typed
    /// (Compose's `textSelectionRange`): the same places in `value` where the field shows what is typed, or hides it
    /// a character for a character (a password); none where it isn't a field.
    pub text_selection: Option<(usize, usize)>,
    /// How far it has scrolled sideways (Compose's `horizontalScrollAxisRange`, read as the tree is made), where it
    /// says.
    pub horizontal_scroll: Option<ScrollPosition>,
    /// How far it has scrolled up and down (Compose's `verticalScrollAxisRange`), where it says.
    pub vertical_scroll: Option<ScrollPosition>,
    /// Where its text's characters and lines are (Compose's `GetTextLayoutResult`, read as the tree is made): a
    /// text's or a field's own layout, what a screen reader's character and line extents and hit tests are of. Its
    /// text is what is shown (a field's `value`, a text's text); the focused field's is its input session's (where
    /// its text is exactly, its decoration around it). None where it has no layout of its own (a node merging a
    /// text below it).
    pub text_layout: Option<Arc<PlatformTextLayout>>,
}

/// Where a scrollable is along an axis as the tree was made (a [`ScrollAxisRange`](super::ScrollAxisRange) read):
/// how far it has scrolled and how far it can, px (a lazy list's estimates), and whether it scrolls the other way
/// (its start at the end).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct ScrollPosition {
    pub value: f32,
    pub max_value: f32,
    pub reverse_scrolling: bool,
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
    /// Scrolled by this many px along each axis (its `ScrollBy` action): to a place (UI Automation's
    /// SetScrollPercent).
    ScrollBy {
        x: f32,
        y: f32,
    },
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
        let focused = owner.focused_layout_node().map(|n| n.semantics_id());
        let mut out = Vec::new();
        for root in roots {
            visit(&root, None, &mut Vec::new(), focused, &mut out);
        }
        out
    }

    /// Performs `action` on the node with `id` (C++'s `performSemanticsAction`); whether it was done. A disabled
    /// node takes none but setting its selection and copying (Compose's accessibility delegate).
    pub fn perform_semantics_action(&self, id: u64, action: SemanticsAction) -> bool {
        let Some(node) = self.find(id, true) else { return false };
        let config = node.config();
        // Compose's `performActionHelper`: only setting the selection and copying can be done to a disabled node.
        let can_be_done_disabled = matches!(action, SemanticsAction::SetSelection { .. } | SemanticsAction::CopyText);
        if config.contains(SemanticsProperties::Disabled) && !can_be_done_disabled {
            return false;
        }
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
            SemanticsAction::ScrollBy { x, y } => {
                config.get(SemanticsActions::ScrollBy).and_then(|a| a.action).is_some_and(|f| f(x, y))
            }
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
        focused: config.get(SemanticsProperties::Focused) == Some(true)
            || focused == Some(node.layout_node().semantics_id()),
        editable,
        scrollable: node.is_scroll_node() || config.contains(SemanticsActions::ScrollBy),
        heading: config.contains(SemanticsProperties::Heading),
        selectable_group: config.contains(SemanticsProperties::SelectableGroup),
        checked,
        toggleable_state: config.get(SemanticsProperties::ToggleableState),
        selected: config.get(SemanticsProperties::Selected),
        expanded: if config.get(SemanticsActions::Collapse).is_some_and(|a| a.action.is_some()) {
            Some(true)
        } else if config.get(SemanticsActions::Expand).is_some_and(|a| a.action.is_some()) {
            Some(false)
        } else {
            None
        },
        error: config.get(SemanticsProperties::Error),
        link: config.contains(SemanticsProperties::LinkTestMarker),
        collection_info: config.get(SemanticsProperties::CollectionInfo),
        collection_item_info: config
            .get(SemanticsProperties::CollectionItemInfo)
            .or_else(|| item_in_group(node, config)),
        range: config.get(SemanticsProperties::ProgressBarRangeInfo),
        // (Compose offers set-progress only on an enabled node: `addSetProgressAction`)
        settable: config.get(SemanticsActions::SetProgress).is_some_and(|a| a.action.is_some())
            && !config.contains(SemanticsProperties::Disabled),
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
        text_selection: config.get(SemanticsProperties::TextSelectionRange).map(|r| {
            // (Byte offsets of the text as typed, as code points: what a bridge reads `value` by.)
            let typed = config
                .get(SemanticsProperties::InputText)
                .or_else(|| config.get(SemanticsProperties::EditableText))
                .map(|t| t.text().to_string())
                .unwrap_or_default();
            let code_points = |byte: usize| typed.char_indices().take_while(|&(i, _)| i < byte).count();
            (code_points(r.start), code_points(r.end))
        }),
        horizontal_scroll: config.get(SemanticsProperties::HorizontalScrollAxisRange).map(scroll_position),
        vertical_scroll: config.get(SemanticsProperties::VerticalScrollAxisRange).map(scroll_position),
        text_layout: text_layout_of(node),
    }
}

/// Where its own text's characters are, from its own `GetTextLayoutResult` (a merging node's would be of a text
/// below it, at another place): its text laid out at its inner coordinates, cut to its bounds.
fn text_layout_of(node: &SemanticsNode) -> Option<Arc<PlatformTextLayout>> {
    if node.is_fake() {
        return None;
    }
    let get = node.unmerged_config().get(SemanticsActions::GetTextLayoutResult)?.action?;
    let mut out = Vec::new();
    if !get(&mut out) {
        return None;
    }
    let layout = out.into_iter().next()?;
    let coordinates = node.layout_node().inner_coordinates();
    if !coordinates.is_attached() {
        return None;
    }
    Some(Arc::new(PlatformTextLayout::of_text(&layout, &coordinates, node.bounds_in_window())))
}

/// A scroll axis's range as it is now.
fn scroll_position(r: ScrollAxisRange) -> ScrollPosition {
    ScrollPosition { value: (r.value)(), max_value: (r.max_value)(), reverse_scrolling: r.reverse_scrolling }
}

/// A selectable's place among the selectables of the selectable group it is in, where that group isn't a lazy
/// collection (Compose's `setCollectionItemInfo`): a row each (a column each where they are side by side).
fn item_in_group(node: &SemanticsNode, config: &SemanticsConfiguration) -> Option<CollectionItemInfo> {
    if !config.contains(SemanticsProperties::Selected) {
        return None;
    }
    let parent = node.parent()?;
    let group = parent.config();
    if !group.contains(SemanticsProperties::SelectableGroup)
        || group.get(SemanticsProperties::CollectionInfo).is_some_and(|c| c.row_count < 0 || c.column_count < 0)
    {
        return None;
    }
    let selectables: Vec<SemanticsNode> =
        parent.replaced_children().into_iter().filter(|c| c.config().contains(SemanticsProperties::Selected)).collect();
    let index = selectables.iter().position(|s| s.id() == node.id())? as i32;
    // (Side by side: the first two at the same height, the second to the right; Compose's
    // `calculateIfHorizontallyStacked`.)
    let horizontal = match selectables.as_slice() {
        [a, b, ..] => {
            let (a, b) = (a.bounds_in_window(), b.bounds_in_window());
            a.top == b.top && b.left > a.left
        }
        _ => false,
    };
    Some(if horizontal {
        CollectionItemInfo { row_index: 0, row_span: 1, column_index: index, column_span: 1 }
    } else {
        CollectionItemInfo { row_index: index, row_span: 1, column_index: 0, column_span: 1 }
    })
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
