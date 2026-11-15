//! What a laid out paragraph draws (D3, Compose's Paragraph.paint as Android's Layout.draw and TextLine draw
//! it): a draw list built once per layout — one [`TextDraw`] per line, style run and bidi run (a justified line's
//! cut after each space it stretches; a start or middle ellipsis its own), its text in logical order with "…"
//! where an ellipsis replaces some, its glyph runs in visual order, its ink — and, each time it is painted, the
//! paint operations it takes: backgrounds, shadows, text in a color or a brush, underlines and strikethroughs, what
//! a draw call gives (a color or brush, a shadow, a decoration, a draw style) going where the text's spans gave
//! none of their own.

use super::Paragraph;
use super::ellipsis::{ELLIPSIS, Ellipsis};
use super::lines::LineGeometry;
use crate::draw::{Glyph, GlyphRun, TextDraw, TextRunStyle};
use crate::line_break::is_stretchable_space;
use crate::shape::{FontInstance, ShapedGlyph};
use crate::span_style::TextForegroundStyle;
use crate::style::{TextDecoration, TextOverflow};
use kompoz_graphics::{Brush, Color, DrawStyle, Offset, Rect, Shadow, Size};
use std::ops::Range;
use std::rc::Rc;

/// A drawn run of a paragraph: what it draws, its style run (what its spans paint it with), where its
/// background goes, and where its decorations go (its face's underline and strikeout, from its baseline, and how
/// thick they are).
#[derive(Clone, Debug)]
pub(crate) struct DrawItem {
    pub(crate) draw: TextDraw,
    pub(crate) run: usize,
    pub(crate) background: Rect,
    pub(crate) underline: (f32, f32),
    pub(crate) strikeout: (f32, f32),
}

/// What a draw call paints text with where the text's spans don't (Compose's Paragraph.paint parameters): its
/// foreground (a color, a brush at an alpha; unspecified: the paragraph's own), a shadow, a decoration and a draw
/// style (None: the paragraph's own).
#[derive(Clone, Debug, Default)]
pub struct TextPaintStyle {
    pub foreground: TextForegroundStyle,
    pub shadow: Option<Shadow>,
    pub decoration: Option<TextDecoration>,
    pub draw_style: Option<DrawStyle>,
}

/// What a run of text, or its decoration, is painted with: a color, or a brush spread over `area` at `alpha` (NaN:
/// the brush's own).
#[derive(Clone, PartialEq, Debug)]
pub enum TextPaint {
    Color(Color),
    Brush { brush: Brush, area: Rect, alpha: f32 },
}

/// A paint operation of laid out text, in its coordinates (the order is the drawing order): a span's background,
/// a run's shadow (its text moved by the shadow's offset), a run of text, a decoration line (`thickness` px thick,
/// its ends square at `from` and `to`).
#[derive(Clone, Debug)]
pub enum TextPaintOp {
    Background { rect: Rect, color: Color },
    Shadow { text: TextDraw, color: Color, blur_radius: f32 },
    Text { text: TextDraw, paint: TextPaint },
    Line { from: Offset, to: Offset, thickness: f32, paint: TextPaint },
}

impl Paragraph {
    /// Its draw list, built when first asked for and kept (so its glyph runs stay the same `Rc`s frame to frame).
    pub(crate) fn draw_items(&self) -> &[DrawItem] {
        self.draw_list.get_or_init(|| self.build_draw_list())
    }

    /// The runs of text it draws (one per line, style run and bidi run), in its coordinates.
    pub fn text_draws(&self) -> Vec<TextDraw> {
        self.draw_items().iter().map(|d| d.draw.clone()).collect()
    }

    /// The paint operations drawing it with `paint` takes, moved down by `top`: its own style's foreground,
    /// shadow, decoration and draw style where `paint` gives none; a brush `paint` gives spreads over
    /// `brush_area`, one of its own style or spans over the paragraph (Compose's MultiParagraph: a draw call's
    /// brush across all of the text, a paragraph's across itself).
    pub fn paint_ops(&self, paint: &TextPaintStyle, brush_area: Rect, top: f32) -> Vec<TextPaintOp> {
        let style = &self.intrinsics.style;
        let own_area = Rect::new(0.0, top, self.width, top + self.height);
        let base = if paint.foreground.is_specified() { paint.foreground.clone() } else { style.foreground() };
        let base_paint = to_paint(&base, if paint.foreground.is_specified() { brush_area } else { own_area });
        let shadow = paint.shadow.clone().or_else(|| style.shadow.clone());
        let decoration = paint.decoration.or(style.text_decoration).unwrap_or(TextDecoration::None);
        let mut ops = Vec::new();
        let moved = Offset::new(0.0, top);
        for item in self.draw_items() {
            let run = &self.intrinsics.runs[item.run].style;
            let mut draw = item.draw.at(item.draw.baseline + moved);
            if !run.own_draw_style
                && let Some(d) = &paint.draw_style
                && *d != draw.style.draw_style
            {
                draw.style = Rc::new(TextRunStyle { draw_style: d.clone(), ..(*draw.style).clone() });
            }
            if run.background.alpha() > 0.0 {
                ops.push(TextPaintOp::Background { rect: item.background.translate(moved), color: run.background });
            }
            let text_paint =
                if run.foreground.is_specified() { to_paint(&run.foreground, own_area) } else { base_paint.clone() };
            if let Some(s) = run.shadow.clone().or_else(|| shadow.clone())
                && s != Shadow::None
                && s.color.alpha() > 0.0
            {
                ops.push(TextPaintOp::Shadow {
                    text: draw.at(draw.baseline + s.offset),
                    color: s.color,
                    blur_radius: s.blur_radius,
                });
            }
            let (x, y, w) = (draw.baseline.x, draw.baseline.y, draw.size.width);
            ops.push(TextPaintOp::Text { text: draw, paint: text_paint.clone() });
            let decoration = run.decoration.unwrap_or(decoration);
            for (on, (position, thickness)) in [
                (decoration.contains(TextDecoration::Underline), item.underline),
                (decoration.contains(TextDecoration::LineThrough), item.strikeout),
            ] {
                if on && w > 0.0 {
                    let half = thickness / 2.0;
                    ops.push(TextPaintOp::Line {
                        from: Offset::new(x + half.min(w / 2.0), y + position),
                        to: Offset::new(x + w - half.min(w / 2.0), y + position),
                        thickness,
                        paint: text_paint.clone(),
                    });
                }
            }
        }
        ops
    }

    fn build_draw_list(&self) -> Vec<DrawItem> {
        let mut out = Vec::new();
        for i in 0..self.lines.len() {
            self.draw_line(i, &mut out);
        }
        out
    }

    /// Line `index`'s runs: its visual runs cut by style run (a justified line after its stretched spaces too, a
    /// start or middle ellipsis on its own), its visible text only (the text's last line its trailing spaces too,
    /// as Android draws them).
    fn draw_line(&self, index: usize, out: &mut Vec<DrawItem>) {
        let p = &self.intrinsics;
        let text = &*p.text;
        let line = &self.lines[index];
        let g = self.geometry(index);
        let mut draw_end = if index + 1 == self.lines.len() {
            let mut end = line.range.end;
            while end > line.range.start && matches!(text.as_bytes()[end - 1], b'\n' | b'\r') {
                end -= 1;
            }
            end
        } else {
            line.visible_end
        };
        if let Some(e) = &line.ellipsis {
            // The character the ellipsis stands in for is drawn, if it is past the visible text.
            let first = text[e.range.start..].chars().next().map_or(0, char::len_utf8);
            draw_end = draw_end.max((e.range.start + first).min(e.range.end));
        }
        let ellipsis = line.ellipsis.as_ref();
        for (run_range, rtl, run_x, _) in g.visual_runs() {
            let (from, to) = (run_range.start, run_range.end.min(draw_end));
            if from >= to {
                continue;
            }
            // Where pieces are cut: style runs, a start or middle ellipsis, a justified line's stretched spaces.
            let mut cuts: Vec<usize> = vec![from, to];
            let first_run = p.run_at(from);
            for r in p.runs[first_run..].iter().take_while(|r| r.range.start < to) {
                for c in [r.range.start, r.range.end] {
                    if c > from && c < to {
                        cuts.push(c);
                    }
                }
            }
            if let Some(e) = ellipsis
                && e.overflow != TextOverflow::Ellipsis
            {
                cuts.extend([e.range.start, e.range.end].into_iter().filter(|&c| c > from && c < to));
            }
            if line.justify {
                for (i, c) in text[from..to].char_indices() {
                    if is_stretchable_space(c) && from + i + 1 < to {
                        cuts.push(from + i + 1);
                    }
                }
            }
            cuts.sort_unstable();
            cuts.dedup();
            let mut pieces: Vec<Range<usize>> = cuts.windows(2).map(|w| w[0]..w[1]).collect();
            if rtl {
                pieces.reverse();
            }
            for piece in pieces {
                let run = p.run_at(piece.start);
                if p.runs[run].style.placeholder.is_some() {
                    continue;
                }
                // Text an ellipsis replaces, but for its first character (the ellipsis), draws nothing.
                if let Some(e) = ellipsis
                    && piece.start > e.range.start
                    && piece.end <= e.range.end
                {
                    continue;
                }
                let x = if rtl {
                    run_x + g.width(piece.end..run_range.end)
                } else {
                    run_x + g.width(run_range.start..piece.start)
                };
                let width = g.width(piece.clone());
                out.push(self.draw_piece(index, g, piece, run, x, width, rtl, ellipsis));
            }
        }
    }

    /// One run of line `line`: `piece` of style run `run`, its left at `x`, `width` wide.
    #[allow(clippy::too_many_arguments)]
    fn draw_piece(
        &self,
        line: usize,
        g: &LineGeometry,
        piece: Range<usize>,
        run: usize,
        x: f32,
        width: f32,
        rtl: bool,
        ellipsis: Option<&Ellipsis>,
    ) -> DrawItem {
        let p = &self.intrinsics;
        let text = &*p.text;
        let l = &self.lines[line];
        let style = &p.runs[run].style;
        // Its text, the ellipsis in place of what it replaces.
        let cut = ellipsis.filter(|e| e.range.start < piece.end && e.range.end > piece.start);
        let shown: String = match cut {
            Some(e) => {
                let head = &text[piece.start..e.range.start.max(piece.start)];
                let tail = &text[e.range.end.min(piece.end).max(piece.start)..piece.end];
                let mark = if e.range.start >= piece.start { ELLIPSIS } else { "" };
                format!("{head}{mark}{tail}")
            }
            None => text[piece.clone()].to_string(),
        };
        // A hyphen edit shows at the line's end (or start), the replaced character's place taken.
        let mut shown = shown;
        if piece.end == l.visible_end && cut.is_none() {
            if l.end_hyphen == crate::line_break::EndHyphenEdit::ReplaceWithHyphen {
                shown.pop();
            }
            shown.push_str(l.end_hyphen.text());
        }
        if piece.start == l.range.start {
            shown.insert_str(0, l.start_hyphen.text());
        }
        let local = |c: usize| -> u32 {
            match cut {
                Some(e) if c >= e.range.end => {
                    (e.range.start.max(piece.start) - piece.start + ELLIPSIS.len() + (c - e.range.end)) as u32
                }
                Some(e) if c >= e.range.start => (e.range.start.max(piece.start) - piece.start) as u32,
                _ => (c - piece.start) as u32,
            }
        };
        let mut runs: Vec<GlyphRun> = Vec::new();
        let mut ink: Option<Rect> = None;
        let mut push = |font: &FontInstance, glyph: Glyph, scale: (f32, f32)| {
            let b = p.backend.glyph_bounds(font, glyph.id);
            if b.width() > 0.0 || b.height() > 0.0 {
                let (sx, skew) = scale;
                // (Android's textSkewX: a point y below the baseline moves y · skew to the right, Skia's y being
                // down; -0.25 leans right.)
                let (a, c) = (b.top * skew, b.bottom * skew);
                let r = Rect::new(
                    glyph.x + b.left * sx + a.min(c),
                    glyph.y + b.top,
                    glyph.x + b.right * sx + a.max(c),
                    glyph.y + b.bottom,
                );
                ink = Some(ink.map_or(r, |i| i.union(r)));
            }
            match runs.last_mut() {
                Some(last) if last.font == font.font && last.coords == font.coords && last.size == font.size => {
                    last.glyphs.push(glyph)
                }
                _ => runs.push(GlyphRun {
                    font: font.font,
                    coords: font.coords.clone(),
                    size: font.size,
                    synthesis: font.synthesis,
                    scale_x: style.drawn.scale_x,
                    skew_x: style.drawn.skew_x,
                    color: p.backend.color_kind(font.font),
                    text_motion: p.settings.text_motion,
                    glyphs: vec![glyph],
                }),
            }
        };
        let scale = (style.drawn.scale_x, style.drawn.skew_x);
        let mut segments: Vec<&super::lines::LineSegment> =
            g.segments.iter().filter(|s| s.range.start < piece.end && s.range.end > piece.start).collect();
        if rtl {
            segments.reverse();
        }
        for s in segments {
            let mut starts: Vec<usize> = s.glyphs.iter().map(|g| g.cluster).collect();
            starts.sort_unstable();
            starts.dedup();
            for (cluster, glyphs) in clusters_in(&s.glyphs, &piece) {
                // Where the cluster's laid out room starts (its left), from the piece's left.
                let next = starts.partition_point(|&c| c <= cluster);
                let end = starts.get(next).copied().unwrap_or(s.range.end);
                let left = if rtl { g.width(end..piece.end) } else { g.width(piece.start..cluster) };
                if let Some(e) = cut
                    && cluster >= e.range.start
                    && cluster < e.range.end
                {
                    if cluster == e.range.start
                        && let Some(eg) = &g.ellipsis
                    {
                        let mut pen = left;
                        for eglyph in &eg.glyphs {
                            let glyph = Glyph {
                                id: eglyph.id,
                                x: pen + eglyph.x_offset,
                                y: -eglyph.y_offset,
                                cluster: local(cluster),
                            };
                            push(&eg.font, glyph, scale);
                            pen += eglyph.x_advance;
                        }
                    }
                    continue;
                }
                let font = &p.segments[s.segment].font;
                let mut pen = left;
                for sg in glyphs {
                    let glyph = Glyph { id: sg.id, x: pen + sg.x_offset, y: -sg.y_offset, cluster: local(cluster) };
                    push(font, glyph, scale);
                    pen += sg.x_advance;
                }
            }
        }
        let height = l.bottom - l.top;
        let mut ink = ink.unwrap_or(Rect::new(0.0, -l.ascent, 0.0, l.descent));
        if let DrawStyle::Stroke(stroke) = &style.drawn.draw_style {
            ink = ink.inflate(stroke.width / 2.0);
        }
        let shift = style.shift_y();
        let draw = TextDraw::new(
            shown,
            Offset::new(x, l.baseline + shift),
            Size::new(width, height),
            ink,
            rtl,
            style.drawn.clone(),
            Rc::from(runs),
        );
        let m = &style.metrics;
        DrawItem {
            draw,
            run,
            background: Rect::new(x, l.top, x + width, l.bottom),
            underline: (m.underline_position, m.underline_thickness),
            strikeout: (m.strikeout_position, m.strikeout_thickness),
        }
    }
}

/// The clusters of `glyphs` (visual order) starting in `piece`, each with its glyphs, in visual order.
fn clusters_in<'g>(glyphs: &'g [ShapedGlyph], piece: &Range<usize>) -> Vec<(usize, &'g [ShapedGlyph])> {
    let mut out: Vec<(usize, &[ShapedGlyph])> = Vec::new();
    let mut i = 0;
    while i < glyphs.len() {
        let c = glyphs[i].cluster;
        let mut j = i;
        while j < glyphs.len() && glyphs[j].cluster == c {
            j += 1;
        }
        if piece.contains(&c) {
            out.push((c, &glyphs[i..j]));
        }
        i = j;
    }
    out
}

/// A foreground as a paint, a brush spread over `area`.
fn to_paint(foreground: &TextForegroundStyle, area: Rect) -> TextPaint {
    match foreground {
        TextForegroundStyle::Color(c) => TextPaint::Color(*c),
        TextForegroundStyle::Brush { brush, alpha } => TextPaint::Brush { brush: brush.clone(), area, alpha: *alpha },
        TextForegroundStyle::Unspecified => TextPaint::Color(Color::Black),
    }
}
