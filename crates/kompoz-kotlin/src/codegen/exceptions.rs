//! Kotlin's exceptions (stage 14 WPB): `throw`, `try`/`catch`/`finally` (a statement and a value), the standard
//! exception classes and the user's classes deriving from one, on Rust's unwinding (`kompoz_runtime::kt::throwable`;
//! **the program needs `panic = "unwind"`**, Rust's default).
//!
//! | Kotlin | Rust |
//! |---|---|
//! | `throw e` | `kt::throw(e)` (a panic of the exception's text, the exception kept for its `catch`) |
//! | `IllegalStateException("m")`, `Exception("m", cause)`, `RuntimeException(cause)` | `kt::exception("IllegalStateException", …)`: a `kt::Throwable` |
//! | a type `Exception`, `IllegalArgumentException`, `Throwable`, … | `kt::Throwable` (the class is the object's: `e is IllegalArgumentException` is `e.is_a("IllegalArgumentException")`) |
//! | `try { a } catch (e: X) { b } finally { c }` | `match kt::catching(\|\| a) { Ok(v) => v, Err(e) if e.is_a("X") => b, Err(e) => kt::rethrow(e) }` (`c` after both, whatever happened); in a coroutine `kt::catching_async(async { a }).await` |
//! | `class E(val code: Int) : IllegalStateException("kod $code")` | a plain class with the exception's message and cause, `kt::KtThrowable` (what `throw` takes), `Into<kt::Throwable>` (what an `Exception` holds it as), `KtAs<E> for kt::Throwable` (`e as E`, `e is E`, `catch (e: E)`); its text Kotlin's `E: kod 7` |
//!
//! A `return`, `break` or `continue` in a `try` body (or, with a `finally`, in a `catch`) leaves the closure the body
//! is: the closure gives `kt::Jump::Return(v)` / `Loop(n)` and the code after the `try` does the jump (the
//! [`JumpFrame`]s). A `try` around a composable call is an error, as Kotlin's compiler says ("Try catch is not
//! supported around composable function invocations": a composition can't be left half written).
//!
//! `catch (e: CancellationException)` in a coroutine: KompozR's coroutines are cancelled by dropping them where they
//! wait, not by throwing; such a `catch` runs when the body's coroutine is cancelled while the body runs (a guard
//! dropped with it, its body a non-suspending block, as Kotlin's in a cancelled coroutine can't suspend), and as any
//! other on a `CancellationException` thrown.

use super::Gen;
use super::expr::{Out, Want};
use super::names::snake;
use super::resolve::*;
use super::stmt::Tail;
use crate::ast::*;
use crate::pos::Loc;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};

/// What the generator keeps for functions and control flow (stage 14 WPB).
#[derive(Default)]
pub(crate) struct Control {
    /// The closures being written whose jumps leave them, innermost last.
    pub jump_frames: Vec<JumpFrame>,
    /// How many calls of composables have been written (a `try` body may make none).
    pub kompozable_calls: usize,
    /// The local `var`s (by their name's place) a kept lambda or a local function assigns: held in a `kt::Ref`
    /// (Kotlin's capture by reference; see `kompoz_runtime::kt::reference`).
    pub shared_vars: std::collections::HashSet<(u32, u32)>,
    /// Such a `var` found while writing a declaration: it is written again, the `var` shared.
    pub shared_found: bool,
    /// The declaration's `var`s assigned (after their initializer), and those a kept lambda or local function uses.
    pub assigned_vars: std::collections::HashSet<(u32, u32)>,
    pub captured_vars: std::collections::HashSet<(u32, u32)>,
    /// The top-level functions declared more than once, by name and receiver (see [`super::overloads`]).
    pub overloaded: std::collections::HashSet<(String, Option<String>)>,
    /// The file's extension properties, by name and receiver type (Rust's owner name): whether each is a `var`.
    pub ext_props: std::collections::HashMap<(String, String), bool>,
    /// The parameters of the file's top-level functions (not extensions), by name, each overload's (a callable
    /// reference's lambda typed by them).
    pub fn_params: std::collections::HashMap<String, Vec<Vec<Param>>>,
    /// A jump out of the lambda being written was met (see `jumps`).
    pub escaped: bool,
    /// (stage 14 FINAL-T) A `hashCode()` override is being written: its `Int`/`Long` `+`, `-`, `*` wrap, as
    /// Kotlin's do (`31 * h + e.hashCode()` of large hashes), not Rust's overflow checks.
    pub wrapping: bool,
    /// The local functions in scope by Rust name: their parameters (their defaults filled in at a call).
    pub local_fns: std::collections::HashMap<String, Vec<Param>>,
    /// The local extension functions: their Kotlin names and receivers' types (`x.twice()` calls one).
    pub local_exts: Vec<(String, String)>,
    /// A plain class's object being initialized (`__kz_init`): a kept lambda using it holds it weakly (`capture`).
    pub in_init: bool,
    /// The function type a callable reference is given to (its parameters: which overload `::f` names).
    pub expected_fn: Option<Vec<super::resolve::Ty>>,
    /// The calls being written that take a lambda, innermost last: where a jump out of an inline function's lambda
    /// lands (see `jumps`).
    pub leaves: Vec<LeaveFrame>,
}

/// A call taking a lambda, being written: the jumps out of its lambdas that land at it (Kotlin's non-local `return`,
/// `break`, `continue` out of an inline function's lambda: `kt::leave` in the lambda, `kt::leaving` around the call).
#[derive(Clone, Debug, Default)]
pub(crate) struct LeaveFrame {
    /// How many lambdas were being written outside it (the jumps landing at it are of that depth).
    pub lambdas: usize,
    /// How many loops were open outside it.
    pub loops: usize,
    /// The slot the jump is kept in (`__kz_leave<line>_<column>`, by the call's place).
    pub slot: String,
    /// A `return` (of the function, or of the lambda around the call) leaves through it.
    pub returned: bool,
    /// The loops jumped to (as [`JumpFrame::jumps`]).
    pub jumps: Vec<(usize, bool, Option<Ident>)>,
}

/// A closure being written whose `return`, `break` and `continue` leave the function or loop around it (a `try`
/// body's): what they became, for the code after it.
#[derive(Clone, Debug, Default)]
pub(crate) struct JumpFrame {
    /// How many lambdas were being written outside it (a jump in a lambda inside is the lambda's own).
    pub lambdas: usize,
    /// How many loops were open outside it.
    pub loops: usize,
    /// A `return` (or `return@label` of the lambda around it) left it.
    pub returned: bool,
    /// The loops jumped to (`break`/`continue` of the loop at that index of `Gen::loops`, its label as written).
    pub jumps: Vec<(usize, bool, Option<Ident>)>,
}

/// The standard exception classes KompozR knows (Kotlin's, Java's, kotlinx's: `kompoz_runtime::kt::throwable`'s).
pub(crate) fn standard_exception(name: &str) -> bool {
    matches!(
        name,
        "Throwable"
            | "Exception"
            | "Error"
            | "RuntimeException"
            | "IOException"
            | "InterruptedException"
            | "CloneNotSupportedException"
            | "IllegalArgumentException"
            | "IllegalStateException"
            | "IndexOutOfBoundsException"
            | "NoSuchElementException"
            | "UnsupportedOperationException"
            | "ArithmeticException"
            | "ClassCastException"
            | "NullPointerException"
            | "ConcurrentModificationException"
            | "UninitializedPropertyAccessException"
            | "NoWhenBranchMatchedException"
            | "DateTimeException"
            | "ArrayStoreException"
            | "NegativeArraySizeException"
            | "NumberFormatException"
            | "PatternSyntaxException"
            | "IllegalFormatException"
            | "SerializationException"
            | "StringIndexOutOfBoundsException"
            | "ArrayIndexOutOfBoundsException"
            | "KotlinNullPointerException"
            | "CancellationException"
            | "TimeoutCancellationException"
            | "FileNotFoundException"
            | "EOFException"
            | "NotImplementedError"
            | "AssertionError"
            | "OutOfMemoryError"
            | "StackOverflowError"
    )
}

/// Whether `name` is a cancellation's class or one a cancellation is (what `catch` of it takes a cancellation).
fn takes_cancellation(name: &str) -> bool {
    name == "CancellationException"
}

impl Gen<'_, '_> {
    /// Whether a Kotlin type's name is an exception class: a standard one (`kt::Throwable`) — not one the user
    /// declares or a local names.
    pub(crate) fn exception_type(&self, name: &str) -> bool {
        standard_exception(name) && self.user_class(name).is_none()
    }

    /// The standard class a user's class derives from (`class Refused(…) : IllegalStateException(…)`), and the
    /// arguments it gives that class's constructor.
    pub(crate) fn exception_super<'c>(&self, c: &'c Class) -> Option<(String, &'c [Arg])> {
        let [s] = c.supers.as_slice() else { return None };
        let name = s.ty.simple_name()?;
        let name = super::last_segment(&name).to_string();
        if !self.exception_type(&name) {
            return None;
        }
        Some((name, s.constructor_args.as_deref().unwrap_or(&[])))
    }

    /// Whether a type (the generator's) is a user's exception class (one deriving from a standard one, or from the
    /// user's open one: its value an open one's shared object too).
    pub(crate) fn user_exception(&self, ty: &Ty) -> bool {
        let Some(owner) = ty.owner() else { return false };
        self.classes.decls.iter().any(|d| {
            (d.info.rust == owner || d.info.members_trait().as_deref() == Some(owner))
                && (matches!(&d.ast, super::class::ClassAst::Class(k) if self.exception_super(k).is_some())
                    || (d.info.rust == owner && self.exception_root(d).is_some()))
        })
    }

    /// The user's classes from a class's base class up to the user's open exception class it derives from (Kotlin's
    /// names, nearest first), the standard class that one derives from, and its members' trait.
    pub(crate) fn exception_root(&self, d: &super::class::ClassDecl) -> Option<(Vec<String>, String, String)> {
        let mut supers = Vec::new();
        // (the open class's own objects: their class is it)
        if let Some(own) = self.decl_of(&d.info.rust)
            && own.info.inherit != super::hierarchy::Inherit::Final
            && let super::class::ClassAst::Class(k) = &own.ast
            && let Some((std, _)) = self.exception_super(k)
        {
            return Some((Vec::new(), std, own.info.members_trait()?));
        }
        for b in self.class_chain(d) {
            supers.push(b.info.kotlin.rsplit('.').next().unwrap_or(&b.info.kotlin).to_string());
            if let super::class::ClassAst::Class(k) = &b.ast
                && let Some((std, _)) = self.exception_super(k)
            {
                return Some((supers, std, b.info.members_trait()?));
            }
        }
        None
    }

    /// What a class deriving from the user's open exception class adds (`class NetworkException(val code: Int) :
    /// AppException("ağ $code")`): the impls making it a Kotlin exception of its classes (its own and those above
    /// it), its message and cause its base part's, its object an open class's shared object where caught as one.
    pub(crate) fn exception_chain_parts(
        &mut self,
        d: &super::class::ClassDecl,
        rust: &str,
        own_to_string: bool,
        span: Span,
    ) -> Option<ExceptionParts> {
        let (supers, std, root) = self.exception_root(d)?;
        let kt = self.kt(span);
        let name = proc_macro2::Ident::new(rust, span);
        let tr = proc_macro2::Ident::new(&root, span);
        let class_lit = proc_macro2::Literal::string(d.info.kotlin.rsplit('.').next().unwrap_or(&d.info.kotlin));
        let base_lit = proc_macro2::Literal::string(&std);
        let supers = supers.iter().map(|s| proc_macro2::Literal::string(s));
        let display = if own_to_string {
            quote_spanned!(span=> __kz_out.push_str(&self.to_string_kt());)
        } else {
            quote_spanned!(span=> __kz_out.push_str(&#kt::KtToString::kt_to_string(&#kt::KtThrowable::to_throwable(self)));)
        };
        let impls = quote_spanned! {span=>
            #[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
            impl #kt::KtThrowable for #name {
                fn to_throwable(&self) -> #kt::Throwable {
                    #kt::Throwable::of_class_object(
                        #class_lit,
                        &[#(#supers),*],
                        #base_lit,
                        <Self as #tr>::message(self),
                        <Self as #tr>::cause(self),
                        ::std::rc::Rc::new(::core::clone::Clone::clone(self)),
                        |o, to| <#name as #tr>::__kz_cast(o.downcast_ref::<#name>()?, to),
                    )
                }
            }
            #[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
            impl ::core::convert::From<#name> for #kt::Throwable {
                fn from(e: #name) -> #kt::Throwable {
                    #kt::KtThrowable::to_throwable(&e)
                }
            }
            #[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
            impl #kt::KtAs<#name> for #kt::Throwable {
                fn kt_as_safe(&self) -> ::core::option::Option<#name> {
                    self.downcast::<#name>()
                }
            }
        };
        Some(ExceptionParts { field: quote!(), empty: quote!(), init: quote!(), members: quote!(), display, impls })
    }

    // ---- throw -------------------------------------------------------------------------------------------------

    /// `throw e`.
    pub(crate) fn throw_expr(&mut self, value: &Expr, loc: Loc) -> Out {
        let span = self.span(loc);
        let kt = self.kt(span);
        let v = self.value(value);
        Out::plain(quote_spanned!(span=> #kt::throw(#v)), Ty::Unknown)
    }

    /// A standard exception class's constructor (`IllegalArgumentException("boş")`, `Exception(message, cause)`),
    /// when `n` names one (and nothing of the user's or a local does).
    pub(crate) fn exception_constructor(&mut self, n: &Ident, args: &[Arg], loc: Loc) -> Option<Out> {
        if !self.exception_type(&n.name) || self.is_local(&n.name) || self.file.function(&n.name).is_some() {
            return None;
        }
        let span = self.span(loc);
        let kt = self.kt(span);
        let class = proc_macro2::Literal::string(&n.name);
        let none = quote_spanned!(span=> ::core::option::Option::None);
        let message_of = |me: &mut Self, e: &Expr| -> TokenStream {
            let v = me.value_want(e, Want { some: true, ..Want::default() });
            quote_spanned!(span=> ::core::option::Option::map(#v, ::std::string::String::from))
        };
        let cause_of = |me: &mut Self, e: &Expr| -> TokenStream {
            let ty = me.peek_ty(e);
            let v = me.value(e);
            if ty.is_nullable() {
                quote_spanned!(span=> ::core::option::Option::map(#v, |c| #kt::KtThrowable::to_throwable(&c)))
            } else {
                quote_spanned!(span=> ::core::option::Option::Some(#kt::KtThrowable::to_throwable(&#v)))
            }
        };
        let named = |name: &str| args.iter().find(|a| a.name.as_ref().is_some_and(|x| x.name == name));
        let ts = match (args, named("message"), named("cause")) {
            ([], _, _) => quote_spanned!(span=> #kt::exception(#class, #none, #none)),
            (_, m, c) if m.is_some() || c.is_some() => {
                let m = m.map(|a| message_of(self, &a.value)).unwrap_or_else(|| none.clone());
                let c = c.map(|a| cause_of(self, &a.value)).unwrap_or_else(|| none.clone());
                quote_spanned!(span=> #kt::exception(#class, #m, #c))
            }
            ([a], _, _) => {
                let ty = self.peek_ty(&a.value);
                let throwable = matches!(ty.non_null(), Ty::Named(ref t, _) if t == "Throwable")
                    || self.user_exception(&ty.non_null());
                if throwable {
                    let c = cause_of(self, &a.value);
                    quote_spanned!(span=> #kt::exception(#class, #none, #c))
                } else {
                    let m = message_of(self, &a.value);
                    quote_spanned!(span=> #kt::exception(#class, #m, #none))
                }
            }
            ([m, c], _, _) => {
                let m = message_of(self, &m.value);
                let c = cause_of(self, &c.value);
                quote_spanned!(span=> #kt::exception(#class, #m, #c))
            }
            _ => {
                self.error(loc, format!("`{}` takes a message and a cause", n.name));
                return Some(Out::plain(quote_spanned!(span=> ::core::unreachable!()), Ty::named("Throwable")));
            }
        };
        Some(Out::plain(ts, Ty::named("Throwable")))
    }

    // ---- is / as -----------------------------------------------------------------------------------------------

    /// `e is IllegalStateException` (a standard class: the object's class, or one it derives from).
    pub(crate) fn exception_is(&mut self, inner: &Expr, negated: bool, ty: &Type, loc: Loc) -> Option<Out> {
        let name = ty.simple_name()?;
        if !self.exception_type(super::last_segment(&name)) {
            return None;
        }
        let span = self.span(loc);
        let class = proc_macro2::Literal::string(super::last_segment(&name));
        let operand = self.expr(inner);
        let o = operand.ts;
        let ts = if operand.ty.is_nullable() {
            quote_spanned!(span=> ::core::option::Option::is_some_and(&#o, |e| e.is_a(#class)))
        } else if self.user_exception(&operand.ty) {
            let kt = self.kt(span);
            quote_spanned!(span=> #kt::KtThrowable::to_throwable(&#o).is_a(#class))
        } else {
            quote_spanned!(span=> #o.is_a(#class))
        };
        Some(Out::plain(if negated { quote_spanned!(span=> !(#ts)) } else { ts }, Ty::Bool))
    }

    /// `e as IllegalStateException` / `as?` (a standard class): the exception itself, or a ClassCastException
    /// (`as?`: null) when it isn't one.
    pub(crate) fn exception_cast(&mut self, inner: &Expr, safe: bool, ty: &Type, loc: Loc) -> Option<Out> {
        let name = ty.simple_name()?;
        if !self.exception_type(super::last_segment(&name)) {
            return None;
        }
        let span = self.span(loc);
        let kt = self.kt(span);
        let class = proc_macro2::Literal::string(super::last_segment(&name));
        let o = self.value(inner);
        let t = self.fresh("t", span);
        let checked = quote_spanned!(span=> {
            let #t = #kt::KtThrowable::to_throwable(&#o);
            if #t.is_a(#class) { ::core::option::Option::Some(#t) } else { ::core::option::Option::None }
        });
        if safe || ty.nullable {
            Some(Out::plain(checked, Ty::named("Throwable").nullable()))
        } else {
            let msg = format!("the exception cannot be cast to {}", super::last_segment(&name));
            Some(Out::plain(
                quote_spanned!(span=> match #checked {
                    ::core::option::Option::Some(e) => e,
                    ::core::option::Option::None => #kt::throw(#kt::exception("ClassCastException", ::core::option::Option::Some(::std::string::String::from(#msg)), ::core::option::Option::None)),
                }),
                Ty::named("Throwable"),
            ))
        }
    }

    // ---- try ---------------------------------------------------------------------------------------------------

    /// The type of a `catch`'s value, its exception named in it.
    pub(crate) fn peek_catch(&self, c: &Catch) -> Ty {
        let class = c.ty.simple_name().unwrap_or_default();
        let class = super::last_segment(&class);
        let ty = match self.user_class(class) {
            Some(u) => Ty::named(&u.rust),
            None => Ty::named("Throwable"),
        };
        self.peek_scopes.borrow_mut().push(std::iter::once((c.name.name.clone(), ty)).collect());
        let t = self.peek_block(&c.body);
        self.peek_scopes.borrow_mut().pop();
        t
    }

    /// `try { … } catch (…) { … } finally { … }`, as a statement (`tail` Unit) or a value.
    pub(crate) fn try_expr(&mut self, t: &Try, tail: Tail, loc: Loc) -> Out {
        let span = self.span(loc);
        let kt = self.kt(span);
        let unit = matches!(tail, Tail::Unit);
        let ty = if unit {
            Ty::Unit
        } else {
            let mut tys = vec![self.peek_block(&t.body)];
            tys.extend(t.catches.iter().map(|c| self.peek_catch(c)));
            self.joined(&tys, tail).0
        };
        let tail = if unit {
            tail
        } else {
            let mut tys = vec![self.peek_block(&t.body)];
            tys.extend(t.catches.iter().map(|c| self.peek_catch(c)));
            self.joined(&tys, tail).1
        };
        if t.catches.is_empty() && t.finally.is_none() {
            self.error(loc, "a `try` needs a `catch` or a `finally`");
        }

        // The body, a closure whose jumps are the frame's.
        let composables = self.control.kompozable_calls;
        self.control.jump_frames.push(JumpFrame {
            lambdas: self.lambdas.len(),
            loops: self.loops.len(),
            ..Default::default()
        });
        let awaits = self.awaits;
        let body = self.braced(&t.body, tail);
        let body_async = self.in_async && self.awaits > awaits;
        let mut frame = self.control.jump_frames.pop().expect("its frame");
        if self.control.kompozable_calls > composables {
            self.error(loc, "Try catch is not supported around composable function invocations.");
        }
        let e = format_ident!("__kz_e", span = span);
        let r = format_ident!("__kz_r", span = span);

        // The catches: an `if` chain on the exception's class, rethrowing what none takes; with a `finally`, in a
        // closure of their own (the `finally` runs whatever they do).
        let in_closure = t.finally.is_some();
        if in_closure {
            self.control.jump_frames.push(JumpFrame {
                lambdas: self.lambdas.len(),
                loops: self.loops.len(),
                returned: frame.returned,
                jumps: frame.jumps.clone(),
            });
        }
        let awaits_catch = self.awaits;
        let mut cancellation: Option<TokenStream> = None;
        let chain = self.catch_chain(&t.catches, tail, &e, span, &mut cancellation);
        let catch_async = self.in_async && self.awaits > awaits_catch;
        if in_closure {
            frame = self.control.jump_frames.pop().expect("its frame");
        }
        let jumps = frame.returned || !frame.jumps.is_empty();
        let wrap = |b: TokenStream| -> TokenStream {
            if jumps {
                let r = if frame.returned { quote_spanned!(span=> _) } else { quote_spanned!(span=> ()) };
                quote_spanned!(span=> #kt::Jump::<_, #r>::Value(#b))
            } else {
                b
            }
        };
        let caught = |me: &mut Self, b: TokenStream, is_async: bool| -> TokenStream {
            if is_async {
                let a = me.awaited(None, loc);
                quote_spanned!(span=> #kt::catching_async(async { #b }) #a)
            } else {
                quote_spanned!(span=> #kt::catching(|| { #b }))
            }
        };
        let body_v = wrap(body);
        let body_caught = caught(self, body_v, body_async);
        let ok = if jumps { self.jump_match(&frame, quote_spanned!(span=> __kz_v), span) } else { quote!(__kz_v) };
        let ts = match &t.finally {
            None => quote_spanned!(span=> match #body_caught {
                ::core::result::Result::Ok(__kz_v) => #ok,
                ::core::result::Result::Err(#e) => #chain,
            }),
            Some(f) => {
                let fin = self.braced(f, Tail::Unit);
                let chain = if t.catches.is_empty() {
                    quote_spanned!(span=> ::core::result::Result::Err(#e))
                } else {
                    let c = wrap(chain);
                    caught(self, c, catch_async)
                };
                quote_spanned!(span=> {
                    let #r = match #body_caught {
                        ::core::result::Result::Ok(__kz_v) => ::core::result::Result::Ok(__kz_v),
                        ::core::result::Result::Err(#e) => #chain,
                    };
                    #fin;
                    match #r {
                        ::core::result::Result::Ok(__kz_v) => #ok,
                        ::core::result::Result::Err(#e) => #kt::rethrow(#e),
                    }
                })
            }
        };
        // (a catch of a cancellation in a coroutine: run when the coroutine is dropped while the body runs)
        let ts = match cancellation {
            Some(guard) if self.in_async => {
                quote_spanned!(span=> {
                    let __kz_cancelled = #kt::OnCancel::new(#guard);
                    let __kz_t = #ts;
                    __kz_cancelled.disarm();
                    __kz_t
                })
            }
            _ => ts,
        };
        Out::plain(ts, ty)
    }

    /// The `catch` clauses of a `try` as an `if` chain on the exception `e` (rethrown when none takes it). A catch
    /// of a cancellation in a coroutine also gives its body as a guard's closure (`cancellation`).
    fn catch_chain(
        &mut self,
        catches: &[Catch],
        tail: Tail,
        e: &proc_macro2::Ident,
        span: Span,
        cancellation: &mut Option<TokenStream>,
    ) -> TokenStream {
        let kt = self.kt(span);
        let mut arms: Vec<(Option<TokenStream>, TokenStream)> = Vec::new();
        for c in catches {
            let cspan = self.span(c.loc);
            let name = c.ty.simple_name().unwrap_or_default();
            let class = super::last_segment(&name).to_string();
            let user = self.user_class(&class).map(|u| u.rust.clone());
            let open = user.as_ref().and_then(|r| self.user_class_rust(r)).is_some_and(|c| c.is_dyn());
            let (cond, bind, ty): (Option<TokenStream>, TokenStream, Ty) = match &user {
                // ((WPA-rest) the user's open exception class: an object of its class or of one deriving from it, as
                // its shared object)
                Some(rust) if open && self.exception_type_rust(rust) => {
                    let lit = proc_macro2::Literal::string(&class);
                    let ty = self.ty_of(&c.ty);
                    let t = self.rust_type(&c.ty, super::types::TypePos::Value);
                    (
                        Some(quote_spanned!(cspan=> #e.is_a(#lit))),
                        quote_spanned!(cspan=> ::core::option::Option::unwrap(#e.cast_object::<#t>())),
                        ty,
                    )
                }
                // ((stage 14 FINAL-T) the user's sealed exception class: an object of one of its cases, as the enum)
                Some(rust)
                    if self.exception_type_rust(rust)
                        && self
                            .user_class_rust(rust)
                            .is_some_and(|c| c.inherit == super::hierarchy::Inherit::Sealed) =>
                {
                    let lit = proc_macro2::Literal::string(&class);
                    let t = proc_macro2::Ident::new(rust, self.span(c.ty.loc));
                    (
                        Some(quote_spanned!(cspan=> #e.is_a(#lit))),
                        quote_spanned!(cspan=> ::core::option::Option::unwrap(#kt::KtAs::<#t>::kt_as_safe(&#e))),
                        Ty::named(rust),
                    )
                }
                Some(rust) if self.exception_type_rust(rust) => {
                    let t = proc_macro2::Ident::new(rust, self.span(c.ty.loc));
                    (
                        Some(quote_spanned!(cspan=> #e.is_object::<#t>())),
                        quote_spanned!(cspan=> ::core::option::Option::unwrap(#e.downcast::<#t>())),
                        Ty::named(rust),
                    )
                }
                Some(_) => {
                    self.error(
                        c.ty.loc,
                        format!("`{class}` isn't an exception class (it derives from no `Throwable`)"),
                    );
                    continue;
                }
                None if self.exception_type(&class) => {
                    let lit = proc_macro2::Literal::string(&class);
                    // (`Throwable` too: what a jump out of an inline function's lambda leaves through isn't one, see
                    // `jumps`)
                    (Some(quote_spanned!(cspan=> #e.is_a(#lit))), quote_spanned!(cspan=> #e), Ty::named("Throwable"))
                }
                None => {
                    self.error(c.ty.loc, format!("`{class}` isn't an exception class KompozR knows"));
                    continue;
                }
            };
            self.push_scope();
            let local = (c.name.name != "_").then(|| {
                let rust = snake(&c.name.name);
                let i = self.name_ident(&rust, c.name.loc);
                self.declare_at(&c.name, rust, LocalKind::Val, ty.clone());
                i
            });
            let body = self.in_range(c.body.loc, |g| g.block_inner(&c.body, tail));
            self.pop_scope();
            let bind = match &local {
                Some(i) => quote_spanned!(cspan=> #[allow(unused_variables)] let #i = #bind;),
                None => quote!(),
            };
            if takes_cancellation(&class) && self.in_async && cancellation.is_none() {
                // (its body again, as the guard's closure: the cancelled coroutine's, run as it is dropped)
                let guard = self.quiet(|g| {
                    g.push_scope();
                    if let Some(i) = &local {
                        g.declare_at(&c.name, i.to_string(), LocalKind::Val, ty.clone());
                    }
                    let was = std::mem::replace(&mut g.in_async, false);
                    let b = g.in_range(c.body.loc, |g| g.block_inner(&c.body, Tail::Unit));
                    g.in_async = was;
                    g.pop_scope();
                    b
                });
                let bind_c = match &local {
                    Some(i) => {
                        quote_spanned!(cspan=> #[allow(unused_variables)] let #i = #kt::exception("CancellationException", ::core::option::Option::None, ::core::option::Option::None);)
                    }
                    None => quote!(),
                };
                *cancellation = Some(quote_spanned!(cspan=> || { #bind_c #guard }));
            }
            arms.push((cond, quote_spanned!(cspan=> { #bind #body })));
        }
        // (the first that takes it; none: thrown on)
        let mut out = quote_spanned!(span=> #kt::rethrow(#e));
        for (cond, body) in arms.into_iter().rev() {
            out = match cond {
                Some(c) => quote_spanned!(span=> if #c #body else { #out }),
                None => body,
            };
        }
        out
    }

    /// Whether a user's class of Rust name `rust` is an exception class.
    fn exception_type_rust(&self, rust: &str) -> bool {
        self.user_exception(&Ty::named(rust))
    }

    // ---- Jumps out of a closure ---------------------------------------------------------------------------------

    /// `return v` (`v` none: `return`) where it is written: out of the closure being written as a `try` body
    /// (`return kt::Jump::Return(v)`), else Rust's.
    pub(crate) fn return_tokens(&mut self, v: Option<TokenStream>, span: Span) -> TokenStream {
        let lambdas = self.lambdas.len();
        if let Some(f) = self.control.jump_frames.last_mut()
            && f.lambdas == lambdas
        {
            f.returned = true;
            let kt = self.kt(span);
            let v = v.unwrap_or_else(|| quote_spanned!(span=> ()));
            return quote_spanned!(span=> return #kt::Jump::Return(#v));
        }
        match v {
            Some(v) => quote_spanned!(span=> return #v),
            None => quote_spanned!(span=> return),
        }
    }

    /// A jump to the loop at index `t` of the loops (out of the closure being written as a `try` body, when the loop
    /// is outside it), or `None` when it is Rust's own (inside).
    pub(crate) fn loop_jump_out(
        &mut self,
        t: usize,
        is_break: bool,
        label: Option<&Ident>,
        span: Span,
    ) -> Option<TokenStream> {
        let lambdas = self.lambdas.len();
        let f = self.control.jump_frames.last_mut()?;
        if f.lambdas != lambdas || t >= f.loops {
            return None;
        }
        let n = f.jumps.len() as u32;
        f.jumps.push((t, is_break, label.cloned()));
        let kt = self.kt(span);
        Some(quote_spanned!(span=> return #kt::Jump::Loop(#n)))
    }

    /// The match doing a closure's jumps (`v` its `kt::Jump`), where the code after it is.
    fn jump_match(&mut self, frame: &JumpFrame, v: TokenStream, span: Span) -> TokenStream {
        let kt = self.kt(span);
        let mut arms = vec![quote_spanned!(span=> #kt::Jump::Value(__kz_x) => __kz_x,)];
        if frame.returned {
            let ret = self.return_tokens(Some(quote_spanned!(span=> __kz_x)), span);
            arms.push(quote_spanned!(span=> #kt::Jump::Return(__kz_x) => #ret,));
        } else {
            arms.push(quote_spanned!(span=> #kt::Jump::Return(()) => ::core::unreachable!(),));
        }
        for (n, (t, is_break, label)) in frame.jumps.iter().enumerate() {
            let n = n as u32;
            let j = self.loop_jump_tokens(*t, *is_break, label.as_ref(), span);
            arms.push(quote_spanned!(span=> #kt::Jump::Loop(#n) => #j,));
        }
        arms.push(quote_spanned!(span=> #kt::Jump::Loop(_) => ::core::unreachable!(),));
        quote_spanned!(span=> match #v { #(#arms)* })
    }

    /// `break`/`continue` of the loop at index `t` where it is written.
    pub(crate) fn loop_jump_tokens(
        &mut self,
        t: usize,
        is_break: bool,
        label: Option<&Ident>,
        span: Span,
    ) -> TokenStream {
        if let Some(out) = self.loop_jump_out(t, is_break, label, span) {
            return out;
        }
        let lp = self.loops[t].clone();
        // (a lambda written as a loop: by its label)
        if let Some((rl, _)) = &lp.lowered {
            *lp.named.borrow_mut() = Some(rl.clone());
            let lt = super::lifetime(rl, span);
            return if is_break { quote_spanned!(span=> break #lt) } else { quote_spanned!(span=> continue #lt) };
        }
        if !is_break && let Some(bl) = &lp.body_label {
            let lt = super::lifetime(bl, span);
            return quote_spanned!(span=> break #lt);
        }
        // (a loop Kotlin doesn't label, jumped to from a lambda written as a loop inside it: labeled for Rust)
        let inner_lowered = self.loops[t + 1..].iter().any(|x| x.lowered.is_some());
        let lt = match label {
            Some(l) => Some(super::lifetime(&snake(&l.name), self.span(l.loc))),
            None if inner_lowered => {
                let name = {
                    let mut named = lp.named.borrow_mut();
                    if named.is_none() {
                        *named = Some(self.fresh("loop", span).to_string());
                    }
                    named.clone().expect("named")
                };
                Some(super::lifetime(&name, span))
            }
            None => None,
        };
        if is_break { quote_spanned!(span=> break #lt) } else { quote_spanned!(span=> continue #lt) }
    }

    // ---- The user's exception classes --------------------------------------------------------------------------

    /// What a plain class deriving from a standard exception class adds (`class Refused(val code: Int) :
    /// IllegalStateException("kod $code")`): its field holding the message and cause its constructor gives the base
    /// (set first, as Kotlin's superclass constructor runs first), the members `message` and `cause`, and the
    /// impls making it a Kotlin exception. `own_to_string`: the class overrides `toString()`.
    pub(crate) fn exception_class_parts(
        &mut self,
        c: &Class,
        rust: &str,
        own_to_string: bool,
        span: Span,
    ) -> Option<ExceptionParts> {
        let (base, args) = self.exception_super(c)?;
        let args = args.to_vec();
        let kt = self.kt(span);
        let name = proc_macro2::Ident::new(rust, span);
        // (the base's constructor, its arguments the constructor's parameters in scope)
        let ctor = Ident { name: base.clone(), backticked: false, loc: c.loc };
        let made = self.exception_constructor(&ctor, &args, c.loc).map(|o| o.ts);
        let made = made.unwrap_or_else(|| quote_spanned!(span=> #kt::exception("Exception", ::core::option::Option::None, ::core::option::Option::None)));
        let field = quote_spanned!(span=> #[doc(hidden)] pub __kz_exception: #kt::KtVal<#kt::Throwable>);
        let empty = quote_spanned!(span=> __kz_exception: #kt::KtVal::new());
        let init = quote_spanned!(span=> self.__kz_this.__kz_exception.init(#made););
        let class_lit = proc_macro2::Literal::string(&c.name.name);
        let base_lit = proc_macro2::Literal::string(&base);
        let members = quote_spanned! {span=>
            /// Kotlin's `message`.
            pub fn message(&self) -> ::core::option::Option<::std::string::String> {
                self.__kz_this.__kz_exception.with(|e| e.message())
            }
            /// Kotlin's `cause`.
            pub fn cause(&self) -> ::core::option::Option<#kt::Throwable> {
                self.__kz_this.__kz_exception.with(|e| e.cause())
            }
        };
        let display = if own_to_string {
            quote_spanned!(span=> __kz_out.push_str(&self.to_string_kt());)
        } else {
            quote_spanned!(span=> __kz_out.push_str(&#kt::KtToString::kt_to_string(&#kt::KtThrowable::to_throwable(self)));)
        };
        let impls = quote_spanned! {span=>
            #[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
            impl #kt::KtThrowable for #name {
                fn to_throwable(&self) -> #kt::Throwable {
                    let (__kz_m, __kz_c) = self.__kz_this.__kz_exception.with(|e| (e.message(), e.cause()));
                    #kt::Throwable::of_object(#class_lit, #base_lit, __kz_m, __kz_c, ::std::rc::Rc::new(::core::clone::Clone::clone(self)))
                }
            }
            #[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
            impl ::core::convert::From<#name> for #kt::Throwable {
                fn from(e: #name) -> #kt::Throwable {
                    #kt::KtThrowable::to_throwable(&e)
                }
            }
            #[allow(non_snake_case, non_camel_case_types, dead_code, clippy::all)]
            impl #kt::KtAs<#name> for #kt::Throwable {
                fn kt_as_safe(&self) -> ::core::option::Option<#name> {
                    self.downcast::<#name>()
                }
            }
        };
        Some(ExceptionParts { field, empty, init, members, display, impls })
    }

    /// The overlay's items of a user's exception class's members (`message`, `cause`).
    pub(crate) fn exception_items(&self, c: &Class, rust: &str) -> Vec<crate::index::Item> {
        if self.exception_super(c).is_none() {
            return Vec::new();
        }
        use crate::index::{Item, Kind, Receiver, flags, overlay};
        let item = |kotlin: &str, ret: &str| Item {
            kotlin: overlay::intern(kotlin),
            rust: overlay::intern(kotlin),
            kind: Kind::Property,
            krate: "",
            module: "",
            owner: overlay::intern(rust),
            via: "",
            receiver: Receiver::Ref,
            params: &[],
            ret: overlay::intern(ret),
            flags: flags::REACHABLE,
        };
        vec![item("message", "Option<String>"), item("cause", "Option<Throwable>")]
    }

    /// A value of a user's exception class given where Kotlin's `Exception` (a `kt::Throwable`) is taken: made one.
    pub(crate) fn as_throwable(&self, v: TokenStream, given: &Ty, target: &Ty, span: Span) -> TokenStream {
        let target_throwable = matches!(target.non_null(), Ty::Named(ref t, _) if t == "Throwable");
        if !target_throwable || !self.user_exception(&given.non_null()) {
            return v;
        }
        let kt = self.kt(span);
        if given.is_nullable() {
            quote_spanned!(span=> ::core::option::Option::map(#v, |e| #kt::KtThrowable::to_throwable(&e)))
        } else if target.is_nullable() {
            quote_spanned!(span=> ::core::option::Option::Some(#kt::KtThrowable::to_throwable(&#v)))
        } else {
            quote_spanned!(span=> #kt::KtThrowable::to_throwable(&#v))
        }
    }
}

/// What [`Gen::exception_class_parts`] gives a plain class.
pub(crate) struct ExceptionParts {
    pub field: TokenStream,
    pub empty: TokenStream,
    pub init: TokenStream,
    pub members: TokenStream,
    /// Its `KtToString`'s body (Kotlin's `Class: message`).
    pub display: TokenStream,
    pub impls: TokenStream,
}
