//! The desktop's settings through the XDG settings portal (C++'s desktop_accent.cpp; `org.freedesktop.portal.Settings`,
//! which GNOME and KDE fill in): the accent color (`org.freedesktop.appearance` `accent-color`, `(ddd)`), the light
//! or dark preference (`color-scheme`, `u`: 0 none, 1 dark, 2 light), GNOME's window buttons
//! (`org.gnome.desktop.wm.preferences` `button-layout`, `s`), and the system's text size
//! (`org.gnome.desktop.interface` `text-scaling-factor`, `d`: the font scale). Each is read once (`ReadOne`, else an
//! older portal's `Read`) and followed as it changes (`SettingChanged`), on a thread of its own with its own
//! connection to the session bus; it notifies as a value changes.
//!
//! `KOMPOZ_NO_DESKTOP_ACCENT`: the accent color isn't read (the rest is).

use super::dbus::{Connection, MatchRule, Message, Value};
use super::desktop::Notify;
use super::sys;
use kompoz_graphics::Color;
use std::os::fd::{AsFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const SETTINGS: &str = "org.freedesktop.portal.Settings";
const APPEARANCE: &str = "org.freedesktop.appearance";
const ACCENT: &str = "accent-color";
const SCHEME: &str = "color-scheme";
const WM: &str = "org.gnome.desktop.wm.preferences";
const LAYOUT: &str = "button-layout";
const INTERFACE: &str = "org.gnome.desktop.interface";
const TEXT_SCALE: &str = "text-scaling-factor";

/// How long the portal is waited for, a value.
const READ_TIMEOUT: Duration = Duration::from_secs(1);

/// What the desktop said, as it is now (none: it doesn't say).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Settings {
    /// The accent color; none: the desktop has none (or doesn't say, or `KOMPOZ_NO_DESKTOP_ACCENT`).
    pub accent: Option<Color>,
    /// Whether it prefers dark; none: no preference.
    pub dark: Option<bool>,
    /// The window buttons' layout as GNOME writes it ("appmenu:close": those before the colon on the left).
    pub button_layout: Option<String>,
    /// The system's text size (1: as designed).
    pub text_scaling_factor: Option<f32>,
}

/// Where the portal is, and what is read of it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PortalOptions {
    /// The session bus's address; none: there is none.
    pub address: Option<String>,
    /// Whether the accent color is read.
    pub accent: bool,
}

impl PortalOptions {
    /// The session bus's, the accent read unless `KOMPOZ_NO_DESKTOP_ACCENT` is set.
    pub fn from_env() -> PortalOptions {
        PortalOptions {
            address: super::dbus::session_bus_address(),
            accent: std::env::var_os("KOMPOZ_NO_DESKTOP_ACCENT").is_none(),
        }
    }
}

struct Shared {
    running: AtomicBool,
    settings: Mutex<Settings>,
    notify: Notify,
    accent_wanted: bool,
}

/// The desktop's settings, read and followed on a thread of its own (C++'s DesktopAccent).
pub struct DesktopSettings {
    shared: Arc<Shared>,
    /// What wakes the thread to stop (an eventfd), and the thread.
    thread: Option<(Arc<OwnedFd>, JoinHandle<()>)>,
}

impl DesktopSettings {
    /// Starts reading (nothing happens where there is no session bus or no portal); `notify` is called on the
    /// thread as a value changes.
    pub fn new(notify: Notify, options: PortalOptions) -> DesktopSettings {
        let shared = Arc::new(Shared {
            running: AtomicBool::new(true),
            settings: Mutex::new(Settings::default()),
            notify,
            accent_wanted: options.accent,
        });
        let thread = options.address.and_then(|address| {
            let stop = Arc::new(sys::eventfd().ok()?);
            let (shared, wake) = (shared.clone(), stop.clone());
            let thread = std::thread::Builder::new()
                .name("kompoz-portal".into())
                .spawn(move || {
                    if let Ok(bus) = Connection::open(&address) {
                        run(&shared, bus, &wake);
                    }
                })
                .ok()?;
            Some((stop, thread))
        });
        DesktopSettings { shared, thread }
    }

    /// Whether it was started: it notifies when a value is learned or changes.
    pub fn listening(&self) -> bool {
        self.thread.as_ref().is_some_and(|(_, t)| !t.is_finished())
    }

    /// All of it, as it is now.
    pub fn settings(&self) -> Settings {
        self.shared.settings.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn accent(&self) -> Option<Color> {
        self.settings().accent
    }

    pub fn dark_scheme(&self) -> Option<bool> {
        self.settings().dark
    }

    pub fn button_layout(&self) -> Option<String> {
        self.settings().button_layout
    }

    pub fn text_scaling_factor(&self) -> Option<f32> {
        self.settings().text_scaling_factor
    }
}

impl Drop for DesktopSettings {
    fn drop(&mut self) {
        self.shared.running.store(false, Ordering::SeqCst);
        if let Some((stop, thread)) = self.thread.take() {
            let _ = sys::eventfd_write(stop.as_fd(), 1);
            let _ = thread.join();
        }
    }
}

/// The thread: the values read now, then followed until stopped (or the bus goes).
fn run(shared: &Shared, mut bus: Connection, stop: &OwnedFd) {
    for namespace in [APPEARANCE, WM, INTERFACE] {
        let rule = MatchRule::signal().interface(SETTINGS).member("SettingChanged").arg(0, namespace);
        let _ = bus.add_match(&rule);
    }
    if shared.accent_wanted {
        read_now(shared, &mut bus, APPEARANCE, ACCENT);
    }
    read_now(shared, &mut bus, APPEARANCE, SCHEME);
    read_now(shared, &mut bus, WM, LAYOUT);
    read_now(shared, &mut bus, INTERFACE, TEXT_SCALE);
    while shared.running.load(Ordering::SeqCst) {
        match bus.read(None, Some(stop.as_fd())) {
            Ok(Some(m)) if m.is_signal(SETTINGS, "SettingChanged") => {
                if let (Some(namespace), Some(key), Some(value)) =
                    (m.arg(0).and_then(Value::as_str), m.arg(1).and_then(Value::as_str), m.arg(2))
                {
                    take(shared, namespace, key, value);
                }
            }
            Ok(Some(call)) if call.kind == super::dbus::MessageType::MethodCall => {
                let _ = bus.answer_peer(&call);
            }
            Ok(_) => {}
            Err(_) => break, // (the bus went away)
        }
    }
}

/// A key's value as it is now: `ReadOne` (portal version 2 on), else `Read` (which wraps it in one more variant).
fn read_now(shared: &Shared, bus: &mut Connection, namespace: &str, key: &str) {
    for method in ["ReadOne", "Read"] {
        let call = Message::method_call(Some(PORTAL), PATH, Some(SETTINGS), method)
            .with_body(vec![Value::str(namespace), Value::str(key)]);
        match bus.call(call, READ_TIMEOUT) {
            Ok(reply) => {
                if let Some(value) = reply.arg(0) {
                    take(shared, namespace, key, value);
                }
                return;
            }
            // No such method (an older portal), or no such key (then Read hasn't it either).
            Err(e) if e.remote_name().is_some() => continue,
            Err(_) => return,
        }
    }
}

/// A key's value, as it is now or as it changed: kept, and told if it changed.
fn take(shared: &Shared, namespace: &str, key: &str, value: &Value) {
    let value = value.unwrap_variants();
    let mut settings = shared.settings.lock().unwrap_or_else(|e| e.into_inner());
    let before = settings.clone();
    match (namespace, key) {
        (APPEARANCE, ACCENT) if shared.accent_wanted => match accent_in(value) {
            Some(accent) => settings.accent = accent,
            None => return,
        },
        (APPEARANCE, SCHEME) => match value.as_u32() {
            Some(scheme) => settings.dark = scheme_dark(scheme),
            None => return,
        },
        (WM, LAYOUT) => match value.as_str() {
            Some(layout) => settings.button_layout = Some(layout.to_string()),
            None => return,
        },
        (INTERFACE, TEXT_SCALE) => match value.as_f64() {
            Some(factor) => settings.text_scaling_factor = (factor > 0.0).then_some(factor as f32),
            None => return,
        },
        _ => return,
    }
    let changed = *settings != before;
    drop(settings);
    if changed {
        (shared.notify)();
    }
}

/// An accent color from a value: `(ddd)`, red, green and blue in 0..1. Out of range: the desktop has none (Some of
/// none); not a color at all: none.
pub fn accent_in(value: &Value) -> Option<Option<Color>> {
    let fields = value.unwrap_variants().fields()?;
    let [r, g, b] = fields else { return None };
    let rgb = [r.as_f64()?, g.as_f64()?, b.as_f64()?];
    if rgb.iter().any(|c| !(0.0..=1.0).contains(c)) {
        return Some(None);
    }
    Some(Some(Color::new(rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, 1.0)))
}

/// Whether a `color-scheme` value prefers dark (none: no preference).
pub fn scheme_dark(scheme: u32) -> Option<bool> {
    match scheme {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_read_as_the_portal_writes_them() {
        let rgb = |r, g, b| Value::Struct(vec![Value::Double(r), Value::Double(g), Value::Double(b)]);
        assert_eq!(accent_in(&rgb(1.0, 0.5, 0.0)), Some(Some(Color::new(1.0, 0.5, 0.0, 1.0))));
        // Read's: one more variant.
        assert_eq!(
            accent_in(&Value::variant(Value::variant(rgb(0.0, 0.0, 1.0)))),
            Some(Some(Color::new(0.0, 0.0, 1.0, 1.0)))
        );
        assert_eq!(accent_in(&rgb(-1.0, -1.0, -1.0)), Some(None)); // (what GNOME says when it has none)
        assert_eq!(accent_in(&Value::Uint32(1)), None);
        assert_eq!(accent_in(&Value::Struct(vec![Value::Double(1.0), Value::Double(1.0)])), None);
        assert_eq!((scheme_dark(0), scheme_dark(1), scheme_dark(2)), (None, Some(true), Some(false)));
    }
}
