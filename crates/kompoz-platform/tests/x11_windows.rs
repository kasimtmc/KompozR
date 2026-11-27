//! The app's windows on X11 on a real display (`KOMPOZ_DISPLAY_TESTS=1`, with `DISPLAY` naming an X server: Xvfb in
//! CI, kwin's Xwayland here; skipped otherwise): one connection, an X window each, a Vulkan device each. A click
//! opens a `Window` of its own (its state's size and place, its title on the X window), a script reaches it by its
//! title, the window manager's close (`window close`) is its `on_close_request` (it closes, the main window going on),
//! another opened minimized and maximized as its state says; no validation errors. And a transparent, undecorated
//! window (a 32-bit visual) with an icon and a menu bar: its menu bar drawn and driven by the keyboard, or, with a
//! global menu's registrar on the session bus, registered with it by the X window's id (the bar not drawn, its
//! item's shortcut still the app's). Run in a process of its own (this test run again).

#![allow(non_snake_case)]

use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_platform::App;
use kompoz_runtime::*;
use kompoz_ui::Modifier;
use kompoz_ui::input::Key;
use kompoz_ui::window::{
    FrameWindowScope, KeyShortcut, MenuBarScope, MenuScope, Window, WindowPlacement, WindowPosition,
    remember_window_state,
};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::rc::Rc;
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "linux")]
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CHILD: &str = "KOMPOZ_X11_WINDOWS_CHILD";

/// The main window white; a click shows "İkinci" (160 by 120 dp at 40, 50, red) and "Üçüncü" (maximized, blue); the
/// second's close request takes it out.
#[kompozable]
fn TwoWindows() {
    let shown = remember(|| mutable_state_of(false));
    let toggle = shown.clone();
    Box(modifier = Modifier.fill_max_size().background(Color::White).clickable(on_click = move || toggle.set(true)));
    if shown.get() {
        let hide = shown.clone();
        Window(
            on_close_request = move || hide.set(false),
            title = "İkinci".to_string(),
            state = remember_window_state(
                position = WindowPosition::Absolute { x: 40.dp(), y: 50.dp() },
                size = DpSize(160.dp(), 120.dp()),
            ),
            content = |_: &FrameWindowScope| Box(modifier = Modifier.fill_max_size().background(Color::Red)),
        );
        Window(
            on_close_request = || {},
            title = "Üçüncü".to_string(),
            state = remember_window_state(placement = WindowPlacement::Maximized, size = DpSize(100.dp(), 80.dp())),
            content = |_: &FrameWindowScope| Box(modifier = Modifier.fill_max_size().background(Color::Blue)),
        );
    }
}

/// A transparent, undecorated window of its own with an icon and a menu bar (Dosya: Bitti, Ctrl+B), over the
/// main window (white; green once Bitti ran); its content a red square 40 dp down.
#[kompozable]
fn Extras() {
    let done = remember(|| mutable_state_of(false));
    Box(modifier = Modifier.fill_max_size().background(if done.get() { Color::Green } else { Color::White }));
    let d = done.clone();
    Window(
        on_close_request = || {},
        title = "Üçüncü".to_string(),
        transparent = true,
        undecorated = true,
        icon = Some(Rc::new(kompoz_ui::draw::ColorPainter(Color::Blue)) as Rc<dyn kompoz_ui::draw::Painter>),
        state = remember_window_state(size = DpSize(200.dp(), 150.dp())),
        content = move |scope: &FrameWindowScope| {
            let d = d.clone();
            scope.MenuBar(move |bar: &MenuBarScope| {
                let d = d.clone();
                bar.Menu(
                    text = "Dosya".to_string(),
                    mnemonic = Some('D'),
                    content = move |m: &MenuScope| {
                        let d = d.clone();
                        m.Item(
                            text = "Bitti".to_string(),
                            shortcut = Some(KeyShortcut(Key::B, ctrl = true)),
                            on_click = move || d.set(true),
                        );
                    },
                );
            });
            Box(modifier = Modifier.padding_sides(top = 40.dp()).size(60.dp()).background(Color::Red));
        },
    );
}

/// A `com.canonical.AppMenu.Registrar` on a session bus of the test's own (a dbus-daemon, stopped when dropped),
/// what it was asked kept: the windows registered (their X ids, their menus' paths). (Linux's D-Bus client: the
/// display tests are Linux's.)
#[cfg(target_os = "linux")]
struct GlobalMenu {
    daemon: std::process::Child,
    address: String,
    stop: Arc<AtomicBool>,
    registered: Arc<Mutex<Vec<(u32, String)>>>,
    registrar: Option<std::thread::JoinHandle<()>>,
}

#[cfg(target_os = "linux")]
impl GlobalMenu {
    fn start(dir: &Path) -> GlobalMenu {
        use kompoz_platform::linux::dbus::{Connection, MessageType, Value};
        let address = format!("unix:abstract=kompoz-x11-test-bus-{}", std::process::id());
        let config = dir.join("bus.conf");
        std::fs::write(
            &config,
            format!(
                "<!DOCTYPE busconfig PUBLIC \"-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN\" \
                 \"http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd\">\n\
                 <busconfig><type>session</type><listen>{address}</listen><auth>EXTERNAL</auth>\
                 <policy context=\"default\"><allow send_destination=\"*\" eavesdrop=\"true\"/>\
                 <allow eavesdrop=\"true\"/><allow own=\"*\"/></policy></busconfig>\n"
            ),
        )
        .unwrap();
        let daemon = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--nopidfile"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon");
        let start = Instant::now();
        let mut bus = loop {
            match Connection::open(&address) {
                Ok(c) => break c,
                Err(e) if start.elapsed() > Duration::from_secs(10) => panic!("the test bus doesn't answer: {e}"),
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        };
        assert_eq!(bus.request_name("com.canonical.AppMenu.Registrar", 4).unwrap(), 1);
        let stop = Arc::new(AtomicBool::new(false));
        let registered = Arc::new(Mutex::new(Vec::new()));
        let (stopped, heard) = (stop.clone(), registered.clone());
        let registrar = std::thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                let Ok(Some(m)) = bus.read(Some(Duration::from_millis(50)), None) else { continue };
                if m.kind != MessageType::MethodCall || bus.answer_peer(&m).unwrap_or(true) {
                    continue;
                }
                if m.member.as_deref() == Some("RegisterWindow") {
                    let xid = m.arg(0).and_then(Value::as_u32).unwrap_or(0);
                    let path = match m.arg(1) {
                        Some(Value::ObjectPath(p)) => p.clone(),
                        _ => String::new(),
                    };
                    heard.lock().unwrap().push((xid, path));
                }
                let _ = bus.reply(&m, vec![]);
            }
        });
        GlobalMenu { daemon, address, stop, registered, registrar: Some(registrar) }
    }
}

#[cfg(target_os = "linux")]
impl Drop for GlobalMenu {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(r) = self.registrar.take() {
            let _ = r.join();
        }
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

fn lavapipe_icd() -> String {
    let dir = Path::new("/usr/share/vulkan/icd.d");
    ["lvp_icd.json".to_string(), format!("lvp_icd.{}.json", std::env::consts::ARCH)]
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.exists())
        .map(|path| path.display().to_string())
        .expect("lavapipe (Mesa's lvp_icd.json) for the display tests")
}

/// The app run with `script` on X11 in `dir`: what it logged.
fn run_app(dir: &Path, script: &str) -> String {
    run_app_on(dir, "windows", script, "unix:path=/nonexistent")
}

/// The app `app` ("windows", "extras") run with `script` on X11 in `dir`, its session bus `bus`: what it logged.
fn run_app_on(dir: &Path, app: &str, script: &str, bus: &str) -> String {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .current_dir(dir)
        .env(CHILD, app)
        .env("KOMPOZ_ATSPI", "0")
        .env("KOMPOZ_PLATFORM", "x11")
        .env("KOMPOZ_GPU_VALIDATION", "1")
        .env("KOMPOZ_SCALE", "1")
        .env("KOMPOZ_SCRIPT", script)
        .env("VK_DRIVER_FILES", lavapipe_icd())
        .env("KOMPOZ_NO_WALLPAPER_COLORS", "1")
        .env("DBUS_SESSION_BUS_ADDRESS", bus) // (the desktop's services: none, or the test's registrar)
        .env_remove("KOMPOZ_HEADLESS")
        .env_remove("WAYLAND_DISPLAY")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the app starts");
    let deadline = Instant::now() + Duration::from_secs(90);
    let status = loop {
        if let Some(status) = child.try_wait().expect("the app's status") {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the app didn't quit within 90 s");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut stderr = String::new();
    if let Some(mut e) = child.stderr.take() {
        use std::io::Read;
        let _ = e.read_to_string(&mut stderr);
    }
    assert!(status.success(), "the app failed ({status}):\n{stderr}");
    stderr
}

fn shot(dir: &Path, name: &str) -> RgbaImage {
    let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    decode_png(&bytes).unwrap_or_else(|| panic!("{name} decodes"))
}

fn pixel(image: &RgbaImage, x: i32, y: i32) -> [u8; 4] {
    let i = ((y * image.width + x) * 4) as usize;
    [image.rgba[i], image.rgba[i + 1], image.rgba[i + 2], image.rgba[i + 3]]
}

fn windows_on_x11() {
    let dir: PathBuf = std::env::temp_dir().join(format!("kompoz-x11-windows-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let script = "wait 30;click 100 100;wait 60;@İkinci: shot second.png;@Üçüncü: shot third.png;wait 5;\
                  shot main.png;@İkinci: window close;wait 30;shot after.png;quit";
    let log = run_app(&dir, script);
    for line in ["GPU validation:", "no window", "couldn't be opened", "no window titled", "kompoz: vulkan:"] {
        assert!(!log.lines().any(|l| l.contains(line)), "logged '{line}':\n{log}");
    }
    assert_eq!(log.matches("renderer: KompozR on vulkan").count(), 3, "a device each:\n{log}");
    let second = shot(&dir, "second.png");
    assert_eq!((second.width, second.height), (160, 120), "the second window at its state's size");
    assert_eq!(pixel(&second, 80, 60), [255, 0, 0, 255]);
    let third = shot(&dir, "third.png");
    assert_eq!(pixel(&third, 10, 10), [0, 0, 255, 255]);
    assert_eq!(pixel(&shot(&dir, "main.png"), 300, 400), [255, 255, 255, 255]);
    assert!(dir.join("after.png").exists(), "the main window went on after the second closed");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The transparent window with its icon and menu bar on X11: see-through where it draws nothing (its 32-bit
/// visual's), the bar drawn and Alt+D, Enter running its item; with a global menu's registrar, the window registered
/// by its X id with its menu's path, the bar not drawn, Ctrl+B running the item; no X or validation errors.
#[cfg(target_os = "linux")]
fn extras_on_x11() {
    for global in [false, true] {
        let dir: PathBuf = std::env::temp_dir().join(format!("kompoz-x11-extras-{}-{global}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bus = global.then(|| GlobalMenu::start(&dir));
        let run = if global {
            "@Üçüncü: key ctrl+b"
        } else {
            "@Üçüncü: key alt+d;wait 10;@Üçüncü: shot menu.png;@Üçüncü: key enter"
        };
        let script = format!("wait 40;@Üçüncü: shot third.png;{run};wait 10;shot main.png;quit");
        let address = bus.as_ref().map_or("unix:path=/nonexistent".to_string(), |b| b.address.clone());
        let log = run_app_on(&dir, "extras", &script, &address);
        for line in ["GPU validation:", "no window", "couldn't be opened", "kompoz: vulkan:", "X error"] {
            assert!(!log.lines().any(|l| l.contains(line)), "logged '{line}':\n{log}");
        }
        let third = shot(&dir, "third.png");
        assert_eq!((third.width, third.height), (200, 150));
        assert_eq!(pixel(&third, 150, 120)[3], 0, "see-through where it draws nothing");
        assert_eq!(pixel(&third, 20, 90), [255, 0, 0, 255], "its content as drawn");
        if global {
            assert_eq!(pixel(&third, 150, 10)[3], 0, "the global menu's: not drawn");
            let registered = bus.as_ref().unwrap().registered.lock().unwrap().clone();
            assert_eq!(registered.len(), 1, "registered once: {registered:?}");
            assert!(registered[0].0 != 0 && registered[0].1.starts_with("/WindowMenu/"), "{registered:?}");
        } else {
            assert_eq!(pixel(&third, 150, 10), [0xf2, 0xf2, 0xf2, 255], "the menu bar drawn at its top");
            assert_eq!(pixel(&third, 40, 45)[3], 0, "nothing below the bar before");
            assert_eq!(pixel(&shot(&dir, "menu.png"), 40, 45)[3], 255, "its menu open below it");
        }
        assert_eq!(pixel(&shot(&dir, "main.png"), 300, 400), [0, 255, 0, 255], "the menu's item ran");
        drop(bus);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

fn main() -> ExitCode {
    if let Some(child) = std::env::var_os(CHILD) {
        if child == "extras" {
            return App::new("Ekstralar").size(360.dp(), 480.dp()).run(Extras);
        }
        return App::new("Pencereler").size(360.dp(), 480.dp()).run(TwoWindows);
    }
    if std::env::var_os("KOMPOZ_DISPLAY_TESTS").is_none_or(|v| v != "1") {
        println!("skipped: KOMPOZ_DISPLAY_TESTS=1 (and an X display) runs it");
        return ExitCode::SUCCESS;
    }
    assert!(std::env::var_os("DISPLAY").is_some(), "KOMPOZ_DISPLAY_TESTS=1 but DISPLAY isn't set");
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let mut failed = 0;
    #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
    let mut tests: Vec<(&str, fn())> = vec![("windows_on_x11", windows_on_x11)];
    #[cfg(target_os = "linux")]
    tests.push(("extras_on_x11", extras_on_x11));
    for (name, test) in tests {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        let ok = std::panic::catch_unwind(test).is_ok();
        failed += usize::from(!ok);
        println!("{} {name}", if ok { "ok    " } else { "FAILED" });
    }
    if failed == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
