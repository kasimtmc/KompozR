//! The pasteboard (C++'s readClipEntry/writeClipboard, made lazy both ways as Linux's selections are): what other
//! apps put on the general pasteboard read only as the app asks for each type (its change count telling whether it
//! still holds what was offered), and what the app puts there promised type by type (a pasteboard item's data
//! provider: the bytes made when another app asks; all of them as the app quits), from the types [`Payload`] offers
//! (`types`: the pasteboard's names). The same items carry a drag of the app's own out of the window, and the same
//! reading reads what a drop brings (at once: a drag's pasteboard is the source's only while it drops).

#![allow(unsafe_code)]

use super::services;
use super::types::{self, CONCEALED, FILE_URL, LEGACY_STRING, PNG, STRING, TEXT_TYPES, TIFF, URL};
use crate::transfer::Payload;
use kompoz_ui::clipboard::{
    ClipDataReader, ClipEntry, ClipItem, ClipMetadata, MIMETYPE_TEXT_URILIST, PASSWORD_MANAGER_HINT, file_uri,
    is_plain_text_type,
};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, ProtocolObject};
use objc2::{AnyThread, DefinedClass, Message, define_class, msg_send};
use objc2_app_kit::{
    NSPasteboard, NSPasteboardItem, NSPasteboardItemDataProvider, NSPasteboardType, NSPasteboardWriting,
};
use objc2_foundation::{NSArray, NSData, NSObjectProtocol, NSString, NSURL};
use std::cell::RefCell;
use std::rc::Rc;

/// The provider's state: what it serves.
pub struct ProviderIvars {
    payload: Rc<Payload>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; the provider implements no Drop.
    #[unsafe(super(NSObject))]
    #[name = "KompozPasteboardProvider"]
    #[ivars = ProviderIvars]
    pub struct Provider;

    // SAFETY: NSObject's protocol, nothing of its own.
    unsafe impl NSObjectProtocol for Provider {}

    // SAFETY: the method's signature is NSPasteboardItemDataProvider's.
    unsafe impl NSPasteboardItemDataProvider for Provider {
        /// Another app (or AppKit as the app quits) asks for a type the app promised: its bytes made now.
        #[unsafe(method(pasteboard:item:provideDataForType:))]
        fn provide(&self, _pasteboard: Option<&NSPasteboard>, item: &NSPasteboardItem, kind: &NSPasteboardType) {
            if let Some(bytes) = bytes_for(&self.ivars().payload, &kind.to_string()) {
                item.setData_forType(&NSData::with_bytes(&bytes), kind);
            }
        }
    }
);

// (The provider is made and used on the main thread only, as AppKit calls it; its ivars aren't Send, so the class
// isn't either: objc2 gives it no Send or Sync.)

impl Provider {
    fn new(payload: Rc<Payload>) -> Retained<Provider> {
        let this = Provider::alloc().set_ivars(ProviderIvars { payload });
        // SAFETY: NSObject's `init` on a freshly allocated object with its ivars set.
        unsafe { msg_send![super(this), init] }
    }
}

/// The bytes of pasteboard type `kind` from what the app offers (TIFF made from the PNG).
fn bytes_for(payload: &Payload, kind: &str) -> Option<Vec<u8>> {
    let offered = types::offered_for(kind);
    let bytes = payload.bytes(offered, TEXT_TYPES)?;
    if kind == TIFF { services::tiff_of(&bytes) } else { Some(bytes.to_vec()) }
}

/// What the app offers, as pasteboard items: the first promising every type through `provider` (text, HTML, an
/// image as PNG and TIFF, the app's own types, the URI list by its name), with the first file's URL (or a URL that
/// isn't a file's) and the concealed marker given at once; each other file an item of its own.
pub struct Items {
    pub items: Vec<Retained<NSPasteboardItem>>,
    /// (Kept while the items are offered: it answers for them.)
    _provider: Retained<Provider>,
}

/// The pasteboard items for `payload`: its types promised (`lazy`), or given at once.
pub fn items(payload: &Rc<Payload>, lazy: bool) -> Items {
    let provider = Provider::new(payload.clone());
    let first = NSPasteboardItem::new();
    let mut items = Vec::new();
    let mut promised: Vec<String> = Vec::new();
    for offered in payload.offered(TEXT_TYPES) {
        if offered == PASSWORD_MANAGER_HINT {
            first.setString_forType(&NSString::from_str(""), &NSString::from_str(CONCEALED));
            continue;
        }
        for kind in types::offered_as(&offered) {
            if !promised.iter().any(|p| p == kind) {
                promised.push(kind.to_string());
            }
        }
    }
    let uris = payload.entry().uris();
    let files = payload.entry().files();
    if !files.is_empty() {
        for (i, path) in files.iter().enumerate() {
            let item = if i == 0 { first.clone() } else { NSPasteboardItem::new() };
            item.setString_forType(&NSString::from_str(&file_uri(path)), &NSString::from_str(FILE_URL));
            if i > 0 {
                items.push(item);
            }
        }
    } else if let [uri] = uris.as_slice() {
        first.setString_forType(&NSString::from_str(uri), &NSString::from_str(URL));
    }
    if !lazy {
        for kind in &promised {
            if let Some(bytes) = bytes_for(payload, kind) {
                first.setData_forType(&NSData::with_bytes(&bytes), &NSString::from_str(kind));
            }
        }
    } else if !promised.is_empty() {
        let kinds: Vec<Retained<NSString>> = promised.iter().map(|p| NSString::from_str(p)).collect();
        first.setDataProvider_forTypes(ProtocolObject::from_ref(&*provider), &NSArray::from_retained_slice(&kinds));
    }
    items.insert(0, first);
    Items { items, _provider: provider }
}

/// The items as what a pasteboard writes.
pub fn writers(items: &[Retained<NSPasteboardItem>]) -> Retained<NSArray<ProtocolObject<dyn NSPasteboardWriting>>> {
    let writers: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> =
        items.iter().map(|i| ProtocolObject::from_retained(i.clone())).collect();
    NSArray::from_retained_slice(&writers)
}

/// What `pasteboard` holds (all of it, by its items' types), read as each type is asked for while it holds the same
/// (its change count); none if it holds nothing the app can read.
pub fn entry_of(pasteboard: &NSPasteboard) -> Option<ClipEntry> {
    let items = pasteboard.pasteboardItems()?;
    let mut kinds: Vec<String> = Vec::new();
    for item in items.iter() {
        for kind in item.types().iter() {
            let kind = kind.to_string();
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
        }
    }
    let metadata = ClipMetadata::of_offered(&types::mime_types(&kinds));
    let data: Vec<String> = metadata.mime_types().to_vec();
    if data.is_empty() {
        return None;
    }
    let reader = PasteboardReader { pasteboard: pasteboard.retain(), change_count: pasteboard.changeCount() };
    Some(ClipEntry::with_metadata(vec![ClipItem::from_reader(data, Rc::new(reader))], metadata))
}

/// Reads a type of what the pasteboard held when the entry was made (none once it holds something else).
struct PasteboardReader {
    pasteboard: Retained<NSPasteboard>,
    change_count: isize,
}

impl ClipDataReader for PasteboardReader {
    fn read(&self, mime_type: &str) -> Option<Vec<u8>> {
        if self.pasteboard.changeCount() != self.change_count {
            return None;
        }
        let mime_type = if is_plain_text_type(mime_type) { "text/plain" } else { mime_type };
        types::sources(mime_type).into_iter().find_map(|kind| self.read_type(kind))
    }
}

impl PasteboardReader {
    /// Pasteboard type `kind` as the MIME type it is read as: URLs (every item's) as a URI list, text as UTF-8,
    /// TIFF as PNG, the rest as it is.
    fn read_type(&self, kind: &str) -> Option<Vec<u8>> {
        let name = NSString::from_str(kind);
        match kind {
            FILE_URL | URL => {
                let mut uris = Vec::new();
                for item in self.pasteboard.pasteboardItems()?.iter() {
                    let Some(s) = item.stringForType(&name) else { continue };
                    // (A file reference URL, file:///.file/id=…, as the file's path.)
                    let uri = NSURL::URLWithString(&s)
                        .and_then(|u| if kind == FILE_URL { u.filePathURL() } else { Some(u) })
                        .and_then(|u| u.absoluteString())
                        .map_or_else(|| s.to_string(), |a| a.to_string());
                    uris.push(uri);
                }
                (!uris.is_empty()).then(|| uris.iter().map(|u| format!("{u}\r\n")).collect::<String>().into_bytes())
            }
            STRING | LEGACY_STRING => self.pasteboard.stringForType(&name).map(|s| s.to_string().into_bytes()),
            TIFF => services::png_of(&self.pasteboard.dataForType(&name)?.to_vec()),
            PNG => self.pasteboard.dataForType(&name).map(|d| d.to_vec()),
            CONCEALED => {
                self.pasteboard.availableTypeFromArray(&NSArray::from_retained_slice(&[name])).map(|_| Vec::new())
            }
            MIMETYPE_TEXT_URILIST => self.pasteboard.dataForType(&name).map(|d| d.to_vec()),
            _ => self.pasteboard.dataForType(&name).map(|d| d.to_vec()),
        }
    }
}

/// What the app put on the general pasteboard: its items' provider and the types promised, while the pasteboard holds
/// them (its change count since).
struct Owned {
    payload: Rc<Payload>,
    /// (Kept: its provider answers while the pasteboard holds them.)
    _items: Items,
    change_count: isize,
}

/// The general pasteboard's side the app keeps: what it put there.
pub struct Clipboard {
    owned: RefCell<Option<Owned>>,
}

impl Clipboard {
    pub fn new() -> Clipboard {
        Clipboard { owned: RefCell::new(None) }
    }

    /// What it holds: the app's own entry where it put it (and nothing since), else another app's (read lazily).
    pub fn entry(&self) -> Option<ClipEntry> {
        let pasteboard = NSPasteboard::generalPasteboard();
        if let Some(own) = self.owned.borrow().as_ref()
            && own.change_count == pasteboard.changeCount()
        {
            return Some(own.payload.entry().clone());
        }
        entry_of(&pasteboard)
    }

    /// `entry` put on it, its types promised (the files' URLs and the sensitive marker given at once: they are
    /// small); none: emptied.
    pub fn set(&self, entry: Option<ClipEntry>) {
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        *self.owned.borrow_mut() = None;
        let Some(entry) = entry else { return };
        let payload = Rc::new(Payload::of_entry(&entry));
        let made = items(&payload, true);
        if !pasteboard.writeObjects(&writers(&made.items)) {
            crate::system::log("kompoz: the pasteboard took nothing");
            return;
        }
        *self.owned.borrow_mut() = Some(Owned { payload, _items: made, change_count: pasteboard.changeCount() });
    }

    /// The app going: what it put there given in full now, if the pasteboard still holds it (a provider can't answer
    /// once the app is gone).
    pub fn provide_all(&self) {
        let Some(own) = self.owned.borrow_mut().take() else { return };
        let pasteboard = NSPasteboard::generalPasteboard();
        if pasteboard.changeCount() != own.change_count {
            return;
        }
        pasteboard.clearContents();
        pasteboard.writeObjects(&writers(&items(&own.payload, false).items));
    }
}
