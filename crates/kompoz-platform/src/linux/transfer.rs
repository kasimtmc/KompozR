//! What the app offers other apps, as both Linux display servers offer it: its clipboard (a Wayland data source, the
//! X11 CLIPBOARD selection) and its drags out of the window (WP5b: a drag within the app that left the window, handed
//! to the platform — Wayland's data source, X11's XDND source). From a [`ClipEntry`]: the types it is offered as
//! (files and URIs as a `text/uri-list` of percent-encoded URIs; text as UTF-8 under the names apps ask for it by,
//! and as Latin-1 `STRING` on X11; HTML; an image as PNG; every other type the app gave as it is), the bytes each is
//! served as (an image encoded once, when first asked for), the actions a drag's source allows, and a drag icon's
//! pixels.

use kompoz_graphics::ImageBitmap;
use kompoz_ui::clipboard::{
    ClipEntry, ClipValue, MIMETYPE_IMAGE_PNG, MIMETYPE_TEXT_HTML, MIMETYPE_TEXT_URILIST, encode_text,
    is_plain_text_type,
};
use kompoz_ui::draganddrop::{DragAndDropTransferAction, DragAndDropTransferData};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

/// Text, as the Wayland data source offers it (the clipboard's own types: UTF-8 under each name).
pub const WAYLAND_TEXT_TYPES: &[&str] = &["text/plain;charset=utf-8", "text/plain", "UTF8_STRING"];
/// Text, as X11 offers it (its targets): UTF-8 under the MIME types and ICCCM's names, Latin-1 as `STRING`.
pub const X11_TEXT_TYPES: &[&str] = &["text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "STRING", "TEXT"];
/// Files and URIs.
pub const URI_LIST: &str = MIMETYPE_TEXT_URILIST;

/// Where a type's bytes come from.
#[derive(Clone, Debug)]
enum Source {
    /// Every item's URIs, a line each.
    UriList,
    /// The items' text, a line between each.
    Text,
    /// The first item's HTML that has some.
    Html,
    /// The first image an item holds, as PNG.
    Image,
    /// An item's own type: its bytes as they are.
    Item(usize, String),
}

/// What the app offers (its clipboard, or what a drag of its own carries), as other apps are given it.
#[derive(Default)]
pub struct Payload {
    entry: ClipEntry,
    /// The types it is offered as, the best first (as named for X11 or Wayland: `offered`).
    sources: Vec<(String, Source)>,
    /// What a drag's source allows, as the app said.
    pub actions: Vec<DragAndDropTransferAction>,
    /// The bytes of the types encoded (an image), kept.
    encoded: RefCell<Vec<(String, Rc<[u8]>)>>,
}

impl fmt::Debug for Payload {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Payload").field("sources", &self.sources).field("actions", &self.actions).finish()
    }
}

impl Payload {
    /// What a drag of `data` carries.
    pub fn new(data: &DragAndDropTransferData) -> Payload {
        Payload { actions: data.supported_actions.clone(), ..Payload::of_entry(&data.clip_entry) }
    }

    /// What `entry` holds (another app's read now: it may go once the app offers it).
    pub fn of_entry(entry: &ClipEntry) -> Payload {
        let entry = entry.materialized();
        let mut sources: Vec<(String, Source)> = Vec::new();
        if !entry.uris().is_empty() {
            sources.push((URI_LIST.into(), Source::UriList));
        }
        if entry.read_plain_text().is_some() {
            sources.push(("text".into(), Source::Text)); // (named as the platform names text: `offered`)
        }
        if entry.items().iter().any(|i| i.html_text().is_some()) {
            sources.push((MIMETYPE_TEXT_HTML.into(), Source::Html));
        }
        let has_image = entry
            .items()
            .iter()
            .any(|i| i.mime_types().iter().any(|m| matches!(i.value(m), Some(ClipValue::Image(_)))));
        if has_image {
            sources.push((MIMETYPE_IMAGE_PNG.into(), Source::Image));
        }
        for (index, item) in entry.items().iter().enumerate() {
            for m in item.mime_types() {
                let covered = is_plain_text_type(&m)
                    || m.eq_ignore_ascii_case(URI_LIST)
                    || m.eq_ignore_ascii_case(MIMETYPE_TEXT_HTML)
                    || (has_image && m.eq_ignore_ascii_case(MIMETYPE_IMAGE_PNG));
                if !covered && !sources.iter().any(|(n, _)| *n == m) {
                    sources.push((m.clone(), Source::Item(index, m)));
                }
            }
        }
        Payload { entry, sources, actions: Vec::new(), encoded: RefCell::default() }
    }

    /// What it holds.
    pub fn entry(&self) -> &ClipEntry {
        &self.entry
    }

    /// The types it is offered as, the best first: files and URIs, text (as `text_types` name it), HTML, an image,
    /// then the app's own types.
    pub fn offered(&self, text_types: &[&str]) -> Vec<String> {
        let mut types = Vec::new();
        for (name, source) in &self.sources {
            match source {
                Source::Text => types.extend(text_types.iter().map(|t| t.to_string())),
                _ => types.push(name.clone()),
            }
        }
        types
    }

    /// Where `mime_type`'s bytes come from (a type it is offered as, with `text_types` its text's names).
    fn source(&self, mime_type: &str, text_types: &[&str]) -> Option<&Source> {
        if text_types.contains(&mime_type) {
            return self.sources.iter().find(|(_, s)| matches!(s, Source::Text)).map(|(_, s)| s);
        }
        self.sources.iter().find(|(n, s)| n == mime_type && !matches!(s, Source::Text)).map(|(_, s)| s)
    }

    /// What it is as `mime_type` (a type it is offered as): the uri-list (a URI a line, each ended by CRLF: RFC
    /// 2483), its text as UTF-8 (Latin-1 for X11's `STRING`: what Latin-1 can't hold as '?'), its HTML as UTF-8, its
    /// image as PNG, an item's own type as it is; none for a type it isn't offered as.
    pub fn bytes(&self, mime_type: &str, text_types: &[&str]) -> Option<Rc<[u8]>> {
        if let Some((_, b)) = self.encoded.borrow().iter().find(|(m, _)| m == mime_type) {
            return Some(b.clone());
        }
        let bytes: Rc<[u8]> = match self.source(mime_type, text_types)? {
            Source::UriList => {
                self.entry.uris().iter().map(|u| u.clone() + "\r\n").collect::<String>().into_bytes().into()
            }
            Source::Text => encode_text(&self.entry.read_plain_text().unwrap_or_default(), mime_type).into(),
            Source::Html => {
                let html: Vec<String> = self.entry.items().iter().filter_map(|i| i.html_text()).collect();
                html.join("\n").into_bytes().into()
            }
            Source::Image => {
                let item = self
                    .entry
                    .items()
                    .iter()
                    .find(|i| i.mime_types().iter().any(|m| matches!(i.value(m), Some(ClipValue::Image(_)))))?;
                let png = item.mime_types().into_iter().find(|m| matches!(item.value(m), Some(ClipValue::Image(_))))?;
                let bytes = item.bytes(&png)?;
                self.encoded.borrow_mut().push((mime_type.to_string(), bytes.clone()));
                bytes
            }
            Source::Item(index, name) => self.entry.items().get(*index)?.bytes(name)?,
        };
        Some(bytes)
    }

    /// The action a drop takes of those its source allows when the user asks for none in particular (AWT's user
    /// action, as within the app: Move, Ctrl Copy, Ctrl+Shift Link; else the first allowed).
    pub fn user_action(&self, ctrl: bool, shift: bool) -> Option<DragAndDropTransferAction> {
        let wanted = match (ctrl, shift) {
            (true, true) => DragAndDropTransferAction::Link,
            (true, false) => DragAndDropTransferAction::Copy,
            _ => DragAndDropTransferAction::Move,
        };
        if self.actions.contains(&wanted) { Some(wanted) } else { self.actions.first().copied() }
    }
}

/// An icon's pixels (an image of straight RGBA) as premultiplied ARGB words (what `wl_shm`'s ARGB8888 and an X
/// window of depth 32 take), `width` × `height` of them, the image at their top left (rows and columns past it
/// transparent).
pub fn premultiplied_argb(image: &ImageBitmap, width: i32, height: i32) -> Vec<u32> {
    let (iw, ih) = (image.width().max(0) as usize, image.height().max(0) as usize);
    let (w, h) = (width.max(0) as usize, height.max(0) as usize);
    let pixels = image.pixels();
    let mut out = vec![0u32; w * h];
    for y in 0..h.min(ih) {
        for x in 0..w.min(iw) {
            let p = &pixels[(y * iw + x) * 4..][..4];
            let a = u32::from(p[3]);
            let c = |v: u8| (u32::from(v) * a + 127) / 255;
            out[y * w + x] = (a << 24) | (c(p[0]) << 16) | (c(p[1]) << 8) | c(p[2]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kompoz_graphics::decode_to_image_bitmap;
    use kompoz_ui::clipboard::{ClipDataReader, ClipItem, files_in_uri_list};
    use kompoz_ui::draganddrop::drag_and_drop_transfer_data;

    fn payload(text: &str, files: &[&str], mime_types: &[&str]) -> Payload {
        let data = kompoz_runtime::named_args!(drag_and_drop_transfer_data(
            text = text.to_string(),
            files = files.iter().map(|f| f.to_string()).collect(),
            mime_types = mime_types.iter().map(|m| m.to_string()).collect(),
            supported_actions = vec![DragAndDropTransferAction::Copy, DragAndDropTransferAction::Move],
        ));
        Payload::new(&data)
    }

    fn bytes(p: &Payload, mime: &str, text_types: &[&str]) -> Option<Vec<u8>> {
        p.bytes(mime, text_types).map(|b| b.to_vec())
    }

    #[test]
    fn files_are_offered_first_as_a_percent_encoded_uri_list() {
        let p = payload("", &["/home/kasim/a b.png", "/tmp/çay%.txt"], &["text/uri-list"]);
        assert_eq!(p.offered(WAYLAND_TEXT_TYPES), ["text/uri-list"]);
        let list = "file:///home/kasim/a%20b.png\r\nfile:///tmp/%C3%A7ay%25.txt\r\n";
        assert_eq!(bytes(&p, "text/uri-list", WAYLAND_TEXT_TYPES).unwrap(), list.as_bytes());
        assert_eq!(bytes(&p, "text/plain", WAYLAND_TEXT_TYPES), None); // (no text)
        // (Back as the receiving side reads it.)
        assert_eq!(files_in_uri_list(list), ["/home/kasim/a b.png", "/tmp/çay%.txt"]);
    }

    #[test]
    fn relative_paths_are_the_working_directorys() {
        let p = payload("", &["notes.txt", "/etc/hosts"], &["text/uri-list"]);
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(p.entry().files(), [cwd.join("notes.txt").to_string_lossy().into_owned(), "/etc/hosts".into()]);
        let data = DragAndDropTransferData::default();
        assert_eq!(Payload::new(&data).actions.len(), 3); // (all three by default)
    }

    #[test]
    fn text_is_served_under_each_name_and_latin1_as_string() {
        let p = payload("Merhaba dünya ğ", &[], &["text/plain"]);
        assert_eq!(p.offered(WAYLAND_TEXT_TYPES), ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING"]);
        assert_eq!(
            p.offered(X11_TEXT_TYPES),
            ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "STRING", "TEXT"]
        );
        for t in WAYLAND_TEXT_TYPES {
            assert_eq!(bytes(&p, t, WAYLAND_TEXT_TYPES).unwrap(), "Merhaba dünya ğ".as_bytes());
        }
        assert_eq!(bytes(&p, "STRING", X11_TEXT_TYPES).unwrap(), b"Merhaba d\xfcnya ?");
        assert_eq!(bytes(&p, "STRING", WAYLAND_TEXT_TYPES), None); // (not offered on Wayland)
        assert_eq!(bytes(&p, "image/png", X11_TEXT_TYPES), None);
        // Text and files: both, files first; the uri-list from the files, the text as itself.
        let p = payload("iki dosya", &["/a", "/b"], &["text/uri-list", "text/plain"]);
        assert_eq!(
            p.offered(WAYLAND_TEXT_TYPES)[..2],
            ["text/uri-list".to_string(), "text/plain;charset=utf-8".into()]
        );
        assert_eq!(bytes(&p, "text/uri-list", WAYLAND_TEXT_TYPES).unwrap(), b"file:///a\r\nfile:///b\r\n");
        assert_eq!(bytes(&p, "text/plain", WAYLAND_TEXT_TYPES).unwrap(), b"iki dosya");
    }

    #[test]
    fn the_apps_own_types_are_offered_as_its_text() {
        let p = payload("elma", &[], &["application/x-kompoz-fruit"]);
        assert_eq!(
            p.offered(WAYLAND_TEXT_TYPES),
            ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING", "application/x-kompoz-fruit"]
        );
        assert_eq!(bytes(&p, "application/x-kompoz-fruit", WAYLAND_TEXT_TYPES).unwrap(), b"elma");
        // Nothing but its own type (and its local state): that type alone, its text empty.
        let p = payload("", &[], &["application/x-kompoz-fruit"]);
        assert_eq!(p.offered(X11_TEXT_TYPES), ["application/x-kompoz-fruit"]);
        assert_eq!(bytes(&p, "application/x-kompoz-fruit", X11_TEXT_TYPES).unwrap(), b"");
        // A uri-list it gave as text (no paths).
        let p = payload("https://kompoz.dev/\r\n", &[], &["text/uri-list"]);
        assert_eq!(bytes(&p, "text/uri-list", X11_TEXT_TYPES).unwrap(), b"https://kompoz.dev/\r\n");
    }

    #[test]
    fn html_images_and_bytes_are_offered_as_their_types() {
        let image = ImageBitmap::from_pixels(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 128]).unwrap();
        let entry = ClipEntry::new(vec![
            ClipItem::with_html_text("kalın", "<b>kalın</b>"),
            ClipItem::with_image(image),
            ClipItem::of(vec![("application/x-kompoz".into(), ClipValue::Bytes(Rc::from(&[1u8, 2, 3][..])))]),
        ]);
        let p = Payload::of_entry(&entry);
        assert_eq!(
            p.offered(WAYLAND_TEXT_TYPES),
            ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING", "text/html", "image/png", "application/x-kompoz"]
        );
        assert_eq!(bytes(&p, "text/html", WAYLAND_TEXT_TYPES).unwrap(), "<b>kalın</b>".as_bytes());
        assert_eq!(bytes(&p, "text/plain", WAYLAND_TEXT_TYPES).unwrap(), "kalın".as_bytes());
        let png = p.bytes("image/png", WAYLAND_TEXT_TYPES).unwrap();
        assert!(Rc::ptr_eq(&png, &p.bytes("image/png", WAYLAND_TEXT_TYPES).unwrap())); // (encoded once)
        let back = decode_to_image_bitmap(&png).unwrap();
        assert_eq!(back.pixels().as_slice(), [255, 0, 0, 255, 0, 0, 255, 128]);
        assert_eq!(bytes(&p, "application/x-kompoz", WAYLAND_TEXT_TYPES).unwrap(), [1, 2, 3]);
        assert_eq!(bytes(&p, "image/jpeg", WAYLAND_TEXT_TYPES), None);
    }

    #[test]
    fn another_apps_entry_is_read_before_it_is_offered() {
        struct Reader;
        impl ClipDataReader for Reader {
            fn read(&self, mime_type: &str) -> Option<Vec<u8>> {
                (mime_type == "UTF8_STRING").then(|| "başka".as_bytes().to_vec())
            }
        }
        let entry = ClipEntry::new(vec![ClipItem::from_reader(vec!["UTF8_STRING".into()], Rc::new(Reader))]);
        let p = Payload::of_entry(&entry);
        assert!(!p.entry().items()[0].is_read_on_request());
        assert_eq!(bytes(&p, "STRING", X11_TEXT_TYPES).unwrap(), b"ba?ka");
    }

    #[test]
    fn the_user_action_is_awts_among_those_allowed() {
        use DragAndDropTransferAction as A;
        let mut p = payload("", &[], &[]);
        p.actions = vec![A::Copy, A::Move, A::Link];
        assert_eq!(p.user_action(false, false), Some(A::Move));
        assert_eq!(p.user_action(true, false), Some(A::Copy));
        assert_eq!(p.user_action(true, true), Some(A::Link));
        assert_eq!(p.user_action(false, true), Some(A::Move));
        p.actions = vec![A::Copy];
        assert_eq!(p.user_action(false, false), Some(A::Copy));
        p.actions.clear();
        assert_eq!(p.user_action(false, false), None);
    }

    #[test]
    fn icons_are_premultiplied_argb() {
        let image = ImageBitmap::from_pixels(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 128]).unwrap();
        assert_eq!(premultiplied_argb(&image, 3, 2), [0xffff0000, 0x80008000, 0, 0, 0, 0]);
        assert_eq!(premultiplied_argb(&image, 1, 1), [0xffff0000]);
    }
}
