//! What a platform tells the host and asks of it besides input and frames (C++'s host setters and entry points
//! the app core calls): input methods' text through a text input session, trackpad pinches and Ctrl+wheel zooms as
//! scale events, drags from outside the app reaching drag and drop targets, the library's texts and right-to-left
//! languages, the system's and the wallpaper's palettes, the media scope, idle work between frames, deep links, and
//! what tells the platform a frame is wanted. Numbers in px.

#![allow(non_snake_case)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::material_color::palettes::DynamicTonalPalette;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::draganddrop::{DragAndDropEvent, DragAndDropTarget, DragAndDropTransferData};
use kompoz_ui::input::*;
use kompoz_ui::media_query::{MediaInfo, PointerPrecision, derived_media_query, media_query};
use kompoz_ui::platform::{
    ImeAction, ImeOptions, LocalDeepLinks, LocalDynamicTonalPalette, LocalUriHandler, LocalWallpaperTonalPalette,
    PlatformCallbacks, TextEditState,
};
use kompoz_ui::strings::{LocalStrings, Strings, is_rtl_language};
use kompoz_ui::text_input::EditCommand;
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;

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

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

/// A platform recording what the host tells it of text input and links.
struct Recorder(Log);

impl PlatformCallbacks for Recorder {
    fn text_input(&self, active: bool, area: Rect, state: &TextEditState, _: &ImeOptions) {
        push(&self.0, format!("text_input {active} {:?} {}", state.text, area.left));
    }
    fn text_input_changed(&self, state: &TextEditState) {
        push(&self.0, format!("changed {:?}", state.text));
    }
    fn open_uri(&self, uri: &str) {
        push(&self.0, format!("open {uri}"));
    }
}

fn state(text: &str, selection: (usize, usize), composition: Option<(usize, usize)>) -> TextEditState {
    TextEditState { text: text.into(), selection_start: selection.0, selection_end: selection.1, composition }
}

fn applied(mut s: TextEditState, command: EditCommand) -> TextEditState {
    s.apply(&command);
    s
}

fn edit_commands_apply_as_composes_editing_buffer() {
    let commit = |text: &str, p| EditCommand::CommitText { text: text.into(), new_cursor_position: p };
    let compose = |text: &str, p| EditCommand::SetComposingText { text: text.into(), new_cursor_position: p };
    // Committed text replaces the selection (no composition), the caret after it (1) or before it (0).
    assert_eq!(applied(state("abc", (1, 2), None), commit("XY", 1)), state("aXYc", (3, 3), None));
    assert_eq!(applied(state("abc", (1, 2), None), commit("XY", 0)), state("aXYc", (1, 1), None));
    assert_eq!(applied(state("abc", (1, 1), None), commit("X", 3)), state("aXbc", (4, 4), None));
    // A composition: replaced by what composes, then by what is committed (the composition finished).
    let s = applied(state("ab", (2, 2), None), compose("ş", 1));
    assert_eq!(s, state("abş", (4, 4), Some((2, 4))));
    let s = applied(s, compose("şe", 1));
    assert_eq!(s, state("abşe", (5, 5), Some((2, 5))));
    assert_eq!(applied(s.clone(), commit("şey", 1)), state("abşey", (6, 6), None));
    // Composing nothing takes the composition away; finishing keeps its text.
    assert_eq!(applied(s.clone(), compose("", 1)), state("ab", (2, 2), None));
    assert_eq!(applied(s, EditCommand::FinishComposingText), state("abşe", (5, 5), None));
    // The region set (in order, whole characters), none if empty.
    let r = |start, end| EditCommand::SetComposingRegion { start, end };
    assert_eq!(applied(state("çay", (0, 0), None), r(3, 0)), state("çay", (0, 0), Some((0, 3))));
    assert_eq!(applied(state("çay", (0, 0), None), r(1, 4)), state("çay", (0, 0), Some((0, 4))));
    assert_eq!(applied(state("çay", (0, 0), Some((0, 2))), r(2, 2)), state("çay", (0, 0), None));
    // Text around the selection deleted, whole characters (a byte into "ğ" takes all of it).
    let d = |before, after| EditCommand::DeleteSurroundingText { before, after };
    assert_eq!(applied(state("ağaç", (3, 3), None), d(1, 1)), state("aç", (1, 1), None));
    assert_eq!(applied(state("abcdef", (2, 4), Some((3, 5))), d(1, 1)), state("acdf", (1, 3), Some((2, 3))));
    assert_eq!(applied(state("ab", (1, 1), None), d(9, 9)), state("", (0, 0), None));
    let sel = |start, end| EditCommand::SetSelection { start, end };
    assert_eq!(applied(state("ağ", (0, 0), None), sel(2, 99)), state("ağ", (1, 3), None));
    // A caret inside the composing text (an input method's): the composition set, then the selection moved into
    // it; the composition stays, and the next composing text or commit still replaces all of it.
    let s = applied(applied(state("ab", (2, 2), None), compose("şey", 1)), sel(4, 4));
    assert_eq!(s, state("abşey", (4, 4), Some((2, 6))));
    let s = applied(s, sel(3, 5));
    assert_eq!(s, state("abşey", (2, 5), Some((2, 6))), "(a byte into ş: its start)");
    assert_eq!(applied(s.clone(), compose("şu", 1)), state("abşu", (5, 5), Some((2, 5))));
    assert_eq!(applied(s, commit("şu", 1)), state("abşu", (5, 5), None));
}

fn input_methods_place_the_caret_inside_their_composing_text() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {}));
    let host = rule.host();
    // Nothing editing: nowhere to compose.
    assert!(!host.set_composing_text_with_selection("x", 0, 0));
    let edits = new_log();
    let e = edits.clone();
    let service = host.text_input_service();
    let session = service.start_input(
        TextEditState { text: "abXc".into(), selection_start: 2, selection_end: 3, composition: None },
        ImeOptions::default(),
        move |commands: &[EditCommand]| push(&e, format!("{commands:?}")),
        |_| {},
    );
    // Composing in place of the selection, the caret after its first character (bytes, whole characters).
    assert!(host.set_composing_text_with_selection("şey", 2, 2));
    assert_eq!(service.session_state(), Some(state("abşeyc", (4, 4), Some((2, 6)))));
    // Again (the composition replaced), part of it selected (in order); a byte into a character: its start.
    assert!(host.set_composing_text_with_selection("çay", 3, 1));
    assert_eq!(service.session_state(), Some(state("abçayc", (2, 5), Some((2, 6)))));
    // Past its end: at its end.
    assert!(host.set_composing_text_with_selection("ka", 9, 9));
    assert_eq!(service.session_state(), Some(state("abkac", (4, 4), Some((2, 4)))));
    assert_eq!(
        taken(&edits),
        [
            "[SetComposingText { text: \"şey\", new_cursor_position: 1 }, SetSelection { start: 4, end: 4 }]",
            "[SetComposingText { text: \"çay\", new_cursor_position: 1 }, SetSelection { start: 2, end: 5 }]",
            "[SetComposingText { text: \"ka\", new_cursor_position: 1 }, SetSelection { start: 4, end: 4 }]",
        ]
    );
    session.dispose();
}

fn text_input_sessions_take_what_the_input_method_does() {
    let platform = new_log();
    let rule = KompozTestRule::new(kompozable_lambda!(|| {}));
    let host = rule.host();
    host.set_callbacks(Rc::new(Recorder(platform.clone())));
    // Nothing editing: typed text goes nowhere.
    assert!(!host.text_input("x"));
    let edits = new_log();
    let actions = new_log();
    let (e, a) = (edits.clone(), actions.clone());
    let service = host.text_input_service();
    let session = service.start_input(
        TextEditState::caret_at_end("ab"),
        ImeOptions::default(),
        move |commands: &[EditCommand]| push(&e, format!("{commands:?}")),
        move |action| push(&a, format!("{action:?}")),
    );
    assert_eq!(taken(&platform), ["text_input true \"ab\" 0"]);
    // Typed text, an input method's preedit and commit, its action key: to the editor, and kept as the input
    // method sees the text.
    assert!(host.text_input("c"));
    assert!(host.set_composing_text("ş", 1));
    assert_eq!(service.session_state().map(|s| (s.text, s.composition)), Some(("abcş".into(), Some((3, 5)))));
    assert!(host.send_edit_commands(&[EditCommand::CommitText { text: "şu".into(), new_cursor_position: 1 }]));
    assert!(host.ime_action(ImeAction::Done));
    assert_eq!(
        taken(&edits),
        [
            "[CommitText { text: \"c\", new_cursor_position: 1 }]",
            "[SetComposingText { text: \"ş\", new_cursor_position: 1 }]",
            "[CommitText { text: \"şu\", new_cursor_position: 1 }]",
        ]
    );
    assert_eq!(taken(&actions), ["Done"]);
    assert_eq!(service.session_state().map(|s| s.text), Some("abcşu".into()));
    // The editor's own changes: the input method synced where they differ; where it is shown.
    session.update_state(&TextEditState::caret_at_end("abcşu"));
    session.update_state(&TextEditState::caret_at_end("x"));
    session.notify_focused_rect(Rect::new(30.0, 40.0, 50.0, 60.0));
    assert_eq!(taken(&platform), ["changed \"x\"", "text_input true \"x\" 30"]);
    // Another session closes this one (the platform told so first: another field's typing begins); disposing it
    // closes the input method.
    let other = service.start_input(TextEditState::default(), ImeOptions::default(), |_: &[EditCommand]| {}, |_| {});
    assert!(!session.is_open() && other.is_open());
    session.dispose();
    other.dispose();
    assert_eq!(taken(&platform), ["text_input false \"x\" 30", "text_input true \"\" 0", "text_input false \"\" 0"]);
    assert!(!host.text_input("y") && !host.ime_action(ImeAction::Go));
}

fn zooms_and_pinches_are_scale_events() {
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        let record = move |label: &'static str| {
            let l = l.clone();
            move |_: &AwaitPointerEventScope, e: &PointerEvent| {
                let c = &e.changes[0];
                push(&l, format!("{label} {} {} {},{}", c.scale_factor, c.rotation, c.position.x, c.position.y));
                c.consume();
            }
        };
        Box(modifier = Modifier
            .size(100.dp())
            .on_pointer_event(PointerEventType::ScaleStart, on_event = record("start"))
            .on_pointer_event(PointerEventType::ScaleChange, on_event = record("change"))
            .on_pointer_event(PointerEventType::ScaleEnd, on_event = record("end")));
    }));
    let h = rule.host();
    // Ctrl+wheel's zoom: a pinch of its own.
    assert!(h.zoom(at(40.0, 50.0), 1.5, 15.0));
    assert_eq!(taken(&log), ["start 1 0 40,50", "change 1.5 15 40,50", "end 1 0 40,50"]);
    // A trackpad's pinch: its steps within it.
    h.scale(at(10.0, 10.0), PointerEventType::ScaleStart, 1.0, 0.0);
    assert!(h.zoom(at(10.0, 10.0), 0.5, 0.0));
    h.scale(at(10.0, 10.0), PointerEventType::ScaleEnd, 1.0, 0.0);
    assert_eq!(taken(&log), ["start 1 0 10,10", "change 0.5 0 10,10", "end 1 0 10,10"]);
    // Nothing there to take it (the mouse moved there first, as it does: what it left is told so); nothing to zoom.
    h.pointer_move(at(300.0, 300.0), -1, 0);
    assert!(!h.zoom(at(300.0, 300.0), 2.0, 0.0));
    assert!(!h.zoom(at(40.0, 50.0), 1.0, 0.0) && !h.zoom(at(40.0, 50.0), 0.0, 0.0));
    let log = taken(&log);
    assert!(log.is_empty(), "{log:?}");
}

/// A target recording what it is told, taking drops as told.
struct Recording {
    name: &'static str,
    log: Log,
    takes: bool,
}

impl DragAndDropTarget for Recording {
    fn on_drop(&self, event: &DragAndDropEvent) -> bool {
        let data = event.transfer_data.as_ref().map(|d| d.text.clone()).unwrap_or_default();
        push(&self.log, format!("{} drop {data}", self.name));
        self.takes
    }
    fn on_started(&self, _: &DragAndDropEvent) {
        push(&self.log, format!("{} started", self.name));
    }
    fn on_entered(&self, _: &DragAndDropEvent) {
        push(&self.log, format!("{} entered", self.name));
    }
    fn on_moved(&self, e: &DragAndDropEvent) {
        push(&self.log, format!("{} moved {},{}", self.name, e.position_in_root.x, e.position_in_root.y));
    }
    fn on_exited(&self, _: &DragAndDropEvent) {
        push(&self.log, format!("{} exited", self.name));
    }
    fn on_ended(&self, _: &DragAndDropEvent) {
        push(&self.log, format!("{} ended", self.name));
    }
}

fn drags_from_outside_reach_the_targets_under_them() {
    let log = new_log();
    let l = log.clone();
    let files: Rc<dyn DragAndDropTarget> = Rc::new(Recording { name: "files", log: log.clone(), takes: true });
    let text: Rc<dyn DragAndDropTarget> = Rc::new(Recording { name: "text", log: log.clone(), takes: false });
    let outer: Rc<dyn DragAndDropTarget> = Rc::new(Recording { name: "outer", log: l, takes: true });
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (files, text, outer) = (files.clone(), text.clone(), outer.clone());
        let wants = |mime: &'static str| move |e: &DragAndDropEvent| e.mime_types().iter().any(|m| m == mime);
        Column(
            modifier = Modifier.size_wh(300.dp(), 400.dp()).drag_and_drop_target(|_: &DragAndDropEvent| true, outer),
            content = move || {
                Box(modifier = Modifier.size(100.dp()).drag_and_drop_target(wants("text/uri-list"), files.clone()));
                Box(modifier = Modifier.size(100.dp()).drag_and_drop_target(wants("text/plain"), text.clone()));
            },
        );
    }));
    let h = rule.host();
    // Files come: the targets that want them are told; over the files' box, then the window's, then off it.
    assert!(h.external_drag_entered(vec!["text/uri-list".into()]));
    assert_eq!(taken(&log), ["outer started", "files started"]);
    h.external_drag_moved(at(50.0, 50.0));
    // (Into the window's target, and at once into its child's, as Compose's nodes go.)
    assert_eq!(taken(&log), ["outer entered", "files entered", "files moved 50,50", "outer exited"]);
    h.external_drag_moved(at(60.0, 50.0));
    assert_eq!(taken(&log), ["files moved 60,50"]);
    h.external_drag_moved(at(50.0, 150.0)); // (the text box doesn't want files)
    assert_eq!(taken(&log), ["outer entered", "outer moved 50,150", "files exited"]);
    h.external_drag_moved(at(50.0, 40.0));
    taken(&log);
    let data = DragAndDropTransferData {
        mime_types: vec!["text/uri-list".into()],
        text: "file:///tmp/a.png\r\n".into(),
        files: vec!["/tmp/a.png".into()],
        ..Default::default()
    };
    assert!(h.external_drop(data, at(50.0, 40.0)));
    assert_eq!(taken(&log), ["files moved 50,40", "files drop file:///tmp/a.png\r\n", "outer ended", "files ended"]);
    // Text comes and leaves without a drop.
    assert!(h.external_drag_entered(vec!["text/plain".into()]));
    h.external_drag_moved(at(20.0, 120.0));
    h.external_drag_left();
    assert_eq!(
        taken(&log),
        [
            "outer started",
            "text started",
            "outer entered",
            "text entered",
            "text moved 20,120",
            "outer exited",
            "outer exited",
            "text exited",
            "outer ended",
            "text ended"
        ]
    );
    // Dropped on a target that doesn't take it: not taken.
    let data =
        DragAndDropTransferData { mime_types: vec!["text/plain".into()], text: "merhaba".into(), ..Default::default() };
    assert!(!h.external_drop(data, at(20.0, 120.0)));
    let log = taken(&log);
    assert!(
        log.contains(&"text drop merhaba".to_string()) && log.ends_with(&["outer ended".into(), "text ended".into()])
    );
}

fn strings_and_right_to_left_languages() {
    assert_eq!(Strings::for_language("tr_TR.UTF-8").copy, "Kopyala");
    assert_eq!(Strings::for_language("TR").language, "tr");
    assert_eq!(Strings::for_language("tra").language, "en");
    assert_eq!(Strings::for_language("").copy, "Copy");
    for rtl in ["ar", "fa-IR", "he_IL.UTF-8", "ckb", "UR"] {
        assert!(is_rtl_language(rtl), "{rtl}");
    }
    for ltr in ["tr", "en_US", "arn", ""] {
        assert!(!is_rtl_language(ltr), "{ltr}");
    }
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || push(&l, LocalStrings.current().select_all)));
    rule.host().set_strings(Strings::for_language("tr"));
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["Select all", "Tümünü seç"]);
    assert_eq!(rule.host().strings(), Strings::turkish());
}

fn palettes_reach_their_locals() {
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let dynamic = LocalDynamicTonalPalette.current().map(|p| p.accent1[8]);
        let wallpaper = LocalWallpaperTonalPalette.current().map(|p| p.accent1[8]);
        push(&l, format!("{} {}", dynamic.is_some(), wallpaper.is_some()));
    }));
    let palette = DynamicTonalPalette::from_seed(Color::from_argb(0xff3366cc));
    rule.host().set_dynamic_tonal_palette(Some(palette));
    rule.wait_for_idle();
    rule.host().set_wallpaper_tonal_palette(Some(palette));
    rule.wait_for_idle();
    rule.host().set_dynamic_tonal_palette(Some(palette)); // (the same: nothing composed)
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["false false", "true false", "true true"]);
}

fn the_media_scope_follows_the_window_and_the_device() {
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let fine = media_query(|m| m.pointer_precision() == PointerPrecision::Fine);
        let wide = derived_media_query(|m| m.window_width() >= Dp(600.0));
        push(&l, format!("fine {fine} wide {}", wide.get()));
    }));
    let h = rule.host();
    h.set_media_info(MediaInfo { pointer_precision: PointerPrecision::Fine, ..MediaInfo::default() });
    rule.wait_for_idle();
    // Resized in steps: composed again only when the answer changes (at 2 px a dp, 1200 px are 600 dp).
    h.set_density(Density::new(2.0, 1.0));
    for width in [900, 1000, 1100] {
        h.set_viewport_size(IntSize::new(width, 800));
        rule.wait_for_idle();
    }
    h.set_viewport_size(IntSize::new(1200, 800));
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["fine false wide false", "fine true wide false", "fine true wide true"]);
    assert_eq!(h.media_info().pointer_precision, PointerPrecision::Fine);
}

fn idle_work_runs_between_frames_in_order_as_it_fits() {
    let log = new_log();
    let shown = mutable_state_of(0);
    let (l, s) = (log.clone(), shown.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || push(&l, format!("composed {}", s.get()))));
    taken(&log);
    let h = rule.host();
    let (l1, l2, l3) = (log.clone(), log.clone(), log.clone());
    let s = shown.clone();
    h.request_idle_work(
        1,
        0,
        move || {
            s.set(1);
            true
        },
        move |_| push(&l1, "took 1"),
    );
    h.request_idle_work(2, i64::MAX, || true, move |_| push(&l2, "took 2"));
    h.request_idle_work(3, 0, || false, move |_| push(&l3, "took 3"));
    assert!(h.has_idle_work());
    // The first composes what it changed; the second doesn't fit (the third waits behind it).
    h.idle(50_000_000);
    assert_eq!(taken(&log), ["composed 1", "took 1"]);
    assert!(h.has_idle_work());
    // A later request of the same owner replaces it.
    let l4 = log.clone();
    h.request_idle_work(2, 0, || false, move |_| push(&l4, "took 2 again"));
    h.idle(50_000_000);
    assert!(taken(&log).is_empty() && !h.has_idle_work());
}

fn deep_links_wait_until_taken_and_the_apps_own_open_in_it() {
    let platform = new_log();
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let links = LocalDeepLinks.current();
        links.add_handler(1, |uri: &str| uri.starts_with("kompoz://"));
        let taken = links.take(|link: &str| link.starts_with("kompoz://item/"));
        push(&l, format!("{taken:?}"));
        let opener = LocalUriHandler.current();
        if taken.as_deref() == Some("kompoz://item/42") {
            opener.open_uri("kompoz://item/7");
            opener.open_uri("https://example.org");
        }
    }));
    let h = rule.host();
    h.set_callbacks(Rc::new(Recorder(platform.clone())));
    h.open_deep_link("other://x"); // (none takes it: it waits)
    rule.wait_for_idle();
    h.open_deep_link("kompoz://item/42");
    rule.wait_for_idle();
    let log = taken(&log);
    assert_eq!(log[..3], ["None", "None", "Some(\"kompoz://item/42\")"]);
    // The app's own link went to it (taken by the next composition); the other one to the platform.
    assert_eq!(log[3], "Some(\"kompoz://item/7\")");
    assert_eq!(taken(&platform), ["open https://example.org"]);
    assert_eq!(h.deep_links().pending(), None);
}

fn frames_are_wanted_for_what_shows() {
    let count = mutable_state_of(0);
    let c = count.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Text(format!("{}", c.get()));
    }));
    let h = rule.host();
    assert!(!h.wants_frame());
    count.set(1);
    assert!(h.wants_frame());
    rule.wait_for_idle();
    assert!(!h.wants_frame());
    h.request_redraw();
    assert!(h.wants_frame() && h.frame(rule.main_clock().current_time()));
    // The keys held: what pointer events carry.
    h.set_keyboard_modifiers(KeyModifiers::Ctrl | KeyModifiers::Shift);
    assert_eq!(h.keyboard_modifiers(), KeyModifiers::Ctrl | KeyModifiers::Shift);
}

tests!(
    edit_commands_apply_as_composes_editing_buffer,
    input_methods_place_the_caret_inside_their_composing_text,
    text_input_sessions_take_what_the_input_method_does,
    zooms_and_pinches_are_scale_events,
    drags_from_outside_reach_the_targets_under_them,
    strings_and_right_to_left_languages,
    palettes_reach_their_locals,
    the_media_scope_follows_the_window_and_the_device,
    idle_work_runs_between_frames_in_order_as_it_fits,
    deep_links_wait_until_taken_and_the_apps_own_open_in_it,
    frames_are_wanted_for_what_shows,
);
