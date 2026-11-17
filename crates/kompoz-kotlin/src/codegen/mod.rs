//! The Rust a Kotlin file becomes (the C++ kompozc's `codegen.cpp`): items `#[kompozable]` then processes, so
//! identity, restarting and skipping, builders, captures and memoized lambdas are the Rust syntax's own. Every token
//! gets the span of the Kotlin it comes from (`SpanMap`): rustc's errors and `#[track_caller]`'s call sites land on
//! Kotlin lines.
//!
//! What is translated (stage 9's Kotlin):
//!
//! | Kotlin | Rust |
//! |---|---|
//! | `@Kompozable fun Name(p: T = d)` (`@Composable` too) | `#[kompozable] pub fn Name(#[default(d)] p: T)`; `inline` → `#[kompozable(inline)]` |
//! | `fun name(…): R = e` / `{ … }` | `#[named] fn name(…) -> R` (`#[named]` when a parameter has a default); its body in `named_args!` |
//! | `fun Recv.name(…)` | a method of `#[named(trait RecvNameExt)] impl Recv` (see below) |
//! | `val NAME: T = e`, `val NAME = e` (top level) | `const` (numbers, booleans, characters, a string literal), a `static` (what a `const fn` makes: `compositionLocalOf { … }`, known by its address), else a `thread_local!` made at first read; its type its value's when not written (a generic's told by a lambda's value or a type argument), a camelCase name in snake_case |
//! | a name whose declaration isn't seen (another file's `val`, a Rust `thread_local!` or constant) | `kt::top_level!(name)`: a thread-local's value, or the value, cloned; a property of it `kt::top_level_with!(Name, |v| v.prop())`, lent in place; an infix on it in place (`Name.provides(v)`) |
//! | `Int Long Float Double Boolean Char String Unit` | `i32 i64 f32 f64 bool char String ()` |
//! | `T?`, `List<T>`, `Map<K, V>`, `Set<T>`, `Pair<A, B>` | `Option<T>`, `Vec<T>`, `kt::KtMap<K, V>`, `kt::KtSet<T>`, `(A, B)` |
//! | parameter `String`, `T?`, `(A) -> R` | `impl Into<String>`, `impl Into<Option<T>>`, `impl Fn(A) -> R + 'static` |
//! | parameter of an interface the API has as a trait (`shape: Shape`) | its object shared as the API holds it (`Arc<dyn Shape>`) |
//! | `val x = e`, `var x = e` | `let x = e;`, `let mut x = e;` (a value read again is cloned: Kotlin shares values) |
//! | `var x by remember { mutableStateOf(0) }` | `let x = remember(|| mutable_state_of(0));`, reads `KtDelegate::get_value(&x)`, writes `KtMutableDelegate::set_value(&x, v)`, `x++` → `post_inc(&x)` |
//! | `"a $b ${c}"` | `kt::template!("a ", b, c)` (Kotlin's `toString()` of each) |
//! | `1`, `1L`, `1f`, `1.0`, `0xFF`, `16.dp` | `1`, `1_i64`, `1_f32`, `1.0`, `0xFF`, `16.dp()` |
//! | `a..b`, `a until b`, `a downTo b`, `r step n`, `x in r` | `kt::range_to`, `kt::until`, `kt::down_to`, `kt::step`, `kt::is_in` |
//! | `a?.b`, `a ?: b`, `a!!`, `null` | `kt::safe!(a, |it| it.b())`, `kt::elvis(a, || b)` (a `match` for `?: return`), `kt::not_null(a)`, `None` |
//! | `if (x != null)` on a local | `if let Some(x) = x.clone()` (Kotlin's smart cast) |
//! | `when (x) { a, b -> …; in r -> …; else -> … }` | an `if` chain on the subject |
//! | `for (x in xs)`, `while`, `do … while`, labels | `for x in KtIterable::kt_iter(&xs)` (a range as it is), `while`, `loop`, `'label:` |
//! | `f(a, name = b) { … }` | `f(a, name = b, last = || …)`: named arguments and defaults through the callee's builder |
//! | `a.b`, `a.b = v` | `a.b()`, `a.set_b(v)` (properties one form); `Type.X` → `Type::X`; `Modifier.Companion` → `Modifier`; a nested object Rust names otherwise (`Arrangement.Absolute.Left` → `AbsoluteArrangement::Left`) |
//! | an argument of another type than its parameter's | what converts it: `Into::into` (a `Stroke` given as a `DrawStyle`), `Rc::new`/`Arc::new` (a value given as an interface), `as i32` (another integer type given as an `Int`), `&v` (a list given as a slice) |
//! | Kotlin's standard members (`xs.map { }`, `s.length`) | the kt shims' traits, by the receiver's type when it is known (`KtList::map(&xs, |it| …)`) |
//! | lambdas | closures: borrowing where the callee runs them before returning (Kotlin's inline functions), else capturing clones (`#[kompozable]` does that for the calls it rewrites); a shared one (`Callback`, `Rc<dyn Fn(&T)>`) made one, its parameters typed; a parameter given a `&mut` (`state.update { it.add(…) }`) reborrowed |
//! | receiver lambdas (`Canvas { drawCircle(…) }`) | a closure taking the scope (`|this| this.draw_circle(…)`): free names that are its members resolve through it |
//! | suspend lambdas (`LaunchedEffect(k) { delay(1) }`) | `async move { delay(1).await }`: known suspending calls are awaited |
//!
//! Extension functions: `fun Modifier.framed(width: Dp = 2.dp): Modifier` is a method of an extension trait,
//! `#[named(pub trait ModifierFramedExt)] impl Modifier { fn framed(self, #[default(2.dp())] width: Dp) -> Modifier }`:
//! one trait per function (two files may extend one type with functions of their own), named after the receiver
//! and the function, as visible as the function; `#[named]` gives it named arguments and defaults at its calls. A
//! call (`Modifier.framed()`, or `framed()` inside another extension of `Modifier`) is a method call: the trait is
//! in scope where the function is (Rust code elsewhere imports it, as Kotlin imports an extension).
//!
//! Kotlin's imports and package are not translated: the names come from the Rust module's own `use`s (the
//! prelude: `use kompoz::prelude::*;`), the kt shims' traits from a `use` the generated bodies have.
//!
//! What stage 9 doesn't take (classes, objects, interfaces, enums, `try`/`throw`, `is`/`as`, generic functions,
//! local functions, callable references, …) is an error saying it comes with stage 14, at the Kotlin construct.
//! Errors are per declaration: a declaration with an error is left out, the others are translated.

mod call;
mod expr;
mod lambda;
mod names;
mod resolve;
mod stmt;
mod types;

use crate::Paths;
use crate::ast::*;
use crate::diag::Diagnostics;
use crate::pos::{Loc, SpanMap};
use names::{ident, is_pascal, pascal, snake, valid, value_name};
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use resolve::*;
use std::collections::HashMap;

/// What the generator works with besides the tree.
pub struct Context<'a> {
    pub spans: &'a dyn SpanMap,
    pub paths: &'a Paths,
    /// See `Options::keyed_calls`.
    pub keyed_calls: bool,
}

/// The items `file`'s declarations become; what can't be translated is a diagnostic (and the declaration it is in
/// is left out).
pub fn generate(file: &File, cx: &Context, diags: &mut Diagnostics) -> TokenStream {
    let mut g = Gen::new(cx, diags);
    g.collect(file);
    let mut out = TokenStream::new();
    for decl in &file.decls {
        let before = g.diags.list.len();
        let ts = g.top_decl(decl);
        if !g.diags.list[before..].iter().any(|d| d.is_error()) {
            out.extend(ts);
        }
    }
    out
}

/// The function being written.
#[derive(Clone, Default)]
pub(crate) struct FnState {
    /// Its body is a `#[kompozable]` one: the macro rewrites its calls and wraps its lambdas.
    pub kompozable: bool,
}

/// A loop being written, for `break` and `continue`.
#[derive(Clone)]
pub(crate) struct LoopCx {
    pub label: Option<String>,
    /// A `do … while` whose body is a labeled block (`continue` breaks out of it to the condition).
    pub body_label: Option<String>,
}

/// A lambda being written, for `return@label`.
#[derive(Clone)]
pub(crate) struct LambdaCx {
    pub labels: Vec<String>,
    /// How many loops were open outside it (a `break` can't leave a lambda).
    pub loops: usize,
}

pub(crate) struct Gen<'a, 'd> {
    pub cx: &'a Context<'a>,
    pub diags: &'d mut Diagnostics,
    pub file: FileTable,
    /// Locals and parameters by Kotlin name, innermost scope last.
    pub scopes: Vec<HashMap<String, Local>>,
    pub receivers: Vec<ImplicitReceiver>,
    pub frames: Vec<CaptureFrame>,
    pub fun: FnState,
    /// In an `async` block: suspending calls are awaited.
    pub in_async: bool,
    pub loops: Vec<LoopCx>,
    pub lambdas: Vec<LambdaCx>,
    /// The names of the calls whose arguments are being written (a lambda's implicit label).
    pub call_labels: Vec<String>,
    /// A kt shim's call being written: its receiver's element type and its own (its lambdas' `it`).
    pub elem_hint: Option<(Ty, Ty)>,
    counter: usize,
}

impl<'a, 'd> Gen<'a, 'd> {
    fn new(cx: &'a Context<'a>, diags: &'d mut Diagnostics) -> Self {
        Gen {
            cx,
            diags,
            file: FileTable::default(),
            scopes: Vec::new(),
            receivers: Vec::new(),
            frames: Vec::new(),
            fun: FnState::default(),
            in_async: false,
            loops: Vec::new(),
            lambdas: Vec::new(),
            call_labels: Vec::new(),
            elem_hint: None,
            counter: 0,
        }
    }

    pub fn span(&self, loc: Loc) -> Span {
        self.cx.spans.span_of(loc)
    }

    pub fn rt(&self) -> &TokenStream {
        &self.cx.paths.runtime
    }

    /// `::…::runtime::kt` at `span`.
    pub fn kt(&self, span: Span) -> TokenStream {
        let rt = respan(self.cx.paths.runtime.clone(), span);
        quote_spanned!(span=> #rt::kt)
    }

    /// A call that keeps something in the composition, given its identity where the spans can't (`keyed_calls`):
    /// `key(&(line, column), || call)`.
    pub fn identified(&self, ts: TokenStream, loc: Loc, keeps: bool) -> TokenStream {
        if !(self.cx.keyed_calls && keeps && self.fun.kompozable) {
            return ts;
        }
        let span = self.span(loc);
        let rt = respan(self.rt().clone(), span);
        let (line, column) = (loc.start.line, loc.start.column);
        quote_spanned!(span=> #rt::key(&(#line, #column), || #ts))
    }

    /// A fresh name for something the generator binds (`__kz_w0`).
    pub fn fresh(&mut self, what: &str, span: Span) -> proc_macro2::Ident {
        self.counter += 1;
        proc_macro2::Ident::new(&format!("__kz_{what}{}", self.counter), span)
    }

    /// An error at `loc`.
    pub fn error(&mut self, loc: Loc, message: impl Into<String>) {
        self.diags.error(loc, message);
    }

    /// An error saying `what` comes with stage 14; the tokens standing in for it (`unreachable!()`).
    pub fn later(&mut self, loc: Loc, what: &str) -> TokenStream {
        self.error(loc, format!("{what} isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)"));
        let span = self.span(loc);
        quote_spanned!(span=> ::core::unreachable!())
    }

    /// A Kotlin name as a Rust identifier (raw for a Rust keyword); an error for one Rust can't have.
    pub fn name_ident(&mut self, name: &str, loc: Loc) -> proc_macro2::Ident {
        let span = self.span(loc);
        if !valid(name) {
            self.error(loc, format!("`{name}` can't be a name in Rust"));
            return proc_macro2::Ident::new("__kz_invalid", span);
        }
        ident(name, span)
    }

    /// `f`'s result without the diagnostics it gives (what is written again later, and reported then).
    pub fn quiet<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let n = self.diags.list.len();
        let out = f(self);
        self.diags.list.truncate(n);
        out
    }

    // ---- Scopes ------------------------------------------------------------------------------------------------

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Declares a local (in the innermost scope); its Rust name.
    pub fn declare(&mut self, kotlin: &str, rust: String, kind: LocalKind, ty: Ty) -> String {
        let depth = self.scopes.len() - 1;
        let local = Local { rust: rust.clone(), kind, ty, depth, kept_closure: false, mut_ref: false };
        self.scopes.last_mut().expect("a scope").insert(kotlin.to_string(), local);
        rust
    }

    /// The local a name is, if it is one; a use of it from inside a lambda is recorded as a capture.
    pub fn local(&mut self, kotlin: &str) -> Option<Local> {
        let found = self.scopes.iter().rev().find_map(|s| s.get(kotlin)).cloned()?;
        for f in &mut self.frames {
            if found.depth < f.boundary && !f.used.contains(&found.rust) {
                f.used.push(found.rust.clone());
            }
        }
        Some(found)
    }

    /// Marks a local of the innermost scope as a `&mut` (see `Local::mut_ref`).
    pub fn mark_mut_ref(&mut self, kotlin: &str) {
        if let Some(l) = self.scopes.last_mut().and_then(|s| s.get_mut(kotlin)) {
            l.mut_ref = true;
        }
    }

    /// Whether a name is a local (without recording a capture).
    pub fn is_local(&self, kotlin: &str) -> bool {
        self.scopes.iter().any(|s| s.contains_key(kotlin))
    }

    /// Narrows a local's type in the innermost scope (a smart cast: the same Rust name, its value non-null).
    pub fn narrow(&mut self, kotlin: &str, ty: Ty) {
        if let Some(mut l) = self.scopes.iter().rev().find_map(|s| s.get(kotlin)).cloned() {
            l.ty = ty;
            l.depth = self.scopes.len() - 1;
            self.scopes.last_mut().unwrap().insert(kotlin.to_string(), l);
        }
    }

    /// The innermost receiver (`this`), or the one labeled `label`; its use from a lambda is a capture.
    pub fn receiver(&mut self, label: Option<&str>) -> Option<ImplicitReceiver> {
        let r = match label {
            None => self.receivers.last().cloned(),
            Some(l) => self.receivers.iter().rev().find(|r| r.labels.iter().any(|x| x == l)).cloned(),
        }?;
        for f in &mut self.frames {
            if r.depth < f.boundary && !f.used.contains(&r.rust) {
                f.used.push(r.rust.clone());
            }
        }
        Some(r)
    }

    // ---- The file ----------------------------------------------------------------------------------------------

    /// The file's top-level functions and properties, for the calls and reads that come before them.
    fn collect(&mut self, file: &File) {
        let mut untyped = Vec::new();
        for decl in &file.decls {
            match decl {
                Decl::Function(f) => {
                    let info = self.quiet(|g| g.file_fn(f));
                    self.file.functions.entry(f.name.name.clone()).or_default().push(info);
                }
                Decl::Property(p) => {
                    if let Binding::Name(n) = &p.binding {
                        match &p.ty {
                            Some(t) => {
                                let ty = self.ty_of(t);
                                self.add_val(p, &n.name, ty);
                            }
                            None => untyped.push((p, n)),
                        }
                    }
                }
                _ => {}
            }
        }
        // A `val` without its type written has its value's, as far as the generator can tell: those its value reads
        // first, wherever they stand in the file (Kotlin's top-level properties are the file's, in any order).
        loop {
            let before = untyped.len();
            untyped.retain(|(p, n)| {
                let PropertyInit::Value(init) = &p.init else { return false };
                let ty = self.peek_ty(init);
                if ty.is_vague() {
                    return true;
                }
                self.add_val(p, &n.name, ty);
                false
            });
            if untyped.len() == before {
                break;
            }
        }
    }

    /// A top-level `val` of type `ty`, for its reads.
    fn add_val(&mut self, p: &Property, kotlin: &str, ty: Ty) {
        let form = val_form(p, &ty);
        self.file.vals.insert(kotlin.to_string(), FileVal { rust: value_name(kotlin), ty, form });
    }

    /// What a call of a file's function needs to know of it.
    fn file_fn(&mut self, f: &Function) -> FileFn {
        let kompozable = f.is_kompozable();
        let inline = f.modifiers.has(ModifierKind::Inline);
        let params: Vec<PInfo> = f.params.iter().map(|p| self.param_info(p, inline)).collect();
        let has_defaults = params.iter().any(|p| p.default != PDefault::None);
        let ret = match (&f.result, &f.body) {
            (Some(t), _) => self.ty_of(t),
            (None, Some(FunctionBody::Block(_))) | (None, None) => Ty::Unit,
            (None, Some(FunctionBody::Expr(e))) => {
                // (its parameters and receiver in scope, to tell its expression's type)
                let saved = (std::mem::take(&mut self.scopes), std::mem::take(&mut self.receivers));
                self.scopes = vec![HashMap::new()];
                for p in &f.params {
                    let ty = p.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown);
                    self.declare(&p.name.name, snake(&p.name.name), LocalKind::Param, ty);
                }
                if let Some(n) = f.receiver.as_ref().and_then(|r| r.simple_name()) {
                    let ty = owner_name(last_segment(&n));
                    self.receivers.push(ImplicitReceiver {
                        rust: "self".into(),
                        ty,
                        labels: Vec::new(),
                        depth: 0,
                        by_ref: false,
                    });
                }
                let t = self.peek_ty(e);
                (self.scopes, self.receivers) = saved;
                t
            }
        };
        FileFn {
            rust: fn_name(f),
            params,
            kompozable,
            suspend: f.is_suspend(),
            ret,
            receiver: f.receiver.as_ref().and_then(|r| r.simple_name()).map(|n| owner_name(last_segment(&n))),
            builder: kompozable || has_defaults,
        }
    }

    fn top_decl(&mut self, decl: &Decl) -> TokenStream {
        match decl {
            Decl::Function(f) => self.function(f),
            Decl::Property(p) => self.top_property(p),
            // (its diagnostics are given)
            Decl::Failed { .. } => TokenStream::new(),
            Decl::Class(c) => {
                let what = match c.kind {
                    ClassKind::Interface | ClassKind::FunInterface => "an interface",
                    ClassKind::Enum => "an enum class",
                    ClassKind::Annotation => "an annotation class",
                    ClassKind::Class if c.is_data() => "a data class",
                    ClassKind::Class => "a class",
                };
                self.later(c.loc, what);
                TokenStream::new()
            }
            Decl::Object(o) => {
                self.later(o.loc, "an object declaration");
                TokenStream::new()
            }
            Decl::TypeAlias(a) => {
                self.later(a.loc, "a type alias");
                TokenStream::new()
            }
        }
    }

    // ---- Functions ---------------------------------------------------------------------------------------------

    /// `pub`, `pub(crate)`, nothing; an error for `protected`.
    fn visibility(&mut self, m: &Modifiers, loc: Loc) -> TokenStream {
        let span = self.span(loc);
        match m.visibility() {
            Visibility::Public => quote_spanned!(span=> pub),
            Visibility::Internal => quote_spanned!(span=> pub(crate)),
            Visibility::Private => quote!(),
            Visibility::Protected => {
                let at = m.find(ModifierKind::Protected).unwrap_or(loc);
                self.error(at, "`protected` is for a class's members");
                quote!()
            }
        }
    }

    /// The modifiers a function may have here; the others are errors.
    fn check_fn_modifiers(&mut self, f: &Function) {
        for m in &f.modifiers.list {
            match m.kind {
                ModifierKind::Public
                | ModifierKind::Private
                | ModifierKind::Internal
                | ModifierKind::Protected
                | ModifierKind::Suspend
                | ModifierKind::Inline => {}
                ModifierKind::Infix | ModifierKind::Operator => {
                    self.later(m.loc, &format!("an `{}` function", m.kind.keyword()));
                }
                _ => {
                    self.later(m.loc, &format!("`{}` on a function", m.kind.keyword()));
                }
            }
        }
        if !f.type_params.is_empty() {
            self.later(f.type_params[0].loc, "a generic function");
        }
        if let Some(c) = f.constraints.first() {
            self.later(c.loc, "a `where` clause");
        }
        if f.body.is_none() {
            self.error(f.name.loc, "a function here needs a body");
        }
    }

    fn function(&mut self, f: &Function) -> TokenStream {
        self.check_fn_modifiers(f);
        let span = self.span(f.loc);
        let kompozable = f.is_kompozable();
        let inline = f.modifiers.has(ModifierKind::Inline);
        let suspend = f.is_suspend();
        if kompozable && suspend {
            self.error(f.loc, "a composable can't be `suspend`");
        }
        let info = self.quiet(|g| g.file_fn(f));
        let unit = info.ret == Ty::Unit;
        let restartable = kompozable && !inline && unit && f.result.as_ref().is_none_or(|t| self.ty_of(t) == Ty::Unit);
        self.fun = FnState { kompozable };
        self.scopes = vec![HashMap::new()];
        self.receivers.clear();
        self.frames.clear();
        self.loops.clear();
        self.lambdas.clear();
        self.in_async = suspend;

        // The receiver of an extension function: `self`.
        let mut self_ty = None;
        if let Some(r) = &f.receiver {
            match (r.simple_name(), r.nullable, &r.kind) {
                (Some(n), false, TypeKind::Named(segs)) if segs.iter().all(|s| s.args.is_empty()) => {
                    let rust_ty = self.rust_type(r, types::TypePos::Value);
                    let name = owner_name(last_segment(&n));
                    self.receivers.push(ImplicitReceiver {
                        rust: "self".into(),
                        ty: name.clone(),
                        labels: vec![f.name.name.clone()],
                        depth: 0,
                        by_ref: false,
                    });
                    self.declare("this", "self".into(), LocalKind::Param, self.ty_of(r));
                    self_ty = Some((rust_ty, name));
                }
                _ => {
                    self.later(r.loc, "an extension of a nullable or generic type");
                }
            }
        }

        // Parameters.
        let mut params = Vec::new();
        let mut converts = Vec::new();
        for p in &f.params {
            let (tokens, convert) = self.param(p, restartable, inline);
            params.push(tokens);
            converts.extend(convert);
        }
        let ret = match &f.result {
            Some(t) if self.ty_of(t) != Ty::Unit => {
                let rt = self.rust_type(t, types::TypePos::Return);
                let s = self.span(t.loc);
                quote_spanned!(s=> -> #rt)
            }
            Some(_) => quote!(),
            None => match &f.body {
                Some(FunctionBody::Expr(e)) => match &info.ret {
                    Ty::Unit => quote!(),
                    t if t.is_vague() => {
                        self.error(
                            f.name.loc,
                            format!(
                                "write `{}`'s return type: its expression's type isn't one KompozR can tell (`fun {}(…): Type = …`)",
                                f.name.name, f.name.name
                            ),
                        );
                        quote!()
                    }
                    ty => {
                        let s = self.span(e.loc);
                        let t = self.rust_of_ty(ty, s);
                        quote_spanned!(s=> -> #t)
                    }
                },
                _ => quote!(),
            },
        };

        let body = match &f.body {
            Some(FunctionBody::Block(b)) => self.fn_block(b, unit),
            Some(FunctionBody::Expr(e)) => {
                if unit {
                    let s = self.stmt_expr(e);
                    quote!(#s;)
                } else {
                    self.value(e)
                }
            }
            None => quote!(),
        };
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let empty = matches!(&f.body, Some(FunctionBody::Block(b)) if b.stmts.is_empty()) && converts.is_empty();
        let body = if empty {
            quote_spanned!(span=> {})
        } else if kompozable {
            quote_spanned!(span=> {
                #[allow(unused_imports)]
                use #kt::ext::*;
                #(#converts)*
                #body
            })
        } else {
            quote_spanned!(span=> {
                #[allow(unused_imports)]
                use #kt::ext::*;
                #(#converts)*
                #rt::named_args!({ #body })
            })
        };

        let vis = self.visibility(&f.modifiers, f.loc);
        let name = if kompozable || is_pascal(&f.name.name) {
            self.name_ident(&f.name.name, f.name.loc)
        } else {
            self.name_ident(&snake(&f.name.name), f.name.loc)
        };
        let fn_kw = quote_spanned!(span=> fn);
        let asyncness = if suspend { quote_spanned!(span=> async) } else { quote!() };
        let allow = allow_attr(span);
        let has_defaults = info.params.iter().any(|p| p.default != PDefault::None);

        if let Some((self_ty, owner)) = self_ty {
            let trait_name = proc_macro2::Ident::new(&format!("{owner}{}Ext", pascal(&f.name.name)), span);
            let trait_vis = match f.modifiers.visibility() {
                Visibility::Private => quote_spanned!(span=> pub(self)),
                _ => vis.clone(),
            };
            if kompozable {
                return self.later(f.loc, "a composable extension function");
            }
            return quote_spanned! {span=>
                #allow
                #[#rt::named(#trait_vis trait #trait_name)]
                impl #self_ty {
                    #asyncness #fn_kw #name(self, #(#params),*) #ret #body
                }
            };
        }
        let attr = if kompozable {
            if inline || !unit {
                quote_spanned!(span=> #[#rt::kompozable(inline)])
            } else {
                quote_spanned!(span=> #[#rt::kompozable])
            }
        } else if has_defaults {
            quote_spanned!(span=> #[#rt::named])
        } else {
            quote!()
        };
        quote_spanned! {span=>
            #allow
            #attr
            #vis #asyncness #fn_kw #name(#(#params),*) #ret #body
        }
    }

    /// A function's block body: its statements, the last one its value unless it returns nothing (a `return` does
    /// otherwise).
    fn fn_block(&mut self, b: &Block, unit: bool) -> TokenStream {
        self.block_inner(b, if unit { stmt::Tail::Unit } else { stmt::Tail::Return })
    }

    /// A parameter: its tokens (`#[default(d)] name: Type`), and what converts it in the body (an `impl Into<T>`
    /// made a `T`, which a restartable composable's macro does itself).
    fn param(&mut self, p: &Param, restartable: bool, inline: bool) -> (TokenStream, Option<TokenStream>) {
        let span = self.span(p.loc);
        if p.is_vararg() {
            self.later(p.loc, "a `vararg` parameter");
        }
        let rust = snake(&p.name.name);
        let name = self.name_ident(&rust, p.name.loc);
        let Some(t) = &p.ty else {
            self.error(p.loc, "a parameter needs a type");
            return (quote!(), None);
        };
        let ty = self.ty_of(t);
        let pos = if p.default.is_some() { types::TypePos::DefaultedParam } else { types::TypePos::Param };
        let rust_ty = self.rust_type(t, pos);
        let info = self.quiet(|g| g.param_info(p, inline));
        let default = p.default.as_ref().map(|d| {
            let saved = std::mem::take(&mut self.scopes);
            self.scopes = vec![HashMap::new()];
            let v = self.value_for(d, Some(&info));
            self.scopes = saved;
            let s = self.span(d.loc);
            quote_spanned!(s=> #[default(#v)])
        });
        let kept_closure = restartable && matches!(t.kind, TypeKind::Function(_)) && !t.nullable;
        let into = types::into_target(&rust_ty);
        let local_ty = ty.clone();
        self.declare(&p.name.name, rust.clone(), LocalKind::Param, local_ty);
        if kept_closure && let Some(l) = self.scopes.last_mut().unwrap().get_mut(&p.name.name) {
            l.kept_closure = true;
        }
        let convert = match into {
            Some(target) if !restartable => Some(quote_spanned!(span=>
                let #name: #target = ::core::convert::Into::into(#name);
            )),
            _ => None,
        };
        (quote_spanned!(span=> #default #name: #rust_ty), convert)
    }

    /// What a call sees of a parameter of the file's own function.
    pub(crate) fn param_info(&mut self, p: &Param, inline: bool) -> PInfo {
        let (ty_text, lambda, option) = match &p.ty {
            Some(t) => {
                let pos = if p.default.is_some() { types::TypePos::DefaultedParam } else { types::TypePos::Param };
                let rust = self.rust_type(t, pos).to_string();
                let lambda = t.as_function().filter(|_| !t.nullable).map(|ft| self.lambda_info(ft, !inline));
                let option = match (t.nullable, p.default.is_some()) {
                    (false, _) => OptionForm::No,
                    (true, false) => OptionForm::IntoOption,
                    (true, true) => OptionForm::Option,
                };
                (rust, lambda, option)
            }
            None => (String::new(), None, OptionForm::No),
        };
        PInfo {
            kotlin: p.name.name.clone(),
            rust: snake(&p.name.name),
            ty: ty_text,
            default: if p.default.is_some() { PDefault::Builder } else { PDefault::None },
            vararg: p.is_vararg(),
            lambda,
            option,
        }
    }

    // ---- Top-level properties ----------------------------------------------------------------------------------

    fn top_property(&mut self, p: &Property) -> TokenStream {
        let span = self.span(p.loc);
        let Binding::Name(n) = &p.binding else {
            return self.later(p.loc, "a top-level destructuring declaration");
        };
        if p.is_var() {
            return self.later(p.loc, "a top-level `var`");
        }
        if p.receiver.is_some() {
            return self.later(p.loc, "an extension property");
        }
        if p.getter.is_some() || p.setter.is_some() {
            return self.later(p.loc, "a property's getter or setter");
        }
        let init = match &p.init {
            PropertyInit::Value(e) => e,
            PropertyInit::Delegate(e) => return self.later(e.loc, "a delegated top-level property"),
            PropertyInit::None => {
                self.error(n.loc, format!("`{}` needs a value", n.name));
                return quote!();
            }
        };
        self.fun = FnState::default();
        self.scopes = vec![HashMap::new()];
        self.receivers.clear();
        // (its type written, or its value's)
        let Some(val) = self.file.vals.get(&n.name).cloned() else {
            self.error(
                n.loc,
                format!("write `{}`'s type: KompozR can't tell its value's type (`val {}: Type = …`)", n.name, n.name),
            );
            return quote!();
        };
        let vis = self.visibility(&p.modifiers, p.loc);
        let name = self.name_ident(&val.rust, n.loc);
        let rt = match &p.ty {
            Some(t) => self.rust_type(t, types::TypePos::Value),
            None => self.rust_of_ty(&val.ty, self.span(n.loc)),
        };
        let allow = allow_attr(span);
        match val.form {
            ValForm::Const => {
                let v = self.value(init);
                quote_spanned!(span=> #allow #vis const #name: #rt = #v;)
            }
            ValForm::StrConst => {
                let v = self.expr(init).ts;
                quote_spanned!(span=> #allow #vis const #name: &str = #v;)
            }
            ValForm::Static => {
                let v = self.value(init);
                let kt = self.kt(span);
                quote_spanned! {span=>
                    #allow
                    #vis static #name: #rt = {
                        #[allow(unused_imports)]
                        use #kt::ext::*;
                        #v
                    };
                }
            }
            ValForm::ThreadLocal => {
                let v = self.value(init);
                let kt = self.kt(span);
                let rtp = respan(self.rt().clone(), span);
                quote_spanned! {span=>
                    ::std::thread_local! {
                        #allow
                        #vis static #name: #rt = {
                            #[allow(unused_imports)]
                            use #kt::ext::*;
                            #rtp::named_args!(#v)
                        };
                    }
                }
            }
        }
    }
}

/// How a top-level `val` of type `ty` is held.
fn val_form(p: &Property, ty: &Ty) -> ValForm {
    let literal = |e: &Expr| -> bool {
        fn lit(e: &Expr) -> bool {
            match &e.kind {
                ExprKind::Number(_) | ExprKind::Bool(_) | ExprKind::Char(_) => true,
                ExprKind::Prefix { op: PrefixOp::Minus | PrefixOp::Plus, operand } => lit(operand),
                ExprKind::Paren(x) => lit(x),
                ExprKind::Binary {
                    op: BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem,
                    lhs,
                    rhs,
                } => lit(lhs) && lit(rhs),
                _ => false,
            }
        }
        lit(e)
    };
    let PropertyInit::Value(init) = &p.init else { return ValForm::ThreadLocal };
    // (made by a `const fn` of the API from literals and lambdas: `compositionLocalOf { … }`, a static, known by its
    // address as a local is)
    let made_const = match &init.kind {
        ExprKind::Call { callee, type_args: _, args, trailing_lambda: _ } => {
            matches!(&callee.kind, ExprKind::Name(n) if free_functions(&n.name).any(|i| i.has(crate::index::flags::CONST)))
                && args.iter().all(|a| literal(&a.value) || matches!(a.value.kind, ExprKind::Lambda(_)))
        }
        _ => false,
    };
    match ty {
        Ty::Int | Ty::Long | Ty::Float | Ty::Double | Ty::Bool | Ty::Char if literal(init) => ValForm::Const,
        Ty::String if matches!(&init.kind, ExprKind::String(s) if s.parts.iter().all(|p| matches!(p, StringPart::Text { .. }))) => {
            ValForm::StrConst
        }
        _ if made_const => ValForm::Static,
        _ => ValForm::ThreadLocal,
    }
}

/// A function's Rust name: a composable's or a PascalCase one as it is, others in snake_case.
fn fn_name(f: &Function) -> String {
    if f.is_kompozable() || is_pascal(&f.name.name) { f.name.name.clone() } else { snake(&f.name.name) }
}

/// The last segment of a dotted name.
pub(crate) fn last_segment(dotted: &str) -> &str {
    dotted.rsplit('.').next().unwrap_or(dotted)
}

/// The lints generated code may trip that the Kotlin it comes from doesn't make wrong: it is written for Kotlin's
/// meaning, not Rust's style.
fn allow_attr(span: Span) -> TokenStream {
    quote_spanned! {span=>
        #[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
    }
}

/// A loop or block label, `'name`.
pub(crate) fn lifetime(name: &str, span: Span) -> TokenStream {
    let tick = proc_macro2::Punct::new('\'', proc_macro2::Spacing::Joint);
    let mut tick = proc_macro2::TokenTree::Punct(tick);
    tick.set_span(span);
    let i = proc_macro2::Ident::new(name, span);
    quote!(#tick #i)
}

/// `tokens` with every span made `span`.
pub(crate) fn respan(tokens: TokenStream, span: Span) -> TokenStream {
    tokens
        .into_iter()
        .map(|t| match t {
            proc_macro2::TokenTree::Group(g) => {
                let mut group = proc_macro2::Group::new(g.delimiter(), respan(g.stream(), span));
                group.set_span(span);
                proc_macro2::TokenTree::Group(group)
            }
            mut t => {
                t.set_span(span);
                t
            }
        })
        .collect()
}
