//! Screen readers on the web (the AT-SPI bridge's parity, in ARIA): the semantics tree mirrored as a tree of the
//! page's elements over the canvas — each node an element where the node is (its bounds, in CSS pixels from its
//! parent's corner: screen readers' focus rectangles and a phone's touch exploration land where the app draws it),
//! transparent and letting the pointer through, with the node's role, name, description and states as ARIA says
//! them ([`view_of`]). web.js makes the elements; this module says what they are and what changes ([`Mirror`]):
//! each published tree against the one before (the shared [`changes`](crate::accessibility::changes): subtrees
//! gone, the focus, live regions to say; each node's attributes against what they were), as operations, a line
//! each ([`Op::encode`]) — no rebuild of what didn't change.
//!
//! What screen readers do comes back as the node's semantics actions ([`Mirror::action`]): a click (an element's
//! `click`, which a screen reader's activation is), a long click (`contextmenu`), the keyboard's focus put on it
//! (`focus`), a slider's value stepped (its keys while it isn't the app's focused node), a page scrolled (a scroll
//! of its element), a custom action (a button of its own beside the node), the selection set in a field.
//!
//! A text field's element while the app's input method is on it is the page's hidden text area itself (linked:
//! [`Op::Link`]): what the screen reader reads and edits there is what the keyboard types into, the field's text
//! and selection (a password field's typed into a password input instead, which screen readers don't read out).
//!
//! `KOMPOZ_ARIA=0` turns the mirror off.

use crate::accessibility::text::{layout_for, lines_of, text_of};
use crate::accessibility::{Change, Snapshot, changes};
use kompoz_ui::semantics::{AccessibilityNode, ProgressBarRangeInfo, Role, SemanticsAction, ToggleableState};
use std::collections::{BTreeMap, HashMap};

/// A node's element as the page has it: its attributes by name — ARIA's (`role`, `aria-…`, `tabindex`), the web
/// platform's own (`data-…`: what web.js listens for), `#text` (the element's text) and `#rect` (where it is:
/// left, top, width, height, CSS pixels from its parent element's corner).
pub type View = BTreeMap<&'static str, String>;

/// What a slider's or a picker's keys step it by, as Compose's accessibility does without steps: a twentieth of
/// its range (`AccessibilityRangeStepCount`).
const RANGE_STEPS: f32 = 20.0;

/// An operation on the mirror's elements.
#[derive(Clone, PartialEq, Debug)]
pub enum Op {
    /// The element of a node that came (placed by an [`Order`](Op::Order) of its parent's).
    New(u64),
    /// An attribute of a node's element set.
    Attr(u64, &'static str, String),
    /// An attribute of a node's element taken away.
    Unattr(u64, &'static str),
    /// The elements of a node's children (none: the window's top nodes) in this order (those that came, and those
    /// that moved here).
    Order(Option<u64>, Vec<u64>),
    /// A node's element and what is still in it gone.
    Remove(u64),
    /// The keyboard's focus on a node's element (none: back on the canvas, if it was in the mirror).
    Focus(Option<u64>),
    /// The text area the keyboard types into linked to an editable node (its element while it is), that field
    /// multi-line or not, a password or not; none: no field linked.
    Link(Option<(u64, bool, bool)>),
    /// Said by the screen reader: politely (after what it says), or at once (`assertive`).
    Say(String, bool),
}

/// A string field of an operation's line: `%`, tabs and line ends percent-encoded (web.js decodes it with
/// `decodeURIComponent`).
fn field(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\t' => out.push_str("%09"),
            '\n' => out.push_str("%0A"),
            '\r' => out.push_str("%0D"),
            _ => out.push(c),
        }
    }
    out
}

impl Op {
    /// Its line for web.js: the operation's name, then its fields, between tabs.
    pub fn encode(&self) -> String {
        let id = |n: Option<u64>| n.map(|n| n.to_string()).unwrap_or_default();
        match self {
            Op::New(n) => format!("new\t{n}"),
            Op::Attr(n, name, value) => format!("attr\t{n}\t{name}\t{}", field(value)),
            Op::Unattr(n, name) => format!("unattr\t{n}\t{name}"),
            Op::Order(parent, children) => {
                let list: Vec<String> = children.iter().map(u64::to_string).collect();
                format!("order\t{}\t{}", id(*parent), list.join(","))
            }
            Op::Remove(n) => format!("remove\t{n}"),
            Op::Focus(n) => format!("focus\t{}", id(*n)),
            Op::Link(None) => "link\t".to_string(),
            Op::Link(Some((n, multi_line, password))) => {
                format!("link\t{n}\t{}\t{}", *multi_line as u8, *password as u8)
            }
            Op::Say(text, assertive) => format!("say\t{}\t{}", *assertive as u8, field(text)),
        }
    }
}

/// The lines of `ops`, for web.js.
pub fn encode(ops: &[Op]) -> String {
    ops.iter().map(Op::encode).collect::<Vec<_>>().join("\n")
}

/// The role a node's element has (none: a plain element — a group of no name, a piece of text).
pub fn role_of(tree: &Snapshot, n: &AccessibilityNode) -> Option<&'static str> {
    let explicit = match n.role {
        Some(Role::Button) => Some("button"),
        Some(Role::Checkbox) => Some("checkbox"),
        Some(Role::Switch) => Some("switch"),
        Some(Role::RadioButton) => Some("radio"),
        Some(Role::Tab) => Some("tab"),
        Some(Role::Image) => Some("img"),
        Some(Role::DropdownList) => Some("combobox"),
        Some(Role::ValuePicker) => Some("spinbutton"),
        // (The ARIA carousel pattern: a group described as a carousel.)
        Some(Role::Carousel) => Some("group"),
        None => None,
    };
    if explicit.is_some() {
        return explicit;
    }
    if n.editable {
        return Some("textbox");
    }
    if n.is_dialog {
        return Some("dialog");
    }
    if n.heading {
        return Some("heading");
    }
    if n.link {
        return Some("link");
    }
    if let Some(r) = &n.range {
        return Some(if n.settable && *r != ProgressBarRangeInfo::Indeterminate { "slider" } else { "progressbar" });
    }
    let kids = tree.children(Some(n.id));
    if n.selectable_group {
        let any = |f: &dyn Fn(&AccessibilityNode) -> bool| kids.iter().any(|&k| f(tree.node(k)));
        if any(&|k| k.role == Some(Role::Tab)) {
            return Some("tablist");
        }
        if any(&|k| k.role == Some(Role::RadioButton)) {
            return Some("radiogroup");
        }
        if any(&|k| k.role.is_none() && k.selected.is_some()) {
            return Some("listbox");
        }
    }
    if let Some(c) = n.collection_info {
        // (A grid: more than one row and column, or a lazy grid's unknown ones.)
        return Some(if c.row_count != 1 && c.column_count != 1 { "grid" } else { "list" });
    }
    if n.toggleable_state.is_some() {
        return Some("checkbox");
    }
    if let Some(parent) = n.parent.map(|p| tree.node(p)) {
        if parent.selectable_group && n.selected.is_some() && role_of(tree, parent) == Some("listbox") {
            return Some("option");
        }
        if let Some(c) = parent.collection_info {
            return Some(if c.row_count != 1 && c.column_count != 1 { "gridcell" } else { "listitem" });
        }
    }
    if n.clickable {
        return Some("button");
    }
    let name = !n.label.is_empty() || n.pane_title.is_some();
    if n.pane_title.is_some() {
        return Some("region");
    }
    if name && !kids.is_empty() {
        return Some("group");
    }
    None
}

/// Whether an element of `role` takes its name from what it holds (its text read where it is, in a screen
/// reader's browse mode) rather than only from `aria-label`.
fn named_by_content(role: Option<&str>) -> bool {
    const BY_CONTENT: [&str; 10] =
        ["button", "link", "heading", "tab", "checkbox", "radio", "switch", "listitem", "gridcell", "option"];
    role.is_none_or(|r| BY_CONTENT.contains(&r))
}

/// A number as an attribute's value (without a fraction where it has none).
fn number(v: f32) -> String {
    if v.fract() == 0.0 && v.abs() < 1e9 { format!("{}", v as i64) } else { format!("{v}") }
}

/// Where a node is, CSS pixels from its parent's corner (`k`: CSS pixels per px).
fn rect_of(tree: &Snapshot, n: &AccessibilityNode, k: f32) -> String {
    let b = n.bounds;
    let (left, top) = match n.parent.and_then(|p| tree.get(p)) {
        Some(p) => (b.left - p.bounds.left, b.top - p.bounds.top),
        None => (b.left, b.top),
    };
    let r = |v: f32| (v * k * 100.0).round() / 100.0;
    format!("{} {} {} {}", r(left), r(top), r(b.width().max(0.0)), r(b.height().max(0.0)))
}

/// Where the lines of an element's text (`text`, its `#text`) are as the node's text layout lays them out, its
/// text being the layout's (a field's: character for character): a line each, `start end left top width height`
/// (UTF-16 units of the text; CSS pixels from the element's corner, `k` a px), `;` between them — web.js puts each
/// line where the canvas draws it, so that what a screen reader highlights, and its character extents, are there.
/// None without a layout (the text then flows in the element).
fn lines_in(n: &AccessibilityNode, text: &str, k: f32) -> Option<String> {
    let layout = layout_for(n, text).filter(|l| n.editable || l.text == text)?;
    let units: Vec<usize> = std::iter::once(0)
        .chain(text.chars().scan(0, |at, c| {
            *at += c.len_utf16();
            Some(*at)
        }))
        .collect();
    let r = |v: f32| (v * k * 100.0).round() / 100.0;
    let lines: Vec<String> = lines_of(layout)
        .into_iter()
        .zip(&layout.lines)
        .map(|((s, e), line)| {
            let b = line.rect;
            let (start, end) = (units[s.max(0) as usize], units[(e.max(s) as usize).min(units.len() - 1)]);
            let (left, top) = (b.left - n.bounds.left, b.top - n.bounds.top);
            format!("{start} {end} {} {} {} {}", r(left), r(top), r(b.width().max(0.0)), r(b.height().max(0.0)))
        })
        .collect();
    Some(lines.join(";"))
}

/// A node's element (see [`View`]), `k` CSS pixels a px.
pub fn view_of(tree: &Snapshot, n: &AccessibilityNode, k: f32) -> View {
    let mut v = View::new();
    let mut set = |name: &'static str, value: String| {
        v.insert(name, value);
    };
    let role = role_of(tree, n);
    if let Some(r) = role {
        set("role", r.to_string());
    }
    if n.role == Some(Role::Carousel) {
        set("aria-roledescription", "carousel".into());
    }
    let name = if n.label.is_empty() { n.pane_title.clone().unwrap_or_default() } else { n.label.clone() };
    let leaf = tree.children(Some(n.id)).is_empty();
    if n.editable {
        // (A field: its label its name, its text what it holds — a password's bullets.)
        if !name.is_empty() {
            set("aria-label", name);
        }
        let text = text_of(n);
        if let Some(lines) = lines_in(n, &text, k) {
            set("#lines", lines);
        }
        set("#text", text);
    } else if leaf && named_by_content(role) {
        if !name.is_empty() {
            if let Some(lines) = lines_in(n, &name, k) {
                set("#lines", lines);
            }
            set("#text", name);
        }
    } else if !name.is_empty() {
        set("aria-label", name);
    }
    let range = n.range.as_ref().filter(|r| **r != ProgressBarRangeInfo::Indeterminate);
    // (A range's state description says its value: its `aria-valuetext`.)
    let mut description: Vec<&str> = Vec::new();
    if !n.state_description.is_empty() {
        if range.is_some() {
            set("aria-valuetext", n.state_description.clone());
        } else {
            description.push(&n.state_description);
        }
    }
    if let Some(e) = n.error.as_deref() {
        set("aria-invalid", "true".into());
        if !e.is_empty() {
            description.push(e);
        }
    }
    if !description.is_empty() {
        set("aria-description", description.join(", "));
    }
    if !n.enabled {
        set("aria-disabled", "true".into());
    }
    let tri = |s: ToggleableState| match s {
        ToggleableState::On => "true",
        ToggleableState::Off => "false",
        ToggleableState::Indeterminate => "mixed",
    };
    match role {
        Some("button") => {
            let pressed = n.toggleable_state.map(tri).or(n.selected.map(|s| if s { "true" } else { "false" }));
            if let Some(p) = pressed {
                set("aria-pressed", p.into());
            }
        }
        Some("checkbox" | "switch" | "radio") => {
            let checked =
                n.toggleable_state.map(tri).or(n.selected.map(|s| if s { "true" } else { "false" })).unwrap_or("false");
            set("aria-checked", checked.into());
        }
        Some("tab" | "option" | "gridcell") => {
            if let Some(s) = n.selected.or(n.checked) {
                set("aria-selected", s.to_string());
            }
        }
        _ => {}
    }
    if let Some(e) = n.expanded {
        set("aria-expanded", e.to_string());
    }
    if n.is_dialog {
        set("aria-modal", "true".into());
    }
    if let Some(r) = range {
        set("aria-valuemin", number(*r.range.start()));
        set("aria-valuemax", number(*r.range.end()));
        set("aria-valuenow", number(r.current));
        if n.settable {
            let span = r.range.end() - r.range.start();
            let step = if r.steps > 0 { span / (r.steps + 1) as f32 } else { span / RANGE_STEPS };
            set("data-step", number(step));
        }
    }
    // A collection's size, an item's place in it (ARIA's are from 1; -1: unknown).
    if let Some(c) = n.collection_info
        && role == Some("grid")
    {
        set("aria-rowcount", c.row_count.max(-1).to_string());
        set("aria-colcount", c.column_count.max(-1).to_string());
    }
    if let Some(parent) = n.parent.map(|p| tree.node(p))
        && let Some(c) = parent.collection_info
    {
        let item = n.collection_item_info;
        match role {
            Some("gridcell") => {
                if let Some(i) = item {
                    set("aria-rowindex", (i.row_index + 1).to_string());
                    set("aria-colindex", (i.column_index + 1).to_string());
                }
            }
            Some("listitem") => {
                // (A row of a column, or a column of a row.)
                let (size, index) = if c.row_count == 1 {
                    (c.column_count, item.map(|i| i.column_index))
                } else {
                    (c.row_count, item.map(|i| i.row_index))
                };
                set("aria-setsize", size.max(-1).to_string());
                if let Some(i) = index {
                    set("aria-posinset", (i + 1).to_string());
                }
            }
            _ => {}
        }
    } else if let (Some(i), Some("option" | "radio" | "tab")) = (n.collection_item_info, role) {
        // (A selectable group's selectables: their place among them, their number.)
        let parent = n.parent.map(|p| tree.children(Some(p))).unwrap_or_default();
        let count = parent.iter().filter(|&&k| tree.node(k).selected.is_some()).count();
        set("aria-posinset", (i.row_index.max(i.column_index) + 1).to_string());
        set("aria-setsize", count.to_string());
    }
    if n.clickable || n.editable || n.settable || n.focused || n.toggleable_state.is_some() || n.selected.is_some() {
        set("tabindex", "-1".into());
    }
    if n.clickable {
        set("data-click", "1".into());
    }
    if n.scrollable {
        set("data-scroll", "1".into());
    }
    if !n.custom_actions.is_empty() {
        set("data-actions", n.custom_actions.join("\n"));
    }
    if let Some(tag) = &n.test_tag {
        set("data-tag", tag.clone());
    }
    set("#rect", rect_of(tree, n, k));
    v
}

/// What a screen reader did to a node's element (web.js's `aria` records).
#[derive(Clone, PartialEq, Debug)]
pub enum AriaAction {
    /// Activated (its `click`).
    Click,
    /// Its context menu asked for (`contextmenu`: a screen reader's long press).
    LongClick,
    /// The keyboard's focus put on it (its `focus`, by the screen reader or the Tab key of a page around).
    Focus,
    /// A slider stepped to this value (its keys).
    Progress(f32),
    /// A page scrolled, forward or back.
    Scroll(bool),
    /// Its custom action of this index (the button of it beside the node).
    Custom(usize),
}

/// The mirror as the page has it: the tree it shows, each node's element, the field linked, the focus.
#[derive(Default)]
pub struct Mirror {
    tree: Snapshot,
    views: HashMap<u64, View>,
    linked: Option<(u64, bool, bool)>,
}

impl Mirror {
    /// The tree `next` (`k` CSS pixels a px) shown instead, the app's input method on a field (`multi_line`) or
    /// not (none): the operations that make the page's elements so.
    pub fn update(&mut self, next: Snapshot, k: f32, input: Option<bool>) -> Vec<Op> {
        let told = changes(&self.tree, &next);
        let mut ops = Vec::new();
        for n in next.nodes() {
            if !self.views.contains_key(&n.id) {
                ops.push(Op::New(n.id));
            }
        }
        let mut views = HashMap::with_capacity(next.nodes().len());
        for n in next.nodes() {
            let view = view_of(&next, n, k);
            let empty = View::new();
            let old = self.views.get(&n.id).unwrap_or(&empty);
            for (name, value) in &view {
                if old.get(name) != Some(value) {
                    ops.push(Op::Attr(n.id, name, value.clone()));
                }
            }
            for name in old.keys() {
                if !view.contains_key(name) {
                    ops.push(Op::Unattr(n.id, name));
                }
            }
            views.insert(n.id, view);
        }
        // Children in their places: each parent whose children aren't those it had in that order (a new parent's).
        let parents = std::iter::once(None).chain(next.nodes().iter().map(|n| Some(n.id)));
        for parent in parents {
            let kids = next.children(parent);
            let had = if parent.is_none_or(|p| self.tree.contains(p)) { self.tree.children(parent) } else { &[] };
            if kids != had && !kids.is_empty() {
                ops.push(Op::Order(parent, kids.to_vec()));
            }
        }
        // (After the orders: what moved out of a subtree that goes has its place already.)
        let mut focus = None;
        let mut said = Vec::new();
        for c in &told {
            match c {
                Change::Removed { node, .. } => ops.push(Op::Remove(*node)),
                Change::Focused(n) => focus = Some(Some(*n)),
                Change::FocusLeft(_) if focus.is_none() => focus = Some(None),
                Change::Announcement { text, assertive, .. } => said.push(Op::Say(text.clone(), *assertive)),
                _ => {}
            }
        }
        let linked = next
            .focused()
            .map(|f| next.node(f))
            .filter(|f| f.editable)
            .and_then(|f| input.map(|multi_line| (f.id, multi_line, f.password)));
        if linked != self.linked {
            ops.push(Op::Link(linked));
            self.linked = linked;
        }
        if let Some(f) = focus {
            ops.push(Op::Focus(f));
        }
        // A pane's title said as it comes or changes (as Android tells a pane's appearing).
        if self.tree.is_published() {
            for n in next.nodes() {
                if let Some(title) = n.pane_title.as_ref().filter(|t| !t.is_empty())
                    && self.tree.get(n.id).is_none_or(|o| o.pane_title.as_ref() != Some(title))
                {
                    ops.push(Op::Say(title.clone(), false));
                }
            }
        }
        ops.extend(said);
        self.views = views;
        self.tree = next;
        ops
    }

    /// The semantics action a screen reader's `action` on node `id` is (none: not a node shown, or nothing to do):
    /// an expandable's click its expanding or collapsing where it has no click of its own.
    pub fn action(&self, id: u64, action: AriaAction) -> Option<SemanticsAction> {
        let n = self.tree.get(id)?;
        Some(match action {
            AriaAction::Click => match n.expanded {
                Some(expanded) if !n.clickable => {
                    if expanded {
                        SemanticsAction::Collapse
                    } else {
                        SemanticsAction::Expand
                    }
                }
                _ => SemanticsAction::Click,
            },
            AriaAction::LongClick => SemanticsAction::LongClick,
            AriaAction::Focus => {
                if n.focused {
                    return None;
                }
                SemanticsAction::Focus
            }
            AriaAction::Progress(v) => {
                let r = n.range.as_ref().filter(|_| n.settable)?;
                SemanticsAction::SetProgress(v.clamp(*r.range.start(), *r.range.end()))
            }
            AriaAction::Scroll(forward) => {
                if !n.scrollable {
                    return None;
                }
                if forward { SemanticsAction::ScrollForward } else { SemanticsAction::ScrollBackward }
            }
            AriaAction::Custom(i) => {
                if i >= n.custom_actions.len() {
                    return None;
                }
                SemanticsAction::CustomAction(i)
            }
        })
    }

    /// The field linked to the text area, if one is.
    pub fn linked(&self) -> Option<u64> {
        self.linked.map(|(id, ..)| id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::Window;
    use kompoz_graphics::Rect;
    use kompoz_ui::semantics::{CollectionInfo, CollectionItemInfo, LiveRegionMode};

    fn node(id: u64, parent: Option<u64>, bounds: Rect) -> AccessibilityNode {
        AccessibilityNode { id, parent, bounds, enabled: true, ..Default::default() }
    }

    fn tree(nodes: Vec<AccessibilityNode>) -> Snapshot {
        Snapshot::new(nodes, Window::default())
    }

    fn attrs(v: &View) -> Vec<(&str, &str)> {
        v.iter().map(|(k, v)| (*k, v.as_str())).collect()
    }

    #[test]
    fn roles_names_and_states_are_arias() {
        let full = Rect::new(0.0, 0.0, 400.0, 800.0);
        let mut button = node(2, Some(1), Rect::new(10.0, 20.0, 110.0, 60.0));
        button.label = "Tamam".into();
        button.role = Some(Role::Button);
        button.clickable = true;
        let mut check = node(3, Some(1), Rect::new(10.0, 70.0, 50.0, 110.0));
        check.label = "Kabul".into();
        check.role = Some(Role::Checkbox);
        check.toggleable_state = Some(ToggleableState::Indeterminate);
        check.clickable = true;
        let mut slider = node(4, Some(1), Rect::new(0.0, 120.0, 400.0, 160.0));
        slider.range = Some(ProgressBarRangeInfo::new(0.25, 0.0..=1.0, 3));
        slider.settable = true;
        slider.state_description = "Çeyrek".into();
        slider.enabled = false;
        let mut field = node(5, Some(1), Rect::new(0.0, 200.0, 400.0, 260.0));
        field.editable = true;
        field.password = true;
        field.value = "gizli".into();
        field.label = "Parola".into();
        field.error = Some("Çok kısa".into());
        let mut heading = node(6, Some(1), Rect::new(0.0, 300.0, 400.0, 340.0));
        heading.heading = true;
        heading.label = "Başlık".into();
        let mut dialog = node(7, Some(1), Rect::new(0.0, 400.0, 400.0, 800.0));
        dialog.is_dialog = true;
        dialog.pane_title = Some("Pencere".into());
        let mut image = node(8, Some(7), Rect::new(10.0, 410.0, 50.0, 450.0));
        image.role = Some(Role::Image);
        image.label = "Kedi".into();
        let mut progress = node(9, Some(7), Rect::new(10.0, 460.0, 50.0, 470.0));
        progress.range = Some(ProgressBarRangeInfo::Indeterminate);
        let mut menu = node(10, Some(7), Rect::new(10.0, 480.0, 50.0, 490.0));
        menu.clickable = true;
        menu.expanded = Some(false);
        menu.label = "Daha".into();
        let mut link = node(11, Some(7), Rect::new(10.0, 500.0, 50.0, 510.0));
        link.link = true;
        link.clickable = true;
        link.label = "Site".into();
        let t =
            tree(vec![node(1, None, full), button, check, slider, field, heading, dialog, image, progress, menu, link]);
        let view = |id| view_of(&t, t.node(id), 0.5);
        assert_eq!(
            attrs(&view(2)),
            [("#rect", "5 10 50 20"), ("#text", "Tamam"), ("data-click", "1"), ("role", "button"), ("tabindex", "-1")]
        );
        assert_eq!(view(3).get("aria-checked").map(String::as_str), Some("mixed"));
        assert_eq!(
            attrs(&view(4)),
            [
                ("#rect", "0 60 200 20"),
                ("aria-disabled", "true"),
                ("aria-valuemax", "1"),
                ("aria-valuemin", "0"),
                ("aria-valuenow", "0.25"),
                ("aria-valuetext", "Çeyrek"),
                ("data-step", "0.25"),
                ("role", "slider"),
                ("tabindex", "-1"),
            ]
        );
        assert_eq!(
            attrs(&view(5)),
            [
                ("#rect", "0 100 200 30"),
                ("#text", "•••••"),
                ("aria-description", "Çok kısa"),
                ("aria-invalid", "true"),
                ("aria-label", "Parola"),
                ("role", "textbox"),
                ("tabindex", "-1"),
            ]
        );
        assert_eq!(attrs(&view(6)), [("#rect", "0 150 200 20"), ("#text", "Başlık"), ("role", "heading")]);
        // A text laid out: its lines where they are drawn (five characters a line), from its corner.
        let mut text = node(12, None, Rect::new(10.0, 40.0, 210.0, 80.0));
        text.label = "Uzun başlık".into();
        let layout = crate::accessibility::text::test_layout(
            "Uzun başlık",
            kompoz_graphics::Offset::new(12.0, 40.0),
            8.0,
            20.0,
            5,
        );
        text.text_layout = Some(layout.into());
        let t2 = tree(vec![node(1, None, full), text]);
        assert_eq!(
            view_of(&t2, t2.node(12), 0.5).get("#lines").map(String::as_str),
            Some("0 5 1 0 20 10;5 10 1 10 20 10;10 11 1 20 4 10")
        );
        assert_eq!(
            attrs(&view(7)),
            [("#rect", "0 200 200 200"), ("aria-label", "Pencere"), ("aria-modal", "true"), ("role", "dialog")]
        );
        assert_eq!(attrs(&view(8)), [("#rect", "5 5 20 20"), ("aria-label", "Kedi"), ("role", "img")]);
        assert_eq!(attrs(&view(9)), [("#rect", "5 30 20 5"), ("role", "progressbar")]);
        assert_eq!(view(10).get("aria-expanded").map(String::as_str), Some("false"));
        assert_eq!(view(11).get("role").map(String::as_str), Some("link"));
        // (The root: a plain element, no name.)
        assert_eq!(attrs(&view(1)), [("#rect", "0 0 200 400")]);
    }

    #[test]
    fn groups_and_collections_say_their_members_places() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        let mut tabs = node(1, None, r);
        tabs.selectable_group = true;
        let tab = |id, selected| {
            let mut t = node(id, Some(1), r);
            t.role = Some(Role::Tab);
            t.selected = Some(selected);
            t.checked = Some(selected);
            t.collection_item_info =
                Some(CollectionItemInfo { row_index: 0, row_span: 1, column_index: id as i32 - 2, column_span: 1 });
            t
        };
        let (a, b) = (tab(2, true), tab(3, false));
        let mut list = node(4, None, r);
        list.collection_info = Some(CollectionInfo { row_count: 100, column_count: 1 });
        list.scrollable = true;
        let mut item = node(5, Some(4), r);
        item.label = "Öğe".into();
        item.clickable = true;
        item.collection_item_info =
            Some(CollectionItemInfo { row_index: 41, row_span: 1, column_index: 0, column_span: 1 });
        let mut grid = node(6, None, r);
        grid.collection_info = Some(CollectionInfo { row_count: -1, column_count: -1 });
        let cell = node(7, Some(6), r);
        let mut options = node(8, None, r);
        options.selectable_group = true;
        let mut option = node(9, Some(8), r);
        option.selected = Some(false);
        option.clickable = true;
        let t = tree(vec![tabs, a, b, list, item, grid, cell, options, option]);
        let get = |id, name| view_of(&t, t.node(id), 1.0).get(name).cloned();
        assert_eq!(get(1, "role").as_deref(), Some("tablist"));
        assert_eq!(get(2, "aria-selected").as_deref(), Some("true"));
        assert_eq!(get(3, "aria-selected").as_deref(), Some("false"));
        assert_eq!((get(3, "aria-posinset"), get(3, "aria-setsize")), (Some("2".into()), Some("2".into())));
        assert_eq!(get(4, "role").as_deref(), Some("list"));
        assert_eq!(get(4, "data-scroll").as_deref(), Some("1"));
        assert_eq!(get(5, "role").as_deref(), Some("listitem"));
        assert_eq!((get(5, "aria-posinset"), get(5, "aria-setsize")), (Some("42".into()), Some("100".into())));
        assert_eq!(get(5, "data-click").as_deref(), Some("1"));
        assert_eq!(get(6, "role").as_deref(), Some("grid"));
        assert_eq!((get(6, "aria-rowcount"), get(6, "aria-colcount")), (Some("-1".into()), Some("-1".into())));
        assert_eq!(get(7, "role").as_deref(), Some("gridcell"));
        assert_eq!(get(8, "role").as_deref(), Some("listbox"));
        assert_eq!((get(9, "role"), get(9, "aria-selected")), (Some("option".into()), Some("false".into())));
    }

    #[test]
    fn changes_are_operations_on_what_changed() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let mut a = node(2, Some(1), Rect::new(0.0, 0.0, 50.0, 10.0));
        a.label = "Bir".into();
        a.clickable = true;
        let b = node(3, Some(1), Rect::new(0.0, 10.0, 50.0, 20.0));
        let mut field = node(4, Some(3), Rect::new(0.0, 10.0, 50.0, 20.0));
        field.editable = true;
        field.focused = true;
        let mut mirror = Mirror::default();
        let ops = mirror.update(tree(vec![node(1, None, r), a.clone(), b.clone(), field.clone()]), 1.0, Some(false));
        assert_eq!(&ops[..4], [Op::New(1), Op::New(2), Op::New(3), Op::New(4)]);
        assert!(ops.contains(&Op::Attr(2, "#text", "Bir".into())));
        let tail: Vec<&Op> = ops.iter().filter(|o| !matches!(o, Op::New(_) | Op::Attr(..))).collect();
        assert_eq!(
            tail,
            [
                &Op::Order(None, vec![1]),
                &Op::Order(Some(1), vec![2, 3]),
                &Op::Order(Some(3), vec![4]),
                &Op::Link(Some((4, false, false))),
                &Op::Focus(Some(4)),
            ]
        );
        // The same tree: nothing.
        let same = tree(vec![node(1, None, r), a.clone(), b.clone(), field.clone()]);
        assert!(mirror.update(same, 1.0, Some(false)).is_empty());
        // A name changed, a live region said, a subtree gone, a node come, the order changed, the focus moved.
        let mut a2 = a.clone();
        a2.label = "İki".into();
        a2.live_region = Some(LiveRegionMode::Polite);
        a2.focused = true;
        let mut c = node(5, Some(1), Rect::new(0.0, 20.0, 50.0, 30.0));
        c.pane_title = Some("Pano".into());
        let ops = mirror.update(tree(vec![node(1, None, r), c, a2]), 1.0, None);
        assert_eq!(
            ops,
            [
                Op::New(5),
                Op::Attr(5, "#rect", "0 20 50 10".into()),
                Op::Attr(5, "aria-label", "Pano".into()),
                Op::Attr(5, "role", "region".into()),
                Op::Attr(2, "#text", "İki".into()),
                Op::Order(Some(1), vec![5, 2]),
                Op::Remove(3),
                Op::Link(None),
                Op::Focus(Some(2)),
                Op::Say("Pano".into(), false),
                Op::Say("İki".into(), false),
            ]
        );
        assert_eq!(encode(&ops[..2]), "new\t5\nattr\t5\t#rect\t0 20 50 10");
        assert_eq!(Op::Attr(1, "aria-label", "50%\tx\ny".into()).encode(), "attr\t1\taria-label\t50%25%09x%0Ay");
        assert_eq!(Op::Order(None, vec![1, 2]).encode(), "order\t\t1,2");
        assert_eq!(Op::Link(Some((4, true, false))).encode(), "link\t4\t1\t0");
    }

    #[test]
    fn screen_readers_actions_are_the_nodes() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        let mut menu = node(1, None, r);
        menu.expanded = Some(true);
        let mut slider = node(2, None, r);
        slider.range = Some(ProgressBarRangeInfo::new(0.5, 0.0..=1.0, 0));
        slider.settable = true;
        slider.custom_actions = vec!["Sil".into()];
        let mut list = node(3, None, r);
        list.scrollable = true;
        list.focused = true;
        let mut mirror = Mirror::default();
        mirror.update(tree(vec![menu, slider, list]), 1.0, None);
        assert_eq!(mirror.action(1, AriaAction::Click), Some(SemanticsAction::Collapse));
        assert_eq!(mirror.action(2, AriaAction::Click), Some(SemanticsAction::Click));
        assert_eq!(mirror.action(2, AriaAction::Progress(1.5)), Some(SemanticsAction::SetProgress(1.0)));
        assert_eq!(mirror.action(1, AriaAction::Progress(0.1)), None);
        assert_eq!(mirror.action(3, AriaAction::Scroll(false)), Some(SemanticsAction::ScrollBackward));
        assert_eq!(mirror.action(2, AriaAction::Scroll(true)), None);
        assert_eq!(mirror.action(2, AriaAction::Custom(0)), Some(SemanticsAction::CustomAction(0)));
        assert_eq!(mirror.action(2, AriaAction::Custom(1)), None);
        assert_eq!(mirror.action(2, AriaAction::Focus), Some(SemanticsAction::Focus));
        assert_eq!(mirror.action(3, AriaAction::Focus), None);
        assert_eq!(mirror.action(9, AriaAction::Click), None);
    }
}
