//! The shaping caches (D6): runs shaped once are kept by what decides their glyphs (the text and the text around
//! it, the font and its coordinates, the direction, script, language and features), in font units, so the same
//! word at another size, letter spacing or stretch is not shaped again; HarfBuzz shape plans are kept by font
//! and settings.

use crate::font::{FontFeature, FontId};
use harfrust::ShapePlan;
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

/// A cache keeping about the `capacity` entries used last (two generations: what is looked up moves to the
/// young one; when it fills, the old one is dropped and the young one becomes it), O(1) a lookup.
pub(crate) struct Cache<K, V> {
    young: HashMap<K, V>,
    old: HashMap<K, V>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<K: Hash + Eq, V: Clone> Cache<K, V> {
    pub(crate) fn new(capacity: usize) -> Cache<K, V> {
        Cache { young: HashMap::new(), old: HashMap::new(), capacity: capacity.max(2), hits: 0, misses: 0 }
    }

    pub(crate) fn get(&mut self, key: &K) -> Option<V> {
        if let Some(v) = self.young.get(key) {
            self.hits += 1;
            return Some(v.clone());
        }
        if let Some((k, v)) = self.old.remove_entry(key) {
            self.hits += 1;
            self.insert_young(k, v.clone());
            return Some(v);
        }
        self.misses += 1;
        None
    }

    pub(crate) fn insert(&mut self, key: K, value: V) {
        self.insert_young(key, value);
    }

    fn insert_young(&mut self, key: K, value: V) {
        if self.young.len() >= self.capacity / 2 {
            self.old = std::mem::take(&mut self.young);
        }
        self.young.insert(key, value);
    }

    pub(crate) fn len(&self) -> usize {
        self.young.len() + self.old.len()
    }

    pub(crate) fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }
}

/// How often the shaping caches answered: lookups that found a shaped run or a shape plan, and those that didn't
/// (and shaped or planned), and how many of each are kept.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ShapeCacheStats {
    pub run_hits: u64,
    pub run_misses: u64,
    pub runs: usize,
    pub plan_hits: u64,
    pub plan_misses: u64,
    pub plans: usize,
}

/// What decides a run's glyphs: its text, up to five characters on each side of it (what joining and kerning
/// across its ends see), the font at its coordinates, the direction, the script (None: guessed from the text),
/// the language and the features.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct RunKey {
    pub(crate) text: Box<str>,
    pub(crate) before: Box<str>,
    pub(crate) after: Box<str>,
    pub(crate) begins_text: bool,
    pub(crate) ends_text: bool,
    pub(crate) font: FontId,
    pub(crate) coords: Rc<[i16]>,
    pub(crate) rtl: bool,
    pub(crate) script: Option<[u8; 4]>,
    pub(crate) language: Option<Box<str>>,
    pub(crate) features: Box<[FontFeature]>,
}

/// A glyph as shaped, in font units, its cluster an offset into the run's text.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct RawGlyph {
    pub(crate) id: u32,
    pub(crate) cluster: u32,
    pub(crate) x_advance: i32,
    pub(crate) y_advance: i32,
    pub(crate) x_offset: i32,
    pub(crate) y_offset: i32,
    pub(crate) unsafe_to_break: bool,
}

/// A run as shaped: its glyphs in visual order and the script it was shaped as.
pub(crate) struct RawRun {
    pub(crate) glyphs: Vec<RawGlyph>,
    pub(crate) script: [u8; 4],
}

/// What a shape plan is for.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlanKey {
    pub(crate) font: FontId,
    pub(crate) coords: Rc<[i16]>,
    pub(crate) rtl: bool,
    pub(crate) script: Option<[u8; 4]>,
    pub(crate) language: Option<Box<str>>,
    pub(crate) features: Box<[FontFeature]>,
}

/// The shaped runs and plans kept.
pub(crate) struct ShapeCaches {
    pub(crate) runs: Cache<RunKey, Rc<RawRun>>,
    pub(crate) plans: Cache<PlanKey, Rc<ShapePlan>>,
}

impl ShapeCaches {
    /// Room for 8192 shaped runs (the C++ Kompoz's cache's size) and 256 plans.
    pub(crate) fn new() -> ShapeCaches {
        ShapeCaches { runs: Cache::new(8192), plans: Cache::new(256) }
    }

    pub(crate) fn stats(&self) -> ShapeCacheStats {
        let (run_hits, run_misses) = self.runs.stats();
        let (plan_hits, plan_misses) = self.plans.stats();
        ShapeCacheStats { run_hits, run_misses, runs: self.runs.len(), plan_hits, plan_misses, plans: self.plans.len() }
    }
}
