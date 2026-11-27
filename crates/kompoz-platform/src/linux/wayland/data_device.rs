//! The clipboard and drops from other apps (C++'s clipboard and data device listeners), through wl_data_device: what
//! each offer has (its MIME types), what the selection or a drop holds read through a pipe from the app that offers
//! it only when it is asked for (a type at a time, once; a second at most between its writes), KompozR's own
//! clipboard offered from a data source as all its entry's types (text under each of its names, HTML, an image as
//! PNG, URIs, the app's own types: [`Payload`]) and given back from the app's own entry while it is the selection
//! (reading our own offer would wait on ourselves), and a drag over the window taken as a copy (a type it offers
//! accepted) and told to the core as SDL told it, what it offers given at the drop to be read as asked for until the
//! core says it is done — the app's own drag coming back over it ([`drag_source`](super::drag_source)) taken with
//! the actions its source allows and told as the app's own (nothing read: the host has what it carries); a drop
//! finished once the core says a target took it, else let go (its source then hears it was refused).

use super::drag_source::actions_mask;
use super::ffi::{Connection, Object, ObjectId};
use super::generated::*;
use super::{Shared, WaylandPlatform};
use crate::app::Event;
use crate::linux::sys::{self, PollFd};
use crate::linux::transfer::{Payload, WAYLAND_TEXT_TYPES};
use kompoz_ui::clipboard::{ClipDataReader, ClipEntry, ClipItem, ClipMetadata};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::rc::{Rc, Weak};
use std::time::Duration;

type Action = WlDataDeviceManagerDndAction;

/// What a drop is accepted as, the best first (else the first type it offers).
const DROP_TYPES: &[&str] = &["text/uri-list", "text/plain;charset=utf-8", "text/plain"];

/// The first of `wanted` that `types` has.
pub(super) fn offered<'a>(types: &[String], wanted: &[&'a str]) -> Option<&'a str> {
    wanted.iter().copied().find(|w| types.iter().any(|t| t == w))
}

/// An offer, the MIME types it has, the action the compositor chose for it as a drop (none yet).
pub(crate) struct Offer {
    offer: WlDataOffer,
    types: Vec<String>,
    action: WlDataDeviceManagerDndAction,
    /// A type was accepted (a drop of it can be finished).
    accepted: bool,
    /// Which offer it is (the compositor may give a later one the same id once this one is destroyed).
    serial: u64,
}

/// The clipboard, as the platform and the host's callbacks share it.
#[derive(Default)]
pub(crate) struct Clipboard {
    pub manager: Option<WlDataDeviceManager>,
    pub device: Option<WlDataDevice>,
    /// The offers the compositor made, by their ids.
    offers: HashMap<ObjectId, Offer>,
    /// The one on the clipboard (another app's or our own source's).
    selection: Option<ObjectId>,
    /// KompozR's own selection, and what it offers.
    source: Option<WlDataSource>,
    payload: Option<Payload>,
    /// The serial of the last input (what the selection is set with).
    pub serial: u32,
    /// The serial the next offer gets.
    next_offer: u64,
}

impl Clipboard {
    /// The device made for `seat` once there are both.
    pub fn attach(&mut self, seat: &WlSeat) {
        if self.device.is_none()
            && let Some(manager) = &self.manager
        {
            self.device = manager.get_data_device(seat);
        }
    }

    /// What the clipboard holds (C++'s readClipboard, all of it): ours as the app put it there; another app's as
    /// the types it offers, read (through `shared`) as they are asked for while it is the selection.
    pub fn entry(&self, shared: &Weak<Shared>) -> Option<ClipEntry> {
        if let Some(payload) = &self.payload {
            return Some(payload.entry().clone());
        }
        let id = self.selection?;
        let offer = self.offers.get(&id)?;
        offer_entry(shared, id, offer)
    }

    /// `entry` on the clipboard (C++'s writeClipboard, all its types): offered from a source of ours; none: the
    /// clipboard cleared.
    pub fn write(&mut self, entry: Option<ClipEntry>) {
        let (Some(manager), Some(device)) = (&self.manager, &self.device) else { return };
        let Some(entry) = entry else {
            device.set_selection(None, self.serial);
            self.source = None; // (destroyed)
            self.payload = None;
            return;
        };
        let Some(source) = manager.create_data_source() else { return };
        let payload = Payload::of_entry(&entry);
        for mime in payload.offered(WAYLAND_TEXT_TYPES) {
            source.offer(&mime);
        }
        device.set_selection(Some(&source), self.serial);
        self.payload = Some(payload);
        self.source = Some(source); // (the one before destroyed: it is no longer the selection)
    }

    fn forget(&mut self, id: ObjectId) {
        self.offers.remove(&id); // (destroyed)
        if self.selection == Some(id) {
            self.selection = None;
        }
    }
}

/// What `offer` holds, its types read through `shared` while it is there (another app's clipboard, or a drop).
fn offer_entry(shared: &Weak<Shared>, id: ObjectId, offer: &Offer) -> Option<ClipEntry> {
    if offer.types.is_empty() {
        return None;
    }
    let reader = Rc::new(OfferReader { shared: shared.clone(), id, serial: offer.serial });
    let item = ClipItem::from_reader(offer.types.clone(), reader);
    Some(ClipEntry::with_metadata(vec![item], ClipMetadata::of_offered(&offer.types)))
}

/// Reads one of an offer's types while the offer is there (the selection, or a drop until the core is done with
/// it).
struct OfferReader {
    shared: Weak<Shared>,
    id: ObjectId,
    serial: u64,
}

impl ClipDataReader for OfferReader {
    fn read(&self, mime_type: &str) -> Option<Vec<u8>> {
        let shared = self.shared.upgrade()?;
        let clipboard = shared.clipboard.borrow();
        let offer = clipboard.offers.get(&self.id).filter(|o| o.serial == self.serial)?;
        if !offer.types.iter().any(|t| t == mime_type) {
            return None;
        }
        receive(&shared.connection, &offer.offer, mime_type)
    }
}

/// What `offer` holds as `mime`, read through a pipe from the app offering it (which may take its time: a second at
/// most between its writes); none if it can't be read.
fn receive(connection: &Connection, offer: &WlDataOffer, mime: &str) -> Option<Vec<u8>> {
    let (read, write) = sys::pipe(false).ok()?;
    offer.receive(mime, write.as_fd());
    drop(write); // (ours closed: the end comes when the other app closes its own)
    connection.flush().ok()?;
    let mut file = std::fs::File::from(read);
    let mut out = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        let mut fd = [PollFd::readable(file.as_raw_fd())];
        if sys::poll(&mut fd, Some(Duration::from_secs(1))).ok()? == 0 {
            break; // (an app that doesn't answer)
        }
        match file.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => out.extend_from_slice(&buffer[..n]),
        }
    }
    Some(out)
}

/// `bytes` written to `fd` (who asked for our selection, or for what our drag carries), on a thread of its own: a
/// slow reader never holds the UI.
pub(super) fn send(fd: OwnedFd, bytes: Vec<u8>) {
    let spawned = std::thread::Builder::new().name("kompoz-clipboard".into()).spawn(move || {
        let _ = sys::set_nonblocking(fd.as_fd(), false);
        let _ = std::fs::File::from(fd).write_all(&bytes);
    });
    if spawned.is_err() {
        crate::system::log("kompoz: Wayland: can't send the clipboard");
    }
}

impl WaylandPlatform {
    /// An offer's MIME types (C++'s offer listener).
    pub(super) fn on_data_offer(&mut self, sender: ObjectId, e: WlDataOfferEvent) {
        let mut clipboard = self.shared.clipboard.borrow_mut();
        let Some(offer) = clipboard.offers.get_mut(&sender) else { return };
        match e {
            WlDataOfferEvent::Offer { mime_type } => offer.types.push(mime_type),
            WlDataOfferEvent::Action { dnd_action } => offer.action = dnd_action,
            WlDataOfferEvent::SourceActions { .. } => {}
        }
    }

    /// Our selection's events (C++'s source listener): what it offers sent as each type is asked for; let go when
    /// another's replaces it.
    pub(super) fn on_data_source(&mut self, sender: ObjectId, e: WlDataSourceEvent) {
        let mut clipboard = self.shared.clipboard.borrow_mut();
        if !clipboard.source.as_ref().is_some_and(|s| s.id() == sender) {
            return;
        }
        match e {
            WlDataSourceEvent::Send { mime_type, fd } => {
                let bytes = clipboard.payload.as_ref().and_then(|p| p.bytes(&mime_type, WAYLAND_TEXT_TYPES));
                send(fd, bytes.map(|b| b.to_vec()).unwrap_or_default());
            }
            WlDataSourceEvent::Cancelled => {
                clipboard.source = None; // (destroyed)
                clipboard.payload = None;
            }
            _ => {}
        }
    }

    /// The data device's events (C++'s data device listener): offers, the selection, drags.
    pub(super) fn on_data_device(&mut self, e: WlDataDeviceEvent) {
        match e {
            WlDataDeviceEvent::DataOffer { id } => {
                let mut clipboard = self.shared.clipboard.borrow_mut();
                clipboard.next_offer += 1;
                let serial = clipboard.next_offer;
                let offer = Offer { offer: id, types: Vec::new(), action: Action::NONE, accepted: false, serial };
                clipboard.offers.insert(offer.offer.id(), offer);
            }
            WlDataDeviceEvent::Selection { id } => {
                let mut clipboard = self.shared.clipboard.borrow_mut();
                let dragged = [self.drag, self.dropped];
                if let Some(old) = clipboard.selection.filter(|&old| Some(old) != id && !dragged.contains(&Some(old))) {
                    clipboard.forget(old);
                }
                clipboard.selection = id;
            }
            // Files from a file manager, text or an image from a browser: taken as a copy (a type it offers accepted,
            // files and text first), told to the core as SDL told them (begin with what it offers, moves, what it
            // carries at the drop, complete; complete alone: it left). The app's own drag (the one drag of the seat,
            // ours while it runs): any of its types, the actions its source allows (Move preferred, as within the app;
            // the compositor's modifiers choose another).
            WlDataDeviceEvent::Enter { serial, surface, x, y, id } => {
                if let Some(old) = self.drag.take() {
                    self.forget_offer(old);
                }
                let on_window = self.win.window.as_ref().is_some_and(|w| w.surface.id() == surface);
                let Some(id) = id else { return };
                if !on_window {
                    self.forget_offer(id); // (over another surface: not the window's to take)
                    return;
                }
                self.drag = Some(id);
                self.drag_own = self.outgoing.is_some();
                (self.drag_x, self.drag_y) = (x, y);
                let own_actions = self.outgoing.as_ref().map(|o| actions_mask(&o.state.payload().actions));
                let mut mime_types = Vec::new();
                if let Some(offer) = self.shared.clipboard.borrow_mut().offers.get_mut(&id) {
                    mime_types = ClipMetadata::of_offered(&offer.types).mime_types().to_vec();
                    let mime = match own_actions {
                        Some(_) => offer.types.first().map(String::as_str),
                        None => offered(&offer.types, DROP_TYPES).or(offer.types.first().map(String::as_str)),
                    };
                    offer.offer.accept(serial, mime);
                    offer.accepted = mime.is_some();
                    if offer.offer.version() >= 3 {
                        let (actions, preferred) = match own_actions {
                            Some(mask) if mask.contains(Action::MOVE) => (mask, Action::MOVE),
                            Some(mask) => (mask, Action::COPY),
                            None => (Action::COPY, Action::COPY),
                        };
                        offer.offer.set_actions(actions, preferred);
                    }
                }
                let (x, y) = (x as f32, y as f32);
                self.win.events.push_back(Event::DropBegin { x, y, mime_types });
                self.win.events.push_back(Event::DropMove { x, y });
            }
            WlDataDeviceEvent::Leave => {
                let Some(id) = self.drag.take() else { return };
                self.forget_offer(id);
                let (x, y) = (self.drag_x as f32, self.drag_y as f32);
                self.win.events.push_back(Event::DropComplete { x, y });
            }
            WlDataDeviceEvent::Motion { x, y, .. } => {
                if self.drag.is_none() {
                    return;
                }
                (self.drag_x, self.drag_y) = (x, y);
                self.win.events.push_back(Event::DropMove { x: x as f32, y: y as f32 });
            }
            // Dropped: what it carries given to be read as it is asked for (the app's own: nothing to read, ours would
            // wait on ourselves), finished or let go once the core says whether a target took it (`finish_drop`).
            WlDataDeviceEvent::Drop => {
                let Some(id) = self.drag.take() else { return };
                let (x, y) = (self.drag_x as f32, self.drag_y as f32);
                if let Some(old) = self.dropped.replace(id) {
                    self.forget_offer(old);
                }
                if std::mem::take(&mut self.drag_own) {
                    self.win.events.push_back(Event::DropOwn { x, y });
                } else {
                    let weak = Rc::downgrade(&self.shared);
                    self.dropped_entry =
                        self.shared.clipboard.borrow().offers.get(&id).and_then(|o| offer_entry(&weak, id, o));
                    if self.dropped_entry.is_some() {
                        self.win.events.push_back(Event::DropData { x, y });
                    }
                }
                self.win.events.push_back(Event::DropComplete { x, y });
            }
        }
    }

    /// The core handled the drop: finished if a target took it (with a type accepted and an action chosen, as the
    /// protocol wants: its source hears `dnd_finished`), else let go (its source hears it was cancelled).
    pub(super) fn finish_drop(&mut self, taken: bool) {
        let Some(id) = self.dropped.take() else { return };
        if taken
            && let Some(offer) = self.shared.clipboard.borrow().offers.get(&id)
            && offer.offer.version() >= 3
            && offer.accepted
            && offer.action != Action::NONE
            && offer.action != Action::ASK
        {
            offer.offer.finish();
        }
        self.forget_offer(id);
        let _ = self.connection.flush();
    }

    /// An offer done with (destroyed), unless it is still the selection.
    fn forget_offer(&mut self, id: ObjectId) {
        let mut clipboard = self.shared.clipboard.borrow_mut();
        if clipboard.selection != Some(id) {
            clipboard.forget(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_best_type_offered_is_taken() {
        const TEXT_TYPES: &[&str] = &["text/plain;charset=utf-8", "text/plain", "UTF8_STRING", "STRING", "TEXT"];
        let types: Vec<String> = ["UTF8_STRING", "text/plain", "text/uri-list"].map(String::from).into();
        assert_eq!(offered(&types, TEXT_TYPES), Some("text/plain"));
        assert_eq!(offered(&types, DROP_TYPES), Some("text/uri-list"));
        assert_eq!(offered(&types[..1], DROP_TYPES), None);
        assert_eq!(offered(&[], TEXT_TYPES), None);
    }
}
