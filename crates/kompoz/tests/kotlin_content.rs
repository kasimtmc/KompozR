//! Receiving content in Kotlin's syntax, as a Compose user writes it (Compose's ReceiveContentBasicSample): a
//! BasicTextField taking pasted images through `Modifier.contentReceiver { transferableContent -> … }` (its
//! `hasMediaType(MediaType.Image)`, `consume { item -> … }`, `return@contentReceiver`), the text still the field's; a
//! box taking dropped files only from a drag and drop (`TransferableContent.Source.DragAndDrop`) — through `kompoz!`;
//! and in a `.kt` file (kompoz_file!) receivers inside receivers and the clipboard through LocalClipboard's suspending
//! `setClipEntry`/`getClipEntry`; and the C++ Kompoz's `content_receivers_as_compose_writes_them`
//! (kompozable_tests.cpp 2655-2687, 3555-3578, whole): a listener object hearing of drags too — `object :
//! ReceiveContentListener { override fun onDragEnter() … }` — around a receiver lambda that takes pasted HTML. The
//! same behaviour crates/kompoz-foundation/tests/content.rs checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use kompoz::ui::draganddrop::DragAndDropTransferData;
use kompoz::ui::input::{Key, KeyModifiers};
use kompoz::ui::platform::PlatformCallbacks;
use std::cell::RefCell;
use std::rc::Rc;

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

fn red() -> ImageBitmap {
    ImageBitmap::from_pixels(3, 2, [255, 0, 0, 255].repeat(6)).unwrap()
}

/// The text of the node whose text has `part`.
fn text_with(rule: &KompozTestRule, part: &str) -> String {
    let node = named_args!(rule.on_node_with_text(part, substring = true)).fetch_semantics_node();
    let texts = node.config().get(kompoz::ui::semantics::SemanticsProperties::Text);
    texts.and_then(|t| t.first().map(|t| t.text().to_string())).unwrap_or_default()
}

fn dropped(entry: ClipEntry) -> DragAndDropTransferData {
    DragAndDropTransferData { clip_entry: entry, ..Default::default() }
}

kompoz! {
    @Composable
    fun PasteImages(received: (Int, Int) -> Unit) {
        val state = rememberTextFieldState()
        Column {
            BasicTextField(
                state = state,
                modifier = Modifier.width(300.dp).testTag("alan").contentReceiver { transferableContent ->
                    if (!transferableContent.hasMediaType(MediaType.Image)) {
                        return@contentReceiver transferableContent
                    }
                    transferableContent.consume { item ->
                        val image = item.image()
                        if (image != null) {
                            received(image.width, image.height)
                            true
                        } else {
                            false
                        }
                    }
                },
            )
            Text("alan: ${state.text}")
        }
    }

    @Composable
    fun DropFiles(dropped: (String) -> Unit) {
        Box(
            modifier = Modifier.size(100.dp).contentReceiver { content ->
                if (content.source != TransferableContent.Source.DragAndDrop) {
                    content
                } else {
                    content.consume { item ->
                        item.files().forEach { dropped(it) }
                        item.files().isNotEmpty()
                    }
                }
            },
        )
    }
}

// ---- content_receivers_as_compose_writes_them (kompozable_tests.cpp 2655-2687) ----------------------------------

kompoz! {
    // Compose foundation's content receiving, as its samples write it: a lambda that takes the images
    // and leaves the rest to the field, and a listener object that hears of drags too.
    @Composable
    fun ImagePicker(onImage: (String) -> Unit) {
        val state = rememberTextFieldState()
        var hovering by remember { mutableStateOf(false) }
        val listener = remember {
            object : ReceiveContentListener {
                override fun onDragEnter() { hovering = true }
                override fun onDragExit() { hovering = false }
                override fun onReceive(transferableContent: TransferableContent): TransferableContent? {
                    if (!transferableContent.hasMediaType(MediaType.Image)) return transferableContent
                    return transferableContent.consume { item ->
                        val uri = item.uri
                        if (uri != null) onImage(uri)
                        uri != null
                    }
                }
            }
        }
        Column(modifier = Modifier.contentReceiver(listener)) {
            Text(if (hovering) "üstünde" else "bekliyor")
            Box(modifier = Modifier.contentReceiver { content ->
                if (content.source == TransferableContent.Source.Clipboard && content.hasMediaType(MediaType.HtmlText)) {
                    onImage("html")
                    return@contentReceiver null
                }
                content
            }) {
                BasicTextField(state, modifier = Modifier.width(300.dp).height(40.dp).testTag("alan"))
            }
            Text("metin ${state.text}")
        }
    }
}

fn content_receivers_as_compose_writes_them() {
    let images: Rc<RefCell<Vec<String>>> = Rc::default();
    let im = images.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let im = im.clone();
            ImagePicker(move |uri: String| im.borrow_mut().push(uri));
        }),
        IntSize::new(400, 800),
    );
    let h = rule.host();
    let field = rule.on_node_with_tag("alan").fetch_semantics_node().bounds_in_root();
    let at = Offset::new(field.left + 10.0, field.top + 10.0);
    let types: Vec<String> = vec!["image/png".into(), "text/plain".into()];
    let items = vec![ClipItem::with_uri("content://resim/7"), ClipItem::with_text("yazı")];
    let both = ClipEntry::with_metadata(items, ClipMetadata::of_offered(&types));
    h.external_drag_entered(types);
    h.external_drag_moved(at);
    rule.wait_for_idle();
    rule.on_node_with_text("üstünde").assert_exists();
    h.external_drop(dropped(both), at);
    rule.wait_for_idle();
    assert_eq!(*images.borrow(), ["content://resim/7"]);
    rule.on_node_with_text("metin yazı").assert_exists();
    // HTML pasted: taken by the inner one, nothing pasted.
    let p = platform(&rule);
    *p.clipboard.borrow_mut() = Some(ClipEntry::with_html_text("kalın", "<b>kalın</b>"));
    rule.on_node_with_tag("alan").perform_click();
    rule.wait_for_idle();
    rule.host().key_down(Key::V, KeyModifiers::Ctrl);
    rule.host().key_up(Key::V, KeyModifiers::Ctrl);
    rule.wait_for_idle();
    assert_eq!(*images.borrow(), ["content://resim/7", "html"]);
    rule.on_node_with_text("metin yazı").assert_exists();
}

// ---- ReceiveContentBasicSample's forms ----------------------------------------------------------------------------

fn a_field_takes_pasted_images_through_its_receiver() {
    let images: Rc<RefCell<Vec<(i32, i32)>>> = Rc::default();
    let im = images.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let im = im.clone();
        PasteImages(move |w, h| im.borrow_mut().push((w, h)));
    }));
    let p = platform(&rule);
    rule.on_node_with_tag("alan").perform_click();
    rule.wait_for_idle();
    *p.clipboard.borrow_mut() = Some(ClipEntry::new(vec![ClipItem::with_image(red()), ClipItem::with_text("yazı")]));
    rule.host().key_down(Key::V, KeyModifiers::Ctrl);
    rule.host().key_up(Key::V, KeyModifiers::Ctrl);
    rule.wait_for_idle();
    assert_eq!(*images.borrow(), [(3, 2)]);
    assert_eq!(text_with(&rule, "alan:"), "alan: yazı");
    // Text alone isn't the listener's: given back, pasted.
    *p.clipboard.borrow_mut() = Some(ClipEntry::with_plain_text("!"));
    rule.host().key_down(Key::V, KeyModifiers::Ctrl);
    rule.host().key_up(Key::V, KeyModifiers::Ctrl);
    rule.wait_for_idle();
    assert_eq!(text_with(&rule, "alan:"), "alan: yazı!");
    assert_eq!(images.borrow().len(), 1);
}

fn a_box_takes_dropped_files() {
    let files: Rc<RefCell<Vec<String>>> = Rc::default();
    let f = files.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let f = f.clone();
        DropFiles(move |path| f.borrow_mut().push(path));
    }));
    let h = rule.host();
    assert!(h.external_drag_entered(vec!["text/uri-list".into()]));
    h.external_drag_moved(Offset::new(50.0, 50.0));
    let entry = ClipEntry::with_files(vec!["/tmp/a b.png".into(), "/tmp/c.txt".into()]);
    assert!(h.external_drop(dropped(entry), Offset::new(50.0, 50.0)));
    assert_eq!(*files.borrow(), ["/tmp/a b.png", "/tmp/c.txt"]);
    // Text, no files: given back whole, not taken.
    h.external_drag_entered(vec!["text/plain".into()]);
    assert!(!h.external_drop(dropped(ClipEntry::with_plain_text("x")), Offset::new(50.0, 50.0)));
    assert_eq!(files.borrow().len(), 2);
}

// ---- kompoz_file! -------------------------------------------------------------------------------------------------

kompoz_file!("tests/kt/content.kt");

fn nested_receivers_in_a_file() {
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        NestedReceivers(move |s| l.borrow_mut().push(s));
    }));
    let h = rule.host();
    h.external_drag_entered(vec!["image/png".into(), "text/plain".into()]);
    h.external_drag_moved(Offset::new(50.0, 50.0));
    let entry = ClipEntry::new(vec![
        ClipItem::with_image(red()),
        ClipItem::with_text("yazı"),
        ClipItem::with_uri("https://kompoz.dev"),
    ]);
    assert!(h.external_drop(dropped(entry), Offset::new(50.0, 50.0)));
    assert_eq!(*log.borrow(), ["iç 3 true", "dış 2"]);
}

fn the_clipboard_from_a_coroutine_in_a_file() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| ClipboardButtons()));
    let p = platform(&rule);
    rule.on_node_with_text("kopyala").perform_click();
    rule.wait_for_idle();
    assert_eq!(p.clipboard.borrow().as_ref().and_then(|e| e.read_plain_text()).as_deref(), Some("panodan"));
    *p.clipboard.borrow_mut() = Some(ClipEntry::with_plain_text("başkasının"));
    rule.on_node_with_text("yapıştır").perform_click();
    rule.wait_for_idle();
    assert_eq!(text_with(&rule, "pano:"), "pano: başkasının");
}

tests![
    content_receivers_as_compose_writes_them,
    a_field_takes_pasted_images_through_its_receiver,
    a_box_takes_dropped_files,
    nested_receivers_in_a_file,
    the_clipboard_from_a_coroutine_in_a_file,
];
