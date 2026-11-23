//! The API index: what the generator knows of the Rust API Kotlin code calls, without compiling it — names
//! (Kotlin's ↔ Rust's), parameters in order with their types and defaults, lambdas (arity, receiver, suspending),
//! receivers, what suspends, what is `#[kompozable]`, value objects and constants, properties. `cargo xtask
//! kotlin-index` writes [`ITEMS`] (`generated.rs`, checked in, `--check` in CI) from the workspace's sources:
//! the public items of the library crates Kotlin code calls (runtime, coroutines, graphics, text, ui,
//! foundation, test).
//!
//! What it reads, and the conventions the Rust API follows for it:
//!
//! - a Kotlin name is the Rust one in camelCase (`join_to_string` → `joinToString`; PascalCase and constants as
//!   they are), unless a doc line ``Kotlin: `name(param: Type = default, …)` `` names it: a Rust function Kotlin
//!   calls by another name (`println_empty` is Kotlin's `println()`, `add_at` its `add(index, element)`, several Rust
//!   functions may have one Kotlin name, told apart by their parameters), or whose Kotlin parameters have defaults
//!   (Kotlin expressions, [`Default::Kotlin`]: the generator translates them as it does any Kotlin and passes them
//!   where a call leaves them out); `vararg` marks a parameter Kotlin passes as several arguments (an array here:
//!   `listOf(1, 2)` → `list_of([1, 2])`);
//! - a Rust parameter of type `Option<T>` takes `Some(v)` for a Kotlin value and `None` for `null`;
//! - a function pointer parameter (`fn(&KompozitionLocalAccessorScope) -> T`) takes a lambda capturing nothing, its
//!   receiver a scope as a closure's is (`compositionLocalWithComputedDefaultOf { LocalX.currentValue }`);
//! - `#[named]`/`#[kompozable]` defaults are Rust expressions ([`Default::Rust`]): such a callee is called through
//!   its builder (`#[kompozable]` does that) when a call leaves them out or names them;
//! - Kotlin's overloads of a constructor that differ by their types (`Color(Int)`, `Color(Float, Float, Float)`) are
//!   one Rust function taking `impl XArgs` ([`flags::OVERLOADED`]): one argument is given as itself, several as a
//!   tuple; each overload is an item of its own ([`Kind::Overload`]) with Kotlin's parameter names, from its
//!   `impl XArgs for (…)`'s doc line;
//! - a Kotlin property read `a.b` is `a.b()`, a write `a.set_b(v)`: [`Kind::Property`] items are the fields of
//!   `#[derive(KotlinProperties)]` types ([`flags::SETTER`] where Kotlin writes them); any other getter is a method;
//! - the methods of `kompoz_runtime::kt`'s extension traits (`KtStr`, `KtList`, …) are found through their trait
//!   ([`Item::via`]), with the receivers they are implemented for unknown here: they are Kotlin's members of
//!   `String`, `Iterable`, `Map`, … (a Rust method of the same name on the type comes first in a method call: such
//!   a call goes through the trait, `KtStr::split(&s, …)`);
//! - what a type derefs to ([`DEREFS`]: a `MeasureScope`'s `Density`) is what Kotlin's supertype is: its members are
//!   the type's too (`dp.roundToPx()` in a measure block); a trait's supertraits are its Kotlin supertypes the same
//!   way (`StyleScope` → `BorderScope`, …; a `Deref<Target = Density>` supertrait → `Density`), and so are the
//!   interfaces a type implements (`impl Painter for VectorPainter`: a trait declaring methods, implemented for a
//!   named type), Kotlin's subtyping (`with(painter) { draw(size) }` on a `VectorPainter`);
//! - a generic type's type parameters ([`GENERICS`]: `Animatable<T, V>`) tell what its members' `T` is of a value's
//!   type arguments (`value` of an `Animatable<Float, …>` a `Float`); an overload's result is its impl's associated
//!   type (`type Animatable = Animatable<f32, AnimationVector1D>` for `Animatable(0f)`);
//! - a trait implemented for the closures of one signature (`impl<F: Fn(&dyn Fn())> TextFieldDecorator for F`) is a
//!   Kotlin fun interface ([`Kind::FunInterface`]): a parameter of it (`impl T`, `Rc<dyn T>`, a Kotlin function's of
//!   its type) takes a lambda, and its SAM constructor (`TextFieldDecorator { … }`) is that lambda, beside the
//!   functions of its name taking no lambda (`Style { … }` and `Style(style1, style2)`);
//! - a type Kotlin names otherwise, a nested object (`Arrangement.Absolute`), says so in a doc line ``Kotlin:
//!   `Arrangement.Absolute` `` ([`TYPES`]);
//! - a parameter of a closure's alias (`Callback`) is written out (`Rc<dyn Fn()>`); what a `macro_rules!` of the
//!   same file makes of a type's name (`transforms!(DrawScope)`) is read as written out; a `const fn` is flagged
//!   ([`flags::CONST`]: what it makes can be a `static`);
//! - a suspending twin ([`flags::ASYNC_TWIN`]) bounding a parameter by a narrower trait than its function does
//!   (`with_style_async(style: impl SuspendingBuilderStyle, …)`, `with_style(style: impl BuilderStyle, …)`) takes
//!   Kotlin's overloads whose lambdas may suspend: the others (the types implementing only the function's trait,
//!   `ParagraphStyle`) take theirs `crossinline` ([`TWIN_BOUNDS`]).

#[rustfmt::skip]
mod generated;

pub use generated::{DEREFS, GENERICS, ITEMS, TWIN_BOUNDS, TYPES};

/// An item of the Rust API.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Item {
    /// Its Kotlin name: a function's, a method's, a property's, a constant's (`Red` of `Color.Red`); possibly
    /// qualified, as a doc line writes it (`PaddingValues.Absolute`).
    pub kotlin: &'static str,
    /// Its Rust name.
    pub rust: &'static str,
    pub kind: Kind,
    /// The crate it is in (`kompoz_foundation`).
    pub krate: &'static str,
    /// The module it is defined in, in its crate (`layout::padding`; the crates' preludes re-export most).
    pub module: &'static str,
    /// The type it belongs to: a method's, an associated function's or constant's, a property's, an enum
    /// entry's; empty for a free item, a trait's method or a blanket impl's.
    pub owner: &'static str,
    /// The trait it is reached through: an extension trait (`#[named(trait FoundationLayoutModifiers)]`, `KtStr`),
    /// a trait's method; for an overload, the `…Args` trait.
    pub via: &'static str,
    /// How a method takes its receiver (`None` for a function).
    pub receiver: Receiver,
    /// Its parameters in order (not the receiver); an enum entry's fields; an overload's tuple's elements.
    pub params: &'static [Param],
    /// What it gives (Rust's type, as text): a function's result, a property's or a value's type.
    pub ret: &'static str,
    /// [`flags`].
    pub flags: u16,
}

/// What an item is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A free function or an associated one (`Rect::from_center`, Kotlin's companion functions).
    Function,
    /// A method (a receiver of [`Item::owner`]'s type, or of any type its trait is implemented for).
    Method,
    /// A Kotlin property of a `#[derive(KotlinProperties)]` type: read `x()`, written `set_x(v)`.
    Property,
    /// A `const` or `static` of a module: a value object (`Modifier`, `LocalDensity`).
    Value,
    /// An associated constant (`Color::Red`, `Dp::Hairline`).
    Constant,
    /// An enum's entry (`TextAlign::Center`).
    Variant,
    /// One of Kotlin's overloads of a function taking them as a tuple ([`flags::OVERLOADED`]).
    Overload,
    /// A Kotlin fun interface: a trait the API implements for the closures of one signature
    /// (`impl<F: Fn(&dyn Fn())> TextFieldDecorator for F`). Its one parameter is the lambda its SAM constructor
    /// (`TextFieldDecorator { innerTextField -> … }`) takes, of type `impl Trait`; [`Item::ret`] the trait's name.
    FunInterface,
}

/// How a method takes its receiver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Receiver {
    None,
    /// `self`.
    Value,
    /// `&self`.
    Ref,
    /// `&mut self`.
    RefMut,
}

/// A parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Param {
    /// Its Rust name (an enum entry's or a tuple's field: its place).
    pub rust: &'static str,
    /// Its Kotlin name.
    pub kotlin: &'static str,
    /// Its Rust type, as text (`impl Fn(&mut DrawScope) + 'static`, `Option<String>`).
    pub ty: &'static str,
    pub default: Default,
    /// Kotlin passes it as several arguments (`vararg`), Rust takes them as one array.
    pub vararg: bool,
    /// What it is when it is a lambda (or a nullable one, or a suspending block).
    pub lambda: Option<Lambda>,
}

impl Param {
    /// Whether a Kotlin argument named `name` is for it (see [`answers_to`]).
    pub fn answers_to(&self, name: &str) -> bool {
        answers_to(self.kotlin, self.ty, name)
    }
}

/// Whether a Kotlin argument named `name` is for a parameter of Kotlin name `kotlin` and Rust type `ty`: its own
/// name, or `color` for a `brush: impl Into<Brush>` — Compose's pairs of overloads taking a `Color` or a `Brush`
/// (`DrawScope.drawRoundRect(color = …)` and `drawRoundRect(brush = …)`) are one Rust function taking either.
pub fn answers_to(kotlin: &str, ty: &str, name: &str) -> bool {
    kotlin == name || (name == "color" && kotlin == "brush" && ty.replace(' ', "").starts_with("implInto<Brush>"))
}

/// A parameter's default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Default {
    None,
    /// `#[default(expr)]`: a Rust expression, the builder's.
    Rust(&'static str),
    /// A doc line's: a Kotlin expression for the generator to translate and pass.
    Kotlin(&'static str),
}

/// A lambda parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lambda {
    pub kind: LambdaKind,
    /// Its parameters' types (Rust's, as text): its arity is their number.
    pub inputs: &'static [&'static str],
    /// What it gives (a suspending one: what its future gives).
    pub output: &'static str,
    /// The type of its receiver when its first parameter is one (a scope: `DrawScope`, `PointerInputScope`,
    /// `SemanticsPropertyReceiver`, a builder…), Kotlin's `T.() -> R`; empty otherwise.
    pub receiver: &'static str,
    /// It suspends: it gives a future (`async move { … }`), or it is one ([`LambdaKind::Future`]).
    pub suspend: bool,
    /// Its future is boxed (`Pin<Box<dyn Future<…>>>`, an alias of one: foundation's `LocalFuture`): it gives
    /// `Box::pin(async move { … })`, Kotlin's suspending function type kept as a trait object.
    pub boxed: bool,
}

/// What kind of closure a lambda is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LambdaKind {
    Fn,
    FnMut,
    FnOnce,
    /// A future itself (`LaunchedEffect`'s `effect: impl Future<Output = ()>`): Kotlin's suspending block, an
    /// `async move { … }`.
    Future,
}

/// A parameter a suspending twin ([`flags::ASYNC_TWIN`]) takes fewer types of than its function: Kotlin's overloads
/// taking the others take their lambda `crossinline`, which can't suspend (`withStyle(ParagraphStyle)`; the twin
/// `with_style_async(style: impl SuspendingBuilderStyle, …)`, where `with_style` takes `impl BuilderStyle`). The types
/// are those the API's crates implement the twin's and the function's traits for ([`TWIN_BOUNDS`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TwinBound {
    /// The function's [`Item::krate`], [`Item::module`], [`Item::owner`] and [`Item::rust`].
    pub krate: &'static str,
    pub module: &'static str,
    pub owner: &'static str,
    pub rust: &'static str,
    /// The parameter's Rust name.
    pub param: &'static str,
    /// The types the twin takes there (Rust's names): Kotlin's overloads whose lambdas may suspend.
    pub suspending: &'static [&'static str],
    /// The types only the function takes there: Kotlin's overloads taking their lambda `crossinline`.
    pub crossinline: &'static [&'static str],
}

/// What [`Item::flags`] holds.
pub mod flags {
    /// `#[kompozable]`.
    pub const KOMPOZABLE: u16 = 1;
    /// `#[kompozable(inline)]`: it runs in its caller's scope.
    pub const INLINE: u16 = 1 << 1;
    /// `#[named]` (or a method of a `#[named]` impl, or one with a defaulted parameter): it has a builder.
    pub const NAMED: u16 = 1 << 2;
    /// It suspends: an `async fn`, or one giving a future (`delay`): awaited in a coroutine.
    pub const SUSPEND: u16 = 1 << 3;
    /// It takes Kotlin's overloads as one `impl …Args` (see [`super::Kind::Overload`]).
    pub const OVERLOADED: u16 = 1 << 4;
    /// A property Kotlin writes: `set_x(v)`.
    pub const SETTER: u16 = 1 << 5;
    /// Every module on the way to it is public (else it is reached through a re-export, as a prelude's).
    pub const REACHABLE: u16 = 1 << 6;
    /// A `const fn`: what it makes can be a `static` (`compositionLocalOf { … }`).
    pub const CONST: u16 = 1 << 7;
    /// A parameter is `#[memoized]`: in a composable body `#[kompozable]` rewrites its calls, giving the closure
    /// passed to it its identity (and capturing for it), as it does a rewritten call's (`LocalX.provides_computed(…)`).
    pub const MEMOIZED: u16 = 1 << 8;
    /// Kotlin's inline function (a kt shim's: `with`, `let`, `forEach`, `repeat`, `map`…) with a suspending twin, a
    /// `#[doc(hidden)] async fn <rust>_async` taking its lambdas as async closures: a lambda given to it in a coroutine
    /// may suspend (Kotlin's inline lambdas suspend with their caller), the call then going to the twin, awaited.
    pub const ASYNC_TWIN: u16 = 1 << 9;
    /// It has a type parameter none of its parameters' types names (`to_route<T: Route>(&self)`, Kotlin's reified
    /// `toRoute<T>()`): Kotlin's type arguments of a call of it are its turbofish.
    pub const TURBOFISH: u16 = 1 << 10;
}

impl Item {
    pub fn has(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }

    /// How many parameters a call must give (those without a default).
    pub fn required(&self) -> usize {
        self.params.iter().filter(|p| p.default == Default::None).count()
    }
}

/// The items of a Kotlin name (all of them: functions, methods of any type, overloads, properties…), in the
/// index's order.
pub fn lookup(kotlin: &str) -> &'static [Item] {
    let start = ITEMS.partition_point(|i| i.kotlin < kotlin);
    let end = start + ITEMS[start..].partition_point(|i| i.kotlin == kotlin);
    &ITEMS[start..end]
}

/// The types whose members a value of Rust type `owner` has, in the order Rust's method calls try them: itself,
/// then what it derefs to, and so on (`MeasureScope` → `Density`); a trait's supertraits, breadth first (Kotlin's
/// supertypes of an interface: `StyleScope` → `CustomStyleScope`, `StyleStateScope`, … → `Density`, …).
pub fn deref_chain(owner: &str) -> Vec<&str> {
    let mut chain = vec![owner];
    let mut next = 0;
    while next < chain.len() {
        let from = chain[next];
        for (f, to) in DEREFS {
            if *f == from && !chain.contains(to) {
                chain.push(to);
            }
        }
        next += 1;
    }
    chain
}

/// The Rust type a Kotlin (qualified) type name is when Rust names it otherwise (`Arrangement.Absolute` →
/// `AbsoluteArrangement`).
pub fn rust_type(kotlin: &str) -> Option<&'static str> {
    TYPES.iter().find(|(k, _)| *k == kotlin).map(|(_, r)| *r)
}

/// Kotlin's name of a Rust type: what [`TYPES`] says, else the Rust one.
pub fn kotlin_type(rust: &str) -> &str {
    TYPES.iter().find(|(_, r)| *r == rust).map_or(rust, |(k, _)| *k)
}

/// The parameters `item`'s suspending twin takes fewer types of ([`TwinBound`]).
pub fn twin_bounds(item: &Item) -> impl Iterator<Item = &'static TwinBound> + use<'_> {
    TWIN_BOUNDS
        .iter()
        .filter(move |b| (b.krate, b.module, b.owner, b.rust) == (item.krate, item.module, item.owner, item.rust))
}

/// A generic type's type parameters, in order (`Animatable` → `["T", "V"]`).
pub fn generics(owner: &str) -> Option<&'static [&'static str]> {
    GENERICS.iter().find(|(o, _)| *o == owner).map(|(_, p)| *p)
}

/// The items of a Rust name, in the index's order (a scan).
pub fn by_rust(rust: &str) -> impl Iterator<Item = &'static Item> + use<'_> {
    ITEMS.iter().filter(move |i| i.rust == rust)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(kotlin: &str, owner: &str) -> &'static Item {
        lookup(kotlin)
            .iter()
            .find(|i| i.owner == owner)
            .unwrap_or_else(|| panic!("no `{kotlin}` of `{owner}` in the index"))
    }

    #[test]
    fn sorted_for_lookup() {
        assert!(ITEMS.windows(2).all(|w| w[0].kotlin <= w[1].kotlin));
        assert!(!lookup("Text").is_empty() || !lookup("BasicText").is_empty());
        assert!(lookup("noSuchNameAnywhere").is_empty());
    }

    #[test]
    fn what_the_generator_reads() {
        // (a composable with defaults and a receiver lambda)
        let canvas = one("Canvas", "");
        assert!(canvas.has(flags::KOMPOZABLE) && canvas.krate == "kompoz_foundation");
        let on_draw = canvas.params.iter().find(|p| p.kotlin == "onDraw").unwrap().lambda.unwrap();
        assert_eq!((on_draw.receiver, on_draw.inputs.len(), on_draw.suspend), ("DrawScope", 1, false));
        // (an extension trait's method)
        let padding = lookup("padding").iter().find(|i| i.owner == "Modifier" && i.has(flags::NAMED)).unwrap();
        assert!(!padding.via.is_empty() && padding.receiver == Receiver::Value);
        // (what suspends)
        assert!(one("delay", "").has(flags::SUSPEND));
        let effect = one("LaunchedEffect", "").params[1].lambda.unwrap();
        assert!(effect.suspend && effect.kind == LambdaKind::Future);
        let pointer = lookup("pointerInput").iter().find(|i| i.owner == "Modifier").unwrap();
        let block = pointer.params.iter().find_map(|p| p.lambda).unwrap();
        assert!(block.suspend && block.receiver == "PointerInputScope");
        // (values, constants, entries, properties)
        assert_eq!(one("Modifier", "").kind, Kind::Value);
        assert_eq!(one("Red", "Color").kind, Kind::Constant);
        assert_eq!(one("Center", "TextAlign").kind, Kind::Variant);
        let width = one("width", "Size");
        assert_eq!((width.kind, width.ret), (Kind::Property, "f32"));
        assert_eq!(one("fontScale", "Density").rust, "font_scale");
        // (Kotlin's overloads as a tuple)
        assert!(one("Color", "").has(flags::OVERLOADED));
        let overloads: Vec<&Item> = lookup("Color").iter().filter(|i| i.kind == Kind::Overload).collect();
        assert!(overloads.iter().any(|o| o.params.len() == 4 && o.params[0].kotlin == "red"));
        // (the kt shims: Kotlin's names, defaults as Kotlin writes them)
        let join = lookup("joinToString").iter().find(|i| i.via == "KtList").unwrap();
        assert_eq!(join.params[0].default, Default::Kotlin("\", \""));
        assert_eq!(join.required(), 0);
        let list_of = one("listOf", "");
        assert!(list_of.params[0].vararg);
        assert!(lookup("println").iter().any(|i| i.rust == "println_empty" && i.params.is_empty()));
        assert!(lookup("add").iter().any(|i| i.rust == "add_at" && i.params.len() == 2));
        assert!(lookup("PaddingValues").iter().any(|i| i.rust == "all"));
        // (Compose's inline functions with suspending twins; a generic type's parameters; an overload's result)
        assert!(one("buildAnnotatedString", "").has(flags::ASYNC_TWIN));
        assert!(one("withStyle", "AnnotatedStringBuilder").has(flags::ASYNC_TWIN));
        assert!(!one("withAnnotation", "AnnotatedStringBuilder").has(flags::ASYNC_TWIN));
        assert!(one("translate", "DrawScope").has(flags::ASYNC_TWIN));
        // (`withStyle(ParagraphStyle)` takes its lambda `crossinline`: the twin takes only a span style)
        let bounds: Vec<_> = twin_bounds(one("withStyle", "AnnotatedStringBuilder")).collect();
        assert_eq!(bounds.len(), 1);
        assert_eq!(
            (bounds[0].param, bounds[0].suspending, bounds[0].crossinline),
            ("style", &["SpanStyle"][..], &["ParagraphStyle"][..])
        );
        assert_eq!(twin_bounds(one("translate", "DrawScope")).count(), 0);
        assert_eq!(generics("Animatable"), Some(&["T", "V"][..]));
        // (a trait's supertraits, breadth first: a style scope's members are its scopes', a Density's too)
        let chain = deref_chain("StyleScope");
        assert!(["CustomStyleScope", "DrawStyleScope", "BorderScope", "Density"].iter().all(|t| chain.contains(t)));
        assert!(chain.iter().position(|t| *t == "DrawStyleScope") < chain.iter().position(|t| *t == "BorderScope"));
        assert!(
            lookup("Animatable")
                .iter()
                .any(|i| i.kind == Kind::Overload && i.ret == "Animatable<f32, AnimationVector1D>")
        );
    }
}
