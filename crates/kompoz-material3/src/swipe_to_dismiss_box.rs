//! Material's swipe to dismiss, as Compose's SwipeToDismissBox.kt: [`SwipeToDismissBox`] (its content swiped away
//! to the start or the end over what is behind it), its state ([`SwipeToDismissBoxState`],
//! [`remember_swipe_to_dismiss_box_state`]), the directions ([`SwipeToDismissBoxValue`]) and the defaults
//! ([`SwipeToDismissBoxDefaults`]).

// (the deprecated overloads call each other, as Compose's do)
#![allow(deprecated)]

use crate::internal::draggable_anchors::DraggableAnchorsModifiers;
use kompoz_coroutines::LaunchedEffect;
use kompoz_foundation::gestures::{
    AnchoredDraggableDefaults, AnchoredDraggableModifiers, AnchoredDraggableState, DraggableAnchors,
    DraggableAnchorsConfig, FlingBehavior, Orientation, anchored_draggable_state_configured,
};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Density, Dp};
use kompoz_runtime::{Saveable, SavedValue, Saver, kompozable, kompozable_block, named, named_args};
use kompoz_ui::Modifier;
use kompoz_ui::platform::LocalDensity;
use std::fmt;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

/// The directions in which a [`SwipeToDismissBox`] can be dismissed (Compose's enum `SwipeToDismissBoxValue`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SwipeToDismissBoxValue {
    /// Can be dismissed by swiping in the reading direction.
    StartToEnd,
    /// Can be dismissed by swiping in the reverse of the reading direction.
    EndToStart,
    /// Cannot currently be dismissed.
    Settled,
}

impl SwipeToDismissBoxValue {
    /// Its values in order (Kotlin's `entries`).
    pub const ENTRIES: [SwipeToDismissBoxValue; 3] =
        [SwipeToDismissBoxValue::StartToEnd, SwipeToDismissBoxValue::EndToStart, SwipeToDismissBoxValue::Settled];
}

/// Saved as its ordinal (Kotlin's enums are Serializable).
impl Saveable for SwipeToDismissBoxValue {
    fn save(&self) -> SavedValue {
        SavedValue::Int(*self as i64)
    }

    fn restore(saved: &SavedValue) -> Option<Self> {
        let ordinal = usize::try_from(i64::restore(saved)?).ok()?;
        SwipeToDismissBoxValue::ENTRIES.get(ordinal).copied()
    }
}

/// The state of a [`SwipeToDismissBox`] (Compose's class `SwipeToDismissBoxState`): where its content is, where it
/// is going and in which direction it has been dismissed. A cheap handle: clones are the same state (equal when
/// they are).
#[derive(Clone)]
pub struct SwipeToDismissBoxState {
    inner: Rc<SwipeToDismissBoxStateInner>,
}

struct SwipeToDismissBoxStateInner {
    anchored_draggable_state: AnchoredDraggableState<SwipeToDismissBoxValue>,
    /// Compose's `lateinit var positionalThreshold`: set by the primary constructor only (the deprecated one gives
    /// the anchored draggable state its thresholds instead).
    positional_threshold: Option<Rc<dyn Fn(f32) -> f32>>,
}

impl PartialEq for SwipeToDismissBoxState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for SwipeToDismissBoxState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SwipeToDismissBoxState(currentValue={:?}, offset={})", self.current_value(), self.offset())
    }
}

/// A swipe-to-dismiss state at `initial_value` (Compose's constructor `SwipeToDismissBoxState(initialValue,
/// positionalThreshold)`): `positional_threshold` gives, from the distance between two anchors (px), how far from
/// the start of a move (px, positive) it must be dragged to settle on the next one while a swipe goes on and once
/// it ends.
pub fn SwipeToDismissBoxState(
    initial_value: SwipeToDismissBoxValue,
    positional_threshold: Rc<dyn Fn(f32) -> f32>,
) -> SwipeToDismissBoxState {
    SwipeToDismissBoxState {
        inner: Rc::new(SwipeToDismissBoxStateInner {
            anchored_draggable_state: AnchoredDraggableState(initial_value),
            positional_threshold: Some(positional_threshold),
        }),
    }
}

/// A swipe-to-dismiss state at `initial_value` that `confirm_value_change` may veto changes of, `density`
/// converting its 125 dp/s velocity threshold (Compose's deprecated constructor `SwipeToDismissBoxState(initialValue,
/// density, confirmValueChange = { true }, positionalThreshold)`: leave the directions not allowed out instead).
///
/// Kotlin: `SwipeToDismissBoxState(initialValue: SwipeToDismissBoxValue, density: Density, confirmValueChange: (SwipeToDismissBoxValue) -> Boolean = { true }, positionalThreshold: (totalDistance: Float) -> Float)`
#[deprecated(note = "confirmValueChange is deprecated without replacement. Rather than relying on a callback to \
                     veto state changes, the anchor set should not include disallowed anchors.")]
#[named]
pub fn swipe_to_dismiss_box_state_confirming(
    initial_value: SwipeToDismissBoxValue,
    density: Density,
    #[default(|_| true)] confirm_value_change: impl Fn(SwipeToDismissBoxValue) -> bool + 'static,
    positional_threshold: Rc<dyn Fn(f32) -> f32>,
) -> SwipeToDismissBoxState {
    SwipeToDismissBoxState::confirming(initial_value, density, Rc::new(confirm_value_change), positional_threshold)
}

impl SwipeToDismissBoxState {
    /// [`swipe_to_dismiss_box_state_confirming`] of a shared closure.
    fn confirming(
        initial_value: SwipeToDismissBoxValue,
        density: Density,
        confirm_value_change: Rc<dyn Fn(SwipeToDismissBoxValue) -> bool>,
        positional_threshold: Rc<dyn Fn(f32) -> f32>,
    ) -> SwipeToDismissBoxState {
        let anchored_draggable_state = anchored_draggable_state_configured(
            initial_value,
            move |total_distance| positional_threshold(total_distance),
            move || density.to_px(DismissVelocityThreshold),
            AnchoredDraggableDefaults::SnapAnimationSpec(),
            AnchoredDraggableDefaults::DecayAnimationSpec(),
            move |value: &SwipeToDismissBoxValue| confirm_value_change(*value),
        );
        SwipeToDismissBoxState {
            inner: Rc::new(SwipeToDismissBoxStateInner { anchored_draggable_state, positional_threshold: None }),
        }
    }

    /// Compose's internal `anchoredDraggableState`.
    pub(crate) fn anchored_draggable_state(&self) -> AnchoredDraggableState<SwipeToDismissBoxValue> {
        self.inner.anchored_draggable_state.clone()
    }

    /// Compose's internal `useFlingBehavior`: made by the primary constructor (its positional threshold set).
    pub(crate) fn use_fling_behavior(&self) -> bool {
        self.inner.positional_threshold.is_some()
    }

    /// Compose's internal `offset`.
    pub(crate) fn offset(&self) -> f32 {
        self.inner.anchored_draggable_state.offset()
    }

    /// The current offset, px (Compose's `requireOffset`): panics (Kotlin's IllegalStateException) before it is
    /// known.
    #[track_caller]
    pub fn require_offset(&self) -> f32 {
        self.inner.anchored_draggable_state.require_offset()
    }

    /// The current state value (Compose's `currentValue`).
    pub fn current_value(&self) -> SwipeToDismissBoxValue {
        self.inner.anchored_draggable_state.current_value()
    }

    /// The target state (Compose's `targetValue`): the closest to the current offset (taking the positional
    /// threshold into account); with no drag or animation going on, the current one.
    pub fn target_value(&self) -> SwipeToDismissBoxValue {
        self.inner.anchored_draggable_state.target_value()
    }

    /// The value it is settled at (Compose's `settledValue`): going over several anchors, A → B → C, it stays
    /// until it settles on one, where the current value follows the closest.
    pub fn settled_value(&self) -> SwipeToDismissBoxValue {
        self.inner.anchored_draggable_state.settled_value()
    }

    /// How far it has gone from the current value to the target one, 0 to 1 (Compose's `progress`).
    pub fn progress(&self) -> f32 {
        self.inner.anchored_draggable_state.progress_to_target()
    }

    /// The direction it has been or is being dismissed in, if any (Compose's `dismissDirection`): to tell the
    /// sides apart in the background.
    pub fn dismiss_direction(&self) -> SwipeToDismissBoxValue {
        let offset = self.offset();
        if offset == 0.0 || offset.is_nan() {
            SwipeToDismissBoxValue::Settled
        } else if offset > 0.0 {
            SwipeToDismissBoxValue::StartToEnd
        } else {
            SwipeToDismissBoxValue::EndToStart
        }
    }

    /// Sets it to `target_value` without an animation, done when it is (Compose's suspending `snapTo`).
    pub fn snap_to(&self, target_value: SwipeToDismissBoxValue) -> impl Future<Output = ()> + 'static {
        self.inner.anchored_draggable_state.snap_to(target_value)
    }

    /// Animates it back to its place, done when it is there or the animation is cancelled (Compose's suspending
    /// `reset`; cancelled, its coroutine is).
    pub fn reset(&self) -> impl Future<Output = ()> + 'static {
        named_args!(self.inner.anchored_draggable_state.animate_to(SwipeToDismissBoxValue::Settled))
    }

    /// Animates it away in `direction`, done when it is gone or the animation is cancelled (Compose's suspending
    /// `dismiss`).
    pub fn dismiss(&self, direction: SwipeToDismissBoxValue) -> impl Future<Output = ()> + 'static {
        named_args!(self.inner.anchored_draggable_state.animate_to(direction))
    }

    /// The default saver of a swipe-to-dismiss state (Compose's `SwipeToDismissBoxState.Saver(positionalThreshold)`):
    /// its current value saved, a state of it restored.
    pub fn Saver(positional_threshold: Rc<dyn Fn(f32) -> f32>) -> Saver<SwipeToDismissBoxState> {
        Saver::new(
            |state: &SwipeToDismissBoxState| state.current_value().save(),
            move |saved| {
                let value = SwipeToDismissBoxValue::restore(saved)?;
                Some(SwipeToDismissBoxState(value, positional_threshold.clone()))
            },
        )
    }

    /// The saver of a state of the deprecated constructor (Compose's deprecated `SwipeToDismissBoxState.Saver(
    /// confirmValueChange, positionalThreshold, density)`).
    ///
    /// Kotlin: `Saver(confirmValueChange: (SwipeToDismissBoxValue) -> Boolean, positionalThreshold: (totalDistance: Float) -> Float, density: Density)`
    #[deprecated(note = "confirmValueChange is deprecated without replacement. Rather than relying on a callback to \
                         veto state changes, the anchor set should not include disallowed anchors.")]
    pub fn saver_confirming(
        confirm_value_change: impl Fn(SwipeToDismissBoxValue) -> bool + 'static,
        positional_threshold: Rc<dyn Fn(f32) -> f32>,
        density: Density,
    ) -> Saver<SwipeToDismissBoxState> {
        SwipeToDismissBoxState::saver_of(Rc::new(confirm_value_change), positional_threshold, density)
    }

    /// [`saver_confirming`](Self::saver_confirming) of a shared closure.
    fn saver_of(
        confirm_value_change: Rc<dyn Fn(SwipeToDismissBoxValue) -> bool>,
        positional_threshold: Rc<dyn Fn(f32) -> f32>,
        density: Density,
    ) -> Saver<SwipeToDismissBoxState> {
        Saver::new(
            |state: &SwipeToDismissBoxState| state.current_value().save(),
            move |saved| {
                let value = SwipeToDismissBoxValue::restore(saved)?;
                Some(SwipeToDismissBoxState::confirming(
                    value,
                    density,
                    confirm_value_change.clone(),
                    positional_threshold.clone(),
                ))
            },
        )
    }
}

/// A swipe-to-dismiss state, saved and restored by [`SwipeToDismissBoxState::Saver`] (Compose's
/// `rememberSwipeToDismissBoxState(initialValue, positionalThreshold)`): at `initial_value` (settled by default);
/// `positional_threshold` gives, from the distance between two anchors (px), how far from the start of a move it
/// must be dragged to settle on the next one (by default [`SwipeToDismissBoxDefaults`]'s 56 dp).
#[kompozable]
pub fn remember_swipe_to_dismiss_box_state(
    #[default(SwipeToDismissBoxValue::Settled)] initial_value: SwipeToDismissBoxValue,
    #[default(SwipeToDismissBoxDefaults::positional_threshold())] positional_threshold: Rc<dyn Fn(f32) -> f32>,
) -> SwipeToDismissBoxState {
    let saver = SwipeToDismissBoxState::Saver(positional_threshold.clone());
    kompoz_runtime::remember_saveable_with(&saver, move || SwipeToDismissBoxState(initial_value, positional_threshold))
}

/// The same for a state `confirm_value_change` may veto changes of (Compose's deprecated
/// `rememberSwipeToDismissBoxState(initialValue, confirmValueChange, positionalThreshold)`).
///
/// Kotlin: `rememberSwipeToDismissBoxState(initialValue: SwipeToDismissBoxValue = SwipeToDismissBoxValue.Settled, confirmValueChange: (SwipeToDismissBoxValue) -> Boolean = { true }, positionalThreshold: (totalDistance: Float) -> Float = SwipeToDismissBoxDefaults.positionalThreshold)`
#[deprecated(note = "confirmValueChange is deprecated without replacement. Rather than relying on a callback to \
                     veto state changes, the anchor set should not include disallowed anchors.")]
#[kompozable]
pub fn remember_swipe_to_dismiss_box_state_confirming(
    #[default(SwipeToDismissBoxValue::Settled)] initial_value: SwipeToDismissBoxValue,
    #[default(|_| true)] confirm_value_change: impl Fn(SwipeToDismissBoxValue) -> bool + 'static,
    #[default(SwipeToDismissBoxDefaults::positional_threshold())] positional_threshold: Rc<dyn Fn(f32) -> f32>,
) -> SwipeToDismissBoxState {
    let density = LocalDensity.current();
    let confirm_value_change: Rc<dyn Fn(SwipeToDismissBoxValue) -> bool> = Rc::new(confirm_value_change);
    let saver = SwipeToDismissBoxState::saver_of(confirm_value_change.clone(), positional_threshold.clone(), density);
    kompoz_runtime::remember_saveable_with(&saver, move || {
        SwipeToDismissBoxState::confirming(initial_value, density, confirm_value_change, positional_threshold)
    })
}

/// A composable that can be dismissed by swiping left or right (Compose's `SwipeToDismissBox`): `content` (in a
/// Row) is dragged horizontally over `background_content` (a Row the content's size, behind it — use `state` to
/// tell the sides apart) between its place and the width away to the end (`enable_dismiss_from_start_to_end`) or
/// to the start (`enable_dismiss_from_end_to_start`), mirrored right to left; `gestures_enabled`, it takes drags
/// while settled. Settled in a direction, `on_dismiss` is called with it.
#[kompozable]
#[allow(clippy::too_many_arguments, clippy::redundant_closure)]
pub fn SwipeToDismissBox(
    state: SwipeToDismissBoxState,
    background_content: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enable_dismiss_from_start_to_end: bool,
    #[default(true)] enable_dismiss_from_end_to_start: bool,
    #[default(true)] gestures_enabled: bool,
    #[default(|_| {})] on_dismiss: impl Fn(SwipeToDismissBoxValue) + 'static,
    content: impl Fn() + 'static,
) {
    let fling_behavior: Option<Rc<dyn FlingBehavior>> = if state.use_fling_behavior() {
        let fling: Rc<dyn FlingBehavior> = AnchoredDraggableDefaults::fling_behavior(
            state.anchored_draggable_state(),
            state.inner.positional_threshold.clone(),
            None,
        );
        Some(fling)
    } else {
        None
    };
    let anchors_state = state.clone();
    Box(
        modifier = named_args!(modifier.anchored_draggable(
            state.anchored_draggable_state(),
            Orientation::Horizontal,
            enabled = gestures_enabled && state.settled_value() == SwipeToDismissBoxValue::Settled,
            fling_behavior = fling_behavior,
        )),
        propagate_min_constraints = true,
        content = || {
            Row(modifier = Modifier.match_parent_size(), content = || background_content());
            Row(
                modifier = Modifier.draggable_anchors(
                    state.anchored_draggable_state(),
                    Orientation::Horizontal,
                    move |size, _| {
                        let state = &anchors_state;
                        let new_anchors = DraggableAnchors(|a: &mut DraggableAnchorsConfig<SwipeToDismissBoxValue>| {
                            let width = size.width as f32;
                            a.at(SwipeToDismissBoxValue::Settled, 0.0);
                            if enable_dismiss_from_start_to_end {
                                a.at(SwipeToDismissBoxValue::StartToEnd, width);
                            }
                            if enable_dismiss_from_end_to_start {
                                a.at(SwipeToDismissBoxValue::EndToStart, -width);
                            }
                        });
                        let is_initialized = state.anchored_draggable_state().anchors().size() > 0;
                        let previous_value = state.current_value();
                        let target_value = state.target_value();
                        let new_target = if !is_initialized && new_anchors.has_position_for(&previous_value) {
                            previous_value
                        } else if new_anchors.has_position_for(&target_value) {
                            target_value
                        } else {
                            SwipeToDismissBoxValue::Settled
                        };
                        (Arc::new(new_anchors) as Arc<dyn DraggableAnchors<SwipeToDismissBoxValue>>, new_target)
                    },
                ),
                content = || content(),
            );
        },
    );
    // (a closure, memoized by what it captures: the callback's identity; the callback itself would have none — hence
    // the function's `clippy::redundant_closure` allowed)
    DismissEffect(state, move |direction| on_dismiss(direction));
}

/// Compose's `LaunchedEffect(state.settledValue, onDismiss) { … }` of SwipeToDismissBox: the effect keyed by the
/// callback's identity as a memoized closure (what a restartable composable keeps it by; the closure given here
/// captures it, so its identity changes with it).
#[kompozable(inline)]
fn DismissEffect(state: SwipeToDismissBoxState, #[memoized] on_dismiss: impl Fn(SwipeToDismissBoxValue) + 'static) {
    let on_dismiss_id = on_dismiss.id();
    let on_dismiss = on_dismiss.into_value();
    LaunchedEffect((state.settled_value(), on_dismiss_id), async move {
        // Only call when state is settled in a dismissed direction.
        if state.settled_value() != SwipeToDismissBoxValue::Settled {
            on_dismiss(state.dismiss_direction());
        }
    });
}

/// The defaults of [`SwipeToDismissBox`] and [`SwipeToDismissBoxState`] (Compose's `object
/// SwipeToDismissBoxDefaults`; its property an associated function:
/// `SwipeToDismissBoxDefaults::positional_threshold()`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SwipeToDismissBoxDefaults;

impl SwipeToDismissBoxDefaults {
    /// The default positional threshold, 56 dp in this density whatever the distance (Compose's composable
    /// `positionalThreshold`).
    #[track_caller]
    pub fn positional_threshold() -> Rc<dyn Fn(f32) -> f32> {
        kompozable_block!({
            let density = LocalDensity.current();
            Rc::new(move |_total_distance: f32| density.to_px(Dp(56.0))) as Rc<dyn Fn(f32) -> f32>
        })
    }
}

/// The fling a deprecated state's swipe must be past to settle on the next value anyway (Compose's private
/// `DismissVelocityThreshold`).
const DismissVelocityThreshold: Dp = Dp(125.0);
