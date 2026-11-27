//! The macOS platform on a real display (`KOMPOZ_DISPLAY_TESTS=1`; on macOS — CI's macos runner —, skipped
//! otherwise), each app in a process of its own (this test run again), `KOMPOZ_SCALE=1` (shots of known sizes on
//! any display), Metal's validation where the environment asks for it (`MTL_DEBUG_LAYER=1`):
//! - the Counter run by CI's script in an NSWindow: a shot, a click on its button, another, the window resized, a
//!   third — the click changed what is shown, the shots the window's size, the resized one the new size;
//! - the app's windows: a click opening two `Window`s of their own (an NSWindow and a Metal device each), one at its
//!   state's size and place, one zoomed (maximized) as its state says, a script reaching each by its title, the
//!   first's close its `on_close_request` (it closes, the main window going on);
//! - a transparent, borderless window with an icon (the Dock's) and a menu bar (the menu bar's: macOS's own, not
//!   drawn in the window): see-through where it draws nothing, its content as drawn.
//!
//! Each run: the app ended as its script says, a renderer on Metal for each window, nothing wrong logged.

#![allow(non_snake_case)]

#[cfg(target_os = "macos")]
mod on_macos {
    use kompoz_foundation::prelude::*;
    use kompoz_graphics::*;
    use kompoz_runtime::*;
    use kompoz_text::TextStyle;
    use kompoz_ui::semantics::Role;
    use kompoz_ui::window::{FrameWindowScope, Window, WindowPlacement, WindowPosition, remember_window_state};
    use kompoz_ui::{Alignment, Modifier};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::rc::Rc;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    /// The app the child process runs (its value: "counter", "windows", "extras").
    pub const CHILD: &str = "KOMPOZ_MACOS_DISPLAY_CHILD";

    /// The Counter (examples/counter's).
    #[kompozable]
    pub fn Counter() {
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

    /// A transparent, undecorated window of its own with an icon and a menu bar (Dosya: Bitti, Cmd+B), over the main
    /// window (white); its content a red square 40 dp down.
    #[kompozable]
    pub fn Extras() {
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
                scope.MenuBar(move |bar: &kompoz_ui::window::MenuBarScope| {
                    let d = d.clone();
                    bar.Menu(
                        text = "Dosya".to_string(),
                        mnemonic = Some('D'),
                        content = move |m: &kompoz_ui::window::MenuScope| {
                            let d = d.clone();
                            m.Item(
                                text = "Bitti".to_string(),
                                shortcut = Some(kompoz_ui::window::KeyShortcut(kompoz_ui::input::Key::B, meta = true)),
                                on_click = move || d.set(true),
                            );
                        },
                    );
                });
                Box(modifier = Modifier.padding_sides(top = 40.dp()).size(60.dp()).background(Color::Red));
            },
        );
    }

    /// The app `which` run with `script` in `dir`: what it logged. Fails if it doesn't end well within two minutes.
    fn run_app(which: &str, dir: &Path, script: &str) -> String {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .current_dir(dir)
            .env(CHILD, which)
            .env("KOMPOZ_SCALE", "1")
            .env("KOMPOZ_SCRIPT", script)
            .env("KOMPOZ_NO_DESKTOP_ACCENT", "1")
            .env_remove("KOMPOZ_HEADLESS")
            .env_remove("KOMPOZ_PLATFORM")
            .env_remove("KOMPOZ_STATE_FILE")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the app starts");
        let reader = child.stderr.take().map(|mut e| {
            std::thread::spawn(move || {
                use std::io::Read;
                let mut s = String::new();
                let _ = e.read_to_string(&mut s);
                s
            })
        });
        let deadline = Instant::now() + Duration::from_secs(120);
        let status = loop {
            if let Some(status) = child.try_wait().expect("the app's status") {
                break status;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the app '{which}' didn't quit within two minutes");
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let log = reader.map(|r| r.join().unwrap_or_default()).unwrap_or_default();
        assert!(status.success(), "the app '{which}' failed ({status}):\n{log}");
        log
    }

    fn dir(name: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("macos-display").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a directory for the shots");
        dir
    }

    fn shot(dir: &Path, name: &str) -> RgbaImage {
        let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        decode_png(&bytes).unwrap_or_else(|| panic!("{name} decodes"))
    }

    fn pixel(image: &RgbaImage, x: i32, y: i32) -> [u8; 4] {
        let i = ((y * image.width + x) * 4) as usize;
        [image.rgba[i], image.rgba[i + 1], image.rgba[i + 2], image.rgba[i + 3]]
    }

    /// Nothing wrong logged, and `renderers` renderers on Metal.
    fn check_log(log: &str, renderers: usize) {
        for line in ["GPU validation:", "no window", "couldn't be opened", "no window titled"] {
            assert!(!log.lines().any(|l| l.contains(line)), "logged '{line}':\n{log}");
        }
        assert_eq!(log.matches("renderer: KompozR on metal").count(), renderers, "a device each:\n{log}");
    }

    pub fn the_counter_in_a_window() {
        let dir = dir("counter");
        let script = "wait 30;shot before.png;click 70 70;wait 30;shot after.png;resize 500 400;sleep 1500;wait 10;\
                      shot resized.png;quit";
        let log = run_app("counter", &dir, script);
        println!("{log}");
        check_log(&log, 1);
        let (before, after) = (shot(&dir, "before.png"), shot(&dir, "after.png"));
        assert_eq!((before.width, before.height), (360, 480), "the window's size");
        assert_ne!(before.rgba, after.rgba, "the click changed nothing");
        assert_eq!(pixel(&before, 300, 400), [255, 255, 255, 255], "its background");
        let resized = shot(&dir, "resized.png");
        assert_eq!((resized.width, resized.height), (500, 400), "the window wasn't resized");
    }

    pub fn several_windows() {
        let dir = dir("windows");
        let script = "wait 30;click 100 100;wait 60;@İkinci: shot second.png;@Üçüncü: shot third.png;wait 5;\
                      shot main.png;@İkinci: window close;wait 30;shot after.png;quit";
        let log = run_app("windows", &dir, script);
        println!("{log}");
        check_log(&log, 3);
        let second = shot(&dir, "second.png");
        assert_eq!((second.width, second.height), (160, 120), "the second window at its state's size");
        assert_eq!(pixel(&second, 80, 60), [255, 0, 0, 255]);
        let third = shot(&dir, "third.png");
        assert!(third.width > 100 && third.height > 80, "the third zoomed: {}x{}", third.width, third.height);
        assert_eq!(pixel(&third, 10, 10), [0, 0, 255, 255]);
        assert_eq!(pixel(&shot(&dir, "main.png"), 300, 400), [255, 255, 255, 255]);
        assert!(dir.join("after.png").exists(), "the main window went on after the second closed");
    }

    pub fn a_transparent_window_with_an_icon_and_a_menu_bar() {
        let dir = dir("extras");
        let script = "wait 40;@Üçüncü: shot third.png;wait 10;shot main.png;quit";
        let log = run_app("extras", &dir, script);
        println!("{log}");
        check_log(&log, 2);
        let third = shot(&dir, "third.png");
        assert_eq!((third.width, third.height), (200, 150));
        assert_eq!(pixel(&third, 150, 120)[3], 0, "see-through where it draws nothing");
        assert_eq!(pixel(&third, 20, 90), [255, 0, 0, 255], "its content as drawn");
        assert_eq!(pixel(&third, 150, 10)[3], 0, "its menu bar macOS's, not drawn in it");
        assert_eq!(pixel(&shot(&dir, "main.png"), 300, 400), [255, 255, 255, 255]);
    }
}

#[cfg(target_os = "macos")]
fn main() -> std::process::ExitCode {
    use kompoz_graphics::UnitExt;
    use kompoz_platform::App;
    if let Some(which) = std::env::var_os(on_macos::CHILD) {
        let app = App::new("Pencereler").size(360.dp(), 480.dp());
        return match which.to_str() {
            Some("counter") => App::new("Sayaç").size(360.dp(), 480.dp()).run(on_macos::Counter),
            Some("windows") => app.run(on_macos::TwoWindows),
            _ => app.run(on_macos::Extras),
        };
    }
    if std::env::var_os("KOMPOZ_DISPLAY_TESTS").is_none_or(|v| v != "1") {
        println!("skipped: KOMPOZ_DISPLAY_TESTS=1 runs it");
        return std::process::ExitCode::SUCCESS;
    }
    let tests: [(&str, fn()); 3] = [
        ("the_counter_in_a_window", on_macos::the_counter_in_a_window),
        ("several_windows", on_macos::several_windows),
        (
            "a_transparent_window_with_an_icon_and_a_menu_bar",
            on_macos::a_transparent_window_with_an_icon_and_a_menu_bar,
        ),
    ];
    let mut failed = 0;
    for (name, test) in tests {
        let ok = std::panic::catch_unwind(test).is_ok();
        println!("{} {name}", if ok { "ok    " } else { "FAILED" });
        failed += usize::from(!ok);
    }
    if failed == 0 { std::process::ExitCode::SUCCESS } else { std::process::ExitCode::FAILURE }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("skipped: the macOS platform's display tests run on macOS");
}
