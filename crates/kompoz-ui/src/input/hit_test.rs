//! Hit testing for pointer input, as Compose's `HitTestResult` and `NodeCoordinator.hitTest` with its
//! `PointerInputSource`: through each layout node's coordinators (their layers' matrices and clips), children
//! topmost first, a hit path of the pointer input nodes under the pointer, outermost first. A finger also hits
//! what is smaller than the minimum touch target (48 dp) near it — the nearest such one when it is near several —
//! and a stylus what a node's touch bounds expansion reaches.

use super::{PointerInputModifierNode, PointerType};
use crate::modifier::{NodeKind, NodeRc, dispatch_for_kind};
use crate::node::{LayoutNode, NodeCoordinator};
use crate::platform::LocalViewConfiguration;
use kompoz_graphics::{Density, Dp, LayoutDirection, Offset, Outline, RectangleShape, Size};
use std::rc::Rc;

/// A hit's distance from the node's edge (negative: inside) and whether it is in every layer above and in an
/// expanded touch bounds (Compose's DistanceAndFlags).
#[derive(Clone, Copy, Debug)]
struct DistanceAndFlags {
    distance: f32,
    in_layer: bool,
    in_expanded_bounds: bool,
}

/// Kotlin's `sign(x).toInt()`: 0 for 0 and NaN.
fn sign(x: f32) -> i32 {
    if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    }
}

impl DistanceAndFlags {
    fn new(distance: f32, in_layer: bool, in_expanded_bounds: bool) -> DistanceAndFlags {
        DistanceAndFlags { distance, in_layer, in_expanded_bounds }
    }

    /// Compose's `compareTo`: in a layer first, then nearer; out of the node, in expanded bounds before a minimum
    /// touch target.
    fn compare(&self, other: &DistanceAndFlags) -> i32 {
        if self.in_layer != other.in_layer {
            return if self.in_layer { -1 } else { 1 };
        }
        let diff = sign(self.distance - other.distance);
        if self.distance.min(other.distance) < 0.0 {
            return diff;
        }
        if self.in_expanded_bounds != other.in_expanded_bounds {
            return if self.in_expanded_bounds { -1 } else { 1 };
        }
        diff
    }
}

/// The nodes a pointer hit, outermost first, with how each was hit (Compose's HitTestResult): `hit_depth` is the
/// depth being tested; what is past it are candidates a better hit may replace.
#[derive(Default)]
pub(crate) struct HitTestResult {
    values: Vec<NodeRc>,
    flags: Vec<DistanceAndFlags>,
    hit_depth: isize,
}

impl HitTestResult {
    pub(crate) fn new() -> HitTestResult {
        HitTestResult { values: Vec::new(), flags: Vec::new(), hit_depth: -1 }
    }

    pub(crate) fn nodes(&self) -> &[NodeRc] {
        &self.values
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.hit_depth = -1;
        self.values.clear();
        self.flags.clear();
    }

    fn last_index(&self) -> isize {
        self.values.len() as isize - 1
    }

    /// Whether what is past the depth is a real hit (in its bounds and every layer).
    pub(crate) fn has_hit(&self) -> bool {
        let d = self.find_best_hit_distance();
        d.distance < 0.0 && d.in_layer && !d.in_expanded_bounds
    }

    /// Keeps the hits so far: siblings tested after may add theirs (Compose's `acceptHits`).
    pub(crate) fn accept_hits(&mut self) {
        self.hit_depth = self.last_index();
    }

    /// Whether a hit `distance_from_edge` away would be better than what is past the depth.
    pub(crate) fn is_hit_in_minimum_touch_target_better(&self, distance_from_edge: f32, in_layer: bool) -> bool {
        if self.hit_depth == self.last_index() {
            return true;
        }
        let candidate = DistanceAndFlags::new(distance_from_edge, in_layer, false);
        self.find_best_hit_distance().compare(&candidate) > 0
    }

    fn find_best_hit_distance(&self) -> DistanceAndFlags {
        let mut best = DistanceAndFlags::new(f32::INFINITY, false, false);
        let start = (self.hit_depth + 1) as usize;
        for d in self.flags.iter().skip(start) {
            if d.compare(&best) < 0 {
                best = *d;
            }
            if best.distance < 0.0 && best.in_layer {
                return best;
            }
        }
        best
    }

    /// A real hit: `node`, then what `child` adds under it.
    fn hit(&mut self, node: &NodeRc, in_layer: bool, child: impl FnOnce(&mut HitTestResult)) {
        self.hit_in_minimum_touch_target(node, -1.0, in_layer, false, child);
    }

    fn hit_in_minimum_touch_target(
        &mut self,
        node: &NodeRc,
        distance_from_edge: f32,
        in_layer: bool,
        in_expanded_bounds: bool,
        child: impl FnOnce(&mut HitTestResult),
    ) {
        let start_depth = self.hit_depth;
        self.remove_nodes_in_range(self.hit_depth + 1, self.values.len() as isize);
        self.hit_depth += 1;
        self.values.push(node.clone());
        self.flags.push(DistanceAndFlags::new(distance_from_edge, in_layer, in_expanded_bounds));
        child(self);
        self.hit_depth = start_depth;
    }

    /// A hit in a node's expanded touch bounds (Compose's `hitExpandedTouchBounds`).
    fn hit_expanded_touch_bounds(&mut self, node: &NodeRc, in_layer: bool, child: impl FnOnce(&mut HitTestResult)) {
        if self.hit_depth == self.last_index() {
            self.hit_in_minimum_touch_target(node, 0.0, in_layer, true, child);
            return;
        }
        let previous_distance = self.find_best_hit_distance();
        let previous_hit_depth = self.hit_depth;
        if previous_distance.in_expanded_bounds {
            self.hit_depth = self.last_index();
            self.hit_in_minimum_touch_target(node, 0.0, in_layer, true, child);
            let new_distance = self.find_best_hit_distance();
            if new_distance.distance < 0.0 {
                let start = previous_hit_depth + 1;
                let end = self.hit_depth + 1;
                self.remove_nodes_in_range(start, end);
            }
            self.hit_depth = previous_hit_depth;
        } else if previous_distance.distance > 0.0 {
            self.hit_in_minimum_touch_target(node, 0.0, in_layer, true, child);
        }
    }

    /// A node that intercepts its children's out of bounds events: kept only if its children give a better hit
    /// than what there is (Compose's `speculativeHit`).
    fn speculative_hit(
        &mut self,
        node: &NodeRc,
        distance_from_edge: f32,
        in_layer: bool,
        child: impl FnOnce(&mut HitTestResult),
    ) {
        if self.hit_depth == self.last_index() {
            self.hit_in_minimum_touch_target(node, distance_from_edge, in_layer, false, child);
            if self.hit_depth + 1 == self.last_index() || self.find_best_hit_distance().in_expanded_bounds {
                self.remove_node_at_depth(self.hit_depth + 1);
            }
            return;
        }
        let previous_distance = self.find_best_hit_distance();
        let previous_hit_depth = self.hit_depth;
        self.hit_depth = self.last_index();
        self.hit_in_minimum_touch_target(node, distance_from_edge, in_layer, false, child);
        let new_best = self.find_best_hit_distance();
        if self.hit_depth + 1 < self.last_index() && previous_distance.compare(&new_best) > 0 {
            let start = previous_hit_depth + 1;
            let end = if new_best.in_expanded_bounds { self.hit_depth + 2 } else { self.hit_depth + 1 };
            self.remove_nodes_in_range(start, end);
        } else {
            self.remove_nodes_in_range(self.hit_depth + 1, self.values.len() as isize);
        }
        self.hit_depth = previous_hit_depth;
    }

    fn remove_node_at_depth(&mut self, depth: isize) {
        let d = depth as usize;
        if d < self.values.len() {
            self.values.remove(d);
            self.flags.remove(d);
        }
    }

    fn remove_nodes_in_range(&mut self, start: isize, end: isize) {
        let end = end.min(self.values.len() as isize);
        if start >= end || start < 0 {
            return;
        }
        self.values.drain(start as usize..end as usize);
        self.flags.drain(start as usize..end as usize);
    }

    /// Hits of siblings: each starts from the same depth (Compose's `siblingHits`).
    fn sibling_hits(&mut self, block: impl FnOnce(&mut HitTestResult)) {
        let depth = self.hit_depth;
        block(self);
        self.hit_depth = depth;
    }
}

/// How far a node's touch bounds reach past its sides, px (Compose's TouchBoundsExpansion): what a stylus hits
/// around it. Start and end follow the layout direction when it is aware of it.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct TouchBoundsExpansion {
    pub start: f32,
    pub top: f32,
    pub end: f32,
    pub bottom: f32,
    pub is_layout_direction_aware: bool,
}

impl TouchBoundsExpansion {
    pub const None: TouchBoundsExpansion =
        TouchBoundsExpansion { start: 0.0, top: 0.0, end: 0.0, bottom: 0.0, is_layout_direction_aware: false };

    /// Start and end that follow the layout direction.
    pub fn new(start: f32, top: f32, end: f32, bottom: f32) -> TouchBoundsExpansion {
        TouchBoundsExpansion { start, top, end, bottom, is_layout_direction_aware: true }
    }

    /// Left and right whatever the layout direction (Compose's `Absolute`).
    pub fn absolute(left: f32, top: f32, right: f32, bottom: f32) -> TouchBoundsExpansion {
        TouchBoundsExpansion { start: left, top, end: right, bottom, is_layout_direction_aware: false }
    }

    pub fn compute_left(&self, layout_direction: LayoutDirection) -> f32 {
        if self.is_layout_direction_aware && layout_direction == LayoutDirection::Rtl { self.end } else { self.start }
    }

    pub fn compute_right(&self, layout_direction: LayoutDirection) -> f32 {
        if self.is_layout_direction_aware && layout_direction == LayoutDirection::Rtl { self.start } else { self.end }
    }
}

/// A touch bounds expansion in dp (Compose's DpTouchBoundsExpansion).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct DpTouchBoundsExpansion {
    pub start: Dp,
    pub top: Dp,
    pub end: Dp,
    pub bottom: Dp,
    pub is_layout_direction_aware: bool,
}

impl DpTouchBoundsExpansion {
    /// Start and end that follow the layout direction.
    pub fn new(start: Dp, top: Dp, end: Dp, bottom: Dp) -> DpTouchBoundsExpansion {
        DpTouchBoundsExpansion { start, top, end, bottom, is_layout_direction_aware: true }
    }

    /// In px at `density`, rounded (Compose's `roundToTouchBoundsExpansion`).
    pub fn round_to_touch_bounds_expansion(&self, density: &Density) -> TouchBoundsExpansion {
        TouchBoundsExpansion {
            start: density.round_to_px(self.start) as f32,
            top: density.round_to_px(self.top) as f32,
            end: density.round_to_px(self.end) as f32,
            bottom: density.round_to_px(self.bottom) as f32,
            is_layout_direction_aware: self.is_layout_direction_aware,
        }
    }
}

/// Calls `f` on the pointer input part of `cell` (it, or its first delegate that is one).
fn with_pointer_input<R>(cell: &NodeRc, f: impl FnOnce(&mut dyn PointerInputModifierNode) -> R) -> Option<R> {
    let mut target = None;
    dispatch_for_kind(cell, NodeKind::PointerInput, &mut |c| {
        if target.is_none() {
            target = Some(c.clone());
        }
    });
    let target = target?;
    let mut node = target.node.borrow_mut();
    node.as_pointer_input().map(f)
}

/// Whether any pointer input part of `cell` says `f`.
fn any_pointer_input(cell: &NodeRc, f: &dyn Fn(&dyn PointerInputModifierNode) -> bool) -> bool {
    let mut found = false;
    dispatch_for_kind(cell, NodeKind::PointerInput, &mut |c| {
        if !found {
            let mut node = c.node.borrow_mut();
            if let Some(p) = node.as_pointer_input() {
                found = f(p);
            }
        }
    });
    found
}

/// The chain nodes a coordinator hit tests, head first (Compose's `visitNodes` without its tail): the wrapping
/// coordinator's layout node (a node both layout and pointer input hit tests in what it measured), then those
/// before its own.
fn pointer_nodes(c: &Rc<NodeCoordinator>) -> Vec<NodeRc> {
    let mut nodes = Vec::new();
    let Some(layout_node) = c.layout_node() else { return nodes };
    if !Rc::ptr_eq(&layout_node.outer_coordinator(), c)
        && let Some(entry) = c.wrapped_by().and_then(|w| w.entry())
    {
        nodes.push(entry);
    }
    let mut own = c.nodes();
    if !c.is_inner() {
        own.pop();
    }
    nodes.extend(own);
    nodes
}

/// The index of the next pointer input node from `from` (Compose's `nextUntil(PointerInput, Layout)`).
fn next_pointer(nodes: &[NodeRc], from: usize) -> Option<usize> {
    (from..nodes.len()).find(|&i| nodes[i].is_kind(NodeKind::PointerInput))
}

/// The minimum touch target in px (LocalViewConfiguration where the layout was composed).
fn minimum_touch_target_size(c: &NodeCoordinator) -> Size {
    c.layout_node()
        .map_or(Size::new(48.0, 48.0), |n| n.current_value_of(&LocalViewConfiguration).minimum_touch_target_size)
}

/// Whether the position isn't cut by its layer's clip (Compose's `withinLayerBounds`; the layer's `isInLayer`:
/// its bounds for a rectangle, its outline otherwise).
fn within_layer_bounds(c: &NodeCoordinator, position: Offset) -> bool {
    if !position.is_finite() {
        return false;
    }
    let layer = c.layer.borrow();
    let Some(layer) = layer.as_ref() else { return true };
    if !layer.props.clip {
        return true;
    }
    let size = c.measured_size.get();
    let (x, y) = (position.x, position.y);
    if layer.props.shape.shape_as_any().is::<RectangleShape>() {
        return 0.0 <= x && x < size.width as f32 && 0.0 <= y && y < size.height as f32;
    }
    match layer.props.outline() {
        Outline::Rectangle(r) => r.contains(position),
        Outline::Rounded(r) => r.contains(position),
        Outline::Generic(p) => p.contains(position),
    }
}

fn is_pointer_in_bounds(c: &NodeCoordinator, position: Offset) -> bool {
    let size = c.measured_size.get();
    position.x >= 0.0 && position.y >= 0.0 && position.x < size.width as f32 && position.y < size.height as f32
}

/// How far a position is outside its size along each axis (Compose's `offsetFromEdge`).
fn offset_from_edge(c: &NodeCoordinator, position: Offset) -> Offset {
    let size = c.measured_size.get();
    let x = position.x;
    let horizontal = (if x < 0.0 { -x } else { x - size.width as f32 }).max(0.0);
    let y = position.y;
    let vertical = (if y < 0.0 { -y } else { y - size.height as f32 }).max(0.0);
    Offset::new(horizontal, vertical)
}

/// What it lacks of the minimum touch target on each side (Compose's `calculateMinimumTouchTargetPadding`).
pub(crate) fn minimum_touch_target_padding(size: kompoz_graphics::IntSize, minimum: Size) -> Size {
    Size::new(
        ((minimum.width - size.width as f32) / 2.0).max(0.0),
        ((minimum.height - size.height as f32) / 2.0).max(0.0),
    )
}

/// The squared distance of a position in its minimum touch target from its bounds, infinite outside it or for
/// something big enough (Compose's `distanceInMinimumTouchTarget`).
fn distance_in_minimum_touch_target(c: &NodeCoordinator, position: Offset) -> f32 {
    let minimum = minimum_touch_target_size(c);
    let size = c.measured_size.get();
    if size.width as f32 >= minimum.width && size.height as f32 >= minimum.height {
        return f32::INFINITY;
    }
    let padding = minimum_touch_target_padding(size, minimum);
    let offset = offset_from_edge(c, position);
    if (padding.width > 0.0 || padding.height > 0.0) && offset.x <= padding.width && offset.y <= padding.height {
        offset.get_distance_squared()
    } else {
        f32::INFINITY
    }
}

/// Hit tests a layout node and what is under it at `position` in the coordinates it is placed in (Compose's
/// `LayoutNode.hitTest`).
pub(crate) fn hit_test_layout_node(
    node: &Rc<LayoutNode>,
    position: Offset,
    result: &mut HitTestResult,
    pointer_type: PointerType,
    in_layer: bool,
) {
    let outer = node.outer_coordinator();
    let local = outer.parent_to_local_position(position);
    hit_test(&outer, local, result, pointer_type, in_layer);
}

/// Compose's `NodeCoordinator.hitTest` with the pointer input source.
fn hit_test(c: &Rc<NodeCoordinator>, position: Offset, r: &mut HitTestResult, pt: PointerType, in_layer: bool) {
    let nodes = pointer_nodes(c);
    let head = next_pointer(&nodes, 0);
    if !within_layer_bounds(c, position) {
        // Clipped away; still a hit within the minimum touch target of something too small.
        if pt == PointerType::Touch {
            let d = distance_in_minimum_touch_target(c, position);
            if d.is_finite() && r.is_hit_in_minimum_touch_target_better(d, false) {
                hit_near(c, &nodes, head, position, r, pt, false, d);
            }
        }
    } else if head.is_none() {
        hit_test_child(c, position, r, pt, in_layer);
    } else if is_pointer_in_bounds(c, position) {
        hit(c, &nodes, head, position, r, pt, in_layer);
    } else {
        let d = if pt != PointerType::Touch { f32::INFINITY } else { distance_in_minimum_touch_target(c, position) };
        let better = d.is_finite() && r.is_hit_in_minimum_touch_target_better(d, in_layer);
        out_of_bounds_hit(c, &nodes, head, position, r, pt, in_layer, d, better);
    }
}

fn hit(
    c: &Rc<NodeCoordinator>,
    nodes: &[NodeRc],
    at: Option<usize>,
    position: Offset,
    r: &mut HitTestResult,
    pt: PointerType,
    in_layer: bool,
) {
    match at {
        None => hit_test_child(c, position, r, pt, in_layer),
        Some(i) => r.hit(&nodes[i], in_layer, |r| hit(c, nodes, next_pointer(nodes, i + 1), position, r, pt, in_layer)),
    }
}

#[allow(clippy::too_many_arguments)]
fn out_of_bounds_hit(
    c: &Rc<NodeCoordinator>,
    nodes: &[NodeRc],
    at: Option<usize>,
    position: Offset,
    r: &mut HitTestResult,
    pt: PointerType,
    in_layer: bool,
    distance: f32,
    better: bool,
) {
    let Some(i) = at else { return hit_test_child(c, position, r, pt, in_layer) };
    let next = next_pointer(nodes, i + 1);
    if is_in_expanded_touch_bounds(c, &nodes[i], position, pt) {
        r.hit_expanded_touch_bounds(&nodes[i], in_layer, |r| {
            out_of_bounds_hit(c, nodes, next, position, r, pt, in_layer, distance, better)
        });
    } else if better {
        hit_near(c, nodes, Some(i), position, r, pt, in_layer, distance);
    } else {
        speculative_hit(c, nodes, Some(i), position, r, pt, in_layer, distance);
    }
}

#[allow(clippy::too_many_arguments)]
fn hit_near(
    c: &Rc<NodeCoordinator>,
    nodes: &[NodeRc],
    at: Option<usize>,
    position: Offset,
    r: &mut HitTestResult,
    pt: PointerType,
    in_layer: bool,
    distance: f32,
) {
    let Some(i) = at else { return hit_test_child(c, position, r, pt, in_layer) };
    let next = next_pointer(nodes, i + 1);
    // Nearer than what there is: recorded.
    r.hit_in_minimum_touch_target(&nodes[i], distance, in_layer, false, |r| {
        out_of_bounds_hit(c, nodes, next, position, r, pt, in_layer, distance, true)
    });
}

#[allow(clippy::too_many_arguments)]
fn speculative_hit(
    c: &Rc<NodeCoordinator>,
    nodes: &[NodeRc],
    at: Option<usize>,
    position: Offset,
    r: &mut HitTestResult,
    pt: PointerType,
    in_layer: bool,
    distance: f32,
) {
    let Some(i) = at else { return hit_test_child(c, position, r, pt, in_layer) };
    let next = next_pointer(nodes, i + 1);
    if any_pointer_input(&nodes[i], &|p| p.intercept_out_of_bounds_child_events()) {
        // Replacing what there is only if a child gives a better hit.
        r.speculative_hit(&nodes[i], distance, in_layer, |r| {
            out_of_bounds_hit(c, nodes, next, position, r, pt, in_layer, distance, false)
        });
    } else {
        out_of_bounds_hit(c, nodes, next, position, r, pt, in_layer, distance, false);
    }
}

/// Whether a stylus is in the node's expanded touch bounds (Compose's `isInExpandedTouchBounds`).
fn is_in_expanded_touch_bounds(c: &NodeCoordinator, cell: &NodeRc, position: Offset, pt: PointerType) -> bool {
    if pt != PointerType::Stylus && pt != PointerType::Eraser {
        return false;
    }
    let Some(e) = with_pointer_input(cell, |p| p.touch_bounds_expansion()) else { return false };
    let direction = c.layout_node().map_or(LayoutDirection::Ltr, |n| n.layout_direction());
    let size = c.measured_size.get();
    position.x >= -e.compute_left(direction)
        && position.x < size.width as f32 + e.compute_right(direction)
        && position.y >= -e.top
        && position.y < size.height as f32 + e.bottom
}

/// What a coordinator wraps, hit tested: the next coordinator in, or the layout node's children topmost first,
/// the first one hit taking it unless it shares with its siblings (Compose's `hitTestChild` of the two
/// coordinators).
fn hit_test_child(c: &Rc<NodeCoordinator>, position: Offset, r: &mut HitTestResult, pt: PointerType, in_layer: bool) {
    if !c.is_inner() {
        if let Some(wrapped) = c.wrapped() {
            let local = wrapped.parent_to_local_position(position);
            hit_test(&wrapped, local, r, pt, in_layer);
        }
        return;
    }
    let Some(layout_node) = c.layout_node() else { return };
    let mut in_layer = in_layer;
    let test_children = if within_layer_bounds(c, position) {
        true
    } else if pt == PointerType::Touch && distance_in_minimum_touch_target(c, position).is_finite() {
        in_layer = false;
        true
    } else {
        false
    };
    if !test_children {
        return;
    }
    r.sibling_hits(|r| {
        for child in layout_node.z_sorted_children().iter().rev() {
            // (a layout taking no input: KompozR's PointerInputModifierNode::blocks_input)
            if !child.is_placed() || super::layout_blocks_input(child) {
                continue;
            }
            hit_test_layout_node(child, position, r, pt, in_layer);
            let was_hit = r.has_hit();
            let continue_hit_test = !was_hit || {
                if should_share_pointer_input_with_siblings(child) {
                    r.accept_hits();
                    true
                } else {
                    false
                }
            };
            if !continue_hit_test {
                break;
            }
        }
    });
}

/// Whether a pointer input node of the layout node's chain shares with its siblings (Compose's
/// `shouldSharePointerInputWithSiblings`).
fn should_share_pointer_input_with_siblings(node: &Rc<LayoutNode>) -> bool {
    node.chain_nodes().iter().any(|cell| {
        cell.meta.attached.get()
            && cell.is_kind(NodeKind::PointerInput)
            && any_pointer_input(cell, &|p| p.share_pointer_input_with_siblings())
    })
}
