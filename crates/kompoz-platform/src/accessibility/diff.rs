//! What changed from one published tree to the next, in the order a bridge tells its listeners: subtrees gone and
//! come (on the parent that stays), the window's focus and title, each node's name, description, live region, checked
//! and enabled states, value and text, then the focus.

use super::snapshot::Snapshot;
use super::text::{edit, text_of};
use kompoz_ui::semantics::LiveRegionMode;

/// A change between two trees. Node offsets in texts are code points.
#[derive(Clone, PartialEq, Debug)]
pub enum Change {
    /// A whole subtree went: told once, on its parent that stays (none: the window), `index` its place among that
    /// parent's children in the old tree. The last ones first, so that the indexes of those before still hold.
    Removed { parent: Option<u64>, node: u64, index: usize },
    /// A whole subtree came: told once, on its parent that was there (none: the window), `index` its place among
    /// that parent's children in the new tree.
    Added { parent: Option<u64>, node: u64, index: usize },
    /// The window got the focus (true) or lost it (the first tree: if it has it).
    WindowFocused(bool),
    /// The window's title is another.
    WindowTitle(String),
    /// A node's name (its label) is another.
    Name { node: u64, name: String },
    /// A node's description (its state description) is another.
    Description { node: u64, description: String },
    /// A live region's name or description changed: what is said ("label, description"), interrupting what is
    /// being said if `assertive`.
    Announcement { node: u64, text: String, assertive: bool },
    /// A node was checked or unchecked (a tab: selected or not).
    Checked { node: u64, checked: bool },
    /// A node was enabled or disabled.
    Enabled { node: u64, enabled: bool },
    /// A range's current value is another.
    Value { node: u64, value: f32 },
    /// Text went from an editable node: `length` code points from `start`, which were `text` (a password's
    /// bullets).
    TextRemoved { node: u64, start: usize, length: usize, text: String },
    /// Text came into an editable node: `length` code points at `start`, which are `text` (a password's bullets).
    TextInserted { node: u64, start: usize, length: usize, text: String },
    /// The node that had the focus has it no longer (and is still there).
    FocusLeft(u64),
    /// The node has the focus now.
    Focused(u64),
}

/// What changed from `old` to `next` (from nothing, if `old` was never published: the window's focus and the focused
/// node, nothing else).
pub fn changes(old: &Snapshot, next: &Snapshot) -> Vec<Change> {
    let mut out = Vec::new();
    let w = next.window();
    let published = old.is_published();
    if published {
        for gone in old.nodes().iter().rev() {
            if !next.contains(gone.id) && gone.parent.is_none_or(|p| next.contains(p)) {
                let index = old.index_in(gone.parent, gone.id).expect("a node is among its parent's children");
                out.push(Change::Removed { parent: gone.parent, node: gone.id, index });
            }
        }
        for new in next.nodes() {
            if !old.contains(new.id) && new.parent.is_none_or(|p| old.contains(p)) {
                let index = next.index_in(new.parent, new.id).expect("a node is among its parent's children");
                out.push(Change::Added { parent: new.parent, node: new.id, index });
            }
        }
    }
    if if published { w.focused != old.window().focused } else { w.focused } {
        out.push(Change::WindowFocused(w.focused));
    }
    if published && w.title != old.window().title {
        out.push(Change::WindowTitle(w.title.clone()));
    }
    for n in next.nodes() {
        let Some(o) = old.get(n.id).filter(|_| published) else { continue };
        let node = n.id;
        if n.label != o.label {
            out.push(Change::Name { node, name: n.label.clone() });
        }
        if n.state_description != o.state_description {
            out.push(Change::Description { node, description: n.state_description.clone() });
        }
        if let Some(live) = n.live_region
            && (n.label != o.label || n.state_description != o.state_description)
        {
            let mut text = n.label.clone();
            if !n.state_description.is_empty() {
                if !text.is_empty() {
                    text.push_str(", ");
                }
                text.push_str(&n.state_description);
            }
            out.push(Change::Announcement { node, text, assertive: live == LiveRegionMode::Assertive });
        }
        if n.checked.unwrap_or(false) != o.checked.unwrap_or(false) {
            out.push(Change::Checked { node, checked: n.checked.unwrap_or(false) });
        }
        if n.enabled != o.enabled {
            out.push(Change::Enabled { node, enabled: n.enabled });
        }
        if let (Some(r), Some(before)) = (&n.range, &o.range)
            && r.current != before.current
        {
            out.push(Change::Value { node, value: r.current });
        }
        if n.editable
            && o.editable
            && n.value != o.value
            && let Some((start, gone, came)) = edit(&text_of(o), &text_of(n))
        {
            if !gone.is_empty() {
                out.push(Change::TextRemoved { node, start, length: gone.chars().count(), text: gone });
            }
            if !came.is_empty() {
                out.push(Change::TextInserted { node, start, length: came.chars().count(), text: came });
            }
        }
    }
    let focused_before = old.focused();
    let focused_now = next.focused();
    if focused_now != focused_before || (!published && focused_now.is_some()) {
        if published
            && let Some(before) = focused_before
            && next.contains(before)
        {
            out.push(Change::FocusLeft(before));
        }
        if let Some(now) = focused_now {
            out.push(Change::Focused(now));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::snapshot::Window;
    use super::*;
    use kompoz_ui::semantics::{AccessibilityNode, ProgressBarRangeInfo};

    fn node(id: u64, parent: Option<u64>) -> AccessibilityNode {
        AccessibilityNode { id, parent, enabled: true, ..Default::default() }
    }

    #[test]
    fn the_first_tree_tells_only_the_focus() {
        let mut field = node(2, Some(1));
        field.focused = true;
        let first = Snapshot::new(vec![node(1, None), field], Window::default());
        assert_eq!(changes(&Snapshot::default(), &first), [Change::WindowFocused(true), Change::Focused(2)]);
        let unfocused = Snapshot::new(vec![node(1, None)], Window { focused: false, ..Window::default() });
        assert!(changes(&Snapshot::default(), &unfocused).is_empty());
    }

    #[test]
    fn subtrees_states_texts_and_the_focus_change() {
        let mut before = vec![node(1, None), node(2, Some(1)), node(3, Some(2)), node(4, Some(1)), node(5, None)];
        before[3].editable = true;
        before[3].value = "merhaba".into();
        before[3].focused = true;
        before[4].range = Some(ProgressBarRangeInfo::new(0.2, 0.0..=1.0, 0));
        let mut after = vec![node(1, None), node(4, Some(1)), node(6, Some(4)), node(5, None)];
        after[1].editable = true;
        after[1].value = "merhabalar".into();
        after[1].label = "Selam".into();
        after[1].live_region = Some(LiveRegionMode::Assertive);
        after[3].range = Some(ProgressBarRangeInfo::new(0.5, 0.0..=1.0, 0));
        after[3].enabled = false;
        after[3].focused = true;
        let old = Snapshot::new(before, Window::default());
        let next = Snapshot::new(after, Window { title: "Yeni".into(), ..Window::default() });
        assert_eq!(
            changes(&old, &next),
            [
                // (node 3 went with its parent 2: told once)
                Change::Removed { parent: Some(1), node: 2, index: 0 },
                Change::Added { parent: Some(4), node: 6, index: 0 },
                Change::WindowTitle("Yeni".into()),
                Change::Name { node: 4, name: "Selam".into() },
                Change::Announcement { node: 4, text: "Selam".into(), assertive: true },
                Change::TextInserted { node: 4, start: 7, length: 3, text: "lar".into() },
                Change::Enabled { node: 5, enabled: false },
                Change::Value { node: 5, value: 0.5 },
                Change::FocusLeft(4),
                Change::Focused(5),
            ]
        );
    }
}
