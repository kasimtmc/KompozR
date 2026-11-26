//! Snackbar hosts, as Compose's SnackbarHost.kt: the state queuing snackbars and showing one at a time
//! ([`SnackbarHostState`], its suspending `show_snackbar`), what is shown ([`SnackbarData`], [`SnackbarVisuals`]),
//! how it ended ([`SnackbarResult`]), how long it stays ([`SnackbarDuration`], longer where the platform's
//! accessibility settings ask), and the host showing the current one ([`SnackbarHost`]), fading and scaling it in
//! and out.

use crate::internal::strings::{Strings, get_string};
use crate::snackbar::DataSnackbar;
use crate::tokens::MotionSchemeKeyTokens;
use kompoz_animation::{Animatable, AnimationSpec};
use kompoz_coroutines::sync::Mutex;
use kompoz_coroutines::{LaunchedEffect, delay};
use kompoz_foundation::prelude::*;
use kompoz_runtime::{
    MutableState, SnapshotMutationPolicy, State, key, kompozable, mutable_confined_state_of_with_policy,
    mutable_state_of, named, named_args, remember,
};
use kompoz_ui::Modifier;
use kompoz_ui::platform::{AccessibilityManager, LocalAccessibilityManager};
use kompoz_ui::semantics::LiveRegionMode;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

/// The state of a [`SnackbarHost`] (Compose's `SnackbarHostState`): the snackbar it shows, and the queue of those
/// to show — one at a time, in the order asked for (a fair suspending mutex, kotlinx's `Mutex`, is the queue).
/// Remember one (`remember(SnackbarHostState::new)`) and hand it to the host. Clones are the same state.
#[derive(Clone)]
pub struct SnackbarHostState {
    inner: Rc<HostInner>,
}

struct HostInner {
    /// Only one [`Snackbar`](crate::Snackbar) can be shown at a time. Since a suspending Mutex is a fair queue, this
    /// manages our message queue and we don't have to maintain one.
    mutex: Mutex,
    /// Compose's `var currentSnackbarData by mutableStateOf<SnackbarData?>(null)` (confined: the data isn't
    /// `Send`; equal when the same data, as Kotlin's `==` of its data).
    current: MutableState<Option<Rc<dyn SnackbarData>>>,
}

impl HostInner {
    fn set_current(&self, data: Option<Rc<dyn SnackbarData>>) {
        self.current.set(data);
    }
}

/// A new host state, showing nothing (Compose's `SnackbarHostState()`).
///
/// Kotlin: `SnackbarHostState()`
pub fn SnackbarHostState() -> SnackbarHostState {
    SnackbarHostState::new()
}

impl Default for SnackbarHostState {
    fn default() -> Self {
        SnackbarHostState::new()
    }
}

/// The same state (Kotlin compares the class by identity).
impl PartialEq for SnackbarHostState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for SnackbarHostState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("SnackbarHostState")
    }
}

impl SnackbarHostState {
    /// A new host state, showing nothing (Compose's `SnackbarHostState()`).
    pub fn new() -> SnackbarHostState {
        SnackbarHostState {
            inner: Rc::new(HostInner {
                mutex: Mutex::new(false),
                current: mutable_confined_state_of_with_policy(None, SnapshotMutationPolicy::new(same_data)),
            }),
        }
    }

    /// The snackbar being shown by the host, if any (Compose's `currentSnackbarData`, a state read).
    pub fn current_snackbar_data(&self) -> Option<Rc<dyn SnackbarData>> {
        self.inner.current.get()
    }

    /// Shows a snackbar of `visuals`, or queues it to be shown, and suspends until it has gone (Compose's
    /// `showSnackbar(visuals)`): at most one is shown at a time — called while one is, this waits until that one
    /// is shown and then answered. The caller cancelled, its snackbar leaves the screen or the queue. Gives
    /// [`SnackbarResult::ActionPerformed`] if its action was clicked, else [`SnackbarResult::Dismissed`] (timed out
    /// or dismissed by the user).
    ///
    /// Kotlin: `showSnackbar(visuals: SnackbarVisuals): SnackbarResult`
    pub fn show_snackbar_visuals(
        &self,
        visuals: Rc<dyn SnackbarVisuals>,
    ) -> impl Future<Output = SnackbarResult> + 'static {
        let inner = self.inner.clone();
        self.inner.mutex.with_lock(None, move || async move {
            /// Kotlin's `finally { currentSnackbarData = null }`.
            struct ClearCurrent(Rc<HostInner>);
            impl Drop for ClearCurrent {
                fn drop(&mut self) {
                    self.0.set_current(None);
                }
            }
            let _finally = ClearCurrent(inner.clone());
            let continuation = Rc::new(Continuation::default());
            inner.set_current(Some(Rc::new(SnackbarDataImpl { visuals, continuation: continuation.clone() })));
            AwaitResult(continuation).await
        })
    }
}

#[named]
impl SnackbarHostState {
    /// Shows a snackbar of `message` — with a button of `action_label` where given, a dismiss button if
    /// `with_dismiss_action` (recommended with an indefinite duration, for accessibility), shown for `duration`
    /// (none: Kotlin's default, [`SnackbarDuration::Short`] without an action, [`SnackbarDuration::Indefinite`] with
    /// one) — or queues it to be shown, and suspends until it has gone (Compose's `showSnackbar(message, …)`; see
    /// [`show_snackbar_visuals`](SnackbarHostState::show_snackbar_visuals)).
    ///
    /// Kotlin: `showSnackbar(message: String, actionLabel: String? = null, withDismissAction: Boolean = false, duration: SnackbarDuration = if (actionLabel == null) SnackbarDuration.Short else SnackbarDuration.Indefinite): SnackbarResult`
    pub fn show_snackbar(
        &self,
        message: String,
        #[default(None)] action_label: Option<String>,
        #[default(false)] with_dismiss_action: bool,
        #[default(None)] duration: Option<SnackbarDuration>,
    ) -> impl Future<Output = SnackbarResult> + 'static {
        let duration = duration.unwrap_or(if action_label.is_none() {
            SnackbarDuration::Short
        } else {
            SnackbarDuration::Indefinite
        });
        self.show_snackbar_visuals(Rc::new(SnackbarVisualsImpl {
            message,
            action_label,
            with_dismiss_action,
            duration,
        }))
    }
}

/// Compose's private `SnackbarVisualsImpl` (a value: equal to another of the same fields).
#[derive(Clone, PartialEq, Debug)]
struct SnackbarVisualsImpl {
    message: String,
    action_label: Option<String>,
    with_dismiss_action: bool,
    duration: SnackbarDuration,
}

impl SnackbarVisuals for SnackbarVisualsImpl {
    fn message(&self) -> String {
        self.message.clone()
    }
    fn action_label(&self) -> Option<String> {
        self.action_label.clone()
    }
    fn with_dismiss_action(&self) -> bool {
        self.with_dismiss_action
    }
    fn duration(&self) -> SnackbarDuration {
        self.duration
    }
}

/// The continuation `show_snackbar` waits on (Compose's `CancellableContinuation<SnackbarResult>`): resumed once
/// with a result; no longer active once resumed or cancelled (its waiter gone).
#[derive(Default)]
struct Continuation {
    result: Cell<Option<SnackbarResult>>,
    cancelled: Cell<bool>,
    waker: RefCell<Option<Waker>>,
}

impl Continuation {
    /// Compose's `continuation.isActive`.
    fn is_active(&self) -> bool {
        self.result.get().is_none() && !self.cancelled.get()
    }

    /// Compose's `continuation.resume(result)`.
    fn resume(&self, result: SnackbarResult) {
        self.result.set(Some(result));
        let waker = self.waker.borrow_mut().take();
        if let Some(w) = waker {
            w.wake();
        }
    }
}

/// Waits for its continuation's result; dropped (the caller cancelled), it cancels the continuation.
struct AwaitResult(Rc<Continuation>);

impl Future for AwaitResult {
    type Output = SnackbarResult;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<SnackbarResult> {
        match self.0.result.get() {
            Some(result) => Poll::Ready(result),
            None => {
                *self.0.waker.borrow_mut() = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

impl Drop for AwaitResult {
    fn drop(&mut self) {
        if self.0.result.get().is_none() {
            self.0.cancelled.set(true);
        }
    }
}

/// Compose's private `SnackbarDataImpl`: its action and dismissal resume the `show_snackbar` waiting, once.
struct SnackbarDataImpl {
    visuals: Rc<dyn SnackbarVisuals>,
    continuation: Rc<Continuation>,
}

impl SnackbarData for SnackbarDataImpl {
    fn visuals(&self) -> Rc<dyn SnackbarVisuals> {
        self.visuals.clone()
    }

    fn perform_action(&self) {
        if self.continuation.is_active() {
            self.continuation.resume(SnackbarResult::ActionPerformed);
        }
    }

    fn dismiss(&self) {
        if self.continuation.is_active() {
            self.continuation.resume(SnackbarResult::Dismissed);
        }
    }
}

/// A host of snackbars (Compose's `SnackbarHost`): shows `host_state`'s current snackbar by `snackbar` (by default
/// a [`DataSnackbar`]), for as long as its duration says (longer where [`LocalAccessibilityManager`] recommends), then
/// dismisses it; each comes in fading in and growing from 80%, and goes fading out and shrinking (the motion
/// scheme's fast effects and fast spatial specs), a polite live region titled "Alert" that accessibility can
/// dismiss. Its show and queue are `host_state`'s.
#[kompozable]
pub fn SnackbarHost(
    host_state: SnackbarHostState,
    #[default(Modifier)] modifier: Modifier,
    #[default(|it: Rc<dyn SnackbarData>| kompoz_runtime::kompozable_block!({ DataSnackbar(it) }))] snackbar: impl Fn(
        Rc<dyn SnackbarData>,
    ) + 'static,
) {
    let current_snackbar_data = host_state.current_snackbar_data();
    let accessibility_manager = LocalAccessibilityManager.current();
    let data = current_snackbar_data.clone();
    LaunchedEffect(DataKey(current_snackbar_data.clone()), async move {
        if let Some(current_snackbar_data) = data {
            let visuals = current_snackbar_data.visuals();
            let duration =
                visuals.duration().to_millis(visuals.action_label().is_some(), accessibility_manager.as_deref());
            delay(duration).await;
            current_snackbar_data.dismiss();
        }
    });
    FadeInFadeOutWithScale(current = host_state.current_snackbar_data(), modifier = modifier, content = snackbar);
}

/// A snackbar's data as an effect's key: the same data (Compose's `SnackbarDataImpl.equals` compares its
/// continuation, so a data equals only itself).
#[derive(Clone)]
struct DataKey(Option<Rc<dyn SnackbarData>>);

impl PartialEq for DataKey {
    fn eq(&self, other: &Self) -> bool {
        same_data(&self.0, &other.0)
    }
}

fn same_data(a: &Option<Rc<dyn SnackbarData>>, b: &Option<Rc<dyn SnackbarData>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// What a snackbar shows (Compose's interface `SnackbarVisuals`): its message, its action's label, whether it has
/// a dismiss button, how long it stays. (An app's own visuals are found again through a data's: `KtAs`.)
pub trait SnackbarVisuals: std::any::Any {
    /// The text shown.
    fn message(&self) -> String;
    /// The action button's label, if any (Compose's `actionLabel`).
    fn action_label(&self) -> Option<String>;
    /// Whether a dismiss button is shown (Compose's `withDismissAction`).
    fn with_dismiss_action(&self) -> bool;
    /// How long it is shown.
    fn duration(&self) -> SnackbarDuration;
}

/// Kotlin's `data.visuals as? MyVisuals`: the visuals a snackbar was shown with, as the app's own type if they are
/// one.
impl<T: std::any::Any + Clone> kompoz_runtime::kt::KtAs<T> for dyn SnackbarVisuals {
    fn kt_as_safe(&self) -> Option<T> {
        let any: &dyn std::any::Any = self;
        any.downcast_ref::<T>().cloned()
    }
}

/// The snackbar a host shows (Compose's interface `SnackbarData`): what it shows, and how it answers the
/// `show_snackbar` waiting. (A data is the same as itself only: compared by identity.)
pub trait SnackbarData {
    /// What it shows.
    fn visuals(&self) -> Rc<dyn SnackbarVisuals>;

    /// Called when the snackbar's action has been performed, to notify the listeners (Compose's `performAction`).
    fn perform_action(&self);

    /// Called when the snackbar is dismissed, by timeout or by the user (Compose's `dismiss`).
    fn dismiss(&self);
}

/// How a snackbar ended (Compose's enum class `SnackbarResult`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SnackbarResult {
    /// The snackbar shown has been dismissed, by timeout or by the user.
    Dismissed,
    /// The snackbar's action has been clicked before the timeout passed.
    ActionPerformed,
}

/// How long a snackbar is shown (Compose's enum class `SnackbarDuration`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SnackbarDuration {
    /// For a short time (4 s).
    Short,
    /// For a long time (10 s).
    Long,
    /// Until it is dismissed or its action clicked.
    Indefinite,
}

impl SnackbarDuration {
    /// How long, ms (Compose's internal `SnackbarDuration.toMillis(hasAction, accessibilityManager)`): 4 s short,
    /// 10 s long, `i64::MAX` (Kotlin's `Long.MAX_VALUE`) indefinite — what `accessibility_manager` recommends for
    /// something holding icons, text and (`has_action`) controls, where there is one.
    // TODO: magic numbers adjustment
    pub(crate) fn to_millis(self, has_action: bool, accessibility_manager: Option<&dyn AccessibilityManager>) -> i64 {
        let original = match self {
            SnackbarDuration::Indefinite => i64::MAX,
            SnackbarDuration::Long => 10000,
            SnackbarDuration::Short => 4000,
        };
        let Some(accessibility_manager) = accessibility_manager else {
            return original;
        };
        accessibility_manager.calculate_recommended_timeout_millis(original, true, true, has_action)
    }
}

/// A transition of an item: its content wrapped (Compose's private typealias `FadeInFadeOutTransition`).
type FadeInFadeOutTransition = Rc<dyn Fn(&dyn Fn())>;

/// An item shown or going (Compose's private `FadeInFadeOutAnimationItem`).
#[derive(Clone)]
struct FadeInFadeOutAnimationItem {
    key: Rc<dyn SnackbarData>,
    transition: FadeInFadeOutTransition,
}

/// Compose's private `FadeInFadeOutState`: the current item, the items shown, the scope to recompose when an item
/// went (a state its readers read: Compose's `currentRecomposeScope`, invalidated).
struct FadeInFadeOutState {
    /// The current data; `None` until the first (Kotlin's `Any()`, equal to no real value).
    current: Option<Option<Rc<dyn SnackbarData>>>,
    items: Vec<FadeInFadeOutAnimationItem>,
}

// TODO: to be replaced with the public customizable implementation
// it's basically tweaked nullable version of Crossfade
/// Shows `current` by `content`, fading and growing it in, and the one before fading and shrinking out (Compose's
/// private `FadeInFadeOutWithScale`).
#[kompozable]
fn FadeInFadeOutWithScale(
    current: Option<Rc<dyn SnackbarData>>,
    #[default(Modifier)] modifier: Modifier,
    content: impl Fn(Rc<dyn SnackbarData>) + 'static,
) {
    let a11y_pane_title = get_string(Strings::SnackbarPaneTitle);
    let state = remember(|| Rc::new(RefCell::new(FadeInFadeOutState { current: None, items: Vec::new() })));
    let scope = remember(|| mutable_state_of(0u64));
    let changed = match &state.borrow().current {
        None => true,
        Some(old) => !same_data(old, &current),
    };
    if changed {
        let mut s = state.borrow_mut();
        s.current = Some(current.clone());
        let mut keys: Vec<Option<Rc<dyn SnackbarData>>> = s.items.iter().map(|it| Some(it.key.clone())).collect();
        if !keys.iter().any(|k| same_data(k, &current)) {
            keys.push(current.clone());
        }
        s.items.clear();
        for key in keys.into_iter().flatten() {
            let (current, state, scope, pane_title) =
                (current.clone(), state.clone(), scope.clone(), a11y_pane_title.clone());
            let item_key = key.clone();
            let transition: FadeInFadeOutTransition = Rc::new(move |children: &dyn Fn()| {
                let is_visible = same_data(&Some(key.clone()), &current);
                let (key_done, state, scope) = (key.clone(), state.clone(), scope.clone());
                // TODO Load the motionScheme tokens from the component tokens file
                let opacity = animated_opacity(
                    MotionSchemeKeyTokens::FastEffects.value::<f32>(),
                    is_visible,
                    on_animation_finish = Rc::new(move || {
                        let still_current =
                            state.borrow().current.as_ref().is_some_and(|c| same_data(c, &Some(key_done.clone())));
                        if !still_current {
                            // leave only the current in the list
                            state.borrow_mut().items.retain(|it| !Rc::ptr_eq(&it.key, &key_done));
                            scope.set(scope.peek().wrapping_add(1));
                        }
                    }),
                );
                let scale = animated_scale(
                    // TODO Load the motionScheme tokens from the component tokens file
                    MotionSchemeKeyTokens::FastSpatial.value::<f32>(),
                    is_visible,
                );
                let (dismissed, pane_title) = (key.clone(), pane_title.clone());
                Box(
                    modifier = named_args!(Modifier.graphics_layer(
                        scale_x = scale.get(),
                        scale_y = scale.get(),
                        alpha = opacity.get()
                    ))
                    .semantics(move |c| {
                        if is_visible {
                            c.set_live_region(LiveRegionMode::Polite);
                        }
                        let dismissed = dismissed.clone();
                        c.dismiss(None, move || {
                            dismissed.dismiss();
                            true
                        });
                        c.set_pane_title(pane_title.clone());
                    }),
                    content = || children(),
                );
            });
            s.items.push(FadeInFadeOutAnimationItem { key: item_key, transition });
        }
    }
    let content = Rc::new(content);
    Box(
        modifier = modifier,
        content = move || {
            scope.get();
            let items = state.borrow().items.clone();
            for FadeInFadeOutAnimationItem { key: item, transition: opacity } in items {
                let content = content.clone();
                key(&(Rc::as_ptr(&item) as *const () as usize), move || {
                    opacity(&|| content(item.clone()));
                });
            }
        },
    );
}

/// An item's opacity, animated to 1 shown and 0 not, `on_animation_finish` called when it gets there (Compose's
/// private `animatedOpacity`).
#[kompozable]
fn animated_opacity(
    animation: Arc<dyn AnimationSpec<f32>>,
    visible: bool,
    #[default(Rc::new(|| {}))] on_animation_finish: Rc<dyn Fn()>,
) -> State<f32> {
    let alpha = remember(|| Animatable(if !visible { 1.0f32 } else { 0.0 }));
    let a = alpha.clone();
    LaunchedEffect(visible, async move {
        named_args!(a.animate_to(if visible { 1.0 } else { 0.0 }, animation_spec = animation)).await;
        on_animation_finish();
    });
    alpha.as_state()
}

/// An item's scale, animated to 1 shown and 0.8 not (Compose's private `animatedScale`).
#[kompozable]
fn animated_scale(animation: Arc<dyn AnimationSpec<f32>>, visible: bool) -> State<f32> {
    let scale = remember(|| Animatable(if !visible { 1.0f32 } else { 0.8 }));
    let s = scale.clone();
    LaunchedEffect(visible, async move {
        named_args!(s.animate_to(if visible { 1.0 } else { 0.8 }, animation_spec = animation)).await;
    });
    scale.as_state()
}
