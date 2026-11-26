//! Kotlin's properties beyond a plain value (stage 14 WPB): by a delegate, with accessors (`get()`, `set(value)`,
//! `field`), `lateinit`, at the top level as a `var`, and extension properties.
//!
//! | Kotlin | Rust |
//! |---|---|
//! | `var count by mutableStateOf(0)` (a class's or an object's; `private set`) | a field holding the delegate (`Rc<dyn kt::KtMutableDelegate<Value = i32>>`), made as the object is; `count()` reads it (`getValue`), `set_count(v)` sets it (`setValue`), the setter as visible as `set` says |
//! | `val doubled by derivedStateOf { count * 2 }`, `val x by lazy { … }` | the same, read only (`Rc<dyn kt::KtDelegate<Value = T>>`) |
//! | `var step = 1; set(value) { field = … }`, `val n get() = …` | the backing field a cell (`kt::KtVar`), the accessors methods whose `field` is it |
//! | `lateinit var user: String` | `kt::KtLateinit<String>`: read before it is set, an `UninitializedPropertyAccessException` |
//! | `var visits = 0` (top level) | `thread_local! { static visits: kt::Ref<i32> }`, read and set through it; `by lazy`/`by mutableStateOf` the same of the delegate |
//! | `val String.shout: String get() = …`, `var Meter.doubled: Int get() … set(v) …` | `pub trait StringShoutExt { fn shout(&self) -> String; }` implemented for the receiver's type (`set_doubled(&self, value)` too; the body has a clone of the receiver); `"selam".shout` → `String::from("selam").shout()` |
//! | `val <T> List<T>.second: T get() = this[1]` | `pub trait ListSecondExt<T> { … } impl<T: Clone + 'static> ListSecondExt<T> for Vec<T>` |
//!
//! A delegate's lambda (`derivedStateOf { count * 2 }`) reads the object's other members: it holds the object (its
//! `self` cloned: Kotlin's lambda holds `this`).

use super::class::MemberCx;
use super::expr::{Out, OutKind, Want};
use super::names::{ident, pascal, snake};
use super::resolve::*;
use super::types::TypePos;
use super::{Gen, respan};
use crate::ast::*;
use crate::pos::Loc;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};

type CellParts = (TokenStream, TokenStream, TokenStream, Option<TokenStream>);

impl Gen<'_, '_> {
    // ---- A class's (or object's) properties ------------------------------------------------------------------

    /// A member property by a delegate, with accessors, or `lateinit`: its field, its empty field, its accessors, its
    /// initialization (see the module's doc); `place` the object's fields (`self.__kz_this`).
    pub(crate) fn member_special_property(
        &mut self,
        p: &Property,
        cx: &MemberCx,
        place: TokenStream,
    ) -> Option<CellParts> {
        let Binding::Name(n) = &p.binding else { return None };
        let span = self.span(p.loc);
        let kt = self.kt(span);
        let i = self.name_ident(&snake(&n.name), n.loc);
        let setter = format_ident!("set_{}", i.to_string().trim_start_matches("r#"), span = span);
        let vis = self.property_vis(&p.modifiers, p.loc);
        let set_vis = match p.setter.as_ref() {
            Some(s) if s.modifiers.visibility() != Visibility::Public => self.property_vis(&s.modifiers, s.loc),
            _ => vis.clone(),
        };
        let ty = self.special_property_ty(p, cx);
        let t = match &p.ty {
            Some(t) => self.rust_type(t, TypePos::Value),
            None if !ty.is_vague() => self.rust_of_ty(&ty, span),
            None => {
                self.error(n.loc, format!("write `{}`'s type: KompozR can't tell its value's type", n.name));
                return None;
            }
        };
        // `lateinit var x: T`
        if p.modifiers.has(ModifierKind::Lateinit) {
            if !p.is_var() || p.ty.is_none() || !matches!(p.init, PropertyInit::None) {
                self.error(p.loc, "a `lateinit` property is a `var` with its type and no value");
                return None;
            }
            let lit = proc_macro2::Literal::string(&n.name);
            let field = quote_spanned!(span=> #i: #kt::KtLateinit<#t>);
            let empty = quote_spanned!(span=> #i: #kt::KtLateinit::new(#lit));
            let acc = quote_spanned! {span=>
                #vis fn #i(&self) -> #t { #kt::KtDelegate::get_value(&#place.#i) }
                #set_vis fn #setter(&self, value: #t) { #kt::KtMutableDelegate::set_value(&#place.#i, value) }
            };
            return Some((field, empty, acc, None));
        }
        // `by delegate`
        if let PropertyInit::Delegate(d) = &p.init {
            if p.getter.is_some() || p.setter.as_ref().is_some_and(|s| s.body.is_some()) {
                self.error(p.loc, "a delegated property has no accessors of its own (its delegate's are)");
                return None;
            }
            let var = p.is_var();
            let dyn_ty = if var {
                quote_spanned!(span=> ::std::rc::Rc<dyn #kt::KtMutableDelegate<Value = #t>>)
            } else {
                quote_spanned!(span=> ::std::rc::Rc<dyn #kt::KtDelegate<Value = #t>>)
            };
            let hint = p.ty.as_ref().map(|t| (self.rust_type(t, TypePos::Value), self.ty_of(t)));
            let outer = std::mem::replace(&mut self.state_value, hint);
            let class = self.user_class_rust(&cx.owner).map(|c| c.kotlin.clone());
            let v = self.keyed_delegate_value(d, &n.name, class.as_deref());
            self.state_value = outer;
            let field = quote_spanned!(span=> #i: #kt::KtVal<#dyn_ty>);
            let empty = quote_spanned!(span=> #i: #kt::KtVal::new());
            let init = quote_spanned!(span=> #place.#i.init(::std::rc::Rc::new(#kt::delegate_of::<#t, _>(#v))););
            let get = quote_spanned!(span=> #vis fn #i(&self) -> #t { #place.#i.with(|__kz_d| #kt::KtDelegate::get_value(__kz_d)) });
            let acc = if var {
                quote_spanned! {span=>
                    #get
                    #set_vis fn #setter(&self, value: #t) {
                        #place.#i.with(|__kz_d| #kt::KtMutableDelegate::set_value(__kz_d, value))
                    }
                }
            } else {
                get
            };
            return Some((field, empty, acc, Some(init)));
        }
        // Accessors: a backing field (`field`) when the property has a value or an accessor without a body.
        let backed = !matches!(p.init, PropertyInit::None);
        let (field, empty, init) = if backed {
            let PropertyInit::Value(e) = &p.init else { unreachable!() };
            let v = if ty.is_nullable() {
                self.value_want(e, Want { some: true, ..Want::default() })
            } else {
                self.value(e)
            };
            (
                quote_spanned!(span=> #i: #kt::KtVar<#t>),
                quote_spanned!(span=> #i: #kt::KtVar::new()),
                Some(quote_spanned!(span=> #place.#i.set(#v);)),
            )
        } else {
            (quote!(), quote!(), None)
        };
        // (the accessors written as a data class's are: `class::accessors_with_field`, their one writer)
        let saved = (std::mem::take(&mut self.scopes), std::mem::take(&mut self.receivers));
        let acc = self.accessors_with_field(p, &i, &t, &ty, &vis, cx, backed.then_some(&place));
        (self.scopes, self.receivers) = saved;
        Some((field, empty, acc, init))
    }

    /// A delegate keyed by the property's name where Kotlin's is (`val user by handle.saved { … }`, `by
    /// handle.saveable { … }`, `by saved { … }`: their key the property's, which a Kotlin `KProperty` gives the
    /// delegate and Rust can't): the call given `key = "name"` (`qualified`: the class's name before it, as Kotlin's
    /// qualified name of a member has).
    pub(crate) fn keyed_delegate(d: &Expr, name: &str, qualified: Option<&str>) -> Expr {
        let ExprKind::Call { callee, type_args, args, trailing_lambda } = &d.kind else { return d.clone() };
        let named = match &callee.kind {
            ExprKind::Member { name, .. } | ExprKind::Name(name) => name.name.as_str(),
            _ => return d.clone(),
        };
        if !matches!(named, "saved" | "saveable") {
            return d.clone();
        }
        let keyed = args.iter().any(|a| a.name.as_ref().is_some_and(|n| n.name == "key"))
            || args.first().is_some_and(|a| a.name.is_none() && matches!(a.value.kind, ExprKind::String(_)));
        if keyed {
            return d.clone();
        }
        let key = match qualified {
            Some(q) => format!("{q}.{name}"),
            None => name.to_string(),
        };
        let lit = Expr {
            kind: ExprKind::String(StringLit { raw: false, parts: vec![StringPart::Text { text: key, loc: d.loc }] }),
            loc: d.loc,
        };
        let mut args = args.clone();
        args.insert(
            0,
            Arg {
                name: Some(Ident { name: "key".into(), backticked: false, loc: d.loc }),
                spread: false,
                value: lit,
                loc: d.loc,
            },
        );
        Expr {
            kind: ExprKind::Call {
                callee: callee.clone(),
                type_args: type_args.clone(),
                args,
                trailing_lambda: trailing_lambda.clone(),
            },
            loc: d.loc,
        }
    }

    /// A delegate keyed by the property's name ([`Gen::keyed_delegate`]) as the value the property holds: where it is
    /// Kotlin's `handle.saveable(saver) { … }` of a value that isn't a state — its `PropertyDelegateProvider` of a
    /// `ReadOnlyProperty` giving the value —, that value as a delegate (`kt::KtValue`); a state is a delegate itself
    /// (`handle.saveable { mutableStateOf(true) }`, Kotlin's `ReadWriteProperty` of the state).
    pub(crate) fn keyed_delegate_value(&mut self, d: &Expr, name: &str, qualified: Option<&str>) -> TokenStream {
        let keyed = Self::keyed_delegate(d, name, qualified);
        let provided = keyed != *d
            && matches!(&d.kind, ExprKind::Call { callee, .. }
                if matches!(&callee.kind, ExprKind::Member { name, .. } if name.name == "saveable"));
        let state = provided && is_state_ty(&self.peek_ty(&keyed));
        // (the provider's value of the property's type: what binds the call's `T`)
        let declared = self.state_value.as_ref().map(|(_, t)| t.clone()).filter(|_| provided && !state);
        let v = match declared {
            Some(t) => self.expecting(&keyed, &t, |g| g.value(&keyed)),
            None => self.value(&keyed),
        };
        if provided && !state {
            let span = self.span(d.loc);
            let kt = self.kt(span);
            quote_spanned!(span=> #kt::KtValue(#v))
        } else {
            v
        }
    }

    /// A member property's type: written, its delegate's, its getter's or its value's.
    fn special_property_ty(&mut self, p: &Property, cx: &MemberCx) -> Ty {
        if let Some(t) = &p.ty {
            return self.ty_of(t);
        }
        let e = match (&p.init, &p.getter) {
            (_, Some(Accessor { body: Some(FunctionBody::Expr(e)), .. })) => e.clone(),
            (PropertyInit::Value(e), _) | (PropertyInit::Delegate(e), _) => e.clone(),
            _ => return Ty::Unknown,
        };
        let saved = (std::mem::take(&mut self.scopes), std::mem::take(&mut self.receivers));
        self.enter_member(cx, "", true);
        let t = if p.is_delegated() { self.delegate_ty(&e) } else { self.peek_ty(&e) };
        (self.scopes, self.receivers) = saved;
        t
    }

    /// A property's visibility as a member's (`private` the class's: Rust's module's).
    fn property_vis(&mut self, m: &Modifiers, loc: Loc) -> TokenStream {
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

    // ---- Top-level properties --------------------------------------------------------------------------------

    /// A top-level `var`, or a property by a delegate: a thread-local delegate (a `var`'s `kt::Ref`), made at its
    /// first use as Kotlin's top-level property is made when its file is first used.
    pub(crate) fn top_level_shared(&mut self, p: &Property) -> TokenStream {
        let span = self.span(p.loc);
        let Binding::Name(n) = &p.binding else { return quote!() };
        self.fun = super::FnState::default();
        self.scopes = vec![std::collections::HashMap::new()];
        self.receivers.clear();
        let Some(val) = self.file.vals.get(&n.name).cloned() else {
            self.error(
                n.loc,
                format!("write `{}`'s type: KompozR can't tell its value's type (`var {}: Type = …`)", n.name, n.name),
            );
            return quote!();
        };
        if p.modifiers.has(ModifierKind::Lateinit) {
            self.error(p.loc, "a top-level `lateinit var` isn't one KompozR translates: give it a value");
            return quote!();
        }
        let kt = self.kt(span);
        let rtp = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let vis = self.visibility(&p.modifiers, p.loc);
        let name = self.name_ident(&val.rust, n.loc);
        let t = match &p.ty {
            Some(t) => self.rust_type(t, TypePos::Value),
            None => self.rust_of_ty(&val.ty, self.span(n.loc)),
        };
        let mutable = p.is_var();
        let (held, made) = match &p.init {
            PropertyInit::Delegate(d) => {
                let hint = p.ty.as_ref().map(|t| (self.rust_type(t, TypePos::Value), self.ty_of(t)));
                let outer = std::mem::replace(&mut self.state_value, hint);
                let v = self.keyed_delegate_value(d, &n.name, None);
                self.state_value = outer;
                let held = if mutable {
                    quote_spanned!(span=> ::std::rc::Rc<dyn #kt::KtMutableDelegate<Value = #t>>)
                } else {
                    quote_spanned!(span=> ::std::rc::Rc<dyn #kt::KtDelegate<Value = #t>>)
                };
                (held, quote_spanned!(span=> ::std::rc::Rc::new(#kt::delegate_of::<#t, _>(#v))))
            }
            PropertyInit::Value(e) => {
                let v = if val.ty.is_nullable() {
                    self.value_want(e, Want { some: true, ..Want::default() })
                } else {
                    self.value(e)
                };
                (quote_spanned!(span=> #kt::Ref<#t>), quote_spanned!(span=> #kt::Ref::new(#v)))
            }
            PropertyInit::None => {
                self.error(n.loc, format!("`{}` needs a value", n.name));
                return quote!();
            }
        };
        let allow = super::allow_attr(span);
        quote_spanned! {span=>
            ::std::thread_local! {
                #allow
                #vis static #name: #held = {
                    #[allow(unused_imports)]
                    use #kt::ext::*;
                    #rtp::named_args!(#kz_types #made)
                };
            }
        }
    }

    /// A top-level property with accessors (`val now get() = …`): not translated yet beyond a getter of no backing
    /// field; see the module's doc.
    pub(crate) fn top_level_accessors(&mut self, p: &Property) -> TokenStream {
        let span = self.span(p.loc);
        let Binding::Name(n) = &p.binding else { return quote!() };
        let Some(val) = self.file.vals.get(&n.name).cloned() else {
            self.error(n.loc, format!("write `{}`'s type: KompozR can't tell its getter's type", n.name));
            return quote!();
        };
        let kt = self.kt(span);
        let rtp = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let vis = self.visibility(&p.modifiers, p.loc);
        let t = match &p.ty {
            Some(t) => self.rust_type(t, TypePos::Value),
            None => self.rust_of_ty(&val.ty, span),
        };
        let name = self.name_ident(&val.rust, n.loc);
        let setter_name = format_ident!("set_{}", name.to_string().trim_start_matches("r#"), span = span);
        let allow = super::allow_attr(span);
        // Its backing field, when it has a value: a thread-local cell its accessors' `field` is.
        let backing = format_ident!("__kz_field_{}", name.to_string().trim_start_matches("r#"), span = span);
        let mut out = TokenStream::new();
        let backed = match &p.init {
            PropertyInit::Value(e) => {
                self.fun = super::FnState::default();
                self.scopes = vec![std::collections::HashMap::new()];
                self.receivers.clear();
                let v = if val.ty.is_nullable() {
                    self.value_want(e, Want { some: true, ..Want::default() })
                } else {
                    self.value(e)
                };
                out.extend(quote_spanned! {span=>
                    ::std::thread_local! {
                        #allow
                        #[doc(hidden)]
                        static #backing: #kt::Ref<#t> = {
                            #[allow(unused_imports)]
                            use #kt::ext::*;
                            #rtp::named_args!(#kz_types { #kt::Ref::new(#v) })
                        };
                    }
                });
                true
            }
            _ => false,
        };
        let field_ident = Ident { name: "field".into(), backticked: false, loc: p.loc };
        let accessor = |me: &mut Self, param: Option<(Ident, Ty)>, body: &FunctionBody| -> TokenStream {
            me.fun = super::FnState {
                kompozable: false,
                nullable_result: param.is_none() && val.ty.is_nullable(),
                shared_result: None,
                result: param.is_none().then(|| val.ty.clone()),
            };
            me.scopes = vec![std::collections::HashMap::new()];
            me.receivers.clear();
            me.frames.clear();
            me.loops.clear();
            me.lambdas.clear();
            me.in_async = false;
            if backed {
                let kind = if param.is_some() { LocalKind::Var } else { LocalKind::Val };
                me.declare_at(&field_ident, "field".into(), kind, val.ty.clone());
            }
            if let Some((p, ty)) = &param {
                me.declare_at(p, snake(&p.name), LocalKind::Param, ty.clone());
            }
            match (body, param.is_none()) {
                (FunctionBody::Block(b), get) => me.fn_block(b, !get),
                (FunctionBody::Expr(e), true) => {
                    me.value_want(e, Want { some: val.ty.is_nullable(), result: true, ..Want::default() })
                }
                (FunctionBody::Expr(e), false) => {
                    let s = me.stmt_expr(e);
                    quote!(#s;)
                }
            }
        };
        let read = if backed {
            quote_spanned!(span=> #[allow(unused_mut, unused_variables, unused_assignments)] let mut field: #t = #backing.with(|c| c.get());)
        } else {
            quote!()
        };
        let get = match (p.getter.as_ref().and_then(|g| g.body.clone()), backed) {
            (Some(body), _) => {
                let b = accessor(self, None, &body);
                quote_spanned!(span=> #read #rtp::named_args!(#kz_types { #b }))
            }
            (None, true) => quote_spanned!(span=> #backing.with(|c| c.get())),
            (None, false) => {
                self.error(n.loc, format!("`{}` needs a value or a getter", n.name));
                return quote!();
            }
        };
        out.extend(quote_spanned! {span=>
            #allow
            #vis fn #name() -> #t {
                #[allow(unused_imports)]
                use #kt::ext::*;
                #get
            }
        });
        if p.is_var() {
            let set_vis = match p.setter.as_ref() {
                Some(s) if s.modifiers.visibility() != Visibility::Public => self.visibility(&s.modifiers, s.loc),
                _ => vis.clone(),
            };
            let set = match (p.setter.as_ref().and_then(|s| s.body.as_ref().map(|b| (s, b.clone()))), backed) {
                (Some((s, body)), _) => {
                    let pname = s.param.as_ref().map(|p| p.name.clone()).unwrap_or(Ident {
                        name: "value".into(),
                        backticked: false,
                        loc: s.loc,
                    });
                    let pi = self.name_ident(&snake(&pname.name), pname.loc);
                    let b = accessor(self, Some((pname, val.ty.clone())), &body);
                    let write = if backed { quote_spanned!(span=> #backing.with(|c| c.set(field));) } else { quote!() };
                    quote_spanned!(span=> fn #setter_name(#pi: #t) {
                        #[allow(unused_imports)]
                        use #kt::ext::*;
                        #read
                        #rtp::named_args!(#kz_types { #b });
                        #write
                    })
                }
                (None, true) => quote_spanned!(span=> fn #setter_name(value: #t) { #backing.with(|c| c.set(value)) }),
                (None, false) => {
                    self.error(n.loc, format!("`{}` is a `var` without a value: it needs a setter", n.name));
                    return quote!();
                }
            };
            out.extend(quote_spanned!(span=> #allow #set_vis #set));
        }
        out
    }

    /// The thread-local delegate of a top-level `var` (or delegated property) `kotlin`, if it is one: its Rust name
    /// and whether it can be set.
    pub(crate) fn file_shared(&self, kotlin: &str) -> Option<(String, bool, Ty)> {
        let v = self.file.vals.get(kotlin)?;
        match v.form {
            ValForm::Shared { mutable } => Some((v.rust.clone(), mutable, v.ty.clone())),
            _ => None,
        }
    }

    /// `name = value` (`op=`) of a top-level `var`: through its delegate.
    pub(crate) fn file_var_assign(&mut self, n: &Ident, op: AssignOp, value: &Expr, loc: Loc) -> Option<TokenStream> {
        // (a property with a setter: `set_name(v)`)
        if let Some(v) = self.file.vals.get(&n.name).cloned()
            && let ValForm::Accessors { setter } = v.form
        {
            let span = self.span(loc);
            if !setter {
                self.error(n.loc, format!("`{}` is a `val`: it can't be assigned", n.name));
                return Some(quote!());
            }
            let s = format_ident!("set_{}", v.rust, span = self.span(n.loc));
            let target = Expr { kind: ExprKind::Name(n.clone()), loc: n.loc };
            let new = match super::operators::compound(op) {
                Some(bop) => Expr {
                    kind: ExprKind::Binary { op: bop, lhs: Box::new(target), rhs: Box::new(value.clone()) },
                    loc,
                },
                None => value.clone(),
            };
            let x = self.value(&new);
            return Some(quote_spanned!(span=> #s(#x);));
        }
        let (rust, mutable, ty) = self.file_shared(&n.name)?;
        let span = self.span(loc);
        if !mutable {
            self.error(n.loc, format!("`{}` is a `val`: it can't be assigned", n.name));
            return Some(quote!());
        }
        let kt = self.kt(span);
        let i = ident(&rust, self.span(n.loc));
        let read = quote_spanned!(span=> #kt::KtDelegate::get_value(__kz_d));
        let v = if op == AssignOp::Assign && ty.is_nullable() {
            self.value_want(value, Want { some: true, ..Want::default() })
        } else if ty.is_float() && matches!(value.kind, ExprKind::Number(_)) {
            self.number_value(value, true)
        } else {
            self.value(value)
        };
        let v = match op {
            AssignOp::Assign => v,
            AssignOp::Add if ty.is_string() => {
                let piece = self.expr(value).ts;
                quote_spanned!(span=> #kt::template!(#read, #piece))
            }
            AssignOp::Add => quote_spanned!(span=> #read + #v),
            AssignOp::Sub => quote_spanned!(span=> #read - #v),
            AssignOp::Mul => quote_spanned!(span=> #read * #v),
            AssignOp::Div => quote_spanned!(span=> #read / #v),
            AssignOp::Rem => quote_spanned!(span=> #read % #v),
        };
        // (the value first, then set: it may read the property itself)
        Some(
            quote_spanned!(span=> #i.with(|__kz_d| { let __kz_v = #v; #kt::KtMutableDelegate::set_value(__kz_d, __kz_v) });),
        )
    }

    /// `name++` (`pre`: `++name`; `inc` else `--`) of a top-level `var`: its value (none: a statement).
    pub(crate) fn file_var_step(&mut self, n: &Ident, inc: bool, pre: Option<bool>, loc: Loc) -> Option<Out> {
        let (rust, mutable, ty) = self.file_shared(&n.name)?;
        let span = self.span(loc);
        if !mutable {
            self.error(n.loc, format!("`{}` is a `val`: it can't be assigned", n.name));
            return Some(Out::plain(quote!(), Ty::Unknown));
        }
        let kt = self.kt(span);
        let i = ident(&rust, self.span(n.loc));
        let f = match (inc, pre.unwrap_or(false)) {
            (true, true) => quote_spanned!(span=> pre_inc),
            (true, false) => quote_spanned!(span=> post_inc),
            (false, true) => quote_spanned!(span=> pre_dec),
            (false, false) => quote_spanned!(span=> post_dec),
        };
        let ts = quote_spanned!(span=> #i.with(|__kz_d| #kt::#f(__kz_d)));
        Some(Out::plain(if pre.is_none() { quote_spanned!(span=> { #ts; }) } else { ts }, ty))
    }

    /// A number literal as a value of Kotlin's float type (`x += 1` of a `Float`).
    fn number_value(&mut self, e: &Expr, float: bool) -> TokenStream {
        match &e.kind {
            ExprKind::Number(n) => self.number(n, e.loc, float).ts,
            _ => self.value(e),
        }
    }

    // ---- Extension properties --------------------------------------------------------------------------------

    /// Notes an extension property (its getter as an extension function of the file, of no parameters), for its
    /// reads before and after it.
    pub(crate) fn collect_extension_property(&mut self, p: &Property) {
        let Binding::Name(n) = &p.binding else { return };
        let Some(r) = &p.receiver else { return };
        let Some(recv) = r.simple_name() else { return };
        let owner = owner_name(super::last_segment(&recv));
        let ret = match (&p.ty, &p.getter) {
            (Some(t), _) => self.ty_of(t),
            (None, Some(Accessor { body: Some(FunctionBody::Expr(e)), .. })) => self.quiet(|g| {
                let saved = std::mem::take(&mut g.receivers);
                g.push_extension_receiver(r, &owner);
                let t = g.peek_ty(e);
                g.receivers = saved;
                t
            }),
            _ => Ty::Unknown,
        };
        let f = FileFn {
            rust: snake(&n.name),
            params: Vec::new(),
            kompozable: false,
            suspend: false,
            ret,
            receiver: Some(owner.clone()),
            builder: false,
        };
        self.file.functions.entry(n.name.clone()).or_default().push(f);
        self.control.ext_props.insert((n.name.clone(), owner), p.is_var());
    }

    /// The receiver `this` of an extension (`self`, of the type it extends).
    fn push_extension_receiver(&mut self, r: &Type, owner: &str) {
        self.receivers.push(ImplicitReceiver {
            rust: "__kz_recv".into(),
            ty: owner.to_string(),
            value: self.ty_of(r),
            labels: Vec::new(),
            depth: 0,
            by_ref: false,
            by_mut: false,
            owned_used: Default::default(),
        });
    }

    /// The Kotlin owner name of a value's type an extension property may be of (a list's `List`).
    fn extension_owners(ty: &Ty) -> Vec<String> {
        match ty.non_null() {
            Ty::List(_) => vec!["List".into(), "MutableList".into(), "Collection".into(), "Iterable".into()],
            Ty::Set(_) => vec!["Set".into(), "MutableSet".into(), "Collection".into(), "Iterable".into()],
            Ty::Map(..) => vec!["Map".into(), "MutableMap".into()],
            t => t.owner().map(|o| vec![o.to_string()]).unwrap_or_default(),
        }
    }

    /// The type of the file's extension property `name` of a value of type `recv`, if it has one.
    pub(crate) fn file_extension_property_ty(&self, recv: &Ty, name: &str) -> Option<Ty> {
        let owner = Self::extension_owners(recv)
            .into_iter()
            .find(|o| self.control.ext_props.contains_key(&(name.to_string(), o.clone())))?;
        let f = self.file.extension(name, Some(&owner))?;
        // (a generic receiver's element: `List<T>.second` of a `List<Int>` an `Int`)
        Some(match (&f.ret, recv) {
            (Ty::Named(t, a), Ty::List(e)) if a.is_empty() && t.len() == 1 => (**e).clone(),
            (t, _) => t.clone(),
        })
    }

    /// `recv.name` of the file's extension property: its getter (`Err(recv)` when it isn't one).
    pub(crate) fn file_extension_property_read(&mut self, recv: Out, name: &Ident, loc: Loc) -> Result<Out, Out> {
        let Some(ty) = self.file_extension_property_ty(&recv.ty, &name.name) else { return Err(recv) };
        let span = self.span(loc);
        let m = ident(&snake(&name.name), self.span(name.loc));
        let r = match recv.kind {
            OutKind::StrLit => {
                let ts = recv.ts;
                quote_spanned!(span=> ::std::string::String::from(#ts))
            }
            _ => self.finish(recv, Want::default(), loc),
        };
        Ok(Out::plain(quote_spanned!(span=> #r.#m()), ty))
    }

    /// An extension property's declaration: a trait of its getter (and setter) implemented for the receiver's type.
    pub(crate) fn extension_property_decl(&mut self, p: &Property) -> TokenStream {
        let span = self.span(p.loc);
        let Binding::Name(n) = &p.binding else { return quote!() };
        let Some(r) = p.receiver.clone() else { return quote!() };
        let Some(recv) = r.simple_name() else {
            return self.later(r.loc, "an extension property of a function type");
        };
        if !matches!(p.init, PropertyInit::None) {
            self.error(p.loc, "an extension property has no value of its own (no backing field): give it a getter");
            return quote!();
        }
        let owner = owner_name(super::last_segment(&recv));
        let kt = self.kt(span);
        let rtp = respan(self.rt().clone(), span);
        let kz_types = self.types_arg(span);
        let vis = match p.modifiers.visibility() {
            Visibility::Private => quote_spanned!(span=> pub(self)),
            _ => self.visibility(&p.modifiers, p.loc),
        };
        let trait_name = format_ident!("{}{}Ext", pascal(&owner), pascal(&n.name), span = span);
        let getter_name = self.name_ident(&snake(&n.name), n.loc);
        let setter_name = format_ident!("set_{}", getter_name.to_string().trim_start_matches("r#"), span = span);
        // Its type parameters (`<T>`), each `Clone + 'static` (a value of it given by value, Kotlin's reference).
        let tps: Vec<proc_macro2::Ident> =
            p.type_params.iter().map(|t| self.name_ident(&t.name.name, t.name.loc)).collect();
        let generics = if tps.is_empty() { quote!() } else { quote_spanned!(span=> <#(#tps),*>) };
        let bounds =
            if tps.is_empty() { quote!() } else { quote_spanned!(span=> <#(#tps: ::core::clone::Clone + 'static),*>) };
        let self_ty = self.rust_type(&r, TypePos::Value);
        // Its type: written, or its getter's.
        let ty = self.file.extension(&n.name, Some(&owner)).map(|f| f.ret.clone()).unwrap_or(Ty::Unknown);
        let t = match &p.ty {
            Some(t) => self.rust_type(t, TypePos::Value),
            None if !ty.is_vague() => self.rust_of_ty(&ty, span),
            None => {
                self.error(n.loc, format!("write `{}`'s type: KompozR can't tell its getter's type", n.name));
                return quote!();
            }
        };
        let accessor = |me: &mut Self, params: &[(Ident, Ty, TokenStream)], body: &FunctionBody, ret: bool| {
            me.fun = super::FnState {
                kompozable: false,
                nullable_result: ty.is_nullable() && ret,
                shared_result: None,
                result: ret.then(|| ty.clone()),
            };
            me.scopes = vec![std::collections::HashMap::new()];
            me.receivers.clear();
            me.frames.clear();
            me.loops.clear();
            me.lambdas.clear();
            me.in_async = false;
            me.push_extension_receiver(&r, &owner);
            let value_ty = me.ty_of(&r);
            me.declare("this", "__kz_recv".into(), LocalKind::Param, value_ty);
            let mut ps = Vec::new();
            for (p, pty, prt) in params {
                let pi = me.name_ident(&snake(&p.name), p.loc);
                me.declare_at(p, snake(&p.name), LocalKind::Param, pty.clone());
                ps.push(quote_spanned!(span=> #pi: #prt));
            }
            let b = match body {
                FunctionBody::Block(b) => me.fn_block(b, !ret),
                FunctionBody::Expr(e) if ret => {
                    me.value_want(e, Want { some: ty.is_nullable(), result: true, ..Want::default() })
                }
                FunctionBody::Expr(e) => {
                    let s = me.stmt_expr(e);
                    quote!(#s;)
                }
            };
            (ps, b)
        };
        let Some(getter) = p.getter.as_ref().and_then(|g| g.body.as_ref()) else {
            self.error(n.loc, format!("the extension property `{}` needs a getter", n.name));
            return quote!();
        };
        let (_, get_body) = accessor(self, &[], getter, true);
        let mut decls = vec![quote_spanned!(span=> fn #getter_name(&self) -> #t;)];
        let mut impls = vec![quote_spanned! {span=>
            fn #getter_name(&self) -> #t {
                #[allow(unused_variables)]
                let __kz_recv = ::core::clone::Clone::clone(self);
                #[allow(unused_imports)]
                use #kt::ext::*;
                #rtp::named_args!(#kz_types { #get_body })
            }
        }];
        if p.is_var() {
            let Some((s, body)) = p.setter.as_ref().and_then(|s| s.body.as_ref().map(|b| (s, b))) else {
                self.error(n.loc, format!("the extension property `{}` is a `var`: it needs a setter", n.name));
                return quote!();
            };
            let pname = s.param.as_ref().map(|p| p.name.clone()).unwrap_or(Ident {
                name: "value".into(),
                backticked: false,
                loc: s.loc,
            });
            let (ps, set_body) = accessor(self, &[(pname, ty.clone(), t.clone())], body, false);
            decls.push(quote_spanned!(span=> fn #setter_name(&self, #(#ps),*);));
            impls.push(quote_spanned! {span=>
                fn #setter_name(&self, #(#ps),*) {
                    #[allow(unused_variables)]
                    let __kz_recv = ::core::clone::Clone::clone(self);
                    #[allow(unused_imports)]
                    use #kt::ext::*;
                    #rtp::named_args!(#kz_types { #set_body })
                }
            });
        }
        let allow = super::allow_attr(span);
        quote_spanned! {span=>
            #allow
            #[allow(non_camel_case_types)]
            #vis trait #trait_name #generics {
                #(#decls)*
            }
            #allow
            impl #bounds #trait_name #generics for #self_ty {
                #(#impls)*
            }
        }
    }
}

/// Whether a value of `t` is a state (a `by` delegate itself).
fn is_state_ty(t: &Ty) -> bool {
    matches!(t, Ty::Named(n, _) if matches!(n.as_str(),
        "MutableState" | "State" | "DerivedState" | "MutableIntState" | "MutableFloatState" | "MutableLongState"
            | "MutableDoubleState" | "IntState" | "FloatState" | "LongState" | "DoubleState"))
}
