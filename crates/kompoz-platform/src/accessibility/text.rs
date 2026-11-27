//! A text's pieces as screen readers ask for them: by code points (AT-SPI's offsets; UI Automation's and Android's
//! are UTF-16 units: [`utf16_len`] converts), a character, a word or the whole line around an offset, and a node's
//! text as it is said (a password's hidden).

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

#[cfg(test)]
mod tests {
    use super::*;

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
