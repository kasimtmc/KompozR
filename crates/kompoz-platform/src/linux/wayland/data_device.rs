//! The clipboard and drops from other apps (C++'s clipboard and data device listeners), through wl_data_device: what
//! each offer has (its MIME types), the selection read through a pipe from the app that offers it (a second at most
//! between its writes), KompozR's own text offered from a data source (and given back from the cache while it is
//! the selection: reading our own offer would wait on ourselves), and a drag over the window taken as a copy (a
//! file manager's `text/uri-list`, a browser's text) and told to the core as SDL told it — the app's own drag coming
//! back over it ([`drag_source`](super::drag_source)) taken with the actions its source allows and told as the app's
//! own (nothing read: the host has what it carries); a drop finished once the core says a target took it, else let
//! go (its source then hears it was refused).

use super::WaylandPlatform;
use super::drag_source::actions_mask;
use super::ffi::{Connection, Object, ObjectId};
use super::generated::*;
use crate::app::Event;
use crate::linux::desktop::files_in_uri_list;
use crate::linux::sys::{self, PollFd};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::time::Duration;

type Action = WlDataDeviceManagerDndAction;

/// Text, as clipboards name it, the best first.
pub(super) const TEXT_TYPES: &[&str] = &["text/plain;charset=utf-8", "text/plain", "UTF8_STRING", "STRING", "TEXT"];
/// What KompozR's own selection is offered as.
const OWN_TYPES: &[&str] = &["text/plain;charset=utf-8", "text/plain", "UTF8_STRING"];
/// What a drop is taken as, the best first.
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
    /// KompozR's own selection, and its text.
    source: Option<WlDataSource>,
    text: String,
    /// The serial of the last input (what the selection is set with).
    pub serial: u32,
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

    /// The clipboard's text (C++'s readClipboard): ours from the cache; another app's read from it.
    pub fn read(&mut self, connection: &Connection) -> Option<String> {
        if self.source.is_some() {
            return Some(self.text.clone());
        }
        let offer = self.offers.get(&self.selection?)?;
        let mime = offered(&offer.types, TEXT_TYPES)?;
        receive(connection, &offer.offer, mime).map(|b| String::from_utf8_lossy(&b).into_owned())
    }

    /// `text` on the clipboard (C++'s writeClipboard): offered from a source of ours.
    pub fn write(&mut self, text: &str) {
        let (Some(manager), Some(device)) = (&self.manager, &self.device) else { return };
        let Some(source) = manager.create_data_source() else { return };
        for mime in OWN_TYPES {
            source.offer(mime);
        }
        device.set_selection(Some(&source), self.serial);
        self.text = text.to_string();
        self.source = Some(source); // (the one before destroyed: it is no longer the selection)
    }

    fn forget(&mut self, id: ObjectId) {
        self.offers.remove(&id); // (destroyed)
        if self.selection == Some(id) {
            self.selection = None;
        }
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

    /// Our selection's events (C++'s source listener): its text sent to who asks; let go when another's replaces it.
    pub(super) fn on_data_source(&mut self, sender: ObjectId, e: WlDataSourceEvent) {
        let mut clipboard = self.shared.clipboard.borrow_mut();
        if !clipboard.source.as_ref().is_some_and(|s| s.id() == sender) {
            return;
        }
        match e {
            WlDataSourceEvent::Send { fd, .. } => send(fd, clipboard.text.clone().into_bytes()),
            WlDataSourceEvent::Cancelled => {
                clipboard.source = None; // (destroyed)
                clipboard.text.clear();
            }
            _ => {}
        }
    }

    /// The data device's events (C++'s data device listener): offers, the selection, drags.
    pub(super) fn on_data_device(&mut self, e: WlDataDeviceEvent) {
        match e {
            WlDataDeviceEvent::DataOffer { id } => {
                let offer = Offer { offer: id, types: Vec::new(), action: Action::NONE, accepted: false };
                self.shared.clipboard.borrow_mut().offers.insert(offer.offer.id(), offer);
            }
            WlDataDeviceEvent::Selection { id } => {
                let mut clipboard = self.shared.clipboard.borrow_mut();
                let dragged = [self.drag, self.dropped];
                if let Some(old) = clipboard.selection.filter(|&old| Some(old) != id && !dragged.contains(&Some(old))) {
                    clipboard.forget(old);
                }
                clipboard.selection = id;
            }
            // Files from a file manager, text from a browser: taken as a copy, told to the core as SDL told them
            // (begin, moves, each file or the text at the drop, complete; complete alone: it left). The app's own drag
            // (the one drag of the seat, ours while it runs): any of its types, the actions its source allows (Move
            // preferred, as within the app; the compositor's modifiers choose another).
            WlDataDeviceEvent::Enter { serial, surface, x, y, id } => {
                if let Some(old) = self.drag.take() {
                    self.forget_offer(old);
                }
                let on_window = self.window.as_ref().is_some_and(|w| w.surface.id() == surface);
                let Some(id) = id else { return };
                if !on_window {
                    self.forget_offer(id); // (over another surface: not the window's to take)
                    return;
                }
                self.drag = Some(id);
                self.drag_own = self.outgoing.is_some();
                (self.drag_x, self.drag_y) = (x, y);
                let own_actions = self.outgoing.as_ref().map(|o| actions_mask(&o.state.payload().actions));
                if let Some(offer) = self.shared.clipboard.borrow_mut().offers.get_mut(&id) {
                    let mime = match own_actions {
                        Some(_) => offer.types.first().map(String::as_str),
                        None => offered(&offer.types, DROP_TYPES),
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
                self.events.push_back(Event::DropBegin { x, y });
                self.events.push_back(Event::DropMove { x, y });
            }
            WlDataDeviceEvent::Leave => {
                let Some(id) = self.drag.take() else { return };
                self.forget_offer(id);
                let (x, y) = (self.drag_x as f32, self.drag_y as f32);
                self.events.push_back(Event::DropComplete { x, y });
            }
            WlDataDeviceEvent::Motion { x, y, .. } => {
                if self.drag.is_none() {
                    return;
                }
                (self.drag_x, self.drag_y) = (x, y);
                self.events.push_back(Event::DropMove { x: x as f32, y: y as f32 });
            }
            // Dropped: what it carries read (the app's own: nothing to read, ours would wait on ourselves), finished
            // or let go once the core says whether a target took it (`finish_drop`).
            WlDataDeviceEvent::Drop => {
                let Some(id) = self.drag.take() else { return };
                let (x, y) = (self.drag_x as f32, self.drag_y as f32);
                if let Some(old) = self.dropped.replace(id) {
                    self.forget_offer(old);
                }
                if std::mem::take(&mut self.drag_own) {
                    self.events.push_back(Event::DropOwn { x, y });
                } else {
                    let clipboard = self.shared.clipboard.borrow();
                    if let Some(offer) = clipboard.offers.get(&id) {
                        if offered(&offer.types, &["text/uri-list"]).is_some() {
                            let list = receive(&self.connection, &offer.offer, "text/uri-list").unwrap_or_default();
                            for path in files_in_uri_list(&String::from_utf8_lossy(&list)) {
                                self.events.push_back(Event::DropFile { x, y, path });
                            }
                        } else if let Some(mime) = offered(&offer.types, &DROP_TYPES[1..]) {
                            let text = receive(&self.connection, &offer.offer, mime).unwrap_or_default();
                            let text = String::from_utf8_lossy(&text).into_owned();
                            self.events.push_back(Event::DropText { x, y, text });
                        }
                    }
                }
                self.events.push_back(Event::DropComplete { x, y });
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
        let types: Vec<String> = ["UTF8_STRING", "text/plain", "text/uri-list"].map(String::from).into();
        assert_eq!(offered(&types, TEXT_TYPES), Some("text/plain"));
        assert_eq!(offered(&types, DROP_TYPES), Some("text/uri-list"));
        assert_eq!(offered(&types[..1], DROP_TYPES), None);
        assert_eq!(offered(&[], TEXT_TYPES), None);
    }
}
