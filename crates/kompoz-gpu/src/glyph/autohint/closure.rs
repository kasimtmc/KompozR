//! HarfBuzz's closure of a glyph set under `GSUB` lookups (`hb_ot_layout_lookups_substitute_closure`, HarfBuzz
//! 14/15's `hb-ot-layout-gsubgpos.hh` and `OT/Layout/GSUB`): every glyph the lookups can turn the set's glyphs into,
//! as FreeType 2.15 takes a default style's coverage (see `shape.rs`).
//!
//! Ported as HarfBuzz computes it, its quirks kept: the glyphs active at each position of a contextual rule (a
//! nested lookup substitutes only those), positions a lookup that may change the glyph count covers taking every
//! glyph since, outputs added to the set once each top-level lookup is done, each lookup skipped when the glyphs
//! active for it were all seen already for the same set, the limits on nesting, lookup visits and rounds, a single
//! substitution whose delta maps a glyph range onto itself left out, coverage and class definition tables read as
//! HarfBuzz reads them (class 0 included). Tables are read from their bytes; a structure that doesn't fit reads as
//! empty (HarfBuzz's Null objects). Coverage and class formats 3 and 4 (HarfBuzz's "beyond 64K" extension) read as
//! empty too.

use super::gsub::{Bytes, NULL};
use std::collections::{BTreeSet, HashMap};

/// HarfBuzz's limits (`hb-limits.hh`).
const MAX_NESTING_LEVEL: u32 = 64;
const MAX_LOOKUP_VISIT_COUNT: u32 = 35000;
const CLOSURE_MAX_STAGES: u32 = 12;

/// HarfBuzz's `HB_SET_VALUE_INVALID`.
const INVALID: u32 = u32::MAX;

/// A set of glyphs (a bit per glyph id below 65536: `GSUB`'s 16-bit glyphs).
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct GlyphSet(Vec<u64>);

impl Default for GlyphSet {
    fn default() -> GlyphSet {
        GlyphSet(vec![0; 1024])
    }
}

impl GlyphSet {
    pub fn new() -> GlyphSet {
        GlyphSet::default()
    }

    pub fn insert(&mut self, g: u32) {
        if g < 0x1_0000 {
            self.0[g as usize >> 6] |= 1 << (g & 63);
        }
    }

    pub fn contains(&self, g: u32) -> bool {
        g < 0x1_0000 && self.0[g as usize >> 6] & (1 << (g & 63)) != 0
    }

    pub fn len(&self) -> u32 {
        self.0.iter().map(|w| w.count_ones()).sum()
    }

    fn is_empty(&self) -> bool {
        self.0.iter().all(|&w| w == 0)
    }

    fn clear(&mut self) {
        self.0.iter_mut().for_each(|w| *w = 0);
    }

    fn union(&mut self, other: &GlyphSet) {
        for (a, b) in self.0.iter_mut().zip(&other.0) {
            *a |= b;
        }
    }

    fn is_subset(&self, other: &GlyphSet) -> bool {
        self.0.iter().zip(&other.0).all(|(a, b)| a & !b == 0)
    }

    /// `hb_set_t::next`: the least element after `g` (any, from INVALID), or None.
    fn next(&self, g: u32) -> Option<u32> {
        let from = if g == INVALID { 0 } else { g.checked_add(1)? };
        let mut word = from as usize >> 6;
        let mut bits = *self.0.get(word)? & (!0u64 << (from & 63));
        loop {
            if bits != 0 {
                return Some((word as u32) << 6 | bits.trailing_zeros());
            }
            word += 1;
            bits = *self.0.get(word)?;
        }
    }

    fn min(&self) -> u32 {
        self.next(INVALID).unwrap_or(INVALID)
    }

    fn max(&self) -> u32 {
        (0..self.0.len())
            .rev()
            .find(|&w| self.0[w] != 0)
            .map_or(INVALID, |w| (w as u32) << 6 | (63 - self.0[w].leading_zeros()))
    }

    /// Whether any glyph from `first` to `last` is in the set.
    fn intersects_range(&self, first: u32, last: u32) -> bool {
        first <= last && self.next(first.wrapping_sub(1)).is_some_and(|g| g <= last)
    }

    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        std::iter::successors(self.next(INVALID), |&g| self.next(g))
    }
}

/// A coverage table (formats 1 and 2) at an offset in `GSUB`.
#[derive(Clone, Copy)]
struct Coverage<'a> {
    gsub: Bytes<'a>,
    at: usize,
}

impl Coverage<'_> {
    /// A 16-bit value `offset` bytes into the table (zero for a null table).
    fn read(&self, offset: usize) -> u32 {
        self.gsub.u16(self.at.wrapping_add(offset))
    }
    fn format(&self) -> u32 {
        self.read(0)
    }

    fn count(&self) -> usize {
        self.read(2) as usize
    }

    /// A format 2 table's range `i`: (first, last, start coverage index).
    fn range(&self, i: usize) -> (u32, u32, u32) {
        let r = 4 + i * 6;
        (self.read(r), self.read(r + 2), self.read(r + 4))
    }

    /// `get_population` (NOT_COVERED for another format).
    fn population(&self) -> u32 {
        match self.format() {
            1 => self.count() as u32,
            2 => (0..self.count())
                .map(|i| {
                    let (first, last, _) = self.range(i);
                    if last < first { 0 } else { last - first + 1 }
                })
                .sum(),
            _ => INVALID,
        }
    }

    /// The glyphs as HarfBuzz's coverage iterator gives them (a format 2 table stops where a range's coverage index
    /// doesn't follow on).
    fn glyphs(&self) -> Vec<u32> {
        match self.format() {
            1 => (0..self.count()).map(|i| self.read(4 + i * 2)).collect(),
            2 => {
                let n = self.count();
                let mut out = Vec::new();
                if n == 0 {
                    return out;
                }
                let (first, last, _) = self.range(0);
                if first > last {
                    return out;
                }
                let (mut i, mut j, mut coverage) = (0usize, first, 0u32);
                while i < n {
                    out.push(j);
                    if j >= self.range(i).1 {
                        i += 1;
                        if i < n {
                            let old = coverage;
                            let (first, _, value) = self.range(i);
                            j = first;
                            coverage = value;
                            if coverage != old.wrapping_add(1) {
                                break;
                            }
                        }
                        continue;
                    }
                    coverage += 1;
                    j += 1;
                }
                out
            }
            _ => Vec::new(),
        }
    }

    /// `intersects`: whether any covered glyph is in `glyphs`.
    fn intersects(&self, glyphs: &GlyphSet) -> bool {
        match self.format() {
            1 => (0..self.count()).any(|i| glyphs.contains(self.read(4 + i * 2))),
            2 => (0..self.count()).any(|i| {
                let (first, last, _) = self.range(i);
                first <= last && glyphs.intersects_range(first, last)
            }),
            _ => false,
        }
    }

    /// `intersect_set`: the covered glyphs in `glyphs` added to `out` (a format 2 table read up to a range that
    /// goes back).
    fn intersect_set(&self, glyphs: &GlyphSet, out: &mut GlyphSet) {
        match self.format() {
            1 => {
                for i in 0..self.count() {
                    let g = self.read(4 + i * 2);
                    if glyphs.contains(g) {
                        out.insert(g);
                    }
                }
            }
            2 => {
                let mut last = 0;
                for i in 0..self.count() {
                    let (first, range_last, _) = self.range(i);
                    if first < last {
                        break;
                    }
                    last = range_last;
                    for g in first..=range_last {
                        if glyphs.contains(g) {
                            out.insert(g);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// A class definition table (formats 1 and 2) at an offset in `GSUB`.
#[derive(Clone, Copy)]
struct ClassDef<'a> {
    gsub: Bytes<'a>,
    at: usize,
}

impl ClassDef<'_> {
    /// A 16-bit value `offset` bytes into the table (zero for a null table).
    fn read(&self, offset: usize) -> u32 {
        self.gsub.u16(self.at.wrapping_add(offset))
    }
    fn format(&self) -> u32 {
        self.read(0)
    }

    /// Format 1: (start glyph, count); a value is `value(i)`.
    fn f1(&self) -> (u32, u32) {
        (self.read(2), self.read(4))
    }

    fn f1_value(&self, i: u32) -> u32 {
        self.read(6 + i as usize * 2)
    }

    fn f2_count(&self) -> usize {
        self.read(2) as usize
    }

    /// Format 2's range `i` (first, last, class); past the end, HarfBuzz's Crap record (zeros).
    fn f2_range(&self, i: usize) -> (u32, u32, u32) {
        if i >= self.f2_count() {
            return (0, 0, 0);
        }
        let r = 4 + i * 6;
        (self.read(r), self.read(r + 2), self.read(r + 4))
    }

    /// `intersects_class`.
    fn intersects_class(&self, glyphs: &GlyphSet, klass: u32) -> bool {
        match self.format() {
            1 => {
                let (start, count) = self.f1();
                if klass == 0 {
                    // Any glyph not listed.
                    let Some(g) = glyphs.next(INVALID) else {
                        return false;
                    };
                    if g < start {
                        return true;
                    }
                    if glyphs.next((start + count).wrapping_sub(1)).is_some() {
                        return true;
                    }
                }
                (0..count).any(|i| self.f1_value(i) == klass && glyphs.contains(start + i))
            }
            2 => {
                let n = self.f2_count();
                if klass == 0 {
                    if n == 0 {
                        return !glyphs.is_empty();
                    }
                    // (HarfBuzz's loop, its iterator `it` advanced apart from the loop's own.)
                    let (mut g, mut last, mut it) = (INVALID, INVALID, 0usize);
                    for r in 0..n {
                        if self.f2_range(it).0 == last.wrapping_add(1) {
                            it += 1;
                            continue;
                        }
                        match glyphs.next(g) {
                            Some(next) => g = next,
                            None => {
                                g = INVALID;
                                break;
                            }
                        }
                        let (first, range_last, _) = self.f2_range(r);
                        if g < first {
                            return true;
                        }
                        g = range_last;
                        last = g;
                    }
                    if g != INVALID && glyphs.next(g).is_some() {
                        return true;
                    }
                }
                (0..n).any(|r| {
                    let (first, last, value) = self.f2_range(r);
                    value == klass && first <= last && glyphs.intersects_range(first, last)
                })
            }
            _ => false,
        }
    }

    /// `intersected_class_glyphs`: the glyphs of class `klass` in `glyphs` added to `out`.
    fn intersected_class_glyphs(&self, glyphs: &GlyphSet, klass: u32, out: &mut GlyphSet) {
        match self.format() {
            1 => {
                let (start, count) = self.f1();
                if klass == 0 {
                    let mut g = INVALID;
                    while let Some(next) = glyphs.next(g) {
                        if next >= start {
                            break;
                        }
                        out.insert(next);
                        g = next;
                    }
                    let mut g = (start + count).wrapping_sub(1);
                    while let Some(next) = glyphs.next(g) {
                        out.insert(next);
                        g = next;
                    }
                    return;
                }
                for i in 0..count {
                    if self.f1_value(i) == klass && glyphs.contains(start + i) {
                        out.insert(start + i);
                    }
                }
            }
            2 => {
                let n = self.f2_count();
                if klass == 0 {
                    let mut g = INVALID;
                    for r in 0..n {
                        let (first, last, _) = self.f2_range(r);
                        let Some(next) = glyphs.next(g) else {
                            return;
                        };
                        g = next;
                        while g < first {
                            out.insert(g);
                            let Some(next) = glyphs.next(g) else {
                                return;
                            };
                            g = next;
                        }
                        g = last;
                    }
                    while let Some(next) = glyphs.next(g) {
                        out.insert(next);
                        g = next;
                    }
                    return;
                }
                for r in 0..n {
                    let (first, last, value) = self.f2_range(r);
                    if value != klass {
                        continue;
                    }
                    let mut g = first.wrapping_sub(1);
                    while let Some(next) = glyphs.next(g) {
                        if next > last {
                            break;
                        }
                        out.insert(next);
                        g = next;
                    }
                }
            }
            _ => {}
        }
    }
}

/// What a contextual rule's values are: glyphs, classes or coverages (HarfBuzz's `ContextFormat`).
#[derive(Clone, Copy, PartialEq)]
enum ContextFormat {
    Simple,
    ClassBased,
    CoverageBased,
}

/// Where a rule's input values (and backtrack and lookahead ones) are read from, with the caches HarfBuzz keeps
/// while it closes over one subtable.
struct RuleContext<'a> {
    format: ContextFormat,
    /// Class definitions (backtrack, input, lookahead) for classes; the subtable for coverages.
    data: [usize; 3],
    intersects_cache: [HashMap<u32, bool>; 3],
    intersected_cache: HashMap<u32, GlyphSet>,
    gsub: Bytes<'a>,
}

impl RuleContext<'_> {
    /// `intersects_glyph` / `intersects_class` / `intersects_coverage` for a value of array `which`.
    fn intersects(&mut self, glyphs: &GlyphSet, which: usize, value: u32) -> bool {
        match self.format {
            ContextFormat::Simple => glyphs.contains(value),
            ContextFormat::ClassBased => {
                if let Some(&v) = self.intersects_cache[which].get(&value) {
                    return v;
                }
                let v = ClassDef { gsub: self.gsub, at: self.data[which] }.intersects_class(glyphs, value);
                self.intersects_cache[which].insert(value, v);
                v
            }
            ContextFormat::CoverageBased => {
                let at = if value == 0 { NULL } else { self.data[which] + value as usize };
                Coverage { gsub: self.gsub, at }.intersects(glyphs)
            }
        }
    }

    /// `intersected_glyph` / `intersected_class_glyphs` (cached by class, whatever the glyph set) /
    /// `intersected_coverage_glyphs` for an input value.
    fn intersected(&mut self, glyphs: &GlyphSet, value: u32, out: &mut GlyphSet) {
        match self.format {
            ContextFormat::Simple => out.insert(value),
            ContextFormat::ClassBased => {
                if let Some(cached) = self.intersected_cache.get(&value) {
                    out.union(cached);
                    return;
                }
                let mut v = GlyphSet::new();
                ClassDef { gsub: self.gsub, at: self.data[1] }.intersected_class_glyphs(glyphs, value, &mut v);
                out.union(&v);
                self.intersected_cache.insert(value, v);
            }
            ContextFormat::CoverageBased => {
                let at = if value == 0 { NULL } else { self.data[1] + value as usize };
                Coverage { gsub: self.gsub, at }.intersect_set(glyphs, out)
            }
        }
    }
}

/// A contextual rule: its backtrack, input (after the first glyph) and lookahead values, its lookup records
/// (sequence index, lookup index).
struct Rule {
    backtrack: Vec<u32>,
    /// The input's length, its first glyph included (as the rule gives it: 0 for none at all).
    input_count: u32,
    input: Vec<u32>,
    lookahead: Vec<u32>,
    records: Vec<(u32, u32)>,
}

/// HarfBuzz's `hb_closure_context_t`, closing a glyph set over the lookups of a `GSUB` table.
struct Closure<'a> {
    gsub: Bytes<'a>,
    lookup_list: usize,
    num_glyphs: u32,
    glyphs: GlyphSet,
    output: GlyphSet,
    active: Vec<GlyphSet>,
    nesting_level_left: u32,
    done_count: HashMap<u32, u32>,
    done_set: HashMap<u32, GlyphSet>,
    lookup_count: u32,
}

impl<'a> Closure<'a> {
    fn parent_active(&self) -> &GlyphSet {
        self.active.last().unwrap_or(&self.glyphs)
    }

    fn previous_parent_active(&self) -> &GlyphSet {
        if self.active.len() <= 1 { &self.glyphs } else { &self.active[self.active.len() - 2] }
    }

    fn lookup_limit_exceeded(&self) -> bool {
        self.lookup_count > MAX_LOOKUP_VISIT_COUNT
    }

    /// `should_visit_lookup`: not past the visit limit, and not done for the active glyphs.
    fn should_visit_lookup(&mut self, lookup: u32) -> bool {
        let count = self.lookup_count;
        self.lookup_count += 1;
        if count > MAX_LOOKUP_VISIT_COUNT {
            return false;
        }
        !self.is_lookup_done(lookup)
    }

    /// `is_lookup_done`: whether the glyphs active now were all seen by the lookup since the set last grew.
    fn is_lookup_done(&mut self, lookup: u32) -> bool {
        let pop = self.glyphs.len();
        if self.done_count.get(&lookup) != Some(&pop) {
            self.done_count.insert(lookup, pop);
            self.done_set.entry(lookup).or_default().clear();
        }
        let active = self.parent_active().clone();
        let covered = self.done_set.entry(lookup).or_default();
        if active.is_subset(covered) {
            return true;
        }
        covered.union(&active);
        false
    }

    /// `flush`: the outputs (real glyphs) added to the set, the active glyphs dropped.
    fn flush(&mut self) {
        for g in self.output.iter().filter(|&g| g >= self.num_glyphs).collect::<Vec<_>>() {
            self.output.0[g as usize >> 6] &= !(1 << (g & 63));
        }
        self.glyphs.union(&self.output);
        self.output.clear();
        self.active.clear();
    }

    /// Lookup `index`'s type and subtables (each resolved through extensions to its own type).
    fn subtables(&self, index: u32) -> Vec<(u32, usize)> {
        let gsub = self.gsub;
        let count = gsub.u16(self.lookup_list) as usize;
        if index as usize >= count {
            return Vec::new();
        }
        let lookup = gsub.offset16(self.lookup_list, self.lookup_list + 2 + index as usize * 2);
        let kind = gsub.u16(lookup);
        let n = gsub.u16(lookup.wrapping_add(4)) as usize;
        (0..n)
            .map(|i| {
                let subtable = gsub.offset16(lookup, lookup + 6 + i * 2);
                if kind == 7 {
                    (gsub.u16(subtable.wrapping_add(2)), gsub.offset32(subtable, subtable.wrapping_add(4)))
                } else {
                    (kind, subtable)
                }
            })
            .collect()
    }

    /// `may_have_non_1to1`: whether a subtable may change the glyph count (multiple, ligature, contextual).
    fn may_have_non_1to1(&self, index: u32) -> bool {
        self.subtables(index).iter().any(|&(kind, _)| matches!(kind, 2 | 4 | 5 | 6))
    }

    /// `SubstLookup::closure`: a top-level lookup, its outputs flushed after.
    fn lookup_closure(&mut self, index: u32) {
        if !self.should_visit_lookup(index) {
            return;
        }
        self.dispatch(index);
        self.flush();
    }

    /// `recurse` (`dispatch_closure_recurse_func`): a nested lookup, on the active glyphs, not flushed.
    fn recurse(&mut self, index: u32, covered: &mut BTreeSet<u32>, seq_index: u32, end_index: u32) {
        if self.nesting_level_left == 0 {
            return;
        }
        self.nesting_level_left -= 1;
        if self.should_visit_lookup(index) {
            if self.may_have_non_1to1(index) {
                covered.extend(seq_index..=end_index);
            }
            self.dispatch(index);
        }
        self.nesting_level_left += 1;
    }

    fn dispatch(&mut self, index: u32) {
        for (kind, subtable) in self.subtables(index) {
            self.subtable_closure(kind, subtable);
        }
    }

    fn coverage(&self, subtable: usize, offset_at: usize) -> Coverage<'a> {
        Coverage { gsub: self.gsub, at: self.gsub.offset16(subtable, offset_at) }
    }

    fn subtable_closure(&mut self, kind: u32, s: usize) {
        let gsub = self.gsub;
        if s == NULL {
            return;
        }
        let format = gsub.u16(s);
        match (kind, format) {
            (1, 1) => self.single1(s),
            (1, 2) => {
                let coverage = self.coverage(s, s + 2);
                let count = gsub.u16(s + 4) as usize;
                let outs: Vec<u32> = coverage
                    .glyphs()
                    .into_iter()
                    .take(count)
                    .enumerate()
                    .filter(|&(_, g)| self.parent_active().contains(g))
                    .map(|(k, _)| gsub.u16(s + 6 + k * 2))
                    .collect();
                outs.into_iter().for_each(|g| self.output.insert(g));
            }
            (2 | 3, 1) => {
                // Multiple and alternate substitutions: every glyph of the sequence or set.
                let coverage = self.coverage(s, s + 2);
                let count = gsub.u16(s + 4) as usize;
                let sets: Vec<usize> = coverage
                    .glyphs()
                    .into_iter()
                    .take(count)
                    .enumerate()
                    .filter(|&(_, g)| self.parent_active().contains(g))
                    .map(|(k, _)| gsub.offset16(s, s + 6 + k * 2))
                    .collect();
                for set in sets.into_iter().filter(|&set| set != NULL) {
                    let n = gsub.u16(set) as usize;
                    for i in 0..n {
                        self.output.insert(gsub.u16(set + 2 + i * 2));
                    }
                }
            }
            (4, 1) => {
                let coverage = self.coverage(s, s + 2);
                let count = gsub.u16(s + 4) as usize;
                let sets: Vec<usize> = coverage
                    .glyphs()
                    .into_iter()
                    .take(count)
                    .enumerate()
                    .filter(|&(_, g)| self.parent_active().contains(g))
                    .map(|(k, _)| gsub.offset16(s, s + 6 + k * 2))
                    .collect();
                for set in sets.into_iter().filter(|&set| set != NULL) {
                    let n = gsub.u16(set) as usize;
                    for i in 0..n {
                        let ligature = gsub.offset16(set, set + 2 + i * 2);
                        if ligature == NULL {
                            continue;
                        }
                        let components = gsub.u16(ligature + 2) as usize;
                        // (All components but the first in the set, the first having been active.)
                        let all = (1..components).all(|k| self.glyphs.contains(gsub.u16(ligature + 4 + (k - 1) * 2)));
                        if all {
                            self.output.insert(gsub.u16(ligature));
                        }
                    }
                }
            }
            (5, 1) => self.context1(s, false),
            (5, 2) => self.context2(s, false),
            (5, 3) => self.context3(s),
            (6, 1) => self.context1(s, true),
            (6, 2) => self.context2(s, true),
            (6, 3) => self.chain_context3(s),
            (8, 1) => self.reverse_chain(s),
            _ => {}
        }
    }

    /// Single substitution format 1: the active covered glyphs moved by the delta, unless they form a range the
    /// delta maps onto itself.
    fn single1(&mut self, s: usize) {
        let coverage = self.coverage(s, s + 2);
        let delta = self.gsub.u16(s + 4);
        let mask = 0xFFFF;
        if coverage.population() >= mask {
            return;
        }
        let mut intersection = GlyphSet::new();
        coverage.intersect_set(self.parent_active(), &mut intersection);
        let (min_before, max_before) = (intersection.min(), intersection.max());
        let min_after = min_before.wrapping_add(delta) & mask;
        let max_after = max_before.wrapping_add(delta) & mask;
        if intersection.len() == max_before.wrapping_sub(min_before).wrapping_add(1)
            && ((min_before <= min_after && min_after <= max_before)
                || (min_before <= max_after && max_after <= max_before))
        {
            return;
        }
        for g in intersection.iter().collect::<Vec<_>>() {
            self.output.insert(g.wrapping_add(delta) & mask);
        }
    }

    /// A rule of a format 1 or 2 rule set at `rule`: glyphs or classes (chained: backtrack, input, lookahead).
    fn read_rule(&self, rule: usize, chained: bool) -> Rule {
        let gsub = self.gsub;
        let array = |at: usize, n: usize| -> Vec<u32> { (0..n).map(|i| gsub.u16(at + i * 2)).collect() };
        if chained {
            let backtrack_count = gsub.u16(rule) as usize;
            let backtrack = array(rule + 2, backtrack_count);
            let at = rule + 2 + backtrack_count * 2;
            let input_count = gsub.u16(at) as usize;
            let input = array(at + 2, input_count.saturating_sub(1));
            let at = at + 2 + input_count.saturating_sub(1) * 2;
            let lookahead_count = gsub.u16(at) as usize;
            let lookahead = array(at + 2, lookahead_count);
            let at = at + 2 + lookahead_count * 2;
            let record_count = gsub.u16(at) as usize;
            let records = (0..record_count).map(|i| (gsub.u16(at + 2 + i * 4), gsub.u16(at + 4 + i * 4))).collect();
            Rule { backtrack, input_count: input_count as u32, input, lookahead, records }
        } else {
            let input_count = gsub.u16(rule) as usize;
            let record_count = gsub.u16(rule + 2) as usize;
            let input = array(rule + 4, input_count.saturating_sub(1));
            let at = rule + 4 + input_count.saturating_sub(1) * 2;
            let records = (0..record_count).map(|i| (gsub.u16(at + i * 4), gsub.u16(at + 2 + i * 4))).collect();
            Rule { backtrack: Vec::new(), input_count: input_count as u32, input, lookahead: Vec::new(), records }
        }
    }

    /// The rules of the rule set at `set` (none at a null offset, none for a null rule).
    fn rule_set(&self, set: usize, chained: bool) -> Vec<Rule> {
        if set == NULL {
            return Vec::new();
        }
        let n = self.gsub.u16(set) as usize;
        (0..n)
            .map(|i| self.gsub.offset16(set, set + 2 + i * 2))
            .filter(|&rule| rule != NULL)
            .map(|rule| self.read_rule(rule, chained))
            .collect()
    }

    /// `(chain_)context_closure_lookup`: a rule whose values all intersect the set, its lookups applied.
    fn rule_closure(&mut self, rule: &Rule, value: u32, context: &mut RuleContext<'_>) {
        if self.lookup_limit_exceeded() {
            return;
        }
        let glyphs = self.glyphs.clone();
        let backtrack_ok = rule.backtrack.iter().all(|&v| context.intersects(&glyphs, 0, v));
        let input_ok = rule.input.iter().all(|&v| context.intersects(&glyphs, 1, v));
        let lookahead_ok = rule.lookahead.iter().all(|&v| context.intersects(&glyphs, 2, v));
        if backtrack_ok && input_ok && lookahead_ok {
            self.recurse_lookups(rule, value, context);
        }
    }

    /// `context_closure_recurse_lookups`: each lookup record applied to the glyphs active at its position (all
    /// glyphs once a lookup that may change the glyph count has covered it).
    fn recurse_lookups(&mut self, rule: &Rule, value: u32, context: &mut RuleContext<'_>) {
        let input_count = rule.input_count;
        let mut covered = BTreeSet::new();
        for &(seq_index, lookup) in &rule.records {
            if seq_index >= input_count {
                continue;
            }
            let mut pos_glyphs = None;
            if !covered.contains(&seq_index) {
                let mut glyphs = GlyphSet::new();
                if seq_index == 0 {
                    match context.format {
                        ContextFormat::Simple => glyphs.insert(value),
                        ContextFormat::ClassBased => {
                            let active = self.parent_active().clone();
                            context.intersected(&active, value, &mut glyphs);
                        }
                        ContextFormat::CoverageBased => glyphs = self.parent_active().clone(),
                    }
                } else {
                    let all = self.glyphs.clone();
                    context.intersected(&all, rule.input[seq_index as usize - 1], &mut glyphs);
                }
                pos_glyphs = Some(glyphs);
            }
            covered.insert(seq_index);
            let active = pos_glyphs.unwrap_or_else(|| self.glyphs.clone());
            self.active.push(active);
            let mut end_index = input_count;
            if context.format == ContextFormat::CoverageBased {
                end_index += 1;
            }
            self.recurse(lookup, &mut covered, seq_index, end_index);
            self.active.pop();
        }
    }

    /// The active glyphs of a contextual subtable pushed: its coverage's of the parent's active glyphs.
    fn push_covered(&mut self, coverage: Coverage<'_>) {
        let mut active = GlyphSet::new();
        coverage.intersect_set(self.parent_active(), &mut active);
        self.active.push(active);
    }

    /// Context and chained context format 1: each rule set of an active covered glyph, by glyph.
    fn context1(&mut self, s: usize, chained: bool) {
        let gsub = self.gsub;
        let coverage = self.coverage(s, s + 2);
        self.push_covered(coverage);
        let count = gsub.u16(s + 4) as usize;
        let mut context = RuleContext {
            format: ContextFormat::Simple,
            data: [0; 3],
            intersects_cache: Default::default(),
            intersected_cache: HashMap::new(),
            gsub,
        };
        let firsts: Vec<(u32, usize)> = coverage
            .glyphs()
            .into_iter()
            .take(count)
            .enumerate()
            .filter(|&(_, g)| self.previous_parent_active().contains(g))
            .map(|(k, g)| (g, k))
            .collect();
        for (first, k) in firsts {
            let rules = self.rule_set(gsub.offset16(s, s + 6 + k * 2), chained);
            if self.lookup_limit_exceeded() {
                continue;
            }
            for rule in &rules {
                self.rule_closure(rule, first, &mut context);
            }
        }
        self.active.pop();
    }

    /// Context and chained context format 2: each rule set of a class with active covered glyphs, by class.
    fn context2(&mut self, s: usize, chained: bool) {
        let gsub = self.gsub;
        let coverage = self.coverage(s, s + 2);
        if !coverage.intersects(&self.glyphs) {
            return;
        }
        self.push_covered(coverage);
        let class_def = |at: usize| gsub.offset16(s, at);
        let (data, sets_at) = if chained {
            ([class_def(s + 4), class_def(s + 6), class_def(s + 8)], s + 10)
        } else {
            let input = class_def(s + 4);
            ([input, input, input], s + 6)
        };
        let input_class_def = ClassDef { gsub, at: data[1] };
        let mut context = RuleContext {
            format: ContextFormat::ClassBased,
            data,
            intersects_cache: Default::default(),
            intersected_cache: HashMap::new(),
            gsub,
        };
        let count = gsub.u16(sets_at) as usize;
        for class in 0..count {
            if !input_class_def.intersects_class(self.parent_active(), class as u32) {
                continue;
            }
            let rules = self.rule_set(gsub.offset16(s, sets_at + 2 + class * 2), chained);
            if self.lookup_limit_exceeded() {
                continue;
            }
            for rule in &rules {
                self.rule_closure(rule, class as u32, &mut context);
            }
        }
        self.active.pop();
    }

    /// Context format 3: one rule of coverages.
    fn context3(&mut self, s: usize) {
        let gsub = self.gsub;
        let glyph_count = gsub.u16(s + 2) as usize;
        let record_count = gsub.u16(s + 4) as usize;
        let first = Coverage { gsub, at: gsub.offset16(s, s + 6) };
        if !first.intersects(&self.glyphs) {
            return;
        }
        self.push_covered(first);
        let input = (1..glyph_count).map(|i| gsub.u16(s + 6 + i * 2)).collect();
        let at = s + 6 + glyph_count * 2;
        let records = (0..record_count).map(|i| (gsub.u16(at + i * 4), gsub.u16(at + 2 + i * 4))).collect();
        let rule =
            Rule { backtrack: Vec::new(), input_count: glyph_count as u32, input, lookahead: Vec::new(), records };
        let mut context = RuleContext {
            format: ContextFormat::CoverageBased,
            data: [s; 3],
            intersects_cache: Default::default(),
            intersected_cache: HashMap::new(),
            gsub,
        };
        self.rule_closure(&rule, 0, &mut context);
        self.active.pop();
    }

    /// Chained context format 3: one rule of coverages.
    fn chain_context3(&mut self, s: usize) {
        let gsub = self.gsub;
        let backtrack_count = gsub.u16(s + 2) as usize;
        let backtrack = (0..backtrack_count).map(|i| gsub.u16(s + 4 + i * 2)).collect();
        let at = s + 4 + backtrack_count * 2;
        let input_count = gsub.u16(at) as usize;
        let inputs: Vec<u32> = (0..input_count).map(|i| gsub.u16(at + 2 + i * 2)).collect();
        let at = at + 2 + input_count * 2;
        let lookahead_count = gsub.u16(at) as usize;
        let lookahead = (0..lookahead_count).map(|i| gsub.u16(at + 2 + i * 2)).collect();
        let at = at + 2 + lookahead_count * 2;
        let record_count = gsub.u16(at) as usize;
        let records = (0..record_count).map(|i| (gsub.u16(at + 2 + i * 4), gsub.u16(at + 4 + i * 4))).collect();
        let first = Coverage {
            gsub,
            at: match inputs.first() {
                Some(&offset) if offset != 0 => s + offset as usize,
                _ => NULL,
            },
        };
        if !first.intersects(&self.glyphs) {
            return;
        }
        self.push_covered(first);
        let input = inputs.iter().skip(1).copied().collect();
        let rule = Rule { backtrack, input_count: input_count as u32, input, lookahead, records };
        let mut context = RuleContext {
            format: ContextFormat::CoverageBased,
            data: [s; 3],
            intersects_cache: Default::default(),
            intersected_cache: HashMap::new(),
            gsub,
        };
        self.rule_closure(&rule, 0, &mut context);
        self.active.pop();
    }

    /// Reverse chained single substitution: when its coverages all intersect the set, the active covered glyphs'
    /// substitutes.
    fn reverse_chain(&mut self, s: usize) {
        let gsub = self.gsub;
        let coverage = self.coverage(s, s + 2);
        let backtrack_count = gsub.u16(s + 4) as usize;
        let backtrack: Vec<u32> = (0..backtrack_count).map(|i| gsub.u16(s + 6 + i * 2)).collect();
        let at = s + 6 + backtrack_count * 2;
        let lookahead_count = gsub.u16(at) as usize;
        let lookahead: Vec<u32> = (0..lookahead_count).map(|i| gsub.u16(at + 2 + i * 2)).collect();
        let at = at + 2 + lookahead_count * 2;
        let substitute_count = gsub.u16(at) as usize;
        let intersects = |offset: u32| {
            let at = if offset == 0 { NULL } else { s + offset as usize };
            Coverage { gsub, at }.intersects(&self.glyphs)
        };
        if !coverage.intersects(&self.glyphs)
            || !backtrack.iter().all(|&o| intersects(o))
            || !lookahead.iter().all(|&o| intersects(o))
        {
            return;
        }
        let outs: Vec<u32> = coverage
            .glyphs()
            .into_iter()
            .take(substitute_count)
            .enumerate()
            .filter(|&(_, g)| self.parent_active().contains(g))
            .map(|(k, _)| gsub.u16(at + 2 + k * 2))
            .collect();
        outs.into_iter().for_each(|g| self.output.insert(g));
    }
}

/// `hb_ot_layout_lookups_substitute_closure`: `glyphs` closed over `lookups` of `gsub` (a font of `num_glyphs`
/// glyphs), round after round while it grows (up to HarfBuzz's limit).
pub(crate) fn substitute_closure(gsub: Bytes<'_>, lookups: &BTreeSet<u32>, glyphs: &mut GlyphSet, num_glyphs: u32) {
    let lookup_list = gsub.offset16(0, 8);
    let mut closure = Closure {
        gsub,
        lookup_list,
        num_glyphs,
        glyphs: std::mem::take(glyphs),
        output: GlyphSet::new(),
        active: Vec::new(),
        nesting_level_left: MAX_NESTING_LEVEL,
        done_count: HashMap::new(),
        done_set: HashMap::new(),
        lookup_count: 0,
    };
    let mut iteration = 0;
    loop {
        closure.lookup_count = 0;
        let length = closure.glyphs.len();
        for &lookup in lookups {
            closure.lookup_closure(lookup);
        }
        let more = iteration <= CLOSURE_MAX_STAGES && length != closure.glyphs.len();
        iteration += 1;
        if !more {
            break;
        }
    }
    closure.flush();
    *glyphs = closure.glyphs;
}
