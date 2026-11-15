//! Ellipsis as Android's StaticLayout computes it (calculateEllipsis): the end of the last line that shows
//! (the start or the middle of a text's only line) replaced by "…" so the line fits.

use super::intrinsics::ParagraphIntrinsics;
use crate::segment::grapheme_breaks;
use crate::style::TextOverflow;
use std::ops::Range;

/// The ellipsis, U+2026.
pub(crate) const ELLIPSIS: &str = "\u{2026}";

/// Where an ellipsis goes on a line: the text it replaces (its first character is drawn as "…", the rest not at
/// all) and which end it cut.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Ellipsis {
    pub(crate) range: Range<usize>,
    pub(crate) overflow: TextOverflow,
}

/// The ellipsis of the line `line` (its whole text: trailing whitespace and `\n` included) that may be
/// `avail` px wide and is `text_width` px wide (its visible text), `overflow` the kind asked for, `max_lines` the
/// lines allowed, `force` set when more text follows the last line allowed (an end ellipsis then replaces at least
/// the line's last character). None where it fits and isn't forced, or for a start or middle ellipsis on more than
/// one line (Android ellipsizes those on one line only).
#[allow(clippy::too_many_arguments)]
pub(crate) fn calculate(
    p: &ParagraphIntrinsics,
    line: Range<usize>,
    avail: f32,
    text_width: f32,
    overflow: TextOverflow,
    max_lines: usize,
    force: bool,
    ellipsis_width: f32,
) -> Option<Ellipsis> {
    if text_width <= avail && !force {
        return None;
    }
    let text = &p.text[line.clone()];
    // The line's characters' widths, a cluster's on its first character (Android's measured widths per char).
    let bounds: Vec<usize> = grapheme_breaks(text).map(|b| line.start + b).collect();
    let pieces: Vec<(usize, usize, f32)> = bounds.windows(2).map(|w| (w[0], w[1], p.width(w[0]..w[1]))).collect();
    let n = pieces.len();
    let avail_left = avail - ellipsis_width;
    let (start, count) = match overflow {
        TextOverflow::Ellipsis => {
            let mut sum = 0.0;
            let mut i = 0;
            while i < n {
                let w = pieces[i].2;
                if w + sum + ellipsis_width > avail {
                    break;
                }
                sum += w;
                i += 1;
            }
            let (mut s, mut c) = (i, n - i);
            if force && c == 0 && n > 0 {
                s = n - 1;
                c = 1;
            }
            (s, c)
        }
        TextOverflow::StartEllipsis if max_lines == 1 => {
            let mut sum = 0.0;
            let mut i = n;
            while i > 0 {
                let w = pieces[i - 1].2;
                if w + sum + ellipsis_width > avail {
                    while i < n && pieces[i].2 == 0.0 {
                        i += 1;
                    }
                    break;
                }
                sum += w;
                i -= 1;
            }
            (0, i)
        }
        TextOverflow::MiddleEllipsis if max_lines == 1 => {
            let ravail = avail_left / 2.0;
            let mut rsum = 0.0;
            let mut right = n;
            while right > 0 {
                let w = pieces[right - 1].2;
                if w + rsum > ravail {
                    while right < n && pieces[right].2 == 0.0 {
                        right += 1;
                    }
                    break;
                }
                rsum += w;
                right -= 1;
            }
            let lavail = avail_left - rsum;
            let mut lsum = 0.0;
            let mut left = 0;
            while left < right {
                let w = pieces[left].2;
                if w + lsum > lavail {
                    break;
                }
                lsum += w;
                left += 1;
            }
            (left, right - left)
        }
        _ => return None,
    };
    if count == 0 {
        return None;
    }
    let from = pieces[start].0;
    let to = pieces[start + count - 1].1;
    Some(Ellipsis { range: from..to, overflow })
}
