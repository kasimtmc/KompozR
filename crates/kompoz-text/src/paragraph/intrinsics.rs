//! A paragraph's text measured once (Compose's ParagraphIntrinsics, Android's AndroidParagraphIntrinsics with its
//! LayoutIntrinsics): its style runs resolved (each span's attributes over the text's, faces resolved), its
//! bidirectional paragraphs (a `\n` ends one, as StaticLayout's), and its text shaped once — each style run whose
//! layout attributes differ, cut by bidi level, face and script, shaped as one run (D6) — with the advance before
//! every byte summed up, so a stretch's width is two lookups and only a cut where shaping changes (HarfBuzz's
//! unsafe-to-break) shapes again. Its min and max intrinsic widths are Compose's.

use crate::annotated_string::{Annotation, Range as AnnotatedRange};
use crate::bidi::BidiParagraph;
use crate::draw::TextRunStyle;
use crate::font::{FontFamilyResolver, FontFeature, Typeface, parse_font_feature_settings};
use crate::intl::{Locale, LocaleList};
use crate::line_break::{EndHyphenEdit, LineMeasure, StartHyphenEdit, TabStops, break_opportunities};
use crate::placeholder::{Placeholder, PlaceholderVerticalAlign};
use crate::platform_style::PlatformParagraphStyle;
use crate::segment::{is_common_or_inherited, script_tag};
use crate::shape::{FontBackend, FontInstance, FontMetrics, FontRunRequest, ShapeRequest, ShapedGlyph, ShapedRun};
use crate::span_style::{SpanStyle, TextForegroundStyle};
use crate::style::{
    BaselineShift, Hyphens, LineBreak, LineHeightStyle, TextAlign, TextDecoration, TextIndent, TextMotion,
};
use crate::text_style::TextStyle;
use kompoz_graphics::{Color, Density, Shadow, TextUnit};
use kompoz_runtime::State;
use std::cell::OnceCell;
use std::collections::HashMap;
use std::fmt;
use std::ops::Range;
use std::rc::Rc;

/// A placeholder's room, px (Compose's PlaceholderSpan: its width and height rounded up to whole pixels), where it
/// sits, and its index among the paragraph's placeholders.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct PlaceholderRoom {
    pub(crate) index: usize,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) align: PlaceholderVerticalAlign,
}

/// What a stretch of text under one set of spans is laid out and painted in: the style it is drawn in, the face
/// it resolved to, its features and locales, how far its baseline shifts (a share of its ascent), what its spans
/// paint it with themselves (a foreground, a shadow, a decoration, a draw style of their own, which a draw call's
/// own don't replace), its background, the room it takes instead of its text if it is a placeholder, its face as
/// a font at its size and that face's metrics.
pub(crate) struct RunStyle {
    pub(crate) drawn: Rc<TextRunStyle>,
    pub(crate) typeface: Typeface,
    pub(crate) features: Rc<[FontFeature]>,
    pub(crate) locales: LocaleList,
    pub(crate) baseline_shift: f32,
    pub(crate) foreground: TextForegroundStyle,
    pub(crate) shadow: Option<Shadow>,
    pub(crate) decoration: Option<TextDecoration>,
    pub(crate) own_draw_style: bool,
    pub(crate) background: Color,
    pub(crate) placeholder: Option<PlaceholderRoom>,
    pub(crate) primary: FontInstance,
    pub(crate) metrics: FontMetrics,
}

impl RunStyle {
    /// Whether text in the two is shaped alike (the same face, size, spacing, stretch, features and language):
    /// neighbours that are become one shaping run.
    fn shapes_like(&self, o: &RunStyle) -> bool {
        self.placeholder.is_none()
            && o.placeholder.is_none()
            && self.typeface == o.typeface
            && self.drawn.font_size == o.drawn.font_size
            && self.drawn.letter_spacing == o.drawn.letter_spacing
            && self.drawn.scale_x == o.drawn.scale_x
            && self.features == o.features
            && self.locales.get(0) == o.locales.get(0)
    }

    /// The language its text is shaped in.
    pub(crate) fn language(&self) -> Option<&Locale> {
        self.locales.get(0)
    }

    /// How far its baseline moves down, px (Android's BaselineShiftSpan: its ascent times the shift, rounded up;
    /// negative: up).
    pub(crate) fn shift_y(&self) -> f32 {
        if self.baseline_shift == 0.0 { 0.0 } else { (-self.metrics.ascent * self.baseline_shift).ceil() }
    }
}

/// A stretch of text under one set of spans (each span's and placeholder's ends cut one).
pub(crate) struct StyleRun {
    pub(crate) range: Range<usize>,
    pub(crate) style: Rc<RunStyle>,
}

/// A bidirectional paragraph: text up to and including a `\n` (the text's last one up to its end; an empty one
/// after a `\n` that ends the text).
pub(crate) struct BidiPara {
    pub(crate) range: Range<usize>,
    pub(crate) bidi: BidiParagraph,
}

/// Text shaped as one run: one face, one direction, one script, the shaping parameters of a style run (`run`, the
/// first of the style runs it spans), its glyphs in visual order (none for a placeholder, which takes its room)
/// and their advances' sum.
pub(crate) struct Segment {
    pub(crate) range: Range<usize>,
    pub(crate) run: usize,
    pub(crate) font: FontInstance,
    pub(crate) metrics: FontMetrics,
    pub(crate) rtl: bool,
    pub(crate) script: Option<[u8; 4]>,
    pub(crate) glyphs: Vec<ShapedGlyph>,
    pub(crate) advance: f32,
    pub(crate) placeholder: bool,
}

/// A cluster starts at the offset (or a segment, or the text, ends there).
const BOUNDARY: u8 = 1;
/// Shaping changes if the text is cut there (HarfBuzz's UNSAFE_TO_BREAK on the cluster's glyph).
const UNSAFE: u8 = 2;

/// A paragraph's settings, in px (em lengths of the paragraph's font size): its alignment, line breaking, hyphens, locale, indents (rounded up, as Android's LeadingMarginSpan), line height and its style,
/// whether its first and last lines are padded to the fonts' tallest glyphs, how its text moves.
#[derive(Clone, Debug)]
pub(crate) struct ParagraphSettings {
    pub(crate) text_align: TextAlign,
    pub(crate) line_break: LineBreak,
    pub(crate) hyphens: Hyphens,
    pub(crate) locale: Locale,
    pub(crate) first_indent: f32,
    pub(crate) rest_indent: f32,
    pub(crate) line_height: Option<f32>,
    pub(crate) line_height_style: Option<LineHeightStyle>,
    pub(crate) include_font_padding: bool,
    pub(crate) text_motion: TextMotion,
}

/// A paragraph's text measured once (Compose's ParagraphIntrinsics): its text, style (its defaults resolved, as
/// TextMeasurer and MultiParagraph give it), span styles and other annotations, placeholders, the density and
/// the resolver its faces come from; its text shaped and its intrinsic widths. A [`Paragraph`] lays it out in a
/// width; many may share one.
///
/// [`Paragraph`]: super::Paragraph
pub struct ParagraphIntrinsics {
    pub(crate) text: Rc<str>,
    pub(crate) style: TextStyle,
    annotations: Vec<AnnotatedRange<Annotation>>,
    placeholders: Vec<AnnotatedRange<Placeholder>>,
    density: Density,
    font_family_resolver: Rc<dyn FontFamilyResolver>,
    soft_wrap: bool,
    pub(crate) backend: Rc<dyn FontBackend>,
    pub(crate) settings: ParagraphSettings,
    pub(crate) base: Rc<RunStyle>,
    pub(crate) runs: Vec<StyleRun>,
    pub(crate) paragraphs: Vec<BidiPara>,
    pub(crate) segments: Vec<Segment>,
    prefix: Vec<f32>,
    flags: Vec<u8>,
    letter_spaced: bool,
    typefaces: Vec<(State<Typeface>, Typeface)>,
    min_intrinsic_width: OnceCell<f32>,
    max_intrinsic_width: OnceCell<f32>,
}

/// A text length in px: sp through the density, em of `em_of` px.
pub(crate) fn text_unit_px(unit: TextUnit, em_of: f32, density: Density) -> f32 {
    if unit.is_em() { unit.value() * em_of } else { density.text_to_px(unit) }
}

impl ParagraphIntrinsics {
    /// `text` in `style` with the span styles among `annotations` over it, `placeholders` taking their room
    /// (offsets into `text`), measured through `font_family_resolver`'s backend (Compose's ParagraphIntrinsics;
    /// `soft_wrap` is carried, as Compose's, for whoever lays it out). Faces are resolved now (reading their
    /// states: a measure or composition reading them lays out again when an async font loads).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        text: &str,
        style: &TextStyle,
        annotations: &[AnnotatedRange<Annotation>],
        placeholders: &[AnnotatedRange<Placeholder>],
        density: Density,
        font_family_resolver: &Rc<dyn FontFamilyResolver>,
        soft_wrap: bool,
    ) -> ParagraphIntrinsics {
        let text: Rc<str> = Rc::from(text);
        let backend = font_family_resolver.backend();
        let mut typefaces: Vec<(State<Typeface>, Typeface)> = Vec::new();
        let base_drawn = TextRunStyle::resolve(style, &[], density);
        let settings = settings_of(style, base_drawn.font_size, density);
        let mut builder = StyleBuilder {
            style,
            density,
            resolver: font_family_resolver,
            backend: &*backend,
            typefaces: &mut typefaces,
            resolved: HashMap::new(),
        };
        let base = Rc::new(builder.run_style(&[], None));
        let runs = builder.style_runs(&text, annotations, placeholders, &base);
        let paragraphs = bidi_paragraphs(&text, style, &settings);
        let letter_spaced = base_drawn.letter_spacing != 0.0
            || annotations
                .iter()
                .any(|a| matches!(&a.item, Annotation::SpanStyle(s) if s.letter_spacing.is_specified()));
        let mut p = ParagraphIntrinsics {
            text,
            style: style.clone(),
            annotations: annotations.to_vec(),
            placeholders: placeholders.to_vec(),
            density,
            font_family_resolver: font_family_resolver.clone(),
            soft_wrap,
            backend,
            settings,
            base,
            runs,
            paragraphs,
            segments: Vec::new(),
            prefix: Vec::new(),
            flags: Vec::new(),
            letter_spaced,
            typefaces,
            min_intrinsic_width: OnceCell::new(),
            max_intrinsic_width: OnceCell::new(),
        };
        p.shape();
        p
    }

    /// Its text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Its style.
    pub fn style(&self) -> &TextStyle {
        &self.style
    }
    /// Its annotations (span styles among them).
    pub fn annotations(&self) -> &[AnnotatedRange<Annotation>] {
        &self.annotations
    }
    /// Its placeholders.
    pub fn placeholders(&self) -> &[AnnotatedRange<Placeholder>] {
        &self.placeholders
    }
    pub fn density(&self) -> Density {
        self.density
    }
    pub fn font_family_resolver(&self) -> &Rc<dyn FontFamilyResolver> {
        &self.font_family_resolver
    }
    pub fn soft_wrap(&self) -> bool {
        self.soft_wrap
    }

    /// Whether a face it was laid out in has changed since (an async font loaded): what was laid out with it is
    /// stale (Compose's hasStaleResolvedFonts).
    pub fn has_stale_resolved_fonts(&self) -> bool {
        self.typefaces.iter().any(|(state, resolved)| state.peek() != *resolved)
    }

    /// The narrowest it can be laid out without breaking inside its pieces (Compose's minIntrinsicWidth, Android's
    /// LayoutIntrinsics): of the ten longest stretches between line break opportunities (by UTF-16 length, as
    /// Java's strings count), the widest as laid out on its own — its trailing whitespace and first line indent
    /// included.
    pub fn min_intrinsic_width(&self) -> f32 {
        *self.min_intrinsic_width.get_or_init(|| self.compute_min_intrinsic_width())
    }

    /// The width it takes laid out without wrapping (Compose's maxIntrinsicWidth, Android's LayoutIntrinsics): its
    /// widest `\n` paragraph, trailing whitespace and first line indent included, rounded up to a whole pixel, and
    /// half a pixel more if letter spacing is in it (so that StaticLayout doesn't wrap it again in that width).
    pub fn max_intrinsic_width(&self) -> f32 {
        *self.max_intrinsic_width.get_or_init(|| self.compute_max_intrinsic_width())
    }

    fn compute_min_intrinsic_width(&self) -> f32 {
        let text = &*self.text;
        let points = break_opportunities(text, self.settings.line_break, &self.settings.locale);
        let unbreakable: Vec<Range<usize>> =
            self.placeholders.iter().map(|p| p.start.min(text.len())..p.end.min(text.len())).collect();
        // A min heap of the ten longest: the shortest of them is replaced by a longer one.
        let mut longest: Vec<(usize, usize, usize)> = Vec::new();
        let mut start = 0;
        for p in points {
            if !p.mandatory
                && p.offset < text.len()
                && unbreakable.iter().any(|r| r.start < p.offset && p.offset < r.end)
            {
                continue;
            }
            let end = p.offset;
            let length = text[start..end].encode_utf16().count();
            if longest.len() < 10 {
                longest.push((length, start, end));
            } else if let Some(min) = longest.iter().enumerate().min_by_key(|(_, l)| l.0).map(|(i, _)| i)
                && longest[min].0 < length
            {
                longest[min] = (length, start, end);
            }
            start = end;
        }
        longest.iter().map(|&(_, s, e)| self.desired_width(s..e)).fold(0.0, f32::max)
    }

    fn compute_max_intrinsic_width(&self) -> f32 {
        let width = self.desired_width(0..self.text.len()).ceil();
        if self.letter_spaced && width != 0.0 { width + 0.5 } else { width }
    }

    /// The width `range` takes laid out without wrapping (Android's Layout.getDesiredWidth): its widest `\n`
    /// paragraph, whitespace and tabs included, plus the first line indent.
    fn desired_width(&self, range: Range<usize>) -> f32 {
        let text = &self.text[range.clone()];
        let mut widest: f32 = 0.0;
        let mut start = range.start;
        for piece in text.split('\n') {
            let end = start + piece.len();
            widest = widest.max(self.tabbed_width(start..end) + self.settings.first_indent);
            start = end + 1;
        }
        widest
    }

    /// How wide `range` is laid out from a line's start, its tabs at their stops.
    pub(crate) fn tabbed_width(&self, range: Range<usize>) -> f32 {
        let stops = TabStops::default();
        let mut x = 0.0;
        let mut from = range.start;
        for (i, _) in self.text[range.clone()].match_indices('\t') {
            let tab = range.start + i;
            x += self.width(from..tab);
            x = stops.next_tab(x);
            from = tab + 1;
        }
        x + self.width(from..range.end)
    }

    // ---- Shaping --------------------------------------------------------------------------------------------

    /// Shapes its text: each bidi paragraph's style runs, those shaped alike together, by level, face and script.
    fn shape(&mut self) {
        let text = self.text.clone();
        let mut segments = Vec::new();
        for para in &self.paragraphs {
            let mut i = self.runs.partition_point(|r| r.range.end <= para.range.start);
            while i < self.runs.len() && self.runs[i].range.start < para.range.end.max(para.range.start + 1) {
                let first = i;
                let style = self.runs[i].style.clone();
                let mut end = self.runs[i].range.end;
                while i + 1 < self.runs.len()
                    && self.runs[i + 1].range.start < para.range.end
                    && style.shapes_like(&self.runs[i + 1].style)
                {
                    i += 1;
                    end = self.runs[i].range.end;
                }
                i += 1;
                let start = self.runs[first].range.start.max(para.range.start);
                let end = end.min(para.range.end);
                if start >= end {
                    continue;
                }
                if let Some(room) = style.placeholder {
                    let level = para.bidi.level_at(start - para.range.start);
                    segments.push(Segment {
                        range: start..end,
                        run: first,
                        font: style.primary.clone(),
                        metrics: style.metrics,
                        rtl: level % 2 == 1,
                        script: None,
                        glyphs: Vec::new(),
                        advance: room.width,
                        placeholder: true,
                    });
                    continue;
                }
                for level_run in para.bidi.level_runs(start - para.range.start..end - para.range.start) {
                    let range = level_run.range.start + para.range.start..level_run.range.end + para.range.start;
                    self.shape_level_run(&text, range, first, &style, level_run.is_rtl(), &mut segments);
                }
            }
        }
        segments.sort_by_key(|s| s.range.start);
        self.segments = segments;
        self.sum_advances();
    }

    /// Shapes `range` (one level, one style) face by face and script by script.
    fn shape_level_run(
        &self,
        text: &str,
        range: Range<usize>,
        run: usize,
        style: &RunStyle,
        rtl: bool,
        out: &mut Vec<Segment>,
    ) {
        let font_runs = self.backend.font_runs(&FontRunRequest {
            text,
            range: range.clone(),
            typeface: &style.typeface,
            size: style.drawn.font_size,
            density: self.density,
            locales: &style.locales,
        });
        for font_run in font_runs {
            let metrics = self.backend.metrics(&font_run.font);
            for (piece, script) in script_runs(text, font_run.range.clone()) {
                let shaped = self.shape_piece(text, piece.clone(), &font_run.font, rtl, script, style);
                out.push(Segment {
                    range: piece,
                    run,
                    font: font_run.font.clone(),
                    metrics,
                    rtl,
                    script,
                    advance: shaped.advance,
                    glyphs: shaped.glyphs,
                    placeholder: false,
                });
            }
        }
    }

    /// `range` of `text` shaped in `font` (its context the text around it), line feeds and carriage returns
    /// taking no room (they end lines; they aren't drawn).
    fn shape_piece(
        &self,
        text: &str,
        range: Range<usize>,
        font: &FontInstance,
        rtl: bool,
        script: Option<[u8; 4]>,
        style: &RunStyle,
    ) -> ShapedRun {
        let mut shaped = self.backend.shape(&ShapeRequest {
            text,
            range,
            font,
            rtl,
            script,
            language: style.language(),
            features: &style.features,
            letter_spacing: style.drawn.letter_spacing,
            scale_x: style.drawn.scale_x,
        });
        if shaped.glyphs.iter().any(|g| matches!(text.as_bytes().get(g.cluster), Some(b'\n' | b'\r'))) {
            for g in &mut shaped.glyphs {
                if matches!(text.as_bytes().get(g.cluster), Some(b'\n' | b'\r')) {
                    g.x_advance = 0.0;
                }
            }
            shaped.advance = shaped.glyphs.iter().map(|g| g.x_advance).sum();
        }
        shaped
    }

    /// The advance before every byte, and where clusters start and shaping may be cut.
    fn sum_advances(&mut self) {
        let len = self.text.len();
        let mut at = vec![0.0f32; len + 1];
        let mut flags = vec![0u8; len + 1];
        flags[0] |= BOUNDARY;
        flags[len] |= BOUNDARY;
        for seg in &self.segments {
            flags[seg.range.start] |= BOUNDARY;
            flags[seg.range.end] |= BOUNDARY;
            if seg.placeholder {
                at[seg.range.start] += seg.advance;
                continue;
            }
            for g in &seg.glyphs {
                at[g.cluster] += g.x_advance;
                flags[g.cluster] |= BOUNDARY;
                if g.unsafe_to_break && g.cluster != seg.range.start {
                    flags[g.cluster] |= UNSAFE;
                }
            }
        }
        let mut prefix = Vec::with_capacity(len + 1);
        let mut sum = 0.0;
        prefix.push(0.0);
        for a in &at[..len] {
            sum += a;
            prefix.push(sum);
        }
        self.prefix = prefix;
        self.flags = flags;
    }

    /// Whether the text may be cut at `offset` keeping both sides' glyphs as shaped.
    pub(crate) fn is_safe_cut(&self, offset: usize) -> bool {
        self.flags.get(offset).is_some_and(|&f| f & BOUNDARY != 0 && f & UNSAFE == 0)
    }

    /// The index of the segment holding `offset` (the last one at the text's end).
    pub(crate) fn segment_at(&self, offset: usize) -> Option<usize> {
        let i = self.segments.partition_point(|s| s.range.end <= offset);
        if i < self.segments.len() { Some(i) } else { self.segments.len().checked_sub(1) }
    }

    /// The index of the style run holding `offset` (the last one at the text's end).
    pub(crate) fn run_at(&self, offset: usize) -> usize {
        let i = self.runs.partition_point(|r| r.range.end <= offset);
        i.min(self.runs.len().saturating_sub(1))
    }

    /// The style at `offset`.
    pub(crate) fn style_at(&self, offset: usize) -> &Rc<RunStyle> {
        self.runs.get(self.run_at(offset)).map_or(&self.base, |r| &r.style)
    }

    /// How wide `range` is as shaped (cluster boundaries where it may be cut keep the paragraph's glyphs; a cut
    /// where shaping would change shapes that piece again, with its context).
    pub(crate) fn width(&self, range: Range<usize>) -> f32 {
        if range.start >= range.end {
            return 0.0;
        }
        if self.is_safe_cut(range.start) && self.is_safe_cut(range.end) {
            return self.prefix[range.end] - self.prefix[range.start];
        }
        let mut width = 0.0;
        let mut i = self.segments.partition_point(|s| s.range.end <= range.start);
        while let Some(seg) = self.segments.get(i)
            && seg.range.start < range.end
        {
            let (a, b) = (range.start.max(seg.range.start), range.end.min(seg.range.end));
            width += if a == seg.range.start && b == seg.range.end {
                seg.advance
            } else if seg.placeholder {
                if a == seg.range.start { seg.advance } else { 0.0 }
            } else if self.is_safe_cut(a) && self.is_safe_cut(b) {
                self.prefix[b] - self.prefix[a]
            } else {
                self.reshape(i, a..b).advance
            };
            i += 1;
        }
        width
    }

    /// `range` of segment `segment` shaped again (with the text around it as its context).
    pub(crate) fn reshape(&self, segment: usize, range: Range<usize>) -> ShapedRun {
        let seg = &self.segments[segment];
        let style = &self.runs[seg.run].style;
        self.shape_piece(&self.text, range, &seg.font, seg.rtl, seg.script, style)
    }

    /// `range` of segment `segment` shaped with hyphen edits at its ends (minikin's: the hyphen in the face at the
    /// cut, the edited text shaped on its own); the glyphs' clusters are offsets into the paragraph's text, an
    /// edit's glyphs the cluster of the character they stand at.
    pub(crate) fn shape_with_edits(
        &self,
        segment: usize,
        range: Range<usize>,
        start: StartHyphenEdit,
        end: EndHyphenEdit,
    ) -> ShapedRun {
        let seg = &self.segments[segment];
        let style = &self.runs[seg.run].style;
        let mut middle = range.clone();
        let last_char =
            self.text[range.clone()].char_indices().next_back().map_or(range.start, |(i, _)| range.start + i);
        if end == EndHyphenEdit::ReplaceWithHyphen {
            middle.end = last_char;
        }
        let prefix = start.text();
        let edited = format!("{prefix}{}{}", &self.text[middle.clone()], end.text());
        let mut shaped = self.backend.shape(&ShapeRequest {
            text: &edited,
            range: 0..edited.len(),
            font: &seg.font,
            rtl: seg.rtl,
            script: seg.script,
            language: style.language(),
            features: &style.features,
            letter_spacing: style.drawn.letter_spacing,
            scale_x: style.drawn.scale_x,
        });
        let middle_end = prefix.len() + middle.len();
        for g in &mut shaped.glyphs {
            g.cluster = if g.cluster < prefix.len() {
                range.start
            } else if g.cluster < middle_end {
                middle.start + g.cluster - prefix.len()
            } else {
                last_char
            };
        }
        shaped
    }

    /// How wide hyphen edits make `range`: its width, its last character replaced or a hyphen added after it, a
    /// hyphen or joiner before it.
    pub(crate) fn width_with_edits(&self, range: Range<usize>, start: StartHyphenEdit, end: EndHyphenEdit) -> f32 {
        if start == StartHyphenEdit::NoEdit && end == EndHyphenEdit::NoEdit {
            return self.width(range);
        }
        let mut width = self.width(range.clone());
        let first = self.segment_at(range.start);
        let last = self.segment_at(range.end.saturating_sub(1).max(range.start));
        if start != StartHyphenEdit::NoEdit
            && let Some(i) = first
        {
            let seg_end = self.segments[i].range.end.min(range.end);
            let plain = self.width(range.start..seg_end);
            width += self.shape_with_edits(i, range.start..seg_end, start, EndHyphenEdit::NoEdit).advance - plain;
        }
        if end != EndHyphenEdit::NoEdit
            && let Some(i) = last
        {
            let seg_start = self.segments[i].range.start.max(range.start);
            let plain = self.width(seg_start..range.end);
            width += self.shape_with_edits(i, seg_start..range.end, StartHyphenEdit::NoEdit, end).advance - plain;
        }
        width
    }

    /// The largest font size of `range`'s text times its stretch.
    pub(crate) fn font_size(&self, range: Range<usize>) -> f32 {
        let i = self.runs.partition_point(|r| r.range.end <= range.start);
        self.runs[i.min(self.runs.len().saturating_sub(1))..]
            .iter()
            .take_while(|r| r.range.start < range.end.max(range.start + 1))
            .map(|r| r.style.drawn.font_size * r.style.drawn.scale_x)
            .fold(self.base.drawn.font_size * self.base.drawn.scale_x, f32::max)
    }

    /// The ranges no line may break inside (placeholders).
    pub(crate) fn unbreakable(&self) -> Vec<Range<usize>> {
        self.runs
            .iter()
            .filter(|r| r.style.placeholder.is_some() && !r.range.is_empty())
            .map(|r| r.range.clone())
            .collect()
    }
}

/// Its text, style and placeholders.
impl fmt::Debug for ParagraphIntrinsics {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ParagraphIntrinsics")
            .field("text", &self.text)
            .field("style", &self.style)
            .field("placeholders", &self.placeholders)
            .finish_non_exhaustive()
    }
}

/// A measure for the line breaker over `text[origin..]` of a paragraph (offsets relative to `origin`).
pub(crate) struct ParagraphMeasure<'a> {
    pub(crate) paragraph: &'a ParagraphIntrinsics,
    pub(crate) origin: usize,
}

impl LineMeasure for ParagraphMeasure<'_> {
    fn width(&self, range: Range<usize>) -> f32 {
        self.paragraph.width(range.start + self.origin..range.end + self.origin)
    }
    fn width_with_hyphens(&self, range: Range<usize>, start: StartHyphenEdit, end: EndHyphenEdit) -> f32 {
        self.paragraph.width_with_edits(range.start + self.origin..range.end + self.origin, start, end)
    }
    fn font_size(&self, range: Range<usize>) -> f32 {
        self.paragraph.font_size(range.start + self.origin..range.end + self.origin)
    }
}

/// A paragraph style's settings in px (Compose's createCharSequence and AndroidParagraph): em lengths of the
/// paragraph's font size, indents rounded up, the line height style Compose's default where a line height has
/// none (unless the fonts' padding is kept and no style given: then the plain line height span).
fn settings_of(style: &TextStyle, font_size: f32, density: Density) -> ParagraphSettings {
    let indent = style.text_indent.unwrap_or(TextIndent::None);
    let indent_px = |u: TextUnit| if u.is_unspecified() { 0.0 } else { text_unit_px(u, font_size, density).ceil() };
    let include_font_padding = style
        .platform_style
        .and_then(|p| p.paragraph_style)
        .unwrap_or(PlatformParagraphStyle::Default)
        .include_font_padding;
    ParagraphSettings {
        text_align: style.text_align.take_or_else(|| TextAlign::Start),
        line_break: style.line_break.take_or_else(|| LineBreak::Simple),
        hyphens: style.hyphens.take_or_else(|| Hyphens::None),
        locale: style.locale_list.as_ref().map_or_else(Locale::current, |l| l.first_or_current()),
        first_indent: indent_px(indent.first_line),
        rest_indent: indent_px(indent.rest_line),
        line_height: style.line_height.is_specified().then(|| text_unit_px(style.line_height, font_size, density)),
        line_height_style: style.line_height_style,
        include_font_padding,
        text_motion: style.text_motion.unwrap_or(TextMotion::Static),
    }
}

/// The text's bidi paragraphs: each up to and including its `\n`, an empty one after a `\n` ending the text (or
/// for empty text), each going its own way (Android's text direction heuristics, paragraph by paragraph).
fn bidi_paragraphs(text: &str, style: &TextStyle, settings: &ParagraphSettings) -> Vec<BidiPara> {
    let locales = style.locale_list.clone().unwrap_or_else(|| LocaleList::new([settings.locale.clone()]));
    let mut out = Vec::new();
    let mut start = 0;
    loop {
        let end = text[start..].find('\n').map_or(text.len(), |i| start + i + 1);
        out.push(BidiPara {
            range: start..end,
            bidi: BidiParagraph::new(&text[start..end], style.text_direction, &locales),
        });
        if end == text.len() {
            if text.ends_with('\n') {
                out.push(BidiPara { range: end..end, bidi: BidiParagraph::new("", style.text_direction, &locales) });
            }
            break;
        }
        start = end;
    }
    out
}

/// `range` cut where its script changes (characters of every script and marks going with the script before them,
/// those at its start with the first one after them), each piece with its script (None: no script of its own).
fn script_runs(text: &str, range: Range<usize>) -> Vec<(Range<usize>, Option<[u8; 4]>)> {
    let mut out: Vec<(Range<usize>, Option<[u8; 4]>)> = Vec::new();
    let mut start = range.start;
    let mut current: Option<[u8; 4]> = None;
    for (i, c) in text[range.clone()].char_indices() {
        let at = range.start + i;
        if is_common_or_inherited(c) {
            continue;
        }
        let script = script_tag(c);
        if script == *b"Zzzz" {
            continue;
        }
        match current {
            None => current = Some(script),
            Some(s) if s != script => {
                out.push((start..at, Some(s)));
                start = at;
                current = Some(script);
            }
            _ => {}
        }
    }
    out.push((start..range.end, current));
    out
}

/// What resolves the runs' styles: the text's style, the density, the resolver and its backend, the faces
/// resolved so far (and their states, for staleness).
struct StyleBuilder<'a> {
    style: &'a TextStyle,
    density: Density,
    resolver: &'a Rc<dyn FontFamilyResolver>,
    backend: &'a dyn FontBackend,
    typefaces: &'a mut Vec<(State<Typeface>, Typeface)>,
    resolved: HashMap<
        (crate::font::FontFamily, crate::font::FontWeight, crate::font::FontStyle, crate::font::FontSynthesis),
        Typeface,
    >,
}

impl StyleBuilder<'_> {
    /// The style of text under `spans` (in their order; a later one's attributes win), a placeholder's room if
    /// it is one.
    fn run_style(&mut self, spans: &[&SpanStyle], placeholder: Option<(usize, &Placeholder)>) -> RunStyle {
        let drawn = TextRunStyle::resolve(self.style, spans, self.density);
        let key = (drawn.font_family.clone(), drawn.font_weight, drawn.font_style, drawn.font_synthesis);
        let typeface = match self.resolved.get(&key) {
            Some(t) => t.clone(),
            None => {
                let state = self.resolver.resolve(
                    Some(&drawn.font_family),
                    drawn.font_weight,
                    drawn.font_style,
                    drawn.font_synthesis,
                );
                let t = state.get();
                self.typefaces.push((state, t.clone()));
                self.resolved.insert(key, t.clone());
                t
            }
        };
        let locales = spans
            .iter()
            .rev()
            .find_map(|s| s.locale_list.clone())
            .or_else(|| self.style.locale_list.clone())
            .unwrap_or_else(LocaleList::current);
        let baseline_shift = spans
            .iter()
            .rev()
            .find_map(|s| s.baseline_shift)
            .or(self.style.baseline_shift)
            .unwrap_or(BaselineShift::None)
            .take_or_else(|| BaselineShift::None)
            .multiplier();
        let foreground =
            spans.iter().fold(TextForegroundStyle::Unspecified, |fg: TextForegroundStyle, s| fg.merge(&s.foreground()));
        let background = spans
            .iter()
            .rev()
            .map(|s| s.background)
            .find(|c| c.is_specified())
            .unwrap_or(self.style.background)
            .take_or_else(|| Color::Transparent);
        let features: Rc<[FontFeature]> = Rc::from(parse_font_feature_settings(&drawn.font_feature_settings));
        let primary = primary_instance(self.backend, &typeface, drawn.font_size, self.density, &locales);
        let metrics = self.backend.metrics(&primary);
        let placeholder = placeholder.map(|(index, p)| PlaceholderRoom {
            index,
            width: text_unit_px(p.width, drawn.font_size, self.density).ceil(),
            height: text_unit_px(p.height, drawn.font_size, self.density).ceil(),
            align: p.placeholder_vertical_align,
        });
        RunStyle {
            drawn: Rc::new(drawn),
            typeface,
            features,
            locales,
            baseline_shift: if baseline_shift.is_nan() { 0.0 } else { baseline_shift },
            foreground,
            shadow: spans.iter().rev().find_map(|s| s.shadow.clone()),
            decoration: spans.iter().rev().find_map(|s| s.text_decoration),
            own_draw_style: spans.iter().any(|s| s.draw_style.is_some()),
            background,
            placeholder,
            primary,
            metrics,
        }
    }

    /// The text cut at every span's and placeholder's ends, each piece's style (pieces under the same spans
    /// sharing one).
    fn style_runs(
        &mut self,
        text: &str,
        annotations: &[AnnotatedRange<Annotation>],
        placeholders: &[AnnotatedRange<Placeholder>],
        base: &Rc<RunStyle>,
    ) -> Vec<StyleRun> {
        let len = text.len();
        let spans: Vec<(Range<usize>, &SpanStyle)> = annotations
            .iter()
            .filter_map(|a| match &a.item {
                Annotation::SpanStyle(s) if a.start < a.end => Some((a.start.min(len)..a.end.min(len), s)),
                _ => None,
            })
            .filter(|(r, _)| !r.is_empty())
            .collect();
        let mut cuts = vec![0, len];
        for (r, _) in &spans {
            cuts.extend([r.start, r.end]);
        }
        for p in placeholders {
            cuts.extend([p.start.min(len), p.end.min(len)]);
        }
        cuts.retain(|&c| text.is_char_boundary(c));
        cuts.sort_unstable();
        cuts.dedup();
        let mut shared: HashMap<(Vec<usize>, Option<usize>), Rc<RunStyle>> = HashMap::new();
        let mut runs = Vec::new();
        for w in cuts.windows(2) {
            let (start, end) = (w[0], w[1]);
            let covering: Vec<usize> = spans
                .iter()
                .enumerate()
                .filter(|(_, (r, _))| r.start <= start && end <= r.end)
                .map(|(i, _)| i)
                .collect();
            let placeholder = placeholders.iter().position(|p| p.start <= start && end <= p.end && p.start < p.end);
            let style = if covering.is_empty() && placeholder.is_none() {
                base.clone()
            } else {
                let key = (covering.clone(), placeholder);
                match shared.get(&key) {
                    Some(s) => s.clone(),
                    None => {
                        let over: Vec<&SpanStyle> = covering.iter().map(|&i| spans[i].1).collect();
                        let s = Rc::new(self.run_style(&over, placeholder.map(|i| (i, &placeholders[i].item))));
                        shared.insert(key, s.clone());
                        s
                    }
                }
            };
            runs.push(StyleRun { range: start..end, style });
        }
        if runs.is_empty() {
            runs.push(StyleRun { range: 0..0, style: base.clone() });
        }
        runs
    }
}

/// A typeface's own face at `size`: the face a space is shaped in (its variation coordinates set), else its font
/// as it is.
fn primary_instance(
    backend: &dyn FontBackend,
    typeface: &Typeface,
    size: f32,
    density: Density,
    locales: &LocaleList,
) -> FontInstance {
    let runs = backend.font_runs(&FontRunRequest { text: " ", range: 0..1, typeface, size, density, locales });
    runs.into_iter().map(|r| r.font).find(|f| f.font == typeface.font).unwrap_or_else(|| FontInstance {
        font: typeface.font,
        coords: Rc::from([]),
        size,
        synthesis: typeface.synthesis,
    })
}
