//! A text's layout as a platform reads it: where each character and each line of a text is, in the window. What
//! Compose's platforms work out of a [`TextLayoutResult`] themselves — Android's CursorAnchorInfoBuilder (each
//! character's bounds, the visible line bounds, the editor bounds), its accessibility delegate's character
//! locations (`getBoundingBoxes`), its handwriting gestures' hit tests (HandwritingGesture.android.kt) — made once
//! into plain data, so that a platform reads it on any thread (UI Automation's and the text services' callers,
//! AT-SPI's) and in its own offsets (UTF-16 units, code points: each converts at its boundary).
//!
//! The focused text field's comes to the platform while its input session is open
//! ([`PlatformCallbacks::update_text_layout_result`](crate::platform::PlatformCallbacks::update_text_layout_result),
//! [`TextInputService::session_text_layout`](crate::text_input::TextInputService::session_text_layout)), told again
//! as it is laid out again or moves; a text node's comes with the accessibility tree
//! ([`AccessibilityNode::text_layout`](crate::semantics::AccessibilityNode::text_layout), Compose's semantics
//! `GetTextLayoutResult`).
//!
//! Offsets are UTF-8 byte offsets of [`PlatformTextLayout::text`] (the field's value as typed for an input
//! session, where a visual transformation shows something else: the boxes are of what shows for it). Boxes are px
//! in the root (the host's window), axis aligned (a rotated field's are the bounds of its turned boxes).

use crate::layout::LayoutCoordinates;
use kompoz_graphics::{Matrix, Offset, Rect};
use kompoz_text::input::OffsetMapping;
use kompoz_text::{ResolvedTextDirection, TextLayoutResult, TextRange};

/// Where a character of the text is (Compose's character bounds, a glyph cluster's: each character of a cluster —
/// a letter and its marks, an emoji sequence — has the whole cluster's box).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CharacterBounds {
    /// Its bytes in the text.
    pub start: usize,
    pub end: usize,
    /// Its box, px in the window: from its leading to its trailing edge, as tall as its line. A character the
    /// visual transformation shows nothing for has a box no wide, where it would be.
    pub rect: Rect,
    /// It is in a right-to-left run (its leading edge is its right).
    pub rtl: bool,
    /// The line it is on (an index into [`PlatformTextLayout::lines`]).
    pub line: usize,
}

/// Where a line of the text is (Compose's visible line bounds: from the line's left to its right, its top to its
/// bottom).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LineBounds {
    /// Its bytes in the text (its end includes the line break that ends it, if one does).
    pub start: usize,
    pub end: usize,
    /// Its box, px in the window.
    pub rect: Rect,
    /// Its baseline's height, px in the window.
    pub baseline: f32,
    /// Its paragraph goes right to left.
    pub rtl: bool,
}

/// What a range of text an area takes counts (Compose's TextGranularity): characters, or whole words.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TextGranularity {
    #[default]
    Character,
    Word,
}

/// A text's layout as a platform reads it (see the module's doc).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct PlatformTextLayout {
    /// The text the offsets are of.
    pub text: String,
    /// Each character's bounds, in the text's order (one per `char`).
    pub characters: Vec<CharacterBounds>,
    /// Each line's bounds, top to bottom (an empty text has one).
    pub lines: Vec<LineBounds>,
    /// Where a caret after the text's last character is, px in the window (no wide, as tall as the last line).
    pub end_cursor: Rect,
    /// What shows of the text: the inner text field's visible bounds (a text node's own bounds), px in the window
    /// (Compose's `innerTextFieldBounds`, mapped).
    pub inner_text_field_bounds: Rect,
    /// The field with its decoration (Compose's `decorationBoxBounds`, mapped), px in the window.
    pub decoration_box_bounds: Rect,
    /// What takes the text layout's coordinates to the window's (Compose's `textFieldToRootTransform`).
    pub transform: Matrix,
}

impl PlatformTextLayout {
    /// The layout of `text`, shown as `text_layout_result` lays out what `offset_mapping` maps it to, the layout's
    /// coordinates taken to the window's by `transform`; what shows of it and the field's box (`inner_text_field_
    /// bounds`, `decoration_box_bounds`) in the window's coordinates already.
    pub fn new(
        text: &str,
        offset_mapping: &OffsetMapping,
        text_layout_result: &TextLayoutResult,
        transform: Matrix,
        inner_text_field_bounds: Rect,
        decoration_box_bounds: Rect,
    ) -> PlatformTextLayout {
        let layout = text_layout_result;
        let shown = layout.layout_input.text.text();
        // Each shown character's box (one pass over the text: Compose's fillBoundingBoxes), the characters of a
        // grapheme cluster given the cluster's.
        let starts: Vec<usize> = shown.char_indices().map(|(i, _)| i).collect();
        let mut boxes = vec![0.0f32; starts.len() * 4];
        if !shown.is_empty() {
            layout.multi_paragraph.fill_bounding_boxes(TextRange::new(0, shown.len()), &mut boxes, 0);
        }
        let mut shown_boxes: Vec<Rect> =
            boxes.as_chunks::<4>().0.iter().map(|b| Rect::new(b[0].min(b[2]), b[1], b[0].max(b[2]), b[3])).collect();
        for cluster in kompoz_text::segment::graphemes(shown) {
            let (first, last) = (index_of(&starts, cluster.start), index_of(&starts, cluster.end));
            if last > first + 1 {
                let whole = shown_boxes[first..last].iter().skip(1).fold(shown_boxes[first], |a, b| a.union(*b));
                shown_boxes[first..last].fill(whole);
            }
        }
        let floor = |at: usize| {
            let mut at = at.min(shown.len());
            while !shown.is_char_boundary(at) {
                at -= 1;
            }
            at
        };
        let rtl_at = |at: usize| {
            if at < shown.len() {
                layout.get_bidi_run_direction(at) == ResolvedTextDirection::Rtl
            } else {
                layout.get_paragraph_direction(at) == ResolvedTextDirection::Rtl
            }
        };
        let characters = text
            .char_indices()
            .map(|(start, c)| {
                let end = start + c.len_utf8();
                let (from, to) = (
                    floor(offset_mapping.original_to_transformed(start)),
                    floor(offset_mapping.original_to_transformed(end)),
                );
                let (first, last) = (index_of(&starts, from), index_of(&starts, to));
                // What shows for it: of what it maps to, the characters that map back past its start (what a
                // transformation inserted before it — a phone number's separator — maps back to it too, but ends
                // where it starts); all of them where none does.
                let own: Vec<usize> = (first..last)
                    .filter(|&k| {
                        let next = starts.get(k + 1).copied().unwrap_or(shown.len());
                        offset_mapping.transformed_to_original(next) > start
                    })
                    .collect();
                let own = if own.is_empty() { (first..last).collect() } else { own };
                let at = own.first().map_or(from, |&k| starts[k]);
                let local = match own.split_first() {
                    Some((&k, rest)) => rest.iter().fold(shown_boxes[k], |a, &b| a.union(shown_boxes[b])),
                    None => layout.get_cursor_rect(from),
                };
                CharacterBounds {
                    start,
                    end,
                    rect: transform.map_rect(local),
                    rtl: rtl_at(at),
                    line: layout.get_line_for_offset(at),
                }
            })
            .collect();
        let lines = (0..layout.line_count())
            .map(|i| {
                let to_original = |at: usize| offset_mapping.transformed_to_original(at).min(text.len());
                let (left, right) = (layout.get_line_left(i), layout.get_line_right(i));
                let local =
                    Rect::new(left.min(right), layout.get_line_top(i), left.max(right), layout.get_line_bottom(i));
                let start = layout.get_line_start(i);
                LineBounds {
                    start: to_original(start),
                    end: to_original(layout.get_line_end(i, false)),
                    rect: transform.map_rect(local),
                    baseline: transform.map(Offset::new(local.left, layout.get_line_baseline(i))).y,
                    rtl: layout.get_paragraph_direction(start.min(shown.len())) == ResolvedTextDirection::Rtl,
                }
            })
            .collect();
        PlatformTextLayout {
            text: text.to_string(),
            characters,
            lines,
            end_cursor: transform.map_rect(layout.get_cursor_rect(shown.len())),
            inner_text_field_bounds,
            decoration_box_bounds,
            transform,
        }
    }

    /// The layout of a text as it is shown (no visual transformation) at `coordinates`, cut to `visible` (both in
    /// the window): a text node's, as accessibility reads it.
    pub fn of_text(
        text_layout_result: &TextLayoutResult,
        coordinates: &LayoutCoordinates,
        visible: Rect,
    ) -> PlatformTextLayout {
        let text = text_layout_result.layout_input.text.text().to_string();
        let transform = window_transform(coordinates);
        PlatformTextLayout::new(&text, &OffsetMapping::Identity, text_layout_result, transform, visible, visible)
    }

    /// The same boxes for `text` in place of its text, character for character (a field's value as accessibility
    /// sees it: a password's bullets for what was typed); none if `text` has another number of characters.
    pub fn with_text(&self, text: &str) -> Option<PlatformTextLayout> {
        if text == self.text {
            return Some(self.clone());
        }
        let starts: Vec<usize> = text.char_indices().map(|(i, _)| i).chain(std::iter::once(text.len())).collect();
        if starts.len() != self.characters.len() + 1 {
            return None;
        }
        // (Each old byte offset that starts a character, or ends the text, to the new one.)
        let to_new = |offset: usize| starts[self.characters.partition_point(|c| c.start < offset)];
        let mut out = self.clone();
        out.text = text.to_string();
        for (i, c) in out.characters.iter_mut().enumerate() {
            (c.start, c.end) = (starts[i], starts[i + 1]);
        }
        for line in &mut out.lines {
            (line.start, line.end) = (to_new(line.start), to_new(line.end));
        }
        Some(out)
    }

    /// The character starting at byte `offset` (an index into [`characters`](Self::characters)), if one does.
    pub fn character_index(&self, offset: usize) -> Option<usize> {
        self.characters.binary_search_by_key(&offset, |c| c.start).ok()
    }

    /// The box of the character at byte `offset` (the one it is in), if it is in the text.
    pub fn bounding_box(&self, offset: usize) -> Option<Rect> {
        let i = self.characters.partition_point(|c| c.end <= offset);
        self.characters.get(i).map(|c| c.rect)
    }

    /// Where a caret at byte `offset` is drawn (Compose's getCursorRect): no wide, as tall as its line, at the
    /// leading edge of the character after it (the end of the text: after the last one).
    pub fn cursor_rect(&self, offset: usize) -> Rect {
        let i = self.characters.partition_point(|c| c.end <= offset);
        match self.characters.get(i) {
            Some(c) => {
                let x = if c.rtl { c.rect.right } else { c.rect.left };
                Rect::new(x, c.rect.top, x, c.rect.bottom)
            }
            None => self.end_cursor,
        }
    }

    /// The line byte `offset` is on (Compose's getLineForOffset; the end of the text is on the last line).
    pub fn line_for_offset(&self, offset: usize) -> usize {
        match self.characters.get(self.characters.partition_point(|c| c.end <= offset)) {
            Some(c) => c.line,
            None => self.lines.len().saturating_sub(1),
        }
    }

    /// The boxes the bytes `start..end` take: one per line they are on, top to bottom, each the union of its
    /// characters' boxes there (UI Automation's GetBoundingRectangles). Empty for an empty range.
    pub fn range_rects(&self, start: usize, end: usize) -> Vec<Rect> {
        let mut out: Vec<(usize, Rect)> = Vec::new();
        for c in self.characters.iter().filter(|c| c.start >= start.min(end) && c.start < end.max(start)) {
            match out.last_mut() {
                Some((line, rect)) if *line == c.line => *rect = rect.union(c.rect),
                _ => out.push((c.line, c.rect)),
            }
        }
        out.into_iter().map(|(_, r)| r).collect()
    }

    /// The box the bytes `start..end` take (the union of their characters'); a caret's for an empty range.
    pub fn range_bounds(&self, start: usize, end: usize) -> Rect {
        let rects = self.range_rects(start, end);
        match rects.split_first() {
            Some((first, rest)) => rest.iter().fold(*first, |a, b| a.union(*b)),
            None => self.cursor_rect(start.min(end)),
        }
    }

    /// The box of the part of `start..end` on the line `start` is on, and where that part ends (macOS's
    /// firstRectForCharacterRange and its actual range); a caret's for an empty range.
    pub fn first_rect(&self, start: usize, end: usize) -> (Rect, usize) {
        let (start, end) = (start.min(end), start.max(end));
        let mut part: Option<(usize, Rect, usize)> = None;
        for c in self.characters.iter().filter(|c| c.start >= start && c.start < end) {
            match &mut part {
                None => part = Some((c.line, c.rect, c.end)),
                Some((line, rect, until)) if *line == c.line => {
                    *rect = rect.union(c.rect);
                    *until = c.end;
                }
                Some(_) => break,
            }
        }
        match part {
            Some((_, rect, until)) => (rect, until),
            None => (self.cursor_rect(start), start),
        }
    }

    /// The character whose box holds `point` (px in the window): its index into
    /// [`characters`](Self::characters), if one does (AT-SPI's GetOffsetAtPoint, macOS's characterIndexForPoint).
    pub fn character_at(&self, point: Offset) -> Option<usize> {
        let line = self.lines.iter().position(|l| point.y >= l.rect.top && point.y < l.rect.bottom)?;
        self.characters
            .iter()
            .position(|c| c.line == line && c.rect.width() > 0.0 && point.x >= c.rect.left && point.x < c.rect.right)
    }

    /// The byte offset a caret goes to for `point` (px in the window; Compose's getOffsetForPosition): on the line
    /// at its height (the first above, the last below), at the edge of the character under it nearer to it (the
    /// nearest character's, beyond the line's ends).
    pub fn offset_for_position(&self, point: Offset) -> usize {
        let Some(line) = self.line_for_vertical_position(point.y) else { return 0 };
        let on_line = self.characters.iter().filter(|c| c.line == line && c.rect.width() > 0.0);
        let distance = |c: &CharacterBounds| {
            if point.x < c.rect.left {
                c.rect.left - point.x
            } else if point.x >= c.rect.right {
                point.x - c.rect.right
            } else {
                0.0
            }
        };
        let Some(c) = on_line.min_by(|a, b| distance(a).total_cmp(&distance(b))) else {
            return self.lines[line].start;
        };
        // (Past the middle towards its trailing edge: after it.)
        let after = if c.rtl { point.x < c.rect.center().x } else { point.x >= c.rect.center().x };
        // (A line ended by a soft wrap or a break: the caret after its last character is the next line's start, so
        // the end stays on this line before what ends it.)
        if after && c.end == self.lines[line].end && line + 1 < self.lines.len() {
            let ends_with_break = self.text[c.start..c.end].chars().all(|ch| ch == '\n' || ch == '\r');
            if ends_with_break { c.start } else { c.end }
        } else if after {
            c.end
        } else {
            c.start
        }
    }

    /// The line at height `y` (px in the window): the first above the text, the last below it; none without lines.
    pub fn line_for_vertical_position(&self, y: f32) -> Option<usize> {
        if self.lines.is_empty() {
            return None;
        }
        Some(self.lines.iter().position(|l| y < l.rect.bottom).unwrap_or(self.lines.len() - 1))
    }

    /// The bytes `rect` (px in the window) takes (Compose's getRangeForRect with its ContainsCenter inclusion): from
    /// the first character whose box's centre is in it to the end of the last, whole words for
    /// [`TextGranularity::Word`] (a word counts where the centre of its box is in it); none if it takes none.
    pub fn range_for_rect(&self, rect: Rect, granularity: TextGranularity) -> Option<TextRange> {
        let center_in = |r: Rect| {
            let c = r.center();
            c.x >= rect.left && c.x <= rect.right && c.y >= rect.top && c.y <= rect.bottom
        };
        match granularity {
            TextGranularity::Character => {
                let mut taken = self.characters.iter().filter(|c| c.rect.width() > 0.0 && center_in(c.rect));
                let first = taken.next()?;
                let last = taken.next_back().unwrap_or(first);
                Some(TextRange::new(first.start, last.end))
            }
            TextGranularity::Word => {
                let words = kompoz_text::segment::words(&self.text, None);
                let mut taken = words
                    .iter()
                    .filter(|w| w.kind.is_word_like())
                    .filter(|w| self.range_rects(w.range.start, w.range.end).into_iter().any(center_in));
                let first = taken.next()?;
                let last = taken.next_back().unwrap_or(first);
                Some(TextRange::new(first.range.start, last.range.end))
            }
        }
    }
}

/// Where an input method places its candidates for the focused field whose text is `state`'s (px in the window):
/// at the caret's glyph as `layout` says where it is (the caret's line from its top to its bottom, a px wide, at
/// the selection's end — inside a composition, where its own caret is), else `area` (the caret's box the host told
/// with `text_input`, Compose's `focusedRectInRoot`). A layout of another text (not laid out yet since it changed)
/// isn't read.
pub fn caret_area(area: Rect, state: &crate::platform::TextEditState, layout: Option<&PlatformTextLayout>) -> Rect {
    match layout {
        Some(layout) if layout.text == state.text => {
            let caret = layout.cursor_rect(state.selection_end.min(state.text.len()));
            Rect::new(caret.left, caret.top, caret.left + caret.width().max(1.0), caret.bottom)
        }
        _ => area,
    }
}

/// Where `offset` is among the starts of a text's characters (`starts.len()` at its end).
fn index_of(starts: &[usize], offset: usize) -> usize {
    starts.partition_point(|&s| s < offset)
}

/// What takes `coordinates`' points to the window's: to its root's, then where the root is in the window (a
/// popup's root is placed in the host's window).
pub fn window_transform(coordinates: &LayoutCoordinates) -> Matrix {
    let mut matrix = Matrix::new();
    if !coordinates.is_attached() {
        return matrix;
    }
    let root = coordinates.find_root_coordinates();
    root.transform_from(coordinates, &mut matrix);
    let at = root.local_to_window(Offset::Zero);
    // (The translation after what it does: the matrix times a translation's, each row's x and y moved by its w.)
    for row in 0..4 {
        let w = matrix.get(row, 3);
        matrix.set(row, 0, matrix.get(row, 0) + w * at.x);
        matrix.set(row, 1, matrix.get(row, 1) + w * at.y);
    }
    matrix
}
