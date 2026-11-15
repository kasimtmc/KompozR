//! What makes a text selectable in a selection container (Compose's SelectionController.kt): BasicText makes one
//! for its text in a container (an id from the registrar, the selection colors' background); the text's node tells
//! it where the text is and how it is laid out, and it subscribes the text's selectable, takes its gestures and
//! draws its part of the selection.
//!
//! The text's node (Compose's SelectableTextAnnotatedStringNode) calls it: [`on_placed`] and [`on_unplaced`] as it is
//! placed and unplaced, [`update_layout_coordinates`] when it is positioned, [`update_text_layout`] after each
//! layout, [`update_pinnable_container`] with its `LocalPinnableContainer` (at attach, none at detach), and
//! [`draw`] before drawing the text; its modifier goes before the node's.
//!
//! [`on_placed`]: SelectionController::on_placed
//! [`on_unplaced`]: SelectionController::on_unplaced
//! [`update_layout_coordinates`]: SelectionController::update_layout_coordinates
//! [`update_text_layout`]: SelectionController::update_text_layout
//! [`update_pinnable_container`]: SelectionController::update_pinnable_container
//! [`draw`]: SelectionController::draw

use super::selection_modifier::{LayoutCoordinatesProvider, selection_modifier};
use crate::relocation::{BringIntoViewRequester, RelocationModifiers};
use crate::text::TextPointerIcon;
use crate::text::selection::{MultiWidgetSelectionDelegate, Selectable, SelectionRegistrar};
use kompoz_graphics::{Color, Path};
use kompoz_text::{TextLayoutResult, TextOverflow};
use kompoz_ui::draw::DrawScope;
use kompoz_ui::graphics_context::ClipOp;
use kompoz_ui::layout::PinnableContainerRef;
use kompoz_ui::{LayoutCoordinates, Modifier};
use std::cell::RefCell;
use std::rc::Rc;

/// What a selectable text's selection knows of it (Compose's StaticTextSelectionParams): where it is, its layout,
/// the container that pins it.
#[derive(Clone, Default)]
pub struct StaticTextSelectionParams {
    pub layout_coordinates: Option<LayoutCoordinates>,
    pub text_layout_result: Option<TextLayoutResult>,
    pub pinnable_container: Option<PinnableContainerRef>,
}

impl StaticTextSelectionParams {
    /// The outline of `start..end`'s selection, none before a layout.
    pub fn get_path_for_range(&self, start: usize, end: usize) -> Option<Path> {
        self.text_layout_result.as_ref().map(|r| r.get_path_for_range(start, end))
    }

    /// Whether the selection is cut to the text's bounds: it overflows and doesn't show its overflow.
    pub fn should_clip(&self) -> bool {
        self.text_layout_result
            .as_ref()
            .is_some_and(|r| r.layout_input.overflow != TextOverflow::Visible && r.has_visual_overflow())
    }
}

/// A text's selection in a selection container (Compose's SelectionController): its selectable (a
/// [`MultiWidgetSelectionDelegate`] over what its node tells it), its gestures and the drawing of its part of the
/// selection.
pub struct SelectionController {
    selectable_id: u64,
    selection_registrar: Rc<dyn SelectionRegistrar>,
    background_selection_color: Color,
    params: Rc<RefCell<StaticTextSelectionParams>>,
    selectable: RefCell<Option<Rc<dyn Selectable>>>,
    bring_into_view_requester: BringIntoViewRequester,
    modifier: Modifier,
}

impl SelectionController {
    pub fn new(
        selectable_id: u64,
        selection_registrar: Rc<dyn SelectionRegistrar>,
        background_selection_color: Color,
    ) -> SelectionController {
        let params = Rc::new(RefCell::new(StaticTextSelectionParams::default()));
        let bring_into_view_requester = BringIntoViewRequester::new();
        let coordinates = params.clone();
        let layout_coordinates_provider: LayoutCoordinatesProvider =
            Rc::new(move || coordinates.borrow().layout_coordinates.clone());
        let modifier = selection_modifier(selection_registrar.clone(), selectable_id, layout_coordinates_provider)
            .bring_into_view_requester(&bring_into_view_requester)
            .pointer_hover_icon(TextPointerIcon);
        SelectionController {
            selectable_id,
            selection_registrar,
            background_selection_color,
            params,
            selectable: RefCell::new(None),
            bring_into_view_requester,
            modifier,
        }
    }

    /// Its text's id in the registrar.
    pub fn selectable_id(&self) -> u64 {
        self.selectable_id
    }

    /// The text's selection gestures, bringing into view and its text pointer icon: before the text's node.
    pub fn modifier(&self) -> Modifier {
        self.modifier.clone()
    }

    /// The text was placed: its selectable subscribes.
    pub fn on_placed(&self) {
        let (coordinates, layout, pinnable) = (self.params.clone(), self.params.clone(), self.params.clone());
        let delegate = MultiWidgetSelectionDelegate::new(
            self.selectable_id,
            move || coordinates.borrow().layout_coordinates.clone(),
            move || layout.borrow().text_layout_result.clone(),
            move || pinnable.borrow().pinnable_container.clone(),
            Some(self.bring_into_view_requester.clone()),
        );
        let selectable = self.selection_registrar.subscribe(Rc::new(delegate));
        *self.selectable.borrow_mut() = Some(selectable);
    }

    /// The text was unplaced: its selectable unsubscribes.
    pub fn on_unplaced(&self) {
        let selectable = self.selectable.borrow_mut().take();
        if let Some(s) = selectable {
            self.selection_registrar.unsubscribe(&*s);
        }
    }

    /// The text was laid out: another text tells the selection it changed (a first layout doesn't).
    pub fn update_text_layout(&self, text_layout_result: &TextLayoutResult) {
        let previous = self.params.borrow().text_layout_result.clone();
        self.params.borrow_mut().text_layout_result = Some(text_layout_result.clone());
        if let Some(previous) = previous
            && previous.layout_input.text != text_layout_result.layout_input.text
        {
            self.selection_registrar.notify_selectable_change(self.selectable_id);
        }
    }

    /// The text was positioned: the selection is told it moved.
    pub fn update_layout_coordinates(&self, coordinates: &LayoutCoordinates) {
        self.params.borrow_mut().layout_coordinates = Some(coordinates.clone());
        self.selection_registrar.notify_position_change(self.selectable_id);
    }

    /// The container that keeps the text composed while it has a selection.
    pub fn update_pinnable_container(&self, pinnable_container: Option<PinnableContainerRef>) {
        self.params.borrow_mut().pinnable_container = pinnable_container;
    }

    /// Draws the text's part of the selection, in the selection background color, cut at its last visible
    /// character (and to its bounds when it overflows clipped). Reads the selection: what draws is drawn again
    /// when it changes.
    pub fn draw(&self, scope: &mut DrawScope) {
        let subselections = self.selection_registrar.subselections();
        let Some(selection) = subselections.get(&self.selectable_id) else { return };
        let (start, end) = if selection.handles_crossed {
            (selection.end.offset, selection.start.offset)
        } else {
            (selection.start.offset, selection.end.offset)
        };
        if start == end {
            return;
        }
        let selectable = self.selectable.borrow().clone();
        let last_offset = selectable.map_or(0, |s| s.get_last_visible_offset());
        let (clipped_start, clipped_end) = (start.min(last_offset), end.min(last_offset));
        let params = self.params.borrow().clone();
        let Some(selection_path) = params.get_path_for_range(clipped_start, clipped_end) else { return };
        let color = self.background_selection_color;
        if params.should_clip() {
            let size = scope.size();
            scope.clip_rect_with(0.0, 0.0, size.width, size.height, ClipOp::Intersect, |s| {
                s.draw_path(&selection_path, color)
            });
        } else {
            scope.draw_path(&selection_path, color);
        }
    }
}
