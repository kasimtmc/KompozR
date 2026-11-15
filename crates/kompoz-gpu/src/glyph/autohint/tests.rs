//! The parts of the automatic hinter FreeType 2.14 and 2.15 added, on tables and outlines made up for them: the
//! hash table the reverse character map is built in, the `GSUB` reading and closure, the vertical separation of
//! diacritics, the style map's encoding. (The hinter as a whole is checked against FreeType's own glyphs in
//! `glyph::tests::freetype`.)

use super::adjust::{self, FtHash};
use super::closure::{GlyphSet, substitute_closure};
use super::gsub::{Bytes, collect_lookups, coverage_glyphs, map_lookup, single_alternate_lookups};
use super::hint::diacritics::separate;
use super::outline::Outline;
use super::style::GlyphStyle;
use skrifa::raw::model::glyph::outline::UnscaledOutlineSink;
use skrifa::raw::tables::glyf::PointFlags;
use std::collections::BTreeSet;

#[test]
fn freetypes_hash_probes_downwards_and_keeps_entries_through_growth() {
    // FreeType's Mocklisp hash of a key's bytes, in 241 buckets at first: two keys of one bucket, the second put in
    // the bucket below, so that iteration (in bucket order) gives it first.
    let hash = |k: u32| -> u64 {
        let mut r = (k & 0xFF) as u64;
        for shift in [8, 16, 24] {
            r = (r << 5) - r + ((k >> shift) & 0xFF) as u64;
        }
        r % 241
    };
    let a = 7u32;
    let b = (8..).find(|&k| hash(k) == hash(a)).unwrap();
    let mut map = FtHash::new();
    map.insert(a, 1);
    map.insert(b, 2);
    let order: Vec<u32> = map.entries().into_iter().map(|(k, _)| k).collect();
    assert_eq!(order, [b, a]);
    // Insertion without overwriting keeps what a key has; `set` replaces it.
    assert!(!map.insert_no_overwrite(a, 9));
    assert_eq!(map.lookup(a), Some(1));
    map.set(a, 5);
    assert_eq!(map.lookup(a), Some(5));
    assert!(map.insert_no_overwrite(1000, 3));
    // Past a third full, the table doubles: everything is still there.
    for k in 2000..2300 {
        map.insert(k, k as u64);
    }
    assert!((2000..2300).all(|k| map.lookup(k) == Some(k as u64)));
    assert_eq!((map.lookup(a), map.lookup(b), map.lookup(1000)), (Some(5), Some(2), Some(3)));
}

/// A `GSUB` table written field by field, offsets patched in once their targets are placed.
#[derive(Default)]
struct Table(Vec<u8>);

impl Table {
    fn u16(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&(v as u16).to_be_bytes());
        self
    }

    fn tag(&mut self, tag: &[u8; 4]) -> &mut Self {
        self.0.extend_from_slice(tag);
        self
    }

    fn pos(&self) -> usize {
        self.0.len()
    }

    /// A 16-bit offset field to fill in later.
    fn slot(&mut self) -> usize {
        self.u16(0);
        self.pos() - 2
    }

    /// Slot `slot` pointing at here, from `base`.
    fn patch(&mut self, slot: usize, base: usize) {
        let offset = (self.pos() - base) as u16;
        self.0[slot..slot + 2].copy_from_slice(&offset.to_be_bytes());
    }
}

/// A table of six lookups, the `latn` script's one feature having the first two:
/// 0. single substitution (format 2) of a coverage of two ranges, 10–11 and 20–21, by 110, 111, 120, 121;
/// 1. alternates 130 and 131 of 30;
/// 2. a ligature 140 of 40 and 41;
/// 3. single substitution (format 1) of 50–52 by 1 more;
/// 4. a chained context (format 3) on 60 or 61 applying lookup 5;
/// 5. single substitution (format 2) of 60 and 61 by 160 and 161.
fn gsub() -> Vec<u8> {
    let mut t = Table::default();
    t.u16(1).u16(0);
    let (script_list, feature_list, lookup_list) = (t.slot(), t.slot(), t.slot());
    t.patch(script_list, 0);
    let sl = t.pos();
    t.u16(1).tag(b"latn");
    let script = t.slot();
    t.patch(script, sl);
    let s = t.pos();
    let default = t.slot();
    t.u16(0);
    t.patch(default, s);
    t.u16(0).u16(0xFFFF).u16(1).u16(0);
    t.patch(feature_list, 0);
    let fl = t.pos();
    t.u16(1).tag(b"smcp");
    let feature = t.slot();
    t.patch(feature, fl);
    t.u16(0).u16(2).u16(0).u16(1);
    t.patch(lookup_list, 0);
    let ll = t.pos();
    t.u16(6);
    let lookups: Vec<usize> = (0..6).map(|_| t.slot()).collect();
    let lookup = |t: &mut Table, slot: usize, kind: u32| -> usize {
        t.patch(slot, ll);
        let l = t.pos();
        t.u16(kind).u16(0).u16(1);
        let sub = t.slot();
        t.patch(sub, l);
        t.pos()
    };
    // 0: single substitution, format 2.
    let s = lookup(&mut t, lookups[0], 1);
    t.u16(2);
    let coverage = t.slot();
    t.u16(4).u16(110).u16(111).u16(120).u16(121);
    t.patch(coverage, s);
    t.u16(2).u16(2).u16(10).u16(11).u16(0).u16(20).u16(21).u16(2);
    // 1: alternates.
    let s = lookup(&mut t, lookups[1], 3);
    t.u16(1);
    let coverage = t.slot();
    t.u16(1);
    let set = t.slot();
    t.patch(coverage, s);
    t.u16(1).u16(1).u16(30);
    t.patch(set, s);
    t.u16(2).u16(130).u16(131);
    // 2: a ligature.
    let s = lookup(&mut t, lookups[2], 4);
    t.u16(1);
    let coverage = t.slot();
    t.u16(1);
    let set = t.slot();
    t.patch(coverage, s);
    t.u16(1).u16(1).u16(40);
    t.patch(set, s);
    let ls = t.pos();
    t.u16(1);
    let ligature = t.slot();
    t.patch(ligature, ls);
    t.u16(140).u16(2).u16(41);
    // 3: single substitution, format 1, a delta of 1.
    let s = lookup(&mut t, lookups[3], 1);
    t.u16(1);
    let coverage = t.slot();
    t.u16(1);
    t.patch(coverage, s);
    t.u16(2).u16(1).u16(50).u16(52).u16(0);
    // 4: a chained context, format 3.
    let s = lookup(&mut t, lookups[4], 6);
    t.u16(3).u16(0).u16(1);
    let input = t.slot();
    t.u16(0).u16(1).u16(0).u16(5);
    t.patch(input, s);
    t.u16(1).u16(2).u16(60).u16(61);
    // 5: single substitution, format 2.
    let s = lookup(&mut t, lookups[5], 1);
    t.u16(2);
    let coverage = t.slot();
    t.u16(2).u16(160).u16(161);
    t.patch(coverage, s);
    t.u16(1).u16(2).u16(60).u16(61);
    t.0
}

#[test]
fn single_and_alternate_lookups_are_mapped_as_freetype_maps_them() {
    let table = gsub();
    let bytes = Bytes(&table);
    // The script's feature's lookups, as HarfBuzz collects them.
    assert_eq!(collect_lookups(bytes, &[u32::from_be_bytes(*b"latn")]), BTreeSet::from([0, 1]));
    assert!(collect_lookups(bytes, &[u32::from_be_bytes(*b"cyrl")]).is_empty());
    // Single and alternate substitutions are valid, a ligature isn't.
    let lookups = single_alternate_lookups(bytes);
    assert_eq!(lookups.len(), 6);
    assert!(lookups[0] != 0 && lookups[1] != 0 && lookups[3] != 0 && lookups[5] != 0);
    assert_eq!((lookups[2], lookups[4]), (0, 0));
    // FreeType's coverage iterator stops on reading the last range: only its first glyph comes out.
    let s = lookups[0] as usize + bytes.u16(lookups[0] as usize + 6) as usize;
    assert_eq!(coverage_glyphs(bytes, s + bytes.u16(s + 2) as usize), [10, 11, 20]);
    let mut map = FtHash::new();
    map_lookup(bytes, &mut map, lookups[0]);
    assert_eq!(
        (map.lookup(10), map.lookup(11), map.lookup(20), map.lookup(21)),
        (Some(110), Some(111), Some(120), None)
    );
    // Several substitutes: the first at the glyph with the others' count above it, the next ones at keys above.
    map_lookup(bytes, &mut map, lookups[1]);
    assert_eq!(map.lookup(30), Some(130 + (1 << 16)));
    assert_eq!(map.lookup(30 + (1 << 16)), Some(131));
}

#[test]
fn closures_are_harfbuzzs() {
    let table = gsub();
    let bytes = Bytes(&table);
    let closure = |lookups: &[u32], glyphs: &[u32]| -> Vec<u32> {
        let mut set = GlyphSet::new();
        glyphs.iter().for_each(|&g| set.insert(g));
        substitute_closure(bytes, &lookups.iter().copied().collect(), &mut set, 1000);
        set.iter().collect()
    };
    // HarfBuzz's coverage iterator reads every range (unlike FreeType's).
    assert_eq!(closure(&[0, 1], &[10, 21, 30]), [10, 21, 30, 110, 121, 130, 131]);
    // A ligature needs all its components.
    assert_eq!(closure(&[2], &[40]), [40]);
    assert_eq!(closure(&[2], &[40, 41]), [40, 41, 140]);
    // A delta that maps a run of glyphs onto itself is left out; a glyph alone moves.
    assert_eq!(closure(&[3], &[50, 51, 52]), [50, 51, 52]);
    // (Moved, the glyph and its substitute make such a run in turn.)
    assert_eq!(closure(&[3], &[50]), [50, 51]);
    // A nested lookup substitutes only the glyphs active at its position.
    assert_eq!(closure(&[4], &[60]), [60, 160]);
    // Outputs past the font's glyphs are dropped.
    let mut set = GlyphSet::new();
    set.insert(60);
    substitute_closure(bytes, &BTreeSet::from([4]), &mut set, 100);
    assert_eq!(set.iter().collect::<Vec<_>>(), [60]);
}

/// An outline of rectangles (26.6, y up) as the hinter holds one once fitted.
fn boxes(rects: &[(i32, i32, i32, i32)]) -> Outline {
    let mut outline = Outline::default();
    for &(x0, y0, x1, y1) in rects {
        outline.start_contour();
        for (x, y) in [(x0, y0), (x0, y1), (x1, y1), (x1, y0)] {
            outline.point(x, y, PointFlags::on_curve());
        }
        outline.end_contour();
    }
    for point in &mut outline.points {
        (point.x, point.y) = (point.fx, point.fy);
    }
    outline
}

fn y_range(outline: &Outline, contour: usize) -> (i32, i32) {
    let ys = outline.points[contour * 4..contour * 4 + 4].iter().map(|p| p.y);
    (ys.clone().min().unwrap(), ys.max().unwrap())
}

#[test]
fn diacritics_are_moved_a_pixel_clear() {
    // A base 0..10 × 0..5 px and a dot 3..5 × 5.5..7 above it: half a pixel apart, moved up half a pixel more.
    let mut o = boxes(&[(0, 0, 640, 320), (192, 352, 320, 448)]);
    separate(&mut o, adjust::UP, 6 * 64);
    assert_eq!(y_range(&o, 1).0, 6 * 64);
    assert_eq!(y_range(&o, 0), (0, 5 * 64));
    // Already a pixel clear: left.
    let mut clear = boxes(&[(0, 0, 640, 320), (192, 384, 320, 448)]);
    separate(&mut clear, adjust::UP, 6 * 64);
    assert_eq!(y_range(&clear, 1), (384, 448));
    // Too tall for a diacritic: left.
    let mut tall = boxes(&[(0, 0, 640, 320), (192, 352, 320, 768)]);
    separate(&mut tall, adjust::UP, 6 * 64);
    assert_eq!(y_range(&tall, 1), (352, 768));
    // Beside the base, not over it: left.
    let mut beside = boxes(&[(0, 0, 640, 320), (704, 352, 768, 448)]);
    separate(&mut beside, adjust::UP, 6 * 64);
    assert_eq!(y_range(&beside, 1), (352, 448));
    // Below: a cedilla-like contour moved down a pixel clear.
    let mut below = boxes(&[(0, 0, 640, 320), (192, -128, 320, -16)]);
    separate(&mut below, adjust::DOWN, 6 * 64);
    assert_eq!(y_range(&below, 1).1, -64);
}

#[test]
fn style_maps_are_encoded_as_freetypes() {
    // A style index; unassigned 0x1FFF; non-base 0x4000; a digit 0x8000.
    assert_eq!(GlyphStyle::from_raw_parts(57, false, false).to_freetype(), 57);
    assert_eq!(GlyphStyle::default().to_freetype(), 0x1FFF);
    assert_eq!(GlyphStyle::from_raw_parts(2, true, true).to_freetype(), 0xC002);
}
