//! Kotlin's names as Rust's: functions, parameters and locals in snake_case (`fillMaxSize` → `fill_max_size`),
//! composables, types and constants as they are (`Column`, `Color`, `MAX`), a raw identifier for a Rust keyword
//! (`r#match`), an error for a name Rust can't have.

use proc_macro2::{Ident, Span};

/// Rust's strict and reserved keywords (2024 edition) that a raw identifier can be.
const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "dyn", "else", "enum", "extern", "false", "fn", "for", "if",
    "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "static", "struct", "trait",
    "true", "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final", "gen", "macro",
    "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
];

/// Whether Rust can name `name` (as written, or raw): letters, digits and `_`, not starting with a digit, not `_`
/// alone, not one of the keywords no raw identifier can be.
pub(crate) fn valid(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_alphanumeric() || c == '_')
        && name != "_"
        && !matches!(name, "self" | "Self" | "super" | "crate")
}

/// `name` as a Rust identifier at `span`: raw for a keyword.
pub(crate) fn ident(name: &str, span: Span) -> Ident {
    if RUST_KEYWORDS.contains(&name) { Ident::new_raw(name, span) } else { Ident::new(name, span) }
}

/// Whether a name is PascalCase (a composable, a type, an object, a constant of a type).
pub(crate) fn is_pascal(name: &str) -> bool {
    name.starts_with(|c: char| c.is_uppercase())
}

/// A Kotlin camelCase name in Rust's snake_case (`fillMaxSize` → `fill_max_size`, `toPx` → `to_px`, `isHTML` →
/// `is_html`, `x2y` → `x2y`); a PascalCase name is kept (`Column`), as is one with an underscore already.
pub(crate) fn snake(name: &str) -> String {
    if is_pascal(name) || name.contains('_') {
        return name.to_string();
    }
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            let after_lower = i > 0 && (chars[i - 1].is_lowercase() || chars[i - 1].is_ascii_digit());
            let ends_acronym =
                i > 0 && chars[i - 1].is_uppercase() && chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            if after_lower || ends_acronym {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// A snake_case or camelCase name in PascalCase (`framed` → `Framed`, `fill_max` → `FillMax`).
pub(crate) fn pascal(name: &str) -> String {
    let mut out = String::new();
    let mut up = true;
    for c in name.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_as_rust_writes_them() {
        assert_eq!(snake("fillMaxSize"), "fill_max_size");
        assert_eq!(snake("toPx"), "to_px");
        assert_eq!(snake("isHTMLText"), "is_html_text");
        assert_eq!(snake("x2y"), "x2y");
        assert_eq!(snake("count"), "count");
        assert_eq!(snake("Column"), "Column");
        assert_eq!(snake("MAX_VALUE"), "MAX_VALUE");
        assert_eq!(snake("onClick"), "on_click");
        assert_eq!(pascal("framed"), "Framed");
        assert_eq!(pascal("fill_max"), "FillMax");
        assert!(valid("ğüş") && !valid("a b") && !valid("_") && !valid("self"));
        assert_eq!(ident("match", Span::call_site()).to_string(), "r#match");
    }
}
