//! Lookahead, as Compose's `LookaheadScope.kt`, `ApproachLayoutModifierNode.kt` and `ApproachMeasureScope.kt`:
//! [`LookaheadScope`] (a scope in which every layout is laid out first where it is going to be, in a lookahead pass,
//! then on its way there), [`ApproachLayoutModifierNode`] and `Modifier.approach_layout` (what measures and places on
//! the way, knowing the destination: [`ApproachMeasureScope::lookahead_size`], lookahead coordinates).

use super::{
    DefaultIntrinsicMeasurable, IntrinsicMeasurable, LayoutCoordinates, LayoutModifierNode, Measurable, MeasureResult,
    MeasureScope, MinMax, PlacementScope, Side, VirtualLayoutNode,
};
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement};
use crate::node::NodeCoordinator;
use crate::node_capabilities;
use crate::unit::Constraints;
use kompoz_graphics::{IntSize, Offset};
use kompoz_runtime::{emit, kompozable, named, remember};
use std::cell::RefCell;
use std::rc::{Rc, Weak};

// ---------------------------------------------------------------------------------------------------------------
// LookaheadScope

/// What the layouts of a [`LookaheadScope`] can ask of it (Compose's `LookaheadScope` interface): their coordinates
/// in the lookahead pass, and the scope's coordinates while placing. Its content gets one; an
/// [`ApproachMeasureScope`] is one too.
pub trait LookaheadScope {
    /// `coordinates` as the lookahead pass laid them out (Compose's `toLookaheadCoordinates`): lookahead
    /// coordinates (the same if they are already), or the coordinates themselves for a layout out of any lookahead.
    fn to_lookahead_coordinates(&self, coordinates: &LayoutCoordinates) -> LayoutCoordinates;

    /// The coordinates of the scope (Compose's `PlacementScope.lookaheadScopeCoordinates`), from a placement scope
    /// (while placing): not lookahead coordinates — [`to_lookahead_coordinates`](Self::to_lookahead_coordinates)
    /// gives those.
    fn lookahead_scope_coordinates(&self, placement_scope: &PlacementScope) -> LayoutCoordinates;

    /// A point of `source_coordinates`'s, both taken as the lookahead pass laid them out, in `coordinates` (Compose's
    /// `LayoutCoordinates.localLookaheadPositionOf`): their lookahead coordinates' `local_position_of`, the offsets
    /// of what was placed under a motion frame of reference left out unless `include_motion_frame_of_reference`.
    fn local_lookahead_position_of(
        &self,
        coordinates: &LayoutCoordinates,
        source_coordinates: &LayoutCoordinates,
        relative_to_source: Offset,
        include_motion_frame_of_reference: bool,
    ) -> Offset {
        let lookahead = self.to_lookahead_coordinates(coordinates);
        let source = self.to_lookahead_coordinates(source_coordinates);
        if lookahead.is_lookahead() {
            lookahead.local_position_of_with(&source, relative_to_source, include_motion_frame_of_reference)
        } else if source.is_lookahead() {
            -source.local_position_of_with(&lookahead, relative_to_source, include_motion_frame_of_reference)
        } else {
            // (Compose 1.12 maps the lookahead coordinates in themselves here, `relative_to_source` as it is; this
            // maps the source's, as its other cases do)
            lookahead.local_position_of_with(&source, relative_to_source, include_motion_frame_of_reference)
        }
    }

    /// The coordinates of the scope, from the coordinates of a layout in it (Compose's
    /// `LookaheadScope.lookaheadScopeCoordinates(sourceCoordinates)`): from the placement scope of that layout.
    fn lookahead_scope_coordinates_of(&self, source_coordinates: &LayoutCoordinates) -> LayoutCoordinates {
        let scope = if source_coordinates.is_lookahead() {
            source_coordinates.coordinator.lookahead_placement_scope()
        } else {
            source_coordinates.coordinator.placement_scope()
        };
        self.lookahead_scope_coordinates(&scope)
    }
}

/// `coordinates`' lookahead coordinates, or themselves out of a lookahead (Compose's `toLookaheadCoordinates` of
/// LookaheadScopeImpl and ApproachMeasureScopeImpl).
fn lookahead_coordinates_of(coordinates: &LayoutCoordinates) -> LayoutCoordinates {
    coordinates.to_lookahead_coordinates()
}

/// A `LookaheadScope`'s scope (Compose's LookaheadScopeImpl): its virtual node, whose parent's coordinates are the
/// scope's.
#[derive(Default)]
pub(crate) struct LookaheadScopeImpl {
    node: RefCell<Weak<VirtualLayoutNode>>,
}

impl LookaheadScope for LookaheadScopeImpl {
    fn to_lookahead_coordinates(&self, coordinates: &LayoutCoordinates) -> LayoutCoordinates {
        lookahead_coordinates_of(coordinates)
    }

    fn lookahead_scope_coordinates(&self, _placement_scope: &PlacementScope) -> LayoutCoordinates {
        let node = self.node.borrow().upgrade().expect("kompoz: the coordinates of a LookaheadScope that left");
        let parent = node.parent().expect("kompoz: the coordinates of a LookaheadScope not in a layout");
        LayoutCoordinates::of(parent.inner.clone())
    }
}

/// A scope in which every layout is laid out first where it is going to be, then on its way there (Compose's
/// `LookaheadScope`): a lookahead pass measures and places all of `content`'s layouts as their measure policies and
/// layout modifiers say — an approach node (`Modifier.approach_layout`, an [`ApproachLayoutModifierNode`]) by its
/// plain `measure` — and only then the main pass, the approach, measures and places them, the approach nodes seeing
/// where their layouts go ([`ApproachMeasureScope::lookahead_size`], lookahead coordinates) and approaching it.
/// `content` gets the scope (for its layouts' lookahead coordinates). It adds no layout: its layouts are its parent's
/// children, as they would be without it.
///
/// Kotlin: `LookaheadScope(content: @Composable LookaheadScope.() -> Unit)`
#[kompozable]
pub fn LookaheadScope(content: impl Fn(&Rc<dyn LookaheadScope>) + 'static) {
    let scope = remember(|| Rc::new(LookaheadScopeImpl::default()));
    let handle: Rc<dyn LookaheadScope> = scope.clone();
    emit(
        VirtualLayoutNode::new,
        move |node: &VirtualLayoutNode| {
            // (Compose's `init { isVirtualLookaheadRoot = true }` and its `scopeCoordinates`)
            *scope.node.borrow_mut() = node.weak();
            if node.lookahead_scope.borrow().is_none() {
                *node.lookahead_scope.borrow_mut() = Some(scope.clone());
            }
        },
        move || content(&handle),
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Approach

/// What an approach node measures in (Compose's `ApproachMeasureScope`, and `ApproachIntrinsicMeasureScope`, the same
/// type here): a measure scope (it derefs to one: `scope.layout(…)`) of the approach pass — never looking ahead —
/// that knows where its layout is going: its [`lookahead_size`](Self::lookahead_size) and
/// [`lookahead_constraints`](Self::lookahead_constraints). It is a [`LookaheadScope`] too.
#[derive(Clone)]
pub struct ApproachMeasureScope {
    scope: MeasureScope,
    coordinator: Rc<NodeCoordinator>,
}

/// Compose's `ApproachIntrinsicMeasureScope`: the same as an approach measure scope here.
pub type ApproachIntrinsicMeasureScope = ApproachMeasureScope;

impl std::ops::Deref for ApproachMeasureScope {
    type Target = MeasureScope;
    fn deref(&self) -> &MeasureScope {
        &self.scope
    }
}

impl ApproachMeasureScope {
    pub(crate) fn new(scope: MeasureScope, coordinator: Rc<NodeCoordinator>) -> ApproachMeasureScope {
        ApproachMeasureScope { scope, coordinator }
    }

    /// The constraints the layout was measured with in the lookahead pass (Compose's `lookaheadConstraints`).
    pub fn lookahead_constraints(&self) -> Constraints {
        self.coordinator
            .lookahead_constraints
            .get()
            .expect("Error: Lookahead constraints requested before lookahead measure.")
    }

    /// The size the approach node measured to in the lookahead pass, with
    /// [`lookahead_constraints`](Self::lookahead_constraints) (Compose's `lookaheadSize`): where its approach is
    /// going.
    pub fn lookahead_size(&self) -> IntSize {
        let delegate = self.coordinator.lookahead_delegate().expect("kompoz: an approach node outside of a lookahead");
        delegate.measure_result().map_or(IntSize::Zero, |r| IntSize::new(r.width, r.height))
    }

    /// The measure scope it is.
    pub fn measure_scope(&self) -> &MeasureScope {
        &self.scope
    }

    /// Whether the approach node was measured in lookahead already (the approach intrinsics measure only then).
    fn has_lookahead_measure_result(&self) -> bool {
        self.coordinator.lookahead_delegate().is_some_and(|d| d.has_measure_result())
    }

    /// The scope an intrinsic measurement of the approach measures in (Compose's ApproachIntrinsicsMeasureScope).
    fn for_intrinsics(&self) -> ApproachMeasureScope {
        ApproachMeasureScope { scope: self.scope.for_intrinsics(), coordinator: self.coordinator.clone() }
    }
}

impl LookaheadScope for ApproachMeasureScope {
    fn to_lookahead_coordinates(&self, coordinates: &LayoutCoordinates) -> LayoutCoordinates {
        lookahead_coordinates_of(coordinates)
    }

    fn lookahead_scope_coordinates(&self, _placement_scope: &PlacementScope) -> LayoutCoordinates {
        let node = self.coordinator.layout_node().expect("kompoz: an approach node of a layout that is gone");
        let root = node.lookahead_root.borrow().clone();
        let root = root
            .expect("Error: Requesting LookaheadScopeCoordinates is not permitted from outside of a LookaheadScope.");
        let coordinator = root.scope_coordinator().expect("kompoz: the lookahead root of an approach node left");
        LayoutCoordinates::of(coordinator)
    }
}

/// A layout modifier node that approaches, in the approach pass, the size and position its layout has in the
/// lookahead pass (Compose's `ApproachLayoutModifierNode`): its layout node is laid out in lookahead first (in a
/// [`LookaheadScope`], or as a lookahead root of its own outside one), where it measures with its plain
/// [`LayoutModifierNode::measure`] (Compose's default: what it wraps measured and placed at its top left —
/// [`approach_lookahead_measure`]); then, in the approach pass,
/// [`is_measurement_approach_in_progress`](Self::is_measurement_approach_in_progress) is asked with the lookahead
/// size, and [`approach_measure`](Self::approach_measure) measures — with what it wraps measured at its lookahead
/// constraints once the approach is complete. Placed, it is asked
/// [`is_placement_approach_in_progress`](Self::is_placement_approach_in_progress) with its lookahead coordinates;
/// once neither approach is in progress, what it wraps is placed where it is in lookahead.
pub trait ApproachLayoutModifierNode: LayoutModifierNode {
    /// Whether its measurement is still on its way to `lookahead_size` (Compose's
    /// `isMeasurementApproachInProgress`), asked before [`approach_measure`](Self::approach_measure).
    fn is_measurement_approach_in_progress(&mut self, lookahead_size: IntSize) -> bool;

    /// Whether its placement is still on its way to where `lookahead_coordinates` are (Compose's
    /// `PlacementScope.isPlacementApproachInProgress`), asked as it is placed. False by default.
    fn is_placement_approach_in_progress(
        &mut self,
        scope: &PlacementScope,
        lookahead_coordinates: &LayoutCoordinates,
    ) -> bool {
        let _ = (scope, lookahead_coordinates);
        false
    }

    /// Measures and places what it wraps on the way to its lookahead size and position (Compose's
    /// `ApproachMeasureScope.approachMeasure`): `scope` knows the lookahead size; placing, its layout's coordinates
    /// against its lookahead coordinates tell where it is going.
    fn approach_measure(
        &mut self,
        scope: &ApproachMeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult;

    /// Its min intrinsic width in the approach (Compose's `minApproachIntrinsicWidth`): measured by its approach
    /// with what it wraps answering with its intrinsic size, once it was measured in lookahead; before, what it
    /// wraps' own.
    fn min_approach_intrinsic_width(
        &mut self,
        scope: &ApproachIntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        height: i32,
    ) -> i32 {
        approach_measuring_intrinsic(self, scope, measurable, MinMax::Min, Side::Width, height)
    }

    fn min_approach_intrinsic_height(
        &mut self,
        scope: &ApproachIntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        width: i32,
    ) -> i32 {
        approach_measuring_intrinsic(self, scope, measurable, MinMax::Min, Side::Height, width)
    }

    fn max_approach_intrinsic_width(
        &mut self,
        scope: &ApproachIntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        height: i32,
    ) -> i32 {
        approach_measuring_intrinsic(self, scope, measurable, MinMax::Max, Side::Width, height)
    }

    fn max_approach_intrinsic_height(
        &mut self,
        scope: &ApproachIntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        width: i32,
    ) -> i32 {
        approach_measuring_intrinsic(self, scope, measurable, MinMax::Max, Side::Height, width)
    }
}

/// What an approach node's lookahead `measure` is unless it says otherwise (Compose's default
/// `ApproachLayoutModifierNode.measure`): what it wraps measured within `constraints`, placed at its top left.
pub fn approach_lookahead_measure(
    scope: &MeasureScope,
    measurable: &dyn Measurable,
    constraints: Constraints,
) -> MeasureResult {
    let placeable = measurable.measure(constraints);
    let (w, h) = (placeable.width(), placeable.height());
    scope.layout(w, h, move |p: &PlacementScope| p.place(&placeable, 0, 0))
}

/// Compose's approach intrinsics (`NodeMeasuringIntrinsics` with an `ApproachMeasureBlock`).
fn approach_measuring_intrinsic<N: ApproachLayoutModifierNode + ?Sized>(
    node: &mut N,
    scope: &ApproachIntrinsicMeasureScope,
    measurable: &dyn IntrinsicMeasurable,
    min_max: MinMax,
    side: Side,
    size: i32,
) -> i32 {
    if !scope.has_lookahead_measure_result() {
        // (before its lookahead measure, its lookahead size and constraints aren't known: what it wraps' own)
        return match (min_max, side) {
            (MinMax::Min, Side::Width) => measurable.min_intrinsic_width(size),
            (MinMax::Max, Side::Width) => measurable.max_intrinsic_width(size),
            (MinMax::Min, Side::Height) => measurable.min_intrinsic_height(size),
            (MinMax::Max, Side::Height) => measurable.max_intrinsic_height(size),
        };
    }
    let wrapped = DefaultIntrinsicMeasurable { measurable, min_max, side };
    let constraints = match side {
        Side::Width => Constraints::new(0, Constraints::Infinity, 0, size),
        Side::Height => Constraints::new(0, size, 0, Constraints::Infinity),
    };
    let result = node.approach_measure(&scope.for_intrinsics(), &wrapped, constraints);
    match side {
        Side::Width => result.width,
        Side::Height => result.height,
    }
}

/// `Modifier.approach_layout`'s placement approach: whether it is still in progress, given the placement scope and
/// the lookahead coordinates.
pub type PlacementApproachInProgress = Rc<dyn Fn(&PlacementScope, &LayoutCoordinates) -> bool>;

/// `Modifier.approach_layout`'s measure block.
pub type ApproachMeasureBlock = Rc<dyn Fn(&ApproachMeasureScope, &dyn Measurable, Constraints) -> MeasureResult>;

#[named]
impl Modifier {
    /// An approach layout (Compose's `Modifier.approachLayout`): in a [`LookaheadScope`] (or as a lookahead root of
    /// its own outside one), its layout is measured in lookahead first as if it weren't there; then
    /// `is_measurement_approach_in_progress` is asked with the lookahead size, and `approach_measure` measures and
    /// places what it wraps on the way there (its scope's `lookahead_size`; placing, its coordinates against its
    /// lookahead coordinates). `is_placement_approach_in_progress` (none: false) says whether its placement is still
    /// on its way; once both are done, what it wraps is measured and placed as in lookahead.
    ///
    /// Kotlin: `approachLayout(isMeasurementApproachInProgress: (lookaheadSize: IntSize) -> Boolean, isPlacementApproachInProgress: Placeable.PlacementScope.(lookaheadCoordinates: LayoutCoordinates) -> Boolean = defaultPlacementApproachInProgress, approachMeasure: ApproachMeasureScope.(measurable: Measurable, constraints: Constraints) -> MeasureResult)`
    pub fn approach_layout(
        self,
        is_measurement_approach_in_progress: impl Fn(IntSize) -> bool + 'static,
        #[default(None)] is_placement_approach_in_progress: Option<PlacementApproachInProgress>,
        approach_measure: impl Fn(&ApproachMeasureScope, &dyn Measurable, Constraints) -> MeasureResult + 'static,
    ) -> Modifier {
        self.then_element(ApproachLayoutElement {
            approach_measure: Rc::new(approach_measure),
            is_measurement_approach_in_progress: Rc::new(is_measurement_approach_in_progress),
            is_placement_approach_in_progress,
        })
    }
}

/// `Modifier.approach_layout`'s element (Compose's ApproachLayoutElement): equal while its three blocks are the same
/// instances.
struct ApproachLayoutElement {
    approach_measure: ApproachMeasureBlock,
    is_measurement_approach_in_progress: Rc<dyn Fn(IntSize) -> bool>,
    is_placement_approach_in_progress: Option<PlacementApproachInProgress>,
}

impl PartialEq for ApproachLayoutElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.approach_measure, &other.approach_measure)
            && Rc::ptr_eq(&self.is_measurement_approach_in_progress, &other.is_measurement_approach_in_progress)
            && match (&self.is_placement_approach_in_progress, &other.is_placement_approach_in_progress) {
                (None, None) => true,
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                _ => false,
            }
    }
}

impl ModifierNodeElement for ApproachLayoutElement {
    type Node = ApproachLayoutModifierNodeImpl;
    fn create(&self) -> ApproachLayoutModifierNodeImpl {
        ApproachLayoutModifierNodeImpl {
            measure_block: self.approach_measure.clone(),
            is_measurement_approach_in_progress: self.is_measurement_approach_in_progress.clone(),
            is_placement_approach_in_progress: self.is_placement_approach_in_progress.clone(),
        }
    }
    fn update(&self, node: &mut ApproachLayoutModifierNodeImpl) {
        node.measure_block = self.approach_measure.clone();
        node.is_measurement_approach_in_progress = self.is_measurement_approach_in_progress.clone();
        node.is_placement_approach_in_progress = self.is_placement_approach_in_progress.clone();
    }
    fn name(&self) -> &'static str {
        "approachLayout"
    }
}

/// `Modifier.approach_layout`'s node (Compose's ApproachLayoutModifierNodeImpl).
pub struct ApproachLayoutModifierNodeImpl {
    measure_block: ApproachMeasureBlock,
    is_measurement_approach_in_progress: Rc<dyn Fn(IntSize) -> bool>,
    is_placement_approach_in_progress: Option<PlacementApproachInProgress>,
}

impl ModifierNode for ApproachLayoutModifierNodeImpl {
    node_capabilities!(layout, approach_layout);
}

impl LayoutModifierNode for ApproachLayoutModifierNodeImpl {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        approach_lookahead_measure(scope, measurable, constraints)
    }
}

impl ApproachLayoutModifierNode for ApproachLayoutModifierNodeImpl {
    fn is_measurement_approach_in_progress(&mut self, lookahead_size: IntSize) -> bool {
        (self.is_measurement_approach_in_progress)(lookahead_size)
    }

    fn is_placement_approach_in_progress(
        &mut self,
        scope: &PlacementScope,
        lookahead_coordinates: &LayoutCoordinates,
    ) -> bool {
        match &self.is_placement_approach_in_progress {
            Some(f) => f(scope, lookahead_coordinates),
            None => false,
        }
    }

    fn approach_measure(
        &mut self,
        scope: &ApproachMeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        (self.measure_block)(scope, measurable, constraints)
    }
}
