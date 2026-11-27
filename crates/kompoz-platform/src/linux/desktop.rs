//! What KompozR's platforms on a Linux desktop (Wayland, X11) take from the desktop alike (C++'s
//! desktop_services): links opened with `xdg-open`, the files a dropped `text/uri-list` names, the title bar's
//! buttons as the desktop lays them out, and the desktop's services ([`DesktopServices`]), each on a thread of its
//! own that wakes the platform's loop: the accent color, the light or dark preference, GNOME's window buttons and
//! the system's text size (the settings portal: [`portal`](super::portal)), the wallpaper's colors
//! ([`wallpaper`](super::wallpaper)), the screen readers (AT-SPI: [`atspi`](super::atspi)), the app's tray icons and
//! the desktop's notifications ([`status_notifier`](super::status_notifier)); the screen saver held off over D-Bus,
//! a microphone and a camera ([`media`](super::media)).
//!
//! # How a platform runs them
//!
//! A platform owns one `DesktopServices` and passes its [`Platform`](crate::app::Platform) hooks on to it:
//!
//! ```text
//! // Platform::attach (the host made): started, its threads waking the platform's loop.
//! self.desktop.start(&config.title, self.event_loop.waker().as_fn());
//! // After each wait of the loop (woken or not): what the services told, as events for the app core.
//! for e in self.desktop.take_events() { core.handle(&e) }        // (or queued with the platform's own)
//! // Platform::metrics:
//! m.dark_theme = self.desktop.dark_theme();
//! m.font_scale = KOMPOZ_FONT_SCALE, else self.desktop.font_scale().unwrap_or(1.0);
//! m.title_bar = own_decorations.then(|| self.desktop.title_bar(maximized, can_minimize, can_maximize));
//! // Platform::metrics_updated, handled, composed, wants_frames, media:
//! fn metrics_updated(&mut self, _: &KompozHost) { self.desktop.metrics_updated() }
//! fn handled(&mut self, host: &KompozHost, e: &Event) { self.desktop.handled(host, e) }
//! fn composed(&mut self, host: &KompozHost, redraw: bool, t: i64) {
//!     // (X11 only: Wayland holds the screen saver off with an idle inhibitor on its surface.)
//!     self.desktop.keep_screen_on(host.keep_screen_on(), &title);
//!     self.desktop.composed(host, redraw, t, window_in_points, points_per_px, &title);
//! }
//! fn wants_frames(&self) -> bool { self.desktop.wants_frames() }  // a frame more: the tree to publish
//! fn media(&mut self, probe: bool) -> MediaInfo { self.desktop.media(probe, pointer, keyboard) }
//! // Before the loop goes: their threads ended.
//! self.desktop.stop();
//! ```

use super::atspi::{AtspiBridge, AtspiOptions, Window as AtspiWindow};
use super::dbus::Connection;
use super::media::MediaDevices;
use super::portal::{DesktopSettings, PortalOptions, Settings};
use super::status_notifier::StatusNotifier;
use super::wallpaper::{WallpaperColors, WallpaperOptions};
use crate::app::{Event, TitleBar, TrayIcon, WindowButton};
use crate::system::log;
use kompoz_graphics::Rect;
use kompoz_graphics::material_color::palettes::DynamicTonalPalette;
use kompoz_ui::KompozHost;
use kompoz_ui::media_query::{MediaInfo, PointerPrecision};
use kompoz_ui::window::Notification;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// What a service on another thread calls as it learns something: the platform's loop woken (an
/// [`event_loop::Waker`](super::event_loop::Waker)'s [`as_fn`](super::event_loop::Waker::as_fn)).
pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// The title bar's buttons as GNOME's `button-layout` lays them out ("appmenu:minimize,maximize,close": those
/// before the colon at the start, those after it at the end; what the window manager can't do left out).
pub fn title_bar_from_layout(layout: &str, maximized: bool, can_minimize: bool, can_maximize: bool) -> TitleBar {
    let side = |names: &str| {
        names
            .split(',')
            .filter_map(|name| match name.trim() {
                "close" => Some(WindowButton::Close),
                "minimize" if can_minimize => Some(WindowButton::Minimize),
                "maximize" if can_maximize => Some(WindowButton::Maximize),
                _ => None,
            })
            .collect()
    };
    let (start, end) = layout.split_once(':').unwrap_or((layout, ""));
    TitleBar { start: side(start), end: side(end), maximized }
}

/// The desktop's buttons where it doesn't say: Minimize, Maximize and Close at the end.
const DEFAULT_LAYOUT: &str = "appmenu:minimize,maximize,close";

/// Where each service is and whether it runs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DesktopOptions {
    pub portal: PortalOptions,
    pub atspi: AtspiOptions,
    pub wallpaper: WallpaperOptions,
    /// The session bus the tray icons and notifications go to (none: none shown).
    pub tray: Option<String>,
}

impl DesktopOptions {
    /// As the environment says.
    pub fn from_env() -> DesktopOptions {
        DesktopOptions {
            portal: PortalOptions::from_env(),
            atspi: AtspiOptions::from_env(),
            wallpaper: WallpaperOptions::from_env(),
            tray: super::dbus::session_bus_address(),
        }
    }
}

/// What the services told since they were last asked.
#[derive(Default)]
struct Told {
    settings: AtomicBool,
    wallpaper: AtomicBool,
    accessibility: AtomicBool,
    tray: AtomicBool,
}

/// The desktop's services a Linux platform runs (C++'s DesktopServices): see the module's documentation for the
/// hooks a platform passes on.
pub struct DesktopServices {
    told: Arc<Told>,
    settings: Option<DesktopSettings>,
    /// The settings as the app was last told them.
    known: Settings,
    wallpaper: Option<WallpaperColors>,
    a11y: Option<AtspiBridge>,
    tray: Option<StatusNotifier>,
    focused: bool,
    semantics_dirty: bool,
    last_semantics: Option<i64>,
    /// The session bus connection holding the screen saver off (closing it lets it go).
    screen_saver: Option<Connection>,
    media: MediaDevices,
}

impl Default for DesktopServices {
    fn default() -> Self {
        DesktopServices::new()
    }
}

impl DesktopServices {
    /// None started yet.
    pub fn new() -> DesktopServices {
        DesktopServices {
            told: Arc::new(Told::default()),
            settings: None,
            known: Settings::default(),
            wallpaper: None,
            a11y: None,
            tray: None,
            focused: false,
            semantics_dirty: true,
            last_semantics: None,
            screen_saver: None,
            media: MediaDevices::default(),
        }
    }

    /// Started as the host is made (`Platform::attach`), for the app titled `title`, as the environment says;
    /// `wake` wakes the platform's loop.
    pub fn start(&mut self, title: &str, wake: Notify) {
        self.start_with(title, wake, DesktopOptions::from_env());
    }

    /// Started with these options (a test's buses).
    pub fn start_with(&mut self, title: &str, wake: Notify, options: DesktopOptions) {
        let notify = |flag: fn(&Told) -> &AtomicBool| -> Notify {
            let (told, wake) = (self.told.clone(), wake.clone());
            Arc::new(move || {
                flag(&told).store(true, Ordering::SeqCst);
                wake();
            })
        };
        self.settings = Some(DesktopSettings::new(notify(|t| &t.settings), options.portal));
        self.wallpaper = Some(WallpaperColors::new(self.dark_theme(), notify(|t| &t.wallpaper), options.wallpaper));
        self.a11y = Some(AtspiBridge::new(title, notify(|t| &t.accessibility), options.atspi));
        self.tray = Some(StatusNotifier::new(title, notify(|t| &t.tray), options.tray));
    }

    /// What they told since, as events for the app core: AccentChanged when the accent color changed,
    /// MetricsChanged when the light or dark preference, the window buttons or the text size did, WallpaperChanged,
    /// AccessibilityRequest (an assistive technology's requests, or its wish for the tree), what the user did with
    /// the tray icons (TrayAction, TrayMenuItem).
    pub fn take_events(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        if self.told.settings.swap(false, Ordering::SeqCst)
            && let Some(settings) = &self.settings
        {
            let now = settings.settings();
            if now.accent != self.known.accent {
                events.push(Event::AccentChanged);
            }
            if now.dark != self.known.dark
                || now.button_layout != self.known.button_layout
                || now.text_scaling_factor != self.known.text_scaling_factor
            {
                events.push(Event::MetricsChanged);
            }
            self.known = now;
        }
        if self.told.wallpaper.swap(false, Ordering::SeqCst) {
            events.push(Event::WallpaperChanged);
        }
        if self.told.accessibility.swap(false, Ordering::SeqCst) {
            events.push(Event::AccessibilityRequest);
        }
        if self.told.tray.swap(false, Ordering::SeqCst)
            && let Some(tray) = &self.tray
        {
            events.extend(tray.take_actions());
        }
        events
    }

    /// `Platform::tray_supported`: a StatusNotifierWatcher on the session bus (a tray shows the icons).
    pub fn tray_supported(&self) -> bool {
        self.tray.as_ref().is_some_and(StatusNotifier::available)
    }

    /// `Platform::sync_trays`: the app's tray icons as they are now.
    pub fn sync_trays(&mut self, trays: &[TrayIcon]) {
        if let Some(tray) = &self.tray {
            tray.sync(trays);
        }
    }

    /// `Platform::send_notification`: a notification of the desktop's, for tray icon `tray`.
    pub fn send_notification(&mut self, tray: &TrayIcon, notification: &Notification) {
        if let Some(t) = &self.tray {
            t.notify(tray, notification);
        }
    }

    /// The desktop's accent color, as the app was last told it.
    pub fn accent(&self) -> Option<kompoz_graphics::Color> {
        self.known.accent
    }

    /// Whether the desktop prefers dark (the portal's color-scheme), as the app was last told.
    pub fn dark_theme(&self) -> bool {
        self.known.dark.unwrap_or(false)
    }

    /// GNOME's window buttons' layout ("appmenu:close"); none: the desktop doesn't say.
    pub fn button_layout(&self) -> Option<String> {
        self.known.button_layout.clone()
    }

    /// The system's text size (`text-scaling-factor`); none: the desktop doesn't say. (`KOMPOZ_FONT_SCALE`, where
    /// set, is the platform's to put first.)
    pub fn font_scale(&self) -> Option<f32> {
        self.known.text_scaling_factor
    }

    /// The title bar KompozR draws, its buttons as the desktop lays them out (elsewhere Minimize, Maximize and
    /// Close at the end), those the window manager can do.
    pub fn title_bar(&self, maximized: bool, can_minimize: bool, can_maximize: bool) -> TitleBar {
        let layout = self.button_layout().unwrap_or_else(|| DEFAULT_LAYOUT.to_string());
        title_bar_from_layout(&layout, maximized, can_minimize, can_maximize)
    }

    /// `Platform::metrics_updated`: the wallpaper's picture for the theme now.
    pub fn metrics_updated(&mut self) {
        if let Some(w) = &self.wallpaper {
            w.set_dark_theme(self.dark_theme());
        }
    }

    /// `Platform::handled`: the accent's and the wallpaper's palettes set, assistive technologies' requests
    /// performed, the window's focus kept for the screen readers.
    pub fn handled(&mut self, host: &KompozHost, event: &Event) {
        match event {
            Event::AccentChanged if self.settings.is_some() => {
                host.set_dynamic_tonal_palette(self.known.accent.map(DynamicTonalPalette::from_seed));
            }
            Event::WallpaperChanged => {
                if let Some(w) = &self.wallpaper {
                    host.set_wallpaper_tonal_palette(w.palette());
                }
            }
            Event::AccessibilityRequest => {
                if let Some(a11y) = &self.a11y {
                    for r in a11y.take_requests() {
                        host.perform_semantics_action(r.id, r.action);
                    }
                }
            }
            Event::FocusGained | Event::FocusLost => {
                self.focused = matches!(event, Event::FocusGained);
                self.semantics_dirty = true;
            }
            _ => {}
        }
    }

    /// `Platform::composed`: the semantics tree published to the screen readers as they ask for it (at once when
    /// they ask, else at most every 50 ms, 500 ms while the app is busy), `window` where the window is (points),
    /// `px_to_points` its points per px, `title` its title.
    pub fn composed(
        &mut self,
        host: &KompozHost,
        redraw: bool,
        frame_time: i64,
        window: Rect,
        px_to_points: f32,
        title: &str,
    ) {
        let Some(a11y) = &self.a11y else { return };
        if !a11y.active() {
            return;
        }
        let tree_requested = a11y.take_tree_request();
        self.semantics_dirty |= redraw || tree_requested || host.take_semantics_changed();
        let interval = if host.has_pending_work() { 500_000_000 } else { 50_000_000 };
        if self.semantics_dirty && (tree_requested || self.last_semantics.is_none_or(|t| frame_time - t > interval)) {
            self.last_semantics = Some(frame_time);
            self.semantics_dirty = false;
            let window = AtspiWindow {
                x: window.left,
                y: window.top,
                width: window.width(),
                height: window.height(),
                px_to_points,
                focused: self.focused,
                title: title.to_string(),
            };
            a11y.publish(host.accessibility_tree(), window);
        }
    }

    /// `Platform::wants_frames`: one more frame publishes the tree.
    pub fn wants_frames(&self) -> bool {
        self.a11y.as_ref().is_some_and(AtspiBridge::active) && self.semantics_dirty
    }

    /// `Platform::media`: the pointer and the keyboard as the platform knows them, a microphone and a camera probed.
    pub fn media(&mut self, probe: bool, pointer_precision: PointerPrecision, physical_keyboard: bool) -> MediaInfo {
        self.media.info(probe, pointer_precision, physical_keyboard)
    }

    /// The screen saver held off (`org.freedesktop.ScreenSaver.Inhibit`, for as long as the connection that asked
    /// is open) for the app named `app`, or let go; where the window system can't (X11).
    pub fn keep_screen_on(&mut self, on: bool, app: &str) {
        if on == self.screen_saver.is_some() {
            return;
        }
        if !on {
            self.screen_saver = None; // (closing the connection lets the screen saver go)
            return;
        }
        let Ok(mut bus) = Connection::session() else { return };
        let inhibited = bus.call_method(
            "org.freedesktop.ScreenSaver",
            "/org/freedesktop/ScreenSaver",
            "org.freedesktop.ScreenSaver",
            "Inhibit",
            vec![app.into(), "keepScreenOn".into()],
            Duration::from_secs(1),
        );
        match inhibited {
            Ok(_) => self.screen_saver = Some(bus),
            Err(e) => log(&format!("kompoz: the screen saver can't be held off ({e})")),
        }
    }

    /// Their threads ended (before the loop they wake goes), the screen saver let go; at once, a wallpaper's palette
    /// still being worked out told to stop and not waited for (it wakes nothing).
    pub fn stop(&mut self) {
        self.keep_screen_on(false, "");
        self.tray = None;
        self.a11y = None;
        self.wallpaper = None;
        self.settings = None;
    }
}

/// Opens `uri` with the desktop's browser, or the app for its scheme (`xdg-open`), its process waited for on a
/// thread of its own (C++'s `openUri`).
pub fn open_uri(uri: &str) {
    match std::process::Command::new("xdg-open").arg(uri).spawn() {
        Ok(mut child) => {
            let spawned = std::thread::Builder::new().name("kompoz-xdg-open".into()).spawn(move || {
                let _ = child.wait();
            });
            if spawned.is_err() {
                log("kompoz: can't wait for xdg-open");
            }
        }
        Err(_) => log(&format!("kompoz: can't open {uri}")),
    }
}

/// The local files a `text/uri-list` names (C++'s `filesInUriList`): kompoz-ui's.
pub use kompoz_ui::clipboard::files_in_uri_list;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_lists_name_their_local_files() {
        let list = "# a comment\r\nfile:///home/kasim/G%C3%B6r%C3%BCnt%C3%BC.png\r\nhttps://example.org/x\r\n\
                    file://host/tmp/a%20b.txt\nfile:///bad%2\r\n";
        assert_eq!(files_in_uri_list(list), ["/home/kasim/Görüntü.png", "/tmp/a b.txt", "/bad%2"]);
        assert!(files_in_uri_list("").is_empty());
    }
}
