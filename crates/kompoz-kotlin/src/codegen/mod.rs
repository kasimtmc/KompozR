//! The Rust a Kotlin file becomes (the C++ kompozc's `codegen.cpp`): items `#[kompozable]` then processes, so
//! identity, restarting and skipping, builders, captures and memoized lambdas are the Rust syntax's own. Every token
//! gets the span of the Kotlin it comes from (`SpanMap`): rustc's errors and `#[track_caller]`'s call sites land on
//! Kotlin lines.
//!
//! What is translated (stage 9's Kotlin):
//!
//! | Kotlin | Rust |
//! |---|---|
//! | `@Kompozable fun Name(p: T = d)` (`@Composable` too) | `#[kompozable] pub fn Name(#[default(d)] p: T)`; `inline` → `#[kompozable(inline)]` |
//! | `fun name(…): R = e` / `{ … }` | `#[named] fn name(…) -> R` (`#[named]` when a parameter has a default); its body in `named_args!` |
//! | `fun Recv.name(…)` | a method of `#[named(trait RecvNameExt)] impl Recv` (see below) |
//! | `val NAME: T = e`, `val NAME = e` (top level) | `const` (numbers, booleans, characters, a string literal), a `static` (what a `const fn` makes: `compositionLocalOf { … }`, known by its address), else a `thread_local!` made at first read; its type its value's when not written (a generic's told by a lambda's value or a type argument), a camelCase name in snake_case |
//! | a name whose declaration isn't seen (another file's `val`, a Rust `thread_local!` or constant) | `kt::top_level!(name)`: a thread-local's value, or the value, cloned; a property of it `kt::top_level_with!(Name, |v| v.prop())`, lent in place; an infix on it in place (`Name.provides(v)`) |
//! | `Int Long Float Double Boolean Char String Unit` | `i32 i64 f32 f64 bool char String ()` |
//! | `T?`, `List<T>`, `Map<K, V>`, `Set<T>`, `Pair<A, B>` | `Option<T>`, `Vec<T>`, `kt::KtMap<K, V>`, `kt::KtSet<T>`, `(A, B)` |
//! | parameter `String`, `T?`, `(A) -> R` | `impl Into<String>`, `impl Into<Option<T>>`, `impl Fn(A) -> R + 'static` |
//! | parameter of an interface the API has as a trait (`shape: Shape`) | its object shared as the API holds it (`Arc<dyn Shape>`) |
//! | `val x = e`, `var x = e` | `let x = e;`, `let mut x = e;` (a value read again is cloned: Kotlin shares values) |
//! | `var x by remember { mutableStateOf(0) }` | `let x = remember(|| mutable_state_of(0));`, reads `KtDelegate::get_value(&x)`, writes `KtMutableDelegate::set_value(&x, v)`, `x++` → `post_inc(&x)` |
//! | `rememberUpdatedState(v)` | `remember_updated_state!(v)` (a macro: Kotlin's `==` of `v` — its type's `==`, an `Rc`'s instance, or never — read where its type is known), a `State<T>` |
//! | `"a $b ${c}"` | `kt::template!("a ", b, c)` (Kotlin's `toString()` of each) |
//! | `1`, `1L`, `1f`, `1.0`, `0xFF`, `16.dp` | `1`, `1_i64`, `1_f32`, `1.0`, `0xFF`, `16.dp()` |
//! | `a..b`, `a until b`, `a downTo b`, `r step n`, `x in r` | `kt::range_to`, `kt::until`, `kt::down_to`, `kt::step`, `kt::is_in` |
//! | `a?.b`, `a ?: b`, `a!!`, `null` | `kt::safe!(a, |it| it.b())`, `kt::elvis(a, || b)` (a `match` for `?: return`), `kt::not_null(a)`, `None` |
//! | `if (x != null)` on a local | `if let Some(x) = x.clone()` (Kotlin's smart cast) |
//! | `when (x) { a, b -> …; in r -> …; else -> … }` | an `if` chain on the subject |
//! | `for (x in xs)`, `while`, `do … while`, labels | `for x in KtIterable::kt_iter(&xs)` (a range as it is), `while`, `loop`, `'label:` |
//! | `f(a, name = b) { … }` | `f(a, name = b, last = || …)`: named arguments and defaults through the callee's builder |
//! | `a.b`, `a.b = v` | `a.b()`, `a.set_b(v)` (properties one form); `Type.X` → `Type::X`; `Modifier.Companion` → `Modifier`; a nested object Rust names otherwise (`Arrangement.Absolute.Left` → `AbsoluteArrangement::Left`) |
//! | an argument of another type than its parameter's | what converts it: `Into::into` (a `Stroke` given as a `DrawStyle`), `Rc::new`/`Arc::new` (a value given as an interface), `as i32` (another integer type given as an `Int`), `&v` (a list given as a slice) |
//! | Kotlin's standard members (`xs.map { }`, `s.length`) | the kt shims' traits, by the receiver's type when it is known (`KtList::map(&xs, |it| …)`) |
//! | lambdas | closures: borrowing where the callee runs them before returning (Kotlin's inline functions), else capturing clones (`#[kompozable]` does that for the calls it rewrites); a shared one (`Callback`, `Rc<dyn Fn(&T)>`) made one, its parameters typed; a parameter given a `&mut` (`state.update { it.add(…) }`) reborrowed |
//! | receiver lambdas (`Canvas { drawCircle(…) }`) | a closure taking the scope (`|this| this.draw_circle(…)`): free names that are its members resolve through it |
//! | suspend lambdas (`LaunchedEffect(k) { delay(1) }`) | `async move { delay(1).await }`: known suspending calls are awaited |
//! | an inline function's lambda that suspends in a coroutine (`with(a) { animateTo(1f) }`, `xs.forEach { delay(1) }`) | an async closure to the kt shim's suspending twin, awaited: `kt::with_async(a, async \|this\| this.animate_to(1.0).await).await` |
//! | `a?.f()`, `a ?: b` whose `f`/`b` suspends | `kt::flatten!(kt::safe_async(a, async \|it\| it.f().await).await)`, `kt::elvis_async(a, async \|\| b).await` |
//!
//! Extension functions: `fun Modifier.framed(width: Dp = 2.dp): Modifier` is a method of an extension trait,
//! `#[named(pub trait ModifierFramedExt)] impl Modifier { fn framed(self, #[default(2.dp())] width: Dp) -> Modifier }`:
//! one trait per function (two files may extend one type with functions of their own), named after the receiver
//! and the function, as visible as the function; `#[named]` gives it named arguments and defaults at its calls. A
//! call (`Modifier.framed()`, or `framed()` inside another extension of `Modifier`) is a method call: the trait is
//! in scope where the function is (Rust code elsewhere imports it, as Kotlin imports an extension).
//!
//! Kotlin's imports and package are not translated: the names come from the Rust module's own `use`s (the
//! prelude: `use kompoz::prelude::*;`), the kt shims' traits from a `use` the generated bodies have.
//!
//! Stage 14 adds Kotlin's classes ([`class`], [`object_expr`]) and its functions and control flow: exceptions
//! (`exceptions`: `try`/`catch`/`finally`, `throw`, on unwinding — the program needs `panic = "unwind"`), overloads
//! (`overloads`), operator and infix functions (`operators`), properties by delegates, with accessors, `lateinit`,
//! top-level `var`s and extension properties (`property`), local and anonymous functions, callable references and
//! Kotlin's capture of `var`s by reference (`capture`), jumps out of inline lambdas and inline functions' suspending
//! twins (`jumps`), varargs and spreads, nullable and suspending function types. What isn't translated yet is an error
//! saying it comes with stage 15, at the Kotlin construct.
//! Errors are per declaration: a declaration with an error is left out, the others are translated.

mod call;
mod callbacks;
mod capture;
pub(crate) mod class;
mod exceptions;
mod expr;
mod generics;
pub(crate) mod hierarchy;
mod jumps;
mod lambda;
pub(crate) mod names;
mod object_expr;
mod operators;
mod overloads;
mod property;
mod record;
mod resolve;
mod stmt;
mod types;
mod variance;

use crate::Paths;
use crate::ast::*;
use crate::diag::Diagnostics;
use crate::pos::{Loc, SpanMap};
use names::{ident, pascal, snake, valid, value_name};
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use resolve::*;
use std::collections::HashMap;

/// What the generator works with besides the tree.
pub struct Context<'a> {
    pub spans: &'a dyn SpanMap,
    pub paths: &'a Paths,
    /// See `Options::keyed_calls`.
    pub keyed_calls: bool,
    /// See `Options::module`.
    pub module: Option<&'a crate::module::ModuleTable>,
    /// See `Options::analyze`.
    pub analyze: bool,
}

/// The items `file`'s declarations become; what can't be translated is a diagnostic (and the declaration it is in
/// is left out).
pub fn generate(
    file: &File,
    cx: &Context,
    diags: &mut Diagnostics,
) -> (TokenStream, Option<crate::analysis::Analysis>) {
    // The module table's Kotlin sources, read for their declarations (their own diagnostics are their macros').
    let module_files: Vec<File> = cx
        .module
        .map(|m| {
            m.sources
                .iter()
                .map(|s| {
                    let mut d = Diagnostics::default();
                    let tokens = crate::lexer::lex(&s.text, s.base, &mut d);
                    crate::parser::parse(&tokens, &mut d)
                })
                .collect()
        })
        .unwrap_or_default();
    let own: Vec<&Decl> = file.decls.iter().collect();
    let own_names: std::collections::HashSet<&str> =
        file.decls.iter().filter_map(|d| d.name()).map(|n| n.name.as_str()).collect();
    // (another file's declaration of a name the text declares is that one: the text's own block, read again from
    // its file)
    // ((stage 14 WPB) a function of another file of a name the text declares, of other parameters, is an overload)
    let own_fn = |g: &Function| {
        file.decls.iter().any(|d| {
            matches!(d, Decl::Function(f) if f.name.name == g.name.name
                && overloads::mangled("", &f.params) == overloads::mangled("", &g.params)
                && f.receiver.as_ref().and_then(|r| r.simple_name()) == g.receiver.as_ref().and_then(|r| r.simple_name()))
        })
    };
    let module: Vec<&Decl> = module_files
        .iter()
        .flat_map(|f| &f.decls)
        .filter(|d| {
            class::exported(d)
                && match d {
                    Decl::Function(g) => !own_fn(g),
                    _ => d.name().is_none_or(|n| !own_names.contains(n.name.as_str())),
                }
        })
        .collect();
    let mut g = Gen::new(cx, diags);
    g.overlay = Some(crate::index::overlay::install(rust_overlay(cx.module)));
    g.collect(file, &own, &module);
    let mut out = TokenStream::new();
    for decl in &file.decls {
        let before = g.diags.list.len();
        g.decl_range = Some(decl.loc());
        // ((stage 14 WPB) again with the `var`s its kept lambdas assign shared, when there are some: `capture`)
        let ts = g.shared_pass(|g| g.top_decl(decl));
        if !g.diags.list[before..].iter().any(|d| d.is_error()) {
            out.extend(ts);
        }
    }
    // (stage 14 WPA) each `types(…)` list kept to the classes its code names
    let out = class::trim_types(out);
    let analysis = g.analysis.take();
    (out, analysis)
}

/// The function being written.
#[derive(Clone, Default)]
pub(crate) struct FnState {
    /// Its body is a `#[kompozable]` one: the macro rewrites its calls and wraps its lambdas.
    pub kompozable: bool,
    /// Its result is nullable (Kotlin's `T?`): a value it returns of a type known not to be is given in `Some`.
    pub nullable_result: bool,
    /// Its declared result is an interface the API holds as a shared trait object (`fun anchors(): DraggableAnchors<Int>
    /// = DraggableAnchors { … }`): a value it returns of a type of its own is made one.
    pub shared_result: Option<expr::Shared>,
    /// (stage 14 WPA) Its declared result type: a value it gives of a sealed class's case, or of a class where the
    /// result is an interface, an open class or `Any`, made one (`fun load(): Outcome<Int> = if (…) Success(1) else
    /// Failure("")`).
    pub result: Option<Ty>,
}

/// A loop being written, for `break` and `continue`.
#[derive(Clone, Default)]
pub(crate) struct LoopCx {
    pub label: Option<String>,
    /// A `do … while` whose body is a labeled block (`continue` breaks out of it to the condition).
    pub body_label: Option<String>,
    /// (stage 14 WPB) An inline lambda written as a loop (`xs.forEach { … }` as a `for`, its jumps leaving it: see
    /// `jumps`): its Rust label (`return@forEach` continues it; Kotlin's `break`/`continue` aren't its).
    pub lowered: Option<(String, Vec<String>)>,
    /// (stage 14 WPB) The Rust label a jump from such a loop inside gave this one (a loop Kotlin doesn't label).
    pub named: std::rc::Rc<std::cell::RefCell<Option<String>>>,
}

/// A lambda being written, for `return@label`.
#[derive(Clone)]
pub(crate) struct LambdaCx {
    pub labels: Vec<String>,
    /// How many loops were open outside it (a `break` can't leave a lambda).
    pub loops: usize,
    /// What its result goes to (a `return@label` value too): Kotlin's `Any` made a key.
    pub result: expr::Want,
    /// (stage 14 FINAL) Its callee runs it in place, before it returns (Kotlin's inline function's lambda, not kept,
    /// not a coroutine's): a jump may leave it (`jumps`).
    pub inline: bool,
}

pub(crate) struct Gen<'a, 'd> {
    pub cx: &'a Context<'a>,
    pub diags: &'d mut Diagnostics,
    pub file: FileTable,
    /// Locals and parameters by Kotlin name, innermost scope last.
    pub scopes: Vec<HashMap<String, Local>>,
    pub receivers: Vec<ImplicitReceiver>,
    pub frames: Vec<CaptureFrame>,
    pub fun: FnState,
    /// In an `async` block: suspending calls are awaited.
    pub in_async: bool,
    /// How many `.await`s have been written ([`Gen::awaited`]): a lambda run in place whose body adds some suspends.
    pub awaits: usize,
    /// The last suspending call awaited (its Kotlin name and place): what an error about a lambda that can't suspend
    /// points at.
    pub last_await: Option<(String, Loc)>,
    /// A lambda given to the call whose arguments are being written suspends (an async closure): the call goes to the
    /// callee's suspending twin, awaited.
    pub inline_async: bool,
    /// The lambdas (by place) a suspending twin could take written plain, in a coroutine (they didn't suspend): a call
    /// whose other lambda suspends writes them again as async closures ([`Gen::force_async`]).
    pub plain_twin_lambdas: Vec<Loc>,
    /// The lambdas (by place) of a call going to its suspending twin: written as async closures whether they suspend
    /// or not (the twin takes each of its lambdas as one: `withTransform({ … }) { delay(1) }`).
    pub force_async: Vec<Loc>,
    pub loops: Vec<LoopCx>,
    pub lambdas: Vec<LambdaCx>,
    /// The names of the calls whose arguments are being written (a lambda's implicit label).
    pub call_labels: Vec<String>,
    /// A kt shim's call being written: its receiver's element type and its own (its lambdas' `it`).
    pub elem_hint: Option<(Ty, Ty)>,
    /// A method of a generic type of one type argument being written (`transition.animateFloat { … }` of a
    /// `Transition<Panel>`): its lambdas' parameters of the type's type parameter (`&S`) are that argument, given as
    /// Kotlin's value of it (a clone).
    pub owner_arg: Option<Ty>,
    /// A generic call's element type `T`, known from the collection it is given (`items(rows, key = { it.id })`:
    /// `items_list(items: impl LazyListItems<T>, key: Rc<dyn Fn(&T) -> LazyKey>)`): what its lambdas' `T` is, written
    /// out where a closure made a trait object can't take it from where it goes.
    pub type_arg: Option<Ty>,
    /// (stage 14 FINAL) The type the expression at the place is to have (a property's declared type, a parameter's
    /// of a generic callee whose type parameter is bound): a generic callee's result `T` (`Saver<T>`) bound by it, for
    /// its lambdas (`listSaver(save = { it… })` given to a `Saver<SnapshotStateList<Item>>`).
    pub expected: Option<(Loc, Ty)>,
    /// (stage 14 FINAL) [`Gen::type_arg`] was bound by [`Gen::expected`]: the lambdas' parameters of it are written
    /// with their types.
    pub type_arg_written: bool,
    /// The name of the type parameter [`Gen::type_arg`] binds (`T` of a collection's element, `S` of
    /// `AnimatedContent(targetState: S, …)`'s state).
    pub type_param: String,
    /// The layout scope of the call whose arguments are being written (`RowScope` for a Row: see
    /// [`call::layout_scope_of`]): its `content` lambda's.
    pub layout_scope_callee: Option<&'static str>,
    /// The layout scopes (Kotlin's `@LayoutScopeMarker` receivers) of the content lambdas being written, innermost
    /// last: a call of a name one of them extends goes to the innermost's (`AnimatedVisibility(…)` in a Row's
    /// content is `RowScope.AnimatedVisibility`), a Box's hiding the outer ones (the marker's DSL rule).
    pub layout_scopes: Vec<&'static str>,
    /// What inference without writing ([`Gen::peek_ty`]) knows inside a lambda or block it looks into: the locals
    /// the block declares before its value (`remember { val a = Animatable(0f); a }`), innermost last…
    pub peek_scopes: std::cell::RefCell<Vec<HashMap<String, Ty>>>,
    /// …and the receivers (Rust type names) of the receiver lambdas it looks into (`with(scope) { … }`,
    /// `providesComputed { … }`), innermost last.
    pub peek_receivers: std::cell::RefCell<Vec<(String, Ty)>>,
    /// The element type the next `listOf(…)` written is to have (a declared type's, an outer list's: Kotlin's
    /// inference of a nested list's elements).
    pub expected_elem: Option<Ty>,
    /// The shared trait object (Rust's type text, `Rc<dyn SceneStrategy<T>>`) the elements of the next `listOf(…)`
    /// written are to be held as: a parameter's `List<SceneStrategy<T>>` given `listOf(DialogSceneStrategy())`,
    /// Kotlin's values of an interface.
    pub shared_elem: Option<String>,
    /// The value type (Rust's, Kotlin's) the next `mutableStateOf(…)` written holds: its type argument, or the declared
    /// type of the `var` delegated to it (`var job: Job? by remember { mutableStateOf(null) }`) — what
    /// `mutable_state_of!` reads to choose a thread-safe or a confined state.
    pub state_value: Option<(TokenStream, Ty)>,
    /// The type arguments of the calls being written, innermost last (`toRoute<Detail>()`): a callee that takes them
    /// only as a turbofish ([`crate::index::flags::TURBOFISH`]) is called with them.
    pub call_type_args: Vec<Vec<TokenStream>>,
    /// The same type arguments as the generator's types (a reified `entry<Profile> { … }`'s lambda typed by them).
    pub call_type_tys: Vec<Vec<Ty>>,
    /// The user's classes, objects, enum classes, interfaces and type aliases (see [`class`]).
    pub classes: class::ClassTable,
    /// The index's overlay while the translation runs (see [`crate::index::overlay`]).
    pub overlay: Option<crate::index::overlay::Installed>,
    counter: usize,
    /// What is recorded for a language server ([`crate::analysis`]), when the translation asks for it.
    pub analysis: Option<crate::analysis::Analysis>,
    /// How deep in [`Gen::quiet`] passes the generator is (nothing is recorded in one).
    pub quiet_depth: usize,
    /// The blocks and lambdas being written, innermost last: where the locals they declare can be named.
    pub ranges: Vec<Loc>,
    /// The declaration being written (where its parameters can be named).
    pub decl_range: Option<Loc>,
    /// The name of the call being resolved (`f` of `f(x)`, `g` of `a.g()`): where what it calls is recorded.
    pub callee_loc: Option<Loc>,
    /// (stage 14 WPB) What functions and control flow need: jumps out of closures, the composable calls written,
    /// the locals shared by the lambdas capturing them (see [`exceptions::Control`]).
    pub control: exceptions::Control,
}

impl<'a, 'd> Gen<'a, 'd> {
    fn new(cx: &'a Context<'a>, diags: &'d mut Diagnostics) -> Self {
        Gen {
            cx,
            diags,
            file: FileTable::default(),
            scopes: Vec::new(),
            receivers: Vec::new(),
            frames: Vec::new(),
            fun: FnState::default(),
            in_async: false,
            awaits: 0,
            last_await: None,
            inline_async: false,
            plain_twin_lambdas: Vec::new(),
            force_async: Vec::new(),
            loops: Vec::new(),
            lambdas: Vec::new(),
            call_labels: Vec::new(),
            elem_hint: None,
            owner_arg: None,
            type_arg: None,
            expected: None,
            type_arg_written: false,
            type_param: "T".to_string(),
            layout_scope_callee: None,
            layout_scopes: Vec::new(),
            peek_scopes: Default::default(),
            peek_receivers: Default::default(),
            expected_elem: None,
            shared_elem: None,
            state_value: None,
            call_type_args: Vec::new(),
            call_type_tys: Vec::new(),
            classes: Default::default(),
            overlay: None,
            counter: 0,
            analysis: if cx.analyze { Some(Default::default()) } else { None },
            quiet_depth: 0,
            ranges: Vec::new(),
            decl_range: None,
            callee_loc: None,
            control: Default::default(),
        }
    }

    pub fn span(&self, loc: Loc) -> Span {
        self.cx.spans.span_of(loc)
    }

    pub fn rt(&self) -> &TokenStream {
        &self.cx.paths.runtime
    }

    /// `::…::runtime::kt` at `span`.
    pub fn kt(&self, span: Span) -> TokenStream {
        let rt = respan(self.cx.paths.runtime.clone(), span);
        quote_spanned!(span=> #rt::kt)
    }

    /// A call that keeps something in the composition, given its identity where the spans can't (`keyed_calls`):
    /// `key(&(line, column), || call)`.
    pub fn identified(&self, ts: TokenStream, loc: Loc, keeps: bool) -> TokenStream {
        if !(self.cx.keyed_calls && keeps && self.fun.kompozable) {
            return ts;
        }
        let span = self.span(loc);
        let rt = respan(self.rt().clone(), span);
        let (line, column) = (loc.start.line, loc.start.column);
        quote_spanned!(span=> #rt::key(&(#line, #column), || #ts))
    }

    /// `.await` at `span`, counted: a suspension point of the coroutine being written. `kotlin` names the suspending
    /// call it awaits; none for a call that suspends because a lambda in it does (a twin's, a safe call's): the
    /// suspending call in that lambda is the one named.
    pub fn awaited(&mut self, kotlin: Option<&str>, loc: Loc) -> TokenStream {
        self.awaits += 1;
        if let Some(k) = kotlin {
            self.last_await = Some((k.to_string(), loc));
        }
        let span = self.span(loc);
        quote_spanned!(span=> .await)
    }

    /// (stage 14 WPB) How many fresh names have been made (a pass written again from there names its own alike).
    pub(crate) fn counter_now(&self) -> usize {
        self.counter
    }

    pub(crate) fn set_counter(&mut self, n: usize) {
        self.counter = n;
    }

    /// A fresh name for something the generator binds (`__kz_w0`).
    pub fn fresh(&mut self, what: &str, span: Span) -> proc_macro2::Ident {
        self.counter += 1;
        proc_macro2::Ident::new(&format!("__kz_{what}{}", self.counter), span)
    }

    /// An error at `loc`.
    pub fn error(&mut self, loc: Loc, message: impl Into<String>) {
        self.diags.error(loc, message);
    }

    /// An error saying `what` comes with stage 15; the tokens standing in for it (`unreachable!()`).
    pub fn later(&mut self, loc: Loc, what: &str) -> TokenStream {
        self.error(loc, format!("{what} isn't translated by KompozR's Kotlin syntax yet (it comes with stage 15)"));
        let span = self.span(loc);
        quote_spanned!(span=> ::core::unreachable!())
    }

    /// A Kotlin name as a Rust identifier (raw for a Rust keyword); an error for one Rust can't have.
    pub fn name_ident(&mut self, name: &str, loc: Loc) -> proc_macro2::Ident {
        let span = self.span(loc);
        if !valid(name) {
            self.error(loc, format!("`{name}` can't be a name in Rust"));
            return proc_macro2::Ident::new("__kz_invalid", span);
        }
        ident(name, span)
    }

    /// `f`'s result without the diagnostics it gives (what is written again later, and reported then).
    /// (stage 14 FINAL) `f` writing `e`, a value of `ty` ([`Gen::expected`]: where `e` is a call).
    pub(crate) fn expecting<R>(&mut self, e: &Expr, ty: &Ty, f: impl FnOnce(&mut Self) -> R) -> R {
        let here = (matches!(e.kind, ExprKind::Call { .. }) && !ty.is_vague()).then(|| (e.loc, ty.clone()));
        let outer = std::mem::replace(&mut self.expected, here);
        let r = f(self);
        self.expected = outer;
        r
    }

    pub fn quiet<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let n = self.diags.list.len();
        // (the jumps it writes out of lambdas land nowhere: the pass is written again, or not at all)
        let leaves = self.control.leaves.clone();
        self.quiet_depth += 1;
        let out = f(self);
        self.quiet_depth -= 1;
        self.diags.list.truncate(n);
        self.control.leaves = leaves;
        out
    }

    // ---- Scopes ------------------------------------------------------------------------------------------------

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Declares a local (in the innermost scope); its Rust name.
    pub fn declare(&mut self, kotlin: &str, rust: String, kind: LocalKind, ty: Ty) -> String {
        let depth = self.scopes.len() - 1;
        let local = Local {
            rust: rust.clone(),
            kind,
            ty,
            depth,
            kept_closure: false,
            shared_closure: false,
            mut_ref: false,
            used: None,
            lent_ref: None,
            deferred: false,
            at: None,
        };
        self.scopes.last_mut().expect("a scope").insert(kotlin.to_string(), local);
        rust
    }

    /// Declares a lambda's implicit `it` (in the innermost scope): the flag set when a use resolves to it.
    pub fn declare_it(&mut self, ty: Ty) -> std::rc::Rc<std::cell::Cell<bool>> {
        let used: std::rc::Rc<std::cell::Cell<bool>> = Default::default();
        self.declare("it", "it".into(), LocalKind::LambdaParam, ty);
        if let Some(l) = self.scopes.last_mut().and_then(|s| s.get_mut("it")) {
            l.used = Some(used.clone());
        }
        used
    }

    /// The local a name is, if it is one; a use of it from inside a lambda is recorded as a capture.
    pub fn local(&mut self, kotlin: &str) -> Option<Local> {
        let mut found = self.scopes.iter().rev().find_map(|s| s.get(kotlin)).cloned()?;
        if let Some(u) = &found.used {
            u.set(true);
        }
        // (a parameter lent by reference used from a lambda kept inside the lending one: the lending lambda's clone)
        if let Some(lent) = &found.lent_ref
            && self.frames.iter().any(|f| found.depth < f.boundary && f.kept)
        {
            lent.set(true);
            found.rust = format!("__kz_{}", found.rust);
        }
        for f in &mut self.frames {
            if found.depth < f.boundary && !f.used.contains(&found.rust) {
                f.used.push(found.rust.clone());
            }
        }
        // ((stage 14 WPB) a `var` a kept lambda captures: shared when it is assigned, `capture`)
        self.note_used(&found);
        Some(found)
    }

    /// Marks a local of the innermost scope as a parameter lent by shared reference (see `Local::lent_ref`): the flag
    /// set when a lambda kept inside uses it.
    pub fn mark_lent_ref(&mut self, kotlin: &str) -> std::rc::Rc<std::cell::Cell<bool>> {
        let flag: std::rc::Rc<std::cell::Cell<bool>> = Default::default();
        if let Some(l) = self.scopes.last_mut().and_then(|s| s.get_mut(kotlin)) {
            l.lent_ref = Some(flag.clone());
        }
        flag
    }

    /// Marks a local of the innermost scope as a `&mut` (see `Local::mut_ref`).
    pub fn mark_mut_ref(&mut self, kotlin: &str) {
        if let Some(l) = self.scopes.last_mut().and_then(|s| s.get_mut(kotlin)) {
            l.mut_ref = true;
        }
    }

    /// Whether a name is a local (without recording a capture).
    pub fn is_local(&self, kotlin: &str) -> bool {
        self.scopes.iter().any(|s| s.contains_key(kotlin))
    }

    /// Narrows a local's type in the innermost scope (a smart cast: the same Rust name, its value non-null).
    pub fn narrow(&mut self, kotlin: &str, ty: Ty) {
        if let Some(mut l) = self.scopes.iter().rev().find_map(|s| s.get(kotlin)).cloned() {
            // ((stage 14 WPD) a nullable function's value narrowed: the shared closure it holds, `Rc<dyn Fn()>`)
            let ty = match (&l.ty, ty) {
                (Ty::Nullable(inner), Ty::Fun(mut f)) if matches!(**inner, Ty::Fun(_)) => {
                    f.shared = true;
                    Ty::Fun(f)
                }
                (_, ty) => ty,
            };
            l.ty = ty;
            l.depth = self.scopes.len() - 1;
            self.scopes.last_mut().unwrap().insert(kotlin.to_string(), l);
        }
    }

    /// The innermost receiver (`this`), or the one labeled `label`; its use from a lambda is a capture (see
    /// [`use_receiver`](Self::use_receiver)).
    pub fn receiver(&mut self, label: Option<&str>) -> Option<ImplicitReceiver> {
        let r = match label {
            None => self.receivers.last().cloned(),
            Some(l) => self.receivers.iter().rev().find(|r| r.labels.iter().any(|x| x == l)).cloned(),
        }?;
        Some(self.use_receiver(&r))
    }

    /// `r` used here: from a lambda inside the one it is the receiver of, a capture; one lent by reference used from
    /// a lambda kept inside the lending one is the lending lambda's clone of it (see `ImplicitReceiver::owned_used`):
    /// a reference can't be captured by a lambda kept past the one lending it. What to write for it.
    pub fn use_receiver(&mut self, r: &ImplicitReceiver) -> ImplicitReceiver {
        let mut r = r.clone();
        if r.by_ref && self.frames.iter().any(|f| r.depth < f.boundary && f.kept) {
            r.owned_used.set(true);
            r.rust = format!("__kz_{}", r.rust);
        }
        for f in &mut self.frames {
            if r.depth < f.boundary && !f.used.contains(&r.rust) {
                f.used.push(r.rust.clone());
            }
        }
        r
    }

    // ---- The file ----------------------------------------------------------------------------------------------

    /// The file's top-level functions and properties, for the calls and reads that come before them.
    fn collect(&mut self, file: &File, own: &[&Decl], module: &[&Decl]) {
        self.collect_imports(file);
        // ((stage 14 WPB) the names declared more than once: overloads)
        self.collect_overloads(own.iter().chain(module).copied());
        self.collect_classes(own, module);
        let mut untyped = Vec::new();
        for decl in own.iter().chain(module).copied() {
            match decl {
                Decl::Function(f) => {
                    let info = self.quiet(|g| g.file_fn(f));
                    self.file.functions.entry(f.name.name.clone()).or_default().push(info);
                }
                // ((stage 14 WPB) an extension property: its getter an extension function's, see `property`)
                Decl::Property(p) if p.receiver.is_some() => self.collect_extension_property(p),
                Decl::Property(p) => {
                    if let Binding::Name(n) = &p.binding {
                        match &p.ty {
                            Some(t) => {
                                let ty = self.ty_of(t);
                                self.add_val(p, &n.name, ty);
                            }
                            None => untyped.push((p, n)),
                        }
                    }
                }
                _ => {}
            }
        }
        // A `val` without its type written has its value's, as far as the generator can tell: those its value reads
        // first, wherever they stand in the file (Kotlin's top-level properties are the file's, in any order).
        loop {
            let before = untyped.len();
            untyped.retain(|(p, n)| {
                // ((stage 14 WPB) a delegated one: what its delegate holds)
                let ty = match &p.init {
                    PropertyInit::Value(init) => self.peek_ty(init),
                    PropertyInit::Delegate(d) => self.delegate_ty(d),
                    PropertyInit::None => match &p.getter {
                        Some(Accessor { body: Some(FunctionBody::Expr(e)), .. }) => self.peek_ty(e),
                        _ => return false,
                    },
                };
                if ty.is_vague() {
                    return true;
                }
                self.add_val(p, &n.name, ty);
                false
            });
            if untyped.len() == before {
                break;
            }
        }
    }

    /// The members of classes and objects the file's imports name (Kotlin's `import a.b.Type.member`): an import
    /// whose path goes on past its first capitalized name (a class or object, packages being lowercase) imports a
    /// member of it, a companion's (`Type.Companion.member`) being its class's. The file's names that are none of its
    /// locals or its receivers' members are those members (`ScreenOffset` → `FloatingToolbarDefaults.ScreenOffset`).
    fn collect_imports(&mut self, file: &File) {
        for import in file.imports.iter().filter(|i| !i.star) {
            let segs = &import.path.segments;
            let Some(class) = segs.iter().position(|s| s.name.starts_with(|c: char| c.is_ascii_uppercase())) else {
                continue;
            };
            let Some((member, between)) = segs[class..].split_last() else { continue };
            if between.is_empty() {
                continue;
            }
            let qualifier: Vec<Ident> = between.iter().filter(|s| s.name != "Companion").cloned().collect();
            let name = import.alias.as_ref().unwrap_or(member).name.clone();
            self.file.imports.insert(name, (qualifier, member.clone()));
        }
    }

    /// A top-level `val` of type `ty`, for its reads.
    fn add_val(&mut self, p: &Property, kotlin: &str, ty: Ty) {
        let form = val_form(p, &ty);
        self.file.vals.insert(kotlin.to_string(), FileVal { rust: value_name(kotlin), ty, form });
    }

    /// What a call of a file's function needs to know of it.
    fn file_fn(&mut self, f: &Function) -> FileFn {
        let kompozable = f.is_kompozable();
        let inline = f.modifiers.has(ModifierKind::Inline);
        let mut params: Vec<PInfo> = f.params.iter().map(|p| self.param_info(p, inline)).collect();
        // ((stage 14 WPB) an inline function's lambdas may suspend in a coroutine: its twin, `jumps`)
        if self.has_inline_twin(f) {
            for l in params.iter_mut().filter_map(|p| p.lambda.as_mut()) {
                l.twin = true;
            }
        }
        let has_defaults = params.iter().any(|p| p.default != PDefault::None);
        let ret = match (&f.result, &f.body) {
            (Some(t), _) => self.ty_of(t),
            (None, Some(FunctionBody::Block(_))) | (None, None) => Ty::Unit,
            (None, Some(FunctionBody::Expr(e))) => {
                // (its parameters and receiver in scope, to tell its expression's type)
                let saved = (std::mem::take(&mut self.scopes), std::mem::take(&mut self.receivers));
                self.scopes = vec![HashMap::new()];
                for p in &f.params {
                    let ty = p.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown);
                    let ty = if p.is_vararg() { Ty::List(Box::new(ty)) } else { ty };
                    self.declare_at(&p.name, snake(&p.name.name), LocalKind::Param, ty);
                }
                if let Some(n) = f.receiver.as_ref().and_then(|r| r.simple_name()) {
                    let ty = owner_name(last_segment(&n));
                    self.receivers.push(ImplicitReceiver {
                        rust: "self".into(),
                        value: Ty::from_rust(&ty),
                        ty,
                        labels: Vec::new(),
                        depth: 0,
                        by_ref: false,
                        by_mut: false,
                        owned_used: Default::default(),
                    });
                }
                let t = self.peek_ty(e);
                (self.scopes, self.receivers) = saved;
                t
            }
        };
        FileFn {
            rust: self.fn_rust_name(f),
            params,
            kompozable,
            suspend: f.is_suspend(),
            ret,
            receiver: f.receiver.as_ref().and_then(|r| r.simple_name()).map(|n| owner_name(last_segment(&n))),
            builder: kompozable || has_defaults,
        }
    }

    fn top_decl(&mut self, decl: &Decl) -> TokenStream {
        match decl {
            Decl::Function(f) => {
                // ((stage 14 WPD) its local classes in scope, written beside it)
                let _locals = self.enter_locals(f.loc);
                let mut out = self.function(f);
                out.extend(self.class_decl(decl));
                out
            }
            Decl::Property(p) => self.top_property(p),
            // (its diagnostics are given)
            Decl::Failed { .. } => TokenStream::new(),
            // (an annotation class: Kotlin's annotations tell its compiler and tools; the generator reads the ones it
            // knows by name (`@Composable`, `@Serializable`…) and leaves the others — nothing to write)
            Decl::Class(c) if c.kind == ClassKind::Annotation => TokenStream::new(),
            Decl::Class(_) | Decl::Object(_) => self.class_decl(decl),
            Decl::TypeAlias(a) => self.type_alias(a),
        }
    }

    // ---- Functions ---------------------------------------------------------------------------------------------

    /// `pub`, `pub(crate)`, nothing; an error for `protected`.
    fn visibility(&mut self, m: &Modifiers, loc: Loc) -> TokenStream {
        let span = self.span(loc);
        match m.visibility() {
            Visibility::Public => quote_spanned!(span=> pub),
            Visibility::Internal => quote_spanned!(span=> pub(crate)),
            Visibility::Private => quote!(),
            Visibility::Protected => {
                let at = m.find(ModifierKind::Protected).unwrap_or(loc);
                self.error(at, "`protected` is for a class's members");
                quote!()
            }
        }
    }

    /// The modifiers a function may have here; the others are errors.
    fn check_fn_modifiers(&mut self, f: &Function) {
        for m in &f.modifiers.list {
            match m.kind {
                ModifierKind::Public
                | ModifierKind::Private
                | ModifierKind::Internal
                | ModifierKind::Protected
                | ModifierKind::Suspend
                | ModifierKind::Inline => {}
                // ((stage 14 WPB) see `operators`)
                ModifierKind::Infix | ModifierKind::Operator => {}
                _ => {
                    self.later(m.loc, &format!("`{}` on a function", m.kind.keyword()));
                }
            }
        }
        if f.body.is_none() {
            self.error(f.name.loc, "a function here needs a body");
        }
    }

    fn function(&mut self, f: &Function) -> TokenStream {
        self.check_fn_modifiers(f);
        let span = self.span(f.loc);
        let kompozable = f.is_kompozable();
        let inline = f.modifiers.has(ModifierKind::Inline);
        let suspend = f.is_suspend();
        if kompozable && suspend {
            self.error(f.loc, "a composable can't be `suspend`");
        }
        let info = self.quiet(|g| g.file_fn(f));
        let unit = info.ret == Ty::Unit;
        let restartable = kompozable && !inline && unit && f.result.as_ref().is_none_or(|t| self.ty_of(t) == Ty::Unit);
        let shared_result = f
            .result
            .as_ref()
            .and_then(|t| expr::Shared::of_rust(&self.quiet(|g| g.rust_type(t, types::TypePos::Return)).to_string()));
        self.fun = FnState {
            kompozable,
            nullable_result: info.ret.is_nullable(),
            shared_result,
            result: Some(info.ret.clone()),
        };
        self.scopes = vec![HashMap::new()];
        self.receivers.clear();
        self.frames.clear();
        self.loops.clear();
        self.lambdas.clear();
        self.in_async = suspend;
        self.inline_async = false;

        // The receiver of an extension function: `self` (a composable's: its hidden function's first parameter, a
        // clone of the value it extends).
        let mut self_ty = None;
        // (a drawing object's extension: its receiver `&mut self`, as Rust lends it)
        let mut self_mut = false;
        let receiver_rust = if kompozable { "__kz_this" } else { "self" };
        // (the star projections of the receiver and the parameters: the function's type parameters, its extension
        // trait impl's for the receiver's — `ThreePaneScaffoldNavigator<*>.isExpanded(…)`)
        self.collect_stars();
        if let Some(r) = &f.receiver {
            let starred = matches!(&r.kind, TypeKind::Named(segs)
                if segs.iter().any(|s| s.args.iter().any(|a| matches!(a, TypeArg::Star(_)))));
            match (r.simple_name(), r.nullable, &r.kind) {
                // (of a generic type, its arguments given: `List<String>.filteredBy` — a method of an extension trait
                // implemented for `Vec<String>`; one of its own type parameters is a generic function's, stage 14's)
                (Some(n), false, TypeKind::Named(segs))
                    if segs.iter().all(|s| s.args.is_empty())
                        || (f.type_params.is_empty() && !self.ty_of(r).is_vague())
                        || !f.type_params.is_empty()
                        || starred =>
                {
                    let rust_ty = self.rust_type(r, types::TypePos::Value);
                    let name = owner_name(last_segment(&n));
                    self_mut = !kompozable && self.lent_mut(r);
                    self.receivers.push(ImplicitReceiver {
                        rust: receiver_rust.into(),
                        ty: name.clone(),
                        value: self.ty_of(r),
                        labels: vec![f.name.name.clone()],
                        depth: 0,
                        by_ref: false,
                        by_mut: self_mut,
                        owned_used: Default::default(),
                    });
                    self.declare("this", receiver_rust.into(), LocalKind::Param, self.ty_of(r));
                    if self_mut {
                        self.mark_mut_ref("this");
                    }
                    self_ty = Some((rust_ty, name));
                }
                _ => {
                    self.later(r.loc, "an extension of a nullable or generic type");
                }
            }
        }

        // Parameters (their star projections the function's type parameters, stage 14 WPA; the receiver's collected
        // above).
        let mut params = Vec::new();
        let mut converts = Vec::new();
        for p in &f.params {
            let (tokens, convert) = self.param(p, restartable, inline);
            params.push(tokens);
            converts.extend(convert);
        }
        let ret = match &f.result {
            Some(t) if self.ty_of(t) != Ty::Unit => {
                let rt = self.rust_type(t, types::TypePos::Return);
                let s = self.span(t.loc);
                quote_spanned!(s=> -> #rt)
            }
            Some(_) => quote!(),
            None => match &f.body {
                Some(FunctionBody::Expr(e)) => match &info.ret {
                    Ty::Unit => quote!(),
                    t if t.is_vague() => {
                        self.error(
                            f.name.loc,
                            format!(
                                "write `{}`'s return type: its expression's type isn't one KompozR can tell (`fun {}(…): Type = …`)",
                                f.name.name, f.name.name
                            ),
                        );
                        quote!()
                    }
                    ty => {
                        let s = self.span(e.loc);
                        let t = self.rust_of_ty(ty, s);
                        quote_spanned!(s=> -> #t)
                    }
                },
                _ => quote!(),
            },
        };

        let body = match &f.body {
            Some(FunctionBody::Block(b)) => self.fn_block(b, unit),
            Some(FunctionBody::Expr(e)) => {
                if unit {
                    let s = self.stmt_expr(e);
                    quote!(#s;)
                } else {
                    let want = expr::Want {
                        result: true,
                        some: info.ret.is_nullable(),
                        shared: self.fun.shared_result,
                        ..Default::default()
                    };
                    self.value_want(e, want)
                }
            }
            None => quote!(),
        };
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let empty = matches!(&f.body, Some(FunctionBody::Block(b)) if b.stmts.is_empty()) && converts.is_empty();
        let body = if empty {
            quote_spanned!(span=> {})
        } else if kompozable {
            quote_spanned!(span=> {
                #[allow(unused_imports)]
                use #kt::ext::*;
                #(#converts)*
                #body
            })
        } else {
            quote_spanned!(span=> {
                #[allow(unused_imports)]
                use #kt::ext::*;
                #(#converts)*
                #rt::named_args!(#kz_types { #body })
            })
        };

        let vis = self.visibility(&f.modifiers, f.loc);
        let name = self.name_ident(&self.fn_rust_name(f), f.name.loc);
        let fn_kw = quote_spanned!(span=> fn);
        let asyncness = if suspend { quote_spanned!(span=> async) } else { quote!() };
        let mut allow = allow_attr(span);
        // (WP7: search/tooltips) Kotlin's `@Suppress("DEPRECATION")` on a function calling deprecated API: Rust's
        // deprecation lint kept quiet in it.
        if suppresses(&f.annotations, "DEPRECATION") {
            allow.extend(quote_spanned!(span=> #[allow(deprecated)]));
        }
        let has_defaults = info.params.iter().any(|p| p.default != PDefault::None);
        // (stage 14 WPA) A generic function's type parameters, bounded by their uses (an extension's: its impl's)
        let generics = self.fn_generics(f, span);

        if let Some((self_ty, owner)) = self_ty {
            let trait_name = proc_macro2::Ident::new(&format!("{}{}Ext", pascal(&owner), pascal(&f.name.name)), span);
            let trait_vis = match f.modifiers.visibility() {
                Visibility::Private => quote_spanned!(span=> pub(self)),
                _ => vis.clone(),
            };
            if kompozable {
                // A composable extension function: a hidden composable of the receiver (a clone of it) and the
                // parameters, restartable as a composable function is, reached by a method of the receiver's type
                // (`scope.Name(…)`, what a call with the receiver implicit is written as), its defaults the
                // method's. The receiver's type must be `Clone` (a scope value: `TabIndicatorScope`).
                let hidden = proc_macro2::Ident::new(&format!("__kz_ext_{}", f.name.name), span);
                let names: Vec<proc_macro2::Ident> =
                    f.params.iter().map(|p| self.name_ident(&snake(&p.name.name), p.name.loc)).collect();
                let attr =
                    if inline || !unit { self.kompozable_attr(true, span) } else { self.kompozable_attr(false, span) };
                let this = proc_macro2::Ident::new(receiver_rust, span);
                return quote_spanned! {span=>
                    #allow
                    #attr
                    #[doc(hidden)]
                    #vis #fn_kw #hidden #generics (#this: #self_ty, #(#params),*) #ret #body

                    #allow
                    #[#rt::named(#trait_vis trait #trait_name)]
                    impl #generics #self_ty {
                        #[track_caller]
                        #fn_kw #name(&self, #(#params),*) #ret {
                            let __kz_receiver = ::core::clone::Clone::clone(self);
                            #rt::kompozable_block!(#kz_types { #hidden(__kz_receiver, #(#names = #names),*) })
                        }
                    }
                };
            }
            let receiver = if self_mut { quote_spanned!(span=> &mut self) } else { quote_spanned!(span=> self) };
            // (a suspending one: an `async fn` of a public trait, no `Send` wanted — coroutines are single-threaded)
            let async_allow = if suspend { quote_spanned!(span=> #[allow(async_fn_in_trait)]) } else { quote!() };
            return quote_spanned! {span=>
                #allow
                #[#rt::named(#trait_vis trait #trait_name)]
                impl #generics #self_ty {
                    #async_allow
                    #asyncness #fn_kw #name(#receiver, #(#params),*) #ret #body
                }
            };
        }
        let attr = if kompozable {
            if inline || !unit { self.kompozable_attr(true, span) } else { self.kompozable_attr(false, span) }
        } else if has_defaults {
            quote_spanned!(span=> #[#rt::named])
        } else {
            quote!()
        };
        // ((stage 14 WPB) an `inline fun` taking lambdas: its suspending twin beside it, `jumps`)
        let twin = if self.has_inline_twin(f) { self.inline_twin(f) } else { quote!() };
        quote_spanned! {span=>
            #allow
            #attr
            #vis #asyncness #fn_kw #name #generics (#(#params),*) #ret #body
            #twin
        }
    }

    /// A function's block body: its statements, the last one its value unless it returns nothing (a `return` does
    /// otherwise).
    fn fn_block(&mut self, b: &Block, unit: bool) -> TokenStream {
        self.in_range(b.loc, |g| g.block_inner(b, if unit { stmt::Tail::Unit } else { stmt::Tail::Return }))
    }

    /// A parameter: its tokens (`#[default(d)] name: Type`), and what converts it in the body (an `impl Into<T>`
    /// made a `T`, which a restartable composable's macro does itself).
    fn param(&mut self, p: &Param, restartable: bool, inline: bool) -> (TokenStream, Option<TokenStream>) {
        let span = self.span(p.loc);
        let rust = snake(&p.name.name);
        let name = self.name_ident(&rust, p.name.loc);
        let Some(t) = &p.ty else {
            self.error(p.loc, "a parameter needs a type");
            return (quote!(), None);
        };
        // ((stage 14 WPB) `vararg xs: T`: a `Vec<T>`, Kotlin's array of the arguments, a `List<T>` in the body)
        if p.is_vararg() {
            let el = self.rust_type(t, types::TypePos::Value);
            let ty = Ty::List(Box::new(self.ty_of(t)));
            self.declare_at(&p.name, rust.clone(), LocalKind::Param, ty);
            if let Some(d) = &p.default {
                self.error(d.loc, "a `vararg` parameter has no default (it is given none)");
            }
            return (quote_spanned!(span=> #name: ::std::vec::Vec<#el>), None);
        }
        let ty = self.ty_of(t);
        let pos = if p.default.is_some() { types::TypePos::DefaultedParam } else { types::TypePos::Param };
        let rust_ty = self.rust_type(t, pos);
        // ((stage 14 FINAL) an inline function's lambda — not `noinline`, not `crossinline` —: run in place, never
        // kept, so it borrows what it uses: no `'static`; (stage 14 JUMP) an inline composable's too, as Compose's
        // `Column`'s content, which a jump may leave)
        let borrowed = inline
            && !p.modifiers.has(ModifierKind::Noinline)
            && !p.modifiers.has(ModifierKind::Crossinline)
            && matches!(self.unaliased(t).kind, TypeKind::Function(_))
            && !self.unaliased(t).nullable;
        let rust_ty = if borrowed { without_static(rust_ty) } else { rust_ty };
        // (in an inline composable's body, a reference to it: what the lambdas it hands to composables capture —
        // `Box { content() }` —, by a copy, as the macro captures every local)
        let by_ref = (borrowed && self.fun.kompozable).then(|| quote_spanned!(span=> let #name = &#name;));
        let info = self.quiet(|g| g.param_info(p, inline));
        let default = p.default.as_ref().map(|d| {
            let saved = std::mem::take(&mut self.scopes);
            self.scopes = vec![HashMap::new()];
            let v = self.value_for(d, Some(&info));
            self.scopes = saved;
            let s = self.span(d.loc);
            quote_spanned!(s=> #[default(#v)])
        });
        let unaliased = self.unaliased(t);
        let kept_closure = restartable && matches!(unaliased.kind, TypeKind::Function(_)) && !unaliased.nullable;
        let into = types::into_target(&rust_ty);
        let local_ty = ty.clone();
        self.declare_at(&p.name, rust.clone(), LocalKind::Param, local_ty);
        // (a drawing object's: a `&mut` held, reborrowed where it is given on)
        if !t.nullable && self.lent_mut(t) {
            self.mark_mut_ref(&p.name.name);
        }
        if kept_closure && let Some(l) = self.scopes.last_mut().unwrap().get_mut(&p.name.name) {
            l.kept_closure = true;
        }
        let convert = match into {
            Some(target) if !restartable => Some(quote_spanned!(span=>
                let #name: #target = ::core::convert::Into::into(#name);
            )),
            _ => by_ref,
        };
        (quote_spanned!(span=> #default #name: #rust_ty), convert)
    }

    /// What a call sees of a parameter of the file's own function.
    pub(crate) fn param_info(&mut self, p: &Param, inline: bool) -> PInfo {
        let (ty_text, lambda, option) = match &p.ty {
            // ((stage 14 WPB) a vararg: a `Vec` of its type)
            Some(t) if p.is_vararg() => {
                (format!("Vec<{}>", self.rust_type(t, types::TypePos::Value)), None, OptionForm::No)
            }
            Some(t) => {
                let pos = if p.default.is_some() { types::TypePos::DefaultedParam } else { types::TypePos::Param };
                let rust = self.rust_type(t, pos).to_string();
                let ut = self.unaliased(t);
                let lambda = ut.as_function().filter(|_| !ut.nullable).map(|ft| self.lambda_info(ft, !inline));
                // (a fun interface's value — `style: Style` — takes a lambda as its SAM conversion: the interface's
                // lambda, kept, its parameters typed for Rust to find the trait's blanket impl)
                let lambda = lambda.or_else(|| {
                    let name = t.simple_name().filter(|_| !t.nullable)?;
                    let fi = resolve::fun_interface_type(&name)?;
                    let mut info = Callee::from_item(fi).params.remove(0).lambda?;
                    (info.stored, info.typed) = (true, true);
                    Some(info)
                });
                let option = match (t.nullable, p.default.is_some()) {
                    (false, _) => OptionForm::No,
                    (true, false) => OptionForm::IntoOption,
                    (true, true) => OptionForm::Option,
                };
                // ((stage 14 WPB) `((A) -> R)?`: an `Option` of a shared closure, a lambda given made one)
                let (lambda, option, rust) = match ut.as_function().filter(|_| ut.nullable) {
                    Some(ft) => {
                        let mut info = self.lambda_info(ft, true);
                        (info.stored, info.typed, info.file_inline) = (true, true, false);
                        // (a suspending one gives a boxed future, as the shared closure does)
                        info.boxed = info.suspend;
                        // (its type as the index writes one, `Option<Rc<dyn Fn(A) -> R>>`: what the calls read)
                        let output = if info.unit_output() { String::new() } else { format!(" -> {}", info.output) };
                        let inner = if info.suspend {
                            format!("Fn({}) -> Pin<Box<dyn Future<Output = {}>>>", info.inputs.join(", "), info.output)
                        } else {
                            format!("Fn({}){output}", info.inputs.join(", "))
                        };
                        (Some(info), OptionForm::Option, format!("Option<Rc<dyn {inner}>>"))
                    }
                    None => (lambda, option, rust),
                };
                (rust, lambda, option)
            }
            None => (String::new(), None, OptionForm::No),
        };
        PInfo {
            kotlin: p.name.name.clone(),
            rust: snake(&p.name.name),
            ty: ty_text,
            default: if p.default.is_some() { PDefault::Builder } else { PDefault::None },
            vararg: p.is_vararg(),
            lambda,
            option,
        }
    }

    // ---- Top-level properties ----------------------------------------------------------------------------------

    /// `v`, of type `value_ty`, as a value of the type `declared` is written as: an interface's shared object made of a
    /// value of its own type (`val fade: AnimationSpec<Color> = tween(…)`: an `Arc<dyn AnimationSpec<Color>>`).
    pub(crate) fn as_declared(&self, v: TokenStream, value_ty: &Ty, declared: &TokenStream, span: Span) -> TokenStream {
        let d = declared.to_string().replace(' ', "");
        let own = matches!(value_ty, Ty::Named(n, _) if !matches!(n.as_str(), "Arc" | "Rc")
            && resolve::trait_object(n).is_none());
        if own && d.starts_with("::std::sync::Arc<dyn") {
            quote_spanned!(span=> ::std::sync::Arc::new(#v))
        } else if own && d.starts_with("::std::rc::Rc<dyn") {
            quote_spanned!(span=> ::std::rc::Rc::new(#v))
        } else {
            v
        }
    }

    fn top_property(&mut self, p: &Property) -> TokenStream {
        let span = self.span(p.loc);
        let Binding::Name(n) = &p.binding else {
            return self.later(p.loc, "a top-level destructuring declaration");
        };
        // ((stage 14 WPB) an extension property, a top-level `var`, a delegated one, one with accessors: `property`)
        if p.receiver.is_some() {
            return self.extension_property_decl(p);
        }
        if p.getter.is_some() || p.setter.is_some() {
            return self.top_level_accessors(p);
        }
        if p.is_var() || p.is_delegated() {
            return self.top_level_shared(p);
        }
        let init = match &p.init {
            PropertyInit::Value(e) => e,
            PropertyInit::Delegate(_) => unreachable!("a delegated property is shared"),
            PropertyInit::None => {
                self.error(n.loc, format!("`{}` needs a value", n.name));
                return quote!();
            }
        };
        self.fun = FnState::default();
        self.scopes = vec![HashMap::new()];
        self.receivers.clear();
        // (its type written, or its value's)
        let Some(val) = self.file.vals.get(&n.name).cloned() else {
            self.error(
                n.loc,
                format!("write `{}`'s type: KompozR can't tell its value's type (`val {}: Type = …`)", n.name, n.name),
            );
            return quote!();
        };
        let vis = self.visibility(&p.modifiers, p.loc);
        let name = self.name_ident(&val.rust, n.loc);
        let rt = match &p.ty {
            Some(t) => self.rust_type(t, types::TypePos::Value),
            None => self.rust_of_ty(&val.ty, self.span(n.loc)),
        };
        let allow = allow_attr(span);
        match val.form {
            ValForm::Shared { .. } => self.top_level_shared(p),
            ValForm::Accessors { .. } => self.top_level_accessors(p),
            ValForm::Const => {
                let v = self.value(init);
                quote_spanned!(span=> #allow #vis const #name: #rt = #v;)
            }
            ValForm::StrConst => {
                // ((WP7: search/tooltips) literals joined by `+`: Kotlin's constant of their text, one literal)
                let v = match literal_text(init) {
                    Some(text) if !matches!(init.kind, ExprKind::String(_)) => {
                        let mut l = proc_macro2::Literal::string(&text);
                        l.set_span(self.span(init.loc));
                        quote!(#l)
                    }
                    _ => self.expr(init).ts,
                };
                quote_spanned!(span=> #allow #vis const #name: &str = #v;)
            }
            ValForm::Static => {
                let value_ty = self.peek_ty(init);
                let v = self.value(init);
                let v = self.as_declared(v, &value_ty, &rt, span);
                let kt = self.kt(span);
                quote_spanned! {span=>
                    #allow
                    #vis static #name: #rt = {
                        #[allow(unused_imports)]
                        use #kt::ext::*;
                        #v
                    };
                }
            }
            ValForm::ThreadLocal => {
                let value_ty = self.peek_ty(init);
                let v = self.value(init);
                let v = self.as_declared(v, &value_ty, &rt, span);
                let kt = self.kt(span);
                let rtp = respan(self.rt().clone(), span);
                let kz_types = self.types_arg(span);
                quote_spanned! {span=>
                    ::std::thread_local! {
                        #allow
                        #vis static #name: #rt = {
                            #[allow(unused_imports)]
                            use #kt::ext::*;
                            #rtp::named_args!(#kz_types #v)
                        };
                    }
                }
            }
        }
    }
}

/// The overlay of the module table's Rust items: its functions, methods, properties, enum variants and types (a
/// `const`/`static`/`thread_local!` is read as before, through `kt::top_level!`, which tells them apart), and its
/// type aliases.
fn rust_overlay(module: Option<&crate::module::ModuleTable>) -> crate::index::overlay::Overlay {
    use crate::index::overlay::{intern, intern_params};
    let mut o = crate::index::overlay::Overlay::default();
    let Some(m) = module else { return o };
    for r in &m.rust {
        // (a Rust trait that is a base class of Kotlin's: stage 14 WPA)
        if let Some((open, required)) = &r.base {
            o.bases.push(crate::index::Base {
                kotlin: intern(&r.kotlin),
                rust: intern(&r.rust),
                open: *open,
                required: crate::index::overlay::intern_strs(required.iter().map(|s| intern(s)).collect()),
                value: "",
                params: &[],
                assoc: &[],
            });
            continue;
        }
        if let Some(target) = &r.alias {
            o.aliases.push((intern(&r.rust), intern(target)));
            continue;
        }
        let is_type = r.kind == crate::index::Kind::Value && r.owner == r.rust;
        if r.kind == crate::index::Kind::Value && !is_type {
            continue;
        }
        let params = r
            .params
            .iter()
            .map(|p| crate::index::Param {
                rust: intern(&p.rust),
                kotlin: intern(&p.kotlin),
                ty: intern(&p.ty),
                default: match &p.default {
                    Some(d) => crate::index::Default::Rust(intern(d)),
                    None => crate::index::Default::None,
                },
                vararg: false,
                lambda: None,
            })
            .collect();
        o.items.push(crate::index::Item {
            kotlin: intern(&r.kotlin),
            rust: intern(&r.rust),
            kind: r.kind,
            krate: "",
            module: intern(&r.module),
            owner: intern(&r.owner),
            via: intern(&r.via),
            receiver: r.receiver,
            params: intern_params(params),
            ret: intern(&r.ret),
            flags: r.flags,
        });
    }
    o
}

/// How a top-level `val` of type `ty` is held.
fn val_form(p: &Property, ty: &Ty) -> ValForm {
    // ((stage 14 WPB) a `var` or a delegated property: through a thread-local delegate, see `property`; one with
    // accessors and no value: its getter)
    if (p.getter.is_some() || p.setter.is_some()) && !p.is_delegated() {
        return ValForm::Accessors { setter: p.is_var() };
    }
    if p.is_var() || p.is_delegated() {
        return ValForm::Shared { mutable: p.is_var() };
    }
    let literal = |e: &Expr| -> bool {
        fn lit(e: &Expr) -> bool {
            match &e.kind {
                ExprKind::Number(_) | ExprKind::Bool(_) | ExprKind::Char(_) => true,
                ExprKind::Prefix { op: PrefixOp::Minus | PrefixOp::Plus, operand } => lit(operand),
                ExprKind::Paren(x) => lit(x),
                ExprKind::Binary {
                    op: BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem,
                    lhs,
                    rhs,
                } => lit(lhs) && lit(rhs),
                _ => false,
            }
        }
        lit(e)
    };
    let PropertyInit::Value(init) = &p.init else { return ValForm::ThreadLocal };
    // (made by a `const fn` of the API from literals and lambdas: `compositionLocalOf { … }`, a static, known by its
    // address as a local is)
    let made_const = match &init.kind {
        ExprKind::Call { callee, type_args: _, args, trailing_lambda: _ } => {
            matches!(&callee.kind, ExprKind::Name(n) if free_functions(&n.name).any(|i| i.has(crate::index::flags::CONST)))
                && args.iter().all(|a| literal(&a.value) || matches!(a.value.kind, ExprKind::Lambda(_)))
        }
        _ => false,
    };
    match ty {
        Ty::Int | Ty::Long | Ty::Float | Ty::Double | Ty::Bool | Ty::Char if literal(init) => ValForm::Const,
        Ty::String if literal_text(init).is_some() => ValForm::StrConst,
        _ if made_const => ValForm::Static,
        _ => ValForm::ThreadLocal,
    }
}

/// The text of a string literal without templates, or of such literals joined by `+` (`"a " + "b"`: (WP7:
/// search/tooltips) Kotlin's constant expression, a `const val`'s).
fn literal_text(e: &Expr) -> Option<String> {
    match &e.kind {
        ExprKind::String(s) => s
            .parts
            .iter()
            .map(|p| match p {
                StringPart::Text { text, .. } => Some(text.as_str()),
                StringPart::Expr { .. } => None,
            })
            .collect(),
        ExprKind::Binary { op: BinaryOp::Add, lhs, rhs } => Some(literal_text(lhs)? + &literal_text(rhs)?),
        ExprKind::Paren(x) => literal_text(x),
        _ => None,
    }
}

/// The last segment of a dotted name.
pub(crate) fn last_segment(dotted: &str) -> &str {
    dotted.rsplit('.').next().unwrap_or(dotted)
}

/// (WP7: search/tooltips) Whether the annotations hold `@Suppress("…")` of `name` (among its names).
fn suppresses(annotations: &[Annotation], name: &str) -> bool {
    annotations.iter().filter(|a| a.name.last().name == "Suppress").flat_map(|a| a.args.iter().flatten()).any(|arg| {
        matches!(&arg.value.kind, ExprKind::String(s)
            if matches!(s.parts.as_slice(), [StringPart::Text { text, .. }] if text == name))
    })
}

/// The lints generated code may trip that the Kotlin it comes from doesn't make wrong: it is written for Kotlin's
/// meaning, not Rust's style.
fn allow_attr(span: Span) -> TokenStream {
    quote_spanned! {span=>
        #[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
    }
}

/// A loop or block label, `'name`.
pub(crate) fn lifetime(name: &str, span: Span) -> TokenStream {
    let tick = proc_macro2::Punct::new('\'', proc_macro2::Spacing::Joint);
    let mut tick = proc_macro2::TokenTree::Punct(tick);
    tick.set_span(span);
    let i = proc_macro2::Ident::new(name, span);
    quote!(#tick #i)
}

/// `tokens` with every span made `span`.
/// An expression an `as` cast is written after: parenthesized when it has a binary operator at its top (`w / 2` —
/// `as` binds tighter —), else as it is (a name, a call chain, a unary operator's operand).
pub(crate) fn cast_operand(ts: TokenStream) -> TokenStream {
    use proc_macro2::TokenTree;
    let binary = ts.clone().into_iter().enumerate().any(
        |(k, t)| matches!(&t, TokenTree::Punct(p) if k > 0 && !matches!(p.as_char(), '.' | ':' | '!' | '#' | '\'')),
    );
    if binary { quote!((#ts)) } else { ts }
}

pub(crate) fn respan(tokens: TokenStream, span: Span) -> TokenStream {
    tokens
        .into_iter()
        .map(|t| match t {
            proc_macro2::TokenTree::Group(g) => {
                let mut group = proc_macro2::Group::new(g.delimiter(), respan(g.stream(), span));
                group.set_span(span);
                proc_macro2::TokenTree::Group(group)
            }
            mut t => {
                t.set_span(span);
                t
            }
        })
        .collect()
}

/// A parameter's type `impl Fn(…) -> R + 'static` without its `+ 'static`.
fn without_static(ts: TokenStream) -> TokenStream {
    let tokens: Vec<proc_macro2::TokenTree> = ts.clone().into_iter().collect();
    match tokens.as_slice() {
        [
            rest @ ..,
            proc_macro2::TokenTree::Punct(plus),
            proc_macro2::TokenTree::Punct(q),
            proc_macro2::TokenTree::Ident(s),
        ] if plus.as_char() == '+' && q.as_char() == '\'' && s == "static" => rest.iter().cloned().collect(),
        _ => ts,
    }
}
