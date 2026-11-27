//! The app's windows on Windows (`KOMPOZ_DISPLAY_TESTS=1`; on Windows, and under wine here; skipped otherwise): one
//! message loop, a Win32 window each, a Vulkan device each. A click opens a `Window` of its own (its state's size
//! and place, its title on the window), a script reaches it by its title, its close (`window close`) is its
//! `on_close_request` (it closes, the main window going on), another opened maximized as its state says; no
//! validation errors. Run in a process of its own (this test run again).

#![allow(non_snake_case)]

#[cfg(windows)]
mod on_windows {
    use kompoz_foundation::prelude::*;
    use kompoz_graphics::*;
    use kompoz_runtime::*;
    use kompoz_ui::Modifier;
    use kompoz_ui::window::{FrameWindowScope, Window, WindowPlacement, WindowPosition, remember_window_state};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    pub const CHILD: &str = "KOMPOZ_WIN32_WINDOWS_CHILD";

    /// The main window white; a click shows "İkinci" (160 by 120 dp at 40, 50, red) and "Üçüncü" (maximized, blue);
    /// the second's close request takes it out.
    #[kompozable]
    pub fn TwoWindows() {
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

    /// The app run with `script` in `dir`: what it logged.
    fn run_app(dir: &Path, script: &str) -> String {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .current_dir(dir)
            .env(CHILD, "1")
            .env("KOMPOZ_SCALE", "1")
            .env("KOMPOZ_SCRIPT", script)
            .env("KOMPOZ_NO_WALLPAPER_COLORS", "1")
            .env_remove("KOMPOZ_HEADLESS")
            .env_remove("KOMPOZ_PLATFORM")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the app starts");
        let deadline = Instant::now() + Duration::from_secs(120);
        let status = loop {
            if let Some(status) = child.try_wait().expect("the app's status") {
                break status;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the app didn't quit within two minutes");
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

    pub fn windows_on_windows() {
        let dir: PathBuf = Path::new(env!("CARGO_TARGET_TMPDIR")).join("win32-windows");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = "wait 30;click 100 100;wait 60;@İkinci: shot second.png;@Üçüncü: shot third.png;wait 5;\
                      shot main.png;@İkinci: window close;wait 30;shot after.png;quit";
        let log = run_app(&dir, script);
        println!("{log}");
        for line in ["GPU validation:", "no window", "couldn't be opened", "no window titled", "kompoz: vulkan:"] {
            assert!(!log.lines().any(|l| l.contains(line)), "logged '{line}':\n{log}");
        }
        assert_eq!(log.matches("renderer: KompozR on vulkan").count(), 3, "a device each:\n{log}");
        let second = shot(&dir, "second.png");
        assert_eq!((second.width, second.height), (160, 120), "the second window at its state's size");
        assert_eq!(pixel(&second, 80, 60), [255, 0, 0, 255]);
        let third = shot(&dir, "third.png");
        assert!(third.width > 100 && third.height > 80, "the third maximized: {}x{}", third.width, third.height);
        assert_eq!(pixel(&third, 10, 10), [0, 0, 255, 255]);
        assert_eq!(pixel(&shot(&dir, "main.png"), 300, 400), [255, 255, 255, 255]);
        assert!(dir.join("after.png").exists(), "the main window went on after the second closed");
    }
}

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    use kompoz_graphics::UnitExt;
    if std::env::var_os(on_windows::CHILD).is_some() {
        return kompoz_platform::App::new("Pencereler").size(360.dp(), 480.dp()).run(on_windows::TwoWindows);
    }
    if std::env::var_os("KOMPOZ_DISPLAY_TESTS").is_none_or(|v| v != "1") {
        println!("skipped: KOMPOZ_DISPLAY_TESTS=1 runs it");
        return std::process::ExitCode::SUCCESS;
    }
    let ok = std::panic::catch_unwind(on_windows::windows_on_windows).is_ok();
    println!("{} windows_on_windows", if ok { "ok    " } else { "FAILED" });
    if ok { std::process::ExitCode::SUCCESS } else { std::process::ExitCode::FAILURE }
}

#[cfg(not(windows))]
fn main() {
    println!("skipped: the Windows platform's windows run on Windows");
}
