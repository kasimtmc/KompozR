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
//! - a suspending one: an `async` block (`LaunchedEffect(k) { … }`), or a closure giving one (`pointerInput(k) { … }`;
//!   boxed, `Box::pin(async move { … })`, where the closure is a trait object giving `Pin<Box<dyn Future>>`:
//!   `draggable`'s `onDragStopped`), whose suspending calls are awaited;
//! - one run in place in a coroutine whose body suspends (`with(anim) { animateTo(1f) }`, `repeat(3) { delay(10) }`:
//!   Kotlin's inline lambdas suspend with their caller): an async closure, borrowing as the plain one does, given to
//!   the callee's suspending twin (`kt::with_async(…, async |this| …).await`, see `kompoz_runtime::kt`; Compose's
//!   inline functions' too: `buildAnnotatedString`, `withStyle`, `PathData`, the draw transforms…); the twin takes
//!   each of its lambdas as an async closure, so where one of a call's suspends the others are written as async
//!   closures too (`withTransform({ translate(1f, 1f) }) { delay(1) }`); a callee without one (Kotlin's
//!   `crossinline` lambdas, functions Kotlin doesn't inline) can't take it, an error as in Kotlin; nor can Kotlin's
//!   overload taking it `crossinline` where the others don't (`withStyle(ParagraphStyle)`, known by its argument's
//!   type, one the twin doesn't take: `index::TwinBound`), an error saying so; nor Compose's snapshot blocks
//!   (`withMutableSnapshot`), which Kotlin inlines but Compose says must not suspend, an error saying so.
//!   `return@label` in it returns from the closure, as in the plain one;
//! - `Layout`'s measure policy: through `measure_policy(…)`, which gives Rust its parameters' types.
//!
//! Its parameters: as written, or `it` when it takes one and names none (Kotlin's `it` is the nearest lambda's that has
//! one: a lambda inside taking none leaves the outer one's, which is taken when a use of `it` resolves to it). Its
//! body's last expression is its value, unless the parameter's lambda gives nothing.

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
            receiver_value: None,
            suspend: false,
            boxed: false,
            stored: true,
            measure_policy: false,
            typed: false,
            twin: false,
            file_inline: false,
            snapshot_block: false,
            crossinline: None,
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
            None => self.lambda_in_place(l, call_label, false, transform_wraps),
        }
    }

    /// A lambda given to a parameter the index doesn't know (a method Rust finds by its name), run in place: it
    /// borrows; `twin`: the callee has a suspending twin (each kt shim the name may be has one), so in a coroutine it
    /// may suspend.
    pub(crate) fn lambda_in_place(
        &mut self,
        l: &Lambda,
        call_label: Option<&str>,
        twin: bool,
        transform_wraps: bool,
    ) -> TokenStream {
        let unknown = LInfo {
            kind: LambdaKind::Fn,
            inputs: Vec::new(),
            output: String::new(),
            receiver: None,
            receiver_value: None,
            suspend: false,
            boxed: false,
            stored: false,
            measure_policy: false,
            typed: false,
            twin,
            file_inline: false,
            snapshot_block: false,
            crossinline: None,
        };
        self.lambda_with(l, &unknown, None, call_label, transform_wraps, true)
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
        // (a measure policy goes through `measure_policy(…)`, which the macro memoizes without wrapping it; a closure
        // made a trait object is one the macro doesn't see)
        let explicit = info.stored && (!transform_wraps || info.measure_policy || info.typed);
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
        self.frames.push(CaptureFrame { boundary, used: Vec::new(), kept: info.stored });
        // (a result of Kotlin's `Any` — a key lambda's —: made the Rust type holding it)
        let any = known.and_then(|k| super::expr::AnyWrap::of_rust(&k.output));
        let some =
            any.is_none() && known.is_some_and(|k| super::resolve::plain_type_text(&k.output).starts_with("Option<"));
        // (a result of Kotlin's interface type held as a shared trait object — a transition spec's —: each value made
        // one, the closure's result type written so its branches' values become it)
        let shared = known.and_then(|k| super::expr::Shared::of_rust(&k.output)).filter(|_| !info.suspend);
        let result = Want { str_ok: false, any, some, shared };
        self.lambdas.push(LambdaCx { labels: labels.clone(), loops: self.loops.len(), result });
        let receivers_before = self.receivers.len();
        let mut params: Vec<TokenStream> = Vec::new();
        let mut unpack = TokenStream::new();
        let mut receiver_ident: Option<proc_macro2::Ident> = None;
        let mut receiver_owned: Option<std::rc::Rc<std::cell::Cell<bool>>> = None;
        // (a closure made a trait object writes its parameters' types: none comes from where it goes)
        let written_inputs: Vec<Option<TokenStream>> = info
            .inputs
            .iter()
            .map(|t| if info.typed { self.written_type(&self.bind_type_arg(t, span), span) } else { None })
            .collect();
        let typed = |k: usize| -> TokenStream {
            match written_inputs.get(k).cloned().flatten() {
                Some(t) => quote_spanned!(span=> : #t),
                None => quote!(),
            }
        };
        if let Some(r) = &info.receiver {
            let name =
                if self.receivers.is_empty() { "this".to_string() } else { format!("this{}", self.receivers.len()) };
            let i = proc_macro2::Ident::new(&name, span);
            // (a receiver of the callee's type parameters, `&mut DraggableAnchorsConfig<T>`, written with them
            // inferred: nothing else tells the closure its type)
            let t = match (typed(0).is_empty(), info.inputs.first()) {
                (true, Some(input)) if names_type_parameter(input) => match self.written_type(input, span) {
                    Some(t) => quote_spanned!(span=> : #t),
                    None => quote!(),
                },
                _ => typed(0),
            };
            params.push(quote!(#i #t));
            receiver_ident = Some(i.clone());
            let by_ref = info.inputs.first().is_some_and(|t| {
                let t = t.trim_start();
                t.starts_with('&') && !t.starts_with("&mut")
            });
            let by_mut = info.inputs.first().is_some_and(|t| t.trim_start().starts_with("&mut"));
            let owned_used: std::rc::Rc<std::cell::Cell<bool>> = Default::default();
            receiver_owned = Some(owned_used.clone());
            self.receivers.push(ImplicitReceiver {
                rust: name,
                ty: r.clone(),
                value: info.receiver_value.clone().unwrap_or_else(|| Ty::from_rust(r)),
                labels: labels.clone(),
                depth: boundary,
                by_ref,
                by_mut,
                owned_used,
            });
        }
        let first_input = usize::from(info.receiver.is_some());
        let input_tys: Vec<Ty> = info.inputs.iter().skip(first_input).map(|t| self.input_ty(t)).collect();
        let mut_refs: Vec<bool> =
            info.inputs.iter().skip(first_input).map(|t| t.trim_start().starts_with("&mut")).collect();
        // (its body's calls are its own: the call it is given to binds no `T` of theirs)
        let outer_type_arg = self.type_arg.take();
        let outer_owner_arg = self.owner_arg.take();
        // (an element the generic callee lends, `&T` — `items(todos) { t -> … }` —, or a value of the type argument of
        // the generic type whose method takes the lambda, `&S` — `transition.animateFloat { it… }` —: Kotlin's value of
        // it, a clone, which what the lambda keeps can capture)
        let lent: Vec<bool> = info
            .inputs
            .iter()
            .skip(first_input)
            .map(|t| {
                outer_type_arg.is_some() && is_lent_type_param(t, &self.type_param)
                    || outer_owner_arg.is_some() && lent_type_parameter(t).is_some()
            })
            .collect();
        // (the parameters it names: Kotlin's needn't be used — `{ lookaheadCoordinates -> moving.value }`)
        let mut named_params: Vec<proc_macro2::Ident> = Vec::new();
        // (the implicit `it` declared, and whether the lambda takes one parameter whatever its body does)
        let mut implicit_it: Option<(std::rc::Rc<std::cell::Cell<bool>>, bool)> = None;
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
                        None => typed(first_input + k),
                    };
                    match &p.binding {
                        Binding::Name(n) if n.name == "_" => params.push(quote_spanned!(span=> _ #annot)),
                        Binding::Name(n) => {
                            let rust = snake(&n.name);
                            let i = self.name_ident(&rust, n.loc);
                            named_params.push(i.clone());
                            self.declare(&n.name, rust, LocalKind::LambdaParam, ty);
                            if mut_refs.get(k) == Some(&true) {
                                self.mark_mut_ref(&n.name);
                            }
                            if lent.get(k) == Some(&true) {
                                unpack.extend(quote_spanned!(span=> let #i = ::core::clone::Clone::clone(#i);));
                            }
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
                // (Kotlin's `it`, the nearest lambda's that has one: declared where the lambda takes one parameter, or
                // one its callee doesn't tell, and taken when a use resolves to it — its body's, or one in a lambda
                // inside it that takes none, `xs.forEach { scope.launch { delay(it) } }`)
                let unknown = arity_from_body && known.is_none();
                if unknown || info.arity() == 1 {
                    let ty = input_tys.first().cloned().unwrap_or(Ty::Unknown);
                    let used = self.declare_it(ty);
                    if mut_refs.first() == Some(&true) {
                        self.mark_mut_ref("it");
                    }
                    implicit_it = Some((used, !unknown));
                } else {
                    for k in 0..info.arity() {
                        let t = typed(first_input + k);
                        params.push(quote_spanned!(span=> _ #t));
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
        // (one run in place in a coroutine — Kotlin's inline function's — suspends with it when its body does)
        let in_place = was_async && !suspend && !info.stored;
        let awaits_before = self.awaits;
        // (a suspend lambda's value too: `async { 7 }` gives 7, `launch { }` nothing)
        let tail = if known.is_some_and(|k| k.unit_output()) { Tail::Unit } else { Tail::Value(result) };
        let body = self.stmts(&l.body.stmts, tail);
        if let Some((used, one)) = implicit_it {
            let t = typed(first_input);
            if used.get() {
                if lent.first() == Some(&true) {
                    unpack.extend(quote_spanned!(span=> let it = ::core::clone::Clone::clone(it);));
                }
                params.push(quote_spanned!(span=> it #t));
            } else if one {
                params.push(quote_spanned!(span=> _ #t));
            }
        }
        // (a scope lent by reference that a lambda inside uses: held as a clone of it, which that lambda captures)
        if let (Some(i), Some(owned)) = (&receiver_ident, &receiver_owned)
            && owned.get()
        {
            let o = proc_macro2::Ident::new(&format!("__kz_{i}"), span);
            unpack = quote_spanned!(span=> let #o = ::core::clone::Clone::clone(#i); #unpack);
        }
        // (Kotlin's `this` needn't be used: a receiver the body leaves alone is no unused variable; nor is a parameter
        // it names and leaves alone)
        for i in receiver_ident.iter().chain(named_params.iter()) {
            if !mentions(&body, i) && !mentions(&unpack, i) {
                unpack.extend(quote_spanned!(span=> let _ = &#i;));
            }
        }
        self.in_async = was_async;
        let suspends = in_place && self.awaits > awaits_before;
        // (one a twin takes that doesn't suspend: an async closure where another lambda of the call makes it go to the
        // twin, else the plain closure — recorded by place, for the call to know)
        let forced = in_place && info.twin && self.force_async.contains(&l.loc);
        if in_place && info.twin && !suspends {
            self.plain_twin_lambdas.push(l.loc);
        }
        if suspend || info.stored {
            // (its suspension points are its own coroutine's, or errors: none of the code around it)
            self.awaits = awaits_before;
        }
        if suspends {
            if info.twin {
                // (an async closure, which the callee's suspending twin takes: the call is awaited)
                self.inline_async = true;
            } else {
                let (name, at) = self.last_await.clone().unwrap_or_else(|| (String::new(), l.loc));
                if info.file_inline {
                    self.later(at, "a suspending call in a lambda given to the file's own `inline fun`");
                } else if let Some(Crossinline { overload, suspending }) = &info.crossinline {
                    let may: Vec<String> = suspending.iter().map(|s| format!("`{s}`'s")).collect();
                    self.error(
                        at,
                        format!(
                            "`{overload}`'s lambda can't suspend: Kotlin's `{overload}` takes it `crossinline` \
                             (`{name}` suspends in it; only {} may)",
                            may.join(", ")
                        ),
                    );
                } else if info.snapshot_block {
                    let callee = call_label.map_or_else(|| "its callee".to_string(), |c| format!("`{c}`"));
                    self.error(
                        at,
                        format!(
                            "`{name}` suspends, in the block {callee} runs with a snapshot current: Compose's snapshot \
                             blocks must not suspend (the snapshot is the thread's current one until the block \
                             returns: the coroutines run meanwhile would run in it)"
                        ),
                    );
                } else {
                    let callee = call_label.map_or_else(|| "its callee".to_string(), |c| format!("`{c}`"));
                    self.error(
                        at,
                        format!(
                            "`{name}` suspends, in a lambda {callee} doesn't run in the coroutine: Kotlin suspends in a \
                             coroutine and in the lambdas an inline function runs in place (`with`, `let`, `apply`, \
                             `forEach`, `map`, `repeat`, …; not a `crossinline` one: `sortedBy`'s)"
                        ),
                    );
                }
            }
        }
        self.receivers.truncate(receivers_before);
        self.type_arg = outer_type_arg;
        self.owner_arg = outer_owner_arg;
        self.lambdas.pop();
        let frame = self.frames.pop().expect("its frame");
        self.pop_scope();
        let captures: Vec<proc_macro2::Ident> =
            frame.used.iter().filter(|u| *u != "self").map(|u| proc_macro2::Ident::new(u, span)).collect();
        // (an outer receiver lambda's scope, given by reference: the scope itself, cloned)
        let scopes: Vec<String> = self.receivers.iter().filter(|r| r.by_ref).map(|r| r.rust.clone()).collect();
        let lets = |span: Span| -> TokenStream {
            let lets = captures.iter().map(|c| {
                if scopes.contains(&c.to_string()) {
                    quote_spanned!(span=> let #c = #rt::__private::capture(#c);)
                } else {
                    quote_spanned!(span=> let #c = #rt::__private::capture(&#c);)
                }
            });
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
        let closure = if suspend && info.boxed {
            // (a trait object giving a boxed future: the closure says so, so the box is made one)
            let inner = lets(span);
            let output = self.written_type(&info.output, span).unwrap_or_else(|| quote_spanned!(span=> _));
            let ret =
                quote_spanned!(span=> ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<Output = #output>>>);
            let mv = if explicit || transform_wraps { quote!(move) } else { quote!() };
            quote_spanned!(span=> #mv |#(#params),*| -> #ret { #inner ::std::boxed::Box::pin(async move #block) })
        } else if suspend {
            // (a closure giving a future: each call's future owns clones of what it uses)
            let inner = lets(span);
            if explicit || transform_wraps {
                quote_spanned!(span=> move |#(#params),*| { #inner async move #block })
            } else {
                quote_spanned!(span=> |#(#params),*| { #inner async move #block })
            }
        } else if (suspends || forced) && info.twin {
            // (run in place, suspending with the coroutine: borrowing what it uses, as the plain one does)
            quote_spanned!(span=> async |#(#params),*| #block)
        } else {
            let mv = if explicit { quote!(move) } else { quote!() };
            match shared.and_then(|_| self.written_type(&info.output, span)) {
                Some(output) => quote_spanned!(span=> #mv |#(#params),*| -> #output #block),
                None => quote_spanned!(span=> #mv |#(#params),*| #block),
            }
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
        let bare = text.trim();
        let bare = bare.strip_prefix("&mut ").or_else(|| bare.strip_prefix('&')).unwrap_or(bare).trim();
        // (a reference's named lifetime: `&'a T`)
        let bare = match bare.strip_prefix('\'') {
            Some(rest) => rest.split_once(' ').map_or(bare, |(_, t)| t.trim()),
            None => bare,
        };
        if let Some((elem, recv)) = &self.elem_hint {
            match bare {
                "Self::Item" | "T" => return elem.clone(),
                "(Self::K, Self::V)" => return recv.element(),
                // (a scope function's `it`: the value itself, `x.let { it… }`)
                "Self" if *recv != Ty::Unknown => return recv.clone(),
                _ => {}
            }
        }
        if bare == "T"
            && let Some(t) = &self.type_arg
        {
            return t.clone();
        }
        // (the type argument of the generic type whose method takes the lambda: `&S` of a `Transition<Panel>`)
        if let Some(t) = &self.owner_arg
            && lent_type_parameter(text).is_some()
        {
            return t.clone();
        }
        Ty::from_rust(text)
    }

    /// A Rust type's text (an index's) with the generic call's element type for its `T`, when it is known
    /// ([`Gen::type_arg`]): `&'a T` → `&'a Row2`.
    pub(crate) fn bind_type_arg(&self, text: &str, span: Span) -> String {
        let Some(t) = self.type_arg.as_ref().filter(|t| **t != Ty::Unknown) else { return text.to_string() };
        let rust = self.rust_of_ty(t, span).to_string();
        let mut out = String::new();
        let mut word = String::new();
        let flush = |word: &mut String, out: &mut String| {
            if *word == self.type_param {
                out.push_str(&rust);
            } else {
                out.push_str(word);
            }
            word.clear();
        };
        for c in text.chars() {
            if c.is_alphanumeric() || c == '_' || (c == '\'' && word.is_empty()) {
                word.push(c);
            } else {
                flush(&mut word, &mut out);
                out.push(c);
            }
        }
        flush(&mut word, &mut out);
        out
    }
}

/// Whether a type's text names a type parameter of the function's (a single capital letter: `Config<T>`).
fn names_type_parameter(text: &str) -> bool {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|w| w.len() == 1 && w.chars().all(|c| c.is_ascii_uppercase()))
}

impl Gen<'_, '_> {
    /// A Rust type of the API index's (its text) as written in generated code, when it names no generic parameter
    /// (`Self::Item`, `T`) or `impl` type: the standard library's names by their paths (`Rc` → `::std::rc::Rc`), the
    /// UI's drawing scopes by theirs (`DrawScope` → `::kompoz::ui::draw::DrawScope`: the prelude has the module, not
    /// its names), the API's others as they are (the prelude has them).
    pub(crate) fn written_type(&self, text: &str, span: Span) -> Option<TokenStream> {
        let words = || text.split(|c: char| !(c.is_alphanumeric() || c == '_'));
        if words().any(|w| w == "Self" || w == "impl") {
            return None;
        }
        let tokens = text.parse::<TokenStream>().ok()?;
        // (the function's type parameters, `T`, left to inference: `Box<dyn FnMut(&Animatable<_, _>)>` still tells
        // the closure what its parameter is)
        Some(respan(std_paths(type_params_inferred(tokens), &self.cx.paths.ui), span))
    }
}

/// `tokens` with a type parameter (a single capital letter) written `_`, a named lifetime `'_` and a `for<…>`
/// left out (a closure's elided lifetimes are higher-ranked): what a closure's type can be written as where the
/// function's generics aren't known.
fn type_params_inferred(tokens: TokenStream) -> TokenStream {
    use proc_macro2::{Group, Ident, Punct, Spacing, TokenTree};
    let all: Vec<TokenTree> = tokens.into_iter().collect();
    let mut out: Vec<TokenTree> = Vec::new();
    let mut k = 0;
    while k < all.len() {
        match &all[k] {
            // `for<'a, …>`: left out
            TokenTree::Ident(i)
                if i == "for" && matches!(all.get(k + 1), Some(TokenTree::Punct(p)) if p.as_char() == '<') =>
            {
                let mut depth = 0;
                k += 1;
                while k < all.len() {
                    if let TokenTree::Punct(p) = &all[k] {
                        match p.as_char() {
                            '<' => depth += 1,
                            '>' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    k += 1;
                }
            }
            // `'a` (not `'static`): `'_`
            TokenTree::Punct(p)
                if p.as_char() == '\'' && matches!(all.get(k + 1), Some(TokenTree::Ident(i)) if i != "static") =>
            {
                let mut quote = Punct::new('\'', Spacing::Joint);
                quote.set_span(p.span());
                out.push(TokenTree::Punct(quote));
                out.push(TokenTree::Ident(Ident::new("_", p.span())));
                k += 1;
            }
            TokenTree::Punct(p) if p.as_char() == '\'' => {
                out.push(all[k].clone());
                if let Some(next) = all.get(k + 1) {
                    out.push(next.clone());
                }
                k += 1;
            }
            TokenTree::Ident(i)
                if {
                    let w = i.to_string();
                    w.len() == 1 && w.starts_with(|c: char| c.is_ascii_uppercase())
                } =>
            {
                out.push(TokenTree::Ident(Ident::new("_", i.span())));
            }
            TokenTree::Group(g) => {
                let mut ng = Group::new(g.delimiter(), type_params_inferred(g.stream()));
                ng.set_span(g.span());
                out.push(TokenTree::Group(ng));
            }
            other => out.push(other.clone()),
        }
        k += 1;
    }
    out.into_iter().collect()
}

/// `tokens` with the standard library's types they name unqualified written by their paths, and the UI's drawing
/// scopes (under `ui`).
fn std_paths(tokens: TokenStream, ui: &TokenStream) -> TokenStream {
    use proc_macro2::{Group, TokenTree};
    let mut out = Vec::new();
    let mut after_path = false;
    for t in tokens {
        match t {
            TokenTree::Ident(i) if !after_path => {
                let path = match i.to_string().as_str() {
                    "Rc" => Some(quote!(::std::rc::Rc)),
                    "Arc" => Some(quote!(::std::sync::Arc)),
                    "Box" => Some(quote!(::std::boxed::Box)),
                    "Fn" => Some(quote!(::core::ops::Fn)),
                    "FnMut" => Some(quote!(::core::ops::FnMut)),
                    "FnOnce" => Some(quote!(::core::ops::FnOnce)),
                    "Option" => Some(quote!(::core::option::Option)),
                    "Vec" => Some(quote!(::std::vec::Vec)),
                    "String" => Some(quote!(::std::string::String)),
                    "DrawScope" | "ContentDrawScope" => Some(quote!(#ui::draw::#i)),
                    _ => None,
                };
                match path {
                    Some(p) => out.extend(p),
                    None => out.push(TokenTree::Ident(i)),
                }
                after_path = false;
            }
            TokenTree::Group(g) => {
                let mut ng = Group::new(g.delimiter(), std_paths(g.stream(), ui));
                ng.set_span(g.span());
                out.push(TokenTree::Group(ng));
                after_path = false;
            }
            TokenTree::Punct(p) => {
                after_path = p.as_char() == ':';
                out.push(TokenTree::Punct(p));
            }
            other => {
                after_path = false;
                out.push(other);
            }
        }
    }
    out.into_iter().collect()
}

/// Whether a lambda input's type (an index's text) is a shared reference to the callee's type parameter: `&T`,
/// `&'a T`.
fn is_lent_type_param(text: &str, name: &str) -> bool {
    let Some(rest) = text.trim().strip_prefix('&') else { return false };
    let rest = rest.trim_start();
    if rest.starts_with("mut ") {
        return false;
    }
    let rest = match rest.strip_prefix('\'') {
        Some(r) => r.split_once(' ').map_or("", |(_, t)| t.trim()),
        None => rest,
    };
    rest == name
}

/// The type parameter a shared reference names (`&S`, `&'a S`: a single capital letter), lent to a lambda.
fn lent_type_parameter(text: &str) -> Option<&str> {
    let rest = text.trim().strip_prefix('&')?.trim_start();
    if rest.starts_with("mut ") {
        return None;
    }
    let rest = match rest.strip_prefix('\'') {
        Some(r) => r.split_once(' ').map_or("", |(_, t)| t.trim()),
        None => rest,
    };
    (rest.len() == 1 && rest.chars().all(|c| c.is_ascii_uppercase())).then_some(rest)
}

/// Whether `tokens` name `ident` anywhere (in groups too).
fn mentions(tokens: &TokenStream, ident: &proc_macro2::Ident) -> bool {
    tokens.clone().into_iter().any(|t| match t {
        proc_macro2::TokenTree::Ident(i) => i == *ident,
        proc_macro2::TokenTree::Group(g) => mentions(&g.stream(), ident),
        _ => false,
    })
}
