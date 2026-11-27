//! The app's windows on the headless platform, with a GPU device each (lavapipe in CI): a `Window` opens on a
//! window of its own as it is composed and closes as it leaves, its content drawn with the caller's locals; scripts
//! reach it by its title ("@Title: command"); the user closing it is its `on_close_request` (the main window's ends
//! the app); its state is followed both ways (size, minimized, placement, position, to the front); hidden, its
//! composition and state stay; `application { }` ends with `exit_application` or with its last window; the tray
//! icon's menu and notifications reach the platform and its clicks come back. One device-holding test after another
//! on this thread; without a device they are skipped, unless `KOMPOZ_GPU_REQUIRED=1`.

#![allow(non_snake_case, non_upper_case_globals)]

use kompoz_foundation::prelude::*;
use kompoz_gpu::device::{self, Backend, Surface};
use kompoz_graphics::*;
use kompoz_platform::headless::{HeadlessPlatform, HeadlessShared, headless_windows, run_windows};
use kompoz_platform::{AppConfig, AppContent, Event, GpuPriority, TrayMenuItem};
use kompoz_runtime::*;
use kompoz_ui::draw::{ColorPainter, Painter};
use kompoz_ui::window::*;
use kompoz_ui::{Alignment, Modifier};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

fn config() -> AppConfig {
    AppConfig {
        title: "KompozR windows test".into(),
        width: Dp(300.0),
        height: Dp(200.0),
        image_disk_cache_bytes: 0,
        args: vec!["test".into()],
        gpu_priority: GpuPriority::Auto,
    }
}

/// A directory of the test's own.
fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kompoz-windows-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A PNG's size and pixels.
fn picture(path: &Path) -> ImageBitmap {
    let bytes = std::fs::read(path).unwrap_or_else(|_| panic!("{} was written", path.display()));
    decode_image(&bytes, ImageRequestSize::default()).expect("a PNG")
}

/// The pixel at `x`, `y` of a picture, RGBA.
fn pixel(image: &ImageBitmap, x: i32, y: i32) -> [u8; 4] {
    let p = image.pixels();
    let i = (y * image.width() + x) as usize * 4;
    [p[i], p[i + 1], p[i + 2], p[i + 3]]
}

/// Runs `content` (the main window's, or the app's composition's) on the headless platform with `script` until it
/// quits; what its windows shared, kept.
fn run(content: AppContent, script: &str, options: &[(&str, &str)]) -> Rc<HeadlessShared> {
    let mut platform = HeadlessPlatform::new();
    platform.set_option("KOMPOZ_SCRIPT", script);
    for (name, value) in options {
        platform.set_option(name, value);
    }
    let shared = platform.shared();
    let queue = platform.queue();
    let mut windows = headless_windows(platform, &config(), &content);
    assert!(windows.start(), "the app starts");
    run_windows(&mut windows, &queue);
    shared
}

/// What a test saw, as the frames went.
type Seen = Rc<RefCell<Vec<String>>>;

static LocalTint: KompozitionLocal<Color> = KompozitionLocal::new_static(|| Color::Black);

#[kompozable]
fn Second(tint: Color) {
    Box(modifier = Modifier.fill_max_size().background(tint));
}

/// The main window: white, a click anywhere showing or hiding the window "Second" (its state the test's), the
/// locals it is composed under the test's blue tint.
#[kompozable]
fn TwoWindows(shown: MutableState<bool>, state: WindowState, closed: Rc<Cell<u32>>) {
    let toggle = shown.clone();
    Box(modifier =
        Modifier.fill_max_size().background(Color::White).clickable(on_click = move || toggle.set(!toggle.get())));
    if shown.get() {
        KompozitionLocalProvider(&[LocalTint.provides(Color::Blue)], move || {
            let (closed, shown) = (closed.clone(), shown.clone());
            Window(
                on_close_request = move || {
                    closed.set(closed.get() + 1);
                    shown.set(false);
                },
                state = state.clone(),
                title = "Second".to_string(),
                content = |_: &FrameWindowScope| Second(LocalTint.current()),
            );
        });
    }
}

fn two_windows(shown: MutableState<bool>, state: WindowState, closed: Rc<Cell<u32>>) -> AppContent {
    AppContent::Window(Rc::new(kompozable_lambda!(move || TwoWindows(shown.clone(), state.clone(), closed.clone()))))
}

/// A click opens the window of its own, its content drawn there (with the caller's tint) at its state's size; a
/// script's "@Second:" commands reach it; the user closing it (its `window close`) is its `on_close_request`, which
/// takes it out of the composition: it closes.
fn a_window_opens_draws_with_the_callers_locals_and_closes() {
    let dir = temp_dir("open");
    let (second, main, after) = (dir.join("second.png"), dir.join("main.png"), dir.join("after.png"));
    let shown = mutable_state_of(false);
    let state = WindowState::new(
        WindowPlacement::Floating,
        false,
        WindowPosition::PlatformDefault,
        DpSize(Dp(120.0), Dp(80.0)),
    );
    let closed = Rc::new(Cell::new(0));
    let script = format!(
        "wait 3; click 150 100; wait 5; @Second: shot {}; wait 3; shot {}; @Second: window close; wait 5; shot {}; quit",
        second.display(),
        main.display(),
        after.display()
    );
    let shared = run(two_windows(shown.clone(), state.clone(), closed.clone()), &script, &[]);
    let s = picture(&second);
    assert_eq!((s.width(), s.height()), (120, 80), "the window is its state's size");
    assert_eq!(pixel(&s, 60, 40), [0, 0, 255, 255], "drawn with the caller's tint");
    let m = picture(&main);
    assert_eq!((m.width(), m.height()), (300, 200));
    assert_eq!(pixel(&m, 10, 10), [255, 255, 255, 255], "the main window's own");
    assert_eq!(closed.get(), 1, "the user closing it is its on_close_request");
    assert!(!shown.peek(), "which took it out of the composition");
    assert!(shared.window_titled("Second").is_none(), "and it closed");
    assert!(after.exists(), "the main window goes on");
}

/// The main window's close ends the app (C++'s): the script's quit never comes.
fn closing_the_main_window_ends_the_app() {
    let dir = temp_dir("main-close");
    let never = dir.join("never.png");
    let script = format!("wait 3; window close; wait 3; shot {}; quit", never.display());
    let shown = mutable_state_of(false);
    run(two_windows(shown, WindowState::default(), Rc::default()), &script, &[]);
    assert!(!never.exists(), "the app ended at the main window's close");
}

/// The window's state is followed both ways: minimized from code, the platform's window minimized; restored by
/// the user (`window restored`), the state told; resized by the user, its size read back (dp); its size set from
/// code, the window resized; to the front; an aligned position worked out in the screen.
fn the_windows_state_is_followed_both_ways() {
    let shown = mutable_state_of(true);
    let state = WindowState::new(
        WindowPlacement::Floating,
        false,
        WindowPosition::Aligned(Alignment::Center),
        DpSize(Dp(400.0), Dp(300.0)),
    );
    let seen: Seen = Rc::default();
    let (st, sn) = (state.clone(), seen.clone());
    let content = AppContent::Window(Rc::new(kompozable_lambda!(move || {
        let (st, sn) = (st.clone(), sn.clone());
        TwoWindows(shown.clone(), st.clone(), Rc::default());
        // (What the state says each frame, as the app would read it.)
        let (placement, minimized, position, size) = (st.placement(), st.is_minimized(), st.position(), st.size());
        sn.borrow_mut().push(format!("{placement:?} {minimized} {position:?} {}x{}", size.width.0, size.height.0));
    })));
    let platform = HeadlessPlatform::new();
    let shared = platform.shared();
    let (st, sh) = (state.clone(), shared.clone());
    let checks = Rc::new(RefCell::new(Vec::<String>::new()));
    let script = "@Second: resize 250 150; wait 30; quit";
    {
        let mut platform = platform;
        platform.set_option("KOMPOZ_SCRIPT", script);
        let queue = platform.queue();
        let mut windows = headless_windows(platform, &config(), &content);
        assert!(windows.start());
        // Frames by hand: the state set from code between them, as an app's callbacks would.
        let mut step = 0;
        loop {
            for (id, e) in queue.take() {
                if e != Event::Wake {
                    windows.handle(id, &e);
                }
            }
            windows.sync();
            for core in windows.cores_mut() {
                core.frame(None);
            }
            windows.sync();
            step += 1;
            let w = sh.window_titled("Second");
            match step {
                2 => {
                    let w = w.expect("open");
                    checks.borrow_mut().push(format!("aligned {:?}", w.position));
                    st.set_is_minimized(true);
                }
                4 => {
                    checks.borrow_mut().push(format!("minimized {}", w.expect("open").minimized));
                    st.to_front();
                }
                6 => {
                    let w = w.expect("open");
                    checks.borrow_mut().push(format!("fronted {} {}", w.fronted, w.minimized));
                    st.set_size(DpSize(Dp(320.0), Dp(240.0)));
                }
                8 => {
                    let w = w.expect("open");
                    checks.borrow_mut().push(format!("sized {:?}", w.size));
                    st.set_placement(WindowPlacement::Maximized);
                }
                10 => {
                    let w = w.expect("open");
                    checks.borrow_mut().push(format!("placed {:?}", w.placement));
                }
                _ => {}
            }
            if windows.quitting() || step > 40 {
                break;
            }
        }
        windows.quit();
    }
    assert_eq!(
        *checks.borrow(),
        ["aligned Some((760, 390))", "minimized true", "fronted 1 false", "sized (320, 240)", "placed Maximized"],
        "the window followed its state"
    );
    let seen = seen.borrow();
    assert!(seen.iter().any(|s| s.contains("250x150")), "the user's resize read back into the state: {seen:?}");
    assert!(seen.iter().any(|s| s.starts_with("Floating true")), "minimized: {seen:?}");
    assert_eq!(state.size(), DpSize(Dp(320.0), Dp(240.0)), "the app's last");
    assert_eq!(state.placement(), WindowPlacement::Maximized);
}

/// Hidden (`visible = false`), the window goes but its composition and state stay: shown again, its counter is
/// where it was.
fn a_hidden_window_keeps_its_composition() {
    let dir = temp_dir("hidden");
    let shot = dir.join("again.png");
    let visible = mutable_state_of(true);
    let count = mutable_state_of(0);
    let made = Rc::new(Cell::new(0));
    let (m, c) = (made.clone(), count.clone());
    let content = AppContent::Window(Rc::new(kompozable_lambda!(move || {
        let (m, c, toggle) = (m.clone(), c.clone(), visible.clone());
        Box(modifier =
            Modifier.fill_max_size().background(Color::White).clickable(on_click = move || toggle.set(!toggle.get())));
        Window(
            on_close_request = || {},
            visible = visible.get(),
            title = "Kept".to_string(),
            state = remember_window_state(size = DpSize(Dp(100.0), Dp(100.0))),
            content = move |_: &FrameWindowScope| {
                let kept = remember(|| {
                    m.set(m.get() + 1);
                    mutable_state_of(0)
                });
                c.set(kept.get());
                let k = kept.clone();
                Box(modifier = Modifier
                    .fill_max_size()
                    .background(if kept.get() > 0 { Color::Green } else { Color::Red })
                    .clickable(on_click = move || k.set(k.get() + 1)));
            },
        );
    })));
    let script = format!(
        "wait 3; @Kept: click 50 50; wait 3; click 10 10; wait 3; click 10 10; wait 5; @Kept: shot {}; wait 3; quit",
        shot.display()
    );
    let shared = run(content, &script, &[]);
    assert_eq!(made.get(), 1, "its composition stayed while it was hidden");
    assert_eq!(count.peek(), 1);
    assert_eq!(pixel(&picture(&shot), 50, 50), [0, 255, 0, 255], "shown again as it was");
    let _ = shared;
}

/// A window while `open` says, its close request taking it out.
#[kompozable]
fn LastWindow(open: MutableState<bool>) {
    if open.get() {
        Window(
            on_close_request = move || open.set(false),
            title = "Last".to_string(),
            content = |_: &FrameWindowScope| {},
        );
    }
}

/// `application { }`: windows of its own, no window of the app's; `exit_application` ends it. Another: its last
/// window closed, it ends.
fn an_application_ends_with_exit_or_its_last_window() {
    let dir = temp_dir("application");
    let (first, never) = (dir.join("first.png"), dir.join("never.png"));
    let exited = Rc::new(Cell::new(false));
    let e = exited.clone();
    let content = AppContent::Application(Rc::new(move |app: &ApplicationScope| {
        let (app, e) = (app.clone(), e.clone());
        kompozable_block!({
            Window(
                on_close_request = move || {
                    e.set(true);
                    app.exit_application();
                },
                title = "Only".to_string(),
                state = remember_window_state(size = DpSize(Dp(90.0), Dp(60.0))),
                content = |_: &FrameWindowScope| Second(Color::Green),
            );
        });
    }));
    let script = format!(
        "wait 3; @Only: shot {}; wait 2; @Only: window close; wait 5; @Only: shot {}; quit",
        first.display(),
        never.display()
    );
    run(content, &script, &[]);
    assert!(exited.get(), "its close request ran");
    assert_eq!(pixel(&picture(&first), 45, 30), [0, 255, 0, 255]);
    assert!(!never.exists(), "exit_application ended it");

    // Its last window gone (its own close request takes it out): it ends with nothing left in it.
    let open = mutable_state_of(true);
    let o = open.clone();
    let last = kompozable_lambda!(move || LastWindow(o.clone()));
    let content = AppContent::Application(Rc::new(move |_: &ApplicationScope| last()));
    let after = dir.join("after.png");
    run(content, &format!("wait 3; @Last: window close; wait 10; shot {}; quit", after.display()), &[]);
    assert!(!open.peek());
}

/// A tray icon: its tooltip and menu reach the platform (its icon drawn by the app's renderer); a click on a menu
/// item comes back to it; a checkbox toggles; a notification sent through its state reaches the platform once.
fn the_tray_icon_its_menu_and_notifications_reach_the_platform() {
    let clicks = Rc::new(Cell::new(0));
    let checked = mutable_state_of(false);
    let (c, k) = (clicks.clone(), checked.clone());
    let state = TrayState::new();
    let tray = state.clone();
    let content = AppContent::Application(Rc::new(move |app: &ApplicationScope| {
        let (c, tray, app, k) = (c.clone(), tray.clone(), app.clone(), k.clone());
        kompozable_block!({
            let icon: Rc<dyn Painter> = Rc::new(ColorPainter(Color::Red));
            Tray(
                icon = icon,
                state = tray.clone(),
                tooltip = Some("KompozR".to_string()),
                on_action = {
                    let tray = tray.clone();
                    move || tray.send_notification(Notification::new("Hi", "there", NotificationType::Info))
                },
                menu = move |menu: &MenuScope| {
                    let (c, app, k, set) = (c.clone(), app.clone(), k.clone(), k.clone());
                    menu.Item(text = "Count".to_string(), mnemonic = Some('C'), on_click = move || c.set(c.get() + 1));
                    menu.CheckboxItem(
                        text = "Check".to_string(),
                        checked = k.get(),
                        on_checked_change = move |v| set.set(v),
                    );
                    menu.Separator();
                    menu.Menu(
                        text = "More".to_string(),
                        content = move |sub: &MenuScope| {
                            let app = app.clone();
                            sub.Item(text = "Quit".to_string(), on_click = move || app.exit_application());
                        },
                    );
                },
            );
        });
    }));
    let platform = HeadlessPlatform::new();
    let shared = platform.shared();
    let queue = platform.queue();
    let mut windows = headless_windows(platform, &config(), &content);
    assert!(windows.start());
    let mut rounds = 0;
    let mut posted = false;
    loop {
        for (id, e) in queue.take() {
            if e != Event::Wake {
                windows.handle(id, &e);
            }
        }
        windows.sync();
        for core in windows.cores_mut() {
            core.frame(None);
        }
        windows.sync();
        rounds += 1;
        if !posted && !shared.trays().is_empty() {
            posted = true;
            let id = shared.trays()[0].id;
            let app = WindowId::APPLICATION;
            queue.post_to(app, Event::TrayMenuItem { tray: id, path: vec![0] });
            queue.post_to(app, Event::TrayMenuItem { tray: id, path: vec![1] });
            queue.post_to(app, Event::TrayAction { tray: id });
            queue.post_to(app, Event::TrayMenuItem { tray: id, path: vec![3, 0] });
        }
        if windows.quitting() || rounds > 60 {
            break;
        }
    }
    let trays = shared.trays();
    windows.quit();
    assert!(rounds <= 60, "the submenu's Quit ended the app");
    assert_eq!(clicks.get(), 1);
    assert!(checked.peek(), "the checkbox toggled");
    assert_eq!(trays.len(), 1);
    let t = &trays[0];
    assert_eq!(t.tooltip, "KompozR");
    let texts: Vec<&str> = t.menu.iter().map(|i: &TrayMenuItem| i.text.as_str()).collect();
    assert_eq!(texts, ["Count", "Check", "", "More"]);
    assert_eq!(t.menu[0].mnemonic, Some('C'));
    assert_eq!(t.menu[1].kind, MenuItemKind::Checkbox { checked: true }, "shown again as it changed");
    assert_eq!(t.menu[3].items[0].text, "Quit");
    let image = t.images.iter().find(|i| i.width == 22).expect("drawn at 22 px");
    assert_eq!(&image.rgba[..4], &[255, 0, 0, 255], "by the app's renderer");
    assert_eq!(
        shared.notifications(),
        [(t.id, Notification::new("Hi", "there", NotificationType::Info))],
        "the notification reached the platform once"
    );
    assert!(shared.trays().is_empty(), "the tray icon went with the app");
}

/// A `Window` composed in a window opens too; the window's own locals are its own (its registry), the caller's
/// theme carried.
fn a_window_in_a_window_opens_too() {
    let dir = temp_dir("nested");
    let shot = dir.join("inner.png");
    let content = AppContent::Window(Rc::new(kompozable_lambda!(move || {
        KompozitionLocalProvider(&[LocalTint.provides(Color::Green)], || {
            Window(
                on_close_request = || {},
                title = "Outer".to_string(),
                state = remember_window_state(size = DpSize(Dp(80.0), Dp(80.0))),
                content = |_: &FrameWindowScope| {
                    Window(
                        on_close_request = || {},
                        title = "Inner".to_string(),
                        state = remember_window_state(size = DpSize(Dp(40.0), Dp(40.0))),
                        content = |_: &FrameWindowScope| Second(LocalTint.current()),
                    );
                },
            );
        });
    })));
    let shared = run(content, &format!("wait 5; @Inner: shot {}; wait 3; quit", shot.display()), &[]);
    assert_eq!(pixel(&picture(&shot), 20, 20), [0, 255, 0, 255], "the theme carried through both");
    let _ = shared;
}

fn main() {
    kompoz_runtime::set_ui_thread();
    let required = std::env::var_os("KOMPOZ_GPU_REQUIRED").is_some_and(|v| v == "1");
    match device::create(Backend::Vulkan, Surface::default()) {
        Ok(d) => println!("device: {}", d.device_name()),
        Err(why) => {
            println!("skipped: {why}");
            std::process::exit(if required { 1 } else { 0 });
        }
    }
    let tests: &[(&str, fn())] = &[
        (
            "a_window_opens_draws_with_the_callers_locals_and_closes",
            a_window_opens_draws_with_the_callers_locals_and_closes,
        ),
        ("closing_the_main_window_ends_the_app", closing_the_main_window_ends_the_app),
        ("the_windows_state_is_followed_both_ways", the_windows_state_is_followed_both_ways),
        ("a_hidden_window_keeps_its_composition", a_hidden_window_keeps_its_composition),
        ("an_application_ends_with_exit_or_its_last_window", an_application_ends_with_exit_or_its_last_window),
        (
            "the_tray_icon_its_menu_and_notifications_reach_the_platform",
            the_tray_icon_its_menu_and_notifications_reach_the_platform,
        ),
        ("a_window_in_a_window_opens_too", a_window_in_a_window_opens_too),
    ];
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
