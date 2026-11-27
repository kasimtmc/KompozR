//! The Windows clipboard (C++'s readClipEntry/writeClipboard, made lazy both ways as Linux's selections are): what
//! other apps put there read only as the app asks for each type (the clipboard's sequence number telling whether it is
//! still what was offered), and what the app puts there promised format by format (delayed rendering:
//! `SetClipboardData(format, NULL)`, the bytes made when another app asks, WM_RENDERFORMAT; all of them as the app
//! quits, WM_RENDERALLFORMATS), from the types [`Payload`] offers. The formats and their conversions are drag and
//! drop's too ([`formats`](super::formats)).

use super::formats::{self, Formats, Medium, Source};
use super::sys::{self, Hwnd, OpenClipboardGuard};
use crate::transfer::Payload;
use kompoz_ui::clipboard::{ClipDataReader, ClipEntry, ClipItem, ClipMetadata};
use std::cell::RefCell;
use std::rc::Rc;

/// The system's part of the formats: the session's registered formats, the ANSI code page.
pub struct SystemFormats;

impl formats::System for SystemFormats {
    fn register(&self, name: &str) -> u32 {
        sys::register_clipboard_format(name)
    }
    fn name(&self, format: u32) -> Option<String> {
        sys::clipboard_format_name(format)
    }
    fn ansi(&self, text: &str) -> Vec<u8> {
        sys::ansi_of(text)
    }
    fn text_of_ansi(&self, bytes: &[u8]) -> String {
        sys::text_of_ansi(bytes)
    }
}

/// The open clipboard as another app's data.
struct Opened<'a>(&'a OpenClipboardGuard);

impl Source for Opened<'_> {
    fn has(&self, format: u32) -> bool {
        sys::clipboard_has(format)
    }
    fn get(&self, format: u32) -> Option<Vec<u8>> {
        self.0.get(format)
    }
}

/// What the clipboard holds (all of it, by its formats), read as each type is asked for while it holds the same
/// (its sequence number); none if it holds nothing the app can read. `owner`: the window opening it.
pub fn clip_entry(formats: Formats, owner: Hwnd) -> Option<ClipEntry> {
    let sequence = sys::clipboard_sequence();
    let types = {
        let clipboard = OpenClipboardGuard::open(owner)?;
        formats.mime_types(&clipboard.formats(), &SystemFormats)
    };
    let metadata = ClipMetadata::of_offered(&types);
    let data: Vec<String> = metadata.mime_types().to_vec();
    if data.is_empty() {
        return None;
    }
    let reader = ClipboardReader { formats, owner, sequence };
    Some(ClipEntry::with_metadata(vec![ClipItem::from_reader(data, Rc::new(reader))], metadata))
}

/// Reads a type of what the clipboard held when the entry was made (none once it holds something else).
struct ClipboardReader {
    formats: Formats,
    owner: Hwnd,
    sequence: u32,
}

impl ClipDataReader for ClipboardReader {
    fn read(&self, mime_type: &str) -> Option<Vec<u8>> {
        if sys::clipboard_sequence() != self.sequence {
            return None;
        }
        let clipboard = OpenClipboardGuard::open(self.owner)?;
        self.formats.read(&Opened(&clipboard), mime_type, &SystemFormats)
    }
}

/// What the app put on the clipboard, promised format by format, rendered as other apps ask (the helper window's:
/// it owns the clipboard).
pub struct Owned {
    payload: Payload,
    /// Each promised format and the type it is made from.
    promised: Vec<(u32, String)>,
    /// The clipboard's sequence number once it held this.
    sequence: u32,
}

/// The clipboard's side the app keeps: its formats, what it put there (while it owns it).
pub struct Clipboard {
    pub formats: Formats,
    owned: RefCell<Option<Owned>>,
}

impl Clipboard {
    pub fn new() -> Clipboard {
        Clipboard { formats: Formats::register(&SystemFormats), owned: RefCell::new(None) }
    }

    /// What it holds: the app's own entry where it put it (it owns it still), else another app's (read lazily).
    pub fn entry(&self, owner: Hwnd) -> Option<ClipEntry> {
        if sys::clipboard_owner() == owner
            && let Some(own) = self.owned.borrow().as_ref()
        {
            return Some(own.payload.entry().clone());
        }
        clip_entry(self.formats, owner)
    }

    /// `entry` put on it by `owner` (the helper window), its formats promised (the sensitive markers given at once:
    /// they are small); none: emptied.
    pub fn set(&self, owner: Hwnd, entry: Option<ClipEntry>) {
        let promised = {
            let Some(clipboard) = OpenClipboardGuard::open(owner) else {
                crate::system::log("kompoz: the clipboard is held by another app");
                return;
            };
            // (Emptying makes WM_DESTROYCLIPBOARD come to the owner before: what it held goes.)
            clipboard.empty();
            *self.owned.borrow_mut() = None;
            let Some(entry) = entry else { return };
            let payload = Payload::of_entry(&entry);
            let promised = self.promise(&clipboard, &payload);
            (payload, promised)
        };
        let (payload, promised) = promised;
        // (Its sequence number once closed: a WM_DESTROYCLIPBOARD of the emptying that comes after, as wine sends it
        // from its clipboard's own thread, is told from another app's taking it.)
        *self.owned.borrow_mut() = Some(Owned { payload, promised, sequence: sys::clipboard_sequence() });
    }

    /// The formats `payload` is offered as, promised on the open clipboard (the sensitive markers given at once:
    /// they are small); each with the type it is made from.
    fn promise(&self, clipboard: &OpenClipboardGuard, payload: &Payload) -> Vec<(u32, String)> {
        let offered = self.formats.offered(payload, Medium::Clipboard, &SystemFormats);
        for (format, bytes) in &offered.given {
            clipboard.set(*format, bytes);
        }
        offered.promised.into_iter().filter(|(format, _)| clipboard.promise(*format)).collect()
    }

    /// WM_RENDERFORMAT: `format`'s bytes made from what the app put there, given to the clipboard (open by the app
    /// asking for it).
    pub fn render(&self, format: u32) {
        let bytes = self.owned.borrow().as_ref().and_then(|o| self.bytes(o, format));
        if let Some(bytes) = bytes {
            sys::set_clipboard_data(format, Some(&bytes));
        }
    }

    /// WM_RENDERALLFORMATS (the owner going: the app quits): every promised format made, if the app still owns the
    /// clipboard.
    pub fn render_all(&self, owner: Hwnd) {
        if self.owned.borrow().is_none() {
            return;
        }
        let Some(clipboard) = OpenClipboardGuard::open(owner) else { return };
        if sys::clipboard_owner() != owner {
            return;
        }
        let owned = self.owned.borrow();
        let Some(o) = owned.as_ref() else { return };
        for (format, _) in &o.promised {
            if let Some(bytes) = self.bytes(o, *format) {
                clipboard.set(*format, &bytes);
            }
        }
    }

    /// WM_DESTROYCLIPBOARD: another app (or the app, emptying it) took the clipboard: what it held goes — unless
    /// `owner` still owns what it put there (the message of an emptying before, come late).
    pub fn lost(&self, owner: Hwnd) {
        let mut owned = self.owned.borrow_mut();
        let still =
            owned.as_ref().is_some_and(|o| sys::clipboard_owner() == owner && sys::clipboard_sequence() == o.sequence);
        if !still {
            *owned = None;
        }
    }

    fn bytes(&self, owned: &Owned, format: u32) -> Option<Vec<u8>> {
        let (_, mime_type) = owned.promised.iter().find(|(f, _)| *f == format)?;
        self.formats.render(&owned.payload, format, mime_type, &SystemFormats)
    }
}
