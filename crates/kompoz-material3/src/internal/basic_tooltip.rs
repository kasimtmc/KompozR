//! Material's tooltip box, as Compose's internal BasicTooltip.kt (with Compose for desktop's actual of its
//! strings): material3's newer copy of foundation's `BasicTooltipBox`, on [`TooltipState`] — the tooltip in a popup
//! where a position provider says, shown on a long press (a finger, a stylus; dismissed once let go if it is still
//! shown when its show ends), while a mouse is over the anchor (dismissed on leaving unless persistent), from
//! accessibility's long click, while the anchor has the keyboard focus (Escape dismisses it), or from code; one at a
//! time through a mutator mutex, for a while or until dismissed. A tooltip with an action takes the focus where
//! accessibility or the keyboard needs it (Tab from the anchor moves into it).

use super::accessibility_service_state_provider::remember_accessibility_service_state;
use super::child_parent_semantics::ChildParentSemanticsModifiers;
use crate::tooltip::TooltipState;
use kompoz_animation::{MutableTransitionState, MutatePriority, MutatorMutex};
use kompoz_coroutines::{
    AsFlow, CoroutineScope, CoroutineStart, MutableStateFlow, catch_cancellation, coroutine_scope,
    remember_coroutine_scope, with_timeout,
};
use kompoz_foundation::gestures::{AwaitPointerEventScopeGestures, LocalFuture, PointerInputScopeGestures};
use kompoz_foundation::layout::Box;
use kompoz_runtime::{
    DisposableEffect, MutableState, State, kompozable, mutable_state_of, named_args, on_dispose, remember,
    remember_keyed,
};
use kompoz_ui::Modifier;
use kompoz_ui::focus::FocusState;
use kompoz_ui::input::{
    Key, KeyEvent, KeyEventType, PointerEventPass, PointerEventType, PointerInputScope, PointerType,
};
use kompoz_ui::semantics::LiveRegionMode;
use kompoz_ui::window::{Popup, PopupPositionProvider, PopupProperties};
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

/// A tooltip box (Compose's internal `BasicTooltipBox`): `tooltip` in a popup placed by `position_provider` while
/// `state` is visible, above the anchor `content` (in a Box of `modifier`). The popup takes the focus if `focusable`
/// — or, `has_action`, where an accessibility service needs it or Tab moved into it from the anchor; a press
/// outside or back calls `on_dismiss_request` (none: the state is dismissed). Without `enable_user_input` it shows
/// only from code.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn BasicTooltipBox(
    position_provider: Rc<dyn PopupPositionProvider>,
    tooltip: impl Fn() + 'static,
    state: Rc<dyn TooltipState>,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] on_dismiss_request: Option<Rc<dyn Fn()>>,
    #[default(false)] focusable: bool,
    #[default(true)] enable_user_input: bool,
    #[default(false)] has_action: bool,
    content: impl Fn() + 'static,
) {
    let scope = remember_coroutine_scope();
    let force_focusable_for_keyboard_nav = remember(|| mutable_state_of(false));
    // The focusable value will be forced to true for correct a11y or keyboard navigation behaviors.
    let should_force_focusable_for_a11y =
        has_action && should_force_focusable_for_a11y(force_focusable_for_keyboard_nav.get());

    let tooltip: Rc<dyn Fn()> = Rc::new(tooltip);
    let content: Rc<dyn Fn()> = Rc::new(content);
    let s = state.clone();
    Box(content = move || {
        if s.is_visible() {
            TooltipPopup(
                position_provider.clone(),
                s.clone(),
                on_dismiss_request.clone(),
                scope.clone(),
                focusable || should_force_focusable_for_a11y,
                force_focusable_for_keyboard_nav.clone(),
                tooltip.clone(),
            );
        }

        WrappedAnchor(
            enable_user_input,
            s.clone(),
            force_focusable_for_keyboard_nav.clone(),
            has_action,
            modifier.clone(),
            content.clone(),
        );
    });

    let s = state.clone();
    DisposableEffect(StateKey(state), move || {
        let s = s.clone();
        on_dispose(move || s.on_dispose())
    });
}

/// Whether the popup is made focusable (Compose's private `shouldForceFocusableForA11y`): a touch exploration or
/// switch access service on, or Tab moved into it.
#[kompozable]
fn should_force_focusable_for_a11y(force_focusable_for_keyboard_nav: bool) -> bool {
    let accessibility_state = remember_touch_exploration_or_switch_access_service_state();
    accessibility_state.get() || force_focusable_for_keyboard_nav
}

/// The anchor: a Box handling the gestures, the long click and the keyboard (Compose's private `WrappedAnchor`).
#[kompozable]
fn WrappedAnchor(
    enable_user_input: bool,
    state: Rc<dyn TooltipState>,
    force_keyboard_focusable: MutableState<bool>,
    has_action: bool,
    modifier: Modifier,
    content: Rc<dyn Fn()>,
) {
    let scope = remember_coroutine_scope();
    let long_press_label = BasicTooltipStrings::label();
    let received_keyboard_focus = remember(|| mutable_state_of(false));
    let modifier = keyboard_behavior(
        anchor_semantics(
            handle_gestures(modifier, enable_user_input, state.clone()),
            long_press_label,
            enable_user_input,
            state.clone(),
            scope.clone(),
        ),
        enable_user_input,
        state,
        scope,
        has_action,
        force_keyboard_focusable,
        received_keyboard_focus,
    );
    Box(modifier = modifier, content = move || content());
}

/// The tooltip in its popup, an assertive live region titled "tooltip", not clipped to the window (Compose's
/// private `TooltipPopup`).
#[kompozable]
fn TooltipPopup(
    position_provider: Rc<dyn PopupPositionProvider>,
    state: Rc<dyn TooltipState>,
    on_dismiss_request: Option<Rc<dyn Fn()>>,
    scope: CoroutineScope,
    focusable: bool,
    force_keyboard_focusable: MutableState<bool>,
    content: Rc<dyn Fn()>,
) {
    let tooltip_description = BasicTooltipStrings::description();
    Popup(
        popup_position_provider = Some(position_provider),
        on_dismiss_request = move || match &on_dismiss_request {
            None => {
                if state.is_visible() {
                    let s = state.clone();
                    scope.launch(async move { s.dismiss() });
                    // Make sure keyboard focus is not trapped once tooltip is dismissed.
                    force_keyboard_focusable.set(false);
                }
            }
            Some(on_dismiss_request) => on_dismiss_request(),
        },
        properties = PopupProperties { focusable, clipping_enabled: false, ..PopupProperties::default() },
        content = move || {
            let (description, content) = (tooltip_description.clone(), content.clone());
            Box(
                modifier = Modifier.semantics(move |c| {
                    c.set_live_region(LiveRegionMode::Assertive);
                    c.set_pane_title(description.clone());
                }),
                content = move || content(),
            );
        },
    );
}

/// Sets the long press's flow false once the gesture is over, however it ends (Kotlin's `finally {
/// isLongPressedFlow.tryEmit(false) }`).
struct NotLongPressed(MutableStateFlow<bool>);

impl Drop for NotLongPressed {
    fn drop(&mut self) {
        self.0.set_value(false);
    }
}

/// A long press (a finger, a stylus) shows the tooltip with the highest priority, and consumes the up that ends it;
/// when its show ends with the tooltip still shown, the tooltip is dismissed once the press is let go. A mouse
/// entering shows it; leaving dismisses it unless it is persistent (Compose's private `Modifier.handleGestures`).
fn handle_gestures(modifier: Modifier, enabled: bool, state: Rc<dyn TooltipState>) -> Modifier {
    if !enabled {
        return modifier;
    }
    let (s1, s2) = (state.clone(), state.clone());
    modifier
        .pointer_input(StateKey(state.clone()), move |scope: PointerInputScope| {
            let state = s1.clone();
            async move {
                coroutine_scope(move |cs| async move {
                    scope
                        .await_each_gesture(move |s| {
                            let (state, cs) = (state.clone(), cs.clone());
                            async move {
                                // Long press will finish before or after show so keep track of it, in a flow to
                                // handle both cases
                                let is_long_pressed_flow = MutableStateFlow::new(false);
                                let long_press_timeout = s.view_configuration().long_press_timeout_millis();
                                let pass = PointerEventPass::Initial;
                                // wait for the first down press
                                let input_type = s.await_first_down_with(true, pass).await.r#type;

                                if input_type == PointerType::Touch || input_type == PointerType::Stylus {
                                    let _finally = NotLongPressed(is_long_pressed_flow.clone());
                                    // listen to if there is up gesture within the longPressTimeout limit
                                    let up = s
                                        .with_timeout(long_press_timeout, move |s| async move {
                                            s.wait_for_up_or_cancellation_with(pass).await
                                        })
                                        .await;
                                    if up.is_err() {
                                        // handle long press - Show the tooltip
                                        let (state, flow) = (state.clone(), is_long_pressed_flow.clone());
                                        named_args!(cs.launch(
                                            start = CoroutineStart::UNDISPATCHED,
                                            coroutine = show_while_long_pressed(state, flow)
                                        ));

                                        // consume the children's click handling. Long press may still be in
                                        // progress
                                        let up_event = s.wait_for_up_or_cancellation_with(pass).await;
                                        if let Some(up_event) = up_event {
                                            up_event.consume();
                                        }
                                    }
                                }
                            }
                        })
                        .await
                })
                .await
            }
        })
        .pointer_input(StateKey(state), move |scope: PointerInputScope| {
            let state = s2.clone();
            async move {
                coroutine_scope(move |cs| async move {
                    scope
                        .await_pointer_event_scope(move |s| async move {
                            let pass = PointerEventPass::Main;

                            loop {
                                let event = s.await_pointer_event_with(pass).await;
                                let input_type = event.changes[0].r#type;
                                if input_type == PointerType::Mouse {
                                    match event.r#type {
                                        PointerEventType::Enter => {
                                            cs.launch(state.show(MutatePriority::UserInput));
                                        }
                                        PointerEventType::Exit if !state.is_persistent() => state.dismiss(),
                                        _ => {}
                                    }
                                }
                            }
                        })
                        .await
                })
                .await
            }
        })
}

/// What a long press launches (Compose's `launch(start = UNDISPATCHED) { try { … show(PreventUserInput) } finally
/// { … } }`): the long press told, the tooltip shown; when its show ends (it only ends cancelled — timed out,
/// dismissed, interrupted —, which Kotlin's `finally` catches) with the tooltip still shown, it is dismissed when
/// the press is let go.
async fn show_while_long_pressed(state: Rc<dyn TooltipState>, is_long_pressed_flow: MutableStateFlow<bool>) {
    is_long_pressed_flow.set_value(true);
    let show = state.clone();
    let _ = catch_cancellation(move |_| show.show(MutatePriority::PreventUserInput)).await;
    if state.is_visible() {
        is_long_pressed_flow
            .as_flow()
            .collect_latest(move |is_long_pressed| {
                if !is_long_pressed {
                    state.dismiss();
                }
                std::future::ready(())
            })
            .await;
    }
}

/// Accessibility's long click (labelled `label`) shows the tooltip; on the anchor's semantics, or on a clickable
/// child's that takes them (Compose's private `Modifier.anchorSemantics`, `parentSemantics`).
fn anchor_semantics(
    modifier: Modifier,
    label: String,
    enabled: bool,
    state: Rc<dyn TooltipState>,
    scope: CoroutineScope,
) -> Modifier {
    if !enabled {
        return modifier;
    }
    modifier.parent_semantics(move |c| {
        let (state, scope) = (state.clone(), scope.clone());
        named_args!(c.on_long_click(label = Some(label.clone()), move || {
            scope.launch(state.show(MutatePriority::Default));
            true
        }));
    })
}

/// The keyboard: the anchor focused shows the tooltip (with the highest priority), unfocused dismisses it; Escape
/// dismisses a tooltip shown; Tab from the anchor of a tooltip with an action moves into it (Compose's private
/// `Modifier.keyboardBehavior`).
#[allow(clippy::too_many_arguments)]
fn keyboard_behavior(
    modifier: Modifier,
    enabled: bool,
    state: Rc<dyn TooltipState>,
    scope: CoroutineScope,
    has_action: bool,
    force_keyboard_focusable: MutableState<bool>,
    received_keyboard_focus: MutableState<bool>,
) -> Modifier {
    if !enabled {
        force_keyboard_focusable.set(false);
        return modifier;
    }
    let (focus_state, received) = (state.clone(), received_keyboard_focus.clone());
    modifier
        .on_focus_changed(move |it: FocusState| {
            let (state, received) = (focus_state.clone(), received.clone());
            scope.launch(async move {
                // Tooltip should show when anchor is keyboard focused.
                if it.is_focused() {
                    received.set(true);
                    state.show(MutatePriority::PreventUserInput).await;
                }
                if received.get() && state.is_visible() && !it.is_focused() {
                    received.set(false);
                    state.dismiss();
                }
            });
        })
        .on_preview_key_event(move |it: &KeyEvent| {
            if !state.is_visible() {
                force_keyboard_focusable.set(false);
            } else {
                // Make sure that tabbing from the anchor navigates to tooltip with action.
                if has_action && is_tab(it) {
                    force_keyboard_focusable.set(true);
                    return true;
                }
                // Escape key should dismiss a currently displayed tooltip.
                if is_escape(it) {
                    received_keyboard_focus.set(false);
                    state.dismiss();
                    return true;
                }
            }
            false
        })
}

/// A tooltip's state remembered while `is_persistent` and `mutator_mutex` stay the same (Compose's internal
/// `rememberBasicTooltipState`): hidden unless `initial_is_visible`, persistent unless `is_persistent` is false,
/// one tooltip at a time through `mutator_mutex`.
#[kompozable]
pub(crate) fn remember_basic_tooltip_state(
    #[default(false)] initial_is_visible: bool,
    #[default(true)] is_persistent: bool,
    #[default(BasicTooltipDefaults::GlobalMutatorMutex())] mutator_mutex: MutatorMutex,
) -> Rc<dyn TooltipState> {
    remember_keyed((is_persistent, mutator_mutex.clone()), move || {
        Rc::new(BasicTooltipStateImpl::new(initial_is_visible, is_persistent, mutator_mutex)) as Rc<dyn TooltipState>
    })
}

/// A tooltip's state (Compose's internal `BasicTooltipState(…)`), as
/// [`remember_basic_tooltip_state`]'s, not remembered.
///
/// Kotlin: `BasicTooltipState(initialIsVisible: Boolean = false, isPersistent: Boolean = true, mutatorMutex: MutatorMutex = BasicTooltipDefaults.GlobalMutatorMutex): TooltipState`
pub(crate) fn BasicTooltipState(
    initial_is_visible: bool,
    is_persistent: bool,
    mutator_mutex: MutatorMutex,
) -> Rc<dyn TooltipState> {
    Rc::new(BasicTooltipStateImpl::new(initial_is_visible, is_persistent, mutator_mutex))
}

/// Compose's private `BasicTooltipStateImpl`: its visibility a state of its own (the transition, still, is only
/// made).
struct BasicTooltipStateImpl {
    is_visible: MutableState<bool>,
    is_persistent: bool,
    mutator_mutex: MutatorMutex,
    transition: MutableTransitionState<bool>,
    /// The continuation used to clean up (Compose's `job`): the `show` waiting, cancelled on dispose.
    job: Rc<RefCell<Option<Rc<Shown>>>>,
}

impl BasicTooltipStateImpl {
    fn new(initial_is_visible: bool, is_persistent: bool, mutator_mutex: MutatorMutex) -> BasicTooltipStateImpl {
        BasicTooltipStateImpl {
            is_visible: mutable_state_of(initial_is_visible),
            is_persistent,
            mutator_mutex,
            transition: MutableTransitionState::new(false),
            job: Rc::new(RefCell::new(None)),
        }
    }
}

/// A `show` waiting until it is cancelled (Compose's `suspendCancellableCoroutine` that is never resumed).
#[derive(Default)]
struct Shown {
    cancelled: std::cell::Cell<bool>,
    waker: RefCell<Option<Waker>>,
}

impl Shown {
    /// Compose's `continuation.cancel()`.
    fn cancel(&self) {
        self.cancelled.set(true);
        if let Some(w) = self.waker.borrow_mut().take() {
            w.wake();
        }
    }
}

/// Waits until its [`Shown`] is cancelled.
struct WaitCancelled(Rc<Shown>);

impl Future for WaitCancelled {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0.cancelled.get() {
            return Poll::Ready(());
        }
        *self.0.waker.borrow_mut() = Some(cx.waker().clone());
        Poll::Pending
    }
}

/// Hides the tooltip as `show`'s mutation ends, however it ends (Kotlin's `finally { isVisible = false }`: timed
/// out, cancelled, interrupted).
struct HideOnDrop(MutableState<bool>);

impl Drop for HideOnDrop {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

impl TooltipState for BasicTooltipStateImpl {
    fn transition(&self) -> MutableTransitionState<bool> {
        self.transition.clone()
    }

    fn is_visible(&self) -> bool {
        self.is_visible.get()
    }

    fn is_persistent(&self) -> bool {
        self.is_persistent
    }

    /// Shows the tooltip of this state, the other tooltips of its mutex dismissed (Compose's `show`): until
    /// dismissed if it is persistent or shown for user input, else for [`BasicTooltipDefaults::TooltipDuration`].
    /// It ends only cancelled (Kotlin's CancellationException out of it): the coroutine awaiting it is cancelled.
    fn show(&self, mutate_priority: MutatePriority) -> LocalFuture<'static, ()> {
        let (visible, job) = (self.is_visible.clone(), self.job.clone());
        let cancellable_show = move || {
            let (visible, job) = (visible.clone(), job.clone());
            async move {
                let continuation = Rc::new(Shown::default());
                visible.set(true);
                *job.borrow_mut() = Some(continuation.clone());
                WaitCancelled(continuation).await;
            }
        };
        let (hide, persistent, mutex) = (self.is_visible.clone(), self.is_persistent, self.mutator_mutex.clone());
        // Show associated tooltip for [TooltipDuration] amount of time or until tooltip is explicitly dismissed
        // depending on [isPersistent].
        let mutation = async move {
            // timeout or cancellation has occurred and we close out the current tooltip.
            let _finally = HideOnDrop(hide);
            if persistent || mutate_priority == MutatePriority::UserInput {
                cancellable_show().await;
            } else {
                let duration = Duration::from_millis(BasicTooltipDefaults::TooltipDuration as u64);
                let _ = with_timeout(duration, move |_| cancellable_show()).await;
            }
        };
        std::boxed::Box::pin(async move {
            named_args!(mutex.mutate(mutate_priority, mutation)).await;
            // (cancelled or timed out: Kotlin's exception goes on out of `show`)
            kompoz_coroutines::cancel_current::<()>().await
        })
    }

    /// Dismisses the tooltip of this state if it is shown (Compose's `dismiss`).
    fn dismiss(&self) {
        self.is_visible.set(false);
    }

    /// Cleans up the mutex when the tooltip of this state leaves the composition (Compose's `onDispose`).
    fn on_dispose(&self) {
        if let Some(job) = self.job.borrow().as_ref() {
            job.cancel();
        }
    }
}

/// The defaults of material3's tooltip boxes (Compose's internal `BasicTooltipDefaults`).
pub(crate) struct BasicTooltipDefaults;

impl BasicTooltipDefaults {
    /// How long a tooltip that isn't persistent shows, ms (Compose's `TooltipDuration`).
    pub(crate) const TooltipDuration: i64 = 1500;

    /// The mutex the tooltips share unless given another: one shows at a time (Compose's `GlobalMutatorMutex`).
    pub(crate) fn GlobalMutatorMutex() -> MutatorMutex {
        thread_local! {
            static GLOBAL: MutatorMutex = MutatorMutex::new();
        }
        GLOBAL.with(MutatorMutex::clone)
    }
}

/// The tooltip box's texts (Compose's internal `expect object BasicTooltipStrings`, Compose for desktop's actual:
/// English texts, copied from foundation's, until it is localized).
pub(crate) struct BasicTooltipStrings;

impl BasicTooltipStrings {
    /// The long click's label (Compose's `label()`).
    pub(crate) fn label() -> String {
        "show tooltip".to_string()
    }

    /// The popup's pane title (Compose's `description()`).
    pub(crate) fn description() -> String {
        "tooltip".to_string()
    }
}

/// Whether a touch exploration or a switch access service is on (Compose's private
/// `rememberTouchExplorationOrSwitchAccessServiceState`).
#[kompozable]
fn remember_touch_exploration_or_switch_access_service_state() -> State<bool> {
    remember_accessibility_service_state(
        listen_to_touch_exploration_state = true,
        listen_to_switch_access_state = true,
        listen_to_voice_access_state = false,
    )
}

/// Compose's private `KeyEvent.isTab`.
fn is_tab(event: &KeyEvent) -> bool {
    event.r#type == KeyEventType::KeyDown && event.key == Key::Tab
}

/// Compose's private `KeyEvent.isEscape`.
fn is_escape(event: &KeyEvent) -> bool {
    event.r#type == KeyEventType::KeyDown && event.key == Key::Escape
}

/// A state compared by its instance (`pointerInput(state)`'s and `DisposableEffect(state)`'s key).
#[derive(Clone)]
struct StateKey(Rc<dyn TooltipState>);

impl PartialEq for StateKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
