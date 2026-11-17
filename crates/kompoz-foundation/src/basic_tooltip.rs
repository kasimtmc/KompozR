//! `BasicTooltipBox`, as Compose's BasicTooltip.kt: an anchor that shows a tooltip in a popup where a position
//! provider says, on a long press (touch, stylus), while the mouse is over it, from accessibility's long click, or
//! from code (its [`BasicTooltipState`]); one tooltip at a time (they share a mutator mutex), for a while or until
//! dismissed.

use crate::gestures::{AwaitPointerEventScopeGestures, LocalFuture, PointerInputScopeGestures};
use crate::gestures::{LongPressResult, wait_for_long_press};
use crate::layout::Box;
use kompoz_animation::{MutatePriority, MutatorMutex};
use kompoz_coroutines::{CoroutineScope, cancel_current, coroutine_scope, remember_coroutine_scope, with_timeout};
use kompoz_runtime::{
    DisposableEffect, MutableState, kompozable, mutable_state_of, named_args, on_dispose, remember_keyed,
};
use kompoz_ui::Modifier;
use kompoz_ui::input::{PointerEventPass, PointerEventType, PointerInputScope, PointerType};
use kompoz_ui::semantics::LiveRegionMode;
use kompoz_ui::strings::LocalStrings;
use kompoz_ui::window::{Popup, PopupPositionProvider, PopupProperties};
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

/// The defaults of tooltips (Compose's BasicTooltipDefaults).
pub struct BasicTooltipDefaults;

impl BasicTooltipDefaults {
    /// How long a tooltip that isn't persistent shows, ms (Compose's `TooltipDuration`).
    pub const TooltipDuration: i64 = 1500;

    /// The mutex all tooltips share unless given another: one shows at a time (Compose's `GlobalMutatorMutex`).
    #[allow(non_snake_case)]
    pub fn GlobalMutatorMutex() -> MutatorMutex {
        thread_local! {
            static GLOBAL: MutatorMutex = MutatorMutex::new();
        }
        GLOBAL.with(MutatorMutex::clone)
    }
}

/// A tooltip's state (Compose's `interface BasicTooltipState`): whether it shows, whether it stays until dismissed,
/// and showing and dismissing it.
pub trait BasicTooltipState {
    /// Whether it shows (Compose's `isVisible`, a state: read in composition, it composes again when it changes).
    fn is_visible(&self) -> bool;

    /// Whether it stays until dismissed; else it goes after [`BasicTooltipDefaults::TooltipDuration`] (Compose's
    /// `isPersistent`).
    fn is_persistent(&self) -> bool;

    /// Shows it (Compose's suspending `show`): done when it goes (dismissed, timed out, another shown through its
    /// mutex with `mutate_priority` at least as high).
    fn show(&self, mutate_priority: MutatePriority) -> LocalFuture<'static, ()>;

    /// Hides it (Compose's `dismiss`).
    fn dismiss(&self);

    /// Cleans up when its tooltip box leaves the composition: a `show` waiting is cancelled (Compose's `onDispose`).
    fn on_dispose(&self);
}

/// A tooltip's state, hidden unless `initial_is_visible` (Compose's `BasicTooltipState(…)`): persistent (shown until
/// dismissed) unless `is_persistent` is false, one tooltip at a time through `mutator_mutex`.
///
/// Kotlin: `BasicTooltipState(initialIsVisible: Boolean = false, isPersistent: Boolean = true, mutatorMutex: MutatorMutex = BasicTooltipDefaults.GlobalMutatorMutex)`
pub fn BasicTooltipState(
    initial_is_visible: bool,
    is_persistent: bool,
    mutator_mutex: MutatorMutex,
) -> Rc<dyn BasicTooltipState> {
    Rc::new(BasicTooltipStateImpl::new(initial_is_visible, is_persistent, mutator_mutex))
}

/// A tooltip's state remembered while `is_persistent` and `mutator_mutex` stay the same (Compose's
/// `rememberBasicTooltipState`).
#[kompozable]
pub fn remember_basic_tooltip_state(
    #[default(false)] initial_is_visible: bool,
    #[default(true)] is_persistent: bool,
    #[default(BasicTooltipDefaults::GlobalMutatorMutex())] mutator_mutex: MutatorMutex,
) -> Rc<dyn BasicTooltipState> {
    remember_keyed((is_persistent, mutator_mutex.clone()), move || {
        Rc::new(BasicTooltipStateImpl::new(initial_is_visible, is_persistent, mutator_mutex))
            as Rc<dyn BasicTooltipState>
    })
}

/// Compose's BasicTooltipStateImpl.
struct BasicTooltipStateImpl {
    is_visible: MutableState<bool>,
    is_persistent: bool,
    mutator_mutex: MutatorMutex,
    /// The `show` waiting (Compose's `job`, its continuation): cancelled on dispose.
    job: Rc<RefCell<Option<Rc<Shown>>>>,
}

impl BasicTooltipStateImpl {
    fn new(initial_is_visible: bool, is_persistent: bool, mutator_mutex: MutatorMutex) -> BasicTooltipStateImpl {
        BasicTooltipStateImpl {
            is_visible: mutable_state_of(initial_is_visible),
            is_persistent,
            mutator_mutex,
            job: Rc::new(RefCell::new(None)),
        }
    }
}

/// A `show` waiting until it is cancelled (Compose's `suspendCancellableCoroutine` that never resumes).
#[derive(Default)]
struct Shown {
    cancelled: Cell<bool>,
    waker: RefCell<Option<Waker>>,
}

impl Shown {
    fn cancel(&self) {
        self.cancelled.set(true);
        if let Some(w) = self.waker.borrow_mut().take() {
            w.wake();
        }
    }
}

/// Waits until its `Shown` is cancelled.
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
/// out, cancelled, interrupted by another).
struct HideOnDrop(MutableState<bool>);

impl Drop for HideOnDrop {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

impl BasicTooltipState for BasicTooltipStateImpl {
    fn is_visible(&self) -> bool {
        self.is_visible.get()
    }

    fn is_persistent(&self) -> bool {
        self.is_persistent
    }

    fn show(&self, mutate_priority: MutatePriority) -> LocalFuture<'static, ()> {
        let (visible, job) = (self.is_visible.clone(), self.job.clone());
        let cancellable_show = move || {
            let (visible, job) = (visible.clone(), job.clone());
            async move {
                let shown = Rc::new(Shown::default());
                visible.set(true);
                *job.borrow_mut() = Some(shown.clone());
                WaitCancelled(shown).await;
            }
        };
        let (hide, persistent, mutex) = (self.is_visible.clone(), self.is_persistent, self.mutator_mutex.clone());
        // Shown for TooltipDuration, or until dismissed when persistent.
        let mutation = async move {
            let _hide = HideOnDrop(hide);
            if persistent {
                cancellable_show().await;
            } else {
                let limit = Duration::from_millis(BasicTooltipDefaults::TooltipDuration as u64);
                let _ = with_timeout(limit, move |_| cancellable_show()).await;
            }
        };
        std::boxed::Box::pin(async move {
            named_args!(mutex.mutate(mutate_priority, mutation)).await;
            // (it ends only cancelled — disposed of, or timed out: Kotlin's exception goes on out of `show`)
            cancel_current::<()>().await
        })
    }

    fn dismiss(&self) {
        self.is_visible.set(false);
    }

    fn on_dispose(&self) {
        if let Some(job) = self.job.borrow().as_ref() {
            job.cancel();
        }
    }
}

/// A state compared by its instance (`pointerInput(state)`'s key).
#[derive(Clone)]
struct StateKey(Rc<dyn BasicTooltipState>);

impl PartialEq for StateKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// An anchor showing `tooltip` in a popup placed by `position_provider` (Compose's `BasicTooltipBox`): on a long
/// press of a finger or a stylus (the press then goes no further), while a mouse is over it, from accessibility's
/// long click (labelled "show tooltip"), or from code through `state`; `enable_user_input` false: only from code.
/// The popup takes the focus if `focusable`; a press outside dismisses it. `content` is laid out in a Box
/// (`propagate_min_constraints` as a Box's).
#[kompozable]
pub fn BasicTooltipBox(
    position_provider: Rc<dyn PopupPositionProvider>,
    tooltip: impl Fn() + 'static,
    state: Rc<dyn BasicTooltipState>,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] focusable: bool,
    #[default(true)] enable_user_input: bool,
    #[default(false)] propagate_min_constraints: bool,
    content: impl Fn() + 'static,
) {
    let scope = remember_coroutine_scope();
    let tooltip = Rc::new(tooltip);
    let content = Rc::new(content);
    let (s, provider) = (state.clone(), position_provider.clone());
    WrappedAnchor(enable_user_input, state.clone(), propagate_min_constraints, modifier, move || {
        if s.is_visible() {
            TooltipPopup(provider.clone(), s.clone(), scope.clone(), focusable, tooltip.clone());
        }
        content();
    });
    let s = state.clone();
    DisposableEffect(StateKey(state), move || {
        let s = s.clone();
        on_dispose(move || s.on_dispose())
    });
}

/// The anchor: a Box handling the gestures and the long click (Compose's `WrappedAnchor`).
#[kompozable]
fn WrappedAnchor(
    enable_user_input: bool,
    state: Rc<dyn BasicTooltipState>,
    propagate_min_constraints: bool,
    modifier: Modifier,
    content: impl Fn() + 'static,
) {
    let scope = remember_coroutine_scope();
    let long_press_label = LocalStrings.current().tooltip_label;
    Box(
        modifier = anchor_semantics(
            handle_gestures(modifier, enable_user_input, state.clone()),
            long_press_label,
            enable_user_input,
            state,
            scope,
        ),
        propagate_min_constraints = propagate_min_constraints,
        content = content,
    );
}

/// The tooltip in its popup, an assertive live region titled "tooltip" (Compose's `TooltipPopup`).
#[kompozable]
fn TooltipPopup(
    position_provider: Rc<dyn PopupPositionProvider>,
    state: Rc<dyn BasicTooltipState>,
    scope: CoroutineScope,
    focusable: bool,
    content: Rc<dyn Fn()>,
) {
    let tooltip_description = LocalStrings.current().tooltip_description;
    Popup(
        popup_position_provider = Some(position_provider),
        on_dismiss_request = move || {
            if state.is_visible() {
                scope.launch(state.show_dismissing());
            }
        },
        properties = PopupProperties { focusable, ..PopupProperties::default() },
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

/// What the popup's dismiss request launches (Compose's `scope.launch { state.dismiss() }`).
trait DismissLater {
    fn show_dismissing(&self) -> LocalFuture<'static, ()>;
}

impl DismissLater for Rc<dyn BasicTooltipState> {
    fn show_dismissing(&self) -> LocalFuture<'static, ()> {
        let state = self.clone();
        std::boxed::Box::pin(async move { state.dismiss() })
    }
}

/// A long press (touch, stylus) shows the tooltip and consumes what follows; the mouse entering shows it, leaving
/// dismisses it (Compose's `handleGestures`).
fn handle_gestures(modifier: Modifier, enabled: bool, state: Rc<dyn BasicTooltipState>) -> Modifier {
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
                                let pass = PointerEventPass::Initial;
                                // (the first press)
                                let input_type = s.await_first_down_with(true, pass).await.r#type;
                                if input_type == PointerType::Touch || input_type == PointerType::Stylus {
                                    let long_press = wait_for_long_press(&s, pass).await;
                                    if let LongPressResult::Success = long_press {
                                        // The tooltip shown, the children's click consumed.
                                        cs.launch(state.show(MutatePriority::UserInput));
                                        let changes = s.await_pointer_event_with(pass).await.changes;
                                        for change in &changes {
                                            change.consume();
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
                                if event.changes[0].r#type == PointerType::Mouse {
                                    match event.r#type {
                                        PointerEventType::Enter => {
                                            cs.launch(state.show(MutatePriority::UserInput));
                                        }
                                        PointerEventType::Exit => state.dismiss(),
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

/// Accessibility's long click shows the tooltip (Compose's `anchorSemantics`), the anchor's children merged.
fn anchor_semantics(
    modifier: Modifier,
    label: String,
    enabled: bool,
    state: Rc<dyn BasicTooltipState>,
    scope: CoroutineScope,
) -> Modifier {
    if !enabled {
        return modifier;
    }
    modifier.semantics_with(true, move |c| {
        let (state, scope) = (state.clone(), scope.clone());
        named_args!(c.on_long_click(label = Some(label.clone()), move || {
            scope.launch(state.show(MutatePriority::Default));
            true
        }));
    })
}
