//! The value-based field's selection moves and deletions by keys (Compose's legacy selection/TextPreparedSelection.kt:
//! BaseTextPreparedSelection, TextFieldPreparedSelection and TextPreparedSelectionState): moves by characters,
//! words, lines, paragraphs and pages as the text was laid out, a movement extending the selection, a deletion's
//! commands — on the field's value, the layout being of what is shown (its offsets mapped).
//!
//! Offsets are UTF-8 bytes: a move by a character steps over a grapheme cluster (Compose's character
//! BreakIterator), a word move searches the layout's words character by character (Compose's UTF-16 unit by unit).

use crate::text::text_layout_result_proxy::TextLayoutResultProxy;
use kompoz_graphics::{Offset, Rect};
use kompoz_text::input::{EditCommand, OffsetMapping, TextFieldValue};
use kompoz_text::segment::{
    find_code_point_or_emoji_start_before, find_following_break, find_paragraph_end, find_paragraph_start,
    find_preceding_break,
};
use kompoz_text::{AnnotatedString, ResolvedTextDirection, TextLayoutResult, TextRange};
use std::rc::Rc;

/// The x a vertical movement keeps (Compose's TextPreparedSelectionState): set at its first move, forgotten by a
/// move of another kind.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextPreparedSelectionState {
    pub cached_x: Option<f32>,
}

impl TextPreparedSelectionState {
    pub fn new() -> TextPreparedSelectionState {
        TextPreparedSelectionState::default()
    }

    /// Forgets the kept x (Compose's `resetCachedX`).
    pub fn reset_cached_x(&mut self) {
        self.cached_x = None;
    }
}

/// A value-based field's selection being moved (Compose's TextFieldPreparedSelection over its
/// BaseTextPreparedSelection): each move changes [`selection`](Self::selection) (moves forgetting the kept x but
/// vertical ones); [`value`](Self::value) is the field's value with it.
pub struct TextFieldPreparedSelection<'a> {
    /// The field's value it started from.
    pub current_value: TextFieldValue,
    pub offset_mapping: OffsetMapping,
    pub layout_result_proxy: Option<Rc<TextLayoutResultProxy>>,
    pub state: &'a mut TextPreparedSelectionState,
    original_selection: TextRange,
    /// The selection as moved.
    pub selection: TextRange,
    /// The text (as it is: no move changes it).
    pub annotated_string: AnnotatedString,
}

/// What a character search finds at a text's end (Compose's NoCharacterFound, BreakIterator.DONE).
pub const NoCharacterFound: Option<usize> = None;

impl<'a> TextFieldPreparedSelection<'a> {
    /// The selection of `current_value` to move, laid out as `layout_result_proxy` says (Compose's
    /// TextFieldPreparedSelection).
    pub fn new(
        current_value: TextFieldValue,
        offset_mapping: OffsetMapping,
        layout_result_proxy: Option<Rc<TextLayoutResultProxy>>,
        state: &'a mut TextPreparedSelectionState,
    ) -> TextFieldPreparedSelection<'a> {
        let selection = current_value.selection();
        let annotated_string = current_value.annotated_string().clone();
        TextFieldPreparedSelection {
            current_value,
            offset_mapping,
            layout_result_proxy,
            state,
            original_selection: selection,
            selection,
            annotated_string,
        }
    }

    /// The field's value with the moved selection (Compose's `value`).
    pub fn value(&self) -> TextFieldValue {
        TextFieldValue::new(self.annotated_string.clone(), self.selection, self.current_value.composition())
    }

    fn text(&self) -> &str {
        self.annotated_string.text()
    }

    fn layout_result(&self) -> Option<&TextLayoutResult> {
        self.layout_result_proxy.as_ref().map(|p| &p.value)
    }

    /// Runs `block` on a non-empty text, the kept x forgotten first if `reset_cached_x` (Compose's `apply`).
    fn apply(&mut self, reset_cached_x: bool, block: impl FnOnce(&mut Self)) -> &mut Self {
        if reset_cached_x {
            self.state.reset_cached_x();
        }
        if !self.text().is_empty() {
            block(self);
        }
        self
    }

    fn set_cursor(&mut self, offset: usize) {
        self.set_selection(offset, offset);
    }

    fn set_selection(&mut self, start: usize, end: usize) {
        self.selection = TextRange::new(start, end);
    }

    /// Selects everything (Compose's `selectAll`).
    pub fn select_all(&mut self) -> &mut Self {
        self.apply(true, |s| {
            let len = s.text().len();
            s.set_selection(0, len);
        })
    }

    /// A caret at the selection's end (Compose's `deselect`).
    pub fn deselect(&mut self) -> &mut Self {
        self.apply(true, |s| s.set_cursor(s.selection.end))
    }

    /// A character to the left: before in left-to-right text, after in right-to-left (Compose's `moveCursorLeft`).
    pub fn move_cursor_left(&mut self) -> &mut Self {
        self.apply(true, |s| if s.is_ltr() { s.move_cursor_prev() } else { s.move_cursor_next() })
    }

    /// A character to the right (Compose's `moveCursorRight`).
    pub fn move_cursor_right(&mut self) -> &mut Self {
        self.apply(true, |s| if s.is_ltr() { s.move_cursor_next() } else { s.move_cursor_prev() })
    }

    /// A selection collapsed to its left side, else `or` (Compose's `collapseLeftOr`).
    pub fn collapse_left_or(&mut self, or: impl FnOnce(&mut Self)) -> &mut Self {
        self.apply(true, |s| {
            if s.selection.collapsed() {
                or(s);
            } else if s.is_ltr() {
                s.set_cursor(s.selection.min());
            } else {
                s.set_cursor(s.selection.max());
            }
        })
    }

    /// A selection collapsed to its right side, else `or` (Compose's `collapseRightOr`).
    pub fn collapse_right_or(&mut self, or: impl FnOnce(&mut Self)) -> &mut Self {
        self.apply(true, |s| {
            if s.selection.collapsed() {
                or(s);
            } else if s.is_ltr() {
                s.set_cursor(s.selection.max());
            } else {
                s.set_cursor(s.selection.min());
            }
        })
    }

    /// Where the code point (or the emoji it ends) before the selection's end starts (Compose's
    /// `getPrecedingCodePointOrEmojiStartIndex`): none at the start.
    pub fn get_preceding_code_point_or_emoji_start_index(&self) -> Option<usize> {
        find_code_point_or_emoji_start_before(self.text(), self.selection.end)
    }

    /// The character break before the selection's end (Compose's `getPrecedingCharacterIndex`).
    pub fn get_preceding_character_index(&self) -> Option<usize> {
        find_preceding_break(self.text(), self.selection.end)
    }

    /// The character break after the selection's end; none at the end (Compose's `getNextCharacterIndex`).
    pub fn get_next_character_index(&self) -> Option<usize> {
        find_following_break(self.text(), self.selection.end)
    }

    fn move_cursor_prev(&mut self) {
        if let Some(prev) = self.get_preceding_character_index() {
            self.set_cursor(prev);
        }
    }

    fn move_cursor_next(&mut self) {
        if let Some(next) = self.get_next_character_index() {
            self.set_cursor(next);
        }
    }

    /// The text's start (Compose's `moveCursorToHome`).
    pub fn move_cursor_to_home(&mut self) -> &mut Self {
        self.apply(true, |s| s.set_cursor(0))
    }

    /// The text's end (Compose's `moveCursorToEnd`).
    pub fn move_cursor_to_end(&mut self) -> &mut Self {
        self.apply(true, |s| {
            let len = s.text().len();
            s.set_cursor(len);
        })
    }

    /// A word to the left (Compose's `moveCursorLeftByWord`).
    pub fn move_cursor_left_by_word(&mut self) -> &mut Self {
        self.apply(true, |s| if s.is_ltr() { s.move_cursor_prev_by_word() } else { s.move_cursor_next_by_word() })
    }

    /// A word to the right (Compose's `moveCursorRightByWord`).
    pub fn move_cursor_right_by_word(&mut self) -> &mut Self {
        self.apply(true, |s| if s.is_ltr() { s.move_cursor_next_by_word() } else { s.move_cursor_prev_by_word() })
    }

    /// Where the next word ends, past the selection's end (Compose's `getNextWordOffset`): none without a layout.
    pub fn get_next_word_offset(&self) -> Option<usize> {
        let layout = self.layout_result()?;
        Some(self.next_word_offset_for_layout(layout))
    }

    fn move_cursor_next_by_word(&mut self) {
        if let Some(offset) = self.get_next_word_offset() {
            self.set_cursor(offset);
        }
    }

    /// Where the previous word starts, before the selection's end (Compose's `getPreviousWordOffset`).
    pub fn get_previous_word_offset(&self) -> Option<usize> {
        let layout = self.layout_result()?;
        Some(self.prev_word_offset(layout))
    }

    fn move_cursor_prev_by_word(&mut self) {
        if let Some(offset) = self.get_previous_word_offset() {
            self.set_cursor(offset);
        }
    }

    /// The paragraph's start, or the previous one's when already there (Compose's `moveCursorPrevByParagraph`).
    pub fn move_cursor_prev_by_paragraph(&mut self) -> &mut Self {
        self.apply(true, |s| {
            let min = s.selection.min();
            let mut paragraph_start = find_paragraph_start(s.text(), min);
            if paragraph_start == min && paragraph_start != 0 {
                paragraph_start = find_paragraph_start(s.text(), paragraph_start - 1);
            }
            s.set_cursor(paragraph_start);
        })
    }

    /// The paragraph's end, or the next one's when already there (Compose's `moveCursorNextByParagraph`).
    pub fn move_cursor_next_by_paragraph(&mut self) -> &mut Self {
        self.apply(true, |s| {
            let max = s.selection.max();
            let mut paragraph_end = find_paragraph_end(s.text(), max);
            if paragraph_end == max && paragraph_end != s.text().len() {
                paragraph_end = find_paragraph_end(s.text(), paragraph_end + 1);
            }
            s.set_cursor(paragraph_end);
        })
    }

    /// A line up, at the kept x (Compose's `moveCursorUpByLine`).
    pub fn move_cursor_up_by_line(&mut self) -> &mut Self {
        self.apply(false, |s| {
            if let Some(offset) = s.layout_result().cloned().map(|l| s.jump_by_lines_offset(&l, -1)) {
                s.set_cursor(offset);
            }
        })
    }

    /// A line down, at the kept x (Compose's `moveCursorDownByLine`).
    pub fn move_cursor_down_by_line(&mut self) -> &mut Self {
        self.apply(false, |s| {
            if let Some(offset) = s.layout_result().cloned().map(|l| s.jump_by_lines_offset(&l, 1)) {
                s.set_cursor(offset);
            }
        })
    }

    /// The start of the selection start's line (Compose's `getLineStartByOffset`).
    pub fn get_line_start_by_offset(&self) -> Option<usize> {
        let layout = self.layout_result()?;
        let line = layout.get_line_for_offset(self.transformed_min_offset());
        Some(self.offset_mapping.transformed_to_original(layout.get_line_start(line)))
    }

    /// Its line's start (Compose's `moveCursorToLineStart`).
    pub fn move_cursor_to_line_start(&mut self) -> &mut Self {
        self.apply(true, |s| {
            if let Some(offset) = s.get_line_start_by_offset() {
                s.set_cursor(offset);
            }
        })
    }

    /// The visible end of the selection end's line (Compose's `getLineEndByOffset`).
    pub fn get_line_end_by_offset(&self) -> Option<usize> {
        let layout = self.layout_result()?;
        let line = layout.get_line_for_offset(self.transformed_max_offset());
        Some(self.offset_mapping.transformed_to_original(layout.get_line_end(line, true)))
    }

    /// Its line's end (Compose's `moveCursorToLineEnd`).
    pub fn move_cursor_to_line_end(&mut self) -> &mut Self {
        self.apply(true, |s| {
            if let Some(offset) = s.get_line_end_by_offset() {
                s.set_cursor(offset);
            }
        })
    }

    /// Its line's left side (Compose's `moveCursorToLineLeftSide`).
    pub fn move_cursor_to_line_left_side(&mut self) -> &mut Self {
        self.apply(true, |s| {
            if s.is_ltr() {
                s.move_cursor_to_line_start();
            } else {
                s.move_cursor_to_line_end();
            }
        })
    }

    /// Its line's right side (Compose's `moveCursorToLineRightSide`).
    pub fn move_cursor_to_line_right_side(&mut self) -> &mut Self {
        self.apply(true, |s| {
            if s.is_ltr() {
                s.move_cursor_to_line_end();
            } else {
                s.move_cursor_to_line_start();
            }
        })
    }

    /// The movement made a selection from the original start to where the end went (Compose's `selectMovement`).
    pub fn select_movement(&mut self) -> &mut Self {
        self.apply(false, |s| s.selection = TextRange::new(s.original_selection.start, s.selection.end))
    }

    /// The selection deleted, else what `or` deletes (Compose's `deleteIfSelectedOr`): the commands, none for
    /// nothing to delete.
    pub fn delete_if_selected_or(
        &mut self,
        or: impl FnOnce(&mut Self) -> Option<EditCommand>,
    ) -> Option<Vec<EditCommand>> {
        if self.selection.collapsed() {
            or(self).map(|c| vec![c])
        } else {
            let min = self.selection.min();
            Some(vec![
                EditCommand::CommitText { text: String::new(), new_cursor_position: 0 },
                EditCommand::SetSelection { start: min, end: min },
            ])
        }
    }

    /// A page up: the visible height of the field (Compose's `moveCursorUpByPage`).
    pub fn move_cursor_up_by_page(&mut self) -> &mut Self {
        self.apply(false, |s| {
            if let Some(offset) = s.jump_by_pages_offset(-1) {
                s.set_cursor(offset);
            }
        })
    }

    /// A page down (Compose's `moveCursorDownByPage`).
    pub fn move_cursor_down_by_page(&mut self) -> &mut Self {
        self.apply(false, |s| {
            if let Some(offset) = s.jump_by_pages_offset(1) {
                s.set_cursor(offset);
            }
        })
    }

    /// Where a jump by `pages_amount` of the field's visible height lands (Compose's `jumpByPagesOffset`).
    fn jump_by_pages_offset(&self, pages_amount: i32) -> Option<usize> {
        let proxy = self.layout_result_proxy.as_ref()?;
        let visible = match (proxy.inner_text_field_coordinates(), proxy.decoration_box_coordinates()) {
            (Some(inner), Some(decoration)) => decoration.local_bounding_box_of(&inner, true),
            _ => Rect::Zero,
        };
        let current_offset = self.offset_mapping.original_to_transformed(self.current_value.selection().end);
        let current_pos = proxy.value.get_cursor_rect(current_offset);
        let x = current_pos.left;
        let y = current_pos.top + visible.height() * pages_amount as f32;
        Some(self.offset_mapping.transformed_to_original(proxy.value.get_offset_for_position(Offset::new(x, y))))
    }

    fn is_ltr(&self) -> bool {
        let direction = self.layout_result().map(|l| l.get_paragraph_direction(self.transformed_end_offset()));
        direction != Some(ResolvedTextDirection::Rtl)
    }

    /// The last character's start of the original text at most (Compose's `charOffset`: length - 1 at most).
    fn char_offset(&self, offset: usize) -> usize {
        let text = self.text();
        let last = text.char_indices().next_back().map_or(0, |(i, _)| i);
        offset.min(last)
    }

    fn next_word_offset_for_layout(&self, layout: &TextLayoutResult) -> usize {
        let original_len = self.text().len();
        let shown = layout.layout_input.text.text();
        let mut current = self.transformed_end_offset();
        loop {
            if current >= original_len {
                return original_len;
            }
            let word = layout.get_word_boundary(self.char_offset(current));
            if word.end <= current {
                current = next_boundary(shown, current);
                continue;
            }
            return self.offset_mapping.transformed_to_original(word.end);
        }
    }

    fn prev_word_offset(&self, layout: &TextLayoutResult) -> usize {
        let shown = layout.layout_input.text.text();
        let mut current = self.transformed_end_offset();
        loop {
            if current == 0 {
                return 0;
            }
            let word = layout.get_word_boundary(self.char_offset(current));
            if word.start >= current {
                current = previous_boundary(shown, current);
                continue;
            }
            return self.offset_mapping.transformed_to_original(word.start);
        }
    }

    /// Where a jump by `lines_amount` lines lands, at the kept x (Compose's `jumpByLinesOffset`): the text's start
    /// above its first line, its end below its last; a line's visible end when the x is past it.
    fn jump_by_lines_offset(&mut self, layout: &TextLayoutResult, lines_amount: i32) -> usize {
        let current_offset = self.transformed_end_offset();
        if self.state.cached_x.is_none() {
            self.state.cached_x = Some(layout.get_cursor_rect(current_offset).left);
        }
        let target_line = layout.get_line_for_offset(current_offset) as i64 + lines_amount as i64;
        if target_line < 0 {
            return 0;
        }
        if target_line >= layout.line_count() as i64 {
            return self.text().len();
        }
        let target_line = target_line as usize;
        let y = layout.get_line_bottom(target_line) - 1.0;
        let x = self.state.cached_x.expect("a kept x");
        let ltr = self.is_ltr();
        if (ltr && x >= layout.get_line_right(target_line)) || (!ltr && x <= layout.get_line_left(target_line)) {
            return layout.get_line_end(target_line, true);
        }
        self.offset_mapping.transformed_to_original(layout.get_offset_for_position(Offset::new(x, y)))
    }

    fn transformed_end_offset(&self) -> usize {
        self.offset_mapping.original_to_transformed(self.selection.end)
    }

    fn transformed_min_offset(&self) -> usize {
        self.offset_mapping.original_to_transformed(self.selection.min())
    }

    fn transformed_max_offset(&self) -> usize {
        self.offset_mapping.original_to_transformed(self.selection.max())
    }
}

/// The next character boundary of `text` after `offset` (past its end: one more).
fn next_boundary(text: &str, offset: usize) -> usize {
    match text.get(offset..).and_then(|rest| rest.chars().next()) {
        Some(c) => offset + c.len_utf8(),
        None => offset + 1,
    }
}

/// The character boundary of `text` before `offset`.
fn previous_boundary(text: &str, offset: usize) -> usize {
    let mut at = offset.min(text.len()).saturating_sub(1);
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}
