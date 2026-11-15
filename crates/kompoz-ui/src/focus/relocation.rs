//! Bringing a node into view (Compose's `androidx.compose.ui.relocation`): a node asks the nearest
//! [`BringIntoViewModifierNode`] above it (a scroll container) to bring a rect of its into view; each one scrolls
//! and asks the one above it in turn.

use crate::layout::LayoutCoordinates;
use crate::modifier::{NodeCx, NodeKind, NodeRc};
use crate::node::LayoutNode;
use kompoz_graphics::{Offset, Rect};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

/// What a bring-into-view request asks for: the rect, in the requesting node's coordinates, when asked (none: it
/// is gone).
pub type BoundsProvider = Rc<dyn Fn() -> Option<Rect>>;

/// A future a node brings something into view in.
pub type BringIntoViewFuture = Pin<Box<dyn Future<Output = ()>>>;

/// A node that brings a rect of a node below it into view (Compose's BringIntoViewModifierNode: a scroll
/// container scrolls it in, then asks the node above it with [`bring_into_view`]). It hands back a future (it may
/// animate); what the future needs it takes along.
pub trait BringIntoViewModifierNode {
    /// Brings `bounds()` of `child_coordinates` (in their coordinates) into view.
    fn bring_into_view(&mut self, child_coordinates: LayoutCoordinates, bounds: BoundsProvider) -> BringIntoViewFuture;
}

/// Asks the bring-into-view node above `cx`'s node to bring `bounds` (in its coordinates; none: its whole size)
/// into view (Compose's `DelegatableNode.bringIntoView`); done at once if there is none.
pub fn bring_into_view(cx: &NodeCx, bounds: Option<BoundsProvider>) -> BringIntoViewFuture {
    if !cx.is_attached() {
        return Box::pin(async {});
    }
    let Some(coordinates) = cx.coordinates() else { return Box::pin(async {}) };
    let mut parent = None;
    cx.visit_ancestors(NodeKind::BringIntoView, false, &mut |c| {
        parent = Some(c.cell.clone());
        false
    });
    ask(parent, coordinates, bounds)
}

/// The same for a layout node (what tests' `perform_scroll_to` asks): the nearest bring-into-view node in its
/// chain or above it, for its whole size (or `bounds`, in its coordinates); none if there is none.
pub fn bring_layout_into_view(node: &Rc<LayoutNode>, bounds: Option<BoundsProvider>) -> Option<BringIntoViewFuture> {
    if !node.is_attached() {
        return None;
    }
    let coordinates = node.coordinates();
    let mut parent = None;
    let mut n = Some(node.clone());
    'outer: while let Some(l) = n {
        for cell in l.chain_nodes().iter().rev() {
            let mut found = None;
            crate::modifier::dispatch_for_kind(cell, NodeKind::BringIntoView, &mut |c| {
                if found.is_none() {
                    found = Some(c.clone());
                }
            });
            if found.is_some() {
                parent = found;
                break 'outer;
            }
        }
        n = l.parent();
    }
    parent.as_ref()?;
    Some(ask(parent, coordinates, bounds))
}

fn ask(parent: Option<NodeRc>, coordinates: LayoutCoordinates, bounds: Option<BoundsProvider>) -> BringIntoViewFuture {
    let Some(parent) = parent else { return Box::pin(async {}) };
    let c = coordinates.clone();
    let provider: BoundsProvider = Rc::new(move || match &bounds {
        Some(b) => b(),
        None => c.is_attached().then(|| Rect::from_offset_size(Offset::Zero, c.size().to_size())),
    });
    let mut node = parent.node.borrow_mut();
    match node.as_bring_into_view() {
        Some(b) => b.bring_into_view(coordinates, provider),
        None => Box::pin(async {}),
    }
}
