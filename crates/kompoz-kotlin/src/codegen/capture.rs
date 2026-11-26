//! What lambdas and local functions capture (stage 14 WPB): the object a member's kept lambda uses (its `self`
//! cloned, `__kz_self`), and Kotlin's capture of a local `var` by reference (U2): a `var` a kept lambda or a local
//! function assigns is held in a `kt::Ref` (see `kompoz_runtime::kt::reference`), the lambda capturing a clone of
//! the cell, every read and write going through it. Which ones those are is known once the declaration has been
//! written: it is written again with them shared ([`Gen::shared_pass`]).

use super::Gen;
use crate::pos::Loc;
use proc_macro2::{Ident, TokenStream, TokenTree};

/// `tokens` with the identifier `self` renamed `__kz_self` (a kept closure's body using the object it holds).
pub(crate) fn renamed_self(tokens: TokenStream) -> TokenStream {
    tokens
        .into_iter()
        .map(|t| match t {
            TokenTree::Ident(i) if i == "self" => TokenTree::Ident(Ident::new("__kz_self", i.span())),
            TokenTree::Group(g) => {
                let mut n = proc_macro2::Group::new(g.delimiter(), renamed_self(g.stream()));
                n.set_span(g.span());
                TokenTree::Group(n)
            }
            t => t,
        })
        .collect()
}

/// What a function written as a closure is (a local function, an anonymous function).
pub(crate) struct ClosureFn {
    /// The closure (`move |a: A| -> R { … }`, its captures cloned before it).
    pub closure: TokenStream,
    /// Its parameters' and result's Rust types (for `Rc<dyn Fn(A) -> R>`).
    pub params: Vec<TokenStream>,
    pub ret: Option<TokenStream>,
    pub ty: super::resolve::Ty,
}

/// A local's identity across passes: where its name is.
pub(crate) fn var_key(at: Loc) -> (u32, u32) {
    (at.start.line, at.start.column)
}

impl Gen<'_, '_> {
    /// A declaration written by `write`, again when a `var` it declares turned out to be assigned by a lambda kept
    /// or a local function (`Control::shared_found`): then with those `var`s shared. The second pass's diagnostics
    /// are the ones reported.
    pub(crate) fn shared_pass(&mut self, write: impl Fn(&mut Self) -> TokenStream) -> TokenStream {
        self.control.shared_found = false;
        self.control.assigned_vars.clear();
        self.control.captured_vars.clear();
        let before = self.diags.list.len();
        let counter = self.counter_now();
        let recorded =
            self.analysis.as_ref().map(|a| (a.declarations.len(), a.locals.len(), a.references.len(), a.types.len()));
        let ts = write(self);
        // (a `var` assigned and captured by a kept lambda or a local function: Kotlin's lambda sees what it is, the
        // code around it what the lambda makes it)
        let both: Vec<(u32, u32)> =
            self.control.assigned_vars.intersection(&self.control.captured_vars).copied().collect();
        for k in both {
            if self.control.shared_vars.insert(k) {
                self.control.shared_found = true;
            }
        }
        if !self.control.shared_found {
            return ts;
        }
        self.diags.list.truncate(before);
        if let (Some(a), Some((d, l, r, t))) = (self.analysis.as_mut(), recorded) {
            a.declarations.truncate(d);
            a.locals.truncate(l);
            a.references.truncate(r);
            a.types.truncate(t);
        }
        self.control.shared_found = false;
        self.set_counter(counter);
        write(self)
    }

    /// Whether the `var` declared at `at` is shared (a `kt::Ref`).
    pub(crate) fn is_shared_var(&self, at: Loc) -> bool {
        self.control.shared_vars.contains(&var_key(at))
    }

    // ---- Callable references ------------------------------------------------------------------------------------

    /// A callable reference: `::f` (a function of the file, a local function), `x::f` (bound to the value `x`),
    /// `Type::f` (the receiver its first parameter), of a function or a property: the lambda calling it
    /// (`{ a, b -> f(a, b) }`, typed by the function's parameters where the file declares it).
    pub(crate) fn callable_ref(
        &mut self,
        receiver: Option<&crate::ast::Expr>,
        name: &crate::ast::Ident,
        loc: Loc,
    ) -> super::expr::Out {
        use super::resolve::*;
        use crate::ast::*;
        let span = self.span(loc);
        let n = |s: &str| Ident { name: s.to_string(), backticked: false, loc };
        let ex = |kind: ExprKind| Expr { kind, loc };
        // (`::local`: the local function's value itself)
        if receiver.is_none() && self.is_local(&name.name) {
            return self.expr(&ex(ExprKind::Name(name.clone())));
        }
        // What it calls: its parameters (Kotlin's types where known), and whether a receiver comes first.
        let (params, receiver_param, call_on): (Vec<Option<Type>>, Option<Type>, Option<Expr>) = match receiver {
            None => match self.control.fn_params.get(&name.name).map(Vec::as_slice) {
                Some([one]) => (one.iter().map(|p| p.ty.clone()).collect(), None, None),
                Some([]) | None => {
                    let item = lookup(&name.name).iter().find(|i| i.kind == crate::index::Kind::Function);
                    match item {
                        Some(i) => (vec![None; i.params.len()], None, None),
                        None => {
                            self.error(loc, format!("`::{}` names no function KompozR knows", name.name));
                            return super::expr::Out::plain(quote::quote!(::core::unreachable!()), Ty::Unknown);
                        }
                    }
                }
                // (one of several overloads: the one whose parameters the function type it goes to has)
                Some(all) => {
                    let expected = self.control.expected_fn.clone();
                    let chosen = expected.and_then(|want| {
                        all.iter()
                            .find(|ps| {
                                ps.len() == want.len()
                                    && ps
                                        .iter()
                                        .zip(&want)
                                        .all(|(p, w)| p.ty.as_ref().is_some_and(|t| self.ty_of(t) == *w))
                            })
                            .cloned()
                    });
                    match chosen {
                        Some(ps) => (ps.iter().map(|p| p.ty.clone()).collect(), None, None),
                        None => {
                            self.error(
                                loc,
                                format!(
                                    "`::{}` names one of several overloads and no function type here says which (give \
                                     it to a parameter or `val` of the function's type, or write the lambda calling \
                                     the one meant)",
                                    name.name
                                ),
                            );
                            return super::expr::Out::plain(quote::quote!(::core::unreachable!()), Ty::Unknown);
                        }
                    }
                }
            },
            Some(r) => {
                // (`Type::f`: a type's name; `x::f`: a value)
                let type_name = match &r.kind {
                    ExprKind::Name(t) if super::names::is_pascal(&t.name) && !self.is_local(&t.name) => Some(t.clone()),
                    _ => None,
                };
                let owner = match &type_name {
                    Some(t) => self.user_class(&t.name).map(|c| c.rust.clone()).unwrap_or(t.name.clone()),
                    None => self.peek_ty(r).non_null().owner().unwrap_or("").to_string(),
                };
                let arity = members_of(&owner, &name.name)
                    .next()
                    .map(|i| i.params.len())
                    .or_else(|| self.file.extension(&name.name, Some(&owner)).map(|f| f.params.len()))
                    .unwrap_or(0);
                match type_name {
                    Some(t) => {
                        let ty = Type {
                            kind: TypeKind::Named(vec![TypeSegment { name: t.clone(), args: Vec::new() }]),
                            nullable: false,
                            annotations: Vec::new(),
                            loc,
                        };
                        (vec![None; arity], Some(ty), None)
                    }
                    None => (vec![None; arity], None, Some(r.clone())),
                }
            }
        };
        let mut lparams = Vec::new();
        let mut args = Vec::new();
        let target = match (&receiver_param, &call_on) {
            (Some(t), _) => {
                lparams.push(LambdaParam { binding: Binding::Name(n("__kz_r")), ty: Some(t.clone()), loc });
                Some(ex(ExprKind::Name(n("__kz_r"))))
            }
            (None, Some(e)) => Some(e.clone()),
            (None, None) => None,
        };
        for (k, t) in params.iter().enumerate() {
            let p = format!("__kz_p{k}");
            lparams.push(LambdaParam { binding: Binding::Name(n(&p)), ty: t.clone(), loc });
            args.push(Arg { name: None, spread: false, value: ex(ExprKind::Name(n(&p))), loc });
        }
        let is_property = target.as_ref().is_some_and(|_| {
            params.is_empty()
                && match (&receiver_param, &call_on) {
                    (Some(t), _) => {
                        let owner = t.simple_name().unwrap_or_default();
                        let owner = self.user_class(&owner).map(|c| c.rust.clone()).unwrap_or(owner);
                        members_of(&owner, &name.name).next().is_some_and(|i| i.kind == crate::index::Kind::Property)
                    }
                    (None, Some(e)) => self.peek_ty(e).owner().is_some_and(|o| {
                        members_of(o, &name.name).next().is_some_and(|i| i.kind == crate::index::Kind::Property)
                    }),
                    _ => false,
                }
        });
        let body = match target {
            Some(t) if is_property => ex(ExprKind::Member { receiver: Box::new(t), safe: false, name: name.clone() }),
            Some(t) => ex(ExprKind::Call {
                callee: Box::new(ex(ExprKind::Member { receiver: Box::new(t), safe: false, name: name.clone() })),
                type_args: Vec::new(),
                args,
                trailing_lambda: None,
            }),
            None => ex(ExprKind::Call {
                callee: Box::new(ex(ExprKind::Name(name.clone()))),
                type_args: Vec::new(),
                args,
                trailing_lambda: None,
            }),
        };
        let lambda = Lambda {
            label: None,
            params: Some(lparams),
            body: Block { stmts: vec![Stmt { kind: StmtKind::Expr(body), loc }], braced: true, loc },
            loc,
        };
        let _ = span;
        self.expr(&ex(ExprKind::Lambda(lambda)))
    }

    // ---- Local and anonymous functions --------------------------------------------------------------------------

    /// A local function, `fun name(…) { … }` in a block: a shared closure (`Rc<dyn Fn(A) -> R>`) holding what it
    /// uses (clones; the `var`s it assigns or reads shared, as Kotlin's local function captures them); one calling
    /// itself reaches itself through a weak reference it is given once made. A `suspend` one gives a boxed future
    /// (its calls awaited); an extension one (`fun Int.twice() = this * 2`) takes its receiver first, called as a
    /// method is (`x.twice()`); its parameters' defaults are filled in where it is called. A generic or composable
    /// one is a nested `fn` item (a closure can't be generic, a composable is a function): it can use none of the
    /// locals around it.
    pub(crate) fn local_function(&mut self, f: &crate::ast::Function) -> TokenStream {
        use quote::quote_spanned;
        let span = self.span(f.loc);
        if !f.type_params.is_empty() {
            return self.generic_local_function(f);
        }
        // ((stage 14 WPD) a composable one too: a composable is a function of its own, the composer's group)
        if f.is_kompozable() {
            return self.generic_local_function(f);
        }
        if let Some(p) = f.params.iter().find(|p| p.is_vararg()) {
            return self.later(p.loc, "a local function's `vararg` parameter");
        }
        let rust = super::names::snake(&f.name.name);
        let name = self.name_ident(&rust, f.name.loc);
        // (its name first, in its own body too: a call of itself)
        let sig = self.quiet(|g| g.closure_fn_ty(f));
        self.declare_at(&f.name, rust.clone(), super::resolve::LocalKind::Val, sig.clone());
        let used: std::rc::Rc<std::cell::Cell<bool>> = Default::default();
        if let Some(l) = self.scopes.last_mut().and_then(|s| s.get_mut(&f.name.name)) {
            l.shared_closure = true;
            l.used = Some(used.clone());
        }
        // (where it is called: its parameters (their defaults), its receiver's type)
        self.control.local_fns.insert(rust.clone(), f.params.clone());
        if let Some(r) = f.receiver.as_ref().and_then(|r| r.simple_name()) {
            let owner = super::resolve::owner_name(super::last_segment(&r));
            self.control.local_exts.push((f.name.name.clone(), owner));
        }
        used.set(false);
        let Some(c) = self.closure_fn(f, Some((&rust, used.clone()))) else { return quote::quote!() };
        let ClosureFn { closure, params, ret, .. } = c;
        let ret_ts = ret.map(|r| quote_spanned!(span=> -> #r));
        let dyn_ty = quote_spanned!(span=> ::std::rc::Rc<dyn ::core::ops::Fn(#(#params),*) #ret_ts>);
        if used.get() {
            let weak = quote_spanned!(span=> ::std::rc::Weak<dyn ::core::ops::Fn(#(#params),*) #ret_ts>);
            quote_spanned! {span=>
                let #name: #dyn_ty = {
                    let __kz_slot: ::std::rc::Rc<::std::cell::OnceCell<#weak>> =
                        ::std::rc::Rc::new(::std::cell::OnceCell::new());
                    let __kz_f: #dyn_ty = ::std::rc::Rc::new(#closure);
                    let _ = __kz_slot.set(::std::rc::Rc::downgrade(&__kz_f));
                    __kz_f
                };
            }
        } else {
            quote_spanned!(span=> let #name: #dyn_ty = ::std::rc::Rc::new(#closure);)
        }
    }

    /// A generic or composable local function: a nested `fn` item, written as a top-level function is (it can't use
    /// the locals around it: an error naming the first it uses).
    fn generic_local_function(&mut self, f: &crate::ast::Function) -> TokenStream {
        // (what it uses of the scopes around it, told by a quiet writing of it as a closure)
        let used = self.quiet(|g| {
            let mut probe = f.clone();
            probe.type_params.clear();
            g.frames.push(super::resolve::CaptureFrame { boundary: g.scopes.len(), used: Vec::new(), kept: true });
            let _ = g.closure_fn(&probe, None);
            g.frames.pop().map(|fr| fr.used).unwrap_or_default()
        });
        if let Some(u) = used.iter().find(|u| *u != "self") {
            let msg = if f.is_kompozable() {
                format!(
                    "a local composable function can't use the locals around it (`{u}`): KompozR's composables are \
                     functions, not closures (pass it as a parameter)"
                )
            } else {
                format!(
                    "a generic local function can't use the locals around it (`{u}`): Rust's generic functions are \
                     items, not closures (pass it as a parameter, or make the function not generic)"
                )
            };
            self.error(f.name.loc, msg);
            return quote::quote!();
        }
        // (written as the file's function is, the state of the function around it kept)
        let saved = (
            std::mem::take(&mut self.scopes),
            std::mem::take(&mut self.receivers),
            std::mem::take(&mut self.frames),
            std::mem::take(&mut self.loops),
            std::mem::take(&mut self.lambdas),
            self.fun.clone(),
            self.in_async,
            self.inline_async,
            std::mem::take(&mut self.control.jump_frames),
        );
        let ts = self.function(f);
        (self.scopes, self.receivers, self.frames, self.loops, self.lambdas) =
            (saved.0, saved.1, saved.2, saved.3, saved.4);
        (self.fun, self.in_async, self.inline_async, self.control.jump_frames) = (saved.5, saved.6, saved.7, saved.8);
        ts
    }

    /// An anonymous function, `fun(x: Int): Int { return x * 2 }`: a closure as a lambda kept is (its `return` its
    /// own).
    pub(crate) fn anonymous_function(&mut self, f: &crate::ast::Function) -> super::expr::Out {
        match self.closure_fn(f, None) {
            Some(c) => super::expr::Out::plain(c.closure, c.ty),
            None => super::expr::Out::plain(quote::quote!(::core::unreachable!()), super::resolve::Ty::Unknown),
        }
    }

    /// A function's type (`(Int) -> String`; an extension's receiver its first parameter) as the generator's.
    fn closure_fn_ty(&mut self, f: &crate::ast::Function) -> super::resolve::Ty {
        use super::resolve::{FunTy, Ty};
        let mut params: Vec<Ty> = Vec::new();
        if let Some(r) = &f.receiver {
            params.push(self.ty_of(r));
        }
        params.extend(f.params.iter().map(|p| p.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown)));
        let ret = match (&f.result, &f.body) {
            (Some(t), _) => self.ty_of(t),
            (None, Some(crate::ast::FunctionBody::Expr(e))) => {
                self.push_scope();
                let receivers = self.receivers.len();
                if let Some(r) = &f.receiver {
                    self.push_local_receiver(r);
                }
                for p in &f.params {
                    let ty = p.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown);
                    self.declare(&p.name.name, super::names::snake(&p.name.name), super::resolve::LocalKind::Param, ty);
                }
                let t = self.peek_ty(e);
                self.receivers.truncate(receivers);
                self.pop_scope();
                t
            }
            _ => Ty::Unit,
        };
        Ty::Fun(Box::new(FunTy { params, ret, receiver: None, suspend: f.is_suspend(), shared: true }))
    }

    /// A local extension function's receiver, `this` (its closure's first parameter, `__kz_recv`).
    fn push_local_receiver(&mut self, r: &crate::ast::Type) {
        let owner = r.simple_name().map(|n| super::resolve::owner_name(super::last_segment(&n))).unwrap_or_default();
        let depth = self.scopes.len().saturating_sub(1);
        self.receivers.push(super::resolve::ImplicitReceiver {
            rust: "__kz_recv".into(),
            ty: owner,
            value: self.ty_of(r),
            labels: Vec::new(),
            depth,
            by_ref: false,
            by_mut: false,
            owned_used: Default::default(),
        });
        let value = self.ty_of(r);
        self.declare("this", "__kz_recv".into(), super::resolve::LocalKind::Param, value);
    }

    /// A function's body as a kept closure (`own`: the local function's Rust name, which it reaches itself by, and
    /// the flag set when it does: through `__kz_slot`'s weak reference).
    fn closure_fn(
        &mut self,
        f: &crate::ast::Function,
        own: Option<(&str, std::rc::Rc<std::cell::Cell<bool>>)>,
    ) -> Option<ClosureFn> {
        let own_flag = own.as_ref().map(|(_, u)| u.clone());
        let own = own.map(|(n, _)| n);
        use super::resolve::*;
        use super::types::TypePos;
        use crate::ast::*;
        use quote::{quote, quote_spanned};
        let span = self.span(f.loc);
        let suspend = f.is_suspend();
        let ty = self.quiet(|g| g.closure_fn_ty(f));
        let Ty::Fun(ft) = &ty else { return None };
        let ret_ty = ft.ret.clone();
        let mut params = Vec::new();
        if let Some(r) = &f.receiver {
            params.push(self.rust_type(r, TypePos::Value));
        }
        for p in &f.params {
            let Some(t) = &p.ty else {
                self.error(p.loc, "a parameter needs a type");
                return None;
            };
            params.push(self.rust_type(t, TypePos::Value));
        }
        let value_ret = match (&f.result, &ret_ty) {
            (_, Ty::Unit) => None,
            (Some(t), _) => Some(self.rust_type(t, TypePos::Value)),
            (None, t) if !t.is_vague() => Some(self.rust_of_ty(t, span)),
            (None, _) => {
                self.error(
                    f.loc,
                    "write the function's return type: its expression's type isn't one KompozR can tell (`fun f(…): Type = …`)",
                );
                return None;
            }
        };
        // (a suspending one gives a boxed future, which its calls await)
        let ret = if suspend {
            let r = value_ret.clone().unwrap_or_else(|| quote_spanned!(span=> ()));
            Some(quote_spanned!(span=> ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<Output = #r>>>))
        } else {
            value_ret
        };
        // Its body: a function of its own (its `return`s), what it uses of the outer scopes captured.
        let saved_fun = self.fun.clone();
        let saved_lambdas = std::mem::take(&mut self.lambdas);
        let saved_loops = std::mem::take(&mut self.loops);
        let saved_jumps = std::mem::take(&mut self.control.jump_frames);
        let saved_async = self.in_async;
        self.fun = super::FnState {
            kompozable: false,
            nullable_result: ret_ty.is_nullable(),
            shared_result: None,
            result: Some(ret_ty.clone()),
        };
        self.in_async = suspend;
        let boundary = self.scopes.len();
        self.frames.push(CaptureFrame { boundary, used: Vec::new(), kept: true });
        self.push_scope();
        self.ranges.push(f.loc);
        let receivers = self.receivers.len();
        let mut names = Vec::new();
        let mut it = params.iter();
        if let Some(r) = &f.receiver {
            let t = it.next().expect("its receiver's type");
            self.push_local_receiver(r);
            names.push(quote_spanned!(span=> __kz_recv: #t));
        }
        for (p, t) in f.params.iter().zip(it) {
            let ty = p.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown);
            let i = self.name_ident(&super::names::snake(&p.name.name), p.name.loc);
            self.declare_at(&p.name, super::names::snake(&p.name.name), LocalKind::Param, ty);
            names.push(quote_spanned!(span=> #i: #t));
        }
        let unit = ret_ty == Ty::Unit;
        let body = match &f.body {
            Some(FunctionBody::Block(b)) => self.fn_block(b, unit),
            Some(FunctionBody::Expr(e)) if unit => {
                let s = self.stmt_expr(e);
                quote!(#s;)
            }
            Some(FunctionBody::Expr(e)) => {
                self.value_want(e, super::expr::Want { some: ret_ty.is_nullable(), result: true, ..Default::default() })
            }
            None => {
                self.error(f.loc, "a function here needs a body");
                quote!()
            }
        };
        self.receivers.truncate(receivers);
        self.ranges.pop();
        self.pop_scope();
        let frame = self.frames.pop().expect("its frame");
        self.fun = saved_fun;
        self.lambdas = saved_lambdas;
        self.loops = saved_loops;
        self.control.jump_frames = saved_jumps;
        self.in_async = saved_async;
        let rt = super::respan(self.rt().clone(), span);
        let kt = self.kt(span);
        let kz_types = self.types_arg(span);
        let uses_self = frame.used.iter().any(|u| u == "self") && self.receivers.iter().any(|r| r.rust == "self");
        let captures: Vec<proc_macro2::Ident> = frame
            .used
            .iter()
            .filter(|u| *u != "self" && *u != "__kz_recv" && Some(u.as_str()) != own)
            .map(|u| super::names::ident(u, span))
            .collect();
        let lets: Vec<TokenStream> =
            captures.iter().map(|c| quote_spanned!(span=> let #c = #rt::__private::capture(&#c);)).collect();
        let own_self = uses_self.then(|| self.self_capture(span));
        let ret_ts = ret.as_ref().map(|r| quote_spanned!(span=> -> #r));
        // (calling itself: itself, through the weak reference it is given once made)
        let recursive = own_flag.is_some_and(|u| u.get());
        let (slot, myself) = match own {
            Some(n) if recursive => {
                let i = super::names::ident(n, span);
                (
                    quote_spanned!(span=> let __kz_slot = ::std::rc::Rc::clone(&__kz_slot);),
                    quote_spanned!(span=> #[allow(unused_variables)] let #i = __kz_slot.get().and_then(::std::rc::Weak::upgrade).expect("a local function, made");),
                )
            }
            _ => (quote!(), quote!()),
        };
        let inner = if suspend {
            // (each call's future owns clones of what it uses)
            let again: Vec<TokenStream> = lets.clone();
            let self_again = uses_self.then(|| quote_spanned!(span=> let __kz_self = __kz_self.clone();));
            quote_spanned!(span=> {
                #self_again
                #(#again)*
                #myself
                ::std::boxed::Box::pin(async move {
                    #[allow(unused_imports)]
                    use #kt::ext::*;
                    #rt::named_args!(#kz_types { #body })
                })
            })
        } else {
            quote_spanned!(span=> {
                #[allow(unused_imports)]
                use #kt::ext::*;
                #myself
                #rt::named_args!(#kz_types { #body })
            })
        };
        let inner = if uses_self { renamed_self(inner) } else { inner };
        let inner = match &own_self {
            Some((_, prelude)) => quote_spanned!(span=> { #prelude #inner }),
            None => inner,
        };
        let outer_self = own_self.map(|(outer, _)| outer);
        let closure = quote_spanned!(span=> {
            #outer_self
            #slot
            #(#lets)*
            move |#(#names),*| #ret_ts #inner
        });
        Some(ClosureFn { closure, params, ret, ty })
    }

    /// What a kept closure using the object (`self`) of the member it is in holds of it, `__kz_self` in its body: before
    /// the closure (`outer`), and at its body's start (`prelude`). A clone of it (Kotlin's lambda holding `this`), but
    /// where the closure is kept by the object itself — made while a plain class's object is initialized (a delegate's
    /// lambda, `val doubled by derivedStateOf { count * 2 }`) — a weak reference, upgraded when it runs: a strong one
    /// would be a cycle no one frees.
    pub(crate) fn self_capture(&self, span: proc_macro2::Span) -> (TokenStream, TokenStream) {
        use quote::{quote, quote_spanned};
        if self.control.in_init {
            (
                quote_spanned!(span=> let __kz_weak_self = ::std::rc::Rc::downgrade(&self.__kz_this);),
                quote_spanned!(span=> #[allow(unused_variables)] let __kz_self = Self {
                    __kz_this: __kz_weak_self.upgrade().expect("the object whose initializer made this closure"),
                };),
            )
        } else {
            (quote_spanned!(span=> let __kz_self = self.clone();), quote!())
        }
    }

    /// Notes that the `var` local `l` is assigned here (see [`Gen::shared_pass`]).
    pub(crate) fn note_assigned(&mut self, l: &super::resolve::Local) {
        if l.kind != super::resolve::LocalKind::Var {
            return;
        }
        if let Some(at) = l.at {
            self.control.assigned_vars.insert(var_key(at));
        }
    }

    /// Notes that the local `l` is used here: from inside a kept lambda (or a local function) declared below it, a
    /// `var` is captured (see [`Gen::shared_pass`]).
    pub(crate) fn note_used(&mut self, l: &super::resolve::Local) {
        if l.kind != super::resolve::LocalKind::Var {
            return;
        }
        let Some(at) = l.at else { return };
        if self.frames.iter().any(|f| l.depth < f.boundary && f.kept) {
            self.control.captured_vars.insert(var_key(at));
        }
    }
}
