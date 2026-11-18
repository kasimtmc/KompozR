//! Which items go on which line of a lazy grid (Compose's LazyGridSpanLayoutProvider.kt): with spans of 1 a line is
//! `slots_per_line` items; with spans of one's own the lines are worked out item by item, from the closest known
//! line start — the starts of every `bucket_size`-th line kept, the last line asked for and the bucket being walked
//! back through cached — so that scrolling either way doesn't walk from the first item.

use super::cells::{GridItemSpan, LazyGridItemSpanScope};
use super::dsl::LazyGridScope;
use crate::lazy::layout::{IntervalList, LazyLayoutIntervalContent};
use std::cell::RefCell;

/// The items of a line (Compose's `LineConfiguration`): the first one's index and each one's span.
#[derive(Clone, Debug)]
pub(crate) struct LineConfiguration {
    pub(crate) first_item_index: i32,
    pub(crate) spans: Vec<GridItemSpan>,
}

/// A line whose start is known (Compose's Bucket): its first item, and that item's span if it broke the line
/// before (0 otherwise).
#[derive(Clone, Copy)]
struct Bucket {
    first_item_index: i32,
    first_item_known_span: i32,
}

impl Bucket {
    fn new(first_item_index: i32) -> Bucket {
        Bucket { first_item_index, first_item_known_span: 0 }
    }
}

struct Cache {
    buckets: Vec<Bucket>,
    last_line_index: i32,
    last_line_start_item_index: i32,
    last_line_start_known_span: i32,
    cached_bucket_index: i32,
    cached_bucket: Vec<i32>,
    previous_default_spans: Vec<GridItemSpan>,
    slots_per_line: i32,
}

impl Cache {
    fn invalidate(&mut self) {
        self.buckets.clear();
        self.buckets.push(Bucket::new(0));
        self.last_line_index = 0;
        self.last_line_start_item_index = 0;
        self.last_line_start_known_span = 0;
        self.cached_bucket_index = -1;
        self.cached_bucket.clear();
    }
}

/// Compose's LazyGridSpanLayoutProvider, of one grid content (its caches as the content's: made again with it).
pub(crate) struct LazyGridSpanLayoutProvider {
    cache: RefCell<Cache>,
}

impl Default for LazyGridSpanLayoutProvider {
    fn default() -> Self {
        LazyGridSpanLayoutProvider {
            cache: RefCell::new(Cache {
                buckets: vec![Bucket::new(0)],
                last_line_index: 0,
                last_line_start_item_index: 0,
                last_line_start_known_span: 0,
                cached_bucket_index: -1,
                cached_bucket: Vec::new(),
                previous_default_spans: Vec::new(),
                slots_per_line: 0,
            }),
        }
    }
}

impl LazyGridSpanLayoutProvider {
    /// The cells of a line (Compose's `slotsPerLine`).
    pub(crate) fn slots_per_line(&self) -> i32 {
        self.cache.borrow().slots_per_line
    }

    /// Sets the cells of a line (the caches forgotten when it changes).
    pub(crate) fn set_slots_per_line(&self, value: i32) {
        let mut cache = self.cache.borrow_mut();
        if value != cache.slots_per_line {
            cache.slots_per_line = value;
            cache.invalidate();
        }
    }

    fn get_default_spans(cache: &mut Cache, current_slots_per_line: i32) -> Vec<GridItemSpan> {
        if current_slots_per_line as usize != cache.previous_default_spans.len() {
            cache.previous_default_spans = vec![GridItemSpan(1); current_slots_per_line as usize];
        }
        cache.previous_default_spans.clone()
    }

    /// The bucket size: lines between two known starts (Compose's `bucketSize`).
    fn bucket_size(content: &LazyGridScope, slots_per_line: i32) -> i32 {
        ((content.item_count() as f64 / slots_per_line as f64).sqrt()) as i32 + 1
    }

    /// The items of line `line_index` (Compose's `getLineConfiguration`).
    pub(crate) fn get_line_configuration(&self, content: &LazyGridScope, line_index: i32) -> LineConfiguration {
        let total_size = content.item_count();
        let mut cache = self.cache.borrow_mut();
        let slots_per_line = cache.slots_per_line;
        if !content.has_custom_spans {
            // (spans of 1: a line is a count of items)
            let first_item_index = line_index * slots_per_line;
            let count = slots_per_line.min(total_size - first_item_index).max(0);
            return LineConfiguration { first_item_index, spans: Self::get_default_spans(&mut cache, count) };
        }

        let bucket_size = Self::bucket_size(content, slots_per_line);
        let bucket_index = (line_index / bucket_size).min(cache.buckets.len() as i32 - 1);
        // From the closest bucket start…
        let mut current_line = bucket_index * bucket_size;
        let mut current_item_index = cache.buckets[bucket_index as usize].first_item_index;
        let mut known_current_item_span = cache.buckets[bucket_index as usize].first_item_known_span;
        // …or the more local cached values.
        if (current_line..=line_index).contains(&cache.last_line_index) {
            // (scrolling along: the last line asked for is closer)
            current_line = cache.last_line_index;
            current_item_index = cache.last_line_start_item_index;
            known_current_item_span = cache.last_line_start_known_span;
        } else if bucket_index == cache.cached_bucket_index
            && ((line_index - current_line) as usize) < cache.cached_bucket.len()
        {
            // (scrolling back: the line's start is in the bucket cached)
            current_item_index = cache.cached_bucket[(line_index - current_line) as usize];
            current_line = line_index;
            known_current_item_span = 0;
        }

        let cache_this_bucket =
            current_line % bucket_size == 0 && (2..bucket_size).contains(&(line_index - current_line));
        if cache_this_bucket {
            cache.cached_bucket_index = bucket_index;
            cache.cached_bucket.clear();
        }

        assert!(current_line <= line_index, "currentLine ({current_line}) > lineIndex ({line_index})");

        while current_line < line_index && current_item_index < total_size {
            if cache_this_bucket {
                cache.cached_bucket.push(current_item_index);
            }
            let mut spans_used = 0;
            while spans_used < slots_per_line && current_item_index < total_size {
                let span = if known_current_item_span == 0 {
                    span_of(content, current_item_index, slots_per_line - spans_used, slots_per_line)
                } else {
                    std::mem::take(&mut known_current_item_span)
                };
                if spans_used + span > slots_per_line {
                    known_current_item_span = span;
                    break;
                }
                current_item_index += 1;
                spans_used += span;
            }
            current_line += 1;
            if current_line % bucket_size == 0 && current_item_index < total_size {
                let current_line_bucket = current_line / bucket_size;
                // (it would have been the start otherwise)
                assert!(cache.buckets.len() as i32 == current_line_bucket, "invalid starting point");
                cache.buckets.push(Bucket {
                    first_item_index: current_item_index,
                    first_item_known_span: known_current_item_span,
                });
            }
        }

        cache.last_line_index = line_index;
        cache.last_line_start_item_index = current_item_index;
        cache.last_line_start_known_span = known_current_item_span;

        let first_item_index = current_item_index;
        let mut spans = Vec::new();
        let mut spans_used = 0;
        while spans_used < slots_per_line && current_item_index < total_size {
            let span = if known_current_item_span == 0 {
                span_of(content, current_item_index, slots_per_line - spans_used, slots_per_line)
            } else {
                std::mem::take(&mut known_current_item_span)
            };
            if spans_used + span > slots_per_line {
                break;
            }
            current_item_index += 1;
            spans.push(GridItemSpan(span));
            spans_used += span;
        }
        LineConfiguration { first_item_index, spans }
    }

    /// The line of item `item_index` (Compose's `getLineIndexOfItem`).
    pub(crate) fn get_line_index_of_item(&self, content: &LazyGridScope, item_index: i32) -> i32 {
        let total_size = content.item_count();
        if total_size <= 0 {
            return 0;
        }
        assert!(item_index < total_size, "ItemIndex > total count");
        let mut cache = self.cache.borrow_mut();
        let slots_per_line = cache.slots_per_line;
        if !content.has_custom_spans {
            return item_index / slots_per_line;
        }
        let bucket_size = Self::bucket_size(content, slots_per_line);

        // (the last bucket starting at or before the item)
        let lower_bound_bucket = match cache.buckets.binary_search_by(|b| b.first_item_index.cmp(&item_index)) {
            Ok(at) => at as i32,
            Err(insertion) => insertion as i32 - 1,
        };
        let mut current_line = lower_bound_bucket * bucket_size;
        let mut current_item_index = cache.buckets[lower_bound_bucket as usize].first_item_index;
        assert!(current_item_index <= item_index, "currentItemIndex > itemIndex");
        let mut spans_used = 0;
        while current_item_index < item_index {
            let span = span_of(content, current_item_index, slots_per_line - spans_used, slots_per_line);
            current_item_index += 1;
            if spans_used + span < slots_per_line {
                spans_used += span;
            } else if spans_used + span == slots_per_line {
                current_line += 1;
                spans_used = 0;
            } else {
                // (it breaks the line)
                current_line += 1;
                spans_used = span;
            }
            if current_line % bucket_size == 0 {
                let current_line_bucket = current_line / bucket_size;
                if current_line_bucket >= cache.buckets.len() as i32 {
                    cache.buckets.push(Bucket::new(current_item_index - if spans_used > 0 { 1 } else { 0 }));
                }
            }
        }
        if spans_used + span_of(content, item_index, slots_per_line - spans_used, slots_per_line) > slots_per_line {
            current_line += 1;
        }
        current_line
    }

    /// Item `item_index`'s span with `max_span` cells left on its line (Compose's `spanOf`).
    pub(crate) fn span_of(&self, content: &LazyGridScope, item_index: i32, max_span: i32) -> i32 {
        let slots_per_line = self.slots_per_line();
        span_of(content, item_index, max_span, slots_per_line)
    }
}

/// The span of item `item_index` with `max_span` cells left on a line of `slots_per_line`: its interval's span
/// function run.
fn span_of(content: &LazyGridScope, item_index: i32, max_span: i32, slots_per_line: i32) -> i32 {
    let scope = LazyGridItemSpanScope { max_current_line_span: max_span, max_line_span: slots_per_line };
    let interval = content.intervals().get(item_index);
    let local_interval_index = item_index - interval.start_index;
    (interval.value.span)(&scope, local_interval_index).current_line_span()
}
