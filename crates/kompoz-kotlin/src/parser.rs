//! Kotlin's syntax read from the tokens into the syntax tree (the C++ kompozc's `parser.cpp`).
//!
//! Module boundary (WP1): `parse(tokens, diags) -> ast::File` is the entry point the pipeline calls; WP1 replaces
//! this module's body with the full grammar (declarations of every kind, statements, the expression precedence
//! chain disjunction → … → primary, templates parsed from their pieces' tokens) and per-declaration recovery
//! (a declaration that fails becomes `Decl::Failed` and reading goes on at the next one).
//!
//! What it reads now: a file of top-level functions — annotations (`@Kompozable`, `@a.b.Name`), modifiers,
//! `fun Name()` with no parameters and an empty block body — separated by newlines or semicolons. Anything else is
//! an error at the token where it stops; reading goes on at the next line that starts a declaration.

use crate::ast::*;
use crate::diag::Diagnostics;
use crate::lexer::{Token, TokenKind};

/// The file the tokens (ending with `Eof`) make.
pub fn parse(tokens: &[Token], diags: &mut Diagnostics) -> File {
    let mut p = Parser { tokens, at: 0, diags };
    let start = p.cur().loc;
    let mut decls = Vec::new();
    loop {
        p.skip_separators();
        if p.cur().kind == TokenKind::Eof {
            break;
        }
        let from = p.at;
        match p.function() {
            Ok(f) => decls.push(Decl::Function(Box::new(f))),
            Err(()) => {
                let loc = p.tokens[from].loc.to(p.tokens[p.at.saturating_sub(1).max(from)].loc);
                p.recover();
                decls.push(Decl::Failed { name: None, loc });
            }
        }
    }
    File { annotations: Vec::new(), package: None, imports: Vec::new(), decls, loc: start.to(p.cur().loc) }
}

struct Parser<'t, 'd> {
    tokens: &'t [Token],
    at: usize,
    diags: &'d mut Diagnostics,
}

type Parsed<T> = Result<T, ()>;

impl Parser<'_, '_> {
    fn cur(&self) -> &Token {
        &self.tokens[self.at.min(self.tokens.len() - 1)]
    }

    fn take(&mut self) -> Token {
        let t = self.cur().clone();
        if t.kind != TokenKind::Eof {
            self.at += 1;
        }
        t
    }

    fn skip_newlines(&mut self) {
        while self.cur().kind == TokenKind::Newline {
            self.at += 1;
        }
    }

    fn skip_separators(&mut self) {
        while self.cur().kind == TokenKind::Newline || self.cur().is_op(";") {
            self.at += 1;
        }
    }

    /// An error at the current token.
    fn fail<T>(&mut self, message: impl Into<String>) -> Parsed<T> {
        let loc = self.cur().loc;
        self.diags.error(loc, message);
        Err(())
    }

    fn expect_op(&mut self, op: &str) -> Parsed<Token> {
        if self.cur().is_op(op) { Ok(self.take()) } else { self.fail(format!("expected `{op}`")) }
    }

    fn ident(&mut self, what: &str) -> Parsed<Ident> {
        match self.cur().kind {
            TokenKind::Ident { backticked } => {
                let t = self.take();
                Ok(Ident { name: t.text, backticked, loc: t.loc })
            }
            _ => self.fail(format!("expected {what}")),
        }
    }

    /// After a failed declaration: on to the next line (outside brackets) that starts one.
    fn recover(&mut self) {
        let mut depth = 0usize;
        loop {
            let t = self.cur();
            match &t.kind {
                TokenKind::Eof => return,
                TokenKind::Op if matches!(t.text.as_str(), "(" | "[" | "{") => depth += 1,
                TokenKind::Op if matches!(t.text.as_str(), ")" | "]" | "}") => depth = depth.saturating_sub(1),
                TokenKind::Newline if depth == 0 => {
                    self.skip_newlines();
                    let t = self.cur();
                    if t.is_op("@") || t.is_word("fun") || ModifierKind::from_keyword(&t.text).is_some() {
                        return;
                    }
                    continue;
                }
                _ => {}
            }
            self.at += 1;
        }
    }

    /// `@Name` / `@a.b.Name` before a declaration.
    fn annotations(&mut self) -> Parsed<Vec<Annotation>> {
        let mut out = Vec::new();
        while self.cur().is_op("@") {
            let at = self.take().loc;
            let first = self.ident("an annotation's name")?;
            let mut segments = vec![first];
            while self.cur().is_op(".") {
                self.take();
                segments.push(self.ident("a name after `.`")?);
            }
            let loc = segments[0].loc.to(segments.last().map_or(at, |s| s.loc));
            out.push(Annotation {
                use_site: None,
                name: Path { segments, loc },
                type_args: Vec::new(),
                args: None,
                loc: at.to(loc),
            });
            self.skip_newlines();
        }
        Ok(out)
    }

    fn modifiers(&mut self) -> Modifiers {
        let mut m = Modifiers::default();
        while let TokenKind::Ident { backticked: false } = self.cur().kind {
            let Some(kind) = ModifierKind::from_keyword(&self.cur().text) else { break };
            let loc = self.take().loc;
            m.list.push(Modifier { kind, loc });
        }
        m
    }

    /// `[annotations] [modifiers] fun Name() { }`.
    fn function(&mut self) -> Parsed<Function> {
        let start = self.cur().loc;
        let annotations = self.annotations()?;
        let modifiers = self.modifiers();
        if !self.cur().is_word("fun") {
            return self.fail("expected a declaration (`fun`)");
        }
        self.take();
        let name = self.ident("the function's name")?;
        self.expect_op("(")?;
        if !self.cur().is_op(")") {
            return self.fail("expected `)`");
        }
        self.take();
        let open = self.expect_op("{")?;
        self.skip_separators();
        if !self.cur().is_op("}") {
            return self.fail("expected `}`");
        }
        let close = self.take();
        Ok(Function {
            annotations,
            modifiers,
            type_params: Vec::new(),
            receiver: None,
            name,
            params: Vec::new(),
            result: None,
            constraints: Vec::new(),
            body: Some(FunctionBody::Block(Block { stmts: Vec::new(), braced: true, loc: open.loc.to(close.loc) })),
            loc: start.to(close.loc),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::pos::Pos;

    fn parse_text(text: &str) -> (File, Diagnostics) {
        let mut d = Diagnostics::default();
        let tokens = lex(text, Pos::start(1, 1), &mut d);
        let file = parse(&tokens, &mut d);
        (file, d)
    }

    #[test]
    fn functions_with_annotations_and_modifiers() {
        let (file, d) =
            parse_text("@Kompozable\nprivate fun Empty() {}\n\n@androidx.compose.runtime.Composable fun B() {\n}");
        assert!(!d.has_errors(), "{:?}", d.list);
        assert_eq!(file.decls.len(), 2);
        let Decl::Function(a) = &file.decls[0] else { panic!() };
        assert!(a.is_kompozable() && a.name.is("Empty") && a.modifiers.visibility() == Visibility::Private);
        assert_eq!((a.loc.start.line, a.loc.end.line), (1, 2));
        let Decl::Function(b) = &file.decls[1] else { panic!() };
        assert!(b.is_kompozable() && b.name.is("B") && b.modifiers.visibility() == Visibility::Public);
    }

    #[test]
    fn a_failed_declaration_and_the_next() {
        let (file, d) = parse_text("val x = 1\n@Kompozable fun A() {}");
        assert_eq!(d.list.len(), 1);
        assert_eq!(d.list[0].message, "expected a declaration (`fun`)");
        assert_eq!(d.list[0].loc.start, Pos { offset: 0, line: 1, column: 1 });
        assert!(matches!(file.decls[0], Decl::Failed { .. }));
        assert!(matches!(&file.decls[1], Decl::Function(f) if f.name.is("A")));
    }
}
