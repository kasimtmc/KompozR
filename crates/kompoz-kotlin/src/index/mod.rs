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
//!   the type's too (`dp.roundToPx()` in a measure block);
//! - a type Kotlin names otherwise, a nested object (`Arrangement.Absolute`), says so in a doc line ``Kotlin:
//!   `Arrangement.Absolute` `` ([`TYPES`]);
//! - a parameter of a closure's alias (`Callback`) is written out (`Rc<dyn Fn()>`); what a `macro_rules!` of the
//!   same file makes of a type's name (`transforms!(DrawScope)`) is read as written out; a `const fn` is flagged
//!   ([`flags::CONST`]: what it makes can be a `static`).

#[rustfmt::skip]
mod generated;

pub use generated::{DEREFS, ITEMS, TYPES};

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
/// then what it derefs to, and so on (`MeasureScope` → `Density`).
pub fn deref_chain(owner: &str) -> Vec<&str> {
    let mut chain = vec![owner];
    while let Some((_, to)) = DEREFS.iter().find(|(from, _)| Some(from) == chain.last())
        && !chain.contains(to)
    {
        chain.push(to);
    }
    chain
}

/// The Rust type a Kotlin (qualified) type name is when Rust names it otherwise (`Arrangement.Absolute` →
/// `AbsoluteArrangement`).
pub fn rust_type(kotlin: &str) -> Option<&'static str> {
    TYPES.iter().find(|(k, _)| *k == kotlin).map(|(_, r)| *r)
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
    }
}
