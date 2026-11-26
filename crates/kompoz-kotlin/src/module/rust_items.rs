//! The `pub` Rust items of a crate's file, read with `syn` (see the module's doc).
//!
//! An item is what the API index has of it (see `xtask/src/kotlin_index.rs`, whose reading this follows): a free
//! function, an impl's method or associated function or constant, a `#[derive(KotlinProperties)]` struct's field
//! properties, an enum's variants, a module's `const`/`static`/`thread_local!` values, a type alias; in every
//! inline module too (a private one's `pub` items are its parent's to use, as a private file module's). A type (a
//! struct, an enum, the type a `#[named(type)]` function makes) is one more item of kind [`Kind::Value`] whose
//! owner, Rust and Kotlin names are all its name (see [`RustItem`]).

use super::{RustItem, RustParam};
use crate::index::{Kind, Receiver, flags};
use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use quote::ToTokens;
use syn::parse::{ParseStream, Parser};
use syn::{Attribute, Fields, FnArg, ImplItem, Item, ReturnType, Signature, Type, Visibility};

/// The `pub` items of a Rust file's text, in the module `module` (`::`-separated from the crate's root). A text
/// `syn` can't read gives none.
pub fn rust_items(text: &str, module: &str) -> Vec<RustItem> {
    let Ok(file) = syn::parse_file(text) else { return Vec::new() };
    let mut out = Vec::new();
    collect(&file.items, module, &mut out);
    out
}

/// The items of one module's body (a file's, an inline `mod`'s).
fn collect(items: &[Item], module: &str, out: &mut Vec<RustItem>) {
    for item in items {
        match item {
            Item::Fn(f) if visible(&f.vis) => {
                let named = named_attr(&f.attrs);
                out.push(function(&f.attrs, &f.sig, "", "", module, named.is_some()));
                // (`#[named(type)]`: the function makes the type of its name, its builder an associated function
                // of it)
                if named.is_some_and(|n| n == Named::OwnType) {
                    out.push(type_item(&unraw(&f.sig.ident), module));
                }
            }
            Item::Struct(s) if visible(&s.vis) => {
                let owner = unraw(&s.ident);
                out.push(type_item(&owner, module));
                properties(s, &owner, module, out);
            }
            Item::Enum(e) if visible(&e.vis) => {
                let owner = unraw(&e.ident);
                out.push(type_item(&owner, module));
                for v in &e.variants {
                    let name = unraw(&v.ident);
                    let mut it = base(&name, &name, Kind::Variant, &owner, module);
                    it.ret = owner.clone();
                    out.push(it);
                }
            }
            Item::Impl(imp) if imp.trait_.is_none() || named_attr(&imp.attrs).is_some() => {
                impl_block(imp, module, out);
            }
            // (stage 14 WPA) A trait: its methods (reached through it), and, when its doc line says ``Kotlin: `open
            // class X` `` (or `abstract class`), a base class Kotlin's classes derive from.
            Item::Trait(t) if visible(&t.vis) => {
                let via = unraw(&t.ident);
                for ti in &t.items {
                    if let syn::TraitItem::Fn(f) = ti {
                        out.push(function(&f.attrs, &f.sig, "", &via, module, false));
                    }
                }
                if let Some(doc) = kotlin_doc_line(&t.attrs) {
                    let open = doc.starts_with("open class ");
                    if let Some(name) = doc.strip_prefix("abstract class ").or_else(|| doc.strip_prefix("open class "))
                    {
                        let name = name.split(['(', '<', ' ']).next().unwrap_or(name);
                        let mut it = base(name, &via, Kind::Value, &via, module);
                        it.flags = 0;
                        let required = t
                            .items
                            .iter()
                            .filter_map(|i| match i {
                                syn::TraitItem::Fn(f) if f.default.is_none() => Some(unraw(&f.sig.ident)),
                                _ => None,
                            })
                            .collect();
                        it.base = Some((open, required));
                        out.push(it);
                    }
                }
            }
            Item::Const(c) if visible(&c.vis) => out.push(value(&unraw(&c.ident), &c.ty, module)),
            Item::Static(s) if visible(&s.vis) => out.push(value(&unraw(&s.ident), &s.ty, module)),
            Item::Type(t) if visible(&t.vis) => {
                let name = unraw(&t.ident);
                let mut it = base(&name, &name, Kind::Value, "", module);
                it.alias = Some(text(t.ty.to_token_stream()));
                out.push(it);
            }
            // (every inline module: a private one's `pub` items are its parent's to use, as a private file
            // module's are)
            Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    let name = unraw(&m.ident);
                    let path = if module.is_empty() { name } else { format!("{module}::{name}") };
                    collect(inner, &path, out);
                }
            }
            Item::Macro(m)
                if m.ident.is_none() && m.mac.path.segments.last().is_some_and(|s| s.ident == "thread_local") =>
            {
                thread_locals(m.mac.tokens.clone(), module, out);
            }
            _ => {}
        }
    }
}

/// Whether a visibility makes an item seen by the crate's other modules: `pub`, `pub(crate)`, `pub(super)`,
/// `pub(in path)` (not an inherited one, nor `pub(self)`, which is the same).
fn visible(v: &Visibility) -> bool {
    match v {
        Visibility::Public(_) => true,
        Visibility::Restricted(r) => !r.path.is_ident("self"),
        Visibility::Inherited => false,
    }
}

/// An item with its names, kind, owner and module, the rest empty (`REACHABLE`: every item of the crate is reached
/// from its own code).
fn base(kotlin: &str, rust: &str, kind: Kind, owner: &str, module: &str) -> RustItem {
    RustItem {
        kotlin: kotlin.to_string(),
        rust: rust.to_string(),
        kind,
        module: module.to_string(),
        owner: owner.to_string(),
        via: String::new(),
        receiver: Receiver::None,
        params: Vec::new(),
        ret: String::new(),
        flags: flags::REACHABLE,
        thread_local: false,
        alias: None,
        base: None,
    }
}

/// A type's item: kind [`Kind::Value`], its name as owner, Rust and Kotlin name, and as its type; no flags.
fn type_item(name: &str, module: &str) -> RustItem {
    let mut it = base(name, name, Kind::Value, name, module);
    it.ret = name.to_string();
    it.flags = 0;
    it
}

/// A module's `const` or `static`: a value of its type.
fn value(name: &str, ty: &Type, module: &str) -> RustItem {
    let mut it = base(name, name, Kind::Value, "", module);
    it.ret = text(ty.to_token_stream());
    it
}

/// What a `#[named]` attribute says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Named {
    /// `#[named]`, or an impl's `#[named(trait X)]`.
    Plain,
    /// A function's `#[named(type)]`: its builder is an associated function of the type of its name.
    OwnType,
}

fn attr_name(a: &Attribute) -> String {
    a.path().segments.last().map(|s| s.ident.to_string()).unwrap_or_default()
}

fn named_attr(attrs: &[Attribute]) -> Option<Named> {
    let a = attrs.iter().find(|a| attr_name(a) == "named")?;
    match &a.meta {
        syn::Meta::List(l)
            if l.tokens.clone().into_iter().any(|t| matches!(&t, TokenTree::Ident(i) if i == "type")) =>
        {
            Some(Named::OwnType)
        }
        _ => Some(Named::Plain),
    }
}

/// A `#[named(trait Name)]` impl's trait name.
fn named_trait(attrs: &[Attribute]) -> Option<String> {
    let a = attrs.iter().find(|a| attr_name(a) == "named")?;
    let syn::Meta::List(list) = &a.meta else { return None };
    let mut tokens = list.tokens.clone().into_iter().skip_while(|t| !matches!(t, TokenTree::Ident(i) if i == "trait"));
    tokens.next();
    tokens.next().map(|t| t.to_string())
}

/// A parameter's `#[default(expr)]` expression as text (an empty one for a bare `#[default]`).
fn param_default(attrs: &[Attribute]) -> Option<String> {
    let a = attrs.iter().find(|a| a.path().is_ident("default"))?;
    match &a.meta {
        syn::Meta::List(l) => Some(text(l.tokens.clone())),
        _ => Some(String::new()),
    }
}

/// The text of an item's doc line ``Kotlin: `…` ``.
fn kotlin_doc_line(attrs: &[Attribute]) -> Option<String> {
    for a in attrs.iter().filter(|a| a.path().is_ident("doc")) {
        let syn::Meta::NameValue(nv) = &a.meta else { continue };
        let syn::Expr::Lit(l) = &nv.value else { continue };
        let syn::Lit::Str(s) = &l.lit else { continue };
        let line = s.value();
        if let Some(text) = line.trim().strip_prefix("Kotlin: `").and_then(|r| r.strip_suffix('`')) {
            return Some(text.to_string());
        }
    }
    None
}

/// The name a doc line ``Kotlin: `name(…)` `` gives a function (its part before the parameters and any type
/// parameters).
fn kotlin_doc_name(attrs: &[Attribute]) -> Option<String> {
    for a in attrs.iter().filter(|a| a.path().is_ident("doc")) {
        let syn::Meta::NameValue(nv) = &a.meta else { continue };
        let syn::Expr::Lit(l) = &nv.value else { continue };
        let syn::Lit::Str(s) = &l.lit else { continue };
        let line = s.value();
        let Some(sig) = line.trim().strip_prefix("Kotlin: `").and_then(|r| r.strip_suffix('`')) else { continue };
        let Some(open) = sig.find('(') else { continue };
        let name = sig[..open].split('<').next().unwrap_or_default().trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

/// A function or method: a method when it takes `self` (its receiver by how), else a free or associated function.
fn function(attrs: &[Attribute], sig: &Signature, owner: &str, via: &str, module: &str, named: bool) -> RustItem {
    let rust = unraw(&sig.ident);
    let kotlin = kotlin_doc_name(attrs).unwrap_or_else(|| camel(&rust));
    let receiver = match sig.inputs.first() {
        Some(FnArg::Receiver(r)) => match &r.kind {
            syn::ReceiverKind::Reference(_, _, Some(_)) => Receiver::RefMut,
            syn::ReceiverKind::Reference(_, _, None) => Receiver::Ref,
            _ => Receiver::Value,
        },
        _ => Receiver::None,
    };
    let kind = if receiver == Receiver::None { Kind::Function } else { Kind::Method };
    let mut it = base(&kotlin, &rust, kind, owner, module);
    it.via = via.to_string();
    it.receiver = receiver;
    for arg in &sig.inputs {
        let FnArg::Typed(pt) = arg else { continue };
        let name = match &*pt.pat {
            syn::Pat::Ident(pi) => unraw(&pi.ident),
            other => text(other.to_token_stream()),
        };
        it.params.push(RustParam {
            kotlin: camel(&name),
            rust: name,
            ty: text(pt.ty.to_token_stream()),
            default: param_default(&pt.attrs),
        });
    }
    it.ret = match &sig.output {
        ReturnType::Default => "()".to_string(),
        ReturnType::Type(_, t) => text(t.to_token_stream()),
    };
    if named || it.params.iter().any(|p| p.default.is_some()) {
        it.flags |= flags::NAMED;
    }
    if let Some(k) = attrs.iter().find(|a| attr_name(a) == "kompozable") {
        it.flags |= flags::KOMPOZABLE;
        if matches!(&k.meta, syn::Meta::List(l) if l.tokens.clone().into_iter().any(|t| matches!(&t, TokenTree::Ident(i) if i == "inline")))
        {
            it.flags |= flags::INLINE;
        }
    }
    if sig.asyncness.is_some() {
        it.flags |= flags::SUSPEND;
    }
    if sig.constness.is_some() {
        it.flags |= flags::CONST;
    }
    it
}

/// An inherent impl's (or a `#[named]` trait impl's) `pub` functions and constants; a `#[named(trait X)]` impl's
/// methods are all seen (an extension trait's have no visibility of their own) and are reached through `X`.
fn impl_block(imp: &syn::ItemImpl, module: &str, out: &mut Vec<RustItem>) {
    let named = named_attr(&imp.attrs).is_some();
    let via = named_trait(&imp.attrs).unwrap_or_default();
    let owner = type_name(&imp.self_ty);
    for item in &imp.items {
        match item {
            ImplItem::Fn(f) if !via.is_empty() || visible(&f.vis) => {
                out.push(function(&f.attrs, &f.sig, &owner, &via, module, named || named_attr(&f.attrs).is_some()));
            }
            ImplItem::Const(c) if visible(&c.vis) => {
                let name = unraw(&c.ident);
                let mut it = base(&name, &name, Kind::Constant, &owner, module);
                it.ret = text(c.ty.to_token_stream());
                out.push(it);
            }
            _ => {}
        }
    }
}

/// A `#[derive(KotlinProperties)]` struct's `pub` named fields as Kotlin properties, read by reference.
fn properties(s: &syn::ItemStruct, owner: &str, module: &str, out: &mut Vec<RustItem>) {
    let derives = s.attrs.iter().any(|a| {
        a.path().is_ident("derive")
            && matches!(&a.meta, syn::Meta::List(l) if l.tokens.to_string().contains("KotlinProperties"))
    });
    let Fields::Named(fields) = &s.fields else { return };
    if !derives {
        return;
    }
    for f in fields.named.iter().filter(|f| visible(&f.vis)) {
        let Some(ident) = &f.ident else { continue };
        let name = unraw(ident);
        let mut it = base(&camel(&name), &name, Kind::Property, owner, module);
        it.receiver = Receiver::Ref;
        it.ret = text(f.ty.to_token_stream());
        out.push(it);
    }
}

/// A `thread_local!`'s visible statics (`pub static X: T = …;`), each a value read with `.with(…)`.
fn thread_locals(tokens: TokenStream, module: &str, out: &mut Vec<RustItem>) {
    let parser = |input: ParseStream| {
        let mut found = Vec::new();
        while !input.is_empty() {
            Attribute::parse_outer(input)?;
            let vis: Visibility = input.parse()?;
            input.parse::<syn::Token![static]>()?;
            let name: syn::Ident = input.parse()?;
            input.parse::<syn::Token![:]>()?;
            let ty: Type = input.parse()?;
            input.parse::<syn::Token![=]>()?;
            input.parse::<syn::Expr>()?;
            if !input.is_empty() {
                input.parse::<syn::Token![;]>()?;
            }
            found.push((vis, name, ty));
        }
        Ok(found)
    };
    let Ok(found) = parser.parse2(tokens) else { return };
    for (_, name, ty) in found.into_iter().filter(|(vis, ..)| visible(vis)) {
        let mut it = value(&unraw(&name), &ty, module);
        it.thread_local = true;
        out.push(it);
    }
}

fn unraw(i: &syn::Ident) -> String {
    i.to_string().trim_start_matches("r#").to_string()
}

/// A type's name as an owner: its path's last segment, generics left out.
fn type_name(ty: &Type) -> String {
    match ty {
        Type::Path(p) => p.path.segments.last().map(|s| unraw(&s.ident)).unwrap_or_default(),
        Type::Reference(r) => type_name(&r.elem),
        other => text(other.to_token_stream()),
    }
}

/// `snake_case` as Kotlin's `camelCase` (`join_to_string` → `joinToString`, a dimension's `draggable2d` →
/// `draggable2D`); a PascalCase or upper-case name as it is.
pub(crate) fn camel(name: &str) -> String {
    let name = name.trim_start_matches("r#");
    if name.chars().next().is_some_and(|c| c.is_uppercase()) {
        return name.to_string();
    }
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    let mut up = false;
    for (i, &c) in chars.iter().enumerate() {
        let dimension = c == 'd'
            && i > 1
            && chars[i - 1].is_ascii_digit()
            && chars[i - 2].is_ascii_alphabetic()
            && chars.get(i + 1).is_none_or(|n| *n == '_');
        if c == '_' {
            up = !out.is_empty();
        } else if up || dimension {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

// ---- Rust as text ----

/// Tokens as Rust is written, as the API index has them: spaces between words, after commas, around `+`, `->`,
/// `=`, `=>`, none elsewhere (`impl Fn(&mut DrawScope<'_>) + 'static`, `Option<Vec<T>>`).
pub(crate) fn text(ts: TokenStream) -> String {
    let mut out = String::new();
    write_tokens(ts, &mut out);
    out.trim().to_string()
}

fn write_tokens(ts: TokenStream, out: &mut String) {
    // (what the last token was: a word wants a space before the next word)
    let mut last_word = false;
    let mut joint_quote = false;
    let tokens: Vec<TokenTree> = ts.into_iter().collect();
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            TokenTree::Ident(id) => {
                if last_word && !joint_quote {
                    out.push(' ');
                }
                out.push_str(&id.to_string());
                last_word = true;
                joint_quote = false;
            }
            TokenTree::Literal(l) => {
                if last_word {
                    out.push(' ');
                }
                out.push_str(&l.to_string());
                last_word = true;
                joint_quote = false;
            }
            TokenTree::Group(g) => {
                let (open, close) = match g.delimiter() {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::Brace => ("{ ", " }"),
                    Delimiter::None => ("", ""),
                };
                // (a keyword before a bracket is a word of its own: `mut [u8]`, `dyn (Fn())`; not a call's `f(`)
                let keyword = last_word
                    && ["mut", "dyn", "impl", "as", "in", "move", "ref", "const", "static", "where", "for"].iter().any(
                        |k| {
                            out.ends_with(k)
                                && !out[..out.len() - k.len()].ends_with(|c: char| c.is_alphanumeric() || c == '_')
                        },
                    );
                if (g.delimiter() == Delimiter::Brace && !out.is_empty() && !out.ends_with(' ')) || keyword {
                    out.push(' ');
                }
                out.push_str(open);
                write_tokens(g.stream(), out);
                out.push_str(close);
                last_word = g.delimiter() == Delimiter::None;
                joint_quote = false;
            }
            TokenTree::Punct(p) => {
                // (a multi-character operator: its puncts joined)
                let mut op = p.as_char().to_string();
                let mut j = i;
                while matches!(&tokens[j], TokenTree::Punct(q) if q.spacing() == Spacing::Joint)
                    && j + 1 < tokens.len()
                    && matches!(&tokens[j + 1], TokenTree::Punct(_))
                {
                    j += 1;
                    if let TokenTree::Punct(q) = &tokens[j] {
                        op.push(q.as_char());
                    }
                }
                i = j;
                match op.as_str() {
                    "'" => {
                        if last_word {
                            out.push(' ');
                        }
                        out.push('\'');
                        joint_quote = true;
                        last_word = false;
                        i += 1;
                        continue;
                    }
                    "," | ";" => {
                        out.push_str(&op);
                        out.push(' ');
                    }
                    "+" | "->" | "=" | "=>" | "|" | "+=" | "-=" | "==" | "!=" | "&&" | "||" | "<=" | ">=" => {
                        out.push(' ');
                        out.push_str(&op);
                        out.push(' ');
                    }
                    ":" => out.push_str(": "),
                    _ => out.push_str(&op),
                }
                last_word = false;
                joint_quote = false;
            }
        }
        i += 1;
    }
    // (a trailing comma's space, or a doubled one)
    while out.ends_with(' ') && !out.ends_with("{ ") {
        out.pop();
    }
    let fixed = out.replace(", )", ")").replace(", >", ">").replace(", ]", "]").replace("  ", " ");
    *out = fixed;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find<'a>(items: &'a [RustItem], owner: &str, rust: &str, kind: Kind) -> &'a RustItem {
        items
            .iter()
            .find(|i| i.owner == owner && i.rust == rust && i.kind == kind)
            .unwrap_or_else(|| panic!("no {kind:?} {owner}::{rust} in {items:#?}"))
    }

    #[test]
    fn free_functions() {
        let items = rust_items(
            r#"
            #[kompozable]
            pub fn greeting_card(name: impl Into<String>, #[default(16)] size: i32, on_click: impl Fn(i32) -> bool + 'static) {}
            #[kompozable(inline)]
            pub(crate) fn r#box(content: &mut DrawScope) -> Option<Rc<dyn Fn()>> { None }
            pub async fn load_all() -> Vec<String> { Vec::new() }
            /// Does it.
            ///
            /// Kotlin: `joinToString(separator: String = ", ")`
            pub fn join_to_string(separator: &str) -> String { String::new() }
            #[named(type)]
            pub fn TextStyle(font_size: f32) -> TextStyle { todo!() }
            fn private_one() {}
            pub(self) fn private_too() {}
            "#,
            "",
        );
        let g = find(&items, "", "greeting_card", Kind::Function);
        assert_eq!(g.kotlin, "greetingCard");
        assert_eq!(g.ret, "()");
        assert_eq!(g.flags, flags::KOMPOZABLE | flags::NAMED | flags::REACHABLE);
        let tys: Vec<_> = g.params.iter().map(|p| (p.kotlin.as_str(), p.ty.as_str(), p.default.as_deref())).collect();
        assert_eq!(
            tys,
            [
                ("name", "impl Into<String>", None),
                ("size", "i32", Some("16")),
                ("onClick", "impl Fn(i32) -> bool + 'static", None)
            ]
        );
        let b = find(&items, "", "box", Kind::Function);
        assert_eq!(b.flags, flags::KOMPOZABLE | flags::INLINE | flags::REACHABLE);
        assert_eq!(b.params[0].ty, "&mut DrawScope");
        assert_eq!(b.ret, "Option<Rc<dyn Fn()>>");
        let l = find(&items, "", "load_all", Kind::Function);
        assert_eq!(l.flags, flags::SUSPEND | flags::REACHABLE);
        assert_eq!(l.ret, "Vec<String>");
        assert_eq!(find(&items, "", "join_to_string", Kind::Function).kotlin, "joinToString");
        let t = find(&items, "", "TextStyle", Kind::Function);
        assert_eq!(t.flags, flags::NAMED | flags::REACHABLE);
        let ty = find(&items, "TextStyle", "TextStyle", Kind::Value);
        assert_eq!((ty.kotlin.as_str(), ty.ret.as_str(), ty.flags), ("TextStyle", "TextStyle", 0));
        assert!(!items.iter().any(|i| i.rust.starts_with("private")));
    }

    #[test]
    fn structs_impls_and_enums() {
        let items = rust_items(
            r#"
            #[derive(Clone, KotlinProperties)]
            pub struct Place { pub title: String, pub(crate) visit_count: u32, secret: bool }
            pub struct Plain { pub x: i32 }
            impl Place {
                pub const EMPTY: Place = Place::new();
                pub fn new() -> Self { todo!() }
                pub fn title(&self) -> &str { &self.title }
                pub fn set_title(&mut self, title: String) {}
                pub fn into_title(self) -> String { self.title }
                fn hidden(&self) {}
            }
            #[named(trait PlaceExt)]
            impl Place {
                fn describe(&self, #[default(1)] depth: u8) -> String { String::new() }
            }
            impl Clone for Plain { fn clone(&self) -> Self { todo!() } }
            pub enum Tab { Home, Places(u32), Settings { dark: bool } }
            enum Hidden { A }
            "#,
            "screens",
        );
        let p = find(&items, "Place", "Place", Kind::Value);
        assert_eq!(p.module, "screens");
        let t = find(&items, "Place", "title", Kind::Property);
        assert_eq!((t.kotlin.as_str(), t.ret.as_str(), t.receiver), ("title", "String", Receiver::Ref));
        assert_eq!(find(&items, "Place", "visit_count", Kind::Property).kotlin, "visitCount");
        assert!(!items.iter().any(|i| i.rust == "secret" || i.owner == "Plain" && i.kind == Kind::Property));
        find(&items, "Plain", "Plain", Kind::Value);
        assert_eq!(find(&items, "Place", "EMPTY", Kind::Constant).ret, "Place");
        assert_eq!(find(&items, "Place", "new", Kind::Function).ret, "Self");
        assert_eq!(find(&items, "Place", "title", Kind::Method).receiver, Receiver::Ref);
        let s = find(&items, "Place", "set_title", Kind::Method);
        assert_eq!((s.kotlin.as_str(), s.receiver), ("setTitle", Receiver::RefMut));
        assert_eq!(find(&items, "Place", "into_title", Kind::Method).receiver, Receiver::Value);
        assert!(!items.iter().any(|i| i.rust == "hidden" || i.rust == "clone"));
        let d = find(&items, "Place", "describe", Kind::Method);
        assert_eq!(d.via, "PlaceExt");
        assert_eq!(d.flags, flags::NAMED | flags::REACHABLE);
        for v in ["Home", "Places", "Settings"] {
            let it = find(&items, "Tab", v, Kind::Variant);
            assert_eq!((it.kotlin.as_str(), it.ret.as_str()), (v, "Tab"));
        }
        find(&items, "Tab", "Tab", Kind::Value);
        assert!(!items.iter().any(|i| i.owner == "Hidden"));
    }

    #[test]
    fn values_aliases_and_modules() {
        let items = rust_items(
            r#"
            pub const LIMIT: usize = 3;
            pub static NAMES: &[&str] = &[];
            static PRIVATE: i32 = 0;
            thread_local! {
                /// The counter.
                pub static COUNTER: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
                static INNER: Cell<u8> = Cell::new(0);
            }
            std::thread_local!(pub(crate) static OTHER: Cell<bool> = Cell::new(false));
            pub type Callback = Rc<dyn Fn(&str)>;
            pub mod list {
                pub fn row() {}
                pub mod deep { pub const D: u8 = 1; }
                fn private() {}
            }
            mod private_mod { pub fn seen() {} fn unseen() {} }
            "#,
            "app",
        );
        let l = find(&items, "", "LIMIT", Kind::Value);
        assert_eq!((l.ret.as_str(), l.thread_local), ("usize", false));
        assert_eq!(find(&items, "", "NAMES", Kind::Value).ret, "&[&str]");
        let c = find(&items, "", "COUNTER", Kind::Value);
        assert_eq!((c.ret.as_str(), c.thread_local), ("RefCell<Vec<i32>>", true));
        assert!(find(&items, "", "OTHER", Kind::Value).thread_local);
        assert!(!items.iter().any(|i| i.rust == "PRIVATE" || i.rust == "INNER" || i.rust == "unseen"));
        let a = find(&items, "", "Callback", Kind::Value);
        assert_eq!(a.alias.as_deref(), Some("Rc<dyn Fn(&str)>"));
        assert_eq!(find(&items, "", "row", Kind::Function).module, "app::list");
        assert_eq!(find(&items, "", "D", Kind::Value).module, "app::list::deep");
        assert_eq!(find(&items, "", "seen", Kind::Function).module, "app::private_mod");
        assert!(!items.iter().any(|i| i.rust == "private"));
    }

    #[test]
    fn traits_and_base_classes() {
        let items = rust_items(
            r#"
            /// Kotlin: `open class Greeting`
            pub trait Greeting {
                fn hello(&self) -> String { String::new() }
                fn name(&self) -> String;
            }
            pub trait Plain { fn run(&self, n: i32); }
            trait Hidden { fn no(&self); }
            "#,
            "",
        );
        let hello = items.iter().find(|i| i.rust == "hello").expect("a trait's method");
        assert_eq!((hello.via.as_str(), hello.kind, hello.receiver), ("Greeting", Kind::Method, Receiver::Ref));
        let base = items.iter().find(|i| i.base.is_some()).expect("the base class");
        assert_eq!((base.kotlin.as_str(), base.rust.as_str()), ("Greeting", "Greeting"));
        assert_eq!(base.base, Some((true, vec!["name".to_string()])));
        assert!(items.iter().any(|i| i.rust == "run" && i.via == "Plain"));
        assert!(!items.iter().any(|i| i.rust == "no"));
        assert_eq!(items.iter().filter(|i| i.base.is_some()).count(), 1);
    }

    #[test]
    fn unreadable_text_gives_nothing() {
        assert!(rust_items("pub fn broken(", "").is_empty());
    }

    #[test]
    fn camel_case() {
        assert_eq!(camel("join_to_string"), "joinToString");
        assert_eq!(camel("r#type"), "type");
        assert_eq!(camel("LaunchedEffect"), "LaunchedEffect");
        assert_eq!(camel("MAX_SIZE"), "MAX_SIZE");
        assert_eq!(camel("draggable2d"), "draggable2D");
    }
}
