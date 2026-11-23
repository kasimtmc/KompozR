//! The windows' navigation events: the host's root owner (Compose for desktop's window owner,
//! `DefaultArchitectureComponentsOwner`'s navigation part), the keys' input
//! (navigationevent/BackNavigationEventInput.kt), the platform's back input (navigationevent's Android
//! `OnBackInvokedInput`: a platform's back gestures and back button), and a focusable popup's or a dialog's own
//! (Android's: a dialog is a window of its own, a `ComponentDialog` with its own dispatcher, its back callback added
//! first — its dismissal —, its content's handlers before it; what C++ Kompoz's back scopes were).
//!
//! Each window — the host's, and each focusable popup or dialog on it — has a root [`NavigationEventDispatcher`]
//! (provided to its content as `LocalNavigationEventDispatcherOwner`) and two inputs: the keys (Escape, and the Back
//! key — a mouse's back button makes one, as Android's input reader does) and the platform's back (Android's back
//! callbacks: a gesture's start, steps, cancellation and invocation, or a back button's invocation). The host sends
//! back to the window on top (a gesture: to the one it started in). An input takes back only while its dispatcher has
//! enabled handlers (as Android registers its callback only then): otherwise the key goes on, and the platform does
//! its own back (leaving the app).

use crate::back::BackEventCompat;
use crate::input::{Key, KeyEvent, KeyEventType};
use kompoz_navigationevent::{
    NavigationEvent, NavigationEventDispatcher, NavigationEventDispatcherOwner, NavigationEventHandler,
    NavigationEventHandlerCallbacks, NavigationEventInfo, NavigationEventInput, NavigationEventInputCallbacks,
};
use kompoz_runtime::{DisposableEffect, current_composite_key_hash, kompozable, on_dispose, remember, remember_keyed};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// A window's root owner (CMP's `DefaultArchitectureComponentsOwner`'s navigation part; a dialog's, Android's
/// `ComponentDialog`'s): the root dispatcher, no fallback (the inputs say whether they took back).
pub(crate) struct WindowNavigationEventDispatcherOwner {
    pub(crate) dispatcher: NavigationEventDispatcher,
}

impl NavigationEventDispatcherOwner for WindowNavigationEventDispatcherOwner {
    fn navigation_event_dispatcher(&self) -> NavigationEventDispatcher {
        self.dispatcher.clone()
    }
}

/// Whether an input is added to a dispatcher, and whether that has enabled handlers (what it hears; the platform's
/// back is taken only then).
#[derive(Default)]
struct Registered {
    added: Cell<bool>,
    has_enabled_handlers: Cell<bool>,
}

/// An input's callbacks: what it hears kept in its [`Registered`].
struct Hears(Rc<Registered>);

impl NavigationEventInputCallbacks for Hears {
    fn on_added(&self, _: &NavigationEventInput, _: &NavigationEventDispatcher) {
        self.0.added.set(true);
    }
    fn on_removed(&self, _: &NavigationEventInput) {
        self.0.added.set(false);
        self.0.has_enabled_handlers.set(false);
    }
    fn on_has_enabled_handlers_changed(&self, _: &NavigationEventInput, has_enabled_handlers: bool) {
        self.0.has_enabled_handlers.set(has_enabled_handlers);
    }
}

/// The keys' input (CMP's `BackNavigationEventInput`): Escape (and the Back key) pressed, back completed — while the
/// dispatcher has enabled handlers (CMP's takes Escape always; here a key nothing takes goes on, as the C++'s root
/// key default).
pub(crate) struct BackNavigationEventInput {
    pub(crate) input: NavigationEventInput,
    registered: Rc<Registered>,
}

impl BackNavigationEventInput {
    fn new() -> BackNavigationEventInput {
        let registered = Rc::new(Registered::default());
        BackNavigationEventInput { input: NavigationEventInput::new(Hears(registered.clone())), registered }
    }

    /// A key event no node took: whether it went back.
    pub(crate) fn on_key_event(&self, event: &KeyEvent) -> bool {
        if event.r#type == KeyEventType::KeyDown
            && (event.key == Key::Escape || event.key == Key::Back)
            && self.registered.has_enabled_handlers.get()
        {
            self.input.dispatch_on_back_completed();
            return true;
        }
        false
    }
}

/// The platform's back (navigationevent's Android `OnBackInvokedInput`: the window's back callback, registered with
/// the platform while the dispatcher has enabled handlers): a gesture's start, steps, cancellation and invocation,
/// or a plain invocation (a back button). Unregistered during a gesture (its handler gone, nothing left to take
/// back), the gesture is called off and its end goes to the platform.
pub(crate) struct PlatformBackNavigationEventInput {
    pub(crate) input: NavigationEventInput,
    registered: Rc<Registered>,
    /// A gesture of its own under way (started, not ended).
    in_gesture: Cell<bool>,
}

impl PlatformBackNavigationEventInput {
    fn new() -> PlatformBackNavigationEventInput {
        let registered = Rc::new(Registered::default());
        PlatformBackNavigationEventInput {
            input: NavigationEventInput::new(Hears(registered.clone())),
            registered,
            in_gesture: Cell::new(false),
        }
    }

    /// Whether the platform's back would come here (its callback registered: an enabled handler).
    pub(crate) fn is_registered(&self) -> bool {
        self.registered.has_enabled_handlers.get()
    }

    /// Whether a gesture of its own is under way.
    pub(crate) fn in_gesture(&self) -> bool {
        self.drop_gesture_if_unregistered();
        self.in_gesture.get()
    }

    /// The gesture under way ends without the callback: called off (the processor let it go when its handler
    /// went; its window gone, there is nothing to tell).
    fn drop_gesture_if_unregistered(&self) {
        if self.in_gesture.get() && !self.is_registered() {
            self.in_gesture.set(false);
            if self.registered.added.get() {
                self.input.dispatch_on_back_cancelled();
            }
        }
    }

    pub(crate) fn back_started(&self, event: NavigationEvent) {
        self.drop_gesture_if_unregistered();
        if !self.is_registered() {
            return;
        }
        if self.in_gesture.get() {
            // (a gesture never ended: called off first, as the platform would before a new one)
            self.input.dispatch_on_back_cancelled();
        }
        self.in_gesture.set(true);
        self.input.dispatch_on_back_started(event);
    }

    pub(crate) fn back_progressed(&self, event: NavigationEvent) {
        self.drop_gesture_if_unregistered();
        if self.in_gesture.get() {
            self.input.dispatch_on_back_progressed(event);
        }
    }

    pub(crate) fn back_cancelled(&self) {
        self.drop_gesture_if_unregistered();
        if self.in_gesture.get() {
            self.in_gesture.set(false);
            self.input.dispatch_on_back_cancelled();
        }
    }

    /// Back invoked (a gesture's end, or a press): whether the app took it (false: the platform's own back).
    pub(crate) fn back_invoked(&self) -> bool {
        self.drop_gesture_if_unregistered();
        if !self.is_registered() {
            return false;
        }
        self.in_gesture.set(false);
        self.input.dispatch_on_back_completed();
        true
    }
}

/// A window's navigation: its root owner and its two inputs (added to its dispatcher). The host's lives as long as
/// it; a focusable popup's or a dialog's while it is composed (disposed as it leaves: its handlers and inputs go).
pub(crate) struct WindowNavigation {
    pub(crate) owner: Rc<WindowNavigationEventDispatcherOwner>,
    pub(crate) keys: BackNavigationEventInput,
    pub(crate) platform: PlatformBackNavigationEventInput,
}

impl WindowNavigation {
    pub(crate) fn new() -> Rc<WindowNavigation> {
        let navigation = WindowNavigation {
            owner: Rc::new(WindowNavigationEventDispatcherOwner { dispatcher: NavigationEventDispatcher::new() }),
            keys: BackNavigationEventInput::new(),
            platform: PlatformBackNavigationEventInput::new(),
        };
        // (CMP's scene mediator adds its key input to the window's dispatcher as the window is attached)
        navigation.owner.dispatcher.add_input(&navigation.keys.input);
        navigation.owner.dispatcher.add_input(&navigation.platform.input);
        Rc::new(navigation)
    }

    /// Its owner, as the locals give it.
    pub(crate) fn owner(&self) -> Rc<dyn NavigationEventDispatcherOwner> {
        self.owner.clone()
    }

    /// Gone with its window: the dispatcher disposed (its inputs removed, its handlers too: a gesture going to one
    /// is called off).
    pub(crate) fn dispose(&self) {
        self.owner.dispatcher.dispose();
        self.platform.in_gesture.set(false);
    }
}

/// A focusable popup's or a dialog's window navigation, remembered while it is composed (disposed as it leaves).
#[kompozable(inline)]
pub(crate) fn remember_window_navigation() -> Rc<WindowNavigation> {
    let navigation = remember(WindowNavigation::new);
    let n = navigation.clone();
    DisposableEffect(SameWindow(navigation.clone()), move || on_dispose(move || n.dispose()));
    navigation
}

/// A window's navigation compared by identity.
struct SameWindow(Rc<WindowNavigation>);

impl PartialEq for SameWindow {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// A focusable popup's or a dialog's back handler's info (the composition's place).
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct WindowBackHandlerInfo {
    composite_key: u64,
}

impl NavigationEventInfo for WindowBackHandlerInfo {}

/// What a window's back handler does (Android's `DialogWrapper` back callback, `PopupLayout`'s back key): back
/// completed — a press, or a gesture's end — dismisses it if `dismiss_on_back_press` says so; taken either way (a
/// dialog that can't be dismissed by back still stops it).
struct WindowBack {
    dismiss_on_back_press: Cell<bool>,
    on_dismiss_request: RefCell<Rc<dyn Fn()>>,
}

impl NavigationEventHandlerCallbacks<WindowBackHandlerInfo> for Rc<WindowBack> {
    fn on_back_completed(&self, _: &NavigationEventHandler<WindowBackHandlerInfo>) {
        if self.dismiss_on_back_press.get() {
            let on_dismiss_request = self.on_dismiss_request.borrow().clone();
            on_dismiss_request();
        }
    }
}

/// A focusable popup's or a dialog's back handler, registered with its own window's dispatcher while composed (its
/// `on_dismiss_request` the latest given; composed before the content: the content's handlers come first).
#[kompozable(inline)]
pub(crate) fn WindowBackHandler(
    navigation: Rc<WindowNavigation>,
    on_dismiss_request: Rc<dyn Fn()>,
    dismiss_on_back_press: bool,
) {
    let composite_key = current_composite_key_hash();
    let (handler, state) = remember_keyed(composite_key, || {
        let state = Rc::new(WindowBack {
            dismiss_on_back_press: Cell::new(dismiss_on_back_press),
            on_dismiss_request: RefCell::new(Rc::new(|| {})),
        });
        let handler = NavigationEventHandler::new(WindowBackHandlerInfo { composite_key }, true, state.clone());
        (handler, state)
    });
    *state.on_dismiss_request.borrow_mut() = on_dismiss_request;
    state.dismiss_on_back_press.set(dismiss_on_back_press);
    let dispatcher = navigation.owner.dispatcher.clone();
    DisposableEffect((dispatcher.clone(), handler.clone()), move || {
        dispatcher.add_handler(&handler);
        on_dispose(move || handler.remove())
    });
}

/// Back as the host sends it to its windows: to the window on top (a gesture to the one it started in, a gesture
/// still under way in another called off first).
#[derive(Default)]
pub(crate) struct BackRouter {
    gesture_window: RefCell<Option<Rc<WindowNavigation>>>,
}

impl BackRouter {
    fn gesture_window(&self) -> Option<Rc<WindowNavigation>> {
        let window = self.gesture_window.borrow().clone();
        window.filter(|w| w.platform.in_gesture())
    }

    pub(crate) fn started(&self, top: &Rc<WindowNavigation>, event: BackEventCompat) {
        if let Some(previous) = self.gesture_window()
            && !Rc::ptr_eq(&previous, top)
        {
            previous.platform.back_cancelled();
        }
        *self.gesture_window.borrow_mut() = Some(top.clone());
        top.platform.back_started(event.to_navigation_event());
    }

    pub(crate) fn progressed(&self, event: BackEventCompat) {
        if let Some(window) = self.gesture_window() {
            window.platform.back_progressed(event.to_navigation_event());
        }
    }

    pub(crate) fn cancelled(&self) {
        if let Some(window) = self.gesture_window.borrow_mut().take() {
            window.platform.back_cancelled();
        }
    }

    pub(crate) fn invoked(&self, top: &Rc<WindowNavigation>) -> bool {
        let window = self.gesture_window.borrow_mut().take();
        match window.filter(|w| w.platform.in_gesture()) {
            Some(window) => window.platform.back_invoked(),
            None => top.platform.back_invoked(),
        }
    }
}
