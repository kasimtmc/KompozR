//! `Modifier.hoverable`, as Compose's Hoverable.kt: a mouse coming over the layout and leaving it, told to an
//! interaction source.

use crate::interaction::{HoverInteraction, MutableInteractionSource};
use kompoz_graphics::IntSize;
use kompoz_runtime::named;
use kompoz_ui::input::{PointerEvent, PointerEventPass, PointerEventType, PointerInputModifierNode};
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// The hover modifier of foundation.
#[named(trait HoverableModifiers)]
impl Modifier {
    /// Tells `interaction_source` when a pointer comes over the layout and leaves it ([`HoverInteraction`]s:
    /// Compose's `Modifier.hoverable`); a finger hovers nothing. Not `enabled`: nothing told.
    fn hoverable(self, interaction_source: MutableInteractionSource, #[default(true)] enabled: bool) -> Modifier {
        if enabled { self.then_element(HoverableElement { interaction_source }) } else { self }
    }

    /// [`hoverable`](HoverableModifiers::hoverable) with every parameter, for plain code.
    fn hoverable_with(self, interaction_source: MutableInteractionSource, enabled: bool) -> Modifier {
        if enabled { self.then_element(HoverableElement { interaction_source }) } else { self }
    }
}

#[derive(PartialEq)]
struct HoverableElement {
    interaction_source: MutableInteractionSource,
}

impl ModifierNodeElement for HoverableElement {
    type Node = HoverableNode;
    fn create(&self) -> HoverableNode {
        HoverableNode {
            state: Rc::new(HoverState {
                interaction_source: RefCell::new(self.interaction_source.clone()),
                hover_interaction: Cell::new(None),
            }),
            handle: None,
        }
    }
    fn update(&self, node: &mut HoverableNode) {
        node.update_interaction_source(self.interaction_source.clone());
    }
    fn name(&self) -> &'static str {
        "hoverable"
    }
}

/// What a hoverable and the coroutines it launches share.
struct HoverState {
    interaction_source: RefCell<MutableInteractionSource>,
    hover_interaction: Cell<Option<HoverInteraction::Enter>>,
}

impl HoverState {
    async fn emit_enter(&self) {
        if self.hover_interaction.get().is_none() {
            let interaction = HoverInteraction::Enter::new();
            let source = self.interaction_source.borrow().clone();
            source.emit(interaction).await;
            self.hover_interaction.set(Some(interaction));
        }
    }

    async fn emit_exit(&self) {
        if let Some(old) = self.hover_interaction.get() {
            let source = self.interaction_source.borrow().clone();
            source.emit(HoverInteraction::Exit::new(old)).await;
            self.hover_interaction.set(None);
        }
    }

    fn try_emit_exit(&self) {
        if let Some(old) = self.hover_interaction.take() {
            self.interaction_source.borrow().try_emit(HoverInteraction::Exit::new(old));
        }
    }
}

/// The node of `Modifier.hoverable` (Compose's HoverableNode).
pub struct HoverableNode {
    state: Rc<HoverState>,
    handle: Option<NodeHandle>,
}

impl HoverableNode {
    fn update_interaction_source(&mut self, interaction_source: MutableInteractionSource) {
        if *self.state.interaction_source.borrow() != interaction_source {
            self.state.try_emit_exit();
            *self.state.interaction_source.borrow_mut() = interaction_source;
        }
    }
}

impl ModifierNode for HoverableNode {
    node_capabilities!(pointer_input);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.state.try_emit_exit();
    }
}

impl PointerInputModifierNode for HoverableNode {
    fn on_pointer_event(&mut self, event: &PointerEvent, pass: PointerEventPass, _bounds: IntSize) {
        if pass != PointerEventPass::Main {
            return;
        }
        let Some(scope) = self.handle.as_ref().and_then(NodeHandle::coroutine_scope) else { return };
        let state = self.state.clone();
        match event.r#type {
            PointerEventType::Enter => {
                scope.launch(async move { state.emit_enter().await });
            }
            PointerEventType::Exit => {
                scope.launch(async move { state.emit_exit().await });
            }
            _ => {}
        }
    }

    fn on_cancel_pointer_input(&mut self) {
        self.state.try_emit_exit();
    }
}
