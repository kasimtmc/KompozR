//! A published semantics tree: its nodes as the host gave them (parents before their children, siblings in order),
//! each one by its id, each one's children, and the window they are in.

use kompoz_ui::semantics::AccessibilityNode;
use std::collections::HashMap;

/// The window a tree is in: where it is on the screen and its size, in the platform's points (the units the
/// accessibility APIs' bounds are in), points per px (the nodes' bounds are in px), whether it has the focus, its
/// title.
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

/// A tree as published: its nodes, where each is, each one's children (none: the window's), the window. The default
/// is the empty one before the first was published.
#[derive(Default)]
pub struct Snapshot {
    nodes: Vec<AccessibilityNode>,
    at: HashMap<u64, usize>,
    children: HashMap<Option<u64>, Vec<u64>>,
    window: Window,
    published: bool,
}

impl Snapshot {
    /// The tree of `nodes` (as [`KompozHost::accessibility_tree`](kompoz_ui::KompozHost::accessibility_tree) gives
    /// them) in `window`, published.
    pub fn new(nodes: Vec<AccessibilityNode>, window: Window) -> Snapshot {
        let mut at = HashMap::new();
        let mut children: HashMap<Option<u64>, Vec<u64>> = HashMap::new();
        for (i, n) in nodes.iter().enumerate() {
            at.insert(n.id, i);
            children.entry(n.parent).or_default().push(n.id);
        }
        Snapshot { nodes, at, children, window, published: true }
    }

    /// One was published (else this is the empty one before).
    pub fn is_published(&self) -> bool {
        self.published
    }

    /// Its nodes, parents before their children.
    pub fn nodes(&self) -> &[AccessibilityNode] {
        &self.nodes
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn contains(&self, id: u64) -> bool {
        self.at.contains_key(&id)
    }

    /// The node `id`, if it is in the tree.
    pub fn get(&self, id: u64) -> Option<&AccessibilityNode> {
        self.at.get(&id).map(|&i| &self.nodes[i])
    }

    /// The node `id` (which is in the tree: panics otherwise).
    pub fn node(&self, id: u64) -> &AccessibilityNode {
        &self.nodes[self.at[&id]]
    }

    /// The children of `parent` (none: the window's top nodes), in order.
    pub fn children(&self, parent: Option<u64>) -> &[u64] {
        self.children.get(&parent).map_or(&[], Vec::as_slice)
    }

    /// Where `child` is among `parent`'s children, if it is one of them.
    pub fn index_in(&self, parent: Option<u64>, child: u64) -> Option<usize> {
        self.children(parent).iter().position(|&id| id == child)
    }

    /// The node that has the focus (the last one saying so), if one has.
    pub fn focused(&self) -> Option<u64> {
        self.nodes.iter().filter(|n| n.focused).map(|n| n.id).next_back()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nodes_are_found_by_their_ids_and_parents() {
        let node = |id, parent, focused| AccessibilityNode { id, parent, focused, ..Default::default() };
        let tree = Snapshot::new(
            vec![node(1, None, false), node(2, Some(1), true), node(3, Some(1), false), node(4, None, false)],
            Window::default(),
        );
        assert!(tree.is_published() && !Snapshot::default().is_published());
        assert_eq!(tree.children(None), [1, 4]);
        assert_eq!(tree.children(Some(1)), [2, 3]);
        assert!(tree.children(Some(2)).is_empty());
        assert_eq!(tree.index_in(Some(1), 3), Some(1));
        assert_eq!(tree.index_in(None, 3), None);
        assert_eq!(tree.focused(), Some(2));
        assert_eq!(tree.get(3).map(|n| n.parent), Some(Some(1)));
        assert!(tree.get(5).is_none() && !tree.contains(5) && tree.contains(4));
    }
}
