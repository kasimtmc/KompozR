//! Semantics, as Compose's `androidx.compose.ui.semantics`: what a layout means (its text, its description, its
//! test tag, its actions…), set by semantics modifier nodes into a [`SemanticsConfiguration`], gathered into a tree
//! of [`SemanticsNode`]s — unmerged, or merged where a node merges its descendants — that accessibility and the
//! test rule read through the host's [`SemanticsOwner`].
//!
//! The core is here: keys, the configuration, the node and its tree, `Modifier.semantics {}` and `test_tag`, and
//! the keys `Text`, `ContentDescription` and `TestTag`; the other properties and actions are the semantics package
//! of this stage.

use crate::layout::LayoutCoordinates;
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement, NodeKind};
use crate::node::{LayoutNode, NodeCoordinator};
use crate::node_capabilities;
use kompoz_graphics::{IntSize, Offset, Rect};
use kompoz_runtime::named;
use std::any::Any;
use std::fmt;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

/// A semantics property (Compose's SemanticsPropertyKey): its name (what it is known by) and how a merging
/// parent takes a child's value (`merge_policy(parent's, child's)`, none: the parent's left as it is).
pub struct SemanticsPropertyKey<T> {
    name: &'static str,
    merge_policy: fn(Option<T>, T) -> Option<T>,
    /// Whether it tells accessibility something (Compose's `isImportantForAccessibility`).
    important_for_accessibility: bool,
    _t: PhantomData<fn() -> T>,
}

impl<T> Clone for SemanticsPropertyKey<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for SemanticsPropertyKey<T> {}

impl<T: Clone + 'static> SemanticsPropertyKey<T> {
    /// A key whose merged value is the parent's if it has one, else the child's (Compose's default policy).
    pub const fn new(name: &'static str) -> SemanticsPropertyKey<T> {
        SemanticsPropertyKey {
            name,
            merge_policy: keep_parent::<T>,
            important_for_accessibility: true,
            _t: PhantomData,
        }
    }

    pub const fn with_merge_policy(name: &'static str, merge_policy: fn(Option<T>, T) -> Option<T>) -> Self {
        SemanticsPropertyKey { name, merge_policy, important_for_accessibility: true, _t: PhantomData }
    }

    /// A key that tells accessibility nothing (a test tag).
    pub const fn not_important_for_accessibility(self) -> Self {
        SemanticsPropertyKey { important_for_accessibility: false, ..self }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The value a merging parent ends up with.
    pub fn merge(&self, parent_value: Option<T>, child_value: T) -> Option<T> {
        (self.merge_policy)(parent_value, child_value)
    }
}

impl<T> fmt::Debug for SemanticsPropertyKey<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AccessibilityKey: {}", self.name)
    }
}

fn keep_parent<T>(parent: Option<T>, child: T) -> Option<T> {
    Some(parent.unwrap_or(child))
}

fn keep_parent_only<T>(parent: Option<T>, _child: T) -> Option<T> {
    parent
}

fn concatenate(parent: Option<Vec<String>>, child: Vec<String>) -> Option<Vec<String>> {
    Some(match parent {
        Some(mut p) => {
            p.extend(child);
            p
        }
        None => child,
    })
}

/// The core keys (Compose's SemanticsProperties; the rest come with the semantics package).
pub struct SemanticsProperties;

impl SemanticsProperties {
    /// What the layout says, a merging parent's the texts of its children after its own. (Compose's are
    /// AnnotatedStrings; plain strings until the text stage.)
    pub const Text: SemanticsPropertyKey<Vec<String>> = SemanticsPropertyKey::with_merge_policy("Text", concatenate);
    /// What describes it to accessibility (an icon's label), merged as texts are.
    pub const ContentDescription: SemanticsPropertyKey<Vec<String>> =
        SemanticsPropertyKey::with_merge_policy("ContentDescription", concatenate);
    /// What tests find it by; a merging parent keeps its own (Compose's TestTag).
    pub const TestTag: SemanticsPropertyKey<String> =
        SemanticsPropertyKey::with_merge_policy("TestTag", keep_parent_only).not_important_for_accessibility();
}

/// A property set: the key's name, its value, how a merge takes it.
#[derive(Clone)]
struct Entry {
    name: &'static str,
    value: Rc<dyn Any>,
    merge: fn(&'static str, Option<Rc<dyn Any>>, Rc<dyn Any>, &dyn Any) -> Option<Rc<dyn Any>>,
    policy: Rc<dyn Any>,
    important: bool,
}

/// The semantics properties of a layout, and whether it merges its descendants' or clears them (Compose's
/// SemanticsConfiguration). Apply-semantics blocks set into it (Compose's SemanticsPropertyReceiver).
#[derive(Clone, Default)]
pub struct SemanticsConfiguration {
    entries: Vec<Entry>,
    /// Its descendants' semantics are its own (a button's label).
    pub is_merging_semantics_of_descendants: bool,
    /// Its descendants' semantics are dropped.
    pub is_clearing_semantics: bool,
}

fn merge_erased<T: Clone + 'static>(
    _name: &'static str,
    parent: Option<Rc<dyn Any>>,
    child: Rc<dyn Any>,
    policy: &dyn Any,
) -> Option<Rc<dyn Any>> {
    let policy = policy.downcast_ref::<fn(Option<T>, T) -> Option<T>>()?;
    let parent = parent.and_then(|p| p.downcast_ref::<T>().cloned());
    let child = child.downcast_ref::<T>()?.clone();
    policy(parent, child).map(|v| Rc::new(v) as Rc<dyn Any>)
}

impl SemanticsConfiguration {
    pub fn new() -> SemanticsConfiguration {
        SemanticsConfiguration::default()
    }

    /// Sets `key` to `value`.
    pub fn set<T: Clone + 'static>(&mut self, key: SemanticsPropertyKey<T>, value: T) {
        let entry = Entry {
            name: key.name,
            value: Rc::new(value),
            merge: merge_erased::<T>,
            policy: Rc::new(key.merge_policy),
            important: key.important_for_accessibility,
        };
        match self.entries.iter_mut().find(|e| e.name == key.name) {
            Some(e) => *e = entry,
            None => self.entries.push(entry),
        }
    }

    /// `key`'s value, if it is set.
    pub fn get<T: Clone + 'static>(&self, key: SemanticsPropertyKey<T>) -> Option<T> {
        self.entries.iter().find(|e| e.name == key.name).and_then(|e| e.value.downcast_ref::<T>().cloned())
    }

    pub fn get_or_else<T: Clone + 'static>(&self, key: SemanticsPropertyKey<T>, default: impl FnOnce() -> T) -> T {
        self.get(key).unwrap_or_else(default)
    }

    pub fn contains<T>(&self, key: SemanticsPropertyKey<T>) -> bool {
        self.entries.iter().any(|e| e.name == key.name)
    }

    /// The names of the properties set, in the order they were first set.
    pub fn names(&self) -> Vec<&'static str> {
        self.entries.iter().map(|e| e.name).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether a property tells accessibility something (Compose's `containsImportantForAccessibility`).
    pub fn contains_important_for_accessibility(&self) -> bool {
        self.entries.iter().any(|e| e.important)
    }

    /// A child's properties taken in by a merging parent (Compose's `mergeChild`).
    pub fn merge_child(&mut self, child: &SemanticsConfiguration) {
        for c in &child.entries {
            let existing = self.entries.iter().position(|e| e.name == c.name);
            let parent_value = existing.map(|i| self.entries[i].value.clone());
            if let Some(merged) = (c.merge)(c.name, parent_value, c.value.clone(), &*c.policy) {
                match existing {
                    Some(i) => self.entries[i].value = merged,
                    None => self.entries.push(Entry { value: merged, ..c.clone() }),
                }
            }
        }
    }

    /// Another node's properties taken in as if set here (Compose's `collapsePeer`): those not set here yet.
    pub fn collapse_peer(&mut self, peer: &SemanticsConfiguration) {
        if peer.is_merging_semantics_of_descendants {
            self.is_merging_semantics_of_descendants = true;
        }
        if peer.is_clearing_semantics {
            self.is_clearing_semantics = true;
        }
        for e in &peer.entries {
            if !self.entries.iter().any(|x| x.name == e.name) {
                self.entries.push(e.clone());
            }
        }
    }

    // Compose's SemanticsPropertyReceiver setters of the core keys.

    /// `text = …`: what it says.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.set(SemanticsProperties::Text, vec![text.into()]);
    }
    /// `contentDescription = …`.
    pub fn set_content_description(&mut self, description: impl Into<String>) {
        self.set(SemanticsProperties::ContentDescription, vec![description.into()]);
    }
    /// `testTag = …`.
    pub fn set_test_tag(&mut self, tag: impl Into<String>) {
        self.set(SemanticsProperties::TestTag, tag.into());
    }
}

impl fmt::Debug for SemanticsConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut first = true;
        if self.is_merging_semantics_of_descendants {
            write!(f, "mergeDescendants=true")?;
            first = false;
        }
        if self.is_clearing_semantics {
            write!(f, "{}isClearingSemantics=true", if first { "" } else { ", " })?;
            first = false;
        }
        for e in &self.entries {
            write!(f, "{}{}", if first { "" } else { ", " }, e.name)?;
            if let Some(v) = e.value.downcast_ref::<Vec<String>>() {
                write!(f, " : {v:?}")?;
            } else if let Some(v) = e.value.downcast_ref::<String>() {
                write!(f, " : {v:?}")?;
            }
            first = false;
        }
        Ok(())
    }
}

/// A node that sets semantics properties (Compose's SemanticsModifierNode).
pub trait SemanticsModifierNode {
    /// Whether the semantics of what is below it are dropped (Compose's `shouldClearDescendantSemantics`).
    fn should_clear_descendant_semantics(&self) -> bool {
        false
    }
    /// Whether what is below it merges into it (Compose's `shouldMergeDescendantSemantics`).
    fn should_merge_descendant_semantics(&self) -> bool {
        false
    }
    /// Whether its bounds are its semantics node's (Compose's `isImportantForBounds`).
    fn is_important_for_bounds(&self) -> bool {
        true
    }
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration);
}

impl Modifier {
    /// A test tag, for tests to find the layout by (Compose's `Modifier.testTag`).
    pub fn test_tag(self, tag: impl Into<String>) -> Modifier {
        self.then_element(TestTagElement { tag: tag.into() })
    }
}

#[named]
impl Modifier {
    /// Semantics properties set by `properties`, and with `merge_descendants` those of what is below merged in
    /// (Compose's `Modifier.semantics`).
    pub fn semantics(
        self,
        #[default(false)] merge_descendants: bool,
        properties: impl Fn(&mut SemanticsConfiguration) + 'static,
    ) -> Modifier {
        self.then_element(SemanticsElement { merge_descendants, clear: false, properties: Rc::new(properties) })
    }

    /// Semantics properties set by `properties` instead of those of what is below (Compose's
    /// `Modifier.clearAndSetSemantics`).
    pub fn clear_and_set_semantics(self, properties: impl Fn(&mut SemanticsConfiguration) + 'static) -> Modifier {
        self.then_element(SemanticsElement { merge_descendants: false, clear: true, properties: Rc::new(properties) })
    }
}

struct SemanticsElement {
    merge_descendants: bool,
    clear: bool,
    properties: Rc<dyn Fn(&mut SemanticsConfiguration)>,
}

impl PartialEq for SemanticsElement {
    fn eq(&self, other: &Self) -> bool {
        self.merge_descendants == other.merge_descendants
            && self.clear == other.clear
            && Rc::ptr_eq(&self.properties, &other.properties)
    }
}

impl ModifierNodeElement for SemanticsElement {
    type Node = CoreSemanticsModifierNode;
    fn create(&self) -> CoreSemanticsModifierNode {
        CoreSemanticsModifierNode {
            merge_descendants: self.merge_descendants,
            clear: self.clear,
            properties: self.properties.clone(),
        }
    }
    fn update(&self, node: &mut CoreSemanticsModifierNode) {
        node.merge_descendants = self.merge_descendants;
        node.clear = self.clear;
        node.properties = self.properties.clone();
    }
    fn name(&self) -> &'static str {
        if self.clear { "clearAndSetSemantics" } else { "semantics" }
    }
}

/// The node of `Modifier.semantics` (Compose's CoreSemanticsModifierNode).
pub struct CoreSemanticsModifierNode {
    merge_descendants: bool,
    clear: bool,
    properties: Rc<dyn Fn(&mut SemanticsConfiguration)>,
}

impl ModifierNode for CoreSemanticsModifierNode {
    node_capabilities!(semantics);
}

impl SemanticsModifierNode for CoreSemanticsModifierNode {
    fn should_clear_descendant_semantics(&self) -> bool {
        self.clear
    }
    fn should_merge_descendant_semantics(&self) -> bool {
        self.merge_descendants
    }
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        (self.properties)(config)
    }
}

#[derive(PartialEq)]
struct TestTagElement {
    tag: String,
}

impl ModifierNodeElement for TestTagElement {
    type Node = TestTagNode;
    fn create(&self) -> TestTagNode {
        TestTagNode { tag: self.tag.clone() }
    }
    fn update(&self, node: &mut TestTagNode) {
        node.tag = self.tag.clone();
    }
    fn name(&self) -> &'static str {
        "testTag"
    }
}

/// The node of `Modifier.test_tag` (Compose's TestTagNode).
pub struct TestTagNode {
    tag: String,
}

impl ModifierNode for TestTagNode {
    node_capabilities!(semantics);
}

impl SemanticsModifierNode for TestTagNode {
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        config.set_test_tag(self.tag.clone());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The tree

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

/// A node of the semantics tree (Compose's SemanticsNode): a layout node with semantics (the root always), its
/// configuration — its own, or with its descendants' merged in when merging is on and it merges them — and its
/// bounds. Made when asked for: it reads the tree as it is then.
#[derive(Clone)]
pub struct SemanticsNode {
    layout_node: Rc<LayoutNode>,
    merging_enabled: bool,
    unmerged_config: SemanticsConfiguration,
    is_root: bool,
}

impl fmt::Debug for SemanticsNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SemanticsNode(id={}, {:?}, bounds={:?})", self.id(), self.config(), self.bounds_in_root())
    }
}

impl SemanticsNode {
    fn of(layout_node: Rc<LayoutNode>, merging_enabled: bool, is_root: bool) -> SemanticsNode {
        let unmerged_config = layout_node.semantics_configuration().unwrap_or_default();
        SemanticsNode { layout_node, merging_enabled, unmerged_config, is_root }
    }

    /// Its id: its layout node's, the same as long as that is.
    pub fn id(&self) -> u64 {
        self.layout_node.id()
    }

    pub fn layout_node(&self) -> &Rc<LayoutNode> {
        &self.layout_node
    }

    pub fn is_root(&self) -> bool {
        self.is_root
    }

    /// Whether this tree merges (it was asked for merged).
    pub fn merging_enabled(&self) -> bool {
        self.merging_enabled
    }

    /// Its own properties, nothing merged in.
    pub fn unmerged_config(&self) -> &SemanticsConfiguration {
        &self.unmerged_config
    }

    fn is_merging_semantics_of_descendants(&self) -> bool {
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
        for child in self.unmerged_children() {
            if !child.is_merging_semantics_of_descendants() {
                merged.merge_child(&child.unmerged_config);
                child.merge_config(merged);
            }
        }
    }

    /// The nearest semantics nodes below its layout node, in drawing order (Compose's `unmergedChildren`).
    fn unmerged_children(&self) -> Vec<SemanticsNode> {
        let mut out = Vec::new();
        fill_one_layer(&self.layout_node, self.merging_enabled, &mut out);
        out
    }

    /// Its children in this tree (Compose's `children`): none past a clearing node; a merging node's only those
    /// below that merge their own.
    pub fn children(&self) -> Vec<SemanticsNode> {
        if self.merging_enabled && self.unmerged_config.is_clearing_semantics {
            return Vec::new();
        }
        if self.is_merging_semantics_of_descendants() {
            let mut out = Vec::new();
            self.find_one_layer_of_merging_nodes(&mut out);
            return out;
        }
        self.unmerged_children()
    }

    fn find_one_layer_of_merging_nodes(&self, out: &mut Vec<SemanticsNode>) {
        for child in self.unmerged_children() {
            if child.is_merging_semantics_of_descendants() {
                out.push(child);
            } else if !child.unmerged_config.is_clearing_semantics {
                child.find_one_layer_of_merging_nodes(out);
            }
        }
    }

    /// Its parent in this tree: the nearest merging ancestor when merging, else the nearest with semantics
    /// (Compose's `parent`).
    pub fn parent(&self) -> Option<SemanticsNode> {
        let root = root_of(&self.layout_node);
        let mut found = None;
        if self.merging_enabled {
            found = closest_parent(&self.layout_node, |n| {
                n.semantics_configuration().is_some_and(|c| c.is_merging_semantics_of_descendants)
            });
        }
        if found.is_none() {
            found = closest_parent(&self.layout_node, |n| n.has_kind(NodeKind::Semantics) || Rc::ptr_eq(n, &root));
        }
        found.map(|n| {
            let is_root = Rc::ptr_eq(&n, &root);
            SemanticsNode::of(n, self.merging_enabled, is_root)
        })
    }

    fn bounds_coordinator(&self) -> Rc<NodeCoordinator> {
        self.layout_node.semantics_coordinator(self.unmerged_config.is_merging_semantics_of_descendants)
    }

    /// The coordinates its bounds are of.
    pub fn coordinates(&self) -> LayoutCoordinates {
        LayoutCoordinates::of(self.bounds_coordinator())
    }

    pub fn size(&self) -> IntSize {
        self.bounds_coordinator().measured_size.get()
    }

    /// Its bounds in the root, cut by the clipping layers above it (Compose's `boundsInRoot`).
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
        coordinates.find_root_coordinates().local_bounding_box_of(&coordinates, false)
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

    /// Whether its layout is placed (shown).
    pub fn is_placed(&self) -> bool {
        self.layout_node.is_placed()
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
            out.push(SemanticsNode::of(child, merging_enabled, false));
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

fn root_of(node: &Rc<LayoutNode>) -> Rc<LayoutNode> {
    let mut n = node.clone();
    while let Some(p) = n.parent() {
        n = p;
    }
    n
}

/// The semantics of a host's tree (Compose's SemanticsOwner): its root node, merged or not, and every node.
#[derive(Clone)]
pub struct SemanticsOwner {
    pub(crate) root: Weak<LayoutNode>,
}

impl SemanticsOwner {
    /// The root of the merged tree (Compose's `rootSemanticsNode`).
    pub fn root_semantics_node(&self) -> Option<SemanticsNode> {
        self.root.upgrade().map(|r| SemanticsNode::of(r, true, true))
    }

    /// The root of the unmerged tree (Compose's `unmergedRootSemanticsNode`).
    pub fn unmerged_root_semantics_node(&self) -> Option<SemanticsNode> {
        self.root.upgrade().map(|r| SemanticsNode::of(r, false, true))
    }

    /// Every node of the tree, merged or not, depth first from the root (Compose's `getAllSemanticsNodes`).
    pub fn all_semantics_nodes(&self, merging_enabled: bool) -> Vec<SemanticsNode> {
        let root = if merging_enabled { self.root_semantics_node() } else { self.unmerged_root_semantics_node() };
        root.map_or_else(Vec::new, |r| r.subtree())
    }
}
