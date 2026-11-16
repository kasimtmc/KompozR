//! Drags from other apps over the window (XDND 5, C++'s `message` for the Xdnd messages): the drag's source, the
//! type taken from what it offers (files, else UTF-8 text), where it is, and what it dropped as KompozR's
//! [`Event::DropBegin`] … [`Event::DropComplete`]. The server's part (reading the source's type list, translating
//! positions, asking for the data, replying) is the platform's; this is what can be tested as it is.

use super::ffi::{Atom, NONE, Time, Window};
use crate::app::Event;
use crate::linux::desktop::files_in_uri_list;
use std::ffi::c_long;

/// The types a drop is taken as, the best first: files, then UTF-8 text, then plain text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DropTypes {
    pub uri_list: Atom,
    pub text_plain_utf8: Atom,
    pub utf8_string: Atom,
    pub text_plain: Atom,
}

impl DropTypes {
    fn preferred(&self) -> [Atom; 4] {
        [self.uri_list, self.text_plain_utf8, self.utf8_string, self.text_plain]
    }
}

/// `XdndStatus`'s flags: the drop is taken (1), and each move is to be told (2).
pub const STATUS_ACCEPT: c_long = 1 | 2;

/// A drag from another app over the window.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct XdndTarget {
    /// The drag's source window (0: none).
    source: Window,
    /// The type taken (none: nothing it offers).
    kind: Atom,
    /// A move told (DropBegin sent).
    begun: bool,
    /// Where it was last, in the window.
    at: (i32, i32),
}

/// What a drop asks of the platform: the data, as `kind`, at `time`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DropRequest {
    pub kind: Atom,
    pub time: Time,
}

impl XdndTarget {
    /// The drag's source (0: none).
    pub fn source(&self) -> Window {
        self.source
    }

    /// Whether the source wants the full type list read (`XdndEnter`'s bit 0: more than three types).
    pub fn has_type_list(data: &[c_long; 5]) -> bool {
        data[1] & 1 != 0
    }

    /// `XdndEnter`: a drag from `data[0]` came in, offering the three types in its message, or `type_list` (its
    /// `XdndTypeList`, read by the platform when [`has_type_list`](XdndTarget::has_type_list)).
    pub fn enter(&mut self, data: &[c_long; 5], type_list: Option<&[c_long]>, types: &DropTypes) {
        self.source = data[0] as Window;
        let offered: Vec<Atom> = match type_list {
            Some(list) => list.iter().map(|&t| t as Atom).collect(),
            None => data[2..5].iter().filter(|&&t| t != 0).map(|&t| t as Atom).collect(),
        };
        self.kind = types.preferred().into_iter().find(|t| offered.contains(t)).unwrap_or(NONE);
        self.begun = false;
    }

    /// `XdndPosition` from `data[0]`: where on the screen it is (`data[2]`: x above, y below), if it is the drag's.
    pub fn position_on_screen(&self, data: &[c_long; 5]) -> Option<(i32, i32)> {
        if self.source == 0 || data[0] as Window != self.source {
            return None;
        }
        Some((((data[2] >> 16) & 0xffff) as i32, (data[2] & 0xffff) as i32))
    }

    /// It moved to (`x`, `y`) in the window: its events, and the status to reply (flags, action taken or none).
    pub fn moved(&mut self, x: i32, y: i32, copy: Atom) -> (Vec<Event>, c_long, Atom) {
        self.at = (x, y);
        let (fx, fy) = (x as f32, y as f32);
        let mut events = Vec::new();
        if !std::mem::replace(&mut self.begun, true) {
            events.push(Event::DropBegin { x: fx, y: fy });
        }
        events.push(Event::DropMove { x: fx, y: fy });
        let taken = self.kind != NONE;
        (events, if taken { STATUS_ACCEPT } else { 0 }, if taken { copy } else { NONE })
    }

    /// `XdndLeave` from `data[0]`: it left, if it is the drag's.
    pub fn leave(&mut self, data: &[c_long; 5]) -> Vec<Event> {
        if self.source == 0 || data[0] as Window != self.source {
            return Vec::new();
        }
        let begun = self.begun;
        *self = XdndTarget::default();
        let (x, y) = (0.0, 0.0);
        if begun { vec![Event::DropComplete { x, y }] } else { Vec::new() }
    }

    /// `XdndDrop` from `data[0]`: what to ask for (none if it offers nothing taken), if it is the drag's.
    pub fn drop_request(&self, data: &[c_long; 5]) -> Option<Option<DropRequest>> {
        if self.source == 0 || data[0] as Window != self.source {
            return None;
        }
        Some((self.kind != NONE).then_some(DropRequest { kind: self.kind, time: data[2] as Time }))
    }

    /// The drop's data came (none: refused, or no answer): its events (each file of a uri-list, else the text, then
    /// the end), and the finish to reply (accepted, action or none). The drag is over.
    pub fn dropped(&mut self, data: Option<String>, types: &DropTypes, copy: Atom) -> (Vec<Event>, c_long, Atom) {
        let (x, y) = (self.at.0 as f32, self.at.1 as f32);
        let mut events = Vec::new();
        match &data {
            Some(list) if self.kind == types.uri_list => {
                events.extend(files_in_uri_list(list).into_iter().map(|path| Event::DropFile { x, y, path }));
            }
            Some(text) => events.push(Event::DropText { x, y, text: text.clone() }),
            None => {}
        }
        events.push(Event::DropComplete { x, y });
        *self = XdndTarget::default();
        let ok = data.is_some();
        (events, c_long::from(ok), if ok { copy } else { NONE })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TYPES: DropTypes = DropTypes { uri_list: 100, text_plain_utf8: 101, utf8_string: 102, text_plain: 103 };
    const COPY: Atom = 200;
    const SOURCE: c_long = 0x3c00007;

    #[test]
    fn files_are_taken_before_text() {
        let mut t = XdndTarget::default();
        t.enter(&[SOURCE, 5 << 24, 103, 102, 0], None, &TYPES);
        assert_eq!((t.source(), t.kind), (SOURCE as Window, 102));
        t.enter(&[SOURCE, (5 << 24) | 1, 0, 0, 0], Some(&[555, 103, 100, 101]), &TYPES);
        assert!(XdndTarget::has_type_list(&[SOURCE, 1, 0, 0, 0]));
        assert_eq!(t.kind, 100);
        t.enter(&[SOURCE, 0, 555, 556, 0], None, &TYPES);
        assert_eq!(t.kind, NONE); // (nothing it offers is taken)
    }

    #[test]
    fn a_drag_moves_then_drops_its_files() {
        let mut t = XdndTarget::default();
        t.enter(&[SOURCE, 0, 100, 0, 0], None, &TYPES);
        let position = [SOURCE, 0, (300 << 16) | 200, 0, COPY as c_long];
        assert_eq!(t.position_on_screen(&position), Some((300, 200)));
        assert_eq!(t.position_on_screen(&[42, 0, 0, 0, 0]), None); // (another source's)
        let (events, flags, action) = t.moved(30, 20, COPY);
        assert_eq!(events, [Event::DropBegin { x: 30.0, y: 20.0 }, Event::DropMove { x: 30.0, y: 20.0 }]);
        assert_eq!((flags, action), (STATUS_ACCEPT, COPY));
        let (events, _, _) = t.moved(31, 22, COPY);
        assert_eq!(events, [Event::DropMove { x: 31.0, y: 22.0 }]);
        assert_eq!(t.drop_request(&[42, 0, 0, 0, 0]), None);
        assert_eq!(t.drop_request(&[SOURCE, 0, 1234, 0, 0]), Some(Some(DropRequest { kind: 100, time: 1234 })));
        let list = "file:///home/kasim/a%20b.png\r\nfile:///tmp/c.txt\r\n".to_string();
        let (events, ok, action) = t.dropped(Some(list), &TYPES, COPY);
        assert_eq!(
            events,
            [
                Event::DropFile { x: 31.0, y: 22.0, path: "/home/kasim/a b.png".into() },
                Event::DropFile { x: 31.0, y: 22.0, path: "/tmp/c.txt".into() },
                Event::DropComplete { x: 31.0, y: 22.0 },
            ]
        );
        assert_eq!((ok, action), (1, COPY));
        assert_eq!(t, XdndTarget::default());
    }

    #[test]
    fn text_drops_refusals_and_leaving() {
        let mut t = XdndTarget::default();
        t.enter(&[SOURCE, 0, 101, 0, 0], None, &TYPES);
        t.moved(5, 6, COPY);
        let (events, ok, action) = t.dropped(Some("merhaba".into()), &TYPES, COPY);
        assert_eq!(
            events,
            [Event::DropText { x: 5.0, y: 6.0, text: "merhaba".into() }, Event::DropComplete { x: 5.0, y: 6.0 }]
        );
        assert_eq!((ok, action), (1, COPY));
        // Nothing it offers is taken: the moves say so, the drop asks for nothing and finishes refused.
        t.enter(&[SOURCE, 0, 555, 0, 0], None, &TYPES);
        assert_eq!(t.moved(1, 1, COPY).1, 0);
        assert_eq!(t.drop_request(&[SOURCE, 0, 0, 0, 0]), Some(None));
        let (events, ok, action) = t.dropped(None, &TYPES, COPY);
        assert_eq!((events.as_slice(), ok, action), ([Event::DropComplete { x: 1.0, y: 1.0 }].as_slice(), 0, NONE));
        // Left before a move: nothing to tell; after one: complete (with nothing: the core tells the app it left).
        t.enter(&[SOURCE, 0, 100, 0, 0], None, &TYPES);
        assert!(t.leave(&[42, 0, 0, 0, 0]).is_empty());
        assert!(t.leave(&[SOURCE, 0, 0, 0, 0]).is_empty());
        t.enter(&[SOURCE, 0, 100, 0, 0], None, &TYPES);
        t.moved(1, 1, COPY);
        assert_eq!(t.leave(&[SOURCE, 0, 0, 0, 0]), [Event::DropComplete { x: 0.0, y: 0.0 }]);
        assert_eq!(t.source(), 0);
    }
}
