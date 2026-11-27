//! A drag of the app's own out to other apps on X11 (WP5b), the platform's part of the XDND source
//! ([`XdndSource`]): the pointer (a mouse's master device, grabbed for the window with the drag's cursor; or the
//! finger whose touch sequence is the window's) and the keyboard (grabbed: Escape calls it off, Shift and Ctrl change
//! the action asked for, as within the app: Move, Ctrl Copy, Ctrl+Shift Link) followed until the release; the
//! target under the pointer found as XDND has it (the top-level window there and down from it the first with
//! `XdndAware` of version 3 or later, or an `XdndProxy` pointing to itself); `XdndSelection` owned by the window,
//! its requests answered with what the drag carries ([`Payload`]: its targets, each type as its bytes, in INCR
//! parts when large); the cursor of the action the target takes (the theme's `dnd-copy`, `dnd-move`, `dnd-link`,
//! `dnd-no-drop`); the app's events meanwhile held back (it gets no pointer events during the platform's drag), its
//! own window a target like any other (the drag comes back to it through the receiving side, as the app's own).
//!
//! The icon: XDND has none; KompozR does what GTK and Qt do (and what Compose desktop asks AWT for: the decoration
//! rendered as the drag's image), an override-redirect window of type `_NET_WM_WINDOW_TYPE_DND` following the
//! pointer, the decoration in it as the app drew it under the pointer (faded) — of the screen's ARGB visual where a
//! compositing manager runs, else of the default visual with the decoration's colors and its outline (alpha half or
//! more) as its shape — taking no input (an empty input shape: the target under it is found through it; without
//! libXext's shapes the window tree is walked past it). One scale: X11 has one for the screen (Xft.dpi).
//!
//! [`Payload`]: crate::linux::transfer::Payload

use super::X11Platform;
use super::dnd_source::{
    ActionAtoms, DragCursor, DropTarget, Message, MessageKind, XDND_OLDEST, XDND_VERSION, XdndSource,
};
use super::ffi::{self, Atom, Cursor, KeySym, NONE, Time, VisualWindow, Window, XiDevice, XiEvent};
use super::selection::{Transfers, part_size};
use crate::app::Event;
use crate::linux::sys;
use crate::linux::transfer::{Payload, X11_TEXT_TYPES, premultiplied_argb};
use kompoz_ui::draganddrop::PlatformDragAndDrop;
use std::ffi::{CString, c_long};

/// Escape's, the Shifts' and the Ctrls' keysyms.
const XK_ESCAPE: KeySym = 0xff1b;
const XK_SHIFT_L: u64 = 0xffe1;
const XK_SHIFT_R: u64 = 0xffe2;
const XK_CONTROL_L: KeySym = 0xffe3;
const XK_CONTROL_R: KeySym = 0xffe4;
/// `XA_WINDOW`.
const XA_WINDOW: Atom = 33;

/// What the drag follows: the mouse (its master pointer, grabbed), or a finger (its touch sequence, the window's).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Follow {
    Mouse { device: i32 },
    Touch { device: i32, touch: i32 },
}

/// Where the pointer (or a finger) last was, from XInput 2's events: what a drag starting now follows.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct PointerAt {
    pub follow: Follow,
    pub root: (f64, f64),
    pub time: Time,
    /// The modifiers then (X's bits): a drag asks for its action with the keys already held.
    pub mods: u32,
}

impl PointerAt {
    /// Where a pointer event says it is.
    pub fn of(d: &XiDevice) -> Option<PointerAt> {
        let follow = match d.evtype {
            ffi::XI_MOTION | ffi::XI_BUTTON_PRESS | ffi::XI_BUTTON_RELEASE => {
                if d.flags & ffi::XI_POINTER_EMULATED != 0 {
                    return None; // (a finger's pointer: the touch's own events are followed)
                }
                Follow::Mouse { device: d.device }
            }
            ffi::XI_TOUCH_BEGIN | ffi::XI_TOUCH_UPDATE | ffi::XI_TOUCH_END => {
                Follow::Touch { device: d.device, touch: d.detail }
            }
            _ => return None,
        };
        Some(PointerAt { follow, root: (d.root_x, d.root_y), time: d.time, mods: d.mods })
    }
}

/// The modifiers' bits a drag's action is asked with (Shift, Ctrl), as a key's press or release leaves them.
pub(super) fn modifiers_after(state: u32, keysym: KeySym, press: bool) -> u32 {
    let bit = match keysym {
        XK_SHIFT_L | XK_SHIFT_R => ffi::SHIFT_MASK,
        XK_CONTROL_L | XK_CONTROL_R => ffi::CONTROL_MASK,
        _ => 0,
    };
    let state = state & (ffi::SHIFT_MASK | ffi::CONTROL_MASK);
    if press { state | bit } else { state & !bit }
}

/// The icon's pixels as the window shows them: premultiplied for a 32-bit window (composited); else the colors
/// alone and its outline (alpha half or more), as a bitmap of whole-byte rows, least significant bit first.
pub(super) fn icon_pixels(argb: &[u32], width: u32, height: u32, composited: bool) -> (Vec<u32>, Option<Vec<u8>>) {
    if composited {
        return (argb.to_vec(), None);
    }
    let row = width.div_ceil(8) as usize;
    let mut mask = vec![0u8; row * height as usize];
    let mut colors = Vec::with_capacity(argb.len());
    for (i, &p) in argb.iter().enumerate() {
        let a = p >> 24;
        let straight = |c: u32| (c * 255 + a / 2).checked_div(a).map_or(0, |v| v.min(255));
        colors.push((straight((p >> 16) & 0xff) << 16) | (straight((p >> 8) & 0xff) << 8) | straight(p & 0xff));
        if a >= 128 {
            let (x, y) = (i % width as usize, i / width as usize);
            mask[y * row + x / 8] |= 1 << (x % 8);
        }
    }
    (colors, Some(mask))
}

/// The drag's icon: its window, its size and where the pointer is in it.
struct Icon {
    window: VisualWindow,
    hotspot: (i32, i32),
}

/// A drag of the app's own on X11.
pub(super) struct OutgoingDrag {
    source: XdndSource,
    payload: Payload,
    /// The targets offered (each type's atom and name), the best first.
    targets: Vec<(Atom, String)>,
    actions: ActionAtoms,
    icon: Option<Icon>,
    follow: Follow,
    /// Shift and Ctrl held (X's bits).
    modifiers: u32,
    /// The cursor shown, and those made (freed at the end).
    cursor: DragCursor,
    cursors: Vec<(DragCursor, Cursor)>,
    /// The last place (on the screen) and time.
    at: (i32, i32, Time),
    /// The target found under the last top-level window (found again only when it changes).
    found: Option<(Window, Option<DropTarget>)>,
    /// The data's transfers in parts (INCR).
    transfers: Transfers,
}

impl OutgoingDrag {
    /// Whether the pointer still drags it (its events and the keys the drag's).
    pub fn is_dragging(&self) -> bool {
        self.source.is_dragging()
    }
}

impl X11Platform {
    /// The drag called off (the window closing): its target told it left, the drag over untaken.
    pub(super) fn cancel_outgoing_drag(&mut self) {
        let Some(o) = &mut self.outgoing else { return };
        let (messages, _) = o.source.cancel();
        self.send_drag_messages(messages);
        self.end_outgoing_drag(None);
    }

    /// The action the user asks for with the modifiers held (Move, Ctrl Copy, Ctrl+Shift Link, among those allowed).
    fn drag_action(o: &OutgoingDrag) -> Atom {
        let ctrl = o.modifiers & ffi::CONTROL_MASK != 0;
        let shift = o.modifiers & ffi::SHIFT_MASK != 0;
        o.payload.user_action(ctrl, shift).map_or(NONE, |a| o.actions.atom(a))
    }

    /// A drag of the app's own the host asked the platform to take (its callbacks queued it): `XdndSelection` owned,
    /// the types offered, the pointer and the keyboard grabbed, the icon shown, the first move told; where it can't
    /// be, ended untaken at once.
    pub(super) fn start_outgoing_drag(&mut self) {
        let Some(drag) = self.shared.drag_request.borrow_mut().take() else { return };
        if self.outgoing.is_some() {
            return;
        }
        let window = self.window();
        let (Some(pointer), true) = (self.pointer, window != NONE) else {
            self.win.events.push_back(Event::OwnDragEnded { action: None });
            return;
        };
        let d = self.shared.display.clone();
        let a = self.shared.atoms;
        let time = pointer.time;
        d.set_selection_owner(a.XdndSelection, window, time);
        if d.selection_owner(a.XdndSelection) != window {
            self.win.events.push_back(Event::OwnDragEnded { action: None });
            return;
        }
        let payload = Payload::new(&drag.transfer_data);
        let names = payload.offered(X11_TEXT_TYPES);
        let cnames: Vec<CString> = names.iter().filter_map(|n| CString::new(n.as_str()).ok()).collect();
        let refs: Vec<&std::ffi::CStr> = cnames.iter().map(|c| c.as_c_str()).collect();
        let atoms = d.intern_atoms(&refs);
        let targets: Vec<(Atom, String)> =
            atoms.into_iter().zip(cnames.iter().map(|c| c.to_string_lossy().into_owned())).collect();
        let actions = ActionAtoms { copy: a.XdndActionCopy, move_: a.XdndActionMove, link: a.XdndActionLink };
        let source = XdndSource::new(window, targets.iter().map(|t| t.0).collect(), actions);
        if source.needs_type_list() {
            let list: Vec<c_long> = targets.iter().map(|t| t.0 as c_long).collect();
            d.set_property_32(window, a.XdndTypeList, ffi::XA_ATOM, &list);
        }
        let (x, y) = (pointer.root.0.round() as i32, pointer.root.1.round() as i32);
        let icon = self.make_drag_icon(&drag, x, y);
        let mut o = OutgoingDrag {
            source,
            payload,
            targets,
            actions,
            icon,
            follow: pointer.follow,
            modifiers: pointer.mods & (ffi::SHIFT_MASK | ffi::CONTROL_MASK),
            cursor: DragCursor::NoDrop,
            cursors: Vec::new(),
            at: (x, y, time),
            found: None,
            transfers: Transfers::default(),
        };
        let cursor = self.drag_cursor(&mut o, DragCursor::NoDrop);
        if let Follow::Mouse { device } = o.follow {
            d.xi_grab_device(
                device,
                window,
                time,
                cursor,
                &[ffi::XI_MOTION, ffi::XI_BUTTON_PRESS, ffi::XI_BUTTON_RELEASE],
            );
        }
        d.grab_keyboard(window, time);
        self.outgoing = Some(o);
        self.shared.dragging.set(true);
        self.drag_moved(x, y, time);
    }

    /// The icon's window at (x, y) less the pointer's place in the decoration, shown; none if nothing draws it.
    fn make_drag_icon(&self, drag: &PlatformDragAndDrop, x: i32, y: i32) -> Option<Icon> {
        let image = drag.render_icon(1.0)?;
        let (w, h) = (image.width().max(1) as u32, image.height().max(1) as u32);
        let d = &self.shared.display;
        let a = self.shared.atoms;
        let cm = CString::new(format!("_NET_WM_CM_S{}", d.screen())).ok()?;
        let composited = d.intern_atoms(&[cm.as_c_str()]).first().is_some_and(|&s| d.selection_owner(s) != NONE);
        let hotspot = (drag.decoration_offset.x.round() as i32, drag.decoration_offset.y.round() as i32);
        let window = d.create_visual_window(x - hotspot.0, y - hotspot.1, w, h, composited)?;
        let argb = premultiplied_argb(&image, w as i32, h as i32);
        let (pixels, outline) = icon_pixels(&argb, w, h, window.depth == 32);
        d.set_window_pixels(&window, w, h, &pixels);
        if let Some(mask) = outline {
            d.set_outline(window.window, w, h, &mask);
        }
        d.set_no_input(window.window);
        d.set_property_32(window.window, a._NET_WM_WINDOW_TYPE, ffi::XA_ATOM, &[a._NET_WM_WINDOW_TYPE_DND as c_long]);
        d.map_raised(window.window);
        Some(Icon { window, hotspot })
    }

    /// The cursor for `kind`: the theme's (made once for the drag), else X's own.
    fn drag_cursor(&self, o: &mut OutgoingDrag, kind: DragCursor) -> Cursor {
        if let Some(&(_, c)) = o.cursors.iter().find(|(k, _)| *k == kind) {
            return c;
        }
        let d = &self.shared.display;
        let (names, shape): (&[&std::ffi::CStr], u32) = match kind {
            DragCursor::Copy => (&[c"dnd-copy", c"copy"], ffi::XC_LEFT_PTR),
            DragCursor::Move => (&[c"dnd-move", c"grabbing"], ffi::XC_LEFT_PTR),
            DragCursor::Link => (&[c"dnd-link", c"alias"], ffi::XC_LEFT_PTR),
            DragCursor::NoDrop => (&[c"dnd-no-drop", c"no-drop", c"dnd-none"], ffi::XC_CIRCLE),
        };
        let cursor = names.iter().find_map(|n| d.theme_cursor(n)).unwrap_or_else(|| d.font_cursor(shape));
        o.cursors.push((kind, cursor));
        cursor
    }

    /// The XDND target under (x, y) on the screen: the top-level window there (past the drag's icon), then down
    /// from it the first window with `XdndAware` (or a proxy that has it).
    fn find_drop_target(&mut self, x: i32, y: i32) -> Option<DropTarget> {
        let d = self.shared.display.clone();
        let root = d.root();
        let icon = self.outgoing.as_ref().and_then(|o| o.icon.as_ref()).map(|i| i.window.window);
        let mut top = d.child_at(root, root, x, y).map_or(NONE, |(_, _, c)| c);
        if top != NONE && Some(top) == icon {
            // (No input shape on the icon: the windows under it, top first.)
            top = d
                .children(root)
                .into_iter()
                .rev()
                .filter(|&w| Some(w) != icon)
                .find(|&w| d.geometry(w).is_some_and(|g| g.viewable && g.contains(x, y)))
                .unwrap_or(NONE);
        }
        if top == NONE {
            return None;
        }
        if let Some((w, found)) = self.outgoing.as_ref().and_then(|o| o.found)
            && w == top
        {
            return found;
        }
        let mut w = top;
        let mut found = None;
        for _ in 0..32 {
            if let Some(t) = self.xdnd_aware(w) {
                found = Some(t);
                break;
            }
            match d.child_at(root, w, x, y) {
                Some((_, _, child)) if child != NONE => w = child,
                _ => break,
            }
        }
        if let Some(o) = &mut self.outgoing {
            o.found = Some((top, found));
        }
        found
    }

    /// `w` as an XDND target: its proxy (an `XdndProxy` pointing to itself) or itself, with `XdndAware` of a version
    /// KompozR speaks with.
    fn xdnd_aware(&self, w: Window) -> Option<DropTarget> {
        let d = &self.shared.display;
        let a = self.shared.atoms;
        let window_of = |w: Window| {
            d.property(w, a.XdndProxy, false).filter(|p| p.kind == XA_WINDOW).and_then(|p| p.longs().first().copied())
        };
        let proxy = window_of(w).map(|p| p as Window).filter(|&p| p != NONE && window_of(p) == Some(p as c_long));
        let aware = proxy.unwrap_or(w);
        let version = d.property(aware, a.XdndAware, false)?.longs().first().copied()? as u32;
        (version >= XDND_OLDEST).then_some(DropTarget { window: w, proxy: aware, version: version.min(XDND_VERSION) })
    }

    /// The drag's messages sent (each to its target's proxy, about it).
    fn send_drag_messages(&self, messages: Vec<Message>) {
        let a = self.shared.atoms;
        let d = &self.shared.display;
        for m in messages {
            let kind = match m.kind {
                MessageKind::Enter => a.XdndEnter,
                MessageKind::Position => a.XdndPosition,
                MessageKind::Leave => a.XdndLeave,
                MessageKind::Drop => a.XdndDrop,
            };
            d.send_client_message(m.to, m.window, kind, m.data, ffi::NO_EVENT_MASK);
        }
        d.flush();
    }

    /// The pointer at (x, y) on the screen at `time`: the icon there, the target under it told.
    fn drag_moved(&mut self, x: i32, y: i32, time: Time) {
        let target = self.find_drop_target(x, y);
        let Some(o) = &mut self.outgoing else { return };
        o.at = (x, y, time);
        if let Some(icon) = &o.icon {
            self.shared.display.move_window(icon.window.window, x - icon.hotspot.0, y - icon.hotspot.1);
        }
        let action = Self::drag_action(o);
        let messages = o.source.moved(target, x, y, time, action);
        self.send_drag_messages(messages);
        self.update_drag_cursor();
    }

    /// The cursor of what the target under the pointer does with a drop (the grab's: a mouse's drag).
    fn update_drag_cursor(&mut self) {
        let window = self.window();
        let Some(mut o) = self.outgoing.take() else { return };
        let kind = o.source.cursor();
        if kind != o.cursor {
            o.cursor = kind;
            let cursor = self.drag_cursor(&mut o, kind);
            if let Follow::Mouse { device } = o.follow {
                self.shared.display.xi_grab_device(
                    device,
                    window,
                    o.at.2,
                    cursor,
                    &[ffi::XI_MOTION, ffi::XI_BUTTON_PRESS, ffi::XI_BUTTON_RELEASE],
                );
            }
        }
        self.outgoing = Some(o);
    }

    /// XInput 2's events while the drag runs: the pointer (or the finger) it follows moving and let go; nothing of
    /// them goes to the app.
    pub(super) fn drag_input(&mut self, e: &XiEvent) {
        let XiEvent::Device(d) = e else { return };
        let Some(o) = &self.outgoing else { return };
        let ours = match (o.follow, d.evtype) {
            (Follow::Mouse { device }, ffi::XI_MOTION | ffi::XI_BUTTON_RELEASE) => {
                d.device == device && d.flags & ffi::XI_POINTER_EMULATED == 0
            }
            (Follow::Touch { touch, .. }, ffi::XI_TOUCH_UPDATE | ffi::XI_TOUCH_END) => d.detail == touch,
            _ => false,
        };
        if !ours {
            return;
        }
        let (x, y) = (d.root_x.round() as i32, d.root_y.round() as i32);
        match d.evtype {
            ffi::XI_MOTION | ffi::XI_TOUCH_UPDATE => self.drag_moved(x, y, d.time),
            _ => {
                if (x, y) != (o.at.0, o.at.1) {
                    self.drag_moved(x, y, d.time);
                }
                let Some(o) = &mut self.outgoing else { return };
                let (messages, ended) = o.source.released(d.time, sys::monotonic_now());
                self.send_drag_messages(messages);
                self.release_drag_input(d.time);
                if let Some(action) = ended {
                    self.end_outgoing_drag(action);
                }
            }
        }
    }

    /// A key while the drag runs (the keyboard is grabbed): Escape calls it off (and goes no further), Shift and
    /// Ctrl change the action asked for. Whether the app is to have it.
    pub(super) fn drag_key(&mut self, keysym: KeySym, state: u32, press: bool) -> bool {
        let Some(o) = &mut self.outgoing else { return true };
        if keysym == XK_ESCAPE {
            if press {
                let (messages, ended) = o.source.cancel();
                self.send_drag_messages(messages);
                self.release_drag_input(ffi::CURRENT_TIME);
                if let Some(action) = ended {
                    self.end_outgoing_drag(action);
                }
            }
            return false;
        }
        let modifiers = modifiers_after(state, keysym, press);
        if modifiers != o.modifiers {
            o.modifiers = modifiers;
            let action = Self::drag_action(o);
            let messages = o.source.action_changed(action);
            self.send_drag_messages(messages);
        }
        true
    }

    /// The grabs let go and the icon taken away (the drag was let go or called off).
    fn release_drag_input(&mut self, time: Time) {
        let d = self.shared.display.clone();
        let Some(o) = &mut self.outgoing else { return };
        if let Follow::Mouse { device } = o.follow {
            d.xi_ungrab_device(device, time);
        }
        d.ungrab_keyboard(time);
        if let Some(icon) = o.icon.take() {
            d.destroy_visual_window(icon.window);
        }
        d.flush();
    }

    /// The drag is over (taken with `action`, or not): what it held let go, the core told.
    fn end_outgoing_drag(&mut self, action: Option<kompoz_ui::draganddrop::DragAndDropTransferAction>) {
        self.release_drag_input(ffi::CURRENT_TIME);
        let Some(o) = self.outgoing.take() else { return };
        let d = &self.shared.display;
        for (_, c) in o.cursors {
            d.free_cursor(c);
        }
        if o.source.needs_type_list() {
            d.delete_property(self.window(), self.shared.atoms.XdndTypeList);
        }
        d.flush();
        self.shared.dragging.set(false);
        self.win.events.push_back(Event::OwnDragEnded { action });
    }

    /// A target's `XdndStatus` or `XdndFinished` (whether it is the drag's).
    pub(super) fn drag_reply(&mut self, kind: Atom, data: [c_long; 5]) {
        let a = self.shared.atoms;
        let Some(o) = &mut self.outgoing else { return };
        if kind == a.XdndStatus {
            let (messages, ended) = o.source.status(&data, sys::monotonic_now());
            self.send_drag_messages(messages);
            self.update_drag_cursor();
            if let Some(action) = ended {
                self.end_outgoing_drag(action);
            }
        } else if kind == a.XdndFinished
            && let Some(action) = o.source.finished(&data)
        {
            self.end_outgoing_drag(action);
        }
    }

    /// A target that didn't answer in time: the drag over, untaken.
    pub(super) fn drag_timeouts(&mut self) {
        let Some(o) = &mut self.outgoing else { return };
        let (messages, ended) = o.source.timed_out(sys::monotonic_now());
        self.send_drag_messages(messages);
        if let Some(action) = ended {
            self.end_outgoing_drag(action);
        }
    }

    /// When the drag gives up waiting for its target, if it waits.
    pub(super) fn drag_deadline(&self) -> Option<i64> {
        self.outgoing.as_ref().and_then(|o| o.source.deadline())
    }

    /// A request for `XdndSelection` (the drag's data): its targets, or a type as its bytes (in parts when it is
    /// larger than a request carries). Whether it was the drag's.
    pub(super) fn answer_drag_selection(
        &mut self,
        requestor: Window,
        target: Atom,
        property: Atom,
        time: Time,
    ) -> bool {
        let a = self.shared.atoms;
        let d = self.shared.display.clone();
        let Some(o) = &mut self.outgoing else { return false };
        o.source.activity(sys::monotonic_now());
        let into = if property != NONE { property } else { target };
        let mut answered = NONE;
        if target == a.TARGETS {
            let mut list = vec![a.TARGETS as c_long];
            list.extend(o.targets.iter().map(|t| t.0 as c_long));
            d.set_property_32(requestor, into, ffi::XA_ATOM, &list);
            answered = into;
        } else if let Some((_, name)) = o.targets.iter().find(|t| t.0 == target)
            && let Some(bytes) = o.payload.bytes(name, X11_TEXT_TYPES)
        {
            let kind = if target == a.TEXT { a.UTF8_STRING } else { target };
            if bytes.len() > part_size(d.max_request_bytes()) {
                d.select_input(requestor, ffi::PROPERTY_CHANGE_MASK);
                d.set_property_32(requestor, into, a.INCR, &[bytes.len() as c_long]);
                o.transfers.start(requestor, into, kind, bytes);
            } else {
                d.set_property_8(requestor, into, kind, &bytes);
            }
            answered = into;
        }
        d.send_selection_notify(requestor, a.XdndSelection, target, answered, time);
        d.flush();
        true
    }

    /// A requestor took a part of the drag's data (deleted the property): the next one. Whether it was the drag's.
    pub(super) fn send_next_drag_part(&mut self, window: Window, property: Atom) -> bool {
        let d = self.shared.display.clone();
        let Some(o) = &mut self.outgoing else { return false };
        if !o.transfers.transfers().iter().any(|t| t.requestor == window && t.property == property) {
            return false;
        }
        o.source.activity(sys::monotonic_now());
        let Some((part, bytes)) = o.transfers.next_part(window, property, part_size(d.max_request_bytes())) else {
            return true;
        };
        d.set_property_8(window, property, part.kind, &bytes[part.range]);
        if part.done {
            d.select_input(window, ffi::NO_EVENT_MASK);
        }
        d.flush();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_and_ctrl_as_keys_leave_them() {
        assert_eq!(modifiers_after(0, XK_CONTROL_L, true), ffi::CONTROL_MASK);
        assert_eq!(modifiers_after(ffi::CONTROL_MASK, XK_SHIFT_R, true), ffi::CONTROL_MASK | ffi::SHIFT_MASK);
        assert_eq!(modifiers_after(ffi::CONTROL_MASK | ffi::SHIFT_MASK, XK_CONTROL_R, false), ffi::SHIFT_MASK);
        // (Other keys and other modifiers: the state as it is, Shift and Ctrl only.)
        assert_eq!(modifiers_after(ffi::MOD1_MASK | ffi::SHIFT_MASK, 0x61, true), ffi::SHIFT_MASK);
    }

    #[test]
    fn an_icon_without_a_compositor_is_its_colors_and_outline() {
        // Opaque red, half-transparent green (premultiplied), a faint blue, nothing; 10 wide (two bytes a row).
        let mut argb = vec![0u32; 10 * 2];
        argb[0] = 0xffff0000;
        argb[1] = 0x80008000;
        argb[9] = 0x10000010;
        argb[10] = 0xff0000ff;
        let (colors, mask) = icon_pixels(&argb, 10, 2, false);
        assert_eq!(&colors[..2], [0xff0000, 0x00ff00]);
        assert_eq!(colors[9], 0x0000ff);
        assert_eq!(mask.unwrap(), [0b0000_0011, 0, 0b0000_0001, 0]);
        let (same, none) = icon_pixels(&argb, 10, 2, true);
        assert_eq!((same, none), (argb, None));
    }

    #[test]
    fn pointer_events_say_what_a_drag_follows() {
        let mut d = XiDevice {
            evtype: ffi::XI_MOTION,
            time: 9,
            device: 2,
            source: 11,
            detail: 0,
            x: 1.0,
            y: 2.0,
            root_x: 101.0,
            root_y: 202.0,
            flags: 0,
            mods: 0,
            valuators: Vec::new(),
        };
        assert_eq!(
            PointerAt::of(&d),
            Some(PointerAt { follow: Follow::Mouse { device: 2 }, root: (101.0, 202.0), time: 9, mods: 0 })
        );
        d.flags = ffi::XI_POINTER_EMULATED;
        assert_eq!(PointerAt::of(&d), None);
        d.evtype = ffi::XI_TOUCH_UPDATE;
        d.detail = 7;
        assert_eq!(PointerAt::of(&d).map(|p| p.follow), Some(Follow::Touch { device: 2, touch: 7 }));
    }
}
