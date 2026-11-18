//! The app core on the headless platform, with a GPU device (lavapipe in CI): scripts drive it (a click changes
//! the shot; a resize gives the window its size; the checks find nothing wrong; a full redraw draws the same), the
//! saved state goes to its file and comes back, frames are paced on the display's beat, the platform's events
//! reach the host as its input (in px: points and notches turned), KompozR's title bar frames the content while the
//! platform reports one and what it asks reaches the platform, metrics are read once before input, and the
//! kept frame reaches swapchain images of every age through only what changed (a swapchain faked over the device).
//! One device-holding test after another on this thread; without a device they are skipped, unless
//! `KOMPOZ_GPU_REQUIRED=1`.

#![allow(non_snake_case)]

use kompoz_foundation::prelude::*;
use kompoz_gpu::device::{
    self, Backend, BufferRef, ClearColor, Device, Format, GpuFrameTime, IRect, Pipeline, PipelineDesc, PipelineFactory,
    PipelineId, Sampler, Surface, Texture, TextureId, TextureUsage,
};
use kompoz_graphics::*;
use kompoz_platform::headless::{EventQueue, HeadlessPlatform, run_core};
use kompoz_platform::{
    AppConfig, AppCore, Event, Metrics, MouseButton, Platform, TitleBar, WindowAction, WindowButton,
};
use kompoz_runtime::*;
use kompoz_ui::KompozHost;
use kompoz_ui::draganddrop::{DragAndDropEvent, DragAndDropTarget};
use kompoz_ui::input::*;
use kompoz_ui::media_query::{MediaInfo, PointerPrecision};
use kompoz_ui::platform::{ImeOptions, PlatformCallbacks, TextEditState};
use kompoz_ui::semantics::Role;
use kompoz_ui::text_input::EditCommand;
use kompoz_ui::{Alignment, Modifier};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

type Log = Rc<RefCell<Vec<String>>>;

fn new_log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn push(log: &Log, s: impl Into<String>) {
    log.borrow_mut().push(s.into());
}

fn taken(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

/// A directory of the test's own.
fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kompoz-app-core-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The headless platform with metrics of a test's (px per dp, px per point), its `metrics` calls counted and its
/// media's probes, its log kept.
struct TestPlatform {
    inner: HeadlessPlatform,
    scale: f32,
    point_to_px: f32,
    metrics_read: Rc<Cell<u32>>,
    probes: Rc<Cell<u32>>,
    log: Log,
    /// KompozR's title bar, as the platform reports it; what it asked of the window.
    title_bar: Option<TitleBar>,
    window_actions: Log,
}

impl TestPlatform {
    fn new(options: &[(&str, &str)]) -> TestPlatform {
        let mut inner = HeadlessPlatform::new();
        for (name, value) in options {
            inner.set_option(name, value);
        }
        TestPlatform {
            inner,
            scale: 1.0,
            point_to_px: 1.0,
            metrics_read: Rc::new(Cell::new(0)),
            probes: Rc::new(Cell::new(0)),
            log: new_log(),
            title_bar: None,
            window_actions: new_log(),
        }
    }
}

impl Platform for TestPlatform {
    fn option(&self, name: &str) -> Option<String> {
        self.inner.option(name)
    }
    fn log(&self, message: &str) {
        push(&self.log, message);
    }
    fn now(&self) -> i64 {
        self.inner.now()
    }
    fn backends(&self) -> Vec<Backend> {
        self.inner.backends()
    }
    fn open_window(&mut self, app: &AppConfig, backend: Backend) -> Result<Surface, String> {
        self.inner.open_window(app, backend)
    }
    fn close_window(&mut self) {
        self.inner.close_window();
    }
    fn show_error(&mut self, message: &str) {
        push(&self.log, message);
    }
    fn metrics(&self) -> Metrics {
        self.metrics_read.set(self.metrics_read.get() + 1);
        let m = self.inner.metrics();
        Metrics {
            scale: self.scale,
            point_to_px: self.point_to_px,
            point_width: (m.pixel_width as f32 / self.point_to_px) as i32,
            point_height: (m.pixel_height as f32 / self.point_to_px) as i32,
            safe_area: None,
            language: "en".into(),
            title_bar: self.title_bar.clone(),
            ..m
        }
    }
    fn window_action(&mut self, action: WindowAction, x: f32, y: f32) {
        push(&self.window_actions, format!("{action:?} {x},{y}"));
    }
    fn pixel_size(&self) -> (i32, i32) {
        self.inner.pixel_size()
    }
    fn set_window_size(&mut self, width: i32, height: i32) {
        self.inner.set_window_size(width, height);
    }
    fn media(&mut self, probe: bool) -> MediaInfo {
        if probe {
            self.probes.set(self.probes.get() + 1);
        }
        MediaInfo { pointer_precision: PointerPrecision::Fine, has_camera: probe, ..MediaInfo::default() }
    }
    fn attach(&mut self, host: &KompozHost) -> Rc<dyn PlatformCallbacks> {
        self.inner.attach(host)
    }
    fn composed(&mut self, host: &KompozHost, redraw: bool, frame_time: i64) {
        self.inner.composed(host, redraw, frame_time);
    }
}

fn config(width: f32, height: f32) -> AppConfig {
    AppConfig {
        title: "KompozR app core test".into(),
        width: Dp(width),
        height: Dp(height),
        image_disk_cache_bytes: 0,
        args: vec!["test".into(), "kompoz://item/1".into()],
    }
}

/// A core started on `platform` composing `content`.
fn started(platform: TestPlatform, width: f32, height: f32, content: impl Content + 'static) -> AppCore<TestPlatform> {
    let mut core = AppCore::new(platform, config(width, height), Rc::new(content));
    assert!(core.start(), "the core starts");
    core
}

/// Runs a started core until its script quits.
fn run(core: &mut AppCore<TestPlatform>) {
    let queue: Arc<EventQueue> = core.platform().inner.queue();
    run_core(core, &queue);
}

/// A PNG's size and pixels.
fn picture(path: &std::path::Path) -> ImageBitmap {
    let bytes = std::fs::read(path).unwrap_or_else(|_| panic!("{} was written", path.display()));
    decode_image(&bytes, ImageRequestSize::default()).expect("a PNG")
}

#[kompozable]
fn Counter(seen: Rc<Cell<i32>>) {
    let count = remember_saveable(|| mutable_state_of(0));
    seen.set(count.get());
    Column(
        modifier = Modifier.fill_max_size().background(Color::White).padding(24.dp()),
        content = move || {
            Box(
                modifier = Modifier
                    .size_wh(120.dp(), 80.dp())
                    .background(Color::from_argb(0xff3f51b5))
                    .clickable(role = Some(Role::Button), on_click = move || count.set(count.get() + 1)),
                content_alignment = Alignment::Center,
                content = || BasicText("+1"),
            );
            BasicText(format!("Count: {}", count.get()));
        },
    );
}

fn counter(seen: &Rc<Cell<i32>>) -> impl Content + 'static {
    let seen = seen.clone();
    kompozable_lambda!(move || {
        Counter(seen.clone());
    })
}

// ---------------------------------------------------------------------------------------------------------------

fn a_scripts_click_changes_the_shot() {
    let dir = temp_dir("click");
    let (a, b) = (dir.join("a.png"), dir.join("b.png"));
    let script = format!("wait 30;shot {};click 70 70;wait 30;shot {};quit", a.display(), b.display());
    let seen = Rc::new(Cell::new(-1));
    let options = [
        ("KOMPOZ_SCRIPT", script.as_str()),
        ("KOMPOZ_DAMAGE_CHECK", "1"),
        ("KOMPOZ_KEEP_CHECK", "1"),
        ("KOMPOZ_TRACE", "1"),
    ];
    let mut core = started(TestPlatform::new(&options), 360.0, 480.0, counter(&seen));
    let log = core.platform().log.clone();
    run(&mut core);
    assert_eq!(seen.get(), 1, "the click counted");
    let (before, after) = (picture(&a), picture(&b));
    assert_eq!((before.width(), before.height(), after.width(), after.height()), (360, 480, 360, 480));
    let (p, q) = (before.pixels(), after.pixels());
    let differ = p.chunks(4).zip(q.chunks(4)).filter(|(x, y)| x != y).count();
    assert!(differ > 20 && differ < 360 * 480 / 10, "{differ} pixels differ: the count's text");
    // The button's color is where it is drawn, the window white elsewhere.
    let at = |img: &ImageBitmap, x: usize, y: usize| img.pixels()[(y * 360 + x) * 4..][..4].to_vec();
    assert_eq!(at(&after, 30, 30), [0x3f, 0x51, 0xb5, 255]);
    assert_eq!(at(&after, 300, 400), [255, 255, 255, 255]);
    drop(core);
    let log = taken(&log);
    assert!(log.iter().any(|l| l.starts_with("kompoz: recomposition")), "{log:?}");
    assert!(log.iter().any(|l| l.contains("damage check: 0 of")), "{log:?}");
    assert!(log.iter().any(|l| l.contains("keep check: 0 of")), "{log:?}");
    assert!(!log.iter().any(|l| l.contains("GPU validation")), "{log:?}");
    // All of every frame drawn: the same pictures.
    let c = dir.join("c.png");
    let script = format!("wait 30;click 70 70;wait 30;shot {};quit", c.display());
    let options = [("KOMPOZ_SCRIPT", script.as_str()), ("KOMPOZ_FULL_REDRAW", "1")];
    let mut core = started(TestPlatform::new(&options), 360.0, 480.0, counter(&seen));
    run(&mut core);
    drop(core);
    assert!(picture(&c).pixels() == after.pixels(), "a full redraw draws what only the changes drew");
    let _ = std::fs::remove_dir_all(&dir);
}

fn a_resize_gives_the_window_its_size() {
    let dir = temp_dir("resize");
    let shot = dir.join("small.png");
    let script = format!("wait 2;resize 200 120;wait 3;shot {};quit", shot.display());
    let seen = Rc::new(Cell::new(-1));
    let mut core = started(TestPlatform::new(&[("KOMPOZ_SCRIPT", &script)]), 360.0, 480.0, counter(&seen));
    run(&mut core);
    let host = core.host().unwrap();
    assert_eq!(host.viewport_size(), IntSize::new(200, 120));
    let small = picture(&shot);
    assert_eq!((small.width(), small.height()), (200, 120));
    drop(core);
    let _ = std::fs::remove_dir_all(&dir);
}

fn the_saved_state_goes_to_its_file_and_comes_back() {
    let dir = temp_dir("state");
    let file = dir.join("state.bin");
    let file = file.to_str().unwrap();
    let seen = Rc::new(Cell::new(-1));
    let options = [("KOMPOZ_SCRIPT", "wait 2;click 70 70;wait 2;click 70 70;wait 2;quit"), ("KOMPOZ_STATE_FILE", file)];
    let mut core = started(TestPlatform::new(&options), 360.0, 480.0, counter(&seen));
    run(&mut core);
    drop(core);
    assert_eq!(seen.get(), 2);
    assert!(std::fs::metadata(file).is_ok_and(|m| m.len() > 0), "the state was written");
    // The next run starts where it left.
    let again = Rc::new(Cell::new(-1));
    let options = [("KOMPOZ_SCRIPT", "wait 1;quit"), ("KOMPOZ_STATE_FILE", file)];
    let mut core = started(TestPlatform::new(&options), 360.0, 480.0, counter(&again));
    run(&mut core);
    assert_eq!(again.get(), 2);
    // The command line's address: a deep link.
    assert_eq!(core.host().unwrap().deep_links().pending().as_deref(), Some("kompoz://item/1"));
    drop(core);
    let _ = std::fs::remove_dir_all(&dir);
}

fn frames_are_paced_on_the_displays_beat() {
    let seen = Rc::new(Cell::new(-1));
    let mut core = started(TestPlatform::new(&[]), 100.0, 100.0, counter(&seen));
    assert_eq!(core.frame_due(), None); // (no frame yet: now)
    // The display's time given (a vsync's): the next due 3/5 of a period after it (60 Hz).
    let vsync = core.platform().now();
    let next = core.frame(Some(vsync));
    assert!(!next.quit);
    assert_eq!(core.frame_due(), Some(vsync + 16_666_667 * 3 / 5));
    // On the platform's clock: on the beat of the last (a whole number of periods after it, never before it), the
    // one nearest the frame's start (at most half a period from it), so due 1/10 to 11/10 of a period after it.
    std::thread::sleep(std::time::Duration::from_millis(40));
    let before = core.platform().now();
    core.frame(None);
    let after = core.platform().now();
    let due = core.frame_due().unwrap();
    let beats = (due - 16_666_667 * 3 / 5 - vsync) as f64 / 16_666_667.0;
    assert!((beats - beats.round()).abs() < 1e-6 && beats >= 2.0, "{beats} beats after the vsync");
    assert!(
        due >= before + 16_666_667 / 10 && due <= after + 16_666_667 * 11 / 10,
        "{due} after {before} (the frame ended at {after})"
    );
    // A script in virtual time has its frames at once.
    let mut scripted = started(TestPlatform::new(&[("KOMPOZ_SCRIPT", "wait 5;quit")]), 100.0, 100.0, counter(&seen));
    scripted.frame(None);
    assert_eq!(scripted.frame_due(), None);
    // Work between frames goes to the host's idle work.
    let ran = Rc::new(Cell::new(false));
    let r = ran.clone();
    core.host().unwrap().request_idle_work(
        1,
        0,
        move || {
            r.set(true);
            false
        },
        |_| {},
    );
    core.idle(5_000_000);
    assert!(ran.get());
}

/// A target recording drops.
struct Drops(Log);

impl DragAndDropTarget for Drops {
    fn on_drop(&self, event: &DragAndDropEvent) -> bool {
        let data = event.transfer_data.clone().unwrap_or_default();
        push(&self.0, format!("drop {:?} {:?} {:?}", data.mime_types(), data.text(), data.files()));
        true
    }
}

fn the_platforms_events_are_the_hosts_input() {
    let log = new_log();
    let l = log.clone();
    let drops: Rc<dyn DragAndDropTarget> = Rc::new(Drops(log.clone()));
    let mut platform = TestPlatform::new(&[]);
    platform.scale = 2.0;
    platform.point_to_px = 1.5;
    let content = kompozable_lambda!(move || {
        let l = l.clone();
        let record = move |label: &'static str| {
            let l = l.clone();
            move |_: &AwaitPointerEventScope, e: &PointerEvent| {
                let c = &e.changes[0];
                let mut entry = format!("{label} {},{}", c.position.x, c.position.y);
                match label {
                    "scroll" => entry += &format!(" d{},{}", c.scroll_delta.x, c.scroll_delta.y),
                    "scale" => entry += &format!(" x{:.4} r{}", c.scale_factor, c.rotation),
                    _ => entry += &format!(" {:?}", c.r#type),
                }
                if matches!(c.r#type, PointerType::Stylus | PointerType::Eraser) {
                    entry += &format!(" p{} b{}", c.pressure, e.buttons.0);
                }
                if let Some(b) = e.button {
                    entry += &format!(" b{}", b.index());
                }
                push(&l, entry);
                if matches!(label, "scroll" | "scale") {
                    c.consume();
                }
            }
        };
        Box(modifier = Modifier
            .fill_max_size()
            .on_pointer_event(PointerEventType::Press, on_event = record("press"))
            .on_pointer_event(PointerEventType::Release, on_event = record("release"))
            .on_pointer_event(PointerEventType::Scroll, on_event = record("scroll"))
            .on_pointer_event(PointerEventType::ScaleChange, on_event = record("scale"))
            .drag_and_drop_target(|_: &DragAndDropEvent| true, drops.clone()));
    });
    let mut core = started(platform, 300.0, 300.0, content);
    core.frame(None);
    let none = KeyModifiers::None;
    let ctrl = KeyModifiers::Ctrl;
    // Points become px (1.5 a point); the left button presses, the others are buttons of the mouse's events.
    for e in [
        Event::MouseMove { x: 10.0, y: 20.0, modifiers: none, time: -1 },
        Event::MouseDown { x: 10.0, y: 20.0, button: MouseButton::Left, modifiers: none, time: -1 },
        Event::MouseUp { x: 10.0, y: 20.0, button: MouseButton::Left, modifiers: none, time: -1 },
        Event::MouseDown { x: 10.0, y: 20.0, button: MouseButton::Right, modifiers: none, time: -1 },
        Event::MouseUp { x: 10.0, y: 20.0, button: MouseButton::Right, modifiers: none, time: -1 },
    ] {
        core.handle(&e);
    }
    assert_eq!(
        taken(&log),
        ["press 15,30 Mouse b0", "release 15,30 Mouse b0", "press 15,30 Mouse b1", "release 15,30 Mouse b1"]
    );
    // The wheel's notches as they come (Compose's scroll deltas: the scrollables make a notch 64 dp), its up a
    // scroll up; parts of notches as they come.
    core.handle(&Event::Wheel { x: 10.0, y: 20.0, dx: 0.0, dy: 1.0, modifiers: none, time: -1 });
    core.handle(&Event::Wheel { x: 10.0, y: 20.0, dx: 0.5, dy: -0.25, modifiers: none, time: -1 });
    assert_eq!(taken(&log), ["scroll 15,30 d0,-1", "scroll 15,30 d0.5,0.25"]);
    // Ctrl+wheel zooms (1.1 a notch), Ctrl+Shift+wheel turns (15° a notch); a touchpad pinches.
    core.handle(&Event::Wheel { x: 10.0, y: 20.0, dx: 0.0, dy: 2.0, modifiers: ctrl, time: -1 });
    core.handle(&Event::Wheel { x: 10.0, y: 20.0, dx: 0.0, dy: 1.0, modifiers: ctrl | KeyModifiers::Shift, time: -1 });
    core.handle(&Event::PinchBegin { x: 20.0, y: 20.0 });
    core.handle(&Event::Pinch { x: 20.0, y: 20.0, scale: 1.5, rotation: 10.0 });
    core.handle(&Event::PinchEnd { x: 20.0, y: 20.0 });
    assert_eq!(taken(&log), ["scale 15,30 x1.2100 r0", "scale 15,30 x1.0000 r-15", "scale 30,30 x1.5000 r10"]);
    assert_eq!(core.host().unwrap().keyboard_modifiers(), ctrl | KeyModifiers::Shift); // (what the wheel carried)
    // A finger: pressed and lifted (and hovering no more).
    core.handle(&Event::TouchDown { pointer: 3, x: 40.0, y: 40.0, time: -1 });
    core.handle(&Event::TouchUp { pointer: 3, x: 40.0, y: 40.0, time: -1 });
    assert_eq!(taken(&log), ["press 60,60 Touch", "release 60,60 Touch"]);
    // A stylus: near, touching (its pressure and barrel buttons), lifted, gone; its eraser touching as it comes.
    let stylus = |pointer, eraser, down, pressure, buttons| Event::Stylus {
        pointer,
        eraser,
        x: 40.0,
        y: 40.0,
        down,
        pressure,
        buttons,
        time: -1,
    };
    for e in [
        stylus(7, false, false, 0.0, 0),
        stylus(7, false, true, 0.5, 2),
        stylus(7, false, false, 0.0, 0),
        Event::StylusLeft { pointer: 7 },
        stylus(8, true, true, 1.0, 0),
        Event::StylusLeft { pointer: 8 },
    ] {
        core.handle(&e);
    }
    assert_eq!(
        taken(&log),
        [
            "press 60,60 Stylus p0.5 b2",
            "release 60,60 Stylus p0 b0",
            "press 60,60 Eraser p1 b0",
            "release 60,60 Eraser p0 b0"
        ]
    );
    // Keys carry their modifiers; text and an input method's go to the editor's session.
    let edits = new_log();
    let e = edits.clone();
    let session = core.host().unwrap().text_input_service().start_input(
        TextEditState::default(),
        ImeOptions::default(),
        move |c: &[EditCommand]| push(&e, format!("{c:?}")),
        |_| {},
    );
    core.handle(&Event::KeyDown { key: Key::A, code_point: 'a' as i32, modifiers: KeyModifiers::Shift, time: -1 });
    assert_eq!(core.host().unwrap().keyboard_modifiers(), KeyModifiers::Shift);
    core.handle(&Event::Text("ş".into()));
    // The composing text's caret after it (or hidden), at its start, or a part of it highlighted.
    core.handle(&Event::Preedit { text: "ka".into(), cursor: Some((2, 2)) });
    core.handle(&Event::Preedit { text: "ka".into(), cursor: None });
    core.handle(&Event::Preedit { text: "ka".into(), cursor: Some((0, 0)) });
    core.handle(&Event::Preedit { text: "kar".into(), cursor: Some((1, 3)) });
    core.handle(&Event::Preedit { text: String::new(), cursor: None });
    core.handle(&Event::DeleteSurroundingText { before: 1, after: 0 });
    assert_eq!(
        taken(&edits),
        [
            "[CommitText { text: \"ş\", new_cursor_position: 1 }]",
            "[SetComposingText { text: \"ka\", new_cursor_position: 1 }]",
            "[SetComposingText { text: \"ka\", new_cursor_position: 1 }]",
            "[SetComposingText { text: \"ka\", new_cursor_position: 1 }, SetSelection { start: 2, end: 2 }]",
            "[SetComposingText { text: \"kar\", new_cursor_position: 1 }, SetSelection { start: 3, end: 5 }]",
            "[SetComposingText { text: \"\", new_cursor_position: 1 }]",
            "[DeleteSurroundingText { before: 1, after: 0 }]",
        ]
    );
    session.dispose();
    // Files dropped from a file manager: a uri-list; text from a browser: plain text; nothing: it left.
    for e in [
        Event::DropBegin { x: 20.0, y: 20.0, mime_types: Vec::new() },
        Event::DropMove { x: 21.0, y: 20.0 },
        Event::DropFile { x: 21.0, y: 20.0, path: "/tmp/a b.png".into() },
        Event::DropComplete { x: 21.0, y: 20.0 },
        Event::DropMove { x: 20.0, y: 20.0 },
        Event::DropText { x: 20.0, y: 20.0, text: "merhaba".into() },
        Event::DropComplete { x: 20.0, y: 20.0 },
        Event::DropBegin { x: 20.0, y: 20.0, mime_types: Vec::new() },
        Event::DropComplete { x: 20.0, y: 20.0 },
    ] {
        core.handle(&e);
    }
    assert_eq!(
        taken(&log),
        [
            "drop [\"text/uri-list\"] \"file:///tmp/a%20b.png\" [\"/tmp/a b.png\"]",
            "drop [\"text/plain\"] \"merhaba\" []",
        ]
    );
    // A quit.
    core.handle(&Event::Quit);
    assert!(core.frame(None).quit);
}

fn the_title_bar_frames_the_content_and_asks_the_window() {
    // C++'s WindowFrame around the app's content while the platform reports a title bar: 40 dp across the top (80
    // px at 2 px a dp), the content below; what it asks goes to the platform in its points (1.5 px each).
    let seen = Rc::new(Cell::new(-1));
    let mut platform = TestPlatform::new(&[]);
    platform.scale = 2.0;
    platform.point_to_px = 1.5;
    platform.title_bar = Some(TitleBar { start: vec![], end: vec![WindowButton::Close], maximized: false });
    let actions = platform.window_actions.clone();
    let mut core = started(platform, 300.0, 300.0, counter(&seen));
    core.frame(None);
    assert!(core.title_bar().get().is_some());
    let click = |core: &mut AppCore<TestPlatform>, x: f32, y: f32, button: MouseButton| {
        let none = KeyModifiers::None;
        core.handle(&Event::MouseMove { x, y, modifiers: none, time: -1 });
        core.handle(&Event::MouseDown { x, y, button, modifiers: none, time: -1 });
        core.handle(&Event::MouseUp { x, y, button, modifiers: none, time: -1 });
        core.frame(None);
    };
    assert_eq!(seen.get(), 0);
    // The bar where the counter's button was; the button below it.
    click(&mut core, 70.0, 40.0, MouseButton::Left);
    assert_eq!(seen.get(), 0);
    click(&mut core, 70.0, 120.0, MouseButton::Left);
    assert_eq!(seen.get(), 1);
    // The menu where the bar was pressed with the secondary button; the close button (28 dp, 8 from the end).
    click(&mut core, 100.0, 20.0, MouseButton::Right);
    click(&mut core, 170.0, 25.0, MouseButton::Left);
    assert_eq!(taken(&actions), ["Menu 100,20", "Close 0,0"]);
    // No bar (the compositor decorates the window now): the content the whole window again, its count kept.
    core.platform_mut().title_bar = None;
    core.handle(&Event::MetricsChanged);
    core.frame(None);
    assert!(core.title_bar().get().is_none());
    click(&mut core, 70.0, 40.0, MouseButton::Left);
    assert_eq!(seen.get(), 2);
    click(&mut core, 100.0, 20.0, MouseButton::Right);
    assert!(taken(&actions).is_empty());
}

fn metrics_are_read_once_before_the_next_input_or_frame() {
    let seen = Rc::new(Cell::new(-1));
    let platform = TestPlatform::new(&[]);
    let read = platform.metrics_read.clone();
    let mut core = started(platform, 100.0, 100.0, counter(&seen));
    let after_start = read.get();
    assert_eq!(after_start, 1);
    // Window events say the metrics changed: read once, when input or a frame comes.
    for e in [Event::Resized, Event::MetricsChanged, Event::FocusGained, Event::MetricsChanged] {
        core.handle(&e);
    }
    assert_eq!(read.get(), after_start);
    core.handle(&Event::MouseMove { x: 1.0, y: 1.0, modifiers: KeyModifiers::None, time: -1 });
    assert_eq!(read.get(), after_start + 1);
    core.frame(None);
    assert_eq!(read.get(), after_start + 1);
    core.handle(&Event::MetricsChanged);
    core.frame(None);
    assert_eq!(read.get(), after_start + 2);
}

// ---- A swapchain faked over the device ----------------------------------------------------------------------------

/// What the fake swapchain saw.
#[derive(Default)]
struct SwapchainLog {
    presents: Cell<u32>,
    /// Frames presented without the image discarded: only what changed copied over it.
    partial: Cell<u32>,
    /// Frames whose presented image wasn't the kept frame.
    wrong: RefCell<Vec<String>>,
    /// The platform's media probes, and how many there were when the first frame was presented.
    probes: Rc<Cell<u32>>,
    probes_at_first_present: Cell<Option<u32>>,
}

/// A device presenting into textures of its own, three in turn, each holding what was last presented into it (its
/// age told as a swapchain's), each presented one compared with the kept frame.
struct FakeSwapchain {
    inner: Box<dyn Device>,
    drawable: Box<dyn Fn() -> (i32, i32)>,
    images: Vec<Texture>,
    shown_at: Vec<u32>,
    next: usize,
    acquired: Option<usize>,
    discarded: bool,
    /// The kept frame: the last render target made at the drawable's size.
    kept: Option<TextureId>,
    log: Rc<SwapchainLog>,
}

impl FakeSwapchain {
    fn image(&mut self) -> Option<TextureId> {
        let (w, h) = (self.drawable)();
        if self.images.first().is_none_or(|t| t.width() != w || t.height() != h) {
            let usage = TextureUsage::SAMPLED | TextureUsage::RENDER_TARGET;
            self.images = (0..3).filter_map(|_| self.inner.create_texture(w, h, Format::Rgba8, usage)).collect();
            self.shown_at = vec![0; self.images.len()];
            self.next = 0;
        }
        self.images.get(self.next).map(Texture::id)
    }
}

macro_rules! passed_on {
    ($($name:ident(&self $(, $arg:ident: $ty:ty)*) -> $ret:ty;)*) => {
        $(fn $name(&self $(, $arg: $ty)*) -> $ret { self.inner.$name($($arg),*) })*
    };
}

macro_rules! passed_on_mut {
    ($($name:ident(&mut self $(, $arg:ident: $ty:ty)*) -> $ret:ty;)*) => {
        $(fn $name(&mut self $(, $arg: $ty)*) -> $ret { self.inner.$name($($arg),*) })*
    };
}

impl Device for FakeSwapchain {
    passed_on! {
        backend(&self) -> Backend;
        device_name(&self) -> String;
        validation_errors(&self) -> u32;
        clip_y_down(&self) -> bool;
        pipeline_factory(&self) -> Option<Arc<dyn PipelineFactory>>;
    }
    passed_on_mut! {
        create_pipeline(&mut self, desc: &PipelineDesc<'_>) -> Option<Pipeline>;
        begin_frame(&mut self) -> ();
        write_frame_data(&mut self, data: &[u8], alignment: usize) -> Option<BufferRef>;
        upload(&mut self, texture: TextureId, region: IRect, pixels: &[u8], row_bytes: usize) -> ();
        begin_pass(&mut self, target: TextureId, clear: Option<ClearColor>, reads: &[TextureId], area: Option<IRect>) -> ();
        set_pipeline(&mut self, pipeline: PipelineId) -> ();
        set_scissor(&mut self, rect: IRect) -> ();
        set_uniforms(&mut self, data: &[u8]) -> ();
        set_texture(&mut self, slot: usize, texture: TextureId, sampler: Sampler) -> ();
        draw(&mut self, instances: Option<BufferRef>, vertex_count: u32, instance_count: u32) -> ();
        end_pass(&mut self) -> ();
        generate_mipmaps(&mut self, texture: TextureId) -> ();
        copy_texture(&mut self, source: TextureId, region: IRect, destination: TextureId, x: i32, y: i32) -> ();
        read_pixels(&mut self, texture: TextureId, region: IRect) -> Vec<u8>;
        set_gpu_timing(&mut self, on: bool) -> bool;
        take_gpu_times(&mut self) -> Vec<GpuFrameTime>;
        wait_idle(&mut self) -> ();
    }

    fn create_texture(&mut self, width: i32, height: i32, format: Format, usage: TextureUsage) -> Option<Texture> {
        let t = self.inner.create_texture(width, height, format, usage)?;
        if (width, height) == (self.drawable)() && usage.contains(TextureUsage::RENDER_TARGET) {
            self.kept = Some(t.id());
        }
        Some(t)
    }

    fn swapchain_format(&self) -> Format {
        Format::Rgba8
    }

    fn swapchain_texture(&mut self) -> Option<TextureId> {
        let image = self.image()?;
        self.acquired = Some(self.next);
        self.discarded = false;
        Some(image)
    }

    fn swapchain_age(&self) -> i32 {
        match self.acquired {
            Some(i) if self.shown_at[i] > 0 => (self.log.presents.get() - self.shown_at[i] + 1) as i32,
            _ => 0,
        }
    }

    fn discard(&mut self, texture: TextureId) {
        if self.acquired.and_then(|i| self.images.get(i)).is_some_and(|t| t.id() == texture) {
            self.discarded = true;
        }
        self.inner.discard(texture);
    }

    fn end_frame(&mut self, present: bool) {
        if let (true, Some(i)) = (present, self.acquired.take()) {
            let image = self.images[i].id();
            let all = image.bounds();
            let shown = self.inner.read_pixels(image, all);
            let kept = self.kept.map(|k| self.inner.read_pixels(k, all)).unwrap_or_default();
            let n = self.log.presents.get() + 1;
            self.log.presents.set(n);
            if n == 1 {
                self.log.probes_at_first_present.set(Some(self.log.probes.get()));
            }
            if shown != kept {
                let differ = shown.chunks(4).zip(kept.chunks(4)).filter(|(a, b)| a != b).count();
                self.log.wrong.borrow_mut().push(format!("present {n}: {differ} pixels differ"));
            }
            if !self.discarded {
                self.log.partial.set(self.log.partial.get() + 1);
            }
            self.shown_at[i] = n;
            self.next = (i + 1) % self.images.len();
        }
        self.inner.end_frame(false);
    }
}

fn the_kept_frame_reaches_swapchain_images_of_every_age() {
    let seen = Rc::new(Cell::new(-1));
    let mut script = String::from("wait 3");
    for _ in 0..7 {
        script += ";click 70 70;wait 1";
    }
    script += ";resize 300 200;wait 2;click 70 70;wait 2;quit";
    let platform = TestPlatform::new(&[("KOMPOZ_SCRIPT", &script)]);
    let probes = platform.probes.clone();
    let platform_log = platform.log.clone();
    let log = Rc::new(SwapchainLog { probes: probes.clone(), ..SwapchainLog::default() });
    let l = log.clone();
    let mut core = AppCore::new(platform, config(360.0, 480.0), Rc::new(counter(&seen)));
    core.set_device_factory(Box::new(move |backend, surface: Surface| {
        let Surface { drawable_size, pipeline_cache, .. } = surface;
        let drawable = drawable_size.ok_or("a drawable size")?;
        let inner = device::create(backend, Surface { pipeline_cache, ..Surface::default() })?;
        Ok(Box::new(FakeSwapchain {
            inner,
            drawable,
            images: Vec::new(),
            shown_at: Vec::new(),
            next: 0,
            acquired: None,
            discarded: false,
            kept: None,
            log: l.clone(),
        }))
    }));
    assert!(core.start());
    run(&mut core);
    drop(core);
    assert_eq!(seen.get(), 8);
    let messages = taken(&platform_log);
    assert!(!messages.iter().any(|l| l.contains("GPU validation")), "{messages:?}");
    let wrong = log.wrong.borrow().clone();
    assert!(wrong.is_empty(), "{wrong:?}");
    // Every click shown, most of them copying only what changed (each image three frames old).
    assert!(log.presents.get() >= 9, "{} presents", log.presents.get());
    assert!(
        log.partial.get() >= 5,
        "{} of {} presents copied only what changed",
        log.partial.get(),
        log.presents.get()
    );
    // The microphone and camera looked for after the first frame shown (and as the media are asked for since).
    assert_eq!(log.probes_at_first_present.get(), Some(0));
    assert!(probes.get() >= 1);
}

// ---------------------------------------------------------------------------------------------------------------

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
        ("a_scripts_click_changes_the_shot", a_scripts_click_changes_the_shot),
        ("a_resize_gives_the_window_its_size", a_resize_gives_the_window_its_size),
        ("the_saved_state_goes_to_its_file_and_comes_back", the_saved_state_goes_to_its_file_and_comes_back),
        ("frames_are_paced_on_the_displays_beat", frames_are_paced_on_the_displays_beat),
        ("the_platforms_events_are_the_hosts_input", the_platforms_events_are_the_hosts_input),
        ("the_title_bar_frames_the_content_and_asks_the_window", the_title_bar_frames_the_content_and_asks_the_window),
        ("metrics_are_read_once_before_the_next_input_or_frame", metrics_are_read_once_before_the_next_input_or_frame),
        ("the_kept_frame_reaches_swapchain_images_of_every_age", the_kept_frame_reaches_swapchain_images_of_every_age),
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
