//! A text's pieces as screen readers ask for them: by code points (AT-SPI's offsets; UI Automation's and Android's
//! are UTF-16 units: [`utf16_len`] converts), a character, a word or the whole line around an offset, and a node's
//! text as it is said (a password's hidden); where they are, from the node's text layout ([`layout_for`]: each
//! character's box, each line's, hit tests — Compose's `GetTextLayoutResult`).

use kompoz_graphics::{Offset, Rect};
use kompoz_ui::platform_text_layout::PlatformTextLayout;
use kompoz_ui::semantics::AccessibilityNode;

/// A text field's text as it is said: a password's hidden (a bullet a character).
pub fn text_of(n: &AccessibilityNode) -> String {
    if n.password { "\u{2022}".repeat(n.value.chars().count()) } else { n.value.clone() }
}

/// A UTF-8 string's code points (the offsets a text is read by): the byte each starts at, then its end.
pub fn code_points(s: &str) -> Vec<usize> {
    s.char_indices().map(|(i, _)| i).chain(std::iter::once(s.len())).collect()
}

/// Code points `from` to `to` (-1: the end) of `s`, clamped.
pub fn slice<'a>(s: &'a str, at: &[usize], from: i32, to: i32) -> &'a str {
    let n = at.len() as i32 - 1;
    let from = from.clamp(0, n);
    let to = if to < 0 { n } else { to.clamp(from, n) };
    &s[at[from as usize]..at[to as usize]]
}

/// How many UTF-16 units the first `code_points` code points of `s` are (UI Automation's and Android's offsets).
pub fn utf16_len(s: &str, code_points: usize) -> usize {
    s.chars().take(code_points).map(char::len_utf16).sum()
}

/// The piece of a text an offset is in: a character, a word (with the spaces after it), or the whole (a field's one
/// line).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unit {
    Char,
    Word,
    Line,
}

/// The `unit` around code point `offset` of `s`: [start, end) in code points.
pub fn segment(s: &str, at: &[usize], offset: i32, unit: Unit) -> (i32, i32) {
    let n = at.len() as i32 - 1;
    if offset < 0 || offset >= n {
        let o = offset.clamp(0, n);
        return (o, o);
    }
    match unit {
        Unit::Char => return (offset, offset + 1),
        Unit::Line => return (0, n),
        Unit::Word => {}
    }
    // (C's isspace on a code point's first byte, as C++'s.)
    let space = |i: i32| matches!(s.as_bytes()[at[i as usize]], b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r');
    let (mut start, mut end) = (offset, offset);
    while start > 0 && space(start) {
        start -= 1; // in the spaces after a word: that word's
    }
    while start > 0 && !space(start - 1) {
        start -= 1;
    }
    while end < n && !space(end) {
        end += 1;
    }
    while end < n && space(end) {
        end += 1;
    }
    (start, end)
}

/// The text layout of `n`'s text as a bridge reads it (`text`: [`text_of`]'s for a field), where it lays out as many
/// characters as `text` has — its character `i` is `text`'s code point `i` (a password's bullet for what was typed).
/// None where it has none (the bridge falls back on the node's box).
pub fn layout_for<'a>(n: &'a AccessibilityNode, text: &str) -> Option<&'a PlatformTextLayout> {
    n.text_layout.as_deref().filter(|l| l.characters.len() == text.chars().count())
}

/// The byte of `layout`'s text code point `offset` starts at (its end past the last).
fn byte_of(layout: &PlatformTextLayout, offset: i32) -> usize {
    usize::try_from(offset).ok().and_then(|i| layout.characters.get(i)).map_or(layout.text.len(), |c| c.start)
}

/// The code point byte `at` of `layout`'s text is in.
fn code_point_of(layout: &PlatformTextLayout, at: usize) -> i32 {
    layout.characters.partition_point(|c| c.end <= at) as i32
}

/// Where code point `offset` is, px in the window: its character's box; at the text's end, the caret's there (no
/// wide). None past the end.
pub fn character_box(layout: &PlatformTextLayout, offset: i32) -> Option<Rect> {
    let n = layout.characters.len() as i32;
    match offset {
        o if (0..n).contains(&o) => Some(layout.characters[o as usize].rect),
        o if o == n => Some(layout.end_cursor),
        _ => None,
    }
}

/// The boxes code points `from..to` take, px in the window: one per line, top to bottom (UI Automation's
/// GetBoundingRectangles); none for an empty range.
pub fn range_boxes(layout: &PlatformTextLayout, from: i32, to: i32) -> Vec<Rect> {
    layout.range_rects(byte_of(layout, from.min(to)), byte_of(layout, from.max(to)))
}

/// The box code points `from..to` take, px in the window (their characters' together); the caret's for an empty
/// range.
pub fn range_box(layout: &PlatformTextLayout, from: i32, to: i32) -> Rect {
    layout.range_bounds(byte_of(layout, from.min(to)), byte_of(layout, from.max(to)))
}

/// The code point whose character's box holds `point` (px in the window), if one does (AT-SPI's
/// GetOffsetAtPoint).
pub fn offset_at_point(layout: &PlatformTextLayout, point: Offset) -> Option<i32> {
    layout.character_at(point).map(|i| i as i32)
}

/// The code point a caret goes before for `point` (px in the window; the nearest on the line at its height: UI
/// Automation's RangeFromPoint, Compose's getOffsetForPosition).
pub fn caret_for_point(layout: &PlatformTextLayout, point: Offset) -> i32 {
    code_point_of(layout, layout.offset_for_position(point))
}

/// The line around code point `offset` as the text is laid out (its soft wraps too): [start, end) in code points,
/// the end with what ends it.
pub fn line_around(layout: &PlatformTextLayout, offset: i32) -> (i32, i32) {
    let line = layout.line_for_offset(byte_of(layout, offset.max(0)));
    match layout.lines.get(line) {
        Some(l) => (code_point_of(layout, l.start), code_point_of(layout, l.end)),
        None => (0, layout.characters.len() as i32),
    }
}

/// Each line of `layout`'s text as it is laid out (soft wraps too): [start, end) in code points, top to bottom.
pub fn lines_of(layout: &PlatformTextLayout) -> Vec<(i32, i32)> {
    layout.lines.iter().map(|l| (code_point_of(layout, l.start), code_point_of(layout, l.end))).collect()
}

/// What was typed or taken away from `before` to `after`: what lies between the same start and the same end, as
/// (start, what went, what came), in code points; none if nothing changed.
pub fn edit(before: &str, after: &str) -> Option<(usize, String, String)> {
    let (a, b) = (code_points(before), code_points(after));
    let (na, nb) = (a.len() as i32 - 1, b.len() as i32 - 1);
    let same = |i: i32, j: i32| slice(before, &a, i, i + 1) == slice(after, &b, j, j + 1);
    let mut head = 0;
    while head < na && head < nb && same(head, head) {
        head += 1;
    }
    let mut tail = 0;
    while tail < na - head && tail < nb - head && same(na - 1 - tail, nb - 1 - tail) {
        tail += 1;
    }
    if na - head - tail == 0 && nb - head - tail == 0 {
        return None;
    }
    let gone = slice(before, &a, head, na - tail).to_string();
    let came = slice(after, &b, head, nb - tail).to_string();
    Some((head as usize, gone, came))
}

/// A text laid out for the platforms' tests: `advance` px a character from `origin`, `line_height` px lines, a new
/// line after each `wrap` characters (and after a line break), left to right.
#[cfg(test)]
pub(crate) fn test_layout(
    text: &str,
    origin: Offset,
    advance: f32,
    line_height: f32,
    wrap: usize,
) -> PlatformTextLayout {
    use kompoz_ui::platform_text_layout::{CharacterBounds, LineBounds};
    let (mut characters, mut lines) = (Vec::new(), Vec::new());
    let (mut line, mut column, mut line_start) = (0, 0, 0);
    let line_box = |line: usize, columns: usize| {
        let top = origin.y + line_height * line as f32;
        Rect::new(origin.x, top, origin.x + advance * columns as f32, top + line_height)
    };
    for (start, c) in text.char_indices() {
        let end = start + c.len_utf8();
        let (x, y) = (origin.x + advance * column as f32, origin.y + line_height * line as f32);
        characters.push(CharacterBounds {
            start,
            end,
            rect: Rect::new(x, y, x + advance, y + line_height),
            rtl: false,
            line,
        });
        column += 1;
        if c == '\n' || column == wrap {
            let rect = line_box(line, column);
            lines.push(LineBounds { start: line_start, end, rect, baseline: rect.bottom - 4.0, rtl: false });
            (line, column, line_start) = (line + 1, 0, end);
        }
    }
    let rect = line_box(line, column);
    lines.push(LineBounds { start: line_start, end: text.len(), rect, baseline: rect.bottom - 4.0, rtl: false });
    let end_cursor = Rect::new(rect.right, rect.top, rect.right, rect.bottom);
    let all = lines.iter().fold(lines[0].rect, |a, l| a.union(l.rect));
    PlatformTextLayout {
        text: text.to_string(),
        characters,
        lines,
        end_cursor,
        inner_text_field_bounds: all,
        decoration_box_bounds: all,
        transform: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn where_a_texts_characters_are_by_code_points() {
        // "héllo wörld" wrapped after "héllo ", 8 px a character from 10, 100.
        let layout = test_layout("héllo wörld", Offset::new(10.0, 100.0), 8.0, 20.0, 6);
        let n =
            AccessibilityNode { value: "héllo wörld".into(), text_layout: Some(layout.into()), ..Default::default() };
        let layout = layout_for(&n, &text_of(&n)).expect("as many characters");
        assert!(layout_for(&n, "kısa").is_none());
        assert_eq!(character_box(layout, 1), Some(Rect::new(18.0, 100.0, 26.0, 120.0)));
        assert_eq!(character_box(layout, 11), Some(Rect::new(50.0, 120.0, 50.0, 140.0)));
        assert_eq!(character_box(layout, 12), None);
        assert_eq!(
            range_boxes(layout, 4, 8),
            [Rect::new(42.0, 100.0, 58.0, 120.0), Rect::new(10.0, 120.0, 26.0, 140.0)]
        );
        assert_eq!(range_box(layout, 4, 8), Rect::new(10.0, 100.0, 58.0, 140.0));
        assert_eq!(offset_at_point(layout, Offset::new(20.0, 130.0)), Some(7));
        assert_eq!(offset_at_point(layout, Offset::new(200.0, 130.0)), None);
        assert_eq!(caret_for_point(layout, Offset::new(25.0, 130.0)), 8);
        assert_eq!(line_around(layout, 7), (6, 11));
        assert_eq!(lines_of(layout), [(0, 6), (6, 11)]);
    }

    #[test]
    fn texts_are_cut_by_code_points_and_words() {
        let s = "héllo wörld";
        let at = code_points(s);
        assert_eq!(at.len() - 1, 11);
        assert_eq!(slice(s, &at, 1, 3), "él");
        assert_eq!(slice(s, &at, 6, -1), "wörld");
        assert_eq!(slice(s, &at, -4, 99), s);
        assert_eq!(segment(s, &at, 7, Unit::Word), (6, 11));
        assert_eq!(segment(s, &at, 5, Unit::Word), (0, 6));
        assert_eq!(segment(s, &at, 1, Unit::Char), (1, 2));
        assert_eq!(segment(s, &at, 3, Unit::Line), (0, 11));
        assert_eq!(segment(s, &at, 11, Unit::Word), (11, 11));
        let password = AccessibilityNode { value: "abc".into(), password: true, ..Default::default() };
        assert_eq!(text_of(&password), "•••");
        assert_eq!(utf16_len("a😀b", 2), 3);
    }

    #[test]
    fn edits_are_what_lies_between_the_same_ends() {
        assert_eq!(edit("hello", "help"), Some((3, "lo".into(), "p".into())));
        assert_eq!(edit("ab", "aXb"), Some((1, String::new(), "X".into())));
        assert_eq!(edit("aXXb", "ab"), Some((1, "XX".into(), String::new())));
        assert_eq!(edit("çay", "çay"), None);
        assert_eq!(edit("", "ğ"), Some((0, String::new(), "ğ".into())));
    }
}
