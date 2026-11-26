//! Kotlin's types as Rust's: `Int` → `i32`, `String?` → `Option<String>`, `List<T>` → `Vec<T>`, a parameter's
//! `String` → `impl Into<String>`, a function type → `impl Fn(…) -> R + 'static`; and the generator's own view of a
//! type ([`Ty`]).

use super::resolve::{FunTy, LInfo, Ty};
use super::{Gen, last_segment};
use crate::ast::*;
use crate::index::LambdaKind;
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};

/// Where a type is written.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypePos {
    /// A parameter: `String` → `impl Into<String>`, `T?` → `impl Into<Option<T>>`, a function → `impl Fn…`.
    Param,
    /// A parameter with a default: its type itself (the builder's default is a value of it), a function → `impl Fn…`.
    DefaultedParam,
    /// A function's result: a function type → `impl Fn…`.
    Return,
    /// Anything else.
    Value,
}

/// Receivers Rust's API doesn't pass (its content lambdas take none: `Row { … }`): a function type with one of them
/// has no receiver parameter.
const IMPLICIT_SCOPES: &[&str] = &[
    "RowScope",
    "ColumnScope",
    "BoxScope",
    "LazyItemScope",
    "FlowRowScope",
    "FlowColumnScope",
    "BoxWithConstraintsScope",
];

/// What `impl Into<T>` converts to (`T`), for a parameter's conversion in a body.
pub(crate) fn into_target(rust: &TokenStream) -> Option<TokenStream> {
    let text = rust.to_string().replace(' ', "");
    if !text.starts_with("implInto<") && !text.starts_with("impl::core::convert::Into<") {
        return None;
    }
    let mut it = rust.clone().into_iter().skip_while(|t| t.to_string() != "<");
    it.next();
    let inner: Vec<proc_macro2::TokenTree> = it.collect();
    // (without the closing `>`)
    let n = inner.len().saturating_sub(1);
    Some(inner.into_iter().take(n).collect())
}

impl Gen<'_, '_> {
    /// What a Kotlin type is to the generator.
    pub(crate) fn ty_of(&self, t: &Type) -> Ty {
        // (a type alias is the type it names)
        if let Some(expanded) = self.expand_alias(t) {
            return self.ty_of(&expanded);
        }
        let base = match &t.kind {
            TypeKind::Named(segs)
                if segs.len() > 1
                    && let Some(rust) = crate::index::rust_type(
                        &segs.iter().map(|s| s.name.name.as_str()).collect::<Vec<_>>().join("."),
                    ) =>
            {
                Ty::Named(rust.to_string(), Vec::new())
            }
            TypeKind::Named(segs) => {
                let last = segs.last().expect("a type has a name");
                let args: Vec<Ty> = last
                    .args
                    .iter()
                    .map(|a| match a {
                        TypeArg::Type { ty, .. } => self.ty_of(ty),
                        TypeArg::Star(_) => Ty::Unknown,
                    })
                    .collect();
                let first = || args.first().cloned().unwrap_or(Ty::Unknown);
                match last.name.name.as_str() {
                    "Int" => Ty::Int,
                    "Long" => Ty::Long,
                    "Float" => Ty::Float,
                    "Double" => Ty::Double,
                    "Boolean" => Ty::Bool,
                    "Char" => Ty::Char,
                    "String" | "CharSequence" => Ty::String,
                    "Unit" => Ty::Unit,
                    "Any" if args.is_empty() => Ty::Named("Rc".into(), vec![Ty::named("KtObject")]),
                    "Short" | "Byte" | "UInt" | "ULong" | "UShort" | "UByte" => Ty::OtherInt,
                    "List" | "MutableList" | "ArrayList" | "Collection" | "Iterable" | "Array" => {
                        Ty::List(Box::new(first()))
                    }
                    "IntArray" => Ty::List(Box::new(Ty::Int)),
                    "FloatArray" => Ty::List(Box::new(Ty::Float)),
                    "Set" | "MutableSet" | "HashSet" => Ty::Set(Box::new(first())),
                    "Map" | "MutableMap" | "HashMap" => {
                        Ty::Map(Box::new(first()), Box::new(args.get(1).cloned().unwrap_or(Ty::Unknown)))
                    }
                    "Pair" => Ty::Pair(Box::new(first()), Box::new(args.get(1).cloned().unwrap_or(Ty::Unknown))),
                    "IntRange" => Ty::Range(Box::new(Ty::Int)),
                    "ClosedFloatingPointRange" | "ClosedRange" => Ty::ClosedRange(Box::new(first())),
                    // (stage 14 WPB) Kotlin's exception classes: one Rust type, the class the object's
                    name if self.exception_type(name) => Ty::named("Throwable"),
                    // (a user's type by its Rust name: a nested one named alone inside its class; an open or
                    // abstract class's values its trait's objects)
                    name if self.user_class(name).is_some() => {
                        Ty::Named(self.user_class(name).map(|c| c.value_rust()).unwrap_or_default(), args)
                    }
                    // (an alias of the API's or of the crate's Rust: what it names, for its members)
                    name if args.is_empty() && crate::index::alias(name).is_some() => {
                        Ty::from_rust(crate::index::alias(name).unwrap_or_default())
                    }
                    name => Ty::Named(name.to_string(), args),
                }
            }
            TypeKind::Function(f) => Ty::Fun(Box::new(FunTy {
                params: f.params.iter().map(|p| self.ty_of(&p.ty)).collect(),
                ret: self.ty_of(&f.result),
                receiver: f.receiver.as_ref().and_then(|r| receiver_name(r)),
                suspend: f.suspend,
                shared: false,
            })),
            TypeKind::Error => Ty::Unknown,
        };
        if t.nullable { base.nullable() } else { base }
    }

    /// A Kotlin type as Rust's, at `pos`.
    pub(crate) fn rust_type(&mut self, t: &Type, pos: TypePos) -> TokenStream {
        // (a type alias is the type it names: written as that, in the position's form)
        if let Some(expanded) = self.expand_alias(t) {
            let mut expanded = expanded;
            expanded.loc = t.loc;
            return self.rust_type(&expanded, pos);
        }
        let span = self.span(t.loc);
        if let TypeKind::Function(f) = &t.kind {
            let mut inputs = Vec::new();
            if let Some(r) = &f.receiver
                && receiver_name(r).is_some()
            {
                let rt = self.rust_type(r, TypePos::Value);
                inputs.push(quote_spanned!(span=> &mut #rt));
            }
            for p in &f.params {
                inputs.push(self.rust_type(&p.ty, TypePos::Value));
            }
            let unit = self.ty_of(&f.result) == Ty::Unit;
            // ((stage 14 WPB) `suspend (A) -> R`: a function giving a future — a parameter's `impl AsyncFn(A) -> R`,
            // a value's shared closure giving a boxed one; `((A) -> R)?`: `Option` of the shared closure)
            if f.suspend {
                let r = if unit { quote_spanned!(span=> ()) } else { self.rust_type(&f.result, TypePos::Value) };
                let shared = quote_spanned!(span=> ::std::rc::Rc<dyn ::core::ops::Fn(#(#inputs),*)
                    -> ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<Output = #r>>>>);
                return match (t.nullable, pos) {
                    (true, _) => quote_spanned!(span=> ::core::option::Option<#shared>),
                    (false, TypePos::Param | TypePos::DefaultedParam | TypePos::Return) => {
                        let output = if unit { quote!() } else { quote_spanned!(span=> -> #r) };
                        quote_spanned!(span=> impl ::core::ops::AsyncFn(#(#inputs),*) #output + 'static)
                    }
                    (false, TypePos::Value) => shared,
                };
            }
            let output = if unit {
                quote!()
            } else {
                let r = self.rust_type(&f.result, TypePos::Value);
                quote_spanned!(span=> -> #r)
            };
            if t.nullable {
                return quote_spanned!(span=> ::core::option::Option<::std::rc::Rc<dyn ::core::ops::Fn(#(#inputs),*) #output>>);
            }
            return match pos {
                TypePos::Param | TypePos::DefaultedParam | TypePos::Return => {
                    quote_spanned!(span=> impl ::core::ops::Fn(#(#inputs),*) #output + 'static)
                }
                TypePos::Value => {
                    quote_spanned!(span=> ::std::rc::Rc<dyn ::core::ops::Fn(#(#inputs),*) #output>)
                }
            };
        }
        let base = match &t.kind {
            TypeKind::Named(segs) => self.named_type(segs, t.loc),
            TypeKind::Function(_) => unreachable!(),
            TypeKind::Error => quote_spanned!(span=> _),
        };
        let is_string = self.ty_of(t) == Ty::String;
        match (t.nullable, pos) {
            (true, TypePos::Param) => quote_spanned!(span=> impl ::core::convert::Into<::core::option::Option<#base>>),
            (true, _) => quote_spanned!(span=> ::core::option::Option<#base>),
            (false, TypePos::Param) if is_string => quote_spanned!(span=> impl ::core::convert::Into<#base>),
            _ => base,
        }
    }

    fn named_type(&mut self, segs: &[TypeSegment], loc: Loc) -> TokenStream {
        let span = self.span(loc);
        if segs.len() > 1 && !segs[..segs.len() - 1].iter().all(|s| s.name.name.starts_with(char::is_lowercase)) {
            // (a nested type Rust names otherwise, by its doc line: `Arrangement.Vertical` → `VerticalArrangement`)
            let kotlin = segs.iter().map(|s| s.name.name.as_str()).collect::<Vec<_>>().join(".");
            if let Some(rust) = crate::index::rust_type(&kotlin)
                && segs.iter().all(|s| s.args.is_empty())
            {
                let i = proc_macro2::Ident::new(rust, span);
                return quote_spanned!(span=> #i);
            }
            // (a user's nested type: its Rust name; its arguments the last segment's)
            if let Some(c) = self.user_class(&kotlin).cloned() {
                let last = segs.last().expect("a type has a name");
                let mut args = Vec::new();
                for a in &last.args {
                    match a {
                        TypeArg::Type { ty, .. } => args.push(self.rust_type(ty, TypePos::Value)),
                        TypeArg::Star(l) => args.push(self.star_projection(*l)),
                    }
                }
                return self.user_type_tokens(&c, args, span);
            }
            // (the API's nested types are Rust's types in a module of the outer's name: `PressInteraction.Press` is
            // `PressInteraction::Press`)
            if segs.iter().all(|s| s.args.is_empty() && super::names::valid(&s.name.name)) {
                let path: Vec<proc_macro2::Ident> =
                    segs.iter().map(|s| proc_macro2::Ident::new(&s.name.name, self.span(s.name.loc))).collect();
                return quote_spanned!(span=> #(#path)::*);
            }
            return self.later(loc, "a nested type");
        }
        let last = segs.last().expect("a type has a name");
        let mut args = Vec::new();
        for a in &last.args {
            match a {
                TypeArg::Type { ty, .. } => args.push(self.rust_type(ty, TypePos::Value)),
                TypeArg::Star(l) => args.push(self.star_projection(*l)),
            }
        }
        let a = |k: usize| args.get(k).cloned().unwrap_or_else(|| quote_spanned!(span=> _));
        let kt = self.kt(span);
        let name = last.name.name.as_str();
        match name {
            "Int" => quote_spanned!(span=> i32),
            "Long" => quote_spanned!(span=> i64),
            "Short" => quote_spanned!(span=> i16),
            "Byte" => quote_spanned!(span=> i8),
            "UInt" => quote_spanned!(span=> u32),
            "ULong" => quote_spanned!(span=> u64),
            "UShort" => quote_spanned!(span=> u16),
            "UByte" => quote_spanned!(span=> u8),
            "Float" => quote_spanned!(span=> f32),
            "Double" => quote_spanned!(span=> f64),
            "Boolean" => quote_spanned!(span=> bool),
            "Char" => quote_spanned!(span=> char),
            "Unit" => quote_spanned!(span=> ()),
            // (Kotlin's `CharSequence` too: a text, which KompozR's APIs take and give as `String`s)
            "String" | "CharSequence" => quote_spanned!(span=> ::std::string::String),
            "Nothing" => quote_spanned!(span=> !),
            // (Kotlin's `Any`: a value whose class can be asked, shared — stage 14 WPA)
            "Any" => quote_spanned!(span=> ::std::rc::Rc<dyn #kt::KtObject>),
            "List" | "MutableList" | "ArrayList" | "Collection" | "Iterable" | "Array" => {
                let t = a(0);
                quote_spanned!(span=> ::std::vec::Vec<#t>)
            }
            "IntArray" => quote_spanned!(span=> ::std::vec::Vec<i32>),
            "LongArray" => quote_spanned!(span=> ::std::vec::Vec<i64>),
            "FloatArray" => quote_spanned!(span=> ::std::vec::Vec<f32>),
            "DoubleArray" => quote_spanned!(span=> ::std::vec::Vec<f64>),
            "BooleanArray" => quote_spanned!(span=> ::std::vec::Vec<bool>),
            "CharArray" => quote_spanned!(span=> ::std::vec::Vec<char>),
            "Set" | "MutableSet" | "HashSet" => {
                let t = a(0);
                quote_spanned!(span=> #kt::KtSet<#t>)
            }
            "Map" | "MutableMap" | "HashMap" => {
                let (k, v) = (a(0), a(1));
                quote_spanned!(span=> #kt::KtMap<#k, #v>)
            }
            "Pair" => {
                let (x, y) = (a(0), a(1));
                quote_spanned!(span=> (#x, #y))
            }
            "Triple" => {
                let (x, y, z) = (a(0), a(1), a(2));
                quote_spanned!(span=> (#x, #y, #z))
            }
            "IntRange" => quote_spanned!(span=> #kt::KtProgression<i32>),
            "ClosedFloatingPointRange" | "ClosedRange" => {
                let t = a(0);
                quote_spanned!(span=> ::core::ops::RangeInclusive<#t>)
            }
            // (stage 14 WPB) Kotlin's exception classes: kt's Throwable (the class is the object's)
            _ if self.exception_type(name) => quote_spanned!(span=> #kt::Throwable),
            // (an interface the API has as a trait: its object shared, as Kotlin's references are — `shape: Shape` is
            // an `Arc<dyn Shape>`, as the API's own shapes are; `AnimationSpec<Float>` an `Arc<dyn AnimationSpec<f32>>`)
            _ if super::resolve::trait_object(name).is_some() => {
                let i = self.name_ident(name, last.name.loc);
                let g = if args.is_empty() { quote!() } else { quote_spanned!(span=> <#(#args),*>) };
                match super::resolve::trait_object(name) {
                    Some("Arc") => quote_spanned!(span=> ::std::sync::Arc<dyn #i #g>),
                    _ => quote_spanned!(span=> ::std::rc::Rc<dyn #i #g>),
                }
            }
            // (a user's type by its Rust name: a nested one named alone inside its class)
            _ if self.user_class(name).is_some() => {
                let c = self.user_class(name).cloned().expect("a user's type");
                self.user_type_tokens(&c, args, span)
            }
            _ => {
                let i = self.name_ident(name, last.name.loc);
                if args.is_empty() { quote_spanned!(span=> #i) } else { quote_spanned!(span=> #i<#(#args),*>) }
            }
        }
    }

    /// A user's type as Rust's, its arguments given: an interface's or an open class's values its trait's shared
    /// objects (`Rc<dyn Titled>`, `Rc<dyn PlateOpen>`), the others their type (`Crate<i32>`).
    fn user_type_tokens(&self, c: &super::class::UserClass, args: Vec<TokenStream>, span: Span) -> TokenStream {
        let i = proc_macro2::Ident::new(&c.value_rust(), span);
        let g = if args.is_empty() { quote!() } else { quote_spanned!(span=> <#(#args),*>) };
        if c.is_dyn() { quote_spanned!(span=> ::std::rc::Rc<dyn #i #g>) } else { quote_spanned!(span=> #i #g) }
    }

    /// A type the generator inferred, as Rust's (for a function's result it wasn't written for).
    pub(crate) fn rust_of_ty(&self, ty: &Ty, span: Span) -> TokenStream {
        let kt = self.kt(span);
        match ty {
            Ty::Unit => quote_spanned!(span=> ()),
            Ty::Int => quote_spanned!(span=> i32),
            Ty::Long => quote_spanned!(span=> i64),
            Ty::Float => quote_spanned!(span=> f32),
            Ty::Double => quote_spanned!(span=> f64),
            Ty::Bool => quote_spanned!(span=> bool),
            Ty::Char => quote_spanned!(span=> char),
            Ty::String => quote_spanned!(span=> ::std::string::String),
            Ty::Nullable(t) => {
                let t = self.rust_of_ty(t, span);
                quote_spanned!(span=> ::core::option::Option<#t>)
            }
            Ty::List(t) => {
                let t = self.rust_of_ty(t, span);
                quote_spanned!(span=> ::std::vec::Vec<#t>)
            }
            Ty::Range(t) => {
                let t = self.rust_of_ty(t, span);
                quote_spanned!(span=> #kt::KtProgression<#t>)
            }
            Ty::ClosedRange(t) => {
                let t = self.rust_of_ty(t, span);
                quote_spanned!(span=> ::core::ops::RangeInclusive<#t>)
            }
            // (a shared or boxed trait object: `Rc<dyn Style>`, the pointer's path written out)
            Ty::Named(n, args)
                if matches!(n.as_str(), "Rc" | "Arc" | "Box")
                    && matches!(args.as_slice(), [Ty::Named(t, _)] if super::resolve::trait_object(t).is_some()) =>
            {
                let inner = match &args[0] {
                    Ty::Named(t, a) => {
                        let i = proc_macro2::Ident::new(t, span);
                        if a.is_empty() || a.contains(&Ty::Unknown) {
                            quote_spanned!(span=> #i)
                        } else {
                            let a = a.iter().map(|t| self.rust_of_ty(t, span));
                            quote_spanned!(span=> #i<#(#a),*>)
                        }
                    }
                    _ => unreachable!(),
                };
                match n.as_str() {
                    "Rc" => quote_spanned!(span=> ::std::rc::Rc<dyn #inner>),
                    "Arc" => quote_spanned!(span=> ::std::sync::Arc<dyn #inner>),
                    _ => quote_spanned!(span=> ::std::boxed::Box<dyn #inner>),
                }
            }
            // (stage 14 WPB) Kotlin's exceptions: kt's Throwable
            Ty::Named(n, args) if n == "Throwable" && args.is_empty() => quote_spanned!(span=> #kt::Throwable),
            Ty::Named(n, args) => {
                let i = proc_macro2::Ident::new(n, span);
                let t = if args.is_empty() || args.contains(&Ty::Unknown) {
                    quote_spanned!(span=> #i)
                } else {
                    let a = args.iter().map(|t| self.rust_of_ty(t, span));
                    quote_spanned!(span=> #i<#(#a),*>)
                };
                // (a user's interface or open class: its values' shared trait object)
                if self.classes.classes.values().any(|c| c.is_dyn() && c.value_rust() == *n) {
                    quote_spanned!(span=> ::std::rc::Rc<dyn #t>)
                } else {
                    t
                }
            }
            _ => quote_spanned!(span=> _),
        }
    }

    /// What a lambda given to a parameter of function type `f` becomes (`stored`: the function keeps it).
    pub(crate) fn lambda_info(&mut self, f: &FunctionType, stored: bool) -> LInfo {
        let mut inputs = Vec::new();
        let receiver = f.receiver.as_ref().and_then(|r| receiver_name(r));
        if let Some(r) = &receiver {
            inputs.push(format!("&mut {r}"));
        }
        for p in &f.params {
            inputs.push(self.rust_type(&p.ty, TypePos::Value).to_string());
        }
        let output = if self.ty_of(&f.result) == Ty::Unit {
            "()".to_string()
        } else {
            self.rust_type(&f.result, TypePos::Value).to_string()
        };
        LInfo {
            kind: LambdaKind::Fn,
            inputs,
            output,
            receiver,
            receiver_value: None,
            suspend: f.suspend,
            boxed: false,
            stored,
            measure_policy: false,
            typed: false,
            twin: false,
            file_inline: !stored,
            snapshot_block: false,
            crossinline: None,
        }
    }
}

/// A function type's receiver as the Rust side passes it: its name, none for a scope Rust doesn't pass.
fn receiver_name(r: &Type) -> Option<String> {
    let n = r.simple_name()?;
    let n = last_segment(&n).to_string();
    (!IMPLICIT_SCOPES.contains(&n.as_str())).then_some(n)
}

use crate::pos::Loc;
