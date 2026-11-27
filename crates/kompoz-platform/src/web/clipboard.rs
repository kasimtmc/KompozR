//! The clipboard and drops on the web (C++'s copy, cut and paste events, completed): what the app copies is offered
//! as the types other apps read (transfer.rs's: files and URIs as a `text/uri-list`, text, HTML, an image as PNG,
//! the app's own types) — into the copy or cut event being handled (Ctrl+C, Ctrl+X: the event's data, read by
//! the browser as the event returns), else through the asynchronous Clipboard API (a copy button's: as the browser
//! allows it, with the page focused). What a paste or a drop brings (its strings and its files: the page reads a
//! file's bytes before handing it over) is a [`ClipEntry`]: the text with its HTML, the URIs, each file as its
//! path in the web's memory ([`files`](super::files): `/drop/<n>/<name>`) with its bytes under its type.

use super::files;
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
use super::sys;
use crate::transfer::Payload;
use kompoz_ui::clipboard::{
    ClipEntry, ClipItem, ClipValue, MIMETYPE_TEXT_HTML, MIMETYPE_TEXT_PLAIN, MIMETYPE_TEXT_URILIST, file_uri,
};

/// Text, as the page's clipboard has it.
const TEXT_TYPES: &[&str] = &[MIMETYPE_TEXT_PLAIN];

/// One of a transfer's items, as the page describes it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TransferItem {
    pub file: bool,
    pub mime: String,
    pub name: String,
}

/// A transfer's items, from the page's lines (`kind\tmime\tname`, the name percent-encoded).
pub fn items_of(lines: &str) -> Vec<TransferItem> {
    lines
        .lines()
        .filter_map(|line| {
            let mut f = line.split('\t');
            let file = f.next()? == "file";
            let mime = f.next()?.to_string();
            let name = super::events::decoded(f.next().unwrap_or(""));
            Some(TransferItem { file, mime, name })
        })
        .collect()
}

/// What a transfer brings, read from its items as `read(index)` gives their bytes; files kept as `kind`'s
/// ("drop", "paste"). None if it brings nothing.
pub fn entry_of(items: &[TransferItem], kind: &str, read: impl Fn(usize) -> Option<Vec<u8>>) -> Option<ClipEntry> {
    let string = |mime: &str| {
        items
            .iter()
            .position(|i| !i.file && i.mime.eq_ignore_ascii_case(mime))
            .and_then(&read)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    };
    let mut entry_items = Vec::new();
    // Files first: as their paths (a uri-list's lines, what targets read files by) and their bytes.
    for (index, item) in items.iter().enumerate().filter(|(_, i)| i.file) {
        let Some(bytes) = read(index) else { continue };
        let mime = if item.mime.is_empty() { "application/octet-stream".to_string() } else { item.mime.clone() };
        let shared: std::rc::Rc<[u8]> = bytes.as_slice().into();
        let path = files::keep(kind, &item.name, bytes);
        entry_items.push(ClipItem::of(vec![
            (MIMETYPE_TEXT_URILIST.into(), ClipValue::Text(file_uri(&path))),
            (mime, ClipValue::Bytes(shared)),
        ]));
    }
    let text = string(MIMETYPE_TEXT_PLAIN);
    let html = string(MIMETYPE_TEXT_HTML);
    match (text, html) {
        (Some(text), Some(html)) => entry_items.push(ClipItem::with_html_text(text, html)),
        (Some(text), None) => entry_items.push(ClipItem::with_text(text)),
        (None, Some(html)) => entry_items.push(ClipItem::of(vec![(MIMETYPE_TEXT_HTML.into(), ClipValue::Text(html))])),
        (None, None) => {}
    }
    // Links (not the files': those are above).
    if !items.iter().any(|i| i.file)
        && let Some(list) = string(MIMETYPE_TEXT_URILIST)
    {
        for uri in kompoz_ui::clipboard::uris_in_uri_list(&list) {
            entry_items.push(ClipItem::with_uri(uri));
        }
    }
    // Every other type a page put in (an app's own, as it gave it).
    for (index, item) in items.iter().enumerate() {
        let known = [MIMETYPE_TEXT_PLAIN, MIMETYPE_TEXT_HTML, MIMETYPE_TEXT_URILIST];
        if item.file || known.iter().any(|k| item.mime.eq_ignore_ascii_case(k)) || item.mime.is_empty() {
            continue;
        }
        if let Some(bytes) = read(index) {
            entry_items.push(ClipItem::of(vec![(item.mime.clone(), ClipValue::Bytes(bytes.as_slice().into()))]));
        }
    }
    (!entry_items.is_empty()).then(|| ClipEntry::new(entry_items))
}

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
/// What the page's transfer `transfer` brings (a paste's, a drop's).
pub fn read_transfer(transfer: u32, kind: &str) -> Option<ClipEntry> {
    let items = items_of(&sys::transfer_items(transfer));
    entry_of(&items, kind, |index| sys::transfer_data(transfer, index as u32))
}

/// What the app offers, as the page is given it: each type and its bytes, the best first.
pub fn offered(entry: &ClipEntry) -> Vec<(String, Vec<u8>)> {
    let payload = Payload::of_entry(entry);
    payload
        .offered(TEXT_TYPES)
        .into_iter()
        .filter_map(|mime| payload.bytes(&mime, TEXT_TYPES).map(|b| (mime, b.to_vec())))
        .collect()
}

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
/// `entry` put on the page's clipboard: into the copy or cut event being handled, else asynchronously.
pub fn offer(entry: &ClipEntry, in_event: bool) {
    sys::clip_begin();
    for (mime, bytes) in offered(entry) {
        sys::clip_add(&mime, &bytes);
    }
    sys::clip_commit(in_event);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(file: bool, mime: &str, name: &str) -> TransferItem {
        TransferItem { file, mime: mime.into(), name: name.into() }
    }

    #[test]
    fn the_pages_items_are_read() {
        assert_eq!(
            items_of("string\ttext/plain\t\nfile\timage/png\tG%C3%B6r%C3%BCnt%C3%BC.png"),
            [item(false, "text/plain", ""), item(true, "image/png", "Görüntü.png")]
        );
    }

    #[test]
    fn transfers_become_entries() {
        let items = [item(false, "text/plain", ""), item(false, "text/html", "")];
        let data = [b"merhaba".to_vec(), b"<b>merhaba</b>".to_vec()];
        let entry = entry_of(&items, "paste", |i| data.get(i).cloned()).unwrap();
        assert_eq!(entry.items().len(), 1);
        assert_eq!(entry.items()[0].text().as_deref(), Some("merhaba"));
        assert_eq!(entry.items()[0].html_text().as_deref(), Some("<b>merhaba</b>"));

        let items = [item(true, "text/csv", "a.csv"), item(false, "text/uri-list", "")];
        let data = [b"1,2".to_vec(), b"https://example.org/".to_vec()];
        let entry = entry_of(&items, "drop", |i| data.get(i).cloned()).unwrap();
        let files = entry.items()[0].files();
        assert_eq!(files.len(), 1);
        assert!(files[0].starts_with("/drop/") && files[0].ends_with("/a.csv"), "{files:?}");
        assert_eq!(files::read(&files[0]), Some(b"1,2".to_vec()));
        assert_eq!(entry.items()[0].bytes("text/csv").as_deref(), Some(&b"1,2"[..]));
        assert_eq!(entry.items().len(), 1, "a file's drop's links are its files'");

        assert_eq!(entry_of(&[], "drop", |_| None), None);
    }

    #[test]
    fn what_the_app_copies_is_offered_as_text_and_html() {
        let offered = offered(&ClipEntry::with_html_text("a", "<i>a</i>"));
        assert!(offered.contains(&("text/plain".into(), b"a".to_vec())), "{offered:?}");
        assert!(offered.contains(&("text/html".into(), b"<i>a</i>".to_vec())), "{offered:?}");
    }
}
