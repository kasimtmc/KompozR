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
//! | `interface I { fun f(): Int; val p: String }` (abstract members) | `pub trait I { fn f(&self) -> i32; fn p(&self) -> String; }`; its companion an object as a class's (`I.Companion`) |
//! | nested declarations (`class A { data class B … }`) | the outer's name before theirs (`AB`), Kotlin's `A.B` that type (the overlay's types), `B` alone inside `A` |
//! | `typealias N<T> = List<T>` | `pub type N<T> = Vec<T>;` and seen through wherever the generator reads a type (Kotlin's alias is the type) |
//! | `@Serializable` on a data class or an object | `#[derive(Route)]` (a typed navigation route: kompoz-navigation's) |
//!
//! A class's members are translated as functions are (named arguments and defaults through `#[named]`), with the
//! class's value the implicit receiver (`self`). What Kotlin's classes do beyond this — inheritance (`open`,
//! `abstract`, `sealed`, interfaces implemented), `inner` classes, generic classes, object expressions, rich enum
//! entries, operator functions, `equals`/`hashCode` overrides, property accessors with backing fields and delegates
//! — is the next packages' (an error saying so where it is written).

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

fn find_in(d: &Decl, outer: Option<(&str, &str)>, own: bool, out: &mut Vec<ClassDecl>) {
    let qualify = |name: &str| match outer {
        Some((k, r)) => (format!("{k}.{name}"), format!("{r}{name}")),
        None => (name.to_string(), name.to_string()),
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
            out.push(ClassDecl {
                info: UserClass { kotlin: kotlin.clone(), rust: rust.clone(), shape, companion },
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
                info: UserClass { kotlin: kotlin.clone(), rust: rust.clone(), shape, companion: None },
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
    /// The type it is a member of (Rust's name).
    pub owner: String,
    pub shape: Shape,
    /// The companion whose members are in scope (the class's own, or the companion itself's).
    pub companion: Option<String>,
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
        let rust = self.classes.names.get(kotlin)?;
        self.classes.classes.get(rust)
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

    // ---- Collecting --------------------------------------------------------------------------------------------

    /// The user's types (the text's own `own`, the module table's `module`): their names first (the overlay's types:
    /// what the members' types name), then their members as the overlay's items.
    pub(crate) fn collect_classes(&mut self, own: &[&Decl], module: &[&Decl]) {
        let mut decls = Vec::new();
        find_classes(own, true, &mut decls);
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
            self.classes.names.entry(d.info.kotlin.clone()).or_insert(d.info.rust.clone());
            if d.outer.is_some() {
                let short = d.info.kotlin.rsplit('.').next().unwrap_or(&d.info.kotlin).to_string();
                self.classes.names.entry(short).or_insert(d.info.rust.clone());
                types.push((overlay::intern(&d.info.kotlin), overlay::intern(&d.info.rust)));
            }
        }
        let mut base = overlay::current().map(|o| (*o).clone()).unwrap_or_default();
        base.types.extend(types);
        base.interfaces
            .extend(decls.iter().filter(|d| d.info.shape == Shape::Interface).map(|d| overlay::intern(&d.info.rust)));
        self.overlay = Some(overlay::install(base.clone()));
        let mut items = Vec::new();
        for d in &decls {
            items.extend(self.quiet(|g| g.class_items(d)));
        }
        base.items.splice(0..0, items);
        self.overlay = None;
        self.overlay = Some(overlay::install(base));
        self.classes.decls = decls;
    }

    /// The overlay's items of a declaration: what a use of it finds.
    fn class_items(&mut self, d: &ClassDecl) -> Vec<index::Item> {
        let mut out = Vec::new();
        let c = &d.info;
        let cx = MemberCx { owner: c.rust.clone(), shape: c.shape, companion: c.companion.clone() };
        let members = match &d.ast {
            ClassAst::Class(k) => &k.body.members,
            ClassAst::Object(o) => &o.body.members,
        };
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
                    let mut ctor = item(&k.name.name, &c.rust, Kind::Function, "");
                    ctor.params = self.item_params(&params, None);
                    ctor.ret = overlay::intern(&c.rust);
                    ctor.flags |= flags::NAMED;
                    out.push(ctor);
                    if c.kotlin != k.name.name {
                        let mut q = ctor;
                        q.kotlin = overlay::intern(&c.kotlin);
                        out.push(q);
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
                                &c.rust,
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
                    let mut it = item(&f.name.name, &snake(&f.name.name), Kind::Method, &c.rust);
                    it.receiver = if c.shape == Shape::Enum { Receiver::Value } else { Receiver::Ref };
                    it.params = self.item_params(&f.params, None);
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
                    let mut it = self.property_item(&n.name, &c.rust, &text, p.is_var());
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
    fn item_params(&mut self, params: &[Param], _cx: Option<&MemberCx>) -> &'static [index::Param] {
        let mut out = Vec::new();
        for p in params {
            let Some(t) = &p.ty else { continue };
            let pos = if p.default.is_some() { TypePos::DefaultedParam } else { TypePos::Param };
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

    /// Opens a member's scope: its receivers (the value `self`, the companion in scope), as `MemberCx` says.
    pub(crate) fn enter_member(&mut self, cx: &MemberCx, label: &str, with_self: bool) {
        self.scopes = vec![HashMap::new()];
        self.receivers.clear();
        self.frames.clear();
        self.loops.clear();
        self.lambdas.clear();
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
            self.receivers.push(ImplicitReceiver {
                rust: "self".into(),
                ty: cx.owner.clone(),
                value: Ty::named(&cx.owner),
                labels: vec![label.to_string()],
                depth: 0,
                by_ref: false,
                by_mut: false,
                owned_used: Default::default(),
            });
            self.declare("this", "self".into(), LocalKind::Param, Ty::named(&cx.owner));
        }
    }

    /// A member function's result type as the index's text.
    fn member_ret_text(&mut self, f: &Function, cx: &MemberCx) -> String {
        if let Some(t) = &f.result {
            return self.quiet(|g| g.rust_type(t, TypePos::Return)).to_string();
        }
        match &f.body {
            Some(FunctionBody::Expr(e)) => {
                let ty = self.member_expr_ty(f, e, cx);
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
    fn member_property_ty(&mut self, p: &Property, cx: &MemberCx) -> Ty {
        if let Some(t) = &p.ty {
            return self.ty_of(t);
        }
        let e = match (&p.init, &p.getter) {
            (PropertyInit::Value(e), _) => e,
            (_, Some(Accessor { body: Some(FunctionBody::Expr(e)), .. })) => e,
            _ => return Ty::Unknown,
        };
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
        let t = self.peek_ty(e);
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
        }
        out
    }

    fn write_class(&mut self, d: &ClassDecl) -> TokenStream {
        match (&d.ast, d.info.shape) {
            (ClassAst::Class(c), Shape::Data) => self.data_class(c, &d.info),
            (ClassAst::Class(c), Shape::Plain) => self.plain_class(c, &d.info),
            (ClassAst::Class(c), Shape::Enum) => self.enum_class(c, &d.info),
            (ClassAst::Class(c), Shape::Interface) => self.interface(c, &d.info),
            (ClassAst::Object(o), _) => self.object(o, &d.info),
            _ => TokenStream::new(),
        }
    }

    /// The modifiers a class may have here; the others are the next packages'.
    fn check_class(&mut self, c: &Class) -> bool {
        let mut ok = true;
        for m in &c.modifiers.list {
            match m.kind {
                ModifierKind::Public | ModifierKind::Private | ModifierKind::Internal | ModifierKind::Data => {}
                ModifierKind::Final => {}
                _ => {
                    self.later(m.loc, &format!("`{}` on a class", m.kind.keyword()));
                    ok = false;
                }
            }
        }
        if let Some(t) = c.type_params.first() {
            self.later(t.loc, "a generic class");
            ok = false;
        }
        if let Some(s) = c.supers.first() {
            self.later(s.loc, "a class's supertypes");
            ok = false;
        }
        ok
    }

    fn class_vis(&mut self, m: &Modifiers, loc: Loc) -> TokenStream {
        let span = self.span(loc);
        match m.visibility() {
            // (a private class is the file's: the Rust module's)
            Visibility::Private => quote_spanned!(span=> pub(crate)),
            _ => self.visibility(m, loc),
        }
    }

    /// `#[derive(Route)]` for `@Serializable`.
    fn route_derive(&self, annotations: &[Annotation], span: Span) -> TokenStream {
        if annotations.iter().any(|a| a.name.last().name == "Serializable") {
            quote_spanned!(span=> #[derive(Route)])
        } else {
            quote!()
        }
    }

    /// A constructor parameter's field type, value and getter: a data class's `var` shared (`kt::KtShared`).
    fn data_class(&mut self, c: &Class, info: &UserClass) -> TokenStream {
        let span = self.span(c.loc);
        if !self.check_class(c) {
            return quote!();
        }
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let vis = self.class_vis(&c.modifiers, c.loc);
        let name = proc_macro2::Ident::new(&info.rust, self.span(c.name.loc));
        let allow = class_allow(span);
        let route = self.route_derive(&c.annotations, span);
        let cx = MemberCx { owner: info.rust.clone(), shape: Shape::Data, companion: info.companion.clone() };
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
                    let v = self.value(d);
                    self.scopes = saved;
                    quote_spanned!(pspan=> #[route(default = #v)])
                }
                _ => quote!(),
            };
            route_attrs.push(route_default.clone());
            if m == Mutability::Var {
                fields.push(quote_spanned!(pspan=> #route_default #fvis #i: #kt::KtShared<#t>));
                field_values.push(quote_spanned!(pspan=> #i: #kt::KtShared::new(#i)));
                let setter = format_ident!("set_{}", i.to_string().trim_start_matches("r#"), span = pspan);
                getters.push(quote_spanned! {pspan=>
                    #fvis fn #i(&self) -> #t { self.#i.get() }
                    #fvis fn #setter(&self, value: #t) { self.#i.set(value) }
                });
                copy_params.push(quote_spanned!(pspan=> #[default(self.#i.get())] #i: #t));
            } else {
                fields.push(quote_spanned!(pspan=> #route_default #fvis #i: #t));
                field_values.push(quote_spanned!(pspan=> #i));
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
        // The body: stored properties computed as the value is made (the constructor's parameters in scope), init
        // blocks run then, member functions.
        self.enter_member(&cx, &c.name.name, false);
        for cp in &params {
            let ty = cp.param.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown);
            self.declare_at(&cp.param.name, snake(&cp.param.name.name), LocalKind::Param, ty);
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
                    if p.is_delegated() || p.getter.is_some() || p.setter.is_some() || p.is_var() {
                        self.later(p.loc, "a data class's `var`, delegated or accessor property in its body");
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
                    let pspan = self.span(p.loc);
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
                    let i = self.name_ident(&snake(&n.name), n.loc);
                    let v = self.value(init);
                    made.push(quote_spanned!(pspan=> let #i: #t = #v;));
                    self.declare_at(n, snake(&n.name), LocalKind::Val, ty.clone());
                    let fvis = self.member_vis(&p.modifiers, p.loc);
                    fields.push(quote_spanned!(pspan=> #fvis #i: #t));
                    field_values.push(quote_spanned!(pspan=> #i));
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
        let kt_name = &c.name.name;
        let display = if own_to_string {
            quote_spanned!(span=> __kz_out.push_str(&self.to_string_kt());)
        } else {
            let open = format!("{kt_name}(");
            quote_spanned!(span=> __kz_out.push_str(#open); #(#to_string)* __kz_out.push(')');)
        };
        let companion_const = self.companion_const(info, span);
        let new_body = quote_spanned!(span=> #(#made)* #name { #(#field_values),* });
        quote_spanned! {span=>
            #allow
            #[derive(::core::clone::Clone, ::core::cmp::PartialEq)]
            #route
            #vis struct #name { #(#fields),* }

            #allow
            #[#rt::named(type)]
            #vis fn #name(#(#ctor_params),*) -> #name {
                #(#converts)*
                #name::__kz_new(#(#names),*)
            }

            #allow
            impl #name {
                #[doc(hidden)]
                #vis fn __kz_new(#(#value_params),*) -> #name {
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
            impl #name {
                #vis fn copy(&self, #(#copy_params),*) -> #name {
                    #name::__kz_new(#(#names),*)
                }
                #(#methods)*
            }

            #allow
            impl #kt::KtToString for #name {
                fn kt_write(&self, __kz_out: &mut ::std::string::String) {
                    #[allow(unused_imports)]
                    use #kt::{KtViaDisplay as _, KtViaToString as _};
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
        }
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
            Visibility::Protected => {
                let at = m.find(ModifierKind::Protected).unwrap_or(loc);
                self.later(at, "a `protected` member");
                quote!()
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
        let span = self.span(f.loc);
        for m in &f.modifiers.list {
            match m.kind {
                ModifierKind::Public
                | ModifierKind::Private
                | ModifierKind::Internal
                | ModifierKind::Suspend
                | ModifierKind::Inline
                | ModifierKind::Final => {}
                ModifierKind::Override if f.name.name == "toString" => {}
                _ => {
                    self.later(m.loc, &format!("`{}` on a member function", m.kind.keyword()));
                }
            }
        }
        if f.is_kompozable() {
            return self.later(f.loc, "a composable member function");
        }
        if let Some(r) = &f.receiver {
            return self.later(r.loc, "a member extension function");
        }
        if let Some(t) = f.type_params.first() {
            return self.later(t.loc, "a generic member function");
        }
        if f.name.name == "equals" || f.name.name == "hashCode" {
            return self.later(f.name.loc, &format!("an `{}` override", f.name.name));
        }
        let suspend = f.is_suspend();
        let ret_text = self.member_ret_text(f, cx);
        let ret_ty = Ty::from_rust(&ret_text);
        let unit = ret_ty == Ty::Unit;
        self.fun = FnState { kompozable: false, nullable_result: ret_ty.is_nullable(), shared_result: None };
        self.enter_member(cx, &f.name.name, true);
        self.in_async = suspend;
        self.inline_async = false;
        let mut params = Vec::new();
        let mut converts = Vec::new();
        for p in &f.params {
            let (tokens, convert) = self.param(p, false, false);
            params.push(tokens);
            converts.extend(convert);
        }
        let ret = match &f.result {
            Some(t) if self.ty_of(t) != Ty::Unit => {
                let rt = self.rust_type(t, TypePos::Return);
                quote_spanned!(span=> -> #rt)
            }
            Some(_) => quote!(),
            None => match &f.body {
                Some(FunctionBody::Expr(e)) if !unit => {
                    if ret_ty.is_vague() {
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
        let body = match &f.body {
            Some(FunctionBody::Block(b)) => self.fn_block(b, unit),
            Some(FunctionBody::Expr(e)) => {
                if unit {
                    let s = self.stmt_expr(e);
                    quote!(#s;)
                } else {
                    let want = super::expr::Want { some: ret_ty.is_nullable(), ..Default::default() };
                    self.value_want(e, want)
                }
            }
            None => {
                self.error(f.name.loc, "a function here needs a body");
                quote!()
            }
        };
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let companion = self.companion_let(cx, span);
        let vis = self.member_vis(&f.modifiers, f.loc);
        let rust_name = if f.name.name == "toString" && f.params.is_empty() {
            "to_string_kt".to_string()
        } else {
            snake(&f.name.name)
        };
        let name = self.name_ident(&rust_name, f.name.loc);
        let recv = cx.receiver(span);
        let asyncness = if suspend { quote_spanned!(span=> async) } else { quote!() };
        quote_spanned! {span=>
            #vis #asyncness fn #name(#recv, #(#params),*) #ret {
                #[allow(unused_imports)]
                use #kt::ext::*;
                #companion
                #(#converts)*
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

    /// A plain class: a reference to an object of cells (see the module's doc).
    fn plain_class(&mut self, c: &Class, info: &UserClass) -> TokenStream {
        let span = self.span(c.loc);
        if !self.check_class(c) {
            return quote!();
        }
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let vis = self.class_vis(&c.modifiers, c.loc);
        let name = proc_macro2::Ident::new(&info.rust, self.span(c.name.loc));
        let fields_ty = format_ident!("__Kz{}", info.rust, span = span);
        let allow = class_allow(span);
        let cx = MemberCx { owner: info.rust.clone(), shape: Shape::Plain, companion: info.companion.clone() };
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
            let i = self.name_ident(&snake(&cp.param.name.name), cp.param.name.loc);
            let t = cp.param.ty.as_ref().map(|t| self.rust_type(t, TypePos::Value)).unwrap_or_default();
            value_params.push(quote_spanned!(span=> #i: #t));
            names.push(i);
        }
        let mut fields = Vec::new();
        let mut empty = Vec::new();
        let mut getters = Vec::new();
        let mut init = Vec::new();
        for cp in params.iter().filter(|cp| cp.property.is_some()) {
            let pspan = self.span(cp.param.loc);
            let i = self.name_ident(&snake(&cp.param.name.name), cp.param.name.loc);
            let t = cp.param.ty.as_ref().map(|t| self.rust_type(t, TypePos::Value)).unwrap_or_default();
            let fvis = self.member_vis(&cp.param.modifiers, cp.param.loc);
            let var = cp.property == Some(Mutability::Var);
            fields.push(self.cell_field(&i, &t, var, pspan));
            empty.push(self.cell_empty(&i, var, pspan));
            getters.push(self.cell_accessors(&i, &t, var, &fvis, quote_spanned!(pspan=> self.__kz_this), pspan));
            init.push(if var {
                quote_spanned!(pspan=> self.__kz_this.#i.set(#i);)
            } else {
                quote_spanned!(pspan=> self.__kz_this.#i.init(#i);)
            });
        }
        // The body in order: initializers and init blocks in `__kz_init`, the object `self`.
        self.enter_member(&cx, &c.name.name, true);
        for cp in &params {
            let ty = cp.param.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown);
            self.declare_at(&cp.param.name, snake(&cp.param.name.name), LocalKind::Param, ty);
        }
        let mut methods = Vec::new();
        let mut own_to_string = false;
        let members = c.body.members.clone();
        let mut body_init = Vec::new();
        for m in &members {
            match m {
                Member::Init(b) => {
                    let s = self.braced(b, super::stmt::Tail::Unit);
                    body_init.push(quote_spanned!(span=> #s));
                }
                Member::Decl(Decl::Property(p)) => {
                    if let Some((f, e, g, i)) = self.cell_property(p, &cx, quote_spanned!(span=> self.__kz_this)) {
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
        let companion = self.companion_let(&cx, span);
        let companion_const = self.companion_const(info, span);
        let kt_name = &c.name.name;
        let display = if own_to_string {
            quote_spanned!(span=> __kz_out.push_str(&self.to_string_kt());)
        } else {
            quote_spanned!(span=> __kz_out.push_str(&#kt::default_to_string(#kt_name, ::std::rc::Rc::as_ptr(&self.__kz_this) as *const ())))
        };
        quote_spanned! {span=>
            #allow
            #[doc(hidden)]
            #vis struct #fields_ty { #(#fields),* }

            #allow
            #[derive(::core::clone::Clone)]
            #vis struct #name { #[doc(hidden)] pub __kz_this: ::std::rc::Rc<#fields_ty> }

            #allow
            #[#rt::named(type)]
            #vis fn #name(#(#ctor_params),*) -> #name {
                #(#converts)*
                #name::__kz_new(#(#names),*)
            }

            #allow
            impl #name {
                #[doc(hidden)]
                #vis fn __kz_new(#(#value_params),*) -> #name {
                    let __kz_this = #name { __kz_this: ::std::rc::Rc::new(#fields_ty { #(#empty),* }) };
                    __kz_this.__kz_init(#(#names),*);
                    __kz_this
                }
                #[doc(hidden)]
                fn __kz_init(&self, #(#value_params),*) {
                    #[allow(unused_imports)]
                    use #kt::ext::*;
                    #companion
                    #(#init)*
                    #rt::named_args!(#kz_types { #(#body_init)* })
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
            impl ::core::cmp::PartialEq for #name {
                fn eq(&self, other: &Self) -> bool {
                    ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
                }
            }
            #allow
            impl ::core::cmp::Eq for #name {}
            #allow
            impl ::core::hash::Hash for #name {
                fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                    ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
                }
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
        }
    }

    fn cell_field(&self, i: &proc_macro2::Ident, t: &TokenStream, var: bool, span: Span) -> TokenStream {
        let kt = self.kt(span);
        if var { quote_spanned!(span=> #i: #kt::KtVar<#t>) } else { quote_spanned!(span=> #i: #kt::KtVal<#t>) }
    }

    fn cell_empty(&self, i: &proc_macro2::Ident, var: bool, span: Span) -> TokenStream {
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
    fn cell_property(
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
        if p.is_delegated() || p.getter.is_some() || p.setter.is_some() {
            self.later(p.loc, "a delegated property or one with accessors in a class");
            return None;
        }
        if p.modifiers.has(ModifierKind::Lateinit) {
            self.later(p.loc, "a `lateinit` property");
            return None;
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
        let v = if ty.is_nullable() {
            self.value_want(init, super::expr::Want { some: true, ..Default::default() })
        } else {
            self.value(init)
        };
        let fvis = self.member_vis(&p.modifiers, p.loc);
        let field = self.cell_field(&i, &t, var, pspan);
        let empty = self.cell_empty(&i, var, pspan);
        let acc = self.cell_accessors(&i, &t, var, &fvis, place.clone(), pspan);
        let set =
            if var { quote_spanned!(pspan=> #place.#i.set(#v);) } else { quote_spanned!(pspan=> #place.#i.init(#v);) };
        Some((field, empty, acc, Some(set)))
    }

    /// An object declaration (or a companion): `struct O {}` and its members as methods, its state the thread's.
    fn object(&mut self, o: &Object, info: &UserClass) -> TokenStream {
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
        if let Some(s) = o.supers.first() {
            return self.later(s.loc, "an object's supertypes");
        }
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let vis = self.class_vis(&o.modifiers, o.loc);
        let name = proc_macro2::Ident::new(&info.rust, self.span(o.name.as_ref().map_or(o.loc, |n| n.loc)));
        let fields_ty = format_ident!("__Kz{}", info.rust, span = span);
        let allow = class_allow(span);
        let route = self.route_derive(&o.annotations, span);
        let cx = MemberCx { owner: info.rust.clone(), shape: Shape::Object, companion: None };
        self.fun = FnState::default();
        self.enter_member(&cx, &info.rust, true);
        let place = quote_spanned!(span=> Self::__kz_state());
        let mut fields = Vec::new();
        let mut empty = Vec::new();
        let mut getters = Vec::new();
        let mut init = Vec::new();
        let mut methods = Vec::new();
        let mut own_to_string = false;
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
            quote_spanned!(span=> __kz_out.push_str(#shown);)
        } else {
            quote_spanned!(span=> __kz_out.push_str(&#kt::default_to_string(#shown, ::std::rc::Rc::as_ptr(&Self::__kz_state()) as *const ()));)
        };
        quote_spanned! {span=>
            #allow
            #[doc(hidden)]
            #vis struct #fields_ty { #(#fields),* }

            #allow
            #[derive(::core::clone::Clone, ::core::marker::Copy, ::core::cmp::PartialEq, ::core::cmp::Eq, ::core::hash::Hash, ::core::default::Default)]
            #route
            #vis struct #name {}

            #allow
            impl #name {
                #[doc(hidden)]
                fn __kz_state() -> ::std::rc::Rc<#fields_ty> {
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
        }
    }

    /// An enum class of entries without bodies (see the module's doc).
    fn enum_class(&mut self, c: &Class, info: &UserClass) -> TokenStream {
        let span = self.span(c.loc);
        for m in &c.modifiers.list {
            if !matches!(m.kind, ModifierKind::Public | ModifierKind::Private | ModifierKind::Internal) {
                self.later(m.loc, &format!("`{}` on an enum class", m.kind.keyword()));
            }
        }
        if let Some(s) = c.supers.first() {
            return self.later(s.loc, "an enum class's supertypes");
        }
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let vis = self.class_vis(&c.modifiers, c.loc);
        let name = proc_macro2::Ident::new(&info.rust, self.span(c.name.loc));
        let allow = class_allow(span);
        let cx = MemberCx { owner: info.rust.clone(), shape: Shape::Enum, companion: info.companion.clone() };
        let kt_name = &c.name.name;
        let mut variants = Vec::new();
        let mut names = Vec::new();
        let mut parse = Vec::new();
        for e in &c.enum_entries {
            if let Some(b) = &e.body {
                self.later(b.loc.unwrap_or(e.loc), "an enum entry's body");
            }
            let v = self.name_ident(&e.name.name, e.name.loc);
            let text = &e.name.name;
            variants.push(quote!(#v));
            names.push(quote_spanned!(span=> #name::#v => #text));
            parse.push(quote_spanned!(span=> #text => #name::#v));
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
                    methods.push(self.member_fn(f, &cx));
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
                    methods.push(self.member_fn(&f, &cx));
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
        let companion_const = self.companion_const(info, span);
        let missing = format!("No enum constant {kt_name}.");
        let entries = variants.iter().map(|v| quote_spanned!(span=> #name::#v));
        let entries2 = entries.clone();
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
                    __kz_out.push_str(&self.name());
                }
            }
            #allow
            impl ::core::fmt::Display for #name {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.write_str(&self.name())
                }
            }
        }
    }

    /// An interface of abstract members: a trait.
    fn interface(&mut self, c: &Class, info: &UserClass) -> TokenStream {
        let span = self.span(c.loc);
        if c.kind == ClassKind::FunInterface || c.is_sealed() {
            return self.later(c.loc, if c.is_sealed() { "a sealed interface" } else { "a fun interface" });
        }
        if let Some(s) = c.supers.first() {
            return self.later(s.loc, "an interface's supertypes");
        }
        if let Some(t) = c.type_params.first() {
            return self.later(t.loc, "a generic interface");
        }
        let vis = self.class_vis(&c.modifiers, c.loc);
        let name = proc_macro2::Ident::new(&info.rust, self.span(c.name.loc));
        let allow = class_allow(span);
        let mut items = Vec::new();
        for m in &c.body.members {
            match m {
                Member::Decl(Decl::Function(f))
                    if f.body.is_none() && f.type_params.is_empty() && f.receiver.is_none() =>
                {
                    let fspan = self.span(f.loc);
                    let i = self.name_ident(&snake(&f.name.name), f.name.loc);
                    let mut ps = Vec::new();
                    for p in &f.params {
                        let pi = self.name_ident(&snake(&p.name.name), p.name.loc);
                        let t = p.ty.as_ref().map(|t| self.rust_type(t, TypePos::Value)).unwrap_or_default();
                        ps.push(quote_spanned!(fspan=> #pi: #t));
                    }
                    let ret = match &f.result {
                        Some(t) if self.ty_of(t) != Ty::Unit => {
                            let t = self.rust_type(t, TypePos::Value);
                            quote_spanned!(fspan=> -> #t)
                        }
                        _ => quote!(),
                    };
                    let asyncness = if f.is_suspend() { quote_spanned!(fspan=> async) } else { quote!() };
                    items.push(quote_spanned!(fspan=> #asyncness fn #i(&self, #(#ps),*) #ret;));
                }
                Member::Decl(Decl::Property(p))
                    if matches!(p.init, PropertyInit::None) && p.getter.is_none() && p.ty.is_some() =>
                {
                    let Binding::Name(n) = &p.binding else { continue };
                    let pspan = self.span(p.loc);
                    let i = self.name_ident(&snake(&n.name), n.loc);
                    let t = self.rust_type(p.ty.as_ref().expect("a type"), TypePos::Value);
                    items.push(quote_spanned!(pspan=> fn #i(&self) -> #t;));
                    if p.is_var() {
                        let s = format_ident!("set_{}", i.to_string().trim_start_matches("r#"), span = pspan);
                        items.push(quote_spanned!(pspan=> fn #s(&self, value: #t);));
                    }
                }
                Member::Decl(Decl::Class(_) | Decl::Object(_) | Decl::TypeAlias(_)) => {}
                Member::Decl(d) => {
                    self.later(d.loc(), "an interface's member with a body");
                }
                Member::Init(b) => {
                    self.error(b.loc, "an interface has no `init` blocks");
                }
                Member::Constructor(s) => {
                    self.error(s.loc, "an interface has no constructors");
                }
            }
        }
        let companion = match &info.companion {
            Some(cc) => {
                let ci = proc_macro2::Ident::new(cc, span);
                quote_spanned! {span=>
                    #allow
                    impl dyn #name {
                        #[allow(non_upper_case_globals)]
                        pub const Companion: #ci = #ci {};
                    }
                }
            }
            None => quote!(),
        };
        quote_spanned! {span=>
            #allow
            #vis trait #name { #(#items)* }
            #companion
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
fn class_allow(span: Span) -> TokenStream {
    let mut a = allow_attr(span);
    a.extend(quote_spanned!(span=> #[allow(dead_code)]));
    a
}

fn class_loc(a: &ClassAst) -> Loc {
    match a {
        ClassAst::Class(c) => c.loc,
        ClassAst::Object(o) => o.loc,
    }
}

/// `t` with the type parameters `binds` names replaced.
fn substitute(t: &Type, binds: &HashMap<String, Type>) -> Type {
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
