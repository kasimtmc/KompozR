//! KompozR's macros: what Compose's compiler plugin does for `@Composable` functions.
//!
//! `#[kompozable]` on a function:
//! - gives each call its identity (the call site, `#[track_caller]`) and, for a function returning nothing, a
//!   restartable scope: it runs again on its own when a state it read changes, and is skipped when its parent runs
//!   with arguments equal to the last ones (every parameter's type `PartialEq`; otherwise it is restartable but
//!   never skipped). `#[kompozable(inline)]` (and a function returning a value) runs in its caller's scope, as
//!   Compose's inline composables (`Column`, `Row`) do;
//! - lets its parameters have defaults, `#[default(expr)]`, and be named at the call (`Text("x", style = s)`);
//! - rewrites its body: calls into the callees' builders, closures passed to calls into ones capturing clones of
//!   the locals they use (see `transform`).
//!
//! `#[named]` gives the same named and defaulted parameters to a plain function or to the methods of an impl block;
//! `#[named(trait Name)]` on an impl puts its methods into a trait implemented for the type, so a crate can add
//! them to a type of another (Compose's extension functions: foundation's `Modifier.padding`).
//! `kompozable_lambda!(|| …)` is a closure written as a `#[kompozable]` body (content given from plain code);
//! `kompozable_block!({ … })` an expression written as one, in place (in a method).

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Ts;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;
use syn::*;

mod builder;
mod transform;

use builder::{Param, Receiver, Spec, builder, fn_sugar, generics, take_params};
use transform::{Transformer, is_pascal, pattern_names};

/// The runtime's path in the calling crate: through the umbrella crate (`::kompoz::runtime`) when it depends on
/// it, else `::kompoz_runtime` as it names it (both crates have `extern crate self as …`, for themselves).
fn runtime() -> Ts {
    use proc_macro_crate::{FoundCrate, crate_name};
    match crate_name("kompoz") {
        Ok(FoundCrate::Itself) => return quote!(::kompoz::runtime),
        Ok(FoundCrate::Name(n)) => {
            let i = format_ident!("{n}");
            return quote!(::#i::runtime);
        }
        Err(_) => {}
    }
    match crate_name("kompoz-runtime") {
        Ok(FoundCrate::Name(n)) => {
            let i = format_ident!("{n}");
            quote!(::#i)
        }
        Ok(FoundCrate::Itself) | Err(_) => quote!(::kompoz_runtime),
    }
}

#[proc_macro_attribute]
pub fn kompozable(attr: TokenStream, item: TokenStream) -> TokenStream {
    let inline = match attr.to_string().as_str() {
        "" => false,
        "inline" => true,
        other => {
            return Error::new(
                proc_macro2::Span::call_site(),
                format!("kompoz: unknown option `{other}` (only `inline`)"),
            )
            .to_compile_error()
            .into();
        }
    };
    let f = parse_macro_input!(item as ItemFn);
    expand_fn(f, Some(inline)).unwrap_or_else(|e| e.to_compile_error()).into()
}

#[proc_macro_attribute]
pub fn named(attr: TokenStream, item: TokenStream) -> TokenStream {
    let ext = if attr.is_empty() { None } else { Some(parse_macro_input!(attr as ExtensionTrait)) };
    let out = match parse_macro_input!(item as Item) {
        Item::Fn(f) if ext.is_none() => expand_fn(f, None),
        Item::Fn(f) => Err(Error::new(f.sig.span(), "kompoz: #[named(trait …)] goes on an impl block")),
        Item::Impl(i) => expand_impl(i, ext),
        other => Err(Error::new(other.span(), "kompoz: #[named] goes on a function or an impl block")),
    };
    out.unwrap_or_else(|e| e.to_compile_error()).into()
}

/// `#[named(trait Name)]` (or `#[named(pub(crate) trait Name)]`): the impl's methods go into a trait of that name
/// (public unless told otherwise), implemented for the type — methods on a type of another crate (Compose's
/// extension functions: `Modifier.padding(…)` from foundation), callable with named arguments where the trait is
/// in scope.
struct ExtensionTrait {
    vis: Visibility,
    name: Ident,
}

impl syn::parse::Parse for ExtensionTrait {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let vis: Visibility = input.parse()?;
        let vis = if matches!(vis, Visibility::Inherited) { parse_quote!(pub) } else { vis };
        input.parse::<Token![trait]>()?;
        let name: Ident = input.parse()?;
        if !input.is_empty() {
            return Err(input.error("kompoz: #[named(trait Name)] takes a trait's name and nothing else"));
        }
        Ok(ExtensionTrait { vis, name })
    }
}

/// A closure written as a `#[kompozable]` body: `set_content(kompozable_lambda!(move || App()))`.
#[proc_macro]
pub fn kompozable_lambda(item: TokenStream) -> TokenStream {
    let mut e = parse_macro_input!(item as Expr);
    let mut t = Transformer::lambda(runtime());
    t.transform_lambda(&mut e);
    quote!(#e).into()
}

/// An expression written as a `#[kompozable]` body, in place: composables called from a method of a type (that
/// isn't itself `#[kompozable]`), as `kompozable_block!({ LaunchedEffect(key, effect) })`.
#[proc_macro]
pub fn kompozable_block(item: TokenStream) -> TokenStream {
    let mut e = parse_macro_input!(item as Expr);
    let mut t = Transformer::lambda(runtime());
    t.visit_expr_mut(&mut e);
    quote!(#e).into()
}

/// The names of a signature's parameters (what its body sees).
fn param_names(sig: &Signature) -> Vec<String> {
    let mut names = Vec::new();
    for arg in &sig.inputs {
        match arg {
            FnArg::Typed(pt) => pattern_names(&pt.pat, &mut names),
            FnArg::Receiver(_) => names.push("self".into()),
        }
    }
    names
}

/// What a restartable composable keeps of a parameter: its value as given, an `impl Into<T>`'s `T`, an
/// `impl Fn`'s closure in an `Rc`.
enum Kept {
    Plain(Type),
    Into(Type),
    Closure(Type, ParenthesizedGenericArguments),
}

fn kept(p: &Param) -> Result<Kept> {
    let Some(bounds) = p.bounds() else { return Ok(Kept::Plain(p.ty.clone())) };
    if let Some(args) = fn_sugar(p) {
        let is_fn = bounds
            .iter()
            .any(|b| matches!(b, TypeParamBound::Trait(t) if t.path.segments.last().unwrap().ident == "Fn"));
        if !is_fn {
            return Err(Error::new(
                p.ty.span(),
                "kompoz: a restartable composable runs its closures again: take `impl Fn…` (or make it #[kompozable(inline)])",
            ));
        }
        let (inputs, output) = (&args.inputs, &args.output);
        return Ok(Kept::Closure(parse_quote!(::std::rc::Rc<dyn Fn(#inputs) #output>), args.clone()));
    }
    for b in bounds {
        if let TypeParamBound::Trait(t) = b {
            let seg = t.path.segments.last().unwrap();
            if seg.ident == "Into"
                && let PathArguments::AngleBracketed(a) = &seg.arguments
                && let Some(GenericArgument::Type(ty)) = a.args.first()
            {
                return Ok(Kept::Into(ty.clone()));
            }
        }
    }
    Err(Error::new(
        p.ty.span(),
        "kompoz: a restartable composable keeps its arguments: take a type, `impl Into<T>` or `impl Fn…` (or make it #[kompozable(inline)])",
    ))
}

/// The body of a restartable composable: its arguments kept (as `kept` says), the body run in a scope of its own.
fn restartable_body(rt: &Ts, params: &[Param], body: &Block) -> Result<Ts> {
    let mut converts = Vec::new();
    let mut types = Vec::new();
    let mut rebinds = Vec::new();
    for p in params {
        let i = &p.ident;
        match kept(p)? {
            Kept::Plain(t) => types.push(t),
            Kept::Into(t) => {
                converts.push(quote!(let #i: #t = ::core::convert::Into::into(#i);));
                types.push(t);
            }
            Kept::Closure(t, args) => {
                converts.push(quote!(let #i: #t = ::std::rc::Rc::new(#i);));
                let names: Vec<Ident> = (0..args.inputs.len()).map(|k| format_ident!("__kz_a{k}")).collect();
                let tys = args.inputs.iter().map(|a| &a.ty);
                let out = &args.output;
                rebinds.push(quote! {
                    let #i = {
                        let __kz_f = #i;
                        move |#(#names: #tys),*| #out { __kz_f(#(#names),*) }
                    };
                });
                types.push(t);
            }
        }
    }
    let names: Vec<Ts> = params
        .iter()
        .map(|p| {
            let (m, i) = (&p.mutability, &p.ident);
            quote!(#m #i)
        })
        .collect();
    let idents = params.iter().map(|p| &p.ident);
    let eqs = types.iter().enumerate().map(|(k, t)| {
        let k = Index::from(k);
        quote!(#rt::__private::eq_by({ use #rt::__private::{EqViaPartialEq as _, EqFallback as _}; (&&#rt::__private::EqProbe::<#t>::new()).__kz_eq() }, &a.#k, &b.#k))
    });
    let somes = types.iter().map(|t| quote!({ use #rt::__private::{EqViaPartialEq as _, EqFallback as _}; (&&#rt::__private::EqProbe::<#t>::new()).__kz_eq() }.is_some()));
    let stmts = &body.stmts;
    Ok(quote! {
        #(#converts)*
        let __kz_args: (#(#types,)*) = (#(#idents,)*);
        let __kz_equals: ::core::option::Option<fn(&(#(#types,)*), &(#(#types,)*)) -> bool> =
            if true #(&& #somes)* {
                ::core::option::Option::Some(|a: &(#(#types,)*), b: &(#(#types,)*)| true #(&& #eqs)*)
            } else {
                ::core::option::Option::None
            };
        #rt::__private::restartable(__kz_args, __kz_equals, move |__kz_args: &(#(#types,)*)| {
            #[allow(unused_variables, unused_mut)]
            let (#(#names,)*) = ::core::clone::Clone::clone(__kz_args);
            #(#rebinds)*
            #(#stmts)*
        });
    })
}

/// `#[kompozable]` (`composable` is `Some(inline)`) or `#[named]` (`None`) on a function.
fn expand_fn(mut f: ItemFn, composable: Option<bool>) -> Result<Ts> {
    let rt = runtime();
    let mut params = take_params(&mut f.sig)?;
    let mut t = Transformer::new(rt.clone(), param_names(&f.sig));
    if composable.is_some() {
        t.visit_block_mut(&mut f.block);
    }

    let vis = f.vis.clone();
    let name = f.sig.ident.clone();
    let prefix = format_ident!("__kz_{}_", name);
    let marker = format_ident!("__kz_{}_Marker", name);
    let new_fn = format_ident!("__kz_{}_new", name);
    let imp = format_ident!("__kz_impl_{}", name);
    let output = f.sig.output.clone();
    let returns_unit = matches!(output, ReturnType::Default);

    // The function itself, under another name, its body wrapped as a composable's.
    // The docs go on what callers see; the function itself is hidden.
    let docs: Vec<Attribute> = f.attrs.iter().filter(|a| a.path().is_ident("doc")).cloned().collect();
    let mut imp_fn = f.clone();
    imp_fn.sig.ident = imp.clone();
    imp_fn.attrs.retain(|a| !a.path().is_ident("doc"));
    if let Some(inline) = composable {
        if !inline && returns_unit {
            for (arg, p) in imp_fn.sig.inputs.iter_mut().zip(&params) {
                // Closures it keeps are kept for good.
                if fn_sugar(p).is_some()
                    && let FnArg::Typed(pt) = arg
                    && let Type::ImplTrait(it) = &mut *pt.ty
                    && !it.bounds.iter().any(|b| matches!(b, TypeParamBound::Lifetime(l) if l.ident == "static"))
                {
                    it.bounds.push(parse_quote!('static));
                }
            }
            for p in params.iter_mut() {
                if let Type::ImplTrait(it) = &mut p.ty
                    && fn_sugar_of(it)
                    && !it.bounds.iter().any(|b| matches!(b, TypeParamBound::Lifetime(l) if l.ident == "static"))
                {
                    it.bounds.push(parse_quote!('static));
                }
            }
            let body = restartable_body(&rt, &params, &f.block)?;
            imp_fn.block = parse_quote!({ #body });
        } else {
            let block = &f.block;
            imp_fn.block = parse_quote!({ #rt::__private::group(|| #block) });
        }
    }

    let original: Vec<(Ident, Type, bool)> =
        params.iter().map(|p| (p.ident.clone(), p.ty.clone(), p.default.is_some())).collect();
    let gens = generics(&rt, &f.sig.generics, None, &mut params)?;
    let args: Vec<Ident> = (0..params.len()).map(|i| format_ident!("__a{i}")).collect();
    let built = builder(Spec {
        rt: &rt,
        params: &params,
        generics: &gens,
        where_clause: f.sig.generics.where_clause.as_ref(),
        output: &output,
        call: quote!(#imp(#(#args),*)),
        recv: None,
        prefix: &prefix,
        vis: &vis,
    })?;
    let (items, new_type) = (built.items, built.new_type);

    // A PascalCase function is a constant of a marker type, which `Probe` turns into the builder; a lowercase one
    // stays a function taking the parameters without a default, and a struct of its name hands out the builder.
    // Taking nothing it must be given (and no generics): content a composition can be given.
    let content = (composable.is_some()
        && returns_unit
        && f.sig.generics.params.is_empty()
        && params.iter().all(|p| p.default.is_some()))
    .then(|| {
        quote! {
            impl #rt::Content for #marker {
                fn compose(&self) {
                    #new_fn().__call()
                }
            }
        }
    });
    let entry = if is_pascal(&name.to_string()) {
        quote! {
            #[doc(hidden)]
            #[allow(non_camel_case_types)]
            #[derive(Clone, Copy)]
            #vis struct #marker;
            #(#docs)*
            #[allow(non_upper_case_globals)]
            #vis const #name: #marker = #marker;
            impl #rt::__private::Kompozable for #marker {
                type Builder = #new_type;
                fn __kz_new(self) -> Self::Builder { #new_fn() }
            }
            #content
        }
    } else {
        let req = original.iter().filter(|o| !o.2).map(|(i, t, _)| quote!(#i: #t));
        let all = params.iter().map(|p| match &p.default {
            Some(d) => quote!(#d),
            None => {
                let i = &p.ident;
                quote!(#i)
            }
        });
        let (gi, _, wc) = f.sig.generics.split_for_impl();
        let attrs = &docs;
        quote! {
            #[allow(non_camel_case_types)]
            #[doc(hidden)]
            #vis struct #name {}
            impl #name {
                pub fn __kz_builder() -> #new_type { #new_fn() }
            }
            #(#attrs)*
            #[track_caller]
            #vis fn #name #gi(#(#req),*) #output #wc { #imp(#(#all),*) }
        }
    };
    Ok(quote! {
        #[doc(hidden)]
        #[allow(non_snake_case, clippy::too_many_arguments)]
        #[track_caller]
        #imp_fn

        #entry

        #items
    })
}

fn fn_sugar_of(it: &TypeImplTrait) -> bool {
    it.bounds.iter().any(|b| {
        matches!(b, TypeParamBound::Trait(t) if ["Fn", "FnMut", "FnOnce"].iter().any(|n| t.path.segments.last().unwrap().ident == n))
    })
}

/// `#[named]` on an impl block: each method with a defaulted parameter gets a builder (`__kz_name`) and keeps its
/// name for the parameters without a default. With `#[named(trait Name)]` the methods (and the builders' entry
/// points) go into a trait instead, implemented for the type: an extension of a type defined elsewhere.
fn expand_impl(mut imp: ItemImpl, ext: Option<ExtensionTrait>) -> Result<Ts> {
    let rt = runtime();
    let self_ty = (*imp.self_ty).clone();
    let ty_name = match &self_ty {
        Type::Path(p) => p.path.segments.last().unwrap().ident.clone(),
        _ => return Err(Error::new(self_ty.span(), "kompoz: #[named] impl: a plain type")),
    };
    if let Some((path, _)) = &imp.trait_ {
        return Err(Error::new(path.span(), "kompoz: #[named] goes on an inherent impl (`impl Type { … }`)"));
    }
    // The builders' items are named by the trait as well: two crates may extend a type with methods of one name.
    let item_prefix = match &ext {
        Some(e) => format!("{}_{}", e.name, ty_name),
        None => ty_name.to_string(),
    };
    let (impl_gi, impl_tg, impl_wc) = imp.generics.split_for_impl();
    let impl_gi = quote!(#impl_gi);
    let impl_tg = quote!(#impl_tg);
    let impl_wc = quote!(#impl_wc);
    let mut kz_fns = Vec::new();
    let mut modules = Vec::new();
    let mut items = Vec::new();
    // The trait's declarations (with `#[named(trait …)]`).
    let mut decls: Vec<Ts> = Vec::new();
    for item in std::mem::take(&mut imp.items) {
        let ImplItem::Fn(mut m) = item else {
            if ext.is_some() {
                return Err(Error::new(item.span(), "kompoz: a #[named(trait …)] impl holds methods only"));
            }
            items.push(item);
            continue;
        };
        let mut params = take_params(&mut m.sig)?;
        if params.iter().all(|p| p.default.is_none()) {
            if ext.is_some() {
                m.vis = Visibility::Inherited;
                let docs = m.attrs.iter().filter(|a| a.path().is_ident("doc"));
                let sig = declaration(&m.sig);
                decls.push(quote!(#(#docs)* #sig;));
            }
            items.push(ImplItem::Fn(m));
            continue;
        }
        let recv = match m.sig.inputs.first() {
            Some(FnArg::Receiver(r)) => match &r.kind {
                ReceiverKind::Value => Receiver::Value(self_ty.clone()),
                ReceiverKind::Reference(_, None, mutability) => Receiver::Ref(self_ty.clone(), mutability.is_some()),
                _ => return Err(Error::new(r.span(), "kompoz: #[named] methods take `self`, `&self` or `&mut self`")),
            },
            _ => return Err(Error::new(m.sig.span(), "kompoz: #[named] in an impl is for methods (with `self`)")),
        };
        let name = m.sig.ident.clone();
        let imp_name = format_ident!("__kz_impl_{}", name);
        let kz_name = format_ident!("__kz_{}", name);
        let prefix = format_ident!("__kz_{}_{}_", item_prefix, name);
        let new_fn = format_ident!("__kz_{}_{}_new", item_prefix, name);
        let output = m.sig.output.clone();
        // A trait's methods have no visibility of their own: its builders are as visible as it is.
        let vis = match &ext {
            Some(e) => e.vis.clone(),
            None => m.vis.clone(),
        };
        let original: Vec<(Ident, Type, bool)> =
            params.iter().map(|p| (p.ident.clone(), p.ty.clone(), p.default.is_some())).collect();
        let mut gens = generics(&rt, &m.sig.generics, Some(&imp.generics), &mut params)?;
        let mut wc: WhereClause = imp.generics.where_clause.clone().unwrap_or_else(|| parse_quote!(where));
        if let Some(w) = &m.sig.generics.where_clause {
            wc.predicates.extend(w.predicates.iter().cloned());
        }
        let args: Vec<Ident> = (0..params.len()).map(|i| format_ident!("__a{i}")).collect();
        // (through the trait by name: another trait in scope may have a method of the same name)
        let call = match &ext {
            Some(e) => {
                let t = &e.name;
                quote!(<#self_ty as #t #impl_tg>::#imp_name(__self, #(#args),*))
            }
            None => quote!(__self.#imp_name(#(#args),*)),
        };
        let built = builder(Spec {
            rt: &rt,
            params: &params,
            generics: &gens,
            where_clause: Some(&wc),
            output: &output,
            call,
            recv: Some(&recv),
            prefix: &prefix,
            vis: &vis,
        })?;
        gens.clear();
        let (b, new_type) = (built.items, built.new_type);
        modules.push(b);
        let receiver = &m.sig.inputs[0];
        let decl_sig = declaration(&m.sig);
        let decl_receiver = &decl_sig.inputs[0];
        if ext.is_some() {
            decls.push(quote! {
                #[doc(hidden)]
                fn #kz_name(#decl_receiver) -> #new_type;
            });
            items.push(parse_quote! {
                fn #kz_name(#receiver) -> #new_type { #new_fn(self) }
            });
        } else {
            kz_fns.push(quote! {
                #[doc(hidden)]
                pub fn #kz_name(#receiver) -> #new_type { #new_fn(self) }
            });
        }

        // The plain method: the parameters without a default, in order.
        let req: Vec<Ts> = original.iter().filter(|o| !o.2).map(|(i, t, _)| quote!(#i: #t)).collect();
        let defaults = params.iter().filter_map(|p| {
            let i = &p.ident;
            p.default.as_ref().map(|d| {
                let mut d = d.clone();
                builder::SelfToRecv.visit_expr_mut(&mut d);
                quote!(let #i = #d;)
            })
        });
        let all = params.iter().map(|p| &p.ident);
        let (mgi, _, mwc) = m.sig.generics.split_for_impl();
        let attrs = m.attrs.clone();
        let method_vis = if ext.is_some() { quote!() } else { quote!(#vis) };
        let inner_call = match &ext {
            Some(e) => {
                let t = &e.name;
                quote!(<Self as #t #impl_tg>::#imp_name(__self, #(#all),*))
            }
            None => quote!(__self.#imp_name(#(#all),*)),
        };
        items.push(parse_quote! {
            #(#attrs)*
            #[track_caller]
            #method_vis fn #name #mgi(#receiver, #(#req),*) #output #mwc {
                let __self = self;
                #(#defaults)*
                #inner_call
            }
        });
        if ext.is_some() {
            let docs = attrs.iter().filter(|a| a.path().is_ident("doc"));
            decls.push(quote!(#(#docs)* fn #name #mgi(#decl_receiver, #(#req),*) #output #mwc;));
        }
        m.sig.ident = imp_name;
        m.attrs.retain(|a| !a.path().is_ident("doc"));
        m.attrs.push(parse_quote!(#[doc(hidden)]));
        if ext.is_some() {
            m.vis = Visibility::Inherited;
            let sig = declaration(&m.sig);
            decls.push(quote!(#[doc(hidden)] #sig;));
        } else {
            m.vis = parse_quote!(pub);
        }
        items.push(ImplItem::Fn(m));
    }
    imp.items = items;
    match ext {
        Some(ExtensionTrait { vis, name }) => {
            let docs = imp.attrs.iter().filter(|a| a.path().is_ident("doc"));
            let others = imp.attrs.iter().filter(|a| !a.path().is_ident("doc"));
            let items = &imp.items;
            Ok(quote! {
                #(#docs)*
                #vis trait #name #impl_gi #impl_wc { #(#decls)* }
                #(#others)*
                impl #impl_gi #name #impl_tg for #self_ty #impl_wc { #(#items)* }
                #(#modules)*
            })
        }
        None => Ok(quote! {
            #imp
            impl #impl_gi #self_ty #impl_wc { #(#kz_fns)* }
            #(#modules)*
        }),
    }
}

/// A method's signature as a trait declares it: its parameters plain names (no `mut`, which only a body has).
fn declaration(sig: &Signature) -> Signature {
    let mut sig = sig.clone();
    for arg in sig.inputs.iter_mut() {
        if let FnArg::Typed(pt) = arg
            && let Pat::Ident(pi) = &mut *pt.pat
        {
            pi.mutability = None;
        }
        if let FnArg::Receiver(r) = arg
            && matches!(r.kind, ReceiverKind::Value)
        {
            r.mutability = None;
        }
    }
    sig
}
