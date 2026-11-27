//! What Android's accessibility is told of the semantics tree (Compose's AndroidComposeViewAccessibilityDelegateCompat;
//! C++ Kompoz's SemanticsProvider): each node of the shared [`Snapshot`] as a virtual view of KompozView — its class
//! by role, its texts, states and actions, its range, collection and place in a collection, its scroll, its text's
//! selection and lines, in reading order — and what changed from one published tree to the next as accessibility
//! events (the shared [`changes`] and what Android hears of besides: selections, scrolls, panes, errors). Kotlin's
//! KompozAccessibility reads both from one little-endian buffer each ([`encode_tree`], [`encode_events`]) and makes
//! the AccessibilityNodeInfos and AccessibilityEvents of them; the texts said for states it takes from its own
//! resources (by [`state`]'s kinds), in the system's language.
//!
//! What a node's semantics say beyond the shared node (its merged texts and descriptions apart, long clicks, the
//! focus it can take, its selection, its scroll axes, which actions it has, its text's lines) comes as its
//! [`Extras`], read from the host by `semantics`. All of it is plain Rust, tested on every host.

use crate::accessibility::{Change, Snapshot, changes};
use kompoz_graphics::{Offset, Rect};
use kompoz_ui::semantics::{AccessibilityNode, LiveRegionMode, Role};
use std::collections::HashMap;

/// The view itself (AccessibilityNodeProvider's HOST_VIEW_ID): the tree's root node.
pub const HOST_VIEW_ID: i32 = -1;
/// No virtual view (Compose's InvalidId).
pub const INVALID_ID: i32 = i32::MIN;

/// What a node is and says, as bits (KompozAccessibility's `FLAG_*`).
pub mod flag {
    pub const CHECKABLE: i32 = 1;
    pub const CHECKED: i32 = 1 << 1;
    pub const ENABLED: i32 = 1 << 2;
    pub const FOCUSABLE: i32 = 1 << 3;
    pub const FOCUSED: i32 = 1 << 4;
    pub const CLICKABLE: i32 = 1 << 5;
    pub const LONG_CLICKABLE: i32 = 1 << 6;
    pub const EDITABLE: i32 = 1 << 7;
    pub const PASSWORD: i32 = 1 << 8;
    pub const SCROLLABLE: i32 = 1 << 9;
    pub const HEADING: i32 = 1 << 10;
    pub const SELECTED: i32 = 1 << 11;
    pub const VISIBLE: i32 = 1 << 12;
    pub const SCREEN_READER_FOCUSABLE: i32 = 1 << 13;
    pub const CONTENT_INVALID: i32 = 1 << 14;
    pub const TEXT_ENTRY_KEY: i32 = 1 << 15;
    pub const IMPORTANT: i32 = 1 << 16;
    pub const SENSITIVE: i32 = 1 << 17;
    pub const DISMISSABLE: i32 = 1 << 18;
    pub const MULTI_LINE: i32 = 1 << 19;
    /// Its text's characters have bounds (EXTRA_DATA_TEXT_CHARACTER_LOCATION_KEY).
    pub const CHARACTER_LOCATIONS: i32 = 1 << 20;
    /// Its text is shown as a hint (an empty field's label).
    pub const SHOWING_HINT: i32 = 1 << 21;
}

/// What can be done to a node, as bits (KompozAccessibility maps each to its AccessibilityAction).
pub mod action {
    pub const CLICK: i32 = 1;
    pub const LONG_CLICK: i32 = 1 << 1;
    pub const FOCUS: i32 = 1 << 2;
    pub const CLEAR_FOCUS: i32 = 1 << 3;
    pub const SCROLL_FORWARD: i32 = 1 << 4;
    pub const SCROLL_BACKWARD: i32 = 1 << 5;
    pub const SCROLL_UP: i32 = 1 << 6;
    pub const SCROLL_DOWN: i32 = 1 << 7;
    pub const SCROLL_LEFT: i32 = 1 << 8;
    pub const SCROLL_RIGHT: i32 = 1 << 9;
    pub const SCROLL_TO_POSITION: i32 = 1 << 10;
    pub const SET_PROGRESS: i32 = 1 << 11;
    pub const SET_TEXT: i32 = 1 << 12;
    pub const SET_SELECTION: i32 = 1 << 13;
    pub const NEXT_AT_GRANULARITY: i32 = 1 << 14;
    pub const PREVIOUS_AT_GRANULARITY: i32 = 1 << 15;
    pub const EXPAND: i32 = 1 << 16;
    pub const COLLAPSE: i32 = 1 << 17;
    pub const DISMISS: i32 = 1 << 18;
    pub const COPY: i32 = 1 << 19;
    pub const CUT: i32 = 1 << 20;
    pub const PASTE: i32 = 1 << 21;
    pub const IME_ENTER: i32 = 1 << 22;
    pub const PAGE_UP: i32 = 1 << 23;
    pub const PAGE_DOWN: i32 = 1 << 24;
    pub const PAGE_LEFT: i32 = 1 << 25;
    pub const PAGE_RIGHT: i32 = 1 << 26;
    pub const SHOW_ON_SCREEN: i32 = 1 << 27;
}

/// Android's movement granularities (AccessibilityNodeInfo's `MOVEMENT_GRANULARITY_*`, the same values).
pub mod granularity {
    pub const CHARACTER: i32 = 1;
    pub const WORD: i32 = 2;
    pub const LINE: i32 = 4;
    pub const PARAGRAPH: i32 = 8;
    pub const PAGE: i32 = 16;
}

/// A state said where the node says none of its own (Compose's `getInfoStateDescriptionOrNull`), by kind:
/// KompozAccessibility's resources say it in the system's language.
pub mod state {
    pub const NONE: i32 = 0;
    pub const ON: i32 = 1;
    pub const OFF: i32 = 2;
    pub const INDETERMINATE: i32 = 3;
    pub const SELECTED: i32 = 4;
    pub const NOT_SELECTED: i32 = 5;
    /// "n percent" (the percent beside it).
    pub const PERCENT: i32 = 6;
    pub const IN_PROGRESS: i32 = 7;
    /// An empty field with nothing else to say.
    pub const EMPTY: i32 = 8;
}

/// What a role is said as where its class doesn't say it (Compose's `roleDescription`).
pub mod role_description {
    pub const NONE: i32 = 0;
    pub const TAB: i32 = 1;
}

/// What changed in a node, as AccessibilityEvent's `CONTENT_CHANGE_TYPE_*` bits (the same values; KompozAccessibility
/// leaves out those the system is too old for). None of them: UNDEFINED.
pub mod change {
    pub const UNDEFINED: i32 = 0;
    pub const SUBTREE: i32 = 1;
    pub const TEXT: i32 = 2;
    pub const CONTENT_DESCRIPTION: i32 = 4;
    pub const PANE_TITLE: i32 = 8;
    pub const PANE_APPEARED: i32 = 16;
    pub const PANE_DISAPPEARED: i32 = 32;
    pub const STATE_DESCRIPTION: i32 = 64;
    pub const CONTENT_INVALID: i32 = 1024;
    pub const ERROR: i32 = 2048;
    pub const ENABLED: i32 = 4096;
    pub const CHECKED: i32 = 8192;
}

/// Bits of which semantics actions a node has (as [`Extras::has`]), apart from whether it is enabled.
pub mod has {
    pub const SCROLL_BY: u32 = 1;
    pub const SCROLL_TO_INDEX: u32 = 1 << 1;
    pub const PAGE_UP: u32 = 1 << 2;
    pub const PAGE_DOWN: u32 = 1 << 3;
    pub const PAGE_LEFT: u32 = 1 << 4;
    pub const PAGE_RIGHT: u32 = 1 << 5;
    pub const COPY: u32 = 1 << 6;
    pub const CUT: u32 = 1 << 7;
    pub const PASTE: u32 = 1 << 8;
    pub const IME_ACTION: u32 = 1 << 9;
    pub const SET_TEXT: u32 = 1 << 10;
    pub const SET_SELECTION: u32 = 1 << 11;
    pub const DISMISS: u32 = 1 << 12;
    pub const SET_PROGRESS: u32 = 1 << 13;
    pub const TEXT_LAYOUT: u32 = 1 << 14;
    pub const REQUEST_FOCUS: u32 = 1 << 15;
}

/// A scrollable's axis as last read (Compose's ScrollAxisRange's values).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct ScrollAxis {
    pub value: f32,
    pub max: f32,
    pub reverse: bool,
}

impl ScrollAxis {
    /// Whether ACTION_SCROLL_FORWARD scrolls it (Compose's `canScrollForward`).
    pub fn can_scroll_forward(self) -> bool {
        (self.value < self.max && !self.reverse) || (self.value > 0.0 && self.reverse)
    }

    /// Whether ACTION_SCROLL_BACKWARD scrolls it.
    pub fn can_scroll_backward(self) -> bool {
        (self.value > 0.0 && !self.reverse) || (self.value < self.max && self.reverse)
    }
}

/// What a node's semantics say beyond its [`AccessibilityNode`] (`semantics` reads them from its merged
/// configuration): what Android's delegate reads from a node's configuration itself.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Extras {
    /// Its texts and its content descriptions, merged (what its label joins).
    pub texts: Vec<String>,
    pub descriptions: Vec<String>,
    /// It merges what is below it (a button: its texts are its own).
    pub merging: bool,
    pub long_clickable: bool,
    /// It can take the keyboard focus (it says whether it has it).
    pub focusable: bool,
    /// A field's selection, UTF-8 byte offsets in its value (start, end: end is the caret).
    pub selection: Option<(usize, usize)>,
    pub horizontal: Option<ScrollAxis>,
    pub vertical: Option<ScrollAxis>,
    /// Which actions it has ([`has`]'s bits).
    pub has: u32,
    /// Where its text's lines start (UTF-8 byte offsets in the text it is read by: its value or its first text) and
    /// their tops (px in its text's layout), as its text layout has them; none without one.
    pub lines: Option<Vec<(usize, f32)>>,
    pub max_text_length: Option<i32>,
    pub text_entry_key: bool,
    /// It shows sensitive data (only accessibility tools see it).
    pub sensitive: bool,
    /// Its layout is right to left.
    pub rtl: bool,
    /// Its touch bounds are empty (offscreen): not visible to the user.
    pub empty_bounds: bool,
    /// A field without the focus, or a text inside one (Compose's `excludeLineAndPageGranularities`).
    pub exclude_lines: bool,
    /// A field whose text is more than one line (its keyboard's Enter types a line).
    pub multi_line: bool,
}

/// A node as Android's accessibility sees it: what KompozAccessibility puts into its AccessibilityNodeInfo.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct AndroidNode {
    /// Its virtual view id ([`HOST_VIEW_ID`]: the view itself, the tree's root).
    pub id: i32,
    /// The virtual view above it (the view itself: [`HOST_VIEW_ID`]; the root's: [`INVALID_ID`], the view's own
    /// parent).
    pub parent: i32,
    /// Its semantics id.
    pub semantics_id: u64,
    pub class_name: &'static str,
    pub role_description: i32,
    pub flags: i32,
    pub actions: i32,
    pub granularities: i32,
    /// Its bounds in the view, px.
    pub bounds: Rect,
    pub text: Option<String>,
    pub content_description: Option<String>,
    /// A field's label (what it is for), said when it is empty.
    pub hint: Option<String>,
    /// Its own state description, else the kind of the one said for it (and a percent's number).
    pub state_description: Option<String>,
    pub state: i32,
    pub state_percent: i32,
    pub error: Option<String>,
    pub pane_title: Option<String>,
    pub test_tag: Option<String>,
    /// The text moved over by characters, words, lines and paragraphs (Compose's
    /// `getIterableTextForAccessibility`).
    pub iterable: Option<String>,
    /// The selection in it (UTF-16 offsets of [`iterable`](Self::iterable)).
    pub selection: Option<(i32, i32)>,
    /// Where its lines start (UTF-16 offsets of [`iterable`](Self::iterable)) and their tops (px): what lines and
    /// pages are moved over by.
    pub lines: Vec<(i32, f32)>,
    pub max_text_length: i32,
    /// 0: none, 1: polite, 2: assertive (View's `ACCESSIBILITY_LIVE_REGION_*`).
    pub live_region: i32,
    /// Its progress: min, max, current (none: no range, or an indeterminate one).
    pub range: Option<(f32, f32, f32)>,
    /// A collection's rows and columns.
    pub collection: Option<(i32, i32)>,
    /// Its place in a collection: row, row span, column, column span, selected.
    pub item: Option<(i32, i32, i32, i32, bool)>,
    /// Its scroll and how far it scrolls, horizontally and vertically (px).
    pub scroll_x: Option<(i32, i32)>,
    pub scroll_y: Option<(i32, i32)>,
    pub custom_actions: Vec<String>,
    /// Its children's virtual ids, in reading order.
    pub children: Vec<i32>,
}

/// A published tree as Android sees it: its nodes in reading order (each one's children after it), by id.
#[derive(Clone, Default, Debug)]
pub struct AndroidTree {
    nodes: Vec<AndroidNode>,
    at: HashMap<i32, usize>,
}

impl AndroidTree {
    pub fn nodes(&self) -> &[AndroidNode] {
        &self.nodes
    }

    pub fn get(&self, id: i32) -> Option<&AndroidNode> {
        self.at.get(&id).map(|&i| &self.nodes[i])
    }

    /// The virtual view of semantics node `id`, if it is in the tree.
    pub fn virtual_id(&self, semantics_id: u64) -> Option<i32> {
        self.nodes.iter().find(|n| n.semantics_id == semantics_id).map(|n| n.id)
    }

    /// The node with the keyboard focus (AccessibilityNodeInfo.FOCUS_INPUT's), if one has it.
    pub fn input_focus(&self) -> Option<i32> {
        self.nodes.iter().filter(|n| n.flags & flag::FOCUSED != 0).map(|n| n.id).next_back()
    }

    /// The virtual view under `(x, y)` (px in the view) that a screen reader's finger finds (Compose's
    /// `hitTestSemanticsAt`): the deepest node there that it would focus (the last in reading order of those as
    /// deep); else [`INVALID_ID`].
    pub fn hit(&self, x: f32, y: f32) -> i32 {
        let at = Offset::new(x, y);
        let mut depth: HashMap<i32, usize> = HashMap::new();
        let mut best: Option<(usize, i32)> = None;
        for n in &self.nodes {
            let d = depth.get(&n.parent).map_or(0, |d| d + 1);
            depth.insert(n.id, d);
            let speaks = n.flags & flag::SCREEN_READER_FOCUSABLE != 0;
            if speaks && n.id != HOST_VIEW_ID && contains(n.bounds, at) && best.is_none_or(|(b, _)| d >= b) {
                best = Some((d, n.id));
            }
        }
        best.map_or(INVALID_ID, |(_, id)| id)
    }
}

/// Whether `r` holds `p` (its right and bottom edges out).
fn contains(r: Rect, p: Offset) -> bool {
    p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom
}

/// The virtual view id of a semantics id (ids count up from 1: one past `i32::MAX` is never reached in practice;
/// such a node is left out).
pub fn virtual_id(semantics_id: u64) -> Option<i32> {
    i32::try_from(semantics_id).ok()
}

/// The UTF-16 offset of UTF-8 byte `byte` of `s` (clamped to its length, to a character's start).
pub fn utf16_at(s: &str, byte: usize) -> i32 {
    let mut b = byte.min(s.len());
    while !s.is_char_boundary(b) {
        b -= 1;
    }
    s[..b].encode_utf16().count() as i32
}

/// The UTF-8 byte offset of UTF-16 offset `unit` of `s` (clamped; inside a surrogate pair: that character's start).
pub fn byte_at(s: &str, unit: i32) -> usize {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units >= unit.max(0) as usize || units + c.len_utf16() > unit.max(0) as usize {
            return i;
        }
        units += c.len_utf16();
    }
    s.len()
}

/// Whether two items stand side by side (Compose's `calculateIfHorizontallyStacked`): their centers further apart
/// across than down, summed over neighbours.
fn horizontally_stacked(bounds: &[Rect]) -> bool {
    if bounds.len() < 2 {
        return true;
    }
    let (mut dx, mut dy) = (0.0, 0.0);
    for pair in bounds.windows(2) {
        let (a, b) = (pair[0].center(), pair[1].center());
        dx += (a.x - b.x).abs();
        dy += (a.y - b.y).abs();
    }
    dy < dx
}

/// The tree Android sees of `snapshot` (its nodes' [`Extras`] by semantics id; a node without: none of them). The
/// first root is the view itself; any other (a popup's) is one of its children.
pub fn android_tree(snapshot: &Snapshot, extras: &HashMap<u64, Extras>) -> AndroidTree {
    let none = Extras::default();
    let host = snapshot.children(None).first().copied();
    let id_of = |semantics: u64| if Some(semantics) == host { Some(HOST_VIEW_ID) } else { virtual_id(semantics) };
    let mut nodes = Vec::with_capacity(snapshot.nodes().len());
    for n in snapshot.nodes() {
        let Some(id) = id_of(n.id) else { continue };
        let parent = match n.parent {
            _ if id == HOST_VIEW_ID => INVALID_ID,
            None => HOST_VIEW_ID,
            Some(p) => match id_of(p) {
                Some(p) => p,
                None => continue,
            },
        };
        let mut children: Vec<i32> = snapshot.children(Some(n.id)).iter().filter_map(|&c| id_of(c)).collect();
        if id == HOST_VIEW_ID {
            // (Popups' roots: the view's children after its own.)
            children.extend(snapshot.children(None).iter().skip(1).filter_map(|&c| id_of(c)));
        }
        let e = extras.get(&n.id).unwrap_or(&none);
        let group: Vec<&AccessibilityNode> = if n.selectable_group && n.collection_info.is_none() {
            snapshot.children(Some(n.id)).iter().map(|&c| snapshot.node(c)).filter(|c| c.selected.is_some()).collect()
        } else {
            Vec::new()
        };
        nodes.push(android_node(n, e, id, parent, children, &group));
    }
    let at = nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();
    AndroidTree { nodes, at }
}

/// Node `n` (with `e`) as virtual view `id` under `parent`; `group`: the selectables in it where it is a selectable
/// group without collection info of its own (its collection then).
fn android_node(
    n: &AccessibilityNode,
    e: &Extras,
    id: i32,
    parent: i32,
    children: Vec<i32>,
    group: &[&AccessibilityNode],
) -> AndroidNode {
    let enabled = n.enabled;
    let texts = (!e.texts.is_empty()).then(|| e.texts.join(" "));
    let descriptions = (!e.descriptions.is_empty()).then(|| e.descriptions.join(" "));
    let leaf = children.is_empty();
    // The class (Compose's order: text, then the role, then a range's, then a scroll's).
    let mut class_name = "android.view.View";
    let mut role_description = role_description::NONE;
    if n.editable {
        class_name = "android.widget.EditText";
    } else if texts.is_some() {
        class_name = "android.widget.TextView";
    }
    match n.role {
        Some(Role::Tab) => role_description = role_description::TAB,
        // (An image is said as one only where it is focused itself, not where it is a part of something.)
        Some(Role::Image) if !(leaf || e.merging) => {}
        Some(role) => {
            if let Some(c) = legacy_class_name(role) {
                class_name = c;
            }
        }
        None => {}
    }
    let indeterminate = n.range.as_ref().is_some_and(|r| r.range.start() == r.range.end() && r.current == 0.0);
    if n.range.is_some() {
        class_name =
            if e.has & has::SET_PROGRESS != 0 { "android.widget.SeekBar" } else { "android.widget.ProgressBar" };
    }
    let collection = n.collection_info.map(|c| (c.row_count, c.column_count)).or_else(|| {
        (!group.is_empty()).then(|| {
            let bounds: Vec<Rect> = group.iter().map(|g| g.bounds).collect();
            let count = group.len() as i32;
            if horizontally_stacked(&bounds) { (1, count) } else { (count, 1) }
        })
    });
    let scrolls = e.has & has::SCROLL_BY != 0;
    // (A collection's class is TalkBack's list already: it scrolls through it by itself.)
    let plain_scroll = scrolls && collection.is_none() && !n.selectable_group;
    if plain_scroll && e.horizontal.is_some() {
        class_name = "android.widget.HorizontalScrollView";
    }
    if plain_scroll && e.vertical.is_some() {
        class_name = "android.widget.ScrollView";
    }

    // Flags.
    let mut flags = flag::IMPORTANT;
    let mut set = |f: i32, on: bool| {
        if on {
            flags |= f;
        }
    };
    let checkable = n.toggleable_state.is_some() || (n.selected.is_some() && n.role != Some(Role::Tab));
    set(flag::CHECKABLE, checkable);
    let checked = match n.toggleable_state {
        Some(kompoz_ui::state::ToggleableState::On) => true,
        Some(_) => false,
        None => n.role != Some(Role::Tab) && n.selected == Some(true),
    };
    set(flag::CHECKED, checked);
    set(flag::SELECTED, n.role == Some(Role::Tab) && n.selected == Some(true));
    set(flag::ENABLED, enabled);
    set(flag::FOCUSABLE, e.focusable);
    set(flag::FOCUSED, e.focusable && n.focused);
    // (A selected tab or radio button can't be selected again: not clickable.)
    let selected_choice = matches!(n.role, Some(Role::Tab | Role::RadioButton)) && n.selected == Some(true);
    let clickable = n.clickable && !selected_choice;
    set(flag::CLICKABLE, clickable);
    set(flag::LONG_CLICKABLE, e.long_clickable);
    set(flag::EDITABLE, n.editable);
    set(flag::PASSWORD, n.password);
    set(flag::HEADING, n.heading);
    // (Off the screen, or cut away by what clips it: not seen, as Compose's `setInvisibleIfEmptyBounds`.)
    let shown = id == HOST_VIEW_ID || (n.bounds.right > n.bounds.left && n.bounds.bottom > n.bounds.top);
    set(flag::VISIBLE, !e.empty_bounds && shown);
    set(flag::CONTENT_INVALID, n.error.is_some());
    set(flag::TEXT_ENTRY_KEY, e.text_entry_key);
    set(flag::SENSITIVE, e.sensitive);
    set(flag::DISMISSABLE, e.has & has::DISMISS != 0 && enabled);
    set(flag::MULTI_LINE, n.editable && e.multi_line);
    let scrollable_x = e.horizontal.is_some_and(|a| scrolls && a.max > 0.0);
    let scrollable_y = e.vertical.is_some_and(|a| scrolls && a.max > 0.0);
    set(flag::SCROLLABLE, scrollable_x || scrollable_y);

    // Texts: a field's text is its value, its label its hint; another's its texts and descriptions.
    let (text, content_description, hint) = if n.editable {
        let label = (!n.label.is_empty()).then(|| n.label.clone());
        (Some(n.value.clone()).filter(|v| !v.is_empty()), None, label)
    } else {
        (texts.clone(), descriptions.clone(), None)
    };
    set(flag::SHOWING_HINT, n.editable && n.value.is_empty() && hint.is_some());

    // The state said (Compose's `getInfoStateDescriptionOrNull`).
    let own = (!n.state_description.is_empty()).then(|| n.state_description.clone());
    let (mut state, mut state_percent) = (state::NONE, 0);
    if own.is_none() {
        match n.toggleable_state {
            Some(kompoz_ui::state::ToggleableState::On) if n.role == Some(Role::Switch) => state = state::ON,
            Some(kompoz_ui::state::ToggleableState::Off) if n.role == Some(Role::Switch) => state = state::OFF,
            Some(kompoz_ui::state::ToggleableState::Indeterminate) => state = state::INDETERMINATE,
            _ => {}
        }
        if state == state::NONE
            && let Some(selected) = n.selected.filter(|_| n.role != Some(Role::Tab))
        {
            state = if selected { state::SELECTED } else { state::NOT_SELECTED };
        }
        if state == state::NONE
            && let Some(r) = &n.range
        {
            if indeterminate {
                state = state::IN_PROGRESS;
            } else {
                let (lo, hi) = (*r.range.start(), *r.range.end());
                let progress = if hi - lo == 0.0 { 0.0 } else { ((r.current - lo) / (hi - lo)).clamp(0.0, 1.0) };
                state = state::PERCENT;
                state_percent = match progress {
                    0.0 => 0,
                    1.0 => 100,
                    p => ((p * 100.0).round() as i32).clamp(1, 99),
                };
            }
        }
    }
    if n.editable {
        // An empty field with nothing else to say says so (Compose's `createStateDescriptionForTextField`).
        state = if n.value.is_empty() && n.label.is_empty() { state::EMPTY } else { state::NONE };
        state_percent = 0;
    }
    // Who a screen reader stops at (Compose's `isScreenReaderFocusable`): what merges, else a leaf that says
    // something.
    let speaks = descriptions.is_some()
        || texts.is_some()
        || (n.editable && (!n.value.is_empty() || state == state::EMPTY))
        || own.is_some()
        || state != state::NONE
        || checkable;
    let screen_reader_focusable = e.merging || (speaks && leaf);
    if screen_reader_focusable {
        flags |= flag::SCREEN_READER_FOCUSABLE;
    }

    // Actions.
    let mut actions = 0;
    let mut add = |a: i32, on: bool| {
        if on {
            actions |= a;
        }
    };
    add(action::CLICK, clickable && enabled);
    add(action::LONG_CLICK, e.long_clickable && enabled);
    if e.focusable && enabled {
        add(if n.focused { action::CLEAR_FOCUS } else { action::FOCUS }, true);
    }
    add(action::COPY, e.has & has::COPY != 0);
    add(action::SET_TEXT, enabled && e.has & has::SET_TEXT != 0);
    add(action::IME_ENTER, enabled && e.has & has::IME_ACTION != 0);
    add(action::CUT, enabled && e.has & has::CUT != 0);
    // (Paste only offered to the focused field; KompozAccessibility checks the clipboard has text.)
    add(action::PASTE, enabled && e.has & has::PASTE != 0 && n.focused);
    if let Some(r) = n.range.as_ref().filter(|_| !indeterminate)
        && n.settable
    {
        let (lo, hi) = (*r.range.start(), *r.range.end());
        add(action::SCROLL_FORWARD, r.current < hi.max(lo));
        add(action::SCROLL_BACKWARD, r.current > lo.min(hi));
        add(action::SET_PROGRESS, true);
    } else if n.settable {
        add(action::SET_PROGRESS, true);
    }
    if scrolls && enabled {
        if let Some(x) = e.horizontal {
            if x.can_scroll_forward() {
                add(action::SCROLL_FORWARD, true);
                add(if e.rtl { action::SCROLL_LEFT } else { action::SCROLL_RIGHT }, true);
            }
            if x.can_scroll_backward() {
                add(action::SCROLL_BACKWARD, true);
                add(if e.rtl { action::SCROLL_RIGHT } else { action::SCROLL_LEFT }, true);
            }
        }
        if let Some(y) = e.vertical {
            if y.can_scroll_forward() {
                add(action::SCROLL_FORWARD, true);
                add(action::SCROLL_DOWN, true);
            }
            if y.can_scroll_backward() {
                add(action::SCROLL_BACKWARD, true);
                add(action::SCROLL_UP, true);
            }
        }
    }
    add(action::SCROLL_TO_POSITION, enabled && e.has & has::SCROLL_TO_INDEX != 0 && collection.is_some());
    if enabled && n.role != Some(Role::Carousel) {
        add(action::PAGE_UP, e.has & has::PAGE_UP != 0);
        add(action::PAGE_DOWN, e.has & has::PAGE_DOWN != 0);
        add(action::PAGE_LEFT, e.has & has::PAGE_LEFT != 0);
        add(action::PAGE_RIGHT, e.has & has::PAGE_RIGHT != 0);
    }
    if enabled {
        add(action::EXPAND, n.expanded == Some(false));
        add(action::COLLAPSE, n.expanded == Some(true));
        add(action::DISMISS, e.has & has::DISMISS != 0);
    }
    add(action::SHOW_ON_SCREEN, true);

    // The text moved over (a description before a field's text, before a text).
    let iterable = if !e.descriptions.is_empty() {
        Some(e.descriptions.join(","))
    } else if n.editable {
        Some(n.value.clone())
    } else {
        e.texts.first().cloned()
    }
    .filter(|t| !t.is_empty());
    let mut granularities = 0;
    let mut selection = None;
    let mut lines = Vec::new();
    if let Some(t) = &iterable {
        add(action::SET_SELECTION, true);
        add(action::NEXT_AT_GRANULARITY, true);
        add(action::PREVIOUS_AT_GRANULARITY, true);
        granularities = granularity::CHARACTER | granularity::WORD | granularity::PARAGRAPH;
        if e.descriptions.is_empty() {
            selection = e.selection.map(|(s, end)| (utf16_at(t, s), utf16_at(t, end)));
            if e.has & has::TEXT_LAYOUT != 0
                && !e.exclude_lines
                && let Some(starts) = &e.lines
            {
                granularities |= granularity::LINE | granularity::PAGE;
                lines = starts.iter().map(|&(b, top)| (utf16_at(t, b), top)).collect();
            }
        }
    }
    if e.has & has::TEXT_LAYOUT != 0 && text.is_some() {
        flags |= flag::CHARACTER_LOCATIONS;
    }

    let item = n
        .collection_item_info
        .map(|i| (i.row_index, i.row_span, i.column_index, i.column_span, n.selected.unwrap_or(false)));
    let axis = |a: Option<ScrollAxis>| a.map(|a| (a.value as i32, a.max as i32));
    AndroidNode {
        id,
        parent,
        semantics_id: n.id,
        class_name,
        role_description,
        flags,
        actions,
        granularities,
        bounds: n.bounds,
        text,
        content_description,
        hint,
        state_description: own,
        state,
        state_percent,
        error: n.error.clone(),
        pane_title: n.pane_title.clone(),
        test_tag: n.test_tag.clone(),
        iterable,
        selection,
        lines,
        max_text_length: e.max_text_length.unwrap_or(-1),
        live_region: match n.live_region {
            None => 0,
            Some(LiveRegionMode::Polite) => 1,
            Some(LiveRegionMode::Assertive) => 2,
        },
        range: n.range.as_ref().filter(|_| !indeterminate).map(|r| (*r.range.start(), *r.range.end(), r.current)),
        collection,
        item,
        scroll_x: axis(e.horizontal),
        scroll_y: axis(e.vertical),
        custom_actions: if enabled { n.custom_actions.clone() } else { Vec::new() },
        children,
    }
}

/// The class Android's views of a role have (Compose's `toLegacyClassName`; a switch is Android's Switch, as C++
/// Kompoz's: TalkBack says "switch" and its state as on or off).
pub fn legacy_class_name(role: Role) -> Option<&'static str> {
    Some(match role {
        Role::Button => "android.widget.Button",
        Role::Checkbox => "android.widget.CheckBox",
        Role::Switch => "android.widget.Switch",
        Role::RadioButton => "android.widget.RadioButton",
        Role::Image => "android.widget.ImageView",
        Role::DropdownList => "android.widget.Spinner",
        Role::ValuePicker => "android.widget.NumberPicker",
        Role::Tab | Role::Carousel => return None,
    })
}

// ---- Events ----

/// What KompozAccessibility sends: its AccessibilityEvent's type and what goes into it.
#[derive(Clone, PartialEq, Debug)]
pub enum Event {
    /// TYPE_WINDOW_CONTENT_CHANGED with [`change`]'s bits.
    ContentChanged { id: i32, changes: i32 },
    /// TYPE_VIEW_TEXT_CHANGED: `removed` UTF-16 units from `from` were `before`'s, `added` came; `text` the field's
    /// now.
    TextChanged { id: i32, from: i32, removed: i32, added: i32, before: String, text: String, password: bool },
    /// TYPE_VIEW_TEXT_SELECTION_CHANGED (UTF-16 offsets; the text's length as its item count).
    SelectionChanged { id: i32, from: i32, to: i32, text: String },
    /// TYPE_VIEW_FOCUSED.
    Focused { id: i32 },
    /// TYPE_VIEW_SELECTED (a tab selected): what it says.
    Selected { id: i32, text: Option<String>, description: Option<String> },
    /// TYPE_VIEW_SCROLLED: where it is and how far it goes (none: that axis isn't its).
    Scrolled { id: i32, x: Option<(i32, i32)>, y: Option<(i32, i32)> },
    /// TYPE_WINDOW_STATE_CHANGED for a pane (appeared, disappeared, its title changed) and its title.
    PaneChanged { id: i32, changes: i32, title: String },
}

/// What a screen reader is told from `old` to `new` (two published trees, the shared ones and Android's): the
/// shared [`changes`] (subtrees, texts, states, the focus) and what Android hears of besides — panes, errors,
/// selections, scrolls, a selected tab, bounds moved, any other property —, in the order Compose sends them.
pub fn events(old: (&Snapshot, &AndroidTree), new: (&Snapshot, &AndroidTree)) -> Vec<Event> {
    let (old_snapshot, old_tree) = old;
    let (new_snapshot, new_tree) = new;
    if !old_snapshot.is_published() {
        return Vec::new(); // (the first tree: the services read it as they are told the window changed)
    }
    let mut out = Vec::new();
    let mut per_node: Vec<(i32, i32)> = Vec::new(); // (node, change bits), first come first
    let mut note = |id: i32, bits: i32| match per_node.iter_mut().find(|(n, _)| *n == id) {
        Some((_, b)) => *b |= bits,
        None => per_node.push((id, bits)),
    };
    let vid = |tree: &AndroidTree, semantics: u64| tree.virtual_id(semantics);
    let mut text_changed = Vec::new();
    let mut focused = None;
    for c in changes(old_snapshot, new_snapshot) {
        match c {
            Change::Removed { parent, .. } | Change::Added { parent, .. } => {
                let id = parent.and_then(|p| vid(new_tree, p)).unwrap_or(HOST_VIEW_ID);
                note(id, change::SUBTREE);
            }
            Change::Announcement { node, .. } => {
                if let Some(id) = vid(new_tree, node) {
                    note(id, change::TEXT);
                }
            }
            Change::Checked { node, .. } => {
                if let Some(id) = vid(new_tree, node) {
                    note(id, change::CHECKED);
                }
            }
            Change::Enabled { node, .. } => {
                if let Some(id) = vid(new_tree, node) {
                    note(id, change::ENABLED);
                }
            }
            Change::TextRemoved { node, .. } | Change::TextInserted { node, .. } => {
                if let Some(id) = vid(new_tree, node)
                    && !text_changed.contains(&id)
                {
                    text_changed.push(id);
                }
            }
            Change::Focused(node) => focused = vid(new_tree, node),
            Change::FocusLeft(node) => {
                if let Some(id) = vid(new_tree, node) {
                    note(id, change::UNDEFINED);
                }
            }
            Change::Name { .. }
            | Change::Description { .. }
            | Change::Value { .. }
            | Change::WindowFocused(_)
            | Change::WindowTitle(_) => {}
        }
    }
    let mut panes = Vec::new();
    let mut selections = Vec::new();
    let mut scrolls = Vec::new();
    let mut selected_tabs = Vec::new();
    let mut workaround = Vec::new(); // (Compose's b/192295060: an UNDEFINED after a state change)
    // Nodes whose subtree is told changed for their bounds or scroll: what moved under them is in it.
    let mut moved: std::collections::HashSet<i32> = std::collections::HashSet::new();
    for n in new_tree.nodes() {
        let Some(o) = old_tree.get(n.id) else {
            if let Some(title) = &n.pane_title {
                panes.push(Event::PaneChanged { id: n.id, changes: change::PANE_APPEARED, title: title.clone() });
            }
            continue;
        };
        let id = n.id;
        match (&o.pane_title, &n.pane_title) {
            (None, Some(t)) => panes.push(Event::PaneChanged { id, changes: change::PANE_APPEARED, title: t.clone() }),
            (Some(t), None) => {
                panes.push(Event::PaneChanged { id, changes: change::PANE_DISAPPEARED, title: t.clone() })
            }
            (Some(a), Some(b)) if a != b => {
                panes.push(Event::PaneChanged { id, changes: change::PANE_TITLE, title: b.clone() })
            }
            _ => {}
        }
        let state_changed = n.state_description != o.state_description
            || n.state != o.state
            || n.state_percent != o.state_percent
            || n.range != o.range;
        if state_changed {
            note(id, change::STATE_DESCRIPTION);
            workaround.push(id);
        }
        if n.flags & flag::CHECKED != o.flags & flag::CHECKED {
            note(id, change::CHECKED);
            workaround.push(id);
        }
        if n.error != o.error {
            note(id, change::ERROR | change::CONTENT_INVALID);
        }
        if n.content_description != o.content_description {
            note(id, change::CONTENT_DESCRIPTION);
        }
        if n.text != o.text && !(n.flags & o.flags & flag::EDITABLE != 0) {
            note(id, change::TEXT);
        }
        let tab_selected = n.flags & flag::SELECTED != 0;
        if tab_selected != (o.flags & flag::SELECTED != 0) {
            if tab_selected {
                selected_tabs.push(Event::Selected {
                    id,
                    text: n.text.clone(),
                    description: n.content_description.clone(),
                });
            } else {
                note(id, change::UNDEFINED);
            }
        }
        let scrolled = n.scroll_x != o.scroll_x || n.scroll_y != o.scroll_y;
        if moved.contains(&n.parent) {
            moved.insert(id); // (told with what is above it)
        } else if n.bounds != o.bounds || scrolled {
            note(id, change::SUBTREE);
            moved.insert(id);
        }
        if n.selection != o.selection
            && n.flags & flag::EDITABLE != 0
            && let (Some((from, to)), Some(text)) = (n.selection, &n.iterable)
        {
            selections.push(Event::SelectionChanged { id, from, to, text: text.clone() });
        }
        if scrolled {
            scrolls.push(Event::Scrolled { id, x: n.scroll_x, y: n.scroll_y });
        }
        // (Shown or not follows the bounds: told with them.)
        let own = !(flag::CHECKED | flag::SELECTED | flag::FOCUSED | flag::VISIBLE);
        let others_changed = n.flags & own != o.flags & own
            || n.actions != o.actions
            || n.custom_actions != o.custom_actions
            || n.class_name != o.class_name
            || n.hint != o.hint
            || n.live_region != o.live_region
            || n.collection != o.collection
            || n.item != o.item;
        if others_changed {
            note(id, change::UNDEFINED);
        }
    }
    for o in old_tree.nodes() {
        if new_tree.get(o.id).is_none()
            && let Some(title) = &o.pane_title
        {
            panes.push(Event::PaneChanged { id: o.id, changes: change::PANE_DISAPPEARED, title: title.clone() });
        }
    }
    // Subtrees first, then the panes, each node's properties, texts typed, selections, scrolls, the focus.
    per_node.sort_by_key(|&(_, bits)| if bits & change::SUBTREE != 0 { 0 } else { 1 });
    out.extend(panes);
    for (id, changes) in per_node {
        if new_tree.get(id).is_some() {
            out.push(Event::ContentChanged { id, changes });
            if workaround.contains(&id) {
                out.push(Event::ContentChanged { id, changes: change::UNDEFINED });
            }
        }
    }
    out.extend(selected_tabs);
    for id in text_changed {
        if let (Some(o), Some(n)) = (old_tree.get(id), new_tree.get(id)) {
            let before = o.text.clone().unwrap_or_default();
            let text = n.text.clone().unwrap_or_default();
            let password = n.flags & flag::PASSWORD != 0;
            if (o.flags & flag::PASSWORD != 0) != password {
                // A password shown or hidden (Compose's b/247891690): the caret reset, then where it is.
                out.push(Event::SelectionChanged { id, from: 0, to: 0, text: text.clone() });
                if let Some((from, to)) = n.selection {
                    out.push(Event::SelectionChanged { id, from, to, text });
                }
                continue;
            }
            let (from, removed, added) = utf16_edit(&before, &text);
            out.push(Event::TextChanged { id, from, removed, added, before, text, password });
        }
    }
    out.extend(selections);
    out.extend(scrolls);
    if let Some(id) = focused {
        out.push(Event::Focused { id });
        out.push(Event::ContentChanged { id, changes: change::UNDEFINED });
    }
    out
}

/// Where two texts differ in UTF-16 units (Compose's TYPE_VIEW_TEXT_CHANGED): the same start's length, how many
/// went, how many came.
pub fn utf16_edit(before: &str, after: &str) -> (i32, i32, i32) {
    let (a, b): (Vec<u16>, Vec<u16>) = (before.encode_utf16().collect(), after.encode_utf16().collect());
    let min = a.len().min(b.len());
    let mut start = 0;
    while start < min && a[start] == b[start] {
        start += 1;
    }
    let mut end = 0;
    while end < min - start && a[a.len() - 1 - end] == b[b.len() - 1 - end] {
        end += 1;
    }
    (start as i32, (a.len() - end - start) as i32, (b.len() - end - start) as i32)
}

// ---- The buffers Kotlin reads ----

/// A little-endian buffer of ints, floats and strings (KompozAccessibility's `Reader` reads them back in order).
#[derive(Default)]
pub struct Writer(pub Vec<u8>);

impl Writer {
    pub fn int(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub fn long(&mut self, v: i64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub fn float(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub fn bool(&mut self, v: bool) {
        self.int(v as i32);
    }

    /// A string: its UTF-8 length, then its bytes (-1: none).
    pub fn str(&mut self, s: Option<&str>) {
        match s {
            Some(s) => {
                self.int(s.len() as i32);
                self.0.extend_from_slice(s.as_bytes());
            }
            None => self.int(-1),
        }
    }

    pub fn rect(&mut self, r: Rect) {
        for v in [r.left, r.top, r.right, r.bottom] {
            self.float(v);
        }
    }
}

/// The tree as KompozAccessibility reads it (its `Tree.read`): the node count, then each node.
pub fn encode_tree(tree: &AndroidTree) -> Vec<u8> {
    let mut w = Writer::default();
    w.int(tree.nodes.len() as i32);
    for n in &tree.nodes {
        for v in [
            n.id,
            n.parent,
            n.flags,
            n.actions,
            n.granularities,
            n.live_region,
            n.role_description,
            n.state,
            n.state_percent,
            n.selection.map_or(-1, |s| s.0),
            n.selection.map_or(-1, |s| s.1),
            n.max_text_length,
        ] {
            w.int(v);
        }
        match n.range {
            Some((min, max, current)) => {
                w.int(1);
                w.float(min);
                w.float(max);
                w.float(current);
            }
            None => w.int(0),
        }
        let (rows, columns) = n.collection.unwrap_or((-1, -1));
        w.bool(n.collection.is_some());
        w.int(rows);
        w.int(columns);
        let (row, row_span, column, column_span, selected) = n.item.unwrap_or((-1, -1, -1, -1, false));
        w.bool(n.item.is_some());
        for v in [row, row_span, column, column_span] {
            w.int(v);
        }
        w.bool(selected);
        for axis in [n.scroll_x, n.scroll_y] {
            let (value, max) = axis.unwrap_or((-1, -1));
            w.bool(axis.is_some());
            w.int(value);
            w.int(max);
        }
        w.rect(n.bounds);
        w.str(Some(n.class_name));
        for s in [
            &n.text,
            &n.content_description,
            &n.hint,
            &n.state_description,
            &n.error,
            &n.pane_title,
            &n.test_tag,
            &n.iterable,
        ] {
            w.str(s.as_deref());
        }
        w.int(n.custom_actions.len() as i32);
        for label in &n.custom_actions {
            w.str(Some(label));
        }
        w.int(n.lines.len() as i32);
        for &(start, top) in &n.lines {
            w.int(start);
            w.float(top);
        }
        w.int(n.children.len() as i32);
        for &c in &n.children {
            w.int(c);
        }
    }
    w.0
}

/// KompozAccessibility's event kinds (its `EVENT_*`).
pub mod event_kind {
    pub const CONTENT_CHANGED: i32 = 0;
    pub const TEXT_CHANGED: i32 = 1;
    pub const SELECTION_CHANGED: i32 = 2;
    pub const FOCUSED: i32 = 3;
    pub const SELECTED: i32 = 4;
    pub const SCROLLED: i32 = 5;
    pub const PANE_CHANGED: i32 = 6;
}

/// The events as KompozAccessibility reads them: the count, then each one's kind, node, content change bits, four
/// numbers and three strings.
pub fn encode_events(events: &[Event], w: &mut Writer) {
    w.int(events.len() as i32);
    for e in events {
        let (kind, id, changes, numbers, strings): (i32, i32, i32, [i32; 4], [Option<&str>; 3]) = match e {
            Event::ContentChanged { id, changes } => (event_kind::CONTENT_CHANGED, *id, *changes, [0; 4], [None; 3]),
            Event::TextChanged { id, from, removed, added, before, text, password } => (
                event_kind::TEXT_CHANGED,
                *id,
                0,
                [*from, *removed, *added, *password as i32],
                [Some(text), Some(before), None],
            ),
            Event::SelectionChanged { id, from, to, text } => (
                event_kind::SELECTION_CHANGED,
                *id,
                0,
                [*from, *to, text.encode_utf16().count() as i32, 0],
                [Some(text), None, None],
            ),
            Event::Focused { id } => (event_kind::FOCUSED, *id, 0, [0; 4], [None; 3]),
            Event::Selected { id, text, description } => {
                (event_kind::SELECTED, *id, 0, [0; 4], [text.as_deref(), None, description.as_deref()])
            }
            Event::Scrolled { id, x, y } => {
                let (sx, mx) = x.unwrap_or((-1, -1));
                let (sy, my) = y.unwrap_or((-1, -1));
                (event_kind::SCROLLED, *id, 0, [sx, mx, sy, my], [None; 3])
            }
            Event::PaneChanged { id, changes, title } => {
                (event_kind::PANE_CHANGED, *id, *changes, [0; 4], [Some(title), None, None])
            }
        };
        w.int(kind);
        w.int(id);
        w.int(changes);
        for v in numbers {
            w.int(v);
        }
        for s in strings {
            w.str(s);
        }
    }
}

// ---- Actions ----

/// What KompozAccessibility asks to be done (its `ACTION_*` codes, after accessibility focus, granularity moves
/// and the custom actions' ids were dealt with in Kotlin).
#[derive(Clone, PartialEq, Debug)]
pub enum Request {
    Click,
    LongClick,
    Focus,
    ClearFocus,
    ScrollForward,
    ScrollBackward,
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    /// A collection's row and column.
    ScrollToPosition {
        row: i32,
        column: i32,
    },
    SetProgress(f32),
    SetText(String),
    /// UTF-16 offsets of the iterable text (-1, -1: none); in traversal (a screen reader moving through it).
    SetSelection {
        start: i32,
        end: i32,
        traversal: bool,
    },
    Expand,
    Collapse,
    Dismiss,
    Copy,
    Cut,
    Paste,
    ImeEnter,
    PageUp,
    PageDown,
    PageLeft,
    PageRight,
    ShowOnScreen,
    /// Its custom action at this index.
    Custom(usize),
}

/// KompozAccessibility's request codes (its `REQUEST_*`).
pub mod request {
    pub const CLICK: i32 = 0;
    pub const LONG_CLICK: i32 = 1;
    pub const FOCUS: i32 = 2;
    pub const CLEAR_FOCUS: i32 = 3;
    pub const SCROLL_FORWARD: i32 = 4;
    pub const SCROLL_BACKWARD: i32 = 5;
    pub const SCROLL_UP: i32 = 6;
    pub const SCROLL_DOWN: i32 = 7;
    pub const SCROLL_LEFT: i32 = 8;
    pub const SCROLL_RIGHT: i32 = 9;
    pub const SCROLL_TO_POSITION: i32 = 10;
    pub const SET_PROGRESS: i32 = 11;
    pub const SET_TEXT: i32 = 12;
    pub const SET_SELECTION: i32 = 13;
    pub const EXPAND: i32 = 14;
    pub const COLLAPSE: i32 = 15;
    pub const DISMISS: i32 = 16;
    pub const COPY: i32 = 17;
    pub const CUT: i32 = 18;
    pub const PASTE: i32 = 19;
    pub const IME_ENTER: i32 = 20;
    pub const PAGE_UP: i32 = 21;
    pub const PAGE_DOWN: i32 = 22;
    pub const PAGE_LEFT: i32 = 23;
    pub const PAGE_RIGHT: i32 = 24;
    pub const SHOW_ON_SCREEN: i32 = 25;
    pub const CUSTOM: i32 = 26;
}

/// The request of code `code` with its arguments (two ints, a float, a text); none if there is no such code.
pub fn decode_request(code: i32, a: i32, b: i32, value: f32, text: Option<String>) -> Option<Request> {
    use request as r;
    Some(match code {
        r::CLICK => Request::Click,
        r::LONG_CLICK => Request::LongClick,
        r::FOCUS => Request::Focus,
        r::CLEAR_FOCUS => Request::ClearFocus,
        r::SCROLL_FORWARD => Request::ScrollForward,
        r::SCROLL_BACKWARD => Request::ScrollBackward,
        r::SCROLL_UP => Request::ScrollUp,
        r::SCROLL_DOWN => Request::ScrollDown,
        r::SCROLL_LEFT => Request::ScrollLeft,
        r::SCROLL_RIGHT => Request::ScrollRight,
        r::SCROLL_TO_POSITION => Request::ScrollToPosition { row: a, column: b },
        r::SET_PROGRESS if value.is_finite() => Request::SetProgress(value),
        r::SET_TEXT => Request::SetText(text.unwrap_or_default()),
        r::SET_SELECTION => Request::SetSelection { start: a, end: b, traversal: value != 0.0 },
        r::EXPAND => Request::Expand,
        r::COLLAPSE => Request::Collapse,
        r::DISMISS => Request::Dismiss,
        r::COPY => Request::Copy,
        r::CUT => Request::Cut,
        r::PASTE => Request::Paste,
        r::IME_ENTER => Request::ImeEnter,
        r::PAGE_UP => Request::PageUp,
        r::PAGE_DOWN => Request::PageDown,
        r::PAGE_LEFT => Request::PageLeft,
        r::PAGE_RIGHT => Request::PageRight,
        r::SHOW_ON_SCREEN => Request::ShowOnScreen,
        r::CUSTOM if a >= 0 => Request::Custom(a as usize),
        _ => return None,
    })
}

/// How far a scroll action goes along an axis (Compose's `performActionHelper`): a viewport's length, back for
/// left, up and backward, the other way where it scrolls reversed, and (left and right) where the layout is right
/// to left. Forward and backward go along both axes.
pub fn scroll_amount(request: &Request, viewport: f32, horizontal: bool, reverse: bool, rtl: bool) -> Option<f32> {
    let applies = match request {
        Request::ScrollForward | Request::ScrollBackward => true,
        Request::ScrollLeft | Request::ScrollRight => horizontal,
        Request::ScrollUp | Request::ScrollDown => !horizontal,
        _ => false,
    };
    if !applies {
        return None;
    }
    let mut amount = viewport;
    if matches!(request, Request::ScrollLeft | Request::ScrollUp | Request::ScrollBackward) {
        amount = -amount;
    }
    if reverse {
        amount = -amount;
    }
    if horizontal && rtl && matches!(request, Request::ScrollLeft | Request::ScrollRight) {
        amount = -amount;
    }
    Some(amount)
}

/// A slider's next value forward or back (Compose's): its steps' size, or a twentieth of its range.
pub fn progress_step(current: f32, min: f32, max: f32, steps: i32, forward: bool) -> f32 {
    let (lo, hi) = (min.min(max), max.max(min));
    let increment = if steps > 0 { (hi - lo) / (steps + 1) as f32 } else { (hi - lo) / 20.0 };
    current + if forward { increment } else { -increment }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::Window;
    use kompoz_ui::semantics::{CollectionItemInfo, ProgressBarRangeInfo};
    use kompoz_ui::state::ToggleableState;

    fn node(id: u64, parent: Option<u64>, bounds: Rect) -> AccessibilityNode {
        AccessibilityNode { id, parent, bounds, enabled: true, ..Default::default() }
    }

    fn rect(l: f32, t: f32, r: f32, b: f32) -> Rect {
        Rect { left: l, top: t, right: r, bottom: b }
    }

    fn texts(t: &[&str]) -> Extras {
        Extras { texts: t.iter().map(|s| s.to_string()).collect(), ..Default::default() }
    }

    #[test]
    fn classes_texts_and_states_follow_the_roles() {
        let mut button = node(2, Some(1), rect(0.0, 0.0, 100.0, 40.0));
        button.role = Some(Role::Button);
        button.clickable = true;
        button.label = "Tamam".into();
        let mut switch = node(3, Some(1), rect(0.0, 50.0, 100.0, 90.0));
        switch.role = Some(Role::Switch);
        switch.toggleable_state = Some(ToggleableState::On);
        switch.clickable = true;
        let mut tab = node(4, Some(1), rect(0.0, 100.0, 50.0, 140.0));
        tab.role = Some(Role::Tab);
        tab.selected = Some(true);
        tab.clickable = true;
        let mut field = node(5, Some(1), rect(0.0, 150.0, 100.0, 190.0));
        field.editable = true;
        field.label = "Ad".into();
        field.focused = true;
        let mut slider = node(6, Some(1), rect(0.0, 200.0, 100.0, 240.0));
        slider.range = Some(ProgressBarRangeInfo::new(0.25, 0.0..=1.0, 0));
        slider.settable = true;
        let mut spinner = node(7, Some(1), rect(0.0, 250.0, 40.0, 290.0));
        spinner.range = Some(ProgressBarRangeInfo::Indeterminate);
        let tree = Snapshot::new(
            vec![node(1, None, rect(0.0, 0.0, 400.0, 800.0)), button, switch, tab, field, slider, spinner],
            Window::default(),
        );
        let mut extras = HashMap::new();
        extras.insert(2, Extras { merging: true, ..texts(&["Tamam"]) });
        extras.insert(5, Extras { focusable: true, has: has::SET_TEXT | has::TEXT_LAYOUT, ..Default::default() });
        extras.insert(6, Extras { has: has::SET_PROGRESS, ..Default::default() });
        let t = android_tree(&tree, &extras);
        let host = t.get(HOST_VIEW_ID).unwrap();
        assert_eq!((host.parent, host.children.as_slice()), (INVALID_ID, [2, 3, 4, 5, 6, 7].as_slice()));
        let b = t.get(2).unwrap();
        assert_eq!((b.class_name, b.text.as_deref(), b.parent), ("android.widget.Button", Some("Tamam"), HOST_VIEW_ID));
        assert!(b.actions & action::CLICK != 0 && b.flags & flag::SCREEN_READER_FOCUSABLE != 0);
        let s = t.get(3).unwrap();
        assert_eq!((s.class_name, s.state), ("android.widget.Switch", state::ON));
        assert!(s.flags & (flag::CHECKABLE | flag::CHECKED) == flag::CHECKABLE | flag::CHECKED);
        let tab = t.get(4).unwrap();
        assert_eq!(tab.role_description, role_description::TAB);
        // (Selected: a tab's own, not checked; it can't be selected again.)
        assert!(tab.flags & flag::SELECTED != 0 && tab.flags & (flag::CHECKABLE | flag::CLICKABLE) == 0);
        let f = t.get(5).unwrap();
        assert_eq!((f.class_name, f.hint.as_deref(), f.text.as_deref()), ("android.widget.EditText", Some("Ad"), None));
        assert!(f.flags & (flag::EDITABLE | flag::FOCUSED | flag::SHOWING_HINT) != 0);
        assert!(f.actions & (action::SET_TEXT | action::CLEAR_FOCUS) == action::SET_TEXT | action::CLEAR_FOCUS);
        assert_eq!(t.input_focus(), Some(5));
        let sl = t.get(6).unwrap();
        assert_eq!((sl.class_name, sl.state, sl.state_percent), ("android.widget.SeekBar", state::PERCENT, 25));
        assert_eq!(sl.range, Some((0.0, 1.0, 0.25)));
        assert!(sl.actions & (action::SCROLL_FORWARD | action::SCROLL_BACKWARD | action::SET_PROGRESS) != 0);
        let sp = t.get(7).unwrap();
        assert_eq!((sp.class_name, sp.state, sp.range), ("android.widget.ProgressBar", state::IN_PROGRESS, None));
        // The finger finds the deepest speaking node.
        assert_eq!(t.hit(10.0, 60.0), 3);
        assert_eq!(t.hit(399.0, 799.0), INVALID_ID);
    }

    #[test]
    fn scrolls_selections_lines_and_groups() {
        let mut list = node(2, Some(1), rect(0.0, 0.0, 100.0, 300.0));
        list.scrollable = true;
        let mut group = node(3, Some(1), rect(0.0, 300.0, 300.0, 340.0));
        group.selectable_group = true;
        let mut a = node(4, Some(3), rect(0.0, 300.0, 100.0, 340.0));
        a.selected = Some(true);
        a.role = Some(Role::RadioButton);
        a.collection_item_info =
            Some(CollectionItemInfo { row_index: 0, row_span: 1, column_index: 0, column_span: 1 });
        let mut b = node(5, Some(3), rect(100.0, 300.0, 200.0, 340.0));
        b.selected = Some(false);
        let mut field = node(6, Some(1), rect(0.0, 400.0, 100.0, 440.0));
        field.editable = true;
        field.value = "ağaç evi".into();
        let tree = Snapshot::new(
            vec![node(1, None, rect(0.0, 0.0, 400.0, 800.0)), list, group, a, b, field],
            Window::default(),
        );
        let mut extras = HashMap::new();
        let axis = ScrollAxis { value: 50.0, max: 200.0, reverse: false };
        extras.insert(2, Extras { vertical: Some(axis), has: has::SCROLL_BY, ..Default::default() });
        let field_extras = Extras {
            selection: Some((3, 8)), // "ağ|aç e|vi": bytes 3 and 8 are units 2 and 6
            lines: Some(vec![(0, 0.0), (7, 20.0)]),
            has: has::TEXT_LAYOUT | has::SET_SELECTION,
            ..Default::default()
        };
        extras.insert(6, field_extras);
        let t = android_tree(&tree, &extras);
        let l = t.get(2).unwrap();
        assert_eq!((l.class_name, l.scroll_y), ("android.widget.ScrollView", Some((50, 200))));
        let all = action::SCROLL_FORWARD | action::SCROLL_BACKWARD | action::SCROLL_UP | action::SCROLL_DOWN;
        assert_eq!(l.actions & all, all);
        assert!(l.flags & flag::SCROLLABLE != 0);
        assert_eq!(t.get(3).unwrap().collection, Some((1, 2)));
        let first = t.get(4).unwrap();
        assert_eq!(first.item, Some((0, 1, 0, 1, true)));
        assert_eq!(first.state, state::SELECTED);
        let f = t.get(6).unwrap();
        assert_eq!(f.selection, Some((2, 6)));
        assert_eq!(f.lines, [(0, 0.0), (5, 20.0)]);
        assert_eq!(f.granularities, 1 | 2 | 4 | 8 | 16);
        assert_eq!(f.text.as_deref(), Some("ağaç evi"));
    }

    #[test]
    fn the_changes_become_androids_events() {
        let root = node(1, None, rect(0.0, 0.0, 400.0, 800.0));
        let mut field = node(2, Some(1), rect(0.0, 0.0, 100.0, 40.0));
        field.editable = true;
        field.value = "kedi".into();
        let mut check = node(3, Some(1), rect(0.0, 50.0, 100.0, 90.0));
        check.toggleable_state = Some(ToggleableState::Off);
        check.role = Some(Role::Checkbox);
        let mut list = node(4, Some(1), rect(0.0, 100.0, 100.0, 400.0));
        list.scrollable = true;
        let before = vec![root.clone(), field.clone(), check.clone(), list.clone()];
        field.value = "kediler".into();
        field.focused = true;
        check.toggleable_state = Some(ToggleableState::On);
        let mut dialog = node(5, Some(1), rect(0.0, 0.0, 10.0, 10.0));
        dialog.pane_title = Some("Uyarı".into());
        let after = vec![root, field, check, list, dialog];
        let extras_of = |scroll: f32| {
            let mut e = HashMap::new();
            e.insert(2, Extras { focusable: true, selection: Some((7, 7)), ..Default::default() });
            let axis = ScrollAxis { value: scroll, max: 900.0, reverse: false };
            e.insert(4, Extras { vertical: Some(axis), has: has::SCROLL_BY, ..Default::default() });
            e
        };
        let (s1, s2) = (Snapshot::new(before, Window::default()), Snapshot::new(after, Window::default()));
        let (t1, t2) = (android_tree(&s1, &extras_of(0.0)), android_tree(&s2, &extras_of(120.0)));
        let ev = events((&s1, &t1), (&s2, &t2));
        assert!(events((&Snapshot::default(), &AndroidTree::default()), (&s2, &t2)).is_empty());
        let host_subtree = ev.iter().position(|e| *e == Event::ContentChanged { id: HOST_VIEW_ID, changes: 1 });
        let pane =
            ev.iter().position(|e| matches!(e, Event::PaneChanged { id: 5, changes: change::PANE_APPEARED, .. }));
        assert!(host_subtree.is_some() && pane.is_some());
        assert!(ev.contains(&Event::ContentChanged { id: 3, changes: change::CHECKED }));
        assert!(ev.contains(&Event::TextChanged {
            id: 2,
            from: 4,
            removed: 0,
            added: 3,
            before: "kedi".into(),
            text: "kediler".into(),
            password: false
        }));
        assert!(ev.contains(&Event::Scrolled { id: 4, x: None, y: Some((120, 900)) }));
        assert_eq!(ev.last(), Some(&Event::ContentChanged { id: 2, changes: change::UNDEFINED }));
        assert!(ev.contains(&Event::Focused { id: 2 }));
        let mut w = Writer::default();
        encode_events(&ev, &mut w);
        assert_eq!(i32::from_le_bytes(w.0[..4].try_into().unwrap()), ev.len() as i32);
        assert!(!encode_tree(&t2).is_empty());
    }

    #[test]
    fn offsets_requests_and_scrolls() {
        assert_eq!(utf16_at("a😀b", 5), 3);
        assert_eq!(utf16_at("a😀b", 3), 1); // (inside the emoji: its start)
        assert_eq!(byte_at("a😀b", 3), 5);
        assert_eq!(byte_at("a😀b", 2), 1);
        assert_eq!(byte_at("a😀b", 99), 6);
        assert_eq!(utf16_edit("hello", "help"), (3, 2, 1));
        assert_eq!(utf16_edit("aa", "aaa"), (2, 0, 1));
        assert_eq!(
            decode_request(request::SET_SELECTION, 1, 4, 1.0, None),
            Some(Request::SetSelection { start: 1, end: 4, traversal: true })
        );
        assert_eq!(decode_request(request::CUSTOM, 2, 0, 0.0, None), Some(Request::Custom(2)));
        assert_eq!(decode_request(99, 0, 0, 0.0, None), None);
        assert_eq!(scroll_amount(&Request::ScrollUp, 300.0, false, false, false), Some(-300.0));
        assert_eq!(scroll_amount(&Request::ScrollRight, 300.0, true, false, true), Some(-300.0));
        assert_eq!(scroll_amount(&Request::ScrollForward, 300.0, false, true, false), Some(-300.0));
        assert_eq!(scroll_amount(&Request::ScrollUp, 300.0, true, false, false), None);
        assert!((progress_step(0.5, 0.0, 1.0, 0, true) - 0.55).abs() < 1e-6);
        assert_eq!(progress_step(2.0, 0.0, 10.0, 4, false), 0.0);
    }
}
