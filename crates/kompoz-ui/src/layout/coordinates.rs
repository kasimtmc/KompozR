//! Where a layout is (Compose's LayoutCoordinates): its size, where it is in its parent, the root and the window,
//! and maps between it and other layouts, through their layers' matrices.

use super::AlignmentLine;
use crate::node::NodeCoordinator;
use kompoz_graphics::{IntSize, Matrix, Offset, Rect};
use std::fmt;
use std::rc::Rc;

/// The coordinates of a layout (one of its coordinators: a layout node's outer one, or one of a layout modifier,
/// or its inner one), as Compose's LayoutCoordinates. A cheap handle; it reads where the layout is when asked.
#[derive(Clone)]
pub struct LayoutCoordinates {
    pub(crate) coordinator: Rc<NodeCoordinator>,
}

impl PartialEq for LayoutCoordinates {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.coordinator, &other.coordinator)
    }
}

impl fmt::Debug for LayoutCoordinates {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LayoutCoordinates({:?} at {:?})", self.size(), self.position_in_root())
    }
}

impl LayoutCoordinates {
    pub(crate) fn of(coordinator: Rc<NodeCoordinator>) -> LayoutCoordinates {
        LayoutCoordinates { coordinator }
    }

    /// Its measured size.
    pub fn size(&self) -> IntSize {
        self.coordinator.measured_size.get()
    }

    /// Whether its layout is in a host (asking for the rest of a detached one panics, as in Compose).
    pub fn is_attached(&self) -> bool {
        self.coordinator.is_attached()
    }

    /// The coordinates of the layout node it is in's parent (its inner coordinator), none at the root (Compose's
    /// `parentLayoutCoordinates`).
    pub fn parent_layout_coordinates(&self) -> Option<LayoutCoordinates> {
        self.coordinator.parent_layout_coordinator().map(LayoutCoordinates::of)
    }

    /// The coordinates wrapping these: the next layout modifier out, or the parent's inner coordinator (Compose's
    /// `parentCoordinates`).
    pub fn parent_coordinates(&self) -> Option<LayoutCoordinates> {
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

    /// A point of these in the root's coordinates.
    pub fn local_to_root(&self, relative_to_local: Offset) -> Offset {
        self.coordinator.local_to_root(relative_to_local)
    }

    /// A point of these in the window's coordinates (the root's, moved by where the host's window puts it).
    pub fn local_to_window(&self, relative_to_local: Offset) -> Offset {
        let in_root = self.local_to_root(relative_to_local);
        in_root + self.coordinator.window_offset()
    }

    /// A point of the window's in these coordinates.
    pub fn window_to_local(&self, relative_to_window: Offset) -> Offset {
        let root = self.find_root_coordinates();
        let in_root = relative_to_window - self.coordinator.window_offset() - root.position_in_root();
        self.local_position_of(&root, in_root)
    }

    /// A point of `source`'s in these coordinates (Compose's `localPositionOf`).
    pub fn local_position_of(&self, source: &LayoutCoordinates, relative_to_source: Offset) -> Offset {
        self.coordinator.local_position_of(&source.coordinator, relative_to_source)
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
}
