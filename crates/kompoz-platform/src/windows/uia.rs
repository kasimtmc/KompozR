//! What UI Automation sees of a window's semantics tree, in plain Rust (tested on every host; `uia_bridge` is the
//! COM shell around it): each node's control type, properties and patterns ([`control_type_of`],
//! [`property_of`], [`has_pattern`] — at the AT-SPI bridge's parity: invoke, toggle, a selectable group's selection
//! and its items', a field's value and text, a range's value, scrolling with its percentages where the node says
//! where it is, expanding and collapsing), what a client's request on it is as a semantics action ([`invoke`],
//! [`scroll_by`], [`scroll_to_percent`]), what changed from one published tree to the next as UI Automation's events
//! ([`raises`]: from the shared [`changes`] — subtrees, names, states, values, texts, live regions, the focus — and
//! what UI Automation tells besides: toggle and selection states, expanding, a field's selection, scroll positions)
//! and a field's text as Narrator and the touch keyboard read it by characters, words, lines and the whole
//! ([`Text`]: the text ranges' moves, the shared word boundaries).
//!
//! The ids are UIAutomationClient.h's (stable numbers: UIA_*PropertyId, UIA_*PatternId, UIA_*EventId,
//! UIA_*ControlTypeId). Offsets in a field's text are code points (a range's ends: UI Automation never sees them).

use crate::accessibility::text::{Unit, code_points, segment, text_of};
use crate::accessibility::{Change, Snapshot, changes};
use kompoz_ui::semantics::{
    AccessibilityNode, LiveRegionMode, ProgressBarRangeInfo, Role, ScrollPosition, SemanticsAction, ToggleableState,
};
use std::collections::HashSet;

/// UIA_*ControlTypeId.
pub mod control {
    pub const BUTTON: i32 = 50000;
    pub const CHECK_BOX: i32 = 50002;
    pub const COMBO_BOX: i32 = 50003;
    pub const EDIT: i32 = 50004;
    pub const HYPERLINK: i32 = 50005;
    pub const IMAGE: i32 = 50006;
    pub const LIST_ITEM: i32 = 50007;
    pub const LIST: i32 = 50008;
    #[cfg(test)]
    pub const MENU_BAR: i32 = 50010;
    pub const PROGRESS_BAR: i32 = 50012;
    pub const RADIO_BUTTON: i32 = 50013;
    pub const SLIDER: i32 = 50015;
    pub const SPINNER: i32 = 50016;
    pub const TAB: i32 = 50018;
    pub const TAB_ITEM: i32 = 50019;
    pub const TEXT: i32 = 50020;
    pub const GROUP: i32 = 50026;
    pub const DATA_GRID: i32 = 50028;
    pub const DATA_ITEM: i32 = 50029;
    pub const WINDOW: i32 = 50032;
    pub const PANE: i32 = 50033;
    #[cfg(test)]
    pub const TITLE_BAR: i32 = 50037;
}

/// UIA_*PropertyId.
pub mod property {
    pub const CONTROL_TYPE: i32 = 30003;
    pub const LOCALIZED_CONTROL_TYPE: i32 = 30004;
    pub const NAME: i32 = 30005;
    pub const HAS_KEYBOARD_FOCUS: i32 = 30008;
    pub const IS_KEYBOARD_FOCUSABLE: i32 = 30009;
    pub const IS_ENABLED: i32 = 30010;
    pub const AUTOMATION_ID: i32 = 30011;
    pub const HELP_TEXT: i32 = 30013;
    pub const IS_PASSWORD: i32 = 30019;
    pub const FRAMEWORK_ID: i32 = 30024;
    pub const ITEM_STATUS: i32 = 30026;
    pub const VALUE: i32 = 30045;
    pub const RANGE_VALUE: i32 = 30047;
    pub const HORIZONTAL_SCROLL_PERCENT: i32 = 30053;
    pub const VERTICAL_SCROLL_PERCENT: i32 = 30055;
    pub const EXPAND_COLLAPSE_STATE: i32 = 30070;
    pub const IS_SELECTED: i32 = 30079;
    pub const TOGGLE_STATE: i32 = 30086;
    pub const IS_DATA_VALID_FOR_FORM: i32 = 30103;
    pub const LIVE_SETTING: i32 = 30135;
    pub const POSITION_IN_SET: i32 = 30152;
    pub const SIZE_OF_SET: i32 = 30153;
    pub const HEADING_LEVEL: i32 = 30173;
    pub const IS_DIALOG: i32 = 30174;
}

/// UIA_*PatternId.
pub mod pattern {
    pub const INVOKE: i32 = 10000;
    pub const SELECTION: i32 = 10001;
    pub const VALUE: i32 = 10002;
    pub const RANGE_VALUE: i32 = 10003;
    pub const SCROLL: i32 = 10004;
    pub const EXPAND_COLLAPSE: i32 = 10005;
    pub const SELECTION_ITEM: i32 = 10010;
    pub const TEXT: i32 = 10014;
    pub const TOGGLE: i32 = 10015;
    pub const TEXT2: i32 = 10024;
}

/// UIA_*EventId.
pub mod event {
    pub const FOCUS_CHANGED: i32 = 20005;
    pub const ELEMENT_SELECTED: i32 = 20012;
    pub const TEXT_SELECTION_CHANGED: i32 = 20014;
    pub const TEXT_CHANGED: i32 = 20015;
    pub const WINDOW_OPENED: i32 = 20016;
    pub const LIVE_REGION_CHANGED: i32 = 20024;
}

/// StructureChangeType.
pub const CHILD_ADDED: i32 = 0;
pub const CHILD_REMOVED: i32 = 1;
/// HeadingLevel1 (Compose's headings have no level: the first).
pub const HEADING_LEVEL_1: i32 = 80051;
/// UIA_ScrollPatternNoScroll: an axis it doesn't scroll along (or doesn't say where it is).
pub const NO_SCROLL: f64 = -1.0;
/// UiaAppendRuntimeId: a fragment's runtime id goes on its window's.
pub const APPEND_RUNTIME_ID: i32 = 3;
/// What the app's nodes say they are made with (UIA_FrameworkIdPropertyId).
pub const FRAMEWORK: &str = "KompozR";

/// What a slider's small change is without steps: a twentieth of its range (Compose's `AccessibilityRangeStepCount`,
/// as the web's mirror steps it).
const RANGE_STEPS: f32 = 20.0;

/// A property's value (a VARIANT's: empty is "not said": UI Automation's default for it).
#[derive(Clone, PartialEq, Debug)]
pub enum Prop {
    Empty,
    Bool(bool),
    Int(i32),
    Double(f64),
    Str(String),
}

/// A node's runtime id under its window's (UiaAppendRuntimeId, then its id's two halves).
pub fn runtime_id(id: u64) -> [i32; 3] {
    [APPEND_RUNTIME_ID, (id >> 32) as i32, id as i32]
}

/// Its name: its label, else its pane's title.
pub fn name_of(n: &AccessibilityNode) -> String {
    match &n.pane_title {
        Some(title) if n.label.is_empty() => title.clone(),
        _ => n.label.clone(),
    }
}

/// What Tab goes to, and what a client may put the focus on: a field, and what does something while it is
/// enabled (clicks, toggles, is selected, is set, expands).
pub fn focusable(n: &AccessibilityNode) -> bool {
    n.editable
        || (n.enabled
            && (n.clickable
                || n.settable
                || n.toggleable_state.is_some()
                || n.selected.is_some()
                || n.expanded.is_some()))
}

/// A selectable's selection (a tab's, a radio button's, an option's: SelectionItem); none where it isn't one.
pub fn selected_of(n: &AccessibilityNode) -> Option<bool> {
    match n.role {
        Some(Role::Tab | Role::RadioButton) => n.selected.or(n.checked),
        _ if n.toggleable_state.is_none() => n.selected,
        _ => None,
    }
}

/// A toggle's state (ToggleState: off 0, on 1, indeterminate 2); none where it isn't one (a selectable isn't).
pub fn toggle_of(n: &AccessibilityNode) -> Option<i32> {
    if selected_of(n).is_some() {
        return None;
    }
    match n.toggleable_state {
        Some(ToggleableState::Off) => Some(0),
        Some(ToggleableState::On) => Some(1),
        Some(ToggleableState::Indeterminate) => Some(2),
        None => n.checked.map(i32::from),
    }
}

/// A range that has a value (an indeterminate progress has none).
fn range_of(n: &AccessibilityNode) -> Option<&ProgressBarRangeInfo> {
    n.range.as_ref().filter(|r| **r != ProgressBarRangeInfo::Indeterminate)
}

/// Whether a grid (more than one row and column, or a lazy grid's unknown ones) rather than a list.
fn is_grid(c: kompoz_ui::semantics::CollectionInfo) -> bool {
    c.row_count != 1 && c.column_count != 1
}

/// Its control type, by what it is and does (the AT-SPI bridge's roles and the web mirror's, in UI Automation's
/// terms; a switch is a button that toggles, as WinUI's ToggleSwitch).
pub fn control_type_of(tree: &Snapshot, n: &AccessibilityNode) -> i32 {
    use control::*;
    match n.role {
        Some(Role::Button | Role::Switch) => return BUTTON,
        Some(Role::Checkbox) => return CHECK_BOX,
        Some(Role::RadioButton) => return RADIO_BUTTON,
        Some(Role::Tab) => return TAB_ITEM,
        Some(Role::Image) => return IMAGE,
        Some(Role::DropdownList) => return COMBO_BOX,
        Some(Role::ValuePicker) => return SPINNER,
        Some(Role::Carousel) | None => {}
    }
    if n.editable {
        return EDIT;
    }
    if n.is_dialog {
        return WINDOW;
    }
    if n.heading {
        return TEXT;
    }
    if n.link {
        return HYPERLINK;
    }
    if let Some(r) = &n.range {
        return if n.settable && *r != ProgressBarRangeInfo::Indeterminate { SLIDER } else { PROGRESS_BAR };
    }
    let kids = tree.children(Some(n.id));
    if n.selectable_group {
        let any = |f: &dyn Fn(&AccessibilityNode) -> bool| kids.iter().any(|&k| f(tree.node(k)));
        if any(&|k| k.role == Some(Role::Tab)) {
            return TAB;
        }
        if any(&|k| k.role == Some(Role::RadioButton)) {
            return GROUP;
        }
        if any(&|k| k.role.is_none() && k.selected.is_some()) {
            return LIST;
        }
    }
    if let Some(c) = n.collection_info {
        return if is_grid(c) { DATA_GRID } else { LIST };
    }
    if n.toggleable_state.is_some() {
        return CHECK_BOX;
    }
    if let Some(parent) = n.parent.and_then(|p| tree.get(p)) {
        if parent.selectable_group && n.selected.is_some() && control_type_of(tree, parent) == LIST {
            return LIST_ITEM;
        }
        if let Some(c) = parent.collection_info {
            return if is_grid(c) { DATA_ITEM } else { LIST_ITEM };
        }
    }
    if n.clickable {
        return BUTTON;
    }
    if n.scrollable || n.pane_title.is_some() {
        return PANE;
    }
    if !n.label.is_empty() && kids.is_empty() {
        return TEXT;
    }
    GROUP
}

/// What its type is called where UI Automation's own name for its control type says less (a switch: Windows'
/// "toggle switch"; a carousel; a dialog).
pub fn localized_control_type(n: &AccessibilityNode) -> Option<&'static str> {
    match n.role {
        Some(Role::Switch) => Some("toggle switch"),
        Some(Role::Carousel) => Some("carousel"),
        _ if n.is_dialog => Some("dialog"),
        _ => None,
    }
}

/// Its place among its set and the set's size, from 1 (a list's items by their collection item info; a selectable
/// group's selectables among them), each where it is known.
pub fn set_place(tree: &Snapshot, n: &AccessibilityNode) -> (Option<i32>, Option<i32>) {
    let parent = n.parent.and_then(|p| tree.get(p));
    if let Some(c) = parent.and_then(|p| p.collection_info) {
        if is_grid(c) {
            return (None, None); // (a grid's cells: their row and column, not a place in a set)
        }
        let item = n.collection_item_info;
        let (size, index) = if c.row_count == 1 {
            (c.column_count, item.map(|i| i.column_index))
        } else {
            (c.row_count, item.map(|i| i.row_index))
        };
        return (index.map(|i| i + 1), (size >= 0).then_some(size));
    }
    if let (Some(i), Some(parent)) = (n.collection_item_info, parent)
        && selected_of(n).is_some()
    {
        let count = tree.children(Some(parent.id)).iter().filter(|&&k| selected_of(tree.node(k)).is_some()).count();
        return (Some(i.row_index.max(i.column_index) + 1), Some(count as i32));
    }
    (None, None)
}

/// Property `id` of node `n` (empty: UI Automation's default; what the window's own HWND provider says of it is
/// the root's).
pub fn property_of(tree: &Snapshot, n: &AccessibilityNode, id: i32) -> Prop {
    use property::*;
    let text = |s: &str| if s.is_empty() { Prop::Empty } else { Prop::Str(s.to_string()) };
    match id {
        CONTROL_TYPE => Prop::Int(control_type_of(tree, n)),
        LOCALIZED_CONTROL_TYPE => localized_control_type(n).map_or(Prop::Empty, |t| Prop::Str(t.into())),
        NAME => text(&name_of(n)),
        AUTOMATION_ID => Prop::Str(n.test_tag.clone().unwrap_or_else(|| n.id.to_string())),
        FRAMEWORK_ID => Prop::Str(FRAMEWORK.into()),
        IS_ENABLED => Prop::Bool(n.enabled),
        HAS_KEYBOARD_FOCUS => Prop::Bool(n.focused),
        IS_KEYBOARD_FOCUSABLE => Prop::Bool(focusable(n)),
        IS_PASSWORD => Prop::Bool(n.password),
        ITEM_STATUS => text(&n.state_description),
        HELP_TEXT => n.error.as_deref().map_or(Prop::Empty, text),
        IS_DATA_VALID_FOR_FORM if n.editable || n.error.is_some() => Prop::Bool(n.error.is_none()),
        LIVE_SETTING => match n.live_region {
            Some(LiveRegionMode::Polite) => Prop::Int(1),
            Some(LiveRegionMode::Assertive) => Prop::Int(2),
            None => Prop::Empty,
        },
        HEADING_LEVEL if n.heading => Prop::Int(HEADING_LEVEL_1),
        IS_DIALOG => Prop::Bool(n.is_dialog),
        POSITION_IN_SET => set_place(tree, n).0.map_or(Prop::Empty, Prop::Int),
        SIZE_OF_SET => set_place(tree, n).1.map_or(Prop::Empty, Prop::Int),
        _ => Prop::Empty,
    }
}

/// Whether node `n` has pattern `id`.
pub fn has_pattern(n: &AccessibilityNode, id: i32) -> bool {
    match id {
        pattern::INVOKE => (n.clickable || n.link) && !n.editable && toggle_of(n).is_none() && selected_of(n).is_none(),
        pattern::TOGGLE => toggle_of(n).is_some(),
        pattern::SELECTION_ITEM => selected_of(n).is_some(),
        pattern::SELECTION => n.selectable_group,
        pattern::VALUE | pattern::TEXT | pattern::TEXT2 => n.editable,
        pattern::RANGE_VALUE => range_of(n).is_some(),
        pattern::SCROLL => n.scrollable,
        pattern::EXPAND_COLLAPSE => n.expanded.is_some(),
        _ => false,
    }
}

/// The selection container of a selectable (its group), if it is in one.
pub fn selection_container(tree: &Snapshot, n: &AccessibilityNode) -> Option<u64> {
    n.parent.filter(|&p| tree.get(p).is_some_and(|g| g.selectable_group))
}

/// The selected ones of a selectable group, in order.
pub fn selection_of(tree: &Snapshot, group: &AccessibilityNode) -> Vec<u64> {
    tree.children(Some(group.id)).iter().copied().filter(|&k| selected_of(tree.node(k)) == Some(true)).collect()
}

/// What invoking it asks for: its click (an expandable without one: expanding or collapsing it).
pub fn invoke(n: &AccessibilityNode) -> SemanticsAction {
    match n.expanded {
        Some(true) if !n.clickable => SemanticsAction::Collapse,
        Some(false) if !n.clickable => SemanticsAction::Expand,
        _ => SemanticsAction::Click,
    }
}

/// ExpandCollapseState: collapsed 0, expanded 1, a leaf 3.
pub fn expand_collapse_state(n: &AccessibilityNode) -> i32 {
    match n.expanded {
        Some(false) => 0,
        Some(true) => 1,
        None => 3,
    }
}

/// A range's numbers as RangeValue tells them: value, minimum, maximum, small change (a step), large change (a
/// tenth).
pub fn range_numbers(n: &AccessibilityNode) -> Option<[f64; 5]> {
    let r = range_of(n)?;
    let (lo, hi) = (*r.range.start(), *r.range.end());
    let span = hi - lo;
    let small = if r.steps > 0 { span / (r.steps + 1) as f32 } else { span / RANGE_STEPS };
    Some([r.current as f64, lo as f64, hi as f64, small as f64, (span / 10.0) as f64])
}

/// What setting a slider to `value` asks for; why not (it can't be set: UIA_E_INVALIDOPERATION; out of its range:
/// E_INVALIDARG).
pub fn set_range_value(n: &AccessibilityNode, value: f64) -> Result<SemanticsAction, RangeRefusal> {
    let r = range_of(n).filter(|_| n.settable).ok_or(RangeRefusal::ReadOnly)?;
    let v = value as f32;
    if !value.is_finite() || v < *r.range.start() || v > *r.range.end() {
        return Err(RangeRefusal::OutOfRange);
    }
    Ok(SemanticsAction::SetProgress(v))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RangeRefusal {
    ReadOnly,
    OutOfRange,
}

// ---- Scrolling ---------------------------------------------------------------------------------------------------

/// An axis: sideways or up and down.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    Horizontal,
    Vertical,
}

fn position(n: &AccessibilityNode, axis: Axis) -> Option<ScrollPosition> {
    match axis {
        Axis::Horizontal => n.horizontal_scroll,
        Axis::Vertical => n.vertical_scroll,
    }
}

fn viewport(n: &AccessibilityNode, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => n.bounds.width(),
        Axis::Vertical => n.bounds.height(),
    }
    .max(0.0)
}

/// Whether it scrolls along `axis` (a scrollable that doesn't say along which: up and down).
pub fn scrolls(n: &AccessibilityNode, axis: Axis) -> bool {
    if !n.scrollable {
        return false;
    }
    match (n.horizontal_scroll, n.vertical_scroll) {
        (None, None) => axis == Axis::Vertical,
        _ => position(n, axis).is_some_and(|p| p.max_value > 0.0),
    }
}

/// How far along `axis` it has scrolled, 0 to 100 from its start (its reversed start at the end); NO_SCROLL where
/// it doesn't scroll along it or doesn't say.
pub fn scroll_percent(n: &AccessibilityNode, axis: Axis) -> f64 {
    match position(n, axis).filter(|p| n.scrollable && p.max_value > 0.0) {
        Some(p) => {
            let f = ((p.value / p.max_value).clamp(0.0, 1.0) * 100.0) as f64;
            if p.reverse_scrolling { 100.0 - f } else { f }
        }
        None => NO_SCROLL,
    }
}

/// How much of what it scrolls shows along `axis`, a percentage (all of it where it doesn't say).
pub fn view_size(n: &AccessibilityNode, axis: Axis) -> f64 {
    match position(n, axis).filter(|p| p.max_value > 0.0) {
        Some(p) => {
            let (shown, more) = (viewport(n, axis) as f64, p.max_value as f64);
            shown / (shown + more) * 100.0
        }
        None => 100.0,
    }
}

/// ScrollAmount's scroll along an axis its viewport long, px on the screen (right and down positive): a page
/// (large), a tenth of one (small), none.
fn amount(amount: i32, page: f32) -> Option<f32> {
    match amount {
        0 => Some(-page),        // LargeDecrement
        1 => Some(-page / 10.0), // SmallDecrement
        3 => Some(page),         // LargeIncrement
        4 => Some(page / 10.0),  // SmallIncrement
        _ => None,               // NoAmount
    }
}

/// What ScrollPattern's Scroll(`horizontal`, `vertical`) (ScrollAmounts) asks for: a scroll by that many px on the
/// screen (Compose's `scrollBy` takes them as the screen has them); none: nothing to do, or along an axis it
/// doesn't scroll (UIA_E_INVALIDOPERATION).
pub fn scroll_by(n: &AccessibilityNode, horizontal: i32, vertical: i32) -> Result<Option<SemanticsAction>, ()> {
    let x = amount(horizontal, viewport(n, Axis::Horizontal));
    let y = amount(vertical, viewport(n, Axis::Vertical));
    if (x.is_some() && !scrolls(n, Axis::Horizontal)) || (y.is_some() && !scrolls(n, Axis::Vertical)) {
        return Err(());
    }
    if x.is_none() && y.is_none() {
        return Ok(None);
    }
    Ok(Some(SemanticsAction::ScrollBy { x: x.unwrap_or(0.0), y: y.unwrap_or(0.0) }))
}

/// What SetScrollPercent(`horizontal`, `vertical`) asks for (NO_SCROLL: that axis as it is): the scroll from where
/// it is to there, px on the screen; none where it doesn't say where it is along an axis asked for, or a percentage
/// is out of 0 to 100.
pub fn scroll_to_percent(n: &AccessibilityNode, horizontal: f64, vertical: f64) -> Option<SemanticsAction> {
    let delta = |axis: Axis, percent: f64| -> Option<f32> {
        if percent == NO_SCROLL {
            return Some(0.0);
        }
        if !(0.0..=100.0).contains(&percent) {
            return None;
        }
        let p = position(n, axis).filter(|p| n.scrollable && p.max_value > 0.0)?;
        let from_start = if p.reverse_scrolling { 100.0 - percent } else { percent };
        let d = (from_start / 100.0) as f32 * p.max_value - p.value;
        // (Its value grows the other way on the screen where it scrolls reversed.)
        Some(if p.reverse_scrolling { -d } else { d })
    };
    let (x, y) = (delta(Axis::Horizontal, horizontal)?, delta(Axis::Vertical, vertical)?);
    Some(SemanticsAction::ScrollBy { x, y })
}

// ---- Events -------------------------------------------------------------------------------------------------------

/// What a published tree raises.
#[derive(Clone, PartialEq, Debug)]
pub enum Raise {
    /// A node's property changed (UiaRaiseAutomationPropertyChangedEvent).
    Property { node: u64, property: i32, old: Prop, new: Prop },
    /// An event on a node (UiaRaiseAutomationEvent).
    Event { node: u64, event: i32 },
    /// Its children changed (UiaRaiseStructureChangedEvent): on the node that came (ChildAdded), on the parent
    /// that stays of one that went (ChildRemoved; none: the window's root), with that child's runtime id.
    Structure { node: Option<u64>, change: i32, child: u64 },
}

/// The events that tell what changed from `old` to `next` (the shared [`changes`] in UI Automation's terms, what it
/// tells besides of each node before the focus moves).
pub fn raises(old: &Snapshot, next: &Snapshot) -> Vec<Raise> {
    use property::*;
    let mut own = own_changes(old, next);
    let mut out = Vec::new();
    let mut texts = HashSet::new();
    let mut flushed = false;
    for change in changes(old, next) {
        if matches!(change, Change::FocusLeft(_) | Change::Focused(_)) && !flushed {
            out.append(&mut own);
            flushed = true;
        }
        match change {
            Change::Removed { parent, node, .. } => {
                out.push(Raise::Structure { node: parent, change: CHILD_REMOVED, child: node });
            }
            Change::Added { node, .. } => {
                out.push(Raise::Structure { node: Some(node), change: CHILD_ADDED, child: node });
                if next.node(node).is_dialog {
                    out.push(Raise::Event { node, event: event::WINDOW_OPENED });
                }
            }
            // (The window's own: its HWND's provider tells them.)
            Change::WindowFocused(_) | Change::WindowTitle(_) => {}
            Change::Name { node, name } => {
                let was = name_of(old.node(node));
                let now = if name.is_empty() { name_of(next.node(node)) } else { name };
                if was != now {
                    out.push(Raise::Property { node, property: NAME, old: Prop::Str(was), new: Prop::Str(now) });
                }
            }
            Change::Description { node, description } => out.push(Raise::Property {
                node,
                property: ITEM_STATUS,
                old: Prop::Str(old.node(node).state_description.clone()),
                new: Prop::Str(description),
            }),
            Change::Announcement { node, .. } => out.push(Raise::Event { node, event: event::LIVE_REGION_CHANGED }),
            // (Toggles and selections: what UI Automation tells of each, below.)
            Change::Checked { .. } => {}
            Change::Enabled { node, enabled } => out.push(Raise::Property {
                node,
                property: IS_ENABLED,
                old: Prop::Bool(!enabled),
                new: Prop::Bool(enabled),
            }),
            Change::Value { node, value } => {
                let was = old.node(node).range.as_ref().map_or(0.0, |r| r.current);
                out.push(Raise::Property {
                    node,
                    property: RANGE_VALUE,
                    old: Prop::Double(was as f64),
                    new: Prop::Double(value as f64),
                });
            }
            Change::TextRemoved { node, .. } | Change::TextInserted { node, .. } => {
                if texts.insert(node) {
                    out.push(Raise::Property {
                        node,
                        property: VALUE,
                        old: Prop::Str(text_of(old.node(node))),
                        new: Prop::Str(text_of(next.node(node))),
                    });
                    out.push(Raise::Event { node, event: event::TEXT_CHANGED });
                }
            }
            Change::FocusLeft(node) => out.push(Raise::Property {
                node,
                property: HAS_KEYBOARD_FOCUS,
                old: Prop::Bool(true),
                new: Prop::Bool(false),
            }),
            Change::Focused(node) => {
                out.push(Raise::Property {
                    node,
                    property: HAS_KEYBOARD_FOCUS,
                    old: Prop::Bool(false),
                    new: Prop::Bool(true),
                });
                out.push(Raise::Event { node, event: event::FOCUS_CHANGED });
            }
        }
    }
    out.append(&mut own);
    out
}

/// What UI Automation tells of each node that stays besides the shared changes: its toggle state, its selection,
/// its expanding, a field's selection, its scroll positions, whether its input is valid.
fn own_changes(old: &Snapshot, next: &Snapshot) -> Vec<Raise> {
    use property::*;
    let mut out = Vec::new();
    if !old.is_published() {
        return out;
    }
    for n in next.nodes() {
        let Some(o) = old.get(n.id) else { continue };
        let node = n.id;
        if let (Some(was), Some(now)) = (toggle_of(o), toggle_of(n))
            && was != now
        {
            out.push(Raise::Property { node, property: TOGGLE_STATE, old: Prop::Int(was), new: Prop::Int(now) });
        }
        if let (Some(was), Some(now)) = (selected_of(o), selected_of(n))
            && was != now
        {
            out.push(Raise::Property { node, property: IS_SELECTED, old: Prop::Bool(was), new: Prop::Bool(now) });
            if now {
                out.push(Raise::Event { node, event: event::ELEMENT_SELECTED });
            }
        }
        if o.expanded.is_some() && n.expanded.is_some() && o.expanded != n.expanded {
            out.push(Raise::Property {
                node,
                property: EXPAND_COLLAPSE_STATE,
                old: Prop::Int(expand_collapse_state(o)),
                new: Prop::Int(expand_collapse_state(n)),
            });
        }
        if n.editable && o.editable && caret_of(o) != caret_of(n) {
            out.push(Raise::Event { node, event: event::TEXT_SELECTION_CHANGED });
        }
        for (axis, property) in
            [(Axis::Horizontal, HORIZONTAL_SCROLL_PERCENT), (Axis::Vertical, VERTICAL_SCROLL_PERCENT)]
        {
            let (was, now) = (scroll_percent(o, axis), scroll_percent(n, axis));
            if was != now {
                out.push(Raise::Property { node, property, old: Prop::Double(was), new: Prop::Double(now) });
            }
        }
        if o.error.is_some() != n.error.is_some() {
            out.push(Raise::Property {
                node,
                property: IS_DATA_VALID_FOR_FORM,
                old: Prop::Bool(o.error.is_none()),
                new: Prop::Bool(n.error.is_none()),
            });
        }
    }
    out
}

// ---- Text ---------------------------------------------------------------------------------------------------------

/// A field's selection in code points of its text as said (start, end in order; the caret where it is collapsed;
/// at the end where the field doesn't say, as AT-SPI's caret), and where its caret is (the selection's end).
pub fn caret_of(n: &AccessibilityNode) -> ((usize, usize), usize) {
    let len = text_of(n).chars().count();
    match n.text_selection {
        Some((start, end)) => {
            let (start, end) = (start.min(len), end.min(len));
            ((start.min(end), start.max(end)), end)
        }
        None => ((len, len), len),
    }
}

/// A text unit as the ranges move by (TextUnit: those it hasn't are the next larger it has — a format run a word,
/// a paragraph a line, a page the whole).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextUnit {
    Character,
    Word,
    Line,
    Document,
}

impl TextUnit {
    /// TextUnit's value.
    pub fn from_uia(unit: i32) -> TextUnit {
        match unit {
            0 => TextUnit::Character,
            1 | 2 => TextUnit::Word,
            3 | 4 => TextUnit::Line,
            _ => TextUnit::Document,
        }
    }
}

/// A field's text as its ranges read it: by code points (the shared word boundaries; lines end after their line
/// breaks: a field knows no other).
pub struct Text {
    s: String,
    at: Vec<usize>,
}

impl Text {
    pub fn new(s: &str) -> Text {
        Text { s: s.to_string(), at: code_points(s) }
    }

    /// Its length, in code points.
    pub fn len(&self) -> usize {
        self.at.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Code points `start` to `end` (clamped).
    pub fn slice(&self, start: usize, end: usize) -> &str {
        let n = self.len();
        let (start, end) = (start.min(n), end.min(n).max(start.min(n)));
        &self.s[self.at[start]..self.at[end]]
    }

    fn char_at(&self, i: usize) -> Option<char> {
        self.slice(i, i + 1).chars().next()
    }

    /// The `unit` code point `pos` is in, [start, end) (at the end: the last one; an empty text's: nothing).
    pub fn enclosing(&self, pos: usize, unit: TextUnit) -> (usize, usize) {
        if self.is_empty() {
            return (0, 0);
        }
        let n = self.len();
        let pos = pos.min(n - 1);
        match unit {
            TextUnit::Character => (pos, pos + 1),
            TextUnit::Word => {
                let (s, e) = segment(&self.s, &self.at, pos as i32, Unit::Word);
                (s as usize, e as usize)
            }
            TextUnit::Line => {
                let mut start = pos;
                while start > 0 && self.char_at(start - 1) != Some('\n') {
                    start -= 1;
                }
                let mut end = pos;
                while end < n && self.char_at(end) != Some('\n') {
                    end += 1;
                }
                (start, (end + 1).min(n))
            }
            TextUnit::Document => (0, n),
        }
    }

    /// The first boundary of `unit` after `pos` (the end is one), if there is one.
    pub fn next_boundary(&self, pos: usize, unit: TextUnit) -> Option<usize> {
        (pos < self.len()).then(|| self.enclosing(pos, unit).1)
    }

    /// The last boundary of `unit` before `pos` (the start is one), if there is one.
    pub fn previous_boundary(&self, pos: usize, unit: TextUnit) -> Option<usize> {
        let pos = pos.min(self.len());
        (pos > 0).then(|| self.enclosing(pos - 1, unit).0)
    }

    /// An end at `pos` moved by `count` units (forward where positive): where it goes, and by how many it moved.
    pub fn move_endpoint(&self, pos: usize, unit: TextUnit, count: i32) -> (usize, i32) {
        let mut pos = pos.min(self.len());
        let mut moved = 0;
        while moved < count.abs() {
            let next = if count > 0 { self.next_boundary(pos, unit) } else { self.previous_boundary(pos, unit) };
            let Some(next) = next else { break };
            pos = next;
            moved += 1;
        }
        (pos, moved * count.signum())
    }

    /// A range moved by `count` units (ITextRangeProvider::Move): a collapsed one stays collapsed, moved by its
    /// end; another becomes the whole unit its start is in, moved by whole units (never past the last).
    pub fn move_range(&self, start: usize, end: usize, unit: TextUnit, count: i32) -> (usize, usize, i32) {
        if count == 0 {
            return (start, end, 0);
        }
        if start == end {
            let (pos, moved) = self.move_endpoint(start, unit, count);
            return (pos, pos, moved);
        }
        let (mut s, _) = self.enclosing(start, unit);
        let mut moved = 0;
        while moved < count.abs() {
            let next = if count > 0 {
                self.next_boundary(s, unit).filter(|&b| b < self.len())
            } else {
                self.previous_boundary(s, unit)
            };
            let Some(next) = next else { break };
            s = next;
            moved += 1;
        }
        let (s, e) = self.enclosing(s, unit);
        (s, e, moved * count.signum())
    }

    /// Where `needle` is in [start, end) (the last one where `backward`), as code points.
    pub fn find(
        &self,
        start: usize,
        end: usize,
        needle: &str,
        backward: bool,
        ignore_case: bool,
    ) -> Option<(usize, usize)> {
        if needle.is_empty() {
            return None;
        }
        let fold = |s: &str| if ignore_case { s.to_lowercase() } else { s.to_string() };
        let hay: Vec<char> = self.slice(start, end).chars().collect();
        let needle: Vec<char> = fold(needle).chars().collect();
        let fits = |i: usize| -> bool {
            i + needle.len() <= hay.len()
                && fold(&hay[i..i + needle.len()].iter().collect::<String>()).chars().eq(needle.iter().copied())
        };
        let found = if backward { (0..hay.len()).rev().find(|&i| fits(i)) } else { (0..hay.len()).find(|&i| fits(i)) }?;
        let start = start.min(self.len());
        Some((start + found, start + found + needle.len()))
    }
}

/// Where a field's range is on the screen, as its bounding rectangles (left, top, width, height): the field's box
/// (the caret's where the range is collapsed and the caret's box is known), as AT-SPI's text extents are.
pub fn range_rects(field: [f64; 4], caret: Option<[f64; 4]>, collapsed: bool) -> Vec<[f64; 4]> {
    match caret {
        Some(c) if collapsed => vec![c],
        _ => vec![field],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::Window;
    use kompoz_graphics::Rect;
    use kompoz_ui::semantics::{CollectionInfo, CollectionItemInfo};

    fn node(id: u64, parent: Option<u64>) -> AccessibilityNode {
        AccessibilityNode { id, parent, enabled: true, ..Default::default() }
    }

    fn tree(nodes: Vec<AccessibilityNode>) -> Snapshot {
        Snapshot::new(nodes, Window::default())
    }

    #[test]
    fn roles_are_control_types() {
        let mut nodes = Vec::new();
        let mut add = |id: u64, parent: Option<u64>, f: &dyn Fn(&mut AccessibilityNode)| {
            let mut n = node(id, parent);
            f(&mut n);
            nodes.push(n);
        };
        add(1, None, &|n| {
            n.label = "OK".into();
            n.clickable = true;
        });
        add(2, None, &|n| {
            n.role = Some(Role::Switch);
            n.toggleable_state = Some(ToggleableState::On);
        });
        add(3, None, &|n| {
            n.editable = true;
            n.password = true;
            n.value = "abc".into();
        });
        add(4, None, &|n| {
            n.range = Some(ProgressBarRangeInfo::new(3.0, 0.0..=10.0, 4));
            n.settable = true;
        });
        add(5, None, &|n| n.range = Some(ProgressBarRangeInfo::Indeterminate));
        add(6, None, &|n| {
            n.heading = true;
            n.label = "Başlık".into();
        });
        add(7, None, &|n| {
            n.is_dialog = true;
            n.pane_title = Some("Ayarlar".into());
        });
        add(8, None, &|n| n.selectable_group = true);
        add(9, Some(8), &|n| {
            n.role = Some(Role::Tab);
            n.selected = Some(true);
        });
        add(10, None, &|n| n.selectable_group = true);
        add(11, Some(10), &|n| n.selected = Some(false));
        add(12, None, &|n| n.collection_info = Some(CollectionInfo { row_count: 3, column_count: 1 }));
        add(13, Some(12), &|n| {
            n.label = "Bir".into();
            n.collection_item_info =
                Some(CollectionItemInfo { row_index: 1, row_span: 1, column_index: 0, column_span: 1 });
        });
        add(14, None, &|n| {
            n.label = "Liste".into();
            n.scrollable = true;
        });
        add(15, Some(14), &|n| n.label = "Öğe".into());
        add(16, None, &|n| {
            n.link = true;
            n.clickable = true;
            n.label = "Site".into();
        });
        add(17, None, &|n| n.label = "Kart".into());
        add(18, Some(17), &|n| n.label = "İçinde".into());
        add(19, None, &|n| n.role = Some(Role::DropdownList));
        add(20, None, &|n| n.toggleable_state = Some(ToggleableState::Indeterminate));
        let t = tree(nodes);
        let types: Vec<i32> = t.nodes().iter().map(|n| control_type_of(&t, n)).collect();
        use control::*;
        assert_eq!(
            types,
            [
                BUTTON,
                BUTTON,
                EDIT,
                SLIDER,
                PROGRESS_BAR,
                TEXT,
                WINDOW,
                TAB,
                TAB_ITEM,
                LIST,
                LIST_ITEM,
                LIST,
                LIST_ITEM,
                PANE,
                TEXT,
                HYPERLINK,
                GROUP,
                TEXT,
                COMBO_BOX,
                CHECK_BOX
            ]
        );
        let p = |id: u64, property: i32| property_of(&t, t.node(id), property);
        assert_eq!(p(2, property::LOCALIZED_CONTROL_TYPE), Prop::Str("toggle switch".into()));
        assert_eq!(p(7, property::LOCALIZED_CONTROL_TYPE), Prop::Str("dialog".into()));
        assert_eq!((p(7, property::IS_DIALOG), p(7, property::NAME)), (Prop::Bool(true), Prop::Str("Ayarlar".into())));
        assert_eq!(p(6, property::HEADING_LEVEL), Prop::Int(HEADING_LEVEL_1));
        assert_eq!(p(1, property::HEADING_LEVEL), Prop::Empty);
        assert_eq!(p(3, property::IS_PASSWORD), Prop::Bool(true));
        assert_eq!(p(1, property::AUTOMATION_ID), Prop::Str("1".into()));
        assert_eq!(p(1, property::FRAMEWORK_ID), Prop::Str(FRAMEWORK.into()));
        assert_eq!(p(1, property::IS_KEYBOARD_FOCUSABLE), Prop::Bool(true));
        assert_eq!(p(15, property::IS_KEYBOARD_FOCUSABLE), Prop::Bool(false));
        assert_eq!((p(13, property::POSITION_IN_SET), p(13, property::SIZE_OF_SET)), (Prop::Int(2), Prop::Int(3)));
        assert_eq!(p(15, property::NAME), Prop::Str("Öğe".into()));
        assert_eq!(p(17, property::ITEM_STATUS), Prop::Empty);
    }

    #[test]
    fn patterns_follow_what_nodes_do() {
        let mut button = node(1, None);
        button.clickable = true;
        let mut check = node(2, None);
        check.clickable = true;
        check.role = Some(Role::Checkbox);
        check.toggleable_state = Some(ToggleableState::Indeterminate);
        let mut tab = node(3, Some(4));
        tab.clickable = true;
        tab.role = Some(Role::Tab);
        tab.checked = Some(false); // (as AT-SPI's era told a tab: checked)
        let mut field = node(5, None);
        field.editable = true;
        field.clickable = true;
        let mut menu = node(6, None);
        menu.expanded = Some(false);
        let has = |n: &AccessibilityNode| -> Vec<i32> {
            use pattern::*;
            [INVOKE, SELECTION, VALUE, RANGE_VALUE, SCROLL, EXPAND_COLLAPSE, SELECTION_ITEM, TEXT, TOGGLE, TEXT2]
                .into_iter()
                .filter(|&p| has_pattern(n, p))
                .collect()
        };
        use pattern::*;
        assert_eq!(has(&button), [INVOKE]);
        assert_eq!(has(&check), [TOGGLE]);
        assert_eq!(toggle_of(&check), Some(2));
        assert_eq!(has(&tab), [SELECTION_ITEM]);
        assert_eq!(selected_of(&tab), Some(false));
        assert_eq!(has(&field), [VALUE, TEXT, TEXT2]);
        assert_eq!(has(&menu), [EXPAND_COLLAPSE]);
        assert_eq!(invoke(&menu), SemanticsAction::Expand);
        assert_eq!(invoke(&button), SemanticsAction::Click);
        assert_eq!(expand_collapse_state(&menu), 0);
        let mut group = node(4, None);
        group.selectable_group = true;
        let mut other = tab.clone();
        other.id = 7;
        other.checked = Some(true);
        let t = tree(vec![group, tab, other]);
        assert_eq!(selection_of(&t, t.node(4)), [7]);
        assert_eq!(selection_container(&t, t.node(3)), Some(4));
        assert_eq!(selection_container(&t, t.node(4)), None);
    }

    #[test]
    fn ranges_and_scrolls_are_set() {
        let mut slider = node(1, None);
        slider.range = Some(ProgressBarRangeInfo::new(3.0, 0.0..=10.0, 4));
        slider.settable = true;
        assert_eq!(range_numbers(&slider), Some([3.0, 0.0, 10.0, 2.0, 1.0]));
        assert_eq!(set_range_value(&slider, 7.0), Ok(SemanticsAction::SetProgress(7.0)));
        assert_eq!(set_range_value(&slider, 11.0), Err(RangeRefusal::OutOfRange));
        slider.settable = false;
        assert_eq!(set_range_value(&slider, 7.0), Err(RangeRefusal::ReadOnly));
        assert_eq!(
            range_numbers(&AccessibilityNode { range: Some(ProgressBarRangeInfo::Indeterminate), ..node(2, None) }),
            None
        );

        let mut list = node(3, None);
        list.scrollable = true;
        list.bounds = Rect::new(0.0, 0.0, 200.0, 300.0);
        // (A scrollable that doesn't say where it is: up and down, of no known place.)
        assert!(scrolls(&list, Axis::Vertical) && !scrolls(&list, Axis::Horizontal));
        assert_eq!((scroll_percent(&list, Axis::Vertical), view_size(&list, Axis::Vertical)), (NO_SCROLL, 100.0));
        assert_eq!(scroll_by(&list, 2, 0), Ok(Some(SemanticsAction::ScrollBy { x: 0.0, y: -300.0 })));
        assert_eq!(scroll_by(&list, 4, 2), Err(()));
        assert_eq!(scroll_to_percent(&list, NO_SCROLL, 50.0), None);
        list.vertical_scroll = Some(ScrollPosition { value: 250.0, max_value: 1000.0, reverse_scrolling: false });
        assert_eq!(scroll_percent(&list, Axis::Vertical), 25.0);
        assert_eq!(view_size(&list, Axis::Vertical), 300.0 / 1300.0 * 100.0);
        assert_eq!(scroll_by(&list, 2, 4), Ok(Some(SemanticsAction::ScrollBy { x: 0.0, y: 30.0 })));
        assert_eq!(scroll_by(&list, 2, 2), Ok(None));
        assert_eq!(scroll_to_percent(&list, NO_SCROLL, 50.0), Some(SemanticsAction::ScrollBy { x: 0.0, y: 250.0 }));
        assert_eq!(scroll_to_percent(&list, NO_SCROLL, 101.0), None);
        list.vertical_scroll = Some(ScrollPosition { value: 250.0, max_value: 1000.0, reverse_scrolling: true });
        assert_eq!(scroll_percent(&list, Axis::Vertical), 75.0);
        // (To 50% of a reversed one: its value to 500, which is up the screen.)
        assert_eq!(scroll_to_percent(&list, NO_SCROLL, 50.0), Some(SemanticsAction::ScrollBy { x: 0.0, y: -250.0 }));
    }

    #[test]
    fn changes_are_raised_as_ui_automation_says_them() {
        let mut before =
            vec![node(1, None), node(2, None), node(3, None), node(4, Some(3)), node(5, None), node(6, None)];
        before[0].label = "OK".into();
        before[0].clickable = true;
        before[1].role = Some(Role::Checkbox);
        before[1].toggleable_state = Some(ToggleableState::Off);
        before[2].label = "Liste".into();
        before[2].scrollable = true;
        before[2].vertical_scroll = Some(ScrollPosition { value: 0.0, max_value: 100.0, reverse_scrolling: false });
        before[4].editable = true;
        before[4].value = "héllo".into();
        before[4].text_selection = Some((5, 5));
        before[5].role = Some(Role::Tab);
        before[5].selected = Some(false);
        before[5].expanded = Some(false);
        let mut after = before.clone();
        after.remove(3); // (the list's item)
        after[0].label = "Tamam".into();
        after[0].live_region = Some(LiveRegionMode::Polite);
        after[1].toggleable_state = Some(ToggleableState::On);
        after[1].enabled = false;
        after[2].vertical_scroll = Some(ScrollPosition { value: 50.0, max_value: 100.0, reverse_scrolling: false });
        after[3].value = "süt".into();
        after[3].text_selection = Some((3, 3));
        after[3].focused = true;
        after[4].selected = Some(true);
        after[4].expanded = Some(true);
        let mut dialog = node(7, None);
        dialog.is_dialog = true;
        after.push(dialog);
        let (old, next) = (tree(before), tree(after));
        use property::*;
        let s = |v: &str| Prop::Str(v.into());
        assert_eq!(
            raises(&old, &next),
            [
                Raise::Structure { node: Some(3), change: CHILD_REMOVED, child: 4 },
                Raise::Structure { node: Some(7), change: CHILD_ADDED, child: 7 },
                Raise::Event { node: 7, event: event::WINDOW_OPENED },
                Raise::Property { node: 1, property: NAME, old: s("OK"), new: s("Tamam") },
                Raise::Event { node: 1, event: event::LIVE_REGION_CHANGED },
                Raise::Property { node: 2, property: IS_ENABLED, old: Prop::Bool(true), new: Prop::Bool(false) },
                Raise::Property { node: 5, property: VALUE, old: s("héllo"), new: s("süt") },
                Raise::Event { node: 5, event: event::TEXT_CHANGED },
                Raise::Property { node: 2, property: TOGGLE_STATE, old: Prop::Int(0), new: Prop::Int(1) },
                Raise::Property {
                    node: 3,
                    property: VERTICAL_SCROLL_PERCENT,
                    old: Prop::Double(0.0),
                    new: Prop::Double(50.0)
                },
                Raise::Event { node: 5, event: event::TEXT_SELECTION_CHANGED },
                Raise::Property { node: 6, property: IS_SELECTED, old: Prop::Bool(false), new: Prop::Bool(true) },
                Raise::Event { node: 6, event: event::ELEMENT_SELECTED },
                Raise::Property { node: 6, property: EXPAND_COLLAPSE_STATE, old: Prop::Int(0), new: Prop::Int(1) },
                Raise::Property {
                    node: 5,
                    property: HAS_KEYBOARD_FOCUS,
                    old: Prop::Bool(false),
                    new: Prop::Bool(true)
                },
                Raise::Event { node: 5, event: event::FOCUS_CHANGED },
            ]
        );
        // The first tree: only the focus.
        assert_eq!(
            raises(&Snapshot::default(), &next),
            [
                Raise::Property {
                    node: 5,
                    property: HAS_KEYBOARD_FOCUS,
                    old: Prop::Bool(false),
                    new: Prop::Bool(true)
                },
                Raise::Event { node: 5, event: event::FOCUS_CHANGED },
            ]
        );
    }

    #[test]
    fn text_ranges_move_by_characters_words_lines_and_the_whole() {
        use TextUnit::*;
        let t = Text::new("héllo wörld\nikinci satır");
        assert_eq!(t.len(), 24);
        assert_eq!(t.slice(1, 5), "éllo");
        assert_eq!(t.enclosing(7, Word), (6, 12)); // (a word with what follows it: here the line break)
        assert_eq!(t.enclosing(3, Line), (0, 12));
        assert_eq!(t.enclosing(15, Line), (12, 24));
        assert_eq!(t.enclosing(24, Character), (23, 24));
        assert_eq!(t.enclosing(5, Document), (0, 24));
        // Ends moved by units, stopping at the ends.
        assert_eq!(t.move_endpoint(0, Word, 2), (12, 2));
        assert_eq!(t.move_endpoint(12, Word, -1), (6, -1));
        assert_eq!(t.move_endpoint(3, Word, -5), (0, -1));
        assert_eq!(t.move_endpoint(20, Line, 3), (24, 1));
        assert_eq!(t.move_endpoint(2, Character, 3), (5, 3));
        assert_eq!(t.move_endpoint(2, Document, 1), (24, 1));
        // Ranges: a collapsed one stays collapsed; another is the whole unit its start is in, moved.
        assert_eq!(t.move_range(3, 3, Character, 2), (5, 5, 2));
        assert_eq!(t.move_range(1, 3, Word, 1), (6, 12, 1));
        assert_eq!(t.move_range(7, 9, Word, 5), (19, 24, 2));
        assert_eq!(t.move_range(13, 14, Line, -1), (0, 12, -1));
        assert_eq!(t.move_range(13, 14, Line, 1), (12, 24, 0));
        assert_eq!(TextUnit::from_uia(1), Word);
        assert_eq!(TextUnit::from_uia(4), Line);
        assert_eq!(TextUnit::from_uia(5), Document);
        assert_eq!(t.find(0, 24, "SATır", false, true), Some((19, 24)));
        assert_eq!(t.find(0, 24, "l", true, false), Some((9, 10)));
        assert_eq!(t.find(0, 5, "wörld", false, false), None);
        let empty = Text::new("");
        assert_eq!((empty.enclosing(0, Word), empty.move_endpoint(0, Character, 1)), ((0, 0), (0, 0)));

        let mut field = node(1, None);
        field.editable = true;
        field.value = "çay".into();
        assert_eq!(caret_of(&field), ((3, 3), 3));
        field.text_selection = Some((3, 1));
        assert_eq!(caret_of(&field), ((1, 3), 1));
        field.text_selection = Some((9, 9));
        assert_eq!(caret_of(&field), ((3, 3), 3));
        let (box_, caret) = ([10.0, 20.0, 200.0, 40.0], [50.0, 25.0, 1.0, 30.0]);
        assert_eq!(range_rects(box_, Some(caret), true), [caret]);
        assert_eq!(range_rects(box_, Some(caret), false), [box_]);
        assert_eq!(range_rects(box_, None, true), [box_]);
        assert_eq!(runtime_id(0x1_0000_0002), [APPEND_RUNTIME_ID, 1, 2]);
    }
}
