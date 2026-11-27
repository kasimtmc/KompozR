//! The app's tray icons and notifications on a Linux desktop (C++'s status_notifier.cpp, 91dff15): each tray icon a
//! StatusNotifierItem on the session bus (`org.kde.StatusNotifierItem`, registered with
//! `org.kde.StatusNotifierWatcher`: KDE's tray, GNOME's AppIndicator extension, waybar's, …), its menu a
//! `com.canonical.dbusmenu`; the desktop's notifications through `org.freedesktop.Notifications`' `Notify` (KDE's,
//! GNOME's, mako, dunst: there with no tray too). Our own D-Bus client ([`dbus`](super::dbus)), on a thread of its
//! own; what the user does with them (an icon activated, a menu item clicked) told to the platform's loop (a wake)
//! as [`Event::TrayAction`] and [`Event::TrayMenuItem`], whose turn runs the request's callbacks.
//!
//! A tray started (or restarted) after the app has its icons registered with it again (the watcher's
//! `NameOwnerChanged`).
//!
//! The windows' menu bars go the same way where the desktop has a global menu (KDE's, Unity's: a
//! `com.canonical.AppMenu.Registrar` on the bus): each window's menus a `com.canonical.dbusmenu` of its own
//! (`/WindowMenu/<window>`), an X11 window registered with the registrar by its id (`RegisterWindow`; a Wayland
//! window's surface told the address through KDE's appmenu protocol by its platform); a click told as an
//! [`Event::MenuBarItem`], the registrar coming or going as [`Event::MenuBarHostChanged`] (the windows' menu bars
//! shown again: by the desktop, or drawn in the windows).

use super::dbus::{Connection, MatchRule, Message, MessageType, Type, Value, errors};
use super::desktop::Notify;
use super::sys;
use crate::app::{Event, TrayIcon, TrayMenuItem};
use kompoz_ui::input::KeyModifiers;
use kompoz_ui::window::{MenuItemKind, Notification, NotificationType, WindowId};
use std::os::fd::{AsFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

const ITEM: &str = "org.kde.StatusNotifierItem";
const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const MENU: &str = "com.canonical.dbusmenu";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const INTROSPECTABLE: &str = "org.freedesktop.DBus.Introspectable";
const NOTIFICATIONS: &str = "org.freedesktop.Notifications";
const NOTIFICATIONS_PATH: &str = "/org/freedesktop/Notifications";
const ITEM_PREFIX: &str = "/StatusNotifierItem/";
const MENU_PREFIX: &str = "/MenuBar/";
const REGISTRAR: &str = "com.canonical.AppMenu.Registrar";
const REGISTRAR_PATH: &str = "/com/canonical/AppMenu/Registrar";
/// A window's menu bar's dbusmenu: `/WindowMenu/<window>` (its `WindowId`'s number).
pub const WINDOW_MENU_PREFIX: &str = "/WindowMenu/";
const CALL_TIMEOUT: Duration = Duration::from_secs(2);

/// An item of an icon's menu as the bus's thread shows it (dbusmenu's id its index: from 1, 0 the root): its
/// children, where it is in the request's menu (what a click names), what it shows.
#[derive(Clone, Debug, Default)]
struct Item {
    children: Vec<i32>,
    path: Vec<usize>,
    item: Option<TrayMenuItem>,
}

/// One icon as the bus's thread shows it: what it had at the last sync, its menu flattened, the menu's revision.
#[derive(Clone, Debug)]
struct Icon {
    icon: TrayIcon,
    /// Its pictures as the spec has them: ARGB32 in network byte order.
    pixmaps: Vec<(i32, i32, Vec<u8>)>,
    items: Vec<Item>,
    revision: u32,
}

/// A window's menu bar as the bus's thread shows it: the window (its id's number), its X11 window (registered with
/// the registrar by it; none: a Wayland window, whose platform tells its compositor), its menus flattened, their
/// revision.
#[derive(Clone, Debug)]
struct WindowMenu {
    window: u64,
    xid: Option<u32>,
    items: Vec<Item>,
    revision: u32,
}

/// A notification to send, as the bus's thread sends it.
struct Sent {
    app: String,
    notification: Notification,
    /// Its icon (the tray icon's, the largest; for one without a type), RGBA.
    image: Option<(i32, i32, Vec<u8>)>,
}

/// What the platform's thread and the bus's share (under the mutex).
#[derive(Default)]
struct State {
    icons: Vec<Icon>,
    /// Icons to register again, or tell to read theirs again.
    changed: bool,
    /// The windows' menu bars, and whether they changed since they were last published.
    windows: Vec<WindowMenu>,
    windows_changed: bool,
    notifications: Vec<Sent>,
    /// What the user did since, for the platform's thread.
    actions: Vec<Event>,
}

struct Shared {
    app: String,
    notify: Notify,
    running: AtomicBool,
    /// A StatusNotifierWatcher is on the bus (a tray shows the icons).
    watcher: AtomicBool,
    /// A global menu's registrar is on the bus (the desktop shows the windows' menu bars).
    registrar: AtomicBool,
    /// The connection's name on the bus (what a Wayland compositor is told a window's menu is at), once connected.
    bus_name: Mutex<Option<String>>,
    state: Mutex<State>,
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The app's tray icons and notifications on the session bus (C++'s StatusNotifier): its thread, what the platform
/// gives it and takes from it.
pub struct StatusNotifier {
    shared: Arc<Shared>,
    /// What wakes the bus's thread (an eventfd: icons synced, a notification, stopping), and the thread.
    thread: Option<(Arc<OwnedFd>, JoinHandle<()>)>,
}

impl StatusNotifier {
    /// Started for the app named `app`, on the session bus at `address` (none: there is none, nothing is shown);
    /// `notify` called on its thread as the user does something with an icon.
    pub fn new(app: &str, notify: Notify, address: Option<String>) -> StatusNotifier {
        let shared = Arc::new(Shared {
            app: app.to_string(),
            notify,
            running: AtomicBool::new(true),
            watcher: AtomicBool::new(false),
            registrar: AtomicBool::new(false),
            bus_name: Mutex::new(None),
            state: Mutex::new(State::default()),
        });
        let thread = address.and_then(|address| {
            let wake = Arc::new(sys::eventfd().ok()?);
            let (shared, their_wake) = (shared.clone(), wake.clone());
            let thread = std::thread::Builder::new()
                .name("kompoz-tray".into())
                .spawn(move || run(&shared, &their_wake, &address))
                .ok()?;
            Some((wake, thread))
        });
        StatusNotifier { shared, thread }
    }

    /// Whether a tray shows the icons: the bus there and a StatusNotifierWatcher on it.
    pub fn available(&self) -> bool {
        self.thread.is_some() && self.shared.watcher.load(Ordering::SeqCst)
    }

    /// The icons as the app asks for them now (the platform's thread): new ones registered, gone ones gone, the
    /// others told to read theirs again.
    pub fn sync(&self, trays: &[TrayIcon]) {
        {
            let mut state = self.shared.state();
            let icons = trays
                .iter()
                .map(|t| {
                    let revision = state.icons.iter().find(|i| i.icon.id == t.id).map_or(1, |i| i.revision + 1);
                    Icon {
                        icon: t.clone(),
                        pixmaps: t.images.iter().map(argb).collect(),
                        items: flatten(&t.menu),
                        revision,
                    }
                })
                .collect();
            state.icons = icons;
            state.changed = true;
        }
        self.wake();
    }

    /// A notification of the desktop's sent by the app for its icon `tray` (shown with the icon's picture when it
    /// has no type).
    pub fn notify(&self, tray: &TrayIcon, notification: &Notification) {
        let image = (notification.r#type == NotificationType::None)
            .then(|| tray.images.iter().max_by_key(|i| i.width))
            .flatten()
            .map(|i| (i.width, i.height, i.rgba.clone()));
        self.shared.state().notifications.push(Sent {
            app: self.shared.app.clone(),
            notification: notification.clone(),
            image,
        });
        self.wake();
    }

    /// Whether the desktop shows the windows' menu bars (a global menu's registrar on the bus).
    pub fn global_menu(&self) -> bool {
        self.thread.is_some() && self.shared.registrar.load(Ordering::SeqCst)
    }

    /// The connection's name on the bus, once connected (where the windows' menus are).
    pub fn bus_name(&self) -> Option<String> {
        self.shared.bus_name.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Window `window`'s menu bar as it is now (none: gone), its X11 window `xid` (registered with the registrar by
    /// it; none: a Wayland window's, told to the compositor by its platform): published at
    /// [`window_menu_path`]`(window)`.
    pub fn set_window_menu(&self, window: u64, menus: Option<&[TrayMenuItem]>, xid: Option<u32>) {
        {
            let mut state = self.shared.state();
            let at = state.windows.iter().position(|w| w.window == window);
            match (menus, at) {
                (Some(menus), Some(i)) => {
                    let w = &mut state.windows[i];
                    (w.items, w.xid, w.revision) = (flatten(menus), xid, w.revision + 1);
                }
                (Some(menus), None) => {
                    state.windows.push(WindowMenu { window, xid, items: flatten(menus), revision: 1 })
                }
                (None, Some(i)) => {
                    state.windows.remove(i);
                }
                (None, None) => return,
            }
            state.windows_changed = true;
        }
        self.wake();
    }

    /// What the user did since, as events for the app's core.
    pub fn take_actions(&self) -> Vec<Event> {
        std::mem::take(&mut self.shared.state().actions)
    }

    fn wake(&self) {
        if let Some((wake, _)) = &self.thread {
            let _ = sys::eventfd_write(wake.as_fd(), 1);
        }
    }
}

impl Drop for StatusNotifier {
    fn drop(&mut self) {
        self.shared.running.store(false, Ordering::SeqCst);
        self.wake();
        if let Some((_, thread)) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// A picture as the StatusNotifierItem spec has it: ARGB32, network byte order.
fn argb(image: &crate::app::TrayImage) -> (i32, i32, Vec<u8>) {
    let mut out = Vec::with_capacity(image.rgba.len());
    for p in image.rgba.as_chunks::<4>().0 {
        out.extend_from_slice(&[p[3], p[0], p[1], p[2]]);
    }
    (image.width, image.height, out)
}

/// A menu flattened for dbusmenu: the root (0), then each item in order, a submenu's after it.
fn flatten(menu: &[TrayMenuItem]) -> Vec<Item> {
    fn add(items: &[TrayMenuItem], parent: i32, at: &[usize], out: &mut Vec<Item>) {
        for (i, m) in items.iter().enumerate() {
            let id = out.len() as i32;
            let mut path = at.to_vec();
            path.push(i);
            out.push(Item { children: Vec::new(), path: path.clone(), item: Some(m.clone()) });
            out[parent as usize].children.push(id);
            if m.kind == MenuItemKind::Menu {
                add(&m.items, id, &path, out);
            }
        }
    }
    let mut out = vec![Item::default()];
    add(menu, 0, &[], &mut out);
    out
}

/// A label as dbusmenu has it: an underscore before the mnemonic's letter (the first such), other underscores
/// doubled.
pub fn dbusmenu_label(text: &str, mnemonic: Option<char>) -> String {
    let mut out = String::new();
    let mut marked = false;
    for c in text.chars() {
        if !marked && mnemonic.is_some_and(|m| m.to_lowercase().eq(c.to_lowercase())) {
            out.push('_');
            marked = true;
        }
        if c == '_' {
            out.push('_');
        }
        out.push(c);
    }
    out
}

fn item_path(id: u64) -> String {
    format!("{ITEM_PREFIX}{id}")
}

fn menu_path(id: u64) -> String {
    format!("{MENU_PREFIX}{id}")
}

/// Where window `window`'s menu bar is published (its dbusmenu).
pub fn window_menu_path(window: u64) -> String {
    format!("{WINDOW_MENU_PREFIX}{window}")
}

/// The bus's thread: connected, watching for the watcher; then publishing, sending and answering until stopped.
fn run(shared: &Shared, wake: &OwnedFd, address: &str) {
    let running = || shared.running.load(Ordering::SeqCst);
    let Ok(mut bus) = Connection::open(address) else { return };
    for name in [WATCHER, REGISTRAR] {
        let rule = MatchRule::signal().interface("org.freedesktop.DBus").member("NameOwnerChanged").arg(0, name);
        let _ = bus.add_match(&rule);
    }
    *shared.bus_name.lock().unwrap_or_else(|e| e.into_inner()) = Some(bus.unique_name().to_string());
    shared.watcher.store(name_there(&mut bus, WATCHER), Ordering::SeqCst);
    if name_there(&mut bus, REGISTRAR) {
        shared.registrar.store(true, Ordering::SeqCst);
        shared.state().actions.push(Event::MenuBarHostChanged); // (the windows' menu bars: the desktop's)
        (shared.notify)();
    }
    let mut registered: Vec<u64> = Vec::new();
    // (The windows registered with the registrar: their ids, their X11 windows.)
    let mut registered_windows: Vec<(u64, u32)> = Vec::new();
    while running() {
        if publish(shared, &mut bus, &mut registered).is_err()
            || publish_windows(shared, &mut bus, &mut registered_windows).is_err()
            || send_notifications(shared, &mut bus).is_err()
        {
            return;
        }
        match bus.read(None, Some(wake.as_fd())) {
            Ok(Some(m)) if m.kind == MessageType::MethodCall => {
                if handle(shared, &mut bus, &m).is_err() {
                    return;
                }
            }
            Ok(Some(m)) if m.is_signal("org.freedesktop.DBus", "NameOwnerChanged") => {
                if m.arg(0).and_then(Value::as_str) == Some(REGISTRAR) {
                    // (A global menu came or went: the windows registered with the new one, or drawn again.)
                    registered_windows.clear();
                    let there = name_there(&mut bus, REGISTRAR);
                    if shared.registrar.swap(there, Ordering::SeqCst) != there {
                        let mut state = shared.state();
                        state.windows_changed = true;
                        state.actions.push(Event::MenuBarHostChanged);
                    }
                    (shared.notify)();
                    continue;
                }
                // (A tray came or went: the icons registered with the new one.)
                registered.clear();
                shared.watcher.store(name_there(&mut bus, WATCHER), Ordering::SeqCst);
                shared.state().changed = true;
                (shared.notify)(); // (is_tray_supported may say otherwise now)
            }
            Ok(_) => {}
            Err(_) => return, // (the bus went away)
        }
    }
}

/// Whether `name` has an owner on the bus (the tray's watcher, a global menu's registrar).
fn name_there(bus: &mut Connection, name: &str) -> bool {
    let reply = bus.call_method(
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "NameHasOwner",
        vec![Value::str(name)],
        CALL_TIMEOUT,
    );
    reply.ok().and_then(|r| r.first().and_then(Value::as_bool)).unwrap_or(false)
}

/// What changed in the windows' menu bars since: X11 windows registered with the registrar (`RegisterWindow`) as
/// they come, `UnregisterWindow` as they go; those that stay told to read their menus again (`LayoutUpdated`).
fn publish_windows(
    shared: &Shared,
    bus: &mut Connection,
    registered: &mut Vec<(u64, u32)>,
) -> Result<(), super::dbus::Error> {
    let windows = {
        let mut state = shared.state();
        if !state.windows_changed {
            return Ok(());
        }
        state.windows_changed = false;
        state.windows.clone()
    };
    let there = shared.registrar.load(Ordering::SeqCst);
    let gone: Vec<(u64, u32)> = registered
        .iter()
        .filter(|(w, xid)| !windows.iter().any(|m| m.window == *w && m.xid == Some(*xid)))
        .copied()
        .collect();
    for (_, xid) in &gone {
        if there {
            let call = Message::method_call(Some(REGISTRAR), REGISTRAR_PATH, Some(REGISTRAR), "UnregisterWindow")
                .with_body(vec![Value::Uint32(*xid)]);
            bus.send(&call)?;
        }
    }
    registered.retain(|r| !gone.contains(r));
    for w in &windows {
        let path = window_menu_path(w.window);
        match w.xid {
            Some(xid) if there && !registered.contains(&(w.window, xid)) => {
                let call = Message::method_call(Some(REGISTRAR), REGISTRAR_PATH, Some(REGISTRAR), "RegisterWindow")
                    .with_body(vec![Value::Uint32(xid), Value::path(path.as_str())]);
                if bus.call(call, CALL_TIMEOUT).is_ok() {
                    registered.push((w.window, xid));
                }
            }
            _ => {
                let layout = Message::signal(&path, MENU, "LayoutUpdated")
                    .with_body(vec![Value::Uint32(w.revision), Value::Int32(0)]);
                bus.send(&layout)?;
            }
        }
    }
    Ok(())
}

/// What changed since: new icons registered with the watcher (by path, on this connection), the others told to
/// read their pictures, tooltips and menus again.
fn publish(shared: &Shared, bus: &mut Connection, registered: &mut Vec<u64>) -> Result<(), super::dbus::Error> {
    let icons = {
        let mut state = shared.state();
        if !state.changed {
            return Ok(());
        }
        state.changed = false;
        state.icons.clone()
    };
    let there = shared.watcher.load(Ordering::SeqCst);
    registered.retain(|id| icons.iter().any(|i| i.icon.id == *id));
    for icon in &icons {
        let id = icon.icon.id;
        if !registered.contains(&id) {
            if there {
                let call =
                    Message::method_call(Some(WATCHER), WATCHER_PATH, Some(WATCHER), "RegisterStatusNotifierItem")
                        .with_body(vec![Value::str(item_path(id))]);
                if bus.call(call, CALL_TIMEOUT).is_ok() {
                    registered.push(id);
                }
            }
            continue;
        }
        for member in ["NewIcon", "NewToolTip", "NewTitle"] {
            bus.send(&Message::signal(&item_path(id), ITEM, member))?;
        }
        let layout = Message::signal(&menu_path(id), MENU, "LayoutUpdated")
            .with_body(vec![Value::Uint32(icon.revision), Value::Int32(0)]);
        bus.send(&layout)?;
    }
    Ok(())
}

/// The notifications sent since: `Notify(app_name, replaces_id, app_icon, summary, body, actions, hints,
/// expire_timeout)`, a type's icon by its freedesktop name, else the tray icon's picture (the `image-data` hint,
/// `(iiibiiay)`), the server's own timeout.
fn send_notifications(shared: &Shared, bus: &mut Connection) -> Result<(), super::dbus::Error> {
    let sent = std::mem::take(&mut shared.state().notifications);
    for n in sent {
        bus.send(&notify_message(&n))?;
    }
    Ok(())
}

fn notify_message(n: &Sent) -> Message {
    let icon = match n.notification.r#type {
        NotificationType::Info => "dialog-information",
        NotificationType::Warning => "dialog-warning",
        NotificationType::Error => "dialog-error",
        NotificationType::None => "",
    };
    let mut hints = Vec::new();
    if let Some((w, h, rgba)) = &n.image
        && icon.is_empty()
    {
        let data = Value::Struct(vec![
            Value::Int32(*w),
            Value::Int32(*h),
            Value::Int32(w * 4),
            Value::Boolean(true),
            Value::Int32(8),
            Value::Int32(4),
            Value::Array(Type::Byte, rgba.iter().map(|&b| Value::Byte(b)).collect()),
        ]);
        hints.push((Value::str("image-data"), Value::variant(data)));
    }
    let urgency = match n.notification.r#type {
        NotificationType::Error => 2u8,
        _ => 1,
    };
    hints.push((Value::str("urgency"), Value::variant(Value::Byte(urgency))));
    Message::method_call(Some(NOTIFICATIONS), NOTIFICATIONS_PATH, Some(NOTIFICATIONS), "Notify").with_body(vec![
        Value::str(n.app.as_str()),
        Value::Uint32(0),
        Value::str(icon),
        Value::str(n.notification.title.as_str()),
        Value::str(n.notification.message.as_str()),
        Value::array(Type::String, Vec::new()),
        Value::dict(Type::String, Type::Variant, hints),
        Value::Int32(-1),
    ])
}

/// A StatusNotifierItem's property.
fn item_property(app: &str, icon: &Icon, name: &str) -> Option<Value> {
    let s = |v: &str| Some(Value::variant(Value::str(v)));
    let pixmaps = |list: &[(i32, i32, Vec<u8>)]| {
        let items = list
            .iter()
            .map(|(w, h, argb)| {
                Value::Struct(vec![
                    Value::Int32(*w),
                    Value::Int32(*h),
                    Value::Array(Type::Byte, argb.iter().map(|&b| Value::Byte(b)).collect()),
                ])
            })
            .collect();
        Value::array(Type::Struct(vec![Type::Int32, Type::Int32, Type::Array(Box::new(Type::Byte))]), items)
    };
    match name {
        "Category" => s("ApplicationStatus"),
        "Id" => s(&format!("{app}-{}-{}", std::process::id(), icon.icon.id)),
        "Title" => s(if icon.icon.tooltip.is_empty() { app } else { &icon.icon.tooltip }),
        "Status" => s("Active"),
        "IconName" | "OverlayIconName" | "AttentionIconName" | "IconThemePath" => s(""),
        "WindowId" => Some(Value::variant(Value::Int32(0))),
        "ItemIsMenu" => Some(Value::variant(Value::Boolean(false))),
        "Menu" => Some(Value::variant(Value::path(menu_path(icon.icon.id)))),
        "IconPixmap" => Some(Value::variant(pixmaps(&icon.pixmaps))),
        "OverlayIconPixmap" | "AttentionIconPixmap" => Some(Value::variant(pixmaps(&[]))),
        "ToolTip" => Some(Value::variant(Value::Struct(vec![
            Value::str(""),
            pixmaps(&[]),
            Value::str(icon.icon.tooltip.as_str()),
            Value::str(""),
        ]))),
        _ => None,
    }
}

const ITEM_PROPERTIES: [&str; 15] = [
    "Category",
    "Id",
    "Title",
    "Status",
    "WindowId",
    "IconName",
    "IconPixmap",
    "OverlayIconName",
    "OverlayIconPixmap",
    "AttentionIconName",
    "AttentionIconPixmap",
    "ToolTip",
    "ItemIsMenu",
    "Menu",
    "IconThemePath",
];

/// A dbusmenu item's properties (the root's: a submenu).
fn item_properties(item: &Item) -> Value {
    let mut p: Vec<(Value, Value)> = Vec::new();
    let mut entry = |key: &str, value: Value| p.push((Value::str(key), Value::variant(value)));
    match &item.item {
        None => entry("children-display", Value::str("submenu")),
        Some(m) if m.kind == MenuItemKind::Separator => entry("type", Value::str("separator")),
        Some(m) => {
            entry("label", Value::String(dbusmenu_label(&m.text, m.mnemonic)));
            // (Both ways told: some hosts keep a disabled state when a later layout leaves it out.)
            entry("enabled", Value::Boolean(m.enabled));
            match m.kind {
                MenuItemKind::Checkbox { checked } => {
                    entry("toggle-type", Value::str("checkmark"));
                    entry("toggle-state", Value::Int32(checked as i32));
                }
                MenuItemKind::RadioButton { selected } => {
                    entry("toggle-type", Value::str("radio"));
                    entry("toggle-state", Value::Int32(selected as i32));
                }
                MenuItemKind::Menu => entry("children-display", Value::str("submenu")),
                _ => {}
            }
            if let Some((modifiers, key)) = &m.shortcut {
                let mut keys = Vec::new();
                for (m, name) in [
                    (KeyModifiers::Ctrl, "Control"),
                    (KeyModifiers::Alt, "Alt"),
                    (KeyModifiers::Shift, "Shift"),
                    (KeyModifiers::Meta, "Super"),
                ] {
                    if modifiers.contains(m) {
                        keys.push(Value::str(name));
                    }
                }
                keys.push(Value::str(key.as_str()));
                let combo = Value::array(Type::String, keys);
                entry("shortcut", Value::array(Type::Array(Box::new(Type::String)), vec![combo]));
            }
            if let Some(icon) = &m.icon
                && let Some(png) = kompoz_graphics::encode_png(icon.width, icon.height, &icon.rgba)
            {
                entry("icon-data", Value::Array(Type::Byte, png.into_iter().map(Value::Byte).collect()));
            }
        }
    }
    Value::dict(Type::String, Type::Variant, p)
}

/// The layout of a menu of `items` from item `id`, `depth` levels down (-1: all), as GetLayout gives it.
fn layout(items: &[Item], id: i32, depth: i32) -> Value {
    let item = &items[id as usize];
    let children = if depth == 0 {
        Vec::new()
    } else {
        item.children
            .iter()
            .map(|&c| Value::variant(layout(items, c, if depth < 0 { -1 } else { depth - 1 })))
            .collect()
    };
    Value::Struct(vec![Value::Int32(id), item_properties(item), Value::array(Type::Variant, children)])
}

/// A call answered (under the mutex: the icons as last synced), the actions it took notified.
fn handle(shared: &Shared, bus: &mut Connection, call: &Message) -> Result<(), super::dbus::Error> {
    if bus.answer_peer(call)? {
        return Ok(());
    }
    let path = call.path.as_deref().unwrap_or("");
    let interface = call.interface.as_deref().unwrap_or("");
    let member = call.member.as_deref().unwrap_or("");
    let (reply, acted) = {
        let mut state = shared.state();
        let before = state.actions.len();
        let reply = answer(shared, &mut state, path, interface, member, call);
        (reply, state.actions.len() > before)
    };
    if acted {
        (shared.notify)(); // (the platform's loop woken)
    }
    match reply {
        Ok(Some(body)) => bus.reply(call, body),
        Ok(None) => Ok(()),
        Err((name, text)) => bus.reply_error(call, name, text),
    }
}

type Answer = Result<Option<Vec<Value>>, (&'static str, &'static str)>;

fn answer(shared: &Shared, state: &mut State, path: &str, interface: &str, member: &str, call: &Message) -> Answer {
    let by = |prefix: &str| path.strip_prefix(prefix).and_then(|id| id.parse::<u64>().ok());
    if let Some(window) = by(WINDOW_MENU_PREFIX) {
        let Some(menu) = state.windows.iter().find(|w| w.window == window).cloned() else {
            return Err((errors::UNKNOWN_OBJECT, "No such object"));
        };
        let clicked = |state: &mut State, path: Vec<usize>| {
            state.actions.push(Event::MenuBarItem { window: WindowId(window), path });
        };
        return dbusmenu(state, &menu.items, menu.revision, interface, member, call, &clicked);
    }
    let (item, menu) = (by(ITEM_PREFIX), by(MENU_PREFIX));
    let Some(icon) = item.or(menu).and_then(|id| state.icons.iter().find(|i| i.icon.id == id)).cloned() else {
        return Err((errors::UNKNOWN_OBJECT, "No such object"));
    };
    let arg_str = |i: usize| call.arg(i).and_then(Value::as_str).unwrap_or("");
    if interface == INTROSPECTABLE && member == "Introspect" && item.is_some() {
        return Ok(Some(vec![Value::str(ITEM_XML)]));
    }
    if interface == PROPERTIES && item.is_some() {
        let (of, name) = (arg_str(0), arg_str(1));
        if member == "Set" {
            return Err((errors::PROPERTY_READ_ONLY, "Read only"));
        }
        let all: Vec<(String, Value)> = if of == ITEM {
            ITEM_PROPERTIES
                .iter()
                .filter_map(|p| Some((p.to_string(), item_property(&shared.app, &icon, p)?)))
                .collect()
        } else {
            Vec::new()
        };
        return match member {
            "Get" => match all.into_iter().find(|(n, _)| n == name) {
                Some((_, v)) => Ok(Some(vec![v])),
                None => Err((errors::UNKNOWN_PROPERTY, "No such property")),
            },
            "GetAll" => {
                let entries = all.into_iter().map(|(n, v)| (Value::String(n), v)).collect();
                Ok(Some(vec![Value::dict(Type::String, Type::Variant, entries)]))
            }
            _ => Err((errors::UNKNOWN_METHOD, "No such method")),
        };
    }
    if item.is_some() && interface == ITEM {
        // Activate (a click), SecondaryActivate (a middle click): the icon's on_action. ContextMenu (the host shows
        // the menu itself from Menu) and Scroll do nothing.
        if member == "Activate" || member == "SecondaryActivate" {
            state.actions.push(Event::TrayAction { tray: icon.icon.id });
        }
        return Ok(Some(Vec::new()));
    }
    if menu.is_some() {
        let tray = icon.icon.id;
        let clicked = |state: &mut State, path: Vec<usize>| state.actions.push(Event::TrayMenuItem { tray, path });
        return dbusmenu(state, &icon.items, icon.revision, interface, member, call, &clicked);
    }
    Err((errors::UNKNOWN_METHOD, "No such method"))
}

/// A `com.canonical.dbusmenu` call (its properties, its introspection) on a menu of `items` at `revision`: a tray
/// icon's, a window's menu bar's; `clicked` told the path of an item clicked.
fn dbusmenu(
    state: &mut State,
    items: &[Item],
    revision: u32,
    interface: &str,
    member: &str,
    call: &Message,
    clicked: &dyn Fn(&mut State, Vec<usize>),
) -> Answer {
    let arg_i32 = |i: usize| call.arg(i).and_then(|v| v.unwrap_variants().as_i32());
    let arg_str = |i: usize| call.arg(i).and_then(Value::as_str).unwrap_or("");
    if interface == INTROSPECTABLE && member == "Introspect" {
        return Ok(Some(vec![Value::str(MENU_XML)]));
    }
    if interface == PROPERTIES {
        let (of, name) = (arg_str(0), arg_str(1));
        if member == "Set" {
            return Err((errors::PROPERTY_READ_ONLY, "Read only"));
        }
        let all: Vec<(String, Value)> = if of == MENU {
            vec![
                ("Version".into(), Value::variant(Value::Uint32(3))),
                ("TextDirection".into(), Value::variant(Value::str("ltr"))),
                ("Status".into(), Value::variant(Value::str("normal"))),
                ("IconThemePath".into(), Value::variant(Value::array(Type::String, Vec::new()))),
            ]
        } else {
            Vec::new()
        };
        return match member {
            "Get" => match all.into_iter().find(|(n, _)| n == name) {
                Some((_, v)) => Ok(Some(vec![v])),
                None => Err((errors::UNKNOWN_PROPERTY, "No such property")),
            },
            "GetAll" => {
                let entries = all.into_iter().map(|(n, v)| (Value::String(n), v)).collect();
                Ok(Some(vec![Value::dict(Type::String, Type::Variant, entries)]))
            }
            _ => Err((errors::UNKNOWN_METHOD, "No such method")),
        };
    }
    if interface != MENU {
        return Err((errors::UNKNOWN_METHOD, "No such method"));
    }
    let click = |state: &mut State, id: i32, kind: &str| {
        if kind == "clicked"
            && let Some(it) = usize::try_from(id).ok().filter(|&i| i > 0).and_then(|i| items.get(i))
            && it
                .item
                .as_ref()
                .is_some_and(|m| m.enabled && !matches!(m.kind, MenuItemKind::Separator | MenuItemKind::Menu))
        {
            clicked(state, it.path.clone());
        }
    };
    match member {
        "GetLayout" => {
            let parent = arg_i32(0).filter(|&p| p >= 0 && (p as usize) < items.len()).unwrap_or(0);
            let depth = arg_i32(1).unwrap_or(-1);
            Ok(Some(vec![Value::Uint32(revision), layout(items, parent, depth)]))
        }
        "GetGroupProperties" => {
            let mut ids: Vec<i32> = call
                .arg(0)
                .and_then(Value::items)
                .map(|items| items.iter().filter_map(Value::as_i32).collect())
                .unwrap_or_default();
            if ids.is_empty() {
                ids = (0..items.len() as i32).collect();
            }
            let entries = ids
                .into_iter()
                .filter(|&id| id >= 0 && (id as usize) < items.len())
                .map(|id| Value::Struct(vec![Value::Int32(id), item_properties(&items[id as usize])]))
                .collect();
            let element = Type::Struct(vec![Type::Int32, Type::dict(Type::String, Type::Variant)]);
            Ok(Some(vec![Value::array(element, entries)]))
        }
        "GetProperty" => {
            let id = arg_i32(0).unwrap_or(-1);
            let props = usize::try_from(id).ok().and_then(|i| items.get(i)).map(item_properties);
            let wanted = arg_str(1);
            let value = props.as_ref().and_then(|p| p.get(wanted)).cloned();
            match value {
                Some(v) => Ok(Some(vec![v])),
                None => Err((errors::INVALID_ARGS, "No such property")),
            }
        }
        "Event" => {
            click(state, arg_i32(0).unwrap_or(-1), arg_str(1));
            Ok(Some(Vec::new()))
        }
        "EventGroup" => {
            for e in call.arg(0).and_then(Value::items).unwrap_or(&[]) {
                if let Some([id, kind, ..]) = e.fields() {
                    click(state, id.as_i32().unwrap_or(-1), kind.as_str().unwrap_or(""));
                }
            }
            Ok(Some(vec![Value::array(Type::Int32, Vec::new())]))
        }
        "AboutToShow" => Ok(Some(vec![Value::Boolean(false)])),
        "AboutToShowGroup" => {
            Ok(Some(vec![Value::array(Type::Int32, Vec::new()), Value::array(Type::Int32, Vec::new())]))
        }
        _ => Err((errors::UNKNOWN_METHOD, "No such method")),
    }
}

const ITEM_XML: &str = r#"<node><interface name="org.kde.StatusNotifierItem">
<property name="Category" type="s" access="read"/><property name="Id" type="s" access="read"/>
<property name="Title" type="s" access="read"/><property name="Status" type="s" access="read"/>
<property name="WindowId" type="i" access="read"/><property name="IconName" type="s" access="read"/>
<property name="IconPixmap" type="a(iiay)" access="read"/><property name="OverlayIconName" type="s" access="read"/>
<property name="OverlayIconPixmap" type="a(iiay)" access="read"/><property name="AttentionIconName" type="s" access="read"/>
<property name="AttentionIconPixmap" type="a(iiay)" access="read"/><property name="ToolTip" type="(sa(iiay)ss)" access="read"/>
<property name="ItemIsMenu" type="b" access="read"/><property name="Menu" type="o" access="read"/>
<property name="IconThemePath" type="s" access="read"/>
<method name="ContextMenu"><arg name="x" type="i" direction="in"/><arg name="y" type="i" direction="in"/></method>
<method name="Activate"><arg name="x" type="i" direction="in"/><arg name="y" type="i" direction="in"/></method>
<method name="SecondaryActivate"><arg name="x" type="i" direction="in"/><arg name="y" type="i" direction="in"/></method>
<method name="Scroll"><arg name="delta" type="i" direction="in"/><arg name="orientation" type="s" direction="in"/></method>
<signal name="NewTitle"/><signal name="NewIcon"/><signal name="NewAttentionIcon"/><signal name="NewOverlayIcon"/>
<signal name="NewToolTip"/><signal name="NewStatus"><arg name="status" type="s"/></signal>
</interface></node>"#;

const MENU_XML: &str = r#"<node><interface name="com.canonical.dbusmenu">
<property name="Version" type="u" access="read"/><property name="TextDirection" type="s" access="read"/>
<property name="Status" type="s" access="read"/><property name="IconThemePath" type="as" access="read"/>
<method name="GetLayout"><arg type="i" name="parentId" direction="in"/><arg type="i" name="recursionDepth" direction="in"/>
<arg type="as" name="propertyNames" direction="in"/><arg type="u" name="revision" direction="out"/>
<arg type="(ia{sv}av)" name="layout" direction="out"/></method>
<method name="GetGroupProperties"><arg type="ai" name="ids" direction="in"/><arg type="as" name="propertyNames" direction="in"/>
<arg type="a(ia{sv})" name="properties" direction="out"/></method>
<method name="GetProperty"><arg type="i" name="id" direction="in"/><arg type="s" name="name" direction="in"/>
<arg type="v" name="value" direction="out"/></method>
<method name="Event"><arg type="i" name="id" direction="in"/><arg type="s" name="eventId" direction="in"/>
<arg type="v" name="data" direction="in"/><arg type="u" name="timestamp" direction="in"/></method>
<method name="EventGroup"><arg type="a(isvu)" name="events" direction="in"/><arg type="ai" name="idErrors" direction="out"/></method>
<method name="AboutToShow"><arg type="i" name="id" direction="in"/><arg type="b" name="needUpdate" direction="out"/></method>
<method name="AboutToShowGroup"><arg type="ai" name="ids" direction="in"/><arg type="ai" name="updatesNeeded" direction="out"/>
<arg type="ai" name="idErrors" direction="out"/></method>
<signal name="ItemsPropertiesUpdated"><arg type="a(ia{sv})" name="updatedProps"/><arg type="a(ias)" name="removedProps"/></signal>
<signal name="LayoutUpdated"><arg type="u" name="revision"/><arg type="i" name="parent"/></signal>
</interface></node>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_mark_their_mnemonic_and_double_underscores() {
        assert_eq!(dbusmenu_label("Open", Some('O')), "_Open");
        assert_eq!(dbusmenu_label("Save as", Some('a')), "S_ave as");
        assert_eq!(dbusmenu_label("snake_case", None), "snake__case");
        assert_eq!(dbusmenu_label("Quit", Some('x')), "Quit");
    }

    #[test]
    fn menus_are_flattened_parents_before_children() {
        let item = |text: &str, kind, items| TrayMenuItem {
            kind,
            text: text.into(),
            enabled: true,
            mnemonic: None,
            shortcut: None,
            icon: None,
            items,
        };
        let menu = vec![
            item("A", MenuItemKind::Item, vec![]),
            item("Sub", MenuItemKind::Menu, vec![item("B", MenuItemKind::Item, vec![])]),
            item("C", MenuItemKind::Item, vec![]),
        ];
        let flat = flatten(&menu);
        assert_eq!(flat.len(), 5);
        assert_eq!(flat[0].children, [1, 2, 4]);
        assert_eq!(flat[2].children, [3]);
        assert_eq!(flat[3].path, [1, 0]);
        assert_eq!(flat[4].path, [2]);
    }

    #[test]
    fn pictures_are_argb_in_network_order() {
        let image = crate::app::TrayImage { width: 1, height: 1, rgba: vec![10, 20, 30, 40] };
        assert_eq!(argb(&image), (1, 1, vec![40, 10, 20, 30]));
    }
}
