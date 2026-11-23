//! navigationevent-compose (androidx.navigationevent.compose): [`LocalNavigationEventDispatcherOwner`],
//! [`remember_navigation_event_dispatcher_owner`], [`NavigationEventState`] and
//! [`remember_navigation_event_state`], the [`NavigationEventHandler`](NavigationEventHandler()),
//! [`NavigationBackHandler`] and [`NavigationForwardHandler`] composables.

use crate::dispatcher::{NavigationEventDispatcher, NavigationEventDispatcherOwner};
use crate::event::{NavigationEvent, NavigationEventInfo, NavigationEventTransitionState};
use crate::handler::{NavigationEventHandler, NavigationEventHandlerCallbacks};
use kompoz_coroutines::LaunchedEffect;
use kompoz_runtime::{
    DisposableEffect, KompozitionLocal, MutableState, ProvidedValue, SideEffect, kompozable, mutable_confined_state_of,
    on_dispose, remember, remember_keyed,
};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

/// The composition local of the nearest [`NavigationEventDispatcherOwner`] (Compose's object
/// `LocalNavigationEventDispatcherOwner`): the host's root owner at the root (CMP's
/// `NavigationEventDispatcherOwnerHostDefaultKey`: the window's), or one provided below (a navigation scope of its
/// own: [`remember_navigation_event_dispatcher_owner`]).
pub struct LocalNavigationEventDispatcherOwnerLocal {
    local: KompozitionLocal<Option<Rc<dyn NavigationEventDispatcherOwner>>>,
}

/// Kotlin: `LocalNavigationEventDispatcherOwner`
pub static LocalNavigationEventDispatcherOwner: LocalNavigationEventDispatcherOwnerLocal =
    LocalNavigationEventDispatcherOwnerLocal { local: KompozitionLocal::new(|| None) };

impl LocalNavigationEventDispatcherOwnerLocal {
    /// The owner provided here, or `None` (nor the host's, outside a host).
    #[track_caller]
    pub fn current(&'static self) -> Option<Rc<dyn NavigationEventDispatcherOwner>> {
        self.local.current()
    }

    /// `navigation_event_dispatcher_owner` provided below (Kotlin's infix `provides`, for
    /// `KompozitionLocalProvider`).
    pub fn provides(
        &'static self,
        navigation_event_dispatcher_owner: Rc<dyn NavigationEventDispatcherOwner>,
    ) -> ProvidedValue {
        self.local.provides(Some(navigation_event_dispatcher_owner))
    }
}

/// The owner [`remember_navigation_event_dispatcher_owner`] makes (Compose's private
/// `ComposeNavigationEventDispatcherOwner`): it holds the dispatcher.
struct ComposeNavigationEventDispatcherOwner {
    navigation_event_dispatcher: NavigationEventDispatcher,
}

impl NavigationEventDispatcherOwner for ComposeNavigationEventDispatcherOwner {
    fn navigation_event_dispatcher(&self) -> NavigationEventDispatcher {
        self.navigation_event_dispatcher.clone()
    }
}

/// The parent [`remember_navigation_event_dispatcher_owner`] takes by default: the nearest owner.
///
/// # Panics
/// None provided ("No NavigationEventDispatcherOwner provided in LocalNavigationEventDispatcherOwner. If you intended
/// to create a root dispatcher, explicitly pass null as the parent.").
#[track_caller]
pub fn default_parent_navigation_event_dispatcher_owner() -> Option<Rc<dyn NavigationEventDispatcherOwner>> {
    Some(LocalNavigationEventDispatcherOwner.current().unwrap_or_else(|| {
        panic!(
            "No NavigationEventDispatcherOwner provided in LocalNavigationEventDispatcherOwner. If you intended to \
             create a root dispatcher, explicitly pass null as the parent."
        )
    }))
}

/// A remembered owner of a dispatcher of its own (Compose's `rememberNavigationEventDispatcherOwner`): a child of
/// `parent`'s (the nearest owner's by default), or a root with `parent = None` (events then come only from an input
/// added to it); enabled as `enabled` says (disabled, it and its children get no events); disposed when it leaves
/// the composition. Provide it as [`LocalNavigationEventDispatcherOwner`] to the content it scopes.
///
/// ```no_run
/// # use kompoz_navigationevent::*;
/// # use kompoz_runtime::*;
/// #[kompozable]
/// fn Scoped(content: impl Fn() + 'static) {
///     let new_owner = remember_navigation_event_dispatcher_owner(enabled = true);
///     KompozitionLocalProvider(&[LocalNavigationEventDispatcherOwner.provides(new_owner)], || content());
/// }
/// ```
#[kompozable]
pub fn remember_navigation_event_dispatcher_owner(
    #[default(true)] enabled: bool,
    #[default(default_parent_navigation_event_dispatcher_owner())] parent: Option<
        Rc<dyn NavigationEventDispatcherOwner>,
    >,
) -> Rc<dyn NavigationEventDispatcherOwner> {
    let key = parent.as_ref().map(|p| SameOwner(p.clone()));
    let local_dispatcher = remember_keyed(key, || match &parent {
        Some(parent) => NavigationEventDispatcher::with_parent(&parent.navigation_event_dispatcher()),
        None => NavigationEventDispatcher::new(),
    });
    let d = local_dispatcher.clone();
    LaunchedEffect(enabled, async move { d.set_is_enabled(enabled) });
    let d = local_dispatcher.clone();
    DisposableEffect(local_dispatcher.clone(), move || on_dispose(move || d.dispose()));
    remember_keyed(local_dispatcher.clone(), move || {
        Rc::new(ComposeNavigationEventDispatcherOwner { navigation_event_dispatcher: local_dispatcher })
            as Rc<dyn NavigationEventDispatcherOwner>
    })
}

/// An owner compared by identity (what `remember(parent)` keys by).
struct SameOwner(Rc<dyn NavigationEventDispatcherOwner>);

impl PartialEq for SameOwner {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

struct StateInner<T> {
    transition_state: MutableState<NavigationEventTransitionState>,
    back_info: MutableState<Vec<T>>,
    current_info: MutableState<T>,
    forward_info: MutableState<Vec<T>>,
    /// The handler it is given to (Compose's `sourceHandler`).
    source_handler: RefCell<Option<Rc<dyn fmt::Debug>>>,
}

/// The Compose side of a handler (Compose's `NavigationEventState<T>`, made by [`remember_navigation_event_state`]):
/// the history partitions the app gives ([`current_info`](Self::current_info), [`back_info`](Self::back_info),
/// [`forward_info`](Self::forward_info)) and the gesture's [`transition_state`](Self::transition_state) the
/// [`NavigationEventHandler`](NavigationEventHandler()) it is given to keeps up to date — what animations follow.
/// Reading them in a composable subscribes it. A cheap handle: clones are the same state (equal by identity).
pub struct NavigationEventState<T> {
    inner: Rc<StateInner<T>>,
}

impl<T> Clone for NavigationEventState<T> {
    fn clone(&self) -> Self {
        NavigationEventState { inner: self.inner.clone() }
    }
}

impl<T> PartialEq for NavigationEventState<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T> fmt::Debug for NavigationEventState<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NavigationEventState@{:p}", Rc::as_ptr(&self.inner))
    }
}

impl<T: NavigationEventInfo + Clone + PartialEq> NavigationEventState<T> {
    fn new(current_info: T, back_info: Vec<T>, forward_info: Vec<T>) -> NavigationEventState<T> {
        NavigationEventState {
            inner: Rc::new(StateInner {
                transition_state: mutable_confined_state_of(NavigationEventTransitionState::Idle),
                back_info: mutable_confined_state_of(back_info),
                current_info: mutable_confined_state_of(current_info),
                forward_info: mutable_confined_state_of(forward_info),
                source_handler: RefCell::new(None),
            }),
        }
    }

    /// The gesture's state, as the handler it is given to has it (`Idle`, or `InProgress` with the gesture's
    /// latest event): it changes at each step.
    pub fn transition_state(&self) -> NavigationEventTransitionState {
        self.inner.transition_state.get()
    }

    /// What back leads to.
    pub fn back_info(&self) -> Vec<T> {
        self.inner.back_info.get()
    }

    /// The active destination's info.
    pub fn current_info(&self) -> T {
        self.inner.current_info.get()
    }

    /// What forward leads to.
    pub fn forward_info(&self) -> Vec<T> {
        self.inner.forward_info.get()
    }
}

/// A remembered [`NavigationEventState`] (Compose's `rememberNavigationEventState`): its infos kept to these
/// (a side effect of each composition); give it to a [`NavigationEventHandler`](NavigationEventHandler()) and
/// read its transition state where the gesture is animated.
#[kompozable]
pub fn remember_navigation_event_state<T: NavigationEventInfo + Clone + PartialEq>(
    current_info: T,
    #[default(Vec::new())] back_info: Vec<T>,
    #[default(Vec::new())] forward_info: Vec<T>,
) -> NavigationEventState<T> {
    let (c, b, f) = (current_info.clone(), back_info.clone(), forward_info.clone());
    let state = remember(move || NavigationEventState::new(c, b, f));
    let s = state.clone();
    SideEffect(move || {
        s.inner.current_info.set(current_info);
        s.inner.back_info.set(back_info);
        s.inner.forward_info.set(forward_info);
    });
    state
}

type Callback = RefCell<Rc<dyn Fn()>>;

/// The handler the composables register (Compose's private `ComposeNavigationEventHandler`): its callbacks the
/// latest given, its transition state told to its state at each step.
struct ComposeNavigationEventHandler<T: NavigationEventInfo + Clone + PartialEq> {
    state: NavigationEventState<T>,
    current_on_forward_cancelled: Callback,
    current_on_forward_completed: Callback,
    current_on_back_cancelled: Callback,
    current_on_back_completed: Callback,
}

impl<T: NavigationEventInfo + Clone + PartialEq> ComposeNavigationEventHandler<T> {
    fn on_transition_state_changed(&self, transition_state: NavigationEventTransitionState) {
        self.state.inner.transition_state.set(transition_state);
    }
}

fn call(callback: &Callback) {
    let f = callback.borrow().clone();
    f();
}

impl<T: NavigationEventInfo + Clone + PartialEq> NavigationEventHandlerCallbacks<T>
    for Rc<ComposeNavigationEventHandler<T>>
{
    fn on_forward_started(&self, handler: &NavigationEventHandler<T>, _: NavigationEvent) {
        self.on_transition_state_changed(handler.transition_state());
    }
    fn on_forward_progressed(&self, handler: &NavigationEventHandler<T>, _: NavigationEvent) {
        self.on_transition_state_changed(handler.transition_state());
    }
    fn on_forward_cancelled(&self, handler: &NavigationEventHandler<T>) {
        self.on_transition_state_changed(handler.transition_state());
        call(&self.current_on_forward_cancelled);
    }
    fn on_forward_completed(&self, handler: &NavigationEventHandler<T>) {
        self.on_transition_state_changed(handler.transition_state());
        call(&self.current_on_forward_completed);
    }
    fn on_back_started(&self, handler: &NavigationEventHandler<T>, _: NavigationEvent) {
        self.on_transition_state_changed(handler.transition_state());
    }
    fn on_back_progressed(&self, handler: &NavigationEventHandler<T>, _: NavigationEvent) {
        self.on_transition_state_changed(handler.transition_state());
    }
    fn on_back_cancelled(&self, handler: &NavigationEventHandler<T>) {
        self.on_transition_state_changed(handler.transition_state());
        call(&self.current_on_back_cancelled);
    }
    fn on_back_completed(&self, handler: &NavigationEventHandler<T>) {
        self.on_transition_state_changed(handler.transition_state());
        call(&self.current_on_back_completed);
    }
}

fn nothing() -> Callback {
    RefCell::new(Rc::new(|| {}))
}

/// Handles navigation gestures with plain callbacks, driven by a hoisted `state` (Compose's
/// `NavigationEventHandler` composable): registered with the nearest dispatcher while composed; the handler composed
/// last among the enabled ones takes an event. Call it unconditionally and use `is_back_enabled` /
/// `is_forward_enabled` (a conditional call changes the order of composition). Its gesture's transition state goes
/// to `state` (what an animation reads); its infos come from `state`. (A gesture may start in a frame before an
/// `enabled` change is applied, and still come here.)
///
/// # Panics
/// No [`LocalNavigationEventDispatcherOwner`] ("No NavigationEventDispatcher was provided via
/// LocalNavigationEventDispatcherOwner"); `state` given to another handler composable too.
#[kompozable]
pub fn NavigationEventHandler<T: NavigationEventInfo + Clone + PartialEq>(
    state: NavigationEventState<T>,
    #[default(true)] is_forward_enabled: bool,
    #[default(|| {})] on_forward_cancelled: impl Fn() + 'static,
    #[default(|| {})] on_forward_completed: impl Fn() + 'static,
    #[default(true)] is_back_enabled: bool,
    #[default(|| {})] on_back_cancelled: impl Fn() + 'static,
    #[default(|| {})] on_back_completed: impl Fn() + 'static,
) {
    let dispatcher = LocalNavigationEventDispatcherOwner
        .current()
        .unwrap_or_else(|| panic!("No NavigationEventDispatcher was provided via LocalNavigationEventDispatcherOwner"))
        .navigation_event_dispatcher();
    let s = state.clone();
    let (callbacks, source_handler) = remember_keyed(state.clone(), move || {
        let callbacks = Rc::new(ComposeNavigationEventHandler {
            state: s.clone(),
            current_on_forward_cancelled: nothing(),
            current_on_forward_completed: nothing(),
            current_on_back_cancelled: nothing(),
            current_on_back_completed: nothing(),
        });
        let handler = NavigationEventHandler::with_forward(s.current_info(), false, false, callbacks.clone());
        (callbacks, handler)
    });
    let (h, s) = (source_handler.clone(), state.clone());
    SideEffect(move || {
        h.set_is_forward_enabled(is_forward_enabled);
        *callbacks.current_on_forward_cancelled.borrow_mut() = Rc::new(on_forward_cancelled);
        *callbacks.current_on_forward_completed.borrow_mut() = Rc::new(on_forward_completed);
        h.set_is_back_enabled(is_back_enabled);
        *callbacks.current_on_back_cancelled.borrow_mut() = Rc::new(on_back_cancelled);
        *callbacks.current_on_back_completed.borrow_mut() = Rc::new(on_back_completed);
        h.set_info(s.inner.current_info.peek(), s.inner.back_info.peek(), s.inner.forward_info.peek());
    });
    DisposableEffect(state.clone(), move || {
        assert!(
            state.inner.source_handler.borrow().is_none(),
            "NavigationEventState '{state:?}' is already registered with a NavigationEventHandler '{source_handler:?}'."
        );
        *state.inner.source_handler.borrow_mut() = Some(Rc::new(source_handler.clone()));
        dispatcher.add_handler(&source_handler);
        on_dispose(move || {
            source_handler.remove();
            *state.inner.source_handler.borrow_mut() = None;
        })
    });
}

/// Handles back gestures only, driven by a hoisted `state` (Compose's `NavigationBackHandler`): a
/// [`NavigationEventHandler`](NavigationEventHandler()) with forward disabled.
///
/// ```no_run
/// # use kompoz_navigationevent::*;
/// # use kompoz_runtime::*;
/// #[kompozable]
/// fn Closable(open: MutableState<bool>) {
///     let state = remember_navigation_event_state(NavigationEventInfoNone);
///     let o = open.clone();
///     NavigationBackHandler(state, is_back_enabled = open.get(), on_back_completed = move || o.set(false));
/// }
/// ```
#[kompozable]
pub fn NavigationBackHandler<T: NavigationEventInfo + Clone + PartialEq>(
    state: NavigationEventState<T>,
    #[default(true)] is_back_enabled: bool,
    #[default(|| {})] on_back_cancelled: impl Fn() + 'static,
    on_back_completed: impl Fn() + 'static,
) {
    NavigationEventHandler(
        state = state,
        on_forward_cancelled = || {},
        on_forward_completed = || {},
        is_forward_enabled = false,
        on_back_cancelled = on_back_cancelled,
        on_back_completed = on_back_completed,
        is_back_enabled = is_back_enabled,
    );
}

/// Handles forward gestures only, driven by a hoisted `state` (Compose's `NavigationForwardHandler`): a
/// [`NavigationEventHandler`](NavigationEventHandler()) with back disabled.
#[kompozable]
pub fn NavigationForwardHandler<T: NavigationEventInfo + Clone + PartialEq>(
    state: NavigationEventState<T>,
    #[default(true)] is_forward_enabled: bool,
    #[default(|| {})] on_forward_cancelled: impl Fn() + 'static,
    on_forward_completed: impl Fn() + 'static,
) {
    NavigationEventHandler(
        state = state,
        on_forward_cancelled = on_forward_cancelled,
        on_forward_completed = on_forward_completed,
        is_forward_enabled = is_forward_enabled,
        on_back_cancelled = || {},
        on_back_completed = || {},
        is_back_enabled = false,
    );
}
