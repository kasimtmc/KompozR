//! The source map: the generated Rust of a Kotlin region printed as text, each of its tokens remembering the Kotlin
//! offset it comes from, so a place rustc (or rust-analyzer) names in the shadow file goes back to a Kotlin place,
//! and a Kotlin place forward to the Rust made of it.
//!
//! The generator gives every token it emits the span of the Kotlin token it comes from ([`kompoz_kotlin::SpanMap`]).
//! Outside a proc macro, spans are proc-macro2's own: [`Markers`] makes one span per Kotlin offset (an identifier
//! `__kzS<offset>` of a text parsed for the purpose, its source text naming the offset back), [`print`] writes the
//! tokens out and reads each one's offset off its span. Tokens made by the generator itself (`call_site`) have none.

use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use std::ops::Range;
use std::str::FromStr;

const MARK: &str = "__kzS";

/// One span per offset of a Kotlin text (its end included).
pub struct Markers {
    spans: Vec<Span>,
}

impl Markers {
    pub fn new(len: usize) -> Markers {
        let mut text = String::with_capacity((len + 1) * (MARK.len() + 7));
        for i in 0..=len {
            text.push_str(MARK);
            text.push_str(&i.to_string());
            text.push(' ');
        }
        let spans =
            TokenStream::from_str(&text).map(|ts| ts.into_iter().map(|t| t.span()).collect()).unwrap_or_default();
        Markers { spans }
    }

    /// The Kotlin offset a span stands for (none: a span not of a marker).
    pub fn offset_of(span: Span) -> Option<usize> {
        span.source_text()?.strip_prefix(MARK)?.parse().ok()
    }
}

impl kompoz_kotlin::SpanMap for Markers {
    fn span_at(&self, offset: usize) -> Span {
        self.spans.get(offset.min(self.spans.len().saturating_sub(1))).copied().unwrap_or_else(Span::call_site)
    }
}

/// Rust printed from tokens, and where each token with a Kotlin origin is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Printed {
    pub text: String,
    /// A token's byte range in `text` and its Kotlin offset, in text order.
    pub marks: Vec<(Range<usize>, usize)>,
}

/// Prints `tokens` (spaces between tokens, none after a joint punctuation; a line break after `;` and braces).
pub fn print(tokens: TokenStream) -> Printed {
    let mut out = Printed::default();
    print_into(tokens, &mut out);
    out
}

fn mark(out: &mut Printed, from: usize, span: Span) {
    if let Some(k) = Markers::offset_of(span) {
        out.marks.push((from..out.text.len(), k));
    }
}

fn space(out: &mut Printed) {
    if !out.text.is_empty() && !out.text.ends_with([' ', '\n']) {
        out.text.push(' ');
    }
}

fn print_into(tokens: TokenStream, out: &mut Printed) {
    let mut joint = false;
    for t in tokens {
        if !joint {
            space(out);
        }
        joint = false;
        let from = out.text.len();
        match t {
            TokenTree::Group(g) => {
                let (open, close) = match g.delimiter() {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::None => ("", ""),
                };
                out.text.push_str(open);
                mark(out, from, g.span_open());
                if g.delimiter() == Delimiter::Brace {
                    out.text.push('\n');
                }
                print_into(g.stream(), out);
                if !close.is_empty() {
                    space(out);
                    let at = out.text.len();
                    out.text.push_str(close);
                    mark(out, at, g.span_close());
                }
                if g.delimiter() == Delimiter::Brace {
                    out.text.push('\n');
                }
            }
            TokenTree::Ident(i) => {
                out.text.push_str(&i.to_string());
                mark(out, from, i.span());
            }
            TokenTree::Punct(p) => {
                out.text.push(p.as_char());
                mark(out, from, p.span());
                joint = p.spacing() == proc_macro2::Spacing::Joint;
                if p.as_char() == ';' && !joint {
                    out.text.push('\n');
                }
            }
            TokenTree::Literal(l) => {
                out.text.push_str(&l.to_string());
                mark(out, from, l.span());
            }
        }
    }
}

/// The length of the Kotlin token at `offset` of `text`: a name or number whole, a string literal to its closing
/// quote on the line, else one character.
pub fn token_len(text: &str, offset: usize) -> usize {
    let rest = text.get(offset..).unwrap_or("");
    let Some(first) = rest.chars().next() else { return 0 };
    if first.is_alphanumeric() || first == '_' {
        return rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
    }
    if first == '"' {
        let mut escaped = false;
        for (i, c) in rest.char_indices().skip(1) {
            match c {
                '\n' => return i,
                '\\' if !escaped => escaped = true,
                '"' if !escaped => return i + 1,
                _ => escaped = false,
            }
        }
        return rest.len();
    }
    first.len_utf8()
}

/// A generated block of a shadow file: where it is, the Kotlin it comes from, its marks.
#[derive(Clone, Debug)]
pub struct Block {
    /// Its byte range in the shadow file.
    pub shadow: Range<usize>,
    /// The origin (an index into the shadow file's origins).
    pub origin: usize,
    /// Where its Kotlin starts in the origin's normalized text.
    pub kotlin_start: usize,
    /// Its marks, shadow ranges absolute (in the shadow file), Kotlin offsets the region's.
    pub marks: Vec<(Range<usize>, usize)>,
    /// The generator found errors in it (its own diagnostics stand; the shadow's are left out).
    pub failed: bool,
}

impl Block {
    /// The Kotlin offset (the region's) a shadow offset of the block comes from: its token's, else the nearest
    /// token's before it.
    fn kotlin_at(&self, shadow: usize) -> Option<usize> {
        let i = self.marks.partition_point(|(r, _)| r.start <= shadow);
        let (_, k) = self.marks.get(i.checked_sub(1)?)?;
        Some(*k)
    }
}

/// A shadow file's map: its generated blocks (in order).
#[derive(Clone, Debug, Default)]
pub struct ShadowMap {
    pub blocks: Vec<Block>,
}

impl ShadowMap {
    /// The origin and its normalized byte range a shadow range comes from (`kotlin`: the origin's normalized text,
    /// by origin index). None outside the generated Rust, or in a block whose Kotlin failed.
    pub fn to_origin<'t>(
        &self,
        range: Range<usize>,
        kotlin: impl Fn(usize) -> &'t str,
    ) -> Option<(usize, Range<usize>)> {
        let b = self.blocks.iter().find(|b| b.shadow.start <= range.start && range.start < b.shadow.end)?;
        if b.failed {
            return None;
        }
        let text = kotlin(b.origin);
        let start = b.kotlin_start + b.kotlin_at(range.start)?;
        let last = range.end.saturating_sub(1).clamp(range.start, b.shadow.end.saturating_sub(1));
        let end = b.kotlin_start + b.kotlin_at(last).unwrap_or(start - b.kotlin_start);
        let end = (end + token_len(text, end)).max(start + token_len(text, start));
        Some((b.origin, start..end.max(start)))
    }

    /// The shadow offset of an origin's normalized offset: in the Rust token made of the Kotlin token there — the
    /// one as long as it (the token itself: the literal, not the conversion around it) before the others, the
    /// first in the shadow.
    pub fn to_shadow(&self, origin: usize, offset: usize, kotlin: &str) -> Option<usize> {
        self.shadow_candidates(origin, offset, kotlin).first().copied()
    }

    /// Every shadow offset made of the Kotlin token at `offset`, best first (see [`ShadowMap::to_shadow`]).
    pub fn shadow_candidates(&self, origin: usize, offset: usize, kotlin: &str) -> Vec<usize> {
        let mut all: Vec<((usize, bool), usize)> = Vec::new();
        for b in self.blocks.iter().filter(|b| b.origin == origin && b.kotlin_start <= offset) {
            let k = offset - b.kotlin_start;
            for (r, at) in &b.marks {
                let len = token_len(kotlin, b.kotlin_start + at).max(1);
                if *at <= k && k < at + len {
                    let d = k - at;
                    all.push(((d, r.len() != len), r.start + d.min(r.len().saturating_sub(1))));
                }
            }
        }
        // (stable: the first in the shadow among equals)
        all.sort_by_key(|(rank, _)| *rank);
        all.into_iter().map(|(_, s)| s).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kompoz_kotlin::{Options, Pos};

    fn translate(kotlin: &str) -> Printed {
        let markers = Markers::new(kotlin.len());
        let t = kompoz_kotlin::translate(kotlin, Pos::start(1, 1), &markers, &Options::default());
        assert!(t.diagnostics.is_empty(), "{:?}", t.diagnostics);
        print(t.tokens)
    }

    #[test]
    fn markers_name_their_offsets() {
        let m = Markers::new(40);
        use kompoz_kotlin::SpanMap;
        for i in [0, 1, 17, 40] {
            assert_eq!(Markers::offset_of(m.span_at(i)), Some(i));
        }
        assert_eq!(Markers::offset_of(m.span_at(400)), Some(40));
        assert_eq!(Markers::offset_of(Span::call_site()), None);
    }

    #[test]
    fn printed_tokens_keep_their_kotlin_offsets() {
        let kotlin = "fun answer(): Int = \"forty-two\"\n";
        let p = translate(kotlin);
        // (the literal: printed as Rust, from the Kotlin literal)
        let lit = p.text.find("\"forty-two\"").unwrap_or_else(|| panic!("{}", p.text));
        let (r, k) = p.marks.iter().find(|(r, _)| r.start == lit).expect("the literal is marked");
        assert_eq!(&p.text[r.clone()], "\"forty-two\"");
        assert_eq!(*k, kotlin.find('"').unwrap());
        // (the function's name)
        let name = p.text.find("answer").unwrap();
        let (_, k) = p.marks.iter().find(|(r, _)| r.start == name).expect("the name is marked");
        assert_eq!(*k, 4);
        // (printed Rust parses back)
        assert!(TokenStream::from_str(&p.text).is_ok());
    }

    #[test]
    fn shadow_ranges_go_back_and_forth() {
        let kotlin = "fun answer(): Int = \"forty-two\"\n";
        let p = translate(kotlin);
        let prefix = "// copied Rust\n";
        let start = prefix.len();
        let map = ShadowMap {
            blocks: vec![Block {
                shadow: start..start + p.text.len(),
                origin: 0,
                kotlin_start: 10,
                marks: p.marks.iter().map(|(r, k)| (r.start + start..r.end + start, *k)).collect(),
                failed: false,
            }],
        };
        let origin = format!("{}{kotlin}", "x".repeat(10));
        let lit = start + p.text.find("\"forty-two\"").unwrap();
        let (o, r) = map.to_origin(lit..lit + 11, |_| origin.as_str()).unwrap();
        assert_eq!((o, &origin[r]), (0, "\"forty-two\""));
        // (a place in the copied Rust: none)
        assert_eq!(map.to_origin(0..2, |_| origin.as_str()), None);
        // (the name, forward and back)
        let at = map.to_shadow(0, 10 + 6, &origin).unwrap();
        let name = start + p.text.find("answer").unwrap();
        assert_eq!(at, name + 2);
        let (_, r) = map.to_origin(at..at + 1, |_| origin.as_str()).unwrap();
        assert_eq!(&origin[r], "answer");
        // (a failed block maps nothing)
        let mut failed = map.clone();
        failed.blocks[0].failed = true;
        assert_eq!(failed.to_origin(lit..lit + 1, |_| origin.as_str()), None);
    }

    #[test]
    fn kotlin_token_lengths() {
        assert_eq!(token_len("abc_1 x", 0), 5);
        assert_eq!(token_len("\"a\\\"b\" x", 0), 6);
        assert_eq!(token_len("+=", 0), 1);
        assert_eq!(token_len("ğx y", 0), 3);
        assert_eq!(token_len("", 0), 0);
    }
}
