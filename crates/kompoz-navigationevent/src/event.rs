//! NavigationEvent.kt, NavigationEventInfo.kt, NavigationEventTransitionState.kt, NavigationEventHistory.kt.

use kompoz_runtime::named;
use std::any::Any;
use std::fmt;
use std::rc::Rc;

/// A system navigation event — a predictive back gesture's step, a back button's press — in one form whatever made
/// it (Compose's `NavigationEvent`): the edge a swipe came from ([`EDGE_LEFT`](Self::EDGE_LEFT),
/// [`EDGE_RIGHT`](Self::EDGE_RIGHT), [`EDGE_NONE`](Self::EDGE_NONE) for what isn't a swipe), how far along it is
/// (0 to 1; a press has 0), the touch's position on the screen in px (0 for a key) and when it happened (ms).
/// Made by [`NavigationEvent()`](NavigationEvent()) with Compose's defaults; Kotlin reads its fields as properties.
#[derive(Clone, Copy, PartialEq, Debug, kompoz_runtime::KotlinProperties)]
pub struct NavigationEvent {
    /// Kotlin's `@SwipeEdge Int`: one of the `EDGE_` constants.
    pub swipe_edge: i32,
    pub progress: f32,
    pub touch_x: f32,
    pub touch_y: f32,
    pub frame_time_millis: i64,
}

impl NavigationEvent {
    /// The gesture comes from the left edge of the screen.
    pub const EDGE_LEFT: i32 = 0;
    /// The gesture comes from the right edge of the screen.
    pub const EDGE_RIGHT: i32 = 1;
    /// Not an edge swipe: a 3-button navigation's press, a hardware back button.
    pub const EDGE_NONE: i32 = 2;
}

impl Default for NavigationEvent {
    fn default() -> Self {
        NavigationEvent {
            swipe_edge: NavigationEvent::EDGE_NONE,
            progress: 0.0,
            touch_x: 0.0,
            touch_y: 0.0,
            frame_time_millis: 0,
        }
    }
}

impl fmt::Display for NavigationEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "NavigationEvent(touchX={}, touchY={}, progress={}, swipeEdge={}, frameTimeMillis={})",
            self.touch_x, self.touch_y, self.progress, self.swipe_edge, self.frame_time_millis
        )
    }
}

/// Compose's `NavigationEvent(swipeEdge = EDGE_NONE, progress = 0f, touchX = 0f, touchY = 0f, frameTimeMillis = 0)`,
/// named arguments in a kompozable body (and Kotlin's syntax); plain code writes the struct.
#[named(type)]
pub fn NavigationEvent(
    #[default(NavigationEvent::EDGE_NONE)] swipe_edge: i32,
    #[default(0.0)] progress: f32,
    #[default(0.0)] touch_x: f32,
    #[default(0.0)] touch_y: f32,
    #[default(0)] frame_time_millis: i64,
) -> NavigationEvent {
    NavigationEvent { swipe_edge, progress, touch_x, touch_y, frame_time_millis }
}

/// Equality of infos of any type: two infos are equal when they are of one type and `==` (what Kotlin's
/// `equals` of a data class is). Every `PartialEq` type has it.
pub trait DynEq {
    fn dyn_eq(&self, other: &dyn Any) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T: PartialEq + 'static> DynEq for T {
    fn dyn_eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|o| self == o)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// What an app tells of a navigation state — a screen, a route — beside its navigation events (Compose's abstract
/// class `NavigationEventInfo`): a data holder compared structurally, as Kotlin's `data class` is (`PartialEq`), so
/// that history comparisons and recompositions work. A type says it is one: `impl NavigationEventInfo for
/// MyInfo {}` (Kotlin's `: NavigationEventInfo()`); [`NavigationEventInfoNone`] is the info of none.
///
/// ```
/// # use kompoz_navigationevent::NavigationEventInfo;
/// #[derive(Clone, PartialEq, Debug)]
/// struct Screen(&'static str);
/// impl NavigationEventInfo for Screen {}
/// ```
///
/// An info of any type, `Rc<dyn NavigationEventInfo>`, is one too (Kotlin's `NavigationEventHandler<NavigationEventInfo>`,
/// a handler whose infos are of any type): the info it holds is what history and the composables see.
pub trait NavigationEventInfo: DynEq + fmt::Debug + 'static {
    /// It as an info of any type (what history holds).
    #[doc(hidden)]
    fn erased(self: Rc<Self>) -> Rc<dyn NavigationEventInfo>
    where
        Self: Sized,
    {
        self
    }
}

/// An info of any type: the one it holds.
impl NavigationEventInfo for Rc<dyn NavigationEventInfo> {
    fn erased(self: Rc<Self>) -> Rc<dyn NavigationEventInfo> {
        (*self).clone()
    }
}

impl dyn NavigationEventInfo {
    /// Whether it is a `T` (Kotlin's `info is T`).
    pub fn is<T: 'static>(&self) -> bool {
        self.as_any().is::<T>()
    }
    /// It as a `T`, if it is one (Kotlin's `info as? T`).
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}

impl PartialEq for dyn NavigationEventInfo {
    fn eq(&self, other: &Self) -> bool {
        self.dyn_eq(other.as_any())
    }
}

/// The info of none: what a navigation event carries when nothing is told of the UI's state (Compose's
/// `NavigationEventInfo.None`, a null object).
///
/// Kotlin: `NavigationEventInfo.None`
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct NavigationEventInfoNone;

impl NavigationEventInfo for NavigationEventInfoNone {}

/// The physical state of a navigation gesture (Compose's sealed class `NavigationEventTransitionState`), apart from
/// the history (what is on the stack): [`Idle`](Self::Idle) with no gesture under way, or
/// [`InProgress`](Self::InProgress) with its latest event and its direction
/// ([`TRANSITIONING_BACK`](Self::TRANSITIONING_BACK) or [`TRANSITIONING_FORWARD`](Self::TRANSITIONING_FORWARD)),
/// from its start (`onBackStarted`) through its steps to its completion or cancellation (back to `Idle`).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum NavigationEventTransitionState {
    /// No gesture under way (the default).
    #[default]
    Idle,
    /// A gesture under way: its latest event (touch, progress 0 to 1) and its direction.
    InProgress { latest_event: NavigationEvent, direction: i32 },
}

impl NavigationEventTransitionState {
    /// The direction isn't known (Compose's internal `TRANSITIONING_UNKNOWN`).
    pub(crate) const TRANSITIONING_UNKNOWN: i32 = 0;
    /// A forward transition under way.
    pub const TRANSITIONING_FORWARD: i32 = 1;
    /// A back transition under way.
    pub const TRANSITIONING_BACK: i32 = -1;

    /// Kotlin's `InProgress(latestEvent, direction)`.
    pub fn in_progress(latest_event: NavigationEvent, direction: i32) -> NavigationEventTransitionState {
        NavigationEventTransitionState::InProgress { latest_event, direction }
    }

    /// Whether it is `Idle` (Kotlin's `state is Idle`).
    pub fn is_idle(&self) -> bool {
        matches!(self, NavigationEventTransitionState::Idle)
    }

    /// Whether a gesture is under way (Kotlin's `state is InProgress`).
    pub fn is_in_progress(&self) -> bool {
        matches!(self, NavigationEventTransitionState::InProgress { .. })
    }

    /// The gesture's latest event, while one is under way (Kotlin's `(state as InProgress).latestEvent`).
    pub fn latest_event(&self) -> Option<NavigationEvent> {
        match self {
            NavigationEventTransitionState::InProgress { latest_event, .. } => Some(*latest_event),
            NavigationEventTransitionState::Idle => None,
        }
    }

    /// The gesture's direction, while one is under way (Kotlin's `(state as InProgress).direction`).
    pub fn direction(&self) -> Option<i32> {
        match self {
            NavigationEventTransitionState::InProgress { direction, .. } => Some(*direction),
            NavigationEventTransitionState::Idle => None,
        }
    }
}

/// A gesture under way, as Kotlin's `is` / `as` give it (`if (transition is NavigationEventTransitionState.InProgress)
/// transition.latestEvent.progress`): [`NavigationEventTransitionState::InProgress`]'s fields.
///
/// Kotlin: `NavigationEventTransitionState.InProgress`
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct NavigationEventTransitionStateInProgress {
    latest_event: NavigationEvent,
    direction: i32,
}

impl NavigationEventTransitionStateInProgress {
    /// The gesture's latest event.
    pub fn latest_event(&self) -> NavigationEvent {
        self.latest_event
    }

    /// Its direction ([`NavigationEventTransitionState::TRANSITIONING_BACK`] or `TRANSITIONING_FORWARD`).
    pub fn direction(&self) -> i32 {
        self.direction
    }
}

/// Kotlin's `state is InProgress` / `state as? InProgress`.
impl kompoz_runtime::kt::KtAs<NavigationEventTransitionStateInProgress> for NavigationEventTransitionState {
    fn kt_as_safe(&self) -> Option<NavigationEventTransitionStateInProgress> {
        match *self {
            NavigationEventTransitionState::InProgress { latest_event, direction } => {
                Some(NavigationEventTransitionStateInProgress { latest_event, direction })
            }
            NavigationEventTransitionState::Idle => None,
        }
    }
}

impl fmt::Display for NavigationEventTransitionState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NavigationEventTransitionState::Idle => write!(f, "Idle()"),
            NavigationEventTransitionState::InProgress { latest_event, direction } => {
                write!(f, "InProgress(latestEvent={latest_event}, direction={direction})")
            }
        }
    }
}

/// A snapshot of the navigation history (Compose's `NavigationEventHistory`): the stack from the root to the leaf
/// ([`merged_history`](Self::merged_history)) and where the active destination is in it
/// ([`current_index`](Self::current_index), -1 when it is empty). Stable during a gesture: it changes when the
/// active handler does, or its info.
#[derive(Clone)]
pub struct NavigationEventHistory {
    current_info: Option<Rc<dyn NavigationEventInfo>>,
    back_info: Vec<Rc<dyn NavigationEventInfo>>,
    forward_info: Vec<Rc<dyn NavigationEventInfo>>,
    current_index: i32,
}

impl NavigationEventHistory {
    /// The empty history (Compose's internal no-argument constructor).
    pub(crate) fn empty() -> NavigationEventHistory {
        NavigationEventHistory {
            current_info: None,
            back_info: Vec::new(),
            forward_info: Vec::new(),
            current_index: -1,
        }
    }

    /// The history of `back_info` + `current_info` + `forward_info`, the current one at `back_info.len()` (Compose's
    /// internal partitions constructor).
    pub(crate) fn new(
        current_info: Rc<dyn NavigationEventInfo>,
        back_info: Vec<Rc<dyn NavigationEventInfo>>,
        forward_info: Vec<Rc<dyn NavigationEventInfo>>,
    ) -> NavigationEventHistory {
        let current_index = back_info.len() as i32;
        NavigationEventHistory { current_info: Some(current_info), back_info, forward_info, current_index }
    }

    /// The stack from the root (0) to the leaf.
    pub fn merged_history(&self) -> Vec<Rc<dyn NavigationEventInfo>> {
        let mut merged = self.back_info.clone();
        merged.extend(self.current_info.clone());
        merged.extend(self.forward_info.iter().cloned());
        merged
    }

    /// Where the active destination is in [`merged_history`](Self::merged_history) (-1: empty).
    pub fn current_index(&self) -> i32 {
        self.current_index
    }
}

impl PartialEq for NavigationEventHistory {
    fn eq(&self, other: &Self) -> bool {
        if self.current_index != other.current_index {
            return false;
        }
        let (a, b) = (self.merged_history(), other.merged_history());
        a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| **x == **y)
    }
}

impl fmt::Debug for NavigationEventHistory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "NavigationEventHistory(currentIndex={}, mergedHistory={:?})",
            self.current_index,
            self.merged_history()
        )
    }
}
