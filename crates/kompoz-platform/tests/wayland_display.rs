//! The Wayland platform on a real compositor (`KOMPOZ_DISPLAY_TESTS=1`; skipped otherwise): KDE's kwin_wayland
//! with a virtual output and no screen locker (else it starts locked while the desktop is, and the window stays
//! suspended), or as `KOMPOZ_TEST_COMPOSITOR` says: `kwin` (the default), `weston` (Weston's headless backend, as CI
//! runs it: lavapipe's WSI presents through wl_shm, which needs no GPU), or a command taking `--socket NAME`; on a
//! session bus and a configuration of its own (kwin writes its outputs' settings: never into the user's), running
//! the Counter in a
//! process of its own (this test run again) with a script: shots before and after a click on its button and after a
//! resize, the GPU's validation on. On lavapipe, on the Intel GPU where there is one, and with the compositor's
//! cursor-shape, fractional-scale, viewporter and decoration globals ignored (the fallbacks: the theme's cursor,
//! the whole buffer scale, KompozR's decorations). Checks: the app ran and quit as its script says, the click changed
//! what is shown, the resized frame is the new size, no validation errors, the connection never broke. And with
//! KompozR's decorations asked for, its title bar: drawn across the top in the system's colors (the baseline light
//! scheme: no desktop services), double-clicked to maximize the window and again to restore it, its close button
//! ending the app.

#![allow(non_snake_case)]

use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_platform::App;
use kompoz_runtime::*;
use kompoz_text::TextStyle;
use kompoz_ui::semantics::Role;
use kompoz_ui::{Alignment, Modifier};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The Counter (examples/counter's).
#[kompozable]
fn Counter() {
    let count = remember_saveable(|| mutable_state_of(0));
    Column(
        modifier = Modifier.fill_max_size().background(Color::White).padding(24.dp()),
        vertical_arrangement = Arrangement::spaced_by(16.dp()),
        content = move || {
            Box(
                modifier = Modifier
                    .size_wh(120.dp(), 80.dp())
                    .background(Color::from_argb(0xff3f51b5), shape = Arc::new(RoundedCornerShape(16.dp())))
                    .clickable(role = Some(Role::Button), on_click = move || count.set(count.get() + 1)),
                content_alignment = Alignment::Center,
                content = || {
                    BasicText(
                        "+1",
                        style = TextStyle { font_size: 28.sp(), color: Color::White, ..TextStyle::Default },
                    );
                },
            );
            BasicText(
                format!("Sayaç: {}", count.get()),
                style = TextStyle { font_size: 32.sp(), color: Color::from_argb(0xff1c1b1f), ..TextStyle::Default },
            );
        },
    );
}

/// The child's mark: it runs the app.
const CHILD: &str = "KOMPOZ_WAYLAND_TEST_CHILD";

/// A compositor of the test's own, stopped (its whole process group) when dropped.
struct Compositor {
    child: Child,
    socket: String,
}

impl Compositor {
    fn start(work: &Path, socket: &str) -> Compositor {
        let config = work.join("config");
        std::fs::create_dir_all(&config).unwrap();
        let chosen = std::env::var("KOMPOZ_TEST_COMPOSITOR").unwrap_or_default();
        let command: Vec<String> = match chosen.trim() {
            "" | "kwin" => {
                "kwin_wayland --virtual --no-lockscreen --no-global-shortcuts --no-kactivities --width 800 --height 600"
                    .split(' ')
                    .chain(["--socket", socket])
                    .map(str::to_string)
                    .collect()
            }
            // (Its options take their values after '='; pixman draws what clients show, from wl_shm buffers.)
            "weston" => [
                "weston",
                "--backend=headless",
                "--renderer=pixman",
                "--width=800",
                "--height=600",
                "--idle-time=0",
                "--no-config",
            ]
            .into_iter()
            .map(str::to_string)
            .chain([format!("--socket={socket}")])
            .collect(),
            other => other.split_whitespace().chain(["--socket", socket]).map(str::to_string).collect(),
        };
        let mut c = Command::new("dbus-run-session");
        c.arg("--").args(&command);
        c.env("XDG_CONFIG_HOME", &config)
            .env("XDG_DATA_HOME", work.join("data"))
            .env("XDG_CACHE_HOME", work.join("cache"));
        c.env_remove("WAYLAND_DISPLAY").env_remove("DISPLAY").env_remove("DBUS_SESSION_BUS_ADDRESS");
        c.stdout(Stdio::null()).stderr(std::fs::File::create(work.join("compositor.log")).unwrap());
        std::os::unix::process::CommandExt::process_group(&mut c, 0);
        let child = c.spawn().expect("dbus-run-session and the compositor");
        let compositor = Compositor { child, socket: socket.to_string() };
        let runtime = PathBuf::from(std::env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR"));
        let start = Instant::now();
        while !runtime.join(socket).exists() {
            assert!(start.elapsed() < Duration::from_secs(20), "the compositor didn't start (see compositor.log)");
            std::thread::sleep(Duration::from_millis(50));
        }
        std::thread::sleep(Duration::from_millis(300)); // (its globals and outputs set up)
        compositor
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        let group = format!("-{}", self.child.id());
        let _ = Command::new("kill").args(["-TERM", "--", &group]).status();
        // (Until every process of the group is gone: the compositor writes its settings on its way out.)
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

/// A PNG's size (its IHDR's).
fn png_size(path: &Path) -> (u32, u32) {
    let data = std::fs::read(path).unwrap_or_else(|_| panic!("no {}", path.display()));
    let word = |at: usize| u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
    (word(16), word(20))
}

/// What a run does: the Counter's script (shots before and after a click, a resize) or the title bar's.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Drive {
    Counter,
    TitleBar,
}

/// The Counter's script: shots before and after a click on its button (70, 70 dp: in it with a title bar above it
/// or without), and after a resize.
fn counter_script(work: &Path) -> String {
    format!(
        "wait 30;shot {0}/before.png;click 70 70;wait 30;shot {0}/after.png;resize 500 400;wait 30;shot \
         {0}/resized.png;quit",
        work.display()
    )
}

/// The title bar's script (a 360 dp wide window, the default buttons on the right: Close's middle 22 dp from the
/// end): a shot, a double click on the bar (two taps 4 frames apart) and a shot of the window maximized, another
/// and one restored, a click on Close; a shot after it (and a quit) only if it didn't end the app.
fn title_bar_script(work: &Path) -> String {
    format!(
        "wait 30;shot {0}/bar.png;click 120 20;wait 4;click 120 20;wait 60;shot {0}/maximized.png;click 120 20;wait \
         4;click 120 20;wait 60;shot {0}/restored.png;click 338 20;wait 60;shot {0}/not-closed.png;quit",
        work.display()
    )
}

/// The Counter on `compositor` with `env`, driven by `script`: what it logged.
fn run_counter(compositor: &Compositor, work: &Path, env: &[(&str, &str)], script: String) -> String {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.env(CHILD, "1")
        .env("WAYLAND_DISPLAY", &compositor.socket)
        .env("KOMPOZ_SCRIPT", script)
        .env("KOMPOZ_GPU_VALIDATION", "1")
        .env("KOMPOZ_NO_WALLPAPER_COLORS", "1")
        .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent") // (the desktop's services: none)
        .env("XDG_CACHE_HOME", work.join("cache"))
        .env_remove("KOMPOZ_HEADLESS")
        .env_remove("KOMPOZ_PLATFORM");
    for (k, v) in env {
        c.env(k, v);
    }
    let log = work.join("counter.log");
    c.stdout(std::fs::File::create(&log).unwrap()).stderr(std::fs::File::create(work.join("counter.err")).unwrap());
    let mut child = c.spawn().unwrap();
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the Counter didn't quit");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let out = std::fs::read_to_string(work.join("counter.err")).unwrap_or_default();
    assert!(status.success(), "the Counter failed ({status}):\n{out}");
    out
}

/// The Counter's run checked: shots taken, the click seen, the resize made, nothing wrong logged.
fn check(work: &Path, out: &str) {
    for line in ["screenshot", "the connection broke", "GPU validation:", "no window"] {
        let found = out.lines().any(|l| l.contains(line));
        assert_eq!(found, line == "screenshot", "{line:?} in:\n{out}");
    }
    assert_eq!(png_size(&work.join("before.png")), (360, 480));
    assert_eq!(png_size(&work.join("after.png")), (360, 480));
    assert_eq!(png_size(&work.join("resized.png")), (500, 400));
    let before = std::fs::read(work.join("before.png")).unwrap();
    let after = std::fs::read(work.join("after.png")).unwrap();
    assert_ne!(before, after, "the click changed nothing");
}

/// A PNG's pixel at (x, y): RGBA.
fn pixel(path: &Path, x: usize, y: usize) -> [u8; 4] {
    let bytes = std::fs::read(path).unwrap_or_else(|_| panic!("no {}", path.display()));
    let image = decode_image(&bytes, ImageRequestSize::default()).expect("a PNG");
    let pixels = image.pixels();
    let at = (y * image.width() as usize + x) * 4;
    [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
}

/// Whether `pixel` is `color` (within 2 a channel: the GPU's rounding).
fn near(pixel: [u8; 4], color: u32) -> bool {
    let want = color.to_be_bytes(); // (ARGB)
    let rgba = [want[1], want[2], want[3], want[0]];
    pixel.iter().zip(rgba).all(|(a, b)| a.abs_diff(b) <= 2)
}

/// The title bar's run checked: the bar across the top in the baseline light scheme's surfaceContainer (F3EDF7),
/// its buttons' circles in surfaceContainerHighest (E6E0E9), the Counter's white below it; maximized by a double
/// click (bigger than the window was), restored by another; Close ended the app (no shot after it).
fn check_title_bar(work: &Path, out: &str) {
    for line in ["the connection broke", "GPU validation:", "no window"] {
        assert!(!out.lines().any(|l| l.contains(line)), "{line:?} in:\n{out}");
    }
    let bar = work.join("bar.png");
    assert_eq!(png_size(&bar), (360, 480));
    for (x, y) in [(5, 5), (120, 20), (180, 34), (355, 39)] {
        assert!(near(pixel(&bar, x, y), 0xFFF3EDF7), "the bar at {x},{y}: {:?}", pixel(&bar, x, y));
    }
    // Close's circle (28 dp round its middle at 338, 20), left of its icon (16 dp); Maximize's and Minimize's.
    for (x, y) in [(327, 20), (291, 20), (255, 20)] {
        assert!(near(pixel(&bar, x, y), 0xFFE6E0E9), "a button at {x},{y}: {:?}", pixel(&bar, x, y));
    }
    assert!(near(pixel(&bar, 180, 45), 0xFFFFFFFF), "the Counter below the bar: {:?}", pixel(&bar, 180, 45));
    let (w, h) = png_size(&work.join("maximized.png"));
    assert!(w > 360 && h > 480, "the double click didn't maximize the window ({w} by {h})");
    assert_eq!(png_size(&work.join("restored.png")), (360, 480));
    assert!(!work.join("not-closed.png").exists(), "Close didn't end the app");
}

/// Mesa's lavapipe's ICD file: `lvp_icd.json`, or `lvp_icd.x86_64.json` as Debian and Ubuntu name it.
fn lavapipe_icd() -> String {
    let dir = Path::new("/usr/share/vulkan/icd.d");
    ["lvp_icd.json".to_string(), format!("lvp_icd.{}.json", std::env::consts::ARCH)]
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.exists())
        .map(|path| path.display().to_string())
        .expect("lavapipe (Mesa's lvp_icd.json) for the display tests")
}

fn intel_icd() -> Option<&'static str> {
    let path = "/usr/share/vulkan/icd.d/intel_icd.json";
    Path::new(path).exists().then_some(path)
}

fn main() -> ExitCode {
    if std::env::var_os(CHILD).is_some() {
        return App::new("Sayaç").size(360.dp(), 480.dp()).run(Counter);
    }
    if std::env::var("KOMPOZ_DISPLAY_TESTS").as_deref() != Ok("1") {
        println!("skipped: the Wayland display tests run with KOMPOZ_DISPLAY_TESTS=1");
        return ExitCode::SUCCESS;
    }
    let lavapipe = lavapipe_icd();
    let lavapipe = lavapipe.as_str();
    let mut runs: Vec<(&str, Drive, Vec<(&str, &str)>)> =
        vec![("lavapipe", Drive::Counter, vec![("VK_DRIVER_FILES", lavapipe)])];
    if let Some(intel) = intel_icd() {
        runs.push(("intel", Drive::Counter, vec![("VK_DRIVER_FILES", intel)]));
    }
    runs.push((
        "fallbacks",
        Drive::Counter,
        vec![
            ("VK_DRIVER_FILES", lavapipe),
            (
                "KOMPOZ_WAYLAND_IGNORE",
                "wp_cursor_shape_manager_v1,wp_fractional_scale_manager_v1,wp_viewporter,zxdg_decoration_manager_v1",
            ),
        ],
    ));
    runs.push((
        "kompoz decorations, scale 1.5",
        Drive::Counter,
        vec![("VK_DRIVER_FILES", lavapipe), ("KOMPOZ_DECORATIONS", "kompoz"), ("KOMPOZ_SCALE", "1.5")],
    ));
    runs.push((
        "kompoz title bar",
        Drive::TitleBar,
        vec![("VK_DRIVER_FILES", lavapipe), ("KOMPOZ_DECORATIONS", "kompoz")],
    ));
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let mut failed = 0;
    for (name, drive, env) in runs {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        let work =
            std::env::temp_dir().join(format!("kompoz-wayland-test-{}-{}", std::process::id(), name.replace(' ', "-")));
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).unwrap();
        let ok = std::panic::catch_unwind(|| {
            let compositor = Compositor::start(&work, &format!("kompoz-test-{}", std::process::id()));
            match drive {
                Drive::Counter => {
                    let out = run_counter(&compositor, &work, &env, counter_script(&work));
                    check(&work, &out);
                }
                Drive::TitleBar => {
                    let out = run_counter(&compositor, &work, &env, title_bar_script(&work));
                    check_title_bar(&work, &out);
                }
            }
        })
        .is_ok();
        if ok {
            let _ = std::fs::remove_dir_all(&work);
        }
        failed += usize::from(!ok);
        println!(
            "{} counter on {name}{}",
            if ok { "ok    " } else { "FAILED" },
            if ok { String::new() } else { format!(" (kept: {})", work.display()) }
        );
    }
    if failed > 0 { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
