//! Where things are in a laid out paragraph (Compose's Paragraph queries as AndroidParagraph answers them through
//! Android's Layout and Compose's TextLayout and LayoutHelper): lines and their bounds, the line at an offset or a
//! height, carets (primary and secondary, downstream and upstream at a line's edges), the offset nearest a
//! position, characters' boxes, selection paths, words, the range an area takes, placeholders.

use super::Paragraph;
use super::ellipsis::Ellipsis;
use super::lines::{self, LineGeometry, visible_end_of};
use crate::placeholder::PlaceholderVerticalAlign;
use crate::range::TextRange;
use crate::segment::{find_following_break, find_preceding_break, grapheme_breaks};
use crate::style::{ResolvedTextDirection, TextOverflow};
use crate::text_granularity::{TextGranularity, TextInclusionStrategy};
use crate::word_boundary::{WordIterator, WordSegmentFinder, word_boundary};
use kompoz_graphics::{Offset, Path, PathDirection, Rect};

impl Paragraph {
    /// Line `line` laid out across (worked out once, when first asked for).
    pub(crate) fn geometry(&self, line: usize) -> &LineGeometry {
        let l = self.line(line);
        l.geometry.get_or_init(|| lines::geometry(&self.intrinsics, l, self.width))
    }

    /// The top of line `line_index` (the first line's is 0).
    #[track_caller]
    pub fn get_line_top(&self, line_index: usize) -> f32 {
        self.line(line_index).top
    }
    /// The bottom of line `line_index`.
    #[track_caller]
    pub fn get_line_bottom(&self, line_index: usize) -> f32 {
        self.line(line_index).bottom
    }
    /// The baseline of line `line_index`.
    #[track_caller]
    pub fn get_line_baseline(&self, line_index: usize) -> f32 {
        self.line(line_index).baseline
    }
    /// How tall line `line_index` is.
    #[track_caller]
    pub fn get_line_height(&self, line_index: usize) -> f32 {
        let l = self.line(line_index);
        l.bottom - l.top
    }
    /// How far line `line_index` reaches above its baseline.
    #[track_caller]
    pub fn get_line_ascent(&self, line_index: usize) -> f32 {
        self.line(line_index).ascent
    }
    /// How far line `line_index` reaches below its baseline (the fonts' leading included).
    #[track_caller]
    pub fn get_line_descent(&self, line_index: usize) -> f32 {
        self.line(line_index).descent
    }
    /// Where the visible text of line `line_index` starts from the paragraph's left.
    #[track_caller]
    pub fn get_line_left(&self, line_index: usize) -> f32 {
        self.geometry(line_index).left
    }
    /// Where the visible text of line `line_index` ends from the paragraph's left.
    #[track_caller]
    pub fn get_line_right(&self, line_index: usize) -> f32 {
        let g = self.geometry(line_index);
        g.left + g.width
    }
    /// How wide the visible text of line `line_index` is (a justified line: its whole room).
    #[track_caller]
    pub fn get_line_width(&self, line_index: usize) -> f32 {
        self.geometry(line_index).width
    }
    /// Where line `line_index` starts in the text.
    #[track_caller]
    pub fn get_line_start(&self, line_index: usize) -> usize {
        self.line(line_index).range.start
    }
    /// Where line `line_index` ends in the text: past its trailing whitespace and `\n` (the text's end for a line
    /// ellipsized at its end), or with `visible_end` where its visible text ends (where the ellipsis starts).
    #[track_caller]
    pub fn get_line_end(&self, line_index: usize, visible_end: bool) -> usize {
        let l = self.line(line_index);
        match &l.ellipsis {
            Some(Ellipsis { range, overflow: TextOverflow::Ellipsis }) => {
                if visible_end {
                    range.start
                } else {
                    self.text().len()
                }
            }
            _ if visible_end => visible_end_of(self.text(), l.range.start, l.range.end),
            _ => l.range.end,
        }
    }
    /// Whether line `line_index` was ellipsized.
    #[track_caller]
    pub fn is_line_ellipsized(&self, line_index: usize) -> bool {
        self.line(line_index).ellipsis.is_some()
    }
    /// Where line `line_index`'s ellipsis starts from the line's start, bytes (0 if it has none: Android's
    /// getEllipsisStart, which Compose's AndroidParagraph keeps internal).
    #[track_caller]
    pub fn get_line_ellipsis_offset(&self, line_index: usize) -> usize {
        let l = self.line(line_index);
        l.ellipsis.as_ref().map_or(0, |e| e.range.start - l.range.start)
    }
    /// How much of line `line_index`'s text its ellipsis replaces, bytes (Android's getEllipsisCount).
    #[track_caller]
    pub fn get_line_ellipsis_count(&self, line_index: usize) -> usize {
        self.line(line_index).ellipsis.as_ref().map_or(0, |e| e.range.len())
    }

    /// The line holding `offset` (a line's end belongs to the next line; past the laid out text, the last line).
    pub fn get_line_for_offset(&self, offset: usize) -> usize {
        self.lines.partition_point(|l| l.range.start <= offset).saturating_sub(1)
    }

    /// The line at `vertical` px from the top (above the first: the first; below the last: the last).
    pub fn get_line_for_vertical_position(&self, vertical: f32) -> usize {
        self.lines.partition_point(|l| l.top <= vertical).saturating_sub(1)
    }

    /// The line holding `offset`, upstream (a line's end belonging to it rather than to the next line) or not
    /// (Compose's Layout.getLineForOffset(offset, upstream)).
    fn line_for_offset_upstream(&self, offset: usize, upstream: bool) -> usize {
        if offset == 0 {
            return 0;
        }
        if offset >= self.text().len() {
            return self.lines.len() - 1;
        }
        let line = self.get_line_for_offset(offset);
        let (start, end) = (self.lines[line].range.start, self.lines[line].range.end);
        if start != offset && end != offset {
            return line;
        }
        if start == offset {
            if upstream { line.saturating_sub(1) } else { line }
        } else if upstream {
            line
        } else {
            (line + 1).min(self.lines.len() - 1)
        }
    }

    /// The paragraph's direction at `offset` (its `\n` paragraph's).
    pub fn get_paragraph_direction(&self, offset: usize) -> ResolvedTextDirection {
        let line = self.get_line_for_offset(offset);
        self.intrinsics.paragraphs[self.lines[line].para].bidi.direction()
    }

    /// Which way the bidi run holding the character at `offset` goes (Android's isRtlCharAt).
    pub fn get_bidi_run_direction(&self, offset: usize) -> ResolvedTextDirection {
        let line = self.get_line_for_offset(offset);
        let g = self.geometry(line);
        g.bidi.run_direction(offset.saturating_sub(g.para_start))
    }

    /// Where a caret at `offset` goes on line `line` (Android's getHorizontal for that line).
    fn horizontal_on(&self, line: usize, offset: usize, primary: bool) -> f32 {
        let l = &self.lines[line];
        let offset = offset.clamp(l.range.start, l.range.end);
        self.geometry(line).horizontal(offset, primary)
    }

    /// Where a caret at `offset` goes, downstream (Android's getPrimaryHorizontal / getSecondaryHorizontal,
    /// clamped to the end of the line it falls on).
    fn downstream_horizontal(&self, offset: usize, primary: bool) -> f32 {
        let line = self.get_line_for_offset(offset);
        self.horizontal_on(line, offset.min(self.lines[line].range.end), primary)
    }

    /// Where a caret at `offset` goes, upstream at a line's edges taking the end of the line before rather than
    /// the start of the line after (Compose's LayoutHelper.getHorizontalPosition).
    fn horizontal_position(&self, offset: usize, primary: bool, upstream: bool) -> f32 {
        if !upstream {
            return self.downstream_horizontal(offset, primary);
        }
        let line = self.line_for_offset_upstream(offset, true);
        let (start, end) = (self.lines[line].range.start, self.lines[line].range.end);
        if (offset != start && offset != end) || offset == 0 || offset == self.text().len() {
            return self.downstream_horizontal(offset, primary);
        }
        let text = self.text();
        let para = &self.intrinsics.paragraphs[self.lines[line].para];
        let para_rtl = para.bidi.direction() == ResolvedTextDirection::Rtl;
        let visible_end = visible_end_of(text, start, end);
        let ps = para.range.start;
        let line_bidi = para.bidi.line(&text[para.range.clone()], start - ps..visible_end.max(start) - ps);
        let g = self.geometry(line);
        let (line_left, line_right) = (g.left, g.left + g.width);
        if line_bidi.runs().len() <= 1 {
            let run_rtl = g.bidi.run_direction(start - ps) == ResolvedTextDirection::Rtl;
            let start_left = if primary || para_rtl == run_rtl { !para_rtl } else { para_rtl };
            let offset_left = if offset == start { start_left } else { !start_left };
            return if offset_left { line_left } else { line_right };
        }
        let runs: Vec<(usize, usize, bool)> =
            line_bidi.runs().iter().map(|r| (r.range.start + ps, r.range.end + ps, r.is_rtl())).collect();
        let last = runs.len() - 1;
        if offset == start {
            let Some(index) = runs.iter().position(|r| r.0 == offset) else {
                return self.downstream_horizontal(offset, primary);
            };
            let left = if primary || para_rtl == runs[index].2 { !para_rtl } else { para_rtl };
            if index == 0 && left {
                line_left
            } else if index == last && !left {
                line_right
            } else if left {
                self.downstream_horizontal(runs[index - 1].0, true)
            } else {
                self.downstream_horizontal(runs[index + 1].0, true)
            }
        } else {
            let adjusted = if offset > visible_end { visible_end_of(text, start, offset) } else { offset };
            let Some(index) = runs.iter().position(|r| r.1 == adjusted) else {
                return self.downstream_horizontal(offset, primary);
            };
            let left = if primary || para_rtl == runs[index].2 { para_rtl } else { !para_rtl };
            if index == 0 && left {
                line_left
            } else if index == last && !left {
                line_right
            } else if left {
                self.downstream_horizontal(runs[index - 1].1, true)
            } else {
                self.downstream_horizontal(runs[index + 1].1, true)
            }
        }
    }

    /// Where a caret at `offset` goes from the paragraph's left (Compose's getHorizontalPosition): where text of
    /// the paragraph's direction would be inserted (`use_primary_direction`), or of the other direction.
    pub fn get_horizontal_position(&self, offset: usize, use_primary_direction: bool) -> f32 {
        self.downstream_horizontal(offset, use_primary_direction)
    }

    /// The offset nearest `position` (Compose's getOffsetForPosition, Android's getOffsetForHorizontal on the line
    /// at its height): of the grapheme boundaries of that line (its end only on the last line), the one whose
    /// primary caret is nearest (the earlier on a tie, but for the farthest one). Below lines cut off by
    /// `max_lines`, the text's end.
    pub fn get_offset_for_position(&self, position: Offset) -> usize {
        let line = self.get_line_for_vertical_position(position.y);
        if self.truncated && line + 1 == self.lines.len() && position.y >= self.lines[line].bottom {
            return self.text().len();
        }
        self.offset_for_horizontal(line, position.x)
    }

    /// The offset on line `line` nearest `x`.
    fn offset_for_horizontal(&self, line: usize, x: f32) -> usize {
        let l = &self.lines[line];
        let (start, end) = (l.range.start, l.range.end);
        // The line's grapheme boundaries (a line starts at one).
        let bounds: Vec<usize> = grapheme_breaks(&self.text()[start..end]).map(|b| start + b).collect();
        let max = if line + 1 == self.lines.len() {
            end
        } else {
            bounds.iter().copied().rev().find(|&b| b < end).unwrap_or(start)
        };
        let g = self.geometry(line);
        let mut best = start;
        let mut best_distance = (g.horizontal(start, true) - x).abs();
        for &b in bounds.iter().filter(|&&b| b > start && b < max) {
            let d = (g.horizontal(b, true) - x).abs();
            if d < best_distance {
                best_distance = d;
                best = b;
            }
        }
        if max > start && (g.horizontal(max, true) - x).abs() <= best_distance {
            best = max;
        }
        best
    }

    /// Where a caret at `offset` is drawn (Compose's getCursorRect): no wide, as tall as its line, at its primary
    /// position. `offset` must be within the text (its end included).
    #[track_caller]
    pub fn get_cursor_rect(&self, offset: usize) -> Rect {
        let len = self.text().len();
        assert!(offset <= len, "kompoz: offset({offset}) is out of bounds [0,{len}]");
        let h = self.horizontal_position(offset, true, false);
        let line = self.get_line_for_offset(offset);
        Rect::new(h, self.lines[line].top, h, self.lines[line].bottom)
    }

    /// The box of the character at `offset` (Compose's getBoundingBox, Compose's TextLayout.getBoundingBox): from
    /// its leading to its trailing edge (upstream at a line's end), as tall as its line. `offset` must be a
    /// character's start in the text.
    #[track_caller]
    pub fn get_bounding_box(&self, offset: usize) -> Rect {
        let len = self.text().len();
        assert!(offset < len, "kompoz: offset({offset}) is out of bounds [0,{len})");
        let next = self.text()[offset..].chars().next().map_or(offset + 1, |c| offset + c.len_utf8());
        self.char_box(offset, next)
    }

    /// The box of the character `offset..next`.
    fn char_box(&self, offset: usize, next: usize) -> Rect {
        let line = self.get_line_for_offset(offset);
        let (top, bottom) = (self.lines[line].top, self.lines[line].bottom);
        let ltr_line = self.get_paragraph_direction(offset) == ResolvedTextDirection::Ltr;
        let rtl_char = self.get_bidi_run_direction(offset) == ResolvedTextDirection::Rtl;
        let (left, right) = match (ltr_line, rtl_char) {
            (true, false) => {
                (self.horizontal_position(offset, true, false), self.horizontal_position(next, true, true))
            }
            (true, true) => {
                (self.horizontal_position(next, false, true), self.horizontal_position(offset, false, false))
            }
            (false, true) => {
                (self.horizontal_position(next, true, true), self.horizontal_position(offset, true, false))
            }
            (false, false) => {
                (self.horizontal_position(offset, false, false), self.horizontal_position(next, false, true))
            }
        };
        Rect::new(left, top, right, bottom)
    }

    /// The box of each character of `range` into `array` from `array_start` (left, top, right, bottom each;
    /// Compose's fillBoundingBoxes, a box per character).
    #[track_caller]
    pub fn fill_bounding_boxes(&self, range: TextRange, array: &mut [f32], array_start: usize) {
        let text = self.text();
        let (start, end) = (range.min(), range.max());
        assert!(start < text.len() && end <= text.len() && end > start, "kompoz: range {range:?} is out of bounds");
        let mut at = array_start;
        for (i, c) in text[start..end].char_indices() {
            let offset = start + i;
            let b = self.char_box(offset, offset + c.len_utf8());
            array[at..at + 4].copy_from_slice(&[b.left, b.top, b.right, b.bottom]);
            at += 4;
        }
    }

    /// The area selecting `start..end` highlights (Compose's getPathForRange, Android's getSelectionPath): on each
    /// line, a rect per bidi run of the range (from a character's leading edge to the trailing edge of the one
    /// before the range's end); where the range goes on past a line, to the paragraph's edge after that line's
    /// text (its left in a right-to-left paragraph), the lines in between whole.
    #[track_caller]
    pub fn get_path_for_range(&self, start: usize, end: usize) -> Path {
        let len = self.text().len();
        assert!(
            start <= end && end <= len,
            "kompoz: start({start}) or end({end}) is out of range [0..{len}], or start > end!"
        );
        let mut path = Path::new();
        if start == end {
            return path;
        }
        let first = self.get_line_for_offset(start);
        let last = self.get_line_for_offset(end);
        let width = self.width;
        let mut rect = |l: f32, t: f32, r: f32, b: f32| {
            path.add_rect(Rect::new(l.min(r), t, l.max(r), b), PathDirection::Clockwise);
        };
        let line_rtl =
            |i: usize| self.intrinsics.paragraphs[self.lines[i].para].bidi.direction() == ResolvedTextDirection::Rtl;
        if first == last {
            for r in self.selection_rects(first, start, end) {
                rect(r.left, r.top, r.right, r.bottom);
            }
            return path;
        }
        let (top, bottom) = (self.lines[first].top, self.lines[first].bottom);
        for r in self.selection_rects(first, start, self.lines[first].range.end) {
            rect(r.left, r.top, r.right, r.bottom);
        }
        if line_rtl(first) {
            rect(self.get_line_left(first), top, 0.0, bottom);
        } else {
            rect(self.get_line_right(first), top, width, bottom);
        }
        for i in first + 1..last {
            rect(0.0, self.lines[i].top, width, self.lines[i].bottom);
        }
        let (top, bottom) = (self.lines[last].top, self.lines[last].bottom);
        for r in self.selection_rects(last, self.lines[last].range.start, end) {
            rect(r.left, r.top, r.right, r.bottom);
        }
        if line_rtl(last) {
            rect(width, top, self.get_line_right(last), bottom);
        } else {
            rect(0.0, top, self.get_line_left(last), bottom);
        }
        path
    }

    /// The rects selecting `start..end` on line `line` covers (Android's Layout.addSelection): one per bidi run
    /// it meets, its `\n` left out.
    fn selection_rects(&self, line: usize, start: usize, end: usize) -> Vec<Rect> {
        let l = &self.lines[line];
        let mut line_end = l.range.end;
        if line_end > l.range.start && self.text().as_bytes()[line_end - 1] == b'\n' {
            line_end -= 1;
        }
        let g = self.geometry(line);
        let ps = g.para_start;
        let mut out = Vec::new();
        for run in g.bidi.logical_runs() {
            let here = run.range.start + ps;
            let there = (run.range.end + ps).min(line_end);
            if start <= there && end >= here {
                let (st, en) = (start.max(here), end.min(there));
                if st != en {
                    let (h1, h2) = (g.position(st, false), g.position(en, true));
                    out.push(Rect::new(h1.min(h2), l.top, h1.max(h2), l.bottom));
                }
            }
        }
        out
    }

    /// The word at `offset` (Compose's getWordBoundary: ICU's words, as Android's WordIterator takes them).
    pub fn get_word_boundary(&self, offset: usize) -> TextRange {
        word_boundary(self.text(), offset, &self.intrinsics.settings.locale)
    }

    /// The range of text `rect` takes, by `granularity`, a piece taken where `inclusion_strategy` says (Compose's
    /// getRangeForRect, its TextLayout's pre-Android-14 way): from the first piece taken on the first line it
    /// takes any on to the last piece on the last; Zero if none.
    pub fn get_range_for_rect(
        &self,
        rect: Rect,
        granularity: TextGranularity,
        inclusion_strategy: &TextInclusionStrategy,
    ) -> TextRange {
        let text = self.text();
        let words = WordIterator::new(text, &self.intrinsics.settings.locale);
        let finder =
            Segments { text, words: (granularity == TextGranularity::Word).then(|| WordSegmentFinder::new(words)) };
        let count = self.lines.len();
        let mut start_line = self.get_line_for_vertical_position(rect.top);
        if rect.top > self.lines[start_line].bottom {
            start_line += 1;
            if start_line >= count {
                return TextRange::Zero;
            }
        }
        let mut end_line = self.get_line_for_vertical_position(rect.bottom);
        if end_line == 0 && rect.bottom < self.lines[0].top {
            return TextRange::Zero;
        }
        let mut start = self.offset_in_line(start_line, rect, &finder, inclusion_strategy, true);
        while start.is_none() && start_line < end_line {
            start_line += 1;
            start = self.offset_in_line(start_line, rect, &finder, inclusion_strategy, true);
        }
        let Some(start) = start else { return TextRange::Zero };
        let mut end = self.offset_in_line(end_line, rect, &finder, inclusion_strategy, false);
        while end.is_none() && start_line < end_line {
            end_line -= 1;
            end = self.offset_in_line(end_line, rect, &finder, inclusion_strategy, false);
        }
        let Some(end) = end else { return TextRange::Zero };
        let start = finder.previous_start_boundary(start + 1).unwrap_or(start);
        let end = finder.next_end_boundary(end.saturating_sub(1)).unwrap_or(end);
        TextRange::new(start, end)
    }

    /// Each character's left and right on line `line` (Compose's TextLayout.fillLineHorizontalBounds), indexed by
    /// byte from the line's start (a character's bounds at its first byte).
    fn line_horizontal_bounds(&self, line: usize) -> Vec<(f32, f32)> {
        let l = &self.lines[line];
        let text = self.text();
        let mut bounds = vec![(0.0, 0.0); l.range.len()];
        for (i, c) in text[l.range.clone()].char_indices() {
            let offset = l.range.start + i;
            let b = self.char_box(offset, offset + c.len_utf8());
            bounds[i] = (b.left, b.right);
        }
        bounds
    }

    /// The first (`get_start`) or last piece `rect` takes on line `line`, run by run (Compose's
    /// getStartOrEndOffsetForRectWithinLine).
    fn offset_in_line(
        &self,
        line: usize,
        rect: Rect,
        finder: &Segments<'_>,
        inclusion: &TextInclusionStrategy,
        get_start: bool,
    ) -> Option<usize> {
        let l = &self.lines[line];
        if l.range.is_empty() {
            return None;
        }
        let bounds = self.line_horizontal_bounds(line);
        let line_start = l.range.start;
        let text = self.text();
        let char_at = |o: usize| bounds[o - line_start];
        // The character starting at or before `o`.
        let char_start = |o: usize| {
            let mut s = o;
            while s > line_start && !text.is_char_boundary(s) {
                s -= 1;
            }
            s
        };
        let last_char = |end: usize| text[..end].char_indices().next_back().map_or(end, |(i, _)| i);
        let g = self.geometry(line);
        let ps = g.para_start;
        let mut runs: Vec<(usize, usize, bool)> =
            g.bidi.logical_runs().iter().map(|r| (r.range.start + ps, r.range.end + ps, r.is_rtl())).collect();
        runs.retain(|r| r.0 < r.1);
        if !get_start {
            runs.reverse();
        }
        let (top, bottom) = (l.top, l.bottom);
        for (start, end, rtl) in runs {
            let (run_left, run_right) = if rtl {
                (char_at(last_char(end)).0, char_at(start).1)
            } else {
                (char_at(start).0, char_at(last_char(end)).1)
            };
            if !(run_right >= rect.left && run_left <= rect.right) {
                continue;
            }
            let bounds_of = |s: usize, e: usize| {
                let (left, right) =
                    if rtl { (char_at(last_char(e)).0, char_at(s).1) } else { (char_at(s).0, char_at(last_char(e)).1) };
                Rect::new(left, top, right, bottom)
            };
            let found = if get_start {
                let first_char = if (!rtl && rect.left <= run_left) || (rtl && rect.right >= run_right) {
                    start
                } else {
                    let (mut low, mut high) = (start, end);
                    while high - low > 1 {
                        let guess = char_start((high + low) / 2).max(low);
                        if guess <= low {
                            break;
                        }
                        let position = char_at(guess).0;
                        if (!rtl && position > rect.left) || (rtl && position < rect.right) {
                            high = guess;
                        } else {
                            low = guess;
                        }
                    }
                    if rtl { high.min(last_char(end)) } else { low }
                };
                let Some(mut seg_end) = finder.next_end_boundary(first_char) else { continue };
                let Some(mut seg_start) = finder.previous_start_boundary(seg_end) else { continue };
                if seg_start >= end {
                    continue;
                }
                seg_start = seg_start.max(start);
                seg_end = seg_end.min(end);
                loop {
                    if inclusion.is_included(bounds_of(seg_start, seg_end), rect) {
                        break Some(seg_start);
                    }
                    match finder.next_start_boundary(seg_start) {
                        Some(s) if s < end => {
                            seg_start = s;
                            seg_end = finder.next_end_boundary(s).unwrap_or(end).min(end);
                        }
                        _ => break None,
                    }
                }
            } else {
                let last = if (!rtl && rect.right >= run_right) || (rtl && rect.left <= run_left) {
                    last_char(end)
                } else {
                    let (mut low, mut high) = (start, end);
                    while high - low > 1 {
                        let guess = char_start((high + low) / 2).max(low);
                        if guess <= low {
                            break;
                        }
                        let position = char_at(guess).0;
                        if (!rtl && position > rect.right) || (rtl && position < rect.left) {
                            high = guess;
                        } else {
                            low = guess;
                        }
                    }
                    if rtl { high.min(last_char(end)) } else { low }
                };
                let next = text[last..].chars().next().map_or(last + 1, |c| last + c.len_utf8());
                let Some(mut seg_start) = finder.previous_start_boundary(next) else { continue };
                let Some(mut seg_end) = finder.next_end_boundary(seg_start) else { continue };
                if seg_end <= start {
                    continue;
                }
                seg_start = seg_start.max(start);
                seg_end = seg_end.min(end);
                loop {
                    if inclusion.is_included(bounds_of(seg_start, seg_end), rect) {
                        break Some(seg_end);
                    }
                    match finder.previous_end_boundary(seg_end) {
                        Some(e) if e > start => {
                            seg_end = e;
                            seg_start = finder.previous_start_boundary(e).unwrap_or(start).max(start);
                        }
                        _ => break None,
                    }
                }
            };
            if found.is_some() {
                return found;
            }
        }
        None
    }

    pub(crate) fn compute_placeholder_rects(&self) -> Vec<Option<Rect>> {
        let p = &self.intrinsics;
        let mut rects = vec![None; p.placeholders().len()];
        for run in &p.runs {
            let Some(room) = run.style.placeholder else { continue };
            let (start, end) = (run.range.start, run.range.end);
            let line = self.get_line_for_offset(start);
            let l = &self.lines[line];
            let ellipsized = l.ellipsis.as_ref().is_some_and(|e| end > e.range.start);
            let truncated = end > self.get_line_end(line, false) || (self.truncated && start >= l.range.end);
            if ellipsized || truncated || line >= self.max_lines() {
                continue;
            }
            let ltr_line = self.get_paragraph_direction(start) == ResolvedTextDirection::Ltr;
            let rtl_char = self.get_bidi_run_direction(start) == ResolvedTextDirection::Rtl;
            let w = room.width;
            let (left, right) = match (ltr_line, rtl_char) {
                (true, false) => {
                    let left = self.horizontal_position(start, true, false);
                    (left, left + w)
                }
                (true, true) => {
                    let right = self.horizontal_position(start, false, false);
                    (right - w, right)
                }
                (false, true) => {
                    let right = self.horizontal_position(start, true, false);
                    (right - w, right)
                }
                (false, false) => {
                    let left = self.horizontal_position(start, false, false);
                    (left, left + w)
                }
            };
            let m = &run.style.metrics;
            let h = room.height;
            let baseline = l.baseline;
            let top = match room.align {
                PlaceholderVerticalAlign::AboveBaseline => baseline - h,
                PlaceholderVerticalAlign::Top => l.top,
                PlaceholderVerticalAlign::Bottom => l.bottom - h,
                PlaceholderVerticalAlign::Center => (l.top + l.bottom - h) / 2.0,
                PlaceholderVerticalAlign::TextTop => baseline - m.ascent,
                PlaceholderVerticalAlign::TextBottom => baseline + m.descent - h,
                PlaceholderVerticalAlign::TextCenter => (m.descent - m.ascent - h) / 2.0 + baseline,
            };
            rects[room.index] = Some(Rect::new(left, top, right, top + h));
        }
        rects
    }
}

/// What a range query picks pieces by (Android's SegmentFinder): words, or grapheme clusters (`words` None).
struct Segments<'a> {
    text: &'a str,
    words: Option<WordSegmentFinder<'a>>,
}

impl Segments<'_> {
    fn previous_start_boundary(&self, offset: usize) -> Option<usize> {
        match &self.words {
            Some(w) => w.previous_start_boundary(offset),
            None => find_preceding_break(self.text, offset),
        }
    }
    fn previous_end_boundary(&self, offset: usize) -> Option<usize> {
        match &self.words {
            Some(w) => w.previous_end_boundary(offset),
            None => find_preceding_break(self.text, offset).filter(|&b| b > 0),
        }
    }
    fn next_start_boundary(&self, offset: usize) -> Option<usize> {
        match &self.words {
            Some(w) => w.next_start_boundary(offset),
            None => find_following_break(self.text, offset).filter(|&b| b < self.text.len()),
        }
    }
    fn next_end_boundary(&self, offset: usize) -> Option<usize> {
        match &self.words {
            Some(w) => w.next_end_boundary(offset),
            None => find_following_break(self.text, offset),
        }
    }
}
