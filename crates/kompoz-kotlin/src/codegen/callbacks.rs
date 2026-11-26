//! (stage 14 WPA-rest) The API's base classes of the second kind: a Kotlin abstract class that is a struct of the
//! API whose open members are a trait of callbacks a constructor of the struct takes (the index's `Base::value`;
//! the trait's doc line ``Kotlin: `abstract class NavigationEventHandler<T>(initialInfo: T, …)` ``).
//!
//! | Kotlin | Rust |
//! |---|---|
//! | `object : NavigationEventHandler<Info>(info, isBackEnabled = true) { override fun onBackCompleted() { … } }` | `NavigationEventHandler::new(info, true, __KzObject { m0: Rc::new(move \|__kz_this: &NavigationEventHandler<Info>\| { … }) })`, the object's `impl NavigationEventHandlerCallbacks<Info>` calling its closures with the handler each method is given (Kotlin's `this`: `transitionState`, `setInfo(…)` in the bodies are the handler's) |
//! | `object : NavType<WrappedUUID>(isNullableAllowed = false) { override fun put(bundle: SavedState, key: String, value: WrappedUUID) … }` | `NavType::new(__KzObject { … })`: `type Value = WrappedUUID`, a constructor's parameter no constructor function takes the trait's method of its name (`is_nullable_allowed()` giving `false`), the methods' Rust parameters given as Kotlin's (`&str` a `String`, `Option<&T>` a `T` or a `T?`), their results made Rust's (`T` → `Ok(Some(v))` of a `Result<Option<T>, _>`) |
//!
//! The constructor function is the struct's whose last parameter takes the trait (`impl NavigationEventHandlerCallbacks<T>`)
//! and whose others are the arguments given (`new(initial_info, is_back_enabled, …)`, `with_forward(…)` when
//! `isForwardEnabled` is given too).

use super::Gen;
use super::names::{ident, snake};
use super::resolve::*;
use super::types::TypePos;
use crate::ast::*;
use crate::index;
use crate::pos::Loc;
use proc_macro2::{Span, TokenStream, TokenTree};
use quote::{format_ident, quote, quote_spanned};
use std::collections::HashMap;

/// A Rust type's text of the index made tokens, its type parameters (`T`, `Self::Value`) the object's arguments.
pub(crate) fn substituted(text: &str, binds: &HashMap<String, TokenStream>, span: Span) -> TokenStream {
    let ts: TokenStream = text.parse().unwrap_or_else(|_| quote!(_));
    let tokens: Vec<TokenTree> = ts.into_iter().collect();
    let mut out = TokenStream::new();
    let mut k = 0;
    while k < tokens.len() {
        // (`Self::Value`: the associated type's argument)
        if let (TokenTree::Ident(s), Some(TokenTree::Punct(a)), Some(TokenTree::Punct(b)), Some(TokenTree::Ident(n))) =
            (&tokens[k], tokens.get(k + 1), tokens.get(k + 2), tokens.get(k + 3))
            && s == "Self"
            && a.as_char() == ':'
            && b.as_char() == ':'
            && let Some(t) = binds.get(&n.to_string())
        {
            out.extend(t.clone());
            k += 4;
            continue;
        }
        let after_path = k > 0 && matches!(&tokens[k - 1], TokenTree::Punct(p) if p.as_char() == ':');
        match &tokens[k] {
            TokenTree::Ident(i) if binds.contains_key(&i.to_string()) => out.extend(binds[&i.to_string()].clone()),
            // (the prelude's types by their paths: a user's `Result` may be in scope)
            TokenTree::Ident(i) if !after_path && i == "Result" => {
                out.extend(quote_spanned!(span=> ::core::result::Result))
            }
            TokenTree::Ident(i) if !after_path && i == "Option" => {
                out.extend(quote_spanned!(span=> ::core::option::Option))
            }
            TokenTree::Ident(i) if !after_path && i == "String" => {
                out.extend(quote_spanned!(span=> ::std::string::String))
            }
            TokenTree::Ident(i) if !after_path && i == "Vec" => out.extend(quote_spanned!(span=> ::std::vec::Vec)),
            TokenTree::Ident(i) if !after_path && i == "Box" => out.extend(quote_spanned!(span=> ::std::boxed::Box)),
            TokenTree::Ident(i) if !after_path && i == "Rc" => out.extend(quote_spanned!(span=> ::std::rc::Rc)),
            TokenTree::Ident(i) if !after_path && i == "Pin" => out.extend(quote_spanned!(span=> ::core::pin::Pin)),
            TokenTree::Ident(i) if !after_path && i == "Future" => {
                out.extend(quote_spanned!(span=> ::core::future::Future))
            }
            TokenTree::Group(g) => {
                let inner = substituted(&g.stream().to_string(), binds, span);
                let mut ng = proc_macro2::Group::new(g.delimiter(), inner);
                ng.set_span(span);
                out.extend([TokenTree::Group(ng)]);
            }
            t => {
                let mut t = t.clone();
                t.set_span(span);
                out.extend([t]);
            }
        }
        k += 1;
    }
    out
}

impl Gen<'_, '_> {
    /// The type of an object expression extending a base of the second kind: its struct's, of the expression's type
    /// arguments where the struct is generic (`NavigationEventHandler<Info>`, `NavType`).
    pub(crate) fn callbacks_value_ty(&self, b: index::Base, args: &[Type]) -> Ty {
        let generic = index::generics(b.value).is_some_and(|g| !g.is_empty());
        // ((stage 14 WPD) an API interface's: its shared trait object — `NavigationEventHandler<NavigationEventInfo>`'s
        // `Rc<dyn NavigationEventInfo>`)
        let arg = |a: &Type| match self.ty_of(a) {
            Ty::Named(t, targs) if targs.is_empty() && super::resolve::trait_object(&t).is_some() => {
                let ptr = super::resolve::trait_object(&t).unwrap_or("Rc");
                Ty::Named(ptr.to_string(), vec![Ty::Named(t, Vec::new())])
            }
            t => t,
        };
        let args = if generic { args.iter().map(arg).collect() } else { Vec::new() };
        Ty::Named(b.value.to_string(), args)
    }

    /// `object : Base<A>(args) { overrides }` of a base of the second kind (see the module's doc).
    pub(crate) fn object_expr_callbacks(
        &mut self,
        b: index::Base,
        s: &SuperType,
        body: &ClassBody,
        loc: Loc,
    ) -> TokenStream {
        let span = self.span(loc);
        let kt = self.kt(span);
        let targs: Vec<Type> = match &s.ty.kind {
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
        };
        let targ_ts: Vec<TokenStream> = targs.iter().map(|t| self.type_arg(t)).collect();
        // (the trait's type parameters and associated types: the expression's arguments, in order)
        let mut binds: HashMap<String, TokenStream> = HashMap::new();
        for (p, t) in index::generics(b.rust).unwrap_or_default().iter().zip(&targ_ts) {
            binds.insert(p.to_string(), t.clone());
        }
        for (a, t) in b.assoc.iter().zip(&targ_ts) {
            binds.insert(a.to_string(), t.clone());
        }
        let trait_generic = index::generics(b.rust).is_some_and(|g| !g.is_empty());
        let tr = {
            let i = ident(b.rust, span);
            if trait_generic { quote_spanned!(span=> #i<#(#targ_ts),*>) } else { quote_spanned!(span=> #i) }
        };
        let value_ty = self.callbacks_value_ty(b, &targs);
        let value_ts = self.rust_of_ty(&value_ty, span);
        let trait_items = Self::callbacks_trait_items(b);
        let handler_first = |i: &index::Item| handler_param(b, i);
        // The constructor function: the struct's taking the trait last, of the arguments given.
        let args = s.constructor_args.clone().unwrap_or_default();
        let Some((ctor, given)) = self.callbacks_ctor(b, &args, &trait_items, s.loc) else {
            let span = self.span(s.loc);
            return quote_spanned!(span=> ::core::unreachable!());
        };
        let own_params = &ctor.params[..ctor.params.len() - 1];
        // (its arguments, in its parameters' order; the others the trait's methods' values)
        let mut call_args = Vec::new();
        for p in own_params {
            match given.iter().find(|(n, _)| *n == p.kotlin) {
                Some((_, e)) => {
                    let v = self.value(e);
                    call_args.push(if p.ty == "String" {
                        quote_spanned!(span=> ::std::string::String::from(#v))
                    } else {
                        v
                    });
                }
                None => {
                    self.error(s.loc, format!("give `{}`'s `{}`", b.kotlin, p.kotlin));
                    call_args.push(quote_spanned!(span=> ::core::unreachable!()));
                }
            }
        }
        let mut fields = Vec::new();
        let mut values = Vec::new();
        let mut methods = Vec::new();
        for (n, e) in &given {
            if own_params.iter().any(|p| p.kotlin == *n) {
                continue;
            }
            let Some(item) = trait_items.iter().find(|i| i.rust == snake(n) && i.params.is_empty()) else { continue };
            let k = fields.len();
            let field = format_ident!("c{k}", span = span);
            let t = substituted(item.ret, &binds, span);
            let v = self.value(e);
            let m = ident(item.rust, span);
            fields.push(quote_spanned!(span=> #field: #t));
            values.push(quote_spanned!(span=> #field: #v));
            methods.push(quote_spanned!(span=> fn #m(&self) -> #t { ::core::clone::Clone::clone(&self.#field) }));
        }
        for a in b.assoc.iter().zip(&targ_ts) {
            let (n, t) = (ident(a.0, span), a.1);
            methods.push(quote_spanned!(span=> type #n = #t;));
        }
        // The overrides.
        for m in &body.members {
            match m {
                Member::Decl(Decl::Function(f)) if f.receiver.is_none() && f.type_params.is_empty() => {
                    let fspan = self.span(f.loc);
                    let Some(item) = trait_items.iter().copied().find(|i| {
                        i.kotlin == f.name.name && i.params.len() == f.params.len() + usize::from(handler_first(i))
                    }) else {
                        self.error(f.name.loc, format!("`{}` overrides no function of `{}`", f.name.name, b.kotlin));
                        continue;
                    };
                    let handler = handler_first(item);
                    let rust_params: Vec<&index::Param> = item.params.iter().skip(usize::from(handler)).collect();
                    // (the closure in Kotlin's terms: its parameters' declared types, its result's)
                    let kparams: Vec<TokenStream> = f
                        .params
                        .iter()
                        .zip(&rust_params)
                        .map(|(p, rp)| match &p.ty {
                            Some(t) => self.rust_type(t, TypePos::Value),
                            None => substituted(rp.ty, &binds, fspan),
                        })
                        .collect();
                    let rust_ret = (item.ret.trim() != "()").then(|| substituted(item.ret, &binds, fspan));
                    let kret: Option<TokenStream> = match (&f.result, &rust_ret) {
                        (Some(t), _) if self.ty_of(t) != Ty::Unit => Some(self.rust_type(t, TypePos::Value)),
                        (Some(_), _) => None,
                        (None, r) => r.clone(),
                    };
                    let names: Vec<proc_macro2::Ident> =
                        f.params.iter().map(|p| self.name_ident(&snake(&p.name.name), p.name.loc)).collect();
                    let k = fields.len();
                    let field = format_ident!("m{k}", span = fspan);
                    let recv = handler.then(|| quote_spanned!(fspan=> &#value_ts,));
                    let r = kret.as_ref().map(|r| quote_spanned!(fspan=> -> #r));
                    fields.push(
                        quote_spanned!(fspan=> #field: ::std::rc::Rc<dyn ::core::ops::Fn(#recv #(#kparams),*) #r>),
                    );
                    let receiver_ts = value_ts.clone();
                    let closure = self.object_fn_closure_with(
                        f,
                        &kparams,
                        kret.as_ref(),
                        &names,
                        handler.then_some((&receiver_ts, b.value)),
                    );
                    values.push(quote_spanned!(fspan=> #field: ::std::rc::Rc::new(#closure)));
                    // (the trait's method: its Rust parameters made Kotlin's, the closure's result Rust's)
                    let m = ident(item.rust, fspan);
                    let mut sig = Vec::new();
                    let mut call = Vec::new();
                    if handler {
                        let h = ident(item.params[0].rust, fspan);
                        let t = substituted(item.params[0].ty, &binds, fspan);
                        sig.push(quote_spanned!(fspan=> #h: #t));
                        call.push(quote_spanned!(fspan=> #h));
                    }
                    for ((p, rp), n) in f.params.iter().zip(&rust_params).zip(&names) {
                        let t = substituted(rp.ty, &binds, fspan);
                        sig.push(quote_spanned!(fspan=> #n: #t));
                        let nullable = p.ty.as_ref().is_some_and(|t| t.nullable);
                        call.push(as_kotlin_arg(&kt, rp.ty, nullable, quote!(#n), fspan));
                    }
                    let knullable = f.result.as_ref().is_some_and(|t| t.nullable);
                    let ret = rust_ret.as_ref().map(|r| quote_spanned!(fspan=> -> #r));
                    let v = quote_spanned!(fspan=> (self.#field)(#(#call),*));
                    let v = match &rust_ret {
                        Some(_) => as_rust_result(item.ret, knullable, v, fspan),
                        None => v,
                    };
                    methods.push(quote_spanned!(fspan=> fn #m(&self, #(#sig),*) #ret { #v }));
                }
                Member::Decl(Decl::Property(p)) if !p.is_var() && matches!(p.init, PropertyInit::Value(_)) => {
                    let (Binding::Name(n), PropertyInit::Value(v)) = (&p.binding, &p.init) else { continue };
                    let pspan = self.span(p.loc);
                    let Some(item) = trait_items.iter().find(|i| i.kotlin == n.name && i.params.is_empty()) else {
                        self.error(n.loc, format!("`{}` overrides no property of `{}`", n.name, b.kotlin));
                        continue;
                    };
                    let k = fields.len();
                    let field = format_ident!("p{k}", span = pspan);
                    let t = substituted(item.ret, &binds, pspan);
                    let val = self.value_want(v, super::expr::Want { str_ok: false, ..Default::default() });
                    let m = ident(item.rust, pspan);
                    fields.push(quote_spanned!(pspan=> #field: #t));
                    values.push(quote_spanned!(pspan=> #field: #val));
                    methods
                        .push(quote_spanned!(pspan=> fn #m(&self) -> #t { ::core::clone::Clone::clone(&self.#field) }));
                }
                Member::Decl(d) => {
                    self.later(d.loc(), "this member of an object expression extending a struct's class (`NavType`)");
                }
                Member::Init(b) => {
                    self.later(b.loc, "an `init` block in an object expression");
                }
                Member::Constructor(c) => self.error(c.loc, "an object has no constructors"),
            }
        }
        let owner = ident(ctor.owner, span);
        let f = ident(ctor.rust, span);
        quote_spanned! {span=>
            {
                #[allow(non_camel_case_types, clippy::type_complexity)]
                struct __KzObject { #(#fields),* }
                #[allow(non_snake_case, clippy::all)]
                impl #tr for __KzObject { #(#methods)* }
                #owner::#f(#(#call_args,)* __KzObject { #(#values),* })
            }
        }
    }

    /// The constructor function of a base of the second kind the super call's `args` go to (the struct's taking the
    /// trait last, its other parameters given or defaulted, the arguments it doesn't take the trait's methods'), and
    /// the arguments by Kotlin's names.
    fn callbacks_ctor(
        &mut self,
        b: index::Base,
        args: &[Arg],
        trait_items: &[&'static index::Item],
        loc: Loc,
    ) -> Option<(&'static index::Item, Vec<(String, Expr)>)> {
        let mut given: Vec<(String, Expr)> = Vec::new();
        for (k, a) in args.iter().enumerate() {
            let name = match &a.name {
                Some(n) => n.name.clone(),
                None => match b.params.get(k) {
                    Some(p) => p.to_string(),
                    None => {
                        self.error(a.loc, format!("`{}`'s constructor takes {} argument(s)", b.kotlin, b.params.len()));
                        continue;
                    }
                },
            };
            given.push((name, a.value.clone()));
        }
        let ctors: Vec<&'static index::Item> = index::ITEMS
            .iter()
            .filter(|i| {
                i.owner == b.value
                    && i.kind == index::Kind::Function
                    && i.params.last().is_some_and(|p| p.ty.replace(' ', "").starts_with(&format!("impl{}", b.rust)))
            })
            .collect();
        let method_named = |n: &str| trait_items.iter().any(|i| i.rust == snake(n) && i.params.is_empty());
        let fits = |c: &index::Item| {
            let own = &c.params[..c.params.len() - 1];
            own.iter().all(|p| p.default != index::Default::None || given.iter().any(|(n, _)| *n == p.kotlin))
                && given.iter().all(|(n, _)| own.iter().any(|p| p.kotlin == *n) || method_named(n))
        };
        match ctors.iter().copied().filter(|c| fits(c)).max_by_key(|c| c.params.len()) {
            Some(c) => Some((c, given)),
            None => {
                self.error(loc, format!("no constructor of `{}` takes these arguments with its callbacks", b.kotlin));
                None
            }
        }
    }

    /// (stage 14 FINAL-T) Whether a member of the user's object `rust` overrides a method of its base of the second
    /// kind that is given the struct (`onBackCompleted` of `object X : NavigationEventHandler<…>(…)`): the trait's
    /// method is its body, not one of the object's own.
    pub(crate) fn handler_override(&self, rust: &str, f: &Function) -> bool {
        let Some(b) = self.callbacks_base_of(rust) else { return false };
        super::hierarchy::is_override(&f.modifiers)
            && Self::callbacks_trait_items(b).iter().any(|i| i.kotlin == f.name.name && handler_param(b, i))
    }

    /// The trait's methods (the API's).
    fn callbacks_trait_items(b: index::Base) -> Vec<&'static index::Item> {
        index::ITEMS.iter().filter(|i| i.via == b.rust && i.kind == index::Kind::Method && i.owner.is_empty()).collect()
    }

    /// The base of the second kind an object of the user's derives from (`object WrappedUUIDNavType :
    /// NavType<WrappedUUID>(…)`), by the object's Rust name.
    pub(crate) fn callbacks_base_of(&self, rust: &str) -> Option<index::Base> {
        let d = self.decl_of(rust)?;
        if d.info.shape != super::class::Shape::Object {
            return None;
        }
        match self.superclass(&d)?.kind {
            super::hierarchy::SuperKind::Base(b) if !b.value.is_empty() => Some(b),
            _ => None,
        }
    }

    /// A class (an object) deriving from a base of the second kind: the trait's items its object implements (its
    /// overrides, Rust's parameters made Kotlin's; the constructor's arguments no constructor function takes the
    /// trait's methods'), and the struct made of the object (`From`).
    pub(crate) fn callbacks_class_items(
        &mut self,
        d: &super::class::ClassDecl,
        b: index::Base,
        args: &[Type],
        place: super::hierarchy::Place,
        span: Span,
    ) -> (Vec<TokenStream>, TokenStream) {
        let kt = self.kt(span);
        let targ_ts: Vec<TokenStream> = args.iter().map(|t| self.type_arg(t)).collect();
        let mut binds: HashMap<String, TokenStream> = HashMap::new();
        for (p, t) in index::generics(b.rust).unwrap_or_default().iter().zip(&targ_ts) {
            binds.insert(p.to_string(), t.clone());
        }
        for (a, t) in b.assoc.iter().zip(&targ_ts) {
            binds.insert(a.to_string(), t.clone());
        }
        let trait_items = Self::callbacks_trait_items(b);
        let mut items = Vec::new();
        for (a, t) in b.assoc.iter().zip(&targ_ts) {
            let n = ident(a, span);
            items.push(quote_spanned!(span=> type #n = #t;));
        }
        let name = ident(&d.info.rust, span);
        let Some(sup) = self.superclass(d) else { return (items, quote!()) };
        let Some((ctor, given)) = self.callbacks_ctor(b, sup.args.as_deref().unwrap_or(&[]), &trait_items, sup.loc)
        else {
            return (items, quote!());
        };
        let own_params = &ctor.params[..ctor.params.len() - 1];
        for (n, e) in &given {
            if own_params.iter().any(|p| p.kotlin == *n) {
                continue;
            }
            let Some(item) = trait_items.iter().find(|i| i.rust == snake(n) && i.params.is_empty()) else { continue };
            let t = substituted(item.ret, &binds, span);
            let v = self.value(e);
            let m = ident(item.rust, span);
            items.push(quote_spanned!(span=> fn #m(&self) -> #t { #v }));
        }
        let members = match &d.ast {
            super::class::ClassAst::Class(c) => c.body.members.clone(),
            super::class::ClassAst::Object(o) => o.body.members.clone(),
        };
        let recv = if place == super::hierarchy::Place::Enum {
            quote_spanned!(span=> *self)
        } else {
            quote_spanned!(span=> self)
        };
        for m in &members {
            match m {
                Member::Decl(Decl::Function(f)) if super::hierarchy::is_override(&f.modifiers) => {
                    let Some(item) = trait_items.iter().copied().find(|i| i.kotlin == f.name.name) else { continue };
                    // ((stage 14 FINAL-T) one given the struct, `onBackCompleted()` of `object X :
                    // NavigationEventHandler<…>(…)`: its body with the object's members and then the struct's
                    // (`isBackEnabled`, `transitionState`) in scope, Kotlin's one `this`)
                    if handler_param(b, item) {
                        let fspan = self.span(f.loc);
                        let value_ty = self.callbacks_value_ty(b, args);
                        let value_ts = self.rust_of_ty(&value_ty, fspan);
                        let rust_params: Vec<&index::Param> = item.params.iter().skip(1).collect();
                        let kparams: Vec<TokenStream> = f
                            .params
                            .iter()
                            .zip(&rust_params)
                            .map(|(p, rp)| match &p.ty {
                                Some(t) => self.rust_type(t, TypePos::Value),
                                None => substituted(rp.ty, &binds, fspan),
                            })
                            .collect();
                        let rust_ret = (item.ret.trim() != "()").then(|| substituted(item.ret, &binds, fspan));
                        let kret: Option<TokenStream> = match (&f.result, &rust_ret) {
                            (Some(t), _) if self.ty_of(t) != Ty::Unit => Some(self.rust_type(t, TypePos::Value)),
                            (Some(_), _) => None,
                            (None, r) => r.clone(),
                        };
                        let names: Vec<proc_macro2::Ident> =
                            f.params.iter().map(|p| self.name_ident(&snake(&p.name.name), p.name.loc)).collect();
                        let cx = super::class::MemberCx {
                            owner: d.info.rust.clone(),
                            shape: d.info.shape,
                            companion: None,
                            outers: self.outers_of(&d.info),
                        };
                        let label = d.info.kotlin.rsplit('.').next().unwrap_or_default().to_string();
                        let saved = (
                            std::mem::take(&mut self.scopes),
                            std::mem::take(&mut self.receivers),
                            std::mem::take(&mut self.frames),
                            std::mem::take(&mut self.loops),
                            std::mem::take(&mut self.lambdas),
                        );
                        self.enter_member(&cx, &label, true);
                        let closure =
                            self.object_fn_closure_with(f, &kparams, kret.as_ref(), &names, Some((&value_ts, b.value)));
                        (self.scopes, self.receivers, self.frames, self.loops, self.lambdas) = saved;
                        let h = ident(item.params[0].rust, fspan);
                        let ht = substituted(item.params[0].ty, &binds, fspan);
                        let mut sig = vec![quote_spanned!(fspan=> #h: #ht)];
                        let mut call = vec![quote_spanned!(fspan=> #h)];
                        for ((p, rp), n) in f.params.iter().zip(&rust_params).zip(&names) {
                            let t = substituted(rp.ty, &binds, fspan);
                            sig.push(quote_spanned!(fspan=> #n: #t));
                            let nullable = p.ty.as_ref().is_some_and(|t| t.nullable);
                            call.push(as_kotlin_arg(&kt, rp.ty, nullable, quote!(#n), fspan));
                        }
                        let mi = ident(item.rust, fspan);
                        let v = quote_spanned!(fspan=> (#closure)(#(#call),*));
                        let (ret, v) = match &rust_ret {
                            None => (quote!(), v),
                            Some(r) => {
                                let knullable = f.result.as_ref().is_some_and(|t| t.nullable);
                                (quote_spanned!(fspan=> -> #r), as_rust_result(item.ret, knullable, v, fspan))
                            }
                        };
                        items.push(quote_spanned!(fspan=> fn #mi(&self, #(#sig),*) #ret { #v }));
                        continue;
                    }
                    let fspan = self.span(f.loc);
                    let mut sig = Vec::new();
                    let mut call = Vec::new();
                    for (p, rp) in f.params.iter().zip(item.params) {
                        let n = ident(rp.rust, fspan);
                        let t = substituted(rp.ty, &binds, fspan);
                        sig.push(quote_spanned!(fspan=> #n: #t));
                        let nullable = p.ty.as_ref().is_some_and(|t| t.nullable);
                        call.push(as_kotlin_arg(&kt, rp.ty, nullable, quote!(#n), fspan));
                    }
                    let mi = ident(item.rust, fspan);
                    let own = self.name_ident(&snake(&f.name.name), f.name.loc);
                    let v = quote_spanned!(fspan=> #name::#own(#recv, #(#call),*));
                    let (ret, v) = if item.ret.trim() == "()" {
                        (quote!(), v)
                    } else {
                        let r = substituted(item.ret, &binds, fspan);
                        let knullable = f.result.as_ref().is_some_and(|t| t.nullable);
                        (quote_spanned!(fspan=> -> #r), as_rust_result(item.ret, knullable, v, fspan))
                    };
                    items.push(quote_spanned!(fspan=> fn #mi(&self, #(#sig),*) #ret { #v }));
                }
                Member::Decl(Decl::Property(p)) if super::hierarchy::is_override(&p.modifiers) => {
                    let Binding::Name(n) = &p.binding else { continue };
                    let Some(item) = trait_items.iter().find(|i| i.kotlin == n.name && i.params.is_empty()) else {
                        continue;
                    };
                    let pspan = self.span(p.loc);
                    let mi = ident(item.rust, pspan);
                    let own = self.name_ident(&snake(&n.name), n.loc);
                    let t = substituted(item.ret, &binds, pspan);
                    items.push(quote_spanned!(pspan=> fn #mi(&self) -> #t { #name::#own(#recv) }));
                }
                _ => {}
            }
        }
        // (the struct of the object: its constructor function's call)
        let mut call_args = Vec::new();
        for p in own_params {
            match given.iter().find(|(n, _)| *n == p.kotlin) {
                Some((_, e)) => {
                    let v = self.value(e);
                    call_args.push(if p.ty == "String" {
                        quote_spanned!(span=> ::std::string::String::from(#v))
                    } else {
                        v
                    });
                }
                None => call_args.push(quote_spanned!(span=> ::core::default::Default::default())),
            }
        }
        let value_ty = self.callbacks_value_ty(b, args);
        let value_ts = self.rust_of_ty(&value_ty, span);
        let owner = ident(ctor.owner, span);
        let f = ident(ctor.rust, span);
        let allow = super::class::class_allow(span);
        // ((stage 14 FINAL-T) an object of a base whose callbacks are given the struct, `object X :
        // NavigationEventHandler<…>(…)`: Kotlin's one object, the struct made once (per thread) — `addHandler(X)`
        // and `X.remove()` of the same handler)
        let single = d.info.shape == super::class::Shape::Object && trait_items.iter().any(|i| handler_param(b, i));
        let made = if single {
            quote_spanned! {span=>
                ::std::thread_local! {
                    static __KZ_ONE: ::std::cell::OnceCell<#value_ts> = const { ::std::cell::OnceCell::new() };
                }
                __KZ_ONE.with(|one| ::core::clone::Clone::clone(one.get_or_init(|| #owner::#f(#(#call_args,)* o))))
            }
        } else {
            quote_spanned!(span=> #owner::#f(#(#call_args,)* o))
        };
        let mut from = quote_spanned! {span=>
            #allow
            impl ::core::convert::From<#name> for #value_ts {
                fn from(o: #name) -> #value_ts {
                    #made
                }
            }
        };
        // ((stage 14 WPD) `object ProductSerializer : DeepLinkSerializer<Product>()`: its type `Saveable` through it,
        // what Kotlin's `@Serializable(with = ProductSerializer::class)` on a key's property or list element makes it
        // — on the type here, every such field of it read and saved through it)
        if b.kotlin == "DeepLinkSerializer"
            && let [t] = targ_ts.as_slice()
        {
            from.extend(quote_spanned!(span=> serializable_with!(#t, #name);));
        }
        (items, from)
    }
}

/// Whether a method of a base's trait is given the struct's object first (`handler: &NavigationEventHandler<T>`:
/// Kotlin's `this`).
fn handler_param(b: index::Base, i: &index::Item) -> bool {
    i.params.first().is_some_and(|p| p.ty.replace(' ', "").starts_with(&format!("&{}", b.value)))
}

/// A Rust method's parameter `v` (its type's text `rust`) as the Kotlin override takes it: a `&str` a `String`, a
/// reference its value's clone, an `Option<&T>` a `T?` (`nullable`) or a `T`.
fn as_kotlin_arg(kt: &TokenStream, rust: &str, nullable: bool, v: TokenStream, span: Span) -> TokenStream {
    let t = rust.replace(' ', "");
    if t == "&str" {
        quote_spanned!(span=> ::std::string::String::from(#v))
    } else if t.starts_with("Option<&") {
        if nullable {
            quote_spanned!(span=> ::core::option::Option::map(#v, ::core::clone::Clone::clone))
        } else {
            quote_spanned!(span=> #kt::not_null(::core::option::Option::map(#v, ::core::clone::Clone::clone)))
        }
    } else if t.starts_with('&') && !t.starts_with("&mut") && !t.starts_with("&dyn") {
        quote_spanned!(span=> ::core::clone::Clone::clone(#v))
    } else {
        v
    }
}

/// The Kotlin override's result `v` as the Rust method gives it (its type's text `rust`): a `T` in `Ok(Some(…))` of a
/// `Result<Option<T>, _>`, in `Some` of an `Option<T>` (a `T?` as it is).
fn as_rust_result(rust: &str, nullable: bool, v: TokenStream, span: Span) -> TokenStream {
    let t = rust.replace(' ', "");
    let (result, inner) = match t.strip_prefix("Result<") {
        Some(rest) => (true, rest.to_string()),
        None => (false, t.clone()),
    };
    let v = if inner.starts_with("Option<") && !nullable {
        quote_spanned!(span=> ::core::option::Option::Some(#v))
    } else {
        v
    };
    if result { quote_spanned!(span=> ::core::result::Result::Ok(#v)) } else { v }
}
