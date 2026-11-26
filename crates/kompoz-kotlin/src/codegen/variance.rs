//! (stage 14 FINAL-T) Kotlin's declaration-site variance of the user's generic value types: an `Outcome<Dog>` is an
//! `Outcome<Animal>` when the class is `Outcome<out T>` (a sealed class of data cases, a data class), Kotlin's one
//! object seen as either. Rust's types are invariant: the value is mapped instead, each `T` in it made the wider
//! type (its own coercion: `Dog` → `Rc<dyn AnimalOpen>`, a case → its sealed class).
//!
//! | Kotlin | Rust |
//! |---|---|
//! | `data class Box<out T>(val item: T, val more: List<T>)` | `impl<T> Box<T> { fn __kz_out<U>(self, f0: &dyn Fn(T) -> U) -> Box<U> { Box { item: f0(self.item), more: self.more.into_iter().map(\|x\| f0(x)).collect(), … } } }` |
//! | `sealed class Outcome<out T> { data class Found<out T>(val value: T) : Outcome<T>(); object Missing : Outcome<Nothing>() }` | `impl<T> Outcome<T> { fn __kz_out<U>(self, f0) -> Outcome<U> { match self { Outcome::Found(c) => Outcome::Found(c.__kz_out(f0)), Outcome::Missing(c) => Outcome::Missing(c) } } }` |
//! | `val a: Outcome<Animal> = dogs()` | `dogs().__kz_out(&\|x: Dog\| -> Rc<dyn AnimalOpen> { Rc::new(x) })` |
//!
//! A type is mapped so when every property holding a type parameter's value is a `val` of a type the mapping
//! reaches (`T`, `T?`, `List<T>`, `Pair`s of them, another such class of them), its other state (a sealed class's
//! base part) holding none: a plain class's object, an interface's, a function of `T` are Kotlin's one object and
//! aren't copied — such a type stays invariant here (rustc's error says where).

use super::Gen;
use super::class::{ClassAst, Shape};
use super::hierarchy::Inherit;
use super::resolve::*;
use crate::ast::*;
use proc_macro2::{Span, TokenStream, TokenTree};
use quote::{format_ident, quote, quote_spanned};
use std::collections::HashMap;

/// `ts` with the identifiers `from` named as `to` (a class's type parameters made the mapping's targets).
fn renamed(ts: TokenStream, names: &HashMap<String, proc_macro2::Ident>) -> TokenStream {
    ts.into_iter()
        .map(|t| match t {
            TokenTree::Ident(i) if names.contains_key(&i.to_string()) => {
                let mut n = names[&i.to_string()].clone();
                n.set_span(i.span());
                TokenTree::Ident(n)
            }
            TokenTree::Group(g) => {
                let mut n = proc_macro2::Group::new(g.delimiter(), renamed(g.stream(), names));
                n.set_span(g.span());
                TokenTree::Group(n)
            }
            t => t,
        })
        .collect()
}

/// Whether a Kotlin type names one of `params`.
fn mentions(t: &Type, params: &[String]) -> bool {
    match &t.kind {
        TypeKind::Named(segs) => segs.iter().any(|s| {
            (segs.len() == 1 && s.args.is_empty() && params.contains(&s.name.name))
                || s.args.iter().any(|a| matches!(a, TypeArg::Type { ty, .. } if mentions(ty, params)))
        }),
        TypeKind::Function(f) => {
            f.params.iter().any(|p| mentions(&p.ty, params))
                || mentions(&f.result, params)
                || f.receiver.as_ref().is_some_and(|r| mentions(r, params))
        }
        TypeKind::Error => false,
    }
}

impl Gen<'_, '_> {
    /// Whether the user's type `rust` has `__kz_out` (see the module's doc): a final data class or a sealed class
    /// (not an interface) of type parameters, its values copied with each mapped.
    pub(crate) fn out_mappable(&self, rust: &str) -> bool {
        self.out_mappable_in(rust, &mut Vec::new())
    }

    fn out_mappable_in(&self, rust: &str, seen: &mut Vec<String>) -> bool {
        if seen.iter().any(|s| s == rust) {
            return true;
        }
        seen.push(rust.to_string());
        let Some(d) = self.decl_of(rust) else { return false };
        let ClassAst::Class(c) = &d.ast else { return false };
        if c.type_params.is_empty() {
            return false;
        }
        let params: Vec<String> = c.type_params.iter().map(|p| p.name.name.clone()).collect();
        // (a class of `out` parameters, or a case of a sealed one: what Kotlin's variance converts)
        let out = |k: &Class| k.type_params.iter().any(|p| p.variance == Some(Variance::Out));
        let of_out_sealed = self.superclass(&d).is_some_and(|s| match &s.kind {
            super::hierarchy::SuperKind::Class(r) => self.decl_of(r).is_some_and(
                |sd| matches!(&sd.ast, ClassAst::Class(k) if sd.info.inherit == Inherit::Sealed && out(k)),
            ),
            _ => false,
        });
        if !out(c) && !of_out_sealed {
            return false;
        }
        match (d.info.shape, d.info.inherit) {
            (Shape::Data, Inherit::Final) => {
                // (its state: the constructor's `val`s, a base part holding none of the parameters)
                let props_ok = c.primary_constructor.iter().flat_map(|p| &p.params).all(|cp| match cp.property {
                    Some(Mutability::Val) => cp.param.ty.as_ref().is_some_and(|t| self.out_type_ok(t, &params, seen)),
                    Some(Mutability::Var) => cp.param.ty.as_ref().is_some_and(|t| !mentions(t, &params)),
                    None => true,
                });
                let body_ok = !c.body.members.iter().any(|m| {
                    matches!(m, Member::Decl(Decl::Property(p)) if p.getter.is_none() || !matches!(p.init, PropertyInit::None))
                });
                let super_ok = self.superclass(&d).is_none_or(|s| match &s.kind {
                    super::hierarchy::SuperKind::Class(r) => !self.base_generic(r),
                    _ => false,
                });
                props_ok && body_ok && super_ok && c.supers.iter().all(|s| s.delegate.is_none())
            }
            (Shape::Data | Shape::Plain, Inherit::Sealed) => {
                if self.base_generic(rust) {
                    return false;
                }
                self.sealed_cases(rust).iter().all(|(_, case)| {
                    let Some(cd) = self.decl_of(case) else { return false };
                    let generic = matches!(&cd.ast, ClassAst::Class(k) if !k.type_params.is_empty());
                    let Some(m) = self.sealed_case_map(case, rust) else { return false };
                    if m.fixed.iter().any(Option::is_some) {
                        return false;
                    }
                    !generic || self.out_mappable_in(case, seen)
                })
            }
            _ => false,
        }
    }

    /// Whether a property's type is one the mapping reaches (see the module's doc).
    fn out_type_ok(&self, t: &Type, params: &[String], seen: &mut Vec<String>) -> bool {
        if !mentions(t, params) {
            return true;
        }
        let TypeKind::Named(segs) = &t.kind else { return false };
        let [seg] = segs.as_slice() else { return false };
        let args: Vec<&Type> = seg
            .args
            .iter()
            .filter_map(|a| match a {
                TypeArg::Type { ty, .. } => Some(ty),
                TypeArg::Star(_) => None,
            })
            .collect();
        if args.is_empty() {
            return params.contains(&seg.name.name);
        }
        match seg.name.name.as_str() {
            "List" | "Pair" => args.iter().all(|a| self.out_type_ok(a, params, seen)),
            n => {
                let Some(c) = self.user_class(n) else { return false };
                let rust = c.rust.clone();
                args.iter().all(|a| self.out_type_ok(a, params, seen)) && self.out_mappable_in(&rust, seen)
            }
        }
    }

    /// A value `v` of the Kotlin type `t` mapped: each type parameter's value by its function (`fs`, a `&dyn Fn`
    /// each).
    fn out_value(&mut self, t: &Type, v: TokenStream, fs: &HashMap<String, TokenStream>, span: Span) -> TokenStream {
        let params: Vec<String> = fs.keys().cloned().collect();
        if !mentions(t, &params) {
            return v;
        }
        let x = format_ident!("__kz_x", span = span);
        let inner = Type { nullable: false, ..t.clone() };
        let mapped = |me: &mut Self, v: TokenStream| -> TokenStream {
            let TypeKind::Named(segs) = &inner.kind else { return v };
            let seg = &segs[0];
            let args: Vec<Type> = seg
                .args
                .iter()
                .filter_map(|a| match a {
                    TypeArg::Type { ty, .. } => Some(ty.clone()),
                    TypeArg::Star(_) => None,
                })
                .collect();
            if args.is_empty() {
                let f = &fs[&seg.name.name];
                return quote_spanned!(span=> (#f)(#v));
            }
            match seg.name.name.as_str() {
                "List" => {
                    let e = me.out_value(&args[0], quote!(#x), fs, span);
                    quote_spanned!(span=> ::core::iter::Iterator::collect::<::std::vec::Vec<_>>(
                        ::core::iter::Iterator::map(::core::iter::IntoIterator::into_iter(#v), |#x| #e)))
                }
                "Pair" => {
                    let (a, b) = (format_ident!("__kz_a", span = span), format_ident!("__kz_b", span = span));
                    let ea = me.out_value(&args[0], quote!(#a), fs, span);
                    let eb = me.out_value(&args[1], quote!(#b), fs, span);
                    quote_spanned!(span=> { let (#a, #b) = #v; (#ea, #eb) })
                }
                _ => {
                    let closures: Vec<TokenStream> = args
                        .iter()
                        .map(|a| {
                            let e = me.out_value(a, quote!(#x), fs, span);
                            quote_spanned!(span=> &|#x| #e)
                        })
                        .collect();
                    quote_spanned!(span=> (#v).__kz_out(#(#closures),*))
                }
            }
        };
        if t.nullable {
            let e = mapped(self, quote!(#x));
            quote_spanned!(span=> ::core::option::Option::map(#v, |#x| #e))
        } else {
            mapped(self, v)
        }
    }

    /// The type parameters of a class, their mapping's targets (`__KzU0`…), the functions' names (`__kz_f0`…), and
    /// the method's generics and parameters.
    fn out_signature(
        &mut self,
        params: &[TypeParam],
        g_decl: &TokenStream,
        span: Span,
    ) -> (HashMap<String, TokenStream>, TokenStream, Vec<TokenStream>, Vec<proc_macro2::Ident>) {
        let mut targets = HashMap::new();
        let mut fs = HashMap::new();
        let mut fparams = Vec::new();
        let mut us = Vec::new();
        for (k, p) in params.iter().enumerate() {
            let u = format_ident!("__KzU{k}", span = span);
            let f = format_ident!("__kz_f{k}", span = span);
            let t = proc_macro2::Ident::new(&p.name.name, span);
            targets.insert(p.name.name.clone(), u.clone());
            fs.insert(p.name.name.clone(), quote!(#f));
            fparams.push(quote_spanned!(span=> #f: &dyn ::core::ops::Fn(#t) -> #u));
            us.push(u);
        }
        let g_u = renamed(g_decl.clone(), &targets);
        (fs, g_u, fparams, us)
    }

    /// A data class's `__kz_out` (see the module's doc); none for one that isn't mapped.
    pub(crate) fn out_method_data(
        &mut self,
        d: &super::class::ClassDecl,
        g_decl: &TokenStream,
        g_use: &TokenStream,
        span: Span,
    ) -> TokenStream {
        if !self.out_mappable(&d.info.rust) {
            return quote!();
        }
        let ClassAst::Class(c) = &d.ast else { return quote!() };
        let name = proc_macro2::Ident::new(&d.info.rust, span);
        let (fs, g_u, fparams, us) = self.out_signature(&c.type_params, g_decl, span);
        let mut fields = Vec::new();
        for cp in c.primary_constructor.iter().flat_map(|p| &p.params) {
            if cp.property.is_none() {
                continue;
            }
            let i = self.name_ident(&super::names::snake(&cp.param.name.name), cp.param.name.loc);
            let v = match &cp.param.ty {
                Some(t) if cp.property == Some(Mutability::Val) => self.out_value(t, quote!(self.#i), &fs, span),
                _ => quote_spanned!(span=> self.#i),
            };
            fields.push(quote_spanned!(span=> #i: #v));
        }
        if self.superclass(d).is_some() {
            fields.push(quote_spanned!(span=> __kz_super: self.__kz_super));
        }
        let allow = super::class::class_allow(span);
        quote_spanned! {span=>
            #allow
            impl #g_decl #name #g_use {
                #[doc(hidden)]
                pub fn __kz_out #g_u (self, #(#fparams),*) -> #name<#(#us),*> {
                    #name { #(#fields),* }
                }
            }
        }
    }

    /// A sealed class's `__kz_out` (see the module's doc): each case mapped (a case of none of the parameters as it
    /// is); none for one that isn't mapped.
    pub(crate) fn out_method_sealed(
        &mut self,
        d: &super::class::ClassDecl,
        cases: &[(String, String)],
        g_decl: &TokenStream,
        g_use: &TokenStream,
        phantom: &TokenStream,
        span: Span,
    ) -> TokenStream {
        if !self.out_mappable(&d.info.rust) {
            return quote!();
        }
        let ClassAst::Class(c) = &d.ast else { return quote!() };
        let name = proc_macro2::Ident::new(&d.info.rust, span);
        let (fs, g_u, fparams, us) = self.out_signature(&c.type_params, g_decl, span);
        let mut arms = Vec::new();
        for (v, case) in cases {
            let vi = proc_macro2::Ident::new(v, span);
            let Some(m) = self.sealed_case_map(case, &d.info.rust) else { continue };
            let arm = if m.case_args.is_empty() {
                quote_spanned!(span=> #name::#vi(c) => #name::#vi(c))
            } else {
                let given: Vec<TokenStream> = m
                    .case_args
                    .iter()
                    .map(|t| match t.simple_name().and_then(|n| fs.get(&n).cloned()) {
                        Some(f) => f,
                        None => quote_spanned!(span=> &|x| x),
                    })
                    .collect();
                quote_spanned!(span=> #name::#vi(c) => #name::#vi(c.__kz_out(#(#given),*)))
            };
            arms.push(arm);
        }
        // (the hidden variant of a parameter no case takes: never a value)
        let phantom = if phantom.is_empty() {
            quote!()
        } else {
            quote_spanned!(span=> #name::__KzOf(_, never) => match never {},)
        };
        let allow = super::class::class_allow(span);
        quote_spanned! {span=>
            #allow
            impl #g_decl #name #g_use {
                #[doc(hidden)]
                pub fn __kz_out #g_u (self, #(#fparams),*) -> #name<#(#us),*> {
                    match self { #(#arms,)* #phantom }
                }
            }
        }
    }

    /// Whether a value of `from` goes where `to` is by the class's variance: one user's mapped class of other type
    /// arguments, each the same or (an `out` parameter's) one coerced.
    pub(crate) fn out_coerces(&self, from: &Ty, to: &Ty) -> bool {
        let (Ty::Named(f, fa), Ty::Named(t, ta)) = (from.non_null(), to.non_null()) else { return false };
        let (fa, ta): (Vec<Ty>, Vec<Ty>) =
            (fa.iter().map(|t| self.dyn_norm(t)).collect(), ta.iter().map(|t| self.dyn_norm(t)).collect());
        if f != t || fa == ta || fa.len() != ta.len() || !self.out_mappable(&f) {
            return false;
        }
        let Some(d) = self.decl_of(&f) else { return false };
        let ClassAst::Class(c) = &d.ast else { return false };
        c.type_params.iter().zip(fa.iter().zip(ta.iter())).all(|(p, (a, b))| {
            a == b
                || (p.variance == Some(Variance::Out)
                    && *a != Ty::Unknown
                    && *b != Ty::Unknown
                    && (self.coerces(a, b) || self.out_coerces(a, b)))
        })
    }

    /// A type as Kotlin's types are the generator's: the shared object of the user's interface or open class
    /// (`Rc<dyn AnimalOpen>`, as the index's Rust texts have it) its trait's name (`AnimalOpen`).
    pub(crate) fn dyn_norm(&self, t: &Ty) -> Ty {
        match t {
            Ty::Named(n, a) if n == "Rc" => match a.as_slice() {
                // (the index's texts without spaces: `Rc<dynAnimalOpen>`)
                [Ty::Named(x, xa)]
                    if self.classes.classes.values().any(|c| {
                        c.is_dyn() && (c.value_rust() == *x || x.strip_prefix("dyn") == Some(&c.value_rust()))
                    }) =>
                {
                    Ty::Named(x.strip_prefix("dyn").unwrap_or(x).to_string(), xa.clone())
                }
                _ => t.clone(),
            },
            Ty::Nullable(i) => Ty::Nullable(Box::new(self.dyn_norm(i))),
            Ty::Named(n, a) => Ty::Named(n.clone(), a.iter().map(|x| self.dyn_norm(x)).collect()),
            _ => t.clone(),
        }
    }

    /// A value of `from` where `to` is taken by the class's variance ([`out_coerces`](Self::out_coerces)).
    pub(crate) fn out_coerce(&mut self, v: TokenStream, from: &Ty, to: &Ty, span: Span) -> TokenStream {
        let (Ty::Named(_, fa), Ty::Named(_, ta)) = (from.non_null(), to.non_null()) else { return v };
        let (fa, ta): (Vec<Ty>, Vec<Ty>) =
            (fa.iter().map(|t| self.dyn_norm(t)).collect(), ta.iter().map(|t| self.dyn_norm(t)).collect());
        let x = format_ident!("__kz_x", span = span);
        let closures: Vec<TokenStream> = fa
            .iter()
            .zip(ta.iter())
            .map(|(a, b)| {
                let (at, bt) = (self.rust_of_ty(a, span), self.rust_of_ty(b, span));
                let e = if a == b {
                    quote!(#x)
                } else if self.out_coerces(a, b) {
                    self.out_coerce(quote!(#x), a, b, span)
                } else {
                    self.coerce(quote!(#x), a, b, span)
                };
                quote_spanned!(span=> &|#x: #at| -> #bt { #e })
            })
            .collect();
        let mapped = if from.is_nullable() {
            let y = format_ident!("__kz_y", span = span);
            quote_spanned!(span=> ::core::option::Option::map(#v, |#y| #y.__kz_out(#(#closures),*)))
        } else {
            quote_spanned!(span=> (#v).__kz_out(#(#closures),*))
        };
        if to.is_nullable() && !from.is_nullable() {
            quote_spanned!(span=> ::core::option::Option::Some(#mapped))
        } else {
            mapped
        }
    }
}
