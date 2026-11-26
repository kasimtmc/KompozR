//! Kotlin's classes, objects, enum classes and type aliases (stage 14 WP0): what the generator knows of them (the
//! translation's own and the module table's, as items of the index's overlay) and the Rust they become.
//!
//! | Kotlin | Rust |
//! |---|---|
//! | `data class P(val x: Int, var y: String = "")` | `#[derive(Clone, PartialEq)] pub struct P { pub x: i32, pub y: kt::KtShared<String> }`; the constructor `#[named(type)] pub fn P(x: i32, #[default(…)] y: String) -> P`; getters `x()`, setters `set_y(v)`; `copy(…)` (`#[named]`, each parameter defaulting to the value's own), `component1()`…; Kotlin's `toString()` (`P(x=1, y=)`) as `KtToString`, `Display` and `Debug` |
//! | `class C(val id: Int) { var n = 0; init { … } }` | `pub struct C(Rc<__KzC>)`: Kotlin's reference (clones share the object; `==` is identity unless `equals` is overridden); its properties in `kt::KtVal`/`kt::KtVar` cells set as the object is made — the constructor's properties, then the initializers and `init` blocks in their order (`__kz_init`), the object already there (a member function an `init` block calls works); secondary constructors `C::__kz_ctor1(…)` delegating to the primary one, then their bodies |
//! | `object O { … }` | `pub struct O {}` (`O {}` its value: a braced struct, so a function `O` can be beside it); its members `&self` methods; its state the thread's (`kt::KtSingleton`), made at first use as Kotlin's object is |
//! | `companion object { … }` of `C` | an object `CCompanion` (`C.Companion`; a named one `CName`), `C::Companion` its value; `C.f()` is `CCompanion {}.f()`; its members are in scope in `C`'s body (an implicit receiver, `__kz_companion`) |
//! | `enum class E(val n: Int) { A(1), B(2) }` | `#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)] pub enum E { A, B }` with `name()`, `ordinal()`, `E::entries()`, `E::values()`, `E::value_of(…)`, the constructor's properties as getters (`n()`: a `match` of the entries' arguments), member functions taking `self` |
//! | `interface I { fun f(): Int; val p: String }` | `pub trait I { fn f(&self) -> i32; fn p(&self) -> String; }` and what Kotlin's values of it are (`Rc<dyn I>`: see [`super::hierarchy`]); its companion an object as a class's (`I.Companion`) |
//! | `inner class C(…)` of `O` | a plain class holding `O`'s value (`__kz_outer`), made by `O`'s method `C(…)` (`o.C(…)`, `C(…)` in `O`); its members see `O`'s (`this@O`) after their own |
//! | `class C<T>(…)`, `fun <T> f(…)` | Rust's type parameters, bounded by their uses (see [`super::generics`]) |
//! | nested declarations (`class A { data class B … }`) | the outer's name before theirs (`AB`), Kotlin's `A.B` that type (the overlay's types), `B` alone inside `A` |
//! | `typealias N<T> = List<T>` | `pub type N<T> = Vec<T>;` and seen through wherever the generator reads a type (Kotlin's alias is the type) |
//! | `@Serializable` on a data class or an object | `#[derive(Route, Saveable)]` (a typed navigation route: kompoz-navigation's; no route of a property of a class or of its own serializer), its companion `serializer()` |
//! | `@Serializable` on a plain, open or sealed class | `impl Saveable` (its constructor's properties by name; a sealed class's case under `"type"`), `serializer()` |
//!
//! A class's members are translated as functions are (named arguments and defaults through `#[named]`), with the
//! class's value the implicit receiver (`self`). Inheritance — interfaces implemented, `open`, `abstract` and
//! `sealed` classes, the API's base classes — is [`super::hierarchy`]'s (stage 14 WPA), what a class's own Rust
//! holds of it written here (its base part `__kz_super`, its constructor's call first in its initializer, the trait
//! impls); an enum entry's body is an arm of its class's members' `match`es; a data class's body holds stored and
//! delegated (`by lazy`) properties, `==` and `toString()` of its constructor's properties only. Operator
//! functions, `equals`/`hashCode` overrides, accessors with backing fields and class delegates, composable members,
//! and interfaces implemented by delegation (`I by value`) aren't translated yet (an error saying so).

use super::names::{ident, is_pascal, snake};
use super::resolve::*;
use super::types::TypePos;
use super::{FnState, Gen, allow_attr, respan};
use crate::ast::*;
use crate::index::{self, Kind, Receiver, flags, overlay};
use crate::pos::Loc;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use std::collections::HashMap;

/// What a user's class-like declaration is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    Data,
    Plain,
    Object,
    Companion,
    Enum,
    Interface,
}

/// How a data class's property is compared and hashed ([`Gen::data_field_kind`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FieldKind {
    /// `==` and `Hash` of its own.
    Plain,
    /// Its bits (`kt::KtBits`): a `Float`/`Double`.
    Bits,
    /// `==` only: the class has no `Hash`.
    Compared,
    /// (stage 14 LISTS) Kotlin's `Any` (`Any?`): `==` of what it holds (`kt::KtObject`'s; a derived `==` of an
    /// `Rc<dyn KtObject>` isn't Rust's to write), no `Hash`.
    Any,
}

/// A user's class, object, enum class or interface, as the generator knows it.
#[derive(Clone, Debug)]
pub(crate) struct UserClass {
    /// Kotlin's name, qualified by the classes it is nested in (`Catalog.Entry`).
    pub kotlin: String,
    /// Its Rust type's name (`CatalogEntry`).
    pub rust: String,
    pub shape: Shape,
    /// Its companion object's Rust type.
    pub companion: Option<String>,
    /// (stage 14 WPA) What may derive from it: a final class, an open, abstract or sealed one.
    pub inherit: super::hierarchy::Inherit,
    /// Its type parameters' names (`T` of `Crate<T>`).
    pub type_params: Vec<String>,
    /// An `inner` class: its objects hold their outer class's (the Rust type of `outer`).
    pub inner: Option<String>,
    /// A `fun interface`.
    pub fun_interface: bool,
}

impl UserClass {
    /// The trait of its members a class others derive from has (`PlateOpen` of `abstract class Plate`; an
    /// interface's is itself); none for a final class.
    pub fn members_trait(&self) -> Option<String> {
        use super::hierarchy::Inherit;
        match (self.shape, self.inherit) {
            // (a sealed interface's values are its enum's: its members' trait another)
            (Shape::Interface, Inherit::Sealed) => Some(format!("{}Open", self.rust)),
            (Shape::Interface, _) => Some(self.rust.clone()),
            (_, Inherit::Open | Inherit::Abstract | Inherit::Sealed) => Some(format!("{}Open", self.rust)),
            _ => None,
        }
    }

    /// The Rust type Kotlin's type of its name is, a value's: an open or abstract class's trait object
    /// (`PlateOpen`, held as `Rc<dyn PlateOpen>`), else its own (an interface's trait, held so too).
    pub fn value_rust(&self) -> String {
        use super::hierarchy::Inherit;
        match (self.shape, self.inherit) {
            (Shape::Data | Shape::Plain, Inherit::Open | Inherit::Abstract) => format!("{}Open", self.rust),
            _ => self.rust.clone(),
        }
    }

    /// Whether Kotlin's values of its type are shared trait objects (`Rc<dyn …>`).
    pub fn is_dyn(&self) -> bool {
        use super::hierarchy::Inherit;
        (self.shape == Shape::Interface && self.inherit != Inherit::Sealed)
            || (matches!(self.shape, Shape::Data | Shape::Plain)
                && matches!(self.inherit, Inherit::Open | Inherit::Abstract))
    }
}

/// A class-like declaration found, with what encloses it.
#[derive(Clone, Debug)]
pub(crate) enum ClassAst {
    Class(Box<Class>),
    Object(Box<Object>),
}

#[derive(Clone, Debug)]
pub(crate) struct ClassDecl {
    pub info: UserClass,
    pub ast: ClassAst,
    /// The Rust name of the class it is nested in.
    pub outer: Option<String>,
    /// Declared by the translation's own text (else the module table's: known, not written).
    pub own: bool,
}

/// The tables of the user's types.
#[derive(Default)]
pub(crate) struct ClassTable {
    /// By Rust name.
    pub classes: HashMap<String, UserClass>,
    /// Kotlin's names (qualified, and a nested one's own name) → Rust names.
    pub names: HashMap<String, String>,
    /// The type aliases (the translation's and the module table's Kotlin ones) by name.
    pub aliases: HashMap<String, TypeAlias>,
    pub decls: Vec<ClassDecl>,
    /// The type arguments of the object expression being written's interface (`Source<Int>`'s).
    pub object_args: Vec<Type>,
    /// Items a class's members put beside the class (a composable member's hidden composable function).
    pub side_items: TokenStream,
    /// The generic functions of the trait being written (its extension trait's).
    pub trait_ext: Vec<TokenStream>,
    /// The where clause of the trait being written's provided methods (its type parameters' bounds).
    pub trait_where: TokenStream,
    /// (stage 14 FINAL-T) What a generic function's body of an extension trait starts with: its overrides' calls
    /// (`hierarchy::generic_dispatch`).
    pub dispatch: TokenStream,
    /// The star projections of the function's parameters being written (their places, the type parameters they are).
    pub stars: Option<Vec<(crate::pos::Loc, proc_macro2::Ident)>>,
    /// (stage 14 WPD) The local classes of the text's functions, by the function's place (its start's offset):
    /// (Kotlin's name, Rust's), what [`Gen::enter_locals`] puts in scope.
    pub locals: HashMap<usize, Vec<(&'static str, &'static str)>>,
    /// The function each local class (Rust's name) is declared in (its start's offset).
    pub local_fn: HashMap<String, usize>,
    /// The places of local declarations refused (with an error of their own: a local interface, enum, object).
    pub refused_locals: Vec<Loc>,
    /// The object expressions made local classes (`find_locals`), by their start's offset: the classes' Rust names.
    pub local_objects: HashMap<usize, String>,
}

/// The Rust name of a companion of the class `outer_rust`, named `name` (none: `Companion`).
fn companion_rust(outer_rust: &str, name: Option<&Ident>) -> String {
    format!("{outer_rust}{}", name.map_or("Companion", |n| n.name.as_str()))
}

/// The class-like declarations of `decls` (and those nested in them), with their Rust names.
pub(crate) fn find_classes(decls: &[&Decl], own: bool, out: &mut Vec<ClassDecl>) {
    for d in decls {
        find_in(d, None, own, out);
    }
}

/// (stage 14 FINAL-T) The Rust names of the standard library's types the generator reads in the API's signatures
/// (`Vec<T>` a list, `Option<T>` a nullable, …) and the prelude puts in every module: a user's top-level class of
/// one of these names is `Kt` and its name in Rust (`class Vec` → `KtVec`), so that neither its signatures are read
/// as the standard type's nor the module's own Rust loses the prelude's type.
const STD_NAMES: &[&str] = &[
    "Vec",
    "VecDeque",
    "Option",
    "String",
    "Box",
    "Rc",
    "Arc",
    "HashMap",
    "HashSet",
    "BTreeMap",
    "BTreeSet",
    "KtSet",
    "KtMap",
    "KtProgression",
    "RangeInclusive",
    "List",
    "MutableList",
];

/// The Rust name of a user's top-level type named `name` (see [`STD_NAMES`]).
pub(crate) fn top_level_rust(name: &str) -> String {
    if STD_NAMES.contains(&name) { format!("Kt{name}") } else { name.to_string() }
}

fn find_in(d: &Decl, outer: Option<(&str, &str)>, own: bool, out: &mut Vec<ClassDecl>) {
    let qualify = |name: &str| match outer {
        Some((k, r)) => (format!("{k}.{name}"), format!("{r}{name}")),
        None => (name.to_string(), top_level_rust(name)),
    };
    match d {
        Decl::Class(c) => {
            let (kotlin, rust) = qualify(&c.name.name);
            let shape = match c.kind {
                ClassKind::Enum => Shape::Enum,
                ClassKind::Interface | ClassKind::FunInterface => Shape::Interface,
                ClassKind::Annotation => return,
                ClassKind::Class if c.is_data() => Shape::Data,
                ClassKind::Class => Shape::Plain,
            };
            let companion = c.companion().map(|o| companion_rust(&rust, o.name.as_ref()));
            let inherit = super::hierarchy::Inherit::of(&c.modifiers, shape);
            let c = &api_typed_overrides(c);
            out.push(ClassDecl {
                info: UserClass {
                    kotlin: kotlin.clone(),
                    rust: rust.clone(),
                    shape,
                    companion,
                    inherit,
                    type_params: c.type_params.iter().map(|t| t.name.name.clone()).collect(),
                    inner: if c.is_inner() { outer.map(|(_, r)| r.to_string()) } else { None },
                    fun_interface: c.kind == ClassKind::FunInterface,
                },
                ast: ClassAst::Class(c.clone()),
                outer: outer.map(|(_, r)| r.to_string()),
                own,
            });
            for m in &c.body.members {
                if let Member::Decl(inner) = m {
                    find_in(inner, Some((&kotlin, &rust)), own, out);
                }
            }
        }
        Decl::Object(o) => {
            let (kotlin, rust, shape) = if o.is_companion() {
                let (ok, or) = outer.unwrap_or(("", ""));
                let name = o.name.as_ref().map_or("Companion", |n| n.name.as_str());
                (format!("{ok}.{name}"), companion_rust(or, o.name.as_ref()), Shape::Companion)
            } else {
                let Some(n) = &o.name else { return };
                let (k, r) = qualify(&n.name);
                (k, r, Shape::Object)
            };
            out.push(ClassDecl {
                info: UserClass {
                    kotlin: kotlin.clone(),
                    rust: rust.clone(),
                    shape,
                    companion: None,
                    inherit: super::hierarchy::Inherit::Final,
                    type_params: Vec::new(),
                    inner: None,
                    fun_interface: false,
                },
                ast: ClassAst::Object(o.clone()),
                outer: outer.map(|(_, r)| r.to_string()),
                own,
            });
            for m in &o.body.members {
                if let Member::Decl(inner) = m {
                    find_in(inner, Some((&kotlin, &rust)), own, out);
                }
            }
        }
        _ => {}
    }
}

/// (stage 14 FINAL-T) A class implementing an API interface, its overriding properties typed as the API's methods
/// give them where the API names a type of its own for what Kotlin's declaration writes otherwise (`override val
/// metadata: Map<String, Any>` of a `Scene`: navigation3's `Metadata`).
fn api_typed_overrides(c: &Class) -> Box<Class> {
    let mut c = Box::new(c.clone());
    let traits: Vec<String> = c
        .supers
        .iter()
        .filter_map(|s| s.ty.simple_name())
        .map(|n| index::rust_type(&n).unwrap_or_else(|| super::last_segment(&n)).to_string())
        .collect();
    let api_type = |name: &str, ty: &Type| -> Option<Type> {
        let item = index::lookup(name).iter().find(|i| {
            traits.iter().any(|t| *t == i.via) && i.kind == Kind::Method && i.params.is_empty() && i.owner.is_empty()
        })?;
        let ret = item.ret.trim();
        let own = ret.chars().all(|ch| ch.is_alphanumeric() || ch == '_');
        let written = ty.simple_name().unwrap_or_default();
        (own && ret.chars().next().is_some_and(char::is_uppercase) && super::last_segment(&written) != ret)
            .then(|| super::hierarchy::named_type(ret, ty.loc))
    };
    for m in &mut c.body.members {
        if let Member::Decl(Decl::Property(p)) = m
            && super::hierarchy::is_override(&p.modifiers)
            && let (Binding::Name(n), Some(t)) = (&p.binding, &p.ty)
            && let Some(nt) = api_type(&n.name, t)
        {
            p.ty = Some(nt);
        }
    }
    if let Some(pc) = &mut c.primary_constructor {
        for cp in &mut pc.params {
            if cp.property.is_some()
                && super::hierarchy::is_override(&cp.param.modifiers)
                && let Some(t) = &cp.param.ty
                && let Some(nt) = api_type(&cp.param.name.name, t)
            {
                cp.param.ty = Some(nt);
            }
        }
    }
    c
}

/// (stage 14 FINAL-T) Whether an initializer reads the object being made (`this`, `super`).
fn reads_object(e: &Expr) -> bool {
    let mut found = false;
    super::generics::walk_expr(e, &mut |x| {
        found |= matches!(x.kind, ExprKind::This { .. } | ExprKind::Super { .. })
            || matches!(&x.kind, ExprKind::Member { receiver, .. } if matches!(receiver.kind, ExprKind::Super { .. }));
    });
    found
}

/// Whether a declaration of the module table is seen by another file (Kotlin's `private` is the file's).
pub(crate) fn exported(d: &Decl) -> bool {
    let m = match d {
        Decl::Function(f) => &f.modifiers,
        Decl::Property(p) => &p.modifiers,
        Decl::Class(c) => &c.modifiers,
        Decl::Object(o) => &o.modifiers,
        Decl::TypeAlias(a) => &a.modifiers,
        Decl::Failed { .. } => return false,
    };
    m.visibility() != Visibility::Private
}

/// What a member function or property is written in.
#[derive(Clone)]
pub(crate) struct MemberCx {
    /// The type it is a member of (Rust's name; an open class's or an interface's trait).
    pub owner: String,
    pub shape: Shape,
    /// The companion whose members are in scope (the class's own, or the companion itself's).
    pub companion: Option<String>,
    /// An inner class's outer objects, nearest first: implicit receivers after its own (`this@Outer`).
    pub outers: Vec<Outer>,
}

/// An inner class's outer object, as its members see it: a local made at their start (`let __kz_outer =
/// self.__kz_outer();`), an implicit receiver labeled with the outer class's name.
#[derive(Clone, Debug)]
pub(crate) struct Outer {
    /// The local (`__kz_outer`, `__kz_outer2`, …).
    pub var: String,
    /// What it is read from (`self.__kz_outer()`, `__kz_outer.__kz_outer()`).
    pub from: String,
    /// The outer class's Rust name.
    pub ty: String,
    pub value: Ty,
    /// Its Kotlin name (`this@Scoreboard`).
    pub label: String,
}

impl ClassDecl {
    /// Where the declaration is.
    pub(crate) fn info_loc(&self) -> Loc {
        class_loc(&self.ast)
    }
}

impl MemberCx {
    /// The receiver a member takes: `&self`, an enum's `self` (a `Copy` value).
    fn receiver(&self, span: Span) -> TokenStream {
        if self.shape == Shape::Enum { quote_spanned!(span=> self) } else { quote_spanned!(span=> &self) }
    }
}

fn item(kotlin: &str, rust: &str, kind: Kind, owner: &str) -> index::Item {
    index::Item {
        kotlin: overlay::intern(kotlin),
        rust: overlay::intern(rust),
        kind,
        krate: "",
        module: "",
        owner: overlay::intern(owner),
        via: "",
        receiver: Receiver::None,
        params: &[],
        ret: "()",
        flags: flags::REACHABLE,
    }
}

impl Gen<'_, '_> {
    // ---- What the generator knows ------------------------------------------------------------------------------

    /// The user's type a Kotlin type name is (`Entry` inside `Catalog`, `Catalog.Entry`, `Note`).
    pub(crate) fn user_class(&self, kotlin: &str) -> Option<&UserClass> {
        // (a local class of the function being written first: Kotlin's innermost declaration)
        if let Some(rust) = overlay::local_class(kotlin) {
            return self.classes.classes.get(rust);
        }
        let rust = self.classes.names.get(kotlin)?;
        self.classes.classes.get(rust)
    }

    /// Puts the local classes of the function at `loc` in scope until the guard is dropped.
    pub(crate) fn enter_locals(&self, loc: Loc) -> overlay::LocalsEntered {
        overlay::enter_locals(self.classes.locals.get(&loc.start.offset).map_or(&[], |l| l.as_slice()))
    }

    /// Puts the local classes of the function a local class (Rust's name) is declared in in scope, so its members
    /// see them, until the guard is dropped.
    fn enter_locals_of(&self, rust: &str) -> overlay::LocalsEntered {
        let names = self.classes.local_fn.get(rust).and_then(|f| self.classes.locals.get(f));
        overlay::enter_locals(names.map_or(&[], |l| l.as_slice()))
    }

    /// Whether a declaration at `loc` is a local class hoisted beside its function (written there, not where
    /// declared), or one refused.
    pub(crate) fn is_hoisted_local(&self, loc: Loc) -> bool {
        self.classes.refused_locals.contains(&loc)
            || self
                .classes
                .decls
                .iter()
                .any(|d| d.own && class_loc(&d.ast) == loc && self.classes.local_fn.contains_key(&d.info.rust))
    }

    /// The user's type of a Rust name.
    pub(crate) fn user_class_rust(&self, rust: &str) -> Option<&UserClass> {
        self.classes.classes.get(rust)
    }

    /// The user's type an expression names, if it is a type's name (`Tally`, `Catalog.Defaults`).
    pub(crate) fn user_type_expr(&self, e: &Expr) -> Option<UserClass> {
        match &e.kind {
            ExprKind::Name(n) if is_pascal(&n.name) && !self.is_local(&n.name) => self.user_class(&n.name).cloned(),
            ExprKind::Member { receiver, safe: false, name } if is_pascal(&name.name) => {
                let outer = self.user_type_expr(receiver)?;
                self.user_class(&format!("{}.{}", outer.kotlin, name.name)).cloned()
            }
            _ => None,
        }
    }

    /// `receiver.member` where `receiver` names a user's object, or a class whose companion has `member`: the object's
    /// value (`Tally {}`, `LedgerCompanion {}`), its type's name.
    pub(crate) fn user_object_value(&self, receiver: &Expr, member: &str, span: Span) -> Option<(TokenStream, String)> {
        let rust = self.user_object_ty(receiver, member)?;
        let i = proc_macro2::Ident::new(&rust, span);
        Some((quote_spanned!(span=> #i {}), rust))
    }

    /// The object [`Gen::user_object_value`] gives: its Rust type's name.
    pub(crate) fn user_object_ty(&self, receiver: &Expr, member: &str) -> Option<String> {
        let c = self.user_type_expr(receiver)?;
        let rust = match c.shape {
            Shape::Object | Shape::Companion => c.rust.clone(),
            _ => {
                let companion = c.companion.clone()?;
                if member == "Companion" || members_of(&companion, member).next().is_some() {
                    companion
                } else {
                    return None;
                }
            }
        };
        Some(rust)
    }

    /// A user's type named as a value (`Tally`, `Ledger` of a class with a companion: Kotlin's companion): the
    /// object's value and type.
    pub(crate) fn user_name_value(&self, name: &str, span: Span) -> Option<(TokenStream, Ty)> {
        let c = self.user_class(name)?;
        let rust = match c.shape {
            Shape::Object | Shape::Companion => c.rust.clone(),
            _ => c.companion.clone()?,
        };
        let i = proc_macro2::Ident::new(&rust, span);
        // ((WPA-rest) an object of a base of the second kind, `object WrappedUUIDNavType : NavType<…>(…)`: Kotlin's
        // value of it the base's struct made of it)
        if let Some(b) = self.callbacks_base_of(&rust) {
            // ((stage 14 FINAL-T) of the base's type arguments: `NavigationEventHandler<NavigationEventInfo.None>`)
            let args = self
                .decl_of(&rust)
                .and_then(|d| self.superclass(&d))
                .map(|s| super::hierarchy::type_args_of(&s.ty))
                .unwrap_or_default();
            // ((stage 14 FINAL) a generic struct given no arguments, `KSerializer<T>`: its arguments the object's,
            // inferred)
            let n = crate::index::generics(b.value).map_or(0, |g| g.len());
            if args.is_empty() && n > 0 {
                let v = proc_macro2::Ident::new(b.value, span);
                let holes = (0..n).map(|_| quote_spanned!(span=> _));
                let v = quote_spanned!(span=> #v<#(#holes),*>);
                return Some((
                    quote_spanned!(span=> <#v as ::core::convert::From<#i>>::from(#i {})),
                    Ty::named(b.value),
                ));
            }
            let ty = self.callbacks_value_ty(b, &args);
            let v = self.rust_of_ty(&ty, span);
            return Some((quote_spanned!(span=> <#v as ::core::convert::From<#i>>::from(#i {})), ty));
        }
        Some((quote_spanned!(span=> #i {}), Ty::named(&rust)))
    }

    /// A Kotlin type alias's type for `t` (`Names` → `List<String>`, `Cells<Int>` → `List<List<Int>>`), its
    /// nullability added; none for a type that isn't one.
    pub(crate) fn expand_alias(&self, t: &Type) -> Option<Type> {
        let TypeKind::Named(segs) = &t.kind else { return None };
        let [seg] = segs.as_slice() else { return None };
        let alias = self.classes.aliases.get(&seg.name.name)?;
        let mut binds: HashMap<String, Type> = HashMap::new();
        for (p, a) in alias.type_params.iter().zip(&seg.args) {
            if let TypeArg::Type { ty, .. } = a {
                binds.insert(p.name.name.clone(), ty.clone());
            }
        }
        let mut out = substitute(&alias.ty, &binds);
        out.nullable |= t.nullable;
        Some(out)
    }

    /// The user's classes whose constructors are `#[named(type)]` functions, as `named_args!` and `#[kompozable]`
    /// are told of them (`Note(rrd), CatalogEntry(rd)`: the API index doesn't list them): their calls with named
    /// arguments or defaults left out go through their types' builders.
    pub(crate) fn types_list(&self, span: Span) -> Option<TokenStream> {
        let mut out = Vec::new();
        for d in &self.classes.decls {
            let ClassAst::Class(k) = &d.ast else { continue };
            if !matches!(d.info.shape, Shape::Data | Shape::Plain) {
                continue;
            }
            // (an abstract or sealed class has no constructor of its own, an inner one's is its outer's method)
            if matches!(d.info.inherit, super::hierarchy::Inherit::Abstract | super::hierarchy::Inherit::Sealed)
                || d.info.inner.is_some()
            {
                continue;
            }
            let letters: String = k
                .primary_constructor
                .iter()
                .flat_map(|p| &p.params)
                .map(|cp| if cp.param.default.is_some() { 'd' } else { 'r' })
                .collect();
            let name = proc_macro2::Ident::new(&d.info.rust, span);
            out.push(if letters.is_empty() {
                quote_spanned!(span=> #name())
            } else {
                let l = proc_macro2::Ident::new(&letters, span);
                quote_spanned!(span=> #name(#l))
            });
        }
        if out.is_empty() { None } else { Some(quote_spanned!(span=> types(#(#out),*))) }
    }

    /// `types(…),` opening a `named_args!`, `kompozable_block!` (none without the user's classes).
    pub(crate) fn types_arg(&self, span: Span) -> TokenStream {
        match self.types_list(span) {
            Some(t) => quote_spanned!(span=> #t,),
            None => quote!(),
        }
    }

    /// `#[kompozable]` (`inline`), telling it of the user's classes ([`Gen::types_list`]).
    pub(crate) fn kompozable_attr(&self, inline: bool, span: Span) -> TokenStream {
        let rt = respan(self.rt().clone(), span);
        let mut options = Vec::new();
        if inline {
            options.push(quote_spanned!(span=> inline));
        }
        options.extend(self.types_list(span));
        if options.is_empty() {
            quote_spanned!(span=> #[#rt::kompozable])
        } else {
            quote_spanned!(span=> #[#rt::kompozable(#(#options),*)])
        }
    }

    /// `t` with the type aliases it is (at its top) expanded: what a function type's alias (`Format`) is.
    pub(crate) fn unaliased(&self, t: &Type) -> Type {
        let mut out = t.clone();
        // (bounded: an alias naming itself through others is an error rustc reports on the `type` items)
        for _ in 0..32 {
            match self.expand_alias(&out) {
                Some(e) => out = e,
                None => break,
            }
        }
        out
    }

    /// Whether a value of `ty` is `Copy`: Kotlin's numbers and the API's small values ([`Ty::is_copy`]), and the
    /// user's enum classes and objects (derived `Copy`).
    pub(crate) fn copied(&self, ty: &Ty) -> bool {
        ty.is_copy()
            || matches!(ty, Ty::Named(n, a) if a.is_empty()
                && self.user_class_rust(n).is_some_and(|c| matches!(c.shape, Shape::Enum | Shape::Object | Shape::Companion)))
    }

    /// (stage 14 FINAL-T) A property of a function type given a lambda, `override val content: @Composable () -> Unit
    /// = { entry.Content() }`: the lambda as that type's (its parameters typed, composable as the type says), the
    /// shared closure the field holds.
    pub(crate) fn lambda_property_value(&mut self, p: &Property, init: &Expr) -> Option<TokenStream> {
        let ExprKind::Lambda(l) = &init.kind else { return None };
        let t = self.unaliased(p.ty.as_ref()?);
        let f = t.as_function().filter(|_| !t.nullable)?;
        let span = self.span(init.loc);
        // (a composable one of no parameters, a `Scene`'s `content`: its body a composable block's, as an object
        // expression's is)
        if f.params.is_empty() && f.receiver.is_none() && t.annotations.iter().any(|a| a.is_kompozable()) {
            let Binding::Name(n) = &p.binding else { return None };
            let g = Function {
                annotations: t.annotations.clone(),
                modifiers: Modifiers::default(),
                type_params: Vec::new(),
                receiver: None,
                name: n.clone(),
                params: Vec::new(),
                result: None,
                constraints: Vec::new(),
                body: Some(FunctionBody::Block(l.body.clone())),
                loc: l.loc,
            };
            let c = self.object_fn_closure_with(&g, &[], None, &[], None);
            return Some(quote_spanned!(span=> ::std::rc::Rc::new(#c)));
        }
        let mut info = self.lambda_info(f, true);
        info.typed = true;
        let c = self.lambda(l, Some(&info), None);
        Some(quote_spanned!(span=> ::std::rc::Rc::new(#c)))
    }

    /// (stage 14 FINAL-T) How a data class's property of type `ty` takes part in its `equals`/`hashCode` as Kotlin's
    /// generated ones do: compared and hashed as it is, by its bits (a `Float`/`Double`, nullable too: Kotlin's
    /// `Float.compare`), or compared only (a type without a structural hash of Rust's: no `Hash` of the class).
    /// `tparams`: the class's type parameters (hashed as they are, bounded `Hash`); `seen`: the data classes asked
    /// about already (a recursive one hashes).
    pub(crate) fn data_field_kind(&self, ty: &Ty, tparams: &[String], seen: &mut Vec<String>) -> FieldKind {
        let plain_if = |b: bool| if b { FieldKind::Plain } else { FieldKind::Compared };
        if super::hierarchy::is_any(&ty.non_null()) {
            return FieldKind::Any;
        }
        match ty {
            Ty::Int | Ty::Long | Ty::Bool | Ty::Char | Ty::String | Ty::OtherInt | Ty::Unit => FieldKind::Plain,
            Ty::Float | Ty::Double => FieldKind::Bits,
            Ty::Nullable(t) if matches!(**t, Ty::Float | Ty::Double) => FieldKind::Bits,
            Ty::Nullable(t) | Ty::List(t) => plain_if(self.data_field_kind(t, tparams, seen) == FieldKind::Plain),
            Ty::Pair(a, b) => plain_if(
                self.data_field_kind(a, tparams, seen) == FieldKind::Plain
                    && self.data_field_kind(b, tparams, seen) == FieldKind::Plain,
            ),
            Ty::Named(n, a) if a.is_empty() && tparams.contains(n) => FieldKind::Plain,
            Ty::Named(n, a) => {
                let Some(c) = self.user_class_rust(n) else { return FieldKind::Compared };
                if !a.iter().all(|t| self.data_field_kind(t, tparams, seen) == FieldKind::Plain) {
                    return FieldKind::Compared;
                }
                match c.shape {
                    Shape::Enum => FieldKind::Plain,
                    Shape::Plain => plain_if(c.inherit == super::hierarchy::Inherit::Final),
                    Shape::Data if c.inherit == super::hierarchy::Inherit::Final => {
                        if seen.contains(n) {
                            return FieldKind::Plain;
                        }
                        seen.push(n.clone());
                        let Some(d) = self.decl_of(n) else { return FieldKind::Compared };
                        let ClassAst::Class(k) = &d.ast else { return FieldKind::Compared };
                        let own = d.info.type_params.clone();
                        plain_if(
                            k.primary_constructor.iter().flat_map(|p| &p.params).filter(|p| p.property.is_some()).all(
                                |p| {
                                    p.param.ty.as_ref().is_some_and(|t| {
                                        self.data_field_kind(&self.ty_of(t), &own, seen) != FieldKind::Compared
                                    })
                                },
                            ),
                        )
                    }
                    _ => FieldKind::Compared,
                }
            }
            _ => FieldKind::Compared,
        }
    }

    // ---- Collecting --------------------------------------------------------------------------------------------

    /// The user's types (the text's own `own`, the module table's `module`): their names first (the overlay's types:
    /// what the members' types name), then their members as the overlay's items.
    pub(crate) fn collect_classes(&mut self, own: &[&Decl], module: &[&Decl]) {
        let mut decls = Vec::new();
        find_classes(own, true, &mut decls);
        self.find_locals(own, &mut decls);
        let own_names: Vec<String> = decls.iter().map(|d| d.info.rust.clone()).collect();
        let mut theirs = Vec::new();
        find_classes(module, false, &mut theirs);
        decls.extend(theirs.into_iter().filter(|d| !own_names.contains(&d.info.rust)));
        for d in own.iter().chain(module) {
            if let Decl::TypeAlias(a) = d {
                self.classes.aliases.entry(a.name.name.clone()).or_insert_with(|| (**a).clone());
            }
        }
        let mut types = Vec::new();
        for d in &decls {
            self.classes.classes.insert(d.info.rust.clone(), d.info.clone());
            // (a local class's names are its function's: `enter_locals`)
            if self.classes.local_fn.contains_key(&d.info.rust) {
                continue;
            }
            self.classes.names.entry(d.info.kotlin.clone()).or_insert(d.info.rust.clone());
            if d.outer.is_some() {
                types.push((overlay::intern(&d.info.kotlin), overlay::intern(&d.info.rust)));
            }
        }
        // (a nested one's own name, where no other type has it: the top-level ones' first)
        for d in decls.iter().filter(|d| d.outer.is_some() && !self.classes.local_fn.contains_key(&d.info.rust)) {
            let short = d.info.kotlin.rsplit('.').next().unwrap_or(&d.info.kotlin).to_string();
            self.classes.names.entry(short).or_insert(d.info.rust.clone());
        }
        self.classes.decls = decls.clone();
        let mut base = overlay::current().map(|o| (*o).clone()).unwrap_or_default();
        base.types.extend(types);
        base.locals.extend(self.classes.local_fn.keys().map(|r| overlay::intern(r)));
        // (the traits whose values are shared objects: interfaces', open and abstract classes')
        base.interfaces.extend(decls.iter().filter(|d| d.info.is_dyn()).map(|d| overlay::intern(&d.info.value_rust())));
        // (what a type's members are: its supertypes' too)
        for d in &decls {
            let own = overlay::intern(&d.info.rust);
            if let Some(t) = d.info.members_trait()
                && t != d.info.rust
            {
                base.supertypes.push((own, overlay::intern(&t)));
            }
            for s in self.supers_of(d) {
                let t = match &s.kind {
                    super::hierarchy::SuperKind::Interface(r) => r.clone(),
                    super::hierarchy::SuperKind::Class(r) => {
                        self.user_class_rust(r).and_then(|c| c.members_trait()).unwrap_or_else(|| r.clone())
                    }
                    super::hierarchy::SuperKind::Base(b) => b.rust.to_string(),
                    super::hierarchy::SuperKind::Api(t) => t.clone(),
                };
                base.supertypes.push((own, overlay::intern(&t)));
                if let Some(mt) = d.info.members_trait()
                    && mt != d.info.rust
                {
                    base.supertypes.push((overlay::intern(&mt), overlay::intern(&t)));
                }
            }
            if !d.info.type_params.is_empty() || d.info.inner.is_some() {
                let mut ps: Vec<&'static str> = Vec::new();
                if let Some(o) = &d.info.inner
                    && let Some(oc) = self.user_class_rust(o)
                {
                    ps.extend(oc.type_params.iter().map(|t| overlay::intern(t)));
                }
                ps.extend(d.info.type_params.iter().map(|t| overlay::intern(t)));
                if !ps.is_empty() {
                    let ps = overlay::intern_strs(ps);
                    base.generics.push((own, ps));
                    if let Some(mt) = d.info.members_trait()
                        && mt != d.info.rust
                    {
                        base.generics.push((overlay::intern(&mt), ps));
                    }
                }
            }
        }
        self.overlay = Some(overlay::install(base.clone()));
        self.classes.decls = decls.clone();
        let mut items = Vec::new();
        for d in &decls {
            items.extend(self.quiet(|g| g.class_items(d)));
        }
        // ((stage 14 WPB) again with them known: a member's type told by another member's — `fun both() = put(2) +
        // put("b")`)
        let mut first = base.clone();
        first.items.splice(0..0, items);
        self.overlay = None;
        self.overlay = Some(overlay::install(first));
        self.classes.decls = decls.clone();
        let mut items = Vec::new();
        for d in &decls {
            items.extend(self.quiet(|g| g.class_items(d)));
        }
        // (again, the first pass's items seen: a member's type told by another class's constructor or member,
        // `val items = MutableStateFlow(listOf(Todo(1, "Yaz")))`)
        let mut first = base.clone();
        first.items.splice(0..0, items);
        self.overlay = None;
        self.overlay = Some(overlay::install(first));
        let mut items = Vec::new();
        for d in &decls {
            items.extend(self.quiet(|g| g.class_items(d)));
        }
        base.items.splice(0..0, items);
        self.overlay = None;
        self.overlay = Some(overlay::install(base));
        self.classes.decls = decls;
    }

    /// (stage 14 WPD) The local classes of the text's functions (declared in their bodies' statements): hoisted
    /// beside their function, private, their Rust names the function's (`StateFlowSampleExampleState`; several
    /// functions may have one of a name), their Kotlin names seen only in their function. They capture nothing of
    /// it (what Kotlin's would capture is rustc's unknown name). An object expression with members of its own
    /// (`remember { object : ViewModel() { fun refresh() … } }`, its members seen through its value's type, Kotlin's
    /// anonymous one) is one too, when it uses none of the function's locals (else it stays an object expression,
    /// its value its supertype's).
    fn find_locals(&mut self, own: &[&Decl], out: &mut Vec<ClassDecl>) {
        for d in own {
            let Decl::Function(f) = d else { continue };
            // ((stage 14 FINAL-T) an expression body's object expressions too: `fun box(): Box = object : Box() {
            // … }`)
            let stmts = match &f.body {
                Some(FunctionBody::Block(b)) => b.stmts.as_slice(),
                Some(FunctionBody::Expr(_)) => &[],
                None => continue,
            };
            for s in stmts {
                let StmtKind::Decl(local) = &s.kind else { continue };
                let mut local = local.clone();
                let (modifiers, loc) = match &mut local {
                    Decl::Class(c) if matches!(c.kind, ClassKind::Interface | ClassKind::FunInterface) => {
                        self.error(c.name.loc, "an interface can't be local (Kotlin's rule)");
                        self.classes.refused_locals.push(c.loc);
                        continue;
                    }
                    Decl::Class(c) if c.kind == ClassKind::Enum => {
                        self.error(c.name.loc, "an enum class can't be local (Kotlin's rule)");
                        self.classes.refused_locals.push(c.loc);
                        continue;
                    }
                    Decl::Class(c) if c.kind == ClassKind::Class => (&mut c.modifiers, c.loc),
                    Decl::Object(o) if o.name.is_some() => {
                        self.error(
                            o.loc,
                            "an object declaration can't be local (Kotlin's rule): an object expression can",
                        );
                        self.classes.refused_locals.push(o.loc);
                        continue;
                    }
                    _ => continue,
                };
                if let Some(m) = modifiers.list.iter().find(|m| {
                    matches!(
                        m.kind,
                        ModifierKind::Public | ModifierKind::Private | ModifierKind::Internal | ModifierKind::Protected
                    )
                }) {
                    self.error(m.loc, "a local class has no visibility (Kotlin's rule)");
                }
                modifiers.list.push(Modifier { kind: ModifierKind::Private, loc });
                self.hoist_local(&local, f, out);
            }
            // (its object expressions with members of their own)
            let mut objects = Vec::new();
            super::generics::walk_body(f.body.as_ref(), &mut |e| {
                if let ExprKind::Object { supers, body } = &e.kind {
                    objects.push((supers.clone(), body.clone(), e.loc));
                }
            });
            let locals = local_names(f);
            for (supers, body, loc) in objects {
                if !has_own_members(&body) || uses_names(&body, &locals) {
                    continue;
                }
                let class = Class {
                    annotations: Vec::new(),
                    modifiers: Modifiers { list: vec![Modifier { kind: ModifierKind::Private, loc }] },
                    kind: ClassKind::Class,
                    name: Ident { name: "Object".into(), backticked: false, loc },
                    type_params: Vec::new(),
                    primary_constructor: None,
                    supers,
                    constraints: Vec::new(),
                    enum_entries: Vec::new(),
                    body,
                    loc,
                };
                let start = self.hoist_local(&Decl::Class(Box::new(class)), f, out);
                // (no name of Kotlin's: its value's type only)
                let rust = out[start].info.rust.clone();
                out[start].info.kotlin = "<object>".into();
                if let Some(names) = self.classes.locals.get_mut(&f.loc.start.offset) {
                    names.retain(|(_, r)| *r != rust);
                }
                self.classes.local_objects.insert(loc.start.offset, rust);
            }
        }
    }

    /// A function's local class (or object expression made one) beside it: its declarations found, their Rust names
    /// the function's, their Kotlin names its; where they start in `out`.
    fn hoist_local(&mut self, local: &Decl, f: &Function, out: &mut Vec<ClassDecl>) -> usize {
        let key = f.loc.start.offset;
        let start = out.len();
        find_in(local, None, true, out);
        // (its Rust name the function's; one more of that name a number)
        let mut prefix = super::names::pascal(&f.name.name);
        let root = out[start].info.rust.clone();
        let taken = |p: &str, out: &[ClassDecl]| out.iter().any(|o| o.info.rust == format!("{p}{root}"));
        if taken(&prefix, &out[..start]) {
            let mut n = 2;
            while taken(&format!("{prefix}{n}"), &out[..start]) {
                n += 1;
            }
            prefix = format!("{prefix}{n}");
        }
        let rename = |r: &mut String| *r = format!("{prefix}{r}");
        for c in &mut out[start..] {
            rename(&mut c.info.rust);
            for r in [c.info.companion.as_mut(), c.info.inner.as_mut(), c.outer.as_mut()].into_iter().flatten() {
                rename(r);
            }
            let names = self.classes.locals.entry(key).or_default();
            names.push((overlay::intern(&c.info.kotlin), overlay::intern(&c.info.rust)));
            self.classes.local_fn.insert(c.info.rust.clone(), key);
        }
        start
    }

    /// The local class an object expression at `loc` was made (its Rust name).
    pub(crate) fn local_object(&self, loc: Loc) -> Option<String> {
        self.classes.local_objects.get(&loc.start.offset).cloned()
    }

    /// The overlay's items of a declaration: what a use of it finds.
    fn class_items(&mut self, d: &ClassDecl) -> Vec<index::Item> {
        let _locals = self.enter_locals_of(&d.info.rust);
        let mut out = Vec::new();
        let c = &d.info;
        // (an open, abstract or sealed class's members are its trait's: its subclasses' through it)
        let mowner = c.members_trait().unwrap_or_else(|| c.rust.clone());
        let cx = MemberCx {
            owner: mowner.clone(),
            shape: c.shape,
            companion: c.companion.clone(),
            outers: self.outers_of(c),
        };
        let members = match &d.ast {
            ClassAst::Class(k) => &k.body.members,
            ClassAst::Object(o) => &o.body.members,
        };
        let generic = |name: &str| -> String {
            let mut ps: Vec<String> = Vec::new();
            if let Some(o) = &c.inner
                && let Some(oc) = self.user_class_rust(o)
            {
                ps.extend(oc.type_params.iter().cloned());
            }
            ps.extend(c.type_params.iter().cloned());
            if ps.is_empty() { name.to_string() } else { format!("{name}<{}>", ps.join(", ")) }
        };
        let self_ty = generic(&c.rust);
        // (the type itself: a value the user's code names)
        let mut ty = item(&c.rust, &c.rust, Kind::Value, &c.rust);
        ty.ret = overlay::intern(&c.rust);
        ty.flags = 0;
        out.push(ty);
        if let ClassAst::Class(k) = &d.ast {
            match c.shape {
                Shape::Data | Shape::Plain => {
                    let params: Vec<Param> = k
                        .primary_constructor
                        .as_ref()
                        .map(|p| p.params.iter().map(|cp| cp.param.clone()).collect())
                        .unwrap_or_default();
                    // ((stage 14 WPB) an exception class's `message` and `cause`)
                    out.extend(self.exception_items(k, &c.rust));
                    // ((WPA-rest) an open exception class's members' trait has them too)
                    if let Some(t) = c.members_trait() {
                        out.extend(self.exception_items(k, &t));
                    }
                    let makes = matches!(c.inherit, super::hierarchy::Inherit::Final | super::hierarchy::Inherit::Open);
                    if let Some(outer) = c.inner.clone().filter(|_| makes) {
                        // (an inner class's constructor: a method of its outer class, `outer.Inner(…)`)
                        let mut ctor = item(&k.name.name, &k.name.name, Kind::Method, &outer);
                        let enum_outer = self.user_class_rust(&outer).is_some_and(|o| o.shape == Shape::Enum);
                        ctor.receiver = if enum_outer { Receiver::Value } else { Receiver::Ref };
                        ctor.params = self.item_params(&params, None);
                        ctor.ret = overlay::intern(&self_ty);
                        ctor.flags |= flags::NAMED;
                        out.push(ctor);
                    } else if makes {
                        let mut ctor = item(&k.name.name, &c.rust, Kind::Function, "");
                        ctor.params = self.item_params(&params, None);
                        ctor.ret = overlay::intern(&self_ty);
                        ctor.flags |= flags::NAMED;
                        if !c.type_params.is_empty() {
                            ctor.flags |= flags::TURBOFISH;
                        }
                        out.push(ctor);
                        if c.kotlin != k.name.name {
                            let mut q = ctor;
                            q.kotlin = overlay::intern(&c.kotlin);
                            out.push(q);
                        }
                    }
                    for (n, m) in members.iter().enumerate().filter_map(|(n, m)| match m {
                        Member::Constructor(s) => Some((n, s)),
                        _ => None,
                    }) {
                        let mut s = item(&k.name.name, &format!("__kz_ctor{n}"), Kind::Function, &c.rust);
                        s.params = self.item_params(&m.params, None);
                        s.ret = overlay::intern(&c.rust);
                        out.push(s);
                    }
                    if let Some(p) = &k.primary_constructor {
                        for cp in p.params.iter().filter(|cp| cp.property.is_some()) {
                            let ty = cp.param.ty.as_ref().map(|t| self.value_type_text(t)).unwrap_or_default();
                            out.push(self.property_item(
                                &cp.param.name.name,
                                &mowner,
                                &ty,
                                cp.property == Some(Mutability::Var),
                            ));
                        }
                    }
                    if c.shape == Shape::Data {
                        let props: Vec<&ConstructorParam> = k
                            .primary_constructor
                            .iter()
                            .flat_map(|p| &p.params)
                            .filter(|cp| cp.property.is_some())
                            .collect();
                        let mut copy = item("copy", "copy", Kind::Method, &c.rust);
                        copy.receiver = Receiver::Ref;
                        let mut ps: Vec<Param> = props.iter().map(|cp| cp.param.clone()).collect();
                        for p in &mut ps {
                            p.default = Some(Expr { kind: ExprKind::Null, loc: p.loc });
                        }
                        copy.params = self.item_params(&ps, None);
                        copy.ret = overlay::intern(&c.rust);
                        copy.flags |= flags::NAMED;
                        out.push(copy);
                        for (k, cp) in props.iter().enumerate() {
                            let name = format!("component{}", k + 1);
                            let mut it = item(&name, &name, Kind::Method, &c.rust);
                            it.receiver = Receiver::Ref;
                            it.ret = overlay::intern(
                                &cp.param.ty.as_ref().map(|t| self.value_type_text(t)).unwrap_or_default(),
                            );
                            out.push(it);
                        }
                    }
                }
                Shape::Enum => {
                    for e in &k.enum_entries {
                        let mut it = item(&e.name.name, &e.name.name, Kind::Variant, &c.rust);
                        it.ret = overlay::intern(&c.rust);
                        out.push(it);
                    }
                    for (name, ret) in [("name", "String"), ("ordinal", "i32")] {
                        let mut it = item(name, name, Kind::Property, &c.rust);
                        it.receiver = Receiver::Value;
                        it.ret = overlay::intern(ret);
                        out.push(it);
                    }
                    let list = format!("Vec<{}>", c.rust);
                    let mut entries = item("entries", "entries", Kind::Function, &c.rust);
                    entries.ret = overlay::intern(&list);
                    out.push(entries);
                    let mut values = item("values", "values", Kind::Function, &c.rust);
                    values.ret = overlay::intern(&list);
                    out.push(values);
                    // (an entry's own functions: methods only its body calls)
                    for e in &k.enum_entries {
                        for m in e.body.iter().flat_map(|b| &b.members) {
                            let Member::Decl(Decl::Function(f)) = m else { continue };
                            if super::hierarchy::is_override(&f.modifiers) || f.receiver.is_some() {
                                continue;
                            }
                            let rust = entry_own_name(&e.name.name, &f.name.name);
                            let mut it = item(&f.name.name, &rust, Kind::Method, &c.rust);
                            it.receiver = Receiver::Value;
                            it.params = self.item_params(&f.params, None);
                            let mut g = (**f).clone();
                            g.name.name = rust.clone();
                            it.ret = overlay::intern(&self.member_ret_text(&g, &cx));
                            out.push(it);
                        }
                    }
                    let mut value_of = item("valueOf", "value_of", Kind::Function, &c.rust);
                    value_of.params = overlay::intern_params(vec![index::Param {
                        rust: "value",
                        kotlin: "value",
                        ty: "impl Into<String>",
                        default: index::Default::None,
                        vararg: false,
                        lambda: None,
                    }]);
                    value_of.ret = overlay::intern(&c.rust);
                    out.push(value_of);
                    if let Some(p) = &k.primary_constructor {
                        for cp in p.params.iter().filter(|cp| cp.property.is_some()) {
                            let ty = cp.param.ty.as_ref().map(|t| self.value_type_text(t)).unwrap_or_default();
                            let mut it = self.property_item(&cp.param.name.name, &c.rust, &ty, false);
                            it.receiver = Receiver::Value;
                            out.push(it);
                        }
                    }
                }
                _ => {}
            }
        }
        // (the members: functions, properties)
        for m in members {
            let Member::Decl(decl) = m else { continue };
            match decl {
                Decl::Function(f) if f.receiver.is_none() => {
                    let rust = self.member_fn_rust(&c.rust, f);
                    let mut it = item(&f.name.name, &rust, Kind::Method, &mowner);
                    it.receiver = if c.shape == Shape::Enum { Receiver::Value } else { Receiver::Ref };
                    // (a trait's method takes its values as they are: no `impl Into<…>`, which a trait object can't)
                    let in_trait = (mowner != c.rust || c.shape == Shape::Interface).then_some(&cx);
                    it.params = self.item_params(&f.params, in_trait);
                    it.ret = overlay::intern(&self.member_ret_text(f, &cx));
                    if f.params.iter().any(|p| p.default.is_some()) {
                        it.flags |= flags::NAMED;
                    }
                    if f.is_suspend() {
                        it.flags |= flags::SUSPEND;
                    }
                    out.push(it);
                }
                Decl::Property(p) if p.receiver.is_none() => {
                    let Binding::Name(n) = &p.binding else { continue };
                    let ty = self.member_property_ty(p, &cx);
                    let text = self.rust_of_ty(&ty, Span::call_site()).to_string();
                    let text = match &p.ty {
                        Some(t) => self.value_type_text(t),
                        None => text,
                    };
                    let mut it = self.property_item(&n.name, &mowner, &text, p.is_var());
                    if c.shape == Shape::Enum {
                        it.receiver = Receiver::Value;
                    }
                    out.push(it);
                }
                _ => {}
            }
        }
        out
    }

    fn property_item(&mut self, kotlin: &str, owner: &str, ty: &str, var: bool) -> index::Item {
        let mut it = item(kotlin, &snake(kotlin), Kind::Property, owner);
        it.receiver = Receiver::Ref;
        it.ret = overlay::intern(ty);
        if var {
            it.flags |= flags::SETTER;
        }
        it
    }

    /// A type as the index's text for a value of it (Rust's, as `quote` prints it).
    pub(crate) fn value_type_text(&mut self, t: &Type) -> String {
        self.quiet(|g| g.rust_type(t, TypePos::Value)).to_string()
    }

    /// Parameters as the index's.
    fn item_params(&mut self, params: &[Param], in_trait: Option<&MemberCx>) -> &'static [index::Param] {
        let mut out = Vec::new();
        for p in params {
            let Some(t) = &p.ty else { continue };
            let pos = match (in_trait, p.default.is_some()) {
                (Some(_), _) => TypePos::Value,
                (None, true) => TypePos::DefaultedParam,
                (None, false) => TypePos::Param,
            };
            let ty = self.quiet(|g| g.rust_type(t, pos)).to_string();
            let ut = self.unaliased(t);
            let lambda = ut.as_function().filter(|_| !ut.nullable).map(|ft| {
                let l = self.quiet(|g| g.lambda_info(ft, true));
                index::Lambda {
                    kind: index::LambdaKind::Fn,
                    inputs: overlay::intern_strs(l.inputs.iter().map(|s| overlay::intern(s)).collect()),
                    output: overlay::intern(&l.output),
                    receiver: overlay::intern(l.receiver.as_deref().unwrap_or("")),
                    suspend: l.suspend,
                    boxed: false,
                }
            });
            out.push(index::Param {
                rust: overlay::intern(&snake(&p.name.name)),
                kotlin: overlay::intern(&p.name.name),
                ty: overlay::intern(&ty),
                default: if p.default.is_some() { index::Default::Rust("_") } else { index::Default::None },
                vararg: p.is_vararg(),
                lambda,
            });
        }
        overlay::intern_params(out)
    }

    /// Opens a member's scope: its receivers (the value `self`, the companion in scope, an inner class's outer
    /// objects), as `MemberCx` says.
    pub(crate) fn enter_member(&mut self, cx: &MemberCx, label: &str, with_self: bool) {
        self.scopes = vec![HashMap::new()];
        self.receivers.clear();
        self.frames.clear();
        self.loops.clear();
        self.lambdas.clear();
        for o in cx.outers.iter().rev() {
            self.receivers.push(ImplicitReceiver {
                rust: o.var.clone(),
                ty: o.ty.clone(),
                value: o.value.clone(),
                labels: vec![o.label.clone()],
                depth: 0,
                by_ref: false,
                by_mut: false,
                owned_used: Default::default(),
            });
        }
        if let Some(c) = &cx.companion {
            self.receivers.push(ImplicitReceiver {
                rust: "__kz_companion".into(),
                ty: c.clone(),
                value: Ty::named(c),
                labels: vec!["Companion".into()],
                depth: 0,
                by_ref: false,
                by_mut: false,
                owned_used: Default::default(),
            });
        }
        if with_self {
            // (a generic class's value of its own type parameters: what its members' `T` is inside it)
            let params: Vec<Ty> = index::generics(&cx.owner).unwrap_or_default().iter().map(|p| Ty::named(p)).collect();
            let value = Ty::Named(cx.owner.clone(), params);
            // ((stage 14 M3NODE) labeled with the class's name too: `this@Node` in a member extension, whose own
            // `this` is its receiver)
            let mut labels = vec![label.to_string()];
            if let Some(c) = self.user_class_rust(&cx.owner) {
                let k = c.kotlin.rsplit('.').next().unwrap_or(&c.kotlin).to_string();
                if k != label {
                    labels.push(k);
                }
            }
            self.receivers.push(ImplicitReceiver {
                rust: "self".into(),
                ty: cx.owner.clone(),
                value: value.clone(),
                labels,
                depth: 0,
                by_ref: false,
                by_mut: false,
                owned_used: Default::default(),
            });
            self.declare("this", "self".into(), LocalKind::Param, value);
        }
    }

    /// (stage 14 M3NODE) The result type of the API's method a member function overrides, where it is a boxed trait
    /// object (`Box<dyn ModifierNode>` of `IndicationNodeFactory.create`): Rust's text.
    pub(crate) fn api_boxed_result(&mut self, cx: &MemberCx, f: &Function) -> Option<String> {
        if !super::hierarchy::is_override(&f.modifiers) {
            return None;
        }
        let d = self.decl_of(&cx.owner)?;
        let ext = usize::from(f.receiver.is_some());
        for a in self.ancestors(&d) {
            let t = match &a.kind {
                super::hierarchy::SuperKind::Api(t) => t.clone(),
                super::hierarchy::SuperKind::Base(b) => b.rust.to_string(),
                _ => continue,
            };
            for tr in super::object_expr::api_chain(&t) {
                let found = index::lookup(&f.name.name)
                    .iter()
                    .find(|i| i.via == tr && i.kind == index::Kind::Method && i.params.len() == f.params.len() + ext);
                if let Some(i) = found {
                    let r = i.ret.trim();
                    return (r.starts_with("Box<dyn ") && !r.contains("Fn")).then(|| r.to_string());
                }
            }
        }
        None
    }

    /// A member function's result type as the index's text.
    pub(crate) fn member_ret_text(&mut self, f: &Function, cx: &MemberCx) -> String {
        if let Some(t) = &f.result {
            return self.quiet(|g| g.rust_type(t, TypePos::Return)).to_string();
        }
        match &f.body {
            Some(FunctionBody::Expr(e)) => {
                let ty = self.member_expr_ty(f, e, cx);
                // (an override's: the overridden member's, where its expression doesn't tell — `override fun next()
                // = value` of a `T`)
                if (ty.is_vague() || matches!(&ty, Ty::Named(n, a) if a.is_empty() && n.len() == 1))
                    && super::hierarchy::is_override(&f.modifiers)
                    && let Some(t) = self.overridden_result(&cx.owner, f)
                {
                    return t;
                }
                if ty == Ty::Unit { "()".into() } else { self.rust_of_ty(&ty, Span::call_site()).to_string() }
            }
            _ => "()".into(),
        }
    }

    /// The type of a member function's expression body, its parameters and receivers in scope.
    fn member_expr_ty(&mut self, f: &Function, e: &Expr, cx: &MemberCx) -> Ty {
        let saved = (std::mem::take(&mut self.scopes), std::mem::take(&mut self.receivers));
        self.enter_member(cx, &f.name.name, true);
        for p in &f.params {
            let ty = p.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown);
            self.declare_at(&p.name, snake(&p.name.name), LocalKind::Param, ty);
        }
        let t = self.peek_ty(e);
        (self.scopes, self.receivers) = saved;
        t
    }

    /// A member property's type: written, or its initializer's (the class's other members and the constructor's
    /// parameters in scope).
    pub(crate) fn member_property_ty(&mut self, p: &Property, cx: &MemberCx) -> Ty {
        if let Some(t) = &p.ty {
            return self.ty_of(t);
        }
        let e = match (&p.init, &p.getter) {
            // ((stage 14 WPB) a getter's expression before an initializer: the property's value is the getter's)
            (_, Some(Accessor { body: Some(FunctionBody::Expr(e)), .. })) => e,
            (PropertyInit::Value(e), _) => e,
            (PropertyInit::Delegate(e), _) => e,
            _ => return Ty::Unknown,
        };
        let delegated = p.is_delegated() && p.getter.is_none();
        let saved = (std::mem::take(&mut self.scopes), std::mem::take(&mut self.receivers));
        self.enter_member(cx, "", true);
        if let Some(c) = self.classes.decls.iter().find(|d| d.info.rust == cx.owner).cloned()
            && let ClassAst::Class(k) = &c.ast
            && let Some(pc) = &k.primary_constructor
        {
            for cp in &pc.params {
                let ty = cp.param.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown);
                self.declare_at(&cp.param.name, snake(&cp.param.name.name), LocalKind::Param, ty);
            }
        }
        let t = if delegated { self.delegate_ty(e) } else { self.peek_ty(e) };
        (self.scopes, self.receivers) = saved;
        t
    }

    // ---- Writing -----------------------------------------------------------------------------------------------

    /// A top-level class-like declaration of the text and those nested in it.
    pub(crate) fn class_decl(&mut self, decl: &Decl) -> TokenStream {
        let loc = decl.loc();
        let found: Vec<ClassDecl> = self
            .classes
            .decls
            .iter()
            .filter(|d| {
                d.own
                    && class_loc(&d.ast).start.offset >= loc.start.offset
                    && class_loc(&d.ast).end.offset <= loc.end.offset
            })
            .cloned()
            .collect();
        let mut out = TokenStream::new();
        for d in &found {
            out.extend(self.write_class(d));
            // (what its members put beside it: a composable member's hidden function)
            out.extend(std::mem::take(&mut self.classes.side_items));
        }
        out
    }

    fn write_class(&mut self, d: &ClassDecl) -> TokenStream {
        use super::hierarchy::Inherit;
        let _locals = self.enter_locals_of(&d.info.rust);
        let serializable = self.serializable_class(d);
        let out = match (&d.ast, d.info.shape, d.info.inherit) {
            (ClassAst::Class(_), Shape::Interface, Inherit::Sealed) => self.open_class(d),
            (ClassAst::Class(_), Shape::Interface, _) => self.interface_trait(d),
            (ClassAst::Class(_), Shape::Data | Shape::Plain, Inherit::Open | Inherit::Abstract | Inherit::Sealed) => {
                self.open_class(d)
            }
            (ClassAst::Class(_), Shape::Data, _) => self.data_class(d),
            (ClassAst::Class(_), Shape::Plain, _) => self.plain_class_decl(d),
            (ClassAst::Class(c), Shape::Enum, _) => self.enum_class(c, d),
            (ClassAst::Object(o), _, _) => self.object(o, d),
            _ => TokenStream::new(),
        };
        quote!(#out #serializable)
    }

    /// ((stage 14 WPD) A `@Serializable` plain class (final or open: Kotlin's reference, not a data class's derive)
    /// or sealed class: `Saveable` as kotlinx serializes it — a class a map of its constructor's properties by their
    /// Kotlin names (read back through its constructor: every one of them a property, each given), a sealed class
    /// its case's name under `"type"` and the case's value under `"value"` —, and its companion `serializer()`.)
    fn serializable_class(&mut self, d: &ClassDecl) -> TokenStream {
        use super::hierarchy::Inherit;
        let ClassAst::Class(c) = &d.ast else { return quote!() };
        let c = (**c).clone();
        if !c.annotations.iter().any(|a| a.name.last().name == "Serializable") {
            return quote!();
        }
        let span = self.span(c.loc);
        let rt = respan(self.rt().clone(), span);
        let allow = class_allow(span);
        let name = ident(&d.info.rust, span);
        let vis = self.class_vis(&c.modifiers, c.loc);
        let (save, restore, descriptor) = match (d.info.shape, d.info.inherit) {
            (Shape::Plain, Inherit::Final | Inherit::Open) if c.type_params.is_empty() => {
                let params: Vec<ConstructorParam> =
                    c.primary_constructor.as_ref().map(|p| p.params.clone()).unwrap_or_default();
                let (mut saves, mut restores, mut elements) = (Vec::new(), Vec::new(), Vec::new());
                for p in &params {
                    let Some(ty) = &p.param.ty else { return quote!() };
                    if p.property.is_none() {
                        return self.later(
                            p.param.loc,
                            "a `@Serializable` class's constructor parameter that isn't a property",
                        );
                    }
                    let key = p.param.name.name.clone();
                    let getter = ident(&snake(&key), span);
                    let t = self.rust_type(ty, TypePos::Value);
                    // ((stage 14 FINAL) `@Serializable(with = UUIDSerializer::class) val uuid: UUID`: written and
                    // read by that serializer)
                    match self.serializer_with(&p.param.annotations, span) {
                        Some(sv) => {
                            saves.push(
                                quote_spanned!(span=> (#key, #rt::IntoSaver::into_saver(#sv).save(&self.#getter()))),
                            );
                            restores.push(
                                quote_spanned!(span=> #rt::IntoSaver::<#t>::into_saver(#sv).restore(__kz_map.get(#key)?)?),
                            );
                        }
                        None => {
                            saves.push(quote_spanned!(span=> (#key, #rt::Saveable::save(&self.#getter()))));
                            restores.push(quote_spanned!(span=> <#t as #rt::Saveable>::restore(__kz_map.get(#key)?)?));
                        }
                    }
                    elements.push(quote_spanned!(span=> #rt::SerialElement::of::<#t>(#key, false)));
                }
                (
                    quote_spanned!(span=> #rt::SavedValue::map::<&str>([#(#saves),*])),
                    quote_spanned!(span=> let __kz_map = __kz_saved.as_map()?; ::core::option::Option::Some(#name(#(#restores),*))),
                    quote_spanned!(span=> #rt::SerialDescriptor::class(::core::any::type_name::<Self>(), ::std::vec![#(#elements),*])),
                )
            }
            (_, Inherit::Sealed) if c.type_params.is_empty() && d.info.shape != Shape::Interface => {
                let cases = self.sealed_cases(&d.info.rust);
                let vs: Vec<proc_macro2::Ident> = cases.iter().map(|(v, _)| ident(v, span)).collect();
                let names: Vec<String> = cases.iter().map(|(v, _)| v.clone()).collect();
                let tys: Vec<proc_macro2::Ident> = cases.iter().map(|(_, r)| ident(r, span)).collect();
                (
                    quote_spanned! {span=>
                        match self {
                            #(#name::#vs(v) => #rt::SavedValue::map::<&str>([
                                ("type", #rt::SavedValue::from(#names)),
                                ("value", #rt::Saveable::save(v)),
                            ]),)*
                        }
                    },
                    quote_spanned! {span=>
                        let __kz_value = __kz_saved.get("value");
                        match __kz_saved.get("type").as_str()? {
                            #(#names => <#tys as #rt::Saveable>::restore(__kz_value).map(#name::#vs),)*
                            _ => ::core::option::Option::None,
                        }
                    },
                    quote_spanned!(span=> #rt::SerialDescriptor::new(::core::any::type_name::<Self>(), #rt::SerialKind::Contextual)),
                )
            }
            _ => return quote!(),
        };
        quote_spanned! {span=>
            #allow
            impl #rt::Saveable for #name {
                fn descriptor() -> #rt::SerialDescriptor {
                    #descriptor
                }
                fn save(&self) -> #rt::SavedValue {
                    #save
                }
                #[allow(unused_variables, clippy::question_mark)]
                fn restore(__kz_saved: &#rt::SavedValue) -> ::core::option::Option<Self> {
                    #restore
                }
            }
            #allow
            impl #name {
                #[allow(dead_code)]
                #vis fn serializer() -> #rt::Saver<Self> {
                    #rt::serializer::<Self>()
                }
            }
        }
    }

    /// The modifiers a class may have; its supertypes known.
    fn check_class(&mut self, d: &ClassDecl, c: &Class) -> bool {
        let mut ok = true;
        for m in &c.modifiers.list {
            match m.kind {
                ModifierKind::Public
                | ModifierKind::Private
                | ModifierKind::Internal
                | ModifierKind::Data
                | ModifierKind::Value
                | ModifierKind::Final
                | ModifierKind::Open
                | ModifierKind::Abstract
                | ModifierKind::Sealed
                | ModifierKind::Inner => {}
                _ => {
                    self.later(m.loc, &format!("`{}` on a class", m.kind.keyword()));
                    ok = false;
                }
            }
        }
        // (Kotlin's rule: a value class has one property, its primary constructor's only parameter, a `val`)
        if c.is_value() {
            let params = c.primary_constructor.as_ref().map(|p| p.params.as_slice()).unwrap_or_default();
            if !matches!(params, [p] if p.property == Some(Mutability::Val)) {
                self.error(c.loc, "a value class has one property: its constructor's only parameter, a `val`");
                ok = false;
            }
        }
        if c.is_inner() && d.outer.is_none() {
            self.error(c.loc, "an `inner` class is one nested in another");
        }
        // (Kotlin's rule: a data class can't be inner, nor open, abstract or sealed)
        if c.is_data() && (c.is_inner() || c.is_open() || c.is_abstract() || c.is_sealed()) {
            self.error(c.loc, "a data class can't be `inner`, `open`, `abstract` or `sealed` (Kotlin's rule)");
            ok = false;
        }
        // ((stage 14 WPB) a standard exception class as the superclass: `exceptions`)
        let exception = usize::from(self.exception_super(c).is_some());
        let known = self.supers_of(d).len() + exception;
        if known < c.supers.len() {
            for s in &c.supers {
                let exception_class = s.ty.simple_name().is_some_and(|n| self.exception_type(super::last_segment(&n)));
                if !exception_class && s.ty.simple_name().is_none_or(|n| self.super_kind(&n).is_none()) {
                    self.error(
                        s.loc,
                        format!(
                            "`{}` isn't a class or interface KompozR knows (the crate's Kotlin, an API trait, a Rust trait whose doc line says ``Kotlin: `open class …` ``)",
                            s.ty.simple_name().unwrap_or_default()
                        ),
                    );
                }
            }
            ok = false;
        }
        if let Some(s) = c.supers.iter().find(|s| {
            s.delegate.is_some()
                && !matches!(
                    s.ty.simple_name().and_then(|n| self.super_kind(&n)),
                    Some(super::hierarchy::SuperKind::Interface(_))
                )
        }) {
            self.error(s.loc, "only an interface is implemented by delegation (`I by value`)");
            ok = false;
        }
        ok
    }

    /// The fields of a class's `I by value` delegates (`__kz_by0: Rc<dyn I>`) and their values (its constructor's
    /// parameters in scope).
    fn delegate_fields(&mut self, d: &ClassDecl, span: Span) -> Vec<(TokenStream, TokenStream)> {
        let mut out = Vec::new();
        for (k, s) in super::hierarchy::written_supers(&d.ast).to_vec().iter().enumerate() {
            let Some(e) = &s.delegate else { continue };
            let ty = self.ty_of(&s.ty);
            let t = self.rust_type(&s.ty, TypePos::Value);
            let from = self.peek_ty(e);
            let v = self.value(e);
            let v = self.coerce(v, &from, &ty, span);
            let f = format_ident!("__kz_by{k}", span = span);
            out.push((quote_spanned!(span=> #[doc(hidden)] pub #f: #t), quote_spanned!(span=> #f: #v)));
        }
        out
    }

    pub(crate) fn class_vis(&mut self, m: &Modifiers, loc: Loc) -> TokenStream {
        let span = self.span(loc);
        match m.visibility() {
            // (a private class is the file's: the Rust module's)
            Visibility::Private => quote_spanned!(span=> pub(crate)),
            _ => self.visibility(m, loc),
        }
    }

    /// ((stage 14 WPD) a function-typed property's constructor parameter — `class SampleCallbackHelper(val
    /// preparePop: () -> Unit, …)` —: the closure given (`impl Fn`), held as the shared one the property is.)
    fn closure_convert(&mut self, p: &Param) -> Option<TokenStream> {
        let t = p.ty.as_ref()?;
        let ut = self.unaliased(t);
        if ut.nullable || ut.as_function().is_none() {
            return None;
        }
        let pos = if p.default.is_some() { TypePos::DefaultedParam } else { TypePos::Param };
        let given = self.quiet(|g| g.rust_type(t, pos)).to_string();
        let held = self.quiet(|g| g.rust_type(t, TypePos::Value));
        let held_text = held.to_string();
        if !given.starts_with("impl") || !(held_text.contains("Rc <") || held_text.contains("Arc <")) {
            return None;
        }
        let span = self.span(p.loc);
        let i = self.name_ident(&snake(&p.name.name), p.name.loc);
        let ptr = if held_text.contains("Arc <") {
            quote_spanned!(span=> ::std::sync::Arc)
        } else {
            quote_spanned!(span=> ::std::rc::Rc)
        };
        Some(quote_spanned!(span=> let #i: #held = #ptr::new(#i);))
    }

    /// (stage 14 FINAL) The serializer `@Serializable(with = S::class)` among `annotations` names, as a value: the
    /// object `S` (its base's struct where it is one's, `KSerializer<T>`), a class's `S()`; none for another.
    fn serializer_with(&mut self, annotations: &[Annotation], span: Span) -> Option<TokenStream> {
        let a = annotations.iter().find(|a| a.name.last().name == "Serializable")?;
        let with = a.args.as_deref()?.iter().find(|x| x.name.as_ref().is_some_and(|n| n.name == "with"))?;
        let ExprKind::CallableRef { receiver: Some(r), name } = &with.value.kind else { return None };
        let ExprKind::Name(n) = &r.kind else { return None };
        if name.name != "class" {
            return None;
        }
        let c = self.user_class(&n.name)?.clone();
        match c.shape {
            Shape::Object => self.user_name_value(&n.name, span).map(|(v, _)| v),
            Shape::Plain => {
                let i = ident(&c.rust, span);
                Some(quote_spanned!(span=> #i()))
            }
            _ => None,
        }
    }

    /// ((stage 14 FINAL) A `@Serializable` plain class: a typed route of its constructor's properties and a route's
    /// value — navigation-compose's `@Serializable class User(val id: WrappedUUID)`, a route whose argument's NavType
    /// is the typeMap's, `@Serializable class WrappedUUID(…)` —: `#[derive(Route, RouteValue)]` with
    /// `#[route(class(…))]` (only a route's value where a property has a serializer of its own).)
    fn plain_route_attrs(&mut self, c: &Class, span: Span) -> TokenStream {
        if !c.annotations.iter().any(|a| a.name.last().name == "Serializable") || !c.type_params.is_empty() {
            return quote!();
        }
        let params: Vec<ConstructorParam> =
            c.primary_constructor.as_ref().map(|p| p.params.clone()).unwrap_or_default();
        if params.iter().any(|p| p.property.is_none() || p.param.ty.is_none()) {
            return quote!();
        }
        if params.iter().any(|p| custom_serializer(&p.param)) {
            return quote_spanned!(span=> #[derive(RouteValue)]);
        }
        let fields: Vec<TokenStream> = params
            .iter()
            .map(|p| {
                let n = proc_macro2::Ident::new(&p.param.name.name, span);
                let t = self.rust_type(p.param.ty.as_ref().expect("its type"), TypePos::Value);
                quote_spanned!(span=> #n: #t)
            })
            .collect();
        // (`@SerialName("…")`: the route's name, as a data class's)
        let named = self.route_derive(&c.annotations, span).into_iter().skip(2).collect::<TokenStream>();
        quote_spanned!(span=> #[derive(Route, RouteValue)] #named #[route(class(#(#fields),*))])
    }

    /// `#[derive(Route, Saveable)]` for `@Serializable`.
    fn route_derive(&self, annotations: &[Annotation], span: Span) -> TokenStream {
        if annotations.iter().any(|a| a.name.last().name == "Serializable") {
            // ((stage 14 WPD) `@SerialName("login")`: the route's name)
            let serial_name = annotations.iter().filter(|a| a.name.last().name == "SerialName").find_map(|a| {
                match a.args.as_deref() {
                    Some([arg]) => match &arg.value.kind {
                        ExprKind::String(lit) => match lit.parts.as_slice() {
                            [StringPart::Text { text, .. }] => Some(text.clone()),
                            [] => Some(String::new()),
                            _ => None,
                        },
                        _ => None,
                    },
                    _ => None,
                }
            });
            // (and saved as kotlinx serializes it: `#[derive(Saveable)]`, what a back stack's keys, `rememberSerializable`
            // and `encodeToSavedState` take)
            let rt = respan(self.rt().clone(), span);
            match serial_name {
                Some(n) => quote_spanned!(span=> #[derive(Route, #rt::Saveable)] #[route(name = #n)]),
                None => quote_spanned!(span=> #[derive(Route, #rt::Saveable)]),
            }
        } else {
            quote!()
        }
    }

    /// ((stage 14 WPD) A `@Serializable` class's companion `serializer()` (kotlinx's generated one: `Home.serializer()`):
    /// its `Saver`, `serializer::<Self>()`; nothing for another class (`route` empty).)
    fn serializer_fn(&self, route: &TokenStream, vis: &TokenStream, header: TokenStream, span: Span) -> TokenStream {
        if route.is_empty() {
            return quote!();
        }
        let rt = respan(self.rt().clone(), span);
        quote_spanned! {span=>
            #header {
                #[allow(dead_code)]
                #vis fn serializer() -> #rt::Saver<Self> {
                    #rt::serializer::<Self>()
                }
            }
        }
    }

    /// The constructors of the inner classes of `outer`: its methods (`outer.Inner(…)`), each giving its object
    /// holding `outer` (a clone of its handle, or its value).
    fn inner_ctors(&mut self, outer: &UserClass) -> Vec<TokenStream> {
        let inner: Vec<ClassDecl> = self
            .classes
            .decls
            .iter()
            .filter(|d| d.own && d.info.inner.as_deref() == Some(outer.rust.as_str()))
            .cloned()
            .collect();
        let mut out = Vec::new();
        for d in inner {
            let ClassAst::Class(c) = &d.ast else { continue };
            if !matches!(d.info.inherit, super::hierarchy::Inherit::Final | super::hierarchy::Inherit::Open) {
                continue;
            }
            let span = self.span(c.loc);
            let cx = MemberCx {
                owner: outer.rust.clone(),
                shape: outer.shape,
                companion: outer.companion.clone(),
                outers: self.outers_of(outer),
            };
            self.fun = FnState::default();
            self.enter_member(&cx, &c.name.name, false);
            let mut params = Vec::new();
            let mut converts = Vec::new();
            let mut names = Vec::new();
            for cp in c.primary_constructor.iter().flat_map(|p| &p.params) {
                let (ts, convert) = self.param(&cp.param, false, false);
                params.push(ts);
                converts.extend(convert);
                names.push(self.name_ident(&snake(&cp.param.name.name), cp.param.name.loc));
            }
            let (_, g_use, _) = self.class_generics(&d, span);
            let ty = proc_macro2::Ident::new(&d.info.rust, span);
            let m = proc_macro2::Ident::new(&c.name.name, self.span(c.name.loc));
            let vis = self.member_vis(&c.modifiers, c.loc);
            let (recv, this) = if outer.shape == Shape::Enum {
                (quote_spanned!(span=> self), quote_spanned!(span=> self))
            } else {
                (quote_spanned!(span=> &self), quote_spanned!(span=> ::core::clone::Clone::clone(self)))
            };
            out.push(quote_spanned! {span=>
                #[allow(non_snake_case)]
                #vis fn #m(#recv, #(#params),*) -> #ty #g_use {
                    #(#converts)*
                    #ty::__kz_new(#this, #(#names),*)
                }
            });
        }
        out
    }

    /// A data class (see the module's doc): its body's stored and delegated properties made with the value, its
    /// base class's part shared by its copies; `==` and `toString()` of its constructor's properties only.
    fn data_class(&mut self, d: &ClassDecl) -> TokenStream {
        use super::hierarchy::Place;
        let ClassAst::Class(c) = &d.ast else { return quote!() };
        let c = &**c;
        let info = &d.info;
        let span = self.span(c.loc);
        if !self.check_class(d, c) {
            return quote!();
        }
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let vis = self.class_vis(&c.modifiers, c.loc);
        let name = proc_macro2::Ident::new(&info.rust, self.span(c.name.loc));
        let allow = class_allow(span);
        let mut route = self.route_derive(&c.annotations, span);
        // ((stage 14 WPD) a property of its own serializer, `@Serializable(with = ProductSerializer::class) val
        // product: Product`, or of a class of the crate's, `val name: Name` (a navigation key's: a deep link's
        // arguments): saved, no typed route of it — a route's arguments are primitives, enums and lists of them)
        let class_typed = |me: &Self, p: &Param| {
            p.ty.as_ref()
                .and_then(|t| t.simple_name())
                .and_then(|n| me.user_class(&n).cloned())
                .is_some_and(|u| u.shape != Shape::Enum)
        };
        if !route.is_empty()
            && c.primary_constructor
                .iter()
                .flat_map(|p| &p.params)
                .any(|p| custom_serializer(&p.param) || class_typed(self, &p.param))
        {
            route = quote_spanned!(span=> #[derive(#rt::Saveable)]);
        }
        let (g_decl, g_use, g_struct) = self.class_generics(d, span);
        let cx = MemberCx {
            owner: info.rust.clone(),
            shape: Shape::Data,
            companion: info.companion.clone(),
            outers: self.outers_of(info),
        };
        let params: Vec<ConstructorParam> =
            c.primary_constructor.as_ref().map(|p| p.params.clone()).unwrap_or_default();
        // The constructor's parameters (a plain function's: `impl Into<String>`, defaults).
        self.fun = FnState::default();
        self.enter_member(&cx, &c.name.name, false);
        let mut ctor_params = Vec::new();
        let mut converts = Vec::new();
        let mut names = Vec::new();
        let mut value_params = Vec::new();
        for cp in &params {
            let (ts, convert) = self.param(&cp.param, false, false);
            ctor_params.push(ts);
            converts.extend(convert);
            converts.extend(self.closure_convert(&cp.param));
            let i = self.name_ident(&snake(&cp.param.name.name), cp.param.name.loc);
            let t = cp.param.ty.as_ref().map(|t| self.rust_type(t, TypePos::Value)).unwrap_or_default();
            value_params.push(quote_spanned!(span=> #i: #t));
            names.push(i);
        }
        // Fields: the constructor's properties, then the body's stored properties.
        let mut fields = Vec::new();
        let mut field_values = Vec::new();
        let mut getters = Vec::new();
        let mut to_string = Vec::new();
        let mut components = Vec::new();
        let mut copy_params = Vec::new();
        let mut route_attrs = Vec::new();
        let mut compared = Vec::new();
        let mut kinds = Vec::new();
        // (a base class's constructor reads the parameters after the value is made: the fields clones of them)
        let has_super = self.superclass(d).is_some();
        for (k, cp) in params.iter().enumerate() {
            let Some(m) = cp.property else {
                continue;
            };
            let pspan = self.span(cp.param.loc);
            let i = self.name_ident(&snake(&cp.param.name.name), cp.param.name.loc);
            let t = cp.param.ty.as_ref().map(|t| self.rust_type(t, TypePos::Value)).unwrap_or_default();
            // (a `Copy` value read as it is: a clone of it is clippy's `clone_on_copy`)
            let read = if cp.param.ty.as_ref().is_some_and(|t| self.copied(&self.ty_of(t))) {
                quote_spanned!(pspan=> self.#i)
            } else {
                quote_spanned!(pspan=> ::core::clone::Clone::clone(&self.#i))
            };
            let fvis = self.member_vis(&cp.param.modifiers, cp.param.loc);
            let route_default = match (&cp.param.default, route.is_empty()) {
                (Some(d), false) => {
                    let saved = std::mem::take(&mut self.scopes);
                    self.scopes = vec![HashMap::new()];
                    // ((stage 14 WPD) a nullable property's: an `Option` — `val userId: String? = "no value given"`)
                    let nullable = cp.param.ty.as_ref().is_some_and(|t| t.nullable);
                    let v = self.value_want(d, super::expr::Want { some: nullable, ..Default::default() });
                    self.scopes = saved;
                    quote_spanned!(pspan=> #[route(default = #v)])
                }
                _ => quote!(),
            };
            // (saved under its Kotlin name, as kotlinx writes it: `userId`)
            let kotlin_name = &cp.param.name.name;
            let route_default = if !route.is_empty() && snake(kotlin_name) != *kotlin_name {
                quote_spanned!(pspan=> #route_default #[saveable(name = #kotlin_name)])
            } else {
                route_default
            };
            route_attrs.push(route_default.clone());
            compared.push(i.clone());
            let kind = cp.param.ty.as_ref().map_or(FieldKind::Compared, |t| {
                self.data_field_kind(&self.ty_of(t), &info.type_params, &mut vec![info.rust.clone()])
            });
            kinds.push(kind);
            let given = if has_super { quote_spanned!(pspan=> ::core::clone::Clone::clone(&#i)) } else { quote!(#i) };
            if m == Mutability::Var {
                fields.push(quote_spanned!(pspan=> #route_default #fvis #i: #kt::KtShared<#t>));
                field_values.push(quote_spanned!(pspan=> #i: #kt::KtShared::new(#given)));
                let setter = format_ident!("set_{}", i.to_string().trim_start_matches("r#"), span = pspan);
                getters.push(quote_spanned! {pspan=>
                    #fvis fn #i(&self) -> #t { self.#i.get() }
                    #fvis fn #setter(&self, value: #t) { self.#i.set(value) }
                });
                copy_params.push(quote_spanned!(pspan=> #[default(self.#i.get())] #i: #t));
            } else {
                fields.push(quote_spanned!(pspan=> #route_default #fvis #i: #t));
                field_values.push(if has_super {
                    quote_spanned!(pspan=> #i: #given)
                } else {
                    quote_spanned!(pspan=> #i)
                });
                getters.push(quote_spanned!(pspan=> #fvis fn #i(&self) -> #t { #read }));
                copy_params.push(quote_spanned!(pspan=> #[default(#read)] #i: #t));
            }
            let label = format!("{}{}=", if to_string.is_empty() { "" } else { ", " }, cp.param.name.name);
            to_string.push(
                quote_spanned!(pspan=> __kz_out.push_str(#label); (&#kt::KtProbe(&self.#i)).kt_write_probe(__kz_out);),
            );
            let comp = format_ident!("component{}", k + 1, span = pspan);
            components.push(quote_spanned!(pspan=> #fvis fn #comp(&self) -> #t { self.#i() }));
        }
        // (its base class's part: shared by the copies, made anew by `copy()`)
        let mut own_fields = false;
        if let Some((f, e)) = self.super_field(d, Place::Data, span) {
            fields.push(f);
            field_values.push(e);
            own_fields = true;
        }
        for (f, e) in self.delegate_fields(d, span) {
            fields.push(f);
            field_values.push(e);
            own_fields = true;
        }
        // The body: stored properties computed as the value is made (the constructor's parameters in scope), init
        // blocks run then, member functions.
        self.enter_member(&cx, &c.name.name, false);
        for cp in &params {
            let ty = cp.param.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown);
            self.declare_at(&cp.param.name, snake(&cp.param.name.name), LocalKind::Param, ty);
        }
        let super_init = self.super_init(d, quote_spanned!(span=> &__kz_v), Place::Data, span);
        // (with a base class, Kotlin's order: the value made first — its body's properties empty cells —, the base's
        // initializer run on it, then the body's initializers and init blocks in order)
        let after_super = !super_init.is_empty();
        // (then the value its initializers see: the base's members theirs, as `this`'s)
        if after_super {
            let params: Vec<Ty> =
                index::generics(&info.rust).unwrap_or_default().iter().map(|p| Ty::named(p)).collect();
            self.receivers.push(ImplicitReceiver {
                rust: "__kz_v".into(),
                ty: info.rust.clone(),
                value: Ty::Named(info.rust.clone(), params),
                labels: vec![c.name.name.clone()],
                depth: 0,
                by_ref: false,
                by_mut: false,
                owned_used: Default::default(),
            });
        }
        let mut made = Vec::new();
        let mut methods = Vec::new();
        let mut own_to_string = false;
        for m in &c.body.members {
            match m {
                Member::Init(b) => {
                    let s = self.braced(b, super::stmt::Tail::Unit);
                    made.push(quote_spanned!(span=> #s));
                }
                Member::Decl(Decl::Property(p)) => {
                    let Binding::Name(n) = &p.binding else {
                        self.later(p.loc, "a destructuring member");
                        continue;
                    };
                    if p.getter.is_some() && matches!(p.init, PropertyInit::None) {
                        methods.push(self.getter_member(p, &cx));
                        continue;
                    }
                    let celled = after_super || p.is_var() || p.getter.is_some() || p.setter.is_some();
                    let kt_path = kt.clone();
                    let pspan = self.span(p.loc);
                    let i = self.name_ident(&snake(&n.name), n.loc);
                    let fvis = self.member_vis(&p.modifiers, p.loc);
                    if celled && !p.is_delegated() {
                        // (a `var`, one with accessors, or one made after the base's initializer: a cell of the value,
                        // shared by its copies, `copy()` making it anew)
                        let PropertyInit::Value(init) = &p.init else {
                            self.error(n.loc, format!("`{}` needs a value", n.name));
                            continue;
                        };
                        let ty = match &p.ty {
                            Some(t) => self.ty_of(t),
                            None => self.peek_ty(init),
                        };
                        let t = match &p.ty {
                            Some(t) => self.rust_type(t, TypePos::Value),
                            None if !ty.is_vague() => self.rust_of_ty(&ty, pspan),
                            None => {
                                self.error(
                                    n.loc,
                                    format!("write `{}`'s type: KompozR can't tell its value's type", n.name),
                                );
                                continue;
                            }
                        };
                        let cell = if p.is_var() {
                            quote_spanned!(pspan=> #kt::KtVar)
                        } else {
                            quote_spanned!(pspan=> #kt::KtVal)
                        };
                        let put = if p.is_var() { quote_spanned!(pspan=> set) } else { quote_spanned!(pspan=> init) };
                        let v = self.expecting(init, &ty, |g| g.value(init));
                        if after_super {
                            made.push(quote_spanned!(pspan=> let #i: #t = #v; __kz_v.#i.#put(::core::clone::Clone::clone(&#i));));
                            field_values.push(quote_spanned!(pspan=> #i: ::std::rc::Rc::new(#cell::new())));
                        } else {
                            made.push(quote_spanned!(pspan=> let #i: #t = #v;));
                            field_values.push(quote_spanned!(pspan=> #i: {
                                let c = #cell::new();
                                c.#put(::core::clone::Clone::clone(&#i));
                                ::std::rc::Rc::new(c)
                            }));
                        }
                        self.declare_at(n, snake(&n.name), LocalKind::Val, ty.clone());
                        fields.push(quote_spanned!(pspan=> #[doc(hidden)] pub #i: ::std::rc::Rc<#cell<#t>>));
                        own_fields = true;
                        let saved = (std::mem::take(&mut self.scopes), std::mem::take(&mut self.receivers));
                        getters.push(self.accessors_with_field(p, &i, &t, &ty, &fvis, &cx, Some(&quote!(self))));
                        (self.scopes, self.receivers) = saved;
                        continue;
                    }
                    // ((stage 14 FINAL-T) a `val` whose initializer reads the object, `super.metadata + more` (Kotlin's
                    // `this` while it is made, the properties above it set): its value made of the object, once, when
                    // first read — the object's whole value then, what the properties above give it alike)
                    if let PropertyInit::Value(init) = &p.init
                        && !p.is_var()
                        && !after_super
                        && reads_object(init)
                    {
                        let Some(pt) = &p.ty else {
                            self.error(n.loc, format!("write `{}`'s type (its value reads the object)", n.name));
                            continue;
                        };
                        let t = self.rust_type(pt, TypePos::Value);
                        let made_name = format!("__kz_make_{}", snake(&n.name));
                        let f = Function {
                            annotations: Vec::new(),
                            modifiers: Modifiers { list: vec![Modifier { kind: ModifierKind::Private, loc: p.loc }] },
                            type_params: Vec::new(),
                            receiver: None,
                            name: Ident { name: made_name.clone(), backticked: false, loc: n.loc },
                            params: Vec::new(),
                            result: Some(pt.clone()),
                            constraints: Vec::new(),
                            body: Some(FunctionBody::Expr(init.clone())),
                            loc: p.loc,
                        };
                        let saved = (std::mem::take(&mut self.scopes), std::mem::take(&mut self.receivers));
                        methods.push(self.member_fn(&f, &cx));
                        (self.scopes, self.receivers) = saved;
                        let make = proc_macro2::Ident::new(&made_name, pspan);
                        fields.push(
                            quote_spanned!(pspan=> #[doc(hidden)] pub #i: ::std::rc::Rc<::std::cell::OnceCell<#t>>),
                        );
                        field_values.push(quote_spanned!(pspan=> #i: ::std::rc::Rc::new(::std::cell::OnceCell::new())));
                        getters.push(quote_spanned!(pspan=>
                            #fvis fn #i(&self) -> #t {
                                ::core::clone::Clone::clone(self.#i.get_or_init(|| self.#make()))
                            }
                        ));
                        own_fields = true;
                        continue;
                    }
                    // (a delegated one, `val label by lazy { … }`: the delegate kept, its value read through it)
                    // ((stage 14 WPB) the delegate held as a plain class's is, `property`: a shared `KtDelegate` (a
                    // `var`'s `KtMutableDelegate`) of the property's type, whatever the delegate's own type is)
                    if let PropertyInit::Delegate(e) = &p.init {
                        let vty = match &p.ty {
                            Some(t) => self.ty_of(t),
                            None => self.delegate_ty(e),
                        };
                        let vt = match &p.ty {
                            Some(t) => self.rust_type(t, TypePos::Value),
                            None if !vty.is_vague() => self.rust_of_ty(&vty, pspan),
                            None => {
                                self.error(n.loc, format!("write `{}`'s type", n.name));
                                continue;
                            }
                        };
                        let dt = if p.is_var() {
                            quote_spanned!(pspan=> ::std::rc::Rc<dyn #kt_path::KtMutableDelegate<Value = #vt>>)
                        } else {
                            quote_spanned!(pspan=> ::std::rc::Rc<dyn #kt_path::KtDelegate<Value = #vt>>)
                        };
                        let class = info.kotlin.clone();
                        let made_delegate = self.keyed_delegate_value(e, &n.name, Some(&class));
                        let v =
                            quote_spanned!(pspan=> ::std::rc::Rc::new(#kt_path::delegate_of::<#vt, _>(#made_delegate)));
                        if p.is_var() {
                            let set = format_ident!("set_{}", i.to_string().trim_start_matches("r#"), span = pspan);
                            let read = if after_super {
                                quote_spanned!(pspan=> self.#i.get_ref())
                            } else {
                                quote_spanned!(pspan=> &self.#i)
                            };
                            getters.push(quote_spanned!(pspan=>
                                #fvis fn #set(&self, value: #vt) { #kt::KtMutableDelegate::set_value(#read, value) }
                            ));
                        }
                        if after_super {
                            made.push(quote_spanned!(pspan=> __kz_v.#i.init(#v);));
                            fields.push(quote_spanned!(pspan=> #[doc(hidden)] pub #i: ::std::rc::Rc<#kt::KtVal<#dt>>));
                            field_values.push(quote_spanned!(pspan=> #i: ::std::rc::Rc::new(#kt::KtVal::new())));
                            getters.push(quote_spanned!(pspan=>
                                #fvis fn #i(&self) -> #vt { #kt::KtDelegate::get_value(self.#i.get_ref()) }
                            ));
                        } else {
                            made.push(quote_spanned!(pspan=> let #i: #dt = #v;));
                            fields.push(quote_spanned!(pspan=> #[doc(hidden)] pub #i: #dt));
                            field_values.push(quote_spanned!(pspan=> #i));
                            getters.push(
                                quote_spanned!(pspan=> #fvis fn #i(&self) -> #vt { #kt::KtDelegate::get_value(&self.#i) }),
                            );
                        }
                        own_fields = true;
                        continue;
                    }
                    let PropertyInit::Value(init) = &p.init else {
                        self.error(n.loc, format!("`{}` needs a value", n.name));
                        continue;
                    };
                    let ty = match &p.ty {
                        Some(t) => self.ty_of(t),
                        None => self.peek_ty(init),
                    };
                    let t = match &p.ty {
                        Some(t) => self.rust_type(t, TypePos::Value),
                        None if !ty.is_vague() => self.rust_of_ty(&ty, pspan),
                        None => {
                            self.error(
                                n.loc,
                                format!("write `{}`'s type: KompozR can't tell its value's type", n.name),
                            );
                            continue;
                        }
                    };
                    let v = match self.lambda_property_value(p, init) {
                        Some(v) => v,
                        None => self.expecting(init, &ty, |g| g.value(init)),
                    };
                    made.push(quote_spanned!(pspan=> let #i: #t = #v;));
                    self.declare_at(n, snake(&n.name), LocalKind::Val, ty.clone());
                    fields.push(quote_spanned!(pspan=> #fvis #i: #t));
                    field_values.push(quote_spanned!(pspan=> #i));
                    own_fields = true;
                    let read = if self.copied(&ty) {
                        quote_spanned!(pspan=> self.#i)
                    } else {
                        quote_spanned!(pspan=> ::core::clone::Clone::clone(&self.#i))
                    };
                    getters.push(quote_spanned!(pspan=> #fvis fn #i(&self) -> #t { #read }));
                }
                Member::Decl(Decl::Function(f)) => {
                    own_to_string |= f.name.name == "toString" && f.params.is_empty();
                    let saved = std::mem::take(&mut self.scopes);
                    methods.push(self.member_fn(f, &cx));
                    self.scopes = saved;
                }
                Member::Decl(Decl::Class(_) | Decl::Object(_) | Decl::TypeAlias(_)) => {}
                Member::Decl(d) => {
                    self.later(d.loc(), "this member");
                }
                Member::Constructor(s) => {
                    methods.push(self.secondary_ctor(s, &cx, &c.name.name));
                }
            }
        }
        methods.extend(self.inner_ctors(info));
        let kt_name = &c.name.name;
        let display = if own_to_string {
            quote_spanned!(span=> __kz_out.push_str(&self.to_string_kt());)
        } else {
            let open = format!("{kt_name}(");
            quote_spanned!(span=> __kz_out.push_str(#open); #(#to_string)* __kz_out.push(')');)
        };
        let companion_const = self.companion_const(info, span);
        let new_body = if super_init.is_empty() {
            quote_spanned!(span=> #(#made)* #name { #(#field_values),* })
        } else {
            quote_spanned!(span=> let __kz_v = #name { #(#field_values),* }; #super_init #(#made)* __kz_v)
        };
        let impls = self.class_impls(d, Place::Data, &g_decl, &g_use);
        // (Kotlin's `equals` of a data class: its constructor's properties)
        // ((stage 14 WPB) `equals`/`hashCode` overridden: theirs)
        // ((stage 14 FINAL-T) a `Float`/`Double` property by its bits, Kotlin's `Float.compare`; hashed — Kotlin's
        // `hashCode()` of the properties — when every property's type hashes, `Eq` then too)
        let bits = kinds.contains(&FieldKind::Bits) || kinds.contains(&FieldKind::Any);
        let compares = compared.iter().zip(&kinds).map(|(i, k)| match k {
            FieldKind::Bits => quote_spanned!(span=> #kt::KtBits::kt_bits(&self.#i) == #kt::KtBits::kt_bits(&other.#i)),
            FieldKind::Any => quote_spanned!(span=> #kt::eq_any(&self.#i, &other.#i)),
            _ => quote_spanned!(span=> self.#i == other.#i),
        });
        let hashes = !kinds.contains(&FieldKind::Compared) && !kinds.contains(&FieldKind::Any);
        let structural_hash = if hashes {
            let each = compared.iter().zip(&kinds).map(|(i, k)| match k {
                FieldKind::Bits => {
                    quote_spanned!(span=> ::core::hash::Hash::hash(&#kt::KtBits::kt_bits(&self.#i), state);)
                }
                _ => quote_spanned!(span=> ::core::hash::Hash::hash(&self.#i, state);),
            });
            quote_spanned!(span=> #(#each)*)
        } else {
            quote!()
        };
        let (eq_body, hash_body) =
            self.equality_bodies(&c.body.members, quote_spanned!(span=> true #(&& #compares)*), structural_hash, span);
        // (the class's type parameters `Hash`/`Eq` where its value is)
        let bound = |tr: TokenStream| -> TokenStream {
            let ps: Vec<proc_macro2::Ident> =
                info.type_params.iter().map(|p| proc_macro2::Ident::new(p, span)).collect();
            if ps.is_empty() { quote!() } else { quote_spanned!(span=> where #(#ps: #tr),*) }
        };
        let hash = if hash_body.is_empty() {
            quote!()
        } else {
            let wh = bound(quote_spanned!(span=> ::core::hash::Hash));
            let eq_wh = bound(quote_spanned!(span=> ::core::cmp::Eq));
            quote_spanned! {span=>
                #allow
                impl #g_decl ::core::hash::Hash for #name #g_use #wh {
                    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                        #hash_body
                    }
                }
                #allow
                impl #g_decl ::core::cmp::Eq for #name #g_use #eq_wh {}
            }
        };
        let overrides_eq = self.overrides_equality(&c.body.members).0;
        let (derive_eq, eq) = if own_fields || overrides_eq || bits {
            (
                quote!(),
                quote_spanned! {span=>
                    #allow
                    impl #g_decl ::core::cmp::PartialEq for #name #g_use {
                        fn eq(&self, other: &Self) -> bool {
                            #eq_body
                        }
                    }
                    #hash
                },
            )
        } else {
            (quote_spanned!(span=> , ::core::cmp::PartialEq), hash)
        };
        let serializer = self.serializer_fn(&route, &vis, quote!(#allow impl #g_decl #name #g_use), span);
        // ((stage 14 FINAL-T) `out` type parameters: `variance`)
        let out_method = self.out_method_data(d, &g_decl, &g_use, span);
        quote_spanned! {span=>
            #allow
            #[derive(::core::clone::Clone #derive_eq)]
            #route
            #vis struct #name #g_struct { #(#fields),* }

            #serializer
            #out_method

            #allow
            #[#rt::named(type)]
            #vis fn #name #g_decl (#(#ctor_params),*) -> #name #g_use {
                #(#converts)*
                #name::__kz_new(#(#names),*)
            }

            #allow
            impl #g_decl #name #g_use {
                #[doc(hidden)]
                #vis fn __kz_new(#(#value_params),*) -> #name #g_use {
                    #[allow(unused_imports)]
                    use #kt::ext::*;
                    #rt::named_args!(#kz_types { #new_body })
                }
                #(#getters)*
                #(#components)*
                #companion_const
            }

            #allow
            #[#rt::named]
            impl #g_decl #name #g_use {
                #vis fn copy(&self, #(#copy_params),*) -> #name #g_use {
                    #name::__kz_new(#(#names),*)
                }
                #(#methods)*
            }

            #eq

            #allow
            impl #g_decl #kt::KtToString for #name #g_use {
                fn kt_write(&self, __kz_out: &mut ::std::string::String) {
                    #[allow(unused_imports)]
                    use #kt::{KtViaDisplay as _, KtViaToString as _};
                    #display
                }
            }
            #allow
            impl #g_decl ::core::fmt::Display for #name #g_use {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.write_str(&#kt::KtToString::kt_to_string(self))
                }
            }
            #allow
            impl #g_decl ::core::fmt::Debug for #name #g_use {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.write_str(&#kt::KtToString::kt_to_string(self))
                }
            }

            #impls
        }
    }
    /// A body property's accessors (a data class's, a plain class's, an object's): its getter and setter, Kotlin's
    /// accessors' bodies where written. `place` holds its backing field (`self` of a data class's value,
    /// `self.__kz_this` of a plain class's object: `place.x` a cell), `field` its value, a setter's assignment to it
    /// written back; none for a property without one (its accessors computing it from the others).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn accessors_with_field(
        &mut self,
        p: &Property,
        i: &proc_macro2::Ident,
        t: &TokenStream,
        ty: &Ty,
        vis: &TokenStream,
        cx: &MemberCx,
        place: Option<&TokenStream>,
    ) -> TokenStream {
        let span = self.span(p.loc);
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let field_ident = Ident { name: "field".into(), backticked: false, loc: p.loc };
        let getter = match (p.getter.as_ref().and_then(|g| g.body.as_ref()), place) {
            (Some(body), _) => {
                self.fun = FnState {
                    kompozable: false,
                    nullable_result: ty.is_nullable(),
                    shared_result: None,
                    result: Some(ty.clone()),
                };
                self.enter_member(cx, "", true);
                let field = match place {
                    Some(place) => {
                        self.declare_at(&field_ident, "field".into(), LocalKind::Val, ty.clone());
                        quote_spanned!(span=> #[allow(unused_variables)] let field: #t = #place.#i.get();)
                    }
                    None => quote!(),
                };
                let b = match body {
                    FunctionBody::Block(b) => self.fn_block(b, false),
                    FunctionBody::Expr(e) => self.value_want(
                        e,
                        super::expr::Want { some: ty.is_nullable(), result: true, ..Default::default() },
                    ),
                };
                let lets = self.member_lets(cx, span);
                quote_spanned! {span=>
                    #vis fn #i(&self) -> #t {
                        #[allow(unused_imports)]
                        use #kt::ext::*;
                        #lets
                        #field
                        #rt::named_args!(#kz_types { #b })
                    }
                }
            }
            (None, Some(place)) => quote_spanned!(span=> #vis fn #i(&self) -> #t { #place.#i.get() }),
            (None, None) => {
                self.error(p.loc, "a property without a value needs a getter");
                return quote!();
            }
        };
        if !p.is_var() {
            return getter;
        }
        let set = format_ident!("set_{}", i.to_string().trim_start_matches("r#"), span = span);
        let setter_vis = match p.setter.as_ref() {
            Some(s) if s.modifiers.visibility() != Visibility::Public => self.member_vis(&s.modifiers, s.loc),
            _ => vis.clone(),
        };
        let setter = match (p.setter.as_ref().and_then(|s| s.body.as_ref().map(|b| (s, b))), place) {
            (Some((s, body)), _) => {
                let pname = s.param.as_ref().map(|p| p.name.clone()).unwrap_or(Ident {
                    name: "value".into(),
                    backticked: false,
                    loc: p.loc,
                });
                self.fun = FnState::default();
                self.enter_member(cx, "", true);
                if place.is_some() {
                    self.declare_at(&field_ident, "field".into(), LocalKind::Var, ty.clone());
                }
                let pr = snake(&pname.name);
                self.declare_at(&pname, pr.clone(), LocalKind::Param, ty.clone());
                let b = match body {
                    FunctionBody::Block(b) => self.fn_block(b, true),
                    FunctionBody::Expr(e) => {
                        let s = self.stmt_expr(e);
                        quote!(#s;)
                    }
                };
                let lets = self.member_lets(cx, span);
                let pi = ident(&pr, span);
                let (read, write) = match place {
                    Some(place) => (
                        quote_spanned!(span=> #[allow(unused_mut, unused_assignments)] let mut field: #t = #place.#i.get();),
                        quote_spanned!(span=> #place.#i.set(field)),
                    ),
                    None => (quote!(), quote!()),
                };
                quote_spanned! {span=>
                    #setter_vis fn #set(&self, #pi: #t) {
                        #[allow(unused_imports)]
                        use #kt::ext::*;
                        #lets
                        #read
                        #rt::named_args!(#kz_types { #b });
                        #write
                    }
                }
            }
            (None, Some(place)) => {
                quote_spanned!(span=> #setter_vis fn #set(&self, value: #t) { #place.#i.set(value) })
            }
            (None, None) => {
                self.error(p.loc, "a `var` without a value needs a setter");
                return quote!();
            }
        };
        quote!(#getter #setter)
    }

    /// `pub const Companion: CCompanion = CCompanion {};` of a class with a companion (Kotlin's `C.Companion`).
    fn companion_const(&self, info: &UserClass, span: Span) -> TokenStream {
        match &info.companion {
            Some(c) => {
                let ci = proc_macro2::Ident::new(c, span);
                quote_spanned!(span=> #[allow(non_upper_case_globals)] pub const Companion: #ci = #ci {};)
            }
            None => quote!(),
        }
    }

    /// A member's visibility: `pub`, `pub(crate)`, private (Kotlin's `private` member: the class's, Rust's module's).
    fn member_vis(&mut self, m: &Modifiers, loc: Loc) -> TokenStream {
        let span = self.span(loc);
        match m.visibility() {
            Visibility::Public => quote_spanned!(span=> pub),
            Visibility::Internal => quote_spanned!(span=> pub(crate)),
            Visibility::Private => quote!(),
            // (Kotlin's `protected`: the class's and its subclasses' — the crate's here, where they are)
            Visibility::Protected => {
                let _ = loc;
                quote_spanned!(span=> pub(crate))
            }
        }
    }

    /// A property with only a getter (`val big get() = total > 100`): a method.
    fn getter_member(&mut self, p: &Property, cx: &MemberCx) -> TokenStream {
        let Binding::Name(n) = &p.binding else { return quote!() };
        let Some(g) = &p.getter else { return quote!() };
        let Some(body) = &g.body else {
            self.later(p.loc, "a property without a value");
            return quote!();
        };
        let f = Function {
            annotations: Vec::new(),
            modifiers: p.modifiers.clone(),
            type_params: Vec::new(),
            receiver: None,
            name: n.clone(),
            params: Vec::new(),
            result: p.ty.clone().or_else(|| g.ty.clone()),
            constraints: Vec::new(),
            body: Some(body.clone()),
            loc: p.loc,
        };
        let saved = std::mem::take(&mut self.scopes);
        let out = self.member_fn(&f, cx);
        self.scopes = saved;
        out
    }

    /// A member function: a method of the type (`&self`; an enum's `self`), translated as a function is.
    pub(crate) fn member_fn(&mut self, f: &Function, cx: &MemberCx) -> TokenStream {
        // ((stage 14 FINAL-T) `hashCode()`'s arithmetic Kotlin's, wrapping)
        let hash = f.name.name == "hashCode" && f.params.is_empty();
        let saved = std::mem::replace(&mut self.control.wrapping, hash);
        let out = self.member_fn_with(f, cx, None);
        self.control.wrapping = saved;
        out
    }

    /// A composable member function, `@Composable fun Content()` of a class: a hidden composable of the object (a
    /// clone of its handle, or its value) and the parameters, restartable as a composable function is, beside the
    /// class; the method `Content(…)` calls it (`state.Content()` in a composable).
    fn composable_member(&mut self, f: &Function, cx: &MemberCx) -> TokenStream {
        let span = self.span(f.loc);
        if !index::generics(&cx.owner).unwrap_or_default().is_empty() || !f.type_params.is_empty() {
            return self.later(f.loc, "a composable member function of a generic class or with type parameters");
        }
        if cx.shape == Shape::Interface || self.user_class_rust(&cx.owner).is_none() {
            return self.later(f.loc, "a composable member function of an interface or an open class");
        }
        let inline = f.modifiers.has(ModifierKind::Inline);
        let unit = f.result.as_ref().is_none_or(|t| self.ty_of(t) == Ty::Unit);
        let restartable = !inline && unit;
        self.fun = FnState { kompozable: true, nullable_result: false, shared_result: None, result: None };
        self.enter_member(cx, &f.name.name, true);
        // (the object: the hidden function's first parameter)
        if let Some(r) = self.receivers.iter_mut().rev().find(|r| r.rust == "self") {
            r.rust = "__kz_this".into();
        }
        let this_ty = self.receivers.iter().rev().find(|r| r.rust == "__kz_this").map(|r| r.value.clone());
        self.declare("this", "__kz_this".into(), LocalKind::Param, this_ty.unwrap_or(Ty::named(&cx.owner)));
        self.in_async = false;
        self.inline_async = false;
        let mut params = Vec::new();
        let mut converts = Vec::new();
        for p in &f.params {
            let (tokens, convert) = self.param(p, restartable, inline);
            params.push(tokens);
            converts.extend(convert);
        }
        let ret = match &f.result {
            Some(t) if self.ty_of(t) != Ty::Unit => {
                let rt = self.rust_type(t, TypePos::Return);
                quote_spanned!(span=> -> #rt)
            }
            _ => quote!(),
        };
        let body = match &f.body {
            Some(FunctionBody::Block(b)) => self.fn_block(b, unit),
            Some(FunctionBody::Expr(e)) => {
                let s = self.stmt_expr(e);
                quote!(#s;)
            }
            None => {
                self.error(f.name.loc, "a function here needs a body");
                quote!()
            }
        };
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let lets = self.member_lets(cx, span).to_string().replace("self.", "__kz_this.");
        let lets: TokenStream = respan(lets.parse().unwrap_or_default(), span);
        let owner = proc_macro2::Ident::new(&cx.owner, span);
        let hidden = format_ident!("__kz_member_{}_{}", cx.owner, f.name.name, span = span);
        // ((stage 14 LISTS) a camelCase one — `fun toLayoutType(): NavigationSuiteType` of an enum — snake_case, as its
        // calls write it; a PascalCase one as written)
        let rust_name =
            if f.name.name.starts_with(char::is_uppercase) { f.name.name.clone() } else { snake(&f.name.name) };
        let name = self.name_ident(&rust_name, f.name.loc);
        let names: Vec<proc_macro2::Ident> =
            f.params.iter().map(|p| self.name_ident(&snake(&p.name.name), p.name.loc)).collect();
        let vis = self.member_vis(&f.modifiers, f.loc);
        let attr = self.kompozable_attr(inline || !unit, span);
        let allow = allow_attr(span);
        let (recv, this) = if cx.shape == Shape::Enum {
            (quote_spanned!(span=> self), quote_spanned!(span=> self))
        } else {
            (quote_spanned!(span=> &self), quote_spanned!(span=> ::core::clone::Clone::clone(self)))
        };
        self.classes.side_items.extend(quote_spanned! {span=>
            #allow
            #attr
            #[doc(hidden)]
            #vis fn #hidden(__kz_this: #owner, #(#params),*) #ret {
                #[allow(unused_imports)]
                use #kt::ext::*;
                #lets
                #(#converts)*
                #body
            }
        });
        let mparams: Vec<TokenStream> = f
            .params
            .iter()
            .map(|p| {
                let saved = std::mem::take(&mut self.scopes);
                self.scopes = vec![HashMap::new()];
                let (t, _) = self.param(p, false, false);
                self.scopes = saved;
                t
            })
            .collect();
        quote_spanned! {span=>
            #[track_caller]
            #vis fn #name(#recv, #(#mparams),*) #ret {
                let __kz_receiver = #this;
                #rt::kompozable_block!(#kz_types { #hidden(__kz_receiver, #(#names = #names),*) })
            }
        }
    }

    /// [`member_fn`](Self::member_fn), its body `given` where there is one (an enum class's `match` of its entries).
    pub(crate) fn member_fn_with(&mut self, f: &Function, cx: &MemberCx, given: Option<TokenStream>) -> TokenStream {
        let span = self.span(f.loc);
        for m in &f.modifiers.list {
            match m.kind {
                ModifierKind::Public
                | ModifierKind::Private
                | ModifierKind::Internal
                | ModifierKind::Suspend
                | ModifierKind::Inline
                | ModifierKind::Final
                | ModifierKind::Open
                | ModifierKind::Abstract
                | ModifierKind::Override
                // ((stage 14 WPB) see `operators`)
                | ModifierKind::Operator
                | ModifierKind::Infix => {}
                _ => {
                    self.later(m.loc, &format!("`{}` on a member function", m.kind.keyword()));
                }
            }
        }
        if f.is_kompozable() {
            return self.composable_member(f, cx);
        }
        // ((stage 14 WPD) a member extension, `override fun SceneStrategyScope<T>.calculateScene(…)`: its receiver an
        // API type's value lent (`&SceneStrategyScope<T>`), the parameter `__kz_ext` before the others, the innermost
        // implicit receiver of its body)
        let ext = match &f.receiver {
            Some(r) if !r.nullable && r.simple_name().is_some_and(|n| self.user_class(&n).is_none()) => Some(r.clone()),
            Some(r) => return self.later(r.loc, "a member extension function of a nullable type or the crate's class"),
            None => None,
        };
        let suspend = f.is_suspend();
        // ((stage 14 M3NODE) an override of the API's method giving a boxed trait object Kotlin writes as an interface
        // its class is — `override fun create(…): DelegatableNode = Node(…)` of an `IndicationNodeFactory`, Rust's
        // `Box<dyn ModifierNode>` —: the API's type, the value boxed)
        let api_boxed = self.api_boxed_result(cx, f);
        let ret_text = api_boxed.clone().unwrap_or_else(|| self.member_ret_text(f, cx));
        let ret_ty = Ty::from_rust(&ret_text);
        let unit = ret_ty == Ty::Unit;
        self.fun = FnState {
            kompozable: false,
            nullable_result: ret_ty.is_nullable(),
            shared_result: None,
            result: Some(ret_ty.clone()),
        };
        self.enter_member(cx, &f.name.name, true);
        self.in_async = suspend;
        self.inline_async = false;
        self.collect_stars();
        let mut params = Vec::new();
        let mut converts = Vec::new();
        // ((stage 14 FINAL-T) the receiver lent, used by a lambda kept past the call — an object expression's
        // closures: a clone of it, `__kz___kz_ext`, `use_receiver`)
        let ext_owned: std::rc::Rc<std::cell::Cell<bool>> = Default::default();
        if let Some(r) = &ext {
            let rust_ty = self.rust_type(r, TypePos::Value);
            let name = r.simple_name().map(|n| owner_name(super::last_segment(&n))).unwrap_or_default();
            self.receivers.push(super::resolve::ImplicitReceiver {
                rust: "__kz_ext".into(),
                ty: name,
                value: self.ty_of(r),
                labels: vec![f.name.name.clone()],
                depth: 0,
                by_ref: true,
                by_mut: false,
                owned_used: ext_owned.clone(),
            });
            let i = proc_macro2::Ident::new("__kz_ext", span);
            // ((stage 14 M3NODE) a drawing object Rust lends mutably, `override fun ContentDrawScope.draw()` of a
            // `DrawModifierNode`: `&mut`, as the trait's method takes it)
            if self.lent_mut(r) {
                if let Some(x) = self.receivers.last_mut() {
                    x.by_mut = true;
                }
                params.push(quote_spanned!(span=> #i: &mut #rust_ty));
            } else {
                params.push(quote_spanned!(span=> #i: &#rust_ty));
            }
        }
        for p in &f.params {
            let (tokens, convert) = self.param(p, false, false);
            params.push(tokens);
            converts.extend(convert);
        }
        let generics = self.fn_generics(f, span);
        let ret = match &f.result {
            _ if api_boxed.is_some() => {
                let rt: TokenStream = respan(ret_text.parse().unwrap_or_default(), span);
                quote_spanned!(span=> -> #rt)
            }
            Some(t) if self.ty_of(t) != Ty::Unit => {
                let rt = self.rust_type(t, TypePos::Return);
                quote_spanned!(span=> -> #rt)
            }
            Some(_) => quote!(),
            None => match &f.body {
                Some(FunctionBody::Expr(e)) if !unit => {
                    // (a generic type of the class's own type parameters, `Duo<B, A>`: as inference wrote it)
                    let written: Option<TokenStream> =
                        (!ret_text.contains('_') && ret_text != "()").then(|| ret_text.parse().ok()).flatten();
                    if ret_ty.is_vague()
                        && let Some(t) = written
                    {
                        let s = self.span(e.loc);
                        let t = respan(t, s);
                        quote_spanned!(s=> -> #t)
                    } else if ret_ty.is_vague() {
                        self.error(
                            f.name.loc,
                            format!(
                                "write `{}`'s return type: its expression's type isn't one KompozR can tell (`fun {}(…): Type = …`)",
                                f.name.name, f.name.name
                            ),
                        );
                        quote!()
                    } else {
                        let s = self.span(e.loc);
                        let t = self.rust_of_ty(&ret_ty, s);
                        quote_spanned!(s=> -> #t)
                    }
                }
                _ => quote!(),
            },
        };
        let body = match (&f.body, given) {
            (_, Some(g)) => g,
            (Some(FunctionBody::Block(b)), None) => self.fn_block(b, unit),
            (Some(FunctionBody::Expr(e)), None) => {
                if unit {
                    let s = self.stmt_expr(e);
                    quote!(#s;)
                } else {
                    let want = super::expr::Want { some: ret_ty.is_nullable(), result: true, ..Default::default() };
                    self.value_want(e, want)
                }
            }
            (None, None) => {
                self.error(f.name.loc, "a function here needs a body");
                quote!()
            }
        };
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let companion = self.member_lets(cx, span);
        let vis = self.member_vis(&f.modifiers, f.loc);
        // ((stage 14 WPB) `toString` as `to_string_kt`; one of several overloads mangled)
        let rust_name = self.member_fn_rust(&cx.owner, f);
        let name = self.name_ident(&rust_name, f.name.loc);
        let recv = cx.receiver(span);
        let asyncness = if suspend { quote_spanned!(span=> async) } else { quote!() };
        // ((WPA-rest) an override's parameters it doesn't use: Kotlin says nothing of them)
        let unused = if super::hierarchy::is_override(&f.modifiers) && !f.params.is_empty() {
            let names = f.params.iter().map(|p| self.name_ident(&snake(&p.name.name), p.name.loc));
            quote_spanned!(span=> let _ = (#(&#names,)*);)
        } else {
            quote!()
        };
        let owned = if ext_owned.get() {
            quote_spanned!(span=> let __kz___kz_ext = ::core::clone::Clone::clone(__kz_ext);)
        } else {
            quote!()
        };
        quote_spanned! {span=>
            #vis #asyncness fn #name #generics (#recv, #(#params),*) #ret {
                #[allow(unused_imports)]
                use #kt::ext::*;
                #companion
                #(#converts)*
                #unused
                #owned
                #rt::named_args!(#kz_types { #body })
            }
        }
    }

    /// `let __kz_companion = CCompanion {};` where a companion's members are in scope.
    fn companion_let(&self, cx: &MemberCx, span: Span) -> TokenStream {
        match &cx.companion {
            Some(c) => {
                let ci = proc_macro2::Ident::new(c, span);
                quote_spanned!(span=> #[allow(unused_variables)] let __kz_companion = #ci {};)
            }
            None => quote!(),
        }
    }

    /// What a member's body starts with: the companion's value, an inner class's outer objects.
    pub(crate) fn member_lets(&self, cx: &MemberCx, span: Span) -> TokenStream {
        let mut out = self.companion_let(cx, span);
        for o in &cx.outers {
            let v = proc_macro2::Ident::new(&o.var, span);
            let from: TokenStream = respan(o.from.parse().expect("a path"), span);
            out.extend(quote_spanned!(span=> #[allow(unused_variables)] let #v = #from;));
        }
        out
    }

    /// The outer objects an inner class's members see (none for another class).
    pub(crate) fn outers_of(&self, info: &UserClass) -> Vec<Outer> {
        let mut out = Vec::new();
        let mut cur = info.clone();
        let mut from = "self.__kz_outer()".to_string();
        while let Some(o) = cur.inner.clone() {
            let Some(oc) = self.user_class_rust(&o).cloned() else { break };
            let var = if out.is_empty() { "__kz_outer".to_string() } else { format!("__kz_outer{}", out.len() + 1) };
            let label = oc.kotlin.rsplit('.').next().unwrap_or(&oc.kotlin).to_string();
            let args: Vec<Ty> = oc.type_params.iter().map(|t| Ty::named(t)).collect();
            out.push(Outer {
                var: var.clone(),
                from: from.clone(),
                ty: oc.rust.clone(),
                value: Ty::Named(oc.rust.clone(), args),
                label,
            });
            from = format!("{var}.__kz_outer()");
            cur = oc;
        }
        out
    }

    /// A secondary constructor: `C::__kz_ctorN(…)`, the primary constructor (`: this(…)`) then its body run on the
    /// object.
    fn secondary_ctor(&mut self, s: &SecondaryConstructor, cx: &MemberCx, class: &str) -> TokenStream {
        let span = self.span(s.loc);
        let n = match self.classes.decls.iter().find(|d| d.info.rust == cx.owner).map(|d| &d.ast) {
            Some(ClassAst::Class(k)) => {
                k.body.members.iter().position(|m| matches!(m, Member::Constructor(x) if x.loc == s.loc)).unwrap_or(0)
            }
            _ => 0,
        };
        if s.delegation.as_ref().is_some_and(|d| d.to_super) {
            return self.later(s.loc, "a constructor delegating to `super`");
        }
        let owner = proc_macro2::Ident::new(&cx.owner, span);
        let ctor = format_ident!("__kz_ctor{n}", span = span);
        let body_fn = format_ident!("__kz_ctor_body{n}", span = span);
        // (the delegation's arguments: the parameters in scope, no object yet)
        self.fun = FnState::default();
        self.enter_member(cx, class, false);
        let mut params = Vec::new();
        let mut converts = Vec::new();
        let mut names = Vec::new();
        for p in &s.params {
            let (tokens, convert) = self.param(p, false, false);
            params.push(tokens);
            converts.extend(convert);
            names.push(self.name_ident(&snake(&p.name.name), p.name.loc));
        }
        let primary = Expr {
            kind: ExprKind::Call {
                callee: Box::new(Expr {
                    kind: ExprKind::Name(Ident { name: class.to_string(), backticked: false, loc: s.loc }),
                    loc: s.loc,
                }),
                type_args: Vec::new(),
                args: s.delegation.as_ref().map(|d| d.args.clone()).unwrap_or_default(),
                trailing_lambda: None,
            },
            loc: s.loc,
        };
        let made = self.value(&primary);
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let body = match &s.body {
            Some(b) => {
                let value_params: Vec<TokenStream> = s
                    .params
                    .iter()
                    .map(|p| {
                        let i = ident(&snake(&p.name.name), span);
                        let t = p.ty.as_ref().map(|t| self.rust_type(t, TypePos::Value)).unwrap_or_default();
                        quote_spanned!(span=> #i: #t)
                    })
                    .collect();
                self.enter_member(cx, class, true);
                for p in &s.params {
                    let ty = p.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown);
                    self.declare_at(&p.name, snake(&p.name.name), LocalKind::Param, ty);
                }
                let b = self.fn_block(b, true);
                let companion = self.companion_let(cx, span);
                quote_spanned! {span=>
                    #[doc(hidden)]
                    fn #body_fn(&self, #(#value_params),*) {
                        #[allow(unused_imports)]
                        use #kt::ext::*;
                        #companion
                        #rt::named_args!(#kz_types { #b })
                    }
                }
            }
            None => quote!(),
        };
        let call_body =
            if s.body.is_some() { quote_spanned!(span=> __kz_this.#body_fn(#(#names),*);) } else { quote!() };
        let vis = self.member_vis(&s.modifiers, s.loc);
        quote_spanned! {span=>
            #[doc(hidden)]
            #vis fn #ctor(#(#params),*) -> #owner {
                #[allow(unused_imports)]
                use #kt::ext::*;
                #(#converts)*
                let __kz_this: #owner = #rt::named_args!(#kz_types #made);
                #call_body
                __kz_this
            }
            #body
        }
    }

    /// A plain class: a reference to an object of cells (see the module's doc); an inner one's object holds its
    /// outer one, a subclass's its base class's part.
    pub(crate) fn plain_class_decl(&mut self, d: &ClassDecl) -> TokenStream {
        use super::hierarchy::Place;
        let ClassAst::Class(c) = &d.ast else { return quote!() };
        let c = &**c;
        let info = &d.info;
        let span = self.span(c.loc);
        if !self.check_class(d, c) {
            return quote!();
        }
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let vis = self.class_vis(&c.modifiers, c.loc);
        let name = proc_macro2::Ident::new(&info.rust, self.span(c.name.loc));
        let fields_ty = format_ident!("__Kz{}", info.rust, span = span);
        let allow = class_allow(span);
        let route_attrs = self.plain_route_attrs(c, span);
        let (g_decl, g_use, g_struct) = self.class_generics(d, span);
        let cx = MemberCx {
            owner: info.rust.clone(),
            shape: Shape::Plain,
            companion: info.companion.clone(),
            outers: self.outers_of(info),
        };
        let params: Vec<ConstructorParam> =
            c.primary_constructor.as_ref().map(|p| p.params.clone()).unwrap_or_default();
        self.fun = FnState::default();
        self.enter_member(&cx, &c.name.name, false);
        let mut ctor_params = Vec::new();
        let mut converts = Vec::new();
        let mut names = Vec::new();
        let mut value_params = Vec::new();
        for cp in &params {
            let (ts, convert) = self.param(&cp.param, false, false);
            ctor_params.push(ts);
            converts.extend(convert);
            converts.extend(self.closure_convert(&cp.param));
            let i = self.name_ident(&snake(&cp.param.name.name), cp.param.name.loc);
            let t = cp.param.ty.as_ref().map(|t| self.rust_type(t, TypePos::Value)).unwrap_or_default();
            value_params.push(quote_spanned!(span=> #i: #t));
            names.push(i);
        }
        let mut fields = Vec::new();
        let mut empty = Vec::new();
        let mut getters = Vec::new();
        let mut init = Vec::new();
        // (an inner class's outer object: given to its constructor, held first)
        let outer = match &info.inner {
            Some(o) => {
                let oc = self.user_class_rust(o).cloned();
                let ot = proc_macro2::Ident::new(o, span);
                let og = oc.map(|oc| super::hierarchy::generic_use_names(&oc.type_params, span)).unwrap_or_default();
                fields.push(quote_spanned!(span=> #[doc(hidden)] pub __kz_outer: #ot #og));
                empty.push(quote_spanned!(span=> __kz_outer));
                getters.push(quote_spanned! {span=>
                    #[doc(hidden)]
                    pub fn __kz_outer(&self) -> #ot #og { ::core::clone::Clone::clone(&self.__kz_this.__kz_outer) }
                });
                Some(quote_spanned!(span=> __kz_outer: #ot #og))
            }
            None => None,
        };
        // (a subclass's base class's part)
        if let Some((f, e)) = self.super_field(d, Place::Plain, span) {
            fields.push(f);
            empty.push(e);
        }
        for (f, e) in self.delegate_fields(d, span) {
            fields.push(f);
            empty.push(e);
        }
        // ((stage 14 FINAL-T) type parameters none of its properties holds — `class Strategy<T : Any> :
        // SceneStrategy<T>`, its `T` its interface's —: Rust's marker of them)
        let own_params: Vec<&str> = c.type_params.iter().map(|t| t.name.name.as_str()).collect();
        let held = |n: &str| {
            params.iter().any(|cp| {
                cp.property.is_some() && cp.param.ty.as_ref().is_some_and(|t| super::hierarchy::type_mentions(t, &[n]))
            }) || c.body.members.iter().any(|m| match m {
                Member::Decl(Decl::Property(p)) => {
                    p.ty.as_ref().is_none_or(|t| super::hierarchy::type_mentions(t, &[n]))
                }
                _ => false,
            })
        };
        let unheld: Vec<proc_macro2::Ident> =
            own_params.iter().filter(|n| !held(n)).map(|n| proc_macro2::Ident::new(n, span)).collect();
        if !unheld.is_empty() {
            fields.push(quote_spanned!(span=> #[doc(hidden)] pub __kz_types: ::core::marker::PhantomData<fn() -> (#(#unheld,)*)>));
            empty.push(quote_spanned!(span=> __kz_types: ::core::marker::PhantomData));
        }
        for cp in params.iter().filter(|cp| cp.property.is_some()) {
            let pspan = self.span(cp.param.loc);
            let i = self.name_ident(&snake(&cp.param.name.name), cp.param.name.loc);
            let t = cp.param.ty.as_ref().map(|t| self.rust_type(t, TypePos::Value)).unwrap_or_default();
            let fvis = self.member_vis(&cp.param.modifiers, cp.param.loc);
            let var = cp.property == Some(Mutability::Var);
            fields.push(self.cell_field(&i, &t, var, pspan));
            empty.push(self.cell_empty(&i, var, pspan));
            getters.push(self.cell_accessors(&i, &t, var, &fvis, quote_spanned!(pspan=> self.__kz_this), pspan));
            // (a value the body's initializers read too: a clone of it held)
            let copied = cp.param.ty.as_ref().is_some_and(|t| self.copied(&self.ty_of(t)));
            let v = if copied {
                quote_spanned!(pspan=> #i)
            } else {
                quote_spanned!(pspan=> ::core::clone::Clone::clone(&#i))
            };
            init.push(if var {
                quote_spanned!(pspan=> self.__kz_this.#i.set(#v);)
            } else {
                quote_spanned!(pspan=> self.__kz_this.#i.init(#v);)
            });
        }
        // The body in order: the base class's constructor, initializers and init blocks in `__kz_init`, the object
        // `self`.
        self.enter_member(&cx, &c.name.name, true);
        for cp in &params {
            let ty = cp.param.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown);
            // ((stage 14 FINAL-T) a function's: the shared closure `__kz_init` is given, `value_params`)
            let ty = match ty {
                Ty::Fun(mut f) => {
                    f.shared = true;
                    Ty::Fun(f)
                }
                t => t,
            };
            self.declare_at(&cp.param.name, snake(&cp.param.name.name), LocalKind::Param, ty);
        }
        let super_init = self.super_init(d, quote_spanned!(span=> self), Place::Plain, span);
        // ((stage 14 WPB) an exception class: the message and cause its base is given, first)
        let overrides_to_string =
            c.body.members.iter().any(
                |m| matches!(m, Member::Decl(Decl::Function(f)) if f.name.name == "toString" && f.params.is_empty()),
            );
        let exception = match self.exception_class_parts(c, &info.rust, overrides_to_string, span) {
            Some(x) => Some(x),
            // ((WPA-rest) one deriving from the user's open exception class: its base part has them)
            None => self.exception_chain_parts(d, &info.rust, overrides_to_string, span),
        };
        if let Some(x) = &exception
            && !x.field.is_empty()
        {
            fields.push(x.field.clone());
            empty.push(x.empty.clone());
            init.insert(0, x.init.clone());
            getters.push(x.members.clone());
        }
        let mut methods = Vec::new();
        let mut own_to_string = false;
        let members = c.body.members.clone();
        let mut body_init = Vec::new();
        for m in &members {
            match m {
                Member::Init(b) => {
                    // ((stage 14 WPB) the object being made: a kept lambda holds it weakly, `capture`)
                    self.control.in_init = true;
                    let s = self.braced(b, super::stmt::Tail::Unit);
                    self.control.in_init = false;
                    body_init.push(quote_spanned!(span=> #s));
                }
                Member::Decl(Decl::Property(p)) => {
                    self.control.in_init = true;
                    let made = self.cell_property(p, &cx, quote_spanned!(span=> self.__kz_this));
                    self.control.in_init = false;
                    if let Some((f, e, g, i)) = made {
                        fields.push(f);
                        empty.push(e);
                        getters.push(g);
                        body_init.extend(i);
                    }
                }
                Member::Decl(Decl::Function(f)) => {
                    own_to_string |= f.name.name == "toString" && f.params.is_empty();
                    let saved = std::mem::take(&mut self.scopes);
                    let saved_r = std::mem::take(&mut self.receivers);
                    methods.push(self.member_fn(f, &cx));
                    self.scopes = saved;
                    self.receivers = saved_r;
                }
                Member::Constructor(s) => {
                    let saved = std::mem::take(&mut self.scopes);
                    let saved_r = std::mem::take(&mut self.receivers);
                    methods.push(self.secondary_ctor(s, &cx, &c.name.name));
                    self.scopes = saved;
                    self.receivers = saved_r;
                }
                Member::Decl(Decl::Class(_) | Decl::Object(_) | Decl::TypeAlias(_)) => {}
                Member::Decl(d) => {
                    self.later(d.loc(), "this member");
                }
            }
        }
        methods.extend(self.inner_ctors(info));
        // ((stage 14 WPB) a property of no field of its own: none)
        fields.retain(|f| !f.is_empty());
        empty.retain(|f| !f.is_empty());
        let lets = self.member_lets(&cx, span);
        let companion_const = self.companion_const(info, span);
        let kt_name = &c.name.name;
        let display = if let Some(x) = &exception {
            x.display.clone()
        } else if own_to_string {
            quote_spanned!(span=> __kz_out.push_str(&self.to_string_kt());)
        } else {
            quote_spanned!(span=> __kz_out.push_str(&#kt::default_to_string(#kt_name, ::std::rc::Rc::as_ptr(&self.__kz_this) as *const ())))
        };
        let new_params: Vec<TokenStream> = outer.iter().cloned().chain(value_params.iter().cloned()).collect();
        let impls = self.class_impls(d, Place::Plain, &g_decl, &g_use);
        // (a ViewModel's object made by its store's factory: Kotlin's constructor taking nothing)
        let default_new = if names.is_empty() && outer.is_none() && self.derives_from(d, "ViewModel") {
            quote_spanned! {span=>
                #[doc(hidden)]
                #vis fn __kz_new_default() -> #name #g_use { #name::__kz_new() }
            }
        } else {
            quote!()
        };
        let view_model = if self.derives_from(d, "ViewModel") {
            quote_spanned! {span=>
                #allow
                impl #g_decl ViewModelRef for #name #g_use {
                    type Model = #name #g_use;
                    fn from_rc(model: ::std::rc::Rc<#name #g_use>) -> #name #g_use {
                        ::core::clone::Clone::clone(&*model)
                    }
                }
            }
        } else {
            quote!()
        };
        let ctor_fn = if outer.is_some() {
            quote!()
        } else {
            quote_spanned! {span=>
                #allow
                #[#rt::named(type)]
                #vis fn #name #g_decl (#(#ctor_params),*) -> #name #g_use {
                    #(#converts)*
                    #name::__kz_new(#(#names),*)
                }
            }
        };
        let exception_impls = exception.map(|x| x.impls);
        // ((stage 14 WPB) `equals`/`hashCode` overridden: Kotlin's `==` and hash are theirs, else the identity's)
        let (eq_body, hash_body) = self.equality_bodies(
            &c.body.members,
            quote_spanned!(span=> ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)),
            quote_spanned!(span=> ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)),
            span,
        );
        quote_spanned! {span=>
            #exception_impls

            #allow
            #[doc(hidden)]
            #vis struct #fields_ty #g_struct { #(#fields),* }

            #allow
            #[derive(::core::clone::Clone)]
            #route_attrs
            #vis struct #name #g_struct { #[doc(hidden)] pub __kz_this: ::std::rc::Rc<#fields_ty #g_use> }

            #ctor_fn

            #allow
            impl #g_decl #name #g_use {
                #[doc(hidden)]
                #vis fn __kz_new(#(#new_params),*) -> #name #g_use {
                    let __kz_this = #name { __kz_this: ::std::rc::Rc::new(#fields_ty { #(#empty),* }) };
                    __kz_this.__kz_init(#(#names),*);
                    __kz_this
                }
                #default_new
                #[doc(hidden)]
                #[allow(unused_variables)]
                fn __kz_init(&self, #(#value_params),*) {
                    #[allow(unused_imports)]
                    use #kt::ext::*;
                    #lets
                    #super_init
                    #(#init)*
                    #rt::named_args!(#kz_types { #(#body_init)* })
                }
                #(#getters)*
                #companion_const
            }

            #allow
            #[#rt::named]
            impl #g_decl #name #g_use {
                #(#methods)*
            }

            #allow
            impl #g_decl ::core::cmp::PartialEq for #name #g_use {
                fn eq(&self, other: &Self) -> bool {
                    #eq_body
                }
            }
            #allow
            impl #g_decl ::core::cmp::Eq for #name #g_use {}
            #allow
            impl #g_decl ::core::hash::Hash for #name #g_use {
                fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                    #hash_body
                }
            }
            #allow
            impl #g_decl #kt::KtToString for #name #g_use {
                fn kt_write(&self, __kz_out: &mut ::std::string::String) {
                    #display
                }
            }
            #allow
            impl #g_decl ::core::fmt::Display for #name #g_use {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.write_str(&#kt::KtToString::kt_to_string(self))
                }
            }
            #allow
            impl #g_decl ::core::fmt::Debug for #name #g_use {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.write_str(&#kt::KtToString::kt_to_string(self))
                }
            }

            #impls
            #view_model
        }
    }

    /// Whether a class derives from a base class of the API (by its Kotlin name), directly or not.
    pub(crate) fn derives_from(&self, d: &ClassDecl, base: &str) -> bool {
        self.ancestors(d).iter().any(|a| matches!(&a.kind, super::hierarchy::SuperKind::Base(b) if b.kotlin == base))
    }
    pub(crate) fn cell_field(&self, i: &proc_macro2::Ident, t: &TokenStream, var: bool, span: Span) -> TokenStream {
        let kt = self.kt(span);
        if var { quote_spanned!(span=> #i: #kt::KtVar<#t>) } else { quote_spanned!(span=> #i: #kt::KtVal<#t>) }
    }

    pub(crate) fn cell_empty(&self, i: &proc_macro2::Ident, var: bool, span: Span) -> TokenStream {
        let kt = self.kt(span);
        if var { quote_spanned!(span=> #i: #kt::KtVar::new()) } else { quote_spanned!(span=> #i: #kt::KtVal::new()) }
    }

    /// A cell property's getter (and setter): `x()` reading `place.x`.
    fn cell_accessors(
        &self,
        i: &proc_macro2::Ident,
        t: &TokenStream,
        var: bool,
        vis: &TokenStream,
        place: TokenStream,
        span: Span,
    ) -> TokenStream {
        let setter = format_ident!("set_{}", i.to_string().trim_start_matches("r#"), span = span);
        let get = quote_spanned!(span=> #vis fn #i(&self) -> #t { #place.#i.get() });
        if var { quote_spanned!(span=> #get #vis fn #setter(&self, value: #t) { #place.#i.set(value) }) } else { get }
    }

    /// A body property of a class or object held in a cell: its field, its empty cell, its accessors, and its
    /// initialization (in `__kz_init`); a getter-only one a method.
    pub(crate) fn cell_property(
        &mut self,
        p: &Property,
        cx: &MemberCx,
        place: TokenStream,
    ) -> Option<(TokenStream, TokenStream, TokenStream, Option<TokenStream>)> {
        let Binding::Name(n) = &p.binding else {
            self.later(p.loc, "a destructuring member");
            return None;
        };
        let pspan = self.span(p.loc);
        if p.getter.is_some() && matches!(p.init, PropertyInit::None) && p.setter.is_none() {
            let m = self.getter_member(p, cx);
            return Some((quote!(), quote!(), m, None));
        }
        // ((stage 14 WPB) by a delegate, with accessors, `lateinit`: `property`)
        if p.is_delegated() || p.getter.is_some() || p.setter.is_some() || p.modifiers.has(ModifierKind::Lateinit) {
            return self.member_special_property(p, cx, place);
        }
        let PropertyInit::Value(init) = &p.init else {
            self.error(n.loc, format!("`{}` needs a value", n.name));
            return None;
        };
        let ty = match &p.ty {
            Some(t) => self.ty_of(t),
            None => self.peek_ty(init),
        };
        let t = match &p.ty {
            Some(t) => self.rust_type(t, TypePos::Value),
            None if !ty.is_vague() => self.rust_of_ty(&ty, pspan),
            None => {
                self.error(n.loc, format!("write `{}`'s type: KompozR can't tell its value's type", n.name));
                return None;
            }
        };
        let i = self.name_ident(&snake(&n.name), n.loc);
        let var = p.is_var();
        let v = if let Some(v) = self.lambda_property_value(p, init) {
            v
        } else if ty.is_nullable() {
            self.expecting(init, &ty, |g| g.value_want(init, super::expr::Want { some: true, ..Default::default() }))
        } else {
            self.expecting(init, &ty, |g| g.value(init))
        };
        // ((stage 14 LISTS) a mutable collection held by a read-only type's property, a list by a view's)
        let from = self.peek_ty(init);
        let v = match self.collection_coerce(v, &from, &ty, pspan) {
            Ok(v) | Err(v) => v,
        };
        let fvis = self.member_vis(&p.modifiers, p.loc);
        let field = self.cell_field(&i, &t, var, pspan);
        let empty = self.cell_empty(&i, var, pspan);
        let acc = self.cell_accessors(&i, &t, var, &fvis, place.clone(), pspan);
        let set =
            if var { quote_spanned!(pspan=> #place.#i.set(#v);) } else { quote_spanned!(pspan=> #place.#i.init(#v);) };
        Some((field, empty, acc, Some(set)))
    }

    /// An object declaration (or a companion): `struct O {}` and its members as methods, its state the thread's
    /// (made at its first use and kept: `&'static`), a base class's part in it.
    fn object(&mut self, o: &Object, d: &ClassDecl) -> TokenStream {
        use super::hierarchy::Place;
        let info = &d.info;
        let span = self.span(o.loc);
        for m in &o.modifiers.list {
            match m.kind {
                ModifierKind::Public
                | ModifierKind::Private
                | ModifierKind::Internal
                | ModifierKind::Data
                | ModifierKind::Companion => {}
                _ => {
                    self.later(m.loc, &format!("`{}` on an object", m.kind.keyword()));
                }
            }
        }
        if self.supers_of(d).len() < o.supers.len() {
            for s in &o.supers {
                if s.ty.simple_name().is_none_or(|n| self.super_kind(&n).is_none()) {
                    self.error(
                        s.loc,
                        format!(
                            "`{}` isn't a class or interface KompozR knows",
                            s.ty.simple_name().unwrap_or_default()
                        ),
                    );
                }
            }
            return quote!();
        }
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let vis = self.class_vis(&o.modifiers, o.loc);
        let name = proc_macro2::Ident::new(&info.rust, self.span(o.name.as_ref().map_or(o.loc, |n| n.loc)));
        let fields_ty = format_ident!("__Kz{}", info.rust, span = span);
        let allow = class_allow(span);
        let route = self.route_derive(&o.annotations, span);
        let cx = MemberCx { owner: info.rust.clone(), shape: Shape::Object, companion: None, outers: Vec::new() };
        self.fun = FnState::default();
        self.enter_member(&cx, &info.rust, true);
        let place = quote_spanned!(span=> Self::__kz_state());
        let mut fields = Vec::new();
        let mut empty = Vec::new();
        let mut getters = Vec::new();
        let mut init = Vec::new();
        let mut methods = Vec::new();
        let mut own_to_string = false;
        if let Some((f, e)) = self.super_field(d, Place::Object, span) {
            fields.push(f);
            empty.push(e);
        }
        for (f, e) in self.delegate_fields(d, span) {
            fields.push(f);
            empty.push(e);
        }
        init.push(self.super_init(d, quote_spanned!(span=> self), Place::Object, span));
        for m in &o.body.members {
            match m {
                Member::Init(b) => {
                    let s = self.braced(b, super::stmt::Tail::Unit);
                    init.push(quote_spanned!(span=> #s));
                }
                Member::Decl(Decl::Property(p)) => {
                    if let Some((f, e, g, i)) = self.cell_property(p, &cx, place.clone()) {
                        fields.push(f);
                        empty.push(e);
                        getters.push(g);
                        init.extend(i);
                    }
                }
                // ((stage 14 FINAL-T) an override given its base's struct: the trait's method's, `callbacks`)
                Member::Decl(Decl::Function(f)) if self.handler_override(&d.info.rust, f) => {}
                Member::Decl(Decl::Function(f)) => {
                    own_to_string |= f.name.name == "toString" && f.params.is_empty();
                    let saved = std::mem::take(&mut self.scopes);
                    let saved_r = std::mem::take(&mut self.receivers);
                    methods.push(self.member_fn(f, &cx));
                    self.scopes = saved;
                    self.receivers = saved_r;
                }
                Member::Decl(Decl::Class(_) | Decl::Object(_) | Decl::TypeAlias(_)) => {}
                Member::Decl(d) => {
                    self.later(d.loc(), "this member");
                }
                Member::Constructor(s) => {
                    self.error(s.loc, "an object has no constructors");
                }
            }
        }
        let fields: Vec<TokenStream> = fields.into_iter().filter(|f| !f.is_empty()).collect();
        let empty: Vec<TokenStream> = empty.into_iter().filter(|f| !f.is_empty()).collect();
        let shown = if info.shape == Shape::Companion {
            format!("{}$Companion", info.kotlin.rsplit_once('.').map_or("", |(o, _)| o))
        } else {
            info.kotlin.clone()
        };
        let display = if own_to_string {
            quote_spanned!(span=> __kz_out.push_str(&self.to_string_kt());)
        } else if o.is_data() {
            // (a data object's text: its own name)
            let own = shown.rsplit('.').next().unwrap_or(&shown).to_string();
            quote_spanned!(span=> __kz_out.push_str(#own);)
        } else {
            quote_spanned!(span=> __kz_out.push_str(&#kt::default_to_string(#shown, Self::__kz_state() as *const #fields_ty as *const ()));)
        };
        let impls = self.class_impls(d, Place::Object, &quote!(), &quote!());
        let serializer = self.serializer_fn(&route, &vis, quote!(#allow impl #name), span);
        quote_spanned! {span=>
            #allow
            #[doc(hidden)]
            #vis struct #fields_ty { #(#fields),* }

            #allow
            #[derive(::core::clone::Clone, ::core::marker::Copy, ::core::cmp::PartialEq, ::core::cmp::Eq, ::core::hash::Hash, ::core::default::Default)]
            #route
            #vis struct #name {}

            #serializer

            #allow
            impl #name {
                #[doc(hidden)]
                fn __kz_state() -> &'static #fields_ty {
                    ::std::thread_local! {
                        static __KZ_STATE: #kt::KtSingleton<#fields_ty> = const { #kt::KtSingleton::new() };
                    }
                    __KZ_STATE.with(|s| s.get(|| #fields_ty { #(#empty),* }, |_| #name {}.__kz_init()))
                }
                #[doc(hidden)]
                fn __kz_init(&self) {
                    #[allow(unused_imports)]
                    use #kt::ext::*;
                    #rt::named_args!(#kz_types { #(#init)* })
                }
                #(#getters)*
            }

            #allow
            #[#rt::named]
            impl #name {
                #(#methods)*
            }

            #allow
            impl #kt::KtToString for #name {
                fn kt_write(&self, __kz_out: &mut ::std::string::String) {
                    #display
                }
            }
            #allow
            impl ::core::fmt::Display for #name {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.write_str(&#kt::KtToString::kt_to_string(self))
                }
            }
            #allow
            impl ::core::fmt::Debug for #name {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.write_str(&#kt::KtToString::kt_to_string(self))
                }
            }

            #impls
        }
    }
    /// An enum class (see the module's doc): its entries' bodies are arms of its members' `match`es (an entry's
    /// override; the class's own member for the others), its interfaces implemented through them.
    fn enum_class(&mut self, c: &Class, d: &ClassDecl) -> TokenStream {
        use super::hierarchy::Place;
        let info = &d.info;
        let span = self.span(c.loc);
        for m in &c.modifiers.list {
            if !matches!(m.kind, ModifierKind::Public | ModifierKind::Private | ModifierKind::Internal) {
                self.later(m.loc, &format!("`{}` on an enum class", m.kind.keyword()));
            }
        }
        if let Some(s) = c.supers.iter().find(|s| {
            s.ty.simple_name().and_then(|n| self.super_kind(&n)).is_none_or(|k| {
                !matches!(k, super::hierarchy::SuperKind::Interface(_) | super::hierarchy::SuperKind::Api(_))
            })
        }) {
            return self.later(s.loc, "an enum class's supertype that isn't an interface");
        }
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let vis = self.class_vis(&c.modifiers, c.loc);
        let name = proc_macro2::Ident::new(&info.rust, self.span(c.name.loc));
        let allow = class_allow(span);
        let cx = MemberCx {
            owner: info.rust.clone(),
            shape: Shape::Enum,
            companion: info.companion.clone(),
            outers: Vec::new(),
        };
        let kt_name = &c.name.name;
        let mut variants = Vec::new();
        let mut names = Vec::new();
        let mut parse = Vec::new();
        for e in &c.enum_entries {
            let v = self.name_ident(&e.name.name, e.name.loc);
            let text = &e.name.name;
            variants.push(quote!(#v));
            names.push(quote_spanned!(span=> #name::#v => #text));
            parse.push(quote_spanned!(span=> #text => #name::#v));
        }
        // (what the entries' bodies override, by member name: (entry, its member))
        let mut entry_fns: HashMap<String, Vec<(Ident, Function)>> = HashMap::new();
        let mut entry_strings: Vec<(Ident, Function)> = Vec::new();
        let mut entry_own: Vec<Function> = Vec::new();
        for e in &c.enum_entries {
            let Some(b) = &e.body else { continue };
            for m in &b.members {
                match m {
                    Member::Decl(Decl::Function(f)) if f.name.name == "toString" && f.params.is_empty() => {
                        entry_strings.push((e.name.clone(), (**f).clone()));
                    }
                    Member::Decl(Decl::Function(f)) if super::hierarchy::is_override(&f.modifiers) => {
                        entry_fns.entry(f.name.name.clone()).or_default().push((e.name.clone(), (**f).clone()));
                    }
                    Member::Decl(Decl::Property(p)) if super::hierarchy::is_override(&p.modifiers) => {
                        let (Binding::Name(n), PropertyInit::Value(v)) = (&p.binding, &p.init) else {
                            self.later(p.loc, "this property of an enum entry");
                            continue;
                        };
                        let f = Function {
                            annotations: Vec::new(),
                            modifiers: p.modifiers.clone(),
                            type_params: Vec::new(),
                            receiver: None,
                            name: n.clone(),
                            params: Vec::new(),
                            result: p.ty.clone(),
                            constraints: Vec::new(),
                            body: Some(FunctionBody::Expr(v.clone())),
                            loc: p.loc,
                        };
                        entry_fns.entry(n.name.clone()).or_default().push((e.name.clone(), f));
                    }
                    // (an entry's own function: a method of the enum only its body calls, `__kz_<entry>_<name>`)
                    Member::Decl(Decl::Function(f)) if f.receiver.is_none() => {
                        let mut own = (**f).clone();
                        own.name.name = entry_own_name(&e.name.name, &f.name.name);
                        entry_own.push(own);
                    }
                    _ => {
                        self.later(m_loc(m), "an enum entry's member that overrides none of its class's");
                    }
                }
            }
        }
        // The constructor's properties: a getter matching the entries' arguments (or the parameter's default).
        let params: Vec<ConstructorParam> =
            c.primary_constructor.as_ref().map(|p| p.params.clone()).unwrap_or_default();
        self.fun = FnState::default();
        self.enter_member(&cx, &c.name.name, false);
        let mut getters = Vec::new();
        for (k, cp) in params.iter().enumerate() {
            if cp.property.is_none() {
                continue;
            }
            if cp.property == Some(Mutability::Var) {
                self.later(cp.param.loc, "an enum class's `var` property");
                continue;
            }
            let pspan = self.span(cp.param.loc);
            let i = self.name_ident(&snake(&cp.param.name.name), cp.param.name.loc);
            let t = cp.param.ty.as_ref().map(|t| self.rust_type(t, TypePos::Value)).unwrap_or_default();
            let pty = cp.param.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown);
            let fvis = self.member_vis(&cp.param.modifiers, cp.param.loc);
            let mut arms = Vec::new();
            for e in &c.enum_entries {
                let v = self.name_ident(&e.name.name, e.name.loc);
                let arg = e
                    .args
                    .iter()
                    .find(|a| a.name.as_ref().is_some_and(|n| n.name == cp.param.name.name))
                    .or_else(|| e.args.iter().take_while(|a| a.name.is_none()).nth(k))
                    .map(|a| a.value.clone())
                    .or_else(|| cp.param.default.clone());
                let value = match arg {
                    Some(a) => {
                        let want = super::expr::Want { some: pty.is_nullable(), ..Default::default() };
                        let v = self.value_want(&a, want);
                        if pty == Ty::String { quote_spanned!(pspan=> ::std::string::String::from(#v)) } else { v }
                    }
                    None => {
                        self.error(e.loc, format!("`{}` gives no `{}`", e.name.name, cp.param.name.name));
                        quote_spanned!(pspan=> ::core::unreachable!())
                    }
                };
                arms.push(quote_spanned!(pspan=> #name::#v => #value));
            }
            getters.push(quote_spanned! {pspan=>
                #fvis fn #i(self) -> #t {
                    #[allow(unused_imports)]
                    use #kt::ext::*;
                    #rt::named_args!(#kz_types match self { #(#arms),* })
                }
            });
        }
        let mut methods = Vec::new();
        for m in &c.body.members {
            match m {
                Member::Decl(Decl::Function(f)) => {
                    let saved = std::mem::take(&mut self.scopes);
                    let saved_r = std::mem::take(&mut self.receivers);
                    let overrides = entry_fns.remove(&f.name.name).unwrap_or_default();
                    methods.push(self.enum_member(c, f, &overrides, &cx));
                    self.scopes = saved;
                    self.receivers = saved_r;
                }
                Member::Decl(Decl::Property(p)) if p.getter.is_some() || matches!(p.init, PropertyInit::Value(_)) => {
                    // (a computed property: Kotlin's value made once per entry, here each time it is read)
                    let Binding::Name(n) = &p.binding else { continue };
                    if p.is_var() {
                        self.later(p.loc, "an enum class's `var` property");
                        continue;
                    }
                    let body = match (&p.init, &p.getter) {
                        (PropertyInit::Value(e), _) => FunctionBody::Expr(e.clone()),
                        (_, Some(Accessor { body: Some(b), .. })) => b.clone(),
                        _ => continue,
                    };
                    let f = Function {
                        annotations: Vec::new(),
                        modifiers: p.modifiers.clone(),
                        type_params: Vec::new(),
                        receiver: None,
                        name: n.clone(),
                        params: Vec::new(),
                        result: p.ty.clone(),
                        constraints: Vec::new(),
                        body: Some(body),
                        loc: p.loc,
                    };
                    let saved = std::mem::take(&mut self.scopes);
                    let saved_r = std::mem::take(&mut self.receivers);
                    let overrides = entry_fns.remove(&n.name).unwrap_or_default();
                    methods.push(self.enum_member(c, &f, &overrides, &cx));
                    self.scopes = saved;
                    self.receivers = saved_r;
                }
                Member::Decl(Decl::Class(_) | Decl::Object(_) | Decl::TypeAlias(_)) => {}
                Member::Decl(d) => {
                    self.later(d.loc(), "this member of an enum class");
                }
                Member::Init(b) => {
                    self.later(b.loc, "an enum class's `init` block");
                }
                Member::Constructor(s) => {
                    self.later(s.loc, "an enum class's secondary constructor");
                }
            }
        }
        for (n, fs) in entry_fns {
            if let Some((_, f)) = fs.first() {
                self.error(f.name.loc, format!("`{n}` overrides none of the enum class's members"));
            }
        }
        methods.extend(self.inner_ctors(info));
        for f in &entry_own {
            let saved = (std::mem::take(&mut self.scopes), std::mem::take(&mut self.receivers));
            methods.push(self.member_fn(f, &cx));
            (self.scopes, self.receivers) = saved;
        }
        // (Kotlin's `toString()`: an entry's own, else its name)
        let mut text_arms = Vec::new();
        for (e, f) in &entry_strings {
            let v = self.name_ident(&e.name, e.loc);
            self.fun = FnState::default();
            self.enter_member(&cx, &f.name.name, true);
            let body = match &f.body {
                Some(FunctionBody::Block(b)) => self.fn_block(b, false),
                Some(FunctionBody::Expr(x)) => self.value(x),
                None => quote!(),
            };
            text_arms.push(quote_spanned!(span=> #name::#v => {
                #[allow(unused_imports)]
                use #kt::ext::*;
                let s: ::std::string::String = #rt::named_args!(#kz_types { #body });
                __kz_out.push_str(&s)
            }));
        }
        let companion_const = self.companion_const(info, span);
        let missing = format!("No enum constant {kt_name}.");
        let entries = variants.iter().map(|v| quote_spanned!(span=> #name::#v));
        let entries2 = entries.clone();
        let impls = self.class_impls(d, Place::Enum, &quote!(), &quote!());
        let text = if text_arms.is_empty() {
            quote_spanned!(span=> __kz_out.push_str(&self.name());)
        } else {
            quote_spanned! {span=>
                #[allow(unreachable_patterns)]
                match self {
                    #(#text_arms,)*
                    _ => __kz_out.push_str(&self.name()),
                }
            }
        };
        let display = if text_arms.is_empty() {
            quote_spanned!(span=> f.write_str(&self.name()))
        } else {
            quote_spanned!(span=> f.write_str(&#kt::KtToString::kt_to_string(self)))
        };
        quote_spanned! {span=>
            #allow
            #[derive(::core::clone::Clone, ::core::marker::Copy, ::core::cmp::PartialEq, ::core::cmp::Eq, ::core::hash::Hash, ::core::cmp::PartialOrd, ::core::cmp::Ord, ::core::fmt::Debug)]
            #vis enum #name { #(#variants),* }

            #allow
            impl #name {
                /// The entry's name (Kotlin's `name`).
                #vis fn name(self) -> ::std::string::String {
                    ::std::string::String::from(match self { #(#names),* })
                }
                /// Its place among the entries (Kotlin's `ordinal`).
                #vis fn ordinal(self) -> i32 {
                    self as i32
                }
                /// The entries in order (Kotlin's `entries`).
                #vis fn entries() -> ::std::vec::Vec<#name> {
                    ::std::vec![#(#entries),*]
                }
                /// The entries in order (Kotlin's `values()`).
                #vis fn values() -> ::std::vec::Vec<#name> {
                    ::std::vec![#(#entries2),*]
                }
                /// The entry of a name (Kotlin's `valueOf`: an IllegalArgumentException for another name).
                #[track_caller]
                #vis fn value_of(value: impl ::core::convert::Into<::std::string::String>) -> #name {
                    let value: ::std::string::String = value.into();
                    match value.as_str() {
                        #(#parse,)*
                        _ => #kt::panic_kt("IllegalArgumentException", &::std::format!("{}{}", #missing, value)),
                    }
                }
                #(#getters)*
                #companion_const
            }

            #allow
            #[#rt::named]
            impl #name {
                #(#methods)*
            }

            #allow
            impl #kt::KtToString for #name {
                fn kt_write(&self, __kz_out: &mut ::std::string::String) {
                    #text
                }
            }
            #allow
            impl ::core::fmt::Display for #name {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    #display
                }
            }

            // ((stage 14 WPD) Kotlin's enums are Serializable: saved by their names, as a Bundle keeps one —
            // `rememberSaveable { mutableStateOf(Choice.A) }`)
            #allow
            impl #rt::Saveable for #name {
                fn save(&self) -> #rt::SavedValue {
                    #rt::SavedValue::String(self.name())
                }
                fn restore(saved: &#rt::SavedValue) -> ::core::option::Option<Self> {
                    let name = saved.as_str()?;
                    Self::entries().into_iter().find(|e| e.name() == name)
                }
            }

            #impls
        }
    }

    /// An enum class's member function: its body, or, where entries override it, a `match` of the entries (each
    /// overriding one's body, its parameters named its own way bound to the class's; the class's body for the
    /// others, an abstract one's none).
    fn enum_member(&mut self, c: &Class, f: &Function, overrides: &[(Ident, Function)], cx: &MemberCx) -> TokenStream {
        if overrides.is_empty() && f.body.is_some() {
            return self.member_fn(f, cx);
        }
        let span = self.span(f.loc);
        let name = proc_macro2::Ident::new(&cx.owner, span);
        // (the class's signature; an abstract function's result type from its declaration, else from an override)
        let mut sig = f.clone();
        if sig.result.is_none() && sig.body.is_none() {
            sig.result = overrides.iter().find_map(|(_, o)| o.result.clone());
        }
        let mut arms = Vec::new();
        for (e, o) in overrides {
            let v = self.name_ident(&e.name, e.loc);
            let mut body_fn = o.clone();
            // (its parameters bound to the class's names)
            let binds: Vec<TokenStream> = o
                .params
                .iter()
                .zip(&f.params)
                .filter(|(a, b)| a.name.name != b.name.name)
                .map(|(a, b)| {
                    let ai = ident(&snake(&a.name.name), span);
                    let bi = ident(&snake(&b.name.name), span);
                    quote_spanned!(span=> let #ai = #bi;)
                })
                .collect();
            body_fn.params = o.params.clone();
            arms.push((v, binds, body_fn));
        }
        let mut written = Vec::new();
        let ret_text = self.member_ret_text(&sig, cx);
        let ret_ty = Ty::from_rust(&ret_text);
        let unit = ret_ty == Ty::Unit;
        for (v, binds, o) in &arms {
            self.fun = FnState {
                kompozable: false,
                nullable_result: ret_ty.is_nullable(),
                shared_result: None,
                result: Some(ret_ty.clone()),
            };
            self.enter_member(cx, &f.name.name, true);
            for p in &o.params {
                let ty = p.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown);
                self.declare_at(&p.name, snake(&p.name.name), LocalKind::Param, ty);
            }
            let body = self.enum_arm_body(o, unit, &ret_ty);
            written.push(quote_spanned!(span=> #name::#v => { #(#binds)* #body }));
        }
        let default = match &f.body {
            Some(_) => {
                self.fun = FnState {
                    kompozable: false,
                    nullable_result: ret_ty.is_nullable(),
                    shared_result: None,
                    result: Some(ret_ty.clone()),
                };
                self.enter_member(cx, &f.name.name, true);
                for p in &f.params {
                    let ty = p.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown);
                    self.declare_at(&p.name, snake(&p.name.name), LocalKind::Param, ty);
                }
                let body = self.enum_arm_body(f, unit, &ret_ty);
                quote_spanned!(span=> _ => { #body })
            }
            None => {
                let missing: Vec<&EnumEntry> =
                    c.enum_entries.iter().filter(|e| !overrides.iter().any(|(n, _)| n.name == e.name.name)).collect();
                if let Some(e) = missing.first() {
                    self.error(e.name.loc, format!("`{}` doesn't implement `{}`", e.name.name, f.name.name));
                }
                quote_spanned!(span=> _ => ::core::unreachable!())
            }
        };
        // (the function itself: the class's parameters and result, a `match` of the entries for its body)
        // (its body for its result's type: the class's, else an entry's)
        let typing = sig.body.clone().or_else(|| overrides.first().and_then(|(_, o)| o.body.clone()));
        let synth = Function {
            body: typing.or_else(|| Some(FunctionBody::Block(Block { stmts: Vec::new(), braced: true, loc: f.loc }))),
            ..sig
        };
        let arms_ts = quote_spanned!(span=> #[allow(unreachable_patterns)] match self { #(#written,)* #default });
        self.member_fn_with(&synth, cx, Some(arms_ts))
    }

    /// An enum arm's body: a function's block or expression, as its result.
    fn enum_arm_body(&mut self, f: &Function, unit: bool, ret: &Ty) -> TokenStream {
        let kt = self.kt(self.span(f.loc));
        let rt = respan(self.rt().clone(), self.span(f.loc));
        let kz_types = self.types_arg(self.span(f.loc));
        let body = match &f.body {
            Some(FunctionBody::Block(b)) => self.fn_block(b, unit),
            Some(FunctionBody::Expr(e)) if unit => {
                let s = self.stmt_expr(e);
                quote!(#s;)
            }
            Some(FunctionBody::Expr(e)) => {
                let want = super::expr::Want { some: ret.is_nullable(), result: true, ..Default::default() };
                self.value_want(e, want)
            }
            None => quote!(),
        };
        quote! {
            #[allow(unused_imports)]
            use #kt::ext::*;
            #rt::named_args!(#kz_types { #body })
        }
    }

    /// `typealias N<T> = Type`: `pub type N<T> = Type;`.
    pub(crate) fn type_alias(&mut self, a: &TypeAlias) -> TokenStream {
        let span = self.span(a.loc);
        let vis = self.class_vis(&a.modifiers, a.loc);
        let name = self.name_ident(&a.name.name, a.name.loc);
        let params: Vec<proc_macro2::Ident> =
            a.type_params.iter().map(|p| proc_macro2::Ident::new(&p.name.name, self.span(p.loc))).collect();
        // (written as it is, not through itself: an alias's own name isn't seen in its target)
        let t = self.rust_type(&a.ty, TypePos::Value);
        let allow = class_allow(span);
        let g = if params.is_empty() { quote!() } else { quote_spanned!(span=> <#(#params),*>) };
        quote_spanned!(span=> #allow #vis type #name #g = #t;)
    }
}

/// The lints a class's Rust keeps quiet: [`allow_attr`]'s, and dead code (what Kotlin gives every class of its
/// kind — `componentN`, `copy`, `entries`, `values()` — is there whether the program uses it or not).
pub(crate) fn class_allow(span: Span) -> TokenStream {
    let mut a = allow_attr(span);
    a.extend(quote_spanned!(span=> #[allow(dead_code)]));
    a
}

/// The translation with each `types(…)` list (what `named_args!`, `kompozable_block!` and `#[kompozable]` are told
/// of the user's classes) naming only the classes the code it is given names: the list is what the macros need to
/// rewrite those classes' calls, and the generator writes every class it knows at each place.
pub(crate) fn trim_types(ts: TokenStream) -> TokenStream {
    use proc_macro2::{Delimiter, Group, TokenTree};
    use std::collections::HashSet;
    fn idents(ts: &TokenStream, out: &mut HashSet<String>) {
        for t in ts.clone() {
            match t {
                TokenTree::Ident(i) => {
                    out.insert(i.to_string());
                }
                TokenTree::Group(g) => idents(&g.stream(), out),
                _ => {}
            }
        }
    }
    /// `types(A(r), B(rd))` kept to the entries `used` names.
    fn filtered(list: &Group, used: &HashSet<String>) -> Option<TokenTree> {
        let mut kept = Vec::new();
        let mut entry: Vec<TokenTree> = Vec::new();
        let flush = |entry: &mut Vec<TokenTree>, kept: &mut Vec<TokenStream>| {
            if let Some(TokenTree::Ident(i)) = entry.first()
                && used.contains(&i.to_string())
            {
                kept.push(entry.iter().cloned().collect());
            }
            entry.clear();
        };
        for t in list.stream() {
            match &t {
                TokenTree::Punct(p) if p.as_char() == ',' => flush(&mut entry, &mut kept),
                _ => entry.push(t),
            }
        }
        flush(&mut entry, &mut kept);
        if kept.is_empty() {
            return None;
        }
        let mut g = Group::new(Delimiter::Parenthesis, quote!(#(#kept),*));
        g.set_span(list.span());
        Some(TokenTree::Group(g))
    }
    fn walk(ts: TokenStream) -> TokenStream {
        let tokens: Vec<TokenTree> = ts.into_iter().collect();
        let mut out: Vec<TokenTree> = Vec::new();
        let mut k = 0;
        while k < tokens.len() {
            let t = tokens[k].clone();
            match t {
                TokenTree::Group(g) => {
                    let inner: Vec<TokenTree> = walk(g.stream()).into_iter().collect();
                    // (`types(…), rest`: a macro's own list, of what the rest names)
                    let inner = match inner.as_slice() {
                        [TokenTree::Ident(i), TokenTree::Group(list), rest @ ..]
                            if i == "types"
                                && list.delimiter() == Delimiter::Parenthesis
                                && matches!(rest.first(), Some(TokenTree::Punct(p)) if p.as_char() == ',') =>
                        {
                            let mut used = HashSet::new();
                            idents(&rest.iter().cloned().collect(), &mut used);
                            match filtered(list, &used) {
                                Some(l) => {
                                    [TokenTree::Ident(i.clone()), l].into_iter().chain(rest.iter().cloned()).collect()
                                }
                                // (none left: the list and its comma go)
                                None => rest.iter().skip(1).cloned().collect::<Vec<_>>(),
                            }
                        }
                        _ => inner,
                    };
                    let mut ng = Group::new(g.delimiter(), inner.into_iter().collect());
                    ng.set_span(g.span());
                    out.push(TokenTree::Group(ng));
                }
                t => out.push(t),
            }
            k += 1;
        }
        // (`#[…kompozable(types(…), …)]` of the item after it: trimmed to what that item names)
        let mut k = 0;
        while k + 1 < out.len() {
            if let (TokenTree::Punct(p), TokenTree::Group(attr)) = (&out[k], &out[k + 1])
                && p.as_char() == '#'
                && attr.delimiter() == Delimiter::Bracket
            {
                let a: Vec<TokenTree> = attr.stream().into_iter().collect();
                if let Some(pos) = a.iter().position(|t| matches!(t, TokenTree::Ident(i) if i == "kompozable"))
                    && let Some(TokenTree::Group(opts)) = a.get(pos + 1)
                    && opts.delimiter() == Delimiter::Parenthesis
                {
                    // (the item: up to its body)
                    let mut used = HashSet::new();
                    for t in &out[k + 2..] {
                        idents(&TokenStream::from(t.clone()), &mut used);
                        if matches!(t, TokenTree::Group(g) if g.delimiter() == Delimiter::Brace) {
                            break;
                        }
                    }
                    let o: Vec<TokenTree> = opts.stream().into_iter().collect();
                    let mut no: Vec<TokenTree> = Vec::new();
                    let mut j = 0;
                    while j < o.len() {
                        match (&o[j], o.get(j + 1)) {
                            (TokenTree::Ident(i), Some(TokenTree::Group(list))) if i == "types" => {
                                if let Some(l) = filtered(list, &used) {
                                    no.push(o[j].clone());
                                    no.push(l);
                                } else if no
                                    .last()
                                    .is_some_and(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == ','))
                                {
                                    no.pop();
                                } else if matches!(o.get(j + 2), Some(TokenTree::Punct(p)) if p.as_char() == ',') {
                                    j += 1;
                                }
                                j += 2;
                            }
                            _ => {
                                no.push(o[j].clone());
                                j += 1;
                            }
                        }
                    }
                    let mut new_a: Vec<TokenTree> = a[..=pos].to_vec();
                    if !no.is_empty() {
                        let mut g = Group::new(Delimiter::Parenthesis, no.into_iter().collect());
                        g.set_span(opts.span());
                        new_a.push(TokenTree::Group(g));
                    }
                    new_a.extend(a[pos + 2..].iter().cloned());
                    let mut ng = Group::new(Delimiter::Bracket, new_a.into_iter().collect());
                    ng.set_span(attr.span());
                    out[k + 1] = TokenTree::Group(ng);
                }
            }
            k += 1;
        }
        out.into_iter().collect()
    }
    walk(ts)
}

/// The Rust name of an enum entry's own function (`__kz_plus_twice`).
fn entry_own_name(entry: &str, name: &str) -> String {
    format!("__kz_{}_{}", entry.to_lowercase(), snake(name))
}

/// Where a member is.
fn m_loc(m: &Member) -> Loc {
    match m {
        Member::Decl(d) => d.loc(),
        Member::Init(b) => b.loc,
        Member::Constructor(s) => s.loc,
    }
}

fn class_loc(a: &ClassAst) -> Loc {
    match a {
        ClassAst::Class(c) => c.loc,
        ClassAst::Object(o) => o.loc,
    }
}

/// `t` with the type parameters `binds` names replaced.
pub(crate) fn substitute(t: &Type, binds: &HashMap<String, Type>) -> Type {
    let mut out = t.clone();
    match &mut out.kind {
        TypeKind::Named(segs) => {
            if let [seg] = segs.as_slice()
                && seg.args.is_empty()
                && let Some(b) = binds.get(&seg.name.name)
            {
                let mut b = b.clone();
                b.nullable |= t.nullable;
                return b;
            }
            for s in segs.iter_mut() {
                for a in s.args.iter_mut() {
                    if let TypeArg::Type { ty, .. } = a {
                        *ty = substitute(ty, binds);
                    }
                }
            }
        }
        TypeKind::Function(f) => {
            if let Some(r) = &mut f.receiver {
                **r = substitute(r, binds);
            }
            for p in f.params.iter_mut() {
                p.ty = substitute(&p.ty, binds);
            }
            *f.result = substitute(&f.result, binds);
        }
        TypeKind::Error => {}
    }
    out
}

/// Whether an object expression's body has more than overriding functions and values (what `object_expr` makes
/// closures and values of): members of its own (its value's type shows them), state, an `init` block, suspending
/// functions — a class's.
fn has_own_members(body: &ClassBody) -> bool {
    body.members.iter().any(|m| match m {
        Member::Decl(Decl::Function(f)) => {
            !super::hierarchy::is_override(&f.modifiers) || f.is_suspend() || !f.type_params.is_empty()
        }
        Member::Decl(Decl::Property(p)) => {
            !super::hierarchy::is_override(&p.modifiers) || p.is_var() || !matches!(p.init, PropertyInit::Value(_))
        }
        Member::Init(_) => true,
        _ => false,
    })
}

/// Whether a constructor's parameter is serialized by a serializer of its own: `@Serializable(with = S::class)` on
/// it or on a type in its type (`List<@Serializable(with = HatSerializer::class) Hat>`).
fn custom_serializer(p: &Param) -> bool {
    fn with(a: &[Annotation]) -> bool {
        a.iter().any(|a| {
            a.name.last().name == "Serializable"
                && a.args
                    .as_deref()
                    .is_some_and(|args| args.iter().any(|x| x.name.as_ref().is_some_and(|n| n.name == "with")))
        })
    }
    fn in_type(t: &Type) -> bool {
        with(&t.annotations)
            || match &t.kind {
                TypeKind::Named(segs) => segs.iter().flat_map(|s| &s.args).any(|a| match a {
                    TypeArg::Type { ty, .. } => in_type(ty),
                    TypeArg::Star(_) => false,
                }),
                _ => false,
            }
    }
    with(&p.annotations) || p.ty.as_ref().is_some_and(in_type)
}

/// The names of a function's parameters and its body's locals (its statements', its local functions').
fn local_names(f: &Function) -> Vec<String> {
    let mut out: Vec<String> = f.params.iter().map(|p| p.name.name.clone()).collect();
    if let Some(FunctionBody::Block(b)) = &f.body {
        for s in &b.stmts {
            match &s.kind {
                StmtKind::Decl(Decl::Property(p)) => match &p.binding {
                    Binding::Name(n) => out.push(n.name.clone()),
                    Binding::Destructure(es) => out.extend(es.iter().map(|e| e.name.name.clone())),
                },
                StmtKind::Decl(Decl::Function(g)) => out.push(g.name.name.clone()),
                _ => {}
            }
        }
    }
    out
}

/// Whether an object expression's members use any of `names`.
fn uses_names(body: &ClassBody, names: &[String]) -> bool {
    let mut used = false;
    let mut look = |e: &Expr| {
        if let ExprKind::Name(n) = &e.kind {
            used |= names.contains(&n.name);
        }
    };
    for m in &body.members {
        match m {
            Member::Decl(Decl::Function(g)) => super::generics::walk_body(g.body.as_ref(), &mut look),
            Member::Decl(Decl::Property(p)) => {
                if let PropertyInit::Value(e) | PropertyInit::Delegate(e) = &p.init {
                    super::generics::walk_expr(e, &mut look);
                }
                for a in [&p.getter, &p.setter].into_iter().flatten() {
                    super::generics::walk_body(a.body.as_ref(), &mut look);
                }
            }
            Member::Init(b) => super::generics::walk_block(b, &mut look),
            _ => {}
        }
    }
    used
}
