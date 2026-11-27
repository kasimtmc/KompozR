//! The Kotlin Counter: it counts (its `.kt` composed by the test rule, a click on its button), and run headless
//! (`KOMPOZ_HEADLESS=1`) with the script CI gives the Rust Counter it draws the Rust Counter's pixels — both shots,
//! before the click and after, the same as the Rust Counter's (examples/counter's, drawn here on the same headless
//! platform), the click changing them. The drawing needs a GPU device (lavapipe in CI): without one it is skipped,
//! unless `KOMPOZ_GPU_REQUIRED=1`.

#![allow(non_snake_case)]

use kompoz::gpu::device::{self, NATIVE_BACKEND, Surface};
use kompoz::platform::AppCore;
use kompoz::platform::headless::{HeadlessPlatform, run_core};
use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

// The Kotlin Counter, as the example's binary has it.
kompoz::kompoz_file!("src/counter.kt");

/// The Rust Counter (examples/counter's), drawn beside it.
#[path = "../../counter/src/counter.rs"]
mod rust_counter;

/// The script CI runs the Rust Counter with: a shot, a click on the button, another shot (into `dir`).
fn script(dir: &Path) -> String {
    let (before, after) = (dir.join("before.png"), dir.join("after.png"));
    format!("wait 30;shot {};click 70 70;wait 30;shot {};quit", before.display(), after.display())
}

/// A directory of the test's own.
fn shots_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("counter-kotlin-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a directory for the shots");
    dir
}

fn shot(dir: &Path, name: &str) -> RgbaImage {
    let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    decode_png(&bytes).unwrap_or_else(|| panic!("{name} decodes"))
}

/// The Kotlin Counter shows its count; a click on its button adds one.
fn the_kotlin_counter_counts() {
    let rule = KompozTestRule::new(Counter);
    rule.on_node_with_text("Sayaç: 0").assert_exists();
    rule.on_node_with_text("+1").perform_click();
    rule.on_node_with_text("Sayaç: 1").assert_exists();
    rule.on_node_with_text("+1").perform_click();
    rule.on_node_with_text("Sayaç: 2").assert_exists();
}

/// The Rust Counter run by `script` on the headless platform, here, as the Rust example's binary runs it.
fn run_rust_counter(dir: &Path) {
    let mut platform = HeadlessPlatform::new();
    platform.set_option("KOMPOZ_SCRIPT", &script(dir));
    let app = App::new("Sayaç").size(360.dp(), 480.dp());
    let mut core = AppCore::new(platform, app.config().clone(), Rc::new(rust_counter::Counter));
    assert!(core.start(), "the Rust Counter starts");
    let queue = core.platform().queue();
    run_core(&mut core, &queue);
}

/// The example's binary run headless by `script`; its stderr. Fails if it doesn't end well within a minute.
fn run_kotlin_counter(dir: &Path) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_counter-kotlin"))
        .current_dir(dir)
        .env("KOMPOZ_HEADLESS", "1")
        .env("KOMPOZ_SCRIPT", script(dir))
        .env_remove("KOMPOZ_STATE_FILE")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the Kotlin Counter starts");
    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child.try_wait().expect("the Kotlin Counter's status") {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the Kotlin Counter didn't quit within a minute");
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

/// The Kotlin Counter's shots are the Rust Counter's, pixel for pixel; the click changed them.
fn it_draws_as_the_rust_counter() {
    let (rust, kotlin) = (shots_dir("rust"), shots_dir("kotlin"));
    run_rust_counter(&rust);
    let log = run_kotlin_counter(&kotlin);
    let renderer = format!("kompoz: renderer: KompozR on {}", NATIVE_BACKEND.name());
    assert!(log.contains(&renderer), "no renderer:\n{log}");
    assert!(!log.contains("GPU validation"), "validation errors:\n{log}");
    for name in ["before.png", "after.png"] {
        let (r, k) = (shot(&rust, name), shot(&kotlin, name));
        assert_eq!((k.width, k.height), (360, 480), "{name}: the window's size");
        assert_eq!((k.width, k.height), (r.width, r.height), "{name}");
        let differ = r.rgba.chunks(4).zip(k.rgba.chunks(4)).filter(|(a, b)| a != b).count();
        assert_eq!(differ, 0, "{name}: {differ} pixels differ from the Rust Counter's (shots in {})", kotlin.display());
    }
    let (before, after) = (shot(&kotlin, "before.png"), shot(&kotlin, "after.png"));
    assert_ne!(before.rgba, after.rgba, "the click changed nothing");
}

fn main() {
    kompoz::runtime::set_ui_thread();
    let mut tests: Vec<(&str, fn())> = vec![("the_kotlin_counter_counts", the_kotlin_counter_counts)];
    let required = std::env::var_os("KOMPOZ_GPU_REQUIRED").is_some_and(|v| v == "1");
    match device::create(NATIVE_BACKEND, Surface::default()) {
        Ok(d) => {
            println!("device: {}", d.device_name());
            tests.push(("it_draws_as_the_rust_counter", it_draws_as_the_rust_counter));
        }
        Err(why) if required => panic!("no GPU device (KOMPOZ_GPU_REQUIRED=1): {why}"),
        Err(why) => println!("skipped it_draws_as_the_rust_counter: {why}"),
    }
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let mut failed = 0;
    for (name, test) in tests {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        let ok = std::panic::catch_unwind(test).is_ok();
        failed += usize::from(!ok);
        println!("{} {name}", if ok { "ok    " } else { "FAILED" });
    }
    if failed > 0 {
        std::process::exit(1);
    }
}
