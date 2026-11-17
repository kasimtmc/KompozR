//! The Kotlin text of a `kompoz! { … }` and the span of every offset in it.
//!
//! Under rustc every token has its source text and its line and column (characters, 1-based). The text is each
//! top-level token's source text (a group's holds its comments and spacing as written) with the gaps between them
//! as the macro call's source text has them (comments, and what Rust's lexer couldn't read: a backticked name's
//! backticks), so that a position in the text is the file's line:column; it starts at the first token's line,
//! padded to its column (`base` = that line, column 1). Where the call's text doesn't hold the tokens (`kompoz!`
//! written by another macro around the user's tokens) the gaps are rebuilt — newlines, then spaces. A doc comment,
//! which Rust makes `#` + `[doc = "…"]` all carrying the comment's span, is taken once (as its source text).
//!
//! Without source text (rust-analyzer's proc-macro server: none, and every position 1:1), or when the positions
//! don't add up (tokens made by another macro), the text is rebuilt from the tokens themselves:
//! [`TextSource::Rebuilt`]. Spacing follows the tokens (no space after a joint punctuation, around `.`, before an
//! argument list …); newlines, which Kotlin needs between statements, go after a token that can end an expression
//! (a name, a literal, a closing bracket, `!!`, a postfix `++`/`--`) before one that starts another: a name other
//! than an infix function's or a continuing keyword (`else`, `catch`, `in`, …) or a trailing lambda's label
//! (`forEach lit@{`), a literal, an annotation. Doc comments are left out. It is best effort — what an editor sees — while the real build has the exact text.
//!
//! The span map: every leaf token (a group's delimiters too) with its offsets in the text; an offset maps to the
//! token containing it (a Kotlin template split into several Rust literals maps to each piece's), else to the
//! next token, else to the last. The leaves also tell what Rust's lexer rejected of the Kotlin (`rejected`).

use crate::rejected;
use kompoz_kotlin::{Diagnostic, Pos, SpanMap, TextSource};
use proc_macro::{Delimiter, Spacing, Span, TokenStream, TokenTree};

pub(crate) struct Acquired {
    pub text: String,
    pub base: Pos,
    pub source: TextSource,
    pub spans: TokenSpans,
    /// What Rust's lexer rejected of the Kotlin (rustc has reported it): why, and what to write instead.
    pub rejected: Vec<Diagnostic>,
}

/// The macro's input as Kotlin text.
pub(crate) fn acquire(input: TokenStream) -> Acquired {
    let tokens: Vec<TokenTree> = input.into_iter().collect();
    let mut a = exact(&tokens).unwrap_or_else(|| rebuilt(&tokens));
    a.rejected = rejected::find(&a.text, a.base, &a.spans.entries, a.source);
    a
}

// ---- The span map ----------------------------------------------------------------------------------------------

/// What a leaf token is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Leaf {
    Ident,
    Literal,
    Punct(char),
    /// A group's opening or closing delimiter.
    Delimiter,
}

pub(crate) struct Entry {
    pub start: usize,
    pub end: usize,
    pub span: proc_macro2::Span,
    pub leaf: Leaf,
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
            rejected: Vec::new(),
        });
    };
    // The top-level tokens: where each is, and its text (a doc comment's `#` and `[doc = …]`: one comment, one
    // text).
    let file = first.span().file();
    let mut pieces: Vec<(LineColumn, LineColumn, String)> = Vec::new();
    for t in tokens {
        let span = t.span();
        let (start, end) = (start_of(span), end_of(span));
        let source = span.source_text()?;
        if pieces.last().is_some_and(|p| (p.0, p.1) == (start, end) && p.2 == source) {
            continue;
        }
        if span.file() != file {
            return None;
        }
        pieces.push((start, end, source));
    }
    // (the call's text, where it holds every token as it is)
    let call = CallText::of(Span::call_site())
        .filter(|c| pieces.iter().all(|(start, end, source)| c.slice(*start, *end) == Some(source.as_str())));
    let base = Pos::start(pieces[0].0.0, 1);
    let mut text = String::new();
    let mut cursor = base;
    for (start, end, source) in pieces {
        let at = (cursor.line, cursor.column);
        if start < at {
            return None;
        }
        // (before the first token: the padding to its column, not the call's `kompoz! {`)
        let verbatim = call.as_ref().filter(|_| !text.is_empty()).and_then(|c| c.slice(at, start));
        let gap = match verbatim {
            Some(g) if line_column(cursor.advanced_by(g)) == start => g.to_string(),
            _ if start.0 > cursor.line => {
                "\n".repeat((start.0 - cursor.line) as usize) + &" ".repeat(start.1 as usize - 1)
            }
            _ => " ".repeat((start.1 - cursor.column) as usize),
        };
        cursor = cursor.advanced_by(&gap);
        text += &gap;
        cursor = cursor.advanced_by(&source);
        text += &source;
        if line_column(cursor) != end {
            return None;
        }
    }
    let lines = LineStarts::of(&text, base.line);
    let mut entries = Vec::new();
    for t in tokens {
        leaves_by_position(t, &lines, &text, &mut entries);
    }
    Some(Acquired { text, base, source: TextSource::Exact, spans: TokenSpans::new(entries), rejected: Vec::new() })
}

fn line_column(p: Pos) -> LineColumn {
    (p.line, p.column)
}

/// The macro call's source text (`kompoz! { … }` as written) and where it starts.
struct CallText {
    text: String,
    start: LineColumn,
    /// The offsets where its lines start.
    lines: Vec<usize>,
}

impl CallText {
    fn of(span: Span) -> Option<CallText> {
        let text = span.source_text()?;
        let mut lines = vec![0];
        lines.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Some(CallText { text, start: start_of(span), lines })
    }

    /// The offset of the file's `line`:`column` in the call's text, if it holds it (just past a line's end too).
    fn offset(&self, (line, column): LineColumn) -> Option<usize> {
        let i = line.checked_sub(self.start.0)? as usize;
        let start = *self.lines.get(i)?;
        let first_column = if i == 0 { self.start.1 } else { 1 };
        let skip = column.checked_sub(first_column)? as usize;
        let rest = &self.text[start..];
        let line_text = &rest[..rest.find('\n').unwrap_or(rest.len())];
        match line_text.char_indices().nth(skip) {
            Some((at, _)) => Some(start + at),
            None => (line_text.chars().count() == skip).then_some(start + line_text.len()),
        }
    }

    /// The call's text from `from` to `to` (positions in the file), if it holds them.
    fn slice(&self, from: LineColumn, to: LineColumn) -> Option<&str> {
        let (a, b) = (self.offset(from)?, self.offset(to)?);
        (a <= b).then(|| &self.text[a..b])
    }
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
    let add = |out: &mut Vec<Entry>, span: Span, leaf: Leaf| {
        let start = lines.offset(text, start_of(span));
        let end = lines.offset(text, end_of(span)).max(start);
        out.push(Entry { start, end, span: span.into(), leaf });
    };
    match t {
        TokenTree::Group(g) => {
            add(out, g.span_open(), Leaf::Delimiter);
            for c in g.stream() {
                leaves_by_position(&c, lines, text, out);
            }
            add(out, g.span_close(), Leaf::Delimiter);
        }
        TokenTree::Ident(i) => add(out, i.span(), Leaf::Ident),
        TokenTree::Literal(l) => add(out, l.span(), Leaf::Literal),
        TokenTree::Punct(p) => add(out, p.span(), Leaf::Punct(p.as_char())),
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
    /// Whether what came before the joint punctuation just written ended an expression (so `x++` is a postfix
    /// increment, which ends one too, and `++x` a prefix one).
    after_end: bool,
    /// The last word written, while it is the last token (`return` of `return@lit`).
    last_word: Option<String>,
    /// The `@` just written follows `return`, `break`, `continue`, `this` or `super`: the name after it is a label
    /// that ends the expression, not an annotation's.
    label_ref: bool,
    /// The `++`/`--` just written was a postfix one.
    after_step: bool,
    /// The first character of a `++`/`--` was just written.
    step_open: Option<char>,
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
                // (a trailing lambda's label: `forEach lit@{ … }` goes on)
                Next::Label => false,
                // (`++`/`--` after what ended an expression and was itself a postfix one: a prefix one, `++i`, on a
                // statement of its own)
                Next::Step if self.prev == Prev::Close && self.after_step => true,
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
            // (`i++`, its two characters together)
            (_, Next::Step) => !after_operand && self.step_open.is_none(),
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
        let leaf = match next {
            Next::Word(_) | Next::Label => Leaf::Ident,
            Next::Literal => Leaf::Literal,
            Next::Open(_) | Next::Close => Leaf::Delimiter,
            Next::Punct(c) => Leaf::Punct(c),
            Next::Step => Leaf::Punct(s.chars().next().unwrap_or('+')),
        };
        self.entries.push(Entry { start, end: self.text.len(), span: span.into(), leaf });
        self.prev = prev;
        self.after_step = false;
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
                    // (an annotation's name, `@Kompozable`, goes on to what it annotates; `return@lit`'s label ends
                    // the expression)
                    let after_at = matches!(self.prev, Prev::Punct { ch: '@', .. });
                    let ends = !listed(NOT_ENDS, &s) && (!after_at || self.label_ref);
                    let label = matches!(
                        &tokens[i + 1..],
                        [TokenTree::Punct(at), TokenTree::Group(g), ..] if at.as_char() == '@' && g.delimiter() == Delimiter::Brace
                    );
                    let next = if label { Next::Label } else { Next::Word(s.clone()) };
                    self.leaf(&s, id.span(), next, Prev::Word { ends });
                    self.last_word = Some(s);
                }
                TokenTree::Punct(p) => {
                    let ch = p.as_char();
                    let joint = p.spacing() == Spacing::Joint;
                    let closes_step = self.step_open == Some(ch);
                    let closes_bang = ch == '!' && !joint && self.prev == (Prev::Punct { ch: '!', joint: true });
                    // (the second `!` of `!!` ends an expression as a closing bracket does; so does a postfix
                    // `++`/`--`'s)
                    let prev = if closes_bang || (closes_step && self.after_end) {
                        Prev::Close
                    } else {
                        Prev::Punct { ch, joint }
                    };
                    // (the first of `++`/`--`: joint, or `+ +` where another macro's tokens lost their spacing —
                    // `a + +b` isn't Kotlin anyone writes, `a - -b` is)
                    let opens_step = !closes_step
                        && matches!(ch, '+' | '-')
                        && matches!(tokens.get(i + 1), Some(TokenTree::Punct(n)) if n.as_char() == ch)
                        && (joint || ch == '+');
                    if opens_step {
                        // (after a postfix one it starts a statement: a prefix one)
                        let starts_statement = self.prev == Prev::Close && self.after_step;
                        self.after_end = self.ends_expression() && !starts_statement;
                    }
                    let postfix = closes_step && self.after_end;
                    self.label_ref = ch == '@'
                        && matches!(self.prev, Prev::Word { .. })
                        && self.last_word.as_deref().is_some_and(|w| listed("return break continue this super", w));
                    let next = if opens_step || closes_step { Next::Step } else { Next::Punct(ch) };
                    self.leaf(&ch.to_string(), p.span(), next, prev);
                    self.step_open = opens_step.then_some(ch);
                    self.after_step = postfix;
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
    /// A name that labels a lambda (`lit` of `lit@{`).
    Label,
    /// The first character of `++` or `--`.
    Step,
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
    let mut w = Writer {
        text: String::new(),
        entries: Vec::new(),
        prev: Prev::Start,
        after_end: false,
        last_word: None,
        label_ref: false,
        after_step: false,
        step_open: None,
    };
    w.tokens(tokens);
    Acquired {
        text: w.text,
        base: Pos::start(1, 1),
        source: TextSource::Rebuilt,
        spans: TokenSpans::new(w.entries),
        rejected: Vec::new(),
    }
}
