//! Input, as Compose's `androidx.compose.ui.input`: pointer events dispatched to pointer input nodes along the hit
//! path, key events along the focused path, nested scroll.
//!
//! Pointers: what the host is told (a finger down, moved, up; the mouse moved, a button pressed, the wheel turned)
//! becomes Compose's [`PointerEvent`]s of [`PointerInputChange`]s, whose consumption is shared by every handler
//! that sees them. The pointer input nodes under a pointer (hit tested through the coordinators, their layers'
//! matrices and clips; a finger's minimum touch target) get each event in three passes ([`PointerEventPass`]).
//! [`Modifier::pointer_input`] runs a coroutine per node that awaits them ([`PointerInputScope`],
//! [`AwaitPointerEventScope`]); an event resumes it in place, so the next handler sees what it consumed. Hover
//! (a mouse's Enter, Move, Exit), the cursor ([`Modifier::pointer_hover_icon`]), [`Modifier::on_pointer_event`],
//! [`VelocityTracker`] and nested scrolling ([`Modifier::nested_scroll`]) are here too; the gesture detectors are
//! foundation's.
//!
//! Keys: [`Key`]s and [`KeyEvent`]s, and the nodes that take them along the focused path ([`Modifier::on_key_event`],
//! [`Modifier::on_preview_key_event`], the soft keyboard interception and rotary nodes); the focus owner sends
//! them (previews down, events up), and what none takes does the host's default (Tab and arrows move the focus,
//! Escape and Back go back).

mod hit_path;
mod hit_test;
mod hover_icon;
mod key;
mod key_input;
mod nested_scroll;
mod on_pointer_event;
mod pointer;
mod processor;
mod suspending;
mod velocity;

pub use hit_test::{DpTouchBoundsExpansion, TouchBoundsExpansion};
pub use hover_icon::{HoverIconModifierNode, PointerIconService};
pub use key::{Key, KeyEvent, KeyEventType, KeyModifiers};
pub(crate) use key_input::key_handlers;
pub use key_input::{
    InterceptedKeyInputNode, KeyHandler, KeyInputModifierNode, KeyInputNode, RotaryInputModifierNode, RotaryInputNode,
    RotaryScrollEvent, SoftKeyboardInterceptionModifierNode,
};
pub use nested_scroll::{
    FlingFuture, NestedScrollConnection, NestedScrollDispatcher, NestedScrollNode, NestedScrollSource,
    nested_scroll_modifier_node,
};
pub use on_pointer_event::OnPointerEventNode;
pub use pointer::{
    HistoricalChange, PointerButton, PointerButtons, PointerEvent, PointerEventClassification, PointerEventPass,
    PointerEventType, PointerId, PointerInputChange, PointerKeyboardModifiers, PointerType,
};
pub use processor::ProcessResult;
pub use suspending::{
    AwaitPointerEventScope, PointerEventTimeoutCancellation, PointerInputScope, SuspendingPointerInputModifierNode,
};
pub use velocity::{VelocityTracker, VelocityTracker1D};

use crate::host::KompozHost;
use crate::modifier::NodeRc;
use crate::node::LayoutNode;
use kompoz_graphics::{IntSize, Offset};
use pointer::{PointerInputEvent, PointerInputEventData};
use processor::PointerInputEventProcessor;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// A node that takes pointer events (Compose's PointerInputModifierNode): each event of the pointers that hit it
/// (positions local to it) in each pass, and a cancel when the gesture is taken away.
pub trait PointerInputModifierNode {
    /// An event in a pass; `bounds` is its size (Compose's `onPointerEvent`).
    fn on_pointer_event(&mut self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize);
    /// The gesture it was part of was cancelled (Compose's `onCancelPointerInput`).
    fn on_cancel_pointer_input(&mut self);
    /// Whether it sees its children's events even outside its own bounds (Compose's
    /// `interceptOutOfBoundsChildEvents`).
    fn intercept_out_of_bounds_child_events(&self) -> bool {
        false
    }
    /// Whether the siblings under it get the events it gets (Compose's `sharePointerInputWithSiblings`).
    fn share_pointer_input_with_siblings(&self) -> bool {
        false
    }
    /// Its layout's density changed (Compose's `onDensityChange`): by default, what is under way is cancelled.
    fn on_density_change(&mut self) {
        self.on_cancel_pointer_input();
    }
    /// Its layout's view configuration changed (Compose's `onViewConfigurationChange`).
    fn on_view_configuration_change(&mut self) {
        self.on_cancel_pointer_input();
    }
    /// How far past its bounds a stylus hits it (Compose's `touchBoundsExpansion`).
    fn touch_bounds_expansion(&self) -> TouchBoundsExpansion {
        TouchBoundsExpansion::None
    }
}

/// A node that takes indirect pointer events (a touchpad not over the window: Compose's
/// IndirectPointerInputModifierNode).
pub trait IndirectPointerInputModifierNode {
    /// Its stream of events was cancelled (Compose's `onCancelIndirectPointerInput`).
    fn on_cancel_indirect_pointer_input(&mut self) {}
}

/// An indirect pointer node left its chain: its stream is cancelled (Compose's autoInvalidate of the kind).
pub(crate) fn on_indirect_pointer_node_removed(node: &NodeRc) {
    let mut n = node.node.borrow_mut();
    if let Some(i) = n.as_indirect_pointer_input() {
        i.on_cancel_indirect_pointer_input();
    }
}

/// Tells the pointer input nodes of a layout that its density changed (Compose's `LayoutNode.density` setter).
pub(crate) fn on_density_changed(node: &LayoutNode) {
    for_pointer_input_nodes(node, &mut |p| p.on_density_change());
}

/// Tells them its view configuration changed.
pub(crate) fn on_view_configuration_changed(node: &LayoutNode) {
    for_pointer_input_nodes(node, &mut |p| p.on_view_configuration_change());
}

fn for_pointer_input_nodes(node: &LayoutNode, f: &mut dyn FnMut(&mut dyn PointerInputModifierNode)) {
    let mut cells = Vec::new();
    node.head_to_tail(crate::modifier::NodeKind::PointerInput, &mut |c| cells.push(c.clone()));
    for cell in cells {
        call_node(|| {
            let mut n = cell.node.borrow_mut();
            if let Some(p) = n.as_pointer_input() {
                f(p);
            }
        });
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Calling nodes: what a node sets off runs once it isn't borrowed

thread_local! {
    /// What nodes being called put off until they are no longer borrowed, a list per call (innermost last).
    static DEFERRED: RefCell<Vec<Vec<Box<dyn FnOnce()>>>> = const { RefCell::new(Vec::new()) };
}

/// Runs `call`, which borrows a node to call it, then what the node put off with [`when_node_released`] (a
/// pointer input coroutine resumed by the event it was handed): user code never runs while a node is borrowed.
pub(crate) fn call_node(call: impl FnOnce()) {
    DEFERRED.with(|d| d.borrow_mut().push(Vec::new()));
    /// Drops the list if the call panics.
    struct Frame(bool);
    impl Drop for Frame {
        fn drop(&mut self) {
            if !self.0 {
                DEFERRED.with(|d| d.borrow_mut().pop());
            }
        }
    }
    let mut frame = Frame(false);
    call();
    frame.0 = true;
    let deferred = DEFERRED.with(|d| d.borrow_mut().pop()).unwrap_or_default();
    for f in deferred {
        f();
    }
}

/// Runs `f` once the node being called is no longer borrowed (now, if no node is being called).
pub(crate) fn when_node_released(f: impl FnOnce() + 'static) {
    let f: Box<dyn FnOnce()> = Box::new(f);
    let rest = DEFERRED.with(|d| match d.borrow_mut().last_mut() {
        Some(list) => {
            list.push(f);
            None
        }
        None => Some(f),
    });
    if let Some(f) = rest {
        f();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The host's input state

/// A finger (or stylus) down, as the host knows it.
#[derive(Clone, Copy, Debug)]
struct TrackedPointer {
    host_id: i64,
    id: PointerId,
    position: Offset,
    r#type: PointerType,
}

/// The mouse over the window: one hover stream, pressed or not (Compose desktop's).
#[derive(Clone, Copy, Debug)]
struct MouseState {
    host_id: i64,
    id: PointerId,
    position: Offset,
    buttons: PointerButtons,
}

/// The host's input state: the pointers down and the mouse as the platform reported them, and Compose's
/// PointerInputEventProcessor with its hit paths.
pub struct InputState {
    pub(crate) processor: Rc<PointerInputEventProcessor>,
    touches: RefCell<Vec<TrackedPointer>>,
    mouse: RefCell<Option<MouseState>>,
    next_id: Cell<i64>,
    keyboard_modifiers: Cell<KeyModifiers>,
    /// The host ids of the pointers whose press only dismissed a popup: what they do until they lift goes
    /// nowhere.
    swallowed: RefCell<Vec<i64>>,
}

impl Default for InputState {
    fn default() -> Self {
        InputState {
            processor: Rc::new(PointerInputEventProcessor::default()),
            touches: RefCell::new(Vec::new()),
            mouse: RefCell::new(None),
            next_id: Cell::new(0),
            keyboard_modifiers: Cell::new(KeyModifiers::None),
            swallowed: RefCell::new(Vec::new()),
        }
    }
}

impl InputState {
    /// The pointers down (a pressed mouse too), by the host's ids.
    pub fn pointers_down(&self) -> Vec<(i64, Offset)> {
        let mut out: Vec<(i64, Offset)> = self.touches.borrow().iter().map(|p| (p.host_id, p.position)).collect();
        if let Some(m) = *self.mouse.borrow()
            && m.buttons.are_any_pressed()
        {
            out.push((m.host_id, m.position));
        }
        out
    }

    /// Where the mouse is, while it is over the window.
    pub fn mouse_position(&self) -> Option<Offset> {
        self.mouse.borrow().map(|m| m.position)
    }

    /// The mouse buttons pressed.
    pub fn mouse_buttons(&self) -> PointerButtons {
        self.mouse.borrow().map_or(PointerButtons::None, |m| m.buttons)
    }

    pub fn keyboard_modifiers(&self) -> KeyModifiers {
        self.keyboard_modifiers.get()
    }

    /// Whether a pointer input node of `node`'s chain is on a hit path (tests ask).
    pub fn is_on_hit_path(&self, node: &LayoutNode) -> bool {
        node.chain_nodes().iter().any(|c| self.processor.tracker.contains(c))
    }

    fn fresh_id(&self) -> PointerId {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        PointerId(id)
    }

    fn event(&self, r#type: PointerEventType, pointers: Vec<PointerInputEventData>) -> PointerInputEvent {
        PointerInputEvent {
            pointers,
            r#type,
            buttons: PointerButtons::None,
            button: None,
            keyboard_modifiers: self.keyboard_modifiers.get().into(),
            classification: PointerEventClassification::None,
        }
    }

    /// The fingers down as an event's pointers, `lifted` (if any) let go at its position.
    fn touch_pointers(&self, time_nanos: i64, lifted: Option<i64>) -> Vec<PointerInputEventData> {
        self.touches
            .borrow()
            .iter()
            .map(|p| data(p.id, time_nanos, p.position, Some(p.host_id) != lifted, p.r#type, false))
            .collect()
    }

    fn mouse_pointer(&self, time_nanos: i64) -> Option<PointerInputEventData> {
        self.mouse
            .borrow()
            .map(|m| data(m.id, time_nanos, m.position, m.buttons.are_any_pressed(), PointerType::Mouse, true))
    }

    /// The mouse at `position` (now over the window if it wasn't).
    fn mouse_at(&self, host_id: Option<i64>, position: Offset) {
        let mut mouse = self.mouse.borrow_mut();
        match mouse.as_mut() {
            Some(m) => {
                m.position = position;
                if let Some(h) = host_id {
                    m.host_id = h;
                }
            }
            None => {
                *mouse = Some(MouseState {
                    host_id: host_id.unwrap_or(0),
                    id: self.fresh_id(),
                    position,
                    buttons: PointerButtons::None,
                })
            }
        }
    }
}

fn data(
    id: PointerId,
    time_nanos: i64,
    position: Offset,
    down: bool,
    r#type: PointerType,
    active_hover: bool,
) -> PointerInputEventData {
    PointerInputEventData {
        id,
        uptime: time_nanos / 1_000_000,
        position,
        down,
        pressure: if down { 1.0 } else { 0.0 },
        r#type,
        active_hover,
        historical: Vec::new(),
        scroll_delta: Offset::Zero,
    }
}

/// The roots to hit test, topmost first: the overlays (popups, dialogs) placed, then the main tree; below a
/// focusable popup or a dialog nothing (it is modal: what it doesn't take goes nowhere).
fn roots(host: &KompozHost) -> Vec<Rc<LayoutNode>> {
    let mut roots = Vec::new();
    for root in host.overlays.roots().into_iter().filter(|n| n.is_placed()).rev() {
        let modal = root.is_modal_popup_root();
        roots.push(root);
        if modal {
            return roots;
        }
    }
    roots.push(host.root().clone());
    roots
}

/// Processes an event, nothing of the host borrowed while the nodes run.
fn process(host: &KompozHost, event: PointerInputEvent, in_bounds: bool) -> ProcessResult {
    let processor = host.input.borrow().processor.clone();
    processor.process(event, &roots(host), in_bounds)
}

// The entry points the host calls. Each returns whether the event went to a pointer input node (`scroll`: whether
// one took it).

/// A finger (or stylus; a mouse's primary button) went down at `position` (the host's px).
pub(crate) fn pointer_down(
    host: &KompozHost,
    position: Offset,
    time_nanos: i64,
    pointer_id: i64,
    pointer_type: PointerType,
) -> bool {
    if pointer_type == PointerType::Mouse {
        return mouse_button(host, position, time_nanos, PointerButton::Primary, true, Some(pointer_id));
    }
    {
        // (a finger joining a press that only dismissed a popup goes nowhere too)
        let state = host.input.borrow();
        let mut swallowed = state.swallowed.borrow_mut();
        if swallowed.iter().any(|p| *p != i64::MIN) {
            if !swallowed.contains(&pointer_id) {
                swallowed.push(pointer_id);
            }
            return false;
        }
    }
    let first = host.input.borrow().touches.borrow().is_empty();
    if first && crate::window::press_outside_popup(host, position) {
        host.input.borrow().swallowed.borrow_mut().push(pointer_id);
        return true;
    }
    // A finger hovers nothing: what the mouse hovered ends.
    let hovering = host.input.borrow().mouse.borrow().filter(|m| !m.buttons.are_any_pressed());
    if hovering.is_some() {
        pointer_exit(host);
    }
    let event = {
        let state = host.input.borrow();
        if state.touches.borrow().iter().any(|p| p.host_id == pointer_id) {
            return false;
        }
        let id = state.fresh_id();
        state.touches.borrow_mut().push(TrackedPointer { host_id: pointer_id, id, position, r#type: pointer_type });
        state.event(PointerEventType::Press, state.touch_pointers(time_nanos, None))
    };
    process(host, event, true).dispatched_to_a_pointer_input_modifier
}

/// A pointer moved: a finger down, else the mouse (hovering, or pressed).
pub(crate) fn pointer_move(host: &KompozHost, position: Offset, time_nanos: i64, pointer_id: i64) -> bool {
    if host.input.borrow().swallowed.borrow().contains(&pointer_id) {
        return false;
    }
    let event = {
        let state = host.input.borrow();
        let is_touch = {
            let mut touches = state.touches.borrow_mut();
            match touches.iter_mut().find(|p| p.host_id == pointer_id) {
                Some(p) => {
                    p.position = position;
                    true
                }
                None => false,
            }
        };
        if is_touch {
            state.event(PointerEventType::Move, state.touch_pointers(time_nanos, None))
        } else {
            state.mouse_at(Some(pointer_id), position);
            let mut e = state.event(PointerEventType::Move, state.mouse_pointer(time_nanos).into_iter().collect());
            e.buttons = state.mouse_buttons();
            e
        }
    };
    process(host, event, true).dispatched_to_a_pointer_input_modifier
}

/// Several fingers moved at once: one event (a platform batching its pointers, as Android's ACTION_MOVE).
pub(crate) fn pointer_moves(host: &KompozHost, moves: &[(i64, Offset)], time_nanos: i64) -> bool {
    let event = {
        let state = host.input.borrow();
        let mut any = false;
        for (pointer_id, position) in moves {
            if let Some(p) = state.touches.borrow_mut().iter_mut().find(|p| p.host_id == *pointer_id) {
                p.position = *position;
                any = true;
            }
        }
        if !any {
            return false;
        }
        state.event(PointerEventType::Move, state.touch_pointers(time_nanos, None))
    };
    process(host, event, true).dispatched_to_a_pointer_input_modifier
}

/// A pointer went up: a finger, else the mouse's primary button (or the first one pressed).
pub(crate) fn pointer_up(host: &KompozHost, position: Offset, time_nanos: i64, pointer_id: i64) -> bool {
    {
        let state = host.input.borrow();
        let mut swallowed = state.swallowed.borrow_mut();
        if let Some(i) = swallowed.iter().position(|p| *p == pointer_id) {
            swallowed.remove(i);
            return false;
        }
    }
    let event = {
        let state = host.input.borrow();
        let is_touch = {
            let mut touches = state.touches.borrow_mut();
            match touches.iter_mut().find(|p| p.host_id == pointer_id) {
                Some(p) => {
                    p.position = position;
                    true
                }
                None => false,
            }
        };
        if !is_touch {
            let buttons = state.mouse_buttons();
            drop(state);
            if !buttons.are_any_pressed() {
                return false;
            }
            let button = if buttons.is_primary_pressed() {
                PointerButton::Primary
            } else {
                PointerButton(buttons.index_of_first_pressed() as u32)
            };
            return mouse_button(host, position, time_nanos, button, false, Some(pointer_id));
        }
        let e = state.event(PointerEventType::Release, state.touch_pointers(time_nanos, Some(pointer_id)));
        state.touches.borrow_mut().retain(|p| p.host_id != pointer_id);
        e
    };
    process(host, event, true).dispatched_to_a_pointer_input_modifier
}

/// A mouse button pressed or let go at `position` (the mouse there: Compose desktop's Press and Release, the
/// pointer pressed while any button is).
pub(crate) fn mouse_button(
    host: &KompozHost,
    position: Offset,
    time_nanos: i64,
    button: PointerButton,
    pressed: bool,
    pointer_id: Option<i64>,
) -> bool {
    const MOUSE: i64 = i64::MIN;
    {
        let state = host.input.borrow();
        let mut swallowed = state.swallowed.borrow_mut();
        if swallowed.contains(&MOUSE) {
            state.mouse_at(pointer_id, position);
            let mut mouse = state.mouse.borrow_mut();
            let m = mouse.as_mut().expect("the mouse is over the window");
            m.buttons = m.buttons.with(button, pressed);
            if !m.buttons.are_any_pressed() {
                swallowed.retain(|p| *p != MOUSE);
            }
            return false;
        }
    }
    let first_press = pressed && !host.input.borrow().mouse_buttons().are_any_pressed();
    if first_press && crate::window::press_outside_popup(host, position) {
        let state = host.input.borrow();
        state.mouse_at(pointer_id, position);
        let mut mouse = state.mouse.borrow_mut();
        let m = mouse.as_mut().expect("the mouse is over the window");
        m.buttons = m.buttons.with(button, true);
        state.swallowed.borrow_mut().push(MOUSE);
        return true;
    }
    let event = {
        let state = host.input.borrow();
        state.mouse_at(pointer_id, position);
        let buttons = {
            let mut mouse = state.mouse.borrow_mut();
            let m = mouse.as_mut().expect("the mouse is over the window");
            m.buttons = m.buttons.with(button, pressed);
            m.buttons
        };
        let r#type = if pressed { PointerEventType::Press } else { PointerEventType::Release };
        let mut e = state.event(r#type, state.mouse_pointer(time_nanos).into_iter().collect());
        e.buttons = buttons;
        e.button = Some(button);
        e
    };
    let dispatched = process(host, event, true).dispatched_to_a_pointer_input_modifier;
    if first_press {
        clear_focus_pressed_outside(host, position);
    }
    dispatched
}

/// A mouse press outside the focused node clears the focus, after the press is handled (so it can move the focus
/// elsewhere first: Compose's cursor-based AutoClearFocusBehavior).
fn clear_focus_pressed_outside(host: &KompozHost, position: Offset) {
    let Some(focused) = host.focus.focused_layout_node() else { return };
    if !focused.is_attached() || !focused.is_placed() {
        return;
    }
    if !focused.coordinates().bounds_in_root().contains(position) {
        host.focus.clear_focus(false);
    }
}

/// The gesture under way was cancelled (the platform took it): every node on a hit path is told.
pub(crate) fn pointer_cancel(host: &KompozHost) {
    let processor = {
        let state = host.input.borrow();
        state.touches.borrow_mut().clear();
        state.swallowed.borrow_mut().clear();
        if let Some(m) = state.mouse.borrow_mut().as_mut() {
            m.buttons = PointerButtons::None;
        }
        state.processor.clone()
    };
    processor.process_cancel();
}

/// The mouse left the window: what it hovered is exited.
pub(crate) fn pointer_exit(host: &KompozHost) {
    let event = {
        let state = host.input.borrow();
        let time = host.dispatcher().now();
        let Some(pointer) = state.mouse_pointer(time) else { return };
        let mut e = state.event(PointerEventType::Exit, vec![pointer]);
        e.buttons = state.mouse_buttons();
        e
    };
    process(host, event, false);
    let state = host.input.borrow();
    let mut mouse = state.mouse.borrow_mut();
    if mouse.is_some_and(|m| !m.buttons.are_any_pressed()) {
        *mouse = None;
    }
}

/// A wheel or touchpad scrolled by `delta` px (down and right positive: the change's `scroll_delta`) with the
/// mouse at `position`.
pub(crate) fn scroll(host: &KompozHost, position: Offset, delta: Offset) -> bool {
    let event = {
        let state = host.input.borrow();
        state.mouse_at(None, position);
        let time = host.dispatcher().now();
        let mut pointer = state.mouse_pointer(time).expect("the mouse is over the window");
        pointer.scroll_delta = delta;
        let mut e = state.event(PointerEventType::Scroll, vec![pointer]);
        e.buttons = state.mouse_buttons();
        e
    };
    process(host, event, true).any_change_consumed
}

/// A key event (Compose's AndroidComposeView `dispatchKeyEventPreIme` then `dispatchKeyEvent`): the soft
/// keyboard interception nodes first, then the key nodes along the focused path (previews down, events up); one
/// none took that is Escape or Back going down goes back (the platform's back). The modifiers held are what pointer
/// events and the window info carry; a key down puts the UI in keyboard mode. Whether it was taken.
pub(crate) fn key_event(host: &KompozHost, event: KeyEvent) -> bool {
    host.input.borrow().keyboard_modifiers.set(event.modifiers);
    host.window_info_keyboard_modifiers(event.modifiers);
    if event.r#type == KeyEventType::KeyDown {
        host.request_keyboard_mode();
    }
    let focus = host.focus.clone();
    if focus.dispatch_intercepted_soft_keyboard_event(&event) {
        return true;
    }
    if focus.dispatch_key_event(&event, &mut || false) {
        return true;
    }
    if event.r#type == KeyEventType::KeyDown && (event.key == Key::Escape || event.key == Key::Back) {
        return host.back_pressed();
    }
    false
}

/// A rotary scroll along the focused path; whether a node took it.
pub(crate) fn rotary_event(host: &KompozHost, event: RotaryScrollEvent) -> bool {
    let focus = host.focus.clone();
    focus.dispatch_rotary_event(&event, &mut || false)
}
