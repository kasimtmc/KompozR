//! A dialog over the whole window, as Compose's internal BasicEdgeToEdgeDialog.kt (common) with Compose for
//! desktop's actual: what a modal bottom sheet, a full-screen date picker and a search view's expanded state are
//! shown in, its content given the state of a predictive back gesture ([`PredictiveBackState`],
//! [`PredictiveBackStateHandler`]).
//!
//! Compose for desktop's actual is ui's `Dialog` of the dismissal properties given, not limited to the platform's
//! default width or insets (nor to the software keyboard's), of no scrim and no transition; the status and
//! navigation bars' looks (`light_status_bars`, `light_navigation_bars`) are Android's, ignored there. KompozR's
//! [`Dialog`] has no scrim, transition or keyboard inset of its own, and limits its content to the system's insets
//! only where `decor_fits_system_windows` (Compose for desktop's `usePlatformInsets`): it is false here.

use super::back_handler::{BackEventCompat, PredictiveBackHandler, SwipeEdge as BackSwipeEdge};
use crate::content_color::LocalContentColor;
use kompoz_coroutines::Flow;
use kompoz_foundation::layout::Box;
use kompoz_runtime::{
    MutableState, State, StateSource, key, kompozable, mutable_state_of, remember, remember_updated_state,
};
use kompoz_ui::Modifier;
use kompoz_ui::window::{Dialog, DialogProperties};
use std::rc::Rc;

/// A dialog over the whole window (Compose's internal `BasicEdgeToEdgeDialog`, Compose for desktop's actual):
/// `content` given its predictive back state, in a Box of `modifier`; `on_dismiss_request` on back and on a press
/// outside as `properties` say.
#[kompozable]
pub(crate) fn BasicEdgeToEdgeDialog(
    on_dismiss_request: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(DialogProperties::default())] properties: DialogProperties,
    #[default(LocalContentColor.current().luminance() < 0.5)] light_status_bars: bool,
    #[default(LocalContentColor.current().luminance() < 0.5)] light_navigation_bars: bool,
    content: impl Fn(&PredictiveBackState) + 'static,
) {
    // (the system bars' looks are Android's: Compose for desktop's actual ignores them)
    let _ = (light_status_bars, light_navigation_bars);
    let content = Rc::new(content);
    Dialog(
        on_dismiss_request = on_dismiss_request,
        properties = DialogProperties {
            dismiss_on_back_press: properties.dismiss_on_back_press,
            dismiss_on_click_outside: properties.dismiss_on_click_outside,
            use_platform_default_width: false,
            // (Compose for desktop's `usePlatformInsets = false`)
            decor_fits_system_windows: false,
        },
        content = move || {
            let predictive_back_state = remember_predictive_back_state();
            let content = content.clone();
            Box(modifier = modifier.clone(), content = move || content(&predictive_back_state));
        },
    );
}

/// The state of a one-shot back (Compose's internal sealed interface `PredictiveBackState` and its one
/// implementation, `PredictiveBackStateImpl`): its [`value`](PredictiveBackState::value), a state.
#[derive(Clone)]
pub(crate) struct PredictiveBackState {
    value: MutableState<BackEventProgress>,
}

impl PredictiveBackState {
    /// Where the back stands (Compose's `value`, a state read).
    pub(crate) fn value(&self) -> BackEventProgress {
        self.value.get()
    }

    pub(crate) fn set_value(&self, value: BackEventProgress) {
        self.value.set(value);
    }
}

impl PartialEq for PredictiveBackState {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

/// The state describing a one-shot back state, with use in a [`PredictiveBackStateHandler`] (Compose's internal
/// sealed interface `BackEventProgress`). Because the back handler can only be used once, there are three states:
/// [`NotRunning`](BackEventProgress::NotRunning), [`InProgress`](BackEventProgress::InProgress) (a predictive back
/// under way) and [`Completed`](BackEventProgress::Completed).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum BackEventProgress {
    /// There is no predictive back ongoing, and the back has not been completed.
    NotRunning,
    /// There is an ongoing predictive back animation, with the given `progress`.
    InProgress { touch_x: f32, touch_y: f32, progress: f32, swipe_edge: SwipeEdge },
    /// The back has completed.
    Completed,
}

/// The edge a back gesture came from (Compose's internal enum `SwipeEdge`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum SwipeEdge {
    Left,
    Right,
    None,
}

/// A predictive back state, not running to start with (Compose's internal `rememberPredictiveBackState`).
#[kompozable]
pub(crate) fn remember_predictive_back_state() -> PredictiveBackState {
    remember(|| PredictiveBackState { value: mutable_state_of(BackEventProgress::NotRunning) })
}

/// Runs `state` through a back while `enabled` and it isn't completed (Compose's internal
/// `PredictiveBackStateHandler`): in progress through the gesture's steps, completed at its end — `on_back` (the
/// latest given) called then —, not running again if the gesture is called off.
#[kompozable]
pub(crate) fn PredictiveBackStateHandler(
    state: PredictiveBackState,
    #[default(true)] enabled: bool,
    on_back: impl Fn() + 'static,
) {
    // Safely update the current `onBack` lambda when a new one is provided
    let current_on_back: State<Rc<dyn Fn()>> = remember_updated_state!(Rc::new(on_back) as Rc<dyn Fn()>);
    // (Compose's `key(state)`: the state is known by its instance, its value's)
    let identity = StateSource::identity(&state.value) as usize;
    key(&identity, || {
        let enabled = enabled && state.value() != BackEventProgress::Completed;
        let (state, current_on_back) = (state.clone(), current_on_back.clone());
        PredictiveBackHandler(enabled, move |progress: Flow<BackEventCompat>| {
            let (state, current_on_back) = (state.clone(), current_on_back.clone());
            async move {
                // (Kotlin's `catch (e: CancellationException) { state.value = NotRunning; throw e }`)
                let mut not_running_if_cancelled = NotRunningIfCancelled(Some(state.clone()));
                let steps = state.clone();
                progress
                    .collect(move |back_event| {
                        steps.set_value(BackEventProgress::InProgress {
                            touch_x: back_event.touch_x,
                            touch_y: back_event.touch_y,
                            progress: back_event.progress,
                            swipe_edge: match back_event.swipe_edge {
                                BackSwipeEdge::Left => SwipeEdge::Left,
                                BackSwipeEdge::Right => SwipeEdge::Right,
                                BackSwipeEdge::None => SwipeEdge::None,
                            },
                        });
                    })
                    .await;
                not_running_if_cancelled.0 = None;
                state.set_value(BackEventProgress::Completed);
                (current_on_back.get())();
            }
        });
    });
}

/// Sets the state not running when dropped armed: the handler's coroutine cancelled while it collects.
struct NotRunningIfCancelled(Option<PredictiveBackState>);

impl Drop for NotRunningIfCancelled {
    fn drop(&mut self) {
        if let Some(state) = self.0.take() {
            state.set_value(BackEventProgress::NotRunning);
        }
    }
}
