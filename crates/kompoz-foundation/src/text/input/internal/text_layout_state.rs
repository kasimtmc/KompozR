//! A state-based field's text layout and where its parts are (Compose's TextLayoutState): the layout of its shown
//! text, the coordinates of the decoration box, the core field (the visible text) and the text layout node (the
//! scrolling text), and the conversions between them.
//!
//! The layout itself is made by the field's layout node (Compose's TextFieldLayoutStateCache in
//! `updateNonMeasureInputs`/`layoutWithNewMeasureInputs`, the BasicTextField composable's), which gives it here
//! with [`set_layout_result`](TextLayoutState::set_layout_result); readers of [`layout_result`](
//! TextLayoutState::layout_result) hear of it as of a state's change.

use crate::relocation::BringIntoViewRequester;
use kompoz_graphics::{Density, Dp, Offset, Rect};
use kompoz_runtime::{MutableState, mutable_state_of};
use kompoz_text::TextLayoutResult;
use kompoz_ui::layout::LayoutCoordinates;
use std::cell::RefCell;
use std::rc::Rc;

/// A value read as a snapshot state, changed (and told) at each set even when equal (Compose's
/// `mutableStateOf(…, neverEqualPolicy())` of a value that isn't a snapshot one).
struct Observed<T> {
    value: RefCell<T>,
    version: MutableState<u64>,
}

impl<T: Clone> Observed<T> {
    fn new(value: T) -> Observed<T> {
        Observed { value: RefCell::new(value), version: mutable_state_of(0) }
    }
    fn get(&self) -> T {
        self.version.get();
        self.value.borrow().clone()
    }
    fn set(&self, value: T) {
        *self.value.borrow_mut() = value;
        self.version.set(self.version.peek().wrapping_add(1));
    }
}

/// Called with the layout as it is made (Compose's `onTextLayout`, BasicTextField's): the density and a getter of
/// the latest layout.
pub type OnTextFieldLayout = Rc<dyn Fn(Density, &dyn Fn() -> Option<TextLayoutResult>)>;

/// A field's text layout and the coordinates of its parts (Compose's TextLayoutState).
pub struct TextLayoutState {
    layout_result: Observed<Option<TextLayoutResult>>,
    /// The scrolling text's node.
    text_layout_node_coordinates: Observed<Option<LayoutCoordinates>>,
    /// The visible inner field.
    core_node_coordinates: Observed<Option<LayoutCoordinates>>,
    /// The decoration box around it.
    decorator_node_coordinates: Observed<Option<LayoutCoordinates>>,
    /// A single-line field's height, not to cut its text (0: none set).
    height_for_single_line_field: MutableState<Dp>,
    /// Brings ranges of the text into view (on the core node, inside the field's scroll container).
    bring_into_view_requester: BringIntoViewRequester,
    on_text_layout: RefCell<Option<OnTextFieldLayout>>,
}

impl Default for TextLayoutState {
    fn default() -> Self {
        TextLayoutState::new()
    }
}

impl TextLayoutState {
    pub fn new() -> TextLayoutState {
        TextLayoutState {
            layout_result: Observed::new(None),
            text_layout_node_coordinates: Observed::new(None),
            core_node_coordinates: Observed::new(None),
            decorator_node_coordinates: Observed::new(None),
            height_for_single_line_field: mutable_state_of(Dp(0.0)),
            bring_into_view_requester: BringIntoViewRequester::new(),
            on_text_layout: RefCell::new(None),
        }
    }

    /// The layout of the shown text, if it was laid out (read as a state).
    pub fn layout_result(&self) -> Option<TextLayoutResult> {
        self.layout_result.get()
    }

    /// The field's node made a layout (its readers told); `on_text_layout` hears of it.
    pub fn set_layout_result(&self, layout_result: Option<TextLayoutResult>, density: Density) {
        self.layout_result.set(layout_result);
        let callback = self.on_text_layout.borrow().clone();
        if let Some(on_text_layout) = callback {
            let latest = || self.layout_result.value.borrow().clone();
            on_text_layout(density, &latest);
        }
    }

    /// What hears of each layout (Compose's `onTextLayout`).
    pub fn set_on_text_layout(&self, on_text_layout: Option<OnTextFieldLayout>) {
        *self.on_text_layout.borrow_mut() = on_text_layout;
    }

    pub fn text_layout_node_coordinates(&self) -> Option<LayoutCoordinates> {
        self.text_layout_node_coordinates.get()
    }

    /// The text layout node was placed (told even when the same coordinates: they are where it is now).
    pub fn set_text_layout_node_coordinates(&self, coordinates: Option<LayoutCoordinates>) {
        self.text_layout_node_coordinates.set(coordinates);
    }

    pub fn core_node_coordinates(&self) -> Option<LayoutCoordinates> {
        self.core_node_coordinates.get()
    }

    pub fn set_core_node_coordinates(&self, coordinates: Option<LayoutCoordinates>) {
        self.core_node_coordinates.set(coordinates);
    }

    pub fn decorator_node_coordinates(&self) -> Option<LayoutCoordinates> {
        self.decorator_node_coordinates.get()
    }

    pub fn set_decorator_node_coordinates(&self, coordinates: Option<LayoutCoordinates>) {
        self.decorator_node_coordinates.set(coordinates);
    }

    /// A single-line field's height (Compose's `heightForSingleLineField`).
    pub fn height_for_single_line_field(&self) -> Dp {
        self.height_for_single_line_field.get()
    }

    pub fn set_height_for_single_line_field(&self, height: Dp) {
        self.height_for_single_line_field.set(height);
    }

    /// What brings the text's ranges into view.
    pub fn bring_into_view_requester(&self) -> &BringIntoViewRequester {
        &self.bring_into_view_requester
    }

    /// The text offset at `position` (relative to the decoration box), or none if not laid out (Compose's
    /// `getOffsetForPosition`, -1 there): the position kept within the visible text first if
    /// `coerce_in_visible_bounds`.
    pub fn get_offset_for_position(&self, position: Offset, coerce_in_visible_bounds: bool) -> Option<usize> {
        let layout = self.layout_result()?;
        let position =
            if coerce_in_visible_bounds { self.coerced_in_visible_bounds_of_input_text(position) } else { position };
        Some(layout.get_offset_for_position(self.from_decoration_to_text_layout(position)))
    }

    /// Whether `offset` (relative to the decoration box) is over a line's text, not the space beside it (Compose's
    /// `isPositionOnText`; above or below the text counts).
    pub fn is_position_on_text(&self, offset: Offset) -> bool {
        let Some(layout) = self.layout_result() else { return false };
        let relative = self.from_decoration_to_text_layout(self.coerced_in_visible_bounds_of_input_text(offset));
        let line = layout.get_line_for_vertical_position(relative.y);
        relative.x >= layout.get_line_left(line) && relative.x <= layout.get_line_right(line)
    }

    /// `offset` (relative to the decoration box) kept within the visible text (Compose's
    /// `coercedInVisibleBoundsOfInputText`).
    pub fn coerced_in_visible_bounds_of_input_text(&self, offset: Offset) -> Offset {
        let visible = match self.text_layout_node_coordinates() {
            Some(text) if text.is_attached() => match self.decorator_node_coordinates() {
                Some(decorator) => decorator.local_bounding_box_of(&text, true),
                None => Rect::Zero,
            },
            _ => Rect::Zero,
        };
        coerce_in(offset, visible)
    }

    /// A position in the text layout node's coordinates in the core node's (Compose's `fromTextLayoutToCore`).
    pub fn from_text_layout_to_core(&self, offset: Offset) -> Offset {
        match (self.text_layout_node_coordinates(), self.core_node_coordinates()) {
            (Some(text), Some(core)) if text.is_attached() && core.is_attached() => {
                core.local_position_of(&text, offset)
            }
            _ => offset,
        }
    }

    /// A position on the decoration box in the text layout node's coordinates (Compose's
    /// `fromDecorationToTextLayout`).
    pub fn from_decoration_to_text_layout(&self, offset: Offset) -> Offset {
        match (self.text_layout_node_coordinates(), self.decorator_node_coordinates()) {
            (Some(text), Some(decorator)) if text.is_attached() && decorator.is_attached() => {
                text.local_position_of(&decorator, offset)
            }
            _ => offset,
        }
    }

    /// A position in the window in the decoration box's coordinates (Compose's `fromWindowToDecoration`).
    pub fn from_window_to_decoration(&self, offset: Offset) -> Offset {
        match self.decorator_node_coordinates() {
            Some(decorator) if decorator.is_attached() => decorator.window_to_local(offset),
            _ => offset,
        }
    }

    /// A rect in the text layout node's coordinates in the decoration box's (Compose's
    /// `fromTextLayoutToDecoration`).
    pub fn from_text_layout_to_decoration(&self, rect: Rect) -> Rect {
        let (Some(text), Some(decorator)) = (self.text_layout_node_coordinates(), self.decorator_node_coordinates())
        else {
            return rect;
        };
        if !text.is_attached() || !decorator.is_attached() {
            return rect;
        }
        let top_left = decorator.local_bounding_box_of(&text, false);
        rect.translate(Offset::new(top_left.left, top_left.top))
    }
}

/// `offset` kept within `rect` (Compose's `Offset.coerceIn(rect)`).
pub fn coerce_in(offset: Offset, rect: Rect) -> Offset {
    let x = if offset.x < rect.left {
        rect.left
    } else if offset.x > rect.right {
        rect.right
    } else {
        offset.x
    };
    let y = if offset.y < rect.top {
        rect.top
    } else if offset.y > rect.bottom {
        rect.bottom
    } else {
        offset.y
    };
    Offset::new(x, y)
}
