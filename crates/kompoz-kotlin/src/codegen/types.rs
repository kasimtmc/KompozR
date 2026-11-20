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
        let base = match &t.kind {
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
                    "String" => Ty::String,
                    "Unit" => Ty::Unit,
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
                    name => Ty::Named(name.to_string(), args),
                }
            }
            TypeKind::Function(f) => Ty::Fun(Box::new(FunTy {
                params: f.params.iter().map(|p| self.ty_of(&p.ty)).collect(),
                ret: self.ty_of(&f.result),
                receiver: f.receiver.as_ref().and_then(|r| receiver_name(r)),
                suspend: f.suspend,
            })),
            TypeKind::Error => Ty::Unknown,
        };
        if t.nullable { base.nullable() } else { base }
    }

    /// A Kotlin type as Rust's, at `pos`.
    pub(crate) fn rust_type(&mut self, t: &Type, pos: TypePos) -> TokenStream {
        let span = self.span(t.loc);
        if let TypeKind::Function(f) = &t.kind {
            if t.nullable {
                return self.later(t.loc, "a nullable function type");
            }
            if f.suspend {
                return self.later(t.loc, "a suspending function type");
            }
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
            let output = if self.ty_of(&f.result) == Ty::Unit {
                quote!()
            } else {
                let r = self.rust_type(&f.result, TypePos::Value);
                quote_spanned!(span=> -> #r)
            };
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
            return self.later(loc, "a nested type");
        }
        let last = segs.last().expect("a type has a name");
        let mut args = Vec::new();
        for a in &last.args {
            match a {
                TypeArg::Type { ty, .. } => args.push(self.rust_type(ty, TypePos::Value)),
                TypeArg::Star(l) => args.push(self.later(*l, "a star projection")),
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
            "String" => quote_spanned!(span=> ::std::string::String),
            "Nothing" => quote_spanned!(span=> !),
            "Any" => self.later(loc, "the type `Any`"),
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
            _ => {
                let i = self.name_ident(name, last.name.loc);
                if args.is_empty() { quote_spanned!(span=> #i) } else { quote_spanned!(span=> #i<#(#args),*>) }
            }
        }
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
            Ty::Named(n, args) => {
                let i = proc_macro2::Ident::new(n, span);
                if args.is_empty() || args.contains(&Ty::Unknown) {
                    quote_spanned!(span=> #i)
                } else {
                    let a = args.iter().map(|t| self.rust_of_ty(t, span));
                    quote_spanned!(span=> #i<#(#a),*>)
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
