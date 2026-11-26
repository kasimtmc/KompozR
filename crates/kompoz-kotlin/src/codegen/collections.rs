//! (stage 14 LISTS) Kotlin's mutable collections as kt's shared cells: `MutableList<T>`, `MutableSet<T>`,
//! `MutableMap<K, V>` are `kt::MutableList`/`kt::MutableSet`/`kt::MutableMap` (Kotlin's references: a clone is the
//! same object, `===` its identity), the read-only `List<T>`/`Set<T>`/`Map<K, V>` values (`Vec`, `KtSet`, `KtMap`),
//! and a `List<T>` that may be a `MutableList` (a parameter its function asks so of, a `val` declared `List<T>`
//! holding a mutable one) `kt::ListView<T>`, the caller's list itself.
//!
//! What converts where Kotlin's types relate ([`Gen::collection_coerce`]): a mutable collection (a state list, a
//! view) where its read-only kind is taken is a snapshot of it (`toList()`), as reading it there would be; a state list
//! where a `MutableList` is taken is that list (`MutableList::of_state`); a list of any kind where a `ListView` is
//! taken is that list.

use super::Gen;
use super::resolve::Ty;
use crate::ast::*;
use crate::pos::Loc;
use proc_macro2::{Span, TokenStream};
use quote::quote_spanned;

impl Gen<'_, '_> {
    /// A local or property declared `List<T>` (`Collection<T>`, `Iterable<T>`) holding a mutable list (a state list,
    /// a view): Kotlin's one object, read where it is read; its type `ListView<T>` then (none otherwise).
    pub(crate) fn list_view_type(&self, declared: Option<&Type>, init: &PropertyInit) -> Option<Type> {
        let (Some(t), PropertyInit::Value(e)) = (declared, init) else { return None };
        if !matches!(self.ty_of(t), Ty::List(_)) || t.nullable {
            return None;
        }
        let TypeKind::Named(segs) = &t.kind else { return None };
        let [seg] = segs.as_slice() else { return None };
        if !matches!(seg.name.name.as_str(), "List" | "Collection" | "Iterable") {
            return None;
        }
        let v = self.peek_ty(e);
        if !matches!(&v, Ty::Named(n, _) if matches!(n.as_str(), "MutableList" | "ListView" | "SnapshotStateList")) {
            return None;
        }
        let mut viewed = t.clone();
        if let TypeKind::Named(segs) = &mut viewed.kind {
            segs[0].name.name = "ListView".to_string();
        }
        Some(viewed)
    }

    /// `a += b` / `a -= b` of a shared mutable collection (its `val`, a property, an element): Kotlin's
    /// `plusAssign`/`minusAssign`, the collection changed — an element or a collection's elements added (a pair or a
    /// map's entries put), removed (a key's entry).
    pub(crate) fn collection_assign(
        &mut self,
        target: &Expr,
        op: AssignOp,
        value: &Expr,
        loc: Loc,
    ) -> Option<TokenStream> {
        if !matches!(op, AssignOp::Add | AssignOp::Sub) {
            return None;
        }
        let ty = self.peek_ty(target);
        let kind = ty.mutable_collection()?;
        let span = self.span(loc);
        let kt = self.kt(span);
        let vty = self.peek_ty(value);
        let vro = vty.read_only().unwrap_or_else(|| vty.clone());
        let many = match kind {
            "MutableMap" => matches!(vro, Ty::Map(..)),
            _ => {
                matches!(vro, Ty::List(_) | Ty::Set(_) | Ty::Range(_))
                    && vro != ty.read_only().unwrap_or(Ty::Unknown).element()
            }
        };
        let r = super::cast_operand(self.expr(target).ts);
        let v = self.value(value);
        let g = self.fresh("g", span);
        Some(match (kind, op, many) {
            ("MutableMap", AssignOp::Add, true) => quote_spanned!(span=> #r.put_all(#kt::KtMapExt::entries(&#v));),
            ("MutableMap", AssignOp::Add, false) => quote_spanned!(span=> { let #g = #v; #r.put(#g.0, #g.1); }),
            ("MutableMap", _, _) => quote_spanned!(span=> #r.remove(#v);),
            (_, AssignOp::Add, true) => quote_spanned!(span=> #r.add_all(#kt::KtList::to_list(&#v));),
            (_, AssignOp::Add, false) => quote_spanned!(span=> #r.add(#v);),
            (_, _, true) => quote_spanned!(span=> {
                let #g = #kt::KtList::to_list(&#v);
                #r.remove_all(move |e| #g.as_slice().contains(&e));
            }),
            (_, _, false) => quote_spanned!(span=> #r.remove(#v);),
        })
    }

    /// `x is MutableList` / `x as? MutableList<T>` (`as`) of a list: a `List` that may be one asked
    /// (`ListView::as_mutable`), a mutable list (a state list) one, a read-only list (a `Vec`) never; none for other
    /// operands or targets. `is_check`: the check's `Bool`, else the cast's value (`MutableList<T>?` when `safe`).
    pub(crate) fn collection_cast(
        &mut self,
        inner: &Expr,
        safe: bool,
        is_check: bool,
        ty: &Type,
        loc: Loc,
    ) -> Option<super::expr::Out> {
        let TypeKind::Named(segs) = &ty.kind else { return None };
        if !(segs.len() == 1 && matches!(segs[0].name.name.as_str(), "MutableList" | "ArrayList")) {
            return None;
        }
        let from = self.peek_ty(inner);
        let kind = match from.non_null() {
            Ty::List(_) => "List",
            Ty::Named(n, _) if matches!(n.as_str(), "ListView" | "MutableList" | "SnapshotStateList") => {
                if n == "ListView" {
                    "ListView"
                } else if n == "MutableList" {
                    "MutableList"
                } else {
                    "SnapshotStateList"
                }
            }
            _ => return None,
        };
        if from.is_nullable() {
            return None;
        }
        let span = self.span(loc);
        let kt = self.kt(span);
        let elem = from.element();
        let x = self.expr(inner).ts;
        let x = super::cast_operand(x);
        if is_check {
            let ts = match kind {
                "ListView" => quote_spanned!(span=> #x.is_mutable()),
                "List" => quote_spanned!(span=> { let _ = &#x; false }),
                _ => quote_spanned!(span=> { let _ = &#x; true }),
            };
            return Some(super::expr::Out::plain(ts, Ty::Bool));
        }
        let e = self.rust_of_ty(&elem, span);
        let opt = match kind {
            "ListView" => quote_spanned!(span=> #x.as_mutable()),
            "MutableList" => quote_spanned!(span=> ::core::option::Option::Some(::core::clone::Clone::clone(&#x))),
            "SnapshotStateList" => quote_spanned!(span=> ::core::option::Option::Some(
                #kt::MutableList::of_state(::core::clone::Clone::clone(&#x)))),
            _ => quote_spanned!(span=> { let _ = &#x; ::core::option::Option::None::<#kt::MutableList<#e>> }),
        };
        let base = Ty::Named("MutableList".into(), vec![elem]);
        Some(if safe || ty.nullable {
            super::expr::Out::plain(opt, base.nullable())
        } else {
            super::expr::Out::plain(quote_spanned!(span=> #kt::not_null(#opt)), base)
        })
    }

    /// `mapOf(k to v, …)` (`mutableMapOf`, `hashMapOf`, `linkedMapOf`) where a map of `Any` values (or keys) is taken —
    /// `fun f(…): Map<String, Any> = mapOf(KEY to value)` —: each value made what Kotlin's map holds (a function's
    /// too: Kotlin's `Any` holds any value), the pairs written in place; none for other expressions or types.
    pub(crate) fn map_literal(&mut self, e: &Expr, to: &Ty) -> Option<TokenStream> {
        let ExprKind::Call { callee, type_args, args, trailing_lambda: None } = &e.kind else { return None };
        let ExprKind::Name(n) = &callee.kind else { return None };
        let maker = match n.name.as_str() {
            "mapOf" => "map_of",
            "mutableMapOf" => "mutable_map_of",
            "hashMapOf" => "hash_map_of",
            "linkedMapOf" => "linked_map_of",
            _ => return None,
        };
        if !type_args.is_empty() || self.file.function(&n.name).is_some() || self.local(&n.name).is_some() {
            return None;
        }
        let target = to.non_null();
        let (k, v) = match target.read_only().unwrap_or(target) {
            Ty::Map(k, v) => (*k, *v),
            _ => return None,
        };
        let wants = |g: &Self, t: &Ty| super::hierarchy::is_any(&t.non_null()) || g.takes_coerced(t);
        if !wants(self, &k) && !wants(self, &v) {
            return None;
        }
        let pairs: Vec<(&Expr, &Expr)> = args
            .iter()
            .map(|a| match &a.value.kind {
                ExprKind::Infix { name, lhs, rhs } if name.name == "to" && !a.spread && a.name.is_none() => {
                    Some((&**lhs, &**rhs))
                }
                _ => None,
            })
            .collect::<Option<_>>()?;
        let span = self.span(e.loc);
        let kt = self.kt(span);
        let mut made = Vec::new();
        for (ke, ve) in pairs {
            let kv = self.literal_part(ke, &k);
            let vv = self.literal_part(ve, &v);
            made.push(quote_spanned!(span=> #kt::to(#kv, #vv)));
        }
        let f = proc_macro2::Ident::new(maker, span);
        Some(quote_spanned!(span=> #kt::#f([#(#made),*])))
    }

    /// A map literal's key or value as the place's type `to` holds it: Kotlin's `Any` its shared object (a function
    /// value's too), else what [`coerce`](Self::coerce) makes.
    fn literal_part(&mut self, e: &Expr, to: &Ty) -> TokenStream {
        let span = self.span(e.loc);
        let from = self.peek_ty(e);
        let v = self.value(e);
        let any = super::hierarchy::is_any(&to.non_null());
        let shared = matches!(from.non_null(), Ty::Named(ref n, ref a) if n == "Rc"
            && matches!(a.as_slice(), [Ty::Named(t, _)] if t == "KtObject"));
        if any && !from.is_nullable() && !shared && !matches!(from, Ty::Unknown | Ty::Unit) {
            let kt = self.kt(span);
            let f = self.fresh("any", span);
            let made = quote_spanned!(span=> { let #f: ::std::rc::Rc<dyn #kt::KtObject> = ::std::rc::Rc::new(#v); #f });
            return if to.is_nullable() { quote_spanned!(span=> ::core::option::Option::Some(#made)) } else { made };
        }
        if self.coerces(&from, to) { self.coerce(v, &from, to, span) } else { v }
    }

    /// `a === b` of shared mutable collections (or views of lists): the same object; none for other types.
    pub(crate) fn collection_identical(&mut self, same: bool, lhs: &Expr, rhs: &Expr, loc: Loc) -> Option<TokenStream> {
        let (a, b) = (self.peek_ty(lhs), self.peek_ty(rhs));
        let kind = |t: &Ty| match t {
            Ty::Named(n, _) if matches!(n.as_str(), "MutableList" | "MutableSet" | "MutableMap" | "ListView") => {
                Some(n.clone())
            }
            _ => None,
        };
        let (ka, kb) = (kind(&a), kind(&b));
        if ka.is_none() && kb.is_none() {
            return None;
        }
        let span = self.span(loc);
        let ts = if ka == kb {
            let (x, y) = (self.expr(lhs).ts, self.expr(rhs).ts);
            let x = super::cast_operand(x);
            quote_spanned!(span=> #x.same(&#y))
        } else {
            // (a view and the mutable list it is: that list's identity; else two kinds of object never one)
            let view_of = |k: &Option<String>, other: &Option<String>| {
                k.as_deref() == Some("ListView") && other.as_deref() == Some("MutableList")
            };
            if view_of(&ka, &kb) || view_of(&kb, &ka) {
                let (x, y) = (self.expr(lhs).ts, self.expr(rhs).ts);
                let (view, list) = if ka.as_deref() == Some("ListView") { (x, y) } else { (y, x) };
                let view = super::cast_operand(view);
                quote_spanned!(span=> #view.as_mutable().is_some_and(|m| m.same(&#list)))
            } else {
                let (x, y) = (self.expr(lhs).ts, self.expr(rhs).ts);
                quote_spanned!(span=> { let _ = (&#x, &#y); false })
            }
        };
        Some(if same { ts } else { quote_spanned!(span=> !(#ts)) })
    }
    /// Whether a value of `from` goes where `to` is taken through [`collection_coerce`](Self::collection_coerce).
    pub(crate) fn collection_coerces(&self, from: &Ty, to: &Ty) -> bool {
        collection_conversion(&from.non_null(), &to.non_null()).is_some() && from.is_nullable() == to.is_nullable()
    }

    /// A value of `from` where `to` is taken (see the module's doc); `v` back when no conversion is Kotlin's there.
    pub(crate) fn collection_coerce(
        &mut self,
        v: TokenStream,
        from: &Ty,
        to: &Ty,
        span: Span,
    ) -> Result<TokenStream, TokenStream> {
        if from.is_nullable() != to.is_nullable() {
            return Err(v);
        }
        let Some(conv) = collection_conversion(&from.non_null(), &to.non_null()) else { return Err(v) };
        let kt = self.kt(span);
        let one = |x: TokenStream| match conv {
            Conversion::Snapshot("to_vec") => quote_spanned!(span=> #x.to_vec()),
            Conversion::Snapshot("to_set") => quote_spanned!(span=> #x.to_set()),
            Conversion::Snapshot(_) => quote_spanned!(span=> #x.to_map()),
            Conversion::OfState => quote_spanned!(span=> #kt::MutableList::of_state(#x)),
            Conversion::View => quote_spanned!(span=> #kt::ListView::from(#x)),
            Conversion::Into => quote_spanned!(span=> ::core::convert::Into::into(#x)),
        };
        Ok(if from.is_nullable() {
            let x = self.fresh("l", span);
            let made = one(quote_spanned!(span=> #x));
            quote_spanned!(span=> ::core::option::Option::map(#v, |#x| #made))
        } else {
            one(super::cast_operand(v))
        })
    }
}

/// How a collection becomes another kind of Kotlin's that it is.
#[derive(Clone, Copy)]
enum Conversion {
    /// Its elements as they are now, by the method named.
    Snapshot(&'static str),
    /// A state list as the `MutableList` it is.
    OfState,
    /// The list itself, as a `List` that may be mutable.
    View,
    /// A value of a type converting from it (`From`).
    Into,
}

/// Whether a value of `from` converts to `to` as one of Kotlin's collections where another kind it is is taken.
pub(crate) fn converts(from: &Ty, to: &Ty) -> bool {
    collection_conversion(from, to).is_some()
}

fn collection_conversion(from: &Ty, to: &Ty) -> Option<Conversion> {
    let named = |t: &Ty| match t {
        Ty::Named(n, _) => Some(n.clone()),
        _ => None,
    };
    let (f, t) = (named(from), named(to));
    match (from, to) {
        // (a list of any kind where a `List` that may be mutable is taken: the list itself)
        (Ty::List(_), _) if t.as_deref() == Some("ListView") => Some(Conversion::View),
        (_, _)
            if t.as_deref() == Some("ListView")
                && matches!(f.as_deref(), Some("MutableList" | "SnapshotStateList")) =>
        {
            Some(Conversion::View)
        }
        // (a state list where a `MutableList` is taken)
        (_, _) if t.as_deref() == Some("MutableList") && f.as_deref() == Some("SnapshotStateList") => {
            Some(Conversion::OfState)
        }
        // (Kotlin's `Map<String, Any>` where navigation3's entry metadata is taken: what it is, its `From`)
        (Ty::Map(..), _) if t.as_deref() == Some("Metadata") => Some(Conversion::Into),
        // (a mutable one where its read-only kind is taken: a snapshot)
        (_, Ty::List(_)) if matches!(f.as_deref(), Some("MutableList" | "ListView")) => {
            Some(Conversion::Snapshot("to_vec"))
        }
        (_, Ty::List(_)) if f.as_deref() == Some("SnapshotStateList") => Some(Conversion::Snapshot("to_vec")),
        (_, Ty::Set(_)) if f.as_deref() == Some("MutableSet") => Some(Conversion::Snapshot("to_set")),
        (_, Ty::Map(..)) if f.as_deref() == Some("MutableMap") => Some(Conversion::Snapshot("to_map")),
        _ => None,
    }
}

/// Whether tokens name `name` as a value (not a field or method after `.`, nor a path's segment): what the `#[named]`
/// builder lends a default of the earlier parameters (its `earlier_in_default`, the same reading).
pub(crate) fn mentions_ident(ts: &TokenStream, name: &str) -> bool {
    use proc_macro2::{Spacing, TokenTree};
    let tokens: Vec<TokenTree> = ts.clone().into_iter().collect();
    tokens.iter().enumerate().any(|(k, t)| match t {
        TokenTree::Group(g) => mentions_ident(&g.stream(), name),
        TokenTree::Ident(i) if *i == name => {
            let prev = k.checked_sub(1).map(|p| &tokens[p]);
            let after = matches!(prev, Some(TokenTree::Punct(p)) if p.as_char() == '.'
                || (p.as_char() == ':' && k >= 2 && matches!(&tokens[k - 2], TokenTree::Punct(q) if q.as_char() == ':')));
            // (a path's segment, a named argument's name — `f(x = 1)` — or a binding's — `let x = …`)
            let before = matches!(tokens.get(k + 1), Some(TokenTree::Punct(p))
                if p.as_char() == ':' && p.spacing() == Spacing::Joint
                    || p.as_char() == '=' && p.spacing() == Spacing::Alone);
            !after && !before
        }
        _ => false,
    })
}

/// Whether tokens call with a named argument (`f(x = 1)`: a name and a lone `=` in a call's parentheses), which the
/// macros' rewriting of calls (`named_args!`) makes a builder's.
pub(crate) fn named_call_in(ts: &TokenStream) -> bool {
    use proc_macro2::{Delimiter, Spacing, TokenTree};
    ts.clone().into_iter().any(|t| match t {
        TokenTree::Group(g) => {
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            let named = g.delimiter() == Delimiter::Parenthesis
                && inner.windows(2).any(|w| {
                    matches!((&w[0], &w[1]), (TokenTree::Ident(_), TokenTree::Punct(p))
                        if p.as_char() == '=' && p.spacing() == Spacing::Alone)
                });
            named || named_call_in(&g.stream())
        }
        _ => false,
    })
}
