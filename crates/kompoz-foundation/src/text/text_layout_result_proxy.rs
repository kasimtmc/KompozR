//! A value-based field's text layout with where it is (Compose's TextLayoutResultProxy.kt): the layout of the inner
//! field's text, and the inner field's and decoration box's coordinates to translate a pointer's position on the
//! decoration box into the text's (kept within the visible text).

use kompoz_graphics::{Offset, Rect};
use kompoz_text::TextLayoutResult;
use kompoz_ui::layout::LayoutCoordinates;
use std::cell::RefCell;

/// A field's text layout and where the inner field and its decoration box are (Compose's TextLayoutResultProxy):
/// a position on the decoration box is translated into the text's, within what shows of it.
pub struct TextLayoutResultProxy {
    /// The layout.
    pub value: TextLayoutResult,
    /// The inner field's coordinates, where the text is shown.
    inner_text_field_coordinates: RefCell<Option<LayoutCoordinates>>,
    /// The decoration box's coordinates: the field's outer bounds.
    decoration_box_coordinates: RefCell<Option<LayoutCoordinates>>,
}

impl TextLayoutResultProxy {
    /// `value` with the decoration box's coordinates known so far.
    pub fn new(
        value: TextLayoutResult,
        decoration_box_coordinates: Option<LayoutCoordinates>,
    ) -> TextLayoutResultProxy {
        TextLayoutResultProxy {
            value,
            inner_text_field_coordinates: RefCell::new(None),
            decoration_box_coordinates: RefCell::new(decoration_box_coordinates),
        }
    }

    pub fn inner_text_field_coordinates(&self) -> Option<LayoutCoordinates> {
        self.inner_text_field_coordinates.borrow().clone()
    }

    pub fn set_inner_text_field_coordinates(&self, coordinates: Option<LayoutCoordinates>) {
        *self.inner_text_field_coordinates.borrow_mut() = coordinates;
    }

    pub fn decoration_box_coordinates(&self) -> Option<LayoutCoordinates> {
        self.decoration_box_coordinates.borrow().clone()
    }

    pub fn set_decoration_box_coordinates(&self, coordinates: Option<LayoutCoordinates>) {
        *self.decoration_box_coordinates.borrow_mut() = coordinates;
    }

    /// The text's offset at `position` on the decoration box (Compose's `getOffsetForPosition`): kept within the
    /// visible text when `coerce_in_visible_bounds`.
    pub fn get_offset_for_position(&self, position: Offset, coerce_in_visible_bounds: bool) -> usize {
        let position =
            if coerce_in_visible_bounds { self.coerced_in_visible_bounds_of_input_text(position) } else { position };
        self.value.get_offset_for_position(self.translate_decoration_to_inner_coordinates(position))
    }

    /// The text's line at `vertical` on the decoration box (Compose's `getLineForVerticalPosition`).
    pub fn get_line_for_vertical_position(&self, vertical: f32) -> usize {
        let coerced = self.coerced_in_visible_bounds_of_input_text(Offset::new(0.0, vertical));
        self.value.get_line_for_vertical_position(self.translate_decoration_to_inner_coordinates(coerced).y)
    }

    /// The end of `line_index` (Compose's `getLineEnd`).
    pub fn get_line_end(&self, line_index: usize, visible_end: bool) -> usize {
        self.value.get_line_end(line_index, visible_end)
    }

    /// Whether `offset` on the decoration box is over the text, within its line's left and right (Compose's
    /// `isPositionOnText`).
    pub fn is_position_on_text(&self, offset: Offset) -> bool {
        let visible = self.coerced_in_visible_bounds_of_input_text(offset);
        let relative = self.translate_decoration_to_inner_coordinates(visible);
        let line = self.value.get_line_for_vertical_position(relative.y);
        relative.x >= self.value.get_line_left(line) && relative.x <= self.value.get_line_right(line)
    }

    /// `offset` on the decoration box in the inner field's coordinates (Compose's
    /// `translateDecorationToInnerCoordinates`): as it is unless both are attached.
    pub fn translate_decoration_to_inner_coordinates(&self, offset: Offset) -> Offset {
        let (Some(inner), Some(decoration)) = (self.attached_inner(), self.attached_decoration()) else {
            return offset;
        };
        inner.local_position_of(&decoration, offset)
    }

    /// `offset` in the inner field's coordinates on the decoration box (Compose's
    /// `translateInnerToDecorationCoordinates`).
    pub fn translate_inner_to_decoration_coordinates(&self, offset: Offset) -> Offset {
        let (Some(inner), Some(decoration)) = (self.attached_inner(), self.attached_decoration()) else {
            return offset;
        };
        decoration.local_position_of(&inner, offset)
    }

    fn attached_inner(&self) -> Option<LayoutCoordinates> {
        self.inner_text_field_coordinates().filter(|c| c.is_attached())
    }

    fn attached_decoration(&self) -> Option<LayoutCoordinates> {
        self.decoration_box_coordinates().filter(|c| c.is_attached())
    }

    /// `offset` kept within the inner field's bounds on the decoration box (Compose's
    /// `coercedInVisibleBoundsOfInputText`): an unattached inner field's are empty.
    fn coerced_in_visible_bounds_of_input_text(&self, offset: Offset) -> Offset {
        let visible = match self.inner_text_field_coordinates() {
            Some(inner) if inner.is_attached() => {
                self.decoration_box_coordinates().map_or(Rect::Zero, |d| d.local_bounding_box_of(&inner, true))
            }
            Some(_) => Rect::Zero,
            None => Rect::Zero,
        };
        coerce_in(offset, visible)
    }
}

/// `offset` within `rect` (Compose's private `Offset.coerceIn`: an edge where it is outside).
fn coerce_in(offset: Offset, rect: Rect) -> Offset {
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
