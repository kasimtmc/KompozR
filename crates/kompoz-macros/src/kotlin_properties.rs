//! `#[derive(KotlinProperties)]`: a struct's public fields as Kotlin properties. The Kotlin syntax reads every
//! property as a call (`a.b` → `a.b()`) and writes it through a setter (`a.b = v` → `a.set_b(v)`), so a Rust type
//! that Kotlin code reads gets a getter of each field's own name, and a setter where Kotlin writes it.
//!
//! - each field as visible as the getter will be (`pub`, `pub(crate)`…; a private one is no property) gets
//!   `fn name(&self) -> T`, a clone of it (Kotlin's values; a `Copy` field is copied);
//! - `#[kotlin(set)]` on a field, or on the struct for all of them, adds `fn set_name(&mut self, value: T)`;
//! - `#[kotlin(ref)]` on a field makes its getter `fn name(&self) -> &T` (a field that isn't `Clone`);
//! - `#[kotlin(skip)]` on a field leaves it out.
//!
//! A type that already has a method of a field's name (`Dp::value`) gets rustc's "duplicate definitions with name
//! `…`" pointing at the field: mark that field `#[kotlin(skip)]` (its method is its property already).

use proc_macro2::TokenStream as Ts;
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::*;

/// What `#[kotlin(…)]` says of a field (or, `set` only, of the struct).
#[derive(Default)]
struct Options {
    set: bool,
    by_ref: bool,
    skip: bool,
}

fn options(attrs: &[Attribute], on_struct: bool) -> Result<Options> {
    let mut o = Options::default();
    for a in attrs.iter().filter(|a| a.path().is_ident("kotlin")) {
        a.parse_nested_meta(|m| {
            if m.path.is_ident("set") {
                o.set = true;
            } else if m.path.is_ident("ref") && !on_struct {
                o.by_ref = true;
            } else if m.path.is_ident("skip") && !on_struct {
                o.skip = true;
            } else {
                let allowed = if on_struct { "`set`" } else { "`set`, `ref` or `skip`" };
                return Err(m.error(format!("kompoz: #[kotlin(…)] here takes {allowed}")));
            }
            Ok(())
        })?;
    }
    Ok(o)
}

pub(crate) fn derive(input: DeriveInput) -> Result<Ts> {
    let fields = match &input.data {
        Data::Struct(DataStruct { fields: Fields::Named(f), .. }) => &f.named,
        _ => {
            return Err(Error::new(
                input.ident.span(),
                "kompoz: KotlinProperties is for a struct with named fields (a tuple struct's or an enum's have no \
                 Kotlin property names)",
            ));
        }
    };
    let all_set = options(&input.attrs, true)?.set;
    let name = &input.ident;
    let (gi, tg, wc) = input.generics.split_for_impl();
    let type_params: Vec<String> = input.generics.type_params().map(|p| p.ident.to_string()).collect();
    let mut methods = Vec::new();
    for f in fields {
        let o = options(&f.attrs, false)?;
        if o.skip || matches!(f.vis, Visibility::Inherited) {
            if o.set || o.by_ref {
                return Err(Error::new(f.span(), "kompoz: a skipped or private field has no Kotlin property"));
            }
            continue;
        }
        let ident = f.ident.as_ref().expect("a named field");
        let (vis, ty) = (&f.vis, &f.ty);
        let span = ident.span();
        let shown = ident.to_string();
        let shown = shown.trim_start_matches("r#");
        let getter_doc = format!("Kotlin's `{}` property (the field `{shown}`).", camel(shown));
        methods.push(if o.by_ref {
            quote_spanned! {span=>
                #[doc = #getter_doc]
                #[inline]
                #vis fn #ident(&self) -> &#ty {
                    &self.#ident
                }
            }
        } else {
            // (a field of a generic type is cloned where it can be: the getter asks it)
            let bound = generic_in(ty, &type_params).then(|| quote!(where #ty: ::core::clone::Clone));
            quote_spanned! {span=>
                #[doc = #getter_doc]
                #[inline]
                #vis fn #ident(&self) -> #ty #bound {
                    ::core::clone::Clone::clone(&self.#ident)
                }
            }
        });
        if o.set || all_set {
            let setter = format_ident!("set_{}", shown, span = span);
            let setter_doc = format!("Kotlin's `{} = value`.", camel(shown));
            methods.push(quote_spanned! {span=>
                #[doc = #setter_doc]
                #[inline]
                #vis fn #setter(&mut self, value: #ty) {
                    self.#ident = value;
                }
            });
        }
    }
    // (a crate-visible field's getter is for Kotlin code in the crate, which may have none)
    Ok(quote! {
        #[allow(clippy::clone_on_copy, dead_code)]
        impl #gi #name #tg #wc {
            #(#methods)*
        }
    })
}

/// Whether `ty` names one of the struct's type parameters.
fn generic_in(ty: &Type, params: &[String]) -> bool {
    fn walk(ts: proc_macro2::TokenStream, params: &[String]) -> bool {
        ts.into_iter().any(|t| match t {
            proc_macro2::TokenTree::Ident(i) => params.iter().any(|p| i == p),
            proc_macro2::TokenTree::Group(g) => walk(g.stream(), params),
            _ => false,
        })
    }
    !params.is_empty() && walk(quote!(#ty), params)
}

/// A snake_case name as Kotlin writes it (`font_scale` → `fontScale`), for the docs.
fn camel(snake: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for c in snake.chars() {
        if c == '_' {
            up = !out.is_empty();
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}
