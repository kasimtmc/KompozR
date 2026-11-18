//! Scrolling a lazy layout to an item (Compose's LazyLayoutScrollScope.kt): a scroll scope that knows the items in
//! view, snaps to an item and tells the distance to one; `animate_scroll_to_item` glides there by a spring, heading
//! for where the item really is once it shows (its distance guessed from the average item size until then),
//! jumping to near it first when it is far (more than a hundred items past those in view).

use crate::gestures::ScrollScope;
use kompoz_animation::{AnimationScope, AnimationState, AnimationVector1D, float_animation_state};
use kompoz_graphics::{Density, Dp};
use kompoz_runtime::named_args;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// A [`ScrollScope`] of a lazy layout (Compose's LazyLayoutScrollScope): what is in view, and how to get to an item.
pub trait LazyLayoutScrollScope: ScrollScope {
    /// The first item in view.
    fn first_visible_item_index(&self) -> i32;
    /// How far it is scrolled out of view, px.
    fn first_visible_item_scroll_offset(&self) -> i32;
    /// The last item in view.
    fn last_visible_item_index(&self) -> i32;
    /// How many items there are.
    fn item_count(&self) -> i32;
    /// Makes item `index` the first in view, scrolled by `offset` px, at once (at the next measure).
    fn snap_to_item(&self, index: i32, offset: i32);
    /// How far item `target_index` (moved by `target_offset`) is from where the first in view is: exact for one in
    /// view, guessed from the average size otherwise.
    fn calculate_distance_to(&self, target_index: i32, target_offset: i32) -> i32;
}

/// A lazy layout's state that gives a [`LazyLayoutScrollScope`] of a scroll scope (what Compose's overloads of
/// `LazyLayoutScrollScope(state, scrollScope)` take: a list's, a grid's, a staggered grid's, a pager's state).
pub trait LazyLayoutScrollScopeState {
    fn lazy_layout_scroll_scope(&self, scroll_scope: Rc<dyn ScrollScope>) -> Rc<dyn LazyLayoutScrollScope>;
}

/// A lazy layout's scroll scope of `state` scrolling in `scroll_scope` (Compose's `LazyLayoutScrollScope(state,
/// scrollScope)`, one for each lazy layout's state): what animating to an item scrolls in.
/// Kotlin: `LazyLayoutScrollScope(state: LazyListState, scrollScope: ScrollScope)`
pub fn LazyLayoutScrollScope<S: LazyLayoutScrollScopeState + ?Sized>(
    state: &S,
    scroll_scope: Rc<dyn ScrollScope>,
) -> Rc<dyn LazyLayoutScrollScope> {
    state.lazy_layout_scroll_scope(scroll_scope)
}

/// Whether item `index` is in view.
pub(crate) fn is_item_visible(scope: &dyn LazyLayoutScrollScope, index: i32) -> bool {
    index >= scope.first_visible_item_index() && index <= scope.last_visible_item_index()
}

const TargetDistance: Dp = Dp(2500.0);
const BoundDistance: Dp = Dp(1500.0);
const MinimumDistance: Dp = Dp(50.0);

/// Glides to item `index`, scrolled by `scroll_offset` (Compose's `animateScrollToItem`): see the module's doc.
/// Ends where it can't go further (the end of the list).
pub(crate) async fn animate_scroll_to_item(
    scope: Rc<dyn LazyLayoutScrollScope>,
    index: i32,
    scroll_offset: i32,
    num_of_items_for_teleport: i32,
    density: Density,
) {
    assert!(index >= 0, "Index should be non-negative");
    let target_distance_px = density.to_px(TargetDistance);
    let bound_distance_px = density.to_px(BoundDistance);
    let min_distance_px = density.to_px(MinimumDistance);
    let mut anim: AnimationState<f32, AnimationVector1D> = float_animation_state(0.0);

    // What Compose throws once the item shows (ItemFoundInScroll): its offset and the animation's state.
    let found: Rc<RefCell<Option<(i32, AnimationState<f32, AnimationVector1D>)>>> = Rc::new(RefCell::new(None));

    if is_item_visible(&*scope, index) {
        // (in view already: straight to it)
        *found.borrow_mut() = Some((scope.calculate_distance_to(index, 0), anim.clone()));
    } else {
        let forward = index > scope.first_visible_item_index();
        let is_overshot = {
            let scope = scope.clone();
            Rc::new(move || {
                let first = scope.first_visible_item_index();
                let offset = scope.first_visible_item_scroll_offset();
                if forward {
                    first > index || (first == index && offset > scroll_offset)
                } else {
                    first < index || (first == index && offset < scroll_offset)
                }
            })
        };
        let looping = Rc::new(Cell::new(true));
        let mut loops = 1;
        while looping.get() && scope.item_count() > 0 && found.borrow().is_none() {
            let expected_distance = scope.calculate_distance_to(index, 0) + scroll_offset;
            let target = if (expected_distance.abs() as f32) < target_distance_px {
                let abs_target_px = (expected_distance.abs() as f32).max(min_distance_px);
                if forward { abs_target_px } else { -abs_target_px }
            } else if forward {
                target_distance_px
            } else {
                -target_distance_px
            };
            anim = named_args!(anim.copy_float(value = 0.0));
            let prev_value = Rc::new(Cell::new(0.0f32));
            let (s, f, o, l, p) = (scope.clone(), found.clone(), is_overshot.clone(), looping.clone(), prev_value);
            let anim_for_found = anim.clone();
            let this_loop = loops;
            let block: Box<dyn for<'a> FnMut(&'a AnimationScope<f32, AnimationVector1D>)> = Box::new(move |a| {
                // Not found yet: is it in view?
                if !is_item_visible(&*s, index) {
                    // (a spring can overshoot its target: clamped to the range)
                    let value = a.value();
                    let coerced_value = if target > 0.0 { value.min(target) } else { value.max(target) };
                    let delta = coerced_value - p.get();
                    let consumed = s.scroll_by(delta);
                    if is_item_visible(&*s, index) {
                        // (found by this scroll)
                    } else if !o() {
                        if delta != consumed {
                            // The end, the item not found.
                            a.cancel_animation();
                            l.set(false);
                            return;
                        }
                        p.set(p.get() + delta);
                        if forward {
                            if value > bound_distance_px {
                                a.cancel_animation();
                            }
                        } else if value < -bound_distance_px {
                            a.cancel_animation();
                        }
                        // Far still: a jump to near it.
                        if forward {
                            if this_loop >= 2 && index - s.last_visible_item_index() > num_of_items_for_teleport {
                                s.snap_to_item(index - num_of_items_for_teleport, 0);
                            }
                        } else if this_loop >= 2 && s.first_visible_item_index() - index > num_of_items_for_teleport {
                            s.snap_to_item(index + num_of_items_for_teleport, 0);
                        }
                    }
                }
                // (snapped to the end position: no glide to it needed)
                if o() {
                    s.snap_to_item(index, scroll_offset);
                    l.set(false);
                    a.cancel_animation();
                } else if is_item_visible(&*s, index) {
                    let target_item_offset = s.calculate_distance_to(index, 0);
                    *f.borrow_mut() = Some((target_item_offset, anim_for_found.clone()));
                    a.cancel_animation();
                }
            });
            let sequential = anim.velocity() != 0.0;
            named_args!(anim.animate_to(target, sequential_animation = sequential, block = block)).await;
            loops += 1;
        }
    }

    let Some((item_offset, previous_animation)) = found.borrow_mut().take() else { return };
    // Found: to the requested position (stopped where it can't go on).
    let anim = named_args!(previous_animation.copy_float(value = 0.0));
    let target = (item_offset + scroll_offset) as f32;
    let prev_value = Rc::new(Cell::new(0.0f32));
    let (s, p) = (scope.clone(), prev_value);
    let block: Box<dyn for<'a> FnMut(&'a AnimationScope<f32, AnimationVector1D>)> = Box::new(move |a| {
        // (a spring can overshoot its target: clamped to the range)
        let value = a.value();
        let coerced_value = if target > 0.0 {
            value.min(target)
        } else if target < 0.0 {
            value.max(target)
        } else {
            0.0
        };
        let delta = coerced_value - p.get();
        let consumed = s.scroll_by(delta);
        // (the end, or it would have overshot: stopped)
        if delta != consumed || coerced_value != value {
            a.cancel_animation();
        }
        p.set(p.get() + delta);
    });
    let sequential = anim.velocity() != 0.0;
    named_args!(anim.animate_to(target, sequential_animation = sequential, block = block)).await;
    // Exactly there in the end (rounding left aside, the item before would show by a hair).
    scope.snap_to_item(index, scroll_offset);
}
