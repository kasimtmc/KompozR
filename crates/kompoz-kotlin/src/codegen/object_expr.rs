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

    /// An object expression's type: the interface's shared object.
    pub(crate) fn object_expr_ty(&self, supers: &[SuperType]) -> Ty {
        match self.object_interface(supers) {
            Some((t, _)) => {
                let ptr = trait_object(&t).unwrap_or("Rc");
                let args: Vec<Ty> = Self::object_type_args(supers).iter().map(|a| self.ty_of(a)).collect();
                Ty::Named(ptr.to_string(), vec![Ty::Named(t, args)])
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
            return Some(Signature { rust, params, ret });
        }
        let item = crate::index::lookup(&f.name.name)
            .iter()
            .find(|i| i.via == iface && i.kind == crate::index::Kind::Method && i.params.len() == f.params.len())?;
        let parse = |t: &str| -> TokenStream {
            let ts: TokenStream = t.parse().unwrap_or_else(|_| quote!(_));
            super::respan(ts, span)
        };
        let params = item.params.iter().map(|p| parse(p.ty)).collect();
        let ret = (item.ret.trim() != "()").then(|| parse(item.ret));
        Some(Signature { rust: item.rust.to_string(), params, ret })
    }

    /// `object : I { … }` (see the module's doc).
    pub(crate) fn object_expr(&mut self, supers: &[SuperType], body: &ClassBody, loc: Loc) -> TokenStream {
        let span = self.span(loc);
        let Some((iface, user)) = self.object_interface(supers) else {
            return self.object_expr_class(supers, body, loc);
        };
        let saved_args = std::mem::replace(&mut self.classes.object_args, Self::object_type_args(supers));
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
                    let ty = match &p.ty {
                        Some(t) => Some(self.rust_type(t, TypePos::Value)),
                        None => self.object_property_ty(&iface, user, &n.name, pspan),
                    };
                    let Some(ty) = ty else {
                        self.error(n.loc, format!("`{}` overrides no property of `{iface}`", n.name));
                        continue;
                    };
                    let k = fields.len();
                    let field = format_ident!("p{k}", span = pspan);
                    let rust = self.name_ident(&snake(&n.name), n.loc);
                    let v = self.value_want(init, super::expr::Want { str_ok: false, ..Default::default() });
                    fields.push(quote_spanned!(pspan=> #field: #ty));
                    methods.push(
                        quote_spanned!(pspan=> fn #rust(&self) -> #ty { ::core::clone::Clone::clone(&self.#field) }),
                    );
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
            if f.is_suspend() || f.is_kompozable() || !f.type_params.is_empty() {
                self.later(f.loc, "a suspending, composable or generic function in an object expression");
                continue;
            }
            let Some(sig) = self.interface_fn(&iface, user, f, fspan) else {
                self.error(f.name.loc, format!("`{}` overrides no function of `{iface}`", f.name.name));
                continue;
            };
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
            bounds.push(quote_spanned!(fspan=> #g: ::core::ops::Fn(#(#ps),*) #ret + 'static));
            methods.push(quote_spanned! {fspan=>
                fn #rust(&self, #(#names: #ps),*) #ret { (self.#field)(#(#names),*) }
            });
            let closure = self.object_fn_closure(f, &sig, &names);
            values.push(quote_spanned!(fspan=> #field: #closure));
        }
        self.classes.object_args = saved_args;
        let ti = proc_macro2::Ident::new(&iface, span);
        let t = if targs.is_empty() { quote_spanned!(span=> #ti) } else { quote_spanned!(span=> #ti<#(#targs),*>) };
        let ptr = if trait_object(&iface) == Some("Arc") {
            quote_spanned!(span=> ::std::sync::Arc)
        } else {
            quote_spanned!(span=> ::std::rc::Rc)
        };
        let o = self.fresh("object", span);
        quote_spanned! {span=>
            {
                #[allow(non_camel_case_types)]
                struct __KzObject<#(#generics),*> { #(#fields),* }
                impl<#(#bounds),*> #t for __KzObject<#(#generics),*> { #(#methods)* }
                let #o: #ptr<dyn #t> = #ptr::new(__KzObject { #(#values),* });
                #o
            }
        }
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
        let item = crate::index::lookup(name)
            .iter()
            .find(|i| i.via == iface && i.kind == crate::index::Kind::Method && i.params.is_empty())?;
        Some(super::respan(item.ret.parse().ok()?, span))
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
        let sig = Signature { rust: String::new(), params: params.to_vec(), ret: ret.cloned() };
        let span = self.span(f.loc);
        // (a function of its own inside the expression: its scope on top of the outer ones, what it uses of them
        // recorded as a lambda's captures are)
        let saved_fun = self.fun.clone();
        let saved_lambdas = std::mem::take(&mut self.lambdas);
        let saved_loops = std::mem::take(&mut self.loops);
        let saved_async = self.in_async;
        let ret_ty = sig.ret.as_ref().map_or(Ty::Unit, |r| Ty::from_rust(&r.to_string()));
        self.fun = super::FnState { kompozable: false, nullable_result: ret_ty.is_nullable(), shared_result: None };
        self.in_async = false;
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
                let want = super::expr::Want { some: ret_ty.is_nullable(), ..Default::default() };
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
