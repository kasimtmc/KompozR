//! The Rust a Kotlin file becomes (the C++ kompozc's `codegen.cpp`): items `#[kompozable]` then processes, so
//! identity, restarting and skipping, builders and captures are the Rust syntax's own. Every token gets the span
//! of the Kotlin it comes from (`SpanMap`): rustc's errors and `#[track_caller]`'s call sites land on Kotlin lines.
//!
//! Module boundary (WP4a): `generate(file, cx, diags) -> TokenStream` is the entry point the pipeline calls; WP4a
//! replaces this module's body with the full generator (name resolution against the API index, declarations of
//! every kind, statements and expressions, the kt shims). What it translates now: functions without parameters
//! and with an empty body — `@Kompozable fun Name() {}` → `#[kompozable] pub fn Name() {}`, a plain `fun` → a
//! plain `fn`; any other declaration is an error saying so.

use crate::Paths;
use crate::ast::*;
use crate::diag::Diagnostics;
use crate::pos::{Loc, SpanMap};
use proc_macro2::{Ident as RustIdent, Span, TokenStream};
use quote::{quote, quote_spanned};

/// What the generator works with besides the tree.
pub struct Context<'a> {
    pub spans: &'a dyn SpanMap,
    pub paths: &'a Paths,
}

impl Context<'_> {
    fn span(&self, loc: Loc) -> Span {
        self.spans.span_of(loc)
    }
}

/// The items `file`'s declarations become; what can't be translated is a diagnostic (and nothing in the output).
pub fn generate(file: &File, cx: &Context, diags: &mut Diagnostics) -> TokenStream {
    let mut out = TokenStream::new();
    for decl in &file.decls {
        match decl {
            Decl::Function(f) => {
                if let Some(ts) = function(f, cx, diags) {
                    out.extend(ts);
                }
            }
            // (its diagnostics are given)
            Decl::Failed { .. } => {}
            Decl::Property(p) => untranslated(p.loc, "a property", diags),
            Decl::Class(c) => untranslated(c.loc, "a class", diags),
            Decl::Object(o) => untranslated(o.loc, "an object", diags),
            Decl::TypeAlias(a) => untranslated(a.loc, "a type alias", diags),
        }
    }
    out
}

fn untranslated(loc: Loc, what: &str, diags: &mut Diagnostics) {
    diags.error(loc, format!("{what} is not translated yet (functions without parameters and with an empty body are)"));
}

fn function(f: &Function, cx: &Context, diags: &mut Diagnostics) -> Option<TokenStream> {
    if !f.params.is_empty() || f.receiver.is_some() || !f.type_params.is_empty() || f.result.is_some() {
        untranslated(f.loc, "a function's signature beyond `name()`", diags);
        return None;
    }
    let body = match &f.body {
        Some(FunctionBody::Block(b)) if b.stmts.is_empty() => b,
        _ => {
            untranslated(f.loc, "a function's body", diags);
            return None;
        }
    };
    let name = rust_ident(&f.name, cx, diags)?;
    let vis = match f.modifiers.visibility() {
        Visibility::Public => quote_spanned!(cx.span(f.loc)=> pub),
        Visibility::Internal => quote_spanned!(cx.span(f.loc)=> pub(crate)),
        Visibility::Private => quote!(),
        Visibility::Protected => {
            let loc = f.modifiers.find(ModifierKind::Protected).unwrap_or(f.loc);
            diags.error(loc, "`protected` is for a class's members");
            return None;
        }
    };
    let attr = f.annotations.iter().find(|a| a.is_kompozable()).map(|a| {
        let rt = &cx.paths.runtime;
        let span = cx.span(a.loc);
        quote_spanned!(span=> #[#rt::kompozable])
    });
    let fn_kw = quote_spanned!(cx.span(f.loc)=> fn);
    let block = quote_spanned!(cx.span(body.loc)=> {});
    Some(quote! {
        #[allow(non_snake_case)]
        #attr
        #vis #fn_kw #name() #block
    })
}

/// A Kotlin name as a Rust identifier: a raw one for a Rust keyword (`r#match`); an error for one Rust can't name
/// (a backticked name with spaces, `self`).
fn rust_ident(name: &Ident, cx: &Context, diags: &mut Diagnostics) -> Option<RustIdent> {
    let span = cx.span(name.loc);
    let n = name.name.as_str();
    let valid = n.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && n.chars().all(|c| c.is_alphanumeric() || c == '_')
        && n != "_";
    if !valid || matches!(n, "self" | "Self" | "super" | "crate") {
        diags.error(name.loc, format!("`{n}` can't be a name in Rust"));
        return None;
    }
    Some(if RUST_KEYWORDS.contains(&n) { RustIdent::new_raw(n, span) } else { RustIdent::new(n, span) })
}

/// Rust's strict and reserved keywords (2024 edition) other than those no raw identifier can be.
const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "dyn", "else", "enum", "extern", "false", "fn", "for", "if",
    "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "static", "struct", "trait",
    "true", "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final", "gen", "macro",
    "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
];
