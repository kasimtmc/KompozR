//! The app's windows on the headless platform, with a GPU device each (lavapipe in CI): a `Window` opens on a
//! window of its own as it is composed and closes as it leaves, its content drawn with the caller's locals; scripts
//! reach it by its title ("@Title: command"); the user closing it is its `on_close_request` (the main window's ends
//! the app); its state is followed both ways (size, minimized, placement, position, to the front); hidden, its
//! composition and state stay; `application { }` ends with `exit_application` or with its last window; the tray
//! icon's menu and notifications reach the platform and its clicks come back. One device-holding test after another
//! on this thread; without a device they are skipped, unless `KOMPOZ_GPU_REQUIRED=1`.

#![allow(non_snake_case, non_upper_case_globals)]

use kompoz_foundation::prelude::*;
use kompoz_gpu::device::{self, NATIVE_BACKEND, Surface};
use kompoz_graphics::*;
use kompoz_platform::headless::{HeadlessPlatform, HeadlessShared, headless_windows, run_windows};
use kompoz_platform::{AppConfig, AppContent, Event, GpuPriority, TrayMenuItem};
use kompoz_runtime::*;
use kompoz_ui::draw::{ColorPainter, Painter};
use kompoz_ui::input::{Key, KeyModifiers};
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

/// A window with a menu bar (Compose Desktop's `MenuBar`): File (Count, Ctrl+K; Shown, a checkbox; More, a submenu
/// with Deep) and a disabled Help; green below the bar; what ran told in `ran`.
fn menu_window(ran: Seen, shown: MutableState<bool>) -> AppContent {
    AppContent::Window(Rc::new(kompozable_lambda!(move || {
        let (ran, shown) = (ran.clone(), shown.clone());
        Window(
            on_close_request = || {},
            title = "Menus".to_string(),
            state = remember_window_state(size = DpSize(Dp(300.0), Dp(200.0))),
            content = move |scope: &FrameWindowScope| {
                let (ran, shown) = (ran.clone(), shown.clone());
                scope.MenuBar(move |bar: &MenuBarScope| {
                    let (ran, shown) = (ran.clone(), shown.clone());
                    bar.Menu(
                        text = "File".to_string(),
                        mnemonic = Some('F'),
                        content = move |m: &MenuScope| {
                            let (count, deep, toggle) = (ran.clone(), ran.clone(), shown.clone());
                            m.Item(
                                text = "Count".to_string(),
                                mnemonic = Some('C'),
                                shortcut = Some(KeyShortcut(Key::K, ctrl = true)),
                                on_click = move || count.borrow_mut().push("count".into()),
                            );
                            m.CheckboxItem(
                                text = "Shown".to_string(),
                                checked = shown.get(),
                                on_checked_change = move |on: bool| toggle.set(on),
                            );
                            m.Separator();
                            m.Menu(
                                text = "More".to_string(),
                                content = move |sub: &MenuScope| {
                                    let deep = deep.clone();
                                    sub.Item(
                                        text = "Deep".to_string(),
                                        on_click = move || deep.borrow_mut().push("deep".into()),
                                    );
                                },
                            );
                        },
                    );
                    bar.Menu(text = "Help".to_string(), enabled = false, content = |_: &MenuScope| {});
                });
                Box(modifier = Modifier.fill_max_size().background(Color::Green));
            },
        );
    })))
}

/// The menu bar KompozR draws (Linux's, as Compose Desktop's Swing JMenuBar): across the window's top, the content
/// below it; its item's shortcut runs it; Alt and the menu's mnemonic opens the menu (its first item highlighted),
/// Enter runs it; Down, Enter toggles the checkbox; into the submenu with Right; the pointer opens the menu and runs
/// an item; a press below closes it; the platform told the menus.
fn a_windows_menu_bar_is_drawn_and_driven() {
    let dir = temp_dir("menu-bar");
    let (closed, open) = (dir.join("closed.png"), dir.join("open.png"));
    let ran: Seen = Rc::default();
    let shown = mutable_state_of(false);
    let script = format!(
        "wait 3; @Menus: shot {closed}; @Menus: key ctrl+k; wait 2; @Menus: key alt+f; wait 3; @Menus: shot {open}; \
         @Menus: key enter; wait 2; @Menus: key alt+f; wait 2; @Menus: key down; @Menus: key enter; wait 2; \
         @Menus: key alt+f; wait 2; @Menus: key up; @Menus: key right; wait 2; @Menus: key enter; wait 2; \
         @Menus: click 20 14; wait 3; @Menus: click 60 46; wait 3; @Menus: click 20 14; wait 3; @Menus: click 150 150; \
         wait 3; @Menus: key enter; wait 2; quit",
        closed = closed.display(),
        open = open.display()
    );
    let shared = run(menu_window(ran.clone(), shown.clone()), &script, &[]);
    let c = picture(&closed);
    let bar = pixel(&c, 150, 10);
    assert_eq!(bar, [0xF2, 0xF2, 0xF2, 255], "the bar across the top");
    assert_eq!(pixel(&c, 150, 100), [0, 255, 0, 255], "the content below it");
    assert_eq!(pixel(&c, 150, 30), [0, 255, 0, 255], "right below it (28 dp)");
    let o = picture(&open);
    assert_eq!(pixel(&o, 250, 100), [0, 255, 0, 255], "beside the menu, the content");
    assert_ne!(pixel(&o, 60, 60), [0, 255, 0, 255], "the menu below File");
    assert_eq!(
        *ran.borrow(),
        ["count", "count", "deep", "count"],
        "Ctrl+K, Alt+F Enter, the submenu by keys, the pointer's click; the press below closed the menu (Enter then \
         did nothing)"
    );
    assert!(shown.peek(), "Alt+F, Down, Enter toggled the checkbox");
    let _ = shared;
}

/// A menu bar the platform shows (`KOMPOZ_HEADLESS_MENU_BAR`): not drawn (the content from the window's top); its
/// clicks come back as `Event::MenuBarItem`; exported (a global menu), its shortcuts are still KompozR's; native
/// (the platform's own menu), the platform's.
fn a_menu_bar_the_platform_shows() {
    for (how, shortcut_runs) in [("exported", true), ("native", false)] {
        let dir = temp_dir(&format!("menu-bar-{how}"));
        let shot = dir.join("shot.png");
        let ran: Seen = Rc::default();
        let shown = mutable_state_of(false);
        let mut platform = HeadlessPlatform::new();
        platform.set_option(
            "KOMPOZ_SCRIPT",
            &format!(
                "wait 3; @Menus: shot {}; @Menus: key ctrl+k; wait 2; @Menus: key alt+f; wait 2; quit",
                shot.display()
            ),
        );
        platform.set_option("KOMPOZ_HEADLESS_MENU_BAR", how);
        let shared = platform.shared();
        let queue = platform.queue();
        let mut windows = headless_windows(platform, &config(), &menu_window(ran.clone(), shown.clone()));
        assert!(windows.start());
        // A few frames, then the platform's menu clicked (More, Deep), as its event comes.
        let mut clicked = false;
        let mut told = None;
        loop {
            for (id, e) in queue.take() {
                windows.handle(id, &e);
            }
            windows.sync();
            for core in windows.cores_mut() {
                core.frame(None);
            }
            windows.sync();
            if !clicked
                && let Some((id, w)) = shared.windows().iter().find(|(_, w)| w.title == "Menus" && w.menu_bar.is_some())
            {
                clicked = true;
                told = w.menu_bar.clone();
                windows.handle(WindowId::MAIN, &Event::MenuBarItem { window: *id, path: vec![0, 3, 0] });
            }
            if windows.quitting() {
                break;
            }
        }
        windows.quit();
        assert_eq!(pixel(&picture(&shot), 150, 10), [0, 255, 0, 255], "{how}: not drawn");
        let mut expected = vec!["deep".to_string()];
        if shortcut_runs {
            expected.push("count".into());
        }
        assert_eq!(*ran.borrow(), expected, "{how}");
        let menus = told.expect("the platform told the menus");
        let texts: Vec<&str> = menus.iter().map(|m| m.text.as_str()).collect();
        assert_eq!(texts, ["File", "Help"]);
        assert_eq!(menus[0].mnemonic, Some('F'));
        assert_eq!(menus[0].items[1].kind, MenuItemKind::Checkbox { checked: false });
        assert_eq!(menus[0].items[0].shortcut, Some((KeyModifiers::Ctrl, "k".to_string())));
        assert_eq!(menus[0].items[3].items[0].text, "Deep");
        assert!(!menus[1].enabled);
    }
}

/// A transparent window (`Window(transparent = true)`, Compose Desktop's): its pixels where the app draws nothing
/// have no alpha (what is behind it shows), what it draws is as drawn; an opaque window's are white there. With
/// `KOMPOZ_HEADLESS_SWAPCHAIN=1` the same through a swapchain (its composite alpha the transparent window's).
fn a_transparent_window_shows_what_is_behind_it() {
    for swapchain in ["0", "1"] {
        let dir = temp_dir(&format!("transparent-{swapchain}"));
        let (clear, opaque) = (dir.join("clear.png"), dir.join("opaque.png"));
        let content = AppContent::Window(Rc::new(kompozable_lambda!(move || {
            for (title, transparent) in [("Clear", true), ("Opaque", false)] {
                Window(
                    on_close_request = || {},
                    title = title.to_string(),
                    undecorated = true,
                    transparent = transparent,
                    state = remember_window_state(size = DpSize(Dp(100.0), Dp(100.0))),
                    content = |_: &FrameWindowScope| {
                        Box(modifier = Modifier.size(Dp(40.0)).background(Color::Red));
                    },
                );
            }
        })));
        let script =
            format!("wait 4; @Clear: shot {}; @Opaque: shot {}; wait 2; quit", clear.display(), opaque.display());
        let shared = Rc::new(RefCell::new(None));
        let s = shared.clone();
        let mut platform = HeadlessPlatform::new();
        platform.set_option("KOMPOZ_SCRIPT", &script);
        platform.set_option("KOMPOZ_HEADLESS_SWAPCHAIN", swapchain);
        let seen = platform.shared();
        let queue = platform.queue();
        let mut windows = headless_windows(platform, &config(), &content);
        assert!(windows.start());
        loop {
            for (id, e) in queue.take() {
                windows.handle(id, &e);
            }
            windows.sync();
            for core in windows.cores_mut() {
                core.frame(None);
            }
            windows.sync();
            if s.borrow().is_none()
                && let Some(w) = seen.window_titled("Clear")
            {
                *s.borrow_mut() = Some(w.transparent);
            }
            if windows.quitting() {
                break;
            }
        }
        windows.quit();
        assert_eq!(*shared.borrow(), Some(true), "the platform's window is a transparent one");
        let c = picture(&clear);
        assert_eq!(pixel(&c, 20, 20), [255, 0, 0, 255], "what it draws as drawn (swapchain {swapchain})");
        assert_eq!(pixel(&c, 80, 80)[3], 0, "nothing where it draws nothing (swapchain {swapchain})");
        let o = picture(&opaque);
        assert_eq!(pixel(&o, 80, 80), [255, 255, 255, 255], "an opaque window's white (swapchain {swapchain})");
    }
}

/// `single_window_application`'s app (Compose Desktop's `singleWindowApplication`): its one window, its content
/// given the window's and the app's scopes (`window.title`, `exit_application`); the user closing it ends the app.
fn a_single_window_application_ends_with_its_window() {
    let dir = temp_dir("single");
    let never = dir.join("never.png");
    let seen: Seen = Rc::default();
    let s = seen.clone();
    let content = kompoz_platform::app::single_window_content(
        WindowState::new(WindowPlacement::Floating, false, WindowPosition::PlatformDefault, DpSize(Dp(80.0), Dp(60.0))),
        true,
        "Solo".to_string(),
        None,
        false,
        false,
        true,
        true,
        true,
        false,
        Rc::new(|_: &kompoz_ui::input::KeyEvent| false),
        Rc::new(|_: &kompoz_ui::input::KeyEvent| false),
        Rc::new(move |scope: &SingleWindowApplicationScope| {
            s.borrow_mut().push(scope.window().title());
            kompozable_block!({
                Second(Color::Green);
            });
        }),
    );
    let script = format!("wait 3; @Solo: window close; wait 5; shot {}; quit", never.display());
    let shared = run(AppContent::Application(Rc::new(content)), &script, &[]);
    assert!(seen.borrow().iter().any(|t| t == "Solo"), "its scope's window: {:?}", seen.borrow());
    assert!(!never.exists(), "the window's close ended the app");
    assert!(shared.window_titled("Solo").is_none());
}

fn main() {
    kompoz_runtime::set_ui_thread();
    let required = std::env::var_os("KOMPOZ_GPU_REQUIRED").is_some_and(|v| v == "1");
    match device::create(NATIVE_BACKEND, Surface::default()) {
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
        ("a_windows_menu_bar_is_drawn_and_driven", a_windows_menu_bar_is_drawn_and_driven),
        ("a_menu_bar_the_platform_shows", a_menu_bar_the_platform_shows),
        ("a_transparent_window_shows_what_is_behind_it", a_transparent_window_shows_what_is_behind_it),
        ("a_single_window_application_ends_with_its_window", a_single_window_application_ends_with_its_window),
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
