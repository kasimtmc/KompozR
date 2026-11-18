//! Dragging between anchors, as Compose's AnchoredDraggable.kt: [`DraggableAnchors`] (a value's position each,
//! made with the [`DraggableAnchors()`] builder's `at`), an [`AnchoredDraggableState`] holding the offset, the
//! value it is at (`current_value`), settled at (`settled_value`) and heading for (`target_value`), dragged in its
//! mutation ([`AnchoredDraggableState::anchored_drag`], restarted when the anchors change), animated or snapped to a
//! value; `Modifier.anchored_draggable` drags it along an axis and settles it with a fling that snaps to an anchor
//! ([`AnchoredDraggableDefaults::fling_behavior`]: Compose's snapping fling over the anchors).

// (Compose's parameter lists: the methods of a `#[named]` trait carry no attributes of their own)
#![allow(clippy::too_many_arguments)]

use super::Orientation;
use super::draggable::{DragDelta, DragGestureCore, DragGestureOwner, DragStopped, ForEachDelta};
use super::fling::{FlingBehavior, TargetedFlingBehavior, same_fling_behavior};
use super::scrollable_state::{LocalFuture, ScrollScope};
use super::snapping::{SnapLayoutInfoProvider, snap_fling_behavior};
use crate::interaction::MutableInteractionSource;
use crate::overscroll::{OverscrollEffect, PerformFling, same_effect};
use kompoz_animation::{
    AnimationScope, AnimationSpec, AnimationVector1D, DecayAnimationSpec, FloatDecayAnimationSpec, FloatDecayTarget,
    MutatePriority, MutatorMutex, animate, exponential_decay, float_animation_state, generate_decay_animation_spec,
    tween,
};
use kompoz_coroutines::{Job, catch_cancellation, snapshot_flow};
use kompoz_graphics::{Density, Dp, IntSize, LayoutDirection, Offset};
use kompoz_runtime::kt::panic_kt;
use kompoz_runtime::{
    MutableState, Saveable, Saver, mutable_float_state_of, mutable_state_of, named, named_args, remember_keyed,
};
use kompoz_ui::input::{
    IndirectPointerEvent, IndirectPointerInputModifierNode, NestedScrollSource, PointerEvent, PointerEventPass,
    PointerInputModifierNode, PointerType,
};
use kompoz_ui::platform::LocalDensity;
use kompoz_ui::unit::Velocity;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::rc::{Rc, Weak};
use std::sync::Arc;

/// What anchors are keyed by (Kotlin's `T` of an anchored state): compared, cloned, held in states.
pub trait AnchorKey: Clone + PartialEq + Send + Sync + 'static {}
impl<T: Clone + PartialEq + Send + Sync + 'static> AnchorKey for T {}

/// Kotlin's `coerceIn` of a float: NaN bounds keep the value; a range the wrong way round panics as Kotlin
/// throws.
#[track_caller]
fn coerce_in(v: f32, minimum_value: f32, maximum_value: f32) -> f32 {
    if minimum_value > maximum_value {
        panic_kt("IllegalArgumentException", "Cannot coerce value to an empty range: maximum is less than minimum.");
    }
    if v < minimum_value {
        minimum_value
    } else if v > maximum_value {
        maximum_value
    } else {
        v
    }
}

/// Kotlin's `minOf`/`maxOf` of floats (Java's Math.min/max): NaN wins.
fn kt_min(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() { f32::NAN } else { a.min(b) }
}

fn kt_max(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() { f32::NAN } else { a.max(b) }
}

// ---------------------------------------------------------------------------------------------------------------
// The anchors

/// Values and their positions, px (Compose's DraggableAnchors): what an anchored state settles on. Two compare as
/// their types say ([`DraggableAnchorsEq`]: equal when `PartialEq` says so); shared as `Arc<dyn
/// DraggableAnchors<T>>`.
pub trait DraggableAnchors<T>: DraggableAnchorsEq + Send + Sync + 'static {
    /// How many anchors (Compose's `size`).
    fn size(&self) -> usize;

    /// Where `anchor` is; NaN when it isn't one (Compose's `positionOf`).
    fn position_of(&self, anchor: &T) -> f32;

    /// Whether `anchor` is one (Compose's `hasPositionFor`).
    fn has_position_for(&self, anchor: &T) -> bool;

    /// The anchor closest to `position` (Compose's `closestAnchor(position)`).
    fn closest_anchor(&self, position: f32) -> Option<T>;

    /// The anchor closest to `position` at or past it, upwards (greater) or downwards (Compose's
    /// `closestAnchor(position, searchUpwards)`).
    /// Kotlin: `closestAnchor(position: Float, searchUpwards: Boolean): T?`
    fn closest_anchor_in_direction(&self, position: f32, search_upwards: bool) -> Option<T>;

    /// The smallest position; NaN when there is none (Compose's `minPosition`).
    fn min_position(&self) -> f32;

    /// The greatest position; NaN when there is none (Compose's `maxPosition`).
    fn max_position(&self) -> f32;

    /// The anchor at `index`, in the order they were given (Compose's `anchorAt`).
    fn anchor_at(&self, index: usize) -> Option<T>;

    /// The position at `index`; NaN past them (Compose's `positionAt`).
    fn position_at(&self, index: usize) -> f32;

    /// Each anchor with its position, in order (Compose's `forEach`).
    fn for_each(&self, block: &mut dyn FnMut(T, f32)) {
        for i in 0..self.size() {
            let key = self
                .anchor_at(i)
                .unwrap_or_else(|| panic_kt("IllegalArgumentException", &format!("There was no key at index {i}.")));
            block(key, self.position_at(i));
        }
    }
}

/// How anchors compare (a blanket impl for every `PartialEq` one).
#[doc(hidden)]
pub trait DraggableAnchorsEq {
    fn draggable_anchors_eq(&self, other: &dyn Any) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<A: PartialEq + Any> DraggableAnchorsEq for A {
    fn draggable_anchors_eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<A>().is_some_and(|o| self == o)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<T: 'static> PartialEq for dyn DraggableAnchors<T> {
    fn eq(&self, other: &dyn DraggableAnchors<T>) -> bool {
        self.draggable_anchors_eq(DraggableAnchorsEq::as_any(other))
    }
}

impl<T: 'static> fmt::Debug for dyn DraggableAnchors<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DraggableAnchors(size={})", self.size())
    }
}

/// Shared anchors are anchors (what [`DraggableAnchors::position_of`] & co. of `state.anchors()` call).
impl<T: AnchorKey> DraggableAnchors<T> for Arc<dyn DraggableAnchors<T>> {
    fn size(&self) -> usize {
        (**self).size()
    }
    fn position_of(&self, anchor: &T) -> f32 {
        (**self).position_of(anchor)
    }
    fn has_position_for(&self, anchor: &T) -> bool {
        (**self).has_position_for(anchor)
    }
    fn closest_anchor(&self, position: f32) -> Option<T> {
        (**self).closest_anchor(position)
    }
    fn closest_anchor_in_direction(&self, position: f32, search_upwards: bool) -> Option<T> {
        (**self).closest_anchor_in_direction(position, search_upwards)
    }
    fn min_position(&self) -> f32 {
        (**self).min_position()
    }
    fn max_position(&self) -> f32 {
        (**self).max_position()
    }
    fn anchor_at(&self, index: usize) -> Option<T> {
        (**self).anchor_at(index)
    }
    fn position_at(&self, index: usize) -> f32 {
        (**self).position_at(index)
    }
}

/// Anchors given as any kind, shared (shared ones as they are).
fn share_anchors<T: AnchorKey>(anchors: impl DraggableAnchors<T>) -> Arc<dyn DraggableAnchors<T>> {
    let any: &dyn Any = DraggableAnchorsEq::as_any(&anchors);
    match any.downcast_ref::<Arc<dyn DraggableAnchors<T>>>() {
        Some(shared) => shared.clone(),
        None => Arc::new(anchors),
    }
}

/// What the [`DraggableAnchors()`] builder is given (Compose's DraggableAnchorsConfig): `key at position`.
pub struct DraggableAnchorsConfig<T> {
    keys: Vec<T>,
    positions: Vec<f32>,
}

impl<T> DraggableAnchorsConfig<T> {
    /// `key` at `position` px (Compose's infix `at`: `Start at 0f` in a `DraggableAnchors { }` block). A key given
    /// twice is there twice (the first one's position is its).
    pub fn at(&mut self, key: T, position: f32) {
        self.keys.push(key);
        self.positions.push(position);
    }
}

/// The anchors `builder` gives (Compose's `DraggableAnchors { … }`): `config.at(key, position)` each.
pub fn DraggableAnchors<T: AnchorKey>(
    builder: impl FnOnce(&mut DraggableAnchorsConfig<T>),
) -> DefaultDraggableAnchors<T> {
    let mut config = DraggableAnchorsConfig { keys: Vec::new(), positions: Vec::new() };
    builder(&mut config);
    DefaultDraggableAnchors::new(config.keys, config.positions)
}

/// Anchors of keys and their positions (Compose's DefaultDraggableAnchors): a cheap handle, equal to another of the
/// same keys and positions.
#[derive(Clone)]
pub struct DefaultDraggableAnchors<T> {
    inner: Arc<(Vec<T>, Vec<f32>)>,
}

impl<T: PartialEq> PartialEq for DefaultDraggableAnchors<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
            || (self.inner.0 == other.inner.0
                && self.inner.1.len() == other.inner.1.len()
                // (Kotlin's FloatArray.contentEquals: by their bits)
                && self.inner.1.iter().zip(&other.inner.1).all(|(a, b)| a.to_bits() == b.to_bits()))
    }
}

impl<T: fmt::Debug> fmt::Debug for DefaultDraggableAnchors<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DraggableAnchors(anchors={{")?;
        for (i, (k, p)) in self.inner.0.iter().zip(&self.inner.1).enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{k:?}={p}")?;
        }
        write!(f, "}})")
    }
}

impl<T> DefaultDraggableAnchors<T> {
    fn new(keys: Vec<T>, anchors: Vec<f32>) -> DefaultDraggableAnchors<T> {
        if keys.len() != anchors.len() {
            panic_kt(
                "IllegalArgumentException",
                "DraggableAnchors were constructed with inconsistent key-value sizes.",
            );
        }
        DefaultDraggableAnchors { inner: Arc::new((keys, anchors)) }
    }

    fn index_of(&self, anchor: &T) -> Option<usize>
    where
        T: PartialEq,
    {
        self.inner.0.iter().position(|k| k == anchor)
    }
}

impl<T: AnchorKey> DraggableAnchors<T> for DefaultDraggableAnchors<T> {
    fn size(&self) -> usize {
        self.inner.1.len()
    }

    fn position_of(&self, anchor: &T) -> f32 {
        self.index_of(anchor).map_or(f32::NAN, |i| self.inner.1[i])
    }

    fn has_position_for(&self, anchor: &T) -> bool {
        self.index_of(anchor).is_some()
    }

    fn closest_anchor(&self, position: f32) -> Option<T> {
        let mut min_anchor_index = None;
        let mut min_distance = f32::INFINITY;
        for (index, &anchor_position) in self.inner.1.iter().enumerate() {
            let distance = (position - anchor_position).abs();
            if distance <= min_distance {
                min_anchor_index = Some(index);
                min_distance = distance;
            }
        }
        min_anchor_index.map(|i| self.inner.0[i].clone())
    }

    fn closest_anchor_in_direction(&self, position: f32, search_upwards: bool) -> Option<T> {
        let mut min_anchor_index = None;
        let mut min_distance = f32::INFINITY;
        for (index, &anchor_position) in self.inner.1.iter().enumerate() {
            let delta = if search_upwards { anchor_position - position } else { position - anchor_position };
            let distance = if delta < 0.0 { f32::INFINITY } else { delta };
            if distance <= min_distance {
                min_anchor_index = Some(index);
                min_distance = distance;
            }
        }
        min_anchor_index.map(|i| self.inner.0[i].clone())
    }

    fn min_position(&self) -> f32 {
        let p = &self.inner.1;
        if p.is_empty() { f32::NAN } else { p[1..].iter().fold(p[0], |m, &e| kt_min(m, e)) }
    }

    fn max_position(&self) -> f32 {
        let p = &self.inner.1;
        if p.is_empty() { f32::NAN } else { p[1..].iter().fold(p[0], |m, &e| kt_max(m, e)) }
    }

    fn anchor_at(&self, index: usize) -> Option<T> {
        self.inner.0.get(index).cloned()
    }

    fn position_at(&self, index: usize) -> f32 {
        self.inner.1.get(index).copied().unwrap_or(f32::NAN)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The state

/// What an anchored drag drags with (Compose's AnchoredDragScope).
pub trait AnchoredDragScope {
    /// Puts the offset at `new_offset`, noting `last_known_velocity` (Compose's `dragTo`): the current value moves
    /// on to the next anchor once it is past half way (if the state confirms it).
    /// Kotlin: `dragTo(newOffset: Float, lastKnownVelocity: Float = 0f)`
    fn drag_to(&self, new_offset: f32, last_known_velocity: f32);
}

/// Whether a state takes a value (Compose's deprecated `confirmValueChange`).
pub type ConfirmValueChange<T> = Rc<dyn Fn(&T) -> bool>;

/// A state dragged between anchors (Compose's AnchoredDraggableState): its offset (NaN until it has anchors), the
/// value it is at (`current_value`: the nearest anchor once past half way to it), settled at (`settled_value`: an
/// anchor it came to rest on) and heading for (`target_value`), its anchors, the velocity of its last move. A
/// cheap handle, equal to its clones. Made by [`AnchoredDraggableState()`] and its overloads.
pub struct AnchoredDraggableState<T> {
    inner: Rc<StateInner<T>>,
}

impl<T: AnchorKey> Clone for AnchoredDraggableState<T> {
    fn clone(&self) -> Self {
        AnchoredDraggableState { inner: self.inner.clone() }
    }
}

impl<T: AnchorKey> PartialEq for AnchoredDraggableState<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T: AnchorKey + fmt::Debug> fmt::Debug for AnchoredDraggableState<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AnchoredDraggableState(current={:?}, offset={})", self.current_value(), self.offset())
    }
}

struct StateInner<T> {
    confirm_value_change: RefCell<ConfirmValueChange<T>>,
    positional_threshold: RefCell<Option<Rc<dyn Fn(f32) -> f32>>>,
    velocity_threshold: RefCell<Option<Rc<dyn Fn() -> f32>>>,
    snap_animation_spec: RefCell<Option<Arc<dyn AnimationSpec<f32>>>>,
    decay_animation_spec: RefCell<Option<Arc<dyn DecayAnimationSpec<f32>>>>,
    drag_mutex: MutatorMutex,
    current_value: MutableState<T>,
    settled_value: MutableState<T>,
    offset: MutableState<f32>,
    last_velocity: MutableState<f32>,
    drag_target: MutableState<Option<T>>,
    anchors: MutableState<Arc<dyn DraggableAnchors<T>>>,
    // (the drag scope's: Compose's anchoredDragScope)
    left_bound: RefCell<Option<T>>,
    right_bound: RefCell<Option<T>>,
    distance: Cell<f32>,
}

/// The state's drag scope (Compose's `anchoredDragScope`).
struct StateDragScope<T>(Weak<StateInner<T>>);

impl<T: AnchorKey> AnchoredDragScope for StateDragScope<T> {
    fn drag_to(&self, new_offset: f32, last_known_velocity: f32) {
        if let Some(inner) = self.0.upgrade() {
            AnchoredDraggableState { inner }.drag_to(new_offset, last_known_velocity);
        }
    }
}

fn empty_draggable_anchors<T: AnchorKey>() -> Arc<dyn DraggableAnchors<T>> {
    Arc::new(DefaultDraggableAnchors::<T>::new(Vec::new(), Vec::new()))
}

impl<T: AnchorKey> AnchoredDraggableState<T> {
    /// Compose's primary constructor: no anchors yet.
    fn make(initial_value: T) -> AnchoredDraggableState<T> {
        let inner = Rc::new(StateInner {
            confirm_value_change: RefCell::new(Rc::new(|_: &T| true)),
            positional_threshold: RefCell::new(None),
            velocity_threshold: RefCell::new(None),
            snap_animation_spec: RefCell::new(None),
            decay_animation_spec: RefCell::new(None),
            drag_mutex: MutatorMutex::new(),
            current_value: mutable_state_of(initial_value.clone()),
            settled_value: mutable_state_of(initial_value),
            offset: mutable_float_state_of(f32::NAN),
            last_velocity: mutable_float_state_of(0.0),
            drag_target: mutable_state_of(None),
            anchors: mutable_state_of(empty_draggable_anchors()),
            left_bound: RefCell::new(None),
            right_bound: RefCell::new(None),
            distance: Cell::new(f32::NAN),
        });
        AnchoredDraggableState { inner }
    }

    /// Compose's `calculateTargetValue` (its target value calculation fix on: ComposeFoundationFlags'
    /// `isAnchoredDraggableTargetValueCalculationFixEnabled`, on by default).
    fn calculate_target_value(&self, current_offset: f32) -> T {
        let current_value = self.inner.current_value.get();
        if current_offset.is_nan() {
            return current_value;
        }
        // Anchors may share an offset: the current value's has priority. (An anchored drag may move to a value not
        // among the anchors yet: its NaN position falls back to the current value.)
        let anchors = self.inner.anchors.get();
        let current_value_offset = anchors.position_of(&current_value);
        if current_value_offset.is_nan() || current_offset == current_value_offset {
            current_value
        } else {
            anchors.closest_anchor(current_offset).unwrap_or(current_value)
        }
    }

    fn confirm(&self, value: &T) -> bool {
        let confirm = self.inner.confirm_value_change.borrow().clone();
        confirm(value)
    }

    /// Whether the state was made with the thresholds and specs Compose's deprecated constructors take (its
    /// `usePreModifierChangeBehavior`): its fling is its own `settle(velocity)` then.
    fn use_pre_modifier_change_behavior(&self) -> bool {
        self.inner.positional_threshold.borrow().is_some()
            && self.inner.velocity_threshold.borrow().is_some()
            && self.inner.snap_animation_spec.borrow().is_some()
            && self.inner.decay_animation_spec.borrow().is_some()
    }

    /// The value it is at: the nearest anchor once a drag is past half way to it (Compose's `currentValue`).
    pub fn current_value(&self) -> T {
        self.inner.current_value.get()
    }

    /// The anchor it last came to rest on (Compose's `settledValue`).
    pub fn settled_value(&self) -> T {
        self.inner.settled_value.get()
    }

    /// Where it is heading: an animation's target, else the anchor nearest the offset (Compose's `targetValue`).
    /// Worked out as it is read (Compose's is a derived state: KompozR's made in a composition are worked out again
    /// only as the composition observes their inputs change, stale right after a write — an anchored drag to a
    /// value reads it as soon as it set its target); what it reads is recorded for its readers all the same.
    pub fn target_value(&self) -> T {
        match self.inner.drag_target.get() {
            Some(t) => t,
            None => self.calculate_target_value(self.offset()),
        }
    }

    /// Its offset, px; NaN before it has anchors (Compose's `offset`).
    pub fn offset(&self) -> f32 {
        self.inner.offset.get()
    }

    /// Its offset, which must be known (Compose's `requireOffset`: it panics when read before the state has
    /// anchors, as Compose throws).
    #[track_caller]
    pub fn require_offset(&self) -> f32 {
        let offset = self.offset();
        if offset.is_nan() {
            panic_kt(
                "IllegalStateException",
                "The offset was read before being initialized. Did you access the offset in a phase before layout, \
                 like effects or composition?",
            );
        }
        offset
    }

    /// Whether an animation to a value runs (Compose's `isAnimationRunning`).
    pub fn is_animation_running(&self) -> bool {
        self.inner.drag_target.get().is_some()
    }

    /// How far it is from `from`'s anchor to `to`'s, 0 to 1 (Compose's `progress(from, to)`; 1 for two anchors at
    /// one place).
    pub fn progress(&self, from: &T, to: &T) -> f32 {
        let anchors = self.inner.anchors.get();
        let from_offset = anchors.position_of(from);
        let to_offset = anchors.position_of(to);
        let current_offset = coerce_in(self.offset(), kt_min(from_offset, to_offset), kt_max(from_offset, to_offset));
        let fraction = (current_offset - from_offset) / (to_offset - from_offset);
        if !fraction.is_nan() {
            // (very close to 0 or 1: rounded to it)
            if fraction < 1e-6 {
                0.0
            } else if fraction > 1.0 - 1e-6 {
                1.0
            } else {
                fraction.abs()
            }
        } else {
            1.0
        }
    }

    /// How far it is from the settled value's anchor to the target's (Compose's deprecated `progress` property:
    /// use [`progress`](Self::progress) of two values).
    pub fn progress_to_target(&self) -> f32 {
        let anchors = self.inner.anchors.get();
        let a = anchors.position_of(&self.settled_value());
        let b = anchors.position_of(&self.target_value());
        let distance = (b - a).abs();
        if !distance.is_nan() && distance > 1e-6 {
            let progress = (self.require_offset() - a) / (b - a);
            if progress < 1e-6 {
                0.0
            } else if progress > 1.0 - 1e-6 {
                1.0
            } else {
                progress
            }
        } else {
            1.0
        }
    }

    /// The velocity of its last move, px/s (Compose's `lastVelocity`).
    pub fn last_velocity(&self) -> f32 {
        self.inner.last_velocity.get()
    }

    /// Its anchors (Compose's `anchors`).
    pub fn anchors(&self) -> Arc<dyn DraggableAnchors<T>> {
        self.inner.anchors.get()
    }

    /// Whether `value` is taken (Compose's internal `confirmValueChange`).
    pub fn confirm_value_change(&self, value: &T) -> bool {
        self.confirm(value)
    }

    /// The offset `delta` px from where it is, kept within its anchors (Compose's internal `newOffsetForDelta`).
    pub(crate) fn new_offset_for_delta(&self, delta: f32) -> f32 {
        let anchors = self.inner.anchors.get();
        let offset = self.offset();
        coerce_in((if offset.is_nan() { 0.0 } else { offset }) + delta, anchors.min_position(), anchors.max_position())
    }

    /// Moves it by `delta` px at once, within its anchors, outside of any mutation: how far it went (Compose's
    /// `dispatchRawDelta`).
    pub fn dispatch_raw_delta(&self, delta: f32) -> f32 {
        let new_offset = self.new_offset_for_delta(delta);
        let consumed_delta = new_offset - self.require_offset();
        self.drag_to(new_offset, 0.0);
        consumed_delta
    }

    /// The drag scope's `dragTo` (Compose's `anchoredDragScope.dragTo`).
    fn drag_to(&self, new_offset: f32, last_known_velocity: f32) {
        let previous_offset = self.offset();
        self.inner.offset.set(new_offset);
        self.inner.last_velocity.set(last_known_velocity);
        if previous_offset.is_nan() {
            return;
        }
        let is_moving_forward = new_offset >= previous_offset;
        self.update_if_needed(is_moving_forward);
    }

    /// Compose's `updateIfNeeded`: past half way to the next anchor, the current value is it (if confirmed).
    fn update_if_needed(&self, is_moving_forward: bool) {
        self.update_bounds(is_moving_forward);
        let anchors = self.inner.anchors.get();
        let current_value = self.current_value();
        let distance_to_current_anchor = (self.offset() - anchors.position_of(&current_value)).abs();
        let crossed_threshold = distance_to_current_anchor >= self.inner.distance.get() / 2.0;
        if crossed_threshold {
            let bound = if is_moving_forward {
                self.inner.right_bound.borrow().clone()
            } else {
                self.inner.left_bound.borrow().clone()
            };
            let closest_anchor = bound.unwrap_or(current_value);
            if self.confirm(&closest_anchor) {
                self.inner.current_value.set(closest_anchor);
            }
        }
    }

    /// Compose's `updateBounds`: the anchors either side of the offset (from the current value's, when at it).
    fn update_bounds(&self, is_moving_forward: bool) {
        let anchors = self.inner.anchors.get();
        let current_value = self.current_value();
        let offset = self.offset();
        let current_anchor_position = anchors.position_of(&current_value);
        let (left, right) = if offset == current_anchor_position {
            let search_start_position = offset + if is_moving_forward { 1.0 } else { -1.0 };
            let closest_excluding_current = anchors
                .closest_anchor_in_direction(search_start_position, is_moving_forward)
                .unwrap_or_else(|| current_value.clone());
            if is_moving_forward {
                (current_value, closest_excluding_current)
            } else {
                (closest_excluding_current, current_value)
            }
        } else {
            let closest_left =
                anchors.closest_anchor_in_direction(offset, false).unwrap_or_else(|| current_value.clone());
            let closest_right = anchors.closest_anchor_in_direction(offset, true).unwrap_or(current_value);
            (closest_left, closest_right)
        };
        self.inner.distance.set((anchors.position_of(&left) - anchors.position_of(&right)).abs());
        *self.inner.left_bound.borrow_mut() = Some(left);
        *self.inner.right_bound.borrow_mut() = Some(right);
    }

    /// Compose's `trySnapTo`: at once if no drag holds the state (whether it could).
    fn try_snap_to(&self, target_value: T) -> bool {
        self.inner.drag_mutex.try_mutate(|| {
            let target_offset = self.inner.anchors.get().position_of(&target_value);
            if !target_offset.is_nan() {
                self.drag_to(target_offset, 0.0);
                self.inner.drag_target.set(None);
            }
            self.inner.current_value.set(target_value.clone());
            self.inner.settled_value.set(target_value);
        })
    }

    fn drag_scope(&self) -> Rc<dyn AnchoredDragScope> {
        Rc::new(StateDragScope(Rc::downgrade(&self.inner)))
    }
}

/// Puts the drag target back however an anchored drag to a value ends (Compose's `finally { dragTarget = null }`).
struct ClearDragTarget<T: AnchorKey>(MutableState<Option<T>>);

impl<T: AnchorKey> Drop for ClearDragTarget<T> {
    fn drop(&mut self) {
        self.0.set(None);
    }
}

/// Runs `block` with the latest inputs, starting it again (the old run cancelled first) each time they change,
/// until a run completes (Compose's `restartable`).
async fn restartable<I: PartialEq + Clone + 'static>(
    inputs: impl Fn() -> I + 'static,
    block: Rc<dyn Fn(I) -> LocalFuture<'static, ()>>,
) {
    // (a run completing cancels the scope: Compose's AnchoredDragFinishedSignal, caught here)
    let _ = catch_cancellation(move |scope| async move {
        let previous_drag: Rc<RefCell<Option<Job>>> = Rc::new(RefCell::new(None));
        snapshot_flow(inputs)
            .collect_async(move |latest_inputs| {
                let (previous_drag, scope, block) = (previous_drag.clone(), scope.clone(), block.clone());
                async move {
                    let previous = previous_drag.borrow_mut().take();
                    if let Some(previous) = previous {
                        previous.cancel();
                        previous.join().await;
                    }
                    let s = scope.clone();
                    let job = scope.launch(async move {
                        block(latest_inputs).await;
                        s.cancel();
                    });
                    *previous_drag.borrow_mut() = Some(job);
                }
            })
            .await;
    })
    .await;
}

/// An anchored drag's block (Compose's `suspend AnchoredDragScope.(anchors) -> Unit`): run again when the anchors
/// change.
pub type AnchoredDragBlock<T> =
    Rc<dyn Fn(Rc<dyn AnchoredDragScope>, Arc<dyn DraggableAnchors<T>>) -> LocalFuture<'static, ()>>;

/// An anchored drag to a value's block (Compose's `suspend AnchoredDragScope.(anchors, targetValue) -> Unit`).
pub type AnchoredDragToTargetBlock<T> =
    Rc<dyn Fn(Rc<dyn AnchoredDragScope>, Arc<dyn DraggableAnchors<T>>, T) -> LocalFuture<'static, ()>>;

#[named]
impl<T: AnchorKey> AnchoredDraggableState<T> {
    /// New anchors (Compose's `updateAnchors`): it goes to `new_target`'s (by default the anchor nearest its
    /// offset, else its target) at once, or, while a drag holds it, the drag restarts with them toward it.
    pub fn update_anchors(&self, new_anchors: impl DraggableAnchors<T>, #[default(None)] new_target: Option<T>) {
        let new_anchors = share_anchors(new_anchors);
        let new_target = new_target.unwrap_or_else(|| {
            let offset = self.offset();
            if !offset.is_nan() {
                new_anchors.closest_anchor(offset).unwrap_or_else(|| self.target_value())
            } else {
                self.target_value()
            }
        });
        if *self.inner.anchors.get() != *new_anchors {
            self.inner.anchors.set(new_anchors);
            // (no drag holds it: the offset now; else the drag under way restarts toward the target)
            let snap_successful = self.try_snap_to(new_target.clone());
            if !snap_successful {
                self.inner.drag_target.set(Some(new_target));
            }
        }
    }

    /// Settles on the anchor nearest its offset if confirmed, else back on its current value, along
    /// `animation_spec` (Compose's `settle(animationSpec)`).
    pub fn settle(&self, animation_spec: Arc<dyn AnimationSpec<f32>>) -> impl Future<Output = ()> + 'static {
        let state = self.clone();
        async move {
            let previous_value = state.current_value();
            let target_value = state.anchors().closest_anchor(state.require_offset());
            match target_value {
                Some(t) if state.confirm(&t) => named_args!(state.animate_to(t, animation_spec = animation_spec)).await,
                // (vetoed: back to where it was)
                _ => named_args!(state.animate_to(previous_value, animation_spec = animation_spec)).await,
            }
        }
    }

    /// Settles with `velocity` px/s by the thresholds of a state made by Compose's deprecated constructors
    /// (Compose's deprecated `settle(velocity)`): the velocity it used.
    /// Kotlin: `settle(velocity: Float): Float`
    pub fn settle_with_velocity(&self, velocity: f32) -> impl Future<Output = f32> + 'static {
        if !self.use_pre_modifier_change_behavior() {
            panic_kt(
                "IllegalArgumentException",
                "AnchoredDraggableState was configured through a constructor without providing positional and \
                 velocity threshold. This overload of settle has been deprecated. Please refer to \
                 AnchoredDraggableState#settle(animationSpec) for more information.",
            );
        }
        let state = self.clone();
        async move {
            let previous_value = state.current_value();
            let positional_threshold = state.inner.positional_threshold.borrow().clone().expect("its threshold");
            let velocity_threshold = state.inner.velocity_threshold.borrow().clone().expect("its threshold");
            let target_value = compute_target(
                &*state.anchors(),
                state.require_offset(),
                velocity,
                &*positional_threshold,
                &*velocity_threshold,
            );
            let to = if state.confirm(&target_value) { target_value } else { previous_value };
            named_args!(state.animate_to_with_decay(to, velocity)).await
        }
    }

    /// Runs `block` with its drag scope and anchors in its mutation of `drag_priority`, again with the new anchors
    /// each time they change (the old run cancelled), until a run completes; then the anchor it is at (within half
    /// a px), if confirmed, is its current and settled value (Compose's `anchoredDrag(dragPriority, block)`).
    pub fn anchored_drag<F, Fut>(
        &self,
        #[default(MutatePriority::Default)] drag_priority: MutatePriority,
        block: F,
    ) -> impl Future<Output = ()> + 'static
    where
        F: Fn(Rc<dyn AnchoredDragScope>, Arc<dyn DraggableAnchors<T>>) -> Fut + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        let block: AnchoredDragBlock<T> = Rc::new(move |scope, anchors| Box::pin(block(scope, anchors)));
        self.anchored_drag_block(drag_priority, block)
    }

    /// Runs `block` toward `target_value` (Compose's `anchoredDrag(targetValue, dragPriority, block)`): it is the
    /// target while it runs (restarted with the latest anchors and target as they change), then, if confirmed, its
    /// current and settled value, at its anchor. A value not among the anchors is, if confirmed, its current and
    /// settled value at once.
    /// Kotlin: `anchoredDrag(targetValue: T, dragPriority: MutatePriority = MutatePriority.Default, block: suspend AnchoredDragScope.(anchor: DraggableAnchors<T>, targetValue: T) -> Unit)`
    pub fn anchored_drag_to_target<F, Fut>(
        &self,
        target_value: T,
        #[default(MutatePriority::Default)] drag_priority: MutatePriority,
        block: F,
    ) -> impl Future<Output = ()> + 'static
    where
        F: Fn(Rc<dyn AnchoredDragScope>, Arc<dyn DraggableAnchors<T>>, T) -> Fut + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        let block: AnchoredDragToTargetBlock<T> =
            Rc::new(move |scope, anchors, target| Box::pin(block(scope, anchors, target)));
        self.anchored_drag_to_target_block(target_value, drag_priority, block)
    }

    /// Goes to `target_value`'s anchor at once, in a mutation (Compose's `snapTo`).
    pub fn snap_to(&self, target_value: T) -> impl Future<Output = ()> + 'static {
        self.anchored_drag_to_target_block(
            target_value,
            MutatePriority::Default,
            Rc::new(|scope, anchors, latest_target| {
                Box::pin(async move {
                    let target_offset = anchors.position_of(&latest_target);
                    if !target_offset.is_nan() {
                        scope.drag_to(target_offset, 0.0);
                    }
                })
            }),
        )
    }

    /// Animates to `target_value`'s anchor along `animation_spec` from its last velocity (Compose's `animateTo`).
    /// Defaulted, the spec is the deprecated constructors' snap spec of a state made by one, else
    /// [`AnchoredDraggableDefaults::SnapAnimationSpec`] (the default is that spec's one shared instance, known by
    /// it: another given is the caller's).
    /// Kotlin: `animateTo(targetValue: T, animationSpec: AnimationSpec<Float> = AnchoredDraggableDefaults.SnapAnimationSpec)`
    pub fn animate_to(
        &self,
        target_value: T,
        #[default(default_snap_animation_spec())] animation_spec: Arc<dyn AnimationSpec<f32>>,
    ) -> impl Future<Output = ()> + 'static {
        let animation_spec = if Arc::ptr_eq(&animation_spec, &default_snap_animation_spec()) {
            self.default_snap_animation_spec()
        } else {
            animation_spec
        };
        let state = self.clone();
        self.anchored_drag_to_target_block(
            target_value,
            MutatePriority::Default,
            Rc::new(move |scope, anchors, latest_target| {
                let (state, spec) = (state.clone(), animation_spec.clone());
                Box::pin(async move {
                    let velocity = state.last_velocity();
                    animate_to_anchor(&state, velocity, scope, &*anchors, &latest_target, spec).await;
                })
            }),
        )
    }

    /// Goes to `target_value`'s anchor flung at `velocity` px/s: a decay along `decay_animation_spec` when it
    /// reaches the anchor that way (stopped there), else an animation along `snap_animation_spec` (Compose's
    /// `animateToWithDecay`; the specs by default the deprecated constructors', else
    /// [`AnchoredDraggableDefaults`]'): the velocity it used.
    /// Kotlin: `animateToWithDecay(targetValue: T, velocity: Float, snapAnimationSpec: AnimationSpec<Float> = AnchoredDraggableDefaults.SnapAnimationSpec, decayAnimationSpec: DecayAnimationSpec<Float> = AnchoredDraggableDefaults.DecayAnimationSpec): Float`
    pub fn animate_to_with_decay(
        &self,
        target_value: T,
        velocity: f32,
        #[default(None)] snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
        #[default(None)] decay_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    ) -> impl Future<Output = f32> + 'static {
        let snap_animation_spec = snap_animation_spec.unwrap_or_else(|| self.default_snap_animation_spec());
        let decay_animation_spec = decay_animation_spec.unwrap_or_else(|| self.default_decay_animation_spec());
        let remaining_velocity = Rc::new(Cell::new(velocity));
        let r = remaining_velocity.clone();
        let state = self.clone();
        let drag = self.anchored_drag_to_target_block(
            target_value,
            MutatePriority::Default,
            Rc::new(move |scope, anchors, latest_target| {
                let (state, snap, decay, r) =
                    (state.clone(), snap_animation_spec.clone(), decay_animation_spec.clone(), r.clone());
                Box::pin(async move {
                    let target_offset = anchors.position_of(&latest_target);
                    if target_offset.is_nan() {
                        return;
                    }
                    let offset = state.offset();
                    let prev = if offset.is_nan() { 0.0 } else { offset };
                    if prev == target_offset {
                        return;
                    }
                    // A target against the fling's direction, or no velocity: an animation to it. (At the target, a
                    // decay that uses no velocity.)
                    if velocity * (target_offset - prev) < 0.0 || velocity == 0.0 {
                        animate_to_anchor(&state, velocity, scope, &*anchors, &latest_target, snap).await;
                        r.set(0.0);
                        return;
                    }
                    let projected_decay_offset = decay.calculate_float_target_value(prev, velocity);
                    let can_decay_to_target = if velocity > 0.0 {
                        projected_decay_offset >= target_offset
                    } else {
                        projected_decay_offset <= target_offset
                    };
                    if can_decay_to_target {
                        let prev = Rc::new(Cell::new(prev));
                        let (p, r2) = (prev.clone(), r.clone());
                        let block: Box<dyn for<'a> FnMut(&'a AnimationScope<f32, AnimationVector1D>)> =
                            Box::new(move |a| {
                                let value = a.value();
                                // Past the target either way: stopped on it.
                                if (value < target_offset && p.get() > target_offset)
                                    || (value > target_offset && p.get() < target_offset)
                                {
                                    let final_value = coerce_to_target(value, target_offset);
                                    scope.drag_to(final_value, a.velocity());
                                    r2.set(if a.velocity().is_nan() { 0.0 } else { a.velocity() });
                                    p.set(final_value);
                                    a.cancel_animation();
                                } else {
                                    scope.drag_to(value, a.velocity());
                                    r2.set(a.velocity());
                                    p.set(value);
                                }
                            });
                        let animation_state =
                            named_args!(float_animation_state(prev.get(), initial_velocity = velocity));
                        named_args!(animation_state.animate_decay(decay, block = block)).await;
                    } else {
                        animate_to_anchor(&state, velocity, scope, &*anchors, &latest_target, snap).await;
                        r.set(0.0);
                    }
                })
            }),
        );
        async move {
            drag.await;
            velocity - remaining_velocity.get()
        }
    }
}

impl<T: AnchorKey> AnchoredDraggableState<T> {
    fn default_snap_animation_spec(&self) -> Arc<dyn AnimationSpec<f32>> {
        if self.use_pre_modifier_change_behavior() {
            self.inner.snap_animation_spec.borrow().clone().expect("its spec")
        } else {
            AnchoredDraggableDefaults::SnapAnimationSpec()
        }
    }

    fn default_decay_animation_spec(&self) -> Arc<dyn DecayAnimationSpec<f32>> {
        if self.use_pre_modifier_change_behavior() {
            self.inner.decay_animation_spec.borrow().clone().expect("its spec")
        } else {
            AnchoredDraggableDefaults::DecayAnimationSpec()
        }
    }

    /// [`anchored_drag`](Self::anchored_drag) of a shared block.
    pub(crate) fn anchored_drag_block(
        &self,
        drag_priority: MutatePriority,
        block: AnchoredDragBlock<T>,
    ) -> impl Future<Output = ()> + 'static {
        let state = self.clone();
        let mutation = async move {
            let (s, scope) = (state.clone(), state.drag_scope());
            restartable(move || s.anchors(), Rc::new(move |latest_anchors| block(scope.clone(), latest_anchors))).await;
            let anchors = state.anchors();
            let offset = state.offset();
            if let Some(closest) = anchors.closest_anchor(offset) {
                let closest_anchor_offset = anchors.position_of(&closest);
                let is_at_closest_anchor = (offset - closest_anchor_offset).abs() < 0.5;
                if is_at_closest_anchor && state.confirm(&closest) {
                    state.inner.settled_value.set(closest.clone());
                    state.inner.current_value.set(closest);
                }
            }
        };
        named_args!(self.inner.drag_mutex.mutate(drag_priority, mutation))
    }

    /// [`anchored_drag_to_target`](Self::anchored_drag_to_target) of a shared block.
    pub(crate) fn anchored_drag_to_target_block(
        &self,
        target_value: T,
        drag_priority: MutatePriority,
        block: AnchoredDragToTargetBlock<T>,
    ) -> LocalFuture<'static, ()> {
        let state = self.clone();
        if !self.anchors().has_position_for(&target_value) {
            return Box::pin(async move {
                if state.confirm(&target_value) {
                    state.inner.settled_value.set(target_value.clone());
                    state.inner.current_value.set(target_value);
                }
            });
        }
        let clear = ClearDragTarget(self.inner.drag_target.clone());
        let mutation = {
            let state = state.clone();
            async move {
                state.inner.drag_target.set(Some(target_value.clone()));
                let (s, scope) = (state.clone(), state.drag_scope());
                restartable(
                    move || (s.anchors(), s.target_value()),
                    Rc::new(move |(anchors, latest_target)| block(scope.clone(), anchors, latest_target)),
                )
                .await;
                if state.confirm(&target_value) {
                    let latest_target_offset = state.anchors().position_of(&target_value);
                    state.drag_to(latest_target_offset, state.last_velocity());
                    state.inner.settled_value.set(target_value.clone());
                    state.inner.current_value.set(target_value);
                }
            }
        };
        let mutate = named_args!(self.inner.drag_mutex.mutate(drag_priority, mutation));
        Box::pin(async move {
            // (Compose's `try { … } finally { dragTarget = null }`)
            let _clear = clear;
            mutate.await;
        })
    }
}

thread_local! {
    /// `animate_to`'s default spec: one instance, told from a spec given.
    static DEFAULT_SNAP_ANIMATION_SPEC: Arc<dyn AnimationSpec<f32>> = AnchoredDraggableDefaults::SnapAnimationSpec();
}

/// `animate_to`'s default spec (its one instance on this thread).
#[doc(hidden)]
pub fn default_snap_animation_spec() -> Arc<dyn AnimationSpec<f32>> {
    DEFAULT_SNAP_ANIMATION_SPEC.with(Arc::clone)
}

/// Animates to `latest_target`'s anchor from the offset (0 if none) at `velocity` along `snap_animation_spec`,
/// overshoots allowed (Compose's private `animateTo(velocity, scope, anchors, latestTarget, spec)`).
async fn animate_to_anchor<T: AnchorKey>(
    state: &AnchoredDraggableState<T>,
    velocity: f32,
    scope: Rc<dyn AnchoredDragScope>,
    anchors: &dyn DraggableAnchors<T>,
    latest_target: &T,
    snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
) {
    let target_offset = anchors.position_of(latest_target);
    let offset = state.offset();
    let prev = if offset.is_nan() { 0.0 } else { offset };
    if !target_offset.is_nan() && prev != target_offset {
        // (onDrag coerces within the bounds; an animation may overshoot, a spring's: allowed, through the
        // mutex's dragTo)
        let block = move |value: f32, velocity: f32| scope.drag_to(value, velocity);
        named_args!(animate(
            prev,
            target_offset,
            initial_velocity = velocity,
            animation_spec = snap_animation_spec,
            block = block
        ))
        .await;
    }
}

/// `value` brought to `target` from its side (Compose's `coerceToTarget`; 0 for a target of 0).
fn coerce_to_target(value: f32, target: f32) -> f32 {
    if target == 0.0 {
        0.0
    } else if target > 0.0 {
        value.min(target)
    } else {
        value.max(target)
    }
}

/// The anchor a fling at `velocity` from `current_offset` settles on (Compose's `computeTarget`): the closest one
/// when still; past the velocity threshold the next one its way; slower, the next one once past the positional
/// threshold of the distance between them, else back.
fn compute_target<T: AnchorKey>(
    anchors: &dyn DraggableAnchors<T>,
    current_offset: f32,
    velocity: f32,
    positional_threshold: &dyn Fn(f32) -> f32,
    velocity_threshold: &dyn Fn() -> f32,
) -> T {
    if current_offset.is_nan() {
        panic_kt("IllegalArgumentException", "The offset provided to computeTarget must not be NaN.");
    }
    let is_moving = velocity.abs() > 0.0;
    let is_moving_forward = is_moving && velocity > 0.0;
    if !is_moving {
        anchors.closest_anchor(current_offset).expect("an anchor")
    } else if velocity.abs() >= velocity_threshold().abs() {
        anchors.closest_anchor_in_direction(current_offset, is_moving_forward).expect("an anchor")
    } else {
        let left = anchors.closest_anchor_in_direction(current_offset, false).expect("an anchor");
        let left_anchor_position = anchors.position_of(&left);
        let right = anchors.closest_anchor_in_direction(current_offset, true).expect("an anchor");
        let right_anchor_position = anchors.position_of(&right);
        let distance = (left_anchor_position - right_anchor_position).abs();
        let relative_threshold = positional_threshold(distance).abs();
        let closest_anchor_from_start = if is_moving_forward { left_anchor_position } else { right_anchor_position };
        let relative_position = (closest_anchor_from_start - current_offset).abs();
        if relative_position >= relative_threshold {
            if is_moving_forward { right } else { left }
        } else if is_moving_forward {
            left
        } else {
            right
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Making one

/// A state at `initial_value`, without anchors until [`update_anchors`](AnchoredDraggableState::update_anchors)
/// (Compose's `AnchoredDraggableState(initialValue)`).
pub fn AnchoredDraggableState<T: AnchorKey>(initial_value: T) -> AnchoredDraggableState<T> {
    AnchoredDraggableState::make(initial_value)
}

/// A state at `initial_value` with `anchors` (Compose's `AnchoredDraggableState(initialValue, anchors)`).
/// Kotlin: `AnchoredDraggableState(initialValue: T, anchors: DraggableAnchors<T>)`
pub fn anchored_draggable_state_with_anchors<T: AnchorKey>(
    initial_value: T,
    anchors: impl DraggableAnchors<T>,
) -> AnchoredDraggableState<T> {
    let state = AnchoredDraggableState::make(initial_value.clone());
    state.inner.anchors.set(share_anchors(anchors));
    state.try_snap_to(initial_value);
    state
}

/// A state at `initial_value` taking only the values `confirm_value_change` confirms (Compose's deprecated
/// `AnchoredDraggableState(initialValue, confirmValueChange)`: leave disallowed values out of the anchors).
/// Kotlin: `AnchoredDraggableState(initialValue: T, confirmValueChange: (newValue: T) -> Boolean)`
pub fn anchored_draggable_state_confirming<T: AnchorKey>(
    initial_value: T,
    confirm_value_change: impl Fn(&T) -> bool + 'static,
) -> AnchoredDraggableState<T> {
    let state = AnchoredDraggableState::make(initial_value);
    *state.inner.confirm_value_change.borrow_mut() = Rc::new(confirm_value_change);
    state
}

/// The same with `anchors` (Compose's deprecated `AnchoredDraggableState(initialValue, anchors,
/// confirmValueChange)`).
/// Kotlin: `AnchoredDraggableState(initialValue: T, anchors: DraggableAnchors<T>, confirmValueChange: (newValue: T) -> Boolean = { true })`
pub fn anchored_draggable_state_with_anchors_confirming<T: AnchorKey>(
    initial_value: T,
    anchors: impl DraggableAnchors<T>,
    confirm_value_change: impl Fn(&T) -> bool + 'static,
) -> AnchoredDraggableState<T> {
    let state = anchored_draggable_state_confirming(initial_value.clone(), confirm_value_change);
    state.inner.anchors.set(share_anchors(anchors));
    state.try_snap_to(initial_value);
    state
}

/// A state settling by its own thresholds and specs (Compose's deprecated `AnchoredDraggableState(initialValue,
/// positionalThreshold, velocityThreshold, snapAnimationSpec, decayAnimationSpec, confirmValueChange)`: give them
/// to [`AnchoredDraggableDefaults::fling_behavior`] instead).
/// Kotlin: `AnchoredDraggableState(initialValue: T, positionalThreshold: (totalDistance: Float) -> Float, velocityThreshold: () -> Float, snapAnimationSpec: AnimationSpec<Float>, decayAnimationSpec: DecayAnimationSpec<Float>, confirmValueChange: (newValue: T) -> Boolean = { true })`
pub fn anchored_draggable_state_configured<T: AnchorKey>(
    initial_value: T,
    positional_threshold: impl Fn(f32) -> f32 + 'static,
    velocity_threshold: impl Fn() -> f32 + 'static,
    snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
    decay_animation_spec: Arc<dyn DecayAnimationSpec<f32>>,
    confirm_value_change: impl Fn(&T) -> bool + 'static,
) -> AnchoredDraggableState<T> {
    let state = anchored_draggable_state_confirming(initial_value, confirm_value_change);
    *state.inner.positional_threshold.borrow_mut() = Some(Rc::new(positional_threshold));
    *state.inner.velocity_threshold.borrow_mut() = Some(Rc::new(velocity_threshold));
    *state.inner.snap_animation_spec.borrow_mut() = Some(snap_animation_spec);
    *state.inner.decay_animation_spec.borrow_mut() = Some(decay_animation_spec);
    state
}

/// The same with `anchors` (Compose's deprecated `AnchoredDraggableState(initialValue, anchors,
/// positionalThreshold, velocityThreshold, snapAnimationSpec, decayAnimationSpec, confirmValueChange)`).
/// Kotlin: `AnchoredDraggableState(initialValue: T, anchors: DraggableAnchors<T>, positionalThreshold: (totalDistance: Float) -> Float, velocityThreshold: () -> Float, snapAnimationSpec: AnimationSpec<Float>, decayAnimationSpec: DecayAnimationSpec<Float>, confirmValueChange: (newValue: T) -> Boolean = { true })`
pub fn anchored_draggable_state_configured_with_anchors<T: AnchorKey>(
    initial_value: T,
    anchors: impl DraggableAnchors<T>,
    positional_threshold: impl Fn(f32) -> f32 + 'static,
    velocity_threshold: impl Fn() -> f32 + 'static,
    snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
    decay_animation_spec: Arc<dyn DecayAnimationSpec<f32>>,
    confirm_value_change: impl Fn(&T) -> bool + 'static,
) -> AnchoredDraggableState<T> {
    let state = anchored_draggable_state_with_anchors_confirming(initial_value, anchors, confirm_value_change);
    *state.inner.positional_threshold.borrow_mut() = Some(Rc::new(positional_threshold));
    *state.inner.velocity_threshold.borrow_mut() = Some(Rc::new(velocity_threshold));
    *state.inner.snap_animation_spec.borrow_mut() = Some(snap_animation_spec);
    *state.inner.decay_animation_spec.borrow_mut() = Some(decay_animation_spec);
    state
}

impl<T: AnchorKey + Saveable> AnchoredDraggableState<T> {
    /// Saves its current value and makes a state at it (Compose's `AnchoredDraggableState.Saver()`).
    pub fn Saver() -> Saver<AnchoredDraggableState<T>> {
        Saver::new(
            |s: &AnchoredDraggableState<T>| s.current_value().save(),
            |v| T::restore(v).map(AnchoredDraggableState),
        )
    }

    /// The same for a state confirming values (Compose's deprecated `Saver(confirmValueChange)`).
    /// Kotlin: `Saver(confirmValueChange: (T) -> Boolean = { true })`
    pub fn saver_confirming(confirm_value_change: impl Fn(&T) -> bool + 'static) -> Saver<AnchoredDraggableState<T>> {
        let confirm: ConfirmValueChange<T> = Rc::new(confirm_value_change);
        Saver::new(
            |s: &AnchoredDraggableState<T>| s.current_value().save(),
            move |v| {
                let confirm = confirm.clone();
                T::restore(v).map(|value| anchored_draggable_state_confirming(value, move |t: &T| confirm(t)))
            },
        )
    }

    /// The same for a state of Compose's deprecated configuring constructor (Compose's deprecated
    /// `Saver(snapAnimationSpec, decayAnimationSpec, positionalThreshold, velocityThreshold, confirmValueChange)`).
    /// Kotlin: `Saver(snapAnimationSpec: AnimationSpec<Float>, decayAnimationSpec: DecayAnimationSpec<Float>, positionalThreshold: (distance: Float) -> Float, velocityThreshold: () -> Float, confirmValueChange: (T) -> Boolean = { true })`
    pub fn saver_configured(
        snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
        decay_animation_spec: Arc<dyn DecayAnimationSpec<f32>>,
        positional_threshold: impl Fn(f32) -> f32 + 'static,
        velocity_threshold: impl Fn() -> f32 + 'static,
        confirm_value_change: impl Fn(&T) -> bool + 'static,
    ) -> Saver<AnchoredDraggableState<T>> {
        let confirm: ConfirmValueChange<T> = Rc::new(confirm_value_change);
        let positional: Rc<dyn Fn(f32) -> f32> = Rc::new(positional_threshold);
        let velocity: Rc<dyn Fn() -> f32> = Rc::new(velocity_threshold);
        Saver::new(
            |s: &AnchoredDraggableState<T>| s.current_value().save(),
            move |v| {
                let (confirm, positional, velocity) = (confirm.clone(), positional.clone(), velocity.clone());
                T::restore(v).map(|value| {
                    anchored_draggable_state_configured(
                        value,
                        move |d| positional(d),
                        move || velocity(),
                        snap_animation_spec.clone(),
                        decay_animation_spec.clone(),
                        move |t: &T| confirm(t),
                    )
                })
            },
        )
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The defaults and the fling

/// The fling below which an anchored drag settles by its position (Compose's AnchoredDraggableMinFlingVelocity),
/// dp/s.
pub(crate) const AnchoredDraggableMinFlingVelocity: Dp = Dp(125.0);

/// The defaults of `anchored_draggable` (Compose's AnchoredDraggableDefaults).
pub struct AnchoredDraggableDefaults;

impl AnchoredDraggableDefaults {
    /// How it animates to an anchor: `tween()` (Compose's `SnapAnimationSpec`).
    pub fn SnapAnimationSpec() -> Arc<dyn AnimationSpec<f32>> {
        Arc::new(named_args!(tween::<f32>()))
    }

    /// How far to an anchor a slow fling must be to settle on it: half way (Compose's `PositionalThreshold`).
    pub fn PositionalThreshold() -> Rc<dyn Fn(f32) -> f32> {
        Rc::new(|distance| distance / 2.0)
    }

    /// The decay of a fling to an anchor: `exponentialDecay()` (Compose's `DecayAnimationSpec`).
    pub fn DecayAnimationSpec() -> Arc<dyn DecayAnimationSpec<f32>> {
        Arc::new(named_args!(exponential_decay::<f32>()))
    }
}

impl AnchoredDraggableDefaults {
    /// The fling of `state`'s drags, remembered for this density, state, threshold and spec (Compose's composable
    /// `flingBehavior`): it snaps to the anchor the velocity and `positional_threshold` choose (past 125 dp/s the
    /// next one its way; slower, the next one past the threshold of the distance between them) if the state
    /// confirms it, else its settled value, along `animation_spec` (`None`s: Compose's defaults,
    /// [`PositionalThreshold`](Self::PositionalThreshold) and [`SnapAnimationSpec`](Self::SnapAnimationSpec)).
    /// Kotlin: `flingBehavior(state: AnchoredDraggableState<T>, positionalThreshold: ((totalDistance: Float) -> Float)? = null, animationSpec: AnimationSpec<Float>? = null): TargetedFlingBehavior`
    #[track_caller]
    pub fn fling_behavior<T: AnchorKey>(
        state: AnchoredDraggableState<T>,
        positional_threshold: Option<Rc<dyn Fn(f32) -> f32>>,
        animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    ) -> Rc<dyn TargetedFlingBehavior> {
        kompoz_runtime::kompozable_block!({
            let density = LocalDensity.current();
            let keys = (
                density.density.to_bits(),
                density.font_scale.to_bits(),
                Rc::as_ptr(&state.inner) as *const () as usize,
                positional_threshold.as_ref().map(|p| Rc::as_ptr(p) as *const () as usize),
                animation_spec.as_ref().map(|s| Arc::as_ptr(s) as *const () as usize),
            );
            remember_keyed(keys, move || {
                anchored_draggable_fling_behavior(
                    state,
                    density,
                    positional_threshold.unwrap_or_else(AnchoredDraggableDefaults::PositionalThreshold),
                    animation_spec.unwrap_or_else(AnchoredDraggableDefaults::SnapAnimationSpec),
                )
            })
        })
    }
}

/// Compose's `anchoredDraggableFlingBehavior`: a snapping fling with no decay, over the anchors.
pub(crate) fn anchored_draggable_fling_behavior<T: AnchorKey>(
    state: AnchoredDraggableState<T>,
    density: Density,
    positional_threshold: Rc<dyn Fn(f32) -> f32>,
    snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
) -> Rc<dyn TargetedFlingBehavior> {
    snap_fling_behavior(
        Rc::new(AnchoredDraggableLayoutInfoProvider {
            state,
            positional_threshold,
            velocity_threshold: Rc::new(move || density.to_px(AnchoredDraggableMinFlingVelocity)),
        }),
        Arc::new(generate_decay_animation_spec::<f32>(NoOpDecayAnimationSpec)),
        snap_animation_spec,
    )
}

/// Where an anchored drag's fling snaps (Compose's AnchoredDraggableLayoutInfoProvider): never an approach; the
/// anchor the velocity and thresholds choose, if confirmed, else the settled value.
struct AnchoredDraggableLayoutInfoProvider<T: AnchorKey> {
    state: AnchoredDraggableState<T>,
    positional_threshold: Rc<dyn Fn(f32) -> f32>,
    velocity_threshold: Rc<dyn Fn() -> f32>,
}

impl<T: AnchorKey> SnapLayoutInfoProvider for AnchoredDraggableLayoutInfoProvider<T> {
    fn calculate_approach_offset(&self, _velocity: f32, _decay_offset: f32) -> f32 {
        0.0
    }

    fn calculate_snap_offset(&self, velocity: f32) -> f32 {
        let current_offset = self.state.require_offset();
        let anchors = self.state.anchors();
        let proposed_target_value =
            compute_target(&*anchors, current_offset, velocity, &*self.positional_threshold, &*self.velocity_threshold);
        let target_value =
            if self.state.confirm(&proposed_target_value) { proposed_target_value } else { self.state.settled_value() };
        anchors.position_of(&target_value) - current_offset
    }
}

/// A decay that goes nowhere (Compose's NoOpDecayAnimationSpec).
struct NoOpDecayAnimationSpec;

impl FloatDecayAnimationSpec for NoOpDecayAnimationSpec {
    fn abs_velocity_threshold(&self) -> f32 {
        0.0
    }
    fn get_value_from_nanos(&self, _play_time_nanos: i64, _initial_value: f32, _initial_velocity: f32) -> f32 {
        0.0
    }
    fn get_duration_nanos(&self, _initial_value: f32, _initial_velocity: f32) -> i64 {
        0
    }
    fn get_velocity_from_nanos(&self, _play_time_nanos: i64, _initial_value: f32, _initial_velocity: f32) -> f32 {
        0.0
    }
    fn get_target_value(&self, _initial_value: f32, _initial_velocity: f32) -> f32 {
        0.0
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Modifier.anchored_draggable

/// The anchored draggable modifier.
#[named(trait AnchoredDraggableModifiers)]
impl Modifier {
    /// Drags `state` along `orientation` between its anchors (Compose's `Modifier.anchoredDraggable(state,
    /// orientation, …)`): after the touch slop (at once while it animates: a finger catches it), each delta moves
    /// its offset within its anchors — through `overscroll_effect` when given, which takes what goes past them —
    /// in a drag of the state; let go, `fling_behavior` (by default [`AnchoredDraggableDefaults::fling_behavior`]'s,
    /// for the node's density) settles it on an anchor. Horizontally right to left, a drag right moves it back.
    /// Drags are emitted to `interaction_source`. A mouse drags it too. Not `enabled`: nothing.
    fn anchored_draggable<T: AnchorKey>(
        self,
        state: AnchoredDraggableState<T>,
        orientation: Orientation,
        #[default(true)] enabled: bool,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        #[default(None)] overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
        #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
    ) -> Modifier {
        self.then_element(AnchoredDraggableElement {
            state,
            orientation,
            enabled,
            reverse_direction: None,
            interaction_source,
            start_drag_immediately: None,
            overscroll_effect,
            fling_behavior,
        })
    }

    /// The same, its drags reversed or not by `reverse_direction` whatever the layout direction (Compose's
    /// `Modifier.anchoredDraggable(state, reverseDirection, orientation, …)`).
    /// Kotlin: `anchoredDraggable(state: AnchoredDraggableState<T>, reverseDirection: Boolean, orientation: Orientation, enabled: Boolean = true, interactionSource: MutableInteractionSource? = null, overscrollEffect: OverscrollEffect? = null, flingBehavior: FlingBehavior? = null)`
    fn anchored_draggable_reversed<T: AnchorKey>(
        self,
        state: AnchoredDraggableState<T>,
        reverse_direction: bool,
        orientation: Orientation,
        #[default(true)] enabled: bool,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        #[default(None)] overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
        #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
    ) -> Modifier {
        self.then_element(AnchoredDraggableElement {
            state,
            orientation,
            enabled,
            reverse_direction: Some(reverse_direction),
            interaction_source,
            start_drag_immediately: None,
            overscroll_effect,
            fling_behavior,
        })
    }

    /// Compose's deprecated `Modifier.anchoredDraggable(state, orientation, …, startDragImmediately, …)`: a drag
    /// starts at once when `start_drag_immediately` says (`None`: while the state animates, Compose's default).
    /// Kotlin: `anchoredDraggable(state: AnchoredDraggableState<T>, orientation: Orientation, enabled: Boolean = true, interactionSource: MutableInteractionSource? = null, overscrollEffect: OverscrollEffect? = null, startDragImmediately: Boolean = state.isAnimationRunning, flingBehavior: FlingBehavior? = null)`
    fn anchored_draggable_starting_immediately<T: AnchorKey>(
        self,
        state: AnchoredDraggableState<T>,
        orientation: Orientation,
        #[default(true)] enabled: bool,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        #[default(None)] overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
        #[default(None)] start_drag_immediately: Option<bool>,
        #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
    ) -> Modifier {
        let start_drag_immediately = Some(start_drag_immediately.unwrap_or_else(|| state.is_animation_running()));
        self.then_element(AnchoredDraggableElement {
            state,
            orientation,
            enabled,
            reverse_direction: None,
            interaction_source,
            start_drag_immediately,
            overscroll_effect,
            fling_behavior,
        })
    }

    /// Compose's deprecated `Modifier.anchoredDraggable(state, reverseDirection, orientation, …,
    /// startDragImmediately, …)`.
    /// Kotlin: `anchoredDraggable(state: AnchoredDraggableState<T>, reverseDirection: Boolean, orientation: Orientation, enabled: Boolean = true, interactionSource: MutableInteractionSource? = null, overscrollEffect: OverscrollEffect? = null, startDragImmediately: Boolean = state.isAnimationRunning, flingBehavior: FlingBehavior? = null)`
    fn anchored_draggable_reversed_starting_immediately<T: AnchorKey>(
        self,
        state: AnchoredDraggableState<T>,
        reverse_direction: bool,
        orientation: Orientation,
        #[default(true)] enabled: bool,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        #[default(None)] overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
        #[default(None)] start_drag_immediately: Option<bool>,
        #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
    ) -> Modifier {
        let start_drag_immediately = Some(start_drag_immediately.unwrap_or_else(|| state.is_animation_running()));
        self.then_element(AnchoredDraggableElement {
            state,
            orientation,
            enabled,
            reverse_direction: Some(reverse_direction),
            interaction_source,
            start_drag_immediately,
            overscroll_effect,
            fling_behavior,
        })
    }
}

struct AnchoredDraggableElement<T: AnchorKey> {
    state: AnchoredDraggableState<T>,
    orientation: Orientation,
    enabled: bool,
    reverse_direction: Option<bool>,
    interaction_source: Option<MutableInteractionSource>,
    start_drag_immediately: Option<bool>,
    overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
    fling_behavior: Option<Rc<dyn FlingBehavior>>,
}

impl<T: AnchorKey> PartialEq for AnchoredDraggableElement<T> {
    fn eq(&self, o: &Self) -> bool {
        self.state == o.state
            && self.orientation == o.orientation
            && self.enabled == o.enabled
            && self.reverse_direction == o.reverse_direction
            && self.interaction_source == o.interaction_source
            && self.start_drag_immediately == o.start_drag_immediately
            && same_effect(&self.overscroll_effect, &o.overscroll_effect)
            && same_fling_behavior(&self.fling_behavior, &o.fling_behavior)
    }
}

/// `anchored_draggable`'s: any pointer drags it (Compose's AlwaysDrag).
fn always_drag(_: PointerType) -> bool {
    true
}

impl<T: AnchorKey> ModifierNodeElement for AnchoredDraggableElement<T> {
    type Node = AnchoredDraggableNode<T>;
    fn create(&self) -> AnchoredDraggableNode<T> {
        AnchoredDraggableNode::new(self)
    }
    fn update(&self, node: &mut AnchoredDraggableNode<T>) {
        node.core.update(self);
    }
    fn name(&self) -> &'static str {
        "anchoredDraggable"
    }
}

/// `anchored_draggable`'s node (Compose's AnchoredDraggableNode): a drag gesture node along its orientation
/// dragging the state's offset in its anchored drag, flinging it to an anchor when let go.
pub struct AnchoredDraggableNode<T: AnchorKey> {
    core: Rc<AnchoredCore<T>>,
}

struct AnchoredCore<T: AnchorKey> {
    me: Weak<AnchoredCore<T>>,
    drag: Rc<DragGestureCore>,
    state: RefCell<AnchoredDraggableState<T>>,
    reverse_direction: Cell<Option<bool>>,
    overscroll_effect: RefCell<Option<Rc<dyn OverscrollEffect>>>,
    start_drag_immediately: Cell<Option<bool>>,
    fling_behavior: RefCell<Option<Rc<dyn FlingBehavior>>>,
    resolved_fling_behavior: RefCell<Option<Rc<dyn FlingBehavior>>>,
    density: Cell<Option<Density>>,
    handle: RefCell<Option<NodeHandle>>,
}

impl<T: AnchorKey> AnchoredDraggableNode<T> {
    fn new(e: &AnchoredDraggableElement<T>) -> AnchoredDraggableNode<T> {
        let drag =
            DragGestureCore::new(Rc::new(always_drag), e.enabled, e.interaction_source.clone(), Some(e.orientation));
        let core = Rc::new_cyclic(|me| AnchoredCore {
            me: me.clone(),
            drag: drag.clone(),
            state: RefCell::new(e.state.clone()),
            reverse_direction: Cell::new(e.reverse_direction),
            overscroll_effect: RefCell::new(e.overscroll_effect.clone()),
            start_drag_immediately: Cell::new(e.start_drag_immediately),
            fling_behavior: RefCell::new(e.fling_behavior.clone()),
            resolved_fling_behavior: RefCell::new(None),
            density: Cell::new(None),
            handle: RefCell::new(None),
        });
        let owner: Rc<dyn DragGestureOwner> = core.clone();
        drag.set_owner(&owner);
        AnchoredDraggableNode { core }
    }
}

impl<T: AnchorKey> AnchoredCore<T> {
    fn cx(&self) -> Option<NodeCx> {
        self.handle.borrow().as_ref().and_then(|h| h.cx())
    }

    fn is_attached(&self) -> bool {
        self.cx().is_some_and(|c| c.is_attached())
    }

    fn orientation(&self) -> Orientation {
        self.drag.orientation.get().unwrap_or(Orientation::Vertical)
    }

    fn state(&self) -> AnchoredDraggableState<T> {
        self.state.borrow().clone()
    }

    /// Compose's `isReverseDirection`: as given, else horizontally right to left.
    fn is_reverse_direction(&self) -> bool {
        match self.reverse_direction.get() {
            Some(r) => r,
            None => {
                self.cx().is_some_and(|c| c.layout_direction() == LayoutDirection::Rtl)
                    && self.orientation() == Orientation::Horizontal
            }
        }
    }

    /// Compose's `updateFlingBehavior`: the given one, else the default for the node's density.
    fn update_fling_behavior(&self, new_fling_behavior: Option<Rc<dyn FlingBehavior>>) {
        let resolved = match new_fling_behavior {
            Some(f) => f,
            None => {
                let density = self.cx().map_or_else(Density::default, |c| c.density());
                self.density.set(Some(density));
                let fling: Rc<dyn FlingBehavior> = anchored_draggable_fling_behavior(
                    self.state(),
                    density,
                    AnchoredDraggableDefaults::PositionalThreshold(),
                    AnchoredDraggableDefaults::SnapAnimationSpec(),
                );
                fling
            }
        };
        *self.resolved_fling_behavior.borrow_mut() = Some(resolved);
    }

    /// Compose's `updateDensity`.
    fn update_density(&self) {
        let Some(new_density) = self.cx().map(|c| c.density()) else { return };
        if self.density.get() != Some(new_density) {
            self.density.set(Some(new_density));
            let fling_behavior = self.fling_behavior.borrow().clone();
            self.update_fling_behavior(fling_behavior);
        }
    }

    /// Compose's AnchoredDraggableNode `update`.
    fn update(&self, e: &AnchoredDraggableElement<T>) {
        *self.fling_behavior.borrow_mut() = e.fling_behavior.clone();
        let mut reset_pointer_input_handling = false;
        if *self.state.borrow() != e.state {
            *self.state.borrow_mut() = e.state.clone();
            self.update_fling_behavior(e.fling_behavior.clone());
            reset_pointer_input_handling = true;
        }
        if self.drag.orientation.get() != Some(e.orientation) {
            reset_pointer_input_handling = true;
        }
        if self.reverse_direction.get() != e.reverse_direction {
            self.reverse_direction.set(e.reverse_direction);
            reset_pointer_input_handling = true;
        }
        self.start_drag_immediately.set(e.start_drag_immediately);
        *self.overscroll_effect.borrow_mut() = e.overscroll_effect.clone();
        self.drag.update(
            Rc::new(always_drag),
            e.enabled,
            e.interaction_source.clone(),
            Some(e.orientation),
            reset_pointer_input_handling,
        );
    }

    fn to_offset(&self, v: f32) -> Offset {
        match self.orientation() {
            Orientation::Horizontal => Offset::new(v, 0.0),
            Orientation::Vertical => Offset::new(0.0, v),
        }
    }

    fn to_velocity(&self, v: f32) -> Velocity {
        match self.orientation() {
            Orientation::Horizontal => Velocity::new(v, 0.0),
            Orientation::Vertical => Velocity::new(0.0, v),
        }
    }

    fn offset_to_float(&self, o: Offset) -> f32 {
        if self.orientation() == Orientation::Vertical { o.y } else { o.x }
    }

    fn velocity_to_float(&self, v: Velocity) -> f32 {
        if self.orientation() == Orientation::Vertical { v.y } else { v.x }
    }

    /// Compose's `fling`: the deprecated constructors' `settle(velocity)`, else the fling behavior in an anchored
    /// drag scrolling the offset within the anchors: the velocity left.
    async fn fling(self: Rc<Self>, velocity: f32) -> f32 {
        let state = self.state();
        if state.use_pre_modifier_change_behavior() {
            return state.settle_with_velocity(velocity).await;
        }
        let leftover_velocity = Rc::new(Cell::new(velocity));
        let (l, me) = (leftover_velocity.clone(), self.clone());
        state
            .anchored_drag_block(
                MutatePriority::Default,
                Rc::new(move |scope, _anchors| {
                    let (l, me) = (l.clone(), me.clone());
                    Box::pin(async move {
                        let state = me.state();
                        let scroll_scope: Rc<dyn ScrollScope> = Rc::new(AnchoredFlingScope { state, scope });
                        let fling_behavior = me.resolved_fling_behavior.borrow().clone();
                        if let Some(fling_behavior) = fling_behavior {
                            l.set(fling_behavior.perform_fling(scroll_scope, velocity).await);
                        }
                    })
                }),
            )
            .await;
        leftover_velocity.get()
    }
}

/// The scroll scope a fling moves the offset in (Compose's `scrollScope` of `fling`): within the anchors, through
/// the anchored drag scope.
struct AnchoredFlingScope<T: AnchorKey> {
    state: AnchoredDraggableState<T>,
    scope: Rc<dyn AnchoredDragScope>,
}

impl<T: AnchorKey> ScrollScope for AnchoredFlingScope<T> {
    fn scroll_by(&self, pixels: f32) -> f32 {
        let new_offset = self.state.new_offset_for_delta(pixels);
        let consumed = new_offset - self.state.offset();
        self.scope.drag_to(new_offset, 0.0);
        consumed
    }
}

impl<T: AnchorKey> DragGestureOwner for AnchoredCore<T> {
    /// Compose's AnchoredDraggableNode `drag`: each delta along the axis (reversed as it says) moves the offset
    /// within the anchors, through the overscroll effect when there is one.
    fn drag(self: Rc<Self>, for_each_delta: ForEachDelta) -> LocalFuture<'static, ()> {
        let state = self.state();
        let me = self.clone();
        Box::pin(state.anchored_drag_block(
            MutatePriority::Default,
            Rc::new(move |scope, _anchors| {
                let (me, for_each_delta) = (me.clone(), for_each_delta.clone());
                Box::pin(async move {
                    let m = me.clone();
                    let process: Rc<dyn Fn(DragDelta)> = Rc::new(move |d: DragDelta| {
                        let delta = if m.is_reverse_direction() { d.delta * -1.0 } else { d.delta };
                        let one_directional_delta = m.offset_to_float(delta);
                        let state = m.state();
                        let overscroll = m.overscroll_effect.borrow().clone();
                        match overscroll {
                            None => scope.drag_to(state.new_offset_for_delta(one_directional_delta), 0.0),
                            Some(overscroll) => {
                                let (m2, scope) = (m.clone(), scope.clone());
                                overscroll.apply_to_scroll(
                                    m.to_offset(one_directional_delta),
                                    NestedScrollSource::UserInput,
                                    &move |delta_for_drag| {
                                        let state = m2.state();
                                        let drag_offset =
                                            state.new_offset_for_delta(m2.offset_to_float(delta_for_drag));
                                        let consumed_delta = m2.to_offset(drag_offset - state.require_offset());
                                        scope.drag_to(drag_offset, 0.0);
                                        consumed_delta
                                    },
                                );
                            }
                        }
                    });
                    for_each_delta(process).await;
                })
            }),
        ))
    }

    fn on_drag_started(&self, _started_position: Offset) {}

    /// Compose's AnchoredDraggableNode `onDragStopped`: the fling to an anchor, through the overscroll effect when
    /// there is one (it gets the velocity back only at an end).
    fn on_drag_stopped(&self, event: DragStopped) {
        if !self.is_attached() {
            return;
        }
        let Some(scope) = self.drag.scope() else { return };
        let Some(me) = self.me.upgrade() else { return };
        scope.launch(async move {
            let velocity = if me.is_reverse_direction() { event.velocity * -1.0 } else { event.velocity };
            let one_directional_velocity = me.velocity_to_float(velocity);
            let overscroll = me.overscroll_effect.borrow().clone();
            match overscroll {
                None => {
                    me.clone().fling(one_directional_velocity).await;
                }
                Some(overscroll) => {
                    let m = me.clone();
                    let perform_fling: PerformFling = Rc::new(move |available_velocity: Velocity| {
                        let m = m.clone();
                        Box::pin(async move {
                            let consumed = m.clone().fling(m.velocity_to_float(available_velocity)).await;
                            let state = m.state();
                            let current_offset = state.require_offset();
                            let anchors = state.anchors();
                            let (min_anchor, max_anchor) = (anchors.min_position(), anchors.max_position());
                            // (the velocity it used only at the first or last anchor)
                            if current_offset >= max_anchor || current_offset <= min_anchor {
                                m.to_velocity(consumed)
                            } else {
                                available_velocity
                            }
                        })
                    });
                    overscroll.apply_to_fling(me.to_velocity(one_directional_velocity), perform_fling).await;
                }
            }
        });
    }

    /// Compose's `startDragImmediately`: as given, else while the state animates (a finger catches it).
    fn start_drag_immediately(&self) -> bool {
        self.start_drag_immediately.get().unwrap_or_else(|| self.state().is_animation_running())
    }
}

impl<T: AnchorKey> ModifierNode for AnchoredDraggableNode<T> {
    node_capabilities!(pointer_input, indirect_pointer_input);

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.core.handle.borrow_mut() = Some(cx.handle());
        self.core.drag.on_attach(cx);
        let fling_behavior = self.core.fling_behavior.borrow().clone();
        self.core.update_fling_behavior(fling_behavior);
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.core.drag.on_detach();
    }
}

impl<T: AnchorKey> PointerInputModifierNode for AnchoredDraggableNode<T> {
    fn on_pointer_event(&mut self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        self.core.drag.on_pointer_event(event, pass, bounds);
    }
    fn on_cancel_pointer_input(&mut self) {
        self.core.drag.on_cancel_pointer_input();
    }
    /// Compose's `onDensityChange`: the gesture reset, the default fling for the new density.
    fn on_density_change(&mut self) {
        self.core.drag.on_cancel_pointer_input();
        if self.core.is_attached() {
            self.core.update_density();
        }
    }
}

impl<T: AnchorKey> IndirectPointerInputModifierNode for AnchoredDraggableNode<T> {
    fn on_indirect_pointer_event(&mut self, event: &IndirectPointerEvent, pass: PointerEventPass) {
        self.core.drag.on_indirect_pointer_event(event, pass);
    }
    fn on_cancel_indirect_pointer_input(&mut self) {
        self.core.drag.on_cancel_indirect_pointer_input();
    }
}
