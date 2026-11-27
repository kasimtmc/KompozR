//! An icon of the app's in the system tray and the desktop's notifications (Compose Desktop's Tray.desktop.kt and
//! Notification.desktop.kt; C++'s tray.hpp, 91dff15): [`Tray`] shows its icon while it is composed, its tooltip
//! and menu ([`MenuScope`]) kept up to date as its caller recomposes, gone as it leaves the composition; a click on
//! the icon is its `on_action`; its menu's items call back as they are clicked.
//!
//! ```text
//! let tray = remember_tray_state();
//! Tray(icon = painter, state = tray.clone(), tooltip = Some("AirLink".into()), on_action = move || shown.set(true),
//!      menu = |menu: &MenuScope| {
//!          menu.Item(text = "Aç".into(), on_click = move || shown.set(true));
//!          menu.Separator();
//!          menu.Item(text = "Çık".into(), on_click = || exit_application());
//!      });
//! Button(on_click = move || tray.send_notification(Notification("AirLink", "3 dosya alındı", NotificationType::Info)), …)
//! ```
//!
//! A notification is shown by the Tray composed with its state: on Linux the desktop's own
//! (`org.freedesktop.Notifications`: KDE's, GNOME's, mako, dunst — there with no tray too). One sent before the
//! Tray is composed waits for it (C++'s; Compose Desktop's is lost). Linux shows the icon as a StatusNotifierItem
//! (KDE; GNOME with the AppIndicator extension), its menu over `com.canonical.dbusmenu`; Windows and macOS in their
//! packages; Android and the web show none ([`is_tray_supported`](super::is_tray_supported)).

use super::application::LocalWindowRegistry;
use super::menu::{MenuItem, MenuScope, same_items};
use crate::draw::Painter;
use kompoz_runtime::{DisposableEffect, SideEffect, kompozable, on_dispose, remember};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

/// What kind of notification it is (Compose Desktop's `Notification.Type`): the icon and sound the platform gives
/// it.
///
/// Kotlin: `Notification.Type`
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum NotificationType {
    /// A simple one (on Linux the tray icon's own icon).
    #[default]
    None,
    Info,
    Warning,
    Error,
}

/// A notification of the desktop's (Compose Desktop's Notification): its title, its text, its kind.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Notification {
    pub title: String,
    pub message: String,
    pub r#type: NotificationType,
}

impl Notification {
    pub fn new(title: impl Into<String>, message: impl Into<String>, r#type: NotificationType) -> Notification {
        Notification { title: title.into(), message: message.into(), r#type }
    }

    /// A copy with what is given instead (Compose's `copy`).
    pub fn copy(
        &self,
        title: Option<String>,
        message: Option<String>,
        r#type: Option<NotificationType>,
    ) -> Notification {
        Notification {
            title: title.unwrap_or_else(|| self.title.clone()),
            message: message.unwrap_or_else(|| self.message.clone()),
            r#type: r#type.unwrap_or(self.r#type),
        }
    }
}

/// Compose Desktop's `Notification(title, message, type = Notification.Type.None)`.
#[kompoz_runtime::named(type)]
pub fn Notification(
    title: impl Into<String>,
    message: impl Into<String>,
    #[default(NotificationType::None)] r#type: NotificationType,
) -> Notification {
    Notification::new(title, message, r#type)
}

/// A notification made once where it is called (Compose Desktop's `rememberNotification`).
#[kompozable]
pub fn remember_notification(
    title: String,
    message: String,
    #[default(NotificationType::None)] r#type: NotificationType,
) -> Notification {
    remember(move || Notification::new(title, message, r#type))
}

/// The notifications a [`TrayState`] was sent, waiting for the Tray composed with it, and what wakes the app's
/// loop for them.
#[derive(Default)]
struct TrayNotifications {
    pending: RefCell<Vec<Notification>>,
    wake: RefCell<Option<Rc<dyn Fn()>>>,
}

/// A tray icon's state (Compose Desktop's TrayState), for its notifications: [`send_notification`] shows one through
/// the Tray composed with it. From the app's thread (its callbacks, effects).
///
/// [`send_notification`]: TrayState::send_notification
#[derive(Clone, Default)]
pub struct TrayState {
    notifications: Rc<TrayNotifications>,
}

impl PartialEq for TrayState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.notifications, &other.notifications)
    }
}

impl fmt::Debug for TrayState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TrayState").field("pending", &self.notifications.pending.borrow().len()).finish()
    }
}

impl TrayState {
    pub fn new() -> TrayState {
        TrayState::default()
    }

    /// `notification` shown by the Tray composed with this state (at its app's next frame).
    pub fn send_notification(&self, notification: Notification) {
        self.notifications.pending.borrow_mut().push(notification);
        let wake = self.notifications.wake.borrow().clone();
        if let Some(w) = wake {
            w();
        }
    }

    /// What was sent and not shown yet, taken (the platform's app core's).
    pub fn take_notifications(&self) -> Vec<Notification> {
        std::mem::take(&mut *self.notifications.pending.borrow_mut())
    }

    fn set_wake(&self, wake: Option<Rc<dyn Fn()>>) {
        *self.notifications.wake.borrow_mut() = wake;
    }

    fn has_pending(&self) -> bool {
        !self.notifications.pending.borrow().is_empty()
    }
}

/// Compose Desktop's `TrayState()`.
#[allow(non_snake_case)]
pub fn TrayState() -> TrayState {
    TrayState::new()
}

/// A tray icon's state remembered (Compose Desktop's `rememberTrayState`).
#[kompozable]
pub fn remember_tray_state() -> TrayState {
    remember(TrayState::new)
}

/// A tray icon the composition asks for: what the platform shows.
pub struct TrayRequest {
    id: u64,
    icon: RefCell<Rc<dyn Painter>>,
    tooltip: RefCell<Option<String>>,
    on_action: RefCell<Rc<dyn Fn()>>,
    menu: RefCell<Vec<MenuItem>>,
    state: RefCell<TrayState>,
    /// Bumped as what it shows changes.
    version: Cell<u64>,
}

impl TrayRequest {
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn icon(&self) -> Rc<dyn Painter> {
        self.icon.borrow().clone()
    }
    pub fn tooltip(&self) -> Option<String> {
        self.tooltip.borrow().clone()
    }
    /// Its menu's items now.
    pub fn menu(&self) -> Vec<MenuItem> {
        self.menu.borrow().clone()
    }
    /// Bumped as its icon, tooltip or menu changes.
    pub fn version(&self) -> u64 {
        self.version.get()
    }
    /// The icon clicked (activated): its `on_action`.
    pub fn action(&self) {
        let f = self.on_action.borrow().clone();
        f();
    }
    /// Its state's notifications not shown yet, taken.
    pub fn take_notifications(&self) -> Vec<Notification> {
        self.state.borrow().take_notifications()
    }
}

impl fmt::Debug for TrayRequest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TrayRequest")
            .field("id", &self.id)
            .field("tooltip", &self.tooltip.borrow())
            .field("menu", &self.menu.borrow())
            .field("version", &self.version.get())
            .finish()
    }
}

/// An icon in the system tray (Compose Desktop's `ApplicationScope.Tray`; a composable anywhere here, as C++'s,
/// an `App::run` app's too): `icon` (drawn at the size the tray asks for), `tooltip`, `on_action` as it is clicked,
/// `menu` its menu's items; `state` sends its notifications. Where no tray is shown nothing is (Compose's: a
/// message in the log).
#[kompozable]
pub fn Tray(
    icon: Rc<dyn Painter>,
    #[default(remember_tray_state())] state: TrayState,
    #[default(None)] tooltip: Option<String>,
    #[default(|| {})] on_action: impl Fn() + 'static,
    #[default(|_: &MenuScope| {})] menu: impl Fn(&MenuScope) + 'static,
) {
    let Some(registry) = LocalWindowRegistry.current() else { return };
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let request = remember({
        let (icon, state) = (icon.clone(), state.clone());
        move || {
            Rc::new(TrayRequest {
                id: NEXT.fetch_add(1, Ordering::Relaxed),
                icon: RefCell::new(icon),
                tooltip: RefCell::new(None),
                on_action: RefCell::new(Rc::new(|| {})),
                menu: RefCell::new(Vec::new()),
                state: RefCell::new(state),
                version: Cell::new(0),
            })
        }
    });
    // (The menu's items made as this composes: what they read composes this again.)
    let scope = MenuScope::new();
    menu(&scope);
    let items = scope.take_items();
    // Kept up to date as the caller recomposes: the latest callbacks always (the menu's clicks too), the version
    // bumped only when what is shown changed (the platform shows it again then).
    let changed = !request.icon.borrow().painter_eq(&*icon)
        || *request.tooltip.borrow() != tooltip
        || !same_items(&request.menu.borrow(), &items);
    *request.icon.borrow_mut() = icon;
    *request.tooltip.borrow_mut() = tooltip;
    *request.on_action.borrow_mut() = Rc::new(on_action);
    *request.menu.borrow_mut() = items;
    {
        let (request, registry) = (request.clone(), registry.clone());
        SideEffect(move || {
            if *request.state.borrow() != state {
                request.state.borrow().set_wake(None);
                *request.state.borrow_mut() = state.clone();
            }
            let wake_registry = registry.clone();
            state.set_wake(Some(Rc::new(move || wake_registry.changed())));
            if changed || state.has_pending() {
                if changed {
                    request.version.set(request.version.get() + 1);
                }
                registry.changed(); // (sent before it was composed: shown now)
            }
        });
    }
    let id = request.id;
    DisposableEffect((), move || {
        registry.add_tray(request.clone());
        on_dispose(move || {
            request.state.borrow().set_wake(None);
            registry.remove_tray(id);
        })
    });
}
