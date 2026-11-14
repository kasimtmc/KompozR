//! Stage 0 prototype of the Rust API's call syntax (form A): Kotlin-like calls in plain Rust.
//!
//! `#[kompozable]` / `#[named]` on a function with `#[default(expr)]` parameters generate, next to it, a typed
//! builder that fills the omitted parameters with their defaults; a PascalCase function becomes a constant of a
//! marker type (`Text`), a lowercase one gets a braced struct of the same name (`top_app_bar_colors`), so that
//! both still read as calls. In a `#[kompozable]` body, calls are rewritten into that builder:
//!
//! ```text
//! Column(modifier = m, || { … })   →   { (&&Probe(Column)).__kz_builder() }.modifier(m).__last(move || { … }).__call()
//! ```
//!
//! - every call of a PascalCase path goes through `Probe`, which picks the builder for a marker and falls back to
//!   a positional call for anything else (`Some(x)`, a tuple struct, an enum variant);
//! - a lowercase path call or a method call is rewritten only when an argument is named (`name = value`);
//! - positional arguments fill the parameters in order, except a last unnamed closure, which goes to the last
//!   parameter (Kotlin's trailing lambda);
//! - closures passed to rewritten calls capture by value (`move`): they can run after the function returned.
//!
//! `#[kompozable(plain)]` (form B) only adds the `move`s.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Ts;
use quote::{format_ident, quote, quote_spanned};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit_mut::{self, VisitMut};
use syn::*;

fn kz() -> Ts {
    quote!(::kompoz_syntax_proto::kz)
}

#[proc_macro_attribute]
pub fn kompozable(attr: TokenStream, item: TokenStream) -> TokenStream {
    let plain = !attr.is_empty();
    let f = parse_macro_input!(item as ItemFn);
    let out = if plain { Ok(expand_plain(f)) } else { expand_fn(f) };
    out.unwrap_or_else(|e| e.to_compile_error()).into()
}

#[proc_macro_attribute]
pub fn named(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let out = match parse_macro_input!(item as Item) {
        Item::Fn(f) => expand_fn(f),
        Item::Impl(i) => expand_impl(i),
        other => Err(Error::new(other.span(), "#[named] goes on a function or an impl block")),
    };
    out.unwrap_or_else(|e| e.to_compile_error()).into()
}

// ---------------------------------------------------------------------------------------------------------------
// Declarations

struct Param {
    ident: Ident,
    ty: Type,
    default: Option<Expr>,
}

impl Param {
    fn bounds(&self) -> Option<&Punctuated<TypeParamBound, Token![+]>> {
        match &self.ty {
            Type::ImplTrait(t) => Some(&t.bounds),
            _ => None,
        }
    }
}

/// Takes the `#[default(…)]` attributes off the parameters and returns the parameters.
fn take_params(sig: &mut Signature) -> Result<Vec<Param>> {
    let mut params = Vec::new();
    for arg in sig.inputs.iter_mut() {
        let FnArg::Typed(pt) = arg else { continue };
        let Pat::Ident(pi) = &*pt.pat else {
            return Err(Error::new(pt.pat.span(), "a named parameter is a plain identifier"));
        };
        let mut default = None;
        let mut kept = Vec::new();
        for a in pt.attrs.drain(..) {
            if a.path().is_ident("default") {
                default = Some(a.parse_args::<Expr>()?);
            } else {
                kept.push(a);
            }
        }
        pt.attrs = kept;
        params.push(Param { ident: pi.ident.clone(), ty: (*pt.ty).clone(), default });
    }
    Ok(params)
}

fn is_pascal(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
}

/// `fn(A, B) -> R` for an `impl Fn(A, B) -> R` parameter's default.
fn fn_pointer(p: &Param) -> Result<Type> {
    for b in p.bounds().into_iter().flatten() {
        if let TypeParamBound::Trait(t) = b {
            let seg = t.path.segments.last().unwrap();
            if ["Fn", "FnMut", "FnOnce"].iter().any(|n| seg.ident == n)
                && let PathArguments::Parenthesized(args) = &seg.arguments
            {
                let inputs = &args.inputs;
                let output = &args.output;
                return Ok(parse_quote!(fn(#inputs) #output));
            }
        }
    }
    Err(Error::new(p.ty.span(), "a defaulted `impl Trait` parameter must be an `impl Fn…`"))
}

/// Replaces `self` in a default with `__self`, the receiver the builder holds.
struct SelfToRecv;
impl VisitMut for SelfToRecv {
    fn visit_ident_mut(&mut self, i: &mut Ident) {
        if i == "self" {
            *i = Ident::new("__self", i.span());
        }
    }
}

/// The builder of a function or method, as the items of its module: `Builder<P0, …>`, a setter per parameter
/// (by name, by position `__argN`, and `__last` for the last), `__call`, and `__kz_new`. `call` is what
/// `__call` evaluates with the arguments bound to `__a0, __a1, …`; `recv`, a method's receiver type.
/// Also returns the type `__kz_new` makes, with its names qualified by `module`.
fn builder(params: &[Param], output: &ReturnType, call: Ts, recv: Option<&Type>, module: &Ident) -> Result<(Ts, Ts)> {
    let kz = kz();
    let n = params.len();
    let ps: Vec<Ident> = (0..n).map(|i| format_ident!("P{i}")).collect();
    let fs: Vec<Ident> = (0..n).map(|i| format_ident!("p{i}")).collect();

    let mut missing = Vec::new();
    let mut initial = Vec::new();
    let mut initial_outside = Vec::new();
    for p in params {
        if p.default.is_some() {
            initial.push(quote!(#kz::Unset));
            initial_outside.push(quote!(#kz::Unset));
        } else {
            let m = format_ident!("Missing_{}", p.ident);
            missing.push(quote!(pub struct #m;));
            initial.push(quote!(#m));
            initial_outside.push(quote!(#module::#m));
        }
    }

    let mut setters = Vec::new();
    for (i, p) in params.iter().enumerate() {
        let name = &p.ident;
        let arg = format_ident!("__arg{i}");
        let ty = &p.ty;
        // A concrete parameter's setter takes exactly its type (so `0.6` is an f32 where an f32 is wanted); an
        // `impl Trait` one is generic with the same bounds (so a closure's parameter types are inferred).
        let (generic, value) = match p.bounds() {
            Some(b) => (quote!(<__V: #b>), quote!(__V)),
            None => (quote!(), quote!(#ty)),
        };
        let out: Vec<Ts> =
            ps.iter().enumerate().map(|(j, pj)| if j == i { quote!(#kz::Set<#value>) } else { quote!(#pj) }).collect();
        let fields =
            fs.iter().enumerate().map(|(j, f)| if j == i { quote!(#f: #kz::Set(v)) } else { quote!(#f: self.#f) });
        let recv_move = recv.map(|_| quote!(recv: self.recv,));
        setters.push(quote! {
            pub fn #name #generic(self, v: #value) -> Builder<#(#out),*> {
                Builder { #recv_move #(#fields),* }
            }
            pub fn #arg #generic(self, v: #value) -> Builder<#(#out),*> {
                self.#name(v)
            }
        });
        if i == n - 1 {
            setters.push(quote! {
                pub fn __last #generic(self, v: #value) -> Builder<#(#out),*> {
                    self.#name(v)
                }
            });
        }
    }

    let mut bounds = Vec::new();
    let mut lets = Vec::new();
    for (i, p) in params.iter().enumerate() {
        let (pi, fi, ty) = (&ps[i], &fs[i], &p.ty);
        let a = format_ident!("__a{i}");
        let default = p.default.clone().map(|mut d| {
            SelfToRecv.visit_expr_mut(&mut d);
            d
        });
        match (p.bounds(), default) {
            (None, None) => {
                bounds.push(quote!(#pi: #kz::Required<Value = #ty>));
                lets.push(quote!(let #a = #kz::Required::take(self.#fi);));
            }
            (Some(b), None) => {
                bounds.push(quote!(#pi: #kz::Required, <#pi as #kz::Required>::Value: #b));
                lets.push(quote!(let #a = #kz::Required::take(self.#fi);));
            }
            (None, Some(d)) => {
                bounds.push(quote!(#pi: #kz::ArgOr<#ty>));
                lets.push(quote!(let #a = #kz::ArgOr::<#ty>::or(self.#fi, || #d);));
            }
            (Some(b), Some(d)) => {
                let fp = fn_pointer(p)?;
                bounds.push(quote!(#pi: #kz::ImplArg<#fp>, <#pi as #kz::ImplArg<#fp>>::Out: #b));
                lets.push(quote!(let #a = #kz::ImplArg::<#fp>::pick(self.#fi, { let d: #fp = #d; d });));
            }
        }
    }

    let recv_field = recv.map(|t| quote!(recv: #t,));
    let recv_let = recv.map(|_| quote!(let __self = self.recv;));
    let new_param = recv.map(|t| quote!(recv: #t));
    let new_init = recv.map(|_| quote!(recv,));
    let items = quote! {
        #(#missing)*
        pub struct Builder<#(#ps),*> { #recv_field #(#fs: #ps),* }
        pub fn __kz_new(#new_param) -> Builder<#(#initial),*> {
            Builder { #new_init #(#fs: #initial),* }
        }
        impl<#(#ps),*> Builder<#(#ps),*> {
            #(#setters)*
        }
        impl<#(#ps),*> Builder<#(#ps),*> where #(#bounds),* {
            #[track_caller]
            pub fn __call(self) #output {
                #recv_let
                #(#lets)*
                #call
            }
        }
    };
    Ok((items, quote!(#module::Builder<#(#initial_outside),*>)))
}

fn module_attrs() -> Ts {
    quote!(#[doc(hidden)] #[allow(non_snake_case, non_camel_case_types, unused_imports)])
}

fn expand_fn(mut f: ItemFn) -> Result<Ts> {
    let kz = kz();
    let params = take_params(&mut f.sig)?;
    Rewriter.visit_block_mut(&mut f.block);

    let vis = f.vis.clone();
    let name = f.sig.ident.clone();
    let module = format_ident!("__kz_{}", name);
    let imp = format_ident!("__kz_impl_{}", name);
    let output = f.sig.output.clone();
    let args: Vec<Ident> = (0..params.len()).map(|i| format_ident!("__a{i}")).collect();
    let (items, new_ty) = builder(&params, &output, quote!(super::#imp(#(#args),*)), None, &module)?;

    let mut imp_fn = f.clone();
    imp_fn.sig.ident = imp.clone();
    imp_fn.vis = parse_quote!(pub);

    // A PascalCase function is a constant of a marker type, which `Probe` turns into the builder; a lowercase one
    // stays a function taking the parameters without a default, and a struct of its name hands out the builder.
    let entry = if is_pascal(&name.to_string()) {
        quote! {
            #[allow(non_upper_case_globals)]
            #vis const #name: #module::Marker = #module::Marker;
            impl #kz::Kompozable for #module::Marker {
                type Builder = #new_ty;
                fn __kz_new(self) -> Self::Builder { #module::__kz_new() }
            }
        }
    } else {
        let req = params.iter().filter(|p| p.default.is_none()).map(|p| {
            let (i, t) = (&p.ident, &p.ty);
            quote!(#i: #t)
        });
        let all = params.iter().map(|p| match &p.default {
            Some(d) => quote!(#d),
            None => {
                let i = &p.ident;
                quote!(#i)
            }
        });
        quote! {
            #[allow(non_camel_case_types)]
            #[doc(hidden)]
            #vis struct #name {}
            impl #name {
                pub fn __kz_builder() -> #new_ty { #module::__kz_new() }
            }
            #[track_caller]
            #vis fn #name(#(#req),*) #output { #imp(#(#all),*) }
        }
    };
    let attrs = module_attrs();
    Ok(quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        #[track_caller]
        #imp_fn

        #entry

        #attrs
        #vis mod #module {
            use super::*;
            #[derive(Clone, Copy)]
            pub struct Marker;
            #items
        }
    })
}

fn expand_impl(mut imp: ItemImpl) -> Result<Ts> {
    let self_ty = (*imp.self_ty).clone();
    let ty_name = match &self_ty {
        Type::Path(p) => p.path.segments.last().unwrap().ident.clone(),
        _ => return Err(Error::new(self_ty.span(), "#[named] impl: a plain type")),
    };
    let mut kz_fns = Vec::new();
    let mut modules = Vec::new();
    let mut items = Vec::new();
    for item in std::mem::take(&mut imp.items) {
        let ImplItem::Fn(mut m) = item else {
            items.push(item);
            continue;
        };
        let params = take_params(&mut m.sig)?;
        Rewriter.visit_block_mut(&mut m.block);
        if params.iter().all(|p| p.default.is_none()) {
            items.push(ImplItem::Fn(m));
            continue;
        }
        if !matches!(m.sig.inputs.first(), Some(FnArg::Receiver(r)) if matches!(r.kind, ReceiverKind::Value)) {
            return Err(Error::new(m.sig.span(), "#[named] methods take `self` by value"));
        }
        let name = m.sig.ident.clone();
        let imp_name = format_ident!("__kz_impl_{}", name);
        let kz_name = format_ident!("__kz_{}", name);
        let module = format_ident!("__kz_{}_{}", ty_name, name);
        let output = m.sig.output.clone();
        let vis = m.vis.clone();
        let args: Vec<Ident> = (0..params.len()).map(|i| format_ident!("__a{i}")).collect();
        let (b, new_ty) = builder(&params, &output, quote!(__self.#imp_name(#(#args),*)), Some(&self_ty), &module)?;
        let attrs = module_attrs();
        modules.push(quote!(#attrs pub mod #module { use super::*; #b }));
        kz_fns.push(quote! {
            #[doc(hidden)]
            pub fn #kz_name(self) -> #new_ty { #module::__kz_new(self) }
        });

        // The plain method: the parameters without a default, in order.
        let req = params.iter().filter(|p| p.default.is_none()).map(|p| {
            let (i, t) = (&p.ident, &p.ty);
            quote!(#i: #t)
        });
        let defaults = params.iter().filter_map(|p| {
            let i = &p.ident;
            p.default.as_ref().map(|d| {
                let mut d = d.clone();
                SelfToRecv.visit_expr_mut(&mut d);
                quote!(let #i = #d;)
            })
        });
        let all = params.iter().map(|p| &p.ident);
        items.push(parse_quote! {
            #[track_caller]
            #vis fn #name(self, #(#req),*) #output {
                let __self = self;
                #(#defaults)*
                __self.#imp_name(#(#all),*)
            }
        });
        m.sig.ident = imp_name;
        m.vis = parse_quote!(pub);
        m.attrs.push(parse_quote!(#[doc(hidden)]));
        items.push(ImplItem::Fn(m));
    }
    imp.items = items;
    Ok(quote! {
        #imp
        impl #self_ty { #(#kz_fns)* }
        #(#modules)*
    })
}

fn expand_plain(mut f: ItemFn) -> Ts {
    MoveClosures.visit_block_mut(&mut f.block);
    quote!(#[allow(non_snake_case)] #f)
}

// ---------------------------------------------------------------------------------------------------------------
// Call sites

/// `name = value`
fn named_arg(e: &Expr) -> Option<(&Ident, &Expr)> {
    if let Expr::Assign(a) = e
        && let Expr::Path(p) = &*a.left
        && let Some(i) = p.path.get_ident()
    {
        return Some((i, &a.right));
    }
    None
}

fn with_move(e: &Expr) -> Expr {
    let mut e = e.clone();
    if let Expr::Closure(c) = &mut e
        && c.capture.is_none()
    {
        c.capture = Some(Default::default());
    }
    e
}

/// `.name(v)` / `.__argN(v)` / `.__last(v)` for each argument.
fn chain(args: &Punctuated<Expr, Token![,]>) -> Vec<Ts> {
    let n = args.len();
    let mut out = Vec::new();
    let mut position = 0;
    for (k, a) in args.iter().enumerate() {
        if let Some((name, v)) = named_arg(a) {
            let v = with_move(v);
            out.push(quote_spanned!(a.span()=> .#name(#v)));
        } else if k == n - 1 && matches!(a, Expr::Closure(_)) {
            let v = with_move(a);
            out.push(quote_spanned!(a.span()=> .__last(#v)));
        } else {
            let m = format_ident!("__arg{position}", span = a.span());
            let v = with_move(a);
            out.push(quote_spanned!(a.span()=> .#m(#v)));
            position += 1;
        }
    }
    out
}

struct Rewriter;

impl VisitMut for Rewriter {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        visit_mut::visit_expr_mut(self, e);
        let kz = kz();
        match e {
            Expr::Call(c) => {
                let Expr::Path(p) = &*c.func else { return };
                let last = p.path.segments.last().unwrap().ident.to_string();
                let has_named = c.args.iter().any(|a| named_arg(a).is_some());
                let span = c.span();
                let links = chain(&c.args);
                let path = &p.path;
                if is_pascal(&last) {
                    *e = parse_quote_spanned!(span=> {
                        #[allow(unused_imports)]
                        use #kz::{ViaBuilder as _, ViaFn as _};
                        (&&#kz::Probe(#path)).__kz_builder()
                    } #(#links)* .__call());
                } else if has_named {
                    *e = parse_quote_spanned!(span=> #path::__kz_builder() #(#links)* .__call());
                }
            }
            Expr::MethodCall(m) => {
                if !m.args.iter().any(|a| named_arg(a).is_some()) {
                    return;
                }
                let span = m.span();
                let links = chain(&m.args);
                let recv = &m.receiver;
                let kz_name = format_ident!("__kz_{}", m.method, span = m.method.span());
                *e = parse_quote_spanned!(span=> #recv.#kz_name() #(#links)* .__call());
            }
            Expr::Macro(m) if m.mac.path.is_ident("vec") => {
                if let Ok(mut items) = Punctuated::<Expr, Token![,]>::parse_terminated.parse2(m.mac.tokens.clone()) {
                    for i in items.iter_mut() {
                        self.visit_expr_mut(i);
                    }
                    m.mac.tokens = quote!(#items);
                }
            }
            _ => {}
        }
    }
}

struct MoveClosures;

impl VisitMut for MoveClosures {
    fn visit_expr_call_mut(&mut self, c: &mut ExprCall) {
        visit_mut::visit_expr_call_mut(self, c);
        for a in c.args.iter_mut() {
            *a = with_move(a);
        }
    }
}
