//! Drags from other apps over the window (XDND 5, C++'s `message` for the Xdnd messages): the drag's source, the
//! types it offers (each atom and its name), where it is, and what it dropped as KompozR's [`Event::DropBegin`]
//! (with what it offers) … [`Event::DropComplete`] — what it carries the platform's to give as an entry of its types,
//! read from the source as a target asks for them ([`Event::DropData`], until the drop is done). The app's
//! own drag coming back over the window (its source our window: [`dnd_source`](super::dnd_source)) is taken whatever
//! it offers, with the action asked for, and dropped as the app's own ([`Event::DropOwn`]: nothing to read).
//! Whether a drop was taken is the core's to say (the finish is sent then). The server's part (reading the source's
//! type list and the atoms' names, translating positions, asking for the data, replying) is the platform's; this is
//! what can be tested as it is.

use super::ffi::{Atom, NONE, Time, Window};
use crate::app::Event;
use kompoz_ui::clipboard::ClipMetadata;
use std::ffi::c_long;

/// `XdndStatus`'s flags: the drop is taken (1), and each move is to be told (2).
pub const STATUS_ACCEPT: c_long = 1 | 2;

/// A drag from another app (or the app's own) over the window.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct XdndTarget {
    /// The drag's source window (0: none).
    source: Window,
    /// It is the app's own (its source the app's window).
    own: bool,
    /// The types it offers: each atom and its name.
    offered: Vec<(Atom, String)>,
    /// A move told (DropBegin sent).
    begun: bool,
    /// Where it was last, in the window.
    at: (i32, i32),
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

    /// The types an `XdndEnter` offers: the three in its message, or `type_list` (its `XdndTypeList`, read by the
    /// platform when [`has_type_list`](XdndTarget::has_type_list)).
    pub fn offered_atoms(data: &[c_long; 5], type_list: Option<&[c_long]>) -> Vec<Atom> {
        match type_list {
            Some(list) => list.iter().map(|&t| t as Atom).filter(|&t| t != NONE).collect(),
            None => data[2..5].iter().filter(|&&t| t != 0).map(|&t| t as Atom).collect(),
        }
    }

    /// Whether it is the app's own drag.
    pub fn is_own(&self) -> bool {
        self.own
    }

    /// The types it offers, as an entry describes them.
    pub fn mime_types(&self) -> Vec<String> {
        let names: Vec<String> = self.offered.iter().map(|t| t.1.clone()).collect();
        ClipMetadata::of_offered(&names).mime_types().to_vec()
    }

    /// The types it offers (each atom and its name).
    pub fn offered(&self) -> &[(Atom, String)] {
        &self.offered
    }

    /// `XdndEnter`: a drag from `data[0]` came in, offering `offered` (each atom and its name); the app's own if
    /// its source is `own_window`.
    pub fn enter(&mut self, data: &[c_long; 5], offered: Vec<(Atom, String)>, own_window: Window) {
        self.source = data[0] as Window;
        self.own = own_window != NONE && self.source == own_window;
        self.offered = offered;
        self.begun = false;
    }

    /// `XdndPosition` from `data[0]`: where on the screen it is (`data[2]`: x above, y below), if it is the drag's.
    pub fn position_on_screen(&self, data: &[c_long; 5]) -> Option<(i32, i32)> {
        if self.source == 0 || data[0] as Window != self.source {
            return None;
        }
        Some((((data[2] >> 16) & 0xffff) as i32, (data[2] & 0xffff) as i32))
    }

    /// It moved to (`x`, `y`) in the window: its events (the first with what it offers), and the status to reply
    /// (flags, the action taken — `action`: copy for another app's, the one asked for for the app's own — or none:
    /// it offers nothing).
    pub fn moved(&mut self, x: i32, y: i32, action: Atom) -> (Vec<Event>, c_long, Atom) {
        self.at = (x, y);
        let (fx, fy) = (x as f32, y as f32);
        let mut events = Vec::new();
        if !std::mem::replace(&mut self.begun, true) {
            events.push(Event::DropBegin { x: fx, y: fy, mime_types: self.mime_types() });
        }
        events.push(Event::DropMove { x: fx, y: fy });
        let taken = (!self.offered.is_empty() || self.own) && action != NONE;
        (events, if taken { STATUS_ACCEPT } else { 0 }, if taken { action } else { NONE })
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

    /// `XdndDrop` from `data[0]`, if it is the drag's: the time its data is to be asked for at (none: nothing to
    /// read — it offers nothing, or it is the app's own: the host has it).
    pub fn drop_time(&self, data: &[c_long; 5]) -> Option<Option<Time>> {
        if self.source == 0 || data[0] as Window != self.source {
            return None;
        }
        Some((!self.offered.is_empty() && !self.own).then_some(data[2] as Time))
    }

    /// Dropped, what it carries read from its source as it is asked for (`has_data`; else nothing to read): its
    /// events (the app's own drop, or its data the platform's; then the end). The drag is over: the finish is sent
    /// once the core says whether a target took it.
    pub fn dropped(&mut self, has_data: bool) -> Vec<Event> {
        let (x, y) = (self.at.0 as f32, self.at.1 as f32);
        let mut events = Vec::new();
        if self.own {
            events.push(Event::DropOwn { x, y });
        } else if has_data {
            events.push(Event::DropData { x, y });
        }
        events.push(Event::DropComplete { x, y });
        *self = XdndTarget::default();
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COPY: Atom = 200;
    const SOURCE: c_long = 0x3c00007;
    const OWN: Window = 0x1e00005;

    fn named(atoms: &[Atom]) -> Vec<(Atom, String)> {
        atoms
            .iter()
            .map(|&a| {
                let name = match a {
                    100 => "text/uri-list",
                    101 => "text/plain;charset=utf-8",
                    102 => "UTF8_STRING",
                    103 => "image/png",
                    _ => "application/x-other",
                };
                (a, name.to_string())
            })
            .collect()
    }

    #[test]
    fn the_types_offered_are_the_messages_or_the_list() {
        let data = [SOURCE, 5 << 24, 103, 102, 0];
        assert_eq!(XdndTarget::offered_atoms(&data, None), [103, 102]);
        assert!(XdndTarget::has_type_list(&[SOURCE, 1, 0, 0, 0]));
        assert_eq!(XdndTarget::offered_atoms(&[SOURCE, 1, 0, 0, 0], Some(&[555, 103, 0, 100])), [555, 103, 100]);
        let mut t = XdndTarget::default();
        t.enter(&data, named(&[102, 103]), OWN);
        assert_eq!((t.source(), t.is_own()), (SOURCE as Window, false));
        // (Described as an entry is: X11's text names as text/plain too.)
        assert_eq!(t.mime_types(), ["UTF8_STRING", "image/png", "text/plain"]);
    }

    #[test]
    fn a_drag_moves_then_drops_what_it_offers() {
        let mut t = XdndTarget::default();
        t.enter(&[SOURCE, 0, 100, 0, 0], named(&[100, 103]), OWN);
        let position = [SOURCE, 0, (300 << 16) | 200, 0, COPY as c_long];
        assert_eq!(t.position_on_screen(&position), Some((300, 200)));
        assert_eq!(t.position_on_screen(&[42, 0, 0, 0, 0]), None); // (another source's)
        let (events, flags, action) = t.moved(30, 20, COPY);
        let mime_types = vec!["text/uri-list".to_string(), "image/png".into()];
        assert_eq!(events, [Event::DropBegin { x: 30.0, y: 20.0, mime_types }, Event::DropMove { x: 30.0, y: 20.0 }]);
        assert_eq!((flags, action), (STATUS_ACCEPT, COPY));
        let (events, _, _) = t.moved(31, 22, COPY);
        assert_eq!(events, [Event::DropMove { x: 31.0, y: 22.0 }]);
        assert_eq!(t.drop_time(&[42, 0, 0, 0, 0]), None);
        assert_eq!(t.drop_time(&[SOURCE, 0, 1234, 0, 0]), Some(Some(1234)));
        let events = t.dropped(true);
        assert_eq!(events, [Event::DropData { x: 31.0, y: 22.0 }, Event::DropComplete { x: 31.0, y: 22.0 }]);
        assert_eq!(t, XdndTarget::default());
    }

    #[test]
    fn refusals_and_leaving() {
        // Nothing offered: the moves say so, the drop asks for nothing (the core then refuses it).
        let mut t = XdndTarget::default();
        t.enter(&[SOURCE, 0, 0, 0, 0], Vec::new(), OWN);
        assert_eq!(
            t.moved(1, 1, COPY),
            (
                vec![Event::DropBegin { x: 1.0, y: 1.0, mime_types: Vec::new() }, Event::DropMove { x: 1.0, y: 1.0 }],
                0,
                NONE
            )
        );
        assert_eq!(t.drop_time(&[SOURCE, 0, 0, 0, 0]), Some(None));
        assert_eq!(t.dropped(false), [Event::DropComplete { x: 1.0, y: 1.0 }]);
        // Left before a move: nothing to tell; after one: complete (with nothing: the core tells the app it left).
        t.enter(&[SOURCE, 0, 100, 0, 0], named(&[100]), OWN);
        assert!(t.leave(&[42, 0, 0, 0, 0]).is_empty());
        assert!(t.leave(&[SOURCE, 0, 0, 0, 0]).is_empty());
        t.enter(&[SOURCE, 0, 100, 0, 0], named(&[100]), OWN);
        t.moved(1, 1, COPY);
        assert_eq!(t.leave(&[SOURCE, 0, 0, 0, 0]), [Event::DropComplete { x: 0.0, y: 0.0 }]);
        assert_eq!(t.source(), 0);
        // Any type at all is taken: what a target takes is the app's to say.
        t.enter(&[SOURCE, 0, 555, 0, 0], named(&[555]), OWN);
        assert_eq!(t.moved(2, 2, COPY).1, STATUS_ACCEPT);
        assert_eq!(t.dropped(true)[0], Event::DropData { x: 2.0, y: 2.0 });
    }

    #[test]
    fn the_apps_own_drag_is_taken_as_asked_and_dropped_as_its_own() {
        const MOVE: Atom = 201;
        let mut t = XdndTarget::default();
        t.enter(&[OWN as c_long, 5 << 24, 555, 0, 0], named(&[555]), OWN);
        assert!(t.is_own());
        let (_, flags, action) = t.moved(7, 8, MOVE);
        assert_eq!((flags, action), (STATUS_ACCEPT, MOVE));
        assert_eq!(t.moved(7, 9, NONE).1, 0); // (an action that isn't one: refused)
        // Nothing to read: the host has it.
        assert_eq!(t.drop_time(&[OWN as c_long, 0, 99, 0, 0]), Some(None));
        assert_eq!(t.dropped(false), [Event::DropOwn { x: 7.0, y: 9.0 }, Event::DropComplete { x: 7.0, y: 9.0 }]);
        assert!(!t.is_own());
    }
}
