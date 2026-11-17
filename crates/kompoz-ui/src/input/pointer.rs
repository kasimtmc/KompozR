//! The pointer model, as Compose's `PointerEvent.kt`: events of changes (one per pointer) whose consumption is
//! shared by every handler that sees them, the passes they go through, buttons and keyboard modifiers, and the
//! change helpers (`changed_to_down`, `position_change`, `is_out_of_bounds`, …).

use kompoz_graphics::{IntSize, Offset, Size};
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

/// Which pointer a change is of (Compose's PointerId): unique while the pointer is down (a finger's from its
/// press to its release), the mouse's while it is over the window.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct PointerId(pub i64);

impl PointerId {
    pub const fn value(self) -> i64 {
        self.0
    }
}

/// What kind of pointer it is (Compose's PointerType).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PointerType {
    #[default]
    Unknown,
    Touch,
    Mouse,
    Stylus,
    Eraser,
}

/// What happened (Compose's PointerEventType). A node's own Enter and Exit are worked out from where the pointer
/// is relative to it; the platform's Move becomes them.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PointerEventType {
    #[default]
    Unknown,
    Press,
    Release,
    Move,
    Enter,
    Exit,
    Scroll,
    ScaleStart,
    ScaleChange,
    ScaleEnd,
    PanStart,
    PanMove,
    PanEnd,
}

/// The pass an event goes round the handlers under the pointer in (Compose's PointerEventPass): Initial from the
/// outermost in (a parent sees, or consumes, it before its children), Main from the innermost out (where gestures
/// usually react), Final from the outermost in again (to see what the others consumed).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PointerEventPass {
    Initial,
    #[default]
    Main,
    Final,
}

/// A mouse button (Compose desktop's PointerButton): its index.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PointerButton(pub u32);

impl PointerButton {
    pub const Primary: PointerButton = PointerButton(0);
    pub const Secondary: PointerButton = PointerButton(1);
    pub const Tertiary: PointerButton = PointerButton(2);
    pub const Back: PointerButton = PointerButton(3);
    pub const Forward: PointerButton = PointerButton(4);
    pub const fn index(self) -> u32 {
        self.0
    }
}

/// The buttons pressed (Compose's PointerButtons), a bit per [`PointerButton`] index.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PointerButtons(pub u32);

impl PointerButtons {
    pub const None: PointerButtons = PointerButtons(0);
    /// Those `buttons` (a mouse's buttons pressed).
    pub fn of(buttons: &[PointerButton]) -> PointerButtons {
        PointerButtons(buttons.iter().fold(0, |b, p| b | 1 << p.0))
    }
    pub fn is_primary_pressed(self) -> bool {
        self.is_pressed(0)
    }
    pub fn is_secondary_pressed(self) -> bool {
        self.is_pressed(1)
    }
    pub fn is_tertiary_pressed(self) -> bool {
        self.is_pressed(2)
    }
    pub fn is_back_pressed(self) -> bool {
        self.is_pressed(3)
    }
    pub fn is_forward_pressed(self) -> bool {
        self.is_pressed(4)
    }
    /// Whether the button of index `button_index` is pressed (Compose's `isPressed`).
    pub fn is_pressed(self, button_index: u32) -> bool {
        button_index < 32 && self.0 & (1 << button_index) != 0
    }
    pub fn are_any_pressed(self) -> bool {
        self.0 != 0
    }
    /// The index of the first button pressed, -1 if none (Compose's `indexOfFirstPressed`).
    pub fn index_of_first_pressed(self) -> i32 {
        if self.0 == 0 { -1 } else { self.0.trailing_zeros() as i32 }
    }
    /// The index of the last button pressed, -1 if none.
    pub fn index_of_last_pressed(self) -> i32 {
        31 - self.0.leading_zeros() as i32
    }
    pub(crate) fn with(self, button: PointerButton, pressed: bool) -> PointerButtons {
        if pressed { PointerButtons(self.0 | 1 << button.0) } else { PointerButtons(self.0 & !(1 << button.0)) }
    }
}

impl fmt::Debug for PointerButtons {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PointerButtons({:#b})", self.0)
    }
}

/// The keyboard modifiers held while a pointer event happened (Compose's PointerKeyboardModifiers): the bits of
/// [`KeyModifiers`](super::KeyModifiers), and the lock and other keys above them.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PointerKeyboardModifiers(pub u32);

impl PointerKeyboardModifiers {
    pub const Shift: u32 = 1;
    pub const Ctrl: u32 = 1 << 1;
    pub const Alt: u32 = 1 << 2;
    pub const Meta: u32 = 1 << 3;
    pub const AltGraph: u32 = 1 << 4;
    pub const Sym: u32 = 1 << 5;
    pub const Function: u32 = 1 << 6;
    pub const CapsLock: u32 = 1 << 7;
    pub const ScrollLock: u32 = 1 << 8;
    pub const NumLock: u32 = 1 << 9;

    pub fn is_ctrl_pressed(self) -> bool {
        self.0 & Self::Ctrl != 0
    }
    pub fn is_meta_pressed(self) -> bool {
        self.0 & Self::Meta != 0
    }
    pub fn is_alt_pressed(self) -> bool {
        self.0 & Self::Alt != 0
    }
    pub fn is_alt_graph_pressed(self) -> bool {
        self.0 & Self::AltGraph != 0
    }
    pub fn is_sym_pressed(self) -> bool {
        self.0 & Self::Sym != 0
    }
    pub fn is_shift_pressed(self) -> bool {
        self.0 & Self::Shift != 0
    }
    pub fn is_function_pressed(self) -> bool {
        self.0 & Self::Function != 0
    }
    pub fn is_caps_lock_on(self) -> bool {
        self.0 & Self::CapsLock != 0
    }
    pub fn is_scroll_lock_on(self) -> bool {
        self.0 & Self::ScrollLock != 0
    }
    pub fn is_num_lock_on(self) -> bool {
        self.0 & Self::NumLock != 0
    }
}

impl From<super::KeyModifiers> for PointerKeyboardModifiers {
    fn from(m: super::KeyModifiers) -> Self {
        PointerKeyboardModifiers(m.0)
    }
}

impl fmt::Debug for PointerKeyboardModifiers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PointerKeyboardModifiers({:#b})", self.0)
    }
}

/// What the platform made of the gesture (Android's MotionEvent classification, which Compose's PointerEvent
/// carries): a deep press is a long press at once.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PointerEventClassification {
    #[default]
    None,
    AmbiguousGesture,
    DeepPress,
    TwoFingerSwipe,
    Pinch,
}

/// A position a pointer passed through between two events (Compose's HistoricalChange).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct HistoricalChange {
    pub uptime_millis: i64,
    pub position: Offset,
    pub scale_factor: f32,
    pub pan_offset: Offset,
    /// Where it was in the window (what a velocity tracker follows: not moved by layout under the pointer).
    pub(crate) original_event_position: Offset,
}

impl HistoricalChange {
    pub fn new(uptime_millis: i64, position: Offset) -> HistoricalChange {
        HistoricalChange {
            uptime_millis,
            position,
            scale_factor: 1.0,
            pan_offset: Offset::Zero,
            original_event_position: position,
        }
    }
}

/// One pointer in an event (Compose's PointerInputChange): where it is and was, whether it is pressed and was,
/// when. Its consumption is shared by its clones — the copies every handler under the pointer gets, positions
/// local to each — so a handler that consumes it tells all the others (Compose's shallow `copy`).
#[derive(Clone, kompoz_runtime::KotlinProperties)]
pub struct PointerInputChange {
    pub id: PointerId,
    pub uptime_millis: i64,
    pub position: Offset,
    pub pressed: bool,
    pub pressure: f32,
    pub previous_uptime_millis: i64,
    pub previous_position: Offset,
    pub previous_pressed: bool,
    pub r#type: PointerType,
    /// A scroll event's wheel or touchpad delta, in notches (Compose's units: down and right positive; a touchpad's
    /// or a free-spinning wheel's parts of notches; foundation's scrollables make a notch 64 dp).
    pub scroll_delta: Offset,
    /// A trackpad pinch's scale (Compose's `scaleFactor`).
    pub scale_factor: f32,
    /// A trackpad pinch's twist, degrees clockwise (KompozR's, as C++'s `zoom` rotation: Compose has none).
    pub rotation: f32,
    /// A trackpad pan's offset (Compose's `panOffset`, Android's gesture scroll distance: the opposite of the
    /// fingers' movement).
    pub pan_offset: Offset,
    historical: Rc<Vec<HistoricalChange>>,
    /// Where it is in the window.
    pub(crate) original_event_position: Offset,
    consumed: Rc<Cell<bool>>,
}

impl PointerInputChange {
    /// A change of its own (consumed with no other), pressure 1, a touch (Compose's constructor).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: PointerId,
        uptime_millis: i64,
        position: Offset,
        pressed: bool,
        previous_uptime_millis: i64,
        previous_position: Offset,
        previous_pressed: bool,
        is_initially_consumed: bool,
    ) -> PointerInputChange {
        PointerInputChange {
            id,
            uptime_millis,
            position,
            pressed,
            pressure: 1.0,
            previous_uptime_millis,
            previous_position,
            previous_pressed,
            r#type: PointerType::Touch,
            scroll_delta: Offset::Zero,
            scale_factor: 1.0,
            rotation: 0.0,
            pan_offset: Offset::Zero,
            historical: Rc::new(Vec::new()),
            original_event_position: position,
            consumed: Rc::new(Cell::new(is_initially_consumed)),
        }
    }

    /// The same of another pointer type.
    pub fn with_type(mut self, r#type: PointerType) -> PointerInputChange {
        self.r#type = r#type;
        self
    }

    /// The same with the positions it passed through since the last event.
    pub fn with_historical(mut self, historical: Vec<HistoricalChange>) -> PointerInputChange {
        self.historical = Rc::new(historical);
        self
    }

    /// The positions it passed through since the last event, oldest first (Compose's `historical`).
    pub fn historical(&self) -> &[HistoricalChange] {
        &self.historical
    }

    /// Whether a handler took it (Compose's `isConsumed`).
    pub fn is_consumed(&self) -> bool {
        self.consumed.get()
    }

    /// Takes it: the other handlers (and scrolling, clicks) see it taken (Compose's `consume`).
    pub fn consume(&self) {
        self.consumed.set(true);
    }

    /// Whether it is the same change as `other` (a copy of it: consumption shared).
    pub fn shares_consumption_with(&self, other: &PointerInputChange) -> bool {
        Rc::ptr_eq(&self.consumed, &other.consumed)
    }

    /// A copy at other positions, consumption shared (what each handler gets).
    pub(crate) fn moved_to(
        &self,
        previous_position: Offset,
        position: Offset,
        historical: Vec<HistoricalChange>,
    ) -> Self {
        PointerInputChange { previous_position, position, historical: Rc::new(historical), ..self.clone() }
    }

    pub(crate) fn set_original_event_position(&mut self, position: Offset) {
        self.original_event_position = position;
    }

    // ---- Compose's change helpers ----

    /// Pressed now, not before, and not consumed (Compose's `changedToDown`).
    pub fn changed_to_down(&self) -> bool {
        !self.is_consumed() && !self.previous_pressed && self.pressed
    }
    pub fn changed_to_down_ignore_consumed(&self) -> bool {
        !self.previous_pressed && self.pressed
    }
    /// Let go now, not consumed (Compose's `changedToUp`).
    pub fn changed_to_up(&self) -> bool {
        !self.is_consumed() && self.previous_pressed && !self.pressed
    }
    pub fn changed_to_up_ignore_consumed(&self) -> bool {
        self.previous_pressed && !self.pressed
    }
    /// Whether it moved (and isn't consumed).
    pub fn position_changed(&self) -> bool {
        self.position_change_internal(false) != Offset::Zero
    }
    pub fn position_changed_ignore_consumed(&self) -> bool {
        self.position_change_internal(true) != Offset::Zero
    }
    /// How far it moved; nothing if it is consumed (Compose's `positionChange`).
    pub fn position_change(&self) -> Offset {
        self.position_change_internal(false)
    }
    pub fn position_change_ignore_consumed(&self) -> Offset {
        self.position_change_internal(true)
    }
    fn position_change_internal(&self, ignore_consumed: bool) -> Offset {
        let offset = self.position - self.previous_position;
        if !ignore_consumed && self.is_consumed() { Offset::Zero } else { offset }
    }

    /// Whether it is outside `size` and, for a finger, outside what `extended_touch_padding` adds round it (the
    /// minimum touch target: Compose's `isOutOfBounds(size, extendedTouchPadding)`).
    pub fn is_out_of_bounds(&self, size: IntSize, extended_touch_padding: Size) -> bool {
        let touch = if self.r#type == PointerType::Touch { 1.0 } else { 0.0 };
        let (x, y) = (self.position.x, self.position.y);
        let extent_x = extended_touch_padding.width * touch;
        let extent_y = extended_touch_padding.height * touch;
        x < -extent_x || x > size.width as f32 + extent_x || y < -extent_y || y > size.height as f32 + extent_y
    }

    /// Whether it is outside `size` (Compose's older `isOutOfBounds(size)`).
    pub(crate) fn is_outside(&self, size: IntSize) -> bool {
        let (x, y) = (self.position.x, self.position.y);
        x < 0.0 || x > size.width as f32 || y < 0.0 || y > size.height as f32
    }
}

impl fmt::Debug for PointerInputChange {
    /// Compose's toString.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "PointerInputChange(id={}, uptimeMillis={}, position={:?}, pressed={}, pressure={}, \
             previousUptimeMillis={}, previousPosition={:?}, previousPressed={}, isConsumed={}, type={:?}, \
             historical={:?}, scrollDelta={:?}, scaleFactor={}, panOffset={:?})",
            self.id.0,
            self.uptime_millis,
            self.position,
            self.pressed,
            self.pressure,
            self.previous_uptime_millis,
            self.previous_position,
            self.previous_pressed,
            self.is_consumed(),
            self.r#type,
            self.historical(),
            self.scroll_delta,
            self.scale_factor,
            self.pan_offset
        )
    }
}

/// A pointer as the platform reported it in an event (Compose's PointerInputEventData).
#[derive(Clone, Debug)]
pub(crate) struct PointerInputEventData {
    pub(crate) id: PointerId,
    pub(crate) uptime: i64,
    pub(crate) position: Offset,
    pub(crate) down: bool,
    pub(crate) pressure: f32,
    pub(crate) r#type: PointerType,
    /// A mouse over the window, pressed or not: its events are one hover stream (Compose's `activeHover`).
    pub(crate) active_hover: bool,
    pub(crate) historical: Vec<HistoricalChange>,
    pub(crate) scroll_delta: Offset,
    pub(crate) scale_factor: f32,
    pub(crate) rotation: f32,
    pub(crate) pan_offset: Offset,
}

/// What the platform sent (Compose's PointerInputEvent): its time, its pointers, and the rest of what a
/// [`PointerEvent`] carries.
#[derive(Clone, Debug)]
pub(crate) struct PointerInputEvent {
    pub(crate) pointers: Vec<PointerInputEventData>,
    pub(crate) r#type: PointerEventType,
    pub(crate) buttons: PointerButtons,
    pub(crate) button: Option<PointerButton>,
    pub(crate) keyboard_modifiers: PointerKeyboardModifiers,
    pub(crate) classification: PointerEventClassification,
}

/// An event being dispatched with its changes (Compose's InternalPointerEvent), shared by the events each node
/// gets.
pub(crate) struct InternalPointerEvent {
    /// The changes, by pointer id (Compose's LongSparseArray: sorted by id).
    pub(crate) changes: Vec<PointerInputChange>,
    pub(crate) event: PointerInputEvent,
    pub(crate) suppress_movement_consumption: Cell<bool>,
}

impl InternalPointerEvent {
    /// Whether the pointer is of a hover stream (Compose's `activeHoverEvent`).
    pub(crate) fn active_hover_event(&self, id: PointerId) -> bool {
        self.event.pointers.iter().find(|p| p.id == id).is_some_and(|p| p.active_hover)
    }
}

/// An event: the changes of the pointers in it (every pointer down, the ones that didn't change too), what
/// happened, the buttons and keyboard modifiers (Compose's PointerEvent; `button` is desktop's).
#[derive(Clone, kompoz_runtime::KotlinProperties)]
pub struct PointerEvent {
    pub changes: Vec<PointerInputChange>,
    pub buttons: PointerButtons,
    pub keyboard_modifiers: PointerKeyboardModifiers,
    pub r#type: PointerEventType,
    /// The button pressed or let go, for a mouse's Press and Release.
    pub button: Option<PointerButton>,
    pub classification: PointerEventClassification,
}

impl PointerEvent {
    /// An event of `changes`, its type worked out from them (Compose's `PointerEvent(changes)`): Release if one
    /// changed to up, Press if one changed to down, Move otherwise.
    pub fn new(changes: Vec<PointerInputChange>) -> PointerEvent {
        let mut r#type = PointerEventType::Move;
        for c in &changes {
            if c.changed_to_up_ignore_consumed() {
                r#type = PointerEventType::Release;
                break;
            }
            if c.changed_to_down_ignore_consumed() {
                r#type = PointerEventType::Press;
                break;
            }
        }
        PointerEvent {
            changes,
            buttons: PointerButtons::None,
            keyboard_modifiers: PointerKeyboardModifiers::default(),
            r#type,
            button: None,
            classification: PointerEventClassification::None,
        }
    }

    /// An event of `changes` of the platform's event `internal`.
    pub(crate) fn of(changes: Vec<PointerInputChange>, internal: &InternalPointerEvent) -> PointerEvent {
        let e = &internal.event;
        PointerEvent {
            changes,
            buttons: e.buttons,
            keyboard_modifiers: e.keyboard_modifiers,
            r#type: e.r#type,
            button: e.button,
            classification: e.classification,
        }
    }

    /// Whether the platform calls it a deep press (Compose foundation's `isDeepPress`).
    pub fn is_deep_press(&self) -> bool {
        self.classification == PointerEventClassification::DeepPress
    }
}

impl fmt::Debug for PointerEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PointerEvent")
            .field("type", &self.r#type)
            .field("changes", &self.changes)
            .field("buttons", &self.buttons)
            .field("keyboard_modifiers", &self.keyboard_modifiers)
            .field("button", &self.button)
            .finish()
    }
}

impl Default for PointerEvent {
    /// No changes (Compose's `EmptyPointerEvent`).
    fn default() -> Self {
        PointerEvent::new(Vec::new())
    }
}
