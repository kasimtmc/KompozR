//! The tree the bridge serves (C++'s atspi_bridge.cpp's published state): the application, its window and the
//! semantics nodes in it as AT-SPI objects — their roles, states, names, extents, actions, texts — and what changed
//! between two trees (the platform-neutral [`changes`](crate::accessibility::changes)) as AT-SPI events.

pub use crate::accessibility::Window;
pub use crate::accessibility::text::{Unit, code_points, segment, slice, text_of};

use crate::accessibility::{Change, Snapshot, changes};
use kompoz_ui::semantics::{AccessibilityNode, ProgressBarRangeInfo, Role, SemanticsAction};

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

/// A tree as published (the neutral [`Snapshot`]), its nodes AT-SPI objects.
#[derive(Default)]
pub struct Tree {
    snapshot: Snapshot,
}

impl Tree {
    pub fn new(nodes: Vec<AccessibilityNode>, window: Window) -> Tree {
        Tree { snapshot: Snapshot::new(nodes, window) }
    }

    pub fn exists(&self, o: Object) -> bool {
        match o {
            Object::Root | Object::Window => true,
            Object::Node(id) => self.snapshot.contains(id),
        }
    }

    pub fn contains(&self, id: u64) -> bool {
        self.snapshot.contains(id)
    }

    /// The node `id` (which exists).
    pub fn node(&self, id: u64) -> &AccessibilityNode {
        self.snapshot.node(id)
    }

    pub fn kids_of(&self, o: Object) -> Vec<Object> {
        let key = match o {
            Object::Root => return vec![Object::Window],
            Object::Window => None,
            Object::Node(id) => Some(id),
        };
        self.snapshot.children(key).iter().map(|&id| Object::Node(id)).collect()
    }

    pub fn parent_of(&self, o: Object) -> Object {
        match o {
            Object::Root | Object::Window => Object::Root,
            Object::Node(id) => Object::of_parent(self.node(id).parent),
        }
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
            Object::Window => self.snapshot.window().title.clone(),
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
            if self.snapshot.window().focused {
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
        let w = self.snapshot.window();
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
            r[0] -= self.snapshot.window().x;
            r[1] -= self.snapshot.window().y;
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

/// The events that tell what changed from `old` to `next` ([`changes`]' in AT-SPI's terms, in their order).
pub fn compare(old: &Tree, next: &Tree) -> Vec<Event> {
    let title = &next.snapshot.window().title;
    let mut events = Vec::new();
    for change in changes(&old.snapshot, &next.snapshot) {
        match change {
            Change::Removed { parent, node, index } => events.push(Event {
                data: EventData::Ref(Object::Node(node)),
                ..Event::plain(Object::of_parent(parent), EVENT_OBJECT, "ChildrenChanged", "remove", index as i32)
            }),
            Change::Added { parent, node, index } => events.push(Event {
                data: EventData::Ref(Object::Node(node)),
                ..Event::plain(Object::of_parent(parent), EVENT_OBJECT, "ChildrenChanged", "add", index as i32)
            }),
            Change::WindowFocused(focused) => {
                let member = if focused { "Activate" } else { "Deactivate" };
                events.push(Event { interface: EVENT_WINDOW, ..Event::text(Object::Window, member, "", 0, 0, title) });
                events.push(Event::plain(Object::Window, EVENT_OBJECT, "StateChanged", "active", focused as i32));
            }
            Change::WindowTitle(title) => {
                events.push(Event::text(Object::Window, "PropertyChange", "accessible-name", 0, 0, &title));
            }
            Change::Name { node, name } => {
                events.push(Event::text(Object::Node(node), "PropertyChange", "accessible-name", 0, 0, &name));
            }
            Change::Description { node, description } => {
                let at = Object::Node(node);
                events.push(Event::text(at, "PropertyChange", "accessible-description", 0, 0, &description));
            }
            Change::Announcement { node, text, assertive } => {
                let politeness = if assertive { 2 } else { 1 }; // (AtspiLive)
                events.push(Event::text(Object::Node(node), "Announcement", "", politeness, 0, &text));
            }
            Change::Checked { node, checked } => {
                let detail = if next.node(node).role == Some(Role::Tab) { "selected" } else { "checked" };
                events.push(Event::plain(Object::Node(node), EVENT_OBJECT, "StateChanged", detail, checked as i32));
            }
            Change::Enabled { node, enabled } => {
                for detail in ["enabled", "sensitive"] {
                    events.push(Event::plain(Object::Node(node), EVENT_OBJECT, "StateChanged", detail, enabled as i32));
                }
            }
            Change::Value { node, value } => events.push(Event {
                data: EventData::Number(value as f64),
                ..Event::plain(Object::Node(node), EVENT_OBJECT, "PropertyChange", "accessible-value", 0)
            }),
            Change::TextRemoved { node, start, length, text } => {
                let at = Object::Node(node);
                events.push(Event::text(at, "TextChanged", "delete", start as i32, length as i32, &text));
            }
            Change::TextInserted { node, start, length, text } => {
                let at = Object::Node(node);
                events.push(Event::text(at, "TextChanged", "insert", start as i32, length as i32, &text));
            }
            Change::FocusLeft(node) => {
                events.push(Event::plain(Object::Node(node), EVENT_OBJECT, "StateChanged", "focused", 0));
            }
            Change::Focused(node) => {
                events.push(Event::plain(Object::Node(node), EVENT_OBJECT, "StateChanged", "focused", 1));
                events.push(Event::plain(Object::Node(node), EVENT_FOCUS, "Focus", "", 0));
            }
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
    fn changes_are_atspi_events() {
        let node = |id, parent| AccessibilityNode { id, parent, enabled: true, ..Default::default() };
        let mut tab = node(2, Some(1));
        tab.role = Some(Role::Tab);
        let old = Tree::new(vec![node(1, None), tab.clone()], Window::default());
        tab.checked = Some(true);
        let next = Tree::new(vec![node(1, None), tab], Window { focused: false, ..Window::default() });
        let events = compare(&old, &next);
        let said: Vec<(&str, &str, i32)> = events.iter().map(|e| (e.member, e.detail.as_str(), e.detail1)).collect();
        assert_eq!(said, [("Deactivate", "", 0), ("StateChanged", "active", 0), ("StateChanged", "selected", 1)]);
        assert_eq!(events[0].interface, EVENT_WINDOW);
    }
}
