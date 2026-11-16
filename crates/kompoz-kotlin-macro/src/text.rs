//! The Kotlin text of a `kompoz! { … }` and the span of every offset in it.
//!
//! Under rustc every token has its source text and its line and column (characters, 1-based). The text is each
//! top-level token's source text (a group's holds its comments and spacing as written) with the gaps between them
//! rebuilt — newlines, then spaces — so that a position in the text is the file's line:column; it starts at the
//! first token's line, padded to its column (`base` = that line, column 1). Comments between top-level tokens
//! become spaces; a doc comment, which Rust makes `#` + `[doc = "…"]` all carrying the comment's span, is taken
//! once (as its source text).
//!
//! Without source text (rust-analyzer's proc-macro server: none, and every position 1:1), or when the positions
//! don't add up (tokens made by another macro), the text is rebuilt from the tokens themselves:
//! [`TextSource::Rebuilt`]. Spacing follows the tokens (no space after a joint punctuation, around `.`, before an
//! argument list …); newlines, which Kotlin needs between statements, go after a token that can end an expression
//! (a name, a literal, a closing bracket, `!!`) before one that starts another: a name other than an infix
//! function's or a continuing keyword (`else`, `catch`, `in`, …), a literal, an annotation. Doc comments are left
//! out. It is best effort — what an editor sees — while the real build has the exact text.
//!
//! The span map: every leaf token (a group's delimiters too) with its offsets in the text; an offset maps to the
//! token containing it (a Kotlin template split into several Rust literals maps to each piece's), else to the
//! next token, else to the last.

use kompoz_kotlin::{Pos, SpanMap, TextSource};
use proc_macro::{Delimiter, Spacing, Span, TokenStream, TokenTree};

pub(crate) struct Acquired {
    pub text: String,
    pub base: Pos,
    pub source: TextSource,
    pub spans: TokenSpans,
}

/// The macro's input as Kotlin text.
pub(crate) fn acquire(input: TokenStream) -> Acquired {
    let tokens: Vec<TokenTree> = input.into_iter().collect();
    exact(&tokens).unwrap_or_else(|| rebuilt(&tokens))
}

// ---- The span map ----------------------------------------------------------------------------------------------

struct Entry {
    start: usize,
    end: usize,
    span: proc_macro2::Span,
}

/// Where each leaf token is in the text.
pub(crate) struct TokenSpans {
    /// By start offset.
    entries: Vec<Entry>,
}

impl TokenSpans {
    fn new(mut entries: Vec<Entry>) -> TokenSpans {
        entries.sort_by_key(|e| (e.start, e.end));
        TokenSpans { entries }
    }
}

impl SpanMap for TokenSpans {
    fn span_at(&self, offset: usize) -> proc_macro2::Span {
        // (leaves don't overlap: a doc comment's pieces share one stretch, and one span)
        let after = self.entries.partition_point(|e| e.start <= offset);
        if let Some(e) = after.checked_sub(1).map(|i| &self.entries[i])
            && offset < e.end
        {
            return e.span;
        }
        match (self.entries.get(after), self.entries.last()) {
            (Some(next), _) => next.span,
            (None, Some(last)) => last.span,
            (None, None) => proc_macro2::Span::call_site(),
        }
    }
}

// ---- Exact: the source text ------------------------------------------------------------------------------------

/// A token's start or end: line and column (characters), 1-based.
type LineColumn = (u32, u32);

fn start_of(span: Span) -> LineColumn {
    let s = span.start();
    (s.line() as u32, s.column() as u32)
}

fn end_of(span: Span) -> LineColumn {
    let e = span.end();
    (e.line() as u32, e.column() as u32)
}

fn exact(tokens: &[TokenTree]) -> Option<Acquired> {
    let Some(first) = tokens.first() else {
        return Some(Acquired {
            text: String::new(),
            base: Pos::start(1, 1),
            source: TextSource::Exact,
            spans: TokenSpans::new(Vec::new()),
        });
    };
    let file = first.span().file();
    let base = Pos::start(start_of(first.span()).0, 1);
    let mut text = String::new();
    let mut cursor = base;
    let mut previous: Option<(LineColumn, LineColumn, String)> = None;
    for t in tokens {
        let span = t.span();
        let (start, end) = (start_of(span), end_of(span));
        let source = span.source_text()?;
        // (a doc comment's `#` and `[doc = …]`: one comment, one text)
        if previous.as_ref().is_some_and(|p| (p.0, p.1) == (start, end) && p.2 == source) {
            continue;
        }
        if span.file() != file || start < (cursor.line, cursor.column) {
            return None;
        }
        let gap = if start.0 > cursor.line {
            "\n".repeat((start.0 - cursor.line) as usize) + &" ".repeat(start.1 as usize - 1)
        } else {
            " ".repeat((start.1 - cursor.column) as usize)
        };
        cursor = cursor.advanced_by(&gap);
        text += &gap;
        cursor = cursor.advanced_by(&source);
        text += &source;
        if (cursor.line, cursor.column) != end {
            return None;
        }
        previous = Some((start, end, source));
    }
    let lines = LineStarts::of(&text, base.line);
    let mut entries = Vec::new();
    for t in tokens {
        leaves_by_position(t, &lines, &text, &mut entries);
    }
    Some(Acquired { text, base, source: TextSource::Exact, spans: TokenSpans::new(entries) })
}

/// The offsets where the text's lines start.
struct LineStarts {
    first_line: u32,
    starts: Vec<usize>,
}

impl LineStarts {
    fn of(text: &str, first_line: u32) -> LineStarts {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        LineStarts { first_line, starts }
    }

    /// The offset of `line`:`column` in `text` (clamped to the text).
    fn offset(&self, text: &str, (line, column): LineColumn) -> usize {
        let Some(&start) = self.starts.get(line.saturating_sub(self.first_line) as usize) else {
            return text.len();
        };
        let rest = &text[start..];
        let line_text = &rest[..rest.find('\n').unwrap_or(rest.len())];
        start + line_text.char_indices().nth(column.saturating_sub(1) as usize).map_or(line_text.len(), |(i, _)| i)
    }
}

fn leaves_by_position(t: &TokenTree, lines: &LineStarts, text: &str, out: &mut Vec<Entry>) {
    let add = |out: &mut Vec<Entry>, span: Span| {
        let start = lines.offset(text, start_of(span));
        let end = lines.offset(text, end_of(span)).max(start);
        out.push(Entry { start, end, span: span.into() });
    };
    match t {
        TokenTree::Group(g) => {
            add(out, g.span_open());
            for c in g.stream() {
                leaves_by_position(&c, lines, text, out);
            }
            add(out, g.span_close());
        }
        other => add(out, other.span()),
    }
}

// ---- Rebuilt: from the tokens ----------------------------------------------------------------------------------

/// What the previous leaf written was, for spacing and newlines.
#[derive(Clone, Copy, PartialEq)]
enum Prev {
    Start,
    /// A name or keyword: whether it can end an expression.
    Word {
        ends: bool,
    },
    Literal,
    Punct {
        ch: char,
        joint: bool,
    },
    Open,
    Close,
}

struct Writer {
    text: String,
    entries: Vec<Entry>,
    prev: Prev,
}

/// Keywords after which an expression goes on (or that start one): not an expression's end.
const NOT_ENDS: &str = "fun val var class interface object typealias if else when while do for try catch finally \
    return throw in is as by where package import constructor init get set data sealed enum annotation private \
    public internal protected open override abstract inline suspend const lateinit vararg companion inner operator \
    infix out reified";

/// Words that go on the expression before them (infix functions, the keywords continuing one); after an
/// expression's end any other word starts a statement.
const CONTINUES: &str = "in is as else catch finally by where until downTo step to and or xor shl shr ushr get set";

/// Whether `word` is one of the space-separated `words`.
fn listed(words: &str, word: &str) -> bool {
    words.split_whitespace().any(|w| w == word)
}

impl Writer {
    fn ends_expression(&self) -> bool {
        matches!(self.prev, Prev::Word { ends: true } | Prev::Literal | Prev::Close)
    }

    /// Before a leaf: a newline where a statement must have ended, a space where tokens would run together.
    fn separate(&mut self, next: &Next) {
        let newline = self.ends_expression()
            && match next {
                Next::Word(w) => !listed(CONTINUES, w),
                Next::Literal => true,
                // an annotation (after a word: a label, `this@Outer`)
                Next::Punct('@') => !matches!(self.prev, Prev::Word { .. }),
                _ => false,
            };
        if newline {
            self.text.push('\n');
            return;
        }
        let after_operand = matches!(self.prev, Prev::Word { .. } | Prev::Close | Prev::Literal);
        let space = match (self.prev, next) {
            (Prev::Start | Prev::Open, _) => false,
            // `?.`, `!!`, `->`, `::`, `==` … as Rust joined them; `a.b`, `@Name`, `return@label`
            (Prev::Punct { joint: true, .. } | Prev::Punct { ch: '.' | '@', .. }, _) => false,
            (_, Next::Punct('.' | ',' | ';' | '@' | '?')) => false,
            // `x!!`, `x: Int`, `A::class`
            (_, Next::Punct('!' | ':')) => !after_operand,
            (_, Next::Open(Delimiter::Parenthesis | Delimiter::Bracket)) => {
                !matches!(self.prev, Prev::Word { .. } | Prev::Close)
            }
            (_, Next::Close) => false,
            _ => true,
        };
        if space {
            self.text.push(' ');
        }
    }

    fn leaf(&mut self, s: &str, span: Span, next: Next, prev: Prev) {
        self.separate(&next);
        let start = self.text.len();
        self.text += s;
        self.entries.push(Entry { start, end: self.text.len(), span: span.into() });
        self.prev = prev;
    }

    fn tokens(&mut self, tokens: &[TokenTree]) {
        let mut i = 0;
        while i < tokens.len() {
            match &tokens[i] {
                // A doc comment (`#[doc = …]`, `#![doc = …]`): Kotlin's KDoc, left out.
                TokenTree::Punct(p) if p.as_char() == '#' && is_doc(&tokens[i + 1..]) => {
                    i += if matches!(&tokens[i + 1], TokenTree::Punct(b) if b.as_char() == '!') { 3 } else { 2 };
                    continue;
                }
                TokenTree::Group(g) => {
                    let (open, close) = match g.delimiter() {
                        Delimiter::Parenthesis => ("(", ")"),
                        Delimiter::Brace => ("{", "}"),
                        Delimiter::Bracket => ("[", "]"),
                        Delimiter::None => ("", ""),
                    };
                    if !open.is_empty() {
                        self.leaf(open, g.span_open(), Next::Open(g.delimiter()), Prev::Open);
                    }
                    let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                    self.tokens(&inner);
                    if !close.is_empty() {
                        self.leaf(close, g.span_close(), Next::Close, Prev::Close);
                    }
                }
                TokenTree::Ident(id) => {
                    let s = id.to_string();
                    // (an annotation's name, `@Kompozable`, goes on to what it annotates)
                    let ends = !listed(NOT_ENDS, &s) && !matches!(self.prev, Prev::Punct { ch: '@', .. });
                    self.leaf(&s, id.span(), Next::Word(s.clone()), Prev::Word { ends });
                }
                TokenTree::Punct(p) => {
                    let ch = p.as_char();
                    let joint = p.spacing() == Spacing::Joint;
                    // (the second `!` of `!!` ends an expression as a closing bracket does)
                    let prev = if ch == '!' && !joint && self.prev == (Prev::Punct { ch: '!', joint: true }) {
                        Prev::Close
                    } else {
                        Prev::Punct { ch, joint }
                    };
                    self.leaf(&ch.to_string(), p.span(), Next::Punct(ch), prev);
                }
                TokenTree::Literal(l) => self.leaf(&l.to_string(), l.span(), Next::Literal, Prev::Literal),
            }
            i += 1;
        }
    }
}

/// What comes next, for `Writer::separate`.
enum Next {
    Word(String),
    Literal,
    Punct(char),
    Open(Delimiter),
    Close,
}

/// Whether the tokens (after a `#`) are a doc attribute: `[doc …]` or `![doc …]`.
fn is_doc(rest: &[TokenTree]) -> bool {
    let group = match rest {
        [TokenTree::Punct(b), TokenTree::Group(g), ..] if b.as_char() == '!' => g,
        [TokenTree::Group(g), ..] => g,
        _ => return false,
    };
    group.delimiter() == Delimiter::Bracket
        && matches!(group.stream().into_iter().next(), Some(TokenTree::Ident(i)) if i.to_string() == "doc")
}

fn rebuilt(tokens: &[TokenTree]) -> Acquired {
    let mut w = Writer { text: String::new(), entries: Vec::new(), prev: Prev::Start };
    w.tokens(tokens);
    Acquired { text: w.text, base: Pos::start(1, 1), source: TextSource::Rebuilt, spans: TokenSpans::new(w.entries) }
}
