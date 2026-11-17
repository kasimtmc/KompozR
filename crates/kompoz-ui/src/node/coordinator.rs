//! Coordinators, as Compose's NodeCoordinator: one for each layout modifier node of a layout node's chain
//! (`LayoutModifierNodeCoordinator`) and the inner one (`InnerNodeCoordinator`: the measure policy and the
//! children), outer wrapping inner. Each has its measured size, its position in the one wrapping it, its measure
//! result and maybe a layer; the chain's other nodes belong to the coordinator after them, which is where they
//! draw, hit test and report coordinates.

use super::{LayoutNode, ReaderScope};
use crate::layout::{
    AlignmentLine, GraphicsLayerScope, IntrinsicMeasurable, IntrinsicMeasureScope, LayerBlock, LayoutCoordinates,
    Measurable, MeasureResult, MeasureScope, PlaceTarget, Placeable, PlacementScope,
};
use crate::modifier::{NodeKind, NodeRc, dispatch_for_kind};
use crate::unit::Constraints;
use kompoz_graphics::{IntOffset, IntSize, Matrix, Offset, Rect};
use kompoz_runtime::without_read_observation;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// What a coordinator measures with.
pub(crate) enum CoordinatorKind {
    /// The layout node's measure policy and children.
    Inner,
    /// A layout modifier node: its chain node, and the node (it or a delegate) that is the layout modifier.
    Modifier { entry: RefCell<NodeRc>, layout: RefCell<NodeRc> },
}

/// A coordinator's layer: its block, the properties it set last, and their matrix and its inverse.
pub(crate) struct Layer {
    pub(crate) block: LayerBlock,
    pub(crate) props: GraphicsLayerScope,
    pub(crate) matrix: Matrix,
    inverse: Matrix,
}

pub struct NodeCoordinator {
    layout_node: Weak<LayoutNode>,
    pub(crate) kind: CoordinatorKind,
    pub(crate) wrapped: RefCell<Option<Rc<NodeCoordinator>>>,
    pub(crate) wrapped_by: RefCell<Weak<NodeCoordinator>>,
    /// The chain nodes that belong to it, head first (its own layout node last).
    nodes: RefCell<Vec<NodeRc>>,
    pub(crate) measured_size: Cell<IntSize>,
    measurement_constraints: Cell<Constraints>,
    pub(crate) position: Cell<IntOffset>,
    pub(crate) z_index: Cell<f32>,
    measure_result: RefCell<Option<MeasureResult>>,
    pub(crate) layer: RefCell<Option<Layer>>,
    layer_reads: RefCell<Option<Rc<ReaderScope>>>,
    /// Placing only to work out alignment lines (Compose's `isPlacingForAlignment`).
    pub(crate) placing_for_alignment: Cell<bool>,
    /// Placing only itself, not what it wraps (Compose's `isShallowPlacing`).
    pub(crate) shallow_placing: Cell<bool>,
    released: Cell<bool>,
    /// The rulers it provides and who read them (made when first needed).
    pub(crate) rulers: RefCell<Option<Rc<super::rulers::CoordinatorRulers>>>,
    this: Weak<NodeCoordinator>,
}

impl NodeCoordinator {
    fn new(layout_node: Weak<LayoutNode>, kind: CoordinatorKind) -> Rc<NodeCoordinator> {
        Rc::new_cyclic(|this| NodeCoordinator {
            layout_node,
            kind,
            wrapped: RefCell::new(None),
            wrapped_by: RefCell::new(Weak::new()),
            nodes: RefCell::new(Vec::new()),
            measured_size: Cell::new(IntSize::Zero),
            measurement_constraints: Cell::new(Constraints::default()),
            position: Cell::new(IntOffset::Zero),
            z_index: Cell::new(0.0),
            measure_result: RefCell::new(None),
            layer: RefCell::new(None),
            layer_reads: RefCell::new(None),
            placing_for_alignment: Cell::new(false),
            shallow_placing: Cell::new(false),
            released: Cell::new(false),
            rulers: RefCell::new(None),
            this: this.clone(),
        })
    }

    pub(crate) fn new_inner(layout_node: Weak<LayoutNode>) -> Rc<NodeCoordinator> {
        NodeCoordinator::new(layout_node, CoordinatorKind::Inner)
    }

    pub(crate) fn new_modifier(layout_node: Weak<LayoutNode>, entry: NodeRc, layout: NodeRc) -> Rc<NodeCoordinator> {
        NodeCoordinator::new(
            layout_node,
            CoordinatorKind::Modifier { entry: RefCell::new(entry), layout: RefCell::new(layout) },
        )
    }

    pub(crate) fn rc(&self) -> Rc<NodeCoordinator> {
        self.this.upgrade().expect("a coordinator in use is alive")
    }

    pub(crate) fn is_inner(&self) -> bool {
        matches!(self.kind, CoordinatorKind::Inner)
    }

    /// The chain node of a layout modifier coordinator.
    pub(crate) fn entry(&self) -> Option<NodeRc> {
        match &self.kind {
            CoordinatorKind::Modifier { entry, .. } => Some(entry.borrow().clone()),
            CoordinatorKind::Inner => None,
        }
    }

    pub(crate) fn set_layout_cell(&self, cell: NodeRc) {
        if let CoordinatorKind::Modifier { layout, .. } = &self.kind {
            let changed = !Rc::ptr_eq(&layout.borrow(), &cell);
            *layout.borrow_mut() = cell;
            if changed {
                self.invalidate_layer();
            }
        }
    }

    fn layout_cell(&self) -> Option<NodeRc> {
        match &self.kind {
            CoordinatorKind::Modifier { layout, .. } => Some(layout.borrow().clone()),
            CoordinatorKind::Inner => None,
        }
    }

    pub(crate) fn set_nodes(&self, nodes: Vec<NodeRc>) {
        *self.nodes.borrow_mut() = nodes;
    }

    /// Its chain nodes, head first.
    pub(crate) fn nodes(&self) -> Vec<NodeRc> {
        self.nodes.borrow().clone()
    }

    /// Calls `f` on its nodes of `kind` (delegates included), head first (Compose's `visitNodes`).
    pub(crate) fn visit_nodes(&self, kind: NodeKind, f: &mut dyn FnMut(&NodeRc)) {
        for node in self.nodes() {
            if node.is_kind(kind) {
                dispatch_for_kind(&node, kind, f);
            }
        }
    }

    pub(crate) fn has_node(&self, kind: NodeKind) -> bool {
        self.nodes.borrow().iter().any(|n| n.is_kind(kind))
    }

    pub(crate) fn layout_node(&self) -> Option<Rc<LayoutNode>> {
        self.layout_node.upgrade()
    }

    fn node(&self) -> Rc<LayoutNode> {
        self.layout_node.upgrade().expect("a coordinator of a layout node that is gone")
    }

    pub(crate) fn wrapped(&self) -> Option<Rc<NodeCoordinator>> {
        self.wrapped.borrow().clone()
    }

    pub(crate) fn wrapped_by(&self) -> Option<Rc<NodeCoordinator>> {
        self.wrapped_by.borrow().upgrade()
    }

    pub(crate) fn is_attached(&self) -> bool {
        !self.released.get() && self.layout_node().is_some_and(|n| n.is_attached())
    }

    pub(crate) fn on_coordinates_used(&self) {
        if let Some(n) = self.layout_node() {
            n.on_coordinates_used();
        }
    }

    pub(crate) fn measure_result(&self) -> Option<MeasureResult> {
        self.measure_result.borrow().clone()
    }

    pub(crate) fn measure_result_lines(&self) -> Vec<AlignmentLine> {
        self.measure_result
            .borrow()
            .as_ref()
            .map_or_else(Vec::new, |r| r.alignment_lines().iter().map(|l| l.0).collect())
    }

    pub(crate) fn measure_result_line_values(&self) -> Vec<(AlignmentLine, i32)> {
        self.measure_result.borrow().as_ref().map_or_else(Vec::new, |r| r.alignment_lines().to_vec())
    }

    pub(crate) fn provides_line(&self, line: AlignmentLine) -> bool {
        self.measure_result.borrow().as_ref().is_some_and(|r| r.alignment_line(line).is_some())
    }

    pub(crate) fn measure_scope(&self) -> MeasureScope {
        MeasureScope::for_node(&self.node())
    }

    /// The scope its measure result's placement block places in (Compose's `placementScope`).
    pub(crate) fn placement_scope(&self) -> PlacementScope {
        let n = self.node();
        PlacementScope {
            parent_width: self.measured_size.get().width,
            parent_layout_direction: n.layout_direction(),
            density: n.density(),
            coordinator: Some(self.rc()),
        }
    }

    // ---- Measuring ----

    /// Measures: the layout modifier with what it wraps, or the measure policy with the children (Compose's
    /// `measure` of LayoutModifierNodeCoordinator and InnerNodeCoordinator).
    pub(crate) fn perform_measure(&self, constraints: Constraints) {
        self.measurement_constraints.set(constraints);
        let scope = self.measure_scope();
        let result = match &self.kind {
            CoordinatorKind::Inner => {
                let n = self.node();
                let children = n.children();
                for c in &children {
                    c.measured_by_parent.set(super::UsageByParent::NotUsed);
                }
                let refs: Vec<&dyn Measurable> = children.iter().map(|c| &**c as &dyn Measurable).collect();
                let policy = n.measure_policy();
                policy.measure(&scope, &refs, constraints)
            }
            CoordinatorKind::Modifier { .. } => {
                let wrapped = self.wrapped().expect("a layout modifier coordinator wraps another");
                let cell = self.layout_cell().unwrap();
                let mut node = cell.node.borrow_mut();
                let layout = node.as_layout().expect("a layout coordinator's node is a layout node");
                layout.measure(&scope, &*wrapped, constraints)
            }
        };
        self.set_measure_result(result);
        self.on_measured();
    }

    /// Compose's `measureResult` setter: a new size resizes its layer or redraws its parent; new lines tell the
    /// layout node's alignment lines.
    pub(crate) fn set_measure_result(&self, result: MeasureResult) {
        let old = self.measure_result.borrow_mut().replace(result.clone());
        let resized = old.as_ref().is_none_or(|o| o.width != result.width || o.height != result.height);
        if resized {
            self.on_measure_result_changed(result.width, result.height);
        }
        let lines_changed = match &old {
            Some(o) => {
                (!o.alignment_lines().is_empty() || !result.alignment_lines().is_empty()) && !o.same_lines(&result)
            }
            None => !result.alignment_lines().is_empty(),
        };
        if lines_changed && let Some(n) = self.layout_node() {
            n.on_alignments_changed();
        }
    }

    fn on_measure_result_changed(&self, width: i32, height: i32) {
        self.measured_size.set(IntSize::new(width, height));
        let has_layer = self.layer.borrow().is_some();
        if has_layer {
            self.update_layer_parameters(false);
            self.invalidate_layer();
        } else if self.layout_node().is_some_and(|n| n.is_placed()) {
            self.invalidate_parent_layer();
        }
        self.on_draw_size_changed();
        if let Some(n) = self.layout_node() {
            n.on_coordinator_rect_changed(self);
        }
    }

    /// Whether its owner is running the layout check (its nodes aren't told then).
    fn checking(&self) -> bool {
        self.layout_node().and_then(|n| n.owner()).is_some_and(|o| o.checking.get())
    }

    /// Tells its nodes that care that it was measured (Compose's `onMeasured`).
    pub(crate) fn on_measured(&self) {
        if !self.has_node(NodeKind::OnRemeasured) || self.checking() {
            return;
        }
        let size = self.measured_size.get();
        without_read_observation(|| {
            self.visit_nodes(NodeKind::OnRemeasured, &mut |cell| {
                if let Some(aware) = cell.node.borrow_mut().as_layout_aware() {
                    aware.on_remeasured(size);
                }
            });
        });
    }

    pub(crate) fn min_intrinsic_width(&self, height: i32) -> i32 {
        self.intrinsic(
            height,
            |n, scope, m, h| n.min_intrinsic_width(scope, m, h),
            |n, h| n.node_min_intrinsic_width(h),
        )
    }
    pub(crate) fn max_intrinsic_width(&self, height: i32) -> i32 {
        self.intrinsic(
            height,
            |n, scope, m, h| n.max_intrinsic_width(scope, m, h),
            |n, h| n.node_max_intrinsic_width(h),
        )
    }
    pub(crate) fn min_intrinsic_height(&self, width: i32) -> i32 {
        self.intrinsic(
            width,
            |n, scope, m, w| n.min_intrinsic_height(scope, m, w),
            |n, w| n.node_min_intrinsic_height(w),
        )
    }
    pub(crate) fn max_intrinsic_height(&self, width: i32) -> i32 {
        self.intrinsic(
            width,
            |n, scope, m, w| n.max_intrinsic_height(scope, m, w),
            |n, w| n.node_max_intrinsic_height(w),
        )
    }

    fn intrinsic(
        &self,
        size: i32,
        modifier: impl FnOnce(
            &mut dyn crate::layout::LayoutModifierNode,
            &IntrinsicMeasureScope,
            &dyn IntrinsicMeasurable,
            i32,
        ) -> i32,
        inner: impl FnOnce(&LayoutNode, i32) -> i32,
    ) -> i32 {
        match &self.kind {
            CoordinatorKind::Inner => inner(&self.node(), size),
            CoordinatorKind::Modifier { .. } => {
                let scope = self.measure_scope();
                let wrapped = self.wrapped().expect("a layout modifier coordinator wraps another");
                let cell = self.layout_cell().unwrap();
                let mut node = cell.node.borrow_mut();
                let layout = node.as_layout().expect("a layout coordinator's node is a layout node");
                modifier(layout, &scope, &*wrapped, size)
            }
        }
    }

    // ---- Placing ----

    /// Places it (Compose's `placeAt` of the two coordinators): itself, then — unless placing only itself — a
    /// layout modifier's nodes are told and its placement block places what it wraps; the inner one tells its
    /// layout node it was placed, which places the children.
    pub(crate) fn place_at(&self, position: IntOffset, z_index: f32, layer: Option<LayerBlock>) {
        self.place_self(position, z_index, layer);
        if self.shallow_placing.get() {
            return;
        }
        match &self.kind {
            CoordinatorKind::Inner => self.node().on_node_placed(),
            CoordinatorKind::Modifier { .. } => {
                self.on_placed();
                let wrapped = self.wrapped().expect("a layout modifier coordinator wraps another");
                let was = wrapped.placing_for_alignment.replace(self.placing_for_alignment.get());
                if let Some(result) = self.measure_result() {
                    result.place_children(&self.placement_scope());
                }
                wrapped.placing_for_alignment.set(was);
            }
        }
    }

    /// Places it again where it was, through the layer it had (Compose's `replace`).
    pub(crate) fn replace(&self) {
        let block = self.layer.borrow().as_ref().map(|l| l.block.clone());
        self.place_at(self.position.get(), self.z_index.get(), block);
    }

    /// How far its real top left is from where what places it thinks it is: centered over its constraints when it
    /// measured itself outside them (Compose's `apparentToRealOffset`).
    pub(crate) fn apparent_to_real_offset(&self) -> IntOffset {
        let c = self.measurement_constraints.get();
        let m = self.measured_size.get();
        let w = m.width.clamp(c.min_width, c.max_width);
        let h = m.height.clamp(c.min_height, c.max_height);
        IntOffset::new((w - m.width) / 2, (h - m.height) / 2)
    }

    /// Moves it alone, what it wraps left where it is (Compose's `placeSelfApparentToRealOffset`, the offset
    /// already added).
    pub(crate) fn place_self_only(&self, position: IntOffset, z_index: f32, layer: Option<LayerBlock>) {
        self.place_self(position, z_index, layer);
    }

    fn place_self(&self, position: IntOffset, z_index: f32, layer: Option<LayerBlock>) {
        self.update_layer_block(layer);
        if self.position.get() != position {
            self.position.set(position);
            self.invalidate_parent_layer();
            if let Some(n) = self.layout_node() {
                n.on_coordinator_rect_changed(self);
                self.invalidate_alignment_lines_from_position_change(&n);
            }
        }
        self.z_index.set(z_index);
        if !self.placing_for_alignment.get() {
            let result = self.measure_result();
            self.capture_rulers_if_needed(result.as_ref());
        }
    }

    /// Compose's `invalidateAlignmentLinesFromPositionChange`: the inner coordinator moving changes its layout
    /// node's lines; another moving changes its parent's.
    fn invalidate_alignment_lines_from_position_change(&self, node: &Rc<LayoutNode>) {
        let wrapped_same_node = self.wrapped().and_then(|w| w.layout_node()).is_some_and(|w| Rc::ptr_eq(&w, node));
        if !wrapped_same_node {
            node.on_alignments_changed();
        } else if let Some(p) = node.parent() {
            p.on_alignments_changed();
        }
    }

    /// Tells its nodes that care that it was placed (Compose's `onPlaced`).
    pub(crate) fn on_placed(&self) {
        if !self.has_node(NodeKind::OnPlaced) || self.checking() {
            return;
        }
        let coordinates = LayoutCoordinates::of(self.rc());
        self.visit_nodes(NodeKind::OnPlaced, &mut |cell| {
            if let Some(aware) = cell.node.borrow_mut().as_layout_aware() {
                aware.on_placed(&coordinates);
            }
        });
    }

    /// Tells them it is no longer placed (Compose's `onUnplaced`).
    pub(crate) fn on_unplaced(&self) {
        self.visit_nodes(NodeKind::Unplaced, &mut |cell| {
            if let Some(aware) = cell.node.borrow_mut().as_unplaced_aware() {
                aware.on_unplaced();
            }
        });
    }

    /// Its layout modifier node left the chain (Compose's `onRelease`).
    pub(crate) fn on_release(&self) {
        self.released.set(true);
        self.reset_provided_rulers();
        self.invalidate_parent_layer();
        self.release_layer();
        if self.position.get() != IntOffset::Zero
            && let Some(n) = self.layout_node()
        {
            n.on_coordinator_rect_changed(self);
        }
    }

    /// Drops its layer (Compose's `releaseLayer`), its layout placed again.
    pub(crate) fn release_layer(&self) {
        if self.layer.borrow().is_some() {
            self.update_layer_block(None);
            if let Some(n) = self.layout_node() {
                n.request_relayout();
            }
        }
    }

    // ---- Layers ----

    /// Sets its layer's block (Compose's `updateLayerBlock`): a layer made, its properties worked out again when
    /// the block is another one, or the layer dropped.
    pub(crate) fn update_layer_block(&self, block: Option<LayerBlock>) {
        let attached = self.layout_node().is_some_and(|n| n.is_attached());
        match block {
            Some(block) if attached => {
                let same = self.layer.borrow().as_ref().is_some_and(|l| Rc::ptr_eq(&l.block, &block));
                if self.layer.borrow().is_none() {
                    *self.layer.borrow_mut() = Some(Layer {
                        block,
                        props: GraphicsLayerScope::default(),
                        matrix: Matrix::new(),
                        inverse: Matrix::new(),
                    });
                    self.update_layer_parameters(true);
                    self.invalidate_parent_layer();
                } else if !same {
                    if let Some(l) = self.layer.borrow_mut().as_mut() {
                        l.block = block;
                    }
                    self.update_layer_parameters(true);
                }
            }
            _ => {
                let had = self.layer.borrow_mut().take();
                *self.layer_reads.borrow_mut() = None;
                if let Some(old) = had {
                    if let Some(n) = self.layout_node() {
                        if old.props.has_positional_properties() {
                            n.on_coordinator_rect_changed(self);
                        }
                        if n.is_attached() && n.is_placed() {
                            n.on_layout_change();
                        }
                    }
                    self.invalidate_parent_layer();
                }
            }
        }
    }

    /// Runs its layer block again, observing what it reads (Compose's `updateLayerParameters`): moved points tell
    /// the layout node, anything else that shows redraws it.
    pub(crate) fn update_layer_parameters(&self, invoke_on_layout_change: bool) {
        let Some(block) = self.layer.borrow().as_ref().map(|l| l.block.clone()) else { return };
        let Some(node) = self.layout_node() else { return };
        let mut scope =
            GraphicsLayerScope::new(self.measured_size.get().to_size(), node.density(), node.layout_direction());
        match node.owner() {
            Some(owner) => {
                let reads = {
                    let mut r = self.layer_reads.borrow_mut();
                    r.get_or_insert_with(|| {
                        let weak = self.this.clone();
                        owner.snapshot_observer().scope(move || {
                            if let Some(c) = weak.upgrade() {
                                c.update_layer_parameters(true);
                            }
                        })
                    })
                    .clone()
                };
                owner.snapshot_observer().observe(&reads, || block(&mut scope));
            }
            None => block(&mut scope),
        }
        let matrix = scope.matrix();
        let mut inverse = matrix;
        inverse.invert();
        let (positional_changed, visible_changed, clip_changed) = {
            let mut layer = self.layer.borrow_mut();
            let Some(layer) = layer.as_mut() else { return };
            let positional = !layer.props.same_positional_properties(&scope);
            let visible = !layer.props.same_as(&scope);
            let clip = layer.props.clip != scope.clip || (scope.clip && *layer.props.shape != *scope.shape);
            layer.props = scope;
            layer.matrix = matrix;
            layer.inverse = inverse;
            (positional, visible, clip)
        };
        if visible_changed {
            self.invalidate_parent_layer();
            node.invalidate_draw();
        }
        if clip_changed {
            node.invalidate_semantics();
        }
        if invoke_on_layout_change && (positional_changed || clip_changed) {
            node.on_layout_change();
        }
        if positional_changed {
            node.on_coordinator_rect_changed(self);
            if node.globally_positioned_observers() > 0
                && let Some(owner) = node.owner()
            {
                owner.request_on_positioned_callback(&node);
            }
        }
    }

    /// The properties its layer has, if it has one.
    pub(crate) fn layer_properties(&self) -> Option<GraphicsLayerScope> {
        self.layer.borrow().as_ref().map(|l| l.props.clone())
    }

    /// What it draws is drawn again (Compose's `invalidateLayer`).
    pub(crate) fn invalidate_layer(&self) {
        if let Some(n) = self.layout_node() {
            n.invalidate_draw();
        }
    }

    /// What it is drawn into is drawn again (its wrapping coordinator's layout node, or its own at the top).
    pub(crate) fn invalidate_parent_layer(&self) {
        match self.wrapped_by().and_then(|w| w.layout_node()) {
            Some(n) => n.invalidate_draw(),
            None => self.invalidate_layer(),
        }
    }

    // ---- Coordinates ----

    /// A point of its in the coordinator wrapping it: through its layer, then moved by its position.
    pub(crate) fn to_parent_position(&self, position: Offset) -> Offset {
        let mapped = match self.layer.borrow().as_ref() {
            Some(l) => l.matrix.map(position),
            None => position,
        };
        mapped + self.position.get().to_offset()
    }

    pub(crate) fn parent_to_local_position(&self, position: Offset) -> Offset {
        let relative = position - self.position.get().to_offset();
        match self.layer.borrow().as_ref() {
            Some(l) => l.inverse.map(relative),
            None => relative,
        }
    }

    pub(crate) fn local_to_root(&self, relative: Offset) -> Offset {
        assert!(self.is_attached(), "kompoz: LayoutCoordinates of a layout that isn't attached");
        self.on_coordinates_used();
        let mut position = relative;
        let mut coordinator = Some(self.rc());
        while let Some(c) = coordinator {
            position = c.to_parent_position(position);
            coordinator = c.wrapped_by();
        }
        position
    }

    /// The window's offset of the host's root (where a host's window puts it).
    pub(crate) fn window_offset(&self) -> Offset {
        self.layout_node().and_then(|n| n.owner()).map_or(Offset::Zero, |o| o.window_offset())
    }

    /// The size of the host's window (its root's size; a popup's coordinates are the window's too).
    pub(crate) fn window_size(&self) -> kompoz_graphics::IntSize {
        let root = self.layout_node().and_then(|n| n.owner()).and_then(|o| o.root());
        match root {
            Some(r) => r.outer_coordinator().measured_size.get(),
            None => self.find_root().measured_size.get(),
        }
    }

    /// The root layout node's outermost coordinator (Compose's `findRootCoordinates`).
    pub(crate) fn find_root(&self) -> Rc<NodeCoordinator> {
        let mut c = self.rc();
        while let Some(up) = c.wrapped_by() {
            c = up;
        }
        c
    }

    /// The coordinates of its layout node's parent: the parent's inner coordinator (Compose's
    /// `parentLayoutCoordinates`).
    pub(crate) fn parent_layout_coordinator(&self) -> Option<Rc<NodeCoordinator>> {
        assert!(self.is_attached(), "kompoz: LayoutCoordinates of a layout that isn't attached");
        self.on_coordinates_used();
        self.node().outer_coordinator().wrapped_by()
    }

    pub(crate) fn local_position_of(&self, source: &Rc<NodeCoordinator>, relative: Offset) -> Offset {
        source.on_coordinates_used();
        let common = self.find_common_ancestor(source);
        let mut position = relative;
        let mut coordinator = source.clone();
        while !Rc::ptr_eq(&coordinator, &common) {
            position = coordinator.to_parent_position(position);
            coordinator = coordinator.wrapped_by().expect("coordinators of one tree");
        }
        self.ancestor_to_local(&common, position)
    }

    fn ancestor_to_local(&self, ancestor: &Rc<NodeCoordinator>, offset: Offset) -> Offset {
        if std::ptr::eq(self, &**ancestor) {
            return offset;
        }
        match self.wrapped_by() {
            Some(w) if !Rc::ptr_eq(&w, ancestor) => {
                self.parent_to_local_position(w.ancestor_to_local(ancestor, offset))
            }
            _ => self.parent_to_local_position(offset),
        }
    }

    pub(crate) fn local_bounding_box_of(&self, source: &Rc<NodeCoordinator>, clip_bounds: bool) -> Rect {
        assert!(self.is_attached(), "kompoz: LayoutCoordinates of a layout that isn't attached");
        assert!(source.is_attached(), "kompoz: LayoutCoordinates of a layout that isn't attached");
        source.on_coordinates_used();
        let common = self.find_common_ancestor(source);
        let size = source.measured_size.get();
        let mut bounds = Rect::new(0.0, 0.0, size.width as f32, size.height as f32);
        let mut coordinator = source.clone();
        while !Rc::ptr_eq(&coordinator, &common) {
            bounds = coordinator.rect_in_parent(bounds, clip_bounds);
            if bounds.is_empty() {
                return Rect::Zero;
            }
            coordinator = coordinator.wrapped_by().expect("coordinators of one tree");
        }
        self.ancestor_rect_to_local(&common, bounds, clip_bounds)
    }

    fn ancestor_rect_to_local(&self, ancestor: &Rc<NodeCoordinator>, rect: Rect, clip_bounds: bool) -> Rect {
        if std::ptr::eq(self, &**ancestor) {
            return rect;
        }
        let rect = match self.wrapped_by() {
            Some(w) => w.ancestor_rect_to_local(ancestor, rect, clip_bounds),
            None => rect,
        };
        self.parent_to_local_rect(rect, clip_bounds)
    }

    /// Its bounds `bounds` (its own coordinates) in the coordinator wrapping it (Compose's `rectInParent`).
    pub(crate) fn rect_in_parent(&self, bounds: Rect, clip_bounds: bool) -> Rect {
        let mut bounds = bounds;
        if let Some(l) = self.layer.borrow().as_ref() {
            if l.props.clip && clip_bounds {
                let size = self.measured_size.get();
                bounds = bounds.intersect(Rect::new(0.0, 0.0, size.width as f32, size.height as f32));
                if bounds.is_empty() {
                    return bounds;
                }
            }
            bounds = l.matrix.map_rect(bounds);
        }
        bounds.translate(self.position.get().to_offset())
    }

    fn parent_to_local_rect(&self, bounds: Rect, clip_bounds: bool) -> Rect {
        let mut bounds = bounds.translate(-self.position.get().to_offset());
        if let Some(l) = self.layer.borrow().as_ref() {
            bounds = l.inverse.map_rect(bounds);
            if l.props.clip && clip_bounds {
                let size = self.measured_size.get();
                bounds = bounds.intersect(Rect::new(0.0, 0.0, size.width as f32, size.height as f32));
            }
        }
        bounds
    }

    /// Sets `matrix` to the transform from `source` to this (Compose's `transformFrom`).
    pub(crate) fn transform_from(&self, source: &Rc<NodeCoordinator>, matrix: &mut Matrix) {
        source.on_coordinates_used();
        let common = self.find_common_ancestor(source);
        matrix.reset();
        source.transform_to_ancestor(&common, matrix);
        self.transform_from_ancestor(&common, matrix);
    }

    fn transform_to_ancestor(&self, ancestor: &Rc<NodeCoordinator>, matrix: &mut Matrix) {
        let mut c = self.rc();
        while !Rc::ptr_eq(&c, ancestor) {
            if let Some(l) = c.layer.borrow().as_ref() {
                matrix.times_assign(&l.matrix);
            }
            let position = c.position.get();
            if position != IntOffset::Zero {
                let mut t = Matrix::new();
                t.translate(position.x as f32, position.y as f32, 0.0);
                matrix.times_assign(&t);
            }
            c = c.wrapped_by().expect("coordinators of one tree");
        }
    }

    fn transform_from_ancestor(&self, ancestor: &Rc<NodeCoordinator>, matrix: &mut Matrix) {
        if std::ptr::eq(self, &**ancestor) {
            return;
        }
        if let Some(w) = self.wrapped_by() {
            w.transform_from_ancestor(ancestor, matrix);
        }
        let position = self.position.get();
        if position != IntOffset::Zero {
            let mut t = Matrix::new();
            t.translate(-position.x as f32, -position.y as f32, 0.0);
            matrix.times_assign(&t);
        }
        if let Some(l) = self.layer.borrow().as_ref() {
            matrix.times_assign(&l.inverse);
        }
    }

    /// The nearest coordinator wrapping both (Compose's `findCommonAncestor`).
    pub(crate) fn find_common_ancestor(&self, other: &Rc<NodeCoordinator>) -> Rc<NodeCoordinator> {
        let me = self.rc();
        let mut a1 = other.node();
        let mut a2 = self.node();
        if Rc::ptr_eq(&a1, &a2) {
            // On the same layout node: the outer of the two.
            let mut c = Some(me.clone());
            while let Some(x) = c {
                if Rc::ptr_eq(&x, other) {
                    return other.clone();
                }
                if x.layout_node().is_none_or(|n| !Rc::ptr_eq(&n, &a2)) {
                    break;
                }
                c = x.wrapped_by();
            }
            return me;
        }
        while a1.depth() > a2.depth() {
            a1 = a1.parent().expect("layouts of one hierarchy");
        }
        while a2.depth() > a1.depth() {
            a2 = a2.parent().expect("layouts of one hierarchy");
        }
        while !Rc::ptr_eq(&a1, &a2) {
            match (a1.parent(), a2.parent()) {
                (Some(p1), Some(p2)) => {
                    a1 = p1;
                    a2 = p2;
                }
                _ => panic!("kompoz: layouts are not part of the same hierarchy"),
            }
        }
        if Rc::ptr_eq(&a2, &self.node()) {
            me
        } else if Rc::ptr_eq(&a1, &other.node()) {
            other.clone()
        } else {
            a1.inner.clone()
        }
    }

    // ---- Alignment lines ----

    /// Where `line` is in it, from its real top left (Compose's LookaheadCapablePlaceable `get`).
    pub(crate) fn alignment_line(&self, line: AlignmentLine) -> i32 {
        if self.measure_result.borrow().is_none() {
            return AlignmentLine::Unspecified;
        }
        let measured = self.calculate_alignment_line(line);
        if measured == AlignmentLine::Unspecified {
            return measured;
        }
        let placeable_constraints = self.measurement_constraints.get();
        let size = self.measured_size.get();
        let width = size.width.clamp(placeable_constraints.min_width, placeable_constraints.max_width);
        let height = size.height.clamp(placeable_constraints.min_height, placeable_constraints.max_height);
        let apparent = if line.is_horizontal() { (height - size.height) / 2 } else { (width - size.width) / 2 };
        measured + apparent
    }

    fn calculate_alignment_line(&self, line: AlignmentLine) -> i32 {
        match &self.kind {
            CoordinatorKind::Inner => {
                let n = self.node();
                n.calculate_alignment_lines();
                n.alignment_lines.get(line).unwrap_or(AlignmentLine::Unspecified)
            }
            CoordinatorKind::Modifier { .. } => {
                // Compose's `calculateAlignmentAndPlaceChildAsNeeded`.
                if let Some(v) = self.measure_result.borrow().as_ref().and_then(|r| r.alignment_line(line)) {
                    return v;
                }
                let child = self.wrapped().expect("a layout modifier coordinator wraps another");
                let in_wrapped = child.alignment_line(line);
                if in_wrapped == AlignmentLine::Unspecified {
                    return AlignmentLine::Unspecified;
                }
                let was_shallow = child.shallow_placing.replace(true);
                let was_for_alignment = self.placing_for_alignment.replace(true);
                self.replace();
                child.shallow_placing.set(was_shallow);
                self.placing_for_alignment.set(was_for_alignment);
                let p = child.position.get();
                in_wrapped + if line.is_horizontal() { p.y } else { p.x }
            }
        }
    }

    pub(crate) fn provided_alignment_lines(&self) -> Vec<AlignmentLine> {
        let mut out = Vec::new();
        let mut c = Some(self.rc());
        while let Some(x) = c {
            for l in x.measure_result_lines() {
                if !out.contains(&l) {
                    out.push(l);
                }
            }
            c = x.wrapped();
        }
        out
    }

    /// What the chain's parent data nodes say, folded tail to head (Compose's `parentData`).
    pub(crate) fn parent_data(&self) -> Option<Rc<dyn Any>> {
        self.layout_node().and_then(|n| n.parent_data())
    }
}

impl IntrinsicMeasurable for NodeCoordinator {
    fn parent_data(&self) -> Option<Rc<dyn Any>> {
        NodeCoordinator::parent_data(self)
    }
    fn min_intrinsic_width(&self, height: i32) -> i32 {
        NodeCoordinator::min_intrinsic_width(self, height)
    }
    fn max_intrinsic_width(&self, height: i32) -> i32 {
        NodeCoordinator::max_intrinsic_width(self, height)
    }
    fn min_intrinsic_height(&self, width: i32) -> i32 {
        NodeCoordinator::min_intrinsic_height(self, width)
    }
    fn max_intrinsic_height(&self, width: i32) -> i32 {
        NodeCoordinator::max_intrinsic_height(self, width)
    }
}

impl Measurable for NodeCoordinator {
    fn measure(&self, constraints: Constraints) -> Placeable {
        match &self.kind {
            // What a layout modifier wraps last: the layout node itself, measured as its own measure pass does.
            CoordinatorKind::Inner => self.perform_measure(constraints),
            CoordinatorKind::Modifier { .. } => self.perform_measure(constraints),
        }
        Placeable::new(self.rc())
    }

    fn retain(&self) -> Option<Rc<dyn Measurable>> {
        Some(self.rc())
    }
}

impl PlaceTarget for NodeCoordinator {
    fn measured_size(&self) -> IntSize {
        self.measured_size.get()
    }
    fn measurement_constraints(&self) -> Constraints {
        self.measurement_constraints.get()
    }
    fn alignment_line(&self, line: AlignmentLine) -> i32 {
        NodeCoordinator::alignment_line(self, line)
    }
    fn place_at(&self, position: IntOffset, z_index: f32, layer: Option<LayerBlock>) {
        NodeCoordinator::place_at(self, position, z_index, layer);
    }
    fn parent_data(&self) -> Option<Rc<dyn Any>> {
        NodeCoordinator::parent_data(self)
    }
}
