//! Maps offsets between a text and the text some edits made of it (Compose's OffsetMappingCalculator), both ways:
//! one-to-one where it can be told, else to the range of offsets it could be (an insertion point maps to all of the
//! inserted text, the inside of a replaced range to all of the replacement). Edits are recorded in order, each in
//! the terms of the text the ones before left. Offsets are bytes.

use kompoz_text::TextRange;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Op {
    offset: usize,
    src_len: usize,
    dest_len: usize,
}

/// The edits made to a text, to map offsets through (Compose's OffsetMappingCalculator).
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct OffsetMappingCalculator {
    ops: Vec<Op>,
}

impl OffsetMappingCalculator {
    pub fn new() -> OffsetMappingCalculator {
        OffsetMappingCalculator::default()
    }

    /// The text from `source_start` to `source_end` (of the text the edits so far left) replaced by `new_length`
    /// bytes. A same-length replacement of under two bytes maps one-to-one and isn't kept (Compose's).
    pub fn record_edit_operation(&mut self, source_start: usize, source_end: usize, new_length: usize) {
        let source_min = source_start.min(source_end);
        let source_max = source_min.max(source_end);
        let source_length = source_max - source_min;
        if source_length < 2 && source_length == new_length {
            return;
        }
        self.ops.push(Op { offset: source_min, src_len: source_length, dest_len: new_length });
    }

    /// Where `offset` of the original text is in the edited one (a range where it can't be told).
    pub fn map_from_source(&self, offset: usize) -> TextRange {
        self.map(offset, true)
    }

    /// Where `offset` of the edited text is in the original one (a range where it can't be told).
    pub fn map_from_dest(&self, offset: usize) -> TextRange {
        self.map(offset, false)
    }

    fn map(&self, offset: usize, from_source: bool) -> TextRange {
        let (mut start, mut end) = (offset, offset);
        let mut step = |op: &Op| {
            let new_start = map_step(start, op, from_source);
            let new_end = map_step(end, op, from_source);
            start = new_start.start.min(new_end.start);
            end = new_start.end.max(new_end.end);
        };
        if from_source {
            self.ops.iter().for_each(&mut step);
        } else {
            self.ops.iter().rev().for_each(&mut step);
        }
        TextRange::new(start, end)
    }
}

fn map_step(offset: usize, op: &Op, from_source: bool) -> TextRange {
    let (src_len, dest_len) = if from_source { (op.src_len, op.dest_len) } else { (op.dest_len, op.src_len) };
    if offset < op.offset {
        TextRange::at(offset)
    } else if offset == op.offset {
        if src_len == 0 { TextRange::new(op.offset, op.offset + dest_len) } else { TextRange::at(op.offset) }
    } else if offset < op.offset + src_len {
        if dest_len == 0 { TextRange::at(op.offset) } else { TextRange::new(op.offset, op.offset + dest_len) }
    } else {
        TextRange::at(offset - src_len + dest_len)
    }
}
