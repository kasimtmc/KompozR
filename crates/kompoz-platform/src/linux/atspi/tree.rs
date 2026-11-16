//! The tree the bridge serves (C++'s atspi_bridge.cpp's published state): the application, its window and the
//! semantics nodes in it as AT-SPI objects — their roles, states, names, extents, actions, texts — and what changed
//! between two trees as AT-SPI events.

use kompoz_ui::semantics::{AccessibilityNode, LiveRegionMode, ProgressBarRangeInfo, Role, SemanticsAction};
use std::collections::HashMap;

pub const ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";
pub const WINDOW_PATH: &str = "/org/a11y/atspi/accessible/window";
pub const NODE_PREFIX: &str = "/org/a11y/atspi/accessible/";
pub const NULL_PATH: &str = "/org/a11y/atspi/null";

pub const ACCESSIBLE: &str = "org.a11y.atspi.Accessible";
pub const APPLICATION: &str = "org.a11y.atspi.Application";
pub const COMPONENT: &str = "org.a11y.atspi.Component";
pub const ACTION: &str = "org.a11y.atspi.Action";
pub const VALUE: &str = "org.a11y.atspi.Value";
pub const TEXT: &str = "org.a11y.atspi.Text";
pub const EDITABLE_TEXT: &str = "org.a11y.atspi.EditableText";
pub const EVENT_OBJECT: &str = "org.a11y.atspi.Event.Object";
pub const EVENT_WINDOW: &str = "org.a11y.atspi.Event.Window";
pub const EVENT_FOCUS: &str = "org.a11y.atspi.Event.Focus";

// AtspiRole
pub const ROLE_CHECK_BOX: u32 = 7;
pub const ROLE_COMBO_BOX: u32 = 11;
pub const ROLE_DIALOG: u32 = 16;
pub const ROLE_FRAME: u32 = 23;
pub const ROLE_IMAGE: u32 = 27;
pub const ROLE_LABEL: u32 = 29;
pub const ROLE_PAGE_TAB: u32 = 37;
pub const ROLE_PANEL: u32 = 39;
pub const ROLE_PASSWORD_TEXT: u32 = 40;
pub const ROLE_PROGRESS_BAR: u32 = 42;
pub const ROLE_BUTTON: u32 = 43;
pub const ROLE_RADIO_BUTTON: u32 = 44;
pub const ROLE_SCROLL_PANE: u32 = 49;
pub const ROLE_SLIDER: u32 = 51;
pub const ROLE_SPIN_BUTTON: u32 = 52;
pub const ROLE_APPLICATION: u32 = 75;
pub const ROLE_ENTRY: u32 = 79;
pub const ROLE_HEADING: u32 = 83;
pub const ROLE_SWITCH: u32 = 130;

// AtspiStateType
pub const STATE_ACTIVE: u32 = 1;
pub const STATE_CHECKED: u32 = 4;
pub const STATE_EDITABLE: u32 = 7;
pub const STATE_ENABLED: u32 = 8;
pub const STATE_FOCUSABLE: u32 = 11;
pub const STATE_FOCUSED: u32 = 12;
pub const STATE_MODAL: u32 = 16;
pub const STATE_RESIZABLE: u32 = 21;
pub const STATE_SELECTABLE: u32 = 22;
pub const STATE_SELECTED: u32 = 23;
pub const STATE_SENSITIVE: u32 = 24;
pub const STATE_SHOWING: u32 = 25;
pub const STATE_VISIBLE: u32 = 30;
pub const STATE_INDETERMINATE: u32 = 32;
pub const STATE_CHECKABLE: u32 = 41;

// AtspiComponentLayer, AtspiCoordType (0: the screen)
pub const LAYER_WIDGET: u32 = 3;
pub const LAYER_WINDOW: u32 = 7;
pub const COORD_WINDOW: u32 = 1;
pub const COORD_PARENT: u32 = 2;

/// The window the tree is in: where it is on the screen and its size, in the platform's points (the units AT-SPI's
/// extents are in), points per px (the nodes' bounds are in px), whether it has the focus, its title.
#[derive(Clone, PartialEq, Debug)]
pub struct Window {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub px_to_points: f32,
    pub focused: bool,
    pub title: String,
}

impl Default for Window {
    fn default() -> Self {
        Window { x: 0.0, y: 0.0, width: 0.0, height: 0.0, px_to_points: 1.0, focused: true, title: String::new() }
    }
}

/// An AT-SPI object of the app: the application, its window, or a semantics node in it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Object {
    Root,
    Window,
    Node(u64),
}

impl Object {
    pub fn path(self) -> String {
        match self {
            Object::Root => ROOT_PATH.to_string(),
            Object::Window => WINDOW_PATH.to_string(),
            Object::Node(id) => format!("{NODE_PREFIX}{id}"),
        }
    }

    /// The object at `path` (whether it exists or not).
    pub fn at(path: &str) -> Option<Object> {
        match path {
            ROOT_PATH => Some(Object::Root),
            WINDOW_PATH => Some(Object::Window),
            _ => {
                let digits = path.strip_prefix(NODE_PREFIX)?;
                if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                digits.parse().ok().map(Object::Node)
            }
        }
    }

    /// The window's own, for a node's parent (none: at the window's top).
    fn of_parent(parent: Option<u64>) -> Object {
        parent.map_or(Object::Window, Object::Node)
    }
}

/// Integer extents (AT-SPI's).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Extents {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Extents {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
}

/// What a node does (Action's actions, in order): its name and the action it asks for.
pub fn actions_of(n: &AccessibilityNode) -> Vec<(String, SemanticsAction)> {
    let mut out = Vec::new();
    if n.clickable || n.editable {
        out.push(("click".to_string(), SemanticsAction::Click));
    }
    if n.scrollable {
        out.push(("scroll forward".to_string(), SemanticsAction::ScrollForward));
        out.push(("scroll backward".to_string(), SemanticsAction::ScrollBackward));
    }
    for (i, label) in n.custom_actions.iter().enumerate() {
        out.push((label.clone(), SemanticsAction::CustomAction(i)));
    }
    out
}

/// A text field's text as it is said: a password's hidden (a bullet a character).
pub fn text_of(n: &AccessibilityNode) -> String {
    if n.password { "\u{2022}".repeat(n.value.chars().count()) } else { n.value.clone() }
}

/// A node's role by what it is and does (as Android's and C++'s bridges tell it).
fn role_of_node(n: &AccessibilityNode) -> u32 {
    match n.role {
        Some(Role::Button) => return ROLE_BUTTON,
        Some(Role::Checkbox) => return ROLE_CHECK_BOX,
        Some(Role::Switch) => return ROLE_SWITCH,
        Some(Role::RadioButton) => return ROLE_RADIO_BUTTON,
        Some(Role::Tab) => return ROLE_PAGE_TAB,
        Some(Role::Image) => return ROLE_IMAGE,
        Some(Role::DropdownList) => return ROLE_COMBO_BOX,
        Some(Role::ValuePicker) => return ROLE_SPIN_BUTTON,
        Some(Role::Carousel) | None => {}
    }
    if n.editable {
        return if n.password { ROLE_PASSWORD_TEXT } else { ROLE_ENTRY };
    }
    if n.is_dialog {
        return ROLE_DIALOG;
    }
    if n.heading {
        return ROLE_HEADING;
    }
    if n.clickable {
        return ROLE_BUTTON;
    }
    if n.range.is_some() {
        return if n.settable { ROLE_SLIDER } else { ROLE_PROGRESS_BAR };
    }
    if n.scrollable {
        return ROLE_SCROLL_PANE;
    }
    ROLE_PANEL
}

pub fn role_name(role: u32) -> &'static str {
    match role {
        ROLE_CHECK_BOX => "check box",
        ROLE_COMBO_BOX => "combo box",
        ROLE_DIALOG => "dialog",
        ROLE_FRAME => "frame",
        ROLE_IMAGE => "image",
        ROLE_LABEL => "label",
        ROLE_PAGE_TAB => "page tab",
        ROLE_PASSWORD_TEXT => "password text",
        ROLE_PROGRESS_BAR => "progress bar",
        ROLE_BUTTON => "button",
        ROLE_RADIO_BUTTON => "radio button",
        ROLE_SCROLL_PANE => "scroll pane",
        ROLE_SLIDER => "slider",
        ROLE_SPIN_BUTTON => "spin button",
        ROLE_APPLICATION => "application",
        ROLE_ENTRY => "entry",
        ROLE_HEADING => "heading",
        ROLE_SWITCH => "switch",
        _ => "panel",
    }
}

/// A UTF-8 string's code points (AT-SPI's text offsets): the byte each starts at, then its end.
pub fn code_points(s: &str) -> Vec<usize> {
    s.char_indices().map(|(i, _)| i).chain(std::iter::once(s.len())).collect()
}

/// Code points `from` to `to` (-1: the end) of `s`, clamped.
pub fn slice<'a>(s: &'a str, at: &[usize], from: i32, to: i32) -> &'a str {
    let n = at.len() as i32 - 1;
    let from = from.clamp(0, n);
    let to = if to < 0 { n } else { to.clamp(from, n) };
    &s[at[from as usize]..at[to as usize]]
}

/// The piece of a text an offset is in: a character, a word (with the spaces after it), or the whole (a field's one
/// line).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unit {
    Char,
    Word,
    Line,
}

/// The `unit` around code point `offset` of `s`: [start, end) in code points.
pub fn segment(s: &str, at: &[usize], offset: i32, unit: Unit) -> (i32, i32) {
    let n = at.len() as i32 - 1;
    if offset < 0 || offset >= n {
        let o = offset.clamp(0, n);
        return (o, o);
    }
    match unit {
        Unit::Char => return (offset, offset + 1),
        Unit::Line => return (0, n),
        Unit::Word => {}
    }
    // (C's isspace on a code point's first byte, as C++'s.)
    let space = |i: i32| matches!(s.as_bytes()[at[i as usize]], b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r');
    let (mut start, mut end) = (offset, offset);
    while start > 0 && space(start) {
        start -= 1; // in the spaces after a word: that word's
    }
    while start > 0 && !space(start - 1) {
        start -= 1;
    }
    while end < n && !space(end) {
        end += 1;
    }
    while end < n && space(end) {
        end += 1;
    }
    (start, end)
}

/// A tree as published: its nodes, where each is, each one's children (none: the window's), the window.
#[derive(Default)]
pub struct Tree {
    pub nodes: Vec<AccessibilityNode>,
    at: HashMap<u64, usize>,
    children: HashMap<Option<u64>, Vec<u64>>,
    pub window: Window,
    /// One was published (else this is the empty one before).
    pub published: bool,
}

impl Tree {
    pub fn new(nodes: Vec<AccessibilityNode>, window: Window) -> Tree {
        let mut at = HashMap::new();
        let mut children: HashMap<Option<u64>, Vec<u64>> = HashMap::new();
        for (i, n) in nodes.iter().enumerate() {
            at.insert(n.id, i);
            children.entry(n.parent).or_default().push(n.id);
        }
        Tree { nodes, at, children, window, published: true }
    }

    pub fn exists(&self, o: Object) -> bool {
        match o {
            Object::Root | Object::Window => true,
            Object::Node(id) => self.at.contains_key(&id),
        }
    }

    pub fn contains(&self, id: u64) -> bool {
        self.at.contains_key(&id)
    }

    /// The node `id` (which exists).
    pub fn node(&self, id: u64) -> &AccessibilityNode {
        &self.nodes[self.at[&id]]
    }

    pub fn kids_of(&self, o: Object) -> Vec<Object> {
        let key = match o {
            Object::Root => return vec![Object::Window],
            Object::Window => None,
            Object::Node(id) => Some(id),
        };
        self.children.get(&key).map(|ids| ids.iter().map(|&id| Object::Node(id)).collect()).unwrap_or_default()
    }

    pub fn parent_of(&self, o: Object) -> Object {
        match o {
            Object::Root | Object::Window => Object::Root,
            Object::Node(id) => Object::of_parent(self.node(id).parent),
        }
    }

    /// Where `child` is among `parent`'s children in this tree (-1: it isn't).
    fn index_in(&self, parent: Option<u64>, child: u64) -> i32 {
        self.children.get(&parent).and_then(|ids| ids.iter().position(|&id| id == child)).map_or(-1, |i| i as i32)
    }

    pub fn interfaces_of(&self, o: Object) -> Vec<&'static str> {
        match o {
            Object::Root => vec![ACCESSIBLE, APPLICATION],
            Object::Window => vec![ACCESSIBLE, COMPONENT],
            Object::Node(id) => {
                let n = self.node(id);
                let mut out = vec![ACCESSIBLE, COMPONENT];
                if !actions_of(n).is_empty() {
                    out.push(ACTION);
                }
                if n.range.is_some() {
                    out.push(VALUE);
                }
                if n.editable {
                    out.extend([TEXT, EDITABLE_TEXT]);
                }
                out
            }
        }
    }

    pub fn implements(&self, o: Object, interface: &str) -> bool {
        self.interfaces_of(o).contains(&interface)
    }

    pub fn role_of(&self, o: Object) -> u32 {
        match o {
            Object::Root => ROLE_APPLICATION,
            Object::Window => ROLE_FRAME,
            Object::Node(id) => {
                let n = self.node(id);
                let role = role_of_node(n);
                // A panel that only says something: a label.
                if role == ROLE_PANEL && !n.label.is_empty() && self.kids_of(o).is_empty() { ROLE_LABEL } else { role }
            }
        }
    }

    pub fn name_of(&self, o: Object, app_name: &str) -> String {
        match o {
            Object::Root => app_name.to_string(),
            Object::Window => self.window.title.clone(),
            Object::Node(id) => {
                let n = self.node(id);
                match &n.pane_title {
                    Some(title) if n.label.is_empty() => title.clone(),
                    _ => n.label.clone(),
                }
            }
        }
    }

    /// Its states: AtspiStateType's bits in two words.
    pub fn states_of(&self, o: Object) -> [u32; 2] {
        let mut bits = [0u32; 2];
        let mut set = |state: u32| bits[(state / 32) as usize] |= 1 << (state % 32);
        if o == Object::Root {
            return bits;
        }
        set(STATE_VISIBLE);
        set(STATE_SHOWING);
        let Object::Node(id) = o else {
            set(STATE_ENABLED);
            set(STATE_SENSITIVE);
            set(STATE_RESIZABLE);
            if self.window.focused {
                set(STATE_ACTIVE);
            }
            return bits;
        };
        let n = self.node(id);
        if n.enabled {
            set(STATE_ENABLED);
            set(STATE_SENSITIVE);
        }
        if n.clickable || n.editable || n.settable {
            set(STATE_FOCUSABLE);
        }
        if n.focused {
            set(STATE_FOCUSED);
        }
        if n.editable {
            set(STATE_EDITABLE);
        }
        if n.is_dialog {
            set(STATE_MODAL);
        }
        if let Some(checked) = n.checked {
            if n.role == Some(Role::Tab) {
                set(STATE_SELECTABLE);
                if checked {
                    set(STATE_SELECTED);
                }
            } else {
                set(STATE_CHECKABLE);
                if checked {
                    set(STATE_CHECKED);
                }
            }
        }
        if n.range == Some(ProgressBarRangeInfo::Indeterminate) {
            set(STATE_INDETERMINATE);
        }
        bits
    }

    /// Where it is on the screen, in points (left, top, width, height).
    fn on_screen(&self, o: Object) -> [f32; 4] {
        let w = &self.window;
        match o {
            Object::Root | Object::Window => [w.x, w.y, w.width, w.height],
            Object::Node(id) => {
                let b = self.node(id).bounds;
                let k = w.px_to_points;
                [w.x + b.left * k, w.y + b.top * k, b.width() * k, b.height() * k]
            }
        }
    }

    /// Where it is, in points: on the screen, in the window, or from its parent's corner (AtspiCoordType).
    pub fn extents_of(&self, o: Object, coord_type: u32) -> Extents {
        let mut r = self.on_screen(o);
        if coord_type == COORD_WINDOW {
            r[0] -= self.window.x;
            r[1] -= self.window.y;
        } else if coord_type == COORD_PARENT && matches!(o, Object::Node(_)) {
            let p = self.on_screen(self.parent_of(o));
            r[0] -= p[0];
            r[1] -= p[1];
        }
        let round = |v: f32| v.round() as i32;
        Extents { x: round(r[0]), y: round(r[1]), width: round(r[2]), height: round(r[3]) }
    }
}

/// What an event carries besides its details.
#[derive(Clone, PartialEq, Debug)]
pub enum EventData {
    None,
    Text(String),
    Number(f64),
    Ref(Object),
}

/// An AT-SPI event, sent by the bus's thread.
#[derive(Clone, PartialEq, Debug)]
pub struct Event {
    pub object: Object,
    pub interface: &'static str,
    pub member: &'static str,
    pub detail: String,
    pub detail1: i32,
    pub detail2: i32,
    pub data: EventData,
}

impl Event {
    fn plain(object: Object, interface: &'static str, member: &'static str, detail: &str, detail1: i32) -> Event {
        Event { object, interface, member, detail: detail.to_string(), detail1, detail2: 0, data: EventData::None }
    }

    fn text(object: Object, member: &'static str, detail: &str, detail1: i32, detail2: i32, text: &str) -> Event {
        Event {
            object,
            interface: EVENT_OBJECT,
            member,
            detail: detail.to_string(),
            detail1,
            detail2,
            data: EventData::Text(text.to_string()),
        }
    }
}

/// The events that tell what changed from `old` to `next`.
pub fn compare(old: &Tree, next: &Tree) -> Vec<Event> {
    let mut events = Vec::new();
    let w = &next.window;
    if old.published {
        // Whole subtrees come and go: told on the parent that stays (the last first, so that the indexes of those
        // before still hold).
        for gone in old.nodes.iter().rev() {
            if !next.contains(gone.id) && gone.parent.is_none_or(|p| next.contains(p)) {
                events.push(Event {
                    data: EventData::Ref(Object::Node(gone.id)),
                    ..Event::plain(
                        Object::of_parent(gone.parent),
                        EVENT_OBJECT,
                        "ChildrenChanged",
                        "remove",
                        old.index_in(gone.parent, gone.id),
                    )
                });
            }
        }
        for new in &next.nodes {
            if !old.contains(new.id) && new.parent.is_none_or(|p| old.contains(p)) {
                events.push(Event {
                    data: EventData::Ref(Object::Node(new.id)),
                    ..Event::plain(
                        Object::of_parent(new.parent),
                        EVENT_OBJECT,
                        "ChildrenChanged",
                        "add",
                        next.index_in(new.parent, new.id),
                    )
                });
            }
        }
    }
    if if old.published { w.focused != old.window.focused } else { w.focused } {
        let member = if w.focused { "Activate" } else { "Deactivate" };
        events.push(Event { interface: EVENT_WINDOW, ..Event::text(Object::Window, member, "", 0, 0, &w.title) });
        events.push(Event::plain(Object::Window, EVENT_OBJECT, "StateChanged", "active", w.focused as i32));
    }
    if old.published && w.title != old.window.title {
        events.push(Event::text(Object::Window, "PropertyChange", "accessible-name", 0, 0, &w.title));
    }
    let focused_before = old.nodes.iter().filter(|n| n.focused).map(|n| n.id).next_back();
    let mut focused_now = None;
    for n in &next.nodes {
        if n.focused {
            focused_now = Some(n.id);
        }
        if !old.published || !old.contains(n.id) {
            continue;
        }
        let o = old.node(n.id);
        let at = Object::Node(n.id);
        if n.label != o.label {
            events.push(Event::text(at, "PropertyChange", "accessible-name", 0, 0, &n.label));
        }
        if n.state_description != o.state_description {
            events.push(Event::text(at, "PropertyChange", "accessible-description", 0, 0, &n.state_description));
        }
        if let Some(live) = n.live_region
            && (n.label != o.label || n.state_description != o.state_description)
        {
            let mut said = n.label.clone();
            if !n.state_description.is_empty() {
                if !said.is_empty() {
                    said.push_str(", ");
                }
                said.push_str(&n.state_description);
            }
            let politeness = if live == LiveRegionMode::Assertive { 2 } else { 1 }; // (AtspiLive)
            events.push(Event::text(at, "Announcement", "", politeness, 0, &said));
        }
        if n.checked.unwrap_or(false) != o.checked.unwrap_or(false) {
            let detail = if n.role == Some(Role::Tab) { "selected" } else { "checked" };
            events.push(Event::plain(at, EVENT_OBJECT, "StateChanged", detail, n.checked.unwrap_or(false) as i32));
        }
        if n.enabled != o.enabled {
            events.push(Event::plain(at, EVENT_OBJECT, "StateChanged", "enabled", n.enabled as i32));
            events.push(Event::plain(at, EVENT_OBJECT, "StateChanged", "sensitive", n.enabled as i32));
        }
        if let (Some(r), Some(before)) = (&n.range, &o.range)
            && r.current != before.current
        {
            events.push(Event {
                data: EventData::Number(r.current as f64),
                ..Event::plain(at, EVENT_OBJECT, "PropertyChange", "accessible-value", 0)
            });
        }
        if n.editable && o.editable && n.value != o.value {
            // What was typed or taken away: what lies between the same start and the same end.
            let (before, after) = (text_of(o), text_of(n));
            let (a, b) = (code_points(&before), code_points(&after));
            let (na, nb) = (a.len() as i32 - 1, b.len() as i32 - 1);
            let same = |i: i32, j: i32| slice(&before, &a, i, i + 1) == slice(&after, &b, j, j + 1);
            let mut head = 0;
            while head < na && head < nb && same(head, head) {
                head += 1;
            }
            let mut tail = 0;
            while tail < na - head && tail < nb - head && same(na - 1 - tail, nb - 1 - tail) {
                tail += 1;
            }
            if na - head - tail > 0 {
                let gone = slice(&before, &a, head, na - tail);
                events.push(Event::text(at, "TextChanged", "delete", head, na - head - tail, gone));
            }
            if nb - head - tail > 0 {
                let typed = slice(&after, &b, head, nb - tail);
                events.push(Event::text(at, "TextChanged", "insert", head, nb - head - tail, typed));
            }
        }
    }
    if focused_now != focused_before || (!old.published && focused_now.is_some()) {
        if old.published
            && let Some(before) = focused_before
            && next.contains(before)
        {
            events.push(Event::plain(Object::Node(before), EVENT_OBJECT, "StateChanged", "focused", 0));
        }
        if let Some(now) = focused_now {
            events.push(Event::plain(Object::Node(now), EVENT_OBJECT, "StateChanged", "focused", 1));
            events.push(Event::plain(Object::Node(now), EVENT_FOCUS, "Focus", "", 0));
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objects_have_paths() {
        for o in [Object::Root, Object::Window, Object::Node(0), Object::Node(u64::MAX)] {
            assert_eq!(Object::at(&o.path()), Some(o));
        }
        for bad in ["/org/a11y/atspi/accessible/", "/org/a11y/atspi/accessible/1a", "/org/a11y/atspi/null", "/x"] {
            assert_eq!(Object::at(bad), None);
        }
        assert_eq!(Object::at("/org/a11y/atspi/accessible/99999999999999999999"), None);
    }

    #[test]
    fn texts_are_cut_by_code_points_and_words() {
        let s = "héllo wörld";
        let at = code_points(s);
        assert_eq!(at.len() - 1, 11);
        assert_eq!(slice(s, &at, 1, 3), "él");
        assert_eq!(slice(s, &at, 6, -1), "wörld");
        assert_eq!(slice(s, &at, -4, 99), s);
        assert_eq!(segment(s, &at, 7, Unit::Word), (6, 11));
        assert_eq!(segment(s, &at, 5, Unit::Word), (0, 6));
        assert_eq!(segment(s, &at, 1, Unit::Char), (1, 2));
        assert_eq!(segment(s, &at, 3, Unit::Line), (0, 11));
        assert_eq!(segment(s, &at, 11, Unit::Word), (11, 11));
        let password = AccessibilityNode { value: "abc".into(), password: true, ..Default::default() };
        assert_eq!(text_of(&password), "•••");
    }
}
