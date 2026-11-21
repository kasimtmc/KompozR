//! What text was laid out from and how (Compose's TextLayoutInput and TextLayoutResult).

use crate::annotated_string::{AnnotatedString, Range as AnnotatedRange};
use crate::font::FontFamilyResolver;
use crate::multi_paragraph::MultiParagraph;
use crate::placeholder::Placeholder;
use crate::range::TextRange;
use crate::style::{ResolvedTextDirection, TextOverflow};
use crate::text_granularity::{TextGranularity, TextInclusionStrategy};
use crate::text_style::TextStyle;
use kompoz_graphics::{Constraints, Density, IntSize, LayoutDirection, Offset, Path, Rect};
use std::fmt;
use std::rc::Rc;

/// What text was laid out from (Compose's TextLayoutInput): the text, its style, its placeholders, how many lines
/// it may take, whether it wraps, what it does when it doesn't fit, the density, the layout's direction, the
/// resolver of its fonts, the constraints. Equal when all of it is (the resolver the same one).
#[derive(Clone)]
pub struct TextLayoutInput {
    pub text: AnnotatedString,
    pub style: TextStyle,
    pub placeholders: Vec<AnnotatedRange<Placeholder>>,
    pub max_lines: usize,
    pub soft_wrap: bool,
    pub overflow: TextOverflow,
    pub density: Density,
    pub layout_direction: LayoutDirection,
    pub font_family_resolver: Rc<dyn FontFamilyResolver>,
    pub constraints: Constraints,
}

impl TextLayoutInput {
    /// Whether `other` lays text out as this does: the same text, placeholders, lines, wrapping, overflow,
    /// density, direction, resolver and constraints, and a style that lays out alike (Compose's
    /// CacheTextLayoutInput equality: what only paints may differ).
    pub fn lays_out_like(&self, other: &TextLayoutInput) -> bool {
        self.text == other.text
            && self.style.has_same_layout_affecting_attributes(&other.style)
            && self.placeholders == other.placeholders
            && self.max_lines == other.max_lines
            && self.soft_wrap == other.soft_wrap
            && self.overflow == other.overflow
            && self.density == other.density
            && self.layout_direction == other.layout_direction
            && same_resolver(&self.font_family_resolver, &other.font_family_resolver)
            && self.constraints == other.constraints
    }
}

/// The same resolver (the same object).
pub(crate) fn same_resolver(a: &Rc<dyn FontFamilyResolver>, b: &Rc<dyn FontFamilyResolver>) -> bool {
    std::ptr::eq(Rc::as_ptr(a) as *const (), Rc::as_ptr(b) as *const ())
}

impl PartialEq for TextLayoutInput {
    fn eq(&self, other: &TextLayoutInput) -> bool {
        self.text == other.text
            && self.style == other.style
            && self.placeholders == other.placeholders
            && self.max_lines == other.max_lines
            && self.soft_wrap == other.soft_wrap
            && self.overflow == other.overflow
            && self.density == other.density
            && self.layout_direction == other.layout_direction
            && same_resolver(&self.font_family_resolver, &other.font_family_resolver)
            && self.constraints == other.constraints
    }
}

impl fmt::Debug for TextLayoutInput {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TextLayoutInput")
            .field("text", &self.text)
            .field("style", &self.style)
            .field("placeholders", &self.placeholders)
            .field("max_lines", &self.max_lines)
            .field("soft_wrap", &self.soft_wrap)
            .field("overflow", &self.overflow)
            .field("density", &self.density)
            .field("layout_direction", &self.layout_direction)
            .field("constraints", &self.constraints)
            .finish_non_exhaustive()
    }
}

/// Text laid out (Compose's TextLayoutResult): what it was laid out from, its paragraphs laid out, and its size
/// (whole pixels, within the constraints). Cheap to clone (its layout is shared); equal to another made from the
/// same input and layout at the same size.
#[derive(Clone)]
pub struct TextLayoutResult {
    pub layout_input: Rc<TextLayoutInput>,
    pub multi_paragraph: Rc<MultiParagraph>,
    pub size: IntSize,
    first_baseline: f32,
    last_baseline: f32,
}

impl TextLayoutResult {
    pub fn new(layout_input: Rc<TextLayoutInput>, multi_paragraph: Rc<MultiParagraph>, size: IntSize) -> Self {
        let (first_baseline, last_baseline) = (multi_paragraph.first_baseline(), multi_paragraph.last_baseline());
        TextLayoutResult { layout_input, multi_paragraph, size, first_baseline, last_baseline }
    }

    /// Its size (Compose's `size` property: the field read through a getter, as Kotlin reads it).
    /// Kotlin: `size: IntSize`
    pub fn size(&self) -> IntSize {
        self.size
    }

    /// The first line's baseline.
    pub fn first_baseline(&self) -> f32 {
        self.first_baseline
    }
    /// The last line's baseline.
    pub fn last_baseline(&self) -> f32 {
        self.last_baseline
    }
    /// Whether it is cut off below: more lines than `max_lines`, or taller than its size.
    pub fn did_overflow_height(&self) -> bool {
        self.multi_paragraph.did_exceed_max_lines() || (self.size.height as f32) < self.multi_paragraph.height()
    }
    /// Whether it is wider than its size.
    pub fn did_overflow_width(&self) -> bool {
        (self.size.width as f32) < self.multi_paragraph.width()
    }
    /// Whether it is cut off either way.
    pub fn has_visual_overflow(&self) -> bool {
        self.did_overflow_width() || self.did_overflow_height()
    }
    /// Where each placeholder is (None where it was cut off or ellipsized).
    pub fn placeholder_rects(&self) -> &[Option<Rect>] {
        self.multi_paragraph.placeholder_rects()
    }
    pub fn line_count(&self) -> usize {
        self.multi_paragraph.line_count()
    }
    /// Where line `line_index` starts.
    #[track_caller]
    pub fn get_line_start(&self, line_index: usize) -> usize {
        self.multi_paragraph.get_line_start(line_index)
    }
    /// Where line `line_index` ends: past its trailing whitespace and line feed (the text's end for a line
    /// ellipsized at its end), or with `visible_end` where its visible text ends.
    #[track_caller]
    pub fn get_line_end(&self, line_index: usize, visible_end: bool) -> usize {
        self.multi_paragraph.get_line_end(line_index, visible_end)
    }
    #[track_caller]
    pub fn is_line_ellipsized(&self, line_index: usize) -> bool {
        self.multi_paragraph.is_line_ellipsized(line_index)
    }
    #[track_caller]
    pub fn get_line_top(&self, line_index: usize) -> f32 {
        self.multi_paragraph.get_line_top(line_index)
    }
    #[track_caller]
    pub fn get_line_baseline(&self, line_index: usize) -> f32 {
        self.multi_paragraph.get_line_baseline(line_index)
    }
    #[track_caller]
    pub fn get_line_bottom(&self, line_index: usize) -> f32 {
        self.multi_paragraph.get_line_bottom(line_index)
    }
    /// How far line `line_index` reaches above its baseline.
    #[track_caller]
    pub fn get_line_ascent(&self, line_index: usize) -> f32 {
        self.multi_paragraph.get_line_ascent(line_index)
    }
    /// How far line `line_index` reaches below its baseline (the fonts' leading included).
    #[track_caller]
    pub fn get_line_descent(&self, line_index: usize) -> f32 {
        self.multi_paragraph.get_line_descent(line_index)
    }
    #[track_caller]
    pub fn get_line_left(&self, line_index: usize) -> f32 {
        self.multi_paragraph.get_line_left(line_index)
    }
    #[track_caller]
    pub fn get_line_right(&self, line_index: usize) -> f32 {
        self.multi_paragraph.get_line_right(line_index)
    }
    /// The line holding `offset`.
    pub fn get_line_for_offset(&self, offset: usize) -> usize {
        self.multi_paragraph.get_line_for_offset(offset)
    }
    /// The line at `vertical` px from the top.
    pub fn get_line_for_vertical_position(&self, vertical: f32) -> usize {
        self.multi_paragraph.get_line_for_vertical_position(vertical)
    }
    /// Where a caret at `offset` goes.
    #[track_caller]
    pub fn get_horizontal_position(&self, offset: usize, use_primary_direction: bool) -> f32 {
        self.multi_paragraph.get_horizontal_position(offset, use_primary_direction)
    }
    #[track_caller]
    pub fn get_paragraph_direction(&self, offset: usize) -> ResolvedTextDirection {
        self.multi_paragraph.get_paragraph_direction(offset)
    }
    #[track_caller]
    pub fn get_bidi_run_direction(&self, offset: usize) -> ResolvedTextDirection {
        self.multi_paragraph.get_bidi_run_direction(offset)
    }
    /// The offset nearest `position`.
    pub fn get_offset_for_position(&self, position: Offset) -> usize {
        self.multi_paragraph.get_offset_for_position(position)
    }
    /// The box of the character at `offset`.
    #[track_caller]
    pub fn get_bounding_box(&self, offset: usize) -> Rect {
        self.multi_paragraph.get_bounding_box(offset)
    }
    /// The word at `offset`.
    #[track_caller]
    pub fn get_word_boundary(&self, offset: usize) -> TextRange {
        self.multi_paragraph.get_word_boundary(offset)
    }
    /// Where a caret at `offset` is drawn.
    #[track_caller]
    pub fn get_cursor_rect(&self, offset: usize) -> Rect {
        self.multi_paragraph.get_cursor_rect(offset)
    }
    /// The area selecting `start..end` highlights.
    #[track_caller]
    pub fn get_path_for_range(&self, start: usize, end: usize) -> Path {
        self.multi_paragraph.get_path_for_range(start, end)
    }
    /// The range of text `rect` takes (Compose's MultiParagraph.getRangeForRect).
    pub fn get_range_for_rect(
        &self,
        rect: Rect,
        granularity: TextGranularity,
        inclusion_strategy: &TextInclusionStrategy,
    ) -> TextRange {
        self.multi_paragraph.get_range_for_rect(rect, granularity, inclusion_strategy)
    }

    /// The same layout from another input and at another size (Compose's copy).
    pub fn copy(&self, layout_input: Rc<TextLayoutInput>, size: IntSize) -> TextLayoutResult {
        TextLayoutResult { layout_input, size, ..self.clone() }
    }
}

impl PartialEq for TextLayoutResult {
    fn eq(&self, o: &TextLayoutResult) -> bool {
        (Rc::ptr_eq(&self.layout_input, &o.layout_input) || *self.layout_input == *o.layout_input)
            && Rc::ptr_eq(&self.multi_paragraph, &o.multi_paragraph)
            && self.size == o.size
    }
}

impl fmt::Debug for TextLayoutResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TextLayoutResult")
            .field("layout_input", &self.layout_input)
            .field("multi_paragraph", &self.multi_paragraph)
            .field("size", &self.size)
            .field("first_baseline", &self.first_baseline)
            .field("last_baseline", &self.last_baseline)
            .finish()
    }
}
