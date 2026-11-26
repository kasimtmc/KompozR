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
    /// (stage 14 FINAL) The bounds the API's generic functions put on those given them as type arguments
    /// (`rememberListDetailPaneScaffoldNavigator<T>()`: `T: NavigatorContentKey`), Rust's text.
    pub api_bounds: Vec<(String, String)>,
    /// (stage 14 FINAL-T) The bounds the API's generic types in the declaration's signatures need of its type
    /// parameters (`NavEntry<T>` → `T: NavEntryKey`): (parameter, trait).
    pub api: Vec<(String, String)>,
    /// Those `all` leaves out: a data class's parameters held only as the API's types' arguments those bound
    /// (`val currentEntry: NavEntry<T>`: shown and compared as the API's type is, `T` as its bound says).
    pub indirect: Vec<String>,
}

/// (stage 14 FINAL-T) What the API's generic types in `t` need of the names they are given (see [`Uses::api`]).
pub(crate) fn api_bounds(t: &Type, out: &mut Vec<(String, String)>) {
    match &t.kind {
        TypeKind::Named(segs) => {
            for seg in segs {
                let args: Vec<&Type> = seg
                    .args
                    .iter()
                    .filter_map(|a| match a {
                        TypeArg::Type { ty, .. } => Some(ty),
                        TypeArg::Star(_) => None,
                    })
                    .collect();
                if args.is_empty() {
                    continue;
                }
                let rust = crate::index::rust_type(&seg.name.name).unwrap_or(&seg.name.name);
                if let Some(bounds) = crate::index::param_bounds(rust) {
                    for (a, b) in args.iter().zip(bounds.iter()) {
                        if let Some(n) = type_name(a) {
                            for tr in b.iter() {
                                if !out.iter().any(|(x, y)| *x == n && y == tr) {
                                    out.push((n.clone(), tr.to_string()));
                                }
                            }
                        }
                    }
                }
                for a in args {
                    api_bounds(a, out);
                }
            }
        }
        TypeKind::Function(f) => {
            for p in &f.params {
                api_bounds(&p.ty, out);
            }
            api_bounds(&f.result, out);
            if let Some(r) = &f.receiver {
                api_bounds(r, out);
            }
        }
        TypeKind::Error => {}
    }
}

/// The single names `t` holds other than as an API generic type's bounded argument (see [`Uses::indirect`]).
fn direct_names(t: &Type, out: &mut Vec<String>) {
    if let Some(n) = type_name(t) {
        out.push(n);
        return;
    }
    match &t.kind {
        TypeKind::Named(segs) => {
            for seg in segs {
                let rust = crate::index::rust_type(&seg.name.name).unwrap_or(&seg.name.name);
                let bounds = crate::index::param_bounds(rust);
                let mut k = 0;
                for a in &seg.args {
                    let TypeArg::Type { ty, .. } = a else { continue };
                    let bounded = bounds.and_then(|b| b.get(k)).is_some_and(|b| !b.is_empty());
                    k += 1;
                    if bounded && type_name(ty).is_some() {
                        continue;
                    }
                    direct_names(ty, out);
                }
            }
        }
        TypeKind::Function(f) => {
            for p in &f.params {
                direct_names(&p.ty, out);
            }
            direct_names(&f.result, out);
        }
        TypeKind::Error => {}
    }
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
        // ((stage 14 FINAL-T) the API's types' bounds; a data class's parameters held only as their arguments)
        let mut api = Vec::new();
        let mut direct = Vec::new();
        for cp in ctor {
            if let Some(t) = &cp.param.ty {
                api_bounds(t, &mut api);
                if cp.property.is_some() {
                    direct_names(t, &mut direct);
                }
            }
        }
        for m in members {
            match m {
                Member::Decl(Decl::Property(p)) => {
                    if let Some(t) = &p.ty {
                        api_bounds(t, &mut api);
                    }
                }
                Member::Decl(Decl::Function(f)) => {
                    for t in f.params.iter().filter_map(|p| p.ty.as_ref()).chain(f.result.as_ref()) {
                        api_bounds(t, &mut api);
                    }
                    if let Some(r) = &f.receiver {
                        api_bounds(r, &mut api);
                    }
                }
                _ => {}
            }
        }
        if data {
            uses.indirect = api.iter().map(|(n, _)| n.clone()).filter(|n| !direct.contains(n)).collect();
        }
        uses.api = api;
        let mut look = |e: &Expr| uses.note(e, &typed, &[]);
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
        // ((stage 14 FINAL-T) the API's types' bounds)
        for t in f.params.iter().filter_map(|p| p.ty.as_ref()).chain(f.result.as_ref()).chain(f.receiver.as_ref()) {
            api_bounds(t, &mut uses.api);
        }
        if f.is_kompozable() {
            uses.compared = f.type_params.iter().map(|t| t.name.name.clone()).collect();
        }
        // ((stage 14 LISTS) the parameters holding a type parameter's values: `xs: List<T>`, their elements of it)
        let elems: Vec<(String, String)> =
            f.params.iter().filter_map(|p| Some((p.name.name.clone(), element_param(p.ty.as_ref()?)?))).collect();
        let mut look = |e: &Expr| uses.note(e, &typed, &elems);
        walk_body(f.body.as_ref(), &mut look);
        let names: Vec<String> = f.type_params.iter().map(|t| t.name.name.clone()).collect();
        let mut api = Vec::new();
        walk_body(f.body.as_ref(), &mut |e: &Expr| api_bounds_of(e, &names, &mut api));
        uses.api_bounds = api;
        uses
    }

    /// An expression's uses: a template's piece, `==`'s operands, `toString()`.
    fn note(&mut self, e: &Expr, typed: &[(String, Option<String>)], elems: &[(String, String)]) {
        let param_of = |x: &Expr| -> Option<String> {
            // ((stage 14 LISTS) an element of a parameter holding the type parameter's values: `xs[i]`, `xs.first()`)
            let elem_of = |r: &Expr| match &r.kind {
                ExprKind::Name(n) => elems.iter().find(|(p, _)| *p == n.name).map(|(_, t)| t.clone()),
                _ => None,
            };
            match &x.kind {
                ExprKind::Index { receiver, .. } => return elem_of(receiver),
                ExprKind::Call { callee, .. } => {
                    return match &callee.kind {
                        ExprKind::Member { receiver, name, .. }
                            if matches!(
                                name.name.as_str(),
                                "first"
                                    | "last"
                                    | "get"
                                    | "elementAt"
                                    | "single"
                                    | "firstOrNull"
                                    | "lastOrNull"
                                    | "getOrNull"
                                    | "singleOrNull"
                                    | "random"
                            ) =>
                        {
                            elem_of(receiver)
                        }
                        _ => None,
                    };
                }
                _ => {}
            }
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

/// (stage 14 FINAL) The bounds a call of a generic function of the API given the type parameters `names` as type
/// arguments puts on them (its own type parameters' trait bounds, [`crate::index::fn_bounds`]; those of plain trait
/// paths: a closure's or another type parameter's aren't Kotlin's to need).
fn api_bounds_of(e: &Expr, names: &[String], out: &mut Vec<(String, String)>) {
    let ExprKind::Call { callee, type_args, .. } = &e.kind else { return };
    if type_args.is_empty() {
        return;
    }
    let name = match &callee.kind {
        ExprKind::Name(n) => &n.name,
        ExprKind::Member { name, .. } => &name.name,
        _ => return,
    };
    for (k, a) in type_args.iter().enumerate() {
        let TypeArg::Type { ty, .. } = a else { continue };
        let Some(p) = type_name(ty).filter(|p| names.contains(p)) else { continue };
        for item in super::resolve::lookup(name) {
            let Some((_, text)) = crate::index::fn_bounds(item).get(k) else { continue };
            for b in text.split('+').map(str::trim).filter(|b| !b.is_empty()) {
                let plain = b.chars().all(|c| c.is_alphanumeric() || c == '_' || c == ':')
                    && !(b.len() == 1 && b.chars().all(|c| c.is_ascii_uppercase()));
                if plain && !out.iter().any(|(q, x)| *q == p && x == b) {
                    out.push((p.clone(), b.to_string()));
                }
            }
        }
    }
}

/// (stage 14 LISTS) The type parameter a collection type holds (`List<T>`, `MutableList<T>`, `Set<T>`, …: `T`).
fn element_param(t: &Type) -> Option<String> {
    let TypeKind::Named(segs) = &t.kind else { return None };
    let [seg] = segs.as_slice() else { return None };
    if !matches!(
        seg.name.name.as_str(),
        "List" | "MutableList" | "Collection" | "Iterable" | "Set" | "MutableSet" | "ArrayList" | "ListView" | "Array"
    ) {
        return None;
    }
    match seg.args.as_slice() {
        [TypeArg::Type { ty, .. }] => type_name(ty),
        _ => None,
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
            let all = uses.all && !uses.indirect.contains(name);
            if all || uses.shown.contains(name) {
                bounds.push(quote_spanned!(span=> #kt::KtToString));
            }
            if all || kompozable || uses.compared.contains(name) {
                bounds.push(quote_spanned!(span=> ::core::cmp::PartialEq));
            }
            // ((stage 14 FINAL) what the API's generic functions given it need of it)
            for (_, b) in uses.api_bounds.iter().filter(|(q, _)| q == name) {
                if let Ok(t) = b.parse::<TokenStream>() {
                    bounds.push(super::respan(t, span));
                }
            }
            // ((stage 14 FINAL-T) what the API's types it is given to need of it: `NavEntry<T>`'s `NavEntryKey`)
            for (_, tr) in uses.api.iter().filter(|(n, _)| n == name) {
                let t = ident(tr, span);
                bounds.push(quote_spanned!(span=> #t));
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
