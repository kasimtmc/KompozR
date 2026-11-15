//! `Strategy.HighQuality` and `Strategy.Balanced`: all of a stretch's breaks chosen at once (minikin's
//! OptimalLineBreaker, a Knuth–Plass-like dynamic programme). Candidates are the break opportunities (a penalty
//! for those inside email addresses and URLs), a hyphenator's points (a hyphenation penalty), and, inside a word
//! wider than the narrowest line, phrase fallback points and grapheme cluster boundaries as desperate ones. A
//! line scores the square of the room it leaves (an overfull one scores as good as never; a justified one may
//! squeeze its spaces by a third, at four times the score), every line adds a penalty, and the last line is free
//! for `HighQuality` (but four times a hyphen's penalty) while `Balanced` counts it like the others.

use super::{Context, EndHyphenEdit, HyphenationType, LineSpec, Segment, StartHyphenEdit};
use crate::style::line_break::Strategy;

/// What an overfull line scores.
const SCORE_OVERFULL: f64 = 1e12;
/// What breaking inside a word at a grapheme boundary costs.
const SCORE_DESPERATE: f64 = 1e10;
/// What breaking inside a phrase where words would break costs.
const SCORE_FALLBACK: f64 = 1e6;
/// How much more a hyphen on the last line costs.
const LAST_LINE_PENALTY_MULTIPLIER: f64 = 4.0;
/// A line's penalty against the hyphenation penalty.
const LINE_PENALTY_MULTIPLIER: f64 = 2.0;
/// How much more a line squeezing its spaces scores.
const SHRINK_PENALTY_MULTIPLIER: f64 = 4.0;
/// How much of its width a space may give up in justified text.
const SHRINKABILITY: f64 = 1.0 / 3.0;

/// A place a line may end (minikin's Candidate): the hyphenation type of a hyphenation point, the penalty of
/// breaking here, whether it splits a word between grapheme clusters (or a phrase where words would break).
#[derive(Clone, Copy, Debug)]
struct Candidate {
    offset: usize,
    hyphenation: Option<HyphenationType>,
    penalty: f64,
    desperate: bool,
}

impl Candidate {
    fn end_edit(&self) -> EndHyphenEdit {
        self.hyphenation.map_or(EndHyphenEdit::NoEdit, HyphenationType::end_edit)
    }
    fn start_edit(&self) -> StartHyphenEdit {
        self.hyphenation.map_or(StartHyphenEdit::NoEdit, HyphenationType::start_edit)
    }
}

/// minikin's computePenalties: the hyphenation penalty for text of `font_size` (px times its stretch) and the
/// line penalty, for lines `first_width` wide.
fn penalties(font_size: f32, first_width: f32, justified: bool) -> (f64, f64) {
    let hyphen = 0.5 * font_size as f64 * first_width as f64;
    if justified {
        // Hyphens more readily in justified text, and no line penalty.
        (hyphen * 0.25, 0.0)
    } else {
        (hyphen, hyphen * LINE_PENALTY_MULTIPLIER)
    }
}

/// The lines of a segment.
pub(crate) fn break_segment(cx: &Context<'_>, segment: &Segment<'_>) -> Vec<LineSpec> {
    let text = cx.text;
    let params = cx.params;
    let first_width = cx.limit(segment.first_line);
    let whole = cx.visible_end(segment.start, segment.end);
    // What fits on its first line is that line.
    if cx.width(segment.start, whole, StartHyphenEdit::NoEdit, EndHyphenEdit::NoEdit) <= first_width {
        let edit = StartHyphenEdit::NoEdit;
        return vec![LineSpec {
            start: segment.start,
            end: segment.end,
            start_edit: edit,
            end_edit: EndHyphenEdit::NoEdit,
        }];
    }
    let justified = params.justified;
    let balanced = params.line_break.strategy == Strategy::Balanced;
    let (_, line_penalty) = penalties(cx.measure.font_size(segment.start..segment.end), first_width, justified);
    let min_width = params.widths.min();

    // The candidates, in order: the segment's start, then each word's inner points and its break.
    let mut candidates = vec![Candidate { offset: segment.start, hyphenation: None, penalty: 0.0, desperate: false }];
    let words = segment.breaks.iter().map(|b| (b.offset, b.inside_email_or_url, b.email_or_url)).chain([(
        segment.end,
        false,
        segment.ends_email_or_url,
    )]);
    let mut word_start = segment.start;
    for (offset, inside_email_or_url, email_or_url) in words {
        let size_at = text[word_start..offset].chars().next_back().map_or(0, char::len_utf8);
        let (hyphen_penalty, _) = penalties(cx.measure.font_size(offset - size_at..offset), first_width, justified);
        let mut inner: Vec<Candidate> = cx
            .hyphenation_points(word_start, offset, email_or_url)
            .into_iter()
            .map(|(h, t)| Candidate { offset: h, hyphenation: Some(t), penalty: hyphen_penalty, desperate: false })
            .collect();
        let visible = cx.visible_end(word_start, offset);
        if cx.width(word_start, visible, StartHyphenEdit::NoEdit, EndHyphenEdit::NoEdit) > min_width {
            for &g in cx.graphemes().iter().filter(|&&g| g > word_start && g < offset) {
                let penalty = if segment.fallback.binary_search(&g).is_ok() { SCORE_FALLBACK } else { SCORE_DESPERATE };
                inner.push(Candidate { offset: g, hyphenation: None, penalty, desperate: true });
            }
        }
        // A hyphenation point and a desperate one at the same place: the hyphenation point.
        inner.sort_by(|a, b| a.offset.cmp(&b.offset).then(a.penalty.total_cmp(&b.penalty)));
        inner.dedup_by_key(|c| c.offset);
        candidates.extend(inner);
        let badness = if inside_email_or_url { 1.0 } else { 0.0 };
        candidates.push(Candidate { offset, hyphenation: None, penalty: hyphen_penalty * badness, desperate: false });
        word_start = offset;
    }

    // The width of a line between two candidates, and the spaces it could squeeze.
    let space_width = text[segment.start..segment.end]
        .char_indices()
        .find(|&(_, c)| is_word_space(c))
        .map_or(0.0, |(i, c)| cx.measure.width(segment.start + i..segment.start + i + c.len_utf8()) as f64);
    let max_shrink = if justified { SHRINKABILITY * space_width } else { 0.0 };
    let line_width = |j: &Candidate, i: &Candidate| {
        let visible = cx.visible_end(j.offset, i.offset);
        cx.width(j.offset, visible, j.start_edit(), i.end_edit()) as f64
    };
    let spaces = |j: &Candidate, i: &Candidate| {
        let visible = cx.visible_end(j.offset, i.offset);
        text[j.offset..visible].chars().filter(|&c| is_word_space(c)).count() as f64
    };

    // minikin's computeBreaks (its scores in doubles, so a desperate break's 1e10 doesn't swallow the lines'
    // differences): each candidate's best score, the candidate before it, its line number.
    let mut best_of: Vec<(f64, usize, usize)> = Vec::with_capacity(candidates.len());
    best_of.push((0.0, 0, 0));
    let mut active = 0;
    let last = candidates.len() - 1;
    for i in 1..candidates.len() {
        let at_end = i == last;
        let mut best = f64::MAX;
        let mut best_prev = 0;
        let mut line_number_last = best_of[active].2;
        let mut width = cx.limit(segment.first_line + line_number_last) as f64;
        let mut best_hope = 0.0;
        let first = active;
        for j in first..i {
            let line_number = best_of[j].2;
            if line_number != line_number_last {
                let width_new = cx.limit(segment.first_line + line_number) as f64;
                if width_new != width {
                    best_hope = 0.0;
                    width = width_new;
                }
                line_number_last = line_number;
            }
            let j_score = best_of[j].0;
            if j_score + best_hope >= best {
                continue;
            }
            let delta = width - line_width(&candidates[j], &candidates[i]);
            let mut width_score = 0.0;
            let mut additional_penalty = 0.0;
            if (at_end || !justified) && delta < 0.0 {
                width_score = SCORE_OVERFULL;
            } else if at_end && !balanced {
                // The last line is free, but a hyphen (or a break in a URL) starting it. A word split before it
                // costs what it costs anywhere: minikin's multiplier is for hyphens, and on a desperate break it
                // would rather add a line than end a split word on the last one.
                if !candidates[j].desperate {
                    additional_penalty = LAST_LINE_PENALTY_MULTIPLIER * candidates[j].penalty;
                }
            } else {
                width_score = delta * delta;
                if delta < 0.0 {
                    if -delta < max_shrink * spaces(&candidates[j], &candidates[i]) {
                        width_score *= SHRINK_PENALTY_MULTIPLIER;
                    } else {
                        width_score = SCORE_OVERFULL;
                    }
                }
            }
            if delta < 0.0 {
                active = j + 1;
            } else {
                best_hope = width_score;
            }
            let score = j_score + width_score + additional_penalty;
            if score <= best {
                best = score;
                best_prev = j;
            }
        }
        best_of.push((best + candidates[i].penalty + line_penalty, best_prev, best_of[best_prev].2 + 1));
    }

    // Back from the end along the best breaks.
    let mut ends = Vec::new();
    let mut i = last;
    while i > 0 {
        ends.push(i);
        i = best_of[i].1;
    }
    ends.reverse();
    let mut lines = Vec::with_capacity(ends.len());
    let mut start = 0;
    for i in ends {
        let (from, to) = (&candidates[start], &candidates[i]);
        let end = if i == last { segment.end } else { to.offset };
        lines.push(LineSpec { start: from.offset, end, start_edit: from.start_edit(), end_edit: to.end_edit() });
        start = i;
    }
    lines
}

/// A space justification stretches and squeezes (minikin's isWordSpace).
fn is_word_space(c: char) -> bool {
    c == ' ' || c == '\u{00A0}'
}
