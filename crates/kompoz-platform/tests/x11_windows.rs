//! The app's windows on X11 on a real display (`KOMPOZ_DISPLAY_TESTS=1`, with `DISPLAY` naming an X server: Xvfb in
//! CI, kwin's Xwayland here; skipped otherwise): one connection, an X window each, a Vulkan device each. A click
//! opens a `Window` of its own (its state's size and place, its title on the X window), a script reaches it by its
//! title, the window manager's close (`window close`) is its `on_close_request` (it closes, the main window going on),
//! another opened minimized and maximized as its state says; no validation errors. Run in a process of its own (this
//! test run again).

#![allow(non_snake_case)]

use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_platform::App;
use kompoz_runtime::*;
use kompoz_ui::Modifier;
use kompoz_ui::window::{FrameWindowScope, Window, WindowPlacement, WindowPosition, remember_window_state};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
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
    let mut child = Command::new(std::env::current_exe().unwrap())
        .current_dir(dir)
        .env(CHILD, "1")
        .env("KOMPOZ_PLATFORM", "x11")
        .env("KOMPOZ_GPU_VALIDATION", "1")
        .env("KOMPOZ_SCALE", "1")
        .env("KOMPOZ_SCRIPT", script)
        .env("VK_DRIVER_FILES", lavapipe_icd())
        .env("KOMPOZ_NO_WALLPAPER_COLORS", "1")
        .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent") // (the desktop's services: none)
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

fn main() -> ExitCode {
    if std::env::var_os(CHILD).is_some() {
        return App::new("Pencereler").size(360.dp(), 480.dp()).run(TwoWindows);
    }
    if std::env::var_os("KOMPOZ_DISPLAY_TESTS").is_none_or(|v| v != "1") {
        println!("skipped: KOMPOZ_DISPLAY_TESTS=1 (and an X display) runs it");
        return ExitCode::SUCCESS;
    }
    assert!(std::env::var_os("DISPLAY").is_some(), "KOMPOZ_DISPLAY_TESTS=1 but DISPLAY isn't set");
    let ok = std::panic::catch_unwind(windows_on_x11).is_ok();
    println!("{} windows_on_x11", if ok { "ok    " } else { "FAILED" });
    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
