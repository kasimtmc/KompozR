//! What a `#[kompozable]` body goes through: calls with named arguments (and every PascalCase call) rewritten into
//! the callee's builder, closures and async blocks passed to calls made to capture clones of the locals they use
//! (Kotlin's lambdas share what they capture; Rust's `move` would take it away), and closures passed to rewritten
//! calls memoized by what they capture (Compose's strong skipping).
//!
//! ```text
//! Column(modifier = m, || Text(format!("{count}")))
//!   →  { (&&Probe(Column)).__kz_builder() }.modifier(m)
//!        .__kz_memo___last(|| (key(&count), ()), { let count = capture(&count); move || … }).__call()
//! ```
//!
//! - every call of a PascalCase path goes through `Probe`, which picks the builder of a `#[kompozable]` function
//!   (its marker) and falls back to a positional call for anything else (`Some(x)`, a tuple struct, a `#[named]`
//!   function called with the parameters it has without a default); a PascalCase `#[named]` function that isn't a
//!   composable is reached through its type (`TextStyle::__kz_builder()`) when called with named arguments or more
//!   positional ones than its parameters without a default, as the API index or the body's own items tell;
//! - a lowercase path call or a method call is rewritten when an argument is named (`name = value`), or when it
//!   gives more positional arguments than the callee's parameters without a default (and no more than all), or
//!   when the callee has a `#[memoized]` parameter and the arguments fill its parameters (its closure gets its
//!   identity: `LocalX.provides_computed(|s| …)`), the callee known from the API index (`known`) or the body's own
//!   items (`color.copy(0.5)`, `top_app_bar_colors(red)`). A function's name counts only on a path of modules (`f(…)`, `module::f(…)`), not a
//!   type's (`Type::f(…)`). A raw name keeps a call as written (`x.r#copy(1)`, `r#Font(a, b)`), and so do the
//!   names `#[kompozable(plain(copy, …))]` lists: methods of other types with the name of an indexed one;
//! - positional arguments fill the parameters in order, except a last unnamed closure, which goes to the last
//!   parameter (Kotlin's trailing lambda);
//! - a closure or async block passed to a rewritten call is made `move` and captures clones of the locals it uses
//!   (they must be `Clone`: states and `Rc`s share what they hold), as Kotlin's lambdas share what they capture;
//!   so does a `move` one passed to any call inside another closure (`on_click = || scope.launch(async move { … })`:
//!   moving would take the value away from a closure that runs again). Written `move` at the top of the body, a
//!   closure moves what it captures, as in plain Rust (to hand over what can't be cloned);
//! - a closure passed to a rewritten call is memoized by the values it captures: each one's key (`KeyProbe`: by
//!   `==`, by instance for an `Rc`/`Arc` without `==`, else never equal) goes with it to the setter's twin
//!   (`__kz_memo_x(keys, closure)`), which, where the parameter keeps the closure, gives it an identity that stays
//!   while the keys are equal (Compose 2's strong skipping: a lambda remembered by its captures). A closure given
//!   through a function of it alone (`measure_policy(move |…| …)`) counts as that closure; a restartable
//!   composable's closure parameter passed on (`Child(on_click)`) or captured keeps its own identity. A closure that
//!   uses `self`, or (in `kompozable_lambda!`) a macro whose names can't be told, is never equal.

use crate::known::Known;
use proc_macro2::{TokenStream as Ts, TokenTree};
use quote::{format_ident, quote, quote_spanned};
use std::collections::HashSet;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::visit_mut::{self, VisitMut};
use syn::*;

pub(crate) fn is_pascal(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
}

/// `name = value`
pub(crate) fn named_arg(e: &Expr) -> Option<(&Ident, &Expr)> {
    if let Expr::Assign(a) = e
        && let Expr::Path(p) = &*a.left
        && let Some(i) = p.path.get_ident()
    {
        return Some((i, &a.right));
    }
    None
}

fn named_arg_mut(e: &mut Expr) -> Option<&mut Expr> {
    if let Expr::Assign(a) = e
        && let Expr::Path(p) = &*a.left
        && p.path.get_ident().is_some()
    {
        return Some(&mut a.right);
    }
    None
}

/// The names a pattern binds.
pub(crate) fn pattern_names(p: &Pat, out: &mut Vec<String>) {
    struct Names<'a>(&'a mut Vec<String>);
    impl<'ast> Visit<'ast> for Names<'_> {
        fn visit_pat_ident(&mut self, i: &'ast PatIdent) {
            self.0.push(i.ident.to_string());
            visit::visit_pat_ident(self, i);
        }
    }
    Names(out).visit_pat(p);
}

/// The names `let`s in an `if`/`while` condition bind.
fn condition_names(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Let(l) => pattern_names(&l.pat, out),
        Expr::Binary(b) => {
            condition_names(&b.left, out);
            condition_names(&b.right, out);
        }
        Expr::Paren(p) => condition_names(&p.expr, out),
        _ => {}
    }
}

/// The identifiers a closure or async block uses that it doesn't bind itself: candidates for its captures. A name
/// bound inside it hides an outer one only after its binding and within its scope (`let w = w.get();` uses the
/// outer `w`, then hides it below). Inside macros (`format!`, `vec!`) every identifier counts, and the `{name}` of a
/// format string.
struct FreeNames {
    used: HashSet<String>,
    /// Where each was first used (what an error about its capture points at).
    spans: std::collections::HashMap<String, proc_macro2::Span>,
    /// The names bound inside it, by scope (innermost last), as the walk reaches them.
    scopes: Vec<HashSet<String>>,
    /// Names only seen as what is called (`f(…)`): functions, not captures, where locals aren't known.
    called: HashSet<String>,
    /// Inside macros whose arguments aren't expressions: every identifier.
    raw: HashSet<String>,
}

impl FreeNames {
    fn new() -> FreeNames {
        FreeNames {
            used: HashSet::new(),
            spans: Default::default(),
            scopes: vec![HashSet::new()],
            called: HashSet::new(),
            raw: HashSet::new(),
        }
    }

    fn bound(&self, name: &str) -> bool {
        self.scopes.iter().any(|s| s.contains(name))
    }

    /// `name`, used here: free unless bound in a scope around this use.
    fn use_name(&mut self, name: String, span: Option<proc_macro2::Span>) {
        if self.bound(&name) {
            return;
        }
        if let Some(span) = span {
            self.spans.entry(name.clone()).or_insert(span);
        }
        self.used.insert(name);
    }

    fn bind_pattern(&mut self, p: &Pat) {
        let mut names = Vec::new();
        pattern_names(p, &mut names);
        self.scopes.last_mut().unwrap().extend(names);
    }

    /// Walks `f` in a scope of its own, `names` bound in it.
    fn scoped(&mut self, names: Vec<String>, f: impl FnOnce(&mut Self)) {
        self.scopes.push(names.into_iter().collect());
        f(self);
        self.scopes.pop();
    }
}

impl<'ast> Visit<'ast> for FreeNames {
    fn visit_expr_path(&mut self, p: &'ast ExprPath) {
        if p.qself.is_none()
            && p.path.leading_colon.is_none()
            && let Some(i) = p.path.get_ident()
        {
            self.use_name(i.to_string(), Some(i.span()));
        }
        visit::visit_expr_path(self, p);
    }
    fn visit_block(&mut self, b: &'ast Block) {
        self.scoped(Vec::new(), |me| {
            for stmt in &b.stmts {
                match stmt {
                    // The initializer sees the names around it; the pattern binds after it.
                    Stmt::Local(l) => {
                        if let Some(init) = &l.init {
                            me.visit_expr(&init.expr);
                            if let Some((_, diverge)) = &init.diverge {
                                me.visit_expr(diverge);
                            }
                        }
                        me.bind_pattern(&l.pat);
                    }
                    Stmt::Item(_) => {}
                    _ => me.visit_stmt(stmt),
                }
            }
        });
    }
    fn visit_expr_closure(&mut self, c: &'ast ExprClosure) {
        let mut names = Vec::new();
        for p in &c.inputs {
            pattern_names(p, &mut names);
        }
        self.scoped(names, |me| me.visit_expr(&c.body));
    }
    fn visit_arm(&mut self, a: &'ast Arm) {
        let mut names = Vec::new();
        pattern_names(&a.pat, &mut names);
        self.scoped(names, |me| {
            me.visit_pat(&a.pat); // (a guard is in the pattern)
            me.visit_expr(&a.body);
        });
    }
    fn visit_expr_for_loop(&mut self, f: &'ast ExprForLoop) {
        self.visit_expr(&f.expr);
        let mut names = Vec::new();
        pattern_names(&f.pat, &mut names);
        self.scoped(names, |me| me.visit_block(&f.body));
    }
    fn visit_expr_if(&mut self, i: &'ast ExprIf) {
        let mut names = Vec::new();
        condition_names(&i.cond, &mut names);
        // (the condition's `let`s bind for what follows them in it and for the then branch)
        self.scoped(names, |me| {
            me.visit_expr(&i.cond);
            me.visit_block(&i.then_branch);
        });
        if let Some((_, e)) = &i.else_branch {
            self.visit_expr(e);
        }
    }
    fn visit_expr_while(&mut self, w: &'ast ExprWhile) {
        let mut names = Vec::new();
        condition_names(&w.cond, &mut names);
        self.scoped(names, |me| {
            me.visit_expr(&w.cond);
            me.visit_block(&w.body);
        });
    }
    fn visit_expr_let(&mut self, l: &'ast ExprLet) {
        // (its pattern's names are bound by the `if`/`while` around it)
        self.visit_expr(&l.expr);
    }
    fn visit_expr_call(&mut self, c: &'ast ExprCall) {
        if let Expr::Path(p) = &*c.func
            && let Some(i) = p.path.get_ident()
        {
            if !self.bound(&i.to_string()) {
                self.called.insert(i.to_string());
            }
        } else {
            self.visit_expr(&c.func);
        }
        for a in &c.args {
            // (`name = value`: the name is a parameter's, not a local)
            self.visit_expr(named_arg(a).map_or(a, |(_, v)| v));
        }
    }
    fn visit_expr_method_call(&mut self, m: &'ast ExprMethodCall) {
        self.visit_expr(&m.receiver);
        for a in &m.args {
            self.visit_expr(named_arg(a).map_or(a, |(_, v)| v));
        }
    }
    fn visit_macro(&mut self, m: &'ast Macro) {
        match Punctuated::<Expr, Token![,]>::parse_terminated.parse2(m.tokens.clone()) {
            Ok(args) => {
                for a in &args {
                    if let Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) = a {
                        let mut names = HashSet::new();
                        format_names(&format!("{:?}", s.value()), &mut names);
                        for n in names {
                            self.use_name(n, None);
                        }
                    }
                    self.visit_expr(a);
                }
            }
            Err(_) => {
                let mut names = HashSet::new();
                tokens_names(m.tokens.clone(), &mut names);
                for n in names {
                    if !self.bound(&n) {
                        self.raw.insert(n);
                    }
                }
            }
        }
    }
    fn visit_expr_struct(&mut self, s: &'ast ExprStruct) {
        // `Point { x, y }`: the shorthand fields use locals.
        for f in &s.fields {
            if let (Member::Named(n), None) = (&f.member, &f.colon_token) {
                self.use_name(n.to_string(), Some(n.span()));
            }
        }
        visit::visit_expr_struct(self, s);
    }
}

fn tokens_names(tokens: Ts, out: &mut HashSet<String>) {
    for t in tokens {
        match t {
            TokenTree::Ident(i) => {
                out.insert(i.to_string());
            }
            TokenTree::Group(g) => tokens_names(g.stream(), out),
            TokenTree::Literal(l) => format_names(&l.to_string(), out),
            TokenTree::Punct(_) => {}
        }
    }
}

/// The `{name}` and `{name:…}` of a format string.
fn format_names(literal: &str, out: &mut HashSet<String>) {
    if !literal.ends_with('"') {
        return;
    }
    let chars: Vec<char> = literal.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            if chars.get(i + 1) == Some(&'{') {
                i += 2;
                continue;
            }
            let start = i + 1;
            let mut j = start;
            while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            if j > start && matches!(chars.get(j), Some('}') | Some(':')) && !chars[start].is_ascii_digit() {
                out.insert(chars[start..j].iter().collect());
            }
            i = j;
        } else {
            i += 1;
        }
    }
}

/// A closure or async block (what captures).
fn capturing(e: &Expr) -> bool {
    matches!(e, Expr::Closure(_) | Expr::Async(_))
}

fn is_move(e: &Expr) -> bool {
    match e {
        Expr::Closure(c) => c.capture.is_some(),
        Expr::Async(a) => a.capture.is_some(),
        _ => false,
    }
}

fn make_move(e: &mut Expr) {
    match e {
        Expr::Closure(c) if c.capture.is_none() => c.capture = Some(Default::default()),
        Expr::Async(a) if a.capture.is_none() => a.capture = Some(Default::default()),
        _ => {}
    }
}

/// Whether a name is written raw (`r#copy`): a call of it is left as written.
fn is_raw(i: &Ident) -> bool {
    i.to_string().starts_with("r#")
}

/// A function's path as the index's names count on it: a name, or one in a module (`kompoz_graphics::Font`), not
/// an associated function of a type (`Constraints::fixed`).
fn module_path(p: &Path) -> bool {
    let n = p.segments.len();
    n == 1 || !is_pascal(&p.segments[n - 2].ident.to_string())
}

/// How a call is rewritten.
#[derive(Clone, Copy, PartialEq)]
enum Route {
    /// As written.
    Keep,
    /// Through `Probe`: a composable's marker gives its builder, anything else a positional call. `named`: for
    /// named arguments (an error unless a builder).
    Probe { named: bool },
    /// Through its type: `path::__kz_builder()`.
    Type,
}

/// What is done with an argument of a call.
enum Plan {
    Keep,
    /// Made `move`, its captures cloned.
    Wrap(Vec<(String, proc_macro2::Span)>),
    /// Memoized: given with `keys` (the closure making its keys), and wrapped if `wrap`.
    Memo {
        wrap: Option<Vec<(String, proc_macro2::Span)>>,
        keys: Ts,
    },
    /// A kept closure parameter passed on: shared (a clone of it, as Kotlin passes a lambda), with its identity.
    PassOn(Ts),
}

/// Rewrites a body: calls into builders, captures into clones, closures memoized. `rt` is the runtime's path.
pub(crate) struct Transformer {
    pub(crate) rt: Ts,
    scopes: Vec<HashSet<String>>,
    /// How many closures and async blocks this is inside.
    depth: usize,
    /// A lambda (`kompozable_lambda!`): the locals around it aren't known, so any lowercase name used as a value
    /// may be one.
    outer_unknown: bool,
    /// Plain code (`named_args!`): only calls with named arguments, or with more positional ones than the
    /// callee's parameters without a default, are rewritten; closures stay as written.
    plain: bool,
    /// The functions and methods with defaulted parameters the calls may call.
    known: Known,
    /// Names whose calls are left as written (`#[kompozable(plain(…))]`).
    left: HashSet<String>,
    /// A restartable composable's closure parameters: each kept with its identity, `__kz_id_<name>`.
    kept_closures: HashSet<String>,
}

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "static",
    "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while", "yield",
];

impl Transformer {
    /// For a `#[kompozable]` function's body: `params` its parameters, `left` the names its calls of are left as
    /// written, `kept_closures` its closure parameters kept with their identities (a restartable composable's).
    pub(crate) fn new(
        rt: Ts,
        params: impl IntoIterator<Item = String>,
        known: Known,
        left: HashSet<String>,
        kept_closures: HashSet<String>,
    ) -> Self {
        Transformer {
            rt,
            scopes: vec![params.into_iter().collect()],
            depth: 0,
            outer_unknown: false,
            plain: false,
            known,
            left,
            kept_closures,
        }
    }

    /// For a lambda: the names around it aren't known.
    pub(crate) fn lambda(rt: Ts, known: Known) -> Self {
        Transformer {
            rt,
            scopes: vec![HashSet::new()],
            depth: 0,
            outer_unknown: true,
            plain: false,
            known,
            left: HashSet::new(),
            kept_closures: HashSet::new(),
        }
    }

    /// For plain code (`named_args!`).
    pub(crate) fn plain(rt: Ts, known: Known) -> Self {
        Transformer { plain: true, ..Transformer::lambda(rt, known) }
    }

    /// A lambda: its body is a composable's (its own closures nested in it, not it in itself).
    pub(crate) fn transform_lambda(&mut self, e: &mut Expr) {
        match e {
            Expr::Closure(c) => {
                let mut names = Vec::new();
                for p in &c.inputs {
                    pattern_names(p, &mut names);
                }
                self.scopes.push(names.into_iter().collect());
                self.visit_expr_mut(&mut c.body);
                self.scopes.pop();
            }
            _ => self.visit_expr_mut(e),
        }
    }

    fn visible(&self, name: &str) -> bool {
        if name == "self" || KEYWORDS.contains(&name) {
            return false;
        }
        self.scopes.iter().any(|s| s.contains(name))
            || (self.outer_unknown && name.starts_with(|c: char| c.is_ascii_lowercase() || c == '_') && name != "_")
    }

    /// Whether `name` here is a kept closure parameter (not a local hiding it).
    fn kept_closure(&self, name: &str) -> bool {
        self.kept_closures.contains(name) && !self.scopes[1..].iter().any(|s| s.contains(name))
    }

    fn bind(&mut self, names: Vec<String>) {
        self.scopes.last_mut().unwrap().extend(names);
    }

    /// The locals a closure or async block captures, with where each is used; and whether it uses what its keys
    /// can't tell (`self`; in a lambda, the names of a macro whose arguments aren't expressions).
    fn captures(&self, e: &Expr) -> (Vec<(String, proc_macro2::Span)>, bool) {
        let mut free = FreeNames::new();
        free.visit_expr(e);
        let uncertain = free.used.contains("self") || (self.outer_unknown && !free.raw.is_empty());
        if !self.outer_unknown {
            // (where the locals are known, a name in a macro's tokens is one only if it is one)
            free.used.extend(free.raw.iter().cloned());
            free.used.extend(free.called.iter().cloned());
        }
        let mut names: Vec<(String, proc_macro2::Span)> = free
            .used
            .into_iter()
            .filter(|n| self.visible(n))
            .map(|n| {
                let span = free.spans.get(&n).copied().unwrap_or_else(|| e.span());
                (n, span)
            })
            .collect();
        names.sort_by(|a, b| a.0.cmp(&b.0));
        (names, uncertain)
    }

    /// `{ let a = capture(&a); …; e }`
    fn wrap(&self, e: Expr, names: &[(String, proc_macro2::Span)]) -> Expr {
        if names.is_empty() {
            return e;
        }
        let rt = &self.rt;
        let lets = names.iter().map(|(n, span)| {
            let i = Ident::new(n, *span);
            quote_spanned!(*span=> let #i = #rt::__private::capture(&#i);)
        });
        parse_quote!({ #(#lets)* #e })
    }

    /// The keys of a closure capturing `names` (a list, `(k1, (k2, ()))`), never equal if `uncertain`.
    fn keys(&self, names: &[(String, proc_macro2::Span)], uncertain: bool) -> Ts {
        let rt = &self.rt;
        let mut list = if uncertain { quote!((#rt::__private::Unequal, ())) } else { quote!(()) };
        for (n, span) in names.iter().rev() {
            let key = if self.kept_closure(n) {
                let id = format_ident!("__kz_id_{}", n);
                quote!(#rt::__private::IdKey(#id))
            } else {
                let i = Ident::new(n, *span);
                quote_spanned!(*span=> {
                    #[allow(unused_imports)]
                    use #rt::__private::{KeyViaPartialEq as _, KeyViaInstance as _, KeyFallback as _};
                    (&&&#rt::__private::KeyProbe(&#i)).__kz_key()
                })
            };
            list = quote!((#key, #list));
        }
        list
    }

    /// How a call of `func` with `args` is rewritten.
    fn route(&self, func: &Expr, args: &Punctuated<Expr, Token![,]>) -> Route {
        let Expr::Path(p) = func else { return Route::Keep };
        if p.qself.is_some() {
            return Route::Keep;
        }
        let seg = p.path.segments.last().unwrap();
        let name = seg.ident.to_string();
        if is_raw(&seg.ident) || self.left.contains(&name) {
            return Route::Keep;
        }
        let named = args.iter().any(|a| named_arg(a).is_some());
        let by_builder = !named
            && seg.arguments.is_none()
            && module_path(&p.path)
            && self.known.function_rewritten(&name, args.len(), args.last().is_some_and(last_is_closure));
        // (a PascalCase function's suspending twin, `PathData_async` — what the Kotlin syntax calls where the lambda
        // of Kotlin's inline function suspends —, is no composable: its async closure is given as it is)
        if is_pascal(&name) && !name.ends_with("_async") {
            if named || by_builder {
                if self.known.is_plain_function(&name) { Route::Type } else { Route::Probe { named } }
            } else if self.plain {
                Route::Keep
            } else {
                Route::Probe { named: false }
            }
        } else if named || by_builder {
            Route::Type
        } else {
            Route::Keep
        }
    }

    /// Whether a method call is rewritten.
    fn method_rewritten(&self, m: &ExprMethodCall) -> bool {
        let name = m.method.to_string();
        if is_raw(&m.method) || self.left.contains(&name) {
            return false;
        }
        m.args.iter().any(|a| named_arg(a).is_some())
            || (m.turbofish.is_none()
                && self.known.method_rewritten(&name, m.args.len(), m.args.last().is_some_and(last_is_closure)))
    }

    /// What is done with each of `args` (a rewritten call's if `rewritten`).
    fn plan(&self, args: &Punctuated<Expr, Token![,]>, rewritten: bool) -> Vec<Plan> {
        if self.plain {
            return args.iter().map(|_| Plan::Keep).collect();
        }
        args.iter()
            .map(|a| {
                let v = named_arg(a).map_or(a, |(_, v)| v);
                match v {
                    Expr::Closure(_) if rewritten => {
                        let (names, uncertain) = self.captures(v);
                        let keys = self.keys(&names, uncertain);
                        if is_move(v) && self.depth == 0 {
                            // (it moves what it captures: the keys are taken first)
                            Plan::Memo { wrap: None, keys: quote!({ let __kz_keys = #keys; move || __kz_keys }) }
                        } else {
                            Plan::Memo { wrap: Some(names), keys: quote!(|| #keys) }
                        }
                    }
                    _ if capturing(v) => {
                        if (rewritten && !(is_move(v) && self.depth == 0)) || (is_move(v) && self.depth > 0) {
                            Plan::Wrap(self.captures(v).0)
                        } else {
                            Plan::Keep
                        }
                    }
                    // A closure given through a function of it alone (`measure_policy(move |…| …)`).
                    Expr::Call(inner)
                        if rewritten
                            && inner.args.len() == 1
                            && matches!(inner.args.first(), Some(Expr::Closure(_)))
                            && self.route(&inner.func, &inner.args) == Route::Keep =>
                    {
                        let (names, uncertain) = self.captures(&inner.args[0]);
                        let keys = self.keys(&names, uncertain);
                        Plan::Memo { wrap: None, keys: quote!({ let __kz_keys = #keys; move || __kz_keys }) }
                    }
                    // A kept closure parameter passed on: its identity.
                    Expr::Path(p)
                        if rewritten
                            && p.qself.is_none()
                            && p.path.get_ident().is_some_and(|i| self.kept_closure(&i.to_string())) =>
                    {
                        let rt = &self.rt;
                        let id = format_ident!("__kz_id_{}", p.path.get_ident().unwrap());
                        Plan::PassOn(quote!(move || (#rt::__private::IdKey(#id), ())))
                    }
                    _ => Plan::Keep,
                }
            })
            .collect()
    }

    /// Wraps what `plan` says; the keys of the memoized arguments.
    fn apply(&self, args: &mut Punctuated<Expr, Token![,]>, plan: Vec<Plan>) -> Vec<Option<Ts>> {
        let mut keys = Vec::new();
        for (a, p) in args.iter_mut().zip(plan) {
            let (wrap, k) = match p {
                Plan::Keep => (None, None),
                Plan::Wrap(names) => (Some(names), None),
                Plan::Memo { wrap, keys } => (wrap, Some(keys)),
                Plan::PassOn(k) => {
                    keys.push(Some(k));
                    let rt = &self.rt;
                    let v = match named_arg_mut(a) {
                        Some(v) => v,
                        None => a,
                    };
                    *v = parse_quote!(#rt::__private::capture(&#v));
                    continue;
                }
            };
            keys.push(k);
            let Some(names) = wrap else { continue };
            let v = match named_arg_mut(a) {
                Some(v) => v,
                None => a,
            };
            make_move(v);
            let taken = std::mem::replace(v, parse_quote!(()));
            *v = self.wrap(taken, &names);
        }
        keys
    }

    /// `.name(v)` / `.__argN(v)` / `.__last(v)` for each argument (`.__kz_memo_name(keys, v)`, … for a memoized
    /// one).
    fn chain(args: &Punctuated<Expr, Token![,]>, keys: &[Option<Ts>]) -> Vec<Ts> {
        let n = args.len();
        let mut out = Vec::new();
        let mut position: usize = 0;
        for (k, a) in args.iter().enumerate() {
            let (setter, v) = if let Some((name, v)) = named_arg(a) {
                (name.clone(), v)
            } else if k == n - 1 && last_is_closure(a) {
                (format_ident!("__last", span = a.span()), a)
            } else {
                position += 1;
                (format_ident!("__arg{}", position - 1, span = a.span()), a)
            };
            out.push(match &keys[k] {
                Some(keys) => {
                    let twin = format_ident!("__kz_memo_{}", setter, span = setter.span());
                    quote_spanned!(a.span()=> .#twin(#keys, #v))
                }
                None => quote_spanned!(a.span()=> .#setter(#v)),
            });
        }
        out
    }
}

impl Transformer {
    /// Macros whose arguments are expressions (`format!`, `vec!`, …): what is in them is a body too.
    fn transform_macro(&mut self, mac: &mut Macro) {
        let name = mac.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
        let exprs = [
            "vec",
            "format",
            "println",
            "print",
            "eprintln",
            "eprint",
            "write",
            "writeln",
            "panic",
            "assert",
            "assert_eq",
            "assert_ne",
            "debug_assert",
            "dbg",
        ];
        // (the kt shims' macros the Kotlin syntax writes — `kt::safe!(a, |it| …)`, `kt::flatten!(…)`,
        // `kt::template!(…)` —: of expressions, whose calls are a composable body's as any)
        let segments: Vec<String> = mac.path.segments.iter().map(|s| s.ident.to_string()).collect();
        let kt = segments.len() >= 2
            && segments[segments.len() - 2] == "kt"
            && matches!(name.as_str(), "safe" | "flatten" | "template");
        if !exprs.contains(&name.as_str()) && !kt {
            return;
        }
        if let Ok(mut items) = Punctuated::<Expr, Token![,]>::parse_terminated.parse2(mac.tokens.clone()) {
            for i in items.iter_mut() {
                self.visit_expr_mut(i);
            }
            mac.tokens = quote!(#items);
        }
    }
}

/// `tokens` with every span made `span`.
fn respanned(tokens: Ts, span: proc_macro2::Span) -> Ts {
    tokens
        .into_iter()
        .map(|t| match t {
            TokenTree::Group(g) => {
                let mut group = proc_macro2::Group::new(g.delimiter(), respanned(g.stream(), span));
                group.set_span(span);
                TokenTree::Group(group)
            }
            mut t => {
                t.set_span(span);
                t
            }
        })
        .collect()
}

/// A closure, or one wrapped with its captures.
fn last_is_closure(e: &Expr) -> bool {
    match e {
        Expr::Closure(_) => true,
        Expr::Block(b) => matches!(b.block.stmts.last(), Some(Stmt::Expr(Expr::Closure(_), None))),
        _ => false,
    }
}

impl VisitMut for Transformer {
    fn visit_block_mut(&mut self, b: &mut Block) {
        self.scopes.push(HashSet::new());
        for stmt in &mut b.stmts {
            match stmt {
                Stmt::Local(l) => {
                    if let Some(init) = &mut l.init {
                        self.visit_expr_mut(&mut init.expr);
                        if let Some((_, diverge)) = &mut init.diverge {
                            self.visit_expr_mut(diverge);
                        }
                    }
                    let mut names = Vec::new();
                    pattern_names(&l.pat, &mut names);
                    self.bind(names);
                }
                Stmt::Item(_) => {}
                _ => self.visit_stmt_mut(stmt),
            }
        }
        self.scopes.pop();
    }

    fn visit_expr_closure_mut(&mut self, c: &mut ExprClosure) {
        let mut names = Vec::new();
        for p in &c.inputs {
            pattern_names(p, &mut names);
        }
        self.scopes.push(names.into_iter().collect());
        self.depth += 1;
        self.visit_expr_mut(&mut c.body);
        self.depth -= 1;
        self.scopes.pop();
    }

    fn visit_expr_for_loop_mut(&mut self, f: &mut ExprForLoop) {
        self.visit_expr_mut(&mut f.expr);
        let mut names = Vec::new();
        pattern_names(&f.pat, &mut names);
        self.scopes.push(names.into_iter().collect());
        self.visit_block_mut(&mut f.body);
        self.scopes.pop();
    }

    fn visit_expr_if_mut(&mut self, i: &mut ExprIf) {
        self.visit_expr_mut(&mut i.cond);
        let mut names = Vec::new();
        condition_names(&i.cond, &mut names);
        self.scopes.push(names.into_iter().collect());
        self.visit_block_mut(&mut i.then_branch);
        self.scopes.pop();
        if let Some((_, e)) = &mut i.else_branch {
            self.visit_expr_mut(e);
        }
    }

    fn visit_expr_while_mut(&mut self, w: &mut ExprWhile) {
        self.visit_expr_mut(&mut w.cond);
        let mut names = Vec::new();
        condition_names(&w.cond, &mut names);
        self.scopes.push(names.into_iter().collect());
        self.visit_block_mut(&mut w.body);
        self.scopes.pop();
    }

    fn visit_arm_mut(&mut self, a: &mut Arm) {
        let mut names = Vec::new();
        pattern_names(&a.pat, &mut names);
        self.scopes.push(names.into_iter().collect());
        self.visit_pat_mut(&mut a.pat); // (a guard is in the pattern)
        self.visit_expr_mut(&mut a.body);
        self.scopes.pop();
    }

    fn visit_stmt_macro_mut(&mut self, m: &mut StmtMacro) {
        self.transform_macro(&mut m.mac);
    }

    fn visit_item_mut(&mut self, _: &mut Item) {
        // (items inside a body are their own: a nested #[kompozable] fn is expanded on its own)
    }

    fn visit_expr_mut(&mut self, e: &mut Expr) {
        let rt = self.rt.clone();
        match e {
            Expr::Call(c) => {
                let route = self.route(&c.func, &c.args);
                let plan = self.plan(&c.args, route != Route::Keep);
                visit_mut::visit_expr_call_mut(self, c);
                let keys = self.apply(&mut c.args, plan);
                if route == Route::Keep {
                    return;
                }
                let Expr::Path(p) = &*c.func else { unreachable!() };
                let span = c.span();
                let links = Self::chain(&c.args, &keys);
                let path = &p.path;
                *e = match route {
                    Route::Probe { named } => {
                        // (the probe and `named` at the call, so what rustc says of them is said there)
                        let probe = respanned(quote!(#rt::__private::Probe), span);
                        let builder = if named {
                            let open = respanned(quote!(#rt::__private::named), span);
                            quote_spanned!(span=> #open((&&#probe(#path)).__kz_named()))
                        } else {
                            quote_spanned!(span=> (&&#probe(#path)).__kz_builder())
                        };
                        parse_quote_spanned!(span=> {
                            #[allow(unused_imports)]
                            use #rt::__private::{ViaBuilder as _, ViaFn as _};
                            #builder
                        } #(#links)* .__call())
                    }
                    _ => parse_quote_spanned!(span=> #path::__kz_builder() #(#links)* .__call()),
                };
            }
            Expr::MethodCall(m) => {
                let rewritten = self.method_rewritten(m);
                let plan = self.plan(&m.args, rewritten);
                visit_mut::visit_expr_method_call_mut(self, m);
                let keys = self.apply(&mut m.args, plan);
                if !rewritten {
                    return;
                }
                let span = m.span();
                let links = Self::chain(&m.args, &keys);
                let recv = &m.receiver;
                let kz_name = format_ident!("__kz_{}", m.method, span = m.method.span());
                *e = parse_quote_spanned!(span=> #recv.#kz_name() #(#links)* .__call());
            }
            Expr::Async(a) => {
                self.scopes.push(HashSet::new());
                self.depth += 1;
                self.visit_block_mut(&mut a.block);
                self.depth -= 1;
                self.scopes.pop();
            }
            Expr::Macro(m) => self.transform_macro(&mut m.mac),
            _ => visit_mut::visit_expr_mut(self, e),
        }
    }
}
