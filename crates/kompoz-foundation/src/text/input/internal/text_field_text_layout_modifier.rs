//! A state-based field's text layout node (Compose's TextFieldTextLayoutModifier): it lays the shown text out
//! (through the layout state's cache), is that size, provides the baselines, tells the layout state where it is,
//! a single-line field's line height, and is what the field's bring-into-view requests start from.

use super::text_layout_state::{OnTextFieldLayout, TextLayoutState};
use super::transformed_text_field_state::TransformedTextFieldState;
use crate::relocation::BringIntoViewRequesterNode;
use crate::text::keyboard_options::KeyboardOptions;
use kompoz_graphics::{Constraints, Density, Dp};
use kompoz_text::TextStyle;
use kompoz_ui::layout::{
    FirstBaseline, GlobalPositionAwareModifierNode, LastBaseline, LayoutCoordinates, LayoutModifierNode, Measurable,
};
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::platform::LocalFontFamilyResolver;
use kompoz_ui::{
    MeasureResult, MeasureScope, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities,
};
use std::rc::Rc;

/// The text layout node's element (Compose's TextFieldTextLayoutModifier): equal for the same states, style,
/// single-lineness, callback and keyboard options.
#[derive(Clone)]
pub struct TextFieldTextLayoutModifier {
    pub text_layout_state: Rc<TextLayoutState>,
    pub text_field_state: TransformedTextFieldState,
    pub text_style: TextStyle,
    pub single_line: bool,
    pub on_text_layout: Option<OnTextFieldLayout>,
    pub keyboard_options: KeyboardOptions,
}

impl PartialEq for TextFieldTextLayoutModifier {
    fn eq(&self, other: &Self) -> bool {
        let same_callback = match (&self.on_text_layout, &other.on_text_layout) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        self.single_line == other.single_line
            && Rc::ptr_eq(&self.text_layout_state, &other.text_layout_state)
            && self.text_field_state == other.text_field_state
            && self.text_style == other.text_style
            && same_callback
            && self.keyboard_options == other.keyboard_options
    }
}

impl ModifierNodeElement for TextFieldTextLayoutModifier {
    type Node = TextFieldTextLayoutModifierNode;
    fn create(&self) -> TextFieldTextLayoutModifierNode {
        TextFieldTextLayoutModifierNode::new(self)
    }
    fn update(&self, node: &mut TextFieldTextLayoutModifierNode) {
        node.update_node(self);
    }
    fn name(&self) -> &'static str {
        "textFieldTextLayout"
    }
}

/// The text layout node (Compose's TextFieldTextLayoutModifierNode).
pub struct TextFieldTextLayoutModifierNode {
    text_layout_state: Rc<TextLayoutState>,
    single_line: bool,
    bring_into_view_requester_node: Option<DelegatedNode<BringIntoViewRequesterNode>>,
    handle: Option<NodeHandle>,
}

impl TextFieldTextLayoutModifierNode {
    fn new(element: &TextFieldTextLayoutModifier) -> TextFieldTextLayoutModifierNode {
        let state = element.text_layout_state.clone();
        state.set_on_text_layout(element.on_text_layout.clone());
        state.update_non_measure_inputs(
            &element.text_field_state,
            &element.text_style,
            element.single_line,
            !element.single_line,
            &element.keyboard_options,
        );
        TextFieldTextLayoutModifierNode {
            text_layout_state: state,
            single_line: element.single_line,
            bring_into_view_requester_node: None,
            handle: None,
        }
    }

    /// Takes a new element's values (Compose's `updateNode`).
    fn update_node(&mut self, element: &TextFieldTextLayoutModifier) {
        let previous = self.text_layout_state.clone();
        self.text_layout_state = element.text_layout_state.clone();
        self.text_layout_state.set_on_text_layout(element.on_text_layout.clone());
        self.single_line = element.single_line;
        self.text_layout_state.update_non_measure_inputs(
            &element.text_field_state,
            &element.text_style,
            element.single_line,
            !element.single_line,
            &element.keyboard_options,
        );
        if !Rc::ptr_eq(&previous, &self.text_layout_state)
            && let Some(node) = &self.bring_into_view_requester_node
        {
            node.borrow_mut().update_requester(self.text_layout_state.bring_into_view_requester().clone());
        }
    }
}

impl ModifierNode for TextFieldTextLayoutModifierNode {
    node_capabilities!(layout, global_position_aware);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        if self.bring_into_view_requester_node.is_none() {
            let requester = self.text_layout_state.bring_into_view_requester().clone();
            self.bring_into_view_requester_node = Some(cx.delegate(BringIntoViewRequesterNode::new(requester)));
        }
    }
}

impl GlobalPositionAwareModifierNode for TextFieldTextLayoutModifierNode {
    fn on_globally_positioned(&mut self, coordinates: &LayoutCoordinates) {
        self.text_layout_state.set_text_layout_node_coordinates(Some(coordinates.clone()));
    }
}

impl LayoutModifierNode for TextFieldTextLayoutModifierNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let resolver = self
            .handle
            .as_ref()
            .map(|h| h.current_value_of(&LocalFontFamilyResolver))
            .unwrap_or_else(|| LocalFontFamilyResolver.default_value());
        let density = Density::new(scope.density, scope.font_scale);
        let result = self.text_layout_state.layout_with_new_measure_inputs(
            density,
            scope.layout_direction(),
            resolver,
            constraints,
        );
        let (width, height) = (result.size.width, result.size.height);
        let placeable = measurable.measure(Constraints::fit_prioritizing_width(width, width, height, height));
        // A single-line field is as tall as its line (a tall glyph's line isn't cut).
        let single_line_height =
            if self.single_line { density.to_dp(result.get_line_bottom(0).ceil()) } else { Dp(0.0) };
        self.text_layout_state.set_height_for_single_line_field(single_line_height);
        let alignment_lines = vec![
            (FirstBaseline.into(), result.first_baseline().round() as i32),
            (LastBaseline.into(), result.last_baseline().round() as i32),
        ];
        MeasureResult::new(width, height, alignment_lines, move |p| p.place(&placeable, 0, 0))
    }
}
