//! Statements: locals (`val`, `var`, `by` delegates, destructuring), assignments, loops, jumps, and the blocks of
//! `if`/`when` (as statements and as values).

use super::expr::{Out, OutKind, Want};
use super::names::{ident, snake};
use super::resolve::*;
use super::{Gen, LoopCx};
use crate::ast::*;
use crate::pos::Loc;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};

/// What a block's last statement is.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Tail {
    /// A statement: the block gives nothing.
    Unit,
    /// The block's value (an `if`/`when` branch, a lambda's body).
    Value(Want),
    /// A function's value.
    Return,
}

impl Tail {
    fn want(self) -> Want {
        match self {
            Tail::Value(w) => w,
            _ => Want::default(),
        }
    }
}

impl Gen<'_, '_> {
    /// A block's statements in a scope of their own (without its braces).
    pub(crate) fn block_inner(&mut self, b: &Block, tail: Tail) -> TokenStream {
        self.push_scope();
        let ts = self.stmts(&b.stmts, tail);
        self.pop_scope();
        ts
    }

    /// A block with its braces, at its position.
    pub(crate) fn braced(&mut self, b: &Block, tail: Tail) -> TokenStream {
        let span = self.span(b.loc);
        let inner = self.block_inner(b, tail);
        quote_spanned!(span=> { #inner })
    }

    pub(crate) fn stmts(&mut self, stmts: &[Stmt], tail: Tail) -> TokenStream {
        let mut out = TokenStream::new();
        for (k, s) in stmts.iter().enumerate() {
            let last = k + 1 == stmts.len();
            match (&s.kind, last, tail) {
                (StmtKind::Expr(e), true, Tail::Value(_) | Tail::Return) if gives_value(e) => {
                    out.extend(self.value_want(e, tail.want()));
                }
                _ => out.extend(self.stmt(s)),
            }
        }
        out
    }

    pub(crate) fn stmt(&mut self, s: &Stmt) -> TokenStream {
        let span = self.span(s.loc);
        match &s.kind {
            StmtKind::Decl(Decl::Property(p)) => self.local_property(p),
            StmtKind::Decl(Decl::Function(f)) => {
                let t = self.later(f.loc, "a local function");
                quote_spanned!(span=> #t;)
            }
            StmtKind::Decl(Decl::Failed { .. }) => quote!(),
            StmtKind::Decl(d) => {
                let t = self.later(d.loc(), "a local class, object or type alias");
                quote_spanned!(span=> #t;)
            }
            StmtKind::Assign { target, op, value } => self.assign(target, *op, value, s.loc),
            StmtKind::For { label, binding, ty, iterable, body } => {
                self.for_loop(label.as_ref(), binding, ty.as_ref(), iterable, body, s.loc)
            }
            StmtKind::While { label, condition, body } => {
                let l = self.label(label.as_ref());
                let c = self.value(condition);
                self.loops.push(LoopCx { label: label.as_ref().map(|l| l.name.clone()), body_label: None });
                let b = self.braced(body, Tail::Unit);
                self.loops.pop();
                quote_spanned!(span=> #l while #c #b)
            }
            StmtKind::DoWhile { label, body, condition } => self.do_while(label.as_ref(), body, condition, s.loc),
            StmtKind::Expr(e) => {
                let t = self.stmt_expr(e);
                if block_like(e) { t } else { quote_spanned!(span=> #t;) }
            }
        }
    }

    /// `'label:` for a loop's label.
    fn label(&self, label: Option<&Ident>) -> TokenStream {
        match label {
            Some(l) => {
                let lt = super::lifetime(&snake(&l.name), self.span(l.loc));
                quote!(#lt:)
            }
            None => quote!(),
        }
    }

    /// An expression as a statement: `x++` an increment, `if`/`when` without values, a call.
    pub(crate) fn stmt_expr(&mut self, e: &Expr) -> TokenStream {
        match &e.kind {
            ExprKind::Postfix { op: PostfixOp::Inc, operand } | ExprKind::Prefix { op: PrefixOp::Inc, operand } => {
                self.step_stmt(operand, true, e.loc)
            }
            ExprKind::Postfix { op: PostfixOp::Dec, operand } | ExprKind::Prefix { op: PrefixOp::Dec, operand } => {
                self.step_stmt(operand, false, e.loc)
            }
            ExprKind::If { condition, then, otherwise } => {
                self.if_expr(condition, then, otherwise.as_ref(), Tail::Unit, e.loc).ts
            }
            ExprKind::When(w) => self.when(w, Tail::Unit, e.loc).ts,
            ExprKind::Return { .. } | ExprKind::Break { .. } | ExprKind::Continue { .. } => self.jump(e),
            ExprKind::Paren(x) => self.stmt_expr(x),
            ExprKind::Annotated { expr, .. } => self.stmt_expr(expr),
            _ => self.expr(e).ts,
        }
    }

    /// `x++`/`x--` (and `++x`, `--x`) as a statement.
    fn step_stmt(&mut self, operand: &Expr, inc: bool, loc: Loc) -> TokenStream {
        let span = self.span(loc);
        let kt = self.kt(span);
        let f = if inc { quote_spanned!(span=> inc) } else { quote_spanned!(span=> dec) };
        match &operand.kind {
            ExprKind::Name(n) => {
                let Some(l) = self.local(&n.name) else {
                    if let Some((r, setter)) = self.receiver_setter(&n.name) {
                        let ri = proc_macro2::Ident::new(&r.rust, span);
                        let read = self.expr(operand).ts;
                        return quote_spanned!(span=> #ri.#setter(#kt::KtInc::#f(#read)));
                    }
                    return self.later(loc, "`++`/`--` on what isn't a local or a property");
                };
                let i = ident(&l.rust, self.span(n.loc));
                if l.kind == LocalKind::Delegated {
                    let g = if inc { quote_spanned!(span=> post_inc) } else { quote_spanned!(span=> post_dec) };
                    return quote_spanned!(span=> #kt::#g(&#i));
                }
                if l.ty.is_number() {
                    if inc { quote_spanned!(span=> #i += 1) } else { quote_spanned!(span=> #i -= 1) }
                } else {
                    quote_spanned!(span=> #i = #kt::KtInc::#f(#i))
                }
            }
            ExprKind::Member { receiver, safe: false, name } => {
                let r = self.expr(receiver).ts;
                let setter = ident(&format!("set_{}", snake(&name.name)), self.span(name.loc));
                let getter = ident(&snake(&name.name), self.span(name.loc));
                quote_spanned!(span=> { let __kz_o = &#r; __kz_o.#setter(#kt::KtInc::#f(__kz_o.#getter())) })
            }
            _ => self.later(loc, "`++`/`--` on this"),
        }
    }

    /// The setter of an implicit receiver's property `kotlin` (`scaleX = 2f` in `graphicsLayer { }`).
    fn receiver_setter(&mut self, kotlin: &str) -> Option<(ImplicitReceiver, proc_macro2::Ident)> {
        let setter = format!("set_{}", snake(kotlin));
        // (a trait's setter too: a scope lent as a trait object, `alpha = 0.5f` in a MutableTransform's block)
        let mut chars = kotlin.chars();
        let kotlin_setter =
            chars.next().map(|c| format!("set{}{}", c.to_uppercase(), chars.as_str())).unwrap_or_default();
        let r = self.receivers.iter().rev().find(|r| {
            crate::index::ITEMS.iter().any(|i| i.owner == r.ty && i.rust == setter)
                || members_of(&r.ty, kotlin).any(|i| i.has(crate::index::flags::SETTER))
                || members_of(&r.ty, &kotlin_setter).any(|i| i.rust == setter && i.params.len() == 1)
        })?;
        let r = r.clone();
        let r = self.use_receiver(&r);
        let span = proc_macro2::Span::call_site();
        Some((r, ident(&setter, span)))
    }

    // ---- Locals ------------------------------------------------------------------------------------------------

    fn local_property(&mut self, p: &Property) -> TokenStream {
        let span = self.span(p.loc);
        if p.receiver.is_some() || p.getter.is_some() || p.setter.is_some() {
            let t = self.later(p.loc, "a local property with a receiver, getter or setter");
            return quote_spanned!(span=> #t;);
        }
        if p.modifiers.has(ModifierKind::Lateinit) {
            let t = self.later(p.loc, "a `lateinit` local");
            return quote_spanned!(span=> #t;);
        }
        let mutable = p.is_var();
        match (&p.binding, &p.init) {
            (Binding::Destructure(entries), PropertyInit::Value(init)) => {
                let v = self.value(init);
                let ty = self.peek_ty(init);
                let d = self.fresh("d", span);
                let mut out = quote_spanned!(span=> let #d = #v;);
                for (k, entry) in entries.iter().enumerate() {
                    if entry.name.name == "_" {
                        continue;
                    }
                    let rust = snake(&entry.name.name);
                    let i = self.name_ident(&rust, entry.name.loc);
                    let c = ident(&format!("component{}", k + 1), span);
                    let ety = match (&ty, k) {
                        (Ty::Pair(a, _), 0) => (**a).clone(),
                        (Ty::Pair(_, b), 1) => (**b).clone(),
                        _ => entry.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown),
                    };
                    let m = if mutable { quote_spanned!(span=> mut) } else { quote!() };
                    out.extend(quote_spanned!(span=> let #m #i = #d.#c();));
                    self.declare(&entry.name.name, rust, if mutable { LocalKind::Var } else { LocalKind::Val }, ety);
                }
                out
            }
            (Binding::Destructure(_), _) => {
                let t = self.later(p.loc, "a destructuring declaration without a value");
                quote_spanned!(span=> #t;)
            }
            (Binding::Name(n), PropertyInit::Delegate(d)) => {
                let ty = self.delegate_ty(d);
                let v = self.value(d);
                let rust = snake(&n.name);
                let i = self.name_ident(&rust, n.loc);
                self.declare(&n.name, rust, LocalKind::Delegated, ty);
                quote_spanned!(span=> let #i = #v;)
            }
            (Binding::Name(n), init) => {
                let declared = p.ty.as_ref().map(|t| (self.ty_of(t), t));
                let rust = snake(&n.name);
                let i = self.name_ident(&rust, n.loc);
                // (a collection held by a `val` is still changed through it: Kotlin's lists are objects)
                let collection = matches!(
                    declared.as_ref().map(|d| d.0.clone()).unwrap_or_else(|| match init {
                        PropertyInit::Value(e) => self.peek_ty(e),
                        _ => Ty::Unknown,
                    }),
                    Ty::List(_) | Ty::Set(_) | Ty::Map(_, _)
                );
                let m = if mutable || collection { quote_spanned!(span=> mut) } else { quote!() };
                let declared_rt = match &declared {
                    Some((ty, t)) if !matches!(ty, Ty::Fun(_)) => Some(self.rust_type(t, super::types::TypePos::Value)),
                    _ => None,
                };
                let annot = match &declared_rt {
                    Some(rt) => quote_spanned!(span=> : #rt),
                    None => quote!(),
                };
                let (value, ty) = match init {
                    PropertyInit::Value(e) => {
                        // (a lambda given a function type: its parameters (and receiver) of that type's, as Kotlin
                        // types them — `val stop: (Animatable<Float, AnimationVector1D>) -> Unit = { it.stop() }`)
                        let o = match (&e.kind, p.ty.as_ref().and_then(|t| t.as_function().filter(|_| !t.nullable))) {
                            (ExprKind::Lambda(l), Some(f)) => {
                                let mut info = self.lambda_info(f, true);
                                info.typed = true;
                                Out::plain(self.lambda(l, Some(&info), None), Ty::Unknown)
                            }
                            _ => self.expr(e),
                        };
                        let ty = match &declared {
                            Some((t, _)) => t.clone(),
                            None => o.ty.clone(),
                        };
                        let wrap = matches!(&declared, Some((Ty::Nullable(_), _)))
                            && o.kind != OutKind::Null
                            && !o.ty.is_nullable()
                            && o.ty != Ty::Unknown;
                        let value_ty = o.ty.clone();
                        let v = self.finish(o, Want::default(), e.loc);
                        let v = match &declared_rt {
                            Some(rt) => self.as_declared(v, &value_ty, rt, span),
                            None => v,
                        };
                        let v = if wrap { quote_spanned!(span=> ::core::option::Option::Some(#v)) } else { v };
                        (quote_spanned!(span=> = #v), ty)
                    }
                    PropertyInit::None => {
                        if declared.is_none() {
                            self.error(n.loc, format!("`{}` needs a type or a value", n.name));
                        }
                        (quote!(), declared.as_ref().map(|d| d.0.clone()).unwrap_or(Ty::Unknown))
                    }
                    PropertyInit::Delegate(_) => unreachable!(),
                };
                self.declare(&n.name, rust, if mutable { LocalKind::Var } else { LocalKind::Val }, ty);
                quote_spanned!(span=> let #m #i #annot #value;)
            }
        }
    }

    /// What a delegate's reads give (`remember { mutableStateOf(0) }` → an `Int`).
    pub(crate) fn delegate_ty(&self, d: &Expr) -> Ty {
        match &d.kind {
            ExprKind::Call { callee, args, trailing_lambda, .. } => {
                let name = match &callee.kind {
                    ExprKind::Name(n) => n.name.as_str(),
                    ExprKind::Member { name, .. } => name.name.as_str(),
                    _ => "",
                };
                match name {
                    "mutableStateOf"
                    | "mutableIntStateOf"
                    | "mutableFloatStateOf"
                    | "mutableLongStateOf"
                    | "mutableDoubleStateOf"
                    | "derivedStateOf"
                    | "lazy"
                    | "rememberUpdatedState"
                    | "mutableStateListOf" => {
                        if let Some(a) = args.first() {
                            return self.peek_ty(&a.value);
                        }
                        if let Some(l) = trailing_lambda {
                            return self.peek_block(&l.body);
                        }
                        Ty::Unknown
                    }
                    "remember" | "rememberSaveable" => match trailing_lambda {
                        Some(l) => match l.body.stmts.last() {
                            Some(Stmt { kind: StmtKind::Expr(e), .. }) => self.delegate_ty(e),
                            _ => Ty::Unknown,
                        },
                        None => Ty::Unknown,
                    },
                    // (any other call giving a state — `animateFloatAsState(…)`, `transition.animateDp { … }`,
                    // `rememberInfiniteTransition().animateFloat(…)` —: what the state holds)
                    _ => state_value_ty(&self.peek_ty(d)),
                }
            }
            _ => state_value_ty(&self.peek_ty(d)),
        }
    }

    // ---- Assignments -------------------------------------------------------------------------------------------

    fn assign(&mut self, target: &Expr, op: AssignOp, value: &Expr, loc: Loc) -> TokenStream {
        let span = self.span(loc);
        let kt = self.kt(span);
        // `target op value` as the new value (`x + v`), its read written by `read`.
        let combined = |me: &mut Self, read: TokenStream, read_ty: &Ty| -> TokenStream {
            let v = if read_ty.is_float()
                && let Some(n) = int_lit(value)
            {
                me.number(n, value.loc, true).ts
            } else {
                me.value(value)
            };
            match op {
                AssignOp::Assign => v,
                AssignOp::Add if read_ty.is_string() || me.peek_ty(value).is_string() && !read_ty.is_number() => {
                    let piece = me.expr(value).ts;
                    quote_spanned!(span=> #kt::template!(#read, #piece))
                }
                AssignOp::Add => quote_spanned!(span=> #read + #v),
                AssignOp::Sub => quote_spanned!(span=> #read - #v),
                AssignOp::Mul => quote_spanned!(span=> #read * #v),
                AssignOp::Div => quote_spanned!(span=> #read / #v),
                AssignOp::Rem => quote_spanned!(span=> #read % #v),
            }
        };
        match &target.kind {
            ExprKind::Name(n) => {
                if let Some(l) = self.local(&n.name) {
                    let i = ident(&l.rust, self.span(n.loc));
                    match l.kind {
                        LocalKind::Delegated => {
                            let read = quote_spanned!(span=> #kt::KtDelegate::get_value(&#i));
                            let v = combined(self, read, &l.ty);
                            return quote_spanned!(span=> #kt::KtMutableDelegate::set_value(&#i, #v););
                        }
                        LocalKind::Val | LocalKind::Param | LocalKind::LambdaParam => {
                            self.error(target.loc, format!("`{}` is a `val`: it can't be assigned", n.name));
                            return quote!();
                        }
                        LocalKind::Var => {}
                    }
                    if op == AssignOp::Assign || l.ty.is_string() {
                        let v = combined(self, quote!(#i), &l.ty);
                        return quote_spanned!(span=> #i = #v;);
                    }
                    let v = if l.ty.is_float()
                        && let Some(n) = int_lit(value)
                    {
                        self.number(n, value.loc, true).ts
                    } else {
                        self.value(value)
                    };
                    return match op {
                        AssignOp::Add => quote_spanned!(span=> #i += #v;),
                        AssignOp::Sub => quote_spanned!(span=> #i -= #v;),
                        AssignOp::Mul => quote_spanned!(span=> #i *= #v;),
                        AssignOp::Div => quote_spanned!(span=> #i /= #v;),
                        _ => quote_spanned!(span=> #i %= #v;),
                    };
                }
                if let Some((r, setter)) = self.receiver_setter(&n.name) {
                    let ri = proc_macro2::Ident::new(&r.rust, span);
                    let setter = proc_macro2::Ident::new(&setter.to_string(), self.span(n.loc));
                    let read_ty = self.peek_ty(target);
                    let read = if op == AssignOp::Assign { quote!() } else { self.expr(target).ts };
                    let v = combined(self, read, &read_ty);
                    return quote_spanned!(span=> #ri.#setter(#v););
                }
                if self.file.vals.contains_key(&n.name) {
                    self.error(target.loc, format!("`{}` is a `val`: it can't be assigned", n.name));
                    return quote!();
                }
                self.error(target.loc, format!("`{}` isn't a local or a property here", n.name));
                quote!()
            }
            ExprKind::Member { receiver, safe, name } => {
                if *safe {
                    let t = self.later(loc, "an assignment through `?.`");
                    return quote_spanned!(span=> #t;);
                }
                let setter = ident(&format!("set_{}", snake(&name.name)), self.span(name.loc));
                let read_ty = self.peek_ty(target);
                let recv = self.expr(receiver);
                if op == AssignOp::Assign {
                    let r = recv.ts;
                    let v = self.value(value);
                    return quote_spanned!(span=> #r.#setter(#v););
                }
                let o = self.fresh("o", span);
                let r = recv.ts;
                let read = self.property_read(Out::plain(quote!(#o), recv.ty), name, target.loc).ts;
                let v = combined(self, read, &read_ty);
                quote_spanned!(span=> { let #o = &#r; #o.#setter(#v); })
            }
            // (Kotlin's `operator fun set` of an API type taking a key: its `set` — `state[key] = value` of a
            // MutableStyleState)
            ExprKind::Index { receiver, indices }
                if op == AssignOp::Assign && super::expr::keyed_operator(&self.peek_ty(receiver), "set") =>
            {
                let mut values: Vec<&Expr> = indices.iter().collect();
                values.push(value);
                let call = self.operator_call(receiver, "set", &values, target.loc).ts;
                quote_spanned!(span=> #call;)
            }
            ExprKind::Index { receiver, indices } => {
                let ty = self.peek_ty(receiver);
                let r = self.expr(receiver).ts;
                let idx: Vec<TokenStream> = indices.iter().map(|i| self.value(i)).collect();
                let read_ty = ty.element();
                let read = if op == AssignOp::Assign { quote!() } else { self.expr(target).ts };
                let v = combined(self, read, &read_ty);
                match ty {
                    Ty::Map(_, _) => quote_spanned!(span=> #r.set(#(#idx,)* #v);),
                    // (a state list: its own `set`, taking `&self` and a `usize`)
                    Ty::Named(n, _) if n == "SnapshotStateList" => {
                        quote_spanned!(span=> #r.set(#((#idx) as usize,)* #v);)
                    }
                    _ => quote_spanned!(span=> #kt::KtMutableList::set(&mut #r, #(#idx,)* #v);),
                }
            }
            _ => {
                let t = self.later(loc, "an assignment to this");
                quote_spanned!(span=> #t;)
            }
        }
    }

    // ---- Loops -------------------------------------------------------------------------------------------------

    fn for_loop(
        &mut self,
        label: Option<&Ident>,
        binding: &Binding,
        ty: Option<&Type>,
        iterable: &Expr,
        body: &Block,
        loc: Loc,
    ) -> TokenStream {
        let span = self.span(loc);
        let kt = self.kt(span);
        let l = self.label(label);
        let it_ty = self.peek_ty(iterable);
        let is_range = matches!(it_ty, Ty::Range(_))
            || matches!(&iterable.kind, ExprKind::Infix { name, .. } if matches!(name.name.as_str(), "until" | "downTo" | "step"))
            || matches!(&iterable.kind, ExprKind::Binary { op: BinaryOp::RangeTo | BinaryOp::RangeUntil, .. });
        let iter = if is_range {
            self.value(iterable)
        } else {
            let o = self.expr(iterable).ts;
            quote_spanned!(span=> #kt::KtIterable::kt_iter(&#o))
        };
        let elem = match ty {
            Some(t) => self.ty_of(t),
            None => it_ty.element(),
        };
        self.push_scope();
        let (pat, unpack) = match binding {
            Binding::Name(n) => {
                let rust = snake(&n.name);
                let i = self.name_ident(&rust, n.loc);
                self.declare(&n.name, rust, LocalKind::Val, elem);
                (quote!(#i), quote!())
            }
            Binding::Destructure(entries) => {
                let d = self.fresh("d", span);
                let mut unpack = TokenStream::new();
                for (k, e) in entries.iter().enumerate() {
                    if e.name.name == "_" {
                        continue;
                    }
                    let rust = snake(&e.name.name);
                    let i = self.name_ident(&rust, e.name.loc);
                    let c = ident(&format!("component{}", k + 1), span);
                    unpack.extend(quote_spanned!(span=> let #i = #d.#c();));
                    let ety = match (&elem, k) {
                        (Ty::Pair(a, _), 0) => (**a).clone(),
                        (Ty::Pair(_, b), 1) => (**b).clone(),
                        (Ty::Named(n, args), 1) if n == "IndexedValue" => args.first().cloned().unwrap_or(Ty::Unknown),
                        (Ty::Named(n, _), 0) if n == "IndexedValue" => Ty::Int,
                        _ => Ty::Unknown,
                    };
                    self.declare(&e.name.name, rust, LocalKind::Val, ety);
                }
                (quote!(#d), unpack)
            }
        };
        self.loops.push(LoopCx { label: label.map(|l| l.name.clone()), body_label: None });
        let b = self.block_inner(body, Tail::Unit);
        self.loops.pop();
        self.pop_scope();
        quote_spanned!(span=> #l for #pat in #iter { #unpack #b })
    }

    /// `do { body } while (condition)`: `loop { body; if !condition { break } }` (the body's locals seen by the
    /// condition, as Kotlin's are); with a `continue` in it, the body is a labeled block it leaves.
    fn do_while(&mut self, label: Option<&Ident>, body: &Block, condition: &Expr, loc: Loc) -> TokenStream {
        let span = self.span(loc);
        let l = self.label(label);
        let has_continue = continues(&body.stmts);
        let body_label = has_continue.then(|| self.counter_label());
        self.loops.push(LoopCx { label: label.map(|l| l.name.clone()), body_label: body_label.clone() });
        self.push_scope();
        let b = self.stmts(&body.stmts, Tail::Unit);
        self.loops.pop();
        let c = self.value(condition);
        self.pop_scope();
        match body_label {
            Some(bl) => {
                let lt = super::lifetime(&bl, span);
                quote_spanned!(span=> #l loop { #lt: { #b } if !(#c) { break; } })
            }
            None => quote_spanned!(span=> #l loop { #b if !(#c) { break; } }),
        }
    }

    fn counter_label(&mut self) -> String {
        let i = self.fresh("body", proc_macro2::Span::call_site());
        i.to_string()
    }

    /// `return`, `break`, `continue` (labeled or not).
    pub(crate) fn jump(&mut self, e: &Expr) -> TokenStream {
        let span = self.span(e.loc);
        match &e.kind {
            ExprKind::Return { label, value } => {
                match (label, self.lambdas.last()) {
                    (None, Some(_)) => {
                        return self.later(e.loc, "a `return` from inside a lambda (Kotlin's non-local return)");
                    }
                    (Some(l), _) => {
                        let ok = self.lambdas.last().is_some_and(|c| c.labels.contains(&l.name));
                        if !ok {
                            return self
                                .later(e.loc, &format!("`return@{}` out of more than the lambda it is in", l.name));
                        }
                    }
                    (None, None) => {}
                }
                match value {
                    Some(v) => {
                        let want = self.lambdas.last().map(|l| l.result).unwrap_or_default();
                        let v = self.value_want(v, want);
                        quote_spanned!(span=> return #v)
                    }
                    None => quote_spanned!(span=> return),
                }
            }
            ExprKind::Break { label } | ExprKind::Continue { label } => {
                let is_break = matches!(e.kind, ExprKind::Break { .. });
                let inside = self.lambdas.last().map_or(0, |l| l.loops);
                let target = match label {
                    None => self.loops.len().checked_sub(1),
                    Some(l) => self.loops.iter().rposition(|x| x.label.as_deref() == Some(l.name.as_str())),
                };
                let Some(t) = target.filter(|t| *t >= inside) else {
                    return match label {
                        Some(l) => {
                            self.error(e.loc, format!("no loop labeled `{}` around it", l.name));
                            quote!()
                        }
                        None => self.later(e.loc, "`break`/`continue` outside a loop (or out of a lambda)"),
                    };
                };
                let lp = self.loops[t].clone();
                if !is_break && let Some(bl) = &lp.body_label {
                    let lt = super::lifetime(bl, span);
                    return quote_spanned!(span=> break #lt);
                }
                let lt = label.as_ref().map(|l| super::lifetime(&snake(&l.name), self.span(l.loc)));
                if is_break { quote_spanned!(span=> break #lt) } else { quote_spanned!(span=> continue #lt) }
            }
            _ => unreachable!(),
        }
    }

    // ---- if / when ---------------------------------------------------------------------------------------------

    /// `if (condition) then else otherwise`: a smart cast for `x != null` (`x == null`) on a local makes it
    /// `if let Some(x) = x.clone()`.
    pub(crate) fn if_expr(
        &mut self,
        condition: &Expr,
        then: &Block,
        otherwise: Option<&Block>,
        tail: Tail,
        loc: Loc,
    ) -> Out {
        let span = self.span(loc);
        let ty = if matches!(tail, Tail::Unit) {
            Ty::Unit
        } else {
            let a = self.peek_block(then);
            if a == Ty::Unknown { otherwise.map(|o| self.peek_block(o)).unwrap_or(Ty::Unit) } else { a }
        };
        let tail = if otherwise.is_none() { Tail::Unit } else { tail };
        // (x == null) with an else: the else branch is the narrowed one
        if let ExprKind::Binary { op: BinaryOp::Eq, lhs, rhs } = &condition.kind
            && matches!(rhs.kind, ExprKind::Null)
            && let ExprKind::Name(n) = &lhs.kind
            && let Some(o) = otherwise
            && self.narrowable(&n.name)
        {
            let l = self.local(&n.name).expect("a local");
            let i = ident(&l.rust, self.span(n.loc));
            let a = self.braced(then, tail);
            self.push_scope();
            self.narrow(&n.name, l.ty.non_null());
            let b = self.braced(o, tail);
            self.pop_scope();
            return Out::plain(
                quote_spanned!(span=> if let ::core::option::Option::Some(#i) = #i.clone() #b else #a),
                ty,
            );
        }
        self.push_scope();
        let c = self.condition(condition);
        let a = self.braced(then, tail);
        self.pop_scope();
        let ts = match otherwise {
            Some(o) => {
                let b = match o.stmts.as_slice() {
                    [Stmt { kind: StmtKind::Expr(e @ Expr { kind: ExprKind::If { .. }, .. }), .. }] if !o.braced => {
                        let ExprKind::If { condition, then, otherwise } = &e.kind else { unreachable!() };
                        self.if_expr(condition, then, otherwise.as_ref(), tail, e.loc).ts
                    }
                    _ => self.braced(o, tail),
                };
                quote_spanned!(span=> if #c #a else #b)
            }
            None => quote_spanned!(span=> if #c #a),
        };
        Out::plain(ts, ty)
    }

    /// Whether a name is a local whose null check smart-casts it.
    fn narrowable(&self, name: &str) -> bool {
        self.scopes.iter().rev().find_map(|s| s.get(name)).is_some_and(|l| {
            matches!(l.kind, LocalKind::Val | LocalKind::Param | LocalKind::LambdaParam | LocalKind::Var)
                && (l.ty.is_nullable() || l.ty == Ty::Unknown)
        })
    }

    /// An `if`'s condition: `x != null` conjuncts on locals made `let Some(x) = x.clone()` (narrowing `x` in the
    /// scope the caller opened for the then branch).
    pub(crate) fn condition(&mut self, c: &Expr) -> TokenStream {
        let span = self.span(c.loc);
        match &c.kind {
            ExprKind::Binary { op: BinaryOp::And, lhs, rhs } if has_null_check(c) => {
                let a = self.condition(lhs);
                let b = self.condition(rhs);
                quote_spanned!(span=> #a && #b)
            }
            ExprKind::Binary { op: BinaryOp::NotEq, lhs, rhs }
                if matches!(rhs.kind, ExprKind::Null)
                    && matches!(&lhs.kind, ExprKind::Name(n) if self.narrowable(&n.name)) =>
            {
                let ExprKind::Name(n) = &lhs.kind else { unreachable!() };
                let l = self.local(&n.name).expect("a local");
                let i = ident(&l.rust, self.span(n.loc));
                self.narrow(&n.name, l.ty.non_null());
                quote_spanned!(span=> let ::core::option::Option::Some(#i) = #i.clone())
            }
            _ => self.value(c),
        }
    }

    /// `when`: an `if` chain on the subject (bound once).
    pub(crate) fn when(&mut self, w: &When, tail: Tail, loc: Loc) -> Out {
        let span = self.span(loc);
        let kt = self.kt(span);
        let ty = if matches!(tail, Tail::Unit) {
            Ty::Unit
        } else {
            w.branches.iter().map(|b| self.peek_block(&b.body)).find(|t| *t != Ty::Unknown).unwrap_or(Ty::Unknown)
        };
        self.push_scope();
        let mut prelude = TokenStream::new();
        let subject = match &w.subject {
            Some(s) => {
                let v = self.value(&s.expr);
                let sty = self.peek_ty(&s.expr);
                let i = match &s.binding {
                    Some((n, _)) => {
                        let rust = snake(&n.name);
                        let i = self.name_ident(&rust, n.loc);
                        self.declare(&n.name, rust, LocalKind::Val, sty.clone());
                        i
                    }
                    None => self.fresh("w", span),
                };
                prelude.extend(quote_spanned!(span=> let #i = #v;));
                Some((i, sty))
            }
            None => None,
        };
        let mut chain = TokenStream::new();
        let mut has_else = false;
        for (k, b) in w.branches.iter().enumerate() {
            let body = self.braced(&b.body, tail);
            if b.is_else() {
                has_else = true;
                if k == 0 {
                    chain.extend(quote!(#body));
                } else {
                    chain.extend(quote_spanned!(span=> else #body));
                }
                break;
            }
            let mut conds = Vec::new();
            for c in &b.conditions {
                conds.push(match (c, &subject) {
                    (WhenCondition::Expr(e), Some((s, sty))) => {
                        let v = if sty.is_float()
                            && let Some(n) = int_lit(e)
                        {
                            self.number(n, e.loc, true).ts
                        } else {
                            self.value_want(e, Want { str_ok: true, any: None, some: false, shared: None })
                        };
                        quote_spanned!(span=> #s == #v)
                    }
                    (WhenCondition::Expr(e), None) => self.value(e),
                    (WhenCondition::In { negated, range, .. }, Some((s, _))) => {
                        let r = self.expr(range).ts;
                        let t = quote_spanned!(span=> #kt::is_in(&#s, &#r));
                        if *negated { quote_spanned!(span=> !#t) } else { t }
                    }
                    (WhenCondition::In { loc, .. }, None) => {
                        self.error(*loc, "`in` in a `when` without a subject");
                        quote!(false)
                    }
                    (WhenCondition::Is { loc, .. }, _) => self.later(*loc, "an `is` branch"),
                });
            }
            if let Some(g) = &b.guard {
                let g = self.value(g);
                let any = quote_spanned!(span=> (#(#conds)||*));
                conds = vec![quote_spanned!(span=> #any && #g)];
            }
            let kw = if k == 0 { quote_spanned!(span=> if) } else { quote_spanned!(span=> else if) };
            chain.extend(quote_spanned!(span=> #kw #(#conds)||* #body));
        }
        if !has_else && !matches!(tail, Tail::Unit) {
            chain.extend(quote_spanned!(span=> else { ::core::unreachable!("no `when` branch matched") }));
        }
        self.pop_scope();
        let ts = if prelude.is_empty() { chain } else { quote_spanned!(span=> { #prelude #chain }) };
        Out::plain(ts, ty)
    }
}

/// Whether an expression statement is written as a block (no `;` after it).
fn block_like(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::If { .. } | ExprKind::When(_) => true,
        ExprKind::Annotated { expr, .. } | ExprKind::Paren(expr) => block_like(expr),
        _ => false,
    }
}

/// Whether an expression statement gives a value as a block's last one (an assignment-like one, a loop, an `if`
/// without `else` don't).
fn gives_value(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::If { otherwise: None, .. } => false,
        ExprKind::Return { .. } | ExprKind::Break { .. } | ExprKind::Continue { .. } | ExprKind::Throw(_) => false,
        ExprKind::Annotated { expr, .. } => gives_value(expr),
        _ => true,
    }
}

fn int_lit(e: &Expr) -> Option<&Number> {
    match &e.kind {
        ExprKind::Number(n) if n.kind == NumberKind::Int && n.radix == 10 => Some(n),
        _ => None,
    }
}

/// Whether a condition has a `x != null` among its `&&`-ed parts.
fn has_null_check(c: &Expr) -> bool {
    match &c.kind {
        ExprKind::Binary { op: BinaryOp::And, lhs, rhs } => has_null_check(lhs) || has_null_check(rhs),
        ExprKind::Binary { op: BinaryOp::NotEq, lhs, rhs } => {
            matches!(rhs.kind, ExprKind::Null) && matches!(lhs.kind, ExprKind::Name(_))
        }
        _ => false,
    }
}

/// Whether statements have a `continue` for the loop they are the body of (not one of a loop inside them).
fn continues(stmts: &[Stmt]) -> bool {
    fn in_expr(e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Continue { label: None } => true,
            ExprKind::If { then, otherwise, .. } => {
                continues(&then.stmts) || otherwise.as_ref().is_some_and(|o| continues(&o.stmts))
            }
            ExprKind::When(w) => w.branches.iter().any(|b| continues(&b.body.stmts)),
            ExprKind::Binary { lhs, rhs, .. } => in_expr(lhs) || in_expr(rhs),
            ExprKind::Paren(x) | ExprKind::Annotated { expr: x, .. } => in_expr(x),
            _ => false,
        }
    }
    stmts.iter().any(|s| match &s.kind {
        StmtKind::Expr(e) => in_expr(e),
        _ => false,
    })
}

/// What a state of type `t` holds (`State<f32>` → `Float`), as a delegate reads it; unknown for anything else.
fn state_value_ty(t: &Ty) -> Ty {
    match t {
        Ty::Named(n, a)
            if a.len() == 1
                && matches!(
                    n.as_str(),
                    "State" | "MutableState" | "DerivedState" | "UpdatedState" | "AnimationState"
                ) =>
        {
            a[0].clone()
        }
        _ => Ty::Unknown,
    }
}
