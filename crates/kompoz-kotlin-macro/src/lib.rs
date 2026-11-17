//! KompozR's Kotlin syntax in Rust sources: `kompoz! { … }` (Kotlin declarations in place) and
//! `kompoz_file!("ui.kt")` (a `.kt` file's), translated by kompoz-kotlin into the Rust `#[kompozable]` processes.
//!
//! ```ignore
//! kompoz! {
//!     @Kompozable
//!     fun Greeting() {}
//! }
//! kompoz_file!("src/screens.kt");
//! ```
//!
//! `kompoz!` takes its text from the tokens' source text (see `text`): positions are the Rust file's own lines and
//! columns, every generated token carries the span of the Kotlin token it comes from. Rust's lexer reads the input
//! first, so Kotlin it rejects can't be written here (backtick names, the escapes `\$`, `\b` and four-digit `\u`:
//! write `${'$'}`, `\u{8}`, `\u{41}`; raw strings with backslashes) — `kompoz_file!` has no such limits.
//! `kompoz_file!` reads the file relative to the crate's directory (`CARGO_MANIFEST_DIR`), rebuilds when it changes
//! (an `include_bytes!` of it: stable proc macros have no tracked paths), reports at the macro's call with the
//! file's `file:line:column` in the message. Its tokens all have that call's span, so the calls that keep something
//! in the composition get their identity from a key of their Kotlin line and column instead (`Options::keyed_calls`).

use kompoz_kotlin::{Diagnostic, OneSpan, Options, Pos, Severity, SpanMap, TextSource};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as Ts;
use quote::{format_ident, quote, quote_spanned};

mod paths;
mod text;

/// Kotlin declarations, translated in place.
#[proc_macro]
pub fn kompoz(input: TokenStream) -> TokenStream {
    let acquired = text::acquire(input);
    let opts = Options { paths: paths::resolve(), text: acquired.source, keyed_calls: false };
    let t = kompoz_kotlin::translate(&acquired.text, acquired.base, &acquired.spans, &opts);
    let mut out = t.tokens;
    out.extend(report(&t.diagnostics, &acquired.spans, None));
    out.into()
}

/// A `.kt` file's declarations (a path relative to the crate's directory), translated in place.
#[proc_macro]
pub fn kompoz_file(input: TokenStream) -> TokenStream {
    let lit = match syn::parse::<syn::LitStr>(input) {
        Ok(lit) => lit,
        Err(e) => {
            return syn::Error::new(e.span(), "kompoz: kompoz_file! takes a file's path: kompoz_file!(\"ui.kt\")")
                .to_compile_error()
                .into();
        }
    };
    let span = lit.span();
    let name = lit.value();
    let dir = std::env::var_os("CARGO_MANIFEST_DIR").map(std::path::PathBuf::from).unwrap_or_default();
    let path = dir.join(&name);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            let message = format!("kompoz: can't read {}: {e}", path.display());
            return quote_spanned!(span=> ::core::compile_error! { #message }).into();
        }
    };
    // (rustc's view of a source: no byte order mark, LF line ends)
    let text = content.strip_prefix('\u{feff}').unwrap_or(&content).replace("\r\n", "\n");
    let spans = OneSpan(span);
    let opts = Options { paths: paths::resolve(), text: TextSource::Exact, keyed_calls: true };
    let t = kompoz_kotlin::translate(&text, Pos::start(1, 1), &spans, &opts);
    let tracked = path.to_string_lossy().into_owned();
    let mut out = quote_spanned!(span=> const _: &[u8] = ::core::include_bytes!(#tracked););
    out.extend(t.tokens);
    out.extend(report(&t.diagnostics, &spans, Some(&name)));
    out.into()
}

/// What `kompoz!` reads its input as, for tests of the text acquisition: `(text, base line, rebuilt)`, the text's
/// first character at the base line's column 1, `rebuilt` when it came from the tokens rather than the source.
#[doc(hidden)]
#[proc_macro]
pub fn __kompoz_text(input: TokenStream) -> TokenStream {
    let a = text::acquire(input);
    let (text, line, rebuilt) = (a.text, a.base.line, a.source == TextSource::Rebuilt);
    quote!((#text, #line, #rebuilt)).into()
}

/// The diagnostics as Rust's: an error is a `compile_error!` at its span; a warning, a deprecated constant used
/// there (stable proc macros have no warnings of their own; rustc shows the note).
fn report(diagnostics: &[Diagnostic], spans: &dyn SpanMap, file: Option<&str>) -> Ts {
    let mut out = Ts::new();
    for (i, d) in diagnostics.iter().enumerate() {
        let span = spans.span_of(d.loc);
        let message = d.render(file);
        out.extend(match d.severity {
            Severity::Error => quote_spanned!(span=> ::core::compile_error! { #message }),
            Severity::Warning => {
                let name = format_ident!("kompoz_warning_{}", i, span = span);
                quote! {
                    const _: () = {
                        #[deprecated(note = #message)]
                        #[allow(non_upper_case_globals)]
                        const #name: () = ();
                        #name
                    };
                }
            }
        });
    }
    out
}
