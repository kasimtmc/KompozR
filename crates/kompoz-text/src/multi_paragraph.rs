//! Text of several paragraph styles laid out (Compose's MultiParagraphIntrinsics and MultiParagraph): an
//! AnnotatedString split where its paragraph styles begin and end, each piece a [`Paragraph`] in its own style,
//! stacked one under the other; offsets, lines and positions mapped across them.

use crate::annotated_string::{AnnotatedString, Range as AnnotatedRange, intersect};
use crate::font::FontFamilyResolver;
use crate::paragraph::{Paragraph, ParagraphIntrinsics, TextPaintOp, TextPaintStyle};
use crate::paragraph_style::ParagraphStyle;
use crate::placeholder::Placeholder;
use crate::range::TextRange;
use crate::style::{ResolvedTextDirection, TextDirection, TextOverflow};
use crate::text_granularity::{TextGranularity, TextInclusionStrategy};
use crate::text_style::TextStyle;
use kompoz_graphics::{Constraints, Density, Offset, Path, Rect};
use std::fmt;
use std::rc::Rc;

/// A piece of the text measured in its paragraph style, and where it lies in the text.
#[derive(Clone)]
pub struct ParagraphIntrinsicInfo {
    pub intrinsics: Rc<ParagraphIntrinsics>,
    pub start_index: usize,
    pub end_index: usize,
}

/// Text of several paragraph styles measured (Compose's MultiParagraphIntrinsics): each piece its paragraph
/// styles make ([`AnnotatedString::map_each_paragraph_style`]) measured in `style` with its paragraph style over
/// it (its direction the text's where it gives none); its min intrinsic width the widest piece's, its max the
/// widest's.
pub struct MultiParagraphIntrinsics {
    annotated_string: AnnotatedString,
    placeholders: Vec<AnnotatedRange<Placeholder>>,
    info_list: Vec<ParagraphIntrinsicInfo>,
}

impl MultiParagraphIntrinsics {
    /// `annotated_string` in `style` (its defaults resolved, as TextMeasurer gives it), its `placeholders`, the
    /// density, the resolver its faces come from, whether it wraps.
    pub fn new(
        annotated_string: &AnnotatedString,
        style: &TextStyle,
        placeholders: &[AnnotatedRange<Placeholder>],
        density: Density,
        font_family_resolver: &Rc<dyn FontFamilyResolver>,
        soft_wrap: bool,
    ) -> MultiParagraphIntrinsics {
        let paragraph_style = style.to_paragraph_style();
        let info_list = annotated_string.map_each_paragraph_style(&paragraph_style, |text, item| {
            let current = if item.item.text_direction != TextDirection::Unspecified {
                item.item.clone()
            } else {
                ParagraphStyle { text_direction: paragraph_style.text_direction, ..item.item.clone() }
            };
            let local: Vec<AnnotatedRange<Placeholder>> = placeholders
                .iter()
                .filter(|p| intersect(item.start, item.end, p.start, p.end))
                .map(|p| {
                    assert!(
                        item.start <= p.start && p.end <= item.end,
                        "kompoz: placeholder can not overlap with paragraph."
                    );
                    AnnotatedRange::new(p.item, p.start - item.start, p.end - item.start)
                })
                .collect();
            ParagraphIntrinsicInfo {
                intrinsics: Rc::new(ParagraphIntrinsics::new(
                    text.text(),
                    &style.merge(&current),
                    text.annotations(),
                    &local,
                    density,
                    font_family_resolver,
                    soft_wrap,
                )),
                start_index: item.start,
                end_index: item.end,
            }
        });
        MultiParagraphIntrinsics {
            annotated_string: annotated_string.clone(),
            placeholders: placeholders.to_vec(),
            info_list,
        }
    }

    /// The text it measured.
    pub fn annotated_string(&self) -> &AnnotatedString {
        &self.annotated_string
    }
    /// Its placeholders.
    pub fn placeholders(&self) -> &[AnnotatedRange<Placeholder>] {
        &self.placeholders
    }
    /// Its pieces, measured.
    pub fn info_list(&self) -> &[ParagraphIntrinsicInfo] {
        &self.info_list
    }
    /// The widest piece's min intrinsic width.
    pub fn min_intrinsic_width(&self) -> f32 {
        self.info_list.iter().map(|i| i.intrinsics.min_intrinsic_width()).fold(0.0, f32::max)
    }
    /// The widest piece's max intrinsic width.
    pub fn max_intrinsic_width(&self) -> f32 {
        self.info_list.iter().map(|i| i.intrinsics.max_intrinsic_width()).fold(0.0, f32::max)
    }
    /// Whether a face one of its pieces was laid out in has changed since (an async font loaded).
    pub fn has_stale_resolved_fonts(&self) -> bool {
        self.info_list.iter().any(|i| i.intrinsics.has_stale_resolved_fonts())
    }
}

impl fmt::Debug for MultiParagraphIntrinsics {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MultiParagraphIntrinsics").field("text", &self.annotated_string.text()).finish_non_exhaustive()
    }
}

/// A piece of the text laid out: its paragraph, where it lies in the text, its first line and the line after
/// its last among all lines, its top and bottom.
#[derive(Debug)]
pub struct ParagraphInfo {
    pub paragraph: Paragraph,
    pub start_index: usize,
    pub end_index: usize,
    pub start_line_index: usize,
    pub end_line_index: usize,
    pub top: f32,
    pub bottom: f32,
}

impl ParagraphInfo {
    fn len(&self) -> usize {
        self.end_index - self.start_index
    }
    fn to_local(&self, index: usize) -> usize {
        index.clamp(self.start_index, self.end_index) - self.start_index
    }
    fn to_global_range(&self, r: TextRange, zero_as_none: bool) -> TextRange {
        if zero_as_none && r == TextRange::Zero {
            return TextRange::Zero;
        }
        TextRange::new(r.start + self.start_index, r.end + self.start_index)
    }
}

/// Text of several paragraph styles laid out (Compose's MultiParagraph): its pieces laid out in
/// `constraints.max_width` one under another (each in what height is left), at most `max_lines` lines in all —
/// the pieces past them left out — cut as `overflow` says.
pub struct MultiParagraph {
    intrinsics: Rc<MultiParagraphIntrinsics>,
    max_lines: usize,
    did_exceed_max_lines: bool,
    width: f32,
    height: f32,
    line_count: usize,
    placeholder_rects: Vec<Option<Rect>>,
    paragraph_info_list: Vec<ParagraphInfo>,
}

impl MultiParagraph {
    /// `intrinsics` laid out (Compose's MultiParagraph(intrinsics, constraints, maxLines, overflow)): a min width
    /// or height other than 0 panics.
    pub fn new(
        intrinsics: Rc<MultiParagraphIntrinsics>,
        constraints: Constraints,
        max_lines: usize,
        overflow: TextOverflow,
    ) -> MultiParagraph {
        assert!(
            constraints.min_width == 0 && constraints.min_height == 0,
            "kompoz: Setting Constraints.minWidth and Constraints.minHeight is not supported, these should be the \
             default zero values instead."
        );
        let mut height = 0.0f32;
        let mut line_count = 0;
        let mut did_exceed_max_lines = false;
        let mut list: Vec<ParagraphInfo> = Vec::new();
        let infos = intrinsics.info_list();
        for (index, info) in infos.iter().enumerate() {
            let max_height = if constraints.has_bounded_height() {
                (constraints.max_height - height.ceil() as i32).max(0)
            } else {
                constraints.max_height
            };
            let paragraph = Paragraph::new(
                info.intrinsics.clone(),
                Constraints::new(0, constraints.max_width, 0, max_height),
                max_lines - line_count,
                overflow,
            );
            let top = height;
            let bottom = height + paragraph.height();
            height = bottom;
            let start_line_index = line_count;
            let end_line_index = start_line_index + paragraph.line_count();
            line_count = end_line_index;
            let exceeded = paragraph.did_exceed_max_lines();
            list.push(ParagraphInfo {
                paragraph,
                start_index: info.start_index,
                end_index: info.end_index,
                start_line_index,
                end_line_index,
                top,
                bottom,
            });
            if exceeded || (end_line_index == max_lines && index + 1 != infos.len()) {
                did_exceed_max_lines = true;
                break;
            }
        }
        let mut placeholder_rects: Vec<Option<Rect>> = list
            .iter()
            .flat_map(|p| p.paragraph.placeholder_rects().iter().map(move |r| r.map(|r| r.translate_by(0.0, p.top))))
            .collect();
        placeholder_rects.resize(intrinsics.placeholders().len().max(placeholder_rects.len()), None);
        MultiParagraph {
            intrinsics,
            max_lines,
            did_exceed_max_lines,
            width: constraints.max_width as f32,
            height,
            line_count,
            placeholder_rects,
            paragraph_info_list: list,
        }
    }

    /// `annotated_string` measured and laid out at once (Compose's MultiParagraph(annotatedString, style,
    /// constraints, density, fontFamilyResolver, placeholders, maxLines, overflow)).
    #[allow(clippy::too_many_arguments)]
    pub fn of_text(
        annotated_string: &AnnotatedString,
        style: &TextStyle,
        constraints: Constraints,
        density: Density,
        font_family_resolver: &Rc<dyn FontFamilyResolver>,
        placeholders: &[AnnotatedRange<Placeholder>],
        max_lines: usize,
        overflow: TextOverflow,
    ) -> MultiParagraph {
        let intrinsics =
            MultiParagraphIntrinsics::new(annotated_string, style, placeholders, density, font_family_resolver, true);
        MultiParagraph::new(Rc::new(intrinsics), constraints, max_lines, overflow)
    }

    pub fn intrinsics(&self) -> &Rc<MultiParagraphIntrinsics> {
        &self.intrinsics
    }
    pub fn max_lines(&self) -> usize {
        self.max_lines
    }
    pub fn min_intrinsic_width(&self) -> f32 {
        self.intrinsics.min_intrinsic_width()
    }
    pub fn max_intrinsic_width(&self) -> f32 {
        self.intrinsics.max_intrinsic_width()
    }
    /// Whether lines past `max_lines` were left out (or the last one ellipsized for them).
    pub fn did_exceed_max_lines(&self) -> bool {
        self.did_exceed_max_lines
    }
    /// Its width: the max width it was laid out in.
    pub fn width(&self) -> f32 {
        self.width
    }
    /// Its pieces' heights together.
    pub fn height(&self) -> f32 {
        self.height
    }
    /// The first piece's first baseline (0 for none).
    pub fn first_baseline(&self) -> f32 {
        self.paragraph_info_list.first().map_or(0.0, |p| p.paragraph.first_baseline())
    }
    /// The last piece's last baseline (0 for none).
    pub fn last_baseline(&self) -> f32 {
        self.paragraph_info_list.last().map_or(0.0, |p| p.paragraph.last_baseline() + p.top)
    }
    pub fn line_count(&self) -> usize {
        self.line_count
    }
    /// Where each placeholder is (None where it was cut off or ellipsized).
    pub fn placeholder_rects(&self) -> &[Option<Rect>] {
        &self.placeholder_rects
    }
    /// Its pieces laid out.
    pub fn paragraph_info_list(&self) -> &[ParagraphInfo] {
        &self.paragraph_info_list
    }
    fn text_len(&self) -> usize {
        self.intrinsics.annotated_string().len()
    }

    /// The paint operations drawing it with `paint` takes (Compose's MultiParagraph.paint): its pieces one under
    /// another, a brush `paint` gives spread over all of it.
    pub fn paint_ops(&self, paint: &TextPaintStyle) -> Vec<TextPaintOp> {
        let area = Rect::new(0.0, 0.0, self.width, self.height);
        let mut ops = Vec::new();
        for p in &self.paragraph_info_list {
            ops.extend(p.paragraph.paint_ops(paint, area, p.top));
        }
        ops
    }

    /// The area selecting `start..end` highlights (Compose's getPathForRange).
    #[track_caller]
    pub fn get_path_for_range(&self, start: usize, end: usize) -> Path {
        let len = self.text_len();
        assert!(
            start <= end && end <= len,
            "kompoz: Start({start}) or End({end}) is out of range [0..{len}), or start > end!"
        );
        let mut path = Path::new();
        if start == end {
            return path;
        }
        self.for_paragraphs_in(TextRange::new(start, end), |p| {
            let local = p.paragraph.get_path_for_range(p.to_local(start), p.to_local(end));
            path.add_path(&local, Offset::new(0.0, p.top));
        });
        path
    }

    /// The line at `vertical` px from the top.
    pub fn get_line_for_vertical_position(&self, vertical: f32) -> usize {
        let p = &self.paragraph_info_list[self.paragraph_by_y(vertical)];
        if p.len() == 0 {
            p.start_line_index
        } else {
            p.paragraph.get_line_for_vertical_position(vertical - p.top) + p.start_line_index
        }
    }

    /// The offset nearest `position`.
    pub fn get_offset_for_position(&self, position: Offset) -> usize {
        let p = &self.paragraph_info_list[self.paragraph_by_y(position.y)];
        if p.len() == 0 {
            p.start_index
        } else {
            p.paragraph.get_offset_for_position(Offset::new(position.x, position.y - p.top)) + p.start_index
        }
    }

    /// The range of text `rect` takes (Compose's getRangeForRect).
    pub fn get_range_for_rect(
        &self,
        rect: Rect,
        granularity: TextGranularity,
        inclusion_strategy: &TextInclusionStrategy,
    ) -> TextRange {
        let local = |p: &ParagraphInfo| {
            p.to_global_range(
                p.paragraph.get_range_for_rect(rect.translate_by(0.0, -p.top), granularity, inclusion_strategy),
                true,
            )
        };
        let list = &self.paragraph_info_list;
        let mut first = self.paragraph_by_y(rect.top);
        if list[first].bottom >= rect.bottom || first + 1 == list.len() {
            return local(&list[first]);
        }
        let mut last = self.paragraph_by_y(rect.bottom);
        let mut start_range = TextRange::Zero;
        while start_range == TextRange::Zero && first <= last {
            start_range = local(&list[first]);
            first += 1;
        }
        if start_range == TextRange::Zero {
            return TextRange::Zero;
        }
        let mut end_range = TextRange::Zero;
        while end_range == TextRange::Zero && first <= last {
            end_range = local(&list[last]);
            if last == 0 {
                break;
            }
            last -= 1;
        }
        if end_range == TextRange::Zero {
            return start_range;
        }
        TextRange::new(start_range.start, end_range.end)
    }

    /// The box of the character at `offset`, which must be within the text (not its end).
    #[track_caller]
    pub fn get_bounding_box(&self, offset: usize) -> Rect {
        let len = self.text_len();
        assert!(offset < len, "kompoz: offset({offset}) is out of bounds [0, {len})");
        let p = &self.paragraph_info_list[self.paragraph_by_index(offset)];
        p.paragraph.get_bounding_box(p.to_local(offset)).translate_by(0.0, p.top)
    }

    /// The boxes of the characters of `range` into `array` from `array_start` (left, top, right, bottom each).
    #[track_caller]
    pub fn fill_bounding_boxes(&self, range: TextRange, array: &mut [f32], array_start: usize) {
        let len = self.text_len();
        assert!(range.min() < len && range.max() <= len, "kompoz: range {range:?} is out of bounds [0, {len}]");
        let mut at = array_start;
        self.for_paragraphs_in(range, |p| {
            let start = p.start_index.max(range.min());
            let end = p.end_index.min(range.max());
            if start >= end {
                return;
            }
            let local = TextRange::new(p.to_local(start), p.to_local(end));
            p.paragraph.fill_bounding_boxes(local, array, at);
            let count = p.paragraph.text()[local.start..local.end].chars().count();
            for k in 0..count {
                array[at + k * 4 + 1] += p.top;
                array[at + k * 4 + 3] += p.top;
            }
            at += count * 4;
        });
    }

    /// The paragraph holding `offset`, its end the last one's.
    fn paragraph_for(&self, offset: usize) -> &ParagraphInfo {
        if offset == self.text_len() {
            self.paragraph_info_list.last().expect("a paragraph")
        } else {
            &self.paragraph_info_list[self.paragraph_by_index(offset)]
        }
    }

    #[track_caller]
    fn require_inclusive(&self, offset: usize) {
        let len = self.text_len();
        assert!(offset <= len, "kompoz: offset({offset}) is out of bounds [0, {len}]");
    }

    /// Where a caret at `offset` goes.
    #[track_caller]
    pub fn get_horizontal_position(&self, offset: usize, use_primary_direction: bool) -> f32 {
        self.require_inclusive(offset);
        let p = self.paragraph_for(offset);
        p.paragraph.get_horizontal_position(p.to_local(offset), use_primary_direction)
    }

    /// The direction of the paragraph at `offset`.
    #[track_caller]
    pub fn get_paragraph_direction(&self, offset: usize) -> ResolvedTextDirection {
        self.require_inclusive(offset);
        let p = self.paragraph_for(offset);
        p.paragraph.get_paragraph_direction(p.to_local(offset))
    }

    /// The direction of the bidi run at `offset`.
    #[track_caller]
    pub fn get_bidi_run_direction(&self, offset: usize) -> ResolvedTextDirection {
        self.require_inclusive(offset);
        let p = self.paragraph_for(offset);
        p.paragraph.get_bidi_run_direction(p.to_local(offset))
    }

    /// The word at `offset`.
    #[track_caller]
    pub fn get_word_boundary(&self, offset: usize) -> TextRange {
        self.require_inclusive(offset);
        let p = self.paragraph_for(offset);
        p.to_global_range(p.paragraph.get_word_boundary(p.to_local(offset)), false)
    }

    /// Where a caret at `offset` is drawn.
    #[track_caller]
    pub fn get_cursor_rect(&self, offset: usize) -> Rect {
        self.require_inclusive(offset);
        let p = self.paragraph_for(offset);
        p.paragraph.get_cursor_rect(p.to_local(offset)).translate_by(0.0, p.top)
    }

    /// The line holding `offset` (past the text: the last line).
    pub fn get_line_for_offset(&self, offset: usize) -> usize {
        let p = if offset >= self.text_len() {
            self.paragraph_info_list.last().expect("a paragraph")
        } else {
            &self.paragraph_info_list[self.paragraph_by_index(offset)]
        };
        p.paragraph.get_line_for_offset(p.to_local(offset)) + p.start_line_index
    }

    #[track_caller]
    fn paragraph_of_line(&self, line_index: usize) -> &ParagraphInfo {
        assert!(
            line_index < self.line_count,
            "kompoz: lineIndex({line_index}) is out of bounds [0, {})",
            self.line_count
        );
        let i = self.paragraph_info_list.partition_point(|p| p.end_line_index <= line_index);
        &self.paragraph_info_list[i]
    }

    #[track_caller]
    pub fn get_line_left(&self, line_index: usize) -> f32 {
        let p = self.paragraph_of_line(line_index);
        p.paragraph.get_line_left(line_index - p.start_line_index)
    }
    #[track_caller]
    pub fn get_line_right(&self, line_index: usize) -> f32 {
        let p = self.paragraph_of_line(line_index);
        p.paragraph.get_line_right(line_index - p.start_line_index)
    }
    #[track_caller]
    pub fn get_line_top(&self, line_index: usize) -> f32 {
        let p = self.paragraph_of_line(line_index);
        p.paragraph.get_line_top(line_index - p.start_line_index) + p.top
    }
    #[track_caller]
    pub fn get_line_baseline(&self, line_index: usize) -> f32 {
        let p = self.paragraph_of_line(line_index);
        p.paragraph.get_line_baseline(line_index - p.start_line_index) + p.top
    }
    #[track_caller]
    pub fn get_line_bottom(&self, line_index: usize) -> f32 {
        let p = self.paragraph_of_line(line_index);
        p.paragraph.get_line_bottom(line_index - p.start_line_index) + p.top
    }
    #[track_caller]
    pub fn get_line_height(&self, line_index: usize) -> f32 {
        let p = self.paragraph_of_line(line_index);
        p.paragraph.get_line_height(line_index - p.start_line_index)
    }
    /// How far line `line_index` reaches above its baseline.
    #[track_caller]
    pub fn get_line_ascent(&self, line_index: usize) -> f32 {
        let p = self.paragraph_of_line(line_index);
        p.paragraph.get_line_ascent(line_index - p.start_line_index)
    }
    /// How far line `line_index` reaches below its baseline.
    #[track_caller]
    pub fn get_line_descent(&self, line_index: usize) -> f32 {
        let p = self.paragraph_of_line(line_index);
        p.paragraph.get_line_descent(line_index - p.start_line_index)
    }
    #[track_caller]
    pub fn get_line_width(&self, line_index: usize) -> f32 {
        let p = self.paragraph_of_line(line_index);
        p.paragraph.get_line_width(line_index - p.start_line_index)
    }
    #[track_caller]
    pub fn get_line_start(&self, line_index: usize) -> usize {
        let p = self.paragraph_of_line(line_index);
        p.paragraph.get_line_start(line_index - p.start_line_index) + p.start_index
    }
    #[track_caller]
    pub fn get_line_end(&self, line_index: usize, visible_end: bool) -> usize {
        let p = self.paragraph_of_line(line_index);
        p.paragraph.get_line_end(line_index - p.start_line_index, visible_end) + p.start_index
    }
    #[track_caller]
    pub fn is_line_ellipsized(&self, line_index: usize) -> bool {
        let p = self.paragraph_of_line(line_index);
        p.paragraph.is_line_ellipsized(line_index - p.start_line_index)
    }

    /// Calls `action` with each piece `range` meets (empty ones left out).
    fn for_paragraphs_in(&self, range: TextRange, mut action: impl FnMut(&ParagraphInfo)) {
        let first = self.paragraph_by_index(range.min());
        for p in &self.paragraph_info_list[first..] {
            if p.start_index >= range.max() {
                break;
            }
            if p.start_index == p.end_index {
                continue;
            }
            action(p);
        }
    }

    /// The piece holding `index` (Compose's findParagraphByIndex).
    #[track_caller]
    fn paragraph_by_index(&self, index: usize) -> usize {
        let list = &self.paragraph_info_list;
        let last_end = list.last().map_or(0, |p| p.end_index);
        assert!(index <= last_end, "kompoz: Index {index} should be less or equal than last line's end {last_end}");
        let i = list.partition_point(|p| p.end_index <= index);
        i.min(list.len() - 1)
    }

    /// The piece at `y` (above the first: the first; below the last: the last).
    fn paragraph_by_y(&self, y: f32) -> usize {
        let list = &self.paragraph_info_list;
        if y <= 0.0 {
            return 0;
        }
        if y >= list.last().map_or(0.0, |p| p.bottom) {
            return list.len() - 1;
        }
        list.partition_point(|p| p.bottom <= y).min(list.len() - 1)
    }
}

impl fmt::Debug for MultiParagraph {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MultiParagraph")
            .field("text", &self.intrinsics.annotated_string().text())
            .field("width", &self.width)
            .field("height", &self.height)
            .field("line_count", &self.line_count)
            .finish_non_exhaustive()
    }
}
