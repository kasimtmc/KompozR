//! What Android's content capture is told of the screen (Compose's AndroidContentCaptureManager): every node of the
//! unmerged semantics tree that is shown, as a virtual view structure (its class, its texts, its description, its
//! test tag, its bounds in its parent, its text's size), and as the tree changes, the nodes that appeared (each one
//! with its subtree: a structure each, in order), those that disappeared, and the texts that changed. Kotlin's
//! KompozContentCapture reads the events from one little-endian buffer ([`encode_events`]) and tells its
//! ContentCaptureSession. A password's node is not captured (Compose's), nor what is below it.
//!
//! Plain Rust, tested on every host; `semantics` reads the nodes from the host.

use super::accessibility::Writer;
use std::collections::{HashMap, HashSet};

/// A shown node of the unmerged tree as content capture sees it.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct CaptureNode {
    /// Its semantics id (its virtual id for content capture and translation).
    pub id: u64,
    /// The node above it (none: the root, whose parent is the view).
    pub parent: Option<u64>,
    /// Its place among its parent's shown children.
    pub index: i32,
    /// Its bounds in its parent's: left, top, width, height (px).
    pub bounds: [f32; 4],
    /// Its class (a text's, a field's, a role's, a traversal group's).
    pub class_name: Option<&'static str>,
    /// Its texts (one a line), a field's text, its content descriptions (one a line).
    pub text: Option<String>,
    pub editable_text: Option<String>,
    pub description: Option<String>,
    pub test_tag: Option<String>,
    /// It holds a password (it isn't captured).
    pub password: bool,
    /// Its text's size, px.
    pub text_size: Option<f32>,
}

/// The shown nodes as last read: parents before their children.
#[derive(Default, Debug)]
pub struct CaptureTree {
    nodes: Vec<CaptureNode>,
    at: HashMap<u64, usize>,
}

impl CaptureTree {
    pub fn new(nodes: Vec<CaptureNode>) -> CaptureTree {
        let at = nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();
        CaptureTree { nodes, at }
    }

    pub fn nodes(&self) -> &[CaptureNode] {
        &self.nodes
    }

    pub fn get(&self, id: u64) -> Option<&CaptureNode> {
        self.at.get(&id).map(|&i| &self.nodes[i])
    }

    pub fn contains(&self, id: u64) -> bool {
        self.at.contains_key(&id)
    }

    /// The node's first text (what a text change tells), if it has texts.
    fn first_text(&self, id: u64) -> Option<&str> {
        self.get(id).and_then(|n| n.text.as_deref()).map(|t| t.split('\n').next().unwrap_or(t))
    }
}

/// What a content capture session is told.
#[derive(Clone, PartialEq, Debug)]
pub enum CaptureEvent {
    /// A node came: its structure (`notifyViewAppeared`).
    Appeared(CaptureNode),
    /// A node went (`notifyViewDisappeared`).
    Disappeared(u64),
    /// A node's text is another (`notifyViewTextChanged`).
    TextChanged(u64, String),
}

/// What changed from `old` to `new` (Compose's `checkForSemanticsChanges` for content capture): the nodes gone,
/// then those come (a whole subtree for each that came under a node that was there; a password's left out with
/// what is below it), then each node's first text that changed (a node come with a text: it too).
pub fn capture_events(old: &CaptureTree, new: &CaptureTree) -> Vec<CaptureEvent> {
    let mut out = Vec::new();
    for n in &old.nodes {
        if !new.contains(n.id) {
            out.push(CaptureEvent::Disappeared(n.id));
        }
    }
    let mut hidden: HashSet<u64> = HashSet::new();
    for n in &new.nodes {
        if n.password || n.parent.is_some_and(|p| hidden.contains(&p)) {
            hidden.insert(n.id);
            continue;
        }
        if !old.contains(n.id) {
            out.push(CaptureEvent::Appeared(n.clone()));
        }
    }
    for n in &new.nodes {
        if hidden.contains(&n.id) {
            continue;
        }
        let now = new.first_text(n.id);
        let changed = match old.get(n.id) {
            None => now.is_some(),
            Some(_) => now != old.first_text(n.id),
        };
        if changed {
            out.push(CaptureEvent::TextChanged(n.id, now.unwrap_or_default().to_string()));
        }
    }
    out
}

/// Every node of `tree` appearing (a session started), or disappearing (it stopped): `appear`.
pub fn all(tree: &CaptureTree, appear: bool) -> Vec<CaptureEvent> {
    if !appear {
        return tree.nodes.iter().map(|n| CaptureEvent::Disappeared(n.id)).collect();
    }
    capture_events(&CaptureTree::default(), tree)
        .into_iter()
        .filter(|e| matches!(e, CaptureEvent::Appeared(_)))
        .collect()
}

/// KompozContentCapture's event kinds (its `CAPTURE_*`).
pub mod kind {
    pub const APPEARED: i32 = 0;
    pub const DISAPPEARED: i32 = 1;
    pub const TEXT_CHANGED: i32 = 2;
}

/// The events as KompozContentCapture reads them: the count, then each one's kind and node (its parent, -1 for
/// none; its index; its bounds; its class, texts, description, test tag; its text size, -1 for none), or the node
/// gone, or the node and its text.
pub fn encode_events(events: &[CaptureEvent]) -> Vec<u8> {
    let mut w = Writer::default();
    w.int(events.len() as i32);
    for e in events {
        match e {
            CaptureEvent::Appeared(n) => {
                w.int(kind::APPEARED);
                w.long(n.id as i64);
                w.long(n.parent.map_or(-1, |p| p as i64));
                w.int(n.index);
                for v in n.bounds {
                    w.float(v);
                }
                w.str(n.class_name);
                for s in [&n.text, &n.editable_text, &n.description, &n.test_tag] {
                    w.str(s.as_deref());
                }
                w.float(n.text_size.unwrap_or(-1.0));
            }
            CaptureEvent::Disappeared(id) => {
                w.int(kind::DISAPPEARED);
                w.long(*id as i64);
            }
            CaptureEvent::TextChanged(id, text) => {
                w.int(kind::TEXT_CHANGED);
                w.long(*id as i64);
                w.str(Some(text));
            }
        }
    }
    w.0
}

/// How the app's texts show for the system's translation (Compose's TranslateStatus): their own, or their
/// translations.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Translation {
    #[default]
    Original,
    Translated,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: u64, parent: Option<u64>, text: Option<&str>) -> CaptureNode {
        CaptureNode { id, parent, text: text.map(str::to_string), ..Default::default() }
    }

    #[test]
    fn nodes_appear_disappear_and_change_text() {
        let old =
            CaptureTree::new(vec![node(1, None, None), node(2, Some(1), Some("Merhaba")), node(3, Some(1), None)]);
        let mut secret = node(5, Some(1), None);
        secret.password = true;
        let new = CaptureTree::new(vec![
            node(1, None, None),
            node(2, Some(1), Some("Selam\nikinci")),
            node(4, Some(1), Some("Yeni")),
            secret,
            node(6, Some(5), Some("gizli")),
        ]);
        assert_eq!(
            capture_events(&old, &new),
            [
                CaptureEvent::Disappeared(3),
                CaptureEvent::Appeared(node(4, Some(1), Some("Yeni"))),
                CaptureEvent::TextChanged(2, "Selam".into()),
                CaptureEvent::TextChanged(4, "Yeni".into()),
            ]
        );
        assert_eq!(all(&old, true).len(), 3);
        assert_eq!(all(&old, false), [1, 2, 3].map(CaptureEvent::Disappeared));
        let bytes = encode_events(&capture_events(&old, &new));
        assert_eq!(i32::from_le_bytes(bytes[..4].try_into().unwrap()), 4);
    }
}
