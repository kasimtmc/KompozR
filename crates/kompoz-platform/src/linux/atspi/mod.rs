//! Screen readers (Orca) and other assistive technologies on Linux read the app through AT-SPI (C++'s
//! atspi_bridge.cpp): the semantics tree of each of its windows (each window's host's
//! [`KompozHost::accessibility_tree`](kompoz_ui::KompozHost)) served on the accessibility bus by a thread of its
//! own, from the trees last published ([`tree`]: one accessible application, a frame for each window under it, as
//! GTK's and Qt's apps have), the app embedded in the AT-SPI registry's desktop. It starts when the desktop's
//! accessibility is on (`org.a11y.Status`'s IsEnabled or ScreenReaderEnabled, also once it is turned on later);
//! `KOMPOZ_ATSPI=1` starts it anyway, `KOMPOZ_ATSPI=0` never.
//! What an assistive technology asks the app to do (click, focus, set a slider, a text) is kept as requests and
//! notified ([`serve`]), for the window whose node it names; what changes in a tree (focus, states, names, values,
//! texts, children, live regions, the window's activation) is told as AT-SPI events, a window opened (its frame
//! added to the application, `window:create`) and closed (`window:destroy`, its frame and subtree taken away) too.

pub mod serve;
pub mod tree;

pub use serve::Request;
pub use tree::Window;

use super::dbus::{Connection, MatchRule, Message, MessageType, Value, errors};
use super::desktop::Notify;
use super::sys;
use crate::accessibility::Snapshot;
use kompoz_ui::semantics::AccessibilityNode;
use serve::{Bus, PROPERTIES, Served};
use std::collections::HashMap;
use std::os::fd::{AsFd, OwnedFd};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;
use tree::{Object, ROOT_PATH};

/// How long the bus's services are waited for.
const CALL_TIMEOUT: Duration = Duration::from_secs(2);
const CACHE_PATH: &str = "/org/a11y/atspi/cache";

/// Where the buses are, and whether the bridge is forced on or off.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct AtspiOptions {
    /// The session bus's address (where the accessibility bus's launcher is); none: there is none.
    pub session_address: Option<String>,
    /// The accessibility bus's address as the environment says (`AT_SPI_BUS_ADDRESS`); none: the launcher's.
    pub a11y_address: Option<String>,
    /// `KOMPOZ_ATSPI`: on (whatever the desktop says), off (never), or as the desktop says (none).
    pub forced: Option<bool>,
}

impl AtspiOptions {
    /// As the environment says.
    pub fn from_env() -> AtspiOptions {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        AtspiOptions {
            session_address: super::dbus::session_bus_address(),
            a11y_address: var("AT_SPI_BUS_ADDRESS"),
            forced: match var("KOMPOZ_ATSPI").as_deref() {
                Some("1") => Some(true),
                Some("0") => Some(false),
                _ => None,
            },
        }
    }
}

struct Shared {
    app_name: String,
    notify: Notify,
    running: AtomicBool,
    connected: AtomicBool,
    /// Bumped as the bridge connects: each window's tree is published again ([`AtspiBridge::take_tree_request`]),
    /// and the generation each window last saw.
    generation: AtomicU64,
    seen: Mutex<HashMap<u64, u64>>,
    served: Mutex<Served>,
}

impl Shared {
    fn served(&self) -> MutexGuard<'_, Served> {
        self.served.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The AT-SPI bridge (C++'s AtspiBridge): its thread, and what the UI thread publishes to it and takes from it.
pub struct AtspiBridge {
    shared: Arc<Shared>,
    /// What wakes the bus's thread (an eventfd: a tree published, stopping), and the thread.
    thread: Option<(Arc<OwnedFd>, JoinHandle<()>)>,
}

impl AtspiBridge {
    /// Starts it for the app named `app_name` (nothing happens where there is no accessibility bus, or until the
    /// desktop's accessibility is turned on); `notify` is called on its thread as it wants the tree or has requests.
    pub fn new(app_name: &str, notify: Notify, options: AtspiOptions) -> AtspiBridge {
        let shared = Arc::new(Shared {
            app_name: app_name.to_string(),
            notify,
            running: AtomicBool::new(true),
            connected: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            seen: Mutex::new(HashMap::new()),
            served: Mutex::new(Served::default()),
        });
        let thread = (options.forced != Some(false))
            .then(|| {
                let wake = Arc::new(sys::eventfd().ok()?);
                let (shared, their_wake) = (shared.clone(), wake.clone());
                let thread = std::thread::Builder::new()
                    .name("kompoz-atspi".into())
                    .spawn(move || {
                        run(&shared, &their_wake, &options);
                        shared.connected.store(false, Ordering::SeqCst);
                    })
                    .ok()?;
                Some((wake, thread))
            })
            .flatten();
        AtspiBridge { shared, thread }
    }

    /// Whether it was started: it will notify.
    pub fn listening(&self) -> bool {
        self.thread.is_some()
    }

    /// Connected to the accessibility bus: the tree is worth publishing.
    pub fn active(&self) -> bool {
        self.shared.connected.load(Ordering::SeqCst)
    }

    /// Whether window `window` (its key) is to publish its tree now even if nothing changed: the bridge connected
    /// (or connected again) since the window last asked.
    pub fn take_tree_request(&self, window: u64) -> bool {
        let generation = self.shared.generation.load(Ordering::SeqCst);
        let mut seen = self.shared.seen.lock().unwrap_or_else(|e| e.into_inner());
        let before = seen.insert(window, generation);
        generation > 0 && before != Some(generation)
    }

    /// Window `window`'s tree (its key: the platform's `WindowId`'s number) as it is now (the UI thread's): a new
    /// window told as the application's child, what changed since its last one told to the listeners.
    pub fn publish(&self, window: u64, nodes: Vec<AccessibilityNode>, frame: Window) {
        let title = frame.title.clone();
        let next = Snapshot::new(nodes, frame);
        {
            let mut served = self.shared.served();
            let active = self.active();
            let new = served.tree.snapshot(window).is_none();
            let old = served.tree.set(window, next);
            if active {
                let mut events = Vec::new();
                if new {
                    let index = served.tree.windows().len() - 1;
                    events.extend(tree::window_added(window, index, &title));
                }
                let next = served.tree.snapshot(window).expect("just set");
                events.extend(tree::compare(window, &old, next));
                served.events.extend(events);
            }
        }
        self.wake();
    }

    /// Window `window` gone (closed, or hidden): its frame and its subtree taken away from the application, told;
    /// its requests not yet taken dropped.
    pub fn remove_window(&self, window: u64) {
        {
            let mut served = self.shared.served();
            let Some((index, old)) = served.tree.remove(window) else { return };
            served.requests.retain(|r| r.window != window);
            if self.active() {
                let events = tree::window_removed(window, index, &old);
                served.events.extend(events);
            }
        }
        self.shared.seen.lock().unwrap_or_else(|e| e.into_inner()).remove(&window);
        self.wake();
    }

    /// What assistive technologies asked of window `window`'s nodes since, in order.
    pub fn take_requests(&self, window: u64) -> Vec<Request> {
        let mut served = self.shared.served();
        let (mine, others) = std::mem::take(&mut served.requests).into_iter().partition(|r| r.window == window);
        served.requests = others;
        mine
    }

    fn wake(&self) {
        if let Some((wake, _)) = &self.thread {
            let _ = sys::eventfd_write(wake.as_fd(), 1);
        }
    }
}

impl Drop for AtspiBridge {
    fn drop(&mut self) {
        self.shared.running.store(false, Ordering::SeqCst);
        self.wake();
        if let Some((_, thread)) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The bus's thread: waits for the desktop's accessibility (unless forced), connects, then serves until stopped.
fn run(shared: &Shared, wake: &OwnedFd, options: &AtspiOptions) {
    let running = || shared.running.load(Ordering::SeqCst);
    let mut session = options.session_address.as_deref().and_then(|a| Connection::open(a).ok());
    if options.forced != Some(true) {
        let Some(s) = session.as_mut() else { return };
        let rule = MatchRule::signal().interface(PROPERTIES).member("PropertiesChanged").path("/org/a11y/bus");
        let _ = s.add_match(&rule);
        let mut enabled = status_enabled(s);
        while !enabled {
            match s.read(None, Some(wake.as_fd())) {
                Ok(Some(m)) if m.is_signal(PROPERTIES, "PropertiesChanged") => enabled = status_enabled(s),
                Ok(Some(m)) => {
                    let _ = s.answer_peer(&m);
                }
                Ok(None) if !running() => return,
                Ok(None) => {}
                Err(_) => return,
            }
        }
    }
    if !running() {
        return;
    }
    let Some((mut bus, names)) = connect(session.as_mut(), options) else { return };
    drop(session); // (not needed any more)
    shared.connected.store(true, Ordering::SeqCst);
    shared.generation.fetch_add(1, Ordering::SeqCst);
    (shared.notify)(); // to publish the tree
    while running() {
        let events = std::mem::take(&mut shared.served().events);
        for e in &events {
            if bus.send(&serve::signal(&names, e)).is_err() {
                return;
            }
        }
        match bus.read(None, Some(wake.as_fd())) {
            Ok(Some(m)) if m.kind == MessageType::MethodCall => {
                if handle(shared, &mut bus, &names, &m).is_err() {
                    return;
                }
            }
            Ok(_) => {}
            Err(_) => return, // (the bus went away)
        }
    }
}

/// Whether the desktop's accessibility is on (what Qt asks too).
fn status_enabled(session: &mut Connection) -> bool {
    for name in ["IsEnabled", "ScreenReaderEnabled"] {
        match session.get_property("org.a11y.Bus", "/org/a11y/bus", "org.a11y.Status", name, CALL_TIMEOUT) {
            Ok(v) if v.unwrap_variants().as_bool() == Some(true) => return true,
            Ok(_) => {}
            Err(_) => return false, // no accessibility bus here
        }
    }
    false
}

/// A connection to the accessibility bus (where the environment or the launcher says), embedded in the registry's
/// desktop, which tells assistive technologies the app is there.
fn connect(session: Option<&mut Connection>, options: &AtspiOptions) -> Option<(Connection, Bus)> {
    let address = match &options.a11y_address {
        Some(a) => a.clone(),
        None => {
            let reply = session?.call_method(
                "org.a11y.Bus",
                "/org/a11y/bus",
                "org.a11y.Bus",
                "GetAddress",
                vec![],
                CALL_TIMEOUT,
            );
            reply.ok()?.first().and_then(Value::as_str)?.to_string()
        }
    };
    let mut bus = Connection::open(&address).ok()?;
    let mut names = Bus {
        unique_name: bus.unique_name().to_string(),
        desktop_name: String::new(),
        desktop_path: tree::NULL_PATH.to_string(),
    };
    let me = Value::Struct(vec![Value::str(names.unique_name.as_str()), Value::path(ROOT_PATH)]);
    let embed =
        bus.call_method("org.a11y.atspi.Registry", ROOT_PATH, "org.a11y.atspi.Socket", "Embed", vec![me], CALL_TIMEOUT);
    if let Some([name, path]) = embed.ok().as_ref().and_then(|r| r.first()).and_then(Value::fields)
        && let (Some(name), Value::ObjectPath(path)) = (name.as_str(), path)
    {
        names.desktop_name = name.to_string();
        names.desktop_path = path.clone();
    }
    Some((bus, names))
}

/// A call answered: from the tree (under the mutex), the requests it made notified.
fn handle(shared: &Shared, bus: &mut Connection, names: &Bus, call: &Message) -> Result<(), super::dbus::Error> {
    if bus.answer_peer(call)? {
        return Ok(());
    }
    let path = call.path.as_deref().unwrap_or("");
    if path == CACHE_PATH && call.member.as_deref() == Some("GetItems") {
        return bus.reply(call, vec![serve::cache_items()]);
    }
    let (reply, requested) = {
        let mut served = shared.served();
        let before = served.requests.len();
        let reply = match Object::at(path).filter(|&o| served.tree.exists(o)) {
            None => Err((errors::UNKNOWN_OBJECT, "No such object")),
            Some(o) => serve::answer(&mut served, &shared.app_name, names, call, o)
                .ok_or((errors::UNKNOWN_METHOD, "No such method")),
        };
        (reply, served.requests.len() > before)
    };
    if requested {
        (shared.notify)(); // the main loop woken
    }
    match reply {
        Ok(body) => bus.reply(call, body),
        Err((name, text)) => bus.reply_error(call, name, text),
    }
}
