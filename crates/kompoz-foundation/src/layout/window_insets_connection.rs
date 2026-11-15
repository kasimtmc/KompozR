//! The keyboard following the finger past the end of what scrolls, as Compose's `Modifier.imeNestedScroll` (its
//! WindowInsetsNestedScrollConnection for the bottom side), driving the platform's keyboard through the host's
//! [`ImeController`] as C++'s did (the platform animates it to the end once let go).

use kompoz_graphics::Offset;
use kompoz_ui::input::{FlingFuture, NestedScrollConnection, NestedScrollNode, NestedScrollSource};
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::platform::{ImeController, LocalImeController};
use kompoz_ui::unit::Velocity;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx};
use std::cell::RefCell;
use std::rc::Rc;

/// The keyboard nested scroll modifier (Compose's `imeNestedScroll`).
pub trait ImeNestedScrollModifiers {
    /// The keyboard follows the finger past the end of what scrolls inside (Compose's `imeNestedScroll`, Android
    /// 11+), with a field that can be edited focused: pulled on up past the end, it comes up with the finger;
    /// pushed down while it is being moved, it goes down first (what scrolls inside takes the rest); let go, the
    /// platform finishes it shown or hidden by the fling. Elsewhere it does nothing. It finds the host's keyboard
    /// where it is attached.
    fn ime_nested_scroll(self) -> Modifier;
}

impl ImeNestedScrollModifiers for Modifier {
    fn ime_nested_scroll(self) -> Modifier {
        self.then_element(ImeNestedScrollElement)
    }
}

/// What moves the keyboard by what is scrolled (the bottom side's: a drag up shows it, down hides it).
struct ImeNestedScrollConnection {
    controller: RefCell<ImeController>,
}

impl ImeNestedScrollConnection {
    fn controller(&self) -> ImeController {
        self.controller.borrow().clone()
    }
}

impl NestedScrollConnection for ImeNestedScrollConnection {
    fn on_pre_scroll(&self, available: Offset, source: NestedScrollSource) -> Offset {
        // Pushed down while the keyboard is under the finger: it goes down first.
        let controller = self.controller();
        if source != NestedScrollSource::UserInput || !controller.is_controlling() || available.y <= 0.0 {
            return Offset::Zero;
        }
        Offset::new(0.0, -controller.control_by(-available.y))
    }

    fn on_post_scroll(&self, _consumed: Offset, available: Offset, source: NestedScrollSource) -> Offset {
        // Pulled on up past the end: it comes up with the finger.
        if source != NestedScrollSource::UserInput || available.y >= 0.0 {
            return Offset::Zero;
        }
        Offset::new(0.0, -self.controller().control_by(-available.y))
    }

    fn on_pre_fling(&self, available: Velocity) -> FlingFuture<'_> {
        let controller = self.controller();
        let taken = if controller.is_controlling() {
            controller.control_end(-available.y);
            available // (the fling was the keyboard's)
        } else {
            Velocity::Zero
        };
        Box::pin(async move { taken })
    }
}

#[derive(PartialEq)]
struct ImeNestedScrollElement;

impl ModifierNodeElement for ImeNestedScrollElement {
    type Node = ImeNestedScrollNode;
    fn create(&self) -> ImeNestedScrollNode {
        ImeNestedScrollNode {
            connection: Rc::new(ImeNestedScrollConnection { controller: RefCell::new(ImeController::default()) }),
            delegate: None,
        }
    }
    fn update(&self, _: &mut ImeNestedScrollNode) {}
    fn name(&self) -> &'static str {
        "imeNestedScroll"
    }
}

/// A nested scroll node (a delegate) with the keyboard's connection, given the host's keyboard as it is attached.
struct ImeNestedScrollNode {
    connection: Rc<ImeNestedScrollConnection>,
    delegate: Option<DelegatedNode<NestedScrollNode>>,
}

impl ModifierNode for ImeNestedScrollNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        *self.connection.controller.borrow_mut() = cx.current_value_of(&LocalImeController);
        if self.delegate.is_none() {
            let connection: Rc<dyn NestedScrollConnection> = self.connection.clone();
            self.delegate = Some(cx.delegate(kompoz_ui::input::nested_scroll_modifier_node(connection, None)));
        }
    }

    fn on_detach(&mut self, _: &NodeCx) {
        // (let go of it, should it still be held)
        let controller = std::mem::take(&mut *self.connection.controller.borrow_mut());
        controller.control_end(0.0);
    }
}
