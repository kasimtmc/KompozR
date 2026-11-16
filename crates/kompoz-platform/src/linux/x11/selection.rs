//! The clipboard as X selections (ICCCM; C++'s `readClipboard`, `writeClipboard`, `answerSelection`,
//! `sendNextPart`): the text the app put there, the requests it answers (its targets, the text as UTF-8 or Latin-1),
//! a large text sent in parts (INCR) to each requestor in turn, and what reading another app's selection gathers.
//! The parts that don't need the server, so that they can be tested as they are.

use super::ffi::{Atom, Window};
use std::ops::Range;

/// What a text target's bytes are: UTF-8 (`UTF8_STRING`, `TEXT`, `text/plain;charset=utf-8`, and `text/plain` as
/// apps send it) or Latin-1 (`STRING`, as ICCCM has it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Encoding {
    Utf8,
    Latin1,
}

/// `text` in `encoding` (Latin-1: what it can't hold as '?').
pub fn encode(text: &str, encoding: Encoding) -> Vec<u8> {
    match encoding {
        Encoding::Utf8 => text.as_bytes().to_vec(),
        Encoding::Latin1 => text.chars().map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')).collect(),
    }
}

/// Bytes in `encoding` as text (UTF-8 that isn't valid: replaced where it isn't).
pub fn decode(bytes: &[u8], encoding: Encoding) -> String {
    match encoding {
        Encoding::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
        Encoding::Latin1 => bytes.iter().map(|&b| char::from(b)).collect(),
    }
}

/// The most of a property one request sets (C++'s `partSize`): what a request carries less its header, at most
/// 256 KiB; a larger text goes in parts.
pub fn part_size(max_request_bytes: usize) -> usize {
    max_request_bytes.saturating_sub(256).clamp(1, 1 << 18)
}

/// A large text being sent in parts (INCR): to which window's property, as which type, from where.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Transfer {
    pub requestor: Window,
    pub property: Atom,
    pub kind: Atom,
    pub offset: usize,
}

/// The next part to put into a requestor's property, the one before taken.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Part {
    pub kind: Atom,
    /// What of the data it is (empty: the end).
    pub range: Range<usize>,
    /// The transfer is over (the empty part after the last sent): the requestor's events no longer wanted.
    pub done: bool,
}

/// The clipboard the app holds (C++'s `clipboard_`, `ownsClipboard_`, `transfers_`).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Clipboard {
    /// The text put there (UTF-8).
    pub text: String,
    /// The app's window owns the CLIPBOARD selection.
    pub owned: bool,
    transfers: Vec<Transfer>,
}

impl Clipboard {
    /// A requestor took a part (deleted `property` of `window`): the next part of its transfer, if it has one going
    /// (C++'s `sendNextPart`), `data` being the text as the transfer's type has it.
    pub fn next_part(&mut self, window: Window, property: Atom, data_len: usize, part_size: usize) -> Option<Part> {
        let i = self.transfers.iter().position(|t| t.requestor == window && t.property == property)?;
        let t = &mut self.transfers[i];
        let start = t.offset.min(data_len);
        let n = part_size.min(data_len - start);
        t.offset = start + n;
        let part = Part { kind: t.kind, range: start..start + n, done: n == 0 };
        if part.done {
            self.transfers.remove(i);
        }
        Some(part)
    }

    /// A transfer in parts begins: its first part goes when the requestor deletes the INCR property.
    pub fn start_transfer(&mut self, requestor: Window, property: Atom, kind: Atom) {
        // (A new request into the same property replaces one going on.)
        self.transfers.retain(|t| !(t.requestor == requestor && t.property == property));
        self.transfers.push(Transfer { requestor, property, kind, offset: 0 });
    }

    /// The transfers going on.
    pub fn transfers(&self) -> &[Transfer] {
        &self.transfers
    }
}

/// Reading another app's selection that comes in parts (INCR): each part a new value of the property, deleted once
/// read, an empty one at the end.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct IncrReader {
    data: Vec<u8>,
}

impl IncrReader {
    /// A part read; the whole once it is the empty last one.
    pub fn part(&mut self, bytes: &[u8]) -> Option<Vec<u8>> {
        if bytes.is_empty() {
            return Some(std::mem::take(&mut self.data));
        }
        self.data.extend_from_slice(bytes);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_are_what_a_request_carries_at_most_256_kib() {
        assert_eq!(part_size(262_140), 261_884); // (no BIG-REQUESTS: 65535 units of 4 bytes)
        assert_eq!(part_size(16_777_212), 1 << 18);
        assert_eq!(part_size(0), 1);
    }

    #[test]
    fn a_large_text_goes_in_parts_then_an_empty_one() {
        let mut c = Clipboard { text: "x".repeat(10), owned: true, ..Clipboard::default() };
        let (w, p, utf8) = (0x400001, 77, 300);
        c.start_transfer(w, p, utf8);
        assert_eq!(c.next_part(0x500000, p, 10, 4), None); // (another window's property)
        let parts: Vec<Part> = std::iter::from_fn(|| c.next_part(w, p, 10, 4)).collect();
        assert_eq!(
            parts,
            [
                Part { kind: utf8, range: 0..4, done: false },
                Part { kind: utf8, range: 4..8, done: false },
                Part { kind: utf8, range: 8..10, done: false },
                Part { kind: utf8, range: 10..10, done: true },
            ]
        );
        assert!(c.transfers().is_empty());
        // Two requestors at once, each in its own place; the text changing meanwhile ends a transfer early.
        c.start_transfer(1, p, utf8);
        c.start_transfer(2, p, utf8);
        c.start_transfer(2, p, utf8); // (asked again: begun again)
        assert_eq!(c.transfers().len(), 2);
        assert_eq!(c.next_part(1, p, 10, 6).unwrap().range, 0..6);
        assert_eq!(c.next_part(2, p, 10, 6).unwrap().range, 0..6);
        assert_eq!(c.next_part(1, p, 3, 6), Some(Part { kind: utf8, range: 3..3, done: true }));
        assert_eq!(c.next_part(2, p, 10, 6).unwrap().range, 6..10);
        assert!(c.next_part(2, p, 10, 6).unwrap().done);
        assert!(c.transfers().is_empty());
    }

    #[test]
    fn parts_read_are_gathered_until_the_empty_one() {
        let mut r = IncrReader::default();
        assert_eq!(r.part(b"Say"), None);
        assert_eq!(r.part("aç".as_bytes()), None);
        assert_eq!(r.part(b"").as_deref(), Some("Sayaç".as_bytes()));
        assert_eq!(r.part(b""), Some(Vec::new()));
    }

    #[test]
    fn text_targets_encode_as_their_charset() {
        assert_eq!(encode("Sayaç €", Encoding::Utf8), "Sayaç €".as_bytes());
        assert_eq!(encode("Sayaç €", Encoding::Latin1), b"Saya\xe7 ?");
        assert_eq!(decode(b"Saya\xe7", Encoding::Latin1), "Sayaç");
        assert_eq!(decode("ğ".as_bytes(), Encoding::Utf8), "ğ");
        assert_eq!(decode(b"\xff", Encoding::Utf8), "\u{fffd}");
    }
}
