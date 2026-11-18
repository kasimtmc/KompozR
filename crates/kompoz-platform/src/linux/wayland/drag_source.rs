//! Drags of the app's own out to other apps (WP5b; Compose's are the platform's from the start, KompozR's leave the
//! window as drags within the app): `wl_data_device.start_drag` with a `wl_data_source` of our own offering what the
//! drag carries ([`DragPayload`]: files as `text/uri-list`, text, the app's own types) and the actions its source
//! allows (copy, move: Wayland has no link), from the press that started it (the implicit grab's serial), with an
//! icon surface showing the decoration as the app drew it under the pointer, the pointer where it was in it. The
//! compositor runs the drag from there: the app gets no pointer events meanwhile; over the window it comes back as
//! a drop offer like any other (told to the core as the app's own: its data is the host's), and its end — taken
//! with the action the compositor settled on (`dnd_finished`), or not (`cancelled`: dropped on nothing, refused,
//! called off) — goes to the core as [`Event::OwnDragEnded`].
//!
//! The icon is drawn for the scale it is shown at: the window's to begin with, then its own surface's as the
//! compositor tells it (fractional-scale with a viewport, else the whole scale it prefers or of the outputs it is
//! on: a drag over a monitor of another scale draws it again). Escape, where the compositor passes it to the app,
//! calls the drag off (the source destroyed); a press let go before the compositor took the drag (it then never
//! starts: the release comes to the app, and nothing to the source by the time a `wl_display.sync` after it is
//! done) ends it untaken. (A compositor may pass a finger's lift to the app after ending the drag it started:
//! KWin does, after the source heard the drop.)
//!
//! [`DragPayload`]: crate::linux::outgoing_drag::DragPayload

use super::WaylandPlatform;
use super::decorations::shm_buffer;
use super::ffi::{Object, ObjectId};
use super::generated::*;
use crate::app::Event;
use crate::linux::outgoing_drag::{DragPayload, WAYLAND_TEXT_TYPES, premultiplied_argb};
use kompoz_ui::draganddrop::{DragAndDropTransferAction, PlatformDragAndDrop};

type Action = WlDataDeviceManagerDndAction;

/// The data device's actions for what a drag's source allows (Wayland has no link: a drag allowing only that can't
/// leave the window).
pub(super) fn actions_mask(actions: &[DragAndDropTransferAction]) -> Action {
    let mut mask = Action::NONE;
    for &a in actions {
        if a == DragAndDropTransferAction::Copy {
            mask |= Action::COPY;
        } else if a == DragAndDropTransferAction::Move {
            mask |= Action::MOVE;
        }
    }
    mask
}

/// The action the compositor settled on, as KompozR's (none for none, or an "ask" never settled).
fn transfer_action(action: Action) -> Option<DragAndDropTransferAction> {
    match action {
        Action::COPY => Some(DragAndDropTransferAction::Copy),
        Action::MOVE => Some(DragAndDropTransferAction::Move),
        _ => None,
    }
}

/// The data source's events, as the drag's state takes them (the protocol's, the send's fd aside).
#[derive(Clone, PartialEq, Debug)]
pub(super) enum SourceEvent {
    Target(Option<String>),
    Send(String),
    Cancelled,
    DropPerformed,
    Finished,
    Action(Action),
}

/// What a source event asks of the platform.
#[derive(Clone, PartialEq, Debug)]
pub(super) enum SourceStep {
    Nothing,
    /// Write these bytes to the send's fd.
    Send(Vec<u8>),
    /// The drag is over: taken with that action, or not.
    Ended(Option<DragAndDropTransferAction>),
}

/// A drag's source as its events leave it: what it carries, the action the compositor last chose, whether it is
/// over. (Which type a target accepts, and the drop itself, are the compositor's to judge: a drop it doesn't take
/// ends cancelled; the source keeps serving until it is finished.)
#[derive(Debug)]
pub(super) struct SourceState {
    payload: DragPayload,
    action: Action,
    /// The compositor told the source something (the drag runs: it started).
    heard: bool,
    ended: bool,
}

impl SourceState {
    pub fn new(payload: DragPayload) -> SourceState {
        SourceState { payload, action: Action::NONE, heard: false, ended: false }
    }

    /// Whether the compositor told the source anything yet (a drag it runs: a target's type or action, the drop,
    /// the end, a request for the data).
    pub fn heard(&self) -> bool {
        self.heard
    }

    /// What it carries.
    pub fn payload(&self) -> &DragPayload {
        &self.payload
    }

    /// An event of its data source: a type asked for answered with its bytes (an empty answer for a type it isn't:
    /// the asker gets nothing rather than waiting), the end told once — `dnd_finished` with the last action chosen,
    /// `cancelled` untaken.
    pub fn event(&mut self, e: SourceEvent) -> SourceStep {
        if self.ended {
            return SourceStep::Nothing;
        }
        self.heard = true;
        match e {
            SourceEvent::Target(_) | SourceEvent::DropPerformed => {}
            SourceEvent::Action(action) => self.action = action,
            SourceEvent::Send(mime) => {
                return SourceStep::Send(self.payload.bytes(&mime, WAYLAND_TEXT_TYPES).unwrap_or_default());
            }
            SourceEvent::Finished => {
                self.ended = true;
                return SourceStep::Ended(transfer_action(self.action));
            }
            SourceEvent::Cancelled => return self.cancel(),
        }
        SourceStep::Nothing
    }

    /// Called off by the app (Escape), or never started: over, untaken (once).
    pub fn cancel(&mut self) -> SourceStep {
        if std::mem::replace(&mut self.ended, true) {
            return SourceStep::Nothing;
        }
        SourceStep::Ended(None)
    }
}

/// The scale an icon is shown at: fractional-scale's (120ths, with a viewport), else a whole buffer scale.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum IconScale {
    Fractional(u32),
    Whole(i32),
}

/// How an icon is drawn for its scale: the decoration drawn `image_scale` times its size (px of the window) into a
/// buffer of `buffer` pixels shown at `buffer_scale`, or shown at `destination` points through a viewport.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct IconLayout {
    pub image_scale: f32,
    pub buffer: (i32, i32),
    pub buffer_scale: i32,
    pub destination: Option<(i32, i32)>,
}

/// How a decoration of `size` px (the window's: `point_to_px` of them a point) is drawn for `scale`: fractional, its
/// own pixels at that scale shown at its size in points; whole, a buffer of whole points at the scale (the
/// decoration at its top left), as the protocol wants a buffer of a scale a multiple of it.
pub(super) fn icon_layout(size: (f32, f32), point_to_px: f64, scale: IconScale) -> IconLayout {
    let points = |px: f32| px as f64 / point_to_px.max(1e-3);
    match scale {
        IconScale::Fractional(scale120) => {
            let k = scale120.max(1) as f64 / 120.0 / point_to_px.max(1e-3);
            let px = |v: f32| ((v as f64 * k).round() as i32).max(1);
            let pt = |v: f32| (points(v).round() as i32).max(1);
            IconLayout {
                image_scale: k as f32,
                buffer: (px(size.0), px(size.1)),
                buffer_scale: 1,
                destination: Some((pt(size.0), pt(size.1))),
            }
        }
        IconScale::Whole(n) => {
            let n = n.max(1);
            let pt = |v: f32| (points(v).ceil() as i32).max(1);
            IconLayout {
                image_scale: (n as f64 / point_to_px.max(1e-3)) as f32,
                buffer: (pt(size.0) * n, pt(size.1) * n),
                buffer_scale: n,
                destination: None,
            }
        }
    }
}

/// Where the icon's top left is from the pointer (the hotspot), in points: the pointer `offset` px (the window's)
/// into the decoration.
pub(super) fn icon_offset(offset: (f32, f32), point_to_px: f64) -> (i32, i32) {
    let k = 1.0 / point_to_px.max(1e-3);
    (-(offset.0 as f64 * k).round() as i32, -(offset.1 as f64 * k).round() as i32)
}

/// A buffer the icon was drawn into: let go once replaced and the compositor no longer reads it.
struct IconBuffer {
    buffer: WlBuffer,
    busy: bool,
    retired: bool,
}

/// The drag's icon: its surface (a viewport and fractional scale on it where the compositor has them), the buffers
/// drawn for it, the scale it was drawn for, the outputs it is on and the whole scale the compositor prefers for it.
/// Dropped in order: what is on the surface, then it.
struct Icon {
    viewport: Option<WpViewport>,
    fractional: Option<WpFractionalScaleV1>,
    buffers: Vec<IconBuffer>,
    surface: WlSurface,
    drawn: Option<IconScale>,
    outputs: Vec<ObjectId>,
    preferred: Option<i32>,
    /// The offset still to give it with its first buffer.
    offset: Option<(i32, i32)>,
}

/// A drag of the app's own the compositor runs: its state, its data source (destroyed with it: the compositor ends
/// a drag whose source is gone), its icon, what it was given (its decoration drawn again for another scale).
pub(super) struct OutgoingDrag {
    pub state: SourceState,
    /// The sync asked for after a release the app got (whether the drag started: its done tells).
    check: Option<WlCallback>,
    source: WlDataSource,
    icon: Option<Icon>,
    drag: PlatformDragAndDrop,
}

impl OutgoingDrag {
    /// Whether `id` is its data source.
    pub fn is_source(&self, id: ObjectId) -> bool {
        self.source.id() == id
    }

    /// Whether `id` is the sync asked for to see whether it started.
    pub fn is_check(&self, id: ObjectId) -> bool {
        self.check.as_ref().is_some_and(|c| c.id() == id)
    }

    /// Whether `id` is its icon's surface.
    pub fn is_icon(&self, id: ObjectId) -> bool {
        self.icon.as_ref().is_some_and(|i| i.surface.id() == id)
    }

    /// Whether `id` is its icon's fractional scale.
    pub fn is_icon_scale(&self, id: ObjectId) -> bool {
        self.icon.as_ref().and_then(|i| i.fractional.as_ref()).is_some_and(|f| f.id() == id)
    }
}

impl WaylandPlatform {
    /// A drag of the app's own the host asked the platform to take (its callbacks queued it): the compositor's drag
    /// started with our data source and icon; or, where it can't be, ended untaken at once.
    pub(super) fn start_outgoing_drag(&mut self) {
        let Some(drag) = self.shared.drag_request.borrow_mut().take() else { return };
        if self.outgoing.is_some() {
            return; // (one at a time: the host starts none while one is with the platform)
        }
        let payload = DragPayload::new(&drag.transfer_data);
        let mask = actions_mask(&payload.actions);
        let source = {
            let clipboard = self.shared.clipboard.borrow();
            match (&clipboard.manager, &clipboard.device, &self.window) {
                (Some(manager), Some(_), Some(_)) if manager.version() >= 3 && mask != Action::NONE => {
                    manager.create_data_source()
                }
                _ => None,
            }
        };
        let Some(source) = source else {
            self.events.push_back(Event::OwnDragEnded { action: None });
            return;
        };
        for mime in payload.offered(WAYLAND_TEXT_TYPES) {
            source.offer(&mime);
        }
        source.set_actions(mask);
        let icon = self.make_icon(&drag);
        {
            let clipboard = self.shared.clipboard.borrow();
            let (Some(device), Some(w)) = (&clipboard.device, &self.window) else { return };
            device.start_drag(Some(&source), &w.surface, icon.as_ref().map(|i| &i.surface), self.press_serial);
        }
        self.outgoing = Some(OutgoingDrag { state: SourceState::new(payload), check: None, source, icon, drag });
        self.shared.dragging.set(true);
        self.draw_icon(); // (its role now given: its first buffer, where the pointer is in it)
        let _ = self.connection.flush();
    }

    /// The icon's surface, a viewport and fractional scale on it where the compositor has both (else whole scales).
    fn make_icon(&self, drag: &PlatformDragAndDrop) -> Option<Icon> {
        let g = &self.globals;
        let surface = g.compositor.as_ref()?.create_surface()?;
        let (viewport, fractional) = match (&g.viewporter, &g.fractional) {
            (Some(viewporter), Some(fractional)) => {
                (viewporter.get_viewport(&surface), fractional.get_fractional_scale(&surface))
            }
            _ => (None, None),
        };
        let point_to_px = self.scale();
        let offset = icon_offset((drag.decoration_offset.x, drag.decoration_offset.y), point_to_px);
        Some(Icon {
            viewport,
            fractional,
            buffers: Vec::new(),
            surface,
            drawn: None,
            outputs: Vec::new(),
            preferred: None,
            offset: Some(offset),
        })
    }

    /// The scale the icon is to be shown at: its surface's fractional scale (else the window's) with a viewport;
    /// else the whole scale its surface prefers, else that of the outputs it is on, else the window's.
    fn icon_scale(&self) -> Option<IconScale> {
        let icon = self.outgoing.as_ref()?.icon.as_ref()?;
        if icon.viewport.is_some() {
            let scale120 = match icon.drawn {
                Some(IconScale::Fractional(s)) => s,
                _ if self.scale120 > 0 => self.scale120,
                _ => (self.scale() * 120.0).round() as u32,
            };
            return Some(IconScale::Fractional(scale120));
        }
        let on = self.outputs.iter().filter(|o| icon.outputs.contains(&o.output.id())).map(|o| o.info.scale).max();
        let whole = icon.preferred.or(on).unwrap_or_else(|| self.scale().ceil() as i32);
        Some(IconScale::Whole(if icon.surface.version() >= 3 { whole.max(1) } else { 1 }))
    }

    /// The icon drawn for `scale` if it isn't already (its first buffer with the pointer's place in it, or a new
    /// one for the scale it is now shown at).
    fn draw_icon_at(&mut self, scale: IconScale) {
        let point_to_px = self.scale();
        let Some(o) = &mut self.outgoing else { return };
        let Some(icon) = &mut o.icon else { return };
        if icon.drawn == Some(scale) {
            return;
        }
        icon.drawn = Some(scale);
        let size = (o.drag.decoration_size.width, o.drag.decoration_size.height);
        let layout = icon_layout(size, point_to_px, scale);
        let Some(image) = o.drag.render_icon(layout.image_scale) else { return };
        let Some(shm) = &self.globals.shm else { return };
        let pixels = premultiplied_argb(&image, layout.buffer.0, layout.buffer.1);
        let Some(buffer) = shm_buffer(shm, "kompoz-drag-icon", &pixels, layout.buffer.0, layout.buffer.1) else {
            crate::system::log("kompoz: Wayland: no memory for the drag's icon");
            return;
        };
        icon.buffers.retain(|b| !b.retired || b.busy);
        for b in &mut icon.buffers {
            b.retired = true;
        }
        let s = &icon.surface;
        let (dx, dy) = icon.offset.take().unwrap_or((0, 0));
        if s.version() >= 5 {
            s.attach(Some(&buffer), 0, 0);
            if (dx, dy) != (0, 0) {
                s.offset(dx, dy);
            }
        } else {
            s.attach(Some(&buffer), dx, dy);
        }
        match (&icon.viewport, layout.destination) {
            (Some(viewport), Some((w, h))) => {
                s.set_buffer_scale(1);
                viewport.set_destination(w, h);
            }
            _ if s.version() >= 3 => s.set_buffer_scale(layout.buffer_scale),
            _ => {}
        }
        if s.version() >= 4 {
            s.damage_buffer(0, 0, layout.buffer.0, layout.buffer.1);
        } else {
            s.damage(0, 0, i32::MAX, i32::MAX);
        }
        s.commit();
        icon.buffers.push(IconBuffer { buffer, busy: true, retired: false });
    }

    /// The icon drawn for the scale it is shown at now.
    pub(super) fn draw_icon(&mut self) {
        if let Some(scale) = self.icon_scale() {
            self.draw_icon_at(scale);
        }
    }

    /// The icon's surface's event: the outputs it is on, the whole scale the compositor prefers for it.
    pub(super) fn on_icon_surface(&mut self, e: WlSurfaceEvent) {
        let Some(icon) = self.outgoing.as_mut().and_then(|o| o.icon.as_mut()) else { return };
        match e {
            WlSurfaceEvent::Enter { output } => icon.outputs.push(output),
            WlSurfaceEvent::Leave { output } => icon.outputs.retain(|&o| o != output),
            WlSurfaceEvent::PreferredBufferScale { factor } => icon.preferred = Some(factor.max(1)),
            WlSurfaceEvent::PreferredBufferTransform { .. } => return,
        }
        self.draw_icon();
    }

    /// The icon's fractional scale: drawn again for it.
    pub(super) fn on_icon_fractional_scale(&mut self, e: WpFractionalScaleV1Event) {
        let WpFractionalScaleV1Event::PreferredScale { scale } = e;
        if self.outgoing.as_ref().and_then(|o| o.icon.as_ref()).is_some_and(|i| i.viewport.is_some()) {
            self.draw_icon_at(IconScale::Fractional(scale.max(1)));
        }
    }

    /// A buffer the compositor is done with, if it is one of the icon's.
    pub(super) fn icon_buffer_released(&mut self, buffer: ObjectId) -> bool {
        let Some(icon) = self.outgoing.as_mut().and_then(|o| o.icon.as_mut()) else { return false };
        let Some(b) = icon.buffers.iter_mut().find(|b| b.buffer.id() == buffer) else { return false };
        b.busy = false;
        icon.buffers.retain(|b| !b.retired || b.busy);
        true
    }

    /// Our drag's data source's event: a type asked for sent, the drag's end told to the core.
    pub(super) fn on_drag_source(&mut self, e: WlDataSourceEvent) {
        let event = match e {
            WlDataSourceEvent::Target { mime_type } => SourceEvent::Target(mime_type),
            WlDataSourceEvent::Send { mime_type, fd } => {
                let Some(o) = &mut self.outgoing else { return };
                if let SourceStep::Send(bytes) = o.state.event(SourceEvent::Send(mime_type)) {
                    super::data_device::send(fd, bytes);
                }
                return;
            }
            WlDataSourceEvent::Cancelled => SourceEvent::Cancelled,
            WlDataSourceEvent::DndDropPerformed => SourceEvent::DropPerformed,
            WlDataSourceEvent::DndFinished => SourceEvent::Finished,
            WlDataSourceEvent::Action { dnd_action } => SourceEvent::Action(dnd_action),
        };
        let Some(o) = &mut self.outgoing else { return };
        let step = o.state.event(event);
        self.drag_step(step);
    }

    /// The press (or finger) the drag began with let go and the app told: if the source heard nothing yet, whether
    /// the compositor took the drag at all is asked (a sync: whatever it tells the source about this release comes
    /// before its done).
    pub(super) fn drag_press_released(&mut self) {
        let display = self.connection.display();
        let Some(o) = &mut self.outgoing else { return };
        if o.state.heard() || o.check.is_some() {
            return;
        }
        o.check = display.sync();
        let _ = self.connection.flush();
    }

    /// The sync after the release is done: a source that still heard nothing belongs to a drag that never started
    /// (the press was let go before the compositor took it).
    pub(super) fn drag_check_done(&mut self) {
        let Some(o) = &mut self.outgoing else { return };
        o.check = None;
        if !o.state.heard() {
            self.cancel_outgoing_drag();
        }
    }

    /// The drag called off by the app (Escape where the compositor lets it through), or found never to have started
    /// (the press let go before the compositor took it): ended untaken, its source destroyed (which ends a drag the
    /// compositor runs).
    pub(super) fn cancel_outgoing_drag(&mut self) {
        let Some(o) = &mut self.outgoing else { return };
        let step = o.state.cancel();
        self.drag_step(step);
    }

    fn drag_step(&mut self, step: SourceStep) {
        if let SourceStep::Ended(action) = step {
            self.outgoing = None; // (the source and the icon destroyed)
            self.shared.dragging.set(false);
            self.events.push_back(Event::OwnDragEnded { action });
            let _ = self.connection.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kompoz_ui::draganddrop::DragAndDropTransferAction as A;

    fn state(text: &str, files: &[&str]) -> SourceState {
        SourceState::new(DragPayload {
            text: text.into(),
            files: files.iter().map(|f| f.to_string()).collect(),
            mime_types: vec!["text/plain".into()],
            actions: vec![A::Copy, A::Move, A::Link],
        })
    }

    #[test]
    fn the_sources_actions_are_copy_and_move() {
        assert_eq!(actions_mask(&[A::Copy, A::Move, A::Link]), Action::COPY | Action::MOVE);
        assert_eq!(actions_mask(&[A::Move]), Action::MOVE);
        assert_eq!(actions_mask(&[A::Link]), Action::NONE); // (no link on Wayland: it stays in the app)
        assert_eq!(transfer_action(Action::ASK), None);
    }

    #[test]
    fn a_drop_is_served_then_finished_with_the_chosen_action() {
        let mut s = state("merhaba", &["/tmp/a b"]);
        assert!(!s.heard());
        assert_eq!(s.event(SourceEvent::Target(Some("text/plain".into()))), SourceStep::Nothing);
        assert!(s.heard()); // (the compositor runs it)
        s.event(SourceEvent::Action(Action::COPY));
        s.event(SourceEvent::Action(Action::MOVE)); // (Shift, as the compositor takes it)
        assert_eq!(s.event(SourceEvent::DropPerformed), SourceStep::Nothing);
        assert_eq!(
            s.event(SourceEvent::Send("text/uri-list".into())),
            SourceStep::Send(b"file:///tmp/a%20b\r\n".to_vec())
        );
        assert_eq!(
            s.event(SourceEvent::Send("text/plain;charset=utf-8".into())),
            SourceStep::Send(b"merhaba".to_vec())
        );
        assert_eq!(s.event(SourceEvent::Send("image/png".into())), SourceStep::Send(Vec::new())); // (not offered)
        assert_eq!(s.event(SourceEvent::Finished), SourceStep::Ended(Some(A::Move)));
        // Over: nothing more of it.
        assert_eq!(s.event(SourceEvent::Cancelled), SourceStep::Nothing);
        assert_eq!(s.event(SourceEvent::Send("text/plain".into())), SourceStep::Nothing);
        assert_eq!(s.cancel(), SourceStep::Nothing);
    }

    #[test]
    fn cancelled_drags_end_untaken_once() {
        // Dropped on nothing, or refused after the drop: the compositor cancels.
        let mut s = state("x", &[]);
        s.event(SourceEvent::Target(None));
        s.event(SourceEvent::Action(Action::NONE));
        s.event(SourceEvent::DropPerformed);
        assert_eq!(s.event(SourceEvent::Cancelled), SourceStep::Ended(None));
        assert_eq!(s.event(SourceEvent::Finished), SourceStep::Nothing);
        // Escape (or never started): the app's own cancel.
        let mut s = state("x", &[]);
        s.event(SourceEvent::Action(Action::COPY));
        assert_eq!(s.cancel(), SourceStep::Ended(None));
        assert_eq!(s.event(SourceEvent::Cancelled), SourceStep::Nothing);
    }

    #[test]
    fn icons_are_drawn_at_the_scale_they_are_shown_at() {
        // A 90×60 px decoration at a window scale of 1.5 is 60×40 points.
        let at = |scale| icon_layout((90.0, 60.0), 1.5, scale);
        assert_eq!(
            at(IconScale::Fractional(180)),
            IconLayout { image_scale: 1.0, buffer: (90, 60), buffer_scale: 1, destination: Some((60, 40)) }
        );
        // Over a monitor at 2: its pixels (the decoration drawn 4/3 bigger), shown at the same size.
        assert_eq!(
            at(IconScale::Fractional(240)),
            IconLayout { image_scale: 4.0 / 3.0, buffer: (120, 80), buffer_scale: 1, destination: Some((60, 40)) }
        );
        // Whole scales: whole points times the scale (the decoration at the buffer's top left).
        let l = icon_layout((91.0, 60.0), 1.0, IconScale::Whole(2));
        assert_eq!(l, IconLayout { image_scale: 2.0, buffer: (182, 120), buffer_scale: 2, destination: None });
        let l = icon_layout((91.0, 61.0), 1.5, IconScale::Whole(2));
        assert_eq!((l.buffer, l.buffer_scale), ((122, 82), 2)); // (61 × 41 points)
        // The pointer 30, 15 px into it: the icon's top left that far up and left of the hotspot, in points.
        assert_eq!(icon_offset((30.0, 15.0), 1.5), (-20, -10));
        assert_eq!(icon_offset((45.0, 30.0), 1.0), (-45, -30));
    }
}
