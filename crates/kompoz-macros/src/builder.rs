//! The builder of a function or method with named, defaulted parameters: what a rewritten call goes through.
//!
//! `Builder<G…, P…>` carries the function's generic parameters `G` (not known yet: `Infer`, or `'static` for a
//! lifetime) and one slot `P` per parameter (`Missing_x`, `Unset`, or `Set<V>`). Setters fill a slot, by name, by
//! position (`__argN`) or as the trailing closure (`__last`); a setter of a parameter whose type names a generic
//! parameter fixes it, so a closure parameter set after it (`items(list, |item| …)`) gets its types inferred.
//! `__call` checks every slot, fills the defaults and calls the function.
//!
//! Each setter has a twin for a memoized closure, `__kz_memo_<setter>(keys, closure)` (see `transform`): where the
//! parameter keeps its closure (`wants_id`), it turns the keys into the closure's identity (`memo_id`) and hands
//! it to the function with the arguments; elsewhere it drops them. A parameter of type `Option<T>` is set from
//! anything `Into<Option<T>>` (`role = Role::Button`, `role = None`), as Kotlin passes a `T` to a `T?`; and each
//! setter has a twin for an argument written `Some(…)`, `__kz_some_<setter>(v)`, taking an `Option<T>` itself, so
//! that what it holds coerces (`icon = Some(Rc::new(|| …))` for an `Option<Rc<dyn Fn()>>`).

use proc_macro2::{Span, TokenStream as Ts};
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::visit_mut::{self, VisitMut};
use syn::*;

pub(crate) struct Param {
    pub(crate) ident: Ident,
    pub(crate) mutability: Option<Token![mut]>,
    pub(crate) ty: Type,
    pub(crate) default: Option<Expr>,
    /// `#[memoized]`: its body gets it as a `Memoized` (with its identity as a memoized closure).
    pub(crate) memoized: Option<Span>,
}

impl Param {
    pub(crate) fn bounds(&self) -> Option<&Punctuated<TypeParamBound, Token![+]>> {
        match &self.ty {
            Type::ImplTrait(t) => Some(&t.bounds),
            _ => None,
        }
    }
}

/// Takes the `#[default(…)]` attributes off the parameters and returns the parameters.
pub(crate) fn take_params(sig: &mut Signature) -> Result<Vec<Param>> {
    let mut params = Vec::new();
    for arg in sig.inputs.iter_mut() {
        let FnArg::Typed(pt) = arg else { continue };
        let Pat::Ident(pi) = &*pt.pat else {
            return Err(Error::new(pt.pat.span(), "kompoz: a parameter here is a plain name (no pattern)"));
        };
        let mut default = None;
        let mut memoized = None;
        let mut kept = Vec::new();
        for a in pt.attrs.drain(..) {
            if a.path().is_ident("default") {
                default = Some(a.parse_args::<Expr>()?);
            } else if a.path().is_ident("memoized") {
                a.meta.require_path_only()?;
                memoized = Some(a.path().span());
            } else {
                kept.push(a);
            }
        }
        pt.attrs = kept;
        params.push(Param {
            ident: pi.ident.clone(),
            mutability: pi.mutability,
            ty: (*pt.ty).clone(),
            default,
            memoized,
        });
    }
    Ok(params)
}

/// `fn(A, B) -> R` for an `impl Fn(A, B) -> R` parameter's default.
pub(crate) fn fn_sugar(p: &Param) -> Option<&ParenthesizedGenericArguments> {
    for b in p.bounds().into_iter().flatten() {
        if let TypeParamBound::Trait(t) = b {
            let seg = t.path.segments.last().unwrap();
            if ["Fn", "FnMut", "FnOnce"].iter().any(|n| seg.ident == n)
                && let PathArguments::Parenthesized(args) = &seg.arguments
            {
                return Some(args);
            }
        }
    }
    None
}

/// The type of an `impl Trait` parameter's default: a fn pointer for an `impl Fn…` (its default a closure or a
/// function), else the unit struct its default names (`#[default(DefaultFlexBoxConfig)] config: impl FlexBoxConfig`:
/// Compose's companion object standing for the default).
/// A default reading earlier parameters (`capturing`) is a closure holding what it takes of them: a boxed `dyn Fn…`.
fn fn_pointer(p: &Param, capturing: bool) -> Result<Type> {
    match fn_sugar(p) {
        Some(args) if capturing => {
            let (inputs, output) = (&args.inputs, &args.output);
            let name = p
                .bounds()
                .into_iter()
                .flatten()
                .find_map(|b| match b {
                    TypeParamBound::Trait(t) => {
                        let seg = t.path.segments.last().unwrap();
                        ["Fn", "FnMut", "FnOnce"].iter().any(|n| seg.ident == n).then(|| seg.ident.clone())
                    }
                    _ => None,
                })
                .expect("an `impl Fn…`");
            Ok(parse_quote!(::std::boxed::Box<dyn #name(#inputs) #output>))
        }
        Some(args) => {
            let (inputs, output) = (&args.inputs, &args.output);
            Ok(parse_quote!(fn(#inputs) #output))
        }
        None => match &p.default {
            Some(Expr::Path(ExprPath { qself: None, path, .. })) => Ok(parse_quote!(#path)),
            _ => Err(Error::new(
                p.ty.span(),
                "kompoz: a defaulted `impl Trait` parameter must be an `impl Fn…`, or default to a unit struct (its path)",
            )),
        },
    }
}

/// The earlier parameters `params[i]`'s default names (Kotlin's `fun f(a: Int, b: Int = a * 2)`: a default sees
/// the parameters before it), by index: an identifier of the default's tokens (inside macros too) that is one of
/// theirs, not a field or method after `.` nor a path's segment.
pub(crate) fn earlier_in_default(params: &[Param], i: usize) -> Vec<usize> {
    let Some(d) = &params[i].default else { return Vec::new() };
    let mut names = std::collections::HashSet::new();
    fn scan(ts: Ts, names: &mut std::collections::HashSet<String>) {
        let mut prev: Option<proc_macro2::TokenTree> = None;
        let tokens: Vec<proc_macro2::TokenTree> = ts.into_iter().collect();
        for (k, t) in tokens.iter().enumerate() {
            match t {
                proc_macro2::TokenTree::Group(g) => scan(g.stream(), names),
                proc_macro2::TokenTree::Ident(id) => {
                    let after_dot_or_path = matches!(&prev, Some(proc_macro2::TokenTree::Punct(p))
                        if p.as_char() == '.' || (p.as_char() == ':' && k >= 2
                            && matches!(&tokens[k - 2], proc_macro2::TokenTree::Punct(q) if q.as_char() == ':')));
                    // (a path's segment, a named argument's name — `f(x = 1)` — or a binding's — `let x = …`)
                    let before_path = matches!(tokens.get(k + 1), Some(proc_macro2::TokenTree::Punct(p))
                        if p.as_char() == ':' && p.spacing() == proc_macro2::Spacing::Joint
                            || p.as_char() == '=' && p.spacing() == proc_macro2::Spacing::Alone);
                    if !after_dot_or_path && !before_path {
                        names.insert(id.to_string());
                    }
                }
                // (a format string's captured names: `format!("{a}/{b}")`)
                proc_macro2::TokenTree::Literal(l) => {
                    let text = l.to_string();
                    if text.ends_with('"') {
                        let mut rest = text.as_str();
                        while let Some(at) = rest.find('{') {
                            rest = &rest[at + 1..];
                            if rest.starts_with('{') {
                                rest = &rest[1..];
                                continue;
                            }
                            let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                            if !name.is_empty() && !name.starts_with(|c: char| c.is_ascii_digit()) {
                                names.insert(name);
                            }
                        }
                    }
                }
                _ => {}
            }
            prev = Some(t.clone());
        }
    }
    scan(quote!(#d), &mut names);
    (0..i).filter(|&k| names.contains(&params[k].ident.to_string())).collect()
}

/// The `let`s giving a default the earlier parameters it names by their names, each lent (`let a = &__a0;`): a
/// default reads them, as Kotlin's does, without taking them from the call.
pub(crate) fn earlier_lets(params: &[Param], i: usize, slot: impl Fn(usize) -> Ts) -> Ts {
    let lets = earlier_in_default(params, i).into_iter().map(|k| {
        let name = &params[k].ident;
        let s = slot(k);
        quote!(#[allow(unused_variables)] let #name = &#s;)
    });
    quote!(#(#lets)*)
}

/// Which defaults a plain call (the parameters without a default given) makes before the call, by the parameters'
/// names: those reading earlier parameters, and the earlier defaults they read.
pub(crate) fn hoisted_defaults(params: &[Param]) -> Vec<bool> {
    let mut hoisted = vec![false; params.len()];
    for i in (0..params.len()).rev() {
        let reads = earlier_in_default(params, i);
        if !reads.is_empty() {
            hoisted[i] = true;
        }
        if hoisted[i] {
            for k in reads {
                if params[k].default.is_some() {
                    hoisted[k] = true;
                }
            }
        }
    }
    hoisted
}

/// Replaces `self` in a default with `__self`, the receiver the builder holds.
pub(crate) struct SelfToRecv;
impl VisitMut for SelfToRecv {
    fn visit_ident_mut(&mut self, i: &mut Ident) {
        if i == "self" {
            *i = Ident::new("__self", i.span());
        }
    }
}

/// Elided lifetimes (`&str`, `'_`) in parameter types made named (`'__kz0`), so the builder can carry them.
struct NameLifetimes<'a> {
    fresh: &'a mut Vec<Lifetime>,
}
impl VisitMut for NameLifetimes<'_> {
    fn visit_type_reference_mut(&mut self, r: &mut TypeReference) {
        if r.lifetime.is_none() {
            let l = Lifetime::new(&format!("'__kz{}", self.fresh.len()), r.and_token.span);
            self.fresh.push(l.clone());
            r.lifetime = Some(l);
        }
        visit_mut::visit_type_reference_mut(self, r);
    }
    fn visit_lifetime_mut(&mut self, l: &mut Lifetime) {
        if l.ident == "_" {
            let fresh = Lifetime::new(&format!("'__kz{}", self.fresh.len()), l.span());
            self.fresh.push(fresh.clone());
            *l = fresh;
        }
    }
    fn visit_type_impl_trait_mut(&mut self, _: &mut TypeImplTrait) {
        // (an `impl Fn(&T)`'s lifetimes are its own: higher-ranked)
    }
    fn visit_parenthesized_generic_arguments_mut(&mut self, _: &mut ParenthesizedGenericArguments) {
        // (so are a `dyn Fn(&T)`'s, a shared closure's: `Option<Rc<dyn Fn(&S) -> SlotKey>>`)
    }
    fn visit_type_fn_ptr_mut(&mut self, _: &mut TypeFnPtr) {
        // (and a `fn(&T)`'s)
    }
}

/// Gives the output's elided lifetimes the one elision gives them (`__call` takes its builder by value, so it has
/// no input to elide them from): a method's receiver's, else the signature's only lifetime.
struct ElideTo(Lifetime);
impl VisitMut for ElideTo {
    fn visit_type_reference_mut(&mut self, r: &mut TypeReference) {
        if r.lifetime.is_none() {
            r.lifetime = Some(self.0.clone());
        }
        visit_mut::visit_type_reference_mut(self, r);
    }
    fn visit_lifetime_mut(&mut self, l: &mut Lifetime) {
        if l.ident == "_" {
            *l = self.0.clone();
        }
    }
    fn visit_type_fn_ptr_mut(&mut self, _: &mut TypeFnPtr) {
        // (a `fn(&T) -> &U`'s lifetimes are its own: higher-ranked)
    }
    fn visit_parenthesized_generic_arguments_mut(&mut self, _: &mut ParenthesizedGenericArguments) {
        // (and an `Fn(&T) -> &U`'s)
    }
}

/// `T` of an `Option<T>` (written `Option`, `core::option::Option` or `std::option::Option`) with no `impl Trait`
/// in it.
fn option_of(ty: &Type) -> Option<&Type> {
    let Type::Path(p) = ty else { return None };
    if p.qself.is_some() {
        return None;
    }
    let names: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    if !matches!(names.as_slice(), ["Option"] | ["core" | "std", "option", "Option"]) {
        return None;
    }
    let PathArguments::AngleBracketed(a) = &p.path.segments.last().unwrap().arguments else { return None };
    let (1, Some(GenericArgument::Type(t))) = (a.args.len(), a.args.first()) else { return None };
    struct HasImpl(bool);
    impl<'ast> Visit<'ast> for HasImpl {
        fn visit_type_impl_trait(&mut self, _: &'ast TypeImplTrait) {
            self.0 = true;
        }
    }
    let mut h = HasImpl(false);
    h.visit_type(t);
    (!h.0).then_some(t)
}

/// Whether `ty` names `name` (a type or lifetime parameter).
/// Whether an `impl` parameter's bounds name the type parameter `name` (`impl BackStackList<T>`).
fn bounds_mention(bounds: &Punctuated<TypeParamBound, Token![+]>, name: &str) -> bool {
    struct Finder<'a>(&'a str, bool);
    impl<'ast> Visit<'ast> for Finder<'_> {
        fn visit_ident(&mut self, i: &'ast Ident) {
            if i == self.0 {
                self.1 = true;
            }
        }
    }
    let mut f = Finder(name, false);
    for b in bounds {
        f.visit_type_param_bound(b);
    }
    f.1
}

fn mentions(ty: &Type, name: &str) -> bool {
    struct Finder<'a>(&'a str, bool);
    impl<'ast> Visit<'ast> for Finder<'_> {
        fn visit_ident(&mut self, i: &'ast Ident) {
            if i == self.0 {
                self.1 = true;
            }
        }
        fn visit_lifetime(&mut self, l: &'ast Lifetime) {
            if l.to_string() == self.0 {
                self.1 = true;
            }
            visit::visit_lifetime(self, l);
        }
        fn visit_type_impl_trait(&mut self, _: &'ast TypeImplTrait) {}
    }
    let mut f = Finder(name, false);
    f.visit_type(ty);
    f.1
}

/// Renames generic parameters in a type.
struct Rename<'a>(&'a HashMap<String, String>);
impl VisitMut for Rename<'_> {
    fn visit_ident_mut(&mut self, i: &mut Ident) {
        if let Some(n) = self.0.get(&i.to_string()) {
            *i = Ident::new(n, i.span());
        }
    }
    fn visit_lifetime_mut(&mut self, l: &mut Lifetime) {
        if let Some(n) = self.0.get(&l.to_string()) {
            *l = Lifetime::new(n, l.span());
        }
    }
}

/// A generic parameter the builder carries.
pub(crate) struct Generic {
    /// `T` or `'a`.
    pub(crate) name: Ts,
    pub(crate) lifetime: bool,
    /// What `__kz_new` starts it as: `Infer`, `'static`, or (an impl's) itself.
    pub(crate) initial: Ts,
    /// Whether a setter fixes it from its argument (the function's own, not an impl's).
    pub(crate) inferred: bool,
    /// Its bounds, for `__call`.
    pub(crate) bounds: Option<Ts>,
    /// Whether it may be unsized (`S: ?Sized`, an impl's for a trait object too): the builder declares it so.
    pub(crate) maybe_unsized: bool,
}

impl Generic {
    fn key(&self) -> String {
        self.name.to_string().replace(' ', "")
    }
}

/// The generic parameters of a signature (and of the impl it is in), the elided lifetimes of its parameters
/// named; const generics aren't supported.
pub(crate) fn generics(
    rt: &Ts,
    sig_generics: &Generics,
    impl_generics: Option<&Generics>,
    params: &mut [Param],
) -> Result<Vec<Generic>> {
    let mut out = Vec::new();
    // (`?Sized` written in the bounds or in a where clause of the impl or the signature)
    let maybe_unsized_in = |bounds: &Punctuated<TypeParamBound, Token![+]>| {
        bounds.iter().any(|b| {
            matches!(b, TypeParamBound::Trait(t) if t.maybe.is_some()
                && t.path.is_ident("Sized"))
        })
    };
    let where_unsized = |ident: &Ident| {
        impl_generics.into_iter().chain([sig_generics]).filter_map(|g| g.where_clause.as_ref()).any(|w| {
            w.predicates.iter().any(|p| {
                matches!(p, WherePredicate::Type(t) if matches!(&t.bounded_ty, Type::Path(tp) if tp.path.is_ident(ident))
                    && maybe_unsized_in(&t.bounds))
            })
        })
    };
    let mut push = |g: &GenericParam, inferred: bool| -> Result<()> {
        match g {
            GenericParam::Type(t) => {
                let (i, b) = (&t.ident, &t.bounds);
                out.push(Generic {
                    name: quote!(#i),
                    lifetime: false,
                    initial: if inferred { quote!(#rt::__private::Infer) } else { quote!(#i) },
                    inferred,
                    bounds: (!b.is_empty()).then(|| quote!(#i: #b)),
                    maybe_unsized: maybe_unsized_in(b) || where_unsized(i),
                });
            }
            GenericParam::Lifetime(l) => {
                let (i, b) = (&l.lifetime, &l.bounds);
                out.push(Generic {
                    name: quote!(#i),
                    lifetime: true,
                    initial: if inferred { quote!('static) } else { quote!(#i) },
                    inferred,
                    bounds: (!b.is_empty()).then(|| quote!(#i: #b)),
                    maybe_unsized: false,
                });
            }
            GenericParam::Const(c) => return Err(Error::new(c.span(), "kompoz: const generics aren't supported here")),
        }
        Ok(())
    };
    if let Some(ig) = impl_generics {
        for g in &ig.params {
            push(g, false)?;
        }
    }
    for g in &sig_generics.params {
        push(g, true)?;
    }
    let mut fresh = Vec::new();
    for p in params.iter_mut() {
        NameLifetimes { fresh: &mut fresh }.visit_type_mut(&mut p.ty);
    }
    for l in fresh {
        out.push(Generic {
            name: quote!(#l),
            lifetime: true,
            initial: quote!('static),
            inferred: true,
            bounds: None,
            maybe_unsized: false,
        });
    }
    Ok(out)
}

/// The receiver a method's builder holds.
pub(crate) enum Receiver {
    Value(Type),
    Ref(Type, bool),
    /// `&'static self` (a `static`'s method: a composition local's).
    Static(Type),
}

/// The items of a builder (named with its prefix, next to the function), the type its `new` makes, and the generic
/// parameters (`<T>`, or nothing) an entry point making it declares: those the builder starts with free.
pub(crate) struct Built {
    pub(crate) items: Ts,
    pub(crate) new_type: Ts,
    pub(crate) new_generics: Ts,
}

/// What a builder is made for. `call` is what `__call` evaluates with the arguments bound to `__a0, __a1, …` (and
/// the receiver to `__self`); `where_clause`, the function's (and impl's); `prefix` names its items.
pub(crate) struct Spec<'a> {
    pub(crate) rt: &'a Ts,
    pub(crate) params: &'a [Param],
    pub(crate) generics: &'a [Generic],
    pub(crate) where_clause: Option<&'a WhereClause>,
    pub(crate) output: &'a ReturnType,
    pub(crate) call: Ts,
    pub(crate) recv: Option<&'a Receiver>,
    pub(crate) prefix: &'a Ident,
    pub(crate) vis: &'a Visibility,
    /// The parameters that keep the identity of a memoized closure given to them (by index). If any does, the
    /// builder carries the identities, which `__call` binds to `__kz_ids: [Option<u64>; n]` for `call` to use.
    pub(crate) wants_id: &'a [bool],
    /// Whether the function's generic parameters no parameter without a default fixes may be the builder's from
    /// its start (`new` and the entry point generic over them, inferred where the call's value goes: `tween(300)`'s
    /// `T`); else they start as `Infer` (a composable's marker, whose builder type is an associated type).
    pub(crate) free_ok: bool,
}

/// The builder of a function or method.
pub(crate) fn builder(spec: Spec<'_>) -> Result<Built> {
    let Spec { rt, params, generics, where_clause, output, call, recv, prefix, vis, wants_id, free_ok } = spec;
    let pass_ids = wants_id.iter().any(|w| *w);
    let mut output = output.clone();
    let own_lifetimes: Vec<&Generic> = generics.iter().filter(|g| g.lifetime && g.inferred).collect();
    let elided = match (recv, own_lifetimes.as_slice()) {
        (Some(Receiver::Ref(..)), _) => Some(Lifetime::new("'__kzr", Span::call_site())),
        (Some(Receiver::Static(_)), _) => Some(Lifetime::new("'static", Span::call_site())),
        (_, [only]) => Some(syn::parse2(only.name.clone())?),
        _ => None,
    };
    if let (Some(l), ReturnType::Type(_, t)) = (elided, &mut output) {
        ElideTo(l).visit_type_mut(t);
    }
    let n = params.len();
    let builder_name = format_ident!("{}Builder", prefix);
    let new_name = format_ident!("{}new", prefix);
    let ps: Vec<Ident> = (0..n).map(|i| format_ident!("P{i}")).collect();
    let fs: Vec<Ident> = (0..n).map(|i| format_ident!("p{i}")).collect();
    let gnames: Vec<&Ts> = generics.iter().map(|g| &g.name).collect();
    let (lifetimes, types): (Vec<&Generic>, Vec<&Generic>) = generics.iter().partition(|g| g.lifetime);
    // What the generics are in a builder type: lifetimes first, then types, then slots.
    let order = |sub: &HashMap<String, Ts>| -> Vec<Ts> {
        lifetimes
            .iter()
            .chain(types.iter())
            .map(|g| sub.get(&g.key()).cloned().unwrap_or_else(|| g.name.clone()))
            .collect()
    };
    let lt_names: Vec<&Ts> = lifetimes.iter().map(|g| &g.name).collect();
    let ty_names: Vec<&Ts> = types.iter().map(|g| &g.name).collect();
    // (each type in a PhantomData of its own: a type parameter may be unsized)
    let phantom =
        quote!(::core::marker::PhantomData<fn() -> (#(&#lt_names (),)* #(::core::marker::PhantomData<#ty_names>,)*)>);

    let recv_ty = recv.map(|r| match r {
        Receiver::Value(t) => quote!(#t),
        Receiver::Ref(t, false) => quote!(&'__kzr #t),
        Receiver::Ref(t, true) => quote!(&'__kzr mut #t),
        Receiver::Static(t) => quote!(&'static #t),
    });
    let recv_lt = matches!(recv, Some(Receiver::Ref(..))).then(|| quote!('__kzr,));
    let recv_field = recv_ty.as_ref().map(|t| quote!(recv: #t,));
    let recv_move = recv.map(|_| quote!(recv: self.recv,));
    let ids_field = pass_ids.then(|| quote!(__kz_ids: [::core::option::Option<u64>; #n],));
    let ids_move = pass_ids.then(|| quote!(__kz_ids: self.__kz_ids,));
    // (a closure parameter left at its default has the default's identity, the same each call, as Kotlin's default
    // lambda is one instance; a setter resets it)
    let ids_init = pass_ids.then(|| {
        let ids = params.iter().zip(wants_id).enumerate().map(|(k, (p, w))| {
            // (one reading earlier parameters is made each call, as Kotlin's capturing lambda is: no identity)
            if *w && p.default.is_some() && earlier_in_default(params, k).is_empty() {
                quote!(#rt::__private::DEFAULT_CLOSURE_ID)
            } else {
                quote!(::core::option::Option::None)
            }
        });
        quote!(__kz_ids: [#(#ids),*],)
    });

    let mut missing = Vec::new();
    let mut initial = Vec::new();
    let mut initial_outside = Vec::new();
    for p in params {
        if p.default.is_some() {
            initial.push(quote!(#rt::__private::Unset));
            initial_outside.push(quote!(#rt::__private::Unset));
        } else {
            let m = format_ident!("{}Missing_{}", prefix, p.ident);
            missing.push(quote!(#[allow(non_camel_case_types)] #[doc(hidden)] #vis struct #m;));
            initial.push(quote!(#m));
            initial_outside.push(quote!(#m));
        }
    }
    // (the function's type generics are free from the builder's start: a setter naming one gives it its type in
    // place, as Rust infers it from the argument, and one no argument names is inferred where the call's value goes
    // — `tween(300)`'s `T`; a later parameter naming one, `animationSpec: Arc<dyn AnimationSpec<T>>` given a
    // `tween(…)`, takes it as it is rather than making it anew. A composable's marker can't: they are fixed there.)
    let free: Vec<&Generic> = if free_ok { types.iter().copied().filter(|g| g.inferred).collect() } else { Vec::new() };
    let free_keys: std::collections::HashSet<String> = free.iter().map(|g| g.key()).collect();
    let free_names: Vec<&Ts> = free.iter().map(|g| &g.name).collect();
    let initial_generics: HashMap<String, Ts> = generics
        .iter()
        .map(|g| (g.key(), if free_keys.contains(&g.key()) { g.name.clone() } else { g.initial.clone() }))
        .collect();
    let init_g = order(&initial_generics);

    let mut setters = Vec::new();
    for (i, p) in params.iter().enumerate() {
        let name = &p.ident;
        let ty = &p.ty;
        // The generics it fixes: the function's that its type names; for an `impl Fn…`, those only its output names
        // (`impl FnOnce() -> R`: the closure says what R is; `impl Fn(&T)`'s T comes from elsewhere, so the closure's
        // parameter types can be inferred) — but the free ones, which it takes as they are.
        let fixed: Vec<&Generic> = match fn_sugar(p) {
            Some(args) => {
                let out_ty: Option<Type> = match &args.output {
                    ReturnType::Type(_, t) => Some((**t).clone()),
                    ReturnType::Default => None,
                };
                let input_tys: Vec<&Type> = args.inputs.iter().map(|a| &a.ty).collect();
                generics
                    .iter()
                    .filter(|g| {
                        g.inferred
                            && out_ty.as_ref().is_some_and(|t| mentions(t, &g.key()))
                            && !input_tys.iter().any(|t| mentions(t, &g.key()))
                    })
                    .collect()
            }
            // (an `impl` of a trait of the generics, `impl BackStackList<T>`: the argument's type tells them, by its one
            // impl of the trait — not `Into`'s, whose blanket impls can't)
            None if p.bounds().is_some_and(|b| {
                b.iter().any(|b| {
                    matches!(b, TypeParamBound::Trait(t)
                        if t.path.segments.last().is_some_and(|s| s.ident == "Into" || s.ident == "IntoIterator"))
                })
            }) =>
            {
                Vec::new()
            }
            None if p.bounds().is_some() => {
                let bounds = p.bounds().expect("bounds");
                generics.iter().filter(|g| g.inferred && !g.lifetime && bounds_mention(bounds, &g.key())).collect()
            }
            None => generics.iter().filter(|g| g.inferred && mentions(ty, &g.key())).collect(),
        };
        let fixed: Vec<&Generic> = fixed.into_iter().filter(|g| !free_keys.contains(&g.key())).collect();
        let renames: HashMap<String, String> = fixed
            .iter()
            .map(|g| {
                let k = g.key();
                let fresh =
                    if g.lifetime { format!("'__kzs_{}", k.trim_start_matches('\'')) } else { format!("__kzs_{k}") };
                (k, fresh)
            })
            .collect();
        let mut set_ty = ty.clone();
        Rename(&renames).visit_type_mut(&mut set_ty);
        let sub: HashMap<String, Ts> = renames
            .iter()
            .map(|(k, v)| {
                let t: Ts = if v.starts_with('\'') {
                    let l = Lifetime::new(v, Span::call_site());
                    quote!(#l)
                } else {
                    let i = Ident::new(v, Span::call_site());
                    quote!(#i)
                };
                (k.clone(), t)
            })
            .collect();
        let out_g = order(&sub);
        let fresh: Vec<Ts> = {
            let mut lts: Vec<Ts> = Vec::new();
            let mut tys: Vec<Ts> = Vec::new();
            for g in &fixed {
                let t = sub[&g.key()].clone();
                if g.lifetime { lts.push(t) } else { tys.push(t) }
            }
            lts.into_iter().chain(tys).collect()
        };
        // The setter's own generic parameters, what it takes, what its slot keeps and how it makes that.
        let (own, value, kept, convert) = match (p.bounds(), option_of(&set_ty)) {
            (Some(b), _) => {
                let mut b = b.clone();
                for bound in b.iter_mut() {
                    struct InBound<'a>(&'a HashMap<String, String>);
                    impl VisitMut for InBound<'_> {
                        fn visit_ident_mut(&mut self, i: &mut Ident) {
                            Rename(self.0).visit_ident_mut(i);
                        }
                    }
                    InBound(&renames).visit_type_param_bound_mut(bound);
                }
                (Some(quote!(__V: #b)), quote!(__V), quote!(__V), quote!(v))
            }
            // (an `Option<T>` whose `T` the setter fixes stays as it is: `Into` couldn't tell what `T` is)
            (None, Some(t)) if fresh.is_empty() => (
                Some(quote!(__V: ::core::convert::Into<::core::option::Option<#t>>)),
                quote!(__V),
                quote!(#set_ty),
                quote!(::core::convert::Into::into(v)),
            ),
            (None, _) => (None, quote!(#set_ty), quote!(#set_ty), quote!(v)),
        };
        // (`fresh` holds the lifetimes first, as a generic parameter list wants them)
        let (fresh_lts, fresh_tys): (Vec<&Ts>, Vec<&Ts>) = fresh.iter().partition(|t| t.to_string().starts_with('\''));
        let generic = {
            let all: Vec<&Ts> = fresh.iter().chain(own.iter()).collect();
            if all.is_empty() { quote!() } else { quote!(<#(#all),*>) }
        };
        let memo_generic = quote!(<#(#fresh_lts,)* __KzK, __KzKeys, #(#fresh_tys,)* #own>);
        let out_p: Vec<Ts> = ps
            .iter()
            .enumerate()
            .map(|(j, pj)| if j == i { quote!(#rt::__private::Set<#kept>) } else { quote!(#pj) })
            .collect();
        let fields = fs
            .iter()
            .enumerate()
            .map(|(j, f)| if j == i { quote!(#f: #rt::__private::Set(#convert)) } else { quote!(#f: self.#f) });
        let ret = quote!(#builder_name<#recv_lt #(#out_g,)* #(#out_p),*>);
        // (at the parameter: rustc's "method defined here" for a positional argument of the wrong type points at it)
        let arg = format_ident!("__arg{i}", span = name.span());
        let last = format_ident!("__last", span = name.span());
        // `setter`'s twin for a memoized closure.
        let memo = |setter: &Ident| {
            let twin = format_ident!("__kz_memo_{}", setter);
            if wants_id[i] {
                quote! {
                    #[track_caller]
                    pub fn #twin #memo_generic(self, keys: __KzK, v: #value) -> #ret
                    where
                        __KzK: ::core::ops::FnOnce() -> __KzKeys,
                        __KzKeys: #rt::__private::Keys + 'static,
                    {
                        let id = #rt::__private::memo_id(keys());
                        let mut b = self.#setter(v);
                        b.__kz_ids[#i] = id;
                        b
                    }
                }
            } else {
                quote! {
                    pub fn #twin #memo_generic(self, keys: __KzK, v: #value) -> #ret
                    where
                        __KzK: ::core::ops::FnOnce() -> __KzKeys,
                    {
                        let _ = keys;
                        self.#setter(v)
                    }
                }
            }
        };
        // `setter`'s twin for an argument written `Some(…)`: an `Option<T>`'s takes the `Option<T>` itself, so that
        // what is in it coerces (`Some(Rc::new(|| …))` to an `Option<Rc<dyn Fn()>>`, which `Into` can't tell) and a
        // free generic `T` is what it holds (`Into` couldn't choose between that and an `Option` of it); but a `T`
        // that is an `Option` (`Some(x)` given for `Some(Some(x))`) or an impl's generic parameter (which may be
        // one) is taken as the setter takes it.
        let coerced = match (p.bounds(), option_of(&set_ty)) {
            (None, Some(t)) if fresh.is_empty() => {
                let name = quote!(#t).to_string().replace(' ', "");
                let impls = generics.iter().any(|g| !g.lifetime && !free_keys.contains(&g.key()) && g.key() == name);
                option_of(t).is_none() && !impls
            }
            _ => false,
        };
        let some = |setter: &Ident| {
            let twin = format_ident!("__kz_some_{}", setter);
            if coerced {
                quote! {
                    pub fn #twin(self, v: #set_ty) -> #ret {
                        self.#setter(v)
                    }
                }
            } else {
                quote! {
                    pub fn #twin #generic(self, v: #value) -> #ret {
                        self.#setter(v)
                    }
                }
            }
        };
        let (memo_name, memo_arg, some_name, some_arg) = (memo(name), memo(&arg), some(name), some(&arg));
        // (an argument given: no identity, unless its memoized twin gives it one after)
        let ids_set = if wants_id[i] {
            Some(quote!(__kz_ids: {
                let mut ids = self.__kz_ids;
                ids[#i] = ::core::option::Option::None;
                ids
            },))
        } else {
            ids_move.clone()
        };
        setters.push(quote! {
            pub fn #name #generic(self, v: #value) -> #ret {
                #builder_name { _g: ::core::marker::PhantomData, #recv_move #ids_set #(#fields),* }
            }
            pub fn #arg #generic(self, v: #value) -> #ret {
                self.#name(v)
            }
            #memo_name
            #memo_arg
            #some_name
            #some_arg
        });
        if i == n - 1 {
            // (no `Some(…)` twin: what goes to `__last` is a closure)
            let memo_last = memo(&last);
            setters.push(quote! {
                pub fn __last #generic(self, v: #value) -> #ret {
                    self.#name(v)
                }
                #memo_last
            });
        }
    }

    let mut bounds: Vec<Ts> = generics.iter().filter_map(|g| g.bounds.clone()).collect();
    if let Some(w) = where_clause {
        bounds.extend(w.predicates.iter().map(|p| quote!(#p)));
    }
    let mut lets = Vec::new();
    for (i, p) in params.iter().enumerate() {
        let (pi, fi, ty) = (&ps[i], &fs[i], &p.ty);
        let a = format_ident!("__a{i}");
        // (the earlier parameters it reads, by their names: Kotlin's `b: Int = a * 2`)
        let earlier = earlier_lets(params, i, |k| {
            let s = format_ident!("__a{k}");
            quote!(#s)
        });
        let capturing = !earlier_in_default(params, i).is_empty();
        let default = p.default.clone().map(|mut d| {
            SelfToRecv.visit_expr_mut(&mut d);
            d
        });
        match (p.bounds(), default) {
            (None, None) => {
                bounds.push(quote!(#pi: #rt::__private::Required<Value = #ty>));
                lets.push(quote!(let #a = #rt::__private::Required::take(self.#fi);));
            }
            (Some(b), None) => {
                bounds.push(quote!(#pi: #rt::__private::Required, <#pi as #rt::__private::Required>::Value: #b));
                lets.push(quote!(let #a = #rt::__private::Required::take(self.#fi);));
            }
            (None, Some(d)) => {
                bounds.push(quote!(#pi: #rt::__private::ArgOr<#ty>));
                let d = if capturing { quote!({ #earlier #d }) } else { quote!(#d) };
                lets.push(quote!(let #a = #rt::__private::ArgOr::<#ty>::or(self.#fi, || #d);));
            }
            (Some(b), Some(d)) => {
                let fp = fn_pointer(p, capturing)?;
                bounds.push(quote!(#pi: #rt::__private::ImplArg<#fp>, <#pi as #rt::__private::ImplArg<#fp>>::Out: #b));
                // (rustc doesn't take a projection's `'static` from the where clause when its default's fn pointer is
                // higher-ranked, `for<'a> fn(&'a T)`: it asks it of the slot's type, `Unset` or `Set<V>`, so that is
                // bound too)
                if b.iter().any(|b| matches!(b, TypeParamBound::Lifetime(l) if l.ident == "static")) {
                    bounds.push(quote!(#pi: 'static));
                }
                let made = if capturing { quote!(::std::boxed::Box::new({ #earlier #d })) } else { quote!(#d) };
                lets.push(
                    quote!(let #a = #rt::__private::ImplArg::<#fp>::pick(self.#fi, || { let d: #fp = #made; d });),
                );
            }
        }
    }
    let recv_let = recv.map(|_| quote!(let __self = self.recv;));
    let ids_let = pass_ids.then(|| quote!(let __kz_ids = self.__kz_ids;));
    let new_param = recv_ty.as_ref().map(|t| quote!(recv: #t));
    let new_init = recv.map(|_| quote!(recv,));
    let _ = gnames;
    let all_g: Vec<Ts> = lifetimes.iter().chain(types.iter()).map(|g| g.name.clone()).collect();
    // (an impl's generics, which `new` starts the builder with: declared on it)
    let outer_g: Vec<Ts> =
        lifetimes.iter().chain(types.iter()).filter(|g| !g.inferred).map(|g| g.name.clone()).collect();
    // (a `&'static self` receiver: the impl's type parameters are `'static`, as the receiver's type wants)
    let static_bounds: Vec<Ts> = match recv {
        Some(Receiver::Static(_)) => types
            .iter()
            .filter(|g| !g.inferred)
            .map(|g| {
                let n = &g.name;
                quote!(#n: 'static)
            })
            .collect(),
        _ => Vec::new(),
    };
    // (a type parameter that may be unsized declared so wherever the builder declares it)
    let unsized_bounds: Vec<Ts> = types
        .iter()
        .filter(|g| g.maybe_unsized)
        .map(|g| {
            let n = &g.name;
            quote!(#n: ?Sized)
        })
        .collect();
    let new_unsized: Vec<Ts> = types
        .iter()
        .filter(|g| g.maybe_unsized && !g.inferred)
        .map(|g| {
            let n = &g.name;
            quote!(#n: ?Sized)
        })
        .collect();
    let head_bounds: Vec<&Ts> = static_bounds.iter().chain(unsized_bounds.iter()).collect();
    let static_wc = (!head_bounds.is_empty()).then(|| quote!(where #(#head_bounds),*));
    let new_bounds: Vec<&Ts> = static_bounds.iter().chain(new_unsized.iter()).collect();
    let new_wc = (!new_bounds.is_empty()).then(|| quote!(where #(#new_bounds),*));
    bounds.extend(static_bounds.iter().cloned());
    let items = quote! {
        #(#missing)*
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        #vis struct #builder_name<#recv_lt #(#all_g,)* #(#ps),*> #static_wc {
            _g: #phantom,
            #recv_field
            #ids_field
            #(#fs: #ps),*
        }
        #[doc(hidden)]
        #[allow(non_snake_case, clippy::needless_lifetimes)]
        #vis fn #new_name<#recv_lt #(#outer_g,)* #(#free_names,)*>(#new_param) -> #builder_name<#recv_lt #(#init_g,)* #(#initial),*> #new_wc {
            #builder_name { _g: ::core::marker::PhantomData, #new_init #ids_init #(#fs: #initial),* }
        }
        #[allow(clippy::type_complexity, clippy::too_many_arguments, non_snake_case)]
        impl<#recv_lt #(#all_g,)* #(#ps),*> #builder_name<#recv_lt #(#all_g,)* #(#ps),*> #static_wc {
            #(#setters)*
        }
        #[allow(clippy::type_complexity, clippy::too_many_arguments)]
        impl<#recv_lt #(#all_g,)* #(#ps),*> #builder_name<#recv_lt #(#all_g,)* #(#ps),*> where #(#bounds),* {
            #[track_caller]
            #[allow(clippy::needless_lifetimes)]
            pub fn __call(self) #output {
                #ids_let
                #recv_let
                #(#lets)*
                #call
            }
        }
    };
    let outside_lt = matches!(recv, Some(Receiver::Ref(..))).then(|| quote!('_,));
    let new_generics = if free_names.is_empty() { quote!() } else { quote!(<#(#free_names),*>) };
    Ok(Built { items, new_type: quote!(#builder_name<#outside_lt #(#init_g,)* #(#initial_outside),*>), new_generics })
}
