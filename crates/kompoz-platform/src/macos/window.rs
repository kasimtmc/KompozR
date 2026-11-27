//! The macOS platform's windows (C++'s MacPlatform's window part): each window's state as its view and delegate
//! keep it ([`WindowState`]: its sizes and scale, placement, focus, cursor, the field the input methods see, its
//! display link's beat, its menu bar, the drops coming in and its own drag going out, the events for its core), and
//! the handle each app core has on its window ([`MacWindow`], the `Platform`).
//!
//! Points are AppKit's (the view's, top down), dp as they are; px are the drawable's: points times the backing scale
//! (`KOMPOZ_SCALE` instead: the drawable that many px a point, the app's density with it — shots of a known size).

#![allow(unsafe_code)]

use super::app::{Shared, host_waker, wake_loop};
use super::clipboard::{self, Items};
use super::display_link::{DisplayLink, Tick};
use super::keyboard;
use super::services;
use super::text_input::{Ime, NOT_FOUND, utf16_of_byte};
use super::types;
use super::view::{KompozView, KompozWindow};
use super::{StepKind, now};
use crate::app::{Event, Metrics, MouseButton, Platform, TrayImage, TrayMenuItem, WindowSpec};
use crate::system::{log, preferred_language};
use crate::transfer::Payload;
use kompoz_gpu::device::{Backend, MetalSurface, Surface};
use kompoz_graphics::Rect;
use kompoz_graphics::material_color::palettes::DynamicTonalPalette;
use kompoz_ui::KompozHost;
use kompoz_ui::MotionDurationScale;
use kompoz_ui::clipboard::ClipEntry;
use kompoz_ui::draganddrop::{DragAndDropTransferAction, PlatformDragAndDrop};
use kompoz_ui::input::KeyModifiers;
use kompoz_ui::media_query::{MediaInfo, PointerPrecision};
use kompoz_ui::platform::{ImeOptions, PlatformCallbacks, PointerIcon, TextEditState};
use kompoz_ui::window::{WindowId, WindowPlacement};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{AnyThread, Message};
use objc2_app_kit::{
    NSColor, NSDragOperation, NSDraggingInfo, NSDraggingItem, NSEvent, NSEventButtonMask, NSEventPhase, NSEventSubtype,
    NSFloatingWindowLevel, NSNormalWindowLevel, NSPointingDeviceType, NSScreen, NSView, NSWindowCollectionBehavior,
    NSWindowStyleMask,
};
use objc2_foundation::{NSArray, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::{Rc, Weak};
use std::sync::Arc;

/// How long a window waits for its display link's tick before it draws anyway (a display that stopped ticking: a
/// virtual one, a link that lost its display).
const TICK_STALL_NS: i64 = 50_000_000;

/// A pen's pointer ids: its device's id above the mouse's and the fingers'.
const STYLUS_POINTER: i64 = 1 << 32;

/// What an event of the mouse's was.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MouseKind {
    Down(MouseButton),
    Up(MouseButton),
    Move,
    /// Moved with a button held.
    Drag,
}

/// The input methods' output while a key is interpreted (`interpretKeyEvents:`), for the key's event to be told
/// with it in order: what they did, whether they composed, whether they passed the key on.
#[derive(Default)]
struct Interpreted {
    events: Vec<Event>,
    composed: bool,
    passed_on: bool,
}

/// A drag of the app's own out of the window, while AppKit runs it.
struct OwnDrag {
    payload: Rc<Payload>,
    /// (Kept: its provider serves the drop's target.)
    _items: Items,
}

/// A window's state (C++'s MacPlatform's fields): what the core reads through its handle, the events for its core.
pub struct WindowState {
    pub(super) id: WindowId,
    shared: Weak<Shared>,
    window: RefCell<Option<Retained<KompozWindow>>>,
    view: RefCell<Option<Retained<KompozView>>>,
    events: RefCell<VecDeque<Event>>,
    /// The drawable's size, px (kept as it was while minimized).
    size: Rc<Cell<(i32, i32)>>,
    /// The view's size, points.
    points: Cell<(f64, f64)>,
    /// px per point: the backing scale, or `KOMPOZ_SCALE`.
    px_per_point: Cell<f64>,
    scale_option: Option<f64>,
    refresh_rate: Cell<f32>,
    placement: Cell<WindowPlacement>,
    minimized: Cell<bool>,
    key: Cell<bool>,
    cursor: Cell<PointerIcon>,
    pointer_inside: Cell<bool>,
    /// The pointer's last place (a drop's leaving is told there).
    last_point: Cell<(f32, f32)>,
    ime: RefCell<Ime>,
    interpreted: RefCell<Option<Interpreted>>,
    tick: Arc<Tick>,
    link: RefCell<Option<DisplayLink>>,
    /// When the window began waiting for its tick (its link asked to run), on the platform's clock.
    waiting_since: Cell<Option<i64>>,
    menus: RefCell<Option<Vec<TrayMenuItem>>>,
    title: RefCell<String>,
    keep_on: Cell<bool>,
    /// A pen near the tablet: its pointer id, whether it is its eraser end.
    stylus: Cell<Option<(i64, bool)>>,
    /// The trackpad's pinch, its turn: which of them is going on.
    magnifying: Cell<bool>,
    rotating: Cell<bool>,
    /// The last press or drag of the mouse: what a drag of the app's own out of the window starts from.
    last_mouse: RefCell<Option<Retained<NSEvent>>>,
    /// The drag coming over the window is the app's own.
    drop_own: Cell<bool>,
    /// What a drop from another app brought, read as it dropped (the core takes it: `take_drop_entry`).
    drop_entry: RefCell<Option<ClipEntry>>,
    own_drag: RefCell<Option<OwnDrag>>,
}

impl WindowState {
    fn new(shared: &Rc<Shared>, id: WindowId) -> WindowState {
        let scale_option = kompoz_runtime::environment::var("KOMPOZ_SCALE")
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|&s| s > 0.0);
        WindowState {
            id,
            shared: Rc::downgrade(shared),
            window: RefCell::new(None),
            view: RefCell::new(None),
            events: RefCell::new(VecDeque::new()),
            size: Rc::new(Cell::new((1, 1))),
            points: Cell::new((1.0, 1.0)),
            px_per_point: Cell::new(scale_option.unwrap_or(1.0)),
            scale_option,
            refresh_rate: Cell::new(60.0),
            placement: Cell::new(WindowPlacement::Floating),
            minimized: Cell::new(false),
            key: Cell::new(false),
            cursor: Cell::new(PointerIcon::Default),
            pointer_inside: Cell::new(false),
            last_point: Cell::new((0.0, 0.0)),
            ime: RefCell::new(Ime::default()),
            interpreted: RefCell::new(None),
            tick: Arc::new(Tick::default()),
            link: RefCell::new(None),
            waiting_since: Cell::new(None),
            menus: RefCell::new(None),
            title: RefCell::new("KompozR".into()),
            keep_on: Cell::new(false),
            stylus: Cell::new(None),
            magnifying: Cell::new(false),
            rotating: Cell::new(false),
            last_mouse: RefCell::new(None),
            drop_own: Cell::new(false),
            drop_entry: RefCell::new(None),
            own_drag: RefCell::new(None),
        }
    }

    fn push(&self, event: Event) {
        self.events.borrow_mut().push_back(event);
    }

    pub(super) fn take_events(&self) -> Vec<Event> {
        self.events.borrow_mut().drain(..).collect()
    }

    fn shared(&self) -> Option<Rc<Shared>> {
        self.shared.upgrade()
    }

    fn window(&self) -> Option<Retained<KompozWindow>> {
        self.window.borrow().clone()
    }

    fn view(&self) -> Option<Retained<KompozView>> {
        self.view.borrow().clone()
    }

    pub fn is_key(&self) -> bool {
        self.key.get()
    }

    pub fn menus(&self) -> Option<Vec<TrayMenuItem>> {
        self.menus.borrow().clone()
    }

    pub fn keeps_display_on(&self) -> bool {
        self.keep_on.get()
    }

    // ---- Frames ----

    /// Whether the window may draw its next frame: its display ticked since its last (its link asked to run while
    /// it waits); one without a link (not shown) at once; a minimized one not until it is restored; one whose link
    /// hasn't ticked for a while ([`TICK_STALL_NS`]) anyway.
    pub fn frame_ready(&self) -> bool {
        let link = self.link.borrow();
        let Some(link) = link.as_ref() else { return true };
        if self.minimized.get() {
            return false;
        }
        if self.tick.take() {
            self.waiting_since.set(None);
            return true;
        }
        link.set_running(true);
        let at = now();
        match self.waiting_since.get() {
            None => {
                self.waiting_since.set(Some(at));
                false
            }
            Some(since) if at - since >= TICK_STALL_NS => {
                self.waiting_since.set(None);
                true
            }
            Some(_) => false,
        }
    }

    /// When a window waiting for its tick draws anyway (the loop's wait ends by then); none if it isn't waiting.
    pub fn stall_ends(&self) -> Option<i64> {
        self.waiting_since.get().map(|since| since + TICK_STALL_NS)
    }

    /// No frame wanted: the display link stopped until one is.
    pub fn frames_unwanted(&self) {
        if let Some(link) = self.link.borrow().as_ref() {
            link.set_running(false);
        }
        self.waiting_since.set(None);
        self.tick.take();
    }

    /// The display link ticked (the view's, on the main thread).
    pub fn ticked(&self) {
        self.tick.ticked();
    }

    // ---- The view's size and scale ----

    /// The view's size or the window's backing scale changed: the drawable's size and the layer's scale with them;
    /// Resized if the size did, MetricsChanged if the scale did.
    pub fn frame_changed(&self, view: &NSView) {
        let bounds = view.bounds();
        let backing = view.window().map_or(1.0, |w| w.backingScaleFactor());
        let scale = self.scale_option.unwrap_or(backing);
        if let Some(layer) = view.layer() {
            layer.setContentsScale(scale);
        }
        let points = (bounds.size.width, bounds.size.height);
        let px = (((points.0 * scale).round() as i32).max(1), ((points.1 * scale).round() as i32).max(1));
        let scale_changed = scale != self.px_per_point.replace(scale);
        let resized = px != self.size.get() || points != self.points.get();
        if points.0 >= 1.0 && points.1 >= 1.0 {
            self.points.set(points);
            self.size.set(px);
        }
        if resized {
            self.push(Event::Resized);
        }
        if scale_changed {
            self.push(Event::MetricsChanged);
        }
    }

    pub fn exposed(&self) {
        self.push(Event::Exposed);
    }

    pub fn appearance_changed(&self) {
        if let Some(shared) = self.shared() {
            shared.appearance_changed();
        }
    }

    // ---- The mouse, the trackpad, the tablet ----

    /// A mouse's event (a pen's as the mouse: the tablet's).
    pub fn mouse(&self, event: &NSEvent, (x, y): (f64, f64), kind: MouseKind) {
        if event.subtype() == NSEventSubtype::TabletPoint {
            let down = match kind {
                MouseKind::Down(MouseButton::Left) | MouseKind::Drag => Some(true),
                MouseKind::Up(MouseButton::Left) | MouseKind::Move => Some(false),
                _ => None,
            };
            if down.is_some() {
                self.tablet_point(event, (x, y), down);
                return;
            }
        }
        let (x, y) = (x as f32, y as f32);
        self.last_point.set((x, y));
        let modifiers = modifiers_of(event);
        let time = time_of(event);
        if matches!(kind, MouseKind::Down(_) | MouseKind::Drag) {
            *self.last_mouse.borrow_mut() = Some(event.retain());
        }
        self.push(match kind {
            MouseKind::Down(button) => Event::MouseDown { x, y, button, modifiers, time },
            MouseKind::Up(button) => Event::MouseUp { x, y, button, modifiers, time },
            MouseKind::Move | MouseKind::Drag => Event::MouseMove { x, y, modifiers, time },
        });
    }

    pub fn pointer_entered(&self) {
        self.pointer_inside.set(true);
        self.show_cursor();
    }

    pub fn pointer_exited(&self) {
        self.pointer_inside.set(false);
        self.push(Event::PointerLeft);
    }

    /// The app's cursor shown.
    pub fn show_cursor(&self) {
        services::cursor(self.cursor.get()).set();
    }

    /// The wheel (C++'s, as SDL tells it): a trackpad's precise scrolling (its momentum after the fingers left too)
    /// a tenth of its points a notch, a wheel's notches whole, away from zero; flipped back where the system scrolls
    /// naturally (the app's own scrolling turns it as the content goes).
    pub fn scroll(&self, event: &NSEvent, (x, y): (f64, f64)) {
        let (dx, dy) = if event.hasPreciseScrollingDeltas() {
            (-event.scrollingDeltaX() * 0.1, event.scrollingDeltaY() * 0.1)
        } else {
            let away = |v: f64| if v > 0.0 { v.ceil() } else { v.floor() };
            (away(-event.deltaX()), away(event.deltaY()))
        };
        let flip = if event.isDirectionInvertedFromDevice() { -1.0 } else { 1.0 };
        let (x, y) = (x as f32, y as f32);
        let (modifiers, time) = (modifiers_of(event), time_of(event));
        self.push(Event::Wheel { x, y, dx: (dx * flip) as f32, dy: (dy * flip) as f32, modifiers, time });
    }

    /// A trackpad's pinch: begun, by each step's magnification, ended (with its turn: one pinch while either goes on).
    pub fn magnify(&self, event: &NSEvent, (x, y): (f64, f64)) {
        let phase = event.phase();
        let (x, y) = (x as f32, y as f32);
        if phase.contains(NSEventPhase::Began) {
            self.gesture(&self.magnifying, true, x, y);
        }
        if phase.intersects(NSEventPhase::Began | NSEventPhase::Changed) {
            self.push(Event::Pinch { x, y, scale: (1.0 + event.magnification()) as f32, rotation: 0.0 });
        }
        if phase.intersects(NSEventPhase::Ended | NSEventPhase::Cancelled) {
            self.gesture(&self.magnifying, false, x, y);
        }
    }

    /// A trackpad's two fingers turning (AppKit's degrees counterclockwise; the app's clockwise).
    pub fn rotate(&self, event: &NSEvent, (x, y): (f64, f64)) {
        let phase = event.phase();
        let (x, y) = (x as f32, y as f32);
        if phase.contains(NSEventPhase::Began) {
            self.gesture(&self.rotating, true, x, y);
        }
        if phase.intersects(NSEventPhase::Began | NSEventPhase::Changed) {
            self.push(Event::Pinch { x, y, scale: 1.0, rotation: -event.rotation() });
        }
        if phase.intersects(NSEventPhase::Ended | NSEventPhase::Cancelled) {
            self.gesture(&self.rotating, false, x, y);
        }
    }

    /// One of the gestures began or ended: the pinch begun as the first begins, ended as the last ends.
    fn gesture(&self, which: &Cell<bool>, on: bool, x: f32, y: f32) {
        let before = self.magnifying.get() || self.rotating.get();
        which.set(on);
        let after = self.magnifying.get() || self.rotating.get();
        if !before && after {
            self.push(Event::PinchBegin { x, y });
        } else if before && !after {
            self.push(Event::PinchEnd { x, y });
        }
    }

    /// A pen's report (its own, or as the mouse: `down` then says whether it touches): where, how hard, its barrel
    /// buttons (the lower one first). Its tilt AppKit tells too, but the app's pointer events carry none (as Linux's).
    pub fn tablet_point(&self, event: &NSEvent, (x, y): (f64, f64), down: Option<bool>) {
        let (pointer, eraser) = self.stylus.get().unwrap_or((STYLUS_POINTER | event.deviceID() as i64, false));
        let mask = event.buttonMask();
        let pressure = event.pressure().clamp(0.0, 1.0);
        let down = down.unwrap_or(mask.contains(NSEventButtonMask::PenTip) || pressure > 0.0);
        let mut buttons = 0;
        if mask.contains(NSEventButtonMask::PenLowerSide) {
            buttons |= 1;
        }
        if mask.contains(NSEventButtonMask::PenUpperSide) {
            buttons |= 2;
        }
        let (x, y) = (x as f32, y as f32);
        self.last_point.set((x, y));
        self.push(Event::Stylus { pointer, eraser, x, y, down, pressure, buttons, time: time_of(event) });
    }

    /// A pen coming near the tablet (its tip or its eraser) or going away.
    pub fn tablet_proximity(&self, event: &NSEvent) {
        let pointer = STYLUS_POINTER | event.deviceID() as i64;
        if event.isEnteringProximity() {
            self.stylus.set(Some((pointer, event.pointingDeviceType() == NSPointingDeviceType::Eraser)));
        } else {
            let (pointer, _) = self.stylus.take().unwrap_or((pointer, false));
            self.push(Event::StylusLeft { pointer });
        }
    }

    // ---- The keyboard and the input methods ----

    /// A key pressed (C++'s `keyDown:`, the input methods' order kept): with a field focused and neither Command nor
    /// Control held, the key interpreted by the input methods first (`interpretKeyEvents:`); the key's own event
    /// told unless they composed with it (a key an input method takes isn't the app's), then what they did.
    pub fn key_down(&self, view: &KompozView, event: &NSEvent) {
        let flags = event.modifierFlags().0 as u64;
        let modifiers = keyboard::modifiers(flags);
        let characters = event.characters().map(|c| c.to_string()).unwrap_or_default();
        let unmodified = event.charactersIgnoringModifiers().map(|c| c.to_string()).unwrap_or_default();
        let key = keyboard::key_of(event.keyCode(), &unmodified);
        let code_point = keyboard::code_point(&characters);
        let down = Event::KeyDown { key, code_point, modifiers, time: time_of(event) };
        let shortcut = modifiers.contains(KeyModifiers::Meta) || modifiers.contains(KeyModifiers::Ctrl);
        let (active, composing) = {
            let ime = self.ime.borrow();
            (ime.active, ime.has_marked_text())
        };
        if !active || shortcut {
            self.push(down);
            return;
        }
        *self.interpreted.borrow_mut() = Some(Interpreted::default());
        view.interpretKeyEvents(&NSArray::from_retained_slice(&[event.retain()]));
        let interpreted = self.interpreted.borrow_mut().take().unwrap_or_default();
        if !(composing || interpreted.composed) || interpreted.passed_on {
            self.push(down);
        }
        for e in interpreted.events {
            self.push(e);
        }
    }

    pub fn key_up(&self, event: &NSEvent) {
        let flags = event.modifierFlags().0 as u64;
        let unmodified = event.charactersIgnoringModifiers().map(|c| c.to_string()).unwrap_or_default();
        let characters = event.characters().map(|c| c.to_string()).unwrap_or_default();
        self.push(Event::KeyUp {
            key: keyboard::key_of(event.keyCode(), &unmodified),
            code_point: keyboard::code_point(&characters),
            modifiers: keyboard::modifiers(flags),
            time: time_of(event),
        });
    }

    /// A modifier key pressed or let go.
    pub fn flags_changed(&self, event: &NSEvent) {
        let flags = event.modifierFlags().0 as u64;
        let Some((key, down)) = keyboard::modifier_change(event.keyCode(), flags) else { return };
        let (modifiers, time) = (keyboard::modifiers(flags), time_of(event));
        self.push(if down {
            Event::KeyDown { key, code_point: 0, modifiers, time }
        } else {
            Event::KeyUp { key, code_point: 0, modifiers, time }
        });
    }

    /// The input methods' events: with the key being interpreted, else at once (the character palette's, the
    /// press-and-hold menu's).
    fn ime_events(&self, events: Vec<Event>, composed: bool) {
        let mut interpreted = self.interpreted.borrow_mut();
        match interpreted.as_mut() {
            Some(i) => {
                i.events.extend(events);
                i.composed |= composed;
            }
            None => {
                drop(interpreted);
                for e in events {
                    self.push(e);
                }
                wake_loop();
            }
        }
    }

    pub fn ime_insert(&self, text: &str, replacement: Option<(usize, usize)>) {
        let active = self.ime.borrow().active;
        if !active {
            // (No field: what a key types is its key event's.)
            return;
        }
        let events = self.ime.borrow_mut().insert(text, replacement);
        self.ime_events(events, false);
    }

    pub fn ime_set_marked(&self, text: &str, selected: (usize, usize), replacement: Option<(usize, usize)>) {
        if !self.ime.borrow().active {
            return;
        }
        let events = self.ime.borrow_mut().set_marked(text, selected, replacement);
        self.ime_events(events, true);
    }

    pub fn ime_unmark(&self) {
        let events = self.ime.borrow_mut().unmark();
        self.ime_events(events, false);
    }

    /// The input method passed the key on (`doCommandBySelector:`).
    pub fn ime_command(&self) {
        if let Some(i) = self.interpreted.borrow_mut().as_mut() {
            i.passed_on = true;
        }
    }

    pub fn ime_selected_range(&self) -> (usize, usize) {
        let ime = self.ime.borrow();
        if ime.active { ime.selected_range() } else { (NOT_FOUND, 0) }
    }

    pub fn ime_marked_range(&self) -> Option<(usize, usize)> {
        let ime = self.ime.borrow();
        if ime.active { ime.marked_range() } else { None }
    }

    pub fn ime_substring(&self, location: usize, length: usize) -> Option<(String, (usize, usize))> {
        let ime = self.ime.borrow();
        if ime.active { ime.substring(location, length) } else { None }
    }

    /// The field's area in the view's points (where the input method's candidates go: below it).
    pub fn ime_area_points(&self) -> NSRect {
        let area = self.ime.borrow().area;
        let k = 1.0 / self.px_per_point.get();
        NSRect::new(
            NSPoint::new(area.left as f64 * k, area.top as f64 * k),
            NSSize::new(area.width() as f64 * k, area.height() as f64 * k),
        )
    }

    /// The character at a point of the view: the caret's (UTF-16) where it is in the field, none elsewhere.
    pub fn ime_index_at(&self, x: f64, y: f64) -> usize {
        let ime = self.ime.borrow();
        let ppp = self.px_per_point.get() as f32;
        let (x, y) = (x as f32 * ppp, y as f32 * ppp);
        let a = ime.area;
        if !ime.active || x < a.left || x > a.right || y < a.top || y > a.bottom {
            return NOT_FOUND;
        }
        utf16_of_byte(&ime.state.text, ime.state.selection_end)
    }

    /// The field as the host tells it (focused or not, where, its text): the input methods' marked text dropped
    /// where the app dropped the composition (or the field went), the input context told its characters moved.
    fn set_text_input(&self, active: bool, area: Rect, state: &TextEditState) {
        let discard = {
            let mut ime = self.ime.borrow_mut();
            let had = ime.has_marked_text();
            ime.active = active;
            ime.area = area;
            ime.state = state.clone();
            had && (!active || state.composition.is_none())
        };
        if let Some(view) = self.view()
            && let Some(context) = view.inputContext()
        {
            if discard {
                context.discardMarkedText();
            }
            context.invalidateCharacterCoordinates();
        }
    }

    /// The field's state as the app core left it after handling what was typed (the session's state).
    pub fn set_ime_state(&self, state: TextEditState) {
        let mut ime = self.ime.borrow_mut();
        if ime.active {
            ime.state = state;
        }
    }

    // ---- The window ----

    pub fn close_requested(&self) {
        self.push(Event::CloseRequest);
    }

    /// The window resized: zoomed (AppKit's maximized) or not any more.
    pub fn placement_changed(&self) {
        let Some(window) = self.window() else { return };
        if self.placement.get() == WindowPlacement::Fullscreen || self.minimized.get() {
            return;
        }
        let zoomed = window.isZoomed();
        match (zoomed, self.placement.get()) {
            (true, WindowPlacement::Floating) => {
                self.placement.set(WindowPlacement::Maximized);
                self.push(Event::Maximized);
            }
            (false, WindowPlacement::Maximized) => {
                self.placement.set(WindowPlacement::Floating);
                self.push(Event::Restored);
            }
            _ => {}
        }
    }

    pub fn minimized_changed(&self, minimized: bool) {
        if self.minimized.replace(minimized) != minimized {
            self.push(if minimized { Event::Minimized } else { Event::Restored });
        }
    }

    /// Key or not: the app's focus, and the menu bar the key window's.
    pub fn focus_changed(&self, key: bool) {
        self.key.set(key);
        self.push(if key { Event::FocusGained } else { Event::FocusLost });
        if let Some(shared) = self.shared() {
            shared.menus_changed();
        }
    }

    /// On another display: its refresh rate and scale read again.
    pub fn screen_changed(&self) {
        let rate = services::refresh_rate(self.window().and_then(|w| w.screen()).as_deref());
        if rate != self.refresh_rate.replace(rate) {
            self.push(Event::MetricsChanged);
        }
        if let Some(view) = self.view() {
            self.frame_changed(&view);
        }
    }

    pub fn full_screen_changed(&self, full: bool) {
        self.placement.set(if full { WindowPlacement::Fullscreen } else { WindowPlacement::Floating });
        self.push(Event::MetricsChanged);
        self.placement_changed();
    }

    // ---- Drops ----

    /// A drag came over the window (another app's, or the app's own back from outside): its types as the app
    /// reads them (the app's own: the core has them).
    pub fn drag_entered(&self, info: &ProtocolObject<dyn NSDraggingInfo>, (x, y): (f64, f64)) -> NSDragOperation {
        let own = info.draggingSource().is_some_and(|s| s.downcast_ref::<KompozView>().is_some());
        self.drop_own.set(own);
        let mime_types = if own {
            Vec::new()
        } else {
            let kinds: Vec<String> = info
                .draggingPasteboard()
                .types()
                .map(|t| t.iter().map(|k| k.to_string()).collect())
                .unwrap_or_default();
            types::mime_types(&kinds)
        };
        let (x, y) = (x as f32, y as f32);
        self.last_point.set((x, y));
        self.push(Event::DropBegin { x, y, mime_types });
        wake_loop();
        drop_operation(info)
    }

    pub fn drag_moved(&self, info: &ProtocolObject<dyn NSDraggingInfo>, (x, y): (f64, f64)) -> NSDragOperation {
        let (x, y) = (x as f32, y as f32);
        self.last_point.set((x, y));
        self.push(Event::DropMove { x, y });
        wake_loop();
        drop_operation(info)
    }

    /// The drag left without dropping: the drop told complete with nothing (the targets see it leave).
    pub fn drag_left(&self) {
        let (x, y) = self.last_point.get();
        self.push(Event::DropComplete { x, y });
        wake_loop();
    }

    /// Dropped: another app's data read now (the source's pasteboard is its own only while it drops), or the app's
    /// own told as such; the app core handles it at once (the loop stepped), what it answers the source's answer.
    pub fn dropped(&self, info: &ProtocolObject<dyn NSDraggingInfo>, (x, y): (f64, f64)) -> bool {
        let (x, y) = (x as f32, y as f32);
        if self.drop_own.get() {
            self.push(Event::DropOwn { x, y });
        } else {
            let entry = clipboard::entry_of(&info.draggingPasteboard()).map(|e| e.materialized());
            *self.drop_entry.borrow_mut() = entry;
            self.push(Event::DropData { x, y });
        }
        self.push(Event::DropComplete { x, y });
        let Some(shared) = self.shared() else { return false };
        shared.drop_taken.set(None);
        shared.step(StepKind::Frame);
        shared.drop_taken.take().unwrap_or(true)
    }

    // ---- The app's own drag out ----

    /// A drag within the app that left the window, handed to AppKit (from the mouse's last press or drag): what it
    /// carries as pasteboard items (promised: made as the target asks), its decoration as the dragged image where it
    /// was under the pointer. Whether AppKit took it.
    fn start_drag(&self, drag: &PlatformDragAndDrop) -> bool {
        let (Some(view), Some(event)) = (self.view(), self.last_mouse.borrow().clone()) else { return false };
        let payload = Rc::new(Payload::new(&drag.transfer_data));
        if payload.actions.is_empty() {
            return false;
        }
        let items = clipboard::items(&payload, true);
        let k = 1.0 / self.px_per_point.get();
        let frame = NSRect::new(
            NSPoint::new(
                (drag.pointer.x - drag.decoration_offset.x) as f64 * k,
                (drag.pointer.y - drag.decoration_offset.y) as f64 * k,
            ),
            NSSize::new(
                (drag.decoration_size.width as f64 * k).max(1.0),
                (drag.decoration_size.height as f64 * k).max(1.0),
            ),
        );
        let image = drag
            .render_icon(1.0)
            .and_then(|icon| services::image_of_rgba(icon.width(), icon.height(), &icon.pixels(), frame.size.width));
        let mut dragging = Vec::new();
        for (i, item) in items.items.iter().enumerate() {
            let writer = ProtocolObject::from_ref(&**item);
            let made = NSDraggingItem::initWithPasteboardWriter(NSDraggingItem::alloc(), writer);
            let contents: Option<&AnyObject> = if i == 0 { image.as_deref().map(|i| i as &AnyObject) } else { None };
            // SAFETY: the contents are an NSImage (or none), what a dragging item's image may be.
            unsafe { made.setDraggingFrame_contents(frame, contents) };
            dragging.push(made);
        }
        let source = ProtocolObject::from_ref(&*view);
        view.beginDraggingSessionWithItems_event_source(&NSArray::from_retained_slice(&dragging), &event, source);
        *self.own_drag.borrow_mut() = Some(OwnDrag { payload, _items: items });
        true
    }

    /// What the app's own drag allows (Copy, Move, Link as the app said).
    pub fn own_drag_operations(&self) -> NSDragOperation {
        let mut mask = NSDragOperation::None;
        if let Some(own) = self.own_drag.borrow().as_ref() {
            for a in &own.payload.actions {
                mask |= if *a == DragAndDropTransferAction::Copy {
                    NSDragOperation::Copy
                } else if *a == DragAndDropTransferAction::Move {
                    NSDragOperation::Move
                } else if *a == DragAndDropTransferAction::Link {
                    NSDragOperation::Link
                } else {
                    NSDragOperation::None
                };
            }
        }
        mask
    }

    /// The app's own drag ended: taken with the operation the target chose (a Trash's delete as a move), or not.
    pub fn own_drag_ended(&self, operation: NSDragOperation) {
        self.own_drag.borrow_mut().take();
        let action = if operation.intersects(NSDragOperation::Move | NSDragOperation::Delete) {
            Some(DragAndDropTransferAction::Move)
        } else if operation.contains(NSDragOperation::Copy) {
            Some(DragAndDropTransferAction::Copy)
        } else if operation.contains(NSDragOperation::Link) {
            Some(DragAndDropTransferAction::Link)
        } else {
            None
        };
        self.push(Event::OwnDragEnded { action });
        wake_loop();
    }
}

/// The operation a drop over the window takes of those its source allows: a copy, else a move, a link, AppKit's
/// generic one.
fn drop_operation(info: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
    let allowed = info.draggingSourceOperationMask();
    for op in [NSDragOperation::Copy, NSDragOperation::Move, NSDragOperation::Link, NSDragOperation::Generic] {
        if allowed.contains(op) {
            return op;
        }
    }
    NSDragOperation::None
}

/// An event's time on the platform's clock (seconds since the system started, as `now`'s).
fn time_of(event: &NSEvent) -> i64 {
    (event.timestamp() * 1e9) as i64
}

fn modifiers_of(event: &NSEvent) -> KeyModifiers {
    keyboard::modifiers(event.modifierFlags().0 as u64)
}

// ---- The handle each core has ----------------------------------------------------------------------------------

/// A window of the app's (the root core's, or a `Window`'s): its state, the platform it shares.
pub struct MacWindow {
    shared: Rc<Shared>,
    state: Rc<WindowState>,
}

impl MacWindow {
    /// A handle on window `id` (opened by the core: `open_window`).
    pub fn new(shared: &Rc<Shared>, id: WindowId) -> MacWindow {
        let state = Rc::new(WindowState::new(shared, id));
        shared.windows.borrow_mut().push(state.clone());
        MacWindow { shared: shared.clone(), state }
    }

    fn window(&self) -> Option<Retained<KompozWindow>> {
        self.state.window()
    }

    /// The screen the window is on (the main one before it is open).
    fn screen(&self) -> Option<Retained<NSScreen>> {
        self.window().and_then(|w| w.screen()).or_else(|| NSScreen::mainScreen(self.shared.mtm))
    }
}

impl Drop for MacWindow {
    fn drop(&mut self) {
        self.close_window();
        self.shared.windows.borrow_mut().retain(|w| !Rc::ptr_eq(w, &self.state));
        self.shared.keep_display_awake();
        self.shared.menus_changed();
    }
}

/// The callbacks the macOS platform gives a window's host (C++'s `attach`): waking the loop, the field for the
/// input methods, the cursor, the pasteboard, links, drags out.
struct MacCallbacks {
    shared: Rc<Shared>,
    state: Rc<WindowState>,
}

impl PlatformCallbacks for MacCallbacks {
    fn waker(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
        Some(host_waker())
    }

    fn text_input(&self, active: bool, area: Rect, state: &TextEditState, _options: &ImeOptions) {
        self.state.set_text_input(active, area, state);
    }

    fn text_input_changed(&self, state: &TextEditState) {
        let active = self.state.ime.borrow().active;
        let area = self.state.ime.borrow().area;
        self.state.set_text_input(active, area, state);
    }

    /// Shown at once where the mouse is over the window, else as it comes in (`cursorUpdate:`).
    fn cursor(&self, icon: PointerIcon) {
        self.state.cursor.set(icon);
        if self.state.pointer_inside.get() {
            services::cursor(icon).set();
        }
    }

    fn get_clip_entry(&self) -> Option<ClipEntry> {
        self.shared.clipboard.entry()
    }

    fn set_clip_entry(&self, entry: Option<ClipEntry>) {
        self.shared.clipboard.set(entry);
    }

    /// Opened by the workspace (C++'s `NSWorkspace openURL`).
    fn open_uri(&self, uri: &str) {
        if !services::open_uri(uri) {
            log(&format!("kompoz: can't open {uri}"));
        }
    }

    fn start_drag_and_drop(&self, drag: &PlatformDragAndDrop) -> bool {
        self.state.start_drag(drag)
    }
}

impl Platform for MacWindow {
    fn option(&self, name: &str) -> Option<String> {
        kompoz_runtime::environment::var(name)
    }

    fn log(&self, message: &str) {
        log(message);
    }

    fn now(&self) -> i64 {
        now()
    }

    fn backends(&self) -> Vec<Backend> {
        vec![Backend::Metal]
    }

    /// The window (C++'s `openWindow`): an NSWindow (titled, closable, minimizable, resizable as asked; borderless
    /// when undecorated), its view the content, the first responder and the delegate, at its place (the screen's
    /// center, or where it is asked to be), see-through where transparent, above the others where asked; shown as it
    /// is to be (maximized, full screen, minimized, without taking the focus); its display link made.
    fn open_window(&mut self, spec: &WindowSpec, backend: Backend) -> Result<Surface, String> {
        if backend != Backend::Metal {
            return Err(format!("no {} on macOS", backend.name()));
        }
        let mtm = self.shared.mtm;
        let state = &self.state.clone();
        *state.title.borrow_mut() = spec.title.clone();
        if state.id == WindowId::MAIN || state.id == WindowId::APPLICATION {
            *self.shared.app_title.borrow_mut() = spec.app_title.clone();
        }
        let mut style = if spec.undecorated {
            NSWindowStyleMask::Borderless | NSWindowStyleMask::Miniaturizable
        } else {
            NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Miniaturizable
        };
        if spec.resizable {
            style |= NSWindowStyleMask::Resizable;
        }
        let (width, height) = (spec.width.0.max(1.0) as f64, spec.height.0.max(1.0) as f64);
        let window = KompozWindow::new(mtm, style, width, height, spec.focusable);
        let view = KompozView::new(mtm, state, width, height);
        window.setContentView(Some(&view));
        window.setDelegate(Some(ProtocolObject::from_ref(&*view)));
        window.makeFirstResponder(Some(&view));
        window.setTitle(&NSString::from_str(&spec.title));
        window.setAcceptsMouseMovedEvents(true);
        window.setCollectionBehavior(NSWindowCollectionBehavior::FullScreenPrimary);
        if spec.transparent {
            window.setOpaque(false);
            window.setBackgroundColor(Some(&NSColor::clearColor()));
        }
        if spec.always_on_top {
            window.setLevel(NSFloatingWindowLevel);
        }
        let dragged: Vec<Retained<NSString>> = [
            types::FILE_URL,
            types::URL,
            types::STRING,
            types::LEGACY_STRING,
            types::HTML,
            types::PNG,
            types::TIFF,
            kompoz_ui::clipboard::MIMETYPE_TEXT_URILIST,
        ]
        .iter()
        .map(|t| NSString::from_str(t))
        .collect();
        view.registerForDraggedTypes(&NSArray::from_retained_slice(&dragged));
        *state.window.borrow_mut() = Some(window.clone());
        *state.view.borrow_mut() = Some(view.clone());
        match spec.position {
            Some((x, y)) => self.set_position(x.0.round() as i32, y.0.round() as i32),
            None => window.center(),
        }
        state.frame_changed(&view);
        state.refresh_rate.set(services::refresh_rate(window.screen().as_deref()));
        *state.link.borrow_mut() = DisplayLink::new(&view, &state.tick);
        if state.link.borrow().is_none() {
            log("kompoz: macOS: no display link: frames paced by the app's clock alone");
        }
        if spec.minimized {
            window.orderFront(None);
            window.miniaturize(None);
        } else if spec.focusable {
            window.makeKeyAndOrderFront(None);
            #[allow(deprecated)]
            // (macOS 14's `activate` asks rather than takes; a program started from a shell needs this)
            self.shared.app.activateIgnoringOtherApps(true);
        } else {
            window.orderFront(None);
        }
        match spec.placement {
            WindowPlacement::Maximized if !window.isZoomed() => window.zoom(None),
            WindowPlacement::Fullscreen => window.toggleFullScreen(None),
            _ => {}
        }
        state.push(Event::Shown);
        let layer = view.layer().ok_or("the view has no layer")?;
        let size = state.size.clone();
        Ok(Surface {
            metal: Some(MetalSurface {
                layer: NonNull::new(Retained::as_ptr(&layer) as *mut c_void).ok_or("no layer")?,
            }),
            drawable_size: Some(Box::new(move || size.get())),
            pipeline_cache: None,
            transparent: false,
        })
    }

    /// The window gone: its display link stopped, its delegate taken away, closed (the view and the window let go;
    /// the device presenting to its layer went before).
    fn close_window(&mut self) {
        self.state.link.borrow_mut().take();
        self.state.waiting_since.set(None);
        let window = self.state.window.borrow_mut().take();
        let view = self.state.view.borrow_mut().take();
        self.state.last_mouse.borrow_mut().take();
        if let Some(window) = window {
            window.setDelegate(None);
            window.orderOut(None);
            window.close();
        }
        drop(view);
    }

    /// An alert (C++'s `showError`).
    fn show_error(&mut self, message: &str) {
        services::show_error(self.shared.mtm, &self.state.title.borrow(), message);
    }

    /// Points are the view's; the scale (px per dp, px per point) the backing scale or `KOMPOZ_SCALE`; the safe area
    /// what a full-screen window leaves around a camera housing.
    fn metrics(&self) -> Metrics {
        let (w, h) = self.state.size.get();
        let (pw, ph) = self.state.points.get();
        let scale = self.state.px_per_point.get() as f32;
        // (macOS 11's: before it, none but the window.)
        let view = self.state.view().filter(|v| v.respondsToSelector(objc2::sel!(safeAreaInsets)));
        let safe_area = view.map(|v| {
            let insets = v.safeAreaInsets();
            Rect::new(insets.left as f32, insets.top as f32, (pw - insets.right) as f32, (ph - insets.bottom) as f32)
        });
        Metrics {
            pixel_width: w,
            pixel_height: h,
            point_width: pw.round() as i32,
            point_height: ph.round() as i32,
            scale,
            point_to_px: scale,
            font_scale: self
                .option("KOMPOZ_FONT_SCALE")
                .and_then(|v| v.trim().parse::<f32>().ok())
                .map(|s| s.max(0.5))
                .unwrap_or(1.0),
            refresh_rate: self.state.refresh_rate.get(),
            safe_area: safe_area.or(Some(Rect::new(0.0, 0.0, pw as f32, ph as f32))),
            dark_theme: self.shared.dark.get(),
            language: preferred_language().unwrap_or_default(),
            // (The system's title bar: AppKit draws it.)
            title_bar: None,
        }
    }

    fn pixel_size(&self) -> (i32, i32) {
        self.state.size.get()
    }

    /// The content resized to `width` × `height` points.
    fn set_window_size(&mut self, width: i32, height: i32) {
        if let Some(window) = self.window() {
            window.setContentSize(NSSize::new(width.max(1) as f64, height.max(1) as f64));
        }
    }

    /// A trackpad or a mouse is a fine pointer, and a Mac has a keyboard; a microphone and a camera as AVFoundation
    /// finds them (C++'s `media`).
    fn media(&mut self, probe: bool) -> MediaInfo {
        if probe && self.shared.probed.get().is_none() {
            self.shared.probed.set(Some(services::probe_media()));
        }
        let (microphone, camera) = self.shared.probed.get().unwrap_or((false, false));
        MediaInfo {
            pointer_precision: PointerPrecision::Fine,
            physical_keyboard: true,
            has_microphone: microphone,
            has_camera: camera,
            ..MediaInfo::default()
        }
    }

    fn attach(&mut self, _: &KompozHost) -> Rc<dyn PlatformCallbacks> {
        Rc::new(MacCallbacks { shared: self.shared.clone(), state: self.state.clone() })
    }

    /// The accent's palette (the dynamic colors' seed) from the start.
    fn started(&mut self, host: &KompozHost) {
        host.set_dynamic_tonal_palette(self.shared.accent.get().map(DynamicTonalPalette::from_seed));
    }

    /// The accent changed: its palette.
    fn handled(&mut self, host: &KompozHost, event: &Event) {
        if *event == Event::AccentChanged {
            host.set_dynamic_tonal_palette(self.shared.accent.get().map(DynamicTonalPalette::from_seed));
        }
    }

    /// The display kept awake while the host asks (any window's: an IOKit assertion).
    fn composed(&mut self, host: &KompozHost, _redraw: bool, _frame_time: i64) {
        let on = host.keep_screen_on() && self.window().is_some();
        if self.state.keep_on.replace(on) != on {
            self.shared.keep_display_awake();
        }
    }

    fn take_drop_entry(&mut self) -> Option<ClipEntry> {
        self.state.drop_entry.borrow_mut().take()
    }

    fn drop_done(&mut self, taken: bool) {
        self.shared.drop_taken.set(Some(taken));
    }

    /// Reduce motion (Accessibility > Display): the app's animations end at once while it is on.
    fn motion_duration_scale(&self) -> Option<MotionDurationScale> {
        let reduce = self.shared.reduce_motion.clone();
        Some(MotionDurationScale::from_fn(move || if reduce.get() { 0.0 } else { 1.0 }))
    }

    fn set_title(&mut self, title: &str) {
        *self.state.title.borrow_mut() = title.to_string();
        if let Some(window) = self.window() {
            window.setTitle(&NSString::from_str(title));
        }
    }

    /// Zoomed (AppKit's maximized), full screen (its own space), or neither.
    fn set_placement(&mut self, placement: WindowPlacement) {
        let Some(window) = self.window() else { return };
        let full = self.state.placement.get() == WindowPlacement::Fullscreen;
        match placement {
            WindowPlacement::Fullscreen if !full => window.toggleFullScreen(None),
            WindowPlacement::Fullscreen => {}
            _ if full => window.toggleFullScreen(None), // (then zoomed or not as it was before)
            WindowPlacement::Maximized if !window.isZoomed() => window.zoom(None),
            WindowPlacement::Floating if window.isZoomed() => window.zoom(None),
            _ => {}
        }
    }

    fn set_minimized(&mut self, minimized: bool) {
        if let Some(window) = self.window() {
            if minimized {
                window.miniaturize(None);
            } else {
                window.deminiaturize(None);
            }
        }
    }

    /// Its frame's top left to `x`, `y` points from its screen's top left (AppKit's y goes up from the bottom).
    fn set_position(&mut self, x: i32, y: i32) {
        let (Some(window), Some(screen)) = (self.window(), self.screen()) else { return };
        let s = screen.frame();
        window.setFrameTopLeftPoint(NSPoint::new(s.origin.x + x as f64, s.origin.y + s.size.height - y as f64));
    }

    /// Restored if minimized, ordered to the front and made key, the app activated (C++'s `raise`).
    fn to_front(&mut self) {
        let Some(window) = self.window() else { return };
        if window.isMiniaturized() {
            window.deminiaturize(None);
        }
        window.makeKeyAndOrderFront(None);
        #[allow(deprecated)]
        self.shared.app.activateIgnoringOtherApps(true);
    }

    fn set_resizable(&mut self, resizable: bool) {
        let Some(window) = self.window() else { return };
        let mut style = window.styleMask();
        style.set(NSWindowStyleMask::Resizable, resizable);
        window.setStyleMask(style);
    }

    fn set_always_on_top(&mut self, on_top: bool) {
        if let Some(window) = self.window() {
            window.setLevel(if on_top { NSFloatingWindowLevel } else { NSNormalWindowLevel });
        }
    }

    /// The app's icon in the Dock (a Mac's windows show none of their own; none: the bundle's).
    fn set_icon(&mut self, icon: Option<&TrayImage>) {
        let image = icon.and_then(|i| services::image_of_rgba(i.width, i.height, &i.rgba, 0.0));
        // SAFETY: an NSImage (or none, for the bundle's own icon), as the application's icon image may be.
        unsafe { self.shared.app.setApplicationIconImage(image.as_deref()) };
    }

    /// The window's menus in the menu bar while it is key (AppKit's: between the app menu and the Window menu); their
    /// clicks come back as `MenuBarItem`, their shortcuts AppKit's key equivalents.
    fn set_menu_bar(&mut self, menus: Option<&[TrayMenuItem]>) -> crate::app::MenuBarShown {
        *self.state.menus.borrow_mut() = menus.map(|m| m.to_vec());
        self.shared.menus_changed();
        crate::app::MenuBarShown::Native
    }

    /// Its frame's top left, points from its screen's top left.
    fn window_position(&self) -> Option<(i32, i32)> {
        let window = self.window()?;
        let screen = self.screen()?.frame();
        let f = window.frame();
        let top = screen.origin.y + screen.size.height - (f.origin.y + f.size.height);
        Some(((f.origin.x - screen.origin.x).round() as i32, top.round() as i32))
    }

    fn window_placement(&self) -> Option<WindowPlacement> {
        self.window().map(|_| self.state.placement.get())
    }

    /// The screen it is on (the main one before it is open), its size in points.
    fn screen_size(&self) -> Option<(i32, i32)> {
        let f = self.screen()?.frame();
        Some((f.size.width.round() as i32, f.size.height.round() as i32))
    }
}
