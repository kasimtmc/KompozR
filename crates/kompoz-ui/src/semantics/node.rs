//! The semantics tree (Compose's SemanticsNode, SemanticsOwner and LayoutNode's collapsed semantics): a layout
//! node's configuration from its chain, the nodes of the merged and unmerged trees, the roots of a host's windows,
//! and the change notifications an accessibility bridge listens to.

use super::configuration::SemanticsConfiguration;
use super::properties::SemanticsProperties;
use crate::layout::{AlignmentLine, LayoutCoordinates};
use crate::modifier::NodeKind;
use crate::node::{LayoutNode, NodeCoordinator, Owner};
use kompoz_graphics::{IntSize, Offset, Rect};
use std::fmt;
use std::rc::{Rc, Weak};

impl LayoutNode {
    /// Its semantics (Compose's `collapsedSemantics`): its chain's semantics nodes applied tail to head, what
    /// they read observed (a change works it out again). None if it has no semantics node.
    pub fn semantics_configuration(&self) -> Option<SemanticsConfiguration> {
        if !self.has_kind(NodeKind::Semantics) {
            return None;
        }
        if !self.semantics_invalidated.get()
            && let Some(c) = self.semantics_configuration.borrow().as_ref()
        {
            return Some(c.clone());
        }
        let config = self.calculate_semantics_configuration();
        *self.semantics_configuration.borrow_mut() = Some(config.clone());
        self.semantics_invalidated.set(false);
        Some(config)
    }

    fn calculate_semantics_configuration(&self) -> SemanticsConfiguration {
        let mut config = SemanticsConfiguration::new();
        let mut apply = || {
            for node in self.chain_nodes().iter().rev() {
                if !node.is_kind(NodeKind::Semantics) {
                    continue;
                }
                crate::modifier::dispatch_for_kind(node, NodeKind::Semantics, &mut |cell| {
                    let mut n = cell.node.borrow_mut();
                    if let Some(s) = n.as_semantics() {
                        if s.should_clear_descendant_semantics() {
                            config = SemanticsConfiguration::new();
                            config.is_clearing_semantics = true;
                        }
                        if s.should_merge_descendant_semantics() {
                            config.is_merging_semantics_of_descendants = true;
                        }
                        s.apply_semantics(&mut config);
                    }
                });
            }
        };
        match self.owner() {
            Some(owner) => {
                let scope = self.reader_scope(&self.semantics_reads, |n| n.invalidate_semantics());
                owner.snapshot_observer().observe(&scope, apply);
            }
            None => apply(),
        }
        config
    }

    /// The coordinator its semantics bounds are of: that of its first semantics node important for bounds (the
    /// merging one, if it merges), else its inner one (Compose's `findCoordinatorToGetBounds`).
    fn semantics_coordinator(&self, merging: bool) -> Rc<NodeCoordinator> {
        let mut found: Option<Rc<NodeCoordinator>> = None;
        let mut stop = false;
        self.head_to_tail(NodeKind::Semantics, &mut |cell| {
            if stop {
                return;
            }
            let mut n = cell.node.borrow_mut();
            let Some(s) = n.as_semantics() else { return };
            if s.is_important_for_bounds() {
                if merging && s.should_merge_descendant_semantics() {
                    found = cell.coordinator();
                    stop = true;
                } else if found.is_none() {
                    found = cell.coordinator();
                    if !merging {
                        stop = true;
                    }
                }
            }
        });
        found.unwrap_or_else(|| self.inner.clone())
    }
}

/// The ids of the nodes a merging node with a role or a content description adds for accessibility (Compose's
/// fake nodes): its own id moved by these.
const ROLE_FAKE_NODE_ID_OFFSET: u64 = 1_000_000_000_000;
const CONTENT_DESCRIPTION_FAKE_NODE_ID_OFFSET: u64 = 2_000_000_000_000;

/// A node of the semantics tree (Compose's SemanticsNode): a layout node with semantics (a window's root always),
/// its configuration — its own, or with its descendants' merged in when merging is on and it merges them — and its
/// bounds. Made when asked for: it reads the tree as it is then.
///
/// A popup's or dialog's root is a root of its own (as Compose's separate windows), its coordinates the host
/// window's.
#[derive(Clone)]
pub struct SemanticsNode {
    layout_node: Rc<LayoutNode>,
    merging_enabled: bool,
    unmerged_config: SemanticsConfiguration,
    /// A fake node's (one accessibility sees for a merging node's role or description): the node it is of.
    fake_parent: Option<Rc<SemanticsNode>>,
    fake_id: u64,
}

impl fmt::Debug for SemanticsNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SemanticsNode(id={}, {:?}, bounds={:?})", self.id(), self.config(), self.bounds_in_root())
    }
}

impl PartialEq for SemanticsNode {
    /// The same node of the same tree.
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id() && self.merging_enabled == other.merging_enabled
    }
}

impl SemanticsNode {
    pub(crate) fn of(layout_node: Rc<LayoutNode>, merging_enabled: bool) -> SemanticsNode {
        let unmerged_config = layout_node.semantics_configuration().unwrap_or_default();
        SemanticsNode { layout_node, merging_enabled, unmerged_config, fake_parent: None, fake_id: 0 }
    }

    /// Its id: its layout node's, the same as long as that is (a fake node's, its node's moved).
    pub fn id(&self) -> u64 {
        if self.fake_parent.is_some() { self.fake_id } else { self.layout_node.id() }
    }

    /// The layout node it is (a fake node's: the node it is of's).
    pub fn layout_node(&self) -> &Rc<LayoutNode> {
        &self.layout_node
    }

    /// Whether it is a fake node (Compose's `isFake`): one accessibility sees for a merging node's role or content
    /// description.
    pub fn is_fake(&self) -> bool {
        self.fake_parent.is_some()
    }

    /// Whether it is a root (of the host's window or of a popup's): it has no parent.
    pub fn is_root(&self) -> bool {
        self.fake_parent.is_none() && self.layout_node.parent().is_none()
    }

    /// Whether this tree merges (it was asked for merged).
    pub fn merging_enabled(&self) -> bool {
        self.merging_enabled
    }

    /// Its own properties, nothing merged in.
    pub fn unmerged_config(&self) -> &SemanticsConfiguration {
        &self.unmerged_config
    }

    /// Whether it merges its descendants in this tree.
    pub fn is_merging_semantics_of_descendants(&self) -> bool {
        self.merging_enabled && self.unmerged_config.is_merging_semantics_of_descendants
    }

    /// Its properties: its own, with those of its descendants that don't merge their own merged in where it
    /// merges (Compose's `config`).
    pub fn config(&self) -> SemanticsConfiguration {
        if self.is_merging_semantics_of_descendants() {
            let mut merged = self.unmerged_config.clone();
            self.merge_config(&mut merged);
            merged
        } else {
            self.unmerged_config.clone()
        }
    }

    fn merge_config(&self, merged: &mut SemanticsConfiguration) {
        if self.unmerged_config.is_clearing_semantics {
            return;
        }
        for child in self.unmerged_children(false) {
            if !child.is_merging_semantics_of_descendants() {
                merged.merge_child(&child.unmerged_config);
                child.merge_config(merged);
            }
        }
    }

    /// The nearest semantics nodes below its layout node, in drawing order, and its fake nodes if asked (Compose's
    /// `unmergedChildren`).
    fn unmerged_children(&self, include_fake_nodes: bool) -> Vec<SemanticsNode> {
        if self.is_fake() {
            return Vec::new();
        }
        let mut out = Vec::new();
        fill_one_layer(&self.layout_node, self.merging_enabled, &mut out);
        if include_fake_nodes {
            self.emit_fake_nodes(&mut out);
        }
        out
    }

    /// Its children in this tree (Compose's `children`): in the merged tree none past a clearing node; a merging
    /// node's only those below that merge their own.
    pub fn children(&self) -> Vec<SemanticsNode> {
        self.get_children(!self.merging_enabled, false)
    }

    /// Its children as accessibility sees them (Compose's `replacedChildren`): none past a clearing node, and the
    /// fake nodes of a merging node's role and description.
    pub fn replaced_children(&self) -> Vec<SemanticsNode> {
        self.get_children(false, true)
    }

    fn get_children(&self, include_replaced_semantics: bool, include_fake_nodes: bool) -> Vec<SemanticsNode> {
        if !include_replaced_semantics && self.unmerged_config.is_clearing_semantics {
            return Vec::new();
        }
        if self.is_merging_semantics_of_descendants() {
            let mut out = Vec::new();
            self.find_one_layer_of_merging_nodes(&mut out);
            return out;
        }
        self.unmerged_children(include_fake_nodes)
    }

    fn find_one_layer_of_merging_nodes(&self, out: &mut Vec<SemanticsNode>) {
        for child in self.unmerged_children(false) {
            if child.is_merging_semantics_of_descendants() {
                out.push(child);
            } else if !child.unmerged_config.is_clearing_semantics {
                child.find_one_layer_of_merging_nodes(out);
            }
        }
    }

    /// A merging node's fake nodes (Compose's `emitFakeNodes`): one for its role after its children, one for its
    /// description before them.
    fn emit_fake_nodes(&self, children: &mut Vec<SemanticsNode>) {
        if !self.unmerged_config.is_merging_semantics_of_descendants || children.is_empty() {
            return;
        }
        if let Some(role) = self.unmerged_config.get(SemanticsProperties::Role) {
            let mut config = SemanticsConfiguration::new();
            config.set_role(role);
            children.push(self.fake_node(config, ROLE_FAKE_NODE_ID_OFFSET));
        }
        if let Some(description) =
            self.unmerged_config.get(SemanticsProperties::ContentDescription).and_then(|d| d.into_iter().next())
        {
            let mut config = SemanticsConfiguration::new();
            config.set_content_description(description);
            children.insert(0, self.fake_node(config, CONTENT_DESCRIPTION_FAKE_NODE_ID_OFFSET));
        }
    }

    fn fake_node(&self, config: SemanticsConfiguration, offset: u64) -> SemanticsNode {
        SemanticsNode {
            layout_node: self.layout_node.clone(),
            merging_enabled: false,
            unmerged_config: config,
            fake_parent: Some(Rc::new(self.clone())),
            fake_id: self.layout_node.id() + offset,
        }
    }

    /// Its parent in this tree: the nearest merging ancestor when merging, else the nearest with semantics or the
    /// root of its window (Compose's `parent`).
    pub fn parent(&self) -> Option<SemanticsNode> {
        if let Some(p) = &self.fake_parent {
            return Some((**p).clone());
        }
        let mut found = None;
        if self.merging_enabled {
            found = closest_parent(&self.layout_node, |n| {
                n.semantics_configuration().is_some_and(|c| c.is_merging_semantics_of_descendants)
            });
        }
        if found.is_none() {
            found = closest_parent(&self.layout_node, |n| n.has_kind(NodeKind::Semantics) || n.parent().is_none());
        }
        found.map(|n| SemanticsNode::of(n, self.merging_enabled))
    }

    /// The same node in the merged tree (Compose's `copyWithMergingEnabled`).
    pub fn with_merging_enabled(&self) -> SemanticsNode {
        SemanticsNode { merging_enabled: true, ..self.clone() }
    }

    fn bounds_coordinator(&self) -> Rc<NodeCoordinator> {
        if let Some(p) = &self.fake_parent {
            return p.bounds_coordinator();
        }
        self.layout_node.semantics_coordinator(self.unmerged_config.is_merging_semantics_of_descendants)
    }

    /// The coordinates its bounds are of.
    pub fn coordinates(&self) -> LayoutCoordinates {
        LayoutCoordinates::of(self.bounds_coordinator())
    }

    pub fn size(&self) -> IntSize {
        self.bounds_coordinator().measured_size.get()
    }

    /// Its bounds in the root (the host's window), cut by the clipping layers above it (Compose's
    /// `boundsInRoot`).
    pub fn bounds_in_root(&self) -> Rect {
        let c = self.bounds_coordinator();
        if !c.is_attached() {
            return Rect::Zero;
        }
        LayoutCoordinates::of(c).bounds_in_root()
    }

    /// Its bounds in the root, not cut (Compose's `unclippedBoundsInRoot`).
    pub fn unclipped_bounds_in_root(&self) -> Rect {
        let c = self.bounds_coordinator();
        if !c.is_attached() {
            return Rect::Zero;
        }
        let coordinates = LayoutCoordinates::of(c);
        let root = coordinates.find_root_coordinates();
        root.local_bounding_box_of(&coordinates, false).translate(root.position_in_root())
    }

    /// The bounds a touch hits it in, in the root (Compose's `touchBoundsInRoot`): its bounds, cut.
    pub fn touch_bounds_in_root(&self) -> Rect {
        self.bounds_in_root()
    }

    pub fn position_in_root(&self) -> Offset {
        let c = self.bounds_coordinator();
        if !c.is_attached() {
            return Offset::Zero;
        }
        LayoutCoordinates::of(c).position_in_root()
    }

    pub fn bounds_in_window(&self) -> Rect {
        let c = self.bounds_coordinator();
        if !c.is_attached() {
            return Rect::Zero;
        }
        LayoutCoordinates::of(c).bounds_in_window()
    }

    pub fn position_in_window(&self) -> Offset {
        let c = self.bounds_coordinator();
        if !c.is_attached() {
            return Offset::Zero;
        }
        LayoutCoordinates::of(c).position_in_window()
    }

    /// Its bounds in its parent's (Compose's `boundsInParent`): in the coordinates of the parent's first node
    /// important for bounds.
    pub fn bounds_in_parent(&self) -> Rect {
        let c = self.bounds_coordinator();
        if !c.is_attached() {
            return Rect::Zero;
        }
        let Some(parent) = self.parent() else { return Rect::Zero };
        let parent_coordinates = parent.coordinates();
        if !parent_coordinates.is_attached() {
            return Rect::Zero;
        }
        parent_coordinates.local_bounding_box_of(&LayoutCoordinates::of(c), true)
    }

    /// Whether a layer above it is fully transparent (Compose's `isTransparent`).
    pub fn is_transparent(&self) -> bool {
        let mut c = Some(self.bounds_coordinator());
        while let Some(x) = c {
            if x.layer.borrow().as_ref().is_some_and(|l| l.props.alpha <= 0.0) {
                return true;
            }
            c = x.wrapped_by();
        }
        false
    }

    /// Where `line` is in it, `AlignmentLine::Unspecified` if it has none (Compose's `getAlignmentLinePosition`).
    pub fn get_alignment_line_position(&self, line: impl Into<AlignmentLine>) -> i32 {
        self.bounds_coordinator().alignment_line(line.into())
    }

    /// Whether its layout is placed (shown).
    pub fn is_placed(&self) -> bool {
        self.layout_node.is_placed()
    }

    /// Whether accessibility leaves it out (Compose's `isHidden`): transparent, or hidden from accessibility.
    pub fn is_hidden(&self) -> bool {
        self.is_transparent()
            || self.unmerged_config.contains(SemanticsProperties::HideFromAccessibility)
            || self.unmerged_config.contains(SemanticsProperties::InvisibleToUser)
    }

    /// Whether it tells accessibility something (Compose's `isImportantForAccessibility`).
    pub fn is_important_for_accessibility(&self) -> bool {
        !self.is_hidden()
            && (self.unmerged_config.is_merging_semantics_of_descendants
                || self.unmerged_config.contains_important_for_accessibility())
    }

    /// Whether it scrolls (it has a scroll axis range).
    pub fn is_scroll_node(&self) -> bool {
        self.unmerged_config.contains(SemanticsProperties::VerticalScrollAxisRange)
            || self.unmerged_config.contains(SemanticsProperties::HorizontalScrollAxisRange)
    }

    /// It and every node below it in this tree, depth first.
    pub fn subtree(&self) -> Vec<SemanticsNode> {
        let mut out = vec![self.clone()];
        for c in self.children() {
            out.extend(c.subtree());
        }
        out
    }
}

fn fill_one_layer(node: &Rc<LayoutNode>, merging_enabled: bool, out: &mut Vec<SemanticsNode>) {
    for child in node.z_sorted_children() {
        if !child.is_attached() {
            continue;
        }
        if child.has_kind(NodeKind::Semantics) {
            out.push(SemanticsNode::of(child, merging_enabled));
        } else {
            fill_one_layer(&child, merging_enabled, out);
        }
    }
}

fn closest_parent(node: &Rc<LayoutNode>, predicate: impl Fn(&Rc<LayoutNode>) -> bool) -> Option<Rc<LayoutNode>> {
    let mut p = node.parent();
    while let Some(n) = p {
        if predicate(&n) {
            return Some(n);
        }
        p = n.parent();
    }
    None
}

/// Told when a layout's semantics changed (Compose's SemanticsListener): what an accessibility bridge listens to.
pub trait SemanticsListener {
    /// `node`'s semantics changed; `previous` is what they were (none if it had none).
    fn on_semantics_changed(&self, node: &Rc<LayoutNode>, previous: Option<&SemanticsConfiguration>);
}

/// The semantics of a host's tree (Compose's SemanticsOwner): the root of its window, merged or not, the roots of
/// its popups and dialogs, every node, and the listeners told of changes.
#[derive(Clone)]
pub struct SemanticsOwner {
    pub(crate) owner: Weak<Owner>,
}

impl SemanticsOwner {
    /// The root of the merged tree of the host's window (Compose's `rootSemanticsNode`).
    pub fn root_semantics_node(&self) -> Option<SemanticsNode> {
        self.owner.upgrade().and_then(|o| o.root()).map(|r| SemanticsNode::of(r, true))
    }

    /// The root of the unmerged tree (Compose's `unmergedRootSemanticsNode`).
    pub fn unmerged_root_semantics_node(&self) -> Option<SemanticsNode> {
        self.owner.upgrade().and_then(|o| o.root()).map(|r| SemanticsNode::of(r, false))
    }

    /// The roots of the windows: the host's, then its popups' and dialogs', bottom first (Compose's roots of a
    /// test's compose views).
    pub fn root_semantics_nodes(&self, merging_enabled: bool) -> Vec<SemanticsNode> {
        let Some(owner) = self.owner.upgrade() else { return Vec::new() };
        let mut out: Vec<SemanticsNode> =
            owner.root().into_iter().map(|r| SemanticsNode::of(r, merging_enabled)).collect();
        out.extend(owner.popup_roots().into_iter().map(|r| SemanticsNode::of(r, merging_enabled)));
        out
    }

    /// Every node of every window, merged or not, depth first from each root (Compose's `getAllSemanticsNodes`).
    pub fn all_semantics_nodes(&self, merging_enabled: bool) -> Vec<SemanticsNode> {
        self.root_semantics_nodes(merging_enabled).iter().flat_map(|r| r.subtree()).collect()
    }

    /// The node with `id`, in the merged tree or not (Compose's `get`), fake nodes included.
    pub fn find(&self, id: u64, merging_enabled: bool) -> Option<SemanticsNode> {
        for n in self.all_semantics_nodes(merging_enabled) {
            if n.id() == id {
                return Some(n);
            }
            if let Some(fake) = n.replaced_children().into_iter().find(|c| c.is_fake() && c.id() == id) {
                return Some(fake);
            }
        }
        None
    }

    /// Adds a listener told of each change of a layout's semantics, after the frame it happened in.
    pub fn add_listener(&self, listener: Rc<dyn SemanticsListener>) {
        if let Some(o) = self.owner.upgrade() {
            o.semantics_listeners.borrow_mut().push(listener);
        }
    }

    pub fn remove_listener(&self, listener: &Rc<dyn SemanticsListener>) {
        if let Some(o) = self.owner.upgrade() {
            o.semantics_listeners.borrow_mut().retain(|l| !Rc::ptr_eq(l, listener));
        }
    }
}

impl Owner {
    /// A layout's semantics were invalidated: noted (with what they were) for the listeners.
    pub(crate) fn note_semantics_invalidated(&self, node: &LayoutNode) {
        if self.semantics_listeners.borrow().is_empty() {
            return;
        }
        let mut pending = self.semantics_pending.borrow_mut();
        if pending.iter().any(|(n, _)| std::ptr::eq(n.as_ptr(), node)) {
            return;
        }
        let previous = node.semantics_configuration.borrow().clone();
        pending.push((node.this.clone(), previous));
    }

    /// Tells the listeners of the semantics that changed since the last time (Compose's
    /// `notifySemanticsChange`).
    pub(crate) fn dispatch_semantics_changes(&self) {
        let pending = std::mem::take(&mut *self.semantics_pending.borrow_mut());
        if pending.is_empty() {
            return;
        }
        let listeners = self.semantics_listeners.borrow().clone();
        for (node, previous) in pending {
            let Some(node) = node.upgrade() else { continue };
            if !node.is_attached() {
                continue;
            }
            for l in &listeners {
                l.on_semantics_changed(&node, previous.as_ref());
            }
        }
    }
}
