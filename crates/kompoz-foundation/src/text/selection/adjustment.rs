//! How a raw selection snaps (Compose's SelectionAdjustment.kt): to characters, words (ICU's word boundaries),
//! paragraphs, or words while growing and characters while shrinking.

use super::layout::{CrossStatus, SelectableInfo, SelectionLayout, is_collapsed};
use super::selection::{AnchorInfo, Selection};
use kompoz_text::TextRange;
use kompoz_text::segment::{find_following_break, find_preceding_break, paragraph_boundary};

/// How the raw selection of a gesture is adjusted (Compose's SelectionAdjustment): a long press dragged on selects
/// by word, a handle dragged by character (words while growing), a mouse's double click by word and its triple
/// click by paragraph.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SelectionAdjustment {
    /// The raw selection as it is.
    None,
    /// The raw selection, at least a character (a grapheme cluster) once it started.
    Character,
    /// Out to the word boundaries around each end (ICU's), crossed if the raw one was.
    Word,
    /// Out to the paragraphs (line feed to line feed) around each end.
    Paragraph,
    /// By word while an end moves out from a word boundary or to another line or selectable, by character while
    /// it shrinks or moves within a word (Compose's CharacterWithWordAccelerate).
    CharacterWithWordAccelerate,
}

impl SelectionAdjustment {
    /// The selection `layout`'s raw one adjusts to (Compose's `adjust`).
    pub fn adjust(self, layout: &SelectionLayout) -> Selection {
        match self {
            SelectionAdjustment::None => Selection {
                start: layout.start_info().anchor_for_offset(layout.start_info().raw_start_handle_offset),
                end: layout.end_info().anchor_for_offset(layout.end_info().raw_end_handle_offset),
                handles_crossed: layout.cross_status() == CrossStatus::Crossed,
            },
            SelectionAdjustment::Character => {
                ensure_at_least_one_char(SelectionAdjustment::None.adjust(layout), layout)
            }
            SelectionAdjustment::Word => {
                adjust_to_boundaries(layout, |info, offset| info.text_layout_result.get_word_boundary(offset))
            }
            SelectionAdjustment::Paragraph => {
                adjust_to_boundaries(layout, |info, offset| paragraph_boundary(info.input_text(), offset))
            }
            SelectionAdjustment::CharacterWithWordAccelerate => character_with_word_accelerate(layout),
        }
    }
}

fn character_with_word_accelerate(layout: &SelectionLayout) -> Selection {
    let Some(previous_selection) = layout.previous_selection().copied() else {
        return SelectionAdjustment::Word.adjust(layout);
    };
    let (previous_anchor, new_anchor, start_anchor, end_anchor);
    if layout.is_start_handle() {
        previous_anchor = previous_selection.start;
        new_anchor = update_selection_boundary(layout, layout.start_info(), previous_anchor);
        start_anchor = new_anchor;
        end_anchor = previous_selection.end;
    } else {
        previous_anchor = previous_selection.end;
        new_anchor = update_selection_boundary(layout, layout.end_info(), previous_anchor);
        start_anchor = previous_selection.start;
        end_anchor = new_anchor;
    }
    if new_anchor == previous_anchor {
        // In bidi text a move of one character may move the offset a long way (crossing from left to right to
        // right to left), so the layout may say crossed where the selection isn't: the old one stays.
        previous_selection
    } else {
        let crossed = layout.cross_status() == CrossStatus::Crossed
            || (layout.cross_status() == CrossStatus::Collapsed && start_anchor.offset > end_anchor.offset);
        ensure_at_least_one_char(Selection { start: start_anchor, end: end_anchor, handles_crossed: crossed }, layout)
    }
}

/// Where the moving end goes (Compose's `updateSelectionBoundary`); `layout` has a previous selection.
fn update_selection_boundary(
    layout: &SelectionLayout,
    info: &SelectableInfo,
    previous_selection_anchor: AnchorInfo,
) -> AnchorInfo {
    let is_start_handle = layout.is_start_handle();
    let current_raw_offset = if is_start_handle { info.raw_start_handle_offset } else { info.raw_end_handle_offset };
    let current_slot = if is_start_handle { layout.start_slot() } else { layout.end_slot() };
    if current_slot != info.slot {
        // between selectables
        return info.anchor_for_offset(current_raw_offset);
    }
    let layout_result = &info.text_layout_result;
    let current_raw_line = layout_result.get_line_for_offset(current_raw_offset);
    let other_raw_offset = if is_start_handle { info.raw_end_handle_offset } else { info.raw_start_handle_offset };
    let anchor_snapped_to_word_boundary = || {
        snap_to_word_boundary(
            info,
            current_raw_line,
            current_raw_offset,
            other_raw_offset,
            is_start_handle,
            layout.cross_status() == CrossStatus::Crossed,
        )
    };
    if info.selectable_id != previous_selection_anchor.selectable_id {
        // moved to another selectable: by word
        return anchor_snapped_to_word_boundary();
    }
    if info.raw_previous_handle_offset == Some(current_raw_offset) {
        // the end didn't move
        return previous_selection_anchor;
    }
    // (no previous position: Compose's -1, on the first line)
    let previous_raw_line = info.raw_previous_handle_offset.map_or(0, |o| layout_result.get_line_for_offset(o));
    // The raw lines: the previous adjusted offset may stay on another line once snapped to a word boundary, which
    // would always look like a change of line and never let it stop snapping.
    if current_raw_line != previous_raw_line {
        return anchor_snapped_to_word_boundary();
    }
    if !is_expanding(info, current_raw_offset, is_start_handle) {
        // shrinking: the raw offset
        return info.anchor_for_offset(current_raw_offset);
    }
    expanding_anchor(info, previous_selection_anchor, current_raw_offset, anchor_snapped_to_word_boundary)
}

/// An end moving out: by word if it was on a word boundary, else (within a word) by character.
fn expanding_anchor(
    info: &SelectableInfo,
    previous_selection_anchor: AnchorInfo,
    current_raw_offset: usize,
    anchor_snapped_to_word_boundary: impl FnOnce() -> AnchorInfo,
) -> AnchorInfo {
    let previous_selection_offset = previous_selection_anchor.offset;
    let previous_selection_word_boundary = info.text_layout_result.get_word_boundary(previous_selection_offset);
    if previous_selection_offset == previous_selection_word_boundary.start
        || previous_selection_offset == previous_selection_word_boundary.end
    {
        anchor_snapped_to_word_boundary()
    } else {
        info.anchor_for_offset(current_raw_offset)
    }
}

/// Whether the moving end goes out (Compose's `isExpanding`).
fn is_expanding(info: &SelectableInfo, current_raw_offset: usize, is_start: bool) -> bool {
    let Some(previous) = info.raw_previous_handle_offset else { return true };
    if current_raw_offset == previous {
        return false;
    }
    let crossed = info.raw_cross_status() == CrossStatus::Crossed;
    if is_start ^ crossed { current_raw_offset < previous } else { current_raw_offset > previous }
}

/// The moving end snapped to the boundary of the word at it on its line (Compose's `snapToWordBoundary`): a word
/// running onto another line (too long, hyphenated) is cut at the line's edge, and a boundary the other end is at
/// isn't taken (the selection would be empty).
fn snap_to_word_boundary(
    info: &SelectableInfo,
    current_line: usize,
    current_offset: usize,
    other_offset: usize,
    is_start: bool,
    crossed: bool,
) -> AnchorInfo {
    let layout = &info.text_layout_result;
    let word_boundary = layout.get_word_boundary(current_offset);
    let line_count = layout.line_count();
    let start = if layout.get_line_for_offset(word_boundary.start) == current_line {
        word_boundary.start
    } else if current_line >= line_count {
        // The line isn't even visible: the last visible line's start.
        layout.get_line_start(line_count - 1)
    } else {
        layout.get_line_start(current_line)
    };
    let end = if layout.get_line_for_offset(word_boundary.end) == current_line {
        word_boundary.end
    } else if current_line >= line_count {
        layout.get_line_end(line_count - 1, false)
    } else {
        layout.get_line_end(current_line, false)
    };
    if start == other_offset {
        return info.anchor_for_offset(end);
    }
    if end == other_offset {
        return info.anchor_for_offset(start);
    }
    let result_offset = if is_start ^ crossed {
        // the start moving uncrossed, or the end crossed
        if current_offset <= end { start } else { end }
    } else if current_offset >= start {
        end
    } else {
        start
    };
    info.anchor_for_offset(result_offset)
}

/// Each end out to the boundary `boundary` gives around it (Compose's `adjustToBoundaries`).
fn adjust_to_boundaries(layout: &SelectionLayout, boundary: impl Fn(&SelectableInfo, usize) -> TextRange) -> Selection {
    let crossed = layout.cross_status() == CrossStatus::Crossed;
    Selection {
        start: anchor_on_boundary(layout.start_info(), crossed, true, layout.start_slot(), &boundary),
        end: anchor_on_boundary(layout.end_info(), crossed, false, layout.end_slot(), &boundary),
        handles_crossed: crossed,
    }
}

fn anchor_on_boundary(
    info: &SelectableInfo,
    crossed: bool,
    is_start: bool,
    slot: i32,
    boundary: &impl Fn(&SelectableInfo, usize) -> TextRange,
) -> AnchorInfo {
    let offset = if is_start { info.raw_start_handle_offset } else { info.raw_end_handle_offset };
    if slot != info.slot {
        return info.anchor_for_offset(offset);
    }
    let range = boundary(info, offset);
    info.anchor_for_offset(if is_start ^ crossed { range.start } else { range.end })
}

/// At least one character (grapheme cluster) selected once a selection started (Compose's
/// `ensureAtLeastOneChar`): a collapsed selection takes the one next to it — unless it spans several selectables
/// (no whitespace to take between them), is a new selection, or there is no text.
pub fn ensure_at_least_one_char(selection: Selection, layout: &SelectionLayout) -> Selection {
    if !is_collapsed(Some(&selection), Some(layout)) {
        return selection;
    }
    let text = layout.current_info().input_text();
    if layout.size() > 1 || layout.previous_selection().is_none() || text.is_empty() {
        return selection;
    }
    expand_one_char(selection, layout)
}

/// A collapsed selection widened by a character (Compose's `expandOneChar`).
fn expand_one_char(selection: Selection, layout: &SelectionLayout) -> Selection {
    let info = layout.current_info();
    let text = info.input_text();
    let offset = info.raw_start_handle_offset; // (start and end are the same)
    let change = |anchor: AnchorInfo, new_offset: usize| AnchorInfo {
        offset: new_offset,
        direction: info.text_layout_result.get_bidi_run_direction(new_offset),
        ..anchor
    };
    let mut s = selection;
    if offset == 0 {
        // At the text's start: the moving end one character in.
        let following = find_following_break(text, 0).unwrap_or(text.len());
        if layout.is_start_handle() {
            s.start = change(s.start, following);
            s.handles_crossed = true;
        } else {
            s.end = change(s.end, following);
            s.handles_crossed = false;
        }
    } else if offset == text.len() {
        let preceding = find_preceding_break(text, text.len()).unwrap_or(0);
        if layout.is_start_handle() {
            s.start = change(s.start, preceding);
            s.handles_crossed = false;
        } else {
            s.end = change(s.end, preceding);
            s.handles_crossed = true;
        }
    } else {
        // Inside: the handles stay crossed or not as they were.
        let crossed = layout.previous_selection().is_some_and(|p| p.handles_crossed);
        let new_offset = if layout.is_start_handle() ^ crossed {
            find_preceding_break(text, offset).unwrap_or(0)
        } else {
            find_following_break(text, offset).unwrap_or(text.len())
        };
        if layout.is_start_handle() {
            s.start = change(s.start, new_offset);
        } else {
            s.end = change(s.end, new_offset);
        }
        s.handles_crossed = crossed;
    }
    s
}
