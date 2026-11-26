//! Declarations (functions, properties, classes of every kind, objects, type aliases, class members), annotations,
//! modifiers and types.

use super::{Parsed, Parser, ident_of, is_hard_keyword, is_name};
use crate::ast::*;
use crate::lexer::TokenKind;
use crate::pos::{Loc, Pos};

/// What a declaration starts with: its annotations and modifiers (in any order, as Kotlin's), and the words that
/// make a class's kind (`enum`, `annotation`).
pub(super) struct Head {
    pub annotations: Vec<Annotation>,
    pub modifiers: Modifiers,
    pub enum_word: Option<Loc>,
    pub annotation_word: Option<Loc>,
    pub start: Pos,
}

impl Head {
    fn empty(&self) -> bool {
        self.annotations.is_empty()
            && self.modifiers.list.is_empty()
            && self.enum_word.is_none()
            && self.annotation_word.is_none()
    }
}

/// Where a declaration is: a property's accessors are a top-level's or a member's, not a local's.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Ctx {
    TopLevel,
    Member,
    Local,
}

/// Kotlin's annotation use-site targets (`@get:Name`).
const USE_SITE_TARGETS: &[&str] =
    &["field", "property", "get", "set", "receiver", "param", "setparam", "delegate", "file", "all"];

/// A modifier word (or a class kind's: `enum`, `annotation`).
fn is_modifier_word(t: &crate::lexer::Token) -> bool {
    t.kind == (TokenKind::Ident { backticked: false })
        && (ModifierKind::from_keyword(&t.text).is_some() || t.text == "enum" || t.text == "annotation")
}

impl Parser<'_, '_> {
    // ---- Lookahead ---------------------------------------------------------------------------------------------

    /// The offset (from the current token) just past the bracket opened at offset `i` (`(`, `[`, `{`, or `<` counting
    /// only angle brackets); the end's offset when it isn't closed.
    pub(super) fn past_brackets(&self, mut i: usize) -> usize {
        let angle = self.peek(i).is_op("<");
        let mut depth = 0usize;
        loop {
            let t = self.peek(i);
            if t.kind == TokenKind::Eof {
                return i;
            }
            let (open, close) = if angle {
                (t.is_op("<"), t.is_op(">"))
            } else {
                (t.is_op("(") || t.is_op("[") || t.is_op("{"), t.is_op(")") || t.is_op("]") || t.is_op("}"))
            };
            if open {
                depth += 1;
            } else if close {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            } else if angle && !(t.is_ident() || t.is_op(",") || t.is_op(".") || t.is_op("?") || t.is_op("*")) {
                return i;
            }
            i += 1;
        }
    }

    /// The offset past the annotations from offset `i` (`i` itself when none is there).
    pub(super) fn past_annotations(&self, mut i: usize) -> usize {
        while self.peek(i).is_op("@") {
            let mut j = i + 1;
            if USE_SITE_TARGETS.iter().any(|w| self.peek(j).is_word(w)) && self.peek(j + 1).is_op(":") {
                j += 2;
            }
            if self.peek(j).is_op("[") {
                j = self.past_brackets(j);
            } else {
                if !self.peek(j).is_ident() {
                    return i;
                }
                j += 1;
                while self.peek(j).is_op(".") && self.peek(j + 1).is_ident() && self.adjacent(j) {
                    j += 2;
                }
                if self.peek(j).is_op("<") && self.adjacent(j) {
                    j = self.past_brackets(j);
                }
                if self.peek(j).is_op("(") && self.adjacent(j) {
                    j = self.past_brackets(j);
                }
            }
            i = j;
        }
        i
    }

    /// The offset past the annotations and modifiers from offset `i`.
    pub(super) fn past_modifiers(&self, mut i: usize) -> usize {
        loop {
            i = self.past_annotations(i);
            let next = self.peek(i + 1);
            if is_modifier_word(self.peek(i)) && (next.is_ident() || next.is_op("@")) {
                i += 1;
            } else {
                return i;
            }
        }
    }

    /// Whether a local declaration (not an expression) starts here: `val`, `var`, a named `fun`, `class`,
    /// `interface`, `object Name`, `typealias`, after any annotations and modifiers.
    pub(super) fn local_declaration_ahead(&self) -> bool {
        let i = self.past_modifiers(0);
        let t = self.peek(i);
        if t.kind != (TokenKind::Ident { backticked: false }) {
            return false;
        }
        match t.text.as_str() {
            "val" | "var" | "class" | "interface" | "typealias" => true,
            "object" => is_name(self.peek(i + 1)) && !self.nl_at(i + 1),
            "fun" => self.named_function_ahead(i + 1),
            _ => false,
        }
    }

    /// After `fun` (at offset `i`): a named function (not an anonymous one, `fun(x) …`, `fun Int.() …`).
    fn named_function_ahead(&self, mut i: usize) -> bool {
        let t = self.peek(i);
        if t.is_word("interface") || t.is_op("<") {
            return true;
        }
        if t.is_op("@") {
            i = self.past_annotations(i);
        }
        if self.peek(i).is_op("(") {
            // `fun (A).name()`: a parenthesized receiver
            let after = self.past_brackets(i);
            return (self.peek(after).is_op(".") || self.peek(after).is_op("?.")) && self.peek(after + 1).is_ident();
        }
        loop {
            if !self.peek(i).is_ident() {
                return false;
            }
            i += 1;
            if self.peek(i).is_op("<") {
                i = self.past_brackets(i);
            }
            if self.peek(i).is_op("?") {
                i += 1;
            }
            if self.peek(i).is_op(".") || self.peek(i).is_op("?.") {
                if self.peek(i + 1).is_op("(") {
                    return false;
                }
                i += 1;
            } else {
                return true;
            }
        }
    }

    // ---- Annotations and modifiers -----------------------------------------------------------------------------

    pub(super) fn annotations(&mut self) -> Parsed<Vec<Annotation>> {
        let mut out = Vec::new();
        while self.is_op("@") {
            self.annotation_into(&mut out)?;
        }
        Ok(out)
    }

    /// `@Name`, `@a.b.Name<T>(args)`, `@get:Name`, `@file:[A B(1)]` (each of a multi-annotation's with its target).
    pub(super) fn annotation_into(&mut self, out: &mut Vec<Annotation>) -> Parsed<()> {
        let at = self.expect_op("@")?.loc.start;
        let use_site = if USE_SITE_TARGETS.iter().any(|w| self.is_word(w)) && self.peek(1).is_op(":") {
            let target = ident_of(self.take());
            self.take();
            Some(target)
        } else {
            None
        };
        if self.is_op("[") {
            self.take();
            self.with_newlines(false, |p| -> Parsed<()> {
                while !p.is_op("]") {
                    let start = p.start();
                    let mut a = p.unescaped_annotation(start, use_site.clone())?;
                    a.loc = p.loc_from(at);
                    out.push(a);
                    p.eat_op(",");
                }
                p.take();
                Ok(())
            })?;
            return Ok(());
        }
        let a = self.unescaped_annotation(at, use_site)?;
        out.push(a);
        Ok(())
    }

    /// An annotation's name, type arguments and arguments (from `start`).
    fn unescaped_annotation(&mut self, start: Pos, use_site: Option<Ident>) -> Parsed<Annotation> {
        let first = self.ident("an annotation's name")?;
        let mut segments = vec![first];
        while self.is_op(".") && self.adjacent(0) && is_name(self.peek(1)) {
            self.take();
            segments.push(ident_of(self.take()));
        }
        let name = Path { loc: segments[0].loc.to(segments[segments.len() - 1].loc), segments };
        let type_args = if self.is_op("<") && self.adjacent(0) {
            self.with_newlines(false, |p| p.type_args())?
        } else {
            Vec::new()
        };
        let args = if self.is_op("(") && self.adjacent(0) { Some(self.value_args()?) } else { None };
        Ok(Annotation { use_site, name, type_args, args, loc: self.loc_from(start) })
    }

    /// The annotations and modifiers a declaration (or parameter, accessor, constructor) starts with.
    pub(super) fn head(&mut self) -> Parsed<Head> {
        let start = self.start();
        let mut head = Head {
            annotations: Vec::new(),
            modifiers: Modifiers::default(),
            enum_word: None,
            annotation_word: None,
            start,
        };
        loop {
            if self.is_op("@") {
                self.annotation_into(&mut head.annotations)?;
                continue;
            }
            let next = self.peek(1);
            if !(is_modifier_word(self.cur()) && (next.is_ident() || next.is_op("@"))) {
                return Ok(head);
            }
            let t = self.take();
            let slot = match t.text.as_str() {
                "enum" => Some(&mut head.enum_word),
                "annotation" => Some(&mut head.annotation_word),
                _ => None,
            };
            let duplicate = match slot {
                Some(slot) => slot.replace(t.loc).is_some(),
                None => {
                    let kind = ModifierKind::from_keyword(&t.text).expect("a modifier word");
                    let dup = head.modifiers.has(kind);
                    head.modifiers.list.push(Modifier { kind, loc: t.loc });
                    dup
                }
            };
            if duplicate {
                self.error_at(t.loc, format!("`{}` is written twice", t.text));
            }
        }
    }

    // ---- Declarations ------------------------------------------------------------------------------------------

    pub(super) fn top_level_declaration(&mut self) -> Parsed<Decl> {
        let head = self.head()?;
        self.declaration(head, Ctx::TopLevel)
    }

    /// A local declaration (`local_declaration_ahead` said one is here).
    pub(super) fn local_declaration(&mut self) -> Parsed<Decl> {
        let head = self.head()?;
        self.declaration(head, Ctx::Local)
    }

    fn declaration(&mut self, head: Head, ctx: Ctx) -> Parsed<Decl> {
        let t = self.cur();
        let kind_word = |p: &mut Self, what: &str| {
            if let Some(loc) = head.enum_word.or(head.annotation_word) {
                p.error_at(loc, format!("`enum` and `annotation` make a class, not {what}"));
            }
        };
        if t.kind != (TokenKind::Ident { backticked: false }) {
            return self.fail(format!("expected a declaration (`fun`, `val`, `class`, …){}", self.found()));
        }
        if t.is_word("context") && self.peek(1).is_op("(") {
            return self
                .fail("context parameters (`context(…)`, experimental in Kotlin) have no place in KompozR's syntax");
        }
        match t.text.as_str() {
            "fun" if self.peek(1).is_word("interface") => {
                kind_word(self, "a fun interface");
                self.take();
                Ok(Decl::Class(Box::new(self.class(head, ClassKind::FunInterface)?)))
            }
            "fun" => {
                kind_word(self, "a function");
                Ok(Decl::Function(Box::new(self.function(head)?)))
            }
            "val" | "var" => {
                kind_word(self, "a property");
                Ok(Decl::Property(Box::new(self.property(head, ctx)?)))
            }
            "class" => {
                let kind = match (head.enum_word, head.annotation_word) {
                    (Some(_), Some(loc)) => {
                        self.error_at(loc, "a class is an enum class or an annotation class, not both");
                        ClassKind::Enum
                    }
                    (Some(_), None) => ClassKind::Enum,
                    (None, Some(_)) => ClassKind::Annotation,
                    (None, None) => ClassKind::Class,
                };
                Ok(Decl::Class(Box::new(self.class(head, kind)?)))
            }
            "interface" => {
                kind_word(self, "an interface");
                Ok(Decl::Class(Box::new(self.class(head, ClassKind::Interface)?)))
            }
            "object" => {
                kind_word(self, "an object");
                Ok(Decl::Object(Box::new(self.object(head)?)))
            }
            "typealias" => {
                kind_word(self, "a type alias");
                Ok(Decl::TypeAlias(Box::new(self.type_alias(head)?)))
            }
            _ => self.fail(format!("expected a declaration (`fun`, `val`, `class`, …){}", self.found())),
        }
    }

    /// `fun [<T>] [Receiver.]name(params)[: Result] [where …] [body]` (at `fun`).
    fn function(&mut self, head: Head) -> Parsed<Function> {
        self.take();
        let type_params = if self.is_op("<") { self.type_params()? } else { Vec::new() };
        let (receiver, name) = self.receiver_and_name(true)?;
        let Some(name) = name else {
            return self.fail(format!("expected the function's name{}", self.found()));
        };
        self.named(&name);
        self.function_rest(head, type_params, receiver, name)
    }

    /// A function's parameters, result, constraints and body (its name read).
    pub(super) fn function_rest(
        &mut self,
        head: Head,
        type_params: Vec<TypeParam>,
        receiver: Option<Type>,
        name: Ident,
    ) -> Parsed<Function> {
        let anonymous = name.name.is_empty();
        if !self.is_op("(") {
            return self.fail(format!("expected the function's parameters `(`{}", self.found()));
        }
        let params = self.value_params(!anonymous)?;
        let result = if self.eat_op(":").is_some() { Some(self.ty()?) } else { None };
        let constraints = self.constraints()?;
        let body = self.function_body()?;
        Ok(Function {
            annotations: head.annotations,
            modifiers: head.modifiers,
            type_params,
            receiver,
            name,
            params,
            result,
            constraints,
            body,
            loc: self.loc_from(head.start),
        })
    }

    /// `{ … }`, `= expression`, or none.
    fn function_body(&mut self) -> Parsed<Option<FunctionBody>> {
        if self.is_op("{") {
            return Ok(Some(FunctionBody::Block(self.block()?)));
        }
        if self.eat_op("=").is_some() {
            return Ok(Some(FunctionBody::Expr(self.with_newlines(true, |p| p.expression())?)));
        }
        Ok(None)
    }

    /// A function's or property's receiver (if it has one) and name: `name`, `Receiver.name`, `a.b.C<T>.name`,
    /// `String?.name`, `(A.() -> Unit).name`. For a function (`function`), no name after the receiver's `.` when a
    /// `(` follows it: an anonymous function's `fun Int.(x: Int)`.
    pub(super) fn receiver_and_name(&mut self, function: bool) -> Parsed<(Option<Type>, Option<Ident>)> {
        let start = self.start();
        let annotations = if self.is_op("@") { self.annotations()? } else { Vec::new() };
        let mut receiver = if self.is_op("(") {
            self.type_atom()?
        } else {
            let mut segments = Vec::new();
            loop {
                let id = self.ident(if segments.is_empty() { "a name" } else { "a name after `.`" })?;
                let args = if self.is_op("<") { self.with_newlines(false, |p| p.type_args())? } else { Vec::new() };
                segments.push(TypeSegment { name: id, args });
                // (a `.` and a name the receiver goes on after; the last name is the declaration's)
                if self.is_op(".") && is_name(self.peek(1)) && self.continues_receiver(2) {
                    self.take();
                    continue;
                }
                break;
            }
            let dotted = self.is_op(".") || self.is_op("?.") || (self.is_op("?") && self.peek(1).is_op("."));
            if !dotted {
                let last = segments.pop().expect("a name");
                if !last.args.is_empty() {
                    let loc = last.name.loc;
                    self.error_at(
                        loc,
                        "a declaration's name takes no type arguments (they go after `fun`/`val`: `fun <T> name`)",
                    );
                }
                if segments.is_empty() {
                    if let Some(a) = annotations.first() {
                        let loc = a.loc;
                        self.error_at(loc, "an annotation here goes before the declaration's keyword");
                    }
                    return Ok((None, Some(last.name)));
                }
                let loc = Loc::new(start, segments[segments.len() - 1].name.loc.end);
                let ty = Type { kind: TypeKind::Named(segments), nullable: false, annotations, loc };
                return Ok((Some(ty), Some(last.name)));
            }
            Type {
                kind: TypeKind::Named(segments),
                nullable: false,
                annotations: Vec::new(),
                loc: self.loc_from(start),
            }
        };
        // the receiver's `.` (`?.` for a nullable one)
        if self.eat_op("?.").is_some() {
            receiver.nullable = true;
        } else if self.is_op("?") && self.peek(1).is_op(".") {
            self.take();
            receiver.nullable = true;
            receiver.loc = self.loc_from(start);
            self.take();
        } else {
            self.expect_op(".")?;
        }
        receiver.annotations.splice(0..0, annotations);
        receiver.loc = Loc::new(start, receiver.loc.end);
        if function && self.is_op("(") {
            return Ok((Some(receiver), None));
        }
        let name = self.ident("the name after the receiver")?;
        Ok((Some(receiver), Some(name)))
    }

    /// Whether, past a receiver's `.` and the name after it (that name at offset `i - 1`), the receiver goes on:
    /// another `.`/`?.`, type arguments or a nullable `?` follow that name.
    fn continues_receiver(&self, i: usize) -> bool {
        let t = self.peek(i);
        t.is_op(".")
            || t.is_op("?.")
            || (t.is_op("<") && self.adjacent(i))
            || (t.is_op("?") && self.peek(i + 1).is_op("."))
    }

    /// `(params)` of a function (`typed`: each with its type; an anonymous function's may leave it out).
    fn value_params(&mut self, typed: bool) -> Parsed<Vec<Param>> {
        self.expect_op("(")?;
        self.with_newlines(false, |p| {
            let mut out = Vec::new();
            while !p.is_op(")") {
                let head = p.head()?;
                if p.is_word("val") || p.is_word("var") {
                    let loc = p.take().loc;
                    p.error_at(loc, "a function's parameter is no property: `val`/`var` are a class constructor's");
                }
                out.push(p.param(head, typed)?);
                if p.eat_op(",").is_none() {
                    break;
                }
            }
            p.expect_op(")")?;
            Ok(out)
        })
    }

    /// `name[: Type] [= default]` after its head.
    fn param(&mut self, head: Head, typed: bool) -> Parsed<Param> {
        let name = self.ident("a parameter's name")?;
        let ty = if self.eat_op(":").is_some() {
            Some(self.ty()?)
        } else if typed {
            return self.fail(format!("expected `:` and the parameter's type{}", self.found()));
        } else {
            None
        };
        let default = if self.eat_op("=").is_some() { Some(self.expression()?) } else { None };
        if let Some(loc) = head.enum_word.or(head.annotation_word) {
            self.error_at(loc, "`enum` and `annotation` make a class, not a parameter");
        }
        Ok(Param {
            annotations: head.annotations,
            modifiers: head.modifiers,
            name,
            ty,
            default,
            read_by_default: false,
            loc: self.loc_from(head.start),
        })
    }

    /// `val|var [<T>] [Receiver.]name[: Type] [where …] [= value | by delegate] [accessors]`, or `val (a, b) = …`
    /// (at `val`/`var`).
    fn property(&mut self, head: Head, ctx: Ctx) -> Parsed<Property> {
        let mutability = if self.take().text == "var" { Mutability::Var } else { Mutability::Val };
        let type_params = if self.is_op("<") { self.type_params()? } else { Vec::new() };
        let (receiver, binding) = if self.is_op("(") && !self.parenthesized_receiver_ahead() {
            (None, Binding::Destructure(self.destructure()?))
        } else {
            let (receiver, name) = self.receiver_and_name(false)?;
            let name = name.expect("a property's name");
            self.named(&name);
            (receiver, Binding::Name(name))
        };
        let ty = if self.eat_op(":").is_some() { Some(self.ty()?) } else { None };
        let constraints = self.constraints()?;
        let init = if self.eat_op("=").is_some() {
            PropertyInit::Value(self.expression()?)
        } else if self.eat_word("by").is_some() {
            PropertyInit::Delegate(self.expression()?)
        } else {
            PropertyInit::None
        };
        let (mut getter, mut setter) = (None, None);
        if ctx != Ctx::Local {
            self.accessors(&mut getter, &mut setter)?;
        }
        Ok(Property {
            annotations: head.annotations,
            modifiers: head.modifiers,
            mutability,
            type_params,
            receiver,
            binding,
            ty,
            constraints,
            init,
            getter,
            setter,
            loc: self.loc_from(head.start),
        })
    }

    /// At `(` after `val`: whether it is a parenthesized receiver (`val (A.() -> Unit).name`) rather than a
    /// destructuring.
    fn parenthesized_receiver_ahead(&self) -> bool {
        let after = self.past_brackets(0);
        let t = self.peek(after);
        t.is_op(".") || t.is_op("?.") || (t.is_op("?") && self.adjacent(after))
    }

    /// `(a, _, b: T)`: a destructuring's names.
    pub(super) fn destructure(&mut self) -> Parsed<Vec<DestructureEntry>> {
        self.expect_op("(")?;
        self.with_newlines(false, |p| {
            let mut out = Vec::new();
            while !p.is_op(")") {
                let start = p.start();
                let annotations = p.annotations()?;
                if let Some(a) = annotations.first() {
                    let loc = a.loc;
                    p.warning_at(loc, "an annotation on a destructured name has no effect here (it is ignored)");
                }
                let name = p.ident("a destructured name")?;
                let ty = if p.eat_op(":").is_some() { Some(p.ty()?) } else { None };
                out.push(DestructureEntry { name, ty, loc: p.loc_from(start) });
                if p.eat_op(",").is_none() {
                    break;
                }
            }
            p.expect_op(")")?;
            if out.is_empty() {
                let loc = p.loc_from(p.prev_end());
                p.error_at(loc, "a destructuring names at least one value");
            }
            Ok(out)
        })
    }

    /// Whether a getter or setter starts here: `[annotations] [modifiers] get|set` (no other declaration starts
    /// with those words; one alone, `private set`, may have the next member after it on its line, as members need
    /// no separator).
    fn accessor_ahead(&self) -> bool {
        let i = self.past_modifiers(0);
        let t = self.peek(i);
        t.is_word("get") || t.is_word("set")
    }

    /// A property's `get`/`set`, in either order, each on the property's line or a line of its own.
    fn accessors(&mut self, getter: &mut Option<Accessor>, setter: &mut Option<Accessor>) -> Parsed<()> {
        for _ in 0..2 {
            let save = self.at;
            self.eat_op(";");
            if !self.accessor_ahead() {
                self.at = save;
                return Ok(());
            }
            let head = self.head()?;
            let word = self.take();
            let is_get = word.text == "get";
            let (mut param, mut ty, mut body) = (None, None, None);
            if self.is_op("(") {
                self.take();
                self.with_newlines(false, |p| -> Parsed<()> {
                    if !is_get {
                        let h = p.head()?;
                        param = Some(p.param(h, false)?);
                        p.eat_op(",");
                    }
                    p.expect_op(")")?;
                    Ok(())
                })?;
                if self.eat_op(":").is_some() {
                    ty = Some(self.ty()?);
                }
                body = self.function_body()?;
                if body.is_none() {
                    let what = if is_get { "getter" } else { "setter" };
                    return self.fail(format!("expected the {what}'s body (`= …` or `{{ … }}`){}", self.found()));
                }
            }
            let accessor = Accessor {
                annotations: head.annotations,
                modifiers: head.modifiers,
                param,
                ty,
                body,
                loc: self.loc_from(head.start),
            };
            let slot = if is_get { &mut *getter } else { &mut *setter };
            if slot.replace(accessor).is_some() {
                self.error_at(word.loc, format!("the property has `{}` twice", word.text));
            }
        }
        Ok(())
    }

    /// `class`/`interface` (at the keyword; `fun` of a fun interface taken).
    fn class(&mut self, head: Head, kind: ClassKind) -> Parsed<Class> {
        self.take();
        let name = self.ident("the class's name")?;
        self.named(&name);
        let type_params = if self.is_op("<") { self.type_params()? } else { Vec::new() };
        let primary_constructor = self.primary_constructor()?;
        let supers = self.supers()?;
        let constraints = self.constraints()?;
        let (mut enum_entries, mut body) = (Vec::new(), ClassBody::default());
        if self.is_op("{") {
            if kind == ClassKind::Enum {
                (enum_entries, body) = self.enum_body()?;
            } else {
                body = self.class_body()?;
            }
        }
        Ok(Class {
            annotations: head.annotations,
            modifiers: head.modifiers,
            kind,
            name,
            type_params,
            primary_constructor,
            supers,
            constraints,
            enum_entries,
            body,
            loc: self.loc_from(head.start),
        })
    }

    /// `[annotations] [modifiers] [constructor](params)` after a class's name, if there.
    fn primary_constructor(&mut self) -> Parsed<Option<PrimaryConstructor>> {
        let explicit = {
            let i = self.past_modifiers(0);
            self.peek(i).is_word("constructor") && self.peek(i + 1).is_op("(")
        };
        if !explicit && !self.is_op("(") {
            return Ok(None);
        }
        let head = self.head()?;
        if explicit {
            self.expect_word("constructor")?;
        }
        self.expect_op("(")?;
        let params = self.with_newlines(false, |p| {
            let mut out = Vec::new();
            while !p.is_op(")") {
                let head = p.head()?;
                let property = if p.eat_word("val").is_some() {
                    Some(Mutability::Val)
                } else if p.eat_word("var").is_some() {
                    Some(Mutability::Var)
                } else {
                    None
                };
                out.push(ConstructorParam { property, param: p.param(head, true)? });
                if p.eat_op(",").is_none() {
                    break;
                }
            }
            p.expect_op(")")?;
            Ok(out)
        })?;
        Ok(Some(PrimaryConstructor {
            annotations: head.annotations,
            modifiers: head.modifiers,
            params,
            loc: self.loc_from(head.start),
        }))
    }

    /// `: A(args), B, C by delegate` (none without the `:`).
    pub(super) fn supers(&mut self) -> Parsed<Vec<SuperType>> {
        if self.eat_op(":").is_none() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        loop {
            let start = self.start();
            let ty = self.ty()?;
            let mut constructor_args = None;
            let mut delegate = None;
            if self.is_op("(") && !self.nl_before() {
                constructor_args = Some(self.value_args()?);
            } else if self.eat_word("by").is_some() {
                // (no trailing lambda: a `{` after it is the class's body)
                let mode = super::Mode { lambdas: false, ..self.mode };
                delegate = Some(self.with_mode(mode, |p| p.expression())?);
            }
            out.push(SuperType { ty, constructor_args, delegate, loc: self.loc_from(start) });
            if self.eat_op(",").is_none() {
                return Ok(out);
            }
        }
    }

    /// `{ members }`.
    pub(super) fn class_body(&mut self) -> Parsed<ClassBody> {
        let open = self.expect_op("{")?.loc.start;
        self.with_newlines(true, |p| {
            let members = p.members();
            p.expect_op("}")?;
            Ok(ClassBody { members, loc: Some(p.loc_from(open)) })
        })
    }

    /// A body's members up to its `}` (not taken), each failed one `Decl::Failed`.
    fn members(&mut self) -> Vec<Member> {
        let mut out = Vec::new();
        loop {
            while self.eat_op(";").is_some() {}
            if self.is_op("}") || self.at_eof() {
                return out;
            }
            let m = self.recovering(true, |p| p.member(), |name, loc| Member::Decl(Decl::Failed { name, loc }));
            out.push(m);
        }
    }

    /// A class body's member: `init { … }`, a secondary constructor, a declaration (the companion object too).
    fn member(&mut self) -> Parsed<Member> {
        if self.is_word("init") && self.peek(1).is_op("{") {
            self.take();
            return Ok(Member::Init(self.block()?));
        }
        let head = self.head()?;
        if self.is_word("constructor") {
            return Ok(Member::Constructor(Box::new(self.secondary_constructor(head)?)));
        }
        if self.is_word("init") && !head.empty() {
            return self.fail("an `init` block has no annotations or modifiers");
        }
        Ok(Member::Decl(self.declaration(head, Ctx::Member)?))
    }

    /// `constructor(params) [: this(args) | super(args)] [{ … }]` (at `constructor`).
    fn secondary_constructor(&mut self, head: Head) -> Parsed<SecondaryConstructor> {
        self.take();
        let params = self.value_params(true)?;
        let delegation = if self.eat_op(":").is_some() {
            let start = self.start();
            let to_super = if self.eat_word("super").is_some() {
                true
            } else if self.eat_word("this").is_some() {
                false
            } else {
                return self.fail(format!("expected `this(…)` or `super(…)`{}", self.found()));
            };
            if !self.is_op("(") {
                return self.fail(format!("expected `(`{}", self.found()));
            }
            let args = self.value_args()?;
            Some(ConstructorDelegation { to_super, args, loc: self.loc_from(start) })
        } else {
            None
        };
        let body = if self.is_op("{") { Some(self.block()?) } else { None };
        Ok(SecondaryConstructor {
            annotations: head.annotations,
            modifiers: head.modifiers,
            params,
            delegation,
            body,
            loc: self.loc_from(head.start),
        })
    }

    /// An enum class's `{ entries [; members] }`.
    fn enum_body(&mut self) -> Parsed<(Vec<EnumEntry>, ClassBody)> {
        let open = self.expect_op("{")?.loc.start;
        self.with_newlines(true, |p| {
            let mut entries = Vec::new();
            // (after the first entry, what follows a `,` is the next one, or the entries' end)
            if p.enum_entry_ahead() {
                loop {
                    entries.push(p.enum_entry()?);
                    if p.eat_op(",").is_none() || p.is_op(";") || p.is_op("}") {
                        break;
                    }
                }
            }
            let mut members = Vec::new();
            if p.eat_op(";").is_some() {
                members = p.members();
            } else if !p.is_op("}") {
                let what = if entries.is_empty() { "an entry" } else { "`,`, `;` or `}`" };
                return p.fail(format!("expected {what} (members come after the entries and a `;`){}", p.found()));
            }
            p.expect_op("}")?;
            Ok((entries, ClassBody { members, loc: Some(p.loc_from(open)) }))
        })
    }

    /// Whether an enum entry starts here: `[annotations] NAME` and then `,`, `;`, `}`, `(`, `{` or a new line.
    fn enum_entry_ahead(&self) -> bool {
        let i = self.past_annotations(0);
        let t = self.peek(i);
        if !is_name(t) || is_modifier_word(t) && self.peek(i + 1).is_ident() && !self.nl_at(i + 1) {
            return false;
        }
        let next = self.peek(i + 1);
        [",", ";", "}", "(", "{"].iter().any(|op| next.is_op(op)) || self.nl_at(i + 1) || next.kind == TokenKind::Eof
    }

    /// `[annotations] NAME[(args)] [{ body }]`.
    fn enum_entry(&mut self) -> Parsed<EnumEntry> {
        let start = self.start();
        let annotations = self.annotations()?;
        let name = self.ident("an enum entry's name")?;
        let args = if self.is_op("(") { self.value_args()? } else { Vec::new() };
        let body = if self.is_op("{") { Some(self.class_body()?) } else { None };
        Ok(EnumEntry { annotations, name, args, body, loc: self.loc_from(start) })
    }

    /// `object [Name] [: supers] [{ members }]` (at `object`; a companion's name may be left out).
    fn object(&mut self, head: Head) -> Parsed<Object> {
        self.take();
        let name = if is_name(self.cur()) && !self.toks[self.at].nl {
            let n = ident_of(self.take());
            self.named(&n);
            Some(n)
        } else if head.modifiers.has(ModifierKind::Companion) {
            None
        } else {
            return self.fail(format!("expected the object's name{}", self.found()));
        };
        let supers = self.supers()?;
        let body = if self.is_op("{") { self.class_body()? } else { ClassBody::default() };
        Ok(Object {
            annotations: head.annotations,
            modifiers: head.modifiers,
            name,
            supers,
            body,
            loc: self.loc_from(head.start),
        })
    }

    /// `typealias Name[<T>] = Type` (at `typealias`).
    fn type_alias(&mut self, head: Head) -> Parsed<TypeAlias> {
        self.take();
        let name = self.ident("the type alias's name")?;
        self.named(&name);
        let type_params = if self.is_op("<") { self.type_params()? } else { Vec::new() };
        self.expect_op("=")?;
        let ty = self.ty()?;
        Ok(TypeAlias {
            annotations: head.annotations,
            modifiers: head.modifiers,
            name,
            type_params,
            ty,
            loc: self.loc_from(head.start),
        })
    }

    // ---- Types -------------------------------------------------------------------------------------------------

    /// `<T, out R : Bound, reified E>`.
    fn type_params(&mut self) -> Parsed<Vec<TypeParam>> {
        self.expect_op("<")?;
        self.with_newlines(false, |p| {
            let mut out = Vec::new();
            while !p.is_op(">") {
                let start = p.start();
                let mut annotations = Vec::new();
                let (mut variance, mut reified) = (None, false);
                loop {
                    if p.is_op("@") {
                        p.annotation_into(&mut annotations)?;
                    } else if p.is_word("in") {
                        p.take();
                        variance = Some(Variance::In);
                    } else if (p.is_word("out") || p.is_word("reified")) && p.peek(1).is_ident() {
                        if p.take().text == "out" {
                            variance = Some(Variance::Out);
                        } else {
                            reified = true;
                        }
                    } else {
                        break;
                    }
                }
                let name = p.ident("a type parameter's name")?;
                let bound = if p.eat_op(":").is_some() { Some(p.ty()?) } else { None };
                out.push(TypeParam { annotations, variance, reified, name, bound, loc: p.loc_from(start) });
                if p.eat_op(",").is_none() {
                    break;
                }
            }
            p.expect_op(">")?;
            Ok(out)
        })
    }

    /// `where T : A, T : B` (none without `where`).
    fn constraints(&mut self) -> Parsed<Vec<TypeConstraint>> {
        if self.eat_word("where").is_none() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        loop {
            let start = self.start();
            let annotations = self.annotations()?;
            if let Some(a) = annotations.first() {
                let loc = a.loc;
                self.warning_at(loc, "an annotation on a type constraint has no effect here (it is ignored)");
            }
            let name = self.ident("a type parameter's name")?;
            self.expect_op(":")?;
            let bound = self.ty()?;
            out.push(TypeConstraint { name, bound, loc: self.loc_from(start) });
            if self.eat_op(",").is_none() {
                return Ok(out);
            }
        }
    }

    /// A type: `Int`, `String?`, `List<Item>`, `Map.Entry<K, V>`, `(String) -> Unit`, `@Kompozable () -> Unit`,
    /// `RowScope.() -> Unit`, `suspend () -> Unit`, `(() -> Unit)?`.
    pub(super) fn ty(&mut self) -> Parsed<Type> {
        let start = self.start();
        let annotations = self.annotations()?;
        let suspend = if self.is_word("suspend") && {
            let n = self.peek(1);
            n.is_op("(") || n.is_ident() || n.is_op("@")
        } {
            Some(self.take().loc)
        } else {
            None
        };
        let mut t = if suspend.is_some() && self.is_op("@") {
            // `suspend @Ann () -> Unit`
            self.ty()?
        } else {
            self.type_atom()?
        };
        // `A.() -> B`, `A?.() -> B`: a function type with a receiver
        let receiver_dot = (self.is_op(".") || self.is_op("?.")) && self.peek(1).is_op("(")
            || (self.is_op("?") && self.peek(1).is_op(".") && self.peek(2).is_op("("));
        if receiver_dot && !self.nl_before() {
            if self.eat_op("?").is_some() || self.is_op("?.") {
                t.nullable = true;
            }
            self.take();
            let receiver = t;
            let params = self.function_type_params()?;
            self.expect_op("->")?;
            let result = self.ty()?;
            t = Type {
                kind: TypeKind::Function(FunctionType {
                    suspend: false,
                    receiver: Some(Box::new(receiver)),
                    params,
                    result: Box::new(result),
                }),
                nullable: false,
                annotations: Vec::new(),
                loc: self.loc_from(start),
            };
        }
        if self.is_op("&") && !self.nl_before() {
            self.take();
            self.ty()?;
            self.error_at(
                Loc::new(start, self.prev_end()),
                "a definitely non-nullable type (`T & Any`) has no place in KompozR's syntax: write `T` with a non-null bound",
            );
            t = Type { kind: TypeKind::Error, nullable: false, annotations: Vec::new(), loc: self.loc_from(start) };
        }
        if let Some(loc) = suspend {
            match &mut t.kind {
                TypeKind::Function(f) => f.suspend = true,
                _ => self.error_at(loc, "`suspend` goes on a function type"),
            }
        }
        t.annotations.splice(0..0, annotations);
        t.loc = self.loc_from(start);
        Ok(t)
    }

    /// A named type with its `?`s, or one in parentheses: a function type (`(A, b: B) -> R`) or a type grouped
    /// (`(() -> Unit)?`).
    pub(super) fn type_atom(&mut self) -> Parsed<Type> {
        let start = self.start();
        if self.is_op("(") {
            let params = self.function_type_params()?;
            if self.eat_op("->").is_some() {
                let result = self.ty()?;
                return Ok(Type {
                    kind: TypeKind::Function(FunctionType {
                        suspend: false,
                        receiver: None,
                        params,
                        result: Box::new(result),
                    }),
                    nullable: false,
                    annotations: Vec::new(),
                    loc: self.loc_from(start),
                });
            }
            let mut params = params;
            if params.len() != 1 || params[0].name.is_some() {
                return self.fail(format!("expected `->` after a function type's parameters{}", self.found()));
            }
            let mut inner = params.pop().expect("one type").ty;
            while self.is_op("?") && !self.nl_before() {
                self.take();
                inner.nullable = true;
            }
            inner.loc = self.loc_from(start);
            return Ok(inner);
        }
        if !is_name(self.cur()) {
            return self.fail(format!("expected a type{}", self.found()));
        }
        let mut segments = Vec::new();
        loop {
            let name = ident_of(self.take());
            let args = if self.is_op("<") && !self.nl_before() {
                self.with_newlines(false, |p| p.type_args())?
            } else {
                Vec::new()
            };
            segments.push(TypeSegment { name, args });
            if self.is_op(".") && !self.nl_before() && is_name(self.peek(1)) {
                self.take();
                continue;
            }
            break;
        }
        let mut t =
            Type { kind: TypeKind::Named(segments), nullable: false, annotations: Vec::new(), loc: Loc::at(start) };
        while self.is_op("?") && !self.nl_before() {
            self.take();
            t.nullable = true;
        }
        t.loc = self.loc_from(start);
        Ok(t)
    }

    /// A function type's `(A, b: B)`.
    fn function_type_params(&mut self) -> Parsed<Vec<FunctionTypeParam>> {
        self.expect_op("(")?;
        self.with_newlines(false, |p| {
            let mut out = Vec::new();
            while !p.is_op(")") {
                let name = if is_name(p.cur()) && p.peek(1).is_op(":") {
                    let n = ident_of(p.take());
                    p.take();
                    Some(n)
                } else {
                    None
                };
                out.push(FunctionTypeParam { name, ty: p.ty()? });
                if p.eat_op(",").is_none() {
                    break;
                }
            }
            p.expect_op(")")?;
            Ok(out)
        })
    }

    /// `<A, *, out B, in C>` (at `<`).
    pub(super) fn type_args(&mut self) -> Parsed<Vec<TypeArg>> {
        self.expect_op("<")?;
        let mut out = Vec::new();
        while !self.is_op(">") {
            if let Some(star) = self.eat_op("*") {
                out.push(TypeArg::Star(star.loc));
            } else {
                let variance = if self.is_word("in") {
                    self.take();
                    Some(Variance::In)
                } else if self.is_word("out") && {
                    let n = self.peek(1);
                    (n.is_ident() && !is_hard_keyword(n)) || n.is_op("(") || n.is_op("@")
                } {
                    self.take();
                    Some(Variance::Out)
                } else {
                    None
                };
                out.push(TypeArg::Type { variance, ty: self.ty()? });
            }
            if self.eat_op(",").is_none() {
                break;
            }
        }
        self.expect_op(">")?;
        Ok(out)
    }
}
