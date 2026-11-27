//! What NSAccessibility says of the semantics tree (the AT-SPI bridge's parity in AppKit's terms), apart from
//! AppKit's objects (accessibility.rs makes those): each node's role and subrole (`AXButton`, `AXCheckBox` with
//! `AXSwitch`, `AXRadioButton` with `AXTabButton` in an `AXTabGroup`, `AXSlider`, `AXProgressIndicator`,
//! `AXTextField` with `AXSecureTextField`, `AXTextArea`, `AXStaticText`, `AXImage`, `AXLink`, `AXList`/`AXGrid` and
//! their `AXRow`s and `AXCell`s, `AXHeading`, a dialog's and a pane's `AXGroup`), its label, value, value range,
//! help and states, the actions it offers and the semantics action each is ([`action_for`]), the node under a point
//! ([`hit_test`]); and what changed from one published tree to the next as AppKit's notifications ([`notices`]: the
//! shared [`changes`] — focus, values, texts, titles, subtrees — with the focused field's selection, live regions'
//! and panes' announcements).
//!
//! AppKit's names are its constants' strings (`NSAccessibilityButtonRole` is `AXButton`; a test on macOS checks
//! each against AppKit's own), so that all of this is plain Rust, tested on every host.

use super::text_input::{byte_of_utf16, utf16_len, utf16_of_byte};
use crate::accessibility::text::{layout_for, line_around, lines_of, range_box, text_of};
use crate::accessibility::{Change, Snapshot, changes};
use kompoz_graphics::{Offset, Rect};
use kompoz_ui::semantics::{AccessibilityNode, ProgressBarRangeInfo, Role, SemanticsAction, ToggleableState};

/// AppKit's roles (`NSAccessibility…Role`).
pub mod role {
    pub const BUTTON: &str = "AXButton";
    pub const CHECK_BOX: &str = "AXCheckBox";
    pub const RADIO_BUTTON: &str = "AXRadioButton";
    pub const RADIO_GROUP: &str = "AXRadioGroup";
    pub const TAB_GROUP: &str = "AXTabGroup";
    pub const SLIDER: &str = "AXSlider";
    pub const PROGRESS_INDICATOR: &str = "AXProgressIndicator";
    pub const INCREMENTOR: &str = "AXIncrementor";
    pub const POP_UP_BUTTON: &str = "AXPopUpButton";
    pub const TEXT_FIELD: &str = "AXTextField";
    pub const TEXT_AREA: &str = "AXTextArea";
    pub const STATIC_TEXT: &str = "AXStaticText";
    pub const IMAGE: &str = "AXImage";
    pub const LINK: &str = "AXLink";
    pub const LIST: &str = "AXList";
    pub const GRID: &str = "AXGrid";
    pub const ROW: &str = "AXRow";
    pub const CELL: &str = "AXCell";
    pub const GROUP: &str = "AXGroup";
    pub const SCROLL_AREA: &str = "AXScrollArea";
    /// (No constant of AppKit's: WebKit's heading role, which VoiceOver's rotor lists.)
    pub const HEADING: &str = "AXHeading";
}

/// AppKit's subroles (`NSAccessibility…Subrole`).
pub mod subrole {
    pub const SWITCH: &str = "AXSwitch";
    pub const TAB_BUTTON: &str = "AXTabButton";
    pub const SECURE_TEXT_FIELD: &str = "AXSecureTextField";
    pub const TABLE_ROW: &str = "AXTableRow";
    pub const CONTENT_LIST: &str = "AXContentList";
    /// (No constant of AppKit's: WebKit's for a dialog inside a window's content.)
    pub const APPLICATION_DIALOG: &str = "AXApplicationDialog";
}

/// AppKit's actions (`NSAccessibility…Action`) and the scroll by a page VoiceOver asks of a scroll area.
pub mod action {
    pub const PRESS: &str = "AXPress";
    pub const INCREMENT: &str = "AXIncrement";
    pub const DECREMENT: &str = "AXDecrement";
    pub const SHOW_MENU: &str = "AXShowMenu";
    pub const CONFIRM: &str = "AXConfirm";
    pub const CANCEL: &str = "AXCancel";
    pub const SCROLL_UP_BY_PAGE: &str = "AXScrollUpByPage";
    pub const SCROLL_DOWN_BY_PAGE: &str = "AXScrollDownByPage";
    pub const SCROLL_LEFT_BY_PAGE: &str = "AXScrollLeftByPage";
    pub const SCROLL_RIGHT_BY_PAGE: &str = "AXScrollRightByPage";
}

/// AppKit's notifications (`NSAccessibility…Notification`).
pub mod notification {
    pub const FOCUSED_UI_ELEMENT_CHANGED: &str = "AXFocusedUIElementChanged";
    pub const VALUE_CHANGED: &str = "AXValueChanged";
    pub const SELECTED_TEXT_CHANGED: &str = "AXSelectedTextChanged";
    pub const SELECTED_CHILDREN_CHANGED: &str = "AXSelectedChildrenChanged";
    pub const TITLE_CHANGED: &str = "AXTitleChanged";
    pub const LAYOUT_CHANGED: &str = "AXLayoutChanged";
    pub const CREATED: &str = "AXCreated";
    pub const UI_ELEMENT_DESTROYED: &str = "AXUIElementDestroyed";
    pub const ANNOUNCEMENT_REQUESTED: &str = "AXAnnouncementRequested";
}

/// The attribute WebKit says an invalid field with (ARIA's `aria-invalid`): no property of the protocol's says it.
pub const INVALID_ATTRIBUTE: &str = "AXInvalid";

/// What a slider's or a picker's increment steps it by without steps of its own: a twentieth of its range (Compose's
/// `AccessibilityRangeStepCount`, as the web's mirror).
const RANGE_STEPS: f32 = 20.0;

/// What a node is to AppKit: its role, its subrole, and what it is called where AppKit has no description of that
/// pair of its own (`NSAccessibilityRoleDescription` gives an empty one: a heading, a dialog's group).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Kind {
    pub role: &'static str,
    pub subrole: Option<&'static str>,
    pub description: Option<&'static str>,
}

impl Kind {
    const fn of(role: &'static str) -> Kind {
        Kind { role, subrole: None, description: None }
    }

    const fn sub(role: &'static str, subrole: &'static str) -> Kind {
        Kind { role, subrole: Some(subrole), description: None }
    }
}

/// A collection's shape: a list (one row or one column) or a grid (more of both, or a lazy grid's unknown ones).
fn is_grid(rows: i32, columns: i32) -> bool {
    rows != 1 && columns != 1
}

/// What `n` (of `tree`) is: its explicit role first, then what it is and does (as the AT-SPI bridge and the web's
/// mirror decide); a field multi-line (`multi_line`: as its input method was told, or its text has lines) an
/// `AXTextArea`.
pub fn kind_of(tree: &Snapshot, n: &AccessibilityNode, multi_line: bool) -> Kind {
    match n.role {
        Some(Role::Button) => return Kind::of(role::BUTTON),
        Some(Role::Checkbox) => return Kind::of(role::CHECK_BOX),
        Some(Role::Switch) => return Kind::sub(role::CHECK_BOX, subrole::SWITCH),
        Some(Role::RadioButton) => return Kind::of(role::RADIO_BUTTON),
        Some(Role::Tab) => return Kind::sub(role::RADIO_BUTTON, subrole::TAB_BUTTON),
        Some(Role::Image) => return Kind::of(role::IMAGE),
        Some(Role::DropdownList) => return Kind::of(role::POP_UP_BUTTON),
        Some(Role::ValuePicker) => return Kind::of(role::INCREMENTOR),
        Some(Role::Carousel) => return Kind { description: Some("carousel"), ..Kind::of(role::GROUP) },
        None => {}
    }
    if n.editable {
        if n.password {
            return Kind::sub(role::TEXT_FIELD, subrole::SECURE_TEXT_FIELD);
        }
        return Kind::of(if multi_line { role::TEXT_AREA } else { role::TEXT_FIELD });
    }
    if n.is_dialog {
        return Kind { description: Some("dialog"), ..Kind::sub(role::GROUP, subrole::APPLICATION_DIALOG) };
    }
    if n.heading {
        return Kind { description: Some("heading"), ..Kind::of(role::HEADING) };
    }
    if n.link {
        return Kind::of(role::LINK);
    }
    if let Some(r) = &n.range {
        let slider = n.settable && *r != ProgressBarRangeInfo::Indeterminate;
        return Kind::of(if slider { role::SLIDER } else { role::PROGRESS_INDICATOR });
    }
    let kids = tree.children(Some(n.id));
    if n.selectable_group {
        let any = |f: &dyn Fn(&AccessibilityNode) -> bool| kids.iter().any(|&k| f(tree.node(k)));
        if any(&|k| k.role == Some(Role::Tab)) {
            return Kind::of(role::TAB_GROUP);
        }
        if any(&|k| k.role == Some(Role::RadioButton)) {
            return Kind::of(role::RADIO_GROUP);
        }
        if any(&|k| k.role.is_none() && k.selected.is_some()) {
            return Kind::sub(role::LIST, subrole::CONTENT_LIST);
        }
    }
    if let Some(c) = n.collection_info {
        return if is_grid(c.row_count, c.column_count) {
            Kind::of(role::GRID)
        } else {
            Kind::sub(role::LIST, subrole::CONTENT_LIST)
        };
    }
    if n.toggleable_state.is_some() {
        return Kind::of(role::CHECK_BOX);
    }
    if let Some(parent) = n.parent.map(|p| tree.node(p)) {
        let options = parent.selectable_group && n.selected.is_some();
        if options && kind_of(tree, parent, false).role == role::LIST {
            return Kind::sub(role::ROW, subrole::TABLE_ROW);
        }
        if let Some(c) = parent.collection_info {
            return if is_grid(c.row_count, c.column_count) {
                Kind::of(role::CELL)
            } else {
                Kind::sub(role::ROW, subrole::TABLE_ROW)
            };
        }
    }
    if n.clickable {
        return Kind::of(role::BUTTON);
    }
    if n.scrollable {
        return Kind::of(role::SCROLL_AREA);
    }
    // A leaf that says something: a piece of text; anything else (a pane's, a named group's, a plain one) a group.
    if kids.is_empty() && !n.label.is_empty() && n.pane_title.is_none() {
        return Kind::of(role::STATIC_TEXT);
    }
    Kind::of(role::GROUP)
}

/// Its label (`accessibilityLabel`): what it says (a pane's title where it says nothing else); none for a piece of
/// text, whose value says it (VoiceOver would read it twice), nor for a field (its label is its title: see
/// [`title_of`]).
pub fn label_of(n: &AccessibilityNode, kind: &Kind) -> Option<String> {
    if kind.role == role::STATIC_TEXT {
        return None;
    }
    let label = if n.label.is_empty() { n.pane_title.clone().unwrap_or_default() } else { n.label.clone() };
    (!label.is_empty()).then_some(label)
}

/// A node's value (`accessibilityValue`).
#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    Number(f64),
    Text(String),
}

/// Its value: a toggle's state (0, 1, 2 for mixed), a radio button's or a tab's selection, a range's current value,
/// a field's text (a password's bullets), a piece of text's text; none for the rest.
pub fn value_of(n: &AccessibilityNode, kind: &Kind) -> Option<Value> {
    let flag = |b: bool| Some(Value::Number(if b { 1.0 } else { 0.0 }));
    match kind.role {
        role::CHECK_BOX => match n.toggleable_state {
            Some(ToggleableState::On) => flag(true),
            Some(ToggleableState::Off) => flag(false),
            Some(ToggleableState::Indeterminate) => Some(Value::Number(2.0)),
            None => flag(n.selected.or(n.checked).unwrap_or(false)),
        },
        role::RADIO_BUTTON => flag(n.selected.or(n.checked).unwrap_or(false)),
        role::SLIDER | role::PROGRESS_INDICATOR | role::INCREMENTOR => n
            .range
            .as_ref()
            .filter(|r| **r != ProgressBarRangeInfo::Indeterminate)
            .map(|r| Value::Number(r.current as f64)),
        role::TEXT_FIELD | role::TEXT_AREA => Some(Value::Text(text_of(n))),
        role::STATIC_TEXT => Some(Value::Text(n.label.clone())),
        _ => None,
    }
}

/// A range's bounds (min, max), a determinate one's.
pub fn bounds_of(n: &AccessibilityNode) -> Option<(f64, f64)> {
    let r = n.range.as_ref().filter(|r| **r != ProgressBarRangeInfo::Indeterminate)?;
    Some((*r.range.start() as f64, *r.range.end() as f64))
}

/// What a range's value says (`accessibilityValueDescription`: its state description, ARIA's `aria-valuetext`).
pub fn value_description_of(n: &AccessibilityNode) -> Option<String> {
    (n.range.is_some() && !n.state_description.is_empty()).then(|| n.state_description.clone())
}

/// Its help (`accessibilityHelp`, what VoiceOver says after a pause): its state description (a range's is its
/// value's) and what is wrong with its input, comma separated (as the web's `aria-description`).
pub fn help_of(n: &AccessibilityNode) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    if !n.state_description.is_empty() && n.range.is_none() {
        parts.push(&n.state_description);
    }
    if let Some(e) = n.error.as_deref().filter(|e| !e.is_empty()) {
        parts.push(e);
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// Its title (`accessibilityTitle`): a field's label (AppKit's fields are titled by their labels: their value is
/// their text).
pub fn title_of(n: &AccessibilityNode, kind: &Kind) -> Option<String> {
    let field = matches!(kind.role, role::TEXT_FIELD | role::TEXT_AREA);
    (field && !n.label.is_empty()).then(|| n.label.clone())
}

/// Selected (`isAccessibilitySelected`): a tab's, an option's, a row's or a cell's selection.
pub fn selected_of(n: &AccessibilityNode, kind: &Kind) -> bool {
    match kind.role {
        role::RADIO_BUTTON | role::ROW | role::CELL => n.selected.or(n.checked).unwrap_or(false),
        _ => n.selected.unwrap_or(false),
    }
}

/// What a slider's increment and decrement step it by.
pub fn step_of(n: &AccessibilityNode) -> Option<f32> {
    let r = n.range.as_ref().filter(|r| **r != ProgressBarRangeInfo::Indeterminate && n.settable)?;
    let span = r.range.end() - r.range.start();
    Some(if r.steps > 0 { span / (r.steps + 1) as f32 } else { span / RANGE_STEPS })
}

/// A collection's rows and columns (`accessibilityRowCount`, `accessibilityColumnCount`; none where unknown: a
/// lazy one's), a list's items its rows.
pub fn counts_of(n: &AccessibilityNode) -> Option<(Option<usize>, Option<usize>)> {
    let c = n.collection_info?;
    let known = |v: i32| usize::try_from(v).ok();
    if is_grid(c.row_count, c.column_count) {
        return Some((known(c.row_count), known(c.column_count)));
    }
    let size = if c.row_count == 1 { c.column_count } else { c.row_count };
    Some((known(size), Some(1)))
}

/// An item's place (`accessibilityIndex` of a row; a cell's `accessibilityRowIndexRange` and
/// `accessibilityColumnIndexRange`: (row, row span, column, column span)), in the collection above it or among the
/// options of a selectable group.
pub fn place_of(tree: &Snapshot, n: &AccessibilityNode) -> Option<(usize, usize, usize, usize)> {
    let i = n.collection_item_info?;
    let along_row = n.parent.and_then(|p| tree.node(p).collection_info).is_some_and(|c| c.row_count == 1);
    let (row, column) = if along_row { (i.column_index, i.row_index) } else { (i.row_index, i.column_index) };
    let at = |v: i32| usize::try_from(v).unwrap_or(0);
    let span = |v: i32| usize::try_from(v).unwrap_or(1).max(1);
    Some((at(row), span(i.row_span), at(column), span(i.column_span)))
}

/// The actions it offers (`accessibilityActionNames`), in order.
pub fn actions_of(n: &AccessibilityNode, kind: &Kind) -> Vec<&'static str> {
    let mut out = Vec::new();
    if n.clickable || n.editable || n.expanded.is_some() || matches!(kind.role, role::RADIO_BUTTON | role::CHECK_BOX) {
        out.push(action::PRESS);
    }
    if step_of(n).is_some() {
        out.extend([action::INCREMENT, action::DECREMENT]);
    }
    if n.clickable {
        out.push(action::SHOW_MENU);
    }
    if n.editable {
        out.push(action::CONFIRM);
    }
    if n.is_dialog || n.is_popup {
        out.push(action::CANCEL);
    }
    if n.scrollable {
        out.extend([
            action::SCROLL_UP_BY_PAGE,
            action::SCROLL_DOWN_BY_PAGE,
            action::SCROLL_LEFT_BY_PAGE,
            action::SCROLL_RIGHT_BY_PAGE,
        ]);
    }
    out
}

/// The semantics action AppKit's `name` on `n` is (none: not one it offers): a press its click (an expandable's
/// expanding or collapsing where it has no click of its own; a field's focus), a slider's steps its progress set, the
/// menu its long click, a field's confirmation its input method action, a dialog's cancel its dismissal, a page's
/// scroll forward or back.
pub fn action_for(n: &AccessibilityNode, kind: &Kind, name: &str) -> Option<SemanticsAction> {
    if !actions_of(n, kind).contains(&name) {
        return None;
    }
    Some(match name {
        action::PRESS => match n.expanded {
            Some(expanded) if !n.clickable => {
                if expanded {
                    SemanticsAction::Collapse
                } else {
                    SemanticsAction::Expand
                }
            }
            _ if n.editable && !n.clickable => SemanticsAction::Focus,
            _ => SemanticsAction::Click,
        },
        action::INCREMENT | action::DECREMENT => {
            let (r, step) = (n.range.as_ref()?, step_of(n)?);
            let to = if name == action::INCREMENT { r.current + step } else { r.current - step };
            SemanticsAction::SetProgress(to.clamp(*r.range.start(), *r.range.end()))
        }
        action::SHOW_MENU => SemanticsAction::LongClick,
        action::CONFIRM => SemanticsAction::ImeAction,
        action::CANCEL => SemanticsAction::Dismiss,
        action::SCROLL_UP_BY_PAGE | action::SCROLL_LEFT_BY_PAGE => SemanticsAction::ScrollBackward,
        action::SCROLL_DOWN_BY_PAGE | action::SCROLL_RIGHT_BY_PAGE => SemanticsAction::ScrollForward,
        _ => return None,
    })
}

/// A value AppKit sets (`setAccessibilityValue:`): a field's text, a slider's value (clamped).
pub fn set_value(n: &AccessibilityNode, kind: &Kind, value: &Value) -> Option<SemanticsAction> {
    match (value, kind.role) {
        (Value::Text(t), role::TEXT_FIELD | role::TEXT_AREA) if n.enabled => Some(SemanticsAction::SetText(t.clone())),
        (Value::Number(v), role::SLIDER) if n.settable => {
            let r = n.range.as_ref()?;
            Some(SemanticsAction::SetProgress((*v as f32).clamp(*r.range.start(), *r.range.end())))
        }
        _ => None,
    }
}

/// The node at `(x, y)` (px in the window) within `within`'s subtree (none: the whole tree): the last one in the
/// tree's order whose bounds hold it — the deepest, the one drawn last over its siblings.
pub fn hit_test(tree: &Snapshot, within: Option<u64>, x: f32, y: f32) -> Option<u64> {
    let inside = |id: u64| {
        let mut at = Some(id);
        while let Some(a) = at {
            if Some(a) == within {
                return true;
            }
            at = tree.node(a).parent;
        }
        within.is_none()
    };
    tree.nodes()
        .iter()
        .rev()
        .find(|n| {
            let b = n.bounds;
            x >= b.left && x < b.right && y >= b.top && y < b.bottom && inside(n.id)
        })
        .map(|n| n.id)
}

/// The line (from 0) UTF-16 offset `offset` of `text` is on (its lines split at line ends).
pub fn line_of(text: &str, offset: usize) -> usize {
    let end = byte_of_utf16(text, offset);
    text[..end].matches('\n').count()
}

/// Line `line`'s range of `text`, UTF-16 (location, length, its line end with it); none past the last.
pub fn line_range(text: &str, line: usize) -> Option<(usize, usize)> {
    let mut start = 0;
    for (i, piece) in text.split_inclusive('\n').enumerate() {
        let len = utf16_len(piece);
        if i == line {
            return Some((start, len));
        }
        start += len;
    }
    (line == 0 || (line == text.matches('\n').count() && text.ends_with('\n'))).then_some((start, 0))
}

/// The code point UTF-16 unit `unit` of `text` is in.
fn code_point_of_utf16(text: &str, unit: usize) -> i32 {
    text[..byte_of_utf16(text, unit)].chars().count() as i32
}

/// Where code point `code_point` of `text` starts, UTF-16.
fn utf16_of_code_point(text: &str, code_point: i32) -> usize {
    let byte = text.char_indices().nth(code_point.max(0) as usize).map_or(text.len(), |(i, _)| i);
    utf16_of_byte(text, byte)
}

/// Where UTF-16 range (`location`, `length`) of node `n`'s text (`text`, as it is said) is, px in the window
/// (`accessibilityFrameForRange:`): its characters' boxes together as its text layout has them (the caret's there
/// for an empty range); none where it has no layout.
pub fn frame_for_range(n: &AccessibilityNode, text: &str, location: usize, length: usize) -> Option<Rect> {
    let layout = layout_for(n, text)?;
    let len = utf16_len(text);
    let (a, b) = (location.min(len), location.saturating_add(length).min(len));
    Some(range_box(layout, code_point_of_utf16(text, a), code_point_of_utf16(text, b)))
}

/// The character (its UTF-16 index) of node `n`'s text at `point` (px in the window:
/// `accessibilityRangeForPosition:`), as its text layout has them; none where none is, or it has no layout.
pub fn index_for_position(n: &AccessibilityNode, text: &str, point: Offset) -> Option<usize> {
    let layout = layout_for(n, text)?;
    layout.character_at(point).map(|i| utf16_of_byte(text, layout.characters[i].start))
}

/// The line UTF-16 offset `offset` of node `n`'s text is on: as its text layout lays the lines out (soft wraps
/// too), else split at line ends.
pub fn line_of_laid_out(n: &AccessibilityNode, text: &str, offset: usize) -> usize {
    let Some(layout) = layout_for(n, text) else { return line_of(text, offset) };
    let cp = code_point_of_utf16(text, offset.min(utf16_len(text)));
    let (start, _) = line_around(layout, cp);
    lines_of(layout).iter().position(|&(s, _)| s == start).unwrap_or(0)
}

/// Line `line`'s UTF-16 range of node `n`'s text: as its text layout lays the lines out, else split at line ends;
/// none past the last.
pub fn line_range_laid_out(n: &AccessibilityNode, text: &str, line: usize) -> Option<(usize, usize)> {
    let Some(layout) = layout_for(n, text) else { return line_range(text, line) };
    let &(s, e) = lines_of(layout).get(line)?;
    let (a, b) = (utf16_of_code_point(text, s), utf16_of_code_point(text, e));
    Some((a, b - a))
}

/// Who a notification is posted on: a node's element, the window's view, the application.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    Node(u64),
    View,
    App,
}

/// A notification to post: its name on its target, an announcement's text and whether it is to interrupt.
#[derive(Clone, PartialEq, Debug)]
pub struct Notice {
    pub target: Target,
    pub name: &'static str,
    pub announcement: Option<(String, bool)>,
}

impl Notice {
    fn on(target: Target, name: &'static str) -> Notice {
        Notice { target, name, announcement: None }
    }

    fn say(text: String, assertive: bool) -> Notice {
        Notice {
            target: Target::App,
            name: notification::ANNOUNCEMENT_REQUESTED,
            announcement: Some((text, assertive)),
        }
    }
}

/// A node and what is below it in `tree`, the node first.
pub fn subtree(tree: &Snapshot, id: u64) -> Vec<u64> {
    let mut out = vec![id];
    let mut i = 0;
    while i < out.len() {
        out.extend_from_slice(tree.children(Some(out[i])));
        i += 1;
    }
    out
}

/// The notifications that tell what changed from `old` to `next` ([`changes`] in AppKit's terms, in their order):
/// each element of a subtree gone destroyed (its root first) and each subtree come created, the layout changed with
/// them (and as the first tree comes); a node's name its title (a piece of text's its value too), a range's
/// description, a toggle's or a selection's state, a value and a text its value; a selection among a group's its
/// parent's selected children; the focus moved; a live region's change and a pane's title coming said. The focused
/// field's selection (`selection`: before, now: the field and its UTF-16 range) changed: its selected text.
/// `multi_line` says whether the focused field is.
pub fn notices(
    old: &Snapshot,
    next: &Snapshot,
    selection: (Option<(u64, (usize, usize))>, Option<(u64, (usize, usize))>),
    multi_line: bool,
) -> Vec<Notice> {
    let mut out: Vec<Notice> = Vec::new();
    let mut push = |n: Notice| {
        if !out.contains(&n) || n.announcement.is_some() {
            out.push(n);
        }
    };
    let mut layout = !old.is_published() && !next.nodes().is_empty();
    let parent_target = |parent: Option<u64>| parent.map_or(Target::View, Target::Node);
    for change in changes(old, next) {
        match change {
            Change::Removed { node, .. } => {
                layout = true;
                for gone in subtree(old, node) {
                    push(Notice::on(Target::Node(gone), notification::UI_ELEMENT_DESTROYED));
                }
            }
            Change::Added { node, .. } => {
                layout = true;
                push(Notice::on(Target::Node(node), notification::CREATED));
            }
            // (The window's own: AppKit tells its activation and its title itself.)
            Change::WindowFocused(_) | Change::WindowTitle(_) => {}
            Change::Name { node, .. } => {
                push(Notice::on(Target::Node(node), notification::TITLE_CHANGED));
                let n = next.node(node);
                if kind_of(next, n, multi_line).role == role::STATIC_TEXT {
                    push(Notice::on(Target::Node(node), notification::VALUE_CHANGED));
                }
            }
            Change::Description { node, .. } => {
                if next.node(node).range.is_some() {
                    push(Notice::on(Target::Node(node), notification::VALUE_CHANGED));
                }
            }
            Change::Announcement { text, assertive, .. } => push(Notice::say(text, assertive)),
            Change::Checked { node, .. } => {
                push(Notice::on(Target::Node(node), notification::VALUE_CHANGED));
                let n = next.node(node);
                let kind = kind_of(next, n, multi_line);
                if matches!(kind.role, role::RADIO_BUTTON | role::ROW | role::CELL) {
                    push(Notice::on(parent_target(n.parent), notification::SELECTED_CHILDREN_CHANGED));
                }
            }
            // (NSAccessibility has no notification for it: AXEnabled is read as it is asked.)
            Change::Enabled { .. } => {}
            Change::Value { node, .. } | Change::TextRemoved { node, .. } | Change::TextInserted { node, .. } => {
                push(Notice::on(Target::Node(node), notification::VALUE_CHANGED));
            }
            // (The node that takes the focus tells it.)
            Change::FocusLeft(_) => {}
            Change::Focused(node) => push(Notice::on(Target::Node(node), notification::FOCUSED_UI_ELEMENT_CHANGED)),
        }
    }
    if let (before, Some((field, range))) = selection
        && before != Some((field, range))
        && next.contains(field)
    {
        push(Notice::on(Target::Node(field), notification::SELECTED_TEXT_CHANGED));
    }
    // A pane's title said as it comes or changes (as Android tells a pane's appearing, as the web's mirror says it).
    if old.is_published() {
        for n in next.nodes() {
            if let Some(title) = n.pane_title.as_ref().filter(|t| !t.is_empty())
                && old.get(n.id).is_none_or(|o| o.pane_title.as_ref() != Some(title))
            {
                push(Notice::say(title.clone(), false));
            }
        }
    }
    if layout {
        // (Before the rest: VoiceOver reads the tree again, then what changed in it.)
        out.insert(0, Notice::on(Target::View, notification::LAYOUT_CHANGED));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::Window;
    use kompoz_graphics::Rect;
    use kompoz_ui::semantics::{CollectionInfo, CollectionItemInfo, LiveRegionMode};

    fn node(id: u64, parent: Option<u64>) -> AccessibilityNode {
        AccessibilityNode { id, parent, enabled: true, ..Default::default() }
    }

    fn tree(nodes: Vec<AccessibilityNode>) -> Snapshot {
        Snapshot::new(nodes, Window::default())
    }

    #[test]
    fn roles_and_subroles_are_appkits() {
        let mut nodes = vec![node(1, None)];
        let mut add = |f: &dyn Fn(&mut AccessibilityNode)| {
            let mut n = node(nodes.len() as u64 + 1, Some(1));
            f(&mut n);
            nodes.push(n);
        };
        add(&|n| n.role = Some(Role::Button)); // 2
        add(&|n| n.role = Some(Role::Checkbox)); // 3
        add(&|n| n.role = Some(Role::Switch)); // 4
        add(&|n| n.role = Some(Role::RadioButton)); // 5
        add(&|n| n.role = Some(Role::Image)); // 6
        add(&|n| n.editable = true); // 7
        add(&|n| {
            n.editable = true;
            n.password = true;
        }); // 8
        add(&|n| {
            n.range = Some(ProgressBarRangeInfo::new(0.5, 0.0..=1.0, 0));
            n.settable = true;
        }); // 9
        add(&|n| n.range = Some(ProgressBarRangeInfo::new(0.5, 0.0..=1.0, 0))); // 10
        add(&|n| n.label = "Merhaba".into()); // 11
        add(&|n| {
            n.heading = true;
            n.label = "Başlık".into();
        }); // 12
        add(&|n| n.link = true); // 13
        add(&|n| n.is_dialog = true); // 14
        add(&|n| n.pane_title = Some("Ayarlar".into())); // 15
        add(&|n| n.clickable = true); // 16
        add(&|n| n.toggleable_state = Some(ToggleableState::Indeterminate)); // 17
        let t = tree(nodes);
        let kind = |id: u64| kind_of(&t, t.node(id), false);
        let pair = |id: u64| (kind(id).role, kind(id).subrole);
        assert_eq!(pair(2), (role::BUTTON, None));
        assert_eq!(pair(3), (role::CHECK_BOX, None));
        assert_eq!(pair(4), (role::CHECK_BOX, Some(subrole::SWITCH)));
        assert_eq!(pair(5), (role::RADIO_BUTTON, None));
        assert_eq!(pair(6), (role::IMAGE, None));
        assert_eq!(pair(7), (role::TEXT_FIELD, None));
        assert_eq!(kind_of(&t, t.node(7), true).role, role::TEXT_AREA);
        assert_eq!(pair(8), (role::TEXT_FIELD, Some(subrole::SECURE_TEXT_FIELD)));
        assert_eq!(pair(9), (role::SLIDER, None));
        assert_eq!(pair(10), (role::PROGRESS_INDICATOR, None));
        assert_eq!(pair(11), (role::STATIC_TEXT, None));
        assert_eq!((pair(12), kind(12).description), ((role::HEADING, None), Some("heading")));
        assert_eq!(pair(13), (role::LINK, None));
        assert_eq!(pair(14), (role::GROUP, Some(subrole::APPLICATION_DIALOG)));
        assert_eq!(pair(15), (role::GROUP, None));
        assert_eq!(pair(16), (role::BUTTON, None));
        assert_eq!(pair(17), (role::CHECK_BOX, None));
        assert_eq!(pair(1), (role::GROUP, None));
        // Labels, values, help.
        let (n11, n15) = (t.node(11), t.node(15));
        assert_eq!(label_of(n11, &kind(11)), None);
        assert_eq!(value_of(n11, &kind(11)), Some(Value::Text("Merhaba".into())));
        assert_eq!(label_of(n15, &kind(15)), Some("Ayarlar".into()));
        assert_eq!(value_of(t.node(9), &kind(9)), Some(Value::Number(0.5)));
        assert_eq!(bounds_of(t.node(9)), Some((0.0, 1.0)));
        assert_eq!(value_of(t.node(17), &kind(17)), Some(Value::Number(2.0)));
        let mut field = node(30, None);
        field.editable = true;
        field.password = true;
        field.value = "abc".into();
        field.label = "Parola".into();
        field.error = Some("Çok kısa".into());
        field.state_description = "Zorunlu".into();
        let k = kind_of(&tree(vec![field.clone()]), &field, false);
        assert_eq!(value_of(&field, &k), Some(Value::Text("•••".into())));
        assert_eq!(title_of(&field, &k), Some("Parola".into()));
        assert_eq!(help_of(&field), Some("Zorunlu, Çok kısa".into()));
    }

    #[test]
    fn groups_and_collections_are_tab_groups_lists_and_grids() {
        let mut nodes = vec![node(1, None)];
        let mut tabs = node(2, Some(1));
        tabs.selectable_group = true;
        nodes.push(tabs);
        for (id, selected) in [(3, true), (4, false)] {
            let mut t = node(id, Some(2));
            t.role = Some(Role::Tab);
            t.selected = Some(selected);
            nodes.push(t);
        }
        let mut list = node(5, Some(1));
        list.collection_info = Some(CollectionInfo { row_count: 3, column_count: 1 });
        nodes.push(list);
        let mut item = node(6, Some(5));
        item.collection_item_info =
            Some(CollectionItemInfo { row_index: 2, row_span: 1, column_index: 0, column_span: 1 });
        nodes.push(item);
        let mut grid = node(7, Some(1));
        grid.collection_info = Some(CollectionInfo { row_count: -1, column_count: 4 });
        nodes.push(grid);
        let mut cell = node(8, Some(7));
        cell.collection_item_info =
            Some(CollectionItemInfo { row_index: 1, row_span: 1, column_index: 3, column_span: 1 });
        nodes.push(cell);
        let t = tree(nodes);
        let kind = |id: u64| kind_of(&t, t.node(id), false);
        assert_eq!(kind(2).role, role::TAB_GROUP);
        assert_eq!((kind(3).role, kind(3).subrole), (role::RADIO_BUTTON, Some(subrole::TAB_BUTTON)));
        assert!(selected_of(t.node(3), &kind(3)) && !selected_of(t.node(4), &kind(4)));
        assert_eq!(value_of(t.node(3), &kind(3)), Some(Value::Number(1.0)));
        assert_eq!(kind(5).role, role::LIST);
        assert_eq!(counts_of(t.node(5)), Some((Some(3), Some(1))));
        assert_eq!(kind(6).role, role::ROW);
        assert_eq!(place_of(&t, t.node(6)), Some((2, 1, 0, 1)));
        assert_eq!(kind(7).role, role::GRID);
        assert_eq!(counts_of(t.node(7)), Some((None, Some(4))));
        assert_eq!(kind(8).role, role::CELL);
        assert_eq!(place_of(&t, t.node(8)), Some((1, 1, 3, 1)));
    }

    #[test]
    fn actions_are_semantics_actions() {
        let mut button = node(1, None);
        button.clickable = true;
        let k = Kind::of(role::BUTTON);
        assert_eq!(actions_of(&button, &k), [action::PRESS, action::SHOW_MENU]);
        assert_eq!(action_for(&button, &k, action::PRESS), Some(SemanticsAction::Click));
        assert_eq!(action_for(&button, &k, action::SHOW_MENU), Some(SemanticsAction::LongClick));
        assert_eq!(action_for(&button, &k, action::INCREMENT), None);
        let mut slider = node(2, None);
        slider.range = Some(ProgressBarRangeInfo::new(0.5, 0.0..=1.0, 0));
        slider.settable = true;
        let k = Kind::of(role::SLIDER);
        assert_eq!(actions_of(&slider, &k), [action::INCREMENT, action::DECREMENT]);
        assert_eq!(action_for(&slider, &k, action::INCREMENT), Some(SemanticsAction::SetProgress(0.55)));
        assert_eq!(set_value(&slider, &k, &Value::Number(3.0)), Some(SemanticsAction::SetProgress(1.0)));
        let mut stepped = slider.clone();
        stepped.range = Some(ProgressBarRangeInfo::new(1.0, 0.0..=1.0, 3));
        assert_eq!(action_for(&stepped, &k, action::INCREMENT), Some(SemanticsAction::SetProgress(1.0)));
        assert_eq!(action_for(&stepped, &k, action::DECREMENT), Some(SemanticsAction::SetProgress(0.75)));
        let mut expander = node(3, None);
        expander.expanded = Some(false);
        let k = Kind::of(role::GROUP);
        assert_eq!(action_for(&expander, &k, action::PRESS), Some(SemanticsAction::Expand));
        let mut field = node(4, None);
        field.editable = true;
        let k = Kind::of(role::TEXT_FIELD);
        assert_eq!(actions_of(&field, &k), [action::PRESS, action::CONFIRM]);
        assert_eq!(action_for(&field, &k, action::PRESS), Some(SemanticsAction::Focus));
        assert_eq!(action_for(&field, &k, action::CONFIRM), Some(SemanticsAction::ImeAction));
        assert_eq!(set_value(&field, &k, &Value::Text("yeni".into())), Some(SemanticsAction::SetText("yeni".into())));
        let mut scroller = node(5, None);
        scroller.scrollable = true;
        let k = Kind::of(role::SCROLL_AREA);
        assert_eq!(action_for(&scroller, &k, action::SCROLL_DOWN_BY_PAGE), Some(SemanticsAction::ScrollForward));
        assert_eq!(action_for(&scroller, &k, action::SCROLL_LEFT_BY_PAGE), Some(SemanticsAction::ScrollBackward));
        let mut dialog = node(6, None);
        dialog.is_dialog = true;
        let k = Kind::of(role::GROUP);
        assert_eq!(action_for(&dialog, &k, action::CANCEL), Some(SemanticsAction::Dismiss));
    }

    #[test]
    fn points_hit_the_deepest_node() {
        let at = |id, parent, r: Rect| AccessibilityNode { bounds: r, ..node(id, parent) };
        let t = tree(vec![
            at(1, None, Rect::new(0.0, 0.0, 100.0, 100.0)),
            at(2, Some(1), Rect::new(10.0, 10.0, 50.0, 50.0)),
            at(3, Some(2), Rect::new(20.0, 20.0, 30.0, 30.0)),
            at(4, Some(1), Rect::new(60.0, 60.0, 90.0, 90.0)),
        ]);
        assert_eq!(hit_test(&t, None, 25.0, 25.0), Some(3));
        assert_eq!(hit_test(&t, None, 15.0, 15.0), Some(2));
        assert_eq!(hit_test(&t, None, 70.0, 70.0), Some(4));
        assert_eq!(hit_test(&t, None, 200.0, 5.0), None);
        assert_eq!(hit_test(&t, Some(4), 25.0, 25.0), None);
        assert_eq!(hit_test(&t, Some(2), 15.0, 15.0), Some(2));
    }

    #[test]
    fn lines_are_counted_in_utf16() {
        let text = "ağ\n😀b\n";
        assert_eq!(line_of(text, 0), 0);
        assert_eq!(line_of(text, 3), 1);
        assert_eq!(line_of(text, 7), 2);
        assert_eq!(line_range(text, 0), Some((0, 3)));
        assert_eq!(line_range(text, 1), Some((3, 4)));
        assert_eq!(line_range(text, 2), Some((7, 0)));
        assert_eq!(line_range(text, 3), None);
        assert_eq!(line_range("", 0), Some((0, 0)));
        // Laid out (8 px a character from 10, 20, wrapped after "ağaç "): the boxes, a point's character, the lines
        // as they are laid out.
        let text = "ağaç dalı";
        let layout = crate::accessibility::text::test_layout(text, Offset::new(10.0, 20.0), 8.0, 20.0, 5);
        let field = AccessibilityNode {
            value: text.into(),
            editable: true,
            text_layout: Some(layout.into()),
            ..Default::default()
        };
        assert_eq!(frame_for_range(&field, text, 1, 2), Some(Rect::new(18.0, 20.0, 34.0, 40.0)));
        assert_eq!(frame_for_range(&field, text, 3, 3), Some(Rect::new(10.0, 20.0, 50.0, 60.0)));
        assert_eq!(index_for_position(&field, text, Offset::new(20.0, 50.0)), Some(6));
        assert_eq!(index_for_position(&field, text, Offset::new(200.0, 50.0)), None);
        assert_eq!((line_of_laid_out(&field, text, 3), line_of_laid_out(&field, text, 6)), (0, 1));
        assert_eq!(line_range_laid_out(&field, text, 1), Some((5, 4)));
        assert_eq!(line_range_laid_out(&field, text, 2), None);
        let plain = AccessibilityNode { value: "a\nb".into(), ..Default::default() };
        assert_eq!((frame_for_range(&plain, "a\nb", 0, 1), line_of_laid_out(&plain, "a\nb", 2)), (None, 1));
    }

    #[test]
    fn changes_are_appkits_notifications() {
        use notification::*;
        let mut before = vec![node(1, None), node(2, Some(1)), node(3, Some(2)), node(4, Some(1))];
        before[3].editable = true;
        before[3].value = "a".into();
        before[3].focused = true;
        let mut after = vec![node(1, None), node(4, Some(1)), node(5, Some(1)), node(6, Some(5))];
        after[1].editable = true;
        after[1].value = "ab".into();
        after[1].label = "Ad".into();
        after[1].live_region = Some(LiveRegionMode::Polite);
        after[2].pane_title = Some("Bölme".into());
        after[3].label = "metin".into();
        after[3].focused = true;
        let (old, next) = (tree(before), tree(after));
        let on = |id, name| Notice::on(Target::Node(id), name);
        assert_eq!(
            notices(&old, &next, (Some((4, (1, 0))), Some((4, (2, 0)))), false),
            [
                Notice::on(Target::View, LAYOUT_CHANGED),
                on(2, UI_ELEMENT_DESTROYED),
                on(3, UI_ELEMENT_DESTROYED),
                on(5, CREATED),
                on(4, TITLE_CHANGED),
                Notice::say("Ad".into(), false),
                on(4, VALUE_CHANGED),
                on(6, FOCUSED_UI_ELEMENT_CHANGED),
                on(4, SELECTED_TEXT_CHANGED),
                Notice::say("Bölme".into(), false),
            ]
        );
        // The first tree: its layout and its focus.
        let first = notices(&Snapshot::default(), &old, (None, None), false);
        assert_eq!(first, [Notice::on(Target::View, LAYOUT_CHANGED), on(4, FOCUSED_UI_ELEMENT_CHANGED)]);
        // A tab selected: its value, its group's selected children.
        let tab = |id, selected| AccessibilityNode {
            role: Some(Role::Tab),
            checked: Some(selected),
            selected: Some(selected),
            ..node(id, Some(1))
        };
        let mut group = node(1, None);
        group.selectable_group = true;
        let a = tree(vec![group.clone(), tab(2, true), tab(3, false)]);
        let b = tree(vec![group, tab(2, false), tab(3, true)]);
        assert_eq!(
            notices(&a, &b, (None, None), false),
            [on(2, VALUE_CHANGED), on(1, SELECTED_CHILDREN_CHANGED), on(3, VALUE_CHANGED)]
        );
        // An assertive live region interrupts.
        let mut live = node(1, None);
        live.live_region = Some(LiveRegionMode::Assertive);
        let quiet = tree(vec![live.clone()]);
        live.label = "Hata".into();
        let said = notices(&quiet, &tree(vec![live]), (None, None), false);
        assert!(said.contains(&Notice::say("Hata".into(), true)), "{said:?}");
    }
}
