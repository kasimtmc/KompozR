//! `Modifier.on_click`, as Compose for desktop's OnClick.skiko.kt: clicks, double clicks and long clicks of the
//! pointer events a [`PointerMatcher`] takes (a mouse's secondary button, fingers only, …), with the keyboard
//! modifiers asked for — input only: unlike `clickable`, no indication, hover, focus, Enter key or accessibility
//! click (add them when wanted). A click moves the focus to the layout's focus target, if it has one.

#![allow(clippy::too_many_arguments)]

use crate::clickable::Callback;
use crate::gestures::{KeyboardModifiersTest, PointerInputScopeGestures, PressGestureScope};
use crate::interaction::{MutableInteractionSource, PressInteraction};
use crate::pointer_matcher::PointerMatcher;
use kompoz_graphics::Offset;
use kompoz_runtime::named;
use kompoz_ui::input::{PointerInputScope, PointerKeyboardModifiers, SuspendingPointerInputModifierNode};
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// `Modifier.on_click` of the desktop's foundation.
#[named(trait OnClickModifiers)]
impl Modifier {
    /// Calls `on_click` for a click of the pointer events `matcher` takes, the keyboard's modifiers as
    /// `keyboard_modifiers` wants them (Compose for desktop's `Modifier.onClick`); `on_double_click` for a double
    /// click (a click then waits for the double click timeout), `on_long_click` for a press held for the long
    /// press timeout. Presses are told to `interaction_source` (one of its own when none). Input only: no
    /// indication, hover, focus or accessibility click. Not `enabled`: nothing. (`keyboard_modifiers` none: any,
    /// Kotlin's default `{ true }`.)
    fn on_click(
        self,
        #[default(true)] enabled: bool,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        #[default(PointerMatcher::Primary)] matcher: PointerMatcher,
        #[default(None)] keyboard_modifiers: Option<KeyboardModifiersTest>,
        #[default(None)] on_double_click: Option<Callback>,
        #[default(None)] on_long_click: Option<Callback>,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        self.on_click_with(
            enabled,
            interaction_source,
            matcher,
            keyboard_modifiers,
            on_double_click,
            on_long_click,
            on_click,
        )
    }

    /// [`on_click`](OnClickModifiers::on_click) with every parameter, for plain code.
    fn on_click_with(
        self,
        enabled: bool,
        interaction_source: Option<MutableInteractionSource>,
        matcher: PointerMatcher,
        keyboard_modifiers: Option<KeyboardModifiersTest>,
        on_double_click: Option<Callback>,
        on_long_click: Option<Callback>,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        if !enabled {
            return self;
        }
        self.then_element(OnClickElement {
            interaction_source,
            callbacks: Rc::new(OnClickCallbacks {
                matcher,
                keyboard_modifiers: keyboard_modifiers.unwrap_or_else(|| Rc::new(|_: &PointerKeyboardModifiers| true)),
                on_double_click,
                on_long_click,
                on_click: Rc::new(on_click),
            }),
        })
    }
}

struct OnClickCallbacks {
    matcher: PointerMatcher,
    keyboard_modifiers: KeyboardModifiersTest,
    on_double_click: Option<Callback>,
    on_long_click: Option<Callback>,
    on_click: Callback,
}

/// (Compose compares the lambdas by identity: a new modifier's are new ones, which the node takes.)
struct OnClickElement {
    interaction_source: Option<MutableInteractionSource>,
    callbacks: Rc<OnClickCallbacks>,
}

impl PartialEq for OnClickElement {
    fn eq(&self, other: &Self) -> bool {
        self.interaction_source == other.interaction_source && Rc::ptr_eq(&self.callbacks, &other.callbacks)
    }
}

impl ModifierNodeElement for OnClickElement {
    type Node = OnClickModifierNode;
    fn create(&self) -> OnClickModifierNode {
        OnClickModifierNode {
            state: Rc::new(OnClickState {
                interaction_source: RefCell::new(self.interaction_source.clone().unwrap_or_default()),
                callbacks: RefCell::new(self.callbacks.clone()),
                press_interaction: Cell::new(None),
                handle: RefCell::new(None),
            }),
            input: None,
        }
    }
    fn update(&self, node: &mut OnClickModifierNode) {
        node.update(self.interaction_source.clone(), self.callbacks.clone());
    }
    fn name(&self) -> &'static str {
        "onClick"
    }
}

/// What the node and its pointer input share.
struct OnClickState {
    interaction_source: RefCell<MutableInteractionSource>,
    callbacks: RefCell<Rc<OnClickCallbacks>>,
    /// Compose's InteractionData: the press shown, to end it.
    press_interaction: Cell<Option<PressInteraction::Press>>,
    handle: RefCell<Option<NodeHandle>>,
}

impl OnClickState {
    fn callbacks(&self) -> Rc<OnClickCallbacks> {
        self.callbacks.borrow().clone()
    }

    /// The focus moved here on a click (Compose's `isRequestFocusOnClickEnabled()`: the desktop's is on).
    fn request_focus(&self) {
        let handle = self.handle.borrow().clone();
        if let Some(cx) = handle.and_then(|h| h.cx()) {
            cx.request_focus();
        }
    }

    fn cancel_press_interaction(&self) {
        if let Some(press) = self.press_interaction.take() {
            self.interaction_source.borrow().try_emit(PressInteraction::Cancel::new(press));
        }
    }

    /// Compose's `handlePressInteraction` (no delay): the press shown, then released or cancelled with it.
    async fn handle_press_interaction(self: Rc<Self>, press_scope: PressGestureScope, press_point: Offset) {
        let source = self.interaction_source.borrow().clone();
        let press = PressInteraction::Press::new(press_point);
        source.emit(press).await;
        self.press_interaction.set(Some(press));
        let success = press_scope.try_await_release().await;
        if let Some(press) = self.press_interaction.get() {
            let source = self.interaction_source.borrow().clone();
            if success {
                source.emit(PressInteraction::Release::new(press)).await;
            } else {
                source.emit(PressInteraction::Cancel::new(press)).await;
            }
        }
        self.press_interaction.set(None);
    }
}

/// `Modifier.on_click`'s node (Compose's OnClickModifierNode): taps of the matched events detected with the latest
/// callbacks; its detector restarted when a double or long click comes or goes, or the interaction source changes.
pub struct OnClickModifierNode {
    state: Rc<OnClickState>,
    input: Option<DelegatedNode<SuspendingPointerInputModifierNode>>,
}

impl OnClickModifierNode {
    fn update(&mut self, interaction_source: Option<MutableInteractionSource>, callbacks: Rc<OnClickCallbacks>) {
        let mut reset = false;
        if let Some(source) = interaction_source
            && *self.state.interaction_source.borrow() != source
        {
            reset = true;
            self.state.cancel_press_interaction();
            *self.state.interaction_source.borrow_mut() = source;
        }
        let old = self.state.callbacks();
        // (Compose compares the presence after taking the new callbacks, so it never restarts for them: the
        // detector reads whether there is a double or long click when it starts)
        if old.on_double_click.is_some() != callbacks.on_double_click.is_some() {
            reset = true;
        }
        if old.on_long_click.is_some() != callbacks.on_long_click.is_some() {
            reset = true;
            self.state.cancel_press_interaction();
        }
        *self.state.callbacks.borrow_mut() = callbacks;
        if reset && let Some(input) = &self.input {
            input.borrow().reset_pointer_input_handler();
        }
    }
}

impl ModifierNode for OnClickModifierNode {
    node_capabilities!();

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.state.handle.borrow_mut() = Some(cx.handle());
        if self.input.is_some() {
            return;
        }
        let state = self.state.clone();
        self.input = Some(cx.delegate(SuspendingPointerInputModifierNode::new(move |scope: PointerInputScope| {
            let state = state.clone();
            let callbacks = state.callbacks();
            let latest = {
                let s = state.clone();
                move || s.callbacks()
            };
            let (keys, tap, double, long, press) =
                (latest.clone(), latest.clone(), latest.clone(), latest.clone(), state.clone());
            let focus = (state.clone(), state.clone(), state.clone());
            let matcher = PointerMatcher::new(move |e| latest().matcher.matches(e));
            let mut detector = scope
                .detect_tap_gestures()
                .matcher(matcher)
                .keyboard_modifiers(move |m| (keys().keyboard_modifiers)(m))
                .on_tap(move |_| {
                    focus.0.request_focus();
                    (tap().on_click)();
                })
                .on_press(move |scope, at| press.clone().handle_press_interaction(scope, at));
            if callbacks.on_double_click.is_some() {
                detector = detector.on_double_tap(move |_| {
                    focus.1.request_focus();
                    if let Some(f) = &double().on_double_click {
                        f();
                    }
                });
            }
            if callbacks.on_long_click.is_some() {
                detector = detector.on_long_press(move |_| {
                    focus.2.request_focus();
                    if let Some(f) = &long().on_long_click {
                        f();
                    }
                });
            }
            detector.into_future()
        })));
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.state.cancel_press_interaction();
    }
}
