//! Fingers and a pen in the app's real window (`KOMPOZ_DISPLAY_TESTS=1`; on Windows; skipped otherwise), injected
//! from a thread of the app's process as a synthetic digitizer's (`InjectSyntheticPointerInput`: CI's windows runner
//! has no touch screen and no pen, and Windows makes the same WM_POINTER messages of injected contacts as of a real
//! digitizer's): a finger's tap on the Counter's button clicks it once (the mouse messages Windows makes of the tap
//! are left out: no second click); a pen hovering over an area, pressing at half its pressure with its barrel
//! button, moving and lifting reaches the area's pointer handlers as a stylus's. Run in a process of its own (this
//! test run again): its client thread injects, the app's loop serves.
//!
//! Wine's user32 has no `InjectSyntheticPointerInput` (nor `InjectTouchInput`): skipped there — the window's own
//! tests send it WM_POINTER messages with reports of theirs instead (`windows::window`'s, through `pointer::Source`).

#![allow(non_snake_case)]

#[cfg(windows)]
mod on_windows {
    use kompoz_foundation::prelude::*;
    use kompoz_graphics::*;
    use kompoz_runtime::*;
    use kompoz_ui::Modifier;
    use kompoz_ui::input::{AwaitPointerEventScope, PointerEvent, PointerEventType};
    use std::process::{Command, Stdio};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::Controls::{
        CreateSyntheticPointerDevice, DestroySyntheticPointerDevice, HSYNTHETICPOINTERDEVICE, POINTER_FEEDBACK_NONE,
        POINTER_TYPE_INFO, POINTER_TYPE_INFO_0,
    };
    use windows::Win32::UI::Input::Pointer::{
        InjectSyntheticPointerInput, POINTER_FLAG_DOWN, POINTER_FLAG_INCONTACT, POINTER_FLAG_INRANGE,
        POINTER_FLAG_PRIMARY, POINTER_FLAG_UP, POINTER_FLAG_UPDATE, POINTER_FLAGS, POINTER_INFO, POINTER_PEN_INFO,
        POINTER_TOUCH_INFO,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, PEN_FLAG_BARREL, PEN_MASK_PRESSURE, POINTER_INPUT_TYPE, PT_PEN, PT_TOUCH, PostMessageW, WM_CLOSE,
    };
    use windows_core::w;

    pub const CHILD: &str = "KOMPOZ_POINTER_WINDOWS_CHILD";
    const TITLE: &str = "Kalem ve parmak";

    /// What the app saw, in order (its handlers' lines: the client thread reads them).
    static SEEN: Mutex<Vec<String>> = Mutex::new(Vec::new());

    fn seen(line: String) {
        SEEN.lock().unwrap().push(line);
    }

    /// The Counter's button (a click a line), an area whose presses, moves and releases are lines with their
    /// pointer's type, pressure and buttons. At scale 1: the button at (16, 16), 120 × 48; the area at (16, 80),
    /// 240 × 120.
    #[kompozable]
    pub fn Screen() {
        let count = remember(|| mutable_state_of(0));
        // (Composed: the client may inject.)
        SideEffect(|| {
            if !has("ready") {
                seen("ready".into());
            }
        });
        let record = |label: &'static str| {
            move |_: &AwaitPointerEventScope, e: &PointerEvent| {
                let c = &e.changes[0];
                seen(format!("{label} {:?} p{:.2} b{}", c.r#type, c.pressure, e.buttons.0));
            }
        };
        Column(
            modifier = Modifier.fill_max_size().background(Color::White).padding(16.dp()),
            vertical_arrangement = Arrangement::spaced_by(16.dp()),
            content = move || {
                let add = count.clone();
                Box(
                    modifier = Modifier.size_wh(120.dp(), 48.dp()).background(Color::from_argb(0xff3f51b5)).clickable(
                        on_click = move || {
                            add.set(add.get() + 1);
                            seen(format!("click {}", add.get()));
                        },
                    ),
                    content = || BasicText("Ekle"),
                );
                Box(modifier = Modifier
                    .size_wh(240.dp(), 120.dp())
                    .background(Color::from_argb(0xffe0e0e0))
                    .on_pointer_event(PointerEventType::Press, on_event = record("press"))
                    .on_pointer_event(PointerEventType::Move, on_event = record("move"))
                    .on_pointer_event(PointerEventType::Release, on_event = record("release")));
            },
        );
    }

    fn wait_for<T>(seconds: u64, mut f: impl FnMut() -> Option<T>) -> Option<T> {
        let until = Instant::now() + Duration::from_secs(seconds);
        loop {
            if let Some(v) = f() {
                return Some(v);
            }
            if Instant::now() > until {
                return None;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn has(line: &str) -> bool {
        SEEN.lock().unwrap().iter().any(|l| l == line)
    }

    /// The app's window and where its client area is on the screen.
    fn window() -> Option<(HWND, (i32, i32))> {
        let title: Vec<u16> = TITLE.encode_utf16().chain([0]).collect();
        // SAFETY: a lookup by class and title; a point the window's client area's origin is written into.
        unsafe {
            let hwnd = FindWindowW(w!("KompozWindow"), windows_core::PCWSTR(title.as_ptr())).ok()?;
            let mut p = POINT::default();
            ClientToScreen(hwnd, &mut p).ok().ok()?;
            Some((hwnd, (p.x, p.y)))
        }
    }

    fn info(kind: POINTER_INPUT_TYPE, flags: POINTER_FLAGS, x: i32, y: i32) -> POINTER_INFO {
        POINTER_INFO {
            pointerType: kind,
            pointerId: 0,
            pointerFlags: flags | POINTER_FLAG_PRIMARY,
            ptPixelLocation: POINT { x, y },
            ..Default::default()
        }
    }

    fn inject(device: HSYNTHETICPOINTERDEVICE, report: POINTER_TYPE_INFO) -> Result<(), String> {
        // SAFETY: a device made here, one report of its type.
        unsafe { InjectSyntheticPointerInput(device, &[report]) }.map_err(|e| format!("injected: {e}"))?;
        std::thread::sleep(Duration::from_millis(30));
        Ok(())
    }

    fn touch(flags: POINTER_FLAGS, x: i32, y: i32) -> POINTER_TYPE_INFO {
        POINTER_TYPE_INFO {
            r#type: PT_TOUCH,
            Anonymous: POINTER_TYPE_INFO_0 {
                touchInfo: POINTER_TOUCH_INFO { pointerInfo: info(PT_TOUCH, flags, x, y), ..Default::default() },
            },
        }
    }

    fn pen(flags: POINTER_FLAGS, x: i32, y: i32, pressure: u32) -> POINTER_TYPE_INFO {
        POINTER_TYPE_INFO {
            r#type: PT_PEN,
            Anonymous: POINTER_TYPE_INFO_0 {
                penInfo: POINTER_PEN_INFO {
                    pointerInfo: info(PT_PEN, flags, x, y),
                    penFlags: PEN_FLAG_BARREL,
                    penMask: PEN_MASK_PRESSURE,
                    pressure,
                    ..Default::default()
                },
            },
        }
    }

    /// The injecting client: a tap on the button, then the pen over the area; what the app saw.
    fn client() -> Result<Vec<String>, String> {
        let (_, (ox, oy)) = wait_for(60, window).ok_or("the app's window didn't come")?;
        // (Its first frames drawn, its layout there.)
        wait_for(30, || SEEN.lock().unwrap().iter().any(|l| l == "ready").then_some(()))
            .ok_or("the app didn't compose")?;
        let up = POINTER_FLAG_UP | POINTER_FLAG_INRANGE;
        let touching = POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT;
        // A finger's tap on the button.
        // SAFETY: a synthetic digitizer of one contact, destroyed below.
        let fingers = unsafe { CreateSyntheticPointerDevice(PT_TOUCH, 1, POINTER_FEEDBACK_NONE) }
            .map_err(|e| format!("no synthetic touch screen: {e}"))?;
        let (bx, by) = (ox + 76, oy + 40);
        inject(fingers, touch(POINTER_FLAG_DOWN | touching, bx, by))?;
        inject(fingers, touch(POINTER_FLAG_UPDATE | touching, bx, by))?;
        inject(fingers, touch(up, bx, by))?;
        wait_for(10, || has("click 1").then_some(())).ok_or("the tap didn't click")?;
        std::thread::sleep(Duration::from_millis(500));
        if has("click 2") {
            return Err("the tap clicked twice (the mouse messages made of it weren't left out)".into());
        }
        // SAFETY: the device made above.
        unsafe { DestroySyntheticPointerDevice(fingers) };
        // A pen over the area: hovering, pressing at half its pressure with its barrel button, moving, lifting,
        // going.
        // SAFETY: a synthetic pen, destroyed below.
        let stylus = unsafe { CreateSyntheticPointerDevice(PT_PEN, 1, POINTER_FEEDBACK_NONE) }
            .map_err(|e| format!("no synthetic pen: {e}"))?;
        let (px, py) = (ox + 136, oy + 140);
        inject(stylus, pen(POINTER_FLAG_UPDATE | POINTER_FLAG_INRANGE, px, py, 0))?;
        inject(stylus, pen(POINTER_FLAG_DOWN | touching, px, py, 512))?;
        inject(stylus, pen(POINTER_FLAG_UPDATE | touching, px + 10, py, 512))?;
        inject(stylus, pen(up, px + 10, py, 0))?;
        inject(stylus, pen(POINTER_FLAG_UPDATE, px + 10, py, 0))?;
        let pressed = wait_for(10, || has("press Stylus p0.50 b1").then_some(()));
        let released =
            wait_for(5, || SEEN.lock().unwrap().iter().any(|l| l.starts_with("release Stylus")).then_some(()));
        // SAFETY: the device made above.
        unsafe { DestroySyntheticPointerDevice(stylus) };
        let lines = SEEN.lock().unwrap().clone();
        if pressed.is_none() || released.is_none() {
            return Err(format!("the pen didn't press and release as a stylus: {lines:?}"));
        }
        Ok(lines)
    }

    pub fn run_child() -> std::process::ExitCode {
        let report = Arc::new(Mutex::new(None));
        let r = report.clone();
        let client = std::thread::spawn(move || {
            let outcome = client();
            // (The app closed whatever happened.)
            if let Some((hwnd, _)) = window() {
                // SAFETY: a posted message to the app's window.
                let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
            }
            *r.lock().unwrap() = Some(outcome);
        });
        let code = kompoz_platform::App::new(TITLE).size(320.dp(), 260.dp()).run(Screen);
        let _ = client.join();
        match report.lock().unwrap().take() {
            Some(Ok(lines)) => {
                for l in lines {
                    eprintln!("pointer: {l}");
                }
                eprintln!("pointer: done");
                code
            }
            Some(Err(why)) => {
                eprintln!("pointer: FAILED {why}");
                std::process::ExitCode::FAILURE
            }
            None => {
                eprintln!("pointer: FAILED the client didn't report");
                std::process::ExitCode::FAILURE
            }
        }
    }

    pub fn fingers_and_a_pen_reach_the_app() {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .env(CHILD, "1")
            .env("KOMPOZ_SCALE", "1")
            .env("KOMPOZ_NO_WALLPAPER_COLORS", "1")
            .env_remove("KOMPOZ_HEADLESS")
            .env_remove("KOMPOZ_PLATFORM")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the app starts");
        let deadline = Instant::now() + Duration::from_secs(180);
        let status = loop {
            if let Some(status) = child.try_wait().expect("the app's status") {
                break status;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the app didn't quit within three minutes");
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let mut log = String::new();
        if let Some(mut e) = child.stderr.take() {
            use std::io::Read;
            let _ = e.read_to_string(&mut log);
        }
        println!("{log}");
        assert!(status.success() && log.contains("pointer: done"), "the client failed ({status}):\n{log}");
    }

    /// Whether this system's user32 injects pointers (wine's hasn't the function: its import would end the
    /// process).
    pub fn injects() -> bool {
        kompoz_platform::windows::sys::user32_function(windows_core::s!("InjectSyntheticPointerInput")).is_some()
    }
}

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    if std::env::var_os(on_windows::CHILD).is_some() {
        return on_windows::run_child();
    }
    if std::env::var_os("KOMPOZ_DISPLAY_TESTS").is_none_or(|v| v != "1") {
        println!("skipped: KOMPOZ_DISPLAY_TESTS=1 runs it");
        return std::process::ExitCode::SUCCESS;
    }
    if !on_windows::injects() {
        println!("skipped: this system's user32 injects no pointers (wine's)");
        return std::process::ExitCode::SUCCESS;
    }
    let ok = std::panic::catch_unwind(on_windows::fingers_and_a_pen_reach_the_app).is_ok();
    println!("{} fingers_and_a_pen_reach_the_app", if ok { "ok    " } else { "FAILED" });
    if ok { std::process::ExitCode::SUCCESS } else { std::process::ExitCode::FAILURE }
}

#[cfg(not(windows))]
fn main() {
    println!("skipped: pens and fingers are injected on Windows");
}
