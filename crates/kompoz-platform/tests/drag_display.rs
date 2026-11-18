//! Drags of the app's own out to other apps (WP5b) on real display servers (`KOMPOZ_DISPLAY_TESTS=1`; skipped
//! otherwise): an app (this test run again) with a drag source — text, a file, a type of its own and local state,
//! Copy and Move allowed — and a drop target of its own, dragged by input the test makes, to a receiver written here
//! (another client): the data it reads (each type it asks for), the action the drag ends with on both sides, a drop
//! on nothing and Escape ending it untaken, and the drag brought back over the app's own window and dropped on its
//! target (what it carries the app's own: its local state too).
//!
//! - Wayland: on KDE's kwin_wayland with a virtual output, on a session bus and configuration of its own (kwin
//!   writes its settings: never into the user's; its window rules place the two windows side by side, undecorated),
//!   the input made through kwin's fake input (`org_kde_kwin_fake_input`, which kwin offers any client); also at
//!   scale 2, the drag's icon drawn at its pixels. Weston's headless backend has no input a client can make: there
//!   (`KOMPOZ_TEST_COMPOSITOR=weston`, as CI runs) the Wayland runs are skipped, said so.
//! - X11: on the Xwayland of a kwin of the test's own, the input made through kwin's fake input (Xwayland's XTEST
//!   goes nowhere without an EI portal); and with `KOMPOZ_X11_XTEST=1` on the X server `DISPLAY` names (Xvfb, as CI
//!   runs: never a desktop's own server), the input made through XTEST (libXtst). The receiver an override-redirect
//!   window whose `XdndProxy` takes the messages (version 5); the drag's icon window seen following the pointer;
//!   Ctrl changing the action asked for.

#![allow(non_snake_case)]

use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_platform::App;
use kompoz_runtime::*;
use kompoz_ui::Modifier;
use kompoz_ui::draganddrop::{DragAndDropEvent, DragAndDropTarget, DragAndDropTransferAction, DragAndDropTransferData};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

/// The child's mark: it runs the app.
const CHILD: &str = "KOMPOZ_DRAG_TEST_CHILD";
const TEXT: &str = "Merhaba, sürükle bırak!";
const FILE: &str = "/tmp/kompoz sürükle/ç.txt";
const OWN_TYPE: &str = "application/x-kompoz-test";

/// What the app's drags carry.
fn transfer() -> DragAndDropTransferData {
    DragAndDropTransferData {
        mime_types: vec!["text/uri-list".into(), "text/plain".into(), OWN_TYPE.into()],
        text: TEXT.into(),
        files: vec![FILE.into()],
        local_state: Some(Rc::new("yerel".to_string())),
        supported_actions: vec![DragAndDropTransferAction::Copy, DragAndDropTransferAction::Move],
        on_transfer_completed: Some(Rc::new(|action| eprintln!("kompoz-drag: completed {action:?}"))),
        ..Default::default()
    }
}

/// The app's own target: what is dropped on it logged.
struct AppTarget;

impl DragAndDropTarget for AppTarget {
    fn on_drop(&self, e: &DragAndDropEvent) -> bool {
        let local = e.local_state::<String>().cloned().unwrap_or_default();
        eprintln!("kompoz-drag: dropped in the app: {} | {local} | {}", e.text(), e.mime_types().join(","));
        true
    }
}

/// The app: a source (120 × 80 dp at 20, 20) and a target (200 × 120 dp at 20, 140), a 360 × 300 dp window.
#[kompozable]
fn DragApp() {
    Column(
        modifier = Modifier.fill_max_size().background(Color::White).padding(20.dp()),
        content = || {
            Box(modifier = Modifier
                .size_wh(120.dp(), 80.dp())
                .background(Color::from_argb(0xff3f51b5))
                .drag_and_drop_source(|_| Some(transfer())));
            Spacer(Modifier.height(40.dp()));
            let target: Rc<dyn DragAndDropTarget> = remember(|| Rc::new(AppTarget) as Rc<dyn DragAndDropTarget>);
            Box(modifier = Modifier
                .size_wh(200.dp(), 120.dp())
                .background(Color::from_argb(0xffe0e0e0))
                .drag_and_drop_target(|_: &DragAndDropEvent| true, target));
        },
    );
}

/// Where things are, in the app's window's points (its top left at the screen's on X11 as found, at 0, 0 on
/// Wayland): the source's middle, a point past the window, the app's target.
const SOURCE: (i32, i32) = (80, 60);
const TARGET: (i32, i32) = (120, 200);
/// The receiver's window: at 400, 0, 300 × 300.
const RECEIVER: (i32, i32, i32, i32) = (400, 0, 300, 300);

/// A key the test presses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TestKey {
    Escape,
    Ctrl,
}

/// What makes the input: kwin's fake input, or XTEST.
trait Input {
    fn move_to(&mut self, x: i32, y: i32);
    fn button(&mut self, pressed: bool);
    fn key(&mut self, key: TestKey, pressed: bool);
    /// The display's events for `time` (a client's), or a wait.
    fn pump(&mut self, time: Duration);
}

/// What a scenario's pointer does: from the source, through `path` (screen points), let go (Escape pressed first
/// at the last point if `escape`).
struct Drag {
    path: Vec<(i32, i32)>,
    escape: bool,
}

/// The way out of the window to the right, then `to`.
fn path_out(origin: (i32, i32), to: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let (ox, oy) = origin;
    let mut path: Vec<(i32, i32)> =
        [(90, 60), (105, 62), (130, 64), (180, 64), (250, 64), (330, 64), (355, 64), (372, 66)]
            .into_iter()
            .map(|(x, y)| (ox + x, oy + y))
            .collect();
    path.extend_from_slice(to);
    path
}

/// The app's log (its stderr) as it is now.
fn log(work: &Path) -> String {
    std::fs::read_to_string(work.join("app.err")).unwrap_or_default()
}

/// Waits until the app's log has `n` lines with `what` (ten seconds at most), meanwhile `pump`ing; whether it did.
fn wait_for(work: &Path, what: &str, n: usize, pump: &mut dyn FnMut(Duration)) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if log(work).lines().filter(|l| l.contains(what)).count() >= n {
            return true;
        }
        pump(Duration::from_millis(20));
    }
    false
}

/// The `n`th "completed" line's action.
fn completed(work: &Path, n: usize) -> String {
    log(work)
        .lines()
        .filter_map(|l| l.strip_prefix("kompoz-drag: completed "))
        .nth(n)
        .map(str::to_string)
        .unwrap_or_else(|| panic!("no completion {n} in:\n{}", tail(&log(work))))
}

/// A long log's end.
fn tail(s: &str) -> String {
    let lines: Vec<&str> =
        s.lines().filter(|l| !l.contains("wl_") && !l.contains("wp_") && !l.contains("xdg_")).collect();
    lines[lines.len().saturating_sub(40)..].join("\n")
}

/// Mesa's lavapipe's ICD file.
fn lavapipe_icd() -> String {
    let dir = Path::new("/usr/share/vulkan/icd.d");
    ["lvp_icd.json".to_string(), format!("lvp_icd.{}.json", std::env::consts::ARCH)]
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.exists())
        .map(|path| path.display().to_string())
        .expect("lavapipe (Mesa's lvp_icd.json) for the display tests")
}

/// The app started on a display (`env`), its stderr into `work`/app.err; once it drew its first frame.
fn start_app(work: &Path, env: &[(&str, &str)], pump: &mut dyn FnMut(Duration)) -> AppProcess {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.env(CHILD, "1")
        .env("VK_DRIVER_FILES", lavapipe_icd())
        .env("KOMPOZ_NO_WALLPAPER_COLORS", "1")
        .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent")
        .env("XDG_CACHE_HOME", work.join("cache"))
        .env_remove("KOMPOZ_HEADLESS")
        .env_remove("KOMPOZ_SCRIPT")
        .env_remove("KOMPOZ_PLATFORM")
        .env_remove("WAYLAND_DEBUG");
    for (k, v) in env {
        c.env(k, v);
    }
    c.stdout(Stdio::null()).stderr(std::fs::File::create(work.join("app.err")).unwrap());
    let child = c.spawn().expect("the app starts");
    let app = AppProcess(child);
    assert!(wait_for(work, "kompoz: renderer:", 1, pump), "the app didn't start drawing:\n{}", tail(&log(work)));
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(800) {
        pump(Duration::from_millis(20)); // (its first frames shown, its window placed)
    }
    app
}

/// The app's process: ended (SIGTERM: it quits) when dropped.
struct AppProcess(Child);

impl Drop for AppProcess {
    fn drop(&mut self) {
        let _ = Command::new("kill").args(["-TERM", &self.0.id().to_string()]).status();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(10) {
            if self.0.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Wayland

mod wayland {
    use super::*;
    use kompoz_platform::linux::sys::{self, Mmap, PollFd};
    use kompoz_platform::linux::wayland::ffi::{Arg, Connection, Interface, Message, Object, ObjectId, Proxy};
    use kompoz_platform::linux::wayland::generated::{self as g, *};
    use std::collections::HashMap;
    use std::io::Read;
    use std::os::fd::{AsFd, AsRawFd};

    // kwin's fake input (plasma-wayland-protocols' fake-input.xml, its requests up to version 4).
    static NONE_1: [Option<&Interface>; 1] = [None];
    static NONE_2: [Option<&Interface>; 2] = [None, None];
    static NONE_3: [Option<&Interface>; 3] = [None, None, None];
    static FAKE_INPUT_REQUESTS: [Message; 11] = [
        Message::new(c"authenticate", c"ss", &NONE_2),
        Message::new(c"pointer_motion", c"ff", &NONE_2),
        Message::new(c"button", c"uu", &NONE_2),
        Message::new(c"axis", c"uf", &NONE_2),
        Message::new(c"touch_down", c"2uff", &NONE_3),
        Message::new(c"touch_motion", c"2uff", &NONE_3),
        Message::new(c"touch_up", c"2u", &NONE_1),
        Message::new(c"touch_cancel", c"2", &[]),
        Message::new(c"touch_frame", c"2", &[]),
        Message::new(c"pointer_motion_absolute", c"3ff", &NONE_2),
        Message::new(c"keyboard_key", c"4uu", &NONE_2),
    ];
    static FAKE_INPUT_EVENTS: [Message; 0] = [];
    static FAKE_INPUT: Interface =
        Interface::new(c"org_kde_kwin_fake_input", 4, &FAKE_INPUT_REQUESTS, &FAKE_INPUT_EVENTS, None);

    /// kwin's fake input device.
    struct FakeInput(Proxy);

    impl Object for FakeInput {
        const INTERFACE: &'static Interface = &FAKE_INPUT;
        fn from_proxy(proxy: Proxy) -> Self {
            FakeInput(proxy)
        }
        fn proxy(&self) -> &Proxy {
            &self.0
        }
    }

    const BTN_LEFT: u32 = 0x110;
    const KEY_ESC: u32 = 1;
    const KEY_LEFTCTRL: u32 = 29;
    type Action = WlDataDeviceManagerDndAction;

    /// What the receiver read from a drop.
    #[derive(Debug, Default)]
    pub struct Received {
        pub types: Vec<String>,
        pub uri_list: String,
        pub text: String,
        pub own_type: String,
        pub action: u32,
    }

    /// The receiving client (a 300 × 300 window taking drops: copy or move, preferring `preferred`) and the input.
    pub struct Receiver {
        fake: Option<FakeInput>,
        offers: HashMap<ObjectId, (WlDataOffer, Vec<String>, u32)>,
        current: Option<ObjectId>,
        pub preferred: Action,
        pub drops: Vec<Received>,
        pub enters: usize,
        pub leaves: usize,
        // (Last of them, the objects; the registry last.)
        buffer: Option<WlBuffer>,
        device: Option<WlDataDevice>,
        toplevel: Option<XdgToplevel>,
        xdg_surface: Option<XdgSurface>,
        surface: Option<WlSurface>,
        compositor: Option<WlCompositor>,
        wm_base: Option<XdgWmBase>,
        shm: Option<WlShm>,
        seat: Option<WlSeat>,
        manager: Option<WlDataDeviceManager>,
        registry: Option<WlRegistry>,
        connection: Connection,
    }

    impl Receiver {
        /// Connected to `socket`, the fake input authenticated; its window shown if `window` (else it is only the
        /// input).
        pub fn connect(socket: &str, window: bool) -> Receiver {
            // SAFETY: the test's main thread, before anything else reads the environment.
            unsafe { std::env::set_var("WAYLAND_DISPLAY", socket) };
            let connection = Connection::connect().expect("the compositor");
            let mut r = Receiver {
                registry: connection.display().get_registry(),
                connection,
                fake: None,
                offers: HashMap::new(),
                current: None,
                preferred: Action::COPY,
                drops: Vec::new(),
                enters: 0,
                leaves: 0,
                buffer: None,
                device: None,
                toplevel: None,
                xdg_surface: None,
                surface: None,
                compositor: None,
                wm_base: None,
                shm: None,
                seat: None,
                manager: None,
            };
            r.roundtrip();
            r.roundtrip();
            let fake = r.fake.as_ref().expect("kwin's fake input (org_kde_kwin_fake_input)");
            fake.0.send(0, &[Arg::Str(Some(c"KompozR drag test")), Arg::Str(Some(c"drives a drag"))]);
            if !window {
                r.pump(Duration::from_millis(200));
                return r;
            }
            let (Some(compositor), Some(wm_base), Some(seat), Some(manager)) =
                (&r.compositor, &r.wm_base, &r.seat, &r.manager)
            else {
                panic!("the compositor lacks a global the receiver needs")
            };
            r.device = manager.get_data_device(seat);
            let surface = compositor.create_surface().unwrap();
            let xdg_surface = wm_base.get_xdg_surface(&surface).unwrap();
            let toplevel = xdg_surface.get_toplevel().unwrap();
            toplevel.set_app_id("kompoz-drag-receiver");
            toplevel.set_title("Alıcı");
            surface.commit();
            (r.surface, r.xdg_surface, r.toplevel) = (Some(surface), Some(xdg_surface), Some(toplevel));
            r.roundtrip();
            r.pump(Duration::from_millis(500));
            r
        }

        fn roundtrip(&mut self) {
            self.connection.roundtrip().expect("a roundtrip");
            self.handle_events();
        }

        /// The compositor's events for `time`.
        pub fn pump(&mut self, time: Duration) {
            let end = Instant::now() + time;
            loop {
                let _ = self.connection.flush();
                while !self.connection.prepare_read() {
                    self.connection.dispatch_pending().expect("the connection");
                    self.handle_events();
                }
                let left = end.saturating_duration_since(Instant::now());
                let mut fds = [PollFd::readable(self.connection.fd())];
                if sys::poll(&mut fds, Some(left)).unwrap_or(0) > 0 {
                    self.connection.read_events().expect("the connection");
                } else {
                    self.connection.cancel_read();
                }
                self.connection.dispatch_pending().expect("the connection");
                self.handle_events();
                if Instant::now() >= end {
                    return;
                }
            }
        }

        fn handle_events(&mut self) {
            while let Some(raw) = self.connection.take_event() {
                if let Some((sender, event)) = g::Event::decode(raw) {
                    self.on_event(sender, event);
                }
            }
        }

        fn on_event(&mut self, sender: ObjectId, event: g::Event) {
            use g::Event as E;
            match event {
                E::WlRegistry(WlRegistryEvent::Global { name, interface, version }) => {
                    let Some(registry) = &self.registry else { return };
                    match interface.as_str() {
                        "wl_compositor" => self.compositor = registry.bind(name, version.min(4)),
                        "xdg_wm_base" => self.wm_base = registry.bind(name, 1),
                        "wl_shm" => self.shm = registry.bind(name, 1),
                        "wl_seat" if self.seat.is_none() => self.seat = registry.bind(name, version.min(5)),
                        "wl_data_device_manager" => self.manager = registry.bind(name, version.min(3)),
                        "org_kde_kwin_fake_input" => self.fake = registry.bind(name, version.min(4)),
                        _ => {}
                    }
                }
                E::XdgWmBase(XdgWmBaseEvent::Ping { serial }) => {
                    if let Some(b) = &self.wm_base {
                        b.pong(serial);
                    }
                }
                E::XdgSurface(XdgSurfaceEvent::Configure { serial }) => {
                    if let Some(x) = &self.xdg_surface {
                        x.ack_configure(serial);
                    }
                    if self.buffer.is_none() {
                        self.buffer = self.make_buffer();
                    }
                    if let (Some(s), Some(b)) = (&self.surface, &self.buffer) {
                        s.attach(Some(b), 0, 0);
                        s.damage(0, 0, RECEIVER.2, RECEIVER.3);
                        s.commit();
                    }
                }
                E::WlDataDevice(WlDataDeviceEvent::DataOffer { id }) => {
                    self.offers.insert(id.id(), (id, Vec::new(), 0));
                }
                E::WlDataOffer(WlDataOfferEvent::Offer { mime_type }) => {
                    if let Some(o) = self.offers.get_mut(&sender) {
                        o.1.push(mime_type);
                    }
                }
                E::WlDataOffer(WlDataOfferEvent::Action { dnd_action }) => {
                    if let Some(o) = self.offers.get_mut(&sender) {
                        o.2 = dnd_action.0;
                    }
                }
                E::WlDataDevice(WlDataDeviceEvent::Enter { serial, id, .. }) => {
                    self.enters += 1;
                    self.current = id;
                    if let Some((offer, _, _)) = id.and_then(|id| self.offers.get(&id)) {
                        offer.accept(serial, Some("text/plain;charset=utf-8"));
                        offer.set_actions(Action::COPY | Action::MOVE, self.preferred);
                    }
                }
                E::WlDataDevice(WlDataDeviceEvent::Leave) => {
                    self.leaves += 1;
                    if let Some(id) = self.current.take() {
                        self.offers.remove(&id);
                    }
                }
                E::WlDataDevice(WlDataDeviceEvent::Drop) => {
                    let Some(id) = self.current.take() else { return };
                    let Some((offer, types, action)) = self.offers.remove(&id) else { return };
                    let read = |mime: &str| -> String {
                        let (r, w) = sys::pipe(false).unwrap();
                        offer.receive(mime, w.as_fd());
                        drop(w);
                        self.connection.flush().unwrap();
                        let mut file = std::fs::File::from(r);
                        let mut out = Vec::new();
                        let mut buffer = [0u8; 4096];
                        loop {
                            let mut fd = [PollFd::readable(file.as_raw_fd())];
                            if sys::poll(&mut fd, Some(Duration::from_secs(5))).unwrap_or(0) == 0 {
                                break;
                            }
                            match file.read(&mut buffer) {
                                Ok(0) | Err(_) => break,
                                Ok(n) => out.extend_from_slice(&buffer[..n]),
                            }
                        }
                        String::from_utf8_lossy(&out).into_owned()
                    };
                    let received = Received {
                        uri_list: read("text/uri-list"),
                        text: read("text/plain;charset=utf-8"),
                        own_type: read(OWN_TYPE),
                        types,
                        action,
                    };
                    offer.finish();
                    self.drops.push(received);
                }
                _ => {}
            }
        }

        fn make_buffer(&self) -> Option<WlBuffer> {
            let shm = self.shm.as_ref()?;
            let (w, h) = (RECEIVER.2, RECEIVER.3);
            let size = (w * h * 4) as usize;
            let fd = sys::memfd("kompoz-drag-receiver").ok()?;
            let file = std::fs::File::from(fd);
            file.set_len(size as u64).ok()?;
            let mut map = Mmap::shared(file.as_fd(), size, true).ok()?;
            map.as_mut_slice().fill(0xff);
            let pool = shm.create_pool(file.as_fd(), size as i32)?;
            pool.create_buffer(0, w, h, w * 4, WlShmFormat::ARGB8888)
        }

        fn fake(&self) -> &FakeInput {
            self.fake.as_ref().unwrap()
        }

        /// The pointer pressed on the source, taken along `drag`'s path, let go.
        pub fn drag(&mut self, d: &Drag) {
            self.move_to(SOURCE.0, SOURCE.1);
            Input::pump(self, Duration::from_millis(150));
            self.button(true);
            for &(x, y) in &d.path {
                self.move_to(x, y);
            }
            Input::pump(self, Duration::from_millis(150));
            if d.escape {
                self.key(TestKey::Escape, true);
                self.key(TestKey::Escape, false);
            }
            self.button(false);
        }

        /// A finger held on the source (a long press starts the drag), taken along `path`, lifted.
        pub fn touch_drag(&mut self, path: &[(i32, i32)]) {
            let at = |x: i32, y: i32| [Arg::Uint(1), Arg::Fixed(x as f64), Arg::Fixed(y as f64)];
            self.fake().0.send(4, &at(SOURCE.0, SOURCE.1));
            self.fake().0.send(8, &[]);
            Receiver::pump(self, Duration::from_millis(900));
            for &(x, y) in path {
                self.fake().0.send(5, &at(x, y));
                self.fake().0.send(8, &[]);
                Receiver::pump(self, Duration::from_millis(50));
            }
            Receiver::pump(self, Duration::from_millis(150));
            self.fake().0.send(6, &[Arg::Uint(1)]);
            self.fake().0.send(8, &[]);
            Receiver::pump(self, Duration::from_millis(80));
        }
    }

    impl Input for Receiver {
        fn move_to(&mut self, x: i32, y: i32) {
            self.fake().0.send(9, &[Arg::Fixed(x as f64), Arg::Fixed(y as f64)]);
            self.pump(Duration::from_millis(50));
        }

        fn button(&mut self, pressed: bool) {
            self.fake().0.send(2, &[Arg::Uint(BTN_LEFT), Arg::Uint(u32::from(pressed))]);
            self.pump(Duration::from_millis(80));
        }

        fn key(&mut self, key: TestKey, pressed: bool) {
            let code = match key {
                TestKey::Escape => KEY_ESC,
                TestKey::Ctrl => KEY_LEFTCTRL,
            };
            self.fake().0.send(10, &[Arg::Uint(code), Arg::Uint(u32::from(pressed))]);
            self.pump(Duration::from_millis(50));
        }

        fn pump(&mut self, time: Duration) {
            Receiver::pump(self, time);
        }
    }

    /// kwin's rules: the app at 0, 0 and the receiver at 400, 0, both undecorated.
    pub const KWIN_RULES: &str = "[General]\ncount=2\nrules=1,2\n\n\
        [1]\nDescription=app\nwmclass=dev.kompoz.app\nwmclassmatch=1\nposition=0,0\npositionrule=2\nnoborder=true\nnoborderrule=2\n\n\
        [2]\nDescription=receiver\nwmclass=kompoz-drag-receiver\nwmclassmatch=1\nposition=400,0\npositionrule=2\nnoborder=true\nnoborderrule=2\n";

    /// kwin's output settings (in the test's own configuration): its virtual output `size` pixels at `scale` (what
    /// kwin's `--scale` doesn't set for a virtual output: it makes the output bigger instead).
    fn output_config(scale: f64, size: (u32, u32)) -> String {
        let output = format!(
            "{{\"connectorName\": \"Virtual-0\", \"mode\": {{\"basic\": {{\"width\": {}, \"height\": {}, \"refreshRate\": \
             60000}}, \"flags\": 1}}, \"scale\": {scale}, \"transform\": \"Normal\"}}",
            size.0, size.1
        );
        let setup = "{\"lidClosed\": false, \"outputs\": [{\"enabled\": true, \"outputIndex\": 0, \"position\": {\"x\": 0, \
                     \"y\": 0}, \"priority\": 0, \"replicationSource\": \"\"}]}";
        format!("[{{\"name\": \"outputs\", \"data\": [{output}]}}, {{\"name\": \"setups\", \"data\": [{setup}]}}]")
    }

    /// A kwin of the test's own (a session bus and configuration of its own), its process group stopped when
    /// dropped.
    pub struct Compositor {
        child: Child,
        pub socket: String,
        /// Its Xwayland's display (`:N`), if it was asked for one, and the process that told it (kwin starts it
        /// out of its own process group: stopped on its own).
        pub x_display: Option<String>,
        x_teller: Option<u32>,
    }

    impl Compositor {
        /// kwin at `scale`, its output `size` pixels; with an Xwayland of its own if `xwayland` (its display
        /// learned from an app kwin runs on it: this test again, writing it down).
        pub fn start(work: &Path, socket: &str, scale: f64, size: (u32, u32), xwayland: bool) -> Compositor {
            let config = work.join("config");
            std::fs::create_dir_all(&config).unwrap();
            std::fs::write(config.join("kwinrulesrc"), KWIN_RULES).unwrap();
            std::fs::write(config.join("kwinoutputconfig.json"), output_config(scale, size)).unwrap();
            let mut c = Command::new("dbus-run-session");
            c.args(["--", "kwin_wayland", "--virtual", "--no-lockscreen", "--no-global-shortcuts", "--no-kactivities"])
                .args(["--width", &size.0.to_string(), "--height", &size.1.to_string()])
                .args(["--socket", socket]);
            let display_file = work.join("x-display");
            if xwayland {
                c.arg("--xwayland").arg(std::env::current_exe().unwrap());
                c.env(CHILD, format!("x-display:{}", display_file.display()));
            }
            c.env("XDG_CONFIG_HOME", &config)
                .env("XDG_DATA_HOME", work.join("data"))
                .env("XDG_CACHE_HOME", work.join("cache"));
            c.env_remove("WAYLAND_DISPLAY").env_remove("DISPLAY").env_remove("DBUS_SESSION_BUS_ADDRESS");
            c.stdout(Stdio::null()).stderr(std::fs::File::create(work.join("compositor.log")).unwrap());
            std::os::unix::process::CommandExt::process_group(&mut c, 0);
            let child = c.spawn().expect("dbus-run-session and kwin_wayland");
            let mut compositor = Compositor { child, socket: socket.to_string(), x_display: None, x_teller: None };
            let runtime = PathBuf::from(std::env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR"));
            let start = Instant::now();
            while !runtime.join(socket).exists() {
                assert!(start.elapsed() < Duration::from_secs(20), "the compositor didn't start (see compositor.log)");
                std::thread::sleep(Duration::from_millis(50));
            }
            while xwayland && compositor.x_display.is_none() {
                assert!(start.elapsed() < Duration::from_secs(30), "kwin's Xwayland didn't start (see compositor.log)");
                compositor.x_display = std::fs::read_to_string(&display_file).ok().filter(|d| d.ends_with('\n'));
                std::thread::sleep(Duration::from_millis(50));
            }
            if let Some(told) = compositor.x_display.take() {
                let mut parts = told.split_whitespace();
                compositor.x_display = parts.next().map(str::to_string);
                compositor.x_teller = parts.next().and_then(|p| p.parse().ok());
            }
            std::thread::sleep(Duration::from_millis(500));
            compositor
        }
    }

    impl Drop for Compositor {
        fn drop(&mut self) {
            if let Some(pid) = self.x_teller {
                let _ = Command::new("kill").args(["-TERM", &pid.to_string()]).status();
            }
            let group = format!("-{}", self.child.id());
            let _ = Command::new("kill").args(["-TERM", "--", &group]).status();
            let alive = || {
                let quiet = Stdio::null;
                Command::new("kill").args(["-0", "--", &group]).stderr(quiet()).status().is_ok_and(|s| s.success())
            };
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(5) {
                let _ = self.child.try_wait();
                if !alive() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            let _ = Command::new("kill").args(["-KILL", "--", &group]).status();
            let _ = self.child.wait();
        }
    }

    /// The drags on kwin at `scale` (its output `size` pixels: 800 × 600 points), the app's icon `icon` pixels.
    pub fn run(work: &Path, scale: f64, size: (u32, u32), icon_pixels: (u32, u32)) {
        let compositor = Compositor::start(work, &format!("kompoz-drag-{}", std::process::id()), scale, size, false);
        let mut r = Receiver::connect(&compositor.socket, true);
        let app =
            start_app(work, &[("WAYLAND_DISPLAY", compositor.socket.as_str()), ("WAYLAND_DEBUG", "1")], &mut |t| {
                r.pump(t)
            });
        let receiver_middle = (RECEIVER.0 + 150, RECEIVER.1 + 150);

        // To the receiver, which prefers Move: what it read, the action on both sides.
        r.preferred = Action::MOVE;
        r.drag(&Drag { path: path_out((0, 0), &[(450, 100), receiver_middle]), escape: false });
        assert!(wait_for(work, "kompoz-drag: completed", 1, &mut |t| r.pump(t)), "no end:\n{}", tail(&log(work)));
        assert_eq!(r.drops.len(), 1, "the receiver got no drop:\n{}", tail(&log(work)));
        let got = &r.drops[0];
        assert_eq!(
            got.types,
            ["text/uri-list", "text/plain;charset=utf-8", "text/plain", "UTF8_STRING", OWN_TYPE],
            "the types offered"
        );
        assert_eq!(got.uri_list, "file:///tmp/kompoz%20s%C3%BCr%C3%BCkle/%C3%A7.txt\r\n");
        assert_eq!((got.text.as_str(), got.own_type.as_str()), (TEXT, TEXT));
        assert_eq!(got.action, Action::MOVE.0, "the receiver's action");
        assert_eq!(completed(work, 0), "Some(Move)");

        // The receiver preferring Copy: Copy.
        r.preferred = Action::COPY;
        r.drag(&Drag { path: path_out((0, 0), &[(450, 100), receiver_middle]), escape: false });
        assert!(wait_for(work, "kompoz-drag: completed", 2, &mut |t| r.pump(t)), "no end:\n{}", tail(&log(work)));
        assert_eq!(r.drops.len(), 2);
        assert_eq!(completed(work, 1), "Some(Copy)");

        // Dropped on nothing (below the receiver): untaken.
        r.drag(&Drag { path: path_out((0, 0), &[(380, 400), (420, 450)]), escape: false });
        assert!(wait_for(work, "kompoz-drag: completed", 3, &mut |t| r.pump(t)), "no end:\n{}", tail(&log(work)));
        assert_eq!(completed(work, 2), "None");

        // Escape over the receiver: called off, nothing dropped on it.
        r.drag(&Drag { path: path_out((0, 0), &[(450, 100), receiver_middle]), escape: true });
        assert!(wait_for(work, "kompoz-drag: completed", 4, &mut |t| r.pump(t)), "no end:\n{}", tail(&log(work)));
        assert_eq!(completed(work, 3), "None", "Escape didn't call the drag off");
        assert_eq!(r.drops.len(), 2, "Escape: the receiver got a drop");

        // Out of the window and back onto the app's own target: its own data (local state too), moved.
        r.drag(&Drag { path: path_out((0, 0), &[(390, 120), (300, 200), TARGET]), escape: false });
        assert!(wait_for(work, "kompoz-drag: completed", 5, &mut |t| r.pump(t)), "no end:\n{}", tail(&log(work)));
        let dropped = log(work).lines().filter(|l| l.starts_with("kompoz-drag: dropped in the app:")).count();
        assert_eq!(dropped, 1, "not dropped on the app's target:\n{}", tail(&log(work)));
        assert!(
            log(work).contains(&format!(
                "kompoz-drag: dropped in the app: {TEXT} | yerel | text/uri-list,text/plain,{OWN_TYPE}"
            )),
            "the app's own data:\n{}",
            tail(&log(work))
        );
        assert_eq!(completed(work, 4), "Some(Move)");

        // A finger's drag (a long press on the source) to the receiver: the compositor's touch drag, from the
        // touch's serial.
        r.touch_drag(&path_out((0, 0), &[(450, 100), receiver_middle]));
        assert!(wait_for(work, "kompoz-drag: completed", 6, &mut |t| r.pump(t)), "no end:\n{}", tail(&log(work)));
        assert_eq!(r.drops.len(), 3, "the finger's drag: no drop:\n{}", tail(&log(work)));
        assert_eq!(r.drops[2].text, TEXT);
        assert_eq!(completed(work, 5), "Some(Copy)");
        drop(app);

        // The icon (the protocol's traffic): a surface given with start_drag, its buffer the decoration's pixels at
        // the scale (120 × 80 points), shown at 120 × 80 points, the pointer where the drag began in it.
        let traffic = log(work);
        let starts: Vec<&str> =
            traffic.lines().filter(|l| l.contains(" -> wl_data_device#") && l.contains(".start_drag(")).collect();
        assert_eq!(starts.len(), 6, "start_drag requests:\n{}", starts.join("\n"));
        let icon = starts[0].split(", ").nth(2).expect("an icon surface").to_string(); // ("wl_surface#N")
        assert!(icon.starts_with("wl_surface#"), "no icon surface: {}", starts[0]);
        let (w, h) = icon_pixels;
        let sized = format!(", 0, {w}, {h}, {}, 0)", w * 4);
        assert!(
            traffic.lines().any(|l| l.contains("create_buffer(new id wl_buffer#") && l.contains(&sized)),
            "no {w}×{h} icon buffer:\n{}",
            traffic.lines().filter(|l| l.contains("create_buffer")).collect::<Vec<_>>().join("\n")
        );
        assert!(traffic.lines().any(|l| l.contains(&format!(" -> {icon}.offset(-60, -40)"))), "the icon's offset");
        assert!(traffic.lines().any(|l| l.contains(".set_destination(120, 80)")), "the icon's size in points");
        drop(compositor);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// X11

mod x11 {
    use super::*;
    use kompoz_platform::linux::sys::Library;
    use kompoz_platform::linux::x11::ffi::{self, Atom, Decoded, Display, NONE, Window};
    use std::ffi::{CStr, c_char, c_int, c_long, c_uint, c_ulong, c_void};
    use std::sync::mpsc;

    /// XTEST through libXtst, on a connection of its own.
    pub struct Xtest {
        display: *mut c_void,
        motion: unsafe extern "C" fn(*mut c_void, c_int, c_int, c_int, c_ulong) -> c_int,
        button: unsafe extern "C" fn(*mut c_void, c_uint, c_int, c_ulong) -> c_int,
        key: unsafe extern "C" fn(*mut c_void, c_uint, c_int, c_ulong) -> c_int,
        keycode: unsafe extern "C" fn(*mut c_void, c_ulong) -> u8,
        sync: unsafe extern "C" fn(*mut c_void, c_int) -> c_int,
        close: unsafe extern "C" fn(*mut c_void) -> c_int,
        _x11: Library,
        _xtst: Library,
    }

    fn symbol<F: Copy>(l: &Library, name: &CStr) -> F {
        let p = l.symbol(name).unwrap_or_else(|| panic!("no {name:?}"));
        // SAFETY: the caller's `F` is the symbol's C signature.
        unsafe { std::mem::transmute_copy::<*mut c_void, F>(&p) }
    }

    impl Xtest {
        pub fn open() -> Xtest {
            let x11 = Library::open(&["libX11.so.6"]).expect("libX11");
            let xtst = Library::open(&["libXtst.so.6"]).expect("libXtst (XTEST) for the X11 drags");
            let open: unsafe extern "C" fn(*const c_char) -> *mut c_void = symbol(&x11, c"XOpenDisplay");
            // SAFETY: the environment's display.
            let display = unsafe { open(std::ptr::null()) };
            assert!(!display.is_null(), "the X display");
            Xtest {
                display,
                motion: symbol(&xtst, c"XTestFakeMotionEvent"),
                button: symbol(&xtst, c"XTestFakeButtonEvent"),
                key: symbol(&xtst, c"XTestFakeKeyEvent"),
                keycode: symbol(&x11, c"XKeysymToKeycode"),
                sync: symbol(&x11, c"XSync"),
                close: symbol(&x11, c"XCloseDisplay"),
                _x11: x11,
                _xtst: xtst,
            }
        }

        fn sync(&self) {
            // SAFETY: an open display.
            unsafe { (self.sync)(self.display, 0) };
            std::thread::sleep(Duration::from_millis(40));
        }
    }

    impl Input for Xtest {
        fn move_to(&mut self, x: i32, y: i32) {
            // SAFETY: an open display; -1: the screen the pointer is on.
            unsafe { (self.motion)(self.display, -1, x, y, 0) };
            self.sync();
        }

        fn button(&mut self, pressed: bool) {
            // SAFETY: an open display.
            unsafe { (self.button)(self.display, 1, c_int::from(pressed), 0) };
            self.sync();
        }

        fn key(&mut self, key: TestKey, pressed: bool) {
            let keysym = match key {
                TestKey::Escape => XK_ESCAPE,
                TestKey::Ctrl => XK_CONTROL_L,
            };
            // SAFETY: an open display.
            let code = unsafe { (self.keycode)(self.display, keysym) };
            // SAFETY: as above.
            unsafe { (self.key)(self.display, c_uint::from(code), c_int::from(pressed), 0) };
            self.sync();
        }

        fn pump(&mut self, time: Duration) {
            std::thread::sleep(time);
        }
    }

    impl Drop for Xtest {
        fn drop(&mut self) {
            // SAFETY: opened in `open`, closed once.
            unsafe { (self.close)(self.display) };
        }
    }

    const XK_ESCAPE: c_ulong = 0xff1b;
    const XK_CONTROL_L: c_ulong = 0xffe3;

    /// What the receiver saw of a drag: the window its messages were about (the proxy's toplevel), the version, the
    /// types offered, what it read, the action it took.
    #[derive(Debug, Default, Clone)]
    pub struct Received {
        pub window: Window,
        pub version: c_long,
        pub types: Vec<String>,
        pub uri_list: String,
        pub text: String,
        pub utf8_string: String,
        pub latin1: String,
        pub own_type: String,
        pub action: String,
    }

    /// What the receiver tells the test.
    pub enum Seen {
        Ready,
        Dropped(Received),
        Left,
    }

    /// The receiver, on a thread and connection of its own: an override-redirect window at `RECEIVER` (XdndAware
    /// through its proxy, an unmapped window whose XdndProxy points to itself), taking drops with the action asked.
    pub fn receiver(seen: mpsc::Sender<Seen>, stop: mpsc::Receiver<()>) {
        let d = Display::open().expect("the X display");
        let names = [
            c"XdndAware",
            c"XdndProxy",
            c"XdndEnter",
            c"XdndPosition",
            c"XdndStatus",
            c"XdndLeave",
            c"XdndDrop",
            c"XdndFinished",
            c"XdndSelection",
            c"XdndTypeList",
            c"text/uri-list",
            c"text/plain;charset=utf-8",
            c"UTF8_STRING",
            c"KOMPOZ_RECEIVED",
            c"application/x-kompoz-test",
        ];
        let [
            aware,
            proxy_atom,
            enter,
            position,
            status,
            leave,
            drop_atom,
            finished,
            selection,
            type_list,
            uri_list,
            plain_utf8,
            utf8,
            received,
            own,
        ] = d.intern_atoms(&names).try_into().unwrap();
        let top = d.create_visual_window(RECEIVER.0, RECEIVER.1, RECEIVER.2 as u32, RECEIVER.3 as u32, false).unwrap();
        let proxy = d.create_visual_window(-10, -10, 1, 1, false).unwrap();
        d.set_property_32(proxy.window, aware, ffi::XA_ATOM, &[5]);
        d.set_property_32(proxy.window, proxy_atom, 33, &[proxy.window as c_long]);
        d.set_property_32(top.window, proxy_atom, 33, &[proxy.window as c_long]);
        d.set_window_pixels(&top, 1, 1, &[0xffffffff]);
        d.map_raised(top.window);
        d.sync();
        seen.send(Seen::Ready).unwrap();
        let known =
            ["text/uri-list", "text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "STRING", "TEXT", OWN_TYPE];
        let known_c = [
            c"text/uri-list",
            c"text/plain;charset=utf-8",
            c"UTF8_STRING",
            c"text/plain",
            c"STRING",
            c"TEXT",
            c"application/x-kompoz-test",
        ];
        let known_atoms = d.intern_atoms(&known_c);
        let name = |a: Atom| known_atoms.iter().position(|&x| x == a).map_or("?".to_string(), |i| known[i].to_string());
        let mut got = Received::default();
        let mut source: Window = NONE;
        let mut asked: Atom = NONE;
        loop {
            if stop.try_recv().is_ok() {
                break;
            }
            while d.pending() > 0 {
                let e = d.next_event();
                let Decoded::ClientMessage { window, message_type, data } = e.decode() else { continue };
                if message_type == enter {
                    source = data[0] as Window;
                    got = Received { window, version: data[1] >> 24, ..Received::default() };
                    let types: Vec<Atom> = if data[1] & 1 != 0 {
                        d.property(source, type_list, false)
                            .map(|p| p.longs())
                            .unwrap_or_default()
                            .iter()
                            .map(|&t| t as Atom)
                            .collect()
                    } else {
                        data[2..5].iter().filter(|&&t| t != 0).map(|&t| t as Atom).collect()
                    };
                    got.types = types.iter().map(|&t| name(t)).collect();
                } else if message_type == position {
                    asked = data[4] as Atom;
                    let reply = [top.window as c_long, 1 | 2, 0, 0, asked as c_long];
                    d.send_client_message(source, source, status, reply, ffi::NO_EVENT_MASK);
                    d.flush();
                } else if message_type == leave {
                    seen.send(Seen::Left).unwrap();
                } else if message_type == drop_atom {
                    let time = data[2] as ffi::Time;
                    let read = |target: Atom| -> String {
                        d.delete_property(top.window, received);
                        d.convert_selection(selection, target, received, top.window, time);
                        d.flush();
                        let deadline = Instant::now() + Duration::from_secs(5);
                        while Instant::now() < deadline {
                            let notify = d.check_if_event(&mut |e| {
                                matches!(e.decode(), Decoded::SelectionNotify { requestor, .. } if requestor == top.window)
                            });
                            if let Some(n) = notify {
                                let Decoded::SelectionNotify { property, .. } = n.decode() else { unreachable!() };
                                if property == NONE {
                                    return String::new();
                                }
                                let p = d.property(top.window, received, true).map(|p| p.bytes).unwrap_or_default();
                                return if target == ffi::XA_STRING {
                                    p.iter().map(|&b| char::from(b)).collect()
                                } else {
                                    String::from_utf8_lossy(&p).into_owned()
                                };
                            }
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        String::from("(no answer)")
                    };
                    got.uri_list = read(uri_list);
                    got.text = read(plain_utf8);
                    got.utf8_string = read(utf8);
                    got.latin1 = read(ffi::XA_STRING);
                    got.own_type = read(own);
                    got.action = match d
                        .intern_atoms(&[c"XdndActionCopy", c"XdndActionMove", c"XdndActionLink"])
                        .iter()
                        .position(|&a| a == asked)
                    {
                        Some(0) => "copy".into(),
                        Some(1) => "move".into(),
                        Some(2) => "link".into(),
                        _ => "?".into(),
                    };
                    let reply = [top.window as c_long, 1, asked as c_long, 0, 0];
                    d.send_client_message(source, source, finished, reply, ffi::NO_EVENT_MASK);
                    d.flush();
                    seen.send(Seen::Dropped(got.clone())).unwrap();
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        d.destroy_visual_window(top);
        d.destroy_visual_window(proxy);
        d.flush();
    }

    /// The app's window (WM_CLASS dev.kompoz.app) and where it is on the screen.
    fn app_window(d: &Display) -> Option<(Window, i32, i32)> {
        let class = d.intern_atoms(&[c"WM_CLASS"])[0];
        let mut stack = vec![(d.root(), 0)];
        while let Some((w, depth)) = stack.pop() {
            if d.property(w, class, false).is_some_and(|p| p.bytes.starts_with(b"dev.kompoz.app")) {
                let (x, y, _) = d.child_at(w, d.root(), 0, 0)?;
                return Some((w, x, y));
            }
            if depth < 3 {
                stack.extend(d.children(w).into_iter().map(|c| (c, depth + 1)));
            }
        }
        None
    }

    /// The drag's icon window (`_NET_WM_WINDOW_TYPE_DND`, shown): its place and size.
    fn icon_window(d: &Display) -> Option<(i32, i32, i32, i32)> {
        let [kind, dnd] = d.intern_atoms(&[c"_NET_WM_WINDOW_TYPE", c"_NET_WM_WINDOW_TYPE_DND"]).try_into().unwrap();
        d.children(d.root()).into_iter().find_map(|w| {
            let is_dnd = d.property(w, kind, false).is_some_and(|p| p.longs().first() == Some(&(dnd as c_long)));
            let g = d.geometry(w)?;
            (is_dnd && g.viewable).then_some((g.x, g.y, g.width, g.height))
        })
    }

    /// The XTEST run: on the X server `DISPLAY` names.
    pub fn run_xtest(work: &Path) {
        let display = std::env::var("DISPLAY").unwrap_or_default();
        run(work, &display, &mut Xtest::open(), &[], true);
    }

    /// The run on the Xwayland of a kwin of the test's own, the input kwin's fake input.
    pub fn run_kwin(work: &Path) {
        let socket = format!("kompoz-drag-x-{}", std::process::id());
        let compositor = wayland::Compositor::start(work, &socket, 1.0, (1280, 1024), true);
        let display = compositor.x_display.clone().expect("kwin's Xwayland");
        // (Its own Xwayland takes no cookie: the desktop's XAUTHORITY isn't for it. Xvfb's, as xvfb-run sets it up,
        // is what the XTEST run needs.)
        // SAFETY: the test's main thread, no other thread running yet.
        unsafe { std::env::remove_var("XAUTHORITY") };
        let mut input = wayland::Receiver::connect(&compositor.socket, false);
        run(work, &display, &mut input, &[("WAYLAND_DISPLAY", "")], false);
        drop(input);
        drop(compositor);
    }

    /// The pointer pressed on the source (the app's window at `origin`), along `path`, Ctrl held from `ctrl_from`
    /// on, Escape pressed at the end if `escape`, let go; the icon window seen just before.
    fn drag(
        input: &mut dyn Input,
        d: &Display,
        origin: (i32, i32),
        path: &[(i32, i32)],
        escape: bool,
        ctrl_from: Option<usize>,
    ) -> Option<(i32, i32, i32, i32)> {
        input.move_to(origin.0 + SOURCE.0, origin.1 + SOURCE.1);
        input.pump(Duration::from_millis(150));
        input.button(true);
        for (i, &(x, y)) in path.iter().enumerate() {
            if ctrl_from == Some(i) {
                input.key(TestKey::Ctrl, true);
            }
            input.move_to(x, y);
        }
        input.pump(Duration::from_millis(200));
        let icon = icon_window(d);
        if escape {
            input.key(TestKey::Escape, true);
            input.key(TestKey::Escape, false);
        }
        input.button(false);
        if ctrl_from.is_some() {
            input.key(TestKey::Ctrl, false);
        }
        icon
    }

    /// The drags on the X server `display`, `input` making the input, the app run with `env` besides.
    /// (`escape`: whether a key pressed reaches the app during its drag — kwin's own bridge of an X drag to Wayland
    /// takes the keys while it runs.)
    fn run(work: &Path, display: &str, input: &mut dyn Input, env: &[(&str, &str)], escape: bool) {
        // SAFETY: the test's main thread, no other thread running yet.
        unsafe { std::env::set_var("DISPLAY", display) };
        let (seen_tx, seen) = mpsc::channel();
        let (stop_tx, stop) = mpsc::channel();
        let thread = std::thread::spawn(move || receiver(seen_tx, stop));
        assert!(matches!(seen.recv_timeout(Duration::from_secs(10)), Ok(Seen::Ready)), "the receiver");
        let d = Display::open().expect("the X display");
        let mut app_env = vec![("KOMPOZ_PLATFORM", "x11"), ("KOMPOZ_SCALE", "1"), ("DISPLAY", display)];
        app_env.extend_from_slice(env);
        let app = start_app(work, &app_env, &mut |t| input.pump(t));
        let (_, ox, oy) = app_window(&d).expect("the app's window");
        let receiver_middle = (RECEIVER.0 + 150, RECEIVER.1 + 150);
        let out = |to: &[(i32, i32)]| path_out((ox, oy), to);
        let next_drop = || loop {
            match seen.recv_timeout(Duration::from_secs(10)) {
                Ok(Seen::Dropped(r)) => return Some(r),
                Ok(_) => continue,
                Err(_) => return None,
            }
        };

        // To the receiver (through its proxy): the types offered, each read, Move (no modifier) on both sides; the
        // icon following the pointer, the pointer where the drag began in it (its middle).
        let icon = drag(input, &d, (ox, oy), &out(&[(450, 100), receiver_middle]), false, None);
        let got = next_drop().unwrap_or_else(|| panic!("the receiver got no drop:\n{}", tail(&log(work))));
        assert!(wait_for(work, "kompoz-drag: completed", 1, &mut |t| input.pump(t)), "no end:\n{}", tail(&log(work)));
        assert_eq!(got.version, 5);
        assert_ne!(got.window, NONE);
        assert_eq!(
            got.types,
            ["text/uri-list", "text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "STRING", "TEXT", OWN_TYPE],
            "the types offered (its XdndTypeList)"
        );
        assert_eq!(got.uri_list, "file:///tmp/kompoz%20s%C3%BCr%C3%BCkle/%C3%A7.txt\r\n");
        assert_eq!((got.text.as_str(), got.utf8_string.as_str(), got.own_type.as_str()), (TEXT, TEXT, TEXT));
        assert_eq!(got.latin1, "Merhaba, sürükle b?rak!"); // (Latin-1: what it can't hold as '?')
        assert_eq!(got.action, "move");
        assert_eq!(completed(work, 0), "Some(Move)");
        assert_eq!(icon, Some((receiver_middle.0 - 60, receiver_middle.1 - 40, 120, 80)), "the drag's icon window");
        assert_eq!(icon_window(&d), None, "the icon left after the drop");

        // Ctrl held on the way: Copy asked for, taken.
        drag(input, &d, (ox, oy), &out(&[(450, 100), receiver_middle]), false, Some(2));
        let got = next_drop().expect("the receiver got no drop");
        assert!(wait_for(work, "kompoz-drag: completed", 2, &mut |t| input.pump(t)), "no end:\n{}", tail(&log(work)));
        assert_eq!(got.action, "copy");
        assert_eq!(completed(work, 1), "Some(Copy)");

        // Dropped on nothing: untaken.
        drag(input, &d, (ox, oy), &out(&[(380, 400), (420, 450)]), false, None);
        assert!(wait_for(work, "kompoz-drag: completed", 3, &mut |t| input.pump(t)), "no end:\n{}", tail(&log(work)));
        assert_eq!(completed(work, 2), "None");

        // Escape over the receiver: it is told the drag left, nothing is dropped. (Where the app gets the key: kwin
        // takes the keys during an X client's drag, for its bridge of it to Wayland's clients.)
        let mut ends = 4;
        if escape {
            drag(input, &d, (ox, oy), &out(&[(450, 100), receiver_middle]), true, None);
            let end = wait_for(work, "kompoz-drag: completed", 4, &mut |t| input.pump(t));
            assert!(end, "no end:\n{}", tail(&log(work)));
            assert_eq!(completed(work, 3), "None");
            assert!(matches!(seen.recv_timeout(Duration::from_secs(2)), Ok(Seen::Left)), "Escape: no leave");
            assert!(seen.recv_timeout(Duration::from_millis(500)).is_err(), "Escape: a drop");
            ends = 5;
        }

        // Out and back onto the app's own target: its own data, moved.
        let back = [(390, 120), (ox + 300, oy + 200), (ox + TARGET.0, oy + TARGET.1)];
        drag(input, &d, (ox, oy), &out(&back), false, None);
        let end = wait_for(work, "kompoz-drag: completed", ends, &mut |t| input.pump(t));
        assert!(end, "no end:\n{}", tail(&log(work)));
        assert!(
            log(work).contains(&format!(
                "kompoz-drag: dropped in the app: {TEXT} | yerel | text/uri-list,text/plain,{OWN_TYPE}"
            )),
            "the app's own data:\n{}",
            tail(&log(work))
        );
        assert_eq!(completed(work, ends - 1), "Some(Move)");
        drop(app);
        stop_tx.send(()).unwrap();
        thread.join().unwrap();
    }
}

fn main() -> ExitCode {
    if let Ok(child) = std::env::var(CHILD) {
        if let Some(file) = child.strip_prefix("x-display:") {
            // (Run by kwin on its Xwayland: its display and process written down, then kept until the test stops
            // it — at the latest an hour on.)
            let display = std::env::var("DISPLAY").unwrap_or_default();
            std::fs::write(file, format!("{display} {}\n", std::process::id())).unwrap();
            std::thread::sleep(Duration::from_secs(3600));
            return ExitCode::SUCCESS;
        }
        return App::new("Sürükle").size(360.dp(), 300.dp()).run(DragApp);
    }
    if std::env::var("KOMPOZ_DISPLAY_TESTS").as_deref() != Ok("1") {
        println!("skipped: the drag display tests run with KOMPOZ_DISPLAY_TESTS=1");
        return ExitCode::SUCCESS;
    }
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let compositor = std::env::var("KOMPOZ_TEST_COMPOSITOR").unwrap_or_default();
    let kwin = matches!(compositor.trim(), "" | "kwin");
    type Run = Box<dyn Fn(&Path)>;
    let mut runs: Vec<(&str, Option<&str>, Run)> = vec![
        (
            "wayland: kwin",
            (!kwin).then_some("Weston's headless backend has no input a client can make: the Wayland drags need kwin"),
            Box::new(|w: &Path| wayland::run(w, 1.0, (800, 600), (120, 80))),
        ),
        (
            "wayland: kwin at scale 2",
            (!kwin).then_some("as above"),
            Box::new(|w: &Path| wayland::run(w, 2.0, (1600, 1200), (240, 160))),
        ),
        (
            "wayland: kwin at scale 1.5",
            (!kwin).then_some("as above"),
            Box::new(|w: &Path| wayland::run(w, 1.5, (1200, 900), (180, 120))),
        ),
    ];
    runs.push((
        "x11: kwin's Xwayland",
        (!kwin).then_some("Xwayland's input is made through kwin's fake input: it needs kwin"),
        Box::new(|w: &Path| x11::run_kwin(w)),
    ));
    let xtest = std::env::var("KOMPOZ_X11_XTEST").as_deref() == Ok("1");
    let display = std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty());
    runs.push((
        "x11: XTEST",
        match (xtest, display) {
            (false, _) => {
                Some("XTEST on the X server DISPLAY names runs with KOMPOZ_X11_XTEST=1 (Xvfb: not a desktop's)")
            }
            (true, false) => Some("KOMPOZ_X11_XTEST=1 but DISPLAY isn't set"),
            (true, true) => None,
        },
        Box::new(|w: &Path| x11::run_xtest(w)),
    ));
    let mut failed = 0;
    for (name, skip, run) in runs {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        if let Some(why) = skip {
            println!("skipped {name}: {why}");
            continue;
        }
        let work = std::env::temp_dir().join(format!(
            "kompoz-drag-test-{}-{}",
            std::process::id(),
            name.replace([' ', ':'], "-")
        ));
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).unwrap();
        let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&work))).is_ok();
        if ok {
            let _ = std::fs::remove_dir_all(&work);
        }
        failed += usize::from(!ok);
        println!(
            "{} {name}{}",
            if ok { "ok    " } else { "FAILED" },
            if ok { String::new() } else { format!(" (kept: {})", work.display()) }
        );
    }
    if failed > 0 { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
