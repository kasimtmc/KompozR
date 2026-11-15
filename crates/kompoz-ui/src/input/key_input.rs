//! Key and rotary input nodes (Compose's KeyInputModifierNode, `Modifier.onKeyEvent`/`onPreviewKeyEvent`,
//! SoftKeyboardInterceptionModifierNode, `onInterceptKeyBeforeSoftKeyboard`, RotaryInputModifierNode and
//! `onRotaryScrollEvent`): what the focus owner sends along the focused path — previews from the outermost node
//! down, then the events from the focused node up.

use super::key::KeyEvent;
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement, NodeRc};
use crate::node_capabilities;
use std::any::Any;
use std::rc::Rc;

/// A node that takes key events on the focused path (Compose's KeyInputModifierNode): previews on the way down
/// (`on_pre_key_event`), then events on the way up (`on_key_event`); true takes the event.
pub trait KeyInputModifierNode {
    fn on_key_event(&mut self, event: &KeyEvent) -> bool;
    fn on_pre_key_event(&mut self, event: &KeyEvent) -> bool;
}

/// A node that sees a hardware key before a software keyboard does (Compose's
/// SoftKeyboardInterceptionModifierNode): previews down, then events up; true takes it.
pub trait SoftKeyboardInterceptionModifierNode {
    fn on_intercept_key_before_soft_keyboard(&mut self, event: &KeyEvent) -> bool;
    fn on_pre_intercept_key_before_soft_keyboard(&mut self, event: &KeyEvent) -> bool;
}

/// A rotary scroll (a watch's crown, a mouse wheel over a focused node: Compose's RotaryScrollEvent), px.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RotaryScrollEvent {
    pub vertical_scroll_pixels: f32,
    pub horizontal_scroll_pixels: f32,
    pub uptime_millis: i64,
    pub input_device_id: i32,
}

/// A node that takes rotary input on the focused path (Compose's RotaryInputModifierNode): previews down, then
/// events up; true takes it.
pub trait RotaryInputModifierNode {
    fn on_rotary_scroll_event(&mut self, event: &RotaryScrollEvent) -> bool;
    fn on_pre_rotary_scroll_event(&mut self, event: &RotaryScrollEvent) -> bool;
}

/// What a key handler of these modifiers is: true takes the event.
pub type KeyHandler = Rc<dyn Fn(&KeyEvent) -> bool>;

impl Modifier {
    /// `on_key_event` told the key events going up from the focused node (Compose's `Modifier.onKeyEvent`); true
    /// takes one.
    pub fn on_key_event(self, on_key_event: impl Fn(&KeyEvent) -> bool + 'static) -> Modifier {
        self.then_element(KeyInputElement { on_event: Some(Rc::new(on_key_event)), on_pre_event: None })
    }

    /// `on_preview_key_event` told the key events going down to the focused node, before it (Compose's
    /// `Modifier.onPreviewKeyEvent`).
    pub fn on_preview_key_event(self, on_preview_key_event: impl Fn(&KeyEvent) -> bool + 'static) -> Modifier {
        self.then_element(KeyInputElement { on_event: None, on_pre_event: Some(Rc::new(on_preview_key_event)) })
    }

    /// Told hardware keys before a software keyboard is, going up (Compose's
    /// `Modifier.onInterceptKeyBeforeSoftKeyboard`).
    pub fn on_intercept_key_before_soft_keyboard(self, handler: impl Fn(&KeyEvent) -> bool + 'static) -> Modifier {
        self.then_element(SoftKeyboardInterceptionElement { on_event: Some(Rc::new(handler)), on_pre_event: None })
    }

    /// The same going down (Compose's `Modifier.onPreInterceptKeyBeforeSoftKeyboard`).
    pub fn on_pre_intercept_key_before_soft_keyboard(self, handler: impl Fn(&KeyEvent) -> bool + 'static) -> Modifier {
        self.then_element(SoftKeyboardInterceptionElement { on_event: None, on_pre_event: Some(Rc::new(handler)) })
    }

    /// Told the rotary scrolls going up from the focused node (Compose's `Modifier.onRotaryScrollEvent`).
    pub fn on_rotary_scroll_event(
        self,
        on_rotary_scroll_event: impl Fn(&RotaryScrollEvent) -> bool + 'static,
    ) -> Modifier {
        self.then_element(RotaryInputElement { on_event: Some(Rc::new(on_rotary_scroll_event)), on_pre_event: None })
    }

    /// The same going down (Compose's `Modifier.onPreRotaryScrollEvent`).
    pub fn on_pre_rotary_scroll_event(
        self,
        on_pre_rotary_scroll_event: impl Fn(&RotaryScrollEvent) -> bool + 'static,
    ) -> Modifier {
        self.then_element(RotaryInputElement {
            on_event: None,
            on_pre_event: Some(Rc::new(on_pre_rotary_scroll_event)),
        })
    }
}

fn same<T: ?Sized>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

struct KeyInputElement {
    on_event: Option<KeyHandler>,
    on_pre_event: Option<KeyHandler>,
}

impl PartialEq for KeyInputElement {
    fn eq(&self, other: &Self) -> bool {
        same(&self.on_event, &other.on_event) && same(&self.on_pre_event, &other.on_pre_event)
    }
}

impl ModifierNodeElement for KeyInputElement {
    type Node = KeyInputNode;
    fn create(&self) -> KeyInputNode {
        KeyInputNode { on_event: self.on_event.clone(), on_pre_event: self.on_pre_event.clone() }
    }
    fn update(&self, node: &mut KeyInputNode) {
        node.on_event = self.on_event.clone();
        node.on_pre_event = self.on_pre_event.clone();
    }
    fn name(&self) -> &'static str {
        if self.on_event.is_some() { "onKeyEvent" } else { "onPreviewKeyEvent" }
    }
}

/// The node of `on_key_event` and `on_preview_key_event` (Compose's KeyInputNode).
pub struct KeyInputNode {
    on_event: Option<KeyHandler>,
    on_pre_event: Option<KeyHandler>,
}

impl ModifierNode for KeyInputNode {
    node_capabilities!(key_input);
}

impl KeyInputModifierNode for KeyInputNode {
    fn on_key_event(&mut self, event: &KeyEvent) -> bool {
        self.on_event.as_ref().is_some_and(|f| f(event))
    }
    fn on_pre_key_event(&mut self, event: &KeyEvent) -> bool {
        self.on_pre_event.as_ref().is_some_and(|f| f(event))
    }
}

/// The handlers of a key modifier's node (preview, event), taken out so they run with the node not borrowed; none
/// for another kind of node.
pub(crate) fn key_handlers(cell: &NodeRc) -> Option<(Option<KeyHandler>, Option<KeyHandler>)> {
    let node = cell.node.borrow();
    let node: &dyn Any = &*node;
    let k = node.downcast_ref::<KeyInputNode>()?;
    Some((k.on_pre_event.clone(), k.on_event.clone()))
}

struct SoftKeyboardInterceptionElement {
    on_event: Option<KeyHandler>,
    on_pre_event: Option<KeyHandler>,
}

impl PartialEq for SoftKeyboardInterceptionElement {
    fn eq(&self, other: &Self) -> bool {
        same(&self.on_event, &other.on_event) && same(&self.on_pre_event, &other.on_pre_event)
    }
}

impl ModifierNodeElement for SoftKeyboardInterceptionElement {
    type Node = InterceptedKeyInputNode;
    fn create(&self) -> InterceptedKeyInputNode {
        InterceptedKeyInputNode { on_event: self.on_event.clone(), on_pre_event: self.on_pre_event.clone() }
    }
    fn update(&self, node: &mut InterceptedKeyInputNode) {
        node.on_event = self.on_event.clone();
        node.on_pre_event = self.on_pre_event.clone();
    }
    fn name(&self) -> &'static str {
        if self.on_event.is_some() {
            "onKeyToSoftKeyboardInterceptedEvent"
        } else {
            "onPreKeyToSoftKeyboardInterceptedEvent"
        }
    }
}

/// The node of the soft keyboard interception modifiers (Compose's InterceptedKeyInputNode).
pub struct InterceptedKeyInputNode {
    on_event: Option<KeyHandler>,
    on_pre_event: Option<KeyHandler>,
}

impl ModifierNode for InterceptedKeyInputNode {
    node_capabilities!(soft_keyboard_interception);
}

impl SoftKeyboardInterceptionModifierNode for InterceptedKeyInputNode {
    fn on_intercept_key_before_soft_keyboard(&mut self, event: &KeyEvent) -> bool {
        self.on_event.as_ref().is_some_and(|f| f(event))
    }
    fn on_pre_intercept_key_before_soft_keyboard(&mut self, event: &KeyEvent) -> bool {
        self.on_pre_event.as_ref().is_some_and(|f| f(event))
    }
}

type RotaryHandler = Rc<dyn Fn(&RotaryScrollEvent) -> bool>;

struct RotaryInputElement {
    on_event: Option<RotaryHandler>,
    on_pre_event: Option<RotaryHandler>,
}

impl PartialEq for RotaryInputElement {
    fn eq(&self, other: &Self) -> bool {
        same(&self.on_event, &other.on_event) && same(&self.on_pre_event, &other.on_pre_event)
    }
}

impl ModifierNodeElement for RotaryInputElement {
    type Node = RotaryInputNode;
    fn create(&self) -> RotaryInputNode {
        RotaryInputNode { on_event: self.on_event.clone(), on_pre_event: self.on_pre_event.clone() }
    }
    fn update(&self, node: &mut RotaryInputNode) {
        node.on_event = self.on_event.clone();
        node.on_pre_event = self.on_pre_event.clone();
    }
    fn name(&self) -> &'static str {
        if self.on_event.is_some() { "onRotaryScrollEvent" } else { "onPreRotaryScrollEvent" }
    }
}

/// The node of the rotary modifiers (Compose's RotaryInputNode).
pub struct RotaryInputNode {
    on_event: Option<RotaryHandler>,
    on_pre_event: Option<RotaryHandler>,
}

impl ModifierNode for RotaryInputNode {
    node_capabilities!(rotary_input);
}

impl RotaryInputModifierNode for RotaryInputNode {
    fn on_rotary_scroll_event(&mut self, event: &RotaryScrollEvent) -> bool {
        self.on_event.as_ref().is_some_and(|f| f(event))
    }
    fn on_pre_rotary_scroll_event(&mut self, event: &RotaryScrollEvent) -> bool {
        self.on_pre_event.as_ref().is_some_and(|f| f(event))
    }
}
