//! Breaking a paragraph into lines, as Android's StaticLayout does through minikin, on UAX #14 opportunities
//! ([`break_opportunities`]): mandatory breaks always end a line; between them `Strategy.Simple` fills each line
//! in turn (minikin's GreedyLineBreaker), `HighQuality` and `Balanced` choose all of a stretch's breaks at once
//! (minikin's OptimalLineBreaker: the least sum of squared room left at the lines' ends plus a penalty per line,
//! the last line free for `HighQuality` and counted for `Balanced`). A word wider than its line is split at
//! grapheme cluster boundaries; whitespace at a line's end hangs past it (it never makes a line overflow); tabs
//! go to their stops (and make every strategy `Simple`, as minikin's optimal breaker takes no tabs); justified
//! text may also squeeze its spaces by a third; a [`Hyphenator`] may hyphenate words (`Hyphens.Auto`). Text that
//! doesn't wrap is broken with an infinite width.
//!
//! What the text measures comes from a [`LineMeasure`] (the paragraph's shaped advances), so this works the same
//! on any font backend. Offsets are UTF-8 bytes.

mod greedy;
mod hyphenation;
mod opportunities;
mod optimal;

pub use hyphenation::{EndHyphenEdit, HyphenationType, Hyphenator, StartHyphenEdit, hyphenator, set_hyphenator};
pub use opportunities::{BreakOpportunity, break_opportunities};

use crate::intl::Locale;
use crate::style::Hyphens;
use crate::style::line_break::{LineBreak, Strategy};
use icu_properties::CodePointMapData;
use icu_properties::props::LineBreak as LineBreakClass;
use std::cell::OnceCell;
use std::ops::Range;

/// What line breaking measures text with: a paragraph's shaped advances (prefix sums of its clusters' advances
/// make these cheap; the fake backend's are half an em a code point).
pub trait LineMeasure {
    /// How wide `text[range]` is on a line, px: its clusters' advances as the paragraph was shaped (letter
    /// spacing included). `range` lies on cluster boundaries and holds no tab.
    fn width(&self, range: Range<usize>) -> f32;

    /// How wide `text[range]` is with hyphen edits at its ends (the hyphen a hyphenated line ends with, shaped in
    /// the font at the cut). Only asked for where a [`Hyphenator`] broke a word; measures that never meet one
    /// may leave it, taking the text as it is.
    fn width_with_hyphens(&self, range: Range<usize>, start: StartHyphenEdit, end: EndHyphenEdit) -> f32 {
        let _ = (start, end);
        self.width(range)
    }

    /// The largest font size of the text in `range`, px, times its horizontal stretch (`scale_x`): minikin's
    /// line and hyphenation penalties grow with it.
    fn font_size(&self, range: Range<usize>) -> f32;
}

/// How wide each line of a paragraph may be, px (minikin's LineWidth): the first lines' own widths, then one
/// for the rest. Compose's TextIndent makes the first line narrower by its first-line indent and the others by
/// their indent.
#[derive(Clone, PartialEq, Debug)]
pub struct LineWidths {
    first: Vec<f32>,
    rest: f32,
}

impl LineWidths {
    /// Every line `width` wide (infinite: text that doesn't wrap).
    pub fn uniform(width: f32) -> LineWidths {
        LineWidths { first: Vec::new(), rest: width }
    }
    /// The first lines as wide as `first_lines` say, the others `rest`.
    pub fn new(first_lines: impl IntoIterator<Item = f32>, rest: f32) -> LineWidths {
        LineWidths { first: first_lines.into_iter().collect(), rest }
    }
    /// `width` less a first-line indent for the first line and a rest-line indent for the others (Compose's
    /// TextIndent, px).
    pub fn indented(width: f32, first_line: f32, rest_line: f32) -> LineWidths {
        LineWidths::new([(width - first_line).max(0.0)], (width - rest_line).max(0.0))
    }
    /// How wide line `line` may be.
    pub fn at(&self, line: usize) -> f32 {
        self.first.get(line).copied().unwrap_or(self.rest)
    }
    /// The narrowest line.
    pub fn min(&self) -> f32 {
        self.first.iter().copied().fold(self.rest, f32::min)
    }
}

/// Where tabs stop, px from a line's start (Android's TabStops): the given stops, then every `increment` (Android
/// lays text out with a 20 px increment and no stops of its own).
#[derive(Clone, PartialEq, Debug)]
pub struct TabStops {
    stops: Vec<f32>,
    increment: f32,
}

impl TabStops {
    /// Android's Layout.TAB_INCREMENT.
    pub const DEFAULT_INCREMENT: f32 = 20.0;

    /// These stops (sorted), then every `increment`.
    pub fn new(stops: impl IntoIterator<Item = f32>, increment: f32) -> TabStops {
        let mut stops: Vec<f32> = stops.into_iter().collect();
        stops.sort_by(f32::total_cmp);
        TabStops { stops, increment }
    }
    /// Where a tab at `x` takes the text: the first stop past `x`, else the next multiple of the increment.
    pub fn next_tab(&self, x: f32) -> f32 {
        if let Some(&stop) = self.stops.iter().find(|&&s| s > x) {
            return stop;
        }
        ((x + self.increment) / self.increment).floor() * self.increment
    }
}

/// Every 20 px.
impl Default for TabStops {
    fn default() -> TabStops {
        TabStops::new([], TabStops::DEFAULT_INCREMENT)
    }
}

/// How a paragraph's lines are broken: the [`LineBreak`] (strategy, strictness, word break; Unspecified parts as
/// `Simple` and `Default`), hyphenation and what hyphenates, the locale (CJK strictness, phrases, dictionaries),
/// the lines' widths, whether the text is justified (spaces may squeeze, and lines aren't penalised), tab stops,
/// ranges never broken inside (inline content's placeholders, Android's replacement spans), and whether only line
/// feeds end lines (Android's StaticLayout, whose paragraphs end at `\n` and nowhere else: a line or paragraph
/// separator, a next line, a vertical tab, a form feed or a lone carriage return is then an opportunity like any
/// other; the paragraph engine breaks so).
#[derive(Clone)]
pub struct LineBreakParams<'a> {
    pub line_break: LineBreak,
    pub hyphens: Hyphens,
    pub hyphenator: Option<&'a dyn Hyphenator>,
    pub locale: Locale,
    pub widths: LineWidths,
    pub justified: bool,
    pub tab_stops: TabStops,
    pub unbreakable: &'a [Range<usize>],
    pub line_feed_breaks_only: bool,
}

impl<'a> LineBreakParams<'a> {
    /// Simple breaking in lines of `widths`, the current locale, nothing hyphenated, nothing justified.
    pub fn new(widths: LineWidths) -> LineBreakParams<'a> {
        LineBreakParams {
            line_break: LineBreak::Simple,
            hyphens: Hyphens::None,
            hyphenator: None,
            locale: Locale::current(),
            widths,
            justified: false,
            tab_stops: TabStops::default(),
            unbreakable: &[],
            line_feed_breaks_only: false,
        }
    }
}

/// A line broken off a paragraph: its text (`range`, its trailing whitespace and the mandatory break ending it
/// included, as Compose's getLineEnd), where its visible text ends (Compose's getLineEnd with visibleEnd: the
/// whitespace and line break characters at its end left out), how wide that visible text is (px, tabs at their
/// stops, hyphen edits included), the hyphen edits a hyphenation point gave its ends, and whether a mandatory
/// break ended it.
#[derive(Clone, PartialEq, Debug)]
pub struct BrokenLine {
    pub range: Range<usize>,
    pub visible_end: usize,
    pub width: f32,
    pub start_hyphen: StartHyphenEdit,
    pub end_hyphen: EndHyphenEdit,
    pub hard_break: bool,
}

/// Whether `c` hangs at a line's end, never making it overflow (minikin's isLineEndSpace, Compose's
/// LayoutHelper.isLineEndSpace): spaces but the figure space and no-break ones, the line feed.
pub fn is_line_end_space(c: char) -> bool {
    matches!(c, ' ' | '\n' | '\u{1680}' | '\u{205F}' | '\u{3000}')
        || ('\u{2000}'..='\u{200A}').contains(&c) && c != '\u{2007}'
}

/// Whether `c` stretches when a line is justified (Android's TextLine: the space).
pub fn is_stretchable_space(c: char) -> bool {
    c == ' '
}

/// Whether `c` ends a line by itself (UAX #14's BK, CR, LF and NL).
fn is_mandatory_break_char(c: char) -> bool {
    matches!(
        CodePointMapData::<LineBreakClass>::new().get(c),
        LineBreakClass::MandatoryBreak
            | LineBreakClass::CarriageReturn
            | LineBreakClass::LineFeed
            | LineBreakClass::NextLine
    )
}

/// Where the visible text of `text[start..end]` ends: hanging whitespace and line break characters at its end
/// left out.
pub fn visible_end(text: &str, start: usize, end: usize) -> usize {
    let mut visible = end;
    for (i, c) in text[start..end].char_indices().rev() {
        if !(is_line_end_space(c) || is_mandatory_break_char(c)) {
            break;
        }
        visible = start + i;
    }
    visible
}

/// The spaces that stretch when the line `text[start..end]` is justified: the stretchable ones before its
/// visible end (Android's TextLine.justify).
pub fn stretchable_spaces(text: &str, start: usize, end: usize) -> Vec<usize> {
    let end = visible_end(text, start, end);
    text[start..end].char_indices().filter(|&(_, c)| is_stretchable_space(c)).map(|(i, _)| start + i).collect()
}

/// The text a line breaker works on: its measure, its parameters, its tabs, its graphemes.
pub(crate) struct Context<'a> {
    pub(crate) text: &'a str,
    pub(crate) measure: &'a dyn LineMeasure,
    pub(crate) params: &'a LineBreakParams<'a>,
    tabs: Vec<usize>,
    graphemes: OnceCell<Vec<usize>>,
}

impl<'a> Context<'a> {
    fn new(text: &'a str, measure: &'a dyn LineMeasure, params: &'a LineBreakParams<'a>) -> Context<'a> {
        let tabs = text.match_indices('\t').map(|(i, _)| i).collect();
        Context { text, measure, params, tabs, graphemes: OnceCell::new() }
    }

    /// How wide line `line` may be.
    pub(crate) fn limit(&self, line: usize) -> f32 {
        self.params.widths.at(line)
    }

    /// Where the visible text of `start..end` ends.
    pub(crate) fn visible_end(&self, start: usize, end: usize) -> usize {
        visible_end(self.text, start, end)
    }

    /// Whether `start..end` holds a tab.
    pub(crate) fn has_tab(&self, start: usize, end: usize) -> bool {
        let i = self.tabs.partition_point(|&t| t < start);
        self.tabs.get(i).is_some_and(|&t| t < end)
    }

    /// How wide `start..end` is as a line starting at `start`: tabs at their stops, hyphen edits at its ends.
    pub(crate) fn width(&self, start: usize, end: usize, start_edit: StartHyphenEdit, end_edit: EndHyphenEdit) -> f32 {
        if start >= end && start_edit == StartHyphenEdit::NoEdit && end_edit == EndHyphenEdit::NoEdit {
            return 0.0;
        }
        let piece = |range: Range<usize>, s: StartHyphenEdit, e: EndHyphenEdit| {
            if s == StartHyphenEdit::NoEdit && e == EndHyphenEdit::NoEdit {
                if range.is_empty() { 0.0 } else { self.measure.width(range) }
            } else {
                self.measure.width_with_hyphens(range, s, e)
            }
        };
        let first_tab = self.tabs.partition_point(|&t| t < start);
        let mut x = 0.0;
        let mut piece_start = start;
        let mut s = start_edit;
        for &tab in self.tabs[first_tab..].iter().take_while(|&&t| t < end) {
            x += piece(piece_start..tab, s, EndHyphenEdit::NoEdit);
            x = self.params.tab_stops.next_tab(x);
            piece_start = tab + 1;
            s = StartHyphenEdit::NoEdit;
        }
        x + piece(piece_start..end, s, end_edit)
    }

    /// Grapheme cluster boundaries of the text.
    pub(crate) fn graphemes(&self) -> &[usize] {
        self.graphemes.get_or_init(|| opportunities::grapheme_boundaries(self.text))
    }

    /// Where a line from `start` that can't break before `end` is split by graphemes: after as many grapheme
    /// clusters as fit in `limit`, at least one.
    pub(crate) fn grapheme_break(&self, start: usize, end: usize, start_edit: StartHyphenEdit, limit: f32) -> usize {
        let graphemes = self.graphemes();
        let first = graphemes.partition_point(|&g| g <= start);
        let mut best = None;
        for &g in &graphemes[first..] {
            if g > end {
                break;
            }
            if best.is_some() && self.width(start, g, start_edit, EndHyphenEdit::NoEdit) > limit {
                break;
            }
            best = Some(g);
        }
        best.unwrap_or(end)
    }

    /// The hyphenation points of the word between the breaks `start` and `end` (absolute offsets): none unless
    /// `Hyphens.Auto` has a hyphenator.
    pub(crate) fn hyphenation_points(
        &self,
        start: usize,
        end: usize,
        in_email_or_url: bool,
    ) -> Vec<(usize, HyphenationType)> {
        let Some(hyphenator) = self.params.hyphenator.filter(|_| self.params.hyphens == Hyphens::Auto) else {
            return Vec::new();
        };
        if in_email_or_url {
            return Vec::new();
        }
        let (word_start, word_end) = opportunities::hyphenation_word(self.text, start, end);
        let word = &self.text[word_start..word_end];
        if word.is_empty() || word.chars().count() > LONGEST_HYPHENATED_WORD {
            return Vec::new();
        }
        let mut points: Vec<(usize, HyphenationType)> = hyphenator
            .hyphenate(word, &self.params.locale)
            .into_iter()
            .filter(|&(o, t)| o > 0 && o < word.len() && word.is_char_boundary(o) && t != HyphenationType::DontBreak)
            .map(|(o, t)| (word_start + o, t))
            .collect();
        points.sort_by_key(|p| p.0);
        points.dedup_by_key(|p| p.0);
        points
    }
}

/// minikin's limit on the words it hyphenates, characters.
const LONGEST_HYPHENATED_WORD: usize = 45;

/// A stretch of text between mandatory breaks, broken on its own (minikin's paragraph): its soft opportunities
/// before its end, whether its last word is in an email address or URL, the phrase fallback points in it, the
/// index of its first line among the paragraph's.
pub(crate) struct Segment<'s> {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) ends_email_or_url: bool,
    pub(crate) breaks: &'s [BreakOpportunity],
    pub(crate) fallback: &'s [usize],
    pub(crate) first_line: usize,
}

/// A line as a breaker decided it: where it starts and ends and its hyphen edits.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct LineSpec {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) start_edit: StartHyphenEdit,
    pub(crate) end_edit: EndHyphenEdit,
}

/// Breaks `text` into lines measured by `measure` with `params`: mandatory breaks first (line feeds only where
/// `params` says so), then each stretch
/// between them by the strategy (`Simple`: greedy; `HighQuality` and `Balanced`: optimal, or greedy where it has
/// tabs). Empty text is one empty line; text ending in a mandatory break has an empty last line after it.
pub fn break_lines(text: &str, measure: &dyn LineMeasure, params: &LineBreakParams<'_>) -> Vec<BrokenLine> {
    let line_break = params.line_break;
    let mut points = break_opportunities(text, line_break, &params.locale);
    if params.line_feed_breaks_only {
        for p in &mut points {
            p.mandatory = p.mandatory && text[..p.offset].ends_with('\n');
        }
    }
    points.retain(|p| p.mandatory || p.offset == text.len() || !inside(params.unbreakable, p.offset));
    let mut fallback = opportunities::phrase_fallback(text, line_break, &params.locale);
    fallback.retain(|&f| !inside(params.unbreakable, f));
    let cx = Context::new(text, measure, params);

    let mut lines: Vec<BrokenLine> = Vec::new();
    let mut start = 0;
    let mut soft_from = 0;
    for (i, point) in points.iter().enumerate() {
        if !point.mandatory && point.offset < text.len() {
            continue;
        }
        let end = point.offset;
        let fallback_from = fallback.partition_point(|&f| f <= start);
        let fallback_to = fallback.partition_point(|&f| f < end);
        let segment = Segment {
            start,
            end,
            ends_email_or_url: point.email_or_url,
            breaks: &points[soft_from..i],
            fallback: &fallback[fallback_from..fallback_to],
            first_line: lines.len(),
        };
        let optimal =
            matches!(line_break.strategy, Strategy::HighQuality | Strategy::Balanced) && !cx.has_tab(start, end);
        let specs = if optimal { optimal::break_segment(&cx, &segment) } else { greedy::break_segment(&cx, &segment) };
        let count = specs.len();
        for (k, spec) in specs.into_iter().enumerate() {
            let visible = cx.visible_end(spec.start, spec.end);
            lines.push(BrokenLine {
                range: spec.start..spec.end,
                visible_end: visible,
                width: cx.width(spec.start, visible, spec.start_edit, spec.end_edit),
                start_hyphen: spec.start_edit,
                end_hyphen: spec.end_edit,
                hard_break: point.mandatory && k + 1 == count,
            });
        }
        start = end;
        soft_from = i + 1;
    }
    if lines.is_empty() || lines.last().is_some_and(|l| l.hard_break) {
        lines.push(BrokenLine {
            range: text.len()..text.len(),
            visible_end: text.len(),
            width: 0.0,
            start_hyphen: StartHyphenEdit::NoEdit,
            end_hyphen: EndHyphenEdit::NoEdit,
            hard_break: false,
        });
    }
    lines
}

/// Whether `offset` lies strictly inside one of `ranges`.
fn inside(ranges: &[Range<usize>], offset: usize) -> bool {
    ranges.iter().any(|r| r.start < offset && offset < r.end)
}

/// The narrowest a paragraph can be laid out without splitting a word: its widest stretch between break
/// opportunities, its hanging whitespace left out (Compose's minIntrinsicWidth measures with it).
pub fn min_intrinsic_width(
    text: &str,
    measure: &dyn LineMeasure,
    line_break: LineBreak,
    locale: &Locale,
    unbreakable: &[Range<usize>],
) -> f32 {
    let params = LineBreakParams {
        line_break,
        unbreakable,
        locale: locale.clone(),
        ..LineBreakParams::new(LineWidths::uniform(f32::INFINITY))
    };
    let cx = Context::new(text, measure, &params);
    let mut start = 0;
    let mut widest: f32 = 0.0;
    for point in break_opportunities(text, line_break, locale) {
        if !point.mandatory && point.offset < text.len() && inside(unbreakable, point.offset) {
            continue;
        }
        let visible = cx.visible_end(start, point.offset);
        widest = widest.max(cx.width(start, visible, StartHyphenEdit::NoEdit, EndHyphenEdit::NoEdit));
        start = point.offset;
    }
    widest
}
