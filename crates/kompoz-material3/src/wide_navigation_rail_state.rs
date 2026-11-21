//! The state of a wide navigation rail, as Compose's WideNavigationRailState.kt (material3 1.5.0-alpha17): whether
//! a [`WideNavigationRail`](crate::WideNavigationRail) is collapsed or expanded ([`WideNavigationRailValue`]), the
//! state an app hoists to watch and set it ([`WideNavigationRailState`], [`remember_wide_navigation_rail_state`]),
//! and what the modal rail adds (its anchored dragging, the predictive back's edge).

use crate::tokens::MotionSchemeKeyTokens;
use kompoz_animation::{Animatable, AnimationSpec, AnimationVector1D, VectorConverter};
use kompoz_foundation::gestures::{AnchoredDraggableState, LocalFuture};
use kompoz_runtime::{
    DerivedState, MutableState, Saveable, SavedValue, Saver, derived_state_of, kompozable, mutable_state_of,
    named_args, remember_saveable_with,
};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

/// Whether a wide navigation rail is collapsed or expanded (Compose's enum `WideNavigationRailValue`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum WideNavigationRailValue {
    /// The state of the rail when it is collapsed.
    Collapsed,
    /// The state of the rail when it is expanded.
    Expanded,
}

impl WideNavigationRailValue {
    /// Its values in order (Kotlin's `entries`).
    pub const ENTRIES: [WideNavigationRailValue; 2] =
        [WideNavigationRailValue::Collapsed, WideNavigationRailValue::Expanded];

    /// Whether it is [`Expanded`](Self::Expanded) (Compose's internal `isExpanded`).
    pub(crate) fn is_expanded(self) -> bool {
        self == WideNavigationRailValue::Expanded
    }
}

/// The other value (Compose's internal `operator fun WideNavigationRailValue.not()`).
impl std::ops::Not for WideNavigationRailValue {
    type Output = WideNavigationRailValue;

    fn not(self) -> WideNavigationRailValue {
        if self == WideNavigationRailValue::Collapsed {
            WideNavigationRailValue::Expanded
        } else {
            WideNavigationRailValue::Collapsed
        }
    }
}

/// Saved as its ordinal (Kotlin's enums are Serializable).
impl Saveable for WideNavigationRailValue {
    fn save(&self) -> SavedValue {
        SavedValue::Int(*self as i64)
    }

    fn restore(saved: &SavedValue) -> Option<Self> {
        let ordinal = usize::try_from(i64::restore(saved)?).ok()?;
        WideNavigationRailValue::ENTRIES.get(ordinal).copied()
    }
}

/// A state an app hoists to watch a wide navigation rail and to collapse or expand it (Compose's interface
/// `WideNavigationRailState`); [`remember_wide_navigation_rail_state`] makes the default one.
pub trait WideNavigationRailState {
    /// Whether the state is currently animating (Compose's `isAnimating`).
    fn is_animating(&self) -> bool;

    /// Whether the rail is going to be expanded or not (Compose's `targetValue`).
    fn target_value(&self) -> WideNavigationRailValue;

    /// Whether the rail is currently expanded or not (Compose's `currentValue`).
    fn current_value(&self) -> WideNavigationRailValue;

    /// Expands the rail with animation, done when it is fully expanded (Compose's suspending `expand`).
    fn expand(&self) -> LocalFuture<'static, ()>;

    /// Collapses the rail with animation, done when it is fully collapsed (Compose's suspending `collapse`).
    fn collapse(&self) -> LocalFuture<'static, ()>;

    /// Collapses the rail with animation if it is expanded, or expands it if it is collapsed, done when it is in its
    /// new state (Compose's suspending `toggle`).
    fn toggle(&self) -> LocalFuture<'static, ()>;

    /// Sets the state to `target_value` without any animation (Compose's suspending `snapTo`).
    fn snap_to(&self, target_value: WideNavigationRailValue) -> LocalFuture<'static, ()>;
}

impl PartialEq for dyn WideNavigationRailState {
    /// The same state (Kotlin's `==` of a class without `equals`: its identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// The default wide navigation rail state, `initial_value` at first, saved and restored (Compose's
/// `rememberWideNavigationRailState`): it moves on the theme's default spatial motion.
///
/// Kotlin: `rememberWideNavigationRailState(initialValue: WideNavigationRailValue = WideNavigationRailValue.Collapsed): WideNavigationRailState`
#[kompozable]
pub fn remember_wide_navigation_rail_state(
    #[default(WideNavigationRailValue::Collapsed)] initial_value: WideNavigationRailValue,
) -> Rc<dyn WideNavigationRailState> {
    // TODO: Load the motionScheme tokens from the component tokens file.
    let animation_spec: Arc<dyn AnimationSpec<f32>> = MotionSchemeKeyTokens::DefaultSpatial.value::<f32>();
    let saver = WideNavigationRailStateImpl::Saver(animation_spec.clone());
    remember_saveable_with(&saver, move || Rc::new(WideNavigationRailStateImpl::new(initial_value, animation_spec)))
        as Rc<dyn WideNavigationRailState>
}

/// The default state (Compose's internal `WideNavigationRailStateImpl`): an Animatable from 0 (collapsed) to 1
/// (expanded), moved by `animation_spec`.
pub(crate) struct WideNavigationRailStateImpl {
    /// The value it was made with (Compose's `var initialValue`, which nothing reads).
    #[allow(dead_code)]
    pub(crate) initial_value: Cell<WideNavigationRailValue>,
    animation_spec: Arc<dyn AnimationSpec<f32>>,
    internal_state: Animatable<f32, AnimationVector1D>,
    current_val: DerivedState<WideNavigationRailValue>,
}

/// The internal value of a collapsed rail (Compose's companion's `Collapsed`).
const Collapsed: f32 = 0.0;
/// The internal value of an expanded rail (Compose's companion's `Expanded`).
const Expanded: f32 = 1.0;

impl WideNavigationRailStateImpl {
    pub(crate) fn new(
        initial_value: WideNavigationRailValue,
        animation_spec: Arc<dyn AnimationSpec<f32>>,
    ) -> WideNavigationRailStateImpl {
        let internal_value = if initial_value.is_expanded() { Expanded } else { Collapsed };
        let internal_state = Animatable((internal_value, f32::VectorConverter));
        let current_val = {
            let internal_state = internal_state.clone();
            derived_state_of(move || {
                if internal_state.value() == Expanded {
                    WideNavigationRailValue::Expanded
                } else {
                    WideNavigationRailValue::Collapsed
                }
            })
        };
        WideNavigationRailStateImpl {
            initial_value: Cell::new(initial_value),
            animation_spec,
            internal_state,
            current_val,
        }
    }

    /// Saves its target value, restored as a state at it (Compose's companion's `Saver(animationSpec)`).
    #[allow(non_snake_case)]
    pub(crate) fn Saver(animation_spec: Arc<dyn AnimationSpec<f32>>) -> Saver<Rc<WideNavigationRailStateImpl>> {
        Saver::new(
            |state: &Rc<WideNavigationRailStateImpl>| state.target_value().save(),
            move |saved| {
                let value = WideNavigationRailValue::restore(saved)?;
                Some(Rc::new(WideNavigationRailStateImpl::new(value, animation_spec.clone())))
            },
        )
    }

    /// Animates the internal value to `target` by the state's spec.
    fn animate_internal_to(&self, target: f32) -> LocalFuture<'static, ()> {
        let animation =
            named_args!(self.internal_state.animate_to(target, animation_spec = self.animation_spec.clone()));
        Box::pin(async move {
            animation.await;
        })
    }
}

impl WideNavigationRailState for WideNavigationRailStateImpl {
    fn is_animating(&self) -> bool {
        self.internal_state.is_running()
    }

    fn target_value(&self) -> WideNavigationRailValue {
        if self.internal_state.target_value() == Expanded {
            WideNavigationRailValue::Expanded
        } else {
            WideNavigationRailValue::Collapsed
        }
    }

    fn current_value(&self) -> WideNavigationRailValue {
        self.current_val.value()
    }

    fn expand(&self) -> LocalFuture<'static, ()> {
        self.animate_internal_to(Expanded)
    }

    fn collapse(&self) -> LocalFuture<'static, ()> {
        self.animate_internal_to(Collapsed)
    }

    fn toggle(&self) -> LocalFuture<'static, ()> {
        self.animate_internal_to(if self.target_value().is_expanded() { Collapsed } else { Expanded })
    }

    fn snap_to(&self, target_value: WideNavigationRailValue) -> LocalFuture<'static, ()> {
        let target = if target_value.is_expanded() { Expanded } else { Collapsed };
        Box::pin(self.internal_state.snap_to(target))
    }
}

/// The state of a modal wide navigation rail (Compose's internal `ModalWideNavigationRailState`): made of the app's
/// state (its target value the start), an anchored dragging between collapsed and expanded that the modal rail's
/// swipes move, animated by `animation_spec`.
///
/// Compose's delegates the interface to the app's state (`WideNavigationRailState by state`) but overrides every
/// member, so nothing of the delegate is kept here.
pub(crate) struct ModalWideNavigationRailState {
    pub(crate) animation_spec: Arc<dyn AnimationSpec<f32>>,
    pub(crate) anchored_draggable_state: AnchoredDraggableState<WideNavigationRailValue>,
}

impl ModalWideNavigationRailState {
    pub(crate) fn new(
        state: &dyn WideNavigationRailState,
        animation_spec: Arc<dyn AnimationSpec<f32>>,
    ) -> ModalWideNavigationRailState {
        ModalWideNavigationRailState {
            animation_spec,
            anchored_draggable_state: AnchoredDraggableState(state.target_value()),
        }
    }

    /// The current position (in pixels) of the rail, or NaN before the offset is initialized (Compose's
    /// `currentOffset`; see [`AnchoredDraggableState::offset`]).
    pub(crate) fn current_offset(&self) -> f32 {
        self.anchored_draggable_state.offset()
    }

    /// Animates the rail to `target_value` by the state's spec (Compose's private `animateTo`).
    fn animate_to(&self, target_value: WideNavigationRailValue) -> LocalFuture<'static, ()> {
        Box::pin(named_args!(
            self.anchored_draggable_state.animate_to(target_value, animation_spec = self.animation_spec.clone())
        ))
    }
}

impl WideNavigationRailState for ModalWideNavigationRailState {
    /// If no swipe or animation is in progress, the value the dismissible modal wide navigation rail is in; if one
    /// is, the value the rail was in before it started (the anchored dragging's settled value).
    fn current_value(&self) -> WideNavigationRailValue {
        // Note: Current Value is mapping to the newly introduced settled value for roughly
        // analogous behavior to internal fork. anchoredDraggableState.currentValue now maps to the
        // value the touch target is closest to, regardless of release/settling.
        self.anchored_draggable_state.settled_value()
    }

    /// If a swipe is in progress, the value the modal rail will animate to if the swipe finishes; if an animation is
    /// running, its target; otherwise the same as [`current_value`](Self::current_value).
    fn target_value(&self) -> WideNavigationRailValue {
        self.anchored_draggable_state.target_value()
    }

    fn is_animating(&self) -> bool {
        self.anchored_draggable_state.is_animation_running()
    }

    fn expand(&self) -> LocalFuture<'static, ()> {
        self.animate_to(WideNavigationRailValue::Expanded)
    }

    fn collapse(&self) -> LocalFuture<'static, ()> {
        self.animate_to(WideNavigationRailValue::Collapsed)
    }

    fn toggle(&self) -> LocalFuture<'static, ()> {
        self.animate_to(!self.target_value())
    }

    fn snap_to(&self, target_value: WideNavigationRailValue) -> LocalFuture<'static, ()> {
        Box::pin(self.anchored_draggable_state.snap_to(target_value))
    }
}

/// Whether a predictive back swipe comes from the rail's edge (Compose's internal `RailPredictiveBackState`).
pub(crate) struct RailPredictiveBackState {
    swipe_edge_matches_rail: MutableState<bool>,
}

impl RailPredictiveBackState {
    pub(crate) fn new() -> RailPredictiveBackState {
        RailPredictiveBackState { swipe_edge_matches_rail: mutable_state_of(true) }
    }

    /// Whether the swipe comes from the edge the rail is at (Compose's `swipeEdgeMatchesRail`).
    pub(crate) fn swipe_edge_matches_rail(&self) -> bool {
        self.swipe_edge_matches_rail.get()
    }

    /// Takes a swipe from the left edge or not, in a right-to-left layout or not (Compose's `update`).
    #[allow(dead_code)] // (Android's dialog tells it the back gesture's edge; Compose for desktop's has no gesture)
    pub(crate) fn update(&self, is_swipe_edge_left: bool, is_rtl: bool) {
        self.swipe_edge_matches_rail.set((is_swipe_edge_left && !is_rtl) || (!is_swipe_edge_left && is_rtl));
    }
}
