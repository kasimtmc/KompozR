//! The Kotlin Counter on Windows in a real window (`KOMPOZ_DISPLAY_TESTS=1`; on Windows, and under wine here), run
//! by the script CI gives the Rust Counter: its shots, before the click and after, are the Rust Counter's drawn
//! headless in this process (the same fonts, the same device), pixel for pixel; the click changed them. Without a
//! GPU device it is skipped, unless `KOMPOZ_GPU_REQUIRED=1`.

#![allow(non_snake_case)]

#[cfg(windows)]
#[path = "../../counter/src/counter.rs"]
mod rust_counter;

#[cfg(windows)]
mod on_windows {
    use super::rust_counter::Counter;
    use kompoz::platform::AppCore;
    use kompoz::platform::headless::{HeadlessPlatform, run_core};
    use kompoz::prelude::*;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    fn script(dir: &Path) -> String {
        let at = |name: &str| dir.join(name).display().to_string();
        format!("wait 30;shot {};click 70 70;wait 30;shot {};quit", at("before.png"), at("after.png"))
    }

    fn shots_dir(name: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("counter-kotlin-windows-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a directory for the shots");
        dir
    }

    fn shot(dir: &Path, name: &str) -> RgbaImage {
        let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        decode_png(&bytes).unwrap_or_else(|| panic!("{name} decodes"))
    }

    fn run_rust_counter(dir: &Path) {
        let mut platform = HeadlessPlatform::new();
        platform.set_option("KOMPOZ_SCRIPT", &script(dir));
        let app = App::new("Sayaç").size(360.dp(), 480.dp());
        let mut core = AppCore::new(platform, app.config().clone(), Rc::new(Counter));
        assert!(core.start(), "the Rust Counter starts");
        let queue = core.platform().queue();
        run_core(&mut core, &queue);
    }

    fn run_kotlin_counter(dir: &Path) -> String {
        let mut child = Command::new(env!("CARGO_BIN_EXE_counter-kotlin"))
            .current_dir(dir)
            .env("KOMPOZ_SCALE", "1")
            .env("KOMPOZ_SCRIPT", script(dir))
            .env_remove("KOMPOZ_HEADLESS")
            .env_remove("KOMPOZ_PLATFORM")
            .env_remove("KOMPOZ_STATE_FILE")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the Kotlin Counter starts");
        let deadline = Instant::now() + Duration::from_secs(120);
        let status = loop {
            if let Some(status) = child.try_wait().expect("the Kotlin Counter's status") {
                break status;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the Kotlin Counter didn't quit within two minutes");
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let mut stderr = String::new();
        if let Some(mut e) = child.stderr.take() {
            use std::io::Read;
            let _ = e.read_to_string(&mut stderr);
        }
        assert!(status.success(), "the Kotlin Counter failed ({status}):\n{stderr}");
        stderr
    }

    pub fn it_draws_as_the_rust_counter_in_a_window() {
        let (rust, kotlin) = (shots_dir("rust"), shots_dir("kotlin"));
        run_rust_counter(&rust);
        let log = run_kotlin_counter(&kotlin);
        println!("{log}");
        assert!(log.contains("kompoz: renderer: KompozR on vulkan"), "no renderer:\n{log}");
        assert!(!log.contains("GPU validation"), "validation errors:\n{log}");
        for name in ["before.png", "after.png"] {
            let (r, k) = (shot(&rust, name), shot(&kotlin, name));
            assert_eq!((k.width, k.height), (360, 480), "{name}: the window's size");
            let differ = r.rgba.chunks(4).zip(k.rgba.chunks(4)).filter(|(a, b)| a != b).count();
            assert_eq!(differ, 0, "{name}: {differ} pixels differ from the Rust Counter's ({})", kotlin.display());
        }
        assert_ne!(shot(&kotlin, "before.png").rgba, shot(&kotlin, "after.png").rgba, "the click changed nothing");
    }
}

#[cfg(windows)]
fn main() {
    use kompoz::gpu::device::{self, Backend, Surface};
    kompoz::runtime::set_ui_thread();
    if std::env::var_os("KOMPOZ_DISPLAY_TESTS").is_none_or(|v| v != "1") {
        println!("skipped: KOMPOZ_DISPLAY_TESTS=1 runs it");
        return;
    }
    let required = std::env::var_os("KOMPOZ_GPU_REQUIRED").is_some_and(|v| v == "1");
    match device::create(Backend::Vulkan, Surface::default()) {
        Ok(d) => println!("device: {}", d.device_name()),
        Err(why) if required => panic!("no GPU device (KOMPOZ_GPU_REQUIRED=1): {why}"),
        Err(why) => {
            println!("skipped it_draws_as_the_rust_counter_in_a_window: {why}");
            return;
        }
    }
    let ok = std::panic::catch_unwind(on_windows::it_draws_as_the_rust_counter_in_a_window).is_ok();
    println!("{} it_draws_as_the_rust_counter_in_a_window", if ok { "ok    " } else { "FAILED" });
    if !ok {
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    println!("skipped: the Kotlin Counter's Windows test runs on Windows");
}
