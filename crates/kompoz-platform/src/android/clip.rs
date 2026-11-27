//! Clips as they cross to KompozClipboard and KompozDragAndDrop and back (C++'s `clipParts`, one byte array where
//! C++ had `byte[][]`, little-endian): an entry of the app's, each item's representations with their bytes, for a
//! ClipData of the system's (text, HTML, URIs, and the app's own types, which Android's items can't hold: carried
//! in the item's Intent); and another app's clip described without being read (its ClipDescription: its MIME types,
//! whether it is sensitive — `ClipDescription.EXTRA_IS_SENSITIVE` — and its time stamp), read as the app asks for a
//! type. Pure: tested on the host.

use kompoz_ui::clipboard::{ClipEntry, ClipMetadata};

/// Writes the wire's values.
#[derive(Default)]
struct Writer(Vec<u8>);

impl Writer {
    fn u32(&mut self, v: usize) {
        self.0.extend_from_slice(&(v.min(u32::MAX as usize) as u32).to_le_bytes());
    }
    fn bytes(&mut self, b: &[u8]) {
        self.u32(b.len());
        self.0.extend_from_slice(b);
    }
}

/// Reads them (none past the end).
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let b = self.data.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(b)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn u32(&mut self) -> Option<usize> {
        self.take(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
    }
    fn i64(&mut self) -> Option<i64> {
        self.take(8).map(|b| i64::from_le_bytes(b.try_into().unwrap_or([0; 8])))
    }
    fn string(&mut self) -> Option<String> {
        let n = self.u32()?;
        Some(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
}

/// An entry of the app's for KompozClipboard's `set` (and a drag's): whether it is sensitive (a byte), its MIME
/// types (a count, then each), its items (a count, then per item its representations: a count, then each one's
/// MIME type and bytes — text as UTF-8, an image as PNG, another app's read now). A representation that can't be
/// read is left out.
pub fn encode_entry(entry: &ClipEntry) -> Vec<u8> {
    let mut w = Writer::default();
    let metadata = entry.clip_metadata();
    w.0.push(metadata.is_sensitive() as u8);
    w.u32(metadata.mime_types().len());
    for mime in metadata.mime_types() {
        w.bytes(mime.as_bytes());
    }
    w.u32(entry.items().len());
    for item in entry.items() {
        let representations: Vec<(String, std::rc::Rc<[u8]>)> = item
            .mime_types()
            .into_iter()
            .filter_map(|mime| {
                let bytes = item.bytes(&mime)?;
                Some((mime, bytes))
            })
            .collect();
        w.u32(representations.len());
        for (mime, bytes) in representations {
            w.bytes(mime.as_bytes());
            w.bytes(&bytes);
        }
    }
    w.0
}

/// Another app's clip as described (KompozClipboard's `description`, a drop's): whether it is sensitive (a byte),
/// its MIME types (a count, then each), its time stamp (8 bytes; what tells a read of a clip replaced since).
#[derive(Clone, PartialEq, Debug)]
pub struct Description {
    pub metadata: ClipMetadata,
    pub timestamp: i64,
}

/// The description's bytes read; none if they aren't one.
pub fn decode_description(bytes: &[u8]) -> Option<Description> {
    let mut r = Reader { data: bytes, at: 0 };
    let sensitive = r.u8()? != 0;
    let count = r.u32()?;
    let mut mime_types = Vec::with_capacity(count.min(64));
    for _ in 0..count {
        let mime = r.string()?;
        if !mime.is_empty() && !mime_types.contains(&mime) {
            mime_types.push(mime);
        }
    }
    let timestamp = r.i64().unwrap_or(0);
    Some(Description { metadata: ClipMetadata::new(mime_types).with_sensitive(sensitive), timestamp })
}

/// A description's bytes as KompozClipboard writes them (for the tests, and what the drag of the app's own is told
/// by).
pub fn encode_description(metadata: &ClipMetadata, timestamp: i64) -> Vec<u8> {
    let mut w = Writer::default();
    w.0.push(metadata.is_sensitive() as u8);
    w.u32(metadata.mime_types().len());
    for mime in metadata.mime_types() {
        w.bytes(mime.as_bytes());
    }
    w.0.extend_from_slice(&timestamp.to_le_bytes());
    w.0
}

/// An encoded entry read back (what the tests check KompozClipboard's reading against): the sensitivity, the
/// types, and per item its representations.
#[cfg(test)]
#[allow(clippy::type_complexity)]
fn decode_entry(bytes: &[u8]) -> Option<(bool, Vec<String>, Vec<Vec<(String, Vec<u8>)>>)> {
    let mut r = Reader { data: bytes, at: 0 };
    let sensitive = r.u8()? != 0;
    let types = (0..r.u32()?).map(|_| r.string()).collect::<Option<Vec<_>>>()?;
    let items = (0..r.u32()?)
        .map(|_| {
            let n = r.u32()?;
            (0..n)
                .map(|_| {
                    let mime = r.string()?;
                    let len = r.u32()?;
                    Some((mime, r.take(len)?.to_vec()))
                })
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Option<Vec<_>>>()?;
    Some((sensitive, types, items))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kompoz_ui::clipboard::{ClipItem, ClipValue};

    #[test]
    fn entries_cross_with_every_representation() {
        let item = ClipItem::of(vec![
            ("text/plain".into(), ClipValue::Text("Merhaba 👋".into())),
            ("text/html".into(), ClipValue::Text("<b>Merhaba</b> 👋".into())),
            ("application/x-kompozr-card".into(), ClipValue::Bytes(vec![1, 2, 3].into())),
        ]);
        let uris = ClipItem::with_uri("content://media/1");
        let entry = ClipEntry::with_metadata(
            vec![item, uris],
            ClipMetadata::new(vec!["text/plain".into(), "text/html".into()]).with_sensitive(true),
        );
        let (sensitive, types, items) = decode_entry(&encode_entry(&entry)).expect("an entry");
        assert!(sensitive);
        assert_eq!(types, vec!["text/plain".to_string(), "text/html".to_string()]);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0][0], ("text/plain".to_string(), "Merhaba 👋".as_bytes().to_vec()));
        assert_eq!(items[0][2], ("application/x-kompozr-card".to_string(), vec![1, 2, 3]));
        assert_eq!(items[1][0].0, "text/uri-list");
        assert!(String::from_utf8_lossy(&items[1][0].1).contains("content://media/1"));
    }

    #[test]
    fn descriptions_are_read_without_the_clip() {
        let metadata = ClipMetadata::new(vec!["text/plain".into(), "image/png".into()]).with_sensitive(true);
        let d = decode_description(&encode_description(&metadata, 1234)).expect("a description");
        assert_eq!(d, Description { metadata, timestamp: 1234 });
        assert!(d.metadata.is_sensitive() && d.metadata.has_mime_type("image/png"));
        // (Cut short: what is there; nothing: none.)
        assert!(decode_description(&[]).is_none());
        assert!(decode_description(&[0, 1, 0, 0, 0]).is_none());
        let d = decode_description(&[0, 0, 0, 0, 0]).expect("no types, no time");
        assert_eq!((d.metadata.mime_type_count(), d.timestamp), (0, 0));
    }
}
