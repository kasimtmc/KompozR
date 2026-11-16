//! What an on-the-spot input method (XIM's `XIMPreeditCallbacks` style) is composing, as its callbacks change it:
//! its start and done (nothing composed), its draws (a stretch of characters replaced, the caret placed) and its
//! caret moves. Pure: the callbacks ([`ffi`](super::ffi)) only apply what they are told here; the platform reads it
//! after (as the core's [`Event::Preedit`](crate::app::Event)).

/// XIM's caret directions (`XIMCaretDirection`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CaretDirection {
    ForwardChar,
    BackwardChar,
    ForwardWord,
    BackwardWord,
    Up,
    Down,
    NextLine,
    PreviousLine,
    LineStart,
    LineEnd,
    AbsolutePosition,
    DontChange,
}

impl CaretDirection {
    /// XIM's value (`XIMForwardChar` 0 … `XIMDontChange` 11); none if it isn't one.
    pub fn of(value: i32) -> Option<CaretDirection> {
        use CaretDirection::*;
        [
            ForwardChar,
            BackwardChar,
            ForwardWord,
            BackwardWord,
            Up,
            Down,
            NextLine,
            PreviousLine,
            LineStart,
            LineEnd,
            AbsolutePosition,
            DontChange,
        ]
        .get(usize::try_from(value).ok()?)
        .copied()
    }
}

/// XIM's feedbacks that mark the part being converted (`XIMReverse`, `XIMHighlight`): what the core highlights.
pub const MARKING_FEEDBACK: std::ffi::c_ulong = 1 | 4;

/// The text being composed, which of its characters are marked (the part being converted), its caret
/// (characters), and whether that changed since the platform last read it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Preedit {
    chars: Vec<char>,
    marked: Vec<bool>,
    caret: usize,
    changed: bool,
}

impl Preedit {
    /// The input method began or ended composing: nothing composed.
    pub fn clear(&mut self) {
        if !self.chars.is_empty() || self.caret != 0 {
            self.changed = true;
        }
        self.chars.clear();
        self.marked.clear();
        self.caret = 0;
    }

    /// A draw: `length` characters from `first` replaced by `chars` (none drawn: deleted), each marked as `marked`
    /// says (missing: not); or, with no `chars`, only those characters' marks changed; the caret at `caret` (each
    /// clamped to the text).
    pub fn draw(&mut self, caret: i32, first: i32, length: i32, chars: Option<&[char]>, marked: &[bool]) {
        let first = (first.max(0) as usize).min(self.chars.len());
        let end = first.saturating_add(length.max(0) as usize).min(self.chars.len());
        let mark = |i: usize| marked.get(i).copied().unwrap_or(false);
        match chars {
            Some(chars) => {
                self.chars.splice(first..end, chars.iter().copied());
                self.marked.splice(first..end, (0..chars.len()).map(mark));
            }
            None => {
                for (i, m) in self.marked[first..end].iter_mut().enumerate() {
                    *m = mark(i);
                }
            }
        }
        self.caret = (caret.max(0) as usize).min(self.chars.len());
        self.changed = true;
    }

    /// A caret move (`position` for an absolute one): where it went, which the input method is told back.
    pub fn move_caret(&mut self, position: i32, direction: CaretDirection) -> usize {
        use CaretDirection::*;
        let len = self.chars.len();
        let at = self.caret;
        let space = |i: usize| self.chars[i].is_whitespace();
        let caret = match direction {
            ForwardChar => (at + 1).min(len),
            BackwardChar => at.saturating_sub(1),
            ForwardWord => {
                let mut i = at;
                while i < len && space(i) {
                    i += 1;
                }
                while i < len && !space(i) {
                    i += 1;
                }
                i
            }
            BackwardWord => {
                let mut i = at;
                while i > 0 && space(i - 1) {
                    i -= 1;
                }
                while i > 0 && !space(i - 1) {
                    i -= 1;
                }
                i
            }
            LineStart => 0,
            LineEnd => len,
            AbsolutePosition => (position.max(0) as usize).min(len),
            // (One line: no other to go up or down to.)
            Up | Down | NextLine | PreviousLine | DontChange => at,
        };
        if caret != self.caret {
            self.caret = caret;
            self.changed = true;
        }
        caret
    }

    /// The composed text (UTF-8) and, in its bytes, its first marked part (the input method converting it), else its
    /// caret (the same at both ends), if it changed since the last time asked.
    pub fn take_change(&mut self) -> Option<(String, (usize, usize))> {
        if !std::mem::take(&mut self.changed) {
            return None;
        }
        let text: String = self.chars.iter().collect();
        let byte = |at: usize| self.chars[..at].iter().map(|c| c.len_utf8()).sum::<usize>();
        let cursor = match self.marked.iter().position(|&m| m) {
            Some(start) => {
                let end = self.marked[start..].iter().position(|&m| !m).map_or(self.chars.len(), |n| start + n);
                (byte(start), byte(end))
            }
            None => (byte(self.caret), byte(self.caret)),
        };
        Some((text, cursor))
    }

    /// The composed text as it is.
    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }
}

/// An XIM text's characters: wide characters (UCS-4 on Linux), or the input method's locale's multibyte text —
/// UTF-8, else (not valid UTF-8: a Latin-1 locale's) a character a byte. At most `length` characters.
pub fn text_chars(wide: Option<&[i32]>, multibyte: Option<&[u8]>, length: usize) -> Vec<char> {
    let chars: Vec<char> = match (wide, multibyte) {
        (Some(wide), _) => wide.iter().filter_map(|&c| char::from_u32(c as u32)).collect(),
        (None, Some(bytes)) => match std::str::from_utf8(bytes) {
            Ok(text) => text.chars().collect(),
            Err(_) => bytes.iter().map(|&b| char::from(b)).collect(),
        },
        (None, None) => Vec::new(),
    };
    chars.into_iter().take(length).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn draws_replace_stretches_and_place_the_caret() {
        let mut p = Preedit::default();
        assert_eq!(p.take_change(), None);
        // Typed in: "ka", the caret after it.
        p.draw(2, 0, 0, Some(&chars("ka")), &[]);
        assert_eq!(p.take_change(), Some(("ka".into(), (2, 2))));
        // Converted: both replaced by "çay", the caret after its first character (bytes: ç is two).
        p.draw(1, 0, 2, Some(&chars("çay")), &[]);
        assert_eq!(p.take_change(), Some(("çay".into(), (2, 2))));
        // A character deleted (no text), the caret clamped; out of range: clamped too.
        p.draw(9, 1, 1, Some(&[]), &[]);
        assert_eq!(p.take_change(), Some(("çy".into(), (3, 3))));
        p.draw(-1, 5, 3, Some(&chars("!")), &[]);
        assert_eq!(p.take_change(), Some(("çy!".into(), (0, 0))));
        // Done: nothing composed (told once).
        p.clear();
        assert_eq!(p.text(), "");
        assert_eq!(p.take_change(), Some((String::new(), (0, 0))));
        p.clear();
        assert_eq!(p.take_change(), None);
    }

    #[test]
    fn marked_parts_are_highlighted() {
        let mut p = Preedit::default();
        // "çay evi" drawn with its first word marked (being converted): that part, in bytes.
        let marks = [true, true, true, false, false, false, false];
        p.draw(7, 0, 0, Some(&chars("çay evi")), &marks);
        assert_eq!(p.take_change(), Some(("çay evi".into(), (0, 4))));
        // Only the marks changed (no text): the second word marked now.
        p.draw(7, 0, 7, None, &[false, false, false, false, true, true, true]);
        assert_eq!(p.take_change(), Some(("çay evi".into(), (5, 8))));
        // A marked part replaced by unmarked text: the caret again.
        p.draw(5, 4, 3, Some(&chars("ev")), &[]);
        assert_eq!(p.take_change(), Some(("çay ev".into(), (6, 6))));
    }

    #[test]
    fn the_caret_moves_as_the_input_method_asks() {
        use CaretDirection::*;
        let mut p = Preedit::default();
        p.draw(0, 0, 0, Some(&chars("bir iki  üç")), &[]);
        p.take_change();
        assert_eq!(p.move_caret(0, ForwardChar), 1);
        assert_eq!(p.move_caret(0, ForwardWord), 3);
        assert_eq!(p.move_caret(0, ForwardWord), 7);
        assert_eq!(p.move_caret(0, ForwardWord), 11);
        assert_eq!(p.move_caret(0, ForwardChar), 11);
        assert_eq!(p.move_caret(0, BackwardWord), 9);
        assert_eq!(p.move_caret(0, BackwardWord), 4);
        assert_eq!(p.move_caret(0, BackwardChar), 3);
        assert_eq!(p.move_caret(0, Up), 3);
        assert_eq!(p.move_caret(0, LineEnd), 11);
        assert_eq!(p.move_caret(0, LineStart), 0);
        assert_eq!(p.move_caret(0, BackwardChar), 0);
        assert_eq!(p.move_caret(6, AbsolutePosition), 6);
        assert_eq!(p.move_caret(60, AbsolutePosition), 11);
        assert_eq!(p.move_caret(3, DontChange), 11);
        assert_eq!(p.take_change(), Some(("bir iki  üç".into(), (13, 13))));
        assert_eq!(CaretDirection::of(10), Some(AbsolutePosition));
        assert_eq!(CaretDirection::of(12), None);
        assert_eq!(CaretDirection::of(-1), None);
    }

    #[test]
    fn xim_texts_are_wide_utf8_or_latin1() {
        assert_eq!(text_chars(Some(&[0x15f, 0x61]), None, 9), chars("şa"));
        assert_eq!(text_chars(None, Some("şey".as_bytes()), 9), chars("şey"));
        assert_eq!(text_chars(None, Some("şey".as_bytes()), 2), chars("şe"));
        assert_eq!(text_chars(None, Some(&[0xe7, 0x61]), 9), chars("ça")); // (Latin-1)
        assert_eq!(text_chars(None, None, 9), Vec::<char>::new());
    }
}
