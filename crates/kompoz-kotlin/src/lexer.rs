//! Kotlin's tokens, read from the text (the C++ kompozc's `lexer.cpp`, extended to Kotlin's whole lexical grammar).
//!
//! Module boundary: `lex(text, base, diags) -> Vec<Token>` and the `Token` types are what the parser reads.
//!
//! Names and keywords are both `Ident` (Kotlin's soft keywords — `by`, `get`, `data`, … — are names elsewhere; the
//! parser tells them apart); a name is Kotlin's: letters (Unicode's, `ğ` and `ı` too), digits and `_`, or anything
//! but a newline or backtick between backticks. Newlines are tokens: they end a statement unless the expression
//! goes on, which only the parser knows (a block comment over several lines counts as one). Comments — `//`, `/* */`
//! (nested, as Kotlin's), KDoc's `/** */` — are spacing; a script's `#!` first line is skipped.
//!
//! Numbers as Kotlin writes them: decimal, `0x` hex and `0b` binary digits with `_` between them, the suffixes `L`,
//! `u`/`U`, `uL`/`UL`, `f`/`F`; a fraction (`1.5`, `.5`) or exponent (`1e3`, `2.5E-3`) makes a `Double` (a `Float`
//! with `f`); `1.` is `1` and then a `.` (`16.dp` is a member of 16).
//!
//! Strings: `"…"` with Kotlin's escapes (`\t \b \n \r \' \" \\ \$ \uXXXX`) and `"""…"""` raw (no escapes, newlines
//! kept, the closing quotes the last three of a run), both with templates: `$name` (`` $`a name` `` too) and
//! `${expression}`, whose tokens are lexed in place, at their real positions, as far as the brace closing it (a
//! string inside, a brace in it, a template inside that: all its own). KompozR also takes `\u{…}` — Rust's form,
//! since `kompoz!`'s input goes through Rust's lexer first, which rejects `\uXXXX`, `\$` and `\b`. A character
//! literal holds one UTF-16 unit, as Kotlin's `Char`.
//!
//! Operators by longest match: `?.`, `?:`, `!!`, `::`, `..<`, `===`, `!==`, `->`, `@`, … A mistake (an unterminated
//! string, character or comment, a stray character, a malformed number) is a diagnostic; the lexer goes on after
//! it.

use crate::ast::NumberKind;
use crate::diag::Diagnostics;
use crate::pos::{Loc, Pos};

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    /// A name or a keyword (`fun`, `val`, `if`, …); a backticked name without its backticks (`backticked`).
    Ident { backticked: bool },
    /// A number as written (`text`): its kind and radix from its form and suffix.
    Number { kind: NumberKind, radix: u32 },
    /// `"…"` or `"""…"""` (`raw`): its pieces.
    String { raw: bool, pieces: Vec<StringPiece> },
    /// `'c'`: the character.
    Char(char),
    /// Punctuation and operators: `( ) { } [ ] . , ; : ?. ?: !! -> .. ..< :: == += @ …` (`text`).
    Op,
    /// A line's end (or a block comment over several lines).
    Newline,
    /// The end of the text (or of a template expression's tokens).
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    /// The source text of a name (without backticks), number or operator.
    pub text: String,
    pub loc: Loc,
    /// Whitespace or a comment (not a newline) right before it: `a?` (nullable) and `a ?` differ, as do
    /// `lbl@ {` (a label) and `x @Ann` (an annotation).
    pub space_before: bool,
}

impl Token {
    pub fn is_op(&self, op: &str) -> bool {
        self.kind == TokenKind::Op && self.text == op
    }

    /// A name or keyword spelled `word` (not a backticked one: `` `fun` `` is a name).
    pub fn is_word(&self, word: &str) -> bool {
        self.kind == TokenKind::Ident { backticked: false } && self.text == word
    }

    pub fn is_ident(&self) -> bool {
        matches!(self.kind, TokenKind::Ident { .. })
    }
}

/// A piece of a string literal.
#[derive(Clone, Debug, PartialEq)]
pub enum StringPiece {
    /// Text, its escapes resolved; `loc` is its source (escapes as written).
    Text { text: String, loc: Loc },
    /// `$name`: the name (without backticks, `backticked`) and where it is (after the `$`).
    Name { name: String, backticked: bool, loc: Loc },
    /// `${ … }`: the expression's tokens (ending with `Eof` at the closing brace), `loc` from `${` to `}`.
    Expr { tokens: Vec<Token>, loc: Loc },
}

/// Operators and punctuation, longest first: the lexer takes the first that matches.
const OPS: &[&str] = &[
    "===", "!==", "..<", "?.", "?:", "!!", "->", "..", "::", "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=",
    "/=", "%=", "++", "--", "(", ")", "{", "}", "[", "]", ".", ",", ";", ":", "?", "!", "=", "<", ">", "+", "-", "*",
    "/", "%", "@", "&",
];

/// The tokens of `text`, whose first character is at `base` (its offset 0), ending with `Eof`.
pub fn lex(text: &str, base: Pos, diags: &mut Diagnostics) -> Vec<Token> {
    let mut lexer = Lexer { src: text, pos: base, diags };
    // A script's first line: `#!/usr/bin/env kotlin`.
    if text.starts_with("#!") {
        while !matches!(lexer.peek(0), None | Some('\n')) {
            lexer.advance();
        }
    }
    let mut out = Vec::new();
    loop {
        let t = lexer.next();
        let end = t.kind == TokenKind::Eof;
        out.push(t);
        if end {
            return out;
        }
    }
}

/// A character a name starts with: a letter (Unicode's) or `_`.
pub fn ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

/// A character of a name after its first: a letter, a digit or `_`.
pub fn ident_part(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

struct Lexer<'a, 'd> {
    src: &'a str,
    pos: Pos,
    diags: &'d mut Diagnostics,
}

/// What `skip_space_and_comments` went over.
struct Skipped {
    space: bool,
    /// A block comment with a newline in it: where it ended.
    newline_at: Option<Pos>,
}

impl Lexer<'_, '_> {
    fn peek(&self, ahead: usize) -> Option<char> {
        self.src[self.pos.offset..].chars().nth(ahead)
    }

    fn rest(&self) -> &str {
        &self.src[self.pos.offset..]
    }

    fn advance(&mut self) {
        if let Some(c) = self.peek(0) {
            self.pos = self.pos.advanced_over(c);
        }
    }

    fn advance_by(&mut self, n: usize) {
        for _ in 0..n {
            self.advance();
        }
    }

    fn error(&mut self, at: Pos, message: impl Into<String>) {
        self.diags.error(Loc::new(at, self.pos.max(at)), message);
    }

    /// Spaces, tabs and comments (Kotlin's block comments nest); a newline is a token of its own.
    fn skip_space_and_comments(&mut self) -> Skipped {
        let mut skipped = Skipped { space: false, newline_at: None };
        loop {
            match self.peek(0) {
                Some(' ' | '\t' | '\r' | '\u{c}') => {
                    self.advance();
                    skipped.space = true;
                }
                Some('/') if self.peek(1) == Some('/') => {
                    while !matches!(self.peek(0), None | Some('\n')) {
                        self.advance();
                    }
                    skipped.space = true;
                }
                Some('/') if self.peek(1) == Some('*') => {
                    let start = self.pos;
                    self.advance_by(2);
                    let mut depth = 1;
                    while depth > 0 {
                        if self.rest().starts_with("/*") {
                            self.advance_by(2);
                            depth += 1;
                        } else if self.rest().starts_with("*/") {
                            self.advance_by(2);
                            depth -= 1;
                        } else if self.peek(0).is_none() {
                            self.error(start, "unterminated comment");
                            break;
                        } else {
                            self.advance();
                        }
                    }
                    if self.pos.line > start.line {
                        skipped.newline_at = Some(self.pos);
                    }
                    skipped.space = true;
                }
                _ => return skipped,
            }
        }
    }

    fn next(&mut self) -> Token {
        let skipped = self.skip_space_and_comments();
        let space_before = skipped.space;
        // (a comment over several lines ends a line as a newline does)
        if let Some(at) = skipped.newline_at
            && !matches!(self.peek(0), Some('\n') | None)
        {
            return Token { kind: TokenKind::Newline, text: String::new(), loc: Loc::at(at), space_before };
        }
        let start = self.pos;
        let token = |kind, text: String, end: Pos| Token { kind, text, loc: Loc::new(start, end), space_before };
        let Some(c) = self.peek(0) else {
            return token(TokenKind::Eof, String::new(), start);
        };
        if c == '\n' {
            self.advance();
            return token(TokenKind::Newline, String::new(), self.pos);
        }
        if ident_start(c) {
            let from = self.pos.offset;
            while self.peek(0).is_some_and(ident_part) {
                self.advance();
            }
            return token(TokenKind::Ident { backticked: false }, self.src[from..self.pos.offset].into(), self.pos);
        }
        if c == '`' {
            let name = self.backticked(start);
            return token(TokenKind::Ident { backticked: true }, name, self.pos);
        }
        if c.is_ascii_digit() || (c == '.' && self.peek(1).is_some_and(|d| d.is_ascii_digit())) {
            let (kind, radix) = self.number(start);
            return token(TokenKind::Number { kind, radix }, self.src[start.offset..self.pos.offset].into(), self.pos);
        }
        if c == '"' {
            let (raw, pieces) = self.string(start);
            return token(TokenKind::String { raw, pieces }, String::new(), self.pos);
        }
        if c == '\'' {
            let value = self.char_literal(start);
            return token(TokenKind::Char(value), String::new(), self.pos);
        }
        if let Some(op) = OPS.iter().find(|op| self.rest().starts_with(**op)) {
            self.advance_by(op.chars().count());
            return token(TokenKind::Op, (*op).into(), self.pos);
        }
        self.advance();
        self.error(start, format!("unexpected character `{c}`"));
        self.next()
    }

    /// `` `a name` `` from its opening backtick: the name between them.
    fn backticked(&mut self, start: Pos) -> String {
        self.advance();
        let from = self.pos.offset;
        while !matches!(self.peek(0), Some('`') | Some('\n') | None) {
            self.advance();
        }
        let name = self.src[from..self.pos.offset].to_string();
        if self.peek(0) == Some('`') {
            self.advance();
            if name.is_empty() {
                self.error(start, "a backticked name can't be empty");
            }
        } else {
            self.error(start, "unterminated `name`");
        }
        name
    }

    /// `'c'` from its opening quote: the character (`\0` where it is wrong, with a diagnostic).
    fn char_literal(&mut self, start: Pos) -> char {
        self.advance();
        let value = match self.peek(0) {
            Some('\\') => {
                let at = self.pos;
                self.advance();
                Some(self.escape(at))
            }
            Some(ch) if ch != '\'' && ch != '\n' => {
                self.advance();
                Some(ch)
            }
            _ => None,
        };
        match value {
            Some(v) if self.peek(0) == Some('\'') => {
                self.advance();
                if v.len_utf16() > 1 {
                    self.error(
                        start,
                        format!("a character literal holds one UTF-16 unit, and `{v}` is two (a String holds it)"),
                    );
                }
                v
            }
            _ => {
                // (on past its closing quote, when its line has one)
                let rest = self.rest();
                let line = &rest[..rest.find('\n').unwrap_or(rest.len())];
                let closed = match line.find('\'') {
                    Some(close) => {
                        let n = line[..close].chars().count() + 1;
                        self.advance_by(n);
                        true
                    }
                    None => false,
                };
                let message = if !closed {
                    "unterminated character literal"
                } else if value.is_none() {
                    "a character literal can't be empty"
                } else {
                    "a character literal holds one character"
                };
                self.error(start, message);
                value.unwrap_or('\0')
            }
        }
    }

    /// Digits (`is_digit`) with `_` between them (`1_000`, `1__000`); whether there were any.
    fn digits(&mut self, is_digit: fn(char) -> bool) -> bool {
        let mut any = false;
        loop {
            match self.peek(0) {
                Some(c) if is_digit(c) => {
                    self.advance();
                    any = true;
                }
                // (underscores only between digits)
                Some('_') if any => {
                    let run = self.rest().chars().take_while(|&c| c == '_').count();
                    if !self.peek(run).is_some_and(is_digit) {
                        return any;
                    }
                    self.advance_by(run);
                }
                _ => return any,
            }
        }
    }

    /// A number from here: its kind and radix (the text is the source's).
    fn number(&mut self, start: Pos) -> (NumberKind, u32) {
        let dec = |c: char| c.is_ascii_digit();
        let mut radix = 10;
        let mut fraction = false;
        let prefixed =
            |l: &Self, x: char| l.peek(0) == Some('0') && l.peek(1).is_some_and(|c| c.eq_ignore_ascii_case(&x));
        if prefixed(self, 'x') || prefixed(self, 'b') {
            radix = if prefixed(self, 'x') { 16 } else { 2 };
            self.advance_by(2);
            let any = if radix == 16 {
                self.digits(|c| c.is_ascii_hexdigit())
            } else {
                self.digits(|c| c == '0' || c == '1')
            };
            if !any {
                self.error(
                    start,
                    if radix == 16 { "expected hex digits after `0x`" } else { "expected binary digits after `0b`" },
                );
            }
        } else {
            self.digits(dec);
            // 1.5 and .5 are numbers; 16.dp is 16, then a member (and 1..2 a range).
            if self.peek(0) == Some('.') && self.peek(1).is_some_and(dec) {
                fraction = true;
                self.advance();
                self.digits(dec);
            }
            if matches!(self.peek(0), Some('e' | 'E'))
                && (self.peek(1).is_some_and(dec)
                    || (matches!(self.peek(1), Some('+' | '-')) && self.peek(2).is_some_and(dec)))
            {
                fraction = true;
                self.advance_by(if self.peek(1).is_some_and(dec) { 1 } else { 2 });
                self.digits(dec);
            }
        }
        let mut kind = if fraction { NumberKind::Double } else { NumberKind::Int };
        let suffix_at = self.pos;
        match self.peek(0) {
            Some('f' | 'F') if radix == 10 => {
                self.advance();
                kind = NumberKind::Float;
            }
            Some('L') => {
                self.advance();
                kind = NumberKind::Long;
            }
            Some('u' | 'U') => {
                self.advance();
                kind = NumberKind::UInt;
                if self.peek(0) == Some('L') {
                    self.advance();
                    kind = NumberKind::ULong;
                }
            }
            _ => {}
        }
        if fraction && matches!(kind, NumberKind::Long | NumberKind::UInt | NumberKind::ULong) {
            self.error(
                suffix_at,
                "a number with a fraction or an exponent is a Double (or a Float with `f`): no `L` or `u`",
            );
        }
        if self.peek(0).is_some_and(ident_part) {
            while self.peek(0).is_some_and(ident_part) {
                self.advance();
            }
            self.error(start, "unexpected characters after a number");
        } else if radix == 10
            && !fraction
            && kind != NumberKind::Float
            && self.src[start.offset..].starts_with('0')
            && self.src[start.offset..self.pos.offset].chars().nth(1).is_some_and(|c| c.is_ascii_digit() || c == '_')
        {
            self.error(start, "a number doesn't start with `0` (Kotlin has no octal numbers)");
        }
        (kind, radix)
    }

    /// After a backslash: the escaped character. Kotlin's (`\t \b \n \r \' \" \\ \$ \uXXXX`) and `\u{…}` (Rust's
    /// form: `kompoz!`'s input goes through Rust's lexer, which rejects `\uXXXX`, `\$` and `\b`).
    fn escape(&mut self, at: Pos) -> char {
        let Some(c) = self.peek(0) else {
            self.error(at, "unterminated escape");
            return '\\';
        };
        if c == '\n' {
            self.error(at, "unterminated escape");
            return '\\';
        }
        self.advance();
        match c {
            't' => '\t',
            'b' => '\u{8}',
            'n' => '\n',
            'r' => '\r',
            '\'' => '\'',
            '"' => '"',
            '\\' => '\\',
            '$' => '$',
            'u' if self.peek(0) == Some('{') => {
                self.advance();
                let from = self.pos.offset;
                while self.peek(0).is_some_and(|h| h.is_ascii_hexdigit()) {
                    self.advance();
                }
                let hex = &self.src[from..self.pos.offset];
                let value = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32);
                match value {
                    Some(v) if self.peek(0) == Some('}') && hex.len() <= 6 => {
                        self.advance();
                        v
                    }
                    _ => {
                        if self.peek(0) == Some('}') {
                            self.advance();
                        }
                        self.error(at, "`\\u{…}` takes the hex digits of a character");
                        '\u{fffd}'
                    }
                }
            }
            'u' => {
                let from = self.pos.offset;
                for _ in 0..4 {
                    if self.peek(0).is_some_and(|h| h.is_ascii_hexdigit()) {
                        self.advance();
                    }
                }
                let hex = &self.src[from..self.pos.offset];
                match (hex.len(), u32::from_str_radix(hex, 16).ok().map(|v| (v, char::from_u32(v)))) {
                    (4, Some((_, Some(v)))) => v,
                    (4, Some((v, None))) => {
                        self.error(at, format!("`\\u{v:04X}` is half of a surrogate pair: write the character itself"));
                        '\u{fffd}'
                    }
                    _ => {
                        self.error(at, "`\\u` takes four hex digits");
                        '\u{fffd}'
                    }
                }
            }
            other => {
                self.error(at, format!("unknown escape `\\{other}`"));
                other
            }
        }
    }

    /// `"…"` or `"""…"""` (raw: no escapes, newlines kept) with `$name` and `${expression}` templates.
    fn string(&mut self, start: Pos) -> (bool, Vec<StringPiece>) {
        let raw = self.rest().starts_with("\"\"\"");
        self.advance_by(if raw { 3 } else { 1 });
        let mut pieces = Vec::new();
        let mut text = String::new();
        let mut text_start = self.pos;
        let flush = |pieces: &mut Vec<StringPiece>, text: &mut String, from: Pos, to: Pos| {
            if !text.is_empty() {
                pieces.push(StringPiece::Text { text: std::mem::take(text), loc: Loc::new(from, to) });
            }
        };
        loop {
            let Some(c) = self.peek(0) else {
                flush(&mut pieces, &mut text, text_start, self.pos);
                self.error(start, "unterminated string");
                break;
            };
            if !raw && c == '\n' {
                flush(&mut pieces, &mut text, text_start, self.pos);
                self.error(start, "unterminated string");
                break;
            }
            // (a raw string ends at the last `"""` of a run of quotes: `"""a""""` is `a"`)
            if raw && self.rest().starts_with("\"\"\"") && self.peek(3) != Some('"') {
                flush(&mut pieces, &mut text, text_start, self.pos);
                self.advance_by(3);
                break;
            }
            if !raw && c == '"' {
                flush(&mut pieces, &mut text, text_start, self.pos);
                self.advance();
                break;
            }
            if !raw && c == '\\' {
                let at = self.pos;
                self.advance();
                text.push(self.escape(at));
            } else if c == '$' && (self.peek(1).is_some_and(ident_start) || self.peek(1) == Some('`')) {
                flush(&mut pieces, &mut text, text_start, self.pos);
                self.advance();
                let at = self.pos;
                let (name, backticked) = if self.peek(0) == Some('`') {
                    (self.backticked(at), true)
                } else {
                    while self.peek(0).is_some_and(ident_part) {
                        self.advance();
                    }
                    (self.src[at.offset..self.pos.offset].to_string(), false)
                };
                pieces.push(StringPiece::Name { name, backticked, loc: Loc::new(at, self.pos) });
                text_start = self.pos;
            } else if c == '$' && self.peek(1) == Some('{') {
                flush(&mut pieces, &mut text, text_start, self.pos);
                let at = self.pos;
                self.advance_by(2);
                let mut tokens = Vec::new();
                let mut depth = 0;
                loop {
                    let t = self.next();
                    match t.kind {
                        TokenKind::Eof => {
                            self.error(at, "unterminated `${…}`");
                            tokens.push(t);
                            break;
                        }
                        TokenKind::Op if t.text == "{" => depth += 1,
                        TokenKind::Op if t.text == "}" => {
                            if depth == 0 {
                                tokens.push(Token {
                                    kind: TokenKind::Eof,
                                    text: String::new(),
                                    loc: Loc::at(t.loc.start),
                                    space_before: t.space_before,
                                });
                                break;
                            }
                            depth -= 1;
                        }
                        _ => {}
                    }
                    tokens.push(t);
                }
                pieces.push(StringPiece::Expr { tokens, loc: Loc::new(at, self.pos) });
                text_start = self.pos;
            } else {
                text.push(c);
                self.advance();
            }
        }
        (raw, pieces)
    }
}
