//! Bidirectional text (Unicode's UAX #9, through unicode-bidi on ICU's Unicode data): which way a paragraph goes
//! (Compose's TextDirection resolved as Android's first-strong heuristics), the embedding level of every byte
//! (explicit embeddings, overrides and isolates included), the level runs a paragraph is shaped in, each line put
//! in visual order (rule L1's trailing whitespace back at the paragraph's level, then L2), and what carets and
//! shapers need from it: a run's direction, level boundaries, Android's primary and secondary horizontal
//! positions, mirrored characters.
//!
//! Offsets are UTF-8 byte offsets into the paragraph's text; levels are UAX #9's (even: left to right, odd: right
//! to left).

use crate::intl::{Locale, LocaleList};
use crate::style::{ResolvedTextDirection, TextDirection};
use icu_properties::props::{BidiClass as IcuBidiClass, BidiMirroringGlyph, BidiPairedBracketType};
use icu_properties::{CodePointMapData, CodePointMapDataBorrowed};
use std::ops::Range;
use unicode_bidi::data_source::BidiMatchedOpeningBracket;
use unicode_bidi::{BidiClass, BidiDataSource, Level, ParagraphBidiInfo};

/// ICU's Bidi_Class and bracket pairs (Bidi_Paired_Bracket_Type with Bidi_Mirroring_Glyph), so bidi reads the same
/// Unicode version as line breaking and segmentation do.
#[derive(Clone, Copy)]
struct IcuBidiData {
    classes: CodePointMapDataBorrowed<'static, IcuBidiClass>,
    mirroring: CodePointMapDataBorrowed<'static, BidiMirroringGlyph>,
}

impl IcuBidiData {
    const fn new() -> IcuBidiData {
        IcuBidiData { classes: CodePointMapData::<IcuBidiClass>::new(), mirroring: CodePointMapData::new() }
    }
}

impl BidiDataSource for IcuBidiData {
    fn bidi_class(&self, c: char) -> BidiClass {
        self.classes.get(c).into()
    }

    /// The opening bracket of `c`'s pair, canonically equivalent brackets made one (rule BD16: U+2329 and U+232A
    /// pair as U+3008 and U+3009 do).
    fn bidi_matched_opening_bracket(&self, c: char) -> Option<BidiMatchedOpeningBracket> {
        let m = self.mirroring.get(c);
        let (opening, is_open) = match m.paired_bracket_type {
            BidiPairedBracketType::Open => (c, true),
            BidiPairedBracketType::Close => (m.mirroring_glyph?, false),
            _ => return None,
        };
        let opening = if opening == '\u{2329}' { '\u{3008}' } else { opening };
        Some(BidiMatchedOpeningBracket { opening, is_open })
    }
}

/// The character `c` is drawn as in right-to-left text (Unicode's Bidi_Mirroring_Glyph: `)` for `(`, `»` for
/// `«`), if it has one. Shapers mirror right-to-left runs themselves (HarfBuzz's `rtlm`); this is for what draws
/// characters without one.
pub fn mirrored_char(c: char) -> Option<char> {
    let m = CodePointMapData::<BidiMirroringGlyph>::new().get(c);
    if m.mirrored { m.mirroring_glyph } else { None }
}

/// Whether `c` is drawn mirrored in right-to-left text (Unicode's Bidi_Mirrored), with a mirror image of its own
/// or not (`∛` has none: a font's `rtlm` feature or the renderer flips it).
pub fn is_mirrored(c: char) -> bool {
    CodePointMapData::<BidiMirroringGlyph>::new().get(c).mirrored
}

/// The direction of `text`'s first strong character (rules P2 and P3, as Android's first-strong text direction
/// heuristics): left to right for a Bidi_Class L, right to left for R or AL, skipping what isolates hold (LRI, RLI
/// or FSI up to its PDI, or to the end); None if there is no strong character.
pub fn first_strong_direction(text: &str) -> Option<ResolvedTextDirection> {
    let data = IcuBidiData::new();
    let mut open_isolates = 0usize;
    for c in text.chars() {
        match data.bidi_class(c) {
            BidiClass::LRI | BidiClass::RLI | BidiClass::FSI => open_isolates += 1,
            BidiClass::PDI => open_isolates = open_isolates.saturating_sub(1),
            BidiClass::L if open_isolates == 0 => return Some(ResolvedTextDirection::Ltr),
            BidiClass::R | BidiClass::AL if open_isolates == 0 => return Some(ResolvedTextDirection::Rtl),
            _ => {}
        }
    }
    None
}

/// Whether `locale`'s script is written right to left (ICU's ULocale.isRightToLeft, what Android's
/// TextUtils.getLayoutDirectionFromLocale asks): by its script subtag, else by the script its language is likely
/// written in (`ar`, `fa`, `he`, `ur`… right to left; `pa-PK`, `az-IR`, `uz-AF` in Arabic letters too).
pub fn locale_is_rtl(locale: &Locale) -> bool {
    const RTL_SCRIPTS: &[&str] = &[
        "Adlm", "Arab", "Aran", "Avst", "Chrs", "Cprt", "Elym", "Gara", "Hebr", "Hung", "Khar", "Lydi", "Mand", "Mani",
        "Mend", "Merc", "Mero", "Narb", "Nbat", "Nkoo", "Orkh", "Ougr", "Palm", "Phli", "Phlp", "Phnx", "Prti", "Rohg",
        "Samr", "Sarb", "Sogd", "Sogo", "Syrc", "Syre", "Syrj", "Syrn", "Thaa", "Yezi",
    ];
    // CLDR's likely subtags: the languages whose likely script is one of those.
    const RTL_LANGUAGES: &[&str] = &[
        "ar", "arc", "ckb", "dv", "fa", "glk", "he", "iw", "ji", "khw", "ks", "lrc", "mzn", "nqo", "pnb", "ps", "sd",
        "sdh", "skr", "syr", "ug", "ur", "yi", "bal", "bgn", "bqi", "luz", "prd", "rhg", "sam", "ary", "arz", "aeb",
        "apc", "acm", "ajp", "lki", "prs",
    ];
    let script = locale.script();
    if !script.is_empty() {
        return RTL_SCRIPTS.contains(&script);
    }
    match (locale.language(), locale.region()) {
        ("pa", "PK") | ("az", "IR") | ("uz", "AF") => true,
        (language, _) => RTL_LANGUAGES.contains(&language),
    }
}

/// Which way a paragraph of `text` goes for a text direction (Compose's resolveTextDirectionHeuristics with
/// Android's heuristics): `Ltr` and `Rtl` whatever it holds; `ContentOrLtr` / `ContentOrRtl` by its first strong
/// character, else left to right / right to left; `Content` (and Unspecified, which a style resolves before it
/// gets here) by its first strong character, else the way the first of `locales` (the current one if none) is
/// written.
pub fn resolve_paragraph_direction(
    text: &str,
    direction: TextDirection,
    locales: &LocaleList,
) -> ResolvedTextDirection {
    match direction {
        TextDirection::Ltr => ResolvedTextDirection::Ltr,
        TextDirection::Rtl => ResolvedTextDirection::Rtl,
        TextDirection::ContentOrLtr => first_strong_direction(text).unwrap_or(ResolvedTextDirection::Ltr),
        TextDirection::ContentOrRtl => first_strong_direction(text).unwrap_or(ResolvedTextDirection::Rtl),
        TextDirection::Content | TextDirection::Unspecified => first_strong_direction(text).unwrap_or_else(|| {
            if locale_is_rtl(&locales.first_or_current()) {
                ResolvedTextDirection::Rtl
            } else {
                ResolvedTextDirection::Ltr
            }
        }),
    }
}

/// The paragraph level of a direction: 0 left to right, 1 right to left.
pub fn direction_level(direction: ResolvedTextDirection) -> u8 {
    match direction {
        ResolvedTextDirection::Ltr => 0,
        ResolvedTextDirection::Rtl => 1,
    }
}

/// The direction of a level: right to left when odd.
pub fn level_direction(level: u8) -> ResolvedTextDirection {
    if level % 2 == 1 { ResolvedTextDirection::Rtl } else { ResolvedTextDirection::Ltr }
}

/// A stretch of text at one embedding level: in logical order where a paragraph's level runs are asked for, in
/// visual order (left to right on screen) in a [`BidiLine`]. A right-to-left run's characters are shown from its
/// end to its start.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BidiRun {
    pub range: Range<usize>,
    pub level: u8,
}

impl BidiRun {
    /// Whether its characters go right to left (its level is odd).
    pub fn is_rtl(&self) -> bool {
        self.level % 2 == 1
    }
    /// Which way its characters go.
    pub fn direction(&self) -> ResolvedTextDirection {
        level_direction(self.level)
    }
}

/// A paragraph's bidi analysis (UAX #9 rules P2–I2: explicit embeddings, overrides and isolates, weak and neutral
/// types, bracket pairs, implicit levels): its direction and the level of every byte of its text. Lines of it
/// ([`line`](BidiParagraph::line)) are put in visual order. A paragraph is text up to and including a paragraph
/// separator (`\n`); Compose's paragraphs end there.
#[derive(Clone, PartialEq, Debug)]
pub struct BidiParagraph {
    direction: ResolvedTextDirection,
    levels: Vec<u8>,
    classes: Vec<BidiClass>,
    pure_ltr: bool,
}

impl BidiParagraph {
    /// `text` analysed in the direction a text direction gives it ([`resolve_paragraph_direction`]).
    pub fn new(text: &str, direction: TextDirection, locales: &LocaleList) -> BidiParagraph {
        BidiParagraph::with_direction(text, resolve_paragraph_direction(text, direction, locales))
    }

    /// `text` analysed as a paragraph going `direction`.
    pub fn with_direction(text: &str, direction: ResolvedTextDirection) -> BidiParagraph {
        let level = match direction {
            ResolvedTextDirection::Ltr => Level::ltr(),
            ResolvedTextDirection::Rtl => Level::rtl(),
        };
        let info = ParagraphBidiInfo::new_with_data_source(&IcuBidiData::new(), text, Some(level));
        let levels: Vec<u8> = info.levels.iter().map(Level::number).collect();
        let pure_ltr = levels.iter().all(|l| l % 2 == 0);
        BidiParagraph { direction, levels, classes: info.original_classes, pure_ltr }
    }

    /// Which way the paragraph goes (Compose's getParagraphDirection).
    pub fn direction(&self) -> ResolvedTextDirection {
        self.direction
    }
    /// Its level: 0 left to right, 1 right to left.
    pub fn base_level(&self) -> u8 {
        direction_level(self.direction)
    }
    /// How long its text is, bytes.
    pub fn len(&self) -> usize {
        self.levels.len()
    }
    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }
    /// The level of every byte (a character's bytes share it).
    pub fn levels(&self) -> &[u8] {
        &self.levels
    }
    /// The level of the character at `offset`; the paragraph's at its end.
    pub fn level_at(&self, offset: usize) -> u8 {
        self.levels.get(offset).copied().unwrap_or_else(|| self.base_level())
    }
    /// Whether nothing in it goes right to left (no reordering needed).
    pub fn is_pure_ltr(&self) -> bool {
        self.pure_ltr
    }
    /// Whether all of it goes one way, the paragraph's.
    pub fn is_single_direction(&self) -> bool {
        let base = self.base_level();
        self.levels.iter().all(|&l| l == base)
    }

    /// Its runs of one level within `range`, in logical order: what a paragraph is shaped in (a run never mixes
    /// directions). An empty range is one empty run at its level.
    pub fn level_runs(&self, range: Range<usize>) -> Vec<BidiRun> {
        level_runs_of(&self.levels[range.clone()], range.start, self.level_at(range.start))
    }

    /// The line `text[range]` of this paragraph (`text` the paragraph's text it was analysed from), in visual
    /// order: its trailing whitespace and isolate controls, and whitespace before a tab, back at the paragraph's
    /// level (rule L1), then its runs reversed level by level (L2).
    pub fn line(&self, text: &str, range: Range<usize>) -> BidiLine {
        assert_eq!(
            text.len(),
            self.levels.len(),
            "kompoz: a bidi line comes from the text its paragraph was analysed from"
        );
        let base = self.base_level();
        let mut levels = self.levels[range.clone()].to_vec();
        reset_line_whitespace(&text[range.clone()], &self.classes[range.clone()], &mut levels, base);
        let logical = level_runs_of(&levels, range.start, base);
        let order = reorder_visual(&logical.iter().map(|r| r.level).collect::<Vec<_>>());
        let runs = order.into_iter().map(|i| logical[i].clone()).collect();
        BidiLine { range, base_level: base, levels, runs }
    }
}

/// `levels` cut where the level changes, as runs of absolute offsets (`origin` is the offset of `levels[0]`); no
/// levels make one empty run at `empty_level`.
fn level_runs_of(levels: &[u8], origin: usize, empty_level: u8) -> Vec<BidiRun> {
    if levels.is_empty() {
        return vec![BidiRun { range: origin..origin, level: empty_level }];
    }
    let mut runs = Vec::new();
    let mut start = 0;
    for i in 1..=levels.len() {
        if i == levels.len() || levels[i] != levels[start] {
            runs.push(BidiRun { range: origin + start..origin + i, level: levels[start] });
            start = i;
        }
    }
    runs
}

/// Rule L1 on a line's levels (unicode-bidi's, with explicit formatting characters kept as X9 retains them):
/// segment and paragraph separators, the whitespace and isolate controls before them and at the line's end back
/// at the paragraph's level; embedding controls and boundary neutrals take the level before them.
fn reset_line_whitespace(line: &str, classes: &[BidiClass], levels: &mut [u8], base: u8) {
    let mut reset_from: Option<usize> = Some(0);
    let mut previous = base;
    for (i, c) in line.char_indices() {
        let len = c.len_utf8();
        match classes[i] {
            BidiClass::B | BidiClass::S => {
                let from = reset_from.unwrap_or(i);
                levels[from..i + len].fill(base);
                reset_from = None;
            }
            BidiClass::WS | BidiClass::FSI | BidiClass::LRI | BidiClass::RLI | BidiClass::PDI => {
                reset_from.get_or_insert(i);
            }
            BidiClass::RLE | BidiClass::LRE | BidiClass::RLO | BidiClass::LRO | BidiClass::PDF | BidiClass::BN => {
                reset_from.get_or_insert(i);
                levels[i..i + len].fill(previous);
            }
            _ => reset_from = None,
        }
        previous = levels[i];
    }
    if let Some(from) = reset_from {
        levels[from..].fill(base);
    }
}

/// The visual order of pieces at `levels` (rule L2: from the highest level down to the lowest odd one, every
/// stretch of pieces at that level or above reversed): the logical indices of the pieces, left to right. For
/// putting a line's pieces (its runs cut further by style) in order.
pub fn reorder_visual(levels: &[u8]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..levels.len()).collect();
    let Some(&max) = levels.iter().max() else { return order };
    let min_odd = levels.iter().copied().min().unwrap_or(0) | 1;
    let mut level = max;
    while level >= min_odd {
        let mut i = 0;
        while i < order.len() {
            if levels[order[i]] < level {
                i += 1;
                continue;
            }
            let start = i;
            while i < order.len() && levels[order[i]] >= level {
                i += 1;
            }
            order[start..i].reverse();
        }
        level -= 1;
    }
    order
}

/// A line of a [`BidiParagraph`] in visual order: its levels after rule L1 and its runs left to right.
#[derive(Clone, PartialEq, Debug)]
pub struct BidiLine {
    range: Range<usize>,
    base_level: u8,
    levels: Vec<u8>,
    runs: Vec<BidiRun>,
}

impl BidiLine {
    /// The line's text in its paragraph.
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }
    /// Its paragraph's direction.
    pub fn paragraph_direction(&self) -> ResolvedTextDirection {
        level_direction(self.base_level)
    }
    /// Its runs left to right on screen (an empty line: one empty run at the paragraph's level). A right-to-left
    /// run is shown from its end to its start.
    pub fn runs(&self) -> &[BidiRun] {
        &self.runs
    }
    /// Its runs in logical order.
    pub fn logical_runs(&self) -> Vec<BidiRun> {
        let mut runs = self.runs.clone();
        runs.sort_by_key(|r| r.range.start);
        runs
    }
    /// The level of the character at `offset` on this line, L1 applied; the paragraph's at the line's end.
    pub fn level_at(&self, offset: usize) -> u8 {
        if self.range.contains(&offset) { self.levels[offset - self.range.start] } else { self.base_level }
    }
    /// The index in [`runs`](BidiLine::runs) of the run holding the character at `offset`.
    pub fn run_index_at(&self, offset: usize) -> Option<usize> {
        self.runs.iter().position(|r| r.range.contains(&offset))
    }
    /// Whether all of the line goes one way (a single run).
    pub fn is_single_run(&self) -> bool {
        self.runs.len() == 1
    }

    /// Which way the run holding the character at `offset` goes (Compose's getBidiRunDirection, Android's
    /// Layout.isRtlCharAt): left to right past the line's end unless all of the line goes right to left.
    pub fn run_direction(&self, offset: usize) -> ResolvedTextDirection {
        match self.run_index_at(offset) {
            Some(i) => self.runs[i].direction(),
            None if self.runs.iter().all(BidiRun::is_rtl) => ResolvedTextDirection::Rtl,
            None => ResolvedTextDirection::Ltr,
        }
    }

    /// Whether `offset` lies between characters of different levels, so a caret there is split (Android's
    /// Layout.isLevelBoundary): at the line's start or end, whether the character there is off the paragraph's
    /// level.
    pub fn is_level_boundary(&self, offset: usize) -> bool {
        if self.runs.len() == 1 && self.runs[0].level == self.base_level {
            return false;
        }
        if self.range.is_empty() {
            return false;
        }
        if offset == self.range.start {
            return self.level_at(offset) != self.base_level;
        }
        if offset == self.range.end {
            return self.levels[self.levels.len() - 1] != self.base_level;
        }
        self.level_at(offset - 1) != self.level_at(offset)
    }

    /// Whether the primary caret at `offset` sits at the trailing edge of the character before it rather than the
    /// leading edge of the one at it (Android's Layout.primaryIsTrailingPrevious): when the level before `offset`
    /// (the paragraph's at the line's start) is lower than the level at it (the paragraph's at its end).
    pub fn primary_is_trailing_previous(&self, offset: usize) -> bool {
        let at = self.level_at(offset);
        if offset > self.range.start && offset < self.range.end && self.level_at(offset - 1) == at {
            return false;
        }
        let before = if offset == self.range.start { self.base_level } else { self.level_at(offset - 1) };
        before < at
    }

    /// Where a caret at `offset` goes, px from the left edge of the line's runs laid side by side (its trailing
    /// whitespace included: on the right in a left-to-right paragraph, on the left in a right-to-left one), as
    /// Android's getPrimaryHorizontal (`primary`: where text of the paragraph's direction would be inserted) and
    /// getSecondaryHorizontal (where text of the other direction would). `width` measures a piece of one run as
    /// it is laid out (`run.range.start..offset`, or a whole run; tabs reaching their stops).
    pub fn horizontal(&self, offset: usize, primary: bool, width: impl FnMut(Range<usize>) -> f32) -> f32 {
        self.position(offset, self.primary_is_trailing_previous(offset) == primary, width)
    }

    /// Where a caret at `offset` goes when it takes the trailing edge of the character before `offset`
    /// (`trailing`) or the leading edge of the one at it (Android's TextLine.measure(offset, trailing)), px from
    /// the left edge of the line's runs laid side by side; `width` as [`horizontal`](BidiLine::horizontal)'s.
    pub fn position(&self, offset: usize, trailing: bool, mut width: impl FnMut(Range<usize>) -> f32) -> f32 {
        let rtl_paragraph = self.base_level % 2 == 1;
        // The caret takes the trailing edge of the character before `offset`, or the leading edge of the one at
        // it; before the line, the line's leading edge.
        let target = match (trailing, offset > self.range.start) {
            (true, true) => Some(offset - 1),
            (true, false) => None,
            (false, _) => Some(offset),
        };
        let mut x = 0.0;
        for run in &self.runs {
            let run_width = width(run.range.clone());
            if target.is_some_and(|t| run.range.contains(&t)) {
                let into = width(run.range.start..offset);
                return if run.is_rtl() { x + run_width - into } else { x + into };
            }
            x += run_width;
        }
        // Before the line its leading edge, past it its trailing edge.
        if target.is_none() == rtl_paragraph { x } else { 0.0 }
    }
}
