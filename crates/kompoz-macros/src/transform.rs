//! What a `#[kompozable]` body goes through: calls with named arguments (and every PascalCase call) rewritten into
//! the callee's builder, and closures and async blocks passed to calls made to capture clones of the locals they
//! use (Kotlin's lambdas share what they capture; Rust's `move` would take it away).
//!
//! ```text
//! Column(modifier = m, || Text(format!("{count}")))
//!   →  { (&&Probe(Column)).__kz_builder() }.modifier(m).__last({ let count = capture(&count); move || … }).__call()
//! ```
//!
//! - every call of a PascalCase path goes through `Probe`, which picks the builder of a `#[kompozable]` or
//!   `#[named]` function and falls back to a positional call for anything else (`Some(x)`, a tuple struct);
//! - a lowercase path call or a method call is rewritten only when an argument is named (`name = value`);
//! - positional arguments fill the parameters in order, except a last unnamed closure, which goes to the last
//!   parameter (Kotlin's trailing lambda);
//! - a closure or async block passed to a rewritten call is made `move` and captures clones of the locals it uses
//!   (they must be `Clone`: states and `Rc`s share what they hold), as Kotlin's lambdas share what they capture;
//!   so does a `move` one passed to any call inside another closure (`on_click = || scope.launch(async move { … })`:
//!   moving would take the value away from a closure that runs again). Written `move` at the top of the body, a
//!   closure moves what it captures, as in plain Rust (to hand over what can't be cloned).

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

/// The identifiers a closure or async block uses that it doesn't bind itself: candidates for its captures. Inside
/// macros (`format!`, `vec!`) every identifier counts, and the `{name}` of a format string.
struct FreeNames {
    used: HashSet<String>,
    /// Where each was first used (what an error about its capture points at).
    spans: std::collections::HashMap<String, proc_macro2::Span>,
    bound: HashSet<String>,
    /// Names only seen as what is called (`f(…)`): functions, not captures, where locals aren't known.
    called: HashSet<String>,
    /// Inside macros whose arguments aren't expressions: every identifier.
    raw: HashSet<String>,
}

impl<'ast> Visit<'ast> for FreeNames {
    fn visit_expr_path(&mut self, p: &'ast ExprPath) {
        if p.qself.is_none()
            && p.path.leading_colon.is_none()
            && let Some(i) = p.path.get_ident()
        {
            self.used.insert(i.to_string());
            self.spans.entry(i.to_string()).or_insert(i.span());
        }
        visit::visit_expr_path(self, p);
    }
    fn visit_pat_ident(&mut self, i: &'ast PatIdent) {
        self.bound.insert(i.ident.to_string());
        visit::visit_pat_ident(self, i);
    }
    fn visit_expr_call(&mut self, c: &'ast ExprCall) {
        if let Expr::Path(p) = &*c.func
            && let Some(i) = p.path.get_ident()
        {
            self.called.insert(i.to_string());
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
                        format_names(&format!("{:?}", s.value()), &mut self.used);
                    }
                    self.visit_expr(a);
                }
            }
            Err(_) => tokens_names(m.tokens.clone(), &mut self.raw),
        }
    }
    fn visit_expr_struct(&mut self, s: &'ast ExprStruct) {
        // `Point { x, y }`: the shorthand fields use locals.
        for f in &s.fields {
            if let (Member::Named(n), None) = (&f.member, &f.colon_token) {
                self.used.insert(n.to_string());
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

/// Rewrites a body: calls into builders, captures into clones. `rt` is the runtime's path.
pub(crate) struct Transformer {
    pub(crate) rt: Ts,
    scopes: Vec<HashSet<String>>,
    /// How many closures and async blocks this is inside.
    depth: usize,
    /// A lambda (`kompozable_lambda!`): the locals around it aren't known, so any lowercase name used as a value
    /// may be one.
    outer_unknown: bool,
}

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "static",
    "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while", "yield",
];

impl Transformer {
    pub(crate) fn new(rt: Ts, params: impl IntoIterator<Item = String>) -> Self {
        Transformer { rt, scopes: vec![params.into_iter().collect()], depth: 0, outer_unknown: false }
    }

    /// For a lambda: the names around it aren't known.
    pub(crate) fn lambda(rt: Ts) -> Self {
        Transformer { rt, scopes: vec![HashSet::new()], depth: 0, outer_unknown: true }
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

    fn bind(&mut self, names: Vec<String>) {
        self.scopes.last_mut().unwrap().extend(names);
    }

    /// The locals a closure or async block captures, with where each is used.
    fn captures(&self, e: &Expr) -> Vec<(String, proc_macro2::Span)> {
        let mut free = FreeNames {
            used: HashSet::new(),
            spans: Default::default(),
            bound: HashSet::new(),
            called: HashSet::new(),
            raw: HashSet::new(),
        };
        free.visit_expr(e);
        if !self.outer_unknown {
            // (where the locals are known, a name in a macro's tokens is one only if it is one)
            free.used.extend(free.raw.iter().cloned());
            free.used.extend(free.called.iter().cloned());
        }
        let mut names: Vec<(String, proc_macro2::Span)> = free
            .used
            .into_iter()
            .filter(|n| !free.bound.contains(n) && self.visible(n))
            .map(|n| {
                let span = free.spans.get(&n).copied().unwrap_or_else(|| e.span());
                (n, span)
            })
            .collect();
        names.sort_by(|a, b| a.0.cmp(&b.0));
        names
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

    /// The capture lists of `args` (by index), for the closures and async blocks to wrap.
    fn plan(
        &self,
        args: &Punctuated<Expr, Token![,]>,
        rewritten: bool,
    ) -> Vec<Option<Vec<(String, proc_macro2::Span)>>> {
        args.iter()
            .map(|a| {
                let v = named_arg(a).map_or(a, |(_, v)| v);
                (capturing(v) && ((rewritten && !(is_move(v) && self.depth == 0)) || (is_move(v) && self.depth > 0)))
                    .then(|| self.captures(v))
            })
            .collect()
    }

    fn apply(&self, args: &mut Punctuated<Expr, Token![,]>, plan: Vec<Option<Vec<(String, proc_macro2::Span)>>>) {
        for (a, names) in args.iter_mut().zip(plan) {
            let Some(names) = names else { continue };
            let v = match named_arg_mut(a) {
                Some(v) => v,
                None => a,
            };
            make_move(v);
            let taken = std::mem::replace(v, parse_quote!(()));
            *v = self.wrap(taken, &names);
        }
    }

    /// `.name(v)` / `.__argN(v)` / `.__last(v)` for each argument.
    fn chain(args: &Punctuated<Expr, Token![,]>) -> Vec<Ts> {
        let n = args.len();
        let mut out = Vec::new();
        let mut position = 0;
        for (k, a) in args.iter().enumerate() {
            if let Some((name, v)) = named_arg(a) {
                out.push(quote_spanned!(a.span()=> .#name(#v)));
            } else if k == n - 1 && last_is_closure(a) {
                out.push(quote_spanned!(a.span()=> .__last(#a)));
            } else {
                let m = format_ident!("__arg{position}", span = a.span());
                out.push(quote_spanned!(a.span()=> .#m(#a)));
                position += 1;
            }
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
        if !exprs.contains(&name.as_str()) {
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
                let pascal = matches!(&*c.func, Expr::Path(p) if p.path.segments.last().is_some_and(|s| is_pascal(&s.ident.to_string())));
                let named = c.args.iter().any(|a| named_arg(a).is_some());
                let rewritten = matches!(&*c.func, Expr::Path(_)) && (pascal || named);
                let plan = self.plan(&c.args, rewritten);
                visit_mut::visit_expr_call_mut(self, c);
                self.apply(&mut c.args, plan);
                if !rewritten {
                    return;
                }
                let Expr::Path(p) = &*c.func else { unreachable!() };
                let span = c.span();
                let links = Self::chain(&c.args);
                let path = &p.path;
                *e = if pascal {
                    parse_quote_spanned!(span=> {
                        #[allow(unused_imports)]
                        use #rt::__private::{ViaBuilder as _, ViaFn as _};
                        (&&#rt::__private::Probe(#path)).__kz_builder()
                    } #(#links)* .__call())
                } else {
                    parse_quote_spanned!(span=> #path::__kz_builder() #(#links)* .__call())
                };
            }
            Expr::MethodCall(m) => {
                let named = m.args.iter().any(|a| named_arg(a).is_some());
                let plan = self.plan(&m.args, named);
                visit_mut::visit_expr_method_call_mut(self, m);
                self.apply(&mut m.args, plan);
                if !named {
                    return;
                }
                let span = m.span();
                let links = Self::chain(&m.args);
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
