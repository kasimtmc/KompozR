//! The app's windows together (C++'s AppWindows): the app's root core (the main window's, or the app's composition's
//! without a window) and a core for each `Window` its compositions ask for, each on a platform window of its own
//! (made by the platform's factory, a handle on its shared connection), opened as its request comes and is shown,
//! kept up to date both ways, hidden and shown again, closed as it goes; the windows' compositions' own `Window`s
//! too. Events are routed to the core of the window they are for ([`WindowId`]); a script's "@Title: command"s to the
//! window of that title; the app's tray icons and their notifications to the root's platform; while the app's script
//! runs in virtual time, its windows' frames follow its clock.
//!
//! The platform's loop owns it: it hands it the events it read by window, asks each core for its frames as its
//! windows are ready for them, and calls [`sync`](AppWindows::sync) between frames.

use super::app_core::paint_image;
use super::{AppCore, Event, Platform, TrayIcon, TrayImage, TrayMenuItem};
use crate::system::log;
use kompoz_ui::input::{Key, KeyModifiers};
use kompoz_ui::window::{
    ApplicationControl, MenuItem, MenuItemKind, Notification, TrayRequest, WindowId, WindowRequest, item_at,
    set_application_control,
};
use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// What makes a platform window for a `Window` (a handle on the platform's connection, for the request's id): none
/// where it can't be made, or the platform gives an app one window (Android, the web).
pub type MakeWindow<P> = Box<dyn FnMut(&WindowRequest) -> Option<P>>;

/// The sizes a tray icon is drawn at (the tray picks the one it needs: 22 dp at 1x, 2x; 16, 24, 32 and 48 for the
/// trays that ask for those).
const TRAY_SIZES: [i32; 6] = [16, 22, 24, 32, 44, 48];

/// A `Window` open: its request, its core.
struct Open<P: Platform> {
    request: Rc<WindowRequest>,
    core: AppCore<P>,
}

/// What `exit_application` and `is_tray_supported` reach on the app's thread.
struct Control {
    exit: Rc<Cell<bool>>,
    wake: Option<kompoz_ui::HostHandle>,
    tray_supported: Rc<Cell<bool>>,
}

impl ApplicationControl for Control {
    fn exit(&self) {
        self.exit.set(true);
        if let Some(h) = &self.wake {
            h.wake();
        }
    }
    fn tray_supported(&self) -> bool {
        self.tray_supported.get()
    }
}

/// The app's cores: its root's and its `Window`s' (C++'s AppWindows).
pub struct AppWindows<P: Platform> {
    /// The windows open, in the order they came (closed before the root goes: its renderer draws the app's
    /// bitmaps).
    windows: Vec<Open<P>>,
    app: AppCore<P>,
    /// Requests whose window couldn't be made: not tried again while they last.
    failed: Vec<WindowId>,
    make: MakeWindow<P>,
    /// The tray icons the platform shows, and what they were made from (their ids and versions).
    trays: Vec<TrayIcon>,
    trays_key: Option<u64>,
    tray_supported: Rc<Cell<bool>>,
}

impl<P: Platform> AppWindows<P> {
    /// The app's cores, its root `app` (not started), its `Window`s' platform windows made by `make`.
    pub fn new(app: AppCore<P>, make: MakeWindow<P>) -> AppWindows<P> {
        AppWindows {
            windows: Vec::new(),
            app,
            failed: Vec::new(),
            make,
            trays: Vec::new(),
            trays_key: None,
            tray_supported: Rc::new(Cell::new(false)),
        }
    }

    /// The root core started (C++'s `AppCore::start`), `exit_application` and `is_tray_supported` reaching it;
    /// false if the app can't run.
    pub fn start(&mut self) -> bool {
        if !self.app.start() {
            return false;
        }
        self.tray_supported.set(self.app.platform().tray_supported());
        set_application_control(Some(Rc::new(Control {
            exit: self.app.exit_flag(),
            wake: self.app.host().map(|h| h.handle()),
            tray_supported: self.tray_supported.clone(),
        })));
        true
    }

    /// The root core (the main window's, or the app's composition's).
    pub fn app(&self) -> &AppCore<P> {
        &self.app
    }

    pub fn app_mut(&mut self) -> &mut AppCore<P> {
        &mut self.app
    }

    /// The id the root's events come with: the main window's, or the app's composition's.
    pub fn app_id(&self) -> WindowId {
        if self.app.is_application() { WindowId::APPLICATION } else { WindowId::MAIN }
    }

    /// The ids of the app's cores, the root's first.
    pub fn ids(&self) -> Vec<WindowId> {
        std::iter::once(self.app_id()).chain(self.windows.iter().map(|w| w.request.id())).collect()
    }

    /// The core of window `id` (the root's for the main window's or the app's id).
    pub fn core(&self, id: WindowId) -> Option<&AppCore<P>> {
        if id == WindowId::MAIN || id == WindowId::APPLICATION {
            return Some(&self.app);
        }
        self.windows.iter().find(|w| w.request.id() == id).map(|w| &w.core)
    }

    pub fn core_mut(&mut self, id: WindowId) -> Option<&mut AppCore<P>> {
        if id == WindowId::MAIN || id == WindowId::APPLICATION {
            return Some(&mut self.app);
        }
        self.windows.iter_mut().find(|w| w.request.id() == id).map(|w| &mut w.core)
    }

    /// The `Window`s' cores, in the order they came.
    pub fn window_cores_mut(&mut self) -> impl Iterator<Item = &mut AppCore<P>> {
        self.windows.iter_mut().map(|w| &mut w.core)
    }

    /// All the cores, the root's first.
    pub fn cores_mut(&mut self) -> Vec<&mut AppCore<P>> {
        let mut all = vec![&mut self.app];
        all.extend(self.windows.iter_mut().map(|w| &mut w.core));
        all
    }

    /// An event for window `id`: to its core (the app's tray icons' to their requests); one for a window that went
    /// is dropped.
    pub fn handle(&mut self, id: WindowId, event: &Event) {
        match event {
            Event::TrayAction { tray } => {
                if let Some(r) = self.tray_request(*tray) {
                    r.action();
                }
            }
            // (A menu bar's item: to the window it is in, whichever platform window it came through.)
            Event::MenuBarItem { window, .. } => {
                if let Some(core) = self.core_mut(*window) {
                    core.handle(event);
                }
            }
            Event::TrayMenuItem { tray, path } => {
                if let Some(r) = self.tray_request(*tray)
                    && let Some(item) = item_at(&r.menu(), path)
                {
                    item.click();
                }
            }
            _ => {
                if let Some(core) = self.core_mut(id) {
                    core.handle(event);
                }
            }
        }
    }

    /// The hosts' requests, the root's first, then each open window's (in the order they came).
    fn wanted(&self) -> Vec<Rc<WindowRequest>> {
        let mut wanted = Vec::new();
        let hosts = std::iter::once(&self.app).chain(self.windows.iter().map(|w| &w.core));
        for core in hosts {
            if let Some(host) = core.host() {
                wanted.extend(host.window_registry().windows());
            }
        }
        wanted
    }

    /// The tray icons all the hosts ask for.
    fn tray_requests(&self) -> Vec<Rc<TrayRequest>> {
        let mut trays = Vec::new();
        let hosts = std::iter::once(&self.app).chain(self.windows.iter().map(|w| &w.core));
        for core in hosts {
            if let Some(host) = core.host() {
                trays.extend(host.window_registry().trays());
            }
        }
        trays
    }

    fn tray_request(&self, id: u64) -> Option<Rc<TrayRequest>> {
        self.tray_requests().into_iter().find(|r| r.id() == id)
    }

    /// Between frames: the windows as their requests are now (C++'s `AppWindows::sync`) — those gone closed, the
    /// new ones (shown) opened, each kept up to date (shown or hidden, its title, state and parameters), a script's
    /// "@Title: command"s handed on, the windows' frames on the app's virtual clock while its script runs; the tray
    /// icons as they are now, and the notifications sent through them.
    pub fn sync(&mut self) {
        let wanted = self.wanted();
        let is_wanted = |id: WindowId| wanted.iter().any(|r| r.id() == id);
        let mut i = self.windows.len();
        while i > 0 {
            i -= 1;
            if !is_wanted(self.windows[i].request.id()) {
                self.close(i);
            }
        }
        self.failed.retain(|&id| is_wanted(id));
        for r in &wanted {
            let id = r.id();
            if !r.visible() || self.windows.iter().any(|w| w.request.id() == id) || self.failed.contains(&id) {
                continue;
            }
            let opened = (self.make)(r).and_then(|platform| {
                let mut core = AppCore::window(platform, self.app.config(), r.clone());
                core.start().then_some(core)
            });
            match opened {
                Some(core) => self.windows.push(Open { request: r.clone(), core }),
                None => {
                    log(&format!("kompoz: the window '{}' couldn't be opened", r.title()));
                    self.failed.push(id);
                }
            }
        }
        let clock = self.app.virtual_time();
        for w in &mut self.windows {
            w.core.set_visible(w.request.visible());
            w.core.sync_window();
            w.core.set_virtual_clock(clock);
        }
        // "@Title: command": to the window of that title (the root's script's, or a window's).
        let mut forwards = self.app.take_script_forwards();
        for w in &mut self.windows {
            forwards.extend(w.core.take_script_forwards());
        }
        for (title, command) in forwards {
            let title = title.trim();
            match self.windows.iter_mut().find(|w| w.core.title() == title) {
                Some(w) => w.core.run_script(&command),
                None if self.app.title() == title => self.app.run_script(&command),
                None => log(&format!("kompoz: script: no window titled '{title}'")),
            }
        }
        self.sync_trays();
    }

    /// The tray icons to the root's platform when any came, went or changed; their states' notifications sent
    /// since, each shown once.
    fn sync_trays(&mut self) {
        self.tray_supported.set(self.app.platform().tray_supported());
        let requests = self.tray_requests();
        let mut hasher = DefaultHasher::new();
        for r in &requests {
            (r.id(), r.version()).hash(&mut hasher);
        }
        let key = (!requests.is_empty()).then(|| hasher.finish());
        if key != self.trays_key && (key.is_some() || self.trays_key.is_some()) {
            self.trays_key = key;
            self.trays = requests.iter().map(|r| tray_icon(r)).collect();
            self.app.platform_mut().sync_trays(&self.trays);
        }
        for r in &requests {
            for n in r.take_notifications() {
                let icon = self.trays.iter().find(|t| t.id == r.id()).cloned().unwrap_or_else(|| tray_icon(r));
                self.app.platform_mut().send_notification(&icon, &n);
            }
        }
    }

    /// A notification sent without a tray icon's state (a platform's own, a test's), through the first tray icon.
    pub fn send_notification(&mut self, notification: &Notification) {
        let icon = self.trays.first().cloned().unwrap_or_default();
        self.app.platform_mut().send_notification(&icon, notification);
    }

    /// Window `index` closed: its core quit, then gone (its host and device before its platform window).
    fn close(&mut self, index: usize) {
        let mut w = self.windows.remove(index);
        w.core.quit();
        drop(w);
    }

    /// Whether the app is done: its root asked (the main window closed, a signal, `exit_application`, a script's
    /// `quit`), a window's script quit, or (`application { }`) nothing is left in it — no window asked for (hidden
    /// ones too: their compositions go on), no tray icon —
    /// after its first frame (Compose Desktop's application ends as its last window closes).
    pub fn quitting(&self) -> bool {
        if self.app.quitting() || self.windows.iter().any(|w| w.core.quitting()) {
            return true;
        }
        self.app.is_application()
            && self.app.frames() > 0
            && self.windows.is_empty()
            && self.wanted().iter().all(|r| self.failed.contains(&r.id()))
            && self.tray_requests().is_empty()
    }

    /// Before the platform goes (C++'s `quit`): the windows closed, the tray icons gone, the root's state written
    /// (`KOMPOZ_STATE_FILE`).
    pub fn quit(&mut self) {
        while !self.windows.is_empty() {
            self.close(self.windows.len() - 1);
        }
        if self.trays_key.take().is_some() {
            self.trays.clear();
            self.app.platform_mut().sync_trays(&[]);
        }
        set_application_control(None);
        self.app.quit();
    }
}

impl<P: Platform> Drop for AppWindows<P> {
    fn drop(&mut self) {
        // (The windows before the root: its renderer has the app's bitmaps.)
        while !self.windows.is_empty() {
            let w = self.windows.pop();
            drop(w);
        }
        set_application_control(None);
    }
}

/// A tray request as the platform shows it: its icon drawn at the tray's sizes, its menu without its callbacks.
fn tray_icon(r: &TrayRequest) -> TrayIcon {
    let icon = r.icon();
    let images: Vec<TrayImage> = TRAY_SIZES.iter().filter_map(|&s| paint_image(&*icon, s)).collect();
    TrayIcon {
        id: r.id(),
        images,
        tooltip: r.tooltip().unwrap_or_default(),
        menu: r.menu().iter().map(menu_item).collect(),
    }
}

pub(crate) fn menu_item(item: &MenuItem) -> TrayMenuItem {
    TrayMenuItem {
        kind: item.kind,
        text: item.text.clone(),
        enabled: item.enabled,
        mnemonic: item.mnemonic,
        shortcut: item.shortcut.map(|s| {
            let mut modifiers = KeyModifiers::None;
            for (on, m) in [
                (s.ctrl, KeyModifiers::Ctrl),
                (s.shift, KeyModifiers::Shift),
                (s.alt, KeyModifiers::Alt),
                (s.meta, KeyModifiers::Meta),
            ] {
                if on {
                    modifiers = modifiers | m;
                }
            }
            (modifiers, key_name(s.key))
        }),
        icon: item.icon.as_ref().and_then(|i| paint_image(&**i, 16)),
        items: if item.kind == MenuItemKind::Menu { item.items.iter().map(menu_item).collect() } else { Vec::new() },
    }
}

/// A key's name as the desktop's menus show a shortcut (xkb's keysym names: "q", "F4", "Delete").
pub fn key_name(key: Key) -> String {
    let named = [
        (Key::Enter, "Return"),
        (Key::Escape, "Escape"),
        (Key::Tab, "Tab"),
        (Key::Spacebar, "space"),
        (Key::Backspace, "BackSpace"),
        (Key::Delete, "Delete"),
        (Key::Insert, "Insert"),
        (Key::MoveHome, "Home"),
        (Key::MoveEnd, "End"),
        (Key::PageUp, "Page_Up"),
        (Key::PageDown, "Page_Down"),
        (Key::DirectionUp, "Up"),
        (Key::DirectionDown, "Down"),
        (Key::DirectionLeft, "Left"),
        (Key::DirectionRight, "Right"),
        (Key::Plus, "plus"),
        (Key::Minus, "minus"),
        (Key::Comma, "comma"),
        (Key::Period, "period"),
    ];
    if let Some((_, name)) = named.iter().find(|(k, _)| *k == key) {
        return name.to_string();
    }
    for (i, f) in crate::keys::FUNCTION_KEYS.iter().enumerate() {
        if *f == key {
            return format!("F{}", i + 1);
        }
    }
    match char::from_u32(key.typed_code_point(false) as u32) {
        Some(c) if c != '\0' => c.to_string(),
        _ => String::new(),
    }
}
