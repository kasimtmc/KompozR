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
    fn want(self, fun: &super::FnState) -> Want {
        match self {
            Tail::Value(w) => w,
            Tail::Return => Want { some: fun.nullable_result, shared: fun.shared_result, ..Want::default() },
            Tail::Unit => Want::default(),
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
        let inner = self.in_range(b.loc, |g| g.block_inner(b, tail));
        quote_spanned!(span=> { #inner })
    }

    pub(crate) fn stmts(&mut self, stmts: &[Stmt], tail: Tail) -> TokenStream {
        let mut out = TokenStream::new();
        for (k, s) in stmts.iter().enumerate() {
            let last = k + 1 == stmts.len();
            match (&s.kind, last, tail) {
                (StmtKind::Expr(e), true, Tail::Value(_) | Tail::Return) if gives_value(e) => {
                    out.extend(self.value_want(e, tail.want(&self.fun)));
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
            // ((stage 14 WPB) a closure: `capture`)
            StmtKind::Decl(Decl::Function(f)) => self.local_function(f),
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
                let c = self.value(condition);
                let cx = LoopCx { label: label.as_ref().map(|l| l.name.clone()), ..Default::default() };
                let named = cx.named.clone();
                self.loops.push(cx);
                let b = self.braced(body, Tail::Unit);
                self.loops.pop();
                let l = self.loop_label(label.as_ref(), &named, span);
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
            ExprKind::Try(t) => self.try_expr(t, Tail::Unit, e.loc).ts,
            ExprKind::Return { .. } | ExprKind::Break { .. } | ExprKind::Continue { .. } => self.jump(e),
            ExprKind::Paren(x) => self.stmt_expr(x),
            ExprKind::Annotated { expr, .. } => self.stmt_expr(expr),
            _ => self.expr(e).ts,
        }
    }

    /// `x++`/`x--` (and `++x`, `--x`) as a statement.
    fn step_stmt(&mut self, operand: &Expr, inc: bool, loc: Loc) -> TokenStream {
        // ((stage 14 WPB) an operator function of the user)
        if let Some(ts) = self.user_step_stmt(operand, inc, loc) {
            return ts;
        }
        let span = self.span(loc);
        let kt = self.kt(span);
        let f = if inc { quote_spanned!(span=> inc) } else { quote_spanned!(span=> dec) };
        match &operand.kind {
            ExprKind::Name(n) => {
                let Some(l) = self.local(&n.name) else {
                    if let Some((r, setter)) = self.receiver_setter(&n.name) {
                        let ri = crate::codegen::names::ident(&r.rust, span);
                        let read = self.expr(operand).ts;
                        return quote_spanned!(span=> #ri.#setter(#kt::KtInc::#f(#read)));
                    }
                    // ((stage 14 WPB) a top-level `var`)
                    if let Some(o) = self.file_var_step(n, inc, None, loc) {
                        return o.ts;
                    }
                    return self.later(loc, "`++`/`--` on what isn't a local or a property");
                };
                let i = ident(&l.rust, self.span(n.loc));
                if l.kind == LocalKind::Delegated {
                    let g = if inc { quote_spanned!(span=> post_inc) } else { quote_spanned!(span=> post_dec) };
                    return quote_spanned!(span=> #kt::#g(&#i));
                }
                self.note_assigned(&l);
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
            // `a[i]++`: Kotlin's `a.set(i, a.get(i).inc())`, the index evaluated once (a list's, a state list's; a
            // map's `get` is nullable, its `m[k]++` no Kotlin)
            ExprKind::Index { receiver, indices }
                if indices.len() == 1
                    && !matches!(self.peek_ty(receiver), Ty::Map(..) | Ty::String)
                    && !super::expr::keyed_operator(&self.peek_ty(receiver), "get") =>
            {
                let ty = self.peek_ty(receiver);
                let r = self.expr(receiver).ts;
                let index = self.value(&indices[0]);
                let i = self.fresh("i", span);
                let v = self.fresh("v", span);
                let set = match ty {
                    // (a state list: its own `set`, taking `&self` and a `usize`)
                    Ty::Named(n, _) if n == "SnapshotStateList" => quote_spanned!(span=> #r.set((#i) as usize, #v)),
                    _ => quote_spanned!(span=> #kt::KtMutableList::set(&mut #r, #i, #v)),
                };
                quote_spanned!(span=> {
                    let #i = #index;
                    let #v = #kt::KtInc::#f(#kt::KtList::get(&#r, #i));
                    #set;
                })
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
        let decl = self.local_property_decl(p);
        // Kotlin's `@Suppress("UNUSED_VARIABLE")` (or `"unused"`) on a local: Rust's unused-variable lint is kept quiet
        // for it too (by a use of it), as Kotlin's compiler is.
        if let Binding::Name(n) = &p.binding
            && suppresses_unused(&p.annotations)
        {
            let span = self.span(p.loc);
            let i = self.name_ident(&snake(&n.name), n.loc);
            return quote_spanned!(span=> #decl let _ = &#i;);
        }
        decl
    }

    fn local_property_decl(&mut self, p: &Property) -> TokenStream {
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
                        // (a state's value and its setter: `val (checked, onCheckedChange) = remember { … }`)
                        (Ty::Named(n, a), 0) if n == "MutableState" && a.len() == 1 => a[0].clone(),
                        (Ty::Named(n, a), 1) if n == "MutableState" && a.len() == 1 => Ty::Fun(Box::new(FunTy {
                            params: vec![a[0].clone()],
                            ret: Ty::Unit,
                            receiver: None,
                            suspend: false,
                            shared: false,
                        })),
                        _ => entry.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown),
                    };
                    let m = if mutable { quote_spanned!(span=> mut) } else { quote!() };
                    out.extend(quote_spanned!(span=> let #m #i = #d.#c();));
                    // (a state's setter is a cloneable closure, not a `Copy` one: cloned when used as a value —
                    // `onExpandedChange = setExpanded` and `{ setExpanded(false) }` both)
                    let setter = k == 1 && matches!(&ty, Ty::Named(n, _) if n == "MutableState");
                    self.declare_at(&entry.name, rust, if mutable { LocalKind::Var } else { LocalKind::Val }, ety);
                    if setter && let Some(l) = self.scopes.last_mut().and_then(|s| s.get_mut(&entry.name.name)) {
                        l.shared_closure = true;
                    }
                }
                out
            }
            (Binding::Destructure(_), _) => {
                let t = self.later(p.loc, "a destructuring declaration without a value");
                quote_spanned!(span=> #t;)
            }
            (Binding::Name(n), PropertyInit::Delegate(d)) => {
                // (a declared type is what a `mutableStateOf` it is delegated to holds: `mutable_state_of!` reads it)
                let hint =
                    p.ty.as_ref()
                        .filter(|t| !matches!(self.ty_of(t), Ty::Fun(_)))
                        .map(|t| (self.rust_type(t, super::types::TypePos::Value), self.ty_of(t)));
                let outer = std::mem::replace(&mut self.state_value, hint);
                let v = self.value(d);
                self.state_value = outer;
                // (its declared type what the delegate holds: `var checkedIndex: Int? by remember { mutableStateOf(null)
                // }`, Rust told it where `null` alone can't; a declared function type isn't one Rust can name: the
                // delegate's own)
                let declared = p.ty.as_ref().filter(|t| !matches!(self.ty_of(t), Ty::Fun(_)));
                let (ty, v) = match declared {
                    Some(t) => {
                        let kt = self.kt(span);
                        let rt = self.rust_type(t, super::types::TypePos::Value);
                        (self.ty_of(t), quote_spanned!(span=> #kt::delegate_of::<#rt, _>(#v)))
                    }
                    None => (self.delegate_ty(d), v),
                };
                let rust = snake(&n.name);
                let i = self.name_ident(&rust, n.loc);
                self.declare_at(n, rust, LocalKind::Delegated, ty);
                self.record_mutable(mutable);
                quote_spanned!(span=> let #i = #v;)
            }
            (Binding::Name(n), init) => {
                let declared = p.ty.as_ref().map(|t| (self.ty_of(t), t));
                let rust = snake(&n.name);
                let i = self.name_ident(&rust, n.loc);
                // (a collection held by a `val` is still changed through it: Kotlin's lists are objects; so is a value
                // of a type changed in place — `val path = Path()`, then `path.lineTo(…)`: a `&mut self` method)
                let value_ty = declared.as_ref().map(|d| d.0.clone()).unwrap_or_else(|| match init {
                    PropertyInit::Value(e) => self.peek_ty(e),
                    _ => Ty::Unknown,
                });
                let collection = matches!(value_ty, Ty::List(_) | Ty::Set(_) | Ty::Map(_, _))
                    || matches!(&value_ty, Ty::Named(n, _) if super::resolve::changed_in_place(n));
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
                        let declared_fn = p.ty.as_ref().map(|t| self.unaliased(t));
                        let o =
                            match (&e.kind, declared_fn.as_ref().and_then(|t| t.as_function().filter(|_| !t.nullable)))
                            {
                                (ExprKind::Lambda(l), Some(f)) => {
                                    let mut info = self.lambda_info(f, true);
                                    info.typed = true;
                                    Out::plain(self.lambda(l, Some(&info), None), Ty::Unknown)
                                }
                                _ => {
                                    // (a list of a declared type: its elements told it — `val icons: List<ImageVector?> =
                                    // listOf(null, null)`)
                                    if let Some((Ty::List(inner) | Ty::Set(inner), _)) = &declared
                                        && super::call::list_of_call(e).is_some()
                                    {
                                        self.expected_elem = Some((**inner).clone());
                                    }
                                    // ((stage 14 WPB) a callable reference: the declared function type's parameters
                                    // tell which overload it names)
                                    let expected = match &declared {
                                        Some((Ty::Fun(f), _)) => Some(f.params.clone()),
                                        _ => None,
                                    };
                                    let outer = std::mem::replace(&mut self.control.expected_fn, expected);
                                    let o = self.expr(e);
                                    self.control.expected_fn = outer;
                                    self.expected_elem = None;
                                    o
                                }
                            };
                        // (a shared closure kept under a declared function type is still one: `val t: (Float) ->
                        // Float = SwipeToDismissBoxDefaults.positionalThreshold`)
                        let ty = match (&declared, &o.ty) {
                            (Some((Ty::Fun(d), _)), Ty::Fun(f)) if f.shared => {
                                let mut d = d.clone();
                                d.shared = true;
                                Ty::Fun(d)
                            }
                            (Some((t, _)), _) => t.clone(),
                            (None, _) => o.ty.clone(),
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
                // (a shared closure an API gave, an `Rc`: cloned when used as a value; (WP4a: scaffold) so is a lambda
                // kept in a `val`, a closure moving its captures in — Kotlin's function value, given to several
                // parameters: `val onRefresh: () -> Unit = { … }` an action's `onClick` and the box's `onRefresh`)
                let lambda_value = matches!(init, PropertyInit::Value(e) if matches!(e.kind, ExprKind::Lambda(_)));
                let shared_closure = matches!(&ty, Ty::Fun(f) if f.shared) || lambda_value;
                let deferred = !mutable && matches!(init, PropertyInit::None);
                // ((stage 14 WPB) a `var` a kept lambda or a local function assigns: a shared cell, `capture`)
                if mutable && self.is_shared_var(n.loc) && !value.is_empty() {
                    let kt = self.kt(span);
                    let v: TokenStream = value.into_iter().skip(1).collect();
                    let cell = match &declared_rt {
                        Some(rt) => quote_spanned!(span=> #kt::Ref::<#rt>::new(#v)),
                        None => quote_spanned!(span=> #kt::Ref::new(#v)),
                    };
                    self.declare_at(n, rust, LocalKind::Delegated, ty);
                    return quote_spanned!(span=> let #i = #cell;);
                }
                self.declare_at(n, rust, if mutable { LocalKind::Var } else { LocalKind::Val }, ty);
                if let Some(l) = self.scopes.last_mut().and_then(|s| s.get_mut(&n.name)) {
                    l.shared_closure = shared_closure;
                    l.deferred = deferred;
                }
                quote_spanned!(span=> let #m #i #annot #value;)
            }
        }
    }

    /// What a delegate's reads give (`remember { mutableStateOf(0) }` → an `Int`).
    pub(crate) fn delegate_ty(&self, d: &Expr) -> Ty {
        match &d.kind {
            ExprKind::Call { callee, args, trailing_lambda, type_args } => {
                let name = match &callee.kind {
                    ExprKind::Name(n) => n.name.as_str(),
                    ExprKind::Member { name, .. } => name.name.as_str(),
                    _ => "",
                };
                // (a state's type argument where written: `mutableStateOf<Animatable<Dp, …>?>(null)`)
                if let Some(crate::ast::TypeArg::Type { ty: t, .. }) =
                    type_args.first().filter(|_| matches!(name, "mutableStateOf" | "mutableStateListOf"))
                {
                    return self.ty_of(t);
                }
                match name {
                    "mutableStateOf"
                    | "mutableIntStateOf"
                    | "mutableFloatStateOf"
                    | "mutableLongStateOf"
                    | "mutableDoubleStateOf"
                    | "derivedStateOf"
                    | "lazy"
                    // ((stage 14 WPB) a saved state's delegate: what its block gives)
                    | "saved"
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

    pub(crate) fn assign(&mut self, target: &Expr, op: AssignOp, value: &Expr, loc: Loc) -> TokenStream {
        // ((stage 14 WPB) an operator function of the user: `a += b`, `a[i] = v`)
        if let Some(ts) = self.user_assign(target, op, value, loc) {
            return ts;
        }
        let span = self.span(loc);
        let kt = self.kt(span);
        // `target op value` as the new value (`x + v`), its read written by `read`.
        let combined = |me: &mut Self, read: TokenStream, read_ty: &Ty| -> TokenStream {
            let v = if read_ty.is_float()
                && let Some(n) = int_lit(value)
            {
                me.number(n, value.loc, true).ts
            } else if op == AssignOp::Assign && read_ty.is_nullable() {
                // (a value to a nullable place: in `Some` unless it is nullable itself)
                me.value_want(value, Want { some: true, ..Want::default() })
            } else {
                me.value(value)
            };
            match op {
                // (a case of a sealed class, a class's value of an interface: the place's type's, stage 14 WPA)
                AssignOp::Assign => {
                    let from = me.peek_ty(value);
                    me.coerce(v, &from, read_ty, span)
                }
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
                        // (a `val` declared without a value takes its one value here: rustc checks it is given once,
                        // as Kotlin does)
                        LocalKind::Val if l.deferred && op == AssignOp::Assign => {}
                        LocalKind::Val | LocalKind::Param | LocalKind::LambdaParam => {
                            self.error(target.loc, format!("`{}` is a `val`: it can't be assigned", n.name));
                            return quote!();
                        }
                        // ((stage 14 WPB) assigned from a kept lambda: shared, `capture`)
                        LocalKind::Var => self.note_assigned(&l),
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
                    let ri = crate::codegen::names::ident(&r.rust, span);
                    let setter = proc_macro2::Ident::new(&setter.to_string(), self.span(n.loc));
                    let read_ty = self.peek_ty(target);
                    let read = if op == AssignOp::Assign { quote!() } else { self.expr(target).ts };
                    let v = combined(self, read, &read_ty);
                    return quote_spanned!(span=> #ri.#setter(#v););
                }
                // ((stage 14 WPB) a top-level `var`)
                if let Some(ts) = self.file_var_assign(n, op, value, loc) {
                    return ts;
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
                // (a property of an API type taking a function — `state.onValueChange = { … }`, a nullable callback
                // `Option<Rc<dyn Fn(f32)>>` —: its setter called as a call is, the lambda made what it takes)
                let kotlin_setter = format!("set{}{}", name.name[..1].to_uppercase(), &name.name[1..]);
                let takes_function = self.peek_ty(receiver).owner().is_some_and(|o| {
                    members_of(o, &kotlin_setter).any(|i| i.params.len() == 1 && i.params[0].lambda.is_some())
                });
                if op == AssignOp::Assign && takes_function {
                    let call = Expr {
                        kind: ExprKind::Call {
                            callee: Box::new(Expr {
                                kind: ExprKind::Member {
                                    receiver: Box::new(receiver.as_ref().clone()),
                                    safe: false,
                                    name: Ident { name: kotlin_setter, backticked: false, loc: name.loc },
                                },
                                loc: target.loc,
                            }),
                            type_args: Vec::new(),
                            args: vec![Arg { name: None, spread: false, value: value.clone(), loc: value.loc }],
                            trailing_lambda: None,
                        },
                        loc,
                    };
                    let c = self.expr(&call).ts;
                    return quote_spanned!(span=> #c;);
                }
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
        } else if it_ty == Ty::String {
            // (a text's characters: Kotlin's `for (c in text)` over a CharSequence)
            let o = self.expr(iterable).ts;
            quote_spanned!(span=> ::core::primitive::str::chars(&#o).collect::<::std::vec::Vec<char>>())
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
                self.declare_at(n, rust, LocalKind::Val, elem);
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
                    self.declare_at(&e.name, rust, LocalKind::Val, ety);
                }
                (quote!(#d), unpack)
            }
        };
        let cx = LoopCx { label: label.map(|l| l.name.clone()), ..Default::default() };
        let named = cx.named.clone();
        self.loops.push(cx);
        let b = self.block_inner(body, Tail::Unit);
        self.loops.pop();
        self.pop_scope();
        let l = if l.is_empty() { self.loop_label(None, &named, span) } else { l };
        quote_spanned!(span=> #l for #pat in #iter { #unpack #b })
    }

    /// A loop's label: Kotlin's, or the one a jump from a lambda written as a loop inside gave it (`jumps`).
    pub(crate) fn loop_label(
        &self,
        label: Option<&Ident>,
        named: &std::rc::Rc<std::cell::RefCell<Option<String>>>,
        span: proc_macro2::Span,
    ) -> TokenStream {
        match (label, named.borrow().as_ref()) {
            (Some(_), _) => self.label(label),
            (None, Some(n)) => {
                let lt = super::lifetime(n, span);
                quote!(#lt:)
            }
            (None, None) => quote!(),
        }
    }

    /// `do { body } while (condition)`: `loop { body; if !condition { break } }` (the body's locals seen by the
    /// condition, as Kotlin's are); with a `continue` in it, the body is a labeled block it leaves.
    fn do_while(&mut self, label: Option<&Ident>, body: &Block, condition: &Expr, loc: Loc) -> TokenStream {
        let span = self.span(loc);
        let has_continue = continues(&body.stmts);
        let body_label = has_continue.then(|| self.counter_label());
        let cx = LoopCx { label: label.map(|l| l.name.clone()), body_label: body_label.clone(), ..Default::default() };
        let named = cx.named.clone();
        self.loops.push(cx);
        self.push_scope();
        let b = self.stmts(&body.stmts, Tail::Unit);
        self.loops.pop();
        let l = self.loop_label(label, &named, span);
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
                // ((stage 14 WPB) `return@forEach` of a lambda written as a loop: its next turn, `jumps`)
                if let Some(l) = label
                    && let Some(t) = self.lowered_loop(&l.name)
                {
                    if value.is_some() {
                        self.error(e.loc, format!("`return@{}` gives nothing (its lambda gives `Unit`)", l.name));
                    }
                    return self.loop_jump_tokens(t, false, None, span);
                }
                match (label, self.lambdas.last()) {
                    (None, Some(_)) => {
                        // ((stage 14 WPB) out of an inline lambda written as a loop: `jumps`)
                        self.control.escaped = true;
                        return self.later(e.loc, "a `return` from inside a lambda (Kotlin's non-local return)");
                    }
                    (Some(l), _) => {
                        let ok = self.lambdas.last().is_some_and(|c| c.labels.contains(&l.name));
                        if !ok {
                            self.control.escaped = true;
                            return self
                                .later(e.loc, &format!("`return@{}` out of more than the lambda it is in", l.name));
                        }
                    }
                    (None, None) => {}
                }
                match value {
                    Some(v) => {
                        let want = match self.lambdas.last() {
                            Some(l) => l.result,
                            None => Want {
                                some: self.fun.nullable_result,
                                shared: self.fun.shared_result,
                                ..Want::default()
                            },
                        };
                        let v = self.value_want(v, want);
                        self.return_tokens(Some(v), span)
                    }
                    None => self.return_tokens(None, span),
                }
            }
            ExprKind::Break { label } | ExprKind::Continue { label } => {
                let is_break = matches!(e.kind, ExprKind::Break { .. });
                let inside = self.lambdas.last().map_or(0, |l| l.loops);
                // ((stage 14 WPB) not a lambda's loop: Kotlin's loop around it)
                let target = match label {
                    None => self.loops.iter().rposition(|x| x.lowered.is_none()),
                    Some(l) => self.loops.iter().rposition(|x| x.label.as_deref() == Some(l.name.as_str())),
                };
                let Some(t) = target.filter(|t| *t >= inside) else {
                    if target.is_some() {
                        self.control.escaped = true;
                    }
                    return match label {
                        Some(l) => {
                            self.error(e.loc, format!("no loop labeled `{}` around it", l.name));
                            quote!()
                        }
                        None => self.later(e.loc, "`break`/`continue` outside a loop (or out of a lambda)"),
                    };
                };
                // (out of a `try` body's closure too: see `exceptions`)
                self.loop_jump_tokens(t, is_break, label.as_ref(), span)
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
        let (ty, tail, shared) = if matches!(tail, Tail::Unit) {
            (Ty::Unit, tail, None)
        } else {
            let a = self.peek_block(then);
            match otherwise {
                Some(o) => {
                    let tys = [a, self.peek_block(o)];
                    self.joined(&tys, tail)
                }
                None => (Ty::Unit, tail, None),
            }
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
            let narrowed = quote_spanned!(span=> let ::core::option::Option::Some(#i) = #i.clone());
            let c = unused_narrowings(narrowed, &b);
            let ts = quote_spanned!(span=> if #c #b else #a);
            return Out::plain(self.held_shared(ts, shared, span), ty);
        }
        self.push_scope();
        let c = self.condition(condition);
        let a = self.braced(then, tail);
        let c = unused_narrowings(c, &a);
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
        Out::plain(self.held_shared(ts, shared, span), ty)
    }

    /// The type `if`/`when` branches of `tys` give together, as Kotlin's common supertype, and the tail their values
    /// are given to: a `null` branch beside a value of a type makes it nullable (the values given in `Some`); a
    /// shared trait object's branch (`Arc<dyn Shape>`) beside values of types of their own (`RoundedCornerShape`)
    /// makes it the shared trait object's type (those values made one: `Arc::new`), the trait named for the value
    /// held ([`Gen::held_shared`]). Otherwise the first type known.
    pub(crate) fn joined(&self, tys: &[Ty], tail: Tail) -> (Ty, Tail, Option<(super::expr::Shared, String)>) {
        let first = tys.iter().find(|t| **t != Ty::Unknown).cloned().unwrap_or(Ty::Unknown);
        let Tail::Value(mut want) = tail else { return (first, tail, None) };
        let is_null = |t: &Ty| matches!(t, Ty::Nullable(x) if **x == Ty::Unknown);
        if tys.iter().any(is_null)
            && let Some(value) = tys.iter().find(|t| !is_null(t) && **t != Ty::Unknown && !t.is_nullable())
        {
            want.some = true;
            return (value.clone().nullable(), Tail::Value(want), None);
        }
        // (`Arc<dyn Shape>` is `Arc` of `Shape` here; a bare trait's name is one too)
        let shared = tys.iter().find_map(|t| match t {
            Ty::Named(p, args) if matches!(p.as_str(), "Arc" | "Rc") && args.len() == 1 => match &args[0] {
                Ty::Named(n, a) if a.is_empty() && trait_object(n).is_some() => {
                    Some((n.clone(), p.as_str(), t.clone()))
                }
                _ => None,
            },
            Ty::Named(n, args) if args.is_empty() => trait_object(n).map(|p| (n.clone(), p, t.clone())),
            _ => None,
        });
        let concrete = |t: &Ty| matches!(t, Ty::Named(n, _) if !matches!(n.as_str(), "Arc" | "Rc" | "Box") && trait_object(n).is_none());
        if let Some((name, ptr, held)) = shared
            && tys.iter().any(concrete)
            && want.shared.is_none()
        {
            let ptr = if ptr == "Arc" { super::expr::Shared::Arc } else { super::expr::Shared::Rc };
            want.shared = Some(ptr);
            return (held, Tail::Value(want), Some((ptr, name)));
        }
        (first, Tail::Value(want), None)
    }

    /// `ts` held as the shared trait object `shared` names (a typed binding: the branches' `Arc::new(…)` coerce to it).
    pub(crate) fn held_shared(
        &mut self,
        ts: TokenStream,
        shared: Option<(super::expr::Shared, String)>,
        span: proc_macro2::Span,
    ) -> TokenStream {
        let Some((ptr, name)) = shared else { return ts };
        let f = self.fresh("shared", span);
        let tr = proc_macro2::Ident::new(&name, span);
        let held = match ptr {
            super::expr::Shared::Arc => quote_spanned!(span=> ::std::sync::Arc<dyn #tr>),
            super::expr::Shared::Rc => quote_spanned!(span=> ::std::rc::Rc<dyn #tr>),
        };
        quote_spanned!(span=> { let #f: #held = #ts; #f })
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
            // (`x is T` on a local: Kotlin's smart cast, `x` a `T` in the then branch)
            ExprKind::Is { expr, negated: false, ty } if matches!(&expr.kind, ExprKind::Name(n) if self.is_local(&n.name)) =>
            {
                let ExprKind::Name(n) = &expr.kind else { unreachable!() };
                let v = self.expr(expr).ts;
                let rust = self.local(&n.name).map(|l| l.rust).unwrap_or_else(|| snake(&n.name));
                let i = ident(&rust, self.span(n.loc));
                let kt = self.kt(span);
                let target = self.rust_type(&Type { nullable: false, ..(**ty).clone() }, super::types::TypePos::Value);
                let tty = self.ty_of(&Type { nullable: false, ..(**ty).clone() });
                // (the same local, of the type the check gives it: where it was declared stays)
                let l = self.local(&n.name);
                let at = l.as_ref().and_then(|l| l.at);
                let v = self.cast_operand_of(v, &l.map(|l| l.ty).unwrap_or(Ty::Unknown), span);
                self.declare(&n.name, rust, LocalKind::Val, tty);
                self.set_local_at(&n.name, at);
                quote_spanned!(span=> let ::core::option::Option::Some(#i) = #kt::KtAs::<#target>::kt_as_safe(&#v))
            }
            _ => self.value(c),
        }
    }

    /// `when`: an `if` chain on the subject (bound once).
    pub(crate) fn when(&mut self, w: &When, tail: Tail, loc: Loc) -> Out {
        // (of a sealed class's value: a `match` of its cases)
        if let Some(out) = self.sealed_when(w, tail, loc) {
            return out;
        }
        let span = self.span(loc);
        let kt = self.kt(span);
        let (ty, tail, shared) = if matches!(tail, Tail::Unit) {
            (Ty::Unit, tail, None)
        } else {
            let tys: Vec<Ty> = w.branches.iter().map(|b| self.peek_block(&b.body)).collect();
            self.joined(&tys, tail)
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
                        self.declare_at(n, rust, LocalKind::Val, sty.clone());
                        i
                    }
                    None => self.fresh("w", span),
                };
                prelude.extend(quote_spanned!(span=> let #i = #v;));
                Some((i, sty))
            }
            None => None,
        };
        // (the subject's local, rebound as its type in a branch of one `is`: Kotlin's smart cast)
        let subject_local: Option<String> = match &w.subject {
            Some(s) => match (&s.binding, &s.expr.kind) {
                (Some((n, _)), _) => Some(n.name.clone()),
                (None, ExprKind::Name(n)) if self.is_local(&n.name) => Some(n.name.clone()),
                _ => None,
            },
            None => None,
        };
        let mut chain = TokenStream::new();
        let mut has_else = false;
        for (k, b) in w.branches.iter().enumerate() {
            let smart = match (b.conditions.as_slice(), &subject, &subject_local, &b.guard) {
                ([WhenCondition::Is { negated: false, ty, .. }], Some((s, _)), Some(name), None) => {
                    Some((ty.clone(), s.clone(), name.clone()))
                }
                _ => None,
            };
            if let Some((ty, s, name)) = smart {
                let sty = subject.as_ref().map(|(_, t)| t.clone()).unwrap_or(Ty::Unknown);
                let s = self.cast_operand_of(quote!(#s), &sty, span);
                let at = self.local(&name).and_then(|l| l.at);
                self.push_scope();
                let target = self.rust_type(&Type { nullable: false, ..ty.clone() }, super::types::TypePos::Value);
                let tty = self.ty_of(&Type { nullable: false, ..ty.clone() });
                let rust = snake(&name);
                let i = ident(&rust, span);
                self.declare(&name, rust, LocalKind::Val, tty);
                self.set_local_at(&name, at);
                let body = self.braced(&b.body, tail);
                self.pop_scope();
                let kw = if k == 0 { quote_spanned!(span=> if) } else { quote_spanned!(span=> else if) };
                chain.extend(quote_spanned! {span=>
                    #kw let ::core::option::Option::Some(#i) = #kt::KtAs::<#target>::kt_as_safe(&#s) #body
                });
                continue;
            }
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
                    (WhenCondition::Is { negated, ty, .. }, Some((s, sty))) => {
                        let target =
                            self.rust_type(&Type { nullable: false, ..ty.clone() }, super::types::TypePos::Value);
                        let s = self.cast_operand_of(quote!(#s), sty, span);
                        let t = quote_spanned!(span=> #kt::kt_is::<#target, _>(&#s));
                        if *negated { quote_spanned!(span=> !#t) } else { t }
                    }
                    (WhenCondition::Is { loc, .. }, None) => {
                        self.error(*loc, "`is` in a `when` without a subject");
                        quote!(false)
                    }
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
        Out::plain(self.held_shared(ts, shared, span), ty)
    }
}

/// Whether the annotations hold `@Suppress("UNUSED_VARIABLE")` or `@Suppress("unused")` (any of its names).
fn suppresses_unused(annotations: &[Annotation]) -> bool {
    annotations.iter().filter(|a| a.name.last().name == "Suppress").flat_map(|a| a.args.iter().flatten()).any(|arg| {
        matches!(&arg.value.kind, ExprKind::String(s)
            if matches!(s.parts.as_slice(), [StringPart::Text { text, .. }] if text == "UNUSED_VARIABLE" || text == "unused"))
    })
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
        ExprKind::Is { expr, negated: false, .. } => matches!(expr.kind, ExprKind::Name(_)),
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
            if a.len() == 1 && matches!(n.as_str(), "State" | "MutableState" | "DerivedState" | "AnimationState") =>
        {
            a[0].clone()
        }
        _ => Ty::Unknown,
    }
}

/// A condition's null checks narrowed for a branch that doesn't read the narrowed value (`if (x != null) "a" else
/// "b"`): `x.is_some()` instead of `let Some(x) = x.clone()`, which would bind a value nothing uses.
pub(crate) fn unused_narrowings(condition: TokenStream, branch: &TokenStream) -> TokenStream {
    use proc_macro2::TokenTree;
    fn mentions(ts: &TokenStream, name: &str) -> bool {
        ts.clone().into_iter().any(|t| match t {
            TokenTree::Ident(i) => i == name,
            TokenTree::Group(g) => mentions(&g.stream(), name),
            _ => false,
        })
    }
    let tokens: Vec<TokenTree> = condition.into_iter().collect();
    let mut out = Vec::new();
    let mut k = 0;
    while k < tokens.len() {
        // `let :: core :: option :: Option :: Some (x) = x . clone ()`: 19 tokens (each `::` two puncts)
        let narrowing = matches!(&tokens[k], TokenTree::Ident(i) if i == "let")
            && tokens.len() >= k + 19
            && matches!(&tokens[k + 12], TokenTree::Ident(i) if i == "Some")
            && matches!(&tokens[k + 13], TokenTree::Group(_))
            && matches!(&tokens[k + 14], TokenTree::Punct(p) if p.as_char() == '=');
        if narrowing
            && let TokenTree::Group(g) = &tokens[k + 13]
            && let [TokenTree::Ident(x)] = g.stream().into_iter().collect::<Vec<_>>().as_slice()
            && !mentions(branch, &x.to_string())
        {
            let span = x.span();
            out.extend(quote_spanned!(span=> #x.is_some()));
            k += 19;
            continue;
        }
        out.push(tokens[k].clone());
        k += 1;
    }
    out.into_iter().collect()
}
