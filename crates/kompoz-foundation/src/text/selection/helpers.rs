//! What selection asks of a text's layout (Compose's SelectionHelpers.kt, TextLayoutHelper's `getLineHeight` and
//! TextSelectionDelegate.kt), and stepping offsets by a character (Compose's `offset ± 1` on UTF-16 code units are
//! UTF-8 character boundaries here).

use kompoz_graphics::Offset;
use kompoz_text::{ResolvedTextDirection, TextLayoutResult, TextRange};

/// The start of the character before `offset` (0 at 0): Compose's `offset - 1`.
pub(crate) fn previous_char(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    text[..offset].char_indices().next_back().map_or(0, |(i, _)| i)
}

/// The end of the character at `offset` (the text's length at its end): Compose's `offset + 1`.
pub(crate) fn next_char(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }
    text[offset..].chars().next().map_or(text.len(), |c| offset + c.len_utf8())
}

/// The direction of the text at `offset` (Compose's `getTextDirectionForOffset`): its bidi run's, or on an empty
/// line its paragraph's (an empty line's run is always left to right, even in a right-to-left paragraph).
pub fn get_text_direction_for_offset(layout: &TextLayoutResult, offset: usize) -> ResolvedTextDirection {
    if is_offset_an_empty_line(layout, offset) {
        layout.get_paragraph_direction(offset)
    } else {
        layout.get_bidi_run_direction(offset)
    }
}

fn is_offset_an_empty_line(layout: &TextLayoutResult, offset: usize) -> bool {
    let text = layout.layout_input.text.text();
    if text.is_empty() {
        return true;
    }
    let current_line = layout.get_line_for_offset(offset);
    (offset == 0 || current_line != layout.get_line_for_offset(previous_char(text, offset)))
        && (offset >= text.len() || current_line != layout.get_line_for_offset(next_char(text, offset)))
}

/// The height of the line `offset` is on, 0 past a line's end or for no text (Compose's
/// `TextLayoutResult.getLineHeight(offset)`).
pub fn get_line_height(layout: &TextLayoutResult, offset: usize) -> f32 {
    let text = layout.layout_input.text.text();
    if text.is_empty() {
        return 0.0;
    }
    let paragraph = &layout.multi_paragraph;
    let line = paragraph
        .get_line_for_offset(offset)
        .min(paragraph.max_lines().saturating_sub(1))
        .min(paragraph.line_count().saturating_sub(1));
    let line_end = paragraph.get_line_end(line, false);
    if offset > line_end {
        return 0.0;
    }
    paragraph.get_line_height(line)
}

/// Where a selection handle at `offset` points (Compose's `getSelectionHandleCoordinates`): the caret's x (on the
/// side of the selected text) at its line's bottom, within the text's size; unspecified on a line cut off by
/// `max_lines`.
pub fn get_selection_handle_coordinates(
    layout: &TextLayoutResult,
    offset: usize,
    is_start: bool,
    are_handles_crossed: bool,
) -> Offset {
    let line = layout.get_line_for_offset(offset);
    if line >= layout.line_count() {
        return Offset::Unspecified;
    }
    let x = get_handle_horizontal_position(layout, offset, is_start, are_handles_crossed)
        .clamp(0.0, layout.size.width as f32);
    let y = layout.get_line_bottom(line).clamp(0.0, layout.size.height as f32);
    Offset::new(x, y)
}

/// The caret's x at `offset` for a handle (Compose's `getHorizontalPosition(offset, isStart, areHandlesCrossed)`):
/// on the primary direction's side when the run there goes as its paragraph does.
pub(crate) fn get_handle_horizontal_position(
    layout: &TextLayoutResult,
    offset: usize,
    is_start: bool,
    are_handles_crossed: bool,
) -> f32 {
    let offset_to_check = if is_start && !are_handles_crossed || !is_start && are_handles_crossed {
        offset
    } else {
        previous_char(layout.layout_input.text.text(), offset)
    };
    let bidi_run_direction = layout.get_bidi_run_direction(offset_to_check);
    let paragraph_direction = layout.get_paragraph_direction(offset);
    layout.get_horizontal_position(offset, bidi_run_direction == paragraph_direction)
}

/// Whether `position` is on a selected character of `selection_range` (Compose's
/// `TextLayoutResult.isPositionInsideSelection`): the offset there and the one before (a position right of a
/// glyph's middle gives the next offset).
pub fn is_position_inside_selection(
    layout: &TextLayoutResult,
    position: Offset,
    selection_range: Option<TextRange>,
) -> bool {
    let Some(range) = selection_range.filter(|r| !r.collapsed()) else { return false };
    let selected_and_contains =
        |offset: usize| range.contains_offset(offset) && layout.get_bounding_box(offset).contains(position);
    let offset = layout.get_offset_for_position(position);
    selected_and_contains(offset)
        || (offset > 0 && selected_and_contains(previous_char(layout.layout_input.text.text(), offset)))
}
