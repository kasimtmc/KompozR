//! Foundation's content receiving (WP7, Compose's `foundation.content`): `Modifier.content_receiver` and its
//! listener, given what is pasted into a state-based field (from the clipboard), dropped (on the receiver or on a
//! field under it) and committed by an input method; nested receivers (the nearest first, what it leaves to the one
//! around it, then to the field), the drag's start, enter, exit and end (counted: going into a receiver or field
//! inside one isn't leaving it), `consume`, and media types matched as Android's ClipDescription matches them. No C++
//! test had it: Compose's behaviour is the measure (its Android actuals: Compose for desktop leaves the API
//! unimplemented). The rich clipboard (`Clipboard`'s entries, `ClipboardManager`'s text) through the host too.
//!
//! The texts are 16 sp in the fake backend: 8 px a character, 20 px lines. Numbers in px.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_coroutines::LaunchedEffect;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_text::{AnnotatedString, TextStyle};
use kompoz_ui::clipboard::{ClipEntry, ClipItem, ClipMetadata, ClipValue};
use kompoz_ui::draganddrop::DragAndDropTransferData;
use kompoz_ui::input::*;
use kompoz_ui::platform::{LocalClipboard, LocalClipboardManager, PlatformCallbacks};
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// Helpers

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn style16() -> TextStyle {
    TextStyle { font_size: 16.sp(), ..TextStyle::Default }
}

/// A platform keeping its clipboard's entry as it is put there.
#[derive(Default)]
struct ClipPlatform {
    clipboard: RefCell<Option<ClipEntry>>,
}

impl PlatformCallbacks for ClipPlatform {
    fn get_clip_entry(&self) -> Option<ClipEntry> {
        self.clipboard.borrow().clone()
    }
    fn set_clip_entry(&self, entry: Option<ClipEntry>) {
        *self.clipboard.borrow_mut() = entry;
    }
}

fn platform(rule: &KompozTestRule) -> Rc<ClipPlatform> {
    let p = Rc::new(ClipPlatform::default());
    rule.host().set_callbacks(p.clone());
    p
}

type Log = Rc<RefCell<Vec<String>>>;

fn taken(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

/// A listener logging what it is told (`<name> start`, `enter`, `exit`, `end`; `<name> <source> <items>` for content:
/// each item's types, `|` between them) and taking the items `takes` says it takes.
struct Logged {
    name: &'static str,
    log: Log,
    takes: fn(&ClipItem) -> bool,
}

impl ReceiveContentListener for Logged {
    fn on_drag_start(&self) {
        self.log.borrow_mut().push(format!("{} start", self.name));
    }
    fn on_drag_end(&self) {
        self.log.borrow_mut().push(format!("{} end", self.name));
    }
    fn on_drag_enter(&self) {
        self.log.borrow_mut().push(format!("{} enter", self.name));
    }
    fn on_drag_exit(&self) {
        self.log.borrow_mut().push(format!("{} exit", self.name));
    }
    fn on_receive(&self, content: TransferableContent) -> Option<TransferableContent> {
        let items: Vec<String> = content.clip_entry().items().iter().map(|i| i.mime_types().join(",")).collect();
        self.log.borrow_mut().push(format!("{} {} {}", self.name, content.source(), items.join("|")));
        content.consume(self.takes)
    }
}

fn logged(name: &'static str, log: &Log, takes: fn(&ClipItem) -> bool) -> Logged {
    Logged { name, log: log.clone(), takes }
}

fn takes_images(item: &ClipItem) -> bool {
    item.has_mime_type("image/*")
}

fn takes_files(item: &ClipItem) -> bool {
    !item.files().is_empty()
}

fn takes_nothing(_: &ClipItem) -> bool {
    false
}

fn takes_text(item: &ClipItem) -> bool {
    item.text().is_some()
}

fn red() -> ImageBitmap {
    ImageBitmap::from_pixels(2, 2, [255, 0, 0, 255].repeat(4)).unwrap()
}

fn key(rule: &KompozTestRule, key: Key, modifiers: KeyModifiers) {
    rule.host().key_down(key, modifiers);
    rule.host().key_up(key, modifiers);
    rule.wait_for_idle();
}

fn secondary_click(rule: &KompozTestRule, p: Offset) {
    rule.host().mouse_button(p, -1, PointerButton::Secondary, true);
    rule.host().mouse_button(p, -1, PointerButton::Secondary, false);
    rule.wait_for_idle();
}

fn center(rule: &KompozTestRule, tag: &str) -> Offset {
    rule.on_node_with_tag(tag).fetch_semantics_node().bounds_in_root().center()
}

/// What a drop from outside carries.
fn dropped(entry: ClipEntry) -> DragAndDropTransferData {
    DragAndDropTransferData { clip_entry: entry, ..Default::default() }
}

// ---------------------------------------------------------------------------------------------------------------
// Tests

/// Compose's ReceiveContentBasicSample: a field taking pasted images (its listener takes the image items), the
/// text of what is pasted still the field's; an image alone pasted leaves the field as it was, and its paste is
/// offered only because a receiver is there.
fn pasting_an_image_into_a_field_under_a_receiver() {
    let log: Log = Rc::default();
    let images: Rc<RefCell<Vec<(i32, i32)>>> = Rc::default();
    let state = TextFieldState::new("");
    let (l, s, im) = (log.clone(), state.clone(), images.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, im, s) = (l.clone(), im.clone(), s.clone());
        let listener = move |content: TransferableContent| {
            l.borrow_mut().push(format!("received {}", content.source()));
            if !content.has_media_type(&MediaType::Image) {
                return Some(content);
            }
            content.consume(|item| match item.image() {
                Some(image) => {
                    im.borrow_mut().push((image.width(), image.height()));
                    true
                }
                None => false,
            })
        };
        Column(move || {
            BasicTextField(
                s.clone(),
                modifier = Modifier.width(300.dp()).content_receiver(listener.clone()).test_tag("alan"),
                text_style = style16(),
            );
            BasicTextField(TextFieldState::new("öbür"), modifier = Modifier.test_tag("öbür"), text_style = style16());
        });
    }));
    let p = platform(&rule);
    rule.on_node_with_tag("alan").perform_click();
    rule.wait_for_idle();
    // An image and text: the image the listener's, the text the field's.
    let entry = ClipEntry::new(vec![ClipItem::with_image(red()), ClipItem::with_text("yazı")]);
    *p.clipboard.borrow_mut() = Some(entry);
    key(&rule, Key::V, shortcut_modifier());
    assert_eq!(taken(&log), ["received Source.Clipboard"]);
    assert_eq!(*images.borrow(), [(2, 2)]);
    assert_eq!(state.text(), "yazı");
    // Text alone: given to the listener first (it gives it back), pasted.
    *p.clipboard.borrow_mut() = Some(ClipEntry::with_plain_text(" ve"));
    key(&rule, Key::V, shortcut_modifier());
    assert_eq!(taken(&log), ["received Source.Clipboard"]);
    assert_eq!(state.text(), "yazı ve");
    // An image alone: the listener takes it all, nothing pasted. The field's menu offers Paste for it (a receiver
    // takes anything); a field with no receiver doesn't.
    *p.clipboard.borrow_mut() = Some(ClipEntry::with_image(red()));
    secondary_click(&rule, center(&rule, "alan"));
    rule.on_node_with_text("Paste").assert_is_enabled().perform_click();
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["received Source.Clipboard"]);
    assert_eq!(images.borrow().len(), 2);
    assert_eq!(state.text(), "yazı ve");
    secondary_click(&rule, center(&rule, "öbür"));
    rule.on_node_with_text("Paste").assert_is_not_enabled();
    // Copying from the field puts its text there (an entry of the app's: its styles kept within the app).
    key(&rule, Key::Escape, KeyModifiers::None);
    rule.on_node_with_tag("alan").perform_click();
    rule.wait_for_idle();
    key(&rule, Key::A, shortcut_modifier());
    key(&rule, Key::C, shortcut_modifier());
    let copied = p.clipboard.borrow().clone().unwrap();
    assert_eq!(copied.read_annotated_string().map(|t| t.text().to_string()).as_deref(), Some("yazı ve"));
    assert_eq!(copied.clip_metadata().mime_types(), ["text/plain"]);
}

/// Compose's ReceiveContentFullSample's drop part: a box under a receiver takes dropped files (a drag of anything
/// starts for it: what it takes is its listener's to say); told the drag started, came over it, left, ended; a drop
/// it gives back whole isn't taken.
fn dropping_files_on_a_receiver_box() {
    let log: Log = Rc::default();
    let l = log.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let l = l.clone();
            Column(move || {
                Box(modifier = Modifier.size(100.dp()).content_receiver(logged("kutu", &l, takes_files)));
                Box(modifier = Modifier.size(100.dp()));
            });
        }),
        IntSize::new(100, 200),
    );
    let h = rule.host();
    // Files come from a file manager: its own types (as X11 lists them), matched as media types.
    let offered: Vec<String> = ["text/uri-list", "UTF8_STRING", "x-special/gnome-icon-list"].map(String::from).into();
    let metadata = ClipMetadata::of_offered(&offered);
    assert!(h.external_drag_entered(metadata.mime_types().to_vec()));
    assert_eq!(taken(&log), ["kutu start"]);
    h.external_drag_moved(at(50.0, 150.0)); // (not over it)
    assert!(taken(&log).is_empty());
    h.external_drag_moved(at(50.0, 50.0));
    assert_eq!(taken(&log), ["kutu enter"]);
    let files = ClipItem::of(vec![
        ("text/uri-list".into(), ClipValue::Text("file:///tmp/a%20b.png\r\nfile:///tmp/c.txt\r\n".into())),
        ("UTF8_STRING".into(), ClipValue::Text("/tmp/a b.png\n/tmp/c.txt".into())),
    ]);
    let entry = ClipEntry::with_metadata(vec![files], metadata);
    assert!(h.external_drop(dropped(entry.clone()), at(50.0, 60.0)));
    assert_eq!(taken(&log), ["kutu Source.DragAndDrop text/uri-list,UTF8_STRING", "kutu end"]);
    // Text it doesn't take: given back whole, the drop isn't taken.
    h.external_drag_entered(vec!["text/plain".into()]);
    h.external_drag_moved(at(50.0, 50.0));
    h.external_drag_moved(at(50.0, 150.0)); // (it left)
    h.external_drag_moved(at(50.0, 50.0));
    assert!(!h.external_drop(dropped(ClipEntry::with_plain_text("merhaba")), at(50.0, 50.0)));
    assert_eq!(
        taken(&log),
        ["kutu start", "kutu enter", "kutu exit", "kutu enter", "kutu Source.DragAndDrop text/plain", "kutu end"]
    );
    // Brought over and taken away: started, entered, left, ended.
    h.external_drag_entered(vec!["image/png".into()]);
    h.external_drag_moved(at(10.0, 10.0));
    h.external_drag_left();
    assert_eq!(taken(&log), ["kutu start", "kutu enter", "kutu exit", "kutu end"]);
}

/// Receivers inside receivers: the nearest first, what it leaves to the one around it; entering the inner one (or a
/// field inside) isn't leaving the outer one — it hears one enter and one exit.
fn nested_receivers_pass_on_what_they_leave() {
    let log: Log = Rc::default();
    let l = log.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let l = l.clone();
            Box(
                modifier = Modifier.size(200.dp()).content_receiver(logged("dış", &l, takes_text)),
                content = move || {
                    Box(modifier = Modifier.size(100.dp()).content_receiver(logged("iç", &l, takes_images)));
                },
            );
        }),
        IntSize::new(200, 200),
    );
    let h = rule.host();
    h.external_drag_entered(vec!["image/png".into(), "text/plain".into(), "text/uri-list".into()]);
    assert_eq!(taken(&log), ["dış start", "iç start"]);
    h.external_drag_moved(at(150.0, 150.0));
    assert_eq!(taken(&log), ["dış enter"]);
    h.external_drag_moved(at(50.0, 50.0)); // (into the inner one: the outer one told it came in again, counted)
    assert_eq!(taken(&log), ["iç enter"]);
    h.external_drag_moved(at(150.0, 50.0)); // (out of the inner one: the outer one still has it)
    assert_eq!(taken(&log), ["iç exit"]);
    h.external_drag_moved(at(50.0, 50.0));
    assert_eq!(taken(&log), ["iç enter"]);
    // Dropped on the inner one: it takes the image, the outer one the text, the URI is left (taken all the same:
    // something was).
    let entry = ClipEntry::new(vec![
        ClipItem::with_image(red()),
        ClipItem::with_text("yazı"),
        ClipItem::with_uri("https://kompoz.dev"),
    ]);
    assert!(h.external_drop(dropped(entry), at(50.0, 50.0)));
    let log = taken(&log);
    assert_eq!(
        log[..2],
        [
            "iç Source.DragAndDrop image/png|text/plain|text/uri-list".to_string(),
            "dış Source.DragAndDrop text/plain|text/uri-list".to_string()
        ]
    );
    assert!(log[2..].contains(&"iç end".to_string()) && log[2..].contains(&"dış end".to_string()));
    assert_eq!(log.len(), 4);
}

/// A state-based field's drops (Compose's TextFieldDecoratorModifier with Android's `textFieldDragAndDropNode`):
/// text dropped goes in at the caret the drag put where it is; under a receiver everything is taken — the receivers
/// first, what they leave of its text inserted —, and the field tells the receivers the drag came in and left (as
/// a receiver would). A drag of no text over a field with no receiver doesn't start for it.
fn a_field_takes_dropped_text_and_receivers_go_first() {
    let log: Log = Rc::default();
    let plain = TextFieldState::new("abcd");
    let received = TextFieldState::new("12");
    let (l, a, b) = (log.clone(), plain.clone(), received.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (l, a, b) = (l.clone(), a.clone(), b.clone());
            Column(move || {
                BasicTextField(a.clone(), modifier = Modifier.width(200.dp()).test_tag("düz"), text_style = style16());
                Box(
                    modifier = Modifier.width(200.dp()).content_receiver(logged("alıcı", &l, takes_images)),
                    content = {
                        let b = b.clone();
                        move || {
                            BasicTextField(
                                b.clone(),
                                modifier = Modifier.width(200.dp()).test_tag("alıcılı"),
                                text_style = style16(),
                            );
                        }
                    },
                );
            });
        }),
        IntSize::new(200, 200),
    );
    let _p = platform(&rule);
    let h = rule.host();
    let plain_y = center(&rule, "düz").y;
    let received_y = center(&rule, "alıcılı").y;
    // Text over the plain field: the caret follows it (between "ab" and "cd"), dropped there.
    h.external_drag_entered(vec!["text/plain".into()]);
    h.external_drag_moved(at(16.0, plain_y));
    rule.wait_for_idle();
    assert_eq!(plain.selection(), kompoz_text::TextRange::at(2));
    assert!(h.external_drop(dropped(ClipEntry::with_plain_text("XY")), at(16.0, plain_y)));
    rule.wait_for_idle();
    assert_eq!(plain.text(), "abXYcd");
    assert_eq!(taken(&log), ["alıcı start", "alıcı end"]);
    // An image over it: no receiver, no text — it doesn't start for the field (the receiver's box, which takes
    // part in every drag, does).
    assert!(h.external_drag_entered(vec!["image/png".into()]));
    h.external_drag_moved(at(16.0, plain_y));
    assert!(!h.external_drop(dropped(ClipEntry::with_image(red())), at(16.0, plain_y)));
    assert_eq!(plain.text(), "abXYcd");
    taken(&log);
    // Over the field under the receiver: told it came in (through the field), the image the receiver's, the text
    // inserted where the caret is (after "1").
    h.external_drag_entered(vec!["image/png".into(), "text/plain".into()]);
    h.external_drag_moved(at(8.0, received_y));
    rule.wait_for_idle();
    assert_eq!(received.selection(), kompoz_text::TextRange::at(1));
    let entry = ClipEntry::new(vec![ClipItem::with_image(red()), ClipItem::with_text("ç")]);
    assert!(h.external_drop(dropped(entry), at(8.0, received_y)));
    rule.wait_for_idle();
    assert_eq!(received.text(), "1ç2");
    let log = taken(&log);
    assert_eq!(log[0], "alıcı start");
    assert!(log.contains(&"alıcı enter".to_string()), "{log:?}");
    assert!(log.contains(&"alıcı Source.DragAndDrop image/png|text/plain".to_string()), "{log:?}");
    assert_eq!(log.last().map(String::as_str), Some("alıcı end"));
}

/// Content an input method commits (Android's commitContent: Linux's input methods send none, so the host's
/// `commit_content` is what a platform calls): a focused state-based field under a receiver takes any (its session
/// tells the input method `*/*`), given to the receivers from the keyboard with its link; a field without one takes
/// none.
fn keyboard_content_goes_to_the_receivers() {
    let log: Log = Rc::default();
    let links: Rc<RefCell<Vec<Option<String>>>> = Rc::default();
    let (l, li) = (log.clone(), links.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, li) = (l.clone(), li.clone());
        let listener = move |content: TransferableContent| {
            l.borrow_mut().push(format!("received {}", content.source()));
            li.borrow_mut().push(content.platform_transferable_content().and_then(|p| p.link_uri.clone()));
            if content.has_media_type(&MediaType::Image) { None } else { Some(content) }
        };
        Column(move || {
            BasicTextField(
                TextFieldState::new(""),
                modifier = Modifier.content_receiver(listener.clone()).test_tag("alıcılı"),
                text_style = style16(),
            );
            BasicTextField(TextFieldState::new(""), modifier = Modifier.test_tag("düz"), text_style = style16());
        });
    }));
    let _p = platform(&rule);
    let h = rule.host();
    rule.on_node_with_tag("alıcılı").perform_click();
    rule.wait_for_idle();
    assert_eq!(h.text_input_service().session_content_mime_types(), ["*/*"]);
    let gif = ClipEntry::with_metadata(
        vec![ClipItem::with_uri("content://klavye/kedi.gif")],
        ClipMetadata::new(vec!["image/gif".into()]),
    );
    assert!(h.commit_content(gif, Some("https://klavye.example/kedi".into())));
    assert_eq!(taken(&log), ["received Source.Keyboard"]);
    assert_eq!(*links.borrow(), [Some("https://klavye.example/kedi".to_string())]);
    // Given back whole: not taken.
    assert!(!h.commit_content(ClipEntry::with_plain_text("x"), None));
    assert_eq!(taken(&log), ["received Source.Keyboard"]);
    // A field with no receiver takes text only.
    rule.on_node_with_tag("düz").perform_click();
    rule.wait_for_idle();
    assert!(h.text_input_service().session_content_mime_types().is_empty());
    assert!(!h.commit_content(ClipEntry::with_image(red()), None));
    assert!(taken(&log).is_empty());
}

/// Compose: only the state-based field takes part in receiving content — the value-based one (CoreTextField)
/// pastes its text and never asks a receiver around it.
fn the_value_based_field_isnt_a_receiver() {
    let log: Log = Rc::default();
    let value = mutable_state_of(String::new());
    let (l, v) = (log.clone(), value.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, v, w) = (l.clone(), v.clone(), value_of(&v));
        Box(
            modifier = Modifier.content_receiver(logged("alıcı", &l, takes_nothing)),
            content = move || {
                let v = v.clone();
                BasicTextFieldString(
                    w.clone(),
                    move |it: String| v.set(it),
                    modifier = Modifier.width(200.dp()).test_tag("eski"),
                    text_style = style16(),
                );
            },
        );
    }));
    let p = platform(&rule);
    rule.on_node_with_tag("eski").perform_click();
    rule.wait_for_idle();
    *p.clipboard.borrow_mut() = Some(ClipEntry::new(vec![ClipItem::with_image(red()), ClipItem::with_text("eski")]));
    key(&rule, Key::V, shortcut_modifier());
    assert_eq!(value.get(), "");
    // (Its paste is the first item's text, as Compose's `readAnnotatedString`: the image has none.)
    *p.clipboard.borrow_mut() = Some(ClipEntry::with_plain_text("eski"));
    key(&rule, Key::V, shortcut_modifier());
    assert_eq!(value.get(), "eski");
    assert!(taken(&log).is_empty());
    assert!(!rule.host().commit_content(ClipEntry::with_image(red()), None));
}

fn value_of(state: &MutableState<String>) -> String {
    state.get()
}

/// The clipboard through Compose's API: `Clipboard`'s suspending `getClipEntry`/`setClipEntry` (an app's coroutine)
/// and the text one Compose deprecated (`ClipboardManager`), on the platform's clipboard.
fn the_clipboard_holds_entries() {
    let read: Rc<RefCell<Vec<String>>> = Rc::default();
    let go = mutable_state_of(false);
    let (r, g) = (read.clone(), go.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let clipboard = LocalClipboard.current();
        let manager = LocalClipboardManager.current();
        let r = r.clone();
        if !g.get() {
            return;
        }
        LaunchedEffect((), async move {
            let entry =
                ClipEntry::new(vec![ClipItem::with_html_text("kalın", "<b>kalın</b>"), ClipItem::with_image(red())]);
            clipboard.set_clip_entry(Some(entry)).await;
            let back = clipboard.get_clip_entry().await.unwrap();
            r.borrow_mut().push(back.clip_metadata().mime_types().join(","));
            r.borrow_mut().push(back.items()[0].html_text().unwrap_or_default());
            r.borrow_mut().push(format!("{:?}", manager.get_text().map(|t| t.text().to_string())));
            manager.set_text(AnnotatedString::new("düz"));
            r.borrow_mut().push(format!("{}", manager.has_text()));
            let clip = manager.get_clip().unwrap();
            r.borrow_mut().push(clip.read_plain_text().unwrap_or_default());
            clipboard.set_clip_entry(None).await;
            let cleared = clipboard.get_clip_entry().await.is_none();
            r.borrow_mut().push(format!("{cleared}"));
        });
    }));
    let _p = platform(&rule);
    go.set(true);
    rule.wait_for_idle();
    assert_eq!(
        *read.borrow(),
        ["text/html,text/plain,image/png", "<b>kalın</b>", "Some(\"kalın\")", "true", "düz", "true"]
    );
}

tests!(
    pasting_an_image_into_a_field_under_a_receiver,
    dropping_files_on_a_receiver_box,
    nested_receivers_pass_on_what_they_leave,
    a_field_takes_dropped_text_and_receivers_go_first,
    keyboard_content_goes_to_the_receivers,
    the_value_based_field_isnt_a_receiver,
    the_clipboard_holds_entries,
);
