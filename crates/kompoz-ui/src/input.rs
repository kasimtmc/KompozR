//! Input, as Compose's `androidx.compose.ui.input`: pointer events dispatched to pointer input nodes along the hit
//! path, key events along the focused path, nested scroll.
//!
//! What is here is the core's part: the capability traits nodes declare (so their kinds are known), the host's
//! input state, and the entry points the host calls. Their insides — Compose's pointer model (`PointerEvent`,
//! `PointerInputChange`, passes, hit testing through coordinators and layers, `pointer_input {}` coroutines), key
//! events and nested scroll — are the input package of this stage, which gives these traits their methods and
//! these functions their bodies.

use crate::modifier::NodeRc;
use crate::node::LayoutNode;
use kompoz_graphics::Offset;
use std::rc::Rc;

/// A node that takes pointer events (Compose's PointerInputModifierNode). The input package gives it
/// `on_pointer_event`; these are the parts the core knows.
pub trait PointerInputModifierNode {
    /// The gesture it was part of was cancelled (Compose's `onCancelPointerInput`).
    fn on_cancel_pointer_input(&mut self) {}
    /// Whether it sees its children's events even outside its own bounds (Compose's
    /// `interceptOutOfBoundsChildEvents`).
    fn intercept_out_of_bounds_child_events(&self) -> bool {
        false
    }
    /// Whether the siblings under it get the events it gets (Compose's `sharePointerInputWithSiblings`).
    fn share_pointer_input_with_siblings(&self) -> bool {
        false
    }
}

/// A node that takes indirect pointer events (a touchpad not over the window: Compose's
/// IndirectPointerInputModifierNode).
pub trait IndirectPointerInputModifierNode {
    /// Its stream of events was cancelled (Compose's `onCancelIndirectPointerInput`).
    fn on_cancel_indirect_pointer_input(&mut self) {}
}

/// A node that takes key events on the focused path (Compose's KeyInputModifierNode). The input package gives it
/// `on_key_event` and `on_pre_key_event`.
pub trait KeyInputModifierNode {}

/// A node that sees a software keyboard's keys before the focused field (Compose's
/// SoftKeyboardInterceptionModifierNode). The input package gives it its methods.
pub trait SoftKeyboardInterceptionModifierNode {}

/// A node that takes rotary input (a watch's crown: Compose's RotaryInputModifierNode). The input package gives it
/// its methods.
pub trait RotaryInputModifierNode {}

/// An indirect pointer node left its chain: its stream is cancelled (Compose's autoInvalidate of the kind).
pub(crate) fn on_indirect_pointer_node_removed(node: &NodeRc) {
    let mut n = node.node.borrow_mut();
    if let Some(i) = n.as_indirect_pointer_input() {
        i.on_cancel_indirect_pointer_input();
    }
}

/// What kind of pointer it is (Compose's PointerType).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PointerType {
    #[default]
    Unknown,
    Touch,
    Mouse,
    Stylus,
    Eraser,
}

/// A key (Compose's Key): its key code. The input package names the keys.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Key(pub u64);

/// The modifier keys held (Compose's keyboard modifiers), as bits.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct KeyModifiers(pub u32);

impl KeyModifiers {
    pub const None: KeyModifiers = KeyModifiers(0);
    pub const Shift: KeyModifiers = KeyModifiers(1);
    pub const Ctrl: KeyModifiers = KeyModifiers(1 << 1);
    pub const Alt: KeyModifiers = KeyModifiers(1 << 2);
    pub const Meta: KeyModifiers = KeyModifiers(1 << 3);
    pub fn contains(self, other: KeyModifiers) -> bool {
        self.0 & other.0 == other.0
    }
}

/// The host's input state (Compose's PointerInputEventProcessor and the rest): where pointers are and what is
/// pressed. The input package keeps the hit paths and gestures under way in it.
#[derive(Default)]
pub struct InputState {
    /// Where each pointer down is, by id.
    pub(crate) pointers: Vec<(i64, Offset, PointerType)>,
    /// The last position of a hovering mouse.
    pub(crate) hover: Option<Offset>,
    pub(crate) keyboard_modifiers: KeyModifiers,
}

impl InputState {
    /// The pointers down, by id.
    pub fn pointers_down(&self) -> Vec<(i64, Offset)> {
        self.pointers.iter().map(|(id, p, _)| (*id, *p)).collect()
    }
    pub fn keyboard_modifiers(&self) -> KeyModifiers {
        self.keyboard_modifiers
    }
}

// The entry points. The host calls them; the input package dispatches. Each returns whether a node took the event.

/// A pointer went down at `position` (root px).
pub(crate) fn pointer_down(
    state: &mut InputState,
    _root: &Rc<LayoutNode>,
    position: Offset,
    _time_nanos: i64,
    pointer_id: i64,
    pointer_type: PointerType,
) -> bool {
    state.pointers.retain(|(id, _, _)| *id != pointer_id);
    state.pointers.push((pointer_id, position, pointer_type));
    false
}

/// A pointer (down, or a hovering mouse) moved.
pub(crate) fn pointer_move(
    state: &mut InputState,
    _root: &Rc<LayoutNode>,
    position: Offset,
    _time_nanos: i64,
    pointer_id: i64,
) -> bool {
    match state.pointers.iter_mut().find(|(id, _, _)| *id == pointer_id) {
        Some(p) => p.1 = position,
        None => state.hover = Some(position),
    }
    false
}

/// A pointer went up.
pub(crate) fn pointer_up(
    state: &mut InputState,
    _root: &Rc<LayoutNode>,
    _position: Offset,
    _time_nanos: i64,
    pointer_id: i64,
) -> bool {
    state.pointers.retain(|(id, _, _)| *id != pointer_id);
    false
}

/// The gesture under way was cancelled (the platform took it).
pub(crate) fn pointer_cancel(state: &mut InputState, _root: &Rc<LayoutNode>) {
    state.pointers.clear();
}

/// The mouse left the window.
pub(crate) fn pointer_exit(state: &mut InputState, _root: &Rc<LayoutNode>) {
    state.hover = None;
}

/// A wheel or touchpad scrolled by `delta` (px) at `position`.
pub(crate) fn scroll(_state: &mut InputState, _root: &Rc<LayoutNode>, _position: Offset, _delta: Offset) -> bool {
    false
}

/// A key went down; the focus package and this one dispatch it along the focused path.
pub(crate) fn key_down(state: &mut InputState, _root: &Rc<LayoutNode>, _key: Key, modifiers: KeyModifiers) -> bool {
    state.keyboard_modifiers = modifiers;
    false
}

/// A key went up.
pub(crate) fn key_up(state: &mut InputState, _root: &Rc<LayoutNode>, _key: Key, modifiers: KeyModifiers) -> bool {
    state.keyboard_modifiers = modifiers;
    false
}
