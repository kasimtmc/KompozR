//! Lambdas as closures.
//!
//! What a lambda becomes depends on the parameter it is given to (the API index's, or the file's function's):
//! - one the callee runs before returning (Kotlin's inline functions: `forEach`, `map`, `remember`'s init — a Rust
//!   parameter without `'static`): a closure borrowing what it uses, so `x += 1` in it changes the `x` outside, as in
//!   Kotlin;
//! - one the callee keeps (`'static`: `onClick`, `derivedStateOf`, a draw lambda): a `move` closure capturing clones
//!   of the locals it uses (Kotlin's lambdas share what they capture: states and `Rc`s share it here too) — written
//!   as `{ let x = capture(&x); move || … }`, except where `#[kompozable]` rewrites the call, which wraps (and
//!   memoizes by its captures, Compose's strong skipping) the plain closure it is given itself;
//! - a receiver lambda (`DrawScope.() -> Unit`): its first parameter is the scope (`this`, `this1`, … for nested
//!   ones), whose members free names in it resolve to (`drawCircle(…)` → `this.draw_circle(…)`);
//! - a suspending one: an `async` block (`LaunchedEffect(k) { … }`), or a closure giving one (`pointerInput(k) { … }`),
//!   whose suspending calls are awaited;
//! - `Layout`'s measure policy: through `measure_policy(…)`, which gives Rust its parameters' types.
//!
//! Its parameters: as written, or `it` when it takes one and names none. Its body's last expression is its value,
//! unless the parameter's lambda gives nothing.

use super::expr::Want;
use super::names::snake;
use super::resolve::*;
use super::stmt::Tail;
use super::{CaptureFrame, Gen, LambdaCx, respan};
use crate::ast::*;
use crate::index::LambdaKind;
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};

impl Gen<'_, '_> {
    /// A lambda as a value of its own (`val f = { x: Int -> x + 1 }`): a closure capturing clones.
    pub(crate) fn lambda(&mut self, l: &Lambda, info: Option<&LInfo>, call_label: Option<&str>) -> TokenStream {
        let stored = LInfo {
            kind: LambdaKind::Fn,
            inputs: Vec::new(),
            output: String::new(),
            receiver: None,
            suspend: false,
            stored: true,
            measure_policy: false,
        };
        match info {
            Some(i) => self.lambda_for(l, Some(i), call_label, false),
            None => self.lambda_with(l, &stored, None, call_label, false, true),
        }
    }

    /// A lambda given to a parameter `info` (none: unknown, it borrows) of a call labeled `call_label`;
    /// `transform_wraps`: `#[kompozable]` rewrites the call and wraps its closures itself.
    pub(crate) fn lambda_for(
        &mut self,
        l: &Lambda,
        info: Option<&LInfo>,
        call_label: Option<&str>,
        transform_wraps: bool,
    ) -> TokenStream {
        match info {
            Some(i) => self.lambda_with(l, i, Some(i), call_label, transform_wraps, false),
            None => {
                let unknown = LInfo {
                    kind: LambdaKind::Fn,
                    inputs: Vec::new(),
                    output: String::new(),
                    receiver: None,
                    suspend: false,
                    stored: false,
                    measure_policy: false,
                };
                self.lambda_with(l, &unknown, None, call_label, transform_wraps, true)
            }
        }
    }

    fn lambda_with(
        &mut self,
        l: &Lambda,
        info: &LInfo,
        known: Option<&LInfo>,
        call_label: Option<&str>,
        transform_wraps: bool,
        arity_from_body: bool,
    ) -> TokenStream {
        let span = self.span(l.loc);
        let rt = respan(self.rt().clone(), span);
        // (a measure policy goes through `measure_policy(…)`, which the macro memoizes without wrapping it)
        let explicit = info.stored && (!transform_wraps || info.measure_policy);
        let mut labels: Vec<String> = Vec::new();
        if let Some(lb) = &l.label {
            labels.push(lb.name.clone());
        }
        if let Some(c) = call_label {
            labels.push(c.to_string());
        }

        // Its scope: parameters, the receiver.
        self.push_scope();
        let boundary = self.scopes.len() - 1;
        self.frames.push(CaptureFrame { boundary, used: Vec::new() });
        self.lambdas.push(LambdaCx { labels: labels.clone(), loops: self.loops.len() });
        let receivers_before = self.receivers.len();
        let mut params: Vec<TokenStream> = Vec::new();
        let mut unpack = TokenStream::new();
        if let Some(r) = &info.receiver {
            let name =
                if self.receivers.is_empty() { "this".to_string() } else { format!("this{}", self.receivers.len()) };
            let i = proc_macro2::Ident::new(&name, span);
            params.push(quote!(#i));
            self.receivers.push(ImplicitReceiver {
                rust: name,
                ty: r.clone(),
                labels: labels.clone(),
                depth: boundary,
            });
        }
        let first_input = usize::from(info.receiver.is_some());
        let input_tys: Vec<Ty> = info.inputs.iter().skip(first_input).map(|t| self.input_ty(t)).collect();
        match &l.params {
            Some(ps) => {
                for (k, p) in ps.iter().enumerate() {
                    let ty =
                        p.ty.as_ref()
                            .map(|t| self.ty_of(t))
                            .or_else(|| input_tys.get(k).cloned())
                            .unwrap_or(Ty::Unknown);
                    let annot = match &p.ty {
                        Some(t) => {
                            let rt = self.rust_type(t, super::types::TypePos::Value);
                            quote_spanned!(span=> : #rt)
                        }
                        None => quote!(),
                    };
                    match &p.binding {
                        Binding::Name(n) if n.name == "_" => params.push(quote_spanned!(span=> _ #annot)),
                        Binding::Name(n) => {
                            let rust = snake(&n.name);
                            let i = self.name_ident(&rust, n.loc);
                            self.declare(&n.name, rust, LocalKind::LambdaParam, ty);
                            params.push(quote_spanned!(span=> #i #annot));
                        }
                        Binding::Destructure(entries) => {
                            let d = self.fresh("d", span);
                            params.push(quote_spanned!(span=> #d #annot));
                            for (k, e) in entries.iter().enumerate() {
                                if e.name.name == "_" {
                                    continue;
                                }
                                let rust = snake(&e.name.name);
                                let i = self.name_ident(&rust, e.name.loc);
                                let c = proc_macro2::Ident::new(&format!("component{}", k + 1), span);
                                unpack.extend(quote_spanned!(span=> let #i = #d.#c();));
                                let ety = match (&ty, k) {
                                    (Ty::Pair(a, _), 0) => (**a).clone(),
                                    (Ty::Pair(_, b), 1) => (**b).clone(),
                                    _ => Ty::Unknown,
                                };
                                self.declare(&e.name.name, rust, LocalKind::LambdaParam, ety);
                            }
                        }
                    }
                }
            }
            None => {
                let takes_it = if arity_from_body && known.is_none() { uses_it(&l.body) } else { info.arity() == 1 };
                if takes_it && uses_it(&l.body) {
                    let ty = input_tys.first().cloned().unwrap_or(Ty::Unknown);
                    self.declare("it", "it".into(), LocalKind::LambdaParam, ty);
                    params.push(quote_spanned!(span=> it));
                } else if takes_it {
                    params.push(quote_spanned!(span=> _));
                } else {
                    for _ in 0..info.arity() {
                        params.push(quote_spanned!(span=> _));
                    }
                }
            }
        }

        // Its body.
        let was_async = self.in_async;
        let suspend = info.suspend || info.kind == LambdaKind::Future;
        if suspend {
            self.in_async = true;
        } else if info.stored {
            // (a kept lambda runs later, outside the coroutine around it)
            self.in_async = false;
        }
        let tail = if known.is_some_and(|k| k.unit_output()) || (known.is_some() && info.kind == LambdaKind::Future) {
            Tail::Unit
        } else {
            Tail::Value(Want::default())
        };
        let body = self.stmts(&l.body.stmts, tail);
        self.in_async = was_async;
        self.receivers.truncate(receivers_before);
        self.lambdas.pop();
        let frame = self.frames.pop().expect("its frame");
        self.pop_scope();
        let captures: Vec<proc_macro2::Ident> =
            frame.used.iter().filter(|u| *u != "self").map(|u| proc_macro2::Ident::new(u, span)).collect();
        let lets = |span: Span| -> TokenStream {
            let lets = captures.iter().map(|c| quote_spanned!(span=> let #c = #rt::__private::capture(&#c);));
            quote!(#(#lets)*)
        };
        let block = quote_spanned!(span=> { #unpack #body });

        if info.kind == LambdaKind::Future {
            return if explicit {
                let l = lets(span);
                quote_spanned!(span=> { #l async move #block })
            } else {
                quote_spanned!(span=> async #block)
            };
        }
        let closure = if suspend {
            // (a closure giving a future: each call's future owns clones of what it uses)
            let inner = lets(span);
            if explicit || transform_wraps {
                quote_spanned!(span=> move |#(#params),*| { #inner async move #block })
            } else {
                quote_spanned!(span=> |#(#params),*| { #inner async move #block })
            }
        } else if explicit {
            quote_spanned!(span=> move |#(#params),*| #block)
        } else {
            quote_spanned!(span=> |#(#params),*| #block)
        };
        let closure = if explicit && !captures.is_empty() {
            let l = lets(span);
            quote_spanned!(span=> { #l #closure })
        } else {
            closure
        };
        if info.measure_policy {
            let ui = respan(self.cx.paths.ui.clone(), span);
            return quote_spanned!(span=> #ui::layout::measure_policy(#closure));
        }
        closure
    }

    /// A lambda input's type, as the index writes it.
    fn input_ty(&self, text: &str) -> Ty {
        if let Some((elem, recv)) = &self.elem_hint {
            match text.trim() {
                "Self::Item" | "T" => return elem.clone(),
                "(Self::K, Self::V)" => return recv.element(),
                _ => {}
            }
        }
        Ty::from_rust(text)
    }
}

/// Whether a lambda's body uses `it` (outside lambdas of its own that name their parameters… or take theirs).
fn uses_it(b: &Block) -> bool {
    fn expr(e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Name(n) => n.name == "it",
            ExprKind::Lambda(_) => false,
            ExprKind::String(s) => s.parts.iter().any(|p| matches!(p, StringPart::Expr { expr: x, .. } if expr(x))),
            // (a lambda in it has an `it` of its own)
            ExprKind::Call { callee, args, .. } => expr(callee) || args.iter().any(|a| expr(&a.value)),
            ExprKind::Member { receiver, .. } => expr(receiver),
            ExprKind::Index { receiver, indices } => expr(receiver) || indices.iter().any(expr),
            ExprKind::Prefix { operand, .. } | ExprKind::Postfix { operand, .. } => expr(operand),
            ExprKind::Binary { lhs, rhs, .. } | ExprKind::Infix { lhs, rhs, .. } => expr(lhs) || expr(rhs),
            ExprKind::If { condition, then, otherwise } => {
                expr(condition) || block(then) || otherwise.as_ref().is_some_and(block)
            }
            ExprKind::When(w) => {
                w.subject.as_ref().is_some_and(|s| expr(&s.expr))
                    || w.branches.iter().any(|b| {
                        block(&b.body)
                            || b.conditions.iter().any(|c| match c {
                                WhenCondition::Expr(x) => expr(x),
                                WhenCondition::In { range, .. } => expr(range),
                                WhenCondition::Is { .. } => false,
                            })
                    })
            }
            ExprKind::Paren(x) | ExprKind::Annotated { expr: x, .. } | ExprKind::Throw(x) => expr(x),
            ExprKind::Return { value, .. } => value.as_ref().is_some_and(|v| expr(v)),
            _ => false,
        }
    }
    fn block(b: &Block) -> bool {
        b.stmts.iter().any(|s| match &s.kind {
            StmtKind::Expr(e) => expr(e),
            StmtKind::Assign { target, value, .. } => expr(target) || expr(value),
            StmtKind::Decl(Decl::Property(p)) => match &p.init {
                PropertyInit::Value(e) | PropertyInit::Delegate(e) => expr(e),
                PropertyInit::None => false,
            },
            StmtKind::For { iterable, body, .. } => expr(iterable) || block(body),
            StmtKind::While { condition, body, .. } | StmtKind::DoWhile { body, condition, .. } => {
                expr(condition) || block(body)
            }
            StmtKind::Decl(_) => false,
        })
    }
    block(b)
}
