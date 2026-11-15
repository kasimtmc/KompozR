//! Breaking a paragraph into lines as Android's StaticLayout does: each `\n` paragraph broken on its own in the
//! paragraph's width less its indents (its first line's own), only `\n` ending a line; past `max_lines`, the last
//! line allowed takes the rest of its paragraph where an ellipsis may cut it (the end, or a single line's start
//! or middle) and the lines after it are dropped; the lines StaticLayout would ellipsize ellipsized; justified
//! lines marked (Android's: every line but a paragraph's last and the text's); then stacked.

use super::Line;
use super::ellipsis;
use super::intrinsics::{ParagraphIntrinsics, ParagraphMeasure};
use super::lines::{self, shape_ellipsis, visible_end_of};
use crate::line_break::{LineBreakParams, LineWidths, TabStops, break_lines, hyphenator};
use crate::style::{TextAlign, TextOverflow};
use std::cell::OnceCell;

/// Lines laid out, the height they take, whether lines past `max_lines` were left out (or the last ellipsized),
/// and whether that left text not laid out (no ellipsis took it).
pub(crate) struct Laid {
    pub(crate) lines: Vec<Line>,
    pub(crate) height: f32,
    pub(crate) did_exceed_max_lines: bool,
    pub(crate) truncated: bool,
}

/// Lays `p` out in `width` px, at most `max_lines` lines, cut as `overflow` says.
pub(crate) fn lay_out(p: &ParagraphIntrinsics, width: f32, max_lines: usize, overflow: TextOverflow) -> Laid {
    let text = &*p.text;
    let settings = &p.settings;
    let hyphenator = hyphenator();
    let unbreakable = p.unbreakable();
    let mut lines: Vec<Line> = Vec::new();
    for (index, para) in p.paragraphs.iter().enumerate() {
        let slice = &text[para.range.clone()];
        let local: Vec<std::ops::Range<usize>> = unbreakable
            .iter()
            .filter(|r| r.start >= para.range.start && r.end <= para.range.end)
            .map(|r| r.start - para.range.start..r.end - para.range.start)
            .collect();
        let params = LineBreakParams {
            line_break: settings.line_break,
            hyphens: settings.hyphens,
            hyphenator: hyphenator.as_deref(),
            locale: settings.locale.clone(),
            widths: LineWidths::indented(width, settings.first_indent, settings.rest_indent),
            justified: settings.text_align == TextAlign::Justify,
            tab_stops: TabStops::default(),
            unbreakable: &local,
            line_feed_breaks_only: true,
        };
        let measure = ParagraphMeasure { paragraph: p, origin: para.range.start };
        let mut broken = break_lines(slice, &measure, &params);
        // A `\n` ending the paragraph ends its last line: the empty line after it is the next paragraph's.
        if slice.ends_with('\n') && broken.len() > 1 && broken.last().is_some_and(|l| l.range.is_empty()) {
            broken.pop();
        }
        for (k, b) in broken.into_iter().enumerate() {
            let start = para.range.start + b.range.start;
            let end = para.range.start + b.range.end;
            lines.push(Line {
                range: start..end,
                visible_end: visible_end_of(text, start, end),
                para: index,
                hard_break: b.hard_break,
                first_in_paragraph: k == 0,
                start_hyphen: b.start_hyphen,
                end_hyphen: b.end_hyphen,
                ellipsis: None,
                justify: false,
                top: 0.0,
                bottom: 0.0,
                baseline: 0.0,
                ascent: 0.0,
                descent: 0.0,
                geometry: OnceCell::new(),
            });
        }
    }

    let mut truncated = false;
    if lines.len() > max_lines {
        let last = max_lines - 1;
        let may_ellipsize = overflow == TextOverflow::Ellipsis
            || (max_lines == 1 && matches!(overflow, TextOverflow::StartEllipsis | TextOverflow::MiddleEllipsis));
        if may_ellipsize {
            // The last line allowed and its paragraph's lines after it, as one line.
            let para = lines[last].para;
            let rest = lines[last..].iter().take_while(|l| l.para == para).count() - 1;
            if rest > 0 {
                let merged = &lines[last + rest];
                let (end, hard_break, end_hyphen) = (merged.range.end, merged.hard_break, merged.end_hyphen);
                let line = &mut lines[last];
                line.range.end = end;
                line.hard_break = hard_break;
                line.end_hyphen = end_hyphen;
                line.visible_end = visible_end_of(text, line.range.start, end);
            }
        } else {
            truncated = true;
        }
        lines.truncate(max_lines);
    }

    if lines::is_ellipsis(overflow) {
        let ellipsis_width = shape_ellipsis(p, &p.base).width;
        for (i, line) in lines.iter_mut().enumerate() {
            let more = line.range.end < text.len();
            let first = i == 0;
            let last_visible = i + 1 == max_lines;
            let ellipsize = (max_lines == 1 && more)
                || (first && !more)
                || (!first && (last_visible || !more) && overflow == TextOverflow::Ellipsis);
            if !ellipsize {
                continue;
            }
            let force = more && i + 1 == max_lines;
            let indent = lines::indent_of(p, line);
            let text_width = p.tabbed_width(line.range.start..line.visible_end);
            line.ellipsis = ellipsis::calculate(
                p,
                line.range.clone(),
                width - indent,
                text_width,
                overflow,
                max_lines,
                force,
                ellipsis_width,
            );
        }
    }

    let justify = settings.text_align == TextAlign::Justify;
    for line in &mut lines {
        line.justify = justify
            && line.ellipsis.is_none()
            && line.range.end < text.len()
            && !text[..line.range.end].ends_with('\n');
    }

    let did_exceed_max_lines =
        lines.len() == max_lines && lines.last().is_some_and(|l| l.ellipsis.is_some() || l.range.end != text.len());
    let height = lines::stack(p, &mut lines);
    Laid { lines, height, did_exceed_max_lines, truncated }
}
