//! The macOS platform on a real display (`KOMPOZ_DISPLAY_TESTS=1`; on macOS — CI's macos runner —, skipped
//! otherwise), each app in a process of its own (this test run again), `KOMPOZ_SCALE=1` (shots of known sizes on
//! any display), Metal's validation where the environment asks for it (`MTL_DEBUG_LAYER=1`):
//! - the Counter run by CI's script in an NSWindow: a shot, a click on its button, another, the window resized, a
//!   third — the click changed what is shown, the shots the window's size, the resized one the new size;
//! - the app's windows: a click opening two `Window`s of their own (an NSWindow and a Metal device each), one at its
//!   state's size and place, one zoomed (maximized) as its state says, a script reaching each by its title, the
//!   first's close its `on_close_request` (it closes, the main window going on);
//! - a transparent, borderless window with an icon (the Dock's) and a menu bar (the menu bar's: macOS's own, not
//!   drawn in the window): see-through where it draws nothing, its content as drawn;
//! - the Counter with a text field read through NSAccessibility from inside the app, as AppKit's server reads it for
//!   the assistive technologies (a thread of the app's process asking on the main thread through its dispatch
//!   queue): the view's elements, their roles, labels, values and frames, the element at a point, the old attribute
//!   API's role and WebKit's AXInvalid; the button pressed and the count read again; the field focused, its text, its
//!   caret's line and box, its selection set and read back. (In-process: asking from another process through
//!   ApplicationServices' AXUIElement needs the Accessibility permission, which CI's runners may not give.)
//!
//! Each run: the app ended as its script says (the last as its reader asks), a renderer on Metal for each window,
//! nothing wrong logged.

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

    /// The app the child process runs (its value: "counter", "windows", "extras", "accessibility").
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

    /// The Counter with a text field ("merhaba", tagged "alan", its input wrong: "Zorunlu alan").
    #[kompozable]
    pub fn Accessible() {
        let count = remember_saveable(|| mutable_state_of(0));
        let field = remember_text_field_state("merhaba".to_string());
        Column(
            modifier = Modifier.fill_max_size().background(Color::White).padding(24.dp()),
            vertical_arrangement = Arrangement::spaced_by(16.dp()),
            content = move || {
                Box(
                    modifier = Modifier
                        .size_wh(120.dp(), 80.dp())
                        .background(Color::from_argb(0xff3f51b5))
                        .clickable(role = Some(Role::Button), on_click = move || count.set(count.get() + 1)),
                    content_alignment = Alignment::Center,
                    content = || BasicText("+1"),
                );
                BasicText(format!("Sayaç: {}", count.get()));
                BasicTextField(
                    field.clone(),
                    modifier = Modifier.width(200.dp()).test_tag("alan").semantics(|c| c.error("Zorunlu alan")),
                    line_limits = TextFieldLineLimits::SingleLine,
                );
            },
        );
    }

    /// What the accessibility test reads of an element (NSAccessibility's properties, as plain data: the elements
    /// stay on the main thread).
    #[derive(Clone, Debug, Default)]
    pub struct Seen {
        pub role: String,
        pub subrole: String,
        pub label: String,
        pub title: String,
        pub value: String,
        pub identifier: String,
        pub frame: (f64, f64, f64, f64),
        pub focused: bool,
    }

    /// NSAccessibility asked as an assistive technology asks (AppKit's server calls these same methods), from the
    /// app's own process.
    mod ax {
        use super::Seen;
        use objc2::rc::Retained;
        use objc2::runtime::AnyObject;
        use objc2::{MainThreadMarker, msg_send};
        use objc2_app_kit::NSApplication;
        use objc2_foundation::{NSArray, NSNumber, NSPoint, NSRange, NSRect, NSString};

        /// `f` on the main thread (inside the app's loop, which serves the main dispatch queue as it waits).
        pub fn on_main<R: Send>(f: impl FnOnce(MainThreadMarker) -> R + Send) -> R {
            let mut out = None;
            dispatch2::DispatchQueue::main().exec_sync(|| {
                out = Some(f(MainThreadMarker::new().expect("the main queue runs on the main thread")));
            });
            out.expect("the main queue ran it")
        }

        /// The app's KompozView (a window's content).
        pub fn view(mtm: MainThreadMarker) -> Option<Retained<AnyObject>> {
            let windows = NSApplication::sharedApplication(mtm).windows();
            let mut views = windows.iter().filter_map(|w| w.contentView());
            views.find(|v| v.class().name().to_str() == Ok("KompozView")).map(Into::into)
        }

        /// An NSString property (a number's as its decimal).
        fn text(obj: &AnyObject, which: &str) -> String {
            // SAFETY: NSAccessibility's getters of an object (an NSString, an NSNumber for a value) or none.
            let value: Option<Retained<AnyObject>> = unsafe {
                match which {
                    "role" => msg_send![obj, accessibilityRole],
                    "subrole" => msg_send![obj, accessibilitySubrole],
                    "label" => msg_send![obj, accessibilityLabel],
                    "title" => msg_send![obj, accessibilityTitle],
                    "identifier" => msg_send![obj, accessibilityIdentifier],
                    _ => msg_send![obj, accessibilityValue],
                }
            };
            let Some(value) = value else { return String::new() };
            if let Some(s) = value.downcast_ref::<NSString>() {
                return s.to_string();
            }
            value.downcast_ref::<NSNumber>().map(|n| n.as_f64().to_string()).unwrap_or_default()
        }

        pub fn seen(obj: &AnyObject) -> Seen {
            // SAFETY: NSAccessibility's frame (an NSRect) and focus (a BOOL).
            let (frame, focused): (NSRect, bool) =
                unsafe { (msg_send![obj, accessibilityFrame], msg_send![obj, isAccessibilityFocused]) };
            Seen {
                role: text(obj, "role"),
                subrole: text(obj, "subrole"),
                label: text(obj, "label"),
                title: text(obj, "title"),
                value: text(obj, "value"),
                identifier: text(obj, "identifier"),
                frame: (frame.origin.x, frame.origin.y, frame.size.width, frame.size.height),
                focused,
            }
        }

        pub fn children(obj: &AnyObject) -> Vec<Retained<AnyObject>> {
            // SAFETY: NSAccessibility's children (an array of elements, or none).
            let kids: Option<Retained<NSArray<AnyObject>>> = unsafe { msg_send![obj, accessibilityChildren] };
            kids.map(|k| k.to_vec()).unwrap_or_default()
        }

        /// The view's elements, depth first, each with what is seen of it.
        pub fn walk(view: &Retained<AnyObject>) -> Vec<(Retained<AnyObject>, Seen)> {
            let mut out = Vec::new();
            let mut stack = children(view);
            stack.reverse();
            while let Some(e) = stack.pop() {
                let mut kids = children(&e);
                kids.reverse();
                let seen = seen(&e);
                out.push((e, seen));
                stack.extend(kids);
            }
            out
        }

        /// The first of the view's elements `pick` takes.
        pub fn find(mtm: MainThreadMarker, pick: impl Fn(&Seen) -> bool) -> Option<Retained<AnyObject>> {
            walk(&view(mtm)?).into_iter().find(|(_, s)| pick(s)).map(|(e, _)| e)
        }

        pub fn press(obj: &AnyObject) -> bool {
            // SAFETY: NSAccessibility's press action.
            unsafe { msg_send![obj, accessibilityPerformPress] }
        }

        pub fn focus(obj: &AnyObject) {
            // SAFETY: NSAccessibility's focus setter.
            unsafe { msg_send![obj, setAccessibilityFocused: true] }
        }

        pub fn selected_range(obj: &AnyObject) -> (usize, usize) {
            // SAFETY: NSAccessibility's selected text range (an NSRange).
            let r: NSRange = unsafe { msg_send![obj, accessibilitySelectedTextRange] };
            (r.location, r.length)
        }

        pub fn select(obj: &AnyObject, location: usize, length: usize) {
            // SAFETY: NSAccessibility's selected text range setter.
            unsafe { msg_send![obj, setAccessibilitySelectedTextRange: NSRange::new(location, length)] }
        }

        pub fn selected_text(obj: &AnyObject) -> String {
            // SAFETY: NSAccessibility's selected text (an NSString, or none).
            let s: Option<Retained<NSString>> = unsafe { msg_send![obj, accessibilitySelectedText] };
            s.map(|s| s.to_string()).unwrap_or_default()
        }

        pub fn characters(obj: &AnyObject) -> isize {
            // SAFETY: NSAccessibility's number of characters (an NSInteger).
            unsafe { msg_send![obj, accessibilityNumberOfCharacters] }
        }

        pub fn line(obj: &AnyObject) -> isize {
            // SAFETY: NSAccessibility's insertion point's line (an NSInteger).
            unsafe { msg_send![obj, accessibilityInsertionPointLineNumber] }
        }

        pub fn frame_for_range(obj: &AnyObject, location: usize, length: usize) -> NSRect {
            // SAFETY: NSAccessibility's frame of a text range (an NSRect).
            unsafe { msg_send![obj, accessibilityFrameForRange: NSRange::new(location, length)] }
        }

        /// Whether the old (attribute) API lists attribute `name`, and its value as text.
        pub fn attribute(obj: &AnyObject, name: &str) -> (bool, String) {
            let name = NSString::from_str(name);
            // SAFETY: NSObject's (NSAccessibility) attribute names (an array of NSStrings) and an attribute's value.
            let (names, value): (Retained<NSArray<NSString>>, Option<Retained<AnyObject>>) = unsafe {
                (msg_send![obj, accessibilityAttributeNames], msg_send![obj, accessibilityAttributeValue: &*name])
            };
            let listed = names.iter().any(|n| *n == *name);
            let value = value.and_then(|v| v.downcast_ref::<NSString>().map(|s| s.to_string())).unwrap_or_default();
            (listed, value)
        }

        /// The element at a point of the screen, as the view answers.
        pub fn hit(view: &Retained<AnyObject>, x: f64, y: f64) -> Option<Retained<AnyObject>> {
            // SAFETY: NSAccessibility's hit test (a point of the screen; an element, or none).
            unsafe { msg_send![view, accessibilityHitTest: NSPoint::new(x, y)] }
        }

        pub fn focused(view: &Retained<AnyObject>) -> Option<Retained<AnyObject>> {
            // SAFETY: NSAccessibility's focused element (an element, or none).
            unsafe { msg_send![view, accessibilityFocusedUIElement] }
        }

        /// The app asked to quit (its delegate's: the loop ends, its state written).
        pub fn quit(mtm: MainThreadMarker) {
            NSApplication::sharedApplication(mtm).terminate(None);
        }
    }

    /// Until `f` (on the main thread) gives something, within 30 s.
    fn until<R: Send>(what: &str, f: impl Fn(objc2::MainThreadMarker) -> Option<R> + Send + Sync) -> R {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(r) = ax::on_main(&f) {
                return r;
            }
            assert!(Instant::now() < deadline, "{what}: not within 30 s");
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn is_button(s: &Seen) -> bool {
        s.role == "AXButton" && s.label == "+1"
    }

    fn is_field(s: &Seen) -> bool {
        s.role == "AXTextField"
    }

    /// In the app's process, on a thread of its own (the app on the main thread): its accessibility read and acted
    /// on, the verdict logged ("accessibility: passed", or what failed), then the app asked to quit.
    pub fn read_the_accessibility() {
        let verdict = std::panic::catch_unwind(|| {
            // The tree (published as AppKit first asks for the view's children): the button, the count, the field.
            let all = until("the view's elements", |mtm| {
                let all: Vec<Seen> = ax::walk(&ax::view(mtm)?).into_iter().map(|(_, s)| s).collect();
                (all.iter().any(is_button) && all.iter().any(is_field)).then_some(all)
            });
            eprintln!("accessibility: the view's elements: {all:#?}");
            assert!(all.iter().any(|s| s.role == "AXStaticText" && s.value == "Sayaç: 0"), "the count");
            let field = all.iter().find(|s| is_field(s)).expect("the field");
            assert_eq!((field.value.as_str(), field.identifier.as_str()), ("merhaba", "alan"));
            // (A plain field: no subrole; no title without a label of its own.)
            assert_eq!((field.subrole.as_str(), field.title.as_str()), ("", ""));
            let button = all.iter().find(|s| is_button(s)).expect("the button").clone();
            let (_, _, w, h) = button.frame;
            assert!(w > 50.0 && h > 30.0, "the button's frame: {:?}", button.frame);
            // The view's hit test at the button's middle: the button.
            let hit = ax::on_main(move |mtm| {
                let (x, y, w, h) = button.frame;
                ax::hit(&ax::view(mtm)?, x + w / 2.0, y + h / 2.0).map(|e| ax::seen(&e))
            });
            assert_eq!(hit.map(|s| (s.role, s.label)), Some(("AXButton".into(), "+1".into())), "the hit test");
            // Through the old (attribute) API: its role; WebKit's AXInvalid on the field whose input is wrong.
            let role = ax::on_main(|mtm| ax::find(mtm, is_button).map(|e| ax::attribute(&e, "AXRole")));
            assert_eq!(role, Some((true, "AXButton".into())), "AXRole");
            let invalid = ax::on_main(|mtm| ax::find(mtm, is_field).map(|e| ax::attribute(&e, "AXInvalid")));
            assert_eq!(invalid, Some((true, "true".into())), "AXInvalid");
            // Pressed: the count goes up.
            assert!(ax::on_main(|mtm| ax::find(mtm, is_button).is_some_and(|e| ax::press(&e))), "pressed");
            until("the count after the press", |mtm| {
                ax::find(mtm, |s| s.role == "AXStaticText" && s.value == "Sayaç: 1").map(|_| ())
            });
            // The field focused: its focus, the view's focused element, its text and its caret.
            ax::on_main(|mtm| ax::find(mtm, is_field).map(|e| ax::focus(&e)));
            let caret = until("the field focused", |mtm| {
                let e = ax::find(mtm, |s| is_field(s) && s.focused)?;
                let focused = ax::focused(&ax::view(mtm)?).map(|f| ax::seen(&f).role);
                (focused.as_deref() == Some("AXTextField")).then(|| ax::selected_range(&e))
            });
            assert!(caret.0 + caret.1 <= 7, "the caret in the text: {caret:?}");
            let (count, line, caret_box) = ax::on_main(|mtm| {
                let e = ax::find(mtm, is_field).expect("the field");
                (ax::characters(&e), ax::line(&e), ax::frame_for_range(&e, 0, 1).size.height)
            });
            assert_eq!((count, line), (7, 0), "its characters, its caret's line");
            assert!(caret_box > 0.0, "the caret's box");
            // Its selection set: read back, its text.
            ax::on_main(|mtm| ax::find(mtm, is_field).map(|e| ax::select(&e, 1, 3)));
            let selected = until("the selection set", |mtm| {
                let e = ax::find(mtm, is_field)?;
                (ax::selected_range(&e) == (1, 3)).then(|| ax::selected_text(&e))
            });
            assert_eq!(selected, "erh");
        });
        match verdict {
            Ok(()) => eprintln!("accessibility: passed"),
            Err(e) => {
                let why =
                    e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()));
                eprintln!("accessibility: FAILED: {}", why.unwrap_or_default());
            }
        }
        ax::on_main(ax::quit);
    }

    pub fn the_accessibility_from_inside() {
        let dir = dir("accessibility");
        let log = run_app("accessibility", &dir, "");
        println!("{log}");
        check_log(&log, 1);
        assert!(!log.contains("accessibility: FAILED"), "{log}");
        assert!(log.contains("accessibility: passed"), "the accessibility wasn't read:\n{log}");
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
            Some("accessibility") => {
                // (Its reader on a thread of its own, the app on the main thread: AppKit's.)
                std::thread::spawn(on_macos::read_the_accessibility);
                App::new("Erişilebilirlik").size(360.dp(), 480.dp()).run(on_macos::Accessible)
            }
            _ => app.run(on_macos::Extras),
        };
    }
    if std::env::var_os("KOMPOZ_DISPLAY_TESTS").is_none_or(|v| v != "1") {
        println!("skipped: KOMPOZ_DISPLAY_TESTS=1 runs it");
        return std::process::ExitCode::SUCCESS;
    }
    let tests: [(&str, fn()); 4] = [
        ("the_counter_in_a_window", on_macos::the_counter_in_a_window),
        ("several_windows", on_macos::several_windows),
        (
            "a_transparent_window_with_an_icon_and_a_menu_bar",
            on_macos::a_transparent_window_with_an_icon_and_a_menu_bar,
        ),
        ("the_accessibility_from_inside", on_macos::the_accessibility_from_inside),
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
