//! The Counter on macOS in a real window (`KOMPOZ_DISPLAY_TESTS=1`; CI's macos runner): its NSWindow drawn into with
//! Metal through its view's CAMetalLayer, a script clicking its button and resizing its window; its shots are the
//! headless Counter's, pixel for pixel (drawn here, in this process, on the headless platform: the same fonts, the
//! same device), the click changing them, the resized one the new size; no validation errors where validation is
//! asked for (`KOMPOZ_GPU_VALIDATION=1`, `MTL_DEBUG_LAYER=1`).

#![allow(non_snake_case)]

#[cfg(target_os = "macos")]
#[path = "../src/counter.rs"]
mod counter;

#[cfg(target_os = "macos")]
mod on_macos {
    use super::counter::Counter;
    use kompoz_graphics::{RgbaImage, UnitExt, decode_png};
    use kompoz_platform::headless::{HeadlessPlatform, run_core};
    use kompoz_platform::{App, AppCore};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    /// The script CI runs the Counter with: a shot, a click on its button, another shot, the window resized, a
    /// third.
    fn script(dir: &Path, resize: bool) -> String {
        let at = |name: &str| dir.join(name).display().to_string();
        let mut s = format!("wait 30;shot {};click 70 70;wait 30;shot {}", at("before.png"), at("after.png"));
        if resize {
            s += &format!(";resize 500 400;sleep 1500;wait 10;shot {}", at("resized.png"));
        }
        s + ";quit"
    }

    fn shots_dir(name: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("counter-macos-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a directory for the shots");
        dir
    }

    fn shot(dir: &Path, name: &str) -> RgbaImage {
        let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        decode_png(&bytes).unwrap_or_else(|| panic!("{name} decodes"))
    }

    /// The Counter run by the script on the headless platform, here.
    fn run_headless(dir: &Path) {
        let mut platform = HeadlessPlatform::new();
        platform.set_option("KOMPOZ_SCRIPT", &script(dir, false));
        let app = App::new("Sayaç").size(360.dp(), 480.dp());
        let mut core = AppCore::new(platform, app.config().clone(), Rc::new(Counter));
        assert!(core.start(), "the headless Counter starts");
        let queue = core.platform().queue();
        run_core(&mut core, &queue);
    }

    /// The Counter's binary run in a window by the script; its stderr. Fails if it doesn't end well within two
    /// minutes.
    fn run_window(dir: &Path) -> String {
        let mut child = Command::new(env!("CARGO_BIN_EXE_counter"))
            .current_dir(dir)
            .env("KOMPOZ_SCALE", "1")
            .env("KOMPOZ_SCRIPT", script(dir, true))
            .env_remove("KOMPOZ_HEADLESS")
            .env_remove("KOMPOZ_PLATFORM")
            .env_remove("KOMPOZ_STATE_FILE")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the Counter starts");
        let deadline = Instant::now() + Duration::from_secs(120);
        let status = loop {
            if let Some(status) = child.try_wait().expect("the Counter's status") {
                break status;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the Counter didn't quit within two minutes");
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let mut stderr = String::new();
        if let Some(mut e) = child.stderr.take() {
            use std::io::Read;
            let _ = e.read_to_string(&mut stderr);
        }
        assert!(status.success(), "the Counter failed ({status}):\n{stderr}");
        stderr
    }

    pub fn the_counter_runs_in_a_window() {
        let (window, headless) = (shots_dir("window"), shots_dir("headless"));
        run_headless(&headless);
        let log = run_window(&window);
        println!("{log}");
        assert!(log.contains("kompoz: renderer: KompozR on metal"), "no renderer:\n{log}");
        assert!(!log.contains("GPU validation:"), "logged validation errors:\n{log}");
        for name in ["before.png", "after.png"] {
            let (w, h) = (shot(&window, name), shot(&headless, name));
            assert_eq!((w.width, w.height), (360, 480), "{name}: the window's size");
            let differ = w.rgba.chunks(4).zip(h.rgba.chunks(4)).filter(|(a, b)| a != b).count();
            assert_eq!(differ, 0, "{name}: {differ} pixels differ from the headless Counter's ({})", window.display());
        }
        assert_ne!(shot(&window, "before.png").rgba, shot(&window, "after.png").rgba, "the click changed nothing");
        let resized = shot(&window, "resized.png");
        assert_eq!((resized.width, resized.height), (500, 400), "the window wasn't resized");
    }
}

#[cfg(target_os = "macos")]
fn main() {
    use kompoz_gpu::device::{self, Backend, Surface};
    kompoz_runtime::set_ui_thread();
    if std::env::var_os("KOMPOZ_DISPLAY_TESTS").is_none_or(|v| v != "1") {
        println!("skipped: KOMPOZ_DISPLAY_TESTS=1 runs it");
        return;
    }
    let required = std::env::var_os("KOMPOZ_GPU_REQUIRED").is_some_and(|v| v == "1");
    match device::create(Backend::Metal, Surface::default()) {
        Ok(d) => println!("device: {}", d.device_name()),
        Err(why) if required => panic!("no GPU device (KOMPOZ_GPU_REQUIRED=1): {why}"),
        Err(why) => {
            println!("skipped the_counter_runs_in_a_window: {why}");
            return;
        }
    }
    let ok = std::panic::catch_unwind(on_macos::the_counter_runs_in_a_window).is_ok();
    println!("{} the_counter_runs_in_a_window", if ok { "ok    " } else { "FAILED" });
    if !ok {
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("skipped: the Counter's macOS test runs on macOS");
}
