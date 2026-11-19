//! Focus search (Compose's FocusTraversal, OneDimensionalFocusSearch and TwoDimensionalFocusSearch): where a
//! direction takes the focus from a target — its custom destination, the next or previous target in placement
//! order (1D), or the best candidate that way by Android's beam search (2D) — looking beyond a lazy layout's
//! bounds when it says there is more.

use super::beyond_bounds::{BeyondBoundsDirection, search_beyond_bounds};
use super::requester::FocusRequester;
use super::target::{
    active_child, fetch_custom_enter, fetch_custom_exit, fetch_focus_properties, find_active_focus_node, focus_rect,
    focus_state, is_eligible_for_focus_search,
};
use super::{FocusDirection, FocusState, ancestors, children, focus_owner_of, nearest_ancestor};
use crate::modifier::{NodeKind, NodeRc};
use crate::node::LayoutNode;
use kompoz_graphics::{LayoutDirection, Rect};
use std::cmp::Ordering;
use std::rc::Rc;

/// Where `direction` goes from `cell` by its properties (Compose's `customFocusSearch`): a requester,
/// `Default` (the usual search), `Cancel`, or `Redirect` (an enter or exit moved the focus itself).
pub(crate) fn custom_focus_search(
    cell: &NodeRc,
    direction: FocusDirection,
    layout_direction: LayoutDirection,
) -> FocusRequester {
    let p = fetch_focus_properties(cell);
    let ltr = layout_direction == LayoutDirection::Ltr;
    match direction {
        FocusDirection::Next => p.next,
        FocusDirection::Previous => p.previous,
        FocusDirection::Up => p.up,
        FocusDirection::Down => p.down,
        FocusDirection::Left => {
            let r = if ltr { p.start } else { p.end };
            if r == FocusRequester::Default { p.left } else { r }
        }
        FocusDirection::Right => {
            let r = if ltr { p.end } else { p.start };
            if r == FocusRequester::Default { p.right } else { r }
        }
        FocusDirection::Enter | FocusDirection::Exit => {
            let r = if direction == FocusDirection::Enter {
                fetch_custom_enter(cell, direction)
            } else {
                fetch_custom_exit(cell, direction)
            };
            r.unwrap_or(FocusRequester::Default)
        }
    }
}

/// Searches `direction` from `cell` (a window's root target), `on_found` asked of each candidate until it takes
/// one (Compose's `focusSearch`); none if the search was called off.
pub(crate) fn focus_search(
    cell: &NodeRc,
    direction: FocusDirection,
    layout_direction: LayoutDirection,
    previously_focused_rect: Option<Rect>,
    on_found: &mut dyn FnMut(&NodeRc) -> bool,
) -> Option<bool> {
    match direction {
        FocusDirection::Next | FocusDirection::Previous => {
            Some(one_dimensional_focus_search(cell, direction, on_found))
        }
        FocusDirection::Left | FocusDirection::Right | FocusDirection::Up | FocusDirection::Down => {
            two_dimensional_focus_search(cell, direction, previously_focused_rect, on_found)
        }
        FocusDirection::Enter => {
            let d = if layout_direction == LayoutDirection::Rtl { FocusDirection::Left } else { FocusDirection::Right };
            let active = scoped_active(cell)?;
            two_dimensional_focus_search(&active, d, previously_focused_rect, on_found)
        }
        FocusDirection::Exit => {
            let parent = scoped_active(cell).and_then(|a| find_non_deactivated_parent(&a));
            Some(match parent {
                None => false,
                Some(p) if Rc::ptr_eq(&p, cell) => false,
                Some(p) => on_found(&p),
            })
        }
    }
}

/// The active node, if it is below `root` (a window's root target).
pub(crate) fn scoped_active(root: &NodeRc) -> Option<NodeRc> {
    let active = find_active_focus_node(root)?;
    if Rc::ptr_eq(&active, root) || ancestors(&active, NodeKind::FocusTarget).iter().any(|a| Rc::ptr_eq(a, root)) {
        Some(active)
    } else {
        None
    }
}

fn find_non_deactivated_parent(cell: &NodeRc) -> Option<NodeRc> {
    ancestors(cell, NodeKind::FocusTarget).into_iter().find(|a| fetch_focus_properties(a).can_focus)
}

// ---------------------------------------------------------------------------------------------------------------
// One dimension

fn one_dimensional_focus_search(
    cell: &NodeRc,
    direction: FocusDirection,
    on_found: &mut dyn FnMut(&NodeRc) -> bool,
) -> bool {
    match direction {
        FocusDirection::Next => forward_focus_search(cell, on_found),
        FocusDirection::Previous => backward_focus_search(cell, on_found),
        _ => panic!("This function should only be used for 1-D focus search"),
    }
}

fn forward_focus_search(cell: &NodeRc, on_found: &mut dyn FnMut(&NodeRc) -> bool) -> bool {
    match focus_state(cell) {
        FocusState::ActiveParent => {
            let child = active_child(cell).expect("ActiveParent must have a focusedChild");
            forward_focus_search(&child, on_found)
                || generate_and_search_children_1d(cell, &child, FocusDirection::Next, on_found)
        }
        FocusState::Active | FocusState::Captured => pick_child_for_forward_search(cell, on_found),
        FocusState::Inactive => {
            if fetch_focus_properties(cell).can_focus {
                on_found(cell)
            } else {
                pick_child_for_forward_search(cell, on_found)
            }
        }
    }
}

fn backward_focus_search(cell: &NodeRc, on_found: &mut dyn FnMut(&NodeRc) -> bool) -> bool {
    match focus_state(cell) {
        FocusState::ActiveParent => {
            let child = active_child(cell).expect("ActiveParent must have a focusedChild");
            match focus_state(&child) {
                FocusState::ActiveParent => {
                    backward_focus_search(&child, on_found)
                        || generate_and_search_children_1d(cell, &child, FocusDirection::Previous, on_found)
                        || (fetch_focus_properties(&child).can_focus && on_found(&child))
                }
                FocusState::Active | FocusState::Captured => {
                    generate_and_search_children_1d(cell, &child, FocusDirection::Previous, on_found)
                }
                FocusState::Inactive => panic!("ActiveParent must have a focusedChild"),
            }
        }
        FocusState::Active | FocusState::Captured => pick_child_for_backward_search(cell, on_found),
        FocusState::Inactive => {
            pick_child_for_backward_search(cell, on_found) || (fetch_focus_properties(cell).can_focus && on_found(cell))
        }
    }
}

fn generate_and_search_children_1d(
    cell: &NodeRc,
    focused: &NodeRc,
    direction: FocusDirection,
    on_found: &mut dyn FnMut(&NodeRc) -> bool,
) -> bool {
    if search_children_1d(cell, focused, direction, on_found) {
        return true;
    }
    let owner = focus_owner_of(cell);
    let before = owner.as_ref().and_then(|o| o.active_focus_target());
    let bb =
        if direction == FocusDirection::Next { BeyondBoundsDirection::After } else { BeyondBoundsDirection::Before };
    search_beyond_bounds(cell, bb, &mut |has_more| {
        let now = owner.as_ref().and_then(|o| o.active_focus_target());
        if !same(&before, &now) {
            return Some(true);
        }
        let found = search_children_1d(cell, focused, direction, on_found);
        if found || !has_more { Some(found) } else { None }
    })
    .unwrap_or(false)
}

fn same(a: &Option<NodeRc>, b: &Option<NodeRc>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

fn search_children_1d(
    cell: &NodeRc,
    focused: &NodeRc,
    direction: FocusDirection,
    on_found: &mut dyn FnMut(&NodeRc) -> bool,
) -> bool {
    assert!(
        focus_state(cell) == FocusState::ActiveParent,
        "This function should only be used within a parent that has focus."
    );
    let mut kids = children(cell, NodeKind::FocusTarget);
    kids.sort_by(focusable_children_order);
    let at = kids.iter().position(|k| Rc::ptr_eq(k, focused));
    match direction {
        FocusDirection::Next => {
            if let Some(i) = at {
                for child in &kids[i + 1..] {
                    if is_eligible_for_focus_search(child) && forward_focus_search(child, on_found) {
                        return true;
                    }
                }
            }
        }
        FocusDirection::Previous => {
            if let Some(i) = at {
                for child in kids[..i].iter().rev() {
                    if is_eligible_for_focus_search(child) && backward_focus_search(child, on_found) {
                        return true;
                    }
                }
            }
        }
        _ => panic!("This function should only be used for 1-D focus search"),
    }
    if direction == FocusDirection::Next || !fetch_focus_properties(cell).can_focus || is_root(cell) {
        return false;
    }
    on_found(cell)
}

fn pick_child_for_forward_search(cell: &NodeRc, on_found: &mut dyn FnMut(&NodeRc) -> bool) -> bool {
    let mut kids = children(cell, NodeKind::FocusTarget);
    kids.sort_by(focusable_children_order);
    kids.iter().any(|k| is_eligible_for_focus_search(k) && forward_focus_search(k, on_found))
}

fn pick_child_for_backward_search(cell: &NodeRc, on_found: &mut dyn FnMut(&NodeRc) -> bool) -> bool {
    let mut kids = children(cell, NodeKind::FocusTarget);
    kids.sort_by(focusable_children_order);
    kids.iter().rev().any(|k| is_eligible_for_focus_search(k) && backward_focus_search(k, on_found))
}

fn is_root(cell: &NodeRc) -> bool {
    nearest_ancestor(cell, NodeKind::FocusTarget).is_none()
}

/// Compose's FocusableChildrenComparator: eligible ones first; between those, by the place order of their layouts
/// where their paths from the root part.
fn focusable_children_order(a: &NodeRc, b: &NodeRc) -> Ordering {
    let (ea, eb) = (is_eligible_for_focus_search(a), is_eligible_for_focus_search(b));
    if !ea || !eb {
        return match (ea, eb) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => Ordering::Equal,
        };
    }
    let (Some(la), Some(lb)) = (a.layout_node(), b.layout_node()) else { return Ordering::Equal };
    if Rc::ptr_eq(&la, &lb) {
        return Ordering::Equal;
    }
    let (pa, pb) = (path_from_root(&la), path_from_root(&lb));
    for (x, y) in pa.iter().zip(pb.iter()) {
        if !Rc::ptr_eq(x, y) {
            return x.place_order.get().cmp(&y.place_order.get());
        }
    }
    panic!("Could not find a common ancestor between the two FocusModifiers.")
}

fn path_from_root(node: &Rc<LayoutNode>) -> Vec<Rc<LayoutNode>> {
    let mut path = vec![node.clone()];
    let mut n = node.parent();
    while let Some(p) = n {
        path.push(p.clone());
        n = p.parent();
    }
    path.reverse();
    path
}

// ---------------------------------------------------------------------------------------------------------------
// Two dimensions

pub(crate) fn two_dimensional_focus_search(
    cell: &NodeRc,
    direction: FocusDirection,
    previously_focused_rect: Option<Rect>,
    on_found: &mut dyn FnMut(&NodeRc) -> bool,
) -> Option<bool> {
    match focus_state(cell) {
        FocusState::Inactive => Some(if fetch_focus_properties(cell).can_focus {
            on_found(cell)
        } else if let Some(rect) = previously_focused_rect {
            search_children_2d(cell, rect, direction, on_found)
        } else {
            find_child_corresponding_to_focus_enter(cell, direction, on_found)
        }),
        FocusState::ActiveParent => {
            let child = active_child(cell).expect("ActiveParent must have a focusedChild");
            match focus_state(&child) {
                FocusState::ActiveParent => {
                    let found = two_dimensional_focus_search(&child, direction, previously_focused_rect, on_found);
                    if found != Some(false) {
                        return found;
                    }
                    let rect = previously_focused_rect.unwrap_or_else(|| {
                        let active = find_active_focus_node(&child).expect("ActiveParent must have a focusedChild");
                        focus_rect(&active)
                    });
                    Some(generate_and_search_children_2d(cell, rect, direction, on_found))
                }
                FocusState::Active | FocusState::Captured => {
                    let rect = previously_focused_rect.unwrap_or_else(|| focus_rect(&child));
                    Some(generate_and_search_children_2d(cell, rect, direction, on_found))
                }
                FocusState::Inactive => panic!("ActiveParent must have a focusedChild"),
            }
        }
        FocusState::Active | FocusState::Captured => {
            Some(find_child_corresponding_to_focus_enter(cell, direction, on_found))
        }
    }
}

/// The child entering `cell` the way `direction` goes lands on (Compose's `findChildCorrespondingToFocusEnter`):
/// its only one, or the best candidate from its edge that way starts at.
pub(crate) fn find_child_corresponding_to_focus_enter(
    cell: &NodeRc,
    direction: FocusDirection,
    on_found: &mut dyn FnMut(&NodeRc) -> bool,
) -> bool {
    let mut focusable = Vec::new();
    collect_accessible_children(cell, &mut focusable);
    if focusable.len() <= 1 {
        return focusable.first().is_some_and(&mut *on_found);
    }
    let requested = if direction == FocusDirection::Enter { FocusDirection::Right } else { direction };
    let r = focus_rect(cell);
    let initial = match requested {
        FocusDirection::Right | FocusDirection::Down => Rect::new(r.left, r.top, r.left, r.top),
        FocusDirection::Left | FocusDirection::Up => Rect::new(r.right, r.bottom, r.right, r.bottom),
        _ => panic!("This function should only be used for 2-D focus search"),
    };
    find_best_candidate(&focusable, initial, requested).is_some_and(|c| on_found(&c))
}

fn generate_and_search_children_2d(
    cell: &NodeRc,
    focused: Rect,
    direction: FocusDirection,
    on_found: &mut dyn FnMut(&NodeRc) -> bool,
) -> bool {
    if search_children_2d(cell, focused, direction, on_found) {
        return true;
    }
    let owner = focus_owner_of(cell);
    let before = owner.as_ref().and_then(|o| o.active_focus_target());
    let bb = match direction {
        FocusDirection::Up => BeyondBoundsDirection::Above,
        FocusDirection::Down => BeyondBoundsDirection::Below,
        FocusDirection::Left => BeyondBoundsDirection::Left,
        FocusDirection::Right => BeyondBoundsDirection::Right,
        _ => panic!("Unsupported direction for beyond bounds layout"),
    };
    search_beyond_bounds(cell, bb, &mut |has_more| {
        let now = owner.as_ref().and_then(|o| o.active_focus_target());
        if !same(&before, &now) {
            return Some(true);
        }
        let found = search_children_2d(cell, focused, direction, on_found);
        if found || !has_more { Some(found) } else { None }
    })
    .unwrap_or(false)
}

fn search_children_2d(
    cell: &NodeRc,
    focused: Rect,
    direction: FocusDirection,
    on_found: &mut dyn FnMut(&NodeRc) -> bool,
) -> bool {
    let mut kids: Vec<NodeRc> =
        children(cell, NodeKind::FocusTarget).into_iter().filter(|c| c.meta.attached.get()).collect();
    while !kids.is_empty() {
        let Some(next) = find_best_candidate(&kids, focused, direction) else { return false };
        if fetch_focus_properties(&next).can_focus {
            return on_found(&next);
        }
        if generate_and_search_children_2d(&next, focused, direction, on_found) {
            return true;
        }
        kids.retain(|k| !Rc::ptr_eq(k, &next));
    }
    false
}

fn collect_accessible_children(cell: &NodeRc, out: &mut Vec<NodeRc>) {
    for child in children(cell, NodeKind::FocusTarget) {
        if !child.meta.attached.get() {
            continue;
        }
        // (what is in a layout taking no input: KompozR's PointerInputModifierNode::blocks_input)
        if child.layout_node().is_some_and(|n| crate::input::input_blocked_at(&n)) {
            continue;
        }
        if fetch_focus_properties(&child).can_focus {
            out.push(child);
        } else {
            collect_accessible_children(&child, out);
        }
    }
}

fn find_best_candidate(candidates: &[NodeRc], focused: Rect, direction: FocusDirection) -> Option<NodeRc> {
    let mut best = match direction {
        FocusDirection::Left => focused.translate(kompoz_graphics::Offset::new(focused.width() + 1.0, 0.0)),
        FocusDirection::Right => focused.translate(kompoz_graphics::Offset::new(-(focused.width() + 1.0), 0.0)),
        FocusDirection::Up => focused.translate(kompoz_graphics::Offset::new(0.0, focused.height() + 1.0)),
        FocusDirection::Down => focused.translate(kompoz_graphics::Offset::new(0.0, -(focused.height() + 1.0))),
        _ => panic!("This function should only be used for 2-D focus search"),
    };
    let mut result = None;
    for candidate in candidates {
        if is_eligible_for_focus_search(candidate) {
            let rect = focus_rect(candidate);
            if is_better_candidate(rect, best, focused, direction) {
                best = rect;
                result = Some(candidate.clone());
            }
        }
    }
    result
}

/// Whether `proposed` is a better candidate than `current` to move `direction` from `focused` to (Compose's
/// `isBetterCandidate`, Android's FocusFinder).
pub fn is_better_candidate(proposed: Rect, current: Rect, focused: Rect, direction: FocusDirection) -> bool {
    let is_candidate = |r: Rect| match direction {
        FocusDirection::Left => (focused.right > r.right || focused.left >= r.right) && focused.left > r.left,
        FocusDirection::Right => (focused.left < r.left || focused.right <= r.left) && focused.right < r.right,
        FocusDirection::Up => (focused.bottom > r.bottom || focused.top >= r.bottom) && focused.top > r.top,
        FocusDirection::Down => (focused.top < r.top || focused.bottom <= r.top) && focused.bottom < r.bottom,
        _ => panic!("This function should only be used for 2-D focus search"),
    };
    let major = |r: Rect| {
        let d = match direction {
            FocusDirection::Left => focused.left - r.right,
            FocusDirection::Right => r.left - focused.right,
            FocusDirection::Up => focused.top - r.bottom,
            FocusDirection::Down => r.top - focused.bottom,
            _ => panic!("This function should only be used for 2-D focus search"),
        };
        d.max(0.0)
    };
    let minor = |r: Rect| match direction {
        FocusDirection::Left | FocusDirection::Right => {
            (focused.top + focused.height() / 2.0) - (r.top + r.height() / 2.0)
        }
        _ => (focused.left + focused.width() / 2.0) - (r.left + r.width() / 2.0),
    };
    let weighted = |r: Rect| {
        let ma = major(r) as i64;
        let mi = minor(r) as i64;
        13 * ma * ma + mi * mi
    };
    if !is_candidate(proposed) {
        false
    } else if !is_candidate(current) || beam_beats(focused, proposed, current, direction) {
        true
    } else if beam_beats(focused, current, proposed, direction) {
        false
    } else {
        weighted(proposed) < weighted(current)
    }
}

fn beam_beats(source: Rect, rect1: Rect, rect2: Rect, direction: FocusDirection) -> bool {
    let in_source_beam = |r: Rect| match direction {
        FocusDirection::Left | FocusDirection::Right => r.bottom > source.top && r.top < source.bottom,
        _ => r.right > source.left && r.left < source.right,
    };
    let in_direction = |r: Rect| match direction {
        FocusDirection::Left => source.left >= r.right,
        FocusDirection::Right => source.right <= r.left,
        FocusDirection::Up => source.top >= r.bottom,
        FocusDirection::Down => source.bottom <= r.top,
        _ => panic!("This function should only be used for 2-D focus search"),
    };
    let major = |r: Rect| {
        let d = match direction {
            FocusDirection::Left => source.left - r.right,
            FocusDirection::Right => r.left - source.right,
            FocusDirection::Up => source.top - r.bottom,
            _ => r.top - source.bottom,
        };
        d.max(0.0)
    };
    let major_to_far_edge = |r: Rect| {
        let d = match direction {
            FocusDirection::Left => source.left - r.left,
            FocusDirection::Right => r.right - source.right,
            FocusDirection::Up => source.top - r.top,
            _ => r.bottom - source.bottom,
        };
        d.max(1.0)
    };
    if in_source_beam(rect2) || !in_source_beam(rect1) {
        false
    } else if !in_direction(rect2) || matches!(direction, FocusDirection::Left | FocusDirection::Right) {
        true
    } else {
        major(rect1) < major_to_far_edge(rect2)
    }
}
