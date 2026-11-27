//! The showcase on macOS in a real window (`KOMPOZ_DISPLAY_TESTS=1`; CI's macos runner): run in its window by the
//! headless test's script (its theme switched for the dark run, the Material tab opened, its first pages shot as the
//! page is dragged up), and run headless by the same script, the same binary, the same fonts (the system's:
//! CoreText's), the same device: the window's shots are the headless run's, pixel for pixel.

#[cfg(target_os = "macos")]
mod on_macos {
    use showcase::shots::{APP_HEIGHT, APP_WIDTH, Config, DRAG, DRAG_FROM, DRAG_X, Picture};
    use std::fmt::Write;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    /// The pages shot: the Material tab's first, and the next as the test drags it up.
    const PAGES: usize = 2;

    /// The headless test's script (shots::script) for the first pages: the window made the app's size, the theme
    /// switched for a dark run, the Material tab opened, each page shot.
    fn script(config: Config) -> String {
        let mut s = String::new();
        let _ = write!(s, "resize {APP_WIDTH} {APP_HEIGHT}; wait 10; ");
        if config.dark {
            s.push_str("click 316 32; wait 40; ");
        }
        s.push_str("click 385 830; wait 60; shot 00.png; ");
        for page in 1..PAGES {
            let _ = write!(s, "down {DRAG_X} {DRAG_FROM}; wait 1; ");
            for k in 1..=DRAG / 50 {
                let _ = write!(s, "move {DRAG_X} {}; wait 1; ", DRAG_FROM - 50 * k);
            }
            let _ = write!(s, "wait 6; up {DRAG_X} {}; wait 60; shot {page:02}.png; ", DRAG_FROM - DRAG);
        }
        s.push_str("quit");
        s
    }

    /// The showcase run by `script` in `dir`, headless or in its window; its stderr.
    fn run(dir: &Path, script: &str, headless: bool) -> String {
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(dir).expect("a directory for the shots");
        let mut command = Command::new(env!("CARGO_BIN_EXE_showcase"));
        if headless {
            command.env("KOMPOZ_HEADLESS", "1");
        } else {
            command.env_remove("KOMPOZ_HEADLESS");
        }
        let mut child = command
            .current_dir(dir)
            .env("KOMPOZ_SCRIPT", script)
            .env("KOMPOZ_SCALE", "1")
            .env("KOMPOZ_LANGUAGE", "tr")
            .env("KOMPOZ_LAYOUT_DIRECTION", "ltr")
            .env("KOMPOZ_NO_DESKTOP_ACCENT", "1")
            .env("KOMPOZ_NO_WALLPAPER_COLORS", "1")
            .env_remove("KOMPOZ_PLATFORM")
            .env_remove("KOMPOZ_STATE_FILE")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the showcase starts");
        let reader = child.stderr.take().map(|mut e| {
            std::thread::spawn(move || {
                use std::io::Read;
                let mut s = String::new();
                let _ = e.read_to_string(&mut s);
                s
            })
        });
        let deadline = Instant::now() + Duration::from_secs(300);
        let status = loop {
            if let Some(status) = child.try_wait().expect("the showcase's status") {
                break status;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the showcase didn't quit within five minutes");
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let log = reader.map(|r| r.join().unwrap_or_default()).unwrap_or_default();
        assert!(status.success(), "the showcase failed ({status}):\n{log}");
        log
    }

    fn dir(name: &str) -> PathBuf {
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("showcase-macos-{name}"))
    }

    pub fn the_showcase_runs_in_a_window() {
        for config in [Config { dark: false, scale: 1 }, Config { dark: true, scale: 1 }] {
            let script = script(config);
            let (window, headless) = (dir(&format!("{}-window", config.name())), dir(&config.name()));
            let log = run(&headless, &script, true);
            assert!(!log.contains("GPU validation"), "validation errors:\n{log}");
            let log = run(&window, &script, false);
            println!("{log}");
            assert!(log.contains("kompoz: renderer: KompozR on metal"), "no renderer:\n{log}");
            assert!(!log.contains("GPU validation"), "validation errors:\n{log}");
            for page in 0..PAGES {
                let name = format!("{page:02}.png");
                let w = Picture::load(&window.join(&name)).unwrap_or_else(|| panic!("{name} in the window's run"));
                let h = Picture::load(&headless.join(&name)).unwrap_or_else(|| panic!("{name} in the headless run"));
                assert_eq!(
                    (w.width, w.height),
                    (APP_WIDTH, APP_HEIGHT),
                    "{}: {name}: the window's size",
                    config.name()
                );
                assert_eq!((w.width, w.height), (h.width, h.height), "{name}");
                let differ = w.rgba.chunks(4).zip(h.rgba.chunks(4)).filter(|(a, b)| a != b).count();
                assert_eq!(differ, 0, "{}: {name}: {differ} pixels differ ({})", config.name(), window.display());
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn main() {
    use kompoz::gpu::device::{self, Backend, Surface};
    if std::env::var_os("KOMPOZ_DISPLAY_TESTS").is_none_or(|v| v != "1") {
        println!("skipped: KOMPOZ_DISPLAY_TESTS=1 runs it");
        return;
    }
    let required = std::env::var_os("KOMPOZ_GPU_REQUIRED").is_some_and(|v| v == "1");
    match device::create(Backend::Metal, Surface::default()) {
        Ok(d) => println!("device: {}", d.device_name()),
        Err(why) if required => panic!("no GPU device (KOMPOZ_GPU_REQUIRED=1): {why}"),
        Err(why) => {
            println!("skipped the_showcase_runs_in_a_window: {why}");
            return;
        }
    }
    let ok = std::panic::catch_unwind(on_macos::the_showcase_runs_in_a_window).is_ok();
    println!("{} the_showcase_runs_in_a_window", if ok { "ok    " } else { "FAILED" });
    if !ok {
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("skipped: the showcase's macOS test runs on macOS");
}
