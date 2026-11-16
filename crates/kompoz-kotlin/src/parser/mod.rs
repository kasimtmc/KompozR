//! Kotlin's syntax read from the tokens into the syntax tree (the C++ kompozc's `parser.cpp`, extended to Kotlin's
//! grammar for everything the tree has).
//!
//! Module boundary: `parse(tokens, diags) -> ast::File` is the entry point the pipeline calls. `decl` reads
//! declarations, annotations, modifiers and types; `expr` statements and expressions (the precedence chain
//! disjunction → … → postfix → primary; templates parsed from their pieces' tokens).
//!
//! Newlines are Kotlin's (as its own parser, not the published grammar, treats them): the parser sees each token
//! with whether a newline came before it, and a newline counts only where the expression could go on. Inside
//! braces (blocks, lambdas, class and `when` bodies) they count: before `.` and `?.` (and `?:`, `&&`, `||`, `as`)
//! the expression goes on, before any other operator, an infix function's name, `(`, `[`, a trailing lambda's `{`,
//! `++`, `--`, `!!` or `::` it ends; after an operator it always goes on; statements are separated by newlines or
//! `;`; `else`, `catch`, `finally` and a `do`'s `while` are found across them; `return`'s value is on its line.
//! Inside parentheses, brackets and type arguments they don't count (`f(a\n+ b)` is `a + b`). Declarations' headers
//! read across them (`fun f()\n: Int`, a getter on the next line).
//!
//! Recovery: a statement that fails leaves an `ExprKind::Error` statement (a local declaration, `Decl::Failed`)
//! and reading goes on at the next line (or `;`, or the block's `}`); a declaration (top-level or a member) that
//! fails becomes `Decl::Failed` with its name when it got that far, and reading goes on at the next line that
//! starts a declaration (or the body's `}`). Brackets opened before the failure are closed first, so a broken
//! argument list doesn't take the rest of its block with it. Constructs Kotlin has but KompozR's tree can't hold
//! (`T & Any`, a callable reference on a type with type arguments, collection literals) are errors in place:
//! reading goes on after them.

mod decl;
mod expr;

use crate::ast::*;
use crate::diag::Diagnostics;
use crate::lexer::{Token, TokenKind};
use crate::pos::{Loc, Pos};

/// The file the tokens (ending with `Eof`) make.
pub fn parse(tokens: &[Token], diags: &mut Diagnostics) -> File {
    let mut p = Parser::new(tokens, diags);
    p.file()
}

/// A token and whether a newline came before it.
#[derive(Clone, Copy)]
struct Tok<'t> {
    t: &'t Token,
    nl: bool,
}

/// Where the parser reads: whether newlines count, whether a `{` may be a trailing lambda (not in a supertype's
/// `by` expression: `class A : I by impl { … }` has a body), and whether this is a `when` branch's unbraced body
/// (where `in x ->` or `is T ->` starts the next branch: `1 -> "a" in 2..5 -> "b"`, as text rebuilt from tokens
/// puts branches on one line; no Kotlin that parses otherwise means anything else).
#[derive(Clone, Copy)]
struct Mode {
    newlines: bool,
    lambdas: bool,
    when_body: bool,
}

struct Parser<'t, 'd> {
    toks: Vec<Tok<'t>>,
    at: usize,
    mode: Mode,
    diags: &'d mut Diagnostics,
    /// The name of each declaration being read inside a recovery point, once read (`Decl::Failed` keeps it).
    names: Vec<Option<Ident>>,
}

type Parsed<T> = Result<T, ()>;

/// Kotlin's hard keywords: never a name unless backticked.
const HARD_KEYWORDS: &str = "as break class continue do else false for fun if in interface is null object package \
    return super this throw true try typealias typeof val var when while";

/// Whether `word` is one of the space-separated `words`.
fn listed(words: &str, word: &str) -> bool {
    words.split_whitespace().any(|w| w == word)
}

fn is_hard_keyword(t: &Token) -> bool {
    t.kind == (TokenKind::Ident { backticked: false }) && listed(HARD_KEYWORDS, &t.text)
}

/// A name (not a hard keyword, or backticked).
fn is_name(t: &Token) -> bool {
    t.is_ident() && !is_hard_keyword(t)
}

fn ident_of(t: &Token) -> Ident {
    Ident { name: t.text.clone(), backticked: t.kind == (TokenKind::Ident { backticked: true }), loc: t.loc }
}

/// How the tokens of a stretch open and close brackets.
fn bracket_of(t: &Token) -> Option<(bool, char)> {
    if t.kind != TokenKind::Op {
        return None;
    }
    match t.text.as_str() {
        "(" => Some((true, ')')),
        "[" => Some((true, ']')),
        "{" => Some((true, '}')),
        ")" => Some((false, ')')),
        "]" => Some((false, ']')),
        "}" => Some((false, '}')),
        _ => None,
    }
}

impl<'t, 'd> Parser<'t, 'd> {
    fn new(tokens: &'t [Token], diags: &'d mut Diagnostics) -> Parser<'t, 'd> {
        let mut toks = Vec::with_capacity(tokens.len());
        let mut nl = false;
        for t in tokens {
            if t.kind == TokenKind::Newline {
                nl = true;
                continue;
            }
            toks.push(Tok { t, nl });
            nl = false;
            if t.kind == TokenKind::Eof {
                break;
            }
        }
        assert!(toks.last().is_some_and(|t| t.t.kind == TokenKind::Eof), "the tokens end with Eof");
        Parser { toks, at: 0, mode: Mode { newlines: true, lambdas: true, when_body: false }, diags, names: Vec::new() }
    }

    // ---- Tokens ------------------------------------------------------------------------------------------------

    fn cur(&self) -> &'t Token {
        self.toks[self.at].t
    }

    /// The token `n` after the current one (newlines skipped; `Eof` past the end).
    fn peek(&self, n: usize) -> &'t Token {
        self.toks[(self.at + n).min(self.toks.len() - 1)].t
    }

    /// Whether a newline came before the token `n` after the current one, counted or not.
    fn nl_at(&self, n: usize) -> bool {
        self.toks[(self.at + n).min(self.toks.len() - 1)].nl
    }

    /// Whether the token `n` after the current one follows the one before it directly (no space, comment or
    /// newline between them): `lbl@`, `return@lbl`, `as?`, `!in`, `@Ann(args)`.
    fn adjacent(&self, n: usize) -> bool {
        !self.peek(n).space_before && !self.nl_at(n)
    }

    /// Whether a newline that counts (newlines count here) comes before the current token.
    fn nl_before(&self) -> bool {
        self.mode.newlines && self.toks[self.at].nl
    }

    fn take(&mut self) -> &'t Token {
        let t = self.cur();
        if t.kind != TokenKind::Eof {
            self.at += 1;
        }
        t
    }

    fn is_op(&self, op: &str) -> bool {
        self.cur().is_op(op)
    }

    fn is_word(&self, word: &str) -> bool {
        self.cur().is_word(word)
    }

    fn at_eof(&self) -> bool {
        self.cur().kind == TokenKind::Eof
    }

    fn eat_op(&mut self, op: &str) -> Option<&'t Token> {
        if self.is_op(op) { Some(self.take()) } else { None }
    }

    fn eat_word(&mut self, word: &str) -> Option<&'t Token> {
        if self.is_word(word) { Some(self.take()) } else { None }
    }

    fn expect_op(&mut self, op: &str) -> Parsed<&'t Token> {
        match self.eat_op(op) {
            Some(t) => Ok(t),
            None => self.fail(format!("expected `{op}`{}", self.found())),
        }
    }

    fn expect_word(&mut self, word: &str) -> Parsed<&'t Token> {
        match self.eat_word(word) {
            Some(t) => Ok(t),
            None => self.fail(format!("expected `{word}`{}", self.found())),
        }
    }

    /// `, found …` for a message about the current token.
    fn found(&self) -> String {
        let t = self.cur();
        match &t.kind {
            TokenKind::Eof => ", found the end".into(),
            TokenKind::Ident { backticked: true } => format!(", found `` `{}` ``", t.text),
            TokenKind::Ident { .. } if is_hard_keyword(t) => format!(", found keyword `{}`", t.text),
            TokenKind::Ident { .. } | TokenKind::Op | TokenKind::Number { .. } => format!(", found `{}`", t.text),
            TokenKind::String { .. } => ", found a string".into(),
            TokenKind::Char(_) => ", found a character".into(),
            TokenKind::Newline => ", found a new line".into(),
        }
    }

    /// A name: `what` says what it names.
    fn ident(&mut self, what: &str) -> Parsed<Ident> {
        if is_name(self.cur()) {
            Ok(ident_of(self.take()))
        } else {
            self.fail(format!("expected {what}{}", self.found()))
        }
    }

    /// Where the last token taken ends (the current token's start before any).
    fn prev_end(&self) -> Pos {
        match self.at.checked_sub(1) {
            Some(i) => self.toks[i].t.loc.end,
            None => self.cur().loc.start,
        }
    }

    /// From `start` to the end of the last token taken.
    fn loc_from(&self, start: Pos) -> Loc {
        Loc::new(start, self.prev_end().max(start))
    }

    fn start(&self) -> Pos {
        self.cur().loc.start
    }

    /// An error at the current token.
    fn fail<T>(&mut self, message: impl Into<String>) -> Parsed<T> {
        let loc = self.cur().loc;
        self.diags.error(loc, message);
        Err(())
    }

    fn error_at(&mut self, loc: Loc, message: impl Into<String>) {
        self.diags.error(loc, message);
    }

    fn warning_at(&mut self, loc: Loc, message: impl Into<String>) {
        self.diags.warning(loc, message);
    }

    /// `f` with newlines counting (`on`) or not, trailing lambdas allowed again (a bracket's inside).
    fn with_newlines<T>(&mut self, on: bool, f: impl FnOnce(&mut Self) -> T) -> T {
        self.with_mode(Mode { newlines: on, lambdas: true, when_body: false }, f)
    }

    fn with_mode<T>(&mut self, mode: Mode, f: impl FnOnce(&mut Self) -> T) -> T {
        let saved = std::mem::replace(&mut self.mode, mode);
        let r = f(self);
        self.mode = saved;
        r
    }

    /// Tries `f`; when it fails, back to where it started with its diagnostics taken back.
    fn speculate<T>(&mut self, f: impl FnOnce(&mut Self) -> Parsed<T>) -> Option<T> {
        let (at, mode, diags) = (self.at, self.mode, self.diags.list.len());
        match f(self) {
            Ok(v) => Some(v),
            Err(()) => {
                self.at = at;
                self.mode = mode;
                self.diags.list.truncate(diags);
                None
            }
        }
    }

    /// The current token ends a statement: a newline before it, `;`, `}` or the end.
    fn at_statement_end(&self) -> bool {
        self.toks[self.at].nl || self.is_op(";") || self.is_op("}") || self.at_eof()
    }

    // ---- Recovery ----------------------------------------------------------------------------------------------

    /// The declaration's name, once read (for `Decl::Failed`).
    fn named(&mut self, name: &Ident) {
        if let Some(slot) = self.names.last_mut()
            && slot.is_none()
        {
            *slot = Some(name.clone());
        }
    }

    /// The brackets still open after the tokens from `from` to the current one (closed ones matched).
    fn open_brackets(&self, from: usize) -> Vec<char> {
        let mut stack = Vec::new();
        for tok in &self.toks[from..self.at] {
            match bracket_of(tok.t) {
                Some((true, close)) => stack.push(close),
                Some((false, close)) => {
                    if let Some(i) = stack.iter().rposition(|&c| c == close) {
                        stack.truncate(i);
                    }
                }
                None => {}
            }
        }
        stack
    }

    /// Past the rest of a failed statement that started at `from`: the brackets it opened closed, on to the next
    /// line, `;` (taken) or the enclosing block's `}` (left).
    fn skip_statement(&mut self, from: usize) {
        let mut stack = self.open_brackets(from);
        // (failing at a line's start inside a parenthesis: a `)` forgotten, the line is the next statement's)
        if self.at > from && self.toks[self.at].nl && !stack.contains(&'}') {
            return;
        }
        let mut first = true;
        loop {
            let t = self.cur();
            if t.kind == TokenKind::Eof {
                return;
            }
            if stack.is_empty() && !first {
                if self.toks[self.at].nl {
                    return;
                }
                if t.is_op(";") {
                    self.take();
                    return;
                }
            }
            match bracket_of(t) {
                Some((true, close)) => stack.push(close),
                Some((false, close)) => match stack.iter().rposition(|&c| c == close) {
                    Some(i) => stack.truncate(i),
                    // (it closes what the statement is in)
                    None if close == '}' => return,
                    None => {}
                },
                None => {}
            }
            self.take();
            first = false;
        }
    }

    /// Whether the current token can start a declaration (for recovery: where reading goes on).
    fn starts_declaration(&self) -> bool {
        let t = self.cur();
        if t.is_op("@") {
            return true;
        }
        if t.kind != (TokenKind::Ident { backticked: false }) {
            return false;
        }
        matches!(
            t.text.as_str(),
            "fun"
                | "val"
                | "var"
                | "class"
                | "interface"
                | "object"
                | "typealias"
                | "enum"
                | "annotation"
                | "init"
                | "constructor"
                | "import"
                | "package"
        ) || (ModifierKind::from_keyword(&t.text).is_some() && self.peek(1).is_ident())
    }

    /// Past the rest of a failed declaration that started at `from`: the brackets it opened closed, on to the next
    /// line that starts a declaration, past a `;`, or (in a body) to the body's `}`.
    fn skip_declaration(&mut self, from: usize, in_body: bool) {
        let mut stack = self.open_brackets(from);
        let mut first = self.at == from;
        loop {
            let t = self.cur();
            if t.kind == TokenKind::Eof {
                return;
            }
            if !first && self.toks[self.at].nl && self.starts_declaration() {
                // (a parenthesis left open doesn't hide the next declaration's keyword: `fun a(` … `fun b()`)
                let keyword = matches!(t.text.as_str(), "fun" | "class" | "interface" | "object" | "typealias")
                    && t.kind == (TokenKind::Ident { backticked: false });
                if stack.is_empty() || (keyword && !stack.contains(&'}')) {
                    return;
                }
            }
            if stack.is_empty() && !first && t.is_op(";") {
                self.take();
                return;
            }
            match bracket_of(t) {
                Some((true, close)) => stack.push(close),
                Some((false, close)) => match stack.iter().rposition(|&c| c == close) {
                    Some(i) => stack.truncate(i),
                    None if close == '}' && in_body => return,
                    None => {}
                },
                None => {}
            }
            self.take();
            first = false;
        }
    }

    /// `f` as a recovery point: what it gave, and the name of the declaration it was reading when it read one.
    fn attempt<T>(&mut self, f: impl FnOnce(&mut Self) -> Parsed<T>) -> (Parsed<T>, Option<Ident>) {
        let depth = self.names.len();
        self.names.push(None);
        let saved = self.mode;
        let r = f(self);
        self.mode = saved;
        let name = self.names.get(depth).cloned().flatten();
        self.names.truncate(depth);
        (r, name)
    }

    /// A declaration-level recovery point: what `f` reads, or (when it fails) `failed(name, loc)` and reading on at
    /// the next declaration (`in_body`: or the body's `}`).
    fn recovering<T>(
        &mut self,
        in_body: bool,
        f: impl FnOnce(&mut Self) -> Parsed<T>,
        failed: impl FnOnce(Option<Ident>, Loc) -> T,
    ) -> T {
        let (from, start) = (self.at, self.start());
        match self.attempt(f) {
            (Ok(v), _) => v,
            (Err(()), name) => {
                self.skip_declaration(from, in_body);
                failed(name, self.loc_from(start))
            }
        }
    }

    // ---- The file ----------------------------------------------------------------------------------------------

    /// `[@file:annotations] [package a.b] [imports] declarations`.
    fn file(&mut self) -> File {
        let start = self.start();
        let mut annotations = Vec::new();
        while self.is_op("@") && self.peek(1).is_word("file") && self.peek(2).is_op(":") {
            let from = self.at;
            if self.annotation_into(&mut annotations).is_err() {
                self.skip_declaration(from, false);
            }
        }
        let mut package = None;
        if self.is_word("package") {
            let from = self.at;
            self.take();
            match self.path("the package's name") {
                Ok(p) => package = Some(p),
                Err(()) => self.skip_declaration(from, false),
            }
        }
        let mut imports = Vec::new();
        let mut decls = Vec::new();
        loop {
            while self.eat_op(";").is_some() {}
            if self.at_eof() {
                break;
            }
            if self.is_word("import") {
                if !decls.is_empty() {
                    let loc = self.cur().loc;
                    self.error_at(loc, "imports go before the declarations");
                }
                let from = self.at;
                match self.import() {
                    Ok(i) => imports.push(i),
                    Err(()) => self.skip_declaration(from, false),
                }
                continue;
            }
            if self.is_op("}") {
                let loc = self.take().loc;
                self.error_at(loc, "unexpected `}`: nothing to close");
                continue;
            }
            let d = self.recovering(false, |p| p.top_level_declaration(), |name, loc| Decl::Failed { name, loc });
            decls.push(d);
        }
        File { annotations, package, imports, decls, loc: Loc::new(start, self.cur().loc.end) }
    }

    /// `import a.b.C`, `import a.b.*`, `import a.b.C as D`.
    fn import(&mut self) -> Parsed<Import> {
        let start = self.take().loc.start;
        let first = self.ident("the imported name")?;
        let mut segments = vec![first];
        let mut star = false;
        while self.is_op(".") {
            self.take();
            if self.eat_op("*").is_some() {
                star = true;
                break;
            }
            segments.push(self.ident("a name after `.`")?);
        }
        let path = Path { loc: segments[0].loc.to(segments[segments.len() - 1].loc), segments };
        let alias = if !star && self.eat_word("as").is_some() { Some(self.ident("the import's alias")?) } else { None };
        Ok(Import { path, star, alias, loc: self.loc_from(start) })
    }

    /// `a.b.c`.
    fn path(&mut self, what: &str) -> Parsed<Path> {
        let first = self.ident(what)?;
        let mut segments = vec![first];
        while self.is_op(".") && is_name(self.peek(1)) && !self.mode_nl_at(1) {
            self.take();
            segments.push(self.ident("a name after `.`")?);
        }
        Ok(Path { loc: segments[0].loc.to(segments[segments.len() - 1].loc), segments })
    }

    /// Whether a newline that counts comes before the token `n` after the current one.
    fn mode_nl_at(&self, n: usize) -> bool {
        self.mode.newlines && self.nl_at(n)
    }
}
