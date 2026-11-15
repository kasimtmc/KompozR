//! A line's metrics and horizontal layout.
//!
//! Vertically (D5): a line reaches as far above and below its baseline as the fonts on it do — each style run's
//! own face (shifted by its baseline shift) and every face its text fell back to (Android's fallback line
//! spacing), placeholders as tall as they ask (Android's PlaceholderSpan) — its descent with the fonts' leading
//! (lines are ascent + descent + leading apart, as the C++ Kompoz's). `include_font_padding` reaches the first
//! line up to the fonts' tallest glyphs and the last down to their lowest (StaticLayout's includePad). A line
//! height is applied as Compose's LineHeightStyleSpan does (its room shared above and below as the alignment
//! says, worked out once from the first line; the text's first top and last bottom trimmed; Fixed, Minimum and
//! Tight; the padding a shrunk untrimmed first or last line needs), or as its LineHeightSpan where the fonts'
//! padding is kept and no style given. In px, not rounded (Android's lines are whole pixels; these aren't).
//!
//! Horizontally: the line's text as shaped (pieces cut where shaping changes shaped again, hyphen edits shaped
//! in), each cluster's advance (shared among the graphemes of a ligature, as minikin's), tabs reaching their stops,
//! justified spaces stretched (or squeezed), an ellipsis taking the room of the text it replaces; its visual runs
//! (UAX #9's L1 and L2, trailing whitespace at the paragraph's level) laid side by side from where its alignment,
//! indent and direction put it.

use super::Line;
use super::ellipsis::{ELLIPSIS, Ellipsis};
use super::intrinsics::{ParagraphIntrinsics, PlaceholderRoom, RunStyle};
use crate::bidi::BidiLine;
use crate::line_break::{EndHyphenEdit, StartHyphenEdit, TabStops, is_line_end_space, is_stretchable_space};
use crate::placeholder::PlaceholderVerticalAlign;
use crate::segment::grapheme_breaks;
use crate::shape::{FontInstance, FontMetrics, FontRunRequest, ShapeRequest, ShapedGlyph};
use crate::style::line_height_style::Mode;
use crate::style::{LineHeightStyle, ResolvedTextDirection, TextAlign, TextOverflow};
use std::ops::Range;

// ---- Vertical ---------------------------------------------------------------------------------------------------

/// How far a line reaches above and below its baseline (positive both; the descent with the leading), and how
/// far its tallest and lowest glyphs could.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub(crate) struct Extent {
    pub(crate) ascent: f32,
    pub(crate) descent: f32,
    pub(crate) top: f32,
    pub(crate) bottom: f32,
}

impl Extent {
    fn grow(&mut self, ascent: f32, descent: f32, top: f32, bottom: f32) {
        self.ascent = self.ascent.max(ascent);
        self.descent = self.descent.max(descent);
        self.top = self.top.max(top).max(ascent);
        self.bottom = self.bottom.max(bottom).max(descent);
    }

    /// A face's, its baseline moved down by `shift` (up if negative): what it reaches grows that way.
    fn grow_font(&mut self, m: &FontMetrics, shift: f32) {
        let (up, down) = (-shift.min(0.0), shift.max(0.0));
        self.grow(m.ascent + up, m.descent + m.leading + down, m.top + up, m.bottom + down);
    }
}

/// How far a placeholder makes its line reach (Android's PlaceholderSpan.getSize): the run's face's extent, grown
/// to hold the placeholder where it sits (the text's own descent, without leading, for the text-relative ones).
fn placeholder_extent(room: &PlaceholderRoom, m: &FontMetrics) -> (f32, f32) {
    let (a, d) = (m.ascent, m.descent + m.leading);
    let h = room.height;
    match room.align {
        PlaceholderVerticalAlign::AboveBaseline => (a.max(h), d),
        PlaceholderVerticalAlign::Top | PlaceholderVerticalAlign::TextTop => (a, d.max(h - a)),
        PlaceholderVerticalAlign::Bottom => (a.max(h - d), d),
        PlaceholderVerticalAlign::TextBottom => (a.max(h - m.descent), d),
        PlaceholderVerticalAlign::Center => {
            if a + d < h {
                let above = a + (h - (a + d)) / 2.0;
                (above, h - above)
            } else {
                (a, d)
            }
        }
        PlaceholderVerticalAlign::TextCenter => {
            if a + m.descent < h {
                let above = a + (h - (a + m.descent)) / 2.0;
                (above, (h - above).max(d))
            } else {
                (a, d)
            }
        }
    }
}

/// How far the text in `range` reaches (an empty range: the style there): its style runs' faces, shifted, their
/// placeholders, and the faces its text fell back to.
pub(crate) fn extent_of(p: &ParagraphIntrinsics, range: Range<usize>) -> Extent {
    let mut e = Extent::default();
    let first = p.run_at(range.start);
    for run in p.runs[first..].iter().take_while(|r| r.range.start < range.end.max(range.start + 1)) {
        let st = &run.style;
        match &st.placeholder {
            Some(room) => {
                let (a, d) = placeholder_extent(room, &st.metrics);
                e.grow(a, d, a, d);
            }
            None => e.grow_font(&st.metrics, st.shift_y()),
        }
    }
    let mut i = p.segments.partition_point(|s| s.range.end <= range.start);
    while let Some(seg) = p.segments.get(i)
        && seg.range.start < range.end
    {
        if !seg.placeholder && seg.font != p.runs[seg.run].style.primary {
            e.grow_font(&seg.metrics, p.runs[seg.run].style.shift_y());
        }
        i += 1;
    }
    e
}

/// What a line height style asks of lines (Compose's LineHeightStyleSpan.calculateTargetMetrics, from the first
/// line's extent): every line's ascent and descent, the first line's ascent and the last's descent, and how much
/// the first's ascent and the last's descent lost (negative: the padding they need).
#[derive(Clone, Copy, Debug)]
struct Target {
    first_ascent: f32,
    ascent: f32,
    descent: f32,
    last_descent: f32,
    first_ascent_diff: f32,
    last_descent_diff: f32,
}

fn target(line_height: f32, style: LineHeightStyle, a: f32, d: f32, trim_top: bool, trim_bottom: bool) -> Target {
    let current = a + d;
    let diff = line_height - current;
    if style.mode == Mode::Minimum && diff <= 0.0 {
        return Target {
            first_ascent: a,
            ascent: a,
            descent: d,
            last_descent: d,
            first_ascent_diff: 0.0,
            last_descent_diff: 0.0,
        };
    }
    let top_ratio = style.alignment.top_ratio();
    let ratio = if top_ratio == -1.0 { a / current } else { top_ratio };
    let descent_diff = if diff <= 0.0 { diff * ratio } else { diff * (1.0 - ratio) };
    let descent = d + descent_diff;
    let ascent = line_height - descent;
    if style.mode == Mode::Fixed || diff >= 0.0 {
        let first_ascent = if trim_top { a } else { ascent };
        let last_descent = if trim_bottom { d } else { descent };
        Target {
            first_ascent,
            ascent,
            descent,
            last_descent,
            first_ascent_diff: first_ascent - a,
            last_descent_diff: last_descent - d,
        }
    } else {
        // Tight: a trimmed end keeps the smaller of the two, an untrimmed one the larger; no padding.
        Target {
            first_ascent: if trim_top { a.min(ascent) } else { a.max(ascent) },
            ascent,
            descent,
            last_descent: if trim_bottom { d.min(descent) } else { d.max(descent) },
            first_ascent_diff: 0.0,
            last_descent_diff: 0.0,
        }
    }
}

/// Lays the lines out vertically: each line's extent, its line height applied, the lines stacked, the padding
/// above the first and below the last added; returns the height.
pub(crate) fn stack(p: &ParagraphIntrinsics, lines: &mut [Line]) -> f32 {
    let text_len = p.text.len();
    let settings = &p.settings;
    let n = lines.len();
    let mut extents: Vec<Extent> = lines.iter().map(|l| extent_of(p, l.range.clone())).collect();
    if settings.include_font_padding {
        if let Some(e) = extents.first_mut() {
            e.ascent = e.ascent.max(e.top);
        }
        if let Some(e) = extents.last_mut() {
            e.descent = e.descent.max(e.bottom);
        }
    }
    let mut metrics: Vec<(f32, f32)> = extents.iter().map(|e| (e.ascent, e.descent)).collect();
    let (mut top_padding, mut bottom_padding) = (0.0f32, 0.0f32);
    if let Some(line_height) = settings.line_height {
        if settings.include_font_padding && settings.line_height_style.is_none() {
            // LineHeightSpan: each line the line height, its descent grown as much as the whole.
            for m in &mut metrics {
                let current = m.0 + m.1;
                if current > 0.0 {
                    let descent = m.1 * line_height / current;
                    *m = (line_height - descent, descent);
                }
            }
        } else {
            let style = settings.line_height_style.unwrap_or(LineHeightStyle::Default);
            let (trim_top, trim_bottom) = (style.trim.is_trim_first_line_top(), style.trim.is_trim_last_line_bottom());
            // The span ends past the text where it is empty or ends in a line feed: no line is its last then
            // (that empty last line is worked out on its own, as Compose's getLastLineMetrics does).
            let span_end = if p.text.is_empty() || p.text.ends_with('\n') { text_len + 1 } else { text_len };
            let empty_last = |i: usize| i + 1 == n && lines[i].range.is_empty() && span_end > text_len;
            let mut once: Option<Target> = None;
            for i in 0..n {
                let (a, d) = metrics[i];
                if a + d <= 0.0 {
                    continue;
                }
                if empty_last(i) {
                    let first_trim = if i != 0 && trim_bottom { false } else { trim_bottom };
                    if !(first_trim && trim_bottom && style.mode != Mode::Tight) {
                        let t = target(line_height, style, a, d, first_trim, trim_bottom);
                        metrics[i] = (t.first_ascent, t.last_descent);
                    }
                    continue;
                }
                let is_first = lines[i].range.start == 0;
                let is_last = lines[i].range.end == span_end;
                if is_first && is_last && trim_top && trim_bottom && style.mode != Mode::Tight {
                    continue;
                }
                let t = *once.get_or_insert_with(|| target(line_height, style, a, d, trim_top, trim_bottom));
                metrics[i] = (
                    if is_first { t.first_ascent } else { t.ascent },
                    if is_last { t.last_descent } else { t.descent },
                );
            }
            if let Some(t) = once {
                let force_top = trim_top && style.mode == Mode::Tight;
                let force_bottom = trim_bottom && style.mode == Mode::Tight;
                let first = if t.first_ascent_diff < 0.0 { -t.first_ascent_diff } else { 0.0 };
                // (Compose keeps the larger of the first line's padding and the last's as the last's.)
                let last = if t.last_descent_diff < 0.0 { first.max(-t.last_descent_diff) } else { 0.0 };
                top_padding = if force_top { 0.0 } else { first };
                bottom_padding = if force_bottom { 0.0 } else { last };
            }
        }
    }
    let mut y = 0.0;
    for (i, line) in lines.iter_mut().enumerate() {
        let (a, d) = metrics[i];
        line.ascent = a;
        line.descent = d;
        line.top = if i == 0 { 0.0 } else { y + top_padding };
        line.baseline = top_padding + y + a;
        y += a + d;
        line.bottom = top_padding + y + if i + 1 == n { bottom_padding } else { 0.0 };
    }
    y + top_padding + bottom_padding
}

// ---- Horizontal -------------------------------------------------------------------------------------------------

/// A line's piece of a segment: its text, the segment, its glyphs (the segment's, or the piece shaped again where
/// it was cut where shaping changes or a hyphen edit goes) in visual order.
pub(crate) struct LineSegment {
    pub(crate) range: Range<usize>,
    pub(crate) segment: usize,
    pub(crate) glyphs: Vec<ShapedGlyph>,
}

/// The ellipsis as shaped: in the face of the text it replaces (or one that has it), its glyphs and width.
pub(crate) struct EllipsisGlyphs {
    pub(crate) font: FontInstance,
    pub(crate) glyphs: Vec<ShapedGlyph>,
    pub(crate) width: f32,
}

/// A line laid out across: its bidi line (offsets into its bidi paragraph's text, `para_start` its start), its
/// pieces of segments in logical order, its units (clusters, or graphemes of a ligature) and their advances as
/// laid out (`prefix[k]` before unit `k`), its ellipsis's glyphs, where its visible text starts (its left), how
/// wide that is, and where its visual runs start (its trailing whitespace included).
pub(crate) struct LineGeometry {
    pub(crate) bidi: BidiLine,
    pub(crate) para_start: usize,
    pub(crate) segments: Vec<LineSegment>,
    pub(crate) units: Vec<usize>,
    pub(crate) prefix: Vec<f32>,
    pub(crate) ellipsis: Option<EllipsisGlyphs>,
    pub(crate) left: f32,
    pub(crate) width: f32,
    pub(crate) x0: f32,
}

impl LineGeometry {
    /// The laid out width of `range` (unit boundaries; an offset inside a unit counts from its start).
    pub(crate) fn width(&self, range: Range<usize>) -> f32 {
        if range.start >= range.end {
            return 0.0;
        }
        self.prefix_at(range.end) - self.prefix_at(range.start)
    }

    /// The advance before `offset` from the line's start.
    pub(crate) fn prefix_at(&self, offset: usize) -> f32 {
        self.prefix[self.units.partition_point(|&u| u < offset)]
    }

    /// Where a caret at `offset` goes (Android's getPrimaryHorizontal or getSecondaryHorizontal), from the
    /// paragraph's left.
    pub(crate) fn horizontal(&self, offset: usize, primary: bool) -> f32 {
        let ps = self.para_start;
        self.x0 + self.bidi.horizontal(offset - ps, primary, |r| self.width(r.start + ps..r.end + ps))
    }

    /// Where the trailing edge of the character before `offset` (`trailing`) or the leading edge of the one at
    /// it is (Android's getHorizontal with TextLine.measure), from the paragraph's left.
    pub(crate) fn position(&self, offset: usize, trailing: bool) -> f32 {
        let ps = self.para_start;
        self.x0 + self.bidi.position(offset - ps, trailing, |r| self.width(r.start + ps..r.end + ps))
    }

    /// The visual runs, left to right: their ranges (into the paragraph's text), whether they go right to left,
    /// their left edges and widths.
    pub(crate) fn visual_runs(&self) -> Vec<(Range<usize>, bool, f32, f32)> {
        let ps = self.para_start;
        let mut x = self.x0;
        self.bidi
            .runs()
            .iter()
            .map(|r| {
                let range = r.range.start + ps..r.range.end + ps;
                let w = self.width(range.clone());
                let run = (range, r.is_rtl(), x, w);
                x += w;
                run
            })
            .collect()
    }
}

/// Where the visible text of `start..end` ends: its trailing line end spaces and line feed left out (Compose's
/// LayoutHelper.lineEndToVisibleEnd).
pub(crate) fn visible_end_of(text: &str, start: usize, end: usize) -> usize {
    let mut visible = end;
    for (i, c) in text[start..end].char_indices().rev() {
        if !is_line_end_space(c) {
            break;
        }
        visible = start + i;
    }
    visible
}

/// The ellipsis shaped in the face `style` resolved to (or the one it falls back to for "…").
pub(crate) fn shape_ellipsis(p: &ParagraphIntrinsics, style: &RunStyle) -> EllipsisGlyphs {
    let runs = p.backend.font_runs(&FontRunRequest {
        text: ELLIPSIS,
        range: 0..ELLIPSIS.len(),
        typeface: &style.typeface,
        size: style.drawn.font_size,
        density: p.density(),
        locales: &style.locales,
    });
    let font = runs.into_iter().next().map_or_else(|| style.primary.clone(), |r| r.font);
    let shaped = p.backend.shape(&ShapeRequest {
        text: ELLIPSIS,
        range: 0..ELLIPSIS.len(),
        font: &font,
        rtl: false,
        script: None,
        language: style.language(),
        features: &style.features,
        letter_spacing: style.drawn.letter_spacing,
        scale_x: style.drawn.scale_x,
    });
    EllipsisGlyphs { font, width: shaped.advance, glyphs: shaped.glyphs }
}

/// How far in a line starts from its paragraph's start: the first line indent on a paragraph's first line, the
/// rest line indent on the others.
pub(crate) fn indent_of(p: &ParagraphIntrinsics, line: &Line) -> f32 {
    if line.first_in_paragraph { p.settings.first_indent } else { p.settings.rest_indent }
}

/// Lays `line` out across in a paragraph `width` px wide.
pub(crate) fn geometry(p: &ParagraphIntrinsics, line: &Line, width: f32) -> LineGeometry {
    let para = &p.paragraphs[line.para];
    let ps = para.range.start;
    let range = line.range.clone();
    let bidi = para.bidi.line(&p.text[para.range.clone()], range.start - ps..range.end - ps);
    let segments = line_segments(p, line);
    // The units: each cluster's advance, a ligature's shared among its graphemes.
    let graphemes: Vec<usize> = grapheme_breaks(&p.text[range.clone()]).map(|g| range.start + g).collect();
    let mut units: Vec<usize> = Vec::new();
    let mut advances: Vec<f32> = Vec::new();
    for s in &segments {
        let seg = &p.segments[s.segment];
        if seg.placeholder {
            units.push(s.range.start);
            advances.push(if s.range.start == seg.range.start { seg.advance } else { 0.0 });
            continue;
        }
        // Each cluster's advance: its glyphs' (in visual order, so sorted by cluster first).
        let mut clusters: Vec<(usize, f32)> = s.glyphs.iter().map(|g| (g.cluster, g.x_advance)).collect();
        clusters.sort_by_key(|c| c.0);
        clusters.dedup_by(|b, a| {
            let same = a.0 == b.0;
            if same {
                a.1 += b.1;
            }
            same
        });
        if clusters.first().is_none_or(|c| c.0 > s.range.start) {
            clusters.insert(0, (s.range.start, 0.0));
        }
        for (k, &(start, advance)) in clusters.iter().enumerate() {
            let end = clusters.get(k + 1).map_or(s.range.end, |c| c.0);
            let from = graphemes.partition_point(|&g| g <= start);
            let to = graphemes.partition_point(|&g| g < end);
            let inside = &graphemes[from..to.max(from)];
            let share = advance / (inside.len() + 1) as f32;
            for g in std::iter::once(start).chain(inside.iter().copied()) {
                // What the shaping cut into the same offset (a cluster split across segments) is one unit.
                if units.last() == Some(&g) {
                    *advances.last_mut().expect("an advance") += share;
                } else {
                    units.push(g);
                    advances.push(share);
                }
            }
        }
    }
    let text = &p.text;
    // Tabs reach their stops (from the line's start, in logical order, as the line breaker measured them).
    if text[range.clone()].contains('\t') {
        let stops = TabStops::default();
        let mut x = 0.0;
        for k in 0..units.len() {
            if text.as_bytes()[units[k]] == b'\t' {
                advances[k] = stops.next_tab(x) - x;
            }
            x += advances[k];
        }
    }
    // The ellipsis takes the room of the first character it replaces; the others none.
    let mut ellipsis = None;
    if let Some(Ellipsis { range: e, .. }) = &line.ellipsis {
        let glyphs = shape_ellipsis(p, p.style_at(e.start));
        for k in 0..units.len() {
            if units[k] >= e.start && units[k] < e.end {
                advances[k] = if units[k] == e.start { glyphs.width } else { 0.0 };
            }
        }
        ellipsis = Some(glyphs);
    }
    let visible_end = match &line.ellipsis {
        Some(e) => line.visible_end.max(e.range.end.min(range.end)),
        None => line.visible_end,
    };
    let indent = indent_of(p, line);
    let room = width - indent;
    // A justified line's stretchable spaces share what it lacks of its room (or give what it has too much).
    if line.justify {
        let spaces: Vec<usize> = (0..units.len())
            .filter(|&k| units[k] < visible_end && text[units[k]..].chars().next().is_some_and(is_stretchable_space))
            .collect();
        if !spaces.is_empty() {
            let w: f32 = (0..units.len()).filter(|&k| units[k] < visible_end).map(|k| advances[k]).sum();
            let extra = (room - w) / spaces.len() as f32;
            for k in spaces {
                advances[k] += extra;
            }
        }
    }
    let mut prefix = Vec::with_capacity(units.len() + 1);
    let mut sum = 0.0;
    prefix.push(0.0);
    for a in &advances {
        sum += a;
        prefix.push(sum);
    }
    let mut g =
        LineGeometry { bidi, para_start: ps, segments, units, prefix, ellipsis, left: 0.0, width: 0.0, x0: 0.0 };
    let visible = g.width(range.start..visible_end);
    let total = g.width(range.clone());
    let rtl = para.bidi.direction() == ResolvedTextDirection::Rtl;
    let left = aligned_left(p.settings.text_align, rtl, width, indent, visible);
    g.left = left;
    g.width = visible;
    g.x0 = if rtl { left + visible - total } else { left };
    g
}

/// Where a line's visible text starts in a paragraph `width` wide (Android's Layout alignment): `Start` and
/// `Justify` (ALIGN_NORMAL) at the paragraph's start past the indent, `End` (ALIGN_OPPOSITE) at its end, `Left` and
/// `Right` so whatever the direction, `Center` in the middle of what the indent leaves (rounded down, as Android
/// draws it); a right-to-left paragraph's indent on its right.
pub(crate) fn aligned_left(align: TextAlign, rtl: bool, width: f32, indent: f32, visible: f32) -> f32 {
    let (pl, pr) = if rtl { (0.0, width - indent) } else { (indent, width) };
    let normal = if rtl { pr - visible } else { pl };
    let opposite = if rtl { pl } else { pr - visible };
    match align {
        TextAlign::Left => {
            if rtl {
                opposite
            } else {
                normal
            }
        }
        TextAlign::Right => {
            if rtl {
                normal
            } else {
                opposite
            }
        }
        TextAlign::Center => pl + ((pr - pl - visible) / 2.0).floor(),
        TextAlign::End => opposite,
        _ => normal,
    }
}

/// `line`'s pieces of segments, in logical order: the segments' glyphs where the line cuts them where shaping
/// doesn't change, else the piece shaped again (and with the line's hyphen edits at its ends).
fn line_segments(p: &ParagraphIntrinsics, line: &Line) -> Vec<LineSegment> {
    let range = line.range.clone();
    let mut out = Vec::new();
    let mut i = p.segments.partition_point(|s| s.range.end <= range.start);
    let first_index = i;
    let last_index = {
        let mut j = i;
        while p.segments.get(j + 1).is_some_and(|s| s.range.start < range.end) {
            j += 1;
        }
        j
    };
    while let Some(seg) = p.segments.get(i)
        && seg.range.start < range.end
    {
        let sub = range.start.max(seg.range.start)..range.end.min(seg.range.end);
        if sub.is_empty() {
            i += 1;
            continue;
        }
        let start_edit = if i == first_index { line.start_hyphen } else { StartHyphenEdit::NoEdit };
        let end_edit = if i == last_index { line.end_hyphen } else { EndHyphenEdit::NoEdit };
        let glyphs = if seg.placeholder {
            Vec::new()
        } else if start_edit != StartHyphenEdit::NoEdit || end_edit != EndHyphenEdit::NoEdit {
            p.shape_with_edits(i, sub.clone(), start_edit, end_edit).glyphs
        } else if sub == seg.range {
            seg.glyphs.clone()
        } else if p.is_safe_cut(sub.start) && p.is_safe_cut(sub.end) {
            seg.glyphs.iter().filter(|g| sub.contains(&g.cluster)).copied().collect()
        } else {
            p.reshape(i, sub.clone()).glyphs
        };
        out.push(LineSegment { range: sub, segment: i, glyphs });
        i += 1;
    }
    out
}

/// Whether `overflow` ellipsizes.
pub(crate) fn is_ellipsis(overflow: TextOverflow) -> bool {
    matches!(overflow, TextOverflow::Ellipsis | TextOverflow::StartEllipsis | TextOverflow::MiddleEllipsis)
}
