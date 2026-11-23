//! Statements and expressions: blocks, loops, assignments, Kotlin's precedence chain (disjunction → conjunction →
//! equality → comparison → `in`/`is` → elvis → infix calls → ranges → additive → multiplicative → `as` → prefix →
//! postfix → primary), lambdas, `if`/`when`/`try`, object literals, anonymous functions, string templates.

use super::decl::Head;
use super::{HARD_KEYWORDS, Parsed, Parser, ident_of, is_hard_keyword, is_name, listed};
use crate::ast::*;
use crate::lexer::{StringPiece, Token, TokenKind};
use crate::pos::{Loc, Pos};

/// The hard keywords that start an expression.
const EXPRESSION_KEYWORDS: &str = "true false null this super if when try throw return break continue object fun";

fn binary(op: BinaryOp, lhs: Expr, rhs: Expr) -> Expr {
    let loc = lhs.loc.to(rhs.loc);
    Expr { kind: ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }, loc }
}

impl<'t> Parser<'t, '_> {
    // ---- Blocks and statements ---------------------------------------------------------------------------------

    /// `{ statements }`.
    pub(super) fn block(&mut self) -> Parsed<Block> {
        let open = self.expect_op("{")?.loc.start;
        self.with_newlines(true, |p| {
            let stmts = p.statements();
            p.expect_op("}")?;
            Ok(Block { stmts, braced: true, loc: p.loc_from(open) })
        })
    }

    /// An empty body (`if (a) ; else b`, `while (x) ;`), where the parser is.
    fn empty_block(&self) -> Block {
        Block { stmts: Vec::new(), braced: false, loc: Loc::at(self.prev_end()) }
    }

    /// A control structure's body: a block, or one statement.
    fn control_body(&mut self) -> Parsed<Block> {
        if self.is_op("{") {
            return self.block();
        }
        let start = self.start();
        let s = self.statement()?;
        Ok(Block { stmts: vec![s], braced: false, loc: self.loc_from(start) })
    }

    /// Statements up to a `}` (not taken), separated by newlines or `;`; each failed one an error statement.
    pub(super) fn statements(&mut self) -> Vec<Stmt> {
        let mut out = Vec::new();
        loop {
            while self.eat_op(";").is_some() {}
            if self.is_op("}") || self.at_eof() {
                return out;
            }
            out.push(self.recovering_statement());
            if !self.at_statement_end() {
                let loc = self.cur().loc;
                self.error_at(loc, format!("expected a new line or `;` after the statement{}", self.found()));
                let at = self.at;
                self.skip_statement(at);
            }
        }
    }

    /// A statement, or (when it fails) an error statement and reading on at the next line.
    fn recovering_statement(&mut self) -> Stmt {
        let from = self.at;
        let start = self.start();
        let declaration = self.local_declaration_ahead();
        match self.attempt(|p| p.statement()) {
            (Ok(s), _) => s,
            (Err(()), name) => {
                self.skip_statement(from);
                let loc = self.loc_from(start);
                let kind = if declaration {
                    StmtKind::Decl(Decl::Failed { name, loc })
                } else {
                    StmtKind::Expr(Expr { kind: ExprKind::Error, loc })
                };
                Stmt { kind, loc }
            }
        }
    }

    pub(super) fn statement(&mut self) -> Parsed<Stmt> {
        let start = self.start();
        if self.label_ahead() && ["for", "while", "do"].iter().any(|w| self.peek(2).is_word(w)) {
            let label = ident_of(self.take());
            self.take();
            return self.loop_statement(Some(label), start);
        }
        if self.local_declaration_ahead() {
            let d = self.local_declaration()?;
            return Ok(Stmt { kind: StmtKind::Decl(d), loc: self.loc_from(start) });
        }
        if ["for", "while", "do"].iter().any(|w| self.is_word(w)) {
            return self.loop_statement(None, start);
        }
        // (annotations at a statement's start are the whole statement's: `@Suppress("x") a + b`)
        let annotations = self.annotations()?;
        if let Some(first) = annotations.first().map(|a| a.loc)
            && ["for", "while", "do"].iter().any(|w| self.is_word(w))
        {
            self.warning_at(first, "an annotation on a loop has no effect here (it is ignored)");
            return self.loop_statement(None, start);
        }
        let target = self.expression()?;
        let op = match self.cur().text.as_str() {
            _ if self.cur().kind != TokenKind::Op || self.nl_before() => None,
            "=" => Some(AssignOp::Assign),
            "+=" => Some(AssignOp::Add),
            "-=" => Some(AssignOp::Sub),
            "*=" => Some(AssignOp::Mul),
            "/=" => Some(AssignOp::Div),
            "%=" => Some(AssignOp::Rem),
            _ => None,
        };
        if let Some(op) = op {
            self.take();
            let value = self.expression()?;
            if let Some(first) = annotations.first().map(|a| a.loc) {
                self.warning_at(first, "an annotation on an assignment has no effect here (it is ignored)");
            }
            return Ok(Stmt { kind: StmtKind::Assign { target, op, value }, loc: self.loc_from(start) });
        }
        let expr = if annotations.is_empty() {
            target
        } else {
            Expr { kind: ExprKind::Annotated { annotations, expr: Box::new(target) }, loc: self.loc_from(start) }
        };
        Ok(Stmt { kind: StmtKind::Expr(expr), loc: self.loc_from(start) })
    }

    /// `for`, `while`, `do … while` (at the keyword, its label read).
    fn loop_statement(&mut self, label: Option<Ident>, start: Pos) -> Parsed<Stmt> {
        let word = self.take();
        let kind = match word.text.as_str() {
            "for" => {
                self.expect_op("(")?;
                let (binding, ty, iterable) = self.with_newlines(false, |p| {
                    let annotations = p.annotations()?;
                    if let Some(a) = annotations.first() {
                        let loc = a.loc;
                        p.warning_at(loc, "an annotation on a loop's variable has no effect here (it is ignored)");
                    }
                    let binding = if p.is_op("(") {
                        Binding::Destructure(p.destructure()?)
                    } else {
                        Binding::Name(p.ident("the loop's variable")?)
                    };
                    let ty = if p.eat_op(":").is_some() { Some(p.ty()?) } else { None };
                    p.expect_word("in")?;
                    let iterable = p.expression()?;
                    p.expect_op(")")?;
                    Ok((binding, ty, iterable))
                })?;
                let body = self.loop_body()?;
                StmtKind::For { label, binding, ty, iterable, body }
            }
            "while" => {
                let condition = self.parenthesized_condition()?;
                let body = self.loop_body()?;
                StmtKind::While { label, condition, body }
            }
            _ => {
                let body = if self.is_word("while") { self.empty_block() } else { self.control_body()? };
                self.expect_word("while")?;
                let condition = self.parenthesized_condition()?;
                StmtKind::DoWhile { label, body, condition }
            }
        };
        Ok(Stmt { kind, loc: self.loc_from(start) })
    }

    /// A `for`'s or `while`'s body (`;`: an empty one).
    fn loop_body(&mut self) -> Parsed<Block> {
        if self.is_op(";") {
            self.take();
            return Ok(self.empty_block());
        }
        self.control_body()
    }

    /// `(condition)` of `if`, `while`, `do … while`.
    fn parenthesized_condition(&mut self) -> Parsed<Expr> {
        self.expect_op("(")?;
        self.with_newlines(false, |p| {
            let e = p.expression()?;
            p.expect_op(")")?;
            Ok(e)
        })
    }

    // ---- The precedence chain ----------------------------------------------------------------------------------

    pub(super) fn expression(&mut self) -> Parsed<Expr> {
        self.disjunction()
    }

    fn disjunction(&mut self) -> Parsed<Expr> {
        let mut l = self.conjunction()?;
        // (`||` and `&&` go on across a newline before them)
        while self.eat_op("||").is_some() {
            let r = self.conjunction()?;
            l = binary(BinaryOp::Or, l, r);
        }
        Ok(l)
    }

    fn conjunction(&mut self) -> Parsed<Expr> {
        let mut l = self.equality()?;
        while self.eat_op("&&").is_some() {
            let r = self.equality()?;
            l = binary(BinaryOp::And, l, r);
        }
        Ok(l)
    }

    /// The operator of `ops` here, on the expression's line.
    fn binary_op(&self, ops: &[(&str, BinaryOp)]) -> Option<BinaryOp> {
        if self.nl_before() || self.cur().kind != TokenKind::Op {
            return None;
        }
        ops.iter().find(|(text, _)| self.cur().text == *text).map(|&(_, op)| op)
    }

    fn equality(&mut self) -> Parsed<Expr> {
        use BinaryOp::*;
        let mut l = self.comparison()?;
        while let Some(op) = self.binary_op(&[("==", Eq), ("!=", NotEq), ("===", Identical), ("!==", NotIdentical)]) {
            self.take();
            let r = self.comparison()?;
            l = binary(op, l, r);
        }
        Ok(l)
    }

    fn comparison(&mut self) -> Parsed<Expr> {
        use BinaryOp::*;
        let mut l = self.named_checks()?;
        while let Some(op) = self.binary_op(&[("<", Lt), (">", Gt), ("<=", Le), (">=", Ge)]) {
            self.take();
            let r = self.named_checks()?;
            l = binary(op, l, r);
        }
        Ok(l)
    }

    /// Whether `!in` / `!is` (`word`) is here: a `!` right before the keyword.
    fn negated_word_ahead(&self, word: &str) -> bool {
        self.is_op("!") && self.peek(1).is_word(word) && self.adjacent(1)
    }

    /// `in`, `!in`, `is`, `!is`.
    fn named_checks(&mut self) -> Parsed<Expr> {
        let mut l = self.elvis()?;
        loop {
            if self.nl_before() {
                return Ok(l);
            }
            let negated = self.negated_word_ahead("in") || self.negated_word_ahead("is");
            if !negated && !self.is_word("in") && !self.is_word("is") {
                return Ok(l);
            }
            if self.mode.when_body && self.when_head_ahead() {
                return Ok(l);
            }
            if negated {
                self.take();
            }
            if self.take().text == "in" {
                let r = self.elvis()?;
                l = binary(if negated { BinaryOp::NotIn } else { BinaryOp::In }, l, r);
            } else {
                let ty = self.ty()?;
                let loc = l.loc.to(ty.loc);
                l = Expr { kind: ExprKind::Is { expr: Box::new(l), negated, ty: Box::new(ty) }, loc };
            }
        }
    }

    fn elvis(&mut self) -> Parsed<Expr> {
        let mut l = self.infix_call()?;
        while self.eat_op("?:").is_some() {
            let r = self.infix_call()?;
            l = binary(BinaryOp::Elvis, l, r);
        }
        Ok(l)
    }

    /// `a until b`, `x to y`: a name between two operands, on the first's line.
    fn infix_call(&mut self) -> Parsed<Expr> {
        let mut l = self.range()?;
        while is_name(self.cur()) && !self.nl_before() && !self.label_ahead() {
            let name = ident_of(self.take());
            let r = self.range()?;
            let loc = l.loc.to(r.loc);
            l = Expr { kind: ExprKind::Infix { name, lhs: Box::new(l), rhs: Box::new(r) }, loc };
        }
        Ok(l)
    }

    fn range(&mut self) -> Parsed<Expr> {
        use BinaryOp::*;
        let mut l = self.additive()?;
        while let Some(op) = self.binary_op(&[("..", RangeTo), ("..<", RangeUntil)]) {
            self.take();
            let r = self.additive()?;
            l = binary(op, l, r);
        }
        Ok(l)
    }

    fn additive(&mut self) -> Parsed<Expr> {
        use BinaryOp::*;
        let mut l = self.multiplicative()?;
        while let Some(op) = self.binary_op(&[("+", Add), ("-", Sub)]) {
            self.take();
            let r = self.multiplicative()?;
            l = binary(op, l, r);
        }
        Ok(l)
    }

    fn multiplicative(&mut self) -> Parsed<Expr> {
        use BinaryOp::*;
        let mut l = self.as_expression()?;
        while let Some(op) = self.binary_op(&[("*", Mul), ("/", Div), ("%", Rem)]) {
            self.take();
            let r = self.as_expression()?;
            l = binary(op, l, r);
        }
        Ok(l)
    }

    /// `x as T`, `x as? T` (going on across a newline before `as`).
    fn as_expression(&mut self) -> Parsed<Expr> {
        let mut l = self.prefix()?;
        while self.is_word("as") {
            self.take();
            let safe = self.is_op("?") && self.adjacent(0);
            if safe {
                self.take();
            }
            let ty = self.ty()?;
            let loc = l.loc.to(ty.loc);
            l = Expr { kind: ExprKind::As { expr: Box::new(l), safe, ty: Box::new(ty) }, loc };
        }
        Ok(l)
    }

    /// A label here: a name with `@` right after it (`outer@`, `lbl@{`).
    pub(super) fn label_ahead(&self) -> bool {
        is_name(self.cur()) && self.peek(1).is_op("@") && self.adjacent(1)
    }

    /// Prefix operators, labels (`lbl@ { … }`) and annotations (`@Ann expr`).
    fn prefix(&mut self) -> Parsed<Expr> {
        let start = self.start();
        if self.label_ahead() {
            let label = ident_of(self.take());
            self.take();
            let mut e = self.prefix()?;
            match &mut e.kind {
                ExprKind::Lambda(l) if l.label.is_none() => {
                    l.label = Some(label);
                    l.loc = Loc::new(start, l.loc.end);
                    e.loc = l.loc;
                }
                _ => self.warning_at(
                    label.loc,
                    format!(
                        "the label `{}@` names nothing `return@`, `break@` or `continue@` can go to (only a lambda or a loop): it is ignored",
                        label.name
                    ),
                ),
            }
            return Ok(e);
        }
        if self.is_op("@") {
            let annotations = self.annotations()?;
            let e = self.prefix()?;
            return Ok(Expr {
                kind: ExprKind::Annotated { annotations, expr: Box::new(e) },
                loc: self.loc_from(start),
            });
        }
        let op = match self.cur().text.as_str() {
            _ if self.cur().kind != TokenKind::Op => None,
            "-" => Some(PrefixOp::Minus),
            "+" => Some(PrefixOp::Plus),
            "!" => Some(PrefixOp::Not),
            "++" => Some(PrefixOp::Inc),
            "--" => Some(PrefixOp::Dec),
            "!!" => Some(PrefixOp::Not),
            _ => None,
        };
        let Some(op) = op else {
            return self.postfix();
        };
        let double = self.take().text == "!!";
        let operand = self.prefix()?;
        let mut e = Expr { kind: ExprKind::Prefix { op, operand: Box::new(operand) }, loc: self.loc_from(start) };
        if double {
            // (`!!x`: two negations)
            let loc = e.loc;
            e = Expr { kind: ExprKind::Prefix { op: PrefixOp::Not, operand: Box::new(e) }, loc };
        }
        Ok(e)
    }

    // ---- Postfix -----------------------------------------------------------------------------------------------

    /// A primary and what follows it: members (`.`, `?.`, on a later line too), calls with their arguments and
    /// trailing lambda, type arguments (`listOf<Int>()`), indexing, callable references, `++`, `--`, `!!`.
    fn postfix(&mut self) -> Parsed<Expr> {
        let start = self.start();
        let mut e = self.primary()?;
        loop {
            let t = self.cur();
            if t.is_op(".") || t.is_op("?.") {
                let safe = self.take().text == "?.";
                let name = self.ident("a member's name after `.`")?;
                e = Expr { kind: ExprKind::Member { receiver: Box::new(e), safe, name }, loc: self.loc_from(start) };
                continue;
            }
            if self.nl_before() {
                return Ok(e);
            }
            if t.is_op("::") {
                self.take();
                let name = self.callable_name()?;
                e = Expr {
                    kind: ExprKind::CallableRef { receiver: Some(Box::new(e)), name },
                    loc: self.loc_from(start),
                };
                continue;
            }
            if t.is_op("(") {
                let args = self.value_args()?;
                let trailing_lambda = self.trailing_lambda()?;
                e = self.call(e, Vec::new(), args, trailing_lambda, start);
                continue;
            }
            if self.trailing_lambda_ahead() {
                let trailing_lambda = self.trailing_lambda()?;
                e = self.call(e, Vec::new(), Vec::new(), trailing_lambda, start);
                continue;
            }
            if t.is_op("[") {
                self.take();
                let indices = self.with_newlines(false, |p| {
                    let mut out = Vec::new();
                    while !p.is_op("]") {
                        out.push(p.expression()?);
                        if p.eat_op(",").is_none() {
                            break;
                        }
                    }
                    p.expect_op("]")?;
                    Ok(out)
                })?;
                if indices.is_empty() {
                    let loc = self.loc_from(start);
                    self.error_at(loc, "an index `[…]` takes at least one value");
                }
                e = Expr { kind: ExprKind::Index { receiver: Box::new(e), indices }, loc: self.loc_from(start) };
                continue;
            }
            if t.is_op("<") && matches!(e.kind, ExprKind::Name(_) | ExprKind::Member { .. }) {
                if let Some(type_args) = self.speculate(|p| p.call_type_args()) {
                    if self.is_op("::") {
                        self.take();
                        self.callable_name()?;
                        let loc = self.loc_from(start);
                        self.error_at(
                            loc,
                            "a callable reference on a type with type arguments (`List<Int>::size`) has no place in KompozR's syntax",
                        );
                        e = Expr { kind: ExprKind::Error, loc };
                        continue;
                    }
                    let args = if self.is_op("(") { self.value_args()? } else { Vec::new() };
                    let trailing_lambda = self.trailing_lambda()?;
                    e = self.call(e, type_args, args, trailing_lambda, start);
                    continue;
                }
                return Ok(e);
            }
            let op = match t.text.as_str() {
                _ if t.kind != TokenKind::Op => None,
                "++" => Some(PostfixOp::Inc),
                "--" => Some(PostfixOp::Dec),
                "!!" => Some(PostfixOp::NotNull),
                _ => None,
            };
            let Some(op) = op else {
                return Ok(e);
            };
            self.take();
            e = Expr { kind: ExprKind::Postfix { op, operand: Box::new(e) }, loc: self.loc_from(start) };
        }
    }

    fn call(
        &self,
        callee: Expr,
        type_args: Vec<TypeArg>,
        args: Vec<Arg>,
        trailing_lambda: Option<Box<Lambda>>,
        start: Pos,
    ) -> Expr {
        let mut args = args;
        // Android's resource functions name their parameters as androidx.compose.ui.res does (`painterResource(id =
        // R.drawable.x)`, `pluralStringResource(id, count)`); `R.*` is KompozR's `Res` (Compose Multiplatform's), whose
        // functions call them `resource` and `quantity`.
        let called = match &callee.kind {
            ExprKind::Name(n) => Some(n),
            ExprKind::Member { name, .. } => Some(name),
            _ => None,
        };
        if let Some(n) = called
            && matches!(
                n.name.as_str(),
                "painterResource"
                    | "imageResource"
                    | "vectorResource"
                    | "stringResource"
                    | "pluralStringResource"
                    | "stringArrayResource"
                    | "animatedVectorResource"
            )
        {
            for a in &mut args {
                if let Some(name) = &mut a.name {
                    match name.name.as_str() {
                        "id" => name.name = "resource".into(),
                        "count" if n.name == "pluralStringResource" => name.name = "quantity".into(),
                        _ => {}
                    }
                }
            }
        }
        Expr {
            kind: ExprKind::Call { callee: Box::new(callee), type_args, args, trailing_lambda },
            loc: self.loc_from(start),
        }
    }

    /// `<…>` after a name, as a call's type arguments only when a call (or a callable reference) follows them:
    /// `listOf<Int>()`, `remember<State> { … }`; otherwise `<` is a comparison (`a < b`).
    fn call_type_args(&mut self) -> Parsed<Vec<TypeArg>> {
        let args = self.with_newlines(false, |p| p.type_args())?;
        if !self.nl_before() && (self.is_op("(") || self.is_op("::") || self.trailing_lambda_ahead()) {
            Ok(args)
        } else {
            Err(())
        }
    }

    /// After `::`: a name, or `class`.
    fn callable_name(&mut self) -> Parsed<Ident> {
        if self.is_word("class") {
            return Ok(ident_of(self.take()));
        }
        self.ident("a name after `::`")
    }

    /// Whether a trailing lambda is here (on the call's line): `{`, `lbl@{`, an annotated one.
    fn trailing_lambda_ahead(&self) -> bool {
        if !self.mode.lambdas || self.nl_before() {
            return false;
        }
        self.is_op("{")
            || (self.label_ahead() && self.peek(2).is_op("{"))
            || (self.is_op("@") && self.peek(self.past_annotations(0)).is_op("{"))
    }

    /// The trailing lambda after a call's arguments, if there.
    fn trailing_lambda(&mut self) -> Parsed<Option<Box<Lambda>>> {
        if !self.trailing_lambda_ahead() {
            return Ok(None);
        }
        let start = self.start();
        if self.is_op("@") {
            let annotations = self.annotations()?;
            let loc = annotations[0].loc.to(annotations[annotations.len() - 1].loc);
            self.error_at(loc, "an annotated lambda goes inside the call's parentheses: `f(@Ann { … })`");
        }
        let label = if self.label_ahead() {
            let l = ident_of(self.take());
            self.take();
            Some(l)
        } else {
            None
        };
        Ok(Some(Box::new(self.lambda(label, start)?)))
    }

    /// `(args)`: `value`, `name = value`, `*spread`, a trailing comma allowed.
    pub(super) fn value_args(&mut self) -> Parsed<Vec<Arg>> {
        self.expect_op("(")?;
        self.with_newlines(false, |p| {
            let mut out = Vec::new();
            while !p.is_op(")") {
                let start = p.start();
                let name = if is_name(p.cur()) && p.peek(1).is_op("=") {
                    let n = ident_of(p.take());
                    p.take();
                    Some(n)
                } else {
                    None
                };
                let spread = p.eat_op("*").is_some();
                let value = p.expression()?;
                out.push(Arg { name, spread, value, loc: p.loc_from(start) });
                if p.eat_op(",").is_none() {
                    break;
                }
            }
            p.expect_op(")")?;
            Ok(out)
        })
    }

    // ---- Primaries ---------------------------------------------------------------------------------------------

    /// Whether an expression can start here.
    fn starts_expression(&self) -> bool {
        let t = self.cur();
        match &t.kind {
            TokenKind::Number { .. } | TokenKind::String { .. } | TokenKind::Char(_) => true,
            TokenKind::Ident { .. } => !is_hard_keyword(t) || listed(EXPRESSION_KEYWORDS, &t.text),
            TokenKind::Op => {
                matches!(t.text.as_str(), "(" | "{" | "::" | "-" | "+" | "!" | "!!" | "++" | "--" | "@" | "[")
            }
            TokenKind::Newline | TokenKind::Eof => false,
        }
    }

    /// `@label` right after `return`, `break`, `continue`, `this`, `super` (no space on either side of `@`).
    fn jump_label(&mut self) -> Option<Ident> {
        if self.is_op("@") && self.adjacent(0) && self.peek(1).is_ident() && self.adjacent(1) {
            self.take();
            Some(ident_of(self.take()))
        } else {
            None
        }
    }

    fn primary(&mut self) -> Parsed<Expr> {
        let t = self.cur();
        let start = t.loc.start;
        let done = |p: &Self, kind| Expr { kind, loc: p.loc_from(start) };
        match &t.kind {
            TokenKind::Number { kind, radix } => {
                self.take();
                Ok(done(self, ExprKind::Number(Number { text: t.text.clone(), kind: *kind, radix: *radix })))
            }
            TokenKind::String { raw, pieces } => {
                self.take();
                let lit = self.string_lit(*raw, pieces);
                Ok(done(self, ExprKind::String(lit)))
            }
            TokenKind::Char(c) => {
                self.take();
                Ok(done(self, ExprKind::Char(*c)))
            }
            TokenKind::Ident { backticked: true } => {
                let name = ident_of(self.take());
                Ok(done(self, ExprKind::Name(name)))
            }
            TokenKind::Ident { backticked: false } => self.word(start),
            TokenKind::Op => match t.text.as_str() {
                "(" => {
                    self.take();
                    let inner = self.with_newlines(false, |p| {
                        let e = p.expression()?;
                        p.expect_op(")")?;
                        Ok(e)
                    })?;
                    Ok(done(self, ExprKind::Paren(Box::new(inner))))
                }
                "{" => Ok(Expr { kind: ExprKind::Lambda(self.lambda(None, start)?), loc: self.loc_from(start) }),
                "::" => {
                    self.take();
                    let name = self.callable_name()?;
                    Ok(done(self, ExprKind::CallableRef { receiver: None, name }))
                }
                "[" => {
                    let end = self.at + self.past_brackets(0);
                    while self.at < end && !self.at_eof() {
                        self.take();
                    }
                    let loc = self.loc_from(start);
                    self.error_at(
                        loc,
                        "a collection literal `[…]` is Kotlin's for annotation arguments only, and has no place in KompozR's syntax: write `arrayOf(…)`",
                    );
                    Ok(Expr { kind: ExprKind::Error, loc })
                }
                _ => self.fail(format!("expected an expression{}", self.found())),
            },
            TokenKind::Newline | TokenKind::Eof => self.fail(format!("expected an expression{}", self.found())),
        }
    }

    /// A primary that starts with a word: a keyword's expression, or a name.
    fn word(&mut self, start: Pos) -> Parsed<Expr> {
        let t = self.cur();
        let done = |p: &Self, kind| Expr { kind, loc: p.loc_from(start) };
        match t.text.as_str() {
            "true" | "false" => {
                self.take();
                Ok(done(self, ExprKind::Bool(t.text == "true")))
            }
            "null" => {
                self.take();
                Ok(done(self, ExprKind::Null))
            }
            "this" => {
                self.take();
                let label = self.jump_label();
                Ok(done(self, ExprKind::This { label }))
            }
            "super" => {
                self.take();
                let ty = if self.is_op("<") && self.adjacent(0) {
                    self.expect_op("<")?;
                    let ty = self.with_newlines(false, |p| p.ty())?;
                    self.expect_op(">")?;
                    Some(Box::new(ty))
                } else {
                    None
                };
                let label = self.jump_label();
                Ok(done(self, ExprKind::Super { ty, label }))
            }
            "if" => self.if_expr(start),
            "when" => self.when_expr(start),
            "try" => self.try_expr(start),
            "throw" => {
                self.take();
                let value = self.expression()?;
                Ok(done(self, ExprKind::Throw(Box::new(value))))
            }
            "return" => {
                self.take();
                let label = self.jump_label();
                let value = if self.starts_expression() && !self.nl_before() {
                    Some(Box::new(self.expression()?))
                } else {
                    None
                };
                Ok(done(self, ExprKind::Return { label, value }))
            }
            "break" | "continue" => {
                let is_break = self.take().text == "break";
                let label = self.jump_label();
                Ok(done(self, if is_break { ExprKind::Break { label } } else { ExprKind::Continue { label } }))
            }
            "object" => {
                self.take();
                let supers = self.supers()?;
                if !self.is_op("{") {
                    return self.fail(format!("expected the object's body `{{`{}", self.found()));
                }
                let body = self.class_body()?;
                Ok(done(self, ExprKind::Object { supers, body }))
            }
            "fun" => self.anonymous_function(start, Modifiers::default()),
            "suspend" if self.peek(1).is_word("fun") => {
                let loc = self.take().loc;
                let modifiers = Modifiers { list: vec![Modifier { kind: ModifierKind::Suspend, loc }] };
                self.anonymous_function(start, modifiers)
            }
            _ if is_hard_keyword(t) => self.fail(format!("expected an expression{}", self.found())),
            _ => {
                let name = ident_of(self.take());
                Ok(done(self, ExprKind::Name(name)))
            }
        }
    }

    /// `fun [Receiver.](params)[: Result] [body]` (at `fun`): an anonymous function.
    fn anonymous_function(&mut self, start: Pos, modifiers: Modifiers) -> Parsed<Expr> {
        let fun = self.take();
        let receiver = if self.is_op("(") {
            None
        } else {
            let (receiver, name) = self.receiver_and_name(true)?;
            if let Some(name) = name {
                self.error_at(
                    name.loc,
                    "an anonymous function has no name (a named one is a declaration, a statement of its own)",
                );
                return Err(());
            }
            receiver
        };
        let head = Head { annotations: Vec::new(), modifiers, enum_word: None, annotation_word: None, start };
        let name = Ident { name: String::new(), backticked: false, loc: Loc::at(fun.loc.start) };
        let f = self.function_rest(head, Vec::new(), receiver, name)?;
        Ok(Expr { kind: ExprKind::AnonymousFunction(Box::new(f)), loc: self.loc_from(start) })
    }

    /// `[label@] { [params ->] statements }` (at `{`; `start` where its label starts).
    pub(super) fn lambda(&mut self, label: Option<Ident>, start: Pos) -> Parsed<Lambda> {
        let open = self.expect_op("{")?.loc.start;
        self.with_newlines(true, |p| {
            let params = p.speculate(|p| p.lambda_params());
            let stmts = p.statements();
            p.expect_op("}")?;
            let body = Block { stmts, braced: true, loc: p.loc_from(open) };
            Ok(Lambda { label, params, body, loc: p.loc_from(start) })
        })
    }

    /// A lambda's `a, (b, c), d: T ->` (failing when no `->` ends them: then they are statements).
    fn lambda_params(&mut self) -> Parsed<Vec<LambdaParam>> {
        self.with_newlines(false, |p| {
            let mut out = Vec::new();
            while !p.is_op("->") {
                let start = p.start();
                let annotations = p.annotations()?;
                if let Some(a) = annotations.first() {
                    let loc = a.loc;
                    p.warning_at(loc, "an annotation on a lambda's parameter has no effect here (it is ignored)");
                }
                let binding = if p.is_op("(") {
                    Binding::Destructure(p.destructure()?)
                } else {
                    Binding::Name(p.ident("a parameter")?)
                };
                let ty = if p.eat_op(":").is_some() { Some(p.ty()?) } else { None };
                out.push(LambdaParam { binding, ty, loc: p.loc_from(start) });
                if p.eat_op(",").is_none() {
                    break;
                }
            }
            p.expect_op("->")?;
            Ok(out)
        })
    }

    /// `if (condition) then [else otherwise]` (an empty branch: `;`). An `else` followed by `->` is the next `when`
    /// branch's, not this `if`'s (`1 -> if (a) b` and then `else -> c`).
    fn if_expr(&mut self, start: Pos) -> Parsed<Expr> {
        self.take();
        let condition = self.parenthesized_condition()?;
        let at_else = |p: &Self| p.is_word("else") && !p.peek(1).is_op("->");
        let then = if self.is_op(";") || at_else(self) { self.empty_block() } else { self.control_body()? };
        let save = self.at;
        self.eat_op(";");
        let otherwise = if at_else(self) {
            self.take();
            Some(if self.is_op(";") { self.empty_block() } else { self.control_body()? })
        } else {
            self.at = save;
            None
        };
        Ok(Expr { kind: ExprKind::If { condition: Box::new(condition), then, otherwise }, loc: self.loc_from(start) })
    }

    /// `when [(subject)] { branches }`.
    fn when_expr(&mut self, start: Pos) -> Parsed<Expr> {
        self.take();
        let subject = if self.is_op("(") {
            self.take();
            Some(Box::new(self.with_newlines(false, |p| {
                let annotations = p.annotations()?;
                if let Some(a) = annotations.first() {
                    let loc = a.loc;
                    p.warning_at(loc, "an annotation on a `when`'s subject has no effect here (it is ignored)");
                }
                let binding = if p.eat_word("val").is_some() {
                    let name = p.ident("the subject's name")?;
                    let ty = if p.eat_op(":").is_some() { Some(p.ty()?) } else { None };
                    p.expect_op("=")?;
                    Some((name, ty))
                } else {
                    None
                };
                let expr = p.expression()?;
                p.expect_op(")")?;
                Ok(WhenSubject { binding, expr })
            })?))
        } else {
            None
        };
        self.expect_op("{")?;
        let branches = self.with_newlines(true, |p| {
            let mut out = Vec::new();
            loop {
                while p.eat_op(";").is_some() {}
                if p.is_op("}") {
                    p.take();
                    return Ok(out);
                }
                out.push(p.when_branch()?);
            }
        })?;
        Ok(Expr { kind: ExprKind::When(When { subject, branches }), loc: self.loc_from(start) })
    }

    /// `conditions [if guard] -> body`, `else [if guard] -> body`.
    fn when_branch(&mut self) -> Parsed<WhenBranch> {
        let start = self.start();
        let (conditions, guard) = self.with_newlines(false, |p| p.when_head())?;
        let mode = super::Mode { when_body: true, ..self.mode };
        let body = self.with_mode(mode, |p| p.control_body())?;
        Ok(WhenBranch { conditions, guard, body, loc: self.loc_from(start) })
    }

    /// A branch's conditions (none for `else`), its guard and its `->`.
    fn when_head(&mut self) -> Parsed<(Vec<WhenCondition>, Option<Expr>)> {
        let mut conditions = Vec::new();
        if self.eat_word("else").is_none() {
            loop {
                conditions.push(self.when_condition()?);
                if self.eat_op(",").is_none() || self.is_op("->") || self.is_word("if") {
                    break;
                }
            }
        }
        let guard = if self.eat_word("if").is_some() { Some(self.expression()?) } else { None };
        self.expect_op("->")?;
        Ok((conditions, guard))
    }

    /// In a `when` branch's body, whether the next branch starts here (`in x ->`, `!is T ->`).
    fn when_head_ahead(&mut self) -> bool {
        let (at, mode, diags) = (self.at, self.mode, self.diags.list.len());
        let found = self.with_newlines(false, |p| p.when_head()).is_ok();
        self.at = at;
        self.mode = mode;
        self.diags.list.truncate(diags);
        found
    }

    fn when_condition(&mut self) -> Parsed<WhenCondition> {
        let start = self.start();
        let negated = self.negated_word_ahead("in") || self.negated_word_ahead("is");
        if negated {
            self.take();
        }
        if self.eat_word("in").is_some() {
            let range = self.expression()?;
            return Ok(WhenCondition::In { negated, range, loc: self.loc_from(start) });
        }
        if self.eat_word("is").is_some() {
            let ty = self.ty()?;
            return Ok(WhenCondition::Is { negated, ty, loc: self.loc_from(start) });
        }
        Ok(WhenCondition::Expr(self.expression()?))
    }

    /// `try { … } catch (e: T) { … } … [finally { … }]`.
    fn try_expr(&mut self, start: Pos) -> Parsed<Expr> {
        let word = self.take();
        let body = self.block()?;
        let mut catches = Vec::new();
        while self.is_word("catch") && self.peek(1).is_op("(") {
            let at = self.take().loc.start;
            self.take();
            let (name, ty) = self.with_newlines(false, |p| {
                let annotations = p.annotations()?;
                if let Some(a) = annotations.first() {
                    let loc = a.loc;
                    p.warning_at(loc, "an annotation on a caught exception has no effect here (it is ignored)");
                }
                let name = p.ident("the exception's name")?;
                p.expect_op(":")?;
                let ty = p.ty()?;
                p.eat_op(",");
                p.expect_op(")")?;
                Ok((name, ty))
            })?;
            let body = self.block()?;
            catches.push(Catch { name, ty, body, loc: self.loc_from(at) });
        }
        let finally = if self.eat_word("finally").is_some() { Some(self.block()?) } else { None };
        if catches.is_empty() && finally.is_none() {
            self.error_at(word.loc, "a `try` needs a `catch` or a `finally`");
        }
        Ok(Expr { kind: ExprKind::Try(Try { body, catches, finally }), loc: self.loc_from(start) })
    }

    // ---- Strings -----------------------------------------------------------------------------------------------

    /// A string literal's parts from its pieces: text, `$name`, `${expression}` (parsed from its tokens).
    fn string_lit(&mut self, raw: bool, pieces: &'t [StringPiece]) -> StringLit {
        let mut parts = Vec::new();
        for piece in pieces {
            match piece {
                StringPiece::Text { text, loc } => parts.push(StringPart::Text { text: text.clone(), loc: *loc }),
                StringPiece::Name { name, backticked, loc } => {
                    // (from its `$`)
                    let dollar =
                        Pos { offset: loc.start.offset - 1, line: loc.start.line, column: loc.start.column - 1 };
                    let part_loc = Loc::new(dollar, loc.end);
                    let kind = match name.as_str() {
                        _ if *backticked => ExprKind::Name(Ident { name: name.clone(), backticked: true, loc: *loc }),
                        "this" => ExprKind::This { label: None },
                        "true" | "false" => ExprKind::Bool(name == "true"),
                        "null" => ExprKind::Null,
                        n if listed(HARD_KEYWORDS, n) => {
                            self.error_at(
                                *loc,
                                format!("`{n}` is a keyword: a template names a value (`${{…}}` takes an expression)"),
                            );
                            ExprKind::Error
                        }
                        _ => ExprKind::Name(Ident { name: name.clone(), backticked: false, loc: *loc }),
                    };
                    parts.push(StringPart::Expr { expr: Expr { kind, loc: *loc }, short: true, loc: part_loc });
                }
                StringPiece::Expr { tokens, loc } => {
                    let expr = self.template_expression(tokens, *loc);
                    parts.push(StringPart::Expr { expr, short: false, loc: *loc });
                }
            }
        }
        StringLit { raw, parts }
    }

    /// A `${…}`'s expression from its tokens (ending with `Eof` at its `}`).
    fn template_expression(&mut self, tokens: &'t [Token], loc: Loc) -> Expr {
        let mut sub = Parser::new(tokens, &mut *self.diags);
        let error =
            |p: &Parser| Expr { kind: ExprKind::Error, loc: Loc::new(loc.start, p.cur().loc.end.max(loc.start)) };
        if sub.at_eof() {
            sub.error_at(loc, "an empty template `${}`: it takes an expression");
            return Expr { kind: ExprKind::Error, loc };
        }
        match sub.expression() {
            Ok(e) if sub.at_eof() => e,
            Ok(_) => {
                let _ = sub.fail::<()>(format!("expected `}}` after the template's expression{}", sub.found()));
                error(&sub)
            }
            Err(()) => error(&sub),
        }
    }
}
