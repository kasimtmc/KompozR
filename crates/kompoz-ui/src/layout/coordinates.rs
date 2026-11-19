//! Where a layout is (Compose's LayoutCoordinates): its size, where it is in its parent, the root and the window,
//! and maps between it and other layouts, through their layers' matrices. In a lookahead scope, a layout also has
//! lookahead coordinates (Compose's LookaheadLayoutCoordinates): its size and position as the lookahead pass laid it
//! out, where it is going to be.

use super::AlignmentLine;
use crate::node::NodeCoordinator;
use kompoz_graphics::{IntSize, Matrix, Offset, Rect};
use kompoz_runtime::named;
use std::fmt;
use std::rc::Rc;

/// The coordinates of a layout (one of its coordinators: a layout node's outer one, or one of a layout modifier,
/// or its inner one), as Compose's LayoutCoordinates. A cheap handle; it reads where the layout is when asked.
/// Lookahead coordinates ([`is_lookahead`](Self::is_lookahead)) read the coordinator's lookahead pass instead
/// (Compose's LookaheadLayoutCoordinates).
#[derive(Clone)]
pub struct LayoutCoordinates {
    pub(crate) coordinator: Rc<NodeCoordinator>,
    /// Lookahead coordinates (Compose's LookaheadLayoutCoordinates).
    pub(crate) lookahead: bool,
}

impl PartialEq for LayoutCoordinates {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.coordinator, &other.coordinator) && self.lookahead == other.lookahead
    }
}

impl fmt::Debug for LayoutCoordinates {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let kind = if self.lookahead { "LookaheadLayoutCoordinates" } else { "LayoutCoordinates" };
        write!(f, "{kind}({:?} at {:?})", self.size(), self.position_in_root())
    }
}

impl LayoutCoordinates {
    pub(crate) fn of(coordinator: Rc<NodeCoordinator>) -> LayoutCoordinates {
        LayoutCoordinates { coordinator, lookahead: false }
    }

    /// The lookahead coordinates of `coordinator` (its lookahead delegate's: Compose's `lookaheadLayoutCoordinates`).
    pub(crate) fn lookahead_of(coordinator: Rc<NodeCoordinator>) -> LayoutCoordinates {
        LayoutCoordinates { coordinator, lookahead: true }
    }

    /// Whether these are lookahead coordinates (Compose's `is LookaheadLayoutCoordinates`): where the layout is in
    /// the lookahead pass.
    pub fn is_lookahead(&self) -> bool {
        self.lookahead
    }

    fn lookahead_delegate(&self) -> Rc<crate::node::LookaheadDelegate> {
        self.coordinator
            .lookahead_delegate()
            .expect("kompoz: lookahead coordinates of a layout out of a lookahead scope")
    }

    /// Its measured size (in lookahead, its lookahead size).
    pub fn size(&self) -> IntSize {
        if self.lookahead {
            return self.lookahead_delegate().size();
        }
        self.coordinator.measured_size.get()
    }

    /// Whether its layout is in a host (asking for the rest of a detached one panics, as in Compose).
    pub fn is_attached(&self) -> bool {
        self.coordinator.is_attached()
    }

    /// Whether it was placed under a motion frame of reference (Compose's `introducesMotionFrameOfReference`: its
    /// parent placed it in `with_motion_frame_of_reference_placement`): its offset is left out by
    /// [`local_position_of_with`](Self::local_position_of_with) when `include_motion_frame_of_reference` is false.
    pub fn introduces_motion_frame_of_reference(&self) -> bool {
        if self.lookahead {
            return self.lookahead_delegate().placed_under_motion_frame_of_reference.get();
        }
        self.coordinator.placed_under_motion_frame_of_reference.get()
    }

    /// The coordinates of the layout node it is in's parent (its inner coordinator), none at the root (Compose's
    /// `parentLayoutCoordinates`).
    pub fn parent_layout_coordinates(&self) -> Option<LayoutCoordinates> {
        if self.lookahead {
            assert!(self.is_attached(), "kompoz: LayoutCoordinates of a layout that isn't attached");
            let node = self.coordinator.layout_node()?;
            let parent = node.outer_coordinator().wrapped_by()?;
            parent.lookahead_delegate()?;
            return Some(LayoutCoordinates::lookahead_of(parent));
        }
        self.coordinator.parent_layout_coordinator().map(LayoutCoordinates::of)
    }

    /// The coordinates wrapping these: the next layout modifier out, or the parent's inner coordinator (Compose's
    /// `parentCoordinates`).
    pub fn parent_coordinates(&self) -> Option<LayoutCoordinates> {
        if self.lookahead {
            assert!(self.is_attached(), "kompoz: LayoutCoordinates of a layout that isn't attached");
            let parent = self.coordinator.wrapped_by()?;
            parent.lookahead_delegate()?;
            return Some(LayoutCoordinates::lookahead_of(parent));
        }
        self.coordinator.on_coordinates_used();
        self.coordinator.wrapped_by().map(LayoutCoordinates::of)
    }

    /// Its top left in the root's coordinates.
    pub fn position_in_root(&self) -> Offset {
        self.local_to_root(Offset::Zero)
    }

    /// Its top left in the window's.
    pub fn position_in_window(&self) -> Offset {
        self.local_to_window(Offset::Zero)
    }

    /// Its top left in its parent layout's coordinates (Compose's `positionInParent`).
    pub fn position_in_parent(&self) -> Offset {
        match self.parent_layout_coordinates() {
            Some(p) => p.local_position_of(self, Offset::Zero),
            None => Offset::Zero,
        }
    }

    /// Its bounds in its parent layout's coordinates (Compose's `boundsInParent`).
    pub fn bounds_in_parent(&self) -> Rect {
        match self.parent_layout_coordinates() {
            Some(p) => p.local_bounding_box_of(self, true),
            None => Rect::from_offset_size(Offset::Zero, self.size().to_size()),
        }
    }

    /// Its bounds in the root's coordinates, cut by the clipping layers above it (Compose's `boundsInRoot`). A
    /// popup's root is placed in the host's window: its position counts (as it does in `position_in_root`).
    pub fn bounds_in_root(&self) -> Rect {
        let root = self.find_root_coordinates();
        root.local_bounding_box_of(self, true).translate(root.position_in_root())
    }

    /// Its bounds in the window's coordinates, also cut by the window (Compose's `boundsInWindow`).
    pub fn bounds_in_window(&self) -> Rect {
        let root = self.find_root_coordinates();
        let bounds = root.local_bounding_box_of(self, true).translate(root.position_in_root());
        let window = self.coordinator.window_size().to_size();
        let clipped = Rect::new(
            bounds.left.clamp(0.0, window.width),
            bounds.top.clamp(0.0, window.height),
            bounds.right.clamp(0.0, window.width),
            bounds.bottom.clamp(0.0, window.height),
        );
        if clipped.width() == 0.0 || clipped.height() == 0.0 {
            return Rect::Zero;
        }
        let offset = self.coordinator.window_offset();
        clipped.translate(offset)
    }

    /// The coordinates of the root (Compose's `findRootCoordinates`): the root layout node's outermost.
    pub fn find_root_coordinates(&self) -> LayoutCoordinates {
        LayoutCoordinates::of(self.coordinator.find_root())
    }

    /// How far a point of its main coordinates is from the same point of its lookahead coordinates, from the top of
    /// its lookahead tree (Compose's LookaheadLayoutCoordinates `lookaheadOffset`): a point of the window in these is
    /// its point in the main coordinates plus this; a point of these in the root is the main coordinates' point at it
    /// minus this. (Compose 1.12's `localToRoot`, `localToWindow` and `localToScreen` add it: they aren't the inverse
    /// of its `windowToLocal` then; KompozR subtracts it, the inverse.)
    fn lookahead_offset(&self) -> Offset {
        let root = self.lookahead_delegate().root_lookahead_delegate();
        let root_coordinator = root.coordinator();
        self.local_position_of(&root.coordinates(), Offset::Zero)
            - self.coordinator.local_position_of(&root_coordinator, Offset::Zero)
    }

    /// A point of these in the root's coordinates.
    pub fn local_to_root(&self, relative_to_local: Offset) -> Offset {
        if self.lookahead {
            return self.coordinator.local_to_root(relative_to_local - self.lookahead_offset());
        }
        self.coordinator.local_to_root(relative_to_local)
    }

    /// A point of these in the window's coordinates (the root's, moved by where the host's window puts it).
    pub fn local_to_window(&self, relative_to_local: Offset) -> Offset {
        let in_root = self.local_to_root(relative_to_local);
        in_root + self.coordinator.window_offset()
    }

    /// A point of the window's in these coordinates.
    pub fn window_to_local(&self, relative_to_window: Offset) -> Offset {
        if self.lookahead {
            let main = LayoutCoordinates::of(self.coordinator.clone());
            return main.window_to_local(relative_to_window) + self.lookahead_offset();
        }
        let root = self.find_root_coordinates();
        let in_root = relative_to_window - self.coordinator.window_offset() - root.position_in_root();
        self.local_position_of(&root, in_root)
    }

    /// A point of `source`'s in these coordinates (Compose's `localPositionOf`).
    pub fn local_position_of(&self, source: &LayoutCoordinates, relative_to_source: Offset) -> Offset {
        self.local_position_of_with(source, relative_to_source, true)
    }

    /// The same, the offsets of the layouts in between that were placed under a motion frame of reference left out
    /// unless `include_motion_frame_of_reference` (Compose's `localPositionOf(…, includeMotionFrameOfReference)`).
    /// Between lookahead coordinates, the positions are the lookahead pass's.
    ///
    /// Kotlin: `localPositionOf(sourceCoordinates: LayoutCoordinates, relativeToSource: Offset, includeMotionFrameOfReference: Boolean)`
    pub fn local_position_of_with(
        &self,
        source: &LayoutCoordinates,
        relative_to_source: Offset,
        include_motion_frame_of_reference: bool,
    ) -> Offset {
        match (self.lookahead, source.lookahead) {
            (false, false) => self.coordinator.local_position_of_with(
                &source.coordinator,
                relative_to_source,
                include_motion_frame_of_reference,
            ),
            // (Compose's NodeCoordinator: the lookahead source's view of this, the other way round)
            (false, true) => {
                source.coordinator.on_coordinates_used();
                -source.local_position_of_with(self, -relative_to_source, include_motion_frame_of_reference)
            }
            (true, true) => {
                self.lookahead_position_of_lookahead(source, relative_to_source, include_motion_frame_of_reference)
            }
            (true, false) => {
                // Mixed: this in lookahead, the source not — the lookahead part from the top of this lookahead tree,
                // then the main part from there.
                let root = self.lookahead_delegate().root_lookahead_delegate();
                let local_lookahead_position = self.local_position_of_with(
                    &root.coordinates(),
                    relative_to_source,
                    include_motion_frame_of_reference,
                ) - root.position.get().to_offset();
                let root_coordinator = root.coordinator();
                let root_coordinates = match root_coordinator.wrapped_by() {
                    Some(parent) => LayoutCoordinates::of(parent),
                    None => LayoutCoordinates::of(root_coordinator),
                };
                let local_position =
                    root_coordinates.local_position_of_with(source, Offset::Zero, include_motion_frame_of_reference);
                local_lookahead_position + local_position
            }
        }
    }

    /// Compose's LookaheadLayoutCoordinates `localPositionOf` of other lookahead coordinates.
    fn lookahead_position_of_lookahead(&self, source: &LayoutCoordinates, relative: Offset, include: bool) -> Offset {
        let me = self.lookahead_delegate();
        let other = source.lookahead_delegate();
        source.coordinator.on_coordinates_used();
        let common = self.coordinator.find_common_ancestor(&source.coordinator);
        match common.lookahead_delegate() {
            Some(ancestor) => {
                let source_in_ancestor = other.position_in(&ancestor, !include) + relative.round();
                let mine_in_ancestor = me.position_in(&ancestor, !include);
                (source_in_ancestor - mine_in_ancestor).to_offset()
            }
            None => {
                // Two lookahead trees: each one's position from its top, then their tops' parents mapped.
                let source_root = other.root_lookahead_delegate();
                let source_position =
                    other.position_in(&source_root, !include) + source_root.position.get() + relative.round();
                let root = me.root_lookahead_delegate();
                let position = me.position_in(&root, !include) + root.position.get();
                let relative_position = (source_position - position).to_offset();
                let target = root.coordinator().wrapped_by().expect("kompoz: a lookahead root under no layout");
                let from = source_root.coordinator().wrapped_by().expect("kompoz: a lookahead root under no layout");
                target.local_position_of_with(&from, relative_position, include)
            }
        }
    }

    /// The bounds of `source` in these coordinates: the box around its corners as they map here, cut by the
    /// clipping layers in between when `clip_bounds` (Compose's `localBoundingBoxOf`).
    pub fn local_bounding_box_of(&self, source: &LayoutCoordinates, clip_bounds: bool) -> Rect {
        self.coordinator.local_bounding_box_of(&source.coordinator, clip_bounds)
    }

    /// Sets `matrix` to the transform from `source`'s coordinates to these (Compose's `transformFrom`).
    pub fn transform_from(&self, source: &LayoutCoordinates, matrix: &mut Matrix) {
        self.coordinator.transform_from(&source.coordinator, matrix);
    }

    /// Sets `matrix` to the transform from these to the root's.
    pub fn transform_to_root(&self, matrix: &mut Matrix) {
        let root = self.find_root_coordinates();
        root.transform_from(self, matrix);
    }

    /// Where `line` is in it, or [`AlignmentLine::Unspecified`] (Compose's `LayoutCoordinates.get`).
    pub fn get(&self, line: impl Into<AlignmentLine>) -> i32 {
        if self.lookahead {
            return self.lookahead_delegate().alignment_line(line.into());
        }
        self.coordinator.alignment_line(line.into())
    }

    /// The lines it and the coordinates it wraps provide (Compose's `providedAlignmentLines`).
    pub fn provided_alignment_lines(&self) -> Vec<AlignmentLine> {
        self.coordinator.provided_alignment_lines()
    }

    /// The layout node it belongs to.
    pub fn layout_node(&self) -> Option<Rc<crate::node::LayoutNode>> {
        self.coordinator.layout_node()
    }

    /// These coordinates as the lookahead pass laid them out (Compose's `LookaheadScope`
    /// `LayoutCoordinates.toLookaheadCoordinates()`, which needs nothing of its scope): lookahead coordinates (the
    /// same if these are), or these for a layout out of any lookahead scope.
    pub fn to_lookahead_coordinates(&self) -> LayoutCoordinates {
        if self.lookahead || self.coordinator.lookahead_delegate().is_none() {
            return self.clone();
        }
        LayoutCoordinates::lookahead_of(self.coordinator.clone())
    }
}

#[named]
impl LayoutCoordinates {
    /// A point of `source_coordinates`'s in these, both as the lookahead pass laid them out (Compose's
    /// `LookaheadScope` `LayoutCoordinates.localLookaheadPositionOf`, which needs nothing of its scope): the offsets of
    /// what was placed under a motion frame of reference left out unless `include_motion_frame_of_reference`.
    pub fn local_lookahead_position_of(
        &self,
        source_coordinates: &LayoutCoordinates,
        #[default(Offset::Zero)] relative_to_source: Offset,
        #[default(true)] include_motion_frame_of_reference: bool,
    ) -> Offset {
        let lookahead = self.to_lookahead_coordinates();
        let source = source_coordinates.to_lookahead_coordinates();
        if lookahead.is_lookahead() {
            lookahead.local_position_of_with(&source, relative_to_source, include_motion_frame_of_reference)
        } else if source.is_lookahead() {
            -source.local_position_of_with(&lookahead, relative_to_source, include_motion_frame_of_reference)
        } else {
            lookahead.local_position_of_with(&source, relative_to_source, include_motion_frame_of_reference)
        }
    }
}
