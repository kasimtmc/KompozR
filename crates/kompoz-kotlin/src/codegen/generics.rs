//! Kotlin's generic classes and functions (stage 14 WPA): their type parameters as Rust's, each bounded by what its
//! values need here — `Clone + 'static` always (Kotlin's values are shared references: a class's cells and getters
//! clone, its objects are `Any`), `kt::KtToString` where a value of it is shown (a string template, a data class's
//! `toString()`), `PartialEq` where one is compared (`==`, a data class's `equals`, a composable's parameters, which
//! it compares to skip), and the bound written (`<T : Comparable<T>>`).
//!
//! | Kotlin | Rust |
//! |---|---|
//! | `class Crate<T>(val value: T) { fun describe() = "Crate($value)" }` | `pub struct Crate<T> { … }`, `impl<T: Clone + 'static + kt::KtToString> Crate<T> { … }`, `fn Crate<T: …>(value: T) -> Crate<T>` |
//! | `fun <T, R> mapAll(items: List<T>, f: (T) -> R): List<R>` | `fn map_all<T: Clone + 'static, R: Clone + 'static>(items: Vec<T>, f: impl Fn(T) -> R + 'static) -> Vec<R>` |
//! | `fun <T> List<T>.second(): T` | `pub trait ListSecondExt<T> { fn second(self) -> T; }` implemented for `Vec<T>` |
//! | `Crate<String>("b")`, `emptyBag<Int>()` | `Crate::<String>(…)`, `empty_bag::<i32>()` (the type arguments written) |

use super::Gen;
use super::names::ident;
use super::types::TypePos;
use crate::ast::*;
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};

/// What a declaration does with its type parameters' values (by name).
#[derive(Clone, Debug, Default)]
pub(crate) struct Uses {
    /// The type parameters whose values are shown (written as text).
    pub shown: Vec<String>,
    /// Those whose values are compared.
    pub compared: Vec<String>,
    /// Every type parameter is shown and compared (a data class's `toString()` and `equals`, a composable's
    /// parameters compared).
    pub all: bool,
}

impl Uses {
    /// What a class's members do with its type parameters (`data`: all of them shown and compared).
    pub(crate) fn of_class(members: &[Member], ctor: &[ConstructorParam], data: bool) -> Uses {
        let mut typed = Vec::new();
        for cp in ctor {
            if let Some(t) = &cp.param.ty {
                typed.push((cp.param.name.name.clone(), type_name(t)));
            }
        }
        for m in members {
            match m {
                Member::Decl(Decl::Property(p)) => {
                    if let (Binding::Name(n), Some(t)) = (&p.binding, &p.ty) {
                        typed.push((n.name.clone(), type_name(t)));
                    }
                }
                Member::Decl(Decl::Function(f)) => {
                    for p in &f.params {
                        if let Some(t) = &p.ty {
                            typed.push((p.name.name.clone(), type_name(t)));
                        }
                    }
                }
                _ => {}
            }
        }
        let mut uses = Uses { all: data, ..Default::default() };
        let mut look = |e: &Expr| uses.note(e, &typed);
        for m in members {
            match m {
                Member::Decl(Decl::Function(f)) => walk_body(f.body.as_ref(), &mut look),
                Member::Decl(Decl::Property(p)) => {
                    if let PropertyInit::Value(e) | PropertyInit::Delegate(e) = &p.init {
                        walk_expr(e, &mut look);
                    }
                    if let Some(g) = &p.getter {
                        walk_body(g.body.as_ref(), &mut look);
                    }
                }
                Member::Init(b) => walk_block(b, &mut look),
                _ => {}
            }
        }
        uses
    }

    /// What a function does with its type parameters (`kompozable`: its parameters compared).
    pub(crate) fn of_function(f: &Function) -> Uses {
        let typed: Vec<(String, Option<String>)> =
            f.params.iter().filter_map(|p| p.ty.as_ref().map(|t| (p.name.name.clone(), type_name(t)))).collect();
        let mut uses = Uses { all: false, ..Default::default() };
        if f.is_kompozable() {
            uses.compared = f.type_params.iter().map(|t| t.name.name.clone()).collect();
        }
        let mut look = |e: &Expr| uses.note(e, &typed);
        walk_body(f.body.as_ref(), &mut look);
        uses
    }

    /// An expression's uses: a template's piece, `==`'s operands, `toString()`.
    fn note(&mut self, e: &Expr, typed: &[(String, Option<String>)]) {
        let param_of = |x: &Expr| -> Option<String> {
            let name = match &x.kind {
                ExprKind::Name(n) => &n.name,
                ExprKind::Member { receiver, name, .. } if matches!(receiver.kind, ExprKind::This { .. }) => &name.name,
                _ => return None,
            };
            typed.iter().find(|(n, _)| n == name).and_then(|(_, t)| t.clone())
        };
        match &e.kind {
            ExprKind::String(s) => {
                for p in &s.parts {
                    if let StringPart::Expr { expr, .. } = p
                        && let Some(t) = param_of(expr)
                    {
                        self.shown.push(t);
                    }
                }
            }
            ExprKind::Binary { op: BinaryOp::Eq | BinaryOp::NotEq, lhs, rhs } => {
                for x in [lhs, rhs] {
                    if let Some(t) = param_of(x) {
                        self.compared.push(t);
                    }
                }
            }
            ExprKind::Binary { op: BinaryOp::Add, lhs, rhs } => {
                if let Some(t) = param_of(rhs).or_else(|| param_of(lhs)) {
                    self.shown.push(t);
                }
            }
            ExprKind::Call { callee, .. } => {
                if let ExprKind::Member { receiver, name, .. } = &callee.kind
                    && name.name == "toString"
                    && let Some(t) = param_of(receiver)
                {
                    self.shown.push(t);
                }
            }
            _ => {}
        }
    }
}

/// A type's name when it is one name alone (a type parameter's).
fn type_name(t: &Type) -> Option<String> {
    match &t.kind {
        TypeKind::Named(segs) if segs.len() == 1 && segs[0].args.is_empty() => Some(segs[0].name.name.clone()),
        _ => None,
    }
}

pub(crate) fn walk_body(b: Option<&FunctionBody>, f: &mut impl FnMut(&Expr)) {
    match b {
        Some(FunctionBody::Block(b)) => walk_block(b, f),
        Some(FunctionBody::Expr(e)) => walk_expr(e, f),
        None => {}
    }
}

pub(crate) fn walk_block(b: &Block, f: &mut impl FnMut(&Expr)) {
    for s in &b.stmts {
        match &s.kind {
            StmtKind::Decl(Decl::Property(p)) => {
                if let PropertyInit::Value(e) | PropertyInit::Delegate(e) = &p.init {
                    walk_expr(e, f);
                }
            }
            StmtKind::Decl(Decl::Function(g)) => walk_body(g.body.as_ref(), f),
            StmtKind::Decl(_) => {}
            StmtKind::Assign { target, value, .. } => {
                walk_expr(target, f);
                walk_expr(value, f);
            }
            StmtKind::For { iterable, body, .. } => {
                walk_expr(iterable, f);
                walk_block(body, f);
            }
            StmtKind::While { condition, body, .. } | StmtKind::DoWhile { body, condition, .. } => {
                walk_expr(condition, f);
                walk_block(body, f);
            }
            StmtKind::Expr(e) => walk_expr(e, f),
        }
    }
}

/// Every expression in `e` (itself first).
pub(crate) fn walk_expr(e: &Expr, f: &mut impl FnMut(&Expr)) {
    f(e);
    match &e.kind {
        ExprKind::String(s) => {
            for p in &s.parts {
                if let StringPart::Expr { expr, .. } = p {
                    walk_expr(expr, f);
                }
            }
        }
        ExprKind::Call { callee, args, trailing_lambda, .. } => {
            walk_expr(callee, f);
            for a in args {
                walk_expr(&a.value, f);
            }
            if let Some(l) = trailing_lambda {
                walk_block(&l.body, f);
            }
        }
        ExprKind::Member { receiver, .. } => walk_expr(receiver, f),
        ExprKind::Index { receiver, indices } => {
            walk_expr(receiver, f);
            for i in indices {
                walk_expr(i, f);
            }
        }
        ExprKind::Lambda(l) => walk_block(&l.body, f),
        ExprKind::Prefix { operand, .. } | ExprKind::Postfix { operand, .. } => walk_expr(operand, f),
        ExprKind::Binary { lhs, rhs, .. } | ExprKind::Infix { lhs, rhs, .. } => {
            walk_expr(lhs, f);
            walk_expr(rhs, f);
        }
        ExprKind::Is { expr, .. } | ExprKind::As { expr, .. } | ExprKind::Paren(expr) => walk_expr(expr, f),
        ExprKind::Annotated { expr, .. } => walk_expr(expr, f),
        ExprKind::If { condition, then, otherwise } => {
            walk_expr(condition, f);
            walk_block(then, f);
            if let Some(o) = otherwise {
                walk_block(o, f);
            }
        }
        ExprKind::When(w) => {
            if let Some(s) = &w.subject {
                walk_expr(&s.expr, f);
            }
            for b in &w.branches {
                for c in &b.conditions {
                    match c {
                        WhenCondition::Expr(x) | WhenCondition::In { range: x, .. } => walk_expr(x, f),
                        WhenCondition::Is { .. } => {}
                    }
                }
                walk_block(&b.body, f);
            }
        }
        ExprKind::Try(t) => {
            walk_block(&t.body, f);
            for c in &t.catches {
                walk_block(&c.body, f);
            }
            if let Some(b) = &t.finally {
                walk_block(b, f);
            }
        }
        ExprKind::Return { value: Some(v), .. } | ExprKind::Throw(v) => walk_expr(v, f),
        _ => {}
    }
}

impl Gen<'_, '_> {
    /// `<T: Clone + 'static + …, …>` of type parameters, bounded by their uses (see the module's doc); empty for
    /// none.
    pub(crate) fn type_param_list(
        &mut self,
        params: &[TypeParam],
        uses: &Uses,
        kompozable: bool,
        span: Span,
    ) -> TokenStream {
        if params.is_empty() {
            return quote!();
        }
        let out = self.type_param_bounds(params, uses, kompozable, span);
        quote_spanned!(span=> <#(#out),*>)
    }

    /// `T: Clone + 'static + …` of each type parameter (see [`type_param_list`](Self::type_param_list)): a where
    /// clause's.
    pub(crate) fn type_param_bounds(
        &mut self,
        params: &[TypeParam],
        uses: &Uses,
        kompozable: bool,
        span: Span,
    ) -> Vec<TokenStream> {
        let kt = self.kt(span);
        let mut out = Vec::new();
        for p in params {
            let i = ident(&p.name.name, self.span(p.name.loc));
            let name = &p.name.name;
            let mut bounds = vec![quote_spanned!(span=> ::core::clone::Clone), quote_spanned!(span=> 'static)];
            if uses.all || uses.shown.contains(name) {
                bounds.push(quote_spanned!(span=> #kt::KtToString));
            }
            if uses.all || kompozable || uses.compared.contains(name) {
                bounds.push(quote_spanned!(span=> ::core::cmp::PartialEq));
            }
            if let Some(b) = &p.bound {
                let bt = self.rust_type(b, TypePos::Value);
                // (a bound of a Kotlin type the Rust side has as a trait object, `T : Shape`: its trait)
                let text = bt.to_string().replace(' ', "");
                if let Some(inner) = text.strip_prefix("::std::rc::Rc<dyn").and_then(|r| r.strip_suffix('>')) {
                    let t: TokenStream = super::respan(inner.parse().unwrap_or_default(), span);
                    bounds.push(t);
                } else if !matches!(text.as_str(), "_") {
                    bounds.push(bt);
                }
            }
            out.push(quote_spanned!(span=> #i: #(#bounds)+*));
        }
        out
    }

    /// What a user's generic class needs of its type parameters, by its Rust name: their names and uses.
    /// ((stage 14 WPA) a generic sealed class's: its cases' too, see `Gen::sealed_uses`)
    pub(crate) fn class_uses(&self, rust: &str) -> Option<(Vec<String>, Uses)> {
        let d = self.decl_of(rust)?;
        if d.info.inherit == super::hierarchy::Inherit::Sealed && !d.info.type_params.is_empty() {
            return Some((d.info.type_params.clone(), self.sealed_uses(rust)));
        }
        self.own_class_uses(rust)
    }

    /// What a class's own members need of its type parameters.
    pub(crate) fn own_class_uses(&self, rust: &str) -> Option<(Vec<String>, Uses)> {
        let d = self.decl_of(rust)?;
        let super::class::ClassAst::Class(c) = &d.ast else { return None };
        let ctor: Vec<ConstructorParam> = c.primary_constructor.iter().flat_map(|p| p.params.clone()).collect();
        let data = d.info.shape == super::class::Shape::Data;
        Some((d.info.type_params.clone(), Uses::of_class(&c.body.members, &ctor, data)))
    }

    /// The uses the type parameters `params` get as type arguments of the user's generic types in `t` (`Crate<R>`:
    /// what `Crate` needs of its `T`, `R` needs).
    pub(crate) fn uses_through(&self, t: &Type, params: &[String], uses: &mut Uses) {
        match &t.kind {
            TypeKind::Named(segs) => {
                let name = segs.iter().map(|s| s.name.name.as_str()).collect::<Vec<_>>().join(".");
                let Some(last) = segs.last() else { return };
                let theirs = self.user_class(&name).and_then(|c| self.class_uses(&c.rust));
                for (k, a) in last.args.iter().enumerate() {
                    let TypeArg::Type { ty, .. } = a else { continue };
                    if let (Some((tps, cu)), Some(p)) = (&theirs, type_name(ty))
                        && params.contains(&p)
                        && let Some(tp) = tps.get(k)
                    {
                        if cu.all || cu.shown.contains(tp) {
                            uses.shown.push(p.clone());
                        }
                        if cu.all || cu.compared.contains(tp) {
                            uses.compared.push(p.clone());
                        }
                    }
                    self.uses_through(ty, params, uses);
                }
            }
            TypeKind::Function(f) => {
                for p in &f.params {
                    self.uses_through(&p.ty, params, uses);
                }
                self.uses_through(&f.result, params, uses);
            }
            TypeKind::Error => {}
        }
    }

    /// A function's type parameters as Rust's (`fn first_or<T: Clone + 'static>`), bounded by their uses.
    pub(crate) fn fn_generics(&mut self, f: &Function, span: Span) -> TokenStream {
        let mut params = f.type_params.clone();
        // (`where T : A`: the bound added to the parameter)
        for c in &f.constraints {
            if let Some(p) = params.iter_mut().find(|p| p.name.name == c.name.name)
                && p.bound.is_none()
            {
                p.bound = Some(c.bound.clone());
            }
        }
        let mut uses = Uses::of_function(f);
        let names: Vec<String> = params.iter().map(|p| p.name.name.clone()).collect();
        for t in f.params.iter().filter_map(|p| p.ty.as_ref()).chain(f.result.as_ref()).chain(f.receiver.as_ref()) {
            self.uses_through(t, &names, &mut uses);
        }
        let mut out = self.type_param_bounds(&params, &uses, f.is_kompozable(), span);
        // (its parameters' star projections: type parameters of its own)
        for (_, s) in self.classes.stars.take().unwrap_or_default() {
            out.push(quote_spanned!(span=> #s: ::core::clone::Clone + 'static));
        }
        if out.is_empty() { quote!() } else { quote_spanned!(span=> <#(#out),*>) }
    }

    /// Starts collecting the star projections of a function's parameters ([`star_projection`](Self::star_projection)).
    pub(crate) fn collect_stars(&mut self) {
        self.classes.stars = Some(Vec::new());
    }

    /// `*` of a type's argument (`List<*>`): in a function's parameters a type parameter of the function's own
    /// (`fn count<__KzStar0: Clone + 'static>(xs: Vec<__KzStar0>)`, one per place), else `_` (Rust's inference).
    pub(crate) fn star_projection(&mut self, loc: crate::pos::Loc) -> TokenStream {
        let span = self.span(loc);
        match &mut self.classes.stars {
            Some(list) => {
                if let Some((_, i)) = list.iter().find(|(l, _)| *l == loc) {
                    return quote!(#i);
                }
                let i = quote::format_ident!("__KzStar{}", list.len(), span = span);
                list.push((loc, i.clone()));
                quote!(#i)
            }
            None => quote_spanned!(span=> _),
        }
    }
}
