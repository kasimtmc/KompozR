//! What a body's calls are known to call: the API index's items that count in the crate being compiled (see
//! `api_index`), and the `#[named]`/`#[kompozable]` items defined in the body itself.

use crate::api_index::{self, Entry, Kind};
use std::collections::HashMap;
use std::sync::OnceLock;
use syn::visit::{self, Visit};
use syn::*;

/// The index, checked in next to this crate (a change to it rebuilds the macros, so what uses them).
const INDEX: &str = include_str!("../api-index.txt");

fn index() -> Result<&'static [Entry]> {
    static PARSED: OnceLock<std::result::Result<Vec<Entry>, String>> = OnceLock::new();
    match PARSED.get_or_init(|| api_index::parse(INDEX)) {
        Ok(entries) => Ok(entries),
        Err(e) => Err(Error::new(proc_macro2::Span::call_site(), format!("kompoz: {e}"))),
    }
}

/// Whether the index has a method named `name` (of any type, in any crate). The macros rewrite a method call by its
/// name (they don't know the receiver's type), so a method of that name without defaults gets a builder too: a call
/// of it rewritten for another type's method (a scope's `group.weight(modifier, 1.0)` beside foundation's
/// `Modifier.weight(weight, fill = true)`) goes through its own builder.
pub(crate) fn indexed_method(name: &str) -> Result<bool> {
    Ok(index()?.iter().any(|e| e.kind == Kind::Method && e.name == name))
}

/// A function's or method's parameters: what it is, whether each has a default, and whether one is `#[memoized]`.
struct Shape {
    kind: Kind,
    defaults: Vec<bool>,
    memoized: bool,
    /// A method's owner: the type of its impl (`None` for a blanket impl, any type) and the trait of a
    /// `#[named(trait …)]` one.
    owner: Option<(Option<String>, Option<String>)>,
}

impl Shape {
    /// Whether a call of it with `n` positional arguments goes through its builder: they are more than its
    /// parameters without a default (what its plain form takes), or it has a `#[memoized]` parameter (the builder
    /// gives a closure its identity), and they fill every one of those in its builder ([`Shape::fills`]).
    fn rewritten(&self, n: usize, last_closure: bool) -> bool {
        let required = self.defaults.iter().filter(|d| !**d).count();
        (n > required || (self.memoized && n > 0)) && self.fills(n, last_closure)
    }

    /// Whether `n` positional arguments (the last a closure, which goes to the last parameter, if `last_closure`)
    /// fill every parameter without a default in its builder, and are no more than its parameters.
    fn fills(&self, n: usize, last_closure: bool) -> bool {
        let total = self.defaults.len();
        if n > total {
            return false;
        }
        let mut given = vec![false; total];
        let in_order = if last_closure { n - 1 } else { n };
        given[..in_order].iter_mut().for_each(|g| *g = true);
        if last_closure {
            given[total - 1] = true;
        }
        self.defaults.iter().zip(&given).all(|(d, g)| *d || *g)
    }
}

/// The functions and methods with named, defaulted parameters a body's calls may call.
#[derive(Default)]
pub(crate) struct Known {
    functions: HashMap<String, Vec<Shape>>,
    methods: HashMap<String, Vec<Shape>>,
    /// The API's supertypes of each type (the index's `super` lines).
    supers: HashMap<String, Vec<String>>,
}

impl Known {
    /// The index's items that count in the crate cargo is compiling (`CARGO_PKG_NAME`, `CARGO_CRATE_NAME`).
    pub(crate) fn for_crate() -> Result<Known> {
        let package = std::env::var("CARGO_PKG_NAME").unwrap_or_default();
        let krate = std::env::var("CARGO_CRATE_NAME").unwrap_or_default();
        let mut known = Known::default();
        for e in index()?.iter().filter(|e| e.scope.includes(&package, &krate)) {
            if e.kind == Kind::Super {
                known.supers.entry(e.name.clone()).or_default().push(e.owner.clone());
                continue;
            }
            let owner = (e.kind == Kind::Method).then(|| owner_of(&e.owner));
            known.add(e.kind, &e.name, e.defaults.clone(), e.has_memoized(), owner);
        }
        Ok(known)
    }

    fn add(
        &mut self,
        kind: Kind,
        name: &str,
        defaults: Vec<bool>,
        memoized: bool,
        owner: Option<(Option<String>, Option<String>)>,
    ) {
        let map = if kind == Kind::Method { &mut self.methods } else { &mut self.functions };
        map.entry(name.to_string()).or_default().push(Shape { kind, defaults, memoized, owner });
    }

    /// The `#[named(type)]` functions a caller names (`types(Note(rrd), …)`: the Kotlin syntax's classes of another
    /// block or file of the crate, which the API index doesn't list): their builders reached through their types.
    pub(crate) fn add_types(&mut self, types: &[TypeFn]) {
        for t in types {
            self.add(Kind::Fn, &t.name.to_string(), t.defaults.clone(), false, None);
        }
    }

    /// Whether a receiver of a type whose names are `heads` (its type and what it holds as a pointer: see
    /// `Transformer::receiver_heads`) may have the `#[named]` method `name`: one of them, or a supertype of one, is
    /// its owner's type or trait, or the method is a blanket impl's.
    pub(crate) fn may_own(&self, name: &str, heads: &[String]) -> bool {
        let mut all: Vec<String> = heads.to_vec();
        let mut k = 0;
        while k < all.len() {
            if let Some(s) = self.supers.get(&all[k]) {
                for t in s {
                    if !all.contains(t) {
                        all.push(t.clone());
                    }
                }
            }
            k += 1;
        }
        self.methods.get(name).is_none_or(|shapes| {
            shapes.iter().any(|s| match &s.owner {
                None | Some((None, _)) => true,
                Some((Some(ty), tr)) => all.contains(ty) || tr.as_ref().is_some_and(|t| all.contains(t)),
            })
        })
    }

    /// Adds the `#[named]` and `#[kompozable]` functions and `#[named]` impls' methods defined in a body, at any
    /// depth.
    pub(crate) fn add_items_in_block(&mut self, b: &Block) {
        Items(self).visit_block(b);
    }

    /// The same in an expression (`kompozable_lambda!`'s).
    pub(crate) fn add_items_in_expr(&mut self, e: &Expr) {
        Items(self).visit_expr(e);
    }

    /// Whether a call of the function `name` with `n` positional arguments (see `Shape::rewritten`) goes through
    /// its builder.
    pub(crate) fn function_rewritten(&self, name: &str, n: usize, last_closure: bool) -> bool {
        self.functions.get(name).is_some_and(|s| s.iter().any(|s| s.rewritten(n, last_closure)))
    }

    /// The same for a method.
    pub(crate) fn method_rewritten(&self, name: &str, n: usize, last_closure: bool) -> bool {
        self.methods.get(name).is_some_and(|s| s.iter().any(|s| s.rewritten(n, last_closure)))
    }

    /// Whether `name` is a `#[named]` function that isn't a composable (and nothing of its name is one): its
    /// builder is reached through its type (`Name::__kz_builder()`), a composable's through its marker.
    pub(crate) fn is_plain_function(&self, name: &str) -> bool {
        self.functions
            .get(name)
            .is_some_and(|s| s.iter().any(|s| s.kind == Kind::Fn) && !s.iter().any(|s| s.kind == Kind::Kompozable))
    }
}

/// What `add_items_in_*` walks with.
struct Items<'k>(&'k mut Known);

impl<'ast> Visit<'ast> for Items<'_> {
    fn visit_item_fn(&mut self, f: &'ast ItemFn) {
        let kind = if has_attr(&f.attrs, "kompozable") {
            Some(Kind::Kompozable)
        } else if has_attr(&f.attrs, "named") {
            Some(Kind::Fn)
        } else {
            None
        };
        if let Some(kind) = kind {
            self.0.add(kind, &f.sig.ident.to_string(), defaults(&f.sig), has_memoized(&f.sig), None);
        }
        visit::visit_item_fn(self, f);
    }
    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        if has_attr(&i.attrs, "named") {
            for item in &i.items {
                if let ImplItem::Fn(m) = item {
                    let (defaults, memoized) = (defaults(&m.sig), has_memoized(&m.sig));
                    if defaults.contains(&true) || memoized {
                        let generic = i
                            .generics
                            .type_params()
                            .any(|p| matches!(&*i.self_ty, Type::Path(t) if t.path.is_ident(&p.ident)));
                        let ty = (!generic).then(|| self_type_name(&i.self_ty));
                        let tr = named_trait_name(&i.attrs);
                        self.0.add(Kind::Method, &m.sig.ident.to_string(), defaults, memoized, Some((ty, tr)));
                    }
                }
            }
        }
        visit::visit_item_impl(self, i);
    }
}

/// A method's owner from its index line's owner field (`<file> <Type>[ as <Trait>]`, `{T}` a blanket impl's type,
/// `dyn X` a trait object's): the type's name (none for a blanket impl) and the trait.
fn owner_of(field: &str) -> (Option<String>, Option<String>) {
    let rest = field.split_once(' ').map_or("", |(_, r)| r);
    let (ty, tr) = match rest.split_once(" as ") {
        Some((ty, tr)) => (ty, Some(tr.trim().to_string())),
        None => (rest, None),
    };
    let ty = ty.trim();
    let ty = (!ty.starts_with('{')).then(|| ty.rsplit(' ').next().unwrap_or(ty).to_string());
    (ty, tr)
}

/// The name of an impl's type as the index writes it: its last path segment (a trait object's trait).
fn self_type_name(t: &Type) -> String {
    match t {
        Type::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default(),
        Type::Reference(r) => self_type_name(&r.elem),
        Type::TraitObject(o) => o
            .bounds
            .iter()
            .find_map(|b| match b {
                TypeParamBound::Trait(t) => t.path.segments.last().map(|s| s.ident.to_string()),
                _ => None,
            })
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// The trait of `#[named(trait Name)]`.
fn named_trait_name(attrs: &[Attribute]) -> Option<String> {
    let a = attrs.iter().find(|a| a.path().segments.last().is_some_and(|s| s.ident == "named"))?;
    let tokens = a.meta.require_list().ok()?.tokens.to_string();
    let rest = tokens.trim().strip_prefix("trait")?;
    rest.split_whitespace().next().map(|s| s.trim_matches(|c: char| !c.is_alphanumeric() && c != '_').to_string())
}

/// Whether `attrs` hold `#[name]` or `#[name(…)]` (by the path's last segment: `#[kompoz_runtime::named]` too).
fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|a| a.path().segments.last().is_some_and(|s| s.ident == name))
}

/// Whether each parameter (not the receiver) has a `#[default(…)]`.
fn defaults(sig: &Signature) -> Vec<bool> {
    sig.inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Typed(pt) => Some(pt.attrs.iter().any(|a| a.path().is_ident("default"))),
            FnArg::Receiver(_) => None,
        })
        .collect()
}

/// Whether a parameter has `#[memoized]`.
fn has_memoized(sig: &Signature) -> bool {
    sig.inputs.iter().any(|arg| match arg {
        FnArg::Typed(pt) => pt.attrs.iter().any(|a| a.path().is_ident("memoized")),
        FnArg::Receiver(_) => false,
    })
}

/// A `#[named(type)]` function a caller names, and which of its parameters have a default: `Note(rrd)` (`r` one
/// without, `d` one with, as the index's letters; `Note()` none).
pub(crate) struct TypeFn {
    pub name: Ident,
    pub defaults: Vec<bool>,
}

impl syn::parse::Parse for TypeFn {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        let inner;
        parenthesized!(inner in input);
        let defaults = if inner.is_empty() {
            Vec::new()
        } else {
            let letters: Ident = inner.parse()?;
            let text = letters.to_string();
            if !text.chars().all(|c| c == 'r' || c == 'd') {
                return Err(Error::new(letters.span(), "kompoz: a parameter is `r` (no default) or `d` (a default)"));
            }
            text.chars().map(|c| c == 'd').collect()
        };
        Ok(TypeFn { name, defaults })
    }
}

/// `types(A(rd), B())` (in `#[kompozable(…)]`), or the same and a `,` opening `named_args!`, `kompozable_block!`
/// and `kompozable_lambda!`.
pub(crate) fn parse_types(input: syn::parse::ParseStream) -> Result<Vec<TypeFn>> {
    let inner;
    parenthesized!(inner in input);
    Ok(syn::punctuated::Punctuated::<TypeFn, Token![,]>::parse_terminated(&inner)?.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checked_in_index_is_as_the_writer_writes_it() {
        let entries = api_index::parse(INDEX).unwrap();
        assert_eq!(api_index::render(&entries), INDEX);
    }

    #[test]
    fn a_call_overflows_where_its_builder_gets_every_parameter_without_a_default() {
        let shape = |p: &str| Shape {
            kind: Kind::Method,
            defaults: p.chars().map(|c| c.eq_ignore_ascii_case(&'d')).collect(),
            memoized: p.contains(['R', 'D']),
            owner: None,
        };
        // `copy(alpha = …, red = …)`: one or two given in order.
        assert!(shape("dd").rewritten(1, false) && shape("dd").rewritten(2, false));
        assert!(!shape("dd").rewritten(0, false) && !shape("dd").rewritten(3, false));
        // `translate(left = 0, top = 0, block)`: the block given last, a closure; not as the plain call takes it.
        assert!(shape("ddr").rewritten(3, true) && shape("ddr").rewritten(2, true));
        assert!(!shape("ddr").rewritten(2, false) && !shape("ddr").rewritten(1, true));
        // `background(color, shape = …)`.
        assert!(shape("rd").rewritten(2, false) && !shape("rd").rewritten(1, false));
        // `LocalX.provides_computed(|s| …)`: a memoized closure, through the builder (its plain form all it takes).
        assert!(shape("R").rewritten(1, true) && !shape("R").rewritten(0, false) && !shape("R").rewritten(2, true));
        // `Layout(modifier, policy, content)`-like: a memoized parameter left to its default, or given.
        assert!(shape("dDr").rewritten(1, true) && shape("dDr").rewritten(3, true));
        assert!(!shape("dDr").rewritten(2, false));
    }
}
