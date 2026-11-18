//! The clipboard as X selections (ICCCM; C++'s `readClipboard`, `writeClipboard`, `answerSelection`,
//! `sendNextPart`): what the app put there (its [`Payload`]: every type of its entry, as targets) and the requests it
//! answers, large data sent in parts (INCR) to each requestor in turn, and what reading another app's selection in
//! parts gathers. The parts that don't need the server, so that they can be tested as they are.

use super::ffi::{Atom, Window};
use crate::linux::transfer::Payload;
use std::ops::Range;
use std::rc::Rc;

/// The most of a property one request sets (C++'s `partSize`): what a request carries less its header, at most
/// 256 KiB; larger data goes in parts.
pub fn part_size(max_request_bytes: usize) -> usize {
    max_request_bytes.saturating_sub(256).clamp(1, 1 << 18)
}

/// Data being sent in parts (INCR): to which window's property, as which type, what, from where.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Transfer {
    pub requestor: Window,
    pub property: Atom,
    pub kind: Atom,
    pub data: Rc<[u8]>,
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

/// The transfers in parts going on (C++'s `transfers_`): the clipboard's, a drag's.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Transfers {
    list: Vec<Transfer>,
}

impl Transfers {
    /// A transfer in parts begins: its first part goes when the requestor deletes the INCR property.
    pub fn start(&mut self, requestor: Window, property: Atom, kind: Atom, data: Rc<[u8]>) {
        // (A new request into the same property replaces one going on.)
        self.list.retain(|t| !(t.requestor == requestor && t.property == property));
        self.list.push(Transfer { requestor, property, kind, data, offset: 0 });
    }

    /// A requestor took a part (deleted `property` of `window`): the next part of its transfer, if it has one going
    /// (C++'s `sendNextPart`), and the data it is of.
    pub fn next_part(&mut self, window: Window, property: Atom, part_size: usize) -> Option<(Part, Rc<[u8]>)> {
        let i = self.list.iter().position(|t| t.requestor == window && t.property == property)?;
        let t = &mut self.list[i];
        let data = t.data.clone();
        let start = t.offset.min(data.len());
        let n = part_size.min(data.len() - start);
        t.offset = start + n;
        let part = Part { kind: t.kind, range: start..start + n, done: n == 0 };
        if part.done {
            self.list.remove(i);
        }
        Some((part, data))
    }

    /// The transfers going on.
    pub fn transfers(&self) -> &[Transfer] {
        &self.list
    }
}

/// The clipboard the app holds (C++'s `clipboard_`, `ownsClipboard_`, `transfers_`): what it offers, as targets
/// (each type's atom and name), whether its window owns CLIPBOARD, the transfers in parts going on.
#[derive(Default)]
pub struct Clipboard {
    pub payload: Option<Payload>,
    pub targets: Vec<(Atom, String)>,
    pub owned: bool,
    pub transfers: Transfers,
}

impl Clipboard {
    /// The name of `target`, one it offers.
    pub fn target_name(&self, target: Atom) -> Option<&str> {
        self.targets.iter().find(|t| t.0 == target).map(|t| t.1.as_str())
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
    fn large_data_goes_in_parts_then_an_empty_one() {
        let mut t = Transfers::default();
        let (w, p, utf8) = (0x400001, 77, 300);
        let data: Rc<[u8]> = Rc::from(&b"xxxxxxxxxx"[..]);
        t.start(w, p, utf8, data.clone());
        assert_eq!(t.next_part(0x500000, p, 4), None); // (another window's property)
        let parts: Vec<Part> = std::iter::from_fn(|| t.next_part(w, p, 4).map(|(part, _)| part)).collect();
        assert_eq!(
            parts,
            [
                Part { kind: utf8, range: 0..4, done: false },
                Part { kind: utf8, range: 4..8, done: false },
                Part { kind: utf8, range: 8..10, done: false },
                Part { kind: utf8, range: 10..10, done: true },
            ]
        );
        assert!(t.transfers().is_empty());
        // Two requestors at once, each in its own place, each of its own data.
        let png: Rc<[u8]> = Rc::from(&[1u8, 2, 3][..]);
        t.start(1, p, utf8, data.clone());
        t.start(2, p, 400, png.clone());
        t.start(2, p, 400, png.clone()); // (asked again: begun again)
        assert_eq!(t.transfers().len(), 2);
        let (part, of) = t.next_part(1, p, 6).unwrap();
        assert_eq!((part.range, Rc::ptr_eq(&of, &data)), (0..6, true));
        let (part, of) = t.next_part(2, p, 2).unwrap();
        assert_eq!((part.kind, part.range, Rc::ptr_eq(&of, &png)), (400, 0..2, true));
        assert_eq!(t.next_part(2, p, 2).unwrap().0.range, 2..3);
        assert!(t.next_part(2, p, 2).unwrap().0.done);
        assert_eq!(t.next_part(1, p, 6).unwrap().0.range, 6..10);
        assert!(t.next_part(1, p, 6).unwrap().0.done);
        assert!(t.transfers().is_empty());
    }

    #[test]
    fn parts_read_are_gathered_until_the_empty_one() {
        let mut r = IncrReader::default();
        assert_eq!(r.part(b"Say"), None);
        assert_eq!(r.part("aç".as_bytes()), None);
        assert_eq!(r.part(b"").as_deref(), Some("Sayaç".as_bytes()));
        assert_eq!(r.part(b""), Some(Vec::new()));
    }
}
