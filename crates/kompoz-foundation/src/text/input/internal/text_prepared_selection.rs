//! A state-based field's caret and selection moves and deletions by characters, words, paragraphs, lines and pages
//! (Compose's SelectionMovementDeletionContext and TextFieldPreparedSelectionState), what its key commands do: as
//! the text is shown (visual lines, bidi directions, transformations kept whole), the selection applied at the end.

use super::transformed_text_field_state::{
    IndexTransformationType, SelectionWedgeAffinity, TransformedTextFieldState, WedgeAffinity,
};
use super::undo::TextFieldEditUndoBehavior;
use crate::text::input::TextFieldCharSequence;
use kompoz_graphics::Offset;
use kompoz_text::segment::{
    find_code_point_or_emoji_start_before, find_following_break, find_paragraph_end, find_paragraph_start,
    find_preceding_break,
};
use kompoz_text::{ResolvedTextDirection, TextLayoutResult, TextRange};

/// What vertical moves keep between key presses (Compose's TextFieldPreparedSelectionState): the caret's x when
/// they began.
#[derive(Clone, Copy, Debug)]
pub struct TextFieldPreparedSelectionState {
    pub cached_x: f32,
}

impl Default for TextFieldPreparedSelectionState {
    fn default() -> Self {
        TextFieldPreparedSelectionState { cached_x: f32::NAN }
    }
}

impl TextFieldPreparedSelectionState {
    /// Forgets the x vertical moves keep.
    pub fn reset_cached_x(&mut self) {
        self.cached_x = f32::NAN;
    }
}

/// A caret position and the wedge affinity it moved with (Compose's CursorAndWedgeAffinity).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CursorAndWedgeAffinity {
    pub cursor: usize,
    pub wedge_affinity: Option<WedgeAffinity>,
}

/// Where a caret moving from `cursor` to `proposed_cursor` (none: no character there) of the shown text goes,
/// keeping out of transformations' insides (Compose's `calculateNextCursorPositionAndWedgeAffinity`): across a
/// replacement to its far side, into an insertion's near side, its wedge affinity for the way it went.
pub fn calculate_next_cursor_position_and_wedge_affinity(
    proposed_cursor: Option<usize>,
    cursor: usize,
    state: &TransformedTextFieldState,
) -> CursorAndWedgeAffinity {
    let Some(proposed) = proposed_cursor else { return CursorAndWedgeAffinity { cursor, wedge_affinity: None } };
    let forward = proposed > cursor;
    let (kind, _untransformed, retransformed) = state.get_index_transformation_type(proposed);
    let at = |cursor, wedge_affinity| CursorAndWedgeAffinity { cursor, wedge_affinity };
    match kind {
        IndexTransformationType::Untransformed => {
            at(proposed, Some(if forward { WedgeAffinity::Start } else { WedgeAffinity::End }))
        }
        IndexTransformationType::Deletion => at(proposed, None),
        IndexTransformationType::Replacement => {
            if forward {
                at(retransformed.end, Some(WedgeAffinity::Start))
            } else {
                at(retransformed.start, Some(WedgeAffinity::End))
            }
        }
        IndexTransformationType::Insertion => {
            if forward {
                if proposed == retransformed.start {
                    at(proposed, Some(WedgeAffinity::Start))
                } else {
                    at(retransformed.end, Some(WedgeAffinity::End))
                }
            } else if proposed == retransformed.end {
                at(proposed, Some(WedgeAffinity::End))
            } else {
                at(retransformed.start, Some(WedgeAffinity::Start))
            }
        }
    }
}

/// The selection moves and deletions of one key command (Compose's SelectionMovementDeletionContext): it starts
/// from the shown text's value (read without observing), moves its own [`selection`](Self::selection), and its
/// deletions edit the state; the caller applies the selection at the end.
pub struct SelectionMovementDeletionContext<'a> {
    state: &'a TransformedTextFieldState,
    text_layout_result: Option<TextLayoutResult>,
    is_from_soft_keyboard: bool,
    is_from_hardware_source: bool,
    visible_text_layout_height: f32,
    prepared: &'a mut TextFieldPreparedSelectionState,
    /// The shown text when it began.
    pub initial_value: TextFieldCharSequence,
    pub initial_wedge_affinity: SelectionWedgeAffinity,
    /// The selection as the moves leave it.
    pub selection: TextRange,
    /// The wedge affinity the last move left, if it set one.
    pub wedge_affinity: Option<WedgeAffinity>,
    text: String,
}

impl<'a> SelectionMovementDeletionContext<'a> {
    pub fn new(
        state: &'a TransformedTextFieldState,
        text_layout_result: Option<TextLayoutResult>,
        is_from_soft_keyboard: bool,
        is_from_hardware_source: bool,
        visible_text_layout_height: f32,
        prepared: &'a mut TextFieldPreparedSelectionState,
    ) -> SelectionMovementDeletionContext<'a> {
        let (initial_value, initial_wedge_affinity) =
            kompoz_runtime::without_read_observation(|| (state.visual_text(), state.selection_wedge_affinity()));
        let text = initial_value.text().to_string();
        SelectionMovementDeletionContext {
            state,
            text_layout_result,
            is_from_soft_keyboard,
            is_from_hardware_source,
            visible_text_layout_height,
            prepared,
            selection: initial_value.selection(),
            initial_wedge_affinity,
            initial_value,
            wedge_affinity: None,
            text,
        }
    }

    fn apply_if_not_empty(&mut self, reset_cached_x: bool, block: impl FnOnce(&mut Self)) -> &mut Self {
        if reset_cached_x {
            self.prepared.reset_cached_x();
        }
        if !self.text.is_empty() {
            block(self);
        }
        self
    }

    /// Selects all the text.
    pub fn select_all(&mut self) -> &mut Self {
        self.apply_if_not_empty(true, |c| c.selection = TextRange::new(0, c.text.len()))
    }

    /// The selection collapsed to its end.
    pub fn deselect(&mut self) -> &mut Self {
        self.apply_if_not_empty(true, |c| c.selection = TextRange::at(c.selection.end))
    }

    pub fn move_cursor_left_by_char(&mut self) -> &mut Self {
        if self.is_ltr() { self.move_cursor_prev_by_char() } else { self.move_cursor_next_by_char() }
    }

    pub fn move_cursor_right_by_char(&mut self) -> &mut Self {
        if self.is_ltr() { self.move_cursor_next_by_char() } else { self.move_cursor_prev_by_char() }
    }

    /// A selection collapsed to its left side, else `or` (Compose's `collapseLeftOr`).
    pub fn collapse_left_or(&mut self, or: impl FnOnce(&mut Self)) -> &mut Self {
        self.apply_if_not_empty(true, |c| {
            if c.selection.collapsed() {
                or(c);
            } else if c.is_ltr() {
                c.selection = TextRange::at(c.selection.min());
            } else {
                c.selection = TextRange::at(c.selection.max());
            }
        })
    }

    /// A selection collapsed to its right side, else `or` (Compose's `collapseRightOr`).
    pub fn collapse_right_or(&mut self, or: impl FnOnce(&mut Self)) -> &mut Self {
        self.apply_if_not_empty(true, |c| {
            if c.selection.collapsed() {
                or(c);
            } else if c.is_ltr() {
                c.selection = TextRange::at(c.selection.max());
            } else {
                c.selection = TextRange::at(c.selection.min());
            }
        })
    }

    /// The character break before the selection's end (Compose's `getPrecedingCharacterIndex`).
    pub fn get_preceding_character_index(&self) -> Option<usize> {
        find_preceding_break(&self.text, self.selection.end)
    }

    /// The character break after the selection's end (Compose's `getNextCharacterIndex`).
    pub fn get_next_character_index(&self) -> Option<usize> {
        find_following_break(&self.text, self.selection.end)
    }

    fn move_cursor_to(&mut self, reset_cached_x: bool, proposed: impl FnOnce(&Self) -> Option<usize>) -> &mut Self {
        self.apply_if_not_empty(reset_cached_x, |c| {
            let old_cursor = c.selection.end;
            let next = calculate_next_cursor_position_and_wedge_affinity(proposed(c), old_cursor, c.state);
            if next.cursor != old_cursor || !c.selection.collapsed() {
                c.selection = TextRange::at(next.cursor);
            }
            if let Some(w) = next.wedge_affinity {
                c.wedge_affinity = Some(w);
            }
        })
    }

    /// Back to the code point or emoji before the caret (what Backspace deletes).
    pub fn move_cursor_prev_by_code_point_or_emoji(&mut self) -> &mut Self {
        self.move_cursor_to(true, |c| find_code_point_or_emoji_start_before(&c.text, c.selection.end))
    }

    pub fn move_cursor_prev_by_char(&mut self) -> &mut Self {
        self.move_cursor_to(true, |c| find_preceding_break(&c.text, c.selection.end))
    }

    pub fn move_cursor_next_by_char(&mut self) -> &mut Self {
        self.move_cursor_to(true, |c| find_following_break(&c.text, c.selection.end))
    }

    pub fn move_cursor_to_home(&mut self) -> &mut Self {
        self.move_cursor_to(true, |_| Some(0))
    }

    pub fn move_cursor_to_end(&mut self) -> &mut Self {
        self.move_cursor_to(true, |c| Some(c.text.len()))
    }

    pub fn move_cursor_left_by_word(&mut self) -> &mut Self {
        if self.is_ltr() { self.move_cursor_prev_by_word() } else { self.move_cursor_next_by_word() }
    }

    pub fn move_cursor_right_by_word(&mut self) -> &mut Self {
        if self.is_ltr() { self.move_cursor_next_by_word() } else { self.move_cursor_prev_by_word() }
    }

    /// To the end of the word at or after the caret (by the layout's word boundaries; the text's end without a
    /// layout).
    pub fn move_cursor_next_by_word(&mut self) -> &mut Self {
        self.move_cursor_to(true, |c| {
            Some(match &c.text_layout_result {
                Some(layout) => c.next_word_offset(layout, c.selection.end),
                None => c.text.len(),
            })
        })
    }

    /// To the start of the word at or before the caret (0 without a layout).
    pub fn move_cursor_prev_by_word(&mut self) -> &mut Self {
        self.move_cursor_to(true, |c| {
            Some(match &c.text_layout_result {
                Some(layout) => c.prev_word_offset(layout, c.selection.end),
                None => 0,
            })
        })
    }

    /// To the start of the paragraph, or of the one before if it is there already.
    pub fn move_cursor_prev_by_paragraph(&mut self) -> &mut Self {
        self.move_cursor_to(true, |c| {
            let min = c.selection.min();
            let mut start = find_paragraph_start(&c.text, min);
            if start == min && start != 0 {
                start = find_paragraph_start(&c.text, start - 1);
            }
            Some(start)
        })
    }

    /// To the end of the paragraph, or of the one after if it is there already.
    pub fn move_cursor_next_by_paragraph(&mut self) -> &mut Self {
        self.move_cursor_to(true, |c| {
            let max = c.selection.max();
            let mut end = find_paragraph_end(&c.text, max);
            if end == max && end != c.text.len() {
                end = find_paragraph_end(&c.text, end + 1);
            }
            Some(end)
        })
    }

    /// A line up, at the x vertical moves keep (to the text's start from the first line).
    pub fn move_cursor_up_by_line(&mut self) -> &mut Self {
        let target = self.text_layout_result.clone().map_or(i64::MIN, |l| self.jump_by_lines_offset(&l, -1));
        self.move_cursor_to(target == i64::MIN, move |_| Some(target.max(0) as usize))
    }

    /// A line down (to the text's end from the last line).
    pub fn move_cursor_down_by_line(&mut self) -> &mut Self {
        let target = self.text_layout_result.clone().map_or(i64::MAX, |l| self.jump_by_lines_offset(&l, 1));
        let len = self.text.len() as i64;
        self.move_cursor_to(target == i64::MAX, move |_| Some(target.min(len) as usize))
    }

    pub fn move_cursor_to_line_left_side(&mut self) -> &mut Self {
        if self.is_ltr() { self.move_cursor_to_line_start() } else { self.move_cursor_to_line_end() }
    }

    pub fn move_cursor_to_line_right_side(&mut self) -> &mut Self {
        if self.is_ltr() { self.move_cursor_to_line_end() } else { self.move_cursor_to_line_start() }
    }

    /// To the visual line's start (the text's start without a layout).
    pub fn move_cursor_to_line_start(&mut self) -> &mut Self {
        self.move_cursor_to(true, |c| {
            Some(
                c.text_layout_result.as_ref().map_or(0, |l| l.get_line_start(l.get_line_for_offset(c.selection.min()))),
            )
        })
    }

    /// To the visual line's end (the text's end without a layout).
    pub fn move_cursor_to_line_end(&mut self) -> &mut Self {
        self.move_cursor_to(true, |c| {
            Some(
                c.text_layout_result
                    .as_ref()
                    .map_or(c.text.len(), |l| l.get_line_end(l.get_line_for_offset(c.selection.max()), true)),
            )
        })
    }

    /// A page (the visible height) up (Page Up).
    pub fn move_cursor_up_by_page(&mut self) -> &mut Self {
        self.move_cursor_to(false, |c| Some(c.jump_by_pages_offset(-1)))
    }

    /// A page down (Page Down).
    pub fn move_cursor_down_by_page(&mut self) -> &mut Self {
        self.move_cursor_to(false, |c| Some(c.jump_by_pages_offset(1)))
    }

    /// The selection from where it started to where the moves left its end (what Shift does).
    pub fn select_movement(&mut self) -> &mut Self {
        self.apply_if_not_empty(false, |c| {
            c.selection = TextRange::new(c.initial_value.selection().start, c.selection.end)
        })
    }

    /// Deletes what the moves went over (or the selection that was there), in the state.
    pub fn delete_movement(&mut self) -> &mut Self {
        self.apply_if_not_empty(false, |c| {
            if !c.initial_value.selection().collapsed() {
                c.state.delete_selected_text(c.is_from_hardware_source);
            } else {
                c.state.replace_text(
                    "",
                    TextRange::new(c.initial_value.selection().start, c.selection.end),
                    TextFieldEditUndoBehavior::MergeIfPossible,
                    !c.is_from_soft_keyboard,
                    c.is_from_hardware_source,
                );
            }
            c.selection = kompoz_runtime::without_read_observation(|| c.state.visual_text().selection());
            c.wedge_affinity = Some(WedgeAffinity::Start);
        })
    }

    fn is_ltr(&self) -> bool {
        self.text_layout_result
            .as_ref()
            .is_none_or(|l| l.get_paragraph_direction(self.selection.end) == ResolvedTextDirection::Ltr)
    }

    fn char_offset(&self, offset: usize) -> usize {
        // (Compose's `offset.coerceAtMost(text.length - 1)`, on a character boundary.)
        let last = self.text.char_indices().next_back().map_or(0, |(i, _)| i);
        offset.min(last)
    }

    fn next_word_offset(&self, layout: &TextLayoutResult, mut current: usize) -> usize {
        loop {
            if current >= self.initial_value.len() {
                return self.initial_value.len();
            }
            let word = layout.get_word_boundary(self.char_offset(current));
            if word.end <= current {
                current = find_following_break_or_next(&self.text, current);
            } else {
                return word.end;
            }
        }
    }

    fn prev_word_offset(&self, layout: &TextLayoutResult, mut current: usize) -> usize {
        loop {
            if current == 0 {
                return 0;
            }
            let word = layout.get_word_boundary(self.char_offset(current));
            if word.start >= current {
                current = previous_char_boundary(&self.text, current);
            } else {
                return word.start;
            }
        }
    }

    /// The offset `lines_amount` lines away at the x vertical moves keep (Compose's `jumpByLinesOffset`):
    /// `i64::MIN` above the first line, `i64::MAX` below the last.
    fn jump_by_lines_offset(&mut self, layout: &TextLayoutResult, lines_amount: i64) -> i64 {
        let current = self.selection.end;
        if self.prepared.cached_x.is_nan() {
            self.prepared.cached_x = layout.get_cursor_rect(current).left;
        }
        let target = layout.get_line_for_offset(current) as i64 + lines_amount;
        if target < 0 {
            return i64::MIN;
        }
        if target >= layout.line_count() as i64 {
            return i64::MAX;
        }
        let target = target as usize;
        let y = layout.get_line_bottom(target) - 1.0;
        let x = self.prepared.cached_x;
        let ltr = self.is_ltr();
        if (ltr && x >= layout.get_line_right(target)) || (!ltr && x <= layout.get_line_left(target)) {
            return layout.get_line_end(target, true) as i64;
        }
        layout.get_offset_for_position(Offset::new(x, y)) as i64
    }

    /// The offset a page (the visible height) away (Compose's `jumpByPagesOffset`): the caret's if not laid out.
    fn jump_by_pages_offset(&self, pages_amount: i32) -> usize {
        let current = self.initial_value.selection().end;
        let Some(layout) = &self.text_layout_result else { return current };
        if self.visible_text_layout_height.is_nan() {
            return current;
        }
        let pos = layout.get_cursor_rect(current);
        let new_pos = pos.translate(Offset::new(0.0, self.visible_text_layout_height * pages_amount as f32));
        let top_line = layout.get_line_for_vertical_position(new_pos.top);
        let separator = layout.get_line_bottom(top_line);
        if (new_pos.top - separator).abs() > (new_pos.bottom - separator).abs() {
            layout.get_offset_for_position(Offset::new(new_pos.left, new_pos.top))
        } else {
            layout.get_offset_for_position(Offset::new(new_pos.left, new_pos.bottom))
        }
    }
}

/// The next character boundary after `at` (Compose's `+ 1` on UTF-16 units, here on characters).
fn find_following_break_or_next(text: &str, at: usize) -> usize {
    text[at..].chars().next().map_or(text.len(), |c| at + c.len_utf8())
}

/// The character boundary before `at` (Compose's `- 1`).
fn previous_char_boundary(text: &str, at: usize) -> usize {
    text[..at].chars().next_back().map_or(0, |c| at - c.len_utf8())
}
