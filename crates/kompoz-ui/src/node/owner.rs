//! The owner of a layout tree (Compose's Owner, as AndroidComposeView is one) and its measure-and-layout delegate
//! (Compose's MeasureAndLayoutDelegate): the nodes to measure or place again, depth sorted, the passes over them,
//! the global position callbacks after, and what else the tree tells its host (a redraw, semantics changes).

use super::alignment_lines::NOT_PLACED;
use super::{LayoutNode, LayoutState, SnapshotObserver, UsageByParent};
use crate::unit::Constraints;
use kompoz_coroutines::Dispatcher;
use kompoz_graphics::{IntOffset, Offset, Rect};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};

/// A set of layout nodes taken shallowest first (Compose's DepthSortedSet).
#[derive(Default)]
struct DepthSortedSet {
    nodes: BTreeMap<(u32, u64), Weak<LayoutNode>>,
}

impl DepthSortedSet {
    fn add(&mut self, node: &Rc<LayoutNode>) {
        self.nodes.insert((node.depth(), node.id), Rc::downgrade(node));
    }
    fn remove(&mut self, node: &LayoutNode) -> bool {
        self.nodes.remove(&(node.depth(), node.id)).is_some()
    }
    fn pop(&mut self) -> Option<Rc<LayoutNode>> {
        while let Some((_, w)) = self.nodes.pop_first() {
            if let Some(n) = w.upgrade() {
                return Some(n);
            }
        }
        None
    }
    fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// A node's place as the layout check writes it down: where its outer and inner coordinators are in the root,
/// whether it is placed, its z-index.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct LaidOut {
    pub(crate) bounds: Rect,
    pub(crate) content: Rect,
    pub(crate) placed: bool,
    pub(crate) z_index: f32,
}

/// The owner of a layout tree: what its nodes ask of their host.
pub(crate) struct Owner {
    root: RefCell<Option<Rc<LayoutNode>>>,
    snapshot_observer: Rc<SnapshotObserver>,
    dispatcher: Dispatcher,
    measure_set: RefCell<DepthSortedSet>,
    place_set: RefCell<DepthSortedSet>,
    on_positioned: RefCell<Vec<Weak<LayoutNode>>>,
    postponed: RefCell<Vec<(Weak<LayoutNode>, bool)>>,
    root_constraints: Cell<Option<Constraints>>,
    during_measure_layout: Cell<bool>,
    during_full_pass: Cell<bool>,
    needs_draw: Cell<bool>,
    semantics_dirty: Cell<bool>,
    window_offset: Cell<Offset>,
    /// Running the layout check: nodes' measure and placement callbacks aren't told.
    pub(crate) checking: Cell<bool>,
    /// How many times a node was measured and placed (the tests read it: C++'s relaidOutCount).
    pub(crate) relaid_out: Cell<u64>,
    /// What runs once the current composition's changes are applied (Compose's `registerOnEndApplyChangesListener`).
    end_apply: RefCell<Vec<Box<dyn FnOnce()>>>,
    pub(crate) keep_screen_on: Cell<i32>,
    pub(crate) sensitive_content: Cell<i32>,
    pub(crate) frame_rates: RefCell<Vec<(u64, crate::platform::FrameRate)>>,
    /// The system gesture exclusion nodes' rects, by node, and whether they changed since the platform was told.
    gesture_exclusion: RefCell<Vec<(usize, kompoz_graphics::IntRect)>>,
    gesture_exclusion_changed: Cell<bool>,
    focus: RefCell<Weak<crate::focus::FocusOwner>>,
    /// The popup roots in the tree, bottom first (the window package's).
    pub(crate) popups: RefCell<Vec<Weak<LayoutNode>>>,
    /// Told of semantics changes (the semantics package's), and the layouts whose semantics changed with what
    /// they were.
    pub(crate) semantics_listeners: RefCell<Vec<Rc<dyn crate::semantics::SemanticsListener>>>,
    pub(crate) semantics_pending: RefCell<Vec<(Weak<LayoutNode>, Option<crate::semantics::SemanticsConfiguration>)>>,
}

impl Owner {
    pub(crate) fn set_focus_owner(&self, focus: &Rc<crate::focus::FocusOwner>) {
        *self.focus.borrow_mut() = Rc::downgrade(focus);
    }

    /// The host's focus state.
    pub(crate) fn focus(&self) -> Option<Rc<crate::focus::FocusOwner>> {
        self.focus.borrow().upgrade()
    }

    pub(crate) fn new(dispatcher: Dispatcher) -> Rc<Owner> {
        Rc::new(Owner {
            root: RefCell::new(None),
            snapshot_observer: SnapshotObserver::new(),
            dispatcher,
            measure_set: RefCell::new(DepthSortedSet::default()),
            place_set: RefCell::new(DepthSortedSet::default()),
            on_positioned: RefCell::new(Vec::new()),
            postponed: RefCell::new(Vec::new()),
            root_constraints: Cell::new(None),
            during_measure_layout: Cell::new(false),
            during_full_pass: Cell::new(false),
            needs_draw: Cell::new(true),
            semantics_dirty: Cell::new(true),
            window_offset: Cell::new(Offset::Zero),
            checking: Cell::new(false),
            relaid_out: Cell::new(0),
            end_apply: RefCell::new(Vec::new()),
            keep_screen_on: Cell::new(0),
            sensitive_content: Cell::new(0),
            frame_rates: RefCell::new(Vec::new()),
            gesture_exclusion: RefCell::new(Vec::new()),
            gesture_exclusion_changed: Cell::new(false),
            focus: RefCell::new(Weak::new()),
            popups: RefCell::new(Vec::new()),
            semantics_listeners: RefCell::new(Vec::new()),
            semantics_pending: RefCell::new(Vec::new()),
        })
    }

    /// Makes `root` its tree's root and attaches it.
    pub(crate) fn set_root(self: &Rc<Self>, root: Rc<LayoutNode>) {
        *self.root.borrow_mut() = Some(root.clone());
        root.attach(self);
    }

    pub(crate) fn root(&self) -> Option<Rc<LayoutNode>> {
        self.root.borrow().clone()
    }

    /// A system gesture exclusion node's rect (`key`: the node): kept in its place, or dropped (Compose's
    /// RectListNode `replaceRect` on the view's list).
    pub(crate) fn set_gesture_exclusion(&self, key: usize, rect: Option<kompoz_graphics::IntRect>) {
        let mut rects = self.gesture_exclusion.borrow_mut();
        let at = rects.iter().position(|(k, _)| *k == key);
        let changed = match (at, rect) {
            (Some(i), Some(r)) => std::mem::replace(&mut rects[i].1, r) != r,
            (Some(i), None) => {
                rects.remove(i);
                true
            }
            (None, Some(r)) => {
                rects.push((key, r));
                true
            }
            (None, None) => false,
        };
        if changed {
            self.gesture_exclusion_changed.set(true);
        }
    }

    /// The system gesture exclusion rects, px in the window.
    pub(crate) fn gesture_exclusion(&self) -> Vec<kompoz_graphics::IntRect> {
        self.gesture_exclusion.borrow().iter().map(|(_, r)| *r).collect()
    }

    /// Whether they changed since this was last asked.
    pub(crate) fn take_gesture_exclusion_changed(&self) -> bool {
        self.gesture_exclusion_changed.replace(false)
    }

    pub(crate) fn snapshot_observer(&self) -> &Rc<SnapshotObserver> {
        &self.snapshot_observer
    }

    pub(crate) fn dispatcher(&self) -> Dispatcher {
        self.dispatcher.clone()
    }

    pub(crate) fn window_offset(&self) -> Offset {
        self.window_offset.get()
    }

    pub(crate) fn set_window_offset(&self, offset: Offset) {
        self.window_offset.set(offset);
    }

    /// A node came into the tree: a popup's root is one of its overlays.
    pub(crate) fn on_attach(&self, node: &Rc<LayoutNode>) {
        if node.is_popup_root() {
            self.register_popup(node);
        }
    }

    /// A node left the tree: nothing of it is waiting any more (Compose's `onNodeDetached`).
    pub(crate) fn on_detach(&self, node: &LayoutNode) {
        if node.is_popup_root() {
            self.unregister_popup(node);
        }
        self.semantics_pending.borrow_mut().retain(|(w, _)| !std::ptr::eq(w.as_ptr(), node));
        self.measure_set.borrow_mut().remove(node);
        self.place_set.borrow_mut().remove(node);
        self.on_positioned.borrow_mut().retain(|w| !std::ptr::eq(w.as_ptr(), node));
        self.frame_rates.borrow_mut().retain(|(id, _)| *id != node.id);
    }

    pub(crate) fn request_draw(&self) {
        self.needs_draw.set(true);
    }

    /// Whether a redraw is due, and it is no longer (the host draws).
    pub(crate) fn take_needs_draw(&self) -> bool {
        self.needs_draw.replace(false)
    }

    pub(crate) fn needs_draw(&self) -> bool {
        self.needs_draw.get()
    }

    pub(crate) fn semantics_changed(&self) {
        self.semantics_dirty.set(true);
    }

    pub(crate) fn semantics_bounds_changed(&self) {
        self.semantics_dirty.set(true);
    }

    pub(crate) fn take_semantics_dirty(&self) -> bool {
        self.semantics_dirty.replace(false)
    }

    /// Runs `f` once the composition's changes are applied (Compose's `Modifier.Node.sideEffect`).
    pub(crate) fn register_on_end_apply_changes(&self, f: Box<dyn FnOnce()>) {
        self.end_apply.borrow_mut().push(f);
    }

    pub(crate) fn run_end_apply_listeners(&self) {
        let fs = std::mem::take(&mut *self.end_apply.borrow_mut());
        for f in fs {
            f();
        }
    }

    // ---- The measure and layout delegate ----

    /// The constraints the root is measured with (the window's size).
    pub(crate) fn update_root_constraints(&self, constraints: Constraints) {
        if self.root_constraints.get() == Some(constraints) {
            return;
        }
        assert!(!self.during_measure_layout.get(), "kompoz: updateRootConstraints called while measuring");
        self.root_constraints.set(Some(constraints));
        if let Some(root) = self.root() {
            root.mark_measure_pending();
            self.measure_set.borrow_mut().add(&root);
        }
    }

    /// The constraints the root is measured with (the window's), once set.
    pub(crate) fn root_constraints(&self) -> Option<Constraints> {
        self.root_constraints.get()
    }

    /// Whether nodes wait to be measured or placed.
    pub(crate) fn has_pending_measure_or_layout(&self) -> bool {
        !self.measure_set.borrow().is_empty() || !self.place_set.borrow().is_empty()
    }

    pub(crate) fn has_pending_on_positioned_callbacks(&self) -> bool {
        !self.on_positioned.borrow().is_empty()
    }

    /// Compose's `requestRemeasure`: whether a layout pass is wanted for it.
    pub(crate) fn request_remeasure(&self, node: &Rc<LayoutNode>, force: bool) -> bool {
        match node.layout_state.get() {
            LayoutState::Measuring => false,
            LayoutState::LayingOut => {
                self.postponed.borrow_mut().push((Rc::downgrade(node), force));
                false
            }
            LayoutState::Idle => {
                if node.measure_pending.get() && !force {
                    return false;
                }
                node.mark_measure_pending();
                if node.is_placed() || can_affect_placed_parent(node) {
                    if node.parent().is_none_or(|p| !p.measure_pending.get()) {
                        self.measure_set.borrow_mut().add(node);
                    }
                    !self.during_full_pass.get()
                } else {
                    false
                }
            }
        }
    }

    /// Compose's `requestRelayout`.
    pub(crate) fn request_relayout(&self, node: &Rc<LayoutNode>, force: bool) -> bool {
        match node.layout_state.get() {
            LayoutState::Measuring | LayoutState::LayingOut => false,
            LayoutState::Idle => {
                let parent = node.parent();
                let parent_is_placed = parent.as_ref().is_none_or(|p| p.is_placed());
                if !force
                    && (node.measure_pending.get()
                        || (node.layout_pending.get()
                            && node.is_placed() == parent_is_placed
                            && node.is_placed() == node.is_placed_by_parent.get()))
                {
                    return false;
                }
                node.mark_layout_pending();
                if node.is_placed_by_parent.get() && parent_is_placed {
                    if parent.as_ref().is_none_or(|p| !p.layout_pending.get() && !p.measure_pending.get()) {
                        self.place_set.borrow_mut().add(node);
                    }
                    !self.during_full_pass.get()
                } else {
                    false
                }
            }
        }
    }

    /// A node's global position callbacks are due after this pass (Compose's `onNodePositioned`).
    pub(crate) fn request_on_positioned_callback(&self, node: &Rc<LayoutNode>) {
        if node.globally_positioned_observers() > 0 {
            self.on_positioned.borrow_mut().push(Rc::downgrade(node));
            node.needs_on_positioned_dispatch.set(true);
        }
    }

    /// Measures and places what waits, shallowest first (Compose's `measureAndLayout`). Whether the root was
    /// resized.
    pub(crate) fn measure_and_layout(&self) -> bool {
        let Some(root) = self.root() else { return false };
        if self.root_constraints.get().is_none() || !root.is_attached() {
            return false;
        }
        assert!(!self.during_measure_layout.get(), "kompoz: measure and layout called while measuring");
        self.during_measure_layout.set(true);
        self.during_full_pass.set(true);
        let mut root_resized = false;
        'passes: loop {
            let (node, relayout) = {
                let next = self.measure_set.borrow_mut().pop();
                match next {
                    Some(n) => (n, false),
                    None => match { self.place_set.borrow_mut().pop() } {
                        Some(n) => (n, true),
                        None => {
                            // The main tree laid out: the popups against it, then what that set off.
                            if self.has_popups() {
                                self.layout_popups();
                                if !self.measure_set.borrow().is_empty() || !self.place_set.borrow().is_empty() {
                                    continue 'passes;
                                }
                            }
                            break;
                        }
                    },
                }
            };
            let size_changed = if relayout {
                self.remeasure_and_relayout_if_needed(&node)
            } else {
                let changed = self.remeasure_if_needed(&node);
                if node.layout_pending.get() {
                    self.place_set.borrow_mut().add(&node);
                }
                changed
            };
            if Rc::ptr_eq(&node, &root) && size_changed {
                root_resized = true;
            }
        }
        self.during_measure_layout.set(false);
        self.during_full_pass.set(false);
        root_resized
    }

    /// Whether a measure and layout pass runs now.
    pub(crate) fn is_measuring(&self) -> bool {
        self.during_measure_layout.get()
    }

    /// Measures `node` within `constraints` and lays it out now, apart from its parent's measure (Compose's
    /// `measureAndLayout(layoutNode, constraints)`: a precomposed slot premeasured).
    pub(crate) fn measure_and_layout_node(&self, node: &Rc<LayoutNode>, constraints: Constraints) {
        let root = self.root();
        assert!(root.is_none_or(|r| !Rc::ptr_eq(&r, node)), "kompoz: measure_and_layout_node is not for the root");
        let nested = self.during_measure_layout.replace(true);
        self.place_set.borrow_mut().remove(node);
        self.do_remeasure(node, Some(constraints));
        if node.layout_pending.get() && node.is_placed() {
            node.replace();
            self.request_on_positioned_callback(node);
        }
        self.drain_postponed();
        self.during_measure_layout.set(nested);
    }

    fn is_used_in_measure_or_layout(&self, node: &LayoutNode) -> bool {
        node.is_placed()
            || node.is_placed_by_parent.get()
            || can_affect_placed_parent(node)
            || node.alignment_lines_required()
    }

    fn root_constraints_for(&self, node: &Rc<LayoutNode>) -> Option<Constraints> {
        let root = self.root()?;
        if Rc::ptr_eq(node, &root) {
            self.root_constraints.get()
        } else if node.parent().is_none() {
            self.popup_constraints(node)
        } else {
            None
        }
    }

    /// Compose's `doRemeasure`: a size change measures (or places) its parent again.
    fn do_remeasure(&self, node: &Rc<LayoutNode>, constraints: Option<Constraints>) -> bool {
        let changed = match constraints {
            Some(c) => {
                if node.intrinsics_usage_by_parent.get() == UsageByParent::NotUsed {
                    node.clear_subtree_intrinsics_usage();
                }
                node.remeasure(c)
            }
            None => node.remeasure_with_last_constraints(),
        };
        self.relaid_out.set(self.relaid_out.get() + 1);
        if changed && let Some(parent) = node.parent() {
            match node.measured_by_parent.get() {
                UsageByParent::InMeasureBlock => parent.request_remeasure_with(false, false),
                UsageByParent::InLayoutBlock => parent.request_relayout(),
                UsageByParent::NotUsed => {}
            }
        }
        changed
    }

    fn remeasure_and_relayout_if_needed(&self, node: &Rc<LayoutNode>) -> bool {
        let mut size_changed = false;
        if self.is_used_in_measure_or_layout(node) {
            let constraints = self.root_constraints_for(node);
            if node.measure_pending.get() {
                size_changed = self.do_remeasure(node, constraints);
            }
            if node.layout_pending.get() {
                let is_root = self.root().is_some_and(|r| Rc::ptr_eq(&r, node));
                if node.is_popup_root() {
                    // (placed by the popups' pass, against its anchor)
                    self.drain_postponed();
                    return size_changed;
                }
                let placed_by_placed_parent =
                    is_root || (node.parent().is_some_and(|p| p.is_placed()) && node.is_placed_by_parent.get());
                if placed_by_placed_parent {
                    if is_root {
                        node.place_root();
                    } else {
                        node.replace();
                    }
                    self.relaid_out.set(self.relaid_out.get() + 1);
                    self.request_on_positioned_callback(node);
                }
            }
            self.drain_postponed();
        }
        size_changed
    }

    fn remeasure_if_needed(&self, node: &Rc<LayoutNode>) -> bool {
        let mut size_changed = false;
        if self.is_used_in_measure_or_layout(node) {
            let constraints = self.root_constraints_for(node);
            if node.measure_pending.get() {
                size_changed = self.do_remeasure(node, constraints);
            }
            self.drain_postponed();
        }
        size_changed
    }

    fn drain_postponed(&self) {
        let postponed = std::mem::take(&mut *self.postponed.borrow_mut());
        for (node, force) in postponed {
            if let Some(n) = node.upgrade()
                && n.is_attached()
            {
                n.request_remeasure_with(force, false);
            }
        }
    }

    /// Measures what in `node` waits to be and can change its size, during its parent's measure (Compose's
    /// `forceMeasureTheSubtree`).
    pub(crate) fn force_measure_the_subtree(&self, node: &Rc<LayoutNode>) {
        if !self.during_measure_layout.get() {
            return;
        }
        self.force_measure_the_subtree_internal(node);
    }

    fn force_measure_the_subtree_internal(&self, node: &Rc<LayoutNode>) {
        for child in node.children() {
            let can_affect =
                child.measured_by_parent.get() == UsageByParent::InMeasureBlock || child.alignment_lines_required();
            if can_affect {
                if child.measure_pending.get() {
                    self.remeasure_if_needed(&child);
                }
                if !child.measure_pending.get() {
                    self.force_measure_the_subtree_internal(&child);
                }
            }
        }
        if node.measure_pending.get() {
            self.remeasure_if_needed(node);
        }
    }

    /// Tells the global position nodes of what was placed in the pass, shallowest first, and those in them
    /// (Compose's OnPositionedDispatcher).
    pub(crate) fn dispatch_on_positioned_callbacks(&self) {
        if self.checking.get() {
            self.on_positioned.borrow_mut().clear();
            return;
        }
        let mut nodes: Vec<Rc<LayoutNode>> =
            std::mem::take(&mut *self.on_positioned.borrow_mut()).iter().filter_map(Weak::upgrade).collect();
        nodes.sort_by(|a, b| b.depth().cmp(&a.depth()).then(a.id.cmp(&b.id)));
        for node in nodes.iter().rev() {
            if node.needs_on_positioned_dispatch.get() {
                dispatch_hierarchy(node);
            }
        }
    }

    /// All of the tree is told where it is (Compose's `onRootNodePositioned`, the window moved).
    pub(crate) fn dispatch_root_positioned(&self) {
        if let Some(root) = self.root()
            && root.globally_positioned_observers() > 0
        {
            self.on_positioned.borrow_mut().clear();
            self.on_positioned.borrow_mut().push(Rc::downgrade(&root));
            root.needs_on_positioned_dispatch.set(true);
        }
    }

    // ---- The layout check ----

    /// Where every node is now (C++'s `collectLayout`).
    pub(crate) fn collect_layout(&self) -> Vec<(u64, LaidOut)> {
        let mut out = Vec::new();
        if let Some(root) = self.root() {
            collect(&root, &mut out);
        }
        out
    }

    /// The layout check (C++'s KOMPOZ_LAYOUT_CHECK): every node measured and placed again from scratch, which must
    /// come out where the incremental layout left it. The differences found, described.
    pub(crate) fn check_layout(&self) -> Vec<String> {
        let Some(root) = self.root() else { return Vec::new() };
        if self.root_constraints.get().is_none() || self.has_pending_measure_or_layout() {
            return Vec::new();
        }
        let before = self.collect_layout();
        self.checking.set(true);
        mark_all_pending(&root);
        self.measure_set.borrow_mut().add(&root);
        self.measure_and_layout();
        self.on_positioned.borrow_mut().clear();
        self.checking.set(false);
        let after = self.collect_layout();
        let mut mismatches = Vec::new();
        for (id, laid) in &after {
            match before.iter().find(|(i, _)| i == id) {
                Some((_, was)) if same_laid_out(was, laid) => {}
                Some((_, was)) => {
                    mismatches.push(format!("kompoz: layout check: node #{id} was {was:?}, is {laid:?}"));
                }
                None => mismatches.push(format!("kompoz: layout check: node #{id} wasn't laid out, is {laid:?}")),
            }
        }
        mismatches
    }
}

fn same_laid_out(a: &LaidOut, b: &LaidOut) -> bool {
    let close = |x: Rect, y: Rect| {
        (x.left - y.left).abs() < 0.01
            && (x.top - y.top).abs() < 0.01
            && (x.right - y.right).abs() < 0.01
            && (x.bottom - y.bottom).abs() < 0.01
    };
    a.placed == b.placed
        && a.z_index == b.z_index
        && (!a.placed || (close(a.bounds, b.bounds) && close(a.content, b.content)))
}

fn collect(node: &Rc<LayoutNode>, out: &mut Vec<(u64, LaidOut)>) {
    let placed = node.is_placed();
    let (bounds, content) = if placed && node.is_attached() {
        let outer = node.coordinates();
        let inner = node.inner_coordinates();
        let root = outer.find_root_coordinates();
        (root.local_bounding_box_of(&outer, false), root.local_bounding_box_of(&inner, false))
    } else {
        (Rect::Zero, Rect::Zero)
    };
    out.push((node.id, LaidOut { bounds, content, placed, z_index: node.z_index.get() }));
    for child in node.children() {
        collect(&child, out);
    }
}

fn mark_all_pending(node: &Rc<LayoutNode>) {
    node.mark_measure_pending();
    node.mark_layout_pending();
    for child in node.children() {
        mark_all_pending(&child);
    }
}

fn dispatch_hierarchy(node: &Rc<LayoutNode>) {
    if node.globally_positioned_observers() > 0 {
        node.dispatch_on_positioned_callbacks();
        node.needs_on_positioned_dispatch.set(false);
        for child in node.children() {
            dispatch_hierarchy(&child);
        }
    }
}

/// Whether measuring it again may change a placed parent (Compose's `canAffectPlacedParent`).
fn can_affect_placed_parent(node: &LayoutNode) -> bool {
    node.measure_pending.get() && measured_by_placed_parent(node)
}

fn measured_by_placed_parent(node: &LayoutNode) -> bool {
    let mut n = node.rc();
    loop {
        let parent_measuring = n.parent().is_some_and(|p| p.layout_state.get() == LayoutState::Measuring);
        if !(n.measured_by_parent.get() != UsageByParent::NotUsed || n.alignment_lines_required() || parent_measuring) {
            return false;
        }
        let Some(parent) = n.parent() else { return false };
        if parent.is_placed() {
            return true;
        }
        n = parent;
    }
}

impl LayoutNode {
    /// `request_remeasure` choosing whether the intrinsics users above are told (Compose's `invalidateIntrinsics`).
    pub(crate) fn request_remeasure_with(&self, force: bool, invalidate_intrinsics: bool) {
        if self.ignore_remeasure_requests.get() {
            return;
        }
        let Some(owner) = self.owner() else { return };
        owner.request_remeasure(&self.rc(), force);
        if invalidate_intrinsics {
            self.invalidate_intrinsics_parent(force);
        }
    }

    /// Whether its parent placed it in the last placement.
    pub fn placed_by_parent(&self) -> bool {
        self.place_order.get() != NOT_PLACED
    }

    /// Whether it waits to be measured or placed again.
    pub fn is_layout_pending(&self) -> bool {
        self.measure_pending.get() || self.layout_pending.get()
    }

    /// Its position in its parent's inner coordinates (where its outer coordinator is).
    pub fn position_in_parent(&self) -> IntOffset {
        self.outer_coordinator().position.get()
    }
}
