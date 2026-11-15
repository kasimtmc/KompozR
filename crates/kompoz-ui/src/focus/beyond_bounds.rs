//! Laying out beyond a lazy layout's bounds for focus search (Compose's BeyondBoundsLayout and the focus side of
//! it): a lazy list composes the items past its edge a direction asks for, while the search looks for a target
//! among them.

use super::nearest_ancestor;
use crate::modifier::{NodeKind, NodeRc};
use std::rc::Rc;

/// Which way a layout is asked to lay out more (Compose's BeyondBoundsLayout.LayoutDirection).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BeyondBoundsDirection {
    Before,
    After,
    Left,
    Right,
    Above,
    Below,
}

/// What a beyond-bounds block is told (Compose's BeyondBoundsScope): whether there is more to lay out that way.
pub struct BeyondBoundsScope {
    pub has_more_content: bool,
}

/// A layout that can lay out more of its content past its bounds while asked (Compose's BeyondBoundsLayout, a
/// lazy layout's): it lays out a step more at a time, calling `block` after each, until the block gives an answer
/// (some) or there is no more; the extra content goes when it returns.
pub trait BeyondBoundsLayout {
    fn layout(
        &mut self,
        direction: BeyondBoundsDirection,
        block: &mut dyn FnMut(&BeyondBoundsScope) -> Option<bool>,
    ) -> Option<bool>;
}

/// The beyond-bounds layout above `cell` (Compose's `beyondBoundsLayoutParent`).
fn beyond_bounds_parent(cell: &NodeRc) -> Option<NodeRc> {
    nearest_ancestor(cell, NodeKind::BeyondBoundsLayout)
}

/// Searches beyond the bounds of the layout above `cell` (Compose's `searchBeyondBounds`): none if there is none,
/// or if the target above it shares it (the search there does it).
pub(crate) fn search_beyond_bounds(
    cell: &NodeRc,
    direction: BeyondBoundsDirection,
    block: &mut dyn FnMut(bool) -> Option<bool>,
) -> Option<bool> {
    let mine = beyond_bounds_parent(cell);
    if let Some(parent_target) = nearest_ancestor(cell, NodeKind::FocusTarget) {
        let theirs = beyond_bounds_parent(&parent_target);
        let same = match (&mine, &theirs) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if same {
            return None;
        }
    }
    let layout = mine?;
    let mut node = layout.node.borrow_mut();
    let b = node.as_beyond_bounds_layout()?;
    b.layout(direction, &mut |scope| block(scope.has_more_content))
}
