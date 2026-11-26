//! Kotlin's object expressions implementing one interface: `object : NestedScrollConnection { override fun
//! onPreScroll(…) = … }`, the interface the user's (a trait of the text or the module, generic too: `object :
//! Source<Int> { … }`) or the API's (a trait of the index); its `override val`s values it holds, made with it (stage
//! 14 WPA).
//!
//! ```text
//! object : Filter { override fun keep(e: Entry) = e.ok }
//!   →  { struct __KzObject<F0> { m0: F0 }
//!        impl<F0: Fn(Entry) -> bool> Filter for __KzObject<F0> { fn keep(&self, e: Entry) -> bool { (self.m0)(e) } }
//!        let o: Rc<dyn Filter> = Rc::new(__KzObject { m0: { let x = x.clone(); move |e: Entry| -> bool { … } } });
//!        o }
//! ```
//!
//! Each overriding function is a closure kept by the object, capturing clones of the locals it uses (Kotlin's
//! object holds what its functions use); the object's value is the interface's shared trait object, as Kotlin's
//! references to it are. The interface's other functions keep their defaults (a trait's provided methods).

use super::Gen;
use super::names::{ident, snake};
use super::resolve::*;
use super::types::TypePos;
use crate::ast::*;
use crate::pos::Loc;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};

/// A function of the interface: its Rust name, its parameters' and result's Rust types.
struct Signature {
    rust: String,
    params: Vec<TokenStream>,
    ret: Option<TokenStream>,
    /// (WPA-rest) The trait it is of (an API interface's supertrait's: `OverlayScene`'s `Scene`); empty: the
    /// interface's own.
    via: String,
}

/// The API traits an API interface's object implements: itself and its supertraits the index has methods of
/// (`OverlayScene` → `Scene`), nearest first.
fn api_chain(iface: &str) -> Vec<&str> {
    crate::index::deref_chain(iface)
        .into_iter()
        .filter(|t| {
            *t == iface
                || crate::index::ITEMS
                    .iter()
                    .any(|i| i.via == *t && i.owner.is_empty() && i.kind == crate::index::Kind::Method)
        })
        .collect()
}

impl Gen<'_, '_> {
    /// The interface an object expression implements: its Rust name, whether it is the user's.
    fn object_interface(&self, supers: &[SuperType]) -> Option<(String, bool)> {
        let [s] = supers else { return None };
        if s.constructor_args.is_some() || s.delegate.is_some() {
            return None;
        }
        let name = s.ty.simple_name()?;
        if let Some(c) = self.user_class(&name) {
            return (c.shape == super::class::Shape::Interface && c.inherit != super::hierarchy::Inherit::Sealed)
                .then(|| (c.rust.clone(), true));
        }
        // (a nested interface Rust names otherwise: `ViewModelProvider.Factory`)
        let last = crate::index::rust_type(&name).unwrap_or_else(|| super::last_segment(&name)).to_string();
        let is_trait = crate::index::ITEMS
            .iter()
            .any(|i| i.via == last && i.owner.is_empty() && i.kind == crate::index::Kind::Method);
        is_trait.then_some((last, false))
    }

    /// An object expression's type: the interface's shared object (one made a local class, `find_locals`: the
    /// class).
    pub(crate) fn object_expr_ty(&self, supers: &[SuperType], loc: Loc) -> Ty {
        if let Some(rust) = self.local_object(loc) {
            return Ty::named(&rust);
        }
        match self.object_interface(supers) {
            Some((t, _)) => {
                let ptr = trait_object(&t).unwrap_or("Rc");
                let args: Vec<Ty> = Self::object_type_args(supers).iter().map(|a| self.ty_of(a)).collect();
                Ty::Named(ptr.to_string(), vec![Ty::Named(t, args)])
            }
            // ((WPA-rest) one of a base of the second kind: the base's struct)
            None if let [s] = supers
                && let Some(super::hierarchy::SuperKind::Base(b)) =
                    s.ty.simple_name().and_then(|n| self.super_kind(&n))
                && !b.value.is_empty() =>
            {
                self.callbacks_value_ty(b, &Self::object_type_args(supers))
            }
            // (one extending a class: the class's shared object)
            None => match self.object_class_trait(supers) {
                Some((t, args)) => {
                    let ptr = trait_object(&t).unwrap_or("Rc");
                    let args: Vec<Ty> = args.iter().map(|a| self.ty_of(a)).collect();
                    Ty::Named(ptr.to_string(), vec![Ty::Named(t, args)])
                }
                None => Ty::Unknown,
            },
        }
    }

    /// The interface's function a member overrides: its signature (the user's interface's from its declaration, the
    /// API's from the index).
    fn interface_fn(&mut self, iface: &str, user: bool, f: &Function, span: Span) -> Option<Signature> {
        if user {
            let decl = self.classes.decls.iter().find(|d| d.info.rust == iface).cloned()?;
            let super::class::ClassAst::Class(c) = &decl.ast else { return None };
            // ((stage 14 WPB) of the interface's overloads of the name, the one of the same parameters' types)
            let same = |g: &Function| {
                g.name.name == f.name.name
                    && super::overloads::mangled("", &g.params) == super::overloads::mangled("", &f.params)
            };
            let m = c
                .body
                .members
                .iter()
                .find_map(|m| match m {
                    Member::Decl(Decl::Function(g)) if same(g) => Some(g.clone()),
                    _ => None,
                })
                .or_else(|| {
                    c.body.members.iter().find_map(|m| match m {
                        Member::Decl(Decl::Function(g))
                            if g.name.name == f.name.name && g.params.len() == f.params.len() =>
                        {
                            Some(g.clone())
                        }
                        _ => None,
                    })
                })?;
            let rust = self.member_rust_name(&c.body.members, &m, &[]);
            // (a generic interface's type parameters: the expression's arguments)
            let binds: std::collections::HashMap<String, Type> =
                decl.info.type_params.iter().cloned().zip(self.classes.object_args.clone()).collect();
            let mut params = Vec::new();
            for p in &m.params {
                params.push(self.rust_type(&super::class::substitute(p.ty.as_ref()?, &binds), TypePos::Value));
            }
            let ret = m
                .result
                .as_ref()
                .map(|t| super::class::substitute(t, &binds))
                .filter(|t| self.ty_of(t) != Ty::Unit)
                .map(|t| self.rust_type(&t, TypePos::Value));
            return Some(Signature { rust, params, ret, via: String::new() });
        }
        let chain = api_chain(iface);
        let item = crate::index::lookup(&f.name.name).iter().find(|i| {
            chain.contains(&i.via) && i.kind == crate::index::Kind::Method && i.params.len() == f.params.len()
        })?;
        let parse = |t: &str| -> TokenStream {
            let ts: TokenStream = t.parse().unwrap_or_else(|_| quote!(_));
            super::respan(ts, span)
        };
        let _ = parse;
        // (the trait's type parameters the expression's arguments)
        let targs: Vec<TokenStream> =
            self.classes.object_args.clone().iter().map(|t| self.rust_type(t, TypePos::Value)).collect();
        let binds: std::collections::HashMap<String, TokenStream> =
            crate::index::generics(item.via).unwrap_or_default().iter().map(|p| p.to_string()).zip(targs).collect();
        let params = item.params.iter().map(|p| super::callbacks::substituted(p.ty, &binds, span)).collect();
        let ret = (item.ret.trim() != "()").then(|| super::callbacks::substituted(item.ret, &binds, span));
        Some(Signature { rust: item.rust.to_string(), params, ret, via: item.via.to_string() })
    }

    /// `object : I { … }` (see the module's doc).
    pub(crate) fn object_expr(&mut self, supers: &[SuperType], body: &ClassBody, loc: Loc) -> TokenStream {
        let span = self.span(loc);
        // ((stage 14 WPD) one with members of its own: its local class's object, `find_locals`)
        if let Some(rust) = self.local_object(loc) {
            let class = proc_macro2::Ident::new(&rust, span);
            return quote_spanned!(span=> #class());
        }
        let Some((iface, user)) = self.object_interface(supers) else {
            return self.object_expr_class(supers, body, loc);
        };
        let saved_args = std::mem::replace(&mut self.classes.object_args, Self::object_type_args(supers));
        // ((stage 14 WPD) an interface the API shares between threads, `Arc<dyn Shape>`: its functions too)
        let arc = trait_object(&iface) == Some("Arc");
        let threads = if arc { quote!(+ ::core::marker::Send + ::core::marker::Sync) } else { quote!() };
        let targs: Vec<TokenStream> =
            self.classes.object_args.clone().iter().map(|t| self.rust_type(t, TypePos::Value)).collect();
        let mut fields = Vec::new();
        let mut generics = Vec::new();
        let mut bounds = Vec::new();
        let mut methods = Vec::new();
        let mut values = Vec::new();
        for m in &body.members {
            let f = match m {
                Member::Decl(Decl::Function(f)) => f,
                // (an `override val`: its value made with the object, held by it)
                Member::Decl(Decl::Property(p)) if !p.is_var() && matches!(p.init, PropertyInit::Value(_)) => {
                    let (Binding::Name(n), PropertyInit::Value(init)) = (&p.binding, &p.init) else { continue };
                    let pspan = self.span(p.loc);
                    let k = fields.len();
                    let field = format_ident!("p{k}", span = pspan);
                    let rust = self.name_ident(&snake(&n.name), n.loc);
                    // ((WPA-rest) a property the API has as a method giving nothing, `override val content:
                    // @Composable () -> Unit = { … }` of a `Scene`: the lambda kept, the method calling it)
                    let via = if user { String::new() } else { self.object_property_via(&iface, &n.name) };
                    if !user
                        && let ExprKind::Lambda(l) = &init.kind
                        && self.object_property_ty(&iface, user, &n.name, pspan).is_some_and(|t| t.to_string() == "()")
                    {
                        // (its body a function's of no parameters, composable as its declared type says)
                        let f = Function {
                            annotations: p.ty.as_ref().map(|t| t.annotations.clone()).unwrap_or_default(),
                            modifiers: Modifiers::default(),
                            type_params: Vec::new(),
                            receiver: None,
                            name: n.clone(),
                            params: Vec::new(),
                            result: None,
                            constraints: Vec::new(),
                            body: Some(FunctionBody::Block(l.body.clone())),
                            loc: l.loc,
                        };
                        let closure = self.object_fn_closure_with(&f, &[], None, &[], None);
                        fields.push(quote_spanned!(pspan=> #field: ::std::rc::Rc<dyn ::core::ops::Fn()>));
                        methods.push((via, quote_spanned!(pspan=> fn #rust(&self) { (self.#field)() })));
                        values.push(quote_spanned!(pspan=> #field: ::std::rc::Rc::new(#closure)));
                        continue;
                    }
                    let ty = match &p.ty {
                        Some(t) => Some(self.rust_type(t, TypePos::Value)),
                        None => self.object_property_ty(&iface, user, &n.name, pspan),
                    };
                    let Some(ty) = ty else {
                        self.error(n.loc, format!("`{}` overrides no property of `{iface}`", n.name));
                        continue;
                    };
                    let v = self.value_want(init, super::expr::Want { str_ok: false, ..Default::default() });
                    fields.push(quote_spanned!(pspan=> #field: #ty));
                    methods.push((
                        via,
                        quote_spanned!(pspan=> fn #rust(&self) -> #ty { ::core::clone::Clone::clone(&self.#field) }),
                    ));
                    values.push(quote_spanned!(pspan=> #field: #v));
                    continue;
                }
                Member::Decl(d) => {
                    self.later(d.loc(), "a `var`, delegated or declaration member in an object expression");
                    continue;
                }
                Member::Init(b) => {
                    self.later(b.loc, "an `init` block in an object expression");
                    continue;
                }
                Member::Constructor(c) => {
                    self.error(c.loc, "an object has no constructors");
                    continue;
                }
            };
            let fspan = self.span(f.loc);
            if f.is_kompozable() {
                self.later(f.loc, "a composable function in an object expression of an interface");
                continue;
            }
            // ((WPA-rest) a generic override of the API's method that isn't (`ViewModelProvider.Factory`'s `fun <T :
            // ViewModel> create(modelClass: KClass<T>, …): T`): its type parameters their bounds, `x as T` the value)
            let erased;
            let f = if !f.type_params.is_empty() && !user {
                erased = erase_type_params(f);
                &erased
            } else if !f.type_params.is_empty() {
                self.later(f.loc, "a generic function in an object expression of an interface");
                continue;
            } else {
                f
            };
            let Some(sig) = self.interface_fn(&iface, user, f, fspan) else {
                self.error(f.name.loc, format!("`{}` overrides no function of `{iface}`", f.name.name));
                continue;
            };
            // ((WPA-rest) a suspending one: of a method giving a boxed future (`OverlayScene.onRemove`))
            if f.is_suspend() && sig.ret.as_ref().is_none_or(|r| future_output(&r.to_string()).is_none()) {
                self.later(f.loc, "a suspending function in an object expression of an interface");
                continue;
            }
            let k = methods.len();
            let field = format_ident!("m{k}", span = fspan);
            let g = format_ident!("F{k}", span = fspan);
            let ps = &sig.params;
            let ret = sig.ret.as_ref().map(|r| quote_spanned!(fspan=> -> #r));
            let names: Vec<proc_macro2::Ident> =
                f.params.iter().map(|p| self.name_ident(&snake(&p.name.name), p.name.loc)).collect();
            let rust = ident(&sig.rust, fspan);
            fields.push(quote_spanned!(fspan=> #field: #g));
            generics.push(quote!(#g));
            bounds.push(quote_spanned!(fspan=> #g: ::core::ops::Fn(#(#ps),*) #ret #threads + 'static));
            methods.push((
                sig.via.clone(),
                quote_spanned! {fspan=>
                    fn #rust(&self, #(#names: #ps),*) #ret { (self.#field)(#(#names),*) }
                },
            ));
            let closure = self.object_fn_closure(f, &sig, &names);
            values.push(quote_spanned!(fspan=> #field: #closure));
        }
        self.classes.object_args = saved_args;
        let ti = proc_macro2::Ident::new(&iface, span);
        let t = if targs.is_empty() { quote_spanned!(span=> #ti) } else { quote_spanned!(span=> #ti<#(#targs),*>) };
        let ptr = if arc { quote_spanned!(span=> ::std::sync::Arc) } else { quote_spanned!(span=> ::std::rc::Rc) };
        // ((WPA-rest) an API interface's supertraits the index has methods of (`OverlayScene` → `Scene`): each
        // implemented, what the object overrides of it; one asking for the object as the interface
        // (`as_overlay_scene(self: Rc<Self>)`) given it)
        let mut impls = TokenStream::new();
        let own: Vec<TokenStream> =
            methods.iter().filter(|(v, _)| v.is_empty() || *v == iface).map(|(_, m)| m.clone()).collect();
        let chain: Vec<&str> = if user { vec![iface.as_str()] } else { api_chain(&iface) };
        let as_self = format!("as_{}", super::hierarchy::type_snake(&iface));
        for tr in chain.iter().skip(1) {
            let mut its: Vec<TokenStream> = methods.iter().filter(|(v, _)| v == tr).map(|(_, m)| m.clone()).collect();
            if let Some(item) = crate::index::ITEMS
                .iter()
                .find(|i| i.via == *tr && i.rust == as_self && i.receiver == crate::index::Receiver::Value)
            {
                let m = ident(item.rust, span);
                let binds: std::collections::HashMap<String, TokenStream> = crate::index::generics(tr)
                    .unwrap_or_default()
                    .iter()
                    .map(|p| p.to_string())
                    .zip(targs.iter().cloned())
                    .collect();
                let r = super::callbacks::substituted(item.ret, &binds, span);
                its.push(quote_spanned! {span=>
                    fn #m(self: ::std::rc::Rc<Self>) -> #r { ::core::option::Option::Some(self) }
                });
            }
            // (the comparing supertraits every `PartialEq` type has by a blanket impl: the object's, below)
            if matches!(*tr, "DynEq" | "ShapeEq") {
                continue;
            }
            let n = crate::index::generics(tr).map_or(0, |g| g.len());
            let ta: Vec<&TokenStream> = targs.iter().take(n).collect();
            let tri = proc_macro2::Ident::new(tr, span);
            let trt = if ta.is_empty() { quote_spanned!(span=> #tri) } else { quote_spanned!(span=> #tri<#(#ta),*>) };
            impls.extend(quote_spanned!(span=> impl<#(#bounds),*> #trt for __KzObject<#(#generics),*> { #(#its)* }));
        }
        // (Kotlin's `Any` members an API interface may ask for — a `Scene`'s (`DynEq`, `Debug`), a `Shape`'s —: equal as
        // the same object only, shown as an object)
        impls.extend(quote_spanned! {span=>
            impl<#(#generics),*> ::core::cmp::PartialEq for __KzObject<#(#generics),*> {
                fn eq(&self, other: &Self) -> bool { ::core::ptr::eq(self, other) }
            }
            impl<#(#generics),*> ::core::fmt::Debug for __KzObject<#(#generics),*> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    ::core::write!(f, "<object>@{:p}", self)
                }
            }
        });
        let o = self.fresh("object", span);
        quote_spanned! {span=>
            {
                #[allow(non_camel_case_types)]
                struct __KzObject<#(#generics),*> { #(#fields),* }
                impl<#(#bounds),*> #t for __KzObject<#(#generics),*> { #(#own)* }
                #impls
                let #o: #ptr<dyn #t> = #ptr::new(__KzObject { #(#values),* });
                #o
            }
        }
    }

    /// The API trait of an interface's chain whose property (a method taking nothing) `name` is.
    fn object_property_via(&self, iface: &str, name: &str) -> String {
        let chain = api_chain(iface);
        crate::index::lookup(name)
            .iter()
            .find(|i| chain.contains(&i.via) && i.kind == crate::index::Kind::Method && i.params.is_empty())
            .map(|i| i.via.to_string())
            .unwrap_or_default()
    }

    /// The type arguments of an object expression's supertype (`Source<Int>`'s).
    fn object_type_args(supers: &[SuperType]) -> Vec<Type> {
        let [s] = supers else { return Vec::new() };
        match &s.ty.kind {
            TypeKind::Named(segs) => segs
                .last()
                .map(|seg| {
                    seg.args
                        .iter()
                        .filter_map(|a| match a {
                            TypeArg::Type { ty, .. } => Some(ty.clone()),
                            TypeArg::Star(_) => None,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// The type of an interface's property an object expression's `override val` gives (the user's interface's
    /// declaration, the API's index).
    fn object_property_ty(&mut self, iface: &str, user: bool, name: &str, span: Span) -> Option<TokenStream> {
        if user {
            let decl = self.classes.decls.iter().find(|d| d.info.rust == iface).cloned()?;
            let super::class::ClassAst::Class(c) = &decl.ast else { return None };
            let binds: std::collections::HashMap<String, Type> =
                decl.info.type_params.iter().cloned().zip(self.classes.object_args.clone()).collect();
            let t = c.body.members.iter().find_map(|m| match m {
                Member::Decl(Decl::Property(p)) if matches!(&p.binding, Binding::Name(n) if n.name == name) => {
                    p.ty.clone()
                }
                _ => None,
            })?;
            return Some(self.rust_type(&super::class::substitute(&t, &binds), TypePos::Value));
        }
        let chain = api_chain(iface);
        let item = crate::index::lookup(name)
            .iter()
            .find(|i| chain.contains(&i.via) && i.kind == crate::index::Kind::Method && i.params.is_empty())?;
        let targs: Vec<TokenStream> =
            self.classes.object_args.clone().iter().map(|t| self.rust_type(t, TypePos::Value)).collect();
        let binds: std::collections::HashMap<String, TokenStream> =
            crate::index::generics(item.via).unwrap_or_default().iter().map(|p| p.to_string()).zip(targs).collect();
        Some(super::callbacks::substituted(item.ret, &binds, span))
    }

    /// An overriding function as the closure the object keeps: a function's body in a closure taking its
    /// parameters, the outer locals it uses cloned into it.
    fn object_fn_closure(&mut self, f: &Function, sig: &Signature, names: &[proc_macro2::Ident]) -> TokenStream {
        self.object_fn_closure_with(f, &sig.params, sig.ret.as_ref(), names, None)
    }

    /// An overriding function as a closure (see [`object_fn_closure`](Self::object_fn_closure)); `receiver`: it takes
    /// the object first (`__kz_this: &Ty`), the implicit receiver of its body, a value of the trait (its Rust name).
    pub(crate) fn object_fn_closure_with(
        &mut self,
        f: &Function,
        params: &[TokenStream],
        ret: Option<&TokenStream>,
        names: &[proc_macro2::Ident],
        receiver: Option<(&TokenStream, &str)>,
    ) -> TokenStream {
        let sig = Signature { rust: String::new(), params: params.to_vec(), ret: ret.cloned(), via: String::new() };
        let span = self.span(f.loc);
        // ((WPA-rest) a suspending one: its body a coroutine, the boxed future the closure gives — `KtFuture`, or
        // the API's `Pin<Box<dyn Future<Output = R>>>` —, its value its output)
        let suspend = f.is_suspend();
        let output = sig.ret.as_ref().and_then(|r| future_output(&r.to_string()));
        // (a function of its own inside the expression: its scope on top of the outer ones, what it uses of them
        // recorded as a lambda's captures are)
        let saved_fun = self.fun.clone();
        let saved_lambdas = std::mem::take(&mut self.lambdas);
        let saved_loops = std::mem::take(&mut self.loops);
        let saved_async = self.in_async;
        let saved_leaves = std::mem::take(&mut self.control.leaves);
        let ret_ty = match (&output, suspend) {
            (Some(o), true) => Ty::from_rust(o),
            _ => sig.ret.as_ref().map_or(Ty::Unit, |r| Ty::from_rust(&r.to_string())),
        };
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
        let pushed = receiver.is_some();
        if let Some((_, tr)) = receiver {
            self.receivers.push(ImplicitReceiver {
                rust: "__kz_this".into(),
                ty: tr.to_string(),
                value: Ty::named(tr),
                labels: Vec::new(),
                depth: self.scopes.len(),
                by_ref: false,
                by_mut: false,
                owned_used: Default::default(),
            });
        }
        for (p, t) in f.params.iter().zip(&sig.params) {
            let ty = p.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or_else(|| Ty::from_rust(&t.to_string()));
            self.declare_at(&p.name, snake(&p.name.name), LocalKind::Param, ty);
        }
        let unit = ret_ty == Ty::Unit;
        let body = match &f.body {
            Some(FunctionBody::Block(b)) => self.fn_block(b, unit),
            Some(FunctionBody::Expr(e)) if unit => {
                let s = self.stmt_expr(e);
                quote!(#s;)
            }
            Some(FunctionBody::Expr(e)) => {
                let want = super::expr::Want { some: ret_ty.is_nullable(), result: true, ..Default::default() };
                self.value_want(e, want)
            }
            None => {
                self.error(f.name.loc, "a function here needs a body");
                quote!()
            }
        };
        self.pop_scope();
        if pushed {
            self.receivers.pop();
        }
        let frame = self.frames.pop().expect("its frame");
        self.fun = saved_fun;
        self.lambdas = saved_lambdas;
        self.loops = saved_loops;
        self.in_async = saved_async;
        self.control.leaves = saved_leaves;
        let clones: Vec<TokenStream> = frame
            .used
            .iter()
            .filter(|n| *n != "self" && *n != "__kz_this")
            .map(|n| {
                let i = ident(n, span);
                quote_spanned!(span=> let #i = ::core::clone::Clone::clone(&#i);)
            })
            .collect();
        let ps = &sig.params;
        let ret = sig.ret.as_ref().map(|r| quote_spanned!(span=> -> #r));
        let kt = self.kt(span);
        let rt = super::respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let this = receiver.map(|(t, _)| quote_spanned!(span=> __kz_this: &#t,));
        if suspend {
            // (a future borrowing the object, `KtFuture<'_, R>`: the closure's result told by the function it is given to)
            let ret = if sig.ret.as_ref().is_some_and(|r| r.to_string().contains('\'')) { None } else { ret };
            return quote_spanned! {span=>
                {
                    #(#clones)*
                    move |#this #(#names: #ps),*| #ret {
                        #(#clones)*
                        ::std::boxed::Box::pin(async move {
                            #[allow(unused_imports)]
                            use #kt::ext::*;
                            let _ = (#(&#names,)*);
                            #rt::named_args!(#kz_types { #body })
                        })
                    }
                }
            };
        }
        // ((WPA-rest) a composable one, a `Scene`'s `content`: its body's calls composable ones)
        if f.is_kompozable() {
            return quote_spanned! {span=>
                {
                    #(#clones)*
                    move |#this #(#names: #ps),*| #ret {
                        #[allow(unused_imports)]
                        use #kt::ext::*;
                        let _ = (#(&#names,)*);
                        #rt::kompozable_block!(#kz_types { #body })
                    }
                }
            };
        }
        quote_spanned! {span=>
            {
                #(#clones)*
                move |#this #(#names: #ps),*| #ret {
                    #[allow(unused_imports)]
                    use #kt::ext::*;
                    // (an override's parameters it doesn't use: Kotlin says nothing of them)
                    let _ = (#(&#names,)*);
                    #rt::named_args!(#kz_types { #body })
                }
            }
        }
    }
}

/// The output of a boxed future's type, as its text is (`KtFuture<'_, R>`, `Pin<Box<dyn Future<Output = R>>>`): `R`.
pub(crate) fn future_output(text: &str) -> Option<String> {
    let t = text.replace(' ', "");
    if let Some(at) = t.find("KtFuture<") {
        let rest = &t[at + "KtFuture<".len()..];
        let rest = rest.strip_suffix('>')?;
        return Some(rest.split_once(',').map(|(_, r)| r.to_string()).unwrap_or_else(|| "()".into()));
    }
    let at = t.find("Output=")?;
    let rest = &t[at + "Output=".len()..];
    // (up to the `>` closing `Future<`)
    let mut depth = 0i32;
    let mut end = rest.len();
    for (k, c) in rest.char_indices() {
        match c {
            '<' => depth += 1,
            '>' if depth == 0 => {
                end = k;
                break;
            }
            '>' => depth -= 1,
            ',' | '+' if depth == 0 => {
                end = k;
                break;
            }
            _ => {}
        }
    }
    Some(rest[..end].to_string())
}

/// A generic function with its type parameters erased: each its bound (`Any` without one) in its parameters' and
/// result's types, `x as T` / `x as? T` (Kotlin's unchecked casts) the value itself in its body's `return`s and its
/// expression body.
fn erase_type_params(f: &Function) -> Function {
    let binds: std::collections::HashMap<String, Type> = f
        .type_params
        .iter()
        .map(|t| {
            let bound = t.bound.clone().unwrap_or_else(|| super::hierarchy::named_type("Any", t.loc));
            (t.name.name.clone(), bound)
        })
        .collect();
    let names: Vec<String> = binds.keys().cloned().collect();
    let uncast = |e: &Expr| -> Expr {
        match &e.kind {
            ExprKind::As { expr, ty, .. } if ty.simple_name().is_some_and(|n| names.contains(&n)) => (**expr).clone(),
            _ => e.clone(),
        }
    };
    let mut g = f.clone();
    g.type_params.clear();
    for p in &mut g.params {
        if let Some(t) = &p.ty {
            p.ty = Some(super::class::substitute(t, &binds));
        }
    }
    g.result = g.result.as_ref().map(|t| super::class::substitute(t, &binds));
    g.body = match &f.body {
        Some(FunctionBody::Expr(e)) => Some(FunctionBody::Expr(uncast(e))),
        Some(FunctionBody::Block(b)) => {
            let mut b = b.clone();
            for s in &mut b.stmts {
                let e = match &mut s.kind {
                    StmtKind::Expr(e) => e,
                    _ => continue,
                };
                // (`@Suppress("UNCHECKED_CAST") return x as T` too)
                let e = match &mut e.kind {
                    ExprKind::Annotated { expr, .. } => &mut **expr,
                    _ => e,
                };
                if let ExprKind::Return { value: Some(v), .. } = &mut e.kind {
                    **v = uncast(v);
                }
            }
            Some(FunctionBody::Block(b))
        }
        None => None,
    };
    g
}
