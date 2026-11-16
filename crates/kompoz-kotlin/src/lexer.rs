//! Kotlin's tokens, read from the text (the C++ kompozc's `lexer.cpp`).
//!
//! Module boundary (WP1): `lex(text, base, diags) -> Vec<Token>` and the `Token` types are what the parser reads;
//! WP1 owns this module (and may extend `Token`, e.g. with comments for KDoc), keeping the entry point.
//!
//! Names and keywords are both `Ident` (Kotlin's soft keywords — `by`, `get`, `data`, … — are names elsewhere; the
//! parser tells them apart). Newlines are tokens: they end a statement unless the expression goes on, which only
//! the parser knows. A string's template expressions are lexed in place into tokens of their own (a brace in a
//! string inside one doesn't end it). A mistake is a diagnostic; the lexer goes on after it.

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
    /// A line's end.
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
    /// `f(x)` and a call's argument list on a token after a space in a few places.
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
    /// Text, its escapes resolved.
    Text { text: String, loc: Loc },
    /// `$name`: the name and where it is (after the `$`).
    Name { name: String, loc: Loc },
    /// `${ … }`: the expression's tokens (ending with `Eof` at the closing brace), `loc` from `${` to `}`.
    Expr { tokens: Vec<Token>, loc: Loc },
}

/// Operators and punctuation, longest first: the lexer takes the first that matches.
const OPS: &[&str] = &[
    "===", "!==", "..<", "?.", "?:", "!!", "->", "..", "::", "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=",
    "/=", "%=", "++", "--", "(", ")", "{", "}", "[", "]", ".", ",", ";", ":", "?", "!", "=", "<", ">", "+", "-", "*",
    "/", "%", "@", "&", "|",
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

fn ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn ident_part(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

struct Lexer<'a, 'd> {
    src: &'a str,
    pos: Pos,
    diags: &'d mut Diagnostics,
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

    /// Spaces, tabs and comments (Kotlin's block comments nest); a newline is a token of its own. Whether any
    /// was skipped.
    fn skip_space_and_comments(&mut self) -> bool {
        let mut saw = false;
        loop {
            match self.peek(0) {
                Some(' ' | '\t' | '\r' | '\u{c}') => {
                    self.advance();
                    saw = true;
                }
                Some('/') if self.peek(1) == Some('/') => {
                    while !matches!(self.peek(0), None | Some('\n')) {
                        self.advance();
                    }
                    saw = true;
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
                    saw = true;
                }
                _ => return saw,
            }
        }
    }

    fn next(&mut self) -> Token {
        let space_before = self.skip_space_and_comments();
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
            self.advance();
            let from = self.pos.offset;
            while !matches!(self.peek(0), Some('`') | Some('\n') | None) {
                self.advance();
            }
            let name = self.src[from..self.pos.offset].to_string();
            if self.peek(0) == Some('`') {
                self.advance();
            } else {
                self.error(start, "unterminated `name`");
            }
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
                    return token(TokenKind::Char(v), String::new(), self.pos);
                }
                _ => {
                    // (on past its closing quote, when its line has one)
                    let rest = self.rest();
                    let line = &rest[..rest.find('\n').unwrap_or(rest.len())];
                    if let Some(close) = line.find('\'') {
                        let n = line[..close].chars().count() + 1;
                        self.advance_by(n);
                    }
                    self.error(start, "a character literal holds one character");
                    return token(TokenKind::Char(value.unwrap_or('\0')), String::new(), self.pos);
                }
            }
        }
        if let Some(op) = OPS.iter().find(|op| self.rest().starts_with(**op)) {
            self.advance_by(op.chars().count());
            return token(TokenKind::Op, (*op).into(), self.pos);
        }
        self.advance();
        self.error(start, format!("unexpected character `{c}`"));
        self.next()
    }

    /// A number from here: its kind and radix (the text is the source's).
    fn number(&mut self, start: Pos) -> (NumberKind, u32) {
        let digits = |l: &mut Self, is_digit: fn(char) -> bool| {
            while l.peek(0).is_some_and(is_digit) || (l.peek(0) == Some('_') && l.peek(1).is_some_and(is_digit)) {
                l.advance();
            }
        };
        let dec = |c: char| c.is_ascii_digit();
        let mut radix = 10;
        let mut fraction = false;
        if self.peek(0) == Some('0') && matches!(self.peek(1), Some('x' | 'X')) {
            radix = 16;
            self.advance_by(2);
            digits(self, |c| c.is_ascii_hexdigit());
        } else if self.peek(0) == Some('0') && matches!(self.peek(1), Some('b' | 'B')) {
            radix = 2;
            self.advance_by(2);
            digits(self, |c| c == '0' || c == '1');
        } else {
            digits(self, dec);
            // 1.5 is a number; 16.dp is 16, then a member.
            if self.peek(0) == Some('.') && self.peek(1).is_some_and(dec) {
                fraction = true;
                self.advance();
                digits(self, dec);
            }
            if matches!(self.peek(0), Some('e' | 'E'))
                && (self.peek(1).is_some_and(dec)
                    || (matches!(self.peek(1), Some('+' | '-')) && self.peek(2).is_some_and(dec)))
            {
                fraction = true;
                self.advance_by(2);
                digits(self, dec);
            }
        }
        let mut kind = if fraction { NumberKind::Double } else { NumberKind::Int };
        match self.peek(0) {
            Some('f' | 'F') if radix == 10 => {
                self.advance();
                kind = NumberKind::Float;
            }
            Some('L') if !fraction => {
                self.advance();
                kind = NumberKind::Long;
            }
            Some('u' | 'U') if !fraction => {
                self.advance();
                kind = NumberKind::UInt;
                if self.peek(0) == Some('L') {
                    self.advance();
                    kind = NumberKind::ULong;
                }
            }
            _ => {}
        }
        if self.peek(0).is_some_and(ident_part) {
            while self.peek(0).is_some_and(ident_part) {
                self.advance();
            }
            self.error(start, "unexpected characters after a number");
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
                if self.peek(0) == Some('}') && value.is_some() {
                    self.advance();
                    value.unwrap_or('\u{fffd}')
                } else {
                    self.error(at, "`\\u{…}` takes the hex digits of a character");
                    '\u{fffd}'
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
                match (hex.len(), u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)) {
                    (4, Some(v)) => v,
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
                self.error(start, "unterminated string");
                break;
            };
            if !raw && c == '\n' {
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
            } else if c == '$' && self.peek(1).is_some_and(ident_start) {
                flush(&mut pieces, &mut text, text_start, self.pos);
                self.advance();
                let at = self.pos;
                while self.peek(0).is_some_and(ident_part) {
                    self.advance();
                }
                let name = self.src[at.offset..self.pos.offset].to_string();
                pieces.push(StringPiece::Name { name, loc: Loc::new(at, self.pos) });
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

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<(TokenKind, String)> {
        let mut d = Diagnostics::default();
        let tokens = lex(text, Pos::start(1, 1), &mut d);
        assert!(!d.has_errors(), "{:?}", d.list);
        tokens.into_iter().map(|t| (t.kind, t.text)).collect()
    }

    #[test]
    fn names_operators_and_newlines() {
        let ident = TokenKind::Ident { backticked: false };
        assert_eq!(
            kinds("a?.b ?: c!!\n`in` ..< x"),
            vec![
                (ident.clone(), "a".into()),
                (TokenKind::Op, "?.".into()),
                (ident.clone(), "b".into()),
                (TokenKind::Op, "?:".into()),
                (ident.clone(), "c".into()),
                (TokenKind::Op, "!!".into()),
                (TokenKind::Newline, String::new()),
                (TokenKind::Ident { backticked: true }, "in".into()),
                (TokenKind::Op, "..<".into()),
                (ident, "x".into()),
                (TokenKind::Eof, String::new()),
            ]
        );
    }

    #[test]
    fn numbers() {
        let n = |text: &str| match &kinds(text)[0] {
            (TokenKind::Number { kind, radix }, t) => (*kind, *radix, t.clone()),
            other => panic!("{other:?}"),
        };
        assert_eq!(n("16"), (NumberKind::Int, 10, "16".into()));
        assert_eq!(n("1_000L"), (NumberKind::Long, 10, "1_000L".into()));
        assert_eq!(n("0xFF00FF00"), (NumberKind::Int, 16, "0xFF00FF00".into()));
        assert_eq!(n("0xFFuL"), (NumberKind::ULong, 16, "0xFFuL".into()));
        assert_eq!(n("1.5f"), (NumberKind::Float, 10, "1.5f".into()));
        assert_eq!(n("1e3"), (NumberKind::Double, 10, "1e3".into()));
        assert_eq!(n(".5"), (NumberKind::Double, 10, ".5".into()));
        // 16.dp: a number, then a member.
        assert_eq!(kinds("16.dp").len(), 4);
    }

    #[test]
    fn strings_with_templates_and_positions() {
        let mut d = Diagnostics::default();
        let tokens = lex("\"ğ $name ${if (a) \"}\" else b}\\t\"", Pos::start(5, 3), &mut d);
        assert!(!d.has_errors(), "{:?}", d.list);
        let TokenKind::String { raw: false, pieces } = &tokens[0].kind else { panic!("{:?}", tokens[0]) };
        assert_eq!(pieces.len(), 5);
        assert!(matches!(&pieces[0], StringPiece::Text { text, loc } if text == "ğ " && loc.start.column == 4));
        assert!(matches!(&pieces[1], StringPiece::Name { name, loc } if name == "name" && loc.start.column == 7));
        assert!(matches!(&pieces[2], StringPiece::Text { text, loc } if text == " " && loc.start.column == 11));
        let StringPiece::Expr { tokens: inner, loc } = &pieces[3] else { panic!() };
        assert_eq!(loc.start.column, 12);
        assert!(inner[0].is_word("if") && inner.last().unwrap().kind == TokenKind::Eof);
        assert!(inner.iter().any(|t| matches!(&t.kind, TokenKind::String { .. })));
        assert!(matches!(&pieces[4], StringPiece::Text { text, .. } if text == "\t"));
        assert_eq!(tokens[0].loc.end.column, 3 + 32);
    }

    #[test]
    fn raw_strings_chars_and_comments() {
        let mut d = Diagnostics::default();
        let tokens = lex("/* a /* b */ */ \"\"\"x\n\"y\"\"\"\" '\\u{41}' // end", Pos::start(1, 1), &mut d);
        assert!(!d.has_errors(), "{:?}", d.list);
        let TokenKind::String { raw: true, pieces } = &tokens[0].kind else { panic!("{:?}", tokens[0]) };
        assert!(matches!(&pieces[..], [StringPiece::Text { text, .. }] if text == "x\n\"y\""));
        assert!(tokens[0].space_before);
        assert_eq!(tokens[1].kind, TokenKind::Char('A'));
        assert_eq!(tokens[2].kind, TokenKind::Eof);
    }

    #[test]
    fn mistakes_are_diagnostics() {
        let mut d = Diagnostics::default();
        let tokens = lex("\"abc\n'xy' # 1z", Pos::start(1, 1), &mut d);
        let messages: Vec<&str> = d.list.iter().map(|x| x.message.as_str()).collect();
        assert_eq!(
            messages,
            [
                "unterminated string",
                "a character literal holds one character",
                "unexpected character `#`",
                "unexpected characters after a number"
            ]
        );
        assert_eq!(tokens.last().unwrap().kind, TokenKind::Eof);
    }
}
