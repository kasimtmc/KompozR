//! Windows of their own on the desktop and the application they belong to, as Compose Desktop's (CMP ui-desktop's
//! Window.desktop.kt, WindowState, WindowPosition, WindowPlacement, Application.desktop.kt; C++'s window.hpp):
//! [`Window`] is composed while the window should be open, its content composed in a composition of its own (the
//! platform's app core gives it a host of its own, a GPU device and a platform window) that shares the app's state
//! and sees the composition locals where `Window` is called (the theme, the app's own), the window's own locals
//! (its density, its focus, its lifecycle) over them; it closes as it leaves the composition.
//!
//! ```text
//! fn main() -> ExitCode {
//!     application(|app: &ApplicationScope| {
//!         let app = app.clone();
//!         Window(on_close_request = move || app.exit_application(), title = "KompozR".to_string(),
//!                content = |_: &FrameWindowScope| App());
//!     })
//! }
//! ```
//!
//! What a window asks for ([`WindowRequest`]) is registered with the host it is composed in ([`WindowRegistry`]):
//! the platform's app core opens, keeps up to date and closes the windows of every host it runs. With
//! `App::run` the app's own window is the main one (closing it ends the app, as C++'s); with `application { }`
//! the app's composition has no window of its own (Compose Desktop's), and ends with
//! [`ApplicationScope::exit_application`] or when nothing is left in it (no window, no tray icon).
//!
//! On Android and the web, which give an app one window, a `Window` isn't shown (the platforms report one window).

use super::tray::TrayRequest;
use crate::alignment::Alignment;
use crate::draw::Painter;
use crate::input::KeyEvent;
use kompoz_graphics::{Dp, DpSize};
use kompoz_runtime::{
    DisposableEffect, KompozitionLocal, KompozitionLocalContext, MutableState, SavedValue, Saver, SideEffect,
    current_kompozition_local_context, kompozable, mutable_confined_state_of, mutable_confined_state_of_with_policy,
    mutable_state_of, never_equal_policy, on_dispose, remember, remember_saveable_with,
};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// A window of the app's, as the platform's app core knows it: the main one's ([`WindowId::MAIN`]), a [`Window`]'s
/// (its request's, for the window's life), or the app's composition without a window ([`WindowId::APPLICATION`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
pub struct WindowId(pub u64);

impl WindowId {
    /// The app's own window (`App::run`'s).
    pub const MAIN: WindowId = WindowId(0);
    /// The app's composition, which has no window (`application { }`'s).
    pub const APPLICATION: WindowId = WindowId(u64::MAX);

    /// A new id for a window of its own (never 0 nor the application's).
    pub fn next() -> WindowId {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        WindowId(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// How a window is placed on the screen (Compose Desktop's WindowPlacement).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum WindowPlacement {
    /// Where its position and size put it.
    #[default]
    Floating,
    /// As big as the screen leaves room for (its size and position are the system's while it is).
    Maximized,
    /// All of the screen, without decorations.
    Fullscreen,
}

/// Where a window is on the screen (Compose Desktop's WindowPosition): where the platform puts it, aligned in the
/// screen, or at a point (dp from the screen's top left).
#[derive(Clone, PartialEq, Debug, Default)]
pub enum WindowPosition {
    /// The platform's (Compose's `WindowPosition.PlatformDefault`).
    #[default]
    PlatformDefault,
    /// Aligned in the screen's area (Compose's `WindowPosition.Aligned`).
    Aligned(Alignment),
    /// At `x`, `y` dp from the screen's top left (Compose's `WindowPosition.Absolute`).
    Absolute { x: Dp, y: Dp },
}

impl WindowPosition {
    /// Its x (unspecified unless absolute).
    pub fn x(&self) -> Dp {
        match self {
            WindowPosition::Absolute { x, .. } => *x,
            _ => Dp::Unspecified,
        }
    }
    /// Its y (unspecified unless absolute).
    pub fn y(&self) -> Dp {
        match self {
            WindowPosition::Absolute { y, .. } => *y,
            _ => Dp::Unspecified,
        }
    }
    /// Whether it is a point (absolute).
    pub fn is_specified(&self) -> bool {
        matches!(self, WindowPosition::Absolute { .. })
    }
}

/// Compose Desktop's `WindowPosition(x, y)` and `WindowPosition(alignment)`: its overloads by their arguments.
#[allow(non_snake_case)]
pub fn WindowPosition(args: impl WindowPositionArgs) -> WindowPosition {
    args.position()
}

/// What [`WindowPosition()`](fn@WindowPosition) is given, each overload by its types.
pub trait WindowPositionArgs {
    fn position(self) -> WindowPosition;
}
/// Kotlin: `WindowPosition(x: Dp, y: Dp)`
impl WindowPositionArgs for (Dp, Dp) {
    fn position(self) -> WindowPosition {
        WindowPosition::Absolute { x: self.0, y: self.1 }
    }
}
/// Kotlin: `WindowPosition(alignment: Alignment)`
impl WindowPositionArgs for Alignment {
    fn position(self) -> WindowPosition {
        WindowPosition::Aligned(self)
    }
}

/// A window's state (Compose Desktop's WindowState), observable and set from code (the window follows at its next
/// frame) and by the user (the window's changes read back into it): how it is placed, whether it is minimized,
/// where it is, its size (dp). C++'s `toFront` too ([`to_front`](WindowState::to_front)).
#[derive(Clone)]
pub struct WindowState {
    placement: MutableState<WindowPlacement>,
    is_minimized: MutableState<bool>,
    position: MutableState<WindowPosition>,
    size: MutableState<DpSize>,
    /// `to_front`s asked so far (the app core does each once).
    front_requests: MutableState<u64>,
}

impl PartialEq for WindowState {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size
    }
}

impl fmt::Debug for WindowState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("WindowState")
            .field("placement", &self.placement.peek())
            .field("is_minimized", &self.is_minimized.peek())
            .field("position", &self.position.peek())
            .field("size", &self.size.peek())
            .finish()
    }
}

impl Default for WindowState {
    fn default() -> Self {
        WindowState::new(
            WindowPlacement::Floating,
            false,
            WindowPosition::PlatformDefault,
            DpSize(Dp(800.0), Dp(600.0)),
        )
    }
}

impl WindowState {
    pub fn new(placement: WindowPlacement, is_minimized: bool, position: WindowPosition, size: DpSize) -> WindowState {
        WindowState {
            placement: mutable_state_of(placement),
            is_minimized: mutable_state_of(is_minimized),
            position: mutable_confined_state_of(position),
            size: mutable_state_of(size),
            front_requests: mutable_state_of(0),
        }
    }

    pub fn placement(&self) -> WindowPlacement {
        self.placement.get()
    }
    pub fn set_placement(&self, placement: WindowPlacement) {
        self.placement.set(placement);
    }
    pub fn is_minimized(&self) -> bool {
        self.is_minimized.get()
    }
    pub fn set_is_minimized(&self, minimized: bool) {
        self.is_minimized.set(minimized);
    }
    pub fn position(&self) -> WindowPosition {
        self.position.get()
    }
    pub fn set_position(&self, position: WindowPosition) {
        self.position.set(position);
    }
    pub fn size(&self) -> DpSize {
        self.size.get()
    }
    pub fn set_size(&self, size: DpSize) {
        self.size.set(size);
    }

    /// The window to the front, restored if it is minimized, the keyboard's focus given to it (C++'s
    /// `WindowState::toFront`; Compose Desktop's `window.toFront()`, [`ComposeWindow::to_front`]). Wayland lets a
    /// window come forward only with an activation token (xdg-activation): the compositor may only mark it as
    /// wanting attention.
    pub fn to_front(&self) {
        self.is_minimized.set(false);
        self.front_requests.set(self.front_requests.peek() + 1);
    }

    /// The `to_front`s asked so far, read without observing (the platform's app core does each once).
    pub fn to_front_requests(&self) -> u64 {
        self.front_requests.peek()
    }

    /// What it holds, read without observing (the platform's app core's view of it).
    pub fn peek(&self) -> (WindowPlacement, bool, WindowPosition, DpSize) {
        (self.placement.peek(), self.is_minimized.peek(), self.position.peek(), self.size.peek())
    }

    /// Compose's `WindowStateImpl.Saver(unspecifiedPosition)`: its placement, whether it is minimized, its position
    /// (an unspecified one restored as `unspecified_position`), its size.
    pub fn saver(unspecified_position: WindowPosition) -> Saver<WindowState> {
        Saver::new(
            |s: &WindowState| {
                let (placement, minimized, position, size) = s.peek();
                let size_or_zero = if size.is_specified() { size } else { DpSize::Zero };
                SavedValue::list([
                    SavedValue::Int(placement as i64),
                    SavedValue::Bool(minimized),
                    SavedValue::Bool(position.is_specified()),
                    SavedValue::Double(position.x().0 as f64),
                    SavedValue::Double(position.y().0 as f64),
                    SavedValue::Double(size_or_zero.width.0 as f64),
                    SavedValue::Double(size_or_zero.height.0 as f64),
                    SavedValue::Bool(size.is_specified()),
                ])
            },
            move |saved: &SavedValue| {
                let l = saved.as_list()?;
                let placement = match l.first()?.as_int()? {
                    1 => WindowPlacement::Maximized,
                    2 => WindowPlacement::Fullscreen,
                    _ => WindowPlacement::Floating,
                };
                let float = |i: usize| l.get(i).and_then(SavedValue::as_f64).map(|v| Dp(v as f32));
                let position = if l.get(2)?.as_bool()? {
                    WindowPosition::Absolute { x: float(3)?, y: float(4)? }
                } else {
                    unspecified_position.clone()
                };
                let size = if l.get(7).and_then(SavedValue::as_bool) != Some(false) {
                    DpSize(float(5)?, float(6)?)
                } else {
                    DpSize::Unspecified
                };
                Some(WindowState::new(placement, l.get(1)?.as_bool()?, position, size))
            },
        )
    }
}

/// A window's state (Compose Desktop's `WindowState(placement, isMinimized, position, size)`), not remembered.
#[kompoz_runtime::named(type)]
pub fn WindowState(
    #[default(WindowPlacement::Floating)] placement: WindowPlacement,
    #[default(false)] is_minimized: bool,
    #[default(WindowPosition::PlatformDefault)] position: WindowPosition,
    #[default(DpSize(Dp(800.0), Dp(600.0)))] size: DpSize,
) -> WindowState {
    WindowState::new(placement, is_minimized, position, size)
}

/// A window's state remembered, and saved with the app's state (Compose Desktop's `rememberWindowState`).
#[kompozable]
pub fn remember_window_state(
    #[default(WindowPlacement::Floating)] placement: WindowPlacement,
    #[default(false)] is_minimized: bool,
    #[default(WindowPosition::PlatformDefault)] position: WindowPosition,
    #[default(DpSize(Dp(800.0), Dp(600.0)))] size: DpSize,
) -> WindowState {
    let saver = WindowState::saver(position.clone());
    remember_saveable_with(&saver, move || WindowState::new(placement, is_minimized, position, size))
}

/// What a window looks like and does, as its `Window` says (all but its state and content).
#[derive(Clone)]
pub struct WindowProperties {
    pub title: String,
    pub visible: bool,
    pub icon: Option<Rc<dyn Painter>>,
    pub undecorated: bool,
    pub transparent: bool,
    pub resizable: bool,
    pub enabled: bool,
    pub focusable: bool,
    pub always_on_top: bool,
}

impl PartialEq for WindowProperties {
    fn eq(&self, o: &Self) -> bool {
        let icons = match (&self.icon, &o.icon) {
            (None, None) => true,
            (Some(a), Some(b)) => a.painter_eq(&**b),
            _ => false,
        };
        icons
            && self.title == o.title
            && self.visible == o.visible
            && self.undecorated == o.undecorated
            && self.transparent == o.transparent
            && self.resizable == o.resizable
            && self.enabled == o.enabled
            && self.focusable == o.focusable
            && self.always_on_top == o.always_on_top
    }
}

impl fmt::Debug for WindowProperties {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("WindowProperties")
            .field("title", &self.title)
            .field("visible", &self.visible)
            .field("icon", &self.icon.is_some())
            .field("undecorated", &self.undecorated)
            .field("transparent", &self.transparent)
            .field("resizable", &self.resizable)
            .field("enabled", &self.enabled)
            .field("focusable", &self.focusable)
            .field("always_on_top", &self.always_on_top)
            .finish()
    }
}

impl Default for WindowProperties {
    fn default() -> Self {
        WindowProperties {
            title: "Untitled".into(),
            visible: true,
            icon: None,
            undecorated: false,
            transparent: false,
            resizable: true,
            enabled: true,
            focusable: true,
            always_on_top: false,
        }
    }
}

type KeyHandler = Rc<dyn Fn(&KeyEvent) -> bool>;
/// A window's content, given its scope.
pub type WindowContent = Rc<dyn Fn(&FrameWindowScope)>;

/// A window the composition asks for (C++'s WindowRequest): what the platform's app core opens, keeps up to date
/// and closes. Its content is composed in a host of its own ([`KompozHost::new_window`](crate::KompozHost)), with the
/// caller's composition locals provided ([`locals`](WindowRequest::locals)), recomposed as its caller is.
pub struct WindowRequest {
    id: WindowId,
    properties: RefCell<WindowProperties>,
    state: RefCell<WindowState>,
    on_close_request: RefCell<Rc<dyn Fn()>>,
    on_preview_key_event: RefCell<KeyHandler>,
    on_key_event: RefCell<KeyHandler>,
    /// Its content, set as its caller recomposes (never equal: the window's composition composes it again).
    content: MutableState<WindowContent>,
    /// The composition locals where `Window` is called.
    locals: MutableState<KompozitionLocalContext>,
    /// Bumped as its properties change (the platform's window follows).
    version: Cell<u64>,
}

impl WindowRequest {
    /// A request with `properties` and `state`, its content `content`, the locals `locals` (a test's, or a
    /// platform's own window made from code).
    pub fn new(
        properties: WindowProperties,
        state: WindowState,
        on_close_request: Rc<dyn Fn()>,
        content: WindowContent,
        locals: KompozitionLocalContext,
    ) -> WindowRequest {
        let no_key: KeyHandler = Rc::new(|_: &KeyEvent| false);
        WindowRequest {
            id: WindowId::next(),
            properties: RefCell::new(properties),
            state: RefCell::new(state),
            on_close_request: RefCell::new(on_close_request),
            on_preview_key_event: RefCell::new(no_key.clone()),
            on_key_event: RefCell::new(no_key),
            content: mutable_confined_state_of_with_policy(content, never_equal_policy()),
            locals: mutable_confined_state_of(locals),
            version: Cell::new(0),
        }
    }

    pub fn id(&self) -> WindowId {
        self.id
    }
    /// What it looks like and does now.
    pub fn properties(&self) -> WindowProperties {
        self.properties.borrow().clone()
    }
    /// Its title now.
    pub fn title(&self) -> String {
        self.properties.borrow().title.clone()
    }
    pub fn visible(&self) -> bool {
        self.properties.borrow().visible
    }
    /// Its state (the one its `Window` was given last).
    pub fn state(&self) -> WindowState {
        self.state.borrow().clone()
    }
    /// The user closing it (its title bar's ×, Alt+F4): its `on_close_request`.
    pub fn close_requested(&self) {
        let f = self.on_close_request.borrow().clone();
        f();
    }
    /// Its key handlers (Compose Desktop's window's `onPreviewKeyEvent`, `onKeyEvent`): before the focused path
    /// sees a key and after it left it.
    pub fn on_preview_key_event(&self, event: &KeyEvent) -> bool {
        let f = self.on_preview_key_event.borrow().clone();
        f(event)
    }
    pub fn on_key_event(&self, event: &KeyEvent) -> bool {
        let f = self.on_key_event.borrow().clone();
        f(event)
    }
    /// Its content, read (by the window's composition: it composes again as the caller does).
    pub fn content(&self) -> WindowContent {
        self.content.get()
    }
    /// The locals where it is composed, read (by the window's host: it provides them again as they change).
    pub fn locals(&self) -> MutableState<KompozitionLocalContext> {
        self.locals.clone()
    }
    /// Bumped as its properties change.
    pub fn version(&self) -> u64 {
        self.version.get()
    }
}

impl fmt::Debug for WindowRequest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("WindowRequest").field("id", &self.id).field("properties", &self.properties.borrow()).finish()
    }
}

/// The windows and tray icons a host's composition asks for, in the order they came (the platform's app core reads
/// them after each frame), and what wakes the platform's loop for them.
#[derive(Clone, Default)]
pub struct WindowRegistry(Rc<RegistryState>);

#[derive(Default)]
struct RegistryState {
    windows: RefCell<Vec<Rc<WindowRequest>>>,
    trays: RefCell<Vec<Rc<TrayRequest>>>,
    /// Bumped as a request comes, goes or changes.
    version: Cell<u64>,
    wake: RefCell<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl PartialEq for WindowRegistry {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl WindowRegistry {
    /// The windows asked for, in the order they came.
    pub fn windows(&self) -> Vec<Rc<WindowRequest>> {
        self.0.windows.borrow().clone()
    }
    /// The tray icons asked for.
    pub fn trays(&self) -> Vec<Rc<TrayRequest>> {
        self.0.trays.borrow().clone()
    }
    /// Bumped as a request comes, goes or changes.
    pub fn version(&self) -> u64 {
        self.0.version.get()
    }
    /// What asks the platform for a frame (the host's waker).
    pub fn set_wake(&self, wake: Option<Arc<dyn Fn() + Send + Sync>>) {
        *self.0.wake.borrow_mut() = wake;
    }
    /// Something changed: the platform's loop woken for its next frame (where the app core syncs).
    pub fn changed(&self) {
        self.0.version.set(self.0.version.get() + 1);
        let wake = self.0.wake.borrow().clone();
        if let Some(w) = wake {
            w();
        }
    }
    pub fn add_window(&self, request: Rc<WindowRequest>) {
        self.0.windows.borrow_mut().push(request);
        self.changed();
    }
    pub fn remove_window(&self, id: WindowId) {
        self.0.windows.borrow_mut().retain(|r| r.id != id);
        self.changed();
    }
    pub(crate) fn add_tray(&self, request: Rc<TrayRequest>) {
        self.0.trays.borrow_mut().push(request);
        self.changed();
    }
    pub(crate) fn remove_tray(&self, id: u64) {
        self.0.trays.borrow_mut().retain(|r| r.id() != id);
        self.changed();
    }
}

/// The host's registry, provided at its root (each window's host its own: a `Window` composed in a window is that
/// window's).
pub static LocalWindowRegistry: KompozitionLocal<Option<WindowRegistry>> = KompozitionLocal::new_static(|| None);

/// What a window's content is given (Compose Desktop's FrameWindowScope): its window.
#[derive(Clone)]
pub struct FrameWindowScope {
    window: ComposeWindow,
}

impl FrameWindowScope {
    pub fn new(window: ComposeWindow) -> FrameWindowScope {
        FrameWindowScope { window }
    }
    /// The window (Compose Desktop's `window`, an AWT ComposeWindow there).
    pub fn window(&self) -> ComposeWindow {
        self.window.clone()
    }
}

/// Compose Desktop's WindowScope (`FrameWindowScope`'s supertype): the same scope.
pub type WindowScope = FrameWindowScope;

/// A window of the app's as its content reaches it (Compose Desktop's ComposeWindow, as far as a platform window is
/// the app's to move): its title, bringing it to the front.
#[derive(Clone)]
pub struct ComposeWindow {
    request: Weak<WindowRequest>,
}

impl ComposeWindow {
    pub fn new(request: &Rc<WindowRequest>) -> ComposeWindow {
        ComposeWindow { request: Rc::downgrade(request) }
    }
    /// Its title.
    pub fn title(&self) -> String {
        self.request.upgrade().map(|r| r.title()).unwrap_or_default()
    }
    /// Its id (the platform's app core's).
    pub fn id(&self) -> Option<WindowId> {
        self.request.upgrade().map(|r| r.id)
    }
    /// The window to the front, restored and given the focus (AWT's `toFront`): [`WindowState::to_front`].
    pub fn to_front(&self) {
        if let Some(r) = self.request.upgrade() {
            r.state().to_front();
        }
    }
}

/// A window of the app's own on the desktop (Compose Desktop's Window): open while it is composed (and `visible`),
/// its `content` composed in a composition of its own that sees the locals where it is called. `on_close_request`
/// runs as the user closes it (the window stays open until the app takes it out of the composition, Compose's).
/// `state` is followed both ways: what the app sets the window does at its next frame, what the user does is read
/// back. `visible = false` hides the window, its composition and state kept (its GPU device and platform window let
/// go meanwhile). `enabled = false` takes no input; `focusable = false` asks the window manager not to give it the
/// keyboard (X11); `always_on_top` keeps it above the others (X11; Wayland has no protocol for it); `resizable`
/// lets the user resize it; `undecorated` leaves the frame out (KompozR's own title bar isn't drawn either);
/// `transparent` isn't supported yet (the window is opaque: see the follow-ups); `icon` is the window's (X11's
/// `_NET_WM_ICON`; Wayland's xdg-toplevel-icon isn't bound yet). `on_preview_key_event` and `on_key_event` see the
/// window's keys before and after its focused path.
#[kompozable]
pub fn Window(
    on_close_request: impl Fn() + 'static,
    #[default(remember_window_state())] state: WindowState,
    #[default(true)] visible: bool,
    #[default("Untitled".to_string())] title: String,
    #[default(None)] icon: Option<Rc<dyn Painter>>,
    #[default(false)] undecorated: bool,
    #[default(false)] transparent: bool,
    #[default(true)] resizable: bool,
    #[default(true)] enabled: bool,
    #[default(true)] focusable: bool,
    #[default(false)] always_on_top: bool,
    #[default(|_: &KeyEvent| false)] on_preview_key_event: impl Fn(&KeyEvent) -> bool + 'static,
    #[default(|_: &KeyEvent| false)] on_key_event: impl Fn(&KeyEvent) -> bool + 'static,
    content: impl Fn(&FrameWindowScope) + 'static,
) {
    let Some(registry) = LocalWindowRegistry.current() else { return };
    let properties = WindowProperties {
        title,
        visible,
        icon,
        undecorated,
        transparent,
        resizable,
        enabled,
        focusable,
        always_on_top,
    };
    let content: WindowContent = Rc::new(content);
    let locals = current_kompozition_local_context();
    let request = remember({
        let (properties, state, content, locals) = (properties.clone(), state.clone(), content.clone(), locals.clone());
        move || Rc::new(WindowRequest::new(properties, state, Rc::new(|| {}), content, locals))
    });
    // Kept up to date as the caller recomposes: the latest callbacks always, the content and the locals for the
    // window's composition (after this one is applied), the version bumped when what the platform shows changed.
    *request.on_close_request.borrow_mut() = Rc::new(on_close_request);
    *request.on_preview_key_event.borrow_mut() = Rc::new(on_preview_key_event);
    *request.on_key_event.borrow_mut() = Rc::new(on_key_event);
    let changed = *request.properties.borrow() != properties || *request.state.borrow() != state;
    *request.properties.borrow_mut() = properties;
    *request.state.borrow_mut() = state;
    {
        let (request, registry) = (request.clone(), registry.clone());
        SideEffect(move || {
            request.content.set(content);
            if request.locals.peek() != locals {
                request.locals.set(locals);
            }
            if changed {
                request.version.set(request.version.get() + 1);
                registry.changed();
            }
        });
    }
    let id = request.id;
    DisposableEffect((), move || {
        registry.add_window(request);
        on_dispose(move || registry.remove_window(id))
    });
}

/// What the app's composition is given in `application { }` (Compose Desktop's ApplicationScope): ending the app.
#[derive(Clone)]
pub struct ApplicationScope {
    exit: Rc<dyn Fn()>,
}

impl ApplicationScope {
    /// A scope whose `exit_application` runs `exit` (the platform's app core's).
    pub fn new(exit: Rc<dyn Fn()>) -> ApplicationScope {
        ApplicationScope { exit }
    }
    /// Closes all the app's windows and ends it (Compose Desktop's `exitApplication`): its effects stopped, its
    /// loop done after this frame.
    pub fn exit_application(&self) {
        (self.exit)();
    }
}

impl fmt::Debug for ApplicationScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("ApplicationScope")
    }
}

/// What the platform's app core does for the calls below (set while it runs, on its thread).
pub trait ApplicationControl {
    /// The app ends after this frame.
    fn exit(&self);
    /// Whether this platform shows tray icons.
    fn tray_supported(&self) -> bool;
}

thread_local! {
    static CONTROL: RefCell<Option<Rc<dyn ApplicationControl>>> = const { RefCell::new(None) };
}

/// The platform's app core's control of the app (none: there is no app running on this thread).
pub fn set_application_control(control: Option<Rc<dyn ApplicationControl>>) {
    CONTROL.with(|c| *c.borrow_mut() = control);
}

fn control() -> Option<Rc<dyn ApplicationControl>> {
    CONTROL.try_with(|c| c.borrow().clone()).ok().flatten()
}

/// The app ends (C++'s `exitApplication()`; Compose Desktop's is [`ApplicationScope::exit_application`], which this
/// is from anywhere on the app's thread: a tray menu's item in an `App::run` app). Nothing where no app runs.
pub fn exit_application() {
    if let Some(c) = control() {
        c.exit();
    }
}

/// Whether this platform shows tray icons (Compose Desktop's `isTraySupported`): Linux with a StatusNotifierItem
/// host (KDE; GNOME with the AppIndicator extension); none elsewhere for now (Windows and macOS: their packages).
pub fn is_tray_supported() -> bool {
    control().is_some_and(|c| c.tray_supported())
}
