//! Styles, scripts and glyph style mapping.

use super::metrics::BlueZones;
use super::shape::{ShaperCoverageKind, VisitedLookupSet};
use skrifa::raw::types::{GlyphId, Tag};
use std::vec::Vec;

/// Defines the script and style associated with a single glyph.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(transparent)]
pub struct GlyphStyle(pub(super) u16);

impl GlyphStyle {
    // The following flags roughly correspond to those defined in FreeType
    // here: https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afglobal.h#L76
    // but with different values because we intend to store "meta style"
    // information differently.
    const STYLE_INDEX_MASK: u16 = 0xFF;
    const UNASSIGNED: u16 = Self::STYLE_INDEX_MASK;
    // A non-base character, perhaps more commonly referred to as a "mark"
    const NON_BASE: u16 = 0x100;
    const DIGIT: u16 = 0x200;
    // Assigned through the character map (FreeType 2.14's AF_HAS_CMAP_ENTRY)
    const HAS_CMAP_ENTRY: u16 = 0x400;
    // Used as intermediate state to mark when a glyph appears as GSUB output
    // for a given script
    const FROM_GSUB_OUTPUT: u16 = 0x8000;

    /// Constructs a new glyph style from the given style index and properties.
    pub const fn from_raw_parts(style_index: u16, is_non_base: bool, is_digit: bool) -> Self {
        let base = is_non_base as u16 * Self::NON_BASE;
        let digit = is_digit as u16 * Self::DIGIT;
        Self((style_index & Self::STYLE_INDEX_MASK) | base | digit)
    }

    /// Returns true if this glyph doesn't have an assigned style.
    pub const fn is_unassigned(self) -> bool {
        self.0 & Self::STYLE_INDEX_MASK == Self::UNASSIGNED
    }

    /// Returns true if this glyph is a non-base (usually a mark).
    pub const fn is_non_base(self) -> bool {
        self.0 & Self::NON_BASE != 0
    }

    /// Returns true if this glyph is a digit.
    pub const fn is_digit(self) -> bool {
        self.0 & Self::DIGIT != 0
    }

    /// The style as FreeType's glyph style map holds it (its `glyph-to-script-map` property): the style index
    /// (0x1FFF unassigned), 0x2000 if assigned through the character map, 0x4000 non-base, 0x8000 a digit.
    pub const fn to_freetype(self) -> u16 {
        let index = self.0 & Self::STYLE_INDEX_MASK;
        let mut out = if index == Self::UNASSIGNED { 0x1FFF } else { index };
        if self.0 & Self::HAS_CMAP_ENTRY != 0 {
            out |= 0x2000;
        }
        if self.0 & Self::NON_BASE != 0 {
            out |= 0x4000;
        }
        if self.0 & Self::DIGIT != 0 {
            out |= 0x8000;
        }
        out
    }

    /// Whether the glyph has no style and no flags (FreeType's `gstyles[idx] == AF_STYLE_UNASSIGNED`).
    pub(super) const fn is_unassigned_without_flags(self) -> bool {
        self.0 & !Self::FROM_GSUB_OUTPUT == Self::UNASSIGNED
    }

    /// The style set to `style_index`.
    pub(super) fn assign(&mut self, style_index: u16) {
        self.0 = (self.0 & !Self::STYLE_INDEX_MASK) | style_index;
    }

    /// Returns the associated style class for this glyph.
    pub fn style_class(self) -> Option<&'static StyleClass> {
        StyleClass::from_index(self.style_index()?)
    }

    /// Returns the index of the style class for this class.
    pub const fn style_index(self) -> Option<u16> {
        let ix = self.0 & Self::STYLE_INDEX_MASK;
        if ix != Self::UNASSIGNED { Some(ix) } else { None }
    }

    fn maybe_assign(&mut self, other: Self) {
        // FreeType walks the style array in order so earlier styles
        // have precedence. Since we walk the cmap and binary search
        // on the full range mapping, our styles are mapped in a
        // different order. This check allows us to replace a currently
        // mapped style if the new style index is lower which matches
        // FreeType's behavior, marking the glyph as mapped by the cmap.
        //
        // FreeType sets the NON_BASE bit of a glyph whose style is the
        // script's while it walks the script's non-base ranges: so a lower
        // style replaces the bit, and another character of the same style
        // adds to it.
        let (current, new) = (self.0 & Self::STYLE_INDEX_MASK, other.0 & Self::STYLE_INDEX_MASK);
        if new < current {
            self.0 = (self.0 & !(Self::STYLE_INDEX_MASK | Self::NON_BASE)) | other.0 | Self::HAS_CMAP_ENTRY;
        } else if new == current {
            self.0 |= other.0 & Self::NON_BASE;
        }
    }

    pub(super) fn set_from_gsub_output(&mut self) {
        self.0 |= Self::FROM_GSUB_OUTPUT
    }

    pub(super) fn clear_from_gsub(&mut self) {
        self.0 &= !Self::FROM_GSUB_OUTPUT;
    }

    /// Assign a style if we've been marked as GSUB output _and_ the
    /// we don't currently have an assigned style.
    ///
    /// This also clears the GSUB output bit.
    ///
    /// Returns `true` if this style was applied.
    pub(super) fn maybe_assign_gsub_output_style(&mut self, style: &StyleClass) -> bool {
        let style_ix = style.index as u16;
        if self.0 & Self::FROM_GSUB_OUTPUT != 0 && self.is_unassigned() {
            self.clear_from_gsub();
            self.0 = (self.0 & !Self::STYLE_INDEX_MASK) | style_ix;
            true
        } else {
            false
        }
    }
}

impl Default for GlyphStyle {
    fn default() -> Self {
        Self(Self::UNASSIGNED)
    }
}

/// Sentinel for unused styles in [`GlyphStyleMap::metrics_map`].
const UNMAPPED_STYLE: u8 = 0xFF;

/// Maps glyph identifiers to glyph styles.
///
/// Also keeps track of the styles that are actually used so we can allocate
/// an appropriately sized metrics array.
#[derive(Debug)]
pub(crate) struct GlyphStyleMap {
    /// List of styles, indexed by glyph id.
    styles: Vec<GlyphStyle>,
    /// Maps an actual style class index into a compacted index for the
    /// metrics table.
    ///
    /// Uses `0xFF` to signify unused styles.
    metrics_map: [u8; MAX_STYLES],
    /// Number of metrics styles in use.
    metrics_count: u8,
}

impl GlyphStyleMap {
    /// Computes a new glyph style map for the given glyph count and character
    /// map.
    ///
    /// Roughly based on <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afglobal.c#L126>
    pub fn new(glyph_count: u32, shaper: &Shaper) -> Self {
        let lookup_count = shaper.lookup_count() as usize;
        if lookup_count > 0 {
            // If we're processing lookups, allocate some temporary memory to
            // store the visited set
            let lookup_set_byte_size = lookup_count.div_ceil(8);
            skrifa::raw::mem::with_scratch(lookup_set_byte_size, |bytes| {
                Self::new_inner(glyph_count, shaper, VisitedLookupSet::new(bytes))
            })
        } else {
            Self::new_inner(glyph_count, shaper, VisitedLookupSet::new(&mut []))
        }
    }

    fn new_inner(glyph_count: u32, shaper: &Shaper, mut visited_set: VisitedLookupSet) -> Self {
        let mut map = Self {
            styles: vec![GlyphStyle::default(); glyph_count as usize],
            metrics_map: [UNMAPPED_STYLE; MAX_STYLES],
            metrics_count: 0,
        };
        // Step 1: compute styles for glyphs covered by OpenType features
        // See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afglobal.c#L233>
        for style in super::style::STYLE_CLASSES {
            if style.feature.is_some()
                && shaper.compute_coverage(style, ShaperCoverageKind::Script, &mut map.styles, &mut visited_set)
            {
                map.use_style(style.index);
            }
        }
        // Step 2: compute styles for glyphs contained in the cmap
        // cmap entries are sorted so we keep track of the most recent range to
        // avoid a binary search per character
        let mut last_range: Option<(usize, StyleRange)> = None;
        shaper.context().for_each_mapping(&mut |ch, gid| {
            let Some(style) = map.styles.get_mut(gid.to_u32() as usize) else {
                return;
            };
            // Charmaps enumerate in order so we're likely to encounter at least
            // a few codepoints in the same range.
            if let Some(last) = last_range
                && last.1.contains(ch)
            {
                style.maybe_assign(last.1.style);
                return;
            }
            let ix = match STYLE_RANGES.binary_search_by(|x| x.first.cmp(&ch)) {
                Ok(i) => i,
                Err(i) => i.saturating_sub(1),
            };
            let Some(range) = STYLE_RANGES.get(ix).copied() else {
                return;
            };
            if range.contains(ch) {
                style.maybe_assign(range.style);
                if let Some(style_ix) = range.style.style_index() {
                    map.use_style(style_ix as usize);
                }
                last_range = Some((ix, range));
            }
        });
        // Step 3a: compute script based coverage
        // See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afglobal.c#L239>
        for style in super::style::STYLE_CLASSES {
            if style.feature.is_none()
                && shaper.compute_coverage(style, ShaperCoverageKind::Script, &mut map.styles, &mut visited_set)
            {
                map.use_style(style.index);
            }
        }
        // Step 3b: compute coverage for "default" script which is always set
        // to Latin in FreeType
        // See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afglobal.c#L248>
        let default_style = &STYLE_CLASSES[StyleClass::LATN];
        if shaper.compute_coverage(default_style, ShaperCoverageKind::Default, &mut map.styles, &mut visited_set) {
            map.use_style(default_style.index);
        }
        // Step 4: Assign a default to all remaining glyphs
        // For some reason, FreeType uses Hani as a default fallback style so
        // let's do the same.
        // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afglobal.h#L69>
        let mut need_hani = false;
        for style in map.styles.iter_mut() {
            if style.is_unassigned() {
                style.0 &= !GlyphStyle::STYLE_INDEX_MASK;
                style.0 |= StyleClass::HANI as u16;
                need_hani = true;
            }
        }
        if need_hani {
            map.use_style(StyleClass::HANI);
        }
        // Step 5: Mark ASCII digits
        // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afglobal.c#L251>
        for digit_char in '0'..='9' {
            if let Some(style) =
                shaper.context().map(digit_char as u32).and_then(|gid| map.styles.get_mut(gid.to_u32() as usize))
            {
                style.0 |= GlyphStyle::DIGIT;
            }
        }
        map
    }

    pub fn style(&self, glyph_id: GlyphId) -> Option<GlyphStyle> {
        self.styles.get(glyph_id.to_u32() as usize).copied()
    }

    /// Returns a compacted metrics index for the given glyph style.
    pub fn metrics_index(&self, style: GlyphStyle) -> Option<usize> {
        let ix = style.style_index()? as usize;
        let metrics_ix = *self.metrics_map.get(ix)? as usize;
        if metrics_ix != UNMAPPED_STYLE as usize { Some(metrics_ix) } else { None }
    }

    /// Returns the required size of the compacted metrics array.
    pub fn metrics_count(&self) -> usize {
        self.metrics_count as usize
    }

    fn use_style(&mut self, style_ix: usize) {
        let mapped = &mut self.metrics_map[style_ix];
        if *mapped == UNMAPPED_STYLE {
            // This the first time we've seen this style so record
            // it in the metrics map
            *mapped = self.metrics_count;
            self.metrics_count += 1;
        }
    }
}

impl Default for GlyphStyleMap {
    fn default() -> Self {
        Self { styles: Default::default(), metrics_map: [UNMAPPED_STYLE; MAX_STYLES], metrics_count: 0 }
    }
}

/// Determines which algorithms the autohinter will use while generating
/// metrics and processing a glyph outline.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum ScriptGroup {
    /// All scripts that are not CJK or Indic.
    ///
    /// FreeType calls this Latin.
    #[default]
    Default,
    Cjk,
    Indic,
}

/// Defines the basic properties for each script supported by the
/// autohinter.
#[derive(Clone, Debug)]
pub struct ScriptClass {
    /// The name of the script class.
    pub name: &'static str,
    /// Group that defines how glyphs belonging to this script are hinted.
    pub group: ScriptGroup,
    /// Unicode tag for the script (a made-up one for a fallback script without its own Unicode script).
    pub tag: Tag,
    /// Whether the script has a HarfBuzz script, so OpenType script tags in layout tables (a fallback script, its
    /// HarfBuzz script `HB_SCRIPT_INVALID`, has none).
    pub layout: bool,
    /// True if outline edges are processed top to bottom.
    pub hint_top_to_bottom: bool,
    /// Characters used to define standard width and height of stems.
    pub std_chars: &'static str,
    /// "Blue" characters used to define alignment zones.
    pub blues: &'static [(&'static str, BlueZones)],
}

/// Defines the basic properties for each style supported by the
/// autohinter.
///
/// There's mostly a 1:1 correspondence between styles and scripts except
/// in the cases where style coverage is determined by OpenType feature
/// coverage.
#[derive(Clone, Debug)]
pub struct StyleClass {
    /// The name of the style class.
    pub name: &'static str,
    /// Index of self in the STYLE_CLASSES array.
    pub index: usize,
    /// Associated Unicode script.
    pub script: &'static ScriptClass,
    /// OpenType feature tag for styles that derive coverage from layout
    /// tables.
    #[allow(unused)]
    pub feature: Option<Tag>,
}

impl StyleClass {
    pub(crate) fn from_index(index: u16) -> Option<&'static StyleClass> {
        STYLE_CLASSES.get(index as usize)
    }
}

/// Associates a basic glyph style with a range of codepoints.
#[derive(Copy, Clone, Debug)]
pub(super) struct StyleRange {
    pub first: u32,
    pub last: u32,
    pub style: GlyphStyle,
}

impl StyleRange {
    pub fn contains(&self, ch: u32) -> bool {
        (self.first..=self.last).contains(&ch)
    }
}

// The following are helpers for generated code.
const fn base_range(first: u32, last: u32, style_index: u16) -> StyleRange {
    StyleRange { first, last, style: GlyphStyle(style_index) }
}

const fn non_base_range(first: u32, last: u32, style_index: u16) -> StyleRange {
    StyleRange { first, last, style: GlyphStyle(style_index | GlyphStyle::NON_BASE) }
}

const MAX_STYLES: usize = STYLE_CLASSES.len();

use super::shape::Shaper;

include!("styles.rs");
