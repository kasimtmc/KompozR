//! The text node of a text in a selection container (Compose's SelectableTextAnnotatedStringElement and
//! SelectableTextAnnotatedStringNode): an annotated string's node that also tells its [`SelectionController`]
//! when it is placed and unplaced, where it is and which pinnable container it is in.

use super::selection_hook::SelectionController;
use super::text_annotated_string::{TextAnnotatedStringElement, TextAnnotatedStringNode, same_rc};
use kompoz_graphics::Constraints;
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode};
use kompoz_ui::layout::{
    GlobalPositionAwareModifierNode, IntrinsicMeasurable, IntrinsicMeasureScope, LayoutAwareModifierNode,
    LayoutCoordinates, LayoutModifierNode, LocalPinnableContainer, Measurable, MeasureResult, MeasureScope,
    PinnableContainerRef, UnplacedAwareModifierNode,
};
use kompoz_ui::modifier::ObserverModifierNode;
use kompoz_ui::semantics::{SemanticsConfiguration, SemanticsModifierNode};
use kompoz_ui::{ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::rc::Rc;

/// The element of a selectable text (Compose's SelectableTextAnnotatedStringElement): an annotated string's
/// element whose selection controller is given.
#[derive(PartialEq)]
pub(crate) struct SelectableTextAnnotatedStringElement {
    pub(crate) text: TextAnnotatedStringElement,
}

impl ModifierNodeElement for SelectableTextAnnotatedStringElement {
    type Node = SelectableTextAnnotatedStringNode;

    fn create(&self) -> SelectableTextAnnotatedStringNode {
        assert!(
            self.text.selection_controller.is_some(),
            "kompoz: Do not use SelectionCapableStaticTextModifier unless selectionController != null"
        );
        SelectableTextAnnotatedStringNode {
            text_node: self.text.create_node(|cx, f| {
                cx.with(|n: &mut SelectableTextAnnotatedStringNode| f(&mut n.text_node));
            }),
            selection_controller: self.text.selection_controller.clone(),
            is_placed: false,
            handle: None,
        }
    }

    fn update(&self, node: &mut SelectableTextAnnotatedStringNode) {
        node.update(&self.text);
    }

    fn name(&self) -> &'static str {
        "selectableTextAnnotatedString"
    }
}

/// The node of a selectable text (Compose's SelectableTextAnnotatedStringNode): an annotated string's text node
/// (measuring, drawing and describing the text), telling the selection controller when the text is placed or
/// unplaced, where it is, and the pinnable container it is in (observed: told again when it changes).
pub(crate) struct SelectableTextAnnotatedStringNode {
    pub(crate) text_node: TextAnnotatedStringNode,
    selection_controller: Option<Rc<dyn SelectionController>>,
    is_placed: bool,
    handle: Option<NodeHandle>,
}

impl SelectableTextAnnotatedStringNode {
    /// The pinnable container it is in, read observed (Compose's retrievePinnableContainer).
    fn retrieve_pinnable_container(&self) -> Option<PinnableContainerRef> {
        let handle = self.handle.as_ref()?;
        handle.observe_reads(|| handle.current_value_of(&LocalPinnableContainer))
    }

    /// Takes a new element (Compose's update): the text node's updates, the new controller told it is placed
    /// (the old one unplaced) and the pinnable container; measured again always.
    fn update(&mut self, element: &TextAnnotatedStringElement) {
        element.update_node(&mut self.text_node);
        let controller = element.selection_controller.clone();
        if self.is_placed && !same_rc(&controller, &self.selection_controller) {
            if let Some(old) = &self.selection_controller {
                old.on_unplaced();
            }
            if let Some(new) = &controller {
                new.on_placed();
            }
        }
        if let Some(c) = &controller {
            c.update_pinnable_container(self.retrieve_pinnable_container());
        }
        self.selection_controller = controller;
        if let Some(h) = &self.handle {
            h.invalidate_measurement();
        }
    }
}

impl ModifierNode for SelectableTextAnnotatedStringNode {
    node_capabilities!(layout, draw, semantics, global_position_aware, layout_aware, unplaced_aware, observer);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        self.text_node.attach(cx);
        if let Some(c) = &self.selection_controller {
            c.update_pinnable_container(self.retrieve_pinnable_container());
        }
    }
    fn on_detach(&mut self, _cx: &NodeCx) {
        if let Some(c) = &self.selection_controller {
            c.update_pinnable_container(None);
        }
        self.text_node.detach();
        self.handle = None;
    }
    fn should_auto_invalidate(&self) -> bool {
        false
    }
}

impl ObserverModifierNode for SelectableTextAnnotatedStringNode {
    fn on_observed_reads_changed(&mut self) {
        if let Some(c) = &self.selection_controller {
            c.update_pinnable_container(self.retrieve_pinnable_container());
        }
    }
}

impl LayoutAwareModifierNode for SelectableTextAnnotatedStringNode {
    fn on_placed(&mut self, _coordinates: &LayoutCoordinates) {
        if self.is_placed {
            return;
        }
        self.is_placed = true;
        if let Some(c) = &self.selection_controller {
            c.on_placed();
        }
    }
    fn observes_remeasure(&self) -> bool {
        false
    }
}

impl UnplacedAwareModifierNode for SelectableTextAnnotatedStringNode {
    fn on_unplaced(&mut self) {
        if !self.is_placed {
            return;
        }
        self.is_placed = false;
        if let Some(c) = &self.selection_controller {
            c.on_unplaced();
        }
    }
}

impl GlobalPositionAwareModifierNode for SelectableTextAnnotatedStringNode {
    fn on_globally_positioned(&mut self, coordinates: &LayoutCoordinates) {
        if let Some(c) = &self.selection_controller {
            c.update_layout_coordinates(coordinates);
        }
    }
}

impl LayoutModifierNode for SelectableTextAnnotatedStringNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        self.text_node.measure(scope, measurable, constraints)
    }
    fn min_intrinsic_width(&mut self, scope: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, height: i32) -> i32 {
        self.text_node.min_intrinsic_width(scope, m, height)
    }
    fn min_intrinsic_height(&mut self, scope: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        self.text_node.min_intrinsic_height(scope, m, width)
    }
    fn max_intrinsic_width(&mut self, scope: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, height: i32) -> i32 {
        self.text_node.max_intrinsic_width(scope, m, height)
    }
    fn max_intrinsic_height(&mut self, scope: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        self.text_node.max_intrinsic_height(scope, m, width)
    }
}

impl DrawModifierNode for SelectableTextAnnotatedStringNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        self.text_node.draw(scope);
    }
}

impl SemanticsModifierNode for SelectableTextAnnotatedStringNode {
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        self.text_node.apply_semantics(config);
    }
}
