//! The styles tracked on a field's text (Compose's TextStyleBuffer over its IntIntervalTree): each a span or
//! paragraph style over a range, kept in the order added, its range following the text's edits — text inserted at
//! its start or end taken in or not as its expand policy says, a range deleted away removing it.
//!
//! Compose keeps the ranges in a red-black interval tree in "gap buffer" coordinates (a gap at the last edit, so
//! that typing moves only the gap); here they are a list in the text's own coordinates, each edit applied to each
//! range as a deletion then an insertion with the same expand rules: the same ranges come out (and the same styles
//! for a query, the same transitions), in the order added. Queries walk the list.

use kompoz_text::annotated_string::{Annotation, Range};
use kompoz_text::{ParagraphStyle, SpanStyle};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_BUFFER_ID: AtomicU64 = AtomicU64::new(1);

/// A style and where it is.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct StyleEntry {
    /// Stable for the style's life in this buffer and its copies.
    pub(crate) id: u32,
    pub(crate) item: Annotation,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) start_expands: bool,
    pub(crate) end_expands: bool,
}

/// A kind of style a query asks for (Compose's reified `R`).
pub(crate) trait StyleKind: Sized {
    fn of(annotation: &Annotation) -> Option<&Self>;
}

impl StyleKind for SpanStyle {
    fn of(annotation: &Annotation) -> Option<&SpanStyle> {
        match annotation {
            Annotation::SpanStyle(s) => Some(s),
            _ => None,
        }
    }
}

impl StyleKind for ParagraphStyle {
    fn of(annotation: &Annotation) -> Option<&ParagraphStyle> {
        match annotation {
            Annotation::ParagraphStyle(s) => Some(s),
            _ => None,
        }
    }
}

/// Whether `[q_start, q_end)` and `[s_start, s_end)` meet: they overlap, or one is a point at the other's start
/// (Compose's `intersect`, ranges closed at their start and open at their end).
fn intersect(l_start: usize, l_end: usize, r_start: usize, r_end: usize) -> bool {
    ((l_start == l_end || r_start == r_end) && l_start == r_start) || (l_start < r_end && r_start < l_end)
}

/// A field's tracked styles (Compose's TextStyleBuffer).
#[derive(Clone, Debug)]
pub struct TextStyleBuffer {
    /// Which buffer it is (Compose's `id`): a copy is another; a tracked range is valid only in its own.
    pub(crate) id: u64,
    mutable: bool,
    entries: Vec<StyleEntry>,
    next_entry_id: u32,
}

impl Default for TextStyleBuffer {
    fn default() -> Self {
        TextStyleBuffer::new()
    }
}

/// The same styles over the same ranges, in the same order (Compose's interval tree equality).
impl PartialEq for TextStyleBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.entries.len() == other.entries.len()
            && self.entries.iter().zip(&other.entries).all(|(a, b)| {
                a.item == b.item
                    && a.start == b.start
                    && a.end == b.end
                    && a.start_expands == b.start_expands
                    && a.end_expands == b.end_expands
            })
    }
}

impl TextStyleBuffer {
    /// An empty, mutable one.
    pub fn new() -> TextStyleBuffer {
        TextStyleBuffer {
            id: NEXT_BUFFER_ID.fetch_add(1, Ordering::Relaxed),
            mutable: true,
            entries: Vec::new(),
            next_entry_id: 1,
        }
    }

    /// A mutable copy of `source` (a buffer of its own).
    pub fn copy_of(source: &TextStyleBuffer) -> TextStyleBuffer {
        TextStyleBuffer { id: NEXT_BUFFER_ID.fetch_add(1, Ordering::Relaxed), mutable: true, ..source.clone() }
    }

    /// An immutable copy (Compose's `toImmutable`).
    pub fn to_immutable(&self) -> TextStyleBuffer {
        TextStyleBuffer { id: NEXT_BUFFER_ID.fetch_add(1, Ordering::Relaxed), mutable: false, ..self.clone() }
    }

    fn check_mutable(&self) {
        if !self.mutable {
            panic!("This TextStyleBuffer is immutable");
        }
    }

    /// Takes `other`'s styles (Compose's `syncTo`).
    pub fn sync_to(&mut self, other: &TextStyleBuffer) {
        self.check_mutable();
        self.entries = other.entries.clone();
        self.next_entry_id = self.next_entry_id.max(other.next_entry_id);
    }

    /// Whether it holds no style.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Adds `item` over `start..end`, expanding at its start and end as told; the new style's id.
    pub(crate) fn add_style(
        &mut self,
        item: Annotation,
        start: usize,
        end: usize,
        start_expands: bool,
        end_expands: bool,
    ) -> u32 {
        self.check_mutable();
        let id = self.next_entry_id;
        self.next_entry_id += 1;
        self.entries.push(StyleEntry { id, item, start, end, start_expands, end_expands });
        id
    }

    /// Removes style `id`: whether it was there.
    pub(crate) fn remove_style(&mut self, id: u32) -> bool {
        self.check_mutable();
        let before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() != before
    }

    pub(crate) fn entry(&self, id: u32) -> Option<&StyleEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    fn entry_mut(&mut self, id: u32) -> Option<&mut StyleEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// The ids of the styles of kind `R` meeting `start..end`, in the order added (Compose's `getStyles`).
    pub(crate) fn get_styles<R: StyleKind>(&self, start: usize, end: usize) -> Vec<u32> {
        if start > end {
            return Vec::new();
        }
        self.entries
            .iter()
            .filter(|e| R::of(&e.item).is_some() && intersect(start, end, e.start, e.end))
            .map(|e| e.id)
            .collect()
    }

    /// The styles of kind `R` meeting `start..end` and where they are, in the order added (Compose's
    /// `getImmutableStyles`).
    pub(crate) fn get_immutable_styles<R: StyleKind + Clone>(&self, start: usize, end: usize) -> Vec<Range<R>> {
        if start > end {
            return Vec::new();
        }
        self.entries
            .iter()
            .filter(|e| intersect(start, end, e.start, e.end))
            .filter_map(|e| R::of(&e.item).map(|r| Range::new(r.clone(), e.start, e.end)))
            .collect()
    }

    /// All the styles and where they are, in the order added (Compose's `getAllStyles`).
    pub fn get_all_styles(&self) -> Vec<Range<Annotation>> {
        self.entries.iter().map(|e| Range::new(e.item.clone(), e.start, e.end)).collect()
    }

    pub(crate) fn set_range(&mut self, id: u32, start: usize, end: usize) {
        self.check_mutable();
        assert!(start < end, "Reversed or collapsed range is not accepted.");
        let e = self.entry_mut(id).expect("TrackedRange is not found.");
        e.start = start;
        e.end = end;
    }

    pub(crate) fn set_expand_policy(&mut self, id: u32, start_expands: bool, end_expands: bool) {
        self.check_mutable();
        let e = self.entry_mut(id).expect("TrackedRange is not found.");
        e.start_expands = start_expands;
        e.end_expands = end_expands;
    }

    pub(crate) fn set_item(&mut self, id: u32, item: Annotation) {
        self.check_mutable();
        let e = self.entry_mut(id).expect("TrackedRange is not found.");
        e.item = item;
    }

    /// The text from `start` to `end` replaced by `new_length` bytes: a deletion of the range (styles left empty
    /// removed), then an insertion at `start`, taken in by a style starting there that expands at its start, by
    /// one ending there that expands at its end, and by one around it (Compose's `replaceText`). False when no
    /// style is tracked.
    pub fn replace_text(&mut self, start: usize, end: usize, new_length: usize) -> bool {
        self.check_mutable();
        if self.entries.is_empty() {
            return false;
        }
        let (start, end) = (start.min(end), start.max(end));
        let deleted = end - start;
        if deleted > 0 {
            let at = |v: usize| {
                if v <= start {
                    v
                } else if v >= end {
                    v - deleted
                } else {
                    start
                }
            };
            for e in &mut self.entries {
                e.start = at(e.start);
                e.end = at(e.end);
            }
            self.entries.retain(|e| e.start < e.end);
        }
        if new_length > 0 {
            for e in &mut self.entries {
                if e.start > start || (e.start == start && !e.start_expands) {
                    e.start += new_length;
                }
                if e.end > start || (e.end == start && e.end_expands) {
                    e.end += new_length;
                }
            }
        }
        true
    }

    /// The first start or end of a style after `start`, or `limit` if none comes before it (Compose's
    /// `nextTransition`).
    pub fn next_transition(&self, start: usize, limit: usize) -> usize {
        assert!(limit >= start, "limit ({limit}) cannot be less than start ({start})");
        self.entries.iter().flat_map(|e| [e.start, e.end]).filter(|&v| v > start).fold(limit, usize::min)
    }

    /// The last start or end of a style before `start`, or `limit` if none comes after it (Compose's
    /// `previousTransition`).
    pub fn previous_transition(&self, start: usize, limit: usize) -> usize {
        assert!(limit <= start, "limit ({limit}) cannot be greater than start ({start})");
        self.entries.iter().flat_map(|e| [e.start, e.end]).filter(|&v| v < start).fold(limit, usize::max)
    }

    /// Removes every style (Compose's `clear`).
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
