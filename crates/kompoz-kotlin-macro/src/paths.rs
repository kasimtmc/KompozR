//! How the calling crate names KompozR's crates: through the umbrella crate `kompoz` when it depends on it,
//! else each crate by the name it gives it (renamed dependencies too); a crate it doesn't depend on keeps its own
//! name (rustc then says what is missing).

use kompoz_kotlin::Paths;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

pub(crate) fn resolve() -> Paths {
    match crate_name("kompoz") {
        Ok(FoundCrate::Name(n)) => return Paths::umbrella(&Ident::new(&n, Span::call_site())),
        // (the umbrella crate itself: `extern crate self as kompoz`)
        Ok(FoundCrate::Itself) => return Paths::umbrella(&Ident::new("kompoz", Span::call_site())),
        Err(_) => {}
    }
    Paths {
        runtime: path("kompoz-runtime"),
        coroutines: path("kompoz-coroutines"),
        graphics: path("kompoz-graphics"),
        text: path("kompoz-text"),
        ui: path("kompoz-ui"),
        foundation: path("kompoz-foundation"),
    }
}

/// One crate's path: `::name` as the calling crate names it, `crate` in the crate itself (Kotlin written in
/// foundation names foundation so).
fn path(package: &str) -> TokenStream {
    let name = match crate_name(package) {
        Ok(FoundCrate::Name(n)) => n,
        Ok(FoundCrate::Itself) => return quote!(crate),
        Err(_) => package.replace('-', "_"),
    };
    let i = Ident::new(&name, Span::call_site());
    quote!(::#i)
}
