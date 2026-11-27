//! The tree the bridge serves (C++'s atspi_bridge.cpp's published state): the application, a frame for each of its
//! windows (as GTK's and Qt's apps have: the main one's, each `Window`'s, in the order they were first published)
//! and the semantics nodes in each as AT-SPI objects — their roles, states, names, extents, actions, texts — and what
//! changed between two trees of a window (the platform-neutral [`changes`](crate::accessibility::changes)) as AT-SPI
//! events, a window coming and going too.

pub use crate::accessibility::Window;
pub use crate::accessibility::text::{Unit, code_points, segment, slice, text_of};

use crate::accessibility::{Change, Snapshot, changes};
use kompoz_ui::semantics::{AccessibilityNode, ProgressBarRangeInfo, Role, SemanticsAction};

pub const ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";
/// A window's objects: its frame `<prefix><window>/window`, its nodes `<prefix><window>/<node>`.
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

/// An AT-SPI object of the app: the application, a window's frame (by the window's key: its `WindowId`'s number), or
/// a semantics node in a window.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Object {
    Root,
    Window(u64),
    Node(u64, u64),
}

impl Object {
    pub fn path(self) -> String {
        match self {
            Object::Root => ROOT_PATH.to_string(),
            Object::Window(w) => format!("{NODE_PREFIX}{w}/window"),
            Object::Node(w, id) => format!("{NODE_PREFIX}{w}/{id}"),
        }
    }

    /// The object at `path` (whether it exists or not).
    pub fn at(path: &str) -> Option<Object> {
        if path == ROOT_PATH {
            return Some(Object::Root);
        }
        let number = |digits: &str| -> Option<u64> {
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            digits.parse().ok()
        };
        let (window, rest) = path.strip_prefix(NODE_PREFIX)?.split_once('/')?;
        let window = number(window)?;
        if rest == "window" { Some(Object::Window(window)) } else { number(rest).map(|id| Object::Node(window, id)) }
    }

    /// The window it is in (none: the application).
    pub fn window(self) -> Option<u64> {
        match self {
            Object::Root => None,
            Object::Window(w) | Object::Node(w, _) => Some(w),
        }
    }

    /// The window's own, for a node's parent (none: at the window's top).
    fn of_parent(window: u64, parent: Option<u64>) -> Object {
        parent.map_or(Object::Window(window), |p| Object::Node(window, p))
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

/// The trees as published: each window's (the neutral [`Snapshot`], by the window's key, in the order they were
/// first published), their nodes AT-SPI objects.
#[derive(Default)]
pub struct Tree {
    windows: Vec<(u64, Snapshot)>,
}

impl Tree {
    /// One window's tree: `nodes` in `window`, as window `key`'s.
    pub fn new(key: u64, nodes: Vec<AccessibilityNode>, window: Window) -> Tree {
        Tree { windows: vec![(key, Snapshot::new(nodes, window))] }
    }

    /// The windows' keys, in order.
    pub fn windows(&self) -> Vec<u64> {
        self.windows.iter().map(|(k, _)| *k).collect()
    }

    /// Window `key`'s tree, if it has one.
    pub fn snapshot(&self, key: u64) -> Option<&Snapshot> {
        self.windows.iter().find(|(k, _)| *k == key).map(|(_, s)| s)
    }

    /// Window `key`'s tree replaced by `snapshot` (a new window after the others); what was there before (the empty
    /// one for a new window).
    pub fn set(&mut self, key: u64, snapshot: Snapshot) -> Snapshot {
        match self.windows.iter_mut().find(|(k, _)| *k == key) {
            Some((_, s)) => std::mem::replace(s, snapshot),
            None => {
                self.windows.push((key, snapshot));
                Snapshot::default()
            }
        }
    }

    /// Window `key`'s tree taken away: where it was among the windows, and the tree.
    pub fn remove(&mut self, key: u64) -> Option<(usize, Snapshot)> {
        let i = self.windows.iter().position(|(k, _)| *k == key)?;
        Some((i, self.windows.remove(i).1))
    }

    fn of(&self, key: u64) -> &Snapshot {
        self.snapshot(key).expect("the window's tree")
    }

    pub fn exists(&self, o: Object) -> bool {
        match o {
            Object::Root => true,
            Object::Window(w) => self.snapshot(w).is_some(),
            Object::Node(w, id) => self.snapshot(w).is_some_and(|s| s.contains(id)),
        }
    }

    pub fn contains(&self, window: u64, id: u64) -> bool {
        self.exists(Object::Node(window, id))
    }

    /// The node `id` of window `window` (which exists).
    pub fn node(&self, window: u64, id: u64) -> &AccessibilityNode {
        self.of(window).node(id)
    }

    /// The window a frame or node is in (none: the application's).
    pub fn window_of(&self, o: Object) -> Option<&Window> {
        self.snapshot(o.window()?).map(Snapshot::window)
    }

    pub fn kids_of(&self, o: Object) -> Vec<Object> {
        let (w, key) = match o {
            Object::Root => return self.windows.iter().map(|(k, _)| Object::Window(*k)).collect(),
            Object::Window(w) => (w, None),
            Object::Node(w, id) => (w, Some(id)),
        };
        self.of(w).children(key).iter().map(|&id| Object::Node(w, id)).collect()
    }

    pub fn parent_of(&self, o: Object) -> Object {
        match o {
            Object::Root | Object::Window(_) => Object::Root,
            Object::Node(w, id) => Object::of_parent(w, self.node(w, id).parent),
        }
    }

    pub fn interfaces_of(&self, o: Object) -> Vec<&'static str> {
        match o {
            Object::Root => vec![ACCESSIBLE, APPLICATION],
            Object::Window(_) => vec![ACCESSIBLE, COMPONENT],
            Object::Node(w, id) => {
                let n = self.node(w, id);
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
            Object::Window(_) => ROLE_FRAME,
            Object::Node(w, id) => {
                let n = self.node(w, id);
                let role = role_of_node(n);
                // A panel that only says something: a label.
                if role == ROLE_PANEL && !n.label.is_empty() && self.kids_of(o).is_empty() { ROLE_LABEL } else { role }
            }
        }
    }

    pub fn name_of(&self, o: Object, app_name: &str) -> String {
        match o {
            Object::Root => app_name.to_string(),
            Object::Window(w) => self.of(w).window().title.clone(),
            Object::Node(w, id) => {
                let n = self.node(w, id);
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
        let (w, id) = match o {
            Object::Root => return bits,
            Object::Window(w) => (w, None),
            Object::Node(w, id) => (w, Some(id)),
        };
        set(STATE_VISIBLE);
        set(STATE_SHOWING);
        let Some(id) = id else {
            set(STATE_ENABLED);
            set(STATE_SENSITIVE);
            set(STATE_RESIZABLE);
            if self.of(w).window().focused {
                set(STATE_ACTIVE);
            }
            return bits;
        };
        let n = self.node(w, id);
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

    /// Where it is on the screen, in points (left, top, width, height); the application: nowhere.
    fn on_screen(&self, o: Object) -> [f32; 4] {
        let Some(w) = self.window_of(o) else { return [0.0; 4] };
        match o {
            Object::Root | Object::Window(_) => [w.x, w.y, w.width, w.height],
            Object::Node(window, id) => {
                let b = self.node(window, id).bounds;
                let k = w.px_to_points;
                [w.x + b.left * k, w.y + b.top * k, b.width() * k, b.height() * k]
            }
        }
    }

    /// Where it is, in points: on the screen, in the window, or from its parent's corner (AtspiCoordType).
    pub fn extents_of(&self, o: Object, coord_type: u32) -> Extents {
        let mut r = self.on_screen(o);
        if coord_type == COORD_WINDOW
            && let Some(w) = self.window_of(o)
        {
            r[0] -= w.x;
            r[1] -= w.y;
        } else if coord_type == COORD_PARENT && matches!(o, Object::Node(..)) {
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

/// The events that tell what changed in window `key`'s tree from `old` to `next` ([`changes`]' in AT-SPI's terms,
/// in their order).
pub fn compare(key: u64, old: &Snapshot, next: &Snapshot) -> Vec<Event> {
    let title = &next.window().title;
    let window = Object::Window(key);
    let node = |id: u64| Object::Node(key, id);
    let mut events = Vec::new();
    for change in changes(old, next) {
        match change {
            Change::Removed { parent, node: gone, index } => events.push(Event {
                data: EventData::Ref(node(gone)),
                ..Event::plain(Object::of_parent(key, parent), EVENT_OBJECT, "ChildrenChanged", "remove", index as i32)
            }),
            Change::Added { parent, node: new, index } => events.push(Event {
                data: EventData::Ref(node(new)),
                ..Event::plain(Object::of_parent(key, parent), EVENT_OBJECT, "ChildrenChanged", "add", index as i32)
            }),
            Change::WindowFocused(focused) => {
                let member = if focused { "Activate" } else { "Deactivate" };
                events.push(Event { interface: EVENT_WINDOW, ..Event::text(window, member, "", 0, 0, title) });
                events.push(Event::plain(window, EVENT_OBJECT, "StateChanged", "active", focused as i32));
            }
            Change::WindowTitle(title) => {
                events.push(Event::text(window, "PropertyChange", "accessible-name", 0, 0, &title));
            }
            Change::Name { node: id, name } => {
                events.push(Event::text(node(id), "PropertyChange", "accessible-name", 0, 0, &name));
            }
            Change::Description { node: id, description } => {
                events.push(Event::text(node(id), "PropertyChange", "accessible-description", 0, 0, &description));
            }
            Change::Announcement { node: id, text, assertive } => {
                let politeness = if assertive { 2 } else { 1 }; // (AtspiLive)
                events.push(Event::text(node(id), "Announcement", "", politeness, 0, &text));
            }
            Change::Checked { node: id, checked } => {
                let detail = if next.node(id).role == Some(Role::Tab) { "selected" } else { "checked" };
                events.push(Event::plain(node(id), EVENT_OBJECT, "StateChanged", detail, checked as i32));
            }
            Change::Enabled { node: id, enabled } => {
                for detail in ["enabled", "sensitive"] {
                    events.push(Event::plain(node(id), EVENT_OBJECT, "StateChanged", detail, enabled as i32));
                }
            }
            Change::Value { node: id, value } => events.push(Event {
                data: EventData::Number(value as f64),
                ..Event::plain(node(id), EVENT_OBJECT, "PropertyChange", "accessible-value", 0)
            }),
            Change::TextRemoved { node: id, start, length, text } => {
                events.push(Event::text(node(id), "TextChanged", "delete", start as i32, length as i32, &text));
            }
            Change::TextInserted { node: id, start, length, text } => {
                events.push(Event::text(node(id), "TextChanged", "insert", start as i32, length as i32, &text));
            }
            Change::FocusLeft(id) => {
                events.push(Event::plain(node(id), EVENT_OBJECT, "StateChanged", "focused", 0));
            }
            Change::Focused(id) => {
                events.push(Event::plain(node(id), EVENT_OBJECT, "StateChanged", "focused", 1));
                events.push(Event::plain(node(id), EVENT_FOCUS, "Focus", "", 0));
            }
        }
    }
    events
}

/// The events that tell window `key` came, `index` its place among the application's windows (GTK's and Qt's:
/// the application's ChildrenChanged, the frame's `window:create`); what its tree says (its activation) follows.
pub fn window_added(key: u64, index: usize, title: &str) -> Vec<Event> {
    let window = Object::Window(key);
    vec![
        Event {
            data: EventData::Ref(window),
            ..Event::plain(Object::Root, EVENT_OBJECT, "ChildrenChanged", "add", index as i32)
        },
        Event { interface: EVENT_WINDOW, ..Event::text(window, "Create", "", 0, 0, title) },
    ]
}

/// The events that tell window `key` went, from `index` among the application's windows, with its tree `old`: its
/// deactivation if it was active, its `window:destroy`, the application's ChildrenChanged (its subtree with it).
pub fn window_removed(key: u64, index: usize, old: &Snapshot) -> Vec<Event> {
    let window = Object::Window(key);
    let title = &old.window().title;
    let mut events = Vec::new();
    if old.is_published() && old.window().focused {
        events.push(Event { interface: EVENT_WINDOW, ..Event::text(window, "Deactivate", "", 0, 0, title) });
        events.push(Event::plain(window, EVENT_OBJECT, "StateChanged", "active", 0));
    }
    events.push(Event { interface: EVENT_WINDOW, ..Event::text(window, "Destroy", "", 0, 0, title) });
    events.push(Event {
        data: EventData::Ref(window),
        ..Event::plain(Object::Root, EVENT_OBJECT, "ChildrenChanged", "remove", index as i32)
    });
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objects_have_paths() {
        for o in [Object::Root, Object::Window(0), Object::Window(7), Object::Node(0, 0), Object::Node(3, u64::MAX)] {
            assert_eq!(Object::at(&o.path()), Some(o));
        }
        assert_eq!(Object::Window(2).path(), "/org/a11y/atspi/accessible/2/window");
        assert_eq!(Object::Node(2, 12).path(), "/org/a11y/atspi/accessible/2/12");
        for bad in [
            "/org/a11y/atspi/accessible/",
            "/org/a11y/atspi/accessible/1a",
            "/org/a11y/atspi/accessible/1",
            "/org/a11y/atspi/accessible/x/window",
            "/org/a11y/atspi/accessible/1/1a",
            "/org/a11y/atspi/null",
            "/x",
        ] {
            assert_eq!(Object::at(bad), None, "{bad}");
        }
        assert_eq!(Object::at("/org/a11y/atspi/accessible/0/99999999999999999999"), None);
    }

    #[test]
    fn changes_are_atspi_events() {
        let node = |id, parent| AccessibilityNode { id, parent, enabled: true, ..Default::default() };
        let mut tab = node(2, Some(1));
        tab.role = Some(Role::Tab);
        let old = Snapshot::new(vec![node(1, None), tab.clone()], Window::default());
        tab.checked = Some(true);
        let next = Snapshot::new(vec![node(1, None), tab], Window { focused: false, ..Window::default() });
        let events = compare(4, &old, &next);
        let said: Vec<(&str, &str, i32)> = events.iter().map(|e| (e.member, e.detail.as_str(), e.detail1)).collect();
        assert_eq!(said, [("Deactivate", "", 0), ("StateChanged", "active", 0), ("StateChanged", "selected", 1)]);
        assert_eq!(events[0].interface, EVENT_WINDOW);
        assert_eq!((events[0].object, events[2].object), (Object::Window(4), Object::Node(4, 2)));
    }

    #[test]
    fn windows_are_the_applications_children() {
        let mut tree = Tree::default();
        let node = |id| AccessibilityNode { id, enabled: true, ..Default::default() };
        let titled = |title: &str, focused| Window { title: title.into(), focused, ..Window::default() };
        assert!(!tree.set(0, Snapshot::new(vec![node(1)], titled("Main", true))).is_published());
        assert!(!tree.set(5, Snapshot::new(vec![node(1), node(2)], titled("Second", false))).is_published());
        assert_eq!(tree.kids_of(Object::Root), [Object::Window(0), Object::Window(5)]);
        assert_eq!(tree.kids_of(Object::Window(5)), [Object::Node(5, 1), Object::Node(5, 2)]);
        assert_eq!(tree.parent_of(Object::Node(5, 2)), Object::Window(5));
        assert_eq!(tree.name_of(Object::Window(5), "App"), "Second");
        assert!(tree.exists(Object::Node(5, 2)) && !tree.exists(Object::Node(0, 2)));
        assert_ne!(tree.states_of(Object::Window(0))[0] & (1 << STATE_ACTIVE), 0);
        assert_eq!(tree.states_of(Object::Window(5))[0] & (1 << STATE_ACTIVE), 0);
        let (index, gone) = tree.remove(0).expect("there");
        assert_eq!(index, 0);
        let said: Vec<&str> = window_removed(0, index, &gone).iter().map(|e| e.member).collect();
        assert_eq!(said, ["Deactivate", "StateChanged", "Destroy", "ChildrenChanged"]);
        assert_eq!(tree.windows(), [5]);
        assert!(!tree.exists(Object::Window(0)) && !tree.exists(Object::Node(0, 1)));
        let added = window_added(5, 0, "Second");
        assert_eq!((added[0].object, added[0].data.clone()), (Object::Root, EventData::Ref(Object::Window(5))));
        assert_eq!((added[1].interface, added[1].member), (EVENT_WINDOW, "Create"));
    }
}
