//! Expressions: names, literals, templates, operators, members, `if`/`when` as values, null handling.
//!
//! An expression is written as it is (`Out`), then made a value where one is taken (`finish`): a local read
//! again is cloned (Kotlin's values are shared references; Rust's move), a string literal is made a `String`
//! (Kotlin's one string type) unless what takes it takes a `&str` too, `null` is `None`.

use super::names::{ident, is_pascal, snake, value_name};
use super::resolve::*;
use super::stmt::Tail;
use super::{Gen, respan};
use crate::ast::*;
use crate::index::{self, Kind};
use proc_macro2::{Literal, Span, TokenStream};
use quote::{quote, quote_spanned};

/// What an expression was written as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutKind {
    Plain,
    /// A local (or `this`) that isn't copied by Rust: a clone of it where a value is taken.
    Local,
    /// A string literal (`&'static str`).
    StrLit,
    /// `null`.
    Null,
    /// An integer literal (which a float operand next to it makes a float one).
    IntLit,
    /// A local holding a `&mut` (see `Local::mut_ref`).
    RefMut,
}

pub(crate) struct Out {
    pub ts: TokenStream,
    pub ty: Ty,
    pub kind: OutKind,
}

impl Out {
    pub fn plain(ts: TokenStream, ty: Ty) -> Out {
        Out { ts, ty, kind: OutKind::Plain }
    }
}

/// What the place an expression's value goes to takes.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Want {
    /// A `&'static str` is taken as it is (`impl Into<String>`, `&str`).
    pub str_ok: bool,
    /// Kotlin's `Any` as the Rust API holds it (a lazy list's key, a slot's id): the value made one.
    pub any: Option<AnyWrap>,
    /// The place is an `Option` (a lambda's result of Kotlin's `T?`: `dragAndDropSource { … }`'s data): a value of a
    /// type known not to be nullable given in `Some`.
    pub some: bool,
    /// The place holds a shared trait object (a lambda's result of Kotlin's interface type: a transition spec's
    /// `Arc<dyn FiniteAnimationSpec<T>>`): a value of a type of its own (`tween(…)`, `snap()`) made one, as a
    /// parameter of that type takes it.
    pub shared: Option<Shared>,
}

/// The pointer a shared trait object is held in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shared {
    Arc,
    Rc,
}

impl Shared {
    /// The pointer of a Rust type's text (`Arc<dyn …>`, `Rc<dyn …>`), if it is a shared trait object.
    pub(crate) fn of_rust(text: &str) -> Option<Shared> {
        let t = super::resolve::plain_type_text(text);
        if t.starts_with("Arc<dyn") {
            Some(Shared::Arc)
        } else if t.starts_with("Rc<dyn") {
            Some(Shared::Rc)
        } else {
            None
        }
    }
}

/// A Rust type holding a value of any type, Kotlin's `Any` (a key compared by its value): `LazyKey::new(v)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnyKind {
    /// A lazy layout's item key (`key = { it.id }`): kompoz-foundation's `LazyKey`.
    LazyKey,
    /// A subcomposition's slot id or a lazy item's content type: kompoz-ui's `SlotKey`.
    SlotKey,
}

/// How a value goes to a place of Kotlin's `Any`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AnyWrap {
    pub kind: AnyKind,
    /// The place is an `Option` (Kotlin's `Any?`, a lambda's result): a value is given in `Some`, a nullable one
    /// mapped, `null` as `None`.
    pub optional: bool,
}

impl AnyWrap {
    /// The wrap a Rust type (an index's text) of Kotlin's `Any` takes: `LazyKey`, `Option<SlotKey>`; none for others.
    pub(crate) fn of_rust(text: &str) -> Option<AnyWrap> {
        let t = super::resolve::plain_type_text(text);
        let (optional, inner) = match t.strip_prefix("Option<").and_then(|r| r.strip_suffix('>')) {
            Some(inner) => (true, inner.to_string()),
            None => (false, t),
        };
        let kind = match inner.rsplit("::").next().unwrap_or("") {
            "LazyKey" => AnyKind::LazyKey,
            "SlotKey" => AnyKind::SlotKey,
            _ => return None,
        };
        Some(AnyWrap { kind, optional })
    }

    fn type_name(self) -> &'static str {
        match self.kind {
            AnyKind::LazyKey => "LazyKey",
            AnyKind::SlotKey => "SlotKey",
        }
    }
}

impl Gen<'_, '_> {
    /// An expression made a value (see the module's doc).
    pub(crate) fn value(&mut self, e: &Expr) -> TokenStream {
        let out = self.expr(e);
        self.finish(out, Want::default(), e.loc)
    }

    pub(crate) fn value_want(&mut self, e: &Expr, want: Want) -> TokenStream {
        // (Kotlin's `Any` from an `if`/`when` with branches of their own types — `if (it == 0) null else "satır"` —:
        // each branch made it; a nullable result too — `if (…) content else content.consume { … }` —: each branch
        // in `Some` unless it is nullable already)
        if want.any.is_some() || want.some || want.shared.is_some() {
            match &e.kind {
                ExprKind::If { condition, then, otherwise: Some(o) } => {
                    return self.if_expr(condition, then, Some(o), Tail::Value(want), e.loc).ts;
                }
                ExprKind::When(w) if w.branches.iter().any(|b| b.is_else()) => {
                    return self.when(w, Tail::Value(want), e.loc).ts;
                }
                ExprKind::Paren(inner) => return self.value_want(inner, want),
                _ => {}
            }
        }
        let out = self.expr(e);
        self.finish(out, want, e.loc)
    }

    pub(crate) fn finish(&self, out: Out, want: Want, loc: Loc) -> TokenStream {
        let span = self.span(loc);
        let ts = out.ts;
        let kind = out.kind;
        let ts = match kind {
            OutKind::Local => quote_spanned!(span=> #ts.clone()),
            OutKind::RefMut => quote_spanned!(span=> (*#ts).clone()),
            OutKind::StrLit if !want.str_ok || want.any.is_some() => {
                quote_spanned!(span=> ::std::string::String::from(#ts))
            }
            _ => ts,
        };
        // (a value of its own type where a shared trait object goes: made one)
        let ts = match (want.shared, &out.ty) {
            (Some(ptr), Ty::Named(name, _))
                if !matches!(name.as_str(), "Arc" | "Rc") && super::resolve::trait_object(name).is_none() =>
            {
                match ptr {
                    Shared::Arc => quote_spanned!(span=> ::std::sync::Arc::new(#ts)),
                    Shared::Rc => quote_spanned!(span=> ::std::rc::Rc::new(#ts)),
                }
            }
            _ => ts,
        };
        match want.any {
            Some(w) => self.any_value(ts, &out.ty, kind, w, span),
            None if want.some
                && kind != OutKind::Null
                && !out.ty.is_nullable()
                && out.ty != Ty::Unknown
                && out.ty != Ty::Unit =>
            {
                quote_spanned!(span=> ::core::option::Option::Some(#ts))
            }
            None => ts,
        }
    }

    /// A value made Kotlin's `Any` as the Rust API holds it (see [`AnyWrap`]): `LazyKey::new(v)`, in `Some` for an
    /// optional place; a nullable value mapped, `null` left `None`; a value of the type already as it is.
    fn any_value(&self, ts: TokenStream, ty: &Ty, kind: OutKind, w: AnyWrap, span: Span) -> TokenStream {
        if kind == OutKind::Null {
            return ts;
        }
        let name = w.type_name();
        let already = |t: &Ty| matches!(t, Ty::Named(n, _) if n == name);
        let path = match w.kind {
            AnyKind::LazyKey => {
                let f = respan(self.cx.paths.foundation.clone(), span);
                quote_spanned!(span=> #f::lazy::layout::LazyKey)
            }
            AnyKind::SlotKey => {
                let ui = respan(self.cx.paths.ui.clone(), span);
                quote_spanned!(span=> #ui::layout::SlotKey)
            }
        };
        match ty {
            Ty::Nullable(inner) if already(inner) => ts,
            Ty::Nullable(_) => quote_spanned!(span=> ::core::option::Option::map(#ts, #path::new)),
            t if already(t) => {
                if w.optional {
                    quote_spanned!(span=> ::core::option::Option::Some(#ts))
                } else {
                    ts
                }
            }
            _ => {
                let v = quote_spanned!(span=> #path::new(#ts));
                if w.optional { quote_spanned!(span=> ::core::option::Option::Some(#v)) } else { v }
            }
        }
    }

    /// An expression as written.
    pub(crate) fn expr(&mut self, e: &Expr) -> Out {
        let span = self.span(e.loc);
        match &e.kind {
            ExprKind::Name(n) => self.name(n, e.loc),
            ExprKind::Number(n) => self.number(n, e.loc, false),
            ExprKind::String(s) => self.string(s, e.loc),
            ExprKind::Char(c) => {
                let mut l = Literal::character(*c);
                l.set_span(span);
                Out::plain(quote!(#l), Ty::Char)
            }
            ExprKind::Bool(b) => {
                let ts = if *b { quote_spanned!(span=> true) } else { quote_spanned!(span=> false) };
                Out::plain(ts, Ty::Bool)
            }
            ExprKind::Null => Out {
                ts: quote_spanned!(span=> ::core::option::Option::None),
                ty: Ty::Nullable(Box::new(Ty::Unknown)),
                kind: OutKind::Null,
            },
            ExprKind::This { label } => match self.receiver(label.as_ref().map(|l| l.name.as_str())) {
                Some(r) => {
                    let i = proc_macro2::Ident::new(&r.rust, span);
                    let ty = r.value.clone();
                    // (`self`, or a receiver lent by reference — `this@LookaheadScope` of an `&Rc<dyn LookaheadScope>`
                    // —: a clone where a value is taken, as Kotlin's reference is)
                    let kind =
                        if (r.rust == "self" || r.by_ref) && !ty.is_copy() { OutKind::Local } else { OutKind::Plain };
                    Out { ts: quote!(#i), ty, kind }
                }
                None => {
                    self.error(
                        e.loc,
                        "`this` here has nothing to be (no extension function or receiver lambda around it)",
                    );
                    Out::plain(quote_spanned!(span=> ::core::unreachable!()), Ty::Unknown)
                }
            },
            ExprKind::Super { .. } => Out::plain(self.later(e.loc, "`super`"), Ty::Unknown),
            ExprKind::Call { callee, type_args, args, trailing_lambda } => {
                let mut o = self.call(e, callee, type_args, args, trailing_lambda.as_deref());
                // (what the call's own reading of its type leaves unknown, the expression's may tell)
                if o.ty.is_vague() {
                    let p = self.peek_ty(e);
                    if p != Ty::Unknown {
                        o.ty = p;
                    }
                }
                o
            }
            ExprKind::Member { receiver, safe, name } => self.member(receiver, *safe, name, e.loc),
            ExprKind::Index { receiver, indices } => self.index(receiver, indices, e.loc),
            ExprKind::CallableRef { .. } => {
                Out::plain(self.later(e.loc, "a callable reference (`::name`)"), Ty::Unknown)
            }
            ExprKind::Lambda(l) => {
                let ts = self.lambda(l, None, None);
                Out::plain(ts, Ty::Unknown)
            }
            ExprKind::AnonymousFunction(_) => Out::plain(self.later(e.loc, "an anonymous function"), Ty::Unknown),
            ExprKind::Object { .. } => Out::plain(self.later(e.loc, "an object expression"), Ty::Unknown),
            ExprKind::Prefix { op, operand } => self.prefix(*op, operand, e.loc),
            ExprKind::Postfix { op, operand } => self.postfix(*op, operand, e.loc),
            ExprKind::Binary { op, lhs, rhs } => self.binary(*op, lhs, rhs, e.loc),
            ExprKind::Infix { name, lhs, rhs } => self.infix(name, lhs, rhs, e.loc),
            ExprKind::Is { .. } => Out::plain(self.later(e.loc, "an `is` check"), Ty::Bool),
            ExprKind::As { .. } => Out::plain(self.later(e.loc, "an `as` cast"), Ty::Unknown),
            ExprKind::If { condition, then, otherwise } => {
                self.if_expr(condition, then, otherwise.as_ref(), Tail::Value(Want::default()), e.loc)
            }
            ExprKind::When(w) => self.when(w, Tail::Value(Want::default()), e.loc),
            ExprKind::Try(_) => Out::plain(self.later(e.loc, "`try`"), Ty::Unknown),
            ExprKind::Paren(inner) => {
                let o = self.expr(inner);
                let ts = o.ts;
                Out { ts: quote_spanned!(span=> (#ts)), ty: o.ty, kind: o.kind }
            }
            ExprKind::Return { .. } | ExprKind::Break { .. } | ExprKind::Continue { .. } => {
                Out::plain(self.jump(e), Ty::Unknown)
            }
            ExprKind::Throw(_) => Out::plain(self.later(e.loc, "`throw`"), Ty::Unknown),
            ExprKind::Annotated { expr, .. } => self.expr(expr),
            // (the parser said what is wrong)
            ExprKind::Error => Out::plain(quote_spanned!(span=> ::core::unreachable!()), Ty::Unknown),
        }
    }

    // ---- Names -------------------------------------------------------------------------------------------------

    fn name(&mut self, n: &Ident, loc: Loc) -> Out {
        let span = self.span(loc);
        let kt = self.kt(span);
        if let Some(l) = self.local(&n.name) {
            let i = ident(&l.rust, span);
            if l.kind == LocalKind::Delegated {
                return Out::plain(quote_spanned!(span=> #kt::KtDelegate::get_value(&#i)), l.ty);
            }
            let copied = l.ty.is_copy() || (matches!(l.ty, Ty::Fun(_)) && !l.kept_closure);
            let kind = if l.mut_ref {
                OutKind::RefMut
            } else if copied {
                OutKind::Plain
            } else {
                OutKind::Local
            };
            return Out { ts: quote!(#i), ty: l.ty, kind };
        }
        // A member of an implicit receiver (a property: its getter).
        if let Some((recv, item)) = self.receiver_member(&n.name, |i| i.params.is_empty()) {
            let r = proc_macro2::Ident::new(&recv.rust, span);
            let m = ident(item.rust, span);
            return Out::plain(quote_spanned!(span=> #r.#m()), owner_arg_ret(&recv.value, item.ret));
        }
        if let Some(v) = self.file.vals.get(&n.name).cloned() {
            let i = ident(&v.rust, span);
            return match v.form {
                ValForm::Const | ValForm::Static => Out::plain(quote!(#i), v.ty),
                ValForm::StrConst => Out { ts: quote!(#i), ty: Ty::String, kind: OutKind::StrLit },
                ValForm::ThreadLocal => Out::plain(quote_spanned!(span=> #i.with(::core::clone::Clone::clone)), v.ty),
            };
        }
        if n.name == "Unit" {
            return Out::plain(quote_spanned!(span=> ()), Ty::Unit);
        }
        // kotlin.math's constants (`PI`).
        if let Some(item) = lookup(&n.name)
            .iter()
            .find(|i| i.kind == Kind::Value && i.krate == "kompoz_runtime" && i.module.starts_with("kt"))
        {
            let i = ident(item.rust, span);
            return Out::plain(quote_spanned!(span=> #kt::#i), Ty::from_rust(item.ret));
        }
        let value = lookup(&n.name).iter().find(|i| i.kind == Kind::Value && i.owner.is_empty());
        let i = self.name_ident(&value_name(&n.name), loc);
        match value {
            // (the API's value objects and constants: `Modifier`, `LocalDensity`, `RectangleShape`)
            Some(item) => Out::plain(quote!(#i), Ty::from_rust(item.ret)),
            // A value whose declaration isn't seen here: a top-level `val` of another `kompoz!` or `.kt` file (a
            // thread-local), or Rust's own; read as Kotlin reads it.
            None => Out::plain(quote_spanned!(span=> #kt::top_level!(#i)), Ty::Unknown),
        }
    }

    /// The innermost implicit receiver with a member `kotlin` (that `accept` takes), and the member.
    pub(crate) fn receiver_member(
        &mut self,
        kotlin: &str,
        accept: impl Fn(&crate::index::Item) -> bool,
    ) -> Option<(ImplicitReceiver, &'static crate::index::Item)> {
        let found = self
            .receivers
            .iter()
            .rev()
            .find_map(|r| members_of(&r.ty, kotlin).find(|i| accept(i)).map(|i| (r.clone(), i)))?;
        // (a use: captured from a lambda)
        let used = self.use_receiver(&found.0);
        Some((used, found.1))
    }

    // ---- Literals ----------------------------------------------------------------------------------------------

    /// A number as Kotlin types it: an `Int` unsuffixed (Rust infers it, `i32` by default), `1L` → `1_i64`, `1f` →
    /// `1_f32`, `1.5` a `f64`; `as_float`: an `Int` literal written as a float (`2` next to a `Float`).
    pub(crate) fn number(&mut self, n: &Number, loc: Loc, as_float: bool) -> Out {
        let span = self.span(loc);
        let digits = n.digits();
        let prefix = match n.radix {
            16 => "0x",
            2 => "0b",
            _ => "",
        };
        let (text, ty) = match n.kind {
            NumberKind::Int => {
                let fits = u64::from_str_radix(&digits, n.radix).is_ok_and(|v| v <= i32::MAX as u64);
                if as_float && n.radix == 10 {
                    (format!("{digits}.0"), Ty::Float)
                } else if fits {
                    (format!("{prefix}{digits}"), Ty::Int)
                } else {
                    (format!("{prefix}{digits}_i64"), Ty::Long)
                }
            }
            NumberKind::Long => (format!("{prefix}{digits}_i64"), Ty::Long),
            NumberKind::UInt => (format!("{prefix}{digits}_u32"), Ty::OtherInt),
            NumberKind::ULong => (format!("{prefix}{digits}_u64"), Ty::OtherInt),
            NumberKind::Float => {
                let d = if digits.starts_with('.') { format!("0{digits}") } else { digits.clone() };
                (format!("{d}_f32"), Ty::Float)
            }
            NumberKind::Double => {
                let d = if digits.starts_with('.') { format!("0{digits}") } else { digits.clone() };
                let d = if d.contains(['.', 'e', 'E']) { d } else { format!("{d}.0") };
                (d, Ty::Double)
            }
        };
        let ts: TokenStream = match text.parse() {
            Ok(ts) => respan(ts, span),
            Err(_) => {
                self.error(loc, format!("`{}` isn't a number Rust can write", n.text));
                quote_spanned!(span=> 0)
            }
        };
        let kind = if n.kind == NumberKind::Int && !as_float { OutKind::IntLit } else { OutKind::Plain };
        Out { ts, ty, kind }
    }

    fn string(&mut self, s: &StringLit, loc: Loc) -> Out {
        let span = self.span(loc);
        if s.parts.iter().all(|p| matches!(p, StringPart::Text { .. })) {
            let text: String = s
                .parts
                .iter()
                .map(|p| match p {
                    StringPart::Text { text, .. } => text.as_str(),
                    StringPart::Expr { .. } => "",
                })
                .collect();
            let mut l = Literal::string(&text);
            l.set_span(span);
            return Out { ts: quote!(#l), ty: Ty::String, kind: OutKind::StrLit };
        }
        let mut pieces = Vec::new();
        for p in &s.parts {
            match p {
                StringPart::Text { text, loc } => {
                    let mut l = Literal::string(text);
                    l.set_span(self.span(*loc));
                    pieces.push(quote!(#l));
                }
                StringPart::Expr { expr, .. } => pieces.push(self.expr(expr).ts),
            }
        }
        let kt = self.kt(span);
        Out::plain(quote_spanned!(span=> #kt::template!(#(#pieces),*)), Ty::String)
    }

    // ---- Members -----------------------------------------------------------------------------------------------

    /// Whether `e` names a type or an object (a PascalCase name that isn't a value here), and its name.
    pub(crate) fn qualifier(&self, e: &Expr) -> Option<String> {
        match &e.kind {
            ExprKind::Name(n)
                if is_pascal(&n.name)
                    && !self.is_local(&n.name)
                    && !self.file.vals.contains_key(&n.name)
                    && !self.receivers.iter().any(|r| members_of(&r.ty, &n.name).next().is_some()) =>
            {
                Some(n.name.clone())
            }
            ExprKind::Member { receiver, safe: false, name } if name.name == "Companion" => self.qualifier(receiver),
            // (a nested object Rust names as a type of its own: `Arrangement.Absolute` → `AbsoluteArrangement`)
            ExprKind::Member { receiver, safe: false, name } if is_pascal(&name.name) => {
                let outer = self.qualifier(receiver)?;
                crate::index::rust_type(&format!("{outer}.{}", name.name)).map(str::to_string)
            }
            _ => None,
        }
    }

    /// `a.b` read: a type's constant or entry (`Color.Red` → `Color::Red`), an object's member, a value's property
    /// (`size.width` → `size.width()`), a kt shim's (`s.length` → `KtStr::length(&s)`); `a?.b` through `safe!`.
    fn member(&mut self, receiver: &Expr, safe: bool, name: &Ident, loc: Loc) -> Out {
        let span = self.span(loc);
        if let Some(q) = self.qualifier(receiver) {
            return self.qualified(&q, receiver, name, loc);
        }
        // `16.dp`, `1.5f.sp`
        if let ExprKind::Number(_) = &receiver.kind
            && matches!(name.name.as_str(), "dp" | "sp" | "em")
        {
            let n = self.expr(receiver).ts;
            let m = ident(&name.name, self.span(name.loc));
            let ty = Ty::named(if name.name == "dp" { "Dp" } else { "TextUnit" });
            return Out::plain(quote_spanned!(span=> #n.#m()), ty);
        }
        let recv = self.expr(receiver);
        if safe {
            let it = self.fresh("it", span);
            let inner_ty = recv.ty.non_null();
            let value = self.finish(recv, Want::default(), receiver.loc);
            let read = self.property_read(Out::plain(quote!(#it), inner_ty), name, loc);
            let kt = self.kt(span);
            let ts = read.ts;
            return Out::plain(quote_spanned!(span=> #kt::safe!(#value, |#it| #ts)), read.ty.nullable());
        }
        self.property_read(recv, name, loc)
    }

    /// `recv.name` read, `recv` written.
    pub(crate) fn property_read(&mut self, recv: Out, name: &Ident, loc: Loc) -> Out {
        let span = self.span(loc);
        let mspan = self.span(name.loc);
        // (kt shims' members: through their trait, by the receiver's type)
        let traits = recv.ty.kt_traits();
        let candidates: Vec<_> = kt_members(&name.name, traits).into_iter().filter(|i| i.params.is_empty()).collect();
        let pick = if recv.ty == Ty::Unknown {
            let all: Vec<_> = kt_members(&name.name, &[]).into_iter().filter(|i| i.params.is_empty()).collect();
            let same_trait = all.first().is_some_and(|f| all.iter().all(|i| i.via == f.via));
            if same_trait && recv_has_no_member(&name.name) { all.first().copied() } else { None }
        } else {
            candidates.first().copied()
        };
        if let Some(item) = pick
            && recv.ty.owner().is_none_or(|o| members_of(o, &name.name).next().is_none())
        {
            let kt = self.kt(span);
            let t = proc_macro2::Ident::new(item.via, mspan);
            let m = ident(item.rust, mspan);
            let arg = match (item.receiver, recv.kind) {
                (crate::index::Receiver::Value, _) => self.finish(recv, Want::default(), loc),
                // (a `&mut` held: reborrowed)
                (crate::index::Receiver::RefMut, OutKind::RefMut) => {
                    let r = recv.ts;
                    quote_spanned!(span=> &mut *#r)
                }
                (_, OutKind::RefMut) => {
                    let r = recv.ts;
                    quote_spanned!(span=> &*#r)
                }
                (crate::index::Receiver::RefMut, _) => {
                    let r = recv.ts;
                    quote_spanned!(span=> &mut #r)
                }
                _ => {
                    let r = recv.ts;
                    quote_spanned!(span=> &#r)
                }
            };
            return Out::plain(quote_spanned!(span=> #kt::#t::#m(#arg)), Ty::from_rust(item.ret));
        }
        // A member extension property of an implicit receiver (`LocalX.currentValue` in a `providesComputed { }`), the
        // value's type having no member of that name.
        if recv.ty.owner().is_some_and(|o| members_of(o, &name.name).next().is_none())
            && let Some((r, item)) = self.extension_property(&recv.ty, &name.name)
        {
            return self.extension_property_read(recv, &r, item, loc);
        }
        let (rust, ty, by_value) =
            match recv.ty.owner().and_then(|o| members_of(o, &name.name).find(|i| i.params.is_empty())) {
                Some(item) => (
                    item.rust.to_string(),
                    owner_arg_ret(&recv.ty, item.ret),
                    item.receiver == crate::index::Receiver::Value,
                ),
                None => (snake(&name.name), Ty::Unknown, false),
            };
        let r = if by_value && recv.kind == OutKind::Local {
            let ts = recv.ts;
            quote_spanned!(span=> ::core::clone::Clone::clone(&#ts))
        } else {
            recv.ts
        };
        let m = ident(&rust, mspan);
        Out::plain(quote_spanned!(span=> #r.#m()), ty)
    }

    /// A member extension property `item` of the implicit receiver `r` read of `recv`: `r.item(&recv)`.
    fn extension_property_read(&mut self, recv: Out, r: &ImplicitReceiver, item: &index::Item, loc: Loc) -> Out {
        let span = self.span(loc);
        let ri = proc_macro2::Ident::new(&r.rust, span);
        let m = ident(item.rust, span);
        let ty = owner_arg_ret(&recv.ty, item.ret);
        let arg = if item.params[0].ty.trim_start().starts_with('&') {
            let v = recv.ts;
            quote_spanned!(span=> &#v)
        } else {
            self.finish(recv, Want::default(), loc)
        };
        Out::plain(quote_spanned!(span=> #ri.#m(#arg)), ty)
    }

    /// `Q.name` where `Q` is a type or an object.
    fn qualified(&mut self, q: &str, qe: &Expr, name: &Ident, loc: Loc) -> Out {
        let span = self.span(loc);
        let qspan = self.span(qe.loc);
        if name.name == "Companion" {
            let i = self.name_ident(q, qe.loc);
            return Out::plain(quote!(#i), Ty::named(q));
        }
        if let Some((ts, ty)) = number_constant(q, &name.name, span) {
            return Out::plain(ts, ty);
        }
        let qi = self.name_ident(q, qe.loc);
        let qi = quote_spanned!(qspan=> #qi);
        // A value object's member: a method (`LocalDensity.current`).
        if is_value_object(q) {
            let mut value_ty = Ty::Unknown;
            let m = members_of(q, &name.name).next().or_else(|| {
                lookup(q).iter().find(|i| i.kind == Kind::Value).and_then(|v| {
                    value_ty = Ty::from_rust(v.ret);
                    value_ty.owner().and_then(|o| members_of(o, &name.name).next())
                })
            });
            // (a member extension property of a receiver: `LocalDensity.currentValue` in a `providesComputed { }`)
            if m.is_none()
                && let Some((r, item)) = self.extension_property(&value_ty, &name.name)
            {
                return self.extension_property_read(Out::plain(qi, value_ty), &r, item, loc);
            }
            let rust = m.map(|i| i.rust.to_string()).unwrap_or_else(|| snake(&name.name));
            // (a generic value's member giving its type argument — `LocalClipboard.current`, a
            // `KompozitionLocal<Clipboard>`'s `T` — is of that type: its methods are known)
            let ty = match (m, &value_ty) {
                (Some(i), Ty::Named(_, a)) if a.len() == 1 && super::call::is_type_parameter(i.ret) => a[0].clone(),
                _ => m.map(|i| Ty::from_rust(i.ret)).unwrap_or(Ty::Unknown),
            };
            let mi = ident(&rust, self.span(name.loc));
            return Out::plain(quote_spanned!(span=> #qi.#mi()), ty);
        }
        let assoc = associated(q, &name.name);
        if let Some(item) = assoc.iter().find(|i| matches!(i.kind, Kind::Constant | Kind::Variant | Kind::Value)) {
            let mi = ident(item.rust, self.span(name.loc));
            return Out::plain(quote_spanned!(span=> #qi::#mi), Ty::from_rust(item.ret));
        }
        if let Some(item) = assoc.iter().find(|i| i.kind == Kind::Function && i.params.is_empty()) {
            let mi = ident(item.rust, self.span(name.loc));
            return Out::plain(quote_spanned!(span=> #qi::#mi()), Ty::from_rust(item.ret));
        }
        if is_pascal(&name.name) || name.name.chars().all(|c| !c.is_lowercase()) {
            let mi = self.name_ident(&name.name, name.loc);
            Out::plain(quote_spanned!(span=> #qi::#mi), Ty::named(q))
        } else if !index::ITEMS.iter().any(|i| i.owner == q) {
            // (a property of what the API has no type of: a top-level `val` of another file — `LocalSpacing.current` —
            // or Rust's, read where it is, as `top_level!` reads it)
            let mi = self.name_ident(&snake(&name.name), name.loc);
            let kt = self.kt(span);
            let v = proc_macro2::Ident::new("__kz_v", span);
            Out::plain(quote_spanned!(span=> #kt::top_level_with!(#qi, |#v| #v.#mi())), Ty::Unknown)
        } else {
            let mi = self.name_ident(&snake(&name.name), name.loc);
            Out::plain(quote_spanned!(span=> #qi::#mi()), Ty::Unknown)
        }
    }

    /// Whether a name is none the generator sees declared: no local, receiver's member, top-level `val` of the file,
    /// or the API's value (another file's `val`, Rust's own).
    pub(crate) fn unseen(&self, name: &str) -> bool {
        !self.is_local(name)
            && !self.receivers.iter().any(|r| members_of(&r.ty, name).next().is_some())
            && !self.file.vals.contains_key(name)
            && name != "Unit"
            && !lookup(name).iter().any(|i| i.kind == Kind::Value && i.owner.is_empty())
    }

    /// `a[i]`: Kotlin's `get` (a list's element, a map's value).
    fn index(&mut self, receiver: &Expr, indices: &[Expr], loc: Loc) -> Out {
        let span = self.span(loc);
        let kt = self.kt(span);
        let recv = self.expr(receiver);
        let r = recv.ts;
        let args: Vec<TokenStream> = indices.iter().map(|i| self.value(i)).collect();
        match &recv.ty {
            Ty::Map(_, v) => {
                let v = (**v).clone();
                Out::plain(quote_spanned!(span=> #kt::KtMapExt::get(&#r, #(#args),*)), v.nullable())
            }
            Ty::String => Out::plain(quote_spanned!(span=> #kt::KtStr::get(&#r, #(#args),*)), Ty::Char),
            t => {
                let el = t.element();
                Out::plain(quote_spanned!(span=> #kt::KtList::get(&#r, #(#args),*)), el)
            }
        }
    }

    // ---- Operators ---------------------------------------------------------------------------------------------

    fn prefix(&mut self, op: PrefixOp, operand: &Expr, loc: Loc) -> Out {
        let span = self.span(loc);
        match op {
            PrefixOp::Minus => {
                let o = self.expr(operand);
                let kind = if o.kind == OutKind::IntLit { OutKind::IntLit } else { OutKind::Plain };
                let ts = o.ts;
                Out { ts: quote_spanned!(span=> -#ts), ty: o.ty, kind }
            }
            PrefixOp::Plus => self.expr(operand),
            PrefixOp::Not => {
                let v = self.value(operand);
                Out::plain(quote_spanned!(span=> !#v), Ty::Bool)
            }
            PrefixOp::Inc | PrefixOp::Dec => self.step_expr(operand, op == PrefixOp::Inc, true, loc),
        }
    }

    fn postfix(&mut self, op: PostfixOp, operand: &Expr, loc: Loc) -> Out {
        let span = self.span(loc);
        match op {
            PostfixOp::NotNull => {
                let o = self.expr(operand);
                let ty = o.ty.non_null();
                let v = self.finish(o, Want::default(), operand.loc);
                let kt = self.kt(span);
                Out::plain(quote_spanned!(span=> #kt::not_null(#v)), ty)
            }
            PostfixOp::Inc | PostfixOp::Dec => self.step_expr(operand, op == PostfixOp::Inc, false, loc),
        }
    }

    /// `++x`, `x++` (`pre`), `--x`, `x--` as values.
    fn step_expr(&mut self, operand: &Expr, inc: bool, pre: bool, loc: Loc) -> Out {
        let span = self.span(loc);
        let kt = self.kt(span);
        let ExprKind::Name(n) = &operand.kind else {
            return Out::plain(self.later(loc, "`++`/`--` on a property as a value"), Ty::Unknown);
        };
        let Some(l) = self.local(&n.name) else {
            return Out::plain(self.later(loc, "`++`/`--` on a property as a value"), Ty::Unknown);
        };
        let i = ident(&l.rust, self.span(n.loc));
        if l.kind == LocalKind::Delegated {
            let f = match (inc, pre) {
                (true, true) => quote_spanned!(span=> pre_inc),
                (true, false) => quote_spanned!(span=> post_inc),
                (false, true) => quote_spanned!(span=> pre_dec),
                (false, false) => quote_spanned!(span=> post_dec),
            };
            return Out::plain(quote_spanned!(span=> #kt::#f(&#i)), l.ty);
        }
        let step =
            if inc { quote_spanned!(span=> #kt::KtInc::inc(#i)) } else { quote_spanned!(span=> #kt::KtInc::dec(#i)) };
        let v = self.fresh("v", span);
        let ts = if pre {
            quote_spanned!(span=> { #i = #step; #i })
        } else {
            quote_spanned!(span=> { let #v = #i; #i = #step; #v })
        };
        Out::plain(ts, l.ty)
    }

    fn binary(&mut self, op: BinaryOp, lhs: &Expr, rhs: &Expr, loc: Loc) -> Out {
        let span = self.span(loc);
        let kt = self.kt(span);
        match op {
            BinaryOp::Or | BinaryOp::And => {
                let (a, b) = (self.value(lhs), self.value(rhs));
                let ts =
                    if op == BinaryOp::Or { quote_spanned!(span=> #a || #b) } else { quote_spanned!(span=> #a && #b) };
                Out::plain(ts, Ty::Bool)
            }
            BinaryOp::Eq | BinaryOp::NotEq => {
                let eq = op == BinaryOp::Eq;
                let null_side = match (&lhs.kind, &rhs.kind) {
                    (_, ExprKind::Null) => Some(lhs),
                    (ExprKind::Null, _) => Some(rhs),
                    _ => None,
                };
                if let Some(side) = null_side {
                    let v = self.expr(side).ts;
                    let ts = if eq { quote_spanned!(span=> #v.is_none()) } else { quote_spanned!(span=> #v.is_some()) };
                    return Out::plain(ts, Ty::Bool);
                }
                let (a, b) = self.operands(lhs, rhs);
                let ts = if eq { quote_spanned!(span=> #a == #b) } else { quote_spanned!(span=> #a != #b) };
                Out::plain(ts, Ty::Bool)
            }
            BinaryOp::Identical | BinaryOp::NotIdentical => {
                Out::plain(self.later(loc, "identity equality (`===`)"), Ty::Bool)
            }
            BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => {
                let (a, b) = self.operands(lhs, rhs);
                let ts = match op {
                    BinaryOp::Lt => quote_spanned!(span=> #a < #b),
                    BinaryOp::Gt => quote_spanned!(span=> #a > #b),
                    BinaryOp::Le => quote_spanned!(span=> #a <= #b),
                    _ => quote_spanned!(span=> #a >= #b),
                };
                Out::plain(ts, Ty::Bool)
            }
            BinaryOp::In | BinaryOp::NotIn => {
                let a = self.value(lhs);
                let b = self.expr(rhs).ts;
                let ts = quote_spanned!(span=> #kt::is_in(&#a, &#b));
                Out::plain(if op == BinaryOp::In { ts } else { quote_spanned!(span=> !#ts) }, Ty::Bool)
            }
            BinaryOp::Elvis => self.elvis(lhs, rhs, loc),
            BinaryOp::RangeTo | BinaryOp::RangeUntil => {
                let (a, b) = self.numeric_operands(lhs, rhs);
                let ty = Ty::Range(Box::new(a.ty.clone()));
                let (a, b) = (a.ts, b.ts);
                let ts = if op == BinaryOp::RangeTo {
                    quote_spanned!(span=> #kt::range_to(#a, #b))
                } else {
                    quote_spanned!(span=> #kt::until(#a, #b))
                };
                Out::plain(ts, ty)
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                if op == BinaryOp::Add && (self.peek_ty(lhs).is_string() || self.peek_ty(rhs).is_string()) {
                    // String concatenation: Kotlin's `toString()` of what is added.
                    let mut pieces = Vec::new();
                    self.concat_pieces(lhs, &mut pieces);
                    self.concat_pieces(rhs, &mut pieces);
                    return Out::plain(quote_spanned!(span=> #kt::template!(#(#pieces),*)), Ty::String);
                }
                let (a, b) = self.numeric_operands(lhs, rhs);
                let ty = if a.ty == Ty::Unknown || (b.ty.is_float() && !a.ty.is_float()) {
                    b.ty.clone()
                } else {
                    a.ty.clone()
                };
                let (a, b) = (a.ts, b.ts);
                let ts = match op {
                    BinaryOp::Add => quote_spanned!(span=> #a + #b),
                    BinaryOp::Sub => quote_spanned!(span=> #a - #b),
                    BinaryOp::Mul => quote_spanned!(span=> #a * #b),
                    BinaryOp::Div => quote_spanned!(span=> #a / #b),
                    _ => quote_spanned!(span=> #a % #b),
                };
                Out::plain(ts, ty)
            }
        }
    }

    /// The pieces of a string concatenation (`a + b + c` flattened while it is one).
    fn concat_pieces(&mut self, e: &Expr, out: &mut Vec<TokenStream>) {
        if let ExprKind::Binary { op: BinaryOp::Add, lhs, rhs } = &e.kind
            && (self.peek_ty(lhs).is_string() || self.peek_ty(rhs).is_string())
        {
            self.concat_pieces(lhs, out);
            self.concat_pieces(rhs, out);
            return;
        }
        out.push(self.expr(e).ts);
    }

    /// Two operands compared: as written (Rust's `==` and `<` borrow), a string literal as it is, numbers of two
    /// types made one (Kotlin compares an `Int` with a `Long`).
    fn operands(&mut self, lhs: &Expr, rhs: &Expr) -> (TokenStream, TokenStream) {
        let (a, b) = self.typed_operands(lhs, rhs, false);
        (a.ts, b.ts)
    }

    /// Two operands of arithmetic, as Kotlin's numbers mix: an integer literal next to a float made a float literal,
    /// an operand of a narrower number type converted to the wider (`Int` + `Float` is a `Float`).
    fn numeric_operands(&mut self, lhs: &Expr, rhs: &Expr) -> (Out, Out) {
        self.typed_operands(lhs, rhs, true)
    }

    fn typed_operands(&mut self, lhs: &Expr, rhs: &Expr, values: bool) -> (Out, Out) {
        let lt = self.peek_ty(lhs);
        let rt = self.peek_ty(rhs);
        let wider = wider_number(&lt, &rt);
        let a = self.operand(lhs, rt.is_float(), values, &lt, wider.as_ref());
        let b = self.operand(rhs, lt.is_float(), values, &rt, wider.as_ref());
        (a, b)
    }

    fn operand(&mut self, e: &Expr, float: bool, value: bool, ty: &Ty, wider: Option<&Ty>) -> Out {
        let span = self.span(e.loc);
        if float && let Some(n) = int_literal(e) {
            let neg = matches!(e.kind, ExprKind::Prefix { op: PrefixOp::Minus, .. });
            let o = self.number(n, e.loc, true);
            if neg {
                let ts = o.ts;
                return Out::plain(quote_spanned!(span=> -#ts), o.ty);
            }
            return o;
        }
        let o = self.expr(e);
        let ty_out = o.ty.clone();
        let kind = o.kind;
        let ts = match kind {
            OutKind::StrLit | OutKind::IntLit => return o,
            OutKind::Local if !value => o.ts,
            _ => self.finish(o, Want { str_ok: true, any: None, some: false, shared: None }, e.loc),
        };
        match wider {
            Some(w) if w != ty && ty.is_number() => {
                let t = self.rust_of_ty(w, span);
                Out::plain(quote_spanned!(span=> (#ts as #t)), w.clone())
            }
            _ => Out { ts, ty: ty_out, kind: OutKind::Plain },
        }
    }

    /// `a ?: b`: `elvis(a, || b)`; a `match` when `b` never gives a value (`return`, `break`, `error(…)`).
    fn elvis(&mut self, lhs: &Expr, rhs: &Expr, loc: Loc) -> Out {
        let span = self.span(loc);
        let kt = self.kt(span);
        let a = self.expr(lhs);
        let ty = a.ty.non_null();
        let a = self.finish(a, Want::default(), lhs.loc);
        if diverges(rhs) {
            let b = self.stmt_expr(rhs);
            let v = self.fresh("v", span);
            return Out::plain(
                quote_spanned!(span=> match #a { ::core::option::Option::Some(#v) => #v, ::core::option::Option::None => #b }),
                ty,
            );
        }
        let awaits = self.awaits;
        let b = self.value(rhs);
        // (`b` suspends: Kotlin's `?:` is no function, it suspends with the coroutine)
        if self.in_async && self.awaits > awaits {
            let at = self.awaited(None, loc);
            return Out::plain(quote_spanned!(span=> #kt::elvis_async(#a, async || #b) #at), ty);
        }
        Out::plain(quote_spanned!(span=> #kt::elvis(#a, || #b)), ty)
    }

    fn infix(&mut self, name: &Ident, lhs: &Expr, rhs: &Expr, loc: Loc) -> Out {
        let span = self.span(loc);
        let kt = self.kt(span);
        match name.name.as_str() {
            "until" | "downTo" => {
                let (a, b) = self.numeric_operands(lhs, rhs);
                let ty = Ty::Range(Box::new(a.ty.clone()));
                let (a, b) = (a.ts, b.ts);
                let f =
                    if name.name == "until" { quote_spanned!(span=> until) } else { quote_spanned!(span=> down_to) };
                Out::plain(quote_spanned!(span=> #kt::#f(#a, #b)), ty)
            }
            "step" => {
                let a = self.expr(lhs);
                let ty = a.ty.clone();
                let a = a.ts;
                let b = self.expr(rhs);
                let b = if b.kind == OutKind::IntLit {
                    b.ts
                } else {
                    let ts = b.ts;
                    quote_spanned!(span=> ::core::convert::Into::<i64>::into(#ts))
                };
                Out::plain(quote_spanned!(span=> #kt::step(#a, #b)), ty)
            }
            "to" => {
                let a = self.value(lhs);
                let b = self.value(rhs);
                let ty = Ty::Pair(Box::new(self.peek_ty(lhs)), Box::new(self.peek_ty(rhs)));
                Out::plain(quote_spanned!(span=> #kt::to(#a, #b)), ty)
            }
            "and" | "or" | "xor" | "shl" | "shr" | "ushr" => {
                let ty = self.peek_ty(lhs);
                let (a, b) = (self.value(lhs), self.value(rhs));
                let ts = match name.name.as_str() {
                    "and" => quote_spanned!(span=> (#a & #b)),
                    "or" => quote_spanned!(span=> (#a | #b)),
                    "xor" => quote_spanned!(span=> (#a ^ #b)),
                    "shl" => quote_spanned!(span=> (#a << #b)),
                    "shr" => quote_spanned!(span=> (#a >> #b)),
                    _ => quote_spanned!(span=> ((#a as u32 >> #b) as i32)),
                };
                Out::plain(ts, ty)
            }
            other => {
                // A member of the left side's type or a member extension of an implicit receiver, as a call is.
                if let Some(out) = self.infix_member(name, lhs, rhs, loc) {
                    return out;
                }
                // An infix member of the index on a value of a type KompozR can't tell (`LocalX provides v`, `LocalX` a
                // `val` of another file).
                let items: Vec<&'static index::Item> =
                    lookup(other).iter().filter(|i| i.kind == Kind::Method && i.params.len() == 1).collect();
                let Some(item) = items.first().copied() else {
                    return Out::plain(self.later(loc, &format!("the infix function `{other}`")), Ty::Unknown);
                };
                // (a name whose declaration isn't seen, in place: a static of another file, `LocalSpacing provides …`)
                let recv = match &lhs.kind {
                    ExprKind::Name(n) if self.unseen(&n.name) => {
                        let i = self.name_ident(&value_name(&n.name), n.loc);
                        Out::plain(quote!(#i), Ty::Unknown)
                    }
                    _ => {
                        let o = self.expr(lhs);
                        Out { ty: Ty::Unknown, ..o }
                    }
                };
                if items.iter().any(|i| i.rust != item.rust) {
                    // (members of several Rust names: the one kind of callee it and its argument fit, as a method
                    // call's)
                    return self.infix_call(recv, lhs, name, rhs, loc);
                }
                // (all of one Rust name: written as each of them is, its argument as it is — Rust finds the method on
                // the value's type —, what it gives theirs where they agree)
                let b = self.value(rhs);
                let a = recv.ts;
                let m = ident(item.rust, self.span(name.loc));
                let ret = if items.iter().all(|i| i.ret == item.ret) { Ty::from_rust(item.ret) } else { Ty::Unknown };
                Out::plain(quote_spanned!(span=> #a.#m(#b)), ret)
            }
        }
    }

    // ---- Inference without writing -----------------------------------------------------------------------------

    /// What an expression's type is, as far as can be told without writing it (no capture recorded): Kotlin's
    /// inference as the index tells the callees — a generic callee's result told by its arguments and by what its
    /// lambdas give (`remember { … }`, `with(x) { … }`, `x.let { … }`), a lambda looked into with its receiver (the
    /// scope's type, or the value a scope function lends) and its parameters' types known, a block's own locals, a
    /// member extension of a receiver (`LocalX.currentValue`, `1.3f at 100`), a shared value's members.
    pub(crate) fn peek_ty(&self, e: &Expr) -> Ty {
        match &e.kind {
            ExprKind::Number(n) => match n.kind {
                NumberKind::Int => Ty::Int,
                NumberKind::Long => Ty::Long,
                NumberKind::Float => Ty::Float,
                NumberKind::Double => Ty::Double,
                _ => Ty::OtherInt,
            },
            ExprKind::String(_) => Ty::String,
            ExprKind::Char(_) => Ty::Char,
            ExprKind::Bool(_) | ExprKind::Is { .. } => Ty::Bool,
            ExprKind::Null => Ty::Nullable(Box::new(Ty::Unknown)),
            ExprKind::Name(n) => {
                if let Some(t) = self.peeked_local(&n.name) {
                    return t;
                }
                if let Some(v) = self.file.vals.get(&n.name) {
                    return v.ty.clone();
                }
                if let Some(t) = self.receiver_values().iter().find_map(|(r, value)| {
                    members_of(r, &n.name).find(|i| i.params.is_empty()).map(|i| owner_arg_ret(value, i.ret))
                }) {
                    return t;
                }
                lookup(&n.name)
                    .iter()
                    .find(|i| i.kind == Kind::Value && i.owner.is_empty())
                    .map(|i| Ty::from_rust(i.ret))
                    .unwrap_or(Ty::Unknown)
            }
            ExprKind::This { label } => {
                let r = match label {
                    None => self.receiver_values().into_iter().next().map(|r| r.1),
                    Some(l) => {
                        self.receivers.iter().rev().find(|r| r.labels.contains(&l.name)).map(|r| r.value.clone())
                    }
                };
                r.unwrap_or(Ty::Unknown)
            }
            ExprKind::Paren(x) | ExprKind::Annotated { expr: x, .. } => self.peek_ty(x),
            ExprKind::Prefix { op: PrefixOp::Not, .. } => Ty::Bool,
            ExprKind::Prefix { operand, .. } => self.peek_ty(operand),
            ExprKind::Postfix { op: PostfixOp::NotNull, operand } => self.peek_ty(operand).non_null(),
            ExprKind::Postfix { operand, .. } => self.peek_ty(operand),
            ExprKind::Binary { op, lhs, rhs } => match op {
                BinaryOp::Or
                | BinaryOp::And
                | BinaryOp::Eq
                | BinaryOp::NotEq
                | BinaryOp::Identical
                | BinaryOp::NotIdentical
                | BinaryOp::Lt
                | BinaryOp::Gt
                | BinaryOp::Le
                | BinaryOp::Ge
                | BinaryOp::In
                | BinaryOp::NotIn => Ty::Bool,
                BinaryOp::Elvis => {
                    let a = self.peek_ty(lhs).non_null();
                    if a == Ty::Unknown { self.peek_ty(rhs) } else { a }
                }
                BinaryOp::RangeTo | BinaryOp::RangeUntil => Ty::Range(Box::new(self.peek_ty(lhs))),
                _ => {
                    let (a, b) = (self.peek_ty(lhs), self.peek_ty(rhs));
                    if *op == BinaryOp::Add && (a.is_string() || b.is_string()) {
                        Ty::String
                    } else if a == Ty::Unknown || (b.is_float() && !a.is_float()) {
                        b
                    } else {
                        a
                    }
                }
            },
            ExprKind::Infix { name, lhs, rhs } => match name.name.as_str() {
                "until" | "downTo" | "step" => {
                    let t = self.peek_ty(lhs);
                    if matches!(t, Ty::Range(_)) { t } else { Ty::Range(Box::new(t)) }
                }
                "to" => Ty::Pair(Box::new(self.peek_ty(lhs)), Box::new(self.peek_ty(rhs))),
                "and" | "or" | "xor" | "shl" | "shr" | "ushr" => self.peek_ty(lhs),
                // (an infix member: the left side's type's, or a member extension of a receiver — `1.3f at 100` in a
                // `keyframes { }` block, the config's `at(value: T, …)` —; the file's own infix extension)
                other => {
                    let l = self.peek_ty(lhs).non_null();
                    if let Some(i) = self.method_of(&l, other) {
                        return owner_arg_ret(&l, i.ret);
                    }
                    if let Some(f) = self.file.extension(other, l.owner()) {
                        return f.ret.clone();
                    }
                    self.receiver_tys()
                        .iter()
                        .find_map(|r| {
                            members_of(r, other).find(|i| {
                                i.kind == Kind::Method && i.params.len() == 2 && {
                                    let first = Ty::from_rust(i.params[0].ty);
                                    super::call::is_type_parameter(i.params[0].ty)
                                        || first.owner().is_some() && first.owner() == l.owner()
                                        || first.is_float() && l.is_number()
                                        || first == l
                                }
                            })
                        })
                        .map(|i| Ty::from_rust(i.ret))
                        .unwrap_or(Ty::Unknown)
                }
            },
            ExprKind::If { then, otherwise, .. } => {
                let a = self.peek_block(then);
                match otherwise {
                    Some(o) if a == Ty::Unknown => self.peek_block(o),
                    Some(_) => a,
                    None => Ty::Unit,
                }
            }
            ExprKind::When(w) => {
                w.branches.iter().map(|b| self.peek_block(&b.body)).find(|t| *t != Ty::Unknown).unwrap_or(Ty::Unknown)
            }
            ExprKind::Member { receiver, safe, name } => {
                if let ExprKind::Number(_) = &receiver.kind {
                    match name.name.as_str() {
                        "dp" => return Ty::named("Dp"),
                        "sp" | "em" => return Ty::named("TextUnit"),
                        _ => {}
                    }
                }
                if let Some(q) = self.qualifier(receiver) {
                    if name.name == "Companion" {
                        return Ty::named(&q);
                    }
                    if is_value_object(&q) {
                        if let Some(i) = members_of(&q, &name.name).next() {
                            return Ty::from_rust(i.ret);
                        }
                        // (a member extension property of a receiver: `LocalX.currentValue`)
                        let value = value_object_ty(&q);
                        return self.peek_extension_property(&value, &name.name);
                    }
                    return associated(&q, &name.name).first().map(|i| Ty::from_rust(i.ret)).unwrap_or(Ty::named(&q));
                }
                let r = self.peek_ty(receiver).non_null();
                let t = self.member_ty(&r, &name.name, false);
                let t = if t == Ty::Unknown { self.peek_extension_property(&r, &name.name) } else { t };
                if *safe { t.nullable() } else { t }
            }
            ExprKind::Call { callee, type_args, args, trailing_lambda } => {
                self.peek_call(callee, type_args, args, trailing_lambda.as_deref())
            }
            ExprKind::Index { receiver, .. } => match self.peek_ty(receiver) {
                Ty::Map(_, v) => v.nullable(),
                t => t.element(),
            },
            _ => Ty::Unknown,
        }
    }

    /// The type of a block's value (its last expression), the locals it declares before it known as far as their
    /// values tell (`remember { val a = Animatable(0f); a }`).
    pub(crate) fn peek_block(&self, b: &Block) -> Ty {
        let Some(Stmt { kind: StmtKind::Expr(e), .. }) = b.stmts.last() else { return Ty::Unit };
        if b.stmts.len() == 1 {
            return self.peek_ty(e);
        }
        self.peek_scopes.borrow_mut().push(std::collections::HashMap::new());
        for s in &b.stmts[..b.stmts.len() - 1] {
            let StmtKind::Decl(Decl::Property(p)) = &s.kind else { continue };
            let Binding::Name(n) = &p.binding else { continue };
            let ty = match (&p.ty, &p.init) {
                (Some(t), _) => self.ty_of(t),
                (None, PropertyInit::Value(v)) => self.peek_ty(v),
                (None, PropertyInit::Delegate(d)) => self.delegate_ty(d),
                (None, PropertyInit::None) => Ty::Unknown,
            };
            self.peek_scopes.borrow_mut().last_mut().expect("its scope").insert(n.name.clone(), ty);
        }
        let t = self.peek_ty(e);
        self.peek_scopes.borrow_mut().pop();
        t
    }

    /// The type of a local of the name as inference sees it: one a block it looks into declares, else one in scope.
    fn peeked_local(&self, name: &str) -> Option<Ty> {
        if let Some(t) = self.peek_scopes.borrow().iter().rev().find_map(|s| s.get(name)) {
            return Some(t.clone());
        }
        self.scopes.iter().rev().find_map(|s| s.get(name)).map(|l| l.ty.clone())
    }

    /// The Rust types of the implicit receivers, innermost first: those of the lambdas inference looks into, then
    /// those of the lambdas and extension functions being written.
    pub(crate) fn receiver_tys(&self) -> Vec<String> {
        let mut out: Vec<String> = self.peek_receivers.borrow().iter().rev().map(|r| r.0.clone()).collect();
        out.extend(self.receivers.iter().rev().map(|r| r.ty.clone()));
        out
    }

    /// The implicit receivers' types with their type arguments where known, in [`receiver_tys`](Self::receiver_tys)'
    /// order (each with its Rust type's name): what their members' `T` is.
    pub(crate) fn receiver_values(&self) -> Vec<(String, Ty)> {
        let mut out: Vec<(String, Ty)> = self.peek_receivers.borrow().iter().rev().cloned().collect();
        out.extend(self.receivers.iter().rev().map(|r| (r.ty.clone(), r.value.clone())));
        out
    }

    /// What `lambda` given to a parameter taking `info` gives, its receiver (a scope's type, or the value the call is
    /// made on where it is lent it: `apply`, `run`, `with`) and its parameters (`it`, the names it gives them) known to
    /// inference as the index's types tell. `lent` is that value's type and the name of the type parameter the
    /// callee lends it as (`Self` for a method of it — its `Self::Item`s its elements —, `T` for `with(x) { }`).
    pub(crate) fn peek_lambda(&self, lambda: &Lambda, info: Option<&index::Lambda>, lent: Option<(&Ty, &str)>) -> Ty {
        let mut binds = TypeBinds::new();
        if let Some((ty, name)) = lent {
            if name == "Self" {
                binds = lent_binds(ty);
            } else {
                binds.insert(name.to_string(), ty.clone());
            }
        }
        self.peek_lambda_bound(lambda, info, &binds, lent.map(|l| l.1))
    }

    /// [`peek_lambda`](Self::peek_lambda) with the callee's type parameters `binds` tells (its parameters' types
    /// read with them: `fold(0f) { acc, x -> … }`'s `acc` a `Float`); `lent` names the one a `&mut` of which is
    /// Kotlin's receiver (`T.() -> R`).
    fn peek_lambda_bound(
        &self,
        lambda: &Lambda,
        info: Option<&index::Lambda>,
        binds: &TypeBinds,
        lent: Option<&str>,
    ) -> Ty {
        let mut inputs: Vec<Ty> = Vec::new();
        let mut receiver = None;
        if let Some(info) = info {
            let bare = |t: &str| {
                let t = t.trim();
                t.strip_prefix("&mut ").or_else(|| t.strip_prefix('&')).unwrap_or(t).trim().to_string()
            };
            let mut rest = info.inputs.iter();
            if !info.receiver.is_empty() {
                receiver = Some((info.receiver.to_string(), Ty::from_rust(info.receiver)));
                rest.next();
            } else if let Some(first) = info.inputs.first()
                && first.trim().starts_with("&mut")
                && let Some(name) = lent
                && bare(first) == name
                && let Some(ty) = binds.get(name)
            {
                // (Kotlin's `T.() -> R` of a scope function, the value lent as `&mut Self` / `&mut T`)
                receiver = ty.owner().map(|o| (o.to_string(), ty.clone()));
                rest.next();
            }
            for t in rest {
                inputs.push(Ty::from_rust_bound(t, binds));
            }
        }
        let mut scope = std::collections::HashMap::new();
        match &lambda.params {
            Some(ps) => {
                for (k, p) in ps.iter().enumerate() {
                    if let Binding::Name(n) = &p.binding {
                        let ty = p.ty.as_ref().map(|t| self.ty_of(t)).or_else(|| inputs.get(k).cloned());
                        scope.insert(n.name.clone(), ty.unwrap_or(Ty::Unknown));
                    }
                }
            }
            None => {
                if let [only] = inputs.as_slice() {
                    scope.insert("it".to_string(), only.clone());
                }
            }
        }
        self.peek_scopes.borrow_mut().push(scope);
        let pushed = receiver.is_some();
        if let Some(r) = receiver {
            self.peek_receivers.borrow_mut().push(r);
        }
        let t = self.peek_block(&lambda.body);
        if pushed {
            self.peek_receivers.borrow_mut().pop();
        }
        self.peek_scopes.borrow_mut().pop();
        t
    }

    /// What a generic method `item` called on a value of type `recv` gives: its result with its type parameters bound
    /// by inference — the value's (`Self`, a kt shim's `Self::Item`, a generic type's arguments), then its arguments'
    /// (`fold`'s `initial: R`), then what its lambdas give (`map`'s `R`, `let`'s).
    fn method_ret(&self, recv: &Ty, item: &index::Item, args: &[Arg], trailing: Option<&Lambda>) -> Ty {
        let mut binds = lent_binds(recv);
        if let Ty::Named(n, a) = recv.through_shared()
            && let Some(ps) = index::generics(n).filter(|ps| ps.len() == a.len())
        {
            for (p, t) in ps.iter().zip(a) {
                binds.insert(p.to_string(), t.clone());
            }
        }
        // (a kt shim's `T` of a collection: its element)
        if item.krate == "kompoz_runtime" && item.module.starts_with("kt") && !binds.contains_key("T") {
            binds.insert("T".to_string(), recv.element());
        }
        self.bound_ret(item, binds, args, trailing, Some("Self"))
    }

    /// What a generic callee `item` gives, its type parameters `binds` tells bound further by its arguments (values
    /// first: what its lambdas' parameters are may be told by them) and by what its lambdas give, each looked into
    /// with the parameters bound so far; `lent` names the type parameter a `&mut` of which is a lambda's receiver.
    fn bound_ret(
        &self,
        item: &index::Item,
        mut binds: TypeBinds,
        args: &[Arg],
        trailing: Option<&Lambda>,
        lent: Option<&str>,
    ) -> Ty {
        let n = item.params.len();
        let given = |k: usize| -> Option<GivenArg<'_>> {
            let p = &item.params[k];
            if let Some(a) = args.iter().find(|a| a.name.as_ref().is_some_and(|x| x.name == p.kotlin)) {
                return Some(GivenArg::of(&a.value));
            }
            if let Some(a) = args.iter().filter(|a| a.name.is_none()).nth(k) {
                return Some(GivenArg::of(&a.value));
            }
            trailing.filter(|_| k + 1 == n).map(GivenArg::Lambda)
        };
        for (k, p) in item.params.iter().enumerate() {
            if let (None, Some(GivenArg::Value(e))) = (p.lambda, given(k)) {
                bind_type_params(p.ty, &self.peek_ty(e), &mut binds);
            }
        }
        for (k, p) in item.params.iter().enumerate() {
            if let (Some(info), Some(GivenArg::Lambda(l))) = (p.lambda, given(k)) {
                let gives = self.peek_lambda_bound(l, Some(&info), &binds, lent);
                if gives != Ty::Unit {
                    bind_type_params(info.output, &gives, &mut binds);
                }
            }
        }
        Ty::from_rust_bound(item.ret, &binds)
    }

    /// The method `name` a value of type `recv` has, as the index tells: its type's own (or its extension traits',
    /// what it derefs to), else a kt shim's of its Kotlin type.
    fn method_of(&self, recv: &Ty, name: &str) -> Option<&'static index::Item> {
        if let Some(o) = recv.owner()
            && let Some(i) = members_of(o, name).find(|i| i.kind == Kind::Method)
        {
            return Some(i);
        }
        kt_members(name, recv.kt_traits()).into_iter().next()
    }

    /// A member extension property of an implicit receiver (Kotlin's `val T.name` declared in a scope:
    /// `LocalX.currentValue` in a `CompositionLocalAccessorScope`) that a value of type `recv` has, as inference sees
    /// the receivers: its type.
    fn peek_extension_property(&self, recv: &Ty, name: &str) -> Ty {
        self.receiver_tys()
            .iter()
            .find_map(|r| extension_member(r, recv, name, 1))
            .map(|i| owner_arg_ret(recv, i.ret))
            .unwrap_or(Ty::Unknown)
    }

    /// The innermost implicit receiver (written) with a member extension property `name` that a value of type `recv`
    /// has, and the property's getter.
    pub(crate) fn extension_property(
        &mut self,
        recv: &Ty,
        name: &str,
    ) -> Option<(ImplicitReceiver, &'static index::Item)> {
        let found =
            self.receivers.iter().rev().find_map(|r| extension_member(&r.ty, recv, name, 1).map(|i| (r.clone(), i)))?;
        // (a use: captured from a lambda)
        let used = self.use_receiver(&found.0);
        Some((used, found.1))
    }

    /// The type of a member `name` of a value of type `recv` (`call`: a method called).
    pub(crate) fn member_ty(&self, recv: &Ty, name: &str, call: bool) -> Ty {
        if let Some(o) = recv.owner()
            && let Some(i) = members_of(o, name).find(|i| call || i.params.is_empty())
        {
            return owner_arg_ret(recv, i.ret);
        }
        if let Some(f) = self.file.extension(name, recv.owner()) {
            return f.ret.clone();
        }
        let kt = kt_members(name, recv.kt_traits());
        let ret = kt.first().map(|i| i.ret).unwrap_or("");
        let elem = recv.element();
        match ret {
            "Self::Item" | "T" => elem,
            "Option<Self::Item>" | "Option<T>" => elem.nullable(),
            "Vec<Self::Item>" | "Vec<T>" => Ty::List(Box::new(elem)),
            "Self" => recv.clone(),
            "" => Ty::Unknown,
            r if r.starts_with("Vec<R>") => Ty::List(Box::new(Ty::Unknown)),
            r => Ty::from_rust(r),
        }
    }

    fn peek_call(&self, callee: &Expr, type_args: &[TypeArg], args: &[Arg], trailing: Option<&Lambda>) -> Ty {
        match &callee.kind {
            ExprKind::Name(n) => {
                if let Some(l) = self.scopes.iter().rev().find_map(|s| s.get(&n.name)) {
                    return match &l.ty {
                        Ty::Fun(f) => f.ret.clone(),
                        _ => Ty::Unknown,
                    };
                }
                if let Some(f) = self.file.function(&n.name) {
                    return f.ret.clone();
                }
                if let Some(t) = self
                    .receiver_values()
                    .iter()
                    .find_map(|(r, value)| members_of(r, &n.name).next().map(|i| owner_arg_ret(value, i.ret)))
                {
                    return t;
                }
                match n.name.as_str() {
                    "listOf" | "mutableListOf" | "arrayListOf" => {
                        let t = args.first().map(|a| self.peek_ty(&a.value)).unwrap_or(Ty::Unknown);
                        return Ty::List(Box::new(t));
                    }
                    "mutableStateOf" => {
                        let t = args.first().map(|a| self.peek_ty(&a.value)).unwrap_or(Ty::Unknown);
                        return Ty::Named("MutableState".into(), vec![t]);
                    }
                    _ => {}
                }
                let n_args = args.len() + usize::from(trailing.is_some());
                let named = |i: &&crate::index::Item| {
                    args.iter().filter_map(|a| a.name.as_ref()).all(|a| i.params.iter().any(|p| p.kotlin == a.name))
                };
                free_functions(&n.name)
                    .find(|i| i.params.len() >= n_args && named(i))
                    .or_else(|| free_functions(&n.name).find(named))
                    .or_else(|| free_functions(&n.name).next())
                    .map(|i| self.generic_ret(i, type_args, args, trailing))
                    .unwrap_or_else(|| if is_pascal(&n.name) { Ty::named(&n.name) } else { Ty::Unknown })
            }
            ExprKind::Member { receiver, safe, name } => {
                if let Some(q) = self.qualifier(receiver) {
                    if is_value_object(&q) || q == "Modifier" {
                        let ret = lookup(&name.name)
                            .iter()
                            .find(|i| i.owner == q && i.kind == Kind::Method)
                            .map(|i| Ty::from_rust(i.ret));
                        if let Some(r) = ret {
                            return r;
                        }
                        if let Some(f) = self.file.extension(&name.name, Some(&q)) {
                            return f.ret.clone();
                        }
                        // (a member extension of a receiver: `Modifier.animateEnterExit()` in a `with(scope) { }`)
                        let value = if q == "Modifier" { Ty::named("Modifier") } else { value_object_ty(&q) };
                        return self
                            .receiver_tys()
                            .iter()
                            .find_map(|r| extension_member(r, &value, &name.name, usize::MAX))
                            .map(|i| owner_arg_ret(&value, i.ret))
                            .unwrap_or(Ty::Unknown);
                    }
                    return associated(&q, &name.name).first().map(|i| Ty::from_rust(i.ret)).unwrap_or(Ty::Unknown);
                }
                let r = self.peek_ty(receiver).non_null();
                let mut t = self.member_ty(&r, &name.name, true);
                // (a generic method: what its type parameters are told by the value it is called on, its arguments and
                // what its lambdas give — `x.let { … }`, `x.run { … }`, `xs.map { it.toFloat() }`, `xs.fold(0f) { … }`)
                if (t == Ty::Unknown || t.is_vague())
                    && let Some(item) = self.method_of(&r, &name.name)
                {
                    let bound = self.method_ret(&r, item, args, trailing);
                    if bound != Ty::Unknown && (t == Ty::Unknown || !bound.is_vague()) {
                        t = bound;
                    }
                }
                if t == Ty::Unknown {
                    // (a member extension of a receiver: `placeable.place(…)`'s, `x.animateEnterExit()`)
                    t = self
                        .receiver_tys()
                        .iter()
                        .find_map(|rt| extension_member(rt, &r, &name.name, usize::MAX))
                        .map(|i| owner_arg_ret(&r, i.ret))
                        .unwrap_or(Ty::Unknown);
                }
                if *safe { t.nullable() } else { t }
            }
            _ => Ty::Unknown,
        }
    }
}

impl Gen<'_, '_> {
    /// What a function of the index gives: its result's type parameter (one) told by the type argument written
    /// (`compositionLocalOf<Account> { … }`), else by the argument that has that type (`mutableStateOf(1)`) or the
    /// lambda that gives it (`remember { mutableStateListOf("a") }`, `staticCompositionLocalOf { Spacing() }`).
    pub(crate) fn generic_ret(
        &self,
        item: &crate::index::Item,
        type_args: &[TypeArg],
        args: &[Arg],
        trailing: Option<&Lambda>,
    ) -> Ty {
        let t = self.generic_ret_of_one(item, type_args, args, trailing);
        // (one of several type parameters, or one told by a part of an argument's type — `checkNotNull(x)`'s `T` of a
        // `T?` —: each bound as a method's are)
        if (t == Ty::Unknown || t.is_vague()) && !item.has(crate::index::flags::OVERLOADED) && type_args.is_empty() {
            let bound = self.bound_ret(item, TypeBinds::new(), args, trailing, None);
            if bound != Ty::Unknown && (t == Ty::Unknown || !bound.is_vague()) {
                return bound;
            }
        }
        t
    }

    /// [`generic_ret`](Self::generic_ret) of a callee whose result names one type parameter.
    fn generic_ret_of_one(
        &self,
        item: &crate::index::Item,
        type_args: &[TypeArg],
        args: &[Arg],
        trailing: Option<&Lambda>,
    ) -> Ty {
        // (an overloaded constructor: what the overload its arguments take gives — `Animatable(0f)` an
        // `Animatable<Float, AnimationVector1D>`)
        if item.has(crate::index::flags::OVERLOADED) {
            let mut given: Vec<(Option<&str>, Ty)> =
                args.iter().map(|a| (a.name.as_ref().map(|n| n.name.as_str()), self.peek_ty(&a.value))).collect();
            if trailing.is_some() {
                given.push((None, Ty::Unknown));
            }
            return overload_ret(item, choose_overload(item.kotlin, item, &given), &given);
        }
        let ret = Ty::from_rust(item.ret);
        let generics: Vec<&str> = item
            .ret
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|w| w.len() == 1 && w.starts_with(|c: char| c.is_ascii_uppercase()))
            .collect();
        let Some(g) = generics.first().copied().filter(|g| generics.iter().all(|x| x == g)) else { return ret };
        let told = match type_args {
            [TypeArg::Type { ty, .. }] => self.ty_of(ty),
            _ => {
                // (the parameter of type `g`, or of a lambda giving one; its argument by name, else by place, a trailing
                // lambda the last)
                let gives = |p: &crate::index::Param| {
                    let ty = p.ty.trim();
                    ty == g
                        // (a vararg of it: `mutableStateListOf(Todo(…), …)`'s `impl IntoIterator<Item = T>`)
                        || p.vararg && {
                            let t = ty.replace(' ', "");
                            t.ends_with(&format!("={g}>")) || t == format!("&[{g}]") || t == format!("[{g}]")
                        }
                        || p.lambda.is_some_and(|l| l.output.trim() == g)
                        || ty
                            .strip_prefix("fn()")
                            .is_some_and(|r| r.trim().strip_prefix("->").map(str::trim) == Some(g))
                };
                let Some(k) = item.params.iter().position(gives) else {
                    // (a parameter of a generic type of it, `impl Into<TransitionState<S>>`: the type argument of the
                    // argument given, a `MutableTransitionState<Int>`'s `Int`)
                    let of_it = format!("<{g}>");
                    let told = item.params.iter().enumerate().find_map(|(k, p)| {
                        if !p.ty.replace(' ', "").contains(&of_it) || p.lambda.is_some() {
                            return None;
                        }
                        let by_name = args.iter().find(|a| a.name.as_ref().is_some_and(|n| n.name == p.kotlin));
                        let by_place = args.iter().filter(|a| a.name.is_none()).nth(k);
                        match by_name.or(by_place).map(|a| self.peek_ty(&a.value)) {
                            Some(Ty::Named(_, a)) if a.len() == 1 && a[0] != Ty::Unknown => Some(a[0].clone()),
                            _ => None,
                        }
                    });
                    return match told {
                        Some(t) => fill_unknown(ret, &t),
                        None => ret,
                    };
                };
                let last = k + 1 == item.params.len();
                let by_name = args.iter().find(|a| a.name.as_ref().is_some_and(|n| n.name == item.params[k].kotlin));
                let by_place = args.iter().filter(|a| a.name.is_none()).nth(k);
                // (`with(x) { … }`: its lambda's receiver is `x`'s type)
                let with_ty = (item.krate == "kompoz_runtime" && item.module.starts_with("kt") && item.rust == "with")
                    .then(|| args.first().map(|a| self.peek_ty(&a.value)))
                    .flatten();
                let info = item.params[k].lambda;
                let value = match (by_name.or(by_place), trailing) {
                    (Some(a), _) => match &a.value.kind {
                        ExprKind::Lambda(l) => self.peek_lambda(l, info.as_ref(), with_ty.as_ref().map(|t| (t, "T"))),
                        _ if item.params[k].ty.trim() == g || item.params[k].vararg => self.peek_ty(&a.value),
                        _ => Ty::Unknown,
                    },
                    (None, Some(l)) if last => self.peek_lambda(l, info.as_ref(), with_ty.as_ref().map(|t| (t, "T"))),
                    _ => Ty::Unknown,
                };
                if value == Ty::Unit { Ty::Unknown } else { value }
            }
        };
        if told == Ty::Unknown { ret } else { fill_unknown(ret, &told) }
    }
}

/// The overload of `f` (Kotlin's overloads of a constructor taken as one argument, `Color(…)`) that arguments of these
/// names and types take.
pub(crate) fn choose_overload(
    kotlin: &str,
    f: &index::Item,
    args: &[(Option<&str>, Ty)],
) -> Option<&'static index::Item> {
    let overloads: Vec<&'static index::Item> = lookup(kotlin)
        .iter()
        .filter(|i| {
            i.kind == Kind::Overload && f.params.first().is_some_and(|p| i.via == p.ty.trim_start_matches("impl "))
        })
        .collect();
    let overloads = if overloads.is_empty() {
        lookup(kotlin).iter().filter(|i| i.kind == Kind::Overload).collect()
    } else {
        overloads
    };
    let any_float = args.iter().any(|a| a.1.is_float());
    let long = args.len() == 1 && args[0].1 == Ty::Long;
    // (`exact`: an argument of a type known is one of the parameter's — a `Float` isn't a `Color`)
    let takes = |o: &&'static index::Item, exact: bool| {
        o.params.len() == args.len()
            && args.iter().zip(o.params).all(|((name, given), p)| {
                name.is_none_or(|n| n == p.kotlin)
                    && match Ty::from_rust(p.ty) {
                        Ty::Float => any_float,
                        Ty::Int => !any_float,
                        Ty::Long => long,
                        // (an offset's `usize`: Kotlin's Int there — `TextRange(0, 3)`)
                        Ty::OtherInt => !any_float && !long,
                        Ty::Named(n, _) if exact => *given == Ty::Unknown || given.owner() == Some(n.as_str()),
                        _ => true,
                    }
            })
    };
    overloads.iter().find(|o| takes(o, true)).or_else(|| overloads.iter().find(|o| takes(o, false))).copied()
}

/// What an overloaded constructor `f` gives through `chosen`: that overload's impl's associated type, its type
/// parameters the ones its arguments tell (`Animatable(Offset.Zero, Offset.VectorConverter)`); an associated type the
/// index doesn't know (`A::Animatable`) is the type of that name.
pub(crate) fn overload_ret(f: &index::Item, chosen: Option<&index::Item>, args: &[(Option<&str>, Ty)]) -> Ty {
    let ret = chosen.map_or(f.ret, |o| o.ret);
    if let Some((of, assoc)) = ret.rsplit_once("::")
        && of.len() == 1
        && assoc.chars().next().is_some_and(char::is_uppercase)
    {
        return Ty::named(assoc);
    }
    let Some(o) = chosen else { return Ty::from_rust(ret) };
    let mut binds = TypeBinds::new();
    for (k, (name, ty)) in args.iter().enumerate() {
        let at = name.and_then(|n| o.params.iter().position(|p| p.kotlin == n)).unwrap_or(k);
        if let Some(p) = o.params.get(at) {
            bind_type_params(p.ty, ty, &mut binds);
        }
    }
    Ty::from_rust_bound(ret, &binds)
}

/// An argument as inference reads it: a lambda, or a value.
enum GivenArg<'e> {
    Lambda(&'e Lambda),
    Value(&'e Expr),
}

impl<'e> GivenArg<'e> {
    fn of(e: &'e Expr) -> GivenArg<'e> {
        match &e.kind {
            ExprKind::Lambda(l) => GivenArg::Lambda(l),
            _ => GivenArg::Value(e),
        }
    }
}

/// What a value of type `ty` lent to a method of it (`Self`) tells of the method's type parameters: `Self`, and a
/// collection's `Self::Item` (its element), a map's `Self::K` and `Self::V`.
fn lent_binds(ty: &Ty) -> TypeBinds {
    let mut binds = TypeBinds::new();
    binds.insert("Self".to_string(), ty.clone());
    binds.insert("Self::Item".to_string(), ty.element());
    if let Ty::Map(k, v) = ty {
        binds.insert("Self::K".to_string(), (**k).clone());
        binds.insert("Self::V".to_string(), (**v).clone());
    }
    binds
}

/// `t` with what it leaves unknown made `with`.
fn fill_unknown(t: Ty, with: &Ty) -> Ty {
    let fill = |b: Box<Ty>| Box::new(fill_unknown(*b, with));
    match t {
        Ty::Unknown => with.clone(),
        Ty::Nullable(x) => Ty::Nullable(fill(x)),
        Ty::List(x) => Ty::List(fill(x)),
        Ty::Set(x) => Ty::Set(fill(x)),
        Ty::Range(x) => Ty::Range(fill(x)),
        Ty::Map(k, v) => Ty::Map(fill(k), fill(v)),
        Ty::Pair(a, b) => Ty::Pair(fill(a), fill(b)),
        Ty::Named(n, args) => Ty::Named(n, args.into_iter().map(|a| fill_unknown(a, with)).collect()),
        other => other,
    }
}

/// The wider of two known number types, when they differ (Kotlin's arithmetic gives it).
fn wider_number(a: &Ty, b: &Ty) -> Option<Ty> {
    let rank = |t: &Ty| match t {
        Ty::Int => Some(1),
        Ty::Long => Some(2),
        Ty::Float => Some(3),
        Ty::Double => Some(4),
        _ => None,
    };
    let (ra, rb) = (rank(a)?, rank(b)?);
    if ra == rb {
        return None;
    }
    // (a Long and a Float make a Float, as Kotlin's)
    Some(if ra > rb { a.clone() } else { b.clone() })
}

/// The integer literal an expression is (`2`, `-2`).
fn int_literal(e: &Expr) -> Option<&Number> {
    match &e.kind {
        ExprKind::Number(n) if n.kind == NumberKind::Int && n.radix == 10 => Some(n),
        ExprKind::Prefix { op: PrefixOp::Minus, operand } => int_literal(operand),
        _ => None,
    }
}

/// Whether an expression never gives a value (what `?:` can't take as a value).
pub(crate) fn diverges(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Return { .. } | ExprKind::Break { .. } | ExprKind::Continue { .. } | ExprKind::Throw(_) => true,
        ExprKind::Call { callee, .. } => {
            matches!(&callee.kind, ExprKind::Name(n) if matches!(n.name.as_str(), "error" | "TODO"))
        }
        ExprKind::Paren(x) => diverges(x),
        _ => false,
    }
}

/// Whether a kt shim member's name is one no Rust type the generator meets has a member of (it can be called
/// through its trait without knowing the receiver's type).
fn recv_has_no_member(name: &str) -> bool {
    !matches!(name, "size" | "first" | "last" | "isEmpty" | "value" | "keys" | "values" | "length")
}

/// Kotlin's number types' constants (`Int.MAX_VALUE` → `i32::MAX`).
fn number_constant(q: &str, name: &str, span: Span) -> Option<(TokenStream, Ty)> {
    let (t, ty) = match q {
        "Int" => (quote_spanned!(span=> i32), Ty::Int),
        "Long" => (quote_spanned!(span=> i64), Ty::Long),
        "Float" => (quote_spanned!(span=> f32), Ty::Float),
        "Double" => (quote_spanned!(span=> f64), Ty::Double),
        _ => return None,
    };
    let c = match name {
        "MAX_VALUE" => quote_spanned!(span=> MAX),
        // (Kotlin's least positive value: the least subnormal one)
        "MIN_VALUE" if ty.is_float() => return Some((quote_spanned!(span=> #t::from_bits(1)), ty)),
        "MIN_VALUE" => quote_spanned!(span=> MIN),
        "POSITIVE_INFINITY" => quote_spanned!(span=> INFINITY),
        "NEGATIVE_INFINITY" => quote_spanned!(span=> NEG_INFINITY),
        "NaN" => quote_spanned!(span=> NAN),
        _ => return None,
    };
    Some((quote_spanned!(span=> #t::#c), ty))
}

use crate::pos::Loc;

/// A value object's type (`LocalDensity`'s `KompozitionLocal<Density>`).
fn value_object_ty(q: &str) -> Ty {
    lookup(q)
        .iter()
        .find(|i| i.kind == Kind::Value && i.owner.is_empty())
        .map(|v| Ty::from_rust(v.ret))
        .unwrap_or(Ty::Unknown)
}

/// A member extension of a receiver of Rust type `receiver` (a method taking the value it extends first) for a value
/// of type `recv` (known: the method's first parameter of its type), of `params` parameters with that value (a
/// property's: 1; any: `usize::MAX`).
fn extension_member(receiver: &str, recv: &Ty, name: &str, params: usize) -> Option<&'static index::Item> {
    let owner = recv.owner()?;
    members_of(receiver, name).find(|i| {
        i.kind == Kind::Method
            && !i.params.is_empty()
            && (params == usize::MAX || i.params.len() == params)
            && Ty::from_rust(i.params[0].ty).owner() == Some(owner)
    })
}

/// What a member of a value of a generic type gives (`State<Float>`'s `value`, a `T`; `Animatable<Float, …>`'s
/// `lowerBound`, a `T?`): its type with the type's parameters the value's type arguments ([`index::GENERICS`]); a type
/// of one type argument whose parameter the index doesn't name: that argument for a bare type parameter.
pub(crate) fn owner_arg_ret(recv: &Ty, ret: &str) -> Ty {
    match recv.through_shared() {
        Ty::Named(n, a) if !a.is_empty() => {
            let bound = match index::generics(n).filter(|ps| ps.len() == a.len()) {
                Some(ps) => {
                    let mut binds: TypeBinds = ps.iter().zip(a).map(|(p, t)| (p.to_string(), t.clone())).collect();
                    binds.insert("Self".to_string(), recv.through_shared().clone());
                    Ty::from_rust_bound(ret, &binds)
                }
                None => Ty::from_rust(ret),
            };
            if bound == Ty::Unknown && a.len() == 1 && super::call::is_type_parameter(ret) {
                a[0].clone()
            } else {
                bound
            }
        }
        _ => Ty::from_rust(ret),
    }
}
