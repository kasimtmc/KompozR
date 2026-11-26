//! Kotlin's class hierarchies (stage 14 WPA): interfaces, open, abstract and sealed classes, the API's and the
//! crate's Rust base classes, and what a class deriving from them is made of.
//!
//! | Kotlin | Rust |
//! |---|---|
//! | `interface I : J { val p: T; fun f(): R; fun g() = … }` | `pub trait I: J { fn p(&self) -> T; fn f(&self) -> R; fn g(&self) -> R { … } }`; a root interface's trait (one with no interface above it) is a `kt::KtObject` and has the hidden methods a class's object answers Kotlin's `==`, `toString()` and casts with; its values `Rc<dyn I>` (`==`, `toString()`, `is`/`as` through them); a `suspend fun` gives a boxed future (`kt::KtFuture`: a trait object's method can't be `async`) |
//! | `fun interface F { fun f(x: A): R }` | the trait, implemented for the closures `Fn(A) -> R` (a suspending one's for the closures giving its future); `F { x -> … }` (SAM) is `Rc::new(closure)` as an `Rc<dyn F>` |
//! | `abstract class B(val p: T) : I { abstract fun f(); open fun g() = …; fun h() = … }` | `pub struct BBase` (its state: the constructor's properties, the body's, in cells; its base class's within), `pub trait BOpen: I` (the members: abstract ones required, the others provided, their bodies hidden methods `__kz_b_g` a subclass's `super.g()` calls; `__kz_b(&self) -> &BBase`, `__kz_b_init(…)` running its initializers); Kotlin's type `B` is `Rc<dyn BOpen>` |
//! | `open class B(…)` | the same, and the class itself: `pub struct B` (an object of it, a subclass of `BOpen` adding nothing), its constructor `B(…)` |
//! | `class C(…) : B(args), I` | the class (its shape: data, plain, object, enum) holding `__kz_super: BBase`, its initializer calling `BOpen::__kz_b_init(self, args)` first; `impl BOpen for C`, `impl I for C`, … (each supertype above it): what it overrides its own members (`C::f(self)`), what a class between overrides that class's hidden body, the rest the traits' defaults |
//! | `sealed class S { data object A : S(); data class B(…) : S() }` (or `sealed interface`) | `pub enum S { A(SA), B(SB), … }` of its subclasses (nested and the file's), `From` each, `KtAs` each (`is`/`as`), `==` with each; its members (`SOpen`) and interfaces the enum's too, through the case it holds; `when (s) { is S.A -> … }` a `match` |
//! | `class V : ViewModel()` (an API base: a trait with a doc line ``Kotlin: `abstract class ViewModel` ``) | `impl ViewModel for V`: its state (`view_model_impl`: a `ViewModelImpl` the object holds, made of the super constructor's arguments), its overrides (`onCleared`), its `new_instance` (Kotlin's reflective constructor: none, or a `SavedStateHandle`), `ViewModelRef` (`viewModel<V>()` gives the handle) |
//! | `class P : Greeting()` (a Rust trait of the crate with a doc line ``Kotlin: `open class Greeting` ``) | `impl Greeting for P`, its overrides |
//!
//! Kotlin's `x is T`/`x as T` of an interface's value go through `kt::KtAs` implemented for the trait object
//! (`dyn I`): the object's class (a downcast), else what the object says it is too (another interface, a sealed
//! class: its `__kz_cast`).

use super::class::{ClassAst, ClassDecl, MemberCx, Shape, substitute};
use super::names::{ident, snake};
use super::resolve::*;
use super::types::TypePos;
use super::{FnState, Gen, last_segment, respan};
use crate::ast::*;
use crate::index;
use crate::pos::Loc;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use std::collections::HashMap;

/// What may derive from a class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Inherit {
    Final,
    Open,
    Abstract,
    Sealed,
}

impl Inherit {
    pub(crate) fn of(m: &Modifiers, shape: Shape) -> Inherit {
        if m.has(ModifierKind::Sealed) {
            Inherit::Sealed
        } else if m.has(ModifierKind::Abstract) && shape != Shape::Interface {
            Inherit::Abstract
        } else if m.has(ModifierKind::Open) && shape != Shape::Interface {
            Inherit::Open
        } else {
            Inherit::Final
        }
    }
}

/// What a supertype is.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SuperKind {
    /// A user's interface (its trait's Rust name).
    Interface(String),
    /// A user's open, abstract or sealed class (its Rust name).
    Class(String),
    /// An abstract or open class of the API or of the crate's Rust (a trait).
    Base(index::Base),
    /// An interface the API has as a trait.
    Api(String),
}

/// A supertype as written: what it is, its type (its arguments), its constructor's arguments.
#[derive(Clone, Debug)]
pub(crate) struct SuperRef {
    pub kind: SuperKind,
    pub ty: Type,
    pub args: Option<Vec<Arg>>,
    pub loc: Loc,
}

/// A supertype of a class, direct or not, its type arguments in the class's own terms.
#[derive(Clone, Debug)]
pub(crate) struct Ancestor {
    pub kind: SuperKind,
    pub args: Vec<Type>,
}

/// A method of a user's trait, as the classes implementing it write it.
#[derive(Clone, Debug)]
pub(crate) struct Sig {
    /// Kotlin's member it is of (a property's name for its getter and setter).
    pub kotlin: String,
    pub rust: String,
    /// Its parameters: Rust's names and value types.
    pub params: Vec<(proc_macro2::Ident, TokenStream)>,
    pub ret: Option<TokenStream>,
    pub suspend: bool,
    pub role: Role,
    /// Its parameter count in Kotlin (a function's).
    pub arity: usize,
    /// It has no body: a class implementing the trait must give it.
    pub required: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    Function,
    Getter,
    Setter,
    /// A hidden one: a body (`__kz_b_g`), the base's state (`__kz_b`), its initializer, the root's.
    Hidden,
}

/// Where a class's object keeps its base class's part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place {
    /// A plain class's object: `self.__kz_this.__kz_super`.
    Plain,
    /// A data class's value: `self.__kz_super` (shared by its copies).
    Data,
    /// An object's state: `Self::__kz_state().__kz_super`.
    Object,
    /// An enum class's entry: no base class.
    Enum,
    /// An open class's trait, its state the class's base part (`self.__kz_b()`).
    Trait,
    /// An object expression's struct: `self.__kz_super`.
    Local,
}

/// The supertypes written after a class's or an object's name.
pub(crate) fn written_supers(a: &ClassAst) -> &[SuperType] {
    match a {
        ClassAst::Class(c) => &c.supers,
        ClassAst::Object(o) => &o.supers,
    }
}

fn members_of_ast(a: &ClassAst) -> &[Member] {
    match a {
        ClassAst::Class(c) => &c.body.members,
        ClassAst::Object(o) => &o.body.members,
    }
}

fn type_args_of(t: &Type) -> Vec<Type> {
    match &t.kind {
        TypeKind::Named(segs) => segs
            .last()
            .map(|s| {
                s.args
                    .iter()
                    .filter_map(|a| match a {
                        TypeArg::Type { ty, .. } => Some(ty.clone()),
                        TypeArg::Star(_) => None,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Whether a member is written `override`.
pub(crate) fn is_override(m: &Modifiers) -> bool {
    m.has(ModifierKind::Override)
}

/// A class's member a supertype's method is overridden by: a function (by its name and arity), a property (by its
/// name), a constructor's property.
pub(crate) fn overrides(members: &[Member], ctor: &[ConstructorParam], sig: &Sig) -> bool {
    match sig.role {
        Role::Function => members.iter().any(|m| {
            matches!(m, Member::Decl(Decl::Function(f)) if is_override(&f.modifiers) && f.name.name == sig.kotlin
                && f.params.len() == sig.arity && f.receiver.is_none())
        }),
        Role::Getter | Role::Setter => {
            members.iter().any(|m| {
                matches!(m, Member::Decl(Decl::Property(p)) if is_override(&p.modifiers)
                    && matches!(&p.binding, Binding::Name(n) if n.name == sig.kotlin))
            }) || ctor
                .iter()
                .any(|cp| cp.property.is_some() && is_override(&cp.param.modifiers) && cp.param.name.name == sig.kotlin)
        }
        Role::Hidden => false,
    }
}

impl Gen<'_, '_> {
    // ---- What a class derives from ---------------------------------------------------------------------------------

    /// The declaration of a user's type, by its Rust name.
    pub(crate) fn decl_of(&self, rust: &str) -> Option<ClassDecl> {
        self.classes.decls.iter().find(|d| d.info.rust == rust).cloned()
    }

    /// A supertype's name resolved: a user's interface or class, a base class, an API interface.
    pub(crate) fn super_kind(&self, name: &str) -> Option<SuperKind> {
        if let Some(c) = self.user_class(name) {
            return Some(if c.shape == Shape::Interface && c.inherit != Inherit::Sealed {
                SuperKind::Interface(c.rust.clone())
            } else {
                SuperKind::Class(c.rust.clone())
            });
        }
        // (a nested type Rust names otherwise: `ViewModelProvider.Factory` → `ViewModelProviderFactory`)
        let last = index::rust_type(name).unwrap_or_else(|| last_segment(name));
        if let Some(b) = index::base(last) {
            return Some(SuperKind::Base(b));
        }
        trait_object(last).map(|_| SuperKind::Api(last.to_string()))
    }

    /// A class's supertypes as written, resolved (those that aren't known: an error said so where it is written).
    pub(crate) fn supers_of(&self, d: &ClassDecl) -> Vec<SuperRef> {
        let mut out = Vec::new();
        for s in written_supers(&d.ast) {
            let Some(name) = s.ty.simple_name() else { continue };
            // (inside the class it is nested in, a sibling by its own name: `Step(2)` of `Wizard.Step`)
            let kind = self.super_kind(&name).or_else(|| {
                let outer = d.outer.as_ref().and_then(|o| self.user_class_rust(o))?;
                self.super_kind(&format!("{}.{name}", outer.kotlin))
            });
            if let Some(kind) = kind {
                out.push(SuperRef { kind, ty: s.ty.clone(), args: s.constructor_args.clone(), loc: s.loc });
            }
        }
        out
    }

    /// A class's base class: a user's (an open, abstract or sealed one) or a base of the API's.
    pub(crate) fn superclass(&self, d: &ClassDecl) -> Option<SuperRef> {
        self.supers_of(d).into_iter().find(|s| {
            matches!(&s.kind, SuperKind::Base(_))
                || matches!(&s.kind, SuperKind::Class(r) if self.user_class_rust(r).is_some_and(|c| c.shape != Shape::Interface))
        })
    }

    /// Every supertype of a class, nearest first, each once, its type arguments in the class's terms.
    pub(crate) fn ancestors(&self, d: &ClassDecl) -> Vec<Ancestor> {
        let mut out: Vec<Ancestor> = Vec::new();
        let mut queue: Vec<(ClassDecl, HashMap<String, Type>)> = vec![(d.clone(), HashMap::new())];
        let mut next = 0;
        while next < queue.len() {
            let (cur, binds) = queue[next].clone();
            next += 1;
            for s in self.supers_of(&cur) {
                let args: Vec<Type> = type_args_of(&s.ty).iter().map(|t| substitute(t, &binds)).collect();
                if out.iter().any(|a| a.kind == s.kind) {
                    continue;
                }
                out.push(Ancestor { kind: s.kind.clone(), args: args.clone() });
                if let SuperKind::Interface(r) | SuperKind::Class(r) = &s.kind
                    && let Some(sd) = self.decl_of(r)
                {
                    let b: HashMap<String, Type> = sd.info.type_params.iter().cloned().zip(args).collect();
                    queue.push((sd, b));
                }
            }
        }
        out
    }

    /// The classes a class derives from, nearest first: its base class, that one's, … (the user's).
    pub(crate) fn class_chain(&self, d: &ClassDecl) -> Vec<ClassDecl> {
        let mut out = Vec::new();
        let mut cur = d.clone();
        while let Some(s) = self.superclass(&cur) {
            let SuperKind::Class(r) = &s.kind else { break };
            let Some(sd) = self.decl_of(r) else { break };
            if out.iter().any(|x: &ClassDecl| x.info.rust == sd.info.rust) || sd.info.rust == d.info.rust {
                break;
            }
            out.push(sd.clone());
            cur = sd;
        }
        out
    }

    /// An ancestor's trait, as a type (`Feed<i32>`, `PlateOpen`, `ViewModel`), or as a path to call its functions
    /// through (`Feed::<i32>`).
    pub(crate) fn ancestor_trait(&mut self, a: &Ancestor, turbofish: bool, span: Span) -> TokenStream {
        let name = match &a.kind {
            SuperKind::Interface(r) => r.clone(),
            SuperKind::Class(r) => self.user_class_rust(r).and_then(|c| c.members_trait()).unwrap_or_default(),
            SuperKind::Base(b) => b.rust.to_string(),
            SuperKind::Api(n) => n.clone(),
        };
        let i = ident(&name, span);
        let args: Vec<TokenStream> = a.args.iter().map(|t| self.rust_type(t, TypePos::Value)).collect();
        match (args.is_empty(), turbofish) {
            (true, _) => quote_spanned!(span=> #i),
            (false, false) => quote_spanned!(span=> #i<#(#args),*>),
            (false, true) => quote_spanned!(span=> #i::<#(#args),*>),
        }
    }

    /// The user's trait an ancestor is (an interface's, a class's members'), with its declaration.
    fn ancestor_decl(&self, a: &Ancestor) -> Option<ClassDecl> {
        match &a.kind {
            SuperKind::Interface(r) | SuperKind::Class(r) => self.decl_of(r),
            _ => None,
        }
    }

    /// The first root trait among a trait's own and its ancestors (the one whose hidden methods answer `==`,
    /// `toString()` and casts), as a path.
    /// (`args`: the trait's type arguments, none for its own type parameters)
    fn root_path(&mut self, d: &ClassDecl, args: Option<&[Type]>, span: Span) -> TokenStream {
        let args: Vec<Type> = match args {
            Some(a) => a.to_vec(),
            None => d.info.type_params.iter().map(|p| named_type(p, Loc::default())).collect(),
        };
        let binds: HashMap<String, Type> = d.info.type_params.iter().cloned().zip(args.iter().cloned()).collect();
        let own = Ancestor {
            kind: if d.info.shape == Shape::Interface && d.info.inherit != Inherit::Sealed {
                SuperKind::Interface(d.info.rust.clone())
            } else {
                SuperKind::Class(d.info.rust.clone())
            },
            args,
        };
        let mut all = vec![own];
        all.extend(
            self.ancestors(d)
                .into_iter()
                .map(|a| Ancestor { kind: a.kind, args: a.args.iter().map(|t| substitute(t, &binds)).collect() }),
        );
        let root = all.into_iter().find(|a| self.ancestor_decl(a).is_some_and(|x| self.is_root(&x)));
        match root {
            Some(r) => self.ancestor_trait(&r, true, span),
            None => quote_spanned!(span=> ::core::unreachable),
        }
    }

    /// Whether a user's trait is a root (no user's trait above it: it declares the hidden methods).
    pub(crate) fn is_root(&self, d: &ClassDecl) -> bool {
        !self.supers_of(d).iter().any(|s| match &s.kind {
            SuperKind::Interface(_) => true,
            SuperKind::Class(r) => self.user_class_rust(r).is_some_and(|c| c.members_trait().is_some()),
            _ => false,
        })
    }

    // ---- Generics ------------------------------------------------------------------------------------------------

    /// A class's type parameters declared (`<T: Clone + 'static>`, the bounds its uses need) and used (`<T>`).
    /// (what an impl and a constructor declare, what names them, what the struct declares: the bounds every use
    /// needs, `Clone + 'static`)
    pub(crate) fn class_generics(&mut self, d: &ClassDecl, span: Span) -> (TokenStream, TokenStream, TokenStream) {
        let (params, data) = match &d.ast {
            ClassAst::Class(c) => (c.type_params.clone(), d.info.shape == Shape::Data),
            ClassAst::Object(_) => (Vec::new(), false),
        };
        // (an inner class's: its outer class's too, first)
        let mut all: Vec<TypeParam> = Vec::new();
        if let Some(o) = &d.info.inner
            && let Some(od) = self.decl_of(o)
            && let ClassAst::Class(oc) = &od.ast
        {
            all.extend(oc.type_params.iter().cloned());
        }
        all.extend(params);
        if all.is_empty() {
            return (quote!(), quote!(), quote!());
        }
        let members = members_of_ast(&d.ast).to_vec();
        let ctor: Vec<ConstructorParam> = match &d.ast {
            ClassAst::Class(c) => c.primary_constructor.iter().flat_map(|p| p.params.clone()).collect(),
            ClassAst::Object(_) => Vec::new(),
        };
        let mut uses = super::generics::Uses::of_class(&members, &ctor, data);
        // (an inner class's: what its outer class's members need of them too, which it calls)
        if let Some(o) = &d.info.inner
            && let Some(od) = self.decl_of(o)
            && let ClassAst::Class(oc) = &od.ast
        {
            let octor: Vec<ConstructorParam> = oc.primary_constructor.iter().flat_map(|p| p.params.clone()).collect();
            let ou = super::generics::Uses::of_class(&oc.body.members, &octor, od.info.shape == Shape::Data);
            uses.all |= ou.all;
            uses.shown.extend(ou.shown);
            uses.compared.extend(ou.compared);
        }
        // (what its supertypes need of the arguments it gives them: `class X<T> : Holder<T>(…)`)
        let pnames: Vec<String> = all.iter().map(|p| p.name.name.clone()).collect();
        for s in written_supers(&d.ast) {
            self.uses_through(&s.ty, &pnames, &mut uses);
        }
        let decl = self.type_param_list(&all, &uses, false, span);
        let names: Vec<proc_macro2::Ident> = all.iter().map(|p| ident(&p.name.name, span)).collect();
        // (a struct's own: none, its impls' bounds the uses', as the `#[named(type)]` builder's names it)
        (decl, quote_spanned!(span=> <#(#names),*>), quote_spanned!(span=> <#(#names),*>))
    }

    // ---- Interfaces ----------------------------------------------------------------------------------------------

    /// An interface: a trait (see the module's doc).
    pub(crate) fn interface_trait(&mut self, d: &ClassDecl) -> TokenStream {
        let ClassAst::Class(c) = &d.ast else { return quote!() };
        let info = d.info.clone();
        let span = self.span(c.loc);
        let kt = self.kt(span);
        let vis = self.class_vis(&c.modifiers, c.loc);
        let name = ident(&info.rust, self.span(c.name.loc));
        let allow = super::class::class_allow(span);
        let uses = super::generics::Uses::of_class(&c.body.members, &[], false);
        let g_decl = self.type_param_list(&c.type_params, &uses, false, span);
        let g_use = generic_use(&c.type_params, span);
        // (its provided methods' bounds: a where clause each, the trait's own parameters free)
        let bounds = self.type_param_bounds(&c.type_params, &uses, false, span);
        let saved_where = std::mem::replace(
            &mut self.classes.trait_where,
            if bounds.is_empty() { quote!() } else { quote_spanned!(span=> where #(#bounds),*) },
        );
        let mut supertraits = Vec::new();
        for s in self.supers_of(d) {
            let a = Ancestor { kind: s.kind.clone(), args: type_args_of(&s.ty) };
            match &s.kind {
                SuperKind::Interface(_) | SuperKind::Api(_) => supertraits.push(self.ancestor_trait(&a, false, span)),
                _ => self.error(s.loc, "an interface derives from interfaces only"),
            }
        }
        let root = self.is_root(d);
        if root {
            supertraits.push(quote_spanned!(span=> #kt::KtObject));
        }
        let cx = MemberCx {
            owner: info.rust.clone(),
            shape: Shape::Interface,
            companion: info.companion.clone(),
            outers: Vec::new(),
        };
        let mut items = Vec::new();
        for m in &c.body.members {
            match m {
                Member::Decl(Decl::Function(f)) => items.extend(self.trait_function(f, &cx, &info.rust)),
                Member::Decl(Decl::Property(p)) => items.extend(self.trait_property(p, &cx, &info.rust)),
                Member::Decl(Decl::Class(_) | Decl::Object(_) | Decl::TypeAlias(_)) => {}
                Member::Decl(d) => {
                    self.later(d.loc(), "this member of an interface");
                }
                Member::Init(b) => self.error(b.loc, "an interface has no `init` blocks"),
                Member::Constructor(s) => self.error(s.loc, "an interface has no constructors"),
            }
        }
        if root {
            items.push(root_hidden(span));
        }
        self.classes.trait_where = saved_where;
        let companion = match &info.companion {
            Some(cc) => {
                let ci = ident(cc, span);
                quote_spanned! {span=>
                    #allow
                    impl #g_decl dyn #name #g_use {
                        #[allow(non_upper_case_globals)]
                        pub const Companion: #ci = #ci {};
                    }
                }
            }
            None => quote!(),
        };
        let fun = if info.fun_interface { self.fun_interface_impl(d, &g_decl, span) } else { quote!() };
        let dyn_impls = self.dyn_impls(d, span);
        let ext = self.trait_ext(&info.rust, &c.type_params, &vis, span);
        quote_spanned! {span=>
            #allow
            #vis trait #name #g_use: #(#supertraits)+* { #(#items)* }
            #ext
            #companion
            #fun
            #dyn_impls
        }
    }

    /// `impl<F: Fn(A) -> R> F for F`: a fun interface's closures (a suspending function's give its future).
    fn fun_interface_impl(&mut self, d: &ClassDecl, g_decl: &TokenStream, span: Span) -> TokenStream {
        let Some(sig) = self.trait_sigs(d, &[]).into_iter().find(|s| s.required && s.role == Role::Function) else {
            self.error(d.info_loc(), "a fun interface has one abstract function");
            return quote!();
        };
        let name = ident(&d.info.rust, span);
        let ClassAst::Class(c) = &d.ast else { return quote!() };
        let g_use = generic_use(&c.type_params, span);
        let inner: Vec<TokenStream> = g_decl_inner(&c.type_params, span);
        let tys: Vec<&TokenStream> = sig.params.iter().map(|(_, t)| t).collect();
        let names: Vec<&proc_macro2::Ident> = sig.params.iter().map(|(n, _)| n).collect();
        let m = ident(&sig.rust, span);
        let out = sig.ret.clone().unwrap_or_else(|| quote_spanned!(span=> ()));
        let kt = self.kt(span);
        let _ = g_decl;
        if sig.suspend {
            quote_spanned! {span=>
                impl<#(#inner,)* __KzF, __KzFut> #name #g_use for __KzF
                where
                    __KzF: ::core::ops::Fn(#(#tys),*) -> __KzFut + 'static,
                    __KzFut: ::core::future::Future<Output = #out> + 'static,
                {
                    fn #m(&self, #(#names: #tys),*) -> #kt::KtFuture<'_, #out> {
                        ::std::boxed::Box::pin(self(#(#names),*))
                    }
                }
            }
        } else {
            quote_spanned! {span=>
                impl<#(#inner,)* __KzF: ::core::ops::Fn(#(#tys),*) -> #out + 'static> #name #g_use for __KzF {
                    fn #m(&self, #(#names: #tys),*) -> #out {
                        self(#(#names),*)
                    }
                }
            }
        }
    }

    /// What a user's trait object is to Kotlin: `==` and `toString()` of the object it holds, `is`/`as` (see the
    /// module's doc).
    fn dyn_impls(&mut self, d: &ClassDecl, span: Span) -> TokenStream {
        let kt = self.kt(span);
        let Some(t) = d.info.members_trait() else { return quote!() };
        let name = ident(&t, span);
        let (tps, g_use) = match &d.ast {
            ClassAst::Class(c) => (c.type_params.clone(), generic_use(&c.type_params, span)),
            ClassAst::Object(_) => (Vec::new(), quote!()),
        };
        let inner = g_decl_inner(&tps, span);
        let g = if inner.is_empty() { quote!() } else { quote_spanned!(span=> <#(#inner),*>) };
        let root = self.root_path(d, None, span);
        let allow = super::class::class_allow(span);
        quote_spanned! {span=>
            #allow
            impl #g ::core::cmp::PartialEq for dyn #name #g_use {
                fn eq(&self, other: &Self) -> bool {
                    #root::__kz_eq(self, #kt::KtObject::as_any(other))
                }
            }
            #allow
            impl #g #kt::KtToString for dyn #name #g_use {
                fn kt_write(&self, __kz_out: &mut ::std::string::String) {
                    #root::__kz_write(self, __kz_out)
                }
            }
            #allow
            impl #g ::core::fmt::Display for dyn #name #g_use {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.write_str(&#kt::KtToString::kt_to_string(self))
                }
            }
            #allow
            impl #g ::core::fmt::Debug for dyn #name #g_use {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.write_str(&#kt::KtToString::kt_to_string(self))
                }
            }
            #allow
            impl<#(#inner,)* __KzT: ::core::any::Any + ::core::clone::Clone> #kt::KtAs<__KzT> for dyn #name #g_use {
                fn kt_as_safe(&self) -> ::core::option::Option<__KzT> {
                    #kt::kt_dyn_as::<__KzT>(#kt::KtObject::as_any(self), || {
                        #root::__kz_cast(self, ::core::any::TypeId::of::<__KzT>())
                    })
                }
            }
        }
    }

    // ---- A trait's members -----------------------------------------------------------------------------------------

    /// The hidden name of a trait's member's body (`__kz_plate_describe`).
    pub(crate) fn hidden_name(trait_owner: &str, rust: &str) -> String {
        format!("__kz_{}_{}", type_snake(trait_owner), rust.trim_start_matches("r#"))
    }

    /// A trait's function: required (abstract), or its body a hidden method and the function calling it (an
    /// override of a supertype's: the hidden method alone, what the supertype's impls call).
    fn trait_function(&mut self, f: &Function, cx: &MemberCx, owner: &str) -> Vec<TokenStream> {
        if f.receiver.is_some() {
            return vec![self.later(f.loc, "a member extension function")];
        }
        // (a generic one with a body: a method of the trait's extension trait, implemented for every value of it —
        // its objects too, `dyn I` —, which a trait object's own methods can't be)
        if !f.type_params.is_empty() {
            if f.body.is_none() {
                self.error(
                    f.loc,
                    "an abstract generic function of an interface or an open class: its trait object can't dispatch it \
                     (give it a body, or make the interface generic)",
                );
                return Vec::new();
            }
            if is_override(&f.modifiers) {
                self.error(f.loc, "an override of a generic function: it is its interface's extension, not overridden");
                return Vec::new();
            }
            let m = self.trait_fn(f, cx, &snake(&f.name.name), true);
            self.classes.trait_ext.push(m);
            return Vec::new();
        }
        if f.is_kompozable() {
            return vec![self.later(f.loc, "a composable member function")];
        }
        let rust = self.member_fn_rust(&cx.owner, f);
        let Some(body) = &f.body else {
            return vec![self.trait_fn(f, cx, &rust, false)];
        };
        let _ = body;
        let hidden = Self::hidden_name(owner, &rust);
        let mut out = vec![self.trait_fn(f, cx, &hidden, true)];
        if !is_override(&f.modifiers) {
            out.push(self.trait_forward(f, cx, &rust, &hidden));
        }
        out
    }

    /// `fn f(&self, a: A) -> R { self.__kz_b_f(a) }`.
    fn trait_forward(&mut self, f: &Function, cx: &MemberCx, rust: &str, hidden: &str) -> TokenStream {
        let span = self.span(f.loc);
        let (params, ret) = self.trait_signature(f, cx);
        let names: Vec<proc_macro2::Ident> = params.iter().map(|(n, _)| n.clone()).collect();
        let ps = params.iter().map(|(n, t)| quote_spanned!(span=> #n: #t));
        let i = ident(rust, self.span(f.name.loc));
        let h = ident(hidden, span);
        let wc = self.classes.trait_where.clone();
        quote_spanned!(span=> fn #i(&self, #(#ps),*) #ret #wc { self.#h(#(#names),*) })
    }

    /// A trait method's parameters (value types) and result (`-> R`, a suspending one's `-> kt::KtFuture<'_, R>`).
    fn trait_signature(
        &mut self,
        f: &Function,
        cx: &MemberCx,
    ) -> (Vec<(proc_macro2::Ident, TokenStream)>, TokenStream) {
        let span = self.span(f.loc);
        let mut params = Vec::new();
        for p in &f.params {
            let n = self.name_ident(&snake(&p.name.name), p.name.loc);
            let t = match &p.ty {
                Some(t) => self.rust_type(t, TypePos::Value),
                None => quote_spanned!(span=> _),
            };
            params.push((n, t));
        }
        let ret_text = self.member_ret_text(f, cx);
        let unit = Ty::from_rust(&ret_text) == Ty::Unit;
        let out = match &f.result {
            Some(t) if self.ty_of(t) != Ty::Unit => Some(self.rust_type(t, TypePos::Value)),
            Some(_) => None,
            None if !unit => {
                let ty = Ty::from_rust(&ret_text);
                let written: Option<TokenStream> = (!ret_text.contains('_')).then(|| ret_text.parse().ok()).flatten();
                if ty.is_vague()
                    && let Some(t) = written
                {
                    Some(respan(t, span))
                } else if ty.is_vague() {
                    self.error(
                        f.name.loc,
                        format!(
                            "write `{}`'s return type: its expression's type isn't one KompozR can tell",
                            f.name.name
                        ),
                    );
                    None
                } else {
                    Some(self.rust_of_ty(&ty, span))
                }
            }
            None => None,
        };
        let kt = self.kt(span);
        let ret = match (f.is_suspend(), out) {
            (true, Some(o)) => quote_spanned!(span=> -> #kt::KtFuture<'_, #o>),
            (true, None) => quote_spanned!(span=> -> #kt::KtFuture<'_, ()>),
            (false, Some(o)) => quote_spanned!(span=> -> #o),
            (false, None) => quote!(),
        };
        (params, ret)
    }

    /// A trait's function as Rust's: `fn name(&self, …) -> R;` without a body, else with its body translated (the
    /// trait's value `self`, its other members in scope).
    pub(crate) fn trait_fn(&mut self, f: &Function, cx: &MemberCx, rust: &str, with_body: bool) -> TokenStream {
        let span = self.span(f.loc);
        let (params, ret) = self.trait_signature(f, cx);
        let generics = if f.type_params.is_empty() { quote!() } else { self.fn_generics(f, span) };
        let i = ident(rust, if with_body { span } else { self.span(f.name.loc) });
        let ps: Vec<TokenStream> = params.iter().map(|(n, t)| quote_spanned!(span=> #n: #t)).collect();
        if !with_body {
            return quote_spanned!(span=> fn #i #generics (&self, #(#ps),*) #ret;);
        }
        let ret_text = self.member_ret_text(f, cx);
        let ret_ty = Ty::from_rust(&ret_text);
        let unit = ret_ty == Ty::Unit;
        let suspend = f.is_suspend();
        self.fun = FnState { kompozable: false, nullable_result: ret_ty.is_nullable(), shared_result: None };
        self.enter_member(cx, &f.name.name, true);
        self.in_async = suspend;
        self.inline_async = false;
        for p in &f.params {
            let ty = p.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown);
            self.declare_at(&p.name, snake(&p.name.name), LocalKind::Param, ty);
        }
        let body = match &f.body {
            Some(FunctionBody::Block(b)) => self.fn_block(b, unit),
            Some(FunctionBody::Expr(e)) if unit => {
                let s = self.stmt_expr(e);
                quote!(#s;)
            }
            Some(FunctionBody::Expr(e)) => {
                let want = super::expr::Want { some: ret_ty.is_nullable(), ..Default::default() };
                self.value_want(e, want)
            }
            None => quote!(),
        };
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let lets = self.member_lets(cx, span);
        let inner = quote_spanned! {span=>
            #[allow(unused_imports)]
            use #kt::ext::*;
            #lets
            #rt::named_args!(#kz_types { #body })
        };
        let wc = self.classes.trait_where.clone();
        if suspend {
            quote_spanned!(span=> fn #i #generics (&self, #(#ps),*) #ret #wc { ::std::boxed::Box::pin(async move { #inner }) })
        } else {
            quote_spanned!(span=> fn #i #generics (&self, #(#ps),*) #ret #wc { #inner })
        }
    }

    /// A trait's property: an abstract one's getter (and setter) required, a getter's body as a function's.
    fn trait_property(&mut self, p: &Property, cx: &MemberCx, owner: &str) -> Vec<TokenStream> {
        let Binding::Name(n) = &p.binding else {
            return vec![self.later(p.loc, "a destructuring member")];
        };
        if p.receiver.is_some() {
            return vec![self.later(p.loc, "a member extension property")];
        }
        let pspan = self.span(p.loc);
        let rust = snake(&n.name);
        if let Some(g) = &p.getter
            && g.body.is_some()
            && matches!(p.init, PropertyInit::None)
        {
            let f = getter_function(p, n, g);
            if p.is_var() {
                self.later(p.loc, "a `var` property with a getter in an interface");
            }
            return self.trait_function(&f, cx, owner);
        }
        if !matches!(p.init, PropertyInit::None) || p.getter.is_some() || p.setter.is_some() {
            return vec![self.error_ts(
                p.loc,
                "an interface's property has no value of its own (an abstract one, or one with a getter)",
            )];
        }
        let Some(t) = &p.ty else {
            return vec![self.error_ts(n.loc, &format!("write `{}`'s type", n.name))];
        };
        let i = self.name_ident(&rust, n.loc);
        let t = self.rust_type(t, TypePos::Value);
        let mut out = vec![quote_spanned!(pspan=> fn #i(&self) -> #t;)];
        if p.is_var() {
            let s = format_ident!("set_{}", rust.trim_start_matches("r#"), span = pspan);
            out.push(quote_spanned!(pspan=> fn #s(&self, value: #t);));
        }
        out
    }

    /// An error, as tokens where some are written.
    fn error_ts(&mut self, loc: Loc, message: &str) -> TokenStream {
        self.error(loc, message.to_string());
        quote!()
    }

    /// The methods of a user's trait (its own, not its supertraits'), as a class implementing it writes them: the
    /// trait's type parameters bound to `args`.
    pub(crate) fn trait_sigs(&mut self, d: &ClassDecl, args: &[Type]) -> Vec<Sig> {
        let tps: Vec<String> = d.info.type_params.clone();
        let binds: HashMap<String, Type> = tps.iter().cloned().zip(args.iter().cloned()).collect();
        let owner = d.info.members_trait().unwrap_or_else(|| d.info.rust.clone());
        let cx = MemberCx {
            owner: owner.clone(),
            shape: d.info.shape,
            companion: d.info.companion.clone(),
            outers: Vec::new(),
        };
        let mut out = Vec::new();
        let span = Span::call_site();
        let sig_of = |me: &mut Self, f: &Function, rust: &str, kotlin: &str, role: Role, required: bool| {
            let mut g = f.clone();
            for p in &mut g.params {
                if let Some(t) = &p.ty {
                    p.ty = Some(substitute(t, &binds));
                }
            }
            g.result = g.result.map(|t| substitute(&t, &binds));
            // (the result itself: a suspending one's future is the signature's)
            g.modifiers.list.retain(|m| m.kind != ModifierKind::Suspend);
            let (params, ret) = me.quiet(|me| me.trait_signature(&g, &cx));
            let ret = if ret.is_empty() {
                None
            } else {
                // (`-> R` without the arrow)
                Some(ret.into_iter().skip(2).collect::<TokenStream>())
            };
            Sig {
                kotlin: kotlin.to_string(),
                rust: rust.to_string(),
                params,
                ret,
                suspend: f.is_suspend(),
                role,
                arity: f.params.len(),
                required,
            }
        };
        if d.info.shape != Shape::Interface {
            out.push(Sig {
                kotlin: String::new(),
                rust: format!("__kz_{}", type_snake(&d.info.rust)),
                params: Vec::new(),
                ret: None,
                suspend: false,
                role: Role::Hidden,
                arity: 0,
                required: true,
            });
        }
        let ctor: Vec<ConstructorParam> = match &d.ast {
            ClassAst::Class(c) => c.primary_constructor.iter().flat_map(|p| p.params.clone()).collect(),
            ClassAst::Object(_) => Vec::new(),
        };
        for cp in ctor.iter().filter(|cp| cp.property.is_some()) {
            let Some(t) = &cp.param.ty else { continue };
            let t = substitute(t, &binds);
            let rust = snake(&cp.param.name.name);
            let tt = self.quiet(|me| me.rust_type(&t, TypePos::Value));
            let name = if is_override(&cp.param.modifiers) { Self::hidden_name(&owner, &rust) } else { rust.clone() };
            out.push(Sig {
                kotlin: cp.param.name.name.clone(),
                rust: name,
                params: Vec::new(),
                ret: Some(tt.clone()),
                suspend: false,
                role: if is_override(&cp.param.modifiers) { Role::Hidden } else { Role::Getter },
                arity: 0,
                required: false,
            });
            if cp.property == Some(Mutability::Var) {
                out.push(Sig {
                    kotlin: cp.param.name.name.clone(),
                    rust: format!("set_{}", rust.trim_start_matches("r#")),
                    params: vec![(ident("value", span), tt)],
                    ret: None,
                    suspend: false,
                    role: Role::Setter,
                    arity: 1,
                    required: false,
                });
            }
        }
        let members = members_of_ast(&d.ast).to_vec();
        for m in &members {
            match m {
                Member::Decl(Decl::Function(f)) if f.receiver.is_none() && f.type_params.is_empty() => {
                    let rust = self.member_fn_rust(&d.info.rust, f);
                    if f.body.is_none() {
                        out.push(sig_of(self, f, &rust, &f.name.name, Role::Function, true));
                    } else {
                        let hidden = Self::hidden_name(&owner, &rust);
                        out.push(sig_of(self, f, &hidden, &f.name.name, Role::Hidden, false));
                        if !is_override(&f.modifiers) {
                            out.push(sig_of(self, f, &rust, &f.name.name, Role::Function, false));
                        }
                    }
                }
                Member::Decl(Decl::Property(p)) if p.receiver.is_none() => {
                    let Binding::Name(n) = &p.binding else { continue };
                    let rust = snake(&n.name);
                    let getter_only =
                        p.getter.as_ref().is_some_and(|g| g.body.is_some()) && matches!(p.init, PropertyInit::None);
                    let abstract_ = matches!(p.init, PropertyInit::None) && p.getter.is_none();
                    let ty = p.ty.as_ref().map(|t| substitute(t, &binds));
                    let tt = match &ty {
                        Some(t) => self.quiet(|me| me.rust_type(t, TypePos::Value)),
                        None => {
                            let pty = self.quiet(|me| me.member_property_ty(p, &cx));
                            self.rust_of_ty(&pty, span)
                        }
                    };
                    let overridden = is_override(&p.modifiers);
                    let (name, role) = if overridden || getter_only && !abstract_ {
                        if overridden {
                            (Self::hidden_name(&owner, &rust), Role::Hidden)
                        } else {
                            (rust.clone(), Role::Getter)
                        }
                    } else {
                        (rust.clone(), Role::Getter)
                    };
                    out.push(Sig {
                        kotlin: n.name.clone(),
                        rust: name,
                        params: Vec::new(),
                        ret: Some(tt.clone()),
                        suspend: false,
                        role,
                        arity: 0,
                        required: abstract_,
                    });
                    if getter_only && !overridden {
                        out.push(Sig {
                            kotlin: n.name.clone(),
                            rust: Self::hidden_name(&owner, &rust),
                            params: Vec::new(),
                            ret: Some(tt.clone()),
                            suspend: false,
                            role: Role::Hidden,
                            arity: 0,
                            required: false,
                        });
                    }
                    if p.is_var() {
                        out.push(Sig {
                            kotlin: n.name.clone(),
                            rust: format!("set_{}", rust.trim_start_matches("r#")),
                            params: vec![(ident("value", span), tt)],
                            ret: None,
                            suspend: false,
                            role: Role::Setter,
                            arity: 1,
                            required: abstract_,
                        });
                    }
                }
                _ => {}
            }
        }
        out
    }

    // ---- What a class implements ---------------------------------------------------------------------------------

    /// The traits a concrete class implements (each supertype above it), what it overrides through its own
    /// members, the hidden methods answering `==`, `toString()` and casts; for a ViewModel its constructor and
    /// handle.
    pub(crate) fn class_impls(
        &mut self,
        d: &ClassDecl,
        place: Place,
        g_decl: &TokenStream,
        g_use: &TokenStream,
    ) -> TokenStream {
        let span = self.span(d.info_loc());
        let kt = self.kt(span);
        let name = ident(&d.info.rust, span);
        let allow = super::class::class_allow(span);
        let ancestors = self.ancestors(d);
        if ancestors.is_empty() {
            return quote!();
        }
        let chain = self.class_chain(d);
        // (`I by value`: the interfaces each delegated one is, and its field)
        let mut delegated: Vec<(SuperKind, TokenStream)> = Vec::new();
        for (k, s) in written_supers(&d.ast).iter().enumerate() {
            if s.delegate.is_none() {
                continue;
            }
            let Some(SuperKind::Interface(r)) = s.ty.simple_name().and_then(|n| self.super_kind(&n)) else { continue };
            let f = format_ident!("__kz_by{k}", span = span);
            let path = match place {
                Place::Data => quote_spanned!(span=> self.#f),
                Place::Object => quote_spanned!(span=> Self::__kz_state().#f),
                _ => quote_spanned!(span=> self.__kz_this.#f),
            };
            delegated.push((SuperKind::Interface(r.clone()), path.clone()));
            if let Some(sd) = self.decl_of(&r) {
                for a in self.ancestors(&sd) {
                    delegated.push((a.kind, path.clone()));
                }
            }
        }
        let own_members = members_of_ast(&d.ast).to_vec();
        let own_ctor: Vec<ConstructorParam> = match &d.ast {
            ClassAst::Class(c) => c.primary_constructor.iter().flat_map(|p| p.params.clone()).collect(),
            ClassAst::Object(_) => Vec::new(),
        };
        let this = |me: &Self| -> TokenStream {
            let _ = me;
            if place == Place::Enum { quote_spanned!(span=> *self) } else { quote_spanned!(span=> self) }
        };
        let casts = self.casts(d, &ancestors, span);
        let mut out = TokenStream::new();
        for a in &ancestors {
            let tr = self.ancestor_trait(a, false, span);
            let mut items = Vec::new();
            match &a.kind {
                SuperKind::Interface(_) | SuperKind::Class(_) => {
                    let Some(ad) = self.ancestor_decl(a) else { continue };
                    // (the base class's part the object holds: at its depth below the class)
                    if let SuperKind::Class(r) = &a.kind
                        && ad.info.shape != Shape::Interface
                        && let Some(depth) = std::iter::once(d.clone()).chain(chain.clone()).position(|x| {
                            self.superclass(&x).is_some_and(|s| matches!(&s.kind, SuperKind::Class(c) if c == r))
                        })
                    {
                        let acc = ident(&format!("__kz_{}", type_snake(r)), span);
                        let base = ident(&format!("{r}Base"), span);
                        let bargs: Vec<TokenStream> =
                            a.args.iter().map(|t| self.rust_type(t, TypePos::Value)).collect();
                        let bg = if bargs.is_empty() { quote!() } else { quote_spanned!(span=> <#(#bargs),*>) };
                        let path = base_path(place, depth, span);
                        items.push(quote_spanned!(span=> fn #acc(&self) -> &#base #bg { #path }));
                    }
                    let sigs = self.trait_sigs(&ad, &a.args);
                    for s in &sigs {
                        if matches!(s.role, Role::Hidden) {
                            continue;
                        }
                        // (who overrides it: the class itself, else a class between, by its hidden body)
                        let mut by: Option<TokenStream> = None;
                        if overrides(&own_members, &own_ctor, s) {
                            let m = ident(&s.rust, span);
                            let names: Vec<&proc_macro2::Ident> = s.params.iter().map(|(n, _)| n).collect();
                            let recv = this(self);
                            by = Some(if s.suspend {
                                quote_spanned!(span=> ::std::boxed::Box::pin(#name::#m(#recv, #(#names),*)))
                            } else {
                                quote_spanned!(span=> #name::#m(#recv, #(#names),*))
                            });
                        } else {
                            for (k, b) in chain.iter().enumerate() {
                                if b.info.rust == ad.info.rust {
                                    break;
                                }
                                let (bm, bc) = (
                                    members_of_ast(&b.ast).to_vec(),
                                    match &b.ast {
                                        ClassAst::Class(c) => {
                                            c.primary_constructor.iter().flat_map(|p| p.params.clone()).collect()
                                        }
                                        ClassAst::Object(_) => Vec::new(),
                                    },
                                );
                                if overrides(&bm, &bc, s) {
                                    let bt = b.info.members_trait().unwrap_or_default();
                                    let h = ident(&Self::hidden_name(&bt, &s.rust), span);
                                    let names: Vec<&proc_macro2::Ident> = s.params.iter().map(|(n, _)| n).collect();
                                    let anc = self
                                        .ancestors(d)
                                        .into_iter()
                                        .find(|x| x.kind == SuperKind::Class(b.info.rust.clone()));
                                    let path = match anc {
                                        Some(x) => self.ancestor_trait(&x, true, span),
                                        None => ident(&bt, span).into_token_stream(),
                                    };
                                    by = Some(quote_spanned!(span=> #path::#h(self, #(#names),*)));
                                    let _ = k;
                                    break;
                                }
                            }
                        }
                        // (else the delegate of `I by value`, where the interface is one)
                        if by.is_none()
                            && let Some((_, path)) = delegated.iter().find(|(k, _)| *k == a.kind)
                        {
                            let m = ident(&s.rust, span);
                            let names: Vec<&proc_macro2::Ident> = s.params.iter().map(|(n, _)| n).collect();
                            let trp = self.ancestor_trait(a, true, span);
                            by = Some(quote_spanned!(span=> #trp::#m(&*#path, #(#names),*)));
                        }
                        if let Some(body) = by {
                            items.push(sig_with_body(s, body, span, &kt));
                        }
                    }
                    // (each root trait's hidden methods: the class's answers, the same in each)
                    if self.is_root(&ad) {
                        items.push(quote_spanned! {span=>
                            fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
                                other.downcast_ref::<Self>().is_some_and(|o| o == self)
                            }
                            fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
                                #kt::KtToString::kt_write(self, __kz_out)
                            }
                            fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
                                #casts
                                ::core::option::Option::None
                            }
                        });
                    }
                }
                SuperKind::Base(b) => {
                    items.extend(self.base_items(d, *b, place, &chain, &own_members, span));
                }
                SuperKind::Api(t) => {
                    items.extend(self.api_overrides(d, t, place, &own_members, span));
                }
            }
            out.extend(quote_spanned! {span=>
                #allow
                impl #g_decl #tr for #name #g_use { #(#items)* }
            });
        }
        // (`==` of the class's value with a value of an interface it is: `shelf == EmptyShelf`)
        for a in &ancestors {
            let dynamic = match &a.kind {
                SuperKind::Interface(_) => true,
                SuperKind::Class(r) => self.user_class_rust(r).is_some_and(|c| c.is_dyn()),
                _ => false,
            };
            if !dynamic {
                continue;
            }
            let tr = self.ancestor_trait(a, false, span);
            let root = match self.ancestor_decl(a) {
                Some(ad) => self.root_path(&ad, Some(&a.args), span),
                None => continue,
            };
            out.extend(quote_spanned! {span=>
                #allow
                impl #g_decl ::core::cmp::PartialEq<#name #g_use> for ::std::rc::Rc<dyn #tr> {
                    fn eq(&self, other: &#name #g_use) -> bool {
                        #root::__kz_eq(&**self, other)
                    }
                }
                #allow
                impl #g_decl ::core::cmp::PartialEq<::std::rc::Rc<dyn #tr>> for #name #g_use {
                    fn eq(&self, other: &::std::rc::Rc<dyn #tr>) -> bool {
                        #root::__kz_eq(&**other, self)
                    }
                }
                // (`x as I` of the class's value: its shared object)
                #allow
                impl #g_decl #kt::KtAs<::std::rc::Rc<dyn #tr>> for #name #g_use {
                    fn kt_as_safe(&self) -> ::core::option::Option<::std::rc::Rc<dyn #tr>> {
                        ::core::option::Option::Some(::std::rc::Rc::new(::core::clone::Clone::clone(self)))
                    }
                }
            });
        }
        out
    }

    /// The casts a class's object answers (`__kz_cast`): each interface and open class trait it is, as the shared
    /// object; each sealed class it is a case of, as its value.
    fn casts(&mut self, d: &ClassDecl, ancestors: &[Ancestor], span: Span) -> TokenStream {
        let mut out = TokenStream::new();
        for a in ancestors {
            match &a.kind {
                SuperKind::Interface(_) => {}
                SuperKind::Class(r) if self.user_class_rust(r).is_some_and(|c| c.is_dyn()) => {}
                SuperKind::Class(r) if self.user_class_rust(r).is_some_and(|c| c.inherit == Inherit::Sealed) => {
                    let s = ident(r, span);
                    out.extend(quote_spanned! {span=>
                        if to == ::core::any::TypeId::of::<#s>() {
                            return ::core::option::Option::Some(::std::boxed::Box::new(<#s as ::core::convert::From<Self>>::from(::core::clone::Clone::clone(self))));
                        }
                    });
                    continue;
                }
                _ => continue,
            }
            let tr = self.ancestor_trait(a, false, span);
            out.extend(quote_spanned! {span=>
                if to == ::core::any::TypeId::of::<::std::rc::Rc<dyn #tr>>() {
                    let o: ::std::rc::Rc<dyn #tr> = ::std::rc::Rc::new(::core::clone::Clone::clone(self));
                    return ::core::option::Option::Some(::std::boxed::Box::new(o));
                }
            });
        }
        let _ = d;
        out
    }

    /// A base class of the API's (or the crate's Rust) implemented: its state's accessor, what the class overrides,
    /// and, for a ViewModel, Kotlin's reflective constructor.
    fn base_items(
        &mut self,
        d: &ClassDecl,
        b: index::Base,
        place: Place,
        chain: &[ClassDecl],
        own: &[Member],
        span: Span,
    ) -> Vec<TokenStream> {
        let mut out = Vec::new();
        // (the state: at the bottom of the chain of base parts)
        if let Some((acc, state)) = base_state(b) {
            if place == Place::Enum {
                self.error(d.info_loc(), format!("an enum class can't derive from `{}`", b.kotlin));
            }
            let depth = chain.len();
            let path = base_path(place, depth, span);
            let a = ident(acc, span);
            let s: TokenStream = respan(state.parse().unwrap_or_else(|_| quote!(_)), span);
            out.push(quote_spanned!(span=> fn #a(&self) -> &#s { (#path).get_ref() }));
        }
        // (what the class and those between override of the trait's methods: by Kotlin's names)
        let mut classes: Vec<(ClassDecl, bool)> = vec![(d.clone(), true)];
        classes.extend(chain.iter().map(|c| (c.clone(), false)));
        let mut done: Vec<String> = Vec::new();
        for (c, own_class) in &classes {
            let members = if *own_class { own.to_vec() } else { members_of_ast(&c.ast).to_vec() };
            for m in &members {
                let Member::Decl(Decl::Function(f)) = m else { continue };
                if !is_override(&f.modifiers) || done.contains(&f.name.name) {
                    continue;
                }
                let Some(item) = index::lookup(&f.name.name)
                    .iter()
                    .find(|i| i.via == b.rust && i.kind == index::Kind::Method && i.params.len() == f.params.len())
                else {
                    continue;
                };
                done.push(f.name.name.clone());
                let body = if *own_class {
                    let n = ident(&d.info.rust, span);
                    let m = ident(&snake(&f.name.name), span);
                    let names: Vec<proc_macro2::Ident> =
                        f.params.iter().map(|p| ident(&snake(&p.name.name), span)).collect();
                    quote_spanned!(span=> #n::#m(self, #(#names),*))
                } else {
                    let bt = c.info.members_trait().unwrap_or_default();
                    let h = ident(&Self::hidden_name(&bt, &snake(&f.name.name)), span);
                    let t = ident(&bt, span);
                    let names: Vec<proc_macro2::Ident> =
                        f.params.iter().map(|p| ident(&snake(&p.name.name), span)).collect();
                    quote_spanned!(span=> #t::#h(self, #(#names),*))
                };
                out.push(api_method(item, body, span));
            }
        }
        // (a ViewModel: Kotlin's reflective constructor a factory calls — none, or a SavedStateHandle —)
        if b.kotlin == "ViewModel" && place != Place::Enum && self.is_concrete(d) {
            let params: Vec<Param> = match &d.ast {
                ClassAst::Class(c) => {
                    c.primary_constructor.iter().flat_map(|p| p.params.iter().map(|cp| cp.param.clone())).collect()
                }
                ClassAst::Object(_) => Vec::new(),
            };
            let required: Vec<&Param> = params.iter().filter(|p| p.default.is_none()).collect();
            let name = ident(&d.info.rust, span);
            let made = match required.as_slice() {
                [] if place != Place::Object => Some(quote_spanned!(span=> #name::__kz_new_default())),
                [p] if p.ty.as_ref().and_then(|t| t.simple_name()).is_some_and(|n| n == "SavedStateHandle")
                    && params.len() == 1 =>
                {
                    Some(quote_spanned!(span=> #name::__kz_new(extras.create_saved_state_handle())))
                }
                _ => None,
            };
            if let Some(made) = made {
                out.push(quote_spanned! {span=>
                    fn new_instance(extras: &CreationExtras) -> ::core::option::Option<Self> {
                        let _ = extras;
                        ::core::option::Option::Some(#made)
                    }
                });
            }
        }
        out
    }

    /// Whether a declaration is a class one makes objects of (not an open class's members, not an interface).
    pub(crate) fn is_concrete(&self, d: &ClassDecl) -> bool {
        d.info.shape != Shape::Interface && matches!(d.info.inherit, Inherit::Final)
    }

    /// What a class overrides of an API interface's methods.
    fn api_overrides(&mut self, d: &ClassDecl, t: &str, place: Place, own: &[Member], span: Span) -> Vec<TokenStream> {
        let mut out = Vec::new();
        let name = ident(&d.info.rust, span);
        for m in own {
            let Member::Decl(Decl::Function(f)) = m else { continue };
            if !is_override(&f.modifiers) {
                continue;
            }
            let Some(item) = index::lookup(&f.name.name)
                .iter()
                .find(|i| i.via == t && i.kind == index::Kind::Method && i.params.len() == f.params.len())
            else {
                continue;
            };
            let m = ident(&snake(&f.name.name), span);
            let recv = if place == Place::Enum { quote_spanned!(span=> *self) } else { quote_spanned!(span=> self) };
            let names: Vec<proc_macro2::Ident> = f.params.iter().map(|p| ident(&snake(&p.name.name), span)).collect();
            out.push(api_method(item, quote_spanned!(span=> #name::#m(#recv, #(#names),*)), span));
        }
        out
    }

    /// The field of the base class's part a class holds (`__kz_super: BBase`), what it is made empty with, and the
    /// super constructor's call that fills it (the class's object `this`, its constructor's parameters in scope).
    pub(crate) fn super_field(
        &mut self,
        d: &ClassDecl,
        place: Place,
        span: Span,
    ) -> Option<(TokenStream, TokenStream)> {
        let s = self.superclass(d)?;
        let kt = self.kt(span);
        match &s.kind {
            SuperKind::Class(r) => {
                let c = self.user_class_rust(r)?.clone();
                if c.shape == Shape::Interface {
                    return None;
                }
                let base = ident(&format!("{r}Base"), span);
                let args: Vec<TokenStream> =
                    type_args_of(&s.ty).iter().map(|t| self.rust_type(t, TypePos::Value)).collect();
                let g = if args.is_empty() { quote!() } else { quote_spanned!(span=> <#(#args),*>) };
                Some(if place == Place::Data {
                    (
                        quote_spanned!(span=> #[doc(hidden)] pub __kz_super: ::std::rc::Rc<#base #g>),
                        quote_spanned!(span=> __kz_super: ::std::rc::Rc::new(#base::__kz_empty())),
                    )
                } else {
                    (
                        quote_spanned!(span=> #[doc(hidden)] pub __kz_super: #base #g),
                        quote_spanned!(span=> __kz_super: #base::__kz_empty()),
                    )
                })
            }
            SuperKind::Base(b) => {
                let (_, state) = base_state(*b)?;
                let st: TokenStream = respan(state.parse().unwrap_or_else(|_| quote!(_)), span);
                Some(if place == Place::Data {
                    (
                        quote_spanned!(span=> #[doc(hidden)] pub __kz_super: ::std::rc::Rc<#kt::KtVal<#st>>),
                        quote_spanned!(span=> __kz_super: ::std::rc::Rc::new(#kt::KtVal::new())),
                    )
                } else {
                    (
                        quote_spanned!(span=> #[doc(hidden)] pub __kz_super: #kt::KtVal<#st>),
                        quote_spanned!(span=> __kz_super: #kt::KtVal::new()),
                    )
                })
            }
            _ => None,
        }
    }

    /// The super constructor's call, first in a class's initializer: `BOpen::__kz_b_init(this, args)`, or the API
    /// base's state made (`ViewModelImpl::new(args)`). `this` is the object (`self`, `&__kz_v`); the class's
    /// constructor's parameters are in scope.
    pub(crate) fn super_init(&mut self, d: &ClassDecl, this: TokenStream, place: Place, span: Span) -> TokenStream {
        let Some(s) = self.superclass(d) else { return quote!() };
        let args = s.args.clone().unwrap_or_default();
        match &s.kind {
            SuperKind::Class(r) => {
                let Some(sd) = self.decl_of(r) else { return quote!() };
                if sd.info.shape == Shape::Interface {
                    return quote!();
                }
                if s.args.is_none() && sd.info.inherit != Inherit::Sealed {
                    self.error(s.loc, format!("call `{}`'s constructor: `{}()`", sd.info.kotlin, sd.info.kotlin));
                }
                let params: Vec<Param> = match &sd.ast {
                    ClassAst::Class(c) => {
                        c.primary_constructor.iter().flat_map(|p| p.params.iter().map(|cp| cp.param.clone())).collect()
                    }
                    ClassAst::Object(_) => Vec::new(),
                };
                let binds: HashMap<String, Type> =
                    sd.info.type_params.iter().cloned().zip(type_args_of(&s.ty)).collect();
                // (a secondary constructor the arguments are of, when the primary one's parameters aren't: its
                // parameters bound to them, the primary one called with its delegation's arguments)
                let required = params.iter().filter(|p| p.default.is_none()).count();
                let fits = args.len() >= required && args.len() <= params.len();
                let secondary = members_of_ast(&sd.ast).iter().find_map(|m| match m {
                    Member::Constructor(c)
                        if !fits
                            && c.params.len() == args.len()
                            && c.delegation.as_ref().is_some_and(|d| !d.to_super) =>
                    {
                        Some(c.clone())
                    }
                    _ => None,
                });
                let (lets, values) = match secondary {
                    Some(c) => {
                        if let Some(b) = &c.body {
                            self.later(b.loc, "a secondary constructor's body run by a subclass's super call");
                        }
                        let given = self.super_args(&c.params, &args, &binds, s.loc);
                        self.push_scope();
                        let mut lets = Vec::new();
                        for (p, v) in c.params.iter().zip(given) {
                            let rust = snake(&p.name.name);
                            let i = self.name_ident(&rust, p.name.loc);
                            let t =
                                p.ty.as_ref()
                                    .map(|t| self.rust_type(&substitute(t, &binds), TypePos::Value))
                                    .unwrap_or_default();
                            let ty = p.ty.as_ref().map(|t| self.ty_of(&substitute(t, &binds))).unwrap_or(Ty::Unknown);
                            self.declare_at(&p.name, rust, LocalKind::Val, ty);
                            lets.push(quote_spanned!(span=> let #i: #t = #v;));
                        }
                        let dargs = c.delegation.as_ref().map(|d| d.args.clone()).unwrap_or_default();
                        let values = self.super_args(&params, &dargs, &binds, s.loc);
                        self.pop_scope();
                        (lets, values)
                    }
                    None => (Vec::new(), self.super_args(&params, &args, &binds, s.loc)),
                };
                let a = Ancestor { kind: s.kind.clone(), args: type_args_of(&s.ty) };
                let tr = self.ancestor_trait(&a, true, span);
                let init = ident(&format!("__kz_{}_init", type_snake(r)), span);
                if lets.is_empty() {
                    quote_spanned!(span=> #tr::#init(#this, #(#values),*);)
                } else {
                    quote_spanned!(span=> { #(#lets)* #tr::#init(#this, #(#values),*); })
                }
            }
            SuperKind::Base(b) => {
                let Some((_, state)) = base_state(*b) else { return quote!() };
                let st: TokenStream = respan(state.parse().unwrap_or_else(|_| quote!(_)), span);
                let vals: Vec<TokenStream> = args.iter().map(|a| self.value(&a.value)).collect();
                let path = match place {
                    Place::Plain => quote_spanned!(span=> self.__kz_this.__kz_super),
                    Place::Data | Place::Local => quote_spanned!(span=> (#this).__kz_super),
                    Place::Object => quote_spanned!(span=> Self::__kz_state().__kz_super),
                    Place::Trait => {
                        let acc = ident(&format!("__kz_{}", type_snake(&d.info.rust)), span);
                        quote_spanned!(span=> self.#acc().__kz_super)
                    }
                    Place::Enum => return quote!(),
                };
                quote_spanned!(span=> #path.init(#st::new(#(#vals),*));)
            }
            _ => quote!(),
        }
    }

    /// The values of a super constructor's call, in its parameters' order: as given (by place or name), the others
    /// their defaults.
    fn super_args(
        &mut self,
        params: &[Param],
        args: &[Arg],
        binds: &HashMap<String, Type>,
        loc: Loc,
    ) -> Vec<TokenStream> {
        let mut out = Vec::new();
        for (k, p) in params.iter().enumerate() {
            let given = args
                .iter()
                .find(|a| a.name.as_ref().is_some_and(|n| n.name == p.name.name))
                .or_else(|| args.iter().take_while(|a| a.name.is_none()).nth(k));
            let ty = p.ty.as_ref().map(|t| self.ty_of(&substitute(t, binds))).unwrap_or(Ty::Unknown);
            let want = super::expr::Want { some: ty.is_nullable(), ..Default::default() };
            match given.map(|a| &a.value).or(p.default.as_ref()) {
                Some(e) => {
                    let v = self.value_want(e, want);
                    let span = self.span(e.loc);
                    out.push(if ty == Ty::String { quote_spanned!(span=> ::std::string::String::from(#v)) } else { v });
                }
                None => {
                    self.error(loc, format!("give the super constructor's `{}`", p.name.name));
                    out.push(quote!(::core::unreachable!()));
                }
            }
        }
        out
    }

    // ---- Open, abstract and sealed classes -------------------------------------------------------------------------

    /// An open, abstract or sealed class: its state, its members' trait, and the class itself (an open one's
    /// objects; a sealed one's enum of its cases).
    pub(crate) fn open_class(&mut self, d: &ClassDecl) -> TokenStream {
        let ClassAst::Class(c) = &d.ast else { return quote!() };
        if d.info.inherit == Inherit::Sealed
            && let Some(t) = c.type_params.first()
        {
            return self.later(t.loc, "a generic sealed class (its cases' `Nothing`, `out T`)");
        }
        // ((stage 14 WPB) an exception class others derive from: not yet, see `exceptions`)
        if self.exception_super(c).is_some() {
            return self.later(c.name.loc, "an open, abstract or sealed class deriving from an exception class");
        }
        let c = (**c).clone();
        let info = d.info.clone();
        let span = self.span(c.loc);
        let kt = self.kt(span);
        let rt = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let vis = self.class_vis(&c.modifiers, c.loc);
        let allow = super::class::class_allow(span);
        let trait_name = info.members_trait().unwrap_or_default();
        let tname = ident(&trait_name, self.span(c.name.loc));
        let base = ident(&format!("{}Base", info.rust), span);
        let (g_decl, g_use, g_struct) = self.class_generics(d, span);
        let wc = {
            let ctor: Vec<ConstructorParam> = c.primary_constructor.iter().flat_map(|p| p.params.clone()).collect();
            let uses = super::generics::Uses::of_class(&c.body.members, &ctor, info.shape == Shape::Data);
            let bounds = self.type_param_bounds(&c.type_params, &uses, false, span);
            if bounds.is_empty() { quote!() } else { quote_spanned!(span=> where #(#bounds),*) }
        };
        let saved_where = std::mem::replace(&mut self.classes.trait_where, wc.clone());
        let cx = MemberCx {
            owner: trait_name.clone(),
            shape: info.shape,
            companion: info.companion.clone(),
            outers: Vec::new(),
        };
        let sealed_interface = info.shape == Shape::Interface;
        let params: Vec<ConstructorParam> =
            c.primary_constructor.as_ref().map(|p| p.params.clone()).unwrap_or_default();
        // The state.
        let mut fields = Vec::new();
        let mut empties = Vec::new();
        let mut items = Vec::new();
        let mut init = Vec::new();
        let place = quote_spanned!(span=> #tname::__kz_base_of(self));
        let _ = &place;
        let acc = ident(&format!("__kz_{}", type_snake(&info.rust)), span);
        let state = quote_spanned!(span=> self.#acc());
        // (the base class's part, within)
        let mut supertraits = Vec::new();
        for s in self.supers_of(d) {
            let a = Ancestor { kind: s.kind.clone(), args: type_args_of(&s.ty) };
            supertraits.push(self.ancestor_trait(&a, false, span));
            if let SuperKind::Base(b) = &s.kind
                && !supertraits.iter().any(|t| t.to_string().contains("KtObject"))
            {
                let _ = b;
                supertraits.push(quote_spanned!(span=> #kt::KtObject));
            }
        }
        if let Some((f, e)) = self.super_field(d, Place::Plain, span) {
            fields.push(f);
            empties.push(e);
        }
        let root = self.is_root(d);
        if root && !supertraits.iter().any(|t| t.to_string().contains("KtObject")) {
            supertraits.push(quote_spanned!(span=> #kt::KtObject));
        }
        if !sealed_interface {
            items.push(quote_spanned!(span=> #[doc(hidden)] fn #acc(&self) -> &#base #g_use;));
        }
        // (its constructor's parameters: the initializer's, its properties the state's)
        self.fun = FnState::default();
        self.enter_member(&cx, &c.name.name, true);
        let mut init_params = Vec::new();
        for cp in &params {
            let i = self.name_ident(&snake(&cp.param.name.name), cp.param.name.loc);
            let t = cp.param.ty.as_ref().map(|t| self.rust_type(t, TypePos::Value)).unwrap_or_default();
            init_params.push(quote_spanned!(span=> #i: #t));
            let ty = cp.param.ty.as_ref().map(|t| self.ty_of(t)).unwrap_or(Ty::Unknown);
            self.declare_at(&cp.param.name, snake(&cp.param.name.name), LocalKind::Param, ty.clone());
            if let Some(m) = cp.property {
                let pspan = self.span(cp.param.loc);
                let var = m == Mutability::Var;
                fields.push(self.cell_field(&i, &t, var, pspan));
                empties.push(self.cell_empty(&i, var, pspan));
                // (a value the body's initializers read too: a clone of it held)
                let v = if self.copied(&ty) {
                    quote_spanned!(pspan=> #i)
                } else {
                    quote_spanned!(pspan=> ::core::clone::Clone::clone(&#i))
                };
                init.push(if var {
                    quote_spanned!(pspan=> #state.#i.set(#v);)
                } else {
                    quote_spanned!(pspan=> #state.#i.init(#v);)
                });
                let getter = if is_override(&cp.param.modifiers) {
                    ident(&Self::hidden_name(&trait_name, &snake(&cp.param.name.name)), pspan)
                } else {
                    i.clone()
                };
                items.push(quote_spanned!(pspan=> fn #getter(&self) -> #t #wc { #state.#i.get() }));
                if var {
                    let s = format_ident!("set_{}", i.to_string().trim_start_matches("r#"), span = pspan);
                    items.push(quote_spanned!(pspan=> fn #s(&self, value: #t) #wc { #state.#i.set(value) }));
                }
            }
        }
        // (the body: its stored properties the state's, initialized in order with the init blocks; functions and
        // getters the trait's)
        let members = c.body.members.clone();
        let mut fns = Vec::new();
        for m in &members {
            match m {
                Member::Init(b) => {
                    let s = self.braced(b, super::stmt::Tail::Unit);
                    init.push(quote_spanned!(span=> #s));
                }
                Member::Decl(Decl::Property(p)) => {
                    let Binding::Name(n) = &p.binding else {
                        self.later(p.loc, "a destructuring member");
                        continue;
                    };
                    let abstract_ = matches!(p.init, PropertyInit::None) && p.getter.is_none();
                    let getter_only =
                        p.getter.as_ref().is_some_and(|g| g.body.is_some()) && matches!(p.init, PropertyInit::None);
                    if abstract_ || getter_only {
                        fns.push(m.clone());
                        continue;
                    }
                    if let Some((f, e, _g, i)) = self.cell_property(p, &cx, state.clone()) {
                        fields.push(f);
                        empties.push(e);
                        init.extend(i);
                        let pspan = self.span(p.loc);
                        let rust = snake(&n.name);
                        let iv = self.name_ident(&rust, n.loc);
                        let ty = self.quiet(|me| me.member_property_ty(p, &cx));
                        let t = match &p.ty {
                            Some(t) => self.rust_type(t, TypePos::Value),
                            None => self.rust_of_ty(&ty, pspan),
                        };
                        let getter = if is_override(&p.modifiers) {
                            ident(&Self::hidden_name(&trait_name, &rust), pspan)
                        } else {
                            iv.clone()
                        };
                        items.push(quote_spanned!(pspan=> fn #getter(&self) -> #t #wc { #state.#iv.get() }));
                        if p.is_var() {
                            let s = format_ident!("set_{}", rust.trim_start_matches("r#"), span = pspan);
                            items.push(quote_spanned!(pspan=> fn #s(&self, value: #t) #wc { #state.#iv.set(value) }));
                        }
                    }
                }
                Member::Decl(Decl::Function(_)) => fns.push(m.clone()),
                Member::Decl(Decl::Class(_) | Decl::Object(_) | Decl::TypeAlias(_)) => {}
                Member::Decl(d) => {
                    self.later(d.loc(), "this member");
                }
                // (a secondary constructor: the open class's own objects' (its class's), and what a subclass's super
                // call `: X(…)` of its parameters goes through — the primary one's arguments of them)
                Member::Constructor(s) => {
                    if s.delegation.as_ref().is_none_or(|d| d.to_super) {
                        self.later(
                            s.loc,
                            "a secondary constructor of an open, abstract or sealed class not delegating to `this(…)`",
                        );
                    } else if s.body.is_some() && info.inherit != Inherit::Open {
                        self.later(s.loc, "a secondary constructor with a body of an abstract or sealed class");
                    }
                }
            }
        }
        let super_init = self.super_init(d, quote_spanned!(span=> self), Place::Trait, span);
        let lets = self.member_lets(&cx, span);
        let init_fn = ident(&format!("__kz_{}_init", type_snake(&info.rust)), span);
        if !sealed_interface {
            items.push(quote_spanned! {span=>
                #[doc(hidden)]
                fn #init_fn(&self, #(#init_params),*) #wc {
                    #[allow(unused_imports)]
                    use #kt::ext::*;
                    #lets
                    #super_init
                    #rt::named_args!(#kz_types { #(#init)* })
                }
            });
        }
        for m in &fns {
            let saved = (std::mem::take(&mut self.scopes), std::mem::take(&mut self.receivers));
            match m {
                Member::Decl(Decl::Function(f)) => items.extend(self.trait_function(f, &cx, &trait_name)),
                Member::Decl(Decl::Property(p)) => items.extend(self.class_trait_property(p, &cx, &trait_name)),
                _ => {}
            }
            (self.scopes, self.receivers) = saved;
        }
        if root {
            items.push(root_hidden(span));
        }
        let base_struct = if sealed_interface {
            quote!()
        } else {
            quote_spanned! {span=>
                #allow
                #[doc(hidden)]
                #vis struct #base #g_struct { #(#fields),* }
                #allow
                impl #g_use #base #g_use {
                    #[doc(hidden)]
                    pub fn __kz_empty() -> Self { #base { #(#empties),* } }
                }
            }
        };
        let tsup = if supertraits.is_empty() {
            quote_spanned!(span=> #kt::KtObject)
        } else {
            quote_spanned!(span=> #(#supertraits)+*)
        };
        self.classes.trait_where = saved_where;
        let dyn_impls = if info.is_dyn() { self.dyn_impls(d, span) } else { quote!() };
        let ext = self.trait_ext(&trait_name, &c.type_params, &vis, span);
        let rest = match info.inherit {
            Inherit::Open => self.open_class_itself(d, &c),
            Inherit::Sealed => self.sealed_enum(d),
            _ => quote!(),
        };
        let companion =
            if info.inherit != Inherit::Sealed { self.companion_on_dyn(d, &g_decl, &g_use, span) } else { quote!() };
        quote_spanned! {span=>
            #base_struct
            #allow
            #vis trait #tname #g_use: #tsup { #(#items)* }
            #ext
            #dyn_impls
            #companion
            #rest
        }
    }

    /// The extension trait of a trait's generic functions (`TitledExt`), implemented for every value of it.
    fn trait_ext(&mut self, t: &str, tps: &[TypeParam], vis: &TokenStream, span: Span) -> TokenStream {
        let items = std::mem::take(&mut self.classes.trait_ext);
        if items.is_empty() {
            return quote!();
        }
        let name = ident(t, span);
        let ext = format_ident!("{t}Ext", span = span);
        let g_use = generic_use(tps, span);
        let inner = g_decl_inner(tps, span);
        let allow = super::class::class_allow(span);
        quote_spanned! {span=>
            #allow
            #vis trait #ext #g_use: #name #g_use { #(#items)* }
            #allow
            impl<#(#inner,)* __KzS: #name #g_use + ?Sized> #ext #g_use for __KzS {}
        }
    }

    /// `X.Companion` of an abstract class: on its trait object, as an interface's.
    fn companion_on_dyn(
        &mut self,
        d: &ClassDecl,
        g_decl: &TokenStream,
        g_use: &TokenStream,
        span: Span,
    ) -> TokenStream {
        let (Some(cc), Some(t)) = (d.info.companion.clone(), d.info.members_trait()) else { return quote!() };
        if d.info.inherit == Inherit::Open {
            // (an open class's companion is its class's: written with it)
            return quote!();
        }
        let ci = ident(&cc, span);
        let tn = ident(&t, span);
        let allow = super::class::class_allow(span);
        quote_spanned! {span=>
            #allow
            impl #g_decl dyn #tn #g_use {
                #[allow(non_upper_case_globals)]
                pub const Companion: #ci = #ci {};
            }
        }
    }

    /// A property of an open class's trait that has no state: abstract (required) or a getter.
    fn class_trait_property(&mut self, p: &Property, cx: &MemberCx, owner: &str) -> Vec<TokenStream> {
        let Binding::Name(n) = &p.binding else { return Vec::new() };
        if let Some(g) = &p.getter
            && g.body.is_some()
        {
            let f = getter_function(p, n, g);
            return self.trait_function(&f, cx, owner);
        }
        let Some(t) = &p.ty else {
            return vec![self.error_ts(n.loc, &format!("write `{}`'s type", n.name))];
        };
        let pspan = self.span(p.loc);
        let rust = snake(&n.name);
        let i = self.name_ident(&rust, n.loc);
        let t = self.rust_type(t, TypePos::Value);
        let mut out = vec![quote_spanned!(pspan=> fn #i(&self) -> #t;)];
        if p.is_var() {
            let s = format_ident!("set_{}", rust.trim_start_matches("r#"), span = pspan);
            out.push(quote_spanned!(pspan=> fn #s(&self, value: #t);));
        }
        out
    }

    /// An open class's own objects: the class (a plain one deriving from its trait, adding nothing), its
    /// constructor taking the open class's parameters.
    fn open_class_itself(&mut self, d: &ClassDecl, c: &Class) -> TokenStream {
        let params: Vec<ConstructorParam> = c
            .primary_constructor
            .iter()
            .flat_map(|p| p.params.iter())
            .map(|cp| ConstructorParam { property: None, param: cp.param.clone() })
            .collect();
        let args: Vec<Arg> = params
            .iter()
            .map(|cp| Arg {
                name: None,
                spread: false,
                value: Expr { kind: ExprKind::Name(cp.param.name.clone()), loc: cp.param.loc },
                loc: cp.param.loc,
            })
            .collect();
        let self_ty = Type {
            kind: TypeKind::Named(vec![TypeSegment {
                name: c.name.clone(),
                args: c
                    .type_params
                    .iter()
                    .map(|t| TypeArg::Type { variance: None, ty: named_type(&t.name.name, t.loc) })
                    .collect(),
            }]),
            nullable: false,
            annotations: Vec::new(),
            loc: c.name.loc,
        };
        let mut modifiers = c.modifiers.clone();
        modifiers
            .list
            .retain(|m| matches!(m.kind, ModifierKind::Public | ModifierKind::Private | ModifierKind::Internal));
        let synth = Class {
            annotations: c.annotations.clone(),
            modifiers,
            kind: ClassKind::Class,
            name: c.name.clone(),
            type_params: c.type_params.clone(),
            primary_constructor: Some(PrimaryConstructor {
                annotations: Vec::new(),
                modifiers: Modifiers::default(),
                params,
                loc: c.loc,
            }),
            supers: vec![SuperType { ty: self_ty, constructor_args: Some(args), delegate: None, loc: c.loc }],
            constraints: c.constraints.clone(),
            enum_entries: Vec::new(),
            body: ClassBody {
                members: c
                    .body
                    .members
                    .iter()
                    .filter(|m| {
                        matches!(m, Member::Decl(Decl::Object(o)) if o.is_companion())
                            || matches!(m, Member::Constructor(_))
                    })
                    .cloned()
                    .collect(),
                ..Default::default()
            },
            loc: c.loc,
        };
        let mut info = d.info.clone();
        info.inherit = Inherit::Final;
        let sd = ClassDecl { info, ast: ClassAst::Class(Box::new(synth)), outer: d.outer.clone(), own: d.own };
        self.plain_class_decl(&sd)
    }

    /// A sealed class's (or interface's) enum of its cases (see the module's doc).
    fn sealed_enum(&mut self, d: &ClassDecl) -> TokenStream {
        let info = d.info.clone();
        let span = self.span(d.info_loc());
        let kt = self.kt(span);
        let vis = match &d.ast {
            ClassAst::Class(c) => self.class_vis(&c.modifiers, c.loc),
            ClassAst::Object(o) => self.class_vis(&o.modifiers, o.loc),
        };
        let name = ident(&info.rust, span);
        let allow = super::class::class_allow(span);
        let cases = self.sealed_cases(&info.rust);
        let mut variants = Vec::new();
        let mut impls = Vec::new();
        let mut writes = Vec::new();
        for (v, case) in &cases {
            let vi = ident(v, span);
            let ci = ident(case, span);
            variants.push(quote_spanned!(span=> #vi(#ci)));
            writes.push(quote_spanned!(span=> #name::#vi(v) => #kt::KtToString::kt_write(v, __kz_out)));
            impls.push(quote_spanned! {span=>
                #allow
                impl ::core::convert::From<#ci> for #name {
                    fn from(value: #ci) -> #name { #name::#vi(value) }
                }
                #allow
                impl #kt::KtAs<#ci> for #name {
                    fn kt_as_safe(&self) -> ::core::option::Option<#ci> {
                        match self {
                            #name::#vi(v) => ::core::option::Option::Some(::core::clone::Clone::clone(v)),
                            #[allow(unreachable_patterns)]
                            _ => ::core::option::Option::None,
                        }
                    }
                }
                #allow
                impl ::core::cmp::PartialEq<#ci> for #name {
                    fn eq(&self, other: &#ci) -> bool {
                        #[allow(unreachable_patterns)]
                        match self { #name::#vi(v) => v == other, _ => false }
                    }
                }
                #allow
                impl ::core::cmp::PartialEq<#name> for #ci {
                    fn eq(&self, other: &#name) -> bool { other == self }
                }
            });
        }
        // (its members and interfaces: through the case it holds)
        let mut delegations = TokenStream::new();
        let mut traits: Vec<Ancestor> = vec![Ancestor { kind: SuperKind::Class(info.rust.clone()), args: Vec::new() }];
        traits.extend(self.ancestors(d));
        for a in &traits {
            let Some(ad) = self.ancestor_decl(a) else { continue };
            let tr = self.ancestor_trait(a, false, span);
            let trp = self.ancestor_trait(a, true, span);
            let mut items = Vec::new();
            let mut sigs = self.trait_sigs(&ad, &a.args);
            if self.is_root(&ad) {
                sigs.extend(root_sigs());
            }
            for s in &sigs {
                let m = ident(&s.rust, span);
                let names: Vec<&proc_macro2::Ident> = s.params.iter().map(|(n, _)| n).collect();
                let arms = cases.iter().map(|(v, _)| {
                    let vi = ident(v, span);
                    quote_spanned!(span=> #name::#vi(v) => #trp::#m(v, #(#names),*))
                });
                let ps = s.params.iter().map(|(n, t)| quote_spanned!(span=> #n: #t));
                let ret = match (&s.ret, s.suspend) {
                    (Some(r), true) => quote_spanned!(span=> -> #kt::KtFuture<'_, #r>),
                    (None, true) => quote_spanned!(span=> -> #kt::KtFuture<'_, ()>),
                    (Some(r), false) => quote_spanned!(span=> -> #r),
                    (None, false) => quote!(),
                };
                let ret = if s.rust.starts_with("__kz_") && s.rust == format!("__kz_{}", type_snake(&ad.info.rust)) {
                    let b = ident(&format!("{}Base", ad.info.rust), span);
                    quote_spanned!(span=> -> &#b)
                } else if s.rust == "__kz_eq" {
                    quote_spanned!(span=> -> bool)
                } else if s.rust == "__kz_cast" {
                    quote_spanned!(span=> -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>>)
                } else {
                    ret
                };
                let ps = match s.rust.as_str() {
                    "__kz_eq" => quote_spanned!(span=> other: &dyn ::core::any::Any),
                    "__kz_write" => quote_spanned!(span=> __kz_out: &mut ::std::string::String),
                    "__kz_cast" => quote_spanned!(span=> to: ::core::any::TypeId),
                    _ => quote_spanned!(span=> #(#ps),*),
                };
                let vs = ident_list(&cases, span);
                let body = match s.rust.as_str() {
                    "__kz_eq" => quote_spanned!(span=> other.downcast_ref::<Self>().is_some_and(|o| o == self)
                        || match self { #(#name::#vs(v) => #trp::__kz_eq(v, other)),* }),
                    "__kz_write" => quote_spanned!(span=> #kt::KtToString::kt_write(self, __kz_out)),
                    "__kz_cast" => quote_spanned!(span=> match self { #(#name::#vs(v) => #trp::__kz_cast(v, to)),* }),
                    _ => quote_spanned!(span=> match self { #(#arms),* }),
                };
                items.push(quote_spanned!(span=> fn #m(&self, #ps) #ret { #body }));
            }
            if ad.info.rust == info.rust && info.shape != Shape::Interface {
                // (its initializer: never called on the enum, its cases' objects are made by their constructors)
                let init = ident(&format!("__kz_{}_init", type_snake(&info.rust)), span);
                let params = self.init_param_types(&ad);
                items.push(quote_spanned!(span=> fn #init(&self, #(_: #params),*) {}));
            }
            delegations.extend(quote_spanned! {span=>
                #allow
                impl #tr for #name { #(#items)* }
            });
        }
        let companion = match &info.companion {
            Some(cc) => {
                let ci = ident(cc, span);
                quote_spanned! {span=>
                    #allow
                    impl #name {
                        #[allow(non_upper_case_globals)]
                        pub const Companion: #ci = #ci {};
                    }
                }
            }
            None => quote!(),
        };
        let none = if cases.is_empty() { quote_spanned!(span=> _ => {}) } else { quote!() };
        quote_spanned! {span=>
            #allow
            #[derive(::core::clone::Clone, ::core::cmp::PartialEq)]
            #vis enum #name { #(#variants),* }
            #(#impls)*
            #allow
            impl #kt::KtToString for #name {
                fn kt_write(&self, __kz_out: &mut ::std::string::String) {
                    match self { #(#writes,)* #none }
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
            #delegations
            #companion
        }
    }

    /// The Rust types of an open class's initializer's parameters (its constructor's).
    fn init_param_types(&mut self, d: &ClassDecl) -> Vec<TokenStream> {
        match &d.ast {
            ClassAst::Class(c) => c
                .primary_constructor
                .iter()
                .flat_map(|p| p.params.iter())
                .map(|cp| cp.param.ty.as_ref().map(|t| self.rust_type(t, TypePos::Value)).unwrap_or_default())
                .collect(),
            ClassAst::Object(_) => Vec::new(),
        }
    }

    /// A sealed class's cases: (the enum's variant, the case's Rust type), in declaration order.
    pub(crate) fn sealed_cases(&self, rust: &str) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for d in self.classes.decls.clone() {
            if d.info.rust == rust || d.info.shape == Shape::Companion {
                continue;
            }
            let is_case = self.supers_of(&d).iter().any(|s| match &s.kind {
                SuperKind::Class(r) => r == rust,
                _ => false,
            });
            if is_case {
                let v = d.info.kotlin.rsplit('.').next().unwrap_or(&d.info.kotlin).to_string();
                if !out.iter().any(|(x, _)| *x == v) {
                    out.push((v, d.info.rust.clone()));
                }
            }
        }
        out
    }

    /// The sealed class a user's type name is (its Rust name), if it is one.
    pub(crate) fn sealed_of(&self, ty: &Ty) -> Option<String> {
        let Ty::Named(n, _) = ty else { return None };
        self.user_class_rust(n).filter(|c| c.inherit == Inherit::Sealed).map(|c| c.rust.clone())
    }

    /// `when (s) { is S.A -> …; is S.B -> …; else -> … }` of a sealed class's value: a `match` of its enum, each
    /// branch's subject the case it holds (Kotlin's smart cast). None where the `when` isn't one.
    pub(crate) fn sealed_when(&mut self, w: &When, tail: super::stmt::Tail, loc: Loc) -> Option<super::expr::Out> {
        let subject = w.subject.as_ref()?;
        let sty = self.peek_ty(&subject.expr);
        let sealed = self.sealed_of(&sty)?;
        let cases = self.sealed_cases(&sealed);
        // (each branch one `is` of a case, a guard none; or `else`)
        let mut plan = Vec::new();
        for b in &w.branches {
            if b.is_else() {
                plan.push(None);
                continue;
            }
            let ([WhenCondition::Is { negated: false, ty, .. }], None) = (b.conditions.as_slice(), &b.guard) else {
                return None;
            };
            let t = self.ty_of(ty);
            let Ty::Named(case, _) = &t else { return None };
            let v = cases.iter().find(|(_, c)| c == case).map(|(v, _)| v.clone())?;
            plan.push(Some((v, ty.clone(), t.clone())));
        }
        let span = self.span(loc);
        let (ty, tail, shared) = if matches!(tail, super::stmt::Tail::Unit) {
            (Ty::Unit, tail, None)
        } else {
            let tys: Vec<Ty> = w.branches.iter().map(|b| self.peek_block(&b.body)).collect();
            self.joined(&tys, tail)
        };
        self.push_scope();
        let v = self.value(&subject.expr);
        let subject_local: Option<String> = match (&subject.binding, &subject.expr.kind) {
            (Some((n, _)), _) => Some(n.name.clone()),
            (None, ExprKind::Name(n)) if self.is_local(&n.name) => Some(n.name.clone()),
            _ => None,
        };
        let s = self.fresh("w", span);
        let enum_name = ident(&sealed, span);
        let mut arms = Vec::new();
        let mut has_else = false;
        for (b, p) in w.branches.iter().zip(&plan) {
            match p {
                Some((variant, _, t)) => {
                    self.push_scope();
                    let vi = ident(variant, span);
                    let bind = self.fresh("case", span);
                    let rebind = match &subject_local {
                        Some(name) => {
                            let at = self.local(name).and_then(|l| l.at);
                            let rust = snake(name);
                            let i = ident(&rust, span);
                            self.declare(name, rust, LocalKind::Val, t.clone());
                            self.set_local_at(name, at);
                            quote_spanned!(span=> #[allow(unused_variables)] let #i = ::core::clone::Clone::clone(#bind);)
                        }
                        None => quote!(),
                    };
                    let body = self.braced(&b.body, tail);
                    self.pop_scope();
                    arms.push(quote_spanned!(span=> #enum_name::#vi(#bind) => { #rebind #body }));
                }
                None => {
                    has_else = true;
                    let body = self.braced(&b.body, tail);
                    arms.push(quote_spanned!(span=> _ => #body));
                }
            }
        }
        if !has_else && plan.len() < cases.len() && !matches!(tail, super::stmt::Tail::Unit) {
            arms.push(quote_spanned!(span=> _ => ::core::unreachable!("no `when` branch matched")));
        } else if !has_else && plan.len() < cases.len() {
            arms.push(quote_spanned!(span=> _ => {}));
        }
        let bind = match &subject.binding {
            Some((n, _)) => {
                let rust = snake(&n.name);
                let i = self.name_ident(&rust, n.loc);
                self.declare_at(n, rust, LocalKind::Val, sty.clone());
                quote_spanned!(span=> #[allow(unused_variables)] let #i = ::core::clone::Clone::clone(&#s);)
            }
            None => quote!(),
        };
        self.pop_scope();
        let ts = quote_spanned!(span=> { let #s = #v; #bind match &#s { #(#arms),* } });
        Some(super::expr::Out::plain(self.held_shared(ts, shared, span), ty))
    }

    /// An `is`/`as` operand as the cast takes it: an interface's shared object lent as the trait object it holds
    /// (`&*x`: `kt::KtAs` is implemented for `dyn I`).
    pub(crate) fn cast_operand_of(&self, ts: TokenStream, ty: &Ty, span: Span) -> TokenStream {
        let dynamic = match ty.non_null() {
            Ty::Named(n, a) if matches!(n.as_str(), "Rc") && a.len() == 1 => {
                matches!(&a[0], Ty::Named(t, _) if self.user_dyn_trait(t) || t == "KtObject")
            }
            Ty::Named(n, _) => self.user_dyn_trait(&n),
            _ => false,
        };
        if dynamic && !ty.is_nullable() { quote_spanned!(span=> (*#ts)) } else { ts }
    }

    /// Whether a Rust type name is a user's trait whose values are shared trait objects.
    fn user_dyn_trait(&self, name: &str) -> bool {
        self.classes.classes.values().any(|c| c.is_dyn() && c.value_rust() == name)
    }

    /// `super.f(…)` in a class's member: the nearest base class's body of `f` (its hidden method), or an
    /// interface's default (`super<I>.f()` naming it).
    pub(crate) fn super_call(&mut self, ty: Option<&Type>, name: &Ident, args: &[Arg], loc: Loc) -> super::expr::Out {
        let span = self.span(loc);
        let owner = self.receivers.iter().rev().find(|r| r.rust == "self").map(|r| r.ty.clone());
        let Some(owner) = owner else {
            return super::expr::Out::plain(self.error_ts(loc, "`super` is a class's member's"), Ty::Unknown);
        };
        // (the class whose member this is: by its Rust name, or its members' trait's)
        let d = self
            .classes
            .decls
            .iter()
            .find(|d| d.info.rust == owner || d.info.members_trait().as_deref() == Some(owner.as_str()))
            .cloned();
        let Some(d) = d else {
            return super::expr::Out::plain(self.error_ts(loc, "`super` here: no class around it"), Ty::Unknown);
        };
        let rust = snake(&name.name);
        let mut candidates: Vec<Ancestor> = Vec::new();
        if let Some(t) = ty.and_then(|t| t.simple_name())
            && let Some(k) = self.super_kind(&t)
        {
            candidates.extend(self.ancestors(&d).into_iter().filter(|a| a.kind == k));
        } else {
            candidates.extend(self.ancestors(&d));
        }
        for a in candidates {
            let Some(ad) = self.ancestor_decl(&a) else { continue };
            let sigs = self.trait_sigs(&ad, &a.args);
            let tname = ad.info.members_trait().unwrap_or_default();
            let hidden = Self::hidden_name(&tname, &rust);
            let Some(s) = sigs.iter().find(|s| s.rust == hidden).cloned() else { continue };
            let path = self.ancestor_trait(&a, true, span);
            let h = ident(&hidden, span);
            let vals: Vec<TokenStream> = args
                .iter()
                .map(|x| self.value_want(&x.value, super::expr::Want { str_ok: false, ..Default::default() }))
                .collect();
            let ret = s.ret.as_ref().map(|r| Ty::from_rust(&r.to_string())).unwrap_or(Ty::Unit);
            let ts = quote_spanned!(span=> #path::#h(self, #(#vals),*));
            let ts = if s.suspend {
                let a = self.awaited(Some(&name.name), loc);
                quote_spanned!(span=> #ts #a)
            } else {
                ts
            };
            return super::expr::Out::plain(ts, ret);
        }
        super::expr::Out::plain(
            self.error_ts(
                name.loc,
                &format!("`super.{}`: no base class or interface above has a body of it", name.name),
            ),
            Ty::Unknown,
        )
    }
}

impl Gen<'_, '_> {
    /// `F { x -> … }` of a user's fun interface `F` (its type arguments those the call writes, `Mapper<Int> { … }`):
    /// the lambda as the closure its trait is implemented for, made the shared object (`Rc<dyn F>`).
    pub(crate) fn user_sam(&mut self, n: &Ident, l: &Lambda, loc: Loc) -> Option<super::expr::Out> {
        let c = self.user_class(&n.name)?.clone();
        if !c.fun_interface || self.is_local(&n.name) {
            return None;
        }
        let d = self.decl_of(&c.rust)?;
        let ClassAst::Class(k) = &d.ast else { return None };
        let f = k.body.members.iter().find_map(|m| match m {
            Member::Decl(Decl::Function(f)) if f.body.is_none() => Some(f.clone()),
            _ => None,
        })?;
        let span = self.span(loc);
        let targs: Vec<TokenStream> = self.call_type_args.last().cloned().unwrap_or_default();
        let tys: Vec<Ty> = self.call_type_tys.last().cloned().unwrap_or_default();
        // (a type parameter of the interface: the call's argument for it)
        let text_of = |me: &mut Self, t: &Type| -> String {
            if let Some(name) = t.simple_name()
                && let Some(k) = c.type_params.iter().position(|p| *p == name)
                && let Some(a) = targs.get(k)
            {
                return a.to_string();
            }
            me.quiet(|me| me.rust_type(t, TypePos::Value)).to_string()
        };
        let mut inputs = Vec::new();
        for p in &f.params {
            let Some(t) = &p.ty else { continue };
            inputs.push(text_of(self, t));
        }
        let output = match &f.result {
            Some(t) if self.ty_of(t) != Ty::Unit => text_of(self, t),
            _ => "()".to_string(),
        };
        let info = LInfo {
            kind: index::LambdaKind::Fn,
            inputs,
            output,
            receiver: None,
            receiver_value: None,
            suspend: f.is_suspend(),
            boxed: false,
            stored: true,
            measure_policy: false,
            typed: true,
            twin: false,
            file_inline: false,
            snapshot_block: false,
            crossinline: None,
        };
        let closure = self.lambda_for(l, Some(&info), Some(&n.name), false);
        let t = ident(&c.rust, self.span(n.loc));
        let tr = if targs.is_empty() { quote_spanned!(span=> #t) } else { quote_spanned!(span=> #t<#(#targs),*>) };
        let o = self.fresh("f", span);
        Some(super::expr::Out::plain(
            quote_spanned!(span=> { let #o: ::std::rc::Rc<dyn #tr> = ::std::rc::Rc::new(#closure); #o }),
            Ty::Named(c.rust.clone(), tys),
        ))
    }
}

impl Gen<'_, '_> {
    /// The result type (Rust's text) of the member an override of `owner` overrides, where a supertype writes it.
    pub(crate) fn overridden_result(&mut self, owner: &str, f: &Function) -> Option<String> {
        let d = self
            .classes
            .decls
            .iter()
            .find(|d| d.info.rust == owner || d.info.members_trait().as_deref() == Some(owner))
            .cloned()?;
        for a in self.ancestors(&d) {
            let Some(ad) = self.ancestor_decl(&a) else { continue };
            let binds: HashMap<String, Type> =
                ad.info.type_params.iter().cloned().zip(a.args.iter().cloned()).collect();
            let found = members_of_ast(&ad.ast).iter().find_map(|m| match m {
                Member::Decl(Decl::Function(g)) if g.name.name == f.name.name && g.params.len() == f.params.len() => {
                    g.result.clone()
                }
                _ => None,
            });
            if let Some(t) = found {
                let t = substitute(&t, &binds);
                return Some(self.quiet(|me| me.rust_type(&t, TypePos::Return)).to_string());
            }
        }
        None
    }
}

impl Gen<'_, '_> {
    /// Whether a value of `from` goes where `to` is taken through [`coerce`](Self::coerce): a case of a sealed
    /// class where the class is (its enum), a class's value where an interface or an open class it is is (its
    /// shared object).
    pub(crate) fn coerces(&self, from: &Ty, to: &Ty) -> bool {
        // (a value of a type of its own where Kotlin's `Any` is: the shared object)
        if is_any(&to.non_null()) {
            return !matches!(from.non_null(), Ty::Unknown | Ty::Unit | Ty::Fun(_) | Ty::Nullable(_))
                && !matches!(from.non_null(), Ty::Named(n, _) if matches!(n.as_str(), "Rc" | "Arc" | "Box") || self.user_dyn_trait(&n));
        }
        let (Ty::Named(f, _), Ty::Named(t, _)) = (from.non_null(), to.non_null()) else { return false };
        if f == t {
            return false;
        }
        if let Some(s) =
            self.user_class_rust(&t).filter(|c| c.inherit == Inherit::Sealed && c.shape != Shape::Companion)
        {
            return self.is_case_of(&f, &s.rust);
        }
        self.user_dyn_trait(&t)
            && !self.user_dyn_trait(&f)
            && !matches!(f.as_str(), "Rc" | "Arc" | "Box")
            && self.user_class_rust(&f).is_some()
    }

    /// Whether a user's type is a case of the sealed class `sealed` (directly, or a case of one of its cases).
    pub(crate) fn is_case_of(&self, rust: &str, sealed: &str) -> bool {
        let Some(d) = self.decl_of(rust) else { return false };
        self.ancestors(&d).iter().any(|a| matches!(&a.kind, SuperKind::Class(r) if r == sealed))
    }

    /// A value of `from` where `to` is taken (see [`coerces`](Self::coerces)); as it is otherwise.
    pub(crate) fn coerce(&mut self, v: TokenStream, from: &Ty, to: &Ty, span: Span) -> TokenStream {
        if !self.coerces(from, to) {
            return v;
        }
        let Ty::Named(t, _) = to.non_null() else { return v };
        let v = if is_any(&to.non_null()) {
            let kt = self.kt(span);
            let f = self.fresh("any", span);
            quote_spanned!(span=> { let #f: ::std::rc::Rc<dyn #kt::KtObject> = ::std::rc::Rc::new(#v); #f })
        } else if self.user_class_rust(&t).is_some_and(|c| c.inherit == Inherit::Sealed) {
            let s = ident(&t, span);
            quote_spanned!(span=> ::core::convert::Into::<#s>::into(#v))
        } else {
            let tt = self.rust_of_ty(&to.non_null(), span);
            let f = self.fresh("shared", span);
            quote_spanned!(span=> { let #f: #tt = ::std::rc::Rc::new(#v); #f })
        };
        if to.is_nullable() && !from.is_nullable() {
            quote_spanned!(span=> ::core::option::Option::Some(#v))
        } else {
            v
        }
    }

    /// The type a collection's elements of several user's types share: the sealed class they are cases of.
    pub(crate) fn common_sealed(&self, tys: &[Ty]) -> Option<Ty> {
        let names: Vec<&str> = tys
            .iter()
            .map(|t| match t {
                Ty::Named(n, _) => Some(n.as_str()),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let first = self.decl_of(names.first()?)?;
        let sealed: Vec<String> = self
            .ancestors(&first)
            .into_iter()
            .filter_map(|a| match a.kind {
                SuperKind::Class(r) if self.user_class_rust(&r).is_some_and(|c| c.inherit == Inherit::Sealed) => {
                    Some(r)
                }
                _ => None,
            })
            .collect();
        sealed.into_iter().find(|s| names.iter().all(|n| *n == s || self.is_case_of(n, s))).map(|s| Ty::named(&s))
    }
}

/// Whether a type is Kotlin's `Any` (`Rc<dyn kt::KtObject>`).
pub(crate) fn is_any(t: &Ty) -> bool {
    matches!(t, Ty::Named(n, a) if n == "Rc" && matches!(a.as_slice(), [Ty::Named(k, x)] if k == "KtObject" && x.is_empty()))
}

/// A type's name as Rust's names of its items take it (`PlateOpen` → `plate_open`).
pub(crate) fn type_snake(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

impl Gen<'_, '_> {
    /// The trait an object expression extending a class (or several types) is a value of: its class's (`BOpen`, an
    /// API base's), else its first interface's.
    pub(crate) fn object_class_trait(&self, supers: &[SuperType]) -> Option<(String, Vec<Type>)> {
        let mut first = None;
        for s in supers {
            let kind = s.ty.simple_name().and_then(|n| self.super_kind(&n))?;
            let args = type_args_of(&s.ty);
            let t = match &kind {
                SuperKind::Class(r) => {
                    let c = self.user_class_rust(r)?;
                    if c.inherit == Inherit::Sealed {
                        return None;
                    }
                    return Some((c.members_trait()?, args));
                }
                SuperKind::Base(b) => return Some((b.rust.to_string(), args)),
                SuperKind::Interface(r) => r.clone(),
                SuperKind::Api(t) => t.clone(),
            };
            first.get_or_insert((t, args));
        }
        first
    }

    /// `object : Base(args), I { … }`: a struct of its own holding the base class's part (a user's open or abstract
    /// class's `BBase`, an API base's state), its overrides as closures of the object (`__kz_this`, their
    /// receiver), its `override val`s' values; each supertype's trait implemented, the base's initializer run on the
    /// object made; its value the class's shared object (`Rc<dyn BOpen>`, `Rc<dyn ViewModel>`).
    pub(crate) fn object_expr_class(&mut self, supers: &[SuperType], body: &ClassBody, loc: Loc) -> TokenStream {
        let span = self.span(loc);
        for s in supers {
            if s.ty.simple_name().and_then(|n| self.super_kind(&n)).is_none() {
                return self.error_ts(
                    s.loc,
                    &format!("`{}` isn't a class or interface KompozR knows", s.ty.simple_name().unwrap_or_default()),
                );
            }
            if s.delegate.is_some() {
                return self.later(s.loc, "an object expression's interface by delegation");
            }
        }
        let Some((main, main_args)) = self.object_class_trait(supers) else {
            return self.later(loc, "an object expression of a sealed class");
        };
        let d = ClassDecl {
            info: super::class::UserClass {
                kotlin: "<object>".into(),
                rust: "__KzObject".into(),
                shape: Shape::Object,
                companion: None,
                inherit: Inherit::Final,
                type_params: Vec::new(),
                inner: None,
                fun_interface: false,
            },
            ast: ClassAst::Object(Box::new(Object {
                annotations: Vec::new(),
                modifiers: Modifiers::default(),
                name: None,
                supers: supers.to_vec(),
                body: body.clone(),
                loc,
            })),
            outer: None,
            own: true,
        };
        let obj = ident("__KzObject", span);
        let mut fields = Vec::new();
        let mut values = Vec::new();
        if let Some((f, e)) = self.super_field(&d, Place::Plain, span) {
            fields.push(f);
            values.push(e);
        }
        let init = self.super_init(&d, quote_spanned!(span=> &__kz_o), Place::Data, span);
        let ancestors = self.ancestors(&d);
        let chain = self.class_chain(&d);
        // (the class trait the overrides' bodies see the object as: its members theirs)
        let recv_trait = main.clone();
        let main_path = self.ancestor_trait_named(&main, &main_args, span);
        // (each own member: the closure or value it is, by Kotlin's name)
        let mut overrides: Vec<(String, usize, TokenStream)> = Vec::new();
        let mut k = 0;
        for m in &body.members {
            match m {
                Member::Decl(Decl::Function(f))
                    if f.receiver.is_none() && f.type_params.is_empty() && !f.is_suspend() =>
                {
                    let Some((params, ret)) = self.object_override_sig(&ancestors, &f.name.name, f.params.len(), false)
                    else {
                        self.error(
                            f.name.loc,
                            format!("`{}` overrides no function of the object's supertypes", f.name.name),
                        );
                        continue;
                    };
                    let fspan = self.span(f.loc);
                    let names: Vec<proc_macro2::Ident> =
                        f.params.iter().map(|p| self.name_ident(&snake(&p.name.name), p.name.loc)).collect();
                    let field = format_ident!("m{k}", span = fspan);
                    k += 1;
                    let r = ret.as_ref().map(|r| quote_spanned!(fspan=> -> #r));
                    fields.push(
                        quote_spanned!(fspan=> #field: ::std::rc::Rc<dyn ::core::ops::Fn(&#obj, #(#params),*) #r>),
                    );
                    let closure = self.object_fn_closure_with(
                        f,
                        &params,
                        ret.as_ref(),
                        &names,
                        Some((&quote!(#obj), &recv_trait)),
                    );
                    values.push(quote_spanned!(fspan=> #field: ::std::rc::Rc::new(#closure)));
                    overrides.push((
                        f.name.name.clone(),
                        f.params.len(),
                        quote_spanned!(fspan=> (self.#field)(self, #(#names),*)),
                    ));
                }
                Member::Decl(Decl::Property(p)) if !p.is_var() && matches!(p.init, PropertyInit::Value(_)) => {
                    let (Binding::Name(n), PropertyInit::Value(v)) = (&p.binding, &p.init) else { continue };
                    let Some((_, Some(t))) = self.object_override_sig(&ancestors, &n.name, 0, true) else {
                        self.error(n.loc, format!("`{}` overrides no property of the object's supertypes", n.name));
                        continue;
                    };
                    let pspan = self.span(p.loc);
                    let field = format_ident!("p{k}", span = pspan);
                    k += 1;
                    let val = self.value_want(v, super::expr::Want { str_ok: false, ..Default::default() });
                    fields.push(quote_spanned!(pspan=> #field: #t));
                    values.push(quote_spanned!(pspan=> #field: #val));
                    overrides.push((
                        n.name.clone(),
                        0,
                        quote_spanned!(pspan=> ::core::clone::Clone::clone(&self.#field)),
                    ));
                }
                Member::Decl(d) => {
                    self.later(d.loc(), "this member of an object expression extending a class (a `var`, a suspending or generic function, a declaration)");
                }
                Member::Init(b) => {
                    self.later(b.loc, "an `init` block in an object expression");
                }
                Member::Constructor(c) => self.error(c.loc, "an object has no constructors"),
            }
        }
        // The traits: each supertype's, what the object overrides routed to its closures.
        let kt = self.kt(span);
        let mut impls = TokenStream::new();
        for a in &ancestors {
            let tr = self.ancestor_trait(a, false, span);
            let mut items = Vec::new();
            match &a.kind {
                SuperKind::Interface(_) | SuperKind::Class(_) => {
                    let Some(ad) = self.ancestor_decl(a) else { continue };
                    if let SuperKind::Class(r) = &a.kind
                        && ad.info.shape != Shape::Interface
                        && let Some(depth) = std::iter::once(d.clone()).chain(chain.clone()).position(|x| {
                            self.superclass(&x).is_some_and(|s| matches!(&s.kind, SuperKind::Class(c) if c == r))
                        })
                    {
                        let acc = ident(&format!("__kz_{}", type_snake(r)), span);
                        let base = ident(&format!("{r}Base"), span);
                        let bargs: Vec<TokenStream> =
                            a.args.iter().map(|t| self.rust_type(t, TypePos::Value)).collect();
                        let bg = if bargs.is_empty() { quote!() } else { quote_spanned!(span=> <#(#bargs),*>) };
                        let path = base_path(Place::Local, depth, span);
                        items.push(quote_spanned!(span=> fn #acc(&self) -> &#base #bg { #path }));
                    }
                    for s in self.trait_sigs(&ad, &a.args) {
                        if s.role == Role::Hidden {
                            continue;
                        }
                        let own = overrides.iter().find(|(n, ar, _)| {
                            *n == s.kotlin && (s.role != Role::Function || *ar == s.arity) && s.role != Role::Setter
                        });
                        let body = match own {
                            Some((_, _, b)) => Some(b.clone()),
                            None => chain.iter().take_while(|b| b.info.rust != ad.info.rust).find_map(|b| {
                                let bc: Vec<ConstructorParam> = match &b.ast {
                                    ClassAst::Class(c) => {
                                        c.primary_constructor.iter().flat_map(|p| p.params.clone()).collect()
                                    }
                                    ClassAst::Object(_) => Vec::new(),
                                };
                                crate::codegen::hierarchy::overrides(members_of_ast(&b.ast), &bc, &s).then(|| {
                                    let bt = b.info.members_trait().unwrap_or_default();
                                    let h = ident(&Self::hidden_name(&bt, &s.rust), span);
                                    let t = ident(&bt, span);
                                    let names: Vec<&proc_macro2::Ident> = s.params.iter().map(|(n, _)| n).collect();
                                    quote_spanned!(span=> #t::#h(self, #(#names),*))
                                })
                            }),
                        };
                        if let Some(b) = body {
                            items.push(sig_with_body(&s, b, span, &kt));
                        }
                    }
                }
                SuperKind::Base(b) => {
                    if let Some((acc, state)) = base_state(*b) {
                        let depth = chain.len();
                        let path = base_path(Place::Local, depth, span);
                        let a = ident(acc, span);
                        let s: TokenStream = respan(state.parse().unwrap_or_else(|_| quote!(_)), span);
                        items.push(quote_spanned!(span=> fn #a(&self) -> &#s { (#path).get_ref() }));
                    }
                    items.extend(self.object_api_overrides(b.rust, &overrides, span));
                }
                SuperKind::Api(t) => {
                    let t = t.clone();
                    items.extend(self.object_api_overrides(&t, &overrides, span));
                }
            }
            impls.extend(quote_spanned!(span=> #[allow(non_snake_case, clippy::all)] impl #tr for #obj { #(#items)* }));
        }
        let ptr = if trait_object(&main) == Some("Arc") {
            quote_spanned!(span=> ::std::sync::Arc)
        } else {
            quote_spanned!(span=> ::std::rc::Rc)
        };
        let o = self.fresh("object", span);
        quote_spanned! {span=>
            {
                #[allow(non_camel_case_types, clippy::type_complexity)]
                struct #obj { #(#fields),* }
                #impls
                let __kz_o = #obj { #(#values),* };
                #init
                let #o: #ptr<dyn #main_path> = #ptr::new(__kz_o);
                #o
            }
        }
    }

    /// A trait's path by its Rust name and type arguments (`PlateOpen`, `Feed<i32>`).
    fn ancestor_trait_named(&mut self, t: &str, args: &[Type], span: Span) -> TokenStream {
        let i = ident(t, span);
        let a: Vec<TokenStream> = args.iter().map(|x| self.rust_type(x, TypePos::Value)).collect();
        if a.is_empty() { quote_spanned!(span=> #i) } else { quote_spanned!(span=> #i<#(#a),*>) }
    }

    /// The signature (Rust's parameter types and result) of the member of an object's supertypes its member named
    /// `name` overrides: a user's trait's (by its declaration), an API trait's (by the index).
    fn object_override_sig(
        &mut self,
        ancestors: &[Ancestor],
        name: &str,
        arity: usize,
        property: bool,
    ) -> Option<(Vec<TokenStream>, Option<TokenStream>)> {
        for a in ancestors {
            match &a.kind {
                SuperKind::Interface(_) | SuperKind::Class(_) => {
                    let Some(ad) = self.ancestor_decl(a) else { continue };
                    let found = self.trait_sigs(&ad, &a.args).into_iter().find(|s| {
                        s.kotlin == name
                            && if property {
                                s.role == Role::Getter
                            } else {
                                s.role == Role::Function && s.arity == arity
                            }
                    });
                    if let Some(s) = found {
                        return Some((s.params.into_iter().map(|(_, t)| t).collect(), s.ret));
                    }
                }
                SuperKind::Base(b) => {
                    if let Some(x) = api_sig(b.rust, name, arity, a.kind.clone()) {
                        return Some(x);
                    }
                }
                SuperKind::Api(t) => {
                    if let Some(x) = api_sig(t, name, arity, a.kind.clone()) {
                        return Some(x);
                    }
                }
            }
        }
        None
    }

    /// An API trait's methods an object's members override, routed to them.
    fn object_api_overrides(
        &mut self,
        t: &str,
        overrides: &[(String, usize, TokenStream)],
        span: Span,
    ) -> Vec<TokenStream> {
        let mut out = Vec::new();
        for (name, arity, body) in overrides {
            let Some(item) = index::lookup(name)
                .iter()
                .find(|i| i.via == t && i.kind == index::Kind::Method && i.params.len() == *arity)
            else {
                continue;
            };
            out.push(api_method(item, body.clone(), span));
        }
        out
    }
}

/// An API trait's method's parameter types and result, by its Kotlin name.
fn api_sig(t: &str, name: &str, arity: usize, _kind: SuperKind) -> Option<(Vec<TokenStream>, Option<TokenStream>)> {
    let item =
        index::lookup(name).iter().find(|i| i.via == t && i.kind == index::Kind::Method && i.params.len() == arity)?;
    let parse = |s: &str| -> TokenStream { s.parse().unwrap_or_else(|_| quote!(_)) };
    let params = item.params.iter().map(|p| parse(p.ty)).collect();
    let ret = (item.ret.trim() != "()").then(|| parse(item.ret));
    Some((params, ret))
}

/// A named type of a name alone (`T`).
pub(crate) fn named_type(name: &str, loc: Loc) -> Type {
    Type {
        kind: TypeKind::Named(vec![TypeSegment {
            name: Ident { name: name.to_string(), backticked: false, loc },
            args: Vec::new(),
        }]),
        nullable: false,
        annotations: Vec::new(),
        loc,
    }
}

/// `<T, U>` of type parameters' names.
pub(crate) fn generic_use_names(names: &[String], span: Span) -> TokenStream {
    if names.is_empty() {
        return quote!();
    }
    let names = names.iter().map(|p| ident(p, span));
    quote_spanned!(span=> <#(#names),*>)
}

/// `<T, U>` of type parameters.
pub(crate) fn generic_use(params: &[TypeParam], span: Span) -> TokenStream {
    if params.is_empty() {
        return quote!();
    }
    let names = params.iter().map(|p| ident(&p.name.name, span));
    quote_spanned!(span=> <#(#names),*>)
}

/// `T: Clone + 'static, …` of type parameters (an impl's of a trait object's, its blanket impl's).
fn g_decl_inner(params: &[TypeParam], span: Span) -> Vec<TokenStream> {
    params
        .iter()
        .map(|p| {
            let i = ident(&p.name.name, span);
            quote_spanned!(span=> #i: ::core::clone::Clone + 'static)
        })
        .collect()
}

/// The variants' names of a sealed class's cases, as identifiers.
fn ident_list(cases: &[(String, String)], span: Span) -> Vec<proc_macro2::Ident> {
    cases.iter().map(|(v, _)| ident(v, span)).collect()
}

/// A getter's body as a function's (`val big get() = …` → `fun big() = …`).
pub(crate) fn getter_function(p: &Property, n: &Ident, g: &Accessor) -> Function {
    Function {
        annotations: Vec::new(),
        modifiers: p.modifiers.clone(),
        type_params: Vec::new(),
        receiver: None,
        name: n.clone(),
        params: Vec::new(),
        result: p.ty.clone().or_else(|| g.ty.clone()),
        constraints: Vec::new(),
        body: g.body.clone(),
        loc: p.loc,
    }
}

/// The hidden methods a root trait declares, with the defaults an object without a class of its own (an object
/// expression, a closure) answers with: identity, Kotlin's default text, no other types.
fn root_hidden(span: Span) -> TokenStream {
    quote_spanned! {span=>
        #[doc(hidden)]
        fn __kz_eq(&self, other: &dyn ::core::any::Any) -> bool {
            ::core::ptr::addr_eq(self as *const Self as *const (), other as *const dyn ::core::any::Any as *const ())
        }
        #[doc(hidden)]
        fn __kz_write(&self, __kz_out: &mut ::std::string::String) {
            let name = ::std::any::type_name::<Self>();
            let name = name.split('<').next().unwrap_or(name);
            let name = name.rsplit("::").next().unwrap_or(name);
            __kz_out.push_str(&::std::format!("{}@{:x}", name, (self as *const Self as *const () as usize) & 0xffff_ffff));
        }
        #[doc(hidden)]
        fn __kz_cast(&self, to: ::core::any::TypeId) -> ::core::option::Option<::std::boxed::Box<dyn ::core::any::Any>> {
            let _ = to;
            ::core::option::Option::None
        }
    }
}

/// The root hidden methods as signatures (an enum delegating them).
fn root_sigs() -> Vec<Sig> {
    ["__kz_eq", "__kz_write", "__kz_cast"]
        .iter()
        .map(|r| Sig {
            kotlin: String::new(),
            rust: r.to_string(),
            params: Vec::new(),
            ret: None,
            suspend: false,
            role: Role::Hidden,
            arity: 0,
            required: false,
        })
        .collect()
}

/// A method of a signature with a body.
fn sig_with_body(s: &Sig, body: TokenStream, span: Span, kt: &TokenStream) -> TokenStream {
    let m = ident(&s.rust, span);
    let ps = s.params.iter().map(|(n, t)| quote_spanned!(span=> #n: #t));
    let ret = match (&s.ret, s.suspend) {
        (Some(r), true) => quote_spanned!(span=> -> #kt::KtFuture<'_, #r>),
        (None, true) => quote_spanned!(span=> -> #kt::KtFuture<'_, ()>),
        (Some(r), false) => quote_spanned!(span=> -> #r),
        (None, false) => quote!(),
    };
    quote_spanned!(span=> fn #m(&self, #(#ps),*) #ret { #body })
}

/// An API trait's method written with its index signature, its body given.
fn api_method(item: &index::Item, body: TokenStream, span: Span) -> TokenStream {
    let parse = |t: &str| -> TokenStream { respan(t.parse().unwrap_or_else(|_| quote!(_)), span) };
    let m = ident(item.rust, span);
    let ps = item.params.iter().map(|p| {
        let n = ident(p.rust, span);
        let t = parse(p.ty);
        quote_spanned!(span=> #n: #t)
    });
    let ret = if item.ret.trim() == "()" {
        quote!()
    } else {
        let r = parse(item.ret);
        quote_spanned!(span=> -> #r)
    };
    let recv = match item.receiver {
        index::Receiver::RefMut => quote_spanned!(span=> &mut self),
        index::Receiver::Value => quote_spanned!(span=> self),
        _ => quote_spanned!(span=> &self),
    };
    quote_spanned!(span=> fn #m(#recv, #(#ps),*) #ret { #body })
}

/// An API base's state: its accessor (a required method giving `&State`) and the state's type.
fn base_state(b: index::Base) -> Option<(&'static str, String)> {
    for r in b.required {
        let item = index::ITEMS
            .iter()
            .copied()
            .chain(index::overlay::items())
            .find(|i| i.via == b.rust && i.rust == *r && i.params.is_empty());
        if let Some(i) = item
            && let Some(state) = i.ret.trim().strip_prefix('&')
        {
            return Some((i.rust, state.trim().to_string()));
        }
    }
    None
}

/// Where a class's base part at `depth` below it is (0: its own `__kz_super`).
fn base_path(place: Place, depth: usize, span: Span) -> TokenStream {
    let mut p = match place {
        Place::Plain => quote_spanned!(span=> &self.__kz_this.__kz_super),
        Place::Data => quote_spanned!(span=> &*self.__kz_super),
        Place::Object => quote_spanned!(span=> &Self::__kz_state().__kz_super),
        Place::Local => quote_spanned!(span=> &self.__kz_super),
        Place::Enum | Place::Trait => quote_spanned!(span=> ::core::unreachable!()),
    };
    for _ in 0..depth {
        p = quote_spanned!(span=> &(#p).__kz_super);
    }
    p
}

use quote::ToTokens;
