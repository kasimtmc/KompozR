//! A paragraph laid out (Compose's Paragraph and ParagraphIntrinsics, Android's AndroidParagraph over
//! StaticLayout and minikin): text in one paragraph style, measured once ([`ParagraphIntrinsics`]: style runs,
//! bidi paragraphs, text shaped once), then laid out in a width ([`Paragraph`]): broken into lines (UAX #14 by the
//! paragraph's strategy, only `\n` ending lines as Android's), cut to `max_lines` and ellipsized as StaticLayout
//! does (the end on the last line, the start or the middle of a single line; the lines that fit a height when
//! the end is ellipsized), each line as tall as the fonts on it make it (Android's fallback line spacing) and its
//! line height and its style ask, aligned, justified, indented; then asked where offsets, lines and positions are
//! (hit testing, carets, selection paths) and drawn (a list of glyph runs built once per layout).
//!
//! Lengths are px; offsets are UTF-8 bytes into the paragraph's text.

mod ellipsis;
mod hit_test;
mod intrinsics;
mod layout;
mod lines;
mod paint;

pub use intrinsics::ParagraphIntrinsics;
pub use paint::{TextPaint, TextPaintOp, TextPaintStyle};

pub(crate) use paint::DrawItem;

use crate::annotated_string::{Annotation, Range as AnnotatedRange};
use crate::font::FontFamilyResolver;
use crate::line_break::{EndHyphenEdit, StartHyphenEdit};
use crate::placeholder::Placeholder;
use crate::style::TextOverflow;
use crate::text_style::TextStyle;
use ellipsis::Ellipsis;
use kompoz_graphics::{Constraints, Density, Rect};
use kompoz_runtime::named;
use lines::LineGeometry;
use std::cell::OnceCell;
use std::fmt;
use std::ops::Range;
use std::rc::Rc;

/// Compose's DefaultMaxLines: as many lines as the text has.
pub const DefaultMaxLines: usize = usize::MAX;

/// A line laid out: its text (its trailing whitespace and the `\n` ending it included; an ellipsized last line
/// all of its paragraph's rest), where its visible text ends, its bidi paragraph, whether a `\n` ended it,
/// whether it starts its paragraph (its first line indent), its hyphen edits, its ellipsis, whether it is
/// justified; its top, bottom and baseline (from the paragraph's top) and its ascent and descent (the descent with
/// the fonts' leading); its horizontal layout, worked out when first asked for.
pub(crate) struct Line {
    pub(crate) range: Range<usize>,
    pub(crate) visible_end: usize,
    pub(crate) para: usize,
    pub(crate) hard_break: bool,
    pub(crate) first_in_paragraph: bool,
    pub(crate) start_hyphen: StartHyphenEdit,
    pub(crate) end_hyphen: EndHyphenEdit,
    pub(crate) ellipsis: Option<Ellipsis>,
    pub(crate) justify: bool,
    pub(crate) top: f32,
    pub(crate) bottom: f32,
    pub(crate) baseline: f32,
    pub(crate) ascent: f32,
    pub(crate) descent: f32,
    pub(crate) geometry: OnceCell<LineGeometry>,
}

/// A paragraph laid out in a width (Compose's Paragraph, Android's AndroidParagraph): its intrinsics laid out in
/// `constraints.max_width` (its width, whatever its lines' are), at most `max_lines` lines, what doesn't fit cut as
/// `overflow` says. Immutable; its draw list is built when first drawn and kept.
pub struct Paragraph {
    pub(crate) intrinsics: Rc<ParagraphIntrinsics>,
    max_lines: usize,
    overflow: TextOverflow,
    constraints: Constraints,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) lines: Vec<Line>,
    did_exceed_max_lines: bool,
    /// Lines were cut off without an ellipsis: what lies past the last one isn't laid out.
    pub(crate) truncated: bool,
    placeholder_rects: OnceCell<Vec<Option<Rect>>>,
    pub(crate) draw_list: OnceCell<Vec<DrawItem>>,
}

impl Paragraph {
    /// `intrinsics` laid out in `constraints` (Compose's Paragraph(paragraphIntrinsics, constraints, maxLines,
    /// overflow)): its width is `constraints.max_width`; a min width or height other than 0, or no line allowed,
    /// panics (as Compose's). With `TextOverflow::Ellipsis`, lines past `constraints.max_height` are left out and
    /// the last one that fits is ellipsized (one at least), as Compose's AndroidParagraph does.
    pub fn new(
        intrinsics: Rc<ParagraphIntrinsics>,
        constraints: Constraints,
        max_lines: usize,
        overflow: TextOverflow,
    ) -> Paragraph {
        assert!(
            constraints.min_width == 0 && constraints.min_height == 0,
            "kompoz: Setting Constraints.minWidth and Constraints.minHeight is not supported, these should be the \
             default zero values instead."
        );
        assert!(max_lines >= 1, "kompoz: maxLines should be greater than 0");
        let width = constraints.max_width as f32;
        let mut laid = layout::lay_out(&intrinsics, width, max_lines, overflow);
        if overflow == TextOverflow::Ellipsis
            && constraints.has_bounded_height()
            && laid.height > constraints.max_height as f32
            && max_lines > 1
        {
            let fit =
                laid.lines.iter().position(|l| l.bottom > constraints.max_height as f32).unwrap_or(laid.lines.len());
            if fit != max_lines {
                laid = layout::lay_out(&intrinsics, width, fit.max(1), overflow);
            }
        }
        Paragraph {
            intrinsics,
            max_lines,
            overflow,
            constraints,
            width,
            height: laid.height,
            lines: laid.lines,
            did_exceed_max_lines: laid.did_exceed_max_lines,
            truncated: laid.truncated,
            placeholder_rects: OnceCell::new(),
            draw_list: OnceCell::new(),
        }
    }

    /// `text` in `style` with `span_styles` and `placeholders`, measured and laid out at once (Compose's
    /// Paragraph(text, style, constraints, density, fontFamilyResolver, spanStyles, placeholders, maxLines,
    /// overflow)); see [`Paragraph::new`].
    #[allow(clippy::too_many_arguments)]
    pub fn of_text(
        text: &str,
        style: &TextStyle,
        constraints: Constraints,
        density: Density,
        font_family_resolver: &Rc<dyn FontFamilyResolver>,
        span_styles: &[AnnotatedRange<crate::span_style::SpanStyle>],
        placeholders: &[AnnotatedRange<Placeholder>],
        max_lines: usize,
        overflow: TextOverflow,
    ) -> Paragraph {
        let annotations: Vec<AnnotatedRange<Annotation>> = span_styles
            .iter()
            .map(|r| AnnotatedRange::tagged(Annotation::SpanStyle(r.item.clone()), r.start, r.end, r.tag.clone()))
            .collect();
        let intrinsics =
            ParagraphIntrinsics::new(text, style, &annotations, placeholders, density, font_family_resolver, true);
        Paragraph::new(Rc::new(intrinsics), constraints, max_lines, overflow)
    }

    /// What it was measured from.
    pub fn intrinsics(&self) -> &Rc<ParagraphIntrinsics> {
        &self.intrinsics
    }
    /// The constraints it was laid out in.
    pub fn constraints(&self) -> Constraints {
        self.constraints
    }
    /// How many lines it may have.
    pub fn max_lines(&self) -> usize {
        self.max_lines
    }
    /// What its text that doesn't fit does.
    pub fn overflow(&self) -> TextOverflow {
        self.overflow
    }
    /// Its width: the max width it was laid out in.
    pub fn width(&self) -> f32 {
        self.width
    }
    /// Its height: its lines' (and the padding a line height style adds above the first and below the last).
    pub fn height(&self) -> f32 {
        self.height
    }
    /// The narrowest it can be without breaking inside a piece between line break opportunities.
    pub fn min_intrinsic_width(&self) -> f32 {
        self.intrinsics.min_intrinsic_width()
    }
    /// The width it takes unwrapped.
    pub fn max_intrinsic_width(&self) -> f32 {
        self.intrinsics.max_intrinsic_width()
    }
    /// Its first line's baseline.
    pub fn first_baseline(&self) -> f32 {
        self.get_line_baseline(0)
    }
    /// Its last line's baseline.
    pub fn last_baseline(&self) -> f32 {
        self.get_line_baseline(self.line_count() - 1)
    }
    /// Whether its text has more lines than `max_lines` (or than fit its height, ellipsized), or was ellipsized
    /// at its end for them.
    pub fn did_exceed_max_lines(&self) -> bool {
        self.did_exceed_max_lines
    }
    /// How many lines it has.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
    /// Where each placeholder is (in its placeholders' order): None for one ellipsized or cut off with the lines
    /// past `max_lines` (Compose's placeholderRects).
    pub fn placeholder_rects(&self) -> &[Option<Rect>] {
        self.placeholder_rects.get_or_init(|| self.compute_placeholder_rects())
    }

    /// Line `index`, which must be one of its lines.
    #[track_caller]
    pub(crate) fn line(&self, index: usize) -> &Line {
        assert!(index < self.lines.len(), "kompoz: lineIndex({index}) is out of bounds [0, {})", self.lines.len());
        &self.lines[index]
    }

    /// Its text.
    pub(crate) fn text(&self) -> &str {
        &self.intrinsics.text
    }
}

/// Its text, width, height and lines.
impl fmt::Debug for Paragraph {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Paragraph")
            .field("text", &self.text())
            .field("width", &self.width)
            .field("height", &self.height)
            .field("line_count", &self.lines.len())
            .finish_non_exhaustive()
    }
}

/// Compose's `Paragraph(text, style, constraints, density, fontFamilyResolver, spanStyles = listOf(), placeholders
/// = listOf(), maxLines = DefaultMaxLines, overflow = TextOverflow.Clip)` by name; plain code calls
/// [`Paragraph::of_text`] or [`Paragraph::new`].
#[named(type)]
#[allow(clippy::too_many_arguments)]
pub fn Paragraph(
    text: &str,
    style: &TextStyle,
    constraints: Constraints,
    density: Density,
    font_family_resolver: &Rc<dyn FontFamilyResolver>,
    #[default(Vec::new())] span_styles: Vec<AnnotatedRange<crate::span_style::SpanStyle>>,
    #[default(Vec::new())] placeholders: Vec<AnnotatedRange<Placeholder>>,
    #[default(DefaultMaxLines)] max_lines: usize,
    #[default(TextOverflow::Clip)] overflow: TextOverflow,
) -> Paragraph {
    Paragraph::of_text(
        text,
        style,
        constraints,
        density,
        font_family_resolver,
        &span_styles,
        &placeholders,
        max_lines,
        overflow,
    )
}
