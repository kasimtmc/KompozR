//! A selectable text's selection gestures (Compose's SelectionModifierElement.kt and SelectionModifierNode.kt):
//! the long press and drag of a finger and the mouse's clicks and drags, told to the registrar.

use crate::text::long_press_text_drag_observer::TextDragObserver;
use crate::text::selection::{
    MouseSelectionObserver, SelectionAdjustment, SelectionRegistrar, await_selection_gestures, has_selection,
};
use kompoz_graphics::Offset;
use kompoz_ui::input::{PointerInputScope, SuspendingPointerInputModifierNode};
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::{LayoutCoordinates, Modifier, ModifierNode, ModifierNodeElement, NodeCx};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Where a selectable is now: none before it is placed.
pub type LayoutCoordinatesProvider = Rc<dyn Fn() -> Option<LayoutCoordinates>>;

/// The selection gestures of the selectable `selectable_id` in `registrar`'s container (Compose's
/// SelectionModifierElement); `layout_coordinates_provider` gives where it is.
pub fn selection_modifier(
    registrar: Rc<dyn SelectionRegistrar>,
    selectable_id: u64,
    layout_coordinates_provider: LayoutCoordinatesProvider,
) -> Modifier {
    Modifier.then_element(SelectionModifierElement { registrar, selectable_id, layout_coordinates_provider })
}

struct SelectionModifierElement {
    registrar: Rc<dyn SelectionRegistrar>,
    selectable_id: u64,
    layout_coordinates_provider: LayoutCoordinatesProvider,
}

fn same_registrar(a: &Rc<dyn SelectionRegistrar>, b: &Rc<dyn SelectionRegistrar>) -> bool {
    std::ptr::eq(Rc::as_ptr(a) as *const (), Rc::as_ptr(b) as *const ())
}

impl PartialEq for SelectionModifierElement {
    fn eq(&self, other: &Self) -> bool {
        same_registrar(&self.registrar, &other.registrar)
            && self.selectable_id == other.selectable_id
            && Rc::ptr_eq(&self.layout_coordinates_provider, &other.layout_coordinates_provider)
    }
}

impl ModifierNodeElement for SelectionModifierElement {
    type Node = SelectionModifierNode;
    fn create(&self) -> SelectionModifierNode {
        SelectionModifierNode::new(self.registrar.clone(), self.selectable_id, self.layout_coordinates_provider.clone())
    }
    fn update(&self, node: &mut SelectionModifierNode) {
        node.update(self.registrar.clone(), self.selectable_id, self.layout_coordinates_provider.clone());
    }
    fn name(&self) -> &'static str {
        "selection"
    }
}

type Observers = (Rc<dyn MouseSelectionObserver>, Rc<dyn TextDragObserver>);

/// The node of a selectable's gestures (Compose's SelectionModifierNode): it delegates to a pointer input node
/// awaiting the selection gestures, its observers asking for the latest id and coordinates.
pub struct SelectionModifierNode {
    registrar: Rc<dyn SelectionRegistrar>,
    selectable_id: Rc<Cell<u64>>,
    layout_coordinates_provider: Rc<RefCell<LayoutCoordinatesProvider>>,
    observers: Rc<RefCell<Observers>>,
    pointer_input_node: Option<DelegatedNode<SuspendingPointerInputModifierNode>>,
}

impl SelectionModifierNode {
    fn new(
        registrar: Rc<dyn SelectionRegistrar>,
        selectable_id: u64,
        layout_coordinates_provider: LayoutCoordinatesProvider,
    ) -> SelectionModifierNode {
        let selectable_id = Rc::new(Cell::new(selectable_id));
        let layout_coordinates_provider = Rc::new(RefCell::new(layout_coordinates_provider));
        let observers = Self::observers(&registrar, &selectable_id, &layout_coordinates_provider);
        SelectionModifierNode {
            registrar,
            selectable_id,
            layout_coordinates_provider,
            observers: Rc::new(RefCell::new(observers)),
            pointer_input_node: None,
        }
    }

    /// Observers over `registrar`, asking for the node's latest id and coordinates.
    fn observers(
        registrar: &Rc<dyn SelectionRegistrar>,
        selectable_id: &Rc<Cell<u64>>,
        layout_coordinates_provider: &Rc<RefCell<LayoutCoordinatesProvider>>,
    ) -> Observers {
        let (id, coordinates) = (selectable_id.clone(), layout_coordinates_provider.clone());
        let id_provider: Rc<dyn Fn() -> u64> = Rc::new(move || id.get());
        let coordinates_provider: LayoutCoordinatesProvider = Rc::new(move || {
            let provider = coordinates.borrow().clone();
            provider()
        });
        (
            Rc::new(DefaultMouseSelectionObserver::new(
                registrar.clone(),
                Some(id_provider.clone()),
                coordinates_provider.clone(),
            )),
            Rc::new(DefaultLongPressDragObserver::new(registrar.clone(), id_provider, coordinates_provider)),
        )
    }

    fn update(
        &mut self,
        registrar: Rc<dyn SelectionRegistrar>,
        selectable_id: u64,
        layout_coordinates_provider: LayoutCoordinatesProvider,
    ) {
        let registrar_changed = !same_registrar(&registrar, &self.registrar);
        self.registrar = registrar;
        self.selectable_id.set(selectable_id);
        *self.layout_coordinates_provider.borrow_mut() = layout_coordinates_provider;
        // Another registrar (rare): observers anew.
        if registrar_changed {
            *self.observers.borrow_mut() =
                Self::observers(&self.registrar, &self.selectable_id, &self.layout_coordinates_provider);
        }
        if let Some(node) = &self.pointer_input_node {
            node.borrow().reset_pointer_input_handler();
        }
    }
}

impl ModifierNode for SelectionModifierNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        let observers = self.observers.clone();
        let node = SuspendingPointerInputModifierNode::new(move |scope: PointerInputScope| {
            let (mouse, long_press) = observers.borrow().clone();
            async move { await_selection_gestures(scope, mouse, Some(long_press)).await }
        });
        self.pointer_input_node = Some(cx.delegate(node));
    }

    fn on_detach(&mut self, cx: &NodeCx) {
        if let Some(node) = self.pointer_input_node.take() {
            cx.undelegate(&node);
        }
    }
}

/// A selectable's long press and drag, told to the registrar (Compose's `DefaultLongPressDragObserver`): the long
/// press starts a selection, drags move its end (scrolling of the selectable meanwhile added in).
pub struct DefaultLongPressDragObserver {
    registrar: Rc<dyn SelectionRegistrar>,
    selectable_id_provider: Rc<dyn Fn() -> u64>,
    layout_coordinates_provider: LayoutCoordinatesProvider,
    /// Where the moves not taken yet began.
    last_position: Cell<Offset>,
    /// Where the selectable was in the root then (b/343917640: a scroll meanwhile is added to the move).
    last_offset_in_root: Cell<Offset>,
    /// How far they went.
    drag_total_distance: Cell<Offset>,
    selection_adjustment_mode: Cell<SelectionAdjustment>,
}

impl DefaultLongPressDragObserver {
    pub fn new(
        registrar: Rc<dyn SelectionRegistrar>,
        selectable_id_provider: Rc<dyn Fn() -> u64>,
        layout_coordinates_provider: LayoutCoordinatesProvider,
    ) -> DefaultLongPressDragObserver {
        DefaultLongPressDragObserver {
            registrar,
            selectable_id_provider,
            layout_coordinates_provider,
            last_position: Cell::new(Offset::Unspecified),
            last_offset_in_root: Cell::new(Offset::Unspecified),
            drag_total_distance: Cell::new(Offset::Zero),
            selection_adjustment_mode: Cell::new(SelectionAdjustment::None),
        }
    }

    fn has_selection(&self) -> bool {
        has_selection(Some(&*self.registrar), (self.selectable_id_provider)())
    }

    fn end(&self) {
        if self.has_selection() {
            self.registrar.notify_selection_update_end();
        }
        self.last_position.set(Offset::Unspecified);
        self.last_offset_in_root.set(Offset::Unspecified);
    }
}

impl TextDragObserver for DefaultLongPressDragObserver {
    fn on_down(&self, _point: Offset) {
        // (a long press's drag doesn't use it)
    }

    fn on_up(&self) {}

    fn on_start(&self, start_point: Offset, selection_adjustment: SelectionAdjustment) {
        self.selection_adjustment_mode.set(selection_adjustment);
        if let Some(c) = (self.layout_coordinates_provider)() {
            if !c.is_attached() {
                return;
            }
            self.registrar.notify_selection_update_start(&c, start_point, selection_adjustment, true);
            self.last_position.set(start_point);
            self.last_offset_in_root.set(c.local_to_root(Offset::Zero));
        }
        // The selection never started.
        if !self.has_selection() {
            return;
        }
        self.drag_total_distance.set(Offset::Zero);
    }

    fn on_drag(&self, delta: Offset) {
        let Some(c) = (self.layout_coordinates_provider)() else { return };
        if !c.is_attached() || !self.has_selection() {
            return;
        }
        self.drag_total_distance.set(self.drag_total_distance.get() + delta);
        let scroll_offset = c.local_to_root(Offset::Zero) - self.last_offset_in_root.get();
        let new_position = self.last_position.get() + self.drag_total_distance.get() - scroll_offset;
        // Only the end moves: the start stays where a word or paragraph adjustment put it.
        let consumed = self.registrar.notify_selection_update(
            &c,
            new_position,
            self.last_position.get(),
            false,
            self.selection_adjustment_mode.get(),
            true,
        );
        if consumed {
            self.last_position.set(new_position);
            self.last_offset_in_root.set(c.local_to_root(Offset::Zero));
            self.drag_total_distance.set(Offset::Zero);
        }
    }

    fn on_stop(&self) {
        self.end();
    }

    fn on_cancel(&self) {
        self.end();
    }
}

/// A mouse's selection gestures, told to the registrar (Compose's `DefaultMouseSelectionObserver`): for a
/// selectable (`selectable_id_provider`), only while it has a part of the selection; without one (the space
/// between a container's texts), always.
pub struct DefaultMouseSelectionObserver {
    registrar: Rc<dyn SelectionRegistrar>,
    selectable_id_provider: Option<Rc<dyn Fn() -> u64>>,
    layout_coordinates_provider: LayoutCoordinatesProvider,
    last_position: Cell<Offset>,
}

impl DefaultMouseSelectionObserver {
    pub fn new(
        registrar: Rc<dyn SelectionRegistrar>,
        selectable_id_provider: Option<Rc<dyn Fn() -> u64>>,
        layout_coordinates_provider: LayoutCoordinatesProvider,
    ) -> DefaultMouseSelectionObserver {
        DefaultMouseSelectionObserver {
            registrar,
            selectable_id_provider,
            layout_coordinates_provider,
            last_position: Cell::new(Offset::Zero),
        }
    }

    fn should_process_selection_gesture(&self) -> bool {
        match &self.selectable_id_provider {
            None => true,
            Some(id) => has_selection(Some(&*self.registrar), id()),
        }
    }

    /// The end moved to `position`, adjusted by `adjustment`.
    fn update(&self, c: &LayoutCoordinates, position: Offset, adjustment: SelectionAdjustment) {
        let consumed =
            self.registrar.notify_selection_update(c, position, self.last_position.get(), false, adjustment, false);
        if consumed {
            self.last_position.set(position);
        }
    }
}

impl MouseSelectionObserver for DefaultMouseSelectionObserver {
    fn on_extend(&self, down_position: Offset) -> bool {
        let Some(c) = (self.layout_coordinates_provider)() else { return false };
        if !c.is_attached() {
            return false;
        }
        self.update(&c, down_position, SelectionAdjustment::None);
        self.should_process_selection_gesture()
    }

    fn on_extend_drag(&self, drag_position: Offset) -> bool {
        if let Some(c) = (self.layout_coordinates_provider)() {
            if !c.is_attached() || !self.should_process_selection_gesture() {
                return false;
            }
            self.update(&c, drag_position, SelectionAdjustment::None);
        }
        true
    }

    fn on_start(&self, down_position: Offset, adjustment: SelectionAdjustment, _click_count: u32) -> bool {
        let Some(c) = (self.layout_coordinates_provider)() else { return false };
        if !c.is_attached() {
            return false;
        }
        self.registrar.notify_selection_update_start(&c, down_position, adjustment, false);
        self.last_position.set(down_position);
        self.should_process_selection_gesture()
    }

    fn on_drag(&self, drag_position: Offset, adjustment: SelectionAdjustment) -> bool {
        if let Some(c) = (self.layout_coordinates_provider)() {
            if !c.is_attached() || !self.should_process_selection_gesture() {
                return false;
            }
            self.update(&c, drag_position, adjustment);
        }
        true
    }

    fn on_drag_done(&self) {
        self.registrar.notify_selection_update_end();
    }
}
