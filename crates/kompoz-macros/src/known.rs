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

/// A function's or method's parameters: what it is, whether each has a default, and whether one is `#[memoized]`.
struct Shape {
    kind: Kind,
    defaults: Vec<bool>,
    memoized: bool,
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
}

impl Known {
    /// The index's items that count in the crate cargo is compiling (`CARGO_PKG_NAME`, `CARGO_CRATE_NAME`).
    pub(crate) fn for_crate() -> Result<Known> {
        let package = std::env::var("CARGO_PKG_NAME").unwrap_or_default();
        let krate = std::env::var("CARGO_CRATE_NAME").unwrap_or_default();
        let mut known = Known::default();
        for e in index()?.iter().filter(|e| e.scope.includes(&package, &krate)) {
            known.add(e.kind, &e.name, e.defaults.clone(), e.has_memoized());
        }
        Ok(known)
    }

    fn add(&mut self, kind: Kind, name: &str, defaults: Vec<bool>, memoized: bool) {
        let map = if kind == Kind::Method { &mut self.methods } else { &mut self.functions };
        map.entry(name.to_string()).or_default().push(Shape { kind, defaults, memoized });
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
            self.0.add(kind, &f.sig.ident.to_string(), defaults(&f.sig), has_memoized(&f.sig));
        }
        visit::visit_item_fn(self, f);
    }
    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        if has_attr(&i.attrs, "named") {
            for item in &i.items {
                if let ImplItem::Fn(m) = item {
                    let (defaults, memoized) = (defaults(&m.sig), has_memoized(&m.sig));
                    if defaults.contains(&true) || memoized {
                        self.0.add(Kind::Method, &m.sig.ident.to_string(), defaults, memoized);
                    }
                }
            }
        }
        visit::visit_item_impl(self, i);
    }
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
