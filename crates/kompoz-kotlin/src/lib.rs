//! KompozR's Kotlin front end: Kotlin's syntax (what `kompoz! { … }` holds, or a `.kt` file read by
//! `kompoz_file!`) read and turned into the Rust that `#[kompozable]` then processes — what the C++ Kompoz's
//! kompozc does for Kotlin in C++ sources.
//!
//! The pipeline, [`translate`]: the text is lexed ([`lexer::lex`]), parsed into the syntax tree ([`ast`],
//! [`parser::parse`]) and the tree generated into Rust items ([`codegen::generate`]); what is wrong is a
//! [`Diagnostic`] at a Kotlin position. Positions ([`Pos`]) are offsets into the text plus the line and column in
//! the file the text came from, and a [`SpanMap`] gives the Rust span for an offset: every emitted token carries its
//! Kotlin token's span, so rustc's errors and `#[track_caller]` identities land on Kotlin lines.
//!
//! This crate knows nothing of proc macros (it is `proc_macro2` throughout): the macros (kompoz-kotlin-macro)
//! acquire the text and the spans, the tools (an editor's language server, golden tests) call it directly.
//!
//! Module boundaries. `lexer` (WP1): `lex(text, base, diags) -> Vec<Token>`. `parser` (WP1):
//! `parse(&tokens, diags) -> ast::File`, the full grammar and per-declaration recovery. `ast`: frozen; every
//! package builds on it. `codegen` (WP4a): `generate(&file, &Context, diags) -> TokenStream`, the full generator.
//! `translate` and its types are the stable entry point the macros call.

pub mod ast;
pub mod codegen;
pub mod diag;
pub mod index;
pub mod lexer;
pub mod parser;
pub mod pos;

pub use diag::{Diagnostic, Diagnostics, Note, Severity};
pub use pos::{Loc, OneSpan, Pos, SpanMap};

use proc_macro2::TokenStream;
use quote::quote;

/// The paths the generated Rust names KompozR's crates by: through the umbrella crate (`::kompoz::runtime`) or
/// the crates themselves (`::kompoz_runtime`), as the calling crate depends on them (the macro finds out).
#[derive(Clone, Debug)]
pub struct Paths {
    pub runtime: TokenStream,
    pub coroutines: TokenStream,
    pub graphics: TokenStream,
    pub text: TokenStream,
    pub ui: TokenStream,
    pub foundation: TokenStream,
}

impl Paths {
    /// Through the umbrella crate `kompoz` (named `name` by the calling crate).
    pub fn umbrella(name: &proc_macro2::Ident) -> Paths {
        Paths {
            runtime: quote!(::#name::runtime),
            coroutines: quote!(::#name::coroutines),
            graphics: quote!(::#name::graphics),
            text: quote!(::#name::text),
            ui: quote!(::#name::ui),
            foundation: quote!(::#name::foundation),
        }
    }
}

impl Default for Paths {
    /// The crates themselves under their own names.
    fn default() -> Paths {
        Paths {
            runtime: quote!(::kompoz_runtime),
            coroutines: quote!(::kompoz_coroutines),
            graphics: quote!(::kompoz_graphics),
            text: quote!(::kompoz_text),
            ui: quote!(::kompoz_ui),
            foundation: quote!(::kompoz_foundation),
        }
    }
}

/// How the text was made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextSource {
    /// The source as written (`kompoz!` under rustc: tokens' source text, the gaps between them rebuilt from their
    /// lines and columns; a `.kt` file): positions are the file's, newlines are the author's.
    #[default]
    Exact,
    /// Rebuilt from the tokens without their source text (rust-analyzer's proc-macro server has none): newlines
    /// are guessed where a statement must have ended, positions are the rebuilt text's own. The front end reads it
    /// as best it can; diagnostics still land on the right tokens through the span map.
    Rebuilt,
}

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub paths: Paths,
    pub text: TextSource,
    /// The spans can't tell calls apart (a `.kt` file's tokens all have the span of `kompoz_file!`'s argument): each
    /// call that keeps something in the composition (a composable, `remember…`) gets a group keyed by its Kotlin
    /// line and column (`key(&(line, column), || …)`), the identity `#[track_caller]` gives it in `kompoz!`.
    pub keyed_calls: bool,
}

/// What a translation gives: the Rust items, and the diagnostics (errors and warnings) in the order found. With
/// errors, the items are what could be translated (each failed declaration left out), so one mistake doesn't
/// bury the rest under errors of their own.
pub struct Translation {
    pub tokens: TokenStream,
    pub diagnostics: Vec<Diagnostic>,
}

/// The Rust that `text` (Kotlin declarations; its offset 0 at `base`) becomes, spans from `spans`.
pub fn translate(text: &str, base: Pos, spans: &dyn SpanMap, opts: &Options) -> Translation {
    let mut diags = Diagnostics::default();
    let tokens = lexer::lex(text, base, &mut diags);
    let file = parser::parse(&tokens, &mut diags);
    let cx = codegen::Context { spans, paths: &opts.paths, keyed_calls: opts.keyed_calls };
    let tokens = codegen::generate(&file, &cx, &mut diags);
    Translation { tokens, diagnostics: diags.list }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lints a generated item allows.
    fn allow() -> TokenStream {
        quote! {
            #[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
        }
    }

    #[test]
    fn a_kompozable_end_to_end() {
        let t = translate(
            "@Kompozable fun Empty() {}\nprivate fun helper() {\n}",
            Pos::start(4, 5),
            &OneSpan::default(),
            &Options::default(),
        );
        assert!(t.diagnostics.is_empty(), "{:?}", t.diagnostics);
        let allow = allow();
        let expected = quote! {
            #allow #[::kompoz_runtime::kompozable] pub fn Empty() {}
            #allow fn helper() {}
        };
        assert_eq!(t.tokens.to_string(), expected.to_string());
    }

    #[test]
    fn through_the_umbrella() {
        let opts = Options {
            paths: Paths::umbrella(&proc_macro2::Ident::new("kz", proc_macro2::Span::call_site())),
            ..Default::default()
        };
        let t = translate("@Composable internal fun `match`() {}", Pos::start(1, 1), &OneSpan::default(), &opts);
        assert!(t.diagnostics.is_empty(), "{:?}", t.diagnostics);
        let allow = allow();
        let expected = quote! { #allow #[::kz::runtime::kompozable] pub(crate) fn r#match() {} };
        assert_eq!(t.tokens.to_string(), expected.to_string());
    }

    #[test]
    fn diagnostics_at_kotlin_positions() {
        let t = translate(
            "@Kompozable fun A() {}\n  fun B(x: ) {}\nfun `a b`() {}",
            Pos::start(10, 1),
            &OneSpan::default(),
            &Options::default(),
        );
        let found: Vec<(String, u32, u32)> =
            t.diagnostics.iter().map(|d| (d.message.clone(), d.loc.start.line, d.loc.start.column)).collect();
        assert_eq!(
            found,
            [("expected a type, found `)`".to_string(), 11, 12), ("`a b` can't be a name in Rust".to_string(), 12, 5)]
        );
        let allow = allow();
        let expected = quote! { #allow #[::kompoz_runtime::kompozable] pub fn A() {} };
        assert_eq!(t.tokens.to_string(), expected.to_string());
    }
}
