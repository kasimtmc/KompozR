//! A document's text and its positions: LSP's (a line and a column in UTF-16 code units, the encoding the server
//! negotiates with the editor and with rust-analyzer), byte offsets, and the view of the text rustc reads (no byte
//! order mark, LF line ends) that the Kotlin front end's offsets are into.

use crate::json::Json;

/// Where each line of a text starts (byte offsets).
#[derive(Clone, Debug, Default)]
pub struct Lines {
    starts: Vec<usize>,
}

impl Lines {
    pub fn new(text: &str) -> Lines {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Lines { starts }
    }

    pub fn count(&self) -> usize {
        self.starts.len()
    }

    /// The line (0-based) holding `offset`.
    pub fn line_of(&self, offset: usize) -> usize {
        self.starts.partition_point(|s| *s <= offset).saturating_sub(1)
    }

    /// The byte offset where `line` starts (the text's end past its last line).
    pub fn start(&self, line: usize, text: &str) -> usize {
        self.starts.get(line).copied().unwrap_or(text.len())
    }

    /// The byte offset of an LSP position (a column past the line's end is its end; a line past the text, its end).
    pub fn offset(&self, text: &str, line: usize, utf16: usize) -> usize {
        let Some(&start) = self.starts.get(line) else { return text.len() };
        let mut units = 0;
        for (i, c) in text[start..].char_indices() {
            if c == '\n' || units >= utf16 {
                return start + i;
            }
            units += c.len_utf16();
        }
        text.len()
    }

    /// The LSP position (line, UTF-16 column) of a byte offset.
    pub fn position(&self, text: &str, offset: usize) -> (usize, usize) {
        let offset = floor_char(text, offset.min(text.len()));
        let line = self.line_of(offset);
        let start = self.starts[line];
        (line, text[start..offset].encode_utf16().count())
    }

    /// An LSP position object of a byte offset.
    pub fn json(&self, text: &str, offset: usize) -> Json {
        let (line, character) = self.position(text, offset);
        Json::object([("line", line.into()), ("character", character.into())])
    }

    /// An LSP range object of a byte range.
    pub fn range(&self, text: &str, from: usize, to: usize) -> Json {
        Json::object([("start", self.json(text, from)), ("end", self.json(text, to))])
    }

    /// The byte offset of an LSP position object.
    pub fn offset_of(&self, text: &str, position: &Json) -> usize {
        self.offset(text, position["line"].int().max(0) as usize, position["character"].int().max(0) as usize)
    }
}

/// The largest char boundary at or before `i`.
pub fn floor_char(text: &str, mut i: usize) -> usize {
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// A text as rustc reads it (no byte order mark, `\r\n` as `\n`) and the way back to the text's own offsets: what
/// a `kompoz!` block's or a `.kt` file's Kotlin offsets are into.
#[derive(Clone, Debug, Default)]
pub struct Normalized {
    pub text: String,
    /// The bytes left out, by their offset in the original text, in order.
    removed: Vec<usize>,
}

impl Normalized {
    pub fn new(original: &str) -> Normalized {
        if !original.starts_with('\u{feff}') && !original.contains('\r') {
            return Normalized { text: original.to_string(), removed: Vec::new() };
        }
        let mut text = String::with_capacity(original.len());
        let mut removed = Vec::new();
        let body = match original.strip_prefix('\u{feff}') {
            Some(rest) => {
                removed.extend(0..3);
                rest
            }
            None => original,
        };
        let base = original.len() - body.len();
        let mut from = 0;
        for (i, _) in body.match_indices("\r\n") {
            text.push_str(&body[from..i]);
            removed.push(base + i);
            from = i + 1;
        }
        text.push_str(&body[from..]);
        Normalized { text, removed }
    }

    /// The original text's offset of an offset of the normalized one.
    pub fn to_original(&self, offset: usize) -> usize {
        let mut o = offset;
        for &r in &self.removed {
            if r <= o {
                o += 1;
            } else {
                break;
            }
        }
        o
    }

    /// The normalized text's offset of an offset of the original (a removed byte: the next one's).
    pub fn to_normalized(&self, offset: usize) -> usize {
        offset - self.removed.iter().take_while(|r| **r < offset).count()
    }
}

/// Applies an LSP content change (`{range, text}`, or `{text}`: the whole text) to `text`.
pub fn apply_change(text: &mut String, change: &Json) {
    let new = change["text"].str();
    if !change.has("range") {
        *text = new.to_string();
        return;
    }
    let lines = Lines::new(text);
    let from = lines.offset_of(text, &change["range"]["start"]);
    let to = lines.offset_of(text, &change["range"]["end"]).max(from);
    text.replace_range(from..to, new);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_in_utf16() {
        let text = "ab\nğ😀x\n";
        let lines = Lines::new(text);
        assert_eq!(lines.offset(text, 1, 0), 3);
        assert_eq!(lines.offset(text, 1, 1), 5);
        assert_eq!(lines.offset(text, 1, 3), 9);
        assert_eq!(lines.offset(text, 1, 99), 10);
        assert_eq!(lines.position(text, 9), (1, 3));
        assert_eq!(lines.position(text, 11), (2, 0));
        let mut t = text.to_string();
        let change =
            Json::parse(r#"{"range":{"start":{"line":1,"character":1},"end":{"line":1,"character":3}},"text":"Z"}"#)
                .unwrap();
        apply_change(&mut t, &change);
        assert_eq!(t, "ab\nğZx\n");
    }

    #[test]
    fn normalized_and_back() {
        let original = "\u{feff}a\r\nb\r\nc";
        let n = Normalized::new(original);
        assert_eq!(n.text, "a\nb\nc");
        for (norm, orig) in [(0, 3), (1, 5), (2, 6), (3, 8), (4, 9)] {
            assert_eq!(n.to_original(norm), orig, "{norm}");
            assert_eq!(n.to_normalized(orig), norm, "{orig}");
        }
    }
}
