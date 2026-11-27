//! What the macOS platform's windows share (C++'s MacPlatform's app part): the application and its delegate (the
//! system's quit, the screens changing, the accent color, the accessibility options, the locale and the time zone
//! changing), the app's menu bar (its app menu and Window menu around the key window's own menus), the events of
//! its own, the loop's wake, the timer that steps the app while AppKit tracks the mouse in a loop of its own
//! (resizing a window, a menu open, a drag), the clipboard, the display kept awake.
//!
//! Waking from any thread (the hosts' wakers, a display link's thread): Grand Central Dispatch's main queue runs a
//! function that posts an application-defined event (C++'s `dispatch_async` and `postEvent:`), which ends the loop's
//! wait for the next event — the main queue's work runs inside that wait, as the run loop serves it.

#![allow(unsafe_code)]

use super::clipboard::Clipboard;
use super::keyboard;
use super::services;
use super::window::WindowState;
use super::{Route, StepKind, route, sys};
use crate::app::{Event, TrayMenuItem};
use dispatch2::DispatchQueue;
use kompoz_graphics::Color;
use kompoz_ui::window::{MenuItemKind, WindowId};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSApplicationTerminateReply, NSEvent,
    NSEventMask, NSEventModifierFlags, NSEventTrackingRunLoopMode, NSEventType, NSMenu, NSMenuItem,
    NSSystemColorsDidChangeNotification, NSWorkspace, NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification,
};
use objc2_foundation::{
    NSCurrentLocaleDidChangeNotification, NSDate, NSDefaultRunLoopMode, NSNotification, NSNotificationCenter,
    NSObjectProtocol, NSPoint, NSRunLoop, NSString, NSSystemTimeZoneDidChangeNotification, NSTimer,
};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// How often the app is stepped while AppKit tracks the mouse in a loop of its own (C++'s 120 Hz timer).
const TRACKING_STEP: f64 = 1.0 / 120.0;

// ---- Waking the loop ---------------------------------------------------------------------------------------------

/// An application-defined event is on its way to the queue (one at a time is enough).
static POSTED: AtomicBool = AtomicBool::new(false);
/// A host's waker asked (the loop tells the cores: `Event::Wake`).
static WOKEN: AtomicBool = AtomicBool::new(false);

/// The loop's wait ended, from any thread: an application-defined event posted (from the main queue where it
/// isn't the main thread that asks).
pub fn wake_loop() {
    if !POSTED.swap(true, Ordering::AcqRel) {
        if MainThreadMarker::new().is_some() {
            post_wake_event();
        } else {
            DispatchQueue::main().exec_async(post_wake_event);
        }
    }
}

/// On the main thread: the event that ends `nextEventMatchingMask:`'s wait.
fn post_wake_event() {
    POSTED.store(false, Ordering::Release);
    let Some(mtm) = MainThreadMarker::new() else { return };
    let event = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
        NSEventType::ApplicationDefined,
        NSPoint::new(0.0, 0.0),
        NSEventModifierFlags(0),
        0.0,
        0,
        None,
        0,
        0,
        0,
    );
    if let Some(event) = event {
        NSApplication::sharedApplication(mtm).postEvent_atStart(&event, false);
    }
}

/// The hosts' waker: work for the next frame (any thread).
pub fn host_waker() -> Arc<dyn Fn() + Send + Sync> {
    Arc::new(|| {
        WOKEN.store(true, Ordering::Release);
        wake_loop();
    })
}

// ---- The application's delegate ------------------------------------------------------------------------------------

/// The delegate's state: the platform it tells.
pub struct AppIvars {
    shared: Weak<Shared>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; the delegate implements no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "KompozAppDelegate"]
    #[ivars = AppIvars]
    pub struct AppDelegate;

    impl AppDelegate {
        /// The system's accent color (or another of its colors) changed.
        #[unsafe(method(systemColorsChanged:))]
        fn system_colors_changed(&self, _notification: &NSNotification) {
            if let Some(shared) = self.ivars().shared.upgrade() {
                shared.accent_changed();
            }
        }

        /// The accessibility display options changed (reduced motion among them).
        #[unsafe(method(accessibilityDisplayOptionsChanged:))]
        fn accessibility_display_options_changed(&self, _notification: &NSNotification) {
            if let Some(shared) = self.ivars().shared.upgrade() {
                shared.reduce_motion.set(services::reduce_motion());
                shared.push(Route::All, Event::MetricsChanged);
            }
        }

        /// The user's languages or region, or the system's time zone, changed.
        #[unsafe(method(localeChanged:))]
        fn locale_changed(&self, _notification: &NSNotification) {
            services::publish_locale();
            if let Some(shared) = self.ivars().shared.upgrade() {
                shared.push(Route::All, Event::MetricsChanged);
            }
        }

        /// The tracking loop's timer: the app stepped.
        #[unsafe(method(trackingTick:))]
        fn tracking_tick(&self, _timer: &NSTimer) {
            if let Some(shared) = self.ivars().shared.upgrade() {
                shared.step(StepKind::Frame);
            }
        }

        /// An item of the app's menus clicked (a window's menu bar's).
        #[unsafe(method(menuItemClicked:))]
        fn menu_item_clicked(&self, item: &NSMenuItem) {
            if let Some(shared) = self.ivars().shared.upgrade() {
                shared.menu_clicked(item.tag());
            }
        }
    }

    // SAFETY: NSObject's protocol, nothing of its own.
    unsafe impl NSObjectProtocol for AppDelegate {}

    // SAFETY: the methods' signatures are NSApplicationDelegate's.
    unsafe impl NSApplicationDelegate for AppDelegate {
        /// The system asks the app to quit (its menu's Quit, logging out): the app's own quit (its state written),
        /// AppKit's cancelled (the loop ends it).
        #[unsafe(method(applicationShouldTerminate:))]
        fn application_should_terminate(&self, _sender: &NSApplication) -> NSApplicationTerminateReply {
            if let Some(shared) = self.ivars().shared.upgrade() {
                shared.push(Route::App, Event::Quit);
                wake_loop();
            }
            NSApplicationTerminateReply::TerminateCancel
        }

        /// The screens changed (one came or went, a resolution, an arrangement).
        #[unsafe(method(applicationDidChangeScreenParameters:))]
        fn application_did_change_screen_parameters(&self, _notification: &NSNotification) {
            if let Some(shared) = self.ivars().shared.upgrade() {
                shared.push(Route::All, Event::MetricsChanged);
            }
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker, shared: Weak<Shared>) -> Retained<AppDelegate> {
        let this = AppDelegate::alloc(mtm).set_ivars(AppIvars { shared });
        // SAFETY: NSObject's `init` on a freshly allocated object with its ivars set.
        unsafe { msg_send![super(this), init] }
    }
}

// ---- What the windows share -------------------------------------------------------------------------------------

/// What the platform's windows share: the application, its delegate and menus, the windows' states (by their cores'
/// ids), the platform's own events, the stepper of AppKit's tracking loops, the system's theme, accent and motion,
/// the clipboard, the display kept awake.
pub struct Shared {
    pub mtm: MainThreadMarker,
    pub app: Retained<NSApplication>,
    delegate: Retained<AppDelegate>,
    pub(super) windows: RefCell<Vec<Rc<WindowState>>>,
    routed: RefCell<VecDeque<(Route, Event)>>,
    stepper: RefCell<Option<Rc<dyn Fn(StepKind)>>>,
    tracking_timer: RefCell<Option<Retained<NSTimer>>>,
    /// The root core's id.
    app_id: WindowId,
    /// The system's dark appearance, as last read.
    pub dark: Cell<bool>,
    /// The system's accent color (System Settings > Appearance), as last read.
    pub accent: Cell<Option<Color>>,
    /// Reduce motion (Accessibility > Display), as last read: what the hosts' motion duration scale reads.
    pub reduce_motion: Rc<Cell<bool>>,
    pub clipboard: Clipboard,
    /// A microphone and a camera, once looked for.
    pub probed: Cell<Option<(bool, bool)>>,
    /// The power assertion keeping the display awake (some window's host asks).
    awake: Cell<Option<u32>>,
    /// The menu bar's items of the app's own windows by their tags (the tag less one): whose and where.
    menu_items: RefCell<Vec<(WindowId, Vec<usize>)>>,
    /// The window whose menus the menu bar shows, and the menus it was built from.
    menu_shown: RefCell<Option<(WindowId, Vec<TrayMenuItem>)>>,
    /// The app's name (its menus').
    pub app_title: RefCell<String>,
    /// What a drop's handling answered (`Platform::drop_done`), for the drag's source as AppKit asks.
    pub drop_taken: Cell<Option<bool>>,
}

impl Shared {
    /// The application made ready (C++'s `connect`): a regular app (in the Dock, with a menu bar), its delegate,
    /// its menus, launched; the system's notifications followed; the tracking loops' timer; the quit signals caught.
    pub fn connect(mtm: MainThreadMarker, app_id: WindowId, app_title: &str) -> Rc<Shared> {
        let app = NSApplication::sharedApplication(mtm);
        let shared = Rc::new_cyclic(|weak: &Weak<Shared>| Shared {
            mtm,
            app: app.clone(),
            delegate: AppDelegate::new(mtm, weak.clone()),
            windows: RefCell::new(Vec::new()),
            routed: RefCell::new(VecDeque::new()),
            stepper: RefCell::new(None),
            tracking_timer: RefCell::new(None),
            app_id,
            dark: Cell::new(services::dark_mode(&app)),
            accent: Cell::new(services::accent_color()),
            reduce_motion: Rc::new(Cell::new(services::reduce_motion())),
            clipboard: Clipboard::new(),
            probed: Cell::new(None),
            awake: Cell::new(None),
            menu_items: RefCell::new(Vec::new()),
            menu_shown: RefCell::new(None),
            app_title: RefCell::new(app_title.to_string()),
            drop_taken: Cell::new(None),
        });
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        app.setDelegate(Some(ProtocolObject::from_ref(&*shared.delegate)));
        shared.install_menus(None);
        app.finishLaunching();
        shared.observe();
        // (A timer only in the tracking mode: it fires only while AppKit runs a loop of its own — a window resized, a
        // menu open, a drag — and steps the app meanwhile.)
        // SAFETY: the delegate implements `trackingTick:` taking the timer; the timer keeps the delegate alive.
        let timer = unsafe {
            NSTimer::timerWithTimeInterval_target_selector_userInfo_repeats(
                TRACKING_STEP,
                &shared.delegate,
                sel!(trackingTick:),
                None,
                true,
            )
        };
        // SAFETY: the main run loop and AppKit's tracking mode (a static string of AppKit's, valid for the process).
        unsafe { NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSEventTrackingRunLoopMode) };
        *shared.tracking_timer.borrow_mut() = Some(timer);
        sys::catch_quit_signals(wake_loop);
        shared
    }

    /// The system's notifications the delegate hears: the system's colors, the accessibility options, the locale and
    /// the time zone.
    fn observe(&self) {
        let center = NSNotificationCenter::defaultCenter();
        let workspace_center = NSWorkspace::sharedWorkspace().notificationCenter();
        let delegate: &AnyObject = &self.delegate;
        // SAFETY: the delegate implements each selector taking the notification; the names are the frameworks'
        // statics; the observers are removed as the platform closes (`close`), before the delegate goes.
        unsafe {
            center.addObserver_selector_name_object(
                delegate,
                sel!(systemColorsChanged:),
                Some(NSSystemColorsDidChangeNotification),
                None,
            );
            center.addObserver_selector_name_object(
                delegate,
                sel!(localeChanged:),
                Some(NSCurrentLocaleDidChangeNotification),
                None,
            );
            center.addObserver_selector_name_object(
                delegate,
                sel!(localeChanged:),
                Some(NSSystemTimeZoneDidChangeNotification),
                None,
            );
            workspace_center.addObserver_selector_name_object(
                delegate,
                sel!(accessibilityDisplayOptionsChanged:),
                Some(NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification),
                None,
            );
        }
    }

    /// The platform going: the timer stopped, the notifications no longer heard, the delegate let go, what the app
    /// put on the clipboard given to it in full (its promises can't be kept once the app is gone), the display let
    /// sleep.
    pub fn close(&self) {
        if let Some(timer) = self.tracking_timer.borrow_mut().take() {
            timer.invalidate();
        }
        let delegate: &AnyObject = &self.delegate;
        // SAFETY: the delegate was added as an observer of these centers (`observe`); removing it from all is sound.
        unsafe {
            NSNotificationCenter::defaultCenter().removeObserver(delegate);
            NSWorkspace::sharedWorkspace().notificationCenter().removeObserver(delegate);
        }
        self.app.setDelegate(None);
        self.clipboard.provide_all();
        if let Some(id) = self.awake.take() {
            sys::let_display_sleep(id);
        }
    }

    pub(super) fn set_stepper(&self, stepper: Option<Rc<dyn Fn(StepKind)>>) {
        *self.stepper.borrow_mut() = stepper;
    }

    /// The loop stepped from inside AppKit (a tracking loop's timer, a drop asking for its answer): its stepper
    /// skips a step while one runs.
    pub(crate) fn step(&self, kind: StepKind) {
        let stepper = self.stepper.borrow().clone();
        if let Some(step) = stepper {
            step(kind);
        }
    }

    pub(super) fn push(&self, route: Route, event: Event) {
        self.routed.borrow_mut().push_back((route, event));
    }

    /// The events gathered since, each for its window.
    pub(super) fn take_events(&self) -> Vec<(WindowId, Event)> {
        let routed: Vec<(Route, Event)> = self.routed.borrow_mut().drain(..).collect();
        let windows = self.windows.borrow().clone();
        route(&windows, routed, self.app_id)
    }

    /// The window state of core `id`.
    pub fn window(&self, id: WindowId) -> Option<Rc<WindowState>> {
        self.windows.borrow().iter().find(|w| w.id == id).cloned()
    }

    // ---- The loop's events ----

    /// What is queued handed to AppKit (each window's become its events), without waiting.
    pub(super) fn drain(&self) {
        let past = NSDate::distantPast();
        while let Some(event) = self.next_event(&past) {
            self.dispatch(&event);
        }
        self.after_wait();
    }

    /// Up to `timeout` (none: until something comes) for an event, a wake or the time: what came handed to AppKit.
    pub(super) fn wait(&self, timeout: Option<Duration>) {
        let until = match timeout {
            Some(t) => NSDate::dateWithTimeIntervalSinceNow(t.as_secs_f64()),
            None => NSDate::distantFuture(),
        };
        if let Some(event) = self.next_event(&until) {
            self.dispatch(&event);
        }
        self.after_wait();
    }

    fn next_event(&self, until: &NSDate) -> Option<Retained<NSEvent>> {
        // SAFETY: Foundation's default run loop mode, a static string valid for the process.
        let mode = unsafe { NSDefaultRunLoopMode };
        self.app.nextEventMatchingMask_untilDate_inMode_dequeue(NSEventMask::Any, Some(until), mode, true)
    }

    /// An event handed to AppKit: the loop's own wakes dropped; a key's release with Command held to its window
    /// directly (NSApplication keeps those from the window: its key equivalents' releases), else as AppKit sends
    /// events (its menus' key equivalents first).
    fn dispatch(&self, event: &NSEvent) {
        let kind = event.r#type();
        if kind == NSEventType::ApplicationDefined {
            return;
        }
        if kind == NSEventType::KeyUp
            && event.modifierFlags().0 as u64 & keyboard::FLAG_COMMAND != 0
            && let Some(window) = event.window(self.mtm)
        {
            window.sendEvent(event);
            return;
        }
        self.app.sendEvent(event);
    }

    /// After a wait: a host's wake to every window, a quit signal to the app.
    fn after_wait(&self) {
        if WOKEN.swap(false, Ordering::AcqRel) {
            self.push(Route::All, Event::Wake);
        }
        if sys::take_quit_signal() {
            self.push(Route::App, Event::Quit);
        }
    }

    // ---- The system's state ----

    /// The appearance changed (a view saw it): the windows told if the theme did.
    pub fn appearance_changed(&self) {
        let dark = services::dark_mode(&self.app);
        if dark != self.dark.replace(dark) {
            self.push(Route::All, Event::MetricsChanged);
        }
        self.accent_changed(); // (the accent's own notification may not come with it)
    }

    /// The system's colors changed: the windows told if the accent did (`Platform::handled` sets the palettes).
    pub fn accent_changed(&self) {
        let accent = services::accent_color();
        if accent != self.accent.replace(accent) {
            self.push(Route::All, Event::AccentChanged);
        }
    }

    /// The display kept awake while any window's host asks.
    pub fn keep_display_awake(&self) {
        let on = self.windows.borrow().iter().any(|w| w.keeps_display_on());
        match (on, self.awake.get()) {
            (true, None) => self.awake.set(sys::keep_display_awake(&self.app_title.borrow())),
            (false, Some(id)) => {
                sys::let_display_sleep(id);
                self.awake.set(None);
            }
            _ => {}
        }
    }

    // ---- The menu bar ----

    /// The menu bar for the key window: the app menu (hide, quit), then `menus` (a window's menu bar: its id and its
    /// menus; none: the app's own alone), then the Window menu (minimize, zoom, full screen). Built again only when
    /// it changed.
    pub fn install_menus(&self, menus: Option<(WindowId, Vec<TrayMenuItem>)>) {
        {
            let shown = self.menu_shown.borrow();
            if self.app.mainMenu().is_some() && *shown == menus {
                return;
            }
        }
        let mtm = self.mtm;
        let main = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(""));
        main.setAutoenablesItems(false);
        let name = self.app_title.borrow().clone();
        // The app menu.
        let app_menu = submenu(mtm, &name);
        app_menu.addItem(&action_item(mtm, &format!("Hide {name}"), sel!(hide:), "h", keyboard::FLAG_COMMAND));
        app_menu.addItem(&action_item(
            mtm,
            "Hide Others",
            sel!(hideOtherApplications:),
            "h",
            keyboard::FLAG_COMMAND | keyboard::FLAG_OPTION,
        ));
        app_menu.addItem(&action_item(mtm, "Show All", sel!(unhideAllApplications:), "", 0));
        app_menu.addItem(&NSMenuItem::separatorItem(mtm));
        app_menu.addItem(&action_item(mtm, &format!("Quit {name}"), sel!(terminate:), "q", keyboard::FLAG_COMMAND));
        main.addItem(&holder(mtm, &name, &app_menu));
        // The window's own menus.
        let mut table = Vec::new();
        if let Some((window, items)) = &menus {
            for (i, menu) in items.iter().enumerate() {
                if let Some(item) = self.menu_item(menu, &[i], *window, &mut table) {
                    main.addItem(&item);
                }
            }
        }
        // The Window menu (AppKit lists the app's windows in it).
        let window_menu = submenu(mtm, "Window");
        window_menu.addItem(&action_item(mtm, "Minimize", sel!(performMiniaturize:), "m", keyboard::FLAG_COMMAND));
        window_menu.addItem(&action_item(mtm, "Zoom", sel!(performZoom:), "", 0));
        window_menu.addItem(&NSMenuItem::separatorItem(mtm));
        window_menu.addItem(&action_item(
            mtm,
            "Enter Full Screen",
            sel!(toggleFullScreen:),
            "f",
            keyboard::FLAG_COMMAND | keyboard::FLAG_CONTROL,
        ));
        main.addItem(&holder(mtm, "Window", &window_menu));
        self.app.setMainMenu(Some(&main));
        self.app.setWindowsMenu(Some(&window_menu));
        *self.menu_items.borrow_mut() = table;
        *self.menu_shown.borrow_mut() = menus;
    }

    /// A menu bar's item as AppKit's: a submenu with its items, a separator, or an item clicking through the
    /// delegate (its tag its place in `table`), its shortcut a key equivalent, checked as it is.
    fn menu_item(
        &self,
        item: &TrayMenuItem,
        path: &[usize],
        window: WindowId,
        table: &mut Vec<(WindowId, Vec<usize>)>,
    ) -> Option<Retained<NSMenuItem>> {
        let mtm = self.mtm;
        // (Mnemonics aren't macOS's: its menus are reached by their key equivalents and typing their titles.)
        let title = item.text.clone();
        let made = match item.kind {
            MenuItemKind::Separator => return Some(NSMenuItem::separatorItem(mtm)),
            MenuItemKind::Menu => {
                let menu = submenu(mtm, &title);
                for (i, child) in item.items.iter().enumerate() {
                    let mut at = path.to_vec();
                    at.push(i);
                    if let Some(c) = self.menu_item(child, &at, window, table) {
                        menu.addItem(&c);
                    }
                }
                holder(mtm, &title, &menu)
            }
            MenuItemKind::Item | MenuItemKind::Checkbox { .. } | MenuItemKind::RadioButton { .. } => {
                let (key, mask) =
                    item.shortcut.as_ref().and_then(|(m, name)| keyboard::key_equivalent(*m, name)).unwrap_or_default();
                let made = action_item(mtm, &title, sel!(menuItemClicked:), &key, mask);
                // SAFETY: the delegate implements `menuItemClicked:` taking the item; it outlives the menus
                // (removed from the application before it goes).
                unsafe { made.setTarget(Some(&self.delegate)) };
                table.push((window, path.to_vec()));
                made.setTag(table.len() as isize);
                let on = matches!(
                    item.kind,
                    MenuItemKind::Checkbox { checked: true } | MenuItemKind::RadioButton { selected: true }
                );
                made.setState(if on { 1 } else { 0 });
                made
            }
        };
        made.setEnabled(item.enabled);
        if let Some(icon) = &item.icon
            && let Some(image) = services::image_of_rgba(icon.width, icon.height, &icon.rgba, 16.0)
        {
            made.setImage(Some(&image));
        }
        Some(made)
    }

    /// The window whose menus the menu bar shows changed (it became key), or its menus did.
    pub fn menus_changed(&self) {
        let key = self.windows.borrow().iter().find(|w| w.is_key()).cloned();
        let menus = key.and_then(|w| w.menus().map(|m| (w.id, m)));
        self.install_menus(menus);
    }

    /// An item of a window's menus clicked: to the app core of that window.
    fn menu_clicked(&self, tag: isize) {
        let found = usize::try_from(tag - 1).ok().and_then(|i| self.menu_items.borrow().get(i).cloned());
        if let Some((window, path)) = found {
            self.push(Route::App, Event::MenuBarItem { window, path });
            wake_loop();
        }
    }
}

/// A menu titled `title`, its items enabled as they say.
fn submenu(mtm: MainThreadMarker, title: &str) -> Retained<NSMenu> {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title));
    menu.setAutoenablesItems(false);
    menu
}

/// The menu bar's (or a menu's) item holding `menu`.
fn holder(mtm: MainThreadMarker, title: &str, menu: &NSMenu) -> Retained<NSMenuItem> {
    let item = NSMenuItem::new(mtm);
    item.setTitle(&NSString::from_str(title));
    item.setSubmenu(Some(menu));
    item
}

/// An item sending `action` (to its target, or the responder chain without one), its key equivalent `key` with
/// the modifier flags `mask` (empty: none).
fn action_item(mtm: MainThreadMarker, title: &str, action: Sel, key: &str, mask: u64) -> Retained<NSMenuItem> {
    // SAFETY: `action` is a selector AppKit's responders (or the delegate, once targeted) implement taking a sender.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            Some(action),
            &NSString::from_str(key),
        )
    };
    if !key.is_empty() {
        item.setKeyEquivalentModifierMask(NSEventModifierFlags(mask as usize));
    }
    item
}
