//! Graphics tablets (tablet-v2): the seat's tablets, their tools and their pads. A pen's tip or its eraser near the
//! window is a stylus (Compose's PointerType Stylus and Eraser: hovering, touching with its pressure, its barrel
//! buttons held: [`Event::Stylus`]); a tablet's mouse or lens is the mouse, a finger on a tablet a touch. What a
//! tool tells is gathered until its frame ([`ToolInput`]), one hardware event. Its distance, tilt, rotation and
//! slider aren't told on: Compose's pointer changes carry the pressure alone. A pad's buttons are the keys Android's
//! generic key layout makes of them (BTN_0… as `KEYCODE_BUTTON_1`…: Compose's `Key.Button1`…); its rings and strips
//! have no counterpart in Compose and aren't bound. A tool near the window shows a cursor of its own: a pen the
//! stylus's hover icon (Compose's `stylusHoverIcon`, else the arrow), a tablet mouse the pointer's, a finger none;
//! by shape (cursor-shape-v1), else from the cursor theme on a surface of its own (animated as the pointer's).

use super::WaylandPlatform;
use super::cursor::{shape_of, theme_names, theme_size};
use super::ffi::{Connection, CursorTheme, Object, ObjectId};
use super::generated::*;
use super::input::mouse_button;
use crate::app::{Event, MouseButton};
use crate::linux::sys;
use kompoz_ui::input::{Key, KeyModifiers};
use kompoz_ui::platform::PointerIcon;
use std::rc::Rc;

/// linux/input-event-codes.h's barrel buttons of a stylus.
const BTN_STYLUS: u32 = 0x14b;
const BTN_STYLUS2: u32 = 0x14c;
const BTN_STYLUS3: u32 = 0x149;

/// The pointer id of the first tool (the ones after it next): above a touch screen's fingers' (wl_touch's ids).
const FIRST_TOOL_POINTER: i64 = 1 << 32;

/// The keys a pad's buttons are (Android's generic key layout: the first sixteen as `KEYCODE_BUTTON_1`…).
const PAD_KEYS: [Key; 16] = [
    Key::Button1,
    Key::Button2,
    Key::Button3,
    Key::Button4,
    Key::Button5,
    Key::Button6,
    Key::Button7,
    Key::Button8,
    Key::Button9,
    Key::Button10,
    Key::Button11,
    Key::Button12,
    Key::Button13,
    Key::Button14,
    Key::Button15,
    Key::Button16,
];

/// What a tool is used as, by its type.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum ToolKind {
    /// A pen, a brush, a pencil, an airbrush: a stylus.
    #[default]
    Pen,
    /// An eraser (a pen's other end, or one of its own): a stylus's eraser.
    Eraser,
    /// A tablet's mouse or lens cursor: the mouse.
    Mouse,
    /// A finger on the tablet: a touch.
    Finger,
}

impl ToolKind {
    pub fn of(tool_type: ZwpTabletToolV2Type) -> ToolKind {
        type Type = ZwpTabletToolV2Type;
        match tool_type {
            Type::ERASER => ToolKind::Eraser,
            Type::MOUSE | Type::LENS => ToolKind::Mouse,
            Type::FINGER => ToolKind::Finger,
            _ => ToolKind::Pen,
        }
    }
}

/// A stylus's barrel button (evdev's code) as its bit in [`Event::Stylus`]'s buttons (the first, the second, the
/// third: Android's primary, secondary and tertiary stylus buttons).
fn stylus_button(code: u32) -> Option<u32> {
    match code {
        BTN_STYLUS => Some(1),
        BTN_STYLUS2 => Some(2),
        BTN_STYLUS3 => Some(4),
        _ => None,
    }
}

/// A tool as told so far, and what its frame being gathered changed.
#[derive(Clone, Debug, Default)]
pub(super) struct ToolInput {
    pointer: i64,
    pub kind: ToolKind,
    /// Near the window (in proximity of its surface, not out since).
    pub near: bool,
    /// Where it is on the window (surface coordinates: points).
    x: f64,
    y: f64,
    /// Touching the tablet, how hard (0 to 1), a stylus's barrel buttons held.
    down: bool,
    pressure: f32,
    buttons: u32,
    // The frame's:
    came: bool,
    went: bool,
    moved: bool,
    changed: bool,
    /// Contact made (true) and lost (false), in order.
    contacts: Vec<bool>,
    /// A tablet mouse's buttons pressed (true) and let go, in order.
    clicks: Vec<(MouseButton, bool)>,
    /// The wheel turned, notches (up positive).
    wheel: f64,
}

impl ToolInput {
    /// A tool whose events are pointer `pointer`'s.
    pub fn new(pointer: i64) -> ToolInput {
        ToolInput { pointer, ..ToolInput::default() }
    }

    /// It came near the window.
    pub fn proximity_in(&mut self) {
        self.near = true;
        self.came = true;
    }

    /// It went away (out of the tablet's range, or onto another surface).
    pub fn proximity_out(&mut self) {
        if self.near {
            self.went = true;
        }
    }

    pub fn down(&mut self) {
        self.down = true;
        self.contacts.push(true);
    }

    pub fn up(&mut self) {
        self.down = false;
        self.contacts.push(false);
    }

    pub fn motion(&mut self, x: f64, y: f64) {
        (self.x, self.y) = (x, y);
        self.moved = true;
    }

    /// The pressure, of 65535.
    pub fn pressure(&mut self, pressure: u32) {
        self.pressure = (pressure.min(65535) as f32) / 65535.0;
        self.changed = true;
    }

    /// Button `code` (evdev's) pressed or let go: a tablet mouse's mouse button, a stylus's barrel button.
    pub fn button(&mut self, code: u32, pressed: bool) {
        if self.kind == ToolKind::Mouse {
            if let Some(button) = mouse_button(code) {
                self.clicks.push((button, pressed));
            }
        } else if let Some(bit) = stylus_button(code) {
            self.buttons = if pressed { self.buttons | bit } else { self.buttons & !bit };
            self.changed = true;
        }
    }

    /// The wheel turned by `degrees` (wl_pointer's vertical axis' way: down positive): 15° a notch (libinput's).
    pub fn wheel(&mut self, degrees: f64) {
        self.wheel -= degrees / 15.0;
    }

    /// Its frame: what changed, as the core's events at `time` (the keys `modifiers` held), while it is near the
    /// window or as it goes; the next frame begun. A stylus: each contact made or lost (where the frame left it),
    /// else one report if anything changed; its wheel; its going. A tablet mouse: its move, its contacts as the left
    /// button, its buttons, its wheel, its leaving. A finger: down, moves while down, up (as it goes too).
    pub fn frame(&mut self, modifiers: KeyModifiers, time: i64) -> Vec<Event> {
        let mut events = Vec::new();
        let (x, y, pointer) = (self.x as f32, self.y as f32, self.pointer);
        let wheel =
            (self.wheel != 0.0).then_some(Event::Wheel { x, y, dx: 0.0, dy: self.wheel as f32, modifiers, time });
        if self.near {
            match self.kind {
                ToolKind::Pen | ToolKind::Eraser => {
                    let eraser = self.kind == ToolKind::Eraser;
                    let (pressure, buttons) = (self.pressure, self.buttons);
                    let stylus = |down: bool| Event::Stylus { pointer, eraser, x, y, down, pressure, buttons, time };
                    events.extend(self.contacts.iter().map(|&down| stylus(down)));
                    if self.contacts.is_empty() && (self.came || self.moved || self.changed) {
                        events.push(stylus(self.down));
                    }
                    events.extend(wheel);
                    if self.went {
                        events.push(Event::StylusLeft { pointer });
                    }
                }
                ToolKind::Mouse => {
                    if self.came || self.moved {
                        events.push(Event::MouseMove { x, y, modifiers, time });
                    }
                    let contacts = self.contacts.iter().map(|&down| (MouseButton::Left, down));
                    for (button, down) in contacts.chain(self.clicks.iter().copied()) {
                        events.push(if down {
                            Event::MouseDown { x, y, button, modifiers, time }
                        } else {
                            Event::MouseUp { x, y, button, modifiers, time }
                        });
                    }
                    events.extend(wheel);
                    if self.went {
                        events.push(Event::PointerLeft);
                    }
                }
                ToolKind::Finger => {
                    for &down in &self.contacts {
                        events.push(if down {
                            Event::TouchDown { pointer, x, y, time }
                        } else {
                            Event::TouchUp { pointer, x, y, time }
                        });
                    }
                    if self.contacts.is_empty() && self.moved && self.down {
                        events.push(Event::TouchMove { pointer, x, y, time });
                    }
                    if self.went && self.down {
                        events.push(Event::TouchUp { pointer, x, y, time }); // (lifted as it goes)
                    }
                }
            }
        }
        if self.went {
            self.near = false;
            self.down = false;
            self.buttons = 0;
        }
        self.came = false;
        self.went = false;
        self.moved = false;
        self.changed = false;
        self.contacts.clear();
        self.clicks.clear();
        self.wheel = 0.0;
        events
    }
}

/// A tool: its object, what it tells, its cursor.
struct Tool {
    /// cursor-shape-v1's device for it (made before it goes).
    shape: Option<WpCursorShapeDeviceV1>,
    /// Else a surface of its own for the theme's cursors, and an animated one's start and next frame (ns).
    themed: Option<WlSurface>,
    animation: Option<(i64, i64)>,
    /// Its proximity_in's serial while it is near the window (what its cursor is set with).
    serial: u32,
    input: ToolInput,
    tool: ZwpTabletToolV2,
}

/// A pad: on the window (its keyboard-like focus) or not, its buttons held.
struct Pad {
    on_window: bool,
    held: Vec<u32>,
    pad: ZwpTabletPadV2,
}

/// The seat's tablets, tools and pads, and the tools' cursors, as the platform and the host's callbacks share them.
pub(crate) struct Tablets {
    connection: Connection,
    tools: Vec<Tool>,
    pads: Vec<Pad>,
    tablets: Vec<ZwpTabletV2>,
    pub seat: Option<ZwpTabletSeatV2>,
    compositor: Option<Rc<WlCompositor>>,
    shm: Option<Rc<WlShm>>,
    /// The cursor theme at the scale it was loaded for (after the tools' surfaces: their buffers are its).
    theme: Option<(CursorTheme, i32)>,
    /// The window's scale, whole.
    scale: i32,
    /// The host's: a hovering stylus's icon (none: the arrow), the pointer's.
    stylus_icon: Option<PointerIcon>,
    pointer_icon: PointerIcon,
    next_pointer: i64,
}

impl Tablets {
    pub fn new(connection: Connection) -> Tablets {
        Tablets {
            connection,
            tools: Vec::new(),
            pads: Vec::new(),
            tablets: Vec::new(),
            seat: None,
            compositor: None,
            shm: None,
            theme: None,
            scale: 1,
            stylus_icon: None,
            pointer_icon: PointerIcon::Default,
            next_pointer: FIRST_TOOL_POINTER,
        }
    }

    /// The seat's tablets asked for (once there are a manager and a seat), with what the tools' cursors are drawn
    /// with where there is no cursor-shape-v1.
    pub fn attach(
        &mut self,
        manager: Option<&ZwpTabletManagerV2>,
        seat: &WlSeat,
        compositor: Option<Rc<WlCompositor>>,
        shm: Option<Rc<WlShm>>,
    ) {
        self.compositor = self.compositor.take().or(compositor);
        self.shm = self.shm.take().or(shm);
        if let Some(manager) = manager
            && self.seat.is_none()
        {
            self.seat = manager.get_tablet_seat(seat);
        }
    }

    /// A tool came into use: its cursor's means made (a shape device, else a surface).
    fn add_tool(&mut self, tool: ZwpTabletToolV2, shapes: Option<&WpCursorShapeManagerV1>) {
        let shape = shapes.and_then(|s| s.get_tablet_tool_v2(&tool));
        let themed = if shape.is_none() { self.compositor.as_ref().and_then(|c| c.create_surface()) } else { None };
        let input = ToolInput::new(self.next_pointer);
        self.next_pointer += 1;
        self.tools.push(Tool { shape, themed, animation: None, serial: 0, input, tool });
    }

    /// The host's stylus hover icon (none: the platform's own, the arrow): shown on the pens near the window.
    pub fn set_stylus_icon(&mut self, icon: Option<PointerIcon>) {
        if icon != self.stylus_icon {
            self.stylus_icon = icon;
            self.show_all();
        }
    }

    /// The host's cursor: shown on the tablet mice near the window.
    pub fn set_pointer_icon(&mut self, icon: PointerIcon) {
        if icon != self.pointer_icon {
            self.pointer_icon = icon;
            self.show_all();
        }
    }

    /// The window's scale (whole): the theme's cursors drawn again at it.
    pub fn set_scale(&mut self, scale: i32) {
        let scale = scale.max(1);
        if scale != self.scale {
            self.scale = scale;
            self.show_all();
        }
    }

    fn show_all(&mut self) {
        for i in 0..self.tools.len() {
            self.show(i);
        }
    }

    /// Tool `i`'s cursor shown, while it is near the window.
    fn show(&mut self, i: usize) {
        let tool = &self.tools[i];
        if tool.serial == 0 {
            return;
        }
        let icon = match tool.input.kind {
            ToolKind::Pen | ToolKind::Eraser => self.stylus_icon.unwrap_or_default(),
            ToolKind::Mouse => self.pointer_icon,
            ToolKind::Finger => {
                tool.tool.set_cursor(tool.serial, None, 0, 0);
                return;
            }
        };
        if let Some(device) = &tool.shape {
            device.set_shape(tool.serial, shape_of(icon));
            return;
        }
        let start = sys::monotonic_now();
        self.draw(i, icon, start, start);
    }

    /// Tool `i`'s cursor `icon` drawn from the theme as it is at `now`, its animation begun at `start`.
    fn draw(&mut self, i: usize, icon: PointerIcon, start: i64, now: i64) {
        let Some(shm) = &self.shm else { return };
        let scale = self.scale;
        if self.theme.as_ref().is_none_or(|(_, s)| *s != scale) {
            let name = std::env::var("XCURSOR_THEME").ok().filter(|n| !n.is_empty());
            self.theme = match CursorTheme::load(&self.connection, name.as_deref(), theme_size() * scale, shm) {
                Ok(theme) => Some((theme, scale)),
                Err(why) => {
                    crate::system::log(&format!("kompoz: Wayland: no cursor theme for the tablet's tools ({why})"));
                    None
                }
            };
        }
        let (Some((theme, _)), Some(tool)) = (&self.theme, self.tools.get_mut(i)) else { return };
        let Some(surface) = &tool.themed else { return };
        let Some(cursor) = theme_names(icon).iter().find_map(|n| theme.cursor(n)) else { return };
        let elapsed_ms = ((now - start).max(0) / 1_000_000) as u32;
        let (frame, duration) = cursor.frame_at(elapsed_ms);
        let Some(image) = cursor.image(frame) else { return };
        surface.attach(Some(image.buffer()), 0, 0);
        surface.set_buffer_scale(scale);
        surface.damage_buffer(0, 0, image.width, image.height);
        surface.commit();
        tool.tool.set_cursor(tool.serial, Some(surface), image.hotspot_x / scale, image.hotspot_y / scale);
        tool.animation =
            (cursor.image_count() > 1 && duration > 0).then(|| (start, now + i64::from(duration) * 1_000_000));
    }

    /// When the next frame of a tool's animated cursor is due (monotonic ns).
    pub fn next_frame_at(&self) -> Option<i64> {
        self.tools.iter().filter_map(|t| t.animation.map(|(_, next)| next)).min()
    }

    /// The animated cursors' frames shown that are due by `now`.
    pub fn animate(&mut self, now: i64) {
        for i in 0..self.tools.len() {
            let tool = &self.tools[i];
            let Some((start, next)) = tool.animation else { continue };
            if now < next || tool.serial == 0 {
                continue;
            }
            let icon = match tool.input.kind {
                ToolKind::Mouse => self.pointer_icon,
                _ => self.stylus_icon.unwrap_or_default(),
            };
            self.draw(i, icon, start, now);
        }
        let _ = self.connection.flush();
    }
}

impl WaylandPlatform {
    /// The tablet seat's news: a tablet, a tool, a pad come into use.
    pub(super) fn on_tablet_seat(&mut self, e: ZwpTabletSeatV2Event) {
        let mut tablets = self.shared.tablet.borrow_mut();
        match e {
            ZwpTabletSeatV2Event::TabletAdded { id } => tablets.tablets.push(id),
            ZwpTabletSeatV2Event::ToolAdded { id } => tablets.add_tool(id, self.globals.cursor_shapes.as_ref()),
            ZwpTabletSeatV2Event::PadAdded { id } => {
                tablets.pads.push(Pad { on_window: false, held: Vec::new(), pad: id })
            }
        }
    }

    /// A tablet's description (its name, ids and device path: nothing KompozR uses), or its going.
    pub(super) fn on_tablet(&mut self, sender: ObjectId, e: ZwpTabletV2Event) {
        if let ZwpTabletV2Event::Removed = e {
            self.shared.tablet.borrow_mut().tablets.retain(|t| t.id() != sender);
        }
    }

    /// A tool's event, gathered until its frame (what it is used as, from its type; its cursor shown as it comes
    /// near the window, on the window's surface only). Its hardware serial and id, capabilities, distance, tilt,
    /// rotation and slider aren't told on (Compose's pointer events have no place for them).
    pub(super) fn on_tablet_tool(&mut self, sender: ObjectId, e: ZwpTabletToolV2Event) {
        type E = ZwpTabletToolV2Event;
        let window = self.win.window.as_ref().map(|w| w.surface.id());
        let modifiers = self.keyboard.modifiers();
        let mut tablets = self.shared.tablet.borrow_mut();
        let Some(i) = tablets.tools.iter().position(|t| t.tool.id() == sender) else { return };
        let tool = &mut tablets.tools[i];
        let events = match e {
            E::Type { tool_type } => {
                tool.input.kind = ToolKind::of(tool_type);
                Vec::new()
            }
            E::ProximityIn { serial, surface, .. } => {
                if window == Some(surface) {
                    tool.input.proximity_in();
                    tool.serial = serial;
                    tablets.show(i);
                }
                Vec::new()
            }
            E::ProximityOut => {
                tool.input.proximity_out();
                tool.serial = 0;
                tool.animation = None;
                Vec::new()
            }
            E::Down { .. } => {
                tool.input.down();
                Vec::new()
            }
            E::Up => {
                tool.input.up();
                Vec::new()
            }
            E::Motion { x, y } => {
                tool.input.motion(x, y);
                Vec::new()
            }
            E::Pressure { pressure } => {
                tool.input.pressure(pressure);
                Vec::new()
            }
            E::Button { button, state, .. } => {
                tool.input.button(button, state == ZwpTabletToolV2ButtonState::PRESSED);
                Vec::new()
            }
            E::Wheel { degrees, .. } => {
                tool.input.wheel(degrees);
                Vec::new()
            }
            E::Frame { .. } => tool.input.frame(modifiers, sys::monotonic_now()),
            E::Removed => {
                // (Gone near the window: it leaves first.)
                tool.input.proximity_out();
                let events = tool.input.frame(modifiers, sys::monotonic_now());
                tablets.tools.remove(i);
                events
            }
            E::HardwareSerial { .. }
            | E::HardwareIdWacom { .. }
            | E::Capability { .. }
            | E::Done
            | E::Distance { .. }
            | E::Tilt { .. }
            | E::Rotation { .. }
            | E::Slider { .. } => Vec::new(),
        };
        drop(tablets);
        self.win.events.extend(events);
    }

    /// A pad's event: its buttons as keys while it is on the window (its focus follows the keyboard's), held ones
    /// let go as it leaves or goes. Its groups (rings, strips, modes) are let go as they come: Compose has nothing
    /// they could be.
    pub(super) fn on_tablet_pad(&mut self, sender: ObjectId, e: ZwpTabletPadV2Event) {
        let window = self.win.window.as_ref().map(|w| w.surface.id());
        let modifiers = self.keyboard.modifiers();
        let time = sys::monotonic_now();
        let key = |button: u32, down: bool| {
            let key = *PAD_KEYS.get(button as usize)?;
            Some(if down {
                Event::KeyDown { key, code_point: 0, modifiers, time }
            } else {
                Event::KeyUp { key, code_point: 0, modifiers, time }
            })
        };
        let mut tablets = self.shared.tablet.borrow_mut();
        let Some(i) = tablets.pads.iter().position(|p| p.pad.id() == sender) else { return };
        let pad = &mut tablets.pads[i];
        let mut events = Vec::new();
        match e {
            ZwpTabletPadV2Event::Button { button, state, .. } => {
                let down = state == ZwpTabletPadV2ButtonState::PRESSED;
                let held = pad.held.contains(&button);
                if pad.on_window && down != held {
                    if down {
                        pad.held.push(button);
                    } else {
                        pad.held.retain(|b| *b != button);
                    }
                    events.extend(key(button, down));
                }
            }
            ZwpTabletPadV2Event::Enter { surface, .. } => pad.on_window = window == Some(surface),
            ZwpTabletPadV2Event::Leave { .. } | ZwpTabletPadV2Event::Removed => {
                pad.on_window = false;
                events.extend(std::mem::take(&mut pad.held).into_iter().filter_map(|b| key(b, false)));
                if let ZwpTabletPadV2Event::Removed = e {
                    tablets.pads.remove(i);
                }
            }
            ZwpTabletPadV2Event::Group { .. }
            | ZwpTabletPadV2Event::Path { .. }
            | ZwpTabletPadV2Event::Buttons { .. }
            | ZwpTabletPadV2Event::Done => {}
        }
        drop(tablets);
        self.win.events.extend(events);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: KeyModifiers = KeyModifiers::None;

    fn stylus(pointer: i64, eraser: bool, x: f32, y: f32, down: bool, pressure: f32, buttons: u32) -> Event {
        Event::Stylus { pointer, eraser, x, y, down, pressure, buttons, time: 7 }
    }

    #[test]
    fn tools_are_what_their_types_say() {
        type Type = ZwpTabletToolV2Type;
        for (t, kind) in [
            (Type::PEN, ToolKind::Pen),
            (Type::BRUSH, ToolKind::Pen),
            (Type::PENCIL, ToolKind::Pen),
            (Type::AIRBRUSH, ToolKind::Pen),
            (Type::ERASER, ToolKind::Eraser),
            (Type::MOUSE, ToolKind::Mouse),
            (Type::LENS, ToolKind::Mouse),
            (Type::FINGER, ToolKind::Finger),
        ] {
            assert_eq!(ToolKind::of(t), kind);
        }
        assert_eq!(stylus_button(BTN_STYLUS), Some(1));
        assert_eq!(stylus_button(BTN_STYLUS2), Some(2));
        assert_eq!(stylus_button(BTN_STYLUS3), Some(4));
        assert_eq!(stylus_button(0x110), None);
    }

    #[test]
    fn a_pens_frames_are_stylus_reports() {
        let mut t = ToolInput::new(40);
        // Told nothing before it comes near the window.
        t.motion(1.0, 1.0);
        assert_eq!(t.frame(NONE, 7), []);
        // Near: hovering where it is; a frame with nothing new: nothing.
        t.proximity_in();
        t.motion(10.0, 20.0);
        t.pressure(0);
        assert_eq!(t.frame(NONE, 7), [stylus(40, false, 10.0, 20.0, false, 0.0, 0)]);
        assert_eq!(t.frame(NONE, 7), []);
        // Touching, its pressure (of 65535) and where the frame left it; pressing harder, its barrel button.
        t.down();
        t.pressure(32768);
        t.motion(11.0, 20.0);
        let half = 32768.0 / 65535.0;
        assert_eq!(t.frame(NONE, 7), [stylus(40, false, 11.0, 20.0, true, half, 0)]);
        t.pressure(65535);
        t.button(BTN_STYLUS2, true);
        assert_eq!(t.frame(NONE, 7), [stylus(40, false, 11.0, 20.0, true, 1.0, 2)]);
        // A tap within one frame: touched and lifted, both told.
        t.up();
        t.down();
        t.button(BTN_STYLUS2, false);
        assert_eq!(
            t.frame(NONE, 7),
            [stylus(40, false, 11.0, 20.0, false, 1.0, 0), stylus(40, false, 11.0, 20.0, true, 1.0, 0)]
        );
        // Its wheel (15° a notch, down as wl_pointer's), lifted and gone in one frame.
        t.wheel(30.0);
        t.up();
        t.pressure(0);
        t.proximity_out();
        assert_eq!(
            t.frame(NONE, 7),
            [
                stylus(40, false, 11.0, 20.0, false, 0.0, 0),
                Event::Wheel { x: 11.0, y: 20.0, dx: 0.0, dy: -2.0, modifiers: NONE, time: 7 },
                Event::StylusLeft { pointer: 40 }
            ]
        );
        assert!(!t.near);
        assert_eq!(t.frame(NONE, 7), []);
        // Gone onto another surface while touching (no up): it goes; touching no more.
        t.proximity_in();
        t.down();
        assert_eq!(t.frame(NONE, 7), [stylus(40, false, 11.0, 20.0, true, 0.0, 0)]);
        t.proximity_out();
        assert_eq!(t.frame(NONE, 7), [Event::StylusLeft { pointer: 40 }]);
        assert!(!t.down);
        // An eraser.
        let mut e = ToolInput { kind: ToolKind::Eraser, ..ToolInput::new(41) };
        e.proximity_in();
        e.down();
        e.motion(3.0, 4.0);
        assert_eq!(e.frame(NONE, 7), [stylus(41, true, 3.0, 4.0, true, 0.0, 0)]);
    }

    #[test]
    fn a_tablet_mouse_is_the_mouse_and_a_finger_a_touch() {
        let ctrl = KeyModifiers::Ctrl;
        let mut m = ToolInput { kind: ToolKind::Mouse, ..ToolInput::new(50) };
        m.proximity_in();
        m.motion(5.0, 6.0);
        m.button(0x111, true);
        m.button(BTN_STYLUS, true); // (a stylus's button: not a mouse's)
        m.wheel(-15.0);
        assert_eq!(
            m.frame(ctrl, 7),
            [
                Event::MouseMove { x: 5.0, y: 6.0, modifiers: ctrl, time: 7 },
                Event::MouseDown { x: 5.0, y: 6.0, button: MouseButton::Right, modifiers: ctrl, time: 7 },
                Event::Wheel { x: 5.0, y: 6.0, dx: 0.0, dy: 1.0, modifiers: ctrl, time: 7 }
            ]
        );
        m.button(0x111, false);
        m.down();
        m.proximity_out();
        assert_eq!(
            m.frame(NONE, 7),
            [
                Event::MouseDown { x: 5.0, y: 6.0, button: MouseButton::Left, modifiers: NONE, time: 7 },
                Event::MouseUp { x: 5.0, y: 6.0, button: MouseButton::Right, modifiers: NONE, time: 7 },
                Event::PointerLeft
            ]
        );
        let mut f = ToolInput { kind: ToolKind::Finger, ..ToolInput::new(60) };
        f.proximity_in();
        f.motion(1.0, 2.0);
        assert_eq!(f.frame(NONE, 7), []); // (a finger doesn't hover)
        f.down();
        assert_eq!(f.frame(NONE, 7), [Event::TouchDown { pointer: 60, x: 1.0, y: 2.0, time: 7 }]);
        f.motion(3.0, 2.0);
        assert_eq!(f.frame(NONE, 7), [Event::TouchMove { pointer: 60, x: 3.0, y: 2.0, time: 7 }]);
        f.proximity_out(); // (gone while down: lifted)
        assert_eq!(f.frame(NONE, 7), [Event::TouchUp { pointer: 60, x: 3.0, y: 2.0, time: 7 }]);
    }

    #[test]
    fn pad_buttons_are_androids_generic_buttons() {
        assert_eq!(PAD_KEYS[0], Key::Button1);
        assert_eq!(PAD_KEYS[15], Key::Button16);
        assert!(PAD_KEYS.windows(2).all(|w| w[0] != w[1]));
    }
}
