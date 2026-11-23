//! `#[derive(Saveable)]`: a struct or enum written out as a `SavedValue` and read back (what Kotlin's
//! `@Serializable` is to savedstate's serialization: `encodeToSavedState`, a `SavedStateHandle`'s value, a
//! `rememberSaveable` without a Saver). As kotlinx.serialization writes a class into a SavedState:
//!
//! - a struct with named fields: a map of its fields by name (`#[saveable(name = "…")]` renames one — Kotlin's
//!   `@SerialName`); a tuple struct: a list of its fields; a unit struct: an empty map;
//! - an enum of unit variants only: its variant's index (kotlinx's `encodeEnum`);
//! - any other enum (Kotlin's sealed class): a map with the variant's name under `"type"` (kotlinx's class
//!   discriminator; `#[saveable(name = "…")]` on a variant) and its fields (a tuple variant's by index: `"0"`, …);
//! - `#[saveable(default)]` on a field: missing (or unreadable) when read back, it is `Default::default()` (Kotlin's
//!   property with a default value); `#[saveable(skip)]`: not written, read back as `Default::default()` (Kotlin's
//!   `@Transient`).
//!
//! Each field's type is `Saveable` (numbers, strings, bools, vectors, options, states, other derived types); a
//! generic parameter is bound by it.

use proc_macro2::TokenStream as Ts;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::*;

#[derive(Default)]
struct Options {
    name: Option<String>,
    default: bool,
    skip: bool,
}

fn options(attrs: &[Attribute]) -> Result<Options> {
    let mut o = Options::default();
    for a in attrs.iter().filter(|a| a.path().is_ident("saveable")) {
        a.parse_nested_meta(|m| {
            if m.path.is_ident("name") {
                let s: LitStr = m.value()?.parse()?;
                o.name = Some(s.value());
            } else if m.path.is_ident("default") {
                o.default = true;
            } else if m.path.is_ident("skip") {
                o.skip = true;
            } else {
                return Err(m.error("kompoz: `#[saveable(…)]` takes `name = \"…\"`, `default` or `skip`"));
            }
            Ok(())
        })?;
    }
    Ok(o)
}

/// How each field is written (`save`: what gives the field's value, by reference) and read (`restore`: from the
/// saved value `src`, given an expression of the saved field `at`).
struct FieldCode {
    /// `(key, saved value)` pairs or list elements.
    save: Vec<Ts>,
    /// `ident: value` / values, in order.
    restore: Vec<Ts>,
}

/// The code of `fields`, their saved values found by `find(key)` (a map's) or by index (a list's).
fn fields_code(rt: &Ts, fields: &Fields, access: impl Fn(&Member) -> Ts, as_map: bool) -> Result<FieldCode> {
    let mut save = Vec::new();
    let mut restore = Vec::new();
    for (i, f) in fields.iter().enumerate() {
        let o = options(&f.attrs)?;
        let member = match &f.ident {
            Some(id) => Member::Named(id.clone()),
            None => Member::Unnamed(Index::from(i)),
        };
        let ty = &f.ty;
        let key = o.name.clone().unwrap_or_else(|| match &f.ident {
            Some(id) => id.to_string().trim_start_matches("r#").to_string(),
            None => i.to_string(),
        });
        let value = access(&member);
        let read = if as_map { quote!(__kz_map.get(#key)) } else { quote!(__kz_list.get(#i)) };
        let restored = if o.skip {
            quote!(::core::default::Default::default())
        } else if o.default {
            quote!(#read.and_then(|v| <#ty as #rt::Saveable>::restore(v)).unwrap_or_default())
        } else {
            quote!(<#ty as #rt::Saveable>::restore(#read?)?)
        };
        if !o.skip {
            save.push(if as_map {
                quote!((::std::string::String::from(#key), #rt::Saveable::save(#value)))
            } else {
                quote!(#rt::Saveable::save(#value))
            });
        }
        restore.push(match &f.ident {
            Some(id) => quote!(#id: #restored),
            None => restored,
        });
    }
    Ok(FieldCode { save, restore })
}

pub(crate) fn derive(input: DeriveInput, rt: Ts) -> Result<Ts> {
    let name = &input.ident;
    let mut generics = input.generics.clone();
    {
        let where_clause = generics.make_where_clause();
        for p in input.generics.type_params() {
            let id = &p.ident;
            where_clause.predicates.push(parse_quote!(#id: #rt::Saveable));
        }
    }
    let (impl_g, ty_g, where_g) = generics.split_for_impl();
    let (save, restore) = match &input.data {
        Data::Struct(s) => struct_code(&rt, &s.fields)?,
        Data::Enum(e) => enum_code(&rt, name, e)?,
        Data::Union(u) => {
            return Err(Error::new(u.union_token.span(), "kompoz: `#[derive(Saveable)]` is for a struct or an enum"));
        }
    };
    Ok(quote! {
        impl #impl_g #rt::Saveable for #name #ty_g #where_g {
            fn save(&self) -> #rt::SavedValue {
                #save
            }
            #[allow(unused_variables, clippy::question_mark)]
            fn restore(__kz_saved: &#rt::SavedValue) -> ::core::option::Option<Self> {
                #restore
            }
        }
    })
}

fn struct_code(rt: &Ts, fields: &Fields) -> Result<(Ts, Ts)> {
    match fields {
        Fields::Named(_) => {
            let c = fields_code(rt, fields, |m| quote!(&self.#m), true)?;
            let (save, restore) = (c.save, c.restore);
            Ok((
                quote!({
                    let __kz_fields: ::std::vec::Vec<(::std::string::String, #rt::SavedValue)> = ::std::vec![#(#save),*];
                    #rt::SavedValue::map(__kz_fields)
                }),
                quote! {
                    let __kz_map = __kz_saved.as_map()?;
                    ::core::option::Option::Some(Self { #(#restore),* })
                },
            ))
        }
        Fields::Unnamed(_) => {
            let c = fields_code(rt, fields, |m| quote!(&self.#m), false)?;
            let (save, restore) = (c.save, c.restore);
            Ok((
                quote!({
                    let __kz_fields: ::std::vec::Vec<#rt::SavedValue> = ::std::vec![#(#save),*];
                    #rt::SavedValue::list(__kz_fields)
                }),
                quote! {
                    let __kz_list = __kz_saved.as_list()?;
                    ::core::option::Option::Some(Self(#(#restore),*))
                },
            ))
        }
        Fields::Unit => Ok((
            quote!(#rt::SavedValue::map(::std::iter::empty::<(::std::string::String, #rt::SavedValue)>())),
            quote! {
                __kz_saved.as_map()?;
                ::core::option::Option::Some(Self)
            },
        )),
    }
}

fn enum_code(rt: &Ts, name: &Ident, e: &DataEnum) -> Result<(Ts, Ts)> {
    let all_unit = e.variants.iter().all(|v| matches!(v.fields, Fields::Unit));
    if all_unit {
        let arms_save = e.variants.iter().enumerate().map(|(i, v)| {
            let id = &v.ident;
            let i = i as i64;
            quote!(#name::#id => #rt::SavedValue::Int(#i))
        });
        let arms_restore = e.variants.iter().enumerate().map(|(i, v)| {
            let id = &v.ident;
            let i = i as i64;
            quote!(#i => ::core::option::Option::Some(#name::#id))
        });
        return Ok((
            quote!(match self { #(#arms_save,)* }),
            quote! {
                match __kz_saved.as_int()? {
                    #(#arms_restore,)*
                    _ => ::core::option::Option::None,
                }
            },
        ));
    }
    let mut saves = Vec::new();
    let mut restores = Vec::new();
    for v in &e.variants {
        let o = options(&v.attrs)?;
        let id = &v.ident;
        let tag = o.name.unwrap_or_else(|| id.to_string());
        let bindings: Vec<Ident> = (0..v.fields.len()).map(|i| format_ident!("__kz_f{i}")).collect();
        let named: Vec<Ident> = v.fields.iter().filter_map(|f| f.ident.clone()).collect();
        let pattern = match &v.fields {
            Fields::Named(_) => quote!(#name::#id { #(#named: #bindings),* }),
            Fields::Unnamed(_) => quote!(#name::#id(#(#bindings),*)),
            Fields::Unit => quote!(#name::#id),
        };
        let at = |m: &Member| -> Ts {
            let i = match m {
                Member::Named(n) => named.iter().position(|x| x == n).unwrap_or(0),
                Member::Unnamed(i) => i.index as usize,
            };
            let b = &bindings[i];
            quote!(#b)
        };
        let c = fields_code(rt, &v.fields, at, true)?;
        let (save, restore) = (c.save, c.restore);
        saves.push(quote! {
            #pattern => {
                let __kz_fields: ::std::vec::Vec<(::std::string::String, #rt::SavedValue)> = ::std::vec![#(#save),*];
                #rt::SavedValue::map(
                    ::core::iter::once((::std::string::String::from("type"), #rt::SavedValue::from(#tag)))
                        .chain(__kz_fields),
                )
            }
        });
        let made = match &v.fields {
            Fields::Named(_) => quote!(#name::#id { #(#restore),* }),
            Fields::Unnamed(_) => quote!(#name::#id(#(#restore),*)),
            Fields::Unit => quote!(#name::#id),
        };
        restores.push(quote!(#tag => ::core::option::Option::Some(#made)));
    }
    Ok((
        quote!(match self { #(#saves)* }),
        quote! {
            let __kz_map = __kz_saved.as_map()?;
            match __kz_map.get("type")?.as_str()? {
                #(#restores,)*
                _ => ::core::option::Option::None,
            }
        },
    ))
}
