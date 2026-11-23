//! `#[derive(Route)]` and `#[derive(RouteValue)]`: typed navigation routes (Compose's `@Serializable` route classes
//! and the types of their properties; kompoz-navigation's `serialization` module).
//!
//! - `#[derive(Route)]` on a struct with named fields (Kotlin's data class) or a unit struct (Kotlin's `object`):
//!   its serial name is the type's name, or `#[route(name = "…")]`'s (Kotlin's `@SerialName`); each field an
//!   argument, named as the field or by its own `#[route(name = "…")]`. A field with a default — an `Option`
//!   (`None`), `#[route(default)]` (`Default::default()`), `#[route(default = expr)]` — is optional: a query
//!   parameter, and what a value not given takes when its arguments are read back. Each field's type is a
//!   `RouteValue`.
//! - `#[derive(RouteValue)]` on an enum of unit variants gives it its `NavType`s (Kotlin's enum `NavType`: entries by
//!   name); on another type, none built in (its `NavType` comes from the routes' `typeMap`).

use proc_macro2::TokenStream as Ts;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::*;

/// The navigation crate's path in the calling crate: through the umbrella crate when it depends on it.
fn navigation() -> Ts {
    use proc_macro_crate::{FoundCrate, crate_name};
    match crate_name("kompoz") {
        Ok(FoundCrate::Itself) => return quote!(::kompoz::navigation),
        Ok(FoundCrate::Name(n)) => {
            let i = format_ident!("{n}");
            return quote!(::#i::navigation);
        }
        Err(_) => {}
    }
    match crate_name("kompoz-navigation") {
        Ok(FoundCrate::Name(n)) => {
            let i = format_ident!("{n}");
            quote!(::#i)
        }
        Ok(FoundCrate::Itself) | Err(_) => quote!(::kompoz_navigation),
    }
}

/// What `#[route(…)]` says.
#[derive(Default)]
struct RouteAttr {
    name: Option<LitStr>,
    /// `Some(None)`: `#[route(default)]`; `Some(Some(e))`: `#[route(default = e)]`.
    default: Option<Option<Expr>>,
}

fn route_attr(attrs: &[Attribute], on_field: bool) -> Result<RouteAttr> {
    let mut out = RouteAttr::default();
    for a in attrs.iter().filter(|a| a.path().is_ident("route")) {
        a.parse_nested_meta(|m| {
            if m.path.is_ident("name") {
                out.name = Some(m.value()?.parse()?);
            } else if m.path.is_ident("default") && on_field {
                out.default = Some(if m.input.peek(Token![=]) { Some(m.value()?.parse()?) } else { None });
            } else {
                let allowed =
                    if on_field { "`name = \"…\"`, `default` or `default = expr`" } else { "`name = \"…\"`" };
                return Err(m.error(format!("kompoz: #[route(…)] here takes {allowed}")));
            }
            Ok(())
        })?;
    }
    Ok(out)
}

/// Whether `ty` is written `Option<…>`.
fn is_option(ty: &Type) -> bool {
    matches!(ty, Type::Path(p) if p.qself.is_none() && p.path.segments.last().is_some_and(|s| s.ident == "Option"))
}

pub(crate) fn derive_route(input: DeriveInput) -> Result<Ts> {
    if !input.generics.params.is_empty() {
        return Err(Error::new(
            input.generics.span(),
            "kompoz: a route is a type without generic parameters (Kotlin's route classes are concrete classes or objects)",
        ));
    }
    let nav = navigation();
    let ident = &input.ident;
    let attr = route_attr(&input.attrs, false)?;
    let serial_name = attr.name.map(|l| l.value()).unwrap_or_else(|| ident.to_string());
    let (fields, unit) = match &input.data {
        Data::Struct(DataStruct { fields: Fields::Named(f), .. }) => (f.named.iter().collect::<Vec<_>>(), false),
        Data::Struct(DataStruct { fields: Fields::Unit, .. }) => (Vec::new(), true),
        _ => {
            return Err(Error::new(
                ident.span(),
                "kompoz: #[derive(Route)] is for a struct with named fields (Kotlin's data class) or a unit struct \
                 (Kotlin's object)",
            ));
        }
    };
    let mut elements = Vec::new();
    let mut values = Vec::new();
    let mut decoded = Vec::new();
    let mut compared = Vec::new();
    for (index, f) in fields.iter().enumerate() {
        let a = route_attr(&f.attrs, true)?;
        let field = f.ident.as_ref().expect("a named field");
        let name = a.name.map(|l| l.value()).unwrap_or_else(|| field.to_string().trim_start_matches("r#").to_string());
        let ty = &f.ty;
        let optional = a.default.is_some() || is_option(ty);
        elements.push(quote! {
            #nav::RouteElement {
                name: #name,
                optional: #optional,
                nullable: <#ty as #nav::RouteValue>::NULLABLE,
                ktype: #nav::type_of::<#ty>,
                nav_type: <#ty as #nav::RouteValue>::nav_type,
            }
        });
        values.push(quote! { #index => #nav::RouteValue::to_nav_value(&self.#field), });
        let missing = match a.default {
            Some(Some(e)) => quote!(#e),
            Some(None) => quote!(<#ty as ::core::default::Default>::default()),
            None if is_option(ty) => quote!(::core::option::Option::None),
            None => quote!(return ::core::result::Result::Err(#nav::missing_field(#name, #serial_name))),
        };
        decoded.push(quote! {
            #field: match value(#index) {
                ::core::option::Option::Some(v) => <#ty as #nav::RouteValue>::from_nav_value(v)
                    .ok_or_else(|| #nav::unexpected_null(#name))?,
                ::core::option::Option::None => #missing,
            },
        });
        compared.push(quote!(&& self.#field == o.#field));
    }
    let count = elements.len();
    let construct = if unit { quote!(#ident) } else { quote!(#ident { #(#decoded)* }) };
    let value_param = if unit { quote!(_value) } else { quote!(value) };
    let element_value = if unit {
        quote! {
            fn element_value(&self, index: usize) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
                panic!("kompoz: {} has no field {index}", #serial_name)
            }
        }
    } else {
        quote! {
            fn element_value(&self, index: usize) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
                match index {
                    #(#values)*
                    _ => panic!("kompoz: {} has no field {index}", #serial_name),
                }
            }
        }
    };
    Ok(quote! {
        impl #nav::Route for #ident {
            fn descriptor() -> &'static #nav::RouteDescriptor {
                static ELEMENTS: [#nav::RouteElement; #count] = [#(#elements),*];
                static DESCRIPTOR: #nav::RouteDescriptor =
                    #nav::RouteDescriptor { serial_name: #serial_name, elements: &ELEMENTS };
                &DESCRIPTOR
            }
            #element_value
            fn from_elements(
                #value_param: &mut dyn FnMut(usize) -> ::core::option::Option<::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>>>,
            ) -> ::core::result::Result<Self, ::std::string::String> {
                ::core::result::Result::Ok(#construct)
            }
            #[allow(unused_variables)]
            fn route_eq(&self, other: &dyn ::core::any::Any) -> bool {
                match other.downcast_ref::<Self>() {
                    ::core::option::Option::Some(o) => true #(#compared)*,
                    ::core::option::Option::None => false,
                }
            }
        }
    })
}

pub(crate) fn derive_route_value(input: DeriveInput) -> Result<Ts> {
    let nav = navigation();
    let ident = &input.ident;
    let (gi, tg, wc) = input.generics.split_for_impl();
    let Data::Enum(e) = &input.data else {
        // (a type of one's own: its NavType is the typeMap's)
        return Ok(quote! { impl #gi #nav::RouteValue for #ident #tg #wc {} });
    };
    if !input.generics.params.is_empty() {
        return Err(Error::new(input.generics.span(), "kompoz: a RouteValue enum has no generic parameters"));
    }
    let mut entries = Vec::new();
    let mut names = Vec::new();
    for v in &e.variants {
        if !matches!(v.fields, Fields::Unit) {
            return Err(Error::new(
                v.span(),
                "kompoz: a RouteValue enum's entries have no fields (Kotlin's enum entries)",
            ));
        }
        let vi = &v.ident;
        let name = vi.to_string();
        entries.push(quote!(#ident::#vi));
        names.push(quote!(#ident::#vi => #name,));
    }
    let count = entries.len();
    Ok(quote! {
        impl #nav::NavEnum for #ident {
            fn entries() -> &'static [Self] {
                static ENTRIES: [#ident; #count] = [#(#entries),*];
                &ENTRIES
            }
            fn name(&self) -> &'static str {
                match self {
                    #(#names)*
                }
            }
        }
        impl #nav::RouteValue for #ident {
            fn nav_type() -> ::core::option::Option<#nav::NavType> {
                ::core::option::Option::Some(#nav::enum_route_nav_types::<Self>().0)
            }
            fn nullable_nav_type() -> ::core::option::Option<#nav::NavType> {
                ::core::option::Option::Some(#nav::enum_route_nav_types::<Self>().1)
            }
            fn list_nav_type() -> ::core::option::Option<#nav::NavType> {
                ::core::option::Option::Some(#nav::enum_route_nav_types::<Self>().2)
            }
        }
    })
}
