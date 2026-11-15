//! `Strategy.Simple`: each line filled in turn (minikin's GreedyLineBreaker). When the word reaching a break
//! doesn't fit, the line breaks at the best of: a hyphenation point of that word that fits, the last break
//! before the word (a break inside an email address or URL only if the line is already inside one: one that
//! starts on a line goes to the next whole if it can, and the next line then breaks inside it rather than
//! splitting its characters), a phrase fallback point, or the last grapheme cluster that fits.

use super::{Context, EndHyphenEdit, LineSpec, Segment, StartHyphenEdit};

/// The lines of a segment.
pub(crate) fn break_segment(cx: &Context<'_>, segment: &Segment<'_>) -> Vec<LineSpec> {
    let mut lines = Vec::new();
    let mut line_start = segment.start;
    let mut start_edit = StartHyphenEdit::NoEdit;
    // The last break a line may end at: its offset, and whether it is inside an email address or URL; and the
    // last break inside one passed over for a break before it, which the line after that break may end at.
    let mut previous: Option<(usize, bool)> = None;
    let mut passed_over: Option<(usize, bool)> = None;
    let mut word_start = segment.start;
    let candidates = segment.breaks.iter().map(|b| (b.offset, b.inside_email_or_url, b.email_or_url)).chain([(
        segment.end,
        false,
        segment.ends_email_or_url,
    )]);
    for (offset, inside_email_or_url, word_in_email_or_url) in candidates {
        'fit: loop {
            let limit = cx.limit(segment.first_line + lines.len());
            let visible = cx.visible_end(line_start, offset);
            if cx.width(line_start, visible, start_edit, EndHyphenEdit::NoEdit) <= limit {
                break;
            }
            let (end, end_edit, next_start_edit) = 'chosen: {
                // A hyphenation point of the word that overflows.
                let hyphen = cx
                    .hyphenation_points(word_start, offset, word_in_email_or_url)
                    .into_iter()
                    .filter(|&(h, _)| h > line_start)
                    .take_while(|&(h, t)| cx.width(line_start, h, start_edit, t.end_edit()) <= limit)
                    .last();
                if let Some((h, t)) = hyphen {
                    previous = None;
                    passed_over = None;
                    break 'chosen (h, t.end_edit(), t.start_edit());
                }
                // The last break before it.
                if let Some((p, _)) = previous.take() {
                    previous = passed_over.take();
                    break 'chosen (p, EndHyphenEdit::NoEdit, StartHyphenEdit::NoEdit);
                }
                // A phrase too long for a line: where it would break but for phrases.
                let fallback = segment
                    .fallback
                    .iter()
                    .copied()
                    .filter(|&f| f > line_start && f < offset)
                    .take_while(|&f| {
                        cx.width(line_start, cx.visible_end(line_start, f), start_edit, EndHyphenEdit::NoEdit) <= limit
                    })
                    .last();
                if let Some(f) = fallback {
                    break 'chosen (f, EndHyphenEdit::NoEdit, StartHyphenEdit::NoEdit);
                }
                // A word wider than the line: as many grapheme clusters as fit. One that can't be split (a
                // single cluster, a placeholder) overflows its line, which breaks after it as after any word.
                let split = cx.grapheme_break(line_start, visible, start_edit, limit);
                if split >= visible {
                    break 'fit;
                }
                (split, EndHyphenEdit::NoEdit, StartHyphenEdit::NoEdit)
            };
            lines.push(LineSpec { start: line_start, end, start_edit, end_edit });
            line_start = end;
            start_edit = next_start_edit;
        }
        if offset < segment.end {
            // A break inside an email address or URL replaces one outside it only when that is inside one too.
            if previous.is_none_or(|(_, p)| p || !inside_email_or_url) {
                previous = Some((offset, inside_email_or_url));
                passed_over = None;
            } else {
                passed_over = Some((offset, inside_email_or_url));
            }
            word_start = offset;
        }
    }
    lines.push(LineSpec { start: line_start, end: segment.end, start_edit, end_edit: EndHyphenEdit::NoEdit });
    lines
}
