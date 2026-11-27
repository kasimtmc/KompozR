//! Android 14's stylus handwriting gestures (Compose's HandwritingGesture.android.kt): what an input method's
//! gesture over the focused field does — select or delete what an area takes (by characters or whole words), the
//! range between two areas, join two words or split one at a point, insert text at a point, remove the spaces along
//! a line —, and the ranges its previews highlight. Hit-tested against the field's text layout (the session's, the
//! platform's own copy: each character's and line's box in the view); what it does is said in the field's UTF-16
//! offsets, for KompozIme to make through its input connection (so that it comes back to the app as any edit of
//! the input method's does).
//!
//! Plain Rust over the layout: tested on every host.

use kompoz_graphics::{Offset, Rect};
use kompoz_text::TextRange;
use kompoz_ui::platform_text_layout::{PlatformTextLayout, TextGranularity};
use kompoz_ui::text_input::TextHighlightType;

/// `InputConnection.HANDWRITING_GESTURE_RESULT_*`.
pub mod result {
    pub const SUCCESS: i32 = 1;
    pub const UNSUPPORTED: i32 = 2;
    pub const FAILED: i32 = 3;
    pub const CANCELLED: i32 = 4;
    pub const FALLBACK: i32 = 5;
}

/// The gestures' kinds as KompozIme encodes them.
pub mod kind {
    pub const SELECT: i32 = 1;
    pub const DELETE: i32 = 2;
    pub const SELECT_RANGE: i32 = 3;
    pub const DELETE_RANGE: i32 = 4;
    pub const JOIN_OR_SPLIT: i32 = 5;
    pub const INSERT: i32 = 6;
    pub const REMOVE_SPACE: i32 = 7;
    /// A preview's cancellation (its highlight taken away).
    pub const CANCEL_PREVIEW: i32 = 8;
}

/// An input method's gesture, px in the view (`HandwritingGesture`'s subclasses).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Gesture {
    Select { area: Rect, granularity: TextGranularity },
    Delete { area: Rect, granularity: TextGranularity },
    SelectRange { start: Rect, end: Rect, granularity: TextGranularity },
    DeleteRange { start: Rect, end: Rect, granularity: TextGranularity },
    JoinOrSplit { point: Offset },
    Insert { point: Offset },
    RemoveSpace { start: Offset, end: Offset },
}

impl Gesture {
    /// The gesture KompozIme encoded: its kind, then its areas' `left, top, right, bottom` (or its points' `x, y`)
    /// in the view, then its granularity (`HandwritingGesture.GRANULARITY_WORD` = 1, `_CHARACTER` = 2).
    pub fn decode(kind: i32, values: &[f32]) -> Option<Gesture> {
        let v = |i: usize| values.get(i).copied();
        let rect = |i: usize| Some(Rect::new(v(i)?, v(i + 1)?, v(i + 2)?, v(i + 3)?));
        let point = |i: usize| Some(Offset::new(v(i)?, v(i + 1)?));
        let granularity = |i: usize| match v(i) {
            Some(g) if g as i32 == 1 => TextGranularity::Word,
            _ => TextGranularity::Character,
        };
        Some(match kind {
            kind::SELECT => Gesture::Select { area: rect(0)?, granularity: granularity(4) },
            kind::DELETE => Gesture::Delete { area: rect(0)?, granularity: granularity(4) },
            kind::SELECT_RANGE => Gesture::SelectRange { start: rect(0)?, end: rect(4)?, granularity: granularity(8) },
            kind::DELETE_RANGE => Gesture::DeleteRange { start: rect(0)?, end: rect(4)?, granularity: granularity(8) },
            kind::JOIN_OR_SPLIT => Gesture::JoinOrSplit { point: point(0)? },
            kind::INSERT => Gesture::Insert { point: point(0)? },
            kind::REMOVE_SPACE => Gesture::RemoveSpace { start: point(0)?, end: point(2)? },
            _ => return None,
        })
    }
}

/// What a gesture does to the field, UTF-16 offsets of its text (KompozIme makes it through its connection).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// The selection set.
    Select(usize, usize),
    /// The text between deleted, the caret left there.
    Delete(usize, usize),
    /// The gesture's own text inserted there, the caret after it.
    Insert(usize),
    /// A space inserted there (a split), the caret after it.
    InsertSpace(usize),
    /// The spaces between taken out.
    RemoveSpaces(usize, usize),
    /// What it did isn't a text edit: the gesture's fallback text committed where the selection is, if it has one
    /// (else it failed).
    Fallback,
}

impl Outcome {
    /// As KompozIme reads it: `[what, a, b]` (what: 1 select, 2 delete, 3 insert, 4 insert a space, 5 remove the
    /// spaces, 6 fall back).
    pub fn encode(self) -> [i32; 3] {
        let i = |v: usize| v as i32;
        match self {
            Outcome::Select(a, b) => [1, i(a), i(b)],
            Outcome::Delete(a, b) => [2, i(a), i(b)],
            Outcome::Insert(at) => [3, i(at), i(at)],
            Outcome::InsertSpace(at) => [4, i(at), i(at)],
            Outcome::RemoveSpaces(a, b) => [5, i(a), i(b)],
            Outcome::Fallback => [6, 0, 0],
        }
    }
}

fn utf16_of(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].encode_utf16().count()
}

/// Whitespace but line breaks (Compose's `isWhitespaceExceptNewline`).
fn is_space(c: char) -> bool {
    c.is_whitespace() && c != '\n' && c != '\r'
}

/// Punctuation, as Java's `Character.getType` counts it (Compose's `isPunctuation`): Unicode's punctuation
/// categories, approximated by ASCII's and the general punctuation block's.
fn is_punctuation(c: char) -> bool {
    c.is_ascii_punctuation() || ('\u{2010}'..='\u{2027}').contains(&c) || ('\u{3001}'..='\u{3003}').contains(&c)
}

/// What an area takes of the text (Compose's `getRangeForScreenRect`, its ContainsCenter inclusion): none for none.
fn range_for(layout: &PlatformTextLayout, area: Rect, granularity: TextGranularity) -> Option<TextRange> {
    layout.range_for_rect(area, granularity).filter(|r| !r.collapsed())
}

/// A word deletion's range widened to the spaces on one side of it (Compose's `adjustHandwritingDeleteGestureRange`):
/// the spaces before it where what follows is a space or punctuation, else those after it where what precedes is.
pub fn adjust_delete_range(text: &str, range: TextRange) -> TextRange {
    let (mut start, mut end) = (range.min(), range.max());
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    let space_or_mark = |c: Option<char>| c.is_none_or(|c| c.is_whitespace() || is_punctuation(c));
    if before.is_some_and(is_space) && space_or_mark(after) {
        while let Some(c) = text[..start].chars().next_back().filter(|&c| is_space(c)) {
            start -= c.len_utf8();
        }
    } else if after.is_some_and(is_space) && space_or_mark(before) {
        while let Some(c) = text[end..].chars().next().filter(|&c| is_space(c)) {
            end += c.len_utf8();
        }
    }
    TextRange::new(start, end)
}

/// The byte offset a point puts a caret at for a gesture (Compose's `getOffsetForHandwritingGesture`): on the line
/// at its height, if it is within `margin` px of that line and of the text's sides (none elsewhere), at the line's
/// middle.
fn offset_for(layout: &PlatformTextLayout, point: Offset, margin: f32) -> Option<usize> {
    let line = layout.line_for_vertical_position(point.y)?;
    let l = layout.lines[line].rect;
    let width = layout.inner_text_field_bounds;
    if point.y < l.top - margin || point.y > l.bottom + margin {
        return None;
    }
    if point.x < width.left.min(l.left) - margin || point.x > width.right.max(l.right) + margin {
        return None;
    }
    Some(layout.offset_for_position(Offset::new(point.x, (l.top + l.bottom) / 2.0)))
}

/// What `gesture` does to the field holding `text`, laid out as `layout` (`margin`: ViewConfiguration's handwriting
/// gesture line margin, px): Compose's `performHandwritingGesture`. A layout of another text (not laid out since it
/// changed) can't be hit-tested: the fallback.
pub fn perform(text: &str, layout: &PlatformTextLayout, gesture: Gesture, margin: f32) -> Outcome {
    if layout.text != text {
        return Outcome::Fallback;
    }
    let u = |byte: usize| utf16_of(text, byte);
    match gesture {
        Gesture::Select { area, granularity } => match range_for(layout, area, granularity) {
            Some(r) => Outcome::Select(u(r.min()), u(r.max())),
            None => Outcome::Fallback,
        },
        Gesture::Delete { area, granularity } => match range_for(layout, area, granularity) {
            Some(r) => {
                let r = if granularity == TextGranularity::Word { adjust_delete_range(text, r) } else { r };
                Outcome::Delete(u(r.min()), u(r.max()))
            }
            None => Outcome::Fallback,
        },
        Gesture::SelectRange { start, end, granularity } | Gesture::DeleteRange { start, end, granularity } => {
            let (Some(a), Some(b)) = (range_for(layout, start, granularity), range_for(layout, end, granularity))
            else {
                return Outcome::Fallback;
            };
            let r = TextRange::new(a.min().min(b.min()), a.max().max(b.max()));
            if matches!(gesture, Gesture::SelectRange { .. }) {
                Outcome::Select(u(r.min()), u(r.max()))
            } else {
                let r = if granularity == TextGranularity::Word { adjust_delete_range(text, r) } else { r };
                Outcome::Delete(u(r.min()), u(r.max()))
            }
        }
        Gesture::JoinOrSplit { point } => {
            let Some(at) = offset_for(layout, point, margin) else { return Outcome::Fallback };
            // (The spaces around it: joined by taking them out; none: split by one.)
            let mut start = at;
            while let Some(c) = text[..start].chars().next_back().filter(|&c| is_space(c)) {
                start -= c.len_utf8();
            }
            let mut end = at;
            while let Some(c) = text[end..].chars().next().filter(|&c| is_space(c)) {
                end += c.len_utf8();
            }
            if start == end { Outcome::InsertSpace(u(at)) } else { Outcome::Delete(u(start), u(end)) }
        }
        Gesture::Insert { point } => match offset_for(layout, point, margin) {
            Some(at) => Outcome::Insert(u(at)),
            None => Outcome::Fallback,
        },
        Gesture::RemoveSpace { start, end } => {
            // (Along the line the start is on: what lies between the two points there.)
            let Some(line) = layout.line_for_vertical_position(start.y) else { return Outcome::Fallback };
            let l = layout.lines[line].rect;
            let area = Rect::new(start.x.min(end.x), l.top, start.x.max(end.x), l.bottom);
            let Some(r) = range_for(layout, area, TextGranularity::Character) else { return Outcome::Fallback };
            if !text[r.min()..r.max()].chars().any(char::is_whitespace) {
                return Outcome::Fallback;
            }
            Outcome::RemoveSpaces(u(r.min()), u(r.max()))
        }
    }
}

/// The range (bytes) a previewable gesture's preview highlights, and how (Compose's `previewHandwritingGesture`:
/// select and delete, of an area or a range of two), none if it takes nothing (or isn't previewable).
pub fn preview(text: &str, layout: &PlatformTextLayout, gesture: Gesture) -> Option<(TextHighlightType, TextRange)> {
    if layout.text != text {
        return None;
    }
    match gesture {
        Gesture::Select { area, granularity } => {
            range_for(layout, area, granularity).map(|r| (TextHighlightType::HandwritingSelectPreview, r))
        }
        Gesture::Delete { area, granularity } => {
            let r = range_for(layout, area, granularity)?;
            let r = if granularity == TextGranularity::Word { adjust_delete_range(text, r) } else { r };
            Some((TextHighlightType::HandwritingDeletePreview, r))
        }
        Gesture::SelectRange { start, end, granularity } | Gesture::DeleteRange { start, end, granularity } => {
            let (a, b) = (range_for(layout, start, granularity)?, range_for(layout, end, granularity)?);
            let r = TextRange::new(a.min().min(b.min()), a.max().max(b.max()));
            if matches!(gesture, Gesture::SelectRange { .. }) {
                Some((TextHighlightType::HandwritingSelectPreview, r))
            } else {
                let r = if granularity == TextGranularity::Word { adjust_delete_range(text, r) } else { r };
                Some((TextHighlightType::HandwritingDeletePreview, r))
            }
        }
        _ => None,
    }
}

/// The CursorAnchorInfo of the field (Compose's CursorAnchorInfoBuilder), as KompozIme reads it: per UTF-16 unit
/// of `text` its box (`left, top, right, bottom`) and flags (`FLAG_HAS_VISIBLE_REGION` 1, `_HAS_INVISIBLE_REGION`
/// 2, `_IS_RTL` 4: a box partly out of what shows of the field has both), the visible lines' boxes (those
/// overlapping what shows), the insertion marker at the caret (`x, top, baseline, bottom`, its flags), the editor's
/// bounds (the decoration box) — px in the view. None if the layout isn't of `text`.
pub struct AnchorInfo {
    pub characters: Vec<f32>,
    pub character_flags: Vec<i32>,
    pub lines: Vec<f32>,
    pub marker: [f32; 4],
    pub marker_flags: i32,
    pub editor: [f32; 4],
}

pub fn anchor_info(text: &str, caret: usize, layout: &PlatformTextLayout) -> Option<AnchorInfo> {
    if layout.text != text {
        return None;
    }
    let visible = layout.inner_text_field_bounds;
    let flags_of = |r: Rect, rtl: bool| {
        let inside = r.left >= visible.left - 0.5
            && r.right <= visible.right + 0.5
            && r.top >= visible.top - 0.5
            && r.bottom <= visible.bottom + 0.5;
        let overlaps =
            r.right > visible.left && r.left < visible.right && r.bottom > visible.top && r.top < visible.bottom;
        let mut flags = if overlaps { 1 } else { 0 };
        if !inside {
            flags |= 2;
        }
        if rtl {
            flags |= 4;
        }
        flags
    };
    let (mut characters, mut character_flags) = (Vec::new(), Vec::new());
    for (c, ch) in layout.characters.iter().zip(text.chars()) {
        for _ in 0..ch.len_utf16() {
            let r = c.rect;
            characters.extend([r.left, r.top, r.right, r.bottom]);
            character_flags.push(flags_of(r, c.rtl));
        }
    }
    let lines = layout
        .lines
        .iter()
        .map(|l| l.rect.intersect(visible))
        .filter(|r| r.width() > 0.0 && r.height() > 0.0)
        .flat_map(|r| [r.left, r.top, r.right, r.bottom])
        .collect();
    let at = caret.min(text.len());
    let cursor = layout.cursor_rect(at);
    let baseline = layout.lines.get(layout.line_for_offset(at)).map_or(cursor.bottom, |l| l.baseline);
    let rtl = layout.characters.get(layout.character_index(at).unwrap_or(usize::MAX)).is_some_and(|c| c.rtl);
    let d = layout.decoration_box_bounds;
    Some(AnchorInfo {
        characters,
        character_flags,
        lines,
        marker: [cursor.left, cursor.top, baseline, cursor.bottom],
        marker_flags: flags_of(cursor, rtl),
        editor: [d.left, d.top, d.right, d.bottom],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::text::test_layout;

    /// "ağaç dalı kuş" 8 px a character from 10, 20, wrapped after "ağaç " and "dalı " (20 px lines).
    fn laid_out() -> (&'static str, PlatformTextLayout) {
        let text = "ağaç dalı kuş";
        (text, test_layout(text, Offset::new(10.0, 20.0), 8.0, 20.0, 5))
    }

    fn rect(l: f32, t: f32, r: f32, b: f32) -> Rect {
        Rect::new(l, t, r, b)
    }

    #[test]
    fn gestures_decode_from_what_kompoz_ime_sends() {
        let select = Gesture::decode(kind::SELECT, &[1.0, 2.0, 3.0, 4.0, 1.0]);
        assert_eq!(
            select,
            Some(Gesture::Select { area: rect(1.0, 2.0, 3.0, 4.0), granularity: TextGranularity::Word })
        );
        let range = Gesture::decode(kind::DELETE_RANGE, &[0.0, 0.0, 1.0, 1.0, 5.0, 5.0, 6.0, 6.0, 2.0]);
        assert!(matches!(range, Some(Gesture::DeleteRange { granularity: TextGranularity::Character, .. })));
        assert_eq!(Gesture::decode(kind::INSERT, &[7.0, 8.0]), Some(Gesture::Insert { point: Offset::new(7.0, 8.0) }));
        assert_eq!(Gesture::decode(kind::REMOVE_SPACE, &[1.0]), None);
        assert_eq!(Gesture::decode(99, &[]), None);
        assert_eq!(Outcome::Delete(2, 5).encode(), [2, 2, 5]);
    }

    #[test]
    fn select_and_delete_take_what_their_area_covers() {
        let (text, layout) = laid_out();
        // Over "ğa" (by characters): selected, in UTF-16 units.
        let over = rect(17.0, 22.0, 35.0, 38.0);
        let select = Gesture::Select { area: over, granularity: TextGranularity::Character };
        assert_eq!(perform(text, &layout, select, 10.0), Outcome::Select(1, 3));
        // By words: the whole of "ağaç" (its box's middle under the area), deleted with the space after it.
        let word = rect(20.0, 22.0, 40.0, 38.0);
        let delete = Gesture::Delete { area: word, granularity: TextGranularity::Word };
        assert_eq!(perform(text, &layout, delete, 10.0), Outcome::Delete(0, 5));
        assert_eq!(
            preview(text, &layout, delete),
            Some((TextHighlightType::HandwritingDeletePreview, TextRange::new(0, 7)))
        );
        // Over nothing: the fallback.
        let nothing = Gesture::Select { area: rect(300.0, 0.0, 310.0, 5.0), granularity: TextGranularity::Character };
        assert_eq!(perform(text, &layout, nothing, 10.0), Outcome::Fallback);
        assert_eq!(preview(text, &layout, nothing), None);
        // From "ç" to "a" of "dalı" (two lines): the range between.
        let range = Gesture::SelectRange {
            start: rect(34.0, 22.0, 42.0, 38.0),
            end: rect(18.0, 42.0, 26.0, 58.0),
            granularity: TextGranularity::Character,
        };
        assert_eq!(perform(text, &layout, range, 10.0), Outcome::Select(3, 7));
        assert_eq!(
            preview(text, &layout, range),
            Some((TextHighlightType::HandwritingSelectPreview, TextRange::new(4, 9)))
        );
        // A layout of another text: the fallback.
        assert_eq!(perform("başka", &layout, select, 10.0), Outcome::Fallback);
    }

    #[test]
    fn join_split_insert_and_remove_spaces_at_their_points() {
        let (text, layout) = laid_out();
        // Between "ağaç" and its space: joined (the space taken out).
        assert_eq!(
            perform(text, &layout, Gesture::JoinOrSplit { point: Offset::new(43.0, 30.0) }, 10.0),
            Outcome::Delete(4, 5)
        );
        // Inside "dalı": split there.
        assert_eq!(
            perform(text, &layout, Gesture::JoinOrSplit { point: Offset::new(27.0, 50.0) }, 10.0),
            Outcome::InsertSpace(7)
        );
        // Inserted before "k"; a point far below every line: the fallback.
        assert_eq!(
            perform(text, &layout, Gesture::Insert { point: Offset::new(11.0, 70.0) }, 10.0),
            Outcome::Insert(10)
        );
        assert_eq!(
            perform(text, &layout, Gesture::Insert { point: Offset::new(11.0, 300.0) }, 10.0),
            Outcome::Fallback
        );
        // Along the first line from "ç" past its end: its space removed; along "dal" (no space): the fallback.
        let remove = Gesture::RemoveSpace { start: Offset::new(36.0, 30.0), end: Offset::new(52.0, 30.0) };
        assert_eq!(perform(text, &layout, remove, 10.0), Outcome::RemoveSpaces(3, 5));
        let none = Gesture::RemoveSpace { start: Offset::new(11.0, 50.0), end: Offset::new(30.0, 50.0) };
        assert_eq!(perform(text, &layout, none, 10.0), Outcome::Fallback);
    }

    #[test]
    fn deletions_take_the_spaces_on_one_side() {
        let r = |a, b| TextRange::new(a, b);
        assert_eq!(adjust_delete_range("bir iki üç", r(4, 7)), r(3, 7)); // (the space before: punctuation-free after)
        assert_eq!(adjust_delete_range("bir iki", r(0, 3)), r(0, 4)); // (the start: the space after)
        assert_eq!(adjust_delete_range("bir, iki", r(5, 8)), r(4, 8));
        assert_eq!(adjust_delete_range("biriki", r(0, 3)), r(0, 3));
    }

    #[test]
    fn the_cursor_anchor_has_every_unit_its_lines_and_the_caret() {
        let text = "a😀b";
        let mut layout = test_layout(text, Offset::new(10.0, 20.0), 8.0, 20.0, 100);
        layout.inner_text_field_bounds = rect(10.0, 20.0, 22.0, 40.0);
        let info = anchor_info(text, 1, &layout).expect("laid out for the text");
        // (The emoji's two units have its box; "b" is out of sight, the emoji partly.)
        assert_eq!(info.characters.len(), 16);
        assert_eq!(&info.characters[4..8], &info.characters[8..12]);
        assert_eq!(info.character_flags, [1, 3, 3, 2]);
        assert_eq!(info.lines, [10.0, 20.0, 22.0, 40.0]);
        assert_eq!(info.marker, [18.0, 20.0, 36.0, 40.0]);
        assert_eq!(info.marker_flags, 1);
        assert!(anchor_info("başka", 0, &layout).is_none());
    }
}
