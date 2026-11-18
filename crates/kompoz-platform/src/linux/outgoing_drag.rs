//! What a drag of the app's own carries out to other apps, as both Linux display servers offer it (WP5b: a drag
//! within the app that left the window, handed to the platform — Wayland's data source, X11's XDND source): the
//! types it is offered as (files as a `text/uri-list` of percent-encoded `file://` URIs; text as UTF-8 under the
//! names apps ask for it by, and as Latin-1 `STRING` on X11; the MIME types the app gave it), the bytes each is
//! served as, the actions its source allows, and its icon's pixels.

use crate::system::file_uri;
use kompoz_graphics::ImageBitmap;
use kompoz_ui::draganddrop::{DragAndDropTransferAction, DragAndDropTransferData};
use std::path::Path;

/// Text, as the Wayland data source offers it (the clipboard's own types: UTF-8 under each name).
pub const WAYLAND_TEXT_TYPES: &[&str] = &["text/plain;charset=utf-8", "text/plain", "UTF8_STRING"];
/// Text, as the XDND source offers it (its targets): UTF-8 under the MIME types and ICCCM's names, Latin-1 as
/// `STRING`.
pub const X11_TEXT_TYPES: &[&str] = &["text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "STRING", "TEXT"];
/// Files.
pub const URI_LIST: &str = "text/uri-list";

/// What a drag of the app's own carries, as other apps are given it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DragPayload {
    /// Its text (UTF-8).
    pub text: String,
    /// Its files' paths (absolute: a relative one is taken from the app's working directory).
    pub files: Vec<String>,
    /// The MIME types the app said it carries.
    pub mime_types: Vec<String>,
    /// What its source allows, as the app said.
    pub actions: Vec<DragAndDropTransferAction>,
}

impl DragPayload {
    /// What `data` carries.
    pub fn new(data: &DragAndDropTransferData) -> DragPayload {
        let cwd = std::env::current_dir().ok();
        let files = data
            .files
            .iter()
            .map(|f| match &cwd {
                Some(cwd) if !Path::new(f).is_absolute() => cwd.join(f).to_string_lossy().into_owned(),
                _ => f.clone(),
            })
            .collect();
        DragPayload {
            text: data.text.clone(),
            files,
            mime_types: data.mime_types.clone(),
            actions: data.supported_actions.clone(),
        }
    }

    /// Whether it carries text: some, or the app said so (a text/… type).
    fn has_text(&self) -> bool {
        !self.text.is_empty() || self.mime_types.iter().any(|m| m == "text/plain" || m.starts_with("text/plain;"))
    }

    /// Whether it carries files: some, or the app said so (its text is then the list).
    fn has_files(&self) -> bool {
        !self.files.is_empty() || self.mime_types.iter().any(|m| m == URI_LIST)
    }

    /// The types it is offered as, the best first: files, then text (as `text_types` name it), then the app's own
    /// types not among them (served as its text).
    pub fn offered(&self, text_types: &[&str]) -> Vec<String> {
        let mut types: Vec<String> = Vec::new();
        if self.has_files() {
            types.push(URI_LIST.into());
        }
        if self.has_text() {
            types.extend(text_types.iter().map(|t| t.to_string()));
        }
        for m in &self.mime_types {
            if !types.contains(m) {
                types.push(m.clone());
            }
        }
        types
    }

    /// Its files as a `text/uri-list` (RFC 2483): a `file://` URI a line, percent-encoded, each line ended by CRLF;
    /// with no files, its text (the app's own list).
    pub fn uri_list(&self) -> String {
        if self.files.is_empty() {
            return self.text.clone();
        }
        self.files.iter().map(|f| file_uri(f) + "\r\n").collect()
    }

    /// What it is as `mime_type` (a type it is offered as): the uri-list, its text as UTF-8 (Latin-1 for X11's
    /// `STRING`: what Latin-1 can't hold as '?'), or its text for the app's own types; none for a type it isn't
    /// offered as.
    pub fn bytes(&self, mime_type: &str, text_types: &[&str]) -> Option<Vec<u8>> {
        if mime_type == URI_LIST && self.has_files() {
            return Some(self.uri_list().into_bytes());
        }
        if mime_type == "STRING" && self.has_text() && text_types.contains(&"STRING") {
            return Some(self.text.chars().map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')).collect());
        }
        if (self.has_text() && text_types.contains(&mime_type)) || self.mime_types.iter().any(|m| m == mime_type) {
            return Some(self.text.as_bytes().to_vec());
        }
        None
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

    fn payload(text: &str, files: &[&str], mime_types: &[&str]) -> DragPayload {
        DragPayload {
            text: text.into(),
            files: files.iter().map(|f| f.to_string()).collect(),
            mime_types: mime_types.iter().map(|m| m.to_string()).collect(),
            actions: vec![DragAndDropTransferAction::Copy, DragAndDropTransferAction::Move],
        }
    }

    #[test]
    fn files_are_offered_first_as_a_percent_encoded_uri_list() {
        let p = payload("", &["/home/kasim/a b.png", "/tmp/çay%.txt"], &["text/uri-list"]);
        assert_eq!(p.offered(WAYLAND_TEXT_TYPES), ["text/uri-list"]);
        let list = "file:///home/kasim/a%20b.png\r\nfile:///tmp/%C3%A7ay%25.txt\r\n";
        assert_eq!(p.uri_list(), list);
        assert_eq!(p.bytes("text/uri-list", WAYLAND_TEXT_TYPES).unwrap(), list.as_bytes());
        assert_eq!(p.bytes("text/plain", WAYLAND_TEXT_TYPES), None); // (no text)
        // (Back as the receiving side reads it.)
        assert_eq!(crate::linux::desktop::files_in_uri_list(list), ["/home/kasim/a b.png", "/tmp/çay%.txt"]);
    }

    #[test]
    fn relative_paths_are_the_working_directorys() {
        let data = DragAndDropTransferData {
            files: vec!["notes.txt".into(), "/etc/hosts".into()],
            mime_types: vec!["text/uri-list".into()],
            ..Default::default()
        };
        let p = DragPayload::new(&data);
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(p.files, [cwd.join("notes.txt").to_string_lossy().into_owned(), "/etc/hosts".into()]);
        assert_eq!(p.actions.len(), 3); // (all three by default)
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
            assert_eq!(p.bytes(t, WAYLAND_TEXT_TYPES).unwrap(), "Merhaba dünya ğ".as_bytes());
        }
        assert_eq!(p.bytes("STRING", X11_TEXT_TYPES).unwrap(), b"Merhaba d\xfcnya ?");
        assert_eq!(p.bytes("STRING", WAYLAND_TEXT_TYPES), None); // (not offered on Wayland)
        assert_eq!(p.bytes("image/png", X11_TEXT_TYPES), None);
        // Text and files: both, files first; the uri-list from the files, the text as itself.
        let p = payload("iki dosya", &["/a", "/b"], &["text/uri-list", "text/plain"]);
        assert_eq!(
            p.offered(WAYLAND_TEXT_TYPES)[..2],
            ["text/uri-list".to_string(), "text/plain;charset=utf-8".into()]
        );
        assert_eq!(p.bytes("text/uri-list", WAYLAND_TEXT_TYPES).unwrap(), b"file:///a\r\nfile:///b\r\n");
        assert_eq!(p.bytes("text/plain", WAYLAND_TEXT_TYPES).unwrap(), b"iki dosya");
    }

    #[test]
    fn the_apps_own_types_are_offered_as_its_text() {
        let p = payload("elma", &[], &["application/x-kompoz-fruit"]);
        assert_eq!(
            p.offered(WAYLAND_TEXT_TYPES),
            ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING", "application/x-kompoz-fruit"]
        );
        assert_eq!(p.bytes("application/x-kompoz-fruit", WAYLAND_TEXT_TYPES).unwrap(), b"elma");
        // Nothing but its own type (and its local state): that type alone, its text empty.
        let p = payload("", &[], &["application/x-kompoz-fruit"]);
        assert_eq!(p.offered(X11_TEXT_TYPES), ["application/x-kompoz-fruit"]);
        assert_eq!(p.bytes("application/x-kompoz-fruit", X11_TEXT_TYPES).unwrap(), b"");
        // A uri-list it gave as text (no paths).
        let p = payload("https://kompoz.dev/\r\n", &[], &["text/uri-list"]);
        assert_eq!(p.bytes("text/uri-list", X11_TEXT_TYPES).unwrap(), b"https://kompoz.dev/\r\n");
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
