//! Calls: what a call calls (a local function value, an implicit receiver's member, the file's function, the API
//! index's function, a method of a value, a type's associated function, a kt shim), which overload, and its
//! arguments: in place (positional), by name and with defaults through the callee's builder, a trailing lambda to
//! the last parameter, varargs as an array, nullables as `Some(…)`, references as `&…`, a missing Kotlin default
//! translated from the index's doc line.
//!
//! A call that names an argument or leaves a default out goes through the builder `#[kompozable]` (or
//! `named_args!`) makes: the generator writes `f(a, name = b)` and the macro rewrites it. A PascalCase call in a
//! composable body always goes through `Probe` (the macro's), whatever its arguments.

use super::expr::{AnyWrap, Out, OutKind, Want};
use super::names::{ident, is_pascal, snake};
use super::resolve::*;
use super::{Gen, respan};
use crate::ast::*;
use crate::index::{Kind, Receiver};
use crate::pos::Loc;
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};

/// An argument of a call: positional, named, or the trailing lambda.
#[derive(Clone, Copy)]
struct ArgRef<'e> {
    name: Option<&'e Ident>,
    value: ArgValue<'e>,
    spread: bool,
    loc: Loc,
}

#[derive(Clone, Copy)]
enum ArgValue<'e> {
    Expr(&'e Expr),
    Lambda(&'e Lambda),
    /// A fun interface's SAM constructor (`TextFieldDecorator { innerTextField -> … }`, `expr`): given to a
    /// parameter taking a lambda, its lambda, as Kotlin's SAM conversion makes the one the other.
    Sam {
        expr: &'e Expr,
        lambda: &'e Lambda,
    },
}

impl ArgValue<'_> {
    fn is_lambda(&self) -> bool {
        matches!(
            self,
            ArgValue::Lambda(_) | ArgValue::Sam { .. } | ArgValue::Expr(Expr { kind: ExprKind::Lambda(_), .. })
        )
    }

    fn lambda(&self) -> Option<&Lambda> {
        match self {
            ArgValue::Lambda(l) | ArgValue::Sam { lambda: l, .. } => Some(l),
            ArgValue::Expr(Expr { kind: ExprKind::Lambda(l), .. }) => Some(l),
            ArgValue::Expr(Expr { kind: ExprKind::Annotated { expr, .. }, .. }) => match &expr.kind {
                ExprKind::Lambda(l) => Some(l),
                _ => None,
            },
            _ => None,
        }
    }
}

/// How the callee is reached.
#[derive(Clone)]
enum Target {
    /// A path call: `name(…)` (a PascalCase one goes through `Probe` in a composable body).
    Path(TokenStream, String),
    /// A method call on a receiver: `recv.name(…)`.
    Method(TokenStream),
    /// Through a trait by name, the receiver its first argument: `Trait::name(&recv, …)`.
    Ufcs(TokenStream, TokenStream),
    /// A method of an implicit receiver whose first argument is the value it extends: `scope.place(&p, …)`
    /// (Kotlin's member extension `p.place(…)` in a `PlacementScope`).
    MethodFirst(TokenStream, TokenStream),
}

/// The arguments mapped to the callee's parameters.
struct Mapping<'e> {
    /// For each parameter: the arguments given to it (several for a vararg, or keys gathered in a tuple).
    given: Vec<Vec<ArgRef<'e>>>,
    /// Whether the arguments were named, in the order written: (parameter index, named).
    order: Vec<(usize, bool, bool)>,
}

impl Gen<'_, '_> {
    pub(crate) fn call(
        &mut self,
        e: &Expr,
        callee: &Expr,
        type_args: &[TypeArg],
        args: &[Arg],
        trailing: Option<&Lambda>,
    ) -> Out {
        let span = self.span(e.loc);
        let mut list: Vec<ArgRef> = args
            .iter()
            .map(|a| {
                let value = match self.sam_lambda(&a.value) {
                    Some(lambda) => ArgValue::Sam { expr: &a.value, lambda },
                    None => ArgValue::Expr(&a.value),
                };
                ArgRef { name: a.name.as_ref(), value, spread: a.spread, loc: a.loc }
            })
            .collect();
        if let Some(l) = trailing {
            list.push(ArgRef { name: None, value: ArgValue::Lambda(l), spread: false, loc: l.loc });
        }
        if let Some(a) = list.iter().find(|a| a.spread) {
            return Out::plain(self.later(a.loc, "a spread argument (`*array`)"), Ty::Unknown);
        }
        let out = match &callee.kind {
            ExprKind::Name(n) => self.name_call(n, &list, e.loc),
            ExprKind::Member { receiver, safe, name } => self.member_call(receiver, *safe, name, &list, e.loc),
            _ => {
                let f = self.expr(callee).ts;
                let vals: Vec<TokenStream> = list.iter().map(|a| self.loose_arg(a)).collect();
                Out::plain(quote_spanned!(span=> (#f)(#(#vals),*)), Ty::Unknown)
            }
        };
        if type_args.is_empty() {
            return out;
        }
        self.typed_call(out, callee, type_args, span)
    }

    /// The lambda of a fun interface's SAM constructor (`TextFieldDecorator { … }`, `KeyboardActionHandler({ … })`):
    /// a call of the interface's name (none of the file's or a local's) with a lambda its only argument.
    fn sam_lambda<'e>(&self, e: &'e Expr) -> Option<&'e Lambda> {
        let ExprKind::Call { callee, type_args, args, trailing_lambda } = &e.kind else { return None };
        let ExprKind::Name(n) = &callee.kind else { return None };
        if !type_args.is_empty() || self.is_local(&n.name) || self.file.function(&n.name).is_some() {
            return None;
        }
        fun_interface(&n.name)?;
        match (args.as_slice(), trailing_lambda) {
            ([], Some(l)) => Some(l),
            ([a], None) if a.name.is_none() && !a.spread => match &a.value.kind {
                ExprKind::Lambda(l) => Some(l),
                _ => None,
            },
            _ => None,
        }
    }

    /// `listOf<String>()`: the type arguments of what makes a collection or a state as the value's type (a Rust
    /// function taking `impl Trait` can't take them in place).
    fn typed_call(&mut self, out: Out, callee: &Expr, type_args: &[TypeArg], span: Span) -> Out {
        let name = match &callee.kind {
            ExprKind::Name(n) => n.name.as_str(),
            ExprKind::Member { name, .. } => name.name.as_str(),
            _ => "",
        };
        let args: Vec<TokenStream> = type_args
            .iter()
            .map(|a| match a {
                TypeArg::Type { ty, .. } => self.rust_type(ty, super::types::TypePos::Value),
                TypeArg::Star(_) => quote_spanned!(span=> _),
            })
            .collect();
        let kt = self.kt(span);
        let a0 = args.first().cloned().unwrap_or_else(|| quote_spanned!(span=> _));
        let a1 = args.get(1).cloned().unwrap_or_else(|| quote_spanned!(span=> _));
        let ty = match name {
            "listOf" | "mutableListOf" | "emptyList" | "arrayListOf" | "listOfNotNull" => {
                quote_spanned!(span=> ::std::vec::Vec<#a0>)
            }
            "setOf" | "mutableSetOf" | "emptySet" => quote_spanned!(span=> #kt::KtSet<#a0>),
            "mapOf" | "mutableMapOf" | "emptyMap" => quote_spanned!(span=> #kt::KtMap<#a0, #a1>),
            "mutableStateOf" => quote_spanned!(span=> MutableState<#a0>),
            "mutableStateListOf" => quote_spanned!(span=> SnapshotStateList<#a0>),
            _ => return out,
        };
        let ts = out.ts;
        Out { ts: quote_spanned!(span=> ::core::convert::identity::<#ty>(#ts)), ty: out.ty, kind: out.kind }
    }

    // ---- `name(…)` ---------------------------------------------------------------------------------------------

    fn name_call(&mut self, n: &Ident, args: &[ArgRef], loc: Loc) -> Out {
        let span = self.span(loc);
        let nspan = self.span(n.loc);
        let kt = self.kt(span);
        // A local holding a function.
        if self.is_local(&n.name) {
            let l = self.local(&n.name).expect("a local");
            let i = ident(&l.rust, nspan);
            let f = if l.kind == LocalKind::Delegated {
                quote_spanned!(span=> (#kt::KtDelegate::get_value(&#i)))
            } else {
                quote!(#i)
            };
            let (receiver, ret, params) = match &l.ty {
                Ty::Fun(ft) => (ft.receiver.clone(), ft.ret.clone(), ft.params.clone()),
                _ => (None, Ty::Unknown, Vec::new()),
            };
            let mut vals = Vec::new();
            if let Some(r) = receiver {
                // (its receiver: the implicit one of that type, or the innermost)
                let label = self.receivers.iter().rev().find(|x| x.ty == r).and_then(|x| x.labels.first().cloned());
                if let Some(x) = self.receiver(label.as_deref()) {
                    let xi = proc_macro2::Ident::new(&x.rust, span);
                    vals.push(quote!(#xi));
                }
            }
            // (a `String` parameter takes a `String`: a literal made one; a number parameter Kotlin's number)
            for (k, a) in args.iter().enumerate() {
                vals.push(match (a.value, params.get(k)) {
                    (ArgValue::Expr(e), Some(Ty::String)) if !a.value.is_lambda() => {
                        self.value_want(e, Want { str_ok: false, any: None, some: false, shared: None })
                    }
                    (ArgValue::Expr(e), Some(to)) if to.is_number() && !a.value.is_lambda() => {
                        let from = self.peek_ty(e);
                        let v = self.value(e);
                        self.kotlin_number(v, &from, to, span)
                    }
                    _ => self.loose_arg(a),
                });
            }
            return Out::plain(quote_spanned!(span=> #f(#(#vals),*)), ret);
        }
        // Special forms.
        if let Some(out) = self.special_call(n, args, loc) {
            return out;
        }
        // A member of an implicit receiver (`drawCircle(…)` in a DrawScope lambda), or an extension function of the
        // file for it (`framed()` in another extension of `Modifier`).
        let found = self.receivers.iter().rev().find_map(|r| {
            let items: Vec<&'static crate::index::Item> =
                members_of(&r.ty, &n.name).filter(|i| i.kind == Kind::Method).collect();
            if !items.is_empty() {
                return Some((r.clone(), Some(items), None));
            }
            self.file.extension(&n.name, Some(&r.ty)).map(|f| (r.clone(), None, Some(f.callee())))
        });
        // A kt shim's member of an implicit receiver (`uppercase()` in an extension of `String`).
        if found.is_none()
            && let Some((r, items)) = self.receivers.iter().rev().find_map(|r| {
                let ty = Ty::from_rust(&r.ty);
                let items = kt_members(&n.name, ty.kt_traits());
                (!items.is_empty() && ty != Ty::Unknown).then(|| (r.clone(), items))
            })
        {
            let label = r.labels.first().cloned();
            let _ = self.receiver(label.as_deref());
            let ri = proc_macro2::Ident::new(&r.rust, span);
            let ty = Ty::from_rust(&r.ty);
            let recv =
                Out { ts: quote!(#ri), ty, kind: if r.rust == "self" { OutKind::Local } else { OutKind::Plain } };
            let _ = items;
            return self.method_on(recv, n, args, loc);
        }
        if let Some((r, items, file)) = found {
            let label = r.labels.first().cloned();
            let _ = self.receiver(label.as_deref());
            let ri = proc_macro2::Ident::new(&r.rust, span);
            let recv_ts = if r.rust == "self" { quote_spanned!(span=> self.clone()) } else { quote!(#ri) };
            let callee = match (items, file) {
                (Some(items), _) => self.choose(&items, args, loc),
                (None, Some(f)) => Some(f),
                _ => None,
            };
            if let Some(c) = callee {
                let recv = match c.item.map(|i| i.receiver) {
                    Some(Receiver::Value) | None => recv_ts,
                    _ => quote!(#ri),
                };
                return self.emit(&c, Target::Method(recv), args, loc, &n.name);
            }
        }
        // The file's own function.
        if let Some(f) = self.file.function(&n.name) {
            let c = f.callee();
            let path = ident(&c.rust, nspan);
            return self.emit(&c, Target::Path(quote!(#path), c.rust.clone()), args, loc, &n.name);
        }
        // The index's functions (and overloaded constructors: `Color(…)`).
        let mut items: Vec<&'static crate::index::Item> = free_functions(&n.name).collect();
        if !items.is_empty() {
            if items.iter().any(|i| i.has(crate::index::flags::OVERLOADED)) {
                return self.overloaded(&n.name, items[0], args, loc);
            }
            // (Kotlin's other constructors of the type the function is named after: its associated functions of that
            // Kotlin name — `PaddingValues(all)` is `PaddingValues::all`, `PaddingValues(horizontal = 8.dp)` is
            // `PaddingValues::hv`)
            items.extend(constructor_overloads(&n.name));
            if let Some(c) = self.choose(&items, args, loc) {
                let item = c.item.expect("an index item");
                let name = ident(&c.rust, nspan);
                let path = if item.krate == "kompoz_runtime" && item.module.starts_with("kt") {
                    quote_spanned!(span=> #kt::#name)
                } else if !item.owner.is_empty() {
                    let owner = ident(item.owner, nspan);
                    quote_spanned!(span=> #owner::#name)
                } else {
                    quote!(#name)
                };
                return self.emit(&c, Target::Path(path, c.rust.clone()), args, loc, &n.name);
            }
            return Out::plain(quote_spanned!(span=> ::core::unreachable!()), Ty::Unknown);
        }
        // A fun interface's SAM constructor (`TextFieldDecorator { innerTextField -> … }`) as a value: its lambda made
        // the trait's object as the API holds it (`Rc<dyn TextFieldDecorator>`: Kotlin's value of the interface).
        if let Some(fi) = fun_interface(&n.name)
            && let [a] = args
            && a.name.is_none()
            && let Some(l) = a.value.lambda()
        {
            let mut info = Callee::from_item(fi).params.remove(0).lambda.expect("a fun interface's lambda");
            // (kept, as a trait object is: capturing clones; its parameters typed for Rust to find the blanket impl)
            (info.stored, info.typed) = (true, true);
            let closure = self.lambda_for(l, Some(&info), Some(&n.name), false);
            let t = ident(&n.name, nspan);
            let ptr = if trait_object(&n.name) == Some("Arc") { "Arc" } else { "Rc" };
            let (ty, made) = if ptr == "Arc" {
                (
                    quote_spanned!(span=> ::std::sync::Arc<dyn #t>),
                    quote_spanned!(span=> ::std::sync::Arc::new(#closure)),
                )
            } else {
                (quote_spanned!(span=> ::std::rc::Rc<dyn #t>), quote_spanned!(span=> ::std::rc::Rc::new(#closure)))
            };
            let f = self.fresh("f", span);
            return Out::plain(
                quote_spanned!(span=> { let #f: #ty = #made; #f }),
                Ty::from_rust(&format!("{ptr}<dyn {}>", n.name)),
            );
        }
        // A type's constructor or a function the index doesn't know (Rust's own, another file's): as written, its
        // arguments as given — a string literal to a PascalCase one (a composable of another file, a constructor) as
        // Kotlin's `String`, which its parameters take (`impl Into<String>`, a defaulted one's `String`).
        let pascal = is_pascal(&n.name);
        let rust = if pascal { n.name.clone() } else { snake(&n.name) };
        let name = self.name_ident(&rust, n.loc);
        let mut vals = Vec::new();
        for a in args {
            let v = match a.value {
                ArgValue::Expr(e) if pascal && !a.value.is_lambda() => {
                    self.value_want(e, Want { str_ok: false, any: None, some: false, shared: None })
                }
                _ => self.loose_arg(a),
            };
            vals.push(match a.name {
                Some(an) => {
                    let ai = ident(&snake(&an.name), self.span(an.loc));
                    quote_spanned!(span=> #ai = #v)
                }
                None => v,
            });
        }
        let ty = if is_pascal(&n.name) { Ty::named(&n.name) } else { Ty::Unknown };
        Out::plain(quote_spanned!(span=> #name(#(#vals),*)), ty)
    }

    /// Calls that are written otherwise than the index says: `remember(k) { … }` (keyed), `x.toString()`.
    fn special_call(&mut self, n: &Ident, args: &[ArgRef], loc: Loc) -> Option<Out> {
        let span = self.span(loc);
        let keyed = match n.name.as_str() {
            "remember" => "rememberKeyed",
            "rememberSaveable" => "rememberSaveableKeyed",
            _ => return None,
        };
        let keys: Vec<&ArgRef> = args.iter().filter(|a| !a.value.is_lambda() && a.name.is_none()).collect();
        if keys.is_empty() {
            return None;
        }
        let items: Vec<&'static crate::index::Item> = free_functions(keyed).collect();
        let item = *items.first()?;
        let c = Callee::from_item(item);
        let name = ident(item.rust, self.span(n.loc));
        let keys_ts = self.keys_tuple(&keys, span);
        let lambda = args.iter().find(|a| a.value.is_lambda());
        let init = match lambda {
            Some(a) => self.arg_value(a, Some(&c.params[1]), false),
            None => {
                self.error(loc, format!("`{}` takes its value as a lambda", n.name));
                quote!()
            }
        };
        let ret = lambda.and_then(|a| a.value.lambda()).map(|l| self.peek_block(&l.body)).unwrap_or(Ty::Unknown);
        let ts = self.identified(quote_spanned!(span=> #name(#keys_ts, #init)), loc, true);
        Some(Out::plain(ts, ret))
    }

    /// Keys given as several arguments: one tuple (`LaunchedEffect(a, b)` → `(a, b)`), `Unit` → `()`.
    fn keys_tuple(&mut self, keys: &[&ArgRef], span: Span) -> TokenStream {
        let vals: Vec<TokenStream> = keys.iter().map(|a| self.loose_arg(a)).collect();
        if vals.len() == 1 { vals.into_iter().next().unwrap() } else { quote_spanned!(span=> (#(#vals,)*)) }
    }

    /// `Color(…)`: Kotlin's overloads of a constructor taken as one argument, or as a tuple of several.
    fn overloaded(&mut self, kotlin: &str, f: &'static crate::index::Item, args: &[ArgRef], loc: Loc) -> Out {
        let span = self.span(loc);
        let overloads: Vec<&'static crate::index::Item> = lookup(kotlin)
            .iter()
            .filter(|i| i.kind == Kind::Overload && i.via == f.params[0].ty.trim_start_matches("impl "))
            .collect();
        let overloads = if overloads.is_empty() {
            lookup(kotlin).iter().filter(|i| i.kind == Kind::Overload).collect()
        } else {
            overloads
        };
        let any_float = args.iter().any(|a| matches!(a.value, ArgValue::Expr(e) if self.peek_ty(e).is_float()));
        let long = args.len() == 1 && matches!(args[0].value, ArgValue::Expr(e) if self.peek_ty(e) == Ty::Long);
        let chosen = overloads.iter().find(|o| {
            o.params.len() == args.len()
                && args.iter().zip(o.params).all(|(a, p)| {
                    a.name.is_none_or(|n| n.name == p.kotlin)
                        && match Ty::from_rust(p.ty) {
                            Ty::Float => any_float,
                            Ty::Int => !any_float,
                            Ty::Long => long,
                            // (an offset's `usize`: Kotlin's Int there — `TextRange(0, 3)`)
                            Ty::OtherInt => !any_float && !long,
                            _ => true,
                        }
                })
        });
        let Some(o) = chosen else {
            self.error(loc, format!("no `{kotlin}` takes these arguments"));
            return Out::plain(quote_spanned!(span=> ::core::unreachable!()), Ty::Unknown);
        };
        let mut vals: Vec<Option<TokenStream>> = vec![None; o.params.len()];
        for (k, a) in args.iter().enumerate() {
            let at = match a.name {
                Some(n) => o.params.iter().position(|p| p.kotlin == n.name).unwrap_or(k),
                None => k,
            };
            let p = &o.params[at];
            let v = match a.value {
                ArgValue::Expr(e) if Ty::from_rust(p.ty).is_float() && int_literal(e).is_some() => {
                    let neg = matches!(e.kind, ExprKind::Prefix { op: PrefixOp::Minus, .. });
                    let n = self.number(int_literal(e).unwrap(), e.loc, true).ts;
                    if neg { quote_spanned!(span=> -#n) } else { n }
                }
                _ => self.loose_arg(a),
            };
            let v = if p.ty.starts_with('&') { quote_spanned!(span=> &#v) } else { v };
            vals[at] = Some(v);
        }
        let vals: Vec<TokenStream> =
            vals.into_iter().map(|v| v.unwrap_or_else(|| quote_spanned!(span=> ::core::unreachable!()))).collect();
        let name = ident(f.rust, span);
        let arg = if vals.len() == 1 { vals[0].clone() } else { quote_spanned!(span=> (#(#vals),*)) };
        // (what it gives: its overloads' associated type, `A::Animatable`, is the type of that name)
        let ret = match f.ret.rsplit_once("::") {
            Some((_, assoc)) if assoc.chars().next().is_some_and(char::is_uppercase) => Ty::named(assoc),
            _ => Ty::from_rust(f.ret),
        };
        Out::plain(quote_spanned!(span=> #name(#arg)), ret)
    }

    // ---- `recv.name(…)` ----------------------------------------------------------------------------------------

    fn member_call(&mut self, receiver: &Expr, safe: bool, name: &Ident, args: &[ArgRef], loc: Loc) -> Out {
        let span = self.span(loc);
        let nspan = self.span(name.loc);
        let kt = self.kt(span);
        // `Type.f(…)`, `Object.f(…)`, `Modifier.padding(…)`.
        if let Some(q) = self.qualifier(receiver) {
            let qi = self.name_ident(&q, receiver.loc);
            let qspan = self.span(receiver.loc);
            // (a Kotlin object whose members are methods of a unit struct of its name: `PagerDefaults.flingBehavior(…)`
            // is `PagerDefaults.fling_behavior(…)`, the struct its own value)
            let object_member = lookup(&name.name).iter().any(|i| i.kind == Kind::Method && i.owner == q);
            if is_value_object(&q) || object_member {
                return self.method_on(Out::plain(quote_spanned!(qspan=> #qi), Ty::named(&q)), name, args, loc);
            }
            let items: Vec<&'static crate::index::Item> =
                associated(&q, &name.name).into_iter().filter(|i| i.kind == Kind::Function).collect();
            if let Some(c) = (!items.is_empty()).then(|| self.choose(&items, args, loc)).flatten() {
                let m = ident(&c.rust, nspan);
                return self.emit(
                    &c,
                    Target::Path(quote_spanned!(span=> #qi::#m), c.rust.clone()),
                    args,
                    loc,
                    &name.name,
                );
            }
            let m = self.name_ident(&snake(&name.name), name.loc);
            let vals: Vec<TokenStream> = args.iter().map(|a| self.loose_arg(a)).collect();
            return Out::plain(quote_spanned!(span=> #qi::#m(#(#vals),*)), Ty::Unknown);
        }
        let recv = self.expr(receiver);
        if safe {
            let it = self.fresh("it", span);
            let inner_ty = recv.ty.non_null();
            let value = self.finish(recv, Want::default(), receiver.loc);
            let inner = self.method_on(Out { ts: quote!(#it), ty: inner_ty, kind: OutKind::Plain }, name, args, loc);
            let ts = inner.ts;
            return Out::plain(quote_spanned!(span=> #kt::safe!(#value, |#it| #ts)), inner.ty.nullable());
        }
        self.method_on(recv, name, args, loc)
    }

    /// `lhs name rhs` (an infix call) as a call of a member: the left side's type's own (`entity using easing`, one
    /// of its overloads by the argument), or a member extension of an implicit receiver (`1.3f at 100` in a
    /// `keyframes { }` block: the config's `at(value, timeStamp)`); `None` when neither has one of that name.
    pub(crate) fn infix_member(&mut self, name: &Ident, lhs: &Expr, rhs: &Expr, loc: Loc) -> Option<Out> {
        let lhs_ty = self.peek_ty(lhs);
        let own = lhs_ty.owner().is_some_and(|o| members_of(o, &name.name).any(|i| i.kind == Kind::Method));
        let extension = self
            .receivers
            .iter()
            .any(|r| members_of(&r.ty, &name.name).any(|i| i.kind == Kind::Method && i.params.len() == 2));
        if !own && !extension {
            return None;
        }
        let recv = self.expr(lhs);
        let arg = ArgRef { name: None, value: ArgValue::Expr(rhs), spread: false, loc: rhs.loc };
        Some(self.method_on(recv, name, &[arg], loc))
    }

    /// A method call on a value written as `recv`.
    fn method_on(&mut self, recv: Out, name: &Ident, args: &[ArgRef], loc: Loc) -> Out {
        let span = self.span(loc);
        let nspan = self.span(name.loc);
        let kt = self.kt(span);
        // `x.toString()`: Kotlin's text of it.
        if name.name == "toString" && args.is_empty() {
            let r = recv.ts;
            return Out::plain(quote_spanned!(span=> #kt::to_string(&#r)), Ty::String);
        }
        let owner = recv.ty.owner().map(str::to_string);
        // An extension function of the file.
        if let Some(f) = self.file.extension(&name.name, owner.as_deref()).cloned()
            && (owner.is_some() || self.file.extension(&name.name, None).is_some())
        {
            let r = self.finish_recv(recv, Receiver::Value, loc);
            return self.emit(&f.callee(), Target::Method(r), args, loc, &name.name);
        }
        // The type's own methods (a generic one's lambdas take its type argument: `state.update { it… }`).
        if let Some(o) = &owner {
            let items: Vec<&'static crate::index::Item> =
                members_of(o, &name.name).filter(|i| i.kind == Kind::Method).collect();
            if !items.is_empty()
                && let Some(c) = self.choose(&items, args, loc)
            {
                let kind = c.item.map_or(Receiver::Ref, |i| i.receiver);
                let hint = match &recv.ty {
                    Ty::Named(_, a) if a.len() == 1 => Some((a[0].clone(), recv.ty.clone())),
                    _ => None,
                };
                let owner_arg = hint.as_ref().map(|(a, _)| a.clone()).filter(|a| *a != Ty::Unknown);
                let r = self.finish_recv(recv, kind, loc);
                let saved = std::mem::replace(&mut self.elem_hint, hint);
                let saved_owner = std::mem::replace(&mut self.owner_arg, owner_arg);
                let out = self.emit(&c, Target::Method(r), args, loc, &name.name);
                self.elem_hint = saved;
                self.owner_arg = saved_owner;
                return out;
            }
        }
        // The kt shims: through their trait.
        let traits = recv.ty.kt_traits();
        let kt_items = if recv.ty == Ty::Unknown {
            let all = kt_members(&name.name, &[]);
            let one_trait = all.first().is_some_and(|f| all.iter().all(|i| i.via == f.via));
            if one_trait && !ambiguous_member(&name.name) { all } else { Vec::new() }
        } else {
            kt_members(&name.name, traits)
        };
        if !kt_items.is_empty()
            && let Some(c) = self.choose_quiet(&kt_items, args)
        {
            let item = c.item.expect("an index item");
            let t = proc_macro2::Ident::new(item.via, nspan);
            let elem = recv.ty.element();
            let recv_ty = recv.ty.clone();
            let r = match (item.receiver, recv.kind) {
                (Receiver::Value, _) => {
                    let v = self.finish(recv, Want::default(), loc);
                    quote!(#v)
                }
                // (a `&mut` held: reborrowed)
                (Receiver::RefMut, OutKind::RefMut) => {
                    let ts = recv.ts;
                    quote_spanned!(span=> &mut *#ts)
                }
                (_, OutKind::RefMut) => {
                    let ts = recv.ts;
                    quote_spanned!(span=> &*#ts)
                }
                (Receiver::RefMut, _) => {
                    let ts = recv.ts;
                    quote_spanned!(span=> &mut #ts)
                }
                _ => {
                    let ts = recv.ts;
                    quote_spanned!(span=> &#ts)
                }
            };
            let mut c = c;
            c.ret = kt_ret(&c.ret, item.ret, &elem, &recv_ty);
            self.elem_hint = Some((elem, recv_ty));
            let out = self.emit(&c, Target::Ufcs(quote_spanned!(span=> #kt::#t), r), args, loc, &name.name);
            self.elem_hint = None;
            return out;
        }
        // A member extension of an implicit receiver (`placeable.place(x, y)` in a PlacementScope; a generic one,
        // `1.3f at 100` in a `keyframes { }` block, the config's `at(value: T, …)`, extends any value).
        let scoped = self.receivers.iter().rev().find_map(|r| {
            let items: Vec<&'static crate::index::Item> =
                members_of(&r.ty, &name.name).filter(|i| i.kind == Kind::Method && !i.params.is_empty()).collect();
            let fits = items.iter().any(|i| {
                let first = Ty::from_rust(i.params[0].ty);
                recv.ty == Ty::Unknown
                    || first == recv.ty
                    || first.is_float() && recv.ty.is_number()
                    || first == Ty::Unknown && is_type_parameter(i.params[0].ty)
                    || first.owner().is_some() && first.owner() == recv.ty.owner()
            });
            (fits && owner_has_no(&recv.ty, &name.name)).then(|| (r.clone(), items))
        });
        if let Some((r, items)) = scoped {
            let label = r.labels.first().cloned();
            let _ = self.receiver(label.as_deref());
            let first = items[0].params[0].ty;
            let first_param_ref = first.starts_with('&');
            let rv = recv.ts;
            // (Kotlin's `Int.toDp()` and `Float.toDp()` are one Rust method taking a float: the number given as one)
            let rv = if matches!(first, "f32" | "f64") && !recv.ty.is_float() {
                let t = proc_macro2::Ident::new(first, span);
                quote_spanned!(span=> (#rv as #t))
            } else {
                rv
            };
            let rv = if first_param_ref { quote_spanned!(span=> &#rv) } else { rv };
            let ri = proc_macro2::Ident::new(&r.rust, span);
            let c = Callee::from_item(items[0]);
            let rest = Callee { params: c.params[1..].to_vec(), ..c };
            return self.emit(&rest, Target::MethodFirst(quote!(#ri), rv), args, loc, &name.name);
        }
        // Any method of the name the index knows (the receiver's type unknown).
        let items: Vec<&'static crate::index::Item> = lookup(&name.name)
            .iter()
            .filter(|i| {
                i.kind == Kind::Method && i.via.is_empty() || i.kind == Kind::Method && !i.via.starts_with("Kt")
            })
            .collect();
        if !items.is_empty()
            && let Some(c) = self.choose_quiet(&items, args)
        {
            let kind = c.item.map_or(Receiver::Ref, |i| i.receiver);
            let r = self.finish_recv(recv, kind, loc);
            return self.emit(&c, Target::Method(r), args, loc, &name.name);
        }
        // Unknown: as written.
        let r = recv.ts;
        let m = self.name_ident(&snake(&name.name), name.loc);
        let mut vals = Vec::new();
        for a in args {
            let v = self.loose_arg(a);
            vals.push(match a.name {
                Some(an) => {
                    let ai = ident(&snake(&an.name), self.span(an.loc));
                    quote_spanned!(span=> #ai = #v)
                }
                None => v,
            });
        }
        Out::plain(quote_spanned!(span=> #r.#m(#(#vals),*)), Ty::Unknown)
    }

    /// A receiver for a method taking it as `kind`: a clone of a local taken by value.
    fn finish_recv(&self, recv: Out, kind: Receiver, loc: Loc) -> TokenStream {
        match (kind, recv.kind) {
            (Receiver::Value, OutKind::Local | OutKind::RefMut) => self.finish(recv, Want::default(), loc),
            _ => recv.ts,
        }
    }

    // ---- Choosing an overload ----------------------------------------------------------------------------------

    /// The item of `items` the arguments fit (an error saying so when none does).
    fn choose(&mut self, items: &[&'static crate::index::Item], args: &[ArgRef], loc: Loc) -> Option<Callee> {
        match self.choose_quiet(items, args) {
            Some(c) => Some(c),
            None => {
                let name = items.first().map(|i| i.kotlin).unwrap_or("it");
                let names: Vec<String> = args.iter().filter_map(|a| a.name.map(|n| format!("`{}`", n.name))).collect();
                let detail = if names.is_empty() { String::new() } else { format!(" (named: {})", names.join(", ")) };
                self.error(loc, format!("no `{name}` takes {} argument(s) like these{detail}", args.len()));
                None
            }
        }
    }

    fn choose_quiet(&self, items: &[&'static crate::index::Item], args: &[ArgRef]) -> Option<Callee> {
        let mut best: Option<(i32, Callee)> = None;
        for item in items {
            let c = Callee::from_item(item);
            let Some(score) = self.fit(&c, args) else { continue };
            if best.as_ref().is_none_or(|(s, _)| score > *s) {
                best = Some((score, c));
            }
        }
        best.map(|(_, c)| c)
    }

    /// How well arguments fit a callee (none: they don't).
    fn fit(&self, c: &Callee, args: &[ArgRef]) -> Option<i32> {
        // (the named arguments of the builder it gives: `detectTapGestures(onTap = { … })`)
        let (args, links) = split_chain(c, args)?;
        if !links.is_empty() && c.params.is_empty() {
            return Some(0);
        }
        let args = args.as_slice();
        let m = self.map_args(c, args).ok()?;
        let mut score = 0;
        for (k, p) in c.params.iter().enumerate() {
            let given = &m.given[k];
            if given.is_empty() {
                // (a vararg given nothing is an empty array)
                if p.default == PDefault::None && !p.vararg {
                    return None;
                }
                score -= 1;
                continue;
            }
            for a in given {
                let lambda_param = p.lambda.is_some()
                    || p.ty.contains("Fn(")
                    || p.ty.contains("Fn<")
                    || p.ty.trim_start().starts_with("fn(");
                if a.value.is_lambda() != lambda_param && !p.vararg {
                    // (a lambda to a non-function parameter, or the reverse: a function value is one, and any value
                    // can be a fun interface's — `easing = LinearEasing` to an `impl Easing`)
                    let fun_interface = p.lambda.is_some()
                        && !p.ty.contains("Fn(")
                        && !p.ty.contains("Fn<")
                        && !p.ty.trim_start().starts_with("fn(");
                    let value_fits = matches!(a.value, ArgValue::Expr(e)
                        if fun_interface || matches!(self.peek_ty(e), Ty::Unknown | Ty::Fun(_)));
                    if a.value.is_lambda() || !value_fits {
                        return None;
                    }
                }
                // (a type that doesn't fit counts against it: what is known of types is partial; one that is the
                // parameter's own type fits best — Kotlin's most specific overload: `rememberTransition` of a
                // `DeferredTransitionState`)
                if let ArgValue::Expr(e) = a.value
                    && !takes_any(&p.ty)
                {
                    let ty = self.peek_ty(e);
                    if !compatible(&ty, &p.ty()) {
                        score -= 5;
                    } else if matches!((&ty, &p.ty()), (Ty::Named(x, _), Ty::Named(y, _)) if x == y) {
                        score += 1;
                    }
                }
                if let (Some(l), Some(info)) = (a.value.lambda(), &p.lambda) {
                    let n = l.params.as_ref().map_or(usize::MAX, |ps| ps.len());
                    if n != usize::MAX && n != info.arity() {
                        return None;
                    }
                }
            }
            score += 2;
        }
        Some(score - c.params.len() as i32)
    }

    /// Which parameter each argument goes to.
    fn map_args<'e>(&self, c: &Callee, args: &[ArgRef<'e>]) -> Result<Mapping<'e>, String> {
        let n = c.params.len();
        let mut given: Vec<Vec<ArgRef>> = vec![Vec::new(); n];
        let mut order = Vec::new();
        let mut next = 0;
        let last = args.len().checked_sub(1);
        for (k, a) in args.iter().enumerate() {
            let trailing = matches!(a.value, ArgValue::Lambda(_)) && Some(k) == last;
            let at = if let Some(name) = a.name {
                c.params
                    .iter()
                    .position(|p| p.kotlin == name.name)
                    .ok_or_else(|| format!("no parameter `{}`", name.name))?
            } else if trailing {
                n.checked_sub(1).ok_or("no parameter for the lambda")?
            } else {
                // (keys gathered: `LaunchedEffect(a, b) { }`)
                if next < n && is_keys(&c.params[next]) && !given[next].is_empty() {
                    given[next].push(*a);
                    continue;
                }
                while next < n && !given[next].is_empty() && !c.params[next].vararg {
                    next += 1;
                }
                if next >= n {
                    return Err("too many arguments".into());
                }
                let at = next;
                if !c.params[at].vararg && !is_keys(&c.params[at]) {
                    next += 1;
                }
                at
            };
            if !given[at].is_empty() && !c.params[at].vararg && !is_keys(&c.params[at]) {
                return Err(format!("`{}` given twice", c.params[at].kotlin));
            }
            given[at].push(*a);
            order.push((at, a.name.is_some(), trailing));
        }
        Ok(Mapping { given, order })
    }

    // ---- Writing the call --------------------------------------------------------------------------------------

    /// The call of `c` through `target` with `args` (a generic one's element type `T` bound from the collection it is
    /// given, for its lambdas: [`Gen::type_arg`]).
    fn emit(&mut self, c: &Callee, target: Target, args: &[ArgRef], loc: Loc, kotlin: &str) -> Out {
        let bound = self.map_args(c, args).ok().and_then(|m| self.collection_type_arg(c, &m));
        let saved = std::mem::replace(&mut self.type_arg, bound);
        let out = self.emit_bound(c, target, args, loc, kotlin);
        self.type_arg = saved;
        out
    }

    /// The element type of the collection a generic callee's `T` comes from (`items: impl LazyListItems<T>`, `Vec<T>`,
    /// `&[T]`, `impl IntoIterator<Item = T>`), as the argument given to it is known.
    fn collection_type_arg(&self, c: &Callee, m: &Mapping) -> Option<Ty> {
        c.params.iter().enumerate().find_map(|(k, p)| {
            let t = plain_type_text(&p.ty);
            let collection_of_t =
                t.contains("<T>") || t.contains("=T>") || t.starts_with("&[T]") || t.starts_with("[T]");
            if !collection_of_t || m.given[k].len() != 1 {
                return None;
            }
            match m.given[k][0].value {
                ArgValue::Expr(e) => Some(self.peek_ty(e).element()).filter(|t| *t != Ty::Unknown),
                ArgValue::Lambda(_) | ArgValue::Sam { .. } => None,
            }
        })
    }

    fn emit_bound(&mut self, c: &Callee, target: Target, args: &[ArgRef], loc: Loc, kotlin: &str) -> Out {
        let span = self.span(loc);
        if let Some((own, links)) = split_chain(c, args)
            && !links.is_empty()
        {
            return self.chain_call(c, target, &own, &links, loc, kotlin);
        }
        let m = match self.map_args(c, args) {
            Ok(m) => m,
            Err(e) => {
                self.error(loc, format!("`{kotlin}`: {e}"));
                return Out::plain(quote_spanned!(span=> ::core::unreachable!()), Ty::Unknown);
            }
        };
        // Through the builder unless the call gives what its plain form takes (the parameters without a default, in
        // order, none named out of order).
        let required: Vec<usize> = (0..c.params.len()).filter(|k| c.params[*k].default == PDefault::None).collect();
        let given_set: Vec<usize> = (0..c.params.len()).filter(|k| !m.given[*k].is_empty()).collect();
        let in_order = m.order.windows(2).all(|w| w[0].0 < w[1].0);
        let pascal_path = matches!(&target, Target::Path(_, name) if is_pascal(name));
        let builder = c.builder && !(given_set == required && in_order);
        let missing_kotlin =
            c.params.iter().enumerate().any(|(k, p)| m.given[k].is_empty() && matches!(p.default, PDefault::Kotlin(_)));
        let rewritten = (builder && !(missing_kotlin && !c.builder)) || (pascal_path && self.fun.kompozable);
        let transform_wraps = rewritten && self.fun.kompozable;
        let call_label = kotlin.to_string();
        // Each argument: its name when named, its value, whether it is a lambda.
        let mut vals: Vec<(Option<proc_macro2::Ident>, TokenStream, bool)> = Vec::new();
        if builder && c.builder {
            for (k, p) in c.params.iter().enumerate() {
                if m.given[k].is_empty() && p.default == PDefault::None && !p.vararg {
                    self.error(loc, format!("`{kotlin}` needs `{}`", p.kotlin));
                }
            }
            // In the order written; named ones by their Rust names (a positional one after a named one too: the
            // builder counts positions among the positional ones).
            let mut named_any = false;
            for (written, a) in args.iter().enumerate() {
                let (at, named, _) = m.order[written.min(m.order.len() - 1)];
                let p = &c.params[at];
                if m.given[at].len() > 1 && m.given[at][0].loc != a.loc {
                    continue;
                }
                let v = if m.given[at].len() > 1 {
                    let all: Vec<&ArgRef> = m.given[at].iter().collect();
                    self.many_value(&all, p, span)
                } else {
                    self.with_label(&call_label, |me| me.arg_value(a, Some(p), transform_wraps))
                };
                let lambda = a.value.is_lambda();
                let last_param = at + 1 == c.params.len();
                let position_ok = !named && !named_any && at == written && !(lambda && !last_param);
                let future = lambda && p.lambda.as_ref().is_some_and(|l| l.kind == crate::index::LambdaKind::Future);
                if position_ok && !future {
                    vals.push((None, v, lambda));
                } else {
                    vals.push((Some(ident(&p.rust, self.span(a.loc))), v, lambda));
                    named_any = true;
                }
            }
            // (make sure the macro rewrites it: a lowercase call needs a named argument)
            if !named_any
                && !pascal_path
                && let Some(last) = vals.last_mut()
            {
                let at = m.order.last().map(|o| o.0).unwrap_or(0);
                last.0 = Some(ident(&c.params[at].rust, span));
            }
        } else {
            for (k, p) in c.params.iter().enumerate() {
                if m.given[k].is_empty() {
                    match &p.default {
                        PDefault::Kotlin(text) => vals.push((None, self.kotlin_default(text, p, loc), false)),
                        PDefault::Builder => {}
                        PDefault::None if p.vararg => vals.push((None, self.many_value(&[], p, span), false)),
                        PDefault::None => {
                            self.error(loc, format!("`{kotlin}` needs `{}`", p.kotlin));
                        }
                    }
                    continue;
                }
                let v = if m.given[k].len() > 1 || p.vararg || is_keys(p) && m.given[k].len() != 1 {
                    let all: Vec<&ArgRef> = m.given[k].iter().collect();
                    self.many_value(&all, p, span)
                } else {
                    let a = m.given[k][0];
                    self.with_label(&call_label, |me| me.arg_value(&a, Some(p), transform_wraps))
                };
                vals.push((None, v, m.given[k].iter().any(|a| a.value.is_lambda())));
            }
        }
        // A method of a scope taken `&mut` whose arguments read the scope (`drawRect(…, size = Size(size.width, …))`):
        // the arguments first, in order (as Kotlin evaluates them), then the call, which borrows the scope.
        let mut hoisted = TokenStream::new();
        let scope_mut = c.item.is_some_and(|i| i.receiver == Receiver::RefMut);
        if let Target::Method(r) | Target::MethodFirst(r, _) = &target
            && scope_mut
        {
            let recv = r.to_string();
            let reads = |v: &TokenStream| v.clone().into_iter().any(|t| mentions(&t, &recv));
            if vals.iter().any(|(_, v, lambda)| !lambda && reads(v)) {
                for (_, v, lambda) in vals.iter_mut() {
                    if *lambda {
                        continue;
                    }
                    let t = self.fresh("a", span);
                    hoisted.extend(quote_spanned!(span=> let #t = #v;));
                    *v = quote!(#t);
                }
            }
        }
        let vals: Vec<TokenStream> = vals
            .into_iter()
            .map(|(n, v, _)| match n {
                Some(n) => quote_spanned!(span=> #n = #v),
                None => v,
            })
            .collect();
        let await_ = if c.suspend {
            if !self.in_async {
                self.error(
                    loc,
                    format!(
                        "`{kotlin}` suspends: call it from a coroutine (`LaunchedEffect`, `launch`, a `suspend fun`)"
                    ),
                );
            }
            quote_spanned!(span=> .await)
        } else {
            quote!()
        };
        let ts = match target {
            Target::Path(p, _) => quote_spanned!(span=> #p(#(#vals),*) #await_),
            Target::Method(r) => {
                let name = ident(&c.rust, span);
                quote_spanned!(span=> #r.#name(#(#vals),*) #await_)
            }
            Target::Ufcs(t, r) => {
                let name = ident(&c.rust, span);
                quote_spanned!(span=> #t::#name(#r #(, #vals)*) #await_)
            }
            Target::MethodFirst(r, first) => {
                let name = ident(&c.rust, span);
                quote_spanned!(span=> #r.#name(#first #(, #vals)*) #await_)
            }
        };
        let ts = if hoisted.is_empty() { ts } else { quote_spanned!(span=> { #hoisted #ts }) };
        let ts = self.identified(ts, loc, keeps_state(c));
        let ret = if c.kompozable && c.ret == Ty::Unknown { Ty::Unit } else { c.ret.clone() };
        Out::plain(ts, ret)
    }

    /// A call of what gives a builder of the rest of its arguments by name (`detectTapGestures(onTap = { … })` →
    /// `detect_tap_gestures().on_tap(…)`, `detectDragGestures(onDragStart = { … }, onDrag = { … })` →
    /// `detect_drag_gestures(…).on_drag_start(…)`, awaited in a coroutine: Kotlin's suspending detectors): the call
    /// with its own arguments (`own`), then the builder's methods with theirs (`links`).
    fn chain_call(
        &mut self,
        c: &Callee,
        target: Target,
        own: &[ArgRef],
        links: &[(ArgRef, &'static crate::index::Item)],
        loc: Loc,
        kotlin: &str,
    ) -> Out {
        let span = self.span(loc);
        let name = ident(&c.rust, span);
        let mut ts = match target {
            _ if !c.params.is_empty() => self.emit(c, target, own, loc, kotlin).ts,
            Target::Path(p, _) => quote_spanned!(span=> #p()),
            Target::Method(r) => quote_spanned!(span=> #r.#name()),
            Target::Ufcs(t, r) => quote_spanned!(span=> #t::#name(#r)),
            Target::MethodFirst(r, first) => quote_spanned!(span=> #r.#name(#first)),
        };
        for &(a, m) in links {
            let p = Callee::from_item(m).params.remove(0);
            let label = a.name.map(|n| n.name.clone()).unwrap_or_default();
            let v = self.with_label(&label, |me| me.arg_value(&a, Some(&p), false));
            let mi = ident(m.rust, self.span(a.loc));
            ts = quote_spanned!(span=> #ts.#mi(#v));
        }
        if self.in_async {
            ts = quote_spanned!(span=> #ts.await);
        }
        Out::plain(ts, Ty::Unit)
    }

    /// Writes `f` with the call's name as the label its lambdas have (`return@forEach`, `this@Canvas`).
    fn with_label<T>(&mut self, label: &str, f: impl FnOnce(&mut Self) -> T) -> T {
        self.call_labels.push(label.to_string());
        let out = f(self);
        self.call_labels.pop();
        out
    }

    /// Several arguments to one parameter: a vararg's array (`[a, b]`, `&[a, b]` for a slice), keys' tuple.
    fn many_value(&mut self, all: &[&ArgRef], p: &PInfo, span: Span) -> TokenStream {
        if is_keys(p) {
            return self.keys_tuple(all, span);
        }
        let el = PInfo { ty: element_ty(&p.ty), vararg: false, ..p.clone() };
        let vals: Vec<TokenStream> = all.iter().map(|a| self.arg_value(a, Some(&el), false)).collect();
        if p.ty.trim_start().starts_with("&[") {
            quote_spanned!(span=> &[#(#vals),*])
        } else {
            quote_spanned!(span=> [#(#vals),*])
        }
    }

    /// An argument's value for a parameter `p` (none known: as it is).
    fn arg_value(&mut self, a: &ArgRef, p: Option<&PInfo>, transform_wraps: bool) -> TokenStream {
        let span = self.span(a.loc);
        let Some(p) = p else { return self.loose_arg(a) };
        // (a nullable parameter whose default isn't null — `overscrollEffect: OverscrollEffect? =
        // rememberOverscrollEffect()` — is Rust's `Option<Option<T>>`, the outer `None` its default: what is given,
        // `null` too, in `Some`)
        if let Some(inner) = p.ty.trim().strip_prefix("Option<").and_then(|r| r.strip_suffix('>'))
            && inner.trim_start().starts_with("Option<")
        {
            let ip = PInfo { ty: inner.trim().to_string(), option: OptionForm::Option, ..p.clone() };
            let v = self.arg_value(a, Some(&ip), transform_wraps);
            return quote_spanned!(span=> ::core::option::Option::Some(#v));
        }
        // (one value to a slice: an array of it, unless it is a list already — `floatArrayOf(8f, 6f)`)
        let a_list = matches!(a.value, ArgValue::Expr(e) if matches!(self.peek_ty(e), Ty::List(_)));
        if p.vararg || (p.ty.trim_start().starts_with("&[") && !a.value.is_lambda() && !a_list) {
            return self.many_value(&[a], p, span);
        }
        if is_keys(p) {
            return self.keys_tuple(&[a], span);
        }
        // (a SAM constructor to what takes no lambda: the interface's value it makes)
        if let ArgValue::Sam { expr, .. } = a.value
            && p.lambda.is_none()
        {
            return self.arg_value(&ArgRef { value: ArgValue::Expr(expr), ..*a }, Some(p), transform_wraps);
        }
        if let Some(l) = a.value.lambda() {
            let label = self.call_labels.last().cloned();
            let closure = self.lambda_for(l, p.lambda.as_ref(), label.as_deref(), transform_wraps);
            let t = plain_type_text(&p.ty);
            // (a shared or boxed closure, `Rc<dyn Fn()>` — `Callback` —, in an `Option` or not)
            let (some, owned) = match t.strip_prefix("Option<") {
                Some(inner) => (true, inner),
                None => (false, t.as_str()),
            };
            let made = if owned.starts_with("Rc<dyn") {
                Some(quote_spanned!(span=> ::std::rc::Rc::new(#closure)))
            } else if owned.starts_with("Arc<dyn") {
                Some(quote_spanned!(span=> ::std::sync::Arc::new(#closure)))
            } else if owned.starts_with("Box<dyn") {
                Some(quote_spanned!(span=> ::std::boxed::Box::new(#closure)))
            } else {
                None
            };
            if let Some(made) = made {
                // (made the trait object its parameter takes, which a builder's generic setter wouldn't coerce it to)
                let ty = p.ty.trim();
                let ty = ty.strip_prefix("Option<").and_then(|t| t.strip_suffix('>')).unwrap_or(ty);
                let made = match super::lambda::written_type(&self.bind_type_arg(ty, span), span) {
                    Some(t) => {
                        let f = self.fresh("f", span);
                        quote_spanned!(span=> { let #f: #t = #made; #f })
                    }
                    None => made,
                };
                return if some { quote_spanned!(span=> ::core::option::Option::Some(#made)) } else { made };
            }
            return if t.starts_with("Option<&") {
                quote_spanned!(span=> ::core::option::Option::Some(&#closure))
            } else if t.starts_with("Option<") {
                quote_spanned!(span=> ::core::option::Option::Some(#closure))
            } else if t.starts_with('&') {
                quote_spanned!(span=> &#closure)
            } else {
                closure
            };
        }
        let ArgValue::Expr(e) = a.value else { unreachable!() };
        // (a kept closure parameter passed on through a rewritten call: as it is, the macro shares it with its identity)
        if transform_wraps
            && let ExprKind::Name(n) = &e.kind
            && self.scopes.iter().rev().find_map(|s| s.get(&n.name)).is_some_and(|l| l.kept_closure)
        {
            return self.expr(e).ts;
        }
        let pty = p.ty();
        if pty.is_float()
            && let Some(n) = int_literal(e)
        {
            let neg = matches!(e.kind, ExprKind::Prefix { op: PrefixOp::Minus, .. });
            let v = self.number(n, e.loc, true).ts;
            return if neg { quote_spanned!(span=> -#v) } else { v };
        }
        // (Kotlin's `Any` — a key, a content type —: the value made the Rust type holding it, in `Some` for an optional
        // one; each branch of an `if`/`when`)
        if let Some(w) = AnyWrap::of_rust(&p.ty) {
            return self.value_want(e, Want { str_ok: false, any: Some(w), some: false, shared: None });
        }
        let o = self.expr(e);
        let nullable_arg = o.kind == OutKind::Null || o.ty.is_nullable();
        let arg_ty = o.ty.clone();
        let v = self.finish(o, Want { str_ok: p.takes_str(), any: None, some: false, shared: None }, e.loc);
        let v = self.kotlin_number(v, &arg_ty, &pty, span);
        let t = plain_type_text(&p.ty);
        let t = t.as_str();
        // (a value of another type than the parameter's, which converts from it: Kotlin's subtype, a Rust `From` —
        // a `Stroke` given as a `DrawStyle`; a type converts to itself, so a wrong guess of the argument's type costs
        // nothing)
        let inner = t.strip_prefix("Option<").and_then(|r| r.strip_suffix('>'));
        let target = inner.unwrap_or(t);
        // (of plain types both: a generic one's arguments aren't known here, `Arrangement<A>`)
        let plain = |x: &Ty| matches!(x, Ty::Named(_, a) if a.is_empty());
        let converts = |a: &Ty, b: &Ty| plain(a) && plain(b) && a != b && plain_path(target) && !takes_any(target);
        let v = match (&arg_ty.non_null(), Ty::from_rust(target)) {
            // (in an `Option`, named: a builder's setter takes anything `Into<Option<T>>`)
            (a, b) if converts(a, &b) && inner.is_some() => {
                let ty: TokenStream = respan(target.parse().expect("a type's path"), span);
                quote_spanned!(span=> ::core::convert::Into::<#ty>::into(#v))
            }
            (a, b) if converts(a, &b) => quote_spanned!(span=> ::core::convert::Into::into(#v)),
            _ => v,
        };
        // (a shared trait object of another trait, `Rc<dyn TargetedFlingBehavior>` given as an `Rc<dyn FlingBehavior>`:
        // Kotlin's subinterface value, upcast where its type is written, which a builder's generic setter wouldn't do)
        let v = match (&arg_ty, Ty::from_rust(target)) {
            (Ty::Named(ptr, a), Ty::Named(want, b))
                if matches!(ptr.as_str(), "Rc" | "Arc") && *ptr == want && !a.is_empty() && *a != b =>
            {
                let raw = p.ty.trim();
                let raw = raw.strip_prefix("Option<").and_then(|r| r.strip_suffix('>')).unwrap_or(raw);
                match super::lambda::written_type(raw, span) {
                    Some(ty) => {
                        let f = self.fresh("f", span);
                        quote_spanned!(span=> { let #f: #ty = #v; #f })
                    }
                    None => v,
                }
            }
            _ => v,
        };
        // (a shared trait object in an `Option`, `placementSpec: Option<Arc<dyn FiniteAnimationSpec<IntOffset>>>`,
        // given a value of its own type, `tween(200)`: `Arc::new` of it, made the trait object where its type is
        // written, which a builder's generic setter wouldn't do)
        let v = match &arg_ty {
            Ty::Named(name, _)
                if inner.is_some()
                    && !nullable_arg
                    && !matches!(name.as_str(), "Arc" | "Rc")
                    && trait_object(name).is_none()
                    && (target.starts_with("Arc<dyn") || target.starts_with("Rc<dyn")) =>
            {
                let raw = p.ty.trim();
                let raw = raw.strip_prefix("Option<").and_then(|r| r.strip_suffix('>')).unwrap_or(raw);
                let made = if target.starts_with("Arc<dyn") {
                    quote_spanned!(span=> ::std::sync::Arc::new(#v))
                } else {
                    quote_spanned!(span=> ::std::rc::Rc::new(#v))
                };
                match super::lambda::written_type(raw, span) {
                    Some(ty) => {
                        let f = self.fresh("f", span);
                        quote_spanned!(span=> { let #f: #ty = #made; #f })
                    }
                    None => made,
                }
            }
            _ => v,
        };
        if p.option == OptionForm::Option && !nullable_arg {
            return quote_spanned!(span=> ::core::option::Option::Some(#v));
        }
        // (a shared trait object, `shape: Arc<dyn Shape>`, given a value of its own type, `RoundedCornerShape(16.dp)`:
        // Kotlin's interface value is the Rust side's `Arc::new` of it; a value of the interface's type is one already)
        if let Ty::Named(name, _) = &arg_ty
            && !matches!(name.as_str(), "Arc" | "Rc")
            && trait_object(name).is_none()
        {
            if t.starts_with("Arc<dyn") {
                return quote_spanned!(span=> ::std::sync::Arc::new(#v));
            }
            if t.starts_with("Rc<dyn") {
                return quote_spanned!(span=> ::std::rc::Rc::new(#v));
            }
        }
        if t.starts_with("&mut") {
            return quote_spanned!(span=> &mut #v);
        }
        if t.starts_with('&') && !t.starts_with("&str") && !t.starts_with("&'static str") {
            return quote_spanned!(span=> &#v);
        }
        v
    }

    /// A value of another of Rust's integer types (`usize`: a layout's `lineCount`) as the Kotlin number `to` is.
    fn kotlin_number(&self, v: TokenStream, from: &Ty, to: &Ty, span: Span) -> TokenStream {
        if *from == Ty::OtherInt && to.is_number() && *to != Ty::OtherInt {
            let t = self.rust_of_ty(to, span);
            quote_spanned!(span=> (#v as #t))
        } else {
            v
        }
    }

    /// An expression's value for a parameter `p` (a default's).
    pub(crate) fn value_for(&mut self, e: &Expr, p: Option<&PInfo>) -> TokenStream {
        let a = ArgRef { name: None, value: ArgValue::Expr(e), spread: false, loc: e.loc };
        self.arg_value(&a, p, false)
    }

    /// An argument whose parameter isn't known: as it is (a lambda borrowing).
    fn loose_arg(&mut self, a: &ArgRef) -> TokenStream {
        match a.value {
            ArgValue::Lambda(l) => {
                let label = self.call_labels.last().cloned();
                self.lambda_for(l, None, label.as_deref(), false)
            }
            ArgValue::Sam { expr, .. } => {
                self.value_want(expr, Want { str_ok: true, any: None, some: false, shared: None })
            }
            ArgValue::Expr(e) => match &e.kind {
                ExprKind::Lambda(l) => self.lambda_for(l, None, None, false),
                _ => self.value_want(e, Want { str_ok: true, any: None, some: false, shared: None }),
            },
        }
    }

    /// A parameter's Kotlin default (the index's doc line's), translated where the call leaves it out.
    fn kotlin_default(&mut self, text: &str, p: &PInfo, loc: Loc) -> TokenStream {
        let span = self.span(loc);
        let mut diags = crate::diag::Diagnostics::default();
        let source = format!("fun __kz_default() = {text}");
        let tokens = crate::lexer::lex(&source, crate::pos::Pos::start(1, 1), &mut diags);
        let file = crate::parser::parse(&tokens, &mut diags);
        let expr = file.decls.first().and_then(|d| match d {
            Decl::Function(f) => match &f.body {
                Some(FunctionBody::Expr(e)) => Some(e.clone()),
                _ => None,
            },
            _ => None,
        });
        let Some(expr) = expr.filter(|_| !diags.has_errors()) else {
            self.error(loc, format!("the default of `{}` (`{text}`) isn't Kotlin KompozR reads: give it", p.kotlin));
            return quote!();
        };
        // (its positions are the default's own text's: its tokens get the call's span)
        let spans = crate::pos::OneSpan(span);
        let cx = super::Context { spans: &spans, paths: self.cx.paths, keyed_calls: false };
        let mut inner_diags = crate::diag::Diagnostics::default();
        let mut g = Gen::new(&cx, &mut inner_diags);
        g.scopes = vec![Default::default()];
        g.fun = self.fun.clone();
        let a = ArgRef { name: None, value: ArgValue::Expr(&expr), spread: false, loc: expr.loc };
        let v = g.arg_value(&a, Some(p), false);
        for d in inner_diags.list {
            self.error(loc, format!("in the default of `{}`: {}", p.kotlin, d.message));
        }
        respan(v, span)
    }
}

/// A call's arguments split: those its parameters take, and the named ones its result's builder takes as its methods
/// of one parameter (`detectDragGestures(onDragStart = …)`: `DetectDragGestures::on_drag_start`); none when a named
/// argument is neither's.
fn split_chain<'e>(
    c: &Callee,
    args: &[ArgRef<'e>],
) -> Option<(Vec<ArgRef<'e>>, Vec<(ArgRef<'e>, &'static crate::index::Item)>)> {
    let mut own = Vec::new();
    let mut links = Vec::new();
    let owner = c.ret.owner();
    for a in args {
        match (a.name, owner) {
            (Some(n), Some(owner)) if !c.params.iter().any(|p| p.kotlin == n.name) => {
                let m = members_of(owner, &n.name).find(|i| i.params.len() == 1)?;
                links.push((*a, m));
            }
            _ => own.push(*a),
        }
    }
    Some((own, links))
}

/// Whether a type's text is a plain path (`DrawStyle`, `kt::KtSet`): no generics, references or bounds.
fn plain_path(t: &str) -> bool {
    !t.is_empty()
        && t.chars().all(|c| c.is_alphanumeric() || c == '_' || c == ':')
        && t.starts_with(|c: char| c.is_alphabetic())
        && !matches!(t, "Self" | "T" | "R" | "K" | "V")
}

/// Whether a token is, or holds, the identifier `name`.
fn mentions(t: &proc_macro2::TokenTree, name: &str) -> bool {
    match t {
        proc_macro2::TokenTree::Ident(i) => *i == name,
        proc_macro2::TokenTree::Group(g) => g.stream().into_iter().any(|t| mentions(&t, name)),
        _ => false,
    }
}

/// Whether a call keeps something in the composition (and so needs an identity of its own): a composable, a
/// `remember…`, the state readers that remember (`collectAsState`, `produceState`).
fn keeps_state(c: &Callee) -> bool {
    c.kompozable || c.rust.starts_with("remember") || matches!(c.rust.as_str(), "collect_as_state" | "produce_state")
}

/// Whether a parameter takes keys (`keys: K`): several arguments are one tuple.
fn is_keys(p: &PInfo) -> bool {
    matches!(p.rust.as_str(), "keys" | "inputs" | "key1") && p.ty.len() <= 2
}

/// The element type of a vararg's or slice's type (`&[ProvidedValue]` → `ProvidedValue`).
fn element_ty(t: &str) -> String {
    let t = t.trim();
    let inner = t.trim_start_matches('&').trim_start_matches('[');
    inner.split(';').next().unwrap_or(inner).trim_end_matches(']').trim().to_string()
}

/// Whether a parameter's type is a trait's (`impl DelayTime`, `impl Into<Brush>`): what it takes isn't told here.
fn takes_any(t: &str) -> bool {
    let t = plain_type_text(t);
    // (a type parameter: `T`, `R`, `T2` — not a short type's name, `Dp`)
    let type_parameter =
        !t.is_empty() && t.len() <= 2 && t.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    (t.starts_with("impl") && !t.starts_with("implInto<String>") && !t.starts_with("implFn")) || type_parameter
}

/// Whether an argument of type `a` may go to a parameter of type `p` (as far as is known).
fn compatible(a: &Ty, p: &Ty) -> bool {
    match (a, p) {
        (Ty::Unknown, _) | (_, Ty::Unknown) => true,
        (Ty::Nullable(_), Ty::Nullable(_)) => true,
        (a, Ty::Nullable(p)) => compatible(a, p),
        (Ty::Int | Ty::Long | Ty::OtherInt, p) => p.is_number(),
        (Ty::Float | Ty::Double, p) => p.is_float(),
        (Ty::Named(x, _), Ty::Named(y, _)) => x == y,
        (Ty::Named(_, _), p) => matches!(p, Ty::Named(_, _) | Ty::Fun(_)),
        (Ty::String, p) => matches!(p, Ty::String | Ty::Named(_, _)),
        (Ty::Bool, p) => *p == Ty::Bool,
        (Ty::List(_), p) => matches!(p, Ty::List(_) | Ty::Named(_, _)),
        _ => true,
    }
}

/// An integer literal (`2`, `-2`).
fn int_literal(e: &Expr) -> Option<&Number> {
    match &e.kind {
        ExprKind::Number(n) if n.kind == NumberKind::Int && n.radix == 10 => Some(n),
        ExprKind::Prefix { op: PrefixOp::Minus, operand } => int_literal(operand),
        _ => None,
    }
}

/// What a kt shim's method gives, its `Self::Item`s made the receiver's element type.
fn kt_ret(c: &Ty, text: &str, elem: &Ty, recv: &Ty) -> Ty {
    match text {
        "Self::Item" | "T" => elem.clone(),
        "Option<Self::Item>" | "Option<T>" => elem.clone().nullable(),
        "Vec<Self::Item>" | "Vec<T>" => Ty::List(Box::new(elem.clone())),
        "Self" => recv.clone(),
        t if t.starts_with("Vec<IndexedValue<") => {
            Ty::List(Box::new(Ty::Named("IndexedValue".into(), vec![elem.clone()])))
        }
        _ => c.clone(),
    }
}

/// Kotlin members a Rust type the generator meets may have a method of its own named so: called through the kt
/// shims' trait only when the receiver's type is known.
fn ambiguous_member(name: &str) -> bool {
    matches!(
        name,
        "first"
            | "last"
            | "get"
            | "contains"
            | "remove"
            | "split"
            | "replace"
            | "trim"
            | "find"
            | "repeat"
            | "map"
            | "filter"
            | "forEach"
            | "isEmpty"
            | "size"
            | "count"
            | "sum"
            | "any"
            | "all"
            | "joinToString"
            | "take"
            | "drop"
            | "reversed"
            | "sorted"
            | "max"
            | "min"
            | "add"
            | "indexOf"
            | "lines"
            | "startsWith"
            | "endsWith"
            | "length"
            | "toInt"
            | "toFloat"
    )
}

/// Whether a value of type `t` has no method `name` of its own (a member extension of a scope may be meant).
fn owner_has_no(t: &Ty, name: &str) -> bool {
    t.owner().is_none_or(|o| members_of(o, name).next().is_none())
}

/// Whether a parameter's Rust type is a type parameter (`T`): what a generic member extension extends.
pub(crate) fn is_type_parameter(ty: &str) -> bool {
    let t = ty.trim();
    t.len() == 1 && t.chars().all(|c| c.is_ascii_uppercase())
}
