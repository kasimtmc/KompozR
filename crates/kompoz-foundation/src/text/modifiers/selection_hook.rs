//! Where a text meets the selection it is in: what `BasicText` and its selectable node ask of it, as Compose's
//! BasicText and SelectableTextAnnotatedStringNode call its SelectionRegistrar and SelectionController.
//!
//! The selection side (`text::selection`: SelectionContainer, its registrar and manager) implements these:
//! - [`SelectionRegistrar`], provided by a selection container through [`LocalSelectionRegistrar`]: BasicText takes
//!   a selectable id from it (remembered and saved while that text has a selection, as Compose's
//!   `selectionIdSaver`) and makes its text's [`SelectionController`] with it (Compose's
//!   `SelectionController(selectableId, selectionRegistrar, backgroundSelectionColor)`, remembered for the id,
//!   the registrar and [`LocalTextSelectionColors`]' background color);
//! - [`SelectionController`], one per text: its [`modifier`](SelectionController::modifier) goes on the text's
//!   layout before its text node (the selection gestures, bringing into view, the text pointer icon), and the
//!   selectable text node tells it where the text is placed, laid out and pinned and lets it draw the selection
//!   under the text.
//!
//! Where no registrar is provided (outside a selection container), BasicText takes its non-selectable path.

use kompoz_graphics::Color;
use kompoz_runtime::KompozitionLocal;
use kompoz_text::TextLayoutResult;
use kompoz_ui::Modifier;
use kompoz_ui::draw::DrawScope;
use kompoz_ui::layout::{LayoutCoordinates, PinnableContainerRef};
use std::rc::Rc;

/// The selection a text is in, as BasicText uses it (Compose's SelectionRegistrar: the part a text calls): ids for
/// its texts, whether one has a selection, and the controller each text works with.
pub trait SelectionRegistrar {
    /// A new id for a text taking part in the selection (Compose's nextSelectableId; never 0, Compose's
    /// InvalidSelectableId).
    fn next_selectable_id(&self) -> i64;

    /// Whether the text of `selectable_id` has a part of the selection (Compose's `subselections` holding it): its
    /// id is saved then, so the selection finds it again after the state is restored.
    fn has_selection(&self, selectable_id: i64) -> bool;

    /// The controller of the text of `selectable_id` (Compose's `SelectionController(selectableId, this,
    /// backgroundSelectionColor)`), its selection drawn in `background_selection_color`.
    fn selection_controller(
        &self,
        selectable_id: i64,
        background_selection_color: Color,
    ) -> Rc<dyn SelectionController>;
}

/// A text's part in a selection (Compose's SelectionController): the modifier its layout gets, and what its
/// selectable node tells it.
pub trait SelectionController {
    /// What the text's layout gets before its text node (Compose's SelectionController.modifier: the selection's
    /// pointer input, a bring-into-view requester, the text pointer icon). The same modifier each time it is
    /// asked for (it is compared).
    fn modifier(&self) -> Modifier;

    /// The text was placed (Compose's onPlaced: it subscribes its selectable to the registrar).
    fn on_placed(&self);

    /// The text is no longer placed (Compose's onUnplaced: it unsubscribes it).
    fn on_unplaced(&self);

    /// The text was laid out anew (Compose's updateTextLayout: another text than before is a change of the
    /// selectable).
    fn update_text_layout(&self, text_layout_result: &TextLayoutResult);

    /// Where the text is now (Compose's updateLayoutCoordinates: the registrar is told its position changed).
    fn update_layout_coordinates(&self, coordinates: &LayoutCoordinates);

    /// The pinnable container the text is in, if any (Compose's updatePinnableContainer).
    fn update_pinnable_container(&self, pinnable_container: Option<PinnableContainerRef>);

    /// Draws the text's part of the selection under it (Compose's SelectionController.draw).
    fn draw(&self, scope: &mut DrawScope);
}

/// A [`SelectionRegistrar`] as [`LocalSelectionRegistrar`] holds it: compared by identity.
#[derive(Clone)]
pub struct SelectionRegistrarRef(pub Rc<dyn SelectionRegistrar>);

impl PartialEq for SelectionRegistrarRef {
    fn eq(&self, other: &SelectionRegistrarRef) -> bool {
        std::ptr::eq(Rc::as_ptr(&self.0) as *const (), Rc::as_ptr(&other.0) as *const ())
    }
}

/// The selection what is composed here is in, none outside a selection container (Compose's
/// LocalSelectionRegistrar).
pub static LocalSelectionRegistrar: KompozitionLocal<Option<SelectionRegistrarRef>> = KompozitionLocal::new(|| None);

/// The colors of selected text (Compose's TextSelectionColors): its handles', and its background's (drawn under
/// the selected text).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextSelectionColors {
    pub handle_color: Color,
    pub background_color: Color,
}

/// Compose's DefaultSelectionColor (0xFF4286F4).
const DEFAULT_SELECTION_COLOR: Color = Color::from_argb(0xFF4286F4);

impl TextSelectionColors {
    /// Compose's DefaultTextSelectionColors: the default selection color, its background at 0.4 alpha.
    pub fn default_colors() -> TextSelectionColors {
        TextSelectionColors {
            handle_color: DEFAULT_SELECTION_COLOR,
            background_color: Color::new(
                DEFAULT_SELECTION_COLOR.red(),
                DEFAULT_SELECTION_COLOR.green(),
                DEFAULT_SELECTION_COLOR.blue(),
                0.4,
            ),
        }
    }
}

/// The colors selected text is shown in here (Compose's LocalTextSelectionColors).
pub static LocalTextSelectionColors: KompozitionLocal<TextSelectionColors> =
    KompozitionLocal::new(TextSelectionColors::default_colors);
