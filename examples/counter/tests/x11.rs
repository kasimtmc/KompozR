//! The Counter on X11 on a real display (`KOMPOZ_DISPLAY_TESTS=1`, with `DISPLAY` naming an X server: Xwayland on a
//! Wayland desktop, Xvfb in CI): its window made and drawn into with Vulkan through `VK_KHR_xlib_surface`, a script
//! clicking its button (the shot after shows the count changed), resizing its window (the shot after is the new
//! size), quitting; no validation errors. On lavapipe, and on the GPU `KOMPOZ_DISPLAY_GPU` names (a part of its
//! name, as `KOMPOZ_GPU_DEVICE` takes it: "Intel").

use kompoz_graphics::{RgbaImage, decode_png};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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

/// The counter run with `script` on X11 (`env` added), in `dir`; its stderr. Fails if it doesn't end well within a
/// minute.
fn run_counter(dir: &Path, script: &str, env: &[(&str, &str)]) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_counter"))
        .current_dir(dir)
        .env("KOMPOZ_PLATFORM", "x11")
        .env("KOMPOZ_GPU_VALIDATION", "1")
        .env("KOMPOZ_SCALE", "1")
        .env("KOMPOZ_SCRIPT", script)
        .env_remove("KOMPOZ_HEADLESS")
        .envs(env.iter().copied())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the counter starts");
    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child.try_wait().expect("the counter's status") {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the counter didn't quit within a minute");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut stderr = String::new();
    if let Some(mut e) = child.stderr.take() {
        use std::io::Read;
        let _ = e.read_to_string(&mut stderr);
    }
    assert!(status.success(), "the counter failed ({status}):\n{stderr}");
    stderr
}

fn shot(dir: &Path, name: &str) -> RgbaImage {
    let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    decode_png(&bytes).unwrap_or_else(|| panic!("{name} decodes"))
}

/// Clicked, resized, quit, on the GPU `env` picks.
fn counter_on(device: &str, env: &[(&str, &str)]) {
    let dir: PathBuf = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x11-{device}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a directory for the shots");
    let script = "wait 30;shot a.png;click 70 70;wait 30;shot b.png;resize 500 400;sleep 1500;wait 10;shot c.png;quit";
    let log = run_counter(&dir, script, env);
    println!("  {device}:\n{log}");
    let validation: Vec<&str> = log.lines().filter(|l| l.starts_with("kompoz: vulkan:")).collect();
    assert!(validation.is_empty(), "{device}: validation errors:\n{}", validation.join("\n"));
    assert!(log.contains("kompoz: renderer: KompozR on vulkan"), "{device}: no renderer:\n{log}");
    let (a, b, c) = (shot(&dir, "a.png"), shot(&dir, "b.png"), shot(&dir, "c.png"));
    assert_eq!((a.width, a.height), (360, 480), "{device}: the window's first size");
    assert_eq!((b.width, b.height), (360, 480));
    assert_ne!(a.rgba, b.rgba, "{device}: the click changed nothing");
    assert_eq!((c.width, c.height), (500, 400), "{device}: the window wasn't resized");
}

#[test]
fn the_counter_runs_on_x11() {
    if std::env::var_os("KOMPOZ_DISPLAY_TESTS").is_none_or(|v| v != "1") {
        println!("skipped: KOMPOZ_DISPLAY_TESTS=1 (and an X display) runs it");
        return;
    }
    assert!(std::env::var_os("DISPLAY").is_some(), "KOMPOZ_DISPLAY_TESTS=1 but DISPLAY isn't set");
    counter_on("lavapipe", &[("VK_DRIVER_FILES", &lavapipe_icd())]);
    if let Ok(gpu) = std::env::var("KOMPOZ_DISPLAY_GPU")
        && !gpu.is_empty()
    {
        counter_on(&gpu, &[("KOMPOZ_GPU_DEVICE", &gpu)]);
    }
}
