//! Kotlin's jumps out of an inline function's lambda (stage 14 WPB): `return` from the function around it, `return@outer`
//! from an outer lambda, `break`/`continue` of a loop around it (Kotlin 2.2's non-local `break` and `continue`).
//! A Rust closure can't do them, so where a lambda of `forEach`, `forEachIndexed` or `repeat` (Kotlin's inline
//! functions running it in place) jumps out of it, the call is written as the loop it is:
//!
//! | Kotlin | Rust |
//! |---|---|
//! | `xs.forEach { x -> … }` | `'__kz_each1: for x in kt::KtIterable::kt_iter(&xs) { … }` |
//! | `xs.forEachIndexed { i, x -> … }` | `for (i, x) in kt::KtIterable::kt_iter(&xs).enumerate() { let i = i as i32; … }` |
//! | `repeat(n) { i -> … }` | `for i in 0..n { … }` |
//!
//! There, `return@forEach` is the loop's `continue`, `return` and `return@outer` are the function's and the outer
//! lambda's own, and `break`/`continue` are the Kotlin loop's around it (labeled for Rust when it isn't). A `try` body's
//! jumps leave its closure the same way (`exceptions`).
//!
//! Out of the lambda of any other inline function — the API's (`let`, `run`, `also`, `buildAnnotatedString`,
//! `withStyle`, `Snapshot.withMutableSnapshot`…), the kt shims', the file's own `inline fun`s — a jump is written as
//! Kotlin's inlining makes it behave: the rest of the callee doesn't run (its `finally` blocks do), the code after the
//! call does the jump. The lambda keeps the jump and leaves the callee's frames by unwinding (`kt::leave`, no panic),
//! the call is caught where it is written:
//!
//! ```text
//! { let __kz_leave3_9 = kt::Leave::new();
//!   match kt::leaving(&__kz_leave3_9, || s.let_(|it| if it < 0 { kt::leave(&__kz_leave3_9, kt::Jump::Return(None)) } else { … })) {
//!       Ok(v) => v,
//!       Err(kt::Jump::Return(x)) => return x,
//!       Err(kt::Jump::Loop(0)) => break,
//!       … } }
//! ```
//!
//! (in a coroutine `kt::leaving_async(&slot, async { call }).await`). Kotlin allows it out of an inline function's
//! lambda only: one the callee keeps (`onClick`), a coroutine's (`launch { }`), is an error, as Kotlin's.
//!
//! Out of a composable call (`Column { if (xs.isEmpty()) return … }`, `key(k) { … }`, the file's own inline
//! composables), or a lambda calling composables (`for (x in xs) { Row { if (…) break } }`, `return@Column`), the
//! groups the jump leaves are ended where it lands, as Compose's compiler ends them (`currentMarker` before the call,
//! `endToMarker` at the jump): `kt::leaving` takes the composer's marker before the call and ends the groups opened
//! since when a jump comes out of it (the composer's `end_to_marker`: each group ended as at its end, what it didn't
//! call this time leaving the composition — the values remembered after the jump forgotten, as Compose trims them).
//! There the slot is a reference (`let slot = &kt::Leave::new();`), which the lambdas a `#[kompozable]` body hands to
//! composables capture by a copy, as they capture every local.

use super::expr::Out;
use super::stmt::Tail;
use super::{Gen, LoopCx};
use crate::ast::*;
use crate::pos::Loc;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};

/// What an inline call written as a loop iterates.
enum Lowered<'e> {
    /// `xs.forEach`, `xs.forEachIndexed` (indexed).
    Each { receiver: &'e Expr, indexed: bool },
    /// `repeat(n)`.
    Repeat(&'e Expr),
}

impl Gen<'_, '_> {
    /// Whether a function of the file has a suspending twin (`<name>_async`): an `inline fun` (not a composable, not
    /// `suspend`, no defaults, not an extension) taking a lambda, which Kotlin lets suspend in a coroutine.
    pub(crate) fn has_inline_twin(&self, f: &Function) -> bool {
        f.modifiers.has(ModifierKind::Inline)
            && !f.is_kompozable()
            && !f.is_suspend()
            && f.receiver.is_none()
            && f.type_params.is_empty()
            && f.params.iter().all(|p| p.default.is_none() && !p.is_vararg())
            && f.params.iter().any(|p| {
                p.ty.as_ref().is_some_and(|t| {
                    let t = self.unaliased(t);
                    matches!(t.kind, TypeKind::Function(_)) && !t.nullable
                })
            })
    }

    /// The suspending twin of a file's `inline fun` (see [`Gen::has_inline_twin`]): `async fn <name>_async`, its
    /// lambdas async closures (`impl AsyncFn(A) -> R`, called awaited), its body written in the coroutine — what a call
    /// whose lambda suspends goes to, awaited (as the kt shims' and Compose's inline functions' twins are).
    pub(crate) fn inline_twin(&mut self, f: &Function) -> TokenStream {
        use super::resolve::{FunTy, LocalKind, Ty};
        use super::types::TypePos;
        let span = self.span(f.loc);
        let kt = self.kt(span);
        let rt = super::respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let info = self.quiet(|g| g.file_fn(f));
        let unit = info.ret == Ty::Unit;
        let (saved_fun, saved_async) = (self.fun.clone(), self.in_async);
        self.fun = super::FnState {
            kompozable: false,
            nullable_result: info.ret.is_nullable(),
            shared_result: None,
            result: Some(info.ret.clone()),
        };
        self.scopes = vec![std::collections::HashMap::new()];
        self.receivers.clear();
        self.frames.clear();
        self.loops.clear();
        self.lambdas.clear();
        self.in_async = true;
        let mut params = Vec::new();
        let mut converts = Vec::new();
        for p in &f.params {
            let Some(t) = &p.ty else { continue };
            let ut = self.unaliased(t);
            match &ut.kind {
                TypeKind::Function(ft) if !ut.nullable => {
                    let mut inputs = Vec::new();
                    let mut tys = Vec::new();
                    for fp in &ft.params {
                        inputs.push(self.rust_type(&fp.ty, TypePos::Value));
                        tys.push(self.ty_of(&fp.ty));
                    }
                    let ret = self.ty_of(&ft.result);
                    let output = if ret == Ty::Unit {
                        quote!()
                    } else {
                        let r = self.rust_type(&ft.result, TypePos::Value);
                        quote_spanned!(span=> -> #r)
                    };
                    let rust = super::names::snake(&p.name.name);
                    let i = self.name_ident(&rust, p.name.loc);
                    params.push(quote_spanned!(span=> #i: impl ::core::ops::AsyncFn(#(#inputs),*) #output));
                    let fty =
                        Ty::Fun(Box::new(FunTy { params: tys, ret, receiver: None, suspend: true, shared: false }));
                    self.declare_at(&p.name, rust, LocalKind::Param, fty);
                }
                _ => {
                    let (ts, convert) = self.param(p, false, true);
                    params.push(ts);
                    converts.extend(convert);
                }
            }
        }
        let ret = match &f.result {
            Some(t) if self.ty_of(t) != Ty::Unit => {
                let r = self.rust_type(t, TypePos::Return);
                quote_spanned!(span=> -> #r)
            }
            Some(_) => quote!(),
            None if unit => quote!(),
            None => {
                let r = self.rust_of_ty(&info.ret, span);
                quote_spanned!(span=> -> #r)
            }
        };
        let body = match &f.body {
            Some(FunctionBody::Block(b)) => self.fn_block(b, unit),
            Some(FunctionBody::Expr(e)) if unit => {
                let s = self.stmt_expr(e);
                quote!(#s;)
            }
            Some(FunctionBody::Expr(e)) => self
                .value_want(e, super::expr::Want { some: info.ret.is_nullable(), result: true, ..Default::default() }),
            None => quote!(),
        };
        self.fun = saved_fun;
        self.in_async = saved_async;
        let vis = self.visibility(&f.modifiers, f.loc);
        let name = proc_macro2::Ident::new(&format!("{}_async", self.fn_rust_name(f)), self.span(f.name.loc));
        let allow = super::allow_attr(span);
        quote_spanned! {span=>
            #allow
            #[doc(hidden)]
            #vis async fn #name(#(#params),*) #ret {
                #[allow(unused_imports)]
                use #kt::ext::*;
                #(#converts)*
                #rt::named_args!(#kz_types { #body })
            }
        }
    }

    // ---- Jumps out of an inline function's lambda (see the module's doc) ------------------------------------------

    /// Whether the lambdas written from depth `depth` in may be jumped out of (each an inline function's); an error
    /// where one isn't.
    fn leavable(&mut self, depth: usize, what: &str, loc: Loc) -> bool {
        if self.lambdas[depth..].iter().all(|l| l.inline) {
            return true;
        }
        self.error(
            loc,
            format!(
                "{what} isn't allowed here: it would leave a lambda its callee keeps or runs in a coroutine of its own \
                 (Kotlin jumps out of an inline function's lambda only)"
            ),
        );
        false
    }

    /// `return v` (`v` none: `return`) out of the lambdas written from depth `depth` in: the function's (`depth` 0)
    /// or the lambda's at `depth - 1` (`return@label`), landing at the innermost call of that depth (see the module's
    /// doc); none where there is none.
    pub(crate) fn leave_return(&mut self, depth: usize, v: Option<TokenStream>, loc: Loc) -> Option<TokenStream> {
        let span = self.span(loc);
        if !self.leavable(depth, "`return`", loc) {
            return Some(quote_spanned!(span=> ::core::unreachable!()));
        }
        let f = self.control.leaves.iter_mut().rev().find(|f| f.lambdas == depth)?;
        f.returned = true;
        let slot = proc_macro2::Ident::new(&f.slot, span);
        let kt = self.kt(span);
        let v = v.unwrap_or_else(|| quote_spanned!(span=> ()));
        Some(quote_spanned!(span=> #kt::leave(&#slot, #kt::Jump::Return(#v))))
    }

    /// `break`/`continue` of the loop at index `t` of the loops, out of the lambdas written inside it (see the
    /// module's doc); none where no call lands it.
    pub(crate) fn leave_loop(
        &mut self,
        t: usize,
        is_break: bool,
        label: Option<&Ident>,
        loc: Loc,
    ) -> Option<TokenStream> {
        let span = self.span(loc);
        let depth = self.lambdas.iter().take_while(|l| l.loops <= t).count();
        let what = if is_break { "`break`" } else { "`continue`" };
        if !self.leavable(depth, what, loc) {
            return Some(quote_spanned!(span=> ::core::unreachable!()));
        }
        let f = self.control.leaves.iter_mut().rev().find(|f| f.lambdas == depth && f.loops > t)?;
        let key = (t, is_break, label.cloned());
        let n = match f.jumps.iter().position(|j| j.0 == key.0 && j.1 == key.1) {
            Some(n) => n,
            None => {
                f.jumps.push(key);
                f.jumps.len() - 1
            }
        } as u32;
        let slot = proc_macro2::Ident::new(&f.slot, span);
        let kt = self.kt(span);
        Some(quote_spanned!(span=> #kt::leave(&#slot, #kt::Jump::Loop(#n))))
    }

    /// A call taking lambdas (`write` writes it): caught where it is written when a jump out of a lambda lands at it
    /// (see the module's doc).
    pub(crate) fn leaving_call(&mut self, loc: Loc, write: impl FnOnce(&mut Self) -> Out) -> Out {
        let slot = format!("__kz_leave{}_{}", loc.start.line, loc.start.column);
        self.control.leaves.push(super::exceptions::LeaveFrame {
            lambdas: self.lambdas.len(),
            loops: self.loops.len(),
            slot: slot.clone(),
            ..Default::default()
        });
        let composables = self.control.kompozable_calls;
        let awaits = self.awaits;
        let mut out = write(self);
        let frame = self.control.leaves.pop().expect("its frame");
        if !frame.returned && frame.jumps.is_empty() {
            return out;
        }
        let span = self.span(loc);
        // (a composable call, or one whose lambdas call composables: `kt::leaving` ends the groups the jump leaves,
        // `endToMarker`; the slot a reference, which the lambdas a `#[kompozable]` body hands to composables capture
        // as they capture every local, by a copy)
        let composing = self.control.kompozable_calls > composables;
        let kt = self.kt(span);
        let slot = proc_macro2::Ident::new(&slot, span);
        let mut arms = Vec::new();
        if frame.returned {
            let ret = self.return_tokens(Some(quote_spanned!(span=> __kz_x)), span);
            arms.push(quote_spanned!(span=> #kt::Jump::Return(__kz_x) => #ret,));
        }
        for (n, (t, is_break, label)) in frame.jumps.iter().enumerate() {
            let n = n as u32;
            let j = self.loop_jump_tokens(*t, *is_break, label.as_ref(), span);
            arms.push(quote_spanned!(span=> #kt::Jump::Loop(#n) => #j,));
        }
        let new = if frame.returned {
            quote_spanned!(span=> #kt::Leave::new())
        } else {
            quote_spanned!(span=> #kt::Leave::<()>::new())
        };
        let call = out.ts;
        let (new, at) = if composing { (quote_spanned!(span=> &#new), quote!(#slot)) } else { (new, quote!(&#slot)) };
        let caught = if self.in_async && self.awaits > awaits {
            let a = self.awaited(None, loc);
            quote_spanned!(span=> #kt::leaving_async(#at, async { #call }) #a)
        } else {
            quote_spanned!(span=> #kt::leaving(#at, || #call))
        };
        out.ts = quote_spanned!(span=> {
            let #slot = #new;
            match #caught {
                ::core::result::Result::Ok(__kz_v) => __kz_v,
                ::core::result::Result::Err(__kz_j) => match __kz_j {
                    #(#arms)*
                    #[allow(unreachable_patterns)]
                    _ => ::core::unreachable!(),
                },
            }
        });
        out
    }

    /// The index of the loop a lambda written as a loop labeled `label` is (`return@forEach`'s).
    pub(crate) fn lowered_loop(&self, label: &str) -> Option<usize> {
        // (only one outside the lambdas being written: a lambda inside it is its own)
        let inside = self.lambdas.last().map_or(0, |l| l.loops);
        let t =
            self.loops.iter().rposition(|x| x.lowered.as_ref().is_some_and(|(_, ls)| ls.iter().any(|l| l == label)))?;
        (t >= inside).then_some(t)
    }

    /// `xs.forEach { … }`, `xs.forEachIndexed { … }`, `repeat(n) { … }` whose lambda jumps out of it, as a loop (see
    /// the module's doc); none otherwise.
    pub(crate) fn lowered_call(
        &mut self,
        callee: &Expr,
        args: &[Arg],
        trailing: Option<&Lambda>,
        loc: Loc,
    ) -> Option<TokenStream> {
        let (what, name, lambda) = match (&callee.kind, args, trailing) {
            (ExprKind::Member { receiver, safe: false, name }, [], Some(l))
                if matches!(name.name.as_str(), "forEach" | "forEachIndexed") =>
            {
                let indexed = name.name == "forEachIndexed";
                (Lowered::Each { receiver, indexed }, name.name.as_str(), l)
            }
            (ExprKind::Name(n), [a], Some(l))
                if n.name == "repeat"
                    && a.name.is_none()
                    && !self.is_local(&n.name)
                    && self.file.function("repeat").is_none() =>
            {
                (Lowered::Repeat(&a.value), "repeat", l)
            }
            _ => return None,
        };
        // (only where its lambda jumps out of it: written as a lambda first, quietly, to tell)
        let escaped = self.quiet(|g| {
            // (nothing of the probe left behind: what it awaits, the twins it asks for, the locals it shares)
            let outer = std::mem::replace(&mut g.control.escaped, false);
            let saved = (g.awaits, g.last_await.clone(), g.inline_async, g.plain_twin_lambdas.len(), g.counter_now());
            let shared = (g.control.assigned_vars.clone(), g.control.captured_vars.clone());
            let frames = g.frames.clone();
            let _ = g.lambda_in_place(lambda, Some(name), true, false);
            g.frames = frames;
            (g.awaits, g.last_await, g.inline_async) = (saved.0, saved.1, saved.2);
            g.plain_twin_lambdas.truncate(saved.3);
            g.set_counter(saved.4);
            (g.control.assigned_vars, g.control.captured_vars) = shared;
            std::mem::replace(&mut g.control.escaped, outer)
        });
        if !escaped {
            return None;
        }
        let span = self.span(loc);
        let kt = self.kt(span);
        let label = self.fresh("each", span).to_string();
        let mut labels = vec![name.to_string()];
        if let Some(l) = &lambda.label {
            labels.push(l.name.clone());
        }
        let (iter, elem, index_ty) = match &what {
            Lowered::Each { receiver, .. } => {
                let ty = self.peek_ty(receiver);
                let o = self.expr(receiver).ts;
                let iter = match ty {
                    super::resolve::Ty::String => {
                        quote_spanned!(span=> ::core::primitive::str::chars(&#o).collect::<::std::vec::Vec<char>>())
                    }
                    _ => quote_spanned!(span=> #kt::KtIterable::kt_iter(&#o)),
                };
                (iter, ty.element(), super::resolve::Ty::Int)
            }
            Lowered::Repeat(n) => {
                let n = self.value(n);
                (quote_spanned!(span=> 0..#n), super::resolve::Ty::Int, super::resolve::Ty::Int)
            }
        };
        self.push_scope();
        self.ranges.push(lambda.loc);
        // Its parameters: the element (an index first for `forEachIndexed`), `it` where none is named.
        let names: Vec<Option<&Ident>> = match &lambda.params {
            Some(ps) => {
                let mut out = Vec::new();
                for p in ps {
                    match &p.binding {
                        Binding::Name(n) => out.push((n.name != "_").then_some(n)),
                        Binding::Destructure(_) => {
                            self.ranges.pop();
                            self.pop_scope();
                            return None;
                        }
                    }
                }
                out
            }
            None => Vec::new(),
        };
        let mut pats = Vec::new();
        let mut unpack = TokenStream::new();
        let indexed = matches!(what, Lowered::Each { indexed: true, .. });
        let tys = if indexed { vec![index_ty, elem] } else { vec![elem] };
        if names.is_empty() && tys.len() == 1 {
            self.declare("it", "it".into(), super::resolve::LocalKind::LambdaParam, tys[0].clone());
            pats.push(quote_spanned!(span=> it));
        } else {
            for (k, ty) in tys.iter().enumerate() {
                match names.get(k).copied().flatten() {
                    Some(n) => {
                        let rust = super::names::snake(&n.name);
                        let i = self.name_ident(&rust, n.loc);
                        self.declare_at(n, rust, super::resolve::LocalKind::LambdaParam, ty.clone());
                        pats.push(quote!(#i));
                    }
                    None => pats.push(quote_spanned!(span=> _)),
                }
            }
        }
        if indexed
            && let Some(first) = pats.first().cloned()
            && first.to_string() != "_"
        {
            unpack = quote_spanned!(span=> let #first = #first as i32;);
        }
        let cx = LoopCx { lowered: Some((label.clone(), labels)), ..Default::default() };
        let used = cx.named.clone();
        self.loops.push(cx);
        let body = self.stmts(&lambda.body.stmts, Tail::Unit);
        self.loops.pop();
        self.ranges.pop();
        self.pop_scope();
        let lt = used.borrow().as_ref().map(|l| {
            let lt = super::lifetime(l, span);
            quote!(#lt:)
        });
        let pat = if pats.len() == 1 { pats.remove(0) } else { quote!((#(#pats),*)) };
        let iter = if indexed { quote_spanned!(span=> ::core::iter::Iterator::enumerate(#iter)) } else { iter };
        Some(quote_spanned!(span=> #lt for #pat in #iter { #unpack #body }))
    }
}
