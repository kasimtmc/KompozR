//! The `GSUB` table as FreeType 2.14's diacritic adjustments read it: the single and alternate substitution lookups
//! (`afgsub.c`, validated and mapped as it does), and the lookups of a script's features as HarfBuzz collects them
//! for it (`hb_ot_layout_collect_lookups` with one script, every language and every feature, feature variations'
//! substitutes included).
//!
//! Both read the table's bytes as their C does: a structure that doesn't fit the table reads as empty (HarfBuzz's
//! Null objects), and the substitutions go into [`FtHash`]es so that they come back out in FreeType's order.

use super::adjust::FtHash;
use std::collections::BTreeSet;

/// HarfBuzz's limits on what collecting features visits (`hb-limits.hh`).
const MAX_SCRIPTS: u32 = 500;
const MAX_LANGSYS: u32 = 2000;
const MAX_FEATURE_INDICES: u32 = 8000;

/// Big-endian reads from a table's bytes; out of range, 0.
#[derive(Clone, Copy)]
pub(crate) struct Bytes<'a>(pub &'a [u8]);

impl Bytes<'_> {
    pub fn u16(&self, at: usize) -> u32 {
        match self.0.get(at..at.wrapping_add(2)) {
            Some(b) => u16::from_be_bytes([b[0], b[1]]) as u32,
            None => 0,
        }
    }

    pub fn u32(&self, at: usize) -> u32 {
        match self.0.get(at..at.wrapping_add(4)) {
            Some(b) => u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
            None => 0,
        }
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    /// The table an offset at `at` points to from `base` (HarfBuzz's `Offset16To`): `NULL` for a null offset.
    pub fn offset16(&self, base: usize, at: usize) -> usize {
        match self.u16(at) {
            0 => NULL,
            offset => base.wrapping_add(offset as usize),
        }
    }

    /// The same for a 32-bit offset.
    pub fn offset32(&self, base: usize, at: usize) -> usize {
        match self.u32(at) {
            0 => NULL,
            offset => base.wrapping_add(offset as usize),
        }
    }
}

/// Where a null offset points: nowhere, so that the table reads as zeros (HarfBuzz's Null object).
pub(crate) const NULL: usize = usize::MAX;

// ---- afgsub.c: validation ------------------------------------------------------------------------------------------

/// `af_validate_coverage`: whether the coverage table at `table` fits before `limit`, and how many glyphs it covers.
fn validate_coverage(gsub: Bytes<'_>, table: usize, limit: usize) -> Option<u32> {
    let mut p = table;
    if limit < p + 4 {
        return None;
    }
    let format = gsub.u16(p);
    p += 2;
    let mut count = 0u32;
    if format == 1 {
        let glyph_count = gsub.u16(p) as usize;
        p += 2;
        // (Glyph ids aren't validated.)
        if limit < p + glyph_count * 2 {
            return None;
        }
        count += glyph_count as u32;
    } else if format == 2 {
        let range_count = gsub.u16(p) as usize;
        p += 2;
        let end = p + range_count * 6;
        if limit < end {
            return None;
        }
        while p < end {
            let start = gsub.u16(p);
            let last = gsub.u16(p + 2);
            if start > last {
                return None;
            }
            count += last - start + 1;
            // (Coverage indices aren't validated.)
            p += 6;
        }
    } else {
        return None;
    }
    Some(count)
}

/// `af_validate_single_subst1`.
fn validate_single_subst1(gsub: Bytes<'_>, table: usize, limit: usize) -> bool {
    // (The format, the coverage offset and the glyph id delta are checked already.)
    validate_coverage(gsub, table + gsub.u16(table + 2) as usize, limit).is_some()
}

/// `af_validate_single_subst2`.
fn validate_single_subst2(gsub: Bytes<'_>, table: usize, limit: usize) -> bool {
    let mut p = table + 2;
    let coverage = table + gsub.u16(p) as usize;
    p += 2;
    let Some(num_glyphs) = validate_coverage(gsub, coverage, limit) else {
        return false;
    };
    let glyph_count = gsub.u16(p) as usize;
    p += 2;
    limit >= p + glyph_count * 2 && glyph_count as u32 == num_glyphs
}

/// `af_validate_alternate`.
fn validate_alternate(gsub: Bytes<'_>, table: usize, limit: usize) -> bool {
    let mut p = table + 2;
    let coverage = table + gsub.u16(p) as usize;
    p += 2;
    let Some(num_glyphs) = validate_coverage(gsub, coverage, limit) else {
        return false;
    };
    let set_count = gsub.u16(p) as usize;
    p += 2;
    let end = p + set_count * 2;
    if limit < end || set_count as u32 != num_glyphs {
        return false;
    }
    while p < end {
        let set = table + gsub.u16(p) as usize;
        p += 2;
        if limit < set + 2 {
            return false;
        }
        let glyph_count = gsub.u16(set) as usize;
        if limit < set + 2 + glyph_count * 2 {
            return false;
        }
    }
    true
}

/// `af_validate_lookup_table`: whether the lookup at `table` is a valid single or alternate substitution (through
/// extensions of one type).
fn validate_lookup(gsub: Bytes<'_>, table: usize, limit: usize) -> bool {
    let mut p = table;
    if limit < p + 6 {
        return false;
    }
    let lookup_type = gsub.u16(p);
    p += 4; // (The lookup flag skipped.)
    let subtable_count = gsub.u16(p) as usize;
    p += 2;
    let end = p + subtable_count * 2;
    if limit < end {
        return false;
    }
    let mut real_type = 0;
    while p < end {
        let mut subtable = table + gsub.u16(p) as usize;
        p += 2;
        if lookup_type == 7 {
            let q = subtable;
            if limit < q + 8 || gsub.u16(q) != 1 {
                return false;
            }
            let extension_type = gsub.u16(q + 2);
            if real_type == 0 {
                real_type = extension_type;
            } else if real_type != extension_type {
                return false;
            }
            subtable += gsub.u32(q + 4) as usize;
        } else {
            real_type = lookup_type;
        }
        if limit < subtable + 6 {
            return false;
        }
        let format = gsub.u16(subtable);
        let valid = match (real_type, format) {
            (1, 1) => validate_single_subst1(gsub, subtable, limit),
            (1, 2) => validate_single_subst2(gsub, subtable, limit),
            (3, 1) => validate_alternate(gsub, subtable, limit),
            _ => false,
        };
        if !valid {
            return false;
        }
    }
    true
}

/// `af_parse_gsub`: each lookup's offset from the start of the table if it is a valid single or alternate
/// substitution, else 0 (empty without a table, or one too short for its lookup list).
pub(crate) fn single_alternate_lookups(gsub: Bytes<'_>) -> Vec<u32> {
    let length = gsub.len();
    if length < 10 {
        return Vec::new();
    }
    let list = gsub.u16(8) as usize;
    if length < list + 2 {
        return Vec::new();
    }
    let count = gsub.u16(list) as usize;
    if length < list + 2 + count * 2 {
        return Vec::new();
    }
    (0..count)
        .map(|i| {
            let offset = gsub.u16(list + 2 + i * 2) as usize;
            if validate_lookup(gsub, list + offset, length) { (list + offset) as u32 } else { 0 }
        })
        .collect()
}

// ---- afgsub.c: mapping ---------------------------------------------------------------------------------------------

/// `af_coverage_iterator`: the glyphs of a (validated) coverage table as FreeType's iterator gives them, in its
/// order. It stops once it has read the table's last record, so that of a format 2 table's last range only the
/// first glyph comes out.
///
/// FreeType counts a range's glyphs in 16 bits, so that a range ending at glyph 0xFFFF (but the last) would wrap
/// round to 0 and never end; it ends at the wrap here.
pub(super) fn coverage_glyphs(gsub: Bytes<'_>, coverage: usize) -> Vec<u16> {
    let format = gsub.u16(coverage);
    let count = gsub.u16(coverage + 2) as usize;
    let mut p = coverage + 4;
    let limit = p + count * if format == 1 { 2 } else { 6 };
    let (mut glyph, mut glyph_limit) = (1u16, 0u16);
    let mut out = Vec::new();
    while p < limit {
        if format == 1 {
            out.push(gsub.u16(p) as u16);
            p += 2;
            continue;
        }
        if glyph > glyph_limit {
            glyph = gsub.u16(p) as u16;
            glyph_limit = gsub.u16(p + 2) as u16;
            p += 6;
        }
        out.push(glyph);
        if glyph == u16::MAX {
            break;
        }
        glyph += 1;
    }
    out
}

/// `af_hash_insert`: `substitute` added to `glyph`'s substitutes (the first one at `glyph`, the n-th at
/// `glyph + (n - 1) << 16`, the first one's value counting the others in its upper half).
fn hash_insert(glyph: u16, substitute: u16, map: &mut FtHash) {
    match map.lookup(glyph as u32) {
        None => map.insert(glyph as u32, substitute as u64),
        Some(value) => {
            let mask = ((value as u32) & 0xFFFF_0000).wrapping_add(0x1_0000);
            map.insert(glyph as u32 | mask, substitute as u64);
            map.set(glyph as u32, value + 0x1_0000);
        }
    }
}

/// `af_map_lookup`: the substitutions of the (validated) single or alternate lookup at `lookup` put into `map`.
pub(crate) fn map_lookup(gsub: Bytes<'_>, map: &mut FtHash, lookup: u32) {
    let table = lookup as usize;
    let lookup_type = gsub.u16(table);
    let subtable_count = gsub.u16(table + 4) as usize;
    for i in 0..subtable_count {
        let mut real_type = lookup_type;
        let mut subtable = table + gsub.u16(table + 6 + i * 2) as usize;
        if lookup_type == 7 {
            real_type = gsub.u16(subtable + 2);
            subtable += gsub.u32(subtable + 4) as usize;
        }
        let coverage = subtable + gsub.u16(subtable + 2) as usize;
        if real_type == 1 {
            if gsub.u16(subtable) == 1 {
                let delta = gsub.u16(subtable + 4);
                for glyph in coverage_glyphs(gsub, coverage) {
                    // (The delta is added modulo 65536.)
                    hash_insert(glyph, (glyph as u32).wrapping_add(delta) as u16, map);
                }
            } else {
                for (k, glyph) in coverage_glyphs(gsub, coverage).into_iter().enumerate() {
                    hash_insert(glyph, gsub.u16(subtable + 6 + k * 2) as u16, map);
                }
            }
        } else {
            for (k, glyph) in coverage_glyphs(gsub, coverage).into_iter().enumerate() {
                let set = subtable + gsub.u16(subtable + 6 + k * 2) as usize;
                let glyph_count = gsub.u16(set) as usize;
                for j in 0..glyph_count {
                    hash_insert(glyph, gsub.u16(set + 2 + j * 2) as u16, map);
                }
            }
        }
    }
}

// ---- HarfBuzz's hb_ot_layout_collect_lookups -----------------------------------------------------------------------

/// What collecting features has visited (HarfBuzz's `hb_collect_features_context_t`): scripts and language systems
/// by where they are in the table, and counts against HarfBuzz's limits.
struct Visits {
    scripts: BTreeSet<usize>,
    langsys: BTreeSet<usize>,
    script_count: u32,
    langsys_count: u32,
    feature_index_count: u32,
}

impl Visits {
    fn visited_feature_indices(&mut self, count: u32) -> bool {
        self.feature_index_count += count;
        self.feature_index_count > MAX_FEATURE_INDICES
    }
}

/// `langsys_collect_features` without a feature filter: the language system's required feature and all its others.
fn langsys_collect_features(gsub: Bytes<'_>, langsys: usize, visits: &mut Visits, features: &mut BTreeSet<u32>) {
    let required = gsub.u16(langsys + 2);
    let count = gsub.u16(langsys + 4);
    // (An empty language system isn't counted.)
    if required == 0xFFFF && count == 0 {
        return;
    }
    visits.langsys_count += 1;
    if visits.langsys_count - 1 > MAX_LANGSYS || !visits.langsys.insert(langsys) {
        return;
    }
    if required != 0xFFFF && !visits.visited_feature_indices(1) {
        features.insert(required);
    }
    if !visits.visited_feature_indices(count) {
        features.extend((0..count as usize).map(|i| gsub.u16(langsys + 6 + i * 2)));
    }
}

/// HarfBuzz's binary search of a list's records by tag (`hb_bsearch`), records of `size` bytes after a count.
fn find_tag(gsub: Bytes<'_>, list: usize, size: usize, tag: u32) -> Option<usize> {
    let count = gsub.u16(list) as i64;
    let (mut min, mut max) = (0i64, count - 1);
    while min <= max {
        let mid = ((min as u64 + max as u64) / 2) as i64;
        let record = gsub.u32(list + 2 + mid as usize * size);
        if tag < record {
            max = mid - 1;
        } else if tag > record {
            min = mid + 1;
        } else {
            return Some(mid as usize);
        }
    }
    None
}

/// The lookups of the features of the scripts `script_tags` in `gsub` (all their languages, all their features,
/// and the features substituting them in feature variations), as `hb_ot_layout_collect_lookups` collects them; none
/// without a tag.
pub(crate) fn collect_lookups(gsub: Bytes<'_>, script_tags: &[u32]) -> BTreeSet<u32> {
    let mut lookups = BTreeSet::new();
    let mut features = BTreeSet::new();
    let mut visits = Visits {
        scripts: BTreeSet::new(),
        langsys: BTreeSet::new(),
        script_count: 0,
        langsys_count: 0,
        feature_index_count: 0,
    };
    let script_list = gsub.u16(4) as usize;
    let feature_list = gsub.u16(6) as usize;
    for &tag in script_tags {
        if script_list == 0 {
            break;
        }
        let Some(index) = find_tag(gsub, script_list, 6, tag) else {
            continue;
        };
        let script = script_list + gsub.u16(script_list + 2 + index * 6 + 4) as usize;
        let default = gsub.u16(script);
        let langsys_count = gsub.u16(script + 2) as usize;
        // (An empty script isn't counted.)
        if default != 0 || langsys_count != 0 {
            visits.script_count += 1;
            if visits.script_count - 1 <= MAX_SCRIPTS && visits.scripts.insert(script) {
                if default != 0 {
                    langsys_collect_features(gsub, script + default as usize, &mut visits, &mut features);
                }
                for i in 0..langsys_count {
                    let offset = gsub.u16(script + 2 + 2 + i * 6 + 4) as usize;
                    if offset != 0 {
                        langsys_collect_features(gsub, script + offset, &mut visits, &mut features);
                    }
                }
            }
        }
    }
    let add_feature = |feature: usize, lookups: &mut BTreeSet<u32>| {
        let count = gsub.u16(feature + 2) as usize;
        lookups.extend((0..count).map(|i| gsub.u16(feature + 4 + i * 2)));
    };
    let feature_count = if feature_list != 0 { gsub.u16(feature_list) } else { 0 };
    for &index in &features {
        // (An index past the feature list is HarfBuzz's Null feature.)
        if index < feature_count {
            let offset = gsub.u16(feature_list + 2 + index as usize * 6 + 4) as usize;
            if offset != 0 {
                add_feature(feature_list + offset, &mut lookups);
            }
        }
    }
    // Feature variations (GSUB 1.1): each record's substitutes of the collected features.
    let variations = if gsub.u16(0) == 1 && gsub.u16(2) >= 1 { gsub.u32(10) as usize } else { 0 };
    if variations != 0 && gsub.u16(variations) == 1 {
        let records = gsub.u32(variations + 4) as usize;
        for r in 0..records {
            let substitution = gsub.u32(variations + 8 + r * 8 + 4) as usize;
            if substitution == 0 {
                continue;
            }
            let table = variations + substitution;
            if gsub.u16(table) != 1 {
                continue;
            }
            let count = gsub.u16(table + 4) as usize;
            for s in 0..count {
                let record = table + 6 + s * 6;
                let feature_index = gsub.u16(record);
                let alternate = gsub.u32(record + 2) as usize;
                if features.contains(&feature_index) && alternate != 0 {
                    add_feature(table + alternate, &mut lookups);
                }
            }
        }
    }
    lookups
}
