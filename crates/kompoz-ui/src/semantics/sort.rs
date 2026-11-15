//! The order accessibility reads nodes in (Compose's SemanticsSort): nodes grouped in rows by their bounds in the
//! window, rows top to bottom, each row from the start, then by their traversal indexes; a traversal group's
//! descendants read together after it.

use super::node::SemanticsNode;
use super::properties::SemanticsProperties;
use kompoz_graphics::{LayoutDirection, Rect};
use std::cmp::Ordering;
use std::collections::HashMap;

impl SemanticsNode {
    /// The nodes of `list` and below, in reading order (Compose's `subtreeSortedByGeometryGrouping`): the visible
    /// traversal groups and screen-reader-focusable containers, each group's descendants right after it.
    pub fn subtree_sorted_by_geometry_grouping(
        &self,
        is_visible: &dyn Fn(&SemanticsNode) -> bool,
        is_focusable_container: &dyn Fn(&SemanticsNode) -> bool,
        list: Vec<SemanticsNode>,
    ) -> Vec<SemanticsNode> {
        let mut containers: HashMap<u64, Vec<SemanticsNode>> = HashMap::new();
        let mut geometry = Vec::new();
        for node in list {
            node.geometry_depth_first_search(&mut geometry, is_visible, is_focusable_container, &mut containers);
        }
        self.sort_by_geometry_groupings(geometry, is_focusable_container, &containers)
    }

    fn geometry_depth_first_search(
        &self,
        geometry: &mut Vec<SemanticsNode>,
        is_visible: &dyn Fn(&SemanticsNode) -> bool,
        is_focusable_container: &dyn Fn(&SemanticsNode) -> bool,
        containers: &mut HashMap<u64, Vec<SemanticsNode>>,
    ) {
        let is_group = self.unmerged_config().get_or_else(SemanticsProperties::IsTraversalGroup, || false);
        if (is_group || is_focusable_container(self)) && is_visible(self) {
            geometry.push(self.clone());
        }
        if is_group {
            let sorted = self.subtree_sorted_by_geometry_grouping(is_visible, is_focusable_container, self.children());
            containers.insert(self.id(), sorted);
        } else {
            for child in self.children() {
                child.geometry_depth_first_search(geometry, is_visible, is_focusable_container, containers);
            }
        }
    }

    /// `list` in reading order (Compose's `sortByGeometryGroupings`): grouped into rows of nodes whose vertical
    /// spans overlap, rows top to bottom, each row from its start (by the layout direction of this node), then a
    /// stable sort by traversal index; a container in `containers` followed by its sorted descendants (and left
    /// out itself unless it is screen-reader focusable).
    pub fn sort_by_geometry_groupings(
        &self,
        list: Vec<SemanticsNode>,
        is_focusable_container: &dyn Fn(&SemanticsNode) -> bool,
        containers: &HashMap<u64, Vec<SemanticsNode>>,
    ) -> Vec<SemanticsNode> {
        let rtl = self.layout_node().layout_direction() == LayoutDirection::Rtl;
        let mut rows: Vec<(Rect, Vec<SemanticsNode>)> = Vec::new();
        for (i, node) in list.into_iter().enumerate() {
            if i == 0 || !placed_entry_row_overlaps(&mut rows, &node) {
                rows.push((node.bounds_in_window(), vec![node]));
            }
        }
        rows.sort_by(|a, b| cmp_f32(a.0.top, b.0.top).then(cmp_f32(a.0.bottom, b.0.bottom)));
        let mut out = Vec::new();
        for (_, mut row) in rows {
            row.sort_by(|a, b| {
                let bounds = if rtl { rtl_bounds(a, b) } else { ltr_bounds(a, b) };
                bounds.then_with(|| z_order(a, b)).then(a.id().cmp(&b.id()))
            });
            out.extend(row);
        }
        out.sort_by(|a, b| cmp_f32(traversal_index(a), traversal_index(b)));
        let mut i = 0;
        while i < out.len() {
            if let Some(children) = containers.get(&out[i].id()) {
                if is_focusable_container(&out[i]) {
                    i += 1;
                } else {
                    out.remove(i);
                }
                let n = children.len();
                for (k, c) in children.iter().enumerate() {
                    out.insert(i + k, c.clone());
                }
                i += n;
            } else {
                i += 1;
            }
        }
        out
    }
}

fn cmp_f32(a: f32, b: f32) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

fn traversal_index(n: &SemanticsNode) -> f32 {
    n.unmerged_config().get_or_else(SemanticsProperties::TraversalIndex, || 0.0)
}

/// Whether `node` overlaps a row vertically: it joins it, the row's span narrowed to the overlap.
fn placed_entry_row_overlaps(rows: &mut [(Rect, Vec<SemanticsNode>)], node: &SemanticsNode) -> bool {
    let bounds = node.bounds_in_window();
    let (top, bottom) = (bounds.top, bounds.bottom);
    let entry_empty = top >= bottom;
    for (rect, nodes) in rows.iter_mut() {
        let group_empty = rect.top >= rect.bottom;
        if !entry_empty && !group_empty && top.max(rect.top) < bottom.min(rect.bottom) {
            *rect = Rect::new(rect.left.max(0.0), rect.top.max(top), rect.right, rect.bottom.min(bottom));
            nodes.push(node.clone());
            return true;
        }
    }
    false
}

fn ltr_bounds(a: &SemanticsNode, b: &SemanticsNode) -> Ordering {
    let (ab, bb) = (a.bounds_in_window(), b.bounds_in_window());
    cmp_f32(ab.left, bb.left)
        .then(cmp_f32(ab.top, bb.top))
        .then(cmp_f32(ab.bottom, bb.bottom))
        .then(cmp_f32(ab.right, bb.right))
}

fn rtl_bounds(a: &SemanticsNode, b: &SemanticsNode) -> Ordering {
    let (ab, bb) = (a.bounds_in_window(), b.bounds_in_window());
    cmp_f32(bb.right, ab.right)
        .then(cmp_f32(ab.top, bb.top))
        .then(cmp_f32(ab.bottom, bb.bottom))
        .then(cmp_f32(bb.left, ab.left))
}

/// Compose's LayoutNode.ZComparator: by z-index, then by the order their parents placed them.
fn z_order(a: &SemanticsNode, b: &SemanticsNode) -> Ordering {
    let (a, b) = (a.layout_node(), b.layout_node());
    cmp_f32(a.z_index.get(), b.z_index.get()).then(a.place_order.get().cmp(&b.place_order.get()))
}
