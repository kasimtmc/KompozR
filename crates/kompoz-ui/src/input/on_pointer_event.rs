//! `Modifier.on_pointer_event`, as Compose desktop's: a callback for the layout's events of one type in one pass.

use super::{
    AwaitPointerEventScope, PointerEvent, PointerEventPass, PointerEventType, SuspendingPointerInputModifierNode,
};
use crate::modifier::{DelegatedNode, Modifier, ModifierNode, ModifierNodeElement, NodeCx};
use kompoz_runtime::named;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What `on_pointer_event` calls: the scope it awaits in, and the event.
type OnEvent = Rc<dyn Fn(&AwaitPointerEventScope, &PointerEvent)>;

/// The node of `Modifier.on_pointer_event`: a pointer input block (its delegate) awaiting each event in its pass
/// and calling back for those of its type — the latest type and callback, as Compose's `rememberUpdatedState`; a
/// new pass restarts it (Compose's key).
pub struct OnPointerEventNode {
    event_type: Rc<Cell<PointerEventType>>,
    on_event: Rc<RefCell<OnEvent>>,
    pass: PointerEventPass,
    delegate: Option<DelegatedNode<SuspendingPointerInputModifierNode>>,
}

impl OnPointerEventNode {
    fn block(
        &self,
    ) -> impl Fn(super::PointerInputScope) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> + 'static {
        let (event_type, on_event, pass) = (self.event_type.clone(), self.on_event.clone(), self.pass);
        move |scope| {
            let (event_type, on_event) = (event_type.clone(), on_event.clone());
            Box::pin(async move {
                scope
                    .await_pointer_event_scope(|s| async move {
                        loop {
                            let event = s.await_pointer_event_with(pass).await;
                            if event.r#type == event_type.get() {
                                let f = on_event.borrow().clone();
                                f(&s, &event);
                            }
                        }
                    })
                    .await
            })
        }
    }
}

impl ModifierNode for OnPointerEventNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        if self.delegate.is_none() {
            let block = self.block();
            self.delegate = Some(cx.delegate(SuspendingPointerInputModifierNode::new(block)));
        }
    }
}

struct OnPointerEventElement {
    event_type: PointerEventType,
    pass: PointerEventPass,
    on_event: OnEvent,
}

impl PartialEq for OnPointerEventElement {
    fn eq(&self, other: &Self) -> bool {
        self.event_type == other.event_type && self.pass == other.pass && Rc::ptr_eq(&self.on_event, &other.on_event)
    }
}

impl ModifierNodeElement for OnPointerEventElement {
    type Node = OnPointerEventNode;
    fn create(&self) -> OnPointerEventNode {
        OnPointerEventNode {
            event_type: Rc::new(Cell::new(self.event_type)),
            on_event: Rc::new(RefCell::new(self.on_event.clone())),
            pass: self.pass,
            delegate: None,
        }
    }
    fn update(&self, node: &mut OnPointerEventNode) {
        node.event_type.set(self.event_type);
        *node.on_event.borrow_mut() = self.on_event.clone();
        if node.pass != self.pass {
            node.pass = self.pass;
            let block = node.block();
            if let Some(d) = &node.delegate {
                d.borrow_mut().set_pointer_input_event_handler(block);
            }
        }
    }
    fn name(&self) -> &'static str {
        "onPointerEvent"
    }
}

#[named]
impl Modifier {
    /// Calls `on_event` with each of the layout's events of `event_type` in `pass` (Compose desktop's
    /// `Modifier.onPointerEvent`): Enter and Exit of a hovering mouse, Press, Release, Move, Scroll.
    pub fn on_pointer_event(
        self,
        event_type: PointerEventType,
        #[default(PointerEventPass::Main)] pass: PointerEventPass,
        on_event: impl Fn(&AwaitPointerEventScope, &PointerEvent) + 'static,
    ) -> Modifier {
        self.then_element(OnPointerEventElement { event_type, pass, on_event: Rc::new(on_event) })
    }
}
